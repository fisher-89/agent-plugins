use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::{
    AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunStatus, SessionStreamRegistry,
    StopRegistry,
};
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord, AgentRunRecord,
    SessionConfigSnapshot, SessionRecord, WorkspaceStores,
};

use super::{
    agent_session_transcript, agent_session_watch_with, agent_sessions, agent_start,
    agent_start_with, agent_stop, list_change_chat_sessions, session_detail,
};

// ---------------------------------------------------------------------------
// 装置：tempdir 真库 + mock app 托管态 + fixture 落库
// ---------------------------------------------------------------------------

/// 数据根 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    data_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("exec-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("exec-cmd-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_dir, ws_root }
    }

    /// 在 workspace 根下创建真实目录并返回路径（for_root canonical 基准）。
    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }

    /// 目录路径转命令面 String root。
    fn root_of(&self, name: &str) -> String {
        self.ws(name).to_string_lossy().into_owned()
    }
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 WorkspaceStores（打开 env
/// 数据根的全局库）与 `Arc<StopRegistry>`（内核治理面挂载，须先于 start 托管）。
fn app_with_stores(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app.manage(Arc::new(StopRegistry::new()));
    // 会话实时流注册表：与 `src/main.rs` 同型空表托管（watch 命令取用面）
    app.manage(Arc::new(SessionStreamRegistry::new()));
    app
}

/// 丢弃型 Channel（发送信封即弃；查询面用例不观测实时流）。
fn discarding_channel() -> Channel<super::agent::AgentRunMessage> {
    Channel::new(|_: InvokeResponseBody| Ok(()))
}

/// 捕获型 Channel：逐信封收下序列化 JSON（终态部件观测半边）。
fn capturing_channel() -> (
    Channel<super::agent::AgentRunMessage>,
    Arc<Mutex<Vec<serde_json::Value>>>,
) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&captured);
    let channel = Channel::new(move |body: InvokeResponseBody| {
        if let InvokeResponseBody::Json(text) = body {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                sink.lock().expect("捕获锁不可中毒").push(value);
            }
        }
        Ok(())
    });
    (channel, captured)
}

fn seed_provider(
    stores: &WorkspaceStores,
    name: &str,
    api_key: &str,
    base_url: &str,
    high: &str,
) -> i64 {
    stores
        .global()
        .upsert_agent_provider(AgentProviderRecord::new(
            name.to_owned(),
            base_url.to_owned(),
            api_key.to_owned(),
            AgentModelTiers {
                high: high.to_owned(),
                medium: "m-medium".to_owned(),
                low: "m-low".to_owned(),
            },
            None,
        ))
        .expect("落 provider fixture")
        .id
}

/// sdk 默认实例 fixture（返回实例 id）。
fn seed_sdk_default(stores: &WorkspaceStores, name: &str, provider_id: i64) -> i64 {
    let id = stores
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            name.to_owned(),
            AgentEngineKind::Sdk,
            Some(provider_id),
        ))
        .expect("落 sdk 实例 fixture")
        .id;
    stores
        .global()
        .set_default_agent_instance(id)
        .expect("置默认实例 fixture");
    id
}

/// sdk 快照会话行 fixture（Continue 用例底座）。
fn seed_sdk_session(
    store: &store::Store,
    id: &str,
    remote: Option<&str>,
    source: &str,
    source_ref: Option<&str>,
) {
    store
        .create_session(&SessionRecord {
            id: id.to_owned(),
            engine_session_id: remote.map(str::to_owned),
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                agent_instance_id: None,
                model: Some("m-high".to_owned()),
                permission_mode: AgentPermissionMode::BypassPermissions,
            },
            source: source.to_owned(),
            source_ref: source_ref.map(str::to_owned),
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("落会话行 fixture");
}

/// 盖戳密封事件 fixture。
fn stamped(seq: u64, kind: AgentEventKind) -> ::agent::AgentEvent {
    ::agent::AgentEvent::stamp(seq, kind)
}

// ---------------------------------------------------------------------------
// agent_sessions / agent_session_transcript：root 寻址直查 DTO（AC-9 查询面）
// ---------------------------------------------------------------------------

#[test]
fn agent_sessions携root直查dto且source过滤透传() {
    let env = Env::new("sessions-query");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");

    // fixture：debug 会话（updated_at 大）+ explore 会话（过滤目标）
    seed_sdk_session(&store, "ses-cmd-debug", Some("sdk-1"), "debug", None);
    store
        .bind_session_remote("ses-cmd-debug", Some("sdk-1"), 1727000009000)
        .expect("bind 应成功");
    seed_sdk_session(&store, "ses-cmd-explore", None, "explore", Some("42"));

    let all = agent_sessions(state.clone(), root.clone(), None, None, 0, 20)
        .expect("清单应成功")
        .sessions;
    let ids: Vec<String> = all.iter().map(|summary| summary.row.id.clone()).collect();
    assert_eq!(
        ids,
        vec!["ses-cmd-debug", "ses-cmd-explore"],
        "updated_at 降序清单（SessionSummary 数组直查）"
    );
    assert_eq!(
        all[0].row.remote_session_id.as_deref(),
        Some("sdk-1"),
        "DTO 逐字段（SessionRow 形态）"
    );

    // source / source_ref 过滤透传
    let explores = agent_sessions(
        state.clone(),
        root.clone(),
        Some("explore".to_owned()),
        None,
        0,
        20,
    )
    .expect("过滤清单应成功")
    .sessions;
    assert_eq!(
        explores
            .iter()
            .map(|s| s.row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["ses-cmd-explore"],
        "source 过滤透传"
    );
    let ref42 = agent_sessions(
        state.clone(),
        root.clone(),
        Some("explore".to_owned()),
        Some("42".to_owned()),
        0,
        20,
    )
    .expect("过滤清单应成功")
    .sessions;
    assert_eq!(ref42.len(), 1);
    let none_match = agent_sessions(
        state.clone(),
        root.clone(),
        Some("explore".to_owned()),
        Some("99".to_owned()),
        0,
        20,
    )
    .expect("过滤清单应成功")
    .sessions;
    assert!(none_match.is_empty(), "source_ref 不匹配为空");
}

#[test]
fn agent_sessions分页offset_limit透传与total为过滤集全量() {
    let env = Env::new("sessions-paged");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    // 三个 debug 会话 + 一个 explore 会话（total 过滤口径对照）
    for id in ["ses-pg-1", "ses-pg-2", "ses-pg-3"] {
        seed_sdk_session(&store, id, None, "debug", None);
    }
    seed_sdk_session(&store, "ses-pg-x", None, "explore", Some("42"));

    // 第 2 页（offset=2, limit=2）：offset/limit 过命令面不加工，直落 store 切片
    let page =
        agent_sessions(state.clone(), root.clone(), None, None, 2, 2).expect("分页清单应成功");
    assert_eq!(page.total, 4, "total = 过滤集全量（非本页长度）");
    assert_eq!(
        page.sessions.len(),
        2,
        "offset/limit 透传（页宽 2 圈定本页）"
    );

    // 越界 offset：空页 + 满 total（与 db 检视同口径）
    let oob = agent_sessions(state.clone(), root.clone(), None, None, 99, 2)
        .expect("越界 offset 合法空页");
    assert!(oob.sessions.is_empty());
    assert_eq!(oob.total, 4, "越界 offset 空页恒携过滤集全量");

    // 过滤 + 分页：total 是过滤集计数（1），不是库内全量
    let filtered = agent_sessions(
        state.clone(),
        root.clone(),
        Some("explore".to_owned()),
        None,
        0,
        20,
    )
    .expect("过滤分页清单应成功");
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.sessions[0].row.id, "ses-pg-x");
}

#[test]
fn agent_session_transcript落库转录后按seq序全史重放() {
    let env = Env::new("transcript-query");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");

    seed_sdk_session(&store, "ses-cmd-t", None, "debug", None);
    store
        .append_session_events(
            "ses-cmd-t",
            &[
                stamped(
                    0,
                    AgentEventKind::RunStarted {
                        model: Some("claude-opus".to_owned()),
                        session_id: Some("s-1".to_owned()),
                        tools: Vec::new(),
                        mcp_servers: Vec::new(),
                    },
                ),
                stamped(
                    2,
                    AgentEventKind::Message {
                        role: AgentMessageRole::Assistant,
                        blocks: Vec::new(),
                        parent_tool_use_id: None,
                    },
                ),
                stamped(
                    5,
                    AgentEventKind::TurnDone {
                        subtype: "success".to_owned(),
                        is_error: false,
                        num_turns: Some(1),
                        duration_ms: Some(10),
                        cost_usd: None,
                        usage: serde_json::Value::Null,
                        session_id: Some("s-1".to_owned()),
                    },
                ),
            ],
        )
        .expect("落转录");

    let transcript =
        agent_session_transcript(state, root, "ses-cmd-t".to_owned()).expect("转录应成功");
    let seqs: Vec<u64> = transcript.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 2, 5], "全史密封事件按 seq 序返回（AC-9）");
    // 密封-only：转录不含 delta（store 防御半边的命令面投影）
    assert!(transcript.iter().all(|event| event.kind.is_sealed()));
}

