use std::ffi::OsString;
use std::path::Path;
use std::sync::{Arc, Mutex};

use agent::{
    AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunStatus,
    SessionProvenance, StopRegistry,
};
use orchestration::port::{RunEventSink, WorkerAgentPort, WorkerRole, WorkerTurnRequest};
use orchestration::state::RunUpdate;
use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord,
    SessionConfigSnapshot, SessionRecord, WorkspaceStores,
};

/// 父模块导入面（KernelWorkerPort / ComposedTurn 等经 worker 模块可见）。
use super::*;
use crate::compose_turn;

/// 会话 provenance 来源（与 walker 的 `SOURCE_CHANGE` 同一契约字面量）。
const SOURCE_CHANGE: &str = "change";

/// PATH 环境变量修改串行化（CLI 臂用例共享；同进程测试并行跑时 set_var 为
/// 进程全局操作）。async-aware 锁：隔离窗口跨 await 持有。与 git_diff_test
/// 的 git 缺失窗口共用 crate 级锁（worker_test 原 PATH_LOCK 静态并轨）。
use crate::TEST_PATH_LOCK as PATH_LOCK;

// ---------------------------------------------------------------------------
// 装置：tempdir 真库双环境 + 假 sink + PATH 隔离 + 假 CLI shim
// ---------------------------------------------------------------------------

/// 数据根 + workspace 根双临时目录装置。
struct DualEnv {
    data_root: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl DualEnv {
    fn new(tag: &str) -> Self {
        let data_root = tempfile::Builder::new()
            .prefix(&format!("worker-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("worker-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_root, ws_root }
    }

    fn root(&self) -> String {
        self.ws_root.path().to_string_lossy().into_owned()
    }

    fn stores(&self) -> WorkspaceStores {
        WorkspaceStores::open(self.data_root.path())
            .unwrap_or_else(|e| panic!("WorkspaceStores::open 应成功: {e}"))
    }
}

/// 假 `RunEventSink`：Arc + Mutex Vec 捕获 run 状态流出（SessionEvent 透传
/// 断言面）。
struct CapturingSink {
    updates: Arc<Mutex<Vec<RunUpdate>>>,
}

impl CapturingSink {
    /// 构造即铸 trait object（KernelWorkerPort 注入面），返回（sink, 捕获缓冲）。
    fn capturing() -> (Arc<dyn RunEventSink>, Arc<Mutex<Vec<RunUpdate>>>) {
        let updates = Arc::new(Mutex::new(Vec::new()));
        (
            Arc::new(Self {
                updates: Arc::clone(&updates),
            }),
            updates,
        )
    }
}

impl RunEventSink for CapturingSink {
    fn emit(&self, update: RunUpdate) {
        self.updates
            .lock()
            .expect("事件捕获锁不可中毒")
            .push(update);
    }
}

/// PATH 环境守卫：构造即替换，drop 恢复原值（断言失败也不遗留污染）。
struct PathIsolation {
    original: Option<OsString>,
}

impl PathIsolation {
    /// PATH 置空（CLI / shim 均不可发现的隔离态）。
    fn empty() -> Self {
        let original = std::env::var_os("PATH");
        std::env::set_var("PATH", "");
        Self { original }
    }

    /// PATH 指向单目录（假 CLI shim 发现态）。
    fn replace_with(dir: &Path) -> Self {
        let original = std::env::var_os("PATH");
        let value = std::env::join_paths([dir]).expect("拼接 PATH 值失败");
        std::env::set_var("PATH", value);
        Self { original }
    }
}

impl Drop for PathIsolation {
    fn drop(&mut self) {
        match self.original.take() {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }
}

/// 平台 JSONL echo 行（Windows cmd 双引号直出；其余平台单引号包裹）。
#[cfg(windows)]
fn shim_echo(json: &str) -> String {
    format!("echo {json}")
}

/// 平台 JSONL echo 行（Unix 版）。
#[cfg(not(windows))]
fn shim_echo(json: &str) -> String {
    format!("echo '{json}'")
}

/// 合成 claude CLI shim 脚本
fn claude_shim_script(pause_mid_turn: bool, pause_tail: bool) -> String {
    const INIT: &str = r#"{"type":"system","subtype":"init","model":"claude-opus","session_id":"s-1","tools":["Bash"],"mcp_servers":[]}"#;
    const ASSISTANT: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"final verdict pass"}]}}"#;
    const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":2,"duration_ms":99,"total_cost_usd":0.1,"usage":{},"session_id":"s-1"}"#;
    #[cfg(windows)]
    let pause: &str = r#""%SystemRoot%\System32\ping.exe" -n 5 127.0.0.1 >nul"#;
    #[cfg(not(windows))]
    let pause: &str = "/bin/sleep 4";

    let mut lines: Vec<String> = Vec::new();
    #[cfg(windows)]
    lines.push("@echo off".to_owned());
    #[cfg(not(windows))]
    lines.push("#!/bin/sh".to_owned());
    lines.push(shim_echo(INIT));
    if pause_mid_turn {
        lines.push(pause.to_owned());
    }
    lines.push(shim_echo(ASSISTANT));
    lines.push(shim_echo(RESULT));
    if pause_tail {
        lines.push(pause.to_owned());
    }
    lines.join("\n")
}

