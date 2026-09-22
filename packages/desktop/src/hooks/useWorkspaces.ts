import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';

import type { WorkspaceRecord } from '../types/dto';

export interface WorkspaceState {
  root: string | null;
  workspaces: WorkspaceRecord[];
  loading: boolean;
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
 * 以恢复根取列表」时序），此后刷新不再恢复，避免取数-恢复循环。
 * add / remove / touch 成功后内部刷新清单，失败置 error 态并返回
 * null / false。无轮询、无文件 watch。
 */
export function useWorkspaces(): WorkspaceState {
  const [root, setRoot] = useState<string | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const restoredRef = useRef(false);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const { add, remove, touch } = useWorkspacesActions(refresh, setError);

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

function useWorkspacesActions(refresh: () => void, setError: (err: string) => void) {
  const add = useCallback(
    async (root: string): Promise<WorkspaceRecord | null> => {
      try {
        const record = await invoke<WorkspaceRecord>('add_workspace', { root });
        refresh();
        return record;
      } catch (err: unknown) {
        setError(String(err));
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
        setError(String(err));
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
        setError(String(err));
        return false;
      }
    },
    [refresh],
  );

  return { add, remove, touch };
}
