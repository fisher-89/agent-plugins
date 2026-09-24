import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';

import { Badge } from '@/components/ui/badge';

import type { ActiveFlowNode, EvalFlowNode, FlowNode, InterruptedFlowNode } from './types';

/**
 * 事件自定义节点（nodeTypes 键 `event`），按 kind 三分类视觉：
 * - eval：实心（pass 绿 / fail 红 token），stale 半透明淡化（废弃支线），
 *   skipped / backtrack 以徽标文案保留（迁移旧 Attempt 块语义）；
 * - active：pulse 运行中，无 verdict，列内接流末端；
 * - interrupted：dashed 灰显留档，展示 startAt ~ endAt（迁移旧中断留档语义）。
 *
 * 顶 / 底各一枚隐形 Handle 供链式边锚定（边锚点依赖 Handle 的 DOM 度量；
 * Handle 依赖 ReactFlow store context，脱离图直渲染本组件时需外包
 * ReactFlowProvider，不依赖 ResizeObserver）。
 */

type EventNodeData = { node: FlowNode };

export type EventFlowNode = Node<EventNodeData, 'event'>;

function nodeClass(node: FlowNode): string {
  const base = 'relative rounded-md border px-2 py-1.5 text-[11px]';
  if (node.kind === 'eval') {
    const tone =
      node.record.verdict === 'pass' ? 'border-pass bg-pass-bg' : 'border-fail bg-fail-bg';
    return `${base} ${tone}${node.record.stale ? ' opacity-50' : ''}`;
  }
  if (node.kind === 'active') return `${base} border-primary bg-card animate-pulse`;
  return `${base} border-dashed border-muted-foreground bg-muted/30 text-muted-foreground`;
}

function EvalBody({ record }: { record: EvalFlowNode['record'] }): React.JSX.Element {
  return (
    <div>
      <div className="flex flex-wrap items-center gap-1.5" data-testid="attempt-meta">
        <span>attempt {record.attempt ?? '—'}</span>
        <Badge variant={record.verdict === 'pass' ? 'pass' : 'fail'} data-testid="attempt-verdict">
          {record.verdict}
        </Badge>
        {record.skipped && <span className="text-muted-foreground">skipped</span>}
        {record.stale && <span className="text-muted-foreground">stale</span>}
      </div>
      {(record.backtrackTo !== null || record.backtrackReason !== null) && (
        <div className="mt-1 text-warn" data-testid="backtrack">
          ↩ 回跳至 {record.backtrackTo ?? '?'}
          {record.backtrackReason !== null && `：${record.backtrackReason}`}
        </div>
      )}
    </div>
  );
}

function ActiveBody({ node }: { node: ActiveFlowNode }): React.JSX.Element {
  return (
    <div className="flex items-center gap-1.5">
      <span className="inline-block h-1.5 w-1.5 rounded-full bg-primary" />
      <span>运行中 · attempt {node.attempt}</span>
    </div>
  );
}

function InterruptedBody({ node }: { node: InterruptedFlowNode }): React.JSX.Element {
  return (
    <div className="flex flex-col gap-0.5">
      <span>中断留档 · attempt {node.attempt}</span>
      <span>start: {node.startAt ?? '—'}</span>
      <span>end: {node.endAt ?? '—'}</span>
    </div>
  );
}

/** 事件节点：按 kind 三分类视觉；事件点击的上抛由 ChangeFlowGraph 的 onNodeClick 承接 */
export function FlowEventNode({ data }: NodeProps<EventFlowNode>): React.JSX.Element {
  return (
    <div className={nodeClass(data.node)} data-testid="flow-node">
      <Handle type="target" position={Position.Top} isConnectable={false} style={{ opacity: 0 }} />
      {data.node.kind === 'eval' && <EvalBody record={data.node.record} />}
      {data.node.kind === 'active' && <ActiveBody node={data.node} />}
      {data.node.kind === 'interrupted' && <InterruptedBody node={data.node} />}
      <Handle
        type="source"
        position={Position.Bottom}
        isConnectable={false}
        style={{ opacity: 0 }}
      />
    </div>
  );
}
