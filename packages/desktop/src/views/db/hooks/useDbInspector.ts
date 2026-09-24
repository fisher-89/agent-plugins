import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import type { ModelInfo, RecordEnvelope } from '../../../types/dto';

/** 记录分页页长 */
const PAGE_SIZE = 50;

export interface DbInspectorState {
  /** 模型清单（挂载取一次；计数 0 也列出） */
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
  /** 是否可能有下一页（以「本页记录数 === PAGE_SIZE」判定） */
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

/** 模型清单取数：挂载 invoke("db_models") 一次（进入页面即用户显式动作） */
function useDbModels(): { models: ModelInfo[]; loading: boolean; error: string | null } {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<ModelInfo[]>('db_models')
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
  }, []);

  return { models, loading, error };
}

/** 记录分页取数：选中模型 / 翻页 / 刷新触发；未选中模型不取数 */
function useDbRecords(
  selected: string | null,
  offset: number,
  tick: number,
): { records: RecordEnvelope[]; recordsLoading: boolean; recordsError: string | null } {
  const [records, setRecords] = useState<RecordEnvelope[]>([]);
  const [recordsLoading, setRecordsLoading] = useState(false);
  const [recordsError, setRecordsError] = useState<string | null>(null);

  useEffect(() => {
    if (selected === null) {
      setRecords([]);
      setRecordsError(null);
      setRecordsLoading(false);
      return;
    }
    let cancelled = false;
    setRecordsLoading(true);
    setRecordsError(null);
    invoke<RecordEnvelope[]>('db_records', { model: selected, offset, limit: PAGE_SIZE })
      .then((result) => {
        if (cancelled) return;
        setRecords(result);
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
  }, [selected, offset, tick]);

  return { records, recordsLoading, recordsError };
}

/**
 * DB 查看器取数收口 hook：模型清单挂载取一次；记录分页由用户显式动作触发
 * （选中模型 / 翻页 / 刷新），invoke "db_records"。错误呈现沿查询轨语义：
 * 失败置 error 态 inline 持久（不走 toast）。无轮询、无事件订阅。
 */
export function useDbInspector(): DbInspectorState {
  const { models, loading, error } = useDbModels();
  const [selected, setSelected] = useState<string | null>(null);
  const [offset, setOffset] = useState(0);
  const [tick, setTick] = useState(0);
  const { records, recordsLoading, recordsError } = useDbRecords(selected, offset, tick);

  const refresh = useCallback(() => setTick((t) => t + 1), []);
  const selectModel = useCallback((name: string) => {
    setSelected(name);
    setOffset(0);
  }, []);
  const nextPage = useCallback(() => setOffset((o) => o + PAGE_SIZE), []);
  const prevPage = useCallback(() => setOffset((o) => Math.max(0, o - PAGE_SIZE)), []);

  return {
    models,
    loading,
    error,
    selected,
    records,
    recordsLoading,
    recordsError,
    offset,
    hasMore: records.length === PAGE_SIZE,
    refresh,
    selectModel,
    nextPage,
    prevPage,
  };
}
