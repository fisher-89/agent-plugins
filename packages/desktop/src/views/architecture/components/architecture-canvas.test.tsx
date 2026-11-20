import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type * as XyflowModule from '@xyflow/react';
import { createElement, type ComponentType } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  ArchiDriftReport,
  ArchiElement,
  ArchiFileEntry,
  ArchiFileTree,
  ArchiGraph,
  ArchiRelation,
  ArchiRelationDetail,
} from '../../../types/generated/bindings';
import { ArchitectureCanvas } from './architecture-canvas';

/**
 * `ArchitectureCanvas` 单测（AC-4 / AC-18 / D9 / D21）：全或无停摆、正常态两栏
 * 布局、缺省 1-hop 聚焦、聚焦切换零取数、空模型与取数失败两态、`mode` 透传、
 * 改档后刷新的三触发纪律。
 *
 * 左栏缺省视图 = 元素树（文件树 / 契约面板只在切到对应分段时挂载）——「缺省视图」
 * 判据一律以元素树在场 + `archi_file_tree` 调用计数表达；`archi-file-tree` 的否定
 * 断言只出现在另有正锚的行旁（元素树在场 / 契约面板在场 / 空模型早退 / 失败行 /
 * 加载占位），禁区是「只用单一否定断言判缺省」。
 *
 * 渲染面断言分两路：节点 / 边集合、边 style、节点坐标、`ReactFlow` 选项
 * 经**观测垫片**取 `ReactFlow` 的入参（非语义替身：真实 `ReactFlow` 照常渲染，
 * 垫片只在渲染前记录入参）；边另有一条**真实 DOM** 断言（`.react-flow__edge`）——
 * xyflow 只在节点挂 Handle 后才为边求位置，堵「props 有边、DOM 无边」的回归盲区。
 * 节点集合 / 坐标 / 文案一律从 jsdom 真实 DOM 断言。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（按命令名分发 `archi_graph` /
 * `archi_module` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / 契约两命令）。
 * xyflow 度量垫片（ResizeObserver / offsetWidth / DOMMatrixReadOnly / getBBox）照抄
 * `change-flow-graph.test.tsx`——环境缺口而非业务 mock。非活动分段面板由 base-ui
 * `Tabs`（`keepMounted = false`）真实卸载——这正是「非活动面板不挂载 ⇒ 首屏零
 * 取数」的判据本体，替身化会让本单测自证。
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
    parentId?: string;
    extent?: string;
    width?: number;
    height?: number;
    measured?: { width: number; height: number };
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
  public static instances: ResizeObserverStub[] = [];
  public elements: Element[] = [];

  constructor(private callback: ResizeObserverCallback) {
    ResizeObserverStub.instances.push(this);
  }

  public observe = (target: Element): void => {
    this.elements.push(target);
  };

  public unobserve = (target: Element): void => {
    this.elements = this.elements.filter((element) => element !== target);
  };

  public disconnect = (): void => {
    this.elements = [];
  };

  /** 重放全部观测目标，触发 xyflow 的节点度量（handleBounds / 尺寸）更新。 */
  public static flush(): void {
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
            // react-resizable-panels 经 borderBoxSize[0] 判在测面板尺寸；
            // 缺省形状只有 contentRect 会让其面板回调读到 undefined[0]。
            borderBoxSize: [{ inlineSize: target.clientWidth, blockSize: target.clientHeight }],
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
    api: 'declared',
    path: [],
    summary: null,
    metadata: {},
    modelFile: '.dev-team/architecture/graph/model/01-core.toml',
    modelLine: 2,
    entry: null,
    doc: null,
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
  return { from, to, description, mark, reason: null, apis: [] };
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
    // 契约状态投影（`ArchiModuleDetail.api` 为必填）：`declared` 才渲契约段与判据列表
    // （`none` 走一行空态）；三路径此处不落事务态，取 `null`（行不渲）
    api: { status: 'declared', contractPath: null, schemaPath: null, generatedPath: null },
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
    apiFindings: [],
    status: options.status ?? 'clean',
  };
}

/** 文件树条目构造器（七字段全给缺省值）。 */
function fileEntry(path: string, overrides: Partial<ArchiFileEntry> = {}): ArchiFileEntry {
  return {
    path,
    name: path.split('/').at(-1) ?? path,
    kind: 'file',
    mark: 'none',
    owners: [],
    contains: 0,
    exists: true,
    ...overrides,
  };
}

/** 文件树 fixture 的归属 FQN 对（缺省对齐健康图的 `App` / `App.Auth`）。 */
const TREE_OWNERS = { root: 'App', leaf: 'App.Auth' } as const;

/** 文件树 fixture：根元素两行同 owners（多 path 形态）+ 叶元素一行。 */
function fileTree(owners: { root: string; leaf: string } = TREE_OWNERS): ArchiFileTree {
  return {
    entries: [
      fileEntry('crates/core/archi', {
        kind: 'dir',
        mark: 'elementRoot',
        owners: [owners.root],
        contains: 1,
      }),
      fileEntry('crates/core/archi/src', { kind: 'dir', mark: 'member', owners: [owners.root] }),
      fileEntry('crates/core/archi/src/lib.rs', { mark: 'member', owners: [owners.root] }),
      fileEntry('src/auth', { kind: 'dir', mark: 'elementRoot', owners: [owners.leaf] }),
    ],
  };
}

/** 缺省取数实现：`archi_graph` 回传入图、`archi_module` 回传入 fqn 的详情、
 * `archi_drift` 回零标记报告、`archi_file_tree` 回缺省文件树（左栏数据面）。 */
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
    if (command === 'archi_file_tree') return Promise.resolve(fileTree());
    if (command === 'archi_set_gate') return Promise.resolve(null);
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
}

