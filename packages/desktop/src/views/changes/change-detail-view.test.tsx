import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation, useNavigate } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  AgentEvent,
  ArtifactDescriptor,
  ArtifactEnvelope,
  AttemptRecord,
  ChangeDetail,
  ChangeRunSnapshot,
  ChangeStepKind,
  ChangeStepState,
  ChangeStepStatus,
  RunUpdate,
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

/** 详情取数命令分发（run describe 复用同一定向，落 run 缺席命令时为空态应答）。 */
function detailIpc(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
  if (command === 'get_change_detail') {
    if (typeof detailResult === 'string') return Promise.reject(detailResult);
    if (holdDetail) {
      return new Promise((resolve, reject) => {
        releaseDetail = () => {
          if (typeof detailResult === 'string') reject(detailResult);
          else resolve(detailResult);
        };
      });
    }
    return Promise.resolve(detailResult);
  }
  if (command === 'read_artifact') {
    const key = `${String(args.kind)}:${String(args.source)}`;
    return Promise.resolve(key in artifactResults ? artifactResults[key] : null);
  }
  // run / 会话命令缺省空态应答（无运行 run / 空会话清单）
  return Promise.resolve(null);
}

class ResizeObserverStub {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}

beforeEach(() => {
  // 流程图区（v1 / v2 详情）挂载 ReactFlow：jsdom 缺口垫片（环境 stub 而非业务 mock）
  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  // xyflow 边标签度量依赖 SVGGraphicsElement.getBBox，jsdom 未实现 → 零包围盒垫片
  const svgPrototype = globalThis.SVGElement?.prototype as unknown as
    | Record<string, unknown>
    | undefined;
  if (svgPrototype !== undefined && typeof svgPrototype.getBBox !== 'function') {
    svgPrototype.getBBox = () => ({ x: 0, y: 0, width: 0, height: 0 });
  }
  ChannelMock.instances.length = 0;
  invokeMock.mockReset();
  loadFixture({ detail: null });
  invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) =>
    detailIpc(command, args),
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
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
    ...overrides,
  };
}

function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: '2026-09-01',
    unparsable: false,
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
    interrupted: [],
    fileLog: [{ op: 'write', scope: 'workflow', attempt: null, path: 'workflow.json', at: null }],
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
// 路由装置（自 change-view.test 迁入的测试形态）：/changes/:name 直挂详情页；
// /changes 以导航桩占位（真实清单页语义归 change-list-view.test），提供落点
// 断言与再入详情的点击入口。
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
const SECOND = 'C:\\demo\\beta';

/** URL 探针：把 MemoryRouter 当前 pathname 投影到 DOM 供断言。 */
function LocationProbe() {
  const { pathname } = useLocation();
  return <span data-testid="location-probe">{pathname}</span>;
}

/** /changes 路由占位桩：模拟清单页再入详情的导航入口。 */
function NavStub() {
  const navigate = useNavigate();
  return (
    <button data-testid="nav-stub" onClick={() => navigate('/changes/beta-fix')}>
      goto-beta-fix
    </button>
  );
}

function detailTree(root: string) {
  return (
    <>
      <Routes>
        <Route path="/changes" element={<NavStub />} />
        <Route path="/changes/:name" element={<ChangeDetailView root={root} />} />
      </Routes>
      <LocationProbe />
    </>
  );
}

/** 装置：灌入详情 fixture 后渲染详情页（默认落 /changes/add-feature）。 */
function renderDetail(f: DetailFixture, root: string = ROOT, initialEntry?: string) {
  loadFixture(f);
  return render(
    <MemoryRouter initialEntries={[initialEntry ?? '/changes/add-feature']}>
      {detailTree(root)}
    </MemoryRouter>,
  );
}

