import { describe, expect, it } from 'vite-plus/test';

import type {
  ChangeRunEntry,
  ChangeRunNodeRecord,
  ChangeStepKind,
  RunStepKind,
} from '../../../types/dto';
import {
  nodeIdentityKey,
  runNodeKey,
  runStepNodes,
  unifiedRunNodes,
  type LiveRunFace,
  type UnifiedRunStep,
} from './run-state';

/**
 * requirement 相位序夹具（D12：前端 `PIPELINE_PHASES` 常量退役，`runStepNodes`
 * 改显式传序，测试各自内联 fixture）：10 列（check-before-flow-start 表头
 * initialize 一次性相位），archive 为第 10 列（colIndex=9）。
 */
const REQUIREMENT_PHASES = [
  'initialize',
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
// run-state 纯函数直测（零 react / 零进程边界依赖）
// ---------------------------------------------------------------------------

/** 宽松默认值构造单条统一视图步行（ChangeStepState + 归属 run id + 活面来源
 * 标注——runStepNodes 与 unifiedRunNodes 期望面的共同口径）。 */
function stepRow(overrides: Partial<UnifiedRunStep> = {}): UnifiedRunStep {
  return {
    phase: 'implement',
    attempt: 1,
    step: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    instanceNo: 0,
    outputRef: null,
    startedAt: 1_727_000_000_000,
    endedAt: null,
    tokenInput: null,
    tokenOutput: null,
    runId: 'run-1727',
    live: true,
    ...overrides,
  };
}

/** 宽松默认值构造活面 run 缝（LiveRunFace：run id + 节点 / 边同源捆绑）。 */
function liveFace(overrides: Partial<LiveRunFace> = {}): LiveRunFace {
  return { runId: 'run-live', nodes: [], edges: [], ...overrides };
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
    startedAt: 1_727_000_000_000,
    endedAt: null,
    tokenInput: null,
    tokenOutput: null,
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
    // 库读史源 → live=false（活面标注半边——liveRun=null 全库读史形态）
    expect(unifiedRunNodes(runs, null)).toEqual([
      stepRow({ runId: 'run-1', step: 'executor', live: false }),
      stepRow({ runId: 'run-1', step: 'staticCheck', sessionId: null, live: false }),
      stepRow({ runId: 'run-1', step: 'evaluator', sessionId: 'ses-eval-1', live: false }),
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
    expect(unifiedRunNodes(runs, null).map((row) => row.step)).toEqual([
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
    const steps = unifiedRunNodes(
      runs,
      liveFace({ nodes: [stepRow({ runId: 'run-live', attempt: 3, status: 'passed' })] }),
    );
    expect(steps.map((row) => row.attempt)).toEqual([1, 2, 3]);
    expect(steps.map((row) => row.runId)).toEqual(['run-1', 'run-2', 'run-live']);
    expect(steps[2]).toMatchObject({ attempt: 3, status: 'passed', runId: 'run-live' });
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
    const steps = unifiedRunNodes(runs, null);
    expect(steps.map((row) => row.instanceNo)).toEqual([0, 1]);
    expect(steps[1]).toMatchObject({
      instanceNo: 1,
      status: 'passed',
      sessionId: 'ses-fix',
      outputRef: 'exec-1',
    });
  });

  it('空 runs / 空活面 → 空数组（零 run 收口后无史形态）；仅 live（首 run 运行中零库史）→ 活节点全量出', () => {
    expect(unifiedRunNodes([], null)).toEqual([]);
    // 首 run 运行中（库面无史）→ 活面节点表全量出、零库读史分量
    expect(
      unifiedRunNodes(
        [],
        liveFace({
          nodes: [
            stepRow({ runId: 'run-live', step: 'executor' }),
            stepRow({ runId: 'run-live', step: 'staticCheck', attempt: 2 }),
          ],
        }),
      ),
    ).toEqual([
      stepRow({ runId: 'run-live', step: 'executor' }),
      stepRow({ runId: 'run-live', step: 'staticCheck', attempt: 2 }),
    ]);
  });
});

describe('unifiedRunNodes / runStepNodes：重启重跑同 attempt（跨 run 同名身份键）', () => {
  it('两 run 同 (phase, attempt, step, instanceNo)：节点 id 携 run 归属段互不顶替、全量出线', () => {
    const runs = [
      runEntry({
        runId: 'run-1',
        nodes: [
          runStepRow({ phase: 'test-gen', attempt: 1, step: 'phase_start', sessionId: null }),
        ],
      }),
    ];
    const steps = unifiedRunNodes(
      runs,
      liveFace({
        runId: 'run-2',
        nodes: [
          stepRow({
            runId: 'run-2',
            phase: 'test-gen',
            attempt: 1,
            step: 'phaseStart',
            sessionId: null,
          }),
        ],
      }),
    );
    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    // 前一轮与重启轮的 phaseStart 同为 attempt 1 实例 0：id 以 run 段区分、两节点并存
    expect(nodes.map((node) => node.id)).toEqual([
      'run:run-1:test-gen:1:phaseStart:0',
      'run:run-2:test-gen:1:phaseStart:0',
    ]);
    expect(nodes.map((node) => node.runId)).toEqual(['run-1', 'run-2']);
    expect(new Set(nodes.map((node) => node.id)).size).toBe(nodes.length);
  });

  it('runNodeKey：run 归属段前拼身份键（边锚定与节点侧同式），nodeIdentityKey 仍为后端端点口径', () => {
    const node = runStepNodes([stepRow({ runId: 'run-9' })], REQUIREMENT_PHASES)[0];
    expect(runNodeKey(node)).toBe('run-9:implement:1:executor:0');
    expect(nodeIdentityKey(node)).toBe('implement:1:executor:0');
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
      'run:run-1727:implement:1:executor:0',
      'run:run-1727:implement:1:executor:1',
    ]);
    expect(nodes[1]).toMatchObject({
      instanceNo: 1,
      status: 'passed',
      detail: '按反馈修复重跑',
      sessionId: 'ses-exec-1',
    });
    expect(new Set(nodes.map((node) => node.id)).size).toBe(nodes.length);
  });

  it('id 定式逐字：run:<runId>:<phase>:<attempt>:<step>:<instanceNo>（`:seq` 堆叠全退役）', () => {
    const steps = [
      stepRow(),
      stepRow({ step: 'staticCheck', sessionId: null }),
      stepRow({ step: 'staticCheck', sessionId: null, status: 'passed' }),
      stepRow({ step: 'staticCheck', sessionId: null, instanceNo: 1 }),
    ];

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    expect(nodes.map((node) => node.id)).toEqual([
      'run:run-1727:implement:1:executor:0',
      'run:run-1727:implement:1:staticCheck:0',
      'run:run-1727:implement:1:staticCheck:0',
      'run:run-1727:implement:1:staticCheck:1',
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
      id: 'run:run-1727:implement:1:executor:0',
      kind: 'runtime',
      phase: 'implement',
      attempt: 1,
      colIndex: 4,
      order: 0,
      parentId: 'col:implement',
      runId: 'run-1727',
      runStepKind: 'executor',
      group: 'workerAgent',
      role: 'executor',
      instanceNo: 0,
      status: 'running',
      sessionId: 'ses-exec-1',
      detail: null,
      outputRef: null,
      startedAt: 1_727_000_000_000,
      endedAt: null,
      tokenInput: null,
      tokenOutput: null,
      live: true,
    });
    expect(nodes[1]).toMatchObject({
      id: 'run:run-1727:implement:1:phaseLog:0',
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

  it('四类可辨：二十二步词汇 → WorkerAgent / ToolStep / Git / Gate 分组与 role 逐词对应', () => {
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
      mutationTest: { group: 'toolStep', role: null },
      archiCheck: { group: 'toolStep', role: null },
      clean: { group: 'git', role: null },
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
      cleanup: { group: 'git', role: null },
      seal: { group: 'toolStep', role: null },
      finalize: { group: 'git', role: null },
    };

    const steps = (Object.keys(GROUP_OF) as ChangeStepKind[]).map((kind, index) =>
      stepRow({ step: kind, attempt: index }),
    );

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    expect(nodes).toHaveLength(22);
    for (const node of nodes) {
      const expected = GROUP_OF[node.runStepKind];
      expect(node.group).toBe(expected.group);
      expect(node.role).toBe(expected.role);
    }
  });

  it('缺号相位跳过：phase 不在相位表内的步不入图（列序随出线）', () => {
    const steps = [stepRow({ phase: 'bootstrap' }), stepRow()];
    expect(runStepNodes(steps, REQUIREMENT_PHASES).map((node) => node.id)).toEqual([
      'run:run-1727:implement:1:executor:0',
    ]);
  });

  it('列内归并序：同列多节点按归并序 0 起编号（order 面）', () => {
    const steps = [
      stepRow(),
      stepRow({ step: 'phaseStart', sessionId: null }),
      stepRow({ step: 'verdictGate', sessionId: null }),
    ];

    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    expect(nodes.every((node) => node.colIndex === 4 && node.parentId === 'col:implement')).toBe(
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
      id: 'run:run-1727:implement:1:whitelistGate:0',
      group: 'gate',
      role: null,
      sessionId: null,
      detail: null,
      outputRef: null,
    });
  });

  it('instanceNo 缺号（undefined）→ 身份串退化不崩，不抛错', () => {
    const broken = { ...stepRow(), instanceNo: undefined } as unknown as UnifiedRunStep;
    const nodes = runStepNodes([broken], REQUIREMENT_PHASES);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].id).toBe('run:run-1727:implement:1:executor:undefined');
  });
});

