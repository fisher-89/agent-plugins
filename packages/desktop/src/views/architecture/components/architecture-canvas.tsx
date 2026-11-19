import {
  ReactFlow,
  ReactFlowProvider,
  type Edge,
  type Node,
  ViewportPortal,
  Background,
} from '@xyflow/react';
import { useMemo, useState } from 'react';

import '@xyflow/react/dist/style.css';

import {
  Card,
  CardContent,
  CardHeader,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import type {
  ArchiApiFinding,
  ArchiElement,
  ArchiGraph,
  ArchiRelation,
} from '@/types/generated/bindings';

import { useArchiDrift } from '../hooks/use-archi-drift';
import { useArchiGraph } from '../hooks/use-archi-graph';
import { driftMarks } from '../lib/drift-marks';
import { shortName } from '../lib/fqn';
import { edgeVisual } from '../lib/marks';
import { ContractPanel } from './contract-panel';
import { ElementTree } from './element-tree';
import { FileTree, type TreeMode } from './file-tree';
import { ModelGroupNode } from './graph/model-group-node';
import { ModelNode } from './graph/model-node';
import { ModelInfo, type CanvasMode } from './model-info';
import { StructureErrorPanel } from './structure-error-panel';

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

export interface ArchitectureCanvasProps {
  /** 观察根（`null` = 壳态无根：不发起取数） */
  root: string | null;
  /** 能力差异：`full` 全量只读 / `incremental` 增量可改档 */
  mode: CanvasMode;
}

type ArchiNodeData = { label: string; mark: string };

type ArchiFlowNode = Node<ArchiNodeData, 'model' | 'modelGroup'>;

const nodeTypes = { model: ModelNode, modelGroup: ModelGroupNode };

/** 缺省聚焦：首个顶层元素，无顶层则首个元素。 */
function defaultFocus(graph: ArchiGraph): string | null {
  const root = graph.elements.find((element) => !element.fqn.includes('.'));
  return root?.fqn ?? graph.elements[0]?.fqn ?? null;
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

type ZoneRect = { x: number; y: number; width: number; height: number };

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

/** 左栏树栏位：分段控件 + 三态行（缺省文件树，可切元素树 / 契约视图）。切换只换左栏
 * 组件，`focus` 共用；取数失败只落本栏行内提示，画布与抽屉照常渲染。 */
function TreeColumn({
  root,
  canvasMode,
  mode,
  onModeChange,
  elements,
  focused,
  onFocus,
  findings,
}: {
  root: string | null;
  canvasMode: CanvasMode;
  mode: TreeMode;
  onModeChange: (mode: TreeMode) => void;
  elements: ArchiElement[];
  focused: string | null;
  onFocus: (fqn: string) => void;
  findings: ArchiApiFinding[];
}): React.JSX.Element {
  const options: { mode: TreeMode; label: string; content: React.JSX.Element }[] = [
    {
      mode: 'file',
      label: '文件',
      content: <FileTree root={root} focused={focused} onFocus={onFocus} />,
    },
    {
      mode: 'element',
      label: '元素',
      content: <ElementTree elements={elements} focused={focused} onFocus={onFocus} />,
    },
    {
      mode: 'contract',
      label: '契约',
      content: (
        <ContractPanel root={root} mode={canvasMode} findings={findings} onFocus={onFocus} />
      ),
    },
  ];
  return (
    <Tabs className="bg-card p-4 w-full h-full" value={mode} onValueChange={onModeChange}>
      <TabsList variant="line" className="mb-2" data-testid="archi-tree-mode" role="group">
        {options.map((option) => (
          <TabsTrigger value={option.mode}>{option.label}</TabsTrigger>
        ))}
      </TabsList>
      {options.map((option) => (
        <TabsContent value={option.mode}>{option.content}</TabsContent>
      ))}
    </Tabs>
  );
}

/** 空模型提示（结构性早退分支，画布零节点零边）。 */
function ModelEmptyNotice(): React.JSX.Element {
  return (
    <div className="text-[13px] text-muted-foreground" data-testid="archi-canvas-empty">
      模型为空：先在 change 的 dev-design 阶段生成骨架（新元素缺省 gate = off）。
    </div>
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

function ModelRelationGraph({
  graph,
  focus,
  setFocusOverride,
  driftError,
}: {
  graph: ArchiGraph;
  focus: string | null;
  setFocusOverride: (focus: string) => void;
  driftError: string | null;
}): React.JSX.Element {
  const nodes = useMemo(() => (focus === null ? [] : toNodes(graph, focus)), [graph, focus]);
  const edges = useMemo(() => (focus === null ? [] : toEdges(graph, focus)), [graph, focus]);
  const zone = useMemo(() => (focus === null ? null : incomingZone(graph, focus)), [graph, focus]);
  return (
    <ReactFlowProvider>
      {driftError === null ? null : <DriftErrorNotice message={driftError} />}
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
      >
        <Background bgColor="black" />
        {zone === null ? null : <IncomingZone zone={zone} />}
      </ReactFlow>
    </ReactFlowProvider>
  );
}

/** 正常态三段布局（左栏树——缺省文件树，可切元素树 / 画布 / 抽屉）。漂移取数失败时
 * 零标记照常渲染 + 画布区顶部一行内提示（叠加信息不阻断图）。 */
function CanvasBody({
  graph,
  root,
  mode,
  onChanged,
  driftError,
  apiFindings,
}: {
  graph: ArchiGraph;
  root: string | null;
  mode: CanvasMode;
  onChanged: () => void;
  driftError: string | null;
  apiFindings: ArchiApiFinding[];
}): React.JSX.Element {
  const [focusOverride, setFocusOverride] = useState<string | null>(null);
  const [treeMode, setTreeMode] = useState<TreeMode>('file');
  const focus = useMemo(() => {
    if (focusOverride !== null && graph.elements.some((element) => element.fqn === focusOverride)) {
      return focusOverride;
    }
    return defaultFocus(graph);
  }, [graph, focusOverride]);
  const focused =
    focus === null ? null : (graph.elements.find((element) => element.fqn === focus) ?? null);
  const relations: ArchiRelation[] = graph.relations;
  if (graph.elements.length === 0) {
    return <ModelEmptyNotice />;
  }
  return (
    <ResizablePanelGroup className="flex h-full min-h-0 gap-2">
      <ResizablePanel defaultSize={300} minSize={200}>
        <TreeColumn
          root={root}
          canvasMode={mode}
          mode={treeMode}
          onModeChange={setTreeMode}
          elements={graph.elements}
          focused={focus}
          onFocus={setFocusOverride}
          findings={apiFindings}
        />
      </ResizablePanel>
      <ResizableHandle />
      <ResizablePanel>
        <Card className="h-full flex flex-col">
          <CardHeader>
            <ModelInfo
              root={root}
              mode={mode}
              element={focused}
              relations={relations}
              findings={apiFindings}
              onChanged={onChanged}
            />
          </CardHeader>
          <CardContent className="min-h-0 flex-1 rounded-md" data-testid="archi-canvas">
            <ModelRelationGraph
              graph={graph}
              focus={focus}
              setFocusOverride={setFocusOverride}
              driftError={driftError}
            />
          </CardContent>
        </Card>
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}

export function ArchitectureCanvas({ root, mode }: ArchitectureCanvasProps): React.JSX.Element {
  const { graph, loading, error, refresh } = useArchiGraph(root);
  // 漂移取数：与图同一观察根、模式随挂载点分流（`full` / `incremental`）；
  // 标记生产在 drift-marks 纯函数，本组件只做一行合并
  const { report, error: driftError } = useArchiDrift(root, mode);
  const marked = useMemo(
    () => (graph === null ? null : driftMarks(report, graph)),
    [graph, report],
  );
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
  if (graph === null || marked === null) {
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
    <CanvasBody
      graph={marked}
      root={root}
      mode={mode}
      onChanged={refresh}
      driftError={driftError}
      apiFindings={report?.apiFindings ?? []}
    />
  );
}
