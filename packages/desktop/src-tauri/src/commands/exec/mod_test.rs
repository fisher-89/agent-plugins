//! exec 命令面（agent_start / agent_stop / agent_runs / agent_run_events /
//! agent_run_chain）的单元 + 「exec 查询命令面 → workspace 库 root 寻址」与
//! 「agent_start → for_root 落库」组合关系测试（AC-5）。
//!
//! `#[tauri::command]` 保留原函数可直调：以 `tauri::test::mock_app()`
//! （MockRuntime，无窗口无事件循环）manage 真实 WorkspaceStores（tempdir 真
//! 开全局库，各 workspace 库经 for_root 惰性开）与 RunStopRegistry（须先于
//! start 托管，沿既有装置）后经 `app.state::<WorkspaceStores>()` 取 State。
//! agent_start 正向（真实 CLI）不直测：命令体首参为 Wry `AppHandle`
//! （MockRuntime 不可构造），失败分支以命令委托的薄入口 `start_agent_run`
//! 承载（隔离 PATH 触发 CliMissing 走 Err 分支，PATH 环境变量修改以共享
//! 互斥锁串行化）；成功链路经 `start_agent_run_with` 泛型缝（假 runner，装置
//! 复用 agent_test.rs）。无 root 入参的查询 / 裸 id 寻址用例形态随 id 域
//! 内化退役（AC-5 复合键化）。

use std::fs;
use std::path::{Path, PathBuf};

use tauri::{App, Manager};

use ::agent::{
    AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunParams, AgentRunStatus,
    RunHandle,
};
use store::{AgentRunRecord, Store, WorkspaceStores};

use super::{
    agent_run_chain, agent_run_events, agent_runs, agent_stop, is_blank_root, RunStopRegistry,
};
use crate::commands::exec::agent::agent_test::{
    capturing_channel, pushed_event_bodies, start_and_wait_terminal, FakeRunner, PATH_LOCK,
};
use crate::commands::exec::agent::{start_agent_run, start_agent_run_with, RunProvenance};

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

/// 数据根 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    data_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("exec-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("exec-cmd-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_dir, ws_root }
    }

    /// 在 workspace 根下创建真实目录并返回路径（for_root canonical 基准）。
    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }

    /// 目录路径转命令面 String root。
    fn root_of(&self, name: &str) -> String {
        self.ws(name).to_string_lossy().into_owned()
    }
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 WorkspaceStores（打开 env
/// 数据根的全局库）与 RunStopRegistry（编排同步段注册停止句柄，须先于 start
/// 托管）。
fn app_with_stores(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app.manage(RunStopRegistry::default());
    app
}

/// running 形态底座记录（id 由 begin 分配；三字段直写契约枚举）。
fn running_run(prompt: &str, started_at: i64, root: &str) -> AgentRunRecord {
    AgentRunRecord {
        id: 0,
        prompt: prompt.to_owned(),
        cwd: root.to_owned(),
        env: AgentEnvMode::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
        status: AgentRunStatus::Running,
        started_at,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
        source: "debug".to_owned(),
        source_ref: None,
        parent_run_id: None,
    }
}

/// 落一份 completed run 记录并返回（begin → finish，落 root 的 workspace 库）。
fn seed_run(store: &Store, prompt: &str, started_at: i64, root: &str) -> AgentRunRecord {
    let mut record = store
        .begin_agent_run(&running_run(prompt, started_at, root))
        .expect("begin 应成功");
    record.status = AgentRunStatus::Completed;
    record.finished_at = Some(started_at + 500);
    record.num_turns = Some(2);
    store
        .finish_agent_run(record.id, &record)
        .expect("finish 应成功");
    record
}

/// 落一条带来源三元组的 run 记录并返回（begin 分配 id）。
fn begin_chain_run(
    store: &Store,
    prompt: &str,
    started_at: i64,
    root: &str,
    source: &str,
    source_ref: Option<&str>,
    parent_run_id: Option<i64>,
) -> AgentRunRecord {
    let mut requested = running_run(prompt, started_at, root);
    requested.source = source.to_owned();
    requested.source_ref = source_ref.map(str::to_owned);
    requested.parent_run_id = parent_run_id;
    store.begin_agent_run(&requested).expect("begin 应成功")
}

/// 落一批事件（AgentEvent 序列类型化落库，模拟 tee store sink）。
fn seed_events(store: &Store, run_id: i64, events: &[AgentEvent]) {
    store
        .append_agent_run_events(run_id, events)
        .expect("append 应成功");
}

