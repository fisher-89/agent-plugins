import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation, useNavigate } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  ActiveRunView,
  AgentEvent,
  ArtifactDescriptor,
  ArtifactEnvelope,
  AttemptRecord,
  ChangeDetail,
  ChangeRunEntry,
  ChangeStepState,
  SessionSummary,
} from '../../types/dto';
import { ChangeDetailView } from './change-detail-view';

// ---------------------------------------------------------------------------
// ChangeDetailView 单测 mock
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((message: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
    /** 测试直投：模拟后端经 Channel 流出的 RunUpdate 信封。 */
    push(message: unknown): void {
      this.onmessage?.(message);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// 详情取数 fixture（useChangeDetail 的进程边界应答面）：get_change_detail 与
// read_artifact 按命令名 + 参数分发；挂起门闩驱动 loading / 刷新竞态形态。
// ---------------------------------------------------------------------------

let detailResult: ChangeDetail | string | null = null;
let activeRunResult: ActiveRunView | null = null;
let artifactResults: Record<string, ArtifactEnvelope | null> = {};
let holdDetail = false;
let releaseDetail: (() => void) | null = null;

interface DetailFixture {
  /** get_change_detail 应答（null = 未知 change；字符串 = reject 文本） */
  detail: ChangeDetail | string | null;
  /** read_artifact 应答表（键 = `<kind>:<source>`；未命中 → null 降级 Fallback 信封） */
  artifacts?: Record<string, ArtifactEnvelope | null>;
  /** 挂起门闩：get_change_detail 不立即落定（loading / 刷新中形态驱动） */
  hold?: boolean;
}

function loadFixture(f: DetailFixture): void {
  detailResult = f.detail;
  artifactResults = f.artifacts ?? {};
  holdDetail = f.hold ?? false;
  releaseDetail = null;
}

/** 统一视图活面装载（activeRun 在场 = 非终态运行期；null = 空闲 / 已收口）。 */
function loadActiveRun(activeRun: ActiveRunView | null): void {
  activeRunResult = activeRun;
}

/** 详情取数命令分发（run describe 复用同一定向，落 run 缺席命令时为空态应答）。 */
function detailIpc(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
  if (command === 'get_change_detail') {
    if (typeof detailResult === 'string') return Promise.reject(detailResult);
    if (holdDetail) {
      return new Promise((resolve, reject) => {
        releaseDetail = () => {
          if (typeof detailResult === 'string') reject(detailResult);
          else if (detailResult === null) resolve(null);
          else resolve({ detail: detailResult, activeRun: activeRunResult });
        };
      });
    }
    // detail=null fixture = 未知 change（命令层 None 整体 null——非「信封在而
    // detail 空」形态）
    if (detailResult === null) return Promise.resolve(null);
    return Promise.resolve({ detail: detailResult, activeRun: activeRunResult });
  }
  if (command === 'read_artifact') {
    const key = `${String(args.kind)}:${String(args.source)}`;
    return Promise.resolve(key in artifactResults ? artifactResults[key] : null);
  }
  // run / 会话命令缺省空态应答（无运行 run / 空会话清单）
  return Promise.resolve(null);
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

  /** 重放全部观测目标，触发 xyflow 的节点度量（measured / handleBounds 更新）。 */
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
  // 流程图区（建档详情）挂载 ReactFlow：jsdom 缺口垫片（环境 stub 而非业务 mock，
  // flush 可重放节点度量——已测量可见性断言需要）
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
  ChannelMock.instances.length = 0;
  invokeMock.mockReset();
  // 活面模块态全局复位（sequence.shuffle 下 run describe 注入的 activeRun
  // 不得泄漏进其他用例——统一查询活面以用例自装载为准）
  loadActiveRun(null);
  loadFixture({ detail: null });
  invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) =>
    detailIpc(command, args),
  );
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

/** 9 站流水线的固定站名（与 Rust PIPELINE_PHASES 一致）。 */
const PIPELINE = [
  'proposal',
  'dev-design',
  'test-design',
  'implement',
  'test-gen',
  'test-execution',
  'code-review',
  'acceptance',
  'code-analyze',
];

/** 以宽松默认值构造单条 AttemptRecord，便于逐字段控制分支形态。 */
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

function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    id: CHANGE_ID,
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: '2026-09-01',
    pipeline: PIPELINE.map((phase) => ({
      phase,
      attempts:
        phase === 'proposal'
          ? [
              attempt({
                verdict: 'fail',
                report: '提案评估未过',
                startAt: '2026-09-01T10:00:00Z',
                checklist: [{ item: '问题清晰', pass: false, evidence: 'L1-10' }],
              }),
            ]
          : phase === 'dev-design'
            ? [
                attempt({
                  verdict: 'pass',
                  report: '设计通过',
                  stale: true,
                  startAt: '2026-09-02T10:00:00Z',
                }),
              ]
            : [],
    })),
    activePhase: null,
    runs: [],
    worktree: null,
    artifacts: [{ kind: 'markdown-doc', source: 'proposal.md', title: '提案' }],
    ...overrides,
  };
}

function envelope(overrides: Partial<ArtifactEnvelope> = {}): ArtifactEnvelope {
  return {
    kind: 'tasks-progress',
    version: 1,
    title: '任务进度',
    payload: { total: 4, done: 4, pending: 0 },
    fallbackText: null,
    ...overrides,
  };
}

/** proposal.md 文档信封（与 detail().artifacts[0] 下标配对）。 */
function proposalDoc(): ArtifactEnvelope {
  return envelope({
    kind: 'markdown-doc',
    title: '提案',
    payload: { markdown: '# 提案正文' },
    fallbackText: '# 提案正文',
  });
}

/** 缺省详情 fixture 的产物信封表（markdown-doc:proposal.md → 提案文档）。 */
const PROPOSAL_ENVELOPES: Record<string, ArtifactEnvelope> = {
  'markdown-doc:proposal.md': proposalDoc(),
};

