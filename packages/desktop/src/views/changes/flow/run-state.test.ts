import { describe, expect, it } from 'vite-plus/test';

import type {
  AgentEvent,
  ChangeRunSnapshot,
  ChangeStepKind,
  ChangeStepState,
  RunUpdate,
} from '../../../types/dto';
import {
  applyRunUpdate,
  initialRunState,
  runStepNodes,
  type ChangeFlowRunState,
} from './run-state';

// ---------------------------------------------------------------------------
// run-state 纯函数直测（零 react / 零进程边界依赖）：RunUpdate / ChangeRunSnapshot
// fixture 以绑定类型内存构造，纯函数直测。覆盖：快照恢复初值 / Step 与
// SessionEvent 归并（纯函数无副作用）/ ask 与确认等待 / 终态收口 / 乱序与
// 重复 update 容忍 / runStepNodes id 定式与三分类载荷（AC-5 归并与 overlay 半边）。
// ---------------------------------------------------------------------------

const TS = 1727000000000;

/** 宽松默认值构造单条步状态行（RunUpdate::Step 载荷）。 */
function stepRow(overrides: Partial<ChangeStepState> = {}): ChangeStepState {
  return {
    phase: 'implement',
    attempt: 1,
    step: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    ...overrides,
  };
}

/** 宽松默认值构造重挂快照。 */
function snapshot(overrides: Partial<ChangeRunSnapshot> = {}): ChangeRunSnapshot {
  return {
    runId: 'run-1727',
    status: 'running',
    phase: 'implement',
    attempt: 2,
    ask: null,
    ...overrides,
  };
}

/** 以 initialRunState 装配基础运行态（running · implement · attempt 2）。 */
function baseState(): ChangeFlowRunState {
  const state = initialRunState(snapshot());
  if (state === null) throw new Error('fixture 构造失败：initialRunState 不应为 null');
  return state;
}

function textMessage(seq: number, text = '实时片段'): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function stepUpdate(row: ChangeStepState): RunUpdate {
  return { ipc: 'step', step: row };
}

function sessionUpdate(sessionId: string, event: AgentEvent): RunUpdate {
  return { ipc: 'sessionEvent', sessionId, event };
}

describe('initialRunState：重挂快照恢复面（D9）', () => {
  it('null 快照 → null（无运行 run 的空闲态，重挂恢复输入面）', () => {
    expect(initialRunState(null)).toBeNull();
  });

  it('ChangeRunSnapshot → 视图模型逐字段镜像：runId/status/phase/attempt/ask 承接，steps/liveEvents/finishedReason 为空态初值', () => {
    expect(initialRunState(snapshot())).toEqual({
      runId: 'run-1727',
      status: 'running',
      phase: 'implement',
      attempt: 2,
      ask: null,
      confirmPhase: null,
      steps: [],
      finishedReason: null,
      liveEvents: {},
    });
  });

  it('waitingConfirm 快照 → confirmPhase 承接当前相位（停等定位）；running 快照 confirmPhase 为 null', () => {
    const waiting = initialRunState(
      snapshot({ status: 'waitingConfirm', phase: 'dev-design', attempt: 1 }),
    );
    expect(waiting).toMatchObject({
      status: 'waitingConfirm',
      phase: 'dev-design',
      confirmPhase: 'dev-design',
    });

    const running = initialRunState(snapshot({ status: 'running', phase: 'implement' }));
    expect(running?.confirmPhase).toBeNull();
  });

  it('waitingAsk 快照 → ask 载荷逐字段承接、confirmPhase 保持 null', () => {
    const waiting = initialRunState(
      snapshot({
        status: 'waitingAsk',
        phase: 'implement',
        ask: { question: 'backtrack 到哪个相位？', options: ['dev-design', 'test-design'] },
      }),
    );
    expect(waiting).toMatchObject({
      status: 'waitingAsk',
      ask: { question: 'backtrack 到哪个相位？', options: ['dev-design', 'test-design'] },
      confirmPhase: null,
    });
  });
});

