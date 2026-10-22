use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::{SessionStreamRegistry, StopRegistry};
use orchestration::control::ChangeFlowControl;
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, WorkspaceStores,
};
use workflow::state::{ChangeStateRecord, ChangeStatus};

use ::agent::{AgentEvent, AgentEventKind};
use orchestration::state::{
    ChangeStepKind, ChangeStepState, ChangeStepStatus, RunNotice, RunUpdate,
};
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
    app.manage(Arc::new(StopRegistry::new()));
    // 会话实时流注册表：与 `src/main.rs` 同型空表托管（两轨同源注入面）
    app.manage(Arc::new(SessionStreamRegistry::new()));
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
            title: name.to_owned(),
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

/// sdk 默认实例 fixture（组合根缺省解析可走通——发起链路真实驱动的用例共用）。
/// provider 三件套留空：executor 轮在 `begin()` open 段以 `ConfigMissing`
/// 显式收敛 failed（CLI 租户退役后原 CliMissing 合成收敛的归位口径——不
/// spawn 进程不落网络）。
fn seed_sdk_default_instance(app: &App<tauri::test::MockRuntime>) {
    let stores = app.state::<WorkspaceStores>();
    let provider = stores
        .inner()
        .global()
        .upsert_agent_provider(AgentProviderRecord::new(
            "组合根供应".to_owned(),
            AgentProviderProtocol::Openai,
            String::new(),
            String::new(),
            AgentModelTiers {
                high: String::new(),
                medium: String::new(),
                low: String::new(),
            },
            None,
            None,
        ))
        .expect("落 provider fixture")
        .id;
    let id = stores
        .inner()
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            "组合根实例".to_owned(),
            AgentEngineKind::Sdk,
            Some(provider),
        ))
        .expect("落 sdk 实例 fixture")
        .id;
    stores
        .inner()
        .global()
        .set_default_agent_instance(id)
        .expect("置默认实例 fixture");
}

/// PATH 隔离窗口关闭（先恢复原值再放锁——供 [`prefix_path`] 前置窗口收尾）。
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
fn wait_for(what: &str, probe: impl FnMut() -> bool) {
    wait_for_up_to(what, Duration::from_secs(30), probe);
}

/// 显式超时版（真件 spawn 组合用例的收敛窗口——sdk 缺席失败链路耗时更长）。
fn wait_for_up_to(what: &str, timeout: Duration, mut probe: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !probe() {
        assert!(
            Instant::now() < deadline,
            "等待{what}超时（{}s）",
            timeout.as_secs()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// initialize 表头 pass 条目种子（命令面真件用例：路由跳过表头——与既有
/// executor 链路行为对齐，表头自身的路由 / 停等形态由其余专测承载）。
fn seed_initialize_passed(app: &App<tauri::test::MockRuntime>, root: &str, id: &str) {
    let store = app
        .state::<WorkspaceStores>()
        .for_root(root)
        .expect("for_root 应成功");
    let started = store
        .start_change_phase(id, "initialize", STARTED_AT)
        .expect("initialize 开相种子应成功");
    store
        .log_change_phase(&workflow::state::PhaseLogCommand {
            change_id: id.to_owned(),
            phase: "initialize".to_owned(),
            verdict: workflow::model::Verdict::Pass,
            report: "基线检查通过".to_owned(),
            skipped: false,
            checklist: Vec::new(),
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
            start_at: Some(started.start_at),
            timestamp: STARTED_AT + 30_000,
        })
        .expect("initialize 落账种子应成功");
}

/** 手动档停等应答回路（命令面真件用例：表头 initialize 收口后停等照常——
 * auto=false 用例需应答 Confirm 方可继续路由；无等待方时 confirm Err 忽略重试）。 */
fn spawn_command_confirmer(
    app: tauri::AppHandle<tauri::test::MockRuntime>,
    control: Arc<ChangeFlowControl>,
    root: String,
    id: &'static str,
) {
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
            if control.snapshot(&root, id).is_none() {
                break;
            }
            let _ = change_flow_confirm_with(app.clone(), root.clone(), id.to_owned(), true, true);
        }
    });
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
    .expect_err("未知 workflow_type 应 Err");
    assert!(
        err.contains("bugfix-change") && err.contains("bug-fix"),
        "成因二（相位表缺失，记录 name 呈现）: {err}"
    );
    assert!(
        err.contains("requirement") && err.contains("test-only"),
        "拒绝文案通吃 {{requirement, test-only}} 两值: {err}"
    );
    assert!(
        !err.contains("仅支持 requirement 工作流"),
        "旧「仅支持 requirement 工作流」字样退役: {err}"
    );
    assert!(
        !err.contains("id-bugfix-anchor"),
        "相位表拒绝面恒 name 化（id → 记录 → name 分辨率单点）: {err}"
    );

    // 边界成因：超长（>1000 字符）workflow_type 相位表同样不命中 → 同一严格
    // 拒绝面（不截断、不折叠、零兜底）
    let long_type = "x".repeat(1200);
    seed_change_full(
        &app,
        &env.root(),
        "id-long-wf",
        "long-wf-change",
        &long_type,
    );
    let err = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        env.root(),
        "id-long-wf".to_owned(),
        false,
    )
    .await
    .expect_err("超长 workflow_type 应 Err");
    assert!(
        err.contains("long-wf-change"),
        "超长类型同样归因相位表缺失（记录 name 呈现）: {err}"
    );
    assert!(
        err.contains("requirement") && err.contains("test-only"),
        "超长类型拒绝文案同样通吃两值: {err}"
    );

    // 两成因互不重合且登记面零副作用（并行冲突分支另见「start同change并行」
    // 用例——本用例失败分支零 begin_run 登记）
    let control = app.state::<Arc<ChangeFlowControl>>();
    for id in ["legacy-cli-change", "id-bugfix-anchor", "id-long-wf"] {
        assert!(
            control.snapshot(&env.root(), id).is_none(),
            "失败分支零登记: {id}"
        );
    }
}

