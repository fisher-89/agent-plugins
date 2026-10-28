import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type * as XyflowModule from '@xyflow/react';
import { createElement, type ComponentType } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  ArchiDriftReport,
  ArchiElement,
  ArchiGraph,
  ArchiRelation,
} from '../../../types/generated/bindings';
import { ArchitectureCanvas } from './architecture-canvas';

/**
 * `ArchitectureCanvas` 单测（AC-4 / AC-18 / D9 / D21）：全或无停摆、正常态三段
 * 布局、缺省 1-hop 聚焦、聚焦切换零取数、空模型与取数失败两态、`mode` 透传、
 * 改档后刷新的三触发纪律。
 *
 * 渲染面断言分两路：节点 / 边集合、边 style、节点坐标、`ReactFlow` 选项
 * 经**观测垫片**取 `ReactFlow` 的入参（非语义替身：真实 `ReactFlow` 照常渲染，
 * 垫片只在渲染前记录入参）；边另有一条**真实 DOM** 断言（`.react-flow__edge`）——
 * xyflow 只在节点挂 Handle 后才为边求位置，堵「props 有边、DOM 无边」的回归盲区。
 * 节点集合 / 坐标 / 文案一律从 jsdom 真实 DOM 断言。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（`archi_graph` /
 * `archi_module` / `archi_set_gate` 按命令名分发）。xyflow 度量垫片
 * （ResizeObserver / offsetWidth / DOMMatrixReadOnly / getBBox）照抄
 * `change-flow-graph.test.tsx`——环境缺口而非业务 mock。
 */

const { invokeMock, flowProps } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  flowProps: [] as unknown[],
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

vi.mock('@xyflow/react', async (importOriginal) => {
  const actual = await importOriginal<typeof XyflowModule>();
  // 观测垫片：记录入参后交真实 `ReactFlow` 渲染（forwardRef 对象不可直接调用）
  const ReactFlowProbe = (props: unknown): unknown => {
    flowProps.push(props);
    return createElement(actual.ReactFlow as unknown as ComponentType, props as never);
  };
  return { ...actual, ReactFlow: ReactFlowProbe };
});

/** 观测到的 `ReactFlow` 入参（本组件用到的子集）。 */
interface FlowProps {
  nodes: {
    id: string;
    type: string;
    position: { x: number; y: number };
    data: { label: string; mark: string };
  }[];
  edges: {
    id: string;
    source: string;
    target: string;
    label?: string;
    style: { stroke: string; strokeDasharray?: string };
    labelStyle: { fill: string; fontSize: number };
  }[];
  nodeTypes: Record<string, unknown>;
  nodesDraggable: boolean;
  nodesConnectable: boolean;
  zoomOnScroll: boolean;
  zoomOnDoubleClick: boolean;
  panOnScroll: boolean;
  fitView?: boolean;
  fitViewOptions?: { padding?: number; minZoom?: number; maxZoom?: number };
}

/** 最近一次 `ReactFlow` 入参（每次渲染追加入栈）。 */
function flow(): FlowProps {
  const last = flowProps.at(-1);
  if (last === undefined) throw new Error('ReactFlow 尚未渲染');
  return last as FlowProps;
}

class ResizeObserverStub {
  static instances: ResizeObserverStub[] = [];
  callback: ResizeObserverCallback;
  elements: Element[] = [];

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    ResizeObserverStub.instances.push(this);
  }

  observe = (target: Element): void => {
    this.elements.push(target);
  };

  unobserve = (target: Element): void => {
    this.elements = this.elements.filter((element) => element !== target);
  };

  disconnect = (): void => {
    this.elements = [];
  };

  /** 重放全部观测目标，触发 xyflow 的节点度量（handleBounds / 尺寸）更新。 */
  static flush(): void {
    for (const observer of ResizeObserverStub.instances) {
      if (observer.elements.length === 0) continue;
      const entries = observer.elements.map(
        (target) =>
          ({
            target,
            contentRect: {
              width: target.clientWidth,
              height: target.clientHeight,
              x: 0,
              y: 0,
              top: 0,
              left: 0,
              bottom: 0,
              right: 0,
            },
          }) as unknown as ResizeObserverEntry,
      );
      observer.callback(entries, observer);
    }
  }
}

const nativeOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth');
const nativeOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');

