// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { RecordEnvelope } from '../../types/dto';
import { DbInspectorView } from './DbInspectorView';
import type { DbInspectorState } from './hooks/useDbInspector';

// ---------------------------------------------------------------------------
// DbInspectorView 单测：useDbInspector hook 以 vi.mock 返回受控 fixture state
// （组件纯 state 驱动，沿 AgentDebugView.test.tsx 模式），不含 IPC 边界。
// sonner 以 spy mock 注入：查询轨错误呈现断言「无任何 toast 调用」（错误
// inline 双轨语义）。只读四件套：清单 + 计数、分页、单条 JSON、空/错误态。
// ---------------------------------------------------------------------------

const { toastErrorMock, toastInfoMock, useDbInspectorMock } = vi.hoisted(() => ({
  toastErrorMock: vi.fn(),
  toastInfoMock: vi.fn(),
  useDbInspectorMock: vi.fn(),
}));

vi.mock('../../hooks/useDbInspector', () => ({ useDbInspector: useDbInspectorMock }));

vi.mock('sonner', () => ({
  toast: { error: toastErrorMock, success: toastInfoMock, info: toastInfoMock },
}));

function envelope(key: unknown, value: Record<string, unknown>): RecordEnvelope {
  return { key, value };
}

function makeState(overrides: Partial<DbInspectorState> = {}): DbInspectorState {
  return {
    models: [
      { name: 'workspace', count: 2 },
      { name: 'agent_run', count: 0 },
    ],
    loading: false,
    error: null,
    selected: null,
    records: [],
    recordsLoading: false,
    recordsError: null,
    offset: 0,
    hasMore: false,
    refresh: vi.fn(),
    selectModel: vi.fn(),
    nextPage: vi.fn(),
    prevPage: vi.fn(),
    ...overrides,
  };
}

