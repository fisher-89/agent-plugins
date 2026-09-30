//! `SdkRunner` 与 sdk loop 的单元 + 组合测试（AC-4 / AC-5 / AC-6 / AC-7 /
//! AC-8 的 SDK 引擎半边）：启动校验（engine_cfg / resume）经真实
//! [`SdkRunner`]（空配置在启动校验即失败，不触 rig 网络）；假流缝全链
//! （loop → policy/sandbox/tools 回灌 → RunResult）经 loop 的 model 注入缝
//! 以内存流替身驱动（同 cli/runner_test 的 pump_lines 内存行流模式，不打
//! 网络）；停止 / 消费端关闭 / 事件洪峰背压经真实 runner 与 loop 装置锁定。

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rig_core::completion::{CompletionError, CompletionModel, CompletionRequest, Usage};
use rig_core::message::{Message, ReasoningContent, UserContent};
use rig_core::streaming::{
    RawStreamingChoice, RawStreamingToolCall, StreamFinal, StreamingCompletionResponse,
    StreamingResult,
};
use tokio::sync::mpsc;

use agent::{
    AgentBlock, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunParams, AgentRunner,
    AgentStartError, RunHandle,
};

use crate::ResumeTranscript;
use crate::sdk::config::EngineConfig;
use crate::sdk::{r#loop, runner::SdkRunner};

// ---------------------------------------------------------------------------
// 装置：tempdir cwd、假流缝模型、loop 直驱回收
// ---------------------------------------------------------------------------

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-runner-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建父目录失败");
    }
    std::fs::write(path, content).expect("写文件失败");
}

fn params(cwd: &Path) -> AgentRunParams {
    AgentRunParams {
        prompt: "帮我看下这个 workspace 🎉".to_owned(),
        cwd: cwd.to_path_buf(),
        permission_mode: AgentPermissionMode::BypassPermissions,
        resume_session_id: None,
    }
}

/// 假流缝模型：逐轮预录 rig 原始流项（内存流替身，同 pump_lines 的内存行流
/// 模式）；逐轮捕获 CompletionRequest（工具回灌断言缝）。
struct FakeModel {
    turns: Mutex<VecDeque<Vec<RawStreamingChoice>>>,
    requests: Mutex<Vec<CompletionRequest>>,
    stream_failure: Option<CompletionError>,
}

impl FakeModel {
    fn with_turns(turns: Vec<Vec<RawStreamingChoice>>) -> Self {
        Self {
            turns: Mutex::new(turns.into()),
            requests: Mutex::new(Vec::new()),
            stream_failure: None,
        }
    }

    fn failing(error: CompletionError) -> Self {
        Self {
            turns: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
            stream_failure: Some(error),
        }
    }

    fn captured_requests(&self) -> Vec<CompletionRequest> {
        self.requests.lock().expect("请求捕获锁不可中毒").clone()
    }
}

