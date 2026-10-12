import { useEffect, useState } from 'react';

import { commands } from '../../../types/generated/bindings';

export interface UseExecOutputResult {
  /** 脚本真实输出（UTF-8 lossy）；null = 未命中 / 未取数（空态语义） */
  text: string | null;
  loading: boolean;
  /** 取数失败（invoke 拒绝）串；miss（返回 null）非错误 */
  error: string | null;
}

/**
 * exec 输出记录取数（stdout 面板数据源）：以 `(root, changeName, execId)` 调
 * `commands.readExecOutput`（磁盘面按 change **name** 定位，与 `record_exec_output`
 * 同口径；id 仅作寻址键）。任一入参 null → 清空态零 invoke；返回 null（miss：
 * 未知 / 非法 exec_id）→ `text === null` 空态；invoke 拒绝 → `error` 错误态（不炸
 * 图）；`execId` 切走后旧响应不回写（disposed 竞态守卫）。
 */
export function useExecOutput(params: {
  root: string | null;
  changeName: string | null;
  execId: string | null;
}): UseExecOutputResult {
  const { root, changeName, execId } = params;
  const [text, setText] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // 任一入参缺席：清空态不发起任何查询（不虚构取数面）
    if (root === null || changeName === null || execId === null) {
      setText(null);
      setLoading(false);
      setError(null);
      return;
    }
    let disposed = false;
    setLoading(true);
    setError(null);
    commands
      .readExecOutput(root, changeName, execId)
      .then((value) => {
        if (disposed) return;
        setText(value ?? null);
        setLoading(false);
      })
      .catch((cause: unknown) => {
        if (disposed) return;
        setText(null);
        setError(typeof cause === 'string' ? cause : String(cause));
        setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [root, changeName, execId]);

  return { text, loading, error };
}
