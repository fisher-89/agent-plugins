import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import {
  MemoryRouter,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useNavigationType,
} from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ExploreRecord } from '../../types/dto';
import { ExploreView } from './explore-view';

// 进程边界 Mock：仅 mock IPC（@tauri-apps/api/core 的 invoke / Channel），被测
// 页面 import 的内部 hooks（useExploreList / useExploreDoc / useExploreSession）
// 真实组合，fixture 经 mock IPC 流入后断言渲染输出（最小 mock 纪律）。
const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

const ROOT = 'C:\\demo\\alpha';
const PLACEHOLDER_NAME = 'draft-1758123456789';

/** 清单 fixture（title ≠ name：展示面必须取 title）。 */
function record(id: number, name: string, title: string): ExploreRecord {
  return {
    id,
    root: ROOT,
    name,
    title,
    workflowType: 'requirement',
    promotedTo: null,
    sessionId: null,
    createdAt: 1727000000000 + id,
    updatedAt: 1727000000000 + id,
  };
}

/** 占位记录 fixture（sessionId 在场——详情页重放与 auto-send 就绪形态）。 */
function placeholderRecord(): ExploreRecord {
  return {
    ...record(3, PLACEHOLDER_NAME, '2026-10-17 08:30:45-分析中'),
    sessionId: 'ses-draft-1727000001000',
  };
}

let records: ExploreRecord[] = [];

/** 进程边界应答面：清单 + 详情页 hooks 所需命令的空态应答。 */
function ipc(command: string): Promise<unknown> {
  if (command === 'list_explore_records') return Promise.resolve(records);
  if (command === 'read_explore') return Promise.resolve(null);
  if (command === 'explore_doc_path')
    return Promise.resolve('C:\\demo\\alpha\\openspec\\explores\\x.md');
  if (command === 'watch_subscribe') return Promise.resolve(1);
  if (command === 'watch_unsubscribe') return Promise.resolve(null);
  if (command === 'session_detail') return Promise.resolve(null);
  if (command === 'agent_session_transcript') return Promise.resolve([]);
  if (command === 'agent_start') {
    // early-resolve running 轮行形态（handleRecord 可安全消费）
    return Promise.resolve({
      turnId: 13,
      sessionId: 'ses-draft-1727000001000',
      status: 'running',
      startedAt: 1727000001000,
      finishedAt: null,
      numTurns: null,
      costUsd: null,
      durationMs: null,
      error: null,
    });
  }
  if (command === 'promote_explore') {
    return Promise.resolve({ changeId: 'chg-1', changeName: 'api-retry' });
  }
  if (command === 'create_explore_record') return Promise.resolve(placeholderRecord());
  if (command === 'update_explore_title') return Promise.resolve(placeholderRecord());
  return Promise.resolve(null);
}

/** 视图树装置（root 可换：workspace 切换用例经 rerender 复用同一实例；
 * `withListLink` 追加「返回清单」导航探针，供详情页动作后回清单断言归组）。 */
function viewTree(path: string, root: string = ROOT, withListLink = false) {
  return (
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/explores" element={<ExploreView root={root} />} />
        <Route path="/explores/:name" element={<ExploreView root={root} />} />
        <Route path="/changes/:id" element={<span data-testid="change-detail-stub" />} />
      </Routes>
      <LocationProbe />
      {withListLink && <ListNavProbe />}
    </MemoryRouter>
  );
}

/** 路由装置：/explores（清单）与 /explores/:name（详情）两形态 + 位置探针。 */
function renderAt(path: string) {
  return render(viewTree(path));
}

/** 路由装置（显式 root）：workspace 根切换用例的起点。 */
function renderAtRoot(path: string, root: string) {
  return render(viewTree(path, root));
}

/** URL 探针：把当前 pathname 投影到 DOM 供 navigate 断言；导航类型探针
 * （POP / PUSH / REPLACE）供 replace 语义断言。 */
function LocationProbe() {
  const { pathname } = useLocation();
  const navigationType = useNavigationType();
  return (
    <>
      <span data-testid="location-probe">{pathname}</span>
      <span data-testid="nav-type-probe">{navigationType}</span>
    </>
  );
}

const startCallCount = () =>
  invokeMock.mock.calls.filter(([name]) => name === 'agent_start').length;

const listCallCount = () =>
  invokeMock.mock.calls.filter(([name]) => name === 'list_explore_records').length;

/** 导航探针：真实 Router 上下文内的「返回清单」入口（详情页动作后回清单断言归组）。 */
function ListNavProbe() {
  const navigate = useNavigate();
  return (
    <button data-testid="list-nav-probe" onClick={() => void navigate('/explores')} type="button">
      返回清单
    </button>
  );
}

