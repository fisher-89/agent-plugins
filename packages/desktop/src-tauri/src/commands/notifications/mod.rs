//! notifications 命令轨道：通知扇出点的装配对象（[`NotificationBus`]）+ 收件箱
//! 五命令薄包装（desktop-notification-hub）。
//!
//! **写命令落本轨道**：收件箱的三条写命令与落库命令属通知域写面，MUST NOT 并入
//! 只读的 db 查看轨道（`commands/db/` 只读纪律）。
//!
//! **只碰全局库**：[`NotificationBus`] 只持全局库实例（`Arc<Store>`），收件箱是
//! user 维度——维度隔离因此是结构性的而非纪律性的（workspace 库实例根本进不来）。
//!
//! **广播语义**：`Channel<Notification>` 只承载 `kind = Event` 的实时增量；
//! `Feedback` 永不进 Hub（落库命令直写 store、不广播）。订阅端滞后丢弃即丢增量，
//! MUST NOT 让本通道成为未读的唯一真相源——抽屉打开时的 `notification_list` 是
//! 权威补拉（沿 `change_flow_watch` 的补订语义）。
//!
//! 错误约定沿全轨道模板：`Err` 由 Tauri 转为前端 reject，`.to_string()` 是唯一
//! 错误映射点；本轨道不吞失败、不改任何业务命令的签名或返回值（通知是旁路）。
//!
//! **事件轨出口适配器**：编排侧 `RunGuard` 只认 `orchestration::notify::RunNotifier`
//! （core 不认渠道），壳以 [`NotificationBridge`] 提供实现——一条通知的落库与
//! 广播都经 [`NotificationBus`] 扇出；`Finished` 的死未读清理也由 bridge 一行转发。

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast;

use ::notification::{Hub, Notification, NotificationFeedbackInput, NotificationPage, Sink};
use orchestration::notify::{NotifyContext, RunNotifier};
use store::{Store, StoreNotificationSink};

#[cfg(test)]
mod mod_test;

/// 广播通道容量（事件稀疏，溢出即丢增量——权威读数在 `notification_list`）。
const NOTIFICATION_CAPACITY: usize = 128;

/// 当前 UTC unix 毫秒（反馈落库的 `created_at` 铸造点；时钟早于 epoch 取 0，
/// 不 panic——与 `change_flow` 的发起时刻铸造同形）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// 通知扇出点：全局库 + [`Hub`] + 广播端收成**一个**装配对象（挂 Tauri State
/// 为 `Arc<NotificationBus>`）。
///
/// 收成一个对象而不是分别 manage Hub 与 Sender：订阅要 `Sender::subscribe()`，
/// 两个状态对象就会引出两条「谁在广播」的路径；而 `store()` 返回的就是全局库
/// 实例，命令面因此只能碰全局库。
pub struct NotificationBus {
    store: Arc<Store>,
    hub: Hub,
    sender: broadcast::Sender<Notification>,
}

impl NotificationBus {
    /// 装配扇出点：先落库 sink、后 in-app sink（行在 toast 之前落库）。
    /// `store` 必须是**全局库**实例。
    pub fn new(store: Arc<Store>) -> Self {
        let (sender, _) = broadcast::channel(NOTIFICATION_CAPACITY);
        let sinks: Vec<Arc<dyn Sink + Send + Sync>> = vec![
            Arc::new(StoreNotificationSink::new(Arc::clone(&store))),
            Arc::new(InAppSink {
                sender: sender.clone(),
            }),
        ];
        Self {
            store,
            hub: Hub::new(sinks),
            sender,
        }
    }

    /// 投递一条通知（顺序遍历 sinks 各一次；旁路，失败由 sink 内部降级）。
    pub fn publish(&self, notification: &Notification) {
        self.hub.publish(notification);
    }

    /// 清同 run 的死未读（`Finished` 之后调用）：失败静默一行留痕，MUST NOT 让
    /// run 的终态收敛依赖它。
    pub fn retire_run(&self, run_id: &str) {
        if let Err(error) = self.store.retire_run_notifications(run_id) {
            eprintln!("[notification] 死未读清理失败（{run_id}）：{error}");
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.sender.subscribe()
    }

    fn store(&self) -> &Store {
        self.store.as_ref()
    }
}

/// 事件轨出口适配器：core 侧 `RunGuard` 只认 [`RunNotifier`]，壳在这里给实现
/// （run 级一次构造、零状态转发）。
///
/// 删除动作只落在本文件：`retire` 转发到 [`NotificationBus::retire_run`]，bridge
/// 本身不持 store 句柄——收件箱的维度隔离由 bus 的结构保证。
pub struct NotificationBridge {
    context: NotifyContext,
    bus: Arc<NotificationBus>,
}

impl NotificationBridge {
    /// 装配出口：`context` 的五值由发起侧装配，`bus` 必须是全局库实例。
    pub fn new(context: NotifyContext, bus: Arc<NotificationBus>) -> Self {
        Self { context, bus }
    }
}

impl RunNotifier for NotificationBridge {
    fn context(&self) -> &NotifyContext {
        &self.context
    }

    fn deliver(&self, notification: &Notification) {
        self.bus.publish(notification);
    }