/** 边视觉常量（`marks.ts` 四值；本期图数据含 none / unfulfilled / crossKind 三值）。 */
const NEUTRAL = { stroke: 'var(--chart-2)', strokeDasharray: undefined };
const UNFULFILLED = { stroke: 'var(--muted-foreground)', strokeDasharray: '6 4' };
const CROSS_KIND = { stroke: 'var(--warn)', strokeDasharray: '4 3' };

function element(fqn: string, overrides: Partial<ArchiElement> = {}): ArchiElement {
  return {
    fqn,
    kind: 'module',
    gate: 'off',
    path: [],
    summary: null,
    metadata: {},
    modelFile: '.dev-team/architecture/models/01-core.toml',
    modelLine: 2,
    mark: 'none',
    ...overrides,
  };
}

function relation(
  from: string,
  to: string,
  description: string | null,
  mark: ArchiRelation['mark'] = 'none',
): ArchiRelation {
  return { from, to, description, mark, reason: null };
}

/** 健康图：四元素 / 三条关系（`App` 为唯一顶层）。 */
function healthyGraph(): ArchiGraph {
  return {
    elements: [
      element('App'),
      element('App.Auth', { gate: 'strict' }),
      element('App.Billing'),
      element('App.Billing.Invoice'),
    ],
    relations: [
      relation('App.Auth', 'App.Billing', '认证调用计费'),
      relation('App.Billing.Invoice', 'App.Billing', '发票属于计费'),
      relation('App.Auth', 'App', null),
    ],
    diagnostics: [],
  };
}

/**
 * 邻域图：聚焦 `Svc` 的 1-hop 三面刻意互不重叠（入边 3 / 出边 2 + 悬空 1 /
 * 直接子元素 2），并含四类「不该进邻域」的邻居：孙元素 `Svc.A.Deep`、点前缀
 * 相近的 `SvcX`、只与邻域成员相连的无关边 `Src1 -> Dst1`、以及未声明目标
 * `Ghost`（关系引用不存在的元素）。
 */
function hubGraph(): ArchiGraph {
  return {
    elements: [
      element('Svc', { kind: 'package', mark: 'unfulfilled' }),
      element('Src1'),
      element('Src2'),
      element('Src3'),
      element('Dst1'),
      element('Dst2'),
      element('Svc.A'),
      element('Svc.B'),
      element('Svc.A.Deep'),
      element('SvcX'),
    ],
    relations: [
      relation('Src1', 'Svc', '入边一'),
      relation('Src2', 'Svc', null),
      relation('Src3', 'Svc', '入边三', 'unfulfilled'),
      relation('Svc', 'Dst1', '出边一'),
      relation('Svc', 'Dst2', null, 'crossKind'),
      relation('Svc', 'Ghost', '悬空去向'),
      relation('Src1', 'Dst1', '无关边'),
    ],
    diagnostics: [],
  };
}

function detailOf(fqn: string) {
  return {
    element: element(fqn),
    dependsOn: [],
    dependedBy: [],
    children: [],
    docs: [],
    warnings: [],
  };
}

function callsOf(command: string): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === command)
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

/** 漂移报告 fixture（两段与状态可指定；其余字段取稳定默认值）。 */
function driftReport(
  options: {
    observations?: ArchiDriftReport['observations'];
    suppressed?: ArchiDriftReport['suppressed'];
    status?: ArchiDriftReport['status'];
  } = {},
): ArchiDriftReport {
  return {
    version: 1,
    timestamp: '2026-10-20T08:31:12Z',
    mode: 'full',
    root: 'C:demoalpha',
    change: null,
    base: null,
    modelHash: 'sha256:00ff',
    model: {
      elements: 2,
      relations: 1,
      byGate: { strict: 0, warn: 0, off: 2 },
      structuralErrors: [],
      warnings: [],
    },
    scan: {
      mode: 'full',
      changedFiles: 0,
      scannedFiles: 2,
      excluded: [],
      unmatchedFiles: [],
      matched: [],
      unresolved: [],
    },
    errors: [],
    warnings: [],
    suppressed: options.suppressed ?? [],
    observations: options.observations ?? [],
    status: options.status ?? 'clean',
  };
}

/** 缺省取数实现：`archi_graph` 回传入图、`archi_module` 回传入 fqn 的详情、
 * `archi_drift` 回零标记报告（漂移面叠加信息，缺省不干扰既有断言）。 */
function serveGraph(graph: ArchiGraph): void {
  serveGraphAndDrift(graph, driftReport());
}