/** 已转变更记录 fixture（promotedTo 非空 = 已 promote 打标）。 */
function promotedRecord(
  id: number,
  name: string,
  title: string,
  promotedTo = 'chg-1',
): ExploreRecord {
  return { ...record(id, name, title), promotedTo };
}

/** 混合清单 fixture：2 条进行中（id 较大）+ 2 条已转变更（id 较小），`createdAt = base + id`。 */
function mixedRecords(): ExploreRecord[] {
  return [
    promotedRecord(1, 'promoted-a', '已转主题甲', 'chg-1'),
    promotedRecord(2, 'promoted-b', '已转主题乙', 'chg-2'),
    record(3, 'active-a', '进行中主题甲'),
    record(4, 'active-b', '进行中主题乙'),
  ];
}

/** 按 `data-name` 取条目按钮（组内条目序不稳时定位不依赖索引）。 */
function itemByName(name: string): HTMLElement {
  const item = screen
    .getAllByTestId('explore-item')
    .find((node) => node.getAttribute('data-name') === name);
  if (item === undefined) throw new Error(`清单内无记录：${name}`);
  return item;
}

/** 按 `data-name` 取条目删除入口（倒序后索引不再稳定指向目标记录）。 */
function deleteButtonByName(name: string): HTMLElement {
  const row = itemByName(name).closest('li');
  if (row === null) throw new Error(`条目行缺失：${name}`);
  return within(row).getByTestId('explore-item-delete');
}

/** 分组区组 `section` 的 testid 序列（组序断言统一口径）。 */
function groupTestIds(): Array<string | null> {
  const items = screen.getByTestId('explore-items');
  return [...items.querySelectorAll('section')].map((node) => node.getAttribute('data-testid'));
}

/** 组标题文本（组名 + 计数；标题内层级 span 不入断言）。 */
function groupLabel(group: HTMLElement): string | null {
  return group.querySelector('h3')?.textContent ?? null;
}

/** 组内条目 `data-name` 序列（顺序断言一律取 data-name，不取本地化时间文本）。 */
function namesIn(group: HTMLElement): Array<string | null> {
  return within(group)
    .queryAllByTestId('explore-item')
    .map((node) => node.getAttribute('data-name'));
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => ipc(command));
  records = [record(1, 'api-retry', '接口重试策略'), record(2, 'layout-design', '布局设计')];
});

afterEach(() => {
  cleanup();
});

// ---------------------------------------------------------------------------
// 清单页渲染（既有回归）
// ---------------------------------------------------------------------------

describe('ExploreView 清单页：条目渲染 record.title（AC-8 清单半边）', () => {
  it('正向：条目标题显示 title（title ≠ name 时显示 title 而非 name）', async () => {
    renderAt('/explores');

    await screen.findByTestId('explore-items');
    const titles = (await screen.findAllByTestId('explore-item-title')).map(
      (node) => node.textContent,
    );
    // 组内按 createdAt 倒序（createdAt = base + id ⇒ id 大者在前）
    expect(titles).toEqual(['布局设计', '接口重试策略']);
    // name 仅作 data-name 保留（零标题语义）
    const items = screen.getAllByTestId('explore-item');
    expect(items.map((node) => node.getAttribute('data-name'))).toEqual([
      'layout-design',
      'api-retry',
    ]);
    expect(screen.queryByText('api-retry')).toBeNull();
  });

  it('边界：title 两态（title = name 缺省样本与显式标题样本）各渲染正确，恒非空零回退分支', async () => {
    records = [record(1, 'same-name', 'same-name'), record(2, 'explicit', '显式标题')];
    renderAt('/explores');

    await screen.findByTestId('explore-items');
    const titles = (await screen.findAllByTestId('explore-item-title')).map(
      (node) => node.textContent,
    );
    expect(titles).toEqual(['显式标题', 'same-name']);
    expect(titles.every((title) => title !== null && title.length > 0)).toBe(true);
  });
});

