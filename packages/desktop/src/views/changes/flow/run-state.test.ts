import { describe, expect, it } from 'vite-plus/test';

import type {
  ChangeRunEntry,
  ChangeRunStepRecord,
  ChangeStepKind,
  ChangeStepState,
  RunStepKind,
} from '../../../types/dto';
import { runStepNodes, unifiedRunSteps } from './run-state';

// ---------------------------------------------------------------------------
// run-state 纯函数直测（零 react / 零进程边界依赖）：统一视图 fixture 以绑定
// 类型内存构造，纯函数直测。覆盖：unifiedRunSteps 双源合成（库读史展开 + 活
// 步并入 + 步词汇归一单点）/ runStepNodes 槽位配对与三分类载荷（图常驻渲染
// 的派生面——状态机镜像 / liveEvents / seq 去重随通知降位解散）。
// ---------------------------------------------------------------------------

/** 宽松默认值构造单条活面步状态行。 */
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

/** 宽松默认值构造库读史步行（ChangeRunStepRecord，五词汇 snake 词）。 */
function runStepRow(overrides: Partial<ChangeRunStepRecord> = {}): ChangeRunStepRecord {
  return {
    seq: 0,
    phase: 'implement',
    attempt: 1,
    step: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    ...overrides,
  };
}

/** 宽松默认值构造单条 run 运行史条目。 */
function runEntry(overrides: Partial<ChangeRunEntry> = {}): ChangeRunEntry {
  return {
    runId: 'run-1727',
    status: 'completed',
    reason: null,
    startedAt: '2026-10-01T08:00:00.000Z',
    finishedAt: '2026-10-01T08:04:00.000Z',
    steps: [],
    ...overrides,
  };
}

describe('unifiedRunSteps：双源合成（库读史 ∪ 活面）', () => {
  it('runs[].steps 展开：5 类库读史步全量出线（seq 升序保持 emit 序），run 归属分层经 runs 序稳定', () => {
    const runs = [
      runEntry({
        runId: 'run-1',
        steps: [
          runStepRow({ seq: 0, step: 'executor' }),
          runStepRow({ seq: 1, step: 'static_check', sessionId: null }),
          runStepRow({ seq: 2, step: 'evaluator', sessionId: 'ses-eval-1' }),
        ],
      }),
    ];
    expect(unifiedRunSteps(runs, [])).toEqual([
      stepRow({ step: 'executor' }),
      stepRow({ step: 'staticCheck', sessionId: null }),
      stepRow({ step: 'evaluator', sessionId: 'ses-eval-1' }),
    ]);
  });

  it('步词汇归一单点：static_check→staticCheck / test_execution→testExecution（snake 词不入活面词汇）', () => {
    const runs = [
      runEntry({
        steps: [
          runStepRow({ seq: 0, step: 'static_check', sessionId: null }),
          runStepRow({ seq: 1, step: 'test_execution', sessionId: null }),
        ],
      }),
    ];
    expect(unifiedRunSteps(runs, []).map((row) => row.step)).toEqual([
      'staticCheck',
      'testExecution',
    ]);
  });

  it('全史叠加：多 run 步节点全保留（runs 序 = startedAt 序即稳定分层序），活步并列入飞活面', () => {
    const runs = [
      runEntry({
        runId: 'run-1',
        startedAt: '2026-10-01T08:00:00.000Z',
        steps: [runStepRow({ seq: 0, attempt: 1 })],
      }),
      runEntry({
        runId: 'run-2',
        startedAt: '2026-10-01T09:00:00.000Z',
        steps: [runStepRow({ seq: 0, attempt: 2, status: 'passed' })],
      }),
    ];
    const steps = unifiedRunSteps(runs, [stepRow({ attempt: 3, status: 'passed' })]);
    expect(steps.map((row) => row.attempt)).toEqual([1, 2, 3]);
    expect(steps[2]).toMatchObject({ attempt: 3, status: 'passed' });
  });

  it('空 runs / 空 liveSteps → 空数组（零 run 收口后无史形态）；仅 live（首 run 运行中零库史）→ 活步全量出', () => {
    expect(unifiedRunSteps([], [])).toEqual([]);
    // 首 run 运行中（库面无史）→ 活面步表全量出、零库读史分量
    expect(
      unifiedRunSteps(
        [],
        [stepRow({ step: 'executor' }), stepRow({ step: 'staticCheck', attempt: 2 })],
      ),
    ).toEqual([stepRow({ step: 'executor' }), stepRow({ step: 'staticCheck', attempt: 2 })]);
  });
});

