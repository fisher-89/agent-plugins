// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { MemoryRouter, useLocation } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { SidebarProvider } from '@/components/ui/sidebar';

import type { WorkspaceRecord } from '../types/dto';
import { AppSidebar } from './app-sidebar';

// ---------------------------------------------------------------------------
// AppSidebar 单测：workspace 组纯回调驱动（onOpen / onAdd / onRemove 以 vi.fn()
// 注入），无进程边界；页面导航组为 NavLink 路由入口，需 Router context，故以
// MemoryRouter 包裹（design D6）。jsdom 环境缺口兜底：radix Tooltip /
// ContextMenu 定位需 ResizeObserver 兜底；断言按 D4-④ 收敛最终态（data-root /
// testid / tooltip role），不复刻 portal 细节。
// ---------------------------------------------------------------------------

class ResizeObserverStub {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}

function stubEnvironment() {
  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  return () => {
    vi.unstubAllGlobals();
  };
}

// ---------------------------------------------------------------------------
// fixture 与装置
// ---------------------------------------------------------------------------

function record(root: string): WorkspaceRecord {
  return {
    root,
    name: root.split(/[\\/]/).pop() ?? root,
    addedAt: 1,
  };
}

const FIRST = record('C:\\demo\\beta');
const SECOND = record('C:\\demo\\alpha');

function mountSidebar(workspaces: WorkspaceRecord[], currentRoot: string) {
  const onOpen = vi.fn();
  const onAdd = vi.fn();
  const onRemove = vi.fn();
  render(
    <MemoryRouter>
      <SidebarProvider>
        <AppSidebar
          currentRoot={currentRoot}
          onAdd={onAdd}
          onOpen={onOpen}
          onRemove={onRemove}
          workspaces={workspaces}
        />
      </SidebarProvider>
    </MemoryRouter>,
  );
  return { onAdd, onOpen, onRemove };
}

/** 以 data-root 定位清单项（同名项由 data-root 区分，不依赖可访问名）。 */
function itemByRoot(root: string): HTMLElement {
  const hit = screen
    .getAllByTestId('workspace-item')
    .find((item) => item.getAttribute('data-root') === root);
  if (!hit) throw new Error(`data-root 为 ${root} 的 workspace-item 不存在`);
  return hit;
}

describe('AppSidebar：清单渲染', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('workspaces 全量渲染：每项带 workspace-item testid + data-root、主文本为 record.name、组标签「工作区」在场', () => {
    mountSidebar([FIRST, SECOND], FIRST.root);

    expect(screen.getByText('工作区') !== null).toBe(true);
    const items = screen.getAllByTestId('workspace-item');
    expect(items).toHaveLength(2);
    expect(items.map((item) => item.getAttribute('data-root'))).toEqual([FIRST.root, SECOND.root]);
    // 双行（主文本 + 父目录）走 lg 尺寸承载，固定单行高度（default）会裁掉副文本
    expect(items.map((item) => item.getAttribute('data-size'))).toEqual(['lg', 'lg']);
    expect(within(items[0]).getByText('beta') !== null).toBe(true);
    expect(within(items[1]).getByText('alpha') !== null).toBe(true);
  });

  it('currentRoot 匹配项呈激活态（isActive 标记），非匹配项不激活', () => {
    mountSidebar([FIRST, SECOND], FIRST.root);

    expect(itemByRoot(FIRST.root).getAttribute('data-active')).toBe('true');
    expect(itemByRoot(SECOND.root).getAttribute('data-active')).toBe('false');
  });

  it('空清单（[]）：组与组标签仍渲染、无列表项、不崩', () => {
    mountSidebar([], '');

    expect(screen.getByText('工作区') !== null).toBe(true);
    expect(screen.queryAllByTestId('workspace-item')).toHaveLength(0);
    // 添加入口仍在（空清单依然可发起添加）
    expect(screen.getByRole('button', { name: '添加 workspace' }) !== null).toBe(true);
  });

  it('超大清单（50 项）：全量渲染无丢失（workspace-item 计数断言）', () => {
    const many = Array.from({ length: 50 }, (_, index) => record(`C:\\ws\\proj-${index}`));

    mountSidebar(many, many[0].root);

    expect(screen.getAllByTestId('workspace-item')).toHaveLength(50);
  });

  it('currentRoot 引用清单中不存在的 root（移除后刷新间隙的瞬时态）：无激活项、不崩、清单照常渲染', () => {
    mountSidebar([FIRST, SECOND], 'C:\\gone\\ghost');

    expect(screen.getByText('beta') !== null).toBe(true);
    expect(screen.getByText('alpha') !== null).toBe(true);
    for (const item of screen.getAllByTestId('workspace-item')) {
      expect(item.getAttribute('data-active')).toBe('false');
    }
  });
});