// ---------------------------------------------------------------------------
// start 提前 resolve 与组合根装配（db 种子，缺省解析 + open 段显式收敛）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start提前resolve返回running摘要且channel首事件到达后台驱动不阻塞() {
    let env = Env::new("early-resolve");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");

    // sdk 默认实例 fixture（组合根缺省解析可走通）
    seed_sdk_default_instance(&app);
    seed_initialize_passed(&app, &root, CHANGE);

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

    // 后台 turn 已收敛
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

    // run 运行史落库证据（unify-run-state-persistence 每 run 两写）：executor
    // 轮 open 段 ConfigMissing 显式收敛 failed；finish 单事务落终态 + 步整包 +
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
        "executor open 段 ConfigMissing 收敛 failed（后台 walker 真实驱动 + 落库）"
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
    let nodes = store
        .list_run_nodes(&runs[0].run_id)
        .expect("run 节点清单应成功");
    assert!(
        nodes.iter().any(|node| node.phase == "proposal"),
        "phase-start 已开相位的节点整包在案（落库写缝证据）"
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.step == workflow::state::RunStepKind::PhaseStart)
            .count(),
        1,
        "phase_start 词汇零过滤落库（表头 initialize 预置 pass 零开相——proposal 单枚开相）"
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
    seed_sdk_default_instance(&app);

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

    // 后台 walker 真实驱动至收敛（executor 轮 open 段 ConfigMissing → failed）
    wait_for("Finished 信封", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "finished")
    });

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
    seed_sdk_default_instance(&app);
    seed_initialize_passed(&app, &root, CHANGE);
    spawn_command_confirmer(
        app.handle().clone(),
        Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner()),
        root.clone(),
        CHANGE,
    );

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

    // 终态面语义不变：open 段 ConfigMissing 收敛 failed（步通知 + 终态通知
    // 照常流出；终态值随 run 运行史落库可查——kind-only 通知零载荷）
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
    let err = change_flow_confirm_with(
        app.handle().clone(),
        env.root(),
        CHANGE.to_owned(),
        true,
        false,
    )
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
    let err = change_flow_confirm_with(
        app.handle().clone(),
        env.root(),
        CHANGE.to_owned(),
        true,
        true,
    )
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
        archive: None,
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
    let err = change_flow_confirm_with(
        app.handle().clone(),
        blank.clone(),
        CHANGE.to_owned(),
        true,
        true,
    )
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
    let err = change_flow_confirm_with(
        app.handle().clone(),
        root.clone(),
        blank.clone(),
        true,
        true,
    )
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
            title: name.to_owned(),
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
    // 组合根缺省解析可走通（发起链路半边）；legacy 发起不因本分支 Err
    //（后台 walker 真实驱动收敛 failed 即证可达）
    seed_sdk_default_instance(&app);
    let result = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await;
    assert!(
        result.is_ok(),
        "legacy 记录（worktree=None）不经 worktree 存在性校验，实际: {result:?}"
    );
}

/// exec root 解析成功：记录携 worktree + 目录在场（tempdir 预置 openspec 树）
/// → 发起成功（提前 resolve summary running）；发起后相位半边落 **workspace
/// root 库**（for_root(root) 实例可查），且数据根 `workspaces/` 子树无以
/// worktree 路径派生的第二库文件（store 身份恒 workspace root——AC-6 db
/// 半边；executor 轮 open 段 ConfigMissing 显式收敛，不 spawn 进程）。
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
    seed_sdk_default_instance(&app); // 组合根缺省解析可走通（发起链路半边）

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
    seed_sdk_default_instance(&app);
    {
        let control = Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner());
        spawn_command_confirmer(app.handle().clone(), control, root_a.clone(), "id-a2");
    }
    {
        let control = Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner());
        spawn_command_confirmer(app.handle().clone(), control, root_b.clone(), CHANGE);
    }

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
///（`current_session` 可查）且原样 `publish`（订阅端收到 SessionEvent 通知
/// ——worker 内核 → 控制注册表的唯一桥，直调锚定转发不改写）；首拍另经
/// `publish` 把会话 id 提前挂到在途会话型步节点并补发 Step 通知（运行中
/// 节点直查实时转录的寻址面）。
#[test]
fn sink事件桥接_session_event记会话锚且首拍挂载在途节点() {
    let env = Env::new("sink-bridge");
    let app = app_with(&env);
    let root = env.root();
    let change = "sink-change";
    let control = Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner());
    let guard = control
        .begin_run(&root, change, "run-sink-1".to_owned(), STARTED_AT)
        .expect("发起应成功");
    let mut updates = control.subscribe(&root, change).expect("订阅应成功");
    // 在途 executor 节点（running emit 不携会话 id——首拍挂载的落点）
    guard.emit(RunUpdate::Step {
        step: ChangeStepState {
            phase: "implement".to_owned(),
            attempt: 1,
            step: ChangeStepKind::Executor,
            instance_no: 0,
            status: ChangeStepStatus::Running,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: STARTED_AT,
            ended_at: None,
            token_input: None,
            token_output: None,
        },
    });
    assert!(
        matches!(updates.try_recv(), Ok(RunNotice::Step)),
        "running emit 通知先排空"
    );

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
    let snap = control.snapshot(&root, change).expect("运行中快照可达");
    assert_eq!(
        snap.nodes[0].session_id.as_deref(),
        Some("ses-sink"),
        "首拍把会话 id 提前挂到在途节点（运行中转录直查寻址）"
    );
    match updates.try_recv() {
        Ok(RunNotice::Step) => {
            // 首挂补发 Step（统一视图重查触发；kind-only 载荷剥离）
        }
        other => panic!("订阅端应先收到 Step 补发通知，实际: {other:?}"),
    }
    match updates.try_recv() {
        Ok(RunNotice::SessionEvent) => {
            // kind-only 通知（载荷剥离单点）：会话锚由 current_session 断言承载
        }
        other => panic!("订阅端应收到 SessionEvent 通知，实际: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 归档链并入 run（AC-10）：反向互斥面摘除 + run 命令面装配
//（stop_before=archive / vcs 缝 / confirm sync_specs 透传）
// ---------------------------------------------------------------------------

/// 归档互斥面摘除（正向）：`change_flow_start` 不再读取任何归档链控制注册表——
/// 同一 (root, id) 直接受理进入运行态（无「归档链进行中」拒绝分支），注册表条目
/// 与 run 运行史照常落（AC-10 命令面收编）。
#[tokio::test]
async fn start归档互斥面摘除_无归档注册表照常受理() {
    let env = Env::new("archive-mutex-removed");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");
    seed_sdk_default_instance(&app);
    seed_initialize_passed(&app, &root, CHANGE);

    let (channel, captured) = capturing_channel();
    let summary = change_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect("无归档注册表亦照常受理（互斥面已摘除）");
    assert!(summary.run_id.starts_with("run-"));

    let control = app.state::<Arc<ChangeFlowControl>>();
    assert_eq!(
        control.snapshot(&root, CHANGE).map(|snap| snap.run_id),
        Some(summary.run_id.clone()),
        "受理即登记（零归档互斥前置）"
    );

    wait_for("Finished 信封", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "finished")
    });
    wait_for("终态除名", || control.snapshot(&root, CHANGE).is_none());
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    let runs = store.list_change_runs(CHANGE).expect("run 史清单应成功");
    assert_eq!(runs.len(), 1, "run 运行史落一行（命令面收编后唯一发起面）");
}