impl CompletionModel for FakeModel {
    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> Result<rig_core::completion::CompletionResponse, CompletionError> {
        Err(CompletionError::ProviderError(
            "unary 路径未被 loop 消费".to_owned(),
        ))
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse, CompletionError> {
        self.requests
            .lock()
            .expect("请求捕获锁不可中毒")
            .push(request);
        if let Some(error) = &self.stream_failure {
            return Err(CompletionError::ProviderError(error.to_string()));
        }
        let items = self
            .turns
            .lock()
            .expect("轮队列锁不可中毒")
            .pop_front()
            .unwrap_or_default();
        let raw: StreamingResult = Box::pin(futures::stream::iter(
            items.into_iter().map(Ok::<_, CompletionError>),
        ));
        Ok(StreamingCompletionResponse::stream("fake-provider", raw))
    }
}

// 原始流项 fixture 构造（rig 类型内存构造，不经 provider client）
fn raw_text(text: &str) -> RawStreamingChoice {
    RawStreamingChoice::Message(text.to_owned())
}

fn raw_reasoning(text: &str) -> RawStreamingChoice {
    RawStreamingChoice::Reasoning {
        id: "r-1".to_owned().into(),
        provider_id: None,
        content: ReasoningContent::Text {
            text: text.to_owned(),
            signature: None,
        },
    }
}

fn raw_tool_call(id: &str, name: &str, arguments: serde_json::Value) -> RawStreamingChoice {
    RawStreamingChoice::ToolCall(RawStreamingToolCall::new(
        id.to_owned(),
        name.to_owned(),
        arguments,
    ))
}

fn raw_final() -> RawStreamingChoice {
    RawStreamingChoice::FinalResponse(StreamFinal::new("fake-provider", Usage::new()))
}

/// loop 直驱：灌事件至 EOF 并回收全部事件（通道容量 256 与真实泵同策略）。
async fn drive_loop(
    model: FakeModel,
    params: AgentRunParams,
    handle: &RunHandle,
) -> Vec<AgentEvent> {
    let (sender, mut receiver) = mpsc::channel::<AgentEvent>(256);
    let task = tokio::spawn(r#loop::run(
        model,
        params,
        "rig-test-model".to_owned(),
        Vec::new(),
        "sdk-test-0".to_owned(),
        sender,
        handle.clone(),
    ));
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 任务正常结束");
    events
}

/// 摘 RunStarted 事件。
fn started_of(events: &[AgentEvent]) -> &AgentEvent {
    events
        .iter()
        .find(|event| matches!(event.kind, AgentEventKind::RunStarted { .. }))
        .expect("RunStarted 在场")
}

/// 摘 RunResult 事件。
fn result_of(events: &[AgentEvent]) -> &AgentEvent {
    events
        .iter()
        .find(|event| matches!(event.kind, AgentEventKind::RunResult { .. }))
        .expect("RunResult 在场")
}

/// 摘 SystemNotice 事件。
fn notices_of(events: &[AgentEvent]) -> Vec<(&str, &serde_json::Value)> {
    events
        .iter()
        .filter_map(|event| match &event.kind {
            AgentEventKind::SystemNotice { subtype, payload } => Some((subtype.as_str(), payload)),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 全链（无工具轮）：RunStarted 口径 + 文本/思考事件 + RunResult 收敛
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假流无工具轮全链_run_started口径与文本思考事件与run_result收敛() {
    let dir = tempdir("no-tool");
    let model = FakeModel::with_turns(vec![vec![
        raw_text("正文结论 🎉"),
        raw_reasoning("先读目录"),
        raw_final(),
    ]]);
    let handle = RunHandle::default();

    let events = drive_loop(model, params(dir.path()), &handle).await;

    // RunStarted：model / sdk- 前缀 session / 六工具名 / mcp_servers 空
    let AgentEventKind::RunStarted {
        model: started_model,
        session_id,
        tools,
        mcp_servers,
    } = &started_of(&events).kind
    else {
        panic!("RunStarted 必须在场");
    };
    assert_eq!(
        started_model.as_deref(),
        Some("rig-test-model"),
        "报 engine_cfg.model 口径"
    );
    assert_eq!(
        session_id.as_deref(),
        Some("sdk-test-0"),
        "sdk- 前缀会话 id（引擎归属标记）"
    );
    assert_eq!(
        tools.as_slice(),
        ["read", "grep", "glob", "ls", "write", "edit"].as_slice(),
        "tools 恒六工具名"
    );
    assert!(mcp_servers.is_empty(), "MCP 恒空");

    // 用户提示词事件（入史前先出事件）
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        AgentEventKind::Message { role, blocks, parent_tool_use_id: None }
            if role == "user" && matches!(blocks.first(), Some(AgentBlock::Text { .. }))
    )));

    // 文本 / 思考事件（assistant，块保真）
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        AgentEventKind::Message { role, blocks, .. }
            if role == "assistant" && matches!(blocks.first(), Some(AgentBlock::Text { text }) if text.contains("正文结论"))
    )));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        AgentEventKind::Message { role, blocks, .. }
            if role == "assistant" && matches!(blocks.first(), Some(AgentBlock::Thinking { thinking }) if thinking.contains("先读目录"))
    )));

    // RunResult 收敛：is_error=false、num_turns=1、cost_usd 恒 None、session id 收口
    let AgentEventKind::RunResult {
        is_error,
        num_turns,
        cost_usd,
        duration_ms,
        session_id: result_session,
        ..
    } = &result_of(&events).kind
    else {
        panic!("RunResult 必须在场");
    };
    assert!(!*is_error, "无工具轮正常收敛");
    assert_eq!(*num_turns, Some(1));
    assert_eq!(*cost_usd, None, "sdk 引擎 cost_usd 恒 None（CLI 线格式口径）");
    assert!(duration_ms.is_some(), "耗时入收敛事件");
    assert_eq!(result_session.as_deref(), Some("sdk-test-0"));
    // seq 单调：0..n 无跳号
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.seq, index as u64, "seq 单调无跳号");
    }
}

