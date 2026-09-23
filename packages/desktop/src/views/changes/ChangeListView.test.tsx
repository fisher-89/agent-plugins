import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeListState } from '../../hooks/useChangeList';
import type { ChangeList } from '../../types/dto';
import { ChangeListView } from './ChangeListView';

/** 以 fixture DTO 构造列表状态（取数已在 hooks 层被测，此处直接注入状态）。 */
function state(
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

const fixtureList: ChangeList = {
  active: [
    {
      name: 'add-feature',
      source: 'active',
      inventory: 'v2',
      created: '2026-09-01',
      unparsable: false,
    },
    { name: 'docs-only', source: 'active', inventory: 'v0', created: null, unparsable: true },
  ],
  archiveGroups: [
    {
      month: '2026-09',
      changes: [
        {
          name: '2026-09-01-first',
          source: 'archive',
          inventory: 'v1',
          created: '2026-09-01',
          unparsable: false,
        },
      ],
    },
    {
      month: '2026-05',
      changes: [
        {
          name: '2026-05-15-second',
          source: 'archive',
          inventory: 'v0',
          created: '2026-05-15',
          unparsable: false,
        },
      ],
    },
    {
      month: null,
      changes: [
        {
          name: 'no-date-archived',
          source: 'archive',
          inventory: 'v1',
          created: null,
          unparsable: false,
        },
      ],
    },
  ],
};

describe('ChangeListView：分组列表、代际徽标与进入详情', () => {
  it('active 列表逐条渲染并带正确代际徽标（v2 / v1 / v0）', () => {
    render(<ChangeListView state={state({ data: fixtureList })} onSelect={() => {}} />);
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(screen.getByText('docs-only') !== null).toBe(true);
    // 徽标宿主为 Badge 组件（INVENTORY_VARIANT 显式映射），在各 change-row 作用域内收集文本
    const badgeTexts = screen
      .getAllByTestId('change-row')
      .map((row) => within(row).getByText(/^v[012]$/).textContent ?? '');
    expect(badgeTexts).toEqual(expect.arrayContaining(['v2', 'v0', 'v1']));
  });

  it('archive 按月分组渲染，month=null 的"未知时间"组渲染在序列尾', () => {
    render(<ChangeListView state={state({ data: fixtureList })} onSelect={() => {}} />);
    const headings = screen.getAllByRole('heading').map((h) => h.textContent ?? '');
    expect(headings.some((text) => text.startsWith('2026-09'))).toBe(true);
    expect(headings.some((text) => text.startsWith('2026-05'))).toBe(true);
    // 未知时间组置尾
    const last = headings[headings.length - 1];
    expect(last.startsWith('未知时间')).toBe(true);
    expect(screen.getByText('no-date-archived') !== null).toBe(true);
  });

  it('点击 change 条目触发进入详情回调并携带 change 名', () => {
    const onSelect = vi.fn();
    render(<ChangeListView state={state({ data: fixtureList })} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('add-feature'));
    expect(onSelect).toHaveBeenCalledWith('add-feature');
  });

  it('error 状态渲染错误提示，不白屏', () => {
    render(<ChangeListView state={state({ error: 'IPC 断开' })} onSelect={() => {}} />);
    expect(screen.getByText(/列表加载失败：IPC 断开/) !== null).toBe(true);
  });

  it('loading 态与空数据渲染空态提示', () => {
    const { container, rerender } = render(
      <ChangeListView state={state({ loading: true })} onSelect={() => {}} />,
    );
    expect(container.textContent).toContain('加载中');

    // 空数据：无 active、无分组 → 引导文案
    rerender(
      <ChangeListView
        state={state({ data: { active: [], archiveGroups: [] } })}
        onSelect={() => {}}
      />,
    );
    expect(container.textContent).toContain('未发现任何 change 目录');
  });

  it('unparsable 条目附"无法解析"标注且仍入列', () => {
    render(<ChangeListView state={state({ data: fixtureList })} onSelect={() => {}} />);
    expect(screen.getByText('workflow.json 无法解析') !== null).toBe(true);
  });
});

describe('ChangeListView：created / 错误条 / 空态提示的分支形态', () => {
  it('created 为 null 的条目不渲染日期，非空条目渲染日期', () => {
    const { container } = render(
      <ChangeListView state={state({ data: fixtureList })} onSelect={() => {}} />,
    );
    // 按钮名本身含日期前缀，因此必须精确断言 created 挂钩的文本而非整页 textContent
    const createdTexts = screen.getAllByTestId('created').map((span) => span.textContent ?? '');
    expect(createdTexts).toEqual(expect.arrayContaining(['2026-09-01', '2026-05-15']));
    for (const text of createdTexts) {
      expect(text.length).toBeGreaterThan(0);
    }
    expect(container.textContent).not.toContain('null');
  });

  it('无错误时不渲染错误提示条', () => {
    render(<ChangeListView state={state({ data: fixtureList })} onSelect={() => {}} />);
    expect(screen.queryByTestId('error-note')).toBeNull();
  });

  it('暂无数据提示仅在空闲、无数据且无错误时出现', () => {
    const idle = render(<ChangeListView state={state({})} onSelect={() => {}} />);
    expect(idle.container.textContent).toContain('暂无数据，点击刷新获取。');

    const loadingState = render(
      <ChangeListView state={state({ loading: true })} onSelect={() => {}} />,
    );
    expect(loadingState.container.textContent).not.toContain('暂无数据');
    expect(loadingState.container.textContent).toContain('加载中');

    const errorState = render(
      <ChangeListView state={state({ error: 'IPC 断开' })} onSelect={() => {}} />,
    );
    expect(errorState.container.textContent).not.toContain('暂无数据');
    expect(errorState.container.textContent).toContain('列表加载失败');

    const withData = render(
      <ChangeListView state={state({ data: fixtureList })} onSelect={() => {}} />,
    );
    expect(withData.container.textContent).not.toContain('暂无数据');
  });

  it('active 为空但 archive 有分组时提示无进行中而不提示 workspace 为空', () => {
    const firstGroup = fixtureList.archiveGroups[0];
    const { container } = render(
      <ChangeListView
        state={state({ data: { active: [], archiveGroups: [firstGroup] } })}
        onSelect={() => {}}
      />,
    );
    expect(container.textContent).toContain('无进行中的 change。');
    expect(container.textContent).not.toContain('未发现任何 change 目录');
    expect(container.textContent).toContain('2026-09-01-first');
  });

  it('active 与 archive 均有内容时不提示 workspace 为空', () => {
    const { container } = render(
      <ChangeListView
        state={state({ data: { active: [fixtureList.active[0]], archiveGroups: [] } })}
        onSelect={() => {}}
      />,
    );
    expect(container.textContent).not.toContain('未发现任何 change 目录');
    expect(container.textContent).toContain('add-feature');
  });
});

describe('ChangeListView：头部刷新行（刷新入口自 App header 迁入）', () => {
  it('头部行先于 error-note 与数据区渲染，且 loading / error / 空数据 / 有数据四形态下「刷新列表」按钮均存在', () => {
    // error 态：头部行是根节点的第一个子元素，先于 error-note
    const withError = render(
      <ChangeListView state={state({ error: 'IPC 断开' })} onSelect={() => {}} />,
    );
    const root = withError.container.firstElementChild;
    const headerRow = screen.getByRole('button', { name: '刷新列表' }).parentElement;
    expect(headerRow).not.toBeNull();
    expect(headerRow).toBe(root?.firstElementChild);
    expect(root?.children[1]?.getAttribute('data-testid')).toBe('error-note');
    withError.unmount();

    // 始终在场：四形态下刷新按钮均存在（空态可操作的前提）
    const shapes = [
      state({ loading: true }),
      state({ error: 'IPC 断开' }),
      state({ data: null }),
      state({ data: fixtureList }),
    ];
    for (const shape of shapes) {
      const view = render(<ChangeListView state={shape} onSelect={() => {}} />);
      expect(within(view.container).getByRole('button', { name: '刷新列表' }) !== null).toBe(true);
      view.unmount();
    }
  });

  it('点击「刷新列表」→ state.refresh 调用恰一次', () => {
    const refresh = vi.fn();
    render(<ChangeListView state={state({ refresh })} onSelect={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: '刷新列表' }));

    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('loading=true：按钮 disabled、点击不触发 refresh（disabled={state.loading}）', () => {
    const refresh = vi.fn();
    render(<ChangeListView state={state({ loading: true, refresh })} onSelect={() => {}} />);

    const button = screen.getByRole('button', { name: '刷新列表' }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
    expect(refresh).not.toHaveBeenCalled();
  });

  it('空数据态（「暂无数据，点击刷新获取。」文案在场）：按钮仍可操作、点击触发 refresh', () => {
    const refresh = vi.fn();
    render(<ChangeListView state={state({ data: null, refresh })} onSelect={() => {}} />);

    expect(screen.getByText('暂无数据，点击刷新获取。') !== null).toBe(true);
    const button = screen.getByRole('button', { name: '刷新列表' }) as HTMLButtonElement;
    expect(button.disabled).toBe(false);
    fireEvent.click(button);
    expect(refresh).toHaveBeenCalledTimes(1);
  });
});
