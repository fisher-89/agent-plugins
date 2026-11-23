import { toast } from 'sonner';

import type { NotificationLevel } from '../types/generated/bindings';
import { ipc } from './ipc';

/**
 * 前端通知门面：本文件是 sonner 的**唯一**生产持有者（另一处是
 * `components/ui/sonner.tsx` 的 `<Toaster />` 渲染件）。视图与 hooks MUST NOT
 * 直引 sonner——两条写入点共用本门面：
 *
 * - `present`：只渲染。供后端事件轨（`Channel<Notification>` 投递的通知**已由
 *   后端落库**，此时只差呈现，MUST NOT 为呈现再写一条 `Feedback`）；
 * - `notify`：渲染 + fire-and-forget 落库（`notification_record`），供前端动作
 *   反馈轨。
 *
 * 四档调用形态与收口前的直调逐字同形（`info` → 裸 `toast(message)`），`body`
 * 在场才补第二实参 `{ description }`；**不向 sonner 传 `id`**——前端反馈的 id
 * 天生唯一（`crypto.randomUUID()`），不存在「同 id 更新」的需求，传 id 只会让
 * 既有精确参数断言同时失效。
 *
 * 落库是旁路：失败（reject / `ipc` 参数校验同步抛）就地吞掉——不抛、不影响已
 * 发生的 toast、不重试、不为失败再发一条通知；DEV 构建留一行 `console.warn`
 *（沿 `lib/ipc-drift.ts` 的 soft-report 形状），prod 零留痕。
 */

/** 第二实参形态：`body` 在场才是一个 `ExternalToast`，缺席即零实参（调用形态逐字保真） */
type DetailArg = [] | [{ description: string }];

/** 四档呈现（不落库）：`level` 词汇取自生成的 IPC 类型，本文件不另立字符串集 */
export function present(level: NotificationLevel, message: string, body?: string): void {
  const detail: DetailArg = body === undefined ? [] : [{ description: body }];
  switch (level) {
    case 'info':
      toast(message, ...detail);
      break;
    case 'success':
      toast.success(message, ...detail);
      break;
    case 'warning':
      toast.warning(message, ...detail);
      break;
    case 'error':
      toast.error(message, ...detail);
      break;
  }
}

/** 渲染 + fire-and-forget 落库（前端动作反馈轨） */
export function notify(level: NotificationLevel, message: string, body?: string): void {
  present(level, message, body);
  record(level, message, body);
}

/** 反馈落库（主键由前端铸造；`body` 缺席出线为 `null`） */
function record(level: NotificationLevel, message: string, body?: string): void {
  try {
    void ipc
      .notificationRecord({ id: crypto.randomUUID(), level, message, body: body ?? null })
      .catch(recordFailed);
  } catch (cause: unknown) {
    recordFailed(cause);
  }
}

/** 落库失败留痕（旁路静默；DEV 恰一行，prod 零输出） */
function recordFailed(cause: unknown): void {
  if (import.meta.env.DEV !== true) return;
  console.warn('[notify] 落库失败（旁路，静默）', cause);
}
