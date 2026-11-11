/**
 * 抽屉取数 hook：元素详情（`archi_module`，`fqn` 变化即重取）。显式取数（零
 * watch 零轮询）；文档正文由 `components/file` 的 `FileViewer` 自取。
 */
import { useEffect, useState } from 'react';

import { commands, type ArchiModuleDetail } from '../../../types/generated/bindings';

export interface ArchiModuleState {
  /** 元素详情（无根 / 无选中为 null） */
  detail: ArchiModuleDetail | null;
  /** 取数进行中 */
  loading: boolean;
  /** 取数失败（命令 reject） */
  error: string | null;
}

export function useArchiModule(root: string | null, fqn: string | null): ArchiModuleState {
  const [detail, setDetail] = useState<ArchiModuleDetail | null>(null);
  const [loadedKey, setLoadedKey] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (root === null || fqn === null) {
      setDetail(null);
      setLoadedKey(null);
      setLoading(false);
      setError(null);
      return;
    }
    const key = `${root}\u0000${fqn}`;
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .archiModule(root, fqn)
      .then((result) => {
        if (cancelled) return;
        setDetail(result);
        setLoadedKey(key);
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setError(String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, fqn]);

  const currentKey = root === null || fqn === null ? null : `${root}\u0000${fqn}`;
  return { detail: loadedKey === currentKey ? detail : null, loading, error };
}
