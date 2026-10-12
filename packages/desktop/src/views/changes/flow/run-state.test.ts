import { describe, expect, it } from 'vite-plus/test';

import type {
  ChangeRunEntry,
  ChangeRunNodeRecord,
  ChangeStepKind,
  ChangeStepState,
  RunStepKind,
} from '../../../types/dto';
import { nodeIdentityKey, runStepNodes, unifiedRunNodes } from './run-state';

/**
 * requirement 相位序夹具（D12：前端 `PIPELINE_PHASES` 常量退役，`runStepNodes`
 * 改显式传序，测试各自内联 fixture）：9 列，archive 为第 9 列（colIndex=8）。
 */
const REQUIREMENT_PHASES = [
  'proposal',
  'dev-design',
  'test-design',
  'implement',
  'test-gen',
  'test-execution',
  'code-review',
  'acceptance',
  'archive',
];

// ---------------------------------------------------------------------------
// run-state 纯函数直测（零 react / 零进程边界依赖）：统一视图 fixture 以绑定
// 类型内存构造。覆盖：unifiedRunNodes 双源合成（`runs[].nodes` 展开 + 活节点
// 并入 + 步词汇十八值归一单点）/ runStepNodes 按后端稳定身份键
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

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
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

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
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

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
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

  it('四类可辨：十八步词汇 → WorkerAgent / ToolStep / Git / Gate 分组与 role 逐词对应', () => {
    const GROUP_OF: Record<
      ChangeStepKind,
      {
        group: 'workerAgent' | 'toolStep' | 'git' | 'gate';
        role: 'executor' | 'evaluator' | 'decision' | 'spec_sync' | 'resolve_conflicts' | null;
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
      preflight: { group: 'toolStep', role: null },
      commit: { group: 'git', role: null },
      rebase: { group: 'git', role: null },
      resolveConflicts: { group: 'workerAgent', role: 'resolve_conflicts' },
      merge: { group: 'git', role: null },
      specSync: { group: 'workerAgent', role: 'spec_sync' },
      seal: { group: 'toolStep', role: null },
      finalize: { group: 'git', role: null },
    };

    const steps = (Object.keys(GROUP_OF) as ChangeStepKind[]).map((kind, index) =>
      stepRow({ step: kind, attempt: index }),
    );

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    expect(nodes).toHaveLength(18);
    for (const node of nodes) {
      const expected = GROUP_OF[node.runStepKind];
      expect(node.group).toBe(expected.group);
      expect(node.role).toBe(expected.role);
    }
  });

  it('缺号相位跳过：phase 不在相位表内的步不入图（列序随出线）', () => {
    const steps = [stepRow({ phase: 'bootstrap' }), stepRow()];
    expect(runStepNodes(steps, REQUIREMENT_PHASES).map((node) => node.id)).toEqual([
      'run:implement:1:executor:0',
    ]);
  });

  it('列内归并序：同列多节点按归并序 0 起编号（order 面）', () => {
    const steps = [
      stepRow(),
      stepRow({ step: 'phaseStart', sessionId: null }),
      stepRow({ step: 'verdictGate', sessionId: null }),
    ];

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    expect(nodes.every((node) => node.colIndex === 3 && node.parentId === 'col:implement')).toBe(
      true,
    );
    expect(nodes.map((node) => node.order)).toEqual([0, 1, 2]);
  });

  it('空节点表 → 空数组（零节点 run 非错误）；单节点零边无反馈边渲染（overlay 缺省输出）', () => {
    expect(runStepNodes([], REQUIREMENT_PHASES)).toEqual([]);
    const single = runStepNodes([stepRow({ status: 'passed' })], REQUIREMENT_PHASES);
    expect(single).toHaveLength(1);
    expect(single[0].instanceNo).toBe(0);
  });
});

