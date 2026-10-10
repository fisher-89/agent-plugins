import { describe, expect, it } from 'vite-plus/test';

import type { AttemptRecord, ChangeDetail, PhaseEntry } from '../../../types/dto';
import { buildFlowGraph } from './graph';
import { PIPELINE_PHASES } from './layout';
import type { RuntimeFlowNode } from './types';

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

/** 建档详情底座：9 站全空 attempts，逐用例按需注入事件。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
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

/** 两站 eval 骨干（proposal → dev-design），供三分类与归并类用例共用。 */
function twoStationDetail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return detail({
    pipeline: PIPELINE_PHASES.map((phase) => {
      if (phase === 'proposal') {
        return station(phase, [attempt({ startAt: '2026-09-02T00:00:00Z' })]);
      }
      if (phase === 'dev-design') {
        return station(phase, [attempt({ startAt: '2026-09-03T00:00:00Z' })]);
      }
      return station(phase, []);
    }),
    ...overrides,
  });
}

function nodeIds(nodes: { id: string }[]): string[] {
  return nodes.map((node) => node.id);
}

/** 统计指向某节点的入边数（链式边推导：每个非首事件恰一条）。 */
function inDegree(edges: { target: string }[], nodeId: string): number {
  return edges.filter((edge) => edge.target === nodeId).length;
}

describe('buildFlowGraph：列与坐标骨架', () => {
  it('v2 详情恒输出 9 列容器（id / phase / colIndex 与 PIPELINE_PHASES 同序），事件节点按归并序排列', () => {
    const graph = buildFlowGraph(twoStationDetail());
    expect(graph.columns).toHaveLength(9);
    expect(graph.columns.map((column) => column.id)).toEqual(
      PIPELINE_PHASES.map((phase) => `col:${phase}`),
    );
    expect(graph.columns.map((column) => column.phase)).toEqual([...PIPELINE_PHASES]);
    expect(graph.columns.map((column) => column.colIndex)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8]);
    // 列容器在前（columns 全量有序），事件节点（nodes）按站序归并在后
    expect(nodeIds(graph.nodes)).toEqual(['eval:proposal:1', 'eval:dev-design:1']);
  });

  it('单站单 attempt：该站恰 1 个事件节点（order=0），其余 8 站为空列', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'implement' ? [attempt({ attempt: 2 })] : []),
        ),
      }),
    );
    expect(graph.columns).toHaveLength(9);
    expect(graph.nodes).toHaveLength(1);
    const only = graph.nodes[0];
    expect(only.phase).toBe('implement');
    expect(only.order).toBe(0);
    expect(only.parentId).toBe('col:implement');
    // 其余 8 列容器存在且无事件节点归属
    const occupied = new Set(graph.nodes.map((node) => node.parentId));
    expect(occupied.size).toBe(1);
    expect(graph.columns.filter((column) => !occupied.has(column.id))).toHaveLength(8);
  });

  it('pipeline 为空数组（v0 形态）→ 返回空图：columns / nodes / edges 全空', () => {
    expect(buildFlowGraph(detail({ pipeline: [] }))).toEqual({
      columns: [],
      nodes: [],
      edges: [],
    });
  });

  it('建档详情（各站 attempts 全空）→ 恒 9 空列、零事件节点、零边，不抛错', () => {
    const graph = buildFlowGraph(detail());
    expect(graph.columns).toHaveLength(9);
    expect(graph.nodes).toHaveLength(0);
    expect(graph.edges).toHaveLength(0);
  });

  it('attempt 为 null 的 eval 记录 → 节点 id 与 attempt 字段按 0 兜底；同站撞车加序号后缀不产生重复 id', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: [station('proposal', [attempt({ attempt: null }), attempt({ attempt: null })])],
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual(['eval:proposal:0', 'eval:proposal:0#1']);
    for (const node of graph.nodes) {
      expect(node.kind).toBe('eval');
      expect(node.attempt).toBe(0);
    }
  });
});

