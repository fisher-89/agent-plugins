//! worker 全链用例（sdk 引擎承载）：假 sink + `DualEnv` 真库 +
//! `KernelWorkerPort`。CLI 租户退役——PATH 隔离 / 假 CLI shim /
//! `TEST_PATH_LOCK` 串行装置与 CliMissing 收敛用例随 cli 档用例群废弃
//! （无消费者即删）；sdk 轮全程无 PATH 轮前预检（`discover` 已随 `cli/`
//! 目录删除，预检分支删除回归经本文件全链用例保绿锚定）。

use std::path::Path;
use std::sync::{Arc, Mutex};

use agent::{
    AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunStatus, ModelLevel,
    SessionProvenance, SessionStreamRegistry, StopRegistry,
};
use orchestration::port::{AskPort, RunEventSink, WorkerAgentPort, WorkerRole, WorkerTurnRequest};
use orchestration::state::{AskPayload, RunUpdate};
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, SessionConfigSnapshot, SessionRecord, WorkspaceStores,
};

/// 父模块导入面（KernelWorkerPort / ComposedTurn 等经 worker 模块可见）。
use super::*;
use crate::compose_turn;

/// 会话 provenance 来源（与 walker 的 `SOURCE_CHANGE` 同一契约字面量）。
const SOURCE_CHANGE: &str = "change";

// ---------------------------------------------------------------------------
// 装置：tempdir 真库双环境 + 假 sink（双面捕获）
// ---------------------------------------------------------------------------

/// 数据根 + workspace 根双临时目录装置（stores 以 OnceLock 缓存——redb 单
/// 进程单句柄，重复 open 同一 db 文件不安全）。
struct DualEnv {
    data_root: tempfile::TempDir,
    ws_root: tempfile::TempDir,
    stores: std::sync::OnceLock<WorkspaceStores>,
}

