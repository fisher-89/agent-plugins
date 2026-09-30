//! 手搓多轮 agent loop：流式响应 → 事件归一化 → policy/sandbox 检查 →
//! 工具执行 → 结果回灌 → 续轮。
//!
//! rig 只作 provider client + 消息/工具类型层：轮循环、事件信封、拒绝
//! 合成、`RunResult` 填充全自控（rig multi_turn 托管不采用——其工具执行
//! 管制与我们的 `AgentEvent` 信封、policy 插值、收敛语义不贴）。
//!
//! 轮结构（关键实现语义定稿 1）：
//! 1. 逐轮组装 `CompletionRequest`（重建历史/回灌史 + 六工具 definitions）
//! 2. `stream()` 归一化流消费至 EOF：文本 / reasoning / tool call 增量出
//!    事件（[`normalize`]），终端 usage 累计；可恢复帧错误 drain 不中断
//! 3. 轮末聚合 `choice` 整体回灌为 assistant 消息；无 tool call → 正常
//!    收敛（`RunResult{is_error:false}`）
//! 4. 有 tool call → 逐个 policy 检查（拒绝 → `SystemNotice{permission_denied}`
//!    + is_error ToolResult）→ sandbox 校验（拒绝 → `SystemNotice{sandbox_denied}`
//!    + is_error ToolResult）→ 执行出 ToolResult → 全部回灌后续轮
//!
//! 停止语义：泵任务（runner）以 `select` 包裹本 loop——停止即 drop 本
//! future，不合成 RunResult（编排侧显式收敛 stopped）；消费端关闭（页面关）
//! 事件发送失败自行退出，同样不合成。轮间空档的信号检查是快速路径兜底。
//!
//! 失败语义：API / 传输失败 → `SystemNotice{api_error}` + `RunResult{is_error:true}`；
//! 轮数熔断（模型持续要求工具不收敛）→ `SystemNotice{error_max_turns}` +
//! `RunResult{is_error:true}`。启动失败不进本模块（runner 以启动错误显式
//! 拒绝，不产生 run 记录）。

use std::path::Path;
use std::time::Instant;

use agent::{AgentBlock, AgentEvent, AgentEventKind, AgentRunParams, RunHandle};
use futures::StreamExt;
use rig_core::completion::{CompletionModel, CompletionRequest, Usage};
use rig_core::message::{AssistantContent, Message, ToolCall};
use rig_core::streaming::StreamedAssistantContent;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::sdk::{normalize, policy, sandbox, tools};

/// 轮数上限（熔断）：模型持续要求工具而不收敛时以失败收敛，防失控烧 token。
const MAX_TURNS: u64 = 50;

/// 正常收敛 / 失败收敛的 `RunResult.subtype`（CLI 线格式口径）。
const SUBTYPE_SUCCESS: &str = "success";

