import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  ArtifactDescriptor,
  ArtifactEnvelope,
  AttemptRecord,
  ChangeDetail,
} from '../../../types/dto';
import { mountMaterials } from './attachments';
import { ChangeFlowGraph } from './change-flow-graph';
import { buildFlowGraph } from './graph';
import type { DrawerSelection, FlowGraph, FlowMaterials, RuntimeFlowNode } from './types';

// ---------------------------------------------------------------------------
// ChangeFlowGraph 单测：ReactFlow 薄层。jsdom 缺口垫片（环境 stub 而非业务
// mock）：ReactFlow 挂载与节点度量依赖 ResizeObserver（design 已备案），垫片
// 捕获回调并在 measureNodes() 中重放，使 handleBounds 度量真实发生、边可渲染。
// 图数据一律来自真实 buildFlowGraph / mountMaterials 产出；react-flow 库自身
// 渲染语义（fitView 数值、subflow 约束）不做逐项断言（AC-8）。
// ---------------------------------------------------------------------------

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

// jsdom 无布局引擎（offsetWidth / offsetHeight 恒 0）：xyflow 度量守卫需要非零
// 尺寸才会落 handleBounds → 垫片固定返回 100（环境垫片而非业务 mock）
const nativeOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth');
const nativeOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
    configurable: true,
    get: () => 100,
  });
  Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
    configurable: true,
    get: () => 100,
  });
  // xyflow 节点度量读取 viewport transform 的缩放系数（仅用 m22），jsdom 的
  // DOMMatrixReadOnly 不可构造 → 恒等矩阵垫片（环境垫片而非业务 mock）
  vi.stubGlobal(
    'DOMMatrixReadOnly',
    class {
      m22 = 1;
    },
  );
  // xyflow 边标签度量依赖 SVGGraphicsElement.getBBox，jsdom 未实现 → 零包围盒垫片
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

/** 渲染后重放节点度量（须在 act 内触发 store 更新）。 */
async function measureNodes(): Promise<void> {
  await act(async () => {
    ResizeObserverStub.flush();
    await Promise.resolve();
  });
}

function attempt(overrides: Partial<AttemptRecord> = {}): AttemptRecord {
  return {
    attempt: 1,
    verdict: 'pass',
    report: '评估记录',
    checklist: [],
    skipped: false,
    stale: false,
    startAt: null,
    timestamp: null,
    backtrackTo: null,
    backtrackReason: null,
    executorSessionId: null,
    evaluatorSessionId: null,
    decisionSessionId: null,
    ...overrides,
  };
}

const DOC_DESCRIPTOR: ArtifactDescriptor = {
  kind: 'markdown-doc',
  source: 'proposal.md',
  title: '提案',
};

function docEnvelope(): ArtifactEnvelope {
  return {
    kind: 'markdown-doc',
    version: 1,
    title: '提案',
    payload: { markdown: '# 提案' },
    fallbackText: null,
  };
}

/** 空站底座：按 attemptsByPhase 注入各站事件。 */
function detail(
  attemptsByPhase: Record<string, AttemptRecord[]> = {},
  overrides: Partial<ChangeDetail> = {},
): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: null,
    unparsable: false,
    pipeline: [
      'proposal',
      'dev-design',
      'test-design',
      'implement',
      'test-gen',
      'test-execution',
      'code-review',
      'acceptance',
      'code-analyze',
    ].map((phase) => ({ phase, attempts: attemptsByPhase[phase] ?? [] })),
    activePhase: null,
    interrupted: [],
    fileLog: [],
    artifacts: [DOC_DESCRIPTOR],
    ...overrides,
  };
}

function materialsFor(graph: FlowGraph, base: ChangeDetail): FlowMaterials {
  return mountMaterials(graph, base, [docEnvelope()]);
}

function renderGraph(
  graph: FlowGraph,
  materials: FlowMaterials,
  onSelect: (selection: DrawerSelection) => void = () => {},
) {
  return render(<ChangeFlowGraph graph={graph} materials={materials} onSelect={onSelect} />);
}

