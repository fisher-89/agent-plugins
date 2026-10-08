//! `archive_flow` 命令组的单元测试（test-design「commands/archive_flow/mod.rs
//! -> mod_test.rs」节）：MockRuntime mock app 四态托管（真实 WorkspaceStores +
//! `Arc<ChangeFlowControl>` + `Arc<StopRegistry>` + `Arc<ArchiveControl>`——
//! 与 main.rs 注入面同型）+ 命令真实走 `*_with` 泛型测试缝。挂 AC-1（命令面
//! 拒绝与读面）、AC-6（PATH 隔离 CLI 失败收敛 + 除名）、AC-9（run 面隔离命令
//! 面负断言 + ArchiveSink 转译）、AC-7（回环摘要半边）。

use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::{AgentEvent, AgentEventKind, StopRegistry};
use orchestration::archive_flow::{
    ArchiveControl, ArchiveStage, ArchiveStageState, ArchiveStageStatus, ArchiveUpdate,
};
use orchestration::control::ChangeFlowControl;
use orchestration::port::RunEventSink;
use orchestration::state::{ChangeStepKind, ChangeStepState, ChangeStepStatus, RunUpdate};
use store::{AgentEngineKind, AgentInstanceRecord, WorkspaceStores};
use workflow::model::Verdict;
use workflow::state::{ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand};
use workflow::write::phase_table;

use super::{
    archive_flow_preflight_with, archive_flow_start_with, archive_flow_state_with,
    archive_flow_stop_with, archive_flow_watch_with, ArchiveSink,
};

/// PATH 环境变量修改串行化（进程全局变量边界；与 change_flow / exec 的 PATH
/// 隔离窗口共用 commands 级锁）。
use crate::commands::TEST_PATH_LOCK as PATH_LOCK;

// ---------------------------------------------------------------------------
// 装置：tempdir 双根 + mock app 托管四态 + db 建档种子 + 捕获型 Channel
// ---------------------------------------------------------------------------

/// 数据根 + workspace 根临时环境（tempfile RAII）。
struct Env {
    data_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("archive-flow-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("archive-flow-cmd-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_dir, ws_root }
    }

    fn root(&self) -> String {
        self.ws_root.path().to_string_lossy().into_owned()
    }

    /// 预置 change 的磁盘目录 + 产物三件 + archive 树（真实 rename 的父目录
    /// 前提——write::archive_test 同式预置）。
    fn change_dir(&self, name: &str) -> std::path::PathBuf {
        let dir = self.ws_root.path().join("openspec/changes").join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        fs::create_dir_all(self.ws_root.path().join("openspec/changes/archive"))
            .expect("预置 archive 树失败");
        for file in ["proposal.md", "design.md", "tasks.md"] {
            fs::write(dir.join(file), format!("{file} 产物\n")).expect("布置产物失败");
        }
        dir
    }

    /// ws_root 预置为真实 git 仓（单提交在案——mergeTarget 探测的
    /// `branch --show-current` 面）。
    fn init_git_repo(&self) {
        let root = self.ws_root.path();
        let run = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .expect("装置 git 拉起失败");
            assert!(
                output.status.success(),
                "装置 git {} 失败: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run(&["init", "-q"]);
        fs::write(root.join("seed.txt"), "seed\n").expect("布置种子文件失败");
        run(&["add", "seed.txt"]);
        run(&[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-q",
            "-m",
            "init",
        ]);
    }
}

/// 以 MockRuntime 建测用 app：manage 真实 WorkspaceStores + `Arc<ChangeFlowControl>` +
/// `Arc<ArchiveControl>` + `Arc<StopRegistry>`（与 main.rs 注入面同型——
/// `change_flow` mod_test 装置的归档注册表一态同构托管）。
fn app_with(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app.manage(Arc::new(ChangeFlowControl::new()));
    app.manage(Arc::new(ArchiveControl::new()));
    app.manage(Arc::new(StopRegistry::new()));
    app
}