impl DualEnv {
    fn new(tag: &str) -> Self {
        let data_root = tempfile::Builder::new()
            .prefix(&format!("worker-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("worker-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self {
            data_root,
            ws_root,
            stores: std::sync::OnceLock::new(),
        }
    }

    fn root(&self) -> String {
        self.ws_root.path().to_string_lossy().into_owned()
    }

    fn stores(&self) -> &WorkspaceStores {
        self.stores.get_or_init(|| {
            WorkspaceStores::open(self.data_root.path())
                .unwrap_or_else(|e| panic!("WorkspaceStores::open 应成功: {e}"))
        })
    }

    /// 预解析 workspace root 库实例（compose 拆参后的命令层注入面同构）。
    fn store(&self) -> std::sync::Arc<store::Store> {
        self.stores()
            .for_root(&self.root())
            .expect("for_root 应成功")
    }
}

/// 假 `RunEventSink`：双面捕获——Mutex Vec 终态断言面 + 无界通道实时唤醒面
///（测试任务挂起等待事件，调度器方可推进 IO / 退避定时器）。
struct CapturingSink {
    updates: Arc<Mutex<Vec<RunUpdate>>>,
    live: tokio::sync::mpsc::UnboundedSender<RunUpdate>,
}

/// 捕获面句柄（断言 + 实时等待）。
struct Captured {
    updates: Arc<Mutex<Vec<RunUpdate>>>,
    live_rx: tokio::sync::mpsc::UnboundedReceiver<RunUpdate>,
}

impl CapturingSink {
    /// 构造即铸 trait object（KernelWorkerPort 注入面），返回（sink, 捕获面）。
    fn capturing() -> (Arc<dyn RunEventSink>, Captured) {
        let updates = Arc::new(Mutex::new(Vec::new()));
        let (live_tx, live_rx) = tokio::sync::mpsc::unbounded_channel();
        (
            Arc::new(Self {
                updates: Arc::clone(&updates),
                live: live_tx,
            }),
            Captured { updates, live_rx },
        )
    }
}

impl RunEventSink for CapturingSink {
    fn emit(&self, update: RunUpdate) {
        let _ = self.live.send(update.clone());
        self.updates
            .lock()
            .expect("事件捕获锁不可中毒")
            .push(update);
    }
}

impl Captured {
    /// 全量已捕获的 SessionEvent 流（session_id, event）。
    fn events(&self) -> Vec<(String, agent::AgentEvent)> {
        session_events_of(&self.updates.lock().expect("事件捕获锁不可中毒"))
    }

    /// 阻塞等待首个会话事件透传（挂起测试任务——事件 IO / 退避定时器经调度
    /// 器推进；返回运行会话 id，即 begin 同步段完成锚）。
    async fn wait_first_session(&mut self) -> String {
        loop {
            let update = self.live_rx.recv().await.expect("run 事件流不应提前关闭");
            if let RunUpdate::SessionEvent { session_id, .. } = update {
                return session_id;
            }
        }
    }

    /// 阻塞等待「未见过」的新会话首事件（多轮顺序用例复用同一捕获面）。
    async fn wait_new_session(&mut self, seen: &[String]) -> String {
        loop {
            let candidate = self.wait_first_session().await;
            if !seen.contains(&candidate) {
                return candidate;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// fixture：sdk 默认实例 / sdk 快照会话行 / 最小密封转录
// ---------------------------------------------------------------------------

/// sdk 默认实例 fixture（provider 齐备——缺省解析可走通的唯一活档臂）。
fn seed_sdk_default(stores: &WorkspaceStores, name: &str) -> i64 {
    seed_sdk_at(stores, name, "http://127.0.0.1:9/v1")
}

/// 端点可指定的 sdk 默认实例 fixture（脚本化桩端点注入面）。
fn seed_sdk_at(stores: &WorkspaceStores, name: &str, base_url: &str) -> i64 {
    let provider = stores
        .global()
        .upsert_agent_provider(AgentProviderRecord::new(
            format!("{name}供应"),
            AgentProviderProtocol::Openai,
            base_url.to_owned(),
            "sk-worker".to_owned(),
            AgentModelTiers {
                high: "m-high".to_owned(),
                medium: "m-medium".to_owned(),
                low: "m-low".to_owned(),
            },
            None,
            None,
        ))
        .expect("落 provider fixture")
        .id;
    let id = stores
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            name.to_owned(),
            AgentEngineKind::Sdk,
            Some(provider),
        ))
        .expect("落 sdk 实例 fixture")
        .id;
    stores
        .global()
        .set_default_agent_instance(id)
        .expect("置默认实例 fixture");
    id
}

/// 空配置 sdk 默认实例 fixture（open 阶段 ConfigMissing 的唯一可达面）。
fn seed_sdk_default_with_blank_provider(stores: &WorkspaceStores) {
    let provider = stores
        .global()
        .upsert_agent_provider(AgentProviderRecord::new(
            "空配置供应".to_owned(),
            AgentProviderProtocol::Openai,
            String::new(),
            String::new(),
            AgentModelTiers {
                high: String::new(),
                medium: String::new(),
                low: String::new(),
            },
            None,
            None,
        ))
        .expect("落空配置 provider fixture")
        .id;
    let id = stores
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            "空配置实例".to_owned(),
            AgentEngineKind::Sdk,
            Some(provider),
        ))
        .expect("落 sdk 实例 fixture")
        .id;
    stores
        .global()
        .set_default_agent_instance(id)
        .expect("置默认实例 fixture");
}

/// sdk 快照会话行 fixture（Continue 引用底座）。
fn seed_sdk_session(store: &store::Store, id: &str, source_ref: Option<&str>) {
    store
        .create_session(&SessionRecord {
            id: id.to_owned(),
            engine_session_id: Some("sdk-legacy-1727000000001".to_owned()),
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                agent_instance_id: Some(7),
                model: Some("m-high".to_owned()),
                permission_mode: AgentPermissionMode::BypassPermissions,
            },
            source: SOURCE_CHANGE.to_owned(),
            source_ref: source_ref.map(str::to_owned),
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("落 sdk 会话行 fixture");
}

/// 最小密封转录 fixture（Continue 全史重建非空的前提）。
fn seed_transcript(store: &store::Store, session_id: &str) {
    let transcript = vec![
        agent::AgentEvent::stamp(
            None,
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
        agent::AgentEvent::stamp(
            None,
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![AgentBlock::Text {
                    text: "上一轮答".to_owned(),
                    citations: None,
                }],
                parent_tool_use_id: None,
            },
        ),
    ];
    store
        .append_session_events(session_id, &transcript)
        .expect("落转录 fixture");
}

/// change 来源的轮请求 fixture（sourceRef 按 `<change>/<phase>/<role>/<attempt>`
/// 定式入参）。
fn turn_request(root: &str, source_ref: &str, role: WorkerRole) -> WorkerTurnRequest {
    WorkerTurnRequest {
        root: root.to_owned(),
        prompt: "run one turn".to_owned(),
        provenance: SessionProvenance {
            source: SOURCE_CHANGE.to_owned(),
            source_ref: Some(source_ref.to_owned()),
        },
        permission: AgentPermissionMode::BypassPermissions,
        model_level: ModelLevel::High,
        continue_session: None,
        agent: None,
        role,
    }
}

/// 摘取 sink 捕获中的 SessionEvent 流（session_id, event）。
fn session_events_of(updates: &[RunUpdate]) -> Vec<(String, agent::AgentEvent)> {
    updates
        .iter()
        .filter_map(|update| match update {
            RunUpdate::SessionEvent { session_id, event } => {
                Some((session_id.clone(), event.clone()))
            }
            _ => None,
        })
        .collect()
}

/// 等待首个会话事件并从捕获面取出会话 id（= ask 注册表键 / core 铸 id）。
async fn wait_first_session_id(updates: &Arc<Mutex<Vec<RunUpdate>>>) -> String {
    for _ in 0..3000 {
        if let Some(RunUpdate::SessionEvent { session_id, .. }) = updates
            .lock()
            .expect("事件捕获锁不可中毒")
            .iter()
            .find(|update| matches!(update, RunUpdate::SessionEvent { .. }))
        {
            return session_id.clone();
        }
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    panic!("等待首个会话事件超时");
}

/// 轮询至注册表受理应答（登记点由执行体异步抵达；未登记时文案稳定同值）。
async fn answer_until_accepted(registry: &Arc<agent::AskRegistry>, session_id: &str, text: &str) {
    for _ in 0..3000 {
        match registry.answer(session_id, text.to_owned()) {
            Ok(()) => return,
            Err(error) => {
                assert_eq!(error, "当前无等待中的 ask", "仅登记未完成时重试: {error}");
                tokio::task::yield_now().await;
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        }
    }
    panic!("等待 ask 登记超时（session={session_id}）");
}

/// 转录内某 `tool_use` 的配对 `tool_result`（应答回灌判据）。
fn transcript_tool_result(
    transcript: &[agent::AgentEvent],
    tool_call_id: &str,
) -> Option<(String, bool)> {
    transcript.iter().find_map(|event| match &event.kind {
        AgentEventKind::Message { blocks, .. } => blocks.iter().find_map(|block| match block {
            AgentBlock::ToolResult {
                id,
                content,
                is_error,
            } if id == tool_call_id => Some((content.clone(), *is_error)),
            _ => None,
        }),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// 装配半边：provenance / permission / 实例外键（AC-7）
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn change来源与permission装配入会话行_engine恒sdk携实例id() {
    let env = DualEnv::new("provenance");
    let stores = env.stores();
    let default_id = seed_sdk_default(stores, "装配实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, mut captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();
    let request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);

    let task = tokio::spawn(async move { port.run(request).await });
    // 首个会话事件透传即 begin 已完成（会话行按 provenance 定式落库），随后
    // 置位停止经退避窗口收敛
    let session_id = captured.wait_first_session().await;
    assert!(
        registry.request_stop(&session_id),
        "begin 同步段已登记停止句柄"
    );

    let outcome = task
        .await
        .expect("run 任务正常结束")
        .expect("装配会话应发起");
    assert_eq!(
        outcome.status,
        AgentRunStatus::Stopped,
        "停止置位后驱动终止以 stopped 收敛"
    );
    assert!(
        outcome.finalized.is_none(),
        "中止会话零收口载荷（finalized 缺席）"
    );

    let store = stores.for_root(&root).expect("for_root 应成功");
    let row = store
        .find_session(&outcome.session_id)
        .expect("find_session 应成功")
        .expect("会话行已落库");
    assert_eq!(
        row.source, SOURCE_CHANGE,
        "provenance source 恒 change（AC-7 反查面）"
    );
    assert_eq!(
        row.source_ref.as_deref(),
        Some("chg-flow/implement/executor/1"),
        "sourceRef 按 <change>/<phase>/<role>/<attempt> 定式落库"
    );
    assert_eq!(
        row.config_snapshot.permission_mode,
        AgentPermissionMode::BypassPermissions,
        "permission 恒 bypassPermissions"
    );
    assert_eq!(
        row.config_snapshot.engine,
        AgentEngineKind::Sdk,
        "sdk 唯一活档装配（CLI 租户退役后 engine 恒 sdk）"
    );
    assert_eq!(
        row.config_snapshot.agent_instance_id,
        Some(default_id),
        "快照携解析实例外键（DD8）"
    );
}

#[tokio::test(start_paused = true)]
async fn continue会话复用既有行不重复建() {
    let env = DualEnv::new("continue");
    let stores = env.stores();
    seed_sdk_default(stores, "续话实例");
    let registry = Arc::new(StopRegistry::new());
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(
        &store,
        "ses-flow-continue",
        Some("chg-flow/proposal/executor/0"),
    );
    seed_transcript(&store, "ses-flow-continue");
    let (sink, mut captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();

    let mut request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);
    request.continue_session = Some("ses-flow-continue".to_owned());

    let task = tokio::spawn(async move { port.run(request).await });
    // 首个会话事件透传即 Continue 会话已发起，随后经停止窗口收敛
    let event_session_id = captured.wait_first_session().await;
    assert!(
        registry.request_stop(&event_session_id),
        "begin 同步段已登记停止句柄"
    );

    let outcome = task
        .await
        .expect("run 任务正常结束")
        .expect("Continue 引用应复用既有会话行");

    assert_eq!(
        outcome.session_id, "ses-flow-continue",
        "continue_session Some → Continue 引用（沿用既有会话 id）"
    );
    let summaries = store.list_sessions(None, None).expect("清单应成功");
    assert_eq!(summaries.len(), 1, "Continue 不重复建会话行");
    assert_eq!(summaries[0].turns.len(), 1, "续轮行照开");
}

// ---------------------------------------------------------------------------
// 解析失败显式 Err（不静默 / 零半成品）
// ---------------------------------------------------------------------------

#[test]
fn 空库缺省解析err引导管理页且零落库() {
    let env = DualEnv::new("no-default");
    let stores = env.stores();

    let error = match compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    ) {
        Err(error) => error,
        Ok(_) => panic!("空库缺省解析必须 Err"),
    };
    assert!(
        error.contains("默认 agent") && error.contains("/agents"),
        "Err 携管理页引导语义（显式失败不静默），实际: {error}"
    );

    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None,)
            .expect("清单应成功")
            .is_empty(),
        "解析失败零落库"
    );
}

#[tokio::test]
async fn open段配置缺失显式err传播且零半成品记录() {
    // ConfigMissing 归位口径（AC-1 预检删除回归的错误面半边）：provider 配置
    // 缺失归 begin() open 段显式 Err——错误面不移位、不静默、零半成品记录
    let env = DualEnv::new("open-fail");
    let stores = env.stores();
    seed_sdk_default_with_blank_provider(stores);
    let (sink, _captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);

    let result = port
        .run(turn_request(
            &env.root(),
            "chg-flow/implement/executor/1",
            WorkerRole::Executor,
        ))
        .await;

    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("open 阶段失败必须 Err 传播"),
    };
    assert!(
        error.contains("配置缺失"),
        "AgentStartError 以 Display 映射 Err 通道（启动失败单一变体），实际: {error}"
    );

    // 内核契约承接：open 阶段失败不产生任何记录
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None,)
            .expect("清单应成功")
            .is_empty(),
        "open 阶段失败零半成品记录"
    );
}

// ---------------------------------------------------------------------------
// 泵收集与透传（sdk 拒连端点一轮一命；AC-7 反查数据面 / AC-5 实时流来源）
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn 泵收集密封转录并透传会话事件_拒连一轮一命收敛() {
    let env = DualEnv::new("pump");
    let stores = env.stores();
    seed_sdk_default(stores, "泵收集实例");
    let (sink, captured) = CapturingSink::capturing();
    let registry = Arc::new(StopRegistry::new());
    let composed: ComposedTurn = compose_turn(
        stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();
    let request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);

    let task = tokio::spawn(async move { port.run(request).await });
    // 自然收敛：拒连端点重试护栏逐次留痕后耗尽（paused 时钟快进退避），一轮
    // 一命收口
    let outcome = task
        .await
        .expect("run 任务正常结束")
        .expect("sdk 会话应正常发起");

    // 终态与转录：密封事件全集收集（RunStarted / user / api_retry ×3 /
    // api_error / TurnDone 全部密封——增量以外皆落转录）
    assert_eq!(outcome.status, AgentRunStatus::Failed, "拒连端点失败收敛");
    assert!(
        outcome.finalized.is_none(),
        "失败收敛零 finish_step notice → finalized 缺席（Option 缺省形态）"
    );
    let transcript = &outcome.transcript;
    assert_eq!(
        transcript.len(),
        7,
        "RunStarted / user / api_retry ×3 / api_error / TurnDone 全集"
    );
    assert!(matches!(
        &transcript[0].kind,
        AgentEventKind::RunStarted { .. }
    ));
    assert!(
        matches!(&transcript[1].kind, AgentEventKind::Message { role, .. }
        if role == &AgentMessageRole::User)
    );
    assert!(
        matches!(&transcript[2].kind, AgentEventKind::SystemNotice { subtype, .. }
        if subtype == "api_retry")
    );
    assert!(
        matches!(&transcript[5].kind, AgentEventKind::SystemNotice { subtype, .. }
        if subtype == "api_error")
    );
    assert!(matches!(
        &transcript[6].kind,
        AgentEventKind::TurnDone { is_error: true, .. }
    ));

    // sink 透传：全量观察以 RunUpdate::SessionEvent 流出且与收集转录同源保真
    let events = captured.events();
    assert_eq!(events.len(), transcript.len(), "观察全量透传");
    assert!(
        events
            .iter()
            .all(|(session_id, _)| session_id == &outcome.session_id),
        "透传 session_id 承接运行会话"
    );
    assert_eq!(
        events
            .into_iter()
            .map(|(_, event)| event)
            .collect::<Vec<_>>(),
        *transcript,
        "透传事件与收集转录同源保真"
    );

    // 真库落盘：change provenance 反查命中 + 轮行终态 + 双 id 映射
    let store = stores.for_root(&root).expect("for_root 应成功");
    let summaries = store
        .list_sessions(Some(SOURCE_CHANGE), Some("chg-flow/implement/executor/1"))
        .expect("反查应成功");
    assert_eq!(
        summaries.len(),
        1,
        "source='change' + sourceRef 精确反查命中"
    );
    assert_eq!(summaries[0].turns[0].status, AgentRunStatus::Failed);
    let remote = summaries[0]
        .row
        .remote_session_id
        .as_deref()
        .expect("引擎侧标识上报落双 id 映射");
    assert!(
        remote.starts_with("sdk-"),
        "sdk 引擎侧标识每轮铸造上报，实际: {remote}"
    );
}

// ---------------------------------------------------------------------------
// `finalized` 出口（agent-finalize-tool AC-6）：字段由 `worker.rs` 自转录填充，
// 提取实现单点在 `orchestration::transcript::finish_payload`
// ---------------------------------------------------------------------------

/// 收口 notice 事件（`SystemNotice{finish_step}`——`finalized` 的唯一来源面）。
fn finish_notice(payload: serde_json::Value) -> AgentEventKind {
    AgentEventKind::SystemNotice {
        subtype: orchestration::finish::TOOL_NAME.to_owned(),
        payload,
    }
}

/// assistant 文本事件（`final_message` 的取数面，与 `finalized` 互不推导）。
fn assistant_text(text: &str) -> AgentEventKind {
    AgentEventKind::Message {
        role: AgentMessageRole::Assistant,
        blocks: vec![AgentBlock::Text {
            text: text.to_owned(),
            citations: None,
        }],
        parent_tool_use_id: None,
    }
}

/// 正常收敛终局事件（`is_error: false`）。
fn turn_done_ok() -> AgentEventKind {
    AgentEventKind::TurnDone {
        subtype: "success".to_owned(),
        is_error: false,
        num_turns: Some(1),
        duration_ms: Some(1),
        cost_usd: None,
        usage: serde_json::json!({}),
        session_id: None,
    }
}

/// 脚本化假 runner：按序投递预录观察事件后关闭通道——`KernelWorkerPort` 的真实
/// 链路（内核盖戳 / 落库 / 收口）零改动，只替换引擎侧事件来源（收口 notice 的
/// 注入面；真实 `compose_turn` 恒装 `SdkRunner`，无法预录）。
struct ScriptedRunner {
    script: Mutex<Vec<AgentEventKind>>,
}

impl ScriptedRunner {
    fn new(script: Vec<AgentEventKind>) -> Arc<Self> {
        Arc::new(Self {
            script: Mutex::new(script),
        })
    }
}

impl agent::AgentRunner for ScriptedRunner {
    fn open_session(
        &self,
        _open: agent::SessionOpen,
    ) -> Result<agent::AgentSession, agent::AgentStartError> {
        let script = std::mem::take(&mut *self.script.lock().expect("脚本锁不可中毒"));
        let (observation_tx, observation_rx) = tokio::sync::mpsc::channel(script.len().max(1) + 4);
        let (question_tx, mut question_rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let _ = question_rx.recv().await;
            for kind in script {
                if observation_tx.send(kind).await.is_err() {
                    break;
                }
            }
        });
        Ok(agent::AgentSession {
            observations: observation_rx,
            questions: question_tx,
            handle: agent::RunHandle::default(),
        })
    }
}

/// 驱动一轮脚本化会话（收口载荷注入面）：真库 `DualEnv` + 假 sink + 脚本
/// runner → `KernelWorkerPort::run` 的真实产出。
async fn run_scripted(
    tag: &str,
    script: Vec<AgentEventKind>,
) -> orchestration::port::WorkerTurnOutcome {
    let env = DualEnv::new(tag);
    let (sink, _captured) = CapturingSink::capturing();
    let composed = crate::compose::composed_with_runner(env.store(), ScriptedRunner::new(script));
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();
    port.run(turn_request(
        &root,
        "chg-flow/implement/evaluator/1",
        WorkerRole::Evaluator,
    ))
    .await
    .expect("脚本化会话应正常发起")
}

/// `finalized` 提取贯通（正向，AC-6）：转录含 `SystemNotice{finish_step}` 的
/// evaluator 载荷 → `WorkerTurnOutcome.finalized == Some(Evaluator(..))` 且逐字段
/// 相等；同轮 `final_message` 仍为末条 assistant 文本（两字段互不推导）。
#[tokio::test]
async fn finalized提取贯通_evaluator载荷逐字段相等() {
    let outcome = run_scripted(
        "finalized-present",
        vec![
            assistant_text("评估结论：见报告正文"),
            finish_notice(serde_json::json!({
                "evaluator": {
                    "verdict": "fail",
                    "report": "评估报告正文",
                    "checklist": [{ "item": "覆盖缺口", "pass": false, "evidence": "缺失行" }],
                }
            })),
            turn_done_ok(),
        ],
    )
    .await;

    let finalized = outcome.finalized.as_ref().expect("收口 notice 在案即 Some");
    let orchestration::finish::FinishPayload::Evaluator(checklist) = finalized else {
        panic!("evaluator 形态载荷，实际: {finalized:?}");
    };
    assert_eq!(
        checklist.verdict.as_str(),
        "fail",
        "verdict 逐字段相等（线词 fail）"
    );
    assert_eq!(checklist.report, "评估报告正文", "report 逐字段相等");
    assert_eq!(
        checklist.checklist[0].item, "覆盖缺口",
        "checklist 逐字段相等"
    );
    assert_eq!(
        outcome.final_message.as_deref(),
        Some("评估结论：见报告正文"),
        "final_message 仍承载末条 assistant 文本（与 finalized 互不推导）"
    );
}

/// decision 形态贯通（正向，AC-6）：决策会话的收口载荷走同一提取面，`finalized`
/// 两臂可辨。
#[tokio::test]
async fn finalized提取贯通_decision形态两臂可辨() {
    let outcome = run_scripted(
        "finalized-decision",
        vec![
            finish_notice(serde_json::json!({
                "decision": { "action": "stop", "reason": "用户裁决终止" }
            })),
            turn_done_ok(),
        ],
    )
    .await;

    assert_eq!(
        outcome.finalized,
        Some(orchestration::finish::FinishPayload::Decision(
            orchestration::decision::DecisionAction::Stop {
                reason: "用户裁决终止".to_owned(),
            }
        )),
        "decision 臂逐字段相等"
    );
}

/// 损坏载荷 → `None` 且 run 不报错（异常，AC-6）：`finish_step` notice 的
/// `payload` 为 `null` / `{}` → `finalized == None`、`run` 仍 `Ok`（提取的宽容性
/// 由 `transcript::finish_payload` 保证，worker 侧不把载荷提取失败升级为 `Err`）。
#[tokio::test]
async fn finalized载荷损坏两形态返回none且不err() {
    for (tag, payload) in [
        ("null", serde_json::Value::Null),
        ("empty-object", serde_json::json!({})),
    ] {
        let outcome = run_scripted(
            &format!("finalized-broken-{tag}"),
            vec![
                assistant_text("正常文本"),
                finish_notice(payload),
                turn_done_ok(),
            ],
        )
        .await;
        assert_eq!(
            outcome.finalized, None,
            "{tag}: 损坏载荷 → None（不 panic、不外溢 Err）"
        );
        assert_eq!(
            outcome.status,
            AgentRunStatus::Completed,
            "{tag}: 会话仍以正常收敛收口（工作面无新增 Err 分支）"
        );
    }
}

/// `finalized` 与转录提取单点一致（正向，回归锚定）：全链产出的
/// `WorkerTurnOutcome.finalized` 恒等于对同一转录调提取单点的结果——两处零
/// 第二份提取实现（本条同时是全链用例的旁证断言，不改其既有断言语义）。
#[tokio::test(start_paused = true)]
async fn finalized恒等于转录提取单点结果() {
    let env = DualEnv::new("finalized-single-point");
    let stores = env.stores();
    seed_sdk_default(stores, "收口实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, _captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();
    let request = turn_request(
        &root,
        "chg-flow/implement/evaluator/1",
        WorkerRole::Evaluator,
    );

    let outcome = tokio::spawn(async move { port.run(request).await })
        .await
        .expect("run 任务正常结束")
        .expect("会话应正常发起");

    assert_eq!(
        outcome.finalized,
        orchestration::transcript::finish_payload(&outcome.transcript),
        "finalized 恒等于提取单点结果（worker 侧零第二份提取实现）"
    );
    assert!(
        outcome.finalized.is_none(),
        "本轮转录零 finish_step notice → 缺席"
    );
    // `final_message` 与 `finalized` 互不推导（前者是末条 assistant 文本）
    assert!(
        outcome.final_message.is_none() || outcome.finalized.is_none(),
        "两字段独立：final_message 有值不蕴含 finalized 有值"
    );
}

/// 提取实现单点（源码自证，异常）：`worker.rs` 内 `finish_payload` 调用恰一次、
/// `from_value::<FinishPayload>` 零命中（MUST NOT 另写提取）。
#[test]
fn 提取实现单点_worker零第二份实现() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/worker.rs"),
    )
    .expect("读取 worker.rs");
    assert_eq!(
        own.matches("finish_payload").count(),
        1,
        "worker.rs 内 finish_payload 调用恰一处"
    );
    // 第二份提取实现判据（形态不敏感）：同一行同时出现 `from_value` 与
    // `FinishPayload` 即「另写了一份提取」（裸调 / 带路径标注 / 带类型参数三形态
    // 一并命中）。
    let from_value = ["from", "_value"].concat();
    let payload_type = ["Finish", "Payload"].concat();
    let duplicated: Vec<&str> = own
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains(&from_value) && line.contains(&payload_type))
        .collect();
    assert!(
        duplicated.is_empty(),
        "worker.rs 内零第二份提取实现（MUST NOT 各写一份）: {duplicated:?}"
    );
    assert!(own.contains("finalized"), "finalized 字段由 worker.rs 填充");
}

