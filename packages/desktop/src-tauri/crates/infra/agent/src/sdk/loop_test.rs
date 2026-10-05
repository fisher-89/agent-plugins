use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rig::completion::{
    CompletionError, CompletionModel, CompletionRequest, CompletionResponse, Usage,
};
use rig::message::{AssistantContent, Message, ReasoningContent};
use rig::streaming::{
    RawStreamingChoice, RawStreamingToolCall, StreamFinal, StreamingCompletionResponse,
    StreamingResult,
};
use tokio::sync::mpsc;

use agent::{AgentDelta, AgentEventKind, AgentMessageRole, AgentPermissionMode, RunHandle};

use crate::sdk::context::ContextDefense;
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

/// 假流缝模型：逐轮预录 rig 原始流项（内存流替身）
struct FakeModel {
    turns: Mutex<VecDeque<Vec<RawStreamingChoice>>>,
    requests: Mutex<Vec<CompletionRequest>>,
    stream_failure: Option<CompletionError>,
    completion_outcomes: Mutex<VecDeque<Result<String, CompletionError>>>,
    rewrite_on_first_stream: Mutex<Option<(PathBuf, String)>>,
    hang: Option<HangPoint>,
}

/// 悬挂点：`Request` = `stream` 调用永不返回（响应头前死）；`Stream` = 流
/// 零帧且永不 EOF（帧间死）。
#[derive(Clone, Copy)]
enum HangPoint {
    Request,
    Stream,
}

impl FakeModel {
    fn with_turns(turns: Vec<Vec<RawStreamingChoice>>) -> Self {
        Self {
            turns: Mutex::new(turns.into()),
            requests: Mutex::new(Vec::new()),
            stream_failure: None,
            completion_outcomes: Mutex::new(VecDeque::new()),
            rewrite_on_first_stream: Mutex::new(None),
            hang: None,
        }
    }

    fn failing(error: CompletionError) -> Self {
        Self {
            turns: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
            stream_failure: Some(error),
            completion_outcomes: Mutex::new(VecDeque::new()),
            rewrite_on_first_stream: Mutex::new(None),
            hang: None,
        }
    }

    fn hanging(point: HangPoint) -> Self {
        Self {
            hang: Some(point),
            ..Self::with_turns(Vec::new())
        }
    }

    /// 两轮假流 + L3 摘要 completion 预编程序列（摘要成功文本 / Err 驱动降级）。
    fn with_turns_and_completions(
        turns: Vec<Vec<RawStreamingChoice>>,
        outcomes: Vec<Result<String, CompletionError>>,
    ) -> Self {
        let model = Self::with_turns(turns);
        *model.completion_outcomes.lock().expect("序列锁不可中毒") = outcomes.into();
        model
    }

    /// 首次 stream 请求时改写文件（模拟 agent 会话中自改 AGENT.md，下一轮
    /// preamble 重读即生效）。
    fn rewrite_file_on_first_stream(&mut self, path: PathBuf, content: &str) {
        *self.rewrite_on_first_stream.lock().expect("改写锁不可中毒") =
            Some((path, content.to_owned()));
    }

    fn captured_requests(&self) -> Vec<CompletionRequest> {
        self.requests.lock().expect("请求捕获锁不可中毒").clone()
    }
}

