// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type { ChangeDetail, ChangeList, ExploreRecord, WorkspaceRecord } from '../types/dto';

// ---------------------------------------------------------------------------
// 路由级集成矩阵（test-design「集成测试」全量场景收敛于此文件）：集成用例
// render(<App />) 驱动 App 自含的真实 HashRouter（design D6，零 Router 包裹、
// 零 mock Router），beforeEach 重置 window.location.hash（jsdom location 跨
// 用例存活）。覆盖五条关系：
//   ① 侧栏 NavLink → 路由表 → 页面渲染（AC-1/AC-3/AC-7）
//   ② 清单行点击 → /changes/:name → useChangeDetail 取数（AC-4/AC-7）
//   ③ / 与未知路径 → 重定向 /changes（AC-1）
//   ④ workspace 根切换 → ChangeView 过渡抑制 → navigate('/changes')（AC-5/AC-7）
//   ⑤ 欢迎态 gate → Router 子树挂载/卸载（AC-6）
// 另含「侧栏导航与路由 → /explores 路由面」关系（AC-10）：
//   ⑥ nav-explores active 派生 / 深链直达 / workspace 切换 replace 回清单 /
//      双段路径兜底（未知路径兜底由既有 #/totally-unknown 用例在新路由并存下
//      继续承载）
//
// 进程边界 Mock：invoke 按命令名分发并记录调用序列（次数/参数断言依赖记录）；
// Channel mock 为可编程 class（explore 详情页订阅链需要）；getVersion /
// dialog open / updater check 固定 resolve 隔离无关分支。链路内组件/hooks/
// 路由运行时全部真实实现。
// ---------------------------------------------------------------------------

const { ChannelMock, checkMock, getVersionMock, invokeMock, openMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
  }
  return {
    ChannelMock,
    checkMock: vi.fn(),
    getVersionMock: vi.fn(),
    invokeMock: vi.fn(),
    openMock: vi.fn(),
  };
});

vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: checkMock }));

// ---------------------------------------------------------------------------
// fixture（默认序：canonical root 升序，FIRST 即默认序第一名）
// ---------------------------------------------------------------------------

const FIRST: WorkspaceRecord = {
  root: 'C:\\demo\\alpha',
  name: 'alpha',
  addedAt: 1,
};
const SECOND: WorkspaceRecord = {
  root: 'C:\\demo\\beta',
  name: 'beta',
  addedAt: 1,
};

const fakeList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', inventory: 'v2', created: null, unparsable: false },
    { name: 'beta-fix', source: 'active', inventory: 'v1', created: null, unparsable: false },
  ],
  archiveGroups: [],
};

function detailDto(name: string): ChangeDetail {
  return {
    name,
    source: 'active',
    inventory: 'v2',
    created: null,
    unparsable: false,
    pipeline: [],
    activePhase: null,
    interrupted: [],
    fileLog: [],
    artifacts: [],
  };
}

function exploreRecordDto(name: string): ExploreRecord {
  return {
    id: name === 'foo' ? 1 : 2,
    root: FIRST.root,
    name,
    createdAt: 1727000000000,
    updatedAt: 1727000000000,
  };
}

// ---------------------------------------------------------------------------
// 可切换 mock 行为与按命令名分发
// ---------------------------------------------------------------------------

let remaining: WorkspaceRecord[];
let addRecord: WorkspaceRecord | null = null;
/** get_change_detail 命中表：未列名（ghost / 超长名等）返回 null 模拟后端未命中 */
let knownDetails: string[];
/** explore 清单 fixture：#root 记录按归属 root 过滤（AC-10 场景用） */
let exploreRecords: ExploreRecord[];

