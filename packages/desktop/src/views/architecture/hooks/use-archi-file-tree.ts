import { useEffect, useState } from 'react';

import { commands, type ArchiFileTree } from '../../../types/generated/bindings';

export interface ArchiFileTreeState {
  /** 文件树（无根 / 首轮未回 / 取数失败为 null——画布照常渲染） */
  tree: ArchiFileTree | null;
  /** 取数进行中 */
  loading: boolean;
  /** 取数失败（命令 reject；行内持久呈现，不阻断图与抽屉） */
  error: string | null;
}

/**
 * 文件树取数 hook：三触发（挂载 / `root` 变化 / 上层刷新 `key` 重挂），零 watch
 * 零轮询零定时器。
 *
 * 手动刷新由 `ArchitectureView` 的 tick → `key` 重挂统一覆盖（与图 / 漂移同步），
 * 故本 hook 不暴露刷新成员——避免出现第二个触发入口。`root === null` 不 invoke。
 * 取数失败清空 `tree` 与已载入 root（旧树 MUST NOT 冒充本次结果），且只落 `error`
 * ——树与图 / 漂移**独立失败**。
 */
export function useArchiFileTree(root: string | null): ArchiFileTreeState {
  const [tree, setTree] = useState<ArchiFileTree | null>(null);
  const [loadedRoot, setLoadedRoot] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (root === null) {
      setTree(null);
      setLoadedRoot(null);
      setLoading(false);
      setError(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .archiFileTree(root)
      .then((result) => {
        if (cancelled) return;
        setTree(result);
        setLoadedRoot(root);
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setTree(null);
        setLoadedRoot(null);
        setError(String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root]);

  return { tree: loadedRoot === root ? tree : null, loading, error };
}