/// loop 主体：灌事件至有界通道，收敛时发送 `RunResult`（停止 / 消费端
/// 关闭路径除外）。`model` 为 rig completion model（openai chat completions
/// 形态）；`history` 为续会话重建史（全新运行为空史）。
pub(crate) async fn run<M>(
    model: M,
    params: AgentRunParams,
    model_name: String,
    mut history: Vec<Message>,
    session_id: String,
    sender: mpsc::Sender<AgentEvent>,
    handle: RunHandle,
) where
    M: CompletionModel,
{
    let started = Instant::now();
    let mut seq: u64 = 0;
    let mut meta = RunMeta {
        started,
        usage: Usage::new(),
        session_id,
    };
    let definitions = tools::definitions();

    // run 启动事件（model / session / tools 口径与 CLI init 对齐；MCP 恒空）
    let started_ok = sender
        .send(AgentEvent::stamp(
            seq,
            AgentEventKind::RunStarted {
                model: Some(model_name.clone()),
                session_id: Some(meta.session_id.clone()),
                tools: tools::TOOL_NAMES
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
                mcp_servers: Vec::new(),
            },
        ))
        .await
        .is_ok();
    seq += 1;
    if !started_ok {
        return; // 消费端关闭：泵自行退出
    }
    // 用户提示词：时间线事件 + 入史（重建史尾追加新轮提示）
    let prompt_ok = sender
        .send(AgentEvent::stamp(
            seq,
            AgentEventKind::Message {
                role: "user".to_owned(),
                blocks: vec![AgentBlock::Text {
                    text: params.prompt.clone(),
                }],
                parent_tool_use_id: None,
            },
        ))
        .await
        .is_ok();
    seq += 1;
    if !prompt_ok {
        return;
    }
    history.push(Message::user(params.prompt.clone()));

    for turn in 1..=MAX_TURNS {
        // 轮间空档停止检查（泵 select 之外的快速路径）
        if handle.stop_requested() {
            return;
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
                    &mut seq,
                    turn,
                    &meta,
                    "api_error",
                    format!("API 请求失败: {error}"),
                )
                .await;
                return;
            }
        };
        // 流消费至 EOF（可恢复帧错误 drain 不中断，rig 契约：Err 非必然终局）
        let mut stream_error: Option<String> = None;
        while let Some(item) = response.next().await {
            match item {
                Ok(StreamedAssistantContent::Final(record)) => {
                    meta.usage += record.usage;
                }
                Ok(item) => {
                    if let Some(kind) = normalize::stream_item(&item) {
                        if sender.send(AgentEvent::stamp(seq, kind)).await.is_err() {
                            return; // 消费端关闭：泵自行退出，不合成 RunResult
                        }
                        seq += 1;
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
                &mut seq,
                turn,
                &meta,
                "api_error",
                format!("API 流失败: {error}"),
            )
            .await;
            return;
        }
        // 轮末聚合：choice 整体复用为 rig assistant 消息回灌
        let choice = std::mem::take(&mut response.choice);
        if choice.is_empty() {
            // 空轮（无文本无工具调用）：以正常收敛防死循环
            finish_success(&sender, &mut seq, turn, &meta).await;
            return;
        }
        let tool_calls: Vec<ToolCall> = choice
            .iter()
            .filter_map(|content| match content {
                AssistantContent::ToolCall(tool_call) => Some(tool_call.clone()),
                _ => None,
            })
            .collect();
        history.push(Message::Assistant {
            id: None,
            content: choice,
        });
        if tool_calls.is_empty() {
            finish_success(&sender, &mut seq, turn, &meta).await;
            return;
        }
        // 工具调用逐个：policy → sandbox → 执行 → 回灌（run 不中断）
        for tool_call in tool_calls {
            let id = tool_call.id.as_str().to_owned();
            let name = tool_call.function.name.clone();
            let input = tool_call.function.arguments.clone();
            if !policy::allows(params.permission_mode, &name) {
                let reason = format!("权限档位不允许该工具: {name}");
                deny(&sender, &mut seq, "permission_denied", &name, &reason, &id).await;
                history.push(Message::tool_result(id, name, reason));
                continue;
            }
            let sandboxed = match sandbox_rewrite(&params.cwd, &name, &input) {
                Ok(rewritten) => rewritten,
                Err(reason) => {
                    deny(&sender, &mut seq, "sandbox_denied", &name, &reason, &id).await;
                    history.push(Message::tool_result(id, name, reason));
                    continue;
                }
            };
            let (content, is_error) = match tools::execute(&params.cwd, &name, &sandboxed).await {
                Ok(text) => (text, false),
                Err(error) => (error, true),
            };
            if sender
                .send(AgentEvent::stamp(
                    seq,
                    AgentEventKind::Message {
                        role: "user".to_owned(),
                        blocks: vec![AgentBlock::ToolResult {
                            id: id.clone(),
                            content: content.clone(),
                            is_error,
                        }],
                        parent_tool_use_id: None,
                    },
                ))
                .await
                .is_err()
            {
                return; // 消费端关闭：泵自行退出
            }
            seq += 1;
            history.push(Message::tool_result(id, name, content));
        }
    }
    // 轮数熔断：以失败收敛（模型持续要求工具不收敛）
    finish_error(
        &sender,
        &mut seq,
        MAX_TURNS,
        &meta,
        "error_max_turns",
        format!("轮数达到上限 {MAX_TURNS} 仍未收敛"),
    )
    .await;
}

