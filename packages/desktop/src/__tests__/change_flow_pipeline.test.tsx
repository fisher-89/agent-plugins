// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArtifactEnvelope, AttemptRecord, ChangeDetail } from '../types/dto';
import { ChangeDetailView } from '../views/changes/ChangeDetailView';
import { mountMaterials } from '../views/changes/flow/attachments';
import { ChangeFlowGraph } from '../views/changes/flow/ChangeFlowGraph';
import { DetailDrawer } from '../views/changes/flow/DetailDrawer';
import { buildFlowGraph } from '../views/changes/flow/graph';
import type { DrawerSelection } from '../views/changes/flow/types';
import type { ChangeDetailState } from '../views/changes/hooks/useChangeDetail';

// ---------------------------------------------------------------------------
// 集成：buildFlowGraph + mountMaterials → ChangeFlowGraph → DetailDrawer 抽屉
// 链路（AC-2 / AC-3 / AC-4 / AC-5）。链路内模块间调用一律真实实现，仅运行环境
// mock（ReactFlow jsdom 挂载依赖 ResizeObserver / SVGGraphicsElement.getBBox，
// 环境垫片而非业务 mock）；页面级用例以 ChangeDetailState fixture 直供。
//
// 三处接缝只有组合才暴露：素材键与节点 id 方案逐字一致、onSelect 载荷与抽屉
// 消费形状对齐、stale / interrupted 节点在真实渲染树中可交互。
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

// jsdom 环境垫片（非业务 mock）：布局引擎缺失下的 xyflow 度量依赖
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

/** 渲染后重放节点度量（须在 act 内触发 store 更新）。 */
async function measureNodes(): Promise<void> {
  await act(async () => {
    ResizeObserverStub.flush();
    await Promise.resolve();
  });
}

// ---------------------------------------------------------------------------
// v2-b 形态 fixture：eval 多 attempt + backtrack + stale + interrupted + active，
// 文档信封 + eval-checklist 信封，phase-scope 与 workflow-scope file_log 混合
// ---------------------------------------------------------------------------

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

const EMPTY_STATIONS = [
  'implement',
  'test-gen',
  'test-execution',
  'code-review',
  'acceptance',
  'code-analyze',
].map((phase) => ({ phase, attempts: [] as AttemptRecord[] }));

const V2B_DETAIL: ChangeDetail = {
  name: 'add-feature',
  source: 'active',
  inventory: 'v2',
  created: '2026-09-01',
  unparsable: false,
  pipeline: [
    {
      phase: 'proposal',
      attempts: [
        attempt({
          verdict: 'fail',
          report: '提案评估未过',
          startAt: '2026-09-01T10:00:00Z',
          checklist: [{ item: '问题清晰', pass: false, evidence: 'L1-10' }],
        }),
      ],
    },
    {
      phase: 'dev-design',
      attempts: [
        attempt({
          attempt: 1,
          verdict: 'fail',
          report: '设计未过（已废弃）',
          stale: true,
          startAt: '2026-09-02T10:00:00Z',
        }),
        attempt({
          attempt: 2,
          verdict: 'pass',
          report: '设计通过',
          startAt: '2026-09-02T18:00:00Z',
          backtrackTo: 'proposal',
          backtrackReason: '设计未对齐提案',
        }),
      ],
    },
    {
      phase: 'test-design',
      attempts: [attempt({ report: '测试设计通过', startAt: '2026-09-03T10:00:00Z' })],
    },
    ...EMPTY_STATIONS,
  ],
  activePhase: { phase: 'implement', attempt: 1, startAt: '2026-09-04T09:00:00Z' },
  interrupted: [
    {
      phase: 'test-design',
      attempt: 1,
      startAt: '2026-09-03T16:00:00Z',
      endAt: '2026-09-03T17:00:00Z',
    },
    {
      phase: 'code-review',
      attempt: 1,
      startAt: '2026-09-02T12:00:00Z',
      endAt: '2026-09-02T14:00:00Z',
    },
  ],
  fileLog: [
    {
      op: 'write',
      scope: 'proposal',
      attempt: 1,
      path: 'docs/proposal.md',
      at: '2026-09-01T10:05:00Z',
    },
    { op: 'write', scope: 'dev-design', attempt: 2, path: 'src/design.ts', at: null },
    {
      op: 'write',
      scope: 'workflow',
      attempt: null,
      path: 'workflow.json',
      at: '2026-09-01T09:00:00Z',
    },
    { op: 'delete', scope: 'unknown-phase', attempt: null, path: 'temp/x.txt', at: null },
  ],
  artifacts: [
    { kind: 'markdown-doc', source: 'proposal.md', title: '提案' },
    { kind: 'markdown-doc', source: 'design.md', title: '设计' },
    { kind: 'eval-checklist', source: '0', title: '评估清单' },
    { kind: 'tasks-progress', source: 'tasks.md', title: '任务进度' },
  ],
};