/// db 建档种子（active 起步；worktree 记录随建档面落）；返回 workspace 库实例
/// 供相位种子与翻转断言复用。
fn seed_change(
    app: &App<tauri::test::MockRuntime>,
    root: &str,
    name: &str,
    worktree: Option<String>,
) -> Arc<store::Store> {
    let store = app
        .state::<WorkspaceStores>()
        .for_root(root)
        .expect("for_root 应成功");
    store
        .create_change_record(ChangeStateRecord {
            name: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: 1727000000000,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree,
            base_commit: None,
        })
        .expect("建档种子应成功");
    store
}

/// 相位行种子（store change 域操作面；verdict 逐相位可配——完成度双态 fixture）。
fn seed_phase(store: &store::Store, change: &str, phase: &str, verdict: Verdict) {
    let started = store
        .start_change_phase(change, phase, 1727000000000)
        .expect("开相种子应成功");
    store
        .log_change_phase(&PhaseLogCommand {
            change: change.to_owned(),
            phase: phase.to_owned(),
            verdict,
            report: "种子".to_owned(),
            skipped: false,
            checklist: Vec::new(),
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
            start_at: Some(started.start_at),
            timestamp: started.start_at + 30_000,
        })
        .expect("落账种子应成功");
}

/// 全相位 pass 种子（preflight completed=true 的 fixture）。
fn seed_all_pass(store: &store::Store, change: &str) {
    let table = phase_table("requirement").expect("requirement 相位表应在案");
    for definition in table {
        seed_phase(store, change, definition.id, Verdict::Pass);
    }
}

/// cli 默认实例 fixture（组合根缺省解析可走通——链真实驱动的用例共用）。
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

/// PATH 隔离窗口关闭（先恢复原值再放锁——窗口过早关闭会把真实 CLI 泄入链）。
fn restore_path(guard: std::sync::MutexGuard<'static, ()>, original: Option<std::ffi::OsString>) {
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(guard);
}

