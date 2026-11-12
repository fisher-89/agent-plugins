use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use native_db::{Builder, Models};
use tokio::sync::mpsc;

use agent::{
    AgentMessageRole, AgentPermissionMode, AgentRunner, AgentSession, AgentStartError, ModelLevel,
    RunHandle, SessionCtx, SessionKernel, SessionOpen, SessionProvenance, SessionQuery, SessionRef,
    SessionStreamRegistry, StopRegistry,
};

use crate::store_port::StoreQuery;
use crate::{compose_turn, ComposedTurn};
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, SessionConfigSnapshot, SessionRecord, WorkspaceStores, CLI_TENANT_RETIRED,
};

/// sdk provider fixture 的可指定窗长变体（AC-9 组合根接线用）。
fn seed_provider_with_context_length(
    stores: &WorkspaceStores,
    name: &str,
    context_length: Option<u64>,
) -> i64 {
    let provider = AgentProviderRecord::new(
        name.to_owned(),
        AgentProviderProtocol::Openai,
        "http://127.0.0.1:9/v1".to_owned(),
        "sk-compose".to_owned(),
        AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
        context_length,
        None,
    );
    stores
        .global()
        .upsert_agent_provider(provider)
        .expect("落 provider fixture")
        .id
}

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
            .prefix(&format!("compose-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("compose-test-{tag}-root-"))
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

/// sdk provider fixture（端点指向本机 discard 端口，解析仅组装不触网络）。
fn seed_provider(stores: &WorkspaceStores, name: &str) -> i64 {
    seed_provider_at(stores, name, "http://127.0.0.1:9/v1")
}

/// 端点可指定的 sdk provider fixture（脚本化桩端点注入面）。
fn seed_provider_at(stores: &WorkspaceStores, name: &str, base_url: &str) -> i64 {
    let provider = AgentProviderRecord::new(
        name.to_owned(),
        AgentProviderProtocol::Openai,
        base_url.to_owned(),
        "sk-compose".to_owned(),
        AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
        None,
        None,
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

/// 预置存量全局库并写入 cli 实例行（native_db 裸构造绕过 store 写入口拒面
/// ——「存量 cli 行」为 AC-4 / AC-5 拒收行为的被测前提数据形态；须在
/// `WorkspaceStores::open` 之前落盘，返回记录 id）。
fn preset_cli_instance(env: &DualEnv, id: i64, name: &str, is_default: bool) -> i64 {
    let path = env.data_root.path().join("desktop-global.redb");
    let mut models = Models::new();
    models
        .define::<store::AgentInstanceRecord>()
        .expect("定义 AgentInstanceRecord 失败");
    let db = Builder::new()
        .create(&models, &path)
        .expect("预置存量全局库失败");
    let rw = db.rw_transaction().expect("开启写事务失败");
    let mut instance = store::AgentInstanceRecord::new(name.to_owned(), AgentEngineKind::Cli, None);
    instance.id = id;
    instance.is_default = is_default;
    rw.insert(instance).expect("写入存量 cli 实例失败");
    rw.commit().expect("提交预置事务失败");
    id
}

/// sdk 快照的会话行 fixture（Continue 校验通过形态）。
fn seed_sdk_session(store: &store::Store, id: &str, remote: Option<&str>) {
    let record = SessionRecord {
        id: id.to_owned(),
        engine_session_id: remote.map(str::to_owned),
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Sdk,
            agent_instance_id: None,
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
        AgentEvent::stamp(
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

fn ctx(root: &str) -> SessionCtx {
    SessionCtx {
        workspace_root: PathBuf::from(root),
        permission_mode: AgentPermissionMode::BypassPermissions,
        model_level: ModelLevel::High,
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
    let provider_id = seed_provider(stores, "解析供应");
    seed_sdk_instance(stores, "解析实例", Some(provider_id));

    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
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
async fn 快照模型按轮级档位定型_low轮取低档high轮取高档() {
    let env = DualEnv::new("model-level");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "双档供应");
    seed_sdk_instance(stores, "双档实例", Some(provider_id));
    let composed = compose_turn(
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
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    let running = composed
        .begin(
            SessionRef::New,
            "低档快照一轮".to_owned(),
            SessionCtx {
                model_level: ModelLevel::Low,
                ..ctx(&env.root())
            },
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session = store
        .find_session(&running.session_id)
        .expect("find_session 应成功")
        .expect("New 建会话行");
    assert_eq!(
        session.config_snapshot.model,
        Some("m-low".to_owned()),
        "Low 档轮快照取 provider low 档（该轮实际选型定型）"
    );

    let running = composed
        .begin(
            SessionRef::New,
            "高档快照一轮".to_owned(),
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
        "High 档轮（缺省档位）快照取 provider high 档"
    );
}

#[tokio::test]
async fn 显式cli实例解析_逐字拒收退役文案且零落库() {
    let env = DualEnv::new("explicit-cli");
    let cli_id = preset_cli_instance(&env, 41, "命令行实例", false);
    let stores = env.stores();

    let error = err_of(compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        Some(cli_id),
        Path::new(&env.root()),
        None,
    ));

    assert_eq!(
        error, CLI_TENANT_RETIRED,
        "显式路径对 engine: Cli 实例逐字拒收（AC-4），实际: {error}"
    );
    // 拒收不落库不静默回退（不构造引擎）
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None,)
            .expect("清单应成功")
            .is_empty(),
        "拒收零落库"
    );
}

#[test]
fn 缺省发起默认实例为存量cli_同臂同文案拒且不清默认标记零落库() {
    let env = DualEnv::new("default-cli");
    let cli_id = preset_cli_instance(&env, 42, "默认命令行实例", true);
    let stores = env.stores();

    let error = err_of(compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    ));

    assert_eq!(
        error, CLI_TENANT_RETIRED,
        "缺省路径同臂同文案（AC-5 缺省发起半边），实际: {error}"
    );
    // 不清 is_default、零数据写入（拒收先于任何写面）
    let default = stores
        .global()
        .default_agent_instance()
        .expect("default 解析应成功")
        .expect("默认实例仍在库");
    assert_eq!(default.id, cli_id, "存量默认 cli 实例原样在库");
    assert!(default.is_default, "拒收不清 is_default");
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None,)
            .expect("清单应成功")
            .is_empty(),
        "拒收零落库"
    );
}

#[test]
fn 空库缺省解析err引导管理页且零落库() {
    let env = DualEnv::new("no-default");
    let stores = env.stores();

    let error = err_of(compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
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
            .list_sessions(None, None,)
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
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        Some(404),
        Path::new(&env.root()),
        None,
    ));
    assert!(error.contains("404"), "Err 携 id 记因，实际: {error}");
}

#[tokio::test]
async fn provider引用完整性守卫使悬空成因不可达_解析照常成功() {
    let env = DualEnv::new("provider-causes");
    let stores = env.stores();

    // resolve 单点的「引用 provider 不存在」分支为存量数据兜底：store 写入口
    // 以引用完整性守卫（provider 被引用禁止删除）使悬空状态经真库 API 不可达
    let provider_id = seed_provider(stores, "被引用供应");
    seed_sdk_instance(stores, "引用实例", Some(provider_id));

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
    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
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
    let provider_id = seed_provider(stores, "续话供应");
    seed_sdk_instance(stores, "续话实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-continue-ok", Some("sdk-11-1727000000001"));
    seed_transcript(&store, "ses-continue-ok");

    let composed = compose_turn(
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
    let provider_id = seed_provider(stores, "续话404供应");
    seed_sdk_instance(stores, "续话404实例", Some(provider_id));

    let composed = compose_turn(
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
    let provider_id = seed_provider(stores, "跨引擎供应");
    seed_sdk_instance(stores, "跨引擎实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    // cli 产出的会话以 sdk 解析 Continue：快照 engine ≠ 解析 kind
    let cli_session = SessionRecord {
        id: "ses-cli-made".to_owned(),
        engine_session_id: Some("s-cli-1".to_owned()),
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Cli,
            agent_instance_id: None,
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

    let composed = compose_turn(
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

#[tokio::test]
async fn continue未开轮会话首轮放行并定型快照() {
    let env = DualEnv::new("unstarted-first-turn");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "首轮供应");
    seed_sdk_instance(stores, "首轮实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    // 未开轮：无引擎句柄（建档预建空会话形态）
    seed_sdk_session(&store, "ses-fresh", None);

    let composed = compose_turn(
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
    let running = composed
        .begin(
            SessionRef::Continue {
                id: "ses-fresh".to_owned(),
            },
            "首轮提问".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("未开轮会话首轮应放行");
    assert_eq!(running.session_id, "ses-fresh", "沿用预建会话 id（非新铸）");

    // 快照定型：占位快照被本轮真实装配覆写（engine / permission 两面）
    let stamped = store
        .find_session("ses-fresh")
        .expect("读会话")
        .expect("会话在库");
    assert_eq!(
        stamped.config_snapshot.engine,
        store::AgentEngineKind::Sdk,
        "快照定型为本轮解析引擎"
    );
}

/// 未开轮判定收紧：**开过轮但句柄未上报**的会话不算未开轮——不走未开轮
/// 首轮放行（快照不重定型），续会话治理照走且 `prior_handle` 以 None 透传
/// 照常续接（DD2 硬门槛退役，句柄缺失不再显式 Err）。
#[tokio::test]
async fn continue已开轮但句柄未上报仍走续会话治理() {
    let env = DualEnv::new("ran-no-handle");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "缺柄供应");
    seed_sdk_instance(stores, "缺柄实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    // 无引擎句柄，但已有轮行（开过轮）
    seed_sdk_session(&store, "ses-ran", None);
    store
        .begin_agent_turn("ses-ran", 1727000000000)
        .expect("开轮行");

    let composed = compose_turn(
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
    let running = composed
        .begin(
            SessionRef::Continue {
                id: "ses-ran".to_owned(),
            },
            "续一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("缺句柄不再硬拒（DD2）：prior_handle 透传 None 照常续接");

    // 已开轮 → 不走未开轮首轮放行：既有快照原样保留（不重定型）
    let session = store
        .find_session("ses-ran")
        .expect("find_session 应成功")
        .expect("续接沿用既有会话行");
    assert_eq!(
        session.engine_session_id, None,
        "留痕列不被回写（prior_handle 透传不加工）"
    );
    assert_eq!(
        session.config_snapshot.agent_instance_id, None,
        "已开轮不走未开轮放行：快照不被本轮装配覆写"
    );
    let summaries = store.list_sessions(None, None).expect("清单应成功");
    assert_eq!(summaries.len(), 1, "不重复建会话行");
    assert_eq!(summaries[0].turns.len(), 2, "既有轮行之上续轮行照开");
    drop(running);
}

#[tokio::test]
async fn 续会话缺引擎句柄不再硬拒_prior_handle透传none照常续接() {
    // AC-6 主锚：`engine_session_id` 缺失不再触发「会话缺少引擎句柄」硬门槛
    // ——`prior_handle` 以 Option 原样透传（None），续会话照常发起
    let env = DualEnv::new("remote-missing");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "句柄缺失供应");
    seed_sdk_instance(stores, "句柄缺失实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-no-remote", None);
    seed_transcript(&store, "ses-no-remote");

    let composed = compose_turn(
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
    let running = composed
        .begin(
            SessionRef::Continue {
                id: "ses-no-remote".to_owned(),
            },
            "续一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("缺引擎句柄不再硬拒（AC-6）：prior_handle 透传 None 照常续接");

    // 留痕保留：engine_session_id 不被 begin 回写（透传不加工不校验）
    let session = store
        .find_session("ses-no-remote")
        .expect("find_session 应成功")
        .expect("续接沿用既有会话行");
    assert_eq!(
        session.engine_session_id, None,
        "prior_handle None 透传，留痕列不被回写"
    );
    let summaries = store.list_sessions(None, None).expect("清单应成功");
    assert_eq!(summaries.len(), 1, "不重复建会话行");
    assert_eq!(summaries[0].turns.len(), 1, "续轮行照开");
    drop(running);
}

// ---------------------------------------------------------------------------
// 装配产物组合（sink/query/门面/内核真实装配：begin_turn/drive 可直接驱动）
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn 装配产物真实驱动_sdk轮失败收敛且会话行轮行转录齐落库() {
    let env = DualEnv::new("drive-real");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "驱动供应");
    seed_sdk_instance(stores, "驱动实例", Some(provider_id));

    let composed = compose_turn(
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
    let session_id = running.session_id.clone();

    // 拒连端点一轮一命：重试护栏逐次留痕后耗尽失败收敛（paused 时钟快进）
    let outcome = running.drive(|_output| {}).await;

    assert_eq!(
        outcome.status,
        agent::AgentRunStatus::Failed,
        "sdk 轮拒连端点以 api_error 失败收敛"
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
    // query 真实装配：转录重放含 api_error 收敛事件
    let transcript = store.list_session_events(&session_id).expect("转录应成功");
    assert!(
        matches!(
            &transcript.last().expect("非空").kind,
            agent::AgentEventKind::TurnDone { subtype, is_error: true, .. } if subtype == "api_error"
        ),
        "转录末条为失败收敛事件，实际: {:?}",
        transcript.last()
    );
}

#[tokio::test]
async fn 装配产物含查询面_continue校验经真实库行成立() {
    let env = DualEnv::new("query-assemble");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "查询面供应");
    seed_sdk_instance(stores, "查询面实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-query", Some("sdk-77"));
    seed_transcript(&store, "ses-query");

    // ComposedTurn 产物即真实装配（query 直查 store 行 → 校验通过形态复用）
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
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
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
            .list_sessions(None, None,)
            .expect("清单应成功")
            .is_empty(),
        "解析失败零落库"
    );
}

// ---------------------------------------------------------------------------
// resolve Sdk 臂 context_length 透传（AC-9 组合根接线唯一触点）
// ---------------------------------------------------------------------------

#[test]
fn resolve_sdk臂透传provider的context_length() {
    let env = DualEnv::new("ctx-window");
    let stores = env.stores();
    let provider_id = seed_provider_with_context_length(stores, "窗长供应", Some(200_000));
    let instance_id = seed_sdk_instance(stores, "窗长实例", Some(provider_id));

    // 解析单点产物（compose_test 为 compose 子模块，可考私有解析产物）：
    // context_window == Some(200_000)，其余装配面不变
    let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
        Ok(resolved) => resolved,
        Err(error) => panic!("解析应成功: {error}"),
    };
    assert_eq!(
        resolved.context_window,
        Some(200_000),
        "provider context_length 原样旁路（不落缺省字面）"
    );
    assert!(
        matches!(resolved.kind, crate::EngineKind::Sdk),
        "kind 装配面不变"
    );
    assert_eq!(resolved.config.model_high, "m-high", "high 档模型装配不变");
    assert_eq!(resolved.config.model_low, "m-low", "low 档模型装配不变");
    assert_eq!(resolved.config.base_url, "http://127.0.0.1:9/v1");
    assert_eq!(resolved.config.api_key, "sk-compose");
}

#[test]
fn resolve_provider缺列时透传none不落128k字面() {
    let env = DualEnv::new("ctx-none");
    let stores = env.stores();
    // seed_provider 固定 context_length = None（含 V1 升级读入的旧记录形态）
    let provider_id = seed_provider(stores, "缺列供应");
    let instance_id = seed_sdk_instance(stores, "缺列实例", Some(provider_id));

    let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
        Ok(resolved) => resolved,
        Err(error) => panic!("解析应成功: {error}"),
    };
    assert_eq!(
        resolved.context_window, None,
        "缺列透传 None（缺省启发式归 ContextDefense::resolve，组合根不落 128_000 字面）"
    );
}

#[test]
fn resolve_cli臂逐字拒收退役文案() {
    let env = DualEnv::new("ctx-cli");
    let cli_id = preset_cli_instance(&env, 43, "cli窗长实例", false);
    let stores = env.stores();

    let error = err_of(super::resolve_agent_engine(stores, Some(cli_id)));

    assert_eq!(
        error, CLI_TENANT_RETIRED,
        "解析单点对 engine: Cli 实例显式拒收（不构造引擎、不静默回退），实际: {error}"
    );
}

// ---------------------------------------------------------------------------
// store 注入化拆参（design D8 / AC-6）：store 半边唯一入口 = 注入参
// ---------------------------------------------------------------------------

/// 注入实例消费：以 `for_root(rootA)` 预解析实例注入 → `composed.begin(...)`
/// 落会话行 / 轮行于**注入实例**（注入 store 的 list_sessions 可查）；异根
/// rootB 库零行——store 半边唯一入口是注入参（cwd / rootB 不触发第二处库解
/// 析；worktree 路径在类型上不可能进入库解析）。
#[tokio::test(start_paused = true)]
async fn 注入实例消费_会话行轮行落注入库且异根库零行() {
    let env = DualEnv::new("inject-isolation");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "注入消费供应");
    let sdk_id = seed_sdk_instance(stores, "注入消费实例", Some(provider_id));
    let root_b = tempfile::Builder::new()
        .prefix("compose-test-inject-other-root-")
        .tempdir()
        .expect("创建异根临时目录失败");
    let root_b_str = root_b.path().to_string_lossy().into_owned();

    // 注入实例 = for_root(rootA) 预解析（命令层装配同构）
    let injected = env.store();
    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        std::sync::Arc::clone(&injected),
        Some(sdk_id),
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");

    let running = composed
        .begin(
            SessionRef::New,
            "注入实例消费验证".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let outcome = running.drive(|_output| {}).await;

    assert_eq!(
        outcome.status,
        agent::AgentRunStatus::Failed,
        "sdk 拒连端点失败收敛（落库面不受收敛形态影响）"
    );

    // 注入实例：会话行 + 轮行在场（begin 即落库，逐字段可查）
    let summaries = injected
        .list_sessions(None, None)
        .expect("注入库清单应成功");
    assert_eq!(summaries.len(), 1, "会话行落注入实例");
    let row = &summaries[0].row;
    assert_eq!(
        row.provenance.source, "debug",
        "provenance source 逐字段一致"
    );
    assert_eq!(summaries[0].turns.len(), 1, "轮行落注入实例");
    assert_eq!(
        summaries[0].turns[0].status,
        agent::AgentRunStatus::Failed,
        "轮行状态逐字段一致"
    );

    // 异根 rootB 库零行（注入参是 store 半边唯一入口——cwd / 其他 root 不产
    // 生第二处落库；for_root(rootB) 实例化亦零行）
    let other = stores
        .for_root(&root_b_str)
        .expect("异根 for_root 应成功（实例化本身合法）");
    let other_summaries = other.list_sessions(None, None).expect("异根库清单应成功");
    assert!(
        other_summaries.is_empty(),
        "异根库零行（store 半边唯一入口 = 注入参）"
    );
}

// ---------------------------------------------------------------------------
// begin() 载荷断言装置：kernel 端口（sink）与 runner 端口均以记录型假件注入
// （trait 假件为显式注入依赖的测试侧替身）——prior_handle / config_snapshot
// 载荷与「跨引擎 Err 时 kernel 零调用」经假件调用面直读；store 半边仍为
// 真库实例（Continue 校验寻址）。
// ---------------------------------------------------------------------------

/// 记录型假 runner：捕获 `open_session` 的 `prior_handle` 与 `injections` 载荷
/// （注入面装配断言缝），返回最小 `AgentSession`（观测 / 提问通道即弃——本节
/// 用例只走 begin 同步段）。
struct RecordingRunner {
    prior_handles: Mutex<Vec<Option<String>>>,
    injections_captured: Mutex<Vec<agent::SessionInjections>>,
}

impl RecordingRunner {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            prior_handles: Mutex::new(Vec::new()),
            injections_captured: Mutex::new(Vec::new()),
        })
    }

    /// 已捕获的 prior_handle 序列（按调用序）。
    fn captured(&self) -> Vec<Option<String>> {
        self.prior_handles.lock().expect("捕获锁不可中毒").clone()
    }

    /// 已捕获的 injections 序列（按调用序）。
    fn captured_injections(&self) -> Vec<agent::SessionInjections> {
        self.injections_captured
            .lock()
            .expect("捕获锁不可中毒")
            .clone()
    }
}

impl AgentRunner for RecordingRunner {
    fn open_session(&self, open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        self.prior_handles
            .lock()
            .expect("捕获锁不可中毒")
            .push(open.prior_handle);
        self.injections_captured
            .lock()
            .expect("捕获锁不可中毒")
            .push(open.injections.clone());
        let (observation_tx, observation_rx) = mpsc::channel(4);
        let (question_tx, question_rx) = mpsc::channel(4);
        drop(observation_tx);
        drop(question_rx);
        Ok(AgentSession {
            observations: observation_rx,
            questions: question_tx,
            handle: RunHandle::default(),
        })
    }
}

/// 记录型假 sink：`SessionSink` 端口调用计数 + `create_session` 载荷捕获。
struct RecordingSink {
    created: Mutex<Vec<agent::NewSessionRow>>,
    begin_turns: AtomicUsize,
}

impl RecordingSink {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            created: Mutex::new(Vec::new()),
            begin_turns: AtomicUsize::new(0),
        })
    }

    /// 已捕获的建行载荷（按调用序）。
    fn created_rows(&self) -> Vec<agent::NewSessionRow> {
        self.created.lock().expect("捕获锁不可中毒").clone()
    }
}

impl agent::SessionSink for RecordingSink {
    fn create_session(&self, session: &agent::NewSessionRow) -> Result<(), String> {
        self.created
            .lock()
            .expect("捕获锁不可中毒")
            .push(session.clone());
        Ok(())
    }

    fn begin_turn(&self, _session_id: &str, _started_at: i64) -> Result<i64, String> {
        Ok(self.begin_turns.fetch_add(1, Ordering::Relaxed) as i64 + 1)
    }

    fn next_seq(&self, _session_id: &str) -> Result<u64, String> {
        Ok(0)
    }

    fn append_sealed(&self, _session_id: &str, _event: &agent::AgentEvent) -> Result<(), String> {
        Ok(())
    }

    fn finish_turn(&self, _turn_id: i64, _outcome: &agent::TurnOutcome) -> Result<(), String> {
        Ok(())
    }

    fn bind_remote_session(
        &self,
        _session_id: &str,
        _remote: &str,
        _updated_at: i64,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// 以假件装配 `ComposedTurn`（compose 子模块可直触私有字段——测试侧装配缝；
/// store 半边仍为真库实例）。
fn composed_with_fakes(
    store: std::sync::Arc<store::Store>,
    runner: Arc<RecordingRunner>,
    sink: Arc<RecordingSink>,
    models: Option<(String, String)>,
    agent_instance_id: i64,
) -> ComposedTurn {
    ComposedTurn {
        kernel: SessionKernel::new(
            sink,
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::new(agent::AskRegistry::default()),
        ),
        runner,
        query: Arc::new(StoreQuery::new(store)),
        store_kind: AgentEngineKind::Sdk,
        models,
        agent_instance_id,
    }
}

#[tokio::test]
async fn prior_handle透传四形态_句柄原样与none缺省与new恒none() {
    let env = DualEnv::new("prior-handle");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let long_handle = format!("sdk-{}", "句".repeat(1100));
    seed_sdk_session(&store, "ses-handle-empty", Some(""));
    seed_sdk_session(&store, "ses-handle-long", Some(&long_handle));
    seed_sdk_session(&store, "ses-handle-weird", Some("句柄\n零\u{0}字符😀"));
    seed_sdk_session(&store, "ses-handle-none", None);

    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        71,
    );

    // 续会话句柄四形态：Some("") / 超长 / 特殊字符原样透传（Option 透传不
    // 校验不加工），None 缺省透传（AC-6 硬门槛退役的载荷面）
    for id in [
        "ses-handle-empty",
        "ses-handle-long",
        "ses-handle-weird",
        "ses-handle-none",
    ] {
        composed
            .begin(
                SessionRef::Continue { id: id.to_owned() },
                "续一轮".to_owned(),
                ctx(&env.root()),
                debug_provenance(),
            )
            .unwrap_or_else(|e| panic!("续会话 {id} 应成功: {e}"));
    }
    // New：prior_handle 恒 None
    composed
        .begin(
            SessionRef::New,
            "新会话一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("New 应成功");

    assert_eq!(
        runner.captured(),
        vec![
            Some(String::new()),
            Some(long_handle),
            Some("句柄\n零\u{0}字符😀".to_owned()),
            None,
            None,
        ],
        "prior_handle 四形态原样透传（Some 三边界不加工 / None 缺省 / New 恒 None）"
    );
    assert_eq!(
        sink.begin_turns.load(Ordering::Relaxed),
        5,
        "五次 begin 均开轮（透传不拦截）"
    );
}

#[tokio::test]
#[allow(non_snake_case)]
async fn 快照组装携agentInstanceId与定型双面_json线键camel_case() {
    let env = DualEnv::new("snapshot-key");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        i64::MAX,
    );

    composed
        .begin(
            SessionRef::New,
            "高档一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    composed
        .begin(
            SessionRef::New,
            "低档一轮".to_owned(),
            SessionCtx {
                model_level: ModelLevel::Low,
                ..ctx(&env.root())
            },
            debug_provenance(),
        )
        .expect("begin 应成功");

    let created = sink.created_rows();
    assert_eq!(created.len(), 2, "恰两轮 create_session");
    let high = &created[0].config_snapshot;
    assert_eq!(
        high["engine"],
        serde_json::json!("sdk"),
        "快照 engine 定型双面保留（DD9）"
    );
    assert_eq!(
        high["agentInstanceId"],
        serde_json::json!(i64::MAX),
        "实例外键恒在（DD8 / AC-7）——i64::MAX 经 serde 数值不失真"
    );
    assert_eq!(high["model"], serde_json::json!("m-high"), "High 档取 high");
    assert_eq!(
        high["permissionMode"],
        serde_json::json!("bypassPermissions"),
        "permissionMode 零变化"
    );
    let low = &created[1].config_snapshot;
    assert_eq!(low["model"], serde_json::json!("m-low"), "Low 档取 low");
    assert_eq!(
        low["agentInstanceId"],
        serde_json::json!(i64::MAX),
        "档位定型不影响实例外键"
    );
}

#[tokio::test]
async fn models装配为none时快照model为null且实例外键int边界serde不失真() {
    // DD9 死臂载荷：models: None（Cli 死臂映射形态）经假件装配直驱——快照
    // model: null 照常、agentInstanceId 恒 Some（不因无模型装配而漏键）；实例
    // id int 边界（0 / -1 / i64::MAX）经 serde 数值不失真（DD8 / AC-7 边界行）
    let env = DualEnv::new("models-none");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();

    for instance_id in [0_i64, -1, i64::MAX] {
        let composed = composed_with_fakes(
            std::sync::Arc::clone(&store),
            Arc::clone(&runner),
            Arc::clone(&sink),
            None,
            instance_id,
        );
        composed
            .begin(
                SessionRef::New,
                "无模型装配一轮".to_owned(),
                ctx(&env.root()),
                debug_provenance(),
            )
            .expect("begin 应成功");
    }

    let created = sink.created_rows();
    assert_eq!(created.len(), 3, "三边界实例 id 各恰一轮 create_session");
    for (index, instance_id) in [0_i64, -1, i64::MAX].into_iter().enumerate() {
        let snapshot = &created[index].config_snapshot;
        assert_eq!(
            snapshot["model"],
            serde_json::Value::Null,
            "model: null 照常出线"
        );
        assert_eq!(
            snapshot["agentInstanceId"],
            serde_json::json!(instance_id),
            "agentInstanceId 恒 Some 且 {instance_id} 经 serde 数值不失真"
        );
    }
}

#[tokio::test]
async fn 跨引擎续会话前置拒_零sink调用零runner开面() {
    // AC-4「存量 cli 会话续接显式失败」+ DD10 门序：跨引擎校验先于 prior_handle
    // 透传与 kernel.begin_turn——前置 Err 不落库不推流（kernel 假件零调用计数）
    let env = DualEnv::new("cross-engine-fake");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    store
        .create_session(&SessionRecord {
            id: "ses-cli-made-fake".to_owned(),
            engine_session_id: Some("s-cli-1".to_owned()),
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Cli,
                agent_instance_id: None,
                model: None,
                permission_mode: AgentPermissionMode::BypassPermissions,
            },
            source: "debug".to_owned(),
            source_ref: None,
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("落存量 cli 会话行");
    seed_transcript(&store, "ses-cli-made-fake");

    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        74,
    );

    let error = err_of(composed.begin(
        SessionRef::Continue {
            id: "ses-cli-made-fake".to_owned(),
        },
        "跨引擎续一轮".to_owned(),
        ctx(&env.root()),
        debug_provenance(),
    ));
    assert!(
        error.contains("非当前引擎产出"),
        "存量 cli 会话续接显式失败（AC-4 落点），实际: {error}"
    );
    assert_eq!(
        runner.captured(),
        Vec::<Option<String>>::new(),
        "门序保持：跨引擎校验先于 prior_handle 透传（runner 零开面）"
    );
    assert_eq!(
        sink.begin_turns.load(Ordering::Relaxed),
        0,
        "kernel 零调用：不开轮行"
    );
    assert!(sink.created_rows().is_empty(), "kernel 零调用：不建会话行");

    // AC-4「会话列表 / 转录照常可读」（R5 呈现兜底）：拒收零落库不推流之外，
    // 存量 cli 会话的读面零改动——session_query 侧单查与转录重放照常可读
    let detail = composed
        .query
        .find_session_detail("ses-cli-made-fake")
        .expect("存量 cli 会话单查照常可读");
    assert_eq!(
        detail.row.id, "ses-cli-made-fake",
        "存量 cli 会话行读面零改动"
    );
    let transcript = composed
        .query
        .transcript("ses-cli-made-fake")
        .expect("存量 cli 会话转录重放照常可读");
    assert_eq!(
        transcript.len(),
        2,
        "密封转录原样可读（呈现兜底不受拒收影响）"
    );
}

#[tokio::test]
async fn question为str边界三形态透传不加工不panic() {
    let env = DualEnv::new("question-edge");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        75,
    );

    for question in [
        String::new(),
        format!("超长一轮{}", "长".repeat(1100)),
        "多行\n提问\u{0}含零宽😀".to_owned(),
    ] {
        composed
            .begin(
                SessionRef::New,
                question,
                ctx(&env.root()),
                debug_provenance(),
            )
            .expect("question str 边界透传不加工（begin 成立即受理）");
    }
    assert_eq!(
        sink.begin_turns.load(Ordering::Relaxed),
        3,
        "三形态均开轮（透传不拦截不 panic）"
    );
}

#[test]
fn 显式agent_id边界零与负数与i64max均miss记因() {
    let env = DualEnv::new("agent-edge");
    let stores = env.stores();
    for id in [0_i64, -1, i64::MAX] {
        let error = err_of(compose_turn(
            stores,
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::new(agent::AskRegistry::default()),
            env.store(),
            Some(id),
            Path::new(&env.root()),
            None,
        ));
        assert!(
            error.contains("agent 不存在") && error.contains(&id.to_string()),
            "miss 走既有「agent 不存在」分支携 id 记因，实际: {error}"
        );
    }
}

#[test]
fn 续会话id边界空串与超长与特殊字符均走会话不存在分支不panic() {
    let env = DualEnv::new("session-id-edge");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "边界供应");
    seed_sdk_instance(stores, "边界实例", Some(provider_id));
    let composed = compose_turn(
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
    for id in [
        "".to_owned(),
        format!("ses-{}", "超".repeat(1100)),
        "特殊\u{0}字符😀\n会话".to_owned(),
    ] {
        let error = err_of(composed.begin(
            SessionRef::Continue { id: id.clone() },
            "边界续一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        ));
        assert!(
            error.contains("会话不存在"),
            "miss 走「会话不存在」分支不 panic，实际: {error}"
        );
    }
}

#[tokio::test]
async fn 新会话快照携实例id_缺省与显式路径同填some() {
    let env = DualEnv::new("snapshot-instance");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "快照供应");
    let sdk_id = seed_sdk_instance(stores, "快照实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    // 显式路径：快照携实例 id（DD8）
    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        Some(sdk_id),
        Path::new(&env.root()),
        None,
    )
    .expect("显式解析应成功");
    let running = composed
        .begin(
            SessionRef::New,
            "显式路径一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session = store
        .find_session(&running.session_id)
        .expect("find_session 应成功")
        .expect("New 建会话行");
    assert_eq!(
        session.config_snapshot.agent_instance_id,
        Some(sdk_id),
        "显式路径快照携实例 id"
    );
    let json = serde_json::to_value(&session.config_snapshot).expect("快照序列化应成功");
    assert_eq!(
        json["agentInstanceId"],
        serde_json::json!(sdk_id),
        "serde 线键 agentInstanceId camelCase 出线"
    );
    drop(running);

    // 缺省路径：同经 resolve 取实例、同填 Some
    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("缺省解析应成功");
    let running = composed
        .begin(
            SessionRef::New,
            "缺省路径一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session = store
        .find_session(&running.session_id)
        .expect("find_session 应成功")
        .expect("New 建会话行");
    assert_eq!(
        session.config_snapshot.agent_instance_id,
        Some(sdk_id),
        "缺省路径同填 Some（同经 resolve 单点取实例）"
    );
    drop(running);
}

// ---------------------------------------------------------------------------
// auto-named-explore：begin 按 provenance.source 装配注入（explore 六工具 +
// Explore 枚举，其余 default）
// ---------------------------------------------------------------------------

use crate::sdk::mount::{EXPLORE_NOTE_TOOL, TOOL_NAMES};

/// explore 来源装配（正向）：`begin(source = "explore")` → 捕获的
/// `TurnRequest.injections.tools` = 六名（无 `write`、序同 `TOOL_NAMES`）+
/// 名字清单含 `create_explore_note`。
#[tokio::test]
async fn explore来源begin装配六工具收窄加explore枚举() {
    let env = DualEnv::new("explore-assemble");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        81,
    );

    composed
        .begin(
            SessionRef::New,
            "explore 装配一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: "explore".to_owned(),
                source_ref: None,
            },
        )
        .expect("explore 来源 begin 应成功");

    let captured = runner.captured_injections();
    assert_eq!(captured.len(), 1, "恰一轮捕获");
    let injections = &captured[0];
    assert_eq!(
        injections.tools.as_ref().expect("tools 在场").as_slice(),
        &[
            "read".to_owned(),
            "grep".to_owned(),
            "glob".to_owned(),
            "ls".to_owned(),
            "edit".to_owned(),
            "bash".to_owned(),
            EXPLORE_NOTE_TOOL.to_owned(),
        ],
        "六名收窄子集 + 专用名（无 write，序同 TOOL_NAMES 保序、专用名尾追加）"
    );
    assert!(
        !injections
            .tools
            .as_ref()
            .expect("tools 在场")
            .iter()
            .any(|name| name == "write"),
        "explore 装配面不含 write（AC-5）"
    );
    assert!(
        injections
            .tools
            .as_ref()
            .expect("tools 在场")
            .iter()
            .any(|name| name == EXPLORE_NOTE_TOOL),
        "名字清单含 create_explore_note（专用工具以名字进清单）"
    );
    assert_eq!(injections.finish, None, "explore 臂无收口档位");
    assert_eq!(injections.preamble, None, "preamble 维持不消费");
}

/// 六名子集校验等价断言：explore 装配产物逐名 ∈ `sdk::tools::TOOL_NAMES`
/// （装配层校验的可达面收窄断言——非法名 `Err` 臂生产不可达的等价收窄）。
#[tokio::test]
async fn explore装配产物逐名都在七通用封闭清单内() {
    let env = DualEnv::new("explore-subset");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        82,
    );

    composed
        .begin(
            SessionRef::New,
            "子集校验一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: "explore".to_owned(),
                source_ref: None,
            },
        )
        .expect("begin 应成功");

    let captured = runner.captured_injections();
    let tools = captured[0].tools.as_ref().expect("tools 在场");
    assert_eq!(tools.len(), 7, "六名收窄子集 + 一个专用名");
    for name in tools {
        assert!(
            TOOL_NAMES.contains(&name.as_str()) || name == EXPLORE_NOTE_TOOL,
            "装配名 {name} ∈ 挂载清单（内建七件 ∪ 专用名）"
        );
    }
}

/// 非 explore 来源零变化（AC-7，参数化）：debug / change 系字面量 / 空串 /
/// 未知来源 → injections 恒 default（`None` / `None`）。
#[tokio::test]
async fn 非explore来源begin恒default注入() {
    let env = DualEnv::new("default-sources");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        83,
    );

    for source in [
        "debug",
        "change-flow",
        "change",
        "apply-change",
        "",
        "unknown-source",
    ] {
        composed
            .begin(
                SessionRef::New,
                format!("default 来源一轮: {source}"),
                ctx(&env.root()),
                SessionProvenance {
                    source: source.to_owned(),
                    source_ref: None,
                },
            )
            .unwrap_or_else(|e| panic!("来源 {source:?} begin 应成功: {e}"));
        let captured = runner.captured_injections();
        let last = captured.last().expect("本轮捕获");
        // `change` 来源按 role 产名字清单（缺省 role 不影响七内建底座）；
        // 其余来源三字段全 None（ask 位恒显式 Generic）
        let expected_tools = if source == "change" {
            Some(seven())
        } else {
            None
        };
        assert_eq!(last.tools, expected_tools, "来源 {source:?} tools 面");
        assert_eq!(last.finish, None, "来源 {source:?} finish = None");
        assert_eq!(last.preamble, None);
        // ask 位逐臂显式置位：change 恒 Flow，其余恒 Generic（任何来源都不落 None）
        let expected_ask = if source == "change" {
            agent::AskMode::Flow
        } else {
            agent::AskMode::Generic
        };
        assert_eq!(
            last.ask,
            Some(expected_ask),
            "来源 {source:?} 的 ask 位（D13：首期无禁用场景）"
        );
    }
    assert_eq!(runner.captured_injections().len(), 6, "六来源各捕获一次");
}

/// 三处字面量同源（compose 与 store 跨 crate 双源断言）：以 store 侧探索
/// 记录会话行的 `source` 字段（`EXPLORE_RUN_SOURCE` 私有字面量的可读投影）
/// 发起 → 命中 explore 装配臂。
#[tokio::test]
async fn store侧explore来源字面量发起命中装配臂() {
    let env = DualEnv::new("dual-source");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    // store 侧铸造：占位建档预建的会话行 source 即 EXPLORE_RUN_SOURCE 投影
    let record = store
        .create_explore_record(&env.root(), None)
        .expect("占位建档应成功");
    let session_id = record.session_id.clone().expect("建档回写会话指针");
    let explore_source = store
        .find_session(&session_id)
        .expect("find_session 应成功")
        .expect("会话行在库")
        .source;
    assert_eq!(
        explore_source, "explore",
        "store 侧同源字面量（双源互链锚点）"
    );

    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        84,
    );

    // 以 store 投影字面量发起 → 命中 explore 装配臂（六名 + Explore）
    composed
        .begin(
            SessionRef::Continue { id: session_id },
            "同源字面量一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: explore_source,
                source_ref: None,
            },
        )
        .expect("同源字面量 begin 应成功");

    let captured = runner.captured_injections();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].tools.as_ref().expect("tools 在场").len(),
        7,
        "命中 explore 装配臂（六名收窄 + 专用名）"
    );
    assert!(
        captured[0]
            .tools
            .as_ref()
            .expect("tools 在场")
            .iter()
            .any(|name| name == EXPLORE_NOTE_TOOL),
        "命中 explore 装配臂（名字清单含 create_explore_note）"
    );
}

