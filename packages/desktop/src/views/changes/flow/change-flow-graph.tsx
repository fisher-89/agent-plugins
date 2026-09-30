import {
  Background,
  BackgroundVariant,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Edge,
} from '@xyflow/react';
import { useEffect, useMemo } from 'react';

import '@xyflow/react/dist/style.css';

import { type EventFlowNode, FlowEventNode } from './flow-event-node';
import { COL_W, COLUMN_HEADER_H, COLUMN_PAD_X, COLUMN_PAD_Y, ROW_H, nodePosition } from './layout';
import { type ColumnFlowNode, PhaseColumnNode } from './phase-column-node';
import type { DrawerSelection, FlowEdge, FlowEdgeKind, FlowGraph, FlowMaterials } from './types';

type ChartNode = ColumnFlowNode | EventFlowNode;

/**
 * 边 kind → 锚定把手 id（对应 FlowEventNode 四侧隐形 Handle）：布局为
 * 9 列横向排布，跨列边水平锚定（前进右出左入、回跳左出右入），同列
 * 重试保持垂直锚定（下出上入）。
 */
const EDGE_HANDLES: Record<FlowEdgeKind, { source: string; target: string }> = {
  forward: { source: 'right', target: 'left' },
  backtrack: { source: 'left', target: 'right' },
  retry: { source: 'bottom', target: 'top' },
};

interface ChangeFlowGraphProps {
  graph: FlowGraph;
  materials: FlowMaterials;
  onSelect: (selection: DrawerSelection) => void;
}

const nodeTypes = { column: PhaseColumnNode, event: FlowEventNode };

const fitOptions = { padding: 0.1, maxZoom: 1 };

/** 列内事件数 → 列容器高度（COLUMN_HEADER_H + 列内节点数 × ROW_H + COLUMN_PAD_Y） */
function columnHeight(graph: FlowGraph, columnId: string): number {
  let count = 0;
  for (const node of graph.nodes) {
    if (node.parentId === columnId) count += 1;
  }
  return COLUMN_HEADER_H + count * ROW_H + COLUMN_PAD_Y;
}

function toChartNodes(
  graph: FlowGraph,
  materials: FlowMaterials,
  onSelect: ChangeFlowGraphProps['onSelect'],
): ChartNode[] {
  const columns: ChartNode[] = graph.columns.map((column) => ({
    id: column.id,
    type: 'column',
    position: nodePosition(column),
    style: { width: COL_W, height: columnHeight(graph, column.id) },
    data: { phase: column.phase, docs: materials.columnDocs[column.id] ?? [], onSelect },
  }));
  const events: ChartNode[] = graph.nodes.map((node) => ({
    id: node.id,
    type: 'event',
    parentId: node.parentId,
    extent: 'parent',
    position: nodePosition(node),
    style: { width: COL_W - COLUMN_PAD_X * 2 },
    data: { node },
  }));
  return [...columns, ...events];
}

function toChartEdges(edges: FlowEdge[]): Edge[] {
  return edges.map((edge) => ({
    id: edge.id,
    source: edge.source,
    target: edge.target,
    sourceHandle: EDGE_HANDLES[edge.kind].source,
    targetHandle: EDGE_HANDLES[edge.kind].target,
    label: edge.label ?? undefined,
    style:
      edge.kind === 'backtrack'
        ? { stroke: 'var(--warn)', strokeDasharray: '6 4' }
        : { stroke: 'var(--border)' },
    labelStyle: { fill: 'var(--warn)', fontSize: 10 },
    labelBgStyle: { fill: 'var(--warn-bg)' },
  }));
}

/** 画布内层：图数据引用变化（显式 refresh 后新 graph）时重新 fitView */
function FlowCanvas({ graph, materials, onSelect }: ChangeFlowGraphProps): React.JSX.Element {
  const { fitView } = useReactFlow();
  const nodes = useMemo(
    () => toChartNodes(graph, materials, onSelect),
    [graph, materials, onSelect],
  );
  const edges = useMemo(() => toChartEdges(graph.edges), [graph.edges]);
  useEffect(() => {
    void fitView({ ...fitOptions, duration: 120 });
  }, [graph, fitView]);
  const onNodeClick = (_event: unknown, node: ChartNode): void => {
    if (node.type === 'event') onSelect({ scope: 'node', nodeId: node.id });
  };
  return (
    <div
      className="h-[440px] w-full overflow-hidden rounded-md bg-background"
      data-testid="flow-graph"
    >
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={onNodeClick}
        nodesDraggable={false}
        nodesConnectable={false}
        fitView
        fitViewOptions={fitOptions}
        minZoom={0.15}
      >
        <Background variant={BackgroundVariant.Dots} />
      </ReactFlow>
    </div>
  );
}

/**
 * 流程图渲染薄层：只做模型 → xyflow 节点 / 边的映射与交互上抛，无布局运算
 * （坐标全部来自 nodePosition）。节点点击经 onSelect 上抛（DrawerSelection）。
 */
export function ChangeFlowGraph(props: ChangeFlowGraphProps): React.JSX.Element {
  return (
    <ReactFlowProvider>
      <FlowCanvas {...props} />
    </ReactFlowProvider>
  );
}