#[test]
fn 不存在会话转录空数组且blank_root守卫空结果() {
    let env = Env::new("transcript-guard");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // 不存在会话：空数组
    let root = env.root_of("alpha");
    let transcript = agent_session_transcript(state.clone(), root, "ses-404".to_owned())
        .expect("不存在会话返回空数组");
    assert!(transcript.is_empty());

    // blank root：守卫拦截返回空结果（root 寻址与 Result 模板保留）
    let blank = agent_session_transcript(state.clone(), String::new(), "ses-x".to_owned())
        .expect("blank root 返回空结果不报错");
    assert!(blank.is_empty());
    let blank_list =
        agent_sessions(state, String::new(), None, None, 0, 20).expect("blank root 清单返回空结果");
    assert!(blank_list.sessions.is_empty());
    assert_eq!(blank_list.total, 0, "blank root 空信封 total=0");
}

// ---------------------------------------------------------------------------
// session_detail：按 id 单查（row + stats + turns 三件套，AC-6 命令面投影）
// ---------------------------------------------------------------------------

/// seed 已收轮 + 密封转录的完整会话（单查三件套断言底座），返回轮 id。
fn seed_session_with_closed_turn(store: &store::Store, id: &str) -> i64 {
    seed_sdk_session(store, id, None, "debug", None);
    let turn = store
        .begin_agent_turn(id, 1727000000000)
        .expect("begin 轮行 fixture")
        .id;
    let record = AgentRunRecord {
        id: turn,
        session_id: Some(id.to_owned()),
        status: AgentRunStatus::Completed,
        started_at: 1727000000000,
        finished_at: Some(1727000005000),
        num_turns: Some(3),
        cost_usd: Some(0.2),
        duration_ms: Some(4000),
        error: None,
    };
    store
        .finish_agent_turn(turn, &record)
        .expect("finish 轮行 fixture");
    store
        .append_session_events(
            id,
            &[
                stamped(
                    0,
                    AgentEventKind::Message {
                        role: AgentMessageRole::Assistant,
                        blocks: Vec::new(),
                        parent_tool_use_id: None,
                    },
                ),
                stamped(
                    2,
                    AgentEventKind::TurnDone {
                        subtype: "success".to_owned(),
                        is_error: false,
                        num_turns: Some(3),
                        duration_ms: Some(4000),
                        cost_usd: None,
                        usage: serde_json::json!({ "input_tokens": 7, "output_tokens": 9 }),
                        session_id: Some("s-1".to_owned()),
                    },
                ),
            ],
        )
        .expect("落转录 fixture");
    turn
}

#[test]
fn session_detail直查返回三件套与清单面对应条目全等() {
    let env = Env::new("detail-hit");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    seed_session_with_closed_turn(&store, "ses-detail-1");

    let found = session_detail(state.clone(), root.clone(), "ses-detail-1".to_owned())
        .expect("单查应成功")
        .expect("命中为 Some 包裹（查无此 id 才 Err）");

    // 与清单面对应条目逐字段全等（聚合形状单一来源）
    let listed = agent_sessions(state.clone(), root.clone(), None, None, 0, 20)
        .expect("清单应成功")
        .sessions;
    assert_eq!(listed.len(), 1);
    assert_eq!(found, listed[0], "命令面单查与清单面同形状");

    // 三件套逐面锚定：row + stats（轮数 / 墙钟 / token 现算）+ turns 全量
    assert_eq!(found.row.id, "ses-detail-1");
    assert_eq!(
        found.row.provenance.source, "debug",
        "row 面（id / provenance / 双时间戳）随行"
    );
    assert_eq!(found.stats.turn_count, 1, "轮数 = 轮统计行行数");
    assert_eq!(found.stats.total_duration_ms, Some(4000));
    assert_eq!(
        found.stats.input_tokens,
        Some(7),
        "token 自 TurnDone usage 求和"
    );
    assert_eq!(found.turns.len(), 1, "轮统计行全量随行");
    assert_eq!(found.turns[0].status, AgentRunStatus::Completed);
    assert_eq!(found.turns[0].duration_ms, Some(4000));
}

#[test]
fn session_detail运行中会话轮行呈running且应答不加状态字段() {
    let env = Env::new("detail-running");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-running", None, "debug", None);
    store
        .begin_agent_turn("ses-running", 1727000000000)
        .expect("begin 轮行 fixture（未收轮）");

    let found = session_detail(state, root, "ses-running".to_owned())
        .expect("单查应成功")
        .expect("Some 包裹");

    assert_eq!(found.turns.len(), 1, "轮行清单为唯一状态事实源");
    assert_eq!(
        found.turns[0].status,
        AgentRunStatus::Running,
        "有 running 轮行即 running（消费方自轮行推导）"
    );
    // 命令面不加状态字段：出线键恒为 row / stats / turns 三件
    let value = serde_json::to_value(&found).expect("出线序列化应成功");
    assert!(
        value.get("status").is_none(),
        "命令面不加顶层状态字段，实际: {value}"
    );
    assert!(
        value.get("row").is_some() && value.get("stats").is_some() && value.get("turns").is_some()
    );
}

#[test]
fn session_detail查无此id_err透传不吞成none() {
    let env = Env::new("detail-miss");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let error = session_detail(state, root, "ses-404".to_owned())
        .expect_err("查无此 id 必须 Err（悬挂 id 不伪装空态）");
    assert!(
        error.contains("会话不存在") && error.contains("ses-404"),
        "core Err 原样透传，实际: {error}"
    );
}

#[test]
fn session_detail_blank_root守卫none先行于库寻址() {
    let env = Env::new("detail-blank-root");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    // 空串
    let none = session_detail(state.clone(), String::new(), "ses-x".to_owned())
        .expect("blank root 返回空结果不报错");
    assert!(
        none.is_none(),
        "空串 → Ok(None)（与 agent_sessions 同口径）"
    );
    // 空白串
    let none = session_detail(state, "   ".to_owned(), "ses-x".to_owned())
        .expect("空白 root 返回空结果不报错");
    assert!(none.is_none(), "空白串 → Ok(None)");
}

#[test]
fn session_detail跨root隔离_会话落a库经b库单查err() {
    let env = Env::new("detail-iso");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root_a = env.root_of("alpha");
    let root_b = env.root_of("beta");
    let store_a = state.for_root(&root_a).expect("for_root 应成功");
    seed_session_with_closed_turn(&store_a, "ses-in-a");

    let found = session_detail(state.clone(), root_a, "ses-in-a".to_owned())
        .expect("A 库单查应成功")
        .expect("A 库命中");
    assert_eq!(found.row.id, "ses-in-a");

    let error = session_detail(state, root_b, "ses-in-a".to_owned())
        .expect_err("B 库单查必须 Err（for_root 按 root 寻址所属库）");
    assert!(
        error.contains("会话不存在"),
        "跨库 miss 显式 Err，实际: {error}"
    );
}

// ---------------------------------------------------------------------------
// agent_start：守卫 / 错误映射 / 提前 resolve / 参数转换（三件事纪律）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agent_start_blank_root守卫err且零落库() {
    let env = Env::new("start-blank-root");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let provider = seed_provider(
        stores.inner(),
        "守卫供应",
        "sk-test",
        "https://api.example.com/v1",
        "m-high",
    );
    seed_sdk_default(stores.inner(), "守卫实例", provider);

    let result = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        "   ".to_owned(),
        "提示词".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    let error = result.expect_err("blank root 必须 Err");
    assert!(
        error.contains("root"),
        "守卫 Err 文案（无 cwd 无从发起），实际: {error}"
    );
    assert!(
        stores
            .inner()
            .for_root(&env.root_of("alpha"))
            .expect("for_root 应成功")
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "守卫拦截零落库"
    );
}

#[tokio::test]
async fn agent_start无默认agent时err引导管理页() {
    let env = Env::new("start-no-default");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let result = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "提示词".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    let error = result.expect_err("空库缺省必须 Err");
    assert!(
        error.contains("默认 agent") && error.contains("/agents"),
        "Err 文案含管理页引导语义（AC-9），实际: {error}"
    );
    // 零落库：解析失败不产任何记录
    let store = stores.inner().for_root(&root).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "解析失败零落库"
    );
}

#[tokio::test]
async fn agent_start_sdk空配置时config_missing映射err直抵前端() {
    let env = Env::new("start-config-missing");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    // 空配置 provider（解析通过、open 段校验失败：错误映射半边的 SDK 臂）
    let provider = seed_provider(stores.inner(), "空配置供应", "", "", "");
    seed_sdk_default(stores.inner(), "空配置实例", provider);

    let result = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "提示词".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    let error = result.expect_err("空配置必须 Err");
    assert!(
        error.contains("配置缺失") && error.contains("api_key"),
        "AgentStartError → Err(String) 映射直抵前端（三件事纪律的错误映射半边），实际: {error}"
    );
    // 启动失败不产生任何记录（open 失败不落库经真实组合验证）
    let store = stores.inner().for_root(&root).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "启动失败零落库"
    );
}

