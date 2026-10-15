// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ModelInfo, RecordEnvelope, RecordsPage } from '../../types/dto';
import { DbInspectorView } from './db-inspector-view';

// ---------------------------------------------------------------------------
// DbInspectorView 单测（最小 mock 原则，沿 agent-debug-view.test.tsx 模式）：
// useDbInspector 为进程内内部协作者，不 mock——组件真实组合 hook，唯一 mock
// 点为 @tauri-apps/api/core 进程边界：invoke 按命令名分发替身（db_models /
// db_records，固定 fixture 并记录调用序列，断言 scope / root 入参）。hook 重置
// 语义（scope 切换 → 清单重取 + 选中 / 分页复位）经真实组合的 invoke 面可观测，
// 不以替身 state 注入降级。sonner 以 spy mock 注入：查询轨错误呈现断言「无任何
// toast 调用」（错误 inline 双轨语义）。只读四件套：清单 + 计数、分页自动表格、
// 单条抽屉详情、空/错误态。
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

// value 允许退化形态（null / 数组 / 标量）：AC-2 的非对象 value 行覆盖
function envelope(key: unknown, value: unknown): RecordEnvelope {
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
 * stockedModel 命中库存，其余模型回空信封——选中即按模型取各自记录）；回
 * 分页信封（records 区间 + total = 库存全量计数）。 */
function mockIpc(inventory: RecordEnvelope[] = [], stockedModel = 'workspace') {
  invokeMock.mockImplementation(
    (command: string, params?: { model?: string; offset?: number; limit?: number }) => {
      if (command === 'db_models') {
        return Promise.resolve(MODELS.map((model) => ({ ...model })));
      }
      if (command === 'db_records') {
        if (params?.model !== stockedModel) return Promise.resolve({ records: [], total: 0 });
        const offset = params?.offset ?? 0;
        const limit = params?.limit ?? PAGE_SIZE;
        const records = inventory.slice(offset, offset + limit).map((e) => ({ ...e }));
        return Promise.resolve({ records, total: inventory.length });
      }
      return Promise.resolve([]);
    },
  );
}

/** 记录区表格：`db-records-table` 容器内的 <table>（shadcn Table 结构）。
 * 抽屉 modal 打开时 base-ui 将背景页置 aria-hidden（inert）——「抽屉保留现状」类
 * 断言须在开抽屉状态下读取背景列表，故角色查询显式含隐藏节点；关闭态语义不变。 */
function recordsTable(): HTMLElement {
  return screen.getByRole('table', { hidden: true });
}

/** 表头列名序列（key 恒首列，后见键补尾列）。 */
function headerColumns(): string[] {
  return within(recordsTable())
    .getAllByRole('columnheader', { hidden: true })
    .map((cell) => cell.textContent ?? '');
}

/** 数据行序列（不含表头行）。 */
function bodyRows(): HTMLElement[] {
  return within(recordsTable()).getAllByRole('row', { hidden: true }).slice(1);
}

/** 某行各单元格文本（与列位一一对齐，缺失键为空串）。 */
function rowCells(row: HTMLElement): string[] {
  return within(row)
    .getAllByRole('cell', { hidden: true })
    .map((cell) => cell.textContent ?? '');
}

/** 点击模型行（以 data-model 精确寻址）并等待记录区呈现（空态或记录表格）。 */
async function selectModelAndWait(modelName: string) {
  const target = screen
    .getAllByTestId('db-model-item')
    .find((item) => within(item).queryByText(modelName, { exact: false }) !== null);
  if (!target) throw new Error(`模型行 ${modelName} 不在清单中`);
  fireEvent.click(target);
  await waitFor(() => {
    if (screen.queryByTestId('db-records-empty') !== null) return;
    if (screen.queryByTestId('db-records-table') !== null) return;
    throw new Error('记录区尚未呈现（空态或记录表格）');
  });
}

/** 真实组合进入「清单已载 + 模型已选中 + 记录已落页」呈现态。 */
async function renderWithModelSelected(inventory: RecordEnvelope[], modelName = 'workspace') {
  cleanup();
  mockIpc(inventory);
  render(<DbInspectorView root={ROOT} />);
  await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
  await selectModelAndWait(modelName);
}

/** 抽屉标题文本（`data-slot="drawer-title"` 定位，避开区块级 h2 标题）。 */
function drawerTitle(): string | null {
  return document.querySelector('[data-slot="drawer-title"]')?.textContent ?? null;
}

/** 分页控件行文案：「第 {page} / {totalPages} 页（共 {total} 条）」。 */
function pageInfoText(): string {
  return screen.getByText(/第 \d+ \/ \d+ 页（共 \d+ 条）/).textContent ?? '';
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
    expect(screen.queryByTestId('db-records-table')).toBeNull();
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
    expect(screen.queryByTestId('db-records-table')).toBeNull();
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
// 模型驱动自动表格（AC-1）：key 恒首列 + value 顶层键并集首见稳定序 + 缺失键
// 空单元格 + 非对象 value 退化 value 尾列；零模型特定分支
// ---------------------------------------------------------------------------

describe('DbInspectorView：模型驱动自动表格', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('正向：记录区呈 shadcn Table 结构，表头为 key + value 顶层键并集（首见稳定序）', async () => {
    await renderWithModelSelected(pagedInventory());

    // shadcn ui/table 组件族真实渲染：table / thead / tbody / tr / th / td 结构在场
    const table = recordsTable();
    expect(table.tagName).toBe('TABLE');
    expect(within(table).getAllByRole('columnheader').length).toBeGreaterThan(0);
    expect(within(table).getAllByRole('row').length).toBeGreaterThan(1);

    // 首行 value 顶层键 {id, prompt}；次行 {eventKey, runId} 后见键补尾列；
    // 其余行同键不重复追加 → 并集首见稳定序，key 恒首列
    expect(headerColumns()).toEqual(['key', 'id', 'prompt', 'eventKey', 'runId']);

    // 各行单元格与列位对齐（缺失键空单元格）
    const rows = bodyRows();
    expect(rows).toHaveLength(PAGE_SIZE); // 60 条库存切第 0 页整 50 条
    expect(rowCells(rows[0])).toEqual(['1', '1', '"你好"', '', '']);
    expect(rowCells(rows[1])).toEqual(['{"runId":1,"seq":0}', '', '', '"0x1"', '1']);
    expect(rowCells(rows[2])).toEqual(['2', '2', '', '', '']);

    // 有记录时既不呈空态也不呈错误区（空态 / 错误 / 记录三分支互斥）
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
    expect(screen.queryByText(/记录扫描失败/)).toBeNull();
    // 控件边界：首页 prev 禁用、整页 hasMore=true 时 next 可用
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(false);
  });

  it('正向：零模型特定分支——换一组顶层键的 fixture 出表无需改组件', async () => {
    // 未在任何既有用例出现过的模型键形：列由数据驱动推导，非硬编码
    await renderWithModelSelected([
      envelope('a', { alpha: 1, beta: 'x' }),
      envelope('b', { gamma: true, alpha: 2 }),
    ]);

    expect(headerColumns()).toEqual(['key', 'alpha', 'beta', 'gamma']);
    const rows = bodyRows();
    expect(rowCells(rows[0])).toEqual(['"a"', '1', '"x"', '']);
    expect(rowCells(rows[1])).toEqual(['"b"', '2', '', 'true']);
  });

  it('异常：value 顶层非对象行（null / 数组 / 标量）退化为 value 尾列承载摘要', async () => {
    await renderWithModelSelected([
      envelope(1, { id: 1 }),
      envelope(2, null),
      envelope(3, [1, 2, 3]),
      envelope(4, '文本'),
      envelope(5, 42),
    ]);

    // 对象行贡献 id 列；非对象行触发 value 尾列（全非对象页则恰 key + value 两列）
    expect(headerColumns()).toEqual(['key', 'id', 'value']);
    const rows = bodyRows();
    expect(rowCells(rows[0])).toEqual(['1', '1', '']); // 对象行：value 尾列空单元格
    expect(rowCells(rows[1])).toEqual(['2', '', 'null']);
    expect(rowCells(rows[2])).toEqual(['3', '', '[1,2,3]']);
    expect(rowCells(rows[3])).toEqual(['4', '', '"文本"']);
    expect(rowCells(rows[4])).toEqual(['5', '', '42']);
  });

  it('异常：全非对象页恰 key + value 两列；混合页并集列与 value 尾列并存不串位', async () => {
    // 全非对象页：无对象行贡献字段列 → 恰 key + value 两列
    await renderWithModelSelected([envelope(1, null), envelope(2, 7)]);
    expect(headerColumns()).toEqual(['key', 'value']);
    expect(rowCells(bodyRows()[0])).toEqual(['1', 'null']);
    expect(rowCells(bodyRows()[1])).toEqual(['2', '7']);

    // 混合页：对象行并集列在前、value 尾列并存，各行列位互不串位
    await renderWithModelSelected([
      envelope(1, { id: 1 }),
      envelope(2, '退化标量'),
      envelope(3, { id: 3, name: 'n' }),
    ]);
    expect(headerColumns()).toEqual(['key', 'id', 'name', 'value']);
    const rows = bodyRows();
    expect(rowCells(rows[0])).toEqual(['1', '1', '', '']);
    expect(rowCells(rows[1])).toEqual(['2', '', '', '"退化标量"']);
    expect(rowCells(rows[2])).toEqual(['3', '3', '"n"', '']);
  });

  it('边界：空对象 value 不贡献列；对象键恰为 value 与退化列同名合并（DD3）', async () => {
    // value = {}（空对象）：不贡献任何顶层键，仅 key 列
    await renderWithModelSelected([envelope(1, {}), envelope(2, { id: 2 })]);
    expect(headerColumns()).toEqual(['key', 'id']);
    expect(rowCells(bodyRows()[0])).toEqual(['1', '']); // 缺失键空单元格

    // 对象键恰为 "value"：与非对象行退化列同名合并，不重复追加第二列
    await renderWithModelSelected([
      envelope(1, { value: '对象内同名键' }),
      envelope(2, '退化行整值'),
    ]);
    expect(headerColumns()).toEqual(['key', 'value']);
    expect(rowCells(bodyRows()[0])).toEqual(['1', '"对象内同名键"']);
    expect(rowCells(bodyRows()[1])).toEqual(['2', '"退化行整值"']);
  });

  it('边界：records 空 / 单元素 / 超大页（PAGE_SIZE 50 行）渲染不炸', async () => {
    // 空：空态分支（表格不挂载）
    await renderWithModelSelected([]);
    expect(screen.queryByTestId('db-records-table')).toBeNull();
    expect(screen.getByTestId('db-records-empty') !== null).toBe(true);

    // 单元素
    await renderWithModelSelected([envelope(1, { id: 1 })]);
    expect(bodyRows()).toHaveLength(1);
    expect(headerColumns()).toEqual(['key', 'id']);

    // 超大页：整 50 行渲染不炸
    await renderWithModelSelected(
      Array.from({ length: PAGE_SIZE }, (_, index) => envelope(index, { id: index })),
    );
    expect(bodyRows()).toHaveLength(PAGE_SIZE);
  });
});

// ---------------------------------------------------------------------------
// 单元格摘要与退化（AC-2）：120 字截断口径 + key 列同口径 + JSON 不可序列化
// 形态退化
// ---------------------------------------------------------------------------

describe('DbInspectorView：单元格摘要与退化', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('正向：短于 120 原样；特殊字符经 JSON 转义呈单行', async () => {
    await renderWithModelSelected([
      envelope(1, { id: 1, prompt: '短摘要' }),
      envelope(2, { id: 2, note: '第一行\n第二行\t含\t制表' }),
    ]);

    const rows = bodyRows();
    // 首行先见 prompt 列、次行后见 note 列补尾（首见稳定序）
    expect(headerColumns()).toEqual(['key', 'id', 'prompt', 'note']);
    // 短值原样呈现（无省略号）
    expect(rowCells(rows[0])).toEqual(['1', '1', '"短摘要"', '']);
    // 含换行 / 制表的字符串经 JSON.stringify 转义为字面量 → 摘要恒单行
    const noteCell = rowCells(rows[1])[3];
    expect(noteCell).toBe(JSON.stringify('第一行\n第二行\t含\t制表'));
    expect(noteCell).not.toContain('\n');
  });

  it('正向：嵌套对象 value 摘要为压缩 JSON 单行（字段名可读）', async () => {
    await renderWithModelSelected([
      envelope(1, { id: 1, payload: { event: { seq: 3, kind: 'raw' }, tags: ['a'] } }),
    ]);

    const cell = rowCells(bodyRows()[0])[2];
    expect(cell).toBe(JSON.stringify({ event: { seq: 3, kind: 'raw' }, tags: ['a'] }));
    expect(cell).not.toContain('\n'); // 压缩形态：无换行
    expect(cell).toContain('"seq"'); // 字段名可读
    expect(cell).toContain('"kind"');
  });

  it('正向：key 列同口径呈主键摘要并按 120 截断', async () => {
    const longKey = `k${'x'.repeat(130)}`; // 131 字符
    await renderWithModelSelected([
      envelope(1, { id: 1 }),
      envelope({ runId: 1, seq: 0 }, { id: 2 }),
      envelope(longKey, { id: 3 }),
    ]);

    const rows = bodyRows();
    // key 列：数字键 / 对象键按单行 JSON 摘要呈现
    expect(rowCells(rows[0])[0]).toBe('1');
    expect(rowCells(rows[1])[0]).toBe('{"runId":1,"seq":0}');
    // 超长 key 同口径 120 截断 + 省略号（JSON 形态带引号，截断作用于整段 JSON 文本）
    expect(rowCells(rows[2])[0]).toBe(`"${longKey.slice(0, 119)}…`);
  });

  it('异常：value 顶层不可序列化形态（undefined）摘要退化为空串不抛错', async () => {
    await renderWithModelSelected([
      envelope(1, { id: 1, missing: undefined }),
      envelope(2, { id: 2, missing: undefined }),
    ]);

    expect(headerColumns()).toEqual(['key', 'id', 'missing']);
    const rows = bodyRows();
    // 键在场但值 undefined → JSON.stringify 返 undefined → valueSummary 退化空串
    expect(rowCells(rows[0])[2]).toBe('');
    expect(rowCells(rows[1])[2]).toBe('');
  });

  it('边界：120 字符摘要恰 120 不截、121 截断加省略号', async () => {
    const exact120 = 'x'.repeat(120); // 恰 120 字符的字段值
    const over121 = 'y'.repeat(121); // 121 字符的字段值
    await renderWithModelSelected([envelope(1, { pad: exact120 }), envelope(2, { pad: over121 })]);

    const rows = bodyRows();
    // 单元格摘要 = 字段值的 JSON 形态（带引号），截断作用于整段 JSON 文本
    // （120 + 省略号），故带引号后恰 120 字符的字段值亦在截断线上
    expect(rowCells(rows[0])[0]).toBe('1');
    expect(rowCells(rows[0])[1]).toBe(`"${exact120.slice(0, 119)}…`);
    expect(rowCells(rows[0])[1]).toHaveLength(121);
    expect(rowCells(rows[1])[1]).toBe(`"${over121.slice(0, 119)}…`);
    expect(rowCells(rows[1])[1]).toHaveLength(121);
  });

  it('边界：超长 value（>1000 字符）截断稳定且空对象摘要为 {}', async () => {
    const longText = 'z'.repeat(1500);
    await renderWithModelSelected([envelope(1, { blob: longText }), envelope(2, {})]);

    const rows = bodyRows();
    // 超长字段值截断稳定：JSON 摘要恰 121 字符（120 字 + 省略号）且以省略号收尾
    const blobCell = rowCells(rows[0])[1];
    expect(blobCell).toHaveLength(121);
    expect(blobCell.endsWith('…')).toBe(true);
    // 空对象行不贡献列（列并集仅 blob），该行 blob 列为空单元格
    expect(rowCells(rows[1])[1]).toBe('');
  });
});

