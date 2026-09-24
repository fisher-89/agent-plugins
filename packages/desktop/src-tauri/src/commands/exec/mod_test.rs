//! exec 三命令（agent_start / agent_runs / agent_run_events）的单元 +
//! 「exec 查询命令面 → store 类型化事件表重放」与「exec 事件 tee → store
//! 类型化事件表 → 命令面重放」集成关系测试（AC-1/AC-4）。
//!
//! `#[tauri::command]` 保留原函数可直调：以 `tauri::test::mock_app()`
//! （MockRuntime，无窗口无事件循环）manage 真实 Store（tempdir 真库）后经
//! `app.state::<Store>()` 取 State，沿 workspaces/mod_test.rs 惯例。
//! agent_start 正向（真实 CLI）不直测：以隔离 PATH 触发 CliMissing 走
//! Err 分支（PATH 环境变量修改以共享互斥锁串行化）。

use std::path::Path;
use std::sync::{Arc, Mutex};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::{AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunParams};
use store::{AgentRunRecord, Store};

use super::{agent_run_chain, agent_run_events, agent_runs, agent_start};
use crate::commands::exec::agent::agent_test::{FakeRunner, PATH_LOCK};
use crate::commands::exec::agent::{run_agent_with, RunProvenance};

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

/// db 文件临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    db_dir: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("exec-cmd-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        Self { db_dir }
    }

    fn db_path(&self) -> std::path::PathBuf {
        self.db_dir.path().join("test.redb")
    }
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 Store（打开 env 的 db 文件）。
fn app_with_store(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let store = Store::open(&env.db_path()).expect("打开测试 db 失败");
    app.manage(store);
    app
}

