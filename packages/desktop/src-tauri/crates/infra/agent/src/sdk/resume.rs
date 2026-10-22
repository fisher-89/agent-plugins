use rig::message::{
    AssistantContent, Issuer, Message, Reasoning, ToolCall, ToolName, ToolResult,
    ToolResultContent, UserContent,
};

use agent::{AgentBlock, AgentEvent, AgentEventKind, AgentMessageRole};

const LOCAL_ISSUER: Issuer = Issuer::from_static("dev-team-sdk");

/// sdk 引擎侧会话 id 铸造前缀：`sdk-<进程内计数>-<毫秒时戳>`（每轮铸造，
/// 经 RunStarted / TurnDone 上报由内核记双 id 映射；不再承担归属校验）。
pub const SESSION_PREFIX: &str = "sdk-";

/// 中断调用的合成结果正文（assistant 已声明、结果回灌前 run 停止——续会话
/// 重建时补一条 is_error 语义的结果，保持 provider 配对协议不变量）。
const INTERRUPTED_RESULT: &str = "工具执行失败: 工具执行被中断（会话在结果回灌前停止）";

/// 重建 rig 对话历史
///
/// provider 配对协议不变量：assistant 消息里的每个 `tool_use` 必须由**紧随
/// 其后的同一条 user 消息**中的全部 `tool_result` 应答。转录按「每个结果一条
/// Message 事件」落库，故重建时同一 assistant 的多个结果并入一条 user 消息；
/// 结果缺失（run 在回灌前停止）时补合成中断结果，孤儿结果（无紧邻 tool_use）
/// 剔除。
pub fn rebuild(events: &[AgentEvent]) -> Result<Vec<Message>, String> {
    let mut history: Vec<Message> = Vec::new();
    // 最近一条 assistant 声明、尚未回灌结果的工具调用（配对只认紧邻消息）
    let mut pending: Vec<ToolCall> = Vec::new();
    for event in events {
        let AgentEventKind::Message {
            role,
            blocks,
            parent_tool_use_id,
        } = &event.kind
        else {
            continue; // RunStarted / SystemNotice / Raw / delta 不进对话史
        };
        if parent_tool_use_id.is_some() {
            continue; // 子代理事件压平丢弃
        }
        match role {
            AgentMessageRole::Assistant => {
                // 上一条 assistant 的未应答调用：先补中断结果，再收新回应
                answer_pending_interrupted(&mut history, &mut pending);
                let content = assistant_content(blocks);
                if !content.is_empty() {
                    pending = content
                        .iter()
                        .filter_map(|item| match item {
                            AssistantContent::ToolCall(call) => Some(call.clone()),
                            _ => None,
                        })
                        .collect();
                    history.push(Message::Assistant { id: None, content });
                }
            }
            AgentMessageRole::User | AgentMessageRole::Tool => {
                // 文本块 = 新轮提问 / 用户言语（tool role 不产文本）；出现文本
                // 即视作上一 assistant 的调用被中断，先行补齐配对
                let content = text_content(blocks);
                if !content.is_empty() {
                    answer_pending_interrupted(&mut history, &mut pending);
                    history.push(Message::User { content });
                }
                // 结果块（生产为 tool role；兼容 user 位承载结果的转录形态）
                append_tool_results(&mut history, &mut pending, blocks);
            }
        }
    }
    // 尾部未应答调用（run 停止于工具执行中）：补结果闭合配对
    answer_pending_interrupted(&mut history, &mut pending);
    if history.is_empty() {
        Err("重建历史为空（无有效顶层转录）".to_owned())
    } else {
        Ok(history)
    }
}

/// user 位文本块映射（非文本块跳过）
fn text_content(blocks: &[AgentBlock]) -> Vec<UserContent> {
    blocks
        .iter()
        .filter_map(|block| match block {
            AgentBlock::Text { text } => Some(UserContent::text(text.clone())),
            _ => None, // user 消息内的 Thinking / ToolUse 非法形态跳过
        })
        .collect()
}

/// assistant 消息块映射
fn assistant_content(blocks: &[AgentBlock]) -> Vec<AssistantContent> {
    blocks
        .iter()
        .filter_map(|block| match block {
            AgentBlock::Text { text } => Some(AssistantContent::text(text.clone())),
            AgentBlock::Thinking { thinking } => Some(AssistantContent::Reasoning(
                Reasoning::new(thinking).sealed(LOCAL_ISSUER),
            )),
            AgentBlock::ToolUse { id, name, input } => {
                let name = ToolName::new(name.clone()).ok()?;
                Some(AssistantContent::tool_call(id.clone(), name, input.clone()))
            }
            _ => None, // assistant 消息内的 ToolResult 非法形态跳过
        })
        .collect()
}

/// 工具结果并入历史：只收 [`pending`] 中声明的调用（紧邻 assistant 的配对），
/// 同一 assistant 的多个结果并入同一条 user 消息（provider 要求下一条消息
/// 覆盖全部 tool_use）。
fn append_tool_results(
    history: &mut Vec<Message>,
    pending: &mut Vec<ToolCall>,
    blocks: &[AgentBlock],
) {
    let mut results: Vec<ToolResult> = Vec::new();
    for block in blocks {
        let AgentBlock::ToolResult {
            id,
            content,
            is_error,
        } = block
        else {
            continue;
        };
        let Some(index) = pending.iter().position(|call| call.id.to_string() == *id) else {
            continue; // 非紧邻 assistant 声明的调用：孤儿结果剔除
        };
        let call = pending.remove(index);
        let body = if *is_error {
            format!("工具执行失败: {content}")
        } else {
            content.clone()
        };
        results.push(ToolResult {
            call: call.id,
            name: call.function.name,
            content: vec![ToolResultContent::text(body)],
        });
    }
    if results.is_empty() {
        return;
    }
    match history.last_mut() {
        // 同一 assistant 的前序结果位：追加而非另起一条 user 消息
        Some(Message::User { content })
            if content
                .iter()
                .all(|item| matches!(item, UserContent::ToolResult(_))) =>
        {
            content.extend(results.into_iter().map(UserContent::ToolResult));
        }
        _ => history.push(Message::tool_results(results)),
    }
}

/// 补齐未应答调用的合成结果（中断语义；与真实结果同消息合并规则）。
fn answer_pending_interrupted(history: &mut Vec<Message>, pending: &mut Vec<ToolCall>) {
    if pending.is_empty() {
        return;
    }
    let results: Vec<ToolResult> = pending
        .drain(..)
        .map(|call| ToolResult {
            call: call.id,
            name: call.function.name,
            content: vec![ToolResultContent::text(INTERRUPTED_RESULT)],
        })
        .collect();
    match history.last_mut() {
        Some(Message::User { content })
            if content
                .iter()
                .all(|item| matches!(item, UserContent::ToolResult(_))) =>
        {
            content.extend(results.into_iter().map(UserContent::ToolResult));
        }
        _ => history.push(Message::tool_results(results)),
    }
}
