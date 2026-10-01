use std::path::{Path, PathBuf};
use std::sync::Arc;

use native_db::{Builder, Models};

use agent::{
    AgentMessageRole, AgentPermissionMode, SessionCtx, SessionProvenance, SessionRef, StopRegistry,
};

use crate::{compose_turn, ComposedTurn};
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord,
    SessionConfigSnapshot, SessionRecord, WorkspaceStores,
};

/// PATH 环境变量修改串行化（CLI 臂真实驱动用例）。
use crate::TEST_PATH_LOCK as PATH_LOCK;

/// Err 半边摘取（ComposedTurn / RunningTurn 不实现 Debug，expect_err 不可用）。
fn err_of<T>(result: Result<T, String>) -> String {
    match result {
        Err(error) => error,
        Ok(_) => panic!("必须 Err"),
    }
}

// ---------------------------------------------------------------------------
// 装置：tempdir 真库 WorkspaceStores + fixture 落库
// ---------------------------------------------------------------------------

/// 打开两级库注册表（数据根注入，零环境解析）。
fn open_stores(data_root: &PathBuf) -> WorkspaceStores {
    WorkspaceStores::open(data_root).unwrap_or_else(|e| panic!("WorkspaceStores::open 应成功: {e}"))
}

/// 数据根 + workspace 根双临时目录装置。
struct DualEnv {
    data_root: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl DualEnv {
    fn new(tag: &str) -> Self {
        let data_root = tempfile::Builder::new()
            .prefix(&format!("compose-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("compose-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_root, ws_root }
    }

    fn root(&self) -> String {
        self.ws_root.path().to_string_lossy().into_owned()
    }

    fn stores(&self) -> WorkspaceStores {
        open_stores(&self.data_root.path().to_path_buf())
    }
}

/// sdk provider fixture（端点指向本机 discard 端口，解析仅组装不触网络）。
fn seed_provider(stores: &WorkspaceStores, name: &str) -> i64 {
    let provider = AgentProviderRecord::new(
        name.to_owned(),
        "http://127.0.0.1:9/v1".to_owned(),
        "sk-compose".to_owned(),
        AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
    );
    stores
        .global()
        .upsert_agent_provider(provider)
        .expect("落 provider fixture")
        .id
}

/// sdk 实例 fixture（可指定 provider_id 与默认标记）。
fn seed_sdk_instance(stores: &WorkspaceStores, name: &str, provider_id: Option<i64>) -> i64 {
    let id = stores
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            name.to_owned(),
            AgentEngineKind::Sdk,
            provider_id,
        ))
        .expect("落 sdk 实例 fixture")
        .id;
    stores
        .global()
        .set_default_agent_instance(id)
        .expect("置默认实例 fixture");
    id
}

/// cli 实例 fixture（默认标记）。
fn seed_cli_instance(stores: &WorkspaceStores, name: &str) -> i64 {
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

/// sdk 快照的会话行 fixture（Continue 校验通过形态）。
fn seed_sdk_session(store: &store::Store, id: &str, remote: Option<&str>) -> () {
    let record = SessionRecord {
        id: id.to_owned(),
        engine_session_id: remote.map(str::to_owned),
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Sdk,
            model: Some("m-high".to_owned()),
            permission_mode: AgentPermissionMode::BypassPermissions,
        },
        source: "debug".to_owned(),
        source_ref: None,
        created_at: 1727000000000,
        updated_at: 1727000000000,
    };
    store.create_session(&record).expect("落会话行 fixture");
}