describe('applyRunUpdate：Step 归并（纯函数无副作用）', () => {
  it('新步插入与既有步状态推进：phase/attempt 跟随步行、steps 按到站序追加（running → passed/failed 成对保留）', () => {
    let state = applyRunUpdate(baseState(), stepUpdate(stepRow()));
    expect(state).toMatchObject({ phase: 'implement', attempt: 1 });
    expect(state?.steps).toHaveLength(1);

    state = applyRunUpdate(state, stepUpdate(stepRow({ status: 'passed' })));
    state = applyRunUpdate(state, stepUpdate(stepRow({ step: 'staticCheck', sessionId: null })));
    state = applyRunUpdate(
      state,
      stepUpdate(
        stepRow({ step: 'staticCheck', sessionId: null, status: 'failed', detail: '诊断文本' }),
      ),
    );

    expect(state?.steps.map((row) => [row.step, row.status])).toEqual([
      ['executor', 'running'],
      ['executor', 'passed'],
      ['staticCheck', 'running'],
      ['staticCheck', 'failed'],
    ]);
    expect(state?.status).toBe('running');
  });

  it('纯归并无副作用：入参 state 不被突变、返回新对象（纯函数无副作用——AC-5 归并半边）', () => {
    const state = baseState();
    state.steps.push(stepRow());
    const before = JSON.stringify(state);

    const next = applyRunUpdate(state, stepUpdate(stepRow({ status: 'passed' })));

    expect(JSON.stringify(state)).toBe(before);
    expect(next).not.toBe(state);
    expect(next?.steps).not.toBe(state.steps);
    expect(next?.steps).toHaveLength(2);
  });

  it('state 为 null（未发起运行）时任意 update 恒为 null', () => {
    expect(applyRunUpdate(null, stepUpdate(stepRow()))).toBeNull();
    expect(applyRunUpdate(null, { ipc: 'finished', status: 'completed', reason: null })).toBeNull();
  });
});

describe('applyRunUpdate：SessionEvent 缓存（节点转录联动 liveEvents 输入面）', () => {
  it('按会话分桶缓存：不同 sessionId 各自追加、互不串桶', () => {
    let state = applyRunUpdate(baseState(), sessionUpdate('ses-exec-1', textMessage(0, 'A0')));
    state = applyRunUpdate(state, sessionUpdate('ses-eval-1', textMessage(0, 'B0')));
    state = applyRunUpdate(state, sessionUpdate('ses-exec-1', textMessage(1, 'A1')));

    expect(state?.liveEvents['ses-exec-1']?.map((event) => event.seq)).toEqual([0, 1]);
    expect(state?.liveEvents['ses-eval-1']?.map((event) => event.seq)).toEqual([0]);
  });

  it('同会话 seq 去重：重复 seq 不重复并入、既有缓存数组不被突变（空缓存起步首事件建桶）', () => {
    const fresh = baseState();
    const first = applyRunUpdate(fresh, sessionUpdate('ses-exec-1', textMessage(0, 'A0')));
    const second = applyRunUpdate(first, sessionUpdate('ses-exec-1', textMessage(1, 'A1')));
    const duplicated = applyRunUpdate(
      second,
      sessionUpdate('ses-exec-1', textMessage(1, 'A1 重复')),
    );

    expect(duplicated?.liveEvents['ses-exec-1']).toHaveLength(2);
    expect(duplicated?.liveEvents['ses-exec-1']?.map((event) => event.seq)).toEqual([0, 1]);
    // 纯归并：中间态与初态的缓存均不被突变
    expect(first?.liveEvents['ses-exec-1']).toHaveLength(1);
    expect(fresh.liveEvents['ses-exec-1']).toBeUndefined();
  });

  it('事件载荷保真：kind/blocks 等字段原样入缓存（转录联动直接消费）', () => {
    const state = applyRunUpdate(
      baseState(),
      sessionUpdate('ses-exec-1', textMessage(3, '载荷保真')),
    );
    expect(state?.liveEvents['ses-exec-1']?.[0]).toEqual(textMessage(3, '载荷保真'));
  });
});