/// 合成 claude 入口目录（PATH 注入目标）：Windows 落 `claude.cmd`，其余平台
/// 落带可执行位的 `claude`。
fn claude_shim_dir(tag: &str, pause_mid_turn: bool, pause_tail: bool) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("worker-test-{tag}-shim-"))
        .tempdir()
        .expect("创建 shim 目录失败");
    let script = claude_shim_script(pause_mid_turn, pause_tail);
    #[cfg(windows)]
    {
        std::fs::write(dir.path().join("claude.cmd"), script).expect("写 shim 失败");
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().join("claude");
        std::fs::write(&path, script).expect("写 shim 失败");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("置 shim 可执行位失败");
    }
    dir
}

/// cli 默认实例 fixture（缺省解析可走通的零配置臂）。
fn seed_cli_default(stores: &WorkspaceStores, name: &str) -> i64 {
    let id = stores
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            name.to_owned(),
            AgentEngineKind::Cli,
            None,
        ))
        .expect("落 cli 实例 fixture")
        .id;
    stores
        .global()
        .set_default_agent_instance(id)
        .expect("置默认实例 fixture");
    id
}

/// 空配置 sdk 默认实例 fixture（open 阶段 ConfigMissing 的唯一可达面）。
fn seed_sdk_default_with_blank_provider(stores: &WorkspaceStores) {
    let provider = stores
        .global()
        .upsert_agent_provider(AgentProviderRecord::new(
            "空配置供应".to_owned(),
            String::new(),
            String::new(),
            AgentModelTiers {
                high: String::new(),
                medium: String::new(),
                low: String::new(),
            },
        ))
        .expect("落空配置 provider fixture")
        .id;
    let id = stores
        .global()
        .upsert_agent_instance(AgentInstanceRecord::new(
            "空配置实例".to_owned(),
            AgentEngineKind::Sdk,
            Some(provider),
        ))
        .expect("落 sdk 实例 fixture")
        .id;
    stores
        .global()
        .set_default_agent_instance(id)
        .expect("置默认实例 fixture");
}

/// cli 快照会话行 fixture（Continue 引用底座）。
fn seed_cli_session(store: &store::Store, id: &str, source_ref: Option<&str>) {
    store
        .create_session(&SessionRecord {
            id: id.to_owned(),
            engine_session_id: Some("s-cli-1".to_owned()),
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Cli,
                model: None,
                permission_mode: AgentPermissionMode::BypassPermissions,
            },
            source: SOURCE_CHANGE.to_owned(),
            source_ref: source_ref.map(str::to_owned),
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("落 cli 会话行 fixture");
}

/// change 来源的轮请求 fixture（sourceRef 按 `<change>/<phase>/<role>/<attempt>`
/// 定式入参）。
fn turn_request(root: &str, source_ref: &str, role: WorkerRole) -> WorkerTurnRequest {
    WorkerTurnRequest {
        root: root.to_owned(),
        prompt: "run one turn".to_owned(),
        provenance: SessionProvenance {
            source: SOURCE_CHANGE.to_owned(),
            source_ref: Some(source_ref.to_owned()),
        },
        permission: AgentPermissionMode::BypassPermissions,
        continue_session: None,
        agent: None,
        role,
    }
}

/// 摘取 sink 捕获中的 SessionEvent 流（session_id, event）。
fn session_events_of(updates: &[RunUpdate]) -> Vec<(String, agent::AgentEvent)> {
    updates
        .iter()
        .filter_map(|update| match update {
            RunUpdate::SessionEvent { session_id, event } => {
                Some((session_id.clone(), event.clone()))
            }
            _ => None,
        })
        .collect()
}

/// 轮询探针至有值（后台透传/收敛的观测窗口）。
async fn wait_until<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "等待{what}超时（30s）"
        );
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

