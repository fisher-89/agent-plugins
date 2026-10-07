use std::path::PathBuf;
use std::time::{Duration, Instant};

use agent::{AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode, RunHandle};
use futures::StreamExt;
use rig::completion::{CompletionRequest, Usage};
use rig::driver::DynModel;
use rig::message::{AssistantContent, Message, ToolCall};
use rig::operation::Completion;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::sdk::context::{ContextDefense, DefenseNotice};
use crate::sdk::log::append_engine_log;
use crate::sdk::{compact, context, normalize, policy, preamble, sandbox, tools};

/// 正常收敛 / 失败收敛的 `TurnDone.subtype`（core 协议收敛事件口径）。
const SUBTYPE_SUCCESS: &str = "success";

/// 提供者 IO 活性护栏
const REQUEST_LIVENESS: Duration = Duration::from_secs(300);
const FRAME_IDLE: Duration = Duration::from_secs(120);

/// 提供者 IO 活性预算
#[derive(Debug, Clone, Copy)]
pub(crate) struct StreamLiveness {
    /// 请求相位预算（`model.stream` 发起 + 响应头）
    pub(crate) request: Duration,
    /// 流帧空闲预算（相邻 SSE 帧间隔）
    pub(crate) frame: Duration,
}

impl Default for StreamLiveness {
    fn default() -> Self {
        Self {
            request: REQUEST_LIVENESS,
            frame: FRAME_IDLE,
        }
    }
}

/// 单轮驱动入参（协议轮参数的 loop 投影；引擎侧会话标识每轮铸造）。
pub(crate) struct LoopTurn {
    /// 轮提问
    pub question: String,
    /// 工作目录
    pub cwd: PathBuf,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
    /// 模型标识（RunStarted 上报口径）
    pub model_name: String,
    /// 引擎侧会话标识（每轮 `sdk-` 前缀铸造，经 RunStarted / TurnDone 上报，
    /// 内核写双 id 映射落库半边）
    pub session_id: String,
    /// 上下文窗防线（runner 侧解析缺省；引擎内部通道，core 契约零触）
    pub defense: ContextDefense,
    /// 提供者 IO 活性预算（缺省缺省常量；泵装配缺省，测试注小值）
    pub liveness: StreamLiveness,
}