/// 隔离/重开复核用事件集：四变体（含 Raw）均携带 run 归属标记（tag = "A" / "B"），
/// 两 run 复用同一 seq 区间（0..=3）——若命令面未按 run_id 隔离，串扰必然显现。
fn isolation_events(tag: &str) -> Vec<AgentEvent> {
    vec![
        AgentEvent::stamp(
            0,
            AgentEventKind::RunStarted {
                model: Some("claude-opus".to_owned()),
                session_id: Some(format!("s-{tag}")),
                tools: vec!["Bash".to_owned()],
                mcp_servers: Vec::new(),
            },
        ),
        AgentEvent::stamp(
            1,
            AgentEventKind::Message {
                role: tag.to_owned(),
                blocks: Vec::new(),
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            2,
            AgentEventKind::Raw {
                event_type: "mystery".to_owned(),
                raw_json: format!(r#"{{"type":"mystery","run":"{tag}"}}"#),
            },
        ),
        AgentEvent::stamp(
            3,
            AgentEventKind::RunResult {
                subtype: "success".to_owned(),
                is_error: false,
                num_turns: Some(1),
                duration_ms: None,
                cost_usd: None,
                usage: serde_json::Value::Null,
                session_id: Some(format!("s-{tag}")),
            },
        ),
    ]
}

fn run_started(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::RunStarted {
            model: Some("claude-opus".to_owned()),
            session_id: Some("s-1".to_owned()),
            tools: vec!["Bash".to_owned()],
            mcp_servers: Vec::new(),
        },
    )
}

fn message(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Message {
            role: "user".to_owned(),
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    )
}

fn run_result(seq: u64, is_error: bool) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::RunResult {
            subtype: "success".to_owned(),
            is_error,
            num_turns: Some(1),
            duration_ms: None,
            cost_usd: None,
            usage: serde_json::Value::Null,
            session_id: Some("s-1".to_owned()),
        },
    )
}

fn raw(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Raw {
            event_type: "mystery".to_owned(),
            raw_json: r#"{"type":"mystery"}"#.to_owned(),
        },
    )
}