// ---------------------------------------------------------------------------
// 装配半边：provenance / permission / continue 引用（AC-7）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn change_provenance_and_permission_are_assembled_into_session_row() {
    let env = DualEnv::new("provenance");
    let stores = env.stores();
    seed_cli_default(&stores, "装配实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, updates) = CapturingSink::capturing();
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::clone(&registry), &env.root(), None).expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();
    let request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let shim = claude_shim_dir("provenance", true, false);
    let _isolation = PathIsolation::replace_with(shim.path());

    let task = tokio::spawn(async move { port.run(request).await });
    // 首个会话事件透传即 begin 已完成（会话行按 provenance 定式落库），随后
    // 经停止窗口收敛（shim 轮中停顿留出置位窗口——沿 sealed_pump / mid_drive
    // 同款装置）
    let session_id = wait_until("首个会话事件透传", || {
        session_events_of(&updates.lock().expect("事件捕获锁不可中毒"))
            .first()
            .map(|(session_id, _)| session_id.clone())
    })
    .await;
    assert!(
        registry.request_stop(&session_id),
        "begin 同步段已登记停止句柄"
    );

    let outcome = task
        .await
        .expect("run 任务正常结束")
        .expect("装配会话应发起");
    drop(_isolation);
    drop(_guard);

    let store = stores.for_root(&root).expect("for_root 应成功");
    let row = store
        .find_session(&outcome.session_id)
        .expect("find_session 应成功")
        .expect("会话行已落库");
    assert_eq!(
        row.source, SOURCE_CHANGE,
        "provenance source 恒 change（AC-7 反查面）"
    );
    assert_eq!(
        row.source_ref.as_deref(),
        Some("chg-flow/implement/executor/1"),
        "sourceRef 按 <change>/<phase>/<role>/<attempt> 定式落库"
    );
    assert_eq!(
        row.config_snapshot.permission_mode,
        AgentPermissionMode::BypassPermissions,
        "permission 恒 bypassPermissions"
    );
    assert_eq!(row.config_snapshot.engine, AgentEngineKind::Cli);
}

#[tokio::test]
async fn continue_session_reuses_existing_row_without_duplicates() {
    let env = DualEnv::new("continue");
    let stores = env.stores();
    seed_cli_default(&stores, "续话实例");
    let registry = Arc::new(StopRegistry::new());
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    seed_cli_session(
        &store,
        "ses-flow-continue",
        Some("chg-flow/proposal/executor/0"),
    );
    let (sink, updates) = CapturingSink::capturing();
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::clone(&registry), &env.root(), None).expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();

    let mut request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);
    request.continue_session = Some("ses-flow-continue".to_owned());

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let shim = claude_shim_dir("continue", true, false);
    let _isolation = PathIsolation::replace_with(shim.path());

    let task = tokio::spawn(async move { port.run(request).await });
    // 首个会话事件透传即 Continue 会话已发起，随后经停止窗口收敛
    let event_session_id = wait_until("首个会话事件透传", || {
        session_events_of(&updates.lock().expect("事件捕获锁不可中毒"))
            .first()
            .map(|(session_id, _)| session_id.clone())
    })
    .await;
    assert!(
        registry.request_stop(&event_session_id),
        "begin 同步段已登记停止句柄"
    );

    let outcome = task
        .await
        .expect("run 任务正常结束")
        .expect("Continue 引用应复用既有会话行");
    drop(_isolation);
    drop(_guard);

    assert_eq!(
        outcome.session_id, "ses-flow-continue",
        "continue_session Some → Continue 引用（沿用既有会话 id）"
    );
    let summaries = store.list_sessions(None, None).expect("清单应成功");
    assert_eq!(summaries.len(), 1, "Continue 不重复建会话行");
    assert_eq!(summaries[0].turns.len(), 1, "续轮行照开");
}