// ---------------------------------------------------------------------------
// 抽屉单条详情（AC-5）+ 列表换血收起（AC-6）：点行选中 + StandardDrawer 右侧
// 滑出，title = key JSON、content = key+value 信封 pretty JSON、footer 恒「关闭」；
// 换模型 / 翻页 / 切 scope / 刷新成功即收起，详情不残留
// ---------------------------------------------------------------------------

describe('DbInspectorView：抽屉单条详情与换血收起', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('正向：点表格行 → 行选中态 + 抽屉滑出（title = key JSON、content = 信封 pretty JSON、footer 恒「关闭」）', async () => {
    const record = envelope(
      { runId: 1, seq: 3 },
      { eventKey: '0x10000000000000003', runId: 1, event: { seq: 3, kind: 'raw' } },
    );
    await renderWithModelSelected([record]);

    // 行选中态（data-state="selected"）：点行后按现行首行重查断言——门禁高
    // 并行下列表重渲染可能换行节点（捕获节点引用会在换血后指向游离节点，5s
    // 等待预算内恒 null），断言用户可见结果而非节点身份
    fireEvent.click(bodyRows()[0]);
    await waitFor(() => expect(bodyRows()[0].getAttribute('data-state')).toBe('selected'));
    // 抽屉右侧滑出：title = key JSON 文本
    await waitFor(() => expect(drawerTitle()).toBe(JSON.stringify(record.key)));
    // content = key + value 信封 pretty JSON
    const detail = await screen.findByTestId('db-record-json');
    expect(detail.textContent).toBe(JSON.stringify(record, null, 2));
    expect(detail.textContent).toContain('"eventKey"');
    expect(detail.textContent).toContain('0x10000000000000003');
    // footer 恒「关闭」（弹层内容容器内恰一枚按钮且为「关闭」）
    const shell = document.querySelector('[data-slot="drawer-content"]');
    expect(shell).not.toBeNull(); // 抽屉内容容器在场（footer 恒单按钮断言前置）
    expect(Array.from(shell?.querySelectorAll('button') ?? []).map((b) => b.textContent)).toEqual([
      '关闭',
    ]);
  });

  it('正向：同页点他行即换详情（单实例受控，title 与 content 同步换血）', async () => {
    const first = envelope(1, { id: 1, name: '第一条' });
    const second = envelope(2, { id: 2, name: '第二条' });
    await renderWithModelSelected([first, second]);

    // 抽屉开合使页面 inert（base-ui Portal）：先取行引用再点开
    const [firstRow, secondRow] = bodyRows();
    fireEvent.click(firstRow);
    await waitFor(() => expect(drawerTitle()).toBe(JSON.stringify(first.key)));

    fireEvent.click(secondRow);
    await waitFor(() => expect(drawerTitle()).toBe(JSON.stringify(second.key)));
    expect(drawerTitle()).not.toBe(JSON.stringify(first.key));
    const detail = screen.getByTestId('db-record-json');
    expect(detail.textContent).toBe(JSON.stringify(second, null, 2));
  });

  it('异常：点「关闭」→ 抽屉退场、行选中清除', async () => {
    await renderWithModelSelected([envelope(1, { id: 1 })]);

    const firstRow = bodyRows()[0];
    fireEvent.click(firstRow);
    await waitFor(() => expect(drawerTitle()).toBe('1'));

    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    await waitFor(() => expect(drawerTitle()).toBeNull());
    await waitFor(() => expect(firstRow.getAttribute('data-state')).toBeNull());
  });

  it('异常：刷新失败（recordsError）不换血 → 抽屉保留现状、详情不越出当前列表', async () => {
    await renderWithModelSelected(pagedInventory());

    fireEvent.click(bodyRows()[0]);
    await waitFor(() => expect(drawerTitle()).toBe('1'));
    const detailBefore = screen.getByTestId('db-record-json').textContent;

    // 刷新失败：db_records reject（db_models 清单轨不受影响）
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return Promise.reject(new Error('db: workspace 库打开失败'));
    });
    fireEvent.click(screen.getByTestId('db-refresh'));

    // 错误 inline 呈现（查询轨错误不走 toast——双轨语义）
    expect(
      (await screen.findByText(/记录扫描失败：Error: db: workspace 库打开失败/)) !== null,
    ).toBe(true);
    expect(toastErrorMock).not.toHaveBeenCalled();

    // 抽屉保留现状：records 快照不换血 → 收起 effect deps（selected / offset /
    // records 引用）未变，开合态、title、content 均不被失败响应顶替；行选中态
    // 按现行首行重查（捕获节点引用不耐列表重渲染换血——同上）
    expect(drawerTitle()).toBe('1');
    expect(screen.getByTestId('db-record-json').textContent).toBe(detailBefore);
    expect(bodyRows()[0].getAttribute('data-state')).toBe('selected');
    // 详情不越出当前列表：抽屉 title 对应记录仍在当前渲染行 key 列中
    // （旧页数据未被半残响应清除，详情不指向不存在的记录）
    expect(bodyRows().map((row) => rowCells(row)[0])).toContain(drawerTitle());
    // 旧页记录与页界文案不被顶替（records / total 快照保留）
    expect(bodyRows()).toHaveLength(PAGE_SIZE);
    expect(pageInfoText()).toBe('第 1 / 2 页（共 60 条）');
  });

  it('边界：翻页 / 刷新成功（列表换血）→ 抽屉收起，详情不残留（DD4）', async () => {
    await renderWithModelSelected(pagedInventory());

    const firstRow = bodyRows()[0];
    fireEvent.click(firstRow);
    await waitFor(() => expect(drawerTitle()).toBe('1'));

    // 翻页（offset 变化）：记录列表换血，抽屉收起不残留
    fireEvent.click(screen.getByTestId('db-page-next'));
    await waitFor(() => expect(drawerTitle()).toBeNull());
    await waitFor(() => expect(rowCells(bodyRows()[0])[0]).toBe('50'));

    // 重新开抽屉 → 刷新成功（records 引用变化）：抽屉同样收起
    const page2FirstRow = bodyRows()[0];
    fireEvent.click(page2FirstRow);
    await waitFor(() => expect(drawerTitle()).toBe('50'));
    fireEvent.click(screen.getByTestId('db-refresh'));
    await waitFor(() => expect(drawerTitle()).toBeNull());
  });

  it('边界：换模型 / 切 scope → 抽屉收起；行内无 JSON 详情块（详情仅抽屉内）', async () => {
    await renderWithModelSelected(pagedInventory());

    const firstRow = bodyRows()[0];
    fireEvent.click(firstRow);
    await waitFor(() => expect(drawerTitle()).toBe('1'));

    // 换模型（selected 变化，agent_run 计数 0 → 空态）：抽屉收起
    fireEvent.click(screen.getAllByTestId('db-model-item')[1]);
    await waitFor(() => expect(drawerTitle()).toBeNull());
    expect(screen.getByTestId('db-records-empty') !== null).toBe(true);

    // 行内无 JSON 详情块：记录区无常驻 db-record-detail / 行内 pre（详情仅抽屉）
    await renderWithModelSelected(pagedInventory());
    expect(screen.queryByTestId('db-record-detail')).toBeNull();
    const rows = bodyRows();
    for (const row of rows) {
      expect(row.querySelector('pre')).toBeNull();
    }

    // 切 scope：selected 复位 → 记录区卸载，抽屉随之退场
    fireEvent.click(screen.getByTestId('db-scope-user'));
    await waitFor(() =>
      expect(screen.getByTestId('db-scope-user').getAttribute('aria-pressed')).toBe('true'),
    );
    expect(drawerTitle()).toBeNull();
    expect(screen.queryByTestId('db-records-table')).toBeNull();
  });

  it('边界：records 空列表无行可选、抽屉不可达', async () => {
    await renderWithModelSelected([]);

    expect(screen.getByTestId('db-records-empty') !== null).toBe(true);
    expect(screen.queryByTestId('db-records-table')).toBeNull();
    expect(drawerTitle()).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 真实页界控件 + 空错态 + 只读边界（AC-4 / AC-7）：页界全部由接口 total 导出，
// 废除 records.length === PAGE_SIZE 启发式
// ---------------------------------------------------------------------------

describe('DbInspectorView：真实页界控件、空错态与只读边界', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastErrorMock.mockReset();
    toastInfoMock.mockReset();
  });

  it('正向：分页控件呈「第 {page} / {totalPages} 页（共 {total} 条）」+ 上一页 / 下一页', async () => {
    await renderWithModelSelected(pagedInventory());

    // 第 1 / 2 页（共 60 条）：total 60、totalPages ceil(60/50)=2
    expect(pageInfoText()).toBe('第 1 / 2 页（共 60 条）');
    expect(screen.getByRole('button', { name: '上一页' }) !== null).toBe(true);
    expect(screen.getByRole('button', { name: '下一页' }) !== null).toBe(true);
  });

  it('正向：total 恰为 PAGE_SIZE 整数倍时末页 next 禁用、无幽灵空页（AC-4 呈现锚）', async () => {
    // 库存恰 100 条 = 2 整页：末页（offset 50）next 应由 total 判定禁用
    await renderWithModelSelected(
      Array.from({ length: 100 }, (_, index) => envelope(index, { id: index })),
    );

    expect(pageInfoText()).toBe('第 1 / 2 页（共 100 条）');
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(false);

    fireEvent.click(screen.getByTestId('db-page-next'));
    await waitFor(() => expect(pageInfoText()).toBe('第 2 / 2 页（共 100 条）'));
    expect(rowCells(bodyRows()[0])[0]).toBe('50'); // 末页真实落页
    // 末页（50 + 50 == total）：next 显式禁用 —— 废除旧
    // `records.length === PAGE_SIZE` 启发式（旧口径此处误判可翻并翻出空页）
    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(true);
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(false);
    expect(lastParamsOf('db_records')).toEqual({
      ...WS_PARAMS,
      model: 'workspace',
      offset: 50,
      limit: PAGE_SIZE,
    });
  });

  it('正向：db-page-prev / db-page-next 真实翻页（IPC 入参透传不变）', async () => {
    await renderWithModelSelected(pagedInventory());

    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true); // 首页
    fireEvent.click(screen.getByTestId('db-page-next'));
    await waitFor(() => expect(pageInfoText()).toBe('第 2 / 2 页（共 60 条）'));
    expect(lastParamsOf('db_records')).toEqual({
      ...WS_PARAMS,
      model: 'workspace',
      offset: 50,
      limit: PAGE_SIZE,
    });

    fireEvent.click(screen.getByTestId('db-page-prev'));
    await waitFor(() => expect(pageInfoText()).toBe('第 1 / 2 页（共 60 条）'));
    expect(lastParamsOf('db_records')).toEqual({
      ...WS_PARAMS,
      model: 'workspace',
      offset: 0,
      limit: PAGE_SIZE,
    });
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(true);
  });

  it('异常：空态（计数 0 → db-records-empty）不误报错误', async () => {
    mockIpc();
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());

    await selectModelAndWait('agent_run');

    expect(screen.getByTestId('db-records-empty') !== null).toBe(true);
    expect(screen.queryByText(/记录扫描失败/)).toBeNull();
    expect(screen.queryByTestId('db-inspector-error')).toBeNull();
  });

  it('异常：inline 持久错误（记录 reject / workspace 库打开失败）不白屏、无 toast 顶替', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return Promise.reject(new Error('db: workspace 库打开失败'));
    });
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());

    fireEvent.click(screen.getAllByTestId('db-model-item')[0]);

    expect(
      (await screen.findByText(/记录扫描失败：Error: db: workspace 库打开失败/)) !== null,
    ).toBe(true);
    // 不白屏：模型区与记录区 header 仍在
    expect(screen.getByTestId('db-model-list') !== null).toBe(true);
    expect(screen.getByRole('heading', { name: 'workspace' }) !== null).toBe(true);
    // 无 toast 顶替（查询轨 inline 持久双轨语义）
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(toastInfoMock).not.toHaveBeenCalled();
    // inline 持久：静置后错误区仍在
    await act(async () => {});
    expect(screen.getByText(/记录扫描失败/) !== null).toBe(true);
  });

  it('边界：recordsLoading 时翻页控件禁用', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'db_models') return Promise.resolve([...MODELS]);
      return new Promise<RecordsPage>(() => {}); // 记录取数在途
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

  it('边界：只读边界——可交互控件仅选择 / 翻页 / 刷新类，无任何写操作入口', async () => {
    await renderWithModelSelected(pagedInventory());

    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBeGreaterThan(0);
    // scope 双 tab 为只读切换（只改查询寻址，不写库），与刷新 / 翻页同类
    const readonlyActions = ['刷新数据', '上一页', '下一页', 'workspace 库', '全局库'];
    const offenders = buttons
      .filter((button) => {
        const isNav = readonlyActions.some((action) => button.textContent === action);
        const isRow = button.getAttribute('data-testid') === 'db-model-item';
        return !isNav && !isRow;
      })
      .map((button) => button.textContent ?? '');
    expect(offenders).toEqual([]);
    // 文案面复核：无新增/删除/修改类写操作动词
    const writeish = screen.queryAllByText(/(新增|添加|删除|修改|编辑|写入|保存|创建)/);
    expect(writeish).toHaveLength(0);
  });

  it('边界：模型行与记录行均可点选（选择类控件语义）', async () => {
    await renderWithModelSelected([envelope(1, { id: 1 })]);

    expect(within(screen.getByTestId('db-model-list')).getAllByRole('button')).toHaveLength(2);
    // 模型行点选已由装置点击承载：db_records 携所选模型名与 root
    expect(lastParamsOf('db_records')).toMatchObject({ model: 'workspace', root: ROOT });
    // 记录行点开抽屉详情
    const firstRow = bodyRows()[0];
    fireEvent.click(firstRow);
    await waitFor(() => expect(drawerTitle()).toBe('1'));
    expect(screen.getByTestId('db-record-json') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// scope 双 tab（desktop-workspace-db-split）：全局库 / workspace 库切换按钮组
// （aria-pressed 同 agent 页切换行模式），默认 workspace 库。真实组合
// useDbInspector：切换经 hook setScope 派发，重置语义经 invoke 面与渲染树联动断言。
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
          const records = [envelope(1, { id: 1 })].slice(offset, offset + limit);
          return Promise.resolve({ records, total: 1 });
        }
        return Promise.resolve([]);
      },
    );
    render(<DbInspectorView root={ROOT} />);
    await waitFor(() => expect(screen.queryByTestId('db-models-loading')).toBeNull());
    expect(countOf('db_models')).toBe(1);
    expect(invokeMock).toHaveBeenCalledWith('db_models', WS_PARAMS);
    await selectModelAndWait('agent_run');
    expect(screen.getByRole('table') !== null).toBe(true);

    fireEvent.click(screen.getByTestId('db-scope-user'));

    // hook 重置语义联动（真实组合，非替身 state）：清单以新 scope 重取
    await waitFor(() => expect(lastParamsOf('db_models')).toEqual({ scope: 'user', root: ROOT }));
    expect(countOf('db_models')).toBe(2);
    expect(screen.getByTestId('db-scope-user').getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByTestId('db-scope-workspace').getAttribute('aria-pressed')).toBe('false');
    // 选中模型复位：记录区卸载（表格与空态均不残留）
    await waitFor(() => expect(screen.queryByTestId('db-records-table')).toBeNull());
    // 两库清单互不混列可观测：全局库清单重取后仅 workspace 一行
    await waitFor(() => expect(screen.getAllByTestId('db-model-item')).toHaveLength(1));
    expect(screen.getAllByTestId('db-model-item')[0].getAttribute('data-model')).toBe('workspace');
    // 静置复核：未选中状态下记录区不复活（scope 切换在位响应被取消守卫丢弃）
    await act(async () => {});
    expect(screen.queryByTestId('db-records-table')).toBeNull();
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