/// 装配产物真实驱动 explore（组合：compose → pump → loop 下发面，AC-5 端到
/// 端）：compose_turn 带 store 句柄 + explore 来源 + Continue 预建空会话 →
/// 转录首事件 RunStarted.tools 报六工具 + `create_explore_note`。
#[tokio::test(start_paused = true)]
async fn 装配产物真实驱动_explore来源runstarted报六工具加专用工具() {
    let env = DualEnv::new("drive-explore");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "驱动供应explore");
    seed_sdk_instance(stores, "驱动实例explore", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    // 预建空会话（占位建档同事务回写）→ Continue 首轮
    let record = store
        .create_explore_record(&env.root(), None)
        .expect("占位建档应成功");
    let session_id = record.session_id.clone().expect("建档回写会话指针");

    let composed = compose_turn(
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
    let running = composed
        .begin(
            SessionRef::Continue {
                id: session_id.clone(),
            },
            "explore 首轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: "explore".to_owned(),
                source_ref: None,
            },
        )
        .expect("begin 应成功");
    drop(running.drive(|_output| {}).await);

    // 转录首事件 RunStarted.tools = 六工具保序 + 专用工具 + ask 工具（下发面单源；
    // ask 为按位追加的基线工具，恒在名单末位）
    let transcript = store.list_session_events(&session_id).expect("转录应成功");
    let agent::AgentEventKind::RunStarted { tools, .. } = &transcript.first().expect("非空").kind
    else {
        panic!("转录首事件应为 RunStarted，实际: {:?}", transcript.first());
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
            "AskUserQuestion".to_owned(),
        ],
        "注入经泵铸造与下发链端到端（AC-5 + ask 位 Generic 追加）"
    );
    assert!(!tools.contains(&"write".to_owned()), "explore 面不含 write");
}

