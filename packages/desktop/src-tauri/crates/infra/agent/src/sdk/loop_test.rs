use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rig_core::completion::{CompletionError, CompletionModel, CompletionRequest, Usage};
use rig_core::message::{Message, ReasoningContent};
use rig_core::streaming::{
    RawStreamingChoice, RawStreamingToolCall, StreamFinal, StreamingCompletionResponse,
    StreamingResult,
};
use tokio::sync::mpsc;

use agent::{AgentDelta, AgentEventKind, AgentMessageRole, AgentPermissionMode, RunHandle};

use crate::sdk::r#loop::{self, LoopTurn};

// ---------------------------------------------------------------------------
// 装置：tempdir cwd、假流缝模型、loop 直驱回收
// ---------------------------------------------------------------------------

fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("sdk-loop-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建父目录失败");
    }
    std::fs::write(path, content).expect("写文件失败");
}

/// 假流缝模型：逐轮预录 rig 原始流项（内存流替身）；逐轮捕获
/// CompletionRequest（工具回灌断言缝）；可编程 stream Err。
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

fn raw_final(usage: Usage) -> RawStreamingChoice {
    RawStreamingChoice::FinalResponse(StreamFinal::new("fake-provider", usage))
}

/// loop 直驱：灌未盖戳事件至 EOF 并回收全部事件（通道容量与真实泵同口径）。
async fn drive_loop(
    model: FakeModel,
    turn: &LoopTurn,
    handle: &RunHandle,
) -> Vec<AgentEventKind> {
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let task = tokio::spawn({
        let turn = LoopTurn {
            question: turn.question.clone(),
            cwd: turn.cwd.clone(),
            permission_mode: turn.permission_mode,
            model_name: turn.model_name.clone(),
            session_id: turn.session_id.clone(),
        };
        let handle = handle.clone();
        async move { r#loop::run(&model, &turn, Vec::new(), sender, handle).await }
    });
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 任务正常结束");
    events
}

fn loop_turn(cwd: &Path, mode: AgentPermissionMode) -> LoopTurn {
    LoopTurn {
        question: "帮我看下这个 workspace 🎉".to_owned(),
        cwd: cwd.to_path_buf(),
        permission_mode: mode,
        model_name: "rig-test-model".to_owned(),
        session_id: "sdk-test-0".to_owned(),
    }
}

/// 摘密封 assistant Message 事件。
fn sealed_messages(events: &[AgentEventKind]) -> Vec<&AgentEventKind> {
    events
        .iter()
        .filter(|event| matches!(event, AgentEventKind::Message { role, .. } if role == &AgentMessageRole::Assistant))
        .collect()
}

/// 摘 SystemNotice (subtype, payload) 序列。
fn notices_of(events: &[AgentEventKind]) -> Vec<(&str, &serde_json::Value)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEventKind::SystemNotice { subtype, payload } => Some((subtype.as_str(), payload)),
            _ => None,
        })
        .collect()
}

/// 摘 TurnDone 事件（恒最后一事件）。
fn turn_done_of(events: &[AgentEventKind]) -> &AgentEventKind {
    let last = events.last().expect("事件流非空");
    assert!(
        matches!(last, AgentEventKind::TurnDone { .. }),
        "收敛事件恒为最后一事件，实际: {last:?}"
    );
    last
}

/// 全量 usage 底座（rig Usage 全字段面）。
fn usage(input: u64, output: u64) -> Usage {
    Usage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: input + output,
        cached_input_tokens: 0,
        cache_creation_input_tokens: 0,
        tool_use_prompt_tokens: 0,
        reasoning_tokens: 0,
    }
}