/// running 形态底座记录（id 由 begin 分配）。
fn running_run(prompt: &str, started_at: i64) -> AgentRunRecord {
    AgentRunRecord {
        id: 0,
        prompt: prompt.to_owned(),
        cwd: "C:\\ws\\demo".to_owned(),
        env: "default".to_owned(),
        permission_mode: "bypassPermissions".to_owned(),
        status: "running".to_owned(),
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

/// 落一份 completed run 记录并返回（begin → finish）。
fn seed_run(store: &Store, prompt: &str, started_at: i64) -> AgentRunRecord {
    let mut record = store
        .begin_agent_run(&running_run(prompt, started_at))
        .expect("begin 应成功");
    record.status = "completed".to_owned();
    record.finished_at = Some(started_at + 500);
    record.num_turns = Some(2);
    store
        .finish_agent_run(record.id, &record)
        .expect("finish 应成功");
    record
}

/// 落一批事件（AgentEvent 序列类型化落库，模拟 tee store sink）。
fn seed_events(store: &Store, run_id: i64, events: &[AgentEvent]) {
    store
        .append_agent_run_events(run_id, events)
        .expect("append 应成功");
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

/// 捕获型 Channel（agent_start 直调入参）。
fn capturing_channel() -> (Channel<AgentEvent>, Arc<Mutex<Vec<serde_json::Value>>>) {
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

// ---------------------------------------------------------------------------
// agent_runs：薄包装不加工 + 空库空数组
// ---------------------------------------------------------------------------

#[test]
fn agent_runs结果与直连清单的serde值一致且按开始时间降序() {
    let env = Env::new("runs-passthrough");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let late = seed_run(&state, "晚启动", 300);
    let early = seed_run(&state, "早启动", 100);

    let via_command = agent_runs(state.clone()).expect("agent_runs 应成功");
    let via_store = state.list_agent_runs().expect("直连 list 应成功");
    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄包装不加工：命令面与直连 store serde 值一致"
    );
    let ids: Vec<i64> = via_command.iter().map(|record| record.id).collect();
    assert_eq!(ids, vec![late.id, early.id], "startedAt 降序");
}

#[test]
fn 空库agent_runs的serde值为空数组() {
    let env = Env::new("runs-empty");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let list = agent_runs(state.clone()).expect("agent_runs 应成功");
    assert_eq!(serde_json::to_value(&list).unwrap(), serde_json::json!([]));
}

// ---------------------------------------------------------------------------
// agent_run_events：类型化事件重放 + 不存在 runId 空数组
// ---------------------------------------------------------------------------

#[test]
fn 落库事件经命令面读回为类型化agent_event按seq升序五变体保真() {
    let env = Env::new("events-replay");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let run = state
        .begin_agent_run(&running_run("重放验证", 1727000000000))
        .expect("begin 应成功");
    let seeded = vec![run_started(0), message(1), run_result(2, false), raw(3)];
    seed_events(&state, run.id, &seeded);

    let replayed = agent_run_events(state.clone(), run.id).expect("agent_run_events 应成功");

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
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let replayed = agent_run_events(state.clone(), 404).expect("miss 幂等不报错");
    assert!(replayed.is_empty());
}

#[test]
fn 两run事件隔离经命令面复核以run_id_a查询不串入run_b的任何事件() {
    let env = Env::new("events-isolation");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let run_a = state
        .begin_agent_run(&running_run("run A", 100))
        .expect("begin 应成功");
    let run_b = state
        .begin_agent_run(&running_run("run B", 200))
        .expect("begin 应成功");
    assert_ne!(run_a.id, run_b.id, "两 run 经 begin 分配不同 id");
    let events_a = isolation_events("A");
    let events_b = isolation_events("B");
    // 两 run 落完全重叠的 seq 区间（0..=3）：复合键若未按 run_id 隔离必然串扰
    seed_events(&state, run_a.id, &events_a);
    seed_events(&state, run_b.id, &events_b);

    let replay_a = agent_run_events(state.clone(), run_a.id).expect("agent_run_events 应成功");

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
    let replay_b = agent_run_events(state.clone(), run_b.id).expect("agent_run_events 应成功");
    assert_eq!(
        replay_b, events_b,
        "以 runId=B 查询恰返回 run B 自己的事件序列"
    );
    assert_events_belong_to(&replay_b, "B");
}

#[test]
fn drop后重开同一db命令面重放结果与重开前一致() {
    let env = Env::new("cmd-reopen");

    // 重开前快照全部经命令面取得（agent_runs / agent_run_events），不直连 store
    let (early_id, late_id, before_runs, before_early, before_late) = {
        let app = app_with_store(&env);
        let before = {
            let state = app.state::<Store>();
            let early = seed_run(&state, "早启动", 100);
            let late = seed_run(&state, "晚启动", 300);
            seed_events(&state, early.id, &isolation_events("A"));
            seed_events(&state, late.id, &isolation_events("B"));

            (
                early.id,
                late.id,
                agent_runs(state.clone()).expect("agent_runs 应成功"),
                agent_run_events(state.clone(), early.id).expect("agent_run_events 应成功"),
                agent_run_events(state.clone(), late.id).expect("agent_run_events 应成功"),
            )
        }; // State 借用先行结束：unmanage 前不得残留引用

        // tauri 托管值生命周期长于 App（redb 文件锁不随 App drop 释放）：
        // 测试取回 Store 显式销毁，重开同一 db 文件才能拿到文件锁
        #[allow(deprecated)]
        let store = app.unmanage::<Store>().expect("Store 应处于托管中");
        drop(store);
        drop(app);
        before
    };

    // drop 后重开同一 db 文件：重建 MockRuntime app + manage 新开的真实 Store
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let after_runs = agent_runs(state.clone()).expect("重开后 agent_runs 应成功");
    let after_early =
        agent_run_events(state.clone(), early_id).expect("重开后 agent_run_events 应成功");
    let after_late =
        agent_run_events(state.clone(), late_id).expect("重开后 agent_run_events 应成功");

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
        "早 run 事件重放与重开前命令面一致（AC-3 重开持久性经命令面复核）"
    );
    assert_eq!(
        after_late, before_late,
        "晚 run 事件重放与重开前命令面一致（AC-3 重开持久性经命令面复核）"
    );
}

// ---------------------------------------------------------------------------
// 集成关系 R3：exec 事件 tee → store 类型化事件表 → 命令面重放（假 runner 经
// run_agent_with 泛型缝注入，沿 agent_test.rs 既有装置）
// ---------------------------------------------------------------------------

/// 泛型缝入参（cwd 隐含 workspace root 语义，编排不读盘）。
fn run_params() -> AgentRunParams {
    AgentRunParams {
        prompt: "R3 链路验证".to_owned(),
        cwd: Path::new("C:\\ws\\demo").to_path_buf(),
        env: AgentEnvMode::Default,
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
async fn 假runner五变体事件流经tee落库后命令面重放逐字段保真seq升序() {
    let env = Env::new("r3-five-variants");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let seeded = vec![
        run_started(0),
        message(1),
        system_notice(2),
        run_result(3, false),
        raw_zh(4),
    ];
    let runner = FakeRunner::with_events(seeded.clone());
    let (channel, captured) = capturing_channel();

    let record = run_agent_with(
        &state,
        &runner,
        channel,
        run_params(),
        RunProvenance::debug(),
    )
    .await
    .expect("completed 会话返回 Ok");

    assert_eq!(record.status, "completed", "RunResult 驱动收敛 completed");
    let replayed = agent_run_events(state.clone(), record.id).expect("agent_run_events 应成功");
    assert_eq!(
        replayed, seeded,
        "五变体（含 systemNotice 与中文/emoji Raw）经 tee 落库后命令面重放逐字段保真"
    );
    let seqs: Vec<u64> = replayed.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3, 4], "重放 seq 升序");
    // Channel 实时路与落库路一致（tee 双 sink 快照）
    let pushed = captured.lock().expect("捕获锁不可中毒").clone();
    assert_eq!(
        serde_json::Value::Array(pushed),
        serde_json::to_value(&replayed).unwrap(),
        "Channel 推送序列与命令面重放一致"
    );
}

#[tokio::test]
async fn 单run千级seq连续产出后命令面重放序完整不回绕() {
    let env = Env::new("r3-thousand");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let seeded: Vec<AgentEvent> = (0..1000u64).map(raw).collect();
    let runner = FakeRunner::with_events(seeded);

    let record = run_agent_with(
        &state,
        &runner,
        capturing_channel().0,
        run_params(),
        RunProvenance::debug(),
    )
    .await
    .expect("completed 会话返回 Ok");

    let replayed = agent_run_events(state.clone(), record.id).expect("agent_run_events 应成功");
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
// agent_start：隔离 PATH 走启动失败分支（AC-4 Err(String) + D5 不留行）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agent_start在cli不可发现时返回err且store无run行且channel零推送() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let env = Env::new("start-missing");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let empty_path = tempfile::tempdir().expect("创建空 PATH 目录失败");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", empty_path.path());
    let result = agent_start(
        state.clone(),
        capturing_channel().0,
        "C:\\ws\\demo".to_owned(),
        "你好".to_owned(),
        AgentEnvMode::Default,
        AgentPermissionMode::BypassPermissions,
        None,
        None,
        None,
        None,
    )
    .await;
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let err = result.expect_err("CLI 不可发现必须 Err(String)");
    assert!(err.contains("CLI"), "错误串可直抵前端，实际: {err}");
    assert!(
        state.list_agent_runs().unwrap().is_empty(),
        "D5：启动失败不留 run 行"
    );
}

// ---------------------------------------------------------------------------
// agent_start 四可选参数：组装语义经 run_agent_with 泛型缝 + FakeRunner 捕获
// 断言（R2：agent_start 参数 → RunProvenance 编排 → AgentRunRecord v2 落库）。
// 命令体直调无法替换真实 CLI runner，组装段以 assemble_agent_start_args 逐行
// 镜像，编排行为经泛型缝全链路观测。
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
        env: AgentEnvMode::Default,
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
    let env = Env::new("r2-debug-default");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let (params, provenance) =
        assemble_agent_start_args("C:\\ws\\demo", "调试一轮", None, None, None, None);
    let runner = FakeRunner::with_events(vec![run_started(0), run_result(1, false)]);
    let (channel, _captured) = capturing_channel();

    let record = run_agent_with(&state, &runner, channel, params, provenance)
        .await
        .expect("completed 会话返回 Ok");

    let captured = runner.captured_params();
    assert_eq!(captured.len(), 1, "恰发起一次运行");
    assert_eq!(
        captured[0].resume_session_id, None,
        "AC-6 回归：调试页 invoke 形态无 --resume 入参"
    );
    assert_eq!(
        record.source, "debug",
        "AC-4：来源缺省 debug（调试链路语义不变）"
    );
    assert_eq!(record.source_ref, None);
    assert_eq!(record.parent_run_id, None);
}

#[tokio::test]
async fn agent_start_explore形态全参resume进params三元组进记录且事件流正常() {
    let env = Env::new("r2-explore-full");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let (params, provenance) = assemble_agent_start_args(
        "C:\\ws\\demo",
        "explore 续轮",
        Some("s-parent".to_owned()),
        Some("explore".to_owned()),
        Some("7".to_owned()),
        Some(42),
    );
    let runner = FakeRunner::with_events(vec![run_started(0), run_result(1, false)]);
    let (channel, captured) = capturing_channel();

    let record = run_agent_with(&state, &runner, channel, params, provenance)
        .await
        .expect("completed 会话返回 Ok");

    // resume 单独流进 runner 契约（--resume flag 组装细节在 flags_test.rs）
    let runner_params = runner.captured_params();
    assert_eq!(runner_params.len(), 1);
    assert_eq!(
        runner_params[0].resume_session_id.as_deref(),
        Some("s-parent"),
        "resume_session_id 进 AgentRunParams（AC-6）"
    );
    assert_eq!(
        runner_params[0].prompt, "explore 续轮",
        "其余 flag 面参数照常透传"
    );
    // 来源三元组走 RunProvenance 旁路落库，两路互不污染
    assert_eq!(record.source, "explore", "AC-4：三元组进记录");
    assert_eq!(record.source_ref.as_deref(), Some("7"));
    assert_eq!(record.parent_run_id, Some(42));
    // 事件经 Channel 正常流出
    assert!(
        !captured.lock().expect("捕获锁不可中毒").is_empty(),
        "事件流正常流出（tee Channel sink）"
    );
    assert_eq!(record.status, "completed");
}

#[tokio::test]
async fn agent_start仅传resume时两路各自缺省无串线() {
    let env = Env::new("r2-resume-only");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let (params, provenance) = assemble_agent_start_args(
        "C:\\ws\\demo",
        "续话轮",
        Some("s-1".to_owned()),
        None,
        None,
        None,
    );
    let runner = FakeRunner::with_events(vec![run_result(0, false)]);

    let record = run_agent_with(&state, &runner, capturing_channel().0, params, provenance)
        .await
        .expect("completed 会话返回 Ok");

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
    let env = Env::new("r2-source-only");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    // 设计未规定来源一致性强校验（source 与 source_ref 独立可选）：锁定现状契约
    let (params, provenance) = assemble_agent_start_args(
        "C:\\ws\\demo",
        "explore 首轮",
        None,
        Some("explore".to_owned()),
        None,
        None,
    );
    let runner = FakeRunner::with_events(vec![run_result(0, false)]);

    let record = run_agent_with(&state, &runner, capturing_channel().0, params, provenance)
        .await
        .expect("completed 会话返回 Ok");

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
    let env = Env::new("r2-dangling-parent");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    // D4：编排不回查链上游存在性（指针语义，指向记录由调用方保证）
    let (params, provenance) = assemble_agent_start_args(
        "C:\\ws\\demo",
        "悬挂指针轮",
        None,
        Some("explore".to_owned()),
        Some("7".to_owned()),
        Some(9999),
    );
    let runner = FakeRunner::with_events(vec![run_result(0, false)]);

    let record = run_agent_with(&state, &runner, capturing_channel().0, params, provenance)
        .await
        .expect("无回查：不因指针悬挂报错");

    assert_eq!(record.parent_run_id, Some(9999), "照常落库");
    assert_eq!(record.source, "explore");
}

#[tokio::test]
async fn explore来源run在in_band失败时落failed终态且三字段保留() {
    let env = Env::new("r2-failed-keeps-source");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let (params, provenance) = assemble_agent_start_args(
        "C:\\ws\\demo",
        "失败轮",
        Some("s-tail".to_owned()),
        Some("explore".to_owned()),
        Some("7".to_owned()),
        Some(42),
    );
    let runner = FakeRunner::with_events(vec![run_result(0, true)]);

    let record = run_agent_with(&state, &runner, capturing_channel().0, params, provenance)
        .await
        .expect("in-band 失败返回 Ok(failed 记录)");

    assert_eq!(record.status, "failed", "is_error 收敛 failed");
    assert_eq!(record.source, "explore", "失败路径不丢来源");
    assert_eq!(record.source_ref.as_deref(), Some("7"));
    assert_eq!(record.parent_run_id, Some(42));
}

// ---------------------------------------------------------------------------
// agent_run_chain：restore_run_chain 命令薄包装（通用面）+ 链还原集成关系
// ---------------------------------------------------------------------------

/// 落一条带来源三元组的 run 记录并返回（begin 分配 id）。
fn begin_chain_run(
    store: &Store,
    prompt: &str,
    started_at: i64,
    source: &str,
    source_ref: Option<&str>,
    parent_run_id: Option<i64>,
) -> AgentRunRecord {
    let mut requested = running_run(prompt, started_at);
    requested.source = source.to_owned();
    requested.source_ref = source_ref.map(str::to_owned);
    requested.parent_run_id = parent_run_id;
    store.begin_agent_run(&requested).expect("begin 应成功")
}

#[test]
fn agent_run_chain与store_restore_run_chain直连同序同值serde等值() {
    let env = Env::new("chain-cmd-passthrough");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let first = begin_chain_run(&state, "首轮", 100, "explore", Some("7"), None);
    let second = begin_chain_run(&state, "续轮", 200, "explore", Some("7"), Some(first.id));

    let via_command =
        agent_run_chain(state.clone(), "explore".to_owned(), "7".to_owned()).expect("查询应成功");
    let via_store = state.restore_run_chain("explore", "7").expect("直连应成功");

    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄包装不加工：命令面与 store 直连 serde 等值"
    );
    let ids: Vec<i64> = via_command.iter().map(|record| record.id).collect();
    assert_eq!(ids, vec![first.id, second.id], "发起序（AC-5）");
}

