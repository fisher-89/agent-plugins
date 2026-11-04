import { useCallback, useEffect, useState } from 'react';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

export interface ChangeDeleteDialogProps {
  /** 待删条目（null = 弹窗闭合；name 恒展示面，id 恒寻址面） */
  target: { id: string; title: string; worktreeCapable: boolean } | null;
  /** 关窗（取消 / 遮罩 / Esc 与成功删除共用） */
  onOpenChange: (open: boolean) => void;
  /** 确认删除：失败应 reject（弹窗行内呈现后端原文并留窗） */
  onConfirm: (cleanWorktree: boolean) => Promise<void>;
}

/**
 * 删除确认弹窗（desktop-change-delete）：库记录（相位 / 步骤 / 运行史）与磁盘
 * 产物目录一并删除的不可撤销告知；带 worktree 的进行中条目另呈「同时清空
 * worktree」勾选（缺省不勾——误删未提交改动是更坏的失败）。
 *
 * 不复用 `StandardDialog`：其 footer 硬编码「取消 / 提交」且无 destructive
 * 变体，本弹窗需要破坏性按钮与「删除中」禁用态。
 */
export function ChangeDeleteDialog({
  target,
  onOpenChange,
  onConfirm,
}: ChangeDeleteDialogProps): React.JSX.Element {
  const [cleanWorktree, setCleanWorktree] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // 每次换目标重置勾选与错误（上一轮的状态不得泄漏到下一次确认）
  useEffect(() => {
    setCleanWorktree(false);
    setError(null);
  }, [target?.id]);

  const onConfirmClick = useCallback(() => {
    if (target === null || submitting) return;
    setSubmitting(true);
    setError(null);
    onConfirm(target.worktreeCapable && cleanWorktree)
      .then(() => {
        setSubmitting(false);
        onOpenChange(false);
      })
      .catch((err: unknown) => {
        setSubmitting(false);
        setError(String(err));
      });
  }, [target, submitting, cleanWorktree, onConfirm, onOpenChange]);

  return (
    <Dialog open={target !== null} onOpenChange={onOpenChange}>
      <DialogContent data-testid="change-delete-dialog">
        <DialogHeader>
          <DialogTitle>删除变更</DialogTitle>
          <DialogDescription>
            将删除「{target?.title ?? ''}」的库记录（相位 / 步骤 /
            运行史）与磁盘产物目录，操作不可撤销。
          </DialogDescription>
        </DialogHeader>
        {target?.worktreeCapable === true && (
          <label className="flex items-start gap-2 text-[13px]">
            <input
              type="checkbox"
              className="mt-0.5"
              checked={cleanWorktree}
              disabled={submitting}
              onChange={(event) => setCleanWorktree(event.target.checked)}
              data-testid="change-delete-clean-worktree"
            />
            <span>
              同时清空 worktree（移除 worktree 目录与分支 change/&lt;name&gt;）；不勾选则原样保留
            </span>
          </label>
        )}
        {error !== null && (
          <div
            className="break-all rounded-md bg-fail-bg px-2 py-1.5 text-xs text-fail"
            data-testid="change-delete-error"
          >
            {error}
          </div>
        )}
        <DialogFooter>
          <DialogClose render={<Button variant="outline" />}>取消</DialogClose>
          <Button
            variant="destructive"
            disabled={submitting}
            onClick={onConfirmClick}
            data-testid="change-delete-confirm"
          >
            {submitting ? '删除中…' : '删除'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
