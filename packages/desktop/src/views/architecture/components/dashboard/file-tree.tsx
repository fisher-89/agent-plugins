import { ChevronRightIcon, FileText, FolderClosed, PlusIcon } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { TextTooltip } from '@/components/standard/tooltip';
import { ScrollArea, Button, Collapsible, CollapsibleContent, Badge } from '@/components/ui';

import type { ArchiFileEntry } from '../../../../types/generated/bindings';
import { useArchiFileTree } from '../../hooks/use-archi-file-tree';

/** 默认展开层级：相对路径段数 ≤ 3 的目录默认展开（更深层默认折叠）。 */
const DEFAULT_EXPAND_SEGMENTS = 3;

export interface FileTreeProps {
  root: string | null;
  /** 当前聚焦元素 FQN（`owners` 含它的条目高亮） */
  focused: string | null;
  /** 聚焦切换（纯前端，零取数） */
  onFocus: (fqn: string) => void;
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

/** 展开态判定：聚焦模块根节点及其祖先链强制展开优先，其次 per-path 翻转，最后默认前 3 段。 */
function isExpanded(
  path: string,
  flipped: ReadonlySet<string>,
  forcedOpen: ReadonlySet<string>,
): boolean {
  if (forcedOpen.has(path)) return true;
  const byDefault = path.split('/').length <= DEFAULT_EXPAND_SEGMENTS;
  return flipped.has(path) ? !byDefault : byDefault;
}

/** 归属标记节点（图标 + 色 + tooltip；`member` / `none` 不渲标记节点）。 */
function MarkBadge({
  entry,
  context,
}: {
  entry: ArchiFileEntry;
  context: RowContext;
}): React.JSX.Element | null {
  const focused = context.focused !== null && entry.owners.includes(context.focused);
  const owner = entry.owners[0];
  switch (entry.mark) {
    case 'elementRoot':
      return (
        <TextTooltip text={`切换至模块 ${owner}`}>
          <Button
            variant={focused ? 'default' : 'secondary'}
            size="xs"
            data-mark={entry.mark}
            data-testid="archi-file-mark"
          >
            模块 {owner.split('.').at(-1)}
            <ChevronRightIcon />
          </Button>
        </TextTooltip>
      );
    case 'ambiguous':
      return (
        <Badge
          variant="destructive"
          data-mark={entry.mark}
          data-testid="archi-file-mark"
          title={`多归属：${entry.owners.join('、')}`}
        >
          多归属
        </Badge>
      );
    case 'containsElements':
      return (
        <Badge
          variant="secondary"
          data-mark={entry.mark}
          data-testid="archi-file-mark"
          title={`含 ${entry.contains} 个模块`}
        >
          <PlusIcon />
          {entry.contains}
        </Badge>
      );
    default:
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

function FileEntry({
  entry,
  context,
}: {
  entry: ArchiFileEntry;
  context: RowContext;
}): React.JSX.Element {
  return (
    <>
      {entry.kind === 'dir' ? <FolderClosed /> : <FileText />}
      <span className={nameClass(entry.mark)}>{entry.name}</span>
      {entry.exists ? null : (
        <span className="text-fail" title="声明了但磁盘上不存在">
          缺失
        </span>
      )}
      <MarkBadge entry={entry} context={context} />
    </>
  );
}

/** 单条目行（递归渲染子树）：归属行整行点击圈定范围，其余标记只切展开（chevron 与
 * 整行同语义，故在 chevron 上阻断冒泡以免触发两次翻转）。 */
function FileRow({
  node,
  context,
}: {
  node: FileTreeNode;
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

  const row = (
    <div
      className={`flex w-full items-center gap-1 rounded-md h-8 pr-2 text-sm select-none text-nowrap [&_svg:not([class*='size-'])]:size-4 [&_svg]:shrink-0 hover:bg-accent hover:text-accent-foreground ${owned || hasChildren ? 'cursor-pointer' : ''} ${focused ? 'bg-muted' : ''}`}
      data-mark={entry.mark}
      data-path={entry.path}
      data-testid="archi-file-node"
      onClick={activate}
      ref={context.refOf(entry.path)}
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
          <ChevronRightIcon className={`transition-transform ${expanded ? 'rotate-90' : ''}`} />
        </button>
      ) : (
        <span className="w-4 shrink-0" />
      )}
      <FileEntry entry={entry} context={context} />
    </div>
  );

  if (!hasChildren) return row;

  return (
    <Collapsible open={expanded} onOpenChange={toggle}>
      {row}
      <CollapsibleContent className="ml-4">
        {children.map((child) => (
          <FileRow context={context} key={child.entry.path} node={child} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}

/** 聚焦反查（纯派生，零 state 写入）：聚焦模块的根节点条目即锚点（`mark = elementRoot`
 * 且 `owners` 含该 FQN；多 `path` 元素会有多条锚点），锚点及其祖先链强制展开、锚点之下
 * 不下钻（用户翻转态不被覆盖），并滚动到首个锚点。 */
function useFocusAnchors(
  entries: ArchiFileEntry[],
  focused: string | null,
): { forcedOpen: ReadonlySet<string>; refOf: RowContext['refOf'] } {
  const nodes = useRef<Map<string, HTMLDivElement>>(new Map());
  const anchors = useMemo(
    () =>
      focused === null
        ? []
        : entries
            .filter((entry) => entry.mark === 'elementRoot' && entry.owners.includes(focused))
            .map((entry) => entry.path),
    [entries, focused],
  );
  const forcedOpen = useMemo(() => {
    const open = new Set<string>();
    for (const anchor of anchors) {
      const segments = anchor.split('/');
      for (let index = 1; index <= segments.length; index += 1) {
        open.add(segments.slice(0, index).join('/'));
      }
    }
    return open;
  }, [anchors]);
  const refOf = useCallback(
    (path: string) =>
      (element: HTMLDivElement | null): void => {
        if (element === null) nodes.current.delete(path);
        else nodes.current.set(path, element);
      },
    [],
  );
  useEffect(() => {
    const first = anchors[0];
    if (first === undefined) return;
    nodes.current.get(first)?.scrollIntoView?.({ block: 'nearest' });
  }, [anchors]);
  return { forcedOpen, refOf };
}

/** 左栏文件树：数据面为后端扁平条目，层级 / 兄弟序 / 缩进与展开态全由本件派生。 */
export function FileTree({ root, focused, onFocus }: FileTreeProps): React.JSX.Element {
  const { tree, error: treeError } = useArchiFileTree(root);

  const [flipped, setFlipped] = useState<ReadonlySet<string>>(() => new Set());
  const onToggle = useCallback((path: string) => {
    setFlipped((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);
  const forest = useMemo(() => buildForest(tree?.entries ?? []), [tree?.entries]);
  const { forcedOpen, refOf } = useFocusAnchors(tree?.entries ?? [], focused);
  const context: RowContext = { focused, flipped, forcedOpen, onToggle, onFocus, refOf };

  if (treeError !== null) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-file-tree-error">
        文件树取数失败：{treeError}（画布照常渲染）
      </div>
    );
  }
  if (tree === null) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-file-tree-loading">
        解析中…
      </div>
    );
  }

  return (
    <ScrollArea className="h-full" data-testid="archi-file-tree">
      {forest.length === 0 ? (
        <div className="text-[13px] text-muted-foreground" data-testid="archi-file-tree-empty">
          仓库内无源文件
        </div>
      ) : (
        forest.map((node) => <FileRow context={context} key={node.entry.path} node={node} />)
      )}
    </ScrollArea>
  );
}