// ---------------------------------------------------------------------------
// 解析失败显式 Err（不静默 / 零半成品）
// ---------------------------------------------------------------------------

#[test]
fn empty_store_default_resolution_errs_with_management_hint() {
    let env = DualEnv::new("no-default");
    let stores = env.stores();

    let error = match compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None) {
        Err(error) => error,
        Ok(_) => panic!("空库缺省解析必须 Err"),
    };
    assert!(
        error.contains("默认 agent") && error.contains("/agents"),
        "Err 携管理页引导语义（显式失败不静默），实际: {error}"
    );

    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "解析失败零落库"
    );
}

#[tokio::test]
async fn open_stage_failure_propagates_err_without_half_records() {
    let env = DualEnv::new("open-fail");
    let stores = env.stores();
    seed_sdk_default_with_blank_provider(&stores);
    let (sink, _updates) = CapturingSink::capturing();
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
            .expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);

    let result = port
        .run(turn_request(
            &env.root(),
            "chg-flow/implement/executor/1",
            WorkerRole::Executor,
        ))
        .await;

    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("open 阶段失败必须 Err 传播"),
    };
    assert!(
        error.contains("配置缺失"),
        "AgentStartError 以 Display 映射 Err 通道（启动失败单一变体），实际: {error}"
    );

    // 内核契约承接：open 阶段失败不产生任何记录
    let store = stores.for_root(&env.root()).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "open 阶段失败零半成品记录"
    );
}

// ---------------------------------------------------------------------------
// CLI 引擎不可达（PATH 隔离 → 轮前预检 Err + 零半成品记录）
// ---------------------------------------------------------------------------

/// CLI 引擎不可达（PATH 隔离）：CLI 引擎臂轮前预检 discover 失败 → Err
/// 传播且 `begin` 未发生——会话 / 轮行零半成品记录、sink 零透传（CliMissing
/// 收敛 Err + 零半成品记录，不以合成 failed 轮的 Ok 形态带病续走）。
#[tokio::test]
async fn path_isolated_cli_missing_errs_with_zero_half_records() {
    let env = DualEnv::new("cli-missing");
    let stores = env.stores();
    seed_cli_default(&stores, "隔离实例");
    let (sink, updates) = CapturingSink::capturing();
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::new(StopRegistry::new()), &env.root(), None)
            .expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let _isolation = PathIsolation::empty();
    let result = port
        .run(turn_request(
            &root,
            "chg-flow/implement/executor/1",
            WorkerRole::Executor,
        ))
        .await;
    drop(_isolation);
    drop(_guard);

    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("PATH 隔离触发 CliMissing 必须 Err（不以合成终态 Ok 带病续走）"),
    };
    assert!(
        error.contains("CLI 未找到") && error.contains("claude"),
        "Err 携 CLI 缺失记因（显式失败不静默），实际: {error}"
    );

    // 预检先行：begin 未发生 → 会话 / 轮行零半成品记录（内核契约承接）
    let store = stores.for_root(&root).expect("for_root 应成功");
    assert!(
        store
            .list_sessions(None, None)
            .expect("清单应成功")
            .is_empty(),
        "CliMissing 零半成品记录（预检先于 begin）"
    );
    assert!(
        updates.lock().expect("事件捕获锁不可中毒").is_empty(),
        "零 SessionEvent 透传（begin 未发生）"
    );
}