/** 切到某左栏分段（分段控件为 base-ui Tabs，钮以 `role="tab"` + 文案寻址）。 */
function switchTo(label: '文件' | '元素' | '契约'): void {
  fireEvent.click(within(screen.getByTestId('archi-tree-mode')).getByRole('tab', { name: label }));
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
      public m22 = 1;
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
          file: '.dev-team/architecture/graph/model/01-core.toml',
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
  it('应渲染三段布局并以首个顶层元素为聚焦（左栏缺省为元素树）', async () => {
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    // 正锚：左栏缺省分支是元素树（缺省视图判据 MUST NOT 只由否定断言承担）
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.getAllByTestId('archi-tree-node').length).toBeGreaterThan(0);
    expect(screen.getByTestId('archi-tree-mode')).toBeTruthy();
    expect(screen.getByTestId('archi-canvas')).toBeTruthy();
    expect(screen.getByTestId('archi-drawer')).toBeTruthy();
    // 文件树不参与首屏：既不挂载也不取数
    expect(callsOf('archi_file_tree')).toHaveLength(0);
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();
    expect(screen.queryByTestId('archi-file-tree-loading')).toBeNull();

    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));
    expect(callsOf('archi_module')[0]).toEqual({ root: 'C:\\demo\\alpha', element: 'App' });
    expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('App');
    expect(callsOf('archi_graph')).toHaveLength(1);
  });

  it('应在点「文件」后恰取数一次（载荷逐字）且切回元素态零新增取数', async () => {
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    const baseline = {
      graph: callsOf('archi_graph').length,
      module: callsOf('archi_module').length,
      drift: callsOf('archi_drift').length,
    };
    expect(callsOf('archi_file_tree')).toHaveLength(0);

    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
    expect(callsOf('archi_file_tree')).toHaveLength(1);
    expect(callsOf('archi_file_tree')[0]).toEqual({ root: 'C:\\demo\\alpha' });
    expect(screen.getAllByTestId('archi-file-node').length).toBeGreaterThan(0);
    // 两视图不共存：元素树已卸载（正锚 = 文件树在场 + 计数）
    expect(screen.queryByTestId('archi-element-tree')).toBeNull();
    // 切换只换左栏组件：图 / 抽屉 / 漂移计数不变
    expect(callsOf('archi_graph')).toHaveLength(baseline.graph);
    expect(callsOf('archi_module')).toHaveLength(baseline.module);
    expect(callsOf('archi_drift')).toHaveLength(baseline.drift);

    switchTo('元素');
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(callsOf('archi_file_tree')).toHaveLength(1);
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

  it('应按纯拓扑网格给出节点坐标（子元素在分组内容区内，出边列避让分组右缘）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    expect(nodeTransform('Svc')).toBe('translate(0px,0px)');
    // 入边 3 条：x = -240；纵向以分组高度 176 的中线（88）居中 ⇒ -8 / 88 / 184
    expect(nodeTransform('Src1')).toBe('translate(-240px,-8px)');
    expect(nodeTransform('Src2')).toBe('translate(-240px,88px)');
    expect(nodeTransform('Src3')).toBe('translate(-240px,184px)');
    // 出边列在分组右缘（宽 240）之外一格 ⇒ x = 480；纵向同上居中
    expect(nodeTransform('Dst1')).toBe('translate(480px,-8px)');
    expect(nodeTransform('Dst2')).toBe('translate(480px,88px)');
    // 直接子元素在分组内容区内（相对分组左上角：x = 16，y = 64 / 116）
    expect(nodeTransform('Svc.A')).toBe('translate(16px,64px)');
    expect(nodeTransform('Svc.B')).toBe('translate(16px,116px)');

    expect(flow().nodes.map((node) => node.position)).toEqual([
      { x: 0, y: 0 },
      { x: 480, y: -8 },
      { x: 480, y: 88 },
      { x: -240, y: -8 },
      { x: -240, y: 88 },
      { x: -240, y: 184 },
      { x: 16, y: 64 },
      { x: 16, y: 116 },
    ]);
  });

  it('应把直接子元素挂到聚焦分组节点下（parentId / extent，尺寸随内容）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    const byId = new Map(flow().nodes.map((node) => [node.id, node]));
    expect(byId.get('Svc')).toMatchObject({
      type: 'modelGroup',
      width: 240,
      height: 176,
      measured: { width: 240, height: 176 },
    });
    expect(byId.get('Svc')?.parentId).toBeUndefined();
    for (const child of ['Svc.A', 'Svc.B']) {
      expect(byId.get(child)).toMatchObject({
        type: 'model',
        parentId: 'Svc',
        extent: 'parent',
      });
      // 子节点不声明尺寸：徽章随短名自适应
      expect(byId.get(child)?.width).toBeUndefined();
      expect(byId.get(child)?.height).toBeUndefined();
    }
    // 子节点行完整落在分组区域内（行高按布局常量 44）
    const group = byId.get('Svc')!;
    for (const childId of ['Svc.A', 'Svc.B']) {
      const child = byId.get(childId)!;
      expect(child.position.x).toBeGreaterThanOrEqual(0);
      expect(child.position.y + 44).toBeLessThanOrEqual(group.height ?? 0);
    }
    // 邻域成员不入分组：无 parentId / extent / 定尺
    for (const fqn of ['Src1', 'Src2', 'Src3', 'Dst1', 'Dst2']) {
      const node = byId.get(fqn);
      expect(node?.parentId).toBeUndefined();
      expect(node?.extent).toBeUndefined();
      expect(node?.width).toBeUndefined();
      expect(node?.height).toBeUndefined();
    }
  });

  it('应以虚线分区框住入边列（dependedBy），分区不入节点集', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());
    await waitFor(() => expect(screen.getByTestId('archi-canvas-zone')).toBeTruthy());

    // 入边 3 条列首 y = -8、列尾 y = 184；分区外扩 20、徽章定高 20、标题带高 30
    const zone = screen.getByTestId('archi-canvas-zone');
    expect(zone.style.left).toBe('-260px');
    expect(zone.style.top).toBe('-58px');
    expect(zone.style.width).toBe('200px');
    expect(zone.style.height).toBe('282px');
    // 分区是背景装饰：不产生节点、不吞点击
    expect(renderedNodeIds()).toHaveLength(8);
    expect(zone.className.split(/\s+/)).toContain('pointer-events-none');
  });

  it('应在无入边时不渲分区（只框 dependedBy，不框出边）', async () => {
    serveGraph({
      elements: [element('Solo'), element('Peer')],
      relations: [relation('Solo', 'Peer', null)],
      diagnostics: [],
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Solo')).toBeTruthy());
    expect(screen.queryByTestId('archi-canvas-zone')).toBeNull();
  });

  it('应按短名渲染节点文案（末段短名，节点不呈标记点）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());

    expect(nodeText('Svc')).toBe('Svc');
    expect(nodeText('Src1')).toBe('Src1');
    expect(nodeText('Svc.A')).toBe('A');
    expect(nodeText('Svc.B')).toBe('B');
    // 节点类型：聚焦元素有子节点 ⇒ 分组节点，其余（含子节点）为叶子节点
    expect(flow().nodes.map((node) => `${node.id}:${node.type}`)).toEqual([
      'Svc:modelGroup',
      'Dst1:model',
      'Dst2:model',
      'Src1:model',
      'Src2:model',
      'Src3:model',
      'Svc.A:model',
      'Svc.B:model',
    ]);
    expect(Object.keys(flow().nodeTypes)).toEqual(['model', 'modelGroup']);
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
    // 直接子元素在分组内容区内：x = 16，y = 64 / 116（声明序自上而下）
    expect(nodeTransform('A.B')).toBe('translate(16px,64px)');
    expect(nodeTransform('A.C')).toBe('translate(16px,116px)');
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
    // 缺省即元素树：无需切换即可点击
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

    // 文件树归属行点击再切一次（第二触发面：切到「文件」后文件树挂载）
    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
    const fileRow = screen
      .getAllByTestId('archi-file-node')
      .find((node) => node.getAttribute('data-path') === 'crates/core/archi');
    expect(fileRow).toBeTruthy();
    if (fileRow !== undefined) fireEvent.click(fileRow);
    await waitFor(() => expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('App'));

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
      // 进程边界按命令名分发：图取数挂起、漂移与文件树取数立即返回
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      if (command === 'archi_file_tree') return Promise.resolve(fileTree());
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
    expect(callsOf('archi_file_tree')).toHaveLength(0);
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
    expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(0);
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

describe('左栏文件树接线（AC-11 / AC-12 / AC-13）', () => {
  it('应缺省渲元素树且切到文件树时 testid 换面（两视图不共存）', async () => {
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    // 正锚：元素树在场 + 文件树零取数（否定断言只作补充）
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.getAllByTestId('archi-tree-node').length).toBeGreaterThan(0);
    expect(callsOf('archi_file_tree')).toHaveLength(0);

    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
    expect(screen.getAllByTestId('archi-file-node').length).toBeGreaterThan(0);
    expect(callsOf('archi_file_tree')).toHaveLength(1);
    expect(screen.queryByTestId('archi-element-tree')).toBeNull();

    switchTo('元素');
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.getAllByTestId('archi-tree-node').length).toBeGreaterThan(0);
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();
    expect(callsOf('archi_file_tree')).toHaveLength(1);
  });

  it('应在两视图往返切换后只让文件树随重挂重取（图 / 抽屉 / 漂移零重取）', async () => {
    serveGraph(hubGraph());
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    await waitFor(() => expect(callsOf('archi_module')).toHaveLength(1));

    const before = {
      graph: callsOf('archi_graph').length,
      module: callsOf('archi_module').length,
      drift: callsOf('archi_drift').length,
      tree: callsOf('archi_file_tree').length,
    };
    // 缺省视图零文件树取数：基准必为 0（切换后才有第 1 次）
    expect(before.tree).toBe(0);

    for (let round = 1; round <= 3; round += 1) {
      switchTo('文件');
      await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
      expect(callsOf('archi_file_tree')).toHaveLength(before.tree + round);

      switchTo('元素');
      await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
      // 非活动分段面板整块卸载：元素态下文件树零节点
      expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(0);
    }
    await act(async () => {
      await Promise.resolve();
    });

    expect(callsOf('archi_graph')).toHaveLength(before.graph);
    expect(callsOf('archi_module')).toHaveLength(before.module);
    expect(callsOf('archi_drift')).toHaveLength(before.drift);
    // 每次切回「文件」都重挂 FileTree，其自身取数随之重发 ⇒ 累计 = 基准 + 3
    expect(callsOf('archi_file_tree')).toHaveLength(before.tree + 3);
  });

  it('应在点文件树归属行后圈定范围并让抽屉取该元素（链路：tree → focus → drawer）', async () => {
    serveGraph(hubGraph());
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'archi_graph') return Promise.resolve(hubGraph());
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      if (command === 'archi_file_tree')
        return Promise.resolve(fileTree({ root: 'Svc', leaf: 'Src1' }));
      if (command === 'archi_module') {
        const fqn = typeof params?.element === 'string' ? params.element : '';
        return Promise.resolve(detailOf(fqn));
      }
      return Promise.resolve(null);
    });
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    // 文件树是懒取数：切到「文件」分段才挂载
    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());

    const row = screen
      .getAllByTestId('archi-file-node')
      .find((node) => node.getAttribute('data-path') === 'crates/core/archi');
    expect(row).toBeTruthy();
    fireEvent.click(row!);

    await waitFor(() => expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('Svc'));
    expect(callsOf('archi_module').at(-1)).toEqual({ root: 'C:demoalpha', element: 'Svc' });
    // 画布切到该元素的 1-hop 邻域（hub 图：入边 3 + 出边 2 + 子元素 2）
    expect(renderedNodeIds().sort()).toEqual(
      ['Dst1', 'Dst2', 'Src1', 'Src2', 'Src3', 'Svc', 'Svc.A', 'Svc.B'].sort(),
    );
  });

  it('应在画布节点点击后让文件树反查高亮多 path 元素（链路：canvas → focus → tree）', async () => {
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'archi_graph') return Promise.resolve(hubGraph());
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      if (command === 'archi_file_tree') {
        return Promise.resolve(fileTree({ root: 'Svc.A', leaf: 'Src1' }));
      }
      if (command === 'archi_module') {
        const fqn = typeof params?.element === 'string' ? params.element : '';
        return Promise.resolve(detailOf(fqn));
      }
      return Promise.resolve(null);
    });
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('rf__node-Svc')).toBeTruthy());
    // 文件树在场需先切到「文件」分段（缺省分支是元素树）
    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
    const treeCalls = callsOf('archi_file_tree').length;
    expect(treeCalls).toBeGreaterThan(0);

    fireEvent.click(screen.getByTestId('rf__node-Svc.A'));
    await waitFor(() => expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('Svc.A'));

    // 文件树两行同 owners ⇒ 同时高亮，且反查是纯派生（零重取数）
    const highlighted = screen
      .queryAllByTestId('archi-file-node')
      .filter((row) => row.className.split(/\s+/).includes('bg-muted'));
    expect(highlighted.length).toBeGreaterThanOrEqual(2);
    expect(callsOf('archi_file_tree')).toHaveLength(treeCalls);
  });

  it('应在文件树取数失败时只落左栏行内提示，画布与抽屉照常（两取数面独立失败）', async () => {
    serveGraphAndDrift(healthyGraph(), driftReport());
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'archi_graph') return Promise.resolve(healthyGraph());
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      if (command === 'archi_file_tree') return Promise.reject(new Error('模型结构错（共 1 条）'));
      if (command === 'archi_module') {
        const fqn = typeof params?.element === 'string' ? params.element : '';
        return Promise.resolve(detailOf(fqn));
      }
      return Promise.resolve(null);
    });
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    // 文件树是懒取数：切到「文件」才发起（失败面只发生在切过去之后）
    switchTo('文件');

    await waitFor(() => expect(screen.getByTestId('archi-file-tree-error')).toBeTruthy());
    const notice = screen.getByTestId('archi-file-tree-error').textContent ?? '';
    expect(notice).toContain('文件树取数失败');
    expect(notice).toContain('模型结构错');
    expect(notice).toContain('画布照常渲染');
    // 画布 / 抽屉照常
    await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());
    expect(screen.getByTestId('archi-drawer')).toBeTruthy();
    expect(flow().nodes.length).toBeGreaterThan(0);
    // 失败态不渲文件树本体
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();

    // 切回「元素」：视图卸载 / 重挂语义不变，失败只留在文件视图
    switchTo('元素');
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.queryByTestId('archi-file-tree-error')).toBeNull();
  });

  it('应在文件树首轮未回时渲「解析中…」占位（树本体不挂载）', async () => {
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'archi_graph') return Promise.resolve(healthyGraph());
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      if (command === 'archi_file_tree') return new Promise<ArchiFileTree>(() => {});
      if (command === 'archi_module') {
        const fqn = typeof params?.element === 'string' ? params.element : '';
        return Promise.resolve(detailOf(fqn));
      }
      return Promise.resolve(null);
    });
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    switchTo('文件');

    await waitFor(() => expect(screen.getByTestId('archi-file-tree-loading')).toBeTruthy());
    expect(screen.getByTestId('archi-file-tree-loading').textContent).toBe('解析中…');
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();
    // 画布与抽屉照常（图取数已回）
    await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());
    expect(screen.getByTestId('archi-drawer')).toBeTruthy();
  });

  it('应在文件树返回空条目时渲「仓库内无源文件」（与加载态可辨）', async () => {
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'archi_graph') return Promise.resolve(healthyGraph());
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      if (command === 'archi_file_tree') return Promise.resolve({ entries: [] });
      if (command === 'archi_module') {
        const fqn = typeof params?.element === 'string' ? params.element : '';
        return Promise.resolve(detailOf(fqn));
      }
      return Promise.resolve(null);
    });
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    switchTo('文件');

    await waitFor(() => expect(screen.getByTestId('archi-file-tree-empty')).toBeTruthy());
    expect(screen.getByTestId('archi-file-tree-empty').textContent).toBe('仓库内无源文件');
    expect(screen.queryByTestId('archi-file-tree-loading')).toBeNull();
    await waitFor(() => expect(screen.getByTestId('rf__node-App')).toBeTruthy());
  });

  it('应在树失败后换 root 成功时清掉错误行并给出新根的条目', async () => {
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'archi_graph') return Promise.resolve(healthyGraph());
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      if (command === 'archi_file_tree') {
        return params?.root === 'B'
          ? Promise.resolve({
              entries: [fileEntry('crates/core/b', { kind: 'dir', owners: ['App'] })],
            })
          : Promise.reject(new Error('root 无效（C:）'));
      }
      if (command === 'archi_module') {
        const fqn = typeof params?.element === 'string' ? params.element : '';
        return Promise.resolve(detailOf(fqn));
      }
      return Promise.resolve(null);
    });
    const { rerender } = render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree-error')).toBeTruthy());

    rerender(<ArchitectureCanvas root={'B'} mode="full" />);
    // 换 root 触发图重取：正常态重挂 ⇒ 左栏回落缺省元素树（缺省值单点的又一证据）
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.queryByTestId('archi-file-tree-error')).toBeNull();
    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
    expect(screen.queryByTestId('archi-file-tree-error')).toBeNull();
    expect(callsOf('archi_file_tree').at(-1)).toEqual({ root: 'B' });
    expect(
      screen.getAllByTestId('archi-file-node').map((node) => node.getAttribute('data-path')),
    ).toContain('crates/core/b');
  });

  it('应在空模型分支下不挂载左栏（文件树与元素树皆缺席）', async () => {
    serveGraph({ elements: [], relations: [], diagnostics: [] });
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);

    await waitFor(() => expect(screen.getByTestId('archi-canvas-empty')).toBeTruthy());
    // 正锚：早退分支 ⇒ 左栏整块不挂载，故文件树零取数（否定断言的支撑面）
    expect(callsOf('archi_file_tree')).toHaveLength(0);
    expect(screen.queryByTestId('archi-tree-mode')).toBeNull();
    expect(screen.queryByTestId('archi-element-tree')).toBeNull();
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();
  });

  it('应让左栏三视图共用同一可调宽栏位（宽度不随视图变化）', async () => {
    render(<ArchitectureCanvas root={'C:demoalpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    const columnOf = (): HTMLElement => screen.getByTestId('archi-tree-mode').parentElement!;
    const panelOf = (): HTMLElement => columnOf().closest('[data-panel]')!;
    const panel = panelOf();
    expect(panel, '左栏由 ResizablePanel 承载').not.toBeNull();
    expect(panel.closest('[data-group]')).not.toBeNull();

    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
    expect(columnOf(), '文件视图沿用同一栏位').toBe(
      screen.getByTestId('archi-tree-mode').parentElement,
    );
    expect(panelOf(), '面板元素不随视图切换重建').toBe(panel);

    switchTo('元素');
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(panelOf(), '往返切换后仍不重建面板').toBe(panel);
  });
});

