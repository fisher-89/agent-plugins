import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';

import { ipc } from '../../../lib/ipc';
import { type ChangeList } from '../../../types/generated/bindings';

export interface ChangeListState {
  data: ChangeList | null;
  loading: boolean;
  error: string | null;
  refresh: () => void;
  /** 删除变更（库记录级联 + 产物目录 + 可选 worktree 清空）：invoke 后 refresh
   * 并经 sonner 呈现结果；失败向外 rethrow（确认弹窗行内呈现，不落清单 error 态） */
  remove: (id: string, cleanWorktree: boolean) => Promise<void>;
}

/** 待删条目的展示标题（清单为其取数源；缺席回落 id，不阻断删除） */
function titleOf(data: ChangeList | null, id: string): string {
  if (data === null) return id;
  const hit =
    data.active.find((summary) => summary.id === id) ??
    data.archiveGroups.flatMap((group) => group.changes).find((summary) => summary.id === id);
  return hit?.title ?? id;
}

/**
 * 列表取数 hook：仅显式 refresh（或选定 workspace）触发 invoke("list_changes")。
 * 无轮询、无文件 watch；未来换推送只改本文件内部实现。列表为 db 单源全量
 * （`ChangeSummary.id` 随 bindings 再生自动跟随——行键 / 导航恒 id），零磁盘
 * 扫描触点，无 db 记录的存量 CLI change 零发现（签名零变化）。
 */
export function useChangeList(root: string | null): ChangeListState {
  const [data, setData] = useState<ChangeList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  const remove = useCallback(
    async (id: string, cleanWorktree: boolean): Promise<void> => {
      if (!root) return;
      const title = titleOf(data, id);
      // 失败向外 rethrow：确认弹窗按行内错误呈现后端原文，弹窗留窗可重试
      const outcome = await ipc.deleteChange(root, id, cleanWorktree);
      refresh();
      // 成功 toast：警告（如 worktree / 分支保留注记）优先以 warning 呈现，
      // 无警告时给成功收束。弹窗随即关闭，用户唯一可见的确认面就是这条提示。
      if (outcome.warnings.length > 0) {
        toast.warning(`已删除变更「${title}」`, { description: outcome.warnings.join('\n') });
      } else {
        toast.success(`已删除变更「${title}」`);
      }
    },
    [root, data, refresh],
  );

  useEffect(() => {
    if (!root) {
      setData(null);
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    ipc
      .listChanges(root)
      .then((result) => {
        if (cancelled) return;
        setData(result);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(String(err));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, tick]);

  return { data, loading, error, refresh, remove };
}