/** 图 + 漂移两路取数装置（漂移报告可指定，用于标记上边 / 节点用例）。 */
function serveGraphAndDrift(graph: ArchiGraph, report: ArchiDriftReport | Error): void {
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_graph') return Promise.resolve(graph);
    if (command === 'archi_drift') {
      return report instanceof Error ? Promise.reject(report) : Promise.resolve(report);
    }
    if (command === 'archi_module') {
      const fqn = typeof params?.element === 'string' ? params.element : '';
      return Promise.resolve(detailOf(fqn));
    }
    if (command === 'archi_set_gate') return Promise.resolve(null);
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
}

/** 画布内渲染出的节点 id 清单（jsdom 真实 DOM，`rf__node-<id>`）。 */
function renderedNodeIds(): string[] {
  return screen.queryAllByTestId(/^rf__node-/).map((node) => node.getAttribute('data-id') ?? '');
}

/** 单个节点的坐标（xyflow 节点包裹层的 transform）。 */
function nodeTransform(fqn: string): string {
  return screen.getByTestId(`rf__node-${fqn}`).style.transform;
}

/** 单个节点的文案（短名；节点状态点已退役）。 */
function nodeText(fqn: string): string {
  const view = within(screen.getByTestId(`rf__node-${fqn}`)).getByTestId('archi-canvas-node');
  return view.textContent ?? '';
}

beforeEach(() => {
  invokeMock.mockReset();
  flowProps.length = 0;
  serveGraph(healthyGraph());

  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
    configurable: true,
    get: () => 100,
  });
  Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
    configurable: true,
    get: () => 100,
  });
  vi.stubGlobal(
    'DOMMatrixReadOnly',
    class {
      m22 = 1;
    },
  );
  const svgPrototype = globalThis.SVGElement?.prototype as unknown as
    | Record<string, unknown>
    | undefined;
  if (svgPrototype !== undefined && typeof svgPrototype.getBBox !== 'function') {
    svgPrototype.getBBox = () => ({ x: 0, y: 0, width: 0, height: 0 });
  }
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  ResizeObserverStub.instances = [];
  if (nativeOffsetWidth !== undefined) {
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', nativeOffsetWidth);
  }
  if (nativeOffsetHeight !== undefined) {
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', nativeOffsetHeight);
  }
});

/** 渲染后重放节点度量（须在 act 内触发 store 更新；handleBounds 就位后边才进 DOM）。 */
async function measureNodes(): Promise<void> {
  await act(async () => {
    ResizeObserverStub.flush();
    await Promise.resolve();
  });
}