describe('DbInspectorView：模型清单', () => {
  beforeEach(() => {
    useDbInspectorMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('models 数据渲染清单：每行模型名 + 计数，无 loading 形态', () => {
    useDbInspectorMock.mockReturnValue(makeState());
    render(<DbInspectorView />);

    expect(screen.getByText('模型清单') !== null).toBe(true);
    const items = screen.getAllByTestId('db-model-item');
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toContain('workspace');
    expect(items[0].textContent).toContain('2');
    expect(items[1].textContent).toContain('agent_run');
    expect(items[1].textContent).toContain('0');
    expect(screen.queryByTestId('db-models-loading')).toBeNull();
    // 未选中模型：记录区不挂载
    expect(screen.queryByTestId('db-record-item')).toBeNull();
  });

  it('loading 态呈现进行中形态且刷新按钮禁用', () => {
    useDbInspectorMock.mockReturnValue(makeState({ loading: true }));
    render(<DbInspectorView />);

    expect(screen.getByTestId('db-models-loading') !== null).toBe(true);
    expect(screen.getByTestId('db-refresh').hasAttribute('disabled')).toBe(true);
  });

  it('点击模型行 → selectModel 以该模型名调用（计数 0 的模型仍可选）', () => {
    const state = makeState();
    useDbInspectorMock.mockReturnValue(state);
    render(<DbInspectorView />);

    const items = screen.getAllByTestId('db-model-item');
    fireEvent.click(items[1]); // agent_run（计数 0）

    expect(state.selectModel).toHaveBeenCalledTimes(1);
    expect(state.selectModel).toHaveBeenCalledWith('agent_run');
  });

  it('计数为 0 的模型选中后：空态呈现而非错误（desktop-db-inspector「清单与计数」）', () => {
    useDbInspectorMock.mockReturnValue(makeState({ selected: 'agent_run' }));
    render(<DbInspectorView />);

    expect(screen.getByTestId('db-records-empty') !== null).toBe(true);
    expect(screen.getByTestId('db-records-empty').textContent).toContain('暂无记录');
    expect(screen.queryByTestId('db-inspector-error')).toBeNull();
    expect(screen.queryByTestId('db-record-item')).toBeNull();
    // 记录区标题以「所选模型名」呈现（agent_run 为清单第二项，非 find 首项）
    expect(screen.getByRole('heading', { name: '记录 agent_run' }) !== null).toBe(true);
    expect(screen.queryByText('记录 workspace')).toBeNull();
  });

  it('模型行 className 精确匹配：仅选中行带 font-medium text-primary，非选中行为基线', () => {
    useDbInspectorMock.mockReturnValue(makeState({ selected: 'workspace' }));
    render(<DbInspectorView />);

    const items = screen.getAllByTestId('db-model-item');
    expect(items[0].className).toBe(
      'block w-full cursor-pointer border-0 bg-transparent px-0 py-1.5 text-left hover:text-primary font-medium text-primary',
    );
    expect(items[1].className).toBe(
      'block w-full cursor-pointer border-0 bg-transparent px-0 py-1.5 text-left hover:text-primary ',
    );
  });

  it('页级错误（error 置位）：inline 错误区呈现且模型清单不丢失', () => {
    useDbInspectorMock.mockReturnValue(makeState({ error: 'db: 清单打开失败' }));
    render(<DbInspectorView />);

    const errorNote = screen.getByTestId('db-inspector-error');
    expect(errorNote.textContent).toContain('模型清单加载失败');
    expect(errorNote.textContent).toContain('db: 清单打开失败');
    expect(screen.getAllByTestId('db-model-item')).toHaveLength(2);
  });
});

describe('DbInspectorView：分页扫描与翻页控件', () => {
  beforeEach(() => {
    useDbInspectorMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  const pageState = (overrides: Partial<DbInspectorState> = {}): DbInspectorState =>
    makeState({
      selected: 'workspace',
      records: [
        envelope(1, { id: 1, prompt: '你好' }),
        envelope({ runId: 1, seq: 0 }, { eventKey: '0x1', runId: 1 }),
      ],
      hasMore: true,
      ...overrides,
    });

  it('选中模型后记录区按信封行呈现：key 预览 + value 摘要', () => {
    useDbInspectorMock.mockReturnValue(pageState());
    render(<DbInspectorView />);

    const rows = screen.getAllByTestId('db-record-item');
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain('key: 1');
    expect(rows[0].textContent).toContain('{"id":1,"prompt":"你好"}');
    expect(rows[1].textContent).toContain('key: {"runId":1,"seq":0}');
    // 有记录时既不呈空态也不呈错误区（空态 / 错误 / 记录三分支互斥）
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
    expect(screen.queryByText(/记录扫描失败/)).toBeNull();
    // 控件边界：首页 prev 禁用、hasMore=true 时 next 可用
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(false);
    expect(screen.getByText('offset 0') !== null).toBe(true);
  });

  it('记录行 className 精确匹配：仅被点开的行带 text-primary，关闭行保持基线', () => {
    useDbInspectorMock.mockReturnValue(pageState());
    render(<DbInspectorView />);

    const base =
      'block w-full cursor-pointer border-0 border-b border-b-border bg-transparent px-0 py-2 text-left last:border-b-0 hover:text-primary ';
    let rows = screen.getAllByTestId('db-record-item');
    expect(rows[0].className).toBe(base);
    expect(rows[1].className).toBe(base);

    fireEvent.click(rows[0]);
    rows = screen.getAllByTestId('db-record-item');
    expect(rows[0].className).toBe(`${base}text-primary`);
    expect(rows[1].className).toBe(base);
  });

  it('value 摘要按 120 字符边界截断：短值原样、恰 120 不截、121 截断加省略号', () => {
    const exact120 = JSON.stringify({ pad: 'x'.repeat(110) }); // 恰 120 字符
    const over = JSON.stringify({ pad: 'y'.repeat(111) }); // 121 字符
    expect(exact120).toHaveLength(120);
    expect(over).toHaveLength(121);
    useDbInspectorMock.mockReturnValue(
      makeState({
        selected: 'workspace',
        records: [
          envelope(1, { id: 1 }),
          envelope(2, JSON.parse(exact120) as Record<string, unknown>),
          envelope(3, JSON.parse(over) as Record<string, unknown>),
        ],
      }),
    );
    render(<DbInspectorView />);

    const rows = screen.getAllByTestId('db-record-item');
    expect(rows[0].textContent).toBe('key: 1{"id":1}'); // 短值原样、无省略号
    expect(rows[1].textContent).toBe(`key: 2${exact120}`); // 恰等于 max：不截断
    expect(rows[2].textContent).toBe(`key: 3${over.slice(0, 120)}…`); // 超出：截断 + 省略号
  });

  it('翻页控件按 offset / hasMore 边界启用或禁用；点击派发 nextPage / prevPage', () => {
    const state = pageState({ offset: 50, hasMore: false });
    useDbInspectorMock.mockReturnValue(state);
    const { rerender } = render(<DbInspectorView />);

    // 非首页：prev 可用；末页（hasMore=false）：next 禁用
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(false);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(true);
    expect(screen.getByText('offset 50') !== null).toBe(true);

    fireEvent.click(screen.getByTestId('db-page-prev'));
    expect(state.prevPage).toHaveBeenCalledTimes(1);

    // 换回首页 + 有下一页的 state：prev 禁用、next 可用并可点
    const nextPageState = pageState({ offset: 0, hasMore: true });
    useDbInspectorMock.mockReturnValue(nextPageState);
    rerender(<DbInspectorView />);
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(false);
    fireEvent.click(screen.getByTestId('db-page-next'));
    expect(nextPageState.nextPage).toHaveBeenCalledTimes(1);
  });

  it('recordsLoading 呈现进行中形态且翻页控件禁用', () => {
    useDbInspectorMock.mockReturnValue(pageState({ recordsLoading: true }));
    render(<DbInspectorView />);

    expect(screen.getByTestId('db-records-loading') !== null).toBe(true);
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(true);
    // loading 中不呈空态（避免误报「暂无记录」）
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
  });
});

describe('DbInspectorView：单条 JSON 查看', () => {
  beforeEach(() => {
    useDbInspectorMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('点开单条记录：以 pretty JSON 完整呈现 key 与 value，字段名可读', () => {
    const record = envelope(
      { runId: 1, seq: 3 },
      { eventKey: '0x10000000000000003', runId: 1, event: { seq: 3, kind: 'raw' } },
    );
    useDbInspectorMock.mockReturnValue(
      makeState({
        models: [{ name: 'agent_event', count: 1 }],
        selected: 'agent_event',
        records: [record],
      }),
    );
    render(<DbInspectorView />);

    fireEvent.click(screen.getByTestId('db-record-item'));

    const detail = screen.getByTestId('db-record-json');
    expect(detail.textContent).toBe(JSON.stringify(record, null, 2));
    expect(detail.textContent).toContain('"eventKey"');
    expect(detail.textContent).toContain('"runId"');
    expect(detail.textContent).toContain('0x10000000000000003');
    expect(screen.getByText(/记录详情（信封 JSON）/) !== null).toBe(true);
  });

  it('换模型 / 换页收起单条详情（记录列表换血详情不残留）', () => {
    const record = envelope(1, { id: 1 });
    useDbInspectorMock.mockReturnValue(makeState({ selected: 'workspace', records: [record] }));
    const { rerender } = render(<DbInspectorView />);

    fireEvent.click(screen.getByTestId('db-record-item'));
    expect(screen.getByTestId('db-record-detail') !== null).toBe(true);

    // 模拟换页：offset 变化的 state 重新注入
    useDbInspectorMock.mockReturnValue(
      makeState({ selected: 'workspace', records: [record], offset: 50, hasMore: false }),
    );
    rerender(<DbInspectorView />);
    expect(screen.queryByTestId('db-record-detail')).toBeNull();
  });
});

describe('DbInspectorView：错误态与只读边界', () => {
  beforeEach(() => {
    useDbInspectorMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('recordsError 置位：inline 持久错误区呈现、页面不白屏、已有清单不丢失、无 toast 调用', async () => {
    useDbInspectorMock.mockReturnValue(
      makeState({
        selected: 'workspace',
        records: [],
        recordsError: 'db: 未知模型: nope',
      }),
    );
    render(<DbInspectorView />);

    expect(screen.getByText(/记录扫描失败：db: 未知模型: nope/) !== null).toBe(true);
    expect(screen.getAllByTestId('db-model-item')).toHaveLength(2); // 清单不丢失
    expect(screen.getByTestId('db-model-list') !== null).toBe(true); // 页面不白屏
    // 错误在场时不呈空态（错误态优先于空态，两区不同时渲染）
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
    // inline 持久：静置后错误区仍在（后续渲染不顶替）
    await waitFor(() => expect(screen.getByText(/记录扫描失败/) !== null).toBe(true));
    // 查询轨「错误呈现双轨」：无任何 toast 调用
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(toastInfoMock).not.toHaveBeenCalled();
  });

  it('只读边界：渲染树可交互控件仅选择、翻页、刷新类，无任何写操作入口', () => {
    const record = envelope(1, { id: 1 });
    useDbInspectorMock.mockReturnValue(
      makeState({ selected: 'workspace', records: [record], hasMore: true }),
    );
    render(<DbInspectorView />);

    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBeGreaterThan(0);
    const readonlyActions = ['刷新', '上一页', '下一页'];
    const offenders = buttons
      .filter((button) => {
        const isNav = readonlyActions.some((action) => button.textContent === action);
        const isRow =
          button.getAttribute('data-testid') === 'db-model-item' ||
          button.getAttribute('data-testid') === 'db-record-item';
        return !isNav && !isRow;
      })
      .map((button) => button.textContent ?? '');
    expect(offenders).toEqual([]);
    // 文案面复核：无新增/删除/修改类写操作动词
    const writeish = screen.queryAllByText(/(新增|添加|删除|修改|编辑|写入|保存|创建)/);
    expect(writeish).toHaveLength(0);
  });

  it('模型行与记录行均可点选（选择类控件语义）', () => {
    const state = makeState({
      selected: 'workspace',
      records: [envelope(1, { id: 1 })],
    });
    useDbInspectorMock.mockReturnValue(state);
    render(<DbInspectorView />);

    expect(within(screen.getByTestId('db-model-list')).getAllByRole('button')).toHaveLength(2);
    fireEvent.click(screen.getAllByTestId('db-model-item')[0]);
    expect(state.selectModel).toHaveBeenCalledWith('workspace');
    fireEvent.click(screen.getByTestId('db-record-item'));
    expect(screen.getByTestId('db-record-detail') !== null).toBe(true);
  });
});
