import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { StandardPagination } from './pagination';

// 零进程边界：不 mock 被测组件与 ui/** 底座（base-ui Button 真实渲染）；
// onPageChange 以 vi.fn() 注入观察面。查询只走 role / name / data-testid，
// 不断言 class。

type PaginationOverrides = {
  page?: number;
  totalPages?: number;
  total?: number;
  disabled?: boolean;
  onPageChange?: (page: number) => void;
};

/** 挂载 StandardPagination（data-testid 固定作分页器查询挂钩）。 */
function renderPagination(overrides: PaginationOverrides = {}) {
  const {
    page = 1,
    totalPages = 3,
    total,
    disabled,
    onPageChange = vi.fn<(page: number) => void>(),
  } = overrides;
  const view = render(
    <StandardPagination
      page={page}
      totalPages={totalPages}
      total={total}
      onPageChange={onPageChange}
      disabled={disabled}
      data-testid="分页器"
    />,
  );
  return { onPageChange, view };
}

/** 页码按钮（data-page 锚定）。 */
function pageLinks(): HTMLElement[] {
  return screen.getAllByTestId('page-link');
}

describe('StandardPagination', () => {
  it('正向：上一页 / 页码按钮 / 下一页齐出，当前页 aria-current=page', () => {
    renderPagination({ page: 2, totalPages: 3 });

    expect(screen.getByRole('button', { name: '上一页' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '下一页' })).toBeTruthy();
    const links = pageLinks();
    expect(links.map((link) => link.textContent)).toEqual(['1', '2', '3']);
    expect(links.at(1)?.getAttribute('aria-current')).toBe('page');
    expect(links.at(0)?.getAttribute('aria-current')).toBe(null);
  });

  it('正向：点页码 / 下一页 / 上一页 分别回调目标页', () => {
    const { onPageChange } = renderPagination({ page: 2, totalPages: 3 });

    fireEvent.click(screen.getByRole('button', { name: '第 3 页' }));
    expect(onPageChange).toHaveBeenCalledWith(3);

    fireEvent.click(screen.getByRole('button', { name: '下一页' }));
    expect(onPageChange).toHaveBeenLastCalledWith(3);

    fireEvent.click(screen.getByRole('button', { name: '上一页' }));
    expect(onPageChange).toHaveBeenLastCalledWith(1);
  });

  it('边界：首页上一页禁用、末页下一页禁用，越界回调不发', () => {
    const { onPageChange, view } = renderPagination({ page: 1, totalPages: 3 });
    const firstPrev = within(view.getByTestId('分页器')).getByRole('button', {
      name: '上一页',
    });
    expect(firstPrev.hasAttribute('disabled')).toBe(true);
    fireEvent.click(firstPrev);
    expect(onPageChange).not.toHaveBeenCalled();
    view.unmount();

    const { onPageChange: onNext, view: lastView } = renderPagination({
      page: 3,
      totalPages: 3,
    });
    const lastNext = within(lastView.getByTestId('分页器')).getByRole('button', {
      name: '下一页',
    });
    expect(lastNext.hasAttribute('disabled')).toBe(true);
    fireEvent.click(lastNext);
    expect(onNext).not.toHaveBeenCalled();
  });

  it('边界：disabled 全量拦点击（页码与上下页）', () => {
    const { onPageChange } = renderPagination({
      page: 2,
      totalPages: 3,
      disabled: true,
    });
    for (const button of screen.getAllByRole('button')) {
      expect(button.hasAttribute('disabled')).toBe(true);
    }
    fireEvent.click(screen.getByRole('button', { name: '第 3 页' }));
    fireEvent.click(screen.getByRole('button', { name: '下一页' }));
    expect(onPageChange).not.toHaveBeenCalled();
  });

  it('正向：total 在场呈摘要行「第 2 / 3 页（共 47 条）」，缺席不呈', () => {
    renderPagination({ page: 2, totalPages: 3, total: 47 });
    expect(screen.getByText('第 2 / 3 页（共 47 条）')).toBeTruthy();
  });

  it('异常：total 缺席不呈摘要行，页码控件仍在场', () => {
    renderPagination({ page: 1, totalPages: 3 });
    expect(screen.queryByText(/第 \d+ \/ \d+ 页/)).toBe(null);
    expect(pageLinks()).toHaveLength(3);
  });

  it('边界：totalPages=1 单页仍渲染（可见性由调用方把关）', () => {
    renderPagination({ page: 1, totalPages: 1, total: 1 });
    const links = pageLinks();
    expect(links).toHaveLength(1);
    expect(links.at(0)?.textContent).toBe('1');
    expect(screen.getByRole('button', { name: '上一页' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '下一页' })).toBeTruthy();
  });

  it('边界：页多时窗口缩为 1 … 当前邻域 … 末页（双省略号）', () => {
    renderPagination({ page: 5, totalPages: 10 });
    const links = pageLinks();
    expect(links.map((link) => link.textContent)).toEqual(['1', '4', '5', '6', '10']);
    expect(screen.getAllByText('More pages')).toHaveLength(2);
  });
});
