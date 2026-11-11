import { useCallback, useEffect, useRef, useState } from 'react';

import { useFileWatchSignal } from '../../../hooks/use-file-watch-signal';
import { ipc } from '../../../lib/ipc';

/**
 * explore 笔记文档（名字 + 正文）。原由后端 `read_explore` 出线；命令退役后
 * 由本层自 `explore_doc_path` + `read_file` 组装，形状与旧 DTO 一致
 *（`explore-preview` 的投影契约零改动）。
 */
export interface ExploreDoc {
  name: string;
  content: string;
}

export interface ExploreDocState {
  /** 当前笔记内容；null 即未落盘 / 已删除（预览空态，记录保留不报错） */
  doc: ExploreDoc | null;
  loading: boolean;
  error: string | null;
  /** 显式重读磁盘 */
  refresh: () => void;
}

/**
 * 单次读取（挂载拉取与信号重拉共用）：路径先经 `resolvePath` 得到（布局派生
 * 路径由 `explore_doc_path` 给出，目录名知识不下沉前端），再经通用 `read_file`
 * 读字节；路径解析为 null 或文件缺失均归一为「未落盘」空态（不置 error）。
 */
function fetchDoc(
  resolvePath: () => Promise<string | null>,
  name: string,
  isCancelled: () => boolean,
  onDoc: (doc: ExploreDoc | null) => void,
  onError: (message: string) => void,
  onSettled: () => void,
): void {
  resolvePath()
    .then((path) => (path === null ? null : ipc.readFile(path, null, null)))
    .then((slice) => {
      if (isCancelled()) return;
      onDoc(slice === null ? null : { name, content: slice.content });
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
 *（`explore_doc_path` → `read_file`，布局派生路径按臂缓存、与 watch 订阅共用同
 * 一次解析）；信号驱动的重拉（失效信号通道，无内容）经 [`useFileWatchSignal`]
 * 订阅——500ms trailing 防抖合并为一次重拉，该件持有 Channel 建立、布局派生路径
 * 订阅、防抖窗口与退订（含迟到补发），订阅失败不阻断本件数据面（仍可显式刷新，
 * 仅失自动刷新）。臂键 = `root`␀`name`，**不含刷新 tick** ⇒ 显式刷新只重取数、
 * 不重订 watch。
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
  /** 当前臂的派生路径（单次解析、读取与 watch 订阅共用；臂变更即弃用） */
  const pathRef = useRef<{ key: string; path: Promise<string | null> } | null>(null);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  // 布局派生路径单点：同一臂（root␀name）只解析一次，读取与 watch 订阅复用同一
  // Promise（订阅侧 `resolvePath` 不另发一次 `explore_doc_path`）
  const resolvePath = useCallback((): Promise<string | null> => {
    if (root === null || name === null) return Promise.resolve(null);
    const key = `${root}\u0000${name}`;
    if (pathRef.current?.key !== key) {
      pathRef.current = { key, path: ipc.exploreDocPath(root, name) };
    }
    return pathRef.current.path;
  }, [root, name]);

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
      fetchDoc(resolvePath, name, isCancelled, setDoc, setError, () => setLoading(false));
    readRef.current = readDoc;
    setLoading(true);
    setError(null);
    readDoc();
    return () => {
      cancelled = true;
      readRef.current = null;
    };
  }, [root, name, tick, resolvePath]);

  // M2 留痕：防抖计时器归共享消费件、按臂键存续 ⇒ 显式刷新与**待发的信号**可能各拉
  // 一次 read_explore（保存后 500ms 内点刷新才会交错）；两次结果幂等、不置 loading、
  // 无 UI 副作用。为它给公共件开 cancelPending() 属为单次场景造抽象（proposal §5 M2
  // 否决），勿当重复拉取缺陷修回去。
  useFileWatchSignal({
    armKey: root && name ? `${root}\u0000${name}` : null,
    resolvePath,
    onSignal: () => readRef.current?.(),
  });

  return { doc, loading, error, refresh };
}
