use serde::{Deserialize, Serialize};
use serde_json::Value;
use workflow::model::{ChecklistItem, Verdict};
use workflow::write::{BacktrackOption, BacktrackWindowEntry};

/// 决策 reason 长度上限
pub(crate) const MAX_REASON_CHARS: usize = 500;

/// 决策动作封闭集（线形态见 `finish.rs` 的 decision 形态 parameters）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum DecisionAction {
    /// 白名单内自主回溯
    Backtrack { to: String, reason: String },
    /// 受控终止 run
    Stop { reason: String },
    /// **生产不可达**：`finish_step` 的 decision 形态 `enum` 已不含 `ask`（提问迁出
    /// 为独立阻塞工具 `AskUserQuestion`）；保留以支持历史 `SystemNotice{finish_step}`
    /// 载荷反序列化，walker 防御臂据此判漂移
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
    /// phase-next 白名单（随 error 响应下发，walker 不自相位表推导——守住路由
    /// 红线）；每项携相位特性描述（路由层自相位表下发），决策 prompt 逐条渲染
    /// 供判断
    pub allowed: Vec<BacktrackOption>,
    /// 候选相位最近 eval report
    pub candidates: Vec<CandidateReport>,
    /// 回溯重做窗口集合（透传自写面 error 响应；walker MUST NOT 自读 store、
    /// MUST NOT 自相位表二次推导——判据与渲染单源 = 写面「回溯重做窗口推导
    /// 与 prompt 携带」）
    pub backtrack_window: Vec<BacktrackWindowEntry>,
}

/// `finish_step` 的 decision 形态参数校验：`action` tag ∈ 封闭集 → 各动作必填
/// 字段（`backtrack`: `to` + `reason`；`stop`: `reason`；`ask`: `question` + 非空
/// `options`——该臂保留供历史 notice 反序列化，生产路径已无 schema 产出它）→
/// `reason ≤ MAX_REASON_CHARS`。Err 文案自足（字段名 + 期望 + 当前值），由工具
/// 体当场回灌——同一 turn 内模型可立即纠正重试，不跨轮计数。
pub fn validate_decision(args: &Value) -> Result<DecisionAction, String> {
    let object = args
        .as_object()
        .ok_or_else(|| "参数形状非法：期望对象 { action, ... }，当前非对象".to_owned())?;
    let action = object
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| "字段 action 缺失或非字符串：期望 backtrack / stop / ask 之一".to_owned())?;
    match action {
        "backtrack" => {
            let to = required_str(object, "to", "backtrack")?;
            let reason = required_str(object, "reason", "backtrack")?;
            ensure_reason_len(&reason)?;
            Ok(DecisionAction::Backtrack { to, reason })
        }
        "stop" => {
            let reason = required_str(object, "reason", "stop")?;
            ensure_reason_len(&reason)?;
            Ok(DecisionAction::Stop { reason })
        }
        "ask" => {
            let question = required_str(object, "question", "ask")?;
            let options: Vec<String> = object
                .get("options")
                .and_then(Value::as_array)
                .ok_or_else(|| "字段 options 缺失或非数组：期望至少一个选项字符串".to_owned())?
                .iter()
                .map(|option| option.as_str().map(str::to_owned))
                .collect::<Option<Vec<String>>>()
                .ok_or_else(|| "字段 options 含非字符串元素：期望字符串数组".to_owned())?;
            if options.is_empty() {
                return Err("字段 options 为空数组：期望至少一个选项".to_owned());
            }
            Ok(DecisionAction::Ask { question, options })
        }
        other => Err(format!(
            "字段 action 值非法: {other:?}：期望封闭集 [\"backtrack\", \"stop\", \"ask\"] 之一"
        )),
    }
}

/// 动作必填字符串字段读取（缺失 / 空白即 Err，文案携动作名与字段名）。
fn required_str(
    object: &serde_json::Map<String, Value>,
    field: &str,
    action: &str,
) -> Result<String, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            format!("动作 {action} 的字段 {field} 缺失 / 非字符串 / 空白：期望非空字符串")
        })
}

/// reason 长度门（自包含：上限常量 + 当前值 + 超出量）。
fn ensure_reason_len(reason: &str) -> Result<(), String> {
    let chars = reason.chars().count();
    if chars > MAX_REASON_CHARS {
        return Err(format!(
            "字段 reason 超长：上限 {MAX_REASON_CHARS} 字符，当前 {chars} 字符，超出 {}",
            chars - MAX_REASON_CHARS
        ));
    }
    Ok(())
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