impl CompletionModel for FakeModel {
    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> Result<CompletionResponse, CompletionError> {
        match self
            .completion_outcomes
            .lock()
            .expect("序列锁不可中毒")
            .pop_front()
        {
            Some(Ok(text)) => Ok(CompletionResponse::new(
                vec![AssistantContent::text(text)],
                Usage::new(),
                "fake-summary-provider",
            )),
            Some(Err(error)) => Err(error),
            None => Err(CompletionError::ProviderError(
                "unary 路径未被 loop 消费".to_owned(),
            )),
        }
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse, CompletionError> {
        self.requests
            .lock()
            .expect("请求捕获锁不可中毒")
            .push(request);
        // 轮间改写钩子：首轮请求捕获后执行（次轮 preamble 重读见改后内容）
        if let Some((path, content)) = self
            .rewrite_on_first_stream
            .lock()
            .expect("改写锁不可中毒")
            .take()
        {
            std::fs::write(path, content).expect("轮间改写 AGENT.md 失败");
        }
        if let Some(error) = &self.stream_failure {
            return Err(CompletionError::ProviderError(error.to_string()));
        }
        // 悬挂装置：请求相位死 = 调用永不返回；流帧死 = 零帧且永不 EOF
        match self.hang {
            Some(HangPoint::Request) => return std::future::pending::<
                Result<StreamingCompletionResponse, CompletionError>,
            >()
            .await,
            Some(HangPoint::Stream) => {
                let raw: StreamingResult = Box::pin(futures::stream::pending());
                return Ok(StreamingCompletionResponse::stream("fake-provider", raw));
            }
            None => {}
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
async fn drive_loop(model: FakeModel, turn: &LoopTurn, handle: &RunHandle) -> Vec<AgentEventKind> {
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let task = tokio::spawn({
        let turn = LoopTurn {
            question: turn.question.clone(),
            cwd: turn.cwd.clone(),
            permission_mode: turn.permission_mode,
            model_name: turn.model_name.clone(),
            session_id: turn.session_id.clone(),
            defense: ContextDefense::resolve(None),
            liveness: turn.liveness,
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
        defense: ContextDefense::resolve(None),
        liveness: crate::sdk::r#loop::StreamLiveness::default(),
    }
}

/// 携小窗防线的轮参数（防线编排水位用例底座；window = 64 → L2 水位 48）。
fn loop_turn_with_defense(
    cwd: &Path,
    mode: AgentPermissionMode,
    defense: ContextDefense,
) -> LoopTurn {
    LoopTurn {
        defense,
        ..loop_turn(cwd, mode)
    }
}

/// 携毫秒级活性预算的轮参数（悬挂超时路径用例底座：不等待产品级分钟预算）。
fn loop_turn_with_fast_liveness(cwd: &Path, mode: AgentPermissionMode) -> LoopTurn {
    LoopTurn {
        liveness: crate::sdk::r#loop::StreamLiveness {
            request: std::time::Duration::from_millis(50),
            frame: std::time::Duration::from_millis(50),
        },
        ..loop_turn(cwd, mode)
    }
}

/// loop 直驱（携初始史）：防线编排断言缝——run() 直驱入口可注入 resume
/// 重建形态的超水位假史（工具轮回灌后的历史形状）。
async fn drive_loop_with_history(
    model: FakeModel,
    turn: LoopTurn,
    history: Vec<Message>,
) -> Vec<AgentEventKind> {
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let task =
        tokio::spawn(async move { r#loop::run(&model, &turn, history, sender, handle).await });
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 任务正常结束");
    events
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

/// 超水位假史基座（工具轮回灌后的形状）：[任务书 user, assistant(tool_use),
/// 超门槛老 tool_result（约 25k tokens）, 大块 filler user（约 50k tokens，
/// 使尾部越过 40k 保护窗）]。
fn oversized_seed_history() -> Vec<Message> {
    let call = rig::message::AssistantContent::tool_call(
        "tu_big",
        "read",
        serde_json::json!({ "path": "big.txt" }),
    );
    vec![
        Message::user("任务书"),
        Message::Assistant {
            id: None,
            content: vec![call],
        },
        Message::tool_result("tu_big", "read", "x".repeat(100_000)),
        Message::user("f".repeat(200_000)),
    ]
}

/// 摘 rig Message 史内指定 call id 的 tool_result 正文。
fn history_tool_result_text(history: &[Message], call: &str) -> Option<String> {
    history.iter().find_map(|message| match message {
        Message::User { content } => content.iter().find_map(|item| match item {
            rig::message::UserContent::ToolResult(result) if result.call.as_str() == call => {
                match &result.content[0] {
                    rig::message::ToolResultContent::Text(text) => Some(text.text.clone()),
                    _ => None,
                }
            }
            _ => None,
        }),
        _ => None,
    })
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

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

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
            raw_tool_call("tu_1", "read", serde_json::json!({ "path": "README.md" })),
            raw_final(usage(5, 6)),
        ],
        vec![raw_text("文件内容已确认"), raw_final(usage(7, 8))],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    // 事件序：RunStarted → user 提示词 → delta → assistant 密封(ToolUse) →
    // ToolResult 密封(user) → delta → assistant 密封(文本) → TurnDone
    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 2, "两轮 assistant 回应各恰一条密封 Message");

    // 第一条密封收 ToolUse 块
    let AgentEventKind::Message { blocks, .. } = sealed[0] else {
        panic!("应为密封 Message");
    };
    assert!(
        matches!(
            &blocks[blocks.len() - 1],
            agent::AgentBlock::ToolUse { id, name, .. } if id == "tu_1" && name == "read"
        ),
        "ToolUse 块收进首轮密封，实际: {blocks:?}"
    );

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
    assert!(
        matches!(
            &blocks[0],
            agent::AgentBlock::ToolResult { id, content, is_error }
                if id == "tu_1" && content.contains("你好工作区") && !is_error
        ),
        "工具真实执行（tempdir 读写），实际: {blocks:?}"
    );

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
        vec![raw_tool_call(
            "tu_2",
            "read",
            serde_json::json!({ "path": "a.txt" }),
        )],
        vec![raw_text("完成")],
    ]));
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task =
        tokio::spawn(
            async move { r#loop::run(&*model_ref, &turn, Vec::new(), sender, handle).await },
        );
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
                if content.iter().any(|item| matches!(item, rig::message::AssistantContent::ToolCall(tool_call) if tool_call.id.as_str() == "tu_2")))),
        "回灌史含 assistant ToolCall 消息"
    );
    assert!(
        second.chat_history.iter().any(|message| matches!(message, Message::User { content }
            if content.iter().any(|item| matches!(item, rig::message::UserContent::ToolResult(result) if result.name == "read")))),
        "回灌史含成对 ToolResult（工具名 read）"
    );
    // 提示词在史首
    assert!(matches!(
        second.chat_history.first(),
        Some(Message::User { content })
            if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
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
    let task =
        tokio::spawn(async move { r#loop::run(&*model_ref, &turn, history, sender, handle).await });
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1);
    let first = &requests[0];
    assert!(
        first.chat_history.iter().any(|message| matches!(message, Message::User { content }
            if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "上一轮问"))),
        "重建史首条（上轮提问）注入首轮请求"
    );
    assert!(
        first.chat_history.iter().any(|message| matches!(message, Message::Assistant { content, .. }
            if matches!(&content[0], rig::message::AssistantContent::Text(text) if text.text == "上一轮答 🎉"))),
        "重建史上轮回应注入首轮请求"
    );
    // 尾部追加新轮提示词（本轮提问在史尾）
    assert!(matches!(
        first.chat_history.last(),
        Some(Message::User { content })
            if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
    ));
}

// ---------------------------------------------------------------------------
// TurnDone 组装唯一口径（AC-5）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn turn_done组装唯一口径_cost恒none_session_id收口_usage承接final() {
    let dir = tempdir("turn-done");
    let model = FakeModel::with_turns(vec![vec![raw_text("好"), raw_final(usage(11, 22))]]);
    let handle = RunHandle::default();

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

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
        done_usage["input_tokens"],
        serde_json::json!(11),
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

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    // RunStarted + user 提示词先产出（失败前已入史），随后记因 + 收敛
    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1, "恰一条记因通知");
    assert_eq!(notices[0].0, "api_error");
    assert!(
        serde_json::to_value(notices[0].1)
            .expect("payload 序列化")
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
// 提供者 IO 活性护栏（悬挂 → 超时失败收敛，不再无限悬挂）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 请求相位悬挂时活性护栏超时失败收敛api_error不悬挂() {
    let dir = tempdir("hang-request");
    let model = FakeModel::hanging(HangPoint::Request); // stream 调用永不返回
    let handle = RunHandle::default();

    let events = drive_loop(
        model,
        &loop_turn_with_fast_liveness(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1, "恰一条记因通知");
    assert_eq!(notices[0].0, "api_error");
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert!(
        payload["error"]
            .as_str()
            .is_some_and(|cause| cause.contains("API 请求超时")),
        "记因指明请求相位超时，实际: {payload}"
    );
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "api_error", "悬挂以超时失败收敛（可观测）");
    assert!(*is_error);
}

#[tokio::test]
async fn 流帧空闲悬挂时活性护栏超时失败收敛api_error不悬挂() {
    let dir = tempdir("hang-stream");
    let model = FakeModel::hanging(HangPoint::Stream); // 零帧且永不 EOF
    let handle = RunHandle::default();

    let events = drive_loop(
        model,
        &loop_turn_with_fast_liveness(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1, "恰一条记因通知");
    assert_eq!(notices[0].0, "api_error");
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert!(
        payload["error"]
            .as_str()
            .is_some_and(|cause| cause.contains("流空闲超时")),
        "记因指明帧空闲超时，实际: {payload}"
    );
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "api_error", "流死悬挂以超时失败收敛（可观测）");
    assert!(*is_error);
}

// ---------------------------------------------------------------------------
// 空轮 / 熔断语义
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 空流轮以正常收敛防死循环不悬挂() {
    let dir = tempdir("empty-round");
    let model = FakeModel::with_turns(vec![vec![]]); // 零流项 → choice 空
    let handle = RunHandle::default();

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

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
                raw_tool_call(
                    "tu_x",
                    "write",
                    serde_json::json!({ "path": "x.txt", "content": "y" }),
                ),
                raw_final(usage(1, 1)),
            ]
        })
        .collect();
    let model = FakeModel::with_turns(turns);
    let handle = RunHandle::default();

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::Default),
        &handle,
    )
    .await;

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
    let task =
        tokio::spawn(async move { r#loop::run(&model, &turn, Vec::new(), sender, handle).await });
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
        !events
            .iter()
            .any(|event| matches!(event, AgentEventKind::TurnDone { .. })),
        "停止路径不合成 TurnDone（为编排侧显式 stopped 收敛让路），实际: {events:?}"
    );
}