// ---------------------------------------------------------------------------
// 全链（工具轮回灌）：policy 允许 + 沙箱放行 → 执行 → 回灌续轮
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假流工具轮全链_允许路径执行tool_result回灌续轮num_turns为2() {
    let dir = tempdir("tool-loop");
    write_file(&dir.path().join("a.txt"), "文件内容 🎉");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call("tu_1", "read", serde_json::json!({ "path": "a.txt" })),
            raw_final(),
        ],
        vec![raw_text("读完收敛"), raw_final()],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(model, params(dir.path()), &handle).await;

    // 工具执行 ToolResult 事件（is_error=false，内容自文件读回）
    let tool_result = events.iter().find_map(|event| match &event.kind {
        AgentEventKind::Message { role, blocks, .. } if role == "user" => match blocks.first() {
            Some(AgentBlock::ToolResult {
                id,
                content,
                is_error,
            }) => Some((id.clone(), content.clone(), *is_error)),
            _ => None,
        },
        _ => None,
    });
    let (result_id, content, is_error) = tool_result.expect("工具执行 ToolResult 在场");
    assert!(!is_error, "执行成功结果非错误");
    assert!(content.contains("文件内容 🎉"), "读回内容保真: {content}");

    // 与 ToolUse 同 id 成对（同轮 assistant 事件先出）
    let tool_use_id = events.iter().find_map(|event| match &event.kind {
        AgentEventKind::Message { role, blocks, .. } if role == "assistant" => match blocks.first()
        {
            Some(AgentBlock::ToolUse { id, name, .. }) if name == "read" => Some(id.clone()),
            _ => None,
        },
        _ => None,
    });
    assert_eq!(tool_use_id.expect("ToolUse 在场"), result_id, "成对口径");

    // RunResult：两轮收敛
    let AgentEventKind::RunResult {
        is_error,
        num_turns,
        ..
    } = &result_of(&events).kind
    else {
        panic!("RunResult 必须在场");
    };
    assert!(!*is_error);
    assert_eq!(*num_turns, Some(2), "工具轮 + 续轮 = 2");
    assert!(notices_of(&events).is_empty(), "允许路径零 SystemNotice");
}

