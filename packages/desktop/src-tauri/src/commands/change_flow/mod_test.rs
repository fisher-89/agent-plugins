use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::StopRegistry;
use orchestration::control::ChangeFlowControl;
use store::{AgentEngineKind, AgentInstanceRecord, WorkspaceStores};
use workflow::state::{ChangeStateRecord, ChangeStatus};

use ::agent::{AgentEvent, AgentEventKind};
use orchestration::state::{RunNotice, RunUpdate};
use orchestration::RunEventSink;

use super::{
    change_flow_answer_with, change_flow_confirm_with, change_flow_start_with,
    change_flow_stop_with, change_flow_watch_with, ChangeFlowSink,
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
    // 归档链控制注册表（change_flow_start 反向互斥前置读取——main.rs 同构托管）
    app.manage(Arc::new(orchestration::archive_flow::ArchiveControl::new()));
    app.manage(Arc::new(StopRegistry::new()));
    app
}

/// db 建档种子（前置校验 1/2 的正向与反相 fixture）：经 store change 域操作
/// 面落 `ChangeRecord`（workflow_type 决定相位表校验走向），id 与 name 同值。
fn seed_change(app: &App<tauri::test::MockRuntime>, root: &str, name: &str, workflow_type: &str) {
    seed_change_full(app, root, name, name, workflow_type);
}

