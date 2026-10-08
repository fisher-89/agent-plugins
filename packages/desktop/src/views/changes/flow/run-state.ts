/**
 * run 状态纯 reducer：`RunUpdate` 流 → 前端 run 视图模型（ChangeFlowRunState）
 * 的纯归并，与运行步节点（图 overlay 输入）推导。零 react 依赖直测；
 * 不持久化——收口后释放订阅，图回落 ChangeDetail 派生规则。
 */
import type {
  AgentEvent,
  ChangeRunSnapshot,
  ChangeRunStatus,
  ChangeRunSummary,
  ChangeStepKind,
  ChangeStepState,
  RunUpdate,
} from '../../../types/dto';
import { PIPELINE_PHASES } from './layout';
import type { FlowRoleLabel, RunStepGroup, RuntimeFlowNode } from './types';

/** ask 中断载荷（RunUpdate::Ask / 快照的 ask 字段同形） */
interface RunAskPayload {
  question: string;
  options: string[];
}

/** 前端 run 视图模型：状态机镜像 + 步状态表 + 最近会话事件缓存 */
export interface ChangeFlowRunState {
  runId: string;
  status: ChangeRunStatus;
  phase: string | null;
  attempt: number | null;
  ask: RunAskPayload | null;
  /** phase 间停等确认的相位（waitingConfirm 时非空） */
  confirmPhase: string | null;
  /** 步状态表（RunUpdate::Step 到达序，running → 终态成对） */
  steps: ChangeStepState[];
  /** 终态记因（终局状态时非空） */
  finishedReason: string | null;
  /** 最近会话事件缓存（key = sessionId，seq 去重保序） */
  liveEvents: Record<string, AgentEvent[]>;
}

/** 重挂快照 → run 视图模型初值；无运行 run（null）即 null。 */
export function initialRunState(snapshot: ChangeRunSnapshot | null): ChangeFlowRunState | null {
  if (snapshot === null) return null;
  return {
    runId: snapshot.runId,
    status: snapshot.status,
    phase: snapshot.phase,
    attempt: snapshot.attempt,
    ask: snapshot.ask,
    confirmPhase: snapshot.status === 'waitingConfirm' ? snapshot.phase : null,
    steps: [],
    finishedReason: null,
    liveEvents: {},
  };
}

/** 发起摘要 → run 视图模型初值（发起路径唯一状态种子）：发起即替换空态 /
 * 旧终态冻结面，后续 update 归并自此起步——否则 null 态与终态守卫会把
 * 新 run 的全部信封丢弃（发起后流程图零呈现）。相位 / attempt 未知置
 * null，步表与事件缓存为空态（与重挂快照种子同形）。 */
export function seedRunState(summary: ChangeRunSummary): ChangeFlowRunState {
  return {
    runId: summary.runId,
    status: summary.status,
    phase: null,
    attempt: null,
    ask: null,
    confirmPhase: null,
    steps: [],
    finishedReason: null,
    liveEvents: {},
  };
}

/** 终局状态判别：三态终局（completed / stopped / failed）。 */
function isTerminal(status: ChangeRunStatus): boolean {
  return status === 'completed' || status === 'stopped' || status === 'failed';
}

/** RunUpdate 流纯归并：步状态 / SessionEvent 缓存 / ask / 确认等待 / 终态。
 * 终态守卫：run 视图一旦到终局即冻结——后续 update 一律忽略（终态后误达的
 * 迟滞信封不回滚运行史、不改记因）。 */
