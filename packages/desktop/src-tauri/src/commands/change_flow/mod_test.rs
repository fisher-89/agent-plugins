//! `commands::change_flow` 六命令的单元测试（test-design「change_flow/mod.rs
//! -> mod_test.rs」节）：前置校验三失败分支（W8 换血后）、start 提前 resolve
//! 与组合根装配、stop / answer / confirm / state / watch 五命令保持面、参数
//! 转换守卫。
//!
//! 装置沿 exec/mod_test.rs 既有先例：`#[tauri::command]` 保留原函数可直调，
//! 以 `tauri::test::mock_app()`（MockRuntime）manage 真实 `WorkspaceStores`
//! / `Arc<ChangeFlowControl>` / `Arc<StopRegistry>` 后直调 `*_with` 泛型测试
//! 缝；`tauri::ipc::Channel::new` 捕获回调收下 RunUpdate 信封（IPC 边界捕
//! 获）；前置校验 / 组合根以 tempdir 真实 change fixture 驱动（fs 进程边界
//! 真实组合）。CLI env 替身随回退出局——引擎可达性以 PATH 隔离驱动（空
//! PATH 下合成收敛，不 spawn 真实 claude）。
//!
//! 废弃注记（test-design 废弃行，断言不落）：「start 前置校验含 CLI 可发现
//! 项（env 指坏路径显式报错）」随 discover_cli / DEV_TEAM_PLUGIN_ROOT 出局
//! 退役——前置校验三道不再含 CLI 可发现分支。

use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::StopRegistry;
use orchestration::control::ChangeFlowControl;
use orchestration::state::{ChangeStepKind, ChangeStepState, ChangeStepStatus};
use store::{AgentEngineKind, AgentInstanceRecord, WorkspaceStores};

use super::{
    change_flow_answer_with, change_flow_confirm_with, change_flow_start_with,
    change_flow_state_with, change_flow_stop_with, change_flow_watch_with,
};

/// PATH 环境变量修改串行化（进程全局变量边界；与 exec/mod_test 的 PATH 隔离
/// 窗口共用 commands 级锁）。
use crate::commands::TEST_PATH_LOCK as PATH_LOCK;

// ---------------------------------------------------------------------------
// 装置：tempdir 双根 + mock app 托管三态
// ---------------------------------------------------------------------------

/// 数据根 + workspace 根临时环境（tempfile RAII）。
struct Env {
    data_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("change-flow-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("change-flow-cmd-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_dir, ws_root }
    }

    fn root(&self) -> String {
        self.ws_root.path().to_string_lossy().into_owned()
    }

    /// 预置 change 的 workflow.json（前置校验 / 组合根 fixture）。
    fn change(&self, name: &str, workflow_json: &str) {
        let dir = self.ws_root.path().join("openspec/changes").join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        fs::write(dir.join("workflow.json"), workflow_json).expect("写 workflow.json 失败");
    }
}

/// 以 MockRuntime 建测用 app：manage 真实 WorkspaceStores（打开 env 数据根的
/// 全局库）+ `Arc<ChangeFlowControl>`（run 控制注册表）+ `Arc<StopRegistry>`
///（内核治理面，须先于 start 托管）。
fn app_with(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app.manage(Arc::new(ChangeFlowControl::new()));
    app.manage(Arc::new(StopRegistry::new()));
    app
}

/// 丢弃型 Channel（发送信封即弃）。
fn discarding_channel() -> Channel<super::RunUpdate> {
    Channel::new(|_: InvokeResponseBody| Ok(()))
}