// ---------------------------------------------------------------------------
// 泵收集与透传（假 CLI shim 预录密封事件；AC-7 反查数据面 / AC-5 实时流来源）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sealed_pump_collects_transcript_and_forwards_session_events() {
    let env = DualEnv::new("pump");
    let stores = env.stores();
    seed_cli_default(&stores, "泵收集实例");
    let (sink, updates) = CapturingSink::capturing();
    let registry = Arc::new(StopRegistry::new());
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::clone(&registry), &env.root(), None).expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();
    let request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let shim = claude_shim_dir("pump", false, true);
    let _isolation = PathIsolation::replace_with(shim.path());

    let task = tokio::spawn(async move { port.run(request).await });
    // 轮末停顿窗口内置位停止：三条密封事件已全量透传，CLI 会话泵随停止请求
    // 收口（观察流闭合），状态机已收敛 → 终态 Completed
    wait_until("三条密封事件透传", || {
        let count = session_events_of(&updates.lock().expect("事件捕获锁不可中毒")).len();
        (count >= 3).then_some(count)
    })
    .await;
    let session_id = session_events_of(&updates.lock().expect("事件捕获锁不可中毒"))[0]
        .0
        .clone();
    assert!(
        registry.request_stop(&session_id),
        "begin 同步段已登记停止句柄"
    );

    let outcome = task
        .await
        .expect("run 任务正常结束")
        .expect("假 CLI 会话应正常收敛");
    drop(_isolation);
    drop(_guard);

    // 终态与转录：密封事件全集收集 + final_message 承接末条 assistant 文本
    assert_eq!(outcome.status, AgentRunStatus::Completed);
    assert_eq!(
        outcome.transcript.len(),
        3,
        "init / assistant / result 全集"
    );
    assert!(matches!(
        &outcome.transcript[0].kind,
        AgentEventKind::RunStarted { model: Some(model), .. } if model == "claude-opus"
    ));
    assert!(matches!(
        &outcome.transcript[1].kind,
        AgentEventKind::Message { role, blocks, .. }
            if role == &AgentMessageRole::Assistant
                && matches!(&blocks[0], AgentBlock::Text { text } if text == "final verdict pass")
    ));
    assert!(matches!(
        &outcome.transcript[2].kind,
        AgentEventKind::TurnDone {
            is_error: false,
            num_turns: Some(2),
            ..
        }
    ));
    assert_eq!(
        outcome.final_message.as_deref(),
        Some("final verdict pass"),
        "final_message 承接终态载体"
    );

    // sink 透传：每条观察以 RunUpdate::SessionEvent 流出且载荷保真
    let events = session_events_of(&updates.lock().expect("事件捕获锁不可中毒"));
    assert_eq!(events.len(), 3, "三条观察全量透传");
    assert!(
        events
            .iter()
            .all(|(session_id, _)| session_id == &outcome.session_id),
        "透传 session_id 承接运行会话"
    );
    assert_eq!(
        events
            .into_iter()
            .map(|(_, event)| event)
            .collect::<Vec<_>>(),
        outcome.transcript,
        "透传事件与收集转录同源保真"
    );

    // 真库落盘：change provenance 反查命中 + 轮行终态 + 双 id 映射
    let store = stores.for_root(&root).expect("for_root 应成功");
    let summaries = store
        .list_sessions(Some(SOURCE_CHANGE), Some("chg-flow/implement/executor/1"))
        .expect("反查应成功");
    assert_eq!(
        summaries.len(),
        1,
        "source='change' + sourceRef 精确反查命中"
    );
    assert_eq!(summaries[0].turns[0].status, AgentRunStatus::Completed);
    assert_eq!(
        summaries[0].row.remote_session_id.as_deref(),
        Some("s-1"),
        "引擎侧标识上报落双 id 映射"
    );
}

// ---------------------------------------------------------------------------
// 停止收敛（AC-7 停止半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mid_drive_stop_converges_stopped_and_keeps_collected_events() {
    let env = DualEnv::new("stop");
    let stores = env.stores();
    seed_cli_default(&stores, "停止实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, updates) = CapturingSink::capturing();
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::clone(&registry), &env.root(), None).expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();
    let request = turn_request(&root, "chg-flow/implement/executor/1", WorkerRole::Executor);

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let shim = claude_shim_dir("stop", true, false);
    let _isolation = PathIsolation::replace_with(shim.path());

    let task = tokio::spawn(async move { port.run(request).await });
    // 首个会话事件透传即取得运行会话槽（命令层 sink 桥的同步半边），随后置位停止
    let session_id = wait_until("首个会话事件透传", || {
        session_events_of(&updates.lock().expect("事件捕获锁不可中毒"))
            .first()
            .map(|(session_id, _)| session_id.clone())
    })
    .await;
    assert!(
        registry.request_stop(&session_id),
        "begin 同步段已登记停止句柄（键 = core session id）"
    );

    let outcome = task
        .await
        .expect("run 任务正常结束")
        .expect("停止收敛仍 Ok 携终态");
    drop(_isolation);
    drop(_guard);

    assert_eq!(
        outcome.status,
        AgentRunStatus::Stopped,
        "停止置位后驱动终止以 stopped 收敛"
    );
    assert!(
        matches!(
            &outcome.transcript[0].kind,
            AgentEventKind::RunStarted { .. }
        ),
        "已收集事件保留（停止不清除）"
    );
    assert!(
        !registry.request_stop(&session_id),
        "终态除名后停止寻址 miss"
    );
    let events = session_events_of(&updates.lock().expect("事件捕获锁不可中毒"));
    assert!(
        events.iter().any(|(id, event)| id == &outcome.session_id
            && matches!(event.kind, AgentEventKind::RunStarted { .. })),
        "停止前的观察已透传 sink（实时流不丢）"
    );
}