/// 装配产物真实驱动 default（组合端到端）：debug 来源 → RunStarted.tools 报
/// 七名 + ask 工具（AC-7 端到端；ask 位恒 Generic 显式置位）。
#[tokio::test(start_paused = true)]
async fn 装配产物真实驱动_debug来源runstarted报七名() {
    let env = DualEnv::new("drive-debug");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "驱动供应debug");
    seed_sdk_instance(stores, "驱动实例debug", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    let composed = compose_turn(
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
    let running = composed
        .begin(
            SessionRef::New,
            "debug 驱动一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    drop(running.drive(|_output| {}).await);

    let transcript = store.list_session_events(&session_id).expect("转录应成功");
    let agent::AgentEventKind::RunStarted { tools, .. } = &transcript.first().expect("非空").kind
    else {
        panic!("转录首事件应为 RunStarted，实际: {:?}", transcript.first());
    };
    let mut expected: Vec<String> = TOOL_NAMES.iter().map(|name| (*name).to_owned()).collect();
    expected.push("AskUserQuestion".to_owned());
    assert_eq!(
        tools, &expected,
        "debug 轮七名 + ask 工具（AC-7 端到端；ask 位恒显式置位）"
    );
}

/// 注入面不破坏失败语义：explore 装配 + discard 端点（模型 IO 恒拒连）→
/// `api_error` 失败收敛形态与既有一致（回归）。
#[tokio::test(start_paused = true)]
async fn explore装配不破坏失败收敛语义() {
    let env = DualEnv::new("explore-failure");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "驱动供应失败面");
    seed_sdk_instance(stores, "驱动实例失败面", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    let record = store
        .create_explore_record(&env.root(), None)
        .expect("占位建档应成功");
    let session_id = record.session_id.clone().expect("建档回写会话指针");

    let composed = compose_turn(
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
    let running = composed
        .begin(
            SessionRef::Continue {
                id: session_id.clone(),
            },
            "explore 失败面一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: "explore".to_owned(),
                source_ref: None,
            },
        )
        .expect("begin 应成功");
    let outcome = running.drive(|_output| {}).await;

    // 失败收敛形态与既有 debug 轮一致：Failed + 转录末条 TurnDone api_error
    assert_eq!(
        outcome.status,
        agent::AgentRunStatus::Failed,
        "explore 装配轮拒连端点同以 api_error 失败收敛"
    );
    let transcript = store.list_session_events(&session_id).expect("转录应成功");
    assert!(
        matches!(
            &transcript.last().expect("非空").kind,
            agent::AgentEventKind::TurnDone { subtype, is_error: true, .. } if subtype == "api_error"
        ),
        "失败收敛形态零漂移，实际: {:?}",
        transcript.last()
    );
}
// ---------------------------------------------------------------------------
// 会话实时流注册表注入缝（compose_turn 第三参）：exec 轨 / change 轨同源进入
// 同一注册表；两表互不串台；解析失败零条目
// ---------------------------------------------------------------------------

use orchestration::port::{RunEventSink, WorkerAgentPort, WorkerRole, WorkerTurnRequest};
use orchestration::state::RunUpdate;

/// 订阅句柄排空：事件序列与终态（断流即用例失败）。
async fn drain_stream(
    mut subscription: agent::SessionStreamSubscription,
) -> (Vec<agent::AgentEvent>, agent::TurnOutcome) {
    let mut events = Vec::new();
    loop {
        match subscription.recv().await {
            Ok(agent::SessionStreamItem::Event(event)) => events.push(event),
            Ok(agent::SessionStreamItem::Finished(outcome)) => return (events, outcome),
            Err(error) => panic!("不应断流（事件与终态齐备）：{error:?}"),
        }
    }
}

/// change 轨观察 sink：首个会话事件即订阅（会话 id 运行期才可知；订阅窗口
/// 落在 `begin` 之后、驱动完成之前），兼记 `SessionEvent` 出口条数。
struct WatchSink {
    streams: Arc<SessionStreamRegistry>,
    subscription: Mutex<Option<agent::SessionStreamSubscription>>,
    updates: Mutex<Vec<RunUpdate>>,
}

impl RunEventSink for WatchSink {
    fn emit(&self, update: RunUpdate) {
        if let RunUpdate::SessionEvent { session_id, .. } = &update {
            let mut slot = self.subscription.lock().expect("订阅槽锁不可中毒");
            if slot.is_none() {
                *slot = self.streams.subscribe(session_id, 0);
            }
        }
        self.updates
            .lock()
            .expect("事件捕获锁不可中毒")
            .push(update);
    }
}

/// 注入面 + 观察面同实例装配（`KernelWorkerPort` 持注入面，测试持观察面）。
struct WatchProbe {
    sink: Arc<WatchSink>,
}

impl WatchProbe {
    fn new(streams: Arc<SessionStreamRegistry>) -> Self {
        Self {
            sink: Arc::new(WatchSink {
                streams,
                subscription: Mutex::new(None),
                updates: Mutex::new(Vec::new()),
            }),
        }
    }

    fn sink(&self) -> Arc<dyn RunEventSink> {
        Arc::clone(&self.sink) as Arc<dyn RunEventSink>
    }

    fn take_subscription(&self) -> Option<agent::SessionStreamSubscription> {
        self.sink
            .subscription
            .lock()
            .expect("订阅槽锁不可中毒")
            .take()
    }

    fn session_event_count(&self) -> usize {
        self.sink
            .updates
            .lock()
            .expect("事件捕获锁不可中毒")
            .iter()
            .filter(|update| matches!(update, RunUpdate::SessionEvent { .. }))
            .count()
    }

    /// 只读快照：`RunUpdate::SessionEvent` 出口捕获的事件（按投递序）。
    fn session_events(&self) -> Vec<agent::AgentEvent> {
        self.sink
            .updates
            .lock()
            .expect("事件捕获锁不可中毒")
            .iter()
            .filter_map(|update| match update {
                RunUpdate::SessionEvent { event, .. } => Some(event.clone()),
                _ => None,
            })
            .collect()
    }
}

/// change 轨轮请求 fixture（sourceRef 按 `<change>/<phase>/<role>/<attempt>`）。
fn change_turn_request(root: &str) -> WorkerTurnRequest {
    WorkerTurnRequest {
        root: root.to_owned(),
        prompt: "change 轨一轮".to_owned(),
        provenance: SessionProvenance {
            source: "change".to_owned(),
            source_ref: Some("chg-flow/implement/executor/1".to_owned()),
        },
        permission: AgentPermissionMode::BypassPermissions,
        model_level: ModelLevel::High,
        continue_session: None,
        agent: None,
        role: WorkerRole::Executor,
    }
}

/// 注入缝生效：`compose_turn(..., streams)` 产物的 `begin` 使该会话 id 在传入
/// 的 `streams` 中可订阅（`subscribe` → `Some`）。
#[tokio::test]
async fn compose_turn注入缝生效_begin后会话在传入streams中可订阅() {
    let env = DualEnv::new("streams-inject");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "注入缝供应");
    seed_sdk_instance(stores, "注入缝实例", Some(provider_id));
    let streams = Arc::new(SessionStreamRegistry::new());

    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let running = composed
        .begin(
            SessionRef::New,
            "注入缝一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");

    assert!(
        streams.subscribe(&running.session_id, 0).is_some(),
        "传入的注册表命中该会话（注入缝生效，非内部临时表）"
    );
}

/// exec 轨真实驱动（拒连端点即时失败收敛）→ 注册表订阅端收该轮 `Finished`，
/// 其 `started_at` 与 running 轮行一致；驱动结束条目除名。
#[tokio::test(start_paused = true)]
async fn exec轨真实驱动_注册表订阅端收failed终态且started_at与轮行一致() {
    let env = DualEnv::new("streams-exec-drive");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "exec 轨供应");
    seed_sdk_instance(stores, "exec 轨实例", Some(provider_id));
    let streams = Arc::new(SessionStreamRegistry::new());

    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let running = composed
        .begin(
            SessionRef::New,
            "exec 轨一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    let started_at = running.started_at;
    let subscription = streams.subscribe(&session_id, 0).expect("订阅方可即刻接入");

    let outcome = running.drive(|_output| {}).await;
    let (_events, terminal) = drain_stream(subscription).await;

    assert_eq!(
        terminal.status,
        agent::AgentRunStatus::Failed,
        "拒连端点以 api_error 失败收敛"
    );
    assert_eq!(terminal, outcome, "注册表终态与驱动返回同构");
    assert_eq!(
        terminal.started_at, started_at,
        "终态 started_at 与 running 轮行一致（收口形状自足）"
    );
    assert!(
        streams.subscribe(&session_id, 0).is_none(),
        "驱动结束条目除名"
    );

    // 库面同源：轮行 started_at 与终态一致
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let summaries = store.list_sessions(None, None).expect("清单应成功");
    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].turns[0].started_at, started_at,
        "库面轮行与注册表终态同一起点"
    );
}