#[tokio::test]
async fn agent_start成功链路提前resolve返回running轮行且会话行轮行已落库() {
    let env = Env::new("start-early-resolve");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    // 非法 base_url（URL 含空格）：open 段配置校验通过、请求编码段同步 Err →
    // 泵以 api_error 即时收敛，不落网络不进重试退避
    let provider = seed_provider(
        stores.inner(),
        "提前resolve供应",
        "sk-test",
        "http://bad host/v1",
        "m-high",
    );
    seed_sdk_default(stores.inner(), "提前resolve实例", provider);

    let (channel, captured) = capturing_channel();
    let result = agent_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        "提前 resolve 一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    // 同步段已完成（begin_turn 落库先行于异步泵）：会话行 / 轮行已落库
    let store = stores.inner().for_root(&root).expect("for_root 应成功");
    let summaries = store.list_sessions(None, None).expect("清单应成功");

    let summary = result.expect("成功链路提前 resolve");
    assert_eq!(
        summary.status,
        ::agent::AgentRunStatus::Running,
        "返回 running 态轮行"
    );
    assert_eq!(
        summary.finished_at, None,
        "finishedAt null（提前 resolve 契约）"
    );
    assert!(summary.session_id.starts_with("ses-"), "core 铸会话 id");

    assert_eq!(summaries.len(), 1, "会话行已落库");
    assert_eq!(summaries[0].row.id, summary.session_id);
    assert_eq!(summaries[0].turns.len(), 1, "轮行已落库");
    assert_eq!(summaries[0].turns[0].turn_id, summary.turn_id);
    assert_eq!(
        summaries[0].turns[0].status,
        ::agent::AgentRunStatus::Running
    );
    assert_eq!(
        summaries[0].row.provenance.source, "debug",
        "来源缺省 debug"
    );

    // 等待后台泵收敛（非法 base_url 编码段同步 Err → api_error 即时收敛，
    // 不落网络不触真实 CLI）
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let terminal = captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "record");
        if terminal {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "等待后台收敛超时");
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[tokio::test]
async fn agent_start参数转换_session引用与来源与agent透传() {
    let env = Env::new("start-params");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    // 显式 agent（sdk）+ Continue 底座会话（存量 cli 快照：跨引擎拒绝先行验证
    // Continue 寻址正确性——若 session_id 未透传则不会命中跨引擎拒绝）
    let provider = seed_provider(
        stores.inner(),
        "参数转换供应",
        "sk-test",
        "https://api.example.com/v1",
        "m-high",
    );
    let sdk_id = seed_sdk_default(stores.inner(), "参数转换实例", provider);
    let store = stores.inner().for_root(&root).expect("for_root 应成功");
    store
        .create_session(&SessionRecord {
            id: "ses-param-continue".to_owned(),
            engine_session_id: Some("s-cli-1".to_owned()),
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Cli,
                agent_instance_id: None,
                model: Some("m-high".to_owned()),
                permission_mode: AgentPermissionMode::BypassPermissions,
            },
            source: "explore".to_owned(),
            source_ref: Some("7".to_owned()),
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("落 Continue 底座会话行");

    // Continue + source/sourceRef 透传：跨引擎拒绝（存量 cli 快照会话以 sdk agent 续）
    let result = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "续会话一轮".to_owned(),
        AgentPermissionMode::AcceptEdits,
        Some("ses-param-continue".to_owned()),
        Some("explore".to_owned()),
        Some("7".to_owned()),
        Some(sdk_id),
        None,
    )
    .await;

    let error = result.expect_err("存量 cli 快照会话以 sdk agent 续必须被拒");
    assert!(
        error.contains("非当前引擎产出") && error.contains("ses-param-continue"),
        "session_id 透传至 Continue 校验（参数转换半边），实际: {error}"
    );

    // New 缺省形态：source 缺省 debug 已由提前 resolve 用例断言
}

#[tokio::test]
async fn agent_start_kernelscoped_stopregistry托管的命中链经channel流出终态部件() {
    let env = Env::new("start-stop-channel");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    // 非法 base_url（URL 含空格）：open 段配置校验通过、请求编码段同步 Err——
    // 停止臂与泵收敛竞速收敛口径不变（终态部件观测半边）
    let provider = seed_provider(
        stores.inner(),
        "停止链供应",
        "sk-test",
        "http://bad host/v1",
        "m-high",
    );
    seed_sdk_default(stores.inner(), "停止链实例", provider);

    let (channel, captured) = capturing_channel();

    let summary = agent_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        "停止链一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("提前 resolve");

    // 停止置位（begin_turn 同步段已登记句柄；若后台泵已收敛除名则 miss 幂等
    // 无副作用——命中/miss 两态均不报错，注册表置位语义由 kernel_test 锁定）
    let registry = app.state::<Arc<StopRegistry>>();
    let _ = registry.request_stop(&summary.session_id);

    // 轮询捕获 Channel 直至终态 Record 信封（后台转发任务；停止截停收敛）
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let terminal = loop {
        let found = captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .find(|value| value["ipc"] == "record")
            .cloned();
        if let Some(record) = found {
            break record;
        }
        assert!(std::time::Instant::now() < deadline, "等待终态 Record 超时");
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(5));
    };

    assert_eq!(
        terminal["record"]["sessionId"],
        serde_json::json!(summary.session_id),
        "终态部件与重放同构（running summary 同会话）"
    );
    assert_eq!(
        terminal["record"]["status"],
        serde_json::json!("stopped"),
        "stopregistry 命中截停 stopped 收敛经 Channel 流出（泵 select 停止臂先于编码段 Err 获胜）"
    );
}

// ---------------------------------------------------------------------------
// agent_stop：会话寻址（root 寻址保留）与幂等
// ---------------------------------------------------------------------------

#[test]
fn agent_stop命中登记句柄置位成功返回ok() {
    let env = Env::new("stop-hit");
    let app = app_with_stores(&env);
    let registry = app.state::<Arc<StopRegistry>>();
    let handle = ::agent::RunHandle::default();
    registry.register("ses-stoppable", handle.clone());

    let result = agent_stop(
        registry.clone(),
        env.root_of("alpha"),
        "ses-stoppable".to_owned(),
    );

    result.expect("命中停止返回 Ok");
    assert!(handle.stop_requested(), "停止信号已置位（AC-7 命令半边）");
}

#[test]
fn agent_stop对未运行会话与不存在session_id幂等ok() {
    let env = Env::new("stop-idempotent");
    let app = app_with_stores(&env);
    let registry = app.state::<Arc<StopRegistry>>();
    let handle = ::agent::RunHandle::default();
    registry.register("ses-live", handle.clone());

    // 未登记 session_id：Ok 不报错
    agent_stop(registry.clone(), env.root_of("alpha"), "ses-404".to_owned()).expect("miss 幂等 Ok");
    assert!(!handle.stop_requested(), "miss 不波及登记句柄");

    // 终态除名后（remove 由内核收敛路径调用）再 stop：幂等忽略
    registry.remove("ses-live");
    agent_stop(
        registry.clone(),
        env.root_of("alpha"),
        "ses-live".to_owned(),
    )
    .expect("除名后幂等 Ok");

    // blank root 天然 miss：Ok 不报错
    agent_stop(registry, String::new(), "ses-live".to_owned()).expect("blank root Ok");
}

/// 错误透传：root 不可寻址（`for_root` canonicalize 失败）→ 三查询命令显式
/// `Err` 记因（不 panic、不产半截结果——查询面错误同口径，blank root 早退
/// 空结果语义之外的显式失败面）。
#[test]
fn root不可寻址_三查询命令err透传不panic() {
    let env = Env::new("for-root-err");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let missing = env
        .data_dir
        .path()
        .join("absent-ws")
        .to_string_lossy()
        .into_owned();

    for err in [
        agent_sessions(state.clone(), missing.clone(), None, None, 0, 20).expect_err("清单应 Err"),
        agent_session_transcript(state.clone(), missing.clone(), "ses-x".to_owned())
            .expect_err("转录应 Err"),
        session_detail(state.clone(), missing.clone(), "ses-x".to_owned()).expect_err("详情应 Err"),
    ] {
        assert!(
            err.contains("canonicalize") && err.contains("absent-ws"),
            "Err 记因 canonicalize 失败与 root 线索，实际: {err}"
        );
    }
}
// ---------------------------------------------------------------------------
// agent_session_watch：会话实时流订阅命令（薄包装三段：blank root / 无运行轮
// 幂等立即结束 / `Event` 前向与末拍 `Record` 后返回；断流不投伪终态）。
// 广播订阅只收订阅后 live 事件（tokio broadcast 语义），故发布拍与命令并发
// 推进（join! 轮询序：命令先完成订阅并挂起，发布随后落拍）。
// ---------------------------------------------------------------------------

/// 发送即失败的 Channel（前向断流臂：命令仍 `Ok` 结束，不 panic 不悬挂）。
fn failing_channel() -> Channel<super::agent::AgentRunMessage> {
    Channel::new(|_: InvokeResponseBody| {
        Err(tauri::Error::AssetNotFound("注入发送失败".to_owned()))
    })
}

/// 注册表句柄（mock app 托管态取出）。
fn streams_of(app: &App<tauri::test::MockRuntime>) -> Arc<SessionStreamRegistry> {
    Arc::clone(app.state::<Arc<SessionStreamRegistry>>().inner())
}

fn sealed_watch_event(seq: u64) -> ::agent::AgentEvent {
    stamped(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: vec![::agent::AgentBlock::Text {
                text: format!("第 {seq} 段正文"),
            }],
            parent_tool_use_id: None,
        },
    )
}

