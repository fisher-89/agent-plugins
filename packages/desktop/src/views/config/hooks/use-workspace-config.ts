import { useCallback, useEffect, useState } from 'react';

import { commands, type WorkspaceConfigReport } from '../../../types/generated/bindings';

export interface WorkspaceConfigState {
  /** 配置报告（root 未就绪/切换过渡轮为 null：旧根报告不呈现） */
  data: WorkspaceConfigReport | null;
  /** 解析进行中（贯穿请求全程，供刷新钮 disabled） */
  loading: boolean;
  /** 解析失败（命令 reject，inline 持久） */
  error: string | null;
  /** 显式刷新：重新发起一次完整解析 */
  refresh: () => void;
}

/**
 * 配置取数收口 hook：显式刷新模型——进入页面 / 点刷新各自以 `[root, tick]`
 * 依赖发起一次 invoke（commands.workspaceConfig typed 调用），无轮询、无
 * watch、无缓存。`cancelled` 防串轮；报告带归属 root 标记，切换工作区的
 * 过渡轮不呈现旧根数据（use-code-stats 抑制哲学）。组件不直接 invoke。
 */
export function useWorkspaceConfig(root: string): WorkspaceConfigState {
  const [data, setData] = useState<WorkspaceConfigReport | null>(null);
  const [dataRoot, setDataRoot] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .workspaceConfig(root)
      .then((result) => {
        if (cancelled) return;
        setData(result);
        setDataRoot(root);
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
    data: dataRoot === root ? data : null,
    loading,
    error,
    refresh,
  };
}