describe('ExploreView 详情页：list 透传与缺记录态（AC-8 / AC-5 动作可达）', () => {
  it('正向：选中记录 → ExploreDetailView 收到 list（详情头 title + promote 动作可达）', async () => {
    renderAt('/explores/api-retry');

    const title = await screen.findByTestId('explore-detail-title');
    expect(title.textContent).toBe('接口重试策略');

    // list.promote 可达：点击「启动变更」→ invoke promote_explore（携选择器当前类型）
    fireEvent.click(screen.getByTestId('explore-promote-trigger'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('promote_explore', {
        root: ROOT,
        name: 'api-retry',
        workflowType: 'requirement',
      }),
    );
  });

  it('边界：未选中 / 缺失记录（清单无该 name 且无快照）→ 详情缺失态占位（既有语义零改动）', async () => {
    renderAt('/explores/ghost-topic');

    expect(await screen.findByTestId('explore-detail-missing')).toBeTruthy();
    expect(screen.queryByTestId('explore-detail-title')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// auto-named-explore：onCreated → 导航携 autoPrompt（AC-1 前半链）
// ---------------------------------------------------------------------------

describe('ExploreView：onCreated 导航链（清单页 → 详情页首轮，AC-1 / AC-8）', () => {
  it('正向：新话题建档 → list.refresh 后 navigate /explores/<占位名> 携 state.autoPrompt；详情页 auto-send 以 stance + 需求原文发起首轮', async () => {
    // 刷新后清单含建档记录（refresh 由 onCreated 触发 → 第二次取数返回占位记录）
    const listQueue: ExploreRecord[][] = [[], [placeholderRecord()]];
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return Promise.resolve(listQueue.shift() ?? []);
      return ipc(command);
    });

    renderAt('/explores');
    fireEvent.click(await screen.findByTestId('explore-create-toggle'));
    fireEvent.change(screen.getByTestId('explore-topic-requirement'), {
      target: { value: '调研重试策略的失效边界' },
    });
    fireEvent.click(screen.getByTestId('explore-topic-create'));

    // 导航至占位名详情（navigate 携 state → 详情页 autoPrompt 读数）
    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe(
        `/explores/${PLACEHOLDER_NAME}`,
      ),
    );
    // 清单显式刷新发生（onCreated → list.refresh）
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([name]) => name === 'list_explore_records').length).toBe(
        2,
      ),
    );
    // 详情页 auto-send：占位记录 → 命名指令形 stance + 需求原文
    await waitFor(() => expect(startCallCount()).toBe(1));
    const startCalls = invokeMock.mock.calls.filter(([name]) => name === 'agent_start');
    expect(startCalls).toHaveLength(1);
    const params = startCalls[0][1] as Record<string, unknown>;
    const prompt = String(params.prompt);
    expect(prompt).toContain('调研重试策略的失效边界');
    expect(prompt).toContain('## 笔记定名与落盘');
    expect(prompt).not.toContain('openspec/explores');
    // 无 MissingRecord 闪现
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
  });

  it('边界：导入路径（autoPrompt = null）→ navigate 不携带 state、详情页零自动发送（AC-8 回归）', async () => {
    const imported = record(4, 'imported-topic', '导入主题');
    const listQueue: ExploreRecord[][] = [
      [{ ...record(1, 'api-retry', '接口重试策略') }],
      [imported],
    ];
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return Promise.resolve(listQueue.shift() ?? []);
      if (command === 'create_explore_record') return Promise.resolve(imported);
      if (command === 'scan_explores') {
        return Promise.resolve([{ name: 'imported-topic', modifiedAt: null }]);
      }
      return ipc(command);
    });

    renderAt('/explores');
    fireEvent.click(await screen.findByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));
    fireEvent.click(await screen.findByTestId('explore-import-item'));

    // 导航至导入名详情（无 state → 零 auto-send）
    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/imported-topic'),
    );
    await act_dummy();
    expect(startCallCount()).toBe(0);
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
  });
});

/** 事件循环排空（等待零断言窗口）。 */
async function act_dummy() {
  await waitFor(() => expect(invokeMock).toHaveBeenCalled());
}

// ---------------------------------------------------------------------------
// auto-named-explore：三级链（id 桥）——旧名 URL 的派生与收敛（AC-4）
// ---------------------------------------------------------------------------

/** 改名流清单队列（模块级，按调用序出队；每用例经 mockRenamedFlow 重置）。 */
const renamedQueue: ExploreRecord[][] = [];

function listShift(): ExploreRecord[] {
  const next = renamedQueue.shift();
  return next ?? [];
}

/** 改名流 mock：首次取数旧名记录；title 回填 refresh 后第二次取数入参清单。 */
function mockRenamedFlow(secondList: ExploreRecord[]) {
  const oldRecord = {
    ...record(1, 'draft-old-name', '旧标题'),
    sessionId: 'ses-old-1727000001000',
  };
  renamedQueue.length = 0;
  renamedQueue.push([oldRecord], secondList);
  invokeMock.mockImplementation((command: string) => {
    if (command === 'list_explore_records') return Promise.resolve(listShift());
    if (command === 'read_explore') {
      return Promise.resolve({ name: 'draft-old-name', content: '# 回填触发的标题\n正文' });
    }
    return ipc(command);
  });
}

