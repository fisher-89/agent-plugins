use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use native_db::{Builder, Models};
use tokio::sync::mpsc;

use agent::{
    AgentMessageRole, AgentPermissionMode, AgentRunner, AgentSession, AgentStartError, ModelLevel,
    RunHandle, SessionCtx, SessionKernel, SessionOpen, SessionProvenance, SessionQuery, SessionRef,
    StopRegistry,
};

use crate::store_port::StoreQuery;
use crate::{compose_turn, ComposedTurn};
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord,
    SessionConfigSnapshot, SessionRecord, WorkspaceStores, CLI_TENANT_RETIRED,
};

/// sdk provider fixture 的可指定窗长变体（AC-9 组合根接线用）。
fn seed_provider_with_context_length(
    stores: &WorkspaceStores,
    name: &str,
    context_length: Option<u64>,
) -> i64 {
    let provider = AgentProviderRecord::new(
        name.to_owned(),
        "http://127.0.0.1:9/v1".to_owned(),
        "sk-compose".to_owned(),
        AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
        context_length,
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
    let provider = AgentProviderRecord::new(
        name.to_owned(),
        "http://127.0.0.1:9/v1".to_owned(),
        "sk-compose".to_owned(),
        AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
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
fn seed_sdk_session(store: &store::Store, id: &str, remote: Option<&str>) -> () {
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
    let provider_id = seed_provider(&stores, "解析供应");
    seed_sdk_instance(&stores, "解析实例", Some(provider_id));

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
    let composed =
        compose_turn(stores, Arc::new(StopRegistry::new()), env.store(), None).expect("解析应成功");
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
        &stores,
        Arc::new(StopRegistry::new()),
        env.store(),
        Some(cli_id),
    ));

    assert_eq!(
        error, CLI_TENANT_RETIRED,
        "显式路径对 engine: Cli 实例逐字拒收（AC-4），实际: {error}"
    );
    // 拒收不落库不静默回退（不构造引擎）
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None)
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
        &stores,
        Arc::new(StopRegistry::new()),
        env.store(),
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
            .list_sessions(None, None)
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
        &stores,
        Arc::new(StopRegistry::new()),
        env.store(),
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
        env.store(),
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
    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
    let provider_id = seed_provider(&stores, "首轮供应");
    seed_sdk_instance(&stores, "首轮实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    // 未开轮：无引擎句柄（建档预建空会话形态）
    seed_sdk_session(&store, "ses-fresh", None);

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
    let provider_id = seed_provider(&stores, "缺柄供应");
    seed_sdk_instance(&stores, "缺柄实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    // 无引擎句柄，但已有轮行（开过轮）
    seed_sdk_session(&store, "ses-ran", None);
    store
        .begin_agent_turn("ses-ran", 1727000000000)
        .expect("开轮行");

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
    let provider_id = seed_provider(&stores, "句柄缺失供应");
    seed_sdk_instance(&stores, "句柄缺失实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-no-remote", None);
    seed_transcript(&store, "ses-no-remote");

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
    let provider_id = seed_provider(&stores, "驱动供应");
    seed_sdk_instance(&stores, "驱动实例", Some(provider_id));

    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
    let provider_id = seed_provider(&stores, "查询面供应");
    seed_sdk_instance(&stores, "查询面实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_sdk_session(&store, "ses-query", Some("sdk-77"));
    seed_transcript(&store, "ses-query");

    // ComposedTurn 产物即真实装配（query 直查 store 行 → 校验通过形态复用）
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
        env.store(),
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

// ---------------------------------------------------------------------------
// resolve Sdk 臂 context_length 透传（AC-9 组合根接线唯一触点）
// ---------------------------------------------------------------------------

#[test]
fn resolve_sdk臂透传provider的context_length() {
    let env = DualEnv::new("ctx-window");
    let stores = env.stores();
    let provider_id = seed_provider_with_context_length(&stores, "窗长供应", Some(200_000));
    let instance_id = seed_sdk_instance(&stores, "窗长实例", Some(provider_id));

    // 解析单点产物（compose_test 为 compose 子模块，可考私有解析产物）：
    // context_window == Some(200_000)，其余装配面不变
    let resolved = match super::resolve_agent_engine(&stores, Some(instance_id)) {
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
    let provider_id = seed_provider(&stores, "缺列供应");
    let instance_id = seed_sdk_instance(&stores, "缺列实例", Some(provider_id));

    let resolved = match super::resolve_agent_engine(&stores, Some(instance_id)) {
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

    let error = err_of(super::resolve_agent_engine(&stores, Some(cli_id)));

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
    let provider_id = seed_provider(&stores, "注入消费供应");
    let sdk_id = seed_sdk_instance(&stores, "注入消费实例", Some(provider_id));
    let root_b = tempfile::Builder::new()
        .prefix("compose-test-inject-other-root-")
        .tempdir()
        .expect("创建异根临时目录失败");
    let root_b_str = root_b.path().to_string_lossy().into_owned();

    // 注入实例 = for_root(rootA) 预解析（命令层装配同构）
    let injected = env.store();
    let composed = compose_turn(
        &stores,
        Arc::new(StopRegistry::new()),
        std::sync::Arc::clone(&injected),
        Some(sdk_id),
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

/// 记录型假 runner：捕获 `open_session` 的 `prior_handle` 载荷，返回最小
/// `AgentSession`（观测 / 提问通道即弃——本节用例只走 begin 同步段）。
struct RecordingRunner {
    prior_handles: Mutex<Vec<Option<String>>>,
}

impl RecordingRunner {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            prior_handles: Mutex::new(Vec::new()),
        })
    }

    /// 已捕获的 prior_handle 序列（按调用序）。
    fn captured(&self) -> Vec<Option<String>> {
        self.prior_handles.lock().expect("捕获锁不可中毒").clone()
    }
}

impl AgentRunner for RecordingRunner {
    fn open_session(&self, open: SessionOpen) -> Result<AgentSession, AgentStartError> {
        self.prior_handles
            .lock()
            .expect("捕获锁不可中毒")
            .push(open.prior_handle);
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
        kernel: SessionKernel::new(sink, Arc::new(StopRegistry::new())),
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
    assert_eq!(transcript.len(), 2, "密封转录原样可读（呈现兜底不受拒收影响）");
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
            &stores,
            Arc::new(StopRegistry::new()),
            env.store(),
            Some(id),
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
    let provider_id = seed_provider(&stores, "边界供应");
    seed_sdk_instance(&stores, "边界实例", Some(provider_id));
    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
    let provider_id = seed_provider(&stores, "快照供应");
    let sdk_id = seed_sdk_instance(&stores, "快照实例", Some(provider_id));
    let store = stores.for_root(&env.root()).expect("for_root 应成功");

    // 显式路径：快照携实例 id（DD8）
    let composed = compose_turn(
        &stores,
        Arc::new(StopRegistry::new()),
        env.store(),
        Some(sdk_id),
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
    let composed = compose_turn(&stores, Arc::new(StopRegistry::new()), env.store(), None)
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
