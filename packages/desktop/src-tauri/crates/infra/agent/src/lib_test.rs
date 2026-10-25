use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::sdk::config::Protocol;
use crate::sdk::runner::SdkRunner;
use crate::{
    compose_turn, session_query, ComposedTurn, EngineConfig, EngineFacade, EngineKind,
    ResumeTranscript,
};
use agent::AgentRunner;
use store::{WorkspaceStores, CLI_TENANT_RETIRED};

/// 内存三形态闭包替身 + 调用计数（装载缝注入依赖的测试侧替身）。
fn counting_loader(
    result: Result<Option<Vec<agent::AgentEvent>>, String>,
) -> (ResumeTranscript, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let loader: ResumeTranscript = Arc::new(move |_session_id: &str| {
        counter.fetch_add(1, Ordering::Relaxed);
        match &result {
            Ok(events) => Ok(events.clone()),
            Err(error) => Err(error.clone()),
        }
    });
    (loader, calls)
}

// ---------------------------------------------------------------------------
// crate 根导出面锚定（任一 re-export 破坏即编译失败）
// ---------------------------------------------------------------------------

#[test]
#[allow(clippy::type_complexity)]
fn crate根导出面锚定_组合根与查询面与既有门面re_export可达() {
    // compose_turn / ComposedTurn 以签名锚定（零调用；store 半边 = 命令层
    // 预解析注入的 workspace root 库实例，design D8 拆参）
    let compose_signature: fn(
        &WorkspaceStores,
        Arc<agent::StopRegistry>,
        Arc<agent::SessionStreamRegistry>,
        Arc<store::Store>,
        Option<i64>,
        &std::path::Path,
    ) -> Result<ComposedTurn, String> = compose_turn;
    let _ = compose_signature;

    // session_query（store_port 模块经 crate 根可达）：签名锚定
    let query_signature: fn(Arc<store::Store>) -> Arc<dyn agent::SessionQuery> = session_query;
    let _ = query_signature;

    // EngineConfig 三字段零改动承诺回归（构造形态与 empty 占位）
    let config = EngineConfig {
        protocol: Protocol::Openai,
        api_key: "k".to_owned(),
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        model_high: "mh".to_owned(),
        model_low: "ml".to_owned(),
    };
    assert!(!config.api_key.is_empty());
    let empty = EngineConfig::empty();
    assert!(
        empty.api_key.is_empty()
            && empty.base_url.is_empty()
            && empty.model_high.is_empty()
            && empty.model_low.is_empty()
    );

    // with_context_window 经门面可达（builder 缝锚定；不新增顶层 re-export 面
    // ——方法挂在 EngineFacade 上，无 crate 根 re-export）
    let windowed = EngineFacade::new().with_context_window(Some(200_000));
    let _ = windowed;
}

#[test]
fn engine_kind线格式值域逐字为cli与sdk且非法值拒绝() {
    for (kind, expected) in [(EngineKind::Cli, "cli"), (EngineKind::Sdk, "sdk")] {
        let serialized = serde_json::to_string(&kind).expect("序列化成功");
        assert_eq!(
            serialized,
            format!("\"{expected}\""),
            "线格式逐字 {expected}"
        );
        let roundtrip: EngineKind = serde_json::from_str(&serialized).expect("反序列化成功");
        assert_eq!(roundtrip, kind);
    }
    assert!(
        serde_json::from_str::<EngineKind>("\"api\"").is_err(),
        "清单外引擎值拒绝"
    );
}

// ---------------------------------------------------------------------------
// ResumeTranscript 缝语义平移（类型形状不变）
// ---------------------------------------------------------------------------