/// 逐事件断言归属标记：任一事件不得携带另一 run 的内容（隔离复核）。
fn assert_events_belong_to(events: &[AgentEvent], tag: &str) {
    for event in events {
        match &event.kind {
            AgentEventKind::RunStarted { session_id, .. } => assert_eq!(
                session_id.as_deref(),
                Some(&format!("s-{tag}")[..]),
                "runStarted 归属标记错乱: {event:?}"
            ),
            AgentEventKind::Message { role, .. } => {
                assert_eq!(role.as_str(), tag, "message 归属标记错乱: {event:?}")
            }
            AgentEventKind::Raw { raw_json, .. } => assert!(
                raw_json.contains(&format!(r#""run":"{tag}""#)),
                "raw 归属标记错乱: {event:?}"
            ),
            AgentEventKind::RunResult { session_id, .. } => assert_eq!(
                session_id.as_deref(),
                Some(&format!("s-{tag}")[..]),
                "runResult 归属标记错乱: {event:?}"
            ),
            // 隔离 fixture 不产出 systemNotice：出现即 fixture 口径漂移
            AgentEventKind::SystemNotice { .. } => {
                panic!("isolation_events 不产出 systemNotice，事件: {event:?}")
            }
        }
    }
}

/// 反向断言：任一事件不得携带 tag 标记（不串入该 run 的任何事件）。
fn assert_no_events_belong_to(events: &[AgentEvent], tag: &str) {
    let marker = format!("s-{tag}");
    for event in events {
        let belongs = match &event.kind {
            AgentEventKind::RunStarted { session_id, .. }
            | AgentEventKind::RunResult { session_id, .. } => {
                session_id.as_deref() == Some(&marker[..])
            }
            AgentEventKind::Message { role, .. } => role.as_str() == tag,
            AgentEventKind::Raw { raw_json, .. } => raw_json.contains(&format!(r#""run":"{tag}""#)),
            // systemNotice 不携带任何 run 标记，恒不归属
            AgentEventKind::SystemNotice { .. } => false,
        };
        assert!(!belongs, "串入 run {tag} 的事件: {event:?}");
    }
}

// ---------------------------------------------------------------------------
// agent_runs：root 寻址当前 workspace 库，清单收窄为本 workspace 历史（AC-5）
// ---------------------------------------------------------------------------

#[test]
fn agent_runs携root结果与直连清单的serde值一致且按开始时间降序() {
    let env = Env::new("runs-passthrough");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");

    let late = seed_run(&store, "晚启动", 300, &root);
    let early = seed_run(&store, "早启动", 100, &root);

    let via_command = agent_runs(state.clone(), root.clone()).expect("agent_runs 应成功");
    let via_store = store.list_agent_runs().expect("直连 list 应成功");
    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄包装不加工：命令面与直连 store serde 值一致"
    );
    let ids: Vec<i64> = via_command.iter().map(|record| record.id).collect();
    assert_eq!(ids, vec![late.id, early.id], "startedAt 降序");
}

#[test]
fn 两workspace各有runs同id并行时agent_runs互不可见() {
    let env = Env::new("runs-parallel");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root_a = env.root_of("ws-a");
    let root_b = env.root_of("ws-b");
    let store_a = state.for_root(&root_a).expect("A for_root 应成功");
    let store_b = state.for_root(&root_b).expect("B for_root 应成功");

    // 两库同 id=1 并行（库域内自增）：裸 id 跨库歧义由 root 消解
    let run_a = seed_run(&store_a, "A 库首轮", 100, &root_a);
    let run_b = seed_run(&store_b, "B 库首轮", 200, &root_b);
    assert_eq!((run_a.id, run_b.id), (1, 1), "同 id 并行");

    let list_a = agent_runs(state.clone(), root_a).expect("A 清单应成功");
    let list_b = agent_runs(state.clone(), root_b).expect("B 清单应成功");
    let prompts_a: Vec<&str> = list_a.iter().map(|record| record.prompt.as_str()).collect();
    let prompts_b: Vec<&str> = list_b.iter().map(|record| record.prompt.as_str()).collect();
    assert_eq!(prompts_a, vec!["A 库首轮"], "A 清单仅本库 runs");
    assert_eq!(prompts_b, vec!["B 库首轮"], "B 清单仅本库 runs，互不可见");
}

#[test]
fn 空workspace库与未选中workspace的agent_runs返回空数组() {
    let env = Env::new("runs-empty");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("empty-ws");

    let empty_store = agent_runs(state.clone(), root).expect("空库 agent_runs 应成功");
    assert_eq!(
        serde_json::to_value(&empty_store).unwrap(),
        serde_json::json!([])
    );
}

// ---------------------------------------------------------------------------
// agent_run_events：root + run_id 寻址该库重放
// ---------------------------------------------------------------------------

#[test]
fn 落库事件经命令面携root读回为类型化agent_event按seq升序五变体保真() {
    let env = Env::new("events-replay");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");

    let run = store
        .begin_agent_run(&running_run("重放验证", 1727000000000, &root))
        .expect("begin 应成功");
    let seeded = vec![run_started(0), message(1), run_result(2, false), raw(3)];
    seed_events(&store, run.id, &seeded);

    let replayed =
        agent_run_events(state.clone(), root.clone(), run.id).expect("agent_run_events 应成功");

    assert_eq!(replayed, seeded, "类型化落库后经命令面重放逐字段保真");
    let seqs: Vec<u64> = replayed.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3], "seq 升序");
    // 五变体判别保真（含 Raw 变体透传形态）
    assert!(matches!(
        replayed[0].kind,
        AgentEventKind::RunStarted { .. }
    ));
    assert!(matches!(replayed[1].kind, AgentEventKind::Message { .. }));
    assert!(matches!(replayed[2].kind, AgentEventKind::RunResult { .. }));
    assert!(matches!(
        &replayed[3].kind,
        AgentEventKind::Raw { event_type, raw_json }
            if event_type == "mystery" && raw_json == r#"{"type":"mystery"}"#
    ));
}

#[test]
fn 不存在run时agent_run_events返回空数组() {
    let env = Env::new("events-miss");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let replayed = agent_run_events(state.clone(), root, 404).expect("miss 幂等不报错");
    assert!(replayed.is_empty());
}

#[test]
fn 两run事件隔离经命令面复核以run_id_a查询不串入run_b的任何事件() {
    let env = Env::new("events-isolation");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let store = state.for_root(&root).expect("for_root 应成功");

    let run_a = store
        .begin_agent_run(&running_run("run A", 100, &root))
        .expect("begin 应成功");
    let run_b = store
        .begin_agent_run(&running_run("run B", 200, &root))
        .expect("begin 应成功");
    assert_ne!(run_a.id, run_b.id, "两 run 经 begin 分配不同 id");
    let events_a = isolation_events("A");
    let events_b = isolation_events("B");
    // 两 run 落完全重叠的 seq 区间（0..=3）：复合键若未按 run_id 隔离必然串扰
    seed_events(&store, run_a.id, &events_a);
    seed_events(&store, run_b.id, &events_b);

    let replay_a =
        agent_run_events(state.clone(), root.clone(), run_a.id).expect("agent_run_events 应成功");

    assert_eq!(
        replay_a, events_a,
        "以 runId=A 经命令面查询恰返回 run A 自己的事件序列（逐字段保真）"
    );
    assert_eq!(
        replay_a.len(),
        events_a.len(),
        "重叠 seq 区间下事件数不翻倍（未串入 run B 的任何事件）"
    );
    assert_events_belong_to(&replay_a, "A");
    assert_no_events_belong_to(&replay_a, "B");

    // 反向半边：以 runId=B 查询同样只含 B 的标记
    let replay_b =
        agent_run_events(state.clone(), root, run_b.id).expect("agent_run_events 应成功");
    assert_eq!(
        replay_b, events_b,
        "以 runId=B 查询恰返回 run B 自己的事件序列"
    );
    assert_events_belong_to(&replay_b, "B");
}

// ---------------------------------------------------------------------------
// agent_run_chain：root 寻址还原本库链，不受他库同定位串干扰（AC-5）
// ---------------------------------------------------------------------------

#[test]
fn agent_run_chain携root还原本库链且不受他库同定位串干扰() {
    let env = Env::new("chain-cross-db");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root_a = env.root_of("ws-a");
    let root_b = env.root_of("ws-b");
    let store_a = state.for_root(&root_a).expect("A for_root 应成功");
    let store_b = state.for_root(&root_b).expect("B for_root 应成功");

    // A 库：explore/"7" 两轮链；B 库：同定位串（source_ref="7"）的单 run
    let first = begin_chain_run(&store_a, "A 首轮", 100, &root_a, "explore", Some("7"), None);
    let second = begin_chain_run(
        &store_a,
        "A 续轮",
        200,
        &root_a,
        "explore",
        Some("7"),
        Some(first.id),
    );
    let decoy = begin_chain_run(
        &store_b,
        "B 同定位首轮",
        300,
        &root_b,
        "explore",
        Some("7"),
        None,
    );

    let chain_a = agent_run_chain(
        state.clone(),
        root_a.clone(),
        "explore".to_owned(),
        "7".to_owned(),
    )
    .expect("A 链还原应成功");
    let chain_b = agent_run_chain(
        state.clone(),
        root_b.clone(),
        "explore".to_owned(),
        "7".to_owned(),
    )
    .expect("B 链还原应成功");

    let ids_a: Vec<i64> = chain_a.iter().map(|record| record.id).collect();
    let ids_b: Vec<i64> = chain_b.iter().map(|record| record.id).collect();
    assert_eq!(
        ids_a,
        vec![first.id, second.id],
        "A 库链按发起序还原（不受他库同定位串干扰）"
    );
    assert_eq!(ids_b, vec![decoy.id], "B 库链只含本库 run");

    // 命令面与 store 直连同序同值 serde 等值（薄包装不加工）
    let via_store = store_a
        .restore_run_chain("explore", "7")
        .expect("直连应成功");
    assert_eq!(
        serde_json::to_value(&chain_a).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄包装不加工：命令面与 store 直连 serde 等值"
    );
}

#[test]
fn agent_run_chain无链与未知source_ref组合均返回空数组不报错() {
    let env = Env::new("chain-cmd-miss");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let empty = agent_run_chain(
        state.clone(),
        root.clone(),
        "explore".to_owned(),
        "404".to_owned(),
    )
    .expect("无链不报错");
    assert!(empty.is_empty(), "无链返回空数组");
    let unknown = agent_run_chain(
        state.clone(),
        root,
        "no-such-source".to_owned(),
        "7".to_owned(),
    )
    .expect("未知 source 不报错");
    assert!(unknown.is_empty(), "未知 source/source_ref 组合空 Vec");
}

// ---------------------------------------------------------------------------
// blank root 纪律矩阵：查询空结果 / stop 幂等 Ok / start 门禁拦截
// ---------------------------------------------------------------------------

#[test]
fn blank_root时三个查询命令返回空结果且stop幂等ok() {
    let env = Env::new("blank-root");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let registry = app.state::<RunStopRegistry>();
    // 空白 root 不进入库解析链路（查询空结果语义）
    assert_eq!(
        agent_runs(state.clone(), String::new()).expect("blank root 空结果"),
        Vec::new()
    );
    assert_eq!(
        agent_run_events(state.clone(), "   ".to_owned(), 1).expect("blank root 空结果"),
        Vec::new()
    );
    assert_eq!(
        agent_run_chain(
            state.clone(),
            String::new(),
            "explore".to_owned(),
            "7".to_owned()
        )
        .expect("blank root 空结果"),
        Vec::new()
    );
    // stop 对 blank root 天然 miss，幂等 Ok
    assert!(agent_stop(registry.clone(), String::new(), 1).is_ok());
    // agent_start 命令体首行守卫：blank root 不进入 runner 与库解析（无 cwd 无从
    // 发起，命令面返回 Err）——守卫谓词在轨道单点，四命令同口径
    assert!(is_blank_root(""));
    assert!(is_blank_root("   "));
    assert!(!is_blank_root("C:\\ws"));
}

// agent_start 命令面携空 root 期待 Err：命令体首参为 Wry AppHandle（MockRuntime
// 不可构造），Err 面经命令委托的编排缝 start_agent_run_with 承载（装置同
// 「for_root 落库」组合用例的 FakeRunner）——runner start 段放行空 cwd 入参后，
// Err 只能死于空白 root 的库解析（for_root("") canonicalize 必然失败），
// 即「无 cwd 无从发起」的命令面 Err 契约。
#[tokio::test]
async fn blank_root的agent_start携空root期待err且零落库零推送() {
    let env = Env::new("blank-start");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let runner = FakeRunner::with_events(Vec::new());
    let (channel, captured) = capturing_channel();

    let result = start_agent_run_with(
        app.handle().clone(),
        &state,
        &runner,
        channel,
        run_params(Path::new("")),
        RunProvenance::debug(),
    );

    let err = result.expect_err("blank root 的 agent_start 必须 Err（无 cwd 无从发起）");
    assert!(!err.is_empty(), "错误串可直抵前端，实际: {err}");
    // Err 归因空白 root 而非 runner 拒绝：空 cwd 入参已放行进入编排链路
    assert_eq!(
        runner.captured_params()[0].cwd,
        PathBuf::new(),
        "空 root 入参已进入 agent_start 编排链路"
    );
    // 启动阶段失败零残留：workspace 库无 run 行、Channel 零推送
    assert!(
        state
            .for_root(&root)
            .expect("for_root 应成功")
            .list_agent_runs()
            .unwrap()
            .is_empty(),
        "blank root 启动失败不落任何 run 行"
    );
    assert!(
        captured.lock().unwrap().is_empty(),
        "blank root 启动失败零推送"
    );
}

// ---------------------------------------------------------------------------
// agent_stop：(root, run id) 复合键寻址，不跨库误停；miss 幂等 Ok
// ---------------------------------------------------------------------------

#[test]
fn agent_stop复合键寻址_同id并行仅停目标库_不跨库误停() {
    let env = Env::new("stop-composite");
    let app = app_with_stores(&env);
    let registry = app.state::<RunStopRegistry>();
    let root_a = env.root_of("ws-a");
    let root_b = env.root_of("ws-b");

    // 注册表登记 (rootA, id=1) 与 (rootB, id=1) 两句柄（同 id 并行）
    let handle_a = RunHandle::default();
    let handle_b = RunHandle::default();
    registry.register(&root_a, 1, handle_a.clone());
    registry.register(&root_b, 1, handle_b.clone());

    assert!(
        agent_stop(registry.clone(), root_b.clone(), 1).is_ok(),
        "命中不报错"
    );
    assert!(
        handle_b.stop_requested(),
        "agent_stop(rootB, 1) 仅 B 命中置位"
    );
    assert!(!handle_a.stop_requested(), "不跨库误停：A 句柄存活未置位");

    // A 句柄存活可再 stop
    assert!(agent_stop(registry.clone(), root_a, 1).is_ok());
    assert!(handle_a.stop_requested(), "A 句柄随后置位");
}

#[test]
fn agent_stop对未注册root与run_id幂等ok() {
    let env = Env::new("stop-idempotent");
    let app = app_with_stores(&env);
    let registry = app.state::<RunStopRegistry>();
    let root = env.root_of("alpha");

    // 未注册 (root, run_id)：registry miss → 无副作用直接 Ok
    assert!(
        agent_stop(registry.clone(), root.clone(), 404).is_ok(),
        "miss 幂等：未注册复合键不报错"
    );

    // 已终态（除名）复合键：登记过但已除名 → 同样幂等 Ok
    registry.register(&root, 7, RunHandle::default());
    registry.remove(&root, 7);
    assert!(
        agent_stop(registry.clone(), root, 7).is_ok(),
        "已终态幂等：除名后不报错"
    );
}

#[test]
fn agent_stop命中running句柄时置位停止信号() {
    let env = Env::new("stop-hit");
    let app = app_with_stores(&env);
    let registry = app.state::<RunStopRegistry>();
    let root = env.root_of("alpha");
    let handle = RunHandle::default();
    registry.register(&root, 9, handle.clone());

    assert!(agent_stop(registry.clone(), root, 9).is_ok(), "命中不报错");
    assert!(
        handle.stop_requested(),
        "agent_stop 经注册表触达句柄置位停止信号"
    );
}

// ---------------------------------------------------------------------------
// agent_start → for_root 落库（组合）：start_agent_run_with 泛型缝（假 runner
// 装置复用 agent_test.rs），running 记录提前 resolve 且落该 root 的 workspace 库
// ---------------------------------------------------------------------------

/// 泛型缝入参（cwd 隐含 workspace root 语义，编排不读盘）。
fn run_params(cwd: &Path) -> AgentRunParams {
    AgentRunParams {
        prompt: "组合链路验证".to_owned(),
        cwd: cwd.to_path_buf(),
        permission_mode: AgentPermissionMode::BypassPermissions,
        resume_session_id: None,
    }
}

fn system_notice(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::SystemNotice {
            subtype: "api_retry".to_owned(),
            payload: serde_json::json!({ "attempt": 2, "note": "重试中" }),
        },
    )
}

