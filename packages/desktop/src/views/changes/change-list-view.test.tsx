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

/** create_change 应答：DTO = resolve（携铸出 id）、字符串 = reject。 */
type CreateAnswer =
  | {
      id: string;
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

/** 归档行名簇（裸名——归档日期前缀仅存在于磁盘目录名，MUST NOT 出线）。 */
const ARCHIVE_ID_FIRST = '0199a2f0-0001-7e45-8a9b-000000000001';
const ARCHIVE_ID_SECOND = '0199a2f0-0002-7e45-8a9b-000000000002';
const ARCHIVE_ID_NO_DATE = '0199a2f0-0003-7e45-8a9b-000000000003';

/** 清单 fixture：active 两条（建档恒定 status 在场）+ archive 三分组（未知时间置尾）；
 * id 恒为 uuid 形态身份锚、name 恒裸名（行键 / 导航 / 展示三面可辨——id ≠ name）。 */
const fixtureList: ChangeList = {
  active: [
    {
      id: '0199a2f0-0011-7e45-8a9b-000000000011',
      name: 'add-feature',
      source: 'active',
      status: 'active',
      activePhase: null,
      created: '2026-09-01',
    },
    {
      id: '0199a2f0-0012-7e45-8a9b-000000000012',
      name: 'docs-only',
      source: 'active',
      status: null,
      activePhase: null,
      created: null,
    },
  ],
  archiveGroups: [
    {
      month: '2026-09',
      changes: [
        {
          id: ARCHIVE_ID_FIRST,
          name: 'first',
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
          id: ARCHIVE_ID_SECOND,
          name: 'second',
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
          id: ARCHIVE_ID_NO_DATE,
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

/** Routes 装置：/changes 直挂清单页；/changes/:id 以导航桩占位（行点击
 * 导航落点经真实路由匹配观察）。 */
function listTree(root: string | null) {
  return (
    <MemoryRouter initialEntries={['/changes']}>
      <Routes>
        <Route path="/changes" element={<ChangeListView root={root} />} />
        <Route path="/changes/:id" element={<span data-testid="detail-stub" />} />
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
          id: 'add-feature',
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

  it('点击 change 条目 → 显式 navigate 落 /changes/:id（携带 summary.id，name 非寻址面）', async () => {
    await renderLoaded();

    fireEvent.click(screen.getByText('add-feature'));

    const activeId = fixtureList.active[0].id;
    expect(probePathname()).toBe(`/changes/${activeId}`);
    // id ≠ name 的 fixture 下：寻址恒 id，name 仅供展示
    expect(probePathname()).not.toBe(`/changes/${fixtureList.active[0].name}`);
    expect(screen.getByTestId('detail-stub') !== null).toBe(true);
  });

  it('归档条目（DTO 裸名 + id）→ 行展示裸名、行键 id 导航落 /changes/:id（零日期前缀形态）', async () => {
    await renderLoaded();

    // 裸名呈现：可见文本恒为 DTO name（created 为 2026-09-01 也不前缀成目录名形态）
    const archivedName = screen.getByText('first');
    expect(archivedName !== null).toBe(true);
    const archivedRow = screen.getByRole('button', { name: /first/ });
    expect(archivedRow.textContent).not.toMatch(/^\d{4}-\d{2}-\d{2}/);
    expect(archivedRow.textContent).not.toContain('2026-09-01-first');

    // 行键 / 导航恒 id：点击归档条目落 /changes/<归档 id>（非裸名、非日期前缀目录名）
    fireEvent.click(archivedName);
    expect(probePathname()).toBe(`/changes/${ARCHIVE_ID_FIRST}`);
    expect(screen.getByTestId('detail-stub') !== null).toBe(true);
  });

  it('error 状态渲染错误提示，不白屏', async () => {
    listAnswer = Promise.reject('IPC 断开');
    render(listTree(ROOT));

    await screen.findByText(/列表加载失败：IPC 断开/);
  });

  it('loading 态与空数据渲染空态提示（db 单源文案：零「未发现任何 change 目录」旧句）', async () => {
    // loading：list_changes 挂起不落定
    listAnswer = pending();
    const loadingView = render(listTree(ROOT));
    await waitFor(() => expect(loadingView.container.textContent).toContain('加载中'));
    loadingView.unmount();

    // 空数据：无 active、无分组 → db 单源引导文案（零磁盘扫描语义——历史上
    // 「未发现任何 change 目录」的磁盘发现口径随 list 单源化退役）
    const emptyView = await renderLoaded(emptyList);
    expect(emptyView.container.textContent).toContain('该 workspace 下未发现已建档 change。');
    expect(emptyView.container.textContent).not.toContain('未发现任何 change 目录');
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
    expect(container.textContent).not.toContain('未发现已建档 change');
    expect(container.textContent).not.toContain('未发现任何 change 目录');
    expect(container.textContent).toContain('first');
  });

  it('active 与 archive 均有内容时不提示 workspace 为空', async () => {
    const { container } = await renderLoaded({
      active: [fixtureList.active[0]],
      archiveGroups: [],
    });

    expect(container.textContent).not.toContain('未发现已建档 change');
    expect(container.textContent).not.toContain('未发现任何 change 目录');
    expect(container.textContent).toContain('add-feature');
  });
});

describe('ChangeListView：头部行（刷新与新建触发器，入口自 App header 迁入）', () => {
  it('头部行为页面根首子元素（刷新 + 新建触发器同居其内），error-note 紧随其后；loading / error / 空数据 / 有数据四形态下「刷新」按钮均存在', async () => {
    // error 态：头部行（刷新图标钮 + 新建变更触发器）是页面根的第一个子元素，随后为 error-note
    listAnswer = Promise.reject('IPC 断开');
    const withError = render(listTree(ROOT));
    await screen.findByTestId('error-note');
    const page = withError.container.firstElementChild;
    const headerRow = screen.getByRole('button', { name: '刷新' }).parentElement;
    expect(headerRow).not.toBeNull();
    expect(headerRow).toBe(page?.firstElementChild);
    expect(headerRow?.contains(screen.getByRole('button', { name: '新建变更' }))).toBe(true);
    expect(page?.children[1]?.getAttribute('data-testid')).toBe('error-note');
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
      expect(within(view.container).getByRole('button', { name: '刷新' }) !== null).toBe(true);
      view.unmount();
    }
  });

  it('点击「刷新」→ 清单重新取数（list_changes 调用数递增）', async () => {
    await renderLoaded();
    const before = listCalls().length;

    fireEvent.click(screen.getByRole('button', { name: '刷新' }));

    await waitFor(() => expect(listCalls().length).toBe(before + 1));
    expect(listCalls().at(-1)).toEqual({ root: ROOT });
  });

  it('loading=true：按钮 disabled、点击不触发重复取数（disabled={state.loading}）', async () => {
    listAnswer = pending();
    render(listTree(ROOT));
    await waitFor(() => expect(listCalls().length).toBe(1));

    const button = screen.getByRole('button', { name: '刷新' });
    expect(button.hasAttribute('disabled')).toBe(true);
    fireEvent.click(button);
    expect(listCalls().length).toBe(1);
  });

  it('空数据态（「暂无数据，点击刷新获取。」文案在场）：按钮仍可操作、root=null 点击零取数', () => {
    render(listTree(null));

    expect(screen.getByText('暂无数据，点击刷新获取。') !== null).toBe(true);
    const button = screen.getByRole('button', { name: '刷新' });
    expect(button.hasAttribute('disabled')).toBe(false);
    fireEvent.click(button);
    expect(invokeMock.mock.calls).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 新建入口挂载与创建流转（desktop-change-create 增量）：root 非空时头部行内
// 挂载新建触发器（PlusIcon 弹窗）；onCreated = state.refresh() + navigate
// （创建后不自动发起 run）。内部模块真实组合：对话框输入 → create_change
// IPC → refresh + navigate 全链路经进程边界观察。
// ---------------------------------------------------------------------------

describe('ChangeListView：新建入口挂载与创建流转（创建 → refresh + navigate）', () => {
  it('root 非空挂载新建入口：新建变更触发器在场且位于头部行内', async () => {
    const { container } = await renderLoaded();

    const trigger = screen.getByRole('button', { name: '新建变更' });
    expect(trigger !== null).toBe(true);
    const page = container.firstElementChild;
    const headerRow = screen.getByRole('button', { name: '刷新' }).parentElement;
    expect(page?.firstElementChild).toBe(headerRow);
    expect(headerRow?.contains(trigger)).toBe(true);
  });

  it('root 为 null 不挂载新建入口且零 invoke（无 workspace 无建档语义）', () => {
    render(listTree(null));

    expect(screen.queryByRole('button', { name: '新建变更' })).toBeNull();
    expect(invokeMock.mock.calls).toHaveLength(0);
  });

  it('清单挂载 → 弹窗提交 → refresh + navigate：create_change 恰一次、清单刷新、pathname 落 /changes/<铸出 id>、change_flow_start 零调用', async () => {
    const createdId = '0199a2f0-00f1-7e45-8a9b-0000000000f1';
    listAnswer = Promise.resolve(fixtureList);
    createAnswer = {
      id: createdId,
      name: 'fix-bug',
      created: '2026-10-02',
      worktree: 'C:home.dev-teamworktrees\repo-ab12\fix-bug',
      warnings: [],
    };
    render(flowTree(ROOT));
    await screen.findByText('add-feature');
    expect(listCalls().length).toBe(1);

    fireEvent.click(screen.getByRole('button', { name: '新建变更' }));
    fireEvent.change(screen.getByTestId('change-create-name'), {
      target: { value: 'fix-bug' },
    });
    fireEvent.change(screen.getByTestId('change-create-goal'), {
      target: { value: '修复登录重试的竞态问题' },
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));

    // 成功即直连 onCreated(outcome.id) → refresh + navigate（成功面退役）；
    // 落点取铸出 id 而非 change 名（name 恒展示面）
    await waitFor(() => expect(probePathname()).toBe(`/changes/${createdId}`));
    expect(probePathname()).not.toBe('/changes/fix-bug');
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'fix-bug', goal: '修复登录重试的竞态问题' },
    ]);
    // 清单刷新：onCreated → state.refresh() → list_changes 重新取数
    await waitFor(() => expect(listCalls().length).toBe(2));
    // 创建后不自动发起 run（proposal 拍板）
    expect(commandCount('change_flow_start')).toBe(0);
  });

  it('创建失败不停流转：行内错误呈现、弹窗留窗、pathname 不变、清单不刷新', async () => {
    listAnswer = Promise.resolve(fixtureList);
    createAnswer = 'change "fix-bug" 已存在: /repo/openspec/changes/fix-bug';
    render(flowTree(ROOT));
    await screen.findByText('add-feature');
    const before = listCalls().length;

    fireEvent.click(screen.getByRole('button', { name: '新建变更' }));
    fireEvent.change(screen.getByTestId('change-create-name'), {
      target: { value: 'fix-bug' },
    });
    fireEvent.change(screen.getByTestId('change-create-goal'), {
      target: { value: '修复登录重试的竞态问题' },
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));

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
// status / activePhase 两态消费（status 缺席条目照常入列）。
// ---------------------------------------------------------------------------

describe('ChangeListView：退役面负断言与状态面消费', () => {
  it('清单 DTO 无 inventory 字段 → V0/V1/V2 代际徽章零渲染（任何形态不出现）；月分组与归档条目渲染持衡', async () => {
    const { container } = await renderLoaded();

    // 代际词汇零残留：historical InventoryBadge 徽章文本（v0/v1/v2）任何形态不出现
    expect(container.textContent).not.toMatch(/v[012]/i);
    // 月分组与归档条目渲染持衡（同夹具双半边——退役不挤掉既有呈现）
    const headings = screen.getAllByRole('heading').map((h) => h.textContent ?? '');
    expect(headings.some((text) => text.startsWith('2026-09'))).toBe(true);
    expect(screen.getByText('first') !== null).toBe(true);
    expect(screen.getByText('no-date-archived') !== null).toBe(true);
  });

  it('DTO 无 unparsable 字段（含历史损坏样本形态条目）→「无法解析」标注与警示零出现', async () => {
    // corrupt-invalid-json：历史上 unparsable=true 的损坏样本同名形态（新 DTO 无该字段）
    const corruptLike: ChangeList = {
      active: [
        {
          id: 'corrupt-invalid-json',
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

  it('状态面消费：active+activePhase → 运行中相位呈现；archived → 归档条目月分组呈现无运行中徽标；status 缺席 → 无状态位照常入列', async () => {
    const mixed: ChangeList = {
      active: [
        {
          id: 'run-now',
          name: 'run-now',
          source: 'active',
          status: 'active',
          activePhase: { phase: 'test-gen', attempt: 1, startAt: null },
          created: null,
        },
        {
          id: 'no-status',
          name: 'no-status',
          source: 'active',
          status: null,
          activePhase: null,
          created: null,
        },
      ],
      archiveGroups: [
        {
          month: '2026-08',
          changes: [
            {
              id: 'done-archived',
              name: 'done-archived',
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
    const doneRow = rows.find((row) => (row.textContent ?? '').includes('done-archived'));
    expect(doneRow !== undefined).toBe(true);
    expect(within(doneRow!).queryByText(/运行中|已归档|未建档/)).toBeNull();
    // status 缺席（Option 形态保留）→ 无任何状态位，条目照常入列
    const noStatusRow = rows.find((row) => (row.textContent ?? '').includes('no-status'));
    expect(noStatusRow !== undefined).toBe(true);
    expect(within(noStatusRow!).queryByText(/运行中|已归档|未建档/)).toBeNull();
  });
});