#[test]
fn resume_transcript缝类型形状入参str返回result_option_vec且闭包替身可注入() {
    // 形状锚定：Arc<dyn Fn(&str) -> Result<Option<Vec<AgentEvent>>, String> + Send + Sync>
    let (loader, calls) = counting_loader(Ok(Some(Vec::new())));

    // 直接调用形状回归：入参 &str、返回 Result<Option<Vec<AgentEvent>>, String>
    // （Ok(None) / Ok(Some(vec)) / Err 三形态均闭包可编程、原样透传）
    let direct = loader("ses-1").expect("Ok 形态返回 Ok");
    assert_eq!(direct, Some(Vec::new()), "Ok(Some(空史)) 形态原样透传");
    let (err_loader, err_calls) = counting_loader(Err("读取失败".to_owned()));
    assert!(err_loader("ses-2").is_err(), "Err 形态原样返回 Err(String)");
    let (none_loader, none_calls) = counting_loader(Ok(None));
    assert_eq!(
        none_loader("ses-3").expect("None 形态返回 Ok"),
        None,
        "Ok(None) 形态原样透传"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(err_calls.load(Ordering::Relaxed), 1);
    assert_eq!(none_calls.load(Ordering::Relaxed), 1);
}

// ---------------------------------------------------------------------------
// 门面分发回归
// ---------------------------------------------------------------------------

#[test]
fn sdk臂空配置open以config_missing显式失败() {
    let runner = EngineFacade::new()
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("Sdk 臂构造成功");
    let error = runner
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: Path::new("C:\\ws").to_path_buf(),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
                model_level: agent::ModelLevel::High,
            },
            session: agent::SessionRef::New,
            prior_handle: None,
        })
        .expect_err("Sdk 臂空配置必须显式失败");
    assert!(
        matches!(error, agent::AgentStartError::ConfigMissing(ref message)
            if message.contains("api_key") && message.contains("base_url") && message.contains("model")),
        "Sdk 臂空配置 ConfigMissing 区分三成因，实际: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// with_context_window builder 缝（AC-9 门面半边）
// ---------------------------------------------------------------------------

/// 直调 open_session 的 Continue 形态底座。
fn open_continue(session_id: &str) -> agent::SessionOpen {
    agent::SessionOpen {
        injections: agent::SessionInjections::default(),
        ctx: agent::SessionCtx {
            workspace_root: Path::new("C:\\ws").to_path_buf(),
            permission_mode: agent::AgentPermissionMode::BypassPermissions,
            model_level: agent::ModelLevel::High,
        },
        session: agent::SessionRef::Continue {
            id: session_id.to_owned(),
        },
        prior_handle: Some(session_id.to_owned()),
    }
}

#[test]
fn with_context_window_builder链式构造且分发行为与直构一致() {
    // 链式构造：new() → with_context_window(Some)（builder 缝，链式语义）
    let via_facade = EngineFacade::new()
        .with_context_window(Some(200_000))
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("Sdk 臂构造成功");
    let via_direct = SdkRunner::new(EngineConfig::empty(), None, Some(200_000), None);

    // 门面分发行为与直构一致（open 校验错误面等价断言，沿既有门面分发一致性口径）
    let facade_error = via_facade
        .open_session(open_continue("ses-window"))
        .expect_err("空配置必须 Err");
    let direct_error = via_direct
        .open_session(open_continue("ses-window"))
        .expect_err("直构同口径 Err");
    assert_eq!(
        facade_error, direct_error,
        "with_context_window 门面分发与直构 SdkRunner::new 三参一致"
    );

    // with_context_window(None) 同为合法载荷（透传 None = 缺省防线）
    let none_facade = EngineFacade::new()
        .with_context_window(None)
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("Sdk 臂构造成功");
    let none_direct = SdkRunner::new(EngineConfig::empty(), None, None, None);
    assert_eq!(
        none_facade
            .open_session(open_continue("ses-window-none"))
            .expect_err("")
            .to_string(),
        none_direct
            .open_session(open_continue("ses-window-none"))
            .expect_err("")
            .to_string(),
        "None 载荷分发行为一致"
    );
}

#[test]
fn 既有构造路径缺省none不变_续会话校验行为与现状一致() {
    // with_resume_transcript 产物（不携窗长）→ runner_for(Sdk)：Continue 校验
    // 行为与直构 SdkRunner::new(cfg, loader, None) 一致（缺省防线装配不改校验面）
    let (loader, _calls) = counting_loader(Ok(None));
    let via_facade = EngineFacade::with_resume_transcript(loader)
        .runner_for(EngineKind::Sdk, complete_cfg())
        .expect("Sdk 臂构造成功");
    let (direct_loader, _direct_calls) = counting_loader(Ok(None));
    let via_direct = SdkRunner::new(complete_cfg(), Some(direct_loader), None, None);

    let facade_error = via_facade
        .open_session(open_continue("ses-facade-default"))
        .expect_err("会话不存在必须 Err");
    let direct_error = via_direct
        .open_session(open_continue("ses-facade-default"))
        .expect_err("直构同口径 Err");
    assert_eq!(
        facade_error, direct_error,
        "不携窗长（缺省 None）行为与现状一致，既有断言零漂移"
    );
    let _ = _calls;
    let _ = _direct_calls;
}

/// 完整配置底座（与 runner_test 同款拒连端点）。
fn complete_cfg() -> EngineConfig {
    EngineConfig {
        protocol: Protocol::Openai,
        api_key: "sk-test".to_owned(),
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        model_high: "rig-test-model".to_owned(),
        model_low: "rig-test-model".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// 门面 Cli 臂显式 Err 防御兜底（DD1 / AC-2 / AC-4 文案面）
// ---------------------------------------------------------------------------

#[test]
fn cli臂直入runner_for以逐字退役文案显式拒绝且不panic() {
    // 旁路构造防御形态：生产路径不可达（组合根 resolve 先行拒 Cli），该臂为
    // 结构保留 + 防御兜底——弃 `unreachable!`，显式 Err 不 panic
    let error = match EngineFacade::new().runner_for(EngineKind::Cli, EngineConfig::empty()) {
        Err(error) => error,
        Ok(_) => panic!("Cli 臂必须显式 Err，不得返回 Ok 产物"),
    };
    assert_eq!(
        error, CLI_TENANT_RETIRED,
        "错误串逐字 CLI_TENANT_RETIRED（四拒点同文案，DD3），实际: {error}"
    );
    assert!(
        error.contains("CLI 租户已下线") && error.contains("/agents"),
        "文案含退役事实与管理页引导，实际: {error}"
    );
}

#[test]
fn cli臂零ok变体混入_双值穷举恰一分支() {
    // `EngineKind` 双值 match 穷举：Sdk 构造 Ok 产物（真实 SdkRunner）、Cli 恒
    // Err——唯一 match 点结构不塌（AC-2），Err 面无已删类型产物（编译锚）
    for kind in [EngineKind::Sdk, EngineKind::Cli] {
        let result = EngineFacade::new().runner_for(kind, complete_cfg());
        match kind {
            EngineKind::Sdk => {
                let runner = result.expect("Sdk 臂构造 Ok");
                // 产物经 trait 面可驱动（空续会话装载缝 → 显式失败不 panic）
                assert!(
                    runner
                        .open_session(open_continue("ses-double-arms"))
                        .is_err(),
                    "Sdk 臂产物为可驱动的 AgentRunner"
                );
            }
            EngineKind::Cli => {
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("Cli 臂恒 Err（不可达死臂不产 runner）"),
                };
                assert_eq!(error, CLI_TENANT_RETIRED);
            }
        }
    }
}

#[test]
fn cli臂拒收面不触装载缝_调用计数恒零() {
    let (loader, calls) = counting_loader(Ok(Some(Vec::new())));
    let error = match EngineFacade::with_resume_transcript(loader)
        .runner_for(EngineKind::Cli, EngineConfig::empty())
    {
        Err(error) => error,
        Ok(_) => panic!("Cli 臂必须显式 Err"),
    };
    assert_eq!(error, CLI_TENANT_RETIRED, "拒收文案不因装载缝注入而漂移");
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "Cli 臂拒收于构造面，装载缝零消费"
    );
}

// ---------------------------------------------------------------------------
// runner_for Sdk 臂构造接线边界（context_window / 字段 str 面）
// ---------------------------------------------------------------------------

#[test]
fn sdk臂窗长边界零与u64max均ok且行为与直构一致() {
    // `context_window: Option<u64>` 边界：Some(0) / Some(u64::MAX) 均 Ok 透传
    // （不 panic 不落缺省字面），分发行为与直构 SdkRunner 一致
    for window in [Some(0), Some(u64::MAX)] {
        let via_facade = EngineFacade::new()
            .with_context_window(window)
            .runner_for(EngineKind::Sdk, complete_cfg())
            .expect("Sdk 臂构造成功");
        let via_direct = SdkRunner::new(complete_cfg(), None, window, None);
        assert_eq!(
            via_facade
                .open_session(open_continue("ses-window-boundary"))
                .expect_err("无装载缝续会话必须显式失败")
                .to_string(),
            via_direct
                .open_session(open_continue("ses-window-boundary"))
                .expect_err("直构同口径失败")
                .to_string(),
            "窗长 {window:?} 载荷门面分发与直构一致"
        );
    }
}

#[tokio::test]
async fn sdk臂字段str边界怪值原样构造且空串仍逐字段判缺() {
    // 空串：仍判缺（不 trim 不回填——空即缺，字段面原样参与校验）
    let runner = EngineFacade::new()
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("空配置亦 Ok 构造（参数校验归 runner open 段，门面不抢做）");
    let blank_error = runner
        .open_session(open_continue("ses-blank-cfg"))
        .expect_err("空配置 open 必须显式失败");
    assert!(
        matches!(&blank_error, agent::AgentStartError::ConfigMissing(msg)
            if msg.contains("api_key") && msg.contains("base_url") && msg.contains("model")),
        "空串判缺成因可辨，实际: {blank_error:?}"
    );

    // 超长（>1000 字符）/ 特殊字符（`\n` `\0` emoji）/ 纯空白：非空即在场，
    // 构造面原样携带不加工（不 panic、不拒绝、不规范化为缺省）
    let weird = EngineConfig {
        protocol: Protocol::Openai,
        api_key: format!("sk-{}\n\u{0}🎯", "x".repeat(1024)),
        base_url: "   ".to_owned(),
        model_high: format!("m-high-{}", "超".repeat(1200)),
        model_low: String::new(),
    };
    let runner = EngineFacade::new()
        .runner_for(EngineKind::Sdk, weird)
        .expect("怪值配置构造成功");
    // Low 档：空串 model_low 原样参与判缺（怪值不吞掉空串成因）
    let low_error = runner
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: Path::new("C:\\ws").to_path_buf(),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
                model_level: agent::ModelLevel::Low,
            },
            session: agent::SessionRef::New,
            prior_handle: None,
        })
        .expect_err("model_low 空串低档必须显式失败");
    assert!(
        matches!(&low_error, agent::AgentStartError::ConfigMissing(msg) if msg.contains("model_low")),
        "空串字段原样参与判缺，实际: {low_error:?}"
    );
    // High 档：怪值字段非缺即在场——open 成功（原样携带不加工不拒绝）
    let _session = runner
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: Path::new("C:\\ws").to_path_buf(),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
                model_level: agent::ModelLevel::High,
            },
            session: agent::SessionRef::New,
            prior_handle: None,
        })
        .expect("超长 / 特殊字符 / 纯空白怪值 open 应成功（原样携带）");
}