#[test]
fn agent_run_chain无链与未知source_ref组合均返回空数组不报错() {
    let env = Env::new("chain-cmd-miss");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let empty =
        agent_run_chain(state.clone(), "explore".to_owned(), "404".to_owned()).expect("无链不报错");
    assert!(empty.is_empty(), "AC-5：无链返回空数组");
    let unknown = agent_run_chain(state.clone(), "no-such-source".to_owned(), "7".to_owned())
        .expect("未知 source 不报错");
    assert!(
        unknown.is_empty(),
        "AC-5：未知 source/source_ref 组合空 Vec"
    );
}

#[test]
fn 两条explore链交替落库后各自完整还原且debug与对方链不入列() {
    let env = Env::new("r3-two-chains");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    // R7 与 R8 交替落库 + 一条 debug 干扰 run
    let r7_1 = begin_chain_run(&state, "R7 首轮", 100, "explore", Some("7"), None);
    let r8_1 = begin_chain_run(&state, "R8 首轮", 150, "explore", Some("8"), None);
    let r7_2 = begin_chain_run(&state, "R7 续轮", 200, "explore", Some("7"), Some(r7_1.id));
    let r8_2 = begin_chain_run(&state, "R8 续轮", 250, "explore", Some("8"), Some(r8_1.id));
    begin_chain_run(&state, "调试 run", 300, "debug", None, None);

    let chain7 =
        agent_run_chain(state.clone(), "explore".to_owned(), "7".to_owned()).expect("R7 应还原");
    let chain8 =
        agent_run_chain(state.clone(), "explore".to_owned(), "8".to_owned()).expect("R8 应还原");

    let ids7: Vec<i64> = chain7.iter().map(|record| record.id).collect();
    let ids8: Vec<i64> = chain8.iter().map(|record| record.id).collect();
    assert_eq!(ids7, vec![r7_1.id, r7_2.id], "R7 链按发起序恰 2 条（AC-5）");
    assert_eq!(
        ids8,
        vec![r8_1.id, r8_2.id],
        "R8 链按发起序恰 2 条，交替落库互不串扰"
    );
    assert!(
        chain7
            .iter()
            .chain(&chain8)
            .all(|record| record.source == "explore"),
        "debug 与对方链不入列"
    );
}

