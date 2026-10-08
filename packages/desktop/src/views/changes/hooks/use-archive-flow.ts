/**
 * 归档链控制 hook：invoke 五命令 + Channel 订阅生命周期（发起 / 重挂订阅）+
 * `archive_flow_state` 快照恢复（`use-change-flow-run` 同构，design D14——
 * Channel 惰性构造：无在案链的常态路径零 Channel）。状态归并经
 * archive-state.ts 纯 reducer（本 hook 零归并逻辑）；终态一次 `onFinish`
 *（详情页 refresh 回落已归档形态、按钮消失——重挂不重复触发）。
 */
import { Channel } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';

import {
  commands,
  type ArchivePreflight,
  type ArchiveUpdate,
} from '../../../types/generated/bindings';
import {
  applyArchiveUpdate,
  initialArchiveState,
  seedArchiveState,
  type ArchiveFlowState,
} from '../flow/archive-state';

export interface UseArchiveFlowResult {
  state: ArchiveFlowState | null;
  /** 归档前置读面（点击归档按钮时取数——确认对话数据面） */
  preflight: () => Promise<ArchivePreflight | null>;
  start: (syncSpecs: boolean) => Promise<void>;
  stop: () => Promise<void>;
  error: string | null;
}

/** 错误字符串归一（invoke reject 面为 string）。 */
function readError(cause: unknown): string {
  return typeof cause === 'string' ? cause : String(cause);
}

/** 订阅 Channel 惰性构造（挂 onmessage 转发；复用既有 channel 不重挂）。 */
function ensureChannel(
  channelRef: React.RefObject<Channel<ArchiveUpdate> | null>,
  onUpdate: (update: ArchiveUpdate) => void,
): Channel<ArchiveUpdate> {
  const existing = channelRef.current;
  if (existing !== null) return existing;
  const channel = new Channel<ArchiveUpdate>();
  channel.onmessage = onUpdate;
  channelRef.current = channel;
  return channel;
}

/**
 * 归档链控制入口：`root` / `change` 就绪时先查快照（重挂恢复进行中链的阶段
 * 镜像 + 补订实时流）；发起后订阅至终态收口（收口后 onFinish 一次显式
 * refresh 承接——详情页回落已归档形态）。
 */
export function useArchiveFlow(params: {
  root: string | null;
  change: string | null;
  onFinish: () => void;
}): UseArchiveFlowResult {
  const { root, change, onFinish } = params;
  const [state, setState] = useState<ArchiveFlowState | null>(null);
  const channelRef = useRef<Channel<ArchiveUpdate> | null>(null);
  const { preflight, start, stop, error } = useArchiveActions(root, change, channelRef, setState);
  useArchiveRecovery(root, change, channelRef, setState);
  useArchiveFinish(state, onFinish);
  return { state, preflight, start, stop, error };
}

/** 动作面返回型（`useArchiveActions` 专用，随 `UseArchiveFlowResult` 对齐）。 */
interface ArchiveActionsFace {
  preflight: () => Promise<ArchivePreflight | null>;
  start: (syncSpecs: boolean) => Promise<void>;
  stop: () => Promise<void>;
  error: string | null;
}

/** 动作面：参数就绪检查 + 错误统一落 error 态（发起先行清错）。 */
function useArchiveActions(
  root: string | null,
  change: string | null,
  channelRef: React.RefObject<Channel<ArchiveUpdate> | null>,
  setState: React.Dispatch<React.SetStateAction<ArchiveFlowState | null>>,
): ArchiveActionsFace {
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (invoke: () => Promise<unknown>): Promise<void> => {
    try {
      await invoke();
    } catch (cause) {
      setError(readError(cause));
    }
  }, []);
  const preflight = useCallback((): Promise<ArchivePreflight | null> => {
    if (root === null || change === null) return Promise.resolve(null);
    // Promise.resolve 包一道：invoke 桩返回非 promise（缺省 mock）时同样降级
    return Promise.resolve(commands.archiveFlowPreflight(root, change)).catch(() => null);
  }, [root, change]);
  const start = useCallback(
    (syncSpecs: boolean) => {
      if (root === null || change === null) return Promise.resolve();
      setError(null);
      const channel = ensureChannel(channelRef, (update) => {
        setState((current) => applyArchiveUpdate(current, update));
      });
      return run(async () => {
        await commands.archiveFlowStart(channel, root, change, syncSpecs);
        setState(initialArchiveState());
      });
    },
    [root, change, channelRef, run, setState],
  );
  const stop = useCallback(() => {
    if (root === null || change === null) return Promise.resolve();
    return run(() => commands.archiveFlowStop(root, change));
  }, [root, change, run]);
  return { preflight, start, stop, error };
}

/** 重挂恢复：快照初值 + 进行中链的 broadcast 补订（组件卸载弃投递）。 */
function useArchiveRecovery(
  root: string | null,
  change: string | null,
  channelRef: React.RefObject<Channel<ArchiveUpdate> | null>,
  setState: React.Dispatch<React.SetStateAction<ArchiveFlowState | null>>,
): void {
  useEffect(() => {
    if (root === null || change === null) {
      setState(null);
      return;
    }
    let disposed = false;
    // Promise.resolve 包一道：invoke 桩返回非 promise（缺省 mock）时同样走降级
    void Promise.resolve(commands.archiveFlowState(root, change))
      .then((raw) => {
        if (disposed) return;
        const snapshot = raw === null || raw === undefined ? null : raw;
        setState(seedArchiveState(snapshot));
        // 仅进行中链需要实时流：Channel 此刻才构造（终态 / 无链零订阅——
        // 快照非空即进行中，终态即除名）
        if (snapshot !== null) {
          const channel = ensureChannel(channelRef, (update) => {
            setState((current) => applyArchiveUpdate(current, update));
          });
          return commands.archiveFlowWatch(channel, root, change);
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

/** 终态一次 onFinish（重挂 / 迟滞信封不重复触发）。 */
function useArchiveFinish(state: ArchiveFlowState | null, onFinish: () => void): void {
  const finishedRef = useRef(false);
  useEffect(() => {
    const finished = state !== null && state.finished;
    if (finished && !finishedRef.current) {
      finishedRef.current = true;
      onFinish();
    }
    if (!finished) finishedRef.current = false;
  }, [state, onFinish]);
}
