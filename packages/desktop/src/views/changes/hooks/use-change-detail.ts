import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react';

import { ipc } from '../../../lib/ipc';
import {
  type ActiveRunView,
  type ArtifactDescriptor,
  type ArtifactEnvelope,
  type ChangeDetail,
} from '../../../types/generated/bindings';

/** 通知去抖常量单点（D8：客户端尾随去抖，通知侧零 coalesce）——统一视图重
 * 查 300ms，去抖窗内到达即重置计时，最后一拍必达。会话事件通知并入同一
 * tick（会话状态重评估信号），转录专用 150ms 去抖链已退役。 */
const DETAIL_REFRESH_DEBOUNCE_MS = 300;

/** 刷新失败有界重试常量单点（D2）：上限 3 次——穷尽后静默回落「靠下一条通知
 * 兜底」；退避 1s / 2s / 4s。终态竞态洞理由：walker 写序 `run_finished`（整包
 * 落库）→ `guard.finish`（发通知），Finished 是 run 最后一条通知——终态重查
 * 若失败且只靠通知恢复则永无下一条，重试必须自带。 */
const REFRESH_RETRY_LIMIT = 3;
const REFRESH_RETRY_BACKOFF_MS = [1_000, 2_000, 4_000] as const;

export interface ChangeDetailState {
  detail: ChangeDetail | null;
  /** 在飞 run 活面（统一视图 activeRun；终态即除名 → null） */
  activeRun: ActiveRunView | null;
  artifacts: ArtifactEnvelope[];
  loading: boolean;
  error: string | null;
  /** 内联失败态（detail 在手的刷新失败轮——null = 无陈旧窗口；非 null = 保
   * last-good 中，视图呈「刷新失败」横幅；首载失败落 error 整页兜底不入此态） */
  refreshFailed: string | null;
  /** 显式刷新（即时语义，不经去抖） */
  refresh: () => void;
  /** 通知触发重查（300ms 尾随去抖——变更与会话事件通知同走，仅失效 / 状态
   * 重评估信号） */
  notifyRefresh: () => void;
  /** 统一刷新键（通知到达自增；视图下传会话组件的 `refreshKey`——会话据其
   * 重评估是否挂流，不作增量数据源） */
  refreshTick: number;
}

function fallbackEnvelope(
  kind: string,
  source: string,
  title: string,
  reason: string,
): ArtifactEnvelope {
  return {
    kind,
    version: 0,
    title,
    payload: null,
    fallbackText: `${reason}（kind: ${kind} · source: ${source}）`,
  };
}

/** 单产物读取：read_artifact（按 change id 寻址）返回空或失败时降级为
 * Fallback 信封，不阻断其余产物。 */
async function readArtifactSafe(
  root: string,
  id: string,
  descriptor: ArtifactDescriptor,
): Promise<ArtifactEnvelope> {
  try {
    const envelope = await ipc.readArtifact(root, id, descriptor.kind, descriptor.source);
    if (envelope) return envelope;
    return fallbackEnvelope(descriptor.kind, descriptor.source, descriptor.title, '产物读取失败');
  } catch {
    return fallbackEnvelope(descriptor.kind, descriptor.source, descriptor.title, '产物读取失败');
  }
}

/** 尾随去抖 bump（计时器面单点）。 */
function useDebouncedBump(bump: () => void, delayMs: number): () => void {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const bumpRef = useRef(bump);
  bumpRef.current = bump;
  const notify = useCallback(() => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = setTimeout(() => bumpRef.current(), delayMs);
  }, [delayMs]);
  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );
  return notify;
}

/** 重试调度面（schedule / cancel；对象引用经 `useMemo` 稳定——effect 依赖面安全）。 */
type RefreshRetry = {
  schedule: () => void;
  cancel: () => void;
};

/** 刷新失败有界重试调度（文件私有计时面，沿 [`useDebouncedBump`] 先例——零导
 * 出）：`schedule` 按退避序列挂起一拍重试（穷尽静默——无第 4 次自动查询）；
 * `cancel` 作废挂起重试并归零退避序列（新意图取代旧重试的取消单点）。`fire`
 * 为重试出口（bumpTick——重试即自动补发的刷新意图，走同一 tick 通道 → 同一
 * 查询路径，无交错重复查询）；卸载清计时。 */
function useRefreshRetry(fire: () => void): RefreshRetry {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const attempt = useRef(0);
  const fireRef = useRef(fire);
  fireRef.current = fire;
  const schedule = useCallback(() => {
    if (attempt.current >= REFRESH_RETRY_LIMIT) return;
    const delayMs = REFRESH_RETRY_BACKOFF_MS[attempt.current];
    attempt.current += 1;
    timer.current = setTimeout(() => fireRef.current(), delayMs);
  }, []);
  const cancel = useCallback(() => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
    attempt.current = 0;
  }, []);
  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );
  return useMemo(() => ({ schedule, cancel }), [schedule, cancel]);
}

/** 统一视图装配单次拉取（详情 + 产物信封同周期组装；`isCancelled` 过期响应
 * 丢弃面——刷新竞态既有语义）。`apply` 为部分写：未携带的键不触碰既有状态
 *（详情在手时刷新不回落加载占位——既有语义）。 */
