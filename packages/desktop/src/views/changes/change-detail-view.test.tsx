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
  SessionPage,
  SessionSummary,
} from '../../types/dto';
import { ChangeDetailView } from './change-detail-view';
import { mountMaterials } from './flow/attachments';

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

// 挂载层观测面（change-flow-show-step-only，AC-4 / D6）：materials memo 依赖收窄
// 与两参调用形态经 `spy: true` 自动 mock 观测（ESM 命名空间不支持 `vi.spyOn`；
// 自动 mock 保留原实现——调用次数可观测、行为零替换）
vi.mock(import('./flow/attachments'), { spy: true });

// ---------------------------------------------------------------------------
// 详情取数 fixture（useChangeDetail 的进程边界应答面）：get_change_detail 与
// read_artifact 按命令名 + 参数分发；挂起门闩驱动 loading / 刷新竞态形态。
// ---------------------------------------------------------------------------

let detailResult: ChangeDetail | string | null = null;
let activeRunResult: ActiveRunView | null = null;
let artifactResults: Record<string, ArtifactEnvelope | null> = {};
let holdDetail = false;
/// `archi_graph` reject 注入位（架格页签错误态用例）
let archiReject: string | null = null;

/** 架构图 fixture（两元素零关系零诊断）。 */
const ARCHI_ELEMENTS = [
  {
    fqn: 'App',
    kind: 'package' as const,
    gate: 'off' as const,
    path: [],
    summary: null,
    metadata: {},
    modelFile: '.dev-team/architecture/models/01-core.toml',
    modelLine: 2,
    mark: 'none' as const,
  },
  {
    fqn: 'App.Auth',
    kind: 'domain' as const,
    gate: 'strict' as const,
    path: [],
    summary: null,
    metadata: {},
    modelFile: '.dev-team/architecture/models/01-core.toml',
    modelLine: 6,
    mark: 'none' as const,
  },
];
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
  archiReject = null;
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
  // 架构页签取数（desktop-archi-canvas 挂载点 B）：archi_graph 回健康图，
  // archi_module 回元素详情，archi_set_gate 回 null（增量改档）
  if (command === 'archi_graph') {
    if (archiReject !== null) return Promise.reject(archiReject);
    return Promise.resolve({
      elements: ARCHI_ELEMENTS.map((element) => ({ ...element })),
      relations: [],
      diagnostics: [],
    });
  }
  if (command === 'archi_module') {
    return Promise.resolve({
      element: { ...ARCHI_ELEMENTS[0] },
      dependsOn: [],
      dependedBy: [],
      children: [],
      docs: [],
      warnings: [],
    });
  }
  if (command === 'archi_set_gate') return Promise.resolve(null);
  // 通用 Agent 面板面（desktop-change-chat）：发起提前 resolve 的 running 轮行、
  // 清单缺省空页、停止幂等空应答；会话单查 / 转录沿用下方空态（既有用例零发起，
  // 互不影响）
  if (command === 'agent_start') return Promise.resolve(panelStartRecord());
  if (command === 'agent_stop') return Promise.resolve(null);
  if (command === 'list_change_chat_sessions') {
    return panelListResult === null
      ? Promise.resolve({ sessions: [], total: 0 })
      : Promise.resolve(panelListResult(args));
  }
  // run / 会话命令缺省空态应答（无运行 run / 空会话清单）
  return Promise.resolve(null);
}

/** 面板发起应答（提前 resolve 的 running 轮行；会话 id 即面板 `cwd` 组合面的报锚）。 */
function panelStartRecord(): SessionSummary['turns'][number] {
  return {
    turnId: 1,
    sessionId: 'ses-chat-1',
    status: 'running',
    startedAt: 1_727_000_000_000,
    finishedAt: null,
    numTurns: null,
    costUsd: null,
    durationMs: null,
    error: null,
  };
}

/** 面板历史清单应答（按 args 分派；null = 缺省空页）。 */
let panelListResult: ((args: Record<string, unknown>) => SessionPage) | null = null;

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
            // 分栏（react-resizable-panels）读 `borderBoxSize[0]` 判分支：垫片
            // 同步给出报告尺寸（jsdom 无布局，尺寸取垫片口径）
            borderBoxSize: [
              {
                inlineSize: (target as HTMLElement).offsetWidth,
                blockSize: (target as HTMLElement).offsetHeight,
              },
            ],
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
  // jsdom 无 CSS 引擎：根元素 fontSize 计算值为空，`3rem` 收起尺寸解析成 NaN。
  // 按浏览器默认根字号（16px）垫片，使分栏的 rem 尺寸与真实渲染同解
  document.documentElement.style.fontSize = '16px';
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

/** requirement 相位序夹具（D12：前端 `PIPELINE_PHASES` 常量退役，列序由
 * `detail.pipeline` 出线承载）：9 列，archive 为末列、无 code-analyze 占位列。 */