fn raw_zh(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Raw {
            event_type: "外星事件".to_owned(),
            raw_json: "{\"kind\":\"外星事件\",\"msg\":\"🚀\"}".to_owned(),
        },
    )
}

#[tokio::test]
async fn agent_start经for_root落库_running提前resolve且落该root的workspace库() {
    let env = Env::new("start-for-root");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let seeded = vec![
        run_started(0),
        message(1),
        system_notice(2),
        run_result(3, false),
        raw_zh(4),
    ];
    let runner = FakeRunner::with_events(seeded.clone());
    let (channel, captured) = capturing_channel();

    let (running, record) = start_and_wait_terminal(
        &app,
        &state,
        &runner,
        &captured,
        channel,
        run_params(Path::new(&root)),
        RunProvenance::debug(),
    )
    .await;

    // 提前 resolve：running 记录 id 立即可用且落该 root 的 workspace 库
    assert_eq!(
        running.status,
        AgentRunStatus::Running,
        "start 即返回 running"
    );
    assert_eq!(
        running.cwd, root,
        "cwd 恒为当前 workspace root（复合键 root 分量）"
    );
    assert_eq!(
        record.status,
        AgentRunStatus::Completed,
        "RunResult 驱动收敛 completed"
    );
    // 落库经 for_root 至当前 workspace 库：agent_runs(root) 可见
    let listed = agent_runs(state.clone(), root.clone()).expect("agent_runs 应成功");
    assert_eq!(listed.len(), 1, "running 记录落该 root 的 workspace 库");
    assert_eq!(listed[0].id, running.id);
    assert_eq!(listed[0].status, AgentRunStatus::Completed, "终态整行替换");
    // 事件随运行追加同库：命令面重放逐字段保真
    let replayed = agent_run_events(state.clone(), root, record.id).expect("重放应成功");
    assert_eq!(
        replayed, seeded,
        "五变体（含 systemNotice 与中文/emoji Raw）经 tee 落库后命令面重放逐字段保真"
    );
    let seqs: Vec<u64> = replayed.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3, 4], "重放 seq 升序");
    // Channel 实时路与落库路一致（tee 双 sink 快照，信封剥壳后对读）
    assert_eq!(
        serde_json::Value::Array(pushed_event_bodies(&captured)),
        serde_json::to_value(&replayed).unwrap(),
        "Channel 推送序列与命令面重放一致"
    );
}