describe('buildFlowGraph：节点两分类', () => {
  it('eval 记录 → kind=eval 节点原样携带 record 且 parentId 为所属列容器 id', () => {
    const record = attempt({ verdict: 'fail', stale: true });
    const graph = buildFlowGraph(detail({ pipeline: [station('proposal', [record])] }));
    expect(graph.nodes).toHaveLength(1);
    const node = graph.nodes[0];
    expect(node.kind).toBe('eval');
    expect(node.parentId).toBe('col:proposal');
    expect(node.colIndex).toBe(0);
    if (node.kind !== 'eval') throw new Error('应为 eval 节点');
    expect(node.record).toBe(record);
  });

  it('activePhase 非空 → kind=active 节点（无 record），列内接流末端（该列 order 最大）', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(
            phase,
            phase === 'dev-design' ? [attempt({ startAt: '2026-09-02T00:00:00Z' })] : [],
          ),
        ),
        activePhase: { phase: 'dev-design', attempt: 2, startAt: '2026-09-03T00:00:00Z' },
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual(['eval:dev-design:1', 'active:dev-design:2']);
    const active = graph.nodes[1];
    expect(active.kind).toBe('active');
    expect('record' in active).toBe(false);
    expect(graph.nodes[0].order).toBe(0);
    expect(active.order).toBe(1);
  });
});

