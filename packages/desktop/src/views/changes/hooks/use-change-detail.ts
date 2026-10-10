import { useCallback, useEffect, useReducer, useRef, useState } from 'react';

import {
  commands,
  type ActiveRunView,
  type ArtifactDescriptor,
  type ArtifactEnvelope,
  type ChangeDetail,
} from '../../../types/generated/bindings';

/** 通知去抖常量单点（D8：客户端尾随去抖，通知侧零 coalesce）——统一视图重
 * 查 300ms / 转录重查 150ms；去抖窗内到达即重置计时，最后一拍必达。 */
const DETAIL_REFRESH_DEBOUNCE_MS = 300;
const TRANSCRIPT_REFRESH_DEBOUNCE_MS = 150;

export interface ChangeDetailState {
  detail: ChangeDetail | null;
  /** 在飞 run 活面（统一视图 activeRun；终态即除名 → null） */
  activeRun: ActiveRunView | null;
  artifacts: ArtifactEnvelope[];
  loading: boolean;
  error: string | null;
  /** 显式刷新（即时语义，不经去抖） */
  refresh: () => void;
  /** 通知触发重查（300ms 尾随去抖——变更通知仅失效信号） */
  notifyRefresh: () => void;
  /** 通知触发转录重查（150ms 尾随去抖——转录库唯一数据源） */
  notifyTranscript: () => void;
  /** 转录刷新键（通知到达自增；视图下传 useSessionTranscript.refreshKey） */
  transcriptTick: number;
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
    const envelope = await commands.readArtifact(root, id, descriptor.kind, descriptor.source);
    if (envelope) return envelope;
    return fallbackEnvelope(descriptor.kind, descriptor.source, descriptor.title, '产物读取失败');
  } catch {
    return fallbackEnvelope(descriptor.kind, descriptor.source, descriptor.title, '产物读取失败');
  }
}

/** 尾随去抖自增键（通知触发重查的计时面）：窗内到达即重置计时，最后一拍
 * 必达；卸载清计时器。 */
function useDebouncedTick(delayMs: number): [number, () => void] {
  const [tick, bump] = useReducer((count: number) => count + 1, 0);
  const notify = useDebouncedBump(bump, delayMs);
  return [tick, notify];
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

/** 统一视图装配单次拉取（详情 + 产物信封同周期组装；`isCancelled` 过期响应
 * 丢弃面——刷新竞态既有语义）。`apply` 为部分写：未携带的键不触碰既有状态
 *（详情在手时刷新不回落加载占位——既有语义）。 */
type UnifiedViewPatch = {
  detail?: ChangeDetail | null;
  activeRun?: ActiveRunView | null;
  artifacts?: ArtifactEnvelope[];
  error?: string | null;
  loading?: boolean;
};

async function loadUnifiedView(
  root: string,
  id: string,
  isCancelled: () => boolean,
  apply: (next: UnifiedViewPatch) => void,
): Promise<void> {
  apply({ loading: true, error: null });
  try {
    const unified = await commands.getChangeDetail(root, id);
    if (isCancelled()) return;
    if (!unified || !unified.detail) {
      apply({ detail: null, activeRun: null, artifacts: [], loading: false });
      return;
    }
    apply({ detail: unified.detail, activeRun: unified.activeRun });
    const envelopes = await Promise.all(
      unified.detail.artifacts.map((descriptor) => readArtifactSafe(root, id, descriptor)),
    );
    if (isCancelled()) return;
    apply({ artifacts: envelopes, loading: false });
  } catch (err: unknown) {
    if (isCancelled()) return;
    apply({ error: String(err), loading: false });
  }
}

/**
 * 详情取数 hook（统一视图承接，unify-run-state-persistence 两钩并一；参数
 * **id 化**——取数 / 产物逐读全链按 change id 寻址）：
 * `getChangeDetail` 一次返回「库读史 ∪ 在飞 run」统一视图；显式 refresh（或
 * 选定 change）即时触发，变更通知经 `notifyRefresh`（300ms 尾随去抖）触发
 * 重查、会话事件通知经 `notifyTranscript`（150ms）触发转录库重查。并在同一
 * 刷新周期内按产物清单逐个 invoke("read_artifact") 组装信封数组；单个产物
 * 读取失败以 Fallback 形态保留、不阻断其余。无轮询、无文件 watch。
 */
export function useChangeDetail(root: string | null, id: string | null): ChangeDetailState {
  const [detail, setDetail] = useState<ChangeDetail | null>(null);
  const [activeRun, setActiveRun] = useState<ActiveRunView | null>(null);
  const [artifacts, setArtifacts] = useState<ArtifactEnvelope[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, bumpTick] = useReducer((count: number) => count + 1, 0);
  // 通知触发重查：尾随去抖（窗内到达即重置计时，最后一拍必达）——统一视图
  // 300ms / 转录 150ms（D8 常量单点）
  const [transcriptTick, notifyTranscript] = useDebouncedTick(TRANSCRIPT_REFRESH_DEBOUNCE_MS);
  const notifyRefresh = useDebouncedBump(bumpTick, DETAIL_REFRESH_DEBOUNCE_MS);
  const refresh = useCallback(() => bumpTick(), [bumpTick]);

  useEffect(() => {
    if (!root || !id) {
      setDetail(null);
      setActiveRun(null);
      setArtifacts([]);
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    void loadUnifiedView(
      root,
      id,
      () => cancelled,
      (next) => {
        if ('detail' in next) setDetail(next.detail ?? null);
        if ('activeRun' in next) setActiveRun(next.activeRun ?? null);
        if (next.artifacts !== undefined) setArtifacts(next.artifacts);
        if (next.error !== undefined) setError(next.error);
        if (next.loading !== undefined) setLoading(next.loading);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [root, id, tick]);

  return {
    detail,
    activeRun,
    artifacts,
    loading,
    error,
    refresh,
    notifyRefresh,
    notifyTranscript,
    transcriptTick,
  };
}