describe('ExploreView：三级链 id 桥（记录改名后旧名 URL 的派生与收敛，AC-4）', () => {
  it('正向：URL 旧名 miss（记录已改名）→ 按快照 id 在清单重寻命中、渲染新名详情并 replace 收敛', async () => {
    mockRenamedFlow([
      { ...record(1, 'new-final-name', '旧标题'), sessionId: 'ses-old-1727000001000' },
    ]);
    renderAt('/explores/draft-old-name');

    // 三级链：旧名 miss → 快照 id 重寻命中 → 新名详情渲染（无 MissingRecord）
    await waitFor(() =>
      expect(screen.getByTestId('explore-detail-title').textContent).toBe('旧标题'),
    );
    // 名变收敛导航：URL replace 至新名（详情页名变收敛 effect）
    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/new-final-name'),
    );
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
  });

  it('边界：URL 名与清单双 miss → 以快照本体兜底渲染、不闪现 MissingRecord（MUST NOT 断言）', async () => {
    // 清单刷新后为空（双 miss 形态）：快照本体兜底渲染
    mockRenamedFlow([]);
    renderAt('/explores/draft-old-name');

    await waitFor(() =>
      expect(screen.getByTestId('explore-detail-title').textContent).toBe('旧标题'),
    );
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([name]) => name === 'list_explore_records').length).toBe(
        2,
      ),
    );
    await act_dummy();
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
    expect(screen.getByTestId('explore-detail-title').textContent).toBe('旧标题');
  });

  it('异常：刷新间隙旧名 miss → 快照兜底维持渲染、全程无 MissingRecord 闪现', async () => {
    // 改名流：清单含新名记录（id 相同）——旧名 miss 窗口以快照 id 重寻兜底
    mockRenamedFlow([
      { ...record(1, 'new-final-name', '旧标题'), sessionId: 'ses-old-1727000001000' },
    ]);
    renderAt('/explores/draft-old-name');

    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/new-final-name'),
    );
    // 全程断言：缺失态占位零出现（闪现即失败）
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 清单页条目动作面：进入详情 / 删除（动作回调接线，两入口各验一次）
// ---------------------------------------------------------------------------

describe('ExploreView 清单页：条目动作接线（AC-8）', () => {
  it('正向：条目点击 → navigate 至 /explores/<record.name> 并渲染该记录详情', async () => {
    renderAt('/explores');
    await screen.findByTestId('explore-items');

    // 组内按 createdAt 倒序：索引 0 = 最新建档的 layout-design
    fireEvent.click(screen.getAllByTestId('explore-item')[0]);

    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/layout-design'),
    );
    expect(screen.getByTestId('explore-detail-title').textContent).toBe('布局设计');
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
  });

  it('正向：条目删除 → invoke delete_explore_record（携 root + name）并刷新清单', async () => {
    renderAt('/explores');
    await screen.findByTestId('explore-items');
    const before = invokeMock.mock.calls.filter(([name]) => name === 'list_explore_records').length;

    // 倒序后索引不再稳定指向目标记录：按 data-name 定位 api-retry 的删除入口
    fireEvent.click(deleteButtonByName('api-retry'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('delete_explore_record', {
        root: ROOT,
        name: 'api-retry',
      }),
    );
    // 删除后清单显式刷新（动作薄封装 invoke 后 refresh）
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([name]) => name === 'list_explore_records').length).toBe(
        before + 1,
      ),
    );
  });
});

// ---------------------------------------------------------------------------
// 清单三态呈现：错误 / 加载中 / 空态 / 有记录（四分支互斥，各自只在对应态在场）
// ---------------------------------------------------------------------------