#[tokio::test]
async fn 单run千级seq连续产出后命令面重放序完整不回绕() {
    let env = Env::new("thousand");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let seeded: Vec<AgentEvent> = (0..1000u64).map(raw).collect();
    let runner = FakeRunner::with_events(seeded);
    let (channel, captured) = capturing_channel();

    let (_running, record) = start_and_wait_terminal(
        &app,
        &state,
        &runner,
        &captured,
        channel,
        run_params(Path::new(&root)),
        RunProvenance::debug(),
    )
    .await;

    let replayed =
        agent_run_events(state.clone(), root, record.id).expect("agent_run_events 应成功");
    assert_eq!(replayed.len(), 1000, "千级事件一条不丢");
    // u128 键打包在大 seq 下的保序性（命令面复核）：重放序完整、严格单调
    for (index, event) in replayed.iter().enumerate() {
        assert_eq!(
            event.seq, index as u64,
            "重放第 {index} 条 seq 恰为 {index}"
        );
    }
}

// ---------------------------------------------------------------------------
// agent_start 失败分支：隔离 PATH 走命令委托薄入口（Err(String)，不留行）——
// 命令体直调需 Wry AppHandle（MockRuntime 不可构造）
// ---------------------------------------------------------------------------

#[test]
fn agent_start薄入口在cli不可发现时返回err且workspace库无run行且channel零推送() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let env = Env::new("start-missing");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let empty_path = tempfile::tempdir().expect("创建空 PATH 目录失败");
    let (channel, captured) = capturing_channel();

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", empty_path.path());
    let result = start_agent_run(
        app.handle().clone(),
        &state,
        channel,
        run_params(Path::new(&root)),
        RunProvenance::debug(),
    );
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let err = result.expect_err("CLI 不可发现必须 Err(String)");
    assert!(err.contains("CLI"), "错误串可直抵前端，实际: {err}");
    assert!(
        state
            .for_root(&root)
            .expect("for_root 应成功")
            .list_agent_runs()
            .unwrap()
            .is_empty(),
        "启动失败不留 run 行"
    );
    assert!(
        captured.lock().unwrap().is_empty(),
        "启动失败不推送任何事件"
    );
}

