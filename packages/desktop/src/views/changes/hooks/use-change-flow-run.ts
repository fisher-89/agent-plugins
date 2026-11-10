/**
 * run 控制 hook（unify-run-state-persistence 两钩并一后的缩位形态）：职责 =
 * 控制动作（start / stop / confirm / answer，invoke 面零改动）+ 通知订阅生
 * 命周期。组件态退化为查询缓存 + 失效重取——`applyRunUpdate` 状态机镜像 /
 * `seedRunState` / `initialRunState` / `changeFlowState` 快照恢复随通知降位
 * 解散（重挂恢复归统一查询 activeRun 面，D4）。
 */
import { Channel } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../../../lib/ipc';
import { type RunNotice } from '../../../types/generated/bindings';

export interface UseChangeFlowRunResult {
  start: (autoNextPhase: boolean) => Promise<void>;
  stop: () => Promise<void>;
  /** 相位间 / 归档前停等确认：`proceed=false` → 受控终态 stopped；`syncSpecs`
   * 仅归档前停等点消费（是否执行 delta specs 同步，相位间停等点置位值不参与） */
  confirm: (proceed: boolean, syncSpecs: boolean) => Promise<void>;
  answer: (text: string) => Promise<void>;
  error: string | null;
}

/**
 * run 控制入口（参数 **id 化**——四命令 + 订阅全链按 change id 寻址）：
 * `activeRunPresent`（统一查询 activeRun 在场，即非终态运行期）为真才
 * `changeFlowWatch` 补订实时通知（Channel<RunNotice> kind-only，onmessage
 * 分流回调交调用方 `onNotice`——各 kind 同走统一视图重查，通知仅失效信号）；
 * 发起动作先行订阅（Channel 此刻构造）再 `changeFlowStart`——该 Channel 已由
 * 发起命令挂上 broadcast，补订据此去重（同一 Channel 双订阅会让每条通知投递
 * 两次）。卸载弃投递。终态后通道自然断开（注册表除名，broadcast 发送端随条目
 * 移除失效）。
 */
export function useChangeFlowRun(params: {
  root: string | null;
  id: string | null;
  activeRunPresent: boolean;
  onNotice: (kind: RunNotice['ipc']) => void;
}): UseChangeFlowRunResult {
  const { root, id, activeRunPresent, onNotice } = params;
  const { ensureChannel, claimSubscription } = useNoticeSubscription({
    root,
    id,
    activeRunPresent,
    onNotice,
  });
  const { start, stop, confirm, answer, error } = useRunActions(
    root,
    id,
    ensureChannel,
    claimSubscription,
  );
  return { start, stop, confirm, answer, error };
}

/** 订阅面共享槽（effect 与动作面同持：Channel 单实例 + 是否已订阅）。 */
interface SubscriptionSlot {
  channel: { current: Channel<RunNotice> | null };
  subscribed: { current: boolean };
}

/** 运行期补订一次（发起侧已为该 Channel 订阅时跳过）：重挂恢复 = 统一查询
 * 先行、本订阅只管通知面；补订失败不阻断页面（下一通知或显式刷新兜底）。 */
function watchOnce(slot: SubscriptionSlot, root: string, id: string): void {
  const channel = slot.channel.current;
  if (channel === null || slot.subscribed.current) return;
  void ipc.changeFlowWatch(channel, root, id).then(
    () => {
      // 清理已释放该 Channel（换 root / id / 卸载）时不再回写，防陈旧置位
      if (slot.channel.current === channel) slot.subscribed.current = true;
    },
    () => {},
  );
}

/** 发起命令的订阅占位单一实现：置位先于 invoke（发起 resolve 后统一视图重查
 * 翻真 activeRun，补订 effect 可能抢在 promise 回调前重跑——先占位才不会重复
 * 挂订阅）；失败回退到既有置位（未订阅成功，重挂补订照常走 watch）。仅当前
 * Channel 生效——在飞期间换 root / 卸载会让位给新实例，陈旧置位不得生效。 */