function mockIpc() {
  remaining = [FIRST, SECOND];
  addRecord = null;
  knownDetails = ['add-feature', 'beta-fix'];
  exploreRecords = [exploreRecordDto('foo'), exploreRecordDto('bar')];
  invokeMock.mockImplementation(
    (command: string, params?: { root?: string; change?: string; name?: string }) => {
      if (command === 'list_workspaces') {
        return Promise.resolve([...remaining]);
      }
      if (command === 'remove_workspace') {
        remaining = remaining.filter((r) => r.root !== params?.root);
        return Promise.resolve(true);
      }
      if (command === 'add_workspace') {
        const rec: WorkspaceRecord = addRecord ?? {
          root: params?.root ?? '',
          name: 'picked',
          addedAt: 1,
        };
        // 对齐后端：库存按默认序（canonical root 升序）返回，新记录未必居首
        remaining = [...remaining.filter((r) => r.root !== rec.root), rec].sort((a, b) =>
          a.root < b.root ? -1 : a.root > b.root ? 1 : 0,
        );
        return Promise.resolve(rec);
      }
      if (command === 'list_changes') {
        return Promise.resolve(fakeList);
      }
      if (command === 'get_change_detail') {
        const change = params?.change;
        const hit = change !== undefined && knownDetails.includes(change);
        return Promise.resolve(hit ? detailDto(change) : null);
      }
      if (command === 'list_explore_records') {
        return Promise.resolve(exploreRecords.filter((record) => record.root === params?.root));
      }
      if (command === 'create_explore_record') {
        const rec: ExploreRecord = {
          id: exploreRecords.length + 1,
          root: params?.root ?? '',
          name: params?.name ?? '',
          createdAt: 1727000000000,
          updatedAt: 1727000000000,
        };
        exploreRecords = [...exploreRecords, rec];
        return Promise.resolve(rec);
      }
      if (command === 'delete_explore_record') {
        exploreRecords = exploreRecords.filter((record) => record.name !== params?.name);
        return Promise.resolve(true);
      }
      // explore 详情恢复路（AC-10 场景只关注导航与清单取数，文档空态即可）
      if (command === 'read_explore') {
        return Promise.resolve(null);
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(null);
      }
      if (command === 'watch_subscribe') {
        return Promise.resolve(1);
      }
      if (command === 'watch_unsubscribe') {
        return Promise.resolve(true);
      }
      if (command === 'agent_run_chain') {
        return Promise.resolve([]);
      }
      if (command === 'agent_run_events') {
        return Promise.resolve([]);
      }
      if (command === 'scan_explores') {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    },
  );
}

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

/** mock.calls 中某命令的调用次数。 */
function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

/** 以 data-root 定位 sidebar 清单项（同名项由 data-root 区分）。 */
function itemByRoot(root: string): HTMLElement {
  const hit = screen
    .getAllByTestId('workspace-item')
    .find((item) => item.getAttribute('data-root') === root);
  if (!hit) throw new Error(`data-root 为 ${root} 的 workspace-item 不存在`);
  return hit;
}

/** 有记录启动：等待自动恢复第一名进入清单页（hash 已落 #/changes、列表已渲染）。 */
async function restored() {
  render(<App />);
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
  );
  await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
  expect(window.location.hash).toBe('#/changes');
}

/** 启动前预置 hash（深链场景：D7 直达恢复，用例内显式预置）。 */
async function bootAt(hash: string) {
  window.location.hash = hash;
  render(<App />);
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
  );
}

/** 壳态内改写 hash（路由监听 jsdom hashchange → HashRouter 状态更新）。 */
async function navigateHash(hash: string) {
  await act(async () => {
    window.location.hash = hash;
  });
}

/** 导航点击（react-router v7 navigate 经 transition 提交，act 包裹冲刷）。 */
async function clickNav(testId: string) {
  await act(async () => {
    fireEvent.click(screen.getByTestId(testId));
  });
}

