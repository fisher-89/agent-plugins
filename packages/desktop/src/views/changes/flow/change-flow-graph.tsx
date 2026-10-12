import { ReactFlow, ReactFlowProvider, type Edge } from '@xyflow/react';
import { useMemo, type CSSProperties } from 'react';

import '@xyflow/react/dist/style.css';

import { graphTranslateExtent, columnLayout, nodeLayout, edgeHandle } from './layout';
import { type ColumnFlowNode, PhaseColumnNode } from './phase-column-node';
import { RunStepNode, type RunStepFlowNode } from './run-step-node';
import type { DrawerSelection, FlowEdgeKind, FlowGraph, FlowMaterials } from './types';

type ChartNode = ColumnFlowNode | RunStepFlowNode;

/** 边 kind 视觉：backtrack 虚线（回跳）+ feedback 虚线（phase 内回边，可辨）、
 * 其余实线。 */
function edgeStyle(kind: FlowEdgeKind): CSSProperties {
  if (kind === 'backtrack') return { stroke: 'var(--warn)', strokeDasharray: '6 4' };
  if (kind === 'feedback') return { stroke: 'var(--primary)', strokeDasharray: '4 3' };
  return { stroke: 'var(--border)' };
}

interface ChangeFlowGraphProps {
  graph: FlowGraph;
  materials: FlowMaterials;
  onSelect: (selection: DrawerSelection) => void;
}

const nodeTypes = { column: PhaseColumnNode, runStep: RunStepNode };

function toChartNodes(
  graph: FlowGraph,
  materials: FlowMaterials,
  onSelect: ChangeFlowGraphProps['onSelect'],
): ChartNode[] {
  const columns: ColumnFlowNode[] = graph.columns.map((column) => {
    return {
      ...columnLayout(column, graph),
      id: column.id,
      type: 'column',
      data: { phase: column.phase, docs: materials.columnDocs[column.id] ?? [], onSelect },
    };
  });
  const steps: RunStepFlowNode[] = graph.nodes.map((node) => {
    return {
      ...nodeLayout(node),
      id: node.id,
      parentId: node.parentId,
      extent: 'parent' as const,
      type: 'runStep',
      data: { node },
    };
  });
  return [...columns, ...steps];
}

function toChartEdges(graph: FlowGraph): Edge[] {
  return graph.edges.map((edge) => ({
    ...edgeHandle(edge, graph),
    id: edge.id,
    source: edge.source,
    target: edge.target,
    label: edge.label ?? undefined,
    style: edgeStyle(edge.kind),
    labelStyle: { fill: 'var(--warn)', fontSize: 10 },
    labelBgStyle: { fill: 'var(--warn-bg)' },
  }));
}

/** 画布内层 */
function FlowCanvas({ graph, materials, onSelect }: ChangeFlowGraphProps): React.JSX.Element {
  const nodes = useMemo(
    () => toChartNodes(graph, materials, onSelect),
    [graph, materials, onSelect],
  );
  const edges = useMemo(() => toChartEdges(graph), [graph]);
  const translateExtent = useMemo(() => graphTranslateExtent(graph), [graph]);
  const onNodeClick = (_event: unknown, node: ChartNode): void => {
    // 点击判据（节点点击判据与 stdout 输出面板）：无信息源的 runtime 节点不可
    // 点击——`role` 与 `outputRef` 皆 null（phaseStart / phaseLog / gate 三门 /
    // git 四步 / preflight / seal）时 detail 短文本已在节点内显示，抽屉无增量
    // 信息，不上抛 onSelect。列头点击经 PhaseColumnNode 本体上抛，不受本判据约束
    if (node.type !== 'runStep') return;
    if (node.data.node.role === null && node.data.node.outputRef === null) return;
    onSelect({ scope: 'node', nodeId: node.id });
  };
  return (
    <div
      className="flex-1 w-full overflow-hidden rounded-md bg-background"
      data-testid="flow-graph"
    >
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={onNodeClick}
        nodesDraggable={false}
        nodesConnectable={false}
        defaultViewport={{ x: 0, y: 0, zoom: 1 }}
        translateExtent={translateExtent}
        zoomOnScroll={false}
        zoomOnPinch={false}
        zoomOnDoubleClick={false}
        panOnScroll
      />
    </div>
  );
}

/**
 * 流程图渲染薄层：只做模型 → xyflow 节点 / 边的映射与交互上抛，无布局运算
 * （坐标全部来自 nodePosition）。节点点击经 onSelect 上抛（DrawerSelection）：
 * 无信息源（`role` 与 `outputRef` 皆 null）的 runtime 节点不上抛。
 */
export function ChangeFlowGraph(props: ChangeFlowGraphProps): React.JSX.Element {
  return (
    <ReactFlowProvider>
      <FlowCanvas {...props} />
    </ReactFlowProvider>
  );
}
