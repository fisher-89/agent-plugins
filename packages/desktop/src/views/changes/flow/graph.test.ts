import { describe, expect, it } from 'vite-plus/test';

import type { AttemptRecord, ChangeDetail, PhaseEntry } from '../../../types/dto';
import { buildFlowGraph } from './graph';
import type { UnifiedRunEdge } from './run-state';
import type { RuntimeFlowNode } from './types';

/**
 * requirement 相位序夹具（D12：前端 `PIPELINE_PHASES` 常量退役，列序改由
 * `detail.pipeline` 出线承载，测试各自内联 fixture）：9 列，archive 为第 9 列。
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
// buildFlowGraph 单测（change-flow-show-step-only，AC-1 / D1 / D5）：节点模型
// 单分类——**仅 runtime step 节点**（`sequence = [...runNodes]`，eval / active
// 相位节点整体退役）；纯函数直测（无 mock）：真实组合 layout（REQUIREMENT_PHASES）
// 与 run-state（nodeIdentityKey）。
// ---------------------------------------------------------------------------

/** 以宽松默认值构造单条 AttemptRecord，便于逐字段控制分支形态。 */
function attempt(overrides: Partial<AttemptRecord> = {}): AttemptRecord {
  return {
    attempt: 1,
    verdict: 'pass',
    report: '评估记录',
    checklist: [],
    skipped: false,
    stale: false,
    startAt: null,
    timestamp: null,
    backtrackTo: null,
    backtrackReason: null,
    executorSessionId: null,
    evaluatorSessionId: null,
    decisionSessionId: null,
    ...overrides,
  };
}

function station(phase: string, attempts: AttemptRecord[]): PhaseEntry {
  return { phase, attempts };
}

/** 建档详情底座：9 站全空 attempts（requirement 相位表序），逐用例按需注入事件。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: '2026-09-01',
    pipeline: REQUIREMENT_PHASES.map((phase) => station(phase, [])),
    activePhase: null,
    runs: [],
    worktree: null,
    artifacts: [],
    ...overrides,
  };
}

/** 多 attempt + activePhase 在场的详情（eval / active 退役前的「非空节点源」形态）。 */
function richDetail(): ChangeDetail {
  return detail({
    pipeline: REQUIREMENT_PHASES.map((phase) => {
      if (phase === 'proposal') return station(phase, [attempt({ attempt: 1 })]);
      if (phase === 'dev-design') {
        return station(phase, [attempt({ attempt: 1 }), attempt({ attempt: 2 })]);
      }
      return station(phase, []);
    }),
    activePhase: { phase: 'implement', attempt: 3, startAt: '2026-09-04T00:00:00Z' },
  });
}

/** 运行步节点 fixture（run-state.ts runStepNodes 同形投影的直构形态）。 */
function runNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return {
    id: 'run:run-1727:implement:1:executor:0',
    kind: 'runtime',
    phase: 'implement',
    attempt: 1,
    colIndex: REQUIREMENT_PHASES.indexOf('implement'),
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
    ...overrides,
  };
}

/** 边行 fixture（统一视图边行；端点恒为节点身份串
 * `"{phase}:{attempt}:{step}:{instanceNo}"`——`step` 为 snake 线词，归属 run id
 * 随行）。 */
function edgeRow(overrides: Partial<UnifiedRunEdge> = {}): UnifiedRunEdge {
  return {
    fromNodeKey: 'implement:1:phase_start:0',
    toNodeKey: 'implement:1:executor:0',
    edgeKind: 'forward',
    runId: 'run-1727',
    ...overrides,
  };
}

function nodeIds(nodes: { id: string }[]): string[] {
  return nodes.map((node) => node.id);
}

describe('buildFlowGraph：列骨架与空图', () => {
  it('pipeline 非空 → 列序与列数取自 detail.pipeline（id / phase / colIndex 与出线序同序）', () => {
    const graph = buildFlowGraph(detail());
    expect(graph.columns).toHaveLength(REQUIREMENT_PHASES.length);
    expect(graph.columns.map((column) => column.id)).toEqual(
      REQUIREMENT_PHASES.map((phase) => `col:${phase}`),
    );
    expect(graph.columns.map((column) => column.phase)).toEqual([...REQUIREMENT_PHASES]);
    expect(graph.columns.map((column) => column.colIndex)).toEqual(
      REQUIREMENT_PHASES.map((_, index) => index),
    );
  });

  it('pipeline 为空数组（v0 形态）→ 返回空图：columns / nodes / edges 全空', () => {
    expect(buildFlowGraph(detail({ pipeline: [] }))).toEqual({
      columns: [],
      nodes: [],
      edges: [],
    });
  });

  it('多 attempt + activePhase 非空 + runNodes 缺省 → 零节点零边（eval / active 不再上图，仅空列）', () => {
    const graph = buildFlowGraph(richDetail());
    expect(graph.columns).toHaveLength(REQUIREMENT_PHASES.length);
    expect(graph.nodes).toEqual([]);
    expect(graph.edges).toEqual([]);
    // 各列容器均无节点归属（未走的站呈现空列）
    expect(graph.columns.filter((column) => column.id.startsWith('col:'))).toHaveLength(
      REQUIREMENT_PHASES.length,
    );
  });
});