/// 归档前停等与确认拒绝（正向）：8 路由相位全 pass 后 run 停在 archive 前——
/// 停止门发 kind-only `ConfirmWait` 通知并挂起（快照 waitingConfirm / phase=
/// archive）；`change_flow_confirm_with(proceed=false)` 收敛 stopped，归档相位零
/// 落账（AC-7 / AC-10）。
#[tokio::test]
async fn start走完路由相位停在archive前且confirm拒绝收敛stopped() {
    let env = Env::new("stop-before-archive");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "requirement");
    seed_sdk_default_instance(&app);
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    for (index, phase) in [
        "proposal",
        "dev-design",
        "test-design",
        "implement",
        "test-gen",
        "test-execution",
        "code-review",
        "acceptance",
    ]
    .iter()
    .enumerate()
    {
        let started = store
            .start_change_phase(CHANGE, phase, STARTED_AT + (index as i64) * 1000)
            .expect("开相种子应成功");
        store
            .log_change_phase(&workflow::state::PhaseLogCommand {
                change_id: CHANGE.to_owned(),
                phase: (*phase).to_owned(),
                verdict: workflow::model::Verdict::Pass,
                report: "种子通过".to_owned(),
                skipped: false,
                checklist: Vec::new(),
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
                start_at: Some(started.start_at),
                timestamp: started.start_at + 500,
            })
            .expect("落账种子应成功");
    }

    let (channel, captured) = capturing_channel();
    let summary = change_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
        true,
    )
    .await
    .expect("受理");
    assert_eq!(summary.status, super::ChangeRunStatus::Running);

    wait_for("ConfirmWait 通知", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "confirmWait")
    });
    let frames = captured.lock().expect("捕获锁不可中毒").clone();
    let wait_frames: Vec<&serde_json::Value> = frames
        .iter()
        .filter(|value| value["ipc"] == "confirmWait")
        .collect();
    assert_eq!(wait_frames.len(), 1, "归档前恰一道停等通知");
    assert_eq!(
        wait_frames[0].as_object().expect("通知为对象").len(),
        1,
        "kind-only 零载荷（归档前置载荷不出进程）"
    );

    let control = app.state::<Arc<ChangeFlowControl>>();
    let snapshot = control.snapshot(&root, CHANGE).expect("run 在册");
    assert_eq!(snapshot.status, super::ChangeRunStatus::WaitingConfirm);
    assert_eq!(snapshot.phase.as_deref(), Some("archive"));

    change_flow_confirm_with(
        app.handle().clone(),
        root.clone(),
        CHANGE.to_owned(),
        false,
        true,
    )
    .expect("confirm 透传应成功");
    wait_for("终态收口", || control.snapshot(&root, CHANGE).is_none());
    let runs = store.list_change_runs(CHANGE).expect("run 史清单应成功");
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].status,
        workflow::state::RunStatus::Stopped,
        "停等未确认 → stopped（归档相位零执行）"
    );
    let archive_entries = store
        .list_phase_records(CHANGE)
        .expect("读相位条目应成功")
        .into_iter()
        .filter(|entry| entry.phase == "archive")
        .count();
    assert_eq!(archive_entries, 0, "归档相位零落账（未进相位）");
}

/// `change_flow_confirm` 透传 `sync_specs`（正向）：五参经命令面抵达注册表
/// 应答通道（停等面无等待方即 Err「当前无等待中的 phase 确认」——参数校验与
/// 透传路径照常，AC-7 / D2）。
#[test]
fn confirm透传sync_specs四组合至注册表() {
    let env = Env::new("confirm-sync-specs");
    let app = app_with(&env);
    let root = env.root();
    let control = app.state::<Arc<ChangeFlowControl>>();

    for (proceed, sync_specs) in [(true, true), (true, false), (false, true), (false, false)] {
        let guard = control
            .begin_run(&root, CHANGE, "run-1".to_owned(), STARTED_AT)
            .expect("登记应成功");
        guard.emit(super::RunUpdate::ConfirmWait {
            phase: "archive".to_owned(),
            archive: None,
        });
        // 无 walker 等待方：confirm 应答 Err（单次通道空置）——透传路径四组合
        // 均不 panic、不误判合法参数
        let err = change_flow_confirm_with(
            app.handle().clone(),
            root.clone(),
            CHANGE.to_owned(),
            proceed,
            sync_specs,
        )
        .expect_err("无挂起等待方应 Err");
        assert_eq!(err, "当前无等待中的 phase 确认");
        guard.finish(super::ChangeRunStatus::Stopped, None);
    }
}