describe('ExploreView 清单页：错误 / 加载中 / 空态三态互斥呈现（AC-8）', () => {
  it('空态：清单为空且非加载非错误 → 空态提示在场、加载中与错误块均不在场', async () => {
    records = [];
    renderAt('/explores');

    const empty = await screen.findByTestId('explore-list-empty');
    expect(empty.textContent).toContain('暂无探索记录');
    expect(screen.queryByTestId('explore-list-loading')).toBeNull();
    expect(screen.queryByTestId('explore-list-error')).toBeNull();
  });

  it('有记录态：清单非空 → 空态提示不在场（三态互斥）、条目照常渲染', async () => {
    renderAt('/explores');

    await screen.findByTestId('explore-items');
    await waitFor(() => expect(screen.getAllByTestId('explore-item')).toHaveLength(2));
    expect(screen.queryByTestId('explore-list-empty')).toBeNull();
    expect(screen.queryByTestId('explore-list-loading')).toBeNull();
    expect(screen.queryByTestId('explore-list-error')).toBeNull();
  });

  it('加载中：挂载取数未返回 → 「加载中…」在场、空态与错误块不在场（不抢跑空态）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return new Promise<ExploreRecord[]>(() => {});
      return ipc(command);
    });
    renderAt('/explores');

    const loading = await screen.findByTestId('explore-list-loading');
    expect(loading.textContent).toBe('加载中…');
    expect(screen.queryByTestId('explore-list-empty')).toBeNull();
    expect(screen.queryByTestId('explore-list-error')).toBeNull();
  });

  it('错误态：清单取数 reject → 错误块呈现原始错误、空态与加载中不在场', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return Promise.reject('清单查询失败');
      return ipc(command);
    });
    renderAt('/explores');

    const error = await screen.findByTestId('explore-list-error');
    expect(error.textContent).toContain('清单加载失败');
    expect(error.textContent).toContain('清单查询失败');
    expect(screen.queryByTestId('explore-list-empty')).toBeNull();
    expect(screen.queryByTestId('explore-list-loading')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// workspace 根切换：resetPending 收敛链（带旧选中即落清单，且不锁死清单页）
// ---------------------------------------------------------------------------

describe('ExploreView：workspace 根切换 resetPending 收敛（AC-4 相邻面）', () => {
  const ROOT2 = 'C:demo\beta';

  /** 按 root 分流清单应答（新根清单含独立记录）。 */
  function mockPerRoot() {
    invokeMock.mockImplementation((command: string, params?: { root?: string }) => {
      if (command === 'list_explore_records') {
        return Promise.resolve(
          params?.root === ROOT2 ? [record(5, 'beta-topic', '乙主题')] : records,
        );
      }
      return ipc(command);
    });
  }

  it('正向：带旧选中时根切换 → navigate 落 /explores（旧记录详情卸载，新根清单在场）', async () => {
    mockPerRoot();
    const { rerender } = renderAtRoot('/explores/api-retry', ROOT);
    await waitFor(() =>
      expect(screen.getByTestId('explore-detail-title').textContent).toBe('接口重试策略'),
    );

    rerender(viewTree('/explores/api-retry', ROOT2));

    await waitFor(() => expect(screen.getByTestId('location-probe').textContent).toBe('/explores'));
    expect(screen.queryByTestId('explore-detail-title')).toBeNull();
    // 收敛导航为 replace（不留旧根的可回退历史条目）
    expect(screen.getByTestId('nav-type-probe').textContent).toBe('REPLACE');
    await waitFor(() => expect(screen.getByText('乙主题')).toBeTruthy());
  });

  it('边界：落清单后条目点击仍进详情（resetPending 已复位，清单页不被残留态锁死）', async () => {
    mockPerRoot();
    const { rerender } = renderAtRoot('/explores/api-retry', ROOT);
    await waitFor(() =>
      expect(screen.getByTestId('explore-detail-title').textContent).toBe('接口重试策略'),
    );

    rerender(viewTree('/explores/api-retry', ROOT2));
    await waitFor(() => expect(screen.getByTestId('location-probe').textContent).toBe('/explores'));

    fireEvent.click(await screen.findByTestId('explore-item'));

    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/beta-topic'),
    );
    expect(screen.getByTestId('explore-detail-title').textContent).toBe('乙主题');
  });
});

// ---------------------------------------------------------------------------
// 清单分组呈现：组序 / 组标题计数 / 条目全局序（AC-4）
// ---------------------------------------------------------------------------

describe('ExploreView 清单页：分组呈现（组序 / 计数 / 条目全局序，AC-4）', () => {
  it('正向：混合清单 → 分组区两个 section 恒序（进行中在前、已转变更在后）', async () => {
    records = mixedRecords();
    renderAt('/explores');

    await screen.findByTestId('explore-items');

    expect(groupTestIds()).toEqual(['explore-group-active', 'explore-group-promoted']);
  });

  it('正向：组标题携带组名与计数，组内条目数与计数一致', async () => {
    records = mixedRecords();
    renderAt('/explores');

    const active = await screen.findByTestId('explore-group-active');
    const promoted = screen.getByTestId('explore-group-promoted');

    expect(groupLabel(active)).toBe('进行中 (2)');
    expect(groupLabel(promoted)).toBe('已转变更 (2)');
    expect(within(active).getAllByTestId('explore-item')).toHaveLength(2);
    expect(within(promoted).getAllByTestId('explore-item')).toHaveLength(2);
    // 进行中组不含已转变更条目（分组判据 = promotedTo）
    expect(namesIn(active)).toEqual(['active-b', 'active-a']);
    expect(namesIn(promoted)).toEqual(['promoted-b', 'promoted-a']);
  });

  it('正向：条目全局序 = 进行中时间倒序 ⊕ 已转变更时间倒序', async () => {
    // 乱序入参：组内 createdAt 与 id 序相反、组间混排
    records = [
      record(1, 'active-older', '进行中较早'),
      promotedRecord(4, 'promoted-newer', '已转较晚', 'chg-1'),
      record(3, 'active-newer', '进行中较晚'),
      promotedRecord(2, 'promoted-older', '已转较早', 'chg-2'),
    ];
    renderAt('/explores');

    await screen.findByTestId('explore-items');

    // createdAt = base + id ⇒ 组内 id 大者在前；组间按组序拼接
    const names = screen
      .getAllByTestId('explore-item')
      .map((node) => node.getAttribute('data-name'));
    expect(names).toEqual(['active-newer', 'active-older', 'promoted-newer', 'promoted-older']);
  });
});

