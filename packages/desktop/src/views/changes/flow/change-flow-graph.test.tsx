import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  ArtifactDescriptor,
  ArtifactEnvelope,
  AttemptRecord,
  ChangeDetail,
  ChangeRunEdgeRecord,
} from '../../../types/dto';
import { mountMaterials } from './attachments';
import { ChangeFlowGraph } from './change-flow-graph';
import { buildFlowGraph } from './graph';
import type { DrawerSelection, FlowGraph, FlowMaterials, RuntimeFlowNode } from './types';

/**
 * requirement 相位序夹具（D12：前端 `PIPELINE_PHASES` 常量退役，列序改由
 * `detail.pipeline` 出线承载，测试各自内联 fixture）：9 列，archive 为第 9 列。
 */
const REQUIREMENT_PHASES = [
  'proposal',
  'dev-design',
  'test-design',
  'implement',
  'test-gen',
  'test-execution',
  'code-review',
  'acceptance',
  'archive',
];

// ---------------------------------------------------------------------------
// ChangeFlowGraph 单测（change-flow-show-step-only，AC-1 / AC-3）：节点模型单
// 分类后图渲染恒走 runStep——`nodeTypes.event` 与 `flow-event-node` 整体退役
//（整树残留检测），节点点击判据收敛为「无信息源（role 与 outputRef 皆 null）
//不上抛」。jsdom 环境垫片（ResizeObserver / offsetWidth / DOMMatrixReadOnly /
//getBBox）为环境缺口而非业务 mock；buildFlowGraph / mountMaterials /
//RunStepNode / PhaseColumnNode 真实组合不 mock。
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
  phase: 'proposal',
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

/** 建档详情底座：10 站（逐站 attempts 由 override 注入）+ 单产物清单。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: null,
    pipeline: REQUIREMENT_PHASES.map((phase) => ({ phase, attempts: [] })),
    activePhase: null,
    runs: [],
    worktree: null,
    artifacts: [DOC_DESCRIPTOR],
    ...overrides,
  };
}

/** 挂载素材（真实 mountMaterials 两参调用——挂载层已去 graph 入参）。 */
function materialsFor(base: ChangeDetail): FlowMaterials {
  return mountMaterials(base, [docEnvelope()]);
}

function renderGraph(
  graph: FlowGraph,
  materials: FlowMaterials,
  onSelect: (selection: DrawerSelection) => void = () => {},
) {
  return render(<ChangeFlowGraph graph={graph} materials={materials} onSelect={onSelect} />);
}

/** 运行步节点 fixture（run-state.ts runStepNodes 同形投影的直构形态）。 */
function runNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return {
    id: 'run:implement:1:executor:0',
    kind: 'runtime',
    phase: 'implement',
    attempt: 1,
    colIndex: REQUIREMENT_PHASES.indexOf('implement'),
    order: 0,
    parentId: 'col:implement',
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    instanceNo: 0,
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    outputRef: null,
    ...overrides,
  };
}

/** 无信息源运行步节点 fixture（role 与 outputRef 皆 null——不可点击判据面；
 * phase / colIndex / parentId 随 phase 自洽派生）。 */
function silentNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  const phase = overrides.phase ?? 'implement';
  return runNode({
    colIndex: REQUIREMENT_PHASES.indexOf(phase),
    parentId: `col:${phase}`,
    phase,
    runStepKind: 'phaseStart',
    group: 'toolStep',
    role: null,
    sessionId: null,
    ...overrides,
  });
}

