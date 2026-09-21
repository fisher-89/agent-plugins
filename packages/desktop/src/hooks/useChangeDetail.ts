import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import type { ArtifactDescriptor, ArtifactEnvelope, ChangeDetail } from '../types/dto';

export interface ChangeDetailState {
  detail: ChangeDetail | null;
  artifacts: ArtifactEnvelope[];
  loading: boolean;
  error: string | null;
  refresh: () => void;
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

/** 单产物读取：read_artifact 返回空或失败时降级为 Fallback 信封，不阻断其余产物。 */
async function readArtifactSafe(
  root: string,
  change: string,
  descriptor: ArtifactDescriptor,
): Promise<ArtifactEnvelope> {
  try {
    const envelope = await invoke<ArtifactEnvelope | null>('read_artifact', {
      root,
      change,
      kind: descriptor.kind,
      source: descriptor.source,
    });
    if (envelope) return envelope;
    return fallbackEnvelope(descriptor.kind, descriptor.source, descriptor.title, '产物读取失败');
  } catch {
    return fallbackEnvelope(descriptor.kind, descriptor.source, descriptor.title, '产物读取失败');
  }
}

/**
 * 详情取数 hook：显式 refresh（或选定 change）触发 invoke("get_change_detail")，
 * 并在同一刷新周期内按产物清单逐个 invoke("read_artifact") 组装信封数组；
 * 单个产物读取失败以 Fallback 形态保留、不阻断其余。无轮询、无文件 watch。
 */
export function useChangeDetail(root: string | null, change: string | null): ChangeDetailState {
  const [detail, setDetail] = useState<ChangeDetail | null>(null);
  const [artifacts, setArtifacts] = useState<ArtifactEnvelope[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (!root || !change) {
      setDetail(null);
      setArtifacts([]);
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<ChangeDetail | null>('get_change_detail', { root, change })
      .then(async (result) => {
        if (cancelled) return;
        if (!result) {
          setDetail(null);
          setArtifacts([]);
          setLoading(false);
          return;
        }
        setDetail(result);
        const envelopes = await Promise.all(
          result.artifacts.map((descriptor) => readArtifactSafe(root, change, descriptor)),
        );
        if (cancelled) return;
        setArtifacts(envelopes);
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
  }, [root, change, tick]);

  return { detail, artifacts, loading, error, refresh };
}