// ---------------------------------------------------------------------------
// 路由装置（自 change-view.test 迁入的测试形态）：/changes/:id 直挂详情页
// （URL 段 = change id——一切寻址以 id 为准，name 恒展示面）；/changes 以导航
// 桩占位（真实清单页语义归 change-list-view.test），提供落点断言与再入详情的
// 点击入口。
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
const SECOND = 'C:\\demo\\beta';
/** 详情 fixture 的 change id（uuid 形态身份锚；name 独立为展示面 'add-feature'）。 */
const CHANGE_ID = '0199a2f0-2001-7e45-8a9b-000000002001';
/** 再入详情（导航桩落点）的第二 change id。 */
const SECOND_CHANGE_ID = '0199a2f0-2002-7e45-8a9b-000000002002';

/** URL 探针：把 MemoryRouter 当前 pathname 投影到 DOM 供断言。 */
function LocationProbe() {
  const { pathname } = useLocation();
  return <span data-testid="location-probe">{pathname}</span>;
}

/** /changes 路由占位桩：模拟清单页再入详情的导航入口（按 id 导航）。 */
function NavStub() {
  const navigate = useNavigate();
  return (
    <button data-testid="nav-stub" onClick={() => navigate(`/changes/${SECOND_CHANGE_ID}`)}>
      goto-second-change
    </button>
  );
}

function detailTree(root: string) {
  return (
    <>
      <Routes>
        <Route path="/changes" element={<NavStub />} />
        <Route path="/changes/:id" element={<ChangeDetailView root={root} />} />
      </Routes>
      <LocationProbe />
    </>
  );
}

/** 装置：灌入详情 fixture 后渲染详情页（默认落 /changes/<CHANGE_ID>）。 */
function renderDetail(f: DetailFixture, root: string = ROOT, initialEntry?: string) {
  loadFixture(f);
  return render(
    <MemoryRouter initialEntries={[initialEntry ?? `/changes/${CHANGE_ID}`]}>
      {detailTree(root)}
    </MemoryRouter>,
  );
}

/** 已挂载详情页换根重渲染（根切换场景）：Router 实例保持、location 不重置。 */
function rerenderDetail(view: ReturnType<typeof renderDetail>, f: DetailFixture, root: string) {
  loadFixture(f);
  view.rerender(
    <MemoryRouter initialEntries={[`/changes/${CHANGE_ID}`]}>{detailTree(root)}</MemoryRouter>,
  );
}

function probePathname(): string {
  return screen.getByTestId('location-probe').textContent ?? '';
}

/** get_change_detail 调用清单（参数面 = useChangeDetail 的取数观察面）。 */
function detailCalls(): Array<Record<string, unknown>> {
  return invokeMock.mock.calls
    .filter(([name]) => name === 'get_change_detail')
    .map(([, args]) => args as Record<string, unknown>);
}