/** 信封按 detail.artifacts 顺序 1:1 生成（对齐 useChangeDetail 配对方式）。 */
const V2B_ENVELOPES: ArtifactEnvelope[] = [
  {
    kind: 'markdown-doc',
    version: 1,
    title: '提案',
    payload: { markdown: '# 提案正文' },
    fallbackText: null,
  },
  {
    kind: 'markdown-doc',
    version: 1,
    title: '设计',
    payload: { markdown: '# 设计正文' },
    fallbackText: null,
  },
  {
    kind: 'eval-checklist',
    version: 1,
    title: '评估清单',
    payload: {
      phase: 'proposal',
      attempt: 1,
      verdict: 'fail',
      items: [{ item: '问题清晰', pass: false, evidence: 'L1-10' }],
    },
    fallbackText: null,
  },
  {
    kind: 'tasks-progress',
    version: 1,
    title: '任务进度',
    payload: { total: 4, done: 1, pending: 3 },
    fallbackText: null,
  },
];

/** 归并序（骨干 eval 追加序 + interrupted / active 按 startAt 插入）：
 * P1 → D1 → I(cr) → D2 → TD1 → I(td) → A(impl) */
const MERGED_IDS = [
  'eval:proposal:1',
  'eval:dev-design:1',
  'interrupted:code-review:1',
  'eval:dev-design:2',
  'eval:test-design:1',
  'interrupted:test-design:1',
  'active:implement:1',
];

/** 抽屉链路 harness：与页面组装层同构的 selection 状态 + 真实转换 / 挂载 / 渲染。 */
function FlowHarness({
  detail,
  envelopes,
}: {
  detail: ChangeDetail;
  envelopes: ArtifactEnvelope[];
}) {
  const [selection, setSelection] = useState<DrawerSelection | null>(null);
  const graph = buildFlowGraph(detail);
  const materials = mountMaterials(graph, detail, envelopes);
  return (
    <>
      <ChangeFlowGraph graph={graph} materials={materials} onSelect={setSelection} />
      <DetailDrawer
        selection={selection}
        graph={graph}
        materials={materials}
        hasFileLog={detail.fileLog !== null}
        onClose={() => setSelection(null)}
      />
    </>
  );
}

function detailState(detail: ChangeDetail, envelopes: ArtifactEnvelope[]): ChangeDetailState {
  return {
    detail,
    artifacts: envelopes,
    loading: false,
    error: null,
    refresh: () => {},
  };
}

function openNode(nodeId: string) {
  fireEvent.click(screen.getByTestId(`rf__node-${nodeId}`));
}

/** 列头点击走容器本体 onClick（非 xyflow onNodeClick），须点内层 flow-column。 */
function openColumn(phase: string) {
  const columnWrapper = screen.getByTestId(`rf__node-col:${phase}`);
  fireEvent.click(within(columnWrapper).getByTestId('flow-column'));
}

function closeDrawer() {
  fireEvent.click(screen.getByRole('button', { name: '关闭' }));
  expect(screen.queryByTestId('detail-drawer')).toBeNull();
}

// ---------------------------------------------------------------------------
// 场景：v2 完整链路——素材上墙与抽屉取材
// ---------------------------------------------------------------------------

