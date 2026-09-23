import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';

import type { WorkspaceRecord } from '../types/dto';

/** 动作失败 toast 固定前缀（design D8：文案供断言；remove 的 store miss 幂等非错误不 toast） */
const ACTION_ERROR_PREFIX = {
  add: '添加 workspace 失败：',
  remove: '移除 workspace 失败：',
  touch: '切换 workspace 失败：',
} as const;

export interface WorkspaceState {
  root: string | null;
  workspaces: WorkspaceRecord[];
  loading: boolean;
  /**
   * 仅承载 list_workspaces 加载失败（双轨语义：add/remove/touch 动作失败在
   * hook 内直调 toast.error 呈现，不置本字段）。
   */
  error: string | null;
  add: (root: string) => Promise<WorkspaceRecord | null>;
  remove: (root: string) => Promise<boolean>;
  touch: (root: string) => Promise<boolean>;
}

/**
 * workspace 清单取数收口 hook：挂载自动 invoke("list_workspaces") 一次——
 * 全部 hooks 中唯一的自动取数例外（支撑启动自动恢复），此后由用户动作触发。
 * root 恒等于清单第一名（后端按 last_opened_at 降序返回）：清单非空即选中
 * 第一名，清单为空时 root 为 null 停欢迎屏。首次取得非空清单时以第一名
 * touch（fire-and-forget，先于 setRoot 派发，保证「挂载取数 → touch →
 * 以恢复根取列表」时序；失败经 toast 呈现，不阻断恢复链），此后刷新不再恢复，
 * 避免取数-恢复循环。错误呈现双轨：查询轨道 list_workspaces 失败置 error 态
 * inline 持久；动作轨道 add/remove/touch 失败 hook 内直调 toast.error
 * （固定前缀 + String(err)），不置 error 态，返回值（null / false）仅供流程
 * 控制。remove 返回 false（store miss）为幂等非错误：无 error、无 toast。
 * 无轮询、无文件 watch。
 */
export function useWorkspaces(): WorkspaceState {
  const [root, setRoot] = useState<string | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const restoredRef = useRef(false);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const { add, remove, touch } = useWorkspacesActions(refresh);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<WorkspaceRecord[]>('list_workspaces')
      .then((result) => {
        if (cancelled) return;
        setWorkspaces(result);
        const first = result[0]?.root ?? null;
        if (!restoredRef.current && first !== null) {
          restoredRef.current = true;
          void touch(first);
        }
        setRoot(first);
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
  }, [tick, touch]);

  return { root, workspaces, loading, error, add, remove, touch };
}

/** 动作轨道：失败直调 toast.error（不置 error 态），返回值仅供流程控制 */
function useWorkspacesActions(refresh: () => void) {
  const add = useCallback(
    async (root: string): Promise<WorkspaceRecord | null> => {
      try {
        const record = await invoke<WorkspaceRecord>('add_workspace', { root });
        refresh();
        return record;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.add}${String(err)}`);
        return null;
      }
    },
    [refresh],
  );

  const remove = useCallback(
    async (root: string): Promise<boolean> => {
      try {
        const hit = await invoke<boolean>('remove_workspace', { root });
        refresh();
        return hit;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.remove}${String(err)}`);
        return false;
      }
    },
    [refresh],
  );

  const touch = useCallback(
    async (root: string): Promise<boolean> => {
      try {
        const hit = await invoke<boolean>('touch_workspace', { root });
        refresh();
        return hit;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.touch}${String(err)}`);
        return false;
      }
    },
    [refresh],
  );

  return { add, remove, touch };
}