/// 最小密封转录 fixture（Continue 全史重建非空的前提）。
fn seed_transcript(store: &store::Store, session_id: &str) {
    use agent::{AgentBlock, AgentEvent, AgentEventKind};
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

fn ctx(root: &str) -> SessionCtx {
    SessionCtx {
        workspace_root: PathBuf::from(root),
        permission_mode: AgentPermissionMode::BypassPermissions,
    }
}

fn debug_provenance() -> SessionProvenance {
    SessionProvenance {
        source: "debug".to_owned(),
        source_ref: None,
    }
}

// ---------------------------------------------------------------------------
// 解析单点：缺省 / 显式 / 无默认 / 不存在 / provider 两成因（AC-9 下沉半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 缺省解析默认sdk实例_快照承接provider组装与high档模型() {
    let env = DualEnv::new("default-sdk");
    let stores = env.stores();
    let provider_id = seed_provider(&stores, "解析供应");
    seed_sdk_instance(&stores, "解析实例", Some(provider_id));

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
        .expect("缺省解析应成功");

    // 快照定型双面（解析产物的可观测投影）：sdk 引擎 + provider high 档模型
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let running = composed
        .begin(
            SessionRef::New,
            "解析快照一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    drop(running);
    let session = store
        .find_session(&session_id)
        .expect("find_session 应成功")
        .expect("New 建会话行");
    assert_eq!(
        session.config_snapshot.engine,
        AgentEngineKind::Sdk,
        "EngineKind::Sdk 快照承接"
    );
    assert_eq!(
        session.config_snapshot.model,
        Some("m-high".to_owned()),
        "sdk 由引用 provider 组装取 high 档"
    );
}

#[tokio::test]
async fn 显式cli实例解析_快照engine为cli且模型为none() {
    let env = DualEnv::new("explicit-cli");
    let stores = env.stores();
    let cli_id = seed_cli_instance(&stores, "命令行实例");

    let composed = compose_turn(
        &stores,
        Arc::new(StopRegistry::new()),
        &env.root(),
        Some(cli_id),
    )
    .expect("显式 cli 解析应成功");

    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let running = composed
        .begin(
            SessionRef::New,
            "cli 快照一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    drop(running);
    let session = store
        .find_session(&session_id)
        .expect("find_session 应成功")
        .expect("New 建会话行");
    assert_eq!(session.config_snapshot.engine, AgentEngineKind::Cli);
    assert_eq!(
        session.config_snapshot.model, None,
        "cli 臂 EngineConfig::empty()（cli 实例不消费配置，无模型装配概念）"
    );
}

#[test]
fn 空库缺省解析err引导管理页且零落库() {
    let env = DualEnv::new("no-default");
    let stores = env.stores();

    let error = err_of(compose_turn(
        &stores,
        Arc::new(StopRegistry::new()),
        &env.root(),
        None,
    ));

    assert!(
        error.contains("默认 agent") && error.contains("/agents"),
        "Err 文案含管理页引导语义（显式 Err 不静默），实际: {error}"
    );
    // 解析失败不落库：workspace 库无会话行（组合根先解析后建行）
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "解析失败零落库"
    );
}

#[test]
fn 显式不存在agent的id解析err携id记因() {
    let env = DualEnv::new("agent-404");
    let stores = env.stores();

    let error = err_of(compose_turn(
        &stores,
        Arc::new(StopRegistry::new()),
        &env.root(),
        Some(404),
    ));
    assert!(error.contains("404"), "Err 携 id 记因，实际: {error}");
}

#[tokio::test]
async fn provider引用完整性守卫使悬空成因不可达_解析照常成功() {
    let env = DualEnv::new("provider-causes");
    let stores = env.stores();

    // resolve 单点的「引用 provider 不存在」分支为存量数据兜底：store 写入口
    // 以引用完整性守卫（provider 被引用禁止删除）使悬空状态经真库 API 不可达
    let provider_id = seed_provider(&stores, "被引用供应");
    seed_sdk_instance(&stores, "引用实例", Some(provider_id));

    let removed = stores.global().remove_agent_provider(provider_id);
    let error = match removed {
        Err(error) => error,
        Ok(_) => panic!("被引用 provider 删除必须被守卫拦截"),
    };
    assert!(
        error.to_string().contains("禁止删除"),
        "引用完整性守卫记因（悬空成因不可达的不变式半边），实际: {error}"
    );

    // provider 完好在库：解析照常成功（默认实例可发起）
    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
        .expect("provider 完好时缺省解析应成功");
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let running = composed
        .begin(
            SessionRef::New,
            "引用完好一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session = store
        .find_session(&running.session_id)
        .expect("find_session 应成功")
        .expect("New 建会话行");
    assert_eq!(
        session.config_snapshot.model,
        Some("m-high".to_owned()),
        "provider 完好时 high 档模型组装照常"
    );
}

// ---------------------------------------------------------------------------
// Continue 快照校验三形态 + 通过形态
// ---------------------------------------------------------------------------

#[tokio::test]
async fn continue快照校验通过时begin成功且会话行不重复建() {
    let env = DualEnv::new("continue-ok");
    let stores = env.stores();
    let provider_id = seed_provider(&stores, "续话供应");
    seed_sdk_instance(&stores, "续话实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-continue-ok", Some("sdk-11-1727000000001"));
    seed_transcript(&store, "ses-continue-ok");

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
        .expect("解析应成功");
    let running = composed
        .begin(
            SessionRef::Continue {
                id: "ses-continue-ok".to_owned(),
            },
            "续会话一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("快照一致且 remote 在场 → 校验通过");

    // 双 id 回供：沿用既有会话行（begin 同步段不再 create_session）
    assert_eq!(running.session_id, "ses-continue-ok");
    // 轮行照开（begin_turn 落 running 初值行）
    let summaries = store.list_sessions(None, None).expect("清单应成功");
    assert_eq!(summaries.len(), 1, "不重复建会话行");
    assert_eq!(summaries[0].turns.len(), 1, "轮行已开");
    assert_eq!(
        summaries[0].turns[0].status,
        agent::AgentRunStatus::Running,
        "running 初值"
    );
}

#[test]
fn continue会话不存在时显式失败() {
    let env = DualEnv::new("continue-404");
    let stores = env.stores();
    let provider_id = seed_provider(&stores, "续话404供应");
    seed_sdk_instance(&stores, "续话404实例", Some(provider_id));

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
        .expect("解析应成功");
    let error = err_of(composed.begin(
        SessionRef::Continue {
            id: "ses-404".to_owned(),
        },
        "续一轮".to_owned(),
        ctx(&env.root()),
        debug_provenance(),
    ));
    assert!(
        error.contains("会话不存在") && error.contains("ses-404"),
        "会话缺失成因显式失败（AC-3 显式半边），实际: {error}"
    );
}

#[test]
fn continue跨引擎续会话被快照比对拒绝() {
    let env = DualEnv::new("cross-engine");
    let stores = env.stores();
    let provider_id = seed_provider(&stores, "跨引擎供应");
    seed_sdk_instance(&stores, "跨引擎实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    // cli 产出的会话以 sdk 解析 Continue：快照 engine ≠ 解析 kind
    let cli_session = SessionRecord {
        id: "ses-cli-made".to_owned(),
        engine_session_id: Some("s-cli-1".to_owned()),
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Cli,
            model: None,
            permission_mode: AgentPermissionMode::BypassPermissions,
        },
        source: "debug".to_owned(),
        source_ref: None,
        created_at: 1727000000000,
        updated_at: 1727000000000,
    };
    store
        .create_session(&cli_session)
        .expect("落 cli 快照会话行");

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
        .expect("解析应成功");
    let error = err_of(composed.begin(
        SessionRef::Continue {
            id: "ses-cli-made".to_owned(),
        },
        "跨引擎续一轮".to_owned(),
        ctx(&env.root()),
        debug_provenance(),
    ));
    assert!(
        error.contains("非当前引擎产出"),
        "跨引擎拒绝显式（本期一律拒绝），实际: {error}"
    );
}

#[test]
fn continue引擎句柄缺失时拒绝且三成因消息互不重合() {
    let env = DualEnv::new("remote-missing");
    let stores = env.stores();
    let provider_id = seed_provider(&stores, "句柄缺失供应");
    seed_sdk_instance(&stores, "句柄缺失实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-no-remote", None);

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
        .expect("解析应成功");
    let error = err_of(composed.begin(
        SessionRef::Continue {
            id: "ses-no-remote".to_owned(),
        },
        "续一轮".to_owned(),
        ctx(&env.root()),
        debug_provenance(),
    ));
    assert!(
        error.contains("缺少引擎句柄"),
        "remote 句柄缺失成因，实际: {error}"
    );

    // 三成因（会话不存在 / 跨引擎 / remote 缺失）消息互不重合
    assert!(
        !error.contains("会话不存在") && !error.contains("非当前引擎产出"),
        "成因消息互不重合"
    );
}

// ---------------------------------------------------------------------------
// 装配产物组合（sink/query/门面/内核真实装配：begin_turn/drive 可直接驱动）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 装配产物真实驱动_cli臂隔离path下failed收敛且会话行轮行转录齐落库() {
    let env = DualEnv::new("drive-real");
    let stores = env.stores();
    let cli_id = seed_cli_instance(&stores, "驱动实例");

    let composed = compose_turn(
        &stores,
        Arc::new(StopRegistry::new()),
        &env.root(),
        Some(cli_id),
    )
    .expect("解析应成功");

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", ""); // ask 段 discover 失败 → 合成收敛

    let running = composed
        .begin(
            SessionRef::New,
            "装配产物驱动一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: "debug".to_owned(), // provenance source 缺省 debug 承接
                source_ref: None,
            },
        )
        .expect("begin 应成功");

    let outcome = running.drive(|_output| {}).await;

    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    assert_eq!(
        outcome.status,
        agent::AgentRunStatus::Failed,
        "CLI 缺失合成收敛 failed"
    );
    // sink 真实装配：会话行 + 轮行 + 转录齐落库；provenance debug 承接
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let summaries = store.list_sessions(None, None).expect("清单应成功");
    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].row.provenance.source, "debug",
        "debug 缺省承接"
    );
    assert_eq!(summaries[0].turns.len(), 1);
    assert_eq!(summaries[0].turns[0].status, agent::AgentRunStatus::Failed);
    // query 真实装配：转录重放含合成 TurnDone
    let transcript = store
        .list_session_events(&summaries[0].row.id)
        .expect("转录应成功");
    assert_eq!(transcript.len(), 1, "恰合成收敛一条密封事件");
    assert!(matches!(
        &transcript[0].kind,
        agent::AgentEventKind::TurnDone { subtype, is_error: true, .. } if subtype == "error_cli_missing"
    ));
}