/// id 与 name 相异的建档种子（id → 记录 → name 分辨率单点的比对锚）。
fn seed_change_full(
    app: &App<tauri::test::MockRuntime>,
    root: &str,
    id: &str,
    name: &str,
    workflow_type: &str,
) {
    app.state::<WorkspaceStores>()
        .for_root(root)
        .expect("for_root 应成功")
        .create_change_record(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            workflow_type: workflow_type.to_owned(),
            created_at: 1727000000000,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
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

/// 丢弃型 Channel（发送通知即弃；kind-only Notice 通道）。
fn discarding_channel() -> Channel<super::RunNotice> {
    Channel::new(|_: InvokeResponseBody| Ok(()))
}

/// 捕获型 Channel：逐通知收下出线 JSON（IPC 边界捕获——kind-only 通知观测）。
fn capturing_channel() -> (
    Channel<super::RunNotice>,
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

/// 发起时刻（UTC unix 毫秒，`begin_run` 加参后的机械随动固定值）。
const STARTED_AT: i64 = 1_726_000_000_000;

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
    // 相位表 fixture：db 有档但 workflow_type 无相位表（bug-fix）；id 与 name
    // 相异——错误面 id → 记录 → name 呈现记录名
    let app = app_with(&env);
    seed_change_full(&app, &root, "id-bugfix-anchor", "bugfix-change", "bug-fix");

    // 前置校验 1：db 无建档记录（存量 CLI change）→ 显式 Err 携 change id
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

    // 前置校验 2：workflow_type 无相位表（phase_table None → 显式拒绝；错误面
    // 呈现记录 name 而非 id）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root,
        "id-bugfix-anchor".to_owned(),
        false,
    )
    .await
    .expect_err("非 requirement 应 Err");
    assert!(
        err.contains("bugfix-change") && err.contains("bug-fix") && err.contains("requirement"),
        "成因二（相位表缺失，记录 name 呈现）: {err}"
    );
    assert!(
        !err.contains("id-bugfix-anchor"),
        "相位表拒绝面恒 name 化（id → 记录 → name 分辨率单点）: {err}"
    );

    // 两成因互不重合且登记面零副作用（并行冲突分支另见「start同change并行」
    // 用例——本用例失败分支零 begin_run 登记）
    let control = app.state::<Arc<ChangeFlowControl>>();
    for id in ["legacy-cli-change", "id-bugfix-anchor"] {
        assert!(
            control.snapshot(&env.root(), id).is_none(),
            "失败分支零登记: {id}"
        );
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
        control
            .snapshot(&env.root(), CHANGE)
            .map(|snap| snap.run_id),
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
        "步通知经 Channel 流出（kind-only Notice——通知仅失效信号）"
    );
    assert!(
        updates.iter().any(|value| value["ipc"] == "finished"),
        "终态通知照常流出"
    );
    drop(updates);

    // 终态收口：注册表除名（run 仅进程内——AC-7 半边）
    wait_for("终态除名", || {
        control.snapshot(&env.root(), CHANGE).is_none()
    });

    // run 运行史落库证据（unify-run-state-persistence 每 run 两写）：PATH 隔离
    // 下 executor CliMissing → 合成收敛 failed；finish 单事务落终态 + 步整包 +
    // active_phase 清位（D1 悬挂杀除——executor 失败前 phase-start 已开相）
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    let runs = store.list_change_runs(CHANGE).expect("run 史清单应成功");
    assert_eq!(runs.len(), 1, "恰一次 run 全史在案");
    // run_id 联结：落库起始行与返回 summary 同 run 会话（命令面装配 AC-2/D5）
    assert_eq!(
        runs[0].run_id, summary.run_id,
        "起始行 run_id 与提前 resolve 摘要同源"
    );
    assert_eq!(
        runs[0].status,
        workflow::state::RunStatus::Failed,
        "PATH 隔离下合成收敛 failed（后台 walker 真实驱动 + 落库）"
    );
    assert!(runs[0].finished_at.is_some(), "收口时刻在案");
    let record = store
        .find_change_record(CHANGE)
        .expect("查档应成功")
        .expect("建档在案");
    assert!(
        record.active_phase.is_none(),
        "finish 单事务 active_phase 清位（悬挂杀除——D1）"
    );
    let steps = store
        .list_run_steps(&runs[0].run_id)
        .expect("run 步清单应成功");
    assert!(
        steps.iter().any(|step| step.phase == "proposal"),
        "phase-start 已开相位的步整包在案（落库写缝证据）"
    );
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
        .begin_run(&root, CHANGE, "run-existing".to_owned(), STARTED_AT)
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
        control
            .snapshot(&env.root(), CHANGE)
            .map(|snap| snap.run_id),
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
        control
            .snapshot(&env.root(), CHANGE)
            .map(|snap| snap.run_id),
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
    assert!(
        updates.iter().any(|value| value["ipc"] == "finished"),
        "终态通知在场"
    );
    drop(updates);

    // 终态收口除名 + run 运行史落库证据（组合根装配照常，写缝落库）
    wait_for("终态除名", || {
        control.snapshot(&env.root(), CHANGE).is_none()
    });
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    let runs = store.list_change_runs(CHANGE).expect("run 史清单应成功");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, workflow::state::RunStatus::Failed);
    let record = store
        .find_change_record(CHANGE)
        .expect("查档应成功")
        .expect("建档在案");
    assert!(
        record.active_phase.is_none(),
        "finish 单事务 active_phase 清位（悬挂杀除——D1）"
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
        control
            .snapshot(&env.root(), CHANGE)
            .map(|snap| snap.run_id),
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

    // 终态面语义不变：合成收敛 failed（步通知 + 终态通知照常流出；终态值
    // 随 run 运行史落库可查——kind-only 通知零载荷）
    let updates = captured.lock().expect("捕获锁不可中毒");
    assert!(
        updates.iter().any(|value| value["ipc"] == "step"),
        "步通知照常流出"
    );
    assert!(
        updates.iter().any(|value| value["ipc"] == "finished"),
        "终态通知在场"
    );
    drop(updates);
    let runs = app
        .state::<WorkspaceStores>()
        .for_root(&env.root())
        .expect("for_root 应成功")
        .list_change_runs(CHANGE)
        .expect("run 史清单应成功");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, workflow::state::RunStatus::Failed);

    wait_for("终态除名", || {
        control.snapshot(&env.root(), CHANGE).is_none()
    });
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
    assert!(
        control.snapshot(&env.root(), CHANGE).is_none(),
        "合法 change 零登记"
    );
    assert!(
        control.snapshot(&env.root(), "不存在的-change").is_none(),
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
        .begin_run(&env.root(), CHANGE, "run-1".to_owned(), STARTED_AT)
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
        .begin_run(&env.root(), CHANGE, "run-1".to_owned(), STARTED_AT)
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
fn watch补订运行中接收后续信封_无run时ok非错误() {
    let env = Env::new("watch");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();

    // 无 run：Ok（非错误——重挂时 run 可能已收口，图读史常驻派生）
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
        .begin_run(&env.root(), CHANGE, "run-1".to_owned(), STARTED_AT)
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

    // stop：Ok（miss 幂等；state 快照命令已退役——重挂恢复归统一查询）
    change_flow_stop_with(app.handle().clone(), blank, CHANGE.to_owned())
        .expect("blank root stop 幂等 Ok");
}

/// blank id 守卫五缝各就位：start / answer / confirm → Err（各命令模板保留
/// ——start 携「无 change 无从发起」语境，answer / confirm 为通用模板），
/// stop / watch → Ok 幂等无副作用——五命令的 id 参数守卫全量覆盖（root 合法、
/// 仅 id 空白，隔离 id 分支）。
#[tokio::test]
async fn 参数转换守卫blank_id五缝各就位() {
    let env = Env::new("blank-id");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    let blank = "   ".to_owned();

    // start：Err（模板携「无 change 无从发起」语境）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        blank.clone(),
        false,
    )
    .await
    .expect_err("blank id start 应 Err");
    assert_eq!(
        err, "非法 id: 不得为空白（无 change 无从发起）",
        "start blank id 模板保留（root 合法仅 id 空白）"
    );

    // answer / confirm：Err（各命令模板保留）
    let err = change_flow_answer_with(
        app.handle().clone(),
        root.clone(),
        blank.clone(),
        "应答".to_owned(),
    )
    .expect_err("blank id answer 应 Err");
    assert_eq!(err, "非法 id: 不得为空白", "answer blank id 模板保留");
    let err = change_flow_confirm_with(app.handle().clone(), root.clone(), blank.clone(), true)
        .expect_err("blank id confirm 应 Err");
    assert_eq!(err, "非法 id: 不得为空白", "confirm blank id 模板保留");

    // stop / watch：Ok 幂等无副作用（blank 不进入注册表链路）
    change_flow_stop_with(app.handle().clone(), root.clone(), blank.clone())
        .expect("blank id stop 幂等 Ok");
    change_flow_watch_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        blank.clone(),
    )
    .expect("blank id watch Ok 非错误");

    // 守卫先行于前置校验：合法 root 在位而 id 空白 → 零登记零 spawn 副作用
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert!(
        control.snapshot(&env.root(), CHANGE).is_none(),
        "守卫分支零登记"
    );
}