/// 捕获型 Channel：逐信封收下出线 JSON（IPC 边界捕获——阶段序 / 终态 / 线词
/// camelCase 观测面）。
fn capturing_channel() -> (Channel<ArchiveUpdate>, Arc<Mutex<Vec<serde_json::Value>>>) {
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

/// 轮询至谓词成立（后台链驱动的观测窗口；30s 上限防挂死）。
fn wait_for(what: &str, mut probe: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !probe() {
        assert!(Instant::now() < deadline, "等待{what}超时（30s）");
        std::thread::sleep(Duration::from_millis(10));
    }
}

const CHANGE: &str = "archive-cmd-change";

// ---------------------------------------------------------------------------
// preflight 命令读面（AC-1 读面半边）
// ---------------------------------------------------------------------------

/// preflight 命令读面：MockRuntime app + 真实建档（active + 全 pass 相位种子 +
/// 产物树）→ Some 且字段面完整；worktree 记录 + 真实 git tempdir 主仓 →
/// `mergeTarget` = 当前分支名（ProcessArchiveVcs 真件半边）。
#[test]
fn preflight命令读面_字段面完整且merge_target出真实分支名() {
    // git 真件调用（init_git_repo + 分支名探测）与 PATH 隔离窗口互斥（进程全
    // 局变量边界——commands 级 PATH 锁全用例持有）
    let _path_guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let env = Env::new("preflight-full");
    env.init_git_repo();
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    let store = seed_change(&app, &root, CHANGE, None);
    seed_all_pass(store.as_ref(), CHANGE);

    // legacy 形态（worktree=None）：mergeTarget 不探测保持 None
    let read = archive_flow_preflight_with(app.handle().clone(), root.clone(), CHANGE.to_owned())
        .expect("active 建档 → Some");
    assert_eq!(read.name, CHANGE);
    assert!(read.completed, "全 pass 相位种子 → completed=true");
    assert!(read.incomplete_phases.is_empty() && read.missing_artifacts.is_empty());
    assert!(read.delta_specs.is_empty(), "无 delta specs");
    assert_eq!(read.worktree, None);
    assert_eq!(read.branch, None);
    assert_eq!(read.merge_target, None, "legacy 不探测合入目标");
    assert!(!read.run_active);

    // worktree 形态（记录随建档面落）：mergeTarget = 真实 git 当前分支名
    let worktree = env.ws_root.path().join("worktree-slot");
    fs::create_dir_all(&worktree).expect("建 worktree 槽位失败");
    let wt_change = "wt-change";
    env.change_dir(wt_change);
    seed_change(
        &app,
        &root,
        wt_change,
        Some(worktree.to_string_lossy().into_owned()),
    );
    let branch = {
        let output = Command::new("git")
            .arg("-C")
            .arg(env.ws_root.path())
            .args(["branch", "--show-current"])
            .output()
            .expect("git 拉起失败");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    let read =
        archive_flow_preflight_with(app.handle().clone(), root.clone(), wt_change.to_owned())
            .expect("worktree 记录在场 → Some");
    assert_eq!(
        read.worktree.as_deref(),
        Some(worktree.to_string_lossy().as_ref())
    );
    assert_eq!(
        read.merge_target.as_deref(),
        Some(branch.as_str()),
        "mergeTarget = 主仓当前分支名（真件半边）"
    );
    assert_eq!(read.branch.as_deref(), Some("change/wt-change"));
}

/// preflight blank 与 None 口径：blank root / change → None；未建档 / 已归档 →
/// None（D6 读语义，不进库解析链路）。
#[test]
fn preflight_blank与none口径_不可归档兜底() {
    let env = Env::new("preflight-none");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    let store = seed_change(&app, &root, CHANGE, None);

    assert_eq!(
        archive_flow_preflight_with(app.handle().clone(), "   ".to_owned(), CHANGE.to_owned()),
        None,
        "blank root 早退 None"
    );
    assert_eq!(
        archive_flow_preflight_with(app.handle().clone(), root.clone(), " ".to_owned()),
        None,
        "blank change 早退 None"
    );
    assert_eq!(
        archive_flow_preflight_with(app.handle().clone(), root.clone(), "no-such".to_owned()),
        None,
        "未建档 → None"
    );
    store
        .set_archived(CHANGE, 1727000000001)
        .expect("翻转种子应成功");
    assert_eq!(
        archive_flow_preflight_with(app.handle().clone(), root, CHANGE.to_owned()),
        None,
        "已归档 → None"
    );
}

// ---------------------------------------------------------------------------
// start 拒绝面与重入防护（AC-1 命令面）
// ---------------------------------------------------------------------------

/// start 拒绝面：blank root / change 显式 Err；未建档 Err；已归档 Err；run 在案
/// → Err 呈现运行中原因且 `ArchiveControl::is_active` 保持 false（零登记——
/// AC-1 正向互斥）。
#[tokio::test]
async fn start拒绝面_blank未建档已归档与run在案各显式err() {
    let env = Env::new("start-reject");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    let store = seed_change(&app, &root, CHANGE, None);
    let (channel, _captured) = capturing_channel();

    let error = archive_flow_start_with(
        app.handle().clone(),
        channel.clone(),
        "  ".to_owned(),
        CHANGE.to_owned(),
        true,
    )
    .await
    .expect_err("blank root 应 Err");
    assert!(error.contains("root"), "blank root 记因: {error}");

    let error = archive_flow_start_with(
        app.handle().clone(),
        channel.clone(),
        root.clone(),
        " ".to_owned(),
        true,
    )
    .await
    .expect_err("blank change 应 Err");
    assert!(error.contains("change"), "blank change 记因: {error}");

    let error = archive_flow_start_with(
        app.handle().clone(),
        channel.clone(),
        root.clone(),
        "no-such".to_owned(),
        true,
    )
    .await
    .expect_err("未建档应 Err");
    assert!(
        error.contains("no-such") && error.contains("未建档"),
        "记因: {error}"
    );

    store
        .set_archived(CHANGE, 1727000000001)
        .expect("翻转种子应成功");
    let error = archive_flow_start_with(
        app.handle().clone(),
        channel.clone(),
        root.clone(),
        CHANGE.to_owned(),
        true,
    )
    .await
    .expect_err("已归档应 Err");
    assert!(error.contains("已归档"), "记因: {error}");

    // run 在案（正向互斥）：显式拒绝且归档注册表零登记
    let occupied = "run-occupied";
    env.change_dir(occupied);
    seed_change(&app, &root, occupied, None);
    let run_control = app.state::<Arc<ChangeFlowControl>>();
    let guard = run_control
        .begin_run(&root, occupied, "run-1".to_owned())
        .expect("run 预登记应成功");
    let error = archive_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        occupied.to_owned(),
        true,
    )
    .await
    .expect_err("run 在案应 Err");
    assert!(error.contains("运行中的 run"), "运行中拒绝记因: {error}");
    assert!(
        !app.state::<Arc<ArchiveControl>>()
            .is_active(&root, occupied),
        "拒绝分支归档注册表零登记（AC-1 正向互斥）"
    );
    drop(guard);
}

/// 重入防护透传：`ArchiveControl::begin` 预登记 (root, change) 后 start → Err
///（链进行中重复点击——R8 防重入的命令面锚）。
#[tokio::test]
async fn start重入防护透传_在案即显式err() {
    let env = Env::new("start-reentry");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, None);
    let (channel, _captured) = capturing_channel();

    let control = app.state::<Arc<ArchiveControl>>();
    let guard = control.begin(&root, CHANGE).expect("预登记应成功");

    let error = archive_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
        true,
    )
    .await
    .expect_err("重入应 Err");
    assert!(error.contains("归档链进行中"), "重入防护记因透传: {error}");
    drop(guard);
}

