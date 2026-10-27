/**
 * 画布取数 hook：**显式取数**——只在三种触发下发起 invoke（组件挂载 / `root`
 * 变化 / 手动刷新），零目录监听、零轮询、零跨调用缓存（沿既有「显式取数」红线）。
 *
 * 停摆态与正常态同形透出：结构错时后端返回零图数据 + 诊断清单，本 hook 只做透传
 * （停摆判据 = `diagnostics` 非空，不在前端二次判定）。`root` 为 null（壳态无根）
 * 时不发 invoke。
 */
import { useCallback, useEffect, useState } from 'react';

import { commands, type ArchiGraph } from '../../../types/generated/bindings';

export interface ArchiGraphState {
  /** 画布数据（停摆/正常两态；无根或首轮未回为 null） */
  graph: ArchiGraph | null;
  /** 取数进行中 */
  loading: boolean;
  /** 取数失败（命令 reject，行内持久呈现） */
  error: string | null;
  /** 手动刷新（第三种触发；唯一显式入口） */
  refresh: () => void;
}

export function useArchiGraph(root: string | null): ArchiGraphState {
  const [graph, setGraph] = useState<ArchiGraph | null>(null);
  const [loadedRoot, setLoadedRoot] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((value) => value + 1), []);

  useEffect(() => {
    if (root === null) {
      setGraph(null);
      setLoadedRoot(null);
      setLoading(false);
      setError(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .archiGraph(root)
      .then((result) => {
        if (cancelled) return;
        setGraph(result);
        setLoadedRoot(root);
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
  }, [root, tick]);

  return { graph: loadedRoot === root ? graph : null, loading, error, refresh };
}
