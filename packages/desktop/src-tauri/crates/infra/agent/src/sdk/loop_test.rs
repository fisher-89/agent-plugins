use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use rig::completion::{CompletionRequest, CompletionResponse, FinishReason, Usage};
use rig::driver::{DynModel, Exchange, Model, Opened, Opening, Transport};
use rig::error::ProviderError;
use rig::message::{AssistantContent, CallId, Message, ToolName, UserContent};
use rig::operation::{Completion, Finish};
use rig::test_utils::{mock_final, MockFrame, MockScript, MockStreamEvent};
use rig::wire::Mode;
use tokio::sync::mpsc;

use agent::{AgentDelta, AgentEventKind, AgentMessageRole, AgentPermissionMode, RunHandle};

use crate::sdk::context::ContextDefense;
use crate::sdk::r#loop::{self, LoopTurn};

// ---------------------------------------------------------------------------
// 装置：tempdir cwd、假流缝模型（MockScript wire + 脚本化 transport）、loop 直驱回收
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

/// 假流缝状态：逐轮预录流事件脚本 + unary 摘要结局序列 + 请求捕获 + 流起手
/// 失败 + 悬挂装置 + 轮间改写钩子（经 Arc 由模型句柄与 transport 克隆共享）。
struct FakeState {
    turns: Mutex<VecDeque<Vec<MockStreamEvent>>>,
    requests: Mutex<Vec<CompletionRequest>>,
    stream_failure: Option<String>,
    completion_outcomes: Mutex<VecDeque<Result<String, String>>>,
    rewrite_on_first_stream: Mutex<Option<(PathBuf, String)>>,
    hang: Option<HangPoint>,
    flaky: Mutex<VecDeque<StreamGlitch>>,
    /// 单点故障注入：第 N 次流请求（1 起）失败一次（续跑不吃重试预算用例）
    fail_at_request: Mutex<Option<(usize, String)>>,
}

/// 悬挂点：`Request` = 首帧永不到达（发送 / 响应头相位死）；`Stream` = 首帧
/// 之后无新帧且永不 EOF（帧间死）。
#[derive(Clone, Copy)]
enum HangPoint {
    Request,
    Stream,
}

/// 流故障单点注入（重试路径用例装置）：前 N 次流请求按脚本故障，弹空后
/// 走正常轮脚本——「首发故障，重发成功」形态。
enum StreamGlitch {
    /// 流起手即失败（Opened::failed——对应流 Err 路径）
    Fail(String),
    /// 悬挂（首帧永不到达 / 首帧后无新帧）
    Hang(HangPoint),
}

/// 悬挂 Opening 装置（恒悬挂与单点注入共享）。
fn hanging_opening(point: HangPoint) -> Opening<MockFrame> {
    match point {
        HangPoint::Request => Opening::new(std::future::pending()),
        HangPoint::Stream => Opening::ready(Opened::new(
            futures::stream::iter([Ok(MockFrame::Event(MockStreamEvent::Text(
                "首帧".to_owned(),
            )))])
            .chain(futures::stream::pending()),
        )),
    }
}

/// 假流缝模型句柄（断言面：请求捕获 / 钩子装配；`erased` 产物供 loop 消费）。
#[derive(Clone)]
struct FakeModel {
    state: Arc<FakeState>,
}

impl FakeModel {
    fn with_turns(turns: Vec<Vec<MockStreamEvent>>) -> Self {
        Self {
            state: Arc::new(FakeState {
                turns: Mutex::new(turns.into()),
                ..empty_state()
            }),
        }
    }

    fn failing(error: &str) -> Self {
        Self {
            state: Arc::new(FakeState {
                stream_failure: Some(error.to_owned()),
                ..empty_state()
            }),
        }
    }

    fn hanging(point: HangPoint) -> Self {
        Self {
            state: Arc::new(FakeState {
                hang: Some(point),
                ..empty_state()
            }),
        }
    }

    /// 两轮假流 + L3 摘要 unary 预编程序列（摘要成功文本 / Err 驱动降级）。
    fn with_turns_and_completions(
        turns: Vec<Vec<MockStreamEvent>>,
        outcomes: Vec<Result<String, String>>,
    ) -> Self {
        let model = Self::with_turns(turns);
        *model
            .state
            .completion_outcomes
            .lock()
            .expect("序列锁不可中毒") = outcomes.into();
        model
    }

    /// 首次流请求时改写文件（模拟 agent 会话中自改 AGENT.md，下一轮
    /// preamble 重读即生效）。
    fn rewrite_file_on_first_stream(&self, path: PathBuf, content: &str) {
        *self
            .state
            .rewrite_on_first_stream
            .lock()
            .expect("改写锁不可中毒") = Some((path, content.to_owned()));
    }

    /// 前 N 次流请求单点故障注入（重试路径用例：首发故障，重发走正常脚本）。
    fn flaky_first_streams(&self, glitches: Vec<StreamGlitch>) {
        *self.state.flaky.lock().expect("故障锁不可中毒") = glitches.into();
    }

    /// 第 N 次流请求（1 起）单点失败一次：弹空后走正常轮脚本——续跑不吃
    /// 重试预算用例的装置（故障落在续跑请求上，而非首发）。
    fn fail_at_stream_request(&self, index: usize, error: &str) {
        *self
            .state
            .fail_at_request
            .lock()
            .expect("单点故障锁不可中毒") = Some((index, error.to_owned()));
    }

    fn captured_requests(&self) -> Vec<CompletionRequest> {
        self.state
            .requests
            .lock()
            .expect("请求捕获锁不可中毒")
            .clone()
    }

    /// loop 消费面的擦除模型（MockScript wire + 同状态 transport，经真实
    /// driver 驱动——0.43 起 mock 面从模型 trait 挪到 transport 层）。
    fn erased(&self) -> DynModel<Completion> {
        Model::new(
            MockScript::default(),
            FakeTransport {
                state: Arc::clone(&self.state),
            },
        )
        .erase()
    }
}

/// 空基座状态（构造器 `..` 展开基）。
fn empty_state() -> FakeState {
    FakeState {
        turns: Mutex::new(VecDeque::new()),
        requests: Mutex::new(Vec::new()),
        stream_failure: None,
        completion_outcomes: Mutex::new(VecDeque::new()),
        rewrite_on_first_stream: Mutex::new(None),
        hang: None,
        flaky: Mutex::new(VecDeque::new()),
        fail_at_request: Mutex::new(None),
    }
}

/// 假流缝 transport：脚本化 [`MockScript`] wire transport——请求捕获、逐轮
/// 流事件脚本（缺终局帧时补缺省 FinalResponse，沿 0.42「EOF 即收口」语义）、
/// unary 摘要结局、流起手失败与两相位悬挂装置。
#[derive(Clone)]
struct FakeTransport {
    state: Arc<FakeState>,
}

impl Transport<MockScript> for FakeTransport {
    // 帧流恒 Ok（Err 面由 Opened::failed 承载）；Err 变体尺寸为 rig
    // ProviderError 自身形态，非本装置可裁
    #[allow(clippy::result_large_err)]
    fn send(&self, request: CompletionRequest, exchange: Exchange) -> Opening<MockFrame> {
        // 请求捕获仅流请求（沿旧装置口径：loop 断言面只看 stream 位；unary
        // 摘要请求归 compact_test 自己的捕获装置）
        if matches!(exchange.mode, Mode::Streaming) {
            self.state
                .requests
                .lock()
                .expect("请求捕获锁不可中毒")
                .push(request);
        }
        match exchange.mode {
            Mode::Unary => {
                match self
                    .state
                    .completion_outcomes
                    .lock()
                    .expect("序列锁不可中毒")
                    .pop_front()
                {
                    Some(Ok(text)) => Opening::ready(Opened::new(futures::stream::iter([Ok(
                        MockFrame::Response(Box::new(CompletionResponse::new(
                            vec![AssistantContent::text(text)],
                            Usage::default(),
                            "fake-summary-provider",
                            serde_json::Value::Null,
                        ))),
                    )]))),
                    Some(Err(message)) => {
                        Opening::ready(Opened::failed(ProviderError::Provider(message)))
                    }
                    None => Opening::ready(Opened::failed(ProviderError::Provider(
                        "unary 路径未被 loop 消费".to_owned(),
                    ))),
                }
            }
            Mode::Streaming => {
                // 单点故障注入（续跑不吃重试预算用例）：第 N 次流请求失败一次
                let request_index = self
                    .state
                    .requests
                    .lock()
                    .expect("请求捕获锁不可中毒")
                    .len();
                let fail_at = self
                    .state
                    .fail_at_request
                    .lock()
                    .expect("单点故障锁不可中毒")
                    .clone();
                if let Some((index, error)) = fail_at {
                    if request_index == index {
                        *self
                            .state
                            .fail_at_request
                            .lock()
                            .expect("单点故障锁不可中毒") = None;
                        return Opening::ready(Opened::failed(ProviderError::Provider(error)));
                    }
                }
                // 轮间改写钩子：首轮流请求捕获后执行（次轮 preamble 重读见改后内容）
                if let Some((path, content)) = self
                    .state
                    .rewrite_on_first_stream
                    .lock()
                    .expect("改写锁不可中毒")
                    .take()
                {
                    std::fs::write(path, content).expect("轮间改写 AGENT.md 失败");
                }
                // 单点故障注入优先于恒故障装置（首发故障，重发走正常脚本）
                match self.state.flaky.lock().expect("故障锁不可中毒").pop_front() {
                    Some(StreamGlitch::Fail(error)) => {
                        return Opening::ready(Opened::failed(ProviderError::Provider(error)));
                    }
                    Some(StreamGlitch::Hang(point)) => return hanging_opening(point),
                    None => {}
                }
                if let Some(error) = &self.state.stream_failure {
                    return Opening::ready(Opened::failed(ProviderError::Provider(error.clone())));
                }
                // 悬挂装置：请求相位死 = 首帧永不到达；流帧死 = 首帧后无新帧且永不 EOF
                if let Some(point) = self.state.hang {
                    return hanging_opening(point);
                }
                let mut items = self
                    .state
                    .turns
                    .lock()
                    .expect("轮队列锁不可中毒")
                    .pop_front()
                    .unwrap_or_default();
                // 隐式终局：脚本未排终局帧时补缺省 FinalResponse（provider 收
                // 口在场，finish() 可折叠完整响应）
                if !matches!(items.last(), Some(MockStreamEvent::FinalResponse(_))) {
                    items.push(MockStreamEvent::FinalResponse(mock_final(Usage::default())));
                }
                // 恒 Ok 帧流（Err 面由 Opened::failed 承载），显式标注 Err 类型
                // 免超大 Err 变体告警
                Opening::ready(Opened::new(futures::stream::iter(
                    items
                        .into_iter()
                        .map(|event| Ok::<_, ProviderError>(MockFrame::Event(event))),
                )))
            }
        }
    }
}

// 原始流项 fixture 构造（rig test-utils 内存构造，经真实 wire 解码出流事件）
fn raw_text(text: &str) -> MockStreamEvent {
    MockStreamEvent::Text(text.to_owned())
}

fn raw_reasoning(text: &str) -> MockStreamEvent {
    MockStreamEvent::ReasoningDelta {
        id: "r-1".to_owned(),
        reasoning: text.to_owned(),
    }
}

fn raw_tool_call(id: &str, name: &str, arguments: serde_json::Value) -> MockStreamEvent {
    MockStreamEvent::ToolCall {
        id: id.to_owned(),
        name: name.to_owned(),
        arguments,
        call_id: None,
    }
}

fn raw_final(usage: Usage) -> MockStreamEvent {
    MockStreamEvent::FinalResponse(mock_final(usage))
}

/// 携显式终局原因的终局帧（`pause_turn` 续跑用例的驱动面）。
fn raw_final_with_reason(reason: FinishReason, usage: Usage) -> MockStreamEvent {
    MockStreamEvent::FinalResponse(Finish {
        usage,
        reason: Some(reason),
        ..Finish::default()
    })
}

/// `pause_turn` 终局帧（provider 侧「服务端工具未完待续」的原值）。
fn raw_pause_turn(usage: Usage) -> MockStreamEvent {
    raw_final_with_reason(FinishReason::Other("pause_turn".to_owned()), usage)
}

/// 服务端 raw 块流项：空文本 + `anthropic_content`（rig 折叠为
/// `Text { text: "", additional_params }`——raw server 块的承载形态）。
fn raw_server_block(raw: serde_json::Value) -> MockStreamEvent {
    MockStreamEvent::TextStart {
        id: String::new(),
        additional_params: Some(serde_json::json!({ "anthropic_content": raw })),
    }
}

/// `server_tool_use` raw（use 侧）。
fn server_use_raw(use_id: &str, query: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "server_tool_use",
        "id": use_id,
        "name": "web_search",
        "input": { "query": query },
        "encrypted_content": "use 加密索引 🎉",
    })
}

/// `web_search_tool_result` raw（result 侧）。
fn server_result_raw(use_id: &str, urls: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "type": "web_search_tool_result",
        "tool_use_id": use_id,
        "content": urls
            .iter()
            .map(|url| serde_json::json!({ "type": "web_search_result", "url": url, "title": format!("标题 {url}") }))
            .collect::<Vec<serde_json::Value>>(),
        "encrypted_content": "result 加密索引",
    })
}

/// loop 直驱：灌未盖戳事件至 EOF 并回收全部事件（通道容量与真实泵同口径）。
async fn drive_loop(model: &FakeModel, turn: &LoopTurn, handle: &RunHandle) -> Vec<AgentEventKind> {
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let dyn_model = model.erased();
    let task = tokio::spawn({
        let turn = LoopTurn {
            question: turn.question.clone(),
            cwd: turn.cwd.clone(),
            permission_mode: turn.permission_mode,
            model_name: turn.model_name.clone(),
            session_id: turn.session_id.clone(),
            defense: ContextDefense::resolve(None),
            liveness: turn.liveness,
            retry: turn.retry,
            injections: turn.injections.clone(),
            explore_note: None,
            archi: None,
            protect: turn.protect.clone(),
            web_search: turn.web_search,
        };
        let handle = handle.clone();
        async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await }
    });
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 任务正常结束");
    events
}

/// loop 直驱（回传事件与最终史）：历史面断言缝（续跑 / 投影用例）。
async fn drive_loop_collecting_history(
    model: &FakeModel,
    turn: &LoopTurn,
    handle: &RunHandle,
) -> (Vec<AgentEventKind>, Vec<Message>) {
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let dyn_model = model.erased();
    let task = tokio::spawn({
        let turn = LoopTurn {
            question: turn.question.clone(),
            cwd: turn.cwd.clone(),
            permission_mode: turn.permission_mode,
            model_name: turn.model_name.clone(),
            session_id: turn.session_id.clone(),
            defense: ContextDefense::resolve(None),
            liveness: turn.liveness,
            retry: turn.retry,
            injections: turn.injections.clone(),
            explore_note: None,
            archi: None,
            protect: turn.protect.clone(),
            web_search: turn.web_search,
        };
        let handle = handle.clone();
        async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await }
    });
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let history = task.await.expect("loop 任务正常结束");
    (events, history)
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
        retry: crate::sdk::r#loop::StreamRetry::default(),
        injections: agent::SessionInjections::default(),
        explore_note: None,
        archi: None,
        protect: crate::sdk::protect::ProtectRules::load(None),
        web_search: false,
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