export function applyRunUpdate(
  state: ChangeFlowRunState | null,
  update: RunUpdate,
): ChangeFlowRunState | null {
  if (state === null) return null;
  if (isTerminal(state.status)) return state;
  if (update.ipc === 'step') {
    return {
      ...state,
      phase: update.step.phase,
      attempt: update.step.attempt,
      steps: [...state.steps, update.step],
    };
  }
  if (update.ipc === 'sessionEvent') {
    return {
      ...state,
      liveEvents: appendEvent(state.liveEvents, update.sessionId, update.event),
    };
  }
  if (update.ipc === 'ask') {
    return {
      ...state,
      status: 'waitingAsk',
      ask: { question: update.question, options: update.options },
    };
  }
  if (update.ipc === 'confirmWait') {
    return {
      ...state,
      status: 'waitingConfirm',
      confirmPhase: update.phase,
      phase: update.phase,
    };
  }
  return {
    ...state,
    status: update.status,
    finishedReason: update.reason,
    ask: null,
    confirmPhase: null,
  };
}

/** 会话事件并入（seq 去重保序：重放与实时并流时同 seq 事件只计一次）。 */
function appendEvent(
  liveEvents: Record<string, AgentEvent[]>,
  sessionId: string,
  event: AgentEvent,
): Record<string, AgentEvent[]> {
  const existing = liveEvents[sessionId] ?? [];
  if (existing.some((seen) => seen.seq === event.seq)) return liveEvents;
  return { ...liveEvents, [sessionId]: [...existing, event] };
}

/** 步词汇 → 节点分组（三类节点可辨）。 */
function groupOf(step: ChangeStepKind): RunStepGroup {
  if (step === 'executor' || step === 'evaluator' || step === 'decision') return 'workerAgent';
  if (step === 'verdictGate' || step === 'retryGate' || step === 'whitelistGate') return 'gate';
  return 'toolStep';
}

/** 步词汇 → role 标签（WorkerAgent 三角色；ToolStep / Gate 为 null）。 */
function roleOf(step: ChangeStepKind): FlowRoleLabel | null {
  if (step === 'executor' || step === 'evaluator' || step === 'decision') return step;
  return null;
}

/** 步状态表 → 图 overlay 运行步节点 */
export function runStepNodes(steps: ChangeStepState[]): RuntimeFlowNode[] {
  const slots = new Map<string, { node: RuntimeFlowNode; open: boolean }[]>();
  const order: string[] = [];
  for (const step of steps) {
    if (PIPELINE_PHASES.indexOf(step.phase) < 0) continue;
    const key = `${step.phase}:${step.attempt}:${step.step}`;
    const list = slots.get(key) ?? [];
    if (list.length === 0) order.push(key);
    if (step.status === 'running') {
      list.push({ node: makeNode(step, list.length), open: true });
    } else {
      const open = [...list].reverse().find((slot) => slot.open);
      if (open) {
        open.node = {
          ...open.node,
          status: step.status,
          sessionId: step.sessionId,
          detail: step.detail,
        };
        open.open = false;
      } else {
        list.push({ node: makeNode(step, list.length), open: false });
      }
    }
    slots.set(key, list);
  }
  const nodes: RuntimeFlowNode[] = [];
  const columnOrder = new Map<string, number>();
  for (const key of order) {
    for (const slot of slots.get(key) ?? []) {
      const counted = columnOrder.get(slot.node.phase) ?? 0;
      slot.node = {
        ...slot.node,
        colIndex: PIPELINE_PHASES.indexOf(slot.node.phase),
        order: counted,
      };
      columnOrder.set(slot.node.phase, counted + 1);
      nodes.push(slot.node);
    }
  }
  return nodes;
}

function makeNode(step: ChangeStepState, seq: number): RuntimeFlowNode {
  const base = `run:${step.phase}:${step.attempt}:${step.step}`;
  return {
    id: seq === 0 ? base : `${base}:${seq}`,
    kind: 'runtime',
    phase: step.phase,
    attempt: step.attempt,
    colIndex: PIPELINE_PHASES.indexOf(step.phase),
    order: 0,
    parentId: `col:${step.phase}`,
    runStepKind: step.step,
    group: groupOf(step.step),
    role: roleOf(step.step),
    status: step.status,
    sessionId: step.sessionId,
    detail: step.detail,
  };
}