/// `change_flow_confirm` 校验（异常 / 回归）：blank root / id 显式 Err；无运行
/// run Err 透传（既有语义不变，AC-7 / AC-10）。
#[test]
fn confirm校验blank与无运行run双面err() {
    let env = Env::new("confirm-guard");
    let app = app_with(&env);
    let root = env.root();

    let err = change_flow_confirm_with(
        app.handle().clone(),
        "   ".to_owned(),
        CHANGE.to_owned(),
        true,
        true,
    )
    .expect_err("blank root 应 Err");
    assert!(err.contains("非法 root"), "blank root 记因: {err}");

    let err = change_flow_confirm_with(
        app.handle().clone(),
        root.clone(),
        "  ".to_owned(),
        true,
        false,
    )
    .expect_err("blank id 应 Err");
    assert!(err.contains("非法 id"), "blank id 记因: {err}");

    let err = change_flow_confirm_with(
        app.handle().clone(),
        root.clone(),
        CHANGE.to_owned(),
        true,
        false,
    )
    .expect_err("无运行 run 应 Err");
    assert!(err.contains("无运行中的 run"), "miss 记因: {err}");
}

/// Channel 面各 kind 通知出线且零载荷键（AC-8 kind 位 + 信封降位线面）：运行
/// 中 run 经 watch 补订，guard 逐一 publish 各类 RunUpdate——每帧捕获恰单键
/// `ipc`（kind-only，载荷剥离单点在 publish 广播侧）；SessionEvent 首拍另把
/// 会话 id 挂到在途 executor 节点并补发一拍 step（运行中转录直查寻址），故
/// step kind 共两拍。
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
            instance_no: 0,
            status: orchestration::ChangeStepStatus::Running,
            session_id: None,
            detail: None,
            output_ref: None,
            started_at: STARTED_AT,
            ended_at: None,
            token_input: None,
            token_output: None,
        },
    });
    guard.emit(super::RunUpdate::Edge {
        edge: orchestration::ChangeStepEdge {
            from: (
                "implement".to_owned(),
                1,
                orchestration::ChangeStepKind::Executor,
                0,
            ),
            to: (
                "implement".to_owned(),
                1,
                orchestration::ChangeStepKind::StaticCheck,
                0,
            ),
            kind: orchestration::state::EdgeKind::Forward,
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
        archive: None,
    });
    guard.finish(super::ChangeRunStatus::Completed, None);

    wait_for(
        "各 kind 通知到齐（含首拍 attach 补发的 step）",
        || captured.lock().expect("捕获锁不可中毒").len() >= 7,
    );
    let frames = captured.lock().expect("捕获锁不可中毒");
    let ipcs: Vec<&str> = frames
        .iter()
        .map(|frame| frame["ipc"].as_str().expect("ipc 判别词"))
        .collect();
    assert_eq!(
        ipcs,
        vec![
            "step",
            "edge",
            "step",
            "sessionEvent",
            "ask",
            "confirmWait",
            "finished"
        ],
        "publish 序即广播序（第二拍 step = SessionEvent 首挂补发；edge kind-only 信封随 shape 演进）"
    );
    for frame in frames.iter() {
        assert_eq!(
            frame.as_object().expect("通知为对象").len(),
            1,
            "零载荷键（恰单键 ipc——kind-only 信封）: {frame}"
        );
    }
}

// ---------------------------------------------------------------------------
// 组合根装配（change-flow-show-step-only，AC-6 / D4）：`ProcessStaticCheck::new()`
// 零参构造（无 `recorded(change)` 装配分支——默认即 recorded）→ `LocalToolSteps`
// StaticCheck 步 → 平台 shim 命令真实 spawn → infra/exec 单点落单一 `.log` 并
// 回填 `output_ref`；审计行落库。与 `change_flow_start_with` 的组合根装配同式
//（同一 `LocalToolSteps::new(...)` 七参 + `ProcessStaticCheck::new()`）。
// ---------------------------------------------------------------------------

/// PATH 前置 workspace 根窗口（shim 裸名经 PATH 解析；经 commands 级 PATH 锁
/// 串行化，窗口关闭时恢复原 PATH）。
fn prefix_path(
    root: &Path,
) -> (
    std::sync::MutexGuard<'static, ()>,
    Option<std::ffi::OsString>,
) {
    let guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    let base = original.clone().unwrap_or_default();
    let dirs = std::env::split_paths(&base);
    let joined = std::env::join_paths(std::iter::once(root.to_path_buf()).chain(dirs))
        .expect("join PATH 失败");
    std::env::set_var("PATH", joined);
    (guard, original)
}

/// 在 workspace 根写入平台 static-check shim（stdout / stderr 交错写）；返回
/// config 应登记的裸名（Windows 带 `.cmd` 后缀）。
fn write_static_shim(root: &Path) -> String {
    #[cfg(windows)]
    {
        let body = "@echo off\r\necho static-out\r\n1>&2 echo static-err\r\n";
        fs::write(root.join("static-shim.cmd"), body).expect("写 shim 失败");
        "static-shim.cmd".to_owned()
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let script = root.join("static-shim");
        fs::write(&script, "#!/bin/sh\necho static-out\necho static-err >&2\n")
            .expect("写 shim 失败");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        "static-shim".to_owned()
    }
}