#[tokio::test]
async fn 消费端先行关闭时loop自行退出不悬挂且不合成收敛() {
    let dir = tempdir("consumer-gone");
    let model = FakeModel::with_turns(vec![vec![raw_text("第一片")], vec![raw_text("第二片")]]);
    let (sender, receiver) = mpsc::channel::<AgentEventKind>(16);
    drop(receiver); // 接收端先行 drop（页面已关）
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let handle = RunHandle::default();

    // 自行退出：join 在有限时间内完成（悬挂则测试永不结束即失败）
    let history =
        tokio::spawn(async move { r#loop::run(&model, &turn, Vec::new(), sender, handle).await })
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

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

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
    assert_eq!(
        deltas, expected,
        "delta 按产出序全量保序（共享单调空间的产出半边）"
    );
    // 密封在全部增量之后、TurnDone 收尾
    assert!(
        matches!(&events[307], AgentEventKind::Message { role, .. } if role == &AgentMessageRole::Assistant)
    );
    assert!(matches!(
        events.last(),
        Some(AgentEventKind::TurnDone { .. })
    ));
}

// ---------------------------------------------------------------------------
// 工具面零回归：policy 拒绝 / 沙箱拦截（真实组合不 mock）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn policy拒绝时permission_denied记因与is_error回灌续轮不中断() {
    let dir = tempdir("policy-deny");
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_3",
            "write",
            serde_json::json!({ "path": "x.txt", "content": "y" }),
        )],
        vec![raw_text("已按拒绝继续")],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::Default),
        &handle,
    )
    .await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, "permission_denied");
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert_eq!(payload["tool"], serde_json::json!("write"));
    assert!(
        payload["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("权限档位")),
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

    let events = drive_loop(
        model,
        &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

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
        defense: ContextDefense::resolve(None),
        liveness: crate::sdk::r#loop::StreamLiveness::default(),
    };
    assert_eq!(turn.question, "q");
    assert_eq!(turn.session_id, "sdk-0-1");
}

// ---------------------------------------------------------------------------
// preamble 每轮重读接入请求（AC-2：loop 请求构造编排半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn preamble每轮重读接入请求_轮间改写agent_md次轮生效() {
    let dir = tempdir("preamble-reload");
    let agent_md = dir.path().join("AGENT.md");
    write_file(&agent_md, "第一版系统提示词");
    // 两轮假流：首轮以工具调用续轮（text-only 轮即收敛），首轮 stream 请求
    // 捕获后改写 AGENT.md（模拟 agent 会话中自改）
    let mut fake = FakeModel::with_turns(vec![
        vec![
            raw_tool_call("tu_pre", "read", serde_json::json!({ "path": "README.md" })),
            raw_final(usage(3, 3)),
        ],
        vec![raw_text("完成"), raw_final(usage(4, 4))],
    ]);
    fake.rewrite_file_on_first_stream(agent_md, "第二版系统提示词 🚀");
    write_file(&dir.path().join("README.md"), "占位内容");
    let model = std::sync::Arc::new(fake);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task =
        tokio::spawn(
            async move { r#loop::run(&*model_ref, &turn, Vec::new(), sender, handle).await },
        );
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 正常结束");

    // 首轮请求 preamble 逐字含文件内容；轮间改写 → 次轮请求 preamble 为改后内容
    let requests = model.captured_requests();
    assert_eq!(requests.len(), 2, "两轮请求（工具续轮）");
    assert_eq!(
        requests[0].preamble.as_deref(),
        Some("第一版系统提示词"),
        "首轮请求 preamble 逐字含 AGENT.md 内容"
    );
    assert_eq!(
        requests[1].preamble.as_deref(),
        Some("第二版系统提示词 🚀"),
        "轮间改写文件 → 次轮请求 preamble 为改后内容（每轮重读无缓存）"
    );
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error);
}