fn delta_watch_event(seq: u64, text: &str) -> ::agent::AgentEvent {
    stamped(
        seq,
        AgentEventKind::MessageDelta {
            parent_tool_use_id: None,
            delta: ::agent::AgentDelta::Text {
                text: text.to_owned(),
            },
        },
    )
}

fn watch_outcome(turn_id: i64, status: AgentRunStatus, started_at: i64) -> ::agent::TurnOutcome {
    ::agent::TurnOutcome {
        turn_id,
        status,
        started_at,
        finished_at: 1_727_000_005_000,
        num_turns: Some(2),
        duration_ms: Some(5_000),
        cost_usd: Some(0.25),
        usage: serde_json::json!({ "inputTokens": 10 }),
        error: None,
        remote_session_id: Some("sdk-9-1727000000009".to_owned()),
    }
}

fn snapshot_of(captured: &Arc<Mutex<Vec<serde_json::Value>>>) -> Vec<serde_json::Value> {
    captured.lock().expect("捕获锁不可中毒").clone()
}

#[tokio::test]
async fn watch前向事件与末拍record后返回ok() {
    let env = Env::new("watch-forward");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-1".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);
    let (channel, captured) = capturing_channel();

    let watch = agent_session_watch_with(
        app.handle().clone(),
        channel,
        env.root_of("alpha"),
        session_id.clone(),
        1,
    );
    let publish = async {
        tokio::task::yield_now().await;
        for seq in 1..=3 {
            streams.publish(&session_id, sealed_watch_event(seq));
        }
        tokio::task::yield_now().await;
        streams.publish(&session_id, sealed_watch_event(4));
        streams.finish(
            &session_id,
            watch_outcome(7, AgentRunStatus::Completed, 1_727_000_000_000),
        );
    };
    let (result, ()) = tokio::join!(watch, publish);
    result.expect("watch 应 Ok 结束");

    let envelopes = snapshot_of(&captured);
    let kinds: Vec<&serde_json::Value> = envelopes.iter().map(|value| &value["ipc"]).collect();
    assert_eq!(
        kinds,
        vec![
            &serde_json::json!("event"),
            &serde_json::json!("event"),
            &serde_json::json!("event"),
            &serde_json::json!("record"),
        ],
        "since_seq=1 过滤 seq 1，其后 2 / 3 / 4 与末拍 record，实际: {envelopes:?}"
    );
    assert_eq!(
        envelopes[0]["event"]["seq"],
        serde_json::json!(2),
        "事件载荷直投（seq 2 起）"
    );
    assert_eq!(envelopes[1]["event"]["seq"], serde_json::json!(3));
    assert_eq!(envelopes[2]["event"]["seq"], serde_json::json!(4));
    // 末拍 record：TurnSummary 字段与 outcome_summary 投影一致（含 startedAt）
    let record = &envelopes[3]["record"];
    assert_eq!(record["turnId"], serde_json::json!(7));
    assert_eq!(record["sessionId"], serde_json::json!(session_id));
    assert_eq!(record["status"], serde_json::json!("completed"));
    assert_eq!(
        record["startedAt"],
        serde_json::json!(1_727_000_000_000_i64),
        "startedAt 取自终态（D4 收口形状自足）"
    );
    assert_eq!(
        record["finishedAt"],
        serde_json::json!(1_727_000_005_000_i64)
    );
    assert_eq!(record["numTurns"], serde_json::json!(2));
    assert_eq!(record["costUsd"], serde_json::json!(0.25));
    assert_eq!(record["durationMs"], serde_json::json!(5_000));
    assert_eq!(record["error"], serde_json::Value::Null);
}

#[tokio::test]
async fn watch结束即订阅释放_同会话再invoke立即ok零事件() {
    let env = Env::new("watch-release");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-release".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);
    let (channel, captured) = capturing_channel();

    let watch = agent_session_watch_with(
        app.handle().clone(),
        channel,
        env.root_of("alpha"),
        session_id.clone(),
        0,
    );
    let publish = async {
        tokio::task::yield_now().await;
        streams.finish(
            &session_id,
            watch_outcome(1, AgentRunStatus::Completed, 1_727_000_000_000),
        );
    };
    let (result, ()) = tokio::join!(watch, publish);
    result.expect("watch 应 Ok");
    assert_eq!(snapshot_of(&captured).len(), 1, "仅末拍 record");

    // 命令结束即订阅释放；条目已除名 → 再 invoke 立即 Ok 零事件
    let (again, captured_again) = capturing_channel();
    agent_session_watch_with(
        app.handle().clone(),
        again,
        env.root_of("alpha"),
        session_id,
        0,
    )
    .await
    .expect("已收口会话立即 Ok（幂等）");
    assert!(
        snapshot_of(&captured_again).is_empty(),
        "再 invoke 零事件（条目已除名，AC-4 / AC-6）"
    );
}

#[tokio::test]
async fn watch_since_seq取已发最大时零事件只投末拍record() {
    let env = Env::new("watch-filter-max");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-max".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);
    let (channel, captured) = capturing_channel();

    let watch = agent_session_watch_with(
        app.handle().clone(),
        channel,
        env.root_of("alpha"),
        session_id.clone(),
        3,
    );
    let publish = async {
        tokio::task::yield_now().await;
        for seq in 1..=3 {
            streams.publish(&session_id, sealed_watch_event(seq));
        }
        tokio::task::yield_now().await;
        streams.finish(&session_id, watch_outcome(2, AgentRunStatus::Completed, 5));
    };
    let (result, ()) = tokio::join!(watch, publish);
    result.expect("watch 应 Ok");

    let envelopes = snapshot_of(&captured);
    assert_eq!(envelopes.len(), 1, "零事件、只投末拍 record");
    assert_eq!(envelopes[0]["ipc"], serde_json::json!("record"));
}

#[tokio::test]
async fn watch_since_seq零时全量实时事件直投() {
    let env = Env::new("watch-filter-zero");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-zero".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);
    let (channel, captured) = capturing_channel();

    let watch = agent_session_watch_with(
        app.handle().clone(),
        channel,
        env.root_of("alpha"),
        session_id.clone(),
        0,
    );
    let publish = async {
        tokio::task::yield_now().await;
        for seq in 1..=3 {
            streams.publish(&session_id, sealed_watch_event(seq));
        }
        tokio::task::yield_now().await;
        streams.finish(&session_id, watch_outcome(3, AgentRunStatus::Completed, 5));
    };
    let (result, ()) = tokio::join!(watch, publish);
    result.expect("watch 应 Ok");

    let envelopes = snapshot_of(&captured);
    let seqs: Vec<serde_json::Value> = envelopes
        .iter()
        .filter(|value| value["ipc"] == serde_json::json!("event"))
        .map(|value| value["event"]["seq"].clone())
        .collect();
    assert_eq!(
        seqs,
        vec![
            serde_json::json!(1),
            serde_json::json!(2),
            serde_json::json!(3)
        ],
        "since_seq=0 全量实时事件（不重投语义单点在后端）"
    );
    assert_eq!(
        envelopes.last().map(|value| value["ipc"].clone()),
        Some(serde_json::json!("record"))
    );
}

#[tokio::test]
async fn watch_blank_root立即ok零事件零订阅副作用() {
    let env = Env::new("watch-blank-root");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-blank".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);

    for blank in ["", "   ", "\t"] {
        let (channel, captured) = capturing_channel();
        agent_session_watch_with(
            app.handle().clone(),
            channel,
            blank.to_owned(),
            session_id.clone(),
            0,
        )
        .await
        .expect("blank root 立即 Ok");
        assert!(
            snapshot_of(&captured).is_empty(),
            "blank root 零事件（{blank:?}）"
        );
    }

    // 零订阅副作用：条目仍在，事件照常可发布并被后续订阅收取
    assert!(
        streams.subscribe(&session_id, 0).is_some(),
        "blank root 不消耗条目"
    );
}

#[tokio::test]
async fn watch未知session_id立即ok零事件不err() {
    let env = Env::new("watch-unknown");
    let app = app_with_stores(&env);
    let (channel, captured) = capturing_channel();

    agent_session_watch_with(
        app.handle().clone(),
        channel,
        env.root_of("alpha"),
        "ses-404".to_owned(),
        0,
    )
    .await
    .expect("未知会话立即 Ok（与 agent_stop miss 幂等同口径）");
    assert!(snapshot_of(&captured).is_empty(), "零事件");
}

#[tokio::test]
async fn watch已收口会话立即ok零副作用() {
    let env = Env::new("watch-closed");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-closed".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);
    streams.finish(&session_id, watch_outcome(1, AgentRunStatus::Completed, 5));

    let (channel, captured) = capturing_channel();
    agent_session_watch_with(
        app.handle().clone(),
        channel,
        env.root_of("alpha"),
        session_id,
        0,
    )
    .await
    .expect("已收口会话立即 Ok（幂等）");
    assert!(snapshot_of(&captured).is_empty(), "零事件零副作用");
}

#[tokio::test]
async fn watch_channel发送失败时前向断流且命令ok结束() {
    let env = Env::new("watch-channel-fail");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-fail".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);

    // 首条事件即发送失败 → 前向断流返回（不 panic 不悬挂）
    let watch = agent_session_watch_with(
        app.handle().clone(),
        failing_channel(),
        env.root_of("alpha"),
        session_id.clone(),
        0,
    );
    let publish = async {
        tokio::task::yield_now().await;
        streams.publish(&session_id, sealed_watch_event(1));
    };
    let (result, ()) = tokio::join!(watch, publish);
    result.expect("发送失败仍 Ok 结束（不 panic）");
}