describe('runStepNodes：槽位配对与 id 定式（overlay 输入面）', () => {
  it('同键 running→passed 成对到达：两行归并单节点（重复状态行不产生重复节点）', () => {
    const steps = [stepRow(), stepRow({ status: 'passed', detail: '执行收口' })];
    expect(steps).toHaveLength(2);

    const nodes = runStepNodes(steps);
    expect(nodes).toHaveLength(1);
    expect(nodes[0]).toMatchObject({
      id: 'run:implement:1:executor',
      status: 'passed',
      sessionId: 'ses-exec-1',
      detail: '执行收口',
    });
  });

  it('终态先行（乱序到站）：passed 先于 running 不崩、节点 id 仍唯一可辨', () => {
    const steps = [stepRow({ status: 'passed' }), stepRow({ status: 'running' })];
    const nodes = runStepNodes(steps);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:implement:1:executor',
      'run:implement:1:executor:1',
    ]);
    expect(new Set(nodes.map((node) => node.id)).size).toBe(nodes.length);
  });

  it('同键终态行重复到站：追加为独立闭节点（seq 后缀防 id 撞车——attempt 复用撞键例外由槽位机制吸收）、不崩', () => {
    const steps = [stepRow(), stepRow({ status: 'passed' }), stepRow({ status: 'passed' })];

    const nodes = runStepNodes(steps);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:implement:1:executor',
      'run:implement:1:executor:1',
    ]);
    expect(nodes.map((node) => node.status)).toEqual(['passed', 'passed']);
  });

  it('id 定式逐字：run:<phase>:<attempt>:<step>；同键第二次迭代（static-check 反馈边）追加 :<seq> 后缀', () => {
    const steps = [
      stepRow(),
      stepRow({ step: 'staticCheck', sessionId: null }),
      stepRow({ step: 'staticCheck', sessionId: null, status: 'passed' }),
      stepRow({ step: 'staticCheck', sessionId: null, status: 'running' }),
    ];

    const nodes = runStepNodes(steps);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:implement:1:executor',
      'run:implement:1:staticCheck',
      'run:implement:1:staticCheck:1',
    ]);
  });

  it('载荷承接：runStepKind/group/role/status/sessionId/detail 六面 + kind/colIndex/parentId 布局面逐字段对应', () => {
    const steps = [
      stepRow(),
      stepRow({
        step: 'phaseLog',
        sessionId: null,
        status: 'passed',
        detail: 'attempt 1 已落账',
      }),
    ];

    const nodes = runStepNodes(steps);
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

  it('三类可辨：十步词汇 → WorkerAgent / ToolStep / Gate 分组与 role 标签逐词对应', () => {
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
      testExecution: { group: 'toolStep', role: null },
      phaseLog: { group: 'toolStep', role: null },
      verdictGate: { group: 'gate', role: null },
      retryGate: { group: 'gate', role: null },
      whitelistGate: { group: 'gate', role: null },
    };

    const steps = (Object.keys(GROUP_OF) as ChangeStepKind[]).map((kind, index) =>
      stepRow({ step: kind, attempt: index }),
    );

    const nodes = runStepNodes(steps);
    expect(nodes).toHaveLength(10);
    for (const node of nodes) {
      const expected = GROUP_OF[node.runStepKind];
      expect(node.group).toBe(expected.group);
      expect(node.role).toBe(expected.role);
    }
  });

  it('缺号相位跳过：phase 不在 9 站内的步不入图（布局恒定）', () => {
    const steps = [stepRow({ phase: 'bootstrap' }), stepRow()];
    expect(runStepNodes(steps).map((node) => node.id)).toEqual(['run:implement:1:executor']);
  });

  it('列内归并序：同列多节点按归并序 0 起编号（order 面）', () => {
    const steps = [
      stepRow(),
      stepRow({ step: 'phaseStart', sessionId: null }),
      stepRow({ step: 'verdictGate', sessionId: null }),
    ];

    const nodes = runStepNodes(steps);
    expect(nodes.every((node) => node.colIndex === 3 && node.parentId === 'col:implement')).toBe(
      true,
    );
    expect(nodes.map((node) => node.order)).toEqual([0, 1, 2]);
  });

  it('空步表 → 空数组（overlay 缺省输出）', () => {
    expect(runStepNodes([])).toEqual([]);
  });
});

describe('run-state：缺省字段（推导不崩）', () => {
  it('detail 缺省（null）+ role null 形态：节点推导不崩、载荷为 null', () => {
    const steps = [stepRow({ step: 'whitelistGate', sessionId: null, detail: null })];
    const nodes = runStepNodes(steps);
    expect(nodes[0]).toMatchObject({
      id: 'run:implement:1:whitelistGate',
      group: 'gate',
      role: null,
      sessionId: null,
      detail: null,
    });
  });
});

// RunStepKind 词汇 import 面锚（五词汇封闭集直测归一单点的输入域）
const PERSISTED_WORDS: RunStepKind[] = [
  'executor',
  'evaluator',
  'decision',
  'static_check',
  'test_execution',
];
describe('unifiedRunSteps：词汇封闭集', () => {
  it('五词汇 snake 词全量归一可达（词汇漂移即断）', () => {
    const steps = PERSISTED_WORDS.map((step, index) =>
      runStepRow({ seq: index, step, sessionId: null }),
    );
    const unified = unifiedRunSteps([runEntry({ steps })], []);
    expect(unified.map((row) => row.step)).toEqual([
      'executor',
      'evaluator',
      'decision',
      'staticCheck',
      'testExecution',
    ]);
  });
});