#[tokio::test]
async fn 组合根装配_staticcheck步默认recorded落单一log并回填引用() {
    use orchestration::port::{ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest};
    use orchestration::steps::LocalToolSteps;
    use workflow::state::ChangeStateStore;
    use workflow::write::SessionAnchors;

    let env = Env::new("static-check-assembly");
    let root = env.root();
    let app = app_with(&env);
    seed_change(&app, &root, CHANGE, "requirement");
    // 平台 shim 命令登记 config（真实 spawn 面）
    let shim = write_static_shim(env.ws_root.path());
    fs::create_dir_all(env.ws_root.path().join("openspec")).expect("建 openspec 目录");
    fs::write(
        env.ws_root.path().join("openspec/config.json"),
        format!("{{ \"static_analysis\": \"{shim}\" }}"),
    )
    .expect("写 config 失败");

    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    let store_port: Arc<dyn ChangeStateStore> = store.clone();
    let tools = LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(checks_runtime::ProcessStaticCheck::new()),
        Arc::new(checks_runtime::ProcessTestExecution::new()),
        Arc::new(checks_runtime::ProcessMutation::new()),
        Arc::new(vcs_runtime::ProcessArchiveVcs::new()),
        store_port,
        "run-assembly".to_owned(),
    );

    // PATH 前置窗口覆盖真实 spawn（shim 裸名解析）
    let (guard, original) = prefix_path(env.ws_root.path());
    let output = tools
        .run(ToolStepRequest {
            root: root.clone(),
            cancel: foundation::cancel::CancelFlag::never(),
            command: ToolCommand::StaticCheck {
                change_id: CHANGE.to_owned(),
                slot: foundation::layout::ReportSlot::Test,
            },
        })
        .await;
    restore_path(guard, original);
    let output = output.expect("静态检查步应 Ok");

    let ToolStepOutput::StaticCheck(outcome) = output else {
        panic!("产出应为 StaticCheck 变体");
    };
    assert!(outcome.passed, "shim 退出 0 → passed");
    assert_eq!(
        outcome.diagnostics.lines().collect::<Vec<_>>(),
        ["static-out", "static-err"],
        "诊断 = 2>&1 交错 stdout"
    );
    let exec_id = outcome
        .output_ref
        .clone()
        .expect("默认 recorded（零参 new() 装配）→ output_ref 恒回填");
    assert!(exec_id.starts_with("exec-"), "exec_id 定式: {exec_id}");

    let dir =
        foundation::layout::change_test_reports(Path::new(&root), CHANGE).join("exec-outputs");
    assert!(
        dir.join(format!("{exec_id}.log")).is_file(),
        "单一 .log 落盘（组合根零记录逻辑——记录单点驻 infra/exec）"
    );
    assert!(
        !dir.join(format!("{exec_id}.err")).exists(),
        ".err 分文件退役"
    );
    let read_back = exec::read_exec_output(Path::new(&root), CHANGE, &exec_id).expect("回查应命中");
    assert_eq!(
        String::from_utf8_lossy(&read_back)
            .lines()
            .collect::<Vec<_>>(),
        ["static-out", "static-err"],
        "记录内容 = 脚本真实交错输出"
    );

    // 审计行落库（StepKind::StaticCheck 经组合根装配链路可枚举）
    let rows = store
        .list_change_steps(CHANGE, Some("run-assembly"))
        .expect("读审计行应成功");
    assert_eq!(rows.len(), 1, "静态检查步恰一条审计行");
    assert_eq!(rows[0].summary, "static_check passed=true");
}

// ---------------------------------------------------------------------------
// start 前置校验的 test-only 放行（desktop-test-only-flow AC-4）：两值分派下
// test-only 记录不再被前置校验拒绝——walker 相位分派由写面相位表承担
// ---------------------------------------------------------------------------

/// test-only 记录发起（正向 / 新增）：`phase_table("test-only")` 命中 → 前置
/// 校验放行，提前 resolve 返回 running 摘要；后台 walker 真实驱动至终态
///（相位分派随 test-only 表，命令层零 workflow_type 分支）。
#[tokio::test]
async fn start_test_only记录放行且提前resolve返回running摘要() {
    let env = Env::new("test-only-start");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, "test-only");
    seed_sdk_default_instance(&app);
    seed_initialize_passed(&app, &root, CHANGE);
    spawn_command_confirmer(
        app.handle().clone(),
        Arc::clone(app.state::<Arc<ChangeFlowControl>>().inner()),
        root.clone(),
        CHANGE,
    );

    let (channel, captured) = capturing_channel();
    let result = change_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await;

    // 前置校验放行：不落入「未知 workflow_type 拒绝」分支
    let summary = match result {
        Ok(summary) => summary,
        Err(err) => panic!("test-only 记录应放行（两值分派），实际 Err: {err}"),
    };
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
    let control = app.state::<Arc<ChangeFlowControl>>();
    assert_eq!(
        control
            .snapshot(&env.root(), CHANGE)
            .map(|snap| snap.run_id),
        Some(summary.run_id.clone()),
        "run 登记即快照可见"
    );

    // 后台 walker 真实驱动终态（test-only 表相位分派由写面承担）
    wait_for("Finished 信封", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "finished")
    });

    // 落库相位属 test-only 表（首相位 proposal 恒为两表共有；零 dev-design /
    // implement 等 requirement 独有相位落账）
    let store = app.state::<WorkspaceStores>();
    let phases: Vec<String> = store
        .for_root(&env.root())
        .expect("for_root 应成功")
        .list_phase_records(CHANGE)
        .expect("读相位史应成功")
        .into_iter()
        .map(|record| record.phase)
        .collect();
    assert!(
        phases.iter().all(|phase| [
            "initialize",
            "proposal",
            "code-analyze",
            "test-design",
            "test-gen",
            "test-execution",
            "archive"
        ]
        .contains(&phase.as_str())),
        "落账相位均在 test-only 表内（initialize 表头种子在案），实际: {phases:?}"
    );
}

