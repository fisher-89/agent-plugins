import { useCallback, useEffect, useState } from 'react';

import { commands, type CodeStatsReport } from '../../../types/generated/bindings';

/** 默认解析深度（spec：depth 默认 5） */
const DEFAULT_DEPTH = 5;

export interface CodeStatsState {
  /** 三面统计报告（root 未就绪/切换过渡轮为 null：旧根报告不呈现） */
  report: CodeStatsReport | null;
  /** 解析进行中（贯穿请求全程，供刷新钮 disabled） */
  loading: boolean;
  /** 解析失败（查询轨 error 态，inline 持久） */
  error: string | null;
  /** 当前解析深度 */
  depth: number;
  /** 调整深度：以新深度重新发起一次解析 */
  setDepth: (depth: number) => void;
  /** 显式刷新：重新发起一次完整解析 */
  refresh: () => void;
}

/**
 * 代码统计取数收口 hook：显式刷新模型——进入页面 / 调深度 / 点刷新各自以
 * `[root, depth, tick]` 依赖发起一次 invoke（commands.codeStats typed 调用），
 * 无轮询、无 watch、无缓存。`cancelled` 防串轮；报告带归属 root 标记，切换
 * 工作区的过渡轮不呈现旧根数据（use-explore-list 抑制哲学）。组件不直接 invoke。
 */
export function useCodeStats(root: string): CodeStatsState {
  const [report, setReport] = useState<CodeStatsReport | null>(null);
  const [reportRoot, setReportRoot] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [depth, setDepthState] = useState(DEFAULT_DEPTH);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const setDepth = useCallback((next: number) => setDepthState(next), []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .codeStats(root, depth)
      .then((result) => {
        if (cancelled) return;
        setReport(result);
        setReportRoot(root);
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
  }, [root, depth, tick]);

  return {
    report: reportRoot === root ? report : null,
    loading,
    error,
    depth,
    setDepth,
    refresh,
  };
}