describe('ChangeDetailView：页面组装（图区 / 产物区 / 抽屉）', () => {
  it('建档详情 → flow-graph 图区、产物区并存，无 workflow 独立面板与文件表', async () => {
    const { container } = renderDetail({
      detail: detail(),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('flow-graph');
    // file_log 独立面板随载体退役：workflow-panel / filelog-table 零残留
    expect(within(container).queryByTestId('workflow-panel')).toBeNull();
    expect(within(container).queryByTestId('filelog-table')).toBeNull();
    // 产物区照常渲染
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('Header 元信息：name / source / created（无代际徽标）；activePhase 运行中 badge（startAt null 不拼时间与占位）', async () => {
    const { container } = renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
    const header = within(container).getByTestId('detail-header');
    expect(within(header).getByText('add-feature') !== null).toBe(true);
    expect(within(header).getByText('进行中') !== null).toBe(true);
    expect(within(header).getByText('2026-09-01') !== null).toBe(true);
    expect(container.textContent).not.toContain('已归档');
    // activePhase 为 null → 无运行中 badge
    expect(screen.queryByText(/运行中/)).toBeNull();

    const archived = renderDetail({
      detail: detail({
        source: 'archive',
        created: null,
        activePhase: { phase: 'implement', attempt: 2, startAt: null },
      }),
    });
    await waitFor(() => expect(archived.container.textContent).toContain('已归档'));
    expect(archived.container.textContent).not.toContain('进行中');
    // created 为 null → 不渲染空占位节点：detail-header 域内全部文本节点非空
    const archivedHeader = within(archived.container).getByTestId('detail-header');
    for (const leaf of within(archivedHeader).getAllByText(/\S/)) {
      expect((leaf.textContent ?? '').trim().length).toBeGreaterThan(0);
    }
    expect(archivedHeader.textContent).not.toContain('null');
    // activePhase startAt null → badge 不拼接时间与「 · —」占位
    expect(within(archivedHeader).getByText('运行中 · implement · attempt 2') !== null).toBe(true);
    expect(archivedHeader.textContent).not.toContain(' · —');
  });

  it('worktree 信息行：detail.worktree 非 null → 头部呈现路径（break-all 类锚）；legacy null 不渲染（零占位）', async () => {
    const worktreePath = 'C:home.dev-teamworktrees\repo-ab12add-feature';
    renderDetail({
      detail: detail({ worktree: worktreePath }),
    });
    await screen.findByTestId('flow-graph');

    const row = screen.getByTestId('detail-worktree');
    expect(row.textContent).toContain(worktreePath);
    expect(row.textContent).toContain('worktree');
    expect(row.className).toContain('break-all');

    // legacy 半边（负断言）：worktree null → 信息行不出现（零占位）
    const legacy = renderDetail({ detail: detail({ worktree: null }) });
    await screen.findByTestId('flow-graph');
    expect(legacy.container.querySelector('[data-testid="detail-worktree"]')).toBeNull();
  });

  it('列头 / 节点点击 → detail-drawer 挂载（selection 状态在本组件）；关闭后卸载', async () => {
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await waitFor(() => expect(screen.getAllByTestId('flow-node')).toHaveLength(2));

    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    expect(screen.queryByTestId('detail-drawer')).toBeNull();

    fireEvent.click(screen.getByTestId('rf__node-eval:proposal:1'));
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    // 抽屉 eval 节为所选 attempt 的 report（页面组装层 selection 上抛链路可用）
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('提案评估未过');
  });

  it('flow-empty 退役：任意 detail 形态（含 status 缺席 + pipeline 空）恒走图区，flow-empty 与「文档形态」文案零出现', async () => {
    // 退役负断言（AC-4 / D10）：历史上 status 缺席走文档形态空图占位，
    // 现详情可达者恒为建档 change——图区常驻，占位分支与文案整体退役
    const retiredCopy = '文档形态：未建档，仅产物清单';
    const doc = renderDetail({ detail: detail({ status: null, pipeline: [] }) });
    await screen.findByTestId('flow-graph');
    expect(within(doc.container).queryByTestId('flow-empty')).toBeNull();
    expect(doc.container.textContent).not.toContain(retiredCopy);
    expect(doc.container.textContent).not.toContain('文档形态');
    expect(doc.container.textContent).not.toContain('未建档');
    expect(doc.container.textContent).not.toContain('无法解析');
    doc.unmount();

    const clean = renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
    expect(within(clean.container).queryByTestId('flow-empty')).toBeNull();
    expect(clean.container.textContent).not.toContain('文档形态');
    expect(within(clean.container).queryByTestId('warn-note')).toBeNull();
  });

  it('产物区多文档 tab 切换：tab 按信封顺序、默认首项选中、点击切换仅渲染当前卡片；清单变短 active 收敛', async () => {
    const three = [
      proposalDoc(),
      envelope({ title: '任务进度' }),
      envelope({
        kind: 'unknown-kind',
        title: '未知产物',
        payload: null,
        fallbackText: '未知保底',
      }),
    ];
    const descriptors: ArtifactDescriptor[] = [
      { kind: 'markdown-doc', source: 'proposal.md', title: '提案' },
      { kind: 'tasks-progress', source: 'tasks.md', title: '任务进度' },
      { kind: 'unknown-kind', source: 'notes.md', title: '未知产物' },
    ];
    const view = renderDetail({
      detail: detail({ artifacts: descriptors }),
      artifacts: {
        'markdown-doc:proposal.md': three[0],
        'tasks-progress:tasks.md': three[1],
        'unknown-kind:notes.md': three[2],
      },
    });
    const { container } = view;
    await waitFor(() => expect(within(container).getAllByTestId('artifact-tab')).toHaveLength(3));
    const tabs = within(container).getAllByTestId('artifact-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['提案', '任务进度', '未知产物']);
    // 默认选中首项（TabsTrigger role=tab、选中态 aria-selected），且页面同时只有一张产物卡片
    expect(tabs[0].getAttribute('aria-selected')).toBe('true');
    expect(tabs[1].getAttribute('aria-selected')).toBe('false');
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
    expect(container.textContent).toContain('提案正文');
    expect(container.textContent).not.toContain('未知保底');

    // base-ui Tabs 激活绑在 click，以 click 模拟切换
    fireEvent.click(tabs[2]);
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
    expect(container.textContent).toContain('未知保底');
    expect(container.textContent).not.toContain('提案正文');
    expect(within(container).getAllByTestId('artifact-tab')[2].getAttribute('aria-selected')).toBe(
      'true',
    );

    // 显式刷新后清单变短（fixture 收敛为两产物）：active=2 越界收敛到末项（任务进度），不重置回首项
    loadFixture({
      detail: detail({ artifacts: descriptors.slice(0, 2) }),
      artifacts: {
        'markdown-doc:proposal.md': three[0],
        'tasks-progress:tasks.md': three[1],
      },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '刷新详情' }));
    });
    await waitFor(() => expect(within(container).getAllByTestId('artifact-tab')).toHaveLength(2));
    const shrunk = within(container).getAllByTestId('artifact-tab');
    expect(shrunk.map((tab) => tab.textContent)).toEqual(['提案', '任务进度']);
    expect(shrunk[1].getAttribute('aria-selected')).toBe('true');
    expect(container.textContent).toContain('100%');
  });

  it('产物区单文档不设 tab 条、卡片直接渲染；空清单「（未发现可读产物）」占位', async () => {
    const view = renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await waitFor(() =>
      expect(within(view.container).getAllByTestId('artifact-card')).toHaveLength(1),
    );
    expect(within(view.container).queryByTestId('artifact-tabs')).toBeNull();
    expect(within(view.container).getAllByTestId('artifact-card')).toHaveLength(1);

    // 显式刷新换空产物清单 fixture：占位文案呈现
    loadFixture({ detail: detail({ artifacts: [] }) });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '刷新详情' }));
    });
    await waitFor(() => expect(view.container.textContent).toContain('（未发现可读产物）'));
  });

  it('点击「刷新详情」重新拉取 get_change_detail；点击「← 返回列表」显式 navigate 落 /changes（不用历史回退）', async () => {
    renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
    const callsBefore = detailCalls().length;

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '刷新详情' }));
    });
    await waitFor(() => expect(detailCalls()).toHaveLength(callsBefore + 1));

    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    expect(probePathname()).toBe('/changes');
    // 落点为 /changes（本装置以导航桩占位，真实清单页语义归 change-list-view.test）
    expect(screen.getByTestId('nav-stub') !== null).toBe(true);
  });

  it('status 缺席 + pipeline 空（零列图）→ flow-graph 照常挂载、产物区照常，零 flow-empty 占位', async () => {
    const { container } = renderDetail({
      detail: detail({
        status: null,
        pipeline: [],
        artifacts: [{ kind: 'markdown-doc', source: 'proposal.md', title: '提案' }],
      }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await waitFor(() => expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1));
    // 图区常驻（零列图仍挂 flow-graph）——文档形态空图占位分支整体退役
    expect(within(container).getByTestId('flow-graph') !== null).toBe(true);
    expect(within(container).queryAllByTestId('flow-column')).toHaveLength(0);
    expect(within(container).queryByTestId('flow-empty')).toBeNull();
    // 产物区不受 pipeline 空影响照常渲染
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
    expect(container.textContent).toContain('提案正文');
  });

  it('建档 change 零 attempt（pipeline 9 空站）→ flow-graph 正常渲染 9 列', async () => {
    renderDetail({
      detail: detail({ pipeline: PIPELINE.map((phase) => ({ phase, attempts: [] })) }),
    });
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    expect(screen.queryByTestId('flow-empty')).toBeNull();
  });

  it('error / loading（无 detail）/ 未找到三态降级页与 detail-note / error-note 挂钩归属正确', async () => {
    // 未找到：get_change_detail 应答 null（非错误）
    const unfound = renderDetail({ detail: null });
    await waitFor(() => expect(unfound.container.textContent).toContain('未找到该 change。'));
    expect(within(unfound.container).getByTestId('detail-note') !== null).toBe(true);
    expect(within(unfound.container).queryByTestId('error-note')).toBeNull();

    // loading：get_change_detail 挂起（请求未返回）
    const loading = renderDetail({ detail: null, hold: true });
    await waitFor(() => expect(loading.container.textContent).toContain('加载中…'));
    expect(within(loading.container).getByTestId('detail-note') !== null).toBe(true);
    expect(within(loading.container).queryByTestId('error-note')).toBeNull();

    // error：挂起请求以 reject 落定（错误提示样式挂钩归属正确）
    detailResult = 'IPC 断开';
    await act(async () => {
      releaseDetail?.();
    });
    await waitFor(() => expect(loading.container.textContent).toContain('详情加载失败：IPC 断开'));
    const note = within(loading.container).getByTestId('error-note');
    expect(note.textContent).toContain('详情加载失败：IPC 断开');
    expect(within(loading.container).queryByTestId('detail-note')).toBeNull();
  });

  it('loading 中已有 detail 不回落加载占位，刷新按钮禁用', async () => {
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');

    // 刷新请求挂起：loading 置位而既有 detail 在场 → 不回落加载占位
    loadFixture({ detail: detail(), artifacts: PROPOSAL_ENVELOPES, hold: true });
    fireEvent.click(screen.getByRole('button', { name: '刷新详情' }));
    expect(screen.queryByText('加载中…')).toBeNull();
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    expect(screen.getByRole('button', { name: '刷新详情' }).hasAttribute('disabled')).toBe(true);

    await act(async () => {
      releaseDetail?.();
    });
  });
});