/// 捕获型 Channel：逐信封收下出线 JSON（IPC 边界捕获——首事件 / 终态观测）。
fn capturing_channel() -> (
    Channel<super::RunUpdate>,
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

/// 轮询至谓词成立（后台 walker 驱动的观测窗口；30s 上限防挂死）。
fn wait_for(what: &str, mut probe: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !probe() {
        assert!(Instant::now() < deadline, "等待{what}超时（30s）");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// 合法 requirement workflow.json（前置校验通过形态）。
const REQUIREMENT_WORKFLOW: &str = r#"{ "workflow_type": "requirement", "eval": [] }"#;

const CHANGE: &str = "flow-change";

// ---------------------------------------------------------------------------
// start 前置校验三失败分支（W8 换血后）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start前置校验三失败分支各自err且成因互不重合() {
    let env = Env::new("guard-branches");
    // 分支 2 的两类 fixture：不存在 change / workflow.json 不可解析
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    env.change("broken-change", "{ not valid json !!!");
    env.change(
        "bugfix-change",
        r#"{ "workflow_type": "bug-fix", "eval": [] }"#,
    );
    let root = env.root();

    let app = app_with(&env);

    // 分支 1：change 不存在（FsSnapshot detail miss）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "不存在的-change".to_owned(),
    )
    .await
    .expect_err("未知 change 应 Err");
    assert!(
        err.contains("不存在的-change") && err.contains("change 不存在"),
        "成因一（change miss）: {err}"
    );

    // 分支 2：workflow.json 不可解析（unparsable 旗标 → 显式 Err）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "broken-change".to_owned(),
    )
    .await
    .expect_err("不可解析应 Err");
    assert!(
        err.contains("broken-change") && err.contains("无法解析"),
        "成因二（unparsable）: {err}"
    );

    // 分支 3：workflow_type 非 requirement（写面相位表 None → 显式拒绝，W8）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root,
        "bugfix-change".to_owned(),
    )
    .await
    .expect_err("非 requirement 应 Err");
    assert!(
        err.contains("bug-fix") && err.contains("requirement"),
        "成因三（W8 分层出口）: {err}"
    );

    // 三成因互不重合且登记面零副作用（并行冲突分支另见「start同change并行」
    // 用例——本用例失败分支零 begin_run 登记）
    let control = app.state::<Arc<ChangeFlowControl>>();
    for name in ["不存在的-change", "broken-change", "bugfix-change"] {
        assert!(control.snapshot(name).is_none(), "失败分支零登记: {name}");
    }
}