// ---------------------------------------------------------------------------
// worktree 维度（design D9 / D10 / AC-6 / AC-7 / AC-8）：exec root 解析三态 +
// 复合键命令面
// ---------------------------------------------------------------------------

/// db 建档携 worktree 执行锚的种子（exec root 解析三态用例共用）。
fn seed_change_with_worktree(
    app: &App<tauri::test::MockRuntime>,
    root: &str,
    name: &str,
    worktree: &std::path::Path,
) {
    app.state::<WorkspaceStores>()
        .for_root(root)
        .expect("for_root 应成功")
        .create_change_record(ChangeStateRecord {
            id: name.to_owned(),
            name: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: 1727000000000,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: Some(worktree.to_string_lossy().into_owned()),
            base_commit: Some("0000000000000000000000000000000000000001".to_owned()),
        })
        .expect("worktree 建档种子应成功");
}

/// worktree 目录缺失拒绝：记录携 `worktree=Some(不存在路径)` → 发起 `Err`
/// 含「worktree 目录不存在（可能已被手动删除）」引导且零 run 登记；仅带
/// worktree 记录生效——legacy 记录不经此校验（对照半边）。
#[tokio::test]
async fn worktree目录缺失拒绝_显式err且零run登记() {
    let env = Env::new("wt-missing");
    let app = app_with(&env);
    let root = env.root();
    let missing_worktree = env
        .data_dir
        .path()
        .join("worktrees")
        .join("gone-seg")
        .join(CHANGE);
    // 目录不创建（被手动删除形态）
    seed_change_with_worktree(&app, &root, CHANGE, &missing_worktree);

    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect_err("worktree 目录缺失应显式 Err");

    assert!(
        err.contains("worktree 目录不存在") && err.contains("可能已被手动删除"),
        "Err 引导文案（手动删除语境），实际: {err}"
    );
    assert!(
        err.contains(missing_worktree.to_string_lossy().as_ref()),
        "Err 携带缺失路径，实际: {err}"
    );
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert!(
        control.snapshot(&root, CHANGE).is_none(),
        "零 run 登记（前置校验先于 begin_run）"
    );

    // 对照：legacy 记录（worktree=None）不经此校验（发起链路零变化——
    // 正向可达性由既有正向行承载）
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    store
        .delete_change_record(CHANGE)
        .expect("清理 worktree 建档");
    drop(store);
    seed_change(&app, &root, CHANGE, "requirement");
    seed_cli_default_instance(&app); // 组合根缺省解析可走通（发起链路半边）
                                     // legacy 发起不因本分支 Err（PATH 隔离下真实驱动收敛 failed 即证可达）
    let (_path_guard, original) = isolate_path();
    let result = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await;
    restore_path(_path_guard, original);
    assert!(
        result.is_ok(),
        "legacy 记录（worktree=None）不经 worktree 存在性校验，实际: {result:?}"
    );
}

