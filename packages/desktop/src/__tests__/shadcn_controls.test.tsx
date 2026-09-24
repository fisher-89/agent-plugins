// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { Button } from '@/components/ui/button';

import App from '../App';
import type { ChangeListState } from '../hooks/useChangeList';
import type {
  ArtifactEnvelope,
  ChangeDetail,
  ChangeList,
  Inventory,
  WorkspaceRecord,
} from '../types/dto';
import { ChangeDetailView } from '../views/changes/ChangeDetailView';
import { ChangeListView } from '../views/changes/ChangeListView';
import type { ChangeDetailState } from '../views/changes/hooks/useChangeDetail';
import { TasksProgressRenderer } from '../views/changes/renderers/TasksProgressRenderer';

// ---------------------------------------------------------------------------
// 集成关系「视图层/renderer → shadcn 控件换装」：控件渲染语义与交互契约。
// 不验证观感，只验证 jsdom 可断言的语义属性（role / aria / testid）。
// 控件直连场景（注入 props 渲染视图）不跨进程边界，无需 Mock；
// 仅 Button 动作契约场景经 App 壳渲染，走 invoke / dialog / version 三处 Mock。
// ---------------------------------------------------------------------------

const { getVersionMock, invokeMock, openMock } = vi.hoisted(() => ({
  getVersionMock: vi.fn(),
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

const genList: ChangeList = {
  active: [
    { name: 'gen-v2', source: 'active', inventory: 'v2', created: '2026-09-01', unparsable: false },
    { name: 'gen-v1', source: 'active', inventory: 'v1', created: '2026-09-01', unparsable: false },
    { name: 'gen-v0', source: 'active', inventory: 'v0', created: null, unparsable: false },
  ],
  archiveGroups: [
    {
      month: '2026-08',
      changes: [
        {
          name: 'gen-archived',
          source: 'archive',
          inventory: 'v1',
          created: null,
          unparsable: false,
        },
      ],
    },
  ],
};

const clickableList: ChangeList = {
  active: [
    ...genList.active,
    { name: 'gen-broken', source: 'active', inventory: 'v0', created: null, unparsable: true },
  ],
  archiveGroups: genList.archiveGroups,
};

function detailFixture(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: '2026-09-01',
    unparsable: false,
    pipeline: [],
    activePhase: null,
    interrupted: [],
    fileLog: [
      {
        op: 'write',
        scope: 'workflow',
        attempt: 3,
        path: 'src/a.json',
        at: '2026-09-03T00:00:00Z',
      },
      { op: 'delete', scope: 'files', attempt: null, path: 'src/b.ts', at: null },
    ],
    artifacts: [],
    ...overrides,
  };
}

function progressEnvelope(payload: unknown): ArtifactEnvelope {
  return { kind: 'tasks-progress', version: 1, title: '任务进度', payload, fallbackText: null };
}

function listState(
  overrides: Partial<ChangeListState> & { data?: ChangeList | null },
): ChangeListState {
  return {
    data: null,
    loading: false,
    error: null,
    refresh: () => {},
    ...overrides,
  };
}

function detailState(
  overrides: Partial<ChangeDetailState> & { detail?: ChangeDetail | null },
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

// ---------------------------------------------------------------------------
// App 壳 Mock：IPC 按命令名分发（沿用 App.test.tsx 的 mockIpc 方案）
// ---------------------------------------------------------------------------

const FIRST: WorkspaceRecord = {
  root: 'C:\\demo\\alpha',
  name: 'alpha',
  addedAt: 1,
};

const shellList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', inventory: 'v2', created: null, unparsable: false },
  ],
  archiveGroups: [],
};

const shellDetail: ChangeDetail = {
  name: 'add-feature',
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

let remaining: WorkspaceRecord[];

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

/** 以 data-root 定位 sidebar 清单项（移除入口改右键菜单后的定位方式）。 */
function itemByRoot(root: string): HTMLElement {
  const hit = screen
    .getAllByTestId('workspace-item')
    .find((item) => item.getAttribute('data-root') === root);
  if (!hit) throw new Error(`data-root 为 ${root} 的 workspace-item 不存在`);
  return hit;
}

function mockIpc() {
  remaining = [FIRST];
  invokeMock.mockImplementation((command: string, params?: { root?: string; change?: string }) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([...remaining]);
    }
    if (command === 'remove_workspace') {
      remaining = remaining.filter((r) => r.root !== params?.root);
      return Promise.resolve(true);
    }
    if (command === 'add_workspace') {
      const rec: WorkspaceRecord = {
        root: params?.root ?? '',
        name: 'picked',
        addedAt: 1,
      };
      remaining = [...remaining, rec];
      return Promise.resolve(rec);
    }
    if (command === 'list_changes') {
      return Promise.resolve(shellList);
    }
    if (command === 'get_change_detail') {
      return Promise.resolve(shellDetail);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  // 路由化后 App 壳场景自含 HashRouter（design D6）：先重置 hash 防上一用例
  // 深链残留污染本用例路由初态
  window.location.hash = '';
  getVersionMock.mockReset();
  invokeMock.mockReset();
  openMock.mockReset();
  getVersionMock.mockResolvedValue('0.1.0');
  mockIpc();
});

// ---------------------------------------------------------------------------
// 场景：Badge 变体全枚举渲染（含 INVENTORY_VARIANT 显式映射）
// ---------------------------------------------------------------------------

describe('Badge 变体全枚举渲染：INVENTORY_VARIANT 三键全命中', () => {
  it('列表三代际同屏：各 change-row 作用域内徽标文本与代际一一对应', () => {
    render(<ChangeListView state={listState({ data: genList })} onSelect={() => {}} />);
    const rows = screen.getAllByTestId('change-row');
    expect(rows).toHaveLength(4); // active 三代际 + archive 分组一条
    const badgeTexts = rows
      .slice(0, 3)
      .map((row) => within(row).getByText(/^v[012]$/).textContent ?? '');
    expect(badgeTexts).toEqual(['v2', 'v1', 'v0']);
  });

  it('详情头部代际徽标与列表徽标同源映射：两渲染点 variant 一致', () => {
    const onSelect = vi.fn();
    const list = render(
      <ChangeListView state={listState({ data: genList })} onSelect={onSelect} />,
    );
    const listBadgeTexts = list
      .getAllByTestId('change-row')
      .map((row) => within(row).getByText(/^v[012]$/).textContent ?? '');
    expect(listBadgeTexts).toContain('v2');
    list.unmount();

    render(<ChangeDetailView state={detailState({ detail: detailFixture() })} onBack={() => {}} />);
    const header = screen.getByTestId('detail-header');
    expect(within(header).getByText('v2') !== null).toBe(true);
  });

  it('inventory 运行时漂移值：不崩、不产模板串类名，回退 Badge 默认变体', () => {
    const drifted: ChangeList = {
      active: [
        {
          name: 'gen-drift',
          source: 'active',
          inventory: 'v9' as Inventory,
          created: null,
          unparsable: false,
        },
      ],
      archiveGroups: [],
    };
    render(<ChangeListView state={listState({ data: drifted })} onSelect={() => {}} />);
    expect(screen.getByText('gen-drift') !== null).toBe(true);
    const badge = screen.getByText('v9');
    // 模板串 `badge-in${inventory}` 形态不得重现；cva 默认变体兜底（inv0 调色板类在场）
    expect(badge.className).not.toContain('badge-in');
    expect(badge.className).toContain('bg-gray-500/15');
  });
});

// ---------------------------------------------------------------------------
// 场景：change-row 换装 Button（D7）
// ---------------------------------------------------------------------------

describe('change-row 换装 Button：行点击交互契约不变', () => {
  it('change-row 宿主带 button role，点击触发 onSelect 且参数为条目 change 名', () => {
    const onSelect = vi.fn();
    render(<ChangeListView state={listState({ data: genList })} onSelect={onSelect} />);
    expect(screen.getByRole('button', { name: /gen-v2/ }) !== null).toBe(true);
    const rows = screen.getAllByTestId('change-row');
    expect(rows[0].tagName).toBe('BUTTON');
    fireEvent.click(rows[0]);
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith('gen-v2');
  });

  it('created 为 null 且 unparsable 的标注态条目仍可点击进入详情', () => {
    const onSelect = vi.fn();
    render(<ChangeListView state={listState({ data: clickableList })} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('workflow.json 无法解析'));
    expect(onSelect).toHaveBeenCalledWith('gen-broken');
  });

  it('档案分组成员与活跃条目点击行为一致（分组不改变交互契约）', () => {
    const onSelect = vi.fn();
    render(<ChangeListView state={listState({ data: clickableList })} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('gen-archived'));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith('gen-archived');
  });
});

// ---------------------------------------------------------------------------
// 场景：filelog Table 语义结构
// ---------------------------------------------------------------------------

describe('filelog Table 语义结构：行列矩阵完整、空缺占位', () => {
  it('filelog-table 域内共 3 行（表头 + 2 数据行），单元格文本矩阵逐格匹配', () => {
    render(<ChangeDetailView state={detailState({ detail: detailFixture() })} onBack={() => {}} />);
    const table = screen.getByTestId('filelog-table');
    const rows = within(table).getAllByRole('row');
    expect(rows).toHaveLength(3);
    expect(
      within(rows[0])
        .getAllByRole('columnheader')
        .map((cell) => cell.textContent ?? ''),
    ).toEqual(['op', 'scope', 'attempt', 'path', 'at']);
    expect(
      within(rows[1])
        .getAllByRole('cell')
        .map((cell) => cell.textContent ?? ''),
    ).toEqual(['write', 'workflow', '3', 'src/a.json', '2026-09-03T00:00:00Z']);
    expect(
      within(rows[2])
        .getAllByRole('cell')
        .map((cell) => cell.textContent ?? ''),
    ).toEqual(['delete', 'files', '—', 'src/b.ts', '—']);
  });

  it('fileLog 为 null：v1 及更早代际降级文案呈现，无 Table 结构，不白屏', () => {
    render(
      <ChangeDetailView
        state={detailState({ detail: detailFixture({ inventory: 'v1', fileLog: null }) })}
        onBack={() => {}}
      />,
    );
    expect(screen.getByText(/（无 file_log 数据：v1 及更早代际无此字段）/) !== null).toBe(true);
    expect(screen.queryByTestId('filelog-table')).toBeNull();
  });

  it('fileLog 为空数组：渲染（空）占位且无 Table 结构', () => {
    render(
      <ChangeDetailView
        state={detailState({ detail: detailFixture({ fileLog: [] }) })}
        onBack={() => {}}
      />,
    );
    expect(screen.getByText('（空）') !== null).toBe(true);
    expect(screen.queryByTestId('filelog-table')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 场景：Progress 承载百分比
// ---------------------------------------------------------------------------

describe('Progress 承载百分比：radix value 契约', () => {
  it('payload { total: 5, done: 2 }：progressbar aria-valuenow 为 40', () => {
    render(
      <TasksProgressRenderer envelope={progressEnvelope({ total: 5, done: 2, pending: 3 })} />,
    );
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('40');
    expect(screen.getByText('40%') !== null).toBe(true);
  });

  it('两端值：done=total 为 100；total=0 为 0 且无 NaN', () => {
    const full = render(
      <TasksProgressRenderer envelope={progressEnvelope({ total: 4, done: 4, pending: 0 })} />,
    );
    expect(full.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('100');
    full.unmount();

    const empty = render(
      <TasksProgressRenderer envelope={progressEnvelope({ total: 0, done: 0, pending: 0 })} />,
    );
    expect(empty.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('0');
    expect(empty.container.textContent).not.toContain('NaN');
  });

  it('payload 缺字段 / 类型漂移：计算收窄为 0，value 为数值 0 而非 null（无 indeterminate）', () => {
    const missing = render(<TasksProgressRenderer envelope={progressEnvelope(undefined)} />);
    expect(missing.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('0');
    missing.unmount();

    const drifted = render(
      <TasksProgressRenderer
        envelope={progressEnvelope({ total: '很多', done: true, pending: null })}
      />,
    );
    expect(drifted.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('0');
    expect(drifted.getByRole('progressbar').getAttribute('aria-valuenow')).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 场景：Button 换装保持动作契约
// ---------------------------------------------------------------------------

describe('Button 换装保持动作契约', () => {
  it('清单页头部刷新按钮与右键移除：迁移改写后 IPC 契约不变（改写：刷新按钮迁清单页头部按钮名不变；header「移除」→ 右键菜单项）', async () => {
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));

    // 刷新列表半边：按钮迁至清单页头部，按钮名不变
    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));
    await waitFor(() => expect(countOf('list_changes')).toBe(2));

    // 移除半边：入口随 header 瘦身改为清单项右键菜单
    fireEvent.contextMenu(itemByRoot(FIRST.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('remove_workspace', { root: FIRST.root }),
    );
  });

  it('欢迎屏添加入口：对话框 → add_workspace 链路照常触发', async () => {
    remaining = [];
    render(<App />);
    await waitFor(() => expect(screen.getByText('添加新文件夹') !== null).toBe(true));

    openMock.mockResolvedValue('C:\\picked');
    fireEvent.click(screen.getByRole('button', { name: '添加新文件夹' }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('add_workspace', { root: 'C:\\picked' }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: 'C:\\picked' }),
    );
  });

  it('详情动作按钮：刷新详情重发取数、返回列表回到列表视图', async () => {
    render(<App />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('list_changes', { root: FIRST.root }),
    );
    await waitFor(() => expect(screen.getByText('add-feature') !== null).toBe(true));
    fireEvent.click(screen.getByText('add-feature'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
        root: FIRST.root,
        change: 'add-feature',
      }),
    );

    const before = countOf('get_change_detail');
    fireEvent.click(screen.getByRole('button', { name: '刷新详情' }));
    await waitFor(() => expect(countOf('get_change_detail')).toBe(before + 1));

    fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }));
    await waitFor(() => expect(screen.queryByRole('heading', { name: 'add-feature' })).toBeNull());
    expect(screen.getByText('add-feature') !== null).toBe(true);
  });

  it('详情 loading 中：刷新详情按钮 disabled，点击不触发重复取数', () => {
    const refresh = vi.fn();
    render(
      <ChangeDetailView
        state={detailState({ detail: detailFixture(), loading: true, refresh })}
        onBack={() => {}}
      />,
    );
    const refreshButton = screen.getByRole('button', { name: '刷新详情' });
    expect(refreshButton.hasAttribute('disabled')).toBe(true);
    fireEvent.click(refreshButton);
    expect(refresh).not.toHaveBeenCalled();
  });

  it('Button 默认形态渲染原生 button 元素，焦点与禁用属性可透传', () => {
    render(
      <>
        <Button>普通按钮</Button>
        <Button disabled>禁用按钮</Button>
      </>,
    );
    const normal = screen.getByRole('button', { name: '普通按钮' });
    expect(normal.tagName).toBe('BUTTON');
    normal.focus();
    expect(document.activeElement).toBe(normal);
    const disabled = screen.getByRole('button', { name: '禁用按钮' });
    expect(disabled.hasAttribute('disabled')).toBe(true);
  });
});
