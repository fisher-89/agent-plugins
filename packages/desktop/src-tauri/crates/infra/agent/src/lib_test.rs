//! `EngineFacade` 与 `EngineKind` 的单元测试（AC-1 / AC-3 / AC-8 的门面半边）：
//! 引擎二值线格式（serde camelCase `"cli" | "sdk"`、清单外值拒绝、specta
//! 收集面）、`runner_for` 引擎构造唯一 match 点的双臂分发（CLI 臂隔离 PATH
//! 下 CliMissing、SDK 臂空配置启动校验即失败）、分发纯度与 trait object
//! 形态、`with_resume_transcript` 装载缝持有不消费（CLI 路径）、crate 根
//! 对外形状锚定（re-export 破坏即编译失败）。
//!
//! PATH 隔离以共享互斥锁串行化（沿 cli/discover_test 的 PATH_LOCK 先例）；
//! 装载缝以闭包替身注入（内存三形态）；sdk 分发臂以真实 SdkRunner 空配置
//! 走启动校验路径即失败，不触 rig 网络。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use agent::{AgentPermissionMode, AgentRunParams, AgentRunner, AgentStartError};

use crate::{EngineConfig, EngineFacade, EngineKind};

/// PATH 环境变量修改串行化（同进程测试并行跑，set_var 为进程全局操作）。
static PATH_LOCK: Mutex<()> = Mutex::new(());

fn params(cwd: &Path) -> AgentRunParams {
    AgentRunParams {
        prompt: "门面分发轮".to_owned(),
        cwd: cwd.to_path_buf(),
        permission_mode: AgentPermissionMode::BypassPermissions,
        resume_session_id: None,
    }
}

fn empty_config() -> EngineConfig {
    // 空缺省占位构造源：from_hardcoded_slot() 退役后改写为 empty()（断言面
    // 零变化——三字段全空的占位形态由 config_test 锁定）
    EngineConfig::empty()
}

// ---------------------------------------------------------------------------
// EngineKind 线格式（正向 + 边界）
// ---------------------------------------------------------------------------

#[test]
fn engine_kind线格式逐字为cli与sdk且roundtrip一致() {
    for (kind, expected) in [(EngineKind::Cli, "cli"), (EngineKind::Sdk, "sdk")] {
        let serialized = serde_json::to_string(&kind).expect("序列化成功");
        assert_eq!(serialized, format!("\"{expected}\""), "线格式逐字一致");
        let roundtrip: EngineKind = serde_json::from_str(&serialized).expect("反序列化成功");
        assert_eq!(roundtrip, kind, "roundtrip 一致");
    }
}

#[test]
fn engine_kind清单外值反序列化一律err值域受控() {
    // 大小写变体 / 空白尾随 / 空串 / 清单外词：不静默兜底
    for invalid in ["Cli", "SDK", "cli ", " sdk", "", "cli-sdk", "rig"] {
        let result = serde_json::from_str::<EngineKind>(invalid);
        assert!(result.is_err(), "engine 清单外值 {invalid:?} 必须 Err");
    }
}

#[test]
fn engine_kind具specta收集面可出线() {
    // specta::Type derive（AC-3 bindings 再生成含 engine 参数的类型镜像前提）：
    // 收集面可触达（定义可取），破坏即编译失败
    fn assert_specta_type<T: specta::Type>() {}
    assert_specta_type::<EngineKind>();
}

// ---------------------------------------------------------------------------
// EngineFacade.runner_for：引擎构造唯一 match 点的双臂分发
// ---------------------------------------------------------------------------

#[test]
fn runner_for_cli臂隔离path下start为cli_missing且engine_cfg不被消费() {
    let _guard = PATH_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let empty_path = tempfile::tempdir().expect("创建空 PATH 目录失败");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", empty_path.path());
    // 引擎构造（CLI 臂零成本）在锁内同段完成：分发本身不读 env
    let runner = EngineFacade::new().runner_for(EngineKind::Cli, empty_config());
    let result = runner.start(params(Path::new("C:\\ws")));
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let error = result.expect_err("隔离 PATH 下 CLI 不可发现");
    let AgentStartError::CliMissing(message) = &error else {
        panic!("CLI 臂失败形态为 CliMissing，实际: {error:?}");
    };
    assert!(!message.is_empty(), "CliMissing 携带原因串: {message}");
    // engine_cfg 不被消费（CLI 路径不读配置）：传空缺省配置不影响失败形态
    assert!(matches!(error, AgentStartError::CliMissing(_)));
}

#[tokio::test]
async fn runner_for_sdk臂空缺省配置start返回config_missing且不产生任何事件() {
    // SDK 臂以真实 SdkRunner 走启动校验路径即失败（不触 rig 网络）；
    // 缺省收敛改由命令层解析单点承载后，该分发路径覆盖解析产物为空配置的
    // sdk 臂，门面自身零缺省逻辑（恒收显式 EngineKind）
    let runner = EngineFacade::new().runner_for(EngineKind::Sdk, empty_config());
    let error = runner
        .start(params(Path::new("C:\\ws")))
        .expect_err("空缺省配置未手填时启动显式失败");
    assert!(
        matches!(error, AgentStartError::ConfigMissing(ref message) if message.contains("api_key")),
        "AC-2 变体的首个消费点（启动校验区分成因），实际: {error:?}"
    );
    // 不产生任何事件：start Err 无事件流产出（失败在通道构造之前）
    assert!(matches!(error, AgentStartError::ConfigMissing(_)));
}