#[tokio::test]
async fn 工具轮回灌请求的chat_history含tool_result成对消息() {
    let dir = tempdir("replay-history");
    write_file(&dir.path().join("a.txt"), "内容");
    // Arc 承载：loop::run 经 rig 的 Arc<M> CompletionModel 转发消费，测试持
    // 同一 Arc 读捕获缝
    let model = Arc::new(FakeModel::with_turns(vec![
        vec![
            raw_tool_call("tu_9", "read", serde_json::json!({ "path": "a.txt" })),
            raw_final(),
        ],
        vec![raw_text("收敛"), raw_final()],
    ]));
    let handle = RunHandle::default();
    let (sender, mut receiver) = mpsc::channel::<AgentEvent>(256);
    let task = tokio::spawn(r#loop::run(
        model.clone(),
        params(dir.path()),
        "rig-test-model".to_owned(),
        Vec::new(),
        "sdk-test-0".to_owned(),
        sender,
        handle.clone(),
    ));
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 2, "两轮两次 stream 请求");
    let second = &requests[1];
    // 第二轮请求史：prompt user → assistant tool call → user tool result
    let tool_result_message = second.chat_history.iter().rev().find_map(|message| match message {
        Message::User { content } => content.iter().find_map(|item| match item {
            UserContent::ToolResult(result) => Some(result),
            _ => None,
        }),
        _ => None,
    });
    let result = tool_result_message.expect("第二轮请求史含回灌 ToolResult");
    assert!(
        result.call.as_str().contains("tu_9"),
        "回灌 ToolResult 与 tool use 同 id 成对: {}",
        result.call.as_str()
    );
    assert_eq!(result.name.as_str(), "read", "回灌结果登记执行工具名");
}

// ---------------------------------------------------------------------------
// 全链（policy 拒绝）：拒绝合成流出 run 不中断
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假流policy拒绝全链_system_notice_permission_denied与is_error回灌续轮不中断() {
    let dir = tempdir("policy-deny");
    write_file(&dir.path().join("target.txt"), "内容");
    let mut deny_params = params(dir.path());
    deny_params.permission_mode = AgentPermissionMode::Default; // 只读档：write 被拒

    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call(
                "tu_deny",
                "write",
                serde_json::json!({ "path": "target.txt", "content": "越权" }),
            ),
            raw_final(),
        ],
        vec![raw_text("收到拒绝，改用只读"), raw_final()],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(model, deny_params, &handle).await;

    // SystemNotice{permission_denied}（payload 携带工具名与拒绝原因）
    let notices = notices_of(&events);
    let (_, payload) = notices
        .iter()
        .find(|(subtype, _)| *subtype == "permission_denied")
        .expect("拒绝合成 SystemNotice 在场");
    assert_eq!(payload["tool"], serde_json::json!("write"));
    assert!(
        payload["reason"]
            .as_str()
            .expect("拒绝原因")
            .contains("权限档位不允许"),
        "实际: {payload}"
    );

    // is_error ToolResult（与 tool_use 同 id）回灌续轮
    let denied_result = events.iter().any(|event| matches!(
        &event.kind,
        AgentEventKind::Message { role, blocks, .. }
            if role == "user" && matches!(blocks.first(), Some(AgentBlock::ToolResult { is_error: true, .. }))
    ));
    assert!(denied_result, "is_error ToolResult 回灌在场");

    // run 不中断：正常收敛（is_error=false，两轮）
    let AgentEventKind::RunResult {
        is_error,
        num_turns,
        ..
    } = &result_of(&events).kind
    else {
        panic!("RunResult 必须在场");
    };
    assert!(!*is_error, "拒绝流出 run 不中断");
    assert_eq!(*num_turns, Some(2));
    // 越权写不得落盘
    assert_eq!(
        std::fs::read_to_string(dir.path().join("target.txt")).expect("读回"),
        "内容",
        "拒绝的 write 不得写盘"
    );
}

// ---------------------------------------------------------------------------
// 全链（沙箱拦截）：root 外路径 SystemNotice{sandbox_denied}
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假流沙箱拦截全链_root外路径system_notice_sandbox_denied与is_error回灌() {
    let dir = tempdir("sandbox-deny");
    let outside = tempdir("sandbox-deny-outside");
    let outside_file = outside.path().join("secret.txt");
    write_file(&outside_file, "外部机密");

    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call(
                "tu_sandbox",
                "read",
                serde_json::json!({ "path": outside_file.to_string_lossy() }),
            ),
            raw_final(),
        ],
        vec![raw_text("明白，只读内部文件"), raw_final()],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(model, params(dir.path()), &handle).await;

    // SystemNotice{sandbox_denied}
    let notices = notices_of(&events);
    let (_, payload) = notices
        .iter()
        .find(|(subtype, _)| *subtype == "sandbox_denied")
        .expect("沙箱拦截 SystemNotice 在场");
    assert!(
        payload["reason"]
            .as_str()
            .expect("原因")
            .contains("越出 workspace root"),
        "实际: {payload}"
    );

    // is_error ToolResult 回灌 + run 不中断
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        AgentEventKind::Message { role, blocks, .. }
            if role == "user" && matches!(blocks.first(), Some(AgentBlock::ToolResult { is_error: true, .. }))
    )));
    let AgentEventKind::RunResult { is_error, .. } = &result_of(&events).kind else {
        panic!("RunResult 必须在场");
    };
    assert!(!*is_error, "拦截流出 run 不中断");
}