// ---------------------------------------------------------------------------
// auto-named-explore：with_explore_note_store builder 缝（组合根 store 句柄
// 注入 builder）
// ---------------------------------------------------------------------------

/// 真实临时 workspace 库句柄装置（store_port_test 同款：tempdir + open_workspace）。
fn temp_store(tag: &str) -> (tempfile::TempDir, Arc<store::Store>) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("facade-explore-store-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败");
    let db_path = dir.path().join("ws.redb");
    let store = Arc::new(
        store::Store::open_workspace(&db_path)
            .unwrap_or_else(|e| panic!("打开临时 workspace 库失败: {e}")),
    );
    (dir, store)
}

/// builder 链式：`with_explore_note_store(store)` 返回 `Self`，与
/// `with_context_window` 双向续链（两序均编译可达且构造成功——builder 缝
/// 与 `with_resume_transcript` / `with_context_window` 同型）。
#[test]
fn with_explore_note_store链式构造且与with_context_window双向续链() {
    let (_dir, store) = temp_store("chain");

    // store → window 序
    let chained = EngineFacade::new()
        .with_explore_note_store(Arc::clone(&store))
        .with_context_window(Some(200_000))
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("续链后 Sdk 臂构造成功");
    let _ = chained;

    // window → store 序（builder 全序可换）
    let chained_other_order = EngineFacade::new()
        .with_context_window(None)
        .with_explore_note_store(Arc::clone(&store))
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("换序续链后 Sdk 臂构造成功");
    let _ = chained_other_order;
}

