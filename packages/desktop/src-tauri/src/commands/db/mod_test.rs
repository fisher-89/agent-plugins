//! `commands::db`（db_models / db_records）的单元 + 「store 信封 API → db 命令
//! 轨道」集成关系测试（AC-6：DbDimension scope 寻址两库、两库互不混列、信封
//! 分页语义不变）。
//!
//! `#[tauri::command]` 保留原函数可直调：以 `tauri::test::mock_app()`
//! （MockRuntime，无窗口无事件循环）manage 真实 WorkspaceStores（tempdir 真
//! 开全局库与各 workspace 库）后经 `app.state::<WorkspaceStores>()` 取 State，
//! 沿 workspaces / exec 轨道既有惯例。

use std::path::{Path, PathBuf};

use ::agent::{AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunStatus};
use tauri::{App, Manager};

use super::{db_models, db_records};
use store::{AgentRunRecord, DbDimension, WorkspaceStores};

/// 数据根 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    data_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("db-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("db-cmd-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_dir, ws_root }
    }

    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        std::fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }

    fn root_of(&self, name: &str) -> String {
        self.ws(name).to_string_lossy().into_owned()
    }
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 WorkspaceStores（打开 env
/// 数据根的全局库）。
fn app_with_stores(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app
}

fn add_workspace(stores: &WorkspaceStores, dir: &Path) -> store::WorkspaceRecord {
    stores
        .global()
        .add_workspace(dir)
        .expect("add_workspace 应成功")
}

fn begin_run(store: &store::Store, prompt: &str, started_at: i64) -> AgentRunRecord {
    store
        .begin_agent_run(&AgentRunRecord {
            id: 0,
            prompt: prompt.to_owned(),
            cwd: "C:\\ws\\demo".to_owned(),
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
        })
        .expect("begin_agent_run 应成功")
}

fn append_events(store: &store::Store, run_id: i64, seqs: &[u64]) {
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
// scope=Global：忽略 root，仅 user 维度模型行（AC-6）
// ---------------------------------------------------------------------------

#[test]
fn db_models_scope_global忽略root仅返回workspace一行且blank_root不影响() {
    let env = Env::new("models-global");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    add_workspace(&state, &env.ws("alpha"));
    add_workspace(&state, &env.ws("beta"));

    let via_command = db_models(state.clone(), DbDimension::User, "ignored".to_owned())
        .expect("db_models 应成功");
    let counts: Vec<(String, u64)> = via_command
        .iter()
        .map(|model| (model.name.clone(), model.count))
        .collect();
    assert_eq!(
        counts,
        vec![("workspace".to_owned(), 2)],
        "Global scope 仅返回全局库 workspace 一行（含计数）"
    );

    // Global 忽略 root：blank root 同口径（不触发空结果分支）
    let blank = db_models(state.clone(), DbDimension::User, String::new())
        .expect("Global scope blank root 应成功");
    assert_eq!(
        serde_json::to_value(&blank).unwrap(),
        serde_json::to_value(&via_command).unwrap(),
        "Global scope 忽略 root（含 blank root）"
    );
}

#[test]
fn db_records_scope_global扫描全局库记录信封_key为root串() {
    let env = Env::new("records-global");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let record = add_workspace(&state, &env.ws("alpha"));

    let via_command = db_records(
        state.clone(),
        DbDimension::User,
        String::new(),
        "workspace".to_owned(),
        0,
        10,
    )
    .expect("db_records 应成功");
    let via_store = state
        .global()
        .scan("workspace", 0, 10)
        .expect("直连 scan 应成功");

    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄包装不加工：命令面与直连 store serde 值一致"
    );
    assert_eq!(via_command.len(), 1);
    assert_eq!(
        via_command[0].key,
        serde_json::json!(record.root),
        "全局库 workspace 记录 key 信封为 canonical root 字符串"
    );
    assert!(
        via_command[0].value.is_object(),
        "key / value 均为 JSON 值（无 bincode 二进制形态泄漏）"
    );
}

// ---------------------------------------------------------------------------
// scope=Workspace：for_root 寻址该 root 的 workspace 库（AC-6）
// ---------------------------------------------------------------------------

#[test]
fn db_models_scope_workspace返回三行且与直连serde一致() {
    let env = Env::new("models-workspace");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    let run = begin_run(&ws, "命令面复核", 100);
    append_events(&ws, run.id, &[0, 1]);
    ws.create_explore_record(&root, "topic")
        .expect("建档应成功");

    let via_command =
        db_models(state.clone(), DbDimension::Workspace, root.clone()).expect("db_models 应成功");
    let via_store = ws.list_models().expect("直连 list_models 应成功");

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
            ("agent_run".to_owned(), 1),
            ("agent_event".to_owned(), 2),
            ("explore".to_owned(), 1),
        ],
        "Workspace scope 返回该 root 的 workspace 库三行（计数与实有记录数一致）"
    );
}

#[test]
fn db_records_scope_workspace按主键自然序分页扫描拼接不重不漏() {
    let env = Env::new("records-workspace");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    for index in 0..5 {
        begin_run(&ws, &format!("run-{index}"), 100 + index);
    }

    // offset 0 → limit → 2·limit：透传不加工，翻页拼接覆盖全部记录
    let limit = 2u32;
    let mut union: Vec<i64> = Vec::new();
    for page in 0..3u32 {
        let page_records = db_records(
            state.clone(),
            DbDimension::Workspace,
            root.clone(),
            "agent_run".to_owned(),
            page * limit,
            limit,
        )
        .expect("db_records 应成功");
        union.extend(
            page_records
                .into_iter()
                .map(|envelope| envelope.key.as_i64().expect("agent_run key 为数值")),
        );
    }
    let mut sorted = union.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted, vec![1, 2, 3, 4, 5], "主键自然序翻页拼接不重不漏");

    // 越界 offset 幂等返回空数组而非错误
    let page = db_records(
        state.clone(),
        DbDimension::Workspace,
        root,
        "agent_run".to_owned(),
        u32::MAX,
        10,
    )
    .expect("越界 offset 返回空数组而非错误");
    assert!(page.is_empty(), "越界返回空数组: {page:?}");
}