#[tokio::test]
async fn preamble缺席时请求preamble为none不注入空串() {
    let dir = tempdir("preamble-absent");
    let model = std::sync::Arc::new(FakeModel::with_turns(vec![vec![
        raw_text("好"),
        raw_final(usage(1, 1)),
    ]]));
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task =
        tokio::spawn(
            async move { r#loop::run(&*model_ref, &turn, Vec::new(), sender, handle).await },
        );
    while receiver.recv().await.is_some() {}
    let _history = task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].preamble, None,
        "root 无 AGENT.md → 请求 preamble 为 None（不注入空串）"
    );
}

#[tokio::test]
async fn preamble在场时请求preamble逐字含文件内容() {
    let dir = tempdir("preamble-capture");
    write_file(&dir.path().join("AGENT.md"), "# 约定 🎉\n- 提交信息用中文");
    let model = std::sync::Arc::new(FakeModel::with_turns(vec![vec![
        raw_text("好"),
        raw_final(usage(1, 1)),
    ]]));
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task =
        tokio::spawn(
            async move { r#loop::run(&*model_ref, &turn, Vec::new(), sender, handle).await },
        );
    while receiver.recv().await.is_some() {}
    let _history = task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(
        requests[0].preamble.as_deref(),
        Some("# 约定 🎉\n- 提交信息用中文"),
        "请求 preamble 逐字含文件内容（含中文 / emoji / 换行）"
    );
}

// ---------------------------------------------------------------------------
// L2 每请求前剪裁（loop 每请求前第二调用点，AC-6 / AC-8）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 小窗防线超水位假史_l2剪裁context_pruned流出且正常收敛() {
    let dir = tempdir("l2-prune");
    let model = FakeModel::with_turns(vec![vec![raw_text("完成"), raw_final(usage(2, 2))]]);
    // 窗长 65_536：L2 水位 49_152（假史约 75k 过线剪裁）、L3 水位 58_982
    // （剪裁后约 50k 不过线）——隔离 L2 单层可考
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(65_536)),
    );

    let events = drive_loop_with_history(model, turn, oversized_seed_history()).await;

    // 事件流出现 SystemNotice{context_pruned}，载荷 before / after / layer 齐
    let notices = notices_of(&events);
    let (subtype, payload) = notices
        .iter()
        .find(|(subtype, _)| *subtype == "context_pruned")
        .expect("剪裁 notice 流出");
    assert_eq!(*subtype, "context_pruned");
    let value = serde_json::to_value(payload).expect("payload 序列化");
    assert_eq!(value["layer"], serde_json::json!("l2"), "layer 记剪裁层");
    for key in ["before", "after"] {
        assert!(value.get(key).is_some(), "载荷含 {key}");
    }
    assert!(
        value["after"].as_u64() < value["before"].as_u64(),
        "剪裁有效减重"
    );

    // run 不受防线影响：正常收敛
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success");
    assert!(!*is_error);
}