describe('停摆态（全或无）', () => {
  it('应在 diagnostics 非空时只渲错误面板（零节点零边、零元素树节点、零 ReactFlow）', async () => {
    serveGraph({
      elements: [],
      relations: [],
      diagnostics: [
        {
          kind: 'cycle',
          message: '依赖成环：App.A -> App.B -> App.A',
          file: '.dev-team/architecture/models/01-core.toml',
          line: 4,
          column: 1,
        },
      ],
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-error-panel')).toBeTruthy());
    expect(screen.getByTestId('archi-error-panel').textContent).toContain('cycle');
    expect(screen.getByTestId('archi-canvas')).toBeTruthy();
    expect(screen.queryAllByTestId('archi-tree-node')).toHaveLength(0);
    expect(screen.queryAllByTestId(/^rf__node-/)).toHaveLength(0);
    expect(screen.queryAllByTestId('archi-canvas-node')).toHaveLength(0);
    // 停摆不落图：ReactFlow 一次都不挂
    expect(flowProps).toHaveLength(0);
    expect(screen.queryByTestId('archi-drawer')).toBeNull();
  });
});

describe('正常态：缺省 1-hop 渲染面', () => {
  it('应渲染三段布局并以首个顶层元素为聚焦（抽屉 fqn 与取数参数一致）', async () => {
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.getByTestId('archi-canvas')).toBeTruthy();
    expect(screen.getByTestId('archi-drawer')).toBeTruthy();

    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    expect(callsOf('archi_module')[0]).toEqual({ root: 'C:\\demo\\alpha', element: 'App' });
    expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('App');
  });

  it('应只把聚焦元素的 1-hop 成员画成节点（出边 / 入边 / 直接子元素，孙元素与点前缀兄弟不入图）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    // 成员集 = 聚焦 + 出边（Dst1 / Dst2 / 悬空 Ghost 无元素体⇒跳过）+ 入边（Src1..3）+ 直接子元素（Svc.A / Svc.B）
    expect(renderedNodeIds().sort()).toEqual(
      ['Dst1', 'Dst2', 'Src1', 'Src2', 'Src3', 'Svc', 'Svc.A', 'Svc.B'].sort(),
    );
    expect(flow().nodes.map((node) => node.id)).toEqual([
      'Svc',
      'Dst1',
      'Dst2',
      'Src1',
      'Src2',
      'Src3',
      'Svc.A',
      'Svc.B',
    ]);
    // 孙元素 / 点前缀相近的兄弟 / 未声明目标 零出现
    for (const excluded of ['Svc.A.Deep', 'SvcX', 'Ghost']) {
      expect(screen.queryByTestId(`rf__node-${excluded}`)).toBeNull();
    }
  });

  it('应按纯拓扑网格给出节点坐标（聚焦居中 / 入边左 / 出边右 / 子元素下方）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    expect(nodeTransform('Svc')).toBe('translate(0px,0px)');
    // 入边 3 条：x = -240，y 以 (n-1)*96/2 居中 ⇒ -96 / 0 / +96
    expect(nodeTransform('Src1')).toBe('translate(-240px,-96px)');
    expect(nodeTransform('Src2')).toBe('translate(-240px,0px)');
    expect(nodeTransform('Src3')).toBe('translate(-240px,96px)');
    // 出边 3 条（含指向未声明元素的 Ghost，其无元素体不入图）：x = +240，y = -96 / 0 / +96
    expect(nodeTransform('Dst1')).toBe('translate(240px,-96px)');
    expect(nodeTransform('Dst2')).toBe('translate(240px,0px)');
    // 直接子元素：x = 0，y = 96 / 192（自上而下堆叠）
    expect(nodeTransform('Svc.A')).toBe('translate(0px,96px)');
    expect(nodeTransform('Svc.B')).toBe('translate(0px,192px)');

    expect(flow().nodes.map((node) => node.position)).toEqual([
      { x: 0, y: 0 },
      { x: 240, y: -96 },
      { x: 240, y: 0 },
      { x: -240, y: -96 },
      { x: -240, y: 0 },
      { x: -240, y: 96 },
      { x: 0, y: 96 },
      { x: 0, y: 192 },
    ]);
  });

  it('应按短名渲染节点文案（末段短名，节点不呈标记点）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    expect(nodeText('Svc')).toBe('Svc');
    expect(nodeText('Src1')).toBe('Src1');
    expect(nodeText('Svc.A')).toBe('A');
    expect(nodeText('Svc.B')).toBe('B');
    // 节点类型固定为局部组件 `archi`
    expect(flow().nodes.every((node) => node.type === 'archi')).toBe(true);
    expect(Object.keys(flow().nodeTypes)).toEqual(['archi']);
  });

  it('应只画与聚焦相连的边（id / 端点 / style 逐值），无关边零出现', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    const edges = flow().edges;
    expect(edges.map((edge) => edge.id)).toEqual([
      'Src1->Svc',
      'Src2->Svc',
      'Src3->Svc',
      'Svc->Dst1',
      'Svc->Dst2',
      'Svc->Ghost',
    ]);
    expect(edges.map((edge) => `${edge.source}->${edge.target}`)).toEqual([
      'Src1->Svc',
      'Src2->Svc',
      'Src3->Svc',
      'Svc->Dst1',
      'Svc->Dst2',
      'Svc->Ghost',
    ]);
    // style 由 marks.ts 四值确定性给出（none 实线 / unfulfilled 灰虚线 / crossKind 橙虚线）
    expect(edges.map((edge) => edge.style)).toStrictEqual([
      NEUTRAL,
      NEUTRAL,
      UNFULFILLED,
      NEUTRAL,
      CROSS_KIND,
      NEUTRAL,
    ]);
    // 无关边（Src1 -> Dst1）不入图
    expect(edges.some((edge) => edge.id === 'Src1->Dst1')).toBe(false);
  });

  it('应在真实 DOM 渲染连线（隐形把手就位、悬空关系无节点不入图）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    // 每节点一对隐形把手：出边右、入边左（xyflow 据此求边坐标）
    const svc = screen.getByTestId('rf__node-Svc');
    expect(svc.querySelectorAll('.react-flow__handle.source')).toHaveLength(1);
    expect(svc.querySelectorAll('.react-flow__handle.target')).toHaveLength(1);

    await measureNodes();
    // 6 条关系里 `Svc -> Ghost` 指向未声明元素（无节点体）⇒ 只渲 5 条
    await waitFor(() => expect(document.querySelectorAll('.react-flow__edge')).toHaveLength(5));
    for (const id of ['Src1->Svc', 'Src2->Svc', 'Src3->Svc', 'Svc->Dst1', 'Svc->Dst2']) {
      expect(screen.queryByTestId(`rf__edge-${id}`)).not.toBeNull();
    }
    expect(screen.queryByTestId('rf__edge-Svc->Ghost')).toBeNull();

    // 样式同 props 面：none 实线 ×3 / unfulfilled 灰虚线 / crossKind 橙虚线
    const styles = [...document.querySelectorAll<SVGPathElement>('.react-flow__edge-path')].map(
      (path) => path.getAttribute('style') ?? '',
    );
    expect(styles).toHaveLength(5);
    expect(styles.filter((style) => style.includes('var(--chart-2)'))).toHaveLength(3);
    expect(
      styles.filter((style) => style.includes('var(--muted-foreground)') && style.includes('6 4')),
    ).toHaveLength(1);
    expect(
      styles.filter((style) => style.includes('var(--warn)') && style.includes('4 3')),
    ).toHaveLength(1);
  });

  it('应以只读交互选项装配 ReactFlow（禁拖 / 禁连 / 禁缩放 / 初视口适配全图）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    const props = flow();
    expect(props.nodesDraggable).toBe(false);
    expect(props.nodesConnectable).toBe(false);
    expect(props.zoomOnScroll).toBe(false);
    expect(props.zoomOnDoubleClick).toBe(false);
    expect(props.panOnScroll).toBe(true);
    expect(props.fitView).toBe(true);
    expect(props.fitViewOptions).toEqual({ padding: 0.2, minZoom: 0.2, maxZoom: 1 });
    expect(props.nodes).toHaveLength(8);
    expect(props.edges).toHaveLength(6);
  });
});