// RunStepKind 词汇 import 面锚（二十二值全词汇直测归一单点的输入域——mutation_test
// 拆步新词、cleanup 尽力步与 clean 基线恢复步新臂在案、archi_check 架构门禁步尾词
// 在案，AC-1 / AC-2 / AC-16）
const PERSISTED_WORDS: RunStepKind[] = [
  'executor',
  'evaluator',
  'decision',
  'static_check',
  'test_execution',
  'mutation_test',
  'clean',
  'phase_start',
  'phase_log',
  'verdict_gate',
  'retry_gate',
  'whitelist_gate',
  'preflight',
  'commit',
  'rebase',
  'resolve_conflicts',
  'merge',
  'spec_sync',
  'cleanup',
  'seal',
  'finalize',
  'archi_check',
];

/** 二十二值 snake 词 → camel 词期望表（normalizeKind 逐词对拍——词汇漂移即断；
 * cleanup 为 identity 词、snake / camel 同形零显式 case；archi_check → archiCheck
 * 为架构门禁步新臂）。 */
const NORMALIZE_EXPECTED: Array<[RunStepKind, ChangeStepKind]> = [
  ['executor', 'executor'],
  ['evaluator', 'evaluator'],
  ['decision', 'decision'],
  ['static_check', 'staticCheck'],
  ['test_execution', 'testExecution'],
  ['mutation_test', 'mutationTest'],
  ['clean', 'clean'],
  ['phase_start', 'phaseStart'],
  ['phase_log', 'phaseLog'],
  ['verdict_gate', 'verdictGate'],
  ['retry_gate', 'retryGate'],
  ['whitelist_gate', 'whitelistGate'],
  ['preflight', 'preflight'],
  ['commit', 'commit'],
  ['rebase', 'rebase'],
  ['resolve_conflicts', 'resolveConflicts'],
  ['merge', 'merge'],
  ['spec_sync', 'specSync'],
  ['cleanup', 'cleanup'],
  ['seal', 'seal'],
  ['finalize', 'finalize'],
  ['archi_check', 'archiCheck'],
];