// ---------------------------------------------------------------------------
// 停止收敛（AC-7 停止半边）
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn 中途停止收敛stopped并保留已收集事件() {
    let env = DualEnv::new("stop");
    let stores = env.stores();
    seed_sdk_default(stores, "停止实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, mut captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();
    let request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);

    let task = tokio::spawn(async move { port.run(request).await });
    // 首个会话事件透传即取得运行会话槽（命令层 sink 桥的同步半边），随后置位停止
    let session_id = captured.wait_first_session().await;
    assert!(
        registry.request_stop(&session_id),
        "begin 同步段已登记停止句柄（键 = core session id）"
    );

    let outcome = task
        .await
        .expect("run 任务正常结束")
        .expect("停止收敛仍 Ok 携终态");

    assert_eq!(
        outcome.status,
        AgentRunStatus::Stopped,
        "停止置位后驱动终止以 stopped 收敛"
    );
    assert!(
        outcome.finalized.is_none(),
        "中止会话零收口载荷（finalized 缺席）"
    );
    assert!(
        matches!(
            &outcome.transcript[0].kind,
            AgentEventKind::RunStarted { .. }
        ),
        "已收集事件保留（停止不清除）"
    );
    assert!(
        !registry.request_stop(&session_id),
        "终态除名后停止寻址 miss"
    );
    let events = captured.events();
    assert!(
        events.iter().any(|(id, event)| id == &outcome.session_id
            && matches!(event.kind, AgentEventKind::RunStarted { .. })),
        "停止前的观察已透传 sink（实时流不丢）"
    );
}