// ---------------------------------------------------------------------------
// initialize 基线真件组合（check-before-flow-start / AC-1 / AC-2）：真实 git
// 主仓 + git worktree 夹具 + 平台 shim 真件 spawn（`write_static_shim` + config
// 登记 + `prefix_path` PATH 注入 + commands 级 PATH 锁串行化——Mock策略：无）
// ---------------------------------------------------------------------------

/// 建真实 git 主仓（单提交在案）+ worktree 落位（`git worktree add`）——返回
/// worktree 绝对路径。直驱 git 子命令（真件夹具，零 mock）。
fn git_main_with_worktree(main_root: &Path, worktree: &Path, branch: &str) {
    let run_git = |args: &[&str], cwd: &Path| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("夹具 git 拉起失败: {error}"));
        assert!(
            output.status.success(),
            "夹具 git {} 失败: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run_git(&["init", "-q"], main_root);
    run_git(&["config", "user.name", "fixture"], main_root);
    run_git(&["config", "user.email", "fixture@example.com"], main_root);
    fs::write(main_root.join("seed.txt"), "seed\n").expect("写种子文件失败");
    fs::write(main_root.join("cwd-probe.txt"), "worktree probe").expect("写探针失败");
    #[cfg(windows)]
    {
        fs::write(
            main_root.join("static-shim.cmd"),
            "@echo off\r\ntype cwd-probe.txt\r\nif exist coverage\\exit-code.txt exit /b 1\r\n",
        )
        .expect("写 shim 失败");
        run_git(
            &["add", "seed.txt", "cwd-probe.txt", "static-shim.cmd"],
            main_root,
        );
    }
    #[cfg(not(windows))]
    {
        {
            use std::os::unix::fs::PermissionsExt;
            let script = main_root.join("static-shim");
            fs::write(
                &script,
                "#!/bin/sh\ncat cwd-probe.txt\nif [ -f coverage/exit-code.txt ]; then exit 1; fi\nexit 0\n",
            )
            .expect("写 shim 失败");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        }
        run_git(
            &["add", "seed.txt", "cwd-probe.txt", "static-shim"],
            main_root,
        );
    }
    run_git(
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-q",
            "-m",
            "init",
        ],
        main_root,
    );
    run_git(
        &["worktree", "add", &worktree.to_string_lossy(), "-b", branch],
        main_root,
    );
}