// ---------------------------------------------------------------------------
// 本变更：左栏第三视图（contract）+ `apiFindings` 下传 + full 模式权限
// ---------------------------------------------------------------------------

/** `archi_api_check` 的 DTO 夹具（三列缺省空、`enabled` 缺省 true）。 */
function apiCheck(overrides: Record<string, unknown> = {}) {
  return {
    enabled: true,
    meta: { toolVersion: '0.6.40', commonDigest: `sha256:${'a'.repeat(64)}` },
    mismatched: [],
    orphaned: [],
    missing: [],
    ...overrides,
  };
}

/** 在既有装置上补 `archi_api_check` / `archi_api_generate` 分发臂。 */
function serveWithApiCheck(
  graph: ArchiGraph,
  report: ArchiDriftReport,
  check: Record<string, unknown> = apiCheck(),
): void {
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_graph') return Promise.resolve(graph);
    if (command === 'archi_drift') return Promise.resolve(report);
    if (command === 'archi_module') {
      const fqn = typeof params?.element === 'string' ? params.element : '';
      return Promise.resolve(detailOf(fqn));
    }
    if (command === 'archi_file_tree') return Promise.resolve(fileTree());
    if (command === 'archi_set_gate') return Promise.resolve(null);
    if (command === 'archi_api_check') return Promise.resolve(check);
    if (command === 'archi_api_generate') {
      return Promise.resolve({ written: [], unchanged: [] });
    }
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
}