/// change 轨形态（`KernelWorkerPort::run` 零改动）：同一 `streams` 上该会话条目
/// 可被订阅并收 `Finished`；`RunUpdate::SessionEvent` 出口照常（两出口同源），
/// 且两出口的事件逐条轮归属相等（跨层同值，见 design D3 的同一枚 `event.clone()`）。
#[tokio::test(start_paused = true)]
async fn change轨真实驱动_同一streams收终态且run_update出口照常() {
    let env = DualEnv::new("streams-change-drive");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "change 轨供应");
    seed_sdk_instance(stores, "change 轨实例", Some(provider_id));
    let streams = Arc::new(SessionStreamRegistry::new());

    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let probe = WatchProbe::new(Arc::clone(&streams));
    let port = crate::KernelWorkerPort::new(composed, probe.sink());

    let root = env.root();
    let outcome = port
        .run(change_turn_request(&root))
        .await
        .expect("change 轨一轮应发起");

    let subscription = probe
        .take_subscription()
        .expect("首个会话事件时已完成订阅（begin 先于驱动）");
    let (events, terminal) = drain_stream(subscription).await;

    assert_eq!(
        terminal.status,
        agent::AgentRunStatus::Failed,
        "拒连端点失败收敛"
    );
    assert_eq!(terminal.status, outcome.status, "两出口状态同源");
    assert!(terminal.turn_id > 0, "终态携轮 id");
    assert!(
        probe.session_event_count() > 0,
        "RunUpdate::SessionEvent 出口照常（两出口同源）"
    );

    // 跨层轮归属：RunUpdate 出口与订阅流投递同一枚事件（内核盖戳单点），
    // 故两出口的事件逐条 `turn_id` 相等，且全部等于终态轮 id。
    let emitted_all = probe.session_events();
    let emitted: Vec<Option<i64>> = emitted_all.iter().map(|event| event.turn_id).collect();
    let streamed: Vec<Option<i64>> = events.iter().map(|event| event.turn_id).collect();

    assert!(
        emitted
            .iter()
            .all(|turn_id| *turn_id == Some(terminal.turn_id)),
        "RunUpdate 出口的事件全部携终态轮 id，实际: {emitted:?}"
    );
    assert!(
        !streamed.is_empty(),
        "订阅流在首枚 SessionEvent 之后仍收到实时事件（fail-fast 前已有事件流出）"
    );
    assert_eq!(
        emitted.len(),
        streamed.len() + 1,
        "订阅在首枚 SessionEvent 广播后才建立，恰漏首枚（订阅端从次枚起同源），条数: RunUpdate={} 订阅流={}",
        emitted.len(),
        streamed.len()
    );
    assert_eq!(
        events.as_slice(),
        &emitted_all[emitted_all.len() - events.len()..],
        "两实时出口投递同一枚事件（逐字段相等，含 turn_id；跨层同源同值）"
    );
    assert!(
        streamed
            .iter()
            .all(|turn_id| *turn_id == Some(terminal.turn_id)),
        "订阅流的事件全部携终态轮 id，实际: {streamed:?}"
    );
    assert!(
        streams.subscribe(&outcome.session_id, 0).is_none(),
        "驱动结束条目除名"
    );
}

/// 增量（`MessageDelta`）不进 run 变更通知：token 级事件只走会话订阅面。
/// 通知是失效信号——前端收一条即重查整个统一视图（含全部产物），逐 token
/// 广播会把重查放大到事件粒度；转录累积面同口径只收密封事件。
#[tokio::test]
async fn change轨增量不进run通知_仅密封事件透传() {
    use agent::AgentDelta;

    let env = DualEnv::new("delta-filtered");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let composed = ComposedTurn {
        kernel: SessionKernel::new(
            RecordingSink::new(),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::new(agent::AskRegistry::default()),
        ),
        runner: Arc::new(ScriptedRunner::new(vec![
            agent::AgentEventKind::MessageDelta {
                parent_tool_use_id: None,
                delta: AgentDelta::Text {
                    text: "逐 token".to_owned(),
                },
            },
            agent::AgentEventKind::SystemNotice {
                subtype: "probe".to_owned(),
                payload: serde_json::json!({}),
            },
            agent::AgentEventKind::TurnDone {
                subtype: "success".to_owned(),
                is_error: false,
                num_turns: Some(1),
                duration_ms: Some(1),
                cost_usd: None,
                usage: serde_json::json!({}),
                session_id: None,
            },
        ])),
        query: Arc::new(StoreQuery::new(store)),
        store_kind: AgentEngineKind::Sdk,
        models: Some(("m-high".to_owned(), "m-low".to_owned())),
        agent_instance_id: 71,
    };
    let watch = WatchProbe::new(Arc::new(SessionStreamRegistry::new()));
    let port = crate::KernelWorkerPort::new(composed, watch.sink());

    let outcome = port
        .run(change_turn_request(&env.root()))
        .await
        .expect("一轮应发起");

    let emitted = watch.session_events();
    assert_eq!(
        emitted.iter().filter(|event| event.kind.is_delta()).count(),
        0,
        "增量零透传（密封以外不出 run 通知）"
    );
    assert_eq!(
        emitted.len(),
        2,
        "SystemNotice + TurnDone 两枚密封事件照常透传"
    );
    assert_eq!(outcome.transcript.len(), 2, "转录累积面同口径只收密封事件");
}

/// 脚本化假 runner：按给定序列投递观察事件后关闭通道（内核盖戳 / 落库 /
/// 收口全走真实链路；测试只替换引擎侧事件来源）。
struct ScriptedRunner {
    script: Mutex<Vec<agent::AgentEventKind>>,
}

impl ScriptedRunner {
    fn new(script: Vec<agent::AgentEventKind>) -> Self {
        Self {
            script: Mutex::new(script),
        }
    }
}

impl AgentRunner for ScriptedRunner {
    fn open_session(&self, _open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        let mut script = std::mem::take(&mut *self.script.lock().expect("脚本锁不可中毒"));
        let (observation_tx, observation_rx) = mpsc::channel(script.len().max(1) + 4);
        let (question_tx, mut question_rx) = mpsc::channel(4);
        // 轮提问送达后按序投递脚本事件并关闭观察通道（驱动循环据此收口）
        tokio::spawn(async move {
            let _ = question_rx.recv().await;
            for kind in script.drain(..) {
                if observation_tx.send(kind).await.is_err() {
                    break;
                }
            }
            drop(observation_tx);
        });
        Ok(AgentSession {
            observations: observation_rx,
            questions: question_tx,
            handle: RunHandle::default(),
        })
    }
}

/// 解析失败（无默认 agent / 显式不存在 id）→ `Err` 且注册表零条目。
#[test]
fn 解析失败不构造内核且零注册表条目() {
    let env = DualEnv::new("streams-resolve-err");
    let stores = env.stores();
    let streams = Arc::new(SessionStreamRegistry::new());

    let error = err_of(compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    ));
    assert!(
        error.contains("默认 agent"),
        "缺省解析 Err 文案既有逐字，实际: {error}"
    );

    let error = err_of(compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        Some(404),
        Path::new(&env.root()),
        None,
    ));
    assert!(error.contains("404"), "显式不存在 id Err，实际: {error}");

    for session_id in ["ses-1", "", "ses-any"] {
        assert!(
            streams.subscribe(session_id, 0).is_none(),
            "解析失败不产生注册表条目（{session_id:?}）"
        );
    }
}

/// 两表互不串台：`begin` 后 stop 命中不影响 streams 条目；驱动收口（两表均
/// 除名）后 `request_stop` miss 幂等 `false` 且 `subscribe` 为 `None`。
#[tokio::test(start_paused = true)]
async fn stop与streams两表互不串台_收口后两表同时除名() {
    let env = DualEnv::new("streams-vs-stop");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "两表供应");
    seed_sdk_instance(stores, "两表实例", Some(provider_id));
    let registry = Arc::new(StopRegistry::new());
    let streams = Arc::new(SessionStreamRegistry::new());

    let composed = compose_turn(
        stores,
        Arc::clone(&registry),
        Arc::clone(&streams),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("解析应成功");
    let running = composed
        .begin(
            SessionRef::New,
            "两表一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();

    assert!(registry.request_stop(&session_id), "stop 命中");
    assert!(
        streams.subscribe(&session_id, 0).is_some(),
        "stop 命中不影响 streams 条目（两表互不替代）"
    );

    let outcome = running.drive(|_output| {}).await;
    assert_eq!(outcome.status, agent::AgentRunStatus::Stopped, "停止收敛");
    assert!(
        streams.subscribe(&session_id, 0).is_none(),
        "收口后 streams 除名"
    );
    assert!(
        !registry.request_stop(&session_id),
        "收口后 stop 寻址 miss 幂等 false"
    );
}

/// 两次 `compose_turn` 共享同一 `Arc<SessionStreamRegistry>`：各自 `begin` 的
/// 条目并存、互不覆盖（exec / change 双轨同源多轨共存形态）。
#[tokio::test]
async fn 两次compose_turn共享同一streams_条目并存互不覆盖() {
    let env = DualEnv::new("streams-shared");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "共享供应");
    seed_sdk_instance(stores, "共享实例", Some(provider_id));
    let streams = Arc::new(SessionStreamRegistry::new());

    let exec_track = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("exec 轨解析应成功");
    let change_track = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::clone(&streams),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("change 轨解析应成功");

    let first = exec_track
        .begin(
            SessionRef::New,
            "exec 轨一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("exec 轨 begin 应成功");
    let second = change_track
        .begin(
            SessionRef::New,
            "change 轨一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: "change".to_owned(),
                source_ref: Some("chg-flow/implement/executor/1".to_owned()),
            },
        )
        .expect("change 轨 begin 应成功");

    assert_ne!(first.session_id, second.session_id, "两会话各自铸 id");
    assert!(
        streams.subscribe(&first.session_id, 0).is_some(),
        "exec 轨条目不被后者覆盖"
    );
    assert!(
        streams.subscribe(&second.session_id, 0).is_some(),
        "change 轨条目并存"
    );
}

