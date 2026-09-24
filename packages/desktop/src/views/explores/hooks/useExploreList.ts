import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import type { ExploreRecord } from '../../../types/dto';

export interface ExploreListState {
  /** 探索记录清单（store 按当前 root 过滤，id 升序；root 未就绪/切换过渡轮为空） */
  records: ExploreRecord[];
  loading: boolean;
  error: string | null;
  /** 显式刷新清单 */
  refresh: () => void;
  /** 新话题建档 / 导入绑定共用：invoke 后 refresh */
  create: (name: string) => void;
  /** in-place 改名（保主键保链）：invoke 后 refresh */
  rename: (name: string, newName: string) => void;
  /** 删除记录（不动磁盘文件）：invoke 后 refresh */
  remove: (name: string) => void;
}

interface ExploreActions {
  create: (name: string) => void;
  rename: (name: string, newName: string) => void;
  remove: (name: string) => void;
}

/** 记录动作薄封装：invoke 后 refresh；失败落 error 态（不中断页面） */
function useExploreActions(
  root: string | null,
  refresh: () => void,
  onError: (message: string) => void,
): ExploreActions {
  const create = useCallback(
    (name: string) => {
      if (!root) return;
      invoke<ExploreRecord>('create_explore_record', { root, name })
        .then(refresh)
        .catch((err: unknown) => onError(String(err)));
    },
    [root, refresh, onError],
  );
  const rename = useCallback(
    (name: string, newName: string) => {
      if (!root) return;
      invoke<ExploreRecord>('rename_explore_record', { root, name, newName })
        .then(refresh)
        .catch((err: unknown) => onError(String(err)));
    },
    [root, refresh, onError],
  );
  const remove = useCallback(
    (name: string) => {
      if (!root) return;
      invoke<boolean>('delete_explore_record', { root, name })
        .then(refresh)
        .catch((err: unknown) => onError(String(err)));
    },
    [root, refresh, onError],
  );
  return { create, rename, remove };
}

/**
 * 探索清单 hook：root 变更与显式动作触发取数（invoke("list_explore_records")），
 * 无轮询。清单数据带归属 root 标记——root 切换的过渡轮不呈现旧根记录（抑制
 * 「新根 + 旧选中」串数据，照 ChangeView 过渡抑制哲学）。
 * create / rename / remove 为薄动作封装（invoke 后 refresh）；组件不直接 invoke。
 */
export function useExploreList(root: string | null): ExploreListState {
  const [records, setRecords] = useState<ExploreRecord[]>([]);
  const [recordsRoot, setRecordsRoot] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const onError = useCallback((message: string) => setError(message), []);
  const actions = useExploreActions(root, refresh, onError);

  useEffect(() => {
    if (!root) {
      setRecords([]);
      setRecordsRoot(null);
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<ExploreRecord[]>('list_explore_records', { root })
      .then((result) => {
        if (cancelled) return;
        setRecords(result);
        setRecordsRoot(root);
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

  return {
    records: recordsRoot === root ? records : [],
    loading,
    error,
    refresh,
    create: actions.create,
    rename: actions.rename,
    remove: actions.remove,
  };
}