/// exec root 解析成功：记录携 worktree + 目录在场（tempdir 预置 openspec 树）
/// → 发起成功（提前 resolve summary running）；发起后相位半边落 **workspace
/// root 库**（for_root(root) 实例可查），且数据根 `workspaces/` 子树无以
/// worktree 路径派生的第二库文件（store 身份恒 workspace root——AC-6 db
/// 半边；PATH 隔离下 CLI 引擎合成收敛既有装置驱动）。
#[tokio::test]
async fn exec_root解析成功_相位落workspace库且无第二库文件() {
    let env = Env::new("wt-exec-root");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    // worktree 在场（openspec 树预置——exec root 即此目录）
    let worktree = env
        .data_dir
        .path()
        .join("worktrees")
        .join("seg")
        .join(CHANGE);
    fs::create_dir_all(worktree.join("openspec/changes").join(CHANGE))
        .expect("预置 worktree openspec 树失败");
    seed_change_with_worktree(&app, &root, CHANGE, &worktree);
    seed_cli_default_instance(&app); // 组合根缺省解析可走通（发起链路半边）

    let (_path_guard, original) = isolate_path();
    let summary = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect("worktree 在场发起应成功");

    assert_eq!(
        summary.status,
        super::ChangeRunStatus::Running,
        "提前 resolve summary running"
    );

    // 等待后台收敛（phase-start 落库先于 executor 失败）
    let control = app.state::<Arc<ChangeFlowControl>>();
    wait_for("run 终态除名", || {
        control.snapshot(&root, CHANGE).is_none()
    });
    restore_path(_path_guard, original);

    // run 运行史落 workspace root 库（写入经注入的 for_root(root) 实例——
    // run 落库写缝与相位写面同实例）
    let runs = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功")
        .list_change_runs(CHANGE)
        .expect("run 史清单应成功");
    assert_eq!(
        runs.len(),
        1,
        "run 史在 workspace root 库（非 worktree 库）"
    );

    // 数据根 workspaces/ 子树恰一个库文件且 = workspace root 身份派生（无以
    // worktree 路径派生的第二库——store 身份恒 workspace root）
    let workspaces_dir = env.data_dir.path().join("workspaces");
    let mut files: Vec<String> = fs::read_dir(&workspaces_dir)
        .expect("workspaces 子树应存在")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files.len(), 1, "恰一个 workspace 库文件，实际: {files:?}");
    assert_eq!(
        files[0],
        format!(
            "{}.redb",
            foundation::identity::workspace_identity_segment(&root)
        ),
        "库文件名 = workspace root 身份段派生（非 worktree 路径派生）"
    );
}