describe('change_flow_pipeline：v2 完整链路——素材上墙与抽屉取材', () => {
  it('真实链路产出三分类节点与边形态（AC-2 / AC-3），点击 eval 节点抽屉三分节精确取材（AC-4 → AC-5）', async () => {
    render(<FlowHarness detail={V2B_DETAIL} envelopes={V2B_ENVELOPES} />);

    // 三分类节点全部上墙：eval 实心（stale 半透明）、interrupted dashed、active pulse
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(7));
    expect(screen.getAllByTestId('flow-column')).toHaveLength(9);
    // 事件节点渲染顺序 = 时间序归并序（eval 追加序骨干 + interrupted / active 插入）
    const renderedIds = [...document.querySelectorAll('[data-testid^="rf__node-"]')]
      .map((element) => element.getAttribute('data-testid') ?? '')
      .filter((testid) => !testid.startsWith('rf__node-col:'))
      .map((testid) => testid.replace('rf__node-', ''));
    expect(renderedIds).toEqual(MERGED_IDS);
    const staleNode = within(screen.getByTestId('rf__node-eval:dev-design:1')).getByTestId(
      'flow-node',
    );
    expect(staleNode.className).toContain('opacity-50');
    const interruptedNode = within(
      screen.getByTestId('rf__node-interrupted:code-review:1'),
    ).getByTestId('flow-node');
    expect(interruptedNode.className).toContain('border-dashed');
    const activeNode = within(screen.getByTestId('rf__node-active:implement:1')).getByTestId(
      'flow-node',
    );
    expect(activeNode.className).toContain('animate-pulse');
    // 4 个 eval 节点携带 attempt-meta（interrupted / active 无 verdict 徽标）
    expect(screen.getAllByTestId('attempt-meta')).toHaveLength(4);
    expect(within(activeNode).queryByTestId('attempt-verdict')).toBeNull();

    // 点击 proposal 列 eval fail 节点 → 抽屉三分节与该 phase+attempt 素材精确对应
    openNode('eval:proposal:1');
    const drawer = screen.getByTestId('detail-drawer');
    // 本站文档：挂列（不随 attempt 重复），该列仅 proposal.md 一份
    const docsSection = within(drawer).getByTestId('drawer-docs-section');
    const docCards = within(docsSection).getAllByTestId('artifact-card');
    expect(docCards).toHaveLength(1);
    expect(docCards[0].textContent).toContain('提案正文');
    // eval 节：record.report + 挂载的 eval-checklist 信封（经 registry 渲染）
    const evalSection = within(drawer).getByTestId('drawer-eval-section');
    expect(evalSection.textContent).toContain('提案评估未过');
    expect(within(evalSection).getByTestId('checklist').textContent).toContain('问题清晰');
    // 文件表节：该 attempt 的 phase-scope 条目；workflow-scope 条目不入任何抽屉文件表
    const filesSection = within(drawer).getByTestId('drawer-files-section');
    const table = within(filesSection).getByTestId('filelog-table');
    expect(table.textContent).toContain('docs/proposal.md');
    expect(table.textContent).not.toContain('workflow.json');
    expect(table.textContent).not.toContain('temp/x.txt');
  });

  it('点击列头 → 同一抽屉入口呈该站文档节，eval 节与文件表节空态（detail-drawer 恰一实例）', async () => {
    render(<FlowHarness detail={V2B_DETAIL} envelopes={V2B_ENVELOPES} />);
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(7));

    openColumn('dev-design');
    const drawers = screen.getAllByTestId('detail-drawer');
    expect(drawers).toHaveLength(1);
    const drawer = drawers[0];
    // 该列两份文档（design.md + tasks.md 均映射 dev-design）
    expect(
      within(within(drawer).getByTestId('drawer-docs-section')).getAllByTestId('artifact-card'),
    ).toHaveLength(2);
    expect(within(drawer).getByTestId('drawer-eval-section').textContent).toContain(
      '（无评估记录）',
    );
    expect(within(drawer).getByTestId('drawer-files-section').textContent).toContain(
      '（列头未对应单一 attempt，不聚合文件表）',
    );
  });

  it('backtrack 边（虚线 + backtrackReason 标签）与 retry / forward 边在真实渲染树中可查询，方向与列差一致', async () => {
    render(<FlowHarness detail={V2B_DETAIL} envelopes={V2B_ENVELOPES} />);
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(7));
    await measureNodes();

    const graph = buildFlowGraph(V2B_DETAIL);
    const rendered = document.querySelectorAll('.react-flow__edge');
    expect(rendered).toHaveLength(graph.edges.length);
    // 边 data-id 与转换层产出逐一对应（source->target 即方向）
    const renderedIds = [...rendered].map((edge) => edge.getAttribute('data-id'));
    expect(renderedIds).toEqual(graph.edges.map((edge) => edge.id));
    // 列差语义：回跳（Δ<0）恰一条且带 reason 标签；重试（Δ=0）与前进（Δ>0）无标签
    const kinds = new Map(graph.edges.map((edge) => [edge.id, edge.kind]));
    expect(kinds.get('edge:interrupted:code-review:1->eval:dev-design:2')).toBe('backtrack');
    expect(kinds.get('edge:eval:dev-design:1->interrupted:code-review:1')).toBe('forward');
    expect(kinds.get('edge:eval:test-design:1->interrupted:test-design:1')).toBe('retry');
    const labels = [...document.querySelectorAll('.react-flow__edge-text')];
    expect(labels).toHaveLength(1);
    expect(labels[0].textContent).toBe('设计未对齐提案');
    // 回跳边虚线标记落在边 path 内联样式上；常规边无虚线
    const backtrackPath = document.querySelector(
      '[data-id="edge:interrupted:code-review:1->eval:dev-design:2"] path',
    );
    expect((backtrackPath?.style.strokeDasharray ?? '').length).toBeGreaterThan(0);
    const forwardPath = document.querySelector(
      '[data-id="edge:eval:dev-design:1->interrupted:code-review:1"] path',
    );
    expect(forwardPath?.style.strokeDasharray ?? '').toBe('');
  });

  it('stale eval 节点呈淡化标记且仍可点击；interrupted 节点与同号 eval 并存可分别选中', async () => {
    render(<FlowHarness detail={V2B_DETAIL} envelopes={V2B_ENVELOPES} />);
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(7));

    // stale 节点可点击，废弃支线信息不丢
    openNode('eval:dev-design:1');
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'dev-design · attempt 1 · eval',
    );
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('设计未过（已废弃）');
    closeDrawer();

    // 同号（test-design:1）interrupted 与 eval 并存，分别选中呈各自标题与内容
    openNode('interrupted:test-design:1');
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'test-design · attempt 1 · interrupted',
    );
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('（无评估记录）');
    closeDrawer();
    openNode('eval:test-design:1');
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'test-design · attempt 1 · eval',
    );
  });

  it('点击无素材对象（空站列容器、无挂载信封的 eval）→ 抽屉三分节全空态不抛错', async () => {
    render(<FlowHarness detail={V2B_DETAIL} envelopes={V2B_ENVELOPES} />);
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(7));

    openColumn('code-analyze');
    const drawer = screen.getByTestId('detail-drawer');
    // 空站列容器：三分节全空态
    expect(within(drawer).getByTestId('drawer-docs-section').textContent).toContain(
      '（无本站文档）',
    );
    expect(within(drawer).getByTestId('drawer-eval-section').textContent).toContain(
      '（无评估记录）',
    );
    expect(within(drawer).getByTestId('drawer-files-section').textContent).toContain(
      '（列头未对应单一 attempt，不聚合文件表）',
    );
    closeDrawer();

    // test-design eval 无 checklist 信封、无 file_log 条目 → 内联清单空态 + 文件表（空）
    openNode('eval:test-design:1');
    const evalDrawer = screen.getByTestId('detail-drawer');
    expect(within(evalDrawer).getByTestId('drawer-eval-section').textContent).toContain(
      '测试设计通过',
    );
    expect(within(evalDrawer).getByTestId('drawer-eval-section').textContent).toContain(
      '（清单为空）',
    );
    expect(within(evalDrawer).getByTestId('drawer-files-section').textContent).toContain('（空）');
  });
});

