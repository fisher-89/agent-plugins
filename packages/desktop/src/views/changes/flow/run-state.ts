import type {
  ChangeRunEdgeRecord,
  ChangeRunEntry,
  ChangeRunNodeRecord,
  ChangeStepKind,
  ChangeStepState,
  RunStepKind,
} from '../../../types/dto';
import type { FlowRoleLabel, RunStepGroup, RuntimeFlowNode } from './types';

/** 步词汇归一单点：库读史 snake 词（RunStepKind）→ 活面 camel 词
 *（ChangeStepKind）——二十值全词汇逐词对应（落库零过滤后读面同词汇；
 * cleanup 为 identity 词，走 `default` 分支直返——commit / rebase / merge /
 * finalize / preflight / seal 同先例）。 */
function normalizeKind(step: RunStepKind): ChangeStepKind {
  switch (step) {
    case 'static_check':
      return 'staticCheck';
    case 'test_execution':
      return 'testExecution';
    case 'mutation_test':
      return 'mutationTest';
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
    case 'mutationTest':
      return 'mutation_test';
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

/** run 维度边端点键（图内边锚定单点）：`"<runId>:<身份键>"`——后端边表端点
 * 身份串无 run 段（跨 run 同名身份串合法：重启 / 中断重跑同 attempt），前拼
 * 归属 run id 后与节点侧同式可比，跨轮边不再错锚。 */
export function runNodeKey(node: RuntimeFlowNode): string {
  return `${node.runId}:${nodeIdentityKey(node)}`;
}

/** 库读史节点行 → 活面步状态行（词汇归一收本文件单点；四字段单一形状纯复制
 * 零转换——节点级毫秒数与活面 `ChangeStepState` 同形）。 */
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
    startedAt: record.startedAt,
    endedAt: record.endedAt,
    tokenInput: record.tokenInput,
    tokenOutput: record.tokenOutput,
  };
}

/** 统一视图步行：`ChangeStepState` + 归属 run id（重启 / 中断重跑同
 * `(phase, attempt, step, instanceNo)` 的跨 run 同名身份键以 run 归属段区分）
 * + 活面来源标注 `live`（走秒仅活面 running 节点挂载，D7）。 */
export interface UnifiedRunStep extends ChangeStepState {
  runId: string;
  /** 活面（在飞 run）来源 true / 库读史来源 false */
  live: boolean;
}

/** 统一视图边行：`ChangeRunEdgeRecord` + 归属 run id（端点身份串按归属 run 内
 * 节点解析——跨 run 同名端点不再错锚到他轮节点）。 */
export interface UnifiedRunEdge extends ChangeRunEdgeRecord {
  runId: string;
}

/** 统一视图活面缝：在飞 run 的节点 / 边与 run id 同源捆绑（双面一致的单参
 * 形态；`ActiveRunView` 结构兼容）。 */
export interface LiveRunFace {
  runId: string;
  nodes: ChangeStepState[];
  edges: ChangeRunEdgeRecord[];
}

/** 统一视图节点源合成（同一转换函数的输入面）：`runs[].nodes`（全词汇库读史，
 * runs 序 = startedAt 序即稳定分层序）∪ liveRun.nodes（在飞 run 活节点）——
 * 收口后活节点褪去、库读史常驻；两源皆携归属 run id（节点 id 与边锚定的 run
 * 维度同源）；`live` 按源标注（persisted false / live true——在飞 run 的库
 * start 行 nodes 恒空，两源无同键重复、标注无歧义）。 */
export function unifiedRunNodes(
  runs: ChangeRunEntry[],
  liveRun: LiveRunFace | null,
): UnifiedRunStep[] {
  const persisted = runs.flatMap((run) =>
    run.nodes.map((record) => ({ ...stepFromRecord(record), runId: run.runId, live: false })),
  );
  const live =
    liveRun === null
      ? []
      : liveRun.nodes.map((step) => ({ ...step, runId: liveRun.runId, live: true }));
  return [...persisted, ...live];
}

/** 统一视图边源合成：`runs[].edges`（库读史边表）∪ liveRun.edges（在飞 run 活
 * 面边表）——两来源同型（身份串端点 + 四 kind），各携归属 run id（边端点按
 * 归属 run 内节点解析）。 */
export function unifiedRunEdges(
  runs: ChangeRunEntry[],
  liveRun: LiveRunFace | null,
): UnifiedRunEdge[] {
  const persisted = runs.flatMap((run) => run.edges.map((edge) => ({ ...edge, runId: run.runId })));
  const live =
    liveRun === null ? [] : liveRun.edges.map((edge) => ({ ...edge, runId: liveRun.runId }));
  return [...persisted, ...live];
}

/** 步词汇 → 节点分组（四类节点可辨）：归档步归组与后端 `StepNode::group()`
 * 同判——commit / rebase / merge / cleanup / finalize 归 git、preflight / seal 归
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
  if (
    step === 'commit' ||
    step === 'rebase' ||
    step === 'merge' ||
    step === 'cleanup' ||
    step === 'finalize'
  ) {
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

/** 步状态表 → 图 overlay 运行步节点（按「run id + 后端稳定身份键」直渲染——
 * 节点 id 携归属 run 段，跨 run 同名身份键互不顶替）。列序由 `phaseOrder` 显式
 * 传入（= `detail.pipeline.map(p => p.phase)`，后端相位表出线序；前端零静态相
 * 位序常量）——坑位校验：`phaseOrder` 外相位的节点不入图。 */
export function runStepNodes(steps: UnifiedRunStep[], phaseOrder: string[]): RuntimeFlowNode[] {
  const nodes: RuntimeFlowNode[] = [];
  const columnOrder = new Map<string, number>();
  for (const step of steps) {
    const colIndex = phaseOrder.indexOf(step.phase);
    if (colIndex < 0) continue;
    const order = columnOrder.get(step.phase) ?? 0;
    columnOrder.set(step.phase, order + 1);
    nodes.push({
      id: `run:${step.runId}:${step.phase}:${step.attempt}:${step.step}:${step.instanceNo}`,
      kind: 'runtime',
      phase: step.phase,
      attempt: step.attempt,
      colIndex,
      order,
      parentId: `col:${step.phase}`,
      runId: step.runId,
      runStepKind: step.step,
      group: groupOf(step.step),
      role: roleOf(step.step),
      instanceNo: step.instanceNo,
      status: step.status,
      sessionId: step.sessionId,
      detail: step.detail,
      outputRef: step.outputRef,
      startedAt: step.startedAt,
      endedAt: step.endedAt,
      tokenInput: step.tokenInput,
      tokenOutput: step.tokenOutput,
      live: step.live,
    });
  }
  return nodes;
}