/// 携毫秒级 IO 护栏的轮参数（活性 + 重试预算；悬挂 / 重试路径用例底座：
/// 不等待产品级分钟预算与秒级退避）。
fn loop_turn_with_fast_io(cwd: &Path, mode: AgentPermissionMode) -> LoopTurn {
    LoopTurn {
        liveness: crate::sdk::r#loop::StreamLiveness {
            request: std::time::Duration::from_millis(50),
            frame: std::time::Duration::from_millis(50),
        },
        retry: crate::sdk::r#loop::StreamRetry {
            max_attempts: 3,
            backoff_base: std::time::Duration::from_millis(1),
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
    let dyn_model = model.erased();
    let task =
        tokio::spawn(async move { r#loop::run(&dyn_model, &turn, history, sender, handle).await });
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

/// 全量 usage 底座（rig Usage 全字段面，0.43 起字段为 Option 计数）。
fn usage(input: u64, output: u64) -> Usage {
    Usage {
        input_tokens: Some(input),
        output_tokens: Some(output),
        total_tokens: Some(input + output),
        cached_input_tokens: Some(0),
        cache_creation_input_tokens: Some(0),
        tool_use_prompt_tokens: Some(0),
        reasoning_tokens: Some(0),
    }
}

/// 超水位假史基座（工具轮回灌后的形状）：[任务书 user, assistant(tool_use),
/// 超门槛老 tool_result（约 25k tokens）, 大块 filler user（约 50k tokens，
/// 使尾部越过 40k 保护窗）]。
fn oversized_seed_history() -> Vec<Message> {
    let call = AssistantContent::tool_call(
        "tu_big",
        ToolName::new("read").expect("工具名非空"),
        serde_json::json!({ "path": "big.txt" }),
    );
    vec![
        Message::user("任务书"),
        Message::Assistant {
            id: None,
            content: vec![call],
        },
        Message::tool_result(
            CallId::from_wire("tu_big"),
            ToolName::new("read").expect("工具名非空"),
            "x".repeat(100_000),
        ),
        Message::user("f".repeat(200_000)),
    ]
}

/// 摘 rig Message 史内指定 call id 的 tool_result 正文。
fn history_tool_result_text(history: &[Message], call: &str) -> Option<String> {
    history.iter().find_map(|message| match message {
        Message::User { content } => content.iter().find_map(|item| match item {
            UserContent::ToolResult(result) if result.call.wire().as_ref() == call => {
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
        &model,
        &loop_turn(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    // 事件序：RunStarted → user 密封提示词 → delta 逐发（流序一致）→ 轮末密封
    // assistant → TurnDone
    assert!(matches!(&events[0], AgentEventKind::RunStarted { .. }));
    assert!(
        matches!(&events[1], AgentEventKind::Message { role, blocks, .. }
            if role == &AgentMessageRole::User && matches!(&blocks[0], agent::AgentBlock::Text { text, .. } if text == "帮我看下这个 workspace 🎉")),
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
        matches!(&blocks[0], agent::AgentBlock::Text { text, .. } if text == "正文结论 🎉续行"),
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
        &model,
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
        agent::AgentBlock::Text { text, .. } if text == "文件内容已确认"
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
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_2",
            "read",
            serde_json::json!({ "path": "a.txt" }),
        )],
        vec![raw_text("完成")],
    ]);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
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
                if content.iter().any(|item| matches!(item, AssistantContent::ToolCall(tool_call) if tool_call.id.wire().as_ref() == "tu_2")))),
        "回灌史含 assistant ToolCall 消息"
    );
    assert!(
        second.chat_history.iter().any(|message| matches!(message, Message::User { content }
            if content.iter().any(|item| matches!(item, UserContent::ToolResult(result) if result.name.as_str() == "read")))),
        "回灌史含成对 ToolResult（工具名 read）"
    );
    // 提示词在史首
    assert!(matches!(
        second.chat_history.first(),
        Some(Message::User { content })
            if matches!(&content[0], UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
    ));
}

#[tokio::test]
async fn 同一assistant多工具调用的第二请求将全部结果收进同一条user消息() {
    let dir = tempdir("tool-pair-multi");
    write_file(&dir.path().join("a.txt"), "内容甲");
    write_file(&dir.path().join("b.txt"), "内容乙");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call("tu_a", "read", serde_json::json!({ "path": "a.txt" })),
            raw_tool_call("tu_b", "read", serde_json::json!({ "path": "b.txt" })),
        ],
        vec![raw_text("完成")],
    ]);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
        );
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    let second = &requests[1];
    // 含两条 tool_use 的 assistant 消息下标
    let assistant_index = second
        .chat_history
        .iter()
        .position(|message| {
            matches!(message, Message::Assistant { content, .. }
                if content.iter().filter(|item| matches!(item, AssistantContent::ToolCall(_))).count() == 2)
        })
        .expect("请求史含两工具调用的 assistant 消息");
    assert_eq!(
        second.chat_history.len(),
        assistant_index + 2,
        "结果恰占紧随 assistant 的一条消息（不拆成多条）"
    );
    let Message::User { content } = &second.chat_history[assistant_index + 1] else {
        panic!("tool_use 下一条应为承载全部结果的 user 消息");
    };
    let ids: Vec<String> = content
        .iter()
        .filter_map(|item| match item {
            UserContent::ToolResult(result) => Some(result.call.wire().as_ref().to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(
        ids,
        vec!["tu_a".to_owned(), "tu_b".to_owned()],
        "全部 tool_result 同条消息且保序，实际: {content:?}"
    );
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
                    citations: None,
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
                    citations: None,
                }],
                parent_tool_use_id: None,
            },
        ),
    ];
    let history = crate::sdk::resume::rebuild(&transcript).expect("重建应成功");
    assert_eq!(history.len(), 2);

    let model = FakeModel::with_turns(vec![vec![raw_text("本轮回应"), raw_final(usage(1, 1))]]);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task =
        tokio::spawn(async move { r#loop::run(&dyn_model, &turn, history, sender, handle).await });
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1);
    let first = &requests[0];
    assert!(
        first
            .chat_history
            .iter()
            .any(|message| matches!(message, Message::User { content }
            if matches!(&content[0], UserContent::Text(text) if text.text == "上一轮问"))),
        "重建史首条（上轮提问）注入首轮请求"
    );
    assert!(
        first.chat_history.iter().any(
            |message| matches!(message, Message::Assistant { content, .. }
            if matches!(&content[0], AssistantContent::Text(text) if text.text == "上一轮答 🎉"))
        ),
        "重建史上轮回应注入首轮请求"
    );
    // 尾部追加新轮提示词（本轮提问在史尾）
    assert!(matches!(
        first.chat_history.last(),
        Some(Message::User { content })
            if matches!(&content[0], UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
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
        &model,
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
        "usage 承接终局记录（引擎无第二统计口径）"
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
async fn 假流stream_err时重试耗尽后api_error记因与is_error收敛且恒最后() {
    let dir = tempdir("api-error");
    let model = FakeModel::failing("连接被重置");
    let handle = RunHandle::default();

    let events = drive_loop(
        &model,
        &loop_turn_with_fast_io(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    // RunStarted + user 提示词先产出（失败前已入史），随后 3 次重试留痕 +
    // 记因 + 收敛（恒失败装置：首发与全部重试都失败）
    let notices = notices_of(&events);
    let subtypes: Vec<&str> = notices.iter().map(|(s, _)| *s).collect();
    assert!(
        subtypes == vec!["api_retry", "api_retry", "api_retry", "api_error"],
        "重试逐次留痕后耗尽失败收敛: {subtypes:?}"
    );
    assert_eq!(
        model.captured_requests().len(),
        4,
        "首发 + 3 次重试共 4 次请求"
    );
    assert!(
        serde_json::to_value(notices[3].1)
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
async fn 请求相位悬挂时重试耗尽后超时失败收敛api_error不悬挂() {
    let dir = tempdir("hang-request");
    let model = FakeModel::hanging(HangPoint::Request); // 首帧永不到达
    let handle = RunHandle::default();

    let events = drive_loop(
        &model,
        &loop_turn_with_fast_io(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    let notices = notices_of(&events);
    let subtypes: Vec<&str> = notices.iter().map(|(s, _)| *s).collect();
    assert!(
        subtypes == vec!["api_retry", "api_retry", "api_retry", "api_error"],
        "悬挂逐次重试留痕后耗尽失败收敛: {subtypes:?}"
    );
    assert_eq!(
        model.captured_requests().len(),
        4,
        "首发 + 3 次重试共 4 次请求（悬挂 Opening 逐次重新发起）"
    );
    let payload = serde_json::to_value(notices[3].1).expect("payload 序列化");
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
async fn 流帧空闲悬挂时重试耗尽后超时失败收敛api_error不悬挂() {
    let dir = tempdir("hang-stream");
    let model = FakeModel::hanging(HangPoint::Stream); // 首帧后无新帧且永不 EOF
    let handle = RunHandle::default();

    let events = drive_loop(
        &model,
        &loop_turn_with_fast_io(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    let notices = notices_of(&events);
    let subtypes: Vec<&str> = notices.iter().map(|(s, _)| *s).collect();
    assert!(
        subtypes == vec!["api_retry", "api_retry", "api_retry", "api_error"],
        "流死逐次重试留痕后耗尽失败收敛: {subtypes:?}"
    );
    let payload = serde_json::to_value(notices[3].1).expect("payload 序列化");
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
// 流 IO 重试恢复（2026-10-07 dev-design 三连败止血：断流不再一次定死 run）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 流起手失败退避重试后成功_api_retry留痕且重发史与首发一致() {
    let dir = tempdir("retry-recover-fail");
    let model = FakeModel::with_turns(vec![vec![raw_text("重试后正文"), raw_final(usage(9, 9))]]);
    model.flaky_first_streams(vec![StreamGlitch::Fail("连接被重置".to_owned())]);
    let handle = RunHandle::default();

    let events = drive_loop(
        &model,
        &loop_turn_with_fast_io(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    // 恰一条重试留痕，无 api_error（恢复即无失败收敛）
    let notices = notices_of(&events);
    let subtypes: Vec<&str> = notices.iter().map(|(s, _)| *s).collect();
    assert_eq!(subtypes, vec!["api_retry"], "恰一条重试留痕: {subtypes:?}");
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert_eq!(payload["attempt"], serde_json::json!(1), "attempt 记第几次");
    assert_eq!(
        payload["max_attempts"],
        serde_json::json!(3),
        "max_attempts 在场"
    );
    assert!(payload.get("delay_ms").is_some(), "退避间隔在场");
    assert!(
        payload["error"]
            .as_str()
            .is_some_and(|cause| cause.contains("连接被重置")),
        "留痕携带失败成因，实际: {payload}"
    );

    // 重发请求与首发同史（幂等重发：同一轮、同一史）
    let requests = model.captured_requests();
    assert_eq!(requests.len(), 2, "首发失败 + 重发成功共 2 次请求");
    assert_eq!(
        requests[0].chat_history.len(),
        requests[1].chat_history.len(),
        "重发史与首发同长（幂等重发）"
    );

    // 恢复后正常收口：恰一条密封 assistant + success 收敛
    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 1, "恢复轮恰一条密封 Message");
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success", "重试恢复以正常收敛收口");
    assert!(!*is_error);
}

#[tokio::test]
async fn 帧空闲悬挂退避重试后成功_不再一次定死run() {
    let dir = tempdir("retry-recover-hang");
    // 生产现场形态（2026-10-07 dev-design 三连败）：大上下文轮首帧后断流 →
    // 帧空闲超时 → 重发成功续跑
    let model = FakeModel::with_turns(vec![vec![raw_text("恢复正文"), raw_final(usage(1, 1))]]);
    model.flaky_first_streams(vec![StreamGlitch::Hang(HangPoint::Stream)]);
    let handle = RunHandle::default();

    let events = drive_loop(
        &model,
        &loop_turn_with_fast_io(dir.path(), AgentPermissionMode::BypassPermissions),
        &handle,
    )
    .await;

    let notices = notices_of(&events);
    let subtypes: Vec<&str> = notices.iter().map(|(s, _)| *s).collect();
    assert_eq!(
        subtypes,
        vec!["api_retry"],
        "断流重试恰一条留痕: {subtypes:?}"
    );
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert!(
        payload["error"]
            .as_str()
            .is_some_and(|cause| cause.contains("流空闲超时")),
        "留痕成因指明帧空闲超时，实际: {payload}"
    );
    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 1, "恢复轮恰一条密封 Message");
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success", "断流重试恢复以正常收敛收口");
    assert!(!*is_error);
}

// ---------------------------------------------------------------------------
// 空轮 / 轮数无上限语义
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 空流轮以正常收敛防死循环不悬挂() {
    let dir = tempdir("empty-round");
    let model = FakeModel::with_turns(vec![vec![]]); // 零流项 → choice 空
    let handle = RunHandle::default();

    let events = drive_loop(
        &model,
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
async fn 超过旧熔断手数的工具轮持续驱动至自然收敛() {
    let dir = tempdir("no-max-turns");
    // default 档 write 工具恒被 policy 拒绝（无 IO）：60 轮被拒工具轮（越过
    // 旧 50 轮熔断线）后模型给出纯文本轮 → 正常收敛（轮数不设上限）
    let mut turns: Vec<Vec<MockStreamEvent>> = (0..60)
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
    turns.push(vec![raw_text("收尾"), raw_final(usage(1, 1))]);
    let model = FakeModel::with_turns(turns);
    let handle = RunHandle::default();

    let events = drive_loop(
        &model,
        &loop_turn(dir.path(), AgentPermissionMode::Default),
        &handle,
    )
    .await;

    let notices = notices_of(&events);
    assert!(
        notices.len() == 60
            && notices
                .iter()
                .all(|(subtype, _)| *subtype == "permission_denied"),
        "60 轮 permission_denied（无 IO 快速拒绝）无熔断记因，实际: {:?}",
        notices.iter().map(|(s, _)| *s).collect::<Vec<_>>()
    );
    let AgentEventKind::TurnDone {
        subtype,
        is_error,
        num_turns,
        ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success", "文本轮以正常收敛收口");
    assert!(!*is_error);
    assert_eq!(
        *num_turns,
        Some(61),
        "61 轮驱动（60 被拒工具轮 + 1 文本轮），旧 50 轮上限不再截停"
    );
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
    let dyn_model = model.erased();
    let task =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
        );
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
    let dyn_model = model.erased();
    let history =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
        )
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
    let mut flood: Vec<MockStreamEvent> = Vec::with_capacity(306);
    for index in 0..305 {
        flood.push(raw_text(&format!("片{index}")));
    }
    flood.push(raw_final(usage(305, 1)));
    let model = FakeModel::with_turns(vec![flood]);
    let handle = RunHandle::default();

    let events = drive_loop(
        &model,
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
        &model,
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
        &model,
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
        retry: crate::sdk::r#loop::StreamRetry::default(),
        injections: agent::SessionInjections::default(),
        explore_note: None,
        archi: None,
        protect: crate::sdk::protect::ProtectRules::load(None),
        web_search: false,
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
    // 两轮假流：首轮以工具调用续轮（text-only 轮即收敛），首轮流请求捕获后
    // 改写 AGENT.md（模拟 agent 会话中自改）
    let fake = FakeModel::with_turns(vec![
        vec![
            raw_tool_call("tu_pre", "read", serde_json::json!({ "path": "README.md" })),
            raw_final(usage(3, 3)),
        ],
        vec![raw_text("完成"), raw_final(usage(4, 4))],
    ]);
    fake.rewrite_file_on_first_stream(agent_md, "第二版系统提示词 🚀");
    write_file(&dir.path().join("README.md"), "占位内容");
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = fake.erased();
    let task =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
        );
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 正常结束");

    // 首轮请求系统提示词逐字含文件内容；轮间改写 → 次轮请求为改后内容
    // （0.43 承载口径：chat_history 首条 System 消息）
    let requests = fake.captured_requests();
    assert_eq!(requests.len(), 2, "两轮请求（工具续轮）");
    assert_eq!(
        requests[0].system_instructions(),
        Some("第一版系统提示词"),
        "首轮请求系统提示词逐字含 AGENT.md 内容"
    );
    assert_eq!(
        requests[1].system_instructions(),
        Some("第二版系统提示词 🚀"),
        "轮间改写文件 → 次轮请求为改后内容（每轮重读无缓存）"
    );
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error);
}

#[tokio::test]
async fn preamble缺席时请求无system消息不注入空串() {
    let dir = tempdir("preamble-absent");
    let model = FakeModel::with_turns(vec![vec![raw_text("好"), raw_final(usage(1, 1))]]);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
        );
    while receiver.recv().await.is_some() {}
    let _history = task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1);
    assert!(
        !requests[0]
            .chat_history
            .iter()
            .any(|message| matches!(message, Message::System { .. })),
        "root 无 AGENT.md → 请求不注入 System 消息（不注入空串）"
    );
}

#[tokio::test]
async fn preamble在场时请求系统提示词逐字含文件内容() {
    let dir = tempdir("preamble-capture");
    write_file(&dir.path().join("AGENT.md"), "# 约定 🎉\n- 提交信息用中文");
    let model = FakeModel::with_turns(vec![vec![raw_text("好"), raw_final(usage(1, 1))]]);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
        );
    while receiver.recv().await.is_some() {}
    let _history = task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    assert_eq!(
        requests[0].system_instructions(),
        Some("# 约定 🎉\n- 提交信息用中文"),
        "请求系统提示词逐字含文件内容（含中文 / emoji / 换行）"
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
    let model = FakeModel::with_turns(vec![vec![raw_text("完成"), raw_final(usage(2, 2))]]);
    // 窗长 65_536：L2 过线剪裁、L3 不过线（水位推导见上一用例注释）
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(65_536)),
    );
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task = tokio::spawn(async move {
        r#loop::run(&dyn_model, &turn, oversized_seed_history(), sender, handle).await
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
            if matches!(&content[0], UserContent::Text(text) if text.text == "任务书")
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
            if matches!(&content[0], UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
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
    let model = FakeModel::with_turns_and_completions(
        vec![vec![raw_text("完成"), raw_final(usage(2, 2))]],
        vec![Ok("摘要要点".to_owned())],
    );
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(64)),
    );
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task = tokio::spawn(async move {
        r#loop::run(&dyn_model, &turn, oversized_seed_history(), sender, handle).await
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
                if matches!(&content[0], UserContent::Text(text) if text.text.starts_with("[历史摘要]"))
        ),
        "摘要消息置顶带 [历史摘要] 头"
    );
    assert!(
        matches!(
            &history[1],
            Message::User { content }
                if matches!(&content[0], UserContent::Text(text) if text.text == "任务书")
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
        vec![Err("端点过载".to_owned()), Err("端点仍过载".to_owned())],
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
    let model = FakeModel::with_turns_and_completions(
        vec![vec![raw_text("完成"), raw_final(usage(2, 2))]],
        vec![Err("端点过载".to_owned()), Err("端点仍过载".to_owned())],
    );
    let turn = loop_turn_with_defense(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        ContextDefense::resolve(Some(64)),
    );
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task = tokio::spawn(async move {
        r#loop::run(&dyn_model, &turn, oversized_seed_history(), sender, handle).await
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
            if matches!(&content[0], UserContent::Text(text) if text.text == "任务书")
    ));
    assert!(matches!(
        &history[1],
        Message::User { content }
            if matches!(&content[0], UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
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
    let model = FakeModel::with_turns(vec![vec![raw_text("完成"), raw_final(usage(1, 1))]]);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions); // 宽窗缺省防线
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
        );
    while receiver.recv().await.is_some() {}
    task.await.expect("loop 正常结束");

    let requests = model.captured_requests();
    let history = &requests[0].chat_history;
    assert_eq!(history.len(), 1, "低水位史零剪裁（仅新轮提示词）");
    assert!(matches!(
        &history[0],
        Message::User { content }
            if matches!(&content[0], UserContent::Text(text) if text.text == "帮我看下这个 workspace 🎉")
    ));
}

// ---------------------------------------------------------------------------
// auto-named-explore：LoopTurn 注入面（definitions 单源装配 + 专用工具路由）
// ---------------------------------------------------------------------------

use crate::sdk::explore_note::ExploreNoteContext;
use crate::sdk::tools::{EXPLORE_NOTE_TOOL, TOOL_NAMES};
use agent::SessionToolset;
use store::Store;

/// explore 来源注入底座（六名收窄 + Explore toolset）。
fn explore_injections() -> agent::SessionInjections {
    agent::SessionInjections {
        preamble: None,
        tools: Some(
            TOOL_NAMES
                .iter()
                .filter(|n| **n != "write")
                .map(|n| n.to_string())
                .collect(),
        ),
        toolset: Some(SessionToolset::Explore),
    }
}

/// loop 直驱（携完整 LoopTurn 所有权——explore_note 上下文不可 Clone，注入
/// 面用例专用通道；事件回收与 `drive_loop` 同口径）。
async fn drive_loop_turn(model: &FakeModel, turn: LoopTurn) -> Vec<AgentEventKind> {
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(256);
    let handle = RunHandle::default();
    let dyn_model = model.erased();
    let task =
        tokio::spawn(
            async move { r#loop::run(&dyn_model, &turn, Vec::new(), sender, handle).await },
        );
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    task.await.expect("loop 任务正常结束");
    events
}

/// 摘 tool role 密封消息内的 ToolResult 块。
fn tool_results_of(events: &[AgentEventKind]) -> Vec<(&str, &str, bool)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEventKind::Message {
                role,
                blocks,
                parent_tool_use_id: _,
            } if role == &AgentMessageRole::Tool => match &blocks[0] {
                agent::AgentBlock::ToolResult {
                    id,
                    content,
                    is_error,
                } => Some((id.as_str(), content.as_str(), *is_error)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// explore 轮 RunStarted.tools：注入在场 → run 顶部 definitions 按其组装，
/// `dispatched` 自 definitions 派生（单源口径）→ 下发面 = 六工具保序 +
/// `create_explore_note`。
#[tokio::test]
async fn explore轮runstarted_tools报六工具加专用工具() {
    let dir = tempdir("explore-runstarted");
    let model = FakeModel::with_turns(vec![vec![raw_text("完成"), raw_final(usage(1, 1))]]);
    let turn = LoopTurn {
        injections: explore_injections(),
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    let AgentEventKind::RunStarted { tools, .. } = &events[0] else {
        panic!("首事件应为 RunStarted，实际: {:?}", events[0]);
    };
    assert_eq!(
        tools,
        &[
            "read".to_owned(),
            "grep".to_owned(),
            "glob".to_owned(),
            "ls".to_owned(),
            "edit".to_owned(),
            "bash".to_owned(),
            EXPLORE_NOTE_TOOL.to_owned(),
        ],
        "explore 轮下发面：TOOL_NAMES 保序过滤 + 专用工具追加"
    );
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error, "explore 轮正常收敛");
}

/// default 轮 RunStarted.tools：七名与 `TOOL_NAMES` 逐名一致（与上线前一致，
/// AC-7——注入缺省零变化）。
#[tokio::test]
async fn default轮runstarted_tools报七名与清单逐名一致() {
    let dir = tempdir("default-runstarted");
    let model = FakeModel::with_turns(vec![vec![raw_text("完成"), raw_final(usage(1, 1))]]);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);

    let events = drive_loop_turn(&model, turn).await;

    let AgentEventKind::RunStarted { tools, .. } = &events[0] else {
        panic!("首事件应为 RunStarted，实际: {:?}", events[0]);
    };
    assert_eq!(
        tools.as_slice(),
        TOOL_NAMES.as_slice(),
        "default 轮下发面与七通用清单同源同序（含 write）"
    );
}

/// 幻觉 write 成员校验拒绝：explore 注入轮发起 `write` 调用（bypass 档也拦
/// ——成员校验先于 policy）→ `SystemNotice{tool_unavailable}` + is_error
/// ToolResult 同 id 回灌、history 收对、run 不中断续轮正常、不触执行体（零
/// 文件残留）。
#[tokio::test]
async fn explore轮幻觉write调用被成员校验拒绝且run不中断() {
    let dir = tempdir("explore-hallucinated-write");
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_ghost",
            "write",
            serde_json::json!({ "path": "ghost.txt", "content": "幻觉写入" }),
        )],
        vec![raw_text("已按拒绝继续")],
    ]);
    let turn = LoopTurn {
        injections: explore_injections(),
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    // 成员校验拒绝形态：tool_unavailable（非 permission_denied——先于 policy）
    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1, "恰一次拒绝记因");
    assert_eq!(notices[0].0, "tool_unavailable");
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert_eq!(payload["tool"], serde_json::json!("write"));
    assert!(
        payload["reason"]
            .as_str()
            .is_some_and(|r| r.contains("下发面")),
        "拒绝记因注明不在本轮下发面，实际: {payload}"
    );

    // is_error ToolResult 同 id 回灌
    let results = tool_results_of(&events);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "tu_ghost", "与 tool_use 同 id 成对");
    assert!(results[0].2, "拒绝结果 is_error");

    // 不触执行体：write 若被执行将落盘 ghost.txt——零残留
    assert!(
        !dir.path().join("ghost.txt").exists(),
        "幻觉调用不触执行体（零文件残留）"
    );

    // run 不中断：第二请求续轮 + 正常收敛；ToolUse/ToolResult 成对入史
    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 2, "拒绝后续轮不中断");
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error);
    let requests = model.captured_requests();
    assert_eq!(requests.len(), 2, "两轮流请求");
    let history = &requests[1].chat_history;
    assert!(
        history_tool_result_text(history, "tu_ghost").is_some(),
        "tool_result 已入史（history 收对）"
    );
}

/// 未知名拒绝形态迁移：default 轮未知名调用 → `tool_unavailable`（自
/// `permission_denied` 形态迁移的回归断言——成员校验自 definitions 派生）。
#[tokio::test]
async fn default轮未知名调用以tool_unavailable拒绝() {
    let dir = tempdir("unknown-name-migrated");
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call("tu_unknown", "rm_rf", serde_json::json!({}))],
        vec![raw_text("已按拒绝继续")],
    ]);
    let turn = loop_turn(dir.path(), AgentPermissionMode::BypassPermissions);

    let events = drive_loop_turn(&model, turn).await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].0, "tool_unavailable",
        "未知名以成员校验形态拒绝（迁移自 permission_denied）"
    );
    assert!(
        !notices
            .iter()
            .any(|(subtype, _)| *subtype == "permission_denied"),
        "未知名不再走 policy 拒绝形态"
    );
    turn_done_of(&events);
}

/// 成员校验先于 policy：未知名 + Default 档 → `tool_unavailable` 而非
/// `permission_denied`（分发循环顶部次序断言）。
#[tokio::test]
async fn 未知名加default档命中成员校验而非policy() {
    let dir = tempdir("member-before-policy");
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_order",
            "phantom_tool",
            serde_json::json!({}),
        )],
        vec![raw_text("继续")],
    ]);
    // Default 档：policy 面该名亦会被拒——但成员校验在先，记因必为
    // tool_unavailable
    let turn = loop_turn(dir.path(), AgentPermissionMode::Default);

    let events = drive_loop_turn(&model, turn).await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].0, "tool_unavailable",
        "分发循环顶部成员校验先于 policy 档位判定"
    );
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert_eq!(payload["tool"], serde_json::json!("phantom_tool"));
}

