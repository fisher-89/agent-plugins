import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeListState } from '../hooks/useChangeList';
import type { ChangeList } from '../types/dto';
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
    const { container } = render(
      <ChangeListView state={state({ data: fixtureList })} onSelect={() => {}} />,
    );
    expect(screen.getByText('add-feature') !== null).toBe(true);
    expect(screen.getByText('docs-only') !== null).toBe(true);
    const badges = Array.from(container.querySelectorAll('.badge')).map((b) => b.textContent ?? '');
    expect(badges.filter((text) => /^v[012]$/.test(text))).toEqual(
      expect.arrayContaining(['v2', 'v0', 'v1']),
    );
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
    // 按钮名本身含日期前缀，因此必须精确断言 .created span 的文本而非整页 textContent
    const createdTexts = Array.from(container.querySelectorAll('span.created')).map(
      (span) => span.textContent ?? '',
    );
    expect(createdTexts).toEqual(expect.arrayContaining(['2026-09-01', '2026-05-15']));
    for (const text of createdTexts) {
      expect(text.length).toBeGreaterThan(0);
    }
    expect(container.textContent).not.toContain('null');
  });

  it('无错误时不渲染错误提示条', () => {
    const { container } = render(
      <ChangeListView state={state({ data: fixtureList })} onSelect={() => {}} />,
    );
    expect(container.querySelector('.error-note')).toBeNull();
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
