import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeList } from '../../types/dto';
import { ChangeListView } from './change-list-view';

// 内部协作模块不 mock（最小 mock 纪律）：useChangeList / ChangeCreateDialog /
// react-router 真实组合，既有 useChangeList 文件级受控替身不沿用——状态回灌
// 一律改经进程边界 invoke mock（@tauri-apps/api/core）按命令名分发：
// list_changes 应答驱动 data / loading / error / 空态四形态，create_change
// 可切换 resolve / reject；MemoryRouter + Routes + LocationProbe 装置沿既有，
// 提供 useNavigate 上下文并以探针观察导航落点。
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = '/repo';

/** URL 探针：把 MemoryRouter 当前 pathname 投影到 DOM 供断言。 */
function LocationProbe() {
  const { pathname } = useLocation();
  return <span data-testid="location-probe">{pathname}</span>;
}

// ---------------------------------------------------------------------------
// IPC fixture 装置：list_changes / create_change 按命令名分发的应答面
// ---------------------------------------------------------------------------

/** create_change 应答：DTO = resolve、字符串 = reject。 */
type CreateAnswer =
  | {
      name: string;
      created: string;
      worktree: string;
      warnings: string[];
    }
  | string
  | null;

let listAnswer: Promise<unknown> = Promise.resolve(null);
let createAnswer: CreateAnswer = null;

/** 按命令名分发的进程边界应答面。 */
function ipc(command: string): Promise<unknown> {
  if (command === 'list_changes') {
    return listAnswer;
  }
  if (command === 'create_change') {
    return typeof createAnswer === 'string'
      ? Promise.reject(createAnswer)
      : Promise.resolve(createAnswer);
  }
  return Promise.resolve(null);
}

/** 空清单 fixture（空态形态）。 */
const emptyList: ChangeList = { active: [], archiveGroups: [] };

/** 清单 fixture：active 两条（建档 + 文档形态）+ archive 三分组（未知时间置尾）。 */
const fixtureList: ChangeList = {
  active: [
    {
      name: 'add-feature',
      source: 'active',
      status: 'active',
      activePhase: null,
      created: '2026-09-01',
    },
    { name: 'docs-only', source: 'active', status: null, activePhase: null, created: null },
  ],
  archiveGroups: [
    {
      month: '2026-09',
      changes: [
        {
          name: '2026-09-01-first',
          source: 'archive',
          status: 'archived',
          activePhase: null,
          created: '2026-09-01',
        },
      ],
    },
    {
      month: '2026-05',
      changes: [
        {
          name: '2026-05-15-second',
          source: 'archive',
          status: null,
          activePhase: null,
          created: '2026-05-15',
        },
      ],
    },
    {
      month: null,
      changes: [
        {
          name: 'no-date-archived',
          source: 'archive',
          status: 'archived',
          activePhase: null,
          created: null,
        },
      ],
    },
  ],
};

/** 挂起 promise：list_changes 不落定（loading 形态）。 */
function pending(): Promise<unknown> {
  return new Promise(() => {});
}

beforeEach(() => {
  invokeMock.mockReset();
  listAnswer = Promise.resolve(emptyList);
  createAnswer = null;
  invokeMock.mockImplementation((command: string) => ipc(command));
});

afterEach(cleanup);

function listCalls(): Array<Record<string, unknown>> {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'list_changes')
    .map(([, args]) => args as Record<string, unknown>);
}

function createCalls(): Array<Record<string, unknown>> {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'create_change')
    .map(([, args]) => args as Record<string, unknown>);
}

function commandCount(name: string): number {
  return invokeMock.mock.calls.filter(([command]) => command === name).length;
}

function probePathname(): string {
  return screen.getByTestId('location-probe').textContent ?? '';
}

/** Routes 装置：/changes 直挂清单页；/changes/:name 以导航桩占位（行点击
 * 导航落点经真实路由匹配观察）。 */
function listTree(root: string | null) {
  return (
    <MemoryRouter initialEntries={['/changes']}>
      <Routes>
        <Route path="/changes" element={<ChangeListView root={root} />} />
        <Route path="/changes/:name" element={<span data-testid="detail-stub" />} />
      </Routes>
      <LocationProbe />
    </MemoryRouter>
  );
}