/// 真实临时 explore 装置：workspace 库 + 占位建档（会话指针回写）。
fn explore_fixture(
    tag: &str,
) -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Arc<Store>,
    store::ExploreRecord,
) {
    let dir = tempdir(tag);
    let db_dir = tempdir(&format!("{tag}-db"));
    let store = Arc::new(
        Store::open_workspace(&db_dir.path().join("ws.redb"))
            .unwrap_or_else(|e| panic!("打开临时 workspace 库失败: {e}")),
    );
    let root = dir.path().to_string_lossy().into_owned();
    let record = store
        .create_explore_record(&root, None)
        .expect("占位建档应成功");
    (dir, db_dir, store, record)
}

/// create 工具路由：explore 轮 `create_explore_note` 调用 + `explore_note`
/// 上下文在场（真实临时库 + 占位记录）+ AcceptEdits → 路由
/// `ExploreNoteContext::execute`，ToolResult 承接返回路径（repo 相对）、
/// ToolUse/ToolResult 成对入史（AC-3 链路收口——落盘与改名真实发生）。
#[tokio::test]
async fn explore轮create工具路由执行体_落盘改名真实发生且结果成对入史() {
    let (dir, _db_dir, store, record) = explore_fixture("create-route");
    let session_id = record.session_id.clone().expect("占位建档回写会话指针");
    let context = ExploreNoteContext::new(Arc::clone(&store), dir.path().to_path_buf(), session_id);
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_note",
            EXPLORE_NOTE_TOOL,
            serde_json::json!({ "name": "api-retry", "content": "# 探索笔记\n重试策略结论" }),
        )],
        vec![raw_text("已定名")],
    ]);
    let turn = LoopTurn {
        injections: explore_injections(),
        explore_note: Some(context),
        ..loop_turn(dir.path(), AgentPermissionMode::AcceptEdits)
    };

    let events = drive_loop_turn(&model, turn).await;

    // 执行体承接：ToolResult 非错误、内容为返回的 repo 相对路径
    let results = tool_results_of(&events);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "tu_note");
    assert!(!results[0].2, "执行成功非 is_error");
    assert_eq!(
        results[0].1, "openspec/explores/api-retry.md",
        "ToolResult 承接返回路径（repo 相对 / 分隔）"
    );

    // 落盘与改名真实发生（真实组合不 mock）
    let note = dir
        .path()
        .join("openspec")
        .join("explores")
        .join("api-retry.md");
    assert!(note.exists(), "笔记文件已落盘");
    assert_eq!(
        std::fs::read_to_string(&note).unwrap(),
        "# 探索笔记\n重试策略结论",
        "content 原样落盘"
    );
    let renamed = store
        .find_explore_record(&dir.path().to_string_lossy(), "api-retry")
        .unwrap()
        .expect("记录已定名");
    assert_eq!(renamed.id, record.id, "in-place 改名保主键");
    assert_eq!(renamed.session_id, record.session_id, "会话指针不变");
    assert!(
        store
            .find_explore_record(&dir.path().to_string_lossy(), &record.name)
            .unwrap()
            .is_none(),
        "占位名不再命中（世上无占位名记录）"
    );

    // ToolUse/ToolResult 成对入史（次轮请求史内以同 id 配对）
    let requests = model.captured_requests();
    assert_eq!(requests.len(), 2);
    let history = &requests[1].chat_history;
    assert!(
        history_tool_result_text(history, "tu_note").is_some(),
        "tool_result 已入史"
    );

    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error, "路由成功轮正常收敛");
}

/// create 上下文缺席：上下文 `None`（生产不可达臂——explore 会话恒装配）→
/// is_error 回灌「执行上下文未装配」，run 不中断。
#[tokio::test]
async fn explore轮create工具上下文缺席_is_error回灌且run不中断() {
    let dir = tempdir("create-no-ctx");
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_noctx",
            EXPLORE_NOTE_TOOL,
            serde_json::json!({ "name": "any-name", "content": "x" }),
        )],
        vec![raw_text("已按缺席继续")],
    ]);
    let turn = LoopTurn {
        injections: explore_injections(),
        explore_note: None,
        archi: None,
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    let results = tool_results_of(&events);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "tu_noctx");
    assert!(results[0].2, "上下文缺席以 is_error 回灌");
    assert!(
        results[0].1.contains("执行上下文未装配"),
        "记因即恢复指引，实际: {}",
        results[0].1
    );
    // run 不中断
    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 2, "缺席回灌后续轮不中断");
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error);
}

/// create + Default 档：`create_explore_note` 调用 + Default 档 →
/// `permission_denied`（policy 写面归属组合用例，链路方向 loop → policy，
/// AC-6）；执行体未被触达（零落盘、记录名不变）。
#[tokio::test]
async fn explore轮create工具default档被policy拒绝且不触执行体() {
    let (dir, _db_dir, store, record) = explore_fixture("create-default-deny");
    let session_id = record.session_id.clone().expect("占位建档回写会话指针");
    let context = ExploreNoteContext::new(Arc::clone(&store), dir.path().to_path_buf(), session_id);
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_deny",
            EXPLORE_NOTE_TOOL,
            serde_json::json!({ "name": "api-retry", "content": "x" }),
        )],
        vec![raw_text("已按拒绝继续")],
    ]);
    let turn = LoopTurn {
        injections: explore_injections(),
        explore_note: Some(context),
        ..loop_turn(dir.path(), AgentPermissionMode::Default)
    };

    let events = drive_loop_turn(&model, turn).await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].0, "permission_denied",
        "Default 档以 policy 写面形态拒绝（AC-6）"
    );
    // 不触执行体：零落盘、记录名不变
    assert!(
        !dir.path()
            .join("openspec")
            .join("explores")
            .join("api-retry.md")
            .exists(),
        "拒绝不落盘"
    );
    let still_draft = store
        .find_explore_record(&dir.path().to_string_lossy(), &record.name)
        .unwrap()
        .expect("记录仍在");
    assert_eq!(still_draft.name, record.name, "记录名未变（未定名）");
    turn_done_of(&events);
}

