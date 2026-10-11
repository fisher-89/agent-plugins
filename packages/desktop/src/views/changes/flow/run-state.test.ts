import { describe, expect, it } from 'vite-plus/test';

import type {
  ChangeRunEntry,
  ChangeRunNodeRecord,
  ChangeStepKind,
  ChangeStepState,
  RunStepKind,
} from '../../../types/dto';
import { runStepNodes, unifiedRunNodes } from './run-state';

// ---------------------------------------------------------------------------
// run-state 纯函数直测（零 react / 零进程边界依赖）：统一视图 fixture 以绑定
// 类型内存构造。覆盖：unifiedRunNodes 双源合成（`runs[].nodes` 展开 + 活节点
// 并入 + 步词汇十值归一单点）/ runStepNodes 按后端稳定身份键
// `(phase, attempt, step, instanceNo)` 直渲染（槽位配对与 `:seq` 堆叠整体退役）。
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
    instanceNo: 0,
    outputRef: null,
    ...overrides,
  };
}

/** 宽松默认值构造库读史节点行（ChangeRunNodeRecord，snake 词全词汇）。 */
function runStepRow(overrides: Partial<ChangeRunNodeRecord> = {}): ChangeRunNodeRecord {
  return {
    instanceNo: 0,
    phase: 'implement',
    attempt: 1,
    step: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    outputRef: null,
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
    nodes: [],
    edges: [],
    ...overrides,
  };
}

describe('unifiedRunNodes：双源合成（库读史 ∪ 活面）', () => {
  it('runs[].nodes 展开：库读史节点全量出线（库读序保持 emit 序），run 归属分层经 runs 序稳定', () => {
    const runs = [
      runEntry({
        runId: 'run-1',
        nodes: [
          runStepRow({ instanceNo: 0, step: 'executor' }),
          runStepRow({ instanceNo: 0, step: 'static_check', sessionId: null }),
          runStepRow({ instanceNo: 0, step: 'evaluator', sessionId: 'ses-eval-1' }),
        ],
      }),
    ];
    expect(unifiedRunNodes(runs, [])).toEqual([
      stepRow({ step: 'executor' }),
      stepRow({ step: 'staticCheck', sessionId: null }),
      stepRow({ step: 'evaluator', sessionId: 'ses-eval-1' }),
    ]);
  });

  it('步词汇归一单点：十值 snake 词 → camel 词（snake 词不入活面词汇）', () => {
    const runs = [
      runEntry({
        nodes: [
          runStepRow({ instanceNo: 0, step: 'static_check', sessionId: null }),
          runStepRow({ instanceNo: 0, step: 'test_execution', sessionId: null }),
          runStepRow({ instanceNo: 0, step: 'phase_start', sessionId: null }),
          runStepRow({ instanceNo: 0, step: 'phase_log', sessionId: null }),
          runStepRow({ instanceNo: 0, step: 'verdict_gate', sessionId: null }),
          runStepRow({ instanceNo: 0, step: 'retry_gate', sessionId: null }),
          runStepRow({ instanceNo: 0, step: 'whitelist_gate', sessionId: null }),
        ],
      }),
    ];
    expect(unifiedRunNodes(runs, []).map((row) => row.step)).toEqual([
      'staticCheck',
      'testExecution',
      'phaseStart',
      'phaseLog',
      'verdictGate',
      'retryGate',
      'whitelistGate',
    ]);
  });

  it('全史叠加：多 run 节点全保留（runs 序 = startedAt 序即稳定分层序），活节点并列入飞活面', () => {
    const runs = [
      runEntry({
        runId: 'run-1',
        startedAt: '2026-10-01T08:00:00.000Z',
        nodes: [runStepRow({ instanceNo: 0, attempt: 1 })],
      }),
      runEntry({
        runId: 'run-2',
        startedAt: '2026-10-01T09:00:00.000Z',
        nodes: [runStepRow({ instanceNo: 0, attempt: 2, status: 'passed' })],
      }),
    ];
    const steps = unifiedRunNodes(runs, [stepRow({ attempt: 3, status: 'passed' })]);
    expect(steps.map((row) => row.attempt)).toEqual([1, 2, 3]);
    expect(steps[2]).toMatchObject({ attempt: 3, status: 'passed' });
  });

  it('稳定身份键随行：instanceNo 与 outputRef 自库读史直透（feedback 重跑实例不串扰）', () => {
    const runs = [
      runEntry({
        nodes: [
          runStepRow({ instanceNo: 0, status: 'failed', detail: '门禁未过' }),
          runStepRow({
            instanceNo: 1,
            status: 'passed',
            detail: '按反馈修复重跑',
            sessionId: 'ses-fix',
            outputRef: 'exec-1',
          }),
        ],
      }),
    ];
    const steps = unifiedRunNodes(runs, []);
    expect(steps.map((row) => row.instanceNo)).toEqual([0, 1]);
    expect(steps[1]).toMatchObject({
      instanceNo: 1,
      status: 'passed',
      sessionId: 'ses-fix',
      outputRef: 'exec-1',
    });
  });

  it('空 runs / 空 liveNodes → 空数组（零 run 收口后无史形态）；仅 live（首 run 运行中零库史）→ 活节点全量出', () => {
    expect(unifiedRunNodes([], [])).toEqual([]);
    // 首 run 运行中（库面无史）→ 活面节点表全量出、零库读史分量
    expect(
      unifiedRunNodes(
        [],
        [stepRow({ step: 'executor' }), stepRow({ step: 'staticCheck', attempt: 2 })],
      ),
    ).toEqual([stepRow({ step: 'executor' }), stepRow({ step: 'staticCheck', attempt: 2 })]);
  });
});