describe('AppSidebar：切换回调与添加入口', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('点击列表项 → onOpen 以该项 root 调用恰一次', () => {
    const { onOpen } = mountSidebar([FIRST, SECOND], FIRST.root);

    fireEvent.click(itemByRoot(SECOND.root));

    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen).toHaveBeenCalledWith(SECOND.root);
  });

  it('SidebarGroupAction（aria-label="添加 workspace"）点击 → onAdd 调用', () => {
    const { onAdd } = mountSidebar([FIRST, SECOND], FIRST.root);

    fireEvent.click(screen.getByRole('button', { name: '添加 workspace' }));

    expect(onAdd).toHaveBeenCalledTimes(1);
  });
});

describe('AppSidebar：右键移除', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('fireEvent.contextMenu 列表项 → 菜单出现 → 点击「移除」→ onRemove 以该项 root 调用（D4-①）', async () => {
    const { onRemove } = mountSidebar([FIRST, SECOND], FIRST.root);

    fireEvent.contextMenu(itemByRoot(SECOND.root));
    const menuItem = await screen.findByRole('menuitem', { name: '移除' });
    fireEvent.click(menuItem);

    expect(onRemove).toHaveBeenCalledTimes(1);
    expect(onRemove).toHaveBeenCalledWith(SECOND.root);
  });

  it('菜单按项作用域隔离：右键非当前项 B 移除的是 B 的 root 而非当前项（data-root 定位不串项）', async () => {
    const { onRemove } = mountSidebar([FIRST, SECOND], FIRST.root);

    // 右键非当前项 SECOND，而非激活的 FIRST
    fireEvent.contextMenu(itemByRoot(SECOND.root));
    fireEvent.click(await screen.findByRole('menuitem', { name: '移除' }));

    expect(onRemove).toHaveBeenCalledTimes(1);
    expect(onRemove).not.toHaveBeenCalledWith(FIRST.root);
    expect(onRemove).toHaveBeenCalledWith(SECOND.root);
  });
});

describe('AppSidebar：Tooltip 与副文本', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('hover 清单项（delayDuration=0 即显，D4-②）→ tooltip 内容为完整 root', async () => {
    mountSidebar([FIRST, SECOND], FIRST.root);

    // radix Tooltip 以 focus 等价 hover 打开（delayDuration=0 由 provider 设定）
    fireEvent.focus(itemByRoot(SECOND.root));
    const tooltip = await screen.findByRole('tooltip');

    expect(tooltip.textContent).toBe(SECOND.root);
  });

  it('同名不同父两项：data-root 定位后 within() 取 workspace-sub，副文本各为父目录（D3 区分能力）', () => {
    const a = record('C:\\a\\plugin');
    const b = record('C:\\b\\plugin');

    mountSidebar([a, b], a.root);

    expect(within(itemByRoot('C:\\a\\plugin')).getByTestId('workspace-sub').textContent).toBe(
      'C:\\a',
    );
    expect(within(itemByRoot('C:\\b\\plugin')).getByTestId('workspace-sub').textContent).toBe(
      'C:\\b',
    );
  });

  it('root 无分隔符（如 plugin）：副文本为空串、workspace-sub 节点不渲染（D3）', () => {
    mountSidebar([record('plugin')], 'plugin');

    const item = itemByRoot('plugin');
    expect(within(item).queryByTestId('workspace-sub')).toBeNull();
    expect(item.textContent).toContain('plugin');
  });

  it('超长 root（>1000 字符）：渲染不崩、完整 root 在 DOM（truncate 承载，供 Tooltip 断言）', () => {
    const longRoot = `C:\\${'dir\\'.repeat(250)}leaf`;

    mountSidebar([record(longRoot)], longRoot);

    const item = itemByRoot(longRoot);
    expect(item.getAttribute('data-root')).toBe(longRoot);
  });
});

