import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation, useNavigate } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArtifactEnvelope, AttemptRecord, ChangeDetail } from '../../types/dto';
import { ChangeDetailView } from './change-detail-view';
import type { ChangeDetailState } from './hooks/use-change-detail';

// ---------------------------------------------------------------------------
// ChangeDetailView 单测：页面组装语义（Header + 流程图区 + workflow 独立面板 +
// 产物区 + 抽屉挂载）。取数下沉后 useChangeDetail 在组件内调用：mock 为受控
// vi.fn 回灌状态（布线 / 根切换抑制断言直接观察 hook 入参，hook 本体契约零
// 改动）；MemoryRouter + /changes/:name 路由为测试装置（提供 useParams /
// useNavigate 上下文并以 initialEntries 控制路由参数初态）。Tauri invoke 的
// 进程边界 mock 仅在 __tests__ 集成用例中使用。旧线性布局断言整体退役
// （test-design「废弃」项）：attempt 序列 / （无记录）/ 中断留档区块 / 页面级
// filelog-table 矩阵等语义分别迁往 flow/*.test 与集成用例。
// ---------------------------------------------------------------------------

const { useChangeDetailMock } = vi.hoisted(() => ({ useChangeDetailMock: vi.fn() }));

vi.mock('./hooks/use-change-detail', () => ({ useChangeDetail: useChangeDetailMock }));

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
  useChangeDetailMock.mockReset();
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

