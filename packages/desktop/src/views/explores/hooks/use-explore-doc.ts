import { Channel, invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import type { ExploreDoc, FileWatchEvent } from '../../../types/dto';

/** watch 信号防抖窗口（毫秒，trailing）：窗口内多次信号合并为一次显式拉取 */
const WATCH_DEBOUNCE_MS = 500;

export interface ExploreDocState {
  /** 当前笔记内容；null 即未落盘 / 已删除（预览空态，记录保留不报错） */
  doc: ExploreDoc | null;
  loading: boolean;
  error: string | null;
  /** 显式重读磁盘 */
  refresh: () => void;
}

/** 单次读取（挂载拉取与信号重拉共用） */
function fetchDoc(
  root: string,
  name: string,
  isCancelled: () => boolean,
  onDoc: (doc: ExploreDoc | null) => void,
  onError: (message: string) => void,
  onSettled: () => void,
): void {
  invoke<ExploreDoc | null>('read_explore', { root, name })
    .then((result) => {
      if (isCancelled()) return;
      onDoc(result);
      onSettled();
    })
    .catch((err: unknown) => {
      if (isCancelled()) return;
      onError(String(err));
      onSettled();
    });
}

/** 防抖合并器（trailing）：窗口内多次信号合并为一次拉取 */
function createDebouncer(
  delayMs: number,
  fn: () => void,
): {
  push: () => void;
  cancel: () => void;
} {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return {
    push: () => {
      if (timer !== null) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        fn();
      }, delayMs);
    },
    cancel: () => {
      if (timer !== null) clearTimeout(timer);
    },
  };
}

/** 布局派生路径 + 订阅单文件；返回退订闭包（幂等，可在卸载后调用） */
async function armWatch(
  root: string,
  name: string,
  channel: Channel<FileWatchEvent>,
  isCancelled: () => boolean,
): Promise<() => void> {
  const path = await invoke<string | null>('explore_doc_path', { root, name });
  if (path === null || isCancelled()) return () => {};
  const subscriptionId = await invoke<number>('watch_subscribe', { onEvent: channel, path });
  return () => {
    void invoke<boolean>('watch_unsubscribe', { subscriptionId });
  };
}

/**
 * 文档取数 hook（详情页右栏数据面）：mount / 参数变更显式拉取一次
 * invoke("read_explore")；随后经 invoke("explore_doc_path") 取布局派生路径并
 * invoke("watch_subscribe") 订阅当前笔记单文件（第二个被认可的推送语义——
 * 失效信号通道，无内容）：信号 500ms trailing 防抖合并为一次 read_explore
 * 重拉；卸载（或参数变更）invoke("watch_unsubscribe") 退订，退订后不再有
 * 信号。订阅失败不阻断页面（数据面仍可显式刷新，仅失自动刷新）。
 */
export function useExploreDoc(root: string | null, name: string | null): ExploreDocState {
  const [doc, setDoc] = useState<ExploreDoc | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (!root || !name) {
      setDoc(null);
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    const isCancelled = () => cancelled;
    let unsubscribe: (() => void) | null = null;
    const readDoc = () =>
      fetchDoc(root, name, isCancelled, setDoc, setError, () => setLoading(false));
    const signals = createDebouncer(WATCH_DEBOUNCE_MS, readDoc);
    setLoading(true);
    setError(null);
    readDoc();
    const channel = new Channel<FileWatchEvent>();
    channel.onmessage = signals.push;
    armWatch(root, name, channel, isCancelled)
      .then((off) => {
        if (cancelled) off();
        else unsubscribe = off;
      })
      .catch(() => {
        // 订阅失败不阻断数据面：页面仍可显式刷新（仅失自动刷新），不静默成功
      });
    return () => {
      cancelled = true;
      signals.cancel();
      unsubscribe?.();
    };
  }, [root, name, tick]);

  return { doc, loading, error, refresh };
}
