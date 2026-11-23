//! 事件投影单点：把 [`RunUpdate`] 投影成「值得喊人的一条通知」。
//!
//! 白名单、level 映射、主键铸造与文案字面**全在这一处**——命令层与 sink 层
//! MUST NOT 出现「按变体 `if` 过滤」的第二份判定。`Step` / `Edge` /
//! `SessionEvent` 是一次 run 数十上百条的运行期信号（视图重查面），永久排除；
//! `Ask` / `ConfirmWait` / `Finished` 三个「用户多半不在场」的时刻纳入。
//!
//! 投影是旁路消费者：本模块不写库、不广播、不 await，也不改动
//! [`RunUpdate`] / `RunNotice` 的任何线面语义。
//!
//! 本模块是投影与投递的**唯一组合点**（[`project`] / [`deliver`]），调用面只有
//! 编排侧的 `RunGuard::emit` 与壳层 `AskPort::ask_started`——两侧各只做一行转发。

use notification::{Notification, NotificationKind, NotificationLevel};
use workflow::write::phase_table;

use crate::state::{ChangeRunStatus, RunUpdate};

/// 投影上下文：五个值全部由发起侧已读出的 `ChangeRecord` 与 run 局部量携带
///（MUST NOT 为文案或主键再查一次 store）。
///
/// 进程内类型：不出线、不进命令签名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyContext {
    /// 工作区归属字符串（workspace root）
    pub root: String,
    /// change id（`source_ref` 的取值）
    pub change_id: String,
    /// change 展示名（文案必含要素）
    pub change_name: String,
    /// run id（幂等主键的成分）
    pub run_id: String,
    /// 工作流类型（`ConfirmWait` 的相位人话标签按它查相位表）
    pub workflow_type: String,
}

/// 把一条运行期更新投影为通知：三变体 `None`、三变体 `Some`。
///
/// `now` 由调用方注入（UTC unix 毫秒）：既是 `created_at`，也是 `Ask` 的
/// `tool_call_id` 缺位时的退化主键成分。
pub fn project(update: &RunUpdate, ctx: &NotifyContext, now: i64) -> Option<Notification> {
    match update {
        // 三变体永久排除：运行期步状态 / 图边 / 会话事件都是「视图重查」信号，
        // 不是值得喊人的事件（转录另有面板承载）
        RunUpdate::Step { .. } | RunUpdate::Edge { .. } | RunUpdate::SessionEvent { .. } => None,
        RunUpdate::Ask { ask } => {
            // 多轮 ask 各一行：主键带 tool_call_id，缺位（旧线载荷）退发起时刻
            let token = ask.tool_call_id.clone().unwrap_or_else(|| now.to_string());
            Some(event(
                ctx,
                now,
                format!("run:{}:ask:{}", ctx.run_id, token),
                NotificationLevel::Warning,
                format!("变更「{}」在等你的回答", ctx.change_name),
                // 选项不进 body：页面里的 AskCard 才是答案面的权威承载
                Some(ask.question.clone()),
            ))
        }
        RunUpdate::ConfirmWait { phase, .. } => Some(event(
            ctx,
            now,
            format!("run:{}:confirm:{}", ctx.run_id, phase),
            NotificationLevel::Warning,
            format!("变更「{}」停在 {phase} 前等你确认", ctx.change_name),
            // 相位人话标签的唯一来源是相位表；查不到即 None（不猜测、不回落英文）
            phase_description(&ctx.workflow_type, phase),
        )),
        RunUpdate::Finished { status, reason } => match status {
            ChangeRunStatus::Completed => Some(event(
                ctx,
                now,
                format!("run:{}:finished", ctx.run_id),
                NotificationLevel::Success,
                format!("变更「{}」已完成全部相位", ctx.change_name),
                None,
            )),
            ChangeRunStatus::Failed => Some(event(
                ctx,
                now,
                format!("run:{}:finished", ctx.run_id),
                NotificationLevel::Error,
                format!("变更「{}」运行失败", ctx.change_name),
                reason.clone(),
            )),
            // 受控终止落库但不弹 toast（前端机械规则：`Info` 不弹）
            ChangeRunStatus::Stopped => Some(event(
                ctx,
                now,
                format!("run:{}:finished", ctx.run_id),
                NotificationLevel::Info,
                format!("变更「{}」已停止", ctx.change_name),
                None,
            )),
            // 非终态 `Finished` 是上游缺陷：防御性不产通知，不改任何状态
            ChangeRunStatus::Running
            | ChangeRunStatus::WaitingConfirm
            | ChangeRunStatus::WaitingAsk => None,
        },
    }
}

/// 事件轨出口缝：core 只认 trait，实现驻壳层（壳把它转发到通知扇出点）。
///
/// **旁路语义**：三个方法都无返回值、非 `async`——投递失败由实现自降级（一行
/// 留痕），MUST NOT 上抛、MUST NOT 让随后的 `control.publish` 或命令返回值受
/// 影响；实现 MUST NOT 在本 trait 内为失败再发一条通知（递归禁止）。
pub trait RunNotifier: Send + Sync {
    /// 本次 run 的投影上下文（五值由发起侧装配，投影零额外 IO）。
    fn context(&self) -> &NotifyContext;
    /// 投递一条已投影的通知（实现自己决定扇出与降级）。
    fn deliver(&self, notification: &Notification);
    /// 终态清理：本 run 的等待类未读置已读（`Finished` 之后发一次）。
    fn retire(&self);
}

/// 六变体旁路投递的**唯一组合点**：投影 → 投递 →（终态）清理。
///
/// 两处调用者（编排侧 `RunGuard::emit` 与壳层 `AskPort::ask_started`）各只做一行
/// 转发——白名单与终态清理判定只写在本函数与 [`project`] 里。
pub fn deliver(update: &RunUpdate, notifier: &dyn RunNotifier, now: i64) {
    if let Some(notification) = project(update, notifier.context(), now) {
        notifier.deliver(&notification);
    }
    if matches!(update, RunUpdate::Finished { .. }) {
        notifier.retire();
    }
}

/// 事件通知装配：三条恒定值（`kind = Event` / `root` / `source_ref`）在此收口。
fn event(
    ctx: &NotifyContext,
    now: i64,
    id: String,
    level: NotificationLevel,
    message: String,
    body: Option<String>,
) -> Notification {
    Notification {
        id,
        kind: NotificationKind::Event,
        level,
        message,
        body,
        root: Some(ctx.root.clone()),
        source_ref: Some(ctx.change_id.clone()),
        created_at: now,
    }
}

/// 相位 id → 相位表的人话标签；工作流类型未知或相位不在表内即 `None`
///（MUST NOT 另立翻译表）。
fn phase_description(workflow_type: &str, phase: &str) -> Option<String> {
    phase_table(workflow_type)?
        .iter()
        .find(|definition| definition.id == phase)
        .map(|definition| definition.description.to_owned())
}