/// create + BypassPermissions：放行且执行体承接（与 AcceptEdits 两写档对照）。
#[tokio::test]
async fn explore轮create工具bypass档放行且执行体承接() {
    let (dir, _db_dir, store, record) = explore_fixture("create-bypass");
    let session_id = record.session_id.clone().expect("占位建档回写会话指针");
    let context = ExploreNoteContext::new(Arc::clone(&store), dir.path().to_path_buf(), session_id);
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_bypass",
            EXPLORE_NOTE_TOOL,
            serde_json::json!({ "name": "layout-design", "content": "布局结论" }),
        )],
        vec![raw_text("已定名")],
    ]);
    let turn = LoopTurn {
        injections: explore_injections(),
        explore_note: Some(context),
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    let results = tool_results_of(&events);
    assert_eq!(results.len(), 1);
    assert!(!results[0].2, "放行且执行成功");
    assert_eq!(results[0].1, "openspec/explores/layout-design.md");
    assert!(
        dir.path()
            .join("openspec")
            .join("explores")
            .join("layout-design.md")
            .exists(),
        "执行体真实落盘"
    );
    assert!(
        store
            .find_explore_record(&dir.path().to_string_lossy(), "layout-design")
            .unwrap()
            .is_some(),
        "记录已定名"
    );
    turn_done_of(&events);
}

/// 探索面六工具收窄不误伤：explore 轮 `read` / `grep` / `glob` / `ls` / `edit`
/// / `bash` 下发面内调用 → 正常执行（逐工具直驱，零拒绝记因）。
#[tokio::test]
async fn explore轮六工具下发面内调用逐一正常执行() {
    struct Case {
        tool: &'static str,
        setup: fn(&Path),
        input: serde_json::Value,
    }
    let cases = [
        Case {
            tool: "read",
            setup: |root: &Path| write_file(&root.join("note.txt"), "第一行\n线索行\n第三行"),
            input: serde_json::json!({ "path": "note.txt" }),
        },
        Case {
            tool: "grep",
            setup: |root: &Path| write_file(&root.join("note.txt"), "无关\n线索 here"),
            input: serde_json::json!({ "path": "note.txt", "pattern": "线索" }),
        },
        Case {
            tool: "glob",
            setup: |root: &Path| write_file(&root.join("hit.rs"), "fn m() {}"),
            input: serde_json::json!({ "pattern": "*.rs" }),
        },
        Case {
            tool: "ls",
            setup: |root: &Path| write_file(&root.join("entry.txt"), "x"),
            input: serde_json::json!({ "path": "." }),
        },
        Case {
            tool: "edit",
            setup: |root: &Path| write_file(&root.join("note.txt"), "旧串待替换"),
            input: serde_json::json!({
                "path": "note.txt",
                "old_string": "旧串",
                "new_string": "新串"
            }),
        },
        Case {
            tool: "bash",
            setup: |_root: &Path| {},
            input: serde_json::json!({ "command": "echo loop-six-ok" }),
        },
    ];

    for case in cases {
        let tag = format!("six-{}", case.tool);
        let dir = tempdir(&tag);
        (case.setup)(dir.path());
        let model = FakeModel::with_turns(vec![
            vec![raw_tool_call("tu_six", case.tool, case.input.clone())],
            vec![raw_text("继续")],
        ]);
        let turn = LoopTurn {
            injections: explore_injections(),
            ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
        };

        let events = drive_loop_turn(&model, turn).await;

        let notices = notices_of(&events);
        assert!(
            notices.is_empty(),
            "{} 不误伤（零拒绝记因），实际: {notices:?}",
            case.tool
        );
        let results = tool_results_of(&events);
        assert_eq!(results.len(), 1, "{} 恰一次结果回灌", case.tool);
        assert_eq!(results[0].0, "tu_six");
        assert!(!results[0].2, "{} 执行成功非 is_error", case.tool);
        let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
            panic!("应为 TurnDone");
        };
        assert!(!*is_error, "{} 轮正常收敛", case.tool);
    }
}

// ---------------------------------------------------------------------------
// 写入保护闸（守卫链第四段：沙箱之后、执行之前）
//
// 装置复用：`FakeModel` + `drive_loop_turn` + `notices_of` / `tool_results_of`
// / `sealed_messages`；规则集经 `ProtectRules::load(None)`（仅内置）或
// `load(Some(tempdir))`（内置 + 真实配置文件用户规则）真实装配。
// ---------------------------------------------------------------------------

/// 配置文件相对路径（由 `foundation::layout` 派生，测试内零目录名字面量）。
fn protected_config_relative() -> String {
    foundation::layout::config_path(Path::new(""))
        .to_string_lossy()
        .replace(r"\", "/")
}

/// 携写入保护规则集的轮参数（既有 `loop_turn` 装置的规则集注入缝）。
fn loop_turn_with_protect(
    cwd: &Path,
    mode: AgentPermissionMode,
    protect: crate::sdk::protect::ProtectRules,
) -> LoopTurn {
    LoopTurn {
        protect,
        ..loop_turn(cwd, mode)
    }
}

/// 落盘真实工作区配置（`foundation::layout::config_path` 派生路径）。
fn write_workspace_config(root: &Path, config: serde_json::Value) {
    let path = foundation::layout::config_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建配置目录失败");
    }
    write_file(
        &path,
        &serde_json::to_string_pretty(&config).expect("配置序列化"),
    );
}

/// 拒绝通知 payload 里的 reason 串。
fn notice_reason(payload: &serde_json::Value) -> String {
    payload["reason"]
        .as_str()
        .expect("拒绝记因必为字符串")
        .to_owned()
}

#[tokio::test]
async fn 写入保护闸_write命中内置_拒且零落盘与run不中断() {
    let dir = tempdir("protect-write-builtin");
    let protected = dir.path().join(protected_config_relative());
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_protect",
            "write",
            serde_json::json!({ "path": protected.to_string_lossy(), "content": "篡改" }),
        )],
        vec![raw_text("已停止改配置")],
    ]);
    let turn = loop_turn_with_protect(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        crate::sdk::protect::ProtectRules::load(None),
    );

    let events = drive_loop_turn(&model, turn).await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1, "恰一次权限拒绝通知");
    assert_eq!(
        notices[0].0, "permission_denied",
        "保护闸复用既有 permission_denied 子类（零新事件词汇）"
    );
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert_eq!(payload["tool"], serde_json::json!("write"));
    let reason = notice_reason(&payload);
    assert!(
        reason.contains("配置权威"),
        "拒绝串为内置模板，实际: {reason}"
    );

    let results = tool_results_of(&events);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "tu_protect", "与 tool_use 同 id 成对");
    assert!(results[0].2, "拒绝合成 is_error ToolResult");
    assert_eq!(
        results[0].1, reason,
        "notice 载荷与 ToolResult 内容逐字节相同（模型可见同一串）"
    );
    assert!(!protected.exists(), "受保护文件零落盘");

    assert_eq!(sealed_messages(&events).len(), 2, "拒绝后续轮不中断");
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error, "拒绝不中断 run（正常收敛）");
}

#[tokio::test]
async fn 写入保护闸_edit命中用户规则_替换后串双面一致且字节不变() {
    let dir = tempdir("protect-edit-user");
    write_workspace_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                { "glob": "docs/**", "reason": "文档冻结：%s（经 %t）" }
            ] }
        }),
    );
    let target = dir.path().join("docs").join("a.md");
    write_file(&target, "原始内容");

    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_edit",
            "edit",
            serde_json::json!({
                "path": target.to_string_lossy(),
                "old_string": "原始",
                "new_string": "改写"
            }),
        )],
        vec![raw_text("已停止")],
    ]);
    let turn = loop_turn_with_protect(
        dir.path(),
        AgentPermissionMode::AcceptEdits,
        crate::sdk::protect::ProtectRules::load(Some(dir.path())),
    );

    let events = drive_loop_turn(&model, turn).await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1);
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    let reason = notice_reason(&payload);
    assert!(
        reason.contains("docs/a.md") && reason.contains("edit"),
        "用户模板占位符已替换（路径 + 工具名），实际: {reason}"
    );
    assert!(reason.contains("文档冻结"), "模板文案透传，实际: {reason}");
    let results = tool_results_of(&events);
    assert_eq!(results[0].1, reason, "双面同串");
    assert_eq!(
        std::fs::read_to_string(&target).expect("文件可读"),
        "原始内容",
        "受保护文件字节不变"
    );
}

#[tokio::test]
async fn 写入保护闸_bash三路写法各自命中() {
    let dir = tempdir("protect-bash-routes");
    let protected = dir.path().join(protected_config_relative());
    let config = protected_config_relative();
    let commands = [
        format!("echo {{}} > {config}"),
        format!("rm {config}"),
        format!("node -e \"require('fs').writeFileSync('{config}','{{}}')\""),
    ];

    for command in commands {
        let model = FakeModel::with_turns(vec![
            vec![raw_tool_call(
                "tu_bash",
                "bash",
                serde_json::json!({ "command": command }),
            )],
            vec![raw_text("已停止")],
        ]);
        let turn = loop_turn_with_protect(
            dir.path(),
            AgentPermissionMode::BypassPermissions,
            crate::sdk::protect::ProtectRules::load(None),
        );

        let events = drive_loop_turn(&model, turn).await;
        let notices = notices_of(&events);
        assert_eq!(notices.len(), 1, "命令各 1 条拒绝记因: {command}");
        assert_eq!(notices[0].0, "permission_denied");
        let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
        assert_eq!(payload["tool"], serde_json::json!("bash"));
        let results = tool_results_of(&events);
        assert_eq!(results.len(), 1, "恰一次结果回灌: {command}");
        assert!(results[0].2, "拒绝结果 is_error: {command}");
        assert_eq!(results[0].1, notice_reason(&payload), "同串双面: {command}");
        assert!(!protected.exists(), "受保护文件零落盘: {command}");
    }
}

#[tokio::test]
async fn 写入保护闸_bash树落点命中即拒而源码子树照常执行() {
    let dir = tempdir("protect-bash-tree");

    let denied_model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_find_all",
            "bash",
            serde_json::json!({ "command": "find . -delete" }),
        )],
        vec![raw_text("已停止")],
    ]);
    let denied = drive_loop_turn(
        &denied_model,
        loop_turn_with_protect(
            dir.path(),
            AgentPermissionMode::BypassPermissions,
            crate::sdk::protect::ProtectRules::load(None),
        ),
    )
    .await;
    let notices = notices_of(&denied);
    assert_eq!(notices.len(), 1, "搜索根覆盖受保护目标即拒");
    assert_eq!(notices[0].0, "permission_denied");

    let allowed_model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_find_src",
            "bash",
            serde_json::json!({ "command": "find src -delete" }),
        )],
        vec![raw_text("继续")],
    ]);
    let allowed = drive_loop_turn(
        &allowed_model,
        loop_turn_with_protect(
            dir.path(),
            AgentPermissionMode::BypassPermissions,
            crate::sdk::protect::ProtectRules::load(None),
        ),
    )
    .await;
    assert!(
        notices_of(&allowed).is_empty(),
        "源码子树搜索根不越拦（零拒绝记因）"
    );
    assert_eq!(
        tool_results_of(&allowed).len(),
        1,
        "执行照常（结果由执行体自身决定）"
    );
}

#[tokio::test]
async fn 写入保护闸_未命中照常执行且文件真实生成() {
    let dir = tempdir("protect-allow");
    let notes = dir.path().join("notes.txt");

    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_write",
            "write",
            serde_json::json!({ "path": notes.to_string_lossy(), "content": "保护闸之外" }),
        )],
        vec![raw_tool_call(
            "tu_bash",
            "bash",
            serde_json::json!({ "command": "echo foo > notes.txt" }),
        )],
        vec![raw_text("完成")],
    ]);
    let turn = loop_turn_with_protect(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        crate::sdk::protect::ProtectRules::load(None),
    );

    let events = drive_loop_turn(&model, turn).await;

    assert!(
        notices_of(&events).is_empty(),
        "未配置用户规则时普通写零拒绝（内置面之外）"
    );
    let results = tool_results_of(&events);
    assert_eq!(results.len(), 2, "两次工具结果");
    assert_eq!(results[0].0, "tu_write");
    assert!(!results[0].2, "write 执行成功（保护闸不抢执行）");
    assert_eq!(results[1].0, "tu_bash");
    assert!(!results[1].2, "bash 执行成功");
    let content = std::fs::read_to_string(&notes).expect("文件真实生成");
    assert!(
        content.contains("foo"),
        "bash 重定向真实落盘，实际内容: {content:?}"
    );
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error);
}

#[tokio::test]
async fn 写入保护闸_只读四工具直通() {
    let dir = tempdir("protect-readonly");
    let protected = dir.path().join(protected_config_relative());
    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call(
                "tu_read",
                "read",
                serde_json::json!({ "path": protected.to_string_lossy() }),
            ),
            raw_tool_call(
                "tu_grep",
                "grep",
                serde_json::json!({ "pattern": "x", "path": dir.path().to_string_lossy() }),
            ),
            raw_tool_call("tu_glob", "glob", serde_json::json!({ "pattern": "**/*" })),
            raw_tool_call(
                "tu_ls",
                "ls",
                serde_json::json!({ "path": dir.path().to_string_lossy() }),
            ),
        ],
        vec![raw_text("读面完成")],
    ]);
    let turn = loop_turn_with_protect(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        crate::sdk::protect::ProtectRules::load(None),
    );

    let events = drive_loop_turn(&model, turn).await;

    assert!(
        notices_of(&events).is_empty(),
        "只读四工具读面全开放（零拒绝记因）"
    );
    assert_eq!(tool_results_of(&events).len(), 4, "四工具各一次结果");
}

#[tokio::test]
async fn 写入保护闸_explore专用工具直通_规则覆盖亦不拦() {
    let (dir, _db_dir, store, record) = explore_fixture("protect-explore");
    let session_id = record.session_id.clone().expect("占位建档回写会话指针");
    let context = ExploreNoteContext::new(Arc::clone(&store), dir.path().to_path_buf(), session_id);
    // 规则覆盖整棵域目录树（含探索笔记目录）：专用工具仍走原执行面
    write_workspace_config(
        dir.path(),
        serde_json::json!({
            "write_protection": { "files": [
                {
                    "glob": format!("{}/**", foundation::layout::domain_dir_name()),
                    "reason": "域树冻结"
                }
            ] }
        }),
    );
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_note",
            EXPLORE_NOTE_TOOL,
            serde_json::json!({ "name": "protect-pass", "content": "# 结论" }),
        )],
        vec![raw_text("已定名")],
    ]);
    let turn = LoopTurn {
        injections: explore_injections(),
        explore_note: Some(context),
        ..loop_turn_with_protect(
            dir.path(),
            AgentPermissionMode::AcceptEdits,
            crate::sdk::protect::ProtectRules::load(Some(dir.path())),
        )
    };

    let events = drive_loop_turn(&model, turn).await;

    assert!(
        notices_of(&events).is_empty(),
        "create_explore_note 走分派表 `_ => None`（零拒绝记因）"
    );
    let results = tool_results_of(&events);
    assert_eq!(results.len(), 1);
    assert!(!results[0].2, "专用工具执行面形态与既有用例一致");
    assert!(
        results[0].1.contains(foundation::layout::domain_dir_name()),
        "返回 repo 相对路径，实际: {}",
        results[0].1
    );
}

#[tokio::test]
async fn 写入保护闸_档位矩阵_两写档生效而default由policy先行() {
    let dir = tempdir("protect-modes");
    for (mode, expect_protect) in [
        (AgentPermissionMode::BypassPermissions, true),
        (AgentPermissionMode::AcceptEdits, true),
        (AgentPermissionMode::Default, false),
    ] {
        let protected = dir.path().join(protected_config_relative());
        let model = FakeModel::with_turns(vec![
            vec![raw_tool_call(
                "tu_mode",
                "write",
                serde_json::json!({ "path": protected.to_string_lossy(), "content": "x" }),
            )],
            vec![raw_text("继续")],
        ]);
        let turn = loop_turn_with_protect(
            dir.path(),
            mode,
            crate::sdk::protect::ProtectRules::load(None),
        );

        let events = drive_loop_turn(&model, turn).await;
        let notices = notices_of(&events);
        assert_eq!(notices.len(), 1, "档位 {mode:?} 恰一次拒绝记因");
        assert_eq!(notices[0].0, "permission_denied");
        let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
        let reason = notice_reason(&payload);
        if expect_protect {
            assert!(
                reason.contains("配置权威"),
                "写档位 {mode:?} 下保护闸生效，实际: {reason}"
            );
        } else {
            assert!(
                reason.contains("权限档位"),
                "Default 档由 policy 先行拒绝（顺序锚），实际: {reason}"
            );
            assert!(
                !reason.contains("配置权威"),
                "保护闸未触（不被抢报），实际: {reason}"
            );
        }
    }
}