// ---------------------------------------------------------------------------
// 场景：workflow 独立面板与图外素材（前置：ChangeDetailView 整页渲染）
// ---------------------------------------------------------------------------

describe('change_flow_pipeline：workflow 独立面板与图外素材', () => {
  it('workflow 面板逐行渲染 outsideFiles 全集（v2 workflow-scope 条目不缺行），phase-scope 条目不入面板', () => {
    const { container } = render(
      <ChangeDetailView state={detailState(V2B_DETAIL, V2B_ENVELOPES)} onBack={() => {}} />,
    );
    const panel = within(container).getByTestId('workflow-panel');
    const table = within(panel).getByTestId('filelog-table');
    const rows = within(table).getAllByRole('row');
    expect(rows).toHaveLength(3); // 表头 + outsideFiles 2 条
    expect(table.textContent).toContain('workflow.json');
    expect(table.textContent).toContain('temp/x.txt');
    // 挂节点的 phase-scope 条目不落面板
    expect(table.textContent).not.toContain('docs/proposal.md');
    expect(table.textContent).not.toContain('src/design.ts');
  });

  it('未命中节点的 phase-scope 条目（attempt null / 未知 phase）落入面板而非图内任何抽屉文件表节', async () => {
    render(<ChangeDetailView state={detailState(V2B_DETAIL, V2B_ENVELOPES)} onBack={() => {}} />);
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(7));

    // 逐一打开挂节点的 eval / active 抽屉：文件表节均不含 workflow-scope 与未命中条目
    for (const nodeId of ['eval:proposal:1', 'eval:dev-design:2', 'active:implement:1']) {
      openNode(nodeId);
      const filesSection = screen.getByTestId('drawer-files-section');
      if (nodeId === 'active:implement:1') {
        expect(filesSection.textContent).toContain('（空）');
      } else {
        expect(filesSection.textContent).not.toContain('workflow.json');
        expect(filesSection.textContent).not.toContain('temp/x.txt');
      }
      closeDrawer();
    }
    // 面板仍完整承载图外条目（「不入图」与「信息不丢」同时成立）
    const panel = screen.getByTestId('workflow-panel');
    expect(within(panel).getByTestId('filelog-table').textContent).toContain('workflow.json');
  });
});
