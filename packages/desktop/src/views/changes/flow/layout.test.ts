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
    name: 'add-feature',
    source: 'active',
    status: 'active',
    created: null,
    pipeline: PIPELINE_PHASES.map((phase) => ({
      phase,
      attempts: phase === 'proposal' || phase === 'dev-design' ? [attempt()] : [],
    })),
    activePhase: null,
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

  it('末列 colIndex=8 → x 恰为 8 × (COL_W + 20)（画布总宽边界 ≈2500px）', () => {
    expect(nodePosition(column(8)).x).toBe(2240);
    // 9 列总宽 = 8 × (COL_W + 20) + COL_W
    expect(8 * (COL_W + 20) + COL_W).toBe(2500);
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