// ---------------------------------------------------------------------------
// 三角色 provenance 定式（D3 / AC-7 反查面）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn three_roles_map_verbatim_into_source_ref_segments() {
    let env = DualEnv::new("roles");
    let stores = env.stores();
    seed_cli_default(&stores, "角色实例");
    let registry = Arc::new(StopRegistry::new());
    let (sink, updates) = CapturingSink::capturing();
    let composed: ComposedTurn =
        compose_turn(&stores, Arc::clone(&registry), &env.root(), None).expect("解析应成功");
    let port = KernelWorkerPort::new(composed, sink);
    let root = env.root();

    let roles = [
        ("chg-flow/implement/executor/1", WorkerRole::Executor),
        ("chg-flow/implement/evaluator/2", WorkerRole::Evaluator),
        ("chg-flow/implement/decision/3", WorkerRole::Decision),
    ];

    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let shim = claude_shim_dir("roles", true, false);
    let _isolation = PathIsolation::replace_with(shim.path());
    let port = Arc::new(port);
    let mut seen: Vec<String> = Vec::new();
    for (source_ref, role) in roles {
        let request = turn_request(&root, source_ref, role);
        let task = tokio::spawn({
            let port = Arc::clone(&port);
            async move { port.run(request).await }
        });
        // 首个「未见会话」事件透传即本轮 begin 已完成，随后经停止窗口收敛
        let session_id = wait_until("本轮首个会话事件透传", || {
            session_events_of(&updates.lock().expect("事件捕获锁不可中毒"))
                .iter()
                .map(|(session_id, _)| session_id.clone())
                .find(|session_id| !seen.contains(session_id))
        })
        .await;
        seen.push(session_id.clone());
        assert!(
            registry.request_stop(&session_id),
            "begin 同步段已登记停止句柄"
        );
        let outcome = task
            .await
            .expect("run 任务正常结束")
            .expect("各角色会话均应发起");
        assert!(outcome.session_id.starts_with("ses-"), "每轮独立新会话");
    }
    drop(_isolation);
    drop(_guard);

    let store = stores.for_root(&root).expect("for_root 应成功");
    let summaries = store
        .list_sessions(Some(SOURCE_CHANGE), None)
        .expect("反查应成功");
    assert_eq!(summaries.len(), 3, "三角色各建一会话");
    let refs: Vec<&str> = summaries
        .iter()
        .map(|summary| {
            summary
                .row
                .provenance
                .source_ref
                .as_deref()
                .expect("sourceRef 在场")
        })
        .collect();
    for expected in [
        "chg-flow/implement/executor/1",
        "chg-flow/implement/evaluator/2",
        "chg-flow/implement/decision/3",
    ] {
        assert!(refs.contains(&expected), "sourceRef 逐字在册: {expected}");
    }
    for summary in &summaries {
        let segments: Vec<&str> = summary
            .row
            .provenance
            .source_ref
            .as_deref()
            .expect("sourceRef 在场")
            .split('/')
            .collect();
        assert_eq!(segments[0], "chg-flow", "第一段 = change 名");
        assert_eq!(segments[1], "implement", "第二段 = phase");
        assert!(
            matches!(segments[2], "executor" | "evaluator" | "decision"),
            "第三段 = 角色线格式词，实际: {}",
            segments[2]
        );
    }
    // 反查精确过滤：单角色 × attempt 命中唯一会话（转录联动反查契约）
    let evaluator_only = store
        .list_sessions(Some(SOURCE_CHANGE), Some("chg-flow/implement/evaluator/2"))
        .expect("精确反查应成功");
    assert_eq!(evaluator_only.len(), 1);
    assert_eq!(
        evaluator_only[0].row.provenance.source_ref.as_deref(),
        Some("chg-flow/implement/evaluator/2")
    );
}