describe('buildFlowGraph：单分类 runtime 节点（runNodes 直录）', () => {
  it('传入 runNodes → nodes 恰为该 runNodes 原序（sequence = [...runNodes]），kind 恒 runtime', () => {
    const nodes = [
      runNode({
        id: 'run:implement:1:phaseStart:0',
        runStepKind: 'phaseStart',
        group: 'toolStep',
        role: null,
      }),
      runNode({ id: 'run:implement:1:executor:1', instanceNo: 1 }),
      runNode({
        id: 'run:implement:1:staticCheck:0',
        runStepKind: 'staticCheck',
        group: 'toolStep',
        role: null,
      }),
    ];
    const graph = buildFlowGraph(richDetail(), nodes);
    expect(nodeIds(graph.nodes)).toEqual(nodeIds(nodes));
    expect(graph.nodes.map((node) => node.kind)).toEqual(['runtime', 'runtime', 'runtime']);
    // 节点载荷原样承接（单分类无归并改写）
    expect(graph.nodes[1].runStepKind).toBe('executor');
    expect(graph.nodes[1].instanceNo).toBe(1);
    expect(graph.nodes[0].group).toBe('toolStep');
  });

  it('assignOrders 按 phase 重编 order：同相自 0 起递增、跨相各自独立', () => {
    const graph = buildFlowGraph(detail(), [
      runNode({ id: 'run:implement:1:executor:0', order: 7 }),
      runNode({
        id: 'run:proposal:1:evaluator:0',
        phase: 'proposal',
        attempt: 1,
        colIndex: REQUIREMENT_PHASES.indexOf('proposal'),
        parentId: 'col:proposal',
        runStepKind: 'evaluator',
        role: 'evaluator',
        order: 9,
      }),
      runNode({
        id: 'run:implement:1:evaluator:0',
        runStepKind: 'evaluator',
        role: 'evaluator',
        order: 3,
      }),
    ]);
    // 传入 order 被重编：implement 首枚 0、proposal 首枚 0、implement 次枚 1
    expect(graph.nodes.map((node) => [node.phase, node.order])).toEqual([
      ['implement', 0],
      ['proposal', 0],
      ['implement', 1],
    ]);
  });

  it('同键多实例（instanceNo 递增）→ 各成一节点且身份键唯一不串扰', () => {
    const graph = buildFlowGraph(detail(), [
      runNode({ id: 'run:implement:1:executor:0', instanceNo: 0 }),
      runNode({ id: 'run:implement:1:executor:1', instanceNo: 1 }),
      runNode({ id: 'run:implement:1:executor:2', instanceNo: 2 }),
    ]);
    expect(graph.nodes).toHaveLength(3);
    expect(new Set(nodeIds(graph.nodes)).size).toBe(3);
    expect(graph.nodes.map((node) => node.instanceNo)).toEqual([0, 1, 2]);
    // 同相列内 order 逐枚递增（列内归并序不重叠）
    expect(graph.nodes.map((node) => node.order)).toEqual([0, 1, 2]);
  });

  it('超长列表（全史叠加多 attempt）→ 顺序保持、零去重丢失', () => {
    const long: RuntimeFlowNode[] = [];
    for (let attemptNo = 1; attemptNo <= 40; attemptNo += 1) {
      long.push(
        runNode({
          id: `run:implement:${attemptNo}:executor:0`,
          attempt: attemptNo,
          instanceNo: 0,
        }),
      );
      long.push(
        runNode({
          id: `run:implement:${attemptNo}:staticCheck:${attemptNo}`,
          attempt: attemptNo,
          instanceNo: attemptNo,
          runStepKind: 'staticCheck',
          group: 'toolStep',
          role: null,
        }),
      );
    }
    const graph = buildFlowGraph(detail(), long);
    expect(graph.nodes).toHaveLength(80);
    expect(nodeIds(graph.nodes)).toEqual(nodeIds(long));
    expect(new Set(nodeIds(graph.nodes)).size).toBe(80);
  });

  it('边界：runNodes 为 undefined / 空数组 → 与缺省同形（零节点零边，不抛错）', () => {
    const base = detail();
    expect(buildFlowGraph(base).nodes).toEqual([]);
    expect(buildFlowGraph(base, []).nodes).toEqual([]);
    expect(buildFlowGraph(base).columns).toHaveLength(REQUIREMENT_PHASES.length);
  });
});

