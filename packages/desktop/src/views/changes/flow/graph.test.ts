import { describe, expect, it } from 'vite-plus/test';

import type { AttemptRecord, ChangeDetail, PhaseEntry } from '../../../types/dto';
import { buildFlowGraph } from './graph';
import { PIPELINE_PHASES } from './layout';

// ---------------------------------------------------------------------------
// buildFlowGraph 单测：转换层为零 react / 零 @xyflow/react / 零 invoke 的纯函数，
// 以手工构造的 ChangeDetail DTO fixture（v2-a / v2-b 夹具形态）直接喂公共入口。
// 覆盖：列与坐标骨架 / 节点三分类 / 时间序归并与边推导（AC-1 / AC-2 / AC-3 / AC-8）。
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
    ...overrides,
  };
}

function station(phase: string, attempts: AttemptRecord[]): PhaseEntry {
  return { phase, attempts };
}

/** v2 详情底座：9 站全空 attempts，逐用例按需注入事件。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: '2026-09-01',
    unparsable: false,
    pipeline: PIPELINE_PHASES.map((phase) => station(phase, [])),
    activePhase: null,
    interrupted: [],
    fileLog: [],
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

  it('unparsable 详情（各站 attempts 全空）→ 恒 9 空列、零事件节点、零边，不抛错', () => {
    const graph = buildFlowGraph(detail({ unparsable: true }));
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

describe('buildFlowGraph：节点三分类', () => {
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

  it('interrupted 非空 → 独立节点携带 startAt / endAt，与同号 eval 并存不合并（v2-b 形态）', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(
            phase,
            phase === 'test-design' ? [attempt({ startAt: '2026-09-03T10:00:00Z' })] : [],
          ),
        ),
        interrupted: [
          {
            phase: 'test-design',
            attempt: 1,
            startAt: '2026-09-03T16:00:00Z',
            endAt: '2026-09-03T17:00:00Z',
          },
        ],
      }),
    );
    // 同 phase 同 attempt：eval 与 interrupted 各自成节点，不合并
    expect(nodeIds(graph.nodes)).toEqual(['eval:test-design:1', 'interrupted:test-design:1']);
    const interrupted = graph.nodes[1];
    if (interrupted.kind !== 'interrupted') throw new Error('应为 interrupted 节点');
    expect(interrupted.startAt).toBe('2026-09-03T16:00:00Z');
    expect(interrupted.endAt).toBe('2026-09-03T17:00:00Z');
    expect('record' in interrupted).toBe(false);
  });

  it('interrupted 与 activePhase 同时存在 → 两类节点共存、各自恰一条入边', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'proposal' || phase === 'dev-design' ? [attempt()] : []),
        ),
        interrupted: [{ phase: 'implement', attempt: 1, startAt: null, endAt: null }],
        activePhase: { phase: 'test-gen', attempt: 1, startAt: null },
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:proposal:1',
      'eval:dev-design:1',
      'interrupted:implement:1',
      'active:test-gen:1',
    ]);
    expect(graph.edges).toHaveLength(3);
    for (const id of ['eval:dev-design:1', 'interrupted:implement:1', 'active:test-gen:1']) {
      expect(inDegree(graph.edges, id)).toBe(1);
    }
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
    // 中断站列靠后（test-execution），其 startAt 插入点在 implement eval 之前，
    // 归并后形成「靠后列中断 → 靠前列 eval」的回跳边
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
        interrupted: [
          { phase: 'test-execution', attempt: 1, startAt: '2026-09-04T12:00:00Z', endAt: null },
        ],
      }),
    );
    const sequence = nodeIds(withReason.nodes);
    expect(sequence).toEqual([
      'eval:proposal:1',
      'eval:dev-design:1',
      'eval:test-design:1',
      'interrupted:test-execution:1',
      'eval:implement:1',
    ]);
    const backtrack = withReason.edges.find((edge) => edge.kind === 'backtrack');
    expect(backtrack?.source).toBe('interrupted:test-execution:1');
    expect(backtrack?.target).toBe('eval:implement:1');
    expect(backtrack?.label).toBe('实现缺陷回改');

    // 同构夹具：目标 eval 无 backtrackReason → label 为 null（不渲染字符串 "null"）
    const withoutReason = buildFlowGraph(withReasonNodesWithoutReasonFixture());
    const nullBacktrack = withoutReason.edges.find((edge) => edge.kind === 'backtrack');
    expect(nullBacktrack?.kind).toBe('backtrack');
    expect(nullBacktrack?.label).toBeNull();
  });

  it('interrupted / active 按 startAt 插入骨干中第一个锚点严格晚于它的 eval 事件之前', () => {
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
        interrupted: [
          // 插入 test-design eval 之前（唯一锚点更晚者）
          { phase: 'implement', attempt: 1, startAt: '2026-09-02T12:00:00Z', endAt: null },
          // 锚点与 test-design eval 相等：不满足「严格晚于」→ 无插入点，追加链尾
          { phase: 'code-review', attempt: 1, startAt: '2026-09-03T00:00:00Z', endAt: null },
        ],
        activePhase: { phase: 'test-design', attempt: 2, startAt: '2026-09-01T12:00:00Z' },
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:proposal:1',
      'active:test-design:2',
      'eval:dev-design:1',
      'interrupted:implement:1',
      'eval:test-design:1',
      'interrupted:code-review:1',
    ]);
  });

  it('归并后序列头（首个事件）无入边', () => {
    const graph = buildFlowGraph(twoStationDetail());
    expect(graph.edges).toHaveLength(1);
    expect(inDegree(graph.edges, 'eval:proposal:1')).toBe(0);
  });

  it('startAt 为 null 的 interrupted / active → 按列表序（先 interrupted[] 后 active）追加链尾', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'proposal' ? [attempt()] : []),
        ),
        interrupted: [{ phase: 'implement', attempt: 1, startAt: null, endAt: null }],
        activePhase: { phase: 'test-gen', attempt: 1, startAt: null },
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:proposal:1',
      'interrupted:implement:1',
      'active:test-gen:1',
    ]);
  });

  it('startAt 为非法时间串（Date.parse 产出 NaN）→ 视同 null 追加链尾，不抛错', () => {
    const graph = buildFlowGraph(
      detail({
        pipeline: PIPELINE_PHASES.map((phase) =>
          station(phase, phase === 'proposal' ? [attempt()] : []),
        ),
        interrupted: [{ phase: 'implement', attempt: 1, startAt: 'not-a-timestamp', endAt: null }],
      }),
    );
    expect(nodeIds(graph.nodes)).toEqual(['eval:proposal:1', 'interrupted:implement:1']);
    expect(graph.edges).toHaveLength(1);
    expect(graph.edges[0].target).toBe('interrupted:implement:1');
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
        interrupted: [
          { phase: 'test-design', attempt: 1, startAt: '2026-09-01T00:00:00Z', endAt: null },
        ],
      }),
    );
    // 首 eval（proposal）锚点 null 不参考：插入点为首个锚点更晚的 dev-design eval 之前
    expect(nodeIds(graph.nodes)).toEqual([
      'eval:proposal:1',
      'interrupted:test-design:1',
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
        interrupted: [
          { phase: 'implement', attempt: 1, startAt: '2026-09-01T00:00:00Z', endAt: null },
        ],
        ...twoStationOverrides(),
      }),
    );
    expect(graph.nodes[0].id).toBe('interrupted:implement:1');
    expect(graph.nodes[0].kind).toBe('interrupted');
    expect(inDegree(graph.edges, 'interrupted:implement:1')).toBe(0);
    // 3 事件链式相连：恰 2 条边，首事件无入边
    expect(graph.edges).toHaveLength(2);
  });
});

/** 两站骨干 + 末列中断夹具（startAt 早于全部锚点用例）。 */
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
    interrupted: [
      { phase: 'test-execution', attempt: 1, startAt: '2026-09-04T12:00:00Z', endAt: null },
    ],
  });
}
