import { describe, expect, it } from 'vite-plus/test';

import type {
  AgentEvent,
  ArchiveSnapshot,
  ArchiveStage,
  ArchiveStageState,
  ArchiveStageStatus,
  ArchiveSummary,
  ArchiveUpdate,
} from '../../../types/dto';
import {
  applyArchiveUpdate,
  ARCHIVE_STAGES,
  initialArchiveState,
  seedArchiveState,
  type ArchiveFlowState,
} from './archive-state';

// ---------------------------------------------------------------------------
// archive-state 纯函数直测（零 react / 零进程边界依赖）：ArchiveUpdate fixture
// 以绑定类型内存构造，纯 reducer 直测。覆盖：Stage 同段后写覆盖与六段互不覆盖 /
// SessionEvent 按会话分槽 seq 去重 / Finished 两态与终态冻结（迟滞信封忽略）/
// 快照种子恢复（AC-7 摘要归并半边 + D5 DTO 消费面）。
// ---------------------------------------------------------------------------

const TS = 1727000000000;

/** 宽松默认值构造单条阶段状态行（ArchiveUpdate::Stage 载荷）。 */
function stageRow(
  overrides: Partial<ArchiveStageState> & { stage: ArchiveStage },
): ArchiveStageState {
  return {
    status: 'running',
    detail: null,
    ...overrides,
  };
}

function stageUpdate(row: ArchiveStageState): ArchiveUpdate {
  return { ipc: 'stage', stage: row };
}

function textMessage(seq: number, text = '同步片段'): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function sessionUpdate(sessionId: string, event: AgentEvent): ArchiveUpdate {
  return { ipc: 'sessionEvent', sessionId, event };
}

/** 成功收口 summary fixture。 */
function summary(overrides: Partial<ArchiveSummary> = {}): ArchiveSummary {
  return {
    name: 'archive-demo',
    archivedDir: '2026-10-08-archive-demo',
    specs: 'synced',
    warnings: [],
    ...overrides,
  };
}

/** 以 initialArchiveState 装配基础运行态（空阶段表起步——发起路径唯一种子）。 */
function baseState(): ArchiveFlowState {
  return initialArchiveState();
}

/** 依序归并多条 update（reducer 折叠）。 */
function applyAll(
  state: ArchiveFlowState | null,
  updates: ArchiveUpdate[],
): ArchiveFlowState | null {
  return updates.reduce((current, update) => applyArchiveUpdate(current, update), state);
}

describe('ARCHIVE_STAGES：阶段清单呈现序（六段固定行）', () => {
  it('六段固定行序：preflight / specSync / commit / merge / seal / finalize', () => {
    expect(ARCHIVE_STAGES).toEqual([
      'preflight',
      'specSync',
      'commit',
      'merge',
      'seal',
      'finalize',
    ]);
  });
});

describe('applyArchiveUpdate：Stage 归并（同段后写覆盖）', () => {
  it('同段 running 后随 passed → 后写覆盖（同段单槽终值）', () => {
    const state = applyAll(baseState(), [
      stageUpdate(stageRow({ stage: 'commit', status: 'running' })),
      stageUpdate(stageRow({ stage: 'commit', status: 'passed' })),
    ]);
    expect(Object.keys(state?.stages ?? {})).toEqual(['commit']);
    expect(state?.stages.commit).toMatchObject({ stage: 'commit', status: 'passed' });
  });

  it('同段 running 后随 skipped（携 detail）→ 单槽终值保留跳过因', () => {
    const state = applyAll(baseState(), [
      stageUpdate(stageRow({ stage: 'specSync', status: 'running' })),
      stageUpdate(stageRow({ stage: 'specSync', status: 'skipped', detail: '无 delta specs' })),
    ]);
    expect(state?.stages.specSync).toEqual({
      stage: 'specSync',
      status: 'skipped',
      detail: '无 delta specs',
    });
  });

  it('同段 running 后随 failed → 单槽终值 failed 携记因', () => {
    const state = applyAll(baseState(), [
      stageUpdate(stageRow({ stage: 'merge', status: 'running' })),
      stageUpdate(stageRow({ stage: 'merge', status: 'failed', detail: 'git merge 冲突' })),
    ]);
    expect(state?.stages.merge).toMatchObject({ status: 'failed', detail: 'git merge 冲突' });
  });

  it('六段互不覆盖：逐段推进各自单槽，阶段表按到达序累积', () => {
    const updates: ArchiveUpdate[] = ARCHIVE_STAGES.flatMap((stage): ArchiveUpdate[] => [
      stageUpdate(stageRow({ stage, status: 'running' })),
      stageUpdate(stageRow({ stage, status: 'passed' })),
    ]);
    const state = applyAll(baseState(), updates);
    expect(Object.keys(state?.stages ?? {})).toHaveLength(6);
    for (const stage of ARCHIVE_STAGES) {
      expect(state?.stages[stage]?.status).toBe<ArchiveStageStatus>('passed');
    }
  });
});