// ---------------------------------------------------------------------------
// 两库互不混列（AC-6 混列防线）+ blank root 纪律 + 未知模型 / limit 截断
// ---------------------------------------------------------------------------

#[test]
fn 两库清单互不混列_跨维度模型名扫描err() {
    let env = Env::new("no-mixing");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    add_workspace(&state, &env.ws("alpha"));
    let root = env.root_of("beta");
    let ws = state.for_root(&root).expect("for_root 应成功");
    begin_run(&ws, "混列防线", 100);

    fn names_of(models: &[store::ModelInfo]) -> Vec<&str> {
        models.iter().map(|model| model.name.as_str()).collect()
    }

    // Global scope 不出现 agent_run / agent_event / explore 行
    let global_models =
        db_models(state.clone(), DbDimension::User, root.clone()).expect("Global 清单应成功");
    let global_names = names_of(&global_models);
    assert_eq!(
        global_names,
        vec!["workspace"],
        "Global scope 仅 workspace 行"
    );
    // Workspace scope 不出现 workspace 行
    let ws_models = db_models(state.clone(), DbDimension::Workspace, root.clone())
        .expect("Workspace 清单应成功");
    let ws_names = names_of(&ws_models);
    assert_eq!(
        ws_names,
        vec!["agent_run", "agent_event", "explore"],
        "Workspace scope 仅 workspace 维度三行"
    );

    // 跨维度模型名扫描 Err（维度由实例锁定）
    let err_global = db_records(
        state.clone(),
        DbDimension::User,
        root.clone(),
        "agent_run".to_owned(),
        0,
        10,
    )
    .expect_err("Global scope 扫描 agent_run 应 Err");
    assert!(err_global.contains("未知模型"), "错误串可读: {err_global}");
    let err_ws = db_records(
        state.clone(),
        DbDimension::Workspace,
        root.clone(),
        "workspace".to_owned(),
        0,
        10,
    )
    .expect_err("Workspace scope 扫描 workspace 应 Err");
    assert!(err_ws.contains("未知模型"), "错误串可读: {err_ws}");
}

#[test]
fn workspace_scope_blank_root的db_models与db_records同口径返回空结果() {
    let env = Env::new("blank-root");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    // 全局库有注册记录、workspace 库有数据：blank root 仍不触发库解析
    add_workspace(&state, &env.ws("alpha"));
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    begin_run(&ws, "blank root 防线", 100);

    let models = db_models(state.clone(), DbDimension::Workspace, String::new())
        .expect("blank root db_models 空结果");
    assert_eq!(
        serde_json::to_value(&models).unwrap(),
        serde_json::json!([]),
        "Workspace scope blank root 的 db_models 返回空结果"
    );
    let records = db_records(
        state.clone(),
        DbDimension::Workspace,
        "   ".to_owned(),
        "agent_run".to_owned(),
        0,
        10,
    )
    .expect("blank root db_records 空结果");
    assert_eq!(
        serde_json::to_value(&records).unwrap(),
        serde_json::json!([]),
        "Workspace scope blank root 的 db_records 同口径返回空结果"
    );
}

#[test]
fn db_records未知模型名返回err错误串与store层一致不静默吞错() {
    let env = Env::new("records-unknown");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let result = db_records(
        state.clone(),
        DbDimension::User,
        String::new(),
        "nope".to_owned(),
        0,
        10,
    );

    let err = result.expect_err("未知模型名必须 Err(String)");
    let via_store = state
        .global()
        .scan("nope", 0, 10)
        .expect_err("直连 store 同样 Err")
        .to_string();
    assert_eq!(err, via_store, "错误串前缀与 store 层一致（不静默吞错）");
    assert!(err.contains("未知模型"), "错误串可读: {err}");
}

#[test]
fn db_records_limit超500截断且截断后剩余可经offset续读() {
    let env = Env::new("records-limit-cap");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    // 505 条 > 上限 500：截断语义与「恰好 500 条」歧义区分
    for index in 0..505 {
        begin_run(&ws, &format!("run-{index}"), index);
    }

    let first_page = db_records(
        state.clone(),
        DbDimension::Workspace,
        root.clone(),
        "agent_run".to_owned(),
        0,
        5000,
    )
    .expect("db_records 应成功");
    assert_eq!(first_page.len(), 500, "limit 超上限截断为 500");

    let rest = db_records(
        state.clone(),
        DbDimension::Workspace,
        root,
        "agent_run".to_owned(),
        500,
        5000,
    )
    .expect("db_records 应成功");
    assert_eq!(rest.len(), 5, "截断后剩余记录可经 offset 续读");
    let mut union: Vec<i64> = first_page
        .iter()
        .chain(rest.iter())
        .map(|envelope| envelope.key.as_i64().unwrap())
        .collect();
    union.sort_unstable();
    assert_eq!(
        union,
        (1..=505).collect::<Vec<i64>>(),
        "两页拼接覆盖全部 505 条不重不漏"
    );
}
