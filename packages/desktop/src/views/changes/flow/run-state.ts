/**
 * run 步节点转换层：统一视图（`detail.runs[].nodes` ∪ `activeRun.nodes`）→ 图
 * overlay 运行步节点；边同源（`detail.runs[].edges` ∪ `activeRun.edges`）。零
 * react 依赖直测；**库读史常驻**（步节点自 `runs[].nodes` 派生收口后常驻上图，
 * 与运行中实时面走同一转换函数）。节点按后端稳定身份键
 * `(phase, attempt, step, instanceNo)` 直渲染——槽位配对 / `:seq` 堆叠机制整体
 * 退役（同键多实例由 instanceNo 区分）。
 */
import type {
  ChangeRunEdgeRecord,
  ChangeRunEntry,
  ChangeRunNodeRecord,
  ChangeStepKind,
  ChangeStepState,
  RunStepKind,
} from '../../../types/dto';
import { PIPELINE_PHASES } from './layout';
import type { FlowRoleLabel, RunStepGroup, RuntimeFlowNode } from './types';

/** 步词汇归一单点：库读史 snake 词（RunStepKind）→ 活面 camel 词
 *（ChangeStepKind）——十八值全词汇逐词对应（落库零过滤后读面同词汇）。 */
function normalizeKind(step: RunStepKind): ChangeStepKind {
  switch (step) {
    case 'static_check':
      return 'staticCheck';
    case 'test_execution':
      return 'testExecution';
    case 'phase_start':
      return 'phaseStart';
    case 'phase_log':
      return 'phaseLog';
    case 'verdict_gate':
      return 'verdictGate';
    case 'retry_gate':
      return 'retryGate';
    case 'whitelist_gate':
      return 'whitelistGate';
    case 'resolve_conflicts':
      return 'resolveConflicts';
    case 'spec_sync':
      return 'specSync';
    default:
      return step;
  }
}

/** 步词汇反向归一（活面 camel 词 → 边表身份串 snake 词）：节点身份键与后端
 * 边表端点 `"{phase}:{attempt}:{step}:{instance_no}"` 逐字可比。 */
function stepWord(kind: ChangeStepKind): string {
  switch (kind) {
    case 'staticCheck':
      return 'static_check';
    case 'testExecution':
      return 'test_execution';
    case 'phaseStart':
      return 'phase_start';
    case 'phaseLog':
      return 'phase_log';
    case 'verdictGate':
      return 'verdict_gate';
    case 'retryGate':
      return 'retry_gate';
    case 'whitelistGate':
      return 'whitelist_gate';
    case 'resolveConflicts':
      return 'resolve_conflicts';
    case 'specSync':
      return 'spec_sync';
    default:
      return kind;
  }
}

/** 节点身份键（后端边表端点口径）：`"{phase}:{attempt}:{step}:{instanceNo}"`。 */
export function nodeIdentityKey(node: RuntimeFlowNode): string {
  return `${node.phase}:${node.attempt}:${stepWord(node.runStepKind)}:${node.instanceNo}`;
}

/** 库读史节点行 → 活面步状态行（词汇归一收本文件单点）。 */
function stepFromRecord(record: ChangeRunNodeRecord): ChangeStepState {
  return {
    phase: record.phase,
    attempt: record.attempt,
    step: normalizeKind(record.step),
    instanceNo: record.instanceNo,
    status: record.status,
    sessionId: record.sessionId,
    detail: record.detail,
    outputRef: record.outputRef,
  };
}

/** 统一视图节点源合成（同一转换函数的输入面）：`runs[].nodes`（全词汇库读史，
 * runs 序 = startedAt 序即稳定分层序）∪ liveNodes（在飞 run 活节点）——收口后
 * 活节点褪去、库读史常驻。 */
export function unifiedRunNodes(
  runs: ChangeRunEntry[],
  liveNodes: ChangeStepState[],
): ChangeStepState[] {
  const persisted = runs.flatMap((run) => run.nodes.map(stepFromRecord));
  return [...persisted, ...liveNodes];
}

/** 统一视图边源合成：`runs[].edges`（库读史边表）∪ liveEdges（在飞 run 活面
 * 边表）——两来源同型（身份串端点 + 四 kind），前端零形状归一。 */
export function unifiedRunEdges(
  runs: ChangeRunEntry[],
  liveEdges: ChangeRunEdgeRecord[],
): ChangeRunEdgeRecord[] {
  const persisted = runs.flatMap((run) => run.edges);
  return [...persisted, ...liveEdges];
}

/** 步词汇 → 节点分组（四类节点可辨）：归档步归组与后端 `StepNode::group()`
 * 同判——commit / rebase / merge / finalize 归 git、preflight / seal 归
 * toolStep、specSync / resolveConflicts 归 workerAgent。 */
function groupOf(step: ChangeStepKind): RunStepGroup {
  if (
    step === 'executor' ||
    step === 'evaluator' ||
    step === 'decision' ||
    step === 'specSync' ||
    step === 'resolveConflicts'
  ) {
    return 'workerAgent';
  }
  if (step === 'commit' || step === 'rebase' || step === 'merge' || step === 'finalize') {
    return 'git';
  }
  if (step === 'verdictGate' || step === 'retryGate' || step === 'whitelistGate') return 'gate';
  return 'toolStep';
}

/** 步词汇 → role 标签（WorkerAgent 步角色；ToolStep / Git / Gate 为 null）。
 * 归档两 agent 步的 role 词与后端 `WorkerRole::as_str()` 同词面（sourceRef
 * 定式第三段逐字可比——节点 sessionId 缺席时的回退反查面）。 */
function roleOf(step: ChangeStepKind): FlowRoleLabel | null {
  if (step === 'specSync') return 'spec_sync';
  if (step === 'resolveConflicts') return 'resolve_conflicts';
  if (step === 'executor' || step === 'evaluator' || step === 'decision') return step;
  return null;
}

/** 步状态表 → 图 overlay 运行步节点（按后端稳定身份键直渲染；坑位校验：pipeline
 * 外相位的节点不入图）。 */
export function runStepNodes(steps: ChangeStepState[]): RuntimeFlowNode[] {
  const nodes: RuntimeFlowNode[] = [];
  const columnOrder = new Map<string, number>();
  for (const step of steps) {
    const colIndex = PIPELINE_PHASES.indexOf(step.phase);
    if (colIndex < 0) continue;
    const order = columnOrder.get(step.phase) ?? 0;
    columnOrder.set(step.phase, order + 1);
    nodes.push({
      id: `run:${step.phase}:${step.attempt}:${step.step}:${step.instanceNo}`,
      kind: 'runtime',
      phase: step.phase,
      attempt: step.attempt,
      colIndex,
      order,
      parentId: `col:${step.phase}`,
      runStepKind: step.step,
      group: groupOf(step.step),
      role: roleOf(step.step),
      instanceNo: step.instanceNo,
      status: step.status,
      sessionId: step.sessionId,
      detail: step.detail,
      outputRef: step.outputRef,
    });
  }
  return nodes;
}