describe('runStepNodes：稳定身份键渲染（overlay 输入面）', () => {
  it('节点按 (phase, attempt, step, instanceNo) 直渲染：同键多实例为独立节点不串扰', () => {
    const steps = [
      stepRow({ instanceNo: 0, status: 'failed', detail: '门禁未过' }),
      stepRow({
        instanceNo: 1,
        status: 'passed',
        detail: '按反馈修复重跑',
      }),
    ];

    const nodes = runStepNodes(steps);
    expect(nodes).toHaveLength(2);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:implement:1:executor:0',
      'run:implement:1:executor:1',
    ]);
    expect(nodes[1]).toMatchObject({
      instanceNo: 1,
      status: 'passed',
      detail: '按反馈修复重跑',
      sessionId: 'ses-exec-1',
    });
    expect(new Set(nodes.map((node) => node.id)).size).toBe(nodes.length);
  });

  it('id 定式逐字：run:<phase>:<attempt>:<step>:<instanceNo>（`:seq` 堆叠全退役）', () => {
    const steps = [
      stepRow(),
      stepRow({ step: 'staticCheck', sessionId: null }),
      stepRow({ step: 'staticCheck', sessionId: null, status: 'passed' }),
      stepRow({ step: 'staticCheck', sessionId: null, instanceNo: 1 }),
    ];

    const nodes = runStepNodes(steps);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:implement:1:executor:0',
      'run:implement:1:staticCheck:0',
      'run:implement:1:staticCheck:0',
      'run:implement:1:staticCheck:1',
    ]);
    expect(nodes.some((node) => node.id.includes(':seq'))).toBe(false);
  });

  it('载荷承接：runStepKind/group/role/status/sessionId/detail/outputRef/instanceNo 面 + kind/colIndex/parentId 布局面逐字段对应', () => {
    const steps = [
      stepRow(),
      stepRow({
        step: 'phaseLog',
        sessionId: null,
        status: 'passed',
        detail: 'attempt 1 已落账',
        outputRef: 'exec-9',
      }),
    ];

    const nodes = runStepNodes(steps);
    expect(nodes[0]).toEqual({
      id: 'run:implement:1:executor:0',
      kind: 'runtime',
      phase: 'implement',
      attempt: 1,
      colIndex: 3,
      order: 0,
      parentId: 'col:implement',
      runStepKind: 'executor',
      group: 'workerAgent',
      role: 'executor',
      instanceNo: 0,
      status: 'running',
      sessionId: 'ses-exec-1',
      detail: null,
      outputRef: null,
    });
    expect(nodes[1]).toMatchObject({
      id: 'run:implement:1:phaseLog:0',
      kind: 'runtime',
      runStepKind: 'phaseLog',
      group: 'toolStep',
      role: null,
      status: 'passed',
      sessionId: null,
      detail: 'attempt 1 已落账',
      outputRef: 'exec-9',
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
    expect(runStepNodes(steps).map((node) => node.id)).toEqual(['run:implement:1:executor:0']);
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

  it('空节点表 → 空数组（零节点 run 非错误）；单节点零边无反馈边渲染（overlay 缺省输出）', () => {
    expect(runStepNodes([])).toEqual([]);
    const single = runStepNodes([stepRow({ status: 'passed' })]);
    expect(single).toHaveLength(1);
    expect(single[0].instanceNo).toBe(0);
  });
});

describe('run-state：缺省字段（推导不崩）', () => {
  it('detail / outputRef 缺省（null）+ role null 形态：节点推导不崩、载荷为 null', () => {
    const steps = [
      stepRow({ step: 'whitelistGate', sessionId: null, detail: null, outputRef: null }),
    ];
    const nodes = runStepNodes(steps);
    expect(nodes[0]).toMatchObject({
      id: 'run:implement:1:whitelistGate:0',
      group: 'gate',
      role: null,
      sessionId: null,
      detail: null,
      outputRef: null,
    });
  });

  it('instanceNo 缺号（undefined）→ 身份串退化不崩，不抛错', () => {
    const broken = { ...stepRow(), instanceNo: undefined } as unknown as ChangeStepState;
    const nodes = runStepNodes([broken]);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].id).toBe('run:implement:1:executor:undefined');
  });
});

// RunStepKind 词汇 import 面锚（十值全词汇直测归一单点的输入域）
const PERSISTED_WORDS: RunStepKind[] = [
  'executor',
  'evaluator',
  'decision',
  'static_check',
  'test_execution',
  'phase_start',
  'phase_log',
  'verdict_gate',
  'retry_gate',
  'whitelist_gate',
];
describe('unifiedRunNodes：词汇全词汇归一', () => {
  it('十值 snake 词全量归一可达（词汇漂移即断）', () => {
    const steps = PERSISTED_WORDS.map((step, index) =>
      runStepRow({ instanceNo: index, step, sessionId: null }),
    );
    const unified = unifiedRunNodes([runEntry({ nodes: steps })], []);
    expect(unified.map((row) => row.step)).toEqual([
      'executor',
      'evaluator',
      'decision',
      'staticCheck',
      'testExecution',
      'phaseStart',
      'phaseLog',
      'verdictGate',
      'retryGate',
      'whitelistGate',
    ]);
  });
});