// ---------------------------------------------------------------------------
// 单轮密封收口（AC-1 落库半边 / AC-2 引擎半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 一轮文本加思考假流_delta逐发且轮末恰一条密封message两块收进() {
    let dir = tempdir("single-round");
    let model = FakeModel::with_turns(vec![vec![
        raw_text("正文结论 🎉"),
        raw_text("续行"),
        raw_reasoning("先读目录"),
        raw_final(usage(11, 22)),
    ]]);
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions), &handle).await;

    // 事件序：RunStarted → user 密封提示词 → delta 逐发（流序一致）→ 轮末密封
    // assistant → TurnDone
    assert!(matches!(&events[0], AgentEventKind::RunStarted { .. }));
    assert!(
        matches!(&events[1], AgentEventKind::Message { role, blocks, .. }
            if role == &AgentMessageRole::User && matches!(&blocks[0], agent::AgentBlock::Text { text } if text == "帮我看下这个 workspace 🎉")),
        "用户提示词密封入时间线"
    );
    let deltas: Vec<&AgentDelta> = events
        .iter()
        .filter_map(|event| match event {
            AgentEventKind::MessageDelta { delta, .. } => Some(delta),
            _ => None,
        })
        .collect();
    assert_eq!(
        deltas,
        vec![
            &AgentDelta::Text {
                text: "正文结论 🎉".to_owned()
            },
            &AgentDelta::Text {
                text: "续行".to_owned()
            },
            &AgentDelta::Thinking {
                thinking: "先读目录".to_owned()
            },
        ],
        "增量逐发且流序一致（text/thinking 可辨）"
    );

    // 轮末恰一条密封 Message：Text+Thinking 两块收进同一条、parentToolUseId=None
    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 1, "每 assistant 回应恰一条密封 Message");
    let AgentEventKind::Message {
        role,
        blocks,
        parent_tool_use_id,
    } = sealed[0]
    else {
        panic!("应为密封 Message");
    };
    assert_eq!(*role, AgentMessageRole::Assistant);
    assert_eq!(*parent_tool_use_id, None);
    assert_eq!(blocks.len(), 2, "Text+Thinking 全部块收进同一条");
    assert!(
        matches!(&blocks[0], agent::AgentBlock::Text { text } if text == "正文结论 🎉续行"),
        "轮末聚合：两枚文本增量聚合为单个 Text 块收进（choice 聚合语义），实际: {:?}",
        blocks[0]
    );
    assert!(matches!(
        &blocks[1],
        agent::AgentBlock::Thinking { thinking } if thinking == "先读目录"
    ));

    // 无工具调用 → 正常收敛
    turn_done_of(&events);
}

// ---------------------------------------------------------------------------
// 工具轮密封收口（两轮假流）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 两轮工具假流_密封收口_tool_use与tool_result与续轮各就位() {
    let dir = tempdir("tool-round");
    write_file(&dir.path().join("README.md"), "你好工作区");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_text("先读文件"),
            raw_tool_call(
                "tu_1",
                "read",
                serde_json::json!({ "path": "README.md" }),
            ),
            raw_final(usage(5, 6)),
        ],
        vec![
            raw_text("文件内容已确认"),
            raw_final(usage(7, 8)),
        ],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions), &handle).await;

    // 事件序：RunStarted → user 提示词 → delta → assistant 密封(ToolUse) →
    // ToolResult 密封(user) → delta → assistant 密封(文本) → TurnDone
    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 2, "两轮 assistant 回应各恰一条密封 Message");

    // 第一条密封收 ToolUse 块
    let AgentEventKind::Message { blocks, .. } = sealed[0] else {
        panic!("应为密封 Message");
    };
    assert!(matches!(
        &blocks[blocks.len() - 1],
        agent::AgentBlock::ToolUse { id, name, .. } if id == "tu_1" && name == "read"
    ), "ToolUse 块收进首轮密封，实际: {blocks:?}");

    // ToolResult 密封（tool 消息，内容为真实读文件产物）
    let tool_results: Vec<&AgentEventKind> = events
        .iter()
        .filter(|event| {
            matches!(event, AgentEventKind::Message { role, blocks, .. }
                if role == &AgentMessageRole::Tool && matches!(&blocks[0], agent::AgentBlock::ToolResult { .. }))
        })
        .collect();
    assert_eq!(tool_results.len(), 1, "ToolResult 密封恰一条");
    let AgentEventKind::Message { blocks, .. } = tool_results[0] else {
        panic!("应为 ToolResult 密封");
    };
    assert!(matches!(
        &blocks[0],
        agent::AgentBlock::ToolResult { id, content, is_error }
            if id == "tu_1" && content.contains("你好工作区") && !is_error
    ), "工具真实执行（tempdir 读写），实际: {blocks:?}");

    // 续轮密封 Message 收最终文本
    let AgentEventKind::Message { blocks, .. } = sealed[1] else {
        panic!("应为续轮密封");
    };
    assert!(matches!(
        &blocks[0],
        agent::AgentBlock::Text { text } if text == "文件内容已确认"
    ));

    // 收敛：numTurns 逐轮累计 = 2
    let AgentEventKind::TurnDone { num_turns, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert_eq!(*num_turns, Some(2), "numTurns 逐轮累计");
}