// ---------------------------------------------------------------------------
// start 受理回环（提前 resolve + Channel 事件流）
// ---------------------------------------------------------------------------

/// start 受理与事件流回环：legacy 建档 + 主仓 active 树 + 无 delta specs +
/// PATH 隔离 CLI 窗口 → Ok；Channel 收阶段序（SpecSync skipped → Commit /
/// Merge skipped legacy → Seal passed → Finalize 状态）+ Finished summary
///（specs=none、archivedDir 日期前缀名）；db status=archived、目录改名落盘；
/// 终态后 `archive_flow_state` → None（除名）。
#[tokio::test]
async fn start受理与事件流回环_legacy无delta链路收口() {
    let env = Env::new("start-loop");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    let store = seed_change(&app, &root, CHANGE, None);
    seed_cli_default_instance(&app);

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let (channel, captured) = capturing_channel();
    let accepted = archive_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
        true,
    )
    .await;

    // 提前 resolve 契约：受理即 Ok（运行态经 Channel 流出）
    assert!(accepted.expect("受理应成功"), "接受即 Ok(true)");

    wait_for("链终态信封", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "finished")
    });
    let envelopes = captured.lock().expect("捕获锁不可中毒").clone();

    // 阶段序：六段全到（每段 running→终态成对——相邻去重后恰六段线词）
    let stage_words: Vec<&str> = envelopes
        .iter()
        .filter(|value| value["ipc"] == "stage")
        .map(|value| value["stage"]["stage"].as_str().expect("阶段线词"))
        .collect();
    let mut distinct: Vec<&str> = Vec::new();
    for word in &stage_words {
        if distinct.last() != Some(word) {
            distinct.push(word);
        }
    }
    assert_eq!(
        distinct,
        vec![
            "preflight",
            "specSync",
            "commit",
            "merge",
            "seal",
            "finalize"
        ],
        "六段阶段线词依序全到（camelCase 线词；信封流: {stage_words:?}）"
    );
    // 终态为成功收口（summary 在场）
    let finished = envelopes
        .iter()
        .find(|value| value["ipc"] == "finished")
        .expect("终态信封应在捕获面");
    let summary = finished["summary"].as_object().expect("成功收口携 summary");
    assert_eq!(summary["name"], CHANGE);
    assert_eq!(summary["specs"], "none", "无 delta specs → specs=none");
    assert!(
        summary["archivedDir"]
            .as_str()
            .expect("归档目录名")
            .ends_with(&format!("-{CHANGE}")),
        "archivedDir 日期前缀名（camelCase 字段出线）"
    );

    // 双写落盘：db 翻转 archived + 目录改名进 archive 树
    assert_eq!(
        store
            .find_change_record(CHANGE)
            .expect("读档应成功")
            .expect("记录应在案")
            .status,
        ChangeStatus::Archived,
        "db 翻转落账"
    );
    assert!(
        !env.ws_root
            .path()
            .join("openspec/changes")
            .join(CHANGE)
            .exists(),
        "主仓 active 树目录已改名"
    );
    let archive_entries: Vec<String> =
        fs::read_dir(env.ws_root.path().join("openspec/changes/archive"))
            .expect("archive 树应在场")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
    assert_eq!(archive_entries.len(), 1, "恰一个归档目录");

    restore_path(_guard, original);

    // 终态后快照除名
    assert_eq!(
        archive_flow_state_with(app.handle().clone(), root, CHANGE.to_owned()),
        None,
        "终态除名 → state None"
    );
}

