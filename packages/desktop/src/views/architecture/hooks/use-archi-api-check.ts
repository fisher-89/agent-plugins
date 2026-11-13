import { useCallback, useEffect, useState } from 'react';

import { commands, type ArchiApiCheck } from '../../../types/generated/bindings';

export interface ArchiApiCheckState {
  /** 生成物新鲜度事实（无根 / 首轮未回 / 取数失败为 null） */
  check: ArchiApiCheck | null;
  /** 取数进行中 */
  loading: boolean;
  /** 取数失败（命令 reject；面板行内呈现，不阻断画布） */
  error: string | null;
  /** 手动刷新（第三种触发；零 watch 零轮询） */
  refresh: () => void;
}

/**
 * 契约检查取数 hook：三触发（挂载 / `root` 变化 / `refresh()`），零 watch 零轮询零
 * 定时器。`root === null` 不 invoke。
 *
 * 取数失败清空上一轮三类差异（旧结果 MUST NOT 冒充本次结论），只落 `error`——本轴是
 * 叠加信息，MUST NOT 阻断图渲染。全量渲染成本高，故只在契约视图挂载时取数（切换左栏
 * 视图不连带重跑）。
 */
export function useArchiApiCheck(root: string | null): ArchiApiCheckState {
  const [check, setCheck] = useState<ArchiApiCheck | null>(null);
  const [loadedRoot, setLoadedRoot] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((value) => value + 1), []);

  useEffect(() => {
    if (root === null) {
      setCheck(null);
      setLoadedRoot(null);
      setLoading(false);
      setError(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .archiApiCheck(root)
      .then((result) => {
        if (cancelled) return;
        setCheck(result);
        setLoadedRoot(root);
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setCheck(null);
        setLoadedRoot(null);
        setError(String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, tick]);

  return { check: loadedRoot === root ? check : null, loading, error, refresh };
}
