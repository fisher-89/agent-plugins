/**
 * 抽屉取数 hooks：元素详情（`archi_module`，`fqn` 变化即重取）与文档正文
 * （`archi_read_doc`，展开才发 invoke）。两者都走显式取数（零 watch 零轮询）；
 * 正文取数只接受关联集内路径——越界由服务端 `Err`，本层只呈现错误串。
 */
import { useCallback, useEffect, useState } from 'react';

import { commands, type ArchiModuleDetail } from '../../../types/generated/bindings';

export interface ArchiModuleState {
  /** 元素详情（无根 / 无选中为 null） */
  detail: ArchiModuleDetail | null;
  /** 取数进行中 */
  loading: boolean;
  /** 取数失败（命令 reject） */
  error: string | null;
}

export function useArchiModule(root: string | null, fqn: string | null): ArchiModuleState {
  const [detail, setDetail] = useState<ArchiModuleDetail | null>(null);
  const [loadedKey, setLoadedKey] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (root === null || fqn === null) {
      setDetail(null);
      setLoadedKey(null);
      setLoading(false);
      setError(null);
      return;
    }
    const key = `${root}\u0000${fqn}`;
    let cancelled = false;
    setLoading(true);
    setError(null);
    commands
      .archiModule(root, fqn)
      .then((result) => {
        if (cancelled) return;
        setDetail(result);
        setLoadedKey(key);
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setError(String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, fqn]);

  const currentKey = root === null || fqn === null ? null : `${root}\u0000${fqn}`;
  return { detail: loadedKey === currentKey ? detail : null, loading, error };
}

export interface ArchiDocState {
  /** 正文（未展开 / 已复位为 null） */
  body: string | null;
  /** 取数进行中 */
  loading: boolean;
  /** 取数失败（越界 / 读盘失败，行内呈现） */
  error: string | null;
  /** 展开时显式取正文 */
  load: (path: string) => void;
  /** 收起 / 切换条目时清态 */
  reset: () => void;
}

export function useArchiDoc(root: string | null): ArchiDocState {
  const [body, setBody] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reset = useCallback(() => {
    setBody(null);
    setLoading(false);
    setError(null);
  }, []);

  const load = useCallback(
    (path: string) => {
      if (root === null) {
        setError('无工作根，无法读取文档');
        return;
      }
      setBody(null);
      setLoading(true);
      setError(null);
      commands
        .archiReadDoc(root, path)
        .then((text) => {
          setBody(text);
          setLoading(false);
        })
        .catch((reason: unknown) => {
          setError(String(reason));
          setLoading(false);
        });
    },
    [root],
  );

  return { body, loading, error, load, reset };
}