/// 句柄注入后引擎选择回归：带 store 句柄的 facade `runner_for(Sdk)` 仍产出
/// Sdk 引擎（builder 不破坏既有选择面——open_session 失败面与直构四参
/// SdkRunner 同口径）。
#[test]
fn 携store句柄facade的runner_for_sdk仍产出sdk引擎() {
    let (_dir, store) = temp_store("engine-choice");
    let via_facade = EngineFacade::new()
        .with_explore_note_store(Arc::clone(&store))
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("Sdk 臂构造成功（仍选 Sdk 臂）");
    let via_direct = SdkRunner::new(EngineConfig::empty(), None, None, Some(Arc::clone(&store)));

    let facade_error = via_facade
        .open_session(open_continue("ses-with-store"))
        .expect_err("空配置必须 Err");
    let direct_error = via_direct
        .open_session(open_continue("ses-with-store"))
        .expect_err("直构同口径 Err");
    assert_eq!(
        facade_error, direct_error,
        "携句柄门面与直构四参 SdkRunner 失败面一致（引擎选择零漂移）"
    );
}

/// 默认构造零句柄回归：`new()` / `with_resume_transcript` 路径行为不随新
/// 字段变化（缺省 `explore_note_store == None` 的行为面等价于既有三参直构）。
#[test]
fn 默认构造零句柄行为不随新字段变化() {
    // new() 路径：与三参 None 直构行为一致
    let via_new = EngineFacade::new()
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("Sdk 臂构造成功");
    let direct_none = SdkRunner::new(EngineConfig::empty(), None, None, None);
    assert_eq!(
        via_new
            .open_session(open_continue("ses-default-no-handle"))
            .expect_err("必须 Err")
            .to_string(),
        direct_none
            .open_session(open_continue("ses-default-no-handle"))
            .expect_err("直构必须 Err")
            .to_string(),
        "new() 门面（零句柄）与 None 直构失败面一致"
    );

    // with_resume_transcript 路径：不携句柄，行为照旧
    let (loader, calls) = counting_loader(Ok(None));
    let via_loader = EngineFacade::with_resume_transcript(loader)
        .runner_for(EngineKind::Sdk, complete_cfg())
        .expect("Sdk 臂构造成功");
    assert!(
        via_loader
            .open_session(open_continue("ses-loader-no-handle"))
            .is_err(),
        "无句柄续会话照旧显式失败（会话不存在臂）"
    );
    // 句柄缺席不改变装载缝消费时点：失败前 loader 被消费一次（既有时点回归）
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "装载缝照常消费（新字段不改变既有路径）"
    );
}