/// 并行冲突键 id（AC-6 命令面半边）：同 root 同 id 二次 start → Err（冲突
/// ——`start同change并行run冲突err` 同锚）；同名不同 id 各自受理互不误拒
/// （name 非键——原「异 root 同名并行互不误拒」扩写）；异 root 同 id 并行
/// 并存；stop 按 (root, id) 定址不波及他键。
#[tokio::test]
async fn 并行冲突键id_同名异id各自受理且异root同id互不误拒() {
    let env = Env::new("composite-key");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root_a = env.root();
    let root_b_dir = tempfile::Builder::new()
        .prefix("change-flow-cmd-test-composite-b-")
        .tempdir()
        .expect("创建 rootB 临时目录失败");
    let root_b = root_b_dir.path().to_string_lossy().into_owned();
    fs::create_dir_all(root_b_dir.path().join("openspec/changes").join(CHANGE))
        .expect("预置 rootB change 目录失败");
    // rootA 两 id 同名（name 非键的比对锚）+ rootB 同 CHANGE id（root 段随行）
    seed_change_full(&app, &root_a, "id-a1", CHANGE, "requirement");
    seed_change_full(&app, &root_a, "id-a2", CHANGE, "requirement");
    seed_change(&app, &root_b, CHANGE, "requirement");
    seed_cli_default_instance(&app);

    let (_path_guard, original) = isolate_path();

    // rootA 预登记运行中 run（guard 在手不收敛——确定性冲突锚；注记：
    // `run-<millis>` 同毫秒可同号，run_id 不作唯一性断言面）
    let control = app.state::<Arc<ChangeFlowControl>>();
    let guard_a1 = control
        .begin_run(&root_a, "id-a1", "run-a-pre".to_owned(), STARTED_AT)
        .expect("rootA 预登记应成功");

    // 同名异 id（同 root）：各自受理（name 非键——互不误拒；命令面真实驱动，
    // run 登记在案）
    let summary_a2 = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root_a.clone(),
        "id-a2".to_owned(),
        false,
    )
    .await
    .expect("rootA 同名异 id 发起应成功（name 非键）");
    assert_eq!(
        summary_a2.status,
        super::ChangeRunStatus::Running,
        "提前 resolve summary running"
    );
    assert!(
        control.snapshot(&root_a, "id-a2").is_some(),
        "id-a2 run 登记在案（与同 root 同名 id-a1 并行并存——name 非键）"
    );
    assert!(
        control.snapshot(&root_a, "id-a1").is_some(),
        "同胞 id-a1 预登记条目不受扰动"
    );

    // 异 root 同 id：不误拒（复合键 root 段），命令面真实驱动登记在案
    let summary_b = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root_b.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect("rootB 同 id 发起应成功（复合键并行解锁）");
    assert_eq!(
        summary_b.status,
        super::ChangeRunStatus::Running,
        "提前 resolve summary running"
    );
    assert!(
        control.snapshot(&root_b, CHANGE).is_some(),
        "rootB run 登记在案（与 rootA 预登记 run 并行并存——复合键生效）"
    );

    // 同 root 同 id 二次发起 Err（并行冲突——键 id 同值）
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root_a.clone(),
        "id-a1".to_owned(),
        false,
    )
    .await
    .expect_err("同 root 同 id 二次发起应 Err");
    assert!(
        err.contains("id-a1") && err.contains("已有运行中的 run"),
        "并行冲突记因: {err}"
    );

    // stop(rootA, id-a1)：仅置位目标键（不波及 id-a2 / rootB——寻址按 (root, id)）
    change_flow_stop_with(app.handle().clone(), root_a.clone(), "id-a1".to_owned())
        .expect("stop 应 Ok");
    assert!(guard_a1.cancelled(), "rootA 预登记 run 取消信号置位");
    // rootB 的 watch 补订照常 Ok（复合键寻址可达——stop(rootA) 零影响）
    change_flow_watch_with(
        app.handle().clone(),
        discarding_channel(),
        root_b.clone(),
        CHANGE.to_owned(),
    )
    .expect("rootB watch 经复合键寻址可达（stop(rootA) 零影响）");

    // 等两条命令发起 run 自然收敛（非 stop 所停——独立驱动到自身终态）
    wait_for("rootA id-a2 与 rootB run 终态除名", || {
        control.snapshot(&root_a, "id-a2").is_none() && control.snapshot(&root_b, CHANGE).is_none()
    });
    restore_path(_path_guard, original);
    drop(guard_a1); // rootA 预登记 run 随 guard 终结除名

    // 各键独立驱动证据：各自 workspace 库 StepRecord 以自身 run_id 串链在案
    //（phase-start / 步审计经注入的 for_root 实例落库）
    let steps_a2 = app
        .state::<WorkspaceStores>()
        .for_root(&root_a)
        .expect("for_root rootA 应成功")
        .list_change_steps("id-a2", Some(&summary_a2.run_id))
        .expect("rootA 步行清单应成功");
    assert!(
        !steps_a2.is_empty(),
        "id-a2 run 以自身 run_id 独立驱动落步（同 root 同名冲突零波及）"
    );
    let steps_b = app
        .state::<WorkspaceStores>()
        .for_root(&root_b)
        .expect("for_root rootB 应成功")
        .list_change_steps(CHANGE, Some(&summary_b.run_id))
        .expect("rootB 步行清单应成功");
    assert!(
        !steps_b.is_empty(),
        "rootB run 以自身 run_id 独立驱动落步（不被 rootA stop 波及）"
    );
}