#[tokio::test]
async fn l2剪裁后捕获请求的chat_history老工具结果为占位符() {
    let dir = tempdir("l2-capture");
    let model = std::sync::Arc::new(FakeModel::with_turns(vec![vec![
        raw_text("完成"),
        raw_final(usage(2, 2)),
    ]]));
    // 窗长 65_536：L2 过线剪裁、L3 不过线（水位推导见上一用例注释）
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(65_536)),
    );
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task = tokio::spawn(async move {
        r#loop::run(&*model_ref, &turn, oversized_seed_history(), sender, handle).await
    });
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1, "单轮单请求");
    let history = &requests[0].chat_history;
    // 首条 user（任务书）保全、新轮提示词在史尾
    assert!(matches!(
        history.first(),
        Some(Message::User { content })
            if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "任务书")
    ));
    // 老工具结果已被占位符替换（请求史而非 store 转录面收口）
    let placeholder = history_tool_result_text(history, "tu_big").expect("tool_result 配对保留");
    assert!(
        placeholder.contains("[已剪裁]"),
        "老工具结果为占位符: {placeholder}"
    );
    assert!(!placeholder.contains("xxxx"), "原正文已从请求史移除");
    // 新轮提示词殿后
    assert!(matches!(
        history.last(),
        Some(Message::User { content })
            if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
    ));
}