describe('applyRunUpdate：停等与终态收口（控制面板卡片输入面）', () => {
  it('ask：status → waitingAsk、question/options 承接（ask 卡片输入面）', () => {
    const state = applyRunUpdate(baseState(), {
      ipc: 'ask',
      question: '选择哪个方向？',
      options: ['A', 'B'],
    });
    expect(state).toMatchObject({
      status: 'waitingAsk',
      ask: { question: '选择哪个方向？', options: ['A', 'B'] },
    });
  });

  it('confirmWait：status → waitingConfirm、confirmPhase 与 phase 同步指向停等相位（phase 间拍板点）', () => {
    const state = applyRunUpdate(baseState(), { ipc: 'confirmWait', phase: 'test-design' });
    expect(state).toMatchObject({
      status: 'waitingConfirm',
      confirmPhase: 'test-design',
      phase: 'test-design',
    });
  });

  it('finished：status/reason 收口、ask 与 confirmPhase 清空；步状态表与事件缓存保留（终态不回滚运行史）', () => {
    const progressed = applyRunUpdate(
      applyRunUpdate(baseState(), stepUpdate(stepRow())),
      sessionUpdate('ses-exec-1', textMessage(0)),
    );
    const finished = applyRunUpdate(progressed, {
      ipc: 'finished',
      status: 'failed',
      reason: 'CLI 漂移',
    });

    expect(finished).toMatchObject({
      status: 'failed',
      finishedReason: 'CLI 漂移',
      ask: null,
      confirmPhase: null,
    });
    expect(finished?.steps).toHaveLength(1);
    expect(finished?.liveEvents['ses-exec-1']).toHaveLength(1);
  });

  it('finished 同载荷重复归并幂等：状态与记因不再变化（收口幂等形态）', () => {
    const once = applyRunUpdate(baseState(), {
      ipc: 'finished',
      status: 'completed',
      reason: null,
    });
    const twice = applyRunUpdate(once, { ipc: 'finished', status: 'completed', reason: null });
    expect(twice).toEqual(once);
    expect(twice).toMatchObject({ status: 'completed', finishedReason: null });
  });

  it('终态守卫：completed/stopped/failed 收口后，step / sessionEvent / ask / confirmWait 迟滞信封一律忽略（状态冻结，步表与事件缓存不回滚）', () => {
    const base = applyRunUpdate(
      applyRunUpdate(baseState(), stepUpdate(stepRow())),
      sessionUpdate('ses-exec-1', textMessage(0)),
    );
    for (const status of ['completed', 'stopped', 'failed'] as const) {
      const terminal = applyRunUpdate(base, {
        ipc: 'finished',
        status,
        reason: `收口 ${status}`,
      });

      const frozen = applyRunUpdate(
        applyRunUpdate(
          applyRunUpdate(
            applyRunUpdate(terminal, stepUpdate(stepRow({ status: 'passed', attempt: 2 }))),
            sessionUpdate('ses-exec-1', textMessage(1, '迟滞事件')),
          ),
          { ipc: 'ask', question: '迟滞提问？', options: ['A'] },
        ),
        { ipc: 'confirmWait', phase: 'test-gen' },
      );

      expect(frozen).toBe(terminal);
      expect(frozen).toEqual(terminal);
      expect(frozen).toMatchObject({ status, finishedReason: `收口 ${status}` });
      expect(frozen?.steps).toHaveLength(1);
      expect(frozen?.liveEvents['ses-exec-1']).toHaveLength(1);
      expect(frozen?.ask).toBeNull();
      expect(frozen?.confirmPhase).toBeNull();
    }
  });
});

describe('applyRunUpdate / runStepNodes：乱序与重复 update 容忍（Channel 流）', () => {
  it('同键 running→passed 成对到达：steps 保留两行、runStepNodes 归并单节点（重复状态行不产生重复节点）', () => {
    const state = applyRunUpdate(
      applyRunUpdate(baseState(), stepUpdate(stepRow())),
      stepUpdate(stepRow({ status: 'passed', detail: '执行收口' })),
    );
    expect(state?.steps).toHaveLength(2);

    const nodes = runStepNodes(state!);
    expect(nodes).toHaveLength(1);
    expect(nodes[0]).toMatchObject({
      id: 'run:implement:1:executor',
      status: 'passed',
      sessionId: 'ses-exec-1',
      detail: '执行收口',
    });
  });

  it('终态先行（乱序到站）：passed 先于 running 不崩、节点 id 仍唯一可辨', () => {
    const state = applyRunUpdate(
      applyRunUpdate(baseState(), stepUpdate(stepRow({ status: 'passed' }))),
      stepUpdate(stepRow({ status: 'running' })),
    );
    const nodes = runStepNodes(state!);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:implement:1:executor',
      'run:implement:1:executor:1',
    ]);
    expect(new Set(nodes.map((node) => node.id)).size).toBe(nodes.length);
  });

  it('同键终态行重复到站：追加为独立闭节点（seq 后缀防 id 撞车）、不崩', () => {
    let state = applyRunUpdate(baseState(), stepUpdate(stepRow()));
    state = applyRunUpdate(state, stepUpdate(stepRow({ status: 'passed' })));
    state = applyRunUpdate(state, stepUpdate(stepRow({ status: 'passed' })));

    const nodes = runStepNodes(state!);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:implement:1:executor',
      'run:implement:1:executor:1',
    ]);
    expect(nodes.map((node) => node.status)).toEqual(['passed', 'passed']);
  });
});

