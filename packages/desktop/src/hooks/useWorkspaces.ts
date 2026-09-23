import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';

import type { WorkspaceRecord } from '../types/dto';

/** 动作失败 toast 固定前缀（design D8：文案供断言；remove 的 store miss 幂等非错误不 toast） */
const ACTION_ERROR_PREFIX = {
  add: '添加 workspace 失败：',
  remove: '移除 workspace 失败：',
} as const;

export interface WorkspaceState {
  root: string | null;
  workspaces: WorkspaceRecord[];
  loading: boolean;
  /**
   * 仅承载 list_workspaces 加载失败（双轨语义：add/remove 动作失败在
   * hook 内直调 toast.error 呈现，不置本字段）。
   */
  error: string | null;
  add: (root: string) => Promise<WorkspaceRecord | null>;
  remove: (root: string) => Promise<boolean>;
  /** 清单项点击：本地切换当前根（无后端调用，清单顺序不变） */
  select: (root: string) => void;
}

/**
 * workspace 清单取数收口 hook：挂载自动 invoke("list_workspaces") 一次——
 * 全部 hooks 中唯一的自动取数例外（支撑启动自动恢复），此后由用户动作触发。
 * 当前根的取值规则：每次取数回调中，当前根仍在清单则
 * 保持；不在（含启动时为 null、被移除）则取默认序第一名，空清单回欢迎屏
 * （null）。add 成功直接以返回记录的 canonical root 为当前根（新记录在
 * 默认序中未必居首）；select 为纯本地切换，无后端调用。错误呈现双轨：
 * 查询轨道 list_workspaces 失败置 error 态 inline 持久；动作轨道
 * add/remove 失败 hook 内直调 toast.error（固定前缀 + String(err)），
 * 不置 error 态，返回值（null / false）仅供流程控制。remove 返回 false
 * （store miss）为幂等非错误：无 error、无 toast。无轮询、无文件 watch。
 */
export function useWorkspaces(): WorkspaceState {
  const [root, setRoot] = useState<string | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const rootRef = useRef<string | null>(null);

  /** root 唯一写口：同步置 ref（异步取数回调读最新值）+ state（驱动渲染） */
  const applyRoot = useCallback((next: string | null) => {
    rootRef.current = next;
    setRoot(next);
  }, []);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const { add, remove } = useWorkspacesActions(applyRoot, refresh);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<WorkspaceRecord[]>('list_workspaces')
      .then((result) => {
        if (cancelled) return;
        setWorkspaces(result);
        // 当前根仍在清单则保持；不在（启动 null / 被移除）则取默认序第一名
        const current = rootRef.current;
        const kept =
          current !== null && result.some((record) => record.root === current)
            ? current
            : (result[0]?.root ?? null);
        applyRoot(kept);
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
  }, [tick, applyRoot]);

  const select = useCallback((next: string) => applyRoot(next), [applyRoot]);

  return { root, workspaces, loading, error, add, remove, select };
}

/** 动作轨道：失败直调 toast.error（不置 error 态），返回值仅供流程控制 */
function useWorkspacesActions(applyRoot: (next: string | null) => void, refresh: () => void) {
  const add = useCallback(
    async (root: string): Promise<WorkspaceRecord | null> => {
      try {
        const record = await invoke<WorkspaceRecord>('add_workspace', { root });
        // 直接以返回记录的 canonical root 为当前根（新记录默认序未必居首）
        applyRoot(record.root);
        refresh();
        return record;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.add}${String(err)}`);
        return null;
      }
    },
    [applyRoot, refresh],
  );

  const remove = useCallback(
    async (root: string): Promise<boolean> => {
      try {
        const hit = await invoke<boolean>('remove_workspace', { root });
        // 当前根被移除时由取数回调顺延取剩余第一名（或回欢迎屏）
        refresh();
        return hit;
      } catch (err: unknown) {
        toast.error(`${ACTION_ERROR_PREFIX.remove}${String(err)}`);
        return false;
      }
    },
    [refresh],
  );

  return { add, remove };
}