// ---------------------------------------------------------------------------
// 全链（API 错误）：SystemNotice{api_error} + RunResult{is_error:true}
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假流api错误全链_system_notice_api_error与run_result_is_error收敛() {
    let dir = tempdir("api-error");
    let model = FakeModel::failing(CompletionError::ProviderError("连接端点失败".to_owned()));
    let handle = RunHandle::default();

    let events = drive_loop(model, params(dir.path()), &handle).await;

    // 失败通道：运行内 SystemNotice{api_error} 记因 + RunResult{is_error:true}
    let notices = notices_of(&events);
    let (_, payload) = notices
        .iter()
        .find(|(subtype, _)| *subtype == "api_error")
        .expect("api_error SystemNotice 在场");
    assert!(
        payload["error"]
            .as_str()
            .expect("记因")
            .contains("连接端点失败"),
        "实际: {payload}"
    );
    let AgentEventKind::RunResult {
        is_error, subtype, ..
    } = &result_of(&events).kind
    else {
        panic!("RunResult 必须在场");
    };
    assert!(*is_error, "运行内失败通道收敛");
    assert_eq!(
        subtype.as_str(),
        "api_error",
        "失败 subtype 与 CLI 合成命名口径一致"
    );
    // RunResult 恒为最后一事件
    assert!(
        matches!(events.last().expect("非空").kind, AgentEventKind::RunResult { .. }),
        "收敛事件恒为最后一事件"
    );
}

// ---------------------------------------------------------------------------
// 启动校验（engine_cfg / resume）：SdkRunner.start 显式失败，不产生 run 行
// ---------------------------------------------------------------------------

fn complete_config() -> EngineConfig {
    EngineConfig {
        api_key: "k".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model: "m".to_owned(),
    }
}

fn with_resume(resume_session_id: &str) -> AgentRunParams {
    let mut resume_params = params(Path::new("C:\\ws"));
    resume_params.resume_session_id = Some(resume_session_id.to_owned());
    resume_params
}

#[test]
fn 空缺省engine_cfg时start返回config_missing且零事件() {
    let runner = SdkRunner::new(EngineConfig::empty(), None);
    let error = runner
        .start(params(Path::new("C:\\ws")))
        .expect_err("空配置必须显式失败");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("单一中性变体承载，实际: {error:?}");
    };
    assert!(
        message.contains("api_key") && message.contains("base_url") && message.contains("model"),
        "消息区分缺失字段成因: {message}"
    );
    // 启动失败不产生任何事件（start Err 无事件通道可言）
    assert!(matches!(error, AgentStartError::ConfigMissing(_)));
}