/// run 元信息束（收敛事件组装共享：起始时刻 / usage 累计 / 会话 id）。
struct RunMeta {
    started: Instant,
    usage: Usage,
    session_id: String,
}

/// 拒绝合成：`SystemNotice{subtype}` + is_error ToolResult（与 tool_use 同
/// id，成对回灌；run 不中断）。
async fn deny(
    sender: &mpsc::Sender<AgentEvent>,
    seq: &mut u64,
    subtype: &str,
    name: &str,
    reason: &str,
    id: &str,
) {
    let notice = AgentEventKind::SystemNotice {
        subtype: subtype.to_owned(),
        payload: serde_json::json!({ "tool": name, "reason": reason }),
    };
    if sender.send(AgentEvent::stamp(*seq, notice)).await.is_err() {
        return; // 消费端关闭：外层续轮的下一发自然退出
    }
    *seq += 1;
    let result = AgentEventKind::Message {
        role: "user".to_owned(),
        blocks: vec![AgentBlock::ToolResult {
            id: id.to_owned(),
            content: reason.to_owned(),
            is_error: true,
        }],
        parent_tool_use_id: None,
    };
    if sender.send(AgentEvent::stamp(*seq, result)).await.is_ok() {
        *seq += 1;
    }
}

/// sandbox 校验与入参改写：路径类工具（path 字段）经 [`sandbox::check`]
/// 解析后回写 `path`；glob 无 path 字段，以 [`sandbox::check_pattern`] 预检
/// pattern（执行体以 root 为基座拼接）。未知工具无路径面，原样放行（policy
/// 已先行拒绝封闭清单外的工具名）。
fn sandbox_rewrite(root: &Path, name: &str, input: &Value) -> Result<Value, String> {
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

/// 正常收敛：发送 `RunResult{is_error:false}`（usage 有值才填，缺失置 null）。
async fn finish_success(
    sender: &mpsc::Sender<AgentEvent>,
    seq: &mut u64,
    turns: u64,
    meta: &RunMeta,
) {
    finish(sender, seq, false, SUBTYPE_SUCCESS, Some(turns), meta).await;
}

/// 失败收敛：`SystemNotice{api_error}` 记因 + `RunResult{is_error:true}`
/// （`result_subtype` 区分成因：API 失败 `api_error`、轮数熔断
/// `error_max_turns`——CLI 合成收敛事件的命名口径）。
async fn finish_error(
    sender: &mpsc::Sender<AgentEvent>,
    seq: &mut u64,
    turns: u64,
    meta: &RunMeta,
    result_subtype: &str,
    cause: String,
) {
    let notice = AgentEventKind::SystemNotice {
        subtype: "api_error".to_owned(),
        payload: serde_json::json!({ "error": cause }),
    };
    if sender.send(AgentEvent::stamp(*seq, notice)).await.is_ok() {
        *seq += 1;
    }
    finish(sender, seq, true, result_subtype, Some(turns), meta).await;
}

/// `RunResult` 组装与发送（收敛事件恒为最后一事件；消费端关闭静默退出）。
async fn finish(
    sender: &mpsc::Sender<AgentEvent>,
    seq: &mut u64,
    is_error: bool,
    subtype: &str,
    turns: Option<u64>,
    meta: &RunMeta,
) {
    let kind = AgentEventKind::RunResult {
        subtype: subtype.to_owned(),
        is_error,
        num_turns: turns,
        duration_ms: Some(meta.started.elapsed().as_millis() as u64),
        cost_usd: None,
        usage: if meta.usage.has_values() {
            serde_json::to_value(meta.usage).unwrap_or(Value::Null)
        } else {
            Value::Null
        },
        session_id: Some(meta.session_id.clone()),
    };
    let _ = sender.send(AgentEvent::stamp(*seq, kind)).await;
    *seq += 1;
}
