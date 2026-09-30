// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ModelInfo, RecordEnvelope } from '../../types/dto';
import { DbInspectorView } from './db-inspector-view';

// ---------------------------------------------------------------------------
// DbInspectorView 单测（最小 mock 原则，沿 agent-debug-view.test.tsx 模式）：
// useDbInspector 为进程内内部协作者，不 mock——组件真实组合 hook，唯一 mock
// 点为 @tauri-apps/api/core 进程边界：invoke 按命令名分发替身（db_models /
// db_records，固定 fixture 并记录调用序列，断言 scope / root 入参）。hook 重置
// 语义（scope 切换 → 清单重取 + 选中 / 分页复位）经真实组合的 invoke 面可观测，
// 不以替身 state 注入降级。sonner 以 spy mock 注入：查询轨错误呈现断言「无任何
// toast 调用」（错误 inline 双轨语义）。只读四件套：清单 + 计数、分页、单条
// JSON、空/错误态。
// ---------------------------------------------------------------------------

const { toastErrorMock, toastInfoMock, invokeMock } = vi.hoisted(() => ({
  toastErrorMock: vi.fn(),
  toastInfoMock: vi.fn(),
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

vi.mock('sonner', () => ({
  toast: { error: toastErrorMock, success: toastInfoMock, info: toastInfoMock },
}));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

/** workspace root（workspace 库 scope 寻址基准；scope 默认 workspace 库） */
const ROOT = 'C:\\demo\\beta';
const WS_PARAMS = { scope: 'workspace', root: ROOT } as const;
/** 记录分页页长（与 hook 内 PAGE_SIZE 同源；常量未导出，按实现值字面书写） */
const PAGE_SIZE = 50;

const MODELS: ModelInfo[] = [
  { name: 'workspace', count: 2 },
  { name: 'agent_run', count: 0 },
];

function envelope(key: unknown, value: Record<string, unknown>): RecordEnvelope {
  return { key, value };
}

/** 分页库存：60 条 = 1 整页（50，hasMore=true）+ 10 条尾页；首两条为特形信封 */
function pagedInventory(): RecordEnvelope[] {
  return [
    envelope(1, { id: 1, prompt: '你好' }),
    envelope({ runId: 1, seq: 0 }, { eventKey: '0x1', runId: 1 }),
    ...Array.from({ length: PAGE_SIZE - 2 + 10 }, (_, index) =>
      envelope(2 + index, { id: 2 + index }),
    ),
  ];
}

// ---------------------------------------------------------------------------
// IPC 替身装置：按命令名分发，记录调用序列
// ---------------------------------------------------------------------------

/** mock.calls 中某命令的调用次数。 */
function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

/** mock.calls 中某命令最近一次的参数。 */
function lastParamsOf(command: string): Record<string, unknown> | undefined {
  const last = invokeMock.mock.calls.filter(([name]) => name === command).at(-1);
  return last?.[1] as Record<string, unknown> | undefined;
}

/** 按命令名分发：db_models 恒回清单，db_records 按 offset/limit 切库存（仅
 * stockedModel 命中库存，其余模型回空——选中即按模型取各自记录）。 */
function mockIpc(inventory: RecordEnvelope[] = [], stockedModel = 'workspace') {
  invokeMock.mockImplementation(
    (command: string, params?: { model?: string; offset?: number; limit?: number }) => {
      if (command === 'db_models') {
        return Promise.resolve(MODELS.map((model) => ({ ...model })));
      }
      if (command === 'db_records') {
        if (params?.model !== stockedModel) return Promise.resolve([]);
        const offset = params?.offset ?? 0;
        const limit = params?.limit ?? PAGE_SIZE;
        return Promise.resolve(inventory.slice(offset, offset + limit).map((e) => ({ ...e })));
      }
      return Promise.resolve([]);
    },
  );
}

/** 点击模型行（以 data-model 精确寻址）并等待记录区呈现（空态或记录行）。 */
async function selectModelAndWait(modelName: string) {
  const target = screen
    .getAllByTestId('db-model-item')
    .find((item) => item.getAttribute('data-model') === modelName);
  if (!target) throw new Error(`模型行 ${modelName} 不在清单中`);
  fireEvent.click(target);
  await waitFor(() => {
    if (screen.queryByTestId('db-records-empty') !== null) return;
    if (screen.getAllByTestId('db-record-item').length > 0) return;
    throw new Error('记录区尚未呈现（空态或记录行）');
  });
}

/** 真实组合进入「清单已载 + 模型已选中 + 记录已落页」呈现态。 */
async function renderWithModelSelected(inventory: RecordEnvelope[], modelName = 'workspace') {
  mockIpc(inventory);
  render(<DbInspectorView root={ROOT} />);
  await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
  await selectModelAndWait(modelName);
}

// ---------------------------------------------------------------------------
// 数据模型区：清单 + 计数 + loading / 错误形态
// ---------------------------------------------------------------------------

describe('DbInspectorView：数据模型', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('models 数据渲染清单：每行模型名 + 计数，无 loading 形态', async () => {
    mockIpc();
    render(<DbInspectorView root={ROOT} />);

    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    expect(screen.getByText('数据模型') !== null).toBe(true);
    const items = screen.getAllByTestId('db-model-item');
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toContain('workspace');
    expect(items[0].textContent).toContain('2');
    expect(items[1].textContent).toContain('agent_run');
    expect(items[1].textContent).toContain('0');
    // 未选中模型：记录区不挂载（真实 hook：selected=null 不发 db_records）
    expect(screen.queryByTestId('db-record-item')).toBeNull();
    expect(countOf('db_records')).toBe(0);
    // 清单取数经 IPC 边界：挂载即携默认 scope（workspace 库）与 root
    expect(invokeMock).toHaveBeenCalledWith('db_models', WS_PARAMS);
  });

  it('loading 态呈现进行中形态且刷新按钮禁用，清单解析后复位', async () => {
    let resolveModels: (models: ModelInfo[]) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') {
        return new Promise<ModelInfo[]>((resolve) => {
          resolveModels = resolve;
        });
      }
      return Promise.resolve([]);
    });
    render(<DbInspectorView root={ROOT} />);

    // 挂载取数进行中：loading 置位、刷新禁用
    expect(screen.getByTestId('db-models-loading') !== null).toBe(true);
    expect(screen.getByTestId('db-refresh').hasAttribute('disabled')).toBe(true);

    await act(async () => {
      resolveModels([...MODELS]);
    });
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    expect(screen.getByTestId('db-refresh').hasAttribute('disabled')).toBe(false);
  });

  it('点击模型行 → 以该模型名发起记录取数（计数 0 的模型仍可选）', async () => {
    mockIpc();
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());

    fireEvent.click(screen.getAllByTestId('db-model-item')[1]); // agent_run（计数 0）

    // 选择语义经 IPC 面可观测：db_records 携 (scope, root, model, offset:0, limit)
    await waitFor(() =>
      expect(lastParamsOf('db_records')).toEqual({
        ...WS_PARAMS,
        model: 'agent_run',
        offset: 0,
        limit: PAGE_SIZE,
      }),
    );
  });

  it('计数为 0 的模型选中后：空态呈现而非错误（desktop-db-inspector「清单与计数」）', async () => {
    mockIpc();
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());

    await selectModelAndWait('agent_run');

    expect(screen.getByTestId('db-records-empty') !== null).toBe(true);
    expect(screen.getByTestId('db-records-empty').textContent).toContain('暂无记录');
    expect(screen.queryByTestId('db-inspector-error')).toBeNull();
    expect(screen.queryByTestId('db-record-item')).toBeNull();
    // 记录区标题以「所选模型名」呈现（agent_run 为清单第二项，非 find 首项）
    expect(screen.getByRole('heading', { name: 'agent_run' }) !== null).toBe(true);
  });

  it('模型行 className 精确匹配：仅选中行带 font-medium text-primary，非选中行为基线', async () => {
    mockIpc();
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    await selectModelAndWait('workspace');

    const items = screen.getAllByTestId('db-model-item');
    expect(items[0].className).toBe(
      'block w-full cursor-pointer border-0 bg-transparent px-0 py-1.5 text-left hover:text-primary font-medium text-primary',
    );
    expect(items[1].className).toBe(
      'block w-full cursor-pointer border-0 bg-transparent px-0 py-1.5 text-left hover:text-primary ',
    );
  });

  it('页级错误（error 置位）：inline 错误区呈现且页面不白屏（失败清单为空）', async () => {
    invokeMock.mockRejectedValue(new Error('db: 清单打开失败'));
    render(<DbInspectorView root={ROOT} />);

    const errorNote = await screen.findByTestId('db-inspector-error');
    expect(errorNote.textContent).toContain('模型加载失败');
    expect(errorNote.textContent).toContain('db: 清单打开失败');
    // 页面不白屏：模型区仍在（真实 hook：清单轨失败 models 复位空数组）
    expect(screen.getByTestId('db-model-list') !== null).toBe(true);
    expect(screen.queryByTestId('db-model-item')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 记录分页区：信封行、翻页控件边界、加载形态（整页库存驱动真实翻页链）
// ---------------------------------------------------------------------------

describe('DbInspectorView：分页扫描与翻页控件', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('选中模型后记录区按信封行呈现：key 预览 + value 摘要，整页 hasMore 驱动翻页控件', async () => {
    await renderWithModelSelected(pagedInventory());

    const rows = screen.getAllByTestId('db-record-item');
    expect(rows).toHaveLength(PAGE_SIZE); // 60 条库存切第 0 页整 50 条
    expect(rows[0].textContent).toContain('key: 1');
    expect(rows[0].textContent).toContain('{"id":1,"prompt":"你好"}');
    expect(rows[1].textContent).toContain('key: {"runId":1,"seq":0}');
    // 有记录时既不呈空态也不呈错误区（空态 / 错误 / 记录三分支互斥）
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
    expect(screen.queryByText(/记录扫描失败/)).toBeNull();
    // 控件边界：首页 prev 禁用、整页 hasMore=true 时 next 可用
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(false);
    expect(screen.getByText('offset 0') !== null).toBe(true);
  });

  it('记录行 className 精确匹配：仅被点开的行带 text-primary，关闭行保持基线', async () => {
    await renderWithModelSelected(pagedInventory());

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

  it('value 摘要按 120 字符边界截断：短值原样、恰 120 不截、121 截断加省略号', async () => {
    const exact120 = JSON.stringify({ pad: 'x'.repeat(110) }); // 恰 120 字符
    const over = JSON.stringify({ pad: 'y'.repeat(111) }); // 121 字符
    expect(exact120).toHaveLength(120);
    expect(over).toHaveLength(121);
    await renderWithModelSelected([
      envelope(1, { id: 1 }),
      envelope(2, JSON.parse(exact120) as Record<string, unknown>),
      envelope(3, JSON.parse(over) as Record<string, unknown>),
    ]);

    const rows = screen.getAllByTestId('db-record-item');
    expect(rows[0].textContent).toBe('key: 1{"id":1}'); // 短值原样、无省略号
    expect(rows[1].textContent).toBe(`key: 2${exact120}`); // 恰等于 max：不截断
    expect(rows[2].textContent).toBe(`key: 3${over.slice(0, 120)}…`); // 超出：截断 + 省略号
  });

  it('翻页控件按 offset / hasMore 边界启用或禁用：整页库存下 next/prev 真实翻页', async () => {
    // 库存恰一整页：第 0 页 hasMore=true，第 1 页空 → hasMore=false
    mockIpc(Array.from({ length: PAGE_SIZE }, (_, index) => envelope(index, { id: index })));
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    await selectModelAndWait('workspace');

    // 首页：prev 禁用、next 可用
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(false);
    expect(screen.getByText('offset 0') !== null).toBe(true);

    fireEvent.click(screen.getByTestId('db-page-next'));
    // 第 1 页空（库存恰一整页）：取数完成空态呈现后，offset 50、hasMore=false
    await waitFor(() => expect(screen.getByTestId('db-records-empty') !== null).toBe(true));
    expect(screen.getByText('offset 50') !== null).toBe(true);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(true);
    expect(lastParamsOf('db_records')).toEqual({
      ...WS_PARAMS,
      model: 'workspace',
      offset: 50,
      limit: PAGE_SIZE,
    });
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(false);
    expect(screen.getByTestId('db-records-empty') !== null).toBe(true);

    fireEvent.click(screen.getByTestId('db-page-prev'));
    await waitFor(() => expect(screen.getAllByTestId('db-record-item')).toHaveLength(PAGE_SIZE));
    expect(lastParamsOf('db_records')).toEqual({
      ...WS_PARAMS,
      model: 'workspace',
      offset: 0,
      limit: PAGE_SIZE,
    });
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
  });

  it('recordsLoading 呈现进行中形态且翻页控件禁用', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return new Promise<RecordEnvelope[]>(() => {}); // 记录取数在途
    });
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());

    fireEvent.click(screen.getAllByTestId('db-model-item')[0]); // workspace
    await waitFor(() => expect(screen.getByTestId('db-records-loading') !== null).toBe(true));

    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(true);
    // loading 中不呈空态（避免误报「暂无记录」）
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 单条 JSON 查看
// ---------------------------------------------------------------------------

describe('DbInspectorView：单条 JSON 查看', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('点开单条记录：以 pretty JSON 完整呈现 key 与 value，字段名可读', async () => {
    const record = envelope(
      { runId: 1, seq: 3 },
      { eventKey: '0x10000000000000003', runId: 1, event: { seq: 3, kind: 'raw' } },
    );
    await renderWithModelSelected([record]);

    fireEvent.click(screen.getByTestId('db-record-item'));

    const detail = screen.getByTestId('db-record-json');
    expect(detail.textContent).toBe(JSON.stringify(record, null, 2));
    expect(detail.textContent).toContain('"eventKey"');
    expect(detail.textContent).toContain('"runId"');
    expect(detail.textContent).toContain('0x10000000000000003');
    expect(screen.getByText(/记录详情（信封 JSON）/) !== null).toBe(true);
  });

  it('换模型 / 换页收起单条详情（记录列表换血详情不残留）', async () => {
    await renderWithModelSelected(pagedInventory());

    fireEvent.click(screen.getAllByTestId('db-record-item')[0]);
    expect(screen.getByTestId('db-record-detail') !== null).toBe(true);

    // 换页（offset 变化）：记录列表换血，详情收起不残留
    fireEvent.click(screen.getByTestId('db-page-next'));
    await waitFor(() => expect(screen.queryByTestId('db-record-detail')).toBeNull());
    await waitFor(() =>
      expect(screen.getAllByTestId('db-record-item')[0].textContent).toContain('key: 50'),
    );

    // 换模型（selected 变化）：详情同样收起
    fireEvent.click(screen.getAllByTestId('db-record-item')[0]);
    expect(screen.getByTestId('db-record-detail') !== null).toBe(true);
    fireEvent.click(screen.getAllByTestId('db-model-item')[1]); // agent_run（计数 0 → 空态）
    await waitFor(() => expect(screen.queryByTestId('db-record-detail')).toBeNull());
    expect(screen.getByTestId('db-records-empty') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 错误态与只读边界（查询轨 error 态回归）
// ---------------------------------------------------------------------------

describe('DbInspectorView：错误态与只读边界', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('recordsError 置位：inline 持久错误区呈现、页面不白屏、已有清单不丢失、无 toast 调用', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return Promise.reject(new Error('db: 未知模型: nope'));
    });
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    fireEvent.click(screen.getAllByTestId('db-model-item')[0]);

    expect((await screen.findByText(/记录扫描失败：Error: db: 未知模型: nope/)) !== null).toBe(
      true,
    );
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

  it('只读边界：渲染树可交互控件仅选择、翻页、刷新类，无任何写操作入口', async () => {
    await renderWithModelSelected(pagedInventory());

    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBeGreaterThan(0);
    // scope 双 tab 为只读切换（只改查询寻址，不写库），与刷新 / 翻页同类
    const readonlyActions = ['刷新数据', '上一页', '下一页', 'workspace 库', '全局库'];
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

  it('模型行与记录行均可点选（选择类控件语义）', async () => {
    await renderWithModelSelected([envelope(1, { id: 1 })]);

    expect(within(screen.getByTestId('db-model-list')).getAllByRole('button')).toHaveLength(2);
    // 模型行点选已由装置点击承载：db_records 携所选模型名与 root
    expect(lastParamsOf('db_records')).toMatchObject({ model: 'workspace', root: ROOT });
    // 记录行点开单条详情
    fireEvent.click(screen.getByTestId('db-record-item'));
    expect(screen.getByTestId('db-record-detail') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// scope 双 tab（desktop-workspace-db-split）：全局库 / workspace 库切换按钮组
// （aria-pressed 同 agent 页切换行模式），默认 workspace 库。真实组合
// useDbInspector：切换经 hook setScope 派发，重置语义（清单以新 scope 重取、
// 选中模型复位 / 记录区卸载）经 invoke 面与渲染树联动断言，不以替身 state
// 注入降级。
// ---------------------------------------------------------------------------

describe('DbInspectorView：scope 双 tab（全局库 / workspace 库）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('渲染双 tab 按钮组：默认 workspace 库 aria-pressed=true、全局库 aria-pressed=false', async () => {
    mockIpc();
    render(<DbInspectorView root={ROOT} />);

    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    expect(screen.getByTestId('db-scope-workspace').getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByTestId('db-scope-user').getAttribute('aria-pressed')).toBe('false');
  });

  it('点击全局库 tab：清单以 user scope 经 IPC 重取、选中模型复位（记录区卸载）、aria-pressed 互换', async () => {
    // 分发替身按 scope 回不同清单（store 双库模型分组口径）：workspace 库三行、全局库仅 workspace 行
    invokeMock.mockImplementation(
      (command: string, params?: { scope?: string; offset?: number; limit?: number }) => {
        if (command === 'db_models') {
          return Promise.resolve(
            params?.scope === 'user'
              ? [{ name: 'workspace', count: 2 }]
              : [
                  { name: 'agent_run', count: 1 },
                  { name: 'agent_event', count: 0 },
                  { name: 'explore', count: 0 },
                ],
          );
        }
        if (command === 'db_records') {
          const offset = params?.offset ?? 0;
          const limit = params?.limit ?? PAGE_SIZE;
          return Promise.resolve([envelope(1, { id: 1 })].slice(offset, offset + limit));
        }
        return Promise.resolve([]);
      },
    );
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    expect(countOf('db_models')).toBe(1);
    expect(invokeMock).toHaveBeenCalledWith('db_models', WS_PARAMS);
    await selectModelAndWait('agent_run');
    expect(screen.getByTestId('db-record-item') !== null).toBe(true);

    fireEvent.click(screen.getByTestId('db-scope-user'));

    // hook 重置语义联动（真实组合，非替身 state）：清单以新 scope 重取
    await waitFor(() => expect(lastParamsOf('db_models')).toEqual({ scope: 'user', root: ROOT }));
    expect(countOf('db_models')).toBe(2);
    expect(screen.getByTestId('db-scope-user').getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByTestId('db-scope-workspace').getAttribute('aria-pressed')).toBe('false');
    // 选中模型复位：记录区卸载（行与空态均不残留）
    await waitFor(() => expect(screen.queryByTestId('db-record-item')).toBeNull());
    // 两库清单互不混列可观测：全局库清单重取后仅 workspace 一行
    await waitFor(() => expect(screen.getAllByTestId('db-model-item')).toHaveLength(1));
    expect(screen.getAllByTestId('db-model-item')[0].getAttribute('data-model')).toBe('workspace');
    // 静置复核：未选中状态下记录区不复活（scope 切换在位响应被取消守卫丢弃，
    // 记录区保持卸载形态稳定）
    await act(async () => {});
    expect(screen.queryByTestId('db-record-item')).toBeNull();
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
    expect(screen.getByTestId('db-model-list') !== null).toBe(true);
  });

  it('空态与 inline 持久错误态不随 scope 切换变形', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return Promise.reject(new Error('db: 未知模型: nope'));
    });
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    fireEvent.click(screen.getAllByTestId('db-model-item')[1]); // agent_run
    expect((await screen.findByText(/记录扫描失败：Error: db: 未知模型: nope/)) !== null).toBe(
      true,
    );
    expect(screen.queryByTestId('db-records-empty')).toBeNull(); // 错误优先于空态

    // scope 切换：选中复位 → 记录区卸载，错误不残留、空态不误报
    fireEvent.click(screen.getByTestId('db-scope-user'));
    await waitFor(() =>
      expect(screen.getByTestId('db-scope-user').getAttribute('aria-pressed')).toBe('true'),
    );
    expect(screen.queryByText(/记录扫描失败/)).toBeNull();
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
    expect(screen.getAllByTestId('db-model-item')).toHaveLength(2);

    // 新 scope 下同口径重建：同一 inline 错误形态（查询轨 error 态回归）
    fireEvent.click(screen.getAllByTestId('db-model-item')[1]);
    expect((await screen.findByText(/记录扫描失败：Error: db: 未知模型: nope/)) !== null).toBe(
      true,
    );
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
    // 查询轨错误呈现不走 toast（inline 持久双轨）
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(toastInfoMock).not.toHaveBeenCalled();
  });
});
