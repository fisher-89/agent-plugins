/**
 * 关系详情按需取数 hook（`archi_relation`）：选中边变化即取、同一元素对零重复
 * invoke、失败保留 `error` 供重试。
 *
 * 取数纪律：本 hook 是画布「三触发」纪律的**显式开窗**——触发面恰为「选中边变化
 * （且该元素对有 `apis`）」与「用户点重试」，由显式用户动作发起；MUST NOT 引入
 * watch、定时轮询或选中态保持期间的自动重取（零 `setInterval`、零订阅）。
 *
 * `edge === null`（无选中 / 该边 `apis` 为空由调用方传 `null`）即零 invoke——没有
 * 可比对的声明清单，取数只是空转。
 */
import { useCallback, useEffect, useRef, useState } from 'react';

import { commands, type ArchiRelationDetail } from '../../../types/generated/bindings';

/** 选中边的元素对身份（与 `ArchiRelation` / 报告聚合同一身份）。 */
export interface RelationEdge {
  from: string;
  to: string;
}

export interface ArchiRelationState {
  /** 关系详情（无根 / 无选中 / 本元素对尚未回数时为 null） */
  detail: ArchiRelationDetail | null;
  /** 取数进行中 */
  loading: boolean;
  /** 取数失败（命令 reject；只落行内错误，不影响图与元素面板） */
  error: string | null;
  /** 强制重取（删缓存键后重新 invoke） */
  retry: () => void;
}

/** 已载入的结果（`key` 随元素对走：换边即不认上一轮结果）。 */
interface LoadedRelation {
  key: string;
  detail: ArchiRelationDetail;
}

/** 缓存键（`\0` 分隔：路径与 FQN 里都不会出现该字符）。 */
function cacheKey(root: string, from: string, to: string): string {
  return `${root}\u0000${from}\u0000${to}`;
}

export function useArchiRelation(
  root: string | null,
  edge: RelationEdge | null,
): ArchiRelationState {
  // 引用缓存挂在本 hook 上（`CanvasBody` 不随面板两态切换卸载）：同一元素对重复
  // 选中零重复 invoke
  const cache = useRef(new Map<string, ArchiRelationDetail>());
  const [loaded, setLoaded] = useState<LoadedRelation | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  // 依赖拆成标量：同一元素对即同一取数（对象引用变化不触发重取）
  const from = edge?.from ?? null;
  const to = edge?.to ?? null;
  const key = root === null || from === null || to === null ? null : cacheKey(root, from, to);

  const retry = useCallback(() => {
    if (key !== null) cache.current.delete(key);
    setTick((value) => value + 1);
  }, [key]);

  useEffect(() => {
    setError(null);
    setLoaded(null);
    setLoading(key !== null);
    if (key === null || root === null || from === null || to === null) return;
    const cached = cache.current.get(key);
    if (cached !== undefined) {
      setLoaded({ key, detail: cached });
      setLoading(false);
      return;
    }
    let cancelled = false;
    commands
      .archiRelation(root, from, to)
      .then((result) => {
        if (cancelled) return;
        cache.current.set(key, result);
        setLoaded({ key, detail: result });
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        // 失败不写缓存：重试要真的再取一次
        setError(String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // `key` 已含 root / from / to，但显式列出可读性更好（依赖恒等式不变）
  }, [root, from, to, key, tick]);

  const detail = loaded !== null && loaded.key === key ? loaded.detail : null;
  return { detail, loading, error, retry };
}
