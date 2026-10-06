import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';

import { Badge } from '@/components/ui/badge';

import type { ChangeStepKind, ChangeStepStatus } from '../../../types/dto';
import type { RunStepGroup, RuntimeFlowNode } from './types';

type RunStepNodeData = { node: RuntimeFlowNode };

export type RunStepFlowNode = Node<RunStepNodeData, 'runStep'>;

/** 分组徽章文案（三类节点可辨）。 */
const GROUP_LABEL: Record<RunStepGroup, string> = {
  workerAgent: 'WorkerAgent',
  toolStep: 'ToolStep',
  gate: 'Gate',
};

/** 步词汇展示文案。 */
const STEP_LABEL: Record<ChangeStepKind, string> = {
  executor: '执行',
  evaluator: '评估',
  decision: '决策',
  phaseStart: '开启阶段',
  staticCheck: '静态检查',
  testExecution: '测试执行',
  phaseLog: '评估落账',
  verdictGate: 'verdict 门',
  retryGate: '重试门',
  whitelistGate: '白名单门',
};

/** 步状态视觉：running pulse / failed 红态 / stopped 灰态 / passed 常态。 */
function statusClass(status: ChangeStepStatus): string {
  switch (status) {
    case 'running':
      return 'border-primary bg-card animate-pulse';
    case 'failed':
      return 'border-fail bg-fail-bg';
    case 'stopped':
      return 'border-dashed border-muted-foreground bg-muted/30 text-muted-foreground';
    default:
      return 'border-border bg-card';
  }
}

/** 状态缀语：running / failed / stopped 呈现，passed 不缀。 */
function StatusSuffix({ status }: { status: ChangeStepStatus }): React.JSX.Element | null {
  if (status === 'running') return <span className="text-primary">运行中</span>;
  if (status === 'failed') return <span className="text-fail">失败</span>;
  if (status === 'stopped') return <span>已停</span>;
  return null;
}

/** 头行：分组徽章 + 步文案 + attempt + 状态缀语。 */
function RunStepHeader({ node }: { node: RuntimeFlowNode }): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <Badge
        variant={node.group === 'workerAgent' ? 'active' : 'inv2'}
        data-testid="run-step-group"
      >
        {GROUP_LABEL[node.group]}
      </Badge>
      <span data-testid="run-step-kind">{STEP_LABEL[node.runStepKind]}</span>
      <span>attempt {node.attempt}</span>
      <StatusSuffix status={node.status} />
    </div>
  );
}

/** 四侧隐形把手（id：top / bottom / left / right）——边锚定方向与事件节点
 * 同式（前进右出左入、回跳左出右入、同列重试下出上入）。 */
function StepHandles(): React.JSX.Element {
  const hidden = { opacity: 0 };
  const anchor = { isConnectable: false, style: hidden } as const;
  return (
    <>
      <Handle type="target" id="top" position={Position.Top} {...anchor} />
      <Handle type="target" id="left" position={Position.Left} {...anchor} />
      <Handle type="target" id="right" position={Position.Right} {...anchor} />
      <Handle type="source" id="bottom" position={Position.Bottom} {...anchor} />
      <Handle type="source" id="left" position={Position.Left} {...anchor} />
      <Handle type="source" id="right" position={Position.Right} {...anchor} />
    </>
  );
}

/** 运行步节点：类型徽章 + 步文案 + 状态视觉。 */
export function RunStepNode({ data }: NodeProps<RunStepFlowNode>): React.JSX.Element {
  const { node } = data;
  return (
    <div
      className={`relative rounded-md border px-2 py-1.5 text-[11px] ${statusClass(node.status)}`}
      data-testid="run-step-node"
      data-run-status={node.status}
      data-run-group={node.group}
    >
      <StepHandles />
      <RunStepHeader node={node} />
      {node.detail !== null && (
        <div className="mt-1 break-words text-muted-foreground" data-testid="run-step-detail">
          {node.detail}
        </div>
      )}
    </div>
  );
}