// ---------------------------------------------------------------------------
// agent_start 四可选参数组装语义：经 start_and_wait_terminal（泛型缝）+
// FakeRunner 捕获断言（命令体参数转换段以 assemble 逐行镜像，编排行为经泛型缝
// 全链路观测）
// ---------------------------------------------------------------------------

/// `agent_start` 命令体的参数转换段（逐行镜像）：IPC 入参 → `AgentRunParams`
/// + `RunProvenance`（`source` 缺省 `debug`，显式传入的定位与链参数始终保留）。
fn assemble_agent_start_args(
    root: &str,
    prompt: &str,
    resume_session_id: Option<String>,
    source: Option<String>,
    source_ref: Option<String>,
    parent_run_id: Option<i64>,
) -> (AgentRunParams, RunProvenance) {
    let params = AgentRunParams {
        prompt: prompt.to_owned(),
        cwd: Path::new(root).to_path_buf(),
        permission_mode: AgentPermissionMode::BypassPermissions,
        resume_session_id,
    };
    let mut provenance = RunProvenance::debug();
    if let Some(source) = source {
        provenance.source = source;
    }
    provenance.source_ref = source_ref;
    provenance.parent_run_id = parent_run_id;
    (params, provenance)
}

#[tokio::test]
async fn agent_start全参缺省调试页形态params无resume且记录debug缺省与现状一致() {
    let env = Env::new("debug-default");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let (params, provenance) = assemble_agent_start_args(&root, "调试一轮", None, None, None, None);
    let runner = FakeRunner::with_events(vec![run_started(0), run_result(1, false)]);
    let (channel, captured) = capturing_channel();

    let (_running, record) = start_and_wait_terminal(
        &app, &state, &runner, &captured, channel, params, provenance,
    )
    .await;

    let captured_params = runner.captured_params();
    assert_eq!(captured_params.len(), 1, "恰发起一次运行");
    assert_eq!(
        captured_params[0].resume_session_id, None,
        "调试页 invoke 形态无 --resume 入参"
    );
    assert_eq!(record.source, "debug", "来源缺省 debug（调试链路语义不变）");
    assert_eq!(record.source_ref, None);
    assert_eq!(record.parent_run_id, None);
}