/// agent 失败收敛与 run 面负断言：delta specs 在场 + PATH 隔离（CliMissing 合
/// 成收敛）→ SpecSync failed + Finished error；ArchiveControl 除名（state
/// None）；全程 `ChangeFlowControl::snapshot` 恒 None、零 run 信封（run 面隔
/// 离的命令面负断言——AC-9）。
#[tokio::test]
async fn start_agent失败收敛_cli_missing且run面零外泄() {
    let env = Env::new("agent-fail");
    let dir = env.change_dir(CHANGE);
    // delta specs 在场（specSync 段真实发起 agent 会话）
    fs::create_dir_all(dir.join("specs/cap-a")).expect("建 specs 失败");
    fs::write(dir.join("specs/cap-a/spec.md"), "## ADDED Requirements\n").expect("写 spec 失败");
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, None);
    seed_cli_default_instance(&app);

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let (channel, captured) = capturing_channel();
    let accepted = archive_flow_start_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
        true,
    )
    .await;
    assert!(accepted.is_ok(), "受理应成功（失败在链内收敛）");

    wait_for("链失败终态", || {
        captured
            .lock()
            .expect("捕获锁不可中毒")
            .iter()
            .any(|value| value["ipc"] == "finished")
    });
    let envelopes = captured.lock().expect("捕获锁不可中毒").clone();
    restore_path(_guard, original);

    let finished = envelopes
        .iter()
        .find(|value| value["ipc"] == "finished")
        .expect("失败也应收终态信封");
    assert!(finished["summary"].is_null(), "失败收口零 summary");
    let error = finished["error"].as_str().expect("失败记因");
    assert!(!error.is_empty(), "Finished error 携 CLI 失败语境: {error}");
    // SpecSync 段 failed 终态在案
    assert!(
        envelopes.iter().any(|value| {
            value["ipc"] == "stage"
                && value["stage"]["stage"] == "specSync"
                && value["stage"]["status"] == "failed"
        }),
        "SpecSync 段 failed 终态"
    );

    // ArchiveControl 除名（state None）
    assert_eq!(
        archive_flow_state_with(app.handle().clone(), root.clone(), CHANGE.to_owned()),
        None,
        "失败终态后除名"
    );
    // run 面负断言：快照恒 None（零 run 落账）
    let run_control = app.state::<Arc<ChangeFlowControl>>();
    assert!(
        run_control.snapshot(&root, CHANGE).is_none(),
        "run 注册表零写入（run 面隔离——AC-9）"
    );
    // 零 run 信封外泄：捕获面全部为归档信封词汇（stage / sessionEvent / finished）
    for value in &envelopes {
        let ipc = value["ipc"].as_str().expect("信封 tag");
        assert!(
            matches!(ipc, "stage" | "sessionEvent" | "finished"),
            "捕获面仅归档信封词汇，实际: {ipc}"
        );
    }
}

