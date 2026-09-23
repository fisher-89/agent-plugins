//! exec 三命令（agent_start / agent_runs / agent_run_events）的单元 +
//! 「exec 查询命令面 → user 维度两表重放」集成关系测试（AC-3/AC-4）。
//!
//! `#[tauri::command]` 保留原函数可直调：以 `tauri::test::mock_app()`
//! （MockRuntime，无窗口无事件循环）manage 真实 Store（tempdir 真库）后经
//! `app.state::<Store>()` 取 State，沿 workspaces/mod_test.rs 惯例。
//! agent_start 正向（真实 CLI）不直测：以隔离 PATH 触发 CliMissing 走
//! Err 分支（PATH 环境变量修改以共享互斥锁串行化）。

use std::sync::{Arc, Mutex};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{App, Manager};

use ::agent::{AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode};
use store::{AgentRunRecord, Store};

use super::{agent_run_events, agent_runs, agent_start};
use crate::commands::exec::agent::agent_test::PATH_LOCK;

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

/// 落一批事件（AgentEvent 序列 → Value 落库，模拟 tee store sink）。
fn seed_events(store: &Store, run_id: i64, events: &[AgentEvent]) {
    let values: Vec<serde_json::Value> = events
        .iter()
        .map(|event| serde_json::to_value(event).expect("序列化成功"))
        .collect();
    store
        .append_agent_run_events(run_id, &values)
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
            AgentEventKind::Raw { raw_json, .. } => {
                raw_json.contains(&format!(r#""run":"{tag}""#))
            }
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
// agent_run_events：Value → AgentEvent 反序列化 + 不存在 runId 空数组
// ---------------------------------------------------------------------------

#[test]
fn 落库事件经命令面读回为反序列化后的agent_event按seq升序五变体保真() {
    let env = Env::new("events-replay");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let run = state
        .begin_agent_run(&running_run("重放验证", 1727000000000))
        .expect("begin 应成功");
    let seeded = vec![run_started(0), message(1), run_result(2, false), raw(3)];
    seed_events(&state, run.id, &seeded);

    let replayed = agent_run_events(state.clone(), run.id).expect("agent_run_events 应成功");

    assert_eq!(replayed, seeded, "Value → AgentEvent 反序列化后逐字段保真");
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
    assert_eq!(replay_b, events_b, "以 runId=B 查询恰返回 run B 自己的事件序列");
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
    let after_early = agent_run_events(state.clone(), early_id).expect("重开后 agent_run_events 应成功");
    let after_late = agent_run_events(state.clone(), late_id).expect("重开后 agent_run_events 应成功");

    assert_eq!(after_runs, before_runs, "run 清单（含终态与汇总）与重开前命令面一致");
    assert_eq!(
        after_runs.iter().map(|record| record.id).collect::<Vec<_>>(),
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
