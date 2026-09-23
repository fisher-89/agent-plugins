// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { SidebarProvider } from '@/components/ui/sidebar';

import type { WorkspaceRecord } from '../types/dto';
import { AppSidebar, type TopPage } from './AppSidebar';

// ---------------------------------------------------------------------------
// AppSidebar 单测：组件纯回调驱动（onOpen / onAdd / onRemove 以 vi.fn() 注入），
// 无进程边界。jsdom 环境缺口兜底：radix Tooltip / ContextMenu 定位需
// ResizeObserver 兜底；断言按 D4-④ 收敛最终态（data-root / testid /
// tooltip role），不复刻 portal 细节。
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

function record(root: string, lastOpenedAt: number): WorkspaceRecord {
  return {
    root,
    name: root.split(/[\\/]/).pop() ?? root,
    addedAt: 1,
    lastOpenedAt,
  };
}

const FIRST = record('C:\\demo\\beta', 900);
const SECOND = record('C:\\demo\\alpha', 100);

function mountSidebar(workspaces: WorkspaceRecord[], currentRoot: string) {
  const onOpen = vi.fn();
  const onAdd = vi.fn();
  const onRemove = vi.fn();
  render(
    <SidebarProvider>
      <AppSidebar
        currentRoot={currentRoot}
        onAdd={onAdd}
        onOpen={onOpen}
        onRemove={onRemove}
        workspaces={workspaces}
      />
    </SidebarProvider>,
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
    const many = Array.from({ length: 50 }, (_, index) => record(`C:\\ws\\proj-${index}`, index));

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
    const a = record('C:\\a\\plugin', 900);
    const b = record('C:\\b\\plugin', 100);

    mountSidebar([a, b], a.root);

    expect(within(itemByRoot('C:\\a\\plugin')).getByTestId('workspace-sub').textContent).toBe(
      'C:\\a',
    );
    expect(within(itemByRoot('C:\\b\\plugin')).getByTestId('workspace-sub').textContent).toBe(
      'C:\\b',
    );
  });

  it('root 无分隔符（如 plugin）：副文本为空串、workspace-sub 节点不渲染（D3）', () => {
    mountSidebar([record('plugin', 1)], 'plugin');

    const item = itemByRoot('plugin');
    expect(within(item).queryByTestId('workspace-sub')).toBeNull();
    expect(item.textContent).toContain('plugin');
  });

  it('超长 root（>1000 字符）：渲染不崩、完整 root 在 DOM（truncate 承载，供 Tooltip 断言）', () => {
    const longRoot = `C:\\${'dir\\'.repeat(250)}leaf`;

    mountSidebar([record(longRoot, 1)], longRoot);

    const item = itemByRoot(longRoot);
    expect(item.getAttribute('data-root')).toBe(longRoot);
  });
});

// ---------------------------------------------------------------------------
// 页面导航组：[变更] [Agent 调试]，顶层视图切换入口（无路由；本变更新增，
// 首次出现非 workspace 入口语义）。组件纯回调驱动（onPageChange 以 vi.fn()
// 注入），workspace 清单组语义不变（既有用例全部保留回归）。
// ---------------------------------------------------------------------------

function mountNav(page: TopPage, workspaces: WorkspaceRecord[] = [FIRST, SECOND]) {
  const onPageChange = vi.fn();
  render(
    <SidebarProvider>
      <AppSidebar
        currentRoot={FIRST.root}
        onAdd={vi.fn()}
        onPageChange={onPageChange}
        onOpen={vi.fn()}
        onRemove={vi.fn()}
        page={page}
        workspaces={workspaces}
      />
    </SidebarProvider>,
  );
  return { onPageChange };
}

describe('AppSidebar：页面导航组（page / onPageChange）', () => {
  let restore: () => void;

  beforeEach(() => {
    restore = stubEnvironment();
  });

  afterEach(() => {
    restore();
  });

  it('「页面」导航组渲染于 workspace 清单组上方，含「变更」与「Agent 调试」两项', () => {
    mountNav('changes');

    const pageLabel = screen.getByText('页面');
    const wsLabel = screen.getByText('工作区');
    expect(
      pageLabel.compareDocumentPosition(wsLabel) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(screen.getByTestId('nav-changes').textContent).toContain('变更');
    expect(screen.getByTestId('nav-agent').textContent).toContain('Agent 调试');
  });

  it('当前 page 项呈激活态、另一项不激活（changes 与 agent 两态）', () => {
    const { unmount } = render(
      <SidebarProvider>
        <AppSidebar
          currentRoot={FIRST.root}
          onAdd={vi.fn()}
          onOpen={vi.fn()}
          onRemove={vi.fn()}
          page="changes"
          workspaces={[FIRST]}
        />
      </SidebarProvider>,
    );
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('false');
    unmount();

    mountNav('agent');
    expect(screen.getByTestId('nav-agent').getAttribute('data-active')).toBe('true');
    expect(screen.getByTestId('nav-changes').getAttribute('data-active')).toBe('false');
  });

  it('点击「Agent 调试」→ onPageChange("agent") 恰一次；点击「变更」→ onPageChange("changes") 恰一次', () => {
    const { onPageChange } = mountNav('changes');

    fireEvent.click(screen.getByTestId('nav-agent'));
    expect(onPageChange).toHaveBeenCalledTimes(1);
    expect(onPageChange).toHaveBeenCalledWith('agent');

    fireEvent.click(screen.getByTestId('nav-changes'));
    expect(onPageChange).toHaveBeenCalledTimes(2);
    expect(onPageChange).toHaveBeenLastCalledWith('changes');
  });

  it("page='agent' 时 workspace 清单组照常渲染、语义不变（激活态/点击回调不串扰）", () => {
    const onPageChange = vi.fn();
    const onOpen = vi.fn();
    render(
      <SidebarProvider>
        <AppSidebar
          currentRoot={FIRST.root}
          onAdd={vi.fn()}
          onPageChange={onPageChange}
          onOpen={onOpen}
          onRemove={vi.fn()}
          page="agent"
          workspaces={[FIRST, SECOND]}
        />
      </SidebarProvider>,
    );

    expect(screen.getAllByTestId('workspace-item')).toHaveLength(2);
    expect(itemByRoot(FIRST.root).getAttribute('data-active')).toBe('true');
    fireEvent.click(itemByRoot(SECOND.root));
    expect(onOpen).toHaveBeenCalledWith(SECOND.root);
    expect(onPageChange).not.toHaveBeenCalled();
  });

  it('两入口各带 lucide 图标（以 DOM 结构断言，非观感）', () => {
    mountNav('changes');

    expect(screen.getByTestId('nav-changes').querySelector('svg') !== null).toBe(true);
    expect(screen.getByTestId('nav-agent').querySelector('svg') !== null).toBe(true);
  });
});