// ---------------------------------------------------------------------------
// 协议 / 上限旁路（AC-4 / AC-8 组合根接线）：store 协议枚举 → 门面 Protocol
// 单点映射，max_tokens 与 context_length 同型旁路
// ---------------------------------------------------------------------------

use crate::sdk::config::Protocol;

/// 协议 / 上限 / 窗长可指定的 provider fixture（组合根解析面用例底座）。
fn seed_full_provider(
    stores: &WorkspaceStores,
    name: &str,
    protocol: AgentProviderProtocol,
    high: &str,
    low: &str,
    context_length: Option<u64>,
    max_tokens: Option<u64>,
) -> i64 {
    let provider = AgentProviderRecord::new(
        name.to_owned(),
        protocol,
        "http://127.0.0.1:9/v1".to_owned(),
        "sk-compose".to_owned(),
        AgentModelTiers {
            high: high.to_owned(),
            medium: "m-medium".to_owned(),
            low: low.to_owned(),
        },
        context_length,
        max_tokens,
    );
    stores
        .global()
        .upsert_agent_provider(provider)
        .expect("落 provider fixture")
        .id
}

#[test]
fn map_protocol两臂一对一映射且无第三分支() {
    // 私有单点映射：两变体各一行，映射不全即失败
    assert_eq!(
        super::map_protocol(AgentProviderProtocol::Openai),
        Protocol::Openai
    );
    assert_eq!(
        super::map_protocol(AgentProviderProtocol::Anthropic),
        Protocol::Anthropic
    );
    // 无兜底缺省：两变体映射结果互异（错映射即协议选臂错）
    assert_ne!(
        super::map_protocol(AgentProviderProtocol::Openai),
        super::map_protocol(AgentProviderProtocol::Anthropic)
    );
}

#[test]
fn resolve_sdk臂透传provider协议且其余字段逐字段保真() {
    for (protocol, expected) in [
        (AgentProviderProtocol::Anthropic, Protocol::Anthropic),
        (AgentProviderProtocol::Openai, Protocol::Openai),
    ] {
        let env = DualEnv::new("protocol-passthrough");
        let stores = env.stores();
        let provider_id = seed_full_provider(
            stores,
            "协议供应",
            protocol,
            "claude-x[1m]",
            "claude-y",
            Some(200_000),
            None,
        );
        let instance_id = seed_sdk_instance(stores, "协议实例", Some(provider_id));

        let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
            Ok(resolved) => resolved,
            Err(error) => panic!("解析应成功: {error}"),
        };
        assert_eq!(
            resolved.config.protocol, expected,
            "provider 协议 {protocol:?} 经 map_protocol 一对一抵达连接配置"
        );
        // 其余连接字段逐字段保真（映射不污染其它面）
        assert_eq!(resolved.config.api_key, "sk-compose");
        assert_eq!(resolved.config.base_url, "http://127.0.0.1:9/v1");
        assert_eq!(
            resolved.config.model_high, "claude-x[1m]",
            "模型原文透传（后缀不归一化）"
        );
        assert_eq!(resolved.config.model_low, "claude-y");
        assert!(matches!(resolved.kind, crate::EngineKind::Sdk));
        assert_eq!(resolved.agent_instance_id, instance_id);
    }
}

#[test]
fn resolve_sdk臂透传provider的max_tokens旁路() {
    let env = DualEnv::new("max-tokens-passthrough");
    let stores = env.stores();
    let provider_id = seed_full_provider(
        stores,
        "上限供应",
        AgentProviderProtocol::Anthropic,
        "claude-x",
        "claude-y",
        Some(200_000),
        Some(8192),
    );
    let instance_id = seed_sdk_instance(stores, "上限实例", Some(provider_id));

    let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
        Ok(resolved) => resolved,
        Err(error) => panic!("解析应成功: {error}"),
    };
    assert_eq!(
        resolved.max_tokens,
        Some(8192),
        "provider max_tokens 原样旁路（与 context_window 并列）"
    );
    assert_eq!(resolved.context_window, Some(200_000), "窗长旁路互不覆盖");
}

#[test]
fn resolve_sdk臂双可空列四组合独立携带且不落缺省字面() {
    // context_length × max_tokens 四组合：两字段各自独立携带、互不覆盖
    for (context_length, max_tokens) in [
        (None, None),
        (Some(200_000), None),
        (None, Some(8192)),
        (Some(131_072), Some(32_000)),
    ] {
        let env = DualEnv::new("dual-nullable");
        let stores = env.stores();
        let provider_id = seed_full_provider(
            stores,
            "双空列供应",
            AgentProviderProtocol::Anthropic,
            "claude-x",
            "claude-y",
            context_length,
            max_tokens,
        );
        let instance_id = seed_sdk_instance(stores, "双空列实例", Some(provider_id));

        let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
            Ok(resolved) => resolved,
            Err(error) => panic!("解析应成功: {error}"),
        };
        assert_eq!(
            resolved.context_window, context_length,
            "窗长列原样携带（组合 {context_length:?} / {max_tokens:?}）"
        );
        assert_eq!(
            resolved.max_tokens, max_tokens,
            "上限列原样携带（组合 {context_length:?} / {max_tokens:?}）"
        );
    }

    // max_tokens = Some(0) 原样携带（解析层不做下限校验，拒面在命令层）
    let env = DualEnv::new("max-tokens-zero");
    let stores = env.stores();
    let provider_id = seed_full_provider(
        stores,
        "零上限供应",
        AgentProviderProtocol::Anthropic,
        "claude-x",
        "claude-y",
        None,
        Some(0),
    );
    let instance_id = seed_sdk_instance(stores, "零上限实例", Some(provider_id));
    let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
        Ok(resolved) => resolved,
        Err(error) => panic!("解析应成功: {error}"),
    };
    assert_eq!(
        resolved.max_tokens,
        Some(0),
        "Some(0) 原样携带（组合根不夹取不拒收）"
    );
}

/// 组合：provider（anthropic + `max_tokens` + `[1m]` 后缀）→ 组合根解析 →
/// 门面 builder → `runner_for(Sdk)` → `open_session` 首轮：裸名上报、配置原文
/// 留快照面、协议抵达引擎（三面同源，discard 端点点零触网）。
#[tokio::test]
async fn 组合根全链_provider协议与上限与后缀抵达引擎且裸名上报() {
    let env = DualEnv::new("full-chain");
    let stores = env.stores();
    let provider_id = seed_full_provider(
        stores,
        "全链供应",
        AgentProviderProtocol::Anthropic,
        "claude-x[1m]",
        "claude-y",
        Some(64),
        Some(8192),
    );
    let instance_id = seed_sdk_instance(stores, "全链实例", Some(provider_id));

    let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
        Ok(resolved) => resolved,
        Err(error) => panic!("解析应成功: {error}"),
    };
    // 协议与上限抵达解析产物（组合根半边）
    assert_eq!(resolved.config.protocol, Protocol::Anthropic);
    assert_eq!(resolved.max_tokens, Some(8192));
    assert_eq!(resolved.context_window, Some(64));

    // 门面 builder 链（与 compose_turn 内同序）→ 引擎 open 首轮
    let runner = crate::EngineFacade::new()
        .with_context_window(resolved.context_window)
        .with_max_tokens(resolved.max_tokens)
        .runner_for(resolved.kind, resolved.config.clone())
        .expect("Sdk 臂构造成功");
    let mut session = runner
        .open_session(SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: ctx(&env.root()),
            session: SessionRef::New,
            session_id: "core-session-1".to_owned(),
            prior_handle: None,
        })
        .expect("协议配置 open 应成功（组装不触网）");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "全链一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    let started = session.observations.recv().await.expect("RunStarted");
    let agent::AgentEventKind::RunStarted { model, .. } = &started else {
        panic!("首事件应为 RunStarted，实际: {started:?}");
    };
    assert_eq!(
        model.as_deref(),
        Some("claude-x"),
        "裸名上报（后缀在引擎侧剥离，配置原文留快照面）"
    );
    // R9 双面裁定：配置原文（快照 model 载荷）不随上报归一化
    assert_eq!(
        resolved.config.model_high, "claude-x[1m]",
        "SessionConfigSnapshot.model 仍为配置原文"
    );
    drop(session);
}

// ---------------------------------------------------------------------------
// compose_turn 尾参（主仓 workspace_root）
//
// 门面只做 `EngineFacade::with_workspace_root(workspace_root.to_path_buf())`
// 透传：不解析、不校验；store 半边仍由注入的 `Arc<store::Store>` 决定（双
// root 不变量）。载荷在门面内不可读回，故断言面收敛为「尾参在场装配照常」
// 与「根形态不校验」「与 store 不同源」。
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compose_turn尾参在场装配照常_引擎快照与既有口径一致() {
    let env = DualEnv::new("tail-arg");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "尾参供应");
    seed_sdk_instance(stores, "尾参实例", Some(provider_id));

    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env.store(),
        None,
        Path::new(&env.root()),
        None,
    )
    .expect("尾参在场解析应成功");

    let running = composed
        .begin(
            SessionRef::New,
            "尾参装配一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    drop(running);

    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    let session = store
        .find_session(&session_id)
        .expect("find_session 应成功")
        .expect("会话行已落库");
    assert_eq!(
        session.config_snapshot.engine,
        AgentEngineKind::Sdk,
        "尾参不改装配产物（引擎选型与快照与既有用例一致）"
    );
    assert_eq!(
        session.config_snapshot.model,
        Some("m-high".to_owned()),
        "快照模型仍取 provider high 档"
    );
}

#[tokio::test]
async fn compose_turn尾参root形态不校验() {
    let env = DualEnv::new("tail-root-shape");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "根形态供应");
    seed_sdk_instance(stores, "根形态实例", Some(provider_id));

    let emoji = tempfile::Builder::new()
        .prefix("compose-test-根-🚀-")
        .tempdir()
        .expect("创建含 emoji 前缀的临时目录失败");
    let roots = [
        std::path::PathBuf::new(),
        std::path::PathBuf::from("D:/__not_exists__/compose-root"),
        emoji.path().to_path_buf(),
    ];
    for root in roots {
        let composed = compose_turn(
            stores,
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::new(agent::AskRegistry::default()),
            env.store(),
            None,
            &root,
            None,
        )
        .unwrap_or_else(|error| panic!("root 形态 {root:?} 只透传不校验（仍 Ok）: {error}"));
        drop(composed);
    }
}

#[tokio::test]
async fn compose_turn尾参与store的root不同源_双root不变量成立() {
    let env = DualEnv::new("dual-root");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "双根供应");
    seed_sdk_instance(stores, "双根实例", Some(provider_id));
    // 尾参 = 主仓 root（worktree 形态下与执行根不同源）
    let main_root = tempfile::Builder::new()
        .prefix("compose-test-main-root-")
        .tempdir()
        .expect("创建主仓 root 临时目录失败");
    let main_root_str = main_root.path().to_string_lossy().into_owned();

    let injected = env.store();
    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        std::sync::Arc::clone(&injected),
        None,
        main_root.path(),
        None,
    )
    .expect("尾参与 store 不同源仍应成功");

    let running = composed
        .begin(
            SessionRef::New,
            "双根一轮".to_owned(),
            ctx(&env.root()),
            debug_provenance(),
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    drop(running);

    assert!(
        injected
            .find_session(&session_id)
            .expect("find_session 应成功")
            .is_some(),
        "会话行落 store 半边（执行根库）"
    );
    let other = stores
        .for_root(&main_root_str)
        .expect("主仓 root 异根实例化合法");
    assert!(
        other
            .list_sessions(None, None)
            .expect("异根库清单应成功")
            .is_empty(),
        "尾参 root 不触发第二处库解析（store 半边唯一入口 = 注入参）"
    );
}

// ---------------------------------------------------------------------------
// web_search 门控单点（AC-8 / D8）：`provider.web_search && protocol == Anthropic`
// ---------------------------------------------------------------------------

/// 携协议与开关的 provider fixture（解析仅组装不触网络）。
fn seed_provider_with_switch(
    stores: &WorkspaceStores,
    name: &str,
    protocol: AgentProviderProtocol,
    web_search: bool,
) -> i64 {
    let provider = AgentProviderRecord::new(
        name.to_owned(),
        protocol,
        "http://127.0.0.1:9/v1".to_owned(),
        "sk-compose".to_owned(),
        AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
        None,
        None,
    )
    .with_web_search(web_search);
    stores
        .global()
        .upsert_agent_provider(provider)
        .expect("落 provider fixture")
        .id
}

#[test]
fn web_search门控四格映射_仅anthropic且开为真() {
    let cases = [
        (AgentProviderProtocol::Anthropic, true, true),
        (AgentProviderProtocol::Openai, true, false),
        (AgentProviderProtocol::Anthropic, false, false),
        (AgentProviderProtocol::Openai, false, false),
    ];

    for (protocol, provider_switch, expected) in cases {
        let env = DualEnv::new("ws-gate");
        let stores = env.stores();
        let provider_id = seed_provider_with_switch(stores, "门控供应", protocol, provider_switch);
        let instance_id = seed_sdk_instance(stores, "门控实例", Some(provider_id));

        let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
            Ok(resolved) => resolved,
            Err(error) => panic!("解析应成功: {error}"),
        };

        assert_eq!(
            resolved.web_search, expected,
            "({protocol:?}, {provider_switch}) → web_search = {expected}"
        );
        // 记录值不被解析期清零（保存期无条件接受、解析期只做最终值判定）
        let stored = stores
            .global()
            .find_agent_provider(provider_id)
            .expect("读 provider 成功")
            .expect("provider 行在场");
        assert_eq!(
            stored.web_search, provider_switch,
            "provider 记录的 web_search 原值不被解析期改写"
        );
    }
}

#[test]
fn 先开开关后改协议_解析为假但记录值保留且改回后恢复为真() {
    let env = DualEnv::new("ws-gate-switch");
    let stores = env.stores();
    // 「先开开关」：记录 web_search = true（openai 下保存期无条件接受）
    let provider_id =
        seed_provider_with_switch(stores, "切换供应", AgentProviderProtocol::Openai, true);
    let instance_id = seed_sdk_instance(stores, "切换实例", Some(provider_id));

    let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
        Ok(resolved) => resolved,
        Err(error) => panic!("解析应成功: {error}"),
    };
    assert!(!resolved.web_search, "openai 协议下不生效（门控单点）");
    let stored = stores
        .global()
        .find_agent_provider(provider_id)
        .expect("读 provider 成功")
        .expect("provider 行在场");
    assert!(stored.web_search, "记录值不被清零（他端仍为 true）");

    // 「再改回 anthropic」：同一记录重新解析即恢复为真（零静默清零）
    let mut updated = stored;
    updated.protocol = AgentProviderProtocol::Anthropic;
    stores
        .global()
        .upsert_agent_provider(updated)
        .expect("改协议落库成功");
    let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
        Ok(resolved) => resolved,
        Err(error) => panic!("改协议后解析应成功: {error}"),
    };
    assert!(resolved.web_search, "改回 anthropic → 解析为真");
}