/** 切到「契约」分段（左栏第三视图）。 */
function switchToContract(): void {
  switchTo('契约');
}

/** 报告夹具：含两条 `apiFindings`（面板 / 抽屉共用同一份报告）。 */
function reportWithFindings(): ArchiDriftReport {
  return {
    ...driftReport(),
    apiFindings: [
      {
        kind: 'apiSignatureMismatch',
        category: 'signature',
        element: 'App.Auth',
        target: 'login',
        message: '契约签名与代码签名不一致',
        occurrences: [],
      },
      {
        kind: 'apiMissing',
        category: 'entry',
        element: 'App.Billing',
        target: 'compose',
        message: '实现缺失',
        occurrences: [],
      },
    ],
  };
}

describe('左栏第三视图（契约）', () => {
  it('应在点「契约」后只渲契约面板（两树均不在场）', async () => {
    serveWithApiCheck(healthyGraph(), reportWithFindings());
    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    // 正锚先行：缺省分支是元素树（否定断言不得单独承担缺省判据）
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(screen.getByTestId('archi-tree-mode')).toBeTruthy();

    switchToContract();
    await waitFor(() => expect(screen.getByTestId('archi-contract-panel')).toBeTruthy());
    // 已切到第三视图：正锚 = 契约面板在场 + 文件树零取数，两树均不在场
    expect(callsOf('archi_file_tree')).toHaveLength(0);
    expect(screen.queryByTestId('archi-element-tree')).toBeNull();
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();
    // 三段布局齐备：左栏 + 画布 + 抽屉
    expect(screen.getByTestId('archi-canvas')).toBeTruthy();
    expect(screen.getByTestId('archi-drawer')).toBeTruthy();
  });

  it('应在三视图往返切换时零重取图 / 抽屉 / 漂移三条命令', async () => {
    serveWithApiCheck(healthyGraph(), reportWithFindings());
    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    await waitFor(() => expect(callsOf('archi_graph').length).toBeGreaterThan(0));
    const baseline = {
      graph: callsOf('archi_graph').length,
      module: callsOf('archi_module').length,
      drift: callsOf('archi_drift').length,
    };

    for (let round = 0; round < 2; round += 1) {
      switchTo('元素');
      switchToContract();
      switchTo('文件');
      await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
    }

    expect(callsOf('archi_graph').length).toBe(baseline.graph);
    expect(callsOf('archi_module').length).toBe(baseline.module);
    expect(callsOf('archi_drift').length).toBe(baseline.drift);
    // `archi_api_check` 每次进契约视图各一次（面板挂载即取数），不连带既有命令
    expect(callsOf('archi_api_check').length).toBe(2);
  });

  it('应把 apiFindings 下传到面板两轴与抽屉契约段', async () => {
    serveWithApiCheck(healthyGraph(), reportWithFindings());
    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-tree-mode')).toBeTruthy());
    switchToContract();
    await waitFor(() => expect(screen.getByTestId('archi-contract-panel')).toBeTruthy());

    const findingsAxis = screen.getByTestId('archi-contract-axis-findings');
    await waitFor(() => expect(findingsAxis.textContent).toContain('判据（2）'));
    expect(
      within(findingsAxis)
        .queryAllByTestId('archi-contract-chip')
        .map((chip) => chip.getAttribute('data-category')),
    ).toEqual(['entry', 'signature']);

    // 抽屉契约段按 `element === fqn` 过滤：缺省聚焦 `App`（零判据 ⇒「本元素无漂移」）
    await waitFor(() =>
      expect(screen.getByTestId('archi-drawer').textContent).toContain('本元素无漂移'),
    );

    // 聚焦 `App.Auth` 后：只列本元素那一条判据（邻元素的事实不串场）
    fireEvent.click(
      within(findingsAxis)
        .queryAllByTestId('archi-contract-item')
        .find((item) => item.textContent?.includes('App.Auth'))!,
    );
    await waitFor(() =>
      expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('App.Auth'),
    );
    await waitFor(() =>
      expect(screen.getByTestId('archi-drawer').textContent).toContain('契约签名与代码签名不一致'),
    );
    expect(screen.getByTestId('archi-drawer').textContent).not.toContain('实现缺失');
  });

  it('应在判据条目点击时把聚焦传给抽屉（取数载荷随 fqn）', async () => {
    serveWithApiCheck(healthyGraph(), reportWithFindings());
    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-tree-mode')).toBeTruthy());
    switchToContract();
    const findingsAxis = screen.getByTestId('archi-contract-axis-findings');
    await waitFor(() =>
      expect(within(findingsAxis).queryAllByTestId('archi-contract-item')).toHaveLength(2),
    );

    fireEvent.click(
      within(findingsAxis)
        .queryAllByTestId('archi-contract-item')
        .find((item) => item.textContent?.includes('App.Billing'))!,
    );
    await waitFor(() =>
      expect(screen.getByTestId('archi-drawer-fqn').textContent).toBe('App.Billing'),
    );
    expect(callsOf('archi_module').some((payload) => payload.element === 'App.Billing')).toBe(true);
    // 画布节点集合切到该元素的 1-hop 邻域（入边两条：App.Auth / 子元素 App.Billing.Invoice）
    expect(renderedNodeIds().sort()).toEqual(['App.Auth', 'App.Billing', 'App.Billing.Invoice']);
  });

  it('应在 full 模式下零生成入口 + 一行只读提示', async () => {
    serveWithApiCheck(healthyGraph(), reportWithFindings());
    render(<ArchitectureCanvas root={'/ws'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-tree-mode')).toBeTruthy());
    switchToContract();
    await waitFor(() => expect(screen.getByTestId('archi-contract-panel')).toBeTruthy());
    await waitFor(() => expect(screen.getByTestId('archi-contract-check')).toBeTruthy());

    expect(screen.queryByTestId('archi-contract-generate')).toBeNull();
    expect(screen.getByTestId('archi-contract-readonly')).toBeTruthy();
    expect(callsOf('archi_api_generate')).toHaveLength(0);
  });

  it('应在 incremental 模式下渲染可用的生成入口', async () => {
    serveWithApiCheck(healthyGraph(), reportWithFindings());
    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-tree-mode')).toBeTruthy());
    switchToContract();
    await waitFor(() =>
      expect((screen.getByTestId('archi-contract-generate') as HTMLButtonElement).disabled).toBe(
        false,
      ),
    );
  });

  it('应让契约视图与另两视图共用同一可调宽栏位（宽度不随视图变化）', async () => {
    serveWithApiCheck(healthyGraph(), reportWithFindings());
    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-tree-mode')).toBeTruthy());

    const columnOf = (): HTMLElement => screen.getByTestId('archi-tree-mode').parentElement!;
    const panelOf = (): HTMLElement => columnOf().closest('[data-panel]')!;
    const panel = panelOf();
    expect(panel).not.toBeNull();
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
    expect(panelOf(), '元素视图沿用同一栏位').toBe(panel);
    switchTo('文件');
    await waitFor(() => expect(screen.getByTestId('archi-file-tree')).toBeTruthy());
    expect(panelOf(), '文件视图沿用同一栏位').toBe(panel);
    switchToContract();
    await waitFor(() => expect(screen.getByTestId('archi-contract-panel')).toBeTruthy());
    expect(panelOf(), '契约视图沿用同一栏位').toBe(panel);
  });
});

describe('抽屉契约段链路（drawer → FileViewer → read_file）', () => {
  it('应在抽屉契约段点「查看」时经 FileViewer 触达 read_file（载荷 = root + repo 相对串）', async () => {
    const contractPath = '.dev-team/architecture/api/Core/App.Auth.md';
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_graph') return Promise.resolve(healthyGraph());
      if (command === 'archi_drift') return Promise.resolve(driftReport());
      if (command === 'archi_module') {
        return Promise.resolve({
          ...detailOf('App'),
          api: { status: 'declared', contractPath, schemaPath: null, generatedPath: null },
        });
      }
      if (command === 'archi_file_tree') return Promise.resolve(fileTree());
      if (command === 'archi_api_check') return Promise.resolve(apiCheck());
      if (command === 'read_file') {
        const content = '# 契约\n';
        return Promise.resolve({
          content,
          stat: { size: content.length, modifiedMs: null, isFile: true, isDir: false },
          offset: 0,
          nextOffset: content.length,
          truncated: false,
          encoding: 'utf8',
        });
      }
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });

    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-tree-mode')).toBeTruthy());
    await waitFor(() => expect(screen.queryAllByTestId('archi-contract-path')).toHaveLength(3));

    const rows = screen.queryAllByTestId('archi-contract-path');
    const row = rows[0]!;
    expect(row.getAttribute('title')).toBe(contractPath);
    // 缺席的两槽位（schema / 生成物）行仍在场但「查看」禁用：不吞行、不误开空路径
    for (const nullRow of [rows[1]!, rows[2]!]) {
      expect(nullRow.querySelector('button')!.disabled).toBe(true);
    }
    fireEvent.click(row.querySelector('button')!);
    await waitFor(() => expect(callsOf('read_file')).toHaveLength(1));
    const payload = callsOf('read_file')[0]!;
    expect(payload.path).toBe(`/ws/${contractPath}`);
    expect(payload.offset).toBeNull();
    expect(payload.limit).toBeNull();
    // 链路只经既有读面：不触发生成命令
    expect(callsOf('archi_api_generate')).toHaveLength(0);
  });
});