// ---------------------------------------------------------------------------
// stop / watch / state 命令面（AC-6 停止面 + 重挂恢复面）
// ---------------------------------------------------------------------------

/// stop 命令：预登记条目 + 已 publish 阶段后 stop → 取消旗置位（控制面经
/// guard.cancelled 观察）；无会话槽 miss 幂等 Ok；blank root / change → Ok 零
/// 副作用。
#[test]
fn stop命令_取消旗置位且miss幂等() {
    let env = Env::new("stop");
    let app = app_with(&env);
    let root = env.root();
    let control = app.state::<Arc<ArchiveControl>>();
    let guard = control.begin(&root, CHANGE).expect("预登记应成功");
    control.publish(
        &root,
        CHANGE,
        ArchiveUpdate::Stage {
            stage: ArchiveStageState {
                stage: ArchiveStage::Preflight,
                status: ArchiveStageStatus::Running,
                detail: None,
            },
        },
    );

    assert!(!guard.cancelled(), "停止前取消旗未置位");
    archive_flow_stop_with(app.handle().clone(), root.clone(), CHANGE.to_owned())
        .expect("stop 应 Ok");
    assert!(guard.cancelled(), "stop 后取消旗置位（控制面观察）");

    // 无会话槽（current_session None）→ 幂等 Ok；blank → Ok 零副作用
    archive_flow_stop_with(app.handle().clone(), root.clone(), "no-such".to_owned())
        .expect("miss 幂等 Ok");
    archive_flow_stop_with(app.handle().clone(), " ".to_owned(), " ".to_owned())
        .expect("blank 零副作用 Ok");
    drop(guard);
}

/// watch 补订：无在案 → Ok 非错误；预登记 + 补订后 publish 一信封 → 该信封经
/// Channel 到达（broadcast 补订语义——补订只收后续信封，重挂恢复面与快照查询
/// 配合——`change_flow` watch 先例同型）。
#[test]
fn watch补订_无在案ok且在案后续信封到达() {
    let env = Env::new("watch");
    let app = app_with(&env);
    let root = env.root();

    // 无在案：Ok 非错误
    let (channel, captured) = capturing_channel();
    archive_flow_watch_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
    )
    .expect("无在案 watch 应 Ok");
    assert!(captured.lock().expect("捕获锁不可中毒").is_empty());

    // 预登记 + 补订先行 + publish → 信封经 Channel 到达
    let control = app.state::<Arc<ArchiveControl>>();
    let guard = control.begin(&root, CHANGE).expect("预登记应成功");
    let (channel, captured) = capturing_channel();
    archive_flow_watch_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
    )
    .expect("在案 watch 应 Ok");
    let stage = ArchiveStageState {
        stage: ArchiveStage::Preflight,
        status: ArchiveStageStatus::Running,
        detail: None,
    };
    control.publish(&root, CHANGE, ArchiveUpdate::Stage { stage });

    wait_for("补订信封", || {
        !captured.lock().expect("捕获锁不可中毒").is_empty()
    });
    let envelopes = captured.lock().expect("捕获锁不可中毒").clone();
    assert_eq!(envelopes[0]["ipc"], "stage", "补订收到 publish 信封");
    assert_eq!(envelopes[0]["stage"]["stage"], "preflight");
    drop(guard);
}