#[test]
fn resume校验四形态均config_missing且消息区分成因() {
    // 非 sdk- 前缀：归属校验拒绝
    let runner = SdkRunner::new(complete_config(), None);
    let error = runner
        .start(with_resume("cli-not-mine"))
        .expect_err("非 sdk 前缀显式失败");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("实际: {error:?}");
    };
    assert!(
        message.contains("会话不存在或非 SDK 产出") && message.contains("cli-not-mine"),
        "实际: {message}"
    );

    // loader 返回 None：会话不存在或非 SDK 产出（loader 缺席同语义）
    let runner = SdkRunner::new(complete_config(), None);
    let error = runner.start(with_resume("sdk-404")).expect_err("loader 缺席显式失败");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("实际: {error:?}");
    };
    assert!(
        message.contains("转录装载器未注入") || message.contains("会话不存在"),
        "实际: {message}"
    );

    // loader 返回 Err：库读取失败
    let loader: ResumeTranscript = Arc::new(|_: &str| Err("redb 打开失败".to_owned()));
    let runner = SdkRunner::new(complete_config(), Some(loader));
    let error = runner
        .start(with_resume("sdk-err-1"))
        .expect_err("loader Err 显式失败");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("实际: {error:?}");
    };
    assert!(
        message.contains("转录读取失败") && message.contains("redb 打开失败"),
        "实际: {message}"
    );

    // loader 返回仅 Raw 转录（重建空历史）：视同会话缺失
    let loader: ResumeTranscript = Arc::new(|_: &str| {
        Ok(Some(vec![AgentEvent::stamp(
            0,
            AgentEventKind::Raw {
                event_type: "sdk_stream".to_owned(),
                raw_json: "{}".to_owned(),
            },
        )]))
    });
    let runner = SdkRunner::new(complete_config(), Some(loader));
    let error = runner
        .start(with_resume("sdk-empty-1"))
        .expect_err("重建空历史显式失败");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("实际: {error:?}");
    };
    assert!(message.contains("重建历史为空"), "实际: {message}");
}

// ---------------------------------------------------------------------------
// 停止与消费端关闭（泵任务终止语义）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 停止先置位时真实runner泵不合成run_result即终止且事件以已产出收尾() {
    // 停止信号前置位：泵 select 的 wait_requested 臂与 loop 臂竞速——loop 至多
    // 产出先导事件（RunStarted / 用户提示词，均在任何模型请求之前）即被终止，
    // 不发出任何模型请求（零网络）、不合成 RunResult（编排侧显式收敛 stopped）。
    // 竞速确定性断言：事件若产出仅限先导两类、恒无 RunResult、recv 以 EOF 收敛
    // （泵任务已终止的直接观测）。
    let dir = tempdir("pump-stop");
    let mut config = complete_config();
    config.base_url = "http://127.0.0.1:9/v1".to_owned(); // 构造成功即可，永不发请求
    let runner = SdkRunner::new(config, None);
    let mut run = runner.start(params(dir.path())).expect("完整配置 start 成功");

    run.handle.request_stop();

    let mut events = Vec::new();
    while let Some(event) = run.events.recv().await {
        events.push(event);
    }

    // 泵任务已终止：recv 自然 EOF（上文循环退出即证）；事件流以已产出事件收尾
    for event in &events {
        assert!(
            matches!(event.kind, AgentEventKind::RunStarted { .. })
                || matches!(
                    &event.kind,
                    AgentEventKind::Message { role, .. } if role == "user"
                ),
            "停止置位后仅先导事件可产出，实际: {event:?}"
        );
    }
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, AgentEventKind::RunResult { .. })),
        "停止路径泵不合成 RunResult（AC-8 停止半边）"
    );
    assert!(events.len() <= 2, "先导事件至多两条: {events:?}");
}

#[tokio::test]
async fn 消费端先行关闭时loop自行退出不悬挂且不合成run_result() {
    let dir = tempdir("consumer-closed");
    let model = FakeModel::with_turns(vec![vec![raw_text("正文"), raw_final()]]);
    let handle = RunHandle::default();
    let (sender, receiver) = mpsc::channel::<AgentEvent>(4);
    drop(receiver); // 消费端立即关闭

    let task = tokio::spawn(r#loop::run(
        model,
        params(dir.path()),
        "rig-test-model".to_owned(),
        Vec::new(),
        "sdk-test-0".to_owned(),
        sender,
        handle.clone(),
    ));

    // 泵自行退出不悬挂：JoinHandle 完成（发送失败路径直接 return，无死循环）
    tokio::task::yield_now().await;
    let mut spins = 0;
    while !task.is_finished() {
        tokio::task::yield_now().await;
        spins += 1;
        assert!(spins < 10_000, "消费端关闭后泵必须自行退出");
    }
    task.await.expect("泵正常退出");
}

