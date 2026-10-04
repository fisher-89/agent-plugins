import { ReactFlow, ReactFlowProvider, type Edge } from '@xyflow/react';
import { useMemo } from 'react';

import '@xyflow/react/dist/style.css';

import { type EventFlowNode, FlowEventNode } from './flow-event-node';
import { COL_W, COLUMN_HEADER_H, COLUMN_PAD_X, COLUMN_PAD_Y, ROW_H, nodePosition } from './layout';
import { type ColumnFlowNode, PhaseColumnNode } from './phase-column-node';
import { RunStepNode, type RunStepFlowNode } from './run-step-node';
import type { DrawerSelection, FlowEdge, FlowEdgeKind, FlowGraph, FlowMaterials } from './types';

type ChartNode = ColumnFlowNode | EventFlowNode | RunStepFlowNode;

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

const nodeTypes = { column: PhaseColumnNode, event: FlowEventNode, runStep: RunStepNode };

/** 列内事件数 → 列容器高度（COLUMN_HEADER_H + 列内节点数 × ROW_H + COLUMN_PAD_Y） */
function columnHeight(graph: FlowGraph, columnId: string): number {
  let count = 0;
  for (const node of graph.nodes) {
    if (node.parentId === columnId) count += 1;
  }
  return COLUMN_HEADER_H + count * ROW_H + COLUMN_PAD_Y;
}

/** 平移边距：内容包围盒向四周外扩量（视口最多越出节点范围至此） */
const PAN_MARGIN = 24;

/** 图内容包围盒 + PAN_MARGIN → translateExtent：平移不越出节点范围 + 少量边距 */
function graphTranslateExtent(graph: FlowGraph): [[number, number], [number, number]] {
  let maxX = 0;
  let maxY = 0;
  for (const column of graph.columns) {
    maxX = Math.max(maxX, nodePosition(column).x + COL_W);
    maxY = Math.max(maxY, columnHeight(graph, column.id));
  }
  return [
    [-PAN_MARGIN, -PAN_MARGIN],
    [maxX + PAN_MARGIN, maxY + PAN_MARGIN],
  ];
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
  const events: ChartNode[] = graph.nodes.flatMap((node): ChartNode[] => {
    const common = {
      id: node.id,
      parentId: node.parentId,
      extent: 'parent' as const,
      position: nodePosition(node),
      style: { width: COL_W - COLUMN_PAD_X * 2 },
    };
    if (node.kind === 'runtime') {
      return [
        {
          ...common,
          type: 'runStep',
          data: {
            node,
            // WorkerAgent 节点显式「查看会话」入口（上抛与节点点击同一
            // DrawerSelection，单一交互入口不变）；ToolStep / Gate 无入口
            onOpenSession:
              node.group === 'workerAgent'
                ? () => onSelect({ scope: 'node', nodeId: node.id })
                : undefined,
          },
        },
      ];
    }
    return [{ ...common, type: 'event', data: { node } }];
  });
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

/** 画布内层：默认 100% 视口；滚轮（含 Ctrl+滚轮）平移画布而非缩放，平移范围
 *  translateExtent 钳制在内容包围盒 + 少量边距内（无点背景，与周边 DOM 融合） */
function FlowCanvas({ graph, materials, onSelect }: ChangeFlowGraphProps): React.JSX.Element {
  const nodes = useMemo(
    () => toChartNodes(graph, materials, onSelect),
    [graph, materials, onSelect],
  );
  const edges = useMemo(() => toChartEdges(graph.edges), [graph.edges]);
  const translateExtent = useMemo(() => graphTranslateExtent(graph), [graph]);
  const onNodeClick = (_event: unknown, node: ChartNode): void => {
    // 事件节点与运行步节点共用同一抽屉入口（运行步节点抽屉含会话转录联动）
    if (node.type === 'event' || node.type === 'runStep') {
      onSelect({ scope: 'node', nodeId: node.id });
    }
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
        defaultViewport={{ x: 0, y: 0, zoom: 1 }}
        translateExtent={translateExtent}
        zoomOnScroll={false}
        zoomOnPinch={false}
        panOnScroll
      />
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