#[tokio::test]
async fn agent_start_explore形态全参resume进params三元组进记录且事件流正常() {
    let env = Env::new("explore-full");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let (params, provenance) = assemble_agent_start_args(
        &root,
        "explore 续轮",
        Some("s-parent".to_owned()),
        Some("explore".to_owned()),
        Some("7".to_owned()),
        Some(42),
    );
    let runner = FakeRunner::with_events(vec![run_started(0), run_result(1, false)]);
    let (channel, captured) = capturing_channel();

    let (_running, record) = start_and_wait_terminal(
        &app, &state, &runner, &captured, channel, params, provenance,
    )
    .await;

    // resume 单独流进 runner 契约（--resume flag 组装细节在 flags_test.rs）
    let runner_params = runner.captured_params();
    assert_eq!(runner_params.len(), 1);
    assert_eq!(
        runner_params[0].resume_session_id.as_deref(),
        Some("s-parent"),
        "resume_session_id 进 AgentRunParams"
    );
    assert_eq!(
        runner_params[0].prompt, "explore 续轮",
        "其余 flag 面参数照常透传"
    );
    // 来源三元组走 RunProvenance 旁路落库，两路互不污染
    assert_eq!(record.source, "explore", "三元组进记录");
    assert_eq!(record.source_ref.as_deref(), Some("7"));
    assert_eq!(record.parent_run_id, Some(42));
    // 事件经 Channel 正常流出
    assert!(
        !captured.lock().expect("捕获锁不可中毒").is_empty(),
        "事件流正常流出（tee Channel sink）"
    );
    assert_eq!(record.status, AgentRunStatus::Completed);
}

#[tokio::test]
async fn agent_start仅传resume时两路各自缺省无串线() {
    let env = Env::new("resume-only");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let (params, provenance) =
        assemble_agent_start_args(&root, "续话轮", Some("s-1".to_owned()), None, None, None);
    let runner = FakeRunner::with_events(vec![run_result(0, false)]);
    let (channel, captured) = capturing_channel();

    let (_running, record) = start_and_wait_terminal(
        &app, &state, &runner, &captured, channel, params, provenance,
    )
    .await;

    assert_eq!(
        runner.captured_params()[0].resume_session_id.as_deref(),
        Some("s-1"),
        "resume 进 params"
    );
    assert_eq!(record.source, "debug", "source 未传仍按缺省 debug 落库");
    assert_eq!(record.source_ref, None, "resume 不串入 source_ref");
    assert_eq!(record.parent_run_id, None, "resume 不串入链指针");
}

