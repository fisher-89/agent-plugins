use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::StopRegistry;
use orchestration::control::ChangeFlowControl;
use orchestration::state::{ChangeStepKind, ChangeStepState, ChangeStepStatus};
use store::{AgentEngineKind, AgentInstanceRecord, WorkspaceStores};
use workflow::state::{ChangeStateRecord, ChangeStatus};

use super::{
    change_flow_answer_with, change_flow_confirm_with, change_flow_start_with,
    change_flow_state_with, change_flow_stop_with, change_flow_watch_with,
};

/// PATH 环境变量修改串行化（进程全局变量边界；与 exec/mod_test 的 PATH 隔离
/// 窗口共用 commands 级锁）。
use crate::commands::TEST_PATH_LOCK as PATH_LOCK;

// ---------------------------------------------------------------------------
// 装置：tempdir 双根 + mock app 托管三态 + db 建档种子
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

    /// 预置 change 的磁盘目录（建档记录由 [`seed_change`] 经 db 落，双载体
    /// 真实组合；存量 CLI 形态用例则只建目录不建档）。
    fn change_dir(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join("openspec/changes").join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        dir
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

/// db 建档种子（前置校验 1/2 的正向与反相 fixture）：经 store change 域操作
/// 面落 `ChangeRecord`（workflow_type 决定相位表校验走向），返回 workspace 库
/// 实例供落库证据断言复用。
fn seed_change(app: &App<tauri::test::MockRuntime>, root: &str, name: &str, workflow_type: &str) {
    app.state::<WorkspaceStores>()
        .for_root(root)
        .expect("for_root 应成功")
        .create_change_record(ChangeStateRecord {
            name: name.to_owned(),
            workflow_type: workflow_type.to_owned(),
            created_at: 1727000000000,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
        })
        .expect("建档种子应成功");
}

/// cli 默认实例 fixture（组合根缺省解析可走通——发起链路真实驱动的用例共用）。
fn seed_cli_default_instance(app: &App<tauri::test::MockRuntime>) {
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

/// PATH 隔离窗口开启（executor 会话以 CliMissing 合成收敛，不 spawn 真实
/// claude）：返回原值供窗口关闭时恢复；窗口经 commands 级 PATH 锁串行化。
fn isolate_path() -> (
    std::sync::MutexGuard<'static, ()>,
    Option<std::ffi::OsString>,
) {
    let guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    (guard, original)
}

/// PATH 隔离窗口关闭（先恢复原值再放锁——窗口过早关闭会把真实 CLI 泄入 run）。
fn restore_path(guard: std::sync::MutexGuard<'static, ()>, original: Option<std::ffi::OsString>) {
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(guard);
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

const CHANGE: &str = "flow-change";

// ---------------------------------------------------------------------------
// start 前置校验（db 建档校验）：无建档拒绝 / 相位表校验（W8 语义平移）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start前置校验无建档与相位表缺失各自err且成因互不重合() {
    let env = Env::new("guard-branches");
    let root = env.root();
    // 无建档 fixture：存量 CLI change 形态（workflow.json 在场、db 零记录——
    // 双向墙用户路径：文档形态 change 不可运行）
    let legacy_dir = env.change_dir("legacy-cli-change");
    fs::write(
        legacy_dir.join("workflow.json"),
        r#"{ "workflow_type": "requirement", "eval": [] }"#,
    )
    .expect("写 workflow.json 失败");
    // 相位表 fixture：db 有档但 workflow_type 无相位表（bug-fix）
    let app = app_with(&env);
    seed_change(&app, &root, "bugfix-change", "bug-fix");

    // 前置校验 1：db 无建档记录（存量 CLI change）→ 显式 Err 携 change 名
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "legacy-cli-change".to_owned(),
        false,
    )
    .await
    .expect_err("无建档应 Err");
    assert!(
        err.contains("legacy-cli-change") && err.contains("未建档"),
        "成因一（db 缺 ChangeRecord）: {err}"
    );

    // 前置校验 2：workflow_type 无相位表（phase_table None → 显式拒绝）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root,
        "bugfix-change".to_owned(),
        false,
    )
    .await
    .expect_err("非 requirement 应 Err");
    assert!(
        err.contains("bugfix-change") && err.contains("bug-fix") && err.contains("requirement"),
        "成因二（相位表缺失）: {err}"
    );

    // 两成因互不重合且登记面零副作用（并行冲突分支另见「start同change并行」
    // 用例——本用例失败分支零 begin_run 登记）
    let control = app.state::<Arc<ChangeFlowControl>>();
    for name in ["legacy-cli-change", "bugfix-change"] {
        assert!(control.snapshot(name).is_none(), "失败分支零登记: {name}");
    }
}

