// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { MemoryRouter, useLocation } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { SidebarProvider } from '@/components/ui/sidebar';

import type { WorkspaceRecord } from '../types/dto';
import { AppSidebar } from './app-sidebar';

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

/** workspace 选择器 trigger（DropdownMenuTrigger 收敛到 data-slot）。 */
function workspaceTrigger(): HTMLElement {
  const trigger = document.querySelector<HTMLElement>('[data-slot="dropdown-menu-trigger"]');
  if (!trigger) throw new Error('workspace 选择器 trigger 不存在');
  return trigger;
}

/** 打开 workspace 选择器：base-ui Menu 以 mousedown 打开、开启动作为 frame
 * 异步执行，以菜单组标签「工作区」出现收敛最终态（D4-④）。 */
async function openWorkspaceMenu(): Promise<void> {
  fireEvent.mouseDown(workspaceTrigger());
  await screen.findByText('工作区');
}

describe('AppSidebar：选择器渲染', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('选择器呈当前项：trigger 主文本为 record.name、副文本 workspace-sub 为父目录、data-size=lg（双行走 lg 承载）', () => {
    mountSidebar([FIRST, SECOND], FIRST.root);

    const trigger = workspaceTrigger();
    expect(trigger.getAttribute('data-size')).toBe('lg');
    expect(within(trigger).getByText('beta') !== null).toBe(true);
    expect(within(trigger).getByTestId('workspace-sub').textContent).toBe('C:\\demo');
  });

  it('currentRoot 决定呈现内容：currentRoot 指向 SECOND 时 trigger 主文本为 alpha、副文本不变', () => {
    mountSidebar([FIRST, SECOND], SECOND.root);

    const trigger = workspaceTrigger();
    expect(within(trigger).getByText('alpha') !== null).toBe(true);
    expect(within(trigger).queryByText('beta')).toBeNull();
    expect(within(trigger).getByTestId('workspace-sub').textContent).toBe('C:\\demo');
  });

  it('空清单（[]）：无下拉 trigger、未关联工作区占位渲染、点击占位即 onAdd、不崩', () => {
    const { onAdd } = mountSidebar([], '');

    expect(document.querySelector('[data-slot="dropdown-menu-trigger"]')).toBeNull();
    const placeholder = screen.getByRole('button', { name: '未关联工作区 点击添加' });
    expect(placeholder.textContent).toContain('未关联工作区');

    fireEvent.click(placeholder);
    expect(onAdd).toHaveBeenCalledTimes(1);
  });

  it('超大清单（50 项）：打开选择器后全量渲染无丢失（menuitem 计数断言）', async () => {
    const many = Array.from({ length: 50 }, (_, index) => record(`C:\\ws\\proj-${index}`));

    mountSidebar(many, many[0].root);

    await openWorkspaceMenu();

    // 50 个 workspace 项 + 分隔后「添加工作区」1 项
    expect(screen.getAllByRole('menuitem')).toHaveLength(51);
    expect(screen.getByRole('menuitem', { name: 'proj-49' }) !== null).toBe(true);
  });

  it('currentRoot 引用清单中不存在的 root（移除后刷新间隙的瞬时态）：选择器不渲染、导航组照常、不崩', () => {
    mountSidebar([FIRST, SECOND], 'C:\\gone\\ghost');

    expect(document.querySelector('[data-slot="dropdown-menu-trigger"]')).toBeNull();
    expect(screen.queryByText('beta')).toBeNull();
    expect(screen.queryByText('alpha')).toBeNull();
    // 选择器降级不影响导航组（currentRoot 非空 → 页面组照常渲染）
    expect(screen.getByTestId('nav-changes') !== null).toBe(true);
    expect(screen.getByText('系统工具') !== null).toBe(true);
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

  it('打开选择器点击清单项 → onOpen 以该项 root 调用恰一次', async () => {
    const { onOpen } = mountSidebar([FIRST, SECOND], FIRST.root);

    await openWorkspaceMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: 'alpha' }));

    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen).toHaveBeenCalledWith(SECOND.root);
  });

  it('打开选择器点击「添加工作区」→ onAdd 调用恰一次', async () => {
    const { onAdd } = mountSidebar([FIRST, SECOND], FIRST.root);

    await openWorkspaceMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: '添加工作区' }));

    expect(onAdd).toHaveBeenCalledTimes(1);
  });
});