// ---------------------------------------------------------------------------
// L3 水位触发摘要编排（成功 + 失败降级，AC-7）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn l3水位触发摘要_下一请求史为摘要置顶加首条user加最近窗口形态() {
    let dir = tempdir("l3-compact");
    let model = FakeModel::with_turns_and_completions(
        vec![vec![raw_text("完成"), raw_final(usage(2, 2))]],
        vec![Ok("摘要要点".to_owned())],
    );
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(64)),
    );

    let events = drive_loop_with_history(model, turn, oversized_seed_history()).await;

    // 事件序：context_pruned → context_compacted → 密封 → 正常收敛
    let notices = notices_of(&events);
    let subtypes: Vec<&str> = notices.iter().map(|(s, _)| *s).collect();
    assert!(
        subtypes == vec!["context_pruned", "context_compacted"],
        "L2 + L3 双 notice 按序流出: {subtypes:?}"
    );
    let compacted_payload = serde_json::to_value(notices[1].1).expect("payload 序列化");
    assert_eq!(
        compacted_payload["layer"],
        serde_json::json!("l3"),
        "layer 记 L3"
    );

    // 收敛不受防线影响：正常 TurnDone
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success");
    assert!(!*is_error, "L3 成功路径 run 正常收敛");
}

#[tokio::test]
async fn l3摘要成功后请求史形态为摘要加首条加保护窗() {
    let dir = tempdir("l3-capture");
    let model = std::sync::Arc::new(FakeModel::with_turns_and_completions(
        vec![vec![raw_text("完成"), raw_final(usage(2, 2))]],
        vec![Ok("摘要要点".to_owned())],
    ));
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(64)),
    );
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task = tokio::spawn(async move {
        r#loop::run(&*model_ref, &turn, oversized_seed_history(), sender, handle).await
    });
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1, "摘要后即收敛（单请求）");
    let history = &requests[0].chat_history;
    assert_eq!(history.len(), 3, "[摘要, 首条 user, 最近窗口] 三段形态");
    assert!(
        matches!(
            &history[0],
            Message::User { content }
                if matches!(&content[0], rig::message::UserContent::Text(text) if text.text.starts_with("[历史摘要]"))
        ),
        "摘要消息置顶带 [历史摘要] 头"
    );
    assert!(
        matches!(
            &history[1],
            Message::User { content }
                if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "任务书")
        ),
        "原首条 user 保留"
    );
    assert!(
        history_tool_result_text(history, "tu_big").is_none(),
        "中段（含超大工具结果）已被摘要置换"
    );
}