#[tokio::test]
async fn 写入保护闸_顺序锚_沙箱先于保护() {
    let dir = tempdir("protect-order-sandbox");
    let escaping = format!("../{}", protected_config_relative());
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_escape",
            "write",
            serde_json::json!({ "path": escaping, "content": "x" }),
        )],
        vec![raw_text("继续")],
    ]);
    let turn = loop_turn_with_protect(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        crate::sdk::protect::ProtectRules::load(None),
    );

    let events = drive_loop_turn(&model, turn).await;
    let notices = notices_of(&events);
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].0, "sandbox_denied",
        "逃逸路径按沙箱成因报错（错误归属不被保护闸抢报）"
    );
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert!(
        notice_reason(&payload).contains("越出"),
        "沙箱文案承接，实际: {payload}"
    );
}

#[tokio::test]
async fn 写入保护闸_顺序锚_成员校验先于保护() {
    let dir = tempdir("protect-order-member");

    // explore 轮发起不在下发面的 write：成员校验拒绝（非保护闸）
    let ghost_model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_ghost",
            "write",
            serde_json::json!({
                "path": dir.path().join(protected_config_relative()).to_string_lossy(),
                "content": "x"
            }),
        )],
        vec![raw_text("继续")],
    ]);
    let ghost_turn = LoopTurn {
        injections: explore_injections(),
        ..loop_turn_with_protect(
            dir.path(),
            AgentPermissionMode::BypassPermissions,
            crate::sdk::protect::ProtectRules::load(None),
        )
    };
    let ghost = drive_loop_turn(&ghost_model, ghost_turn).await;
    let notices = notices_of(&ghost);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, "tool_unavailable");
    let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
    assert!(
        !notice_reason(&payload).contains("配置权威"),
        "零保护文案（成员校验在先）"
    );

    // 未知名 rm_rf：成员校验拒绝，不触 protect
    let unknown_model = FakeModel::with_turns(vec![
        vec![raw_tool_call("tu_rm_rf", "rm_rf", serde_json::json!({}))],
        vec![raw_text("继续")],
    ]);
    let unknown = drive_loop_turn(
        &unknown_model,
        loop_turn_with_protect(
            dir.path(),
            AgentPermissionMode::BypassPermissions,
            crate::sdk::protect::ProtectRules::load(None),
        ),
    )
    .await;
    let notices = notices_of(&unknown);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, "tool_unavailable");
    assert!(
        !notices
            .iter()
            .any(|(subtype, _)| *subtype == "permission_denied"),
        "未知名不走 policy / 保护闸拒绝形态"
    );
}

#[tokio::test]
async fn 写入保护闸_拒绝后history收对且模型可见完整指令() {
    let dir = tempdir("protect-history");
    let protected = dir.path().join(protected_config_relative());
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_history",
            "write",
            serde_json::json!({ "path": protected.to_string_lossy(), "content": "x" }),
        )],
        vec![raw_text("已按拒绝收敛")],
    ]);
    let turn = loop_turn_with_protect(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        crate::sdk::protect::ProtectRules::load(None),
    );

    let events = drive_loop_turn(&model, turn).await;
    let payload = serde_json::to_value(notices_of(&events)[0].1).expect("payload 序列化");
    let reason = notice_reason(&payload);

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 2, "拒绝后照常发起第二轮请求");
    let history = &requests[1].chat_history;
    let result_text = history_tool_result_text(history, "tu_history").expect("tool_result 已入史");
    assert_eq!(result_text, reason, "史内 tool_result 正文即拒绝串");
    assert!(
        result_text.contains("不要改用其他命令或工具尝试绕过"),
        "模型可见的完整指令（禁绕过句）"
    );
    assert_eq!(sealed_messages(&events).len(), 2, "两轮密封齐全");
}

#[tokio::test]
async fn 写入保护闸_同文件连撞三轮不中断也不硬中断() {
    let dir = tempdir("protect-repeat");
    let config = protected_config_relative();
    let model = FakeModel::with_turns(vec![
        vec![raw_tool_call(
            "tu_1",
            "write",
            serde_json::json!({
                "path": dir.path().join(&config).to_string_lossy(),
                "content": "x"
            }),
        )],
        vec![raw_tool_call(
            "tu_2",
            "bash",
            serde_json::json!({ "command": format!("echo x > {config}") }),
        )],
        vec![raw_tool_call(
            "tu_3",
            "bash",
            serde_json::json!({ "command": format!("cp /tmp/x {config}") }),
        )],
        vec![raw_text("三轮后停止")],
    ]);
    let turn = loop_turn_with_protect(
        dir.path(),
        AgentPermissionMode::BypassPermissions,
        crate::sdk::protect::ProtectRules::load(None),
    );

    let events = drive_loop_turn(&model, turn).await;

    let notices = notices_of(&events);
    assert_eq!(notices.len(), 3, "连续三轮各一次拒绝记因");
    assert!(
        notices
            .iter()
            .all(|(subtype, _)| *subtype == "permission_denied"),
        "拒绝形态一致"
    );
    let results = tool_results_of(&events);
    assert_eq!(results.len(), 3, "三条同 id is_error ToolResult");
    assert!(
        results.iter().all(|(_, _, is_error)| *is_error),
        "三条结果均 is_error"
    );
    assert_eq!(
        results.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
        vec!["tu_1", "tu_2", "tu_3"],
        "结果与 tool_use 逐一对位"
    );
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(
        !*is_error,
        "deny 不中断 run（引擎不加「二次拒绝即硬中断」机制）"
    );
    assert!(
        !dir.path().join(&config).exists(),
        "三轮撞同一受保护文件均零落盘"
    );
}

// ---------------------------------------------------------------------------
// web_search 开关：请求侧形状（AC-3）
// ---------------------------------------------------------------------------

/// 携 `web_search` 开关的轮参数（其余面同基线）。
fn web_search_turn(cwd: &Path, enabled: bool) -> LoopTurn {
    LoopTurn {
        web_search: enabled,
        ..loop_turn(cwd, AgentPermissionMode::BypassPermissions)
    }
}

/// 摘 RunStarted 的 tools 清单。
fn run_started_tools(events: &[AgentEventKind]) -> Vec<String> {
    events
        .iter()
        .find_map(|event| match event {
            AgentEventKind::RunStarted {
                tools, mcp_servers, ..
            } => {
                assert!(mcp_servers.is_empty(), "mcp 服务器清单恒空");
                Some(tools.clone())
            }
            _ => None,
        })
        .expect("RunStarted 应在场")
}

#[tokio::test]
async fn 开关开时请求additional_params_tools恰为web_search一项三键() {
    let dir = tempdir("ws-on");
    let model = FakeModel::with_turns(vec![vec![raw_text("好"), raw_final(usage(1, 1))]]);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1);
    let params = requests[0]
        .additional_params
        .as_ref()
        .expect("开关开：additional_params 必须在场");
    assert_eq!(
        params["tools"],
        serde_json::json!([
            { "type": "web_search_20250305", "name": "web_search", "max_uses": 5 }
        ]),
        "additional_params.tools 深等于恰一项三键"
    );
    let entry = params["tools"][0].as_object().expect("工具项为对象");
    assert_eq!(
        entry.len(),
        3,
        "恰三键（无 cache_control / allowed_domains）"
    );
    assert_eq!(
        entry["max_uses"],
        serde_json::json!(5),
        "max_uses 为 JSON 数字 5"
    );

    // 客户端工具面不受影响：request.tools 仍是 definitions 派生集，不含 web_search
    let dispatched: Vec<String> = requests[0]
        .tools
        .iter()
        .map(|definition| definition.name.clone())
        .collect();
    assert_eq!(
        dispatched,
        run_started_tools(&events),
        "request.tools 与 RunStarted.tools 同源（definitions 派生）"
    );
    assert!(
        !dispatched.iter().any(|name| name == "web_search"),
        "服务端工具不进客户端工具面：{dispatched:?}"
    );
}

#[tokio::test]
async fn 开关关时additional_params缺席且其余请求字段与基线逐字一致() {
    let dir = tempdir("ws-off");
    let model = FakeModel::with_turns(vec![vec![raw_text("好"), raw_final(usage(1, 1))]]);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), false), &handle).await;

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(
        request.additional_params.is_none(),
        "开关关：additional_params 无 tools 键（恒 None），实际: {:?}",
        request.additional_params
    );
    assert!(request.model.is_none(), "model 不覆写");
    assert!(request.temperature.is_none());
    assert!(request.max_tokens.is_none());
    assert!(request.tool_choice.is_none());
    assert!(request.output_schema.is_none());
    assert!(request.documents.is_empty());

    // RunStarted.tools 两态均不含 web_search（关态对照）
    let dispatched = run_started_tools(&events);
    assert!(!dispatched.iter().any(|name| name == "web_search"));
}

#[tokio::test]
async fn 开关两态下run_started_tools均等于definitions派生名单且不含web_search() {
    for enabled in [true, false] {
        let dir = tempdir(if enabled {
            "ws-runstarted-on"
        } else {
            "ws-runstarted-off"
        });
        let model = FakeModel::with_turns(vec![vec![raw_text("好"), raw_final(usage(1, 1))]]);
        let handle = RunHandle::default();

        let events = drive_loop(&model, &web_search_turn(dir.path(), enabled), &handle).await;

        let tools = run_started_tools(&events);
        let expected: Vec<String> =
            crate::sdk::tools::definitions(&agent::SessionInjections::default())
                .iter()
                .map(|definition| definition.name.clone())
                .collect();
        assert_eq!(
            tools, expected,
            "开关 {enabled}：RunStarted.tools = definitions 派生名单"
        );
        assert!(
            !tools.iter().any(|name| name == "web_search"),
            "开关 {enabled}：RunStarted.tools 不含 web_search"
        );
    }
}

// ---------------------------------------------------------------------------
// server 块投影（AC-4 loop 侧）：不触发客户端工具执行
// ---------------------------------------------------------------------------

#[tokio::test]
async fn server块投影为web_search块且不触发客户端工具执行与结果回灌() {
    let dir = tempdir("ws-server-blocks");
    let model = FakeModel::with_turns(vec![vec![
        raw_server_block(server_use_raw("srvtoolu_1", "今日汇率")),
        raw_server_block(server_result_raw(
            "srvtoolu_1",
            &["https://a/1", "https://b/2"],
        )),
        raw_final(usage(3, 4)),
    ]]);
    let handle = RunHandle::default();

    let (events, history) =
        drive_loop_collecting_history(&model, &web_search_turn(dir.path(), true), &handle).await;

    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 1, "恰一条密封 assistant Message");
    let AgentEventKind::Message { blocks, .. } = sealed[0] else {
        unreachable!()
    };
    assert_eq!(blocks.len(), 1, "blocks 仅一块 WebSearch（无 ToolUse 块）");
    match &blocks[0] {
        agent::AgentBlock::WebSearch {
            id,
            query,
            results,
            error,
            raw_use,
            raw_result,
        } => {
            assert_eq!(id, "srvtoolu_1");
            assert_eq!(query, "今日汇率");
            assert_eq!(results.len(), 2, "两条来源并入");
            assert!(error.is_none());
            assert!(raw_use.is_some() && raw_result.is_some(), "两侧原文承载");
        }
        other => panic!("唯一块应为 WebSearch，实际: {other:?}"),
    }

    // 该轮以 stop 收口：历史按「prompt + 一条 assistant」增长，无 user 位 tool_result
    assert_eq!(history.len(), 2, "prompt + 一条 assistant（零结果回灌）");
    assert!(matches!(history[0], Message::User { .. }), "史首为 prompt");
    assert!(
        matches!(&history[1], Message::Assistant { content, .. }
            if content.iter().all(|item| !matches!(item, AssistantContent::ToolCall(_)))),
        "assistant 位不含客户端工具调用（server 块不进客户端工具面）"
    );
    assert!(
        history
            .iter()
            .all(|message| !matches!(message, Message::User { content }
                if content.iter().any(|item| matches!(item, UserContent::ToolResult(_))))),
        "无 user 位 tool_result 回灌（server 块不进客户端工具面）"
    );
    let AgentEventKind::TurnDone { subtype, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success");
}

#[tokio::test]
async fn server块与客户端工具混排时走工具执行分支且续跑计数不增() {
    let dir = tempdir("ws-mixed-blocks");
    write_file(&dir.path().join("a.txt"), "内容甲");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_server_block(server_use_raw("srvtoolu_mix", "混合查询")),
            raw_server_block(server_result_raw("srvtoolu_mix", &["https://mix/1"])),
            raw_tool_call("tu_mix", "read", serde_json::json!({ "path": "a.txt" })),
            raw_final(usage(1, 1)),
        ],
        vec![raw_text("收尾"), raw_final(usage(1, 1))],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

    let sealed = sealed_messages(&events);
    assert_eq!(sealed.len(), 2, "两轮各一条密封 assistant");
    let AgentEventKind::Message { blocks, .. } = sealed[0] else {
        unreachable!()
    };
    assert!(
        matches!(&blocks[0], agent::AgentBlock::WebSearch { .. })
            && matches!(&blocks[1], agent::AgentBlock::ToolUse { .. }),
        "原序 [WebSearch, ToolUse]，实际: {blocks:?}"
    );
    // 走工具执行分支：不续跑（续跑计数不增，零 pause_turn 通知）
    let subtypes: Vec<&str> = notices_of(&events)
        .iter()
        .map(|(subtype, _)| *subtype)
        .collect();
    assert!(
        !subtypes
            .iter()
            .any(|subtype| *subtype == "pause_turn_continued" || *subtype == "pause_turn_capped"),
        "客户端 tool_calls 非空 → 走工具执行，实际通知: {subtypes:?}"
    );
    assert_eq!(
        model.captured_requests().len(),
        2,
        "工具轮后正常收口（恰两请求）"
    );

    // 工具结果回灌后第二轮正常收口
    let requests = model.captured_requests();
    assert!(
        requests[1].chat_history.iter().any(|message| matches!(message, Message::User { content }
            if content.iter().any(|item| matches!(item, UserContent::ToolResult(result) if result.name.as_str() == "read")))),
        "第二轮史含成对工具结果"
    );
    let AgentEventKind::TurnDone { subtype, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success");
}

#[tokio::test]
async fn text块citations落密封事件且无引用块为none() {
    let dir = tempdir("ws-citations");
    let model = FakeModel::with_turns(vec![vec![
        MockStreamEvent::TextStart {
            id: "cite".to_owned(),
            additional_params: Some(serde_json::json!({
                "citations": [
                    { "url": "https://cite/1", "title": "引用标题", "cited_text": "片段", "encrypted_index": "x" }
                ]
            })),
        },
        raw_text("带引用正文"),
        // 换段：关闭携引用的文本部件，另起无引用的文本部件
        MockStreamEvent::TextStart {
            id: "plain".to_owned(),
            additional_params: None,
        },
        raw_text("无引用正文"),
        raw_final(usage(1, 1)),
    ]]);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

    let AgentEventKind::Message { blocks, .. } = sealed_messages(&events)[0] else {
        unreachable!()
    };
    let text_blocks: Vec<&agent::AgentBlock> = blocks
        .iter()
        .filter(|block| matches!(block, agent::AgentBlock::Text { .. }))
        .collect();
    assert_eq!(text_blocks.len(), 2, "两个文本块（引用 / 无引用）");
    let agent::AgentBlock::Text { citations, .. } = text_blocks[0] else {
        unreachable!()
    };
    let citations = citations.as_ref().expect("携引用块 citations 在场");
    assert_eq!(citations.len(), 1);
    assert_eq!(
        citations[0].raw,
        serde_json::json!({
            "url": "https://cite/1", "title": "引用标题", "cited_text": "片段", "encrypted_index": "x"
        }),
        "citations.raw 与原 wire 逐字节相等"
    );
    let agent::AgentBlock::Text { citations, .. } = text_blocks[1] else {
        unreachable!()
    };
    assert!(citations.is_none(), "无引用文本块 citations 为 None");
}

