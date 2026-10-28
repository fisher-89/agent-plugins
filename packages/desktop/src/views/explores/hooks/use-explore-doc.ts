import { useCallback, useEffect, useRef, useState } from 'react';

import { useFileWatchSignal } from '../../../hooks/use-file-watch-signal';
import { ipc } from '../../../lib/ipc';
import { type ExploreDoc } from '../../../types/generated/bindings';

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
  ipc
    .readExplore(root, name)
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

/**
 * 文档取数 hook（详情页右栏数据面）：mount / 参数变更显式拉取一次
 * invoke("read_explore")；信号驱动的重拉（失效信号通道，无内容）经
 * [`useFileWatchSignal`] 订阅——500ms trailing 防抖合并为一次 read_explore 重拉，
 * 该件持有 Channel 建立、布局派生路径订阅、防抖窗口与退订（含迟到补发），
 * 订阅失败不阻断本件数据面（仍可显式刷新，仅失自动刷新）。臂键 = `root`␀`name`，
 * **不含刷新 tick** ⇒ 显式刷新只重取数、不重订 watch。
 *
 * 信号重拉走**当前**取数 effect 实例的闭包（`readRef`）：不置 `loading`、不清
 * `error`（否则预览的 loading 会在每次外部保存时闪一次），且旧实例因 `cancelled`
 * 自动 no-op。
 */
export function useExploreDoc(root: string | null, name: string | null): ExploreDocState {
  const [doc, setDoc] = useState<ExploreDoc | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  /** 指向**当前**取数 effect 实例的取数闭包（信号重拉用；其 `cancelled` 由该实例 cleanup 置真） */
  const readRef = useRef<(() => void) | null>(null);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (!root || !name) {
      setDoc(null);
      setError(null);
      setLoading(false);
      readRef.current = null;
      return;
    }
    let cancelled = false;
    const isCancelled = () => cancelled;
    const readDoc = () =>
      fetchDoc(root, name, isCancelled, setDoc, setError, () => setLoading(false));
    readRef.current = readDoc;
    setLoading(true);
    setError(null);
    readDoc();
    return () => {
      cancelled = true;
      readRef.current = null;
    };
  }, [root, name, tick]);

  // M2 留痕：防抖计时器归共享消费件、按臂键存续 ⇒ 显式刷新与**待发的信号**可能各拉
  // 一次 read_explore（保存后 500ms 内点刷新才会交错）；两次结果幂等、不置 loading、
  // 无 UI 副作用。为它给公共件开 cancelPending() 属为单次场景造抽象（proposal §5 M2
  // 否决），勿当重复拉取缺陷修回去。
  useFileWatchSignal({
    armKey: root && name ? `${root}\u0000${name}` : null,
    // 空参数臂只为类型完备——armKey 为 null 时本件不会调用本回调（`!root || !name` 同口径）
    resolvePath: () =>
      root === null || name === null ? Promise.resolve(null) : ipc.exploreDocPath(root, name),
    onSignal: () => readRef.current?.(),
  });

  return { doc, loading, error, refresh };
}