/** 流转装置：清单页恒挂载（不随导航卸载）——onCreated 的 refresh + navigate
 * 同批触发，页面留在挂载态才可同时观测「清单刷新（list_changes 递增）」与
 * 「导航落点」两半边。 */
function flowTree(root: string | null) {
  return (
    <MemoryRouter initialEntries={['/changes']}>
      <ChangeListView root={root} />
      <LocationProbe />
    </MemoryRouter>
  );
}

/** 以 fixture 清单载入后渲染（等待取数落定：loading 退场 + 数据 / 空态在场）。 */
async function renderLoaded(list: ChangeList = fixtureList, root: string | null = ROOT) {
  listAnswer = Promise.resolve(list);
  const view = render(listTree(root));
  await waitFor(() => expect(view.container.textContent).not.toContain('加载中'));
  if (list.active.length > 0) {
    await screen.findByText(list.active[0].name);
  }
  return view;
}

// ---------------------------------------------------------------------------
// 既有清单渲染语义沿用（分组 / 徽标 / 空态 / created 分支 / 头部刷新行），
// 状态回灌改经 mock IPC fixture
// ---------------------------------------------------------------------------

describe('ChangeListView：分组列表、运行中徽标与进入详情', () => {
  it('清单页挂载即以 root 发起 list_changes 取数（布线经进程边界观察）', async () => {
    await renderLoaded();

    expect(listCalls()).toEqual([{ root: ROOT }]);
  });

  it('active 列表逐条渲染，运行中条目（activePhase 在场）带运行中徽标', async () => {
    const running: ChangeList = {
      active: [
        {
          name: 'add-feature',
          source: 'active',
          status: 'active',
          activePhase: { phase: 'implement', attempt: 2, startAt: null },
          created: null,
        },
      ],
      archiveGroups: [],
    };
    await renderLoaded(running);

    expect(screen.getByText('add-feature') !== null).toBe(true);
    // 运行中徽标：activePhase 三元组随行呈现
    const badge = within(screen.getAllByTestId('change-row')[0]).getByText(
      /运行中 · implement · attempt 2/,
    );
    expect(badge !== null).toBe(true);
  });

  it('archive 按月分组渲染，month=null 的"未知时间"组渲染在序列尾', async () => {
    await renderLoaded();

    const headings = screen.getAllByRole('heading').map((h) => h.textContent ?? '');
    expect(headings.some((text) => text.startsWith('2026-09'))).toBe(true);
    expect(headings.some((text) => text.startsWith('2026-05'))).toBe(true);
    // 未知时间组置尾
    const last = headings[headings.length - 1];
    expect(last.startsWith('未知时间')).toBe(true);
    expect(screen.getByText('no-date-archived') !== null).toBe(true);
  });

  it('点击 change 条目 → 显式 navigate 落 /changes/:name（携带 change 名）', async () => {
    await renderLoaded();

    fireEvent.click(screen.getByText('add-feature'));
    expect(probePathname()).toBe('/changes/add-feature');
    expect(screen.getByTestId('detail-stub') !== null).toBe(true);
  });

  it('error 状态渲染错误提示，不白屏', async () => {
    listAnswer = Promise.reject('IPC 断开');
    render(listTree(ROOT));

    await screen.findByText(/列表加载失败：IPC 断开/);
  });

  it('loading 态与空数据渲染空态提示', async () => {
    // loading：list_changes 挂起不落定
    listAnswer = pending();
    const loadingView = render(listTree(ROOT));
    await waitFor(() => expect(loadingView.container.textContent).toContain('加载中'));
    loadingView.unmount();

    // 空数据：无 active、无分组 → 引导文案
    const emptyView = await renderLoaded(emptyList);
    expect(emptyView.container.textContent).toContain('未发现任何 change 目录');
  });

  it('文档形态条目（status 缺席）照常入列且无状态标注', async () => {
    await renderLoaded();

    expect(screen.getByText('docs-only') !== null).toBe(true);
    expect(screen.queryByText(/无法解析/)).toBeNull();
  });
});

