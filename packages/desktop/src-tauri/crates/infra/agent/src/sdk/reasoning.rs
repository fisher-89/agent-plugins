use rig::message::{AssistantContent, Issuer, Message, Reasoning};

/// 思考凭证的签发方（见模块 doc：anthropic wire 的回放判别值）。
pub(crate) const ISSUER: &str = "anthropic";

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