/// state 快照口径：预登记 + publish 两阶段信封 → Some（stages 累积 / sessionId
/// None）；除名后 → None；blank → None。
#[test]
fn state快照口径_累积与除名() {
    let env = Env::new("state");
    let app = app_with(&env);
    let root = env.root();
    let control = app.state::<Arc<ArchiveControl>>();

    assert_eq!(
        archive_flow_state_with(app.handle().clone(), root.clone(), CHANGE.to_owned()),
        None,
        "无在案 → None"
    );
    assert_eq!(
        archive_flow_state_with(app.handle().clone(), " ".to_owned(), CHANGE.to_owned()),
        None,
        "blank → None"
    );

    let guard = control.begin(&root, CHANGE).expect("预登记应成功");
    for (stage, status) in [
        (ArchiveStage::Preflight, ArchiveStageStatus::Passed),
        (ArchiveStage::SpecSync, ArchiveStageStatus::Running),
    ] {
        control.publish(
            &root,
            CHANGE,
            ArchiveUpdate::Stage {
                stage: ArchiveStageState {
                    stage,
                    status,
                    detail: None,
                },
            },
        );
    }

    let snapshot = archive_flow_state_with(app.handle().clone(), root.clone(), CHANGE.to_owned())
        .expect("运行期快照在场");
    assert_eq!(snapshot.stages.len(), 2, "stages 累积");
    assert_eq!(snapshot.stages[0].stage, ArchiveStage::Preflight);
    assert_eq!(snapshot.session_id, None, "会话槽未同步 → None");

    guard.finish(None, Some("收口".to_owned()));
    assert_eq!(
        archive_flow_state_with(app.handle().clone(), root, CHANGE.to_owned()),
        None,
        "终态除名 → None"
    );
}

// ---------------------------------------------------------------------------
// ArchiveSink 转译（AC-9 run 面隔离的 D4 半边）
// ---------------------------------------------------------------------------

/// ArchiveSink 转译：直构 sink——`emit(RunUpdate::SessionEvent)` → 归档
/// broadcast 收 `ArchiveUpdate::SessionEvent` + `current_session` 同步更新；
/// `emit` 其他 RunUpdate 变体 → 归档面零信封（run 信封不外泄）。
#[test]
fn archive_sink转译_session_event直译且其余变体零外泄() {
    let env = Env::new("sink");
    let app = app_with(&env);
    let root = env.root();
    let control = app.state::<Arc<ArchiveControl>>();
    let guard = control.begin(&root, CHANGE).expect("预登记应成功");
    let mut rx = control.subscribe(&root, CHANGE).expect("订阅应成功");

    let sink = ArchiveSink {
        root: root.clone(),
        change: CHANGE.to_owned(),
        control: Arc::clone(&control),
    };

    // SessionEvent：即时转译 + 会话槽同步
    sink.emit(RunUpdate::SessionEvent {
        session_id: "sess-sink-1".to_owned(),
        event: AgentEvent::stamp(
            0,
            AgentEventKind::Raw {
                event_type: "system".to_owned(),
                raw_json: "{}".to_owned(),
            },
        ),
    });
    match rx.try_recv() {
        Ok(ArchiveUpdate::SessionEvent { session_id, .. }) => {
            assert_eq!(session_id, "sess-sink-1", "信封转译保 sessionId");
        }
        other => panic!("应收 SessionEvent 信封，实际: {other:?}"),
    }
    assert_eq!(
        control.current_session(&root, CHANGE).as_deref(),
        Some("sess-sink-1"),
        "会话槽同步更新（停止寻址先行可见）"
    );

    // 其余 RunUpdate 变体：归档面零信封（run 信封不外泄——D4 字面）
    sink.emit(RunUpdate::Step {
        step: ChangeStepState {
            phase: "implement".to_owned(),
            attempt: 1,
            step: ChangeStepKind::Executor,
            status: ChangeStepStatus::Running,
            session_id: Some("sess-x".to_owned()),
            detail: None,
        },
    });
    sink.emit(RunUpdate::Ask {
        question: "q".to_owned(),
        options: Vec::new(),
    });
    sink.emit(RunUpdate::ConfirmWait {
        phase: "implement".to_owned(),
    });
    sink.emit(RunUpdate::Finished {
        status: orchestration::ChangeRunStatus::Completed,
        reason: None,
    });
    assert!(rx.try_recv().is_err(), "非 SessionEvent 变体零归档信封");

    drop(guard);
}