describe('缺省聚焦与 1-hop 边界', () => {
  it('应跳过首个元素不是顶层元素的声明序，聚焦到首个顶层元素', async () => {
    serveGraph({
      elements: [element('A.B'), element('A'), element('A.C')],
      relations: [],
      diagnostics: [],
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    // 首个元素 `A.B` 带 `.` ⇒ 不是顶层；`A` 才是缺省聚焦
    // 节点 DOM 在测量回调之后才挂载，候等条件须覆盖节点集（只等抽屉文案会读到空集）
    await waitFor(() => {
      expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('A');
      expect(renderedNodeIds().sort()).toEqual(['A', 'A.B', 'A.C']);
    });
    expect(nodeTransform('A')).toBe('translate(0px,0px)');
    // 直接子元素按声明序堆叠在下方
    expect(nodeTransform('A.B')).toBe('translate(0px,96px)');
    expect(nodeTransform('A.C')).toBe('translate(0px,192px)');
  });

  it('应在无任何顶层元素时以首个元素为聚焦', async () => {
    serveGraph({
      elements: [element('A.B'), element('A.C')],
      relations: [],
      diagnostics: [],
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => {
      expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('A.B');
      expect(renderedNodeIds()).toEqual(['A.B']);
    });
  });

  it('应在元素树点击后按新邻域重算且零取数（archi_graph 计数不变）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getAllByTestId('archi-tree-node').length).toBeGreaterThan(0));
    const graphCalls = callsOf('archi_graph').length;

    const src1Node = screen
      .getAllByTestId('archi-tree-node')
      .find((node) => node.getAttribute('data-fqn') === 'Src1')!;
    fireEvent.click(src1Node);
    await waitFor(() => expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('Src1'));

    // 聚焦 Src1 的邻域：出边（Svc / Dst1）+ 无入边 + 无子元素
    expect(renderedNodeIds().sort()).toEqual(['Dst1', 'Src1', 'Svc']);
    expect(flow().edges.map((edge) => edge.id)).toEqual(['Src1->Svc', 'Src1->Dst1']);
    expect(nodeTransform('Src1')).toBe('translate(0px,0px)');
    expect(nodeTransform('Svc')).toBe('translate(240px,-48px)');
    expect(nodeTransform('Dst1')).toBe('translate(240px,48px)');

    expect(callsOf('archi_graph')).toHaveLength(graphCalls);
    expect(callsOf('archi_module').at(-1)).toEqual({
      root: 'C:\\demo\\alpha',
      element: 'Src1',
    });
  });

  it('应在画布节点点击时切换聚焦（节点 id 即 FQN，零取数）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Dst1')).toBeTruthy());
    const graphCalls = callsOf('archi_graph').length;

    fireEvent.click(screen.getByTestId('rf__node-Dst1'));
    await waitFor(() => expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('Dst1'));
    // 聚焦 Dst1：入边 Svc / Src1（无关边）+ 无出边 + 无子元素
    expect(renderedNodeIds().sort()).toEqual(['Dst1', 'Src1', 'Svc']);
    expect(callsOf('archi_graph')).toHaveLength(graphCalls);
  });
});