    fn retire(&self) {
        self.bus.retire_run(&self.context.run_id);
    }
}

/// in-app sink：投进 broadcast，由订阅转发任务回流到前端 `Channel`。
struct InAppSink {
    sender: broadcast::Sender<Notification>,
}

impl Sink for InAppSink {
    fn deliver(&self, notification: &Notification) {
        // 无订阅者（窗口未订阅 / 已卸载）静默丢弃——通知是旁路
        let _ = self.sender.send(notification.clone());
    }
}

/// broadcast → `Channel` 转发任务。`Lagged` 不 break：本通道的权威读数是抽屉
/// 打开时的列表，断流会让「铃铛从此不再响」且无自愈面（与 `change_flow` 的
/// 「滞后即断流」不同——那一侧靠统一视图重查兜底）。
fn spawn_notification_forward(
    on_event: Channel<Notification>,
    mut receiver: broadcast::Receiver<Notification>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(notification) => {
                    if on_event.send(notification).is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// 订阅后端事件通知：返回**当前未读数**作为前端徽标初值。
#[tauri::command]
#[specta::specta]
pub fn notification_subscribe(
    app: AppHandle,
    on_event: Channel<Notification>,
) -> Result<u32, String> {
    notification_subscribe_with(app, on_event)
}

/// [`notification_subscribe`] 的泛型测试缝。
///
/// **四步顺序是不动点**（「订阅建立与计数快照之间无窗口丢更新」的实现形态）：
/// ① 先取 receiver → ② 再读未读计数 → ③ 起转发任务 → ④ 返回计数。②失败即整体
/// `Err`（前端静默、徽标缺席），MUST NOT 返回半成品订阅；MUST NOT 为此加去重表
/// 或第二次计数读（多算窗口登记为已接受的边界，收敛面是列表与 `mark_*` 返回值）。
pub(crate) fn notification_subscribe_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    on_event: Channel<Notification>,
) -> Result<u32, String> {
    let bus = Arc::clone(app.state::<Arc<NotificationBus>>().inner());
    let receiver = bus.subscribe();
    let unread = bus
        .store()
        .unread_notification_count()
        .map_err(|e| e.to_string())?;
    spawn_notification_forward(on_event, receiver);
    Ok(unread)
}

/// 收件箱分页读（倒序 + 页界信封；只 `kind = Event`）。
#[tauri::command]
#[specta::specta]
pub fn notification_list(
    app: AppHandle,
    offset: u32,
    limit: u32,
) -> Result<NotificationPage, String> {
    notification_list_with(app, offset, limit)
}

/// [`notification_list`] 的泛型测试缝。
pub(crate) fn notification_list_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    offset: u32,
    limit: u32,
) -> Result<NotificationPage, String> {
    let bus = Arc::clone(app.state::<Arc<NotificationBus>>().inner());
    bus.store()
        .list_notifications(offset, limit)
        .map_err(|e| e.to_string())
}

/// 逐条置已读；返回剩余未读数（前端据此回写徽标，MUST NOT 自行推算）。
#[tauri::command]
#[specta::specta]
pub fn notification_mark_read(app: AppHandle, ids: Vec<String>) -> Result<u32, String> {
    notification_mark_read_with(app, ids)
}

/// [`notification_mark_read`] 的泛型测试缝。
pub(crate) fn notification_mark_read_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    ids: Vec<String>,
) -> Result<u32, String> {
    let bus = Arc::clone(app.state::<Arc<NotificationBus>>().inner());
    bus.store()
        .mark_notifications_read(&ids)
        .map_err(|e| e.to_string())
}

/// 全部已读（含未加载页）；返回剩余未读数（恒 0）。
#[tauri::command]
#[specta::specta]
pub fn notification_mark_all_read(app: AppHandle) -> Result<u32, String> {
    notification_mark_all_read_with(app)
}

/// [`notification_mark_all_read`] 的泛型测试缝。
pub(crate) fn notification_mark_all_read_with<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<u32, String> {
    let bus = Arc::clone(app.state::<Arc<NotificationBus>>().inner());
    bus.store()
        .mark_all_notifications_read()
        .map_err(|e| e.to_string())
}

/// 前端动作反馈落库：`kind = Feedback` 由服务端补（[`Notification::feedback`]），
/// **不广播**（本窗口的交互反馈不是跨窗口事件）。
#[tauri::command]
#[specta::specta]
pub fn notification_record(
    app: AppHandle,
    record: NotificationFeedbackInput,
) -> Result<(), String> {
    notification_record_with(app, record)
}

/// [`notification_record`] 的泛型测试缝：空白 `id` / 空白 `message` 先显式
/// `Err`（空白入参不进入库链路，与全轨道口径一致）。
pub(crate) fn notification_record_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    record: NotificationFeedbackInput,
) -> Result<(), String> {
    if record.id.trim().is_empty() {
        return Err("非法 id: 不得为空白".to_owned());
    }
    if record.message.trim().is_empty() {
        return Err("非法 message: 不得为空白".to_owned());
    }
    let bus = Arc::clone(app.state::<Arc<NotificationBus>>().inner());
    bus.store()
        .publish_notification(&Notification::feedback(record, now_millis()))
        .map_err(|e| e.to_string())
}