// ---------------------------------------------------------------------------
// 页面导航组：[变更] [Agent 调试]，NavLink 路由入口（active 由当前 URL 派生，
// 断言经 URL（location-probe），workspace 清单组语义不变（既有用例全部保留
// 回归）。
// ---------------------------------------------------------------------------

/** URL 探针：把 MemoryRouter 当前 pathname 投影到 DOM 供断言 */
function LocationProbe() {
  const { pathname } = useLocation();
  return <span data-testid="location-probe">{pathname}</span>;
}

function mountNav(initialEntry: string, workspaces: WorkspaceRecord[] = [FIRST, SECOND]) {
  const onOpen = vi.fn();
  const onAdd = vi.fn();
  const onRemove = vi.fn();
  const view = render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <SidebarProvider>
        <AppSidebar
          currentRoot={FIRST.root}
          onAdd={onAdd}
          onOpen={onOpen}
          onRemove={onRemove}
          workspaces={workspaces}
        />
        <LocationProbe />
      </SidebarProvider>
    </MemoryRouter>,
  );
  return { ...view, onAdd, onOpen, onRemove };
}

/** 导航项所属组容器（向上取最近 sidebar-group），供组归属 DOM 结构断言。 */
function groupOf(testId: string): HTMLElement {
  const group = screen.getByTestId(testId).closest<HTMLElement>('[data-slot="sidebar-group"]');
  if (!group) throw new Error(`${testId} 不在任何 sidebar-group 内`);
  return group;
}

