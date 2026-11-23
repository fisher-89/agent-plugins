import { cn } from 'cn';
import { BellIcon, CircleCheckIcon, InfoIcon, OctagonXIcon, TriangleAlertIcon } from 'lucide-react';
import { useState } from 'react';
import { useNavigate } from 'react-router';

import { StandardDrawer } from '@/components/standard/drawer';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { type NotificationInbox, useNotifications } from '@/hooks/use-notifications';
import type { Notification, NotificationLevel } from '@/types/generated/bindings';

/**
 * 壳态顶栏的收件箱入口：铃铛 + 未读徽标 + 抽屉列表。
 *
 * 零新增导航：不新增路由、不新增侧栏项，收件箱只在顶栏抽屉里（`routes.tsx` 与
 * `app-sidebar.tsx` 零 diff）。欢迎态（`root` 为 null）由页壳不渲染 header 保证，
 * 本件因此恒有 `root`。
 *
 * 跨工作区条目（收件箱是 user 维度）：点击**先**切根（本地 `select`，零 IPC）**再**
 * `navigate('/changes/{source_ref}')`——切根会重置当前视图，故顺序不可颠倒；
 * 同根直接跳转；`source_ref` 缺位或不可达的行点击惰性（不报错、不跳转）。
 *
 * 灰态是会话本地态（`readIds` / `allRead`）：`read_at` 不出线，重启后不恢复。
 */

/** 级别图标（四档，与 `<Toaster />` 的 icons 同族对位） */
function levelIcon(level: NotificationLevel): React.JSX.Element {
  switch (level) {
    case 'info':
      return <InfoIcon className="size-4 shrink-0" />;
    case 'success':
      return <CircleCheckIcon className="size-4 shrink-0" />;
    case 'warning':
      return <TriangleAlertIcon className="size-4 shrink-0" />;
    case 'error':
      return <OctagonXIcon className="size-4 shrink-0" />;
  }
}

/** 徽标文案：`0` 不渲染、`≤99` 呈数字、`>99` 呈 `99+` */
function badgeText(unread: number): string | null {
  if (unread === 0) return null;
  return unread > 99 ? '99+' : String(unread);
}

/** 单条通知行：级别图标 / 主文案 / 副文案 / 时间；已读置灰 */
function InboxRow({
  notification,
  read,
  onOpen,
}: {
  notification: Notification;
  read: boolean;
  onOpen: (notification: Notification) => void;
}): React.JSX.Element {
  return (
    <button
      className={cn(
        'flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-muted',
        read && 'opacity-50',
      )}
      data-state={read ? 'read' : 'unread'}
      data-testid="notification-item"
      type="button"
      onClick={() => onOpen(notification)}
    >
      {levelIcon(notification.level)}
      <span className="flex min-w-0 flex-col">
        <span className="text-sm">{notification.message}</span>
        {notification.body !== null && (
          <span className="text-xs text-muted-foreground">{notification.body}</span>
        )}
        <span className="text-xs text-muted-foreground">
          {new Date(notification.createdAt).toLocaleString()}
        </span>
      </span>
    </button>
  );
}

/** 抽屉正文：全部已读 + inline 错误 + 空态 + 列表 */
function InboxContent({
  inbox,
  onOpen,
}: {
  inbox: NotificationInbox;
  onOpen: (notification: Notification) => void;
}): React.JSX.Element {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-end">
        <Button size="sm" variant="outline" onClick={() => void inbox.markAllRead()}>
          全部已读
        </Button>
      </div>
      {inbox.error !== null && (
        <p className="text-xs text-destructive" data-testid="notification-error">
          {inbox.error}
        </p>
      )}
      {inbox.items.length === 0 ? (
        <p className="px-2 py-4 text-sm text-muted-foreground" data-testid="notification-empty">
          {inbox.loading ? '加载中…' : '暂无通知'}
        </p>
      ) : (
        <div className="flex flex-col gap-0.5" data-testid="notification-list">
          {inbox.items.map((notification) => (
            <InboxRow
              key={notification.id}
              notification={notification}
              read={inbox.allRead || inbox.readIds.has(notification.id)}
              onOpen={onOpen}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export function NotificationBell({
  root,
  onSelect,
}: {
  root: string;
  onSelect: (root: string) => void;
}): React.JSX.Element {
  const inbox = useNotifications();
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();

  const openItem = (notification: Notification): void => {
    void inbox.markRead([notification.id]);
    const sourceRef = notification.sourceRef;
    if (sourceRef === null) return;
    if (notification.root !== null && notification.root !== root) onSelect(notification.root);
    void navigate(`/changes/${sourceRef}`);
  };

  const badge = badgeText(inbox.unread);
  return (
    <StandardDrawer
      content={<InboxContent inbox={inbox} onOpen={openItem} />}
      open={open}
      title="通知"
      trigger={
        <Button aria-label="通知" className="relative" size="icon" variant="ghost">
          <BellIcon />
          {badge !== null && (
            <Badge
              className="absolute -top-1 -right-1 px-1"
              data-testid="notification-badge"
              variant="destructive"
            >
              {badge}
            </Badge>
          )}
        </Button>
      }
      onOpenChange={(next) => {
        setOpen(next);
        if (next) inbox.load();
      }}
    />
  );
}