describe('ChangeDetailView：路由参数取数、深链与根切换抑制（页面自取数，invoke 边界观察）', () => {
  it('详情路由 /changes/:id：get_change_detail 以 (root, id) 调用恰一次（URL 段 id 即取数键）', async () => {
    renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');

    expect(detailCalls()).toHaveLength(1);
    expect(detailCalls()[0]).toEqual({ root: ROOT, id: CHANGE_ID });
    // 页内身份源 = URL id：name 非取数键（负断言防回归 name 寻址）
    expect(detailCalls()[0]).not.toHaveProperty('change');
  });

  it('未知 id 深链透传（/changes/ghost）：以 (root, ghost) 取数（null 应答），呈现「未找到该 change。」降级页且不崩', async () => {
    renderDetail({ detail: null }, ROOT, '/changes/ghost');

    await waitFor(() => expect(screen.getByText('未找到该 change。') !== null).toBe(true));
    expect(detailCalls()).toEqual([{ root: ROOT, id: 'ghost' }]);
    // detail=null → 既有「未找到该 change。」降级兜底，返回入口在场
    expect(screen.getByRole('button', { name: '← 返回列表' }) !== null).toBe(true);
    // 未找到降级页零图区（不静默回落空图面）
    expect(screen.queryByTestId('flow-graph')).toBeNull();
  });

  it('详情态以新 root 重渲染：过渡轮抑制取数（零「新根 + 旧 id」误发），replace 导航落 /changes；再入详情恢复 (新root, 新 id) 取数', async () => {
    const view = renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
    expect(detailCalls()).toEqual([{ root: ROOT, id: CHANGE_ID }]);

    rerenderDetail(view, { detail: detail() }, SECOND);

    // 导航落点：/changes（URL 无 :id 段）
    await waitFor(() => expect(probePathname()).toBe('/changes'));
    expect(screen.getByTestId('nav-stub') !== null).toBe(true);

    // 抑制不变量（invoke 观察面）：过渡轮 useChangeDetail(SECOND, null) 无从
    // 取数 → 零「新根 + 旧 id」误发、抑制轮零 invoke（抑制位随组件卸载终结；
    // 端到端「无新根 + 旧 id 误发」由 app.test 的 invoke 记录承接）
    expect(detailCalls()).toHaveLength(1);

    // 组件卸载后抑制不残留：再入详情（新根 + 新 id）恢复正常取数
    loadFixture({ detail: detail({ id: SECOND_CHANGE_ID }) });
    fireEvent.click(screen.getByTestId('nav-stub'));
    expect(probePathname()).toBe(`/changes/${SECOND_CHANGE_ID}`);
    await waitFor(() =>
      expect(detailCalls().at(-1)).toEqual({ root: SECOND, id: SECOND_CHANGE_ID }),
    );
  });
});

// ---------------------------------------------------------------------------
// run 组装（unify-run-state-persistence 两钩并一后形态）：页面调
// useChangeFlowRun（控制动作 + 通知订阅）、RunControlPanel 挂 activeRun 活面
// 与 runs 尾行、runStep overlay 自统一视图派生（runs[].steps ∪
// activeRun.steps 同一转换函数）、抽屉转录经 refreshKey 重查。运行态经统一
// 查询 activeRun 面驱动（change_flow_state 快照命令退役），通知 kind-only
// 经 Channel 直投（step → 统一视图重查 300ms 去抖、sessionEvent → 转录重查
// 150ms 去抖）；ResizeObserver / getBBox 垫片沿既有装置。
// ---------------------------------------------------------------------------