// ---------------------------------------------------------------------------
// 已转变更组：点击进 promoted 态详情 / 清单内零直达 change 入口（AC-6）
// ---------------------------------------------------------------------------

describe('ExploreView 清单页：已转变更组的详情链与零新入口（AC-6）', () => {
  it('正向：已转变更组条目点击 → promoted 态详情（徽标 + 查看变更跳 /changes/<id>）', async () => {
    records = [promotedRecord(1, 'promoted-a', '已转主题甲', 'chg-1')];
    renderAt('/explores');

    await screen.findByTestId('explore-items');
    fireEvent.click(itemByName('promoted-a'));

    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/promoted-a'),
    );
    expect(screen.getByTestId('explore-promoted-badge').textContent).toBe('已转变更');
    // 清单页不进 promoted 详情路径以外的导航：直达 change 需详情页按钮
    fireEvent.click(screen.getByTestId('explore-promoted-jump'));
    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/changes/chg-1'),
    );
    expect(screen.getByTestId('change-detail-stub')).toBeTruthy();
  });

  it('异常：清单内零直达 change 入口（组内仅条目 / 标题 / 删除三 testid）', async () => {
    records = mixedRecords();
    renderAt('/explores');

    await screen.findByTestId('explore-items');

    expect(screen.queryByTestId('explore-promoted-jump')).toBeNull();
    expect(screen.queryByTestId('explore-promoted-badge')).toBeNull();
    for (const groupTestId of ['explore-group-active', 'explore-group-promoted']) {
      const testIds = new Set(
        within(screen.getByTestId(groupTestId))
          .queryAllByTestId(/explore-/)
          .map((node) => node.getAttribute('data-testid')),
      );
      expect(testIds).toEqual(
        new Set(['explore-item', 'explore-item-title', 'explore-item-delete']),
      );
    }
  });
});

// ---------------------------------------------------------------------------
// promote 移组链：详情页 promote → 重取 → 回清单落在已转变更组（AC-7）
// ---------------------------------------------------------------------------

