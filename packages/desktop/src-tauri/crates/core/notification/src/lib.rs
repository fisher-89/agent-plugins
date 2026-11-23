//! 通知词汇与扇出点：本 crate 只定义「一条通知是什么」（词表）与「怎么把它
//! 投出去」（[`Hub`] + [`Sink`]），不含任何 IO、渠道与业务知识。
//!
//! # 三条不变量（写在 crate doc 而不靠调用方自觉）
//!
//! 1. **通知是旁路**：一个 sink 失败 MUST NOT 影响其他 sink，MUST NOT 让业务
//!    动作失败（业务返回值与状态零变化）。
//! 2. **不递归**：通知 MUST NOT 为自己的失败再发一条通知。
//! 3. **只投一次**：core 只保证投递一次；重试 / 退避 / 批量 / 排队归 sink 内部，
//!    同一 sink 内 FIFO，跨 sink 不保证顺序。
//!
//! # 渠道缝
//!
//! 本 crate 不认识任何渠道，渠道是 [`Sink`] 的实现（落库 / in-app / 将来的
//! 写文件 / 其他应用 CLI / 远程 API）。第二条渠道立项前 MUST NOT 在此预铺渠道
//! 开关或注册表。

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

/// 呈现级别四档（camelCase 线格式）：逐档对位前端 sonner 的四档呈现。
///
/// 变体序冻结——对位关系是线面契约，重排即呈现档位错位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NotificationLevel {
    /// 信息档（对位 sonner 的裸 `toast(...)`；前端不为它弹第二遍）
    Info,
    /// 成功档（对位 `toast.success`）
    Success,
    /// 警告档（对位 `toast.warning`）
    Warning,
    /// 错误档（对位 `toast.error`）
    Error,
}

/// 通知的两种用途（camelCase 线格式）：后端事件与前端动作反馈同一模型两用途
///（MUST NOT 拆两张表，过滤与未读口径靠本字段单点）。
///
/// **变体序冻结**：落库行以 serde 编码承载本枚举，重排即旧行解码错位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NotificationKind {
    /// 后端事件（流程停等 / 终态）：进收件箱列表、计入未读、经通道实时增量
    Event,
    /// 前端动作反馈：只落库，不进列表、不计未读、不广播（本窗口的交互反馈）
    Feedback,
}

/// 一条通知（出线 DTO 亦是它：`Channel` 投递与命令返回同型）。
///
/// 载荷全量出线、MUST NOT 降位为 kind-only 信号——通知是**内容本身**，与
/// 「状态已变、请重查」的失效信号语义不同。**不含读状态**：读属收件箱记录面，
/// 投递面是「不可变的一条内容」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// 稳定标识：事件为幂等串（同主键重投即更新同一行），反馈为前端铸造的 UUID
    pub id: String,
    /// 用途二值（事件 / 反馈）
    pub kind: NotificationKind,
    /// 呈现级别四档
    pub level: NotificationLevel,
    /// 主文案（事件文案的字面唯一真相在编排侧投影函数）
    pub message: String,
    /// 副文案（前端呈现为 sonner 的 `description`）；无则 None
    pub body: Option<String>,
    /// 工作区归属字符串（记录自带归属，MUST NOT 依赖跨库 join）；反馈轨恒 None
    pub root: Option<String>,
    /// 触发源引用（事件为 change id）；反馈轨恒 None
    pub source_ref: Option<String>,
    /// 产生时刻（UTC unix 毫秒）
    pub created_at: i64,
}

impl Notification {
    /// 前端动作反馈构造器：固定 `kind = Feedback`、`root` / `source_ref` 恒 None
    /// （反馈不带工作区归属断言——门面是前端纯函数，不持有当前根），
    /// `created_at` 由服务端补。域语义在此收口，命令层不自拼。
    pub fn feedback(input: NotificationFeedbackInput, created_at: i64) -> Self {
        Self {
            id: input.id,
            kind: NotificationKind::Feedback,
            level: input.level,
            message: input.message,
            body: input.body,
            root: None,
            source_ref: None,
            created_at,
        }
    }
}

/// 收件箱分页信封：本页条目 + 该 kind 全量计数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPage {
    /// 本页条目（倒序，页界自洽）
    pub items: Vec<Notification>,
    /// `kind = Event` 的**全量**计数（非本页长度；分页边界与行集同一读快照）
    pub total: u32,
}

/// 前端反馈轨的落库入参：id 由前端铸造（`crypto.randomUUID()`），服务端补
/// `kind` / `root` / `source_ref` / `created_at`（见 [`Notification::feedback`]）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NotificationFeedbackInput {
    /// 前端铸造的唯一标识（反馈天生唯一，无 dedupe 需求）
    pub id: String,
    /// 呈现级别四档
    pub level: NotificationLevel,
    /// 主文案
    pub message: String,
    /// 副文案；无则 None
    pub body: Option<String>,
}

/// 投递缝：同步、无返回值、非 `async`、无 `Result`——重试 / 退避 / 批量 / 排队
/// 全部归实现内部，core 侧不 await。失败 MUST NOT 上抛（旁路不变量）。
pub trait Sink {
    fn deliver(&self, notification: &Notification);
}

/// 扇出点：按装配顺序遍历 sinks 各投递一次。
///
/// 顺序即装配顺序（壳侧固定「先落库 sink、后 in-app sink」——行在呈现之前
/// 落库）。**不** `catch_unwind`：panic 是实现缺陷而非「失败」，本仓既有 sink
/// 纪律是回调 panic 不被吞掉；「一 sink 失败不影响其他」的落地形态是 sink 内部
/// 把自身的 `Err` 降级为一行留痕。
pub struct Hub {
    sinks: Vec<Arc<dyn Sink + Send + Sync>>,
}

impl Hub {
    /// 装配扇出点：入参顺序即投递顺序。
    pub fn new(sinks: Vec<Arc<dyn Sink + Send + Sync>>) -> Self {
        Self { sinks }
    }

    /// 投递一条通知：顺序遍历全部 sink 各投递一次；无返回值、不重投、不吞 panic。
    pub fn publish(&self, notification: &Notification) {
        for sink in &self.sinks {
            sink.deliver(notification);
        }
    }
}

#[cfg(test)]
mod lib_test;
