use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::{
    AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunStatus, StopRegistry,
};
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord, AgentRunRecord,
    SessionConfigSnapshot, SessionRecord, WorkspaceStores,
};

use super::{
    agent_session_transcript, agent_sessions, agent_start_with, agent_stop, session_detail,
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

/// PATH 环境变量修改串行化（进程全局操作；与 change_flow/mod_test 的 PATH
/// 隔离窗口共用 commands 级共享锁）。
use crate::commands::TEST_PATH_LOCK as PATH_LOCK;

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 WorkspaceStores（打开 env
/// 数据根的全局库）与 `Arc<StopRegistry>`（内核治理面挂载，须先于 start 托管）。
fn app_with_stores(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app.manage(Arc::new(StopRegistry::new()));
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

/// cli 默认实例 fixture（返回实例 id）。
fn seed_cli_default(stores: &WorkspaceStores, name: &str) -> i64 {
    let id = stores
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            name.to_owned(),
            AgentEngineKind::Cli,
            None,
        ))
        .expect("落 cli 实例 fixture")
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

    let all = agent_sessions(state.clone(), root.clone(), None, None).expect("清单应成功");
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
    )
    .expect("过滤清单应成功");
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
    )
    .expect("过滤清单应成功");
    assert_eq!(ref42.len(), 1);
    let none_match = agent_sessions(
        state.clone(),
        root.clone(),
        Some("explore".to_owned()),
        Some("99".to_owned()),
    )
    .expect("过滤清单应成功");
    assert!(none_match.is_empty(), "source_ref 不匹配为空");
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
        agent_sessions(state, String::new(), None, None).expect("blank root 清单返回空结果");
    assert!(blank_list.is_empty());
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
                        usage: serde_json::json!({ "inputTokens": 7, "outputTokens": 9 }),
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
    let listed = agent_sessions(state.clone(), root.clone(), None, None).expect("清单应成功");
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
    seed_cli_default(stores.inner(), "守卫实例");

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
    seed_cli_default(stores.inner(), "提前resolve实例");

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", ""); // ask 段 discover 失败（不影响提前 resolve）

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

    // PATH 恢复前等待后台泵收敛（隔离 PATH 下 spawn 失败合成收敛，不触真实 CLI）
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
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
}

#[tokio::test]
async fn agent_start参数转换_session引用与来源与agent透传() {
    let env = Env::new("start-params");
    let app = app_with_stores(&env);
    let stores = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    // 显式 agent（cli）+ Continue 底座会话（cli 快照：跨引擎拒绝先行验证
    // Continue 寻址正确性——若 session_id 未透传则不会命中跨引擎拒绝）
    let cli_id = seed_cli_default(stores.inner(), "参数转换实例");
    let store = stores.inner().for_root(&root).expect("for_root 应成功");
    store
        .create_session(&SessionRecord {
            id: "ses-param-continue".to_owned(),
            engine_session_id: Some("s-cli-1".to_owned()),
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                model: Some("m-high".to_owned()),
                permission_mode: AgentPermissionMode::BypassPermissions,
            },
            source: "explore".to_owned(),
            source_ref: Some("7".to_owned()),
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("落 Continue 底座会话行");

    // Continue + source/sourceRef 透传：跨引擎拒绝（sdk 快照会话以 cli agent 续）
    let result = agent_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "续会话一轮".to_owned(),
        AgentPermissionMode::AcceptEdits,
        Some("ses-param-continue".to_owned()),
        Some("explore".to_owned()),
        Some("7".to_owned()),
        Some(cli_id),
    )
    .await;

    let error = result.expect_err("cli agent 续 sdk 快照会话必须被拒");
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
    seed_cli_default(stores.inner(), "停止链实例");

    let (channel, captured) = capturing_channel();
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", ""); // CLI 缺失 → 合成收敛 failed（Channel 观测）

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
    )
    .await
    .expect("提前 resolve");

    // 停止置位（begin_turn 同步段已登记句柄；若后台泵已收敛除名则 miss 幂等
    // 无副作用——命中/miss 两态均不报错，注册表置位语义由 kernel_test 锁定）
    let registry = app.state::<Arc<StopRegistry>>();
    let _ = registry.request_stop(&summary.session_id);

    // 轮询捕获 Channel 直至终态 Record 信封（后台转发任务；合成收敛 failed）
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

    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    assert_eq!(
        terminal["record"]["sessionId"],
        serde_json::json!(summary.session_id),
        "终态部件与重放同构（running summary 同会话）"
    );
    assert_eq!(
        terminal["record"]["status"],
        serde_json::json!("failed"),
        "CLI 缺失合成收敛 failed 经 Channel 流出"
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