#[test]
fn 解析失败面不被开关改写_agent缺失与cli两形态同一err且不完整配置在同一点失败() {
    // 1) agent 不存在：解析层同一 Err 文案，开关不改变失败面
    for web_search in [true, false] {
        let env = DualEnv::new("ws-gate-missing-agent");
        let stores = env.stores();
        // 先落一个带开关的 provider（记录形态与失败面无关，仅保证库内形态一致）
        let _provider_id = seed_provider_with_switch(
            stores,
            "失败面供应",
            AgentProviderProtocol::Anthropic,
            web_search,
        );
        let error = err_of(super::resolve_agent_engine(stores, Some(999)));
        assert!(
            error.contains("agent 不存在: id=999"),
            "开关 {web_search}：agent 缺失错误面不变，实际: {error}"
        );
    }

    // 2) cli 臂：解析单点即拒，逐字退役文案（与开关无关）
    for web_search in [true, false] {
        let env = DualEnv::new("ws-gate-cli");
        let cli_id = preset_cli_instance(&env, 71, "cli开关实例", false);
        let stores = env.stores();
        assert_eq!(
            err_of(super::resolve_agent_engine(stores, Some(cli_id))),
            CLI_TENANT_RETIRED,
            "开关 {web_search}：cli 臂逐字拒收文案不变"
        );
    }

    // 3) 记录不完整：解析层不做完整性判定（web_search 不参与 is_complete），
    //    失败面后移到 runner 启动期，且开关不改变该失败文案
    for web_search in [true, false] {
        let env = DualEnv::new("ws-gate-incomplete");
        let stores = env.stores();
        let provider_id = stores
            .global()
            .upsert_agent_provider(
                AgentProviderRecord::new(
                    "残缺供应".to_owned(),
                    AgentProviderProtocol::Anthropic,
                    String::new(),
                    "sk-compose".to_owned(),
                    AgentModelTiers {
                        high: "m-high".to_owned(),
                        medium: "m-medium".to_owned(),
                        low: "m-low".to_owned(),
                    },
                    None,
                    None,
                )
                .with_web_search(web_search),
            )
            .expect("落残缺 fixture")
            .id;
        let instance_id = seed_sdk_instance(stores, "残缺实例", Some(provider_id));

        let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
            Ok(resolved) => resolved,
            Err(error) => panic!("解析层不做完整性判定（失败面在 runner 启动期）: {error}"),
        };
        assert_eq!(
            resolved.web_search, web_search,
            "残缺配置不改变开关产物（开关不参与完整性判定）"
        );
        assert!(resolved.config.base_url.is_empty(), "残缺配置原样透传");

        // 开关不改变启动期失败文案
        let with_switch = crate::EngineFacade::new()
            .with_web_search(resolved.web_search)
            .runner_for(crate::EngineKind::Sdk, resolved.config.clone())
            .expect("Sdk 臂构造成功");
        let without_switch = crate::EngineFacade::new()
            .runner_for(crate::EngineKind::Sdk, resolved.config.clone())
            .expect("Sdk 臂构造成功");
        let open = SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: ctx(r"D:\工作区"),
            session: SessionRef::Continue {
                id: "ses-incomplete".to_owned(),
            },
            session_id: "core-session-1".to_owned(),
            prior_handle: Some("ses-incomplete".to_owned()),
        };
        assert_eq!(
            with_switch
                .open_session(open.clone())
                .expect_err("残缺配置续会话必须显式失败")
                .to_string(),
            without_switch
                .open_session(open)
                .expect_err("同口径失败")
                .to_string(),
            "开关 {web_search}：启动期失败文案一致"
        );
    }
}

#[test]
fn 门面链传递可考_解析产物经门面装配后与直构runner同值等价() {
    use crate::sdk::runner::SdkRunner;
    use crate::EngineFacade;

    let env = DualEnv::new("ws-gate-facade");
    let stores = env.stores();
    let provider_id =
        seed_provider_with_switch(stores, "门面供应", AgentProviderProtocol::Anthropic, true);
    let instance_id = seed_sdk_instance(stores, "门面实例", Some(provider_id));

    let resolved = match super::resolve_agent_engine(stores, Some(instance_id)) {
        Ok(resolved) => resolved,
        Err(error) => panic!("解析应成功: {error}"),
    };
    assert!(
        resolved.web_search,
        "门控产物为真（bool 全链非 Option<bool>）"
    );

    // 门面装配（与 compose_turn 同链）vs 直构 runner + 同值开关：行为等价
    let via_facade = EngineFacade::new()
        .with_web_search(resolved.web_search)
        .runner_for(crate::EngineKind::Sdk, resolved.config.clone())
        .expect("Sdk 臂构造成功");
    let via_direct =
        SdkRunner::new(resolved.config.clone(), None, None, None).with_web_search(true);

    let open = SessionOpen {
        injections: agent::SessionInjections::default(),
        ctx: ctx(r"D:\工作区"),
        session: SessionRef::Continue {
            id: "ses-facade-web-search".to_owned(),
        },
        session_id: "core-session-1".to_owned(),
        prior_handle: Some("ses-facade-web-search".to_owned()),
    };
    assert_eq!(
        via_facade
            .open_session(open.clone())
            .expect_err("无装载缝续会话必须显式失败")
            .to_string(),
        via_direct
            .open_session(open)
            .expect_err("直构同口径失败")
            .to_string(),
        "门面链传递与直构 runner 同值等价"
    );
}

// ---------------------------------------------------------------------------
// 注入矩阵（archi 章节）：`(phase, role)` 七行单源 + fail-closed 面
// ---------------------------------------------------------------------------

/// 观察流内某 `tool_use` 的配对 `tool_result`（应答回灌判据）。
fn tool_result_in(events: &[agent::AgentEventKind], tool_call_id: &str) -> Option<(String, bool)> {
    events.iter().find_map(|event| match event {
        agent::AgentEventKind::Message { blocks, .. } => {
            blocks.iter().find_map(|block| match block {
                agent::AgentBlock::ToolResult {
                    id,
                    content,
                    is_error,
                } if id == tool_call_id => Some((content.clone(), *is_error)),
                _ => None,
            })
        }
        _ => None,
    })
}

/// 装配入口（模块私有，本子模块直读——矩阵单源即被测语义）。
fn injections_of(source: &str, source_ref: Option<&str>) -> agent::SessionInjections {
    use super::assemble_injections;

    assemble_injections(&SessionProvenance {
        source: source.to_owned(),
        source_ref: source_ref.map(str::to_owned),
    })
    .expect("装配不得因 source_ref 形态报错（fail-closed 回落 default）")
}

/// 七内建工具名（名字清单底座）。
fn seven() -> Vec<String> {
    TOOL_NAMES.iter().map(|name| (*name).to_owned()).collect()
}

/// 取 archi 工具面的两位（名字清单里两名的在场形态；均缺席即 `None`）。
fn archi_bits(injections: &agent::SessionInjections) -> Option<(bool, bool)> {
    let names = injections.tools.as_deref().unwrap_or_default();
    let module = names
        .iter()
        .any(|name| name == crate::sdk::mount::ARCHI_MODULE_TOOL);
    let search = names
        .iter()
        .any(|name| name == crate::sdk::mount::ARCHI_SEARCH_TOOL);
    match (module, search) {
        (false, false) => None,
        (module, search) => Some((module, search)),
    }
}

/// 正向：矩阵七行逐行命中（每行 = 七工具名 + 矩阵勾选的 archi 名）。
#[test]
fn 注入矩阵七行逐行命中() {
    let rows: [(&str, &str, (bool, bool)); 7] = [
        ("proposal", "executor", (false, true)),
        ("dev-design", "executor", (true, true)),
        ("dev-design", "evaluator", (true, true)),
        ("test-design", "executor", (false, true)),
        ("implement", "executor", (true, false)),
        ("code-review", "evaluator", (true, false)),
        ("acceptance", "evaluator", (true, false)),
    ];
    for (phase, role, expected) in rows {
        let source_ref = format!("add-feature/{phase}/{role}/1");
        let injections = injections_of("change", Some(&source_ref));
        assert_eq!(
            archi_bits(&injections),
            Some(expected),
            "矩阵行 {phase}/{role} 的两位"
        );
        let mut expected_names = seven();
        if expected.0 {
            expected_names.push(crate::sdk::mount::ARCHI_MODULE_TOOL.to_owned());
        }
        if expected.1 {
            expected_names.push(crate::sdk::mount::ARCHI_SEARCH_TOOL.to_owned());
        }
        assert_eq!(
            injections.tools,
            Some(expected_names),
            "命中行名字清单 = 七工具名 + 矩阵勾选: {phase}/{role}"
        );
        assert!(injections.preamble.is_none(), "preamble 维持不消费");
    }
}

/// 异常：矩阵外相位 / role → `SessionInjections::default()`（零 archi 工具），不 Err。
#[test]
fn 矩阵外相位回落default() {
    for source_ref in [
        "add-feature/test-gen/executor/1",
        "add-feature/test-execution/executor/1",
        "add-feature/archive/spec_sync/1",
        "add-feature/code-analyze/executor/1",
        "add-feature/initialize/executor/1",
        "add-feature/proposal/evaluator/1",
    ] {
        let injections = injections_of("change", Some(source_ref));
        assert_eq!(
            injections.tools,
            Some(seven()),
            "矩阵外相位零 archi 工具（名字清单为七工具）: {source_ref}"
        );
        assert_eq!(archi_bits(&injections), None, "零 archi 工具: {source_ref}");
    }
}

/// 异常：畸形 `source_ref` → 恒 `default()`，不抛装配错误、不回退全量 archi 工具。
#[test]
fn 畸形source_ref恒default() {
    for source_ref in [
        "",
        "add-feature/implement/executor",
        "add-feature/implement/1",
        "add-feature/implement/executor/x",
        "add-feature/implement/executor/1/extra",
        "///",
    ] {
        let injections = injections_of("change", Some(source_ref));
        assert_eq!(
            injections.tools,
            Some(seven()),
            "畸形 source_ref {source_ref:?} 恒零 archi 工具（七工具名）"
        );
        assert_eq!(archi_bits(&injections), None);
    }
    // source_ref 缺席亦零 archi 工具
    let absent = injections_of("change", None);
    assert_eq!(absent.tools, Some(seven()));
    assert_eq!(archi_bits(&absent), None);
}

/// 边界：`attempt` 段边界（判据只校验可解析性、不校验范围、不做符号黑名单）——
/// `0` / `1` / `u32::MAX` / `+1`（`u32::from_str` 接受前导 `+`）均命中；`-1` /
/// 前导空格 / `1.0` / 非数字 / 溢出 `u32` → 解析失败回落 default。
#[test]
fn attempt段解析边界() {
    for attempt in ["0", "1", "4294967295"] {
        let source_ref = format!("add-feature/implement/executor/{attempt}");
        assert_eq!(
            archi_bits(&injections_of("change", Some(&source_ref))),
            Some((true, false)),
            "可解析的 attempt {attempt} 命中（不校验范围）"
        );
    }
    // `u32::from_str` 接受前导 `+`：`"+1"` 属可解析形态（判据只校验可解析性，
    // 不做正负号黑名单）→ 与 `0` / `1` / `u32::MAX` 同属命中面
    let plus_one = injections_of("change", Some("add-feature/implement/executor/+1"));
    assert_eq!(
        archi_bits(&plus_one),
        Some((true, false)),
        "前导 + 的 attempt 「+1」可解析（u32 FromStr 语义）→ 命中"
    );
    // 不可解析面：负号 / 前导空格 / 小数 / 非数字 / 超出 u32 的溢出值 → default
    for attempt in ["-1", " 1", "1.0", "u32", "4294967296"] {
        let source_ref = format!("add-feature/implement/executor/{attempt}");
        assert_eq!(
            archi_bits(&injections_of("change", Some(&source_ref))),
            None,
            "不可解析的 attempt {attempt:?} 零 archi 工具"
        );
    }
}

/// 回归：explore 臂名字清单（六工具 + `create_explore_note` 专用名、
/// `preamble = None`、零收口档位）。
#[test]
fn explore臂零回归() {
    let injections = injections_of("explore", Some("任何定位串"));
    assert_eq!(
        injections.tools,
        Some(vec![
            "read".to_owned(),
            "grep".to_owned(),
            "glob".to_owned(),
            "ls".to_owned(),
            "edit".to_owned(),
            "bash".to_owned(),
            EXPLORE_NOTE_TOOL.to_owned(),
        ]),
        "explore 臂名字清单 = 六工具 + 专用名（write 移除、序同 TOOL_NAMES + 尾追加）"
    );
    assert!(injections.preamble.is_none());
    assert_eq!(injections.finish, None, "explore 臂零收口档位");
}

/// 回归：其余来源（`debug` / 未知 / explore 之外）→ `default()`。
#[test]
fn 其余来源恒显式generic且零archi与零收口() {
    for source in ["debug", "unknown-source", ""] {
        let injections = injections_of(source, Some("add-feature/implement/executor/1"));
        assert_eq!(
            injections.tools, None,
            "来源 {source:?} 不注入 archi 工具面（tools 位 None = 内建七件）"
        );
        assert!(injections.preamble.is_none(), "其余来源零 preamble");
        assert!(injections.finish.is_none(), "其余来源零收口档位");
        assert_eq!(
            injections.ask,
            Some(agent::AskMode::Generic),
            "来源 {source:?} 的 ask 位为显式 Generic（R8：MUST NOT default()）"
        );
        assert_ne!(
            injections,
            agent::SessionInjections::default(),
            "来源 {source:?} 不是 default（其 ask = None 即静默禁用）"
        );
    }
}

// ---------------------------------------------------------------------------
// 注入矩阵（archi 会话章节）：`source = "archi"` 臂——七工具全量 + `Archi
// { module: true, search: true }` + 架构会话知识 preamble；该臂以 `source` 短路
// 命中，零 `source_ref` 读取（不碰相位解析、不 fail-closed）。
// ---------------------------------------------------------------------------

