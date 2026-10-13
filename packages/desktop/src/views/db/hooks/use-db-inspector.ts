import { useCallback, useEffect, useRef, useState } from 'react';

import {
  commands,
  type DbDimension,
  type ModelInfo,
  type RecordEnvelope,
} from '../../../types/generated/bindings';

/** 记录分页页长 */
const PAGE_SIZE = 50;

export interface DbInspectorState {
  /** 当前 scope（全局库 / workspace 库；默认 workspace 库——调试主看 workspace 域模型） */
  scope: DbDimension;
  /** 切换 scope（用户显式动作）：重置选中模型与分页并重取清单 */
  setScope: (scope: DbDimension) => void;
  /** 模型清单（scope / root 变更即重取；计数 0 也列出） */
  models: ModelInfo[];
  /** 模型清单加载中 */
  loading: boolean;
  /** 模型清单加载失败（查询轨 error 态，inline 持久） */
  error: string | null;
  /** 当前选中模型名；null 即未选中 */
  selected: string | null;
  /** 当前页记录信封 */
  records: RecordEnvelope[];
  /** 记录扫描加载中 */
  recordsLoading: boolean;
  /** 记录扫描失败（查询轨 error 态，inline 持久） */
  recordsError: string | null;
  /** 当前页起始偏移 */
  offset: number;
  /** 该模型全量计数（接口信封 `total` 快照；未选中 / 无数据为 0） */
  total: number;
  /** 当前页码（1 起，`offset / PAGE_SIZE + 1` 导出） */
  page: number;
  /** 总页数（`max(1, ceil(total / PAGE_SIZE))` 导出） */
  totalPages: number;
  /** 是否可能有下一页（由接口 `total` 导出：`offset + records.length < total`） */
  hasMore: boolean;
  /** 重取当前页 */
  refresh: () => void;
  /** 选中模型：置选中并回到第 0 页 */
  selectModel: (name: string) => void;
  /** 下一页 */
  nextPage: () => void;
  /** 上一页（第 0 页时不动） */
  prevPage: () => void;
}

/** 模型清单取数：scope / root 挂载与变更即 invoke("db_models")（进入页面 /
 * 切 scope 均用户显式动作；scope 寻址两库，Global 忽略 root） */
function useDbModels(
  scope: DbDimension,
  root: string,
): { models: ModelInfo[]; loading: boolean; error: string | null } {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .dbModels(scope, root)
      .then((result) => {
        if (cancelled) return;
        setModels(result);
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
  }, [scope, root]);

  return { models, loading, error };
}

/** 记录分页取数：选中模型 / 翻页 / 刷新 / scope 变更触发；未选中模型不取数。
 * 消费分页信封（records + total），存 `total` 快照供页界导出 */
function useDbRecords(
  scope: DbDimension,
  root: string,
  selected: string | null,
  offset: number,
  tick: number,
): {
  records: RecordEnvelope[];
  total: number;
  recordsLoading: boolean;
  recordsError: string | null;
} {
  const [records, setRecords] = useState<RecordEnvelope[]>([]);
  const [total, setTotal] = useState(0);
  const [recordsLoading, setRecordsLoading] = useState(false);
  const [recordsError, setRecordsError] = useState<string | null>(null);

  useEffect(() => {
    if (selected === null) {
      setRecords([]);
      setTotal(0);
      setRecordsError(null);
      setRecordsLoading(false);
      return;
    }
    let cancelled = false;
    setRecordsLoading(true);
    setRecordsError(null);
    commands
      .dbRecords(scope, root, selected, offset, PAGE_SIZE)
      .then((result) => {
        if (cancelled) return;
        setRecords(result.records);
        setTotal(result.total);
        setRecordsLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setRecordsError(String(err));
        setRecordsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [scope, root, selected, offset, tick]);

  return { records, total, recordsLoading, recordsError };
}

/**
 * 数据库器取数收口 hook：scope 态（默认 workspace 库，切全局库为显式动作）；
 * 模型清单随 scope / root 变更重取，scope / root 变更同步重置选中模型与分页；
 * 记录分页由用户显式动作触发（选中模型 / 翻页 / 刷新），invoke "db_records"；
 * 页界（hasMore / 页码 / 总页数）由接口分页信封 `total` 导出，无页长启发式。
 * 错误呈现沿查询轨语义：失败置 error 态 inline 持久（不走 toast）。无轮询、
 * 无事件订阅。
 */
/** 跳过 effect 首跑：挂载不算「变更」，仅在 dep 真正变化时触发回调。
 *  用于 scope / root 复位——首跑落地会晚于用户选中动作、把刚选中的模型抹掉。 */
function useChangedEffect(deps: readonly unknown[], run: () => void): void {
  const firstRun = useRef(true);
  useEffect(() => {
    if (firstRun.current) {
      firstRun.current = false;
      return;
    }
    run();
  }, deps);
}

export function useDbInspector(root: string): DbInspectorState {
  const [scope, setScopeState] = useState<DbDimension>('workspace');
  const [selected, setSelected] = useState<string | null>(null);
  const [offset, setOffset] = useState(0);
  const [tick, setTick] = useState(0);

  // scope / root 变更即重置选中模型与分页（两库清单互不混列，选中与页码
  // 不跨库沿用），模型清单经 useDbModels 的 effect 随之重取
  useChangedEffect([scope, root], () => {
    setSelected(null);
    setOffset(0);
  });

  const { models, loading, error } = useDbModels(scope, root);
  const { records, total, recordsLoading, recordsError } = useDbRecords(
    scope,
    root,
    selected,
    offset,
    tick,
  );

  const setScope = useCallback((next: DbDimension) => setScopeState(next), []);
  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const selectModel = useCallback((name: string) => {
    setSelected(name);
    setOffset(0);
  }, []);
  const nextPage = useCallback(() => setOffset((o) => o + PAGE_SIZE), []);
  const prevPage = useCallback(() => setOffset((o) => Math.max(0, o - PAGE_SIZE)), []);

  return {
    scope,
    setScope,
    models,
    loading,
    error,
    selected,
    records,
    recordsLoading,
    recordsError,
    offset,
    total,
    page: offset / PAGE_SIZE + 1,
    totalPages: Math.max(1, Math.ceil(total / PAGE_SIZE)),
    hasMore: offset + records.length < total,
    refresh,
    selectModel,
    nextPage,
    prevPage,
  };
}