describe('AppSidebar：页面导航组与系统工具组（NavLink 路由导航）', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('壳态渲染「页面」「工作区」「系统工具」三组标签：页面最上、系统工具最下', () => {
    mountNav('/changes');

    const pageLabel = screen.getByText('页面');
    const toolsLabel = screen.getByText('系统工具');
    const wsLabel = screen.getByText('工作区');
    expect(
      pageLabel.compareDocumentPosition(wsLabel) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      wsLabel.compareDocumentPosition(toolsLabel) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(screen.getByTestId('nav-changes').textContent).toContain('变更');
    expect(screen.getByTestId('nav-agent').textContent).toContain('Agent 调试');
  });

  it('「页面」组仅含 nav-changes；nav-agent 归属「系统工具」组（组归属以 DOM 结构断言）', () => {
    mountNav('/changes');

    const pageGroup = groupOf('nav-changes');
    expect(within(pageGroup).getByTestId('nav-changes') !== null).toBe(true);
    expect(within(pageGroup).queryByTestId('nav-agent')).toBeNull();
    expect(within(pageGroup).queryByTestId('nav-db')).toBeNull();

    const toolsGroup = groupOf('nav-agent');
    expect(toolsGroup).toBe(groupOf('nav-db'));
    expect(toolsGroup.textContent).toContain('系统工具');
    expect(within(toolsGroup).getByTestId('nav-agent').textContent).toContain('Agent 调试');
    expect(within(toolsGroup).getByTestId('nav-db').textContent).toContain('数据库');
  });

  it('nav-* 渲染为锚点元素：testid 落在 NavLink 锚点上（D8 asChild Slot 合并到锚点，非 button 嵌套 anchor）', () => {
    mountNav('/changes');

    expect(screen.getByTestId('nav-changes').tagName).toBe('A');
    expect(screen.getByTestId('nav-agent').tagName).toBe('A');
    expect(screen.getByTestId('nav-changes').getAttribute('href')).toBe('/changes');
    expect(screen.getByTestId('nav-agent').getAttribute('href')).toBe('/agent');
  });

  it('pathname 无匹配前缀（/bogus）：nav 均 data-active=false，锚点与 workspace 组照常渲染不崩（active 派生安全降级，组件级不等同 App 层重定向兜底）', () => {
    mountNav('/bogus');

    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-db').getAttribute('data-active')).toBe('false');
    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);
    expect(screen.getByRole('button', { name: '添加 workspace' }) !== null).toBe(true);
  });

  it('active 态由 URL 派生：/changes 与 /changes/:name 激活变更项，/agent 激活 Agent 项', () => {
    const first = mountNav('/changes', [FIRST]);
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');
    first.unmount();

    // 详情路由同样激活变更项（与路由化前 page='changes' 含详情态一致）
    const detail = mountNav('/changes/add-feature', [FIRST]);
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');
    detail.unmount();

    mountNav('/agent', [FIRST]);
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('false');
  });

  it('点击「Agent 调试」→ URL 变为 /agent；点击「变更」→ URL 变回 /changes', () => {
    mountNav('/changes');
    expect(screen.getByTestId('location-probe').textContent).toBe('/changes');

    fireEvent.click(screen.getByTestId('nav-agent'));
    expect(screen.getByTestId('location-probe').textContent).toBe('/agent');

    fireEvent.click(screen.getByTestId('nav-changes'));
    expect(screen.getByTestId('location-probe').textContent).toBe('/changes');
  });

  it('URL 为 /agent 时 workspace 清单组照常渲染、语义不变（激活态/点击回调不串扰）', () => {
    const { onAdd, onOpen, onRemove } = mountNav('/agent');

    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);
    expect(itemByRoot(FIRST.root).getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('true');
    fireEvent.click(itemByRoot(SECOND.root));
    expect(onOpen).toHaveBeenCalledWith(SECOND.root);
    expect(onAdd).not.toHaveBeenCalled();
    expect(onRemove).not.toHaveBeenCalled();
  });

  it('URL 为根路径 /：三导航项均不激活（激活态由 URL 派生；/ → /changes 重定向为 App 层职责，route_pages 覆盖）', () => {
    mountSidebar([FIRST, SECOND], FIRST.root);

    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-db').getAttribute('data-active')).toBe('false');
  });

  it('两入口各带 lucide 图标（以 DOM 结构断言，非观感）', () => {
    mountNav('/changes');

    expect(screen.getByTestId('nav-changes').querySelector('svg') !== null).toBe(true);
    expect(screen.getByTestId('nav-agent').querySelector('svg') !== null).toBe(true);
    expect(screen.getByTestId('nav-db').querySelector('svg') !== null).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 页面导航组 nav-info 扩员（AC-1）：[基础信息] 居「页面」组首位（组序
// 基础信息 → 变更 → 探索），active 由 URL 派生。页面组扩员不改变他项——
// nav-changes / nav-explores / nav-agent / nav-db 的组归属与激活语义既有
// 断言保留回归（上方既有套件全部保留）。
// ---------------------------------------------------------------------------

describe('AppSidebar：页面导航组 nav-info（AC-1）', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('nav-info 在场且居「页面」组首位（基础信息 → 变更 → 探索 → 配置），tagName 为 A、href=/info、带 svg 图标', () => {
    mountNav('/changes');

    const navInfo = screen.getByTestId('nav-info');
    expect(navInfo.tagName).toBe('A');
    expect(navInfo.getAttribute('href')).toBe('/info');
    expect(navInfo.textContent).toContain('基础信息');
    expect(navInfo.querySelector('svg') !== null).toBe(true);

    // 组序以 DOM 结构断言：基础信息居首，其后为 变更 → 探索 → 配置（nav-config 扩员）
    const pageGroup = groupOf('nav-info');
    const navOrder = within(pageGroup)
      .getAllByTestId(/^nav-/)
      .map((nav) => nav.getAttribute('data-testid'));
    expect(navOrder).toEqual(['nav-info', 'nav-changes', 'nav-explores', 'nav-config']);
  });

  it('pathname=/info → nav-info data-active=true 且其余 nav 为 false；/changes 下 nav-info 为 false（active 由 URL 派生）', () => {
    const onInfo = mountNav('/info', [FIRST]);
    expect(screen.getByTestId('nav-info').getAttribute('data-active')).toBe('true');
    for (const id of ['nav-explores', 'nav-changes', 'nav-agent', 'nav-db']) {
      expect(screen.getByTestId(id).getAttribute('data-active')).toBe('false');
    }
    onInfo.unmount();

    mountNav('/changes', [FIRST]);
    expect(screen.getByTestId('nav-info').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
  });

  it('点击 nav-info → location-probe 呈 /info', () => {
    mountNav('/changes');

    fireEvent.click(screen.getByTestId('nav-info'));

    expect(screen.getByTestId('location-probe').textContent).toBe('/info');
  });

  it('pathname 无匹配前缀（/bogus）→ nav-info data-active=false、渲染不崩（对齐既有降级用例）', () => {
    mountNav('/bogus');

    expect(screen.getByTestId('nav-info').getAttribute('data-active')).toBe('false');
    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);
    expect(screen.getByRole('button', { name: '添加 workspace' }) !== null).toBe(true);
  });

  it('既有挂钩回归：页面组含 nav-info / nav-explores / nav-changes、系统工具组含 nav-agent / nav-db（页面组扩员不改变他项）', () => {
    mountNav('/changes');

    const pageGroup = groupOf('nav-info');
    expect(groupOf('nav-explores')).toBe(pageGroup);
    expect(groupOf('nav-changes')).toBe(pageGroup);
    expect(within(pageGroup).getByTestId('nav-explores').textContent).toContain('探索');
    expect(within(pageGroup).getByTestId('nav-changes').textContent).toContain('变更');
    for (const id of ['nav-agent', 'nav-db']) {
      expect(within(pageGroup).queryByTestId(id)).toBeNull();
    }

    const toolsGroup = groupOf('nav-agent');
    expect(toolsGroup).toBe(groupOf('nav-db'));
    expect(toolsGroup.textContent).toContain('系统工具');
    expect(within(toolsGroup).getByTestId('nav-agent').textContent).toContain('Agent 调试');
    expect(within(toolsGroup).getByTestId('nav-db').textContent).toContain('数据库');
  });
});

// ---------------------------------------------------------------------------
// 页面导航组 nav-config 扩员（AC-4）：[配置] 居「页面」组末位（组序
// 基础信息 → 变更 → 探索 → 配置），active 由 URL 派生。扩员不改变他项——
// nav-info / nav-changes / nav-explores / nav-agent / nav-db 的组归属与激活
// 语义既有断言保留回归（上方既有套件全部保留）。
// ---------------------------------------------------------------------------

describe('AppSidebar：页面导航组 nav-config 扩员（AC-4）', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('nav-config 在场且居「页面」组末位（组序 nav-info → nav-changes → nav-explores → nav-config），tagName 为 A、href=/config、带 svg 图标、文案「配置」', () => {
    mountNav('/changes');

    const navConfig = screen.getByTestId('nav-config');
    expect(navConfig.tagName).toBe('A');
    expect(navConfig.getAttribute('href')).toBe('/config');
    expect(navConfig.textContent).toContain('配置');
    expect(navConfig.querySelector('svg') !== null).toBe(true);

    // 组序以 DOM 结构断言：配置居「页面」组末位
    const pageGroup = groupOf('nav-config');
    const navOrder = within(pageGroup)
      .getAllByTestId(/^nav-/)
      .map((nav) => nav.getAttribute('data-testid'));
    expect(navOrder).toEqual(['nav-info', 'nav-changes', 'nav-explores', 'nav-config']);
  });

  it('pathname=/config → nav-config data-active=true 且其余 nav 全 false；/changes 下 nav-config 为 false（active 由 URL 派生）', () => {
    const onConfig = mountNav('/config', [FIRST]);
    expect(screen.getByTestId('nav-config').getAttribute('data-active')).toBe('true');
    for (const id of ['nav-info', 'nav-changes', 'nav-explores', 'nav-agent', 'nav-db']) {
      expect(screen.getByTestId(id).getAttribute('data-active')).toBe('false');
    }
    onConfig.unmount();

    mountNav('/changes', [FIRST]);
    expect(screen.getByTestId('nav-config').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
  });

  it('点击 nav-config → location-probe 呈 /config', () => {
    mountNav('/changes');

    fireEvent.click(screen.getByTestId('nav-config'));

    expect(screen.getByTestId('location-probe').textContent).toBe('/config');
  });

  it('pathname 无匹配前缀（/bogus）→ nav-config data-active=false、渲染不崩（对齐既有降级用例）', () => {
    mountNav('/bogus');

    expect(screen.getByTestId('nav-config').getAttribute('data-active')).toBe('false');
    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);
    expect(screen.getByRole('button', { name: '添加 workspace' }) !== null).toBe(true);
  });

  it('既有挂钩回归：页面组四项与系统工具组（nav-agent / nav-db）的组归属与激活语义（扩员不改变他项）', () => {
    mountNav('/config');

    const pageGroup = groupOf('nav-config');
    expect(groupOf('nav-info')).toBe(pageGroup);
    expect(groupOf('nav-changes')).toBe(pageGroup);
    expect(groupOf('nav-explores')).toBe(pageGroup);
    expect(within(pageGroup).getByTestId('nav-info').textContent).toContain('基础信息');
    expect(within(pageGroup).getByTestId('nav-changes').textContent).toContain('变更');
    expect(within(pageGroup).getByTestId('nav-explores').textContent).toContain('探索');
    for (const id of ['nav-agent', 'nav-db']) {
      expect(within(pageGroup).queryByTestId(id)).toBeNull();
      expect(screen.getByTestId(id).getAttribute('data-active')).toBe('false');
    }

    const toolsGroup = groupOf('nav-agent');
    expect(toolsGroup).toBe(groupOf('nav-db'));
    expect(toolsGroup.textContent).toContain('系统工具');
    expect(within(toolsGroup).getByTestId('nav-agent').textContent).toContain('Agent 调试');
    expect(within(toolsGroup).getByTestId('nav-db').textContent).toContain('数据库');
  });
});

// ---------------------------------------------------------------------------
// 系统工具组 [Agent 管理] 入口（AC-1）：nav-agents 归属「系统工具」组且居组
// 内首位（组序 Agent 管理 → Agent 调试 → 数据库），active 由 pathname ===
// '/agents' 派生。新增项不改变他项——「页面」组四项与既有 nav testid 零变化
// （回归锚定）。
// ---------------------------------------------------------------------------

describe('AppSidebar：系统工具组 nav-agents 入口（AC-1）', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('nav-agents 在场且归属「系统工具」组：文案「Agent 管理」、tagName 为 A、href=/agents、带 svg 图标', () => {
    mountNav('/changes');

    const navAgents = screen.getByTestId('nav-agents');
    expect(navAgents.tagName).toBe('A');
    expect(navAgents.getAttribute('href')).toBe('/agents');
    expect(navAgents.textContent).toContain('Agent 管理');
    expect(navAgents.querySelector('svg') !== null).toBe(true);

    const toolsGroup = groupOf('nav-agents');
    expect(toolsGroup.textContent).toContain('系统工具');
    expect(within(toolsGroup).getByTestId('nav-agent').textContent).toContain('Agent 调试');
    expect(within(toolsGroup).getByTestId('nav-db').textContent).toContain('数据库');
    expect(within(toolsGroup).queryByTestId('nav-changes')).toBeNull();
  });

  it('组内排序恰为 [Agent 管理] [Agent 调试] [数据库]（DOM 结构断言）', () => {
    mountNav('/changes');

    const toolsGroup = groupOf('nav-agents');
    const navOrder = within(toolsGroup)
      .getAllByTestId(/^nav-/)
      .map((nav) => nav.getAttribute('data-testid'));
    expect(navOrder).toEqual(['nav-agents', 'nav-agent', 'nav-db']);
  });

  it('/agents 路径下 nav-agents active 态成立、他路径下不成立（active 由 URL 派生）', () => {
    const onAgents = mountNav('/agents', [FIRST]);
    expect(screen.getByTestId('nav-agents').getAttribute('data-active')).toBe('true');
    for (const id of [
      'nav-info',
      'nav-changes',
      'nav-explores',
      'nav-config',
      'nav-agent',
      'nav-db',
    ]) {
      expect(screen.getByTestId(id).getAttribute('data-active')).toBe('false');
    }
    onAgents.unmount();

    const onChanges = mountNav('/changes', [FIRST]);
    expect(screen.getByTestId('nav-agents').getAttribute('data-active')).toBe('false');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
    onChanges.unmount();

    // 无匹配前缀（/bogus）降级口径一致
    const onBogus = mountNav('/bogus', [FIRST]);
    expect(screen.getByTestId('nav-agents').getAttribute('data-active')).toBe('false');
    onBogus.unmount();
  });

  it('点击 nav-agents → location-probe 呈 /agents', () => {
    mountNav('/changes');

    fireEvent.click(screen.getByTestId('nav-agents'));

    expect(screen.getByTestId('location-probe').textContent).toBe('/agents');
  });

  it('回归锚定：nav-agents 扩员后「页面」组四项与既有 nav testid 零变化', () => {
    mountNav('/changes');

    const pageGroup = groupOf('nav-changes');
    const navOrder = within(pageGroup)
      .getAllByTestId(/^nav-/)
      .map((nav) => nav.getAttribute('data-testid'));
    expect(navOrder).toEqual(['nav-info', 'nav-changes', 'nav-explores', 'nav-config']);
    expect(groupOf('nav-agents')).not.toBe(pageGroup);
    for (const id of ['nav-agents', 'nav-agent', 'nav-db']) {
      expect(within(pageGroup).queryByTestId(id)).toBeNull();
    }
  });
});