/// 正向：archi 臂全字段——`preamble` 与 `knowledge()` 逐字相等、名字清单为七
/// 工具名 + 两 archi 名、`finish = None`，返回 `Ok`（零 `Err` 路径）。
#[test]
fn archi臂全字段() {
    let injections = injections_of("archi", None);

    let expected = crate::archi_knowledge::knowledge();
    assert!(expected.is_some(), "真实骨架应拼装成功（前置）");
    assert_eq!(
        injections.preamble, expected,
        "preamble 即 knowledge() 同一调用结果（逐字相等）"
    );
    let mut expected_names = seven();
    expected_names.push(crate::sdk::mount::ARCHI_MODULE_TOOL.to_owned());
    expected_names.push(crate::sdk::mount::ARCHI_SEARCH_TOOL.to_owned());
    assert_eq!(
        injections.tools,
        Some(expected_names),
        "archi 臂名字清单 = 七工具名 + 两 archi 名"
    );
    assert_eq!(injections.finish, None, "archi 臂无收口档位");
}

/// 边界：`source_ref` 七形态一律落 archi 臂（不碰相位解析）——含合法四段
/// `{changeId}/implement/executor/1`（change 臂会命中 `{module: true, search:
/// false}`）与畸形四段 / 五段等，与 change 臂的可观察差异逐形态锁死。
#[test]
fn archi臂source_ref形态无关() {
    const FORMS: [Option<&str>; 7] = [
        None,
        Some(""),
        Some("anything"),
        Some("chg-1"),
        Some("chg-1/implement/executor/1"),
        Some("chg-1/implement/executor/not-a-number"),
        Some("a/b/c/1/2"),
    ];
    for source_ref in FORMS {
        let injections = injections_of("archi", source_ref);
        assert!(
            injections.preamble.is_some(),
            "source_ref = {source_ref:?} 时 preamble 仍取 knowledge()（不因形态缺席）"
        );
        assert_eq!(
            archi_bits(&injections),
            Some((true, true)),
            "source_ref = {source_ref:?} 恒落 archi 臂双开"
        );
        assert_eq!(
            injections.tools.as_ref().map(Vec::len),
            Some(9),
            "工具面九名"
        );
    }

    // 差异锁死：同一 source_ref 在 change 臂命中 `{true, false}`，archi 臂恒
    // `{true, true}`、且 change 臂 preamble 恒 None
    let same_ref = "chg-1/implement/executor/1";
    assert_eq!(
        archi_bits(&injections_of("change", Some(same_ref))),
        Some((true, false)),
        "change 臂 implement/executor 行（对照臂）"
    );
    assert!(
        injections_of("change", Some(same_ref)).preamble.is_none(),
        "change 臂不注入知识 preamble（对照臂）"
    );
    assert!(
        injections_of("archi", Some(same_ref)).preamble.is_some(),
        "archi 臂不读 source_ref 相位，preamble 恒在"
    );
}

/// 边界：臂序——`source = "explore"` 且 `source_ref` 取两形态仍为 explore 臂
/// （六工具 + `Explore` + `preamble = None`），archi 臂不抢占。
#[test]
fn 臂序explore在前() {
    for source_ref in [None, Some("chg-1/implement/executor/1")] {
        let injections = injections_of("explore", source_ref);
        assert_eq!(
            injections.tools,
            Some(vec![
                "read".to_owned(),
                "grep".to_owned(),
                "glob".to_owned(),
                "ls".to_owned(),
                "edit".to_owned(),
                "bash".to_owned(),
                EXPLORE_NOTE_TOOL.to_owned(),
            ]),
            "source_ref = {source_ref:?} 仍为 explore 名字清单（六名 + 专用名）"
        );
        assert!(
            injections
                .tools
                .as_ref()
                .expect("tools 在场")
                .iter()
                .any(|name| name == EXPLORE_NOTE_TOOL),
            "explore 臂名字清单含 create_explore_note"
        );
        assert!(injections.preamble.is_none(), "explore 臂不注入 archi 知识");
    }
}

/// 异常：archi 臂在 change 短路之前——三种变形 `source_ref` 不得落到
/// `SessionInjections::default()`（fail-closed 分支），否则即「`source_ref` 变形
/// 丢工具」。
#[test]
fn 臂序archi在change短路之前三变形不落default() {
    for source_ref in ["chg-1/implement/executor/x", "chg-1/implement", "a/b"] {
        let injections = injections_of("archi", Some(source_ref));
        assert_eq!(
            archi_bits(&injections),
            Some((true, true)),
            "archi 臂先于 change 短路：{source_ref:?} 不得 fail-closed"
        );
        assert_ne!(
            injections,
            agent::SessionInjections::default(),
            "变形 source_ref 下注入面仍为 archi 臂（非 default）"
        );
    }
}

/// 异常：`source` 词面精确——大小写变体 / 前后空格 / 近词 / 空串均非 archi 臂
/// （走 change 短路 → `default()`）；受控词大小写敏感、零 trim 归一。
#[test]
fn source词面精确零归一() {
    for source in ["Archi", "ARCHI", " archi", "archi ", "archi-chat", ""] {
        let injections = injections_of(source, None);
        assert_eq!(
            injections.tools, None,
            "来源 {source:?} 不命中 archi 臂（大小写敏感 + 零 trim）"
        );
        assert!(injections.preamble.is_none());
        assert!(injections.finish.is_none());
        assert_eq!(
            injections.ask,
            Some(agent::AskMode::Generic),
            "来源 {source:?} 落其余来源臂（ask 位显式 Generic）"
        );
    }
}

/// 边界：降级范围单字段（D15）——`preamble` 位独立取 `knowledge()`，`tools` 与
/// `toolset` 恒定；不得出现「preamble 无 ⇒ 整注入 default」的连带失效。
#[test]
fn 降级范围单字段preamble独立() {
    let injections = injections_of("archi", None);
    let default = agent::SessionInjections::default();

    // 逐字段：知识与工具面同拍在场，且工具面与 default 明确不同（连带失效的
    // 唯一可观察形态就是「整注入 = default」）
    assert_eq!(
        injections.preamble,
        crate::archi_knowledge::knowledge(),
        "preamble 位是唯一的可选位（与 knowledge() 同源）"
    );
    assert_eq!(injections.tools.as_ref().map(Vec::len), Some(9));
    assert_eq!(
        archi_bits(&injections),
        Some((true, true)),
        "工具面不因 preamble 位缺席而降级"
    );
    assert_ne!(
        injections, default,
        "知识拼不出时只剩 preamble 缺席，注入面仍非 default"
    );
    assert!(
        archi_bits(&default).is_none(),
        "default 形态零 archi 工具（对照：archi 臂工具面恒定）"
    );
}

/// 单例：`ARCHI_SOURCE` 字面量与命令层 `ARCHI_CHAT_SOURCE`、前端
/// `use-archi-chat` 三处同值（跨语言一侧由前端用例的 invoke 参数承载）。
#[test]
fn archi_source字面量单点() {
    assert_eq!(
        super::ARCHI_SOURCE,
        "archi",
        "archi 来源受控字符串单点（与 ARCHI_CHAT_SOURCE / 前端同值）"
    );
    assert_eq!(
        super::ARCHI_SOURCE.trim(),
        super::ARCHI_SOURCE,
        "字面量无前后空白（受控词精确匹配、零 trim 归一）"
    );
    assert_ne!(
        super::ARCHI_SOURCE,
        "Archi",
        "大小写敏感：首字母大写形态不命中"
    );
}

/// 正向：调用点链路——`composed.begin` 以 `SessionProvenance { source: "archi",
/// source_ref: None }` 发起，记录型假 runner 捕获的 `injections` 与
/// `assemble_injections` 直调结果逐字段全等（臂结果唯一来源，组合根零二次改写）。
#[tokio::test]
async fn archi臂调用点链路捕获与直调全等() {
    let env = DualEnv::new("archi-arm-chain");
    let stores = env.stores();
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    let runner = RecordingRunner::new();
    let sink = RecordingSink::new();
    let composed = composed_with_fakes(
        store,
        Arc::clone(&runner),
        Arc::clone(&sink),
        Some(("m-high".to_owned(), "m-low".to_owned())),
        81,
    );

    composed
        .begin(
            SessionRef::New,
            "架构页一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: super::ARCHI_SOURCE.to_owned(),
                source_ref: None,
            },
        )
        .expect("archi 来源发起应成功");

    let captured = runner.captured_injections();
    assert_eq!(captured.len(), 1, "恰一次 open_session 捕获");
    assert_eq!(
        captured[0],
        injections_of("archi", None),
        "组合根零二次改写（捕获面与装配面逐字段全等）"
    );
    assert_eq!(
        archi_bits(&captured[0]),
        Some((true, true)),
        "链路上仍是 archi 臂双开"
    );
    assert_eq!(sink.begin_turns.load(Ordering::Relaxed), 1, "一轮已开");
}

// ---------------------------------------------------------------------------
// `finish` 位按 role 置位 + 与矩阵解耦（agent-finalize-tool AC-4 / D5）
// ---------------------------------------------------------------------------

/// 三档 role 的 `finish` 位（正向）：evaluator → EvaluatorChecklist、
/// decision → Decision、executor → None（6 份 executor md 无 finish_step 字样
/// 的一致性面）。
#[test]
fn finish位按role置位三形态() {
    let cases = [
        ("evaluator", Some(agent::FinishSpec::EvaluatorChecklist)),
        ("decision", Some(agent::FinishSpec::Decision)),
        ("executor", None),
    ];
    for (role, expected) in cases {
        let source_ref = format!("c/implement/{role}/1");
        let injections = injections_of("change", Some(&source_ref));
        assert_eq!(injections.finish, expected, "role {role} 的 finish 位");
    }
}

/// `finish` 位与 archi 矩阵解耦（关键判据，AC-4 / D5）：决策会话落
/// `(fail_phase, "decision")` 恒不命中 7 行矩阵 → `tools == 七内建`，但
/// `finish` 位 MUST NOT 随矩阵 `None` 一并丢失（否则决策会话拿不到
/// `finish_step`）。
#[test]
fn finish位与矩阵解耦_决策会话恒持收口档位() {
    let injections = injections_of("change", Some("c/implement/decision/2"));
    assert_eq!(
        injections.finish,
        Some(agent::FinishSpec::Decision),
        "决策会话收口档位在位（finish 位不随矩阵 None 丢失）"
    );
    assert_eq!(
        injections.tools,
        Some(seven()),
        "矩阵未命中 → 工具面为七内建（零 archi 名）"
    );

    // 对照臂：evaluator 命中矩阵（code-review）→ 七内建 + archi_module，
    // finish 位同为 EvaluatorChecklist（两判据独立）
    let injections = injections_of("change", Some("c/code-review/evaluator/1"));
    assert_eq!(
        injections.finish,
        Some(agent::FinishSpec::EvaluatorChecklist)
    );
    assert_eq!(
        archi_bits(&injections),
        Some((true, false)),
        "对照：矩阵命中 archi_module"
    );
}

/// explore / archi 两臂 `finish: None`（边界）：专用会话与架构页通用会话不挂
/// 收口工具。
#[test]
fn 专用会话两臂零收口档位() {
    for (source, source_ref) in [
        ("explore", None),
        ("explore", Some("any/source/ref/1")),
        ("archi", None),
        ("archi", Some("c/implement/evaluator/1")),
    ] {
        let injections = injections_of(source, source_ref);
        assert_eq!(
            injections.finish, None,
            "来源 {source}（source_ref {source_ref:?}）零收口档位"
        );
    }
}

/// 未命中来源恒 default（边界）：`source` 非三者 / 未知串 → 三字段全 None。
#[test]
fn 未命中来源三字段全none且ask位显式generic() {
    for source in ["chat", "", "CHANGE", "Explore"] {
        let injections = injections_of(source, Some("c/implement/evaluator/1"));
        assert_eq!(injections.preamble, None, "来源 {source} 零 preamble");
        assert_eq!(injections.tools, None, "来源 {source} 零名字清单");
        assert_eq!(injections.finish, None, "来源 {source} 零收口档位");
        assert_eq!(
            injections.ask,
            Some(agent::AskMode::Generic),
            "来源 {source} 的 ask 位显式置位（不落 None）"
        );
    }
}

/// 非法名 `Err` 携名与来源（异常）：装配层成员校验经 `mount::lookup` 判定，
/// `Err` 面 MUST NOT 返回部分注入面。
#[test]
fn 装配层非法名err携名与来源() {
    use super::injected;
    let provenance = SessionProvenance {
        source: "change".to_owned(),
        source_ref: None,
    };
    let error = injected(
        &["read", "no_such_tool"],
        None,
        None,
        Some(agent::AskMode::Generic),
        &provenance,
    )
    .expect_err("清单外名字应 Err");
    assert!(
        error.contains("no_such_tool"),
        "文案携非法名（可归因）: {error}"
    );
    assert!(
        error.contains("change"),
        "文案携来源（装配面唯一可归因处）: {error}"
    );
    assert!(
        error.starts_with("注入面非法工具名"),
        "文案前缀与 mount 侧统一: {error}"
    );
}

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`assemble_injections` 四臂显式置位 ask 位（装配链入口）
// ---------------------------------------------------------------------------

/// explore 臂（正向，AC-1）：`ask == Some(Generic)`；`tools` 清单逐名不变（七件
/// 含 `create_explore_note`）；名字清单内不含 `AskUserQuestion`（ask 不占清单）。
#[test]
fn 装配explore臂ask位generic且清单零ask工具名() {
    let injections = injections_of("explore", None);
    assert_eq!(
        injections.ask,
        Some(agent::AskMode::Generic),
        "explore 臂 = Generic（只阻塞，零编排广播）"
    );
    assert_eq!(injections.finish, None, "explore 臂无收口工具");
    let names = injections.tools.as_deref().expect("explore 臂下发名字清单");
    assert_eq!(names.len(), 7, "六件 + create_explore_note");
    assert!(
        names
            .iter()
            .any(|name| name == crate::sdk::mount::EXPLORE_NOTE_TOOL),
        "清单含 explore 专用工具"
    );
    assert!(
        !names.iter().any(|name| name == "AskUserQuestion"),
        "ask 不占名字清单（按位追加的基线工具）"
    );
}

/// archi 臂（正向，参数化 2 形态 source_ref）：`ask == Some(Generic)`；九名 +
/// `preamble` 在场；`source_ref` 取任意值不改变该臂。
#[test]
fn 装配archi臂ask位generic且九名与preamble在场() {
    for source_ref in [None, Some("add-feature/dev-design/executor/1")] {
        let injections = injections_of("archi", source_ref);
        assert_eq!(
            injections.ask,
            Some(agent::AskMode::Generic),
            "archi 臂 = Generic（source_ref={source_ref:?} 不改变该臂）"
        );
        assert!(injections.preamble.is_some(), "archi 臂 preamble 在场");
        let names = injections.tools.as_deref().expect("archi 臂下发名字清单");
        assert_eq!(names.len(), 9, "七件 + 两 archi 名");
        assert_eq!(
            archi_bits(&injections),
            Some((true, true)),
            "两 archi 名双开"
        );
    }
}