#[tokio::test]
async fn 工具轮第二请求的chat_history含成对tool_result且提示词在史首() {
    let dir = tempdir("tool-pair");
    write_file(&dir.path().join("a.txt"), "内容甲");
    let model = std::sync::Arc::new(FakeModel::with_turns(vec![
        vec![raw_tool_call("tu_2", "read", serde_json::json!({ "path": "a.txt" }))],
        vec![raw_text("完成")],
    ]));
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task = tokio::spawn(async move {
        r#loop::run(&*model_ref, &turn, Vec::new(), sender, handle).await
    });
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 2, "两轮请求");
    let second = &requests[1];
    assert!(
        second
            .chat_history
            .iter()
            .any(|message| matches!(message, Message::Assistant { content, .. }
                if content.iter().any(|item| matches!(item, rig_core::message::AssistantContent::ToolCall(tool_call) if tool_call.id.as_str() == "tu_2")))),
        "回灌史含 assistant ToolCall 消息"
    );
    assert!(
        second.chat_history.iter().any(|message| matches!(message, Message::User { content }
            if content.iter().any(|item| matches!(item, rig_core::message::UserContent::ToolResult(result) if result.name == "read")))),
        "回灌史含成对 ToolResult（工具名 read）"
    );
    // 提示词在史首
    assert!(matches!(
        second.chat_history.first(),
        Some(Message::User { content })
            if matches!(&content[0], rig_core::message::UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
    ));
}

// ---------------------------------------------------------------------------
// 重建史注入首轮请求（AC-3 引擎半边：Continue 装载缝闭合的组合断言）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 重建史注入首轮请求的chat_history尾部追加新轮提示词() {
    let dir = tempdir("resume-history");
    // 会话全史转录（上一轮往返）经 resume::rebuild 重建为 rig 对话史
    use agent::{AgentBlock, AgentEvent};
    let transcript = vec![
        AgentEvent::stamp(
            0,
            AgentEventKind::Message {
                role: AgentMessageRole::User,
                blocks: vec![AgentBlock::Text {
                    text: "上一轮问".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![AgentBlock::Text {
                    text: "上一轮答 🎉".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
    ];
    let history = crate::sdk::resume::rebuild(&transcript).expect("重建应成功");
    assert_eq!(history.len(), 2);

    let model = std::sync::Arc::new(FakeModel::with_turns(vec![vec![
        raw_text("本轮回应"),
        raw_final(usage(1, 1)),
    ]]));
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task = tokio::spawn(async move {
        r#loop::run(&*model_ref, &turn, history, sender, handle).await
    });
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1);
    let first = &requests[0];
    assert!(
        first.chat_history.iter().any(|message| matches!(message, Message::User { content }
            if matches!(&content[0], rig_core::message::UserContent::Text(text) if text.text == "上一轮问"))),
        "重建史首条（上轮提问）注入首轮请求"
    );
    assert!(
        first.chat_history.iter().any(|message| matches!(message, Message::Assistant { content, .. }
            if matches!(&content[0], rig_core::message::AssistantContent::Text(text) if text.text == "上一轮答 🎉"))),
        "重建史上轮回应注入首轮请求"
    );
    // 尾部追加新轮提示词（本轮提问在史尾）
    assert!(matches!(
        first.chat_history.last(),
        Some(Message::User { content })
            if matches!(&content[0], rig_core::message::UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
    ));
}

// ---------------------------------------------------------------------------
// TurnDone 组装唯一口径（AC-5）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn turn_done组装唯一口径_cost恒none_session_id收口_usage承接final() {
    let dir = tempdir("turn-done");
    let model = FakeModel::with_turns(vec![vec![
        raw_text("好"),
        raw_final(usage(11, 22)),
    ]]);
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions), &handle).await;

    let AgentEventKind::TurnDone {
        subtype,
        is_error,
        num_turns,
        duration_ms,
        cost_usd,
        usage: done_usage,
        session_id,
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success");
    assert!(!*is_error);
    assert_eq!(*num_turns, Some(1));
    assert!(duration_ms.is_some(), "durationMs 在场");
    assert_eq!(*cost_usd, None, "cost 恒 None（无价格表，缺席合法缺省）");
    assert_eq!(
        done_usage["input_tokens"], serde_json::json!(11),
        "usage 承接 Final（引擎无第二统计口径）"
    );
    assert_eq!(done_usage["output_tokens"], serde_json::json!(22));
    assert_eq!(
        session_id.as_deref(),
        Some("sdk-test-0"),
        "sessionId 收口（引擎侧标识上报）"
    );
}

// ---------------------------------------------------------------------------
// API 失败收敛
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 假流stream_err时api_error记因与is_error收敛且恒最后() {
    let dir = tempdir("api-error");
    let model = FakeModel::failing(CompletionError::ProviderError("连接被重置".to_owned()));
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions), &handle).await;

    // RunStarted + user 提示词先产出（失败前已入史），随后记因 + 收敛
    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1, "恰一条记因通知");
    assert_eq!(notices[0].0, "api_error");
    assert!(
        serde_json::to_value(notices[0].1).expect("payload 序列化")
            .to_string()
            .contains("连接被重置"),
        "记因携带流失败原因"
    );
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "api_error", "subtype 区分成因");
    assert!(*is_error, "API 失败收敛 is_error=true");
}