/// loop 主体
pub(crate) async fn run(
    model: &DynModel<Completion>,
    turn: &LoopTurn,
    mut history: Vec<Message>,
    sender: mpsc::Sender<AgentEventKind>,
    handle: RunHandle,
) -> Vec<Message> {
    let started = Instant::now();
    let mut usage = Usage::default();
    let definitions = tools::definitions();

    // run 启动事件（core 协议 RunStarted 的 model / session / tools 口径；MCP 恒空）
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
        append_engine_log(&format!(
            "loop 事件通道关闭退出（RunStarted 发送）session={}",
            turn.session_id
        ));
        return history; // 消费端关闭：泵自行退出
    }
    // 用户提示词：密封入史（时间线事件 + 入史；重建史尾追加新轮提示）
    let prompt_ok = sender
        .send(AgentEventKind::Message {
            role: AgentMessageRole::User,
            blocks: vec![AgentBlock::Text {
                text: turn.question.clone(),
            }],
            parent_tool_use_id: None,
        })
        .await
        .is_ok();
    if !prompt_ok {
        append_engine_log(&format!(
            "loop 事件通道关闭退出（user 提示词发送）session={}",
            turn.session_id
        ));
        return history;
    }
    history.push(Message::user(turn.question.clone()));

    // 轮驱动主循环：不设轮数上限（change 任务重型 phase agent 的工具轮数
    // 远超手数启发，熔断误伤正常收敛）；失控防护交停止信号、活性护栏与
    // 空轮收敛三线
    let mut turn_index: u64 = 1;
    loop {
        // 轮间空档停止检查（泵 select 之外的快速路径）
        if handle.stop_requested() {
            append_engine_log(&format!(
                "loop 停止截停（轮间快速路径）session={} turn_index={turn_index}",
                turn.session_id
            ));
            return history;
        }
        // 上下文防线（每次发请求前，第二调用点）
        history = defend(model, history, &turn.defense, &turn.liveness, &sender).await;
        // 系统提示词每轮重读
        let mut chat_history = Vec::with_capacity(history.len() + 1);
        if let Some(preamble) = preamble::load(&turn.cwd).await {
            chat_history.push(Message::system(preamble));
        }
        chat_history.extend(history.iter().cloned());
        let request = CompletionRequest {
            model: None,
            chat_history,
            documents: Vec::new(),
            tools: definitions.clone(),
            temperature: None,
            max_tokens: None,
            tool_choice: None,
            additional_params: None,
            output_schema: None,
            record_telemetry_content: false,
        };
        // 0.43 起 `stream` 同步返回（编码错误即时 Err；请求发送推迟到首次轮
        // 询），请求相位活性护栏挪至首帧等待
        let mut response = match model.stream(request) {
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

        let mut stream_error: Option<String> = None;
        let mut first_frame = true;
        loop {
            let budget = if first_frame {
                turn.liveness.request
            } else {
                turn.liveness.frame
            };
            let item = match tokio::time::timeout(budget, response.next()).await {
                Ok(item) => item,
                Err(_) => {
                    let cause = if first_frame {
                        format!(
                            "API 请求超时（{} s 无响应帧）",
                            turn.liveness.request.as_secs()
                        )
                    } else {
                        format!(
                            "API 流空闲超时（{} s 无新帧）",
                            turn.liveness.frame.as_secs()
                        )
                    };
                    finish_error(
                        &sender,
                        turn_index,
                        started,
                        &usage,
                        turn,
                        "api_error",
                        cause,
                    )
                    .await;
                    return history;
                }
            };
            first_frame = false;
            let Some(item) = item else {
                break;
            };
            match item {
                Ok(item) => {
                    if let Some(kind) = normalize::stream_item(&item) {
                        if sender.send(kind).await.is_err() {
                            append_engine_log(&format!(
                                "loop 事件通道关闭退出（流帧发送）session={}",
                                turn.session_id
                            ));
                            return history; // 消费端关闭：泵自行退出，不合成 TurnDone
                        }
                    }
                }
                Err(error) => {
                    stream_error.get_or_insert_with(|| error.to_string());
                    break; // 0.43 契约：Err 即流尾
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
        // 轮末收口：`finish` 把已读帧折叠为完整响应（choice 与 usage 单口
        // 径；0.43 起终局记录不再以独立流帧出现）
        let rig::completion::CompletionResponse {
            choice,
            usage: turn_usage,
            ..
        } = match response.finish().await {
            Ok(response) => response,
            Err(error) => {
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
        };
        usage += turn_usage;
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
                // 0.43 起 reasoning 以 Sealed 承载（发行方才可读）：以自身发行
                // 方打开取可显示文本（text / summary 块按行拼接）
                AssistantContent::Reasoning(reasoning) => blocks.push(AgentBlock::Thinking {
                    thinking: reasoning
                        .open(reasoning.issuer())
                        .map(|value| value.display_text())
                        .unwrap_or_default(),
                }),
                AssistantContent::ToolCall(tool_call) => {
                    blocks.push(AgentBlock::ToolUse {
                        id: tool_call.id.to_string(),
                        name: tool_call.function.name.as_str().to_owned(),
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
                role: AgentMessageRole::Assistant,
                blocks,
                parent_tool_use_id: None,
            })
            .await
            .is_err()
        {
            append_engine_log(&format!(
                "loop 事件通道关闭退出（assistant 密封发送）session={} turn_index={turn_index}",
                turn.session_id
            ));
            return history; // 消费端关闭：泵自行退出
        }
        if tool_calls.is_empty() {
            finish_success(&sender, turn_index, started, &usage, turn).await;
            return history;
        }
        // 工具调用逐个：policy → sandbox → 执行 → 回灌（run 不中断）；0.43 起
        // tool_result 以 CallId / ToolName 构造（与 tool_use 同源配对）
        for tool_call in tool_calls {
            let id = tool_call.id.to_string();
            let name = tool_call.function.name.as_str().to_owned();
            let input = tool_call.function.arguments.clone();
            if !policy::allows(turn.permission_mode, &name) {
                let reason = format!("权限档位不允许该工具: {name}");
                deny(&sender, "permission_denied", &name, &reason, &id).await;
                history.push(Message::tool_result(
                    tool_call.id,
                    tool_call.function.name,
                    reason,
                ));
                continue;
            }
            let sandboxed = match sandbox_rewrite(&turn.cwd, &name, &input) {
                Ok(rewritten) => rewritten,
                Err(reason) => {
                    deny(&sender, "sandbox_denied", &name, &reason, &id).await;
                    history.push(Message::tool_result(
                        tool_call.id,
                        tool_call.function.name,
                        reason,
                    ));
                    continue;
                }
            };
            let (content, is_error) = match tools::execute(&turn.cwd, &name, &sandboxed).await {
                Ok(text) => (text, false),
                Err(error) => (error, true),
            };
            if sender
                .send(AgentEventKind::Message {
                    // 工具结果管道以 tool role 密封（引擎归一：rig 以 user 位
                    // 承载 result，进 core 前重标，core+ 不做引擎特判）
                    role: AgentMessageRole::Tool,
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
                append_engine_log(&format!(
                    "loop 事件通道关闭退出（tool 结果发送）session={} turn_index={turn_index}",
                    turn.session_id
                ));
                return history; // 消费端关闭：泵自行退出
            }
            history.push(Message::tool_result(
                tool_call.id,
                tool_call.function.name,
                content,
            ));
        }
        turn_index += 1;
    }
}

/// 拒绝合成：`SystemNotice{subtype}` + is_error ToolResult 密封 tool 消息
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
        role: AgentMessageRole::Tool,
        blocks: vec![AgentBlock::ToolResult {
            id: id.to_owned(),
            content: reason.to_owned(),
            is_error: true,
        }],
        parent_tool_use_id: None,
    };
    let _ = sender.send(result).await;
}

/// 上下文防线编排（loop 请求前调用点）：L2 prune → notice 转发 → 水位过
/// L3 触发比则 `compact::summarize`（成功发 `context_compacted`，失败/超时
/// 降级 `hard_prune` 并以 `fallback:true` 留痕），run 不失败收敛。notice
/// 消费端关闭时尽力流出（外层续轮的下一发自然退出）。摘要调用带请求相位
/// 活性预算（悬挂的摘要请求同样会挂起整轮）。
async fn defend(
    model: &DynModel<Completion>,
    history: Vec<Message>,
    defense: &ContextDefense,
    liveness: &StreamLiveness,
    sender: &mpsc::Sender<AgentEventKind>,
) -> Vec<Message> {
    let (pruned, notices) = context::prune(history, defense);
    for notice in &notices {
        forward_notice(sender, notice).await;
    }
    let current = pruned;
    if !defense.over_l3(&current) {
        return current;
    }
    let before = context::estimate_history(&current);
    match tokio::time::timeout(
        liveness.request,
        compact::summarize(model, &current, defense),
    )
    .await
    {
        Ok(Ok(compacted)) => {
            let after = context::estimate_history(&compacted);
            forward_notice(
                sender,
                &DefenseNotice {
                    subtype: "context_compacted".to_owned(),
                    payload: serde_json::json!({
                        "before": before,
                        "after": after,
                        "layer": "l3"
                    }),
                },
            )
            .await;
            compacted
        }
        Ok(Err(error)) => {
            append_engine_log(&format!("L3 摘要失败降级硬裁: {error}"));
            // 降级路径：摘要失败不打断 run，L2 硬裁收口（fallback 留痕在
            // hard_prune 的 notice 载荷内）
            let (hard, notice) = context::hard_prune(current, defense);
            forward_notice(sender, &notice).await;
            hard
        }
        Err(_) => {
            append_engine_log(&format!(
                "L3 摘要超时（{} s）降级硬裁",
                liveness.request.as_secs()
            ));
            let (hard, notice) = context::hard_prune(current, defense);
            forward_notice(sender, &notice).await;
            hard
        }
    }
}

/// 防线 notice 转发：`SystemNotice{subtype, payload}` 经密封通道流出落库
/// （开放词典 subtype，非 delta 事件）。
async fn forward_notice(sender: &mpsc::Sender<AgentEventKind>, notice: &DefenseNotice) {
    let _ = sender
        .send(AgentEventKind::SystemNotice {
            subtype: notice.subtype.clone(),
            payload: notice.payload.clone(),
        })
        .await;
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
/// （`result_subtype` 记成因，现行唯一成因 `api_error`——core 协议收敛事件
/// 的命名口径）。记因同落引擎异常日志（消费端已关时事件面失声，日志面保
/// 观测）。
async fn finish_error(
    sender: &mpsc::Sender<AgentEventKind>,
    turns: u64,
    started: Instant,
    usage: &Usage,
    turn: &LoopTurn,
    result_subtype: &str,
    cause: String,
) {
    append_engine_log(&format!(
        "loop 失败收敛 session={} subtype={result_subtype} turns={turns} cause={cause}",
        turn.session_id
    ));
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
        usage: if usage.is_reported() {
            serde_json::to_value(usage).unwrap_or(Value::Null)
        } else {
            Value::Null
        },
        session_id: Some(turn.session_id.clone()),
    };
    let _ = sender.send(kind).await;
}
