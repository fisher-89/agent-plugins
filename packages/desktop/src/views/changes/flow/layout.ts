import type { FlowColumn, FlowEdge, FlowGraph, FlowNode } from './types';

const GRAPH_PADDING = 24;

export const COLUMN_HEADER_H = 48;
const COLUMN_WIDTH = 260;
const COLUMN_GAP_X = 20;
const COLUMN_PADDING = 18;

const NODE_WIDTH = COLUMN_WIDTH - COLUMN_PADDING * 2;
const NODE_HEIGHT = 64;
const NODE_GAP_Y = 32;

type Position = { x: number; y: number };

type Size = { width: number; height: number };

type Layout = Size & { position: Position };

export function graphTranslateExtent(graph: FlowGraph): [[number, number], [number, number]] {
  const { width, height } = graphSize(graph);
  return [
    [-GRAPH_PADDING, -GRAPH_PADDING],
    [width + GRAPH_PADDING, height + GRAPH_PADDING],
  ];
}

function graphSize(graph: FlowGraph): Size {
  const columnCount = graph.columns.length;
  const width = columnCount * COLUMN_WIDTH + (columnCount - 1) * COLUMN_GAP_X;
  let maxY = 0;
  for (const column of graph.columns) {
    maxY = Math.max(maxY, columnSize(column, graph).height);
  }
  return { width, height: maxY };
}

export function columnLayout(column: FlowColumn, graph: FlowGraph): Layout {
  return { ...columnSize(column, graph), position: columnPosition(column) };
}

function columnSize(column: FlowColumn, graph: FlowGraph): Size {
  let count = 0;
  for (const node of graph.nodes) {
    if (node.parentId === column.id) count += 1;
  }
  const height =
    COLUMN_HEADER_H + count * NODE_HEIGHT + (count - 1) * NODE_GAP_Y + COLUMN_PADDING * 2;
  return { width: COLUMN_WIDTH, height };
}

function columnPosition(column: FlowColumn): Position {
  return { x: column.colIndex * (COLUMN_WIDTH + COLUMN_GAP_X), y: 0 };
}

export function nodeLayout(node: FlowNode): Layout {
  return {
    ...nodeSize(node),
    position: nodePositionInColumn(node),
  };
}

function nodeSize(_: FlowNode): Size {
  return { width: NODE_WIDTH, height: NODE_HEIGHT };
}

/** react-flow 就绪坐标：列容器为画布绝对坐标，事件节点为相对父列坐标 */
function nodePositionInColumn(node: FlowNode): Position {
  return {
    x: COLUMN_PADDING,
    y: COLUMN_HEADER_H + COLUMN_PADDING + node.order * (NODE_HEIGHT + NODE_GAP_Y),
  };
}

export function edgeHandle(
  edge: FlowEdge,
  graph: FlowGraph,
): { sourceHandle: string; targetHandle: string } | undefined {
  const sourceNode = graph.nodes.find((node) => node.id === edge.source);
  const targetNode = graph.nodes.find((node) => node.id === edge.target);
  if (!sourceNode || !targetNode) {
    return;
  }
  const isCrossPhase = sourceNode.parentId !== targetNode.parentId;
  const isForward = edge.kind === 'forward';
  const sourceHandle = isCrossPhase ? (isForward ? 'right' : 'left') : 'bottom';
  const targetHandle = isCrossPhase ? (isForward ? 'left' : 'right') : 'top';
  return { sourceHandle, targetHandle };
}
