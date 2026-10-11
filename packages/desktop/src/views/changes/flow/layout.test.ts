import { describe, expect, it } from 'vite-plus/test';

import type { AttemptRecord, ChangeDetail } from '../../../types/dto';
import { buildFlowGraph } from './graph';
import {
  COL_W,
  COLUMN_HEADER_H,
  COLUMN_PAD_X,
  PIPELINE_PHASES,
  ROW_H,
  nodePosition,
} from './layout';
import type { FlowColumn, FlowNode } from './types';

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

/** proposal + dev-design 各 1 条 eval 的最小建档详情（产出可用于坐标取样的节点）。 */
function detailWithTwoEvals(): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: null,
    pipeline: PIPELINE_PHASES.map((phase) => ({
      phase,
      attempts: phase === 'proposal' || phase === 'dev-design' ? [attempt()] : [],
    })),
    activePhase: null,
    runs: [],
    worktree: null,
    artifacts: [],
  };
}

function column(colIndex: number): FlowColumn {
  const phase = PIPELINE_PHASES[colIndex];
  return { id: `col:${phase}`, phase, colIndex };
}

describe('nodePosition：布局即语义坐标公式', () => {
  it('列容器节点 x = colIndex × (COL_W + 20)、y = 0（colIndex 0 / 4 / 8 抽样验证）', () => {
    expect(nodePosition(column(0))).toEqual({ x: 0, y: 0 });
    expect(nodePosition(column(4))).toEqual({ x: 4 * (COL_W + 20), y: 0 });
    expect(nodePosition(column(8))).toEqual({ x: 8 * (COL_W + 20), y: 0 });
    expect(COL_W).toBe(260);
  });

  it('事件节点 x = COLUMN_PAD_X、y = COLUMN_HEADER_H + order × ROW_H（常量取值 18 / 48 / 128）', () => {
    const graph = buildFlowGraph(detailWithTwoEvals());
    const proposal = graph.nodes.find((node) => node.phase === 'proposal');
    const devDesign = graph.nodes.find((node) => node.phase === 'dev-design');
    if (proposal === undefined || devDesign === undefined) throw new Error('夹具节点缺失');
    expect(proposal.order).toBe(0);
    expect(devDesign.order).toBe(0);
    expect(COLUMN_PAD_X).toBe(18);
    expect(COLUMN_HEADER_H).toBe(48);
    expect(ROW_H).toBe(128);
    expect(nodePosition(proposal)).toEqual({ x: COLUMN_PAD_X, y: COLUMN_HEADER_H });
    expect(nodePosition(devDesign)).toEqual({ x: 18, y: 48 });
  });

  it('order=0（列内首个事件）→ y 恰为 COLUMN_HEADER_H', () => {
    const graph = buildFlowGraph(detailWithTwoEvals());
    const first = graph.nodes[0];
    expect(first.order).toBe(0);
    expect(nodePosition(first).y).toBe(48);
  });

  it('末列 colIndex=9（archive）→ x 恰为 9 × (COL_W + 20)（画布总宽边界上移一列）', () => {
    expect(nodePosition(column(9)).x).toBe(9 * (COL_W + 20));
    // 10 列总宽 = 9 × (COL_W + 20) + COL_W
    expect(9 * (COL_W + 20) + COL_W).toBe(2780);
  });

  it('超大 order（如 1000）→ 坐标按公式线性外推，无钳制、无 NaN、不抛错', () => {
    const graph = buildFlowGraph(detailWithTwoEvals());
    const node: FlowNode = { ...graph.nodes[0], order: 1000 };
    const position = nodePosition(node);
    expect(position).toEqual({ x: 18, y: 48 + 1000 * 128 });
    expect(Number.isFinite(position.x)).toBe(true);
    expect(Number.isFinite(position.y)).toBe(true);
  });
});

describe('layout：PIPELINE_PHASES 十列布局（AC-1 / D5 双侧同步）', () => {
  it('正向：PIPELINE_PHASES 长度 10 且含 archive（末列、code-analyze 之后）', () => {
    expect(PIPELINE_PHASES).toHaveLength(10);
    expect(PIPELINE_PHASES[PIPELINE_PHASES.length - 1]).toBe('archive');
    expect(PIPELINE_PHASES[PIPELINE_PHASES.length - 2]).toBe('code-analyze');
    expect(new Set(PIPELINE_PHASES).size).toBe(10);
  });

  it('正向：与后端 PIPELINE_PHASES 同序（十站表序逐位一致，双侧同步不漂移）', () => {
    expect([...PIPELINE_PHASES]).toEqual([
      'proposal',
      'dev-design',
      'test-design',
      'implement',
      'test-gen',
      'test-execution',
      'code-review',
      'acceptance',
      'code-analyze',
      'archive',
    ]);
  });

  it('边界：archive 列（colIndex=9）坐标合法且空列时节点列表不报错', () => {
    const archiveColumn = column(9);
    expect(archiveColumn.phase).toBe('archive');
    const position = nodePosition(archiveColumn);
    expect(position.y).toBe(0);
    expect(position.x).toBe(9 * (COL_W + 20));
    expect(Number.isFinite(position.x)).toBe(true);

    // 空列（archive 零 attempt）→ 图仍产出该列容器、零事件节点
    const graph = buildFlowGraph(detailWithTwoEvals());
    const station = graph.columns.find((entry) => entry.phase === 'archive');
    expect(station).toBeDefined();
    expect(graph.nodes.filter((node) => node.parentId === 'col:archive')).toHaveLength(0);
  });

  it('异常（回归）：非法相位名（非十列成员）的步不入图（pipeline 外相位节点丢弃）', () => {
    const graph = buildFlowGraph(detailWithTwoEvals());
    const phases = new Set(graph.columns.map((entry) => entry.phase));
    expect(phases.has('unknown-phase')).toBe(false);
    for (const node of graph.nodes) {
      expect(phases.has(node.phase)).toBe(true);
    }
  });
});
