/**
 * run 步节点转换层：统一视图（detail.runs 库读史 ∪ activeRun 活面）→ 图
 * overlay 运行步节点。零 react 依赖直测；**库读史常驻**（unify-run-state-
 * persistence 翻案「不持久化 / 图回落」——步节点自 runs[].steps 派生收口后
 * 常驻上图，与运行中实时面走同一转换函数；状态机镜像 / liveEvents / seq 去
 * 重随通知降位解散，见 flow-view MODIFIED 条款）。
 */
import type {
  ChangeRunEntry,
  ChangeRunStepRecord,
  ChangeStepKind,
  ChangeStepState,
  RunStepKind,
} from '../../../types/dto';
import { PIPELINE_PHASES } from './layout';
import type { FlowRoleLabel, RunStepGroup, RuntimeFlowNode } from './types';

/** 步词汇归一单点：库读史 snake 词（RunStepKind）→ 活面 camel 词
 *（ChangeStepKind）——`static_check`→`staticCheck` /
 * `test_execution`→`testExecution`，其余三词两域同形。 */
function normalizeKind(step: RunStepKind): ChangeStepKind {
  if (step === 'static_check') return 'staticCheck';
  if (step === 'test_execution') return 'testExecution';
  return step;
}

/** 库读史步行 → 活面步状态行（词汇归一收本文件单点）。 */
function stepFromRecord(record: ChangeRunStepRecord): ChangeStepState {
  return {
    phase: record.phase,
    attempt: record.attempt,
    step: normalizeKind(record.step),
    status: record.status,
    sessionId: record.sessionId,
    detail: record.detail,
  };
}

/** 统一视图步源合成（同一转换函数的输入面）：runs[].steps（5 类库读史，
 * runs 序 = startedAt 序即稳定分层序）∪ liveSteps（在飞 run 活步，全词汇
 * emit 序）——收口后活步褪去、库读史常驻。 */
export function unifiedRunSteps(
  runs: ChangeRunEntry[],
  liveSteps: ChangeStepState[],
): ChangeStepState[] {
  const persisted = runs.flatMap((run) => run.steps.map(stepFromRecord));
  return [...persisted, ...liveSteps];
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

/** 步状态表 → 图 overlay 运行步节点（槽位配对机制保留：同键 running→终态
 * 配对、重号 `:seq` 后缀堆叠——attempt 复用撞键例外由槽位机制吸收，见
 * design 边界留痕节）。 */
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