#[tokio::test]
async fn l3摘要失败降级硬裁_notice携fallback且run不失败收敛() {
    let dir = tempdir("l3-fallback");
    let model = FakeModel::with_turns_and_completions(
        vec![vec![raw_text("完成"), raw_final(usage(2, 2))]],
        vec![
            Err(CompletionError::ProviderError("端点过载".to_owned())),
            Err(CompletionError::ProviderError("端点仍过载".to_owned())),
        ],
    );
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(64)),
    );

    let events = drive_loop_with_history(model, turn, oversized_seed_history()).await;

    // 事件序：context_pruned → context_compacted（fallback:true）→ 密封 → 正常收敛
    let notices = notices_of(&events);
    let subtypes: Vec<&str> = notices.iter().map(|(s, _)| *s).collect();
    assert!(
        subtypes == vec!["context_pruned", "context_compacted"],
        "防线双 notice 按序流出（降级不重复 pruned 痕）: {subtypes:?}"
    );
    let payload = serde_json::to_value(notices[1].1).expect("payload 序列化");
    assert_eq!(
        payload["layer"],
        serde_json::json!("l3"),
        "layer 记 L3 硬裁"
    );
    assert_eq!(payload["fallback"], serde_json::json!(true), "降级留痕");
    for key in ["before", "after"] {
        assert!(payload.get(key).is_some(), "载荷含 {key}");
    }

    // SystemNotice 为密封变体（delta 零落库纪律不破）
    let notices_events: Vec<&AgentEventKind> = events
        .iter()
        .filter(|event| matches!(event, AgentEventKind::SystemNotice { .. }))
        .collect();
    assert!(
        notices_events
            .iter()
            .all(|event| event.is_sealed() && !event.is_delta()),
        "防线 notice 全部密封词汇（无 delta 面）"
    );

    // 本轮仍以正常 TurnDone 收敛（不 api_error 不中断）
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success", "降级不改变收敛词汇");
    assert!(!*is_error, "run 不失败收敛");
}

#[tokio::test]
async fn l3降级硬裁后请求史为硬裁形态() {
    let dir = tempdir("l3-hard-capture");
    let model = std::sync::Arc::new(FakeModel::with_turns_and_completions(
        vec![vec![raw_text("完成"), raw_final(usage(2, 2))]],
        vec![
            Err(CompletionError::ProviderError("端点过载".to_owned())),
            Err(CompletionError::ProviderError("端点仍过载".to_owned())),
        ],
    ));
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(64)),
    );
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task = tokio::spawn(async move {
        r#loop::run(&*model_ref, &turn, oversized_seed_history(), sender, handle).await
    });
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    let history = &requests[0].chat_history;
    // 硬裁：保首条 user + 保护窗（尾部问题），中段整段丢弃
    assert_eq!(history.len(), 2, "[首条 user, 保护窗] 形态");
    assert!(matches!(
        &history[0],
        Message::User { content }
            if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "任务书")
    ));
    assert!(matches!(
        &history[1],
        Message::User { content }
            if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
    ));
    assert!(
        history_tool_result_text(history, "tu_big").is_none(),
        "中段工具对整段移除"
    );
}

// ---------------------------------------------------------------------------
// 未触水位：请求前防线零扰动（与既有用例事件序兼容）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 未触水位时请求史原样且零防线notice() {
    let dir = tempdir("no-defense");
    let model = std::sync::Arc::new(FakeModel::with_turns(vec![vec![
        raw_text("完成"),
        raw_final(usage(1, 1)),
    ]]));
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions); // 宽窗缺省防线
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let model_ref = std::sync::Arc::clone(&model);
    let task =
        tokio::spawn(
            async move { r#loop::run(&*model_ref, &turn, Vec::new(), sender, handle).await },
        );
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    let history = &requests[0].chat_history;
    assert_eq!(history.len(), 1, "低水位史零剪裁（仅新轮提示词）");
    assert!(matches!(
        &history[0],
        Message::User { content }
            if matches!(&content[0], rig::message::UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
    ));
}
