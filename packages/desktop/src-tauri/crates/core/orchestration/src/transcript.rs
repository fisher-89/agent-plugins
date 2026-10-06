use agent::{AgentBlock, AgentEventKind, AgentMessageRole};

/// 密封转录 → 末条 assistant Message 的文本块拼接
pub fn final_assistant_text(transcript: &[agent::AgentEvent]) -> Option<String> {
    transcript.iter().rev().find_map(|event| {
        let AgentEventKind::Message { role, blocks, .. } = &event.kind else {
            return None;
        };
        if *role != AgentMessageRole::Assistant {
            return None;
        }
        let text = blocks
            .iter()
            .filter_map(|block| match block {
                AgentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}