#[tokio::test]
async fn 装配产物含查询面_continue校验经真实库行成立() {
    let env = DualEnv::new("query-assemble");
    let stores = env.stores();
    let provider_id = seed_provider(&stores, "查询面供应");
    seed_sdk_instance(&stores, "查询面实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-query", Some("sdk-77"));
    seed_transcript(&store, "ses-query");

    // ComposedTurn 产物即真实装配（query 直查 store 行 → 校验通过形态复用）
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
            .expect("解析应成功");
    composed
        .begin(
            SessionRef::Continue {
                id: "ses-query".to_owned(),
            },
            "查询面续一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("真实查询面装配的 Continue 校验应通过");
}

/// 预置仅注册 AgentInstanceRecord 的存量全局库并写入「engine=sdk 且
/// provider_id=None」实例（native_db 裸构造绕过 store 写入口守卫——该形态为
/// 存量数据兜底，resolve 单点缺 provider 分支的唯一可达面）。
fn preset_global_with_providerless_sdk_instance(path: &Path, name: &str) {
    let mut models = Models::new();
    models
        .define::<store::AgentInstanceRecord>()
        .expect("定义 AgentInstanceRecord 失败");
    let db = Builder::new()
        .create(&models, path)
        .expect("预置存量全局库失败");
    let rw = db.rw_transaction().expect("开启写事务失败");
    let mut instance = store::AgentInstanceRecord::new(name.to_owned(), AgentEngineKind::Sdk, None);
    instance.is_default = true;
    rw.insert(instance)
        .expect("写入缺 provider 的 sdk 实例失败");
    rw.commit().expect("提交预置事务失败");
}

#[test]
fn sdk实例缺provider_id的存量形态解析err引导管理页() {
    let env = DualEnv::new("provider-missing");
    // 存量全局库直接落打开点路径（WorkspaceStores::open 即读该文件）
    preset_global_with_providerless_sdk_instance(
        &env.data_root.path().join("desktop-global.redb"),
        "存量实例",
    );
    let stores = env.stores();

    let error = err_of(compose_turn(
        &stores,
        Arc::new(StopRegistry::new()),
        &env.root(),
        None,
    ));

    assert!(
        error.contains("未配置 provider") && error.contains("/agents"),
        "缺 provider_id 成因显式 Err 引导管理页修正，实际: {error}"
    );
    // 解析失败零落库（不静默回退 CLI）
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "解析失败零落库"
    );
}