#[tokio::test]
async fn watch_lagged断流结束时零伪record() {
    let env = Env::new("watch-lagged");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-lagged".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);
    let (channel, captured) = capturing_channel();

    // 订阅后一次性涌入超容量事件（生产快于消费）→ lagged 断流收尾
    let watch = agent_session_watch_with(
        app.handle().clone(),
        channel,
        env.root_of("alpha"),
        session_id.clone(),
        0,
    );
    let publish = async {
        tokio::task::yield_now().await;
        for seq in 0..4097 {
            streams.publish(&session_id, delta_watch_event(seq, "溢出增量"));
        }
    };
    let (result, ()) = tokio::join!(watch, publish);
    result.expect("lagged 断流同样 Ok 结束");

    let envelopes = snapshot_of(&captured);
    assert!(
        envelopes
            .iter()
            .all(|value| value["ipc"] != serde_json::json!("record")),
        "断流不投伪终态（AC-6），实际: {envelopes:?}"
    );
}

#[tokio::test]
async fn watch信封线格式与since_seq极值承接() {
    let env = Env::new("watch-wire");
    let app = app_with_stores(&env);
    let session_id = "ses-watch-wire".to_owned();
    let streams = streams_of(&app);
    streams.begin(&session_id);
    let (channel, captured) = capturing_channel();

    // sinceSeq = Number.MAX_SAFE_INTEGER（JS 安全整数上界）逐字承接：事件全滤
    let watch = agent_session_watch_with(
        app.handle().clone(),
        channel,
        env.root_of("alpha"),
        session_id.clone(),
        9_007_199_254_740_991,
    );
    let publish = async {
        tokio::task::yield_now().await;
        streams.publish(&session_id, sealed_watch_event(1));
        tokio::task::yield_now().await;
        streams.finish(
            &session_id,
            watch_outcome(4, AgentRunStatus::Failed, 1_727_000_000_000),
        );
    };
    let (result, ()) = tokio::join!(watch, publish);
    result.expect("watch 应 Ok");

    let envelopes = snapshot_of(&captured);
    assert_eq!(envelopes.len(), 1, "极值 sinceSeq 下事件全滤、终态恒投");
    assert_eq!(envelopes[0]["ipc"], serde_json::json!("record"));
    // 线格式：camelCase 字段 + 判别键
    let record = &envelopes[0]["record"];
    assert!(record.get("turnId").is_some(), "record.turnId 在场");
    assert!(record.get("startedAt").is_some(), "record.startedAt 在场");
    assert!(record.get("finishedAt").is_some(), "record.finishedAt 在场");
    assert_eq!(record["status"], serde_json::json!("failed"));
}

#[tokio::test]
async fn watch期间delta直投而转录查询面零delta行() {
    let env = Env::new("watch-delta");
    let app = app_with_stores(&env);
    let root = env.root_of("alpha");
    let session_id = "ses-watch-delta".to_owned();
    // 真库种子：会话行在位（转录空）
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    seed_sdk_session(&store, &session_id, None, "debug", None);

    let streams = streams_of(&app);
    streams.begin(&session_id);
    let (channel, captured) = capturing_channel();

    let watch = agent_session_watch_with(
        app.handle().clone(),
        channel,
        root.clone(),
        session_id.clone(),
        0,
    );
    let publish = async {
        tokio::task::yield_now().await;
        // since_seq = 0 为「已观测最大 seq」上界：事件自 seq 1 起可投
        streams.publish(&session_id, delta_watch_event(1, "实时增量正文"));
        tokio::task::yield_now().await;
        streams.finish(
            &session_id,
            watch_outcome(1, AgentRunStatus::Completed, 1_727_000_000_000),
        );
    };
    let (result, ()) = tokio::join!(watch, publish);
    result.expect("watch 应 Ok");

    let envelopes = snapshot_of(&captured);
    assert_eq!(
        envelopes[0]["event"]["kind"],
        serde_json::json!("messageDelta"),
        "实时面可见 delta"
    );

    // 持久面零新增：转录查询面不含 delta 行
    let transcript = agent_session_transcript(app.state::<WorkspaceStores>(), root, session_id)
        .expect("转录查询应成功");
    assert!(
        transcript.is_empty(),
        "delta 零落库（实时面与持久面分流），实际: {transcript:?}"
    );
}

#[tokio::test]
async fn watch与agent_start终态同源_started_at逐字承接() {
    let env = Env::new("watch-vs-start");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    // 非法 base_url：请求编码段同步 Err → 泵以 api_error 即时收敛（不落网络）
    let provider = seed_provider(
        stores.inner(),
        "同源供应",
        "sk-test",
        "http://bad host/v1",
        "m-high",
    );
    seed_sdk_default(stores.inner(), "同源实例", provider);

    let (start_channel, start_captured) = capturing_channel();
    let summary = agent_start_with(
        app.handle().clone(),
        start_channel,
        root.clone(),
        "同源一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("提前 resolve");

    // 发起流末拍 record（后台驱动任务）与 running 轮行同源同起点
    let start_terminal = wait_for_record(&start_captured).await;
    assert_eq!(
        start_terminal["record"]["status"],
        serde_json::json!("failed"),
        "拒连端点失败收敛"
    );
    assert_eq!(
        start_terminal["record"]["turnId"],
        serde_json::json!(summary.turn_id),
        "发起流终态即本轮轮行"
    );
    assert_eq!(
        start_terminal["record"]["startedAt"],
        serde_json::json!(summary.started_at),
        "终态 startedAt 与 running 轮行同一起点（D4 单一来源）"
    );

    // 收口后订阅面幂等（条目已除名，AC-4 / AC-6 命令面）
    let (watch_channel, watch_captured) = capturing_channel();
    agent_session_watch_with(
        app.handle().clone(),
        watch_channel,
        root,
        summary.session_id.clone(),
        0,
    )
    .await
    .expect("已收口会话 watch 立即 Ok");
    assert!(snapshot_of(&watch_captured).is_empty(), "收口后订阅零事件");
}

/// 轮询捕获直至末拍 `record` 信封（后台驱动任务；不新增计时依赖）。
async fn wait_for_record(captured: &Arc<Mutex<Vec<serde_json::Value>>>) -> serde_json::Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let found = captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .find(|value| value["ipc"] == serde_json::json!("record"))
            .cloned();
        if let Some(record) = found {
            return record;
        }
        assert!(std::time::Instant::now() < deadline, "等待终态 Record 超时");
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

// ---------------------------------------------------------------------------
// agent_start 的 cwd 面（双 root 拆分，AC-1 / AC-2）：执行根进 SessionCtx、store
// 半边恒 workspace root；cwd 缺省 / 空白回落 root；非目录显式 Err（不进引擎、
// 不推流、不落库）。复用既有装置（Env / app_with_stores / capturing_channel /
// seed_provider / seed_sdk_default），不新建装置。
// ---------------------------------------------------------------------------

/// chat 归属的 change id（`source_ref` 身份键；URL id 形态原样使用）。
const CHAT_CHANGE_ID: &str = "0199a2f0-chat-0001-000000000001";

/// `workspaces/` 子树下 db 文件计数（`for_root` 唯一的落盘观测面——不以
/// `for_root(&cwd)` 观测，避免断言自身铸库）。
fn workspace_db_count(env: &Env) -> usize {
    let dir = env.data_dir.path().join("workspaces");
    let Ok(entries) = fs::read_dir(&dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "redb"))
        .count()
}

/// sdk 默认实例 fixture（非法 base_url：泵即时收敛，不落网络不进重试退避）。
fn seed_fast_failing_default(stores: &WorkspaceStores, tag: &str) {
    let provider = seed_provider(
        stores,
        &format!("{tag}供应"),
        "sk-test",
        "http://bad host/v1",
        "m-high",
    );
    seed_sdk_default(stores, &format!("{tag}实例"), provider);
}

/// 发起后 root 库内会话清单（断言落库半边的统一读取面）。
fn sessions_in_root(stores: &WorkspaceStores, root: &str) -> Vec<::agent::SessionSummary> {
    stores
        .for_root(root)
        .expect("for_root 应成功")
        .list_sessions(None, None)
        .expect("清单应成功")
}

#[test]
fn agent_start薄入口参数面形状_cwd末位十参同序() {
    // 形状锚：闭包体以 10 个位置实参同序转调命令薄入口（错位 / 缺位即编译失
    // 败）；闭包永不执行——行为断言归下方 `agent_start_with` 用例，生成绑定逐
    // 字断言归 `src/bindings/mod_test.rs`（双侧锁形）。
    let _shape = |app: tauri::AppHandle,
                  on_event: Channel<super::agent::AgentRunMessage>,
                  root: String,
                  prompt: String,
                  permission_mode: ::agent::AgentPermissionMode,
                  session_id: Option<String>,
                  source: Option<String>,
                  source_ref: Option<String>,
                  agent: Option<i64>,
                  cwd: Option<String>| {
        agent_start(
            app,
            on_event,
            root,
            prompt,
            permission_mode,
            session_id,
            source,
            source_ref,
            agent,
            cwd,
        )
    };
}