interface ChannelLike {
  onmessage: ((message: unknown) => void) | null;
  push(message: unknown): void;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (instance === undefined) throw new Error('订阅未建立：无 Channel 实例');
  return instance;
}

/** 统一视图活面 fixture（ActiveRunView 同形；缺省运行中 implement attempt 1）。 */
function activeRunView(overrides: Partial<ActiveRunView> = {}): ActiveRunView {
  return {
    runId: 'run-1727',
    status: 'running',
    phase: 'implement',
    attempt: 1,
    ask: null,
    startedAt: '2026-10-01T08:00:00.000Z',
    steps: [],
    ...overrides,
  };
}

/** 库读史 run 尾行 fixture（ChangeRunEntry 同形——收口后终态徽章与记因）。 */
function finishedRun(overrides: Partial<ChangeRunEntry> = {}): ChangeRunEntry {
  return {
    runId: 'run-1727',
    status: 'completed',
    reason: '全相位通过',
    startedAt: '2026-10-01T08:00:00.000Z',
    finishedAt: '2026-10-01T08:04:00.000Z',
    steps: [],
    ...overrides,
  };
}

/** 密封文本消息事件 fixture（message 块 → AgentTimeline block-text 呈现）。 */
function textEvent(seq: number, role: 'user' | 'assistant', text: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000 + seq,
    kind: 'message',
    role,
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

/** 会话清单项 fixture（agent_sessions 应答；轮行已收敛 = 重放形态）。 */
function transcriptSession(sessionId: string, sourceRef: string): SessionSummary {
  return {
    row: {
      id: sessionId,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'change', sourceRef },
      createdAt: 1727000000000,
      updatedAt: 1727000001000,
    },
    stats: { turnCount: 1, totalDurationMs: 1234, inputTokens: null, outputTokens: null },
    turns: [
      {
        turnId: 1,
        sessionId,
        status: 'completed',
        startedAt: 1727000000000,
        finishedAt: 1727000001000,
        numTurns: 1,
        costUsd: 0.5,
        durationMs: 1234,
        error: null,
      },
    ],
  };
}