/// 空配置启动失败面回归：携 store 句柄的 facade + `EngineConfig::empty()` →
/// `open_session` 仍以 `ConfigMissing`（配置三件套逐项在册）显式失败——新
/// 字段与 builder 不吞改既有失败语义。
#[test]
fn 携store句柄的空配置open仍config_missing显式失败() {
    let (_dir, store) = temp_store("empty-cfg");
    let runner = EngineFacade::new()
        .with_explore_note_store(Arc::clone(&store))
        .with_context_window(None)
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("Sdk 臂构造成功");

    let error = runner
        .open_session(open_continue("ses-empty-cfg-with-store"))
        .expect_err("空配置必须显式失败");
    assert!(
        matches!(error, agent::AgentStartError::ConfigMissing(ref message)
            if message.contains("api_key") && message.contains("base_url") && message.contains("model")),
        "配置三件套校验先于句柄消费，实际: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// with_max_tokens builder 缝（DD14 / AC-8 门面接线半边）：与
// with_context_window 同型并列，`max_tokens` MUST NOT 进 EngineConfig
// ---------------------------------------------------------------------------

#[tokio::test]
async fn with_max_tokens分发等价_与直构runner加同载荷行为一致() {
    // 门面链：new() → with_max_tokens(Some(8192)) → runner_for(Sdk)
    let via_facade = EngineFacade::new()
        .with_max_tokens(Some(8192))
        .runner_for(EngineKind::Sdk, complete_cfg())
        .expect("Sdk 臂构造成功");
    // 直构链：SdkRunner::new(四参签名不动) + with_max_tokens
    let via_direct = SdkRunner::new(complete_cfg(), None, None, None).with_max_tokens(Some(8192));

    // 同一 SessionOpen 下错误面 / 行为面一致（无装载缝续会话的启动校验面）
    assert_eq!(
        via_facade
            .open_session(open_continue("ses-max-tokens"))
            .expect_err("无装载缝续会话必须显式失败")
            .to_string(),
        via_direct
            .open_session(open_continue("ses-max-tokens"))
            .expect_err("直构同口径失败")
            .to_string(),
        "with_max_tokens 门面分发与直构一致（既有「与直构一致」对拍模式延伸）"
    );
    // 载荷不改变非相关行为：New 会话 open 照常成功（拒连端点，组装不触网）
    let session = via_facade
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: Path::new(r"C:\ws").to_path_buf(),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
                model_level: agent::ModelLevel::High,
            },
            session: agent::SessionRef::New,
            prior_handle: None,
        })
        .expect("完整配置 New 会话 open 应成功");
    drop(session);
}