describe('buildFlowGraph：时间序归并与边推导', () => {
  it('多站 eval 主链：非首事件恰一条入边，kind 按端点列差派生（forward / retry）', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) => {
          if (phase === 'proposal') return station(phase, [attempt({ attempt: 1 })]);
          if (phase === 'dev-design') {
            return station(phase, [attempt({ attempt: 1 }), attempt({ attempt: 2 })]);
          }
          if (phase === 'test-design') return station(phase, [attempt({ attempt: 1 })]);
          return station(phase, []);
        }),
      }),
    );
    const sequence = nodeIds(graph.nodes);
    expect(sequence).toEqual([
      'eval:proposal:1',
      'eval:dev-design:1',
      'eval:dev-design:2',
      'eval:test-design:1',
    ]);
    expect(graph.edges.map((edge) => edge.id)).toEqual([
      'edge:eval:proposal:1->eval:dev-design:1',
      'edge:eval:dev-design:1->eval:dev-design:2',
      'edge:eval:dev-design:2->eval:test-design:1',
    ]);
    // Δ>0 前进、Δ=0 重试
    expect(graph.edges.map((edge) => edge.kind)).toEqual(['forward', 'retry', 'forward']);
    expect(graph.edges.map((edge) => edge.label)).toEqual([null, null, null]);
    for (const id of sequence.slice(1)) {
      expect(inDegree(graph.edges, id)).toBe(1);
    }
  });

  it('backtrack 边 label = 目标节点 record.backtrackReason；reason 为 null 的回跳边 label 为 null', () => {
    // active 站列靠后（test-execution），其 startAt 插入点在 implement eval 之前，
    // 归并后形成「靠后列 active → 靠前列 eval」的回跳边
    const withReason = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) => {
          if (phase === 'proposal') {
            return station(phase, [attempt({ startAt: '2026-09-02T00:00:00Z' })]);
          }
          if (phase === 'dev-design') {
            return station(phase, [attempt({ startAt: '2026-09-03T00:00:00Z' })]);
          }
          if (phase === 'test-design') {
            return station(phase, [attempt({ startAt: '2026-09-04T00:00:00Z' })]);
          }
          if (phase === 'implement') {
            return station(phase, [
              attempt({
                startAt: '2026-09-05T00:00:00Z',
                backtrackTo: 'test-design',
                backtrackReason: '实现缺陷回改',
              }),
            ]);
          }
          return station(phase, []);
        }),
        activePhase: { phase: 'test-execution', attempt: 1, startAt: '2026-09-04T12:00:00Z' },
      }),
    );
    const sequence = nodeIds(withReason.nodes);
    expect(sequence).toEqual([
      'eval:proposal:1',
      'eval:dev-design:1',
      'eval:test-design:1',
      'active:test-execution:1',
      'eval:implement:1',
    ]);
    const backtrack = withReason.edges.find((edge) => edge.kind === 'backtrack');
    expect(backtrack?.source).toBe('active:test-execution:1');
    expect(backtrack?.target).toBe('eval:implement:1');
    expect(backtrack?.label).toBe('实现缺陷回改');

    // 同构夹具：目标 eval 无 backtrackReason → label 为 null（不渲染字符串 "null"）
    const withoutReason = buildFlowGraph(withReasonNodesWithoutReasonFixture());
    const nullBacktrack = withoutReason.edges.find((edge) => edge.kind === 'backtrack');
    expect(nullBacktrack?.kind).toBe('backtrack');
    expect(nullBacktrack?.label).toBeNull();
  });

  it('active 按 startAt 插入骨干中第一个锚点严格晚于它的 eval 事件之前', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) => {
          if (phase === 'proposal') {
            return station(phase, [attempt({ startAt: '2026-09-01T00:00:00Z' })]);
          }
          if (phase === 'dev-design') {
            return station(phase, [attempt({ startAt: '2026-09-02T00:00:00Z' })]);
          }
          if (phase === 'test-design') {
            return station(phase, [attempt({ startAt: '2026-09-03T00:00:00Z' })]);
          }
          return station(phase, []);
        }),
        activePhase: { phase: 'test-design', attempt: 2, startAt: '2026-09-01T12:00:00Z' },
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:proposal:1',
      'active:test-design:2',
      'eval:dev-design:1',
      'eval:test-design:1',
    ]);
  });

  it('归并后序列头（首个事件）无入边', () => {
    const graph = buildFlowGraph(twoStationDetail());
    expect(graph.edges).toHaveLength(1);
    expect(inDegree(graph.edges, 'eval:proposal:1')).toBe(0);
  });

  it('startAt 为 null 的 active → 追加链尾', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'proposal' ? [attempt()] : []),
        ),
        activePhase: { phase: 'test-gen', attempt: 1, startAt: null },
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual(['eval:proposal:1', 'active:test-gen:1']);
  });

  it('startAt 为非法时间串（Date.parse 产出 NaN）→ 视同 null 追加链尾，不抛错', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'proposal' ? [attempt()] : []),
        ),
        activePhase: { phase: 'implement', attempt: 1, startAt: 'not-a-timestamp' },
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual(['eval:proposal:1', 'active:implement:1']);
    expect(graph.edges).toHaveLength(1);
    expect(graph.edges[0].target).toBe('active:implement:1');
  });

  it('骨干 eval 锚点为 null → 不作插入参考，插入点跳过 null 锚点落位', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) => {
          if (phase === 'proposal') return station(phase, [attempt({ startAt: null })]);
          if (phase === 'dev-design') {
            return station(phase, [attempt({ startAt: '2026-09-03T00:00:00Z' })]);
          }
          return station(phase, []);
        }),
        activePhase: { phase: 'test-design', attempt: 1, startAt: '2026-09-01T00:00:00Z' },
      }),
    );
    // 首 eval（proposal）锚点 null 不参考：插入点为首个锚点更晚的 dev-design eval 之前
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:proposal:1',
      'active:test-design:1',
      'eval:dev-design:1',
    ]);
  });

  it('eval 按startAt时间重排', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) => {
          if (phase === 'proposal') {
            return station(phase, [attempt({ startAt: '2026-09-05T00:00:00Z' })]);
          }
          if (phase === 'dev-design') {
            return station(phase, [
              attempt({ attempt: 1, startAt: '2026-09-01T00:00:00Z' }),
              attempt({ attempt: 2, startAt: '2026-09-02T00:00:00Z' }),
            ]);
          }
          return station(phase, []);
        }),
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:dev-design:1',
      'eval:dev-design:2',
      'eval:proposal:1',
    ]);
    expect(graph.edges.map((edge) => edge.kind)).toEqual(['retry', 'backtrack']);
  });

  it('startAt 早于全部 eval 锚点 → 插入为序列头（该节点无入边）', () => {
    const graph = buildFlowGraph(
      detail({
        activePhase: { phase: 'implement', attempt: 1, startAt: '2026-09-01T00:00:00Z' },
        ...twoStationOverrides(),
      }),
    );
    expect(graph.nodes[0].id).toBe('active:implement:1');
    expect(graph.nodes[0].kind).toBe('active');
    expect(inDegree(graph.edges, 'active:implement:1')).toBe(0);
    // 3 事件链式相连：恰 2 条边，首事件无入边
    expect(graph.edges).toHaveLength(2);
  });
});

