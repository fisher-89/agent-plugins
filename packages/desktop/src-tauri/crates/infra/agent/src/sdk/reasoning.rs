use agent::AgentBlock;
use rig::message::{AssistantContent, Issuer, Message, Reasoning, ReasoningContent, Sealed};

/// 思考凭证的签发方（见模块 doc：anthropic wire 的回放判别值）。
pub(crate) const ISSUER: &str = "anthropic";

/// 一条 sealed reasoning → 零到多块 `Thinking`：**逐载荷一块，保原序**。
///
/// 不拼接是硬约束：文本与凭证逐块配对，拼成一条即让凭证错位（严格 provider
/// 直接拒收）。以**自身发行方**打开：展示不因跨发行方失明；凭证原样随块。
///
/// 载荷映射：`Text` → 文本 + 签名；`Summary` → 仅文本（无凭证）；`Redacted`
/// → 只有密文（`thinking` 空串）；`Encrypted` → 不产块（无可展示文本，本期也
/// 不在回放口径内）。
pub(crate) fn project(reasoning: &Sealed<Reasoning>) -> Vec<AgentBlock> {
    let Some(value) = reasoning.open(reasoning.issuer()) else {
        return Vec::new();
    };
    value
        .content
        .iter()
        .filter_map(|part| match part {
            ReasoningContent::Text { text, signature } => Some(AgentBlock::Thinking {
                thinking: text.clone(),
                signature: signature.clone(),
                redacted: None,
            }),
            ReasoningContent::Summary(summary) => Some(AgentBlock::Thinking {
                thinking: summary.clone(),
                signature: None,
                redacted: None,
            }),
            ReasoningContent::Redacted { data } => Some(AgentBlock::Thinking {
                thinking: String::new(),
                signature: None,
                redacted: Some(data.clone()),
            }),
            ReasoningContent::Encrypted(_) => None,
        })
        .collect()
}

/// 一条思考块 → 回放载荷（无凭证返回 `None`）。
///
/// `redacted` 优先：安全过滤块没有可读文本，只有密文。
pub(crate) fn replay(
    thinking: &str,
    signature: Option<&str>,
    redacted: Option<&str>,
) -> Option<AssistantContent> {
    let reasoning = match (signature, redacted) {
        (_, Some(data)) => Reasoning::redacted(data),
        (Some(signature), None) => {
            Reasoning::new_with_signature(thinking, Some(signature.to_owned()))
        }
        (None, None) => return None,
    };
    Some(AssistantContent::Reasoning(
        reasoning.sealed(Issuer::from_static(ISSUER)),
    ))
}

/// 跨轮剔除：整段历史的思考块不进请求史（调用点 = 重建史入口）。
///
/// 退化保护：剔除后会变空的 assistant 消息（实体内容全在思考里）**原样保留**
/// ——空 assistant 消息会被 provider 拒。
pub(crate) fn strip_all(history: Vec<Message>) -> Vec<Message> {
    history
        .into_iter()
        .map(|message| {
            let fallback = message.clone();
            without_reasoning(message).unwrap_or(fallback)
        })
        .collect()
}

/// 剔除 reasoning 项后的单条消息（`None` = 已无实体内容）；摘要转录
/// （`compact`）用本口径——密文进摘要提示词既浪费体量也无语义。
pub(crate) fn without_reasoning(message: Message) -> Option<Message> {
    let Message::Assistant { id, content } = message else {
        return Some(message);
    };
    let kept: Vec<AssistantContent> = content
        .into_iter()
        .filter(|item| !matches!(item, AssistantContent::Reasoning(_)))
        .collect();
    (!kept.is_empty()).then_some(Message::Assistant { id, content: kept })
}