describe('buildFlowGraph：边表直读（runEdges）', () => {
  it('未供 runEdges → 零边（不臆造集合间边）', () => {
    const base = detail();
    expect(buildFlowGraph(base, [runNode()]).edges).toEqual([]);
    expect(buildFlowGraph(base, [runNode()], []).edges).toEqual([]);
  });

  it('图边投影锚：输入含 retry 边（存量库行）→ 输出图零 retry，其余三类按输入序出线', () => {
    const graph = buildFlowGraph(
      detail(),
      [
        runNode({
          id: 'run:implement:1:phaseStart:0',
          runStepKind: 'phaseStart',
          group: 'toolStep',
          role: null,
        }),
        runNode({ id: 'run:implement:1:executor:0' }),
        runNode({
          id: 'run:implement:1:evaluator:0',
          runStepKind: 'evaluator',
          role: 'evaluator',
        }),
        runNode({
          id: 'run:implement:1:verdictGate:0',
          runStepKind: 'verdictGate',
          group: 'gate',
          role: null,
        }),
      ],
      [
        edgeRow({
          fromNodeKey: 'implement:1:phase_start:0',
          toNodeKey: 'implement:1:executor:0',
          edgeKind: 'forward',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:evaluator:0',
          toNodeKey: 'implement:1:executor:0',
          edgeKind: 'feedback',
        }),
        // 存量 `retry` 行（读面投影前的旧库行）：端点不在节点集 → 跳过
        edgeRow({
          fromNodeKey: 'implement:1:verdict_gate:0',
          toNodeKey: 'ghost:1:executor:0',
          edgeKind: 'retry',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:verdict_gate:0',
          toNodeKey: 'implement:1:executor:0',
          edgeKind: 'backtrack',
        }),
      ],
    );
    expect(graph.edges.map((edge) => edge.kind)).toEqual(['forward', 'feedback', 'backtrack']);
    expect(graph.edges.some((edge) => edge.kind === 'retry')).toBe(false);
    expect(graph.edges[0]).toMatchObject({
      id: 'edge:0:run:implement:1:phaseStart:0->run:implement:1:executor:0',
      source: 'run:implement:1:phaseStart:0',
      target: 'run:implement:1:executor:0',
    });
    // 序号驻输入序：跳边不使后续边序号前移（backtrack 原输入下标 3）
    expect(graph.edges[2].id).toBe(
      'edge:3:run:implement:1:verdictGate:0->run:implement:1:executor:0',
    );
    // 非回跳边恒无标签（label 词汇仅回跳边承载）
    expect(graph.edges.slice(0, 2).map((edge) => edge.label)).toEqual([null, null]);
  });

  it('backtrack 边 label 取发起相位最近一次条目记因（pipeline 参数保留生效）', () => {
    const base = detail({
      pipeline: REQUIREMENT_PHASES.map((phase) =>
        phase === 'implement'
          ? station(phase, [
              attempt({ attempt: 1 }),
              attempt({ attempt: 2, backtrackTo: 'test-design', backtrackReason: '实现缺陷回改' }),
            ])
          : station(phase, []),
      ),
    });
    const graph = buildFlowGraph(
      base,
      [
        runNode({ id: 'run:implement:2:executor:0', attempt: 2 }),
        runNode({
          id: 'run:implement:2:whitelistGate:0',
          attempt: 2,
          runStepKind: 'whitelistGate',
          group: 'gate',
          role: null,
        }),
      ],
      [
        edgeRow({
          fromNodeKey: 'implement:2:whitelist_gate:0',
          toNodeKey: 'implement:2:executor:0',
          edgeKind: 'backtrack',
        }),
      ],
    );
    expect(graph.edges).toHaveLength(1);
    expect(graph.edges[0].label).toBe('实现缺陷回改');
  });

  it('发起相位无记因（backtrackReason 全 null）→ 回跳边 label 为 null（不渲染字符串 "null"）', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: REQUIREMENT_PHASES.map((phase) =>
          phase === 'implement' ? station(phase, [attempt({ attempt: 1 })]) : station(phase, []),
        ),
      }),
      [runNode({ id: 'run:implement:1:executor:0' })],
      [
        edgeRow({
          fromNodeKey: 'implement:1:executor:0',
          toNodeKey: 'implement:1:executor:0',
          edgeKind: 'backtrack',
        }),
      ],
    );
    expect(graph.edges[0].label).toBeNull();
  });

  it('端点悬空（ghost 身份串）→ 该边跳过不炸图，其余边照常出线', () => {
    const graph = buildFlowGraph(
      detail(),
      [runNode({ id: 'run:implement:1:executor:0' })],
      [
        edgeRow({
          fromNodeKey: 'ghost:9:executor:7',
          toNodeKey: 'implement:1:executor:0',
          edgeKind: 'feedback',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:executor:0',
          toNodeKey: 'ghost:9:phase_log:0',
          edgeKind: 'retry',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:executor:0',
          toNodeKey: 'implement:1:executor:0',
          edgeKind: 'forward',
        }),
      ],
    );
    expect(graph.edges).toHaveLength(1);
    expect(graph.edges[0].kind).toBe('forward');
  });

  it('异常：runEdges 端点全悬空 / runNodes 缺省仍携边表 → 不抛错，边收口为空', () => {
    expect(() =>
      buildFlowGraph(
        detail(),
        [runNode()],
        [edgeRow({ fromNodeKey: 'a:1:x:0', toNodeKey: 'b:1:y:0' })],
      ),
    ).not.toThrow();
    expect(
      buildFlowGraph(
        detail(),
        [runNode()],
        [edgeRow({ fromNodeKey: 'a:1:x:0', toNodeKey: 'b:1:y:0' })],
      ).edges,
    ).toEqual([]);
    // 节点缺省（零节点）而边表在场：全部悬空 → 零边（不炸）
    expect(buildFlowGraph(detail(), undefined, [edgeRow()]).edges).toEqual([]);
  });

  it('多实例 feedback 边端点各异不串扰（instanceNo 区分同键两实例）', () => {
    const graph = buildFlowGraph(
      detail(),
      [
        runNode({ id: 'run:implement:1:executor:0', instanceNo: 0 }),
        runNode({ id: 'run:implement:1:executor:1', instanceNo: 1 }),
      ],
      [
        edgeRow({
          fromNodeKey: 'implement:1:executor:0',
          toNodeKey: 'implement:1:executor:1',
          edgeKind: 'feedback',
        }),
      ],
    );
    expect(graph.edges).toHaveLength(1);
    expect(graph.edges[0].source).not.toBe(graph.edges[0].target);
    expect(graph.edges[0].source).toBe('run:implement:1:executor:0');
    expect(graph.edges[0].target).toBe('run:implement:1:executor:1');
  });

  it('重启重跑同 attempt：跨 run 同名身份串边按归属 run 锚定（不锚错他轮节点）', () => {
    const graph = buildFlowGraph(
      detail(),
      [
        runNode({
          id: 'run:run-1:test-gen:1:phaseStart:0',
          runId: 'run-1',
          phase: 'test-gen',
          runStepKind: 'phaseStart',
          group: 'toolStep',
          role: null,
          sessionId: null,
        }),
        runNode({
          id: 'run:run-1:test-gen:1:executor:0',
          runId: 'run-1',
          phase: 'test-gen',
          sessionId: 'ses-run-1',
          status: 'passed',
        }),
        runNode({
          id: 'run:run-2:test-gen:1:phaseStart:0',
          runId: 'run-2',
          phase: 'test-gen',
          runStepKind: 'phaseStart',
          group: 'toolStep',
          role: null,
          sessionId: null,
        }),
        runNode({
          id: 'run:run-2:test-gen:1:executor:0',
          runId: 'run-2',
          phase: 'test-gen',
          sessionId: 'ses-run-2',
          status: 'running',
        }),
      ],
      [
        edgeRow({
          runId: 'run-1',
          fromNodeKey: 'test-gen:1:phase_start:0',
          toNodeKey: 'test-gen:1:executor:0',
        }),
        edgeRow({
          runId: 'run-2',
          fromNodeKey: 'test-gen:1:phase_start:0',
          toNodeKey: 'test-gen:1:executor:0',
        }),
      ],
    );
    // 两轮 phaseStart 节点并存（重启节点 id 携 run 段互不顶替）
    expect(graph.nodes.map((node) => node.id)).toEqual([
      'run:run-1:test-gen:1:phaseStart:0',
      'run:run-1:test-gen:1:executor:0',
      'run:run-2:test-gen:1:phaseStart:0',
      'run:run-2:test-gen:1:executor:0',
    ]);
    // 边按归属 run 锚定：run-1 边落 run-1 节点、run-2 边落 run-2 节点（同身份串
    // 不跨轮错锚）
    expect(graph.edges.map((edge) => [edge.source, edge.target])).toEqual([
      ['run:run-1:test-gen:1:phaseStart:0', 'run:run-1:test-gen:1:executor:0'],
      ['run:run-2:test-gen:1:phaseStart:0', 'run:run-2:test-gen:1:executor:0'],
    ]);
  });
});