describe('ChangeListView：created / 错误条 / 空态提示的分支形态', () => {
  it('created 为 null 的条目不渲染日期，非空条目渲染日期', async () => {
    const { container } = await renderLoaded();

    // 按钮名本身含日期前缀，因此必须精确断言 created 挂钩的文本而非整页 textContent
    const createdTexts = screen.getAllByTestId('created').map((span) => span.textContent ?? '');
    expect(createdTexts).toEqual(expect.arrayContaining(['2026-09-01', '2026-05-15']));
    for (const text of createdTexts) {
      expect(text.length).toBeGreaterThan(0);
    }
    expect(container.textContent).not.toContain('null');
  });

  it('无错误时不渲染错误提示条', async () => {
    await renderLoaded();

    expect(screen.queryByTestId('error-note')).toBeNull();
  });

  it('暂无数据提示仅在空闲、无数据且无错误时出现（真实组合下空闲态 = root 为 null）', async () => {
    // 空闲：root=null 不发起取数，data=null / loading=false / error=null
    const idle = render(listTree(null));
    expect(idle.container.textContent).toContain('暂无数据，点击刷新获取。');
    idle.unmount();

    // loading：挂起取数不回落空态提示
    listAnswer = pending();
    const loadingState = render(listTree(ROOT));
    await waitFor(() => expect(loadingState.container.textContent).toContain('加载中'));
    expect(loadingState.container.textContent).not.toContain('暂无数据');
    loadingState.unmount();

    // error：拒绝应答不回落空态提示
    listAnswer = Promise.reject('IPC 断开');
    const errorState = render(listTree(ROOT));
    await screen.findByTestId('error-note');
    expect(errorState.container.textContent).not.toContain('暂无数据');
    expect(errorState.container.textContent).toContain('列表加载失败');
    errorState.unmount();

    // 有数据：不提示空态
    await renderLoaded(emptyList);
    expect(screen.queryByText('暂无数据')).toBeNull();
  });

  it('active 为空但 archive 有分组时提示无进行中而不提示 workspace 为空', async () => {
    const firstGroup = fixtureList.archiveGroups[0];
    const { container } = await renderLoaded({ active: [], archiveGroups: [firstGroup] });

    expect(container.textContent).toContain('无进行中的 change。');
    expect(container.textContent).not.toContain('未发现任何 change 目录');
    expect(container.textContent).toContain('2026-09-01-first');
  });

  it('active 与 archive 均有内容时不提示 workspace 为空', async () => {
    const { container } = await renderLoaded({
      active: [fixtureList.active[0]],
      archiveGroups: [],
    });

    expect(container.textContent).not.toContain('未发现任何 change 目录');
    expect(container.textContent).toContain('add-feature');
  });
});

