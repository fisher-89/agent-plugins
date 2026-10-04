//! evaluator verdict 解析器：evaluator 最终消息 → checklist JSON（容忍围栏
//! 代码块包裹）。结构漂移 / report 超长显式 `Err` 停给用户——不臆测 verdict、
//! 不静默降级（解析器密集测试锁漂移形态）。checklist 载荷直用
//! `workflow::model` 域类型（无跨进程视图镜像层）。

use serde::Deserialize;
use workflow::model::{ChecklistItem, Verdict};

/// report 长度上限（与插件 `phaseLogSchema.report` max 2000 同源，超长在
/// 桌面侧先行拒绝——写面落账前最后一道）。
pub const MAX_REPORT_CHARS: usize = 2000;

/// verdict 封闭结构：evaluator 最终消息输出的 checklist JSON 形状。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EvaluatorChecklist {
    pub phase: String,
    pub attempt: Option<u32>,
    pub verdict: Verdict,
    pub report: String,
    pub checklist: Vec<ChecklistItem>,
    pub skipped: Option<bool>,
}

/// 从最终消息文本提取 JSON 载体：裸 JSON 直取；围栏代码块剥壳；兜底取首个
/// `{` 到末个 `}` 的子串。三者皆失败 → 漂移 Err。
pub(crate) fn extract_json(text: &str) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("最终消息为空，无 JSON 载体".to_owned());
    }
    for candidate in [
        trimmed.to_owned(),
        fenced_block(trimmed).unwrap_or_default(),
        brace_slice(trimmed).unwrap_or_default(),
    ] {
        if candidate.is_empty() {
            continue;
        }
        if serde_json::from_str::<serde_json::Value>(&candidate).is_ok() {
            return Ok(candidate);
        }
    }
    Err("未在最终消息中找到合法 JSON（围栏代码块或裸 JSON 皆未命中）".to_owned())
}

/// 围栏代码块剥壳：首个 ``` 之后（跳过语言标签行）到末个 ``` 之前的内容。
fn fenced_block(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let after = &text[start + 3..];
    let content_start = after.find('\n').map(|idx| idx + 1).unwrap_or(0);
    let rest = &after[content_start..];
    let end = rest.rfind("```")?;
    Some(rest[..end].trim().to_owned())
}

/// 裸 JSON 兜底：首个 `{` 到末个 `}` 的子串。
fn brace_slice(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start < end).then(|| text[start..=end].to_owned())
}

/// 解析 evaluator 最终消息为 checklist：JSON 载体提取 → 封闭结构校验 →
/// report 长度门。任一步漂移显式 `Err`（walker 停给用户）。
pub fn parse_verdict(text: &str) -> Result<EvaluatorChecklist, String> {
    let raw = extract_json(text)?;
    let checklist: EvaluatorChecklist =
        serde_json::from_str(&raw).map_err(|e| format!("checklist JSON 结构漂移: {e}"))?;
    if checklist.report.chars().count() > MAX_REPORT_CHARS {
        return Err(format!(
            "checklist report 超长（{} > {MAX_REPORT_CHARS}），拒绝落账",
            checklist.report.chars().count()
        ));
    }
    Ok(checklist)
}
