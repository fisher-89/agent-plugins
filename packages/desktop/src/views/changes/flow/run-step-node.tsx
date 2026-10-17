import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import { useEffect, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { HoverCard, HoverCardContent, HoverCardTrigger } from '@/components/ui/hover-card';

import type { ChangeStepKind, ChangeStepStatus } from '../../../types/dto';
import { formatTokens, runningDurationLabel, settledDurationLabel } from './metrics';
import type { RunStepGroup, RuntimeFlowNode } from './types';

type RunStepNodeData = { node: RuntimeFlowNode };

export type RunStepFlowNode = Node<RunStepNodeData, 'runStep'>;

/** 走秒 tick 周期（活面 running 节点耗时每秒刷新）。 */
const TICK_MS = 1000;

/** 分组徽章文案（四类节点可辨）。 */
const GROUP_LABEL: Record<RunStepGroup, string> = {
  workerAgent: 'WorkerAgent',
  toolStep: 'ToolStep',
  git: 'Git',
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
  mutationTest: '突变执行',
  clean: '基线恢复',
  phaseLog: '评估落账',
  verdictGate: 'verdict 门',
  retryGate: '重试门',
  whitelistGate: '白名单门',
  preflight: '归档前置',
  commit: 'worktree 提交',
  rebase: '分支重放',
  resolveConflicts: '解冲突',
  merge: '主仓合入',
  specSync: 'spec 同步',
  cleanup: 'worktree 清理',
  seal: '归档收口',
  finalize: '归档落盘',
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
        variant={node.group === 'workerAgent' ? 'default' : 'secondary'}
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

/** 标题下常驻度量行：`⏱ <耗时>` 段 + `tokens <in>/<out>` 段（token 段仅
 * workerAgent 组渲染，running 亦渲染、缺席 `—/—`），两段齐备以 ` ▸ ` 分隔；
 * 行渲染条件 = 至少一段在场（0ms 瞬步且非 workerAgent 整行省显）。耗时段
 * running 走秒（活面）与静态收口两态经 metrics 单点判定（缺席 / 旧数据 "-"）。 */
function StepMetricsRow({ node, tickNow }: { node: RuntimeFlowNode; tickNow: number }) {
  const ticking = node.live && node.status === 'running';
  const duration = ticking
    ? runningDurationLabel(node.startedAt, tickNow)
    : settledDurationLabel(node.startedAt, node.endedAt);
  const showTokens = node.group === 'workerAgent';
  if (duration === null && !showTokens) return null;
  return (
    <div className="mt-1 text-muted-foreground" data-testid="run-step-metrics">
      {duration !== null && <span data-testid="run-step-duration">⏱ {duration}</span>}
      {showTokens && (
        <>
          {duration !== null && <span> ▸ </span>}
          <span data-testid="run-step-tokens">
            tokens {formatTokens(node.tokenInput)}/{formatTokens(node.tokenOutput)}
          </span>
        </>
      )}
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

/** 运行步节点：类型徽章 + 步文案 + 状态视觉 + 常驻度量行（走秒仅活面
 * running 节点挂载——组件内自持 interval 局部重渲染本节点，翻态 / 卸载即清）。 */
export function RunStepNode({ data }: NodeProps<RunStepFlowNode>): React.JSX.Element {
  const { node } = data;
  const ticking = node.live && node.status === 'running';
  const [tickNow, setTickNow] = useState(() => Date.now());
  useEffect(() => {
    if (!ticking) return;
    setTickNow(Date.now());
    const timer = setInterval(() => setTickNow(Date.now()), TICK_MS);
    return () => clearInterval(timer);
  }, [ticking]);
  return (
    <div
      className={`relative rounded-md border px-2 py-1.5 text-[11px] ${statusClass(node.status)}`}
      data-testid="run-step-node"
      data-run-status={node.status}
      data-run-group={node.group}
    >
      <StepHandles />
      <RunStepHeader node={node} />
      <StepMetricsRow node={node} tickNow={tickNow} />
      {node.detail !== null && (
        <HoverCard>
          <HoverCardTrigger
            render={
              <div className="mt-1 text-muted-foreground truncate" data-testid="run-step-detail">
                {node.detail}
              </div>
            }
          ></HoverCardTrigger>
          <HoverCardContent>{node.detail}</HoverCardContent>
        </HoverCard>
      )}
    </div>
  );
}