beforeEach(() => {
  // design D6：集成用例 beforeEach 重置 window.location.hash（jsdom 跨用例存活，
  // 上一用例点击导航/深链残留的 hash 会改变下一用例启动路由初态）
  window.location.hash = '';
  getVersionMock.mockReset();
  invokeMock.mockReset();
  openMock.mockReset();
  checkMock.mockReset();
  checkMock.mockResolvedValue(null);
  getVersionMock.mockResolvedValue('0.1.0');
  toast.dismiss();
  mockIpc();
  // jsdom 环境缺口兜底（沿 AppSidebar.test.tsx 惯例）：explore 详情页的
  // react-resizable-panels / radix 定位需要 ResizeObserver
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe = vi.fn();
      unobserve = vi.fn();
      disconnect = vi.fn();
    },
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// 关系①：侧栏 NavLink → HashRouter 路由表 → 页面渲染（AC-1/AC-3/AC-7）
// ---------------------------------------------------------------------------

describe('route_pages：侧栏点击切换页面路由', () => {
  it('restored 后点击 nav-agent → hash 落 #/agent、agent-run-form 在场、清单内容卸载、nav-agent data-active=true', async () => {
    await restored();

    await clickNav('nav-agent');

    await waitFor(() => expect(window.location.hash).toBe('#/agent'));
    expect(screen.getByTestId('agent-run-form') !== null).toBe(true);
    expect(screen.queryByText('add-feature')).toBeNull();
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('false');
  });

  it('再点击 nav-changes → hash 回 #/changes、清单呈现、agent-run-form 卸载、nav-changes 呈激活态', async () => {
    await restored();
    await clickNav('nav-agent');
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));

    await clickNav('nav-changes');

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');
  });

  it('详情态（hash=#/changes/add-feature）下 nav-changes 仍激活（前缀派生）；点击 nav-changes → 落 #/changes 呈现清单', async () => {
    await restored();
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() => expect(window.location.hash).toBe('#/changes/add-feature'));

    // 前缀派生：详情态不变更项不失活
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');

    await clickNav('nav-changes');

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
  });

  it('详情 → 切 Agent → 切回 changes：清单呈现、get_change_detail 总调用次数保持不变（不以旧选中重发）', async () => {
    await restored();
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );

    await clickNav('nav-agent');
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));
    await clickNav('nav-changes');
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(countOf('get_change_detail')).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// 关系①：导航零命令副作用与清单数据驻留（AC-7）
// ---------------------------------------------------------------------------