type UnifiedViewPatch = {
  detail?: ChangeDetail | null;
  activeRun?: ActiveRunView | null;
  artifacts?: ArtifactEnvelope[];
  error?: string | null;
  /** 内联失败态（失败轮置错误串；成功 / 真 None 轮清 null；轮启动不清——防提示闪烁） */
  refreshFailed?: string | null;
  /** 仅承载轮落定清面（false）——置位 true 归首载轮 / 手动刷新两个显式入口，
   * 通知 / 重试等自动轮不触碰（类型收窄：非法置位不可表达） */
  loading?: false;
};

/** 取数轮结局判别（effect 据此分派重试调度）：`ok`（Some 与真 None 均计）→
 * 归零重试序列；`stale`（失败且 detail 在手——保 last-good）→ 调度退避重试；
 * `error`（首载失败——整页兜底，不重试）/ `cancelled`（换 root / id、卸载
 * 等过期轮）→ no-op。 */
type UnifiedLoadOutcome = 'ok' | 'stale' | 'error' | 'cancelled';

async function loadUnifiedView(
  root: string,
  id: string,
  isCancelled: () => boolean,
  hasDetail: () => boolean,
  apply: (next: UnifiedViewPatch) => void,
): Promise<UnifiedLoadOutcome> {
  // 轮启动不清 refreshFailed（重查中提示保持——成功才消失，防提示闪烁）；
  // loading 置位不归轮本体——首载轮 / 手动刷新各自置位，自动轮零触碰
  apply({ error: null });
  try {
    const unified = await ipc.getChangeDetail(root, id);
    if (isCancelled()) return 'cancelled';
    if (!unified || !unified.detail) {
      // 真 None（未找到）：清面兜底（既有语义），陈旧提示随面清零
      apply({ detail: null, activeRun: null, artifacts: [], refreshFailed: null, loading: false });
      return 'ok';
    }
    // 成功轮：detail 到手即清陈旧提示（查询结果权威；迁移间更新不被吃掉）
    apply({ detail: unified.detail, activeRun: unified.activeRun, refreshFailed: null });
    const envelopes = await Promise.all(
      unified.detail.artifacts.map((descriptor) => readArtifactSafe(root, id, descriptor)),
    );
    if (isCancelled()) return 'cancelled';
    apply({ artifacts: envelopes, loading: false });
    return 'ok';
  } catch (err: unknown) {
    if (isCancelled()) return 'cancelled';
    // 失败轮分流（D5）：detail 在手 → 保 last-good + 内联失败态（不清
    // detail / activeRun / artifacts）；首载失败 → 既有整页 error 兜底
    if (hasDetail()) {
      apply({ refreshFailed: String(err), loading: false });
      return 'stale';
    }
    apply({ error: String(err), loading: false });
    return 'error';
  }
}

/** 统一视图 state setter 束（apply 装配 / 空参复位共用面——useState setter
 * 引用恒稳定，束对象在 effect 内装配）。 */
type UnifiedViewSetters = {
  setDetail: (value: ChangeDetail | null) => void;
  setActiveRun: (value: ActiveRunView | null) => void;
  setArtifacts: (value: ArtifactEnvelope[]) => void;
  setError: (value: string | null) => void;
  setRefreshFailed: (value: string | null) => void;
  setLoading: (value: boolean) => void;
};

/** 统一视图 partial patch 装配单点（未携带键不触碰既有 state——`'detail' in
 * next` 判别携带面）；`loading` 仅承载轮落定清面（false）——置位 true 由首载
 * 轮 / 手动刷新显式发起，自动轮不触碰。 */
function applyUnifiedPatch(setters: UnifiedViewSetters, next: UnifiedViewPatch): void {
  if ('detail' in next) setters.setDetail(next.detail ?? null);
  if ('activeRun' in next) setters.setActiveRun(next.activeRun ?? null);
  if (next.artifacts !== undefined) setters.setArtifacts(next.artifacts);
  if (next.error !== undefined) setters.setError(next.error);
  if (next.refreshFailed !== undefined) setters.setRefreshFailed(next.refreshFailed);
  if (next.loading === false) setters.setLoading(next.loading);
}

/** 空参复位（root / id 缺席 → 清面静默；内联失败态随面清零）。 */
function resetUnifiedView(setters: UnifiedViewSetters): void {
  setters.setDetail(null);
  setters.setActiveRun(null);
  setters.setArtifacts([]);
  setters.setError(null);
  setters.setRefreshFailed(null);
  setters.setLoading(false);
}

/** 首载键判别（`root\0id` 首轮 = 首载阻塞面）：`beginRound` 键首次出现返回
 * true（调用方据此置 loading），同键后续轮（通知重查 / 退避重试）返回 false；
 * 空键清面复键（同 id 再进入重新计首载）。文件私有、零导出——沿
 * [`useRefreshRetry`] 先例。 */
function useFirstLoadKey(): (key: string | null) => boolean {
  const keyRef = useRef<string | null>(null);
  return useCallback((key: string | null): boolean => {
    if (key === null) {
      keyRef.current = null;
      return false;
    }
    if (keyRef.current === key) return false;
    keyRef.current = key;
    return true;
  }, []);
}