/** 两站骨干 + 末列 active 夹具（startAt 早于全部锚点用例）。 */
function twoStationOverrides(): Partial<ChangeDetail> {
  return {
    pipeline: PIPELINE_PHASES.map((phase) => {
      if (phase === 'proposal') {
        return station(phase, [attempt({ startAt: '2026-09-02T00:00:00Z' })]);
      }
      if (phase === 'dev-design') {
        return station(phase, [attempt({ startAt: '2026-09-03T00:00:00Z' })]);
      }
      return station(phase, []);
    }),
  };
}

/** 回跳边 label null 分支夹具：目标 eval 无 backtrackReason。 */
function withReasonNodesWithoutReasonFixture(): ChangeDetail {
  return detail({
    pipeline: PIPELINE_PHASES.map((phase) => {
      if (phase === 'proposal') {
        return station(phase, [attempt({ startAt: '2026-09-02T00:00:00Z' })]);
      }
      if (phase === 'dev-design') {
        return station(phase, [attempt({ startAt: '2026-09-03T00:00:00Z' })]);
      }
      if (phase === 'test-design') {
        return station(phase, [attempt({ startAt: '2026-09-04T00:00:00Z' })]);
      }
      if (phase === 'implement') {
        return station(phase, [attempt({ startAt: '2026-09-05T00:00:00Z' })]);
      }
      return station(phase, []);
    }),
    activePhase: { phase: 'test-execution', attempt: 1, startAt: '2026-09-04T12:00:00Z' },
  });
}

// ---------------------------------------------------------------------------
// 运行步 overlay（可选第二参 runNodes，desktop-change-flow 增量）：运行步节点
// 恒追加归并链尾并参与同一条链的边推导（执行图 = 展示图）；缺省 / 空参输出与
// 现状零 diff（AC-5 回归半边）。RuntimeFlowNode fixture 沿既有手工构造装置
// 直构（run-state.ts runStepNodes 的同形投影）。
// ---------------------------------------------------------------------------

/** 运行步 overlay 节点 fixture（runStepNodes 同形投影的直构形态）。 */
function runNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return {
    id: 'run:implement:1:executor',
    kind: 'runtime',
    phase: 'implement',
    attempt: 1,
    colIndex: PIPELINE_PHASES.indexOf('implement'),
    order: 0,
    parentId: 'col:implement',
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    ...overrides,
  };
}