// ---------------------------------------------------------------------------
// start 提前 resolve 与组合根装配（真实组合根，PATH 隔离驱动收敛）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start提前resolve返回running摘要且channel首事件到达后台驱动不阻塞() {
    let env = Env::new("early-resolve");
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    let app = app_with(&env);
    let root = env.root();

    // cli 默认实例 fixture（组合根缺省解析可走通）
    {
        let stores = app.state::<WorkspaceStores>();
        let id = stores
            .inner()
            .global()
            .upsert_agent_instance(AgentInstanceRecord::new(
                "组合根实例".to_owned(),
                AgentEngineKind::Cli,
                None,
            ))
            .expect("落 cli 实例 fixture")
            .id;
        stores
            .inner()
            .global()
            .set_default_agent_instance(id)
            .expect("置默认实例 fixture");
    }

    // PATH 隔离：executor 会话以 CliMissing 合成收敛（不 spawn 真实 claude）。
    // 隔离窗口覆盖至后台 turn 收敛（提前 resolve 后 walker 仍在驱动，窗口
    // 过早关闭会把真实 CLI 泄入 run）；窗口经共享 PATH 锁串行化
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let (channel, captured) = capturing_channel();
    let result = change_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
    )
    .await;

    // 提前 resolve 契约：run_id 立即可知、running 态（后台驱动不阻塞——
    // start 同步段即返回，运行态经 Channel 流出）
    let summary = result.expect("成功链路提前 resolve");
    assert!(
        summary.run_id.starts_with("run-"),
        "run-<millis> 铸造: {}",
        summary.run_id
    );
    assert_eq!(
        summary.status,
        super::ChangeRunStatus::Running,
        "提前 resolve 返回 running 态摘要"
    );
    // 订阅先行于 walker：run 登记即快照可见（重挂快照恢复输入面）
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert_eq!(
        control.snapshot(CHANGE).map(|snap| snap.run_id),
        Some(summary.run_id.clone())
    );

    // Channel 首事件到达（转发任务把 broadcast 信封送出）
    wait_for("Channel 首事件", || {
        !captured.lock().expect("捕获锁不可中毒").is_empty()
    });

    // 后台 walker 真实驱动：真实 LocalToolSteps 相位机步直调写面（fixture
    // workflow.json 被 phase-start 写入 active_phase），更新流含步状态与终态
    wait_for("Finished 信封", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "finished")
    });

    // 后台 turn 已收敛，隔离窗口可关闭（先恢复 PATH 再放锁）
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(_guard);
    let updates = captured.lock().expect("捕获锁不可中毒");
    assert!(
        updates.iter().any(|value| value["ipc"] == "step"),
        "步状态信封经 Channel 流出（AC-5 状态流信封）"
    );
    let finished = updates
        .iter()
        .find(|value| value["ipc"] == "finished")
        .expect("终态信封在场");
    assert_eq!(
        finished["status"], "failed",
        "PATH 隔离下 executor CliMissing → 合成收敛 failed（后台 walker 真实驱动）"
    );
    drop(updates);

    // 终态收口：注册表除名（run 仅进程内——AC-7 半边）
    wait_for("终态除名", || control.snapshot(CHANGE).is_none());

    // 组合根装配动态证据：phase-start 经进程内缝直调写面落盘
    let workflow_text = fs::read_to_string(
        env.ws_root
            .path()
            .join("openspec/changes")
            .join(CHANGE)
            .join("workflow.json"),
    )
    .expect("读 workflow.json 失败");
    let doc: serde_json::Value = serde_json::from_str(&workflow_text).expect("应可解析");
    assert!(
        doc.get("active_phase").is_some(),
        "组合根 LocalToolSteps 直调写面落盘（进程内缝证据）"
    );
}

#[tokio::test]
async fn start同change并行run冲突err() {
    let env = Env::new("parallel-conflict");
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    let app = app_with(&env);

    // 预登记同 change 的 run（前置校验第三分支：begin_run 冲突检测）
    let control = app.state::<Arc<ChangeFlowControl>>();
    let _guard = control
        .begin_run(CHANGE, "run-existing".to_owned())
        .expect("预登记应成功");

    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        env.root(),
        CHANGE.to_owned(),
    )
    .await
    .expect_err("并行发起应 Err");
    assert!(
        err.contains(CHANGE) && err.contains("已有运行中的 run"),
        "并行冲突记因: {err}"
    );
    // 既有 run 不受扰动
    assert_eq!(
        control.snapshot(CHANGE).map(|snap| snap.run_id),
        Some("run-existing".to_owned())
    );
}

// ---------------------------------------------------------------------------
// 五命令保持面：stop / answer / confirm / state / watch
// ---------------------------------------------------------------------------

#[test]
fn stop运行中置位且幂等忽略不报错() {
    let env = Env::new("stop");
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    let app = app_with(&env);
    let control = app.state::<Arc<ChangeFlowControl>>();

    // miss 幂等：非运行态 / 未知 change → Ok 不报错（AC-7 停止幂等半边）
    change_flow_stop_with(app.handle().clone(), env.root(), CHANGE.to_owned())
        .expect("miss 幂等应 Ok");

    // 运行中置位：Ok 且注册表 cancelled 置位（经 guard.cancelled 观测）
    let guard = control
        .begin_run(CHANGE, "run-1".to_owned())
        .expect("登记应成功");
    change_flow_stop_with(app.handle().clone(), env.root(), CHANGE.to_owned())
        .expect("运行中 stop 应 Ok");
    assert!(guard.cancelled(), "取消信号同步观测");

    // 重复 stop：Ok 幂等（停止寻址不重复报错）
    change_flow_stop_with(app.handle().clone(), env.root(), CHANGE.to_owned())
        .expect("重复 stop 幂等 Ok");
}