describe('unifiedRunNodes：二十二值词汇全量归一', () => {
  it('二十二值 snake 词全量归一可达（normalizeKind 逐词对拍——词汇漂移即断）', () => {
    const steps = PERSISTED_WORDS.map((step, index) =>
      runStepRow({ instanceNo: index, step, sessionId: null }),
    );
    const unified = unifiedRunNodes([runEntry({ nodes: steps })], null);
    expect(unified.map((row) => row.step)).toEqual(NORMALIZE_EXPECTED.map(([, camel]) => camel));
    // 逐词对拍（mutation_test → mutationTest 拆步新词在案；cleanup / clean identity 在案）
    for (const [snake, camel] of NORMALIZE_EXPECTED) {
      const row = unified.find((_, index) => PERSISTED_WORDS[index] === snake);
      // normalizeKind(${snake}) → ${camel}
      expect(row?.step).toBe(camel);
    }
  });

  it('双向 round-trip：stepWord(normalizeKind(word)) 恒回 snake 词（二十二值全词汇）', () => {
    const steps = PERSISTED_WORDS.map((step, index) =>
      runStepRow({ instanceNo: index, step, sessionId: null }),
    );
    const unified = unifiedRunNodes([runEntry({ nodes: steps })], null);
    const nodes = runStepNodes(unified, REQUIREMENT_PHASES);
    // nodeIdentityKey 内嵌 stepWord 反向归一——round-trip 逐词回到 snake 词面
    for (const [snake, camel] of NORMALIZE_EXPECTED) {
      const node = nodes.find((candidate) => candidate.runStepKind === camel);
      expect(node).toBeDefined();
      if (node) {
        expect(nodeIdentityKey(node)).toBe(
          `implement:1:${snake}:${PERSISTED_WORDS.indexOf(snake)}`,
        );
      }
    }
  });
});

describe('run-state：cleanup 步词归一与身份键（worktree-cleanup-and-active-cap）', () => {
  it("normalizeKind('cleanup') → 'cleanup'：identity 词走 default 分支直返（与 commit / rebase / merge / finalize / preflight / seal 先例一致）", () => {
    const unified = unifiedRunNodes(
      [runEntry({ nodes: [runStepRow({ phase: 'archive', step: 'cleanup', sessionId: null })] })],
      null,
    );
    expect(unified.map((row) => row.step)).toEqual(['cleanup']);
  });

  it('cleanup 节点身份键含 stepWord 词面：{phase}:{attempt}:cleanup:{instanceNo}（与后端边表端点逐字可比）', () => {
    const node = runStepNodes(
      [stepRow({ phase: 'archive', step: 'cleanup', sessionId: null, instanceNo: 0 })],
      REQUIREMENT_PHASES,
    )[0];
    expect(nodeIdentityKey(node)).toBe('archive:1:cleanup:0');
  });

  it("groupOf('cleanup') 归 git 分类：经 runStepNodes 分组出线（与 commit / rebase / merge / finalize 同组）", () => {
    const node = runStepNodes(
      [stepRow({ phase: 'archive', step: 'cleanup', sessionId: null })],
      REQUIREMENT_PHASES,
    )[0];
    expect(node.group).toBe('git');
    expect(node.role).toBeNull();
  });

  it('库读史行 step=cleanup（RunStepKind 线词）→ 统一视图节点归一出线（双源合成对新词透明）', () => {
    const steps = [
      runStepRow({ instanceNo: 0, phase: 'archive', step: 'spec_sync', sessionId: 'ses-arch' }),
      runStepRow({ instanceNo: 0, phase: 'archive', step: 'cleanup', sessionId: null }),
      runStepRow({ instanceNo: 0, phase: 'archive', step: 'seal', sessionId: null }),
    ];
    const unified = unifiedRunNodes([runEntry({ nodes: steps })], null);
    expect(unified.map((row) => row.step)).toEqual(['specSync', 'cleanup', 'seal']);
    expect(unified[1].sessionId).toBeNull();
    const nodes = runStepNodes(unified, REQUIREMENT_PHASES);
    expect(nodes.map((node) => node.group)).toEqual(['workerAgent', 'git', 'toolStep']);
  });
});