// ---------------------------------------------------------------------------
// DTO 字段面（IPC 边界捕获）
// ---------------------------------------------------------------------------

/// DTO 字段面：Channel 信封 JSON 出线——tag `ipc` 三变体、ArchiveStage 线词、
/// camelCase 字段名（archivedDir / sessionId 等）——IPC 边界捕获（既有捕获型
/// Channel 装置同型）。
#[test]
fn dto字段面_ipc线词与camel_case出线() {
    let env = Env::new("dto");
    env.change_dir(CHANGE);
    let app = app_with(&env);
    let root = env.root();
    seed_change(&app, &root, CHANGE, None);
    let control = app.state::<Arc<ArchiveControl>>();
    let guard = control.begin(&root, CHANGE).expect("预登记应成功");

    // Stage / SessionEvent / Finished 三变体逐一 publish 经捕获型 Channel 出线
    let (channel, captured) = capturing_channel();
    archive_flow_watch_with(
        app.handle().clone(),
        channel,
        root.clone(),
        CHANGE.to_owned(),
    )
    .expect("watch 应 Ok");
    control.publish(
        &root,
        CHANGE,
        ArchiveUpdate::Stage {
            stage: ArchiveStageState {
                stage: ArchiveStage::SpecSync,
                status: ArchiveStageStatus::Running,
                detail: None,
            },
        },
    );
    control.publish(
        &root,
        CHANGE,
        ArchiveUpdate::SessionEvent {
            session_id: "sess-dto".to_owned(),
            event: AgentEvent::stamp(
                0,
                AgentEventKind::Raw {
                    event_type: "system".to_owned(),
                    raw_json: "{}".to_owned(),
                },
            ),
        },
    );
    control.publish(
        &root,
        CHANGE,
        ArchiveUpdate::Finished {
            summary: None,
            error: Some("x".to_owned()),
        },
    );
    wait_for("三信封出线", || {
        captured.lock().expect("捕获锁不可中毒").len() >= 3
    });
    let envelopes = captured.lock().expect("捕获锁不可中毒").clone();

    assert_eq!(envelopes[0]["ipc"], "stage");
    assert_eq!(
        envelopes[0]["stage"]["stage"], "specSync",
        "阶段线词 camelCase"
    );
    assert_eq!(envelopes[0]["stage"]["status"], "running");
    assert!(
        envelopes[0]["stage"].get("detail").is_some(),
        "detail 字段出线（null 态）"
    );
    assert_eq!(envelopes[1]["ipc"], "sessionEvent", "变体 tag camelCase");
    assert_eq!(
        envelopes[1]["sessionId"], "sess-dto",
        "camelCase sessionId 字段"
    );
    assert!(envelopes[1]["event"].is_object(), "AgentEvent 载荷透传");
    assert_eq!(envelopes[2]["ipc"], "finished");
    assert!(envelopes[2]["summary"].is_null() && envelopes[2]["error"] == "x");

    // preflight DTO 的 camelCase 字段面（serde 直出线核对——确认对话数据面）
    let read = archive_flow_preflight_with(app.handle().clone(), root.clone(), CHANGE.to_owned())
        .expect("active 建档 → Some");
    let wire = serde_json::to_value(&read).expect("preflight 出线");
    for field in [
        "name",
        "completed",
        "incompletePhases",
        "missingArtifacts",
        "deltaSpecs",
        "worktree",
        "branch",
        "mergeTarget",
        "runActive",
    ] {
        assert!(
            wire.get(field).is_some(),
            "preflight camelCase 字段出线: {field}"
        );
    }
    drop(guard);
}
