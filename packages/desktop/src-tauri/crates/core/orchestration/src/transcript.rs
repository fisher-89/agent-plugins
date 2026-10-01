//! 转录提取器：末条 assistant 文本（verdict / 决策 JSON 载体）。纯函数，
//! 输入即内核泵收集的密封事件全集。变更文件上下文不在此提取——桌面 run
//! 的 `file_log` 零新增（AC-3），上下文面由 `DiffContextPort` 的 git diff 承载。

use agent::{AgentBlock, AgentEventKind, AgentMessageRole};

/// 密封转录 → 末条 assistant Message 的文本块拼接（verdict / 决策 JSON
/// 载体）：无 assistant 消息或文本块全空 → `None`。
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