#[tokio::test]
async fn 开关开时模型调用客户端web_search名被成员校验拒且run不中断() {
    let dir = tempdir("ws-client-name");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call(
                "tu_ws",
                "web_search",
                serde_json::json!({ "query": "今日汇率" }),
            ),
            raw_final(usage(1, 1)),
        ],
        vec![raw_text("改用其他方式"), raw_final(usage(1, 1))],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

    let subtypes: Vec<&str> = notices_of(&events)
        .iter()
        .map(|(subtype, _)| *subtype)
        .collect();
    assert!(
        subtypes.contains(&"tool_unavailable"),
        "web_search 不在 TOOL_NAMES / dispatch 面：成员校验拒，实际: {subtypes:?}"
    );
    // 拒绝结果以 is_error ToolResult 回灌（同 id），run 不中断
    let tool_results: Vec<&agent::AgentBlock> = events
        .iter()
        .filter_map(|event| match event {
            AgentEventKind::Message { role, blocks, .. } if role == &AgentMessageRole::Tool => {
                Some(blocks.as_slice())
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert!(
        tool_results.iter().any(|block| matches!(block,
            agent::AgentBlock::ToolResult { id, is_error, .. } if id == "tu_ws" && *is_error)),
        "拒绝合成 is_error 结果（同 id 成对回灌）"
    );
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success", "run 不中断（工具被拒后收口）");
    assert!(!*is_error);
    assert_eq!(
        model.captured_requests().len(),
        2,
        "拒绝后进入第二轮正常收口"
    );
}

// ---------------------------------------------------------------------------
// pause_turn 续跑（AC-6）：独立计数轴、事件流可观测、到顶收敛
// ---------------------------------------------------------------------------

/// 摘 `pause_turn_continued` / `pause_turn_capped` 通知的 continuations 计数序列。
fn continuation_counts(events: &[AgentEventKind], subtype: &str) -> Vec<u64> {
    notices_of(events)
        .into_iter()
        .filter(|(name, _)| *name == subtype)
        .map(|(_, payload)| {
            payload["continuations"]
                .as_u64()
                .unwrap_or_else(|| panic!("{subtype} payload 应携 continuations 计数"))
        })
        .collect()
}

#[tokio::test]
async fn pause_turn续跑发第二次请求且每次一条continued通知() {
    let dir = tempdir("pause-turn-once");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_server_block(server_use_raw("srvtoolu_p", "续跑查询")),
            raw_server_block(server_result_raw("srvtoolu_p", &["https://p/1"])),
            raw_pause_turn(usage(2, 3)),
        ],
        vec![raw_text("搜索结果已确认"), raw_final(usage(4, 5))],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 2, "恰两次流请求（续发一次）");
    // 第二次请求的 chat_history 含前一轮 assistant server 块原样
    assert!(
        requests[1].chat_history.iter().any(
            |message| matches!(message, Message::Assistant { content, .. }
            if content.iter().any(|item| matches!(item, AssistantContent::Text(text)
                if text.additional_params.as_ref().is_some_and(|params|
                    params.get("anthropic_content").is_some()))))
        ),
        "续跑请求携前一轮 assistant server 块原样（additional_params.anthropic_content 在场）"
    );

    assert_eq!(
        continuation_counts(&events, "pause_turn_continued"),
        vec![1],
        "每次续跑恰一条 pause_turn_continued，payload continuations 从 1 起"
    );
    assert!(
        continuation_counts(&events, "pause_turn_capped").is_empty(),
        "未到顶不发 capped"
    );
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success", "最终以 stop 语义成功收敛");
    assert!(!*is_error);
}

#[tokio::test]
async fn pause_turn续跑不吃重试预算_续跑请求失败仍以完整预算重试成功() {
    let dir = tempdir("pause-turn-retry");
    let model = FakeModel::with_turns(vec![
        vec![raw_text("首轮正文"), raw_pause_turn(usage(1, 1))],
        vec![raw_text("续跑正文"), raw_final(usage(1, 1))],
    ]);
    // 续跑请求（第 2 次流请求）单点失败一次；预算仅 1 次重试
    model.fail_at_stream_request(2, "续跑请求瞬时失败");
    let handle = RunHandle::default();
    let mut turn = loop_turn_with_fast_io(dir.path(), AgentPermissionMode::BypassPermissions);
    turn.retry.max_attempts = 1;

    let events = drive_loop(&model, &turn, &handle).await;

    let subtypes: Vec<&str> = notices_of(&events)
        .iter()
        .map(|(subtype, _)| *subtype)
        .collect();
    assert!(
        subtypes.contains(&"api_retry"),
        "续跑请求失败经既有重试路径留痕，实际: {subtypes:?}"
    );
    assert!(
        !subtypes.contains(&"api_error"),
        "续跑前的 attempt 归零：完整预算可用，零 api_error，实际: {subtypes:?}"
    );
    assert_eq!(
        model.captured_requests().len(),
        3,
        "首发 + 续跑首发 + 续跑重试共 3 次请求"
    );
    assert_eq!(
        continuation_counts(&events, "pause_turn_continued"),
        vec![1],
        "续跑通知仍恰一条（重试不重复发续跑通知）"
    );
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
async fn pause_turn连续到顶时五条continued加一条capped且按stop语义收敛() {
    let dir = tempdir("pause-turn-capped");
    let turns: Vec<Vec<MockStreamEvent>> = (0..6)
        .map(|index| {
            vec![
                raw_text(&format!("第 {index} 段")),
                raw_pause_turn(usage(1, 1)),
            ]
        })
        .collect();
    let model = FakeModel::with_turns(turns);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

    assert_eq!(
        continuation_counts(&events, "pause_turn_continued"),
        vec![1, 2, 3, 4, 5],
        "恰 5 条 continued（payload 逐条递增）"
    );
    assert_eq!(
        continuation_counts(&events, "pause_turn_capped"),
        vec![5],
        "恰 1 条 capped（connections = 上限 5）"
    );
    assert_eq!(
        model.captured_requests().len(),
        6,
        "零第 7 次请求（上限到顶即收口）"
    );
    let AgentEventKind::TurnDone {
        subtype,
        is_error,
        num_turns,
        ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(
        subtype, "success",
        "到顶按 stop 语义成功收敛（非 api_error）"
    );
    assert!(!*is_error);
    assert_eq!(*num_turns, Some(1), "续跑属同轮：num_turns 不变");
}

#[tokio::test]
async fn pause_turn使用量逐次累加且不取末次不抹零() {
    let dir = tempdir("pause-turn-usage");
    let model = FakeModel::with_turns(vec![
        vec![raw_text("第一段"), raw_pause_turn(usage(11, 13))],
        vec![raw_text("第二段"), raw_pause_turn(usage(17, 19))],
        vec![raw_text("第三段"), raw_final(usage(23, 29))],
    ]);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

    let AgentEventKind::TurnDone {
        usage: done_usage, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(
        done_usage["input_tokens"],
        serde_json::json!(11 + 17 + 23),
        "三帧 input 线性累加（不取末次、不抹零）"
    );
    assert_eq!(
        done_usage["output_tokens"],
        serde_json::json!(13 + 19 + 29),
        "三帧 output 线性累加"
    );
}

#[tokio::test]
async fn pause_turn空choice不续跑且走既有空轮收敛路径() {
    let dir = tempdir("pause-turn-empty");
    let model = FakeModel::with_turns(vec![vec![raw_pause_turn(usage(1, 1))]]);
    let handle = RunHandle::default();

    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

    assert!(
        continuation_counts(&events, "pause_turn_continued").is_empty(),
        "空 choice 不续跑（零 continued）"
    );
    assert_eq!(model.captured_requests().len(), 1, "零第二次请求");
    let AgentEventKind::TurnDone {
        subtype, is_error, ..
    } = turn_done_of(&events)
    else {
        panic!("应为 TurnDone");
    };
    assert_eq!(subtype, "success", "走既有空轮收敛路径");
    assert!(!*is_error);
}

#[tokio::test]
async fn 续跑间隙停止截停时返回且不投续跑请求与通知() {
    let dir = tempdir("pause-turn-stop");
    let model = FakeModel::with_turns(vec![vec![raw_text("首段"), raw_pause_turn(usage(1, 1))]]);
    let handle = RunHandle::default();
    // 通道容量 1：loop 每发一事件即阻塞，停止信号可在续跑判定前确定性置位
    // （current_thread 运行时下 recv 返回后同任务不 yield，竞态不存在）
    let (sender, mut receiver) = mpsc::channel::<AgentEventKind>(1);
    let turn = web_search_turn(dir.path(), true);
    let dyn_model = model.erased();
    let loop_handle = handle.clone();
    let task = tokio::spawn(async move {
        r#loop::run(&dyn_model, &turn, Vec::new(), sender, loop_handle).await
    });

    // 首个事件（RunStarted）到达即置位停止：loop 后续进入续跑间隙检查
    let mut events = vec![receiver.recv().await.expect("首事件在场")];
    handle.request_stop();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let history = task.await.expect("loop 正常结束");

    assert!(
        continuation_counts(&events, "pause_turn_continued").is_empty(),
        "停止截停：不发 pause_turn_continued"
    );
    assert!(
        continuation_counts(&events, "pause_turn_capped").is_empty(),
        "停止截停：不发 pause_turn_capped"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEventKind::TurnDone { .. })),
        "停止路径不合成 TurnDone（为编排侧显式 stopped 收敛让路）"
    );
    assert!(
        model.captured_requests().len() <= 1,
        "零孤儿续跑请求，实际: {}",
        model.captured_requests().len()
    );
    assert!(history.len() <= 2, "历史无新增续跑 assistant 消息");
}

#[tokio::test]
async fn finish_reason判据精确等值_pause_turn变体一律不续跑() {
    // 非精确匹配四形态：pause_turn_x / PAUSE_TURN / Stop / None 一律不续跑
    let cases: Vec<(&str, Option<FinishReason>)> = vec![
        (
            "pause_turn_x",
            Some(FinishReason::Other("pause_turn_x".to_owned())),
        ),
        (
            "PAUSE_TURN",
            Some(FinishReason::Other("PAUSE_TURN".to_owned())),
        ),
        ("stop", Some(FinishReason::Stop)),
        ("none", None),
    ];

    for (label, reason) in cases {
        let dir = tempdir("pause-turn-exact");
        let final_frame = match reason {
            Some(reason) => raw_final_with_reason(reason, usage(1, 1)),
            None => raw_final(usage(1, 1)),
        };
        let model = FakeModel::with_turns(vec![vec![raw_text("正文"), final_frame]]);
        let handle = RunHandle::default();

        let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

        assert!(
            continuation_counts(&events, "pause_turn_continued").is_empty(),
            "{label}：非精确 pause_turn 不续跑"
        );
        assert_eq!(model.captured_requests().len(), 1, "{label}：恰一次请求");
        let AgentEventKind::TurnDone { subtype, .. } = turn_done_of(&events) else {
            panic!("{label}：应为 TurnDone");
        };
        assert_eq!(subtype, "success", "{label}：正常收敛");
    }

    // 对照组：恰 Other("pause_turn") 唯一续跑
    let dir = tempdir("pause-turn-exact-hit");
    let model = FakeModel::with_turns(vec![
        vec![raw_text("正文"), raw_pause_turn(usage(1, 1))],
        vec![raw_text("收尾"), raw_final(usage(1, 1))],
    ]);
    let handle = RunHandle::default();
    let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;
    assert_eq!(
        continuation_counts(&events, "pause_turn_continued"),
        vec![1],
        "恰 pause_turn 唯一触发续跑"
    );
}

#[tokio::test]
async fn 计数轴隔离_续跑不增轮数且run_started恰一条() {
    for continuations in [1_usize, 5] {
        let dir = tempdir("pause-turn-axis");
        let mut turns: Vec<Vec<MockStreamEvent>> = (0..continuations)
            .map(|index| {
                vec![
                    raw_text(&format!("段 {index}")),
                    raw_pause_turn(usage(1, 1)),
                ]
            })
            .collect();
        turns.push(vec![raw_text("收尾"), raw_final(usage(1, 1))]);
        let model = FakeModel::with_turns(turns);
        let handle = RunHandle::default();

        let events = drive_loop(&model, &web_search_turn(dir.path(), true), &handle).await;

        let started = events
            .iter()
            .filter(|event| matches!(event, AgentEventKind::RunStarted { .. }))
            .count();
        assert_eq!(started, 1, "{continuations} 次续跑：RunStarted 恰一条");
        let AgentEventKind::TurnDone {
            num_turns, subtype, ..
        } = turn_done_of(&events)
        else {
            panic!("应为 TurnDone");
        };
        assert_eq!(
            *num_turns,
            Some(1),
            "{continuations} 次续跑不增轮数（属同轮）"
        );
        assert_eq!(subtype, "success");
        assert_eq!(
            continuation_counts(&events, "pause_turn_continued").len(),
            continuations,
            "续跑计数逐次递增"
        );
        assert_eq!(
            model.captured_requests().len(),
            continuations + 1,
            "续跑次数 + 收口请求"
        );
    }
}

// ---------------------------------------------------------------------------
// archi-graph-doc-agent：执行臂三分派（`LoopTurn.archi` 上下文路由）+ 成员校验
// 矩阵外拒绝 + 两 archi 名在守卫链后段零拦截
//
// 装置：真实 tempdir 模型（`openspec/architecture/models/*.toml`）+ 真实
// `ArchiContext`（读面即被测语义，执行体不 mock）；loop 直驱与既有 explore
// 注入面用例同口径。
// ---------------------------------------------------------------------------

use crate::sdk::archi_tool::ArchiContext;
use crate::sdk::tools::{ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL};

/// 健康模型（`App` / `App.Auth` 两元素，`App.Auth` 带 gate 与 summary）。
const ARCHI_CORE: &str = concat!(
    "# 核心包\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"strict\"\n",
    "summary = \"认证域\"\n",
);

/// 落一个模型文件（`openspec/architecture/models/`）。
fn archi_write_model(root: &Path, name: &str, content: &str) {
    let dir = root.join("openspec/architecture/models");
    std::fs::create_dir_all(&dir).expect("创建模型目录失败");
    std::fs::write(dir.join(name), content).expect("写模型文件失败");
}

/// 健康合成根（模型在盘；archi 执行体真实读盘）。
fn archi_fixture(tag: &str) -> tempfile::TempDir {
    let dir = tempdir(tag);
    archi_write_model(dir.path(), "01-core.toml", ARCHI_CORE);
    dir
}

/// archi 来源注入底座（两布尔位按需；`tools` 不收窄 = 七通用工具全量）。
fn archi_injections(module: bool, search: bool) -> agent::SessionInjections {
    agent::SessionInjections {
        preamble: None,
        tools: None,
        toolset: Some(SessionToolset::Archi { module, search }),
    }
}

/// 执行臂三分派（正向，表驱动两工具名）：`turn.archi = Some(ctx)` 且下发面含
/// 对应 archi 工具 → 走 `ArchiContext::execute`（真实 tempdir 模型），结果
/// `is_error = false` 且内容为 `archi::dto` 投影 JSON（`archi_module` 携
/// `element`，`archi_search` 携 `elements`），run 不中断。
#[tokio::test]
async fn archi分派两工具到执行体且结果非错误() {
    struct Case {
        tool: &'static str,
        key: &'static str,
        input: serde_json::Value,
    }
    let cases = [
        Case {
            tool: ARCHI_MODULE_TOOL,
            key: "element",
            input: serde_json::json!({ "element": "App.Auth" }),
        },
        Case {
            tool: ARCHI_SEARCH_TOOL,
            key: "elements",
            input: serde_json::json!({ "query": "auth" }),
        },
    ];
    for Case { tool, key, input } in cases {
        let dir = archi_fixture(&format!("archi-dispatch-{tool}"));
        let model = FakeModel::with_turns(vec![
            vec![raw_tool_call("tu_archi", tool, input)],
            vec![raw_text("已看模型")],
        ]);
        let turn = LoopTurn {
            injections: archi_injections(true, true),
            archi: Some(ArchiContext::new(dir.path().to_path_buf())),
            ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
        };

        let events = drive_loop_turn(&model, turn).await;

        assert!(
            notices_of(&events).is_empty(),
            "{tool}: 分派路径零拒绝记因（成员校验 / policy / sandbox / protect 全放行）"
        );
        let results = tool_results_of(&events);
        assert_eq!(results.len(), 1, "{tool}: 恰一条工具结果");
        assert_eq!(results[0].0, "tu_archi", "{tool}: 与 tool_use 同 id 成对");
        assert!(!results[0].2, "{tool}: 执行体承接 → 非 is_error");
        let payload: serde_json::Value = serde_json::from_str(results[0].1)
            .unwrap_or_else(|error| panic!("{tool}: 结果为 dto 投影 JSON 串: {error}"));
        assert!(
            payload.get(key).is_some(),
            "{tool}: 结果含 dto 字段 {key}，实际: {payload}"
        );
        if tool == ARCHI_MODULE_TOOL {
            // AC-1 / AC-10：声明位置单点（`modelFile` repo 相对 + `modelLine` 行号）
            assert!(
                payload["element"]["modelFile"]
                    .as_str()
                    .is_some_and(|file| file.ends_with("openspec/architecture/models/01-core.toml")),
                "{tool}: modelFile 指向声明文件，实际: {payload}"
            );
            assert_eq!(
                payload["element"]["modelLine"],
                serde_json::json!(5),
                "{tool}: modelLine 指向 `[element.\"App.Auth\"]` 表头行"
            );
        }

        // run 不中断：执行成功后续轮照常
        assert_eq!(sealed_messages(&events).len(), 2, "{tool}: 续轮不中断");
        let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
            panic!("应为 TurnDone");
        };
        assert!(!*is_error, "{tool}: 正常收敛");
    }
}