#[tokio::test]
async fn 停止晚于流eof时按eof语义收敛且无二次收敛() {
    let dir = tempdir("stop-after-eof");
    let model = FakeModel::with_turns(vec![vec![raw_text("正文"), raw_final()]]);
    let handle = RunHandle::default();

    let events = drive_loop(model, params(dir.path()), &handle).await;

    // EOF 收敛已完成（RunResult 为最后一事件）
    assert!(
        matches!(events.last().expect("非空").kind, AgentEventKind::RunResult { .. }),
        "EOF 语义收敛（正常 RunResult 收尾）"
    );
    let results = events
        .iter()
        .filter(|event| matches!(event.kind, AgentEventKind::RunResult { .. }))
        .count();
    assert_eq!(results, 1, "恰一次收敛，无二次收敛");

    // EOF 后置位停止：loop future 已返回，无续发事件（句柄侧仅信号置位）
    handle.request_stop();
    assert_eq!(results, 1, "停止晚于 EOF 不产生二次收敛事件");
}

// ---------------------------------------------------------------------------
// 有界事件通道：容量 256 洪峰背压全量送达零丢失
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 事件洪峰经容量256有界通道背压全量送达零丢失() {
    let dir = tempdir("flood");
    // 洪峰：单轮 314 个文本流项（> 容量 256）
    let mut turn: Vec<RawStreamingChoice> = (0..314)
        .map(|index| raw_text(&format!("行{index}")))
        .collect();
    turn.push(raw_final());
    let model = FakeModel::with_turns(vec![turn]);
    let handle = RunHandle::default();
    let (sender, mut receiver) = mpsc::channel::<AgentEvent>(256);

    let task = tokio::spawn(r#loop::run(
        model,
        params(dir.path()),
        "rig-test-model".to_owned(),
        Vec::new(),
        "sdk-test-0".to_owned(),
        sender,
        handle.clone(),
    ));

    // 消费端同步排空：有界通道背压下生产端阻塞等消费，全量送达
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 正常结束");

    // RunStarted + 用户提示词 + 314 文本 + RunResult = 317，一条不丢
    assert_eq!(events.len(), 317, "洪峰全量送达零丢失");
    let text_events = events
        .iter()
        .filter(|event| matches!(
            &event.kind,
            AgentEventKind::Message { role, blocks, .. }
                if role == "assistant" && matches!(blocks.first(), Some(AgentBlock::Text { .. }))
        ))
        .count();
    assert_eq!(text_events, 314, "314 文本事件全数送达");
    // seq 全程单调 0..n
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.seq, index as u64, "第 {index} 条 seq 单调");
    }
    assert!(
        matches!(events.last().expect("非空").kind, AgentEventKind::RunResult { .. }),
        "洪峰后正常收敛"
    );
}

// ---------------------------------------------------------------------------
// 门面分发路径复核：runner_for(Sdk) 产物即 SdkRunner 契约实现（与 lib_test
// 的分发用例互为锚定，此处锁定 trait object 启动校验行为一致）
// ---------------------------------------------------------------------------

#[test]
fn 门面sdk分发的trait_object启动校验与直构runner一致() {
    use crate::EngineFacade;

    let facade = EngineFacade::new();
    let boxed: Box<dyn AgentRunner> =
        facade.runner_for(crate::EngineKind::Sdk, EngineConfig::empty());
    let error = boxed
        .start(params(Path::new("C:\\ws")))
        .expect_err("空配置显式失败");
    let direct = SdkRunner::new(EngineConfig::empty(), None)
        .start(params(Path::new("C:\\ws")))
        .expect_err("直构 runner 同样显式失败");
    assert_eq!(error, direct, "门面分发与直构启动校验行为一致");
}
