import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArtifactEnvelope, AttemptRecord, ChangeDetail } from '../../types/dto';
import { ChangeDetailView } from './change-detail-view';
import type { ChangeDetailState } from './hooks/use-change-detail';

// ---------------------------------------------------------------------------
// ChangeDetailView 单测：页面组装语义（Header + 流程图区 + workflow 独立面板 +
// 产物区 + 抽屉挂载），state 以 ChangeDetailState 手工 fixture 直供（无跨进程
// Mock）；Tauri invoke 的进程边界 mock 仅在 __tests__ 集成用例中使用。
// 旧线性布局断言整体退役（test-design「废弃」项）：attempt 序列 / （无记录）/
// 中断留档区块 / 页面级 filelog-table 矩阵等语义分别迁往 flow/*.test 与集成用例。
// ---------------------------------------------------------------------------

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

describe('ChangeDetailView：页面组装（图区 / workflow 面板 / 产物区 / 抽屉）', () => {
  it('v2 详情 → flow-graph 图区、workflow-panel 面板（内含 filelog-table）、产物区三者并存', async () => {
    const base = detail();
    const { container } = render(
      <ChangeDetailView
        state={state({ detail: base, artifacts: [proposalDoc()] })}
        onBack={() => {}}
      />,
    );
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    const panel = screen.getByTestId('workflow-panel');
    expect(within(panel).getByTestId('filelog-table') !== null).toBe(true);
    // 面板行集 = outsideFiles（workflow-scope 条目不入图）
    expect(within(panel).getAllByRole('row')).toHaveLength(2);
    // 产物区独立于面板照常渲染
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('Header 元信息：name / inventory 徽标（detail-header 域）/ source / created；activePhase 运行中 badge（startAt null 不拼时间与占位）', () => {
    const { container } = render(
      <ChangeDetailView state={state({ detail: detail() })} onBack={() => {}} />,
    );
    const header = within(container).getByTestId('detail-header');
    expect(within(header).getByText('add-feature') !== null).toBe(true);
    expect(within(header).getByText('v2') !== null).toBe(true);
    expect(within(header).getByText('进行中') !== null).toBe(true);
    expect(within(header).getByText('2026-09-01') !== null).toBe(true);
    expect(container.textContent).not.toContain('已归档');
    // activePhase 为 null → 无运行中 badge
    expect(screen.queryByText(/运行中/)).toBeNull();

    const archived = render(
      <ChangeDetailView
        state={state({
          detail: detail({
            source: 'archive',
            created: null,
            activePhase: { phase: 'implement', attempt: 2, startAt: null },
          }),
        })}
        onBack={() => {}}
      />,
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
    render(
      <ChangeDetailView
        state={state({ detail: detail(), artifacts: [proposalDoc()] })}
        onBack={() => {}}
      />,
    );
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
    const warned = render(
      <ChangeDetailView
        state={state({ detail: detail({ unparsable: true }) })}
        onBack={() => {}}
      />,
    );
    expect(within(warned.container).getByTestId('warn-note').textContent).toContain(
      'workflow.json 无法解析',
    );
    warned.unmount();

    const clean = render(
      <ChangeDetailView state={state({ detail: detail() })} onBack={() => {}} />,
    );
    expect(within(clean.container).queryByTestId('warn-note')).toBeNull();
    expect(clean.container.textContent).not.toContain('workflow.json 无法解析');
  });

  it('产物区按信封顺序渲染 ArtifactView 列表；空清单「（未发现可读产物）」占位', () => {
    const { container, rerender } = render(
      <ChangeDetailView
        state={state({
          detail: detail(),
          artifacts: [
            proposalDoc(),
            envelope({ title: '任务进度' }),
            envelope({
              kind: 'unknown-kind',
              title: '未知产物',
              payload: null,
              fallbackText: '未知保底',
            }),
          ],
        })}
        onBack={() => {}}
      />,
    );
    const cards = within(container).getAllByTestId('artifact-card');
    expect(cards).toHaveLength(3);
    expect(cards[0].textContent).toContain('提案正文');
    expect(cards[1].textContent).toContain('100%');
    expect(cards[2].textContent).toContain('未知保底');
    rerender(
      <ChangeDetailView state={state({ detail: detail(), artifacts: [] })} onBack={() => {}} />,
    );
    expect(container.textContent).toContain('（未发现可读产物）');
  });

  it('点击「← 返回列表」回调触发；点击「刷新详情」触发 refresh 回调', () => {
    const onBack = vi.fn();
    const refresh = vi.fn();
    render(<ChangeDetailView state={state({ detail: detail(), refresh })} onBack={onBack} />);
    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    expect(onBack).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: '刷新详情' }));
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('v0（inventory v0 + pipeline 空）→ 不挂 flow-graph，渲染 flow-empty 占位，产物区照常', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({
          detail: detail({
            inventory: 'v0',
            pipeline: [],
            fileLog: null,
            artifacts: [{ kind: 'markdown-doc', source: 'proposal.md', title: '提案' }],
          }),
          artifacts: [proposalDoc()],
        })}
        onBack={() => {}}
      />,
    );
    expect(screen.queryByTestId('flow-graph')).toBeNull();
    expect(within(container).getByTestId('flow-empty').textContent).toContain(
      'v0 早期代际：无 workflow.json，仅文档形态',
    );
    // 产物区不受图区缺位影响照常
    expect(within(container).getAllByTestId('artifact-card')).toHaveLength(1);
  });

  it('v1（fileLog null）→ flow-graph 正常渲染且无 workflow-panel', async () => {
    const { container } = render(
      <ChangeDetailView
        state={state({ detail: detail({ inventory: 'v1', fileLog: null }) })}
        onBack={() => {}}
      />,
    );
    await waitFor(() => expect(screen.getAllByTestId('flow-column')).toHaveLength(9));
    expect(screen.getAllByTestId('flow-node').length).toBeGreaterThan(0);
    expect(within(container).queryByTestId('workflow-panel')).toBeNull();
  });

  it('error / loading（无 detail）/ 未找到三态降级页与 detail-note / error-note 挂钩归属正确', () => {
    const { container, rerender } = render(
      <ChangeDetailView state={state({})} onBack={() => {}} />,
    );
    expect(container.textContent).toContain('未找到该 change。');
    expect(within(container).getByTestId('detail-note') !== null).toBe(true);
    expect(within(container).queryByTestId('error-note')).toBeNull();

    rerender(<ChangeDetailView state={state({ loading: true })} onBack={() => {}} />);
    expect(container.textContent).toContain('加载中…');
    expect(within(container).getByTestId('detail-note') !== null).toBe(true);
    expect(within(container).queryByTestId('error-note')).toBeNull();

    rerender(<ChangeDetailView state={state({ error: 'IPC 断开' })} onBack={() => {}} />);
    expect(container.textContent).toContain('详情加载失败：IPC 断开');
    const note = within(container).getByTestId('error-note');
    expect(note.textContent).toContain('详情加载失败：IPC 断开');
    expect(within(container).queryByTestId('detail-note')).toBeNull();
  });

  it('loading 中已有 detail 不回落加载占位，刷新按钮禁用', () => {
    render(
      <ChangeDetailView state={state({ loading: true, detail: detail() })} onBack={() => {}} />,
    );
    expect(screen.queryByText('加载中…')).toBeNull();
    expect(screen.getByTestId('flow-graph') !== null).toBe(true);
    expect(screen.getByRole('button', { name: '刷新详情' }).hasAttribute('disabled')).toBe(true);
  });
});