describe('AppSidebar：选择器副文本', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('同名不同父两项：菜单项同名不合并（两枚 plugin menuitem）、副文本为当前项父目录（D3 区分能力）', async () => {
    const a = record('C:\\a\\plugin');
    const b = record('C:\\b\\plugin');

    mountSidebar([a, b], a.root);

    expect(within(workspaceTrigger()).getByTestId('workspace-sub').textContent).toBe('C:\\a');

    await openWorkspaceMenu();
    expect(screen.getAllByRole('menuitem', { name: 'plugin' })).toHaveLength(2);
  });

  it('root 无分隔符（如 plugin）：副文本为空串、workspace-sub 节点不渲染（D3）', () => {
    mountSidebar([record('plugin')], 'plugin');

    const trigger = workspaceTrigger();
    expect(within(trigger).queryByTestId('workspace-sub')).toBeNull();
    expect(trigger.textContent).toContain('plugin');
  });

  it('超长 root（>1000 字符）：渲染不崩、完整父目录在 workspace-sub（truncate 承载）', () => {
    const longRoot = `C:\\${'dir\\'.repeat(250)}leaf`;

    mountSidebar([record(longRoot)], longRoot);

    const trigger = workspaceTrigger();
    expect(within(trigger).getByText('leaf') !== null).toBe(true);
    expect(within(trigger).getByTestId('workspace-sub').textContent).toBe(
      longRoot.slice(0, longRoot.lastIndexOf('\\')),
    );
  });
});