#[tokio::test]
async fn agent_start_with_cwd真目录作执行根而会话行落root库() {
    let env = Env::new("cwd-exec-root");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    seed_fast_failing_default(stores.inner(), "执行根");
    let worktree = env.ws("wt").to_string_lossy().into_owned();

    let (channel, captured) = capturing_channel();
    let summary = agent_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        "带执行根一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        Some(worktree.clone()),
    )
    .await
    .expect("cwd 为真目录时应成功发起");

    assert_eq!(
        summary.status,
        ::agent::AgentRunStatus::Running,
        "提前 resolve 的 running 态轮行"
    );
    assert!(summary.session_id.starts_with("ses-"), "core 铸会话 id");

    let summaries = sessions_in_root(stores.inner(), &root);
    assert_eq!(
        summaries.len(),
        1,
        "会话行落 workspace root 库（store 半边恒 root）"
    );
    assert_eq!(summaries[0].row.id, summary.session_id);
    assert_eq!(summaries[0].turns.len(), 1, "轮行随会话行落同一库");
    assert_eq!(
        summaries[0].turns[0].status,
        ::agent::AgentRunStatus::Running
    );

    // 后台泵以 api_error 收敛（不落网络）：终态经 Channel 收尾
    let record = wait_for_record(&captured).await;
    assert_eq!(
        record["record"]["sessionId"],
        serde_json::json!(summary.session_id),
        "终态部件同会话"
    );
}

#[tokio::test]
async fn agent_start_with_cwd不派生第二份workspace库() {
    let env = Env::new("cwd-double-root");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    seed_fast_failing_default(stores.inner(), "双根");
    // 基线：先经 root 铸一次库，再计数（此后 cwd 不得再派生文件）
    stores.inner().for_root(&root).expect("for_root 应成功");
    let before = workspace_db_count(&env);
    assert_eq!(before, 1, "基线恰一份 workspace 库");

    // cwd 指向 ws_root 之外的现成目录（数据根自身）：若该值进 `for_root`，
    // workspaces/ 下会多出一份 db 文件
    let outside = env.data_dir.path().to_string_lossy().into_owned();
    agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "双 root 一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        Some(outside),
    )
    .await
    .expect("执行根存在即可发起");

    assert_eq!(
        workspace_db_count(&env),
        before,
        "cwd 不进 for_root——不派生第二份 workspace 库（双 root 不变量）"
    );
    assert_eq!(
        sessions_in_root(stores.inner(), &root).len(),
        1,
        "会话行仍落 root 库"
    );
}

#[tokio::test]
async fn agent_start_with_cwd缺省回落root行为逐字不变() {
    let env = Env::new("cwd-none");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    seed_fast_failing_default(stores.inner(), "缺省根");

    let (channel, captured) = capturing_channel();
    let summary = agent_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        "无执行根一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("cwd 缺省零 Err（既有语义逐字不变）");

    assert_eq!(summary.status, ::agent::AgentRunStatus::Running);
    let summaries = sessions_in_root(stores.inner(), &root);
    assert_eq!(summaries.len(), 1, "会话行落 root 库");
    assert_eq!(summaries[0].turns.len(), 1, "轮行 running 落库");
    let record = wait_for_record(&captured).await;
    assert_eq!(
        record["record"]["turnId"],
        serde_json::json!(summary.turn_id),
        "Channel 收 record 收尾（终态不依赖 invoke 返回）"
    );
}

#[tokio::test]
async fn agent_start_with_空白cwd回落root不当不存在目录() {
    for blank in ["", "   ", "\t\n "] {
        let env = Env::new("cwd-blank");
        let app = app_with_stores(&env);
        let stores = app.state::<WorkspaceStores>();
        let root = env.root_of("alpha");
        seed_fast_failing_default(stores.inner(), "空白根");

        let summary = agent_start_with(
            app.handle().clone(),
            discarding_channel(),
            root.clone(),
            "空白 cwd 一轮".to_owned(),
            AgentPermissionMode::BypassPermissions,
            None,
            None,
            None,
            None,
            Some(blank.to_owned()),
        )
        .await
        .unwrap_or_else(|error| panic!("空白 cwd {blank:?} 应回落 root 不报错，实际: {error}"));

        let summaries = sessions_in_root(stores.inner(), &root);
        assert_eq!(summaries.len(), 1, "空白 cwd {blank:?} 的会话行落 root 库");
        assert_eq!(summaries[0].row.id, summary.session_id);
    }
}

#[tokio::test]
async fn agent_start_with归属写侧source与source_ref原样落库() {
    let env = Env::new("cwd-provenance");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    seed_fast_failing_default(stores.inner(), "归属");

    let summary = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "归属一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        Some("change".to_owned()),
        Some(CHAT_CHANGE_ID.to_owned()),
        None,
        Some(env.ws("wt").to_string_lossy().into_owned()),
    )
    .await
    .expect("归属字段不影响发起");

    let summaries = sessions_in_root(stores.inner(), &root);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].row.id, summary.session_id);
    assert_eq!(
        summaries[0].row.provenance.source, "change",
        "source 写侧原样落库（AC-3 写侧前置）"
    );
    assert_eq!(
        summaries[0].row.provenance.source_ref.as_deref(),
        Some(CHAT_CHANGE_ID),
        "source_ref 即 change id（不 trim / 不规范化）"
    );
}

#[tokio::test]
async fn agent_start_with_cwd不存在时显式err且零落库零推流() {
    let env = Env::new("cwd-missing");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let missing = env
        .ws_root
        .path()
        .join("gone-wt")
        .to_string_lossy()
        .into_owned();

    let (channel, captured) = capturing_channel();
    let error = agent_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        "目录缺失一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        Some(missing.clone()),
    )
    .await
    .expect_err("cwd 非存在目录必须显式 Err");

    assert!(
        error.contains("worktree 目录不存在（可能已被手动删除）"),
        "Err 文案含缺失判据，实际: {error}"
    );
    assert!(
        error.contains(&missing),
        "Err 文案含路径原文，实际: {error}"
    );
    assert!(
        sessions_in_root(stores.inner(), &root).is_empty(),
        "未进引擎：零会话行（无会话即无轮行）"
    );
    assert!(
        snapshot_of(&captured).is_empty(),
        "未进引擎：零信封（不推流）"
    );
}

#[tokio::test]
async fn agent_start_with_cwd指向文件时按is_dir判据err() {
    let env = Env::new("cwd-file");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let file = env.ws_root.path().join("not-a-dir.txt");
    fs::write(&file, "不是目录").expect("落临时文件 fixture");
    assert!(file.is_file(), "fixture 确为文件而非目录");
    let as_dir = file.to_string_lossy().into_owned();

    let (channel, captured) = capturing_channel();
    let error = agent_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        "指向文件一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        Some(as_dir.clone()),
    )
    .await
    .expect_err("cwd 指向文件（存在但非目录）必须 Err");

    assert!(
        error.contains("worktree 目录不存在（可能已被手动删除）"),
        "判据是 is_dir 而非 exists，实际: {error}"
    );
    assert!(error.contains(&as_dir));
    assert!(sessions_in_root(stores.inner(), &root).is_empty(), "零落库");
    assert!(snapshot_of(&captured).is_empty(), "零推流");
}

#[tokio::test]
async fn agent_start_with_blank_root守卫先于cwd校验() {
    let env = Env::new("cwd-guard-order");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();

    for blank in ["", "   "] {
        let missing = env
            .ws_root
            .path()
            .join("gone-wt")
            .to_string_lossy()
            .into_owned();
        let error = agent_start_with(
            app.handle().clone(),
            discarding_channel(),
            blank.to_owned(),
            "守卫序一轮".to_owned(),
            AgentPermissionMode::BypassPermissions,
            None,
            None,
            None,
            None,
            Some(missing),
        )
        .await
        .expect_err("blank root 先行 Err");

        assert!(
            error.contains("非法 root: 不得为空白（无 cwd 无从发起）"),
            "root 校验先于 cwd 校验（错误面不串味），实际: {error}"
        );
        assert!(
            !error.contains("worktree 目录不存在"),
            "cwd 校验不得抢先，实际: {error}"
        );
    }
    assert!(
        stores
            .inner()
            .for_root(&env.root_of("alpha"))
            .expect("for_root 应成功")
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "守卫拦截零落库"
    );
}

#[tokio::test]
async fn agent_start_with_cwd超长字符串不截断() {
    let env = Env::new("cwd-long");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let long = "a".repeat(1200);
    let long_path = env
        .ws_root
        .path()
        .join(&long)
        .to_string_lossy()
        .into_owned();
    assert!(long_path.contains(&long), "fixture 路径含 1200 字符段");

    let error = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "超长 cwd 一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        Some(long_path.clone()),
    )
    .await
    .expect_err("超长非存在路径必须 Err");

    assert!(
        error.contains(&long_path),
        "文案含完整路径（不截断、不深埋进引擎错误），实际长度: {}",
        error.len()
    );
    assert!(sessions_in_root(stores.inner(), &root).is_empty(), "零落库");
}

