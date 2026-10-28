import {
  Handle,
  Position,
  ReactFlow,
  ReactFlowProvider,
  type Edge,
  type Node,
  type NodeProps,
} from '@xyflow/react';
import { useMemo, useState } from 'react';

import '@xyflow/react/dist/style.css';

import { Badge } from '@/components/ui/badge';
import type { ArchiElement, ArchiGraph, ArchiRelation } from '@/types/generated/bindings';

import { useArchiGraph } from '../hooks/use-archi-graph';
import { shortName } from '../lib/fqn';
import { edgeVisual } from '../lib/marks';
import { ElementDrawer, type CanvasMode } from './element-drawer';
import { ElementTree } from './element-tree';
import { StructureErrorPanel } from './structure-error-panel';

/** 邻域布局网格步长（纯拓扑定位，不引布局引擎）。 */
const STEP_X = 240;
const STEP_Y = 96;

/** 初视口适配全图：留边距，单节点不过度放大。 */
const FIT_VIEW_OPTIONS = { padding: 0.2, minZoom: 0.2, maxZoom: 1 };

export interface ArchitectureCanvasProps {
  /** 观察根（`null` = 壳态无根：不发起取数） */
  root: string | null;
  /** 能力差异：`full` 全量只读 / `incremental` 增量可改档 */
  mode: CanvasMode;
}

type ArchiNodeData = { label: string; mark: string };

type ArchiFlowNode = Node<ArchiNodeData, 'archi'>;

/** 隐形把手（连线锚点：入边左 / 出边右）；xyflow 无把手即不求边位置，边不入 DOM。 */
function NodeHandles(): React.JSX.Element {
  const rounded = {
    width: '100%',
    height: '100%',
    top: 0,
    left: 0,
    opacity: 0,
    transform: 'none',
  };
  return (
    <>
      <Handle type="target" position={Position.Left} isConnectable={false} style={rounded} />
      <Handle type="source" position={Position.Right} isConnectable={false} style={rounded} />
    </>
  );
}

/** 节点视图（本文件内局部组件——不为单节点单建文件）。 */
function ArchiNodeView({ data }: NodeProps<ArchiFlowNode>): React.JSX.Element {
  return (
    <Badge variant="secondary" className="p-3" data-testid="archi-canvas-node">
      <NodeHandles />
      {data.label}
    </Badge>
  );
}

const nodeTypes = { archi: ArchiNodeView };

/** 缺省聚焦：首个顶层元素，无顶层则首个元素。 */
function defaultFocus(graph: ArchiGraph): string | null {
  const root = graph.elements.find((element) => !element.fqn.includes('.'));
  return root?.fqn ?? graph.elements[0]?.fqn ?? null;
}

/** 邻域坐标（纯函数）：聚焦居中、出边向右、入边向左、子元素下方（网格堆叠）。 */
function neighborhoodPositions(
  focus: string,
  outgoing: string[],
  incoming: string[],
  children: string[],
): Map<string, { x: number; y: number }> {
  const positions = new Map<string, { x: number; y: number }>();
  positions.set(focus, { x: 0, y: 0 });
  const place = (fqns: string[], x: number, y0: number): void => {
    fqns.forEach((fqn, index) => {
      positions.set(fqn, { x, y: y0 + index * STEP_Y });
    });
  };
  place(incoming, -STEP_X, -((incoming.length - 1) * STEP_Y) / 2);
  place(outgoing, STEP_X, -((outgoing.length - 1) * STEP_Y) / 2);
  place(children, 0, STEP_Y);
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

function toNodes(graph: ArchiGraph, focus: string): ArchiFlowNode[] {
  const { outgoing, incoming, children } = neighborhood(graph, focus);
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
    nodes.push({
      id: fqn,
      type: 'archi',
      position,
      data: { label: shortName(fqn), mark: element.mark },
    });
  }
  return nodes;
}

function toEdges(graph: ArchiGraph, focus: string): Edge[] {
  return graph.relations
    .filter((relation) => relation.from === focus || relation.to === focus)
    .map((relation) => {
      const visual = edgeVisual(relation.mark);
      return {
        id: `${relation.from}->${relation.to}`,
        source: relation.from,
        target: relation.to,
        style: {
          stroke: visual.stroke,
          strokeDasharray: visual.dasharray ?? undefined,
        },
      };
    });
}

/** 正常态三段布局（元素树 / 画布 / 抽屉）。 */
function CanvasBody({
  graph,
  root,
  mode,
  onChanged,
}: {
  graph: ArchiGraph;
  root: string | null;
  mode: CanvasMode;
  onChanged: () => void;
}): React.JSX.Element {
  const [focusOverride, setFocusOverride] = useState<string | null>(null);
  const focus = useMemo(() => {
    if (focusOverride !== null && graph.elements.some((element) => element.fqn === focusOverride)) {
      return focusOverride;
    }
    return defaultFocus(graph);
  }, [graph, focusOverride]);
  const nodes = useMemo(() => (focus === null ? [] : toNodes(graph, focus)), [graph, focus]);
  const edges = useMemo(() => (focus === null ? [] : toEdges(graph, focus)), [graph, focus]);
  const focused =
    focus === null ? null : (graph.elements.find((element) => element.fqn === focus) ?? null);
  const relations: ArchiRelation[] = graph.relations;
  if (graph.elements.length === 0) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-canvas-empty">
        模型为空：先在 change 的 dev-design 阶段生成骨架（新元素缺省 gate = off）。
      </div>
    );
  }
  return (
    <div className="flex h-full min-h-0 gap-2">
      <ElementTree elements={graph.elements} focused={focus} onFocus={setFocusOverride} />
      <div
        className="min-h-0 flex-1 overflow-hidden rounded-md bg-background"
        data-testid="archi-canvas"
      >
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodeClick={(_event, node) => setFocusOverride(node.id)}
          nodesDraggable={false}
          nodesConnectable={false}
          fitView={true}
          fitViewOptions={FIT_VIEW_OPTIONS}
          zoomOnScroll={false}
          zoomOnDoubleClick={false}
          panOnScroll
        />
      </div>
      <ElementDrawer
        root={root}
        mode={mode}
        element={focused}
        relations={relations}
        onChanged={onChanged}
      />
    </div>
  );
}

export function ArchitectureCanvas({ root, mode }: ArchitectureCanvasProps): React.JSX.Element {
  const { graph, loading, error, refresh } = useArchiGraph(root);
  if (loading && graph === null) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-canvas-loading">
        解析中…
      </div>
    );
  }
  if (error !== null) {
    return (
      <div
        className="break-all rounded-md bg-fail-bg px-3 py-2 text-[13px] text-fail"
        data-testid="archi-canvas-error"
      >
        {error}
      </div>
    );
  }
  if (graph === null) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-canvas-empty">
        无工作区根：无法解析架构模型。
      </div>
    );
  }
  // 全或无停摆：任何结构错 ⇒ 只渲错误面板（零图数据、零降级样式）
  if (graph.diagnostics.length > 0) {
    return (
      <div data-testid="archi-canvas">
        <StructureErrorPanel diagnostics={graph.diagnostics} />
      </div>
    );
  }
  return (
    <ReactFlowProvider>
      <CanvasBody graph={graph} root={root} mode={mode} onChanged={refresh} />
    </ReactFlowProvider>
  );
}