#[test]
fn 链还原后逐run事件重放拼合按发起序无缝拼接() {
    let env = Env::new("r3-chain-replay");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let first = begin_chain_run(&state, "首轮", 100, "explore", Some("7"), None);
    let second = begin_chain_run(&state, "续轮", 200, "explore", Some("7"), Some(first.id));
    // 每条 run 落一组可区分事件（归属标记 tag）
    let events_first = isolation_events("71");
    let events_second = isolation_events("72");
    state
        .append_agent_run_events(first.id, &events_first)
        .expect("append 应成功");
    state
        .append_agent_run_events(second.id, &events_second)
        .expect("append 应成功");

    // 前端重放的同一收口：链还原 → 逐 run list_agent_run_events → 拼合
    let chain =
        agent_run_chain(state.clone(), "explore".to_owned(), "7".to_owned()).expect("链还原应成功");
    let mut combined = Vec::new();
    for run in &chain {
        combined.extend(agent_run_events(state.clone(), run.id).expect("事件重放应成功"));
    }

    assert_eq!(
        combined.len(),
        events_first.len() + events_second.len(),
        "全链事件无重叠无遗漏"
    );
    assert_events_belong_to(&combined[..events_first.len()], "71");
    assert_events_belong_to(&combined[events_first.len()..], "72");
    let seqs: Vec<u64> = combined.iter().map(|event| event.seq).collect();
    assert_eq!(
        seqs,
        vec![0, 1, 2, 3, 0, 1, 2, 3],
        "按发起序逐 run 拼接（run 内 seq 升序、run 间发起序）"
    );
}