describe('漂移面接线（AC-19）', () => {
  it('应在有报告时把未兑现声明的边与节点标成 unfulfilled', async () => {
    serveGraphAndDrift(
      healthyGraph(),
      driftReport({
        observations: [
          {
            kind: 'unfulfilledDeclaration',
            from: 'App.Auth',
            to: 'App',
            message: '本次扫描面内未见 App.Auth → App 的 import',
          },
        ],
      }),
    );
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());

    // 节点：不呈标记点，只呈短名（节点标记不呈现）
    expect(screen.queryAllByTestId('archi-node-mark')).toHaveLength(0);

    // 边：命中项转灰虚线（unfulfilled），未命中项保持实线
    await measureNodes();
    const styles = [...document.querySelectorAll<SVGPathElement>('.react-flow__edge-path')].map(
      (path) => path.getAttribute('style') ?? '',
    );
    expect(styles).toHaveLength(1);
    expect(
      styles.filter((style) => style.includes('var(--muted-foreground)') && style.includes('6 4')),
    ).toHaveLength(1);
    expect(styles.filter((style) => style.includes('var(--chart-2)'))).toHaveLength(0);
  });

  it('应在豁免留痕时把边标成 exempt（灰实线），节点标记零改动', async () => {
    serveGraphAndDrift(
      healthyGraph(),
      driftReport({
        suppressed: [
          {
            from: 'App.Auth',
            to: 'App',
            via: 'config',
            reason: '迁移期允许',
            file: null,
            line: null,
          },
        ],
      }),
    );
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());
    expect(screen.queryAllByTestId('archi-node-mark')).toHaveLength(0);

    await measureNodes();
    const styles = [...document.querySelectorAll<SVGPathElement>('.react-flow__edge-path')].map(
      (path) => path.getAttribute('style') ?? '',
    );
    // exempt = 灰实线（无 dasharray）
    expect(styles).toHaveLength(1);
    expect(
      styles.filter((style) => style.includes('var(--muted-foreground)') && !style.includes('6 4')),
    ).toHaveLength(1);
  });

  it('应在漂移取数失败时照常渲染零标记图并呈现行内提示', async () => {
    serveGraphAndDrift(healthyGraph(), new Error('archi_drift 不可用'));
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drift-error')).toBeTruthy());
    expect(screen.getByTestId('archi-drift-error').textContent).toContain('漂移取数失败');
    expect(screen.queryByTestId('archi-canvas-error')).toBeNull();
    await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());
    expect(screen.queryAllByTestId('archi-node-mark')).toHaveLength(0);
  });

  it('应在 clean / skipped / noChanges 三态下全图零标记且节点边照常渲染', async () => {
    const edgeStyles = (): string[] =>
      [...document.querySelectorAll<SVGPathElement>('.react-flow__edge-path')].map(
        (path) => path.getAttribute('style') ?? '',
      );

    // 基线：干净报告的节点 / 边数（三态逐值对照「与无漂移时相等」）
    serveGraphAndDrift(healthyGraph(), driftReport({ status: 'clean' }));
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());
    await measureNodes();
    const baselineNodes = renderedNodeIds().length;
    const baselineEdges = edgeStyles().length;
    expect(baselineNodes).toBeGreaterThan(0);
    expect(baselineEdges).toBeGreaterThan(0);
    cleanup();

    for (const status of ['clean', 'skipped', 'noChanges'] as const) {
      serveGraphAndDrift(healthyGraph(), driftReport({ status }));
      render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
      await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());
      await measureNodes();

      // 全图零标记：边样式全为正常色（无灰 / 无虚线），节点不呈标记点
      expect(screen.queryAllByTestId('archi-node-mark')).toHaveLength(0);
      const styles = edgeStyles();
      expect(styles.every((style) => style.includes('var(--chart-2)'))).toBe(true);
      expect(styles.some((style) => style.includes('var(--muted-foreground)'))).toBe(false);

      // 节点 / 边照常渲染（与无漂移基线逐值相等——三态不塌图）
      expect(renderedNodeIds().length).toBe(baselineNodes);
      expect(styles.length).toBe(baselineEdges);
      cleanup();
    }
  });

  it('应在 archi_graph 与 archi_drift 双失败时以图错误为优先', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_graph') return Promise.reject(new Error('模型解析失败（root 无效）'));
      if (command === 'archi_drift') return Promise.reject(new Error('archi_drift 不可用'));
      if (command === 'archi_module') return Promise.resolve(detailOf('App'));
      return Promise.resolve(null);
    });
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);

    // 图错误面接管：既有的错误面板在场且载图失败文案
    await waitFor(() => expect(screen.getByTestId('archi-canvas-error')).toBeTruthy());
    expect(screen.getByTestId('archi-canvas-error').textContent).toContain('模型解析失败');
    // 漂移失败提示 MUST NOT 掩盖图错误（两失败不同权）
    expect(screen.queryByTestId('archi-drift-error')).toBeNull();
    expect(screen.queryAllByTestId('archi-canvas-node')).toHaveLength(0);
    expect(screen.queryAllByTestId(/^rf__node-/)).toHaveLength(0);
  });

  it('应在两种挂载形态下把 mode 与 root 逐值透传给 archi_drift', async () => {
    for (const mode of ['full', 'incremental'] as const) {
      invokeMock.mockClear();
      serveGraphAndDrift(healthyGraph(), driftReport());
      render(<ArchitectureCanvas root={'C:demoalpha'} mode={mode} />);
      await waitFor(() => expect(callsOf('archi_drift').length).toBeGreaterThan(0));
      expect(callsOf('archi_drift').at(-1)).toEqual({ root: 'C:demoalpha', mode });
      cleanup();
    }
  });

  it('应在聚焦切换后保持 archi_drift 调用次数不变（三触发纪律）', async () => {
    serveGraphAndDrift(healthyGraph(), driftReport());
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-App.Auth')).toBeTruthy());
    const before = callsOf('archi_drift').length;
    expect(before).toBeGreaterThan(0);

    // 画布节点点击切换聚焦
    fireEvent.click(screen.getByTestId('rf__node-App.Auth'));
    await waitFor(() =>
      expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('App.Auth'),
    );

    // 元素树点击再切一次（第二触发面）
    const billing = screen
      .getAllByTestId('archi-tree-node')
      .find((node) => node.getAttribute('data-fqn') === 'App.Billing');
    expect(billing).toBeTruthy();
    if (billing !== undefined) fireEvent.click(billing);
    await waitFor(() =>
      expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('App.Billing'),
    );

    // 零取数：一个宏任务窗口过后计数不增（聚焦不是取数触发）
    await act(async () => {
      await Promise.resolve();
    });
    expect(callsOf('archi_drift')).toHaveLength(before);
  });
});