const PIPELINE = [
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

/** test-only 相位序夹具：6 列，code-analyze 第 2 列、archive 第 6 列。 */
const TEST_ONLY_PIPELINE = [
  'proposal',
  'code-analyze',
  'test-design',
  'test-gen',
  'test-execution',
  'archive',
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
    artifacts: [{ kind: 'markdown-doc', source: 'proposal.md', title: '提案', phase: 'proposal' }],
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

describe('ChangeDetailView：页面组装（图区 / 抽屉）', () => {
  it('建档详情 → flow-graph 图区，无 workflow 独立面板与文件表', async () => {
    const { container } = renderDetail({
      detail: detail(),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('flow-graph');
    // file_log 独立面板随载体退役：workflow-panel / filelog-table 零残留
    expect(within(container).queryByTestId('workflow-panel')).toBeNull();
    expect(within(container).queryByTestId('filelog-table')).toBeNull();
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

  it('列头 / 运行步节点点击 → detail-drawer 挂载（selection 状态在本组件）；关闭后卸载', async () => {
    loadActiveRun(
      activeRunView({
        nodes: [
          {
            phase: 'proposal',
            attempt: 1,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-exec-1',
            detail: null,
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
          },
        ],
      }),
    );
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(1));

    // 列头 → 该站过程素材单列（挂载层两参调用：文档 + checklist 均挂列头）
    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    expect(screen.getByTestId('drawer-column-section').textContent).toContain('提案正文');
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    expect(screen.queryByTestId('detail-drawer')).toBeNull();

    // 运行步节点（role 在场）→ 会话转录单列（页面组装层 selection 上抛链路可用）
    fireEvent.click(screen.getByTestId('rf__node-run:run-1727:proposal:1:executor:0'));
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    await screen.findByTestId('session-transcript-panel');
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'proposal · attempt 1 · runtime',
    );
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

  it('点击「刷新详情」重新拉取 get_change_detail；点击「← 返回列表」显式 navigate 落 /changes（不用历史回退）', async () => {
    renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
    const callsBefore = detailCalls().length;

    await act(async () => {
      fireEvent.click(screen.getByTestId('refresh-change-detail-button'));
    });
    await waitFor(() => expect(detailCalls()).toHaveLength(callsBefore + 1));

    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    expect(probePathname()).toBe('/changes');
    // 落点为 /changes（本装置以导航桩占位，真实清单页语义归 change-list-view.test）
    expect(screen.getByTestId('nav-stub') !== null).toBe(true);
  });

  it('status 缺席 + pipeline 空（零列图）→ flow-graph 照常挂载，零 flow-empty 占位', async () => {
    const { container } = renderDetail({
      detail: detail({
        status: null,
        pipeline: [],
        artifacts: [
          { kind: 'markdown-doc', source: 'proposal.md', title: '提案', phase: 'proposal' },
        ],
      }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    // 图区常驻（零列图仍挂 flow-graph）——文档形态空图占位分支整体退役
    await waitFor(() => expect(within(container).getByTestId('flow-graph')).not.toBeNull());
    expect(within(container).queryAllByTestId('flow-column')).toHaveLength(0);
    expect(within(container).queryByTestId('flow-empty')).toBeNull();
  });

  it('建档 change 零 attempt（pipeline 9 空站含 archive）→ flow-graph 正常渲染 9 列', async () => {
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
    fireEvent.click(screen.getByTestId('refresh-change-detail-button'));
    expect(screen.queryByText('加载中…')).toBeNull();
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    expect(screen.getByTestId('refresh-change-detail-button').hasAttribute('disabled')).toBe(true);

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
// 与 runs 尾行、runStep overlay 自统一视图派生（runs[].nodes ∪
// activeRun.nodes 同一转换函数）、抽屉转录经 refreshKey 重查。运行态经统一
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
    archive: null,
    startedAt: '2026-10-01T08:00:00.000Z',
    nodes: [],
    edges: [],
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
    nodes: [],
    edges: [],
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

/** store `SessionConfigSnapshot` 快照线形态（serde camelCase；v2 增
 * `agentInstanceId` 实例外键——fixture 缺键即类型红，防漂移）。 */
interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

/** 会话清单项 fixture（agent_sessions 应答；轮行已收敛 = 重放形态）。 */
function transcriptSession(sessionId: string, sourceRef: string): SessionSummary {
  return {
    row: {
      id: sessionId,
      remoteSessionId: null,
      configSnapshot: {
        engine: 'sdk',
        agentInstanceId: null,
        model: 'glm-high',
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
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
    renderDetail({
      detail: detail(),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('flow-graph');
    expect(screen.getByTestId('run-start') !== null).toBe(true);
    expect(screen.queryByTestId('run-stop')).toBeNull();
    expect(screen.queryByTestId('run-status')).toBeNull();
    expect(screen.queryByTestId('run-error')).toBeNull();

    // 发起入口接通 useChangeFlowRun：change_flow_start 携 root / change id 与 Channel
    fireEvent.click(screen.getByTestId('run-start'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('change_flow_start', {
        onEvent: expect.anything(),
        root: ROOT,
        id: CHANGE_ID,
        autoNextPhase: true,
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
        nodes: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-exec',
            detail: null,
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
          },
          {
            phase: 'implement',
            attempt: 1,
            step: 'staticCheck',
            status: 'failed',
            sessionId: null,
            detail: '2 处诊断',
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
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
    const executor = screen.getByTestId('rf__node-run:run-1727:implement:1:executor:0');
    expect(within(executor).getByTestId('run-step-group').textContent).toBe('Agent');
    const check = screen.getByTestId('rf__node-run:run-1727:implement:1:staticCheck:0');
    expect(within(check).getByTestId('run-step-group').textContent).toBe('Tool');
    expect(within(check).getByTestId('run-step-detail').textContent).toBe('2 处诊断');
    // 节点模型单分类：事件节点挂钩（flow-node）零残留，运行步节点经
    // run-step-node 挂钩可辨
    expect(screen.queryByTestId('flow-node')).toBeNull();
    expect(screen.getAllByTestId('run-step-node')).toHaveLength(2);
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
        instanceNo: 0,
        outputRef: null,
        startedAt: 1_727_000_000_000,
        endedAt: null,
        tokenInput: null,
        tokenOutput: null,
      },
      {
        phase: 'implement',
        attempt: 1,
        step: 'staticCheck',
        status: 'running',
        sessionId: null,
        detail: null,
        instanceNo: 0,
        outputRef: null,
        startedAt: 1_727_000_000_000,
        endedAt: null,
        tokenInput: null,
        tokenOutput: null,
      },
    ];
    loadActiveRun(activeRunView({ nodes: liveSteps }));
    const first = renderDetail({ detail: detail() });
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    first.unmount();

    // remount：统一查询先行（activeRun 面）→ 步表与 emit 序一致恢复上图 +
    // 运行中才补订 watch（第二个 Channel 实例）
    loadActiveRun(activeRunView({ nodes: liveSteps }));
    renderDetail({ detail: detail() });
    await screen.findByTestId('run-stop');
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(2), { timeout: 5000 });
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    // 步表与 emit 序一致：executor（passed）在前、staticCheck（running）随后
    const nodes = screen.getAllByTestId('run-step-node');
    expect(nodes[0].getAttribute('data-run-status')).toBe('passed');
    expect(nodes[1].getAttribute('data-run-status')).toBe('running');
    expect(screen.getByTestId('rf__node-run:run-1727:implement:1:staticCheck:0') !== null).toBe(
      true,
    );
  });

  it('库读史常驻（图常驻渲染）：runs[].nodes 零通知即上图（收口后不回落），activeRun 活节点叠加其上', async () => {
    loadActiveRun(
      activeRunView({
        nodes: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'evaluator',
            status: 'running',
            sessionId: 'ses-eval',
            detail: null,
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
          },
        ],
      }),
    );
    renderDetail({
      detail: detail({
        runs: [
          finishedRun({
            nodes: [
              {
                instanceNo: 0,
                phase: 'implement',
                attempt: 1,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-exec',
                detail: null,
                outputRef: null,
                startedAt: 1_727_000_000_000,
                endedAt: null,
                tokenInput: null,
                tokenOutput: null,
              },
            ],
          }),
        ],
      }),
    });
    await screen.findByTestId('flow-graph');

    // 库读史步节点常驻（passed 终态样式）+ 活步叠加（running）
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    expect(screen.getByTestId('rf__node-run:run-1727:implement:1:executor:0') !== null).toBe(true);
    expect(screen.getByTestId('rf__node-run:run-1727:implement:1:evaluator:0') !== null).toBe(true);
  });

  it('两 run 全史叠加 + 第三 run 活节点叠加（AC-9）：同列同相位 attempt 递增各自成节点可辨，稳定身份键 (phase,attempt,step,instanceNo) 无冲突', async () => {
    // 第三 run 在飞活步（attempt 3，库面无史——首拍未收口）
    loadActiveRun(
      activeRunView({
        runId: 'run-3',
        attempt: 3,
        nodes: [
          {
            phase: 'implement',
            attempt: 3,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-live-3',
            detail: null,
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
          },
        ],
      }),
    );
    renderDetail({
      detail: detail({
        runs: [
          finishedRun({
            runId: 'run-1',
            nodes: [
              {
                instanceNo: 0,
                phase: 'implement',
                attempt: 1,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-1',
                detail: null,
                outputRef: null,
                startedAt: 1_727_000_000_000,
                endedAt: null,
                tokenInput: null,
                tokenOutput: null,
              },
            ],
          }),
          finishedRun({
            runId: 'run-2',
            startedAt: '2026-10-01T09:00:00.000Z',
            nodes: [
              {
                instanceNo: 0,
                phase: 'implement',
                attempt: 2,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-2',
                detail: null,
                outputRef: null,
                startedAt: 1_727_000_000_000,
                endedAt: null,
                tokenInput: null,
                tokenOutput: null,
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
    expect(status('rf__node-run:run-1:implement:1:executor:0')).toBe('passed');
    expect(status('rf__node-run:run-2:implement:2:executor:0')).toBe('passed');
    expect(status('rf__node-run:run-3:implement:3:executor:0')).toBe('running');
  });

  it('重启重跑同 attempt：跨 run 同名步节点并存，选中重启轮节点 → 会话直查取重启轮 session（不串上一轮）', async () => {
    // 前一轮（run-1）与重启轮（run-2）同 (implement, 1, executor, 0)：节点 id 携
    // run 段并存；抽屉选中须按 run 解析到重启轮节点（sessionId = ses-new）
    loadActiveRun(
      activeRunView({
        runId: 'run-2',
        nodes: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-new',
            detail: null,
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
          },
        ],
      }),
    );
    detailFixture = {
      'ses-new': transcriptSession('ses-new', `${CHANGE_ID}/implement/executor/1`),
    };
    transcriptFixture = { 'ses-new': [textEvent(0, 'user', '重启轮正文')] };
    renderDetail({
      detail: detail({
        runs: [
          finishedRun({
            runId: 'run-1',
            nodes: [
              {
                instanceNo: 0,
                phase: 'implement',
                attempt: 1,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-old',
                detail: null,
                outputRef: null,
                startedAt: 1_727_000_000_000,
                endedAt: null,
                tokenInput: null,
                tokenOutput: null,
              },
            ],
          }),
        ],
      }),
    });
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));

    // 两轮 executor 节点并存（id 携 run 段、互不顶替）
    expect(screen.getByTestId('rf__node-run:run-1:implement:1:executor:0') !== null).toBe(true);
    fireEvent.click(screen.getByTestId('rf__node-run:run-2:implement:1:executor:0'));

    const panel = await screen.findByTestId('session-transcript-panel');
    await waitFor(() => expect(panel.textContent).toContain('重启轮正文'));
    expect(invokeMock).toHaveBeenCalledWith('session_detail', {
      root: ROOT,
      sessionId: 'ses-new',
    });
    expect(invokeMock).not.toHaveBeenCalledWith('session_detail', {
      root: ROOT,
      sessionId: 'ses-old',
    });
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
        nodes: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-exec',
            detail: null,
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
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

    fireEvent.click(await screen.findByTestId('rf__node-run:run-1727:implement:1:executor:0'));

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

  it('详情恒走图区：status 在场 → 9 站流水线图（末列 archive）+ 抽屉入口；status 缺席 + pipeline 空 → 同走图区（零列图）不崩溃不空白，产物区照常', async () => {
    // 建档：9 列流水线（末列 archive）+ 列头点击开抽屉（单一交互入口可达）
    const filed = renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    filed.unmount();
  });

  it('面板传参收敛：命令面 prop changeId == URL id、展示面 name == detail.name（页内单一身份源 AC-7）', async () => {
    // URL 段 id 与 detail.name 刻意不同：命令面（取数 / 运行 / 转录反查）恒取
    // URL id；展示面（标题 / aria-label / stdout 磁盘面 change name）恒取
    // detail.name
    const urlId = '0199a2f0-2003-7e45-8a9b-000000002003';
    // 节点源取库读史（runs 尾行）：无在飞 run → 主操作仍为发起（run-start）；
    // 步节点经统一视图常驻（收口后活面褪去、库读史常驻）
    renderDetail(
      {
        detail: detail({
          id: urlId,
          name: 'add-feature',
          runs: [
            finishedRun({
              nodes: [
                {
                  phase: 'implement',
                  attempt: 1,
                  step: 'executor',
                  status: 'passed',
                  sessionId: null,
                  detail: null,
                  instanceNo: 0,
                  outputRef: null,
                  startedAt: 1_727_000_000_000,
                  endedAt: null,
                  tokenInput: null,
                  tokenOutput: null,
                },
              ],
            }),
          ],
        }),
        artifacts: PROPOSAL_ENVELOPES,
      },
      ROOT,
      `/changes/${urlId}`,
    );
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(1));

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
        autoNextPhase: true,
      }),
    );

    // 命令面（抽屉）：运行步节点槽位 sessionId 缺席 → 无反查兜底（sourceRef 定式
    // 反查退役），呈「此步无会话记录」步面空态；零 agent_sessions 反查命令
    fireEvent.click(screen.getByTestId('rf__node-run:run-1727:implement:1:executor:0'));
    await screen.findByTestId('detail-drawer');
    await waitFor(() =>
      expect(screen.getByTestId('drawer-session-empty').textContent).toBe('此步无会话记录'),
    );
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 归档链退役（merge-archive-steps-into-flow AC-9）：归档入口按钮 / 归档面板 /
// 归档状态源整体摘除——归档入口由 run 流程的 stop-before-archive 停等面承载，
// 归档列（第 9 列 requirement / 第 6 列 test-only）经详情图区常驻渲染
// ---------------------------------------------------------------------------

describe('ChangeDetailView：归档链退役（入口按钮 / 面板 / 状态源零渲染 + 归档列上图）', () => {
  it('ArchivePanel 组装与入口按钮移除：archive-trigger / archive-confirm-dialog 零渲染（active / archived / status 缺席三态一致）', async () => {
    for (const shape of [
      detail(),
      detail({ status: 'archived', source: 'archive' }),
      detail({ status: null }),
    ]) {
      const view = renderDetail({ detail: shape, artifacts: PROPOSAL_ENVELOPES });
      await screen.findByTestId('detail-header');
      expect(screen.queryByTestId('archive-trigger')).toBeNull();
      expect(screen.queryByTestId('archive-confirm-dialog')).toBeNull();
      view.unmount();
    }
  });

  it('归档状态源摘除：不再调用 useArchiveFlow 链命令（archive_flow_state / archive_flow_start / archive_flow_preflight 零调用）', async () => {
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    await waitFor(() => expect(detailCalls()).toHaveLength(1));

    const commands = invokeMock.mock.calls.map((call) => call[0] as string);
    expect(
      commands.some((command) => command.startsWith('archive_flow_')),
      `归档链命令零调用，实际: ${commands.join(', ')}`,
    ).toBe(false);
  });

  it('归档列入图：第 9 列 archive 列头常驻（列序与 requirement 相位表同序，末位 archive）', async () => {
    const { container } = renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    const columns = await screen.findAllByTestId('flow-column');
    expect(columns).toHaveLength(9);
    const ids = columns.map((column) => column.getAttribute('data-testid') ?? '');
    expect(ids.length).toBe(9);
    // 末列为 archive（列容器 identity 锚：col:archive）；requirement 表零 code-analyze 占位列
    expect(container.querySelector('[data-testid="rf__node-col:archive"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="rf__node-col:code-analyze"]')).toBeNull();
  });

  it('归档列空态（无 run / 无 archive attempt）→ 详情图空列不报错、停等面降级渲染', async () => {
    const { container } = renderDetail({
      detail: detail({
        pipeline: PIPELINE.map((phase) => ({ phase, attempts: [] })),
      }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    // 归档空列仍在图内且零事件节点不崩溃
    expect(container.querySelector('[data-testid="rf__node-col:archive"]')).not.toBeNull();
    expect(screen.queryByTestId('flow-empty')).toBeNull();
    // activeRun.archive === null（无在飞 run）→ 停等面不渲染归档确认卡片
    expect(screen.queryByTestId('archive-confirm-dialog')).toBeNull();
  });

  it('run 停等面承载归档确认：activeRun 停等在 archive 相位 → 归档确认卡片渲染（archive 载荷在场）', async () => {
    const original = invokeMock.getMockImplementation();
    loadActiveRun(null);
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command !== 'change_flow_start' && command !== 'change_flow_confirm') {
        return original!(command, args);
      }
      return Promise.resolve(null);
    });
    loadFixture({
      detail: detail({ activePhase: { phase: 'archive', attempt: 1, startAt: null } }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    loadActiveRun({
      runId: 'run-archive-wait',
      status: 'waitingConfirm',
      phase: 'archive',
      attempt: 1,
      ask: null,
      archive: {
        name: 'add-feature',
        completed: false,
        incompletePhases: ['implement'],
        missingArtifacts: ['design.md'],
        deltaSpecs: ['desktop-flow'],
        worktree: null,
        branch: null,
        mergeTarget: null,
      },
      startedAt: '2026-10-08T00:00:00Z',
      nodes: [],
      edges: [],
    });
    render(
      <MemoryRouter initialEntries={[`/changes/${CHANGE_ID}`]}>{detailTree(ROOT)}</MemoryRouter>,
    );

    const card = await screen.findByTestId('run-archive-confirm-card');
    expect(card.textContent).toContain('implement');
    expect(card.textContent).toContain('design.md');
    expect(card.textContent).toContain('desktop-flow');
    // 退役入口仍零渲染（归档确认只此一条路径）
    expect(screen.queryByTestId('archive-trigger')).toBeNull();
    expect(screen.queryByTestId('archive-confirm-dialog')).toBeNull();
  });

  it('detail 拉取失败 → 详情页降级渲染（错误态）不崩溃，归档链零命令且退役面零渲染', async () => {
    // 挂起门闩下以 reject 落定（既有错误态驱动同式）
    renderDetail({ detail: null, hold: true });
    detailResult = '库读失败';
    await act(async () => {
      releaseDetail?.();
    });

    await waitFor(() => expect(screen.getByTestId('error-note').textContent).toContain('库读失败'));
    expect(screen.queryByTestId('archive-trigger')).toBeNull();
    expect(screen.queryByTestId('archive-confirm-dialog')).toBeNull();
    expect(screen.queryByTestId('run-archive-confirm-card')).toBeNull();
    const commands = invokeMock.mock.calls.map((call) => call[0] as string);
    expect(commands.some((command) => command.startsWith('archive_flow_'))).toBe(false);
  });

  it('既有渲染回归：详情头 / 状态徽章 / 抽屉入口不受归档链退役影响', async () => {
    const { container } = renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('detail-header');
    expect(within(container).getByTestId('detail-header').textContent).toContain('进行中');
    // 抽屉入口仍经列头可达（单一交互入口不变）
    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    // run 控制面板（归档入口的新承载）照常在场
    expect(screen.getByTestId('run-start') !== null).toBe(true);
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
    // 归档入口已并入 run 停等面：退役按钮零渲染（name 仅展名面留存）
    expect(screen.queryByTestId('archive-trigger')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 数据源收窄与 name 透传（change-flow-show-step-only，AC-4 / AC-5 / D6）：
// `mountMaterials(detail, artifacts)` 两参调用（挂载与图无关）、materials memo
// 依赖收窄 `[detail, artifacts]`（去 `graph`）、DetailDrawer 传 `detail.name`
//（stdout 输出记录磁盘面与 `record_exec_output` 同口径按 change name 定位，
// 非 URL id）。mountMaterials 经 `vi.mock(..., { spy: true })` 自动 mock 观测
// 调用次数（ESM 命名空间不支持 `vi.spyOn`；原实现保留、行为零替换）。
// ---------------------------------------------------------------------------

/** proposal.md 文档片段描述符（文档归列按 detail.artifacts[].phase 下标配对）。 */
const PROPOSAL_DESCRIPTOR: ArtifactDescriptor = {
  kind: 'markdown-doc',
  source: 'proposal.md',
  title: '提案',
  phase: 'proposal',
};

/** 清单信封 fixture 的清单归属相位（checklist 挂列头按 payload.phase）。 */
const CHECKLIST_DESCRIPTOR: ArtifactDescriptor = {
  kind: 'eval-checklist',
  source: 'phases/proposal/attempt-1.md',
  title: '评估清单',
  phase: 'proposal',
};

/** session_detail 调用清单（抽屉转录重查的观察锚）。 */
function sessionDetailCalls(): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === 'session_detail');
}

function checklistEnvelope(): ArtifactEnvelope {
  return envelope({
    kind: 'eval-checklist',
    title: '评估清单',
    payload: { phase: 'proposal', attempt: 1, verdict: 'pass', items: [] },
  });
}

/** 运行步节点行 fixture（统一查询活面步表行）。 */
function stepRow(overrides: Partial<ChangeStepState> = {}): ChangeStepState {
  return {
    phase: 'implement',
    attempt: 1,
    step: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    instanceNo: 0,
    outputRef: null,
    startedAt: 1_727_000_000_000,
    endedAt: null,
    tokenInput: null,
    tokenOutput: null,
    ...overrides,
  };
}

describe('ChangeDetailView：素材挂载两参与 memo 依赖收窄（AC-4 / D6）', () => {
  beforeEach(() => {
    vi.mocked(mountMaterials).mockClear();
  });

  it('正向：mountMaterials 两参调用（detail + artifacts，无 graph 位置）→ 文档归列与 eval-checklist 挂列头照常', async () => {
    const spy = vi.mocked(mountMaterials);
    const artifacts = [proposalDoc(), checklistEnvelope()];
    renderDetail({
      detail: detail({ artifacts: [PROPOSAL_DESCRIPTOR, CHECKLIST_DESCRIPTOR] }),
      artifacts: {
        'markdown-doc:proposal.md': proposalDoc(),
        'eval-checklist:phases/proposal/attempt-1.md': checklistEnvelope(),
      },
    });
    await screen.findByTestId('flow-graph');
    await waitFor(() => expect(screen.getByTestId('column-docs').textContent).toBe('2 份文档'));

    // 调用形态：恰两参（graph 位置缺席——挂载层不再接收图）
    expect(spy.mock.calls.length).toBeGreaterThan(0);
    for (const call of spy.mock.calls) {
      expect(call).toHaveLength(2);
      expect(call[0]).toMatchObject({ id: CHANGE_ID });
      expect(call[1]).toEqual(expect.any(Array));
    }
    // 真实组合产出：proposal 列头挂文档 + 清单两条（列头徽章计数为运行时证据）
    const lastCall = spy.mock.calls[spy.mock.calls.length - 1];
    expect(lastCall[1]).toHaveLength(artifacts.length);
    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    await screen.findByTestId('drawer-column-section');
    expect(screen.getByTestId('drawer-column-section').textContent).toContain('提案正文');
  });

  it('正向：materials memo 不随渲染放大——会话事件通知并入统一重查（detail 引用更新一拍）后至多重算一次', async () => {
    const spy = vi.mocked(mountMaterials);
    loadActiveRun(activeRunView({ nodes: [stepRow()] }));
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));

    // 抽屉选中运行 executor 步：转录直查（重渲染的观察锚——通知到站须再查一次）
    fireEvent.click(await screen.findByTestId('rf__node-run:run-1727:implement:1:executor:0'));
    await screen.findByTestId('session-transcript-panel');
    // 会话组件自带一次重放装载（装载拍数不参与本用例断言：只取基线）
    await waitFor(() => expect(sessionDetailCalls().length).toBeGreaterThanOrEqual(1));
    await act(async () => {});
    const detailBaseline = sessionDetailCalls().length;
    const baseline = spy.mock.calls.length;
    expect(baseline).toBeGreaterThan(0);

    // 会话事件通知到站：统一 300ms 去抖 → refreshTick 自增 → ChangeDetailView /
    // DetailLoaded 重渲染（数据源引用全不变）
    await act(async () => {
      lastChannel().push({ ipc: 'sessionEvent' });
    });
    // 重渲染确已发生（转录重查为证：refreshKey 变化驱动重查一次）
    await waitFor(() => expect(sessionDetailCalls().length).toBeGreaterThan(detailBaseline), {
      timeout: 5000,
    });

    // 统一重查单拍：detail 语义引用更新一次 → memo 至多追加一拍（渲染次数不放大）
    const delta = spy.mock.calls.length - baseline;
    expect(delta).toBeGreaterThanOrEqual(0);
    expect(delta).toBeLessThanOrEqual(1);
    expect(spy.mock.calls.every((call) => call.length === 2)).toBe(true);
  });

  it('正向：挂载随数据源提交驱动重算——统一查询刷新（detail / artifacts 引用更新）重算，增量受提交拍数约束', async () => {
    const spy = vi.mocked(mountMaterials);
    loadActiveRun(activeRunView({ nodes: [] }));
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    await waitFor(() => expect(spy.mock.calls.length).toBeGreaterThan(0));
    const baseline = spy.mock.calls.length;

    // 统一查询重查：活面步节点增删（graph 活面变化随查询一并提交流入）
    loadActiveRun(
      activeRunView({
        nodes: [stepRow(), stepRow({ step: 'staticCheck', outputRef: 'exec-1727-1' })],
      }),
    );
    fireEvent.click(screen.getByText('刷新详情'));
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));

    // 一次刷新周期 = detail 提交 + 产物提交两拍（各一次重算）：挂载随数据源提交
    // 而非渲染次数放大（图活面变化不追加第三拍）
    const delta = spy.mock.calls.length - baseline;
    expect(delta).toBeGreaterThan(0);
    expect(delta).toBeLessThanOrEqual(2);
    expect(spy.mock.calls.every((call) => call.length === 2)).toBe(true);
  });

  it('边界：artifacts = [] / runs = []（零图零产物）→ materials 空 columnDocs、抽屉 name 仍透传，不抛错', async () => {
    loadActiveRun(null);
    renderDetail({ detail: detail({ artifacts: [] }), artifacts: {} });
    await screen.findByTestId('flow-graph');

    // 零产物 → 列头无文档徽章、列内零节点
    expect(screen.queryByTestId('column-docs')).toBeNull();
    expect(screen.queryByTestId('run-step-node')).toBeNull();
    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    await screen.findByTestId('drawer-column-section');
    expect(screen.getByTestId('drawer-column-section').textContent).toContain('（无本站文档）');
    expect(screen.queryByTestId('stdout-panel')).toBeNull();
  });
});

describe('ChangeDetailView：stdout 面板按 detail.name 取数（AC-5 / D6）', () => {
  /** URL id ≠ detail.name 的 stdout 用例装置（RUN 面 static-check 步携 outputRef）。 */
  const URL_ID = '0199a2f0-2004-7e45-8a9b-000000002004';

  function renderOutputNodeDetail() {
    loadActiveRun(
      activeRunView({
        nodes: [
          stepRow({
            step: 'staticCheck',
            status: 'passed',
            sessionId: null,
            outputRef: 'exec-1727000000000-9',
          }),
        ],
      }),
    );
    return renderDetail(
      { detail: detail({ id: URL_ID, name: 'add-feature' }), artifacts: PROPOSAL_ENVELOPES },
      ROOT,
      `/changes/${URL_ID}`,
    );
  }

  it('正向：id ≠ name 建档 → 选中 outputRef 节点后抽屉经 readExecOutput(root, changeName=detail.name, execId) 取数', async () => {
    renderOutputNodeDetail();
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(1));

    fireEvent.click(screen.getByTestId('rf__node-run:run-1727:implement:1:staticCheck:0'));
    const panel = await screen.findByTestId('stdout-panel');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('read_exec_output', {
        root: ROOT,
        changeName: 'add-feature',
        execId: 'exec-1727000000000-9',
      }),
    );
    // 磁盘面键恒 detail.name（与 record_exec_output 同口径，非 URL id）
    const readCalls = invokeMock.mock.calls.filter(([name]) => name === 'read_exec_output');
    expect(readCalls).toHaveLength(1);
    const readArgs = readCalls[0][1] as { changeName: string; execId: string };
    expect(readArgs.changeName).toBe('add-feature');
    expect(readArgs.changeName).not.toBe(URL_ID);
    expect(readArgs.execId).toBe('exec-1727000000000-9');
    await waitFor(() => expect(panel.textContent).toContain('（无输出记录）'));
  });

  it('异常：miss（未知 exec_id / 记录缺失）→ stdout 空态呈现，不因 id 寻址漂移而炸图', async () => {
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === 'read_exec_output') return Promise.resolve(null);
      return detailIpc(command, args);
    });
    renderOutputNodeDetail();
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(1));

    fireEvent.click(screen.getByTestId('rf__node-run:run-1727:implement:1:staticCheck:0'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('read_exec_output', {
        root: ROOT,
        changeName: 'add-feature',
        execId: 'exec-1727000000000-9',
      }),
    );
    expect(screen.getByTestId('stdout-empty').textContent).toBe('（无输出记录）');
    expect(screen.queryByTestId('stdout-error')).toBeNull();
    // 抽屉其余面照常（错误态不阻断）
    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 流程图层列序随 detail.pipeline 出线（desktop-test-only-flow AC-6 / AC-5）：
// requirement 9 列 / test-only 6 列；code-analyze 节点列位由 phaseOrder 派生
// ---------------------------------------------------------------------------

describe('ChangeDetailView：列序随 detail.pipeline（requirement / test-only 两表）', () => {
  it('正向：requirement detail → 流程图 9 列（末列 archive、零 code-analyze 占位列）', async () => {
    const { container } = renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });

    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    const ids = screen
      .getAllByTestId('flow-column')
      .map((column) => column.getAttribute('data-testid') ?? '');
    expect(ids).toHaveLength(9);
    expect(container.querySelector('[data-testid="rf__node-col:archive"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="rf__node-col:code-analyze"]')).toBeNull();
  });

  it('正向：test-only detail → 流程图 6 列，code-analyze 列头在场、archive 第 6 列', async () => {
    const { container } = renderDetail({
      detail: detail({
        pipeline: TEST_ONLY_PIPELINE.map((phase) => ({
          phase,
          attempts: phase === 'proposal' ? [attempt({ verdict: 'pass' })] : [],
        })),
      }),
    });

    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(6));
    expect(container.querySelector('[data-testid="rf__node-col:code-analyze"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="rf__node-col:archive"]')).not.toBeNull();
    // requirement 独有相位零列（不臆造占位列）
    for (const phase of ['dev-design', 'implement', 'code-review', 'acceptance']) {
      expect(container.querySelector(`[data-testid="rf__node-col:${phase}"]`)).toBeNull();
    }
  });

  it('正向：test-only 的 code-analyze 运行步节点上图（列位 = pipeline 序第 2 列）', async () => {
    const { container } = renderDetail({
      detail: detail({
        pipeline: TEST_ONLY_PIPELINE.map((phase) => ({ phase, attempts: [] })),
        runs: [
          finishedRun({
            nodes: [
              {
                instanceNo: 0,
                phase: 'code-analyze',
                attempt: 1,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-code-analyze',
                detail: null,
                outputRef: null,
                startedAt: 1_727_000_000_000,
                endedAt: null,
                tokenInput: null,
                tokenOutput: null,
              },
            ],
          }),
        ],
      }),
    });

    await screen.findByTestId('flow-graph');
    await waitFor(() =>
      expect(
        container.querySelector('[data-testid="rf__node-run:run-1727:code-analyze:1:executor:0"]'),
      ).not.toBeNull(),
    );
  });

  it('异常：test-only 列序外相位（implement）运行步 → 不入图（phaseOrder.indexOf < 0 跳过）', async () => {
    const { container } = renderDetail({
      detail: detail({
        pipeline: TEST_ONLY_PIPELINE.map((phase) => ({ phase, attempts: [] })),
        runs: [
          finishedRun({
            nodes: [
              {
                instanceNo: 0,
                phase: 'implement',
                attempt: 1,
                step: 'executor',
                status: 'passed',
                sessionId: 'ses-out-of-table',
                detail: null,
                outputRef: null,
                startedAt: 1_727_000_000_000,
                endedAt: null,
                tokenInput: null,
                tokenOutput: null,
              },
            ],
          }),
        ],
      }),
    });

    await screen.findByTestId('flow-graph');
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(6));
    expect(
      container.querySelector('[data-testid="rf__node-run:run-1727:implement:1:executor:0"]'),
    ).toBeNull();
  });

  it('边界：空 pipeline → 零列图渲染不崩（flow-graph 常驻、flow-empty 不出现）', async () => {
    const { container } = renderDetail({ detail: detail({ pipeline: [], runs: [] }) });

    await screen.findByTestId('flow-graph');
    expect(within(container).queryAllByTestId('flow-column')).toHaveLength(0);
    expect(within(container).queryByTestId('flow-empty')).toBeNull();
  });

  it('边界：两表列数各自稳定（requirement 9 / test-only 6，列序与出线序逐位一致）', async () => {
    const requirement = renderDetail({ detail: detail() });
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    requirement.unmount();

    const testOnly = renderDetail({
      detail: detail({ pipeline: TEST_ONLY_PIPELINE.map((phase) => ({ phase, attempts: [] })) }),
    });
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(6));
    testOnly.unmount();
  });
});

// ---------------------------------------------------------------------------
// configSnapshot fixture 补 agentInstanceId 键（fixture 随动，AC-7）
// ---------------------------------------------------------------------------

describe('ChangeDetailView：configSnapshot fixture 携 agentInstanceId 键（AC-7 随动）', () => {
  it('fixture 补键后页面组装既有断言保绿（null 存量与具体 id 两形态同口径）', async () => {
    // helper 默认形态：agentInstanceId null（存量缺列语义）
    const legacy = transcriptSession('ses-legacy', `${CHANGE_ID}/implement/executor/1`);
    expect((legacy.row.configSnapshot as ConfigSnapshotFixture).agentInstanceId).toBeNull();

    // 具体实例 id 形态（新会话语义）
    const fresh = transcriptSession('ses-fresh', `${CHANGE_ID}/implement/executor/1`);
    fresh.row.configSnapshot = {
      engine: 'sdk',
      agentInstanceId: 7,
      model: 'glm-high',
      permissionMode: 'bypassPermissions',
    } satisfies ConfigSnapshotFixture;
    expect((fresh.row.configSnapshot as ConfigSnapshotFixture).agentInstanceId).toBe(7);

    // 既有组装断言保绿（详情 / 图区 / 控制面板恒态不受 fixture 形状随动影响）
    renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
  });
});

// ---------------------------------------------------------------------------
// 刷新失败降级与订阅粘性
// ---------------------------------------------------------------------------

/** get_change_detail 轮剧本（逐轮消费；'ok' 读当前 fixture——loadFixture 可
 * 中途改写、'reject' 以错误串拒绝、'null' 真未找到）。 */
interface DetailPlanStep {
  kind: 'ok' | 'reject' | 'null';
  message?: string;
}

let detailPlan: DetailPlanStep[] = [{ kind: 'ok' }];
let detailPlanCalls = 0;

function planDetailRounds(plan: DetailPlanStep[]): void {
  detailPlan = [...plan];
  detailPlanCalls = 0;
}

describe('ChangeDetailView：刷新失败降级与订阅粘性（AC-1 / AC-2 / AC-6 / AC-7 / AC-8 / AC-9）', () => {
  /** 推进虚拟时间并冲刷微任务链（fake timers 下的轮拍推进单点）。 */
  async function flush(ms: number): Promise<void> {
    act(() => {
      vi.advanceTimersByTime(ms);
    });
    await act(async () => {});
  }

  /** 通知触发的统一视图重查（kind-only step 通知 → 300ms 去抖 → 第 2 轮）。 */
  async function notifyAndFlush(): Promise<void> {
    await act(async () => {
      lastChannel().push({ ipc: 'step' });
    });
    await flush(300);
  }

  beforeEach(() => {
    vi.useFakeTimers();
    planDetailRounds([{ kind: 'ok' }]);
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === 'get_change_detail') {
        const step = detailPlan[Math.min(detailPlanCalls, detailPlan.length - 1)];
        detailPlanCalls += 1;
        if (step.kind === 'reject') return Promise.reject(step.message ?? '注入失败');
        if (step.kind === 'null') return Promise.resolve(null);
        return detailIpc(command, args);
      }
      return detailIpc(command, args);
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('内联横幅呈现（AC-1 / AC-9）：detail 在手的刷新失败轮 → refresh-stale-note 横幅（文案含后端错误串原样与「刷新详情」引导），header / 停止入口 / 图面照常渲染 last-good', async () => {
    loadActiveRun(activeRunView());
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 注入失败：读事务不可用' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    expect(screen.queryByTestId('refresh-stale-note')).toBeNull();

    await notifyAndFlush();

    // 横幅呈现：data-testid 挂钩 + 文案含错误串原样与重试引导（D1）
    const note = screen.getByTestId('refresh-stale-note');
    expect(note.textContent).toContain('刷新失败');
    expect(note.textContent).toContain('db: 注入失败：读事务不可用');
    expect(note.textContent).toContain('刷新详情');

    // 保 last-good：header / 停止入口 / 状态徽章 / 图面照常（MUST NOT 兜底页）
    expect(screen.getByTestId('detail-header') !== null).toBe(true);
    expect(screen.getByTestId('run-stop') !== null).toBe(true);
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    expect(screen.queryByTestId('error-note')).toBeNull();
    expect(screen.queryByText('未找到该 change。')).toBeNull();
  });

  it('内联横幅消失（AC-2 视图半边）：失败（横幅在）→ 1s 重试成功 → 横幅出销、图面节点更新到最新轮 detail；横幅收进右栏（header 整宽 → 右栏「横幅 → 动作面板 → 图区」D16）', async () => {
    loadActiveRun(activeRunView());
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 注入失败' }, { kind: 'ok' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    expect(screen.queryByTestId('run-step-node')).toBeNull();

    // DOM 序：header 整宽留在分栏之上；右栏内部序为「横幅 → 动作面板 → 图区」
    await notifyAndFlush();
    const container = screen.getByTestId('refresh-stale-note').parentElement!;
    const note = container.querySelector('[data-testid="refresh-stale-note"]');
    const panel = container.querySelector('[data-testid="run-control-panel"]');
    const column = document.querySelector('[data-slot="resizable-panel-group"]');
    const header = screen.getByTestId('detail-header');
    expect(
      column !== null &&
        (header.compareDocumentPosition(column) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
      'header 整宽在分栏之前（左栏不位移）',
    ).toBe(true);
    expect(
      container.querySelector('[data-testid="detail-header"]'),
      '横幅收进右栏（不在页首整宽层）',
    ).toBeNull();
    expect(
      note !== null &&
        panel !== null &&
        (note.compareDocumentPosition(panel) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
      '右栏内横幅在动作面板之上',
    ).toBe(true);

    // 1s 重试成功（应答携活面步表）→ 横幅出销 + 图面更新到最新轮
    loadActiveRun(
      activeRunView({
        nodes: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-exec',
            detail: null,
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
          },
        ],
      }),
    );
    await flush(1000);

    expect(screen.queryByTestId('refresh-stale-note')).toBeNull();
    expect(screen.getAllByTestId('run-step-node')).toHaveLength(1);
    expect(screen.getByTestId('rf__node-run:run-1727:implement:1:executor:0') !== null).toBe(true);
  });

  it('error 收敛为首载语义（AC-6 异常半边）：detail 在手的刷新失败轮不渲染整页 DetailFallback（error-note / 「未找到该 change。」零出现、横幅在案）；对照首载失败整页错误兜底页（error-note 在案、横幅零出现）', async () => {
    // 半边一：detail 在手的刷新失败轮 → 内联横幅（不再整页 DetailFallback）
    loadActiveRun(activeRunView());
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 读失败' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    await notifyAndFlush();
    expect(screen.getByTestId('refresh-stale-note') !== null).toBe(true);
    expect(screen.queryByTestId('error-note')).toBeNull();
    expect(screen.queryByText('未找到该 change。')).toBeNull();
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);

    // 半边二：首载失败（detail 不在手）→ 整页错误兜底页（既有语义不变）
    planDetailRounds([{ kind: 'reject', message: 'IPC 断开' }]);
    const firstLoad = renderDetail({ detail: null });
    await act(async () => {});
    const note = within(firstLoad.container).getByTestId('error-note');
    expect(note.textContent).toContain('详情加载失败：IPC 断开');
    expect(within(firstLoad.container).queryByTestId('refresh-stale-note')).toBeNull();
    expect(within(firstLoad.container).queryByTestId('flow-graph')).toBeNull();
  });

  it('真 None 清面兜底（AC-6 边界半边）：失败轮后重试成功返回 null → 「未找到该 change。」兜底页（detail-note）、横幅同步消失（None 与 Err 分流不混同）', async () => {
    loadActiveRun(activeRunView());
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 读失败' }, { kind: 'null' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    await notifyAndFlush();
    expect(screen.getByTestId('refresh-stale-note') !== null).toBe(true);

    await flush(1000);

    expect(screen.queryByTestId('refresh-stale-note')).toBeNull();
    expect(screen.getByTestId('detail-note').textContent).toBe('未找到该 change。');
    expect(screen.queryByTestId('flow-graph')).toBeNull();
  });

  it('订阅粘性（AC-7）：失败轮期间 change_flow_watch 零重订（Channel 实例数不增）；经既有 Channel 直投下一条通知 → 统一视图重查照常触发（零丢通知）', async () => {
    loadActiveRun(activeRunView());
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 读失败' }, { kind: 'ok' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    expect(ChannelMock.instances).toHaveLength(1);

    // 失败轮：detail / activeRun 保持 → activeRunPresent 真值 → 订阅不被拆
    await notifyAndFlush();
    expect(screen.getByTestId('refresh-stale-note') !== null).toBe(true);
    expect(ChannelMock.instances).toHaveLength(1);
    const callsAfterFailedRound = detailCalls().length;
    expect(callsAfterFailedRound).toBe(2);

    // 下一条通知经既有 Channel 直投 → 重查照常触发（零丢通知、零重订）
    loadFixture({ detail: detail({ title: '最新轮标题' }), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {
      lastChannel().push({ ipc: 'edge' });
    });
    await flush(300);

    expect(detailCalls()).toHaveLength(callsAfterFailedRound + 1);
    expect(ChannelMock.instances).toHaveLength(1);
    expect(screen.getByTestId('detail-header').textContent).toContain('最新轮标题');
    expect(screen.queryByTestId('refresh-stale-note')).toBeNull();
  });

  it('图面不塌（AC-1 / D3）：失败轮期间节点集合保持上一成功轮（零 nodes=0 空图）、运行徽章在案；重试轮挂起时 loading 仅作用于「刷新详情」禁用面（无整页加载占位）', async () => {
    loadActiveRun(
      activeRunView({
        nodes: [
          {
            phase: 'implement',
            attempt: 1,
            step: 'executor',
            status: 'running',
            sessionId: 'ses-exec',
            detail: null,
            instanceNo: 0,
            outputRef: null,
            startedAt: 1_727_000_000_000,
            endedAt: null,
            tokenInput: null,
            tokenOutput: null,
          },
        ],
      }),
    );
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 读失败' }, { kind: 'ok' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    expect(screen.getAllByTestId('run-step-node')).toHaveLength(1);

    await notifyAndFlush();

    // 失败轮：图面节点不清零、徽章在案、零空图占位
    expect(screen.getAllByTestId('run-step-node')).toHaveLength(1);
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');
    expect(screen.queryByText('加载中…')).toBeNull();

    // 重试轮挂起（门闩）：loading 置位仅作用刷新按钮禁用面
    loadFixture({ detail: detail(), artifacts: PROPOSAL_ENVELOPES, hold: true });
    await flush(1000);
    expect(screen.queryByText('加载中…')).toBeNull();
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    expect(screen.getAllByTestId('run-step-node')).toHaveLength(1);
    expect(screen.getByTestId('refresh-change-detail-button').hasAttribute('disabled')).toBe(false);

    // 门闩释放（成功轮）→ 横幅出销
    await act(async () => {
      releaseDetail?.();
    });
    expect(screen.queryByTestId('refresh-stale-note')).toBeNull();
  });

  it('降级窗口动作可用（AC-8）：保 last-good 窗口内停止入口照常可发（change_flow_stop 独立 invoke 不受降级态影响），控制入口状态与 last-good activeRun 一致', async () => {
    loadActiveRun(activeRunView());
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 读失败' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    await notifyAndFlush();
    expect(screen.getByTestId('refresh-stale-note') !== null).toBe(true);
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');

    fireEvent.click(screen.getByTestId('run-stop'));

    expect(invokeMock).toHaveBeenCalledWith('change_flow_stop', {
      root: ROOT,
      id: CHANGE_ID,
    });
    expect(screen.getByTestId('refresh-stale-note') !== null).toBe(true);
  });

  it('降级窗口动作可用（AC-8）：phase 间确认卡片照常呈现与提交（change_flow_confirm 独立 invoke，proceed=true）', async () => {
    loadActiveRun(activeRunView({ status: 'waitingConfirm', phase: 'test-design', attempt: 1 }));
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 读失败' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    await notifyAndFlush();
    expect(screen.getByTestId('refresh-stale-note') !== null).toBe(true);

    // 控制入口状态与 last-good activeRun 一致：确认卡片携停等相位名
    const card = screen.getByTestId('run-confirm-card');
    expect(card.textContent).toContain('test-design');

    fireEvent.click(screen.getByTestId('run-confirm-proceed'));

    expect(invokeMock).toHaveBeenCalledWith('change_flow_confirm', {
      root: ROOT,
      id: CHANGE_ID,
      proceed: true,
      syncSpecs: true,
    });
  });

  it('降级窗口动作可用（AC-8）：ask 应答通道照常可发（change_flow_answer 独立 invoke，自由文本）', async () => {
    loadActiveRun(
      activeRunView({
        status: 'waitingAsk',
        ask: { question: '继续还是停止？', options: [] },
      }),
    );
    planDetailRounds([{ kind: 'ok' }, { kind: 'reject', message: 'db: 读失败' }]);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await act(async () => {});
    await notifyAndFlush();
    expect(screen.getByTestId('refresh-stale-note') !== null).toBe(true);
    expect(screen.getByTestId('run-ask-question').textContent).toBe('继续还是停止？');

    fireEvent.change(screen.getByTestId('run-ask-input'), { target: { value: '继续' } });
    fireEvent.click(screen.getByTestId('run-ask-submit'));

    expect(invokeMock).toHaveBeenCalledWith('change_flow_answer', {
      root: ROOT,
      id: CHANGE_ID,
      answer: '继续',
    });
  });
});
// ---------------------------------------------------------------------------
// 通知分流收敛（AC-5 / D13）：SessionEvent 与其余通知同走统一 300ms 重查 tick
// ---------------------------------------------------------------------------

describe('ChangeDetailView：通知分流收敛（统一 300ms tick）', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) =>
      detailIpc(command, args),
    );
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /** get_change_detail 调用次数（统一重查 tick 的观测锚）。 */
  function detailCalls(): number {
    return invokeMock.mock.calls.filter(([name]) => name === 'get_change_detail').length;
  }

  /** 推进虚拟时间并冲刷微任务链（fake timers 下的拍推进单点）。 */
  async function flush(ms: number): Promise<void> {
    act(() => {
      vi.advanceTimersByTime(ms);
    });
    await act(async () => {});
  }

  it('SessionEvent 通知与其余通知同走 300ms 统一重查：去抖窗内混合序列合并为恰一拍', async () => {
    loadActiveRun(activeRunView({ nodes: [stepRow()] }));
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await flush(0);
    await flush(300);
    expect(screen.getByTestId('flow-graph')).toBeTruthy();
    const baseline = detailCalls();
    expect(baseline).toBeGreaterThan(0);

    // 混合通知序列（step + sessionEvent + finished）在同一去抖窗内到站
    await act(async () => {
      lastChannel().push({ ipc: 'step' });
      lastChannel().push({ ipc: 'sessionEvent' });
      lastChannel().push({ ipc: 'finished' });
    });
    await flush(299);
    expect(detailCalls(), '去抖窗内静默').toBe(baseline);

    await flush(1);
    expect(detailCalls(), '窗内合并为恰一拍重查').toBe(baseline + 1);
  });

  it('抽屉面板零动作面：sessionRefreshKey 变更仅触发重取，面板内无发送输入 / 停止按钮', async () => {
    loadActiveRun(activeRunView({ nodes: [stepRow()] }));
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await flush(0);
    await flush(300);
    expect(screen.getByTestId('flow-graph')).toBeTruthy();

    fireEvent.click(screen.getByTestId('rf__node-run:run-1727:implement:1:executor:0'));
    await flush(0);
    const panel = screen.getByTestId('session-transcript-panel');
    await flush(0);
    expect(sessionDetailCalls().length).toBeGreaterThanOrEqual(1);
    const sessionBaseline = sessionDetailCalls().length;

    // 只读消费点：面板内零动作面
    expect(within(panel).queryByTestId('explore-send')).toBeNull();
    expect(within(panel).queryByTestId('explore-stop')).toBeNull();
    expect(within(panel).queryByTestId('explore-prompt')).toBeNull();
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_stop')).toHaveLength(0);

    // sessionRefreshKey 变更（SessionEvent 通知 → 统一 tick）→ 面板重取一拍
    await act(async () => {
      lastChannel().push({ ipc: 'sessionEvent' });
    });
    await flush(300);
    expect(sessionDetailCalls().length, '统一 tick 驱动面板重取').toBeGreaterThan(sessionBaseline);
  });
});

// ---------------------------------------------------------------------------
// 左栏通用 Agent 面板分栏（desktop-change-chat，AC-1 / AC-4 / AC-5 / AC-6 /
// AC-7）：`DetailLoaded` 纵向单列 → `ResizablePanelGroup orientation="horizontal"`
// （左面板 collapsible + 窄条、右栏「陈旧横幅 → 动作面板 → 图区」），header 整宽
// 留在分栏之上；折叠真相取分栏命令面（`onResize` 回传），header 开关与窄条展开
// 共用同一 `toggle()`。组合用例入口 = 本视图（链路最上层）。
// ---------------------------------------------------------------------------

/** 面板历史清单项 fixture（`list_change_chat_sessions` 应答项）。 */
function chatSummary(sessionId: string): SessionSummary {
  return {
    row: {
      id: sessionId,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'change', sourceRef: CHANGE_ID },
      createdAt: 1_727_000_000_000,
      updatedAt: 1_727_000_001_000,
    },
    stats: { turnCount: 1, totalDurationMs: null, inputTokens: null, outputTokens: null },
    turns: [
      {
        turnId: 1,
        sessionId,
        status: 'completed',
        startedAt: 1_727_000_000_000,
        finishedAt: 1_727_000_001_000,
        numTurns: 1,
        costUsd: null,
        durationMs: 1_000,
        error: null,
      },
    ],
  };
}

function sessionCommandCalls(name: string): unknown[][] {
  return invokeMock.mock.calls.filter(([command]) => command === name);
}

/**
 * 面板像素宽报告（jsdom 无布局引擎）：把 chat 面板子树的 `offsetWidth` 报告为
 * 指定值并重放 `ResizeObserver`，驱动分栏 `onResize` 回传折叠真相（`inPixels`
 * <= 48px 即收起）。折叠断言仍以命令面结果为准（开关文案 / 窄条 testid）。
 */
function reportChatPanelWidth(px: number): void {
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
    configurable: true,
    get(this: HTMLElement) {
      const isChatPanel =
        this.getAttribute('data-testid') === 'change-chat-panel' ||
        this.querySelector('[data-testid="change-chat-panel"]') !== null;
      return isChatPanel ? px : 100;
    },
  });
  act(() => {
    ResizeObserverStub.flush();
  });
}

function panelPrompt(): HTMLTextAreaElement {
  return screen.getByTestId('change-chat-prompt') as HTMLTextAreaElement;
}

function panelSend(): HTMLButtonElement {
  return screen.getByTestId('change-chat-send') as HTMLButtonElement;
}

async function sendFromPanel(text: string): Promise<void> {
  fireEvent.change(panelPrompt(), { target: { value: text } });
  fireEvent.click(panelSend());
  await act(async () => {});
  await act(async () => {});
}

describe('ChangeDetailView：面板分栏骨架与折叠开关', () => {
  beforeEach(() => {
    // 清单应答按用例注入（缺省空页）：乱序执行时不得跨用例泄漏
    panelListResult = null;
  });

  it('分栏骨架：横向分栏 + 左栏面板 / 右栏图区，header 整宽留在分栏之前', async () => {
    renderDetail({ detail: detail({ worktree: 'C:\\demo\\wt' }), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');

    const group = document.querySelector('[data-slot="resizable-panel-group"]')! as HTMLElement;
    expect(group, '分栏宿主在场').not.toBeNull();
    expect(group.style.flexDirection, 'orientation="horizontal"（横向分栏）').toBe('row');

    const chatPanel = screen.getByTestId('change-chat-panel');
    expect(chatPanel.closest('[data-slot="resizable-panel"]'), '面板在分栏面板内').not.toBeNull();
    expect(
      screen.getByTestId('flow-graph').closest('[data-slot="resizable-panel"]'),
      '图区在另一侧分栏面板内',
    ).not.toBeNull();
    expect(chatPanel.closest('[data-slot="resizable-panel"]'), '左右两栏为不同面板').not.toBe(
      screen.getByTestId('flow-graph').closest('[data-slot="resizable-panel"]'),
    );

    const header = screen.getByTestId('detail-header');
    expect(
      (header.compareDocumentPosition(group) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
      'header 位于分栏之前（整宽 D16）',
    ).toBe(true);
  });

  it('初拍默认折叠：挂载即窄条形态（布局份额取收起尺寸、开关文案收起、主体不渲染）', async () => {
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');

    // jsdom 无布局：两面板各报 100px（组宽 200px），收起尺寸 3rem = 48px → 份额 24%
    const chatPanelElement = screen
      .getByTestId('change-chat-panel')
      .closest<HTMLElement>('[data-panel]')!;
    expect(Number(chatPanelElement.style.flexGrow), '挂载即收起的布局份额').toBeCloseTo(24, 5);

    reportChatPanelWidth(30);
    expect(screen.getByTestId('detail-chat-toggle').textContent).toBe('显示通用 Agent');
    expect(screen.getByTestId('change-chat-rail') !== null).toBe(true);
    expect(screen.getByTestId('change-chat-expand') !== null).toBe(true);
    expect(screen.queryByTestId('change-chat-prompt'), '收起后主体不在场').toBeNull();
  });

  it('面板显隐开关：窄条展开入口 / header 开关共用 toggle（默认折叠 → 展开 → 再收起）', async () => {
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(30);

    // 默认收起 → 窄条展开入口
    fireEvent.click(screen.getByTestId('change-chat-expand'));
    reportChatPanelWidth(100);

    await waitFor(() =>
      expect(screen.getByTestId('detail-chat-toggle').textContent).toBe('隐藏通用 Agent'),
    );
    expect(screen.getByTestId('change-chat-prompt') !== null).toBe(true);
    expect(screen.queryByTestId('change-chat-expand')).toBeNull();

    // header 开关再收起
    fireEvent.click(screen.getByTestId('detail-chat-toggle'));
    reportChatPanelWidth(30);

    await waitFor(() =>
      expect(screen.getByTestId('detail-chat-toggle').textContent).toBe('显示通用 Agent'),
    );
    expect(screen.queryByTestId('change-chat-prompt'), '收起后主体不在场').toBeNull();
    expect(screen.getByTestId('change-chat-expand') !== null).toBe(true);
  });

  it('change id 变化折叠复位：展开后切到另一 change → 面板回到默认收起', async () => {
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(30);
    fireEvent.click(screen.getByTestId('change-chat-expand'));
    reportChatPanelWidth(100);
    await waitFor(() =>
      expect(screen.getByTestId('detail-chat-toggle').textContent).toBe('隐藏通用 Agent'),
    );

    // 返回列表 → 再入另一 change（URL id 切换）
    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    await waitFor(() => expect(probePathname()).toBe('/changes'));
    loadFixture({ detail: detail({ id: SECOND_CHANGE_ID }) });
    fireEvent.click(screen.getByTestId('nav-stub'));
    await waitFor(() => expect(probePathname()).toBe(`/changes/${SECOND_CHANGE_ID}`));
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(30);

    await waitFor(() =>
      expect(screen.getByTestId('detail-chat-toggle').textContent).toBe('显示通用 Agent'),
    );
    expect(screen.queryByTestId('change-chat-prompt'), '收起后主体不在场').toBeNull();
    expect(screen.getByTestId('change-chat-expand') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 面板传参与判据组合面（AC-1 / AC-4 / AC-5 / AC-6）：视图 → 面板 → hook →
// 会话基建 → transport → agent_start 全链路（内部零 mock）。
// ---------------------------------------------------------------------------

describe('ChangeDetailView：面板传参与判据组合面', () => {
  beforeEach(() => {
    // 清单应答按用例注入（缺省空页）：乱序执行时不得跨用例泄漏
    panelListResult = null;
  });

  it('组合：worktree 作执行根、URL id 作 sourceRef，首发即 New（sessionId null）', async () => {
    renderDetail({ detail: detail({ worktree: 'C:\\demo\\wt' }), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(100);

    await sendFromPanel('就此 change 提问');

    expect(sessionCommandCalls('agent_start')).toHaveLength(1);
    expect(sessionCommandCalls('agent_start')[0][1]).toMatchObject({
      cwd: 'C:\\demo\\wt',
      source: 'change',
      sourceRef: CHANGE_ID,
      sessionId: null,
      prompt: '就此 change 提问',
    });
  });

  it('组合：打开即零会话取数（detail 落位后 session_detail / 转录 / 清单均零 invoke）', async () => {
    renderDetail({ detail: detail({ worktree: 'C:\\demo\\wt' }), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    await act(async () => {});

    expect(sessionCommandCalls('session_detail')).toHaveLength(0);
    expect(sessionCommandCalls('agent_session_transcript')).toHaveLength(0);
    expect(sessionCommandCalls('list_change_chat_sessions')).toHaveLength(0);
    expect(screen.getByTestId('change-chat-panel') !== null).toBe(true);
  });

  it('组合：无 run 的 active 正常收发（发送恰一次且 sessionId 为 null）', async () => {
    loadActiveRun(null);
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(100);
    expect(screen.queryByTestId('change-chat-disabled-reason')).toBeNull();

    await sendFromPanel('空闲态提问');

    expect(sessionCommandCalls('agent_start')).toHaveLength(1);
    expect(sessionCommandCalls('agent_start')[0][1]).toMatchObject({ sessionId: null });
  });

  it('组合：run 运行中禁发（理由「流程运行中」、发送零 agent_start、历史仍可用）', async () => {
    loadActiveRun(activeRunView());
    panelListResult = () => ({ sessions: [chatSummary('ses-panel-1')], total: 1 });
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(100);

    expect(screen.getByTestId('change-chat-disabled-reason').textContent).toBe('流程运行中');
    expect(panelPrompt().disabled).toBe(true);
    fireEvent.change(panelPrompt(), { target: { value: '不该发出' } });
    fireEvent.click(panelSend());
    await act(async () => {});
    expect(sessionCommandCalls('agent_start')).toHaveLength(0);

    fireEvent.click(screen.getByTestId('change-chat-history'));
    await act(async () => {});
    await act(async () => {});
    expect(sessionCommandCalls('list_change_chat_sessions'), '历史仍可用').toHaveLength(1);
    expect(screen.getAllByTestId('change-chat-history-item')).toHaveLength(1);
  });

  it('组合：归档 change 只读（悬空 worktree 形态）→ 理由行 + 全禁发 + 历史仍可切', async () => {
    panelListResult = () => ({ sessions: [chatSummary('ses-panel-1')], total: 1 });
    renderDetail({
      detail: detail({ source: 'archive', worktree: 'C:\\gone\\wt' }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(100);

    expect(screen.getByTestId('change-chat-disabled-reason').textContent).toBe(
      '已归档 change 只读',
    );
    expect(panelPrompt().disabled).toBe(true);
    expect(screen.getByTestId<HTMLInputElement>('change-chat-new').disabled).toBe(true);

    fireEvent.click(panelSend());
    await act(async () => {});
    expect(sessionCommandCalls('agent_start')).toHaveLength(0);

    fireEvent.click(screen.getByTestId('change-chat-history'));
    await act(async () => {});
    await act(async () => {});
    expect(sessionCommandCalls('list_change_chat_sessions')).toHaveLength(1);
  });

  it('组合：legacy worktree = null → 面板 cwd 传 null（后端回落 root），图区与动作面板照常', async () => {
    renderDetail({ detail: detail({ worktree: null }), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(100);
    expect(screen.getByTestId('run-control-panel') !== null).toBe(true);

    await sendFromPanel('legacy 记录提问');

    const args = sessionCommandCalls('agent_start')[0][1] as Record<string, unknown>;
    expect(args['cwd']).toBeNull();
    expect(args['sessionId']).toBeNull();
  });

  it('组合：无重放风暴——无会话时会话取数恒 0；已选会话时恒 1，均不随 refreshTick 增长', async () => {
    loadActiveRun(activeRunView());
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    reportChatPanelWidth(100);
    await waitFor(() => expect(ChannelMock.instances.length).toBeGreaterThan(0));
    const detailsBefore = detailCalls().length;

    // 无会话：N 条密封事件驱动 refreshTick 递增，会话取数恒 0
    await act(async () => {
      for (let round = 0; round < 12; round += 1) lastChannel().push({ ipc: 'sessionEvent' });
    });
    await waitFor(() => expect(detailCalls().length).toBeGreaterThan(detailsBefore), {
      timeout: 5000,
    });
    expect(sessionCommandCalls('session_detail'), '无会话零取数').toHaveLength(0);
    expect(sessionCommandCalls('agent_session_transcript')).toHaveLength(0);

    // 已选会话：会话取数恒 1，通知到站不再追加
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === 'list_change_chat_sessions') {
        return Promise.resolve({ sessions: [chatSummary('ses-panel-1')], total: 1 });
      }
      if (command === 'session_detail') return Promise.resolve(chatSummary('ses-panel-1'));
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      return detailIpc(command, args);
    });
    fireEvent.click(screen.getByTestId('change-chat-history'));
    await act(async () => {});
    await act(async () => {});
    fireEvent.click(screen.getAllByTestId('change-chat-history-item')[0]);
    await act(async () => {});
    await act(async () => {});
    expect(sessionCommandCalls('session_detail')).toHaveLength(1);
    expect(sessionCommandCalls('agent_session_transcript')).toHaveLength(1);

    const detailsSecond = detailCalls().length;
    await act(async () => {
      for (let round = 0; round < 12; round += 1) lastChannel().push({ ipc: 'sessionEvent' });
    });
    await waitFor(() => expect(detailCalls().length).toBeGreaterThan(detailsSecond), {
      timeout: 5000,
    });
    expect(sessionCommandCalls('session_detail'), '不随 refreshTick 增长').toHaveLength(1);
    expect(sessionCommandCalls('agent_session_transcript')).toHaveLength(1);
  });
});

describe('ChangeDetailView：抽屉非模态回归与首载降级', () => {
  it('抽屉挂载 / 卸载回归：列头点击 → detail-drawer 非模态（data-modal="false" + 无遮罩），关闭即卸载', async () => {
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');

    fireEvent.click(screen.getAllByTestId('flow-column')[0]);
    await screen.findByTestId('detail-drawer');

    const viewport = document.querySelector('[data-slot="drawer-viewport"]');
    expect(viewport?.getAttribute('data-modal'), '非模态（AC-7）').toBe('false');
    expect(document.querySelector('[data-slot="drawer-overlay"]'), '无遮罩节点').toBeNull();

    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    await waitFor(() => expect(screen.queryByTestId('detail-drawer')).toBeNull());
  });

  it('首载失败降级：get_change_detail reject → 降级页、面板零挂载、零 agent 类 invoke', async () => {
    renderDetail({ detail: 'db: 读详情失败' });
    await waitFor(() => expect(screen.getByTestId('error-note') !== null).toBe(true));

    expect(screen.getByTestId('error-note').textContent).toContain('db: 读详情失败');
    expect(screen.queryByTestId('change-chat-panel'), '面板不抢跑').toBeNull();
    expect(
      invokeMock.mock.calls.filter(([command]) => String(command).startsWith('agent_')),
      '零 agent 类 invoke',
    ).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// archi 章节：右栏「架构」页签（挂载点 B，增量可改档）
// ---------------------------------------------------------------------------

describe('ChangeDetailView：架构页签（挂载点 B）', () => {
  it('应呈现 flow 与 architecture 两页签，缺省 flow（架构画布不在场）', async () => {
    renderDetail({ detail: detail({ worktree: 'C:demowt' }) });
    await waitFor(() => expect(screen.getByTestId('detail-tab-flow') !== null).toBe(true));
    expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true);
    expect(screen.queryByTestId('archi-canvas')).toBeNull();
  });

  it('应在切换页签时挂载架构画布并卸载流程图，切回后复原', async () => {
    renderDetail({ detail: detail({ worktree: 'C:demowt' }) });
    await waitFor(() => expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true));

    fireEvent.click(screen.getByTestId('detail-tab-architecture'));
    await waitFor(() => expect(screen.getByTestId('archi-canvas') !== null).toBe(true));

    fireEvent.click(screen.getByTestId('detail-tab-flow'));
    await waitFor(() => expect(screen.queryByTestId('archi-canvas')).toBeNull());
    expect(screen.getByTestId('detail-tab-flow') !== null).toBe(true);
  });

  it('应在未激活架构页签时零 archi 取数，首次激活发起一次，反复切换每次各一次', async () => {
    renderDetail({ detail: detail({ worktree: 'C:demowt' }) });
    await waitFor(() => expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true));
    const archiCalls = () =>
      invokeMock.mock.calls.filter(([name]) => name === 'archi_graph').length;
    expect(archiCalls()).toBe(0);

    fireEvent.click(screen.getByTestId('detail-tab-architecture'));
    await waitFor(() => expect(archiCalls()).toBe(1));

    fireEvent.click(screen.getByTestId('detail-tab-flow'));
    await waitFor(() => expect(screen.queryByTestId('archi-canvas')).toBeNull());
    fireEvent.click(screen.getByTestId('detail-tab-architecture'));
    await waitFor(() => expect(archiCalls()).toBe(2));
  });

  it('应以 detail.worktree 为画布 root（archi_graph 参数逐字相等）', async () => {
    renderDetail({ detail: detail({ worktree: 'C:demoalpha.worktreesadd-feature' }) });
    await waitFor(() => expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true));
    fireEvent.click(screen.getByTestId('detail-tab-architecture'));

    await waitFor(() =>
      expect(
        invokeMock.mock.calls.some(
          ([name, args]) =>
            name === 'archi_graph' &&
            (args as { root?: string }).root === 'C:demoalpha.worktreesadd-feature',
        ),
      ).toBe(true),
    );
  });

  it('应在归档 change（worktree = null）时回落到页面 root 取数', async () => {
    renderDetail({ detail: detail({ source: 'archive', worktree: null }) }, 'C:demomain');
    await waitFor(() => expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true));
    fireEvent.click(screen.getByTestId('detail-tab-architecture'));

    await waitFor(() =>
      expect(
        invokeMock.mock.calls.some(
          ([name, args]) =>
            name === 'archi_graph' && (args as { root?: string }).root === 'C:demomain',
        ),
      ).toBe(true),
    );
  });

  it('应以 incremental 模式装配画布（抽屉内 gate 可提交）', async () => {
    renderDetail({ detail: detail({ worktree: 'C:demowt' }) });
    await waitFor(() => expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true));
    fireEvent.click(screen.getByTestId('detail-tab-architecture'));
    await waitFor(() => expect(screen.getByTestId('archi-gate-select') !== null).toBe(true));
    expect(screen.queryByTestId('archi-gate-readonly')).toBeNull();
  });

  it('应在架构页签取数失败时呈现错误态且 flow 页签仍可切回', async () => {
    renderDetail({ detail: detail({ worktree: 'C:demowt' }) });
    await waitFor(() => expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true));
    archiReject = 'root 无效（空路径）';
    fireEvent.click(screen.getByTestId('detail-tab-architecture'));
    await waitFor(() => expect(screen.getByTestId('archi-canvas-error') !== null).toBe(true));
    expect(screen.getByTestId('archi-canvas-error').textContent).toContain('root 无效');

    fireEvent.click(screen.getByTestId('detail-tab-flow'));
    await waitFor(() => expect(screen.queryByTestId('archi-canvas-error')).toBeNull());
    expect(screen.getByTestId('detail-tab-flow') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 详情头刷新面与 archiRoot 派生边界（AC-18 / 挂载点 B 回落口径）
// ---------------------------------------------------------------------------

describe('ChangeDetailView：详情头刷新态与时间面', () => {
  it('刷新中：刷新按钮内呈现 spinner 且按钮禁用；落定后 spinner 消退', async () => {
    renderDetail({ detail: detail(), artifacts: PROPOSAL_ENVELOPES });
    await screen.findByTestId('flow-graph');
    const refreshButton = () =>
      screen.getByTestId<HTMLButtonElement>('refresh-change-detail-button');

    expect(refreshButton().querySelector('[data-slot="spinner"]')).toBeNull();
    expect(refreshButton().disabled).toBe(false);

    holdDetail = true;
    fireEvent.click(refreshButton());
    await waitFor(() =>
      expect(refreshButton().querySelector('[data-slot="spinner"]')).toBeTruthy(),
    );
    expect(refreshButton().disabled).toBe(true);

    await act(async () => {
      releaseDetail?.();
    });
    await waitFor(() => expect(refreshButton().querySelector('[data-slot="spinner"]')).toBeNull());
    expect(refreshButton().disabled).toBe(false);
  });

  it('activePhase.startAt 在场：运行中 badge 逐字拼接时间（不落占位符）', async () => {
    const { container } = renderDetail({
      detail: detail({
        activePhase: { phase: 'implement', attempt: 2, startAt: '2026-10-01T08:00:00.000Z' },
      }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('flow-graph');
    const header = within(container).getByTestId('detail-header');
    expect(header.textContent).toContain('运行中 · implement · attempt 2');
    expect(header.textContent).toContain(' · 2026-10-01T08:00:00.000Z');
    expect(header.textContent).not.toContain(' · —');
  });

  it('created 缺席：头部零空文本节点（不渲空 span 占位）', async () => {
    const { container } = renderDetail({
      detail: detail({ created: null }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('flow-graph');
    const header = within(container).getByTestId('detail-header');
    const leaves = Array.from(header.querySelectorAll('span'));
    expect(leaves.length).toBeGreaterThan(0);
    expect(leaves.every((leaf) => (leaf.textContent ?? '').trim().length > 0)).toBe(true);
  });
});

describe('ChangeDetailView：archiRoot 派生边界（挂载点 B 回落口径）', () => {
  function archiGraphRoots(): string[] {
    return invokeMock.mock.calls
      .filter(([name]) => name === 'archi_graph')
      .map(([, args]) => String((args as { root?: unknown }).root));
  }

  it('legacy change（worktree = null 且非归档）应以页面主仓根取数', async () => {
    renderDetail({ detail: detail({ worktree: null }), artifacts: PROPOSAL_ENVELOPES }, ROOT);
    await waitFor(() => expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true));
    fireEvent.click(screen.getByTestId('detail-tab-architecture'));

    await waitFor(() => expect(archiGraphRoots()).toEqual([ROOT]));
  });

  it('归档 change（source = archive 且 worktree 残留）仍以页面主仓根取数（不碰已清理 worktree）', async () => {
    renderDetail(
      {
        detail: detail({ source: 'archive', worktree: 'C:\\demo\\alpha\\.worktrees\\add-feature' }),
        artifacts: PROPOSAL_ENVELOPES,
      },
      ROOT,
    );
    await waitFor(() => expect(screen.getByTestId('detail-tab-architecture') !== null).toBe(true));
    fireEvent.click(screen.getByTestId('detail-tab-architecture'));

    await waitFor(() => expect(archiGraphRoots()).toEqual([ROOT]));
    expect(archiGraphRoots()).not.toContain('C:\\demo\\alpha\\.worktrees\\add-feature');
  });
});
