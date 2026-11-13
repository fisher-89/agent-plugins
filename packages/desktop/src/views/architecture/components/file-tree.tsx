/**
 * 左栏文件树 + 分段控件。
 *
 * 数据面为后端扁平条目（`path` 字节序），层级 / 兄弟序 / 缩进全由本件按 `path` 派生
 * （目录先于文件、各自按 `name` 序）；展开态为 per-path 翻转集合、折叠不渲子树 DOM。
 * 五种归属标记只表达归属，MUST NOT 承载漂移违例 / 门禁 / 契约状态。数据面与信息页的
 * 目录树不同（marks / owners 对行数聚合），故不与之抽公共件。
 */

import { ChevronDown, ChevronRight, FileText, FolderClosed } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ScrollArea } from '@/components/ui/scroll-area';

import type { ArchiFileEntry } from '../../../types/generated/bindings';

/** 默认展开层级：相对路径段数 ≤ 2 的目录默认展开（更深层默认折叠）。 */
const DEFAULT_EXPAND_SEGMENTS = 2;

/** 每层级缩进宽度（px）。 */
const INDENT_STEP = 16;

/** 左栏树视图（分段控件的三值；缺省 `file`）。 */
export type TreeMode = 'file' | 'element' | 'contract';

export interface FileTreeProps {
  /** 后端扁平条目（`path` 字节序；层级由本件按 `path` 派生） */
  entries: ArchiFileEntry[];
  /** 当前聚焦元素 FQN（`owners` 含它的条目高亮） */
  focused: string | null;
  /** 聚焦切换（纯前端，零取数） */
  onFocus: (fqn: string) => void;
}

export interface TreeModeSwitchProps {
  /** 当前左栏视图 */
  mode: TreeMode;
  /** 视图切换（只换左栏组件，`focus` 共用） */
  onChange: (mode: TreeMode) => void;
}

/** 派生森林节点（文件内形态，不导出）。 */
interface FileTreeNode {
  entry: ArchiFileEntry;
  children: FileTreeNode[];
}

/** 扁平条目 → 森林：按 `path` 切段挂父（父缺席的条目挂根）。 */
function buildForest(entries: ArchiFileEntry[]): FileTreeNode[] {
  const byPath = new Map<string, FileTreeNode>();
  for (const entry of entries) byPath.set(entry.path, { entry, children: [] });
  const roots: FileTreeNode[] = [];
  for (const node of byPath.values()) {
    const cut = node.entry.path.lastIndexOf('/');
    const parent = cut < 0 ? undefined : byPath.get(node.entry.path.slice(0, cut));
    if (parent === undefined) roots.push(node);
    else parent.children.push(node);
  }
  const sortSiblings = (nodes: FileTreeNode[]): void => {
    nodes.sort((left, right) => {
      const leftRank = left.entry.kind === 'dir' ? 0 : 1;
      const rightRank = right.entry.kind === 'dir' ? 0 : 1;
      if (leftRank !== rightRank) return leftRank - rightRank;
      return left.entry.name.localeCompare(right.entry.name);
    });
    for (const node of nodes) sortSiblings(node.children);
  };
  sortSiblings(roots);
  return roots;
}

/** 展开态判定：聚焦命中的祖先链强制展开优先，其次 per-path 翻转，最后默认前 2 段。 */
function isExpanded(
  path: string,
  flipped: ReadonlySet<string>,
  forcedOpen: ReadonlySet<string>,
): boolean {
  if (forcedOpen.has(path)) return true;
  const byDefault = path.split('/').length <= DEFAULT_EXPAND_SEGMENTS;
  return flipped.has(path) ? !byDefault : byDefault;
}

/** 归属标记（图标 + 色 + tooltip 按词表五值；`none` 无标记零 tooltip）。 */
function MarkBadge({ entry }: { entry: ArchiFileEntry }): React.JSX.Element | null {
  const owner = entry.owners[0];
  switch (entry.mark) {
    case 'elementRoot':
      return (
        <span
          className="shrink-0 rounded-full bg-primary size-2"
          data-mark={entry.mark}
          data-testid="archi-file-mark"
          title={`模块 ${owner ?? ''}`}
        />
      );
    case 'ambiguous':
      return (
        <span
          className="shrink-0 rounded-full bg-warn size-2"
          data-mark={entry.mark}
          data-testid="archi-file-mark"
          title={`多归属：${entry.owners.join('、')}`}
        />
      );
    case 'member':
      return (
        <span
          className="h-3 shrink-0 border-l-2 border-muted-foreground/60"
          data-mark={entry.mark}
          data-testid="archi-file-mark"
          title={`属于 ${owner ?? ''}`}
        />
      );
    case 'containsElements':
      return (
        <span className="flex shrink-0 items-center gap-1">
          <span
            className="rounded-full border border-muted-foreground size-2"
            data-mark={entry.mark}
            data-testid="archi-file-mark"
            title={`含 ${entry.contains} 个模块`}
          />
          <span className="text-[11px] tabular-nums text-muted-foreground">·{entry.contains}</span>
        </span>
      );
    case 'none':
      return null;
  }
}

