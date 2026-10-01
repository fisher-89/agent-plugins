use std::path::PathBuf;
use std::time::Instant;

use agent::{AgentBlock, AgentEventKind, AgentPermissionMode, RunHandle};
use futures::StreamExt;
use rig_core::completion::{CompletionModel, CompletionRequest, Usage};
use rig_core::message::{AssistantContent, Message, ToolCall};
use rig_core::streaming::StreamedAssistantContent;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::sdk::{normalize, policy, sandbox, tools};

/// 轮数上限（熔断）：模型持续要求工具而不收敛时以失败收敛，防失控烧 token。
const MAX_TURNS: u64 = 50;

/// 正常收敛 / 失败收敛的 `TurnDone.subtype`（CLI 线格式口径）。
const SUBTYPE_SUCCESS: &str = "success";

/// 单轮驱动入参（协议轮参数的 loop 投影；引擎侧会话标识每轮铸造）。
pub(crate) struct LoopTurn {
    /// 轮提问
    pub question: String,
    /// 工作目录
    pub cwd: PathBuf,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
    /// 模型标识（RunStarted 上报口径与 CLI init 对齐）
    pub model_name: String,
    /// 引擎侧会话标识（每轮 `sdk-` 前缀铸造，经 RunStarted / TurnDone 上报，
    /// 内核写双 id 映射落库半边）
    pub session_id: String,
}