/// sink 事件桥接半边（ChangeFlowSink::emit）：`SessionEvent` 记 run 级会话锚
///（`current_session` 可查）且原样 `publish`（订阅端收到同 session_id 载荷
/// ——worker 内核 → 控制注册表的唯一桥，直调锚定转发不改写）。
#[test]
fn sink事件桥接_session_event记会话锚且publish透传() {
    let env = Env::new("sink-bridge");
    let app = app_with(&env);
    let root = env.root();
    let change = "sink-change";
    let control = Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner());
    let _guard = control
        .begin_run(&root, change, "run-sink-1".to_owned(), STARTED_AT)
        .expect("发起应成功");
    let mut updates = control.subscribe(&root, change).expect("订阅应成功");

    let sink = ChangeFlowSink {
        root: root.clone(),
        change_id: change.to_owned(),
        control: Arc::clone(&control),
    };
    sink.emit(RunUpdate::SessionEvent {
        session_id: "ses-sink".to_owned(),
        event: AgentEvent::stamp(
            0,
            AgentEventKind::Raw {
                event_type: "probe".to_owned(),
                raw_json: "{}".to_owned(),
            },
        ),
    });

    assert_eq!(
        control.current_session(&root, change).as_deref(),
        Some("ses-sink"),
        "SessionEvent 记 run 级会话锚"
    );
    match updates.try_recv() {
        Ok(RunNotice::SessionEvent) => {
            // kind-only 通知（载荷剥离单点）：会话锚由 current_session 断言承载
        }
        other => panic!("订阅端应收到 SessionEvent 通知，实际: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 归档反向互斥（desktop-archive-change D3——前置校验序列 +1：run 命令面唯一
// 触点；walker / 装配形态与其余五命令零改动）
// ---------------------------------------------------------------------------

/// 归档进行中 run 发起被拒：`ArchiveControl::begin` 预登记 (root, id) 后
/// `change_flow_start_with` → Err 含归档进行中原因；零 run 落账
///（`ChangeFlowControl::snapshot` None、库内零 StepRecord）；同名异 id 同 root /
/// 异 root 同 id 不误拒（互斥键为 (root, id)——name 非键，发起照常进入既有
/// 前置校验面）。
#[tokio::test]
async fn start归档进行中被拒_复合键寻址异键不误拒() {
    let env = Env::new("archive-mutex");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");

    let archive_control = app.state::<Arc<orchestration::archive_flow::ArchiveControl>>();
    let guard = archive_control
        .begin(&root, CHANGE)
        .expect("归档链预登记应成功");

    // 同键：run 发起被拒（归档进行中原因）
    let error = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect_err("归档进行中应 Err");
    assert!(
        error.contains("归档链进行中"),
        "归档进行中拒绝记因: {error}"
    );

    // 零 run 落账：快照 None（未进入运行态）
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert!(
        control.snapshot(&root, CHANGE).is_none(),
        "拒绝分支零 run 登记"
    );
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    assert!(
        store
            .list_change_steps(CHANGE, None)
            .expect("步骤枚举应成功")
            .is_empty(),
        "库内零 StepRecord"
    );

    // 同名异 id 同 root：不误拒（互斥键为 (root, id)——name 非键；进入既有
    // 校验面——compose 缺省解析无实例 Err 而非归档记因）
    seed_change_full(&app, &root, "id-same-name", CHANGE, "requirement");
    let error = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        "id-same-name".to_owned(),
        false,
    )
    .await
    .expect_err("同名异 id 应走既有校验面");
    assert!(
        !error.contains("归档链进行中"),
        "同名异 id 不误拒归档互斥（互斥键为 (root, id)）: {error}"
    );

    // 异 root 同名 change：不误拒（复合键寻址）
    let other_env = Env::new("archive-mutex-other-root");
    other_env.change_dir(CHANGE);
    seed_change(&app, &other_env.root(), CHANGE, "requirement");
    let error = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        other_env.root(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect_err("异 root 应走既有校验面");
    assert!(
        !error.contains("归档链进行中"),
        "异 root 不误拒归档互斥: {error}"
    );

    drop(guard);
}

/// Channel 面五 kind 通知各自一拍出线且零载荷键（AC-8 kind 位 + 信封降位线
/// 面）：运行中 run 经 watch 补订，guard 逐一 publish 五类 RunUpdate——每帧
/// 捕获恰单键 `ipc`（kind-only，载荷剥离单点在 publish 广播侧）。
#[test]
fn channel面五kind通知各自一拍且零载荷键() {
    let env = Env::new("notice-wire");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();

    let control = app.state::<Arc<ChangeFlowControl>>();
    let guard = control
        .begin_run(&env.root(), CHANGE, "run-1".to_owned(), STARTED_AT)
        .expect("登记应成功");
    let (channel, captured) = capturing_channel();
    change_flow_watch_with(app.handle().clone(), channel, root, CHANGE.to_owned())
        .expect("补订应成功");

    guard.emit(super::RunUpdate::Step {
        step: orchestration::ChangeStepState {
            phase: "implement".to_owned(),
            attempt: 1,
            step: orchestration::ChangeStepKind::Executor,
            status: orchestration::ChangeStepStatus::Running,
            session_id: None,
            detail: None,
        },
    });
    guard.emit(super::RunUpdate::SessionEvent {
        session_id: "sess-1".to_owned(),
        event: agent::AgentEvent {
            seq: 0,
            timestamp_ms: 1_726_000_000_000,
            kind: agent::AgentEventKind::Raw {
                event_type: "system".to_owned(),
                raw_json: "{}".to_owned(),
            },
        },
    });
    guard.emit(super::RunUpdate::Ask {
        question: "回溯到哪?".to_owned(),
        options: vec!["proposal".to_owned()],
    });
    guard.emit(super::RunUpdate::ConfirmWait {
        phase: "test-gen".to_owned(),
    });
    guard.finish(super::ChangeRunStatus::Completed, None);

    wait_for("五 kind 通知到齐", || {
        captured.lock().expect("捕获锁不可中毒").len() >= 5
    });
    let frames = captured.lock().expect("捕获锁不可中毒");
    let ipcs: Vec<&str> = frames
        .iter()
        .map(|frame| frame["ipc"].as_str().expect("ipc 判别词"))
        .collect();
    assert_eq!(
        ipcs,
        vec!["step", "sessionEvent", "ask", "confirmWait", "finished"],
        "五 kind 各自一拍（publish 序即广播序）"
    );
    for frame in frames.iter() {
        assert_eq!(
            frame.as_object().expect("通知为对象").len(),
            1,
            "零载荷键（恰单键 ipc——kind-only 信封）: {frame}"
        );
    }
}
