//! `commands::db`（db_models / db_records）的单元 + 「store 信封 API → db 命令
//! 轨道」集成关系测试（AC-5/AC-7）。
//!
//! `#[tauri::command]` 保留原函数可直调：以 `tauri::test::mock_app()`
//! （MockRuntime，无窗口无事件循环）manage 真实 Store（tempdir 真库）后经
//! `app.state::<Store>()` 取 State，沿 workspaces / exec 轨道既有惯例。

use std::path::{Path, PathBuf};

use agent::{AgentEvent, AgentEventKind};
use tauri::{App, Manager};

use super::{db_models, db_records};
use store::{AgentRunRecord, Store};

/// db 文件 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    db_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("db-cmd-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("db-cmd-test-{tag}-ws-"))
            .tempdir()
            .expect("创建 workspace 临时目录失败");
        Self { db_dir, ws_root }
    }

    fn db_path(&self) -> PathBuf {
        self.db_dir.path().join("test.redb")
    }

    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        std::fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 Store（打开 env 的 db 文件）。
fn app_with_store(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let store = Store::open(&env.db_path()).expect("打开测试 db 失败");
    app.manage(store);
    app
}

fn add_workspace(store: &Store, dir: &Path) -> store::WorkspaceRecord {
    store.add_workspace(dir).expect("add_workspace 应成功")
}

fn begin_run(store: &Store, prompt: &str, started_at: i64) -> AgentRunRecord {
    store
        .begin_agent_run(&AgentRunRecord {
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
        })
        .expect("begin_agent_run 应成功")
}

fn append_events(store: &Store, run_id: i64, seqs: &[u64]) {
    let events: Vec<AgentEvent> = seqs
        .iter()
        .map(|&seq| {
            AgentEvent::stamp(
                seq,
                AgentEventKind::Raw {
                    event_type: "mystery".to_owned(),
                    raw_json: format!(r#"{{"seq":{seq}}}"#),
                },
            )
        })
        .collect();
    store
        .append_agent_run_events(run_id, &events)
        .expect("append 应成功");
}

// ---------------------------------------------------------------------------
// db_models：薄包装不加工 + 空态可呈现
// ---------------------------------------------------------------------------

#[test]
fn db_models写入数据后与直连list_models的serde值一致_薄包装不加工() {
    let env = Env::new("models-passthrough");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    add_workspace(&state, &env.ws("alpha"));
    let run = begin_run(&state, "命令面复核", 100);
    append_events(&state, run.id, &[0, 1]);

    let via_command = db_models(state.clone()).expect("db_models 应成功");
    let via_store = state.list_models().expect("直连 list_models 应成功");

    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄包装不加工：命令面与直连 store serde 值一致"
    );
    let counts: Vec<(String, u64)> = via_command
        .iter()
        .map(|model| (model.name.clone(), model.count))
        .collect();
    assert_eq!(
        counts,
        vec![
            ("workspace".to_owned(), 1),
            ("agent_run".to_owned(), 1),
            ("agent_event".to_owned(), 2),
        ],
        "命令面计数与各模型实有记录数一致"
    );
}

#[test]
fn 空库db_models返回全模型清单且计数为0() {
    let env = Env::new("models-empty");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let models = db_models(state.clone()).expect("空库 db_models 应成功");

    let counts: Vec<u64> = models.iter().map(|model| model.count).collect();
    assert_eq!(
        counts,
        vec![0, 0, 0],
        "全模型照列且计数 0（空态可呈现的命令面前提）"
    );
    assert_eq!(models.len(), 3);
}

// ---------------------------------------------------------------------------
// db_records：薄包装不加工 + 分页透传 + 错误约定 + 越界空数组
// ---------------------------------------------------------------------------

#[test]
fn db_records写入数据后与直连scan的serde值一致且key_value均为json值() {
    let env = Env::new("records-passthrough");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    add_workspace(&state, &env.ws("alpha"));

    let via_command =
        db_records(state.clone(), "workspace".to_owned(), 0, 10).expect("db_records 应成功");
    let via_store = state.scan("workspace", 0, 10).expect("直连 scan 应成功");

    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄包装不加工：命令面与直连 store serde 值一致"
    );
    assert_eq!(via_command.len(), 1);
    assert!(
        via_command[0].key.is_string() && via_command[0].value.is_object(),
        "key / value 均为 JSON 值（无 bincode 二进制形态泄漏）"
    );
}

#[test]
fn db_records连续翻页offset严格步进拼接覆盖全部记录不重不漏() {
    let env = Env::new("records-paging");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    for name in ["alpha", "beta", "gamma", "delta", "epsilon"] {
        add_workspace(&state, &env.ws(name));
    }
    let total = state.scan("workspace", 0, 1).expect("scan 应成功").len();
    assert_eq!(total, 1, "首页恰一页（探针）");

    // offset 0 → limit → 2·limit：透传不加工，翻页拼接覆盖全部记录
    let limit = 2u32;
    let mut union: Vec<serde_json::Value> = Vec::new();
    for page in 0..3u32 {
        let page_records = db_records(state.clone(), "workspace".to_owned(), page * limit, limit)
            .expect("db_records 应成功");
        union.extend(page_records.into_iter().map(|envelope| envelope.key));
    }
    let mut sorted = union.clone();
    sorted.sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
    sorted.dedup();
    assert_eq!(sorted.len(), union.len(), "翻页拼接不重");
    assert_eq!(union.len(), 5, "翻页拼接不漏：覆盖全部 5 条记录");
}

#[test]
fn db_records未知模型名返回err错误串与store层一致不静默吞错() {
    let env = Env::new("records-unknown");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let result = db_records(state.clone(), "nope".to_owned(), 0, 10);

    let err = result.expect_err("未知模型名必须 Err(String)");
    let via_store = state
        .scan("nope", 0, 10)
        .expect_err("直连 store 同样 Err")
        .to_string();
    assert_eq!(err, via_store, "错误串前缀与 store 层一致（不静默吞错）");
    assert!(err.contains("未知模型"), "错误串可读: {err}");
}

#[test]
fn db_records大offset越界返回空数组而非错误() {
    let env = Env::new("records-offset-overflow");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    add_workspace(&state, &env.ws("alpha"));

    let page = db_records(state.clone(), "workspace".to_owned(), u32::MAX, 10)
        .expect("越界 offset 幂等返回空数组而非错误");

    assert!(page.is_empty(), "越界返回空数组: {page:?}");
}