describe('buildFlowGraph：运行步 overlay（可选第二参 runNodes）', () => {
  it('缺省与空参输出逐字段相等，且与既有场景基线一致（零 diff 回归）', () => {
    const scenarios: ChangeDetail[] = [
      twoStationDetail(),
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'proposal' ? [attempt()] : []),
        ),
        activePhase: { phase: 'test-gen', attempt: 1, startAt: null },
      }),
      detail({ pipeline: [] }),
    ];
    for (const base of scenarios) {
      expect(buildFlowGraph(base)).toEqual(buildFlowGraph(base, []));
    }
    // 基线锚：overlay 通道的存在不漂移既有派生输出（desktop-change-flow-view 既有场景全绿半边）
    expect(nodeIds(buildFlowGraph(twoStationDetail()).nodes)).toEqual([
      'eval:proposal:1',
      'eval:dev-design:1',
    ]);
  });

  it('传入 runNodes → 运行步节点按给定序追加归并链尾，每个非首节点恰一条入边延伸至运行步节点', () => {
    const graph = buildFlowGraph(twoStationDetail(), [runNode()]);
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:proposal:1',
      'eval:dev-design:1',
      'run:implement:1:executor',
    ]);
    // 边推导规则不变：链尾运行步恰一条入边，kind 由两端列差派生（col1 → col3 forward）
    expect(graph.edges).toHaveLength(2);
    expect(graph.edges[1]).toEqual({
      id: 'edge:eval:dev-design:1->run:implement:1:executor',
      source: 'eval:dev-design:1',
      target: 'run:implement:1:executor',
      kind: 'forward',
      label: null,
    });
    expect(inDegree(graph.edges, 'run:implement:1:executor')).toBe(1);
    expect(inDegree(graph.edges, 'eval:proposal:1')).toBe(0);
  });

  it('WorkerAgent / ToolStep / Gate 三类运行步节点经 overlay 上图，kind / runStepKind 载荷可辨', () => {
    const graph = buildFlowGraph(detail(), [
      runNode(),
      runNode({
        id: 'run:implement:1:staticCheck',
        runStepKind: 'staticCheck',
        group: 'toolStep',
        role: null,
        sessionId: null,
        detail: '2 处诊断',
      }),
      runNode({
        id: 'run:implement:1:verdictGate',
        runStepKind: 'verdictGate',
        group: 'gate',
        role: null,
        sessionId: null,
      }),
    ]);
    expect(graph.nodes.map((node) => node.kind)).toEqual(['runtime', 'runtime', 'runtime']);
    const [worker, tool, gate] = graph.nodes;
    if (worker.kind !== 'runtime' || tool.kind !== 'runtime' || gate.kind !== 'runtime') {
      throw new Error('应为 runtime 节点');
    }
    expect(worker.group).toBe('workerAgent');
    expect(worker.runStepKind).toBe('executor');
    expect(worker.role).toBe('executor');
    expect(tool.group).toBe('toolStep');
    expect(tool.runStepKind).toBe('staticCheck');
    expect(tool.detail).toBe('2 处诊断');
    expect(gate.group).toBe('gate');
    expect(gate.runStepKind).toBe('verdictGate');
    // 空骨干上三类节点链式相连：同列 Δ=0 → 两条 retry 边，链头无入边
    expect(graph.edges.map((edge) => edge.id)).toEqual([
      'edge:run:implement:1:executor->run:implement:1:staticCheck',
      'edge:run:implement:1:staticCheck->run:implement:1:verdictGate',
    ]);
    expect(graph.edges.map((edge) => edge.kind)).toEqual(['retry', 'retry']);
    expect(inDegree(graph.edges, 'run:implement:1:executor')).toBe(0);
  });

  it('attempt 交错归并：overlay 恒链尾、列内 order 接续既有事件计数、attempt 缺号兜底不变', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(
            phase,
            phase === 'proposal'
              ? [attempt({ attempt: null, startAt: '2026-09-02T00:00:00Z' })]
              : [],
          ),
        ),
      }),
      [
        runNode({ id: 'run:implement:2:executor', phase: 'implement', attempt: 2 }),
        runNode({
          id: 'run:proposal:2:evaluator',
          phase: 'proposal',
          attempt: 2,
          colIndex: PIPELINE_PHASES.indexOf('proposal'),
          parentId: 'col:proposal',
          runStepKind: 'evaluator',
        }),
      ],
    );
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:proposal:0',
      'run:implement:2:executor',
      'run:proposal:2:evaluator',
    ]);
    const [evalNode, implementRun, proposalRun] = graph.nodes;
    expect(evalNode.order).toBe(0);
    expect(implementRun.order).toBe(0);
    // 交错体现在列内计数：proposal 列的运行步接续既有 eval 事件编号，骨干不重排
    expect(proposalRun.order).toBe(1);
    // 边推导规则不变：col0 → col3 forward、col3 → col0 backtrack
    expect(graph.edges.map((edge) => edge.kind)).toEqual(['forward', 'backtrack']);
    // attempt 缺号兜底不变（eval null → 0），overlay 节点 attempt 原样承接
    expect(evalNode.attempt).toBe(0);
    expect(proposalRun.attempt).toBe(2);
  });

  it('混合终态 overlay（failed / stopped / passed 混存）归并稳定不炸、终态载荷与链式边保留', () => {
    const graph = buildFlowGraph(twoStationDetail(), [
      runNode({ status: 'failed', detail: '会话失败：CLI 漂移' }),
      runNode({
        id: 'run:test-gen:1:phaseStart',
        phase: 'test-gen',
        runStepKind: 'phaseStart',
        group: 'toolStep',
        role: null,
        status: 'stopped',
        sessionId: null,
      }),
      runNode({
        id: 'run:test-execution:1:evaluator',
        phase: 'test-execution',
        runStepKind: 'evaluator',
        status: 'passed',
        sessionId: 'ses-eval-1',
      }),
    ]);
    expect(graph.nodes).toHaveLength(5);
    expect(graph.nodes.map((node) => (node.kind === 'runtime' ? node.status : null))).toEqual([
      null,
      null,
      'failed',
      'stopped',
      'passed',
    ]);
    expect(graph.edges).toHaveLength(4);
    for (const node of graph.nodes.slice(1)) {
      expect(inDegree(graph.edges, node.id)).toBe(1);
    }
  });
});