describe('空态 / 加载态 / 取数失败 / 无根', () => {
  it('应在空模型（三数组皆空且零诊断）时呈现空态而非停摆面板', async () => {
    serveGraph({ elements: [], relations: [], diagnostics: [] });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-canvas-empty')).toBeTruthy());
    expect(screen.queryByTestId('archi-error-panel')).toBeNull();
    expect(screen.getByTestId('archi-canvas-empty').textContent).toContain('模型为空');
    expect(screen.queryByTestId('archi-canvas-node')).toBeNull();
  });

  it('应在首轮未回时呈现加载态（不提前落无根空态）', async () => {
    let resolveGraph: ((graph: ArchiGraph) => void) | undefined;
    invokeMock.mockImplementation((command: string) => {
      // 进程边界按命令名分发：图取数挂起、漂移取数立即返回零标记报告
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      return new Promise<ArchiGraph>((resolve) => {
        resolveGraph = resolve;
      });
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-canvas-loading')).toBeTruthy());
    expect(screen.getByTestId('archi-canvas-loading').textContent).toContain('解析中');
    expect(screen.queryByTestId('archi-canvas-empty')).toBeNull();

    await act(async () => {
      resolveGraph?.(healthyGraph());
    });
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.queryByTestId('archi-canvas-loading')).toBeNull();
  });

  it('应在 root 为 null 时不发 invoke 并呈现无根空态', async () => {
    render(<ArchitectureCanvas root={null} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-canvas-empty')).toBeTruthy());
    expect(screen.getByTestId('archi-canvas-empty').textContent).toContain('无工作区根');
    expect(callsOf('archi_graph')).toHaveLength(0);
  });

  it('应在 archi_graph reject 时呈现错误面板（零节点零边）并在 root 变化后恢复', async () => {
    invokeMock.mockImplementationOnce(() => Promise.reject(new Error('root 无效（C:）')));
    const { rerender } = render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-canvas-error')).toBeTruthy());
    expect(screen.getByTestId('archi-canvas-error').textContent).toContain('root 无效');
    expect(screen.queryAllByTestId('archi-tree-node')).toHaveLength(0);
    expect(screen.queryAllByTestId(/^rf__node-/)).toHaveLength(0);

    rerender(<ArchitectureCanvas root={'C:\\demo\\beta'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
  });

  it('应在 root 由 A 换到 B 时重新取数（新根新图）', async () => {
    const { rerender } = render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(callsOf('archi_graph')).toHaveLength(1));
    rerender(<ArchitectureCanvas root={'C:\\demo\\beta'} mode="full" />);
    await waitFor(() => expect(callsOf('archi_graph')).toHaveLength(2));
    expect(callsOf('archi_graph')[1]).toEqual({ root: 'C:\\demo\\beta' });
  });
});

describe('mode 差异与改档刷新', () => {
  it('应在 full 模式下抽屉零 gate 控件', async () => {
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-gate-readonly')).toBeTruthy());
    expect(screen.queryByTestId('archi-gate-select')).toBeNull();
  });

  it('应在 incremental 模式下提供 gate 控件并在成功后重取一次 archi_graph', async () => {
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-gate-select')).toBeTruthy());
    const before = callsOf('archi_graph').length;

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-gate-submit'));
    });
    await waitFor(() => expect(callsOf('archi_graph').length).toBe(before + 1));
    expect(callsOf('archi_set_gate')).toHaveLength(1);
  });

  it('应在改档失败时不重取并保留错误呈现', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_graph') return Promise.resolve(healthyGraph());
      if (command === 'archi_module') return Promise.resolve(detailOf('App'));
      return Promise.reject(new Error('元素不存在：App'));
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-gate-submit')).toBeTruthy());
    const before = callsOf('archi_graph').length;
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-gate-submit'));
    });
    await waitFor(() => expect(screen.getByTestId('archi-gate-error')).toBeTruthy());
    expect(callsOf('archi_graph')).toHaveLength(before);
  });

  it('应在刷新后聚焦元素消失时回落到缺省聚焦（不画空图）', async () => {
    const before: ArchiGraph = {
      elements: [element('Root'), element('Root.Gone'), element('Root.Keep')],
      relations: [],
      diagnostics: [],
    };
    const after: ArchiGraph = {
      elements: [element('Root'), element('Root.Keep')],
      relations: [],
      diagnostics: [],
    };
    let graphCalls = 0;
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'archi_graph') {
        graphCalls += 1;
        return Promise.resolve(graphCalls === 1 ? before : after);
      }
      if (command === 'archi_module') {
        const fqn = typeof params?.element === 'string' ? params.element : '';
        return Promise.resolve(detailOf(fqn));
      }
      return Promise.resolve(null);
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Root.Gone')).toBeTruthy());

    // 聚焦 `Root.Gone` 后改档触发重取，新图里该元素已删
    await act(async () => {
      fireEvent.click(screen.getByTestId('rf__node-Root.Gone'));
    });
    await waitFor(() =>
      expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('Root.Gone'),
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-gate-submit'));
    });
    await waitFor(() => expect(callsOf('archi_graph')).toHaveLength(2));

    await waitFor(() => expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('Root'));
    expect(renderedNodeIds().sort()).toEqual(['Root', 'Root.Keep']);
  });
});

describe('停摆与文档警告无关', () => {
  it('应不因文档警告停摆（diagnostics 空即正常态，文档缺失不进图面判据）', async () => {
    serveGraph(healthyGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.queryByTestId('archi-error-panel')).toBeNull();
    await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());
  });
});
