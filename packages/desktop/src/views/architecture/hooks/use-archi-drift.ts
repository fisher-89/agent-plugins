import { useCallback, useEffect, useState } from 'react';

import {
  commands,
  type ArchiDriftMode,
  type ArchiDriftReport,
} from '../../../types/generated/bindings';

export interface ArchiDriftState {
  /** 漂移报告（无根 / 首轮未回 / 取数失败为 null——画布照常渲染零标记图） */
  report: ArchiDriftReport | null;
  /** 取数进行中 */
  loading: boolean;
  /** 取数失败（命令 reject；行内持久呈现，不阻断图渲染） */
  error: string | null;
  /** 手动刷新（第三种触发；零 watch 零轮询） */
  refresh: () => void;
}

/**
 * 漂移取数 hook：三触发（挂载 / 观察根或模式变化 / 手动刷新），零 watch。
 *
 * `mode` 由挂载点给定（`/architecture` = `full`、change 详情 = `incremental`），
 * 直传 `archi_drift`；`root === null` 不 invoke。取数失败降级为 `report = null` +
 * `error`——漂移面是叠加信息，MUST NOT 影响图本身的渲染。
 */
export function useArchiDrift(root: string | null, mode: ArchiDriftMode): ArchiDriftState {
  const [report, setReport] = useState<ArchiDriftReport | null>(null);
  const [loadedKey, setLoadedKey] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((value) => value + 1), []);

  useEffect(() => {
    if (root === null) {
      setReport(null);
      setLoadedKey(null);
      setLoading(false);
      setError(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .archiDrift(root, mode)
      .then((result) => {
        if (cancelled) return;
        setReport(result);
        setLoadedKey(`${mode}:${root}`);
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        // 取数失败：清空报告与已载入键（MUST NOT 让上一轮旧报告冒充本次结论——
        // 画布退回零标记图，只留行内提示）
        setReport(null);
        setLoadedKey(null);
        setError(String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, mode, tick]);

  return { report: loadedKey === `${mode}:${root}` ? report : null, loading, error, refresh };
}