/// 铸造根传递（正向，表驱动 2 组合 × 2 root）：某一 root 铸造的 `LoopTurn.archi`
/// 跑完整一轮后工具结果只取自该 root 下模型；换第二个 root（另一 tempdir、另一套
/// 元素）→ 结果逐字段随之切换（root 传错即红）。
#[tokio::test]
async fn archi执行臂结果只取自铸造根且换根即换内容() {
    const ROOT_A: &str = concat!("# 根 A\n", "[element.\"Alpha\"]\n", "kind = \"package\"\n",);
    const ROOT_B: &str = concat!("# 根 B\n", "[element.\"Beta\"]\n", "kind = \"package\"\n",);
    for (module, search) in [(true, true), (true, false)] {
        for (tag, model, expected) in [
            ("archi-root-a", ROOT_A, "Alpha"),
            ("archi-root-b", ROOT_B, "Beta"),
        ] {
            let dir = tempdir(tag);
            archi_write_model(dir.path(), "01-model.toml", model);
            let scripted = FakeModel::with_turns(vec![
                vec![raw_tool_call(
                    "tu_root",
                    ARCHI_MODULE_TOOL,
                    serde_json::json!({ "element": expected }),
                )],
                vec![raw_text("已看该根模型")],
            ]);
            let turn = LoopTurn {
                injections: archi_injections(module, search),
                archi: Some(ArchiContext::new(dir.path().to_path_buf())),
                ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
            };

            let events = drive_loop_turn(&scripted, turn).await;

            let results = tool_results_of(&events);
            assert_eq!(
                results.len(),
                1,
                "{{module: {module}, search: {search}}} {tag}: 恰一条工具结果"
            );
            assert!(!results[0].2, "{tag}: 上下文在场 → 非 is_error");
            let payload: serde_json::Value = serde_json::from_str(results[0].1)
                .unwrap_or_else(|error| panic!("{tag}: 结果为 dto 投影 JSON 串: {error}"));
            // 元素定位只在该 root 的模型里成立（另一 root 无此元素 → 解析即 Err），
            // `modelFile` / `modelLine` 指向该 root 下模型文件的声明行
            assert_eq!(
                payload["element"]["fqn"],
                serde_json::json!(expected),
                "{tag}: 命中元素的 fqn 只来自铸造根（换 root 即换内容）"
            );
            assert!(
                payload["element"]["modelFile"].as_str().is_some_and(
                    |file| file.ends_with("openspec/architecture/models/01-model.toml")
                ),
                "{tag}: modelFile 指向铸造根下的模型文件，实际: {payload}"
            );
            assert_eq!(
                payload["element"]["modelLine"],
                serde_json::json!(2),
                "{tag}: modelLine 指向铸造根模型文件的表头行"
            );
        }
    }
}

/// 执行臂上下文缺席（异常，表驱动两工具名）：`turn.archi = None` 而下发面含
/// archi 工具（生产不可达臂）→ 工具结果 `is_error = true` 且文案「专用工具执行
/// 上下文未装配」，run 不中断。
#[tokio::test]
async fn archi上下文缺席以is_error回灌且run不中断() {
    for tool in [ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL] {
        let dir = tempdir(&format!("archi-no-ctx-{tool}"));
        let model = FakeModel::with_turns(vec![
            vec![raw_tool_call(
                "tu_noctx",
                tool,
                serde_json::json!({ "element": "App.Auth", "query": "auth" }),
            )],
            vec![raw_text("已按缺席继续")],
        ]);
        let turn = LoopTurn {
            injections: archi_injections(true, true),
            archi: None,
            ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
        };

        let events = drive_loop_turn(&model, turn).await;

        assert!(
            notices_of(&events).is_empty(),
            "{tool}: 上下文缺席不走拒绝形态（非成员校验 / policy 拒绝）"
        );
        let results = tool_results_of(&events);
        assert_eq!(results.len(), 1, "{tool}: 恰一条工具结果");
        assert_eq!(results[0].0, "tu_noctx", "{tool}: 同 id 回灌");
        assert!(results[0].2, "{tool}: 缺席以 is_error 回灌");
        assert_eq!(
            results[0].1, "专用工具执行上下文未装配",
            "{tool}: 记因即恢复指引（与 explore 专用工具同文案）"
        );

        // run 不中断
        assert_eq!(
            sealed_messages(&events).len(),
            2,
            "{tool}: 缺席回灌后续轮不中断"
        );
        let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
            panic!("应为 TurnDone");
        };
        assert!(!*is_error, "{tool}: 正常收敛");
    }
}

/// 成员校验（矩阵外相位，异常，表驱动两工具名）：下发面不含 archi 工具而模型
/// 调用之 → `SystemNotice{subtype: "tool_unavailable"}` + is_error ToolResult
/// （同 id）；执行体**零调用**（内容即成员校验记因，非 dto JSON），run 不中断。
///
/// Default 档下同时成立「成员校验先于 policy」：两 archi 名在只读面本可放行，
/// 记因仍为 `tool_unavailable`（次序若反则得 `permission_denied`）。
#[tokio::test]
async fn 矩阵外相位调用archi工具被成员校验拒绝且零执行() {
    for tool in [ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL] {
        let dir = archi_fixture(&format!("archi-unavailable-{tool}"));
        let model = FakeModel::with_turns(vec![
            vec![raw_tool_call(
                "tu_ghost",
                tool,
                serde_json::json!({ "element": "App.Auth", "query": "auth" }),
            )],
            vec![raw_text("已按拒绝继续")],
        ]);
        // 矩阵外相位：注入缺省（零 archi 工具）；上下文在场——若执行体被触达，
        // 结果必为 dto JSON，故「内容 == 成员校验记因」即零调用的行为证据
        let turn = LoopTurn {
            injections: agent::SessionInjections::default(),
            archi: Some(ArchiContext::new(dir.path().to_path_buf())),
            ..loop_turn(dir.path(), AgentPermissionMode::Default)
        };

        let events = drive_loop_turn(&model, turn).await;

        let notices = notices_of(&events);
        assert_eq!(notices.len(), 1, "{tool}: 恰一次拒绝记因");
        assert_eq!(
            notices[0].0, "tool_unavailable",
            "{tool}: 成员校验先于 policy（Default 档亦不落 permission_denied）"
        );
        let payload = serde_json::to_value(notices[0].1).expect("payload 序列化");
        assert_eq!(payload["tool"], serde_json::json!(tool));
        assert!(
            payload["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("下发面")),
            "{tool}: 记因注明不在本轮下发面，实际: {payload}"
        );

        let results = tool_results_of(&events);
        assert_eq!(results.len(), 1, "{tool}: 恰一条工具结果");
        assert_eq!(results[0].0, "tu_ghost", "{tool}: 同 id 回灌");
        assert!(results[0].2, "{tool}: 拒绝结果 is_error");
        assert_eq!(
            results[0].1,
            format!("工具不在本轮下发面: {tool}"),
            "{tool}: 内容即成员校验记因（执行体零调用——否则为 dto JSON）"
        );
        assert!(
            serde_json::from_str::<serde_json::Value>(results[0].1).is_err(),
            "{tool}: 结果非 JSON（执行体未被触达）"
        );

        // run 不中断：拒绝后续轮照常收敛
        assert_eq!(
            sealed_messages(&events).len(),
            2,
            "{tool}: 拒绝后续轮不中断"
        );
        let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
            panic!("应为 TurnDone");
        };
        assert!(!*is_error, "{tool}: 正常收敛");
    }
}

/// 守卫链后段零拦截（回归，表驱动两工具名）：两 archi 名在 sandbox 段原样通过
/// （`input_path` 恒 `None`——入参夹带 `path` 字段亦不改写 / 不校验）、protect
/// 分派表 `_ => None` 零拦截（夹带受保护配置路径亦不拒）→ 无
/// `sandbox_denied` / `permission_denied`，执行体承接。
#[tokio::test]
async fn archi两工具在守卫链后段零拦截() {
    struct Case {
        tool: &'static str,
        input: serde_json::Value,
    }
    let cases = [
        Case {
            tool: ARCHI_MODULE_TOOL,
            input: serde_json::json!({ "element": "App.Auth", "path": "openspec/config.json" }),
        },
        Case {
            tool: ARCHI_SEARCH_TOOL,
            input: serde_json::json!({ "query": "auth", "path": "openspec/config.json" }),
        },
    ];
    for Case { tool, input } in cases {
        let dir = archi_fixture(&format!("archi-guard-{tool}"));
        let model = FakeModel::with_turns(vec![
            vec![raw_tool_call("tu_guard", tool, input)],
            vec![raw_text("已取模型")],
        ]);
        // Default 档：policy 只读面放行两 archi 名（三档决策表由 policy_test 断言）
        let turn = LoopTurn {
            injections: archi_injections(true, true),
            archi: Some(ArchiContext::new(dir.path().to_path_buf())),
            ..loop_turn(dir.path(), AgentPermissionMode::Default)
        };

        let events = drive_loop_turn(&model, turn).await;

        let subtypes: Vec<&str> = notices_of(&events).iter().map(|(s, _)| *s).collect();
        assert!(
            !subtypes.contains(&"sandbox_denied"),
            "{tool}: 无 sandbox 拒绝（input_path 恒 None，路径字段不入沙箱）"
        );
        assert!(
            !subtypes.contains(&"permission_denied"),
            "{tool}: 无 protect / policy 拒绝（读面零拦截）"
        );
        let results = tool_results_of(&events);
        assert_eq!(results.len(), 1, "{tool}: 恰一条工具结果");
        assert!(!results[0].2, "{tool}: 执行体承接（未被守卫链截断）");
        assert!(
            serde_json::from_str::<serde_json::Value>(results[0].1).is_ok(),
            "{tool}: 内容为 dto 投影 JSON，实际: {}",
            results[0].1
        );
        turn_done_of(&events);
    }
}

/// `LoopTurn` 两专用上下文并列（边界）：同一轮内 `archi` 在场、`explore_note`
/// 缺席 → archi 工具走执行体、explore 专用工具落成员校验拒绝（矩阵外）——
/// 两字段互不串道，新增字段不改既有 `explore_note` 臂语义。
#[tokio::test]
async fn archi与explore专用上下文并列不串道() {
    let dir = archi_fixture("archi-with-explore-note");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call(
                "tu_archi",
                ARCHI_MODULE_TOOL,
                serde_json::json!({ "element": "App.Auth" }),
            ),
            raw_tool_call(
                "tu_note",
                EXPLORE_NOTE_TOOL,
                serde_json::json!({ "name": "any-name", "content": "x" }),
            ),
        ],
        vec![raw_text("已分别处置")],
    ]);
    let turn = LoopTurn {
        injections: archi_injections(true, true),
        archi: Some(ArchiContext::new(dir.path().to_path_buf())),
        explore_note: None,
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    let results = tool_results_of(&events);
    assert_eq!(results.len(), 2, "两工具各得一条结果");
    let archi = results
        .iter()
        .find(|(id, _, _)| *id == "tu_archi")
        .expect("archi 结果在场");
    assert!(!archi.2, "archi 臂走执行体（上下文在场）");
    assert!(
        serde_json::from_str::<serde_json::Value>(archi.1).is_ok(),
        "archi 结果为 dto JSON"
    );
    let note = results
        .iter()
        .find(|(id, _, _)| *id == "tu_note")
        .expect("explore 笔记结果在场");
    assert!(note.2, "explore 工具不在 Archi 下发面 → is_error");
    assert_eq!(
        note.1,
        format!("工具不在本轮下发面: {EXPLORE_NOTE_TOOL}"),
        "既有 explore 臂语义零改动（矩阵外拒绝形态）"
    );
    assert_eq!(sealed_messages(&events).len(), 2, "分派 / 拒绝后照常续轮");
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error);
}

// ---------------------------------------------------------------------------
// 系统消息合成单点（`system_prompt`）：会话 preamble（知识，前）+ 本轮
// `AGENT.md`（逐字，后）→ 恰一条 System 消息内容；两段之间单空行分隔。
//
// 纯函数直接调用（断言即契约），run 侧用例经既有假流装置 + 真实 tempdir 的
// `AGENT.md`（`preamble::load` 不 mock：真实读取与真实 32KB 截断口径）。
// ---------------------------------------------------------------------------

/// 纯函数面：`(Some, Some)` → 知识在前、`AGENT.md` 逐字在后、恰一个空行分隔
/// （无标题 / 无前缀包装）。
#[test]
fn system_prompt双在场知识在前agent_md逐字在后() {
    assert_eq!(
        crate::sdk::r#loop::system_prompt(Some("K"), Some("A".to_owned())),
        Some("K\n\nA".to_owned())
    );
    assert_eq!(
        crate::sdk::r#loop::system_prompt(
            Some("# 知识\n第二行"),
            Some("# 约定\n- 中文".to_owned())
        ),
        Some("# 知识\n第二行\n\n# 约定\n- 中文".to_owned()),
        "只有两段之间的一个空行是插入物（段内换行逐字保留）"
    );
}

/// 纯函数面：仅知识 → `Some(knowledge)`（不追加分隔符、不追加空行）。
#[test]
fn system_prompt仅知识不追加分隔符() {
    assert_eq!(
        crate::sdk::r#loop::system_prompt(Some("K"), None),
        Some("K".to_owned())
    );
    assert_eq!(
        crate::sdk::r#loop::system_prompt(Some("K\n"), None),
        Some("K\n".to_owned()),
        "末换行不归一（逐字返回）"
    );
}

/// 纯函数面：仅 `AGENT.md` → 逐字返回（既有语义零改写：不加前后缀、不改写、
/// 不包装）。
#[test]
fn system_prompt仅agent_md逐字() {
    assert_eq!(
        crate::sdk::r#loop::system_prompt(None, Some("A".to_owned())),
        Some("A".to_owned())
    );
    assert_eq!(
        crate::sdk::r#loop::system_prompt(None, Some(String::new())),
        Some(String::new()),
        "空串原样返回（不降级、不补默认）"
    );
}

