/**
 * run 控制 hook（unify-run-state-persistence 两钩并一后的缩位形态）：职责 =
 * 控制动作（start / stop / confirm / answer，invoke 面零改动）+ 通知订阅生
 * 命周期。组件态退化为查询缓存 + 失效重取——`applyRunUpdate` 状态机镜像 /
 * `seedRunState` / `initialRunState` / `changeFlowState` 快照恢复随通知降位
 * 解散（重挂恢复归统一查询 activeRun 面，D4）。
 */
import { Channel } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';

import { commands, type RunNotice } from '../../../types/generated/bindings';

export interface UseChangeFlowRunResult {
  start: (autoNextPhase: boolean) => Promise<void>;
  stop: () => Promise<void>;
  confirm: (proceed: boolean) => Promise<void>;
  answer: (text: string) => Promise<void>;
  error: string | null;
}

/**
 * run 控制入口：`activeRunPresent`（统一查询 activeRun 在场，即非终态运行
 * 期）为真才 `changeFlowWatch` 补订实时通知（Channel<RunNotice> kind-only，
 * onmessage 分流回调——sessionEvent → 转录重查，其余 → 统一视图重查）；发起
 * 动作先行订阅（Channel 此刻构造）再 `changeFlowStart`；卸载弃投递。终态后
 * 通道自然断开（注册表除名，broadcast 发送端随条目移除失效）。
 */
export function useChangeFlowRun(params: {
  root: string | null;
  change: string | null;
  activeRunPresent: boolean;
  onNotice: (kind: RunNotice['ipc']) => void;
}): UseChangeFlowRunResult {
  const { root, change, activeRunPresent, onNotice } = params;
  const channelRef = useRef<Channel<RunNotice> | null>(null);
  const noticeRef = useRef(onNotice);
  noticeRef.current = onNotice;

  const ensureChannel = useCallback((): Channel<RunNotice> => {
    const existing = channelRef.current;
    if (existing !== null) return existing;
    const channel = new Channel<RunNotice>();
    // 分流回调单点：发起直连与重挂补订共用（sessionEvent → 转录重查，其余
    // → 统一视图重查；分流面归调用方 onNotice）
    channel.onmessage = (notice) => noticeRef.current(notice.ipc);
    channelRef.current = channel;
    return channel;
  }, []);

  // 订阅生命周期：activeRun 在场才补订（重挂恢复 = 统一查询先行、本订阅只
  // 管通知面）；卸载 / 参数变化弃投递。
  useEffect(() => {
    if (root === null || change === null || !activeRunPresent) {
      return;
    }
    const channel = ensureChannel();
    void commands.changeFlowWatch(channel, root, change).catch(() => {
      // 补订失败不阻断页面（下一通知或显式刷新兜底——R7 语义既定）
    });
    return () => {
      channelRef.current = null;
    };
  }, [root, change, activeRunPresent, ensureChannel]);

  const { start, stop, confirm, answer, error } = useRunActions(root, change, ensureChannel);
  return { start, stop, confirm, answer, error };
}

/** 错误字符串归一（invoke reject 面为 string）。 */
function readError(cause: unknown): string {
  return typeof cause === 'string' ? cause : String(cause);
}

/** 四命令动作面：参数就绪检查 + 错误统一落 error 态（发起先行清错、先行订
 * 阅再 invoke——提前 resolve 后通知即刻有落点）。 */
function useRunActions(
  root: string | null,
  change: string | null,
  ensureChannel: () => Channel<RunNotice>,
): UseChangeFlowRunResult {
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (invoke: () => Promise<unknown>): Promise<void> => {
    try {
      await invoke();
    } catch (cause) {
      setError(readError(cause));
    }
  }, []);
  const start = useCallback(
    (autoNextPhase: boolean) => {
      if (root === null || change === null) return Promise.resolve();
      setError(null);
      const channel = ensureChannel();
      return run(() => commands.changeFlowStart(channel, root, change, autoNextPhase));
    },
    [root, change, ensureChannel, run],
  );
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
