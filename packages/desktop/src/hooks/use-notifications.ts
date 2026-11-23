import { Channel } from '@tauri-apps/api/core';
import { type Dispatch, type SetStateAction, useCallback, useEffect, useState } from 'react';

import { ipc } from '../lib/ipc';
import { present } from '../lib/notify';
import type { Notification } from '../types/generated/bindings';

/**
 * 收件箱状态面（订阅 + 未读 + 首屏 + 标记已读，单点）。本文件是
 * `Channel<Notification>` 构造与 `notification_subscribe` 调用的**唯一**位置：
 * 这是第三类执行流通道（沿 agent 执行流与 change run 通知的先例）。
 *
 * 通道纪律：MUST NOT 并入 `use-file-watch-signal.ts`——该件的划界原则按**驱动
 * 源**切分（文件失效信号 ≠ 后端事件）；MUST NOT 引入定时轮询（无 `setInterval`
 * / 轮询型 `setTimeout`）。
 *
 * 数据流：挂载订阅一次并取初值未读（订阅频率与 `root` 无关——收件箱是 user
 * 维度）；Channel 消息按 level 呈现（`Info` 不弹 toast，机械规则）、前插列表、
 * 未读 +1；抽屉打开时 `load()` 拉首屏（权威补拉，广播滞后丢的增量由此收敛）；
 * 标记已读以命令返回的剩余未读数回写徽标（MUST NOT 由前端自行推算）。
 *
 * 失败面分流（沿错误呈现双轨）：订阅失败**静默**（不阻断壳、不白屏、不弹错误
 * toast，仅失实时增量）；列表与标记失败置 `error`（inline 持久、抽屉不关）。
 *
 * 已读灰态是**会话本地态**（`readIds` / `allRead`）：`read_at` 不出线，重启后
 * 列表项灰态不恢复——登记的已知显示边界，MUST NOT 为它把读状态塞进载荷。
 */

/** 抽屉首屏页宽（UI 首屏口径，与后端 IPC 滥用兜底上限脱钩） */
const PAGE_SIZE = 50;

export interface NotificationInbox {
  /** 未读总数（徽标唯一来源：订阅初值 / 每条 +1 / `mark_*` 返回值覆盖） */
  unread: number;
  /** 抽屉列表条目（只含 `kind = Event`，后端已过滤，前端零二次过滤） */
  items: Notification[];
  /** `kind = Event` 全量计数（分页边界与行集同一读快照） */
  total: number;
  loading: boolean;
  /** 列表 / 标记失败（inline 持久）；订阅失败不进本字段 */
  error: string | null;
  /** 本会话已读 id（灰态的一半） */
  readIds: ReadonlySet<string>;
  /** 本会话「全部已读」标记（灰态的另一半：覆盖未加载页） */
  allRead: boolean;
  /** 拉首屏（抽屉打开时调用） */
  load: () => void;
  /** 逐条置已读（徽标按命令返回值回写） */
  markRead: (ids: string[]) => Promise<void>;
  /** 全部已读（含未加载页） */
  markAllRead: () => Promise<void>;
}

/** 收件箱在途状态（一次 `useState` 收口：动作与订阅共改同一份快照） */
type InboxState = Omit<NotificationInbox, 'load' | 'markRead' | 'markAllRead'>;

const EMPTY_INBOX: InboxState = {
  unread: 0,
  items: [],
  total: 0,
  loading: false,
  error: null,
  readIds: new Set(),
  allRead: false,
};

/**
 * 订阅装配：`Channel` 单实例 + 一次订阅 + 初值未读回调。订阅失败静默（不置
 * error、不弹 toast、不重试）；卸载只置作废标记——`Channel` 无退订 API，卸载
 * 后发送失败会让后端转发任务自然退出。
 */
function useNotificationStream(
  onEvent: (notification: Notification) => void,
  onInitialUnread: (unread: number) => void,
): void {
  useEffect(() => {
    let alive = true;
    const channel = new Channel<Notification>();
    channel.onmessage = onEvent;
    ipc.notificationSubscribe(channel).then(
      (value) => {
        if (alive) onInitialUnread(typeof value === 'number' ? value : 0);
      },
      () => {
        // 订阅失败静默：壳照常渲染，仅失实时增量（权威读数仍在 load()）
      },
    );
    return () => {
      alive = false;
    };
  }, [onEvent, onInitialUnread]);
}

/** 收件箱动作面：三个动作只经 `dispatch` 改同一份快照（与状态装配解耦） */
function useInboxActions(
  setState: Dispatch<SetStateAction<InboxState>>,
): Pick<NotificationInbox, 'load' | 'markRead' | 'markAllRead'> {
  const load = useCallback(() => {
    setState((previous) => ({ ...previous, loading: true, error: null }));
    ipc.notificationList(0, PAGE_SIZE).then(
      (page) => {
        setState((previous) => ({
          ...previous,
          items: page.items,
          total: page.total,
          loading: false,
        }));
      },
      (reason: unknown) => {
        setState((previous) => ({ ...previous, error: String(reason), loading: false }));
      },
    );
  }, [setState]);

  const markRead = useCallback(
    async (ids: string[]): Promise<void> => {
      setState((previous) => ({ ...previous, readIds: new Set([...previous.readIds, ...ids]) }));
      try {
        const remaining = await ipc.notificationMarkRead(ids);
        setState((previous) => ({ ...previous, unread: remaining }));
      } catch (reason: unknown) {
        setState((previous) => ({ ...previous, error: String(reason) }));
      }
    },
    [setState],
  );

  const markAllRead = useCallback(async (): Promise<void> => {
    try {
      const remaining = await ipc.notificationMarkAllRead();
      setState((previous) => ({ ...previous, unread: remaining, allRead: true }));
    } catch (reason: unknown) {
      setState((previous) => ({ ...previous, error: String(reason) }));
    }
  }, [setState]);

  return { load, markRead, markAllRead };
}

/** 订阅与收件箱状态装配 */
export function useNotifications(): NotificationInbox {
  const [state, setState] = useState<InboxState>(EMPTY_INBOX);

  const applyIncoming = useCallback((notification: Notification) => {
    if (notification.level !== 'info') {
      present(notification.level, notification.message, notification.body ?? undefined);
    }
    setState((previous) => ({
      ...previous,
      items: [notification, ...previous.items],
      unread: previous.unread + 1,
      total: previous.total + 1,
    }));
  }, []);

  const applyInitialUnread = useCallback((unread: number) => {
    setState((previous) => ({ ...previous, unread }));
  }, []);

  useNotificationStream(applyIncoming, applyInitialUnread);

  const actions = useInboxActions(setState);

  return { ...state, ...actions };
}