describe('ExploreView：promote 成功后移组（AC-7）', () => {
  const activeApiRetry = (): ExploreRecord => record(2, 'api-retry', '接口重试策略');
  const activeLayout = (): ExploreRecord => record(1, 'layout-design', '布局设计');

  /** 应答序列：首次取数全为草稿态；promote 触发的第二次取数中 api-retry 已打标
   * （队列耗尽后稳定返回打标后的清单，避免多余重取打乱归组断言）。 */
  function mockPromoteFlow(queue: ExploreRecord[][]): void {
    const draftList = [activeApiRetry(), activeLayout()];
    const promotedList = [{ ...activeApiRetry(), promotedTo: 'chg-1' }, activeLayout()];
    queue.length = 0;
    queue.push(draftList, promotedList);
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return Promise.resolve(queue.shift() ?? promotedList);
      return ipc(command);
    });
  }

  it('正向：promote 成功重取后记录移入已转变更组、进行中组计数减一', async () => {
    const queue: ExploreRecord[][] = [];
    mockPromoteFlow(queue);
    render(viewTree('/explores/api-retry', ROOT, true));

    await screen.findByTestId('explore-detail-title');
    expect(screen.getByTestId('explore-detail-title').textContent).toBe('接口重试策略');

    fireEvent.click(screen.getByTestId('explore-promote-trigger'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('promote_explore', {
        root: ROOT,
        name: 'api-retry',
        workflowType: 'requirement',
      }),
    );
    // promote 动作薄封装 invoke 后 refresh → 第二次取数已出队
    await waitFor(() => expect(queue).toHaveLength(0));

    fireEvent.click(screen.getByTestId('list-nav-probe'));

    const promoted = await screen.findByTestId('explore-group-promoted');
    const active = screen.getByTestId('explore-group-active');
    expect(within(promoted).getAllByTestId('explore-item')).toHaveLength(1);
    expect(namesIn(promoted)).toEqual(['api-retry']);
    expect(groupLabel(active)).toBe('进行中 (1)');
    expect(namesIn(active)).toEqual(['layout-design']);
    expect(screen.queryByTestId('explore-list-empty')).toBeNull();
  });

  it('正向：promote 后零导航、详情不卸载（移组只在回清单时可见）', async () => {
    const queue: ExploreRecord[][] = [];
    mockPromoteFlow(queue);
    render(viewTree('/explores/api-retry', ROOT, true));

    await screen.findByTestId('explore-detail-title');
    const navTypeBefore = screen.getByTestId('nav-type-probe').textContent;

    fireEvent.click(screen.getByTestId('explore-promote-trigger'));
    await waitFor(() => expect(queue).toHaveLength(0));

    // URL 不迁移、无新 PUSH / REPLACE、详情面持续在场
    expect(screen.getByTestId('location-probe').textContent).toBe('/explores/api-retry');
    expect(screen.getByTestId('nav-type-probe').textContent).toBe(navTypeBefore);
    expect(screen.getByTestId('explore-detail-title')).toBeTruthy();
    // 移组只改清单数据，清单分组区未抢跑进入详情页
    expect(screen.queryByTestId('explore-items')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 空组占位 / 零记录 / 单条 / 同刻兜底 / 大清单 / 三态过渡（AC-5）
// ---------------------------------------------------------------------------

describe('ExploreView 清单页：分组空态与分组区边界（AC-5）', () => {
  it('边界：仅含已转变更记录 → 进行中组在场且空态占位、条目数为 0', async () => {
    records = [promotedRecord(1, 'promoted-a', '已转主题甲', 'chg-1')];
    renderAt('/explores');

    const active = await screen.findByTestId('explore-group-active');
    expect(active.textContent).toContain('无进行中的探索。');
    expect(groupLabel(active)).toBe('进行中 (0)');
    expect(within(active).queryAllByTestId('explore-item')).toHaveLength(0);
    const promoted = screen.getByTestId('explore-group-promoted');
    expect(within(promoted).getAllByTestId('explore-item')).toHaveLength(1);
  });

  it('边界：仅含进行中记录 → 已转变更组在场且空态占位、组序不变', async () => {
    records = [record(1, 'api-retry', '接口重试策略')];
    renderAt('/explores');

    const promoted = await screen.findByTestId('explore-group-promoted');
    expect(promoted.textContent).toContain('暂无已转变更。');
    expect(groupLabel(promoted)).toBe('已转变更 (0)');
    expect(within(promoted).queryAllByTestId('explore-item')).toHaveLength(0);
    expect(groupTestIds()).toEqual(['explore-group-active', 'explore-group-promoted']);
  });

  it('边界：零记录 → 清单整体空态在场、分组区与两组均不在场', async () => {
    records = [];
    renderAt('/explores');

    expect(await screen.findByTestId('explore-list-empty')).toBeTruthy();
    expect(screen.queryByTestId('explore-items')).toBeNull();
    expect(screen.queryByTestId('explore-group-active')).toBeNull();
    expect(screen.queryByTestId('explore-group-promoted')).toBeNull();
  });

  it('边界：单条记录（createdAt = 0）→ 进行中组计数 1、已转变更组占位在场且时间渲染不抛错', async () => {
    records = [{ ...record(1, 'solo', '单条主题'), createdAt: 0, updatedAt: 0 }];
    renderAt('/explores');

    const active = await screen.findByTestId('explore-group-active');
    expect(groupLabel(active)).toBe('进行中 (1)');
    expect(screen.getByTestId('explore-item-title').textContent).toBe('单条主题');
    expect(screen.getByTestId('explore-group-promoted').textContent).toContain('暂无已转变更。');
    // 纪元起点时间戳渲染不抛错（排序断言不以显示文本为据）
    expect(screen.getByTestId('explore-item')).toBeTruthy();
  });

  it('边界：同刻 createdAt 的两条记录（注入 id 与期望逆序）→ DOM 按 id 倒序', async () => {
    const sameTick = 1727000000000;
    records = [
      { ...record(1, 'id-small', '小 id'), createdAt: sameTick, updatedAt: sameTick },
      { ...record(2, 'id-large', '大 id'), createdAt: sameTick, updatedAt: sameTick },
    ];
    renderAt('/explores');

    await screen.findByTestId('explore-items');

    expect(namesIn(screen.getByTestId('explore-group-active'))).toEqual(['id-large', 'id-small']);
  });

  it('边界：120 条混合清单（createdAt 与 id 序相反）→ 组序恒定、计数与条目数一致', async () => {
    records = Array.from({ length: 120 }, (_, index) => {
      const id = index + 1;
      const createdAt = 1727000000000 + (200 - id);
      return {
        ...record(id, `topic-${id}`, `主题 ${id}`),
        promotedTo: id % 2 === 0 ? `chg-${id}` : null,
        createdAt,
        updatedAt: createdAt,
      };
    });
    renderAt('/explores');

    await screen.findByTestId('explore-items');

    const active = screen.getByTestId('explore-group-active');
    const promoted = screen.getByTestId('explore-group-promoted');
    expect(groupTestIds()).toEqual(['explore-group-active', 'explore-group-promoted']);
    expect(groupLabel(active)).toBe('进行中 (60)');
    expect(groupLabel(promoted)).toBe('已转变更 (60)');
    // createdAt 与 id 序相反 ⇒ 组内按 id 升序呈现
    expect(namesIn(active)).toEqual(Array.from({ length: 60 }, (_, k) => `topic-${k * 2 + 1}`));
    expect(namesIn(promoted)).toEqual(Array.from({ length: 60 }, (_, k) => `topic-${k * 2 + 2}`));
  });

  it('边界：加载中态 → 分组区不抢跑（两组 testid 与 explore-items 均缺席）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return new Promise<ExploreRecord[]>(() => {});
      return ipc(command);
    });
    renderAt('/explores');

    expect(await screen.findByTestId('explore-list-loading')).toBeTruthy();
    expect(screen.queryByTestId('explore-group-active')).toBeNull();
    expect(screen.queryByTestId('explore-group-promoted')).toBeNull();
    expect(screen.queryByTestId('explore-items')).toBeNull();
  });

  it('异常：取数失败态 → 错误块在场、分组区与空态均不在场（三态互斥）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_explore_records') return Promise.reject('清单查询失败');
      return ipc(command);
    });
    renderAt('/explores');

    const error = await screen.findByTestId('explore-list-error');
    expect(error.textContent).toContain('清单查询失败');
    expect(screen.queryByTestId('explore-list-empty')).toBeNull();
    expect(screen.queryByTestId('explore-group-active')).toBeNull();
    expect(screen.queryByTestId('explore-group-promoted')).toBeNull();
    expect(screen.queryByTestId('explore-items')).toBeNull();
  });

  it('正向：删除动作接线回归（已转变更组条目删除 → 携 root + name 并重取）', async () => {
    records = mixedRecords();
    renderAt('/explores');
    await screen.findByTestId('explore-items');
    const before = listCallCount();

    fireEvent.click(deleteButtonByName('promoted-a'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('delete_explore_record', {
        root: ROOT,
        name: 'promoted-a',
      }),
    );
    await waitFor(() => expect(listCallCount()).toBe(before + 1));
  });

  it('边界：draft-* 占位记录与非占位进行中记录同处「进行中」组（不另设第三组）', async () => {
    records = [placeholderRecord(), record(5, 'named-active', '已定名进行中')];
    renderAt('/explores');

    await screen.findByTestId('explore-items');

    expect(groupTestIds()).toEqual(['explore-group-active', 'explore-group-promoted']);
    const active = screen.getByTestId('explore-group-active');
    expect(groupLabel(active)).toBe('进行中 (2)');
    expect(namesIn(active)).toEqual(['named-active', PLACEHOLDER_NAME]);
  });
});

