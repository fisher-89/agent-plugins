/**
 * run 控制 hook：invoke 六命令 + Channel 订阅生命周期（发起 / 重挂订阅、
 * 收口释放）+ `change_flow_state` 快照恢复。状态归并经 run-state.ts 纯
 * reducer（本 hook 零归并逻辑，只管 invoke 与订阅生命周期）。Channel 惰性
 * 构造（确有订阅需求才建——无运行 run 的常态路径零 Channel）。
 */
import { Channel } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';

import { commands, type RunUpdate } from '../../../types/generated/bindings';
import { applyRunUpdate, initialRunState, type ChangeFlowRunState } from '../flow/run-state';

export interface UseChangeFlowRunResult {
  state: ChangeFlowRunState | null;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  confirm: (proceed: boolean) => Promise<void>;
  answer: (text: string) => Promise<void>;
  error: string | null;
}

/**
 * run 控制入口：`root` / `change` 就绪时先查快照（重挂恢复运行中 run 的
 * 状态机镜像 + 补订实时流）；发起后订阅至终态收口（图回落由页面在终态时
 * 触发显式 refresh 承接）。
 */
export function useChangeFlowRun(params: {
  root: string | null;
  change: string | null;
}): UseChangeFlowRunResult {
  const { root, change } = params;
  const [state, setState] = useState<ChangeFlowRunState | null>(null);
  const channelRef = useRef<Channel<RunUpdate> | null>(null);
  const { start, stop, confirm, answer, error } = useRunActions(root, change, channelRef, setState);
  useRunRecovery(root, change, channelRef, setState);
  return { state, start, stop, confirm, answer, error };
}

/** 错误字符串归一（invoke reject 面为 string）。 */
function readError(cause: unknown): string {
  return typeof cause === 'string' ? cause : String(cause);
}

/** 订阅 Channel 惰性构造（挂 onmessage 转发；复用既有 channel 不重挂）。 */
function ensureChannel(
  channelRef: React.RefObject<Channel<RunUpdate> | null>,
  onUpdate: (update: RunUpdate) => void,
): Channel<RunUpdate> {
  const existing = channelRef.current;
  if (existing !== null) return existing;
  const channel = new Channel<RunUpdate>();
  channel.onmessage = onUpdate;
  channelRef.current = channel;
  return channel;
}

/** 六命令动作面：参数就绪检查 + 错误统一落 error 态（发起先行清错）。 */
function useRunActions(
  root: string | null,
  change: string | null,
  channelRef: React.RefObject<Channel<RunUpdate> | null>,
  setState: React.Dispatch<React.SetStateAction<ChangeFlowRunState | null>>,
): {
  start: () => Promise<void>;
  stop: () => Promise<void>;
  confirm: (proceed: boolean) => Promise<void>;
  answer: (text: string) => Promise<void>;
  error: string | null;
} {
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (invoke: () => Promise<unknown>): Promise<void> => {
    try {
      await invoke();
    } catch (cause) {
      setError(readError(cause));
    }
  }, []);
  const start = useCallback(() => {
    if (root === null || change === null) return Promise.resolve();
    setError(null);
    const channel = ensureChannel(channelRef, (update) => {
      setState((current) => applyRunUpdate(current, update));
    });
    return run(() => commands.changeFlowStart(channel, root, change));
  }, [root, change, channelRef, run, setState]);
  const stop = useCallback(() => {
    if (root === null || change === null) return Promise.resolve();
    return run(() => commands.changeFlowStop(root, change));
  }, [root, change, run]);
  const confirm = useCallback(
    (proceed: boolean) => {
      if (root === null || change === null) return Promise.resolve();
      return run(() => commands.changeFlowConfirm(root, change, proceed));
    },
    [root, change, run],
  );
  const answer = useCallback(
    (text: string) => {
      if (root === null || change === null) return Promise.resolve();
      return run(() => commands.changeFlowAnswer(root, change, text));
    },
    [root, change, run],
  );
  return { start, stop, confirm, answer, error };
}

/** 重挂恢复：快照初值 + 运行中 run 的 broadcast 补订（组件卸载弃投递）。 */
function useRunRecovery(
  root: string | null,
  change: string | null,
  channelRef: React.RefObject<Channel<RunUpdate> | null>,
  setState: React.Dispatch<React.SetStateAction<ChangeFlowRunState | null>>,
): void {
  useEffect(() => {
    if (root === null || change === null) {
      setState(null);
      return;
    }
    let disposed = false;
    // Promise.resolve 包一道：invoke 桩返回非 promise（缺省 mock）时同样走降级
    void Promise.resolve(commands.changeFlowState(root, change))
      .then((raw) => {
        if (disposed) return;
        const snapshot = raw === null || raw === undefined ? null : raw;
        setState(initialRunState(snapshot));
        // 仅运行中 run 需要实时流：Channel 此刻才构造（终局 / 无 run 零订阅）
        if (snapshot !== null && !isTerminalStatus(snapshot.status)) {
          const channel = ensureChannel(channelRef, (update) => {
            setState((current) => applyRunUpdate(current, update));
          });
          return commands.changeFlowWatch(channel, root, change);
        }
        return undefined;
      })
      .catch(() => {
        // 快照查询失败不阻断页面（空态降级）；前置错误在发起时由命令面重报
        if (!disposed) setState(null);
      });
    return () => {
      disposed = true;
      channelRef.current = null;
    };
  }, [root, change, channelRef, setState]);
}

function isTerminalStatus(status: ChangeFlowRunState['status']): boolean {
  return status === 'completed' || status === 'stopped' || status === 'failed';
}
