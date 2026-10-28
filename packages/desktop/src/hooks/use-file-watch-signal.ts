import { Channel } from '@tauri-apps/api/core';
import { useEffect, useRef } from 'react';

import { ipc } from '../lib/ipc';
import type { FileWatchEvent } from '../types/generated/bindings';

/**
 * 文件 watch 失效信号消费件（单点）。收拢五件事，且只做这五件：
 * 1) `Channel<FileWatchEvent>` 建立并挂派发；2) `resolvePath()` 取布局派生路径
 * → null 则不订阅，否则 `watch_subscribe(channel, path)`；3) trailing 防抖合并
 * （窗口见 `SIGNAL_DEBOUNCE_MS`）；4) 臂键变化 / 卸载取消待发计时并退订，订阅
 * 迟到就绪（清理已发生）立即补发退订；5) 订阅链任一环失败静默——不冒泡为消费方
 * 错误态、不阻断其取数路径（页面仍可显式刷新，仅失自动刷新）。
 *
 * 边界纪律：信号**无载荷**（`FileWatchEvent` 只作 `Channel` 泛型参数，本件 MUST
 * NOT 读其字段）；零取数（不调 `read_explore` 等数据命令，挂载 / 重臂时不替消费
 * 方发起取数）；零消费方状态（不触碰 `loading` / `error`）；零目录知识（路径由
 * 命令层给出）。`resolvePath` / `onSignal` 以最新引用生效，MUST NOT 因回调身份
 * 变化而重订订阅。
 *
 * M1 记账：重订**只**由 `armKey` 变化触发。数据面刷新维度（如显式刷新的 tick）
 * MUST NOT 进 `armKey` ⇒ 显式刷新不重订 watch（代价：「手动刷新顺带自愈死亡
 * watcher」这条不可靠旁路消失）。勿把它当 bug 修回去——把刷新并进臂键会让每次
 * 刷新多出一对 `watch_unsubscribe` / `watch_subscribe` IPC。
 *
 * 划界原则：公共件按**驱动源**切分，不按「都是防抖」切分。文件失效信号（磁盘 →
 * 需路径解析 + 订阅 + 退订）属本件；run 内存通知（`change_flow_watch` 的
 * kind-only `RunNotice`，无路径解析、无订阅、下游是整页重查）留在
 * `views/changes/hooks/use-change-detail.ts` 的 `useDebouncedBump`，MUST NOT 并入。
 *
 * 参数面恰三项，不开 `debounceMs` / `enabled` / `retry` / 错误回调（无消费者，
 * `armKey: null` 已覆盖「不订阅」，订阅失败已定「静默不阻断」）。
 */

/** trailing 防抖窗口（毫秒，单点）：消费方 MUST NOT 自建防抖窗口——不开参数、不导出 */
const SIGNAL_DEBOUNCE_MS = 500;

/** 防抖合并器（trailing）：窗口内多次信号重置计时，最后一拍必达；`cancel` 幂等 */
function createDebouncer(onDue: () => void): { push: () => void; cancel: () => void } {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return {
    push: () => {
      if (timer !== null) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        onDue();
      }, SIGNAL_DEBOUNCE_MS);
    },
    cancel: () => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    },
  };
}

/**
 * 订阅装配与退订外壳（单订阅两态）。`resolvePath()` 在 async 体首个 `await`
 * 之前被同步调用 ⇒ 读到**本臂**闭包；`path === null` 与「已作废」共一条短路
 * ⇒ `subscriptionId` 恒 null ⇒ 无订阅可退。装配链任一环 reject 由尾部 `catch`
 * 归入同一静默面（含 `resolvePath` 的同步抛出）。
 */
function armSubscription(
  channel: Channel<FileWatchEvent>,
  resolvePath: () => Promise<string | null>,
): () => void {
  let disposed = false;
  let subscriptionId: number | null = null;
  void (async () => {
    const path = await resolvePath();
    if (path === null || disposed) return;
    const id = await ipc.watchSubscribe(channel, path);
    if (disposed) {
      void ipc.watchUnsubscribe(id);
      return;
    }
    subscriptionId = id;
  })().catch(() => {
    // 订阅失败不阻断数据面：页面仍可显式刷新（仅失自动刷新），不抛、不重试、不上报
  });
  return () => {
    disposed = true;
    if (subscriptionId !== null) void ipc.watchUnsubscribe(subscriptionId);
  };
}

/**
 * 文件 watch 失效信号消费：`armKey` 非 null 才建 Channel 并订阅；信号经
 * `SIGNAL_DEBOUNCE_MS` trailing 防抖合并为一次 `onSignal`；臂键变化 / 卸载时
 * 取消待发计时并退订（迟到就绪补发退订）；订阅失败静默。
 */
export function useFileWatchSignal(params: {
  /** 订阅身份键（寻址维度的无碰撞编码）：变化即退订重建；null = 不订阅 */
  armKey: string | null;
  /** 布局派生路径（命令层给出，前端零目录知识）：null = 不订阅 */
  resolvePath: () => Promise<string | null>;
  /** 信号到达（防抖后）调用一次 */
  onSignal: () => void;
}): void {
  const { armKey, onSignal, resolvePath } = params;
  const resolvePathRef = useRef(resolvePath);
  resolvePathRef.current = resolvePath;
  const onSignalRef = useRef(onSignal);
  onSignalRef.current = onSignal;

  // 依赖面只含臂键：数据面刷新维度 MUST NOT 进臂键（M1），否则显式刷新会重订订阅
  useEffect(() => {
    if (armKey === null) return;
    const channel = new Channel<FileWatchEvent>();
    const signals = createDebouncer(() => onSignalRef.current());
    // 0 参 `push` ⇒ 载荷在读面上无处可达（信号无载荷语义）
    channel.onmessage = signals.push;
    const disposeSubscription = armSubscription(channel, () => resolvePathRef.current());
    return () => {
      signals.cancel();
      disposeSubscription();
    };
  }, [armKey]);
}