// ---------------------------------------------------------------------------
// FlowNodeKind 两分类收敛（desktop-drawer-session-column，AC-5）：
// `collectInterrupted` 删除、归并列表只余 `collectActive(detail)`——事件节点
// kind ∈ { eval, active }、运行步节点 kind = runtime，无第三分类可走
// ---------------------------------------------------------------------------

describe('buildFlowGraph：FlowNodeKind 两分类收敛（AC-5）', () => {
  it('输出节点 kind 穷举：事件节点 kind ∈ {eval, active}、运行步节点 kind = runtime（无第三分类）', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'proposal' ? [attempt()] : []),
        ),
        activePhase: { phase: 'implement', attempt: 1, startAt: null },
      }),
      [runNode()],
    );
    // 事件节点两分类穷举（collectInterrupted 删除后无第三事件分类可产出）
    const eventKinds = [
      ...new Set(graph.nodes.filter((node) => node.kind !== 'runtime').map((node) => node.kind)),
    ].sort();
    expect(eventKinds).toEqual(['active', 'eval']);
    for (const node of graph.nodes) {
      expect(['eval', 'active', 'runtime']).toContain(node.kind);
    }
    // 运行步节点 kind 恒 runtime
    expect(graph.nodes.at(-1)?.kind).toBe('runtime');
  });

  it('同号 eval 缺席的 active → 独立成节点且恰一条入边（归并链收敛后唯一事件分类的正向锚）', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'proposal' ? [attempt()] : []),
        ),
        activePhase: { phase: 'implement', attempt: 2, startAt: '2026-09-04T00:00:00Z' },
      }),
    );
    // active 单节点归并：同号 eval 缺席时独立成节点、非首事件恰一条入边
    expect(nodeIds(graph.nodes)).toEqual(['eval:proposal:1', 'active:implement:2']);
    expect(inDegree(graph.edges, 'active:implement:2')).toBe(1);
    expect(graph.edges).toHaveLength(1);
    expect(graph.edges[0].kind).toBe('forward');
  });
});
