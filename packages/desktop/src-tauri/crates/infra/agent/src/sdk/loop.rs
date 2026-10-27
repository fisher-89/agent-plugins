use std::path::PathBuf;
use std::time::{Duration, Instant};

use agent::{AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode, RunHandle};
use futures::StreamExt;
use rig::completion::{CompletionRequest, FinishReason, Usage};
use rig::driver::DynModel;
use rig::message::{Message, ToolCall, ToolResult, ToolResultContent};
use rig::operation::Completion;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::sdk::archi_tool::ArchiContext;
use crate::sdk::context::{ContextDefense, DefenseNotice};
use crate::sdk::explore_note::ExploreNoteContext;
use crate::sdk::log::append_engine_log;
use crate::sdk::protect::ProtectRules;
use crate::sdk::{compact, context, normalize, policy, preamble, sandbox, tools, web_search};

/// 正常收敛 / 失败收敛的 `TurnDone.subtype`（core 协议收敛事件口径）。
const SUBTYPE_SUCCESS: &str = "success";

/// **单轮**续跑上限（`pause_turn` 续跑计数轴；与
/// `web_search::WEB_SEARCH_MAX_USES`（单请求搜索次数上限）是两个概念——
/// 常量名 / 注释 / 日志字段 MUST NOT 混用）。
const PAUSE_TURN_MAX_CONTINUATIONS: u32 = 5;

/// `pause_turn` 的 rig 终局标记（`FinishReason::Other("pause_turn")` 的原值）。
const PAUSE_TURN_REASON: &str = "pause_turn";

/// 提供者 IO 活性护栏
const REQUEST_LIVENESS: Duration = Duration::from_secs(300);
/// 流帧空闲预算分档宽于请求相位
const FRAME_IDLE: Duration = Duration::from_secs(600);

/// 提供者 IO 重试护栏
const STREAM_RETRY_MAX: u32 = 3;
const STREAM_RETRY_BACKOFF: Duration = Duration::from_secs(2);
const STREAM_RETRY_BACKOFF_STEP: u32 = 4;

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

/// 提供者 IO 失败重试预算（泵装配缺省，测试注小值）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct StreamRetry {
    /// 同轮重发上限（不含首发；耗尽以 api_error 失败收敛）
    pub(crate) max_attempts: u32,
    /// 退避基数（指数步进 ×4：2s → 8s → 32s）
    pub(crate) backoff_base: Duration,
}

impl Default for StreamRetry {
    fn default() -> Self {
        Self {
            max_attempts: STREAM_RETRY_MAX,
            backoff_base: STREAM_RETRY_BACKOFF,
        }
    }
}