#[test]
fn max_tokens与context_window两载荷互不覆盖_两序链同构() {
    // 顺序 A：先窗长后上限
    let forward = EngineFacade::new()
        .with_context_window(Some(200_000))
        .with_max_tokens(Some(8192));
    // 顺序 B：先上限后窗长
    let backward = EngineFacade::new()
        .with_max_tokens(Some(8192))
        .with_context_window(Some(200_000));

    for facade in [&forward, &backward] {
        assert_eq!(
            facade.context_window,
            Some(200_000),
            "窗长载荷不被上限 builder 覆盖"
        );
        assert_eq!(
            facade.max_tokens,
            Some(8192),
            "上限载荷不被窗长 builder 覆盖"
        );
    }

    // 两链分发行为一致（带后缀 / 无后缀两组配置分别对拍）
    for (high, low) in [
        ("claude-x[1m]".to_owned(), "claude-y".to_owned()),
        ("gpt-4o".to_owned(), "gpt-4o-mini".to_owned()),
    ] {
        let cfg = EngineConfig {
            model_high: high,
            model_low: low,
            ..complete_cfg()
        };
        let forward_runner = forward
            .runner_for(EngineKind::Sdk, cfg.clone())
            .expect("Sdk 臂构造成功");
        let backward_runner = backward
            .runner_for(EngineKind::Sdk, cfg)
            .expect("Sdk 臂构造成功");
        assert_eq!(
            forward_runner
                .open_session(open_continue("ses-two-payloads"))
                .expect_err("无装载缝续会话必须显式失败")
                .to_string(),
            backward_runner
                .open_session(open_continue("ses-two-payloads"))
                .expect_err("同口径失败")
                .to_string(),
            "两 builder 顺序不改变分发行为"
        );
    }
}

#[test]
fn 既有三条构造路径max_tokens缺省none不落缺省字面() {
    // 三条既有构造路径均不调 with_max_tokens → 载荷恒 None（初始化漏补即刻失败）
    assert_eq!(EngineFacade::new().max_tokens, None, "new() 缺省 None");
    let (loader, _calls) = counting_loader(Ok(None));
    assert_eq!(
        EngineFacade::with_resume_transcript(loader).max_tokens,
        None,
        "with_resume_transcript 缺省 None"
    );
    let (_dir, store) = temp_store("max-tokens-default");
    assert_eq!(
        EngineFacade::new()
            .with_explore_note_store(Arc::clone(&store))
            .max_tokens,
        None,
        "with_explore_note_store 缺省 None"
    );
    // 缺省档分发行为与现状一致（直构三参 None 同口径）
    let facade_runner = EngineFacade::new()
        .runner_for(EngineKind::Sdk, complete_cfg())
        .expect("Sdk 臂构造成功");
    let direct_runner = SdkRunner::new(complete_cfg(), None, None, None);
    assert_eq!(
        facade_runner
            .open_session(open_continue("ses-default-none"))
            .expect_err("无装载缝续会话必须显式失败")
            .to_string(),
        direct_runner
            .open_session(open_continue("ses-default-none"))
            .expect_err("直构同口径失败")
            .to_string(),
        "缺省 None 行为与现状一致"
    );
}

#[test]
fn with_max_tokens载荷三态原样透传且重复调用后者覆盖() {
    // 门面不做下限 / 上限校验（下限单点在命令层，DD6）：三态构造均可达
    for payload in [Some(0), Some(1), Some(u64::MAX), None] {
        let facade = EngineFacade::new().with_max_tokens(payload);
        assert_eq!(
            facade.max_tokens, payload,
            "载荷 {payload:?} 原样透传（门面零校验零夹取）"
        );
        // 可分发（构造可达）
        assert!(
            facade.runner_for(EngineKind::Sdk, complete_cfg()).is_ok(),
            "载荷 {payload:?} 下 Sdk 臂构造成功"
        );
    }

    // 重复调用 builder：后者覆盖前者（顺序语义）
    let overridden = EngineFacade::new()
        .with_max_tokens(Some(4096))
        .with_max_tokens(Some(32768));
    assert_eq!(overridden.max_tokens, Some(32768), "重复调用后者覆盖前者");
}

