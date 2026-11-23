//! 通知落库面：记录 ↔ 载荷的一比一映射单点 + 落库 sink 实现。
//!
//! 映射只在本文件——`Store` 的通知方法 MUST NOT 手抄字段（两个方向的字段面
//! 差异只有一处已知：`read_at` 只在记录侧存在）。
//!
//! sink 是旁路：落库失败 `eprintln!` 一行降级，MUST NOT 上抛、MUST NOT 让业务
//! 动作失败、MUST NOT 为失败再发一条通知（`notification` crate 的三不变量）。

use std::sync::Arc;

use ::notification::{Notification, Sink};

use crate::model::NotificationRecord;
use crate::store::Store;

impl NotificationRecord {
    /// 载荷 → 记录（新行语义：`read_at = None`）。
    pub fn from_notification(notification: &Notification) -> Self {
        Self {
            id: notification.id.clone(),
            kind: notification.kind,
            level: notification.level,
            message: notification.message.clone(),
            body: notification.body.clone(),
            root: notification.root.clone(),
            source_ref: notification.source_ref.clone(),
            created_at: notification.created_at,
            read_at: None,
        }
    }

    /// 记录 → 载荷（丢弃 `read_at`：读状态不出线）。
    pub fn to_notification(&self) -> Notification {
        Notification {
            id: self.id.clone(),
            kind: self.kind,
            level: self.level,
            message: self.message.clone(),
            body: self.body.clone(),
            root: self.root.clone(),
            source_ref: self.source_ref.clone(),
            created_at: self.created_at,
        }
    }
}

/// 落库 sink（扇出顺序的第一位：行在呈现之前落库）。
pub struct StoreNotificationSink {
    store: Arc<Store>,
}

impl StoreNotificationSink {
    /// 以全局库实例构造（收件箱是 user 维度，MUST NOT 传 workspace 库实例）。
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

impl Sink for StoreNotificationSink {
    fn deliver(&self, notification: &Notification) {
        if let Err(error) = self.store.publish_notification(notification) {
            // 旁路降级：一行留痕即止（不重试、不上抛、不为失败再发通知）
            eprintln!("[notification] 落库失败（{}）：{error}", notification.id);
        }
    }
}