describe('停摆 / 空模型 / 报告缺席三态的左栏', () => {
  it('应在结构错停摆态与空模型分支挂载零契约视图（零 archi_api_check）', async () => {
    serveWithApiCheck(
      {
        elements: [],
        relations: [],
        diagnostics: [
          {
            kind: 'cycle',
            message: '依赖成环',
            file: '.dev-team/architecture/graph/model/01-core.toml',
            line: 4,
            column: 1,
          },
        ],
      },
      driftReport(),
    );
    const first = render(<ArchitectureCanvas root={'/ws'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-error-panel')).toBeTruthy());
    expect(screen.queryByTestId('archi-contract-panel')).toBeNull();
    expect(screen.queryByTestId('archi-tree-mode')).toBeNull();
    expect(callsOf('archi_api_check')).toHaveLength(0);
    first.unmount();

    serveWithApiCheck({ elements: [], relations: [], diagnostics: [] }, driftReport());
    render(<ArchitectureCanvas root={'/ws'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-canvas-empty')).toBeTruthy());
    expect(screen.queryByTestId('archi-contract-panel')).toBeNull();
    expect(callsOf('archi_api_check')).toHaveLength(0);
  });

  it('应在报告缺席时面板判据区为零且与漂移错误行内提示并存', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'archi_graph') return Promise.resolve(healthyGraph());
      if (command === 'archi_drift') return Promise.reject(new Error('git 不可用'));
      if (command === 'archi_module') return Promise.resolve(detailOf('App.Auth'));
      if (command === 'archi_file_tree') return Promise.resolve(fileTree());
      if (command === 'archi_api_check') return Promise.resolve(apiCheck());
      return Promise.reject(new Error(`未预期命令: ${command}`));
    });
    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-tree-mode')).toBeTruthy());
    switchToContract();
    await waitFor(() => expect(screen.getByTestId('archi-contract-panel')).toBeTruthy());

    const findingsAxis = screen.getByTestId('archi-contract-axis-findings');
    expect(findingsAxis.textContent).toContain('判据（0）');
    expect(within(findingsAxis).queryAllByTestId('archi-contract-chip')).toHaveLength(0);
    expect(screen.getByTestId('archi-drift-error')).toBeTruthy();
  });
});