impl StreamRetry {
    /// 第 n 次（1 起）重试的退避间隔（基数 × 4^(n-1)）。
    fn backoff(&self, attempt: u32) -> Duration {
        self.backoff_base * STREAM_RETRY_BACKOFF_STEP.saturating_pow(attempt - 1)
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
    /// 提供者 IO 失败重试预算（泵装配缺省，测试注小值）
    pub retry: StreamRetry,
    /// 会话级注入（definitions 组装唯一源：收窄子集 + Explore 枚举追加专用
    /// 工具；非 explore 来源恒 default）
    pub injections: agent::SessionInjections,
    /// `create_explore_note` 专用工具执行上下文（per-turn 铸造随轮携带；
    /// `None` = 未装配——非 explore 会话恒 None，专用工具调用以 is_error 回灌）
    pub explore_note: Option<ExploreNoteContext>,
    /// 架构只读工具执行上下文（per-turn 铸造随轮携带；`None` = 未装配——非
    /// Archi 会话恒 None，工具调用以 is_error 回灌）
    pub archi: Option<ArchiContext>,
    /// 写入保护规则集（泵起播时装配一次、随轮携带；loop 纯消费——与
    /// `defense` / `retry` / `liveness` 同列的「装配点算好」载荷）
    pub protect: ProtectRules,
    /// Anthropic 云端 `web_search` 服务端工具开关（**已按协议门控**的最终
    /// 值——门控在组合根解析期完成，loop 零协议判定）；真才挂 provider tool
    pub web_search: bool,
}

/// 系统消息合成单点：会话 preamble（产品知识，前）与本轮 `AGENT.md`（逐字，后）
/// 合为**恰一条** System 消息内容——两段之间以单空行分隔；仅一者在场时该条为其
/// 原文（`AGENT.md` 逐字语义零改动）；两者皆无 → `None`（不注入 System）。
///
/// `AGENT.md` 的 32KB 截断口径只作用于该文件自身（见 `preamble::load`），会话
/// preamble 不在该截断逻辑内，其体量由生产者侧测试锚定上限。
pub(crate) fn system_prompt(knowledge: Option<&str>, agent_md: Option<String>) -> Option<String> {
    match (knowledge, agent_md) {
        (Some(knowledge), Some(agent_md)) => Some(format!("{knowledge}\n\n{agent_md}")),
        (Some(knowledge), None) => Some(knowledge.to_owned()),
        (None, agent_md) => agent_md,
    }
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
    // 下发面单源：definitions 按会话注入面组装，RunStarted.tools 与成员校验
    // 自 definitions 本体派生（零第二份名单——定义 / 上报 / 校验三者天然一致）
    let definitions = tools::definitions(&turn.injections);
    let dispatched: Vec<String> = definitions.iter().map(|d| d.name.clone()).collect();

    // run 启动事件（core 协议 RunStarted 的 model / session / tools 口径；MCP 恒空）
    let started_ok = sender
        .send(AgentEventKind::RunStarted {
            model: Some(turn.model_name.clone()),
            session_id: Some(turn.session_id.clone()),
            tools: dispatched.clone(),
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
                citations: None,
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
    // 空轮收敛三线。每轮内含**续跑循环**（`pause_turn` 独立计数轴：续跑不
    // 计轮数、不发第二条 RunStarted、不吃流重试预算）
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
        // 续跑计数轴（与流重试轴 `attempt` 分名独立：每次续跑前 attempt 归零）
        let mut continuation: u32 = 0;
        // 续跑循环：同一 turn_index 内一发再发，直到收口或转为工具执行
        let tool_calls: Vec<ToolCall> = 'continuation: loop {
            // 上下文防线（每次发请求前，第二调用点）
            history = defend(model, history, &turn.defense, &turn.liveness, &sender).await;
            // 系统提示词每轮重读；合并次序 = 会话 preamble 在前、`AGENT.md` 逐字在后，单空行分隔，恰一条 System 消息
            let mut chat_history = Vec::with_capacity(history.len() + 1);
            if let Some(system) = system_prompt(
                turn.injections.preamble.as_deref(),
                preamble::load(&turn.cwd).await,
            ) {
                chat_history.push(Message::system(system));
            }
            chat_history.extend(history.iter().cloned());
            // 同轮重发装置（每次续跑前归零：续跑不继承重试预算）
            let mut attempt: u32 = 0;
            let (choice, turn_usage, finish_reason) = 'attempt: loop {
                let mut request = CompletionRequest {
                    model: None,
                    chat_history: chat_history.clone(),
                    documents: Vec::new(),
                    tools: definitions.clone(),
                    temperature: None,
                    max_tokens: None,
                    tool_choice: None,
                    additional_params: None,
                    output_schema: None,
                    record_telemetry_content: false,
                };
                // 服务端工具按需挂载（开关=组合根已门控的最终值；客户端工具面
                // `tools` 与 `RunStarted.tools` 不受影响——server tool 不进
                // 客户端工具面）
                if turn.web_search {
                    request = request.provider_tool(web_search::provider_tool());
                }
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

                let mut failure: Option<String> = None;
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
                            failure = Some(if first_frame {
                                format!(
                                    "API 请求超时（{} s 无响应帧）",
                                    turn.liveness.request.as_secs()
                                )
                            } else {
                                format!(
                                    "API 流空闲超时（{} s 无新帧）",
                                    turn.liveness.frame.as_secs()
                                )
                            });
                            break;
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
                            // 0.43 契约：Err 即流尾；已读帧不足以收口，按 IO 失败进重试
                            failure = Some(format!("API 流失败: {error}"));
                            break;
                        }
                    }
                }
                if failure.is_none() {
                    // 轮末收口：`finish` 把已读帧折叠为完整响应（choice 与 usage
                    // 单口径；0.43 起终局记录不再以独立流帧出现）；EOF 而折叠失
                    // 败同按 IO 失败进重试
                    match response.finish().await {
                        Ok(response) => {
                            // 终局原因须在解构前读（`finish_reason(&self)` 借用整体；
                            // 解构后 `choice` 已移出）
                            let finish_reason = response.finish_reason();
                            let rig::completion::CompletionResponse {
                                choice,
                                usage: attempt_usage,
                                ..
                            } = response;
                            break 'attempt (choice, attempt_usage, finish_reason);
                        }
                        Err(error) => {
                            failure = Some(format!("API 流失败: {error}"));
                        }
                    }
                }
                // 重试决策：预算内退避重发（同一轮、同一史；api_retry 留痕），
                // 耗尽以 api_error 失败收敛
                let cause = failure.expect("重试路径必有失败成因");
                if attempt >= turn.retry.max_attempts {
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
                attempt += 1;
                let delay = turn.retry.backoff(attempt);
                append_engine_log(&format!(
                    "loop 流重试 session={} turn_index={turn_index} attempt={attempt}/{} delay={}ms cause={cause}",
                    turn.session_id,
                    turn.retry.max_attempts,
                    delay.as_millis()
                ));
                // 重试留痕尽力流出（消费端关闭时外层续 attempt 的下一发自然退出）
                let _ = sender
                    .send(AgentEventKind::SystemNotice {
                        subtype: "api_retry".to_owned(),
                        payload: serde_json::json!({
                            "attempt": attempt,
                            "max_attempts": turn.retry.max_attempts,
                            "delay_ms": delay.as_millis() as u64,
                            "error": cause,
                        }),
                    })
                    .await;
                tokio::time::sleep(delay).await;
            };
            usage += turn_usage;
            if choice.is_empty() {
                // 空轮（无文本无工具调用）：以正常收敛防死循环；空 choice 不续跑
                finish_success(&sender, turn_index, started, &usage, turn).await;
                return history;
            }
            // 密封收口：choice 全部块收进恰一条密封 Message（role=assistant，
            // 增量层在传输面的 provisional 让位即同键替换）。块语义单点：
            // `web_search::project_choice`（server 块投影 + 客户端工具调用）
            let (blocks, tool_calls) = web_search::project_choice(&choice);
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
            // 工具执行优先收口（含客户端 tool_calls 时不续跑，续跑计数不增）
            if !tool_calls.is_empty() {
                break 'continuation tool_calls;
            }
            // 非 `pause_turn` 终局：本轮自然收敛
            if !is_pause_turn(finish_reason.as_ref()) {
                finish_success(&sender, turn_index, started, &usage, turn).await;
                return history;
            }
            // 续跑上限到顶：按 `stop` 语义正常收口（非 api_error）并留痕
            if continuation >= PAUSE_TURN_MAX_CONTINUATIONS {
                append_engine_log(&format!(
                    "loop pause_turn 续跑到顶 session={} turn_index={turn_index} continuations={PAUSE_TURN_MAX_CONTINUATIONS}",
                    turn.session_id
                ));
                let _ = sender
                    .send(AgentEventKind::SystemNotice {
                        subtype: "pause_turn_capped".to_owned(),
                        payload: serde_json::json!({
                            "continuations": PAUSE_TURN_MAX_CONTINUATIONS,
                        }),
                    })
                    .await;
                finish_success(&sender, turn_index, started, &usage, turn).await;
                return history;
            }
            // 续跑间隙停止检查（停后不留孤儿续跑）
            if handle.stop_requested() {
                append_engine_log(&format!(
                    "loop 停止截停（续跑间隙）session={} turn_index={turn_index}",
                    turn.session_id
                ));
                return history;
            }
            continuation += 1;
            // 过程唯一可观测点：每次续跑一条 notice（无搜索实时 delta）
            append_engine_log(&format!(
                "loop pause_turn 续跑 session={} turn_index={turn_index} continuations={continuation}",
                turn.session_id
            ));
            let _ = sender
                .send(AgentEventKind::SystemNotice {
                    subtype: "pause_turn_continued".to_owned(),
                    payload: serde_json::json!({ "continuations": continuation }),
                })
                .await;
        };
        // 工具调用逐个：成员校验 → policy 档位门 → 路径沙箱 → 写入保护闸 →
        // 执行 → 回灌（run 不中断）；0.43 起 tool_result 以 CallId / ToolName
        // 构造（与 tool_use 同源配对）。同一 assistant 的全部结果收进一条 user
        // 消息——provider 要求 tool_use 的下一条消息覆盖全部 tool_result。
        // 写入保护闸在沙箱**之后**：canonical 路径已在手（判定零 IO），且逃逸
        // 路径先按沙箱成因报错、不被保护闸抢报；保护闸在任何 permission_mode
        // 下均生效（档位门只做「档位是否放行该工具」，保护闸只做「档位已放行
        // 之后的追加拒绝」，三档决策表零改写）。`create_explore_note` 与两个
        // archi 工具均不在射程——专用工具由各自工具名分派表达，不做 loop 侧
        // 特判；两 archi 工具为读面：`input_path` 的显式 None 臂使沙箱原样返回，
        // `protect::evaluate` 分派表 `_ => None`，二者零介入
        let mut results: Vec<ToolResult> = Vec::with_capacity(tool_calls.len());
        for tool_call in tool_calls {
            let id = tool_call.id.to_string();
            let name = tool_call.function.name.as_str().to_owned();
            let input = tool_call.function.arguments.clone();
            // 成员校验（先于 policy）：调用名 ∉ 本轮实际下发面即拒绝——「从
            // definitions 移除」自此等于硬禁用，幻觉调用不直通执行、不触执行体
            if !dispatched.contains(&name) {
                let reason = format!("工具不在本轮下发面: {name}");
                deny(&sender, "tool_unavailable", &name, &reason, &id).await;
                results.push(outcome_result(tool_call, reason));
                continue;
            }
            if !policy::allows(turn.permission_mode, &name) {
                let reason = format!("权限档位不允许该工具: {name}");
                deny(&sender, "permission_denied", &name, &reason, &id).await;
                results.push(outcome_result(tool_call, reason));
                continue;
            }
            let sandboxed = match sandbox_rewrite(&turn.cwd, &name, &input) {
                Ok(rewritten) => rewritten,
                Err(reason) => {
                    deny(&sender, "sandbox_denied", &name, &reason, &id).await;
                    results.push(outcome_result(tool_call, reason));
                    continue;
                }
            };
            // 写入保护闸（守卫链第四段）：受保护文件的 write / edit / bash 写入
            // 面命中即拒。拒绝串单变量同时进 notice 载荷与 is_error ToolResult
            // （模型可见——`deny` 不中断 run，靠文案掐断换路重试链）
            if let Some(reason) = turn.protect.evaluate(&name, &sandboxed, &turn.cwd) {
                deny(&sender, "permission_denied", &name, &reason, &id).await;
                results.push(outcome_result(tool_call, reason));
                continue;
            }
            // 执行臂路由（三分派）：专用工具走 `LoopTurn` 携带的执行上下文
            // （per-turn 铸造——store 句柄 / 轮 root + 会话指针，`tools::execute`
            // 纯函数签名零侵入）；七通用工具走纯函数分发。上下文缺席以 is_error
            // 回灌（生产不可达——对应会话恒装配）
            let (content, is_error) = if name == tools::EXPLORE_NOTE_TOOL {
                match &turn.explore_note {
                    Some(ctx) => match ctx.execute(&input).await {
                        Ok(text) => (text, false),
                        Err(error) => (error, true),
                    },
                    None => ("专用工具执行上下文未装配".to_owned(), true),
                }
            } else if name == tools::ARCHI_MODULE_TOOL || name == tools::ARCHI_SEARCH_TOOL {
                match &turn.archi {
                    Some(ctx) => match ctx.execute(&name, &input).await {
                        Ok(text) => (text, false),
                        Err(error) => (error, true),
                    },
                    None => ("专用工具执行上下文未装配".to_owned(), true),
                }
            } else {
                match tools::execute(&turn.cwd, &name, &sandboxed).await {
                    Ok(text) => (text, false),
                    Err(error) => (error, true),
                }
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
            results.push(outcome_result(tool_call, content));
        }
        // 配对收口：全部结果收进恰一条 user 消息（tool_use 的下一条消息必须
        // 覆盖全部 tool_result，逐条消息会被 provider 判 400）
        if !results.is_empty() {
            history.push(Message::tool_results(results));
        }
        turn_index += 1;
    }
}

/// `pause_turn` 终局判据（精确等值：不做字符串包含匹配，防
/// `Other("pause_turn_x")` 误判；`None` 一律视为非 `pause_turn`）。
fn is_pause_turn(finish_reason: Option<&FinishReason>) -> bool {
    matches!(finish_reason, Some(FinishReason::Other(reason)) if reason == PAUSE_TURN_REASON)
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

/// 历史面结果条目：与 tool_use 同源（CallId / ToolName）配对，由调用点聚成
/// 单条 user 消息。
fn outcome_result(call: ToolCall, content: String) -> ToolResult {
    ToolResult {
        call: call.id,
        name: call.function.name,
        content: vec![ToolResultContent::text(content)],
    }
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
/// pattern（执行体以 root 为基座拼接）。其余工具无路径面，原样放行（成员
/// 校验已先行拒绝下发面外的调用名）。
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
