use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::sdk::runner::SdkRunner;
use crate::{
    compose_turn, session_query, ClaudeCliRunner, ComposedTurn, EngineConfig, EngineFacade,
    EngineKind, ResumeTranscript,
};
use agent::AgentRunner;
use store::WorkspaceStores;

/// PATH 环境变量修改串行化（进程全局操作）。
static PATH_LOCK: Mutex<()> = Mutex::new(());

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
fn crate根导出面锚定_组合根与查询面与既有门面re_export可达() {
    // compose_turn / ComposedTurn 以签名锚定（零调用；store 半边 = 命令层
    // 预解析注入的 workspace root 库实例，design D8 拆参）
    let compose_signature: fn(
        &WorkspaceStores,
        Arc<agent::StopRegistry>,
        Arc<store::Store>,
        Option<i64>,
    ) -> Result<ComposedTurn, String> = compose_turn;
    let _ = compose_signature;

    // session_query（store_port 模块经 crate 根可达）：签名锚定
    let query_signature: fn(Arc<store::Store>) -> Arc<dyn agent::SessionQuery> = session_query;
    let _ = query_signature;

    // 既有门面 re-export 零 diff 回归：EngineKind / EngineConfig /
    // ClaudeCliRunner / cli 模块别名
    let cli_runner = ClaudeCliRunner::new();
    assert_eq!(std::mem::size_of_val(&cli_runner), 0, "无状态 CLI runner");

    // EngineConfig 三字段零改动承诺回归（构造形态与 empty 占位）
    let config = EngineConfig {
        api_key: "k".to_owned(),
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        model: "m".to_owned(),
    };
    assert!(!config.api_key.is_empty());
    let empty = EngineConfig::empty();
    assert!(empty.api_key.is_empty() && empty.base_url.is_empty() && empty.model.is_empty());

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
// ResumeTranscript 缝语义平移（类型形状不变 + CLI 臂持有不消费）
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

#[tokio::test]
async fn cli分发路径持有装载缝不消费_调用计数恒零() {
    let (loader, calls) = counting_loader(Ok(Some(Vec::new())));
    let facade = EngineFacade::with_resume_transcript(loader);

    // CLI 臂：open_session（持有缝不消费）→ ask（spawn_turn 不读转录，续会话
    // 走 --resume flag）——装载缝调用计数恒零
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let dir = tempfile::Builder::new()
        .prefix("facade-cli-resume-")
        .tempdir()
        .expect("创建合成目录失败");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", ""); // ask 段 discover 失败（不触装载缝）

    let runner = facade.runner_for(EngineKind::Cli, EngineConfig::empty());
    let mut session = runner
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: dir.path().to_path_buf(),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
            },
            session: agent::SessionRef::Continue {
                id: "ses-cli-1".to_owned(),
            },
            prior_handle: Some("s-legacy-1".to_owned()),
        })
        .expect("CLI 臂 open 段无 IO");
    let _ = session
        .questions
        .send(agent::TurnQuestion {
            prompt: "CLI 续会话一轮".to_owned(),
        })
        .await;
    while session.observations.recv().await.is_some() {}

    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "CLI 引擎持有装载缝不消费（调用计数恒零回归）"
    );
}

// ---------------------------------------------------------------------------
// 门面双臂分发回归
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cli臂隔离path下ask段以cli_missing合成收敛() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let dir = tempfile::Builder::new()
        .prefix("facade-cli-missing-")
        .tempdir()
        .expect("创建合成目录失败");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let runner = EngineFacade::new().runner_for(EngineKind::Cli, EngineConfig::empty());
    let mut session = runner
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: dir.path().to_path_buf(),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
            },
            session: agent::SessionRef::New,
            prior_handle: None,
        })
        .expect("CLI 臂 open 段无 IO");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "隔离 PATH 下的一轮".to_owned(),
        })
        .await
        .expect("问题送达");
    let mut events = Vec::new();
    while let Some(event) = session.observations.recv().await {
        events.push(event);
    }

    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    // 隔离 PATH：ask 段 discover 失败 → CliMissing 形态的合成收敛（engine 分发
    // 住门面后 CLI 租户行为面零回归）
    assert_eq!(events.len(), 1, "恰一条合成收敛");
    assert!(matches!(
        &events[0],
        agent::AgentEventKind::TurnDone { subtype, is_error: true, .. }
            if subtype == "error_cli_missing"
    ));
}

#[test]
fn sdk臂空配置open以config_missing显式失败() {
    let runner = EngineFacade::new().runner_for(EngineKind::Sdk, EngineConfig::empty());
    let error = runner
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: Path::new("C:\\ws").to_path_buf(),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
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
        .runner_for(EngineKind::Sdk, EngineConfig::empty());
    let via_direct = SdkRunner::new(EngineConfig::empty(), None, Some(200_000));

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
        .runner_for(EngineKind::Sdk, EngineConfig::empty());
    let none_direct = SdkRunner::new(EngineConfig::empty(), None, None);
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
    let via_facade =
        EngineFacade::with_resume_transcript(loader).runner_for(EngineKind::Sdk, complete_cfg());
    let (direct_loader, _direct_calls) = counting_loader(Ok(None));
    let via_direct = SdkRunner::new(complete_cfg(), Some(direct_loader), None);

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
        api_key: "sk-test".to_owned(),
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        model: "rig-test-model".to_owned(),
    }
}