// ---------------------------------------------------------------------------
// 分组派生与既有回归面：根切换落清单 / id 桥三级链（AC-8）
// ---------------------------------------------------------------------------

describe('ExploreView：分组派生下的根切换与 id 桥回归（AC-8）', () => {
  const ROOT2 = 'C:\\demo\\beta';

  it('边界：根切换落清单后新根记录按分组呈现（旧根条目不在场）', async () => {
    const betaActive = { ...record(9, 'beta-active', '乙根进行中'), root: ROOT2 };
    const betaPromoted = {
      ...record(8, 'beta-promoted', '乙根已转'),
      root: ROOT2,
      promotedTo: 'chg-2',
    };
    invokeMock.mockImplementation((command: string, params?: { root?: string }) => {
      if (command === 'list_explore_records') {
        return Promise.resolve(params?.root === ROOT2 ? [betaActive, betaPromoted] : records);
      }
      return ipc(command);
    });
    const { rerender } = renderAtRoot('/explores/api-retry', ROOT);
    await waitFor(() =>
      expect(screen.getByTestId('explore-detail-title').textContent).toBe('接口重试策略'),
    );

    rerender(viewTree('/explores/api-retry', ROOT2));

    await waitFor(() => expect(screen.getByTestId('location-probe').textContent).toBe('/explores'));
    const active = await screen.findByTestId('explore-group-active');
    expect(groupLabel(active)).toBe('进行中 (1)');
    expect(namesIn(active)).toEqual(['beta-active']);
    expect(namesIn(screen.getByTestId('explore-group-promoted'))).toEqual(['beta-promoted']);
    const allNames = screen
      .getAllByTestId('explore-item')
      .map((node) => node.getAttribute('data-name'));
    expect(allNames).not.toContain('api-retry');
  });

  it('边界：旧名 URL + 改名后混合清单 → 详情照旧派生并 replace 收敛至新名', async () => {
    mockRenamedFlow([
      { ...record(1, 'new-final-name', '旧标题'), sessionId: 'ses-old-1727000001000' },
      promotedRecord(2, 'promoted-peer', '已转主题', 'chg-9'),
    ]);
    renderAt('/explores/draft-old-name');

    await waitFor(() =>
      expect(screen.getByTestId('explore-detail-title').textContent).toBe('旧标题'),
    );
    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/new-final-name'),
    );
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
  });
});