describe('ChangeDetailView：run 控制面板与运行 overlay 组装', () => {
  let sessionsFixture: Record<string, SessionSummary[]>;
  let transcriptFixture: Record<string, AgentEvent[]>;
  let detailFixture: Record<string, SessionSummary | null>;

  beforeEach(() => {
    activeRunResult = null;
    sessionsFixture = {};
    transcriptFixture = {};
    detailFixture = {};
    invokeMock.mockImplementation(
      (command: string, args: { sourceRef?: string; sessionId?: string } = {}) => {
        // 详情取数经共享定向（useChangeDetail 真实组合的进程边界）
        if (command === 'get_change_detail' || command === 'read_artifact') {
          return detailIpc(command, args);
        }
        if (command === 'change_flow_start') {
          return Promise.resolve({ runId: 'run-1727', status: 'running' });
        }
        if (command === 'session_detail') {
          const hit = detailFixture[args.sessionId ?? ''];
          return Promise.resolve(hit ?? null);
        }
        if (command === 'agent_sessions') {
          return Promise.resolve(sessionsFixture[args.sourceRef ?? ''] ?? []);
        }
        if (command === 'agent_session_transcript') {
          return Promise.resolve(transcriptFixture[args.sessionId ?? ''] ?? []);
        }
        // change_flow_watch / stop / confirm / answer：空操作应答
        return Promise.resolve(null);
      },
    );
  });

  afterEach(() => {
    // 恢复缺省应答：用例乱序（sequence.shuffle）时既有用例恒拿到 Promise 空态
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) =>
      detailIpc(command, args),
    );
  });

  it('页面头部区挂 RunControlPanel：无运行 run 时发起入口在场（停止 / 状态徽章不呈现），既有组装并存零回归', async () => {
    const { container } = renderDetail({
      detail: detail(),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('flow-graph');
    expect(screen.getByTestId('run-control-panel').textContent).toContain('运行控制');
    expect(screen.getByTestId('run-start') !== null).toBe(true);
    expect(screen.queryByTestId('run-stop')).toBeNull();
    expect(screen.queryByTestId('run-status')).toBeNull();
    expect(screen.queryByTestId('run-error')).toBeNull();
    // 既有组装（新增为加法）：Header + 图区 + 产物区
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);

    // 发起入口接通 useChangeFlowRun：change_flow_start 携 root / change id 与 Channel
    fireEvent.click(screen.getByTestId('run-start'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('change_flow_start', {
        onEvent: expect.anything(),
        root: ROOT,
        id: CHANGE_ID,
        autoNextPhase: false,
      }),
    );
  });

  it('运行中（统一查询 activeRun 在场）→ 停止入口在场；活面步表经通知触发重查流入图，既有事件节点不受影响', async () => {
    loadActiveRun(activeRunView());
    renderDetail({ detail: detail() });

    // activeRun 在场 → 状态徽章与停止入口呈现、watch 补订建立
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));
    await screen.findByTestId('flow-graph');
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');

    // 通知到达（kind-only step）→ 300ms 尾随去抖 → 统一视图重查：fixture 活面
    // 已携步表 → runStep 节点出现在图（同一转换函数派生）
    loadActiveRun(
      activeRunView({
        steps: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-exec',
            detail: null,
          },
          {
            phase: 'implement',
            attempt: 1,
            step: 'staticCheck',
            status: 'failed',
            sessionId: null,
            detail: '2 处诊断',
          },
        ],
      }),
    );
    await act(async () => {
      lastChannel().push({ ipc: 'step' });
    });
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2), {
      timeout: 5000,
    });
    const executor = screen.getByTestId('rf__node-run:implement:1:executor');
    expect(within(executor).getByTestId('run-step-group').textContent).toBe('WorkerAgent');
    const check = screen.getByTestId('rf__node-run:implement:1:staticCheck');
    expect(within(check).getByTestId('run-step-group').textContent).toBe('ToolStep');
    expect(within(check).getByTestId('run-step-detail').textContent).toBe('2 处诊断');
    // overlay 为加法：既有事件节点集合不变（flow-node 挂钩 = eval/active 事件
    // 节点面；运行步节点经 run-step-node 挂钩另行可辨）
    expect(screen.getAllByTestId('flow-node')).toHaveLength(2);
  });

  it('重挂恢复（AC-10）：run 运行中 remount → activeRun 经统一查询恢复、步表非空且与 emit 序一致、运行中才补订 watch', async () => {
    // 运行中形态：activeRun 携全词汇 emit 序步表（非空——重挂步表不再恒空）
    const liveSteps: ChangeStepState[] = [
      {
        phase: 'implement',
        attempt: 1,
        step: 'executor',
        status: 'passed',
        sessionId: 'ses-exec',
        detail: null,
      },
      {
        phase: 'implement',
        attempt: 1,
        step: 'staticCheck',
        status: 'running',
        sessionId: null,
        detail: null,
      },
    ];
    loadActiveRun(activeRunView({ steps: liveSteps }));
    const first = renderDetail({ detail: detail() });
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    first.unmount();

    // remount：统一查询先行（activeRun 面）→ 步表与 emit 序一致恢复上图 +
    // 运行中才补订 watch（第二个 Channel 实例）
    loadActiveRun(activeRunView({ steps: liveSteps }));
    renderDetail({ detail: detail() });
    await screen.findByTestId('run-stop');
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(2), { timeout: 5000 });
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    // 步表与 emit 序一致：executor（passed）在前、staticCheck（running）随后
    const nodes = screen.getAllByTestId('run-step-node');
    expect(nodes[0].getAttribute('data-run-status')).toBe('passed');
    expect(nodes[1].getAttribute('data-run-status')).toBe('running');
    expect(screen.getByTestId('rf__node-run:implement:1:staticCheck') !== null).toBe(true);
  });

  it('库读史常驻（图常驻渲染）：runs[].steps 零通知即上图（收口后不回落），activeRun 活步叠加其上', async () => {
    loadActiveRun(
      activeRunView({
        steps: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'evaluator',
            status: 'running',
            sessionId: 'ses-eval',
            detail: null,
          },
        ],
      }),
    );
    renderDetail({
      detail: detail({
        runs: [
          finishedRun({
            steps: [
              {
                seq: 0,
                phase: 'implement',
                attempt: 1,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-exec',
                detail: null,
              },
            ],
          }),
        ],
      }),
    });
    await screen.findByTestId('flow-graph');

    // 库读史步节点常驻（passed 终态样式）+ 活步叠加（running）
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    expect(screen.getByTestId('rf__node-run:implement:1:executor') !== null).toBe(true);
    expect(screen.getByTestId('rf__node-run:implement:1:evaluator') !== null).toBe(true);
  });

  it('两 run 全史叠加 + 第三 run 活步叠加（AC-9）：同列同相位 attempt 递增各自成节点可辨，键 phase:attempt:step 无冲突', async () => {
    // 第三 run 在飞活步（attempt 3，库面无史——首拍未收口）
    loadActiveRun(
      activeRunView({
        runId: 'run-3',
        attempt: 3,
        steps: [
          {
            phase: 'implement',
            attempt: 3,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-live-3',
            detail: null,
          },
        ],
      }),
    );
    renderDetail({
      detail: detail({
        runs: [
          finishedRun({
            runId: 'run-1',
            steps: [
              {
                seq: 0,
                phase: 'implement',
                attempt: 1,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-1',
                detail: null,
              },
            ],
          }),
          finishedRun({
            runId: 'run-2',
            startedAt: '2026-10-01T09:00:00.000Z',
            steps: [
              {
                seq: 0,
                phase: 'implement',
                attempt: 2,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-2',
                detail: null,
              },
            ],
          }),
        ],
      }),
    });
    await screen.findByTestId('flow-graph');

    // 两 run 库读史 + 一 run 活步 = 三节点全量上图（全史不回落 + 在飞叠加）
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(3));
    // 同列 attempt 递增不撞键：三节点各自成节点、库读史两枚终态、活步一枚运行中
    const status = (nodeId: string) =>
      within(screen.getByTestId(nodeId))
        .getByTestId('run-step-node')
        .getAttribute('data-run-status');
    expect(status('rf__node-run:implement:1:executor')).toBe('passed');
    expect(status('rf__node-run:implement:2:executor')).toBe('passed');
    expect(status('rf__node-run:implement:3:executor')).toBe('running');
  });

  it('run 终态通知触发统一视图重查：收口后主操作回到发起、收口记因自 runs 尾行呈现', async () => {
    loadActiveRun(activeRunView());
    renderDetail({ detail: detail() });
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));
    const baseline = detailCalls().length;

    // fixture 换收口形态（refresh 的应答面——终态已除名、runs 尾行在案），再投
    // finished 通知（300ms 去抖后重查）
    loadActiveRun(null);
    loadFixture({ detail: detail({ runs: [finishedRun()] }) });
    await act(async () => {
      lastChannel().push({ ipc: 'finished' });
    });
    await waitFor(() => expect(detailCalls().length).toBeGreaterThan(baseline), {
      timeout: 5000,
    });
    expect(screen.getByTestId('run-finished-reason').textContent).toContain('全相位通过');
    // 收口对齐：终局后主操作回到发起
    expect(screen.getByTestId('run-start') !== null).toBe(true);
  });

  it('抽屉选中运行步节点 → 转录 props 下传链路接通：运行步实时 sessionId 直查 session_detail、会话事件通知经 refreshKey 触发转录库重查', async () => {
    loadActiveRun(
      activeRunView({
        steps: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-exec',
            detail: null,
          },
        ],
      }),
    );
    detailFixture = {
      'ses-exec': transcriptSession('ses-exec', `${CHANGE_ID}/implement/executor/1`),
    };
    transcriptFixture = { 'ses-exec': [textEvent(0, 'user', '重放正文')] };
    renderDetail({ detail: detail() });
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));

    fireEvent.click(await screen.findByTestId('rf__node-run:implement:1:executor'));

    const panel = await screen.findByTestId('session-transcript-panel');
    await waitFor(() => expect(panel.textContent).toContain('重放正文'));
    // 运行步节点携实时 sessionId → 槽位 id 直查（desktop-change-session-visibility
    // 直查优先半边的页面级投影）
    expect(invokeMock).toHaveBeenCalledWith('session_detail', {
      root: ROOT,
      sessionId: 'ses-exec',
    });

    // 会话事件通知到站：转录库已追加密封事件，150ms 去抖后 refreshKey 重查
    transcriptFixture['ses-exec'] = [
      textEvent(0, 'user', '重放正文'),
      textEvent(1, 'assistant', '实时增量正文'),
    ];
    await act(async () => {
      lastChannel().push({ ipc: 'sessionEvent' });
    });
    await waitFor(
      () =>
        expect(screen.getByTestId('session-transcript-panel').textContent).toContain(
          '实时增量正文',
        ),
      { timeout: 5000 },
    );
    expect(screen.getByTestId('session-transcript-panel').textContent).toContain('重放正文');
  });
});