function state(
  overrides: Partial<ChangeDetailState> & {
    detail?: ChangeDetail | null;
    artifacts?: ArtifactEnvelope[];
  },
): ChangeDetailState {
  return {
    detail: null,
    artifacts: [],
    loading: false,
    error: null,
    refresh: () => {},
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

/** 装置：回灌受控详情态后渲染详情页（默认落 /changes/add-feature）。 */
function renderDetail(s: ChangeDetailState, root: string = ROOT, initialEntry?: string) {
  useChangeDetailMock.mockReturnValue(s);
  return render(
    <MemoryRouter initialEntries={[initialEntry ?? '/changes/add-feature']}>
      {detailTree(root)}
    </MemoryRouter>,
  );
}

/** 已挂载详情页换根重渲染（根切换场景）：Router 实例保持、location 不重置。 */
function rerenderDetail(view: ReturnType<typeof renderDetail>, s: ChangeDetailState, root: string) {
  useChangeDetailMock.mockReturnValue(s);
  view.rerender(
    <MemoryRouter initialEntries={['/changes/add-feature']}>{detailTree(root)}</MemoryRouter>,
  );
}

function probePathname(): string {
  return screen.getByTestId('location-probe').textContent ?? '';
}

describe('ChangeDetailView：页面组装（图区 / workflow 面板 / 产物区 / 抽屉）', () => {
  it('v2 详情 → flow-graph 图区、workflow-panel 面板（内含 filelog-table）、产物区三者并存', async () => {
    const base = detail();
    const { container } = renderDetail(state({ detail: base, artifacts: [proposalDoc()] }));
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    const panel = screen.getByTestId('workflow-panel');
    expect(within(panel).getByTestId('filelog-table') !== null).toBe(true);
    // 面板行集 = outsideFiles（workflow-scope 条目不入图）
    expect(within(panel).getAllByRole('row')).toHaveLength(2);
    // 产物区独立于面板照常渲染
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('Header 元信息：name / inventory 徽标（detail-header 域）/ source / created；activePhase 运行中 badge（startAt null 不拼时间与占位）', () => {
    const { container } = renderDetail(state({ detail: detail() }));
    const header = within(container).getByTestId('detail-header');
    expect(within(header).getByText('add-feature') !== null).toBe(true);
    expect(within(header).getByText('v2') !== null).toBe(true);
    expect(within(header).getByText('进行中') !== null).toBe(true);
    expect(within(header).getByText('2026-09-01') !== null).toBe(true);
    expect(container.textContent).not.toContain('已归档');
    // activePhase 为 null → 无运行中 badge
    expect(screen.queryByText(/运行中/)).toBeNull();

    const archived = renderDetail(
      state({
        detail: detail({
          source: 'archive',
          created: null,
          activePhase: { phase: 'implement', attempt: 2, startAt: null },
        }),
      }),
    );
    expect(archived.container.textContent).toContain('已归档');
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
    renderDetail(state({ detail: detail(), artifacts: [proposalDoc()] }));
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

  it('unparsable 警示条正负两例（warn-note 有 / 无）', () => {
    const warned = renderDetail(state({ detail: detail({ unparsable: true }) }));
    expect(within(warned.container).getByTestId('warn-note').textContent).toContain(
      'workflow.json 无法解析',
    );
    warned.unmount();

    const clean = renderDetail(state({ detail: detail() }));
    expect(within(clean.container).queryByTestId('warn-note')).toBeNull();
    expect(clean.container.textContent).not.toContain('workflow.json 无法解析');
  });

  it('产物区多文档 tab 切换：tab 按信封顺序、默认首项选中、点击切换仅渲染当前卡片；清单变短 active 收敛', () => {
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
    const view = renderDetail(state({ detail: detail(), artifacts: three }));
    const { container } = view;
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

    // 刷新后清单变短：active=2 越界收敛到末项（任务进度），不重置回首项
    rerenderDetail(view, state({ detail: detail(), artifacts: three.slice(0, 2) }), ROOT);
    const shrunk = within(container).getAllByTestId('artifact-tab');
    expect(shrunk.map((tab) => tab.textContent)).toEqual(['提案', '任务进度']);
    expect(shrunk[1].getAttribute('aria-selected')).toBe('true');
    expect(container.textContent).toContain('100%');
  });

  it('产物区单文档不设 tab 条、卡片直接渲染；空清单「（未发现可读产物）」占位', () => {
    const view = renderDetail(state({ detail: detail(), artifacts: [proposalDoc()] }));
    expect(within(view.container).queryByTestId('artifact-tabs')).toBeNull();
    expect(within(view.container).getAllByTestId('artifact-card')).toHaveLength(1);
    rerenderDetail(view, state({ detail: detail(), artifacts: [] }), ROOT);
    expect(view.container.textContent).toContain('（未发现可读产物）');
  });

  it('点击「刷新详情」触发 refresh；点击「← 返回列表」显式 navigate 落 /changes（不用历史回退）', () => {
    const refresh = vi.fn();
    renderDetail(state({ detail: detail(), refresh }));
    fireEvent.click(screen.getByRole('button', { name: '刷新详情' }));
    expect(refresh).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    expect(probePathname()).toBe('/changes');
    // 落点为 /changes（本装置以导航桩占位，真实清单页语义归 change-list-view.test）
    expect(screen.getByTestId('nav-stub') !== null).toBe(true);
  });

  it('v0（inventory v0 + pipeline 空）→ 不挂 flow-graph，渲染 flow-empty 占位，产物区照常', () => {
    const { container } = renderDetail(
      state({
        detail: detail({
          inventory: 'v0',
          pipeline: [],
          fileLog: null,
          artifacts: [{ kind: 'markdown-doc', source: 'proposal.md', title: '提案' }],
        }),
        artifacts: [proposalDoc()],
      }),
    );
    expect(screen.queryByTestId('flow-graph')).toBeNull();
    expect(within(container).getByTestId('flow-empty').textContent).toContain(
      'v0 早期代际：无 workflow.json，仅文档形态',
    );
    // 产物区不受图区缺位影响照常
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('v1（fileLog null）→ flow-graph 正常渲染且无 workflow-panel', async () => {
    const { container } = renderDetail(
      state({ detail: detail({ inventory: 'v1', fileLog: null }) }),
    );
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    expect(screen.getAllByTestId('flow-node').length).toBeGreaterThan(0);
    expect(within(container).queryByTestId('workflow-panel')).toBeNull();
  });

  it('error / loading（无 detail）/ 未找到三态降级页与 detail-note / error-note 挂钩归属正确', () => {
    const view = renderDetail(state({}));
    expect(view.container.textContent).toContain('未找到该 change。');
    expect(within(view.container).getByTestId('detail-note') !== null).toBe(true);
    expect(within(view.container).queryByTestId('error-note')).toBeNull();

    rerenderDetail(view, state({ loading: true }), ROOT);
    expect(view.container.textContent).toContain('加载中…');
    expect(within(view.container).getByTestId('detail-note') !== null).toBe(true);
    expect(within(view.container).queryByTestId('error-note')).toBeNull();

    rerenderDetail(view, state({ error: 'IPC 断开' }), ROOT);
    expect(view.container.textContent).toContain('详情加载失败：IPC 断开');
    const note = within(view.container).getByTestId('error-note');
    expect(note.textContent).toContain('详情加载失败：IPC 断开');
    expect(within(view.container).queryByTestId('detail-note')).toBeNull();
  });

  it('loading 中已有 detail 不回落加载占位，刷新按钮禁用', () => {
    renderDetail(state({ loading: true, detail: detail() }));
    expect(screen.queryByText('加载中…')).toBeNull();
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    expect(screen.getByRole('button', { name: '刷新详情' }).hasAttribute('disabled')).toBe(true);
  });
});

describe('ChangeDetailView：路由参数取数、深链与根切换抑制（页面自取数）', () => {
  it('详情路由 /changes/:name：useChangeDetail 以 (root, name) 调用恰一次', () => {
    renderDetail(state({ detail: detail() }));

    expect(useChangeDetailMock).toHaveBeenCalledTimes(1);
    expect(useChangeDetailMock).toHaveBeenCalledWith(ROOT, 'add-feature');
  });

  it('未知 change 深链透传（/changes/ghost）：选中态不做校验、以 (root, ghost) 调用，呈现交由 detail 态（降级页）且不崩', () => {
    renderDetail(state({}), ROOT, '/changes/ghost');

    expect(useChangeDetailMock).toHaveBeenCalledWith(ROOT, 'ghost');
    // detail=null → 既有「未找到该 change。」降级兜底，返回入口在场
    expect(screen.getByText('未找到该 change。') !== null).toBe(true);
    expect(screen.getByRole('button', { name: '← 返回列表' }) !== null).toBe(true);
  });

  it('详情态以新 root 重渲染：过渡轮 useChangeDetail 以 (新root, null) 调用（旧名抑制），随后 replace 导航落 /changes；再入详情恢复 (新root, 新名) 取数', async () => {
    const view = renderDetail(state({ detail: detail() }));
    expect(useChangeDetailMock).toHaveBeenCalledWith(ROOT, 'add-feature');

    rerenderDetail(view, state({ detail: detail() }), SECOND);

    // 导航落点：/changes（URL 无 :name 段）
    await waitFor(() => expect(probePathname()).toBe('/changes'));
    expect(screen.getByTestId('nav-stub') !== null).toBe(true);

    // 抑制不变量：一旦过渡轮 (新root, null) 出现，其后不再有 (新root, 非null) 入参
    // （抑制位随组件卸载终结；端到端「无新根 + 旧名误发」由 app.test 的 invoke 记录承接）
    const calls = useChangeDetailMock.mock.calls as Array<[string, string | null]>;
    const firstSuppressed = calls.findIndex(([root, change]) => root === SECOND && change === null);
    expect(firstSuppressed).toBeGreaterThan(-1);
    expect(
      calls.slice(firstSuppressed).some(([root, change]) => root === SECOND && change !== null),
    ).toBe(false);

    // 组件卸载后抑制不残留：再入详情（新根 + 新名）恢复正常取数
    fireEvent.click(screen.getByTestId('nav-stub'));
    expect(probePathname()).toBe('/changes/beta-fix');
    expect(useChangeDetailMock).toHaveBeenLastCalledWith(SECOND, 'beta-fix');
  });
});