describe('applyArchiveUpdate：SessionEvent 缓存（按会话分槽 seq 去重）', () => {
  it('按会话 id 分槽缓存：两会话事件各归各槽', () => {
    const state = applyAll(baseState(), [
      sessionUpdate('sess-a', textMessage(0, 'A1')),
      sessionUpdate('sess-b', textMessage(1, 'B1')),
      sessionUpdate('sess-a', textMessage(2, 'A2')),
    ]);
    expect(state?.liveEvents['sess-a']).toHaveLength(2);
    expect(state?.liveEvents['sess-b']).toHaveLength(1);
    expect(state?.sessionId).toBe('sess-a');
  });

  it('同 seq 事件只计一次（去重保序——live 与重放并流前提）', () => {
    const duplicate = textMessage(7, '同 seq 重投');
    const state = applyAll(baseState(), [
      sessionUpdate('sess-a', duplicate),
      sessionUpdate('sess-a', textMessage(3, '先到后 seq')),
      sessionUpdate('sess-a', duplicate),
    ]);
    const events = state?.liveEvents['sess-a'] ?? [];
    // 同 seq 重复投递只计一次；到达序保序（排序交给转录装配）
    expect(events).toHaveLength(2);
    expect(events.map((event) => event.seq)).toEqual([7, 3]);
  });
});

describe('applyArchiveUpdate：Finished 两态与终态冻结', () => {
  it('Finished { summary } → summary 落态、error 保持 null、finished 置位', () => {
    const state = applyArchiveUpdate(baseState(), {
      ipc: 'finished',
      summary: summary(),
      error: null,
    });
    expect(state?.summary).toEqual(summary());
    expect(state?.error).toBeNull();
    expect(state?.finished).toBe(true);
  });

  it('Finished { error } → error 落态、summary 保持 null', () => {
    const state = applyArchiveUpdate(baseState(), {
      ipc: 'finished',
      summary: null,
      error: 'git merge 冲突（CONFLICT）',
    });
    expect(state?.summary).toBeNull();
    expect(state?.error).toBe('git merge 冲突（CONFLICT）');
    expect(state?.finished).toBe(true);
  });

  it('终态后迟滞信封（Stage / SessionEvent）一律忽略（不回滚运行史）', () => {
    const finished = applyArchiveUpdate(baseState(), {
      ipc: 'finished',
      summary: summary(),
      error: null,
    });
    const after = applyAll(finished, [
      stageUpdate(stageRow({ stage: 'finalize', status: 'failed', detail: '迟滞失败' })),
      sessionUpdate('sess-late', textMessage(99, '迟滞事件')),
      { ipc: 'finished', summary: null, error: '迟滞终态' },
    ]);
    expect(after).toBe(finished);
    expect(after?.error).toBeNull();
    expect(after?.stages.finalize).toBeUndefined();
    expect(after?.liveEvents['sess-late']).toBeUndefined();
  });

  it('null 态入参恒 null（未发起链的归并零副作用）', () => {
    expect(applyArchiveUpdate(null, stageUpdate(stageRow({ stage: 'seal' })))).toBeNull();
  });
});

describe('seedArchiveState：快照种子（重挂恢复面）', () => {
  it('snapshot null → null（无在案链的空闲态）', () => {
    expect(seedArchiveState(null)).toBeNull();
  });

  it('Some 快照 → 初值 stages 逐段一致 + sessionId 透传 + summary / error 空态', () => {
    const snapshot: ArchiveSnapshot = {
      stages: [
        stageRow({ stage: 'preflight', status: 'passed' }),
        stageRow({ stage: 'specSync', status: 'running' }),
      ],
      sessionId: 'sess-restore',
    };
    expect(seedArchiveState(snapshot)).toEqual({
      stages: {
        preflight: snapshot.stages[0],
        specSync: snapshot.stages[1],
      },
      sessionId: 'sess-restore',
      summary: null,
      error: null,
      finished: false,
      liveEvents: {},
    } satisfies ArchiveFlowState);
  });
});

describe('initialArchiveState：发起种子', () => {
  it('空阶段表 + 空会话 + 双空终态起步', () => {
    expect(initialArchiveState()).toEqual({
      stages: {},
      sessionId: null,
      summary: null,
      error: null,
      finished: false,
      liveEvents: {},
    } satisfies ArchiveFlowState);
  });
});