// ===========================================================================
// 边点击 → 关系态面板（`relation-api-dependency-check` / AC-7 / D14 / D15 / D16 / D17）
//
// 两态同槽互斥：点边 → `RelationInfo`，点节点 → `ModelInfo`。取数纪律是「显式动作
// 开窗」：挂载 / `root` 变化 / 重挂载零 `archi_relation`，点边恰一次，同一元素对
// 重复点开命中 hook 缓存。未声明 `apis` 的边零取数（面板仍开、空态① 渲染）。
// ===========================================================================

/** 可选边的图：聚焦 `App`（缺省顶层），两条出边一有 `apis` 一为空。 */
function selectableGraph(): ArchiGraph {
  return {
    elements: [element('App'), element('App.Auth', { gate: 'strict' }), element('App.Billing')],
    relations: [
      {
        from: 'App',
        to: 'App.Auth',
        description: '核心聚合认证',
        mark: 'none',
        reason: null,
        apis: ['Token', 'Session'],
      },
      {
        from: 'App',
        to: 'App.Billing',
        description: null,
        mark: 'none',
        reason: null,
        apis: [],
      },
    ],
    diagnostics: [],
  };
}

/** 关系详情 fixture（`archi_relation` 的返回体）。 */
function relationDetailOf(from: string, to: string): ArchiRelationDetail {
  return {
    from,
    to,
    description: '核心聚合认证',
    mark: 'none',
    reason: null,
    modelFile: '.dev-team/architecture/graph/model/06-frontend.toml',
    modelLine: 188,
    declared: ['Token', 'Session'],
    used: ['Token'],
    usedNotDeclared: [],
    declaredNotUsed: ['Session'],
    occurrences: [
      { file: 'src/views/a.ts', line: 4, specifier: '../../../types', typeOnly: false },
    ],
  };
}

/** 可选边语料的取数装置（`archi_relation` 可指定失败 / 报告可带豁免留痕或整份缺席）。 */
function serveSelectable(
  graph: ArchiGraph,
  options: {
    relationError?: Error;
    suppressed?: ArchiDriftReport['suppressed'];
    driftError?: Error;
  } = {},
): void {
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_graph') return Promise.resolve(graph);
    if (command === 'archi_drift') {
      if (options.driftError !== undefined) return Promise.reject(options.driftError);
      return Promise.resolve(
        driftReport(options.suppressed === undefined ? {} : { suppressed: options.suppressed }),
      );
    }
    if (command === 'archi_module') {
      const element = typeof params?.element === 'string' ? params.element : '';
      return Promise.resolve(detailOf(element));
    }
    if (command === 'archi_file_tree') return Promise.resolve(fileTree());
    if (command === 'archi_set_gate') return Promise.resolve(null);
    if (command === 'archi_relation') {
      if (options.relationError !== undefined) return Promise.reject(options.relationError);
      const from = typeof params?.from === 'string' ? params.from : '';
      const to = typeof params?.to === 'string' ? params.to : '';
      return Promise.resolve(relationDetailOf(from, to));
    }
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
}

/** 点击一条已渲染的边（xyflow 的 `rf__edge-<id>` 包裹层绑 `onEdgeClick`）。 */
async function clickEdge(id: string): Promise<void> {
  await measureNodes();
  await waitFor(() => expect(screen.queryByTestId(`rf__edge-${id}`)).not.toBeNull());
  fireEvent.click(screen.getByTestId(`rf__edge-${id}`));
}

/** 合成一次边点击（从观测垫片取 `ReactFlow` 的 `onEdgeClick`）：用于图上不存在的陈旧
 * id 与重复声明的动作面——两者都不保证有唯一 DOM 包裹层。 */
function clickEdgeProp(source: string, target: string): void {
  const props = flow() as unknown as {
    onEdgeClick: (event: unknown, edge: { source: string; target: string }) => void;
  };
  act(() => {
    props.onEdgeClick({}, { source, target });
  });
}