describe('ChangeListView：头部刷新行（刷新入口自 App header 迁入）', () => {
  it('头部行先于新建入口与 error-note 渲染，loading / error / 空数据 / 有数据四形态下「刷新列表」按钮均存在', async () => {
    // error 态：头部行是页面根的第一个子元素，随后为新建入口卡片、error-note
    listAnswer = Promise.reject('IPC 断开');
    const withError = render(listTree(ROOT));
    await screen.findByTestId('error-note');
    const page = withError.container.firstElementChild;
    const headerRow = screen.getByRole('button', { name: '刷新列表' }).parentElement;
    expect(headerRow).not.toBeNull();
    expect(headerRow).toBe(page?.firstElementChild);
    expect(page?.children[1]?.getAttribute('data-testid')).toBe('change-create-dialog');
    expect(page?.children[2]?.getAttribute('data-testid')).toBe('error-note');
    withError.unmount();

    // 始终在场：四形态下刷新按钮均存在（空态可操作的前提）
    const shapes: Array<[string | null, Promise<unknown>]> = [
      [ROOT, pending()], // loading
      [ROOT, Promise.reject('IPC 断开')], // error
      [null, Promise.resolve(emptyList)], // 空数据（root=null 零取数）
      [ROOT, Promise.resolve(fixtureList)], // 有数据
    ];
    for (const [root, answer] of shapes) {
      listAnswer = answer;
      const view = render(listTree(root));
      expect(within(view.container).getByRole('button', { name: '刷新列表' }) !== null).toBe(true);
      view.unmount();
    }
  });

  it('点击「刷新列表」→ 清单重新取数（list_changes 调用数递增）', async () => {
    await renderLoaded();
    const before = listCalls().length;

    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));

    await waitFor(() => expect(listCalls().length).toBe(before + 1));
    expect(listCalls().at(-1)).toEqual({ root: ROOT });
  });

  it('loading=true：按钮 disabled、点击不触发重复取数（disabled={state.loading}）', async () => {
    listAnswer = pending();
    render(listTree(ROOT));
    await waitFor(() => expect(listCalls().length).toBe(1));

    const button = screen.getByRole('button', { name: '刷新列表' });
    expect(button.hasAttribute('disabled')).toBe(true);
    fireEvent.click(button);
    expect(listCalls().length).toBe(1);
  });

  it('空数据态（「暂无数据，点击刷新获取。」文案在场）：按钮仍可操作、root=null 点击零取数', () => {
    render(listTree(null));

    expect(screen.getByText('暂无数据，点击刷新获取。') !== null).toBe(true);
    const button = screen.getByRole('button', { name: '刷新列表' });
    expect(button.hasAttribute('disabled')).toBe(false);
    fireEvent.click(button);
    expect(invokeMock.mock.calls).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 新建入口挂载与创建流转（desktop-change-create 增量）：root 非空时头部行下
// 挂载 ChangeCreateDialog；onCreated = state.refresh() + navigate（创建后不
// 自动发起 run）。内部模块真实组合：对话框输入 → create_change IPC →
// refresh + navigate 全链路经进程边界观察。
// ---------------------------------------------------------------------------

describe('ChangeListView：新建入口挂载与创建流转（创建 → refresh + navigate）', () => {
  it('root 非空挂载新建入口：toggle 在场且挂载于头部行之下', async () => {
    const { container } = await renderLoaded();

    expect(screen.getByTestId('change-create-toggle') !== null).toBe(true);
    const page = container.firstElementChild;
    const headerRow = screen.getByRole('button', { name: '刷新列表' }).parentElement;
    expect(page?.firstElementChild).toBe(headerRow);
    expect(page?.children[1]?.getAttribute('data-testid')).toBe('change-create-dialog');
  });

  it('root 为 null 不挂载新建入口且零 invoke（无 workspace 无建档语义）', () => {
    render(listTree(null));

    expect(screen.queryByTestId('change-create-toggle')).toBeNull();
    expect(screen.queryByTestId('change-create-dialog')).toBeNull();
    expect(invokeMock.mock.calls).toHaveLength(0);
  });

  it('清单挂载 → 对话框提交 → refresh + navigate：create_change 恰一次、清单刷新、pathname 落 /changes/:name、change_flow_start 零调用', async () => {
    listAnswer = Promise.resolve(fixtureList);
    createAnswer = {
      name: 'fix-bug',
      created: '2026-10-02',
      worktree: 'C:home.dev-teamworktrees\repo-ab12\fix-bug',
      warnings: [],
    };
    render(flowTree(ROOT));
    await screen.findByText('add-feature');
    expect(listCalls().length).toBe(1);

    fireEvent.click(screen.getByTestId('change-create-toggle'));
    fireEvent.change(screen.getByTestId('change-create-name'), {
      target: { value: 'fix-bug' },
    });
    fireEvent.change(screen.getByTestId('change-create-goal'), {
      target: { value: '修复登录重试的竞态问题' },
    });
    fireEvent.click(screen.getByTestId('change-create-submit'));

    // 成功面（D14）：进入详情按钮触发 onCreated → refresh + navigate
    fireEvent.click(await screen.findByTestId('change-create-open-detail'));
    await waitFor(() => expect(probePathname()).toBe('/changes/fix-bug'));
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'fix-bug', goal: '修复登录重试的竞态问题' },
    ]);
    // 清单刷新：onCreated → state.refresh() → list_changes 重新取数
    await waitFor(() => expect(listCalls().length).toBe(2));
    // 创建后不自动发起 run（proposal 拍板）
    expect(commandCount('change_flow_start')).toBe(0);
  });

  it('创建失败不停流转：行内错误呈现、pathname 不变、清单不刷新', async () => {
    listAnswer = Promise.resolve(fixtureList);
    createAnswer = 'change "fix-bug" 已存在: /repo/openspec/changes/fix-bug';
    render(flowTree(ROOT));
    await screen.findByText('add-feature');
    const before = listCalls().length;

    fireEvent.click(screen.getByTestId('change-create-toggle'));
    fireEvent.change(screen.getByTestId('change-create-name'), {
      target: { value: 'fix-bug' },
    });
    fireEvent.change(screen.getByTestId('change-create-goal'), {
      target: { value: '修复登录重试的竞态问题' },
    });
    fireEvent.click(screen.getByTestId('change-create-submit'));

    const error = await screen.findByTestId('change-create-error');
    expect(error.textContent).toContain('已存在');
    expect(probePathname()).toBe('/changes');
    expect(listCalls().length).toBe(before);
  });
});