/// loop 主体：灌未盖戳事件至有界通道，收敛时发送 `TurnDone`（停止 / 消费端
/// 关闭路径除外），返回累积后的对话史（会话泵任务跨轮续跑的全史）。`model`
/// 为 rig completion model（openai chat completions 形态）；`history` 为续
/// 会话重建史（全新运行为空史）。
pub(crate) async fn run<M>(
    model: &M,
    turn: &LoopTurn,
    mut history: Vec<Message>,
    sender: mpsc::Sender<AgentEventKind>,
    handle: RunHandle,
) -> Vec<Message>
where
    M: CompletionModel,
{
    let started = Instant::now();
    let mut usage = Usage::new();
    let definitions = tools::definitions();

    // run 启动事件（model / session / tools 口径与 CLI init 对齐；MCP 恒空）
    let started_ok = sender
        .send(AgentEventKind::RunStarted {
            model: Some(turn.model_name.clone()),
            session_id: Some(turn.session_id.clone()),
            tools: tools::TOOL_NAMES
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            mcp_servers: Vec::new(),
        })
        .await
        .is_ok();
    if !started_ok {
        return history; // 消费端关闭：泵自行退出
    }
    // 用户提示词：密封入史（时间线事件 + 入史；重建史尾追加新轮提示）
    let prompt_ok = sender
        .send(AgentEventKind::Message {
            role: "user".to_owned(),
            blocks: vec![AgentBlock::Text {
                text: turn.question.clone(),
            }],
            parent_tool_use_id: None,
        })
        .await
        .is_ok();
    if !prompt_ok {
        return history;
    }
    history.push(Message::user(turn.question.clone()));

    for turn_index in 1..=MAX_TURNS {
        // 轮间空档停止检查（泵 select 之外的快速路径）
        if handle.stop_requested() {
            return history;
        }
        let request = CompletionRequest {
            model: None,
            preamble: None,
            chat_history: history.clone(),
            documents: Vec::new(),
            tools: definitions.clone(),
            temperature: None,
            max_tokens: None,
            tool_choice: None,
            additional_params: None,
            output_schema: None,
            record_telemetry_content: false,
        };
        let mut response = match model.stream(request).await {
            Ok(response) => response,
            Err(error) => {
                finish_error(
                    &sender,
                    turn_index,
                    started,
                    &usage,
                    turn,
                    "api_error",
                    format!("API 请求失败: {error}"),
                )
                .await;
                return history;
            }
        };
        // 流消费至 EOF（可恢复帧错误 drain 不中断，rig 契约：Err 非必然终局）
        let mut stream_error: Option<String> = None;
        while let Some(item) = response.next().await {
            match item {
                Ok(StreamedAssistantContent::Final(record)) => {
                    usage += record.usage;
                }
                Ok(item) => {
                    if let Some(kind) = normalize::stream_item(&item) {
                        if sender.send(kind).await.is_err() {
                            return history; // 消费端关闭：泵自行退出，不合成 TurnDone
                        }
                    }
                }
                Err(error) => {
                    stream_error.get_or_insert_with(|| error.to_string());
                }
            }
        }
        if let Some(error) = stream_error {
            finish_error(
                &sender,
                turn_index,
                started,
                &usage,
                turn,
                "api_error",
                format!("API 流失败: {error}"),
            )
            .await;
            return history;
        }
        // 轮末聚合：choice 整体复用为 rig assistant 消息回灌
        let choice = std::mem::take(&mut response.choice);
        if choice.is_empty() {
            // 空轮（无文本无工具调用）：以正常收敛防死循环
            finish_success(&sender, turn_index, started, &usage, turn).await;
            return history;
        }
        // 密封收口：choice 全部块收进恰一条密封 Message（role=assistant，
        // 增量层在传输面的 provisional 让位即同键替换）
        let mut blocks: Vec<AgentBlock> = Vec::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        for content in &choice {
            match content {
                AssistantContent::Text(text) => blocks.push(AgentBlock::Text {
                    text: text.text.clone(),
                }),
                AssistantContent::Reasoning(reasoning) => blocks.push(AgentBlock::Thinking {
                    thinking: reasoning.display_text(),
                }),
                AssistantContent::ToolCall(tool_call) => {
                    blocks.push(AgentBlock::ToolUse {
                        id: tool_call.id.as_str().to_owned(),
                        name: tool_call.function.name.clone(),
                        input: tool_call.function.arguments.clone(),
                    });
                    tool_calls.push(tool_call.clone());
                }
                AssistantContent::Image(_) => {} // sdk 引擎不产图像内容（工具面无边）
            }
        }
        history.push(Message::Assistant {
            id: None,
            content: choice,
        });
        if sender
            .send(AgentEventKind::Message {
                role: "assistant".to_owned(),
                blocks,
                parent_tool_use_id: None,
            })
            .await
            .is_err()
        {
            return history; // 消费端关闭：泵自行退出
        }
        if tool_calls.is_empty() {
            finish_success(&sender, turn_index, started, &usage, turn).await;
            return history;
        }
        // 工具调用逐个：policy → sandbox → 执行 → 回灌（run 不中断）
        for tool_call in tool_calls {
            let id = tool_call.id.as_str().to_owned();
            let name = tool_call.function.name.clone();
            let input = tool_call.function.arguments.clone();
            if !policy::allows(turn.permission_mode, &name) {
                let reason = format!("权限档位不允许该工具: {name}");
                deny(&sender, "permission_denied", &name, &reason, &id).await;
                history.push(Message::tool_result(id, name, reason));
                continue;
            }
            let sandboxed = match sandbox_rewrite(&turn.cwd, &name, &input) {
                Ok(rewritten) => rewritten,
                Err(reason) => {
                    deny(&sender, "sandbox_denied", &name, &reason, &id).await;
                    history.push(Message::tool_result(id, name, reason));
                    continue;
                }
            };
            let (content, is_error) = match tools::execute(&turn.cwd, &name, &sandboxed).await {
                Ok(text) => (text, false),
                Err(error) => (error, true),
            };
            if sender
                .send(AgentEventKind::Message {
                    role: "user".to_owned(),
                    blocks: vec![AgentBlock::ToolResult {
                        id: id.clone(),
                        content: content.clone(),
                        is_error,
                    }],
                    parent_tool_use_id: None,
                })
                .await
                .is_err()
            {
                return history; // 消费端关闭：泵自行退出
            }
            history.push(Message::tool_result(id, name, content));
        }
    }
    // 轮数熔断：以失败收敛（模型持续要求工具不收敛）
    finish_error(
        &sender,
        MAX_TURNS,
        started,
        &usage,
        turn,
        "error_max_turns",
        format!("轮数达到上限 {MAX_TURNS} 仍未收敛"),
    )
    .await;
    history
}