interface RowContext {
  focused: string | null;
  flipped: ReadonlySet<string>;
  forcedOpen: ReadonlySet<string>;
  onToggle: (path: string) => void;
  onFocus: (fqn: string) => void;
  refOf: (path: string) => (element: HTMLDivElement | null) => void;
}

/** 行名样式（按词表五值：`elementRoot` / `ambiguous` 加粗、`member` 弱化、其余常规）。 */
function nameClass(mark: ArchiFileEntry['mark']): string {
  switch (mark) {
    case 'elementRoot':
      return 'font-medium text-primary';
    case 'ambiguous':
      return 'font-medium text-warn';
    case 'member':
      return 'text-muted-foreground';
    default:
      return '';
  }
}

/** 单条目行（递归渲染子树）：归属行整行点击圈定范围，其余标记只切展开（chevron 与
 * 整行同语义，故在 chevron 上阻断冒泡以免触发两次翻转）。 */
function FileRow({
  node,
  level,
  context,
}: {
  node: FileTreeNode;
  level: number;
  context: RowContext;
}): React.JSX.Element {
  const { entry, children } = node;
  const expanded = isExpanded(entry.path, context.flipped, context.forcedOpen);
  const hasChildren = children.length > 0;
  const owned = entry.mark === 'elementRoot' || entry.mark === 'member';
  const focused = context.focused !== null && entry.owners.includes(context.focused);
  const toggle = (): void => context.onToggle(entry.path);
  const activate = (): void => {
    if (owned) {
      const owner = entry.owners[0];
      if (owner !== undefined) context.onFocus(owner);
      return;
    }
    if (hasChildren) toggle();
  };
  return (
    <>
      <div
        className={`flex items-center gap-1 rounded-md py-0.5 pr-2 hover:bg-muted ${owned || hasChildren ? 'cursor-pointer' : ''} ${focused ? 'bg-muted' : ''}`}
        data-mark={entry.mark}
        data-path={entry.path}
        data-testid="archi-file-node"
        onClick={activate}
        ref={context.refOf(entry.path)}
        style={{ paddingLeft: 8 + level * INDENT_STEP }}
        title={entry.path}
      >
        {hasChildren ? (
          <button
            type="button"
            aria-expanded={expanded}
            aria-label={`${expanded ? '收起' : '展开'} ${entry.path}`}
            className="shrink-0 cursor-pointer border-0 bg-transparent p-0 text-muted-foreground"
            onClick={(event) => {
              event.stopPropagation();
              toggle();
            }}
          >
            {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </button>
        ) : (
          <span className="w-[14px] shrink-0" />
        )}
        <span className="flex min-w-0 flex-1 items-center gap-1.5 text-left">
          {entry.kind === 'dir' ? (
            <FolderClosed className="shrink-0 text-muted-foreground" size={14} />
          ) : (
            <FileText className="shrink-0 text-muted-foreground" size={14} />
          )}
          <span className={`truncate text-sm ${nameClass(entry.mark)}`}>{entry.name}</span>
          {entry.exists ? null : (
            <span className="shrink-0 text-[11px] text-fail" title="声明了但磁盘上不存在">
              缺失
            </span>
          )}
          <MarkBadge entry={entry} />
        </span>
      </div>
      {expanded && hasChildren
        ? children.map((child) => (
            <FileRow context={context} key={child.entry.path} level={level + 1} node={child} />
          ))
        : null}
    </>
  );
}

/** 聚焦反查（纯派生，零 state 写入）：`owners` 含该 FQN 的条目即命中（多 `path`
 * 元素会有多条命中），祖先链强制展开（用户翻转态不被覆盖），并滚动到首条。 */
function useFocusAnchors(
  entries: ArchiFileEntry[],
  focused: string | null,
): { hitPaths: string[]; forcedOpen: ReadonlySet<string>; refOf: RowContext['refOf'] } {
  const nodes = useRef<Map<string, HTMLDivElement>>(new Map());
  const hitPaths = useMemo(
    () =>
      focused === null
        ? []
        : entries.filter((entry) => entry.owners.includes(focused)).map((entry) => entry.path),
    [entries, focused],
  );
  const forcedOpen = useMemo(() => {
    const open = new Set<string>();
    for (const hit of hitPaths) {
      const segments = hit.split('/');
      for (let index = 1; index < segments.length; index += 1) {
        open.add(segments.slice(0, index).join('/'));
      }
    }
    return open;
  }, [hitPaths]);
  const refOf = useCallback(
    (path: string) =>
      (element: HTMLDivElement | null): void => {
        if (element === null) nodes.current.delete(path);
        else nodes.current.set(path, element);
      },
    [],
  );
  useEffect(() => {
    const first = hitPaths[0];
    if (first === undefined) return;
    nodes.current.get(first)?.scrollIntoView?.({ block: 'nearest' });
  }, [hitPaths]);
  return { hitPaths, forcedOpen, refOf };
}

/** 树底部可折叠图例（说明树面 = 判定面）。 */
function Legend({ open, onToggle }: { open: boolean; onToggle: () => void }): React.JSX.Element {
  return (
    <div
      className="mt-1 shrink-0 text-[11px] text-muted-foreground"
      data-testid="archi-file-tree-legend"
    >
      <button
        type="button"
        aria-expanded={open}
        className="cursor-pointer border-0 bg-transparent p-0 text-[11px] text-muted-foreground hover:text-foreground"
        onClick={onToggle}
      >
        图例
      </button>
      {open ? (
        <div className="mt-0.5 leading-snug">
          仅显示判定面源文件（测试文件与架构配置 exclude 命中项不在树中；none 行 = 无元素认领）
        </div>
      ) : null}
    </div>
  );
}

/** 左栏文件树：数据面为后端扁平条目，层级 / 兄弟序 / 缩进与展开态全由本件派生。 */
export function FileTree({ entries, focused, onFocus }: FileTreeProps): React.JSX.Element {
  const [flipped, setFlipped] = useState<ReadonlySet<string>>(() => new Set());
  const [legendOpen, setLegendOpen] = useState(false);
  const onToggle = useCallback((path: string) => {
    setFlipped((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);
  const forest = useMemo(() => buildForest(entries), [entries]);
  const { hitPaths, forcedOpen, refOf } = useFocusAnchors(entries, focused);
  const context: RowContext = { focused, flipped, forcedOpen, onToggle, onFocus, refOf };
  return (
    <section className="min-h-0 flex flex-1 flex-col" data-testid="archi-file-tree">
      <h2 className="m-0 mb-1.5 text-[13px] shrink-0 font-medium text-muted-foreground">文件</h2>
      {focused !== null && hitPaths.length === 0 ? (
        <div
          className="mb-1.5 text-[11px] text-muted-foreground"
          data-testid="archi-file-tree-focus-miss"
        >
          该元素在树上无可定位节点（容器元素不持有文件，或其源文件均在判定面之外）
        </div>
      ) : null}
      {entries.length === 0 ? (
        <div className="text-[13px] text-muted-foreground" data-testid="archi-file-tree-empty">
          仓库内无源文件
        </div>
      ) : (
        <ScrollArea className="flex-1 min-h-0 pr-2">
          {forest.map((node) => (
            <FileRow context={context} key={node.entry.path} level={0} node={node} />
          ))}
        </ScrollArea>
      )}
      <Legend open={legendOpen} onToggle={() => setLegendOpen((open) => !open)} />
    </section>
  );
}

/**
 * 左栏视图分段控件（缺省「文件」；三视图 = 文件 / 元素 / 契约）。文件 / 元素两视图的
 * 分段语义与联动逐字不变，契约视图的内容归 `ContractPanel`。
 */
export function TreeModeSwitch({ mode, onChange }: TreeModeSwitchProps): React.JSX.Element {
  const options: { mode: TreeMode; label: string }[] = [
    { mode: 'file', label: '文件' },
    { mode: 'element', label: '元素' },
    { mode: 'contract', label: '契约' },
  ];
  return (
    <div className="mb-1.5 flex shrink-0 gap-1" data-testid="archi-tree-mode" role="group">
      {options.map((option) => (
        <button
          key={option.mode}
          type="button"
          aria-pressed={mode === option.mode}
          className={`cursor-pointer rounded-md border px-2 py-0.5 text-[11px] ${
            mode === option.mode
              ? 'border-primary/40 bg-muted font-medium text-foreground'
              : 'border-transparent bg-transparent text-muted-foreground hover:text-foreground'
          }`}
          data-mode={option.mode}
          onClick={() => onChange(option.mode)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