/** 已挂载详情页换根重渲染（根切换场景）：Router 实例保持、location 不重置。 */
function rerenderDetail(view: ReturnType<typeof renderDetail>, f: DetailFixture, root: string) {
  loadFixture(f);
  view.rerender(
    <MemoryRouter initialEntries={['/changes/add-feature']}>{detailTree(root)}</MemoryRouter>,
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

describe('ChangeDetailView：页面组装（图区 / workflow 面板 / 产物区 / 抽屉）', () => {
  it('v2 详情 → flow-graph 图区、workflow-panel 面板（内含 filelog-table）、产物区三者并存', async () => {
    const { container } = renderDetail({
      detail: detail(),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await screen.findByTestId('flow-graph');
    const panel = screen.getByTestId('workflow-panel');
    expect(within(panel).getByTestId('filelog-table') !== null).toBe(true);
    // 面板行集 = outsideFiles（workflow-scope 条目不入图）
    expect(within(panel).getAllByRole('row')).toHaveLength(2);
    // 产物区独立于面板照常渲染
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('Header 元信息：name / inventory 徽标（detail-header 域）/ source / created；activePhase 运行中 badge（startAt null 不拼时间与占位）', async () => {
    const { container } = renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
    const header = within(container).getByTestId('detail-header');
    expect(within(header).getByText('add-feature') !== null).toBe(true);
    expect(within(header).getByText('v2') !== null).toBe(true);
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

  it('unparsable 警示条正负两例（warn-note 有 / 无）', async () => {
    const warned = renderDetail({ detail: detail({ unparsable: true }) });
    await waitFor(() =>
      expect(within(warned.container).getByTestId('warn-note').textContent).toContain(
        'workflow.json 无法解析',
      ),
    );
    warned.unmount();

    const clean = renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
    expect(within(clean.container).queryByTestId('warn-note')).toBeNull();
    expect(clean.container.textContent).not.toContain('workflow.json 无法解析');
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

    // TabsTrigger 激活绑在 mousedown（Radix 1.1 行为），点击事件用 mouseDown 模拟
    fireEvent.mouseDown(tabs[2]);
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

  it('v0（inventory v0 + pipeline 空）→ 不挂 flow-graph，渲染 flow-empty 占位，产物区照常', async () => {
    const { container } = renderDetail({
      detail: detail({
        inventory: 'v0',
        pipeline: [],
        fileLog: null,
        artifacts: [{ kind: 'markdown-doc', source: 'proposal.md', title: '提案' }],
      }),
      artifacts: PROPOSAL_ENVELOPES,
    });
    await waitFor(() => expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1));
    expect(screen.queryByTestId('flow-graph')).toBeNull();
    expect(within(container).getByTestId('flow-empty').textContent).toContain(
      'v0 早期代际：无 workflow.json，仅文档形态',
    );
    // 产物区不受图区缺位影响照常
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('v1（fileLog null）→ flow-graph 正常渲染且无 workflow-panel', async () => {
    const { container } = renderDetail({
      detail: detail({ inventory: 'v1', fileLog: null }),
    });
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    expect(screen.getAllByTestId('flow-node').length).toBeGreaterThan(0);
    expect(within(container).queryByTestId('workflow-panel')).toBeNull();
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
  it('详情路由 /changes/:name：get_change_detail 以 (root, name) 调用恰一次', async () => {
    renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');

    expect(detailCalls()).toHaveLength(1);
    expect(detailCalls()[0]).toEqual({ root: ROOT, change: 'add-feature' });
  });

  it('未知 change 深链透传（/changes/ghost）：以 (root, ghost) 取数（null 应答），呈现「未找到该 change。」降级页且不崩', async () => {
    renderDetail({ detail: null }, ROOT, '/changes/ghost');

    await waitFor(() => expect(screen.getByText('未找到该 change。') !== null).toBe(true));
    expect(detailCalls()).toEqual([{ root: ROOT, change: 'ghost' }]);
    // detail=null → 既有「未找到该 change。」降级兜底，返回入口在场
    expect(screen.getByRole('button', { name: '← 返回列表' }) !== null).toBe(true);
  });

  it('详情态以新 root 重渲染：过渡轮抑制取数（零「新根 + 旧名」误发），replace 导航落 /changes；再入详情恢复 (新root, 新名) 取数', async () => {
    const view = renderDetail({ detail: detail() });
    await screen.findByTestId('flow-graph');
    expect(detailCalls()).toEqual([{ root: ROOT, change: 'add-feature' }]);

    rerenderDetail(view, { detail: detail() }, SECOND);

    // 导航落点：/changes（URL 无 :name 段）
    await waitFor(() => expect(probePathname()).toBe('/changes'));
    expect(screen.getByTestId('nav-stub') !== null).toBe(true);

    // 抑制不变量（invoke 观察面）：过渡轮 useChangeDetail(SECOND, null) 无从
    // 取数 → 零「新根 + 旧名」误发、抑制轮零 invoke（抑制位随组件卸载终结；
    // 端到端「无新根 + 旧名误发」由 app.test 的 invoke 记录承接）
    expect(detailCalls()).toHaveLength(1);

    // 组件卸载后抑制不残留：再入详情（新根 + 新名）恢复正常取数
    fireEvent.click(screen.getByTestId('nav-stub'));
    expect(probePathname()).toBe('/changes/beta-fix');
    await waitFor(() => expect(detailCalls().at(-1)).toEqual({ root: SECOND, change: 'beta-fix' }));
  });
});

// ---------------------------------------------------------------------------
// run 组装（desktop-change-flow 增量）：页面调 useChangeFlowRun、头部区挂
// RunControlPanel、runStep overlay 并入 buildFlowGraph、抽屉转录 props 下传、
// run 终态触发一次显式 refresh。运行态经重挂快照恢复（change_flow_state 运行中
// 快照 → watch 补订）驱动，信封流入真实 applyRunUpdate / runStepNodes 纯函数
// 归并后并入图 overlay；ResizeObserver / getBBox 垫片沿既有装置。
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

/** 重挂快照 fixture（change_flow_state 应答；缺省运行中 implement attempt 1）。 */
function flowSnapshot(overrides: Partial<ChangeRunSnapshot> = {}): ChangeRunSnapshot {
  return {
    runId: 'run-1727',
    status: 'running',
    phase: 'implement',
    attempt: 1,
    ask: null,
    ...overrides,
  };
}

function stepUpdate(
  step: ChangeStepKind,
  status: ChangeStepStatus,
  overrides: Partial<ChangeStepState> = {},
): RunUpdate {
  return {
    ipc: 'step',
    step: {
      phase: 'implement',
      attempt: 1,
      step,
      status,
      sessionId: null,
      detail: null,
      ...overrides,
    },
  };
}

function sessionUpdate(seq: number, sessionId: string, text: string): RunUpdate {
  return {
    ipc: 'sessionEvent',
    sessionId,
    event: {
      seq,
      timestampMs: 1727000000000 + seq,
      kind: 'message',
      role: 'assistant',
      blocks: [{ kind: 'text', text }],
      parentToolUseId: null,
    },
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
  let stateSnapshot: ChangeRunSnapshot | null;
  let sessionsFixture: Record<string, SessionSummary[]>;
  let transcriptFixture: Record<string, AgentEvent[]>;

  beforeEach(() => {
    stateSnapshot = null;
    sessionsFixture = {};
    transcriptFixture = {};
    invokeMock.mockImplementation(
      (command: string, args: { sourceRef?: string; sessionId?: string } = {}) => {
        // 详情取数经共享定向（useChangeDetail 真实组合的进程边界）
        if (command === 'get_change_detail' || command === 'read_artifact') {
          return detailIpc(command, args);
        }
        if (command === 'change_flow_state') return Promise.resolve(stateSnapshot);
        if (command === 'change_flow_start') {
          return Promise.resolve({ runId: 'run-1727', status: 'running' });
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
    // 既有组装（新增为加法）：Header + 图区 + workflow 独立面板 + 产物区
    expect(within(container).getByTestId('workflow-panel') !== null).toBe(true);
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);

    // 发起入口接通 useChangeFlowRun：change_flow_start 携 root/change 与 Channel 实例
    fireEvent.click(screen.getByTestId('run-start'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('change_flow_start', {
        onEvent: expect.anything(),
        root: ROOT,
        change: 'add-feature',
      }),
    );
  });

  it('运行中（重挂快照恢复）→ 停止入口在场；步状态流入后 runStep 节点出现在图，既有事件节点不受影响', async () => {
    stateSnapshot = flowSnapshot({ status: 'running', phase: 'implement', attempt: 1 });
    renderDetail({ detail: detail() });

    // 快照恢复 → 状态徽章与停止入口呈现、watch 补订建立
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));
    await screen.findByTestId('flow-graph');
    expect(screen.getByTestId('run-status').textContent).toBe('运行中');

    await act(async () => {
      lastChannel().push(stepUpdate('executor', 'running', { sessionId: 'ses-exec' }));
      lastChannel().push(stepUpdate('staticCheck', 'failed', { detail: '2 处诊断' }));
    });
    await waitFor(() => expect(screen.getAllByTestId('run-step-node')).toHaveLength(2));
    const executor = screen.getByTestId('rf__node-run:implement:1:executor');
    expect(within(executor).getByTestId('run-step-group').textContent).toBe('WorkerAgent');
    const check = screen.getByTestId('rf__node-run:implement:1:staticCheck');
    expect(within(check).getByTestId('run-step-group').textContent).toBe('ToolStep');
    expect(within(check).getByTestId('run-step-detail').textContent).toBe('2 处诊断');
    // overlay 为加法：既有事件节点集合不变
    expect(screen.getAllByTestId('flow-node')).toHaveLength(2);
  });

  it('run 终态（非终局 → 终局迁移）恰触发一次显式 refresh；终态后追加信封不再触发、主操作回到发起', async () => {
    stateSnapshot = flowSnapshot({ status: 'running', phase: 'implement', attempt: 1 });
    renderDetail({ detail: detail() });
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));
    const baseline = detailCalls().length;

    await act(async () => {
      lastChannel().push({ ipc: 'finished', status: 'completed', reason: '全相位通过' });
    });
    await waitFor(() => expect(detailCalls().length).toBe(baseline + 1));
    expect(screen.getByTestId('run-finished-reason').textContent).toContain('全相位通过');
    // 收口对齐：终局后主操作回到发起
    expect(screen.getByTestId('run-start') !== null).toBe(true);

    // 终态后到站信封不再触发（状态不再迁移 = 「非终局 → 终局」判定不重现）
    await act(async () => {
      lastChannel().push(stepUpdate('phaseStart', 'stopped', { phase: 'test-gen' }));
    });
    expect(detailCalls().length).toBe(baseline + 1);
  });

  it('抽屉选中运行步节点 → 转录 props 下传链路接通：反查携带 role×attempt sourceRef、实时事件经展平并入面板', async () => {
    stateSnapshot = flowSnapshot({ status: 'running', phase: 'implement', attempt: 1 });
    sessionsFixture = {
      'add-feature/implement/executor/1': [
        transcriptSession('ses-exec', 'add-feature/implement/executor/1'),
      ],
    };
    transcriptFixture = { 'ses-exec': [textEvent(0, 'user', '重放正文')] };
    renderDetail({ detail: detail() });
    await screen.findByTestId('run-stop');
    await waitFor(() => expect(ChannelMock.instances).toHaveLength(1));

    await act(async () => {
      lastChannel().push(stepUpdate('executor', 'running', { sessionId: 'ses-exec' }));
      lastChannel().push(sessionUpdate(1, 'ses-exec', '实时增量正文'));
    });
    fireEvent.click(await screen.findByTestId('rf__node-run:implement:1:executor'));

    const panel = await screen.findByTestId('session-transcript-panel');
    await waitFor(() => expect(panel.textContent).toContain('重放正文'));
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'change',
      sourceRef: 'add-feature/implement/executor/1',
    });

    // 实时事件随后到站：liveEvents 展平身份变化 → 抽屉按 sessionId 过滤下传 → 面板按 seq 并入
    await act(async () => {
      lastChannel().push(sessionUpdate(2, 'ses-exec', '二次实时正文'));
    });
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').textContent).toContain('实时增量正文'),
    );
    expect(screen.getByTestId('session-transcript-panel').textContent).toContain('重放正文');
  });
});
