import {
  Background,
  type Edge,
  ReactFlow,
  ReactFlowProvider,
  ViewportPortal,
  type Node,
} from '@xyflow/react';
import { useMemo } from 'react';

import { type ArchiElement, type ArchiGraph } from '@/types/generated/bindings';

import { shortName } from '../../lib/fqn';
import { edgeVisual } from '../../lib/marks';
import { type RelationEdge } from '../dashboard/use-archi-relation';
import { ElementNode, ElementGroupNode } from './element-node';

/** 邻域布局网格步长（纯拓扑定位，不引布局引擎）。 */
const STEP_X = 240;
const STEP_Y = 96;

/** 分组节点（有子节点的聚焦元素）与子节点行尺寸：标题栏 + 内容区留白 + 行距。 */
const GROUP_WIDTH = 240;
const GROUP_HEADER = 48;
const GROUP_PADDING = 16;
const CHILD_HEIGHT = 44;
const CHILD_GAP = 8;

/** 入边分区（`dependedBy` 虚线框）：内侧留白、分区宽度与邻居徽章定高（Badge h-5）。 */
const ZONE_PADDING = 20;
const ZONE_WIDTH = 200;
const ZONE_TITLE_HEIGHT = 30;
const PILL_HEIGHT = 20;

/** 初视口适配全图：留边距，单节点不过度放大。 */
const FIT_VIEW_OPTIONS = { padding: 0.2, minZoom: 0.2, maxZoom: 1 };

type ArchiNodeData = { label: string; mark: string };

type ArchiFlowNode = Node<ArchiNodeData, 'model' | 'modelGroup'>;

const nodeTypes = { model: ElementNode, modelGroup: ElementGroupNode };

type ZoneRect = { x: number; y: number; width: number; height: number };

function toNodes(graph: ArchiGraph, focus: string): ArchiFlowNode[] {
  const { outgoing, incoming, children } = neighborhood(graph, focus);
  const hasChildren = children.length > 0;
  const positions = neighborhoodPositions(focus, outgoing, incoming, children);
  const members = new Set<string>([focus, ...outgoing, ...incoming, ...children]);
  const byFqn = new Map<string, ArchiElement>(
    graph.elements.map((element) => [element.fqn, element]),
  );
  const nodes: ArchiFlowNode[] = [];
  for (const fqn of members) {
    const element = byFqn.get(fqn);
    const position = positions.get(fqn);
    if (element === undefined || position === undefined) continue;
    const isChild = children.includes(fqn);
    const isGroup = fqn === focus && hasChildren;
    // 尺寸即布局常量；measured 一并声明（子节点行 / 分组重建时不再丢 handle 面）
    const size = isGroup ? { width: GROUP_WIDTH, height: groupHeight(children.length) } : null;
    nodes.push({
      id: fqn,
      type: isGroup ? 'modelGroup' : 'model',
      parentId: isChild ? focus : undefined,
      extent: isChild ? 'parent' : undefined,
      ...(size === null ? {} : { ...size, measured: size }),
      position,
      data: { label: shortName(fqn), mark: element.mark },
    });
  }
  return nodes;
}

function toEdges(graph: ArchiGraph, focus: string, selectedKey: string | null): Edge[] {
  return graph.relations
    .filter((relation) => relation.from === focus || relation.to === focus)
    .map((relation) => {
      const visual = edgeVisual(relation.mark);
      const id = `${relation.from}->${relation.to}`;
      return {
        id,
        source: relation.from,
        target: relation.to,
        style: {
          stroke: visual.stroke,
          strokeDasharray: visual.dasharray ?? undefined,
          // 选中态只加粗既有描边（不引新标记词汇、不换色）
          ...(id === selectedKey ? { strokeWidth: 3 } : {}),
        },
      };
    });
}

/** 分组高度：标题栏 + 上下留白 + 子节点行（行距 CHILD_GAP）。 */
function groupHeight(childCount: number): number {
  return (
    GROUP_HEADER + GROUP_PADDING * 2 + childCount * CHILD_HEIGHT + (childCount - 1) * CHILD_GAP
  );
}

/** 邻域坐标（纯函数）：聚焦居中、出边向右、入边向左、子元素在聚焦节点内容区内
 * 自上而下堆叠。子节点在时聚焦节点成组：入 / 出边列按分组高度垂直居中，出边列
 * 让到分组右缘之外（绝对坐标）。 */