describe('ChangeFlowGraph：runStep 单分类节点渲染', () => {
  it('含 RuntimeFlowNode 的图 → runStep 组件逐节点可辨、列数与出线序一致 + 节点计数与 graph 一致', async () => {
    const base = detail();
    const graph = buildFlowGraph(base, [
      runNode(),
      runNode({
        id: 'run:implement:1:staticCheck:0',
        runStepKind: 'staticCheck',
        group: 'toolStep',
        role: null,
        sessionId: null,
        status: 'failed',
        detail: '2 处诊断',
      }),
    ]);
    renderGraph(graph, materialsFor(base));

    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    await waitFor(() =>
      expect(screen.getAllByTestId('flow-column')).toHaveLength(REQUIREMENT_PHASES.length),
    );
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    expect(screen.getAllByTestId('run-step-node')).toHaveLength(graph.nodes.length);

    const running = within(screen.getByTestId('rf__node-run:implement:1:executor:0')).getByTestId(
      'run-step-node',
    );
    expect(running.getAttribute('data-run-group')).toBe('workerAgent');
    expect(running.getAttribute('data-run-status')).toBe('running');
    expect(within(running).getByTestId('run-step-kind').textContent).toBe('执行');
    const failed = within(screen.getByTestId('rf__node-run:implement:1:staticCheck:0')).getByTestId(
      'run-step-node',
    );
    expect(failed.getAttribute('data-run-status')).toBe('failed');
    expect(within(failed).getByTestId('run-step-group').textContent).toBe('ToolStep');
    expect(within(failed).getByTestId('run-step-detail').textContent).toBe('2 处诊断');
  });

  it('flow-event-node 整体退役：事件节点挂钩与节点类型在整树零残留，渲染节点集与图节点集逐 id 一致', async () => {
    const base = detail();
    const nodes = [runNode(), silentNode({ id: 'run:implement:1:phaseLog:0' })];
    const graph = buildFlowGraph(base, nodes);
    const { container } = renderGraph(graph, materialsFor(base));
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));

    // 事件节点 testid / 判定徽章零残留（节点模型单分类：整树唯一节点组件为 runStep）
    expect(screen.queryByTestId('flow-node')).toBeNull();
    expect(container.querySelector('[data-testid="flow-node"]')).toBeNull();
    expect(container.querySelector('[data-testid="attempt-verdict"]')).toBeNull();
    // 渲染节点集与图节点集逐 id 一致（列容器 id `col:*` 不计）
    const renderedIds = screen
      .getAllByTestId(/^rf__node-/)
      .map((element) => element.getAttribute('data-testid')?.slice('rf__node-'.length) ?? '')
      .filter((id) => !id.startsWith('col:'))
      .sort();
    expect(renderedIds).toEqual(nodes.map((node) => node.id));
  });

  it('列容器凭声明尺寸未测量即可见：泳道可见性不依赖 ResizeObserver 测量', async () => {
    const base = detail();
    renderGraph(buildFlowGraph(base, [runNode()]), materialsFor(base));

    // 不 flush 测量（未测量窗口内）：运行步节点尚在 hidden（对照证明环境确处未
    // 测量态），列容器凭节点属性面声明的 width/height 立即 visible
    const column = screen.getByTestId('rf__node-col:proposal');
    expect(column.style.visibility).toBe('visible');
    // 列高按列内节点数换算（HEADER 48 + 1 × NODE_H 64 + PAD 36 = 148）
    await measureNodes();
    await waitFor(() =>
      expect(screen.getByTestId('rf__node-col:implement').style.height).toBe('148px'),
    );
  });
});

describe('ChangeFlowGraph：节点点击判据（AC-3）', () => {
  it('点击 role !== null 的 runtime 节点 → onSelect 收到 { scope: node, nodeId }', async () => {
    const onSelect = vi.fn();
    const base = detail();
    renderGraph(buildFlowGraph(base, [runNode()]), materialsFor(base), onSelect);
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(1));

    fireEvent.click(screen.getByTestId('rf__node-run:implement:1:executor:0'));

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith({ scope: 'node', nodeId: 'run:implement:1:executor:0' });
  });

  it('点击 outputRef !== null 的 runtime 节点（static-check / test-execution）→ onSelect 照常上抛', async () => {
    const onSelect = vi.fn();
    const base = detail();
    const node = runNode({
      id: 'run:test-execution:1:testExecution:0',
      phase: 'test-execution',
      colIndex: REQUIREMENT_PHASES.indexOf('test-execution'),
      parentId: 'col:test-execution',
      runStepKind: 'testExecution',
      group: 'toolStep',
      role: null,
      sessionId: null,
      outputRef: 'exec-1727000000000-7',
    });
    renderGraph(buildFlowGraph(base, [node]), materialsFor(base), onSelect);
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(1));

    fireEvent.click(screen.getByTestId('rf__node-run:test-execution:1:testExecution:0'));

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith({
      scope: 'node',
      nodeId: 'run:test-execution:1:testExecution:0',
    });
  });

  it('点击列头 → onSelect 收到 { scope: column, phase }（列头不受节点判据约束）', async () => {
    const onSelect = vi.fn();
    const base = detail();
    renderGraph(buildFlowGraph(base, [runNode()]), materialsFor(base), onSelect);
    await waitFor(() =>
      expect(screen.getAllByTestId('flow-column')).toHaveLength(REQUIREMENT_PHASES.length),
    );

    fireEvent.click(screen.getAllByTestId('flow-column')[REQUIREMENT_PHASES.indexOf('implement')]);

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith({ scope: 'column', phase: 'implement' });
  });

  it('异常：role 与 outputRef 皆 null 的节点（phaseStart / phaseLog / gate 三门 / git 四步 / preflight / seal）→ 点击不触发 onSelect', async () => {
    const onSelect = vi.fn();
    const base = detail();
    const nodes = [
      silentNode({ id: 'run:implement:1:phaseStart:0' }),
      silentNode({ id: 'run:implement:1:phaseLog:0', runStepKind: 'phaseLog' }),
      silentNode({
        id: 'run:implement:1:verdictGate:0',
        runStepKind: 'verdictGate',
        group: 'gate',
      }),
      silentNode({ id: 'run:implement:1:retryGate:0', runStepKind: 'retryGate', group: 'gate' }),
      silentNode({
        id: 'run:implement:1:whitelistGate:0',
        runStepKind: 'whitelistGate',
        group: 'gate',
      }),
      silentNode({
        id: 'run:archive:1:commit:0',
        phase: 'archive',
        runStepKind: 'commit',
        group: 'git',
      }),
      silentNode({
        id: 'run:archive:1:rebase:0',
        phase: 'archive',
        runStepKind: 'rebase',
        group: 'git',
      }),
      silentNode({
        id: 'run:archive:1:merge:0',
        phase: 'archive',
        runStepKind: 'merge',
        group: 'git',
      }),
      silentNode({
        id: 'run:archive:1:finalize:0',
        phase: 'archive',
        runStepKind: 'finalize',
        group: 'git',
      }),
      silentNode({ id: 'run:archive:1:preflight:0', phase: 'archive', runStepKind: 'preflight' }),
      silentNode({ id: 'run:archive:1:seal:0', phase: 'archive', runStepKind: 'seal' }),
    ];
    const graph = buildFlowGraph(base, nodes);
    renderGraph(graph, materialsFor(base), onSelect);
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(11));

    // 每个无信息源节点逐一点击：onSelect 恒零调用（不增长即不上抛）
    for (const node of graph.nodes) {
      fireEvent.click(screen.getByTestId(`rf__node-${node.id}`));
      expect(onSelect).not.toHaveBeenCalled();
    }
  });
});