#[tokio::test]
async fn agent_start仅传source不传定位与链指针时照入参落库() {
    let env = Env::new("source-only");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    // 设计未规定来源一致性强校验（source 与 source_ref 独立可选）：锁定现状契约
    let (params, provenance) = assemble_agent_start_args(
        &root,
        "explore 首轮",
        None,
        Some("explore".to_owned()),
        None,
        None,
    );
    let runner = FakeRunner::with_events(vec![run_result(0, false)]);
    let (channel, captured) = capturing_channel();

    let (_running, record) = start_and_wait_terminal(
        &app, &state, &runner, &captured, channel, params, provenance,
    )
    .await;

    assert_eq!(record.source, "explore");
    assert_eq!(record.source_ref, None);
    assert_eq!(record.parent_run_id, None);
    assert_eq!(
        runner.captured_params()[0].resume_session_id,
        None,
        "resume 不受 source 影响"
    );
}

#[tokio::test]
async fn parent_run_id指向不存在的run时编排无回查照常落库() {
    let env = Env::new("dangling-parent");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    // 编排不回查链上游存在性（指针语义，指向记录由调用方保证）
    let (params, provenance) = assemble_agent_start_args(
        &root,
        "悬挂指针轮",
        None,
        Some("explore".to_owned()),
        Some("7".to_owned()),
        Some(9999),
    );
    let runner = FakeRunner::with_events(vec![run_result(0, false)]);
    let (channel, captured) = capturing_channel();

    // 无回查：编排不因指针悬挂报错（发起与终态等待均正常返回）
    let (_running, record) = start_and_wait_terminal(
        &app, &state, &runner, &captured, channel, params, provenance,
    )
    .await;

    assert_eq!(record.parent_run_id, Some(9999), "照常落库");
    assert_eq!(record.source, "explore");
}

#[tokio::test]
async fn explore来源run在in_band失败时落failed终态且三字段保留() {
    let env = Env::new("failed-keeps-source");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");

    let (params, provenance) = assemble_agent_start_args(
        &root,
        "失败轮",
        Some("s-tail".to_owned()),
        Some("explore".to_owned()),
        Some("7".to_owned()),
        Some(42),
    );
    let runner = FakeRunner::with_events(vec![run_result(0, true)]);
    let (channel, captured) = capturing_channel();

    let (_running, record) = start_and_wait_terminal(
        &app, &state, &runner, &captured, channel, params, provenance,
    )
    .await;

    assert_eq!(
        record.status,
        AgentRunStatus::Failed,
        "is_error 收敛 failed"
    );
    assert_eq!(record.source, "explore", "失败路径不丢来源");
    assert_eq!(record.source_ref.as_deref(), Some("7"));
    assert_eq!(record.parent_run_id, Some(42));
}

// ---------------------------------------------------------------------------
// WorkspaceStores 生命周期重开：workspace 库数据持久化经命令面复核
// ---------------------------------------------------------------------------

#[test]
fn 释放stores后重开同一数据根命令面重放结果与重开前一致() {
    let env = Env::new("stores-reopen");
    let root;
    let (early_id, late_id, before_runs, before_early, before_late) = {
        let app = app_with_stores(&env);
        let state = app.state::<WorkspaceStores>();
        root = env.root_of("alpha");
        let store = state.for_root(&root).expect("for_root 应成功");
        let early = seed_run(&store, "早启动", 100, &root);
        let late = seed_run(&store, "晚启动", 300, &root);
        seed_events(&store, early.id, &isolation_events("A"));
        seed_events(&store, late.id, &isolation_events("B"));

        let before = (
            early.id,
            late.id,
            agent_runs(state.clone(), root.clone()).expect("agent_runs 应成功"),
            agent_run_events(state.clone(), root.clone(), early.id)
                .expect("agent_run_events 应成功"),
            agent_run_events(state.clone(), root.clone(), late.id)
                .expect("agent_run_events 应成功"),
        );
        // tauri 托管值生命周期长于 App（redb 文件锁不随 App drop 释放）：
        // 测试取回 WorkspaceStores 显式销毁，重开同一数据根才能拿到文件锁
        #[allow(deprecated)]
        let _stores = app.unmanage::<WorkspaceStores>().expect("应处于托管中");
        before
    };

    // drop 后重开同一数据根：重建 MockRuntime app + manage 新开的 WorkspaceStores
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let after_runs = agent_runs(state.clone(), root.clone()).expect("重开后 agent_runs 应成功");
    let after_early = agent_run_events(state.clone(), root.clone(), early_id)
        .expect("重开后 agent_run_events 应成功");
    let after_late =
        agent_run_events(state.clone(), root, late_id).expect("重开后 agent_run_events 应成功");

    assert_eq!(
        after_runs, before_runs,
        "run 清单（含终态与汇总）与重开前命令面一致"
    );
    assert_eq!(
        after_runs
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>(),
        vec![late_id, early_id],
        "重开后 startedAt 降序保持"
    );
    assert_eq!(
        after_early, before_early,
        "早 run 事件重放与重开前命令面一致（重开持久性经命令面复核）"
    );
    assert_eq!(
        after_late, before_late,
        "晚 run 事件重放与重开前命令面一致（重开持久性经命令面复核）"
    );
}