function neighborhoodPositions(
  focus: string,
  outgoing: string[],
  incoming: string[],
  children: string[],
): Map<string, { x: number; y: number }> {
  const positions = new Map<string, { x: number; y: number }>();
  positions.set(focus, { x: 0, y: 0 });
  const hasChildren = children.length > 0;
  const center = hasChildren ? groupHeight(children.length) / 2 : 0;
  const place = (fqns: string[], x: number, y0: number): void => {
    fqns.forEach((fqn, index) => {
      positions.set(fqn, { x, y: y0 + index * STEP_Y });
    });
  };
  place(incoming, -STEP_X, center - ((incoming.length - 1) * STEP_Y) / 2);
  place(
    outgoing,
    hasChildren ? GROUP_WIDTH + STEP_X : STEP_X,
    center - ((outgoing.length - 1) * STEP_Y) / 2,
  );
  children.forEach((fqn, index) => {
    positions.set(fqn, {
      x: GROUP_PADDING,
      y: GROUP_HEADER + GROUP_PADDING + index * (CHILD_HEIGHT + CHILD_GAP),
    });
  });
  return positions;
}

/** 邻域三项（与 `ModelGraph` 邻域查询同构：出边 / 入边 / 直接子元素）。 */
function neighborhood(
  graph: ArchiGraph,
  focus: string,
): { outgoing: string[]; incoming: string[]; children: string[] } {
  const outgoing = graph.relations
    .filter((relation) => relation.from === focus)
    .map((relation) => relation.to);
  const incoming = graph.relations
    .filter((relation) => relation.to === focus)
    .map((relation) => relation.from);
  const prefix = `${focus}.`;
  const children = graph.elements
    .filter(
      (element) =>
        element.fqn.startsWith(prefix) && !element.fqn.slice(prefix.length).includes('.'),
    )
    .map((element) => element.fqn);
  return { outgoing, incoming, children };
}

/** 入边分区矩形（纯函数）：框住整列入边节点（`dependedBy`）；无入边时无分区。 */
function incomingZone(graph: ArchiGraph, focus: string): ZoneRect | null {
  const { outgoing, incoming, children } = neighborhood(graph, focus);
  const first = incoming[0];
  const last = incoming.at(-1);
  if (first === undefined || last === undefined) return null;
  const positions = neighborhoodPositions(focus, outgoing, incoming, children);
  const firstPosition = positions.get(first);
  const lastPosition = positions.get(last);
  if (firstPosition === undefined || lastPosition === undefined) return null;
  return {
    x: firstPosition.x - ZONE_PADDING,
    y: firstPosition.y - ZONE_PADDING - ZONE_TITLE_HEIGHT,
    width: ZONE_WIDTH,
    height: lastPosition.y + PILL_HEIGHT - firstPosition.y + ZONE_TITLE_HEIGHT + ZONE_PADDING * 2,
  };
}

/** 入边分区（虚线框）：纯背景装饰，不进节点集、不响应点击（`pointer-events-none`）。 */
function IncomingZone({ zone }: { zone: ZoneRect }): React.JSX.Element {
  return (
    <ViewportPortal>
      <div
        className="pointer-events-none absolute -z-10 rounded-lg border border-dashed border-muted-foreground/50 p-2"
        style={{ left: zone.x, top: zone.y, width: zone.width, height: zone.height }}
        data-testid="archi-canvas-zone"
      >
        <span className="text-sm text-muted-foreground">被以下模块引用</span>
      </div>
    </ViewportPortal>
  );
}

/** 漂移取数失败行内提示（叠加信息，不阻断图）。 */
function DriftErrorNotice({ message }: { message: string }): React.JSX.Element {
  return (
    <div className="px-2 py-1 text-[11px] text-muted-foreground" data-testid="archi-drift-error">
      漂移取数失败：{message}（图照常渲染，零标记）
    </div>
  );
}

export function NodeRelationGraph({
  graph,
  focus,
  setFocusOverride,
  driftError,
  selectedEdge,
  onSelectEdge,
}: {
  graph: ArchiGraph;
  focus: string | null;
  setFocusOverride: (focus: string) => void;
  driftError: string | null;
  selectedEdge: RelationEdge | null;
  onSelectEdge: (edge: RelationEdge | null) => void;
}): React.JSX.Element {
  const nodes = useMemo(() => (focus === null ? [] : toNodes(graph, focus)), [graph, focus]);
  const selectedKey = selectedEdge === null ? null : `${selectedEdge.from}->${selectedEdge.to}`;
  const edges = useMemo(
    () => (focus === null ? [] : toEdges(graph, focus, selectedKey)),
    [graph, focus, selectedKey],
  );
  const zone = useMemo(() => (focus === null ? null : incomingZone(graph, focus)), [graph, focus]);
  return (
    <ReactFlowProvider>
      {driftError === null ? null : <DriftErrorNotice message={driftError} />}
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={(_event, node) => {
          onSelectEdge(null);
          setFocusOverride(node.id);
        }}
        onEdgeClick={(_event, edge) => onSelectEdge({ from: edge.source, to: edge.target })}
        nodesDraggable={false}
        nodesConnectable={false}
        fitView={true}
        fitViewOptions={FIT_VIEW_OPTIONS}
        zoomOnScroll={false}
        zoomOnDoubleClick={false}
        panOnScroll
      >
        <Background bgColor="black" />
        {zone === null ? null : <IncomingZone zone={zone} />}
      </ReactFlow>
    </ReactFlowProvider>
  );
}