// ---------------------------------------------------------------------------
// start 提前 resolve 与组合根装配（db 种子，PATH 隔离驱动收敛）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start提前resolve返回running摘要且channel首事件到达后台驱动不阻塞() {
    let env = Env::new("early-resolve");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");

    // cli 默认实例 fixture（组合根缺省解析可走通）
    seed_cli_default_instance(&app);

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
        false,
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

    // 后台 walker 真实驱动：真实 LocalToolSteps 相位机步直调写面（phase-start
    // 经 port 落库 active_phase），更新流含步状态与终态
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

    // 组合根装配动态证据：phase-start 经进程内缝直调写面落库（db active_phase
    // 在位——写面经 ChangeStateStore port，executor 失败前已开相未落账）
    let record = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功")
        .find_change_record(CHANGE)
        .expect("查档应成功")
        .expect("建档在案");
    let active = record
        .active_phase
        .expect("phase-start 落库证据（active_phase 在位——组合根 LocalToolSteps 直调写面经 port）");
    assert_eq!(active.phase, "proposal", "建档零相位行 → 首相位开相");
    assert_eq!(active.attempt, 1, "attempt 事务内推导（零条目 + 1）");
}

#[tokio::test]
async fn start同change并行run冲突err() {
    let env = Env::new("parallel-conflict");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");

    // 预登记同 change 的 run（前置校验第三分支：begin_run 冲突检测）
    let control = app.state::<Arc<ChangeFlowControl>>();
    let _guard = control
        .begin_run(CHANGE, "run-existing".to_owned())
        .expect("预登记应成功");

    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root,
        CHANGE.to_owned(),
        false,
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
// auto_next_phase 透传受理（AC-3：参数面受理与透传布线——auto 与手动的行为分叉
// 语义直测归 orchestration walker_test，本节证明新尾参不破命令面受理契约）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auto_next_phase_true透传受理零confirmwait照常后台收敛() {
    let env = Env::new("auto-true");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");
    seed_cli_default_instance(&app);

    // PATH 隔离：executor 会话以 CliMissing 合成收敛（窗口覆盖至后台 turn 收敛）
    let (path_guard, original) = isolate_path();

    let (channel, captured) = capturing_channel();
    let result = change_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
        true,
    )
    .await;

    // 受理契约：run_id 立即可知、running 态摘要（后台驱动不阻塞）
    let summary = result.expect("auto_next_phase=true 合法受理");
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

    // 订阅先行于 walker：run 登记即快照可见
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert_eq!(
        control.snapshot(CHANGE).map(|snap| snap.run_id),
        Some(summary.run_id.clone())
    );

    // 后台 walker 真实驱动至收敛（PATH 隔离下 executor CliMissing → failed）
    wait_for("Finished 信封", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "finished")
    });

    // 后台 turn 已收敛，隔离窗口可关闭
    restore_path(path_guard, original);

    // auto 参数达 walker：更新流全程零 confirmWait 信封
    let updates = captured.lock().expect("捕获锁不可中毒");
    assert!(
        updates
            .iter()
            .all(|value| value["ipc"] != serde_json::json!("confirmWait")),
        "auto 模式更新流零 confirmWait 信封: {:?}",
        updates
            .iter()
            .map(|value| value["ipc"].clone())
            .collect::<Vec<_>>()
    );
    let finished = updates
        .iter()
        .find(|value| value["ipc"] == "finished")
        .expect("终态信封在场");
    assert_eq!(
        finished["status"], "failed",
        "PATH 隔离下 executor CliMissing → 合成收敛 failed"
    );
    drop(updates);

    // 终态收口除名 + phase-start 落库证据（组合根装配照常，写面经 port 落 db）
    wait_for("终态除名", || control.snapshot(CHANGE).is_none());
    let record = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功")
        .find_change_record(CHANGE)
        .expect("查档应成功")
        .expect("建档在案");
    assert!(
        record.active_phase.is_some(),
        "phase-start 落库证据在场（组合根 LocalToolSteps 直调写面经 port）"
    );
}