describe('ChangeFlowGraph：边渲染与重渲染', () => {
  it('label 为 null 的常规边与带 label 的回跳边混存 → 均正常渲染不抛错', async () => {
    const base = detail({
      pipeline: REQUIREMENT_PHASES.map((phase) =>
        phase === 'dev-design'
          ? {
              phase,
              attempts: [
                attempt({
                  startAt: '2026-09-03T00:00:00Z',
                  backtrackTo: 'proposal',
                  backtrackReason: '设计未对齐提案',
                }),
              ],
            }
          : { phase, attempts: [] },
      ),
    });
    const nodes: RuntimeFlowNode[] = [
      runNode({
        id: 'run:dev-design:1:executor:0',
        phase: 'dev-design',
        colIndex: REQUIREMENT_PHASES.indexOf('dev-design'),
        parentId: 'col:dev-design',
        status: 'passed',
      }),
      runNode({
        id: 'run:dev-design:1:verdictGate:0',
        phase: 'dev-design',
        colIndex: REQUIREMENT_PHASES.indexOf('dev-design'),
        parentId: 'col:dev-design',
        runStepKind: 'verdictGate',
        group: 'gate',
        role: null,
        sessionId: null,
      }),
    ];
    const edges: ChangeRunEdgeRecord[] = [
      {
        fromNodeKey: 'dev-design:1:executor:0',
        toNodeKey: 'dev-design:1:verdict_gate:0',
        edgeKind: 'forward',
      },
      {
        fromNodeKey: 'dev-design:1:executor:0',
        toNodeKey: 'dev-design:1:verdict_gate:0',
        edgeKind: 'backtrack',
      },
    ];
    const graph = buildFlowGraph(base, nodes, edges);
    expect(graph.edges.map((edge) => edge.kind)).toEqual(['forward', 'backtrack']);
    renderGraph(graph, materialsFor(base));

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
    expect(warnPaths).toHaveLength(1);
    expect(warnPaths[0]?.getAttribute('style')).toContain('6 4');
  });

  it('边界：空 graph（columns / nodes / edges 为空）→ 渲染空画布不抛错', () => {
    const materials: FlowMaterials = { columnDocs: {} };
    expect(() => renderGraph({ columns: [], nodes: [], edges: [] }, materials)).not.toThrow();
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    expect(screen.queryByTestId('flow-column')).toBeNull();
    expect(screen.queryByTestId('run-step-node')).toBeNull();
  });

  it('边界：graph 变化重渲染 → 新节点集合渲染、旧节点零残留', async () => {
    const before = detail();
    const { rerender } = renderGraph(buildFlowGraph(before, [runNode()]), materialsFor(before));
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(1));

    const after = detail();
    const secondGraph = buildFlowGraph(after, [
      runNode({
        id: 'run:test-design:1:evaluator:0',
        phase: 'test-design',
        colIndex: REQUIREMENT_PHASES.indexOf('test-design'),
        parentId: 'col:test-design',
        runStepKind: 'evaluator',
        role: 'evaluator',
      }),
      runNode({
        id: 'run:test-design:1:decision:0',
        phase: 'test-design',
        colIndex: REQUIREMENT_PHASES.indexOf('test-design'),
        parentId: 'col:test-design',
        runStepKind: 'decision',
        role: 'decision',
      }),
    ]);
    rerender(
      <ChangeFlowGraph graph={secondGraph} materials={materialsFor(after)} onSelect={() => {}} />,
    );

    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    expect(screen.getByTestId('rf__node-run:test-design:1:evaluator:0') !== null).toBe(true);
    expect(screen.queryByTestId('rf__node-run:implement:1:executor:0')).toBeNull();
    expect(screen.getAllByTestId('flow-column')).toHaveLength(REQUIREMENT_PHASES.length);
  });
});