#[test]
fn answer与confirm无等待方时err透传() {
    let env = Env::new("pending");
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    let app = app_with(&env);

    // 无运行 run：Err 携 change 名（单次通道错误面透传）
    let err = change_flow_answer_with(
        app.handle().clone(),
        env.root(),
        CHANGE.to_owned(),
        "应答".to_owned(),
    )
    .expect_err("无 run 应 Err");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");
    let err = change_flow_confirm_with(app.handle().clone(), env.root(), CHANGE.to_owned(), true)
        .expect_err("无 run 应 Err");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");

    // 有 run 无挂起：Err「当前无等待」
    let control = app.state::<Arc<ChangeFlowControl>>();
    let _guard = control
        .begin_run(CHANGE, "run-1".to_owned())
        .expect("登记应成功");
    let err = change_flow_answer_with(
        app.handle().clone(),
        env.root(),
        CHANGE.to_owned(),
        "应答".to_owned(),
    )
    .expect_err("无挂起 ask 应 Err");
    assert_eq!(err, "当前无等待中的 ask");
    let err = change_flow_confirm_with(app.handle().clone(), env.root(), CHANGE.to_owned(), true)
        .expect_err("无挂起确认应 Err");
    assert_eq!(err, "当前无等待中的 phase 确认");
}

#[test]
fn state快照查询运行中some_无run与终态后none() {
    let env = Env::new("state");
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    let app = app_with(&env);
    let root = env.root();

    // 无 run → None（重挂恢复输入面）
    let state = change_flow_state_with(app.handle().clone(), root.clone(), CHANGE.to_owned())
        .expect("查询应成功");
    assert_eq!(state, None);

    // 运行中 → Some(ChangeRunSnapshot)（状态机镜像）
    let control = app.state::<Arc<ChangeFlowControl>>();
    let guard = control
        .begin_run(CHANGE, "run-1".to_owned())
        .expect("登记应成功");
    guard.emit(super::RunUpdate::Step {
        step: ChangeStepState {
            phase: "proposal".to_owned(),
            attempt: 1,
            step: ChangeStepKind::Executor,
            status: ChangeStepStatus::Running,
            session_id: None,
            detail: None,
        },
    });
    let state = change_flow_state_with(app.handle().clone(), root.clone(), CHANGE.to_owned())
        .expect("查询应成功")
        .expect("运行中应有快照");
    assert_eq!(state.run_id, "run-1");
    assert_eq!(state.status, super::ChangeRunStatus::Running);
    assert_eq!(state.phase.as_deref(), Some("proposal"));

    // 终态收口 → None（除名，图回落派生规则）
    guard.finish(super::ChangeRunStatus::Stopped, None);
    let state =
        change_flow_state_with(app.handle().clone(), root, CHANGE.to_owned()).expect("查询应成功");
    assert_eq!(state, None, "终态除名 → None");
}

#[test]
fn watch补订运行中接收后续信封_无run时ok非错误() {
    let env = Env::new("watch");
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    let app = app_with(&env);
    let root = env.root();

    // 无 run：Ok（非错误——重挂时 run 可能已收口，图回落派生规则）
    change_flow_watch_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        CHANGE.to_owned(),
    )
    .expect("无 run 补订应 Ok");

    // 运行中补订：后续信封经 Channel 到达（重挂补订）
    let control = app.state::<Arc<ChangeFlowControl>>();
    let guard = control
        .begin_run(CHANGE, "run-1".to_owned())
        .expect("登记应成功");
    let (channel, captured) = capturing_channel();
    change_flow_watch_with(app.handle().clone(), channel, root, CHANGE.to_owned())
        .expect("补订应成功");
    guard.emit(super::RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
    });
    wait_for("补订信封", || {
        !captured.lock().expect("捕获锁不可中毒").is_empty()
    });
    assert_eq!(
        captured.lock().expect("捕获锁不可中毒")[0]["ipc"],
        serde_json::json!("confirmWait"),
        "RunUpdate 信封经补订 Channel 到达"
    );
}

