//! worker 全链用例（sdk 引擎承载）：假 sink + `DualEnv` 真库 +
//! `KernelWorkerPort`。CLI 租户退役——PATH 隔离 / 假 CLI shim /
//! `TEST_PATH_LOCK` 串行装置与 CliMissing 收敛用例随 cli 档用例群废弃
//! （无消费者即删）；sdk 轮全程无 PATH 轮前预检（`discover` 已随 `cli/`
//! 目录删除，预检分支删除回归经本文件全链用例保绿锚定）。

use std::sync::{Arc, Mutex};

use agent::{
    AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunStatus, ModelLevel,
    SessionProvenance, SessionStreamRegistry, StopRegistry,
};
use orchestration::port::{RunEventSink, WorkerAgentPort, WorkerRole, WorkerTurnRequest};
use orchestration::state::RunUpdate;
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
    let provider = stores
        .global()
        .upsert_agent_provider(AgentProviderRecord::new(
            format!("{name}供应"),
            AgentProviderProtocol::Openai,
            "http://127.0.0.1:9/v1".to_owned(),
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
            0,
            AgentEventKind::Message {
                role: AgentMessageRole::User,
                blocks: vec![AgentBlock::Text {
                    text: "上一轮问".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
        agent::AgentEvent::stamp(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![AgentBlock::Text {
                    text: "上一轮答".to_owned(),
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

// ---------------------------------------------------------------------------
// 装配半边：provenance / permission / 实例外键（AC-7）
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn change来源与permission装配入会话行_engine恒sdk携实例id() {
    let env = DualEnv::new("provenance");
    let stores = env.stores();
    let default_id = seed_sdk_default(&stores, "装配实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, mut captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        &stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        env.store(),
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
    seed_sdk_default(&stores, "续话实例");
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
        &stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        env.store(),
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
        &stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        env.store(),
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
            .list_sessions(None, None)
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
    seed_sdk_default_with_blank_provider(&stores);
    let (sink, _captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        &stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        env.store(),
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
            .list_sessions(None, None)
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
    seed_sdk_default(&stores, "泵收集实例");
    let (sink, captured) = CapturingSink::capturing();
    let registry = Arc::new(StopRegistry::new());
    let composed: ComposedTurn = compose_turn(
        &stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        env.store(),
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
// 停止收敛（AC-7 停止半边）
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn 中途停止收敛stopped并保留已收集事件() {
    let env = DualEnv::new("stop");
    let stores = env.stores();
    seed_sdk_default(&stores, "停止实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, mut captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        &stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        env.store(),
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
    seed_sdk_default(&stores, "角色实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, mut captured) = CapturingSink::capturing();
    let composed: ComposedTurn = compose_turn(
        &stores,
        Arc::clone(&registry),
        Arc::new(SessionStreamRegistry::new()),
        env.store(),
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