describe('runStepNodes：id 定式与载荷承接（overlay 输入面）', () => {
  it('id 定式逐字：run:<phase>:<attempt>:<step>；同键第二次迭代（static-check 反馈边）追加 :<seq> 后缀', () => {
    let state = applyRunUpdate(baseState(), stepUpdate(stepRow()));
    state = applyRunUpdate(state, stepUpdate(stepRow({ step: 'staticCheck', sessionId: null })));
    state = applyRunUpdate(
      state,
      stepUpdate(stepRow({ step: 'staticCheck', sessionId: null, status: 'passed' })),
    );
    state = applyRunUpdate(
      state,
      stepUpdate(stepRow({ step: 'staticCheck', sessionId: null, status: 'running' })),
    );

    const nodes = runStepNodes(state!);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:implement:1:executor',
      'run:implement:1:staticCheck',
      'run:implement:1:staticCheck:1',
    ]);
  });

  it('载荷承接：runStepKind/group/role/status/sessionId/detail 六面 + kind/colIndex/parentId 布局面逐字段对应', () => {
    let state = applyRunUpdate(baseState(), stepUpdate(stepRow()));
    state = applyRunUpdate(
      state,
      stepUpdate(
        stepRow({
          step: 'phaseLog',
          sessionId: null,
          status: 'passed',
          detail: 'attempt 1 已落账',
        }),
      ),
    );

    const nodes = runStepNodes(state!);
    expect(nodes[0]).toEqual({
      id: 'run:implement:1:executor',
      kind: 'runtime',
      phase: 'implement',
      attempt: 1,
      colIndex: 3,
      order: 0,
      parentId: 'col:implement',
      runStepKind: 'executor',
      group: 'workerAgent',
      role: 'executor',
      status: 'running',
      sessionId: 'ses-exec-1',
      detail: null,
    });
    expect(nodes[1]).toMatchObject({
      id: 'run:implement:1:phaseLog',
      kind: 'runtime',
      runStepKind: 'phaseLog',
      group: 'toolStep',
      role: null,
      status: 'passed',
      sessionId: null,
      detail: 'attempt 1 已落账',
    });
  });

  it('三类可辨：九步词汇 → WorkerAgent / ToolStep / Gate 分组与 role 标签逐词对应', () => {
    const GROUP_OF: Record<
      ChangeStepKind,
      {
        group: 'workerAgent' | 'toolStep' | 'gate';
        role: 'executor' | 'evaluator' | 'decision' | null;
      }
    > = {
      executor: { group: 'workerAgent', role: 'executor' },
      evaluator: { group: 'workerAgent', role: 'evaluator' },
      decision: { group: 'workerAgent', role: 'decision' },
      phaseStart: { group: 'toolStep', role: null },
      staticCheck: { group: 'toolStep', role: null },
      phaseLog: { group: 'toolStep', role: null },
      verdictGate: { group: 'gate', role: null },
      retryGate: { group: 'gate', role: null },
      whitelistGate: { group: 'gate', role: null },
    };

    let state: ChangeFlowRunState | null = baseState();
    (Object.keys(GROUP_OF) as ChangeStepKind[]).forEach((kind, index) => {
      state = applyRunUpdate(state, stepUpdate(stepRow({ step: kind, attempt: index })));
    });

    const nodes = runStepNodes(state);
    expect(nodes).toHaveLength(9);
    for (const node of nodes) {
      const expected = GROUP_OF[node.runStepKind];
      expect(node.group).toBe(expected.group);
      expect(node.role).toBe(expected.role);
    }
  });

  it('缺号相位跳过：phase 不在 9 站内的步不入图（布局恒定）', () => {
    let state = applyRunUpdate(baseState(), stepUpdate(stepRow({ phase: 'bootstrap' })));
    state = applyRunUpdate(state, stepUpdate(stepRow()));
    expect(runStepNodes(state!).map((node) => node.id)).toEqual(['run:implement:1:executor']);
  });

  it('列内归并序：同列多节点按归并序 0 起编号（order 面）', () => {
    let state = applyRunUpdate(baseState(), stepUpdate(stepRow()));
    state = applyRunUpdate(state, stepUpdate(stepRow({ step: 'phaseStart', sessionId: null })));
    state = applyRunUpdate(state, stepUpdate(stepRow({ step: 'verdictGate', sessionId: null })));

    const nodes = runStepNodes(state!);
    expect(nodes.every((node) => node.colIndex === 3 && node.parentId === 'col:implement')).toBe(
      true,
    );
    expect(nodes.map((node) => node.order)).toEqual([0, 1, 2]);
  });

  it('空步表 → 空数组（overlay 缺省输出与现状一致的输入半边）', () => {
    expect(runStepNodes(baseState())).toEqual([]);
  });
});

describe('run-state：空缓存与缺省字段（归并/推导不崩）', () => {
  it('无 SessionEvent 缓存（liveEvents 空对象）+ detail 缺省（null）+ role null 形态：归并与节点推导不崩、载荷为 null', () => {
    const state = applyRunUpdate(
      baseState(),
      stepUpdate(stepRow({ step: 'whitelistGate', sessionId: null, detail: null })),
    );
    expect(state?.liveEvents).toEqual({});

    const nodes = runStepNodes(state!);
    expect(nodes[0]).toMatchObject({
      id: 'run:implement:1:whitelistGate',
      group: 'gate',
      role: null,
      sessionId: null,
      detail: null,
    });
  });
});