describe('ChangeFlowGraph：ReactFlow 薄层挂载与交互上抛', () => {
  it('输入真实 buildFlowGraph 产出 → 列容器与事件节点渲染计数与 graph 一致', async () => {
    const base = detail(
      { proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] },
      { activePhase: { phase: 'dev-design', attempt: 1, startAt: '2026-09-02T00:00:00Z' } },
    );
    const graph = buildFlowGraph(base);
    renderGraph(graph, materialsFor(graph, base));

    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    expect(screen.getAllByTestId('flow-node')).toHaveLength(graph.nodes.length);
    expect(graph.nodes).toHaveLength(2);
  });

  it('点击事件节点 → onSelect 收到 { scope: node, nodeId }；点击列头 → { scope: column, phase }', async () => {
    const onSelect = vi.fn();
    const base = detail({ proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] });
    const graph = buildFlowGraph(base);
    renderGraph(graph, materialsFor(graph, base), onSelect);
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(1));

    fireEvent.click(screen.getByTestId('flow-node'));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith({ scope: 'node', nodeId: 'eval:proposal:1' });

    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    // 列头点击恰一次上抛（onNodeClick 不越权重复触发列头选择）
    expect(onSelect).toHaveBeenCalledTimes(2);
    expect(onSelect).toHaveBeenLastCalledWith({ scope: 'column', phase: 'proposal' });
  });

  it('空 graph（columns / nodes 为空数组）→ 渲染空画布不抛错', () => {
    const materials: FlowMaterials = {
      columnDocs: {},
      nodeChecklists: {},
      nodeFiles: {},
      outsideFiles: [],
    };
    expect(() => renderGraph({ columns: [], nodes: [], edges: [] }, materials)).not.toThrow();
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    expect(screen.queryByTestId('flow-column')).toBeNull();
    expect(screen.queryByTestId('flow-node')).toBeNull();
  });

  it('布局常量经列尺寸 / 节点宽度机械换算落到 DOM：列宽高、列绝对坐标、节点宽逐项可核', async () => {
    // proposal 列 2 枚 eval + implement 列 1 枚 active：列高按列内节点数非对称换算
    const base = detail(
      {
        proposal: [
          attempt({ startAt: '2026-09-01T00:00:00Z' }),
          attempt({ startAt: '2026-09-01T12:00:00Z' }),
        ],
      },
      { activePhase: { phase: 'implement', attempt: 1, startAt: '2026-09-02T00:00:00Z' } },
    );
    const graph = buildFlowGraph(base);
    renderGraph(graph, materialsFor(graph, base));
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(3));
    await measureNodes();

    // 列容器：宽 COL_W=260；高 = HEADER 48 + 列内节点 2 × ROW_H 128 + PAD_Y 16 = 320
    const proposalColumn = screen.getByTestId('rf__node-col:proposal');
    expect(proposalColumn.style.width).toBe('260px');
    expect(proposalColumn.style.height).toBe('320px');
    // 单节点列高 = 48 + 1 × 128 + 16 = 192（列高随列内事件数变化，非全局常量）
    expect(screen.getByTestId('rf__node-col:implement').style.height).toBe('192px');
    // 列绝对坐标 x = colIndex × (COL_W + GAP)：implement colIndex 3 → 840（proposal colIndex 0 恒 0 不设断言）
    expect(screen.getByTestId('rf__node-col:implement').style.transform).toContain('840px');
    // 事件节点宽 = COL_W − PAD_X×2 = 224
    expect(screen.getByTestId('rf__node-eval:proposal:1').style.width).toBe('224px');
  });

  it('graph props 引用变化（显式 refresh 后新 graph）→ 重渲染为新节点集合，无残留旧节点', async () => {
    const before = detail({ proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] });
    const firstGraph = buildFlowGraph(before);
    const { rerender } = renderGraph(firstGraph, materialsFor(firstGraph, before));
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(1));

    // refresh 形态：proposal 站 eval 消失，test-design 站新增 eval、implement 站出现 active
    const after = detail(
      { 'test-design': [attempt({ startAt: '2026-09-03T00:00:00Z' })] },
      { activePhase: { phase: 'implement', attempt: 1, startAt: '2026-09-04T00:00:00Z' } },
    );
    const secondGraph = buildFlowGraph(after);
    rerender(
      <ChangeFlowGraph
        graph={secondGraph}
        materials={materialsFor(secondGraph, after)}
        onSelect={() => {}}
      />,
    );

    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(2));
    expect(screen.getByTestId('rf__node-eval:test-design:1') !== null).toBe(true);
    expect(screen.getByTestId('rf__node-active:implement:1') !== null).toBe(true);
    expect(screen.queryByTestId('rf__node-eval:proposal:1')).toBeNull();
    expect(screen.getAllByTestId('flow-column')).toHaveLength(9);
  });

  it('label 为 null 的常规边与带 label 的回跳边混存 → 均正常渲染不抛错', async () => {
    // 中断站列（code-review）startAt 插入 dev-design eval 之前 → 归并后成回跳边
    const base = detail(
      {
        proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })],
        'dev-design': [
          attempt({
            startAt: '2026-09-03T00:00:00Z',
            backtrackTo: 'proposal',
            backtrackReason: '设计未对齐提案',
          }),
        ],
      },
      {
        interrupted: [
          { phase: 'code-review', attempt: 1, startAt: '2026-09-02T00:00:00Z', endAt: null },
        ],
      },
    );
    const graph = buildFlowGraph(base);
    expect(graph.edges.map((edge) => edge.kind)).toEqual(['forward', 'backtrack']);
    renderGraph(graph, materialsFor(graph, base));

    // 重放节点度量后，handleBounds 就位、边随真实渲染树输出
    await measureNodes();
    await waitFor(() =>
      expect(document.querySelectorAll('.react-flow__edge')).toHaveLength(graph.edges.length),
    );
    const labels = [...document.querySelectorAll('.react-flow__edge-text')];
    expect(labels).toHaveLength(1);
    expect(labels[0].textContent).toBe('设计未对齐提案');
    // 边样式按 kind 分流：回跳边 warn 虚线、前进边 border 实线
    const paths = [...document.querySelectorAll<SVGPathElement>('.react-flow__edge-path')];
    expect(paths).toHaveLength(2);
    const warnPaths = paths.filter((path) =>
      (path.getAttribute('style') ?? '').includes('var(--warn)'),
    );
    const borderPaths = paths.filter((path) =>
      (path.getAttribute('style') ?? '').includes('var(--border)'),
    );
    expect(warnPaths).toHaveLength(1);
    expect(borderPaths).toHaveLength(1);
    expect(warnPaths[0]?.getAttribute('style')).toContain('6 4');
    expect(borderPaths[0]?.getAttribute('style')).not.toContain('6 4');
    // 边标签着色随 warn 词汇（文本与底色两层）
    expect(labels[0]?.getAttribute('style') ?? '').toContain('var(--warn)');
    const labelBg = document.querySelector<SVGRectElement>('.react-flow__edge-textbg');
    expect(labelBg?.getAttribute('style') ?? '').toContain('var(--warn-bg)');
  });

  it('graph.edges 变化的重渲染 → 边集随新图重建（边 memo 依赖不悬空）', async () => {
    const withBacktrack = detail(
      {
        proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })],
        'dev-design': [
          attempt({
            startAt: '2026-09-03T00:00:00Z',
            backtrackTo: 'proposal',
            backtrackReason: '设计未对齐提案',
          }),
        ],
      },
      {
        interrupted: [
          { phase: 'code-review', attempt: 1, startAt: '2026-09-02T00:00:00Z', endAt: null },
        ],
      },
    );
    const firstGraph = buildFlowGraph(withBacktrack);
    const { rerender } = renderGraph(firstGraph, materialsFor(firstGraph, withBacktrack));
    await measureNodes();
    await waitFor(() =>
      expect(document.querySelectorAll('.react-flow__edge')).toHaveLength(firstGraph.edges.length),
    );
    expect(firstGraph.edges).toHaveLength(2);

    // 新图无边集 → DOM 边随新图归零（陈旧 memo 不残留旧边）
    const plain = detail({ proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] });
    const secondGraph = buildFlowGraph(plain);
    expect(secondGraph.edges).toHaveLength(0);
    rerender(
      <ChangeFlowGraph
        graph={secondGraph}
        materials={materialsFor(secondGraph, plain)}
        onSelect={() => {}}
      />,
    );
    await waitFor(() => expect(document.querySelectorAll('.react-flow__edge')).toHaveLength(0));
  });
});