/// 纯函数面：两者皆无 → `None`（不注入空串 System）。
#[test]
fn system_prompt两者皆无返回none() {
    assert_eq!(crate::sdk::r#loop::system_prompt(None, None), None);
}

/// 边界：空串入参按公式拼接（不降级到单侧分支、不 panic、不吞首段）。
#[test]
fn system_prompt空串入参按公式拼接() {
    assert_eq!(
        crate::sdk::r#loop::system_prompt(Some(""), Some("A".to_owned())),
        Some("\n\nA".to_owned())
    );
    assert_eq!(
        crate::sdk::r#loop::system_prompt(Some("K"), Some(String::new())),
        Some("K\n\n".to_owned())
    );
    assert_eq!(
        crate::sdk::r#loop::system_prompt(Some(""), Some(String::new())),
        Some("\n\n".to_owned()),
        "双空串亦按公式（恰一个分隔空行）"
    );
}

/// 边界：特殊字符逐字保留（零转义 / 零归一），分段位置可用 `split("\n\n")` 稳定
/// 还原为两段。
#[test]
fn system_prompt特殊字符逐字保留() {
    let cases = [
        ("换行", "第一段\n第二行", "文件段\n第二行"),
        ("NUL", "知识\u{0}段", "文件\u{0}段"),
        ("emoji", "知识🎉段", "文件🚀段"),
        ("CRLF", "知识\r\n第二行", "文件\r\n第二行"),
    ];
    for (label, knowledge, agent_md) in cases {
        let merged = crate::sdk::r#loop::system_prompt(Some(knowledge), Some(agent_md.to_owned()))
            .unwrap_or_else(|| panic!("{label}: 双在场应 Some"));
        let parts: Vec<&str> = merged.split("\n\n").collect();
        assert_eq!(parts.len(), 2, "{label}: 双换行切分 可还原两段: {merged:?}");
        assert_eq!(parts[0], knowledge, "{label}: 首段 = 知识原文");
        assert_eq!(parts[1], agent_md, "{label}: 次段 = AGENT.md 原文");
        assert_eq!(
            merged.len(),
            knowledge.len() + agent_md.len() + 2,
            "{label}: 长度 = 两段之和 + 2（零增删）"
        );
    }
}

/// 边界：超长与不截断——`agent_md` 40KB + 知识 4KB → 返回长度 = 两段之和 + 2
/// （32KB 截断责任在 `preamble::load`，知识不截）。
#[test]
fn system_prompt超长不截断长度守恒() {
    let knowledge = "k".repeat(4096);
    let agent_md = "a".repeat(40 * 1024);
    let merged = crate::sdk::r#loop::system_prompt(Some(&knowledge), Some(agent_md.clone()))
        .expect("双在场应 Some");
    assert_eq!(
        merged.len(),
        knowledge.len() + agent_md.len() + 2,
        "system_prompt 不截断（两段长度守恒 + 分隔符）"
    );
    assert!(merged.starts_with(&knowledge), "知识段完整在前");
    assert!(merged.ends_with(&agent_md), "AGENT.md 段完整在后");
}

/// 边界：纯函数性——同参两次调用逐字相等（入参即全部输入，不读盘不查环境）。
#[test]
fn system_prompt纯函数性() {
    let first = crate::sdk::r#loop::system_prompt(Some("K"), Some("A".to_owned()));
    let second = crate::sdk::r#loop::system_prompt(Some("K"), Some("A".to_owned()));
    assert_eq!(first, second, "同参两次调用逐字相等");
    let other = crate::sdk::r#loop::system_prompt(Some("K"), Some("B".to_owned()));
    assert_ne!(first, other, "入参变即结果变（无内部缓存）");
}

/// 运行态：`preamble = Some(K)` + root 有 `AGENT.md` → 首轮请求恰一条 System，
/// 内容 = `K\n\n<AGENT.md 原文>`（不为两段各推一条）。
#[tokio::test]
async fn run系统消息合成双在场恰一条system() {
    let dir = tempdir("system-merge-both");
    write_file(&dir.path().join("AGENT.md"), "文件约定 🎉");
    let model = FakeModel::with_turns(vec![vec![raw_text("好"), raw_final(usage(1, 1))]]);
    let turn = LoopTurn {
        injections: agent::SessionInjections {
            preamble: Some("架构知识段".to_owned()),
            tools: None,
            toolset: None,
        },
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1, "单轮一次请求");
    let systems = requests[0]
        .chat_history
        .iter()
        .filter(|message| matches!(message, Message::System { .. }))
        .count();
    assert_eq!(systems, 1, "恰一条 System 消息（不为两段各推一条）");
    assert_eq!(
        requests[0].system_instructions(),
        Some("架构知识段\n\n文件约定 🎉"),
        "合成内容 = 知识在前 + AGENT.md 逐字在后（单空行分隔）"
    );
    turn_done_of(&events);
}

/// 运行态：`preamble = Some(K)` + root 无 `AGENT.md` → 恰一条 System = `K`
/// （既有「缺席不注入空串」语义扩展到单侧在场）。
#[tokio::test]
async fn run系统消息合成仅preamble恰一条system() {
    let dir = tempdir("system-merge-knowledge-only");
    let model = FakeModel::with_turns(vec![vec![raw_text("好"), raw_final(usage(1, 1))]]);
    let turn = LoopTurn {
        injections: agent::SessionInjections {
            preamble: Some("架构知识段".to_owned()),
            tools: None,
            toolset: None,
        },
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    let requests = model.captured_requests();
    let systems = requests[0]
        .chat_history
        .iter()
        .filter(|message| matches!(message, Message::System { .. }))
        .count();
    assert_eq!(systems, 1, "恰一条 System");
    assert_eq!(
        requests[0].system_instructions(),
        Some("架构知识段"),
        "无 AGENT.md 时即知识原文（不追加分隔 / 空行）"
    );
    turn_done_of(&events);
}

/// 正向：每轮重读——两轮假流 + 首轮流后改写 `AGENT.md` → 两次请求的 System 均
/// 为合并体，次轮的 `AGENT.md` 段为改后内容、知识段与首轮逐字相同（知识来自会话
/// 注入不重读盘）。
#[tokio::test]
async fn run系统消息合并每轮重读知识段不重读盘() {
    let dir = tempdir("system-merge-reload");
    let agent_md = dir.path().join("AGENT.md");
    write_file(&agent_md, "第一版约定");
    write_file(&dir.path().join("README.md"), "占位内容");
    let fake = FakeModel::with_turns(vec![
        vec![
            raw_tool_call("tu_sys", "read", serde_json::json!({ "path": "README.md" })),
            raw_final(usage(3, 3)),
        ],
        vec![raw_text("完成"), raw_final(usage(4, 4))],
    ]);
    fake.rewrite_file_on_first_stream(agent_md, "第二版约定 🚀");
    let turn = LoopTurn {
        injections: agent::SessionInjections {
            preamble: Some("架构知识段".to_owned()),
            tools: None,
            toolset: None,
        },
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&fake, turn).await;

    let requests = fake.captured_requests();
    assert_eq!(requests.len(), 2, "两轮请求（工具续轮）");
    assert_eq!(
        requests[0].system_instructions(),
        Some("架构知识段\n\n第一版约定"),
        "首轮：知识 + 首版 AGENT.md"
    );
    assert_eq!(
        requests[1].system_instructions(),
        Some("架构知识段\n\n第二版约定 🚀"),
        "次轮：知识段逐字相同、AGENT.md 段取自重读后的磁盘内容"
    );
    assert_eq!(sealed_messages(&events).len(), 2, "续轮不中断");
    turn_done_of(&events);
}

// ---------------------------------------------------------------------------
// archi 会话面（`source = "archi"` 注入：七工具全量 + 两 archi 工具双开）：下发面
// / 档位 / 直改主仓写盘 / 写坏不 panic 且可自修
//
// 装置沿既有 archi 章节：真实 tempdir 模型 + 真实 `ArchiContext`（读面即被测语
// 义），`tools::definitions` / `policy` / `protect` 全真实组合不 mock。
// ---------------------------------------------------------------------------

/// archi 会话面的注入底座（`source = "archi"` 臂的逐字段形态：七工具全量 +
/// 双开 + 知识 preamble）。
fn archi_session_injections(knowledge: Option<&str>) -> agent::SessionInjections {
    agent::SessionInjections {
        preamble: knowledge.map(str::to_owned),
        tools: None,
        toolset: Some(SessionToolset::Archi {
            module: true,
            search: true,
        }),
    }
}

/// 正向（AC-6）：archi 面 `RunStarted.tools` = `TOOL_NAMES` 七名（含 write /
/// edit / bash，保序）+ 两 archi 工具，且与实际 `request.tools` 逐名一致。
#[tokio::test]
async fn archi面runstarted_tools报七工具加archi两工具() {
    let dir = archi_fixture("archi-face-tools");
    let model = FakeModel::with_turns(vec![vec![raw_text("完成"), raw_final(usage(1, 1))]]);
    let turn = LoopTurn {
        injections: archi_session_injections(Some("架构知识")),
        archi: Some(ArchiContext::new(dir.path().to_path_buf())),
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    let mut expected: Vec<String> = TOOL_NAMES.iter().map(|name| (*name).to_owned()).collect();
    expected.push(ARCHI_MODULE_TOOL.to_owned());
    expected.push(ARCHI_SEARCH_TOOL.to_owned());
    assert_eq!(
        run_started_tools(&events),
        expected,
        "archi 面下发面 = 七通用工具保序 + archi_module + archi_search"
    );

    let requests = model.captured_requests();
    let dispatched: Vec<String> = requests[0]
        .tools
        .iter()
        .map(|definition| definition.name.clone())
        .collect();
    assert_eq!(
        dispatched,
        run_started_tools(&events),
        "request.tools 与 RunStarted.tools 同源（definitions 派生）"
    );
    assert!(
        dispatched.iter().any(|name| name == "write"),
        "archi 面保留写面（write 在全量七工具内）: {dispatched:?}"
    );
    turn_done_of(&events);
}

/// 回归（AC-6）：`default` 档下两 archi 工具放行、写面被 policy 拒；`acceptEdits` /
/// `bypassPermissions` 档写面放行（三档决策表归既有 `policy_test`，本节只锁
/// 「archi 面下同一判据」）。
#[tokio::test]
async fn archi面档位两工具放行写面按档判() {
    let dir = archi_fixture("archi-face-default");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call(
                "tu_read",
                ARCHI_MODULE_TOOL,
                serde_json::json!({ "element": "App.Auth" }),
            ),
            raw_tool_call(
                "tu_write",
                "write",
                serde_json::json!({
                    "path": "openspec/architecture/models/99-denied.toml",
                    "content": "[element.\"Denied\"]\nkind = \"module\"\n",
                }),
            ),
            raw_final(usage(3, 3)),
        ],
        vec![raw_text("已按档位继续")],
    ]);
    let turn = LoopTurn {
        injections: archi_session_injections(None),
        archi: Some(ArchiContext::new(dir.path().to_path_buf())),
        ..loop_turn(dir.path(), AgentPermissionMode::Default)
    };

    let events = drive_loop_turn(&model, turn).await;

    let notices: Vec<&str> = notices_of(&events).iter().map(|(name, _)| *name).collect();
    assert_eq!(
        notices,
        vec!["permission_denied"],
        "default 档：唯写面被拒（两 archi 工具零记因 = 放行）"
    );
    let results = tool_results_of(&events);
    let archi_result = results
        .iter()
        .find(|(id, _, _)| *id == "tu_read")
        .expect("archi 工具结果在场");
    assert!(!archi_result.2, "default 档 archi 工具放行（执行体承接）");
    let write_result = results
        .iter()
        .find(|(id, _, _)| *id == "tu_write")
        .expect("写面结果在场");
    assert!(write_result.2, "default 档写面被拒（is_error 回灌）");
    assert!(
        !dir.path()
            .join("openspec/architecture/models/99-denied.toml")
            .exists(),
        "被拒写面零落盘"
    );

    for mode in [
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        let dir = archi_fixture(&format!("archi-face-write-{mode:?}"));
        let model = FakeModel::with_turns(vec![
            vec![
                raw_tool_call(
                    "tu_write",
                    "write",
                    serde_json::json!({
                        "path": "openspec/architecture/models/98-allowed.toml",
                        "content": "[element.\"Allowed\"]\nkind = \"module\"\n",
                    }),
                ),
                raw_final(usage(2, 2)),
            ],
            vec![raw_text("写完")],
        ]);
        let turn = LoopTurn {
            injections: archi_session_injections(None),
            archi: Some(ArchiContext::new(dir.path().to_path_buf())),
            ..loop_turn(dir.path(), mode)
        };

        let events = drive_loop_turn(&model, turn).await;

        assert!(
            notices_of(&events).is_empty(),
            "{mode:?} 档：写面无拒绝记因，实际: {:?}",
            notices_of(&events)
        );
        let results = tool_results_of(&events);
        assert!(!results[0].2, "{mode:?} 档：写面放行（is_error = false）");
        assert!(
            dir.path()
                .join("openspec/architecture/models/98-allowed.toml")
                .exists(),
            "{mode:?} 档：文件落盘"
        );
    }
}

/// 正向（AC-7）：archi 面（`tools = None`）在 `bypassPermissions` 档直改主仓——
/// `write` 新建 `<archi_dir>/models/09-x.toml`、`edit` 改既有 `gate` 行 → 工具结果
/// `is_error = false`、文件落盘、写入保护零命中；随后 `archi_module` 读到改动。
#[tokio::test]
async fn archi面write新建与edit改gate行落盘且保护零拦截() {
    let dir = archi_fixture("archi-face-write-path");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call(
                "tu_new",
                "write",
                serde_json::json!({
                    "path": "openspec/architecture/models/09-x.toml",
                    "content": "[element.\"App.Auth.Token\"]\nkind = \"component\"\n",
                }),
            ),
            raw_tool_call(
                "tu_edit",
                "edit",
                serde_json::json!({
                    "path": "openspec/architecture/models/01-core.toml",
                    "old_string": "gate = \"strict\"",
                    "new_string": "gate = \"warn\"",
                }),
            ),
            raw_tool_call(
                "tu_check",
                ARCHI_MODULE_TOOL,
                serde_json::json!({ "element": "App.Auth" }),
            ),
            raw_final(usage(4, 4)),
        ],
        vec![raw_text("已改模型")],
    ]);
    let turn = LoopTurn {
        injections: archi_session_injections(None),
        archi: Some(ArchiContext::new(dir.path().to_path_buf())),
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    assert!(
        notices_of(&events).is_empty(),
        "直改主仓径路零拦截记因（<archi_dir>/models/** 不在内置锁面），实际: {:?}",
        notices_of(&events)
    );
    let results = tool_results_of(&events);
    assert_eq!(results.len(), 3, "三工具各一条结果");
    for (id, _, is_error) in &results {
        assert!(!*is_error, "{id}: 写面 / 读面均 is_error = false");
    }
    let created = dir.path().join("openspec/architecture/models/09-x.toml");
    assert!(created.exists(), "write 新建文件落盘");
    let edited =
        std::fs::read_to_string(dir.path().join("openspec/architecture/models/01-core.toml"))
            .expect("读既有模型文件");
    assert!(
        edited.contains("gate = \"warn\"") && !edited.contains("gate = \"strict\""),
        "edit 改既有 gate 行成功，实际文件:\n{edited}"
    );
    let check = results
        .iter()
        .find(|(id, _, _)| *id == "tu_check")
        .expect("archi_module 结果在场");
    let payload: serde_json::Value = serde_json::from_str(check.1)
        .unwrap_or_else(|error| panic!("读面结果应为 dto JSON: {error}"));
    assert_eq!(
        payload["element"]["gate"],
        serde_json::json!("warn"),
        "读面读到改动后的 gate 档"
    );
    turn_done_of(&events);
}

/// 异常（AC-8）：写入坏模型（非法 TOML）后调 `archi_module` → 工具结果
/// `is_error = true` 且文案含 `file:line` 结构化诊断；写入本身零 `Err`（无写前
/// 校验插口）；run 不中断（后续续轮正常收敛）。
#[tokio::test]
async fn archi面写坏模型读面is_error带file_line且run不中断() {
    let dir = archi_fixture("archi-face-broken");
    let model = FakeModel::with_turns(vec![
        vec![
            raw_tool_call(
                "tu_bad",
                "write",
                serde_json::json!({
                    "path": "openspec/architecture/models/02-bad.toml",
                    "content": "[element.\"Broken\"\nkind = \"module\"\n",
                }),
            ),
            raw_tool_call("tu_diag", ARCHI_MODULE_TOOL, serde_json::json!({})),
            raw_final(usage(3, 3)),
        ],
        vec![raw_text("已按诊断修复指引继续")],
    ]);
    let turn = LoopTurn {
        injections: archi_session_injections(None),
        archi: Some(ArchiContext::new(dir.path().to_path_buf())),
        ..loop_turn(dir.path(), AgentPermissionMode::BypassPermissions)
    };

    let events = drive_loop_turn(&model, turn).await;

    let results = tool_results_of(&events);
    assert_eq!(results.len(), 2, "写 / 读各一条结果");
    let written = results
        .iter()
        .find(|(id, _, _)| *id == "tu_bad")
        .expect("写结果在场");
    assert!(!written.2, "写入本身零 Err（无新增写前校验插口）");
    assert!(
        dir.path()
            .join("openspec/architecture/models/02-bad.toml")
            .exists(),
        "坏模型照常落盘（写入面不校验）"
    );

    let diagnostic = results
        .iter()
        .find(|(id, _, _)| *id == "tu_diag")
        .expect("读面结果在场");
    assert!(diagnostic.2, "坏模型读面以 is_error 回灌（不 panic）");
    assert!(
        diagnostic.1.contains("02-bad.toml"),
        "诊断文案含文件名，实际: {}",
        diagnostic.1
    );
    assert!(
        diagnostic
            .1
            .lines()
            .any(|line| line.contains("02-bad.toml:")),
        "诊断文案含 file:line 结构化位置，实际: {}",
        diagnostic.1
    );

    assert_eq!(sealed_messages(&events).len(), 2, "写坏后续轮不中断");
    let AgentEventKind::TurnDone { is_error, .. } = turn_done_of(&events) else {
        panic!("应为 TurnDone");
    };
    assert!(!*is_error, "坏模型不炸 run（正常收敛）");
}
