import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import type { ChangeList } from '../types/dto';

export interface ChangeListState {
  data: ChangeList | null;
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

/**
 * 列表取数 hook：仅显式 refresh（或选定 workspace）触发 invoke("list_changes")。
 * 无轮询、无文件 watch；未来换推送只改本文件内部实现。
 */
export function useChangeList(root: string | null): ChangeListState {
  const [data, setData] = useState<ChangeList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

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
    invoke<ChangeList>('list_changes', { root })
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

  return { data, loading, error, refresh };
}