/// change 臂（正向，组合 4 源形态 × 3 role，AC-2）：全部 `ask == Some(Flow)`
///（**不分 role / 不分矩阵命中**）；`finish` 位按 role 各自置位。
#[test]
fn 装配change臂ask位恒flow且finish按role置位() {
    let source_refs = [
        "add-feature/implement/executor/1",
        "add-feature/code-review/evaluator/2",
        "add-feature/implement/decision/1",
        // 畸形源（段数不足 / 空串）：role 解析失败但 ask 位不变
        "add-feature",
        "",
    ];
    for source_ref in source_refs {
        let injections = injections_of("change", Some(source_ref));
        assert_eq!(
            injections.ask,
            Some(agent::AskMode::Flow),
            "change 臂恒 Flow（source_ref={source_ref:?}）"
        );
        let role = source_ref.split('/').nth(2).unwrap_or("");
        let expect_finish = match role {
            "evaluator" => Some(agent::FinishSpec::EvaluatorChecklist),
            "decision" => Some(agent::FinishSpec::Decision),
            _ => None,
        };
        assert_eq!(
            injections.finish, expect_finish,
            "finish 位按 role 置位（source_ref={source_ref:?}）"
        );
    }
}

/// 其余来源臂显式 Generic（正向，参数化 2 形态，AC-1 / R8）：`debug` 与「不传
/// 来源」→ `ask == Some(Generic)`、其余三位全 None；**不是**
/// `SessionInjections::default()`（`default().ask == None`）。
#[test]
fn 装配其余来源臂显式generic而非default() {
    for source in ["debug", ""] {
        let injections = injections_of(source, None);
        assert_eq!(
            injections.ask,
            Some(agent::AskMode::Generic),
            "source={source:?} 臂 = Generic（显式置位）"
        );
        assert_eq!(injections.preamble, None);
        assert_eq!(injections.tools, None);
        assert_eq!(injections.finish, None);
        assert_ne!(
            injections,
            agent::SessionInjections::default(),
            "MUST NOT default()（其 ask = None 即静默禁用）"
        );
    }
}

/// 逐臂 `ask` 位穷举（边界，参数化 6 形态）：任何来源都不落 `None`（首期无禁用
/// 场景，D13）。
#[test]
fn 装配逐臂ask位穷举恒some() {
    let cases: [(&str, agent::AskMode); 6] = [
        ("explore", agent::AskMode::Generic),
        ("archi", agent::AskMode::Generic),
        ("change", agent::AskMode::Flow),
        ("debug", agent::AskMode::Generic),
        ("", agent::AskMode::Generic),
        ("未知来源", agent::AskMode::Generic),
    ];
    for (source, expect) in cases {
        let injections = injections_of(source, Some("add-feature/implement/executor/1"));
        assert_eq!(
            injections.ask,
            Some(expect),
            "source={source:?} 的 ask 位 = {expect:?}（任何来源都不落 None）"
        );
    }
}

/// 零 `default()` 兜底自证（异常，源码自证，R8）：`compose.rs` 的
/// `assemble_injections` 函数体零 `SessionInjections::default()` 命中，其余来源臂
/// 为显式结构字面量。
#[test]
fn 装配函数体零default兜底() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/compose.rs"),
    )
    .expect("读取 compose.rs");
    let start = own
        .find("fn assemble_injections(")
        .expect("assemble_injections 在场");
    let rest = &own[start..];
    let end = rest
        .find("\nfn ")
        .filter(|offset| *offset > 0)
        .unwrap_or(rest.len());
    let body = &rest[..end];
    assert!(
        !body.contains("SessionInjections::default()"),
        "assemble_injections 体内零 default() 命中（逐臂显式置位）"
    );
    assert!(
        body.contains("ask: Some(agent::AskMode::Generic)"),
        "其余来源臂为显式结构字面量（四字段全给）"
    );
}

/// 非法名 `Err` 语义零改（异常）：注册表含非法名 → 既有 `Err`（文案携非法名与
/// 来源），零 ask 相关新增分支干扰。
#[test]
fn 装配非法名err语义零改() {
    // `injected` 单点的 Err 文案由未知来源的合成名字清单触发——经 mount 校验
    let error = crate::sdk::mount::definitions(
        &agent::SessionInjections {
            preamble: None,
            tools: Some(vec!["no_such_tool".to_owned()]),
            finish: None,
            ask: Some(agent::AskMode::Generic),
        },
        "",
    )
    .expect_err("非法名应 Err");
    assert!(error.contains("no_such_tool"), "文案携非法名: {error}");
}

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`compose_turn` 两实参（注册表 + ask port）贯通
// ---------------------------------------------------------------------------

/// 记录式假 `AskPort`（组合面用具：把「组合根 → 执行体」的广播拍记成序）。
#[derive(Default)]
struct RecordingAskPort {
    calls: Mutex<Vec<&'static str>>,
    payloads: Mutex<Vec<orchestration::state::AskPayload>>,
}

impl RecordingAskPort {
    fn labels(&self) -> Vec<&'static str> {
        self.calls.lock().expect("调用记录锁不可中毒").clone()
    }

    fn payloads(&self) -> Vec<orchestration::state::AskPayload> {
        self.payloads.lock().expect("载荷记录锁不可中毒").clone()
    }
}

impl orchestration::port::AskPort for RecordingAskPort {
    fn ask_started(&self, ask: orchestration::state::AskPayload) {
        self.calls
            .lock()
            .expect("调用记录锁不可中毒")
            .push("started");
        self.payloads.lock().expect("载荷记录锁不可中毒").push(ask);
    }

    fn ask_resolved(&self) {
        self.calls
            .lock()
            .expect("调用记录锁不可中毒")
            .push("resolved");
    }
}

/// 注册表单表共享（正向，DD3 的机械判据）：`compose_turn(…, Arc::clone(&reg), …)`
/// 后组合根内的内核与门面指的是**同一张表**——外部 `reg.answer` 能命中引擎侧
/// 登记（登记 → 外部应答 → 接收方收到）。
#[tokio::test]
async fn compose_turn的注册表经双通道注入为同一张表() {
    let env = DualEnv::new("ask-shared-registry");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "ask 供应");
    seed_sdk_instance(stores, "ask 实例", Some(provider_id));
    let registry = Arc::new(agent::AskRegistry::default());

    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::clone(&registry),
        env.store(),
        None,
        Path::new(&env.root()),
        Some(Arc::new(RecordingAskPort::default())),
    )
    .expect("解析应成功");

    // 引擎侧登记的槽（模拟执行体进入阻塞前）经外部同一张表被应答
    let (tx, mut rx) = tokio::sync::oneshot::channel::<String>();
    assert!(
        registry.register("core-session-shared", tx),
        "外部登记（与组合根同一张表）"
    );
    assert!(
        registry
            .answer("core-session-shared", "共享表应答".to_owned())
            .is_ok(),
        "外部应答命中引擎侧登记"
    );
    assert_eq!(rx.try_recv().expect("收到"), "共享表应答");
    drop(composed);
}

/// 应答轮询：起轮后由测试侧等待注册表登记（登记点由执行体异步抵达）。
async fn answer_until_registered(registry: &Arc<agent::AskRegistry>, key: &str, text: &str) {
    for _ in 0..3000 {
        match registry.answer(key, text.to_owned()) {
            Ok(()) => return,
            Err(error) => {
                assert_eq!(error, "当前无等待中的 ask", "仅登记未完成时重试: {error}");
                tokio::task::yield_now().await;
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        }
    }
    panic!("等待 ask 登记超时（key={key}）");
}

/// `ask_port` 两形态（边界，参数化 2 形态，AC-2）：模型真的调用
/// `AskUserQuestion` →
/// - `Some(port)`：假 port 观测到 `["started","resolved"]` 两拍且载荷 `session_id`
///   等于组合根铸的 core 会话 id（组合根 → 门面 → 泵 → 执行体的贯通判据）；
/// - `None`：该调用以 `is_error` 回灌（DD2 防御臂），轮不中断。
///
/// 两形态都以脚本化 SSE 桩作模型边界——「组合成功」由此升级为「组合根注入的
/// 端口真的被执行体吃到」。
#[tokio::test]
async fn compose_turn的ask_port两形态均贯通执行体() {
    // Some 形态：广播两拍 + 载荷键为 core 会话 id
    let env = DualEnv::new("ask-port-shape-Some");
    let stores = env.stores();
    let stub = crate::ask_scripted_provider::AskScriptedProvider::new(
        "compose-flow",
        "tu_compose_ask",
        "选哪个？",
        &["甲", "乙"],
    );
    let provider_id = seed_provider_at(stores, "ask port 供应", &stub.base_url);
    seed_sdk_instance(stores, "ask port 实例", Some(provider_id));
    let registry = Arc::new(agent::AskRegistry::default());
    let port = Arc::new(RecordingAskPort::default());
    let composed = compose_turn(
        stores,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::clone(&registry),
        env.store(),
        None,
        Path::new(&env.root()),
        Some(Arc::clone(&port) as Arc<dyn orchestration::port::AskPort>),
    )
    .expect("Some 形态组合成功");
    let running = composed
        .begin(
            SessionRef::New,
            "组合根 ask 一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: "change".to_owned(),
                source_ref: Some("chg-ask/implement/executor/1".to_owned()),
            },
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    let answerer = {
        let registry = Arc::clone(&registry);
        let key = session_id.clone();
        tokio::spawn(async move { answer_until_registered(&registry, &key, "甲").await })
    };
    let outcome = running.drive(|_output| {}).await;
    answerer.await.expect("应答任务结束");

    assert_eq!(
        outcome.status,
        agent::AgentRunStatus::Completed,
        "应答后同 turn 收口"
    );
    assert_eq!(port.labels(), vec!["started", "resolved"], "两拍各一次");
    let payloads = port.payloads();
    assert_eq!(payloads.len(), 1, "started 载荷一条");
    assert_eq!(
        payloads[0].session_id.as_deref(),
        Some(session_id.as_str()),
        "载荷 session_id = 组合根铸的 core 会话 id"
    );
    assert_eq!(
        payloads[0].tool_call_id.as_deref(),
        Some("tu_compose_ask"),
        "载荷 tool_call_id = 本次 tool_use id"
    );

    // None 形态：Flow 档无 port → is_error 防御臂（不悬挂）
    let env_none = DualEnv::new("ask-port-shape-None");
    let stores_none = env_none.stores();
    let stub_none = crate::ask_scripted_provider::AskScriptedProvider::new(
        "compose-flow-none",
        "tu_compose_no_port",
        "无编排面",
        &["x"],
    );
    let provider_none = seed_provider_at(stores_none, "ask 无 port 供应", &stub_none.base_url);
    seed_sdk_instance(stores_none, "ask 无 port 实例", Some(provider_none));
    let composed_none = compose_turn(
        stores_none,
        Arc::new(StopRegistry::new()),
        Arc::new(SessionStreamRegistry::new()),
        Arc::new(agent::AskRegistry::default()),
        env_none.store(),
        None,
        Path::new(&env_none.root()),
        None,
    )
    .expect("None 形态组合成功");
    let running_none = composed_none
        .begin(
            SessionRef::New,
            "无 port 一轮".to_owned(),
            ctx(&env_none.root()),
            SessionProvenance {
                source: "change".to_owned(),
                source_ref: Some("chg-ask/implement/executor/1".to_owned()),
            },
        )
        .expect("begin 应成功");
    let mut observed: Vec<agent::AgentEventKind> = Vec::new();
    let outcome_none = running_none
        .drive(|output| {
            if let agent::KernelOutput::Observation(event) = output {
                observed.push(event.kind);
            }
        })
        .await;
    assert_eq!(
        outcome_none.status,
        agent::AgentRunStatus::Completed,
        "防御臂不中断 run"
    );
    assert_eq!(
        tool_result_in(&observed, "tu_compose_no_port"),
        Some(("ask 编排状态面未装配".to_owned(), true)),
        "None 形态以 is_error 回灌 DD2 防御文案"
    );
}

/// change 来源端到端（组合，AC-2 的装配链侧证）：`source="change"` +
/// `source_ref = <change>/implement/executor/1` → `RunStarted.tools` 同样含
/// `AskUserQuestion`（同名覆盖不因来源而变），且 ask 项排在名字清单末位
/// （按位追加、不占名字清单）。
#[tokio::test(start_paused = true)]
async fn 装配产物真实驱动_change来源runstarted含ask工具() {
    use crate::sdk::mount::ARCHI_MODULE_TOOL;

    let env = DualEnv::new("drive-change-ask");
    let stores = env.stores();
    let provider_id = seed_provider(stores, "驱动供应change");
    seed_sdk_instance(stores, "驱动实例change", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    let composed = compose_turn(
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
    let running = composed
        .begin(
            SessionRef::New,
            "change 驱动一轮".to_owned(),
            ctx(&env.root()),
            SessionProvenance {
                source: "change".to_owned(),
                source_ref: Some("chg-ask/implement/executor/1".to_owned()),
            },
        )
        .expect("begin 应成功");
    let session_id = running.session_id.clone();
    drop(running.drive(|_output| {}).await);

    let transcript = store.list_session_events(&session_id).expect("转录应成功");
    let agent::AgentEventKind::RunStarted { tools, .. } = &transcript.first().expect("非空").kind
    else {
        panic!("转录首事件应为 RunStarted，实际: {:?}", transcript.first());
    };
    let expected: Vec<String> = TOOL_NAMES
        .iter()
        .map(|name| (*name).to_owned())
        .chain(std::iter::once(ARCHI_MODULE_TOOL.to_owned()))
        .chain(std::iter::once("AskUserQuestion".to_owned()))
        .collect();
    assert_eq!(
        tools, &expected,
        "change 来源的 RunStarted.tools = 七内建 + 矩阵命中的 archi 工具 + ask（末位）"
    );
    assert!(
        !tools[..tools.len() - 1].contains(&"AskUserQuestion".to_owned()),
        "ask 不占名字清单（仅末位按位追加）"
    );
}

/// 手工构造 `ask: None` 的注入面（不经 `assemble_injections`，异常 / AC-3）：
/// 名单投影不含 `AskUserQuestion`——`RunStarted.tools` 随该名单派生，故「显式
/// 禁用」在组合根下游等价于硬禁用。
#[test]
fn 手工构造ask位none的注入面零ask名单项() {
    use crate::sdk::mount;

    let injections = agent::SessionInjections {
        preamble: None,
        tools: None,
        finish: None,
        ask: None,
    };
    let mounts = mount::definitions(&injections, "").expect("内建名单装配应成功");
    let names: Vec<String> = mounts
        .iter()
        .map(|mount| mount.definition.name.clone())
        .collect();
    assert_eq!(
        names,
        seven(),
        "ask: None → 名单恰七内建（零 ask 项、零 finish 项）"
    );
    assert!(
        !names.iter().any(|name| name == "AskUserQuestion"),
        "显式禁用面: 名单零 ask 工具"
    );
    // 对照臂：装配链产出的注入面（任何来源）都带 ask 位，故上面形态只能手工构造
    for source in ["explore", "archi", "change", "debug"] {
        assert!(
            injections_of(source, Some("chg/implement/executor/1"))
                .ask
                .is_some(),
            "来源 {source} 的装配面 ask 位恒 Some（禁用形态不经装配链）"
        );
    }
}
