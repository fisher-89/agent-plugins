import { describe, expect, it } from 'vite-plus/test';

import type { ChangeDetail } from '../../../types/dto';
import { buildFlowGraph } from './graph';
import { COLUMN_HEADER_H, columnLayout, nodeLayout } from './layout';
import type { FlowColumn, FlowGraph, FlowNode, RuntimeFlowNode } from './types';

const COLUMN_PADDING = 18;
const COLUMN_WIDTH = 260;

/**
 * requirement 相位序夹具（D12：前端 `PIPELINE_PHASES` 常量退役，列序改由
 * `detail.pipeline` 出线承载，测试各自内联 fixture）：9 列，archive 为第 9 列、
 * 无 code-analyze 占位列。
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

/** 九站空 attempts 的建档详情（列骨架来源——节点模型单分类后节点经 runNodes 直供）。 */
function detail(): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: null,
    pipeline: REQUIREMENT_PHASES.map((phase) => ({ phase, attempts: [] })),
    activePhase: null,
    runs: [],
    worktree: null,
    artifacts: [],
  };
}

/** 运行步节点 fixture（run-state.ts runStepNodes 同形投影的直构形态）。 */
function runNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return {
    id: 'run:proposal:1:executor:0',
    kind: 'runtime',
    phase: 'proposal',
    attempt: 1,
    colIndex: 0,
    order: 0,
    parentId: 'col:proposal',
    runId: 'run-1727',
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    instanceNo: 0,
    status: 'running',
    sessionId: null,
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

/** 两列各一枚运行步节点的图（坐标取样面——单分类节点模型的坐标公式）。 */
function graphWithTwoRunSteps(): FlowGraph {
  return buildFlowGraph(detail(), [
    runNode(),
    runNode({
      id: 'run:dev-design:1:executor:0',
      phase: 'dev-design',
      colIndex: 1,
      parentId: 'col:dev-design',
    }),
  ]);
}

function graph(): FlowGraph {
  return { columns: [], nodes: [], edges: [] };
}

function column(colIndex: number): FlowColumn {
  const phase = REQUIREMENT_PHASES[colIndex];
  return { id: `col:${phase}`, phase, colIndex };
}

describe('nodePosition：布局即语义坐标公式', () => {
  it('列容器节点 x = colIndex × (COL_W + 20)、y = 0（colIndex 0 / 4 / 8 抽样验证）', () => {
    expect(columnLayout(column(0), graph()).position).toEqual({ x: 0, y: 0 });
    expect(columnLayout(column(4), graph()).position).toEqual({ x: 4 * (COLUMN_WIDTH + 20), y: 0 });
    expect(columnLayout(column(8), graph()).position).toEqual({ x: 8 * (COLUMN_WIDTH + 20), y: 0 });
    expect(COLUMN_WIDTH).toBe(260);
  });

  it('运行步节点 x = COLUMN_PAD_X、y = COLUMN_HEADER_H + order × ROW_H（常量取值 18 / 48 / 128）', () => {
    const graph = graphWithTwoRunSteps();
    const proposal = graph.nodes.find((node) => node.phase === 'proposal');
    const devDesign = graph.nodes.find((node) => node.phase === 'dev-design');
    if (proposal === undefined || devDesign === undefined) throw new Error('夹具节点缺失');
    expect(proposal.order).toBe(0);
    expect(devDesign.order).toBe(0);
    expect(nodeLayout(proposal).position).toEqual({
      x: COLUMN_PADDING,
      y: COLUMN_HEADER_H + COLUMN_PADDING,
    });
    expect(nodeLayout(devDesign).position).toEqual({ x: 18, y: 66 });
  });

  it('order=0（列内首个节点）→ y 恰为 COLUMN_HEADER_H + COLUMN_PADDING', () => {
    const graph = graphWithTwoRunSteps();
    const first = graph.nodes[0];
    expect(first.order).toBe(0);
    expect(nodeLayout(first).position.y).toBe(66);
  });

  it('末列 colIndex=8（archive）→ x 恰为 8 × (COL_W + 20)（画布总宽边界上移一列）', () => {
    expect(columnLayout(column(8), graph()).position.x).toBe(8 * (COLUMN_WIDTH + 20));
    // 9 列总宽 = 8 × (COL_W + 20) + COL_W
    expect(8 * (COLUMN_WIDTH + 20) + COLUMN_WIDTH).toBe(2500);
  });

  it('超大 order（如 1000）→ 坐标按公式线性外推，无钳制、无 NaN、不抛错', () => {
    const graph = graphWithTwoRunSteps();
    const node: FlowNode = { ...graph.nodes[0], order: 1000 };
    const { position } = nodeLayout(node);
    expect(position).toEqual({ x: 18, y: 48 + COLUMN_PADDING + 1000 * 96 });
    expect(Number.isFinite(position.x)).toBe(true);
    expect(Number.isFinite(position.y)).toBe(true);
  });
});

describe('layout：列序由 detail.pipeline 出线（D12：PIPELINE_PHASES 常量退役）', () => {
  it('边界：archive 列（colIndex=8）坐标合法且空列时节点列表不报错', () => {
    const archiveColumn = column(8);
    expect(archiveColumn.phase).toBe('archive');
    const { position } = columnLayout(archiveColumn, graph());
    expect(position.y).toBe(0);
    expect(position.x).toBe(8 * (COLUMN_WIDTH + 20));
    expect(Number.isFinite(position.x)).toBe(true);

    // 空列（archive 零步节点）→ 图仍产出该列容器、零节点
    const mockGraph = graphWithTwoRunSteps();
    const station = mockGraph.columns.find((entry) => entry.phase === 'archive');
    expect(station).toBeDefined();
    expect(mockGraph.nodes.filter((node) => node.parentId === 'col:archive')).toHaveLength(0);
  });

  it('异常（回归）：列词汇恒为该 change 相位表已知相位（未知相位不在列集合）', () => {
    const graph = graphWithTwoRunSteps();
    const phases = new Set(graph.columns.map((entry) => entry.phase));
    expect(phases.has('unknown-phase')).toBe(false);
    expect([...phases]).toEqual([...REQUIREMENT_PHASES]);
  });
});