// ---------------------------------------------------------------------------
// 退役面负断言与详情图区恒态（desktop-workflow-db-state / desktop-change-db-identity）：
// detail DTO 已删 inventory / unparsable / fileLog 三字段（TS 类型面随 bindings
// 同步删除），视图退役代际徽章、「workflow.json 无法解析」警示条与 workflow 独立
// 面板（含文件表）；文档形态空图占位（flow-empty）随 db 单源语义整体退役——
// 详情可达者恒为建档 change，流程图区常驻。
// ---------------------------------------------------------------------------

describe('ChangeDetailView：退役元素零渲染与详情图区恒态', () => {
  it('detail DTO 删三字段 → 代际徽章、「workflow.json 无法解析」警示条、workflow 独立面板（含文件表）零渲染', async () => {
    const { container } = renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');

    // 代际徽章退役：historical InventoryBadge 挂点（detail-header 域内）无 v0/v1/v2 文本
    // （产物卡版本脚注 v1 为信封版本面，不在该域内）
    const header = within(container).getByTestId('detail-header');
    expect(within(header).queryByText(/v[012]/)).toBeNull();
    // 「workflow.json 无法解析」警示条退役：historical warn-note 挂点与文案零残留
    expect(within(container).queryByTestId('warn-note')).toBeNull();
    expect(container.textContent).not.toContain('无法解析');
    expect(container.textContent).not.toContain('workflow.json');
    // workflow 独立面板（含文件表）退役：historical testid 零残留
    expect(within(container).queryByTestId('workflow-panel')).toBeNull();
    expect(within(container).queryByTestId('filelog-table')).toBeNull();
  });

  it('详情恒走图区：status 在场 → 9 站流水线图 + 抽屉入口；status 缺席 + pipeline 空 → 同走图区（零列图）不崩溃不空白，产物区照常', async () => {
    // 建档：9 列流水线 + 列头点击开抽屉（单一交互入口可达）
    const filed = renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    filed.unmount();

    // status 缺席 + pipeline 空：图区照常（零列图）+ 产物卡片照常渲染
    //（有实质内容，非空白非崩溃；flow-empty 占位与其「文档形态」文案零出现）
    const doc = renderDetail({
      detail: detail({
        status: null,
        pipeline: [],
        artifacts: [{ kind: 'markdown-doc', source: 'proposal.md', title: '提案' }],
      }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await waitFor(() =>
      expect(within(doc.container).getAllByTestId('artifact-card')).toHaveLength(1),
    );
    expect(within(doc.container).getByTestId('flow-graph') !== null).toBe(true);
    expect(within(doc.container).queryByTestId('flow-empty')).toBeNull();
    expect(doc.container.textContent).not.toContain('文档形态');
    expect(doc.container.textContent).toContain('提案正文');
    expect(doc.container.textContent).not.toContain('无法解析');
  });

  it('面板传参收敛：命令面 prop changeId == URL id、展示面 name == detail.name（页内单一身份源 AC-7）', async () => {
    // URL 段 id 与 detail.name 刻意不同：命令面（取数 / 运行 / 归档 / 转录
    // 反查）恒取 URL id；展示面（标题 / aria-label）恒取 detail.name
    const urlId = '0199a2f0-2003-7e45-8a9b-000000002003';
    renderDetail(
      { detail: detail({ id: urlId, name: 'add-feature' }), artifacts: PROPOSAL_ENVELOPES },
      ROOT,
      `/changes/${urlId}`,
    );
    await screen.findByTestId('flow-graph');

    // 取数面：URL id 即取数键
    expect(detailCalls()).toEqual([{ root: ROOT, id: urlId }]);
    // 展示面：头部标题与两面板 aria-label 取 detail.name
    expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true);
    expect(screen.getByLabelText('change add-feature 运行控制') !== null).toBe(true);

    // 命令面（运行控制面板）：change_flow_start 携 URL id 而非 name
    fireEvent.click(screen.getByTestId('run-start'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('change_flow_start', {
        onEvent: expect.anything(),
        root: ROOT,
        id: urlId,
        autoNextPhase: false,
      }),
    );

    // 命令面（归档面板）：确认对话 aria-label 取 detail.name；preflight 指令带 URL id
    fireEvent.click(screen.getByTestId('archive-trigger'));
    await screen.findByLabelText('change add-feature 归档确认');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('archive_flow_preflight', { root: ROOT, id: urlId }),
    );

    // 命令面（抽屉）：eval 节点选中 → 转录反查 sourceRef 身份段 = URL id
    fireEvent.click(screen.getByTestId(`rf__node-eval:proposal:1`));
    await screen.findByTestId('detail-drawer');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
        root: ROOT,
        source: 'change',
        sourceRef: `${urlId}/proposal/executor/1`,
      }),
    );
  });
});