// ---------------------------------------------------------------------------
// 空轮 / 熔断语义
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 空流轮以正常收敛防死循环不悬挂() {
    let dir = tempdir("empty-round");
    let model = FakeModel::with_turns(vec![vec![]]); // 零流项 → choice 空
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions), &handle).await;

    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEventKind::Message { role, .. } if role == &AgentMessageRole::Assistant)),
        "空轮不出密封 assistant 消息"
    );
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success", "空轮以正常收敛防死循环");
    assert!(!*is_error);
}

#[tokio::test]
async fn 连续被拒工具轮耗尽上限时熔断收敛error_max_turns() {
    let dir = tempdir("max-turns");
    // default 档 write 工具恒被 policy 拒绝（无 IO，50 轮快速耗尽）
    let turns: Vec<Vec<RawStreamingChoice>> = (0..50)
        .map(|_| {
            vec![
                raw_tool_call("tu_x", "write", serde_json::json!({ "path": "x.txt", "content": "y" })),
                raw_final(usage(1, 1)),
            ]
        })
        .collect();
    let model = FakeModel::with_turns(turns);
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::Default), &handle).await;

    let notices = notices_of(&events);
    assert!(
        notices.len() == 51
            && notices[..50]
                .iter()
                .all(|(subtype, _)| *subtype == "permission_denied")
            && notices[50].0 == "api_error",
        "50 轮 permission_denied（无 IO 快速拒绝）+ 熔断记因一条，实际: {:?}",
        notices.iter().map(|(s, _)| *s).collect::<Vec<_>>()
    );
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "error_max_turns", "轮数熔断 subtype 区分成因");
    assert!(*is_error, "熔断以失败收敛");
}

// ---------------------------------------------------------------------------
// 停止 / 消费端关闭
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 停止先置位时泵终止不合成turn_done且已产出事件保留() {
    let dir = tempdir("stop-first");
    let model = FakeModel::with_turns(vec![vec![raw_text("不会到达的流")]]);
    let handle = RunHandle::default();
    handle.request_stop(); // 停止信号先置位

    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(16);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let task = tokio::spawn(async move {
        r#loop::run(&model, &turn, Vec::new(), sender, handle).await
    });
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 自行退出不悬挂");

    // 已产出事件原样保留：RunStarted + user 密封提示词
    assert!(matches!(&events[0], AgentEventKind::RunStarted { .. }));
    assert!(matches!(
        &events[1],
        AgentEventKind::Message { role, .. } if role == &AgentMessageRole::User
    ));
    assert!(
        !events.iter().any(|event| matches!(event, AgentEventKind::TurnDone { .. })),
        "停止路径不合成 TurnDone（为编排侧显式 stopped 收敛让路），实际: {events:?}"
    );
}

#[tokio::test]
async fn 消费端先行关闭时loop自行退出不悬挂且不合成收敛() {
    let dir = tempdir("consumer-gone");
    let model = FakeModel::with_turns(vec![
        vec![raw_text("第一片")],
        vec![raw_text("第二片")],
    ]);
    let (sender, receiver) = mpsc::channel::<AgentEventKind>(16);
    drop(receiver); // 接收端先行 drop（页面已关）
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let handle = RunHandle::default();

    // 自行退出：join 在有限时间内完成（悬挂则测试永不结束即失败）
    let history = tokio::spawn(async move {
        r#loop::run(&model, &turn, Vec::new(), sender, handle).await
    })
    .await
    .expect("loop 自行退出");

    // 返回累积史：消费端关闭先于任何事件送达（RunStarted 发送即失败），
    // loop 以未增史退出——不悬挂、不合成收敛
    assert_eq!(history.len(), 0, "消费端关闭即以空史退出（发送失败半边）");
}

