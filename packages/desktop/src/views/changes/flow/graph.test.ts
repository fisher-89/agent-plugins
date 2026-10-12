import { describe, expect, it } from 'vite-plus/test';

import type {
  AttemptRecord,
  ChangeDetail,
  ChangeRunEdgeRecord,
  PhaseEntry,
} from '../../../types/dto';
import { buildFlowGraph } from './graph';
import { PIPELINE_PHASES } from './layout';
import type { RuntimeFlowNode } from './types';

// ---------------------------------------------------------------------------
// buildFlowGraph 单测（change-flow-show-step-only，AC-1 / D1 / D5）：节点模型
// 单分类——**仅 runtime step 节点**（`sequence = [...runNodes]`，eval / active
// 相位节点整体退役）；纯函数直测（无 mock）：真实组合 layout（PIPELINE_PHASES）
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

/** 建档详情底座：10 站全空 attempts，逐用例按需注入事件。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: '2026-09-01',
    pipeline: PIPELINE_PHASES.map((phase) => station(phase, [])),
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
    pipeline: PIPELINE_PHASES.map((phase) => {
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
    id: 'run:implement:1:executor:0',
    kind: 'runtime',
    phase: 'implement',
    attempt: 1,
    colIndex: PIPELINE_PHASES.indexOf('implement'),
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
    ...overrides,
  };
}

/** 边行 fixture（后端边表条目；端点恒为节点身份串
 * `"{phase}:{attempt}:{step}:{instanceNo}"`——`step` 为 snake 线词）。 */
function edgeRow(overrides: Partial<ChangeRunEdgeRecord> = {}): ChangeRunEdgeRecord {
  return {
    fromNodeKey: 'implement:1:phase_start:0',
    toNodeKey: 'implement:1:executor:0',
    edgeKind: 'forward',
    ...overrides,
  };
}

function nodeIds(nodes: { id: string }[]): string[] {
  return nodes.map((node) => node.id);
}

describe('buildFlowGraph：列骨架与空图', () => {
  it('pipeline 非空 → 恒 10 列容器（id / phase / colIndex 与 PIPELINE_PHASES 同序）', () => {
    const graph = buildFlowGraph(detail());
    expect(graph.columns).toHaveLength(10);
    expect(graph.columns.map((column) => column.id)).toEqual(
      PIPELINE_PHASES.map((phase) => `col:${phase}`),
    );
    expect(graph.columns.map((column) => column.phase)).toEqual([...PIPELINE_PHASES]);
    expect(graph.columns.map((column) => column.colIndex)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
  });

  it('pipeline 为空数组（v0 形态）→ 返回空图：columns / nodes / edges 全空', () => {
    expect(buildFlowGraph(detail({ pipeline: [] }))).toEqual({
      columns: [],
      nodes: [],
      edges: [],
    });
  });

  it('多 attempt + activePhase 非空 + runNodes 缺省 → 零节点零边（eval / active 不再上图，仅 10 空列）', () => {
    const graph = buildFlowGraph(richDetail());
    expect(graph.columns).toHaveLength(10);
    expect(graph.nodes).toEqual([]);
    expect(graph.edges).toEqual([]);
    // 10 列容器均无节点归属（未走的站呈现空列）
    expect(graph.columns.filter((column) => column.id.startsWith('col:'))).toHaveLength(10);
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
        colIndex: PIPELINE_PHASES.indexOf('proposal'),
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
    expect(buildFlowGraph(base).columns).toHaveLength(10);
  });
});

describe('buildFlowGraph：边表直读（runEdges）', () => {
  it('未供 runEdges → 零边（不臆造集合间边）', () => {
    const base = detail();
    expect(buildFlowGraph(base, [runNode()]).edges).toEqual([]);
    expect(buildFlowGraph(base, [runNode()], []).edges).toEqual([]);
  });

  it('四 kind 逐值出线：端点按节点 id 寻址、id 定式 `edge:<idx>:<source>-><target>`', () => {
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
        edgeRow({
          fromNodeKey: 'implement:1:verdict_gate:0',
          toNodeKey: 'implement:1:evaluator:0',
          edgeKind: 'retry',
        }),
        edgeRow({
          fromNodeKey: 'implement:1:verdict_gate:0',
          toNodeKey: 'implement:1:executor:0',
          edgeKind: 'backtrack',
        }),
      ],
    );
    expect(graph.edges.map((edge) => edge.kind)).toEqual([
      'forward',
      'feedback',
      'retry',
      'backtrack',
    ]);
    expect(graph.edges[0]).toMatchObject({
      id: 'edge:0:run:implement:1:phaseStart:0->run:implement:1:executor:0',
      source: 'run:implement:1:phaseStart:0',
      target: 'run:implement:1:executor:0',
    });
    // 非回跳边恒无标签（label 词汇仅回跳边承载）
    expect(graph.edges.slice(0, 3).map((edge) => edge.label)).toEqual([null, null, null]);
  });

  it('backtrack 边 label 取发起相位最近一次条目记因（pipeline 参数保留生效）', () => {
    const base = detail({
      pipeline: PIPELINE_PHASES.map((phase) =>
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
        pipeline: PIPELINE_PHASES.map((phase) =>
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
});
