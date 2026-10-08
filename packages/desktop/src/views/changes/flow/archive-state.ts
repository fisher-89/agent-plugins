/**
 * 归档链状态纯 reducer：`ArchiveUpdate` 流 → 前端归档视图模型
 * （ArchiveFlowState）的纯归并（`run-state.ts` 同构）。零 react 依赖直测；
 * 不持久化——收口后释放订阅，终态由 db / 磁盘事实承载（详情页 refresh 回落
 * 已归档形态、按钮消失）。
 */
import type {
  AgentEvent,
  ArchiveSnapshot,
  ArchiveStage,
  ArchiveStageState,
  ArchiveSummary,
  ArchiveUpdate,
} from '../../../types/dto';

/** 阶段清单呈现序（六段固定行；`data-testid="archive-stage-<stage>"` 挂钩）。 */
export const ARCHIVE_STAGES: ArchiveStage[] = [
  'preflight',
  'specSync',
  'commit',
  'merge',
  'seal',
  'finalize',
];

/** 前端归档视图模型：阶段状态表（同段后写覆盖单槽终值）+ 最近会话事件缓存
 * + 终态面（summary / error 互斥）。 */
export interface ArchiveFlowState {
  /** 阶段状态表（未到达段缺席；同段 running → 终态后写覆盖） */
  stages: Partial<Record<ArchiveStage, ArchiveStageState>>;
  /** 当前归档 agent 会话 id（转录入口寻址；null = 无会话段 / 未发起） */
  sessionId: string | null;
  /** 终态摘要（成功收口；失败停止为 null） */
  summary: ArchiveSummary | null;
  /** 终态错误（失败 / 停止；成功为 null） */
  error: string | null;
  /** 终局判别（Finished 后冻结——迟滞信封一律忽略） */
  finished: boolean;
  /** 最近会话事件缓存（key = sessionId，seq 去重保序） */
  liveEvents: Record<string, AgentEvent[]>;
}

/** 重挂快照 → 归档视图模型初值；无在案链（null）即 null。 */
export function seedArchiveState(snapshot: ArchiveSnapshot | null): ArchiveFlowState | null {
  if (snapshot === null) return null;
  const stages: Partial<Record<ArchiveStage, ArchiveStageState>> = {};
  for (const stage of snapshot.stages) stages[stage.stage] = stage;
  return {
    stages,
    sessionId: snapshot.sessionId,
    summary: null,
    error: null,
    finished: false,
    liveEvents: {},
  };
}

/** 发起种子（发起路径唯一状态种子：空阶段表起步，后续 update 归并自此起步）。 */
export function initialArchiveState(): ArchiveFlowState {
  return {
    stages: {},
    sessionId: null,
    summary: null,
    error: null,
    finished: false,
    liveEvents: {},
  };
}

/** `ArchiveUpdate` 流纯归并：Stage 同段覆盖 / SessionEvent 按会话缓存 /
 * Finished 收 summary 或 error 并冻结（终态守卫——迟滞信封不回滚运行史）。 */
export function applyArchiveUpdate(
  state: ArchiveFlowState | null,
  update: ArchiveUpdate,
): ArchiveFlowState | null {
  if (state === null) return null;
  if (state.finished) return state;
  if (update.ipc === 'stage') {
    return {
      ...state,
      stages: { ...state.stages, [update.stage.stage]: update.stage },
    };
  }
  if (update.ipc === 'sessionEvent') {
    return {
      ...state,
      sessionId: update.sessionId,
      liveEvents: appendEvent(state.liveEvents, update.sessionId, update.event),
    };
  }
  return {
    ...state,
    summary: update.summary,
    error: update.error,
    finished: true,
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