// ---------------------------------------------------------------------------
// delta 占 seq 共享单调空间与洪峰背压
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delta与密封混合流按产出序排列且洪峰经容量256通道零丢失() {
    let dir = tempdir("flood");
    // 单轮 305 文本增量（> 通道容量 256）+ 密封 + 收敛
    let mut flood: Vec<RawStreamingChoice> = Vec::with_capacity(306);
    for index in 0..305 {
        flood.push(raw_text(&format!("片{index}")));
    }
    flood.push(raw_final(usage(305, 1)));
    let model = FakeModel::with_turns(vec![flood]);
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions), &handle).await;

    // 全量送达零丢失：RunStarted + user + 305 delta + 密封 + TurnDone
    assert_eq!(events.len(), 1 + 1 + 305 + 1 + 1, "洪峰全量送达零丢失");
    let deltas: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            AgentEventKind::MessageDelta {
                delta: AgentDelta::Text { text },
                ..
            } => Some(text.clone()),
            _ => None,
        })
        .collect();
    let expected: Vec<String> = (0..305).map(|index| format!("片{index}")).collect();
    assert_eq!(deltas, expected, "delta 按产出序全量保序（共享单调空间的产出半边）");
    // 密封在全部增量之后、TurnDone 收尾
    assert!(matches!(&events[307], AgentEventKind::Message { role, .. } if role == &AgentMessageRole::Assistant));
    assert!(matches!(events.last(), Some(AgentEventKind::TurnDone { .. })));
}

// ---------------------------------------------------------------------------
// 工具面零回归：policy 拒绝 / 沙箱拦截（真实组合不 mock）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn policy拒绝时permission_denied记因与is_error回灌续轮不中断() {
    let dir = tempdir("policy-deny");
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call("tu_3", "write", serde_json::json!({ "path": "x.txt", "content": "y" }))],
        vec![raw_text("已按拒绝继续")],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::Default), &handle).await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, "permission_denied");
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert_eq!(payload["tool"], serde_json::json!("write"));
    assert!(
        payload["reason"].as_str().is_some_and(|reason| reason.contains("权限档位")),
        "拒绝记因携带档位语义，实际: {payload}"
    );
    // is_error ToolResult 密封
    let has_error_result = events.iter().any(|event| {
        matches!(event, AgentEventKind::Message { role, blocks, .. }
            if role == &AgentMessageRole::Tool
                && matches!(&blocks[0], agent::AgentBlock::ToolResult { is_error: true, .. }))
    });
    assert!(has_error_result, "拒绝合成 is_error ToolResult 回灌");
    // 续轮不中断：第二请求产出密封 + 正常收敛
    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 2, "拒绝后续轮不中断（两 assistant 密封）");
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error, "拒绝不中断 run（正常收敛）");
}

#[tokio::test]
async fn 沙箱拦截时sandbox_denied记因与is_error回灌() {
    let dir = tempdir("sandbox-deny");
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_4",
            "read",
            serde_json::json!({ "path": "D:\\outside\\secret.txt" }),
        )],
        vec![raw_text("已按拦截继续")],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(model, &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions), &handle).await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, "sandbox_denied", "root 外路径沙箱拦截记因");
    let has_error_result = events.iter().any(|event| {
        matches!(event, AgentEventKind::Message { role, blocks, .. }
            if role == &AgentMessageRole::Tool
                && matches!(&blocks[0], agent::AgentBlock::ToolResult { is_error: true, .. }))
    });
    assert!(has_error_result, "拦截合成 is_error ToolResult 回灌");
    turn_done_of(&events);
}

// ---------------------------------------------------------------------------
// 编译期锚定：LoopTurn 字段面（协议轮参数的 loop 投影）
// ---------------------------------------------------------------------------

#[test]
fn loop_turn字段面完整构造锚定() {
    let turn = LoopTurn {
        question: "q".to_owned(),
        cwd: PathBuf::from("."),
        permission_mode: AgentPermissionMode::AcceptEdits,
        model_name: "m".to_owned(),
        session_id: "sdk-0-1".to_owned(),
    };
    assert_eq!(turn.question, "q");
    assert_eq!(turn.session_id, "sdk-0-1");
}