describe('route_pages：导航零命令副作用与清单数据驻留', () => {
  it('agent ↔ changes 往返点击后：list_workspaces / list_changes / add_workspace / remove_workspace 调用计数与基线一致（导航零命令）', async () => {
    await restored();

    const baseline = {
      workspaces: countOf('list_workspaces'),
      changes: countOf('list_changes'),
      add: countOf('add_workspace'),
      remove: countOf('remove_workspace'),
    };

    await clickNav('nav-agent');
    await clickNav('nav-changes');
    await clickNav('nav-agent');
    await clickNav('nav-changes');
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    expect(countOf('list_workspaces')).toBe(baseline.workspaces);
    expect(countOf('list_changes')).toBe(baseline.changes);
    expect(countOf('add_workspace')).toBe(baseline.add);
    expect(countOf('remove_workspace')).toBe(baseline.remove);
  });

  it('切往 agent 再切回 changes：清单内容无需重取即时呈现、list_changes 不重发（数据驻留 App 层）', async () => {
    await restored();

    await clickNav('nav-agent');
    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));
    await clickNav('nav-changes');

    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(countOf('list_changes')).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// 关系①：深链直达 agent 页（AC-1/AC-7）
// ---------------------------------------------------------------------------

describe('route_pages：深链直达 agent 页', () => {
  it('hash 预置 #/agent 启动 → AgentDebugView 直渲染、nav-agent data-active=true、无任何 NavLink 点击发生', async () => {
    await bootAt('#/agent');

    await waitFor(() => expect(screen.getByTestId('agent-run-form') !== null).toBe(true));
    expect(window.location.hash).toBe('#/agent');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('false');
    expect(screen.queryByText('add-feature')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 关系②：清单行点击 → /changes/:name → useChangeDetail 取数（AC-4/AC-7）
// ---------------------------------------------------------------------------

describe('route_pages：行点击进入详情与返回落清单', () => {
  it('点击 add-feature 行 → hash 变为 #/changes/add-feature、get_change_detail 以 {root, change} 恰一次发起、详情 heading 在场', async () => {
    await restored();

    fireEvent.click(screen.getByText('add-feature'));

    await waitFor(() => expect(window.location.hash).toBe('#/changes/add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );
    expect(countOf('get_change_detail')).toBe(1);
    expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true);
  });

  it('详情页点击「← 返回列表」→ hash 回 #/changes、清单呈现、list_changes 不重发（list 驻留 App 层）、刷新入口重新在场', async () => {
    await restored();
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );

    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(screen.getByText('刷新列表') !== null).toBe(true);
    expect(countOf('list_changes')).toBe(1);
  });

  it('先后进入 add-feature、返回、进入 beta-fix：两次 get_change_detail 参数分别为两者（URL 参数实时透传，无旧参残留）', async () => {
    await restored();

    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );
    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    await waitFor(() => expect(window.location.hash).toBe('#/changes'));

    fireEvent.click(screen.getByText('beta-fix'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenLastCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'beta-fix',
      }),
    );

    const detailCalls = invokeMock.mock.calls.filter(([name]) => name === 'get_change_detail');
    expect(detailCalls.map(([, params]) => params?.change)).toEqual(['add-feature', 'beta-fix']);
  });
});

// ---------------------------------------------------------------------------
// 关系②：深链直达详情与不存在 change 降级（AC-1/AC-4，D7）
// ---------------------------------------------------------------------------

describe('route_pages：深链直达详情与不存在 change 降级', () => {
  it('hash 预置 #/changes/add-feature 启动 → 详情直渲染、get_change_detail 以 URL 参数发起、无行点击发生', async () => {
    await bootAt('#/changes/add-feature');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );
    expect(countOf('get_change_detail')).toBe(1);
    expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true);
    expect(window.location.hash).toBe('#/changes/add-feature');
  });

  it('hash 预置 #/changes/ghost（不存在的 change）启动 → 既有「未找到该 change」降级呈现，不崩、不发起 ghost 之外的参数取数', async () => {
    await bootAt('#/changes/ghost');

    await waitFor(() => expect(screen.getByText('未找到该 change。') !== null).toBe(true));
    const detailCalls = invokeMock.mock.calls.filter(([name]) => name === 'get_change_detail');
    expect(detailCalls).toHaveLength(1);
    expect(detailCalls[0]).toEqual(['get_change_detail', { root: FIRST.root, change: 'ghost' }]);
    // 降级不崩：返回入口在场
    expect(screen.getByRole('button', { name: '← 返回列表' }) !== null).toBe(true);
  });

  it('hash 预置超长 name 段（>1000 字符）启动 → 参数透传不崩、呈现交由 detail 态决定（URL 参数边界鲁棒性）', async () => {
    const longName = `long-change-${'a'.repeat(1000)}`;
    await bootAt(`#/changes/${longName}`);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: longName,
      }),
    );
    // 未命中 → 降级页兜底，组件不白屏不崩
    expect(screen.getByText('未找到该 change。') !== null).toBe(true);
    expect(countOf('get_change_detail')).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// 关系②：畸形多段路径兜底（AC-1）
// ---------------------------------------------------------------------------

describe('route_pages：畸形多段路径兜底', () => {
  it('hash 预置 #/changes/a/b（双段超出 :name 单参）启动 → 兜底落 #/changes 渲染清单，不空白不崩', async () => {
    await bootAt('#/changes/a/b');

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    // 落清单而非详情：无 detail 取数、无详情标题
    expect(countOf('get_change_detail')).toBe(0);
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 关系③：/ 与未知路径 → 重定向 /changes（AC-1）
// ---------------------------------------------------------------------------

describe('route_pages：启动根路径重定向与首屏落点', () => {
  it('常规启动（hash 为空）→ hash 落 #/changes、清单内容渲染、取数序列为 list_workspaces + list_changes 各一次（无重定向引发的多余调用）', async () => {
    await restored();

    expect(window.location.hash).toBe('#/changes');
    // 空 hash 默认激活变更项（「缺省 changes」语义现由 / 重定向承载，侧栏按 URL 派生）
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-db').getAttribute('data-active')).toBe('false');
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(invokeMock.mock.calls.map(([name]) => name)).toEqual([
      'list_workspaces',
      'list_changes',
    ]);
    expect(countOf('get_change_detail')).toBe(0);
  });
});

describe('route_pages：未知路径兜底与欢迎态进壳落点', () => {
  it('壳态内将 hash 改写为 #/totally-unknown → 兜底落 #/changes 渲染清单，不空白不崩', async () => {
    await restored();

    await navigateHash('#/totally-unknown');

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(countOf('list_changes')).toBe(1);
  });

  it('欢迎态添加 workspace 成功进入壳态（Router 初次挂载、起点为 /）→ 落 #/changes 渲染清单（重定向对 Router 首挂同样生效）', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    addRecord = { root: 'C:\\canonical\\picked', name: 'picked', addedAt: 1 };
    openMock.mockResolvedValue('/raw/PICKED DIR');
    fireEvent.click(screen.getByText('添加新文件夹'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '/raw/PICKED DIR' }),
    );
    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(screen.getByTestId('nav-changes') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 关系④：workspace 根切换 → ChangeView 过渡抑制 → navigate('/changes')（AC-5/AC-7）
// ---------------------------------------------------------------------------

describe('route_pages：切换根清选中落清单', () => {
  it('详情态点击另一 workspace 项 → hash 落 #/changes、清单呈现、list_changes 以新根（SECOND.root）发起', async () => {
    await restored();
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );

    fireEvent.click(itemByRoot(SECOND.root));

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
    const listCalls = invokeMock.mock.calls.filter(([name]) => name === 'list_changes');
    expect(listCalls.at(-1)).toEqual(['list_changes', { root: SECOND.root }]);
  });

  it('上述切换全程检查 get_change_detail 全部调用：root 均为旧根（FIRST.root），不存在「新根 + 旧名」组合（过渡轮抑制生效，AC-7）', async () => {
    await restored();
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );

    fireEvent.click(itemByRoot(SECOND.root));

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    const detailCalls = invokeMock.mock.calls.filter(([name]) => name === 'get_change_detail');
    expect(detailCalls).toHaveLength(1);
    expect(detailCalls[0]).toEqual([
      'get_change_detail',
      { root: FIRST.root, change: 'add-feature' },
    ]);
  });

  it('清单态（无选中）切换根 → hash 保持 #/changes 无跳变、仅 list_changes 以新根重发（抑制位不置位、无导航发生）', async () => {
    await restored();

    fireEvent.click(itemByRoot(SECOND.root));

    await waitFor(() => {
      const lastListChanges = invokeMock.mock.calls
        .filter(([name]) => name === 'list_changes')
        .at(-1);
      expect(lastListChanges).toEqual(['list_changes', { root: SECOND.root }]);
    });
    expect(window.location.hash).toBe('#/changes');
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(countOf('get_change_detail')).toBe(0);
  });
});

describe('route_pages：移除当前根与添加新根落清单', () => {
  it('详情态右键当前根 → 「移除」→ 顺延剩余第一名 → hash 落 #/changes、list_changes 以顺延根发起', async () => {
    // 默认 remaining=[FIRST, SECOND]（beforeEach mockIpc）：移除 FIRST 顺延 SECOND
    await bootAt('#/changes/add-feature');
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );

    fireEvent.contextMenu(itemByRoot(FIRST.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    const listCalls = invokeMock.mock.calls.filter(([name]) => name === 'list_changes');
    expect(listCalls.at(-1)).toEqual(['list_changes', { root: SECOND.root }]);
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
  });

  it('详情态经 GroupAction 添加新根（对话框 resolve 新路径、add_workspace 返回 canonical root）→ hash 落 #/changes、清单以新根呈现（添加流在 Router 外，导航由 ChangeView 单点收口，D4）', async () => {
    await bootAt('#/changes/add-feature');
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );

    addRecord = { root: 'C:\\canonical\\picked', name: 'picked', addedAt: 1 };
    openMock.mockResolvedValue('/raw/PICKED DIR');
    fireEvent.click(screen.getByRole('button', { name: '添加 workspace' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: '/raw/PICKED DIR' }),
    );
    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    const listCalls = invokeMock.mock.calls.filter(([name]) => name === 'list_changes');
    expect(listCalls.at(-1)).toEqual(['list_changes', { root: 'C:\\canonical\\picked' }]);
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
  });

  it('移除当前根至空清单 → 退出壳态回欢迎态：不残留旧 hash 渲染（Router 子树卸载面归欢迎态 gate 关系）', async () => {
    remaining = [FIRST];
    window.location.hash = '#/changes/add-feature';
    render(<App />);
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'add-feature' }) !== null).toBe(true),
    );

    fireEvent.contextMenu(itemByRoot(FIRST.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));
    // 旧 hash 残留不再驱动任何壳渲染：无壳 DOM、无 nav
    expect(document.querySelector('[data-slot="sidebar-wrapper"]')).toBeNull();
    expect(screen.queryByTestId('nav-changes')).toBeNull();
    expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull();
    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 关系⑤：欢迎态 gate → Router 子树挂载/卸载（AC-6）
// ---------------------------------------------------------------------------

describe('route_pages：欢迎态路由隔离与双向迁移', () => {
  it('空清单启动 → WelcomeView 全屏文案在场、无 sidebar-wrapper / header / nav-changes / nav-agent、Toaster 在场', async () => {
    remaining = [];
    render(<App />);

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
    expect(document.querySelector('[data-slot="sidebar-wrapper"]')).toBeNull();
    expect(document.querySelector('header')).toBeNull();
    expect(screen.queryByTestId('nav-changes')).toBeNull();
    expect(screen.queryByTestId('nav-agent')).toBeNull();
    expect(document.querySelector('section[aria-label^="Notifications"]') !== null).toBe(true);
  });

  it('壳态右键移除唯一根至空清单 → Router 子树卸载回欢迎态：无壳 DOM 残留、WelcomeView 呈现、不因 Router 卸载崩溃', async () => {
    remaining = [FIRST];
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(document.querySelector('[data-slot="sidebar-wrapper"]') !== null).toBe(true);

    fireEvent.contextMenu(itemByRoot(FIRST.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));
    expect(document.querySelector('[data-slot="sidebar-wrapper"]')).toBeNull();
    expect(screen.queryByTestId('nav-changes')).toBeNull();
    expect(screen.queryByTestId('nav-agent')).toBeNull();
    expect(screen.getByText(/还没有记录/) !== null).toBe(true);
    expect(screen.getByText(/选择一个项目根目录/) !== null).toBe(true);
  });

  it('欢迎态添加 workspace 成功 → 壳 DOM 与 nav testid 在场、Router 挂载落 #/changes 渲染清单', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    openMock.mockResolvedValue('C:\\picked');
    fireEvent.click(screen.getByText('添加新文件夹'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: 'C:\\picked' }),
    );
    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    expect(document.querySelector('[data-slot="sidebar-wrapper"]') !== null).toBe(true);
    expect(screen.getByTestId('nav-changes') !== null).toBe(true);
    expect(screen.getByTestId('nav-agent') !== null).toBe(true);
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
  });
});

// ---------------------------------------------------------------------------
// 关系⑥：侧栏导航与路由 → /explores 路由面（AC-10）
// ---------------------------------------------------------------------------

describe('route_pages：nav-explores 点击与 active 派生（AC-10）', () => {
  it('restored 后点击 nav-explores → hash 落 #/explores、自身 active 他项失活、list_explore_records 以当前 root 发起', async () => {
    await restored();

    await clickNav('nav-explores');

    await waitFor(() => expect(window.location.hash).toBe('#/explores'));
    expect(screen.getByTestId('nav-explores').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_explore_records', { root: FIRST.root }),
    );
    expect(screen.getByTestId('explore-list') !== null).toBe(true);
    expect(screen.getByText('foo') !== null).toBe(true);
  });

  it('详情态（hash=#/explores/foo）下 nav-explores 仍 active（前缀派生）；点击 nav-explores 回落 #/explores 清单', async () => {
    await bootAt('#/explores/foo');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('read_explore', { root: FIRST.root, name: 'foo' }),
    );
    expect(window.location.hash).toBe('#/explores/foo');
    expect(screen.getByTestId('nav-explores').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('false');

    await clickNav('nav-explores');

    await waitFor(() => expect(window.location.hash).toBe('#/explores'));
    expect(screen.getByTestId('explore-list') !== null).toBe(true);
    expect(screen.getByTestId('nav-explores').getAttribute('data-active')).toBe('true');
  });

  it('深链 #/explores/foo 启动直达详情态：read_explore 以 URL 参数发起、无 NavLink 点击发生', async () => {
    await bootAt('#/explores/foo');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('read_explore', { root: FIRST.root, name: 'foo' }),
    );
    expect(countOf('read_explore')).toBe(1);
    expect(window.location.hash).toBe('#/explores/foo');
    expect(screen.getByTestId('explore-detail') !== null).toBe(true);
    expect(screen.getByTestId('nav-explores').getAttribute('data-active')).toBe('true');
  });

  it('changes / agent / explores 往返后各 nav 项 active 始终与 URL 一致（既有 active 语义回归）', async () => {
    await restored();

    await clickNav('nav-agent');
    await waitFor(() => expect(window.location.hash).toBe('#/agent'));
    expect(screen.getByTestId('nav-explores').getAttribute('data-active')).toBe('false');

    await clickNav('nav-explores');
    await waitFor(() => expect(window.location.hash).toBe('#/explores'));
    expect(screen.getByTestId('nav-explores').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');

    await clickNav('nav-changes');
    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    expect(screen.getByTestId('nav-explores').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
  });
});

describe('route_pages：explores workspace 切换 replace 回清单与畸形路径兜底（AC-10）', () => {
  it('详情态点击另一 workspace 项 → hash replace 回 #/explores、全程无「新 root + 旧 name」取数', async () => {
    await bootAt('#/explores/foo');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('read_explore', { root: FIRST.root, name: 'foo' }),
    );

    fireEvent.click(itemByRoot(SECOND.root));

    await waitFor(() => expect(window.location.hash).toBe('#/explores'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_explore_records', { root: SECOND.root }),
    );
    // 过渡抑制（ChangeView 模式平移）：read_explore 只以旧根发起过，无新根 + 旧名组合
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    const readCalls = invokeMock.mock.calls.filter(([name]) => name === 'read_explore');
    expect(readCalls).toHaveLength(1);
    expect(readCalls[0]).toEqual(['read_explore', { root: FIRST.root, name: 'foo' }]);
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
  });

  it('未知路径 #/totally-unknown 在新路由并存下仍兜底 #/changes（AC-10 新路由不破坏兜底）', async () => {
    await restored();

    await navigateHash('#/totally-unknown');

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
  });

  it('hash 预置 #/explores/a/b（双段超出 :name 单参）启动 → 兜底落 #/changes 渲染清单，不空白不崩', async () => {
    await bootAt('#/explores/a/b');

    await waitFor(() => expect(window.location.hash).toBe('#/changes'));
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    expect(countOf('read_explore')).toBe(0);
    expect(countOf('list_explore_records')).toBe(0);
    expect(screen.queryByTestId('explore-detail')).toBeNull();
  });
});