// ---------------------------------------------------------------------------
// 参数转换守卫：blank root（root 寻址与 `Result<T, String>` 模板保留）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 参数转换守卫blank_root各命令模板保留() {
    let env = Env::new("blank-root");
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    let app = app_with(&env);
    let blank = "   ".to_owned();

    // start：Err（无 cwd 无从发起）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        blank.clone(),
        CHANGE.to_owned(),
    )
    .await
    .expect_err("blank root start 应 Err");
    assert!(err.contains("root"), "守卫 Err 文案: {err}");

    // answer / confirm：Err
    let err = change_flow_answer_with(
        app.handle().clone(),
        blank.clone(),
        CHANGE.to_owned(),
        "应答".to_owned(),
    )
    .expect_err("blank root answer 应 Err");
    assert!(err.contains("root"), "守卫 Err 文案: {err}");
    let err =
        change_flow_confirm_with(app.handle().clone(), blank.clone(), CHANGE.to_owned(), true)
            .expect_err("blank root confirm 应 Err");
    assert!(err.contains("root"), "守卫 Err 文案: {err}");

    // state：Ok(None)；stop：Ok（miss 幂等）
    let state = change_flow_state_with(app.handle().clone(), blank.clone(), CHANGE.to_owned())
        .expect("blank root state 应 Ok(None)");
    assert_eq!(state, None);
    change_flow_stop_with(app.handle().clone(), blank, CHANGE.to_owned())
        .expect("blank root stop 幂等 Ok");
}

/// blank change 守卫六缝各就位：start / answer / confirm → Err（无 change
/// 无从寻址），stop / watch → Ok 幂等无副作用，state → Ok(None)——六命令
/// 的 change 参数守卫全量覆盖（root 合法、仅 change 空白，隔离 change 分支）。
#[tokio::test]
async fn 参数转换守卫blank_change六缝各就位() {
    let env = Env::new("blank-change");
    env.change(CHANGE, REQUIREMENT_WORKFLOW);
    let app = app_with(&env);
    let root = env.root();
    let blank = "   ".to_owned();

    // start：Err（无 change 无从发起）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        blank.clone(),
    )
    .await
    .expect_err("blank change start 应 Err");
    assert!(
        err.contains("change") && !err.contains("root"),
        "守卫 Err 记因 change 分支: {err}"
    );

    // answer / confirm：Err
    let err = change_flow_answer_with(
        app.handle().clone(),
        root.clone(),
        blank.clone(),
        "应答".to_owned(),
    )
    .expect_err("blank change answer 应 Err");
    assert!(err.contains("change"), "守卫 Err 文案: {err}");
    let err = change_flow_confirm_with(app.handle().clone(), root.clone(), blank.clone(), true)
        .expect_err("blank change confirm 应 Err");
    assert!(err.contains("change"), "守卫 Err 文案: {err}");

    // stop / watch：Ok 幂等无副作用（blank 不进入注册表链路）
    change_flow_stop_with(app.handle().clone(), root.clone(), blank.clone())
        .expect("blank change stop 幂等 Ok");
    change_flow_watch_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        blank.clone(),
    )
    .expect("blank change watch Ok 非错误");

    // state：Ok(None)
    let state = change_flow_state_with(app.handle().clone(), root, blank)
        .expect("blank change state 应 Ok(None)");
    assert_eq!(state, None);

    // 守卫先行于前置校验：合法 root 在位而 change 空白 → 零登记零 spawn 副作用
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert!(control.snapshot(CHANGE).is_none(), "守卫分支零登记");
}