// ---------------------------------------------------------------------------
// runStep 运行步节点上图（desktop-change-flow 增量）：nodeTypes 增 runStep
// 注册为加法——含 RuntimeFlowNode 的图渲染 runStep 组件可辨不炸；既有事件 /
// 列节点渲染与交互上抛零回归。图数据来自真实 buildFlowGraph(detail, runNodes)。
// ---------------------------------------------------------------------------

/** 运行步 overlay 节点 fixture（run-state.ts runStepNodes 同形投影的直构形态）。 */
function runNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return {
    id: 'run:implement:1:executor',
    kind: 'runtime',
    phase: 'implement',
    attempt: 1,
    colIndex: 3,
    order: 0,
    parentId: 'col:implement',
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    ...overrides,
  };
}

describe('ChangeFlowGraph：runStep 运行步节点上图（overlay 加法回归）', () => {
  it('含 RuntimeFlowNode 的图 → nodeTypes 解析渲染 runStep 组件：data-testid 可辨、徽章 / 状态 / detail 可读、不炸', async () => {
    const base = detail({ proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] });
    const graph = buildFlowGraph(base, [
      runNode(),
      runNode({
        id: 'run:implement:1:staticCheck',
        runStepKind: 'staticCheck',
        group: 'toolStep',
        role: null,
        sessionId: null,
        status: 'failed',
        detail: '2 处诊断',
      }),
    ]);
    renderGraph(graph, materialsFor(graph, base));

    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    const running = within(screen.getByTestId('rf__node-run:implement:1:executor')).getByTestId(
      'run-step-node',
    );
    expect(running.getAttribute('data-run-group')).toBe('workerAgent');
    expect(running.getAttribute('data-run-status')).toBe('running');
    expect(within(running).getByTestId('run-step-group').textContent).toBe('WorkerAgent');
    expect(within(running).getByTestId('run-step-kind').textContent).toBe('执行');
    const failed = within(screen.getByTestId('rf__node-run:implement:1:staticCheck')).getByTestId(
      'run-step-node',
    );
    expect(failed.getAttribute('data-run-status')).toBe('failed');
    expect(within(failed).getByTestId('run-step-group').textContent).toBe('ToolStep');
    expect(within(failed).getByTestId('run-step-detail').textContent).toBe('2 处诊断');
  });

  it('既有事件 / 列节点渲染与交互上抛零回归；点击 runStep 节点同样上抛 { scope: node, nodeId }', async () => {
    const onSelect = vi.fn();
    const base = detail({ proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] });
    const graph = buildFlowGraph(base, [
      runNode({
        id: 'run:dev-design:1:evaluator',
        phase: 'dev-design',
        runStepKind: 'evaluator',
        status: 'passed',
        sessionId: 'ses-eval-1',
      }),
    ]);
    renderGraph(graph, materialsFor(graph, base), onSelect);

    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(1));
    expect(screen.getAllByTestId('flow-column')).toHaveLength(9);

    // 既有交互上抛：事件节点照旧
    fireEvent.click(screen.getByTestId('flow-node'));
    expect(onSelect).toHaveBeenCalledWith({ scope: 'node', nodeId: 'eval:proposal:1' });
    // 加法分支：运行步节点共用同一抽屉入口
    fireEvent.click(screen.getByTestId('rf__node-run:dev-design:1:evaluator'));
    expect(onSelect).toHaveBeenCalledWith({ scope: 'node', nodeId: 'run:dev-design:1:evaluator' });
  });
});

