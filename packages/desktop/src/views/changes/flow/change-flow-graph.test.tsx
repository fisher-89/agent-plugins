import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
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
import type { DrawerSelection, FlowGraph, FlowMaterials } from './types';

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
    expect(onSelect).toHaveBeenCalledWith({ scope: 'node', nodeId: 'eval:proposal:1' });

    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    expect(onSelect).toHaveBeenCalledWith({ scope: 'column', phase: 'proposal' });
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
  });
});