describe('边点击 → 关系态面板（两态同槽互斥）', () => {
  it('应在点击 apis 非空的边时切到关系面板并恰取一次详情', async () => {
    serveSelectable(selectableGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    await clickEdge('App->App.Auth');

    await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());
    expect(screen.queryByTestId('archi-drawer')).toBeNull();
    await waitFor(() => expect(callsOf('archi_relation')).toHaveLength(1));
    expect(callsOf('archi_relation')[0]).toEqual({
      root: 'C:\\demo\\alpha',
      from: 'App',
      to: 'App.Auth',
    });
    await waitFor(() =>
      expect(screen.getByTestId('archi-relation-used').textContent).toBe('Token'),
    );
  });

  it('应在选中边上只加粗既有描边（零新标记词汇）', async () => {
    serveSelectable(selectableGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    const before = flow()
      .edges.filter((edge) => edge.id === 'App->App.Auth')
      .map((edge) => (edge.style as { strokeWidth?: number }).strokeWidth);
    expect(before).toEqual([undefined]);

    await clickEdge('App->App.Auth');
    await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());

    const after = flow().edges.find((edge) => edge.id === 'App->App.Auth');
    expect((after?.style as { strokeWidth?: number } | undefined)?.strokeWidth).toBe(3);
    expect(after?.style).toMatchObject({ stroke: NEUTRAL.stroke });
    // 未选中的边零加粗
    const other = flow().edges.find((edge) => edge.id === 'App->App.Billing');
    expect((other?.style as { strokeWidth?: number } | undefined)?.strokeWidth).toBeUndefined();
  });

  it('应在点击节点时回到元素态并清空选中（零新增 invoke）', async () => {
    serveSelectable(selectableGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    await clickEdge('App->App.Auth');
    await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());
    const relationCalls = callsOf('archi_relation').length;

    await waitFor(() => expect(screen.queryByTestId('rf__node-App.Auth')).not.toBeNull());
    fireEvent.click(screen.getByTestId('rf__node-App.Auth'));

    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());
    expect(screen.queryByTestId('archi-relation-info')).toBeNull();
    expect(callsOf('archi_relation')).toHaveLength(relationCalls);
  });

  it('应在未声明 apis 的边上照常开面板但零取数（空态①）', async () => {
    serveSelectable(selectableGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    await clickEdge('App->App.Billing');

    await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());
    expect(screen.getByTestId('archi-relation-undeclared').textContent).toContain('未声明依赖 API');
    expect(callsOf('archi_relation')).toHaveLength(0);
  });

  it('应在同一元素对重复点开时 invoke 计数恒 1（缓存跨面板两态切换）', async () => {
    serveSelectable(selectableGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    await clickEdge('App->App.Auth');
    await waitFor(() => expect(callsOf('archi_relation')).toHaveLength(1));

    await waitFor(() => expect(screen.queryByTestId('rf__node-App.Auth')).not.toBeNull());
    fireEvent.click(screen.getByTestId('rf__node-App.Auth'));
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    await clickEdge('App->App.Auth');
    await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());
    expect(callsOf('archi_relation')).toHaveLength(1);
  });

  it('应在豁免留痕时把来源与理由一并呈现（空态③）', async () => {
    serveSelectable(selectableGraph(), {
      suppressed: [
        {
          from: 'App',
          to: 'App.Auth',
          via: 'config',
          reason: '迁移期允许',
          file: null,
          line: null,
        },
      ],
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    await clickEdge('App->App.Auth');

    await waitFor(() => expect(screen.getByTestId('archi-relation-exempt')).toBeTruthy());
    const exempt = screen.getByTestId('archi-relation-exempt');
    expect(exempt.textContent).toContain('config');
    expect(exempt.textContent).toContain('迁移期允许');
  });

  it('应在取数失败时只落行内错误与重试（图与元素面板零影响）', async () => {
    serveSelectable(selectableGraph(), { relationError: new Error('取数失败') });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());
    const graphCalls = callsOf('archi_graph').length;
    const driftCalls = callsOf('archi_drift').length;

    await clickEdge('App->App.Auth');

    await waitFor(() => expect(screen.getByTestId('archi-relation-error')).toBeTruthy());
    expect(screen.getByTestId('archi-relation-error').textContent).toContain('取数失败');
    expect(screen.getByTestId('archi-relation-retry')).toBeTruthy();
    expect(callsOf('archi_graph')).toHaveLength(graphCalls);
    expect(callsOf('archi_drift')).toHaveLength(driftCalls);
    // 图照常渲染（节点仍在）
    expect(screen.queryByTestId('rf__node-App')).not.toBeNull();
  });

  it('应在挂载与 root 变化时零 archi_relation（取数只由显式动作发起）', async () => {
    serveSelectable(selectableGraph());
    const { rerender } = render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    rerender(<ArchitectureCanvas root={'C:\\demo\\beta'} mode="full" />);
    await waitFor(() => expect(callsOf('archi_graph')).toHaveLength(2));
    expect(callsOf('archi_relation')).toHaveLength(0);

    await clickEdge('App->App.Auth');
    await waitFor(() => expect(callsOf('archi_relation')).toHaveLength(1));
    expect(callsOf('archi_relation')[0]?.root).toBe('C:\\demo\\beta');
  });

  it('应在结构错停摆时零边点击面与零 archi_relation', async () => {
    serveSelectable({
      elements: [],
      relations: [],
      diagnostics: [
        {
          kind: 'cycle',
          message: '依赖成环：App.A -> App.B -> App.A',
          file: '.dev-team/architecture/graph/model/01-core.toml',
          line: 4,
          column: 1,
        },
      ],
    });
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-error-panel')).toBeTruthy());
    expect(document.querySelectorAll('.react-flow__edge')).toHaveLength(0);
    expect(callsOf('archi_relation')).toHaveLength(0);
  });

  it('应在 suppressed 为空或报告缺席时零豁免来源（点边零抛错）', async () => {
    const variants: {
      tag: string;
      options: { suppressed?: ArchiDriftReport['suppressed']; driftError?: Error };
    }[] = [
      { tag: 'suppressed 空', options: { suppressed: [] } },
      { tag: '报告缺席', options: { driftError: new Error('git 不可用') } },
    ];
    for (const variant of variants) {
      invokeMock.mockReset();
      flowProps.length = 0;
      serveSelectable(selectableGraph(), variant.options);
      const view = render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
      await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

      await clickEdge('App->App.Auth');

      await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());
      expect(screen.queryByTestId('archi-relation-exempt'), variant.tag).toBeNull();
      expect(callsOf('archi_relation'), variant.tag).toHaveLength(1);
      if (variant.options.driftError !== undefined) {
        expect(screen.getByTestId('archi-drift-error'), variant.tag).toBeTruthy();
      }
      view.unmount();
    }
  });

  it('应在点击不在图内的陈旧边 id 时保持元素态并零 archi_relation', async () => {
    serveSelectable(selectableGraph());
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());
    const graphCalls = callsOf('archi_graph').length;

    clickEdgeProp('App.Ghost', 'App.Auth');
    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.getByTestId('archi-drawer')).toBeTruthy();
    expect(screen.queryByTestId('archi-relation-info')).toBeNull();
    expect(callsOf('archi_relation')).toHaveLength(0);
    expect(callsOf('archi_graph')).toHaveLength(graphCalls);
  });

  it('应在模型重复声明同一元素对时按 (from, to) 去重（零双面板）', async () => {
    const graph = selectableGraph();
    graph.relations.push({ ...relation('App', 'App.Auth', '核心聚合认证'), apis: ['Token'] });
    serveSelectable(graph);
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    clickEdgeProp('App', 'App.Auth');

    await waitFor(() => expect(screen.getAllByTestId('archi-relation-info')).toHaveLength(1));
    expect(callsOf('archi_relation')).toHaveLength(1);
    expect(callsOf('archi_relation')[0]).toEqual({
      root: 'C:\\demo\\alpha',
      from: 'App',
      to: 'App.Auth',
    });
  });

  it('应让入边分区只作装饰：边身份与过滤语义不因选中而变', async () => {
    const graph = selectableGraph();
    graph.relations.push({ ...relation('App.Auth', 'App', '反向引用'), apis: ['Token'] });
    serveSelectable(graph);
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());

    await clickEdge('App.Auth->App');

    await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());
    expect(callsOf('archi_relation')).toHaveLength(1);
    expect(callsOf('archi_relation')[0]).toEqual({
      root: 'C:\\demo\\alpha',
      from: 'App.Auth',
      to: 'App',
    });
    expect(screen.getByTestId('archi-canvas-zone')).toBeTruthy();
    expect(
      flow()
        .edges.map((edge) => edge.id)
        .sort(),
    ).toEqual(['App->App.Auth', 'App->App.Billing', 'App.Auth->App']);
    const incoming = flow().edges.find((edge) => edge.id === 'App.Auth->App');
    expect(incoming?.source).toBe('App.Auth');
    expect(incoming?.target).toBe('App');
  });

  it('应在 root 变化时清空选中回到元素态', async () => {
    serveSelectable(selectableGraph());
    const { rerender } = render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());
    await clickEdge('App->App.Auth');
    await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());

    rerender(<ArchitectureCanvas root={'C:\\demo\\beta'} mode="full" />);

    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());
    expect(screen.queryByTestId('archi-relation-info')).toBeNull();
  });

  it('应在重挂载后不复活旧选中', async () => {
    serveSelectable(selectableGraph());
    const first = render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());
    await clickEdge('App->App.Auth');
    await waitFor(() => expect(screen.getByTestId('archi-relation-info')).toBeTruthy());

    first.unmount();
    render(<ArchitectureCanvas root={'C:\\demo\\alpha'} mode="full" />);

    await waitFor(() => expect(screen.getByTestId('archi-drawer')).toBeTruthy());
    expect(screen.queryByTestId('archi-relation-info')).toBeNull();
  });

  it('应在源码内零定时器（点边取数不被自动重取放大）', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/views/architecture/components/architecture-canvas.tsx'),
      'utf8',
    );
    expect(source).not.toContain('setInterval');
    expect(source).not.toContain('setTimeout');
  });
});

// ---------------------------------------------------------------------------
// 缺省视图的时机与分段控件（AC-5 / AC-6）
// ---------------------------------------------------------------------------