#[test]
fn runner_for_sdk臂启动校验成因集不因max_tokens载荷变化() {
    // 空配置（protocol 缺省 openai）→ ConfigMissing 三成因逐字，载荷不新增变体
    let runner = EngineFacade::new()
        .with_max_tokens(Some(8192))
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("Sdk 臂构造成功");
    let error = runner
        .open_session(open_continue("ses-max-tokens-validation"))
        .expect_err("空配置必须显式失败");
    let agent::AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    for field in ["api_key", "base_url", "model"] {
        assert!(
            message.contains(field),
            "成因集逐字为既有三档（缺 {field}），实际: {message}"
        );
    }
    assert!(
        !message.contains("max_tokens"),
        "上限载荷不进启动校验成因集，实际: {message}"
    );
}

#[test]
#[allow(clippy::type_complexity)]
fn runner_for结构化返回面锚定_引擎具体类型不出门面() {
    // 返回值仍为 `Result<Box<dyn AgentRunner>, String>`（引擎具体类型不出门面）
    let signature: fn(
        &EngineFacade,
        EngineKind,
        EngineConfig,
    ) -> Result<Box<dyn AgentRunner>, String> = EngineFacade::runner_for;
    let runner =
        signature(&EngineFacade::new(), EngineKind::Sdk, complete_cfg()).expect("Sdk 臂构造成功");
    assert!(
        runner.open_session(open_continue("ses-boxed")).is_err(),
        "Box<dyn AgentRunner> 经 trait 面可驱动"
    );
}

#[test]
fn crate根导出面锚定_后缀解析与下限常量经根可达() {
    // 命令层同一函数 / 同一常量消费的前提：crate 根再导出可达（破坏即编译失败）
    let parse: fn(&str) -> Result<crate::ModelSpec, crate::ModelSpecError> =
        crate::parse_model_spec;
    let spec = parse("claude-sonnet-4-5[1m]").expect("后缀解析经 crate 根可达");
    assert_eq!(spec.name, "claude-sonnet-4-5");
    assert_eq!(spec.length, Some(1_000_000));
    assert!(
        matches!(
            parse("x[2m]"),
            Err(crate::ModelSpecError { ref reason, .. }) if !reason.is_empty()
        ),
        "ModelSpecError 经 crate 根可达且拒收面一致"
    );
    assert_eq!(
        crate::MIN_OUTPUT_TOKENS,
        4_096,
        "保存期下限常量经 crate 根可达（与引擎同一门槛，无第二套字面量）"
    );
}

// ---------------------------------------------------------------------------
// 主仓 root 注入缝（EngineFacade::with_workspace_root）
//
// 门面只做透传：`runner_for` 的 sdk 臂以 `with_workspace_root` 交给
// `SdkRunner`，零额外语义；root 载荷在门面内不可读回，故断言面收敛为构造链
// / 分发等价 / 缺省零变 / Cli 臂零变（主仓口径的行为面由 `protect` 双根判定
// 用例承接）。
// ---------------------------------------------------------------------------

/// 主仓 root 临时目录装置。
fn facade_root(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("facade-write-protection-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败")
}

#[test]
fn with_workspace_root链式构造五序均可达() {
    let dir = facade_root("chain");
    let root = dir.path().to_path_buf();

    // 与既有四 builder 双向续链（builder 全序可换）
    let sequences: Vec<EngineFacade> = vec![
        EngineFacade::new().with_workspace_root(root.clone()),
        EngineFacade::new()
            .with_workspace_root(root.clone())
            .with_context_window(Some(200_000)),
        EngineFacade::new()
            .with_context_window(Some(200_000))
            .with_workspace_root(root.clone()),
        EngineFacade::new()
            .with_workspace_root(root.clone())
            .with_max_tokens(Some(8_192))
            .with_context_window(None),
        EngineFacade::with_resume_transcript(counting_loader(Ok(None)).0)
            .with_workspace_root(root.clone()),
    ];
    for facade in sequences {
        let runner = facade
            .runner_for(EngineKind::Sdk, complete_cfg())
            .expect("注入 root 后 Sdk 臂构造成功");
        // 产物经 trait 面可驱动（无装载缝续会话 → 显式失败不 panic）
        assert!(
            runner
                .open_session(open_continue("ses-root-chain"))
                .is_err(),
            "Sdk 臂产物为可驱动的 AgentRunner"
        );
    }

    let (_store_dir, store) = temp_store("root-chain");
    let with_store = EngineFacade::new()
        .with_explore_note_store(Arc::clone(&store))
        .with_workspace_root(root)
        .runner_for(EngineKind::Sdk, complete_cfg())
        .expect("store 与 root 两载荷续链后构造成功");
    drop(with_store);
}