// ---------------------------------------------------------------------------
// 退役面负断言与状态面消费（desktop-workflow-db-state）：清单 DTO 已无
// inventory / unparsable 字段（TS 类型面随 bindings 同步删除，夹具带该字段
// 即编译失败），视图退役 InventoryBadge 与「无法解析」标注；条目状态面改
// status / activePhase 两态消费，文档形态条目照常入列。
// ---------------------------------------------------------------------------

describe('ChangeListView：退役面负断言与状态面消费', () => {
  it('清单 DTO 无 inventory 字段 → V0/V1/V2 代际徽章零渲染（任何形态不出现）；月分组与归档条目渲染持衡', async () => {
    const { container } = await renderLoaded();

    // 代际词汇零残留：historical InventoryBadge 徽章文本（v0/v1/v2）任何形态不出现
    expect(container.textContent).not.toMatch(/v[012]/i);
    // 月分组与归档条目渲染持衡（同夹具双半边——退役不挤掉既有呈现）
    const headings = screen.getAllByRole('heading').map((h) => h.textContent ?? '');
    expect(headings.some((text) => text.startsWith('2026-09'))).toBe(true);
    expect(screen.getByText('2026-09-01-first') !== null).toBe(true);
    expect(screen.getByText('no-date-archived') !== null).toBe(true);
  });

  it('DTO 无 unparsable 字段（含历史损坏样本形态条目）→「无法解析」标注与警示零出现', async () => {
    // corrupt-invalid-json：历史上 unparsable=true 的损坏样本同名形态（新 DTO 无该字段）
    const corruptLike: ChangeList = {
      active: [
        {
          name: 'corrupt-invalid-json',
          source: 'active',
          status: null,
          activePhase: null,
          created: null,
        },
      ],
      archiveGroups: [],
    };
    const { container } = await renderLoaded(corruptLike);

    expect(screen.getByText('corrupt-invalid-json') !== null).toBe(true);
    expect(container.textContent).not.toContain('无法解析');
    expect(container.textContent).not.toContain('workflow.json');
  });

  it('状态面消费：active+activePhase → 运行中相位呈现；archived → 归档条目月分组呈现无运行中徽标；双 null → 无状态位照常入列', async () => {
    const mixed: ChangeList = {
      active: [
        {
          name: 'run-now',
          source: 'active',
          status: 'active',
          activePhase: { phase: 'test-gen', attempt: 1, startAt: null },
          created: null,
        },
        { name: 'doc-only', source: 'active', status: null, activePhase: null, created: null },
      ],
      archiveGroups: [
        {
          month: '2026-08',
          changes: [
            {
              name: '2026-08-01-done',
              source: 'archive',
              status: 'archived',
              activePhase: null,
              created: null,
            },
          ],
        },
      ],
    };
    await renderLoaded(mixed);

    const rows = screen.getAllByTestId('change-row');
    // active + activePhase 在场 → 进行中相位徽标呈现
    const runRow = rows.find((row) => (row.textContent ?? '').includes('run-now'));
    expect(within(runRow!).getByText(/运行中 · test-gen · attempt 1/) !== null).toBe(true);
    // archived → 归档条目照常呈现（月分组内），无运行中徽标、无独立状态 chip
    const doneRow = rows.find((row) => (row.textContent ?? '').includes('2026-08-01-done'));
    expect(doneRow !== undefined).toBe(true);
    expect(within(doneRow!).queryByText(/运行中|已归档|未建档/)).toBeNull();
    // 双 null（文档形态）→ 无任何状态位，条目照常入列
    const docRow = rows.find((row) => (row.textContent ?? '').includes('doc-only'));
    expect(docRow !== undefined).toBe(true);
    expect(within(docRow!).queryByText(/运行中|已归档|未建档/)).toBeNull();
  });
});