describe('run-state：归档 9 步词归一与 Git 分类（AC-2 / AC-9）', () => {
  it('归档 9 步词归一：库读史 snake 词 → 活面 camel 词（resolve_conflicts / spec_sync 多词步逐词对应、cleanup identity）', () => {
    const steps = [
      'preflight',
      'commit',
      'rebase',
      'resolve_conflicts',
      'merge',
      'spec_sync',
      'cleanup',
      'seal',
      'finalize',
    ].map((step, index) => runStepRow({ instanceNo: index, step: step as RunStepKind }));

    const unified = unifiedRunNodes([runEntry({ nodes: steps })], null);

    expect(unified.map((row) => row.step)).toEqual([
      'preflight',
      'commit',
      'rebase',
      'resolveConflicts',
      'merge',
      'specSync',
      'cleanup',
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
      ['cleanup', 'cleanup'],
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

  it('归档节点渲染：十一步按 (phase, attempt, step, instanceNo) 直渲染，group / runStepKind / instanceNo 逐字段在案', () => {
    const kinds: ChangeStepKind[] = [
      'phaseStart',
      'preflight',
      'commit',
      'rebase',
      'resolveConflicts',
      'merge',
      'specSync',
      'cleanup',
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
      'git',
      'toolStep',
      'git',
      'toolStep',
    ]);
    expect(nodes.every((node) => node.phase === 'archive')).toBe(true);
    expect(nodes.map((node) => node.instanceNo)).toEqual([0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    // 会话面：仅两个 agent 会话步携 sessionId
    expect(nodes.filter((node) => node.sessionId !== null).map((node) => node.runStepKind)).toEqual(
      ['resolveConflicts', 'specSync'],
    );
    // 列归属：archive 为第 10 列（colIndex=9）
    expect(nodes.every((node) => node.colIndex === 9)).toBe(true);
  });

  it('归档列零步：零节点 → 空数组（archive 列空列不报错，miss 非错误）', () => {
    expect(runStepNodes([], REQUIREMENT_PHASES)).toEqual([]);
    // 仅其它相位节点时 archive 零节点不参与渲染
    const nodes = runStepNodes(
      [stepRow({ phase: 'implement', step: 'executor' })],
      REQUIREMENT_PHASES,
    );
    expect(nodes).toHaveLength(1);
    expect(nodes[0].colIndex).not.toBe(9);
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

/** test-only 相位序夹具：7 列（check-before-flow-start 表头 initialize 一次性相位），
 * code-analyze 下标 2、archive 下标 6。 */
const TEST_ONLY_PHASES = [
  'initialize',
  'proposal',
  'code-analyze',
  'test-design',
  'test-gen',
  'test-execution',
  'archive',
];

describe('runStepNodes：phaseOrder 显式传序（AC-5 / AC-6）', () => {
  it('test-only phaseOrder：code-analyze 下标 2、test-gen 下标 4、archive 下标 6（列序随出线序）', () => {
    const steps = [
      stepRow({ phase: 'code-analyze', step: 'executor' }),
      stepRow({ phase: 'test-gen', step: 'staticCheck', sessionId: null }),
      stepRow({ phase: 'archive', step: 'commit', sessionId: null }),
    ];

    const nodes = runStepNodes(steps, TEST_ONLY_PHASES);

    expect(nodes.map((node) => node.colIndex)).toEqual([2, 4, 6]);
    expect(nodes.map((node) => node.parentId)).toEqual([
      'col:code-analyze',
      'col:test-gen',
      'col:archive',
    ]);
  });

  it('同一相位在 requirement 10 列与 test-only 7 列下 colIndex 各自为真（列序即语义坐标）', () => {
    const steps = [stepRow({ phase: 'archive', step: 'commit', sessionId: null })];

    expect(runStepNodes(steps, REQUIREMENT_PHASES)[0].colIndex).toBe(9);
    expect(runStepNodes(steps, TEST_ONLY_PHASES)[0].colIndex).toBe(6);
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
    expect(requirementNodes.map((node) => node.colIndex)).toEqual([4, 1]);

    const testOnlyNodes = runStepNodes(steps, TEST_ONLY_PHASES);
    expect(testOnlyNodes.map((node) => node.phase)).toEqual(['code-analyze', 'proposal']);
    expect(testOnlyNodes.map((node) => node.colIndex)).toEqual([2, 1]);
  });

  it('边界：列内归并序随传入 phaseOrder 独立成立（不跨表串扰）', () => {
    const steps = [
      stepRow({ phase: 'test-design', attempt: 1, step: 'executor' }),
      stepRow({ phase: 'test-design', attempt: 1, step: 'evaluator' }),
      stepRow({ phase: 'test-design', attempt: 1, step: 'verdictGate', sessionId: null }),
    ];

    const nodes = runStepNodes(steps, TEST_ONLY_PHASES);

    expect(nodes.map((node) => node.order)).toEqual([0, 1, 2]);
    expect(nodes.every((node) => node.colIndex === 3)).toBe(true);
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

// ---------------------------------------------------------------------------
// mutationTest 步词汇（desktop-fix-mutation-test / AC-1 / AC-2）：归一表新词 +
// groupOf 落 toolStep（exec 组）+ 枚举外字符串 default 面 + 零过滤丢步
// ---------------------------------------------------------------------------

describe('mutationTest 步词汇归一与分组（AC-2）', () => {
  it('normalizeKind：mutation_test → mutationTest 拆步新词逐字在案', () => {
    const steps = [runStepRow({ instanceNo: 0, step: 'mutation_test', sessionId: null })];
    const unified = unifiedRunNodes([runEntry({ nodes: steps })], null);
    expect(unified.map((row) => row.step)).toEqual(['mutationTest']);
  });

  it('stepWord 反向归一：mutationTest → mutation_test（nodeIdentityKey 与后端边表端点逐字可比）', () => {
    const node = runStepNodes(
      [
        stepRow({
          phase: 'test-execution',
          attempt: 1,
          step: 'mutationTest',
          instanceNo: 0,
          sessionId: null,
        }),
      ],
      REQUIREMENT_PHASES,
    )[0];
    expect(node).toBeDefined();
    if (node) {
      expect(nodeIdentityKey(node)).toBe('test-execution:1:mutation_test:0');
      expect(runNodeKey(node)).toBe('run-1727:test-execution:1:mutation_test:0');
    }
  });

  it('groupOf：mutationTest 经 default 落 toolStep（exec 组）；role 恒 null', () => {
    const nodes = runStepNodes(
      [stepRow({ phase: 'test-execution', step: 'mutationTest', sessionId: null })],
      REQUIREMENT_PHASES,
    );
    expect(nodes).toHaveLength(1);
    // mutationTest 归 exec 组（经 groupOf default）
    expect(nodes[0].group).toBe('toolStep');
    expect(nodes[0].role).toBeNull();
  });

  it('枚举外字符串（未知词 cast）→ default 落 toolStep 不崩不抛（稳健面）', () => {
    const rogue = 'totallyUnknownStep' as ChangeStepKind;
    const nodes = runStepNodes([stepRow({ step: rogue, sessionId: null })], REQUIREMENT_PHASES);
    expect(nodes).toHaveLength(1);
    // 未知词经 default 兜底落 toolStep
    expect(nodes[0].group).toBe('toolStep');
    expect(nodes[0].role).toBeNull();
  });

  it('零过滤丢步：二十二词全词汇经 runStepNodes 全量出图（mutation_test / cleanup / clean / archi_check 在案不被滤除）', () => {
    const steps = PERSISTED_WORDS.map((step, index) =>
      stepRow({ instanceNo: index, step: normalizeWord(step), sessionId: null }),
    );
    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    // 二十二词全量出图（零过滤）
    expect(nodes).toHaveLength(22);
    expect(nodes.some((node) => node.runStepKind === 'mutationTest')).toBe(true);
    expect(nodes.some((node) => node.runStepKind === 'cleanup')).toBe(true);
    expect(nodes.some((node) => node.runStepKind === 'clean')).toBe(true);
    expect(nodes.some((node) => node.runStepKind === 'archiCheck')).toBe(true);
    expect(new Set(nodes.map((node) => node.id)).size).toBe(22);
  });
});

/** snake 词 → 活面 camel 词（测试内小表——22 词 round-trip 用）。 */
function normalizeWord(word: RunStepKind): ChangeStepKind {
  const entry = NORMALIZE_EXPECTED.find(([snake]) => snake === word);
  if (!entry) {
    throw new Error(`词汇表缺项: ${word}`);
  }
  return entry[1];
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot：live 源标注与步级四字段透传（AC-1 / AC-8）
// ---------------------------------------------------------------------------

describe('unifiedRunNodes：live 源标注（AC-8 双源半边）', () => {
  it('库读史 runs[].nodes 携四字段 → 步行四字段透传 + live=false（stepFromRecord 纯复制半程锚）', () => {
    const runs = [
      runEntry({
        runId: 'run-his',
        nodes: [
          runStepRow({
            step: 'executor',
            sessionId: 'ses-his-1',
            startedAt: 1_727_000_000_000,
            endedAt: 1_727_000_060_000,
            tokenInput: 12_000,
            tokenOutput: 3_400,
          }),
        ],
      }),
    ];
    const rows = unifiedRunNodes(runs, null);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      step: 'executor',
      startedAt: 1_727_000_000_000,
      endedAt: 1_727_000_060_000,
      tokenInput: 12_000,
      tokenOutput: 3_400,
      runId: 'run-his',
      live: false,
    });
  });

  it('liveRun.nodes 携四字段 → 透传 + live=true（AC-8 活面标注半边）', () => {
    const rows = unifiedRunNodes(
      [],
      liveFace({
        runId: 'run-live',
        nodes: [
          stepRow({
            runId: 'run-live',
            status: 'running',
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: 500,
            tokenOutput: 60,
          }),
        ],
      }),
    );
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      startedAt: 1_727_000_000_000,
      endedAt: null,
      tokenInput: 500,
      tokenOutput: 60,
      runId: 'run-live',
      live: true,
    });
  });

  it('liveRun=null → 仅库读史步、全部 live=false（既有回归持衡）', () => {
    const runs = [
      runEntry({
        nodes: [
          runStepRow({ step: 'executor' }),
          runStepRow({ step: 'static_check', sessionId: null }),
        ],
      }),
    ];
    const rows = unifiedRunNodes(runs, null);
    expect(rows).toHaveLength(2);
    expect(rows.every((row) => row.live === false)).toBe(true);
  });

  it('双源同 (phase, attempt, step, instanceNo)（跨 run 同名身份键）→ 两行并存、live 标注各随其源不串扰', () => {
    const runs = [
      runEntry({
        runId: 'run-old',
        nodes: [
          runStepRow({ phase: 'test-gen', attempt: 1, step: 'phase_start', sessionId: null }),
        ],
      }),
    ];
    const rows = unifiedRunNodes(
      runs,
      liveFace({
        runId: 'run-new',
        nodes: [
          stepRow({
            runId: 'run-new',
            phase: 'test-gen',
            attempt: 1,
            step: 'phaseStart',
            sessionId: null,
          }),
        ],
      }),
    );
    expect(rows).toHaveLength(2);
    expect(rows.map((row) => row.live)).toEqual([false, true]);
    expect(rows.map((row) => row.runId)).toEqual(['run-old', 'run-new']);
    // 四字段各随其源（库读史行 vs 活面行默认四字段互不串写）
    expect(rows[0].startedAt).toBe(runStepRow({}).startedAt);
    expect(rows[1].live).toBe(true);
  });

  it('库读史旧行形态（startedAt=0 / endedAt=null / token null）→ 原样透传零改写', () => {
    const runs = [
      runEntry({
        nodes: [
          runStepRow({
            step: 'static_check',
            sessionId: null,
            startedAt: 0,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
          }),
        ],
      }),
    ];
    const rows = unifiedRunNodes(runs, null);
    expect(rows[0]).toMatchObject({
      startedAt: 0,
      endedAt: null,
      tokenInput: null,
      tokenOutput: null,
      live: false,
    });
  });

  it('runs 空数组 + liveRun=null → 空数组（不产生幽灵步）', () => {
    expect(unifiedRunNodes([], null)).toEqual([]);
  });
});

describe('runStepNodes：四字段 + live 五字段透传（RuntimeFlowNode）', () => {
  it('UnifiedRunStep 携四字段 + live → RuntimeFlowNode 五字段逐字段等值', () => {
    const steps = [
      stepRow({
        step: 'executor',
        status: 'passed',
        startedAt: 1_727_000_000_000,
        endedAt: 1_727_000_060_000,
        tokenInput: 12_000,
        tokenOutput: 3_400,
        live: false,
      }),
    ];
    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    expect(nodes[0]).toMatchObject({
      startedAt: 1_727_000_000_000,
      endedAt: 1_727_000_060_000,
      tokenInput: 12_000,
      tokenOutput: 3_400,
      live: false,
    });
  });

  it('多步混排（workerAgent / toolStep / gate）→ 各节点五字段各自透传、输出序保持输入序', () => {
    const steps = [
      stepRow({
        step: 'executor',
        tokenInput: 12_000,
        tokenOutput: 3_400,
        endedAt: 1_727_000_060_000,
      }),
      stepRow({
        step: 'staticCheck',
        sessionId: null,
        tokenInput: null,
        tokenOutput: null,
        endedAt: 1_727_000_030_000,
        live: false,
      }),
      stepRow({
        step: 'verdictGate',
        sessionId: null,
        tokenInput: null,
        tokenOutput: null,
        endedAt: 1_727_000_090_000,
        live: true,
      }),
    ];
    const nodes = runStepNodes(steps, REQUIREMENT_PHASES);
    expect(nodes.map((node) => node.runStepKind)).toEqual([
      'executor',
      'staticCheck',
      'verdictGate',
    ]);
    expect(nodes.map((node) => node.tokenInput ?? 'null')).toEqual([12_000, 'null', 'null']);
    expect(nodes.map((node) => node.live)).toEqual([true, false, true]);
    expect(nodes.map((node) => node.endedAt)).toEqual([
      1_727_000_060_000, 1_727_000_030_000, 1_727_000_090_000,
    ]);
  });

  it('四字段 0 / null 边界值形态 → 原样进节点零转换零默认', () => {
    const nodes = runStepNodes(
      [
        stepRow({
          step: 'phaseStart',
          sessionId: null,
          status: 'passed',
          startedAt: 0,
          endedAt: 0,
          tokenInput: null,
          tokenOutput: null,
          live: false,
        }),
      ],
      REQUIREMENT_PHASES,
    );
    expect(nodes[0].startedAt).toBe(0);
    expect(nodes[0].endedAt).toBe(0);
    expect(nodes[0].tokenInput).toBeNull();
    expect(nodes[0].tokenOutput).toBeNull();
    expect(nodes[0].live).toBe(false);
  });

  it('phaseOrder 外相位步 → 不入图（坑位校验回归，四字段不改变出局判定）', () => {
    const nodes = runStepNodes(
      [
        stepRow({
          phase: 'bootstrap',
          step: 'executor',
          startedAt: 1_727_000_000_000,
          endedAt: 1_727_000_060_000,
          tokenInput: 1,
          tokenOutput: 2,
        }),
        stepRow(),
      ],
      REQUIREMENT_PHASES,
    );
    expect(nodes).toHaveLength(1);
    expect(nodes[0].id).toBe('run:run-1727:implement:1:executor:0');
  });

  it('载荷含非法 status cast（沿既有 weirdStatus 先例）→ 透传不炸（四字段随行原样）', () => {
    const broken = {
      ...stepRow({ startedAt: 1_727_000_000_000, tokenInput: 7, tokenOutput: 8 }),
      status: 'weirdStatus' as unknown as UnifiedRunStep['status'],
    };
    const nodes = runStepNodes([broken], REQUIREMENT_PHASES);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].status).toBe('weirdStatus' as unknown as UnifiedRunStep['status']);
    expect(nodes[0].tokenInput).toBe(7);
    expect(nodes[0].tokenOutput).toBe(8);
  });
});

describe('stepFromRecord → unifiedRunNodes → runStepNodes 透传链（AC-1 / AC-8 全程锚）', () => {
  it('库读史 ChangeRunNodeRecord 四字段 → 最终 RuntimeFlowNode 逐字段一致', () => {
    const record = runStepRow({
      step: 'executor',
      sessionId: 'ses-chain-1',
      status: 'passed',
      detail: '实现完成',
      startedAt: 1_727_000_000_000,
      endedAt: 1_727_000_060_000,
      tokenInput: 12_000,
      tokenOutput: 3_400,
    });
    const runs = [runEntry({ runId: 'run-chain', nodes: [record] })];
    const nodes = runStepNodes(unifiedRunNodes(runs, null), REQUIREMENT_PHASES);
    expect(nodes).toHaveLength(1);
    expect(nodes[0]).toMatchObject({
      runId: 'run-chain',
      runStepKind: 'executor',
      status: 'passed',
      sessionId: 'ses-chain-1',
      startedAt: record.startedAt,
      endedAt: record.endedAt,
      tokenInput: record.tokenInput,
      tokenOutput: record.tokenOutput,
      live: false,
    });
  });
});

describe('run-state：clean 基线恢复步归一与身份键（check-before-flow-start）', () => {
  it("normalizeKind('clean') → 'clean'：identity 词走 default 分支直返（snake / camel 同形零显式 case——与 commit / rebase / merge / finalize / preflight / seal / cleanup 先例一致）", () => {
    const unified = unifiedRunNodes(
      [runEntry({ nodes: [runStepRow({ phase: 'initialize', step: 'clean', sessionId: null })] })],
      null,
    );
    expect(unified.map((row) => row.step)).toEqual(['clean']);
  });

  it('clean 节点身份键含 stepWord 词面：{phase}:{attempt}:clean:{instanceNo}（与后端边表端点逐字可比）', () => {
    const node = runStepNodes(
      [stepRow({ phase: 'initialize', step: 'clean', sessionId: null, instanceNo: 0 })],
      REQUIREMENT_PHASES,
    )[0];
    expect(nodeIdentityKey(node)).toBe('initialize:1:clean:0');
  });

  it("groupOf('clean') 归 git 分类：经 runStepNodes 分组出线（与 commit / rebase / merge / finalize / cleanup 同组——AC-8 观测半边）", () => {
    const node = runStepNodes(
      [stepRow({ phase: 'initialize', step: 'clean', sessionId: null })],
      REQUIREMENT_PHASES,
    )[0];
    expect(node.group).toBe('git');
    expect(node.role).toBeNull();
  });

  it('库读史行 step=clean（RunStepKind 线词）→ 统一视图节点归一出线（双源合成对新词透明）', () => {
    const steps = [
      runStepRow({ instanceNo: 0, phase: 'initialize', step: 'phase_start', sessionId: null }),
      runStepRow({ instanceNo: 0, phase: 'initialize', step: 'clean', sessionId: null }),
      runStepRow({ instanceNo: 0, phase: 'initialize', step: 'static_check', sessionId: null }),
    ];
    const unified = unifiedRunNodes([runEntry({ nodes: steps })], null);
    expect(unified.map((row) => row.step)).toEqual(['phaseStart', 'clean', 'staticCheck']);
    expect(unified[1].sessionId).toBeNull();
    const nodes = runStepNodes(unified, REQUIREMENT_PHASES);
    expect(nodes.map((node) => node.group)).toEqual(['toolStep', 'git', 'toolStep']);
  });

  it('未知步词 default 回落不崩持衡（归一面不因扩词松动——回归锚）', () => {
    // clean 扩词后未知词仍走 default 分支（越集词不入 CHANGE_STEP_KINDS 联合——
    // 类型面拒绝；运行时未知串与既有越集词同样落 default 归一）
    const unified = unifiedRunNodes(
      [
        runEntry({
          nodes: [
            runStepRow({ phase: 'x', step: 'clear' as unknown as RunStepKind, sessionId: null }),
          ],
        }),
      ],
      null,
    );
    expect(unified.map((row) => row.step)).toEqual(['clear']);
  });
});

describe('run-state：archi_check 架构门禁步归一与身份键（AC-16）', () => {
  it("normalizeKind('archi_check') → 'archiCheck'：库读史 snake 词经统一视图归一出线（snake 词不入活面词汇）", () => {
    const unified = unifiedRunNodes(
      [
        runEntry({
          nodes: [runStepRow({ instanceNo: 0, step: 'archi_check', sessionId: null })],
        }),
      ],
      null,
    );
    expect(unified.map((row) => row.step)).toEqual(['archiCheck']);
    // 工具步零会话（库读史 sessionId = null 原样透传）
    expect(unified[0].sessionId).toBeNull();
  });

  it('archi_check 节点身份键含 stepWord 词面：{phase}:{attempt}:archi_check:{instanceNo}（与后端边表端点逐字可比）', () => {
    const node = runStepNodes(
      [
        stepRow({
          phase: 'implement',
          attempt: 1,
          step: 'archiCheck',
          sessionId: null,
          instanceNo: 0,
        }),
      ],
      REQUIREMENT_PHASES,
    )[0];
    expect(node).toBeDefined();
    expect(nodeIdentityKey(node)).toBe('implement:1:archi_check:0');
    // 邻接步的身份键同样可对（archi_check 与 static_check 在同一 attempt 内相邻）
    const neighbour = runStepNodes(
      [stepRow({ step: 'staticCheck', sessionId: null, instanceNo: 0 })],
      REQUIREMENT_PHASES,
    )[0];
    expect(nodeIdentityKey(neighbour)).toBe('implement:1:static_check:0');
  });

  it("groupOf('archiCheck') 归 toolStep：经 runStepNodes 的 default 分支出线（与 staticCheck / mutationTest 同组），role 恒 null", () => {
    const node = runStepNodes(
      [stepRow({ phase: 'implement', step: 'archiCheck', sessionId: null })],
      REQUIREMENT_PHASES,
    )[0];
    expect(node.group).toBe('toolStep');
    expect(node.role).toBeNull();
  });

  it('词汇外步词退化不崩：越集词走 default 归一且分组回落 toolStep（稳健面不因扩词松动）', () => {
    for (const illegal of ['ARCHI_CHECK', 'archicheck', '']) {
      const unified = unifiedRunNodes(
        [
          runEntry({
            nodes: [
              runStepRow({
                instanceNo: 0,
                step: illegal as unknown as RunStepKind,
                sessionId: null,
              }),
            ],
          }),
        ],
        null,
      );
      // 越集词原值透传（不伪造 archiCheck 词）
      expect(unified.map((row) => row.step)).toEqual([illegal]);
      const node = runStepNodes(unified, REQUIREMENT_PHASES)[0];
      expect(node.group).toBe('toolStep');
      expect(node.role).toBeNull();
    }
  });
});

describe('run-state：畸形步记录防御（旧报告 / 反序列化残留）', () => {
  it('step 缺失 / null / 非字符串 → 不抛错、原值透传、group 回落 toolStep、身份键仍可构造（不伪造 archiCheck 词）', () => {
    const broken: Array<unknown> = [undefined, null, 42, true];
    for (const step of broken) {
      const record = {
        ...runStepRow({ phase: 'implement', attempt: 1, instanceNo: 3 }),
        step,
      } as unknown as ChangeRunNodeRecord;
      // 库读史行 → 统一视图：原值透传（不归一、不伪造合法词）
      const unified = unifiedRunNodes([runEntry({ nodes: [record] })], null);
      expect(unified).toHaveLength(1);
      expect(unified[0].step).toBe(step);
      // 出图不抛错：分组回落 toolStep、身份键仍可构造
      const node = runStepNodes(unified, REQUIREMENT_PHASES)[0];
      expect(node).toBeDefined();
      expect(node.runStepKind).not.toBe('archiCheck');
      expect(node.group).toBe('toolStep');
      expect(node.role).toBeNull();
      expect(nodeIdentityKey(node)).toBe(`implement:1:${String(step)}:3`);
    }
  });

  it('instanceNo 缺失 → 身份键退化不崩（既有稳健面），步词仍按归一单点出线', () => {
    const record = {
      ...runStepRow({ phase: 'implement', attempt: 1, step: 'archi_check' }),
      instanceNo: undefined,
    } as unknown as ChangeRunNodeRecord;
    const unified = unifiedRunNodes([runEntry({ nodes: [record] })], null);
    const node = runStepNodes(unified, REQUIREMENT_PHASES)[0];
    expect(node.runStepKind).toBe('archiCheck');
    expect(nodeIdentityKey(node)).toBe('implement:1:archi_check:undefined');
  });
});