// ---------------------------------------------------------------------------
// 三角色 provenance 定式（D3 / AC-7 反查面）
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn 三角色provenance定式逐段落库且精确反查命中() {
    let env = DualEnv::new("roles");
    let stores = env.stores();
    seed_sdk_default(stores, "角色实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, mut captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let port = Arc::new(KernelWorkerPort::new(composed, sink));
    let root = env.root();

    let roles = [
        ("chg-flow/implement/executor/1", WorkerRole::Executor),
        ("chg-flow/implement/evaluator/2", WorkerRole::Evaluator),
        ("chg-flow/implement/decision/3", WorkerRole::Decision),
    ];

    let mut seen: Vec<String> = Vec::new();
    for (source_ref, role) in roles {
        let request = turn_request(&root, source_ref, role);
        let task = tokio::spawn({
            let port = Arc::clone(&port);
            async move { port.run(request).await }
        });
        // 本轮「未见会话」首事件透传即本轮 begin 已完成，随后经停止窗口收敛
        let session_id = captured.wait_new_session(&seen).await;
        seen.push(session_id.clone());
        assert!(
            registry.request_stop(&session_id),
            "begin 同步段已登记停止句柄"
        );
        let outcome = task
            .await
            .expect("run 任务正常结束")
            .expect("各角色会话均应发起");
        assert!(outcome.session_id.starts_with("ses-"), "每轮独立新会话");
    }

    let store = stores.for_root(&root).expect("for_root 应成功");
    let summaries = store
        .list_sessions(Some(SOURCE_CHANGE), None)
        .expect("反查应成功");
    assert_eq!(summaries.len(), 3, "三角色各建一会话");
    let refs: Vec<&str> = summaries
        .iter()
        .map(|summary| {
            summary
                .row
                .provenance
                .source_ref
                .as_deref()
                .expect("sourceRef 在场")
        })
        .collect();
    for expected in [
        "chg-flow/implement/executor/1",
        "chg-flow/implement/evaluator/2",
        "chg-flow/implement/decision/3",
    ] {
        assert!(refs.contains(&expected), "sourceRef 逐字在册: {expected}");
    }
    for summary in &summaries {
        let segments: Vec<&str> = summary
            .row
            .provenance
            .source_ref
            .as_deref()
            .expect("sourceRef 在场")
            .split('/')
            .collect();
        assert_eq!(segments[0], "chg-flow", "第一段 = change 名");
        assert_eq!(segments[1], "implement", "第二段 = phase");
        assert!(
            matches!(segments[2], "executor" | "evaluator" | "decision"),
            "第三段 = 角色线格式词，实际: {}",
            segments[2]
        );
    }
    // 反查精确过滤：单角色 × attempt 命中唯一会话（转录联动反查契约）
    let evaluator_only = store
        .list_sessions(Some(SOURCE_CHANGE), Some("chg-flow/implement/evaluator/2"))
        .expect("精确反查应成功");
    assert_eq!(evaluator_only.len(), 1);
    assert_eq!(
        evaluator_only[0].row.provenance.source_ref.as_deref(),
        Some("chg-flow/implement/evaluator/2")
    );
}

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`AskPort` 句柄经组合根抵达 `Flow` 档执行体（唯一装配点）
// ---------------------------------------------------------------------------

/// 双面假 sink：`RunEventSink` + `AskPort` 同实例（两路观测到同一份内部计数
/// ——同一 `Arc` 两次 trait 强转的机械判据）。
#[derive(Default)]
struct DualFaceSink {
    updates: Arc<Mutex<Vec<RunUpdate>>>,
    ask_calls: Arc<Mutex<Vec<&'static str>>>,
    ask_payloads: Arc<Mutex<Vec<AskPayload>>>,
}

/// 两路记录句柄（复杂度告警的类型别名）。
type AskCallLog = Arc<Mutex<Vec<&'static str>>>;
type AskPayloadLog = Arc<Mutex<Vec<AskPayload>>>;

impl DualFaceSink {
    fn new() -> Self {
        Self::default()
    }

    fn ask_handle(&self) -> (AskCallLog, AskPayloadLog) {
        (Arc::clone(&self.ask_calls), Arc::clone(&self.ask_payloads))
    }

    fn updates_handle(&self) -> Arc<Mutex<Vec<RunUpdate>>> {
        Arc::clone(&self.updates)
    }
}

impl RunEventSink for DualFaceSink {
    fn emit(&self, update: RunUpdate) {
        self.updates
            .lock()
            .expect("事件捕获锁不可中毒")
            .push(update);
    }
}

impl AskPort for DualFaceSink {
    fn ask_started(&self, ask: AskPayload) {
        self.ask_calls
            .lock()
            .expect("ask 记录锁不可中毒")
            .push("started");
        self.ask_payloads
            .lock()
            .expect("ask 载荷锁不可中毒")
            .push(ask);
    }

    fn ask_resolved(&self) {
        self.ask_calls
            .lock()
            .expect("ask 记录锁不可中毒")
            .push("resolved");
    }
}

/// worker 装配夹具：按来源 + ask port 形态构造 `KernelWorkerPort`（注册表注入
/// 同一张表以便应答回流）。
fn worker_with_ask_port(
    env: &DualEnv,
    sink: Arc<dyn RunEventSink>,
    registry: Arc<StopRegistry>,
    ask_registry: Arc<agent::AskRegistry>,
    ask_port: Option<Arc<dyn AskPort>>,
) -> KernelWorkerPort {
    let composed = compose_turn(
        env.stores(),
        registry,
        Arc::new(SessionStreamRegistry::new()),
        ask_registry,
        env.store(),
        None,
        Path::new(&env.root()),
        ask_port,
    )
    .expect("解析应成功");
    KernelWorkerPort::new(composed, sink)
}

/// change 来源拿到句柄（正向，AC-2）：假 sink 同时实现 `RunEventSink` +
/// `AskPort`；`source="change"` + `ask = Some(Flow)` 的轮 → 脚本化流缝让模型
/// **真的调用** `AskUserQuestion` → 假 sink 的 `ask_started` / `ask_resolved`
/// 各记录一条、载荷 `tool_call_id` 非空、应答经 `AskRegistry` 回灌后续跑。
///
/// 观测边界：模型边界由本机回环脚本化 SSE 桩承担（`ask_scripted_provider`）；
/// 轮内的铸造 / 广播由真实 `SdkRunner` 泵 + `loop` 执行体产生，不经直构
/// `AskContext` 绕过。
#[tokio::test]
async fn change来源的flow档ask经组合根广播两拍并可应答() {
    let env = DualEnv::new("ask-port-change");
    let stores = env.stores();
    let stub = crate::ask_scripted_provider::AskScriptedProvider::new(
        "worker-flow",
        "tu_w_ask",
        "选哪个？",
        &["甲", "乙"],
    );
    seed_sdk_at(stores, "ask 广播实例", &stub.base_url);
    let ask_registry = Arc::new(agent::AskRegistry::default());
    let sink = Arc::new(DualFaceSink::new());
    let (ask_calls, ask_payloads) = sink.ask_handle();
    let updates = sink.updates_handle();

    // 两次 trait 强转同一 `Arc`（`RunEventSink` + `AskPort`）
    let event_face: Arc<dyn RunEventSink> = sink.clone();
    let ask_face: Arc<dyn AskPort> = sink.clone();
    let port = worker_with_ask_port(
        &env,
        event_face,
        Arc::new(StopRegistry::new()),
        Arc::clone(&ask_registry),
        Some(ask_face),
    );
    let root = env.root();
    let request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);

    let task = tokio::spawn(async move { port.run(request).await });
    // 应答：等待执行体登记（键 = core 铸会话 id），投递后同 turn 续跑
    let session_id = {
        let mut found = None;
        for _ in 0..3000 {
            if let Some(RunUpdate::SessionEvent { session_id, .. }) = updates
                .lock()
                .expect("事件捕获锁不可中毒")
                .iter()
                .find(|update| matches!(update, RunUpdate::SessionEvent { .. }))
            {
                found = Some(session_id.clone());
                break;
            }
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        found.expect("装配后首事件透传（会话 id 为注册表键）")
    };
    let mut answered = false;
    for _ in 0..3000 {
        match ask_registry.answer(&session_id, "甲".to_owned()) {
            Ok(()) => {
                answered = true;
                break;
            }
            Err(error) => {
                assert_eq!(error, "当前无等待中的 ask", "仅登记未完成时重试: {error}");
                tokio::task::yield_now().await;
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        }
    }
    assert!(answered, "流程档 ask 经注册表受理");

    let outcome = task.await.expect("run 任务正常结束").expect("会话应发起");
    assert_eq!(
        outcome.status,
        AgentRunStatus::Completed,
        "应答回灌后同 turn 续跑收口，实际: {:?}",
        outcome.status
    );

    // `ask_started` / `ask_resolved` 各一次（两臂清理单点）
    assert_eq!(
        ask_calls.lock().expect("ask 记录锁不可中毒").as_slice(),
        &["started", "resolved"],
        "流程档两方法各一次"
    );
    let payloads = ask_payloads.lock().expect("ask 载荷锁不可中毒").clone();
    assert_eq!(payloads.len(), 1, "started 载荷一条");
    assert_eq!(payloads[0].question, "选哪个？", "题干逐字");
    assert_eq!(payloads[0].options, vec!["甲", "乙"], "选项逐项");
    assert_eq!(
        payloads[0].tool_call_id.as_deref(),
        Some("tu_w_ask"),
        "tool_call_id 取本次 tool_use id"
    );
    assert_eq!(
        payloads[0].session_id.as_deref(),
        Some(session_id.as_str()),
        "session_id 取 core 铸会话 id（注册表键）"
    );
    // 应答原文回灌为该 tool_use 的配对结果
    let paired = transcript_tool_result(&outcome.transcript, "tu_w_ask");
    assert_eq!(
        paired,
        Some(("甲".to_owned(), false)),
        "应答原文回灌为配对 tool_result"
    );
}

/// 同一 `Arc` 两次 trait 强转（边界）：`Arc<DualFaceSink>` 分别作为
/// `Arc<dyn RunEventSink>` 与 `Arc<dyn AskPort>` 传入 → 两侧观测到同一实例状态
///（以 sink 内部计数两路可见即证）。
#[test]
fn 同一arc两次trait强转两侧观测同一实例() {
    let sink = Arc::new(DualFaceSink::new());
    let event_face: Arc<dyn RunEventSink> = sink.clone();
    let ask_face: Arc<dyn AskPort> = sink.clone();

    ask_face.ask_started(AskPayload {
        question: "q".to_owned(),
        options: vec!["a".to_owned()],
        tool_call_id: Some("call_1".to_owned()),
        session_id: Some("core-1".to_owned()),
    });
    ask_face.ask_resolved();
    event_face.emit(RunUpdate::Ask {
        ask: AskPayload {
            question: "q".to_owned(),
            options: vec!["a".to_owned()],
            tool_call_id: Some("call_1".to_owned()),
            session_id: Some("core-1".to_owned()),
        },
    });

    let (ask_calls, ask_payloads) = sink.ask_handle();
    assert_eq!(
        ask_calls.lock().expect("ask 记录锁不可中毒").as_slice(),
        &["started", "resolved"],
        "AskPort 侧记录"
    );
    assert_eq!(
        ask_payloads.lock().expect("ask 载荷锁不可中毒").len(),
        1,
        "同一实例的载荷记录（非两份 sink）"
    );
    assert_eq!(
        sink.updates_handle()
            .lock()
            .expect("事件捕获锁不可中毒")
            .len(),
        1,
        "RunEventSink 侧记录同一实例状态"
    );
}

/// 非 change 来源（`Generic` 档）→ `AskPort` 方法零调用（正向：port 只服务
/// `Flow` 档；explore 来源装配 `ask = Some(Generic)`）。
///
/// 驱动面同上：模型真的调用 `AskUserQuestion` → 执行体只登记 + 阻塞（不吃
/// port），应答回灌后同 turn 续跑。
#[tokio::test]
async fn 非change来源的泛用档不吃ask_port() {
    let env = DualEnv::new("ask-port-explore");
    let stub = crate::ask_scripted_provider::AskScriptedProvider::new(
        "worker-generic",
        "tu_w_generic",
        "怎么答？",
        &["甲", "乙"],
    );
    seed_sdk_at(env.stores(), "ask 泛用实例", &stub.base_url);
    let ask_registry = Arc::new(agent::AskRegistry::default());
    let sink = Arc::new(DualFaceSink::new());
    let (ask_calls, ask_payloads) = sink.ask_handle();
    let updates = sink.updates_handle();
    let event_face: Arc<dyn RunEventSink> = sink.clone();
    let ask_face: Arc<dyn AskPort> = sink;

    let port = worker_with_ask_port(
        &env,
        event_face,
        Arc::new(StopRegistry::new()),
        Arc::clone(&ask_registry),
        Some(ask_face),
    );
    let root = env.root();
    let mut request = turn_request(&root, "unused", WorkerRole::Executor);
    request.provenance = SessionProvenance {
        source: "explore".to_owned(),
        source_ref: None,
    };

    let task = tokio::spawn(async move { port.run(request).await });
    let session_id = wait_first_session_id(&updates).await;
    answer_until_accepted(&ask_registry, &session_id, "乙").await;
    let outcome = task.await.expect("run 任务正常结束").expect("会话应发起");
    assert_eq!(
        outcome.status,
        AgentRunStatus::Completed,
        "泛用档应答回灌后续跑收口，实际: {:?}",
        outcome.status
    );

    assert!(
        ask_calls.lock().expect("ask 记录锁不可中毒").is_empty(),
        "Generic 档不吃 port（触了 ask 工具仍零广播）"
    );
    assert!(
        ask_payloads.lock().expect("ask 载荷锁不可中毒").is_empty(),
        "零载荷记录"
    );
    assert_eq!(
        transcript_tool_result(&outcome.transcript, "tu_w_generic"),
        Some(("乙".to_owned(), false)),
        "应答仍可回灌续跑（只阻塞，零编排广播）"
    );
}

/// `Flow` 档但组合根传 `None` port（异常，DD2 防御臂的生产形态）：模型调
/// `AskUserQuestion` → 该调用以 `is_error`「ask 编排状态面未装配」回灌、注册表
/// 零登记、run 不中断（正常收口）。
#[tokio::test]
async fn 流程档无port时调用以is_error回灌且轮正常收口() {
    let env = DualEnv::new("ask-port-flow-none");
    let stub = crate::ask_scripted_provider::AskScriptedProvider::new(
        "worker-flow-none",
        "tu_w_no_port",
        "无编排面",
        &["x"],
    );
    seed_sdk_at(env.stores(), "ask 无 port 实例", &stub.base_url);
    let ask_registry = Arc::new(agent::AskRegistry::default());
    let sink = Arc::new(DualFaceSink::new());
    let (ask_calls, _ask_payloads) = sink.ask_handle();

    let port = worker_with_ask_port(
        &env,
        sink as Arc<dyn RunEventSink>,
        Arc::new(StopRegistry::new()),
        Arc::clone(&ask_registry),
        None,
    );
    let root = env.root();
    let request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);
    let outcome = port.run(request).await.expect("会话应发起");

    assert_eq!(
        outcome.status,
        AgentRunStatus::Completed,
        "防御臂不中断 run，实际: {:?}",
        outcome.status
    );
    assert_eq!(
        transcript_tool_result(&outcome.transcript, "tu_w_no_port"),
        Some(("ask 编排状态面未装配".to_owned(), true)),
        "DD2 防御文案 + is_error"
    );
    assert!(
        ask_calls.lock().expect("ask 记录锁不可中毒").is_empty(),
        "无 port 形态零广播"
    );
    assert!(
        ask_registry
            .answer(&outcome.session_id, "x".to_owned())
            .is_err(),
        "零登记（防御臂先于登记）"
    );
}