// ---------------------------------------------------------------------------
// 列骨架随 detail.pipeline 出线（desktop-test-only-flow AC-5 / AC-6）：两表各自
// 成立；pipeline 外相位零列（不臆造）
// ---------------------------------------------------------------------------

/** test-only 相位序夹具：6 列，code-analyze 第 2 列、archive 第 6 列。 */
const TEST_ONLY_PHASES = [
  'proposal',
  'code-analyze',
  'test-design',
  'test-gen',
  'test-execution',
  'archive',
];

/** test-only 详情底座：6 站（test-only 相位表序）全空 attempts。 */
function testOnlyDetail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return detail({
    pipeline: TEST_ONLY_PHASES.map((phase) => station(phase, [])),
    ...overrides,
  });
}

describe('buildFlowGraph：两表列骨架（requirement 9 / test-only 6）', () => {
  it('正向：test-only detail（6 相位 pipeline）→ 6 列，code-analyze colIndex=1、archive colIndex=5', () => {
    const graph = buildFlowGraph(testOnlyDetail());

    expect(graph.columns).toHaveLength(6);
    expect(graph.columns.map((column) => column.phase)).toEqual([...TEST_ONLY_PHASES]);
    expect(graph.columns.map((column) => column.id)).toEqual(
      TEST_ONLY_PHASES.map((phase) => `col:${phase}`),
    );
    expect(graph.columns.map((column) => column.colIndex)).toEqual([0, 1, 2, 3, 4, 5]);
    expect(graph.columns.find((column) => column.phase === 'code-analyze')?.colIndex).toBe(1);
    expect(graph.columns.find((column) => column.phase === 'archive')?.colIndex).toBe(5);
  });

  it('正向：test-only 列骨架零 requirement 独有相位（不越表臆造列）', () => {
    const graph = buildFlowGraph(testOnlyDetail());

    for (const phase of ['dev-design', 'implement', 'code-review', 'acceptance']) {
      expect(graph.columns.some((column) => column.phase === phase)).toBe(false);
    }
  });

  it('异常：pipeline 外相位的运行步节点 → 零列承载（列集合恒由 pipeline 决定）', () => {
    const graph = buildFlowGraph(testOnlyDetail(), [
      runNode({
        id: 'run:implement:1:executor:0',
        phase: 'implement',
        parentId: 'col:implement',
        colIndex: -1,
      }),
    ]);

    expect(graph.columns).toHaveLength(6);
    expect(graph.columns.some((column) => column.phase === 'implement')).toBe(false);
    // 节点原序直录（列集合不臆造归属列）
    expect(nodeIds(graph.nodes)).toEqual(['run:implement:1:executor:0']);
  });

  it('边界：两表列数各自稳定且 archive 恒为末列（requirement colIndex=8 / test-only colIndex=5）', () => {
    const requirementGraph = buildFlowGraph(detail());
    const requirementArchive = requirementGraph.columns.at(-1);
    expect(requirementGraph.columns).toHaveLength(9);
    expect(requirementArchive?.phase).toBe('archive');
    expect(requirementArchive?.colIndex).toBe(8);

    const testOnlyArchive = buildFlowGraph(testOnlyDetail()).columns.at(-1);
    expect(testOnlyArchive?.phase).toBe('archive');
    expect(testOnlyArchive?.colIndex).toBe(5);
  });

  it('边界：test-only pipeline 空数组 → 空图（与 requirement 同款空态语义）', () => {
    expect(buildFlowGraph(detail({ pipeline: [] }))).toEqual({ columns: [], nodes: [], edges: [] });
  });
});

