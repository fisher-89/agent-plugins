import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { ChangeDeleteDialog } from './change-delete-dialog';

const TARGET = { id: 'chg-1', title: '待删变更', worktreeCapable: true };

afterEach(cleanup);

/** 受控装置：外部持有 target 与 open 态（关窗回调只记录）。 */
function renderDialog({
  target = TARGET as { id: string; title: string; worktreeCapable: boolean } | null,
  onConfirm = vi.fn(() => Promise.resolve()),
  onOpenChange = vi.fn(),
}: {
  target?: { id: string; title: string; worktreeCapable: boolean } | null;
  onConfirm?: (cleanWorktree: boolean) => Promise<void>;
  onOpenChange?: (open: boolean) => void;
} = {}) {
  return render(
    <ChangeDeleteDialog target={target} onConfirm={onConfirm} onOpenChange={onOpenChange} />,
  );
}

describe('ChangeDeleteDialog：不可撤销告知与勾选语义', () => {
  it('target 非空时呈现标题、条目标题与不可撤销告知', () => {
    renderDialog();

    expect(screen.getByTestId('change-delete-dialog') !== null).toBe(true);
    expect(screen.getByText('删除变更') !== null).toBe(true);
    expect(screen.getByText(/待删变更/)).toBeTruthy();
    expect(screen.getByText(/不可撤销/)).toBeTruthy();
  });

  it('target 为 null 时不渲染弹窗内容', () => {
    renderDialog({ target: null });

    expect(screen.queryByTestId('change-delete-dialog')).toBeNull();
  });

  it('worktreeCapable=true：勾选框在场且缺省未勾选', () => {
    renderDialog();

    const checkbox = screen.getByTestId('change-delete-clean-worktree') as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    expect(screen.getByText(/分支 change\/<name>/)).toBeTruthy();
  });

  it('worktreeCapable=false（归档条目）：零勾选框', () => {
    renderDialog({ target: { ...TARGET, worktreeCapable: false } });

    expect(screen.queryByTestId('change-delete-clean-worktree')).toBeNull();
  });

  it('确认未勾选 → onConfirm(false) 并关窗', async () => {
    const onConfirm = vi.fn(() => Promise.resolve());
    const onOpenChange = vi.fn();
    renderDialog({ onConfirm, onOpenChange });

    fireEvent.click(screen.getByTestId('change-delete-confirm'));

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(onConfirm).toHaveBeenCalledWith(false);
  });

  it('勾选后确认 → onConfirm(true)', async () => {
    const onConfirm = vi.fn(() => Promise.resolve());
    renderDialog({ onConfirm });

    fireEvent.click(screen.getByTestId('change-delete-clean-worktree'));
    fireEvent.click(screen.getByTestId('change-delete-confirm'));

    await waitFor(() => expect(onConfirm).toHaveBeenCalledWith(true));
  });

  it('worktreeCapable=false 时确认恒传 false（勾选面零参与）', async () => {
    const onConfirm = vi.fn(() => Promise.resolve());
    renderDialog({ target: { ...TARGET, worktreeCapable: false }, onConfirm });

    fireEvent.click(screen.getByTestId('change-delete-confirm'));

    await waitFor(() => expect(onConfirm).toHaveBeenCalledWith(false));
  });
});

describe('ChangeDeleteDialog：失败行内呈现且留窗', () => {
  it('onConfirm reject → 行内错误呈现后端原文、弹窗不关、按钮恢复可用', async () => {
    const onConfirm = vi.fn(() =>
      Promise.reject('change "chg-1" 正在运行中，请先停止运行后再删除'),
    );
    const onOpenChange = vi.fn();
    renderDialog({ onConfirm, onOpenChange });

    fireEvent.click(screen.getByTestId('change-delete-confirm'));

    const error = await screen.findByTestId('change-delete-error');
    expect(error.textContent).toContain('正在运行中');
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(screen.getByTestId('change-delete-confirm').hasAttribute('disabled')).toBe(false);
  });

  it('再次确认清空上轮错误（错误不跨提交残留）', async () => {
    const onConfirm = vi
      .fn<(cleanWorktree: boolean) => Promise<void>>()
      .mockRejectedValueOnce('第一次失败')
      .mockResolvedValueOnce(undefined);
    renderDialog({ onConfirm });

    fireEvent.click(screen.getByTestId('change-delete-confirm'));
    await screen.findByTestId('change-delete-error');

    fireEvent.click(screen.getByTestId('change-delete-confirm'));
    await waitFor(() => expect(screen.queryByTestId('change-delete-error')).toBeNull());
  });
});