// ---------------------------------------------------------------------------
// 「查看会话」入口注入（desktop-change-session-visibility，AC-8）：toChartNodes
// 仅对 kind==='runtime' && group==='workerAgent' 节点注入 data.onOpenSession
//（上抛与节点点击同一 DrawerSelection——打开同一抽屉转录联动，无独立会话
// route）；ToolStep / Gate 节点 data 无 onOpenSession 键不渲染按钮。
// ---------------------------------------------------------------------------

describe('ChangeFlowGraph：查看会话入口注入（AC-8）', () => {
  it('workerAgent 运行节点渲染 view-session 按钮；点击 → onSelect 与节点点击同参（打开同一抽屉）', async () => {
    const onSelect = vi.fn();
    const base = detail({ proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] });
    const graph = buildFlowGraph(base, [runNode()]);
    renderGraph(graph, materialsFor(graph, base), onSelect);

    const running = await waitFor(() =>
      within(screen.getByTestId('rf__node-run:implement:1:executor')).getByTestId('view-session'),
    );
    expect(running.textContent).toBe('查看会话');

    fireEvent.click(running);
    // 与节点点击同一 DrawerSelection（同一 onSelect 载荷）
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith({ scope: 'node', nodeId: 'run:implement:1:executor' });
  });

  it('toolStep / gate 节点 data 无 onOpenSession 键：不渲染 view-session 按钮', async () => {
    const base = detail({ proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] });
    const graph = buildFlowGraph(base, [
      runNode({
        id: 'run:implement:1:staticCheck',
        runStepKind: 'staticCheck',
        group: 'toolStep',
        role: null,
        sessionId: null,
        status: 'failed',
      }),
      runNode({
        id: 'run:implement:1:verdictGate',
        runStepKind: 'verdictGate',
        group: 'gate',
        role: null,
        sessionId: null,
        status: 'passed',
      }),
    ]);
    renderGraph(graph, materialsFor(graph, base));

    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    expect(screen.queryByTestId('view-session')).toBeNull();
  });

  it('注入后图结构逐项不变：节点集 / 边集与同 overlay 重建图一致（图结构零触达）', async () => {
    const base = detail({ proposal: [attempt({ startAt: '2026-09-01T00:00:00Z' })] });
    const graph = buildFlowGraph(base, [runNode()]);
    renderGraph(graph, materialsFor(graph, base));

    // 注入不增删节点：列 9 + 事件节点 1 + 运行步节点 1
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(1));
    expect(screen.getAllByTestId('run-step-node')).toHaveLength(1);
    // 边集零触达：与同 overlay 输入的重建图逐 id 一致（onOpenSession 注入不产边）
    const rebuilt = buildFlowGraph(base, [runNode()]);
    expect(graph.edges.map((edge) => edge.id)).toEqual(rebuilt.edges.map((edge) => edge.id));
  });
});
