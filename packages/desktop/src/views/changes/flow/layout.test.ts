import { describe, expect, it } from 'vite-plus/test';

import type { ChangeDetail } from '../../../types/dto';
import { buildFlowGraph } from './graph';
import { COLUMN_HEADER_H, columnLayout, nodeLayout, PIPELINE_PHASES } from './layout';
import type { FlowColumn, FlowGraph, FlowNode, RuntimeFlowNode } from './types';

const COLUMN_PADDING = 18;
const COLUMN_WIDTH = 260;

/** 十站空 attempts 的建档详情（列骨架来源——节点模型单分类后节点经 runNodes 直供）。 */
function detail(): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: null,
    pipeline: PIPELINE_PHASES.map((phase) => ({ phase, attempts: [] })),
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
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    instanceNo: 0,
    status: 'running',
    sessionId: null,
    detail: null,
    outputRef: null,
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
  const phase = PIPELINE_PHASES[colIndex];
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
    expect(nodeLayout(proposal).position).toEqual({ x: COLUMN_PADDING, y: COLUMN_HEADER_H });
    expect(nodeLayout(devDesign).position).toEqual({ x: 18, y: 48 });
  });

  it('order=0（列内首个节点）→ y 恰为 COLUMN_HEADER_H', () => {
    const graph = graphWithTwoRunSteps();
    const first = graph.nodes[0];
    expect(first.order).toBe(0);
    expect(nodeLayout(first).position.y).toBe(48);
  });

  it('末列 colIndex=9（archive）→ x 恰为 9 × (COL_W + 20)（画布总宽边界上移一列）', () => {
    expect(columnLayout(column(9), graph()).position.x).toBe(9 * (COLUMN_WIDTH + 20));
    // 10 列总宽 = 9 × (COL_W + 20) + COL_W
    expect(9 * (COLUMN_WIDTH + 20) + COLUMN_WIDTH).toBe(2780);
  });

  it('超大 order（如 1000）→ 坐标按公式线性外推，无钳制、无 NaN、不抛错', () => {
    const graph = graphWithTwoRunSteps();
    const node: FlowNode = { ...graph.nodes[0], order: 1000 };
    const { position } = nodeLayout(node);
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
    const { position } = columnLayout(archiveColumn, graph());
    expect(position.y).toBe(0);
    expect(position.x).toBe(9 * (COLUMN_WIDTH + 20));
    expect(Number.isFinite(position.x)).toBe(true);

    // 空列（archive 零步节点）→ 图仍产出该列容器、零节点
    const mockGraph = graphWithTwoRunSteps();
    const station = mockGraph.columns.find((entry) => entry.phase === 'archive');
    expect(station).toBeDefined();
    expect(mockGraph.nodes.filter((node) => node.parentId === 'col:archive')).toHaveLength(0);
  });

  it('异常（回归）：列词汇恒为十站已知相位（未知相位不在列集合）', () => {
    const graph = graphWithTwoRunSteps();
    const phases = new Set(graph.columns.map((entry) => entry.phase));
    expect(phases.has('unknown-phase')).toBe(false);
    expect([...phases]).toEqual([...PIPELINE_PHASES]);
  });
});