// ---------------------------------------------------------------------------
// 归档入口（desktop-archive-change D14——DetailHeader 归档按钮两态 + ArchivePanel
// 挂载位 + 链终态一次 refresh 回落已归档形态。归档面板状态经真实 useArchiveFlow
// 走 mock IPC：archive_flow_state 出快照 → Channel 投递信封驱动终态）
// ---------------------------------------------------------------------------

describe('ChangeDetailView：归档入口（按钮两态 / panel 挂载位 / 终态 refresh）', () => {
  it('detail status=active → archive-trigger 呈现；archived / status 缺席 → 不渲染（负断言）', async () => {
    // active 建档：按钮在场
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('archive-trigger');
    cleanup();

    // 已归档：按钮不渲染
    renderDetail({
      detail: detail({ status: 'archived', source: 'archive' }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('detail-header');
    expect(screen.queryByTestId('archive-trigger')).toBeNull();
    cleanup();

    // status 缺席（status null）：无入口
    renderDetail({ detail: detail({ status: null }), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('detail-header');
    expect(screen.queryByTestId('archive-trigger')).toBeNull();
  });

  it('归档面板挂载位：active 形态点击触发 → panel 于 detail-header 之后、run 控制面板之前；status 非 active → panel 不渲染', async () => {
    const view = renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    fireEvent.click(await screen.findByTestId('archive-trigger'));

    // panel 呈现（确认对话形态）且 DOM 序位于 detail-header 之后
    const panel = await screen.findByTestId('archive-confirm-dialog');
    const header = screen.getByTestId('detail-header');
    expect(header.compareDocumentPosition(panel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    // 与 run 面并置但组件隔离：run 控制面板在场且位于 panel 之后（DOM 序锚）
    const runPanelButton = document.querySelector('[data-testid="run-auto-next-phase"]');
    expect(runPanelButton).not.toBeNull();
    expect(
      panel.compareDocumentPosition(runPanelButton as Node) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    // status 非 active（已归档形态）：panel 挂载条件不成立（按钮本就不渲染——
    // 两态分流由上一用例负断言；本行防渲染器回归的重挂双保险）
    view.unmount();
    renderDetail({
      detail: detail({ status: 'archived', source: 'archive' }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('detail-header');
    expect(screen.queryByTestId('archive-confirm-dialog')).toBeNull();
  });

  it('链终态 refresh：Finished 信封 → detail 重查恰一次、回落已归档形态（archive-trigger 消失）', async () => {
    // archive_flow_state 出运行中快照（hook 补订实时流），detail 初态 active
    const original = invokeMock.getMockImplementation();
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === 'archive_flow_state') {
        return Promise.resolve({
          stages: [{ stage: 'seal', status: 'running', detail: null }],
          sessionId: 'sess-arch',
        });
      }
      return original!(command, args);
    });
    loadFixture({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    render(
      <MemoryRouter initialEntries={[`/changes/${CHANGE_ID}`]}>{detailTree(ROOT)}</MemoryRouter>,
    );
    await screen.findByTestId('archive-trigger');

    // fixture 换已归档形态（refresh 的应答面——链收口后 db 事实已翻转），再投
    // Finished 终态信封（hook 归并 → finished → onFinish=refresh）
    loadFixture({
      detail: detail({ status: 'archived', source: 'archive' }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    const channel = ChannelMock.instances.at(-1);
    expect(channel).toBeDefined();
    act(() => {
      channel?.push({
        ipc: 'finished',
        summary: {
          name: 'add-feature',
          archivedDir: '2026-10-08-add-feature',
          specs: 'none',
          warnings: [],
        },
        error: null,
      });
    });

    // refresh：detail 重查恰一次（初始 + 终态各一次）；回落已归档形态
    await waitFor(() => expect(detailCalls()).toHaveLength(2));
    await waitFor(() => expect(screen.queryByTestId('archive-trigger')).toBeNull());
    expect(screen.getByTestId('detail-header').textContent).toContain('已归档');
  });
});

// ---------------------------------------------------------------------------
// 详情头渲染 detail.title（explore-name-file-binding AC-8 change 详情半边）：
// 标题面取 title（恒非空），name 保留供归档 / run 控制面板
// ---------------------------------------------------------------------------

describe('ChangeDetailView：详情头渲染 detail.title（AC-8）', () => {
  it('正向：title ≠ name 时标题取 detail.title（name 零作标题展示）', async () => {
    const urlId = '0199a2f0-2009-7e45-8a9b-000000002009';
    renderDetail(
      {
        detail: detail({ id: urlId, name: 'add-feature', title: '新增特性（人类可读）' }),
        artifacts: PROPOSAL_ENVELOPES,
      },
      ROOT,
      `/changes/${urlId}`,
    );
    await screen.findByTestId('flow-graph');

    expect(screen.getByRole('heading', { name: '新增特性（人类可读）' }) !== null).toBe(true);
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
  });

  it('边界：name 保留供面板（归档 / run 控制面板 aria-label 仍取 detail.name）', async () => {
    const urlId = '0199a2f0-200a-7e45-8a9b-00000000200a';
    renderDetail(
      {
        detail: detail({ id: urlId, name: 'add-feature', title: '新增特性（人类可读）' }),
        artifacts: PROPOSAL_ENVELOPES,
      },
      ROOT,
      `/changes/${urlId}`,
    );
    await screen.findByTestId('flow-graph');

    // 展示面取 title、命令面 / 面板面取 name（双字段可辨）
    expect(screen.getByLabelText('change add-feature 运行控制') !== null).toBe(true);
    fireEvent.click(screen.getByTestId('archive-trigger'));
    await screen.findByLabelText('change add-feature 归档确认');
  });
});