describe('左栏分段控件与缺省视图（AC-5 / AC-6）', () => {
  it('应按「元素 / 文件 / 契约」声明分段钮且缺省选中「元素」', async () => {
    render(<ArchitectureCanvas root={'/ws'} mode="full" />);
    await waitFor(() => expect(screen.getByTestId('archi-tree-mode')).toBeTruthy());

    const tabs = within(screen.getByTestId('archi-tree-mode')).getAllByRole('tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['元素', '文件', '契约']);
    // 选中态由 base-ui `TabsTab` 提供（退役口径：自绘 `aria-pressed` / `data-mode`）
    expect(tabs.map((tab) => tab.getAttribute('aria-selected'))).toEqual([
      'true',
      'false',
      'false',
    ]);
  });

  it('应在三视图切换后把选中态移到目标钮（零重取图 / 抽屉 / 漂移）', async () => {
    serveWithApiCheck(healthyGraph(), driftReport());
    render(<ArchitectureCanvas root={'/ws'} mode="incremental" />);
    await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());

    const selected = (): (string | null)[] =>
      within(screen.getByTestId('archi-tree-mode'))
        .getAllByRole('tab')
        .filter((tab) => tab.getAttribute('aria-selected') === 'true')
        .map((tab) => tab.textContent);
    const baseline = {
      graph: callsOf('archi_graph').length,
      module: callsOf('archi_module').length,
      drift: callsOf('archi_drift').length,
    };
    // 缺省恰一枚选中且是「元素」
    expect(selected()).toEqual(['元素']);

    for (const [label, anchor] of [
      ['文件', 'archi-file-tree'],
      ['契约', 'archi-contract-panel'],
      ['元素', 'archi-element-tree'],
    ] as const) {
      switchTo(label);
      await waitFor(() => expect(screen.getByTestId(anchor)).toBeTruthy());
      expect(selected()).toEqual([label]);
    }

    expect(callsOf('archi_graph')).toHaveLength(baseline.graph);
    expect(callsOf('archi_module')).toHaveLength(baseline.module);
    expect(callsOf('archi_drift')).toHaveLength(baseline.drift);
  });

  it('应在两处挂载形态（full / incremental）下都缺省渲元素树且零文件树取数', async () => {
    for (const mode of ['full', 'incremental'] as const) {
      invokeMock.mockReset();
      serveGraphAndDrift(healthyGraph(), driftReport());
      render(<ArchitectureCanvas root={'C:demoalpha'} mode={mode} />);

      await waitFor(() => expect(screen.getByTestId('archi-element-tree')).toBeTruthy());
      expect(callsOf('archi_file_tree')).toHaveLength(0);
      // 漂移载荷逐值透传（两挂载点的模式分流不被缺省视图影响）
      await waitFor(() => expect(callsOf('archi_drift').length).toBeGreaterThan(0));
      expect(callsOf('archi_drift').at(-1)).toEqual({ root: 'C:demoalpha', mode });
      cleanup();
    }
  });
});

// ---------------------------------------------------------------------------
// 源码自证：缺省值与声明序各自单点（AC-8 / D7 / D8）
// ---------------------------------------------------------------------------

describe('源码自证（缺省值单点 / 声明序单点）', () => {
  it('应把缺省视图与分段声明序各自单点持有（architecture-canvas.tsx）', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/views/architecture/components/architecture-canvas.tsx'),
      'utf8',
    );

    // 插桩不敏感形态：Stryker 沙箱把初始化式包成
    // `stryMutAct_*(…) ? "" : (stryCov_*(…), 'element')`，整表达式逐字邻接在沙箱内消失，
    // 故判据只取「声明锚 + 同行字面量」两段（先例 use-config-chat.test.ts:601）。
    // 缺省值单点：全源恰一处 `TreeMode` 初值声明，且该行初值为 'element'
    expect(source.match(/useState<TreeMode>\(/g) ?? []).toHaveLength(1);
    expect(source).toMatch(/useState<TreeMode>\([^\n]*'element'/);
    // 否定臂：'file' / 'contract' 不作初值（缺省不是文件树）
    expect(source).not.toMatch(/useState<TreeMode>\([^\n]*'(file|contract)'/);

    // 声明序单点：三钮显示名首现次序 元素 → 文件 → 契约
    // （锚显示名字面量而非 `mode: '…'` 邻接——字面量不被插桩，邻接会）
    const displayOrder = ["'元素'", "'文件'", "'契约'"].map((label) => source.indexOf(label));
    for (const index of displayOrder) expect(index).toBeGreaterThan(-1);
    expect(displayOrder).toEqual([...displayOrder].sort((left, right) => left - right));
    expect(source).toMatch(/label: [^\n]*'元素'/);
    expect(source).toMatch(/label: [^\n]*'文件'/);
    expect(source).toMatch(/label: [^\n]*'契约'/);

    // 分段钮与面板内容都由同一份 options 派生（JSX 内零第二份顺序）
    expect(source.includes('<TabsTrigger value={option.mode}>')).toBe(true);
    expect(source.includes('<TabsContent value={option.mode}>')).toBe(true);
    expect(source.includes('options.map(')).toBe(true);
    expect(source.includes('data-testid="archi-tree-mode"')).toBe(true);
    expect(source.includes('role="group"')).toBe(true);

    // 旧注释口径零命中（缺省陈述只由 treeMode 初值持有）
    expect(source.includes('缺省文件树')).toBe(false);
    expect(source.includes('左栏缺省为文件树')).toBe(false);
  });

  it('应让 file-tree.tsx 的注释不再自行持有缺省值', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/views/architecture/components/file-tree.tsx'),
      'utf8',
    );

    // 旧注释形态（「缺省」后接反引号包裹的 file）零命中
    expect(source.includes('缺省 `file`')).toBe(false);
    // 仍出现「缺省」的行必须指向单点持有者：不得在本文件重新持有缺省值
    const lines = source.split('\n').filter((line) => line.includes('缺省'));
    expect(lines).toHaveLength(1);
    for (const line of lines) expect(line.includes('architecture-canvas.tsx')).toBe(true);
    // 跨文件反证：本文件零 `TreeMode` 初值声明（缺省值单点只在 architecture-canvas.tsx）
    expect(source).not.toMatch(/useState<TreeMode>\(/);

    // 三值联合齐备（声明锚 + 三取值 indexOf 均在锚后；不锚整句逐字）
    const unionAnchor = source.indexOf('export type TreeMode =');
    expect(unionAnchor).toBeGreaterThan(-1);
    expect(source.match(/export type TreeMode =/g) ?? []).toHaveLength(1);
    for (const literal of ["'file'", "'element'", "'contract'"]) {
      expect(source.indexOf(literal)).toBeGreaterThan(unionAnchor);
    }

    // 导出面与 testid 零变动（组件行为与「缺省视图」解耦）
    expect(source.includes('export function FileTree')).toBe(true);
    expect(source.includes('export interface FileTreeProps')).toBe(true);
    for (const testId of [
      'archi-file-tree',
      'archi-file-node',
      'archi-file-mark',
      'archi-file-tree-error',
      'archi-file-tree-loading',
      'archi-file-tree-empty',
    ]) {
      expect(source.includes(`data-testid="${testId}"`)).toBe(true);
    }

    // 对照臂：旧缺省口径在缺省值持有者内同样零命中
    const canvasSource = readFileSync(
      resolve(process.cwd(), 'src/views/architecture/components/architecture-canvas.tsx'),
      'utf8',
    );
    expect(canvasSource.includes('缺省文件树')).toBe(false);
  });
});