// ---------------------------------------------------------------------------
// 分发纯度与 trait object 形态（边界）
// ---------------------------------------------------------------------------

#[test]
fn 同一门面重复分发与多实例分发互不共享运行态() {
    // 同一门面重复分发：每次返回独立 runner（SDK 臂各自持配置与装载缝）
    let facade = EngineFacade::new();
    let first = facade.runner_for(EngineKind::Sdk, empty_config());
    let second = facade.runner_for(EngineKind::Sdk, empty_config());
    let first_error = first.start(params(Path::new("C:\\ws"))).expect_err("首 runner 失败");
    let second_error = second.start(params(Path::new("C:\\ws"))).expect_err("次 runner 失败");
    assert_eq!(first_error, second_error, "互不共享运行态：同输入同独立错误形态");

    // 多实例分发：不同门面互不串线
    let other = EngineFacade::new();
    let other_error = other
        .runner_for(EngineKind::Sdk, empty_config())
        .start(params(Path::new("C:\\ws")))
        .expect_err("他实例同样失败");
    assert_eq!(other_error, first_error, "实例间零共享");
}

#[test]
fn runner_for产物为trait_object形态可注入编排() {
    // `Box<dyn AgentRunner>`：引擎具体类型不出门面的编译期锚定
    let boxed: Box<dyn AgentRunner> =
        EngineFacade::new().runner_for(EngineKind::Sdk, empty_config());
    let error = boxed.start(params(Path::new("C:\\ws")));
    assert!(matches!(error, Err(AgentStartError::ConfigMissing(_))));

    // Box<dyn AgentRunner> 可再转 &dyn AgentRunner 注入编排缝（app 层
    // start_agent_run_with 的 runner 参数形态）
    let boxed: Box<dyn AgentRunner> =
        EngineFacade::new().runner_for(EngineKind::Cli, empty_config());
    let as_dyn: &dyn AgentRunner = boxed.as_ref();
    let _ = as_dyn; // 形态编译期锚定（实际启动见 PATH 隔离用例）
}

// ---------------------------------------------------------------------------
// EngineFacade::new / with_resume_transcript（正向）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn new门面可发起cli运行且with_resume_transcript装载缝持有不消费() {
    let _guard = PATH_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let empty_path = tempfile::tempdir().expect("创建空 PATH 目录失败");

    // 装载缝替身：按 session_id 返回内存转录 / None / Err 三形态的调用计数
    // （CLI 分发路径不消费 loader——计数恒零即「持有不消费」的观测证据）
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let loader: crate::ResumeTranscript = Arc::new(move |session_id: &str| {
        counter.fetch_add(1, Ordering::SeqCst);
        let _ = session_id;
        Ok(Some(Vec::new()))
    });

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", empty_path.path());
    let facade = EngineFacade::with_resume_transcript(loader);
    let result = facade
        .runner_for(EngineKind::Cli, empty_config())
        .start(params(Path::new("C:\\ws")));
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let error = result.expect_err("隔离 PATH 下 CLI 不可发现");
    assert!(matches!(error, AgentStartError::CliMissing(_)));
    // loader 为引擎中立数据：CLI 分发路径发起行为不受 loader 影响（持有不消费）
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "CLI 路径不消费转录装载缝"
    );

    // new() 门面（无 resume 注入）同路径可发起（CLI 路径零成本）
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", empty_path.path());
    let plain = EngineFacade::new()
        .runner_for(EngineKind::Cli, empty_config())
        .start(params(Path::new("C:\\ws")));
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    assert!(matches!(plain, Err(AgentStartError::CliMissing(_))));
}

// ---------------------------------------------------------------------------
// 门面 re-export 对外形状（AC-1 平移后 crate 根可达性，编译期锚定）
// ---------------------------------------------------------------------------

#[test]
fn crate根对外形状锚定_平移模块经根可达() {
    // cli/ 四实现文件自 src/ 平移落位后，对外形状经 crate 根可达：
    // discover / build_args / normalize_line / ClaudeCliRunner / EngineConfig
    // （用例内使用锚定，任一 re-export 破坏即编译失败）
    let runner: crate::ClaudeCliRunner = crate::ClaudeCliRunner::new();
    let _ = runner;
    let config: EngineConfig = EngineConfig::empty();
    let _ = config;
    fn build_args_shape(params: &AgentRunParams) -> Vec<String> {
        crate::flags::build_args(params)
    }
    fn normalize_line_shape(line: &str) -> Option<agent::AgentEventKind> {
        crate::jsonl::normalize_line(line)
    }
    fn discover_shape() -> Result<PathBuf, AgentStartError> {
        crate::discover::discover()
    }
    let _ = (build_args_shape, normalize_line_shape, discover_shape);

    // 类型别名可达（编译期即锚定 re-export 存续）
    let _cwd: PathBuf = PathBuf::from("C:\\ws");
}