/** 统一视图取数轮 effect 本体（空参复位 / 轮驱动 / 轮结局分派单点——函数体
 * 拆出保 [`useChangeDetail`] 在 `max-lines-per-function` 红线内）：`stale` →
 * 调度退避重试（陈旧窗口有界自愈）；`ok` → 归零重试序列（新失败串从 1s 重新
 * 起算）；`error` / `cancelled` → no-op（首载失败不重试）。轮本体不做 cancel
 * ——重试自身 bump 的 tick 重跑 effect 时不得重置退避序列。`beginRound` 判别
 * 该 `root` / `id` 的首轮（首载阻塞面），通知 / 重试等自动轮不置 loading。 */
function runUnifiedViewEffect(ctx: {
  root: string | null;
  id: string | null;
  retry: RefreshRetry;
  hasDetail: () => boolean;
  setters: UnifiedViewSetters;
  beginRound: (key: string | null) => boolean;
}): () => void {
  const { root, id, retry, hasDetail, setters, beginRound } = ctx;
  if (!root || !id) {
    beginRound(null);
    resetUnifiedView(setters);
    return () => {};
  }
  if (beginRound(`${root}\u0000${id}`)) setters.setLoading(true);
  let cancelled = false;
  void loadUnifiedView(
    root,
    id,
    () => cancelled,
    hasDetail,
    (next) => applyUnifiedPatch(setters, next),
  ).then((outcome) => {
    if (outcome === 'stale') retry.schedule();
    else if (outcome === 'ok') retry.cancel();
  });
  return () => {
    cancelled = true;
  };
}

/**
 * 详情取数 hook（统一视图承接，unify-run-state-persistence 两钩并一；参数
 * **id 化**——取数 / 产物逐读全链按 change id 寻址）：
 * `getChangeDetail` 一次返回「库读史 ∪ 在飞 run」统一视图；显式 refresh（或
 * 选定 change）即时触发，变更通知与会话事件通知经同一 `notifyRefresh`
 * （300ms 尾随去抖）触发重查——`SessionEvent` 仅作会话状态重评估信号，实时面
 * 走会话订阅（delta 不落库，重查只能见密封粒度）。并在同一刷新周期内按产物
 * 清单逐个 invoke("read_artifact") 组装信封数组；单个产物读取失败以 Fallback
 * 形态保留、不阻断其余。无轮询、无文件 watch。
 */
export function useChangeDetail(root: string | null, id: string | null): ChangeDetailState {
  const [detail, setDetail] = useState<ChangeDetail | null>(null);
  const [activeRun, setActiveRun] = useState<ActiveRunView | null>(null);
  const [artifacts, setArtifacts] = useState<ArtifactEnvelope[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshFailed, setRefreshFailed] = useState<string | null>(null);
  const [tick, bumpTick] = useReducer((count: number) => count + 1, 0);
  const retry = useRefreshRetry(bumpTick);
  // 首载键：仅该 `root\0id` 首轮取数置 loading（首载阻塞面）；同键的通知
  // 重查 / 退避重试为后台刷新，不触碰 loading（按钮不闪禁用、无 Spinner）
  const beginRound = useFirstLoadKey();
  // 通知触发重查：尾随去抖（窗内到达即重置计时，最后一拍必达）——统一视图
  // 300ms（D8 常量单点）；新意图取代旧重试：手动刷新与通知到达（去抖拍前）
  // 先作废挂起重试并重置退避序列。只有手动刷新置 loading——通知到达即后台
  // 重查（过程中自动刷新不阻塞加载态）
  const debouncedNotifyRefresh = useDebouncedBump(bumpTick, DETAIL_REFRESH_DEBOUNCE_MS);
  const refresh = useCallback(() => {
    setLoading(true);
    retry.cancel();
    bumpTick();
  }, [retry, bumpTick]);
  const notifyRefresh = useCallback(() => {
    retry.cancel();
    debouncedNotifyRefresh();
  }, [retry, debouncedNotifyRefresh]);
  // detail 在手镜像（effect 闭包内 state 快照过期——失败轮分流经 ref 判定）
  const detailRef = useRef(detail);
  detailRef.current = detail;
  const hasDetail = useCallback(() => detailRef.current !== null, []);
  useEffect(() => {
    const setters: UnifiedViewSetters = {
      setDetail,
      setActiveRun,
      setArtifacts,
      setError,
      setRefreshFailed,
      setLoading,
    };
    return runUnifiedViewEffect({ root, id, retry, hasDetail, setters, beginRound });
  }, [root, id, tick, retry, hasDetail, beginRound]);
  // 取消通道（D2）：换 root / id 与卸载即作废挂起重试（tick 不入依赖——重试
  // 自身 bump 的 tick 重跑不重置退避序列）
  useEffect(() => () => retry.cancel(), [retry, root, id]);
  return {
    detail,
    activeRun,
    artifacts,
    loading,
    error,
    refreshFailed,
    refresh,
    notifyRefresh,
    refreshTick: tick,
  };
}