function claimOn(
  channelRef: { current: Channel<RunNotice> | null },
  subscribedRef: { current: boolean },
  channel: Channel<RunNotice>,
  invoke: () => Promise<unknown>,
): Promise<unknown> {
  if (channelRef.current !== channel) return invoke();
  const previous = subscribedRef.current;
  subscribedRef.current = true;
  return invoke().catch((cause: unknown) => {
    if (channelRef.current === channel) subscribedRef.current = previous;
    throw cause;
  });
}

/** 订阅生命周期面：Channel 惰性构造（单实例）、发起侧占位与失败回退、运行
 * 期补订、卸载 / 参数变化弃投递。 */
function useNoticeSubscription(params: {
  root: string | null;
  id: string | null;
  activeRunPresent: boolean;
  onNotice: (kind: RunNotice['ipc']) => void;
}): {
  ensureChannel: () => Channel<RunNotice>;
  claimSubscription: (
    channel: Channel<RunNotice>,
    invoke: () => Promise<unknown>,
  ) => Promise<unknown>;
} {
  const { root, id, activeRunPresent, onNotice } = params;
  const channelRef = useRef<Channel<RunNotice> | null>(null);
  /** 当前 Channel 是否已持有后端 broadcast 订阅（发起命令自带订阅；补订成功
   * 后置位）。同一 Channel 挂两条订阅会让每条通知投递两次。 */
  const subscribedRef = useRef(false);
  const noticeRef = useRef(onNotice);
  noticeRef.current = onNotice;

  const ensureChannel = useCallback((): Channel<RunNotice> => {
    const existing = channelRef.current;
    if (existing !== null) return existing;
    const channel = new Channel<RunNotice>();
    // 通知回调单点：发起直连与重挂补订共用（kind 解释归调用方 onNotice）
    channel.onmessage = (notice) => noticeRef.current(notice.ipc);
    channelRef.current = channel;
    subscribedRef.current = false;
    return channel;
  }, []);

  useEffect(() => {
    if (root === null || id === null || !activeRunPresent) {
      return;
    }
    const slot: SubscriptionSlot = { channel: channelRef, subscribed: subscribedRef };
    ensureChannel();
    watchOnce(slot, root, id);
    return () => {
      channelRef.current = null;
      subscribedRef.current = false;
    };
  }, [root, id, activeRunPresent, ensureChannel]);

  const claimSubscription = useCallback(
    (channel: Channel<RunNotice>, invoke: () => Promise<unknown>) =>
      claimOn(channelRef, subscribedRef, channel, invoke),
    [],
  );

  return { ensureChannel, claimSubscription };
}

/** 错误字符串归一（invoke reject 面为 string）。 */
function readError(cause: unknown): string {
  return typeof cause === 'string' ? cause : String(cause);
}

/** 四命令动作面：参数就绪检查 + 错误统一落 error 态（发起先行清错、先行订
 * 阅再 invoke——提前 resolve 后通知即刻有落点）。`claimSubscription` 承载
 * 「发起命令自带订阅」的占位与失败回退。 */
function useRunActions(
  root: string | null,
  id: string | null,
  ensureChannel: () => Channel<RunNotice>,
  claimSubscription: (
    channel: Channel<RunNotice>,
    invoke: () => Promise<unknown>,
  ) => Promise<unknown>,
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
      if (root === null || id === null) return Promise.resolve();
      setError(null);
      const channel = ensureChannel();
      return run(() =>
        claimSubscription(channel, () => ipc.changeFlowStart(channel, root, id, autoNextPhase)),
      );
    },
    [root, id, ensureChannel, claimSubscription, run],
  );
  const stop = useCallback(() => {
    if (root === null || id === null) return Promise.resolve();
    return run(() => ipc.changeFlowStop(root, id));
  }, [root, id, run]);
  const confirm = useCallback(
    (proceed: boolean, syncSpecs: boolean) => {
      if (root === null || id === null) return Promise.resolve();
      return run(() => ipc.changeFlowConfirm(root, id, proceed, syncSpecs));
    },
    [root, id, run],
  );
  const answer = useCallback(
    (text: string) => {
      if (root === null || id === null) return Promise.resolve();
      return run(() => ipc.changeFlowAnswer(root, id, text));
    },
    [root, id, run],
  );
  return { start, stop, confirm, answer, error };
}