// ---------------------------------------------------------------------------
// 图边投影锚（test-execution-zero-agent-decision AC-8 前端侧）：读面投影前
// 的存量库行（含 `retry`）进图后零 `retry` 出线——`graph.ts` 零改动，只锚
// 「悬空端点降级跳过」的既有结果契约。
// ---------------------------------------------------------------------------

describe('buildFlowGraph：retry 边投影锚', () => {
  const edgeNodes = () => [
    runNode({ id: 'run:implement:1:executor:0' }),
    runNode({
      id: 'run:implement:1:evaluator:0',
      runStepKind: 'evaluator',
      role: 'evaluator',
    }),
    runNode({
      id: 'run:implement:1:phaseLog:0',
      runStepKind: 'phaseLog',
      group: 'toolStep',
      role: null,
    }),
  ];

  it('retry 行位于输入首 / 中 / 尾三位置 → 输出仍零 retry，其余边相对序与输入序一致', () => {
    const shapes: UnifiedRunEdge[][] = [
      // 首
      [
        edgeRow({
          fromNodeKey: 'implement:1:evaluator:0',
          toNodeKey: 'ghost:1:executor:0',
          edgeKind: 'retry',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:executor:0',
          toNodeKey: 'implement:1:evaluator:0',
          edgeKind: 'forward',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:evaluator:0',
          toNodeKey: 'implement:1:phase_log:0',
          edgeKind: 'feedback',
        }),
      ],
      // 中
      [
        edgeRow({
          fromNodeKey: 'implement:1:executor:0',
          toNodeKey: 'implement:1:evaluator:0',
          edgeKind: 'forward',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:evaluator:0',
          toNodeKey: 'ghost:1:executor:0',
          edgeKind: 'retry',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:evaluator:0',
          toNodeKey: 'implement:1:phase_log:0',
          edgeKind: 'feedback',
        }),
      ],
      // 尾
      [
        edgeRow({
          fromNodeKey: 'implement:1:executor:0',
          toNodeKey: 'implement:1:evaluator:0',
          edgeKind: 'forward',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:evaluator:0',
          toNodeKey: 'implement:1:phase_log:0',
          edgeKind: 'feedback',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:evaluator:0',
          toNodeKey: 'ghost:1:executor:0',
          edgeKind: 'retry',
        }),
      ],
    ];
    const expectedKinds = [
      ['forward', 'feedback'],
      ['forward', 'feedback'],
      ['forward', 'feedback'],
    ];
    shapes.forEach((edges, position) => {
      const graph = buildFlowGraph(detail(), edgeNodes(), edges);
      expect(graph.edges.map((edge) => edge.kind)).toEqual(expectedKinds[position]);
      expect(graph.edges.some((edge) => edge.kind === 'retry')).toBe(false);
      // 序号驻输入序：跳边不使其余边序号错位到别的 kind 上
      const expectedIndices = edges
        .map((edge, index) => ({ edge, index }))
        .filter(({ edge }) => edge.edgeKind !== 'retry')
        .map(({ index }) => index);
      expect(graph.edges.map((edge) => Number(edge.id.split(':')[1]))).toEqual(expectedIndices);
    });
  });

  it('输入边表仅 retry 行（端点全悬空）→ edges 空数组不抛错；输入空边表 → 空数组', () => {
    const onlyRetry = buildFlowGraph(detail(), edgeNodes(), [
      edgeRow({
        fromNodeKey: 'ghost:1:executor:0',
        toNodeKey: 'ghost:1:evaluator:0',
        edgeKind: 'retry',
      }),
    ]);
    expect(onlyRetry.edges).toEqual([]);

    const emptyTable = buildFlowGraph(detail(), edgeNodes(), []);
    expect(emptyTable.edges).toEqual([]);
  });

  it('retry 行一端悬空（ghost 身份串）→ 该边跳过不炸图，同批 feedback / forward 边照常出线', () => {
    const graph = buildFlowGraph(detail(), edgeNodes(), [
      edgeRow({
        fromNodeKey: 'ghost:9:executor:7',
        toNodeKey: 'implement:1:executor:0',
        edgeKind: 'feedback',
      }),
      edgeRow({
        fromNodeKey: 'implement:1:executor:0',
        toNodeKey: 'ghost:9:phase_log:0',
        edgeKind: 'retry',
      }),
      edgeRow({
        fromNodeKey: 'implement:1:executor:0',
        toNodeKey: 'implement:1:evaluator:0',
        edgeKind: 'forward',
      }),
    ]);
    expect(graph.edges).toHaveLength(1);
    expect(graph.edges[0].kind).toBe('forward');
  });
});