#[tokio::test]
async fn auto_next_phase_false默认档受理面回归() {
    let env = Env::new("auto-false");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");
    seed_cli_default_instance(&app);

    let (path_guard, original) = isolate_path();

    let (channel, captured) = capturing_channel();
    let result = change_flow_start_with(
        app.handle().clone(),
        channel,
        root,
        CHANGE.to_owned(),
        false,
    )
    .await;

    // 受理契约零变更：run- 前缀摘要 + running 态
    let summary = result.expect("默认档合法受理");
    assert!(summary.run_id.starts_with("run-"));
    assert_eq!(summary.status, super::ChangeRunStatus::Running);
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert_eq!(
        control.snapshot(CHANGE).map(|snap| snap.run_id),
        Some(summary.run_id.clone()),
        "begin_run 登记 + 订阅先行零变更"
    );

    // 订阅先行与后台驱动时序零变更：首事件先行、终态经 Channel 流出
    wait_for("Channel 首事件", || {
        !captured.lock().expect("捕获锁不可中毒").is_empty()
    });
    wait_for("Finished 信封", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "finished")
    });

    restore_path(path_guard, original);

    // 终态面语义不变：合成收敛 failed（步状态 + 终态信封照常流出）
    let updates = captured.lock().expect("捕获锁不可中毒");
    assert!(
        updates.iter().any(|value| value["ipc"] == "step"),
        "步状态信封照常流出"
    );
    let finished = updates
        .iter()
        .find(|value| value["ipc"] == "finished")
        .expect("终态信封在场");
    assert_eq!(
        finished["status"], "failed",
        "PATH 隔离下合成收敛 failed（终态面语义不变）"
    );
    drop(updates);

    wait_for("终态除名", || control.snapshot(CHANGE).is_none());
}

#[tokio::test]
async fn auto_next_phase_true不绕过守卫与前置校验() {
    let env = Env::new("auto-guard");
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");

    // blank root + true：Err（blank 守卫先行）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        "   ".to_owned(),
        CHANGE.to_owned(),
        true,
    )
    .await
    .expect_err("blank root start 应 Err");
    assert!(err.contains("root"), "blank 守卫 Err 文案: {err}");

    // 无建档 change + true：Err 携 change 名（前置校验 1，db 缺 ChangeRecord）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root,
        "不存在的-change".to_owned(),
        true,
    )
    .await
    .expect_err("未知 change 应 Err");
    assert!(
        err.contains("不存在的-change") && err.contains("未建档"),
        "成因（db 缺 ChangeRecord）: {err}"
    );

    // 注册表零登记（auto 参数不绕过守卫直入运行态）
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert!(control.snapshot(CHANGE).is_none(), "合法 change 零登记");
    assert!(
        control.snapshot("不存在的-change").is_none(),
        "失败分支零登记"
    );
}

// ---------------------------------------------------------------------------
// 五命令保持面：stop / answer / confirm / state / watch
// ---------------------------------------------------------------------------

#[test]
fn stop运行中置位且幂等忽略不报错() {
    let env = Env::new("stop");
    env.change_dir(CHANGE);
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
    env.change_dir(CHANGE);
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
    env.change_dir(CHANGE);
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
    env.change_dir(CHANGE);
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
// 参数转换守卫：blank root / blank change（守卫先行于前置校验，root 寻址与
// `Result<T, String>` 模板保留）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 参数转换守卫blank_root各命令模板保留() {
    let env = Env::new("blank-root");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let blank = "   ".to_owned();

    // start：Err（无 cwd 无从发起）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        blank.clone(),
        CHANGE.to_owned(),
        false,
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
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    let blank = "   ".to_owned();

    // start：Err（无 change 无从发起）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        blank.clone(),
        false,
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