#[tokio::test]
async fn agent_start_with_cwd特殊字符各自err不panic() {
    let env = Env::new("cwd-special");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    for special in ["wt\nwith\nnewline", "wt\0null", "工作树-🚀"] {
        let error = agent_start_with(
            app.handle().clone(),
            discarding_channel(),
            root.clone(),
            "特殊 cwd 一轮".to_owned(),
            AgentPermissionMode::BypassPermissions,
            None,
            None,
            None,
            None,
            Some(special.to_owned()),
        )
        .await
        .expect_err("非目录的特殊字符串 cwd 必须 Err");
        assert!(
            error.contains("worktree 目录不存在"),
            "特殊字符 cwd {special:?} 走同一 Err 面，实际: {error}"
        );
    }
    assert!(sessions_in_root(stores.inner(), &root).is_empty(), "零落库");

    // 含 emoji 的串若为真目录则接受（UTF-8 路径不作转义改写）
    seed_fast_failing_default(stores.inner(), "emoji");
    let unicode_dir = env.ws("工作树-🚀").to_string_lossy().into_owned();
    let summary = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "emoji 目录一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        Some(unicode_dir),
    )
    .await
    .expect("emoji 目录存在即接受");

    let summaries = sessions_in_root(stores.inner(), &root);
    assert_eq!(summaries.len(), 1, "会话行落 root 库");
    assert_eq!(summaries[0].row.id, summary.session_id);
}

#[tokio::test]
async fn agent_start_with_cwd等于root时与缺省等价() {
    let env = Env::new("cwd-equals-root");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    seed_fast_failing_default(stores.inner(), "同根");

    let summary = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "cwd 等于 root 一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        Some(root.clone()),
    )
    .await
    .expect("cwd 等于 root（真目录）零 Err");

    let summaries = sessions_in_root(stores.inner(), &root);
    assert_eq!(summaries.len(), 1, "会话行落同一库");
    assert_eq!(summaries[0].row.id, summary.session_id);
    assert_eq!(workspace_db_count(&env), 1, "不派生第二份库");
}

#[tokio::test]
async fn agent_start_with_cwd合法而root不可寻址时for_root错误透传() {
    let env = Env::new("cwd-root-unaddressable");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let missing_root = env
        .data_dir
        .path()
        .join("absent-ws")
        .to_string_lossy()
        .into_owned();

    let error = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        missing_root,
        "root 不可寻址一轮".to_owned(),
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
        Some(env.ws("wt").to_string_lossy().into_owned()),
    )
    .await
    .expect_err("root 不可寻址必须 Err（store 半边不以 cwd 兜底寻址）");

    assert!(
        error.contains("canonicalize") && error.contains("absent-ws"),
        "for_root 的 Err 透传不 panic，实际: {error}"
    );
    assert_eq!(
        workspace_db_count(&env),
        0,
        "cwd 合法也不为 store 半边铸库（双 root 不变量反向锚）"
    );
    assert!(
        stores
            .inner()
            .for_root(&env.root_of("alpha"))
            .expect("for_root 应成功")
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "零落库"
    );
}

#[tokio::test]
async fn agent_start_with_相对路径cwd按is_dir实际结果执行() {
    let env = Env::new("cwd-relative");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    seed_fast_failing_default(stores.inner(), "相对路径");

    for relative in [".", ".."] {
        let before = sessions_in_root(stores.inner(), &root).len();
        let summary = agent_start_with(
            app.handle().clone(),
            discarding_channel(),
            root.clone(),
            format!("相对路径 {relative} 一轮"),
            AgentPermissionMode::BypassPermissions,
            None,
            None,
            None,
            None,
            Some(relative.to_owned()),
        )
        .await
        .unwrap_or_else(|error| panic!("相对路径 {relative} 为目录应接受，实际: {error}"));

        let summaries = sessions_in_root(stores.inner(), &root);
        assert_eq!(
            summaries.len(),
            before + 1,
            "相对 cwd 不 panic 且会话行落 root 库"
        );
        assert!(
            summaries
                .iter()
                .any(|item| item.row.id == summary.session_id),
            "新会话行在 root 库"
        );
    }
}

// ---------------------------------------------------------------------------
// list_change_chat_sessions：某 change 的 chat 会话清单（探测语义薄包装，AC-3）
// ——`source` 词常量收在命令层、`source_ref` 即 change id 原样精确匹配（流程
// 四段 `{changeId}/{phase}/{role}/{attempt}` 与 `<id>x` 前缀混淆项零命中）；
// blank root / 未知 id / 空串 → 空页信封。
// ---------------------------------------------------------------------------

/// change chat 会话行 fixture（`source = "change"`；`updated_at` 显式给定以
/// 驱动分页序，非 mock 时钟——排序键即 fixture 数值）。
fn seed_chat_session(store: &store::Store, id: &str, source_ref: Option<&str>, updated_at: i64) {
    store
        .create_session(&SessionRecord {
            id: id.to_owned(),
            engine_session_id: None,
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                agent_instance_id: None,
                model: Some("m-high".to_owned()),
                permission_mode: AgentPermissionMode::BypassPermissions,
            },
            source: "change".to_owned(),
            source_ref: source_ref.map(str::to_owned),
            created_at: updated_at,
            updated_at,
        })
        .expect("落 chat 会话行 fixture");
}

/// chat 归属 fixture 底座：2 条本 change 的 chat + 1 条流程四段 + 1 条他 change。
fn seed_chat_world(store: &store::Store) {
    seed_chat_session(store, "ses-chat-1", Some(CHAT_CHANGE_ID), 1_727_000_000_001);
    seed_chat_session(store, "ses-chat-2", Some(CHAT_CHANGE_ID), 1_727_000_000_002);
    let flow_ref = format!("{CHAT_CHANGE_ID}/implement/worker/1");
    seed_sdk_session(store, "ses-flow-1", None, "change", Some(flow_ref.as_str()));
    seed_chat_session(
        store,
        "ses-other-chat",
        Some("chg-other"),
        1_727_000_000_003,
    );
}

#[test]
fn list_change_chat_sessions只命中本change的chat归属会话() {
    let env = Env::new("chat-list-hit");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    seed_chat_world(&store);

    let page = list_change_chat_sessions(
        state.clone(),
        root.clone(),
        CHAT_CHANGE_ID.to_owned(),
        0,
        20,
    )
    .expect("清单应成功");

    let ids: Vec<&str> = page
        .sessions
        .iter()
        .map(|item| item.row.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["ses-chat-2", "ses-chat-1"],
        "updated_at 降序（本 change chat 两条）"
    );
    assert_eq!(
        page.total, 2,
        "total = 过滤集全量（流程四段与他 change 零命中）"
    );

    // 调试页 `change` 筛选照常可见（读面不被本命令影响）
    let debug_page = agent_sessions(state, root, Some("change".to_owned()), None, 0, 20)
        .expect("调试页清单应成功");
    assert_eq!(debug_page.total, 4, "同 source 的会话仍全部可见");
}

#[test]
fn list_change_chat_sessions与agent_sessions同参数信封逐字段全等() {
    let env = Env::new("chat-list-equiv");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    seed_chat_world(&store);
    // 轮行随行：chat 会话带一条已收轮（stats.turnCount 对账面）
    let turn = store
        .begin_agent_turn("ses-chat-2", 1_727_000_000_500)
        .expect("begin 轮行 fixture")
        .id;
    store
        .finish_agent_turn(
            turn,
            &AgentRunRecord {
                id: turn,
                session_id: Some("ses-chat-2".to_owned()),
                status: AgentRunStatus::Completed,
                started_at: 1_727_000_000_500,
                finished_at: Some(1_727_000_001_500),
                num_turns: Some(1),
                cost_usd: Some(0.1),
                duration_ms: Some(1_000),
                error: None,
            },
        )
        .expect("finish 轮行 fixture");

    let via_chat = list_change_chat_sessions(
        state.clone(),
        root.clone(),
        CHAT_CHANGE_ID.to_owned(),
        0,
        20,
    )
    .expect("chat 清单应成功");
    let via_debug = agent_sessions(
        state,
        root,
        Some("change".to_owned()),
        Some(CHAT_CHANGE_ID.to_owned()),
        0,
        20,
    )
    .expect("通用清单应成功");

    assert_eq!(via_chat.total, via_debug.total, "total 逐字全等");
    assert_eq!(
        via_chat.sessions, via_debug.sessions,
        "sessions 序与逐字段全等（含轮行 / stats.turnCount）"
    );
    assert_eq!(
        via_chat.sessions[0].stats.turn_count, 1,
        "轮行与统计随行（聚合面不被薄包装改写）"
    );
}