#[test]
fn with_workspace_root分发等价_与直构runner同载荷行为一致() {
    let dir = facade_root("equivalent");
    let root = dir.path().to_path_buf();
    let via_facade = EngineFacade::new()
        .with_workspace_root(root.clone())
        .runner_for(EngineKind::Sdk, complete_cfg())
        .expect("Sdk 臂构造成功");
    let via_direct = SdkRunner::new(complete_cfg(), None, None, None).with_workspace_root(root);

    assert_eq!(
        via_facade
            .open_session(open_continue("ses-root-equivalent"))
            .expect_err("无装载缝续会话必须显式失败")
            .to_string(),
        via_direct
            .open_session(open_continue("ses-root-equivalent"))
            .expect_err("直构同口径失败")
            .to_string(),
        "门面只做透传（失败面逐字一致）"
    );
}

#[test]
fn with_workspace_root缺省零变_五构造点均不产出保护关闭语义() {
    let (_loader, _calls) = counting_loader(Ok(None));
    let (_store_dir, store) = temp_store("root-default");

    assert_eq!(EngineFacade::new().workspace_root, None, "new() 缺省 None");
    assert_eq!(
        EngineFacade::with_resume_transcript(counting_loader(Ok(None)).0).workspace_root,
        None,
        "with_resume_transcript 缺省 None"
    );
    assert_eq!(
        EngineFacade::new()
            .with_context_window(Some(1_000))
            .workspace_root,
        None,
        "with_context_window 不引入 root 载荷"
    );
    assert_eq!(
        EngineFacade::new()
            .with_max_tokens(Some(8_192))
            .workspace_root,
        None,
        "with_max_tokens 不引入 root 载荷"
    );
    assert_eq!(
        EngineFacade::new()
            .with_explore_note_store(Arc::clone(&store))
            .workspace_root,
        None,
        "with_explore_note_store 不引入 root 载荷"
    );

    // 缺省档分发行为与直构四参 None 同口径（None = 仅内置规则，非「保护关闭」）
    let facade_runner = EngineFacade::new()
        .runner_for(EngineKind::Sdk, complete_cfg())
        .expect("Sdk 臂构造成功");
    assert!(
        facade_runner
            .open_session(open_continue("ses-root-default"))
            .is_err(),
        "缺省 root 不改变既有失败面"
    );
}

#[test]
fn with_workspace_root边界形态门面不校验() {
    let long_root = std::path::PathBuf::from(format!("{}/中文🚀", "长".repeat(600)));
    for root in [
        std::path::PathBuf::new(),
        std::path::PathBuf::from("D:/__not_exists__/facade-root"),
        long_root,
    ] {
        let runner = EngineFacade::new()
            .with_workspace_root(root.clone())
            .runner_for(EngineKind::Sdk, complete_cfg())
            .unwrap_or_else(|error| panic!("root 形态 {root:?} 不校验（仍 Ok）: {error}"));
        drop(runner);
    }

    let dir = facade_root("emoji");
    let runner = EngineFacade::new()
        .with_workspace_root(dir.path().to_path_buf())
        .runner_for(EngineKind::Sdk, complete_cfg())
        .expect("含中文与 emoji 路径不校验（仍 Ok）");
    drop(runner);
}

#[test]
fn with_workspace_root不改变cli臂退役拒收面且装载缝零消费() {
    let dir = facade_root("cli-arm");
    let (loader, calls) = counting_loader(Ok(Some(Vec::new())));
    let error = match EngineFacade::with_resume_transcript(loader)
        .with_workspace_root(dir.path().to_path_buf())
        .runner_for(EngineKind::Cli, complete_cfg())
    {
        Err(error) => error,
        Ok(_) => panic!("Cli 臂必须显式 Err"),
    };
    assert_eq!(
        error, CLI_TENANT_RETIRED,
        "root 载荷不进退役臂，拒收文案逐字不变"
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "Cli 臂拒收于构造面，装载缝零消费"
    );
}
