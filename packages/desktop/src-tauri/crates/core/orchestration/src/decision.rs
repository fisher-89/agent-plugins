//! 决策解析与白名单预校验：决策 agent 最终消息 → 四动作封闭集（backtrack /
//! retry / stop / ask，tag `action` 判别）；backtrack 越权在 walker 侧先行
//! 预校验（写面 `backtrack` 白名单二次校验兜底——坏决议损坏不了状态）。
//! 结构漂移显式 `Err` 停给用户；reason 长度不在解析层设门——backtrack
//! reason ≤500 由写面 [`workflow::write::backtrack`] 单点拒绝（解析层透传）。
//! 白名单即写面下发的相位 id 串（`Vec<String>`），verdict 引用直用
//! `workflow::model` 域类型。

use serde::Deserialize;
use workflow::model::{ChecklistItem, Verdict};

/// 决策四动作封闭集（JSON 形状见 `prompt.rs` 决策协议附录）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum DecisionAction {
    /// 白名单内自主回溯（reason 必带，≤500）
    Backtrack { to: String, reason: String },
    /// 原相位重试（walker 自走，消耗相位 retry 预算）
    Retry,
    /// 受控终止 run
    Stop { reason: String },
    /// 无法裁决 → UI 中断提问（应答后 Continue 决策会话重出封闭集）
    Ask {
        question: String,
        options: Vec<String>,
    },
}

/// 候选相位最近一次 eval report（决策有界输入的第三面）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateReport {
    pub phase: String,
    pub verdict: Option<Verdict>,
    pub report: Option<String>,
}

/// 决策有界输入：全部来自 workflow.json 只读装配与 phase-next 缓存白名单，
/// 无其它上下文（有界性 = 输入白名单三面，prompt 组装不增料）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionInput {
    /// 失败相位 id
    pub phase: String,
    /// 失败 attempt 号
    pub attempt: u32,
    /// fail checklist（fail 项 + evidence）
    pub fail_checklist: Vec<ChecklistItem>,
    /// phase-next 白名单（walker 缓存，不自相位表推导——守住路由红线）
    pub allowed: Vec<String>,
    /// 候选相位最近 eval report
    pub candidates: Vec<CandidateReport>,
}

/// 解析决策最终消息为四动作封闭集：JSON 载体提取 → tag `action` 判别。
/// 漂移显式 `Err`（walker 停给用户）；reason 长度透传不设门——backtrack
/// reason ≤500 归写面 `backtrack` 单点拒绝。
pub fn parse_decision(text: &str) -> Result<DecisionAction, String> {
    let raw = crate::verdict::extract_json(text)?;
    let action: DecisionAction =
        serde_json::from_str(&raw).map_err(|e| format!("决策 JSON 结构漂移: {e}"))?;
    Ok(action)
}

/// 越权 backtrack 预校验（walker 侧第一道）：非 backtrack 动作恒通过；
/// backtrack 目标不在白名单 → `Err`（写面 backtrack 白名单二次校验兜底，
/// 不执行回溯）。
pub fn ensure_backtrack_allowed(action: &DecisionAction, allowed: &[String]) -> Result<(), String> {
    let DecisionAction::Backtrack { to, .. } = action else {
        return Ok(());
    };
    if allowed.iter().any(|phase| phase == to) {
        return Ok(());
    }
    Err(format!(
        "越权 backtrack：目标 \"{to}\" 不在白名单内（allowed: {}）",
        allowed.join(", ")
    ))
}