/// 拒绝合成：`SystemNotice{subtype}` + is_error ToolResult 密封 user 消息
/// （与 tool_use 同 id，成对回灌；run 不中断）。
async fn deny(
    sender: &mpsc::Sender<AgentEventKind>,
    subtype: &str,
    name: &str,
    reason: &str,
    id: &str,
) {
    let notice = AgentEventKind::SystemNotice {
        subtype: subtype.to_owned(),
        payload: serde_json::json!({ "tool": name, "reason": reason }),
    };
    if sender.send(notice).await.is_err() {
        return; // 消费端关闭：外层续轮的下一发自然退出
    }
    let result = AgentEventKind::Message {
        role: "user".to_owned(),
        blocks: vec![AgentBlock::ToolResult {
            id: id.to_owned(),
            content: reason.to_owned(),
            is_error: true,
        }],
        parent_tool_use_id: None,
    };
    let _ = sender.send(result).await;
}

/// sandbox 校验与入参改写：路径类工具（path 字段）经 [`sandbox::check`]
/// 解析后回写 `path`；glob 无 path 字段，以 [`sandbox::check_pattern`] 预检
/// pattern（执行体以 root 为基座拼接）。未知工具无路径面，原样放行（policy
/// 已先行拒绝封闭清单外的工具名）。
fn sandbox_rewrite(root: &std::path::Path, name: &str, input: &Value) -> Result<Value, String> {
    let Some(raw) = tools::input_path(name, input) else {
        return Ok(input.clone());
    };
    if name == "glob" {
        sandbox::check_pattern(root, raw)?;
        return Ok(input.clone());
    }
    let resolved = sandbox::check(root, raw)?;
    let mut rewritten = input.clone();
    if let Some(object) = rewritten.as_object_mut() {
        object.insert(
            "path".to_owned(),
            Value::String(resolved.to_string_lossy().into_owned()),
        );
    }
    Ok(rewritten)
}

/// 正常收敛：发送 `TurnDone{is_error:false}`（usage 有值才填，缺失置 null）。
async fn finish_success(
    sender: &mpsc::Sender<AgentEventKind>,
    turns: u64,
    started: Instant,
    usage: &Usage,
    turn: &LoopTurn,
) {
    finish(
        sender,
        false,
        SUBTYPE_SUCCESS,
        Some(turns),
        started,
        usage,
        turn,
    )
    .await;
}

/// 失败收敛：`SystemNotice{api_error}` 记因 + `TurnDone{is_error:true}`
/// （`result_subtype` 区分成因：API 失败 `api_error`、轮数熔断
/// `error_max_turns`——CLI 合成收敛事件的命名口径）。
async fn finish_error(
    sender: &mpsc::Sender<AgentEventKind>,
    turns: u64,
    started: Instant,
    usage: &Usage,
    turn: &LoopTurn,
    result_subtype: &str,
    cause: String,
) {
    let notice = AgentEventKind::SystemNotice {
        subtype: "api_error".to_owned(),
        payload: serde_json::json!({ "error": cause }),
    };
    // 记因通知尽力流出（消费端关闭时收敛事件仍组装，发送失败静默退出）
    let _ = sender.send(notice).await;
    finish(
        sender,
        true,
        result_subtype,
        Some(turns),
        started,
        usage,
        turn,
    )
    .await;
}

/// `TurnDone` 组装与发送（统计字段面唯一出口、cost 恒 None——无价格表，缺席
/// 合法缺省；收敛事件恒为最后一事件；消费端关闭静默退出）。
async fn finish(
    sender: &mpsc::Sender<AgentEventKind>,
    is_error: bool,
    subtype: &str,
    turns: Option<u64>,
    started: Instant,
    usage: &Usage,
    turn: &LoopTurn,
) {
    let kind = AgentEventKind::TurnDone {
        subtype: subtype.to_owned(),
        is_error,
        num_turns: turns,
        duration_ms: Some(started.elapsed().as_millis() as u64),
        cost_usd: None,
        usage: if usage.has_values() {
            serde_json::to_value(usage).unwrap_or(Value::Null)
        } else {
            Value::Null
        },
        session_id: Some(turn.session_id.clone()),
    };
    let _ = sender.send(kind).await;
}
