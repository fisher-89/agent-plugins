import { ChevronDown, ChevronRight } from 'lucide-react';
import { useCallback, useState } from 'react';

import type { DirNode } from '../../../types/generated/bindings';

/** 默认展开层级：相对路径段数 ≤ 2 的节点默认展开（更深层默认折叠） */
const DEFAULT_EXPAND_SEGMENTS = 2;

/** 每层级缩进宽度（px） */
const INDENT_STEP = 16;

/**
 * 展开态判定：默认展开由路径段数派生（前 2 级），flipped 集合存显式翻转
 * （per-path 键稳定，调深 / 刷新后同名目录保持用户操作态，不随重解析重置）。
 */
function isExpanded(path: string, flipped: ReadonlySet<string>): boolean {
  const byDefault = path.split('/').length <= DEFAULT_EXPAND_SEGMENTS;
  return flipped.has(path) ? !byDefault : byDefault;
}

/**
 * 单目录行（递归渲染子树）：目录名 + 子树聚合统计（文件 / 代码）+ 缩进层级。
 * 折叠节点子树不渲染 DOM（按需渲染收敛树面规模）；展开 / 折叠纯本地 state，
 * 零新增 invoke。
 */
function DirNodeRow({
  node,
  level,
  flipped,
  onToggle,
}: {
  node: DirNode;
  level: number;
  flipped: ReadonlySet<string>;
  onToggle: (path: string) => void;
}): React.JSX.Element {
  const expanded = isExpanded(node.path, flipped);
  return (
    <>
      <div
        className="flex items-center gap-2 rounded-md py-1 pr-2 hover:bg-muted"
        data-path={node.path}
        data-testid="info-dir-node"
        style={{ paddingLeft: 8 + level * INDENT_STEP }}
      >
        <button
          type="button"
          aria-expanded={expanded}
          className="flex min-w-0 cursor-pointer items-center gap-1 border-0 bg-transparent p-0 text-left hover:text-primary"
          data-testid="info-dir-toggle"
          onClick={() => onToggle(node.path)}
        >
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          <span className="truncate text-sm">{node.name}</span>
        </button>
        <span className="ml-auto shrink-0 text-xs tabular-nums text-muted-foreground">
          {node.files} 文件 · {node.code.toLocaleString()} 代码行
        </span>
      </div>
      {expanded &&
        node.children.map((child) => (
          <DirNodeRow
            flipped={flipped}
            key={child.path}
            level={level + 1}
            node={child}
            onToggle={onToggle}
          />
        ))}
    </>
  );
}

/**
 * 目录树面：树上数据为单次解析结果的聚合投影，本组件零取数——展开 / 折叠为
 * per-path 本地 state（显式翻转集合），默认展开前 2 级。
 */
export function DirTree({ nodes }: { nodes: DirNode[] }): React.JSX.Element {
  const [flipped, setFlipped] = useState<ReadonlySet<string>>(() => new Set());
  const toggle = useCallback((path: string) => {
    setFlipped((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);
  return (
    <div data-testid="info-dir-tree">
      {nodes.map((node) => (
        <DirNodeRow flipped={flipped} key={node.path} level={0} node={node} onToggle={toggle} />
      ))}
    </div>
  );
}