// ---------------------------------------------------------------------------
// 页面导航组：[变更] [Agent 调试]，NavLink 路由入口（active 由当前 URL 派生，
// 断言经 URL（location-probe），workspace 选择器语义不变（既有用例全部保留
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

  it('壳态渲染「工作区」「系统工具」两组标签：工作区组最上、系统工具组最下', async () => {
    mountNav('/changes');

    // 「工作区」为选择器菜单组标签，随菜单打开而挂载（portal 于 body 级，
    // 不参与侧栏组序）；组序以 DOM 结构断言
    await openWorkspaceMenu();
    expect(screen.getByText('工作区') !== null).toBe(true);

    const groups = Array.from(
      document.querySelectorAll<HTMLElement>('[data-slot="sidebar-group"]'),
    );
    expect(groups[0].contains(workspaceTrigger())).toBe(true);
    expect(groups[groups.length - 1].contains(screen.getByText('系统工具'))).toBe(true);
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

    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-db').hasAttribute('data-active')).toBe(false);
    // workspace 选择器照常渲染当前项
    expect(within(workspaceTrigger()).getByText('beta') !== null).toBe(true);
  });

  it('active 态由 URL 派生：/changes 与 /changes/:name 激活变更项，/agent 激活 Agent 项', () => {
    const first = mountNav('/changes', [FIRST]);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(false);
    first.unmount();

    // 详情路由同样激活变更项（与路由化前 page='changes' 含详情态一致）
    const detail = mountNav('/changes/add-feature', [FIRST]);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(false);
    detail.unmount();

    mountNav('/agent', [FIRST]);
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(true);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
  });

  it('点击「Agent 调试」→ URL 变为 /agent；点击「变更」→ URL 变回 /changes', () => {
    mountNav('/changes');
    expect(screen.getByTestId('location-probe').textContent).toBe('/changes');

    fireEvent.click(screen.getByTestId('nav-agent'));
    expect(screen.getByTestId('location-probe').textContent).toBe('/agent');

    fireEvent.click(screen.getByTestId('nav-changes'));
    expect(screen.getByTestId('location-probe').textContent).toBe('/changes');
  });

  it('URL 为 /agent 时 workspace 选择器照常渲染、语义不变（激活态/点击回调不串扰）', async () => {
    const { onAdd, onOpen, onRemove } = mountNav('/agent');

    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(true);
    await openWorkspaceMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: 'alpha' }));
    expect(onOpen).toHaveBeenCalledWith(SECOND.root);
    expect(onAdd).not.toHaveBeenCalled();
    expect(onRemove).not.toHaveBeenCalled();
  });

  it('URL 为根路径 /：三导航项均不激活（激活态由 URL 派生；/ → /changes 重定向为 App 层职责，route_pages 覆盖）', () => {
    mountSidebar([FIRST, SECOND], FIRST.root);

    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-agent').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-db').hasAttribute('data-active')).toBe(false);
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
    expect(screen.getByTestId('nav-info').hasAttribute('data-active')).toBe(true);
    for (const id of ['nav-explores', 'nav-changes', 'nav-agent', 'nav-db']) {
      expect(screen.getByTestId(id).hasAttribute('data-active')).toBe(false);
    }
    onInfo.unmount();

    mountNav('/changes', [FIRST]);
    expect(screen.getByTestId('nav-info').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(true);
  });

  it('点击 nav-info → location-probe 呈 /info', () => {
    mountNav('/changes');

    fireEvent.click(screen.getByTestId('nav-info'));

    expect(screen.getByTestId('location-probe').textContent).toBe('/info');
  });

  it('pathname 无匹配前缀（/bogus）→ nav-info data-active=false、渲染不崩（对齐既有降级用例）', () => {
    mountNav('/bogus');

    expect(screen.getByTestId('nav-info').hasAttribute('data-active')).toBe(false);
    expect(within(workspaceTrigger()).getByText('beta') !== null).toBe(true);
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
    expect(screen.getByTestId('nav-config').hasAttribute('data-active')).toBe(true);
    for (const id of ['nav-info', 'nav-changes', 'nav-explores', 'nav-agent', 'nav-db']) {
      expect(screen.getByTestId(id).hasAttribute('data-active')).toBe(false);
    }
    onConfig.unmount();

    mountNav('/changes', [FIRST]);
    expect(screen.getByTestId('nav-config').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(true);
  });

  it('点击 nav-config → location-probe 呈 /config', () => {
    mountNav('/changes');

    fireEvent.click(screen.getByTestId('nav-config'));

    expect(screen.getByTestId('location-probe').textContent).toBe('/config');
  });

  it('pathname 无匹配前缀（/bogus）→ nav-config data-active=false、渲染不崩（对齐既有降级用例）', () => {
    mountNav('/bogus');

    expect(screen.getByTestId('nav-config').hasAttribute('data-active')).toBe(false);
    expect(within(workspaceTrigger()).getByText('beta') !== null).toBe(true);
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
      expect(screen.getByTestId(id).hasAttribute('data-active')).toBe(false);
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
    expect(screen.getByTestId('nav-agents').hasAttribute('data-active')).toBe(true);
    for (const id of [
      'nav-info',
      'nav-changes',
      'nav-explores',
      'nav-config',
      'nav-agent',
      'nav-db',
    ]) {
      expect(screen.getByTestId(id).hasAttribute('data-active')).toBe(false);
    }
    onAgents.unmount();

    const onChanges = mountNav('/changes', [FIRST]);
    expect(screen.getByTestId('nav-agents').hasAttribute('data-active')).toBe(false);
    expect(screen.getByTestId('nav-changes').hasAttribute('data-active')).toBe(true);
    onChanges.unmount();

    // 无匹配前缀（/bogus）降级口径一致
    const onBogus = mountNav('/bogus', [FIRST]);
    expect(screen.getByTestId('nav-agents').hasAttribute('data-active')).toBe(false);
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
