use serde::Deserialize;
use workflow::model::{ChecklistItem, Verdict};
use workflow::write::BacktrackOption;

/// 决策 reason 长度上限
pub(crate) const MAX_REASON_CHARS: usize = 500;

/// 决策三动作封闭集（JSON 形状见 `prompt.rs` 决策协议附录）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum DecisionAction {
    /// 白名单内自主回溯（reason 必带，≤500）；目标为失败相位自身即「重做本
    /// 相位」（仅当该相位在白名单内）
    Backtrack { to: String, reason: String },
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

/// 决策有界输入
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionInput {
    /// 失败相位 id
    pub phase: String,
    /// 失败 attempt 号
    pub attempt: u32,
    /// fail checklist（fail 项 + evidence）
    pub fail_checklist: Vec<ChecklistItem>,
    /// 失败条目 `report` 原文（升格落账写入的完整明细，有界 ≤
    /// [`MAX_REPORT_CHARS`](crate::verdict::MAX_REPORT_CHARS)）——决策 prompt
    /// 「失败相位报告」段的唯一来源
    pub fail_report: String,
    /// phase-next 白名单（walker 缓存，不自相位表推导——守住路由红线）；每项
    /// 携相位特性描述（路由层自相位表下发），决策 prompt 逐条渲染供判断
    pub allowed: Vec<BacktrackOption>,
    /// 候选相位最近 eval report
    pub candidates: Vec<CandidateReport>,
}

/// 解析决策最终消息为三动作封闭集：JSON 载体提取 → tag `action` 判别。
/// 已退役的 `retry`、拼写漂移与未知动作同走 serde 结构漂移 `Err`（由
/// `decision_session` 软拒环处置，不一次打死 run）；reason 长度透传不设门——
/// backtrack reason ≤500 归写面 `backtrack` 单点拒绝。
pub fn parse_decision(text: &str) -> Result<DecisionAction, String> {
    let raw = crate::verdict::extract_json(text)?;
    let action: DecisionAction =
        serde_json::from_str(&raw).map_err(|e| format!("决策 JSON 结构漂移: {e}"))?;
    Ok(action)
}

/// 越权 backtrack 预校验（决策会话内每轮软拒环的一道）：非 backtrack 动作恒
/// 通过；backtrack 目标不在白名单 → `Err`（写面 backtrack 白名单二次校验兜底，
/// 不执行回溯）。拒绝文案两类：自环（`to == 失败相位` 且该相位不在白名单——
/// 零 agent 步相位不可重做）与通用越权（携 allowed 列表）。自环判定唯一条件 =
/// `to == 失败相位` 且该相位不在白名单（白名单自含 ⟺ 可重做）。
pub fn ensure_backtrack_allowed(
    action: &DecisionAction,
    failed_phase: &str,
    allowed: &[BacktrackOption],
) -> Result<(), String> {
    let DecisionAction::Backtrack { to, .. } = action else {
        return Ok(());
    };
    if allowed.iter().any(|option| option.phase == *to) {
        return Ok(());
    }
    if to == failed_phase {
        return Err(format!(
            "本相位 \"{failed_phase}\" 为确定性门禁相位（零 agent 步），不支持重做；请从白名单其他相位中选择，或 stop / ask"
        ));
    }
    Err(format!(
        "越权 backtrack：目标 \"{to}\" 不在白名单内（allowed: {}）",
        allowed
            .iter()
            .map(|option| option.phase.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}
