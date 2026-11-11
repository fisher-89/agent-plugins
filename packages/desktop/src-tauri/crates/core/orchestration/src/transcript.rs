use agent::{AgentBlock, AgentEventKind, AgentMessageRole};

use crate::finish::FinishPayload;

/// 密封转录 → 收口载荷：取**末条** `SystemNotice{subtype: finish_step}`，其
/// `payload` 反序列化为 [`FinishPayload`]（缺失 / 损坏 → `None`）。本函数是收口
/// 载荷**唯一提取实现**——`worker.rs` 与测试夹具同调它，MUST NOT 各写一份。
///
/// 「末条」是语义而非「最后一条可解析的」：末条损坏时 MUST NOT 回退到前条的
/// 可解析载荷——定位与反序列化因此分两步，而非单条 `find_map` 扫描。
pub fn finish_payload(transcript: &[agent::AgentEvent]) -> Option<FinishPayload> {
    let payload = transcript
        .iter()
        .rev()
        .find_map(|event| match &event.kind {
            AgentEventKind::SystemNotice { subtype, payload }
                if subtype == crate::finish::TOOL_NAME =>
            {
                Some(payload)
            }
            _ => None,
        })?;
    serde_json::from_value(payload.clone()).ok()
}

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
                AgentBlock::Text { text, .. } => Some(text.as_str()),
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

/// 密封转录 → 失败成因（人读一句话）：引擎失败收敛的成因提取单点
/// （`api_error` 记因通知的 `error` 载荷 / 兜底 `TurnDone.subtype`）。
/// 只在失败收敛语境下调用；成因缺席为 `None`（调用点回落通用文案）。
pub fn failure_reason(transcript: &[agent::AgentEvent]) -> Option<String> {
    // 末条 api_error 记因（`error` 载荷）——sdk 引擎失败收敛的记因通道
    let cause = transcript.iter().rev().find_map(|event| {
        let AgentEventKind::SystemNotice { subtype, payload } = &event.kind else {
            return None;
        };
        if subtype != "api_error" {
            return None;
        }
        payload
            .get("error")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    });
    if cause.is_some() {
        return cause;
    }
    // 记因通知缺席（如 CLI 引擎无 api_error 留痕）：退到失败 TurnDone 的 subtype
    transcript.iter().rev().find_map(|event| {
        let AgentEventKind::TurnDone {
            subtype, is_error, ..
        } = &event.kind
        else {
            return None;
        };
        if !is_error {
            return None;
        }
        let subtype = subtype.trim();
        if subtype.is_empty() {
            None
        } else {
            Some(subtype.to_owned())
        }
    })
}