#[test]
fn list_change_chat_sessions分页信封与updated_at降序() {
    let env = Env::new("chat-list-paged");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    for (id, updated_at) in [
        ("ses-pg-1", 1_727_000_000_001_i64),
        ("ses-pg-2", 1_727_000_000_002),
        ("ses-pg-3", 1_727_000_000_003),
    ] {
        seed_chat_session(&store, id, Some(CHAT_CHANGE_ID), updated_at);
    }

    let first =
        list_change_chat_sessions(state.clone(), root.clone(), CHAT_CHANGE_ID.to_owned(), 0, 1)
            .expect("首页应成功");
    assert_eq!(first.sessions.len(), 1);
    assert_eq!(
        first.sessions[0].row.id, "ses-pg-3",
        "offset 0 取 updated_at 最大者"
    );
    assert_eq!(first.total, 3, "total 为过滤集全量（非本页长度）");

    let second =
        list_change_chat_sessions(state.clone(), root.clone(), CHAT_CHANGE_ID.to_owned(), 1, 1)
            .expect("第二页应成功");
    assert_eq!(second.sessions[0].row.id, "ses-pg-2", "offset 1 取第二条");
    assert_eq!(second.total, 3);

    // 序为 updated_at 降序；并列按 id 降序（行层排定）
    seed_chat_session(&store, "ses-tie-b", Some(CHAT_CHANGE_ID), 1_727_000_000_009);
    seed_chat_session(&store, "ses-tie-a", Some(CHAT_CHANGE_ID), 1_727_000_000_009);
    let tied = list_change_chat_sessions(state, root, CHAT_CHANGE_ID.to_owned(), 0, 2)
        .expect("并列页应成功");
    let tied_ids: Vec<&str> = tied
        .sessions
        .iter()
        .map(|item| item.row.id.as_str())
        .collect();
    assert_eq!(
        tied_ids,
        vec!["ses-tie-b", "ses-tie-a"],
        "updated_at 并列按 id 降序"
    );
}

#[test]
fn list_change_chat_sessions流程四段与前缀混淆项零误入() {
    let env = Env::new("chat-list-exact");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    for (id, reference) in [
        (
            "ses-flow-a",
            format!("{CHAT_CHANGE_ID}/implement/executor/1"),
        ),
        ("ses-flow-b", format!("{CHAT_CHANGE_ID}/test-gen/worker/2")),
        ("ses-prefix", format!("{CHAT_CHANGE_ID}x")),
    ] {
        seed_sdk_session(&store, id, None, "change", Some(reference.as_str()));
    }

    let page = list_change_chat_sessions(state, root, CHAT_CHANGE_ID.to_owned(), 0, 20)
        .expect("清单应成功");
    assert!(
        page.sessions.is_empty(),
        "精确相等而非前缀匹配（流程四段与 `<id>x` 零命中），实际: {:?}",
        page.sessions
            .iter()
            .map(|item| item.row.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(page.total, 0, "total 同样零命中");
}

#[test]
fn list_change_chat_sessions_blank_root空页且不寻址库() {
    let env = Env::new("chat-list-blank");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    // 真实数据在位：blank root 守卫先行（不得命中）
    let store = state
        .for_root(&env.root_of("alpha"))
        .expect("for_root 应成功");
    seed_chat_world(&store);
    let before = workspace_db_count(&env);

    for blank in ["", "   "] {
        let page = list_change_chat_sessions(
            state.clone(),
            blank.to_owned(),
            CHAT_CHANGE_ID.to_owned(),
            0,
            20,
        )
        .expect("blank root 返回空页不报错");
        assert!(page.sessions.is_empty(), "blank root {blank:?} 零会话");
        assert_eq!(page.total, 0, "blank root 空页信封 total=0");
    }
    assert_eq!(
        workspace_db_count(&env),
        before,
        "守卫先行：不寻址库、不铸新库"
    );
}

#[test]
fn list_change_chat_sessions未知change_id返回空页信封() {
    let env = Env::new("chat-list-unknown");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    seed_chat_world(&store);

    let page = list_change_chat_sessions(state, root, "chg-never-seen".to_owned(), 0, 20)
        .expect("探测语义选择空页而非 Err");
    assert!(page.sessions.is_empty());
    assert_eq!(page.total, 0, "未知 change id → 空页信封（不 Err）");
}

#[test]
fn list_change_chat_sessions_id空串只匹配空串source_ref不当全量() {
    let env = Env::new("chat-list-empty-id");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    seed_chat_world(&store);
    // source_ref 恰为空串的 chat 会话（空串 ≠ 传 None，只命中该条）
    seed_chat_session(&store, "ses-blank-ref", Some(""), 1_727_000_000_004);

    let page =
        list_change_chat_sessions(state, root, String::new(), 0, 20).expect("id 空串不 panic");
    let ids: Vec<&str> = page
        .sessions
        .iter()
        .map(|item| item.row.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["ses-blank-ref"],
        "空串作精确 source_ref，不回落全量"
    );
    assert_eq!(page.total, 1);
}

#[test]
fn list_change_chat_sessions_id特殊与超长原样精确匹配() {
    let env = Env::new("chat-list-special-id");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    let long_id = "chg-".to_owned() + &"0".repeat(1200);
    assert!(long_id.len() > 1000, "fixture 超长 id");
    seed_chat_session(&store, "ses-emoji-ref", Some("chg-🚀"), 1_727_000_000_001);
    seed_chat_session(
        &store,
        "ses-newline-ref",
        Some("chg\nid"),
        1_727_000_000_002,
    );
    seed_chat_session(
        &store,
        "ses-long-ref",
        Some(long_id.as_str()),
        1_727_000_000_003,
    );

    for (id, expected) in [
        ("chg-🚀", "ses-emoji-ref"),
        ("chg\nid", "ses-newline-ref"),
        (long_id.as_str(), "ses-long-ref"),
    ] {
        let page = list_change_chat_sessions(state.clone(), root.clone(), id.to_owned(), 0, 20)
            .expect("特殊 / 超长 id 不 panic");
        assert_eq!(page.total, 1, "原样精确匹配命中一条");
        assert_eq!(page.sessions[0].row.id, expected);
    }

    // 零命中的特殊形态同样空页不 panic
    let miss = list_change_chat_sessions(state, root, "chg-🚀-δ".to_owned(), 0, 20)
        .expect("零命中不 panic");
    assert!(miss.sessions.is_empty());
    assert_eq!(miss.total, 0);
}

#[test]
fn list_change_chat_sessions越界offset与极值offset空页且total全量() {
    let env = Env::new("chat-list-oob");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    for index in 1..=3 {
        seed_chat_session(
            &store,
            &format!("ses-oob-{index}"),
            Some(CHAT_CHANGE_ID),
            1_727_000_000_000 + index as i64,
        );
    }

    let oob = list_change_chat_sessions(
        state.clone(),
        root.clone(),
        CHAT_CHANGE_ID.to_owned(),
        4,
        20,
    )
    .expect("越界 offset 合法空页");
    assert!(oob.sessions.is_empty(), "offset = total + 1 空页");
    assert_eq!(oob.total, 3, "越界空页仍携过滤集全量");

    let extreme = list_change_chat_sessions(
        state.clone(),
        root.clone(),
        CHAT_CHANGE_ID.to_owned(),
        u32::MAX,
        20,
    )
    .expect("offset 极值不溢出 panic");
    assert!(extreme.sessions.is_empty(), "offset = u32::MAX 空页");
    assert_eq!(extreme.total, 3, "total 恒为过滤集全量");
}

#[test]
fn list_change_chat_sessions_limit零空页与超上限截断() {
    let env = Env::new("chat-list-limit");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");
    // 501 条 chat 会话（超大列表边界；limit 上限由 store 侧截断）
    for index in 0..501 {
        seed_chat_session(
            &store,
            &format!("ses-big-{index:03}"),
            Some(CHAT_CHANGE_ID),
            1_727_000_000_000 + index,
        );
    }

    let zero =
        list_change_chat_sessions(state.clone(), root.clone(), CHAT_CHANGE_ID.to_owned(), 0, 0)
            .expect("limit = 0 合法空页");
    assert!(zero.sessions.is_empty(), "limit = 0 → 空页");
    assert_eq!(zero.total, 501, "limit = 0 时 total 仍全量");

    let clamped = list_change_chat_sessions(state, root, CHAT_CHANGE_ID.to_owned(), 0, u32::MAX)
        .expect("limit 极值不 panic");
    assert_eq!(
        clamped.sessions.len(),
        500,
        "limit 超 MAX_SESSION_PAGE_LIMIT 截断为 500"
    );
    assert_eq!(clamped.total, 501, "total 不受 limit 截断影响");
}

#[test]
fn list_change_chat_sessions跨root隔离() {
    let env = Env::new("chat-list-iso");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root_a = env.root_of("alpha");
    let root_b = env.root_of("beta");
    let store_a = state.for_root(&root_a).expect("for_root 应成功");
    seed_chat_world(&store_a);

    let in_a = list_change_chat_sessions(state.clone(), root_a, CHAT_CHANGE_ID.to_owned(), 0, 20)
        .expect("A 库清单应成功");
    assert_eq!(in_a.total, 2, "A 库命中本 change 的两条 chat");

    let in_b = list_change_chat_sessions(state, root_b, CHAT_CHANGE_ID.to_owned(), 0, 20)
        .expect("B 库清单应成功");
    assert!(in_b.sessions.is_empty(), "跨 root 隔离：库位 = root 寻址");
    assert_eq!(in_b.total, 0);
}

#[test]
fn list_change_chat_sessions_root不可寻址err透传不panic() {
    let env = Env::new("chat-list-root-err");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let missing = env
        .data_dir
        .path()
        .join("absent-ws")
        .to_string_lossy()
        .into_owned();

    let error = list_change_chat_sessions(state, missing.clone(), CHAT_CHANGE_ID.to_owned(), 0, 20)
        .expect_err("root 不可寻址必须 Err");
    assert!(
        error.contains("canonicalize") && error.contains("absent-ws"),
        "for_root 的 Err 透传不 panic（与三查询命令同口径），实际: {error}"
    );
}