describe('run-state：缺省字段（推导不崩）', () => {
  it('detail / outputRef 缺省（null）+ role null 形态：节点推导不崩、载荷为 null', () => {
    const steps = [
      stepRow({ step: 'whitelistGate', sessionId: null, detail: null, outputRef: null }),
    ];
    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
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
    const nodes = runStepNodes([broken], REQUIREMENT_PHASES);
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

describe('run-state：归档 8 步词归一与 Git 分类（AC-2 / AC-9）', () => {
  it('归档 8 步词归一：库读史 snake 词 → 活面 camel 词（resolve_conflicts / spec_sync 多词步逐词对应）', () => {
    const steps = [
      'preflight',
      'commit',
      'rebase',
      'resolve_conflicts',
      'merge',
      'spec_sync',
      'seal',
      'finalize',
    ].map((step, index) => runStepRow({ instanceNo: index, step: step as RunStepKind }));

    const unified = unifiedRunNodes([runEntry({ nodes: steps })], []);

    expect(unified.map((row) => row.step)).toEqual([
      'preflight',
      'commit',
      'rebase',
      'resolveConflicts',
      'merge',
      'specSync',
      'seal',
      'finalize',
    ]);
  });

  it('反向归一：活面 camel 词 → 边表身份串 snake 词（nodeIdentityKey 与后端端点逐字可比）', () => {
    const words: Array<[ChangeStepKind, string]> = [
      ['resolveConflicts', 'resolve_conflicts'],
      ['specSync', 'spec_sync'],
      ['preflight', 'preflight'],
      ['commit', 'commit'],
      ['rebase', 'rebase'],
      ['merge', 'merge'],
      ['seal', 'seal'],
      ['finalize', 'finalize'],
    ];

    for (const [kind, snake] of words) {
      const node = runStepNodes(
        [stepRow({ phase: 'archive', attempt: 1, step: kind, instanceNo: 0 })],
        REQUIREMENT_PHASES,
      )[0];
      expect(node).toBeDefined();
      expect(nodeIdentityKey(node)).toBe(`archive:1:${snake}:0`);
    }
  });

  it('归档节点渲染：十步按 (phase, attempt, step, instanceNo) 直渲染，group / runStepKind / instanceNo 逐字段在案', () => {
    const kinds: ChangeStepKind[] = [
      'phaseStart',
      'preflight',
      'commit',
      'rebase',
      'resolveConflicts',
      'merge',
      'specSync',
      'seal',
      'finalize',
      'phaseLog',
    ];
    const steps = kinds.map((kind, index) =>
      stepRow({
        phase: 'archive',
        attempt: 1,
        step: kind,
        instanceNo: index === 0 ? 0 : 1,
        sessionId: kind === 'specSync' || kind === 'resolveConflicts' ? 'ses-arch' : null,
      }),
    );

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);

    expect(nodes.map((node) => node.runStepKind)).toEqual(kinds);
    expect(nodes.map((node) => node.group)).toEqual([
      'toolStep',
      'toolStep',
      'git',
      'git',
      'workerAgent',
      'git',
      'workerAgent',
      'toolStep',
      'git',
      'toolStep',
    ]);
    expect(nodes.every((node) => node.phase === 'archive')).toBe(true);
    expect(nodes.map((node) => node.instanceNo)).toEqual([0, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    // 会话面：仅两个 agent 会话步携 sessionId
    expect(nodes.filter((node) => node.sessionId !== null).map((node) => node.runStepKind)).toEqual(
      ['resolveConflicts', 'specSync'],
    );
    // 列归属：archive 为第 9 列（colIndex=8）
    expect(nodes.every((node) => node.colIndex === 8)).toBe(true);
  });

  it('归档列零步：零节点 → 空数组（archive 列空列不报错，miss 非错误）', () => {
    expect(runStepNodes([], REQUIREMENT_PHASES)).toEqual([]);
    // 仅其它相位节点时 archive 零节点不参与渲染
    const nodes = runStepNodes(
      [stepRow({ phase: 'implement', step: 'executor' })],
      REQUIREMENT_PHASES,
    );
    expect(nodes).toHaveLength(1);
    expect(nodes[0].colIndex).not.toBe(8);
  });

  it('未知步词降级：越集步词经 groupOf 缺省分组回落不崩（稳健面）', () => {
    const nodes = runStepNodes(
      [stepRow({ phase: 'archive', step: 'unknownStep' as ChangeStepKind })],
      REQUIREMENT_PHASES,
    );
    expect(nodes).toHaveLength(1);
    expect(nodes[0].runStepKind).toBe('unknownStep');
    expect(nodes[0].role).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// phaseOrder 显式传序（desktop-test-only-flow AC-5 / AC-6）：列序恒由入参相位序
// 推导（前端零静态相位序常量），两表各自成立
// ---------------------------------------------------------------------------

/** test-only 相位序夹具：6 列，code-analyze 下标 1、archive 下标 5。 */
const TEST_ONLY_PHASES = [
  'proposal',
  'code-analyze',
  'test-design',
  'test-gen',
  'test-execution',
  'archive',
];

describe('runStepNodes：phaseOrder 显式传序（AC-5 / AC-6）', () => {
  it('test-only phaseOrder：code-analyze 下标 1、test-gen 下标 3、archive 下标 5（列序随出线序）', () => {
    const steps = [
      stepRow({ phase: 'code-analyze', step: 'executor' }),
      stepRow({ phase: 'test-gen', step: 'staticCheck', sessionId: null }),
      stepRow({ phase: 'archive', step: 'commit', sessionId: null }),
    ];

    const nodes = runStepNodes(steps, TEST_ONLY_PHASES);

    expect(nodes.map((node) => node.colIndex)).toEqual([1, 3, 5]);
    expect(nodes.map((node) => node.parentId)).toEqual([
      'col:code-analyze',
      'col:test-gen',
      'col:archive',
    ]);
  });

  it('同一相位在 requirement 9 列与 test-only 6 列下 colIndex 各自为真（列序即语义坐标）', () => {
    const steps = [stepRow({ phase: 'archive', step: 'commit', sessionId: null })];

    expect(runStepNodes(steps, REQUIREMENT_PHASES)[0].colIndex).toBe(8);
    expect(runStepNodes(steps, TEST_ONLY_PHASES)[0].colIndex).toBe(5);
  });

  it('异常：phaseOrder 为空数组 → 全部步不入图（零臆造列位）', () => {
    const steps = [
      stepRow({ phase: 'proposal' }),
      stepRow({ phase: 'code-analyze' }),
      stepRow({ phase: 'archive', step: 'commit', sessionId: null }),
    ];

    expect(runStepNodes(steps, [])).toEqual([]);
  });

  it('异常：两表互不越集——requirement 独有相位在 test-only 序下跳过、test-only 独有相位在 requirement 序下跳过', () => {
    const steps = [
      stepRow({ phase: 'implement', step: 'executor' }),
      stepRow({ phase: 'code-analyze', step: 'executor' }),
      stepRow({ phase: 'proposal', step: 'executor' }),
    ];

    const requirementNodes = runStepNodes(steps, REQUIREMENT_PHASES);
    expect(requirementNodes.map((node) => node.phase)).toEqual(['implement', 'proposal']);
    expect(requirementNodes.map((node) => node.colIndex)).toEqual([3, 0]);

    const testOnlyNodes = runStepNodes(steps, TEST_ONLY_PHASES);
    expect(testOnlyNodes.map((node) => node.phase)).toEqual(['code-analyze', 'proposal']);
    expect(testOnlyNodes.map((node) => node.colIndex)).toEqual([1, 0]);
  });

  it('边界：列内归并序随传入 phaseOrder 独立成立（不跨表串扰）', () => {
    const steps = [
      stepRow({ phase: 'test-design', attempt: 1, step: 'executor' }),
      stepRow({ phase: 'test-design', attempt: 1, step: 'evaluator' }),
      stepRow({ phase: 'test-design', attempt: 1, step: 'verdictGate', sessionId: null }),
    ];

    const nodes = runStepNodes(steps, TEST_ONLY_PHASES);

    expect(nodes.map((node) => node.order)).toEqual([0, 1, 2]);
    expect(nodes.every((node) => node.colIndex === 2)).toBe(true);
    expect(nodes.map((node) => node.runStepKind)).toEqual(['executor', 'evaluator', 'verdictGate']);
  });

  it('边界：身份键与列位解耦——同一步在两套 phaseOrder 下 id 不变、colIndex 变', () => {
    const steps = [stepRow({ phase: 'archive', step: 'phaseStart', sessionId: null })];

    const requirementNode = runStepNodes(steps, REQUIREMENT_PHASES)[0];
    const testOnlyNode = runStepNodes(steps, TEST_ONLY_PHASES)[0];

    expect(nodeIdentityKey(requirementNode)).toBe('archive:1:phase_start:0');
    expect(nodeIdentityKey(testOnlyNode)).toBe(nodeIdentityKey(requirementNode));
    expect(requirementNode.colIndex).not.toBe(testOnlyNode.colIndex);
  });
});