/// 基线绿走真件全链（组合，AC-1）：真实 git 主仓 + worktree 夹具（exec root ≠
/// 主 root）+ 绿 static shim（退出 0）→ initialize 相位先于 proposal 路由、
/// Clean / StaticCheck / TestExecution / PhaseLog 节点齐备、检查在 worktree 内
/// 执行（cwd 断言：shim 相对路径读到 worktree 内探针文件）、报告与 exec-outputs
/// 落 `openspec/changes/<name>/reports/baseline/`（AC-1 真件全链）。
#[tokio::test]
async fn 基线绿走_initialize先于proposal_报告落baseline树() {
    let env = Env::new("baseline-green");
    let app = app_with(&env);
    let root = env.root();
    let worktree = env.data_dir.path().join("wt").join(CHANGE);
    fs::create_dir_all(env.data_dir.path().join("wt")).expect("建 wt 父目录失败");
    git_main_with_worktree(env.ws_root.path(), &worktree, &format!("change/{CHANGE}"));

    // worktree 内预置 openspec 树（config 在排除面——clean 保留）；探针与 shim
    // 已随主仓提交（tracked——clean `reset --hard` 后仍在场；shim 相对路径读
    // cwd-probe.txt 即 spawn cwd = worktree 的内嵌断言）
    fs::create_dir_all(worktree.join("openspec/changes").join(CHANGE))
        .expect("预置 worktree openspec 树失败");
    seed_change_with_worktree(&app, &root, CHANGE, &worktree);

    // shim 裸名（tracked 夹具供给；零 coverage/exit-code.txt → 退出 0 绿态）
    #[cfg(windows)]
    let shim = "static-shim.cmd".to_owned();
    #[cfg(not(windows))]
    let shim = "static-shim".to_owned();
    fs::write(
        worktree.join("openspec/config.json"),
        format!("{{ \"static_analysis\": \"{shim}\" }}"),
    )
    .expect("写 config 失败");
    seed_sdk_default_instance(&app);

    // commands 级 PATH 锁串行化 + PATH 前置 worktree（guard 持有至后台收敛）
    let (guard, original) = prefix_path(&worktree);
    let (diag_channel, diag_captured) = capturing_channel();
    let summary = change_flow_start_with(
        app.handle().clone(),
        diag_channel,
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect("基线绿走发起应成功");
    assert_eq!(summary.status, super::ChangeRunStatus::Running);
    eprintln!("DBG-绿走 summary={summary:?}");

    let control = app.state::<Arc<ChangeFlowControl>>();
    // confirmer 回路：手动档逐相位 ConfirmWait 应答（auto=false——initialize 收口
    // 后停等照常，确认后继续路由 proposal；无等待方时 confirm Err 忽略重试）
    {
        let handle_confirm = app.handle().clone();
        let control_confirm = Arc::clone(&control);
        let root_confirm = root.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(120);
            while Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
                if control_confirm.snapshot(&root_confirm, CHANGE).is_none() {
                    break;
                }
                let _ = change_flow_confirm_with(
                    handle_confirm.clone(),
                    root_confirm.clone(),
                    CHANGE.to_owned(),
                    true,
                    true,
                );
            }
        });
    }
    {
        let store_dbg = app
            .state::<WorkspaceStores>()
            .for_root(&root)
            .expect("for_root");
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut last_dbg = Instant::now();
        loop {
            if control.snapshot(&root, CHANGE).is_none() {
                break;
            }
            if last_dbg.elapsed() >= Duration::from_secs(5) {
                last_dbg = Instant::now();
                if let Some(snap) = control.snapshot(&root, CHANGE) {
                    eprintln!(
                        "DBG-绿走 snapshot: run={:?} phase={:?}",
                        snap.run_id, snap.phase
                    );
                }
                let captured_dbg = diag_captured.lock().expect("捕获锁");
                eprintln!(
                    "DBG-绿走 updates: {:?}",
                    captured_dbg
                        .iter()
                        .map(|value| {
                            (
                                value["ipc"].as_str().unwrap_or("?").to_owned(),
                                value["step"].as_str().map(str::to_owned),
                                value["phase"].as_str().map(str::to_owned),
                            )
                        })
                        .collect::<Vec<_>>()
                );
            }
            if Instant::now() >= deadline {
                if let Ok(runs) = store_dbg.list_change_runs(CHANGE) {
                    for run in &runs {
                        eprintln!(
                            "DBG-绿走 run={} status={:?} reason={:?}",
                            run.run_id, run.status, run.reason
                        );
                        if let Ok(nodes) = store_dbg.list_run_nodes(&run.run_id) {
                            eprintln!(
                                "DBG-绿走 nodes: {:?}",
                                nodes
                                    .iter()
                                    .map(|n| {
                                        (
                                            n.phase.as_str(),
                                            n.step.as_str(),
                                            format!("{:?}", n.status),
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            );
                        }
                    }
                }
                panic!("等待run 终态除名超时（120s）");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    restore_path(guard, original);

    // 节点整包：initialize 五节点先于 proposal（相位序逐位——AC-1）
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    let runs = store.list_change_runs(CHANGE).expect("run 史应成功");
    eprintln!(
        "DBG 绿走终态: {:?} reason={:?}",
        runs[0].status, runs[0].reason
    );
    assert_eq!(runs.len(), 1);
    let nodes = store
        .list_run_nodes(&runs[0].run_id)
        .expect("节点清单应成功");
    let initialize_words: Vec<&str> = nodes
        .iter()
        .filter(|node| node.phase == "initialize")
        .map(|node| node.step.as_str())
        .collect();
    assert_eq!(
        initialize_words,
        [
            "phase_start",
            "clean",
            "static_check",
            "test_execution",
            "phase_log"
        ],
        "initialize 五节点齐备且词面逐位"
    );
    let first_phase = nodes
        .first()
        .map(|node| node.phase.as_str())
        .expect("首节点应在案");
    assert_eq!(
        first_phase, "initialize",
        "initialize 先于 proposal 路由（表头先行）"
    );
    assert!(
        nodes.iter().any(|node| node.phase == "proposal"),
        "initialize 后续路由 proposal（表头一次性——proposal 站开跑）"
    );

    // 检查在 worktree 内执行 + 报告与 exec-outputs 落 worktree baseline 树
    let baseline = worktree
        .join("openspec/changes")
        .join(CHANGE)
        .join("reports/baseline");
    assert!(
        baseline.join("summary.json").is_file() || baseline.exists(),
        "baseline 报告树在 worktree 内: {}",
        baseline.display()
    );
    let exec_outputs = baseline.join("exec-outputs");
    assert!(
        exec_outputs.exists()
            && fs::read_dir(&exec_outputs)
                .expect("读 exec-outputs")
                .count()
                > 0,
        "exec-outputs 落 baseline 树（static-check 输出记录）"
    );

    // PhaseRecord：initialize pass 在案（先于 proposal）
    let entries = store.list_phase_records(CHANGE).expect("读相位条目应成功");
    let initialize = entries
        .iter()
        .find(|entry| entry.phase == "initialize")
        .expect("initialize 条目应在案");
    assert_eq!(
        initialize.verdict,
        workflow::model::Verdict::Pass,
        "基线绿 → initialize pass 落账"
    );
    assert!(!initialize.skipped, "worktree 分支非跳过语义");
    assert_eq!(
        entries.first().map(|entry| entry.phase.as_str()),
        Some("initialize"),
        "initialize 条目先于 proposal（表序落账）"
    );
}

/// 基线红停真件全链（组合，重试链 / AC-2）：shim 非零退出 → run 以 Terminal
/// 收敛 failed、终态文案引导在基线上修复后重新发起；PhaseRecord 零 initialize
/// 条目；shim 转绿后重新发起 → clean 恢复基线后干净重试（initialize pass 落账
/// ——AC-2 真件全链）。
#[tokio::test]
async fn 基线红停_terminal收敛文案引导且转绿后干净重试() {
    let env = Env::new("baseline-red");
    let app = app_with(&env);
    let root = env.root();
    let worktree = env.data_dir.path().join("wt").join(CHANGE);
    fs::create_dir_all(env.data_dir.path().join("wt")).expect("建 wt 父目录失败");
    git_main_with_worktree(env.ws_root.path(), &worktree, &format!("change/{CHANGE}"));
    fs::create_dir_all(worktree.join("openspec/changes").join(CHANGE))
        .expect("预置 worktree openspec 树失败");
    seed_change_with_worktree(&app, &root, CHANGE, &worktree);

    // shim 红形态经排除面控制文件编程（coverage/exit-code.txt 在 clean 排除面
    // ——未跟踪控制文件跨 clean 存活；tracked shim 读它切非零退出）。「转绿」=
    // 删除控制文件（零文件改写——tracked shim 全程不被 clean 触碰）
    let arm_red = |red: bool| {
        let dir = worktree.join("coverage");
        if red {
            fs::create_dir_all(&dir).expect("建 coverage 控制目录失败");
            fs::write(dir.join("exit-code.txt"), "1").expect("写控制文件失败");
        } else {
            let _ = fs::remove_file(dir.join("exit-code.txt"));
        }
    };
    arm_red(true);
    fs::write(
        worktree.join("openspec/config.json"),
        "{ \"static_analysis\": \"static-shim\" }",
    )
    .expect("写 config 失败");
    seed_sdk_default_instance(&app);

    // 首 run：基线红 → Terminal failed
    let (guard, original) = prefix_path(&worktree);
    change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect("红发起仍受理成功（失败在后台收敛）");
    let control = app.state::<Arc<ChangeFlowControl>>();
    wait_for_up_to("首 run 终态除名", Duration::from_secs(120), || {
        control.snapshot(&root, CHANGE).is_none()
    });

    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root 应成功");
    let runs = store.list_change_runs(CHANGE).expect("run 史应成功");
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].status,
        workflow::state::RunStatus::Failed,
        "基线红 → run failed（Terminal 收敛）"
    );
    assert!(
        runs[0]
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("基线静态检查未通过")),
        "终态文案引导基线修复后重新发起: {:?}",
        runs[0].reason
    );
    let entries = store.list_phase_records(CHANGE).expect("读相位条目应成功");
    assert!(
        !entries.iter().any(|entry| entry.phase == "initialize"),
        "红停零 initialize 条目"
    );
    // 预置未跟踪残留（下轮 clean 恢复面的真件锚）
    fs::write(worktree.join("leftover.txt"), "leftover\n").expect("write leftover");

    // shim 转绿：摘除控制文件 → 重新发起（clean 恢复基线后干净重试）
    arm_red(false);
    let second = change_flow_start_with(
        app.handle().clone(),
        discarding_channel(),
        root.clone(),
        CHANGE.to_owned(),
        false,
    )
    .await
    .expect("二次发起应受理成功");

    // 二次 run：initialize pass 落账后主动 stop（测试聚焦 initialize 表头——
    // 后续 proposal 相位的 agent 收敛不属本用例面）
    let store_dbg = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .expect("for_root");
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let entries = store_dbg
            .list_phase_records(CHANGE)
            .expect("读相位条目应成功");
        if entries.iter().any(|entry| {
            entry.phase == "initialize" && entry.verdict == workflow::model::Verdict::Pass
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "等待二次 run initialize pass 超时（120s）"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = change_flow_stop_with(app.handle().clone(), root.clone(), CHANGE.to_owned());
    wait_for_up_to("二次 run 终态除名", Duration::from_secs(120), || {
        control.snapshot(&root, CHANGE).is_none()
    });
    restore_path(guard, original);

    // 二次 run：clean 恢复基线后干净重试（initialize pass + clean 节点在案）
    let runs = store.list_change_runs(CHANGE).expect("run 史应成功");
    assert_eq!(runs.len(), 2, "两轮 run 全史");
    let entries = store.list_phase_records(CHANGE).expect("读相位条目应成功");
    let initialize = entries
        .iter()
        .find(|entry| entry.phase == "initialize")
        .expect("二次 run initialize pass 应在案");
    assert_eq!(initialize.verdict, workflow::model::Verdict::Pass);
    assert!(!initialize.skipped, "worktree 分支正常落账");
    let nodes = store
        .list_run_nodes(&second.run_id)
        .expect("二次 run 节点应成功");
    assert!(
        nodes
            .iter()
            .any(|node| node.phase == "initialize"
                && node.step == workflow::state::RunStepKind::Clean),
        "二次 run clean 节点在案（基线恢复面）"
    );
    assert!(
        !worktree.join("leftover.txt").exists(),
        "clean 恢复基线：残留未跟踪文件被清除（AC-5 真件半边）"
    );
}
// ---------------------------------------------------------------------------
// 会话实时流注册表托管注入缝（compose_turn 第三参）：空表托管零 panic 零注册表
// 条目；未托管形态的取用错误与 `StopRegistry` 同口径（托管契约由 src/main.rs
// 与各命令测试装置共同锚定）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 空表streams托管下受理前置拒绝零panic且注册表零条目() {
    let env = Env::new("streams-hosted");
    let app = app_with(&env);
    let root = env.root();
    let streams = Arc::clone(app.state::<Arc<SessionStreamRegistry>>().inner());

    // 托管的是空表（与 `src/main.rs` 同型 `SessionStreamRegistry::default()`）
    for session_id in ["", "ses-any", "特殊\u{0}😀"] {
        assert!(
            streams.subscribe(session_id, 0).is_none(),
            "空表托管：任意 id 订阅为 None（{session_id:?}）"
        );
    }

    // 受理前置拒绝路径：注入缝不改受理语义（Err 文案既有逐字）
    let (channel, captured) = capturing_channel();
    let error = change_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        String::new(),
        false,
    )
    .await
    .expect_err("blank id 必须 Err");
    assert!(error.contains("id"), "拒绝文案携 id 线索，实际: {error}");
    assert!(
        captured.lock().expect("捕获锁不可中毒").is_empty(),
        "拒绝路径零通知"
    );
    assert!(
        streams.subscribe("ses-any", 0).is_none(),
        "未驱动的受理路径不产生注册表条目"
    );
}

#[tokio::test]
async fn 未托管streams形态取用错误与stopregistry同口径() {
    // 未托管：显式构造 mock app 而两者皆不 manage → state 取用即 panic（同一
    // 托管契约口径；正托管形态由上面用例与既有受理 / 控制 / 通知面用例承载）
    let app = tauri::test::mock_app();
    let streams_missing = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.state::<Arc<SessionStreamRegistry>>();
    }))
    .is_err();
    let stop_missing = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.state::<Arc<StopRegistry>>();
    }))
    .is_err();
    assert!(streams_missing, "未托管 streams 取用失败");
    assert!(stop_missing, "未托管 stop 取用失败");
    assert!(
        streams_missing == stop_missing,
        "两表托管契约同口径（缺一即同型失败）"
    );
}
