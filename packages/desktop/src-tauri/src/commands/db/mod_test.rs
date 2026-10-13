use std::path::{Path, PathBuf};

use ::agent::{AgentEvent, AgentEventKind};
use tauri::{App, Manager};

use super::{db_models, db_records};
use store::{AgentEngineKind, DbDimension, SessionConfigSnapshot, SessionRecord, WorkspaceStores};

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

fn seed_session(store: &store::Store, id: &str) -> String {
    store
        .create_session(&SessionRecord {
            id: id.to_owned(),
            engine_session_id: None,
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                model: Some("m-high".to_owned()),
                permission_mode: ::agent::AgentPermissionMode::BypassPermissions,
            },
            source: "debug".to_owned(),
            source_ref: None,
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("create_session 应成功");
    id.to_owned()
}

fn append_events(store: &store::Store, session_id: &str, seqs: &[u64]) {
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
        .append_session_events(session_id, &events)
        .expect("append 应成功");
}

// ---------------------------------------------------------------------------
// scope=Global：忽略 root，仅 user 维度模型行（AC-6）
// ---------------------------------------------------------------------------

#[test]
fn db_models_scope_global忽略root仅返回user维度三行且blank_root不影响() {
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
        vec![
            ("workspace".to_owned(), 2),
            ("agent_provider".to_owned(), 0),
            ("agent_instance".to_owned(), 0)
        ],
        "Global scope 仅返回全局库 user 维度三行（agent 管理两模型计数 0 也列出）"
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
    assert_eq!(via_command.total, 1, "total = 全库记录计数");
    let records = via_command.records;
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].key,
        serde_json::json!(record.root),
        "全局库 workspace 记录 key 信封为 canonical root 字符串"
    );
    assert!(
        records[0].value.is_object(),
        "key / value 均为 JSON 值（无 bincode 二进制形态泄漏）"
    );
}

// ---------------------------------------------------------------------------
// scope=Workspace：for_root 寻址该 root 的 workspace 库（AC-6）
// ---------------------------------------------------------------------------

#[test]
fn db_models_scope_workspace返回八行且与直连serde一致() {
    let env = Env::new("models-workspace");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    let session_id = seed_session(&ws, "ses-cmd-review");
    append_events(&ws, &session_id, &[0, 1]);
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
            ("agent_run".to_owned(), 0),
            ("session".to_owned(), 1),
            ("session_event".to_owned(), 2),
            ("explore".to_owned(), 1),
            ("change".to_owned(), 0),
            ("phase".to_owned(), 0),
            ("checklist_item".to_owned(), 0),
            ("step".to_owned(), 0),
        ],
        "Workspace scope 返回该 root 的 workspace 库八行（change 流程状态四模型计数 0 也列出，计数与实有记录数一致）"
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
        seed_session(&ws, &format!("ses-page-{index}"));
    }

    // offset 0 → limit → 2·limit：透传不加工，翻页拼接覆盖全部记录
    let limit = 2u32;
    let mut union: Vec<String> = Vec::new();
    for page in 0..3u32 {
        let page_records = db_records(
            state.clone(),
            DbDimension::Workspace,
            root.clone(),
            "session".to_owned(),
            page * limit,
            limit,
        )
        .expect("db_records 应成功");
        assert_eq!(
            page_records.total, 5,
            "total = 该模型全量计数（非本页条数），逐页一致"
        );
        union.extend(page_records.records.into_iter().map(|envelope| {
            envelope
                .key
                .as_str()
                .expect("session key 为会话 id 字符串")
                .to_owned()
        }));
    }
    let mut sorted = union.clone();
    sorted.sort_unstable();
    sorted.dedup();
    let expected: Vec<String> = (0..5).map(|index| format!("ses-page-{index}")).collect();
    assert_eq!(sorted, expected, "主键自然序翻页拼接不重不漏");

    // 越界 offset 幂等返回空信封而非错误（records 空、total 恒全量）
    let page = db_records(
        state.clone(),
        DbDimension::Workspace,
        root,
        "session".to_owned(),
        u32::MAX,
        10,
    )
    .expect("越界 offset 返回空信封而非错误");
    assert!(
        page.records.is_empty(),
        "越界返回空 records: {:?}",
        page.records
    );
    assert_eq!(page.total, 5, "越界空页 total 恒全量计数");
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
    seed_session(&ws, "ses-no-mixing");

    fn names_of(models: &[store::ModelInfo]) -> Vec<&str> {
        models.iter().map(|model| model.name.as_str()).collect()
    }

    // Global scope 不出现会话域与 explore 行（user 维度三行）
    let global_models =
        db_models(state.clone(), DbDimension::User, root.clone()).expect("Global 清单应成功");
    let global_names = names_of(&global_models);
    assert_eq!(
        global_names,
        vec!["workspace", "agent_provider", "agent_instance"],
        "Global scope 恰 user 维度三行"
    );
    // Workspace scope 不出现 workspace 行
    let ws_models = db_models(state.clone(), DbDimension::Workspace, root.clone())
        .expect("Workspace 清单应成功");
    let ws_names = names_of(&ws_models);
    assert_eq!(
        ws_names,
        vec![
            "agent_run",
            "session",
            "session_event",
            "explore",
            "change",
            "phase",
            "checklist_item",
            "step"
        ],
        "Workspace scope 仅 workspace 维度八行（change 流程状态四模型在列）"
    );

    // 跨维度模型名扫描 Err（维度由实例锁定；workspace 库实例扫 user 维度
    // 新模型名同口径）
    let err_global = db_records(
        state.clone(),
        DbDimension::User,
        root.clone(),
        "session".to_owned(),
        0,
        10,
    )
    .expect_err("Global scope 扫描 session 应 Err");
    assert!(err_global.contains("未知模型"), "错误串可读: {err_global}");
    let err_new_model = db_records(
        state.clone(),
        DbDimension::Workspace,
        root.clone(),
        "agent_provider".to_owned(),
        0,
        10,
    )
    .expect_err("Workspace scope 扫描 agent_provider 应 Err");
    assert!(
        err_new_model.contains("未知模型"),
        "错误串可读: {err_new_model}"
    );
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
    seed_session(&ws, "ses-blank-guard");

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
        "session".to_owned(),
        0,
        10,
    )
    .expect("blank root db_records 空信封");
    assert_eq!(
        serde_json::to_value(&records).unwrap(),
        serde_json::json!({ "records": [], "total": 0 }),
        "Workspace scope blank root 的 db_records 返回空信封（DD2）"
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
        seed_session(&ws, &format!("ses-cap-{index}"));
    }

    let first_page = db_records(
        state.clone(),
        DbDimension::Workspace,
        root.clone(),
        "session".to_owned(),
        0,
        5000,
    )
    .expect("db_records 应成功");
    assert_eq!(first_page.records.len(), 500, "limit 超上限截断为 500");
    assert_eq!(
        first_page.total, 505,
        "截断只作用 records 区间，total 仍全量计数"
    );

    let rest = db_records(
        state.clone(),
        DbDimension::Workspace,
        root,
        "session".to_owned(),
        500,
        5000,
    )
    .expect("db_records 应成功");
    assert_eq!(rest.records.len(), 5, "截断后剩余记录可经 offset 续读");
    assert_eq!(rest.total, 505, "续页 total 恒全量计数");
    let mut union: Vec<String> = first_page
        .records
        .iter()
        .chain(rest.records.iter())
        .map(|envelope| {
            envelope
                .key
                .as_str()
                .expect("session key 为会话 id 字符串")
                .to_owned()
        })
        .collect();
    union.sort_unstable();
    // 主键自然序为字符串字典序（session 主键即 core 铸会话 id 字符串）
    let mut expected: Vec<String> = (0..505).map(|index| format!("ses-cap-{index}")).collect();
    expected.sort_unstable();
    assert_eq!(
        union, expected,
        "两页拼接覆盖全部 505 条不重不漏（同序逐字一致）"
    );
}

/// 错误透传：Workspace 维度 root 不可寻址（`for_root` canonicalize 失败）→
/// db_models / db_records 显式 `Err` 记因（不 panic、不产半截结果——寻址
/// 失败先于模型名校验）。
#[test]
fn workspace维度root不可寻址_两命令err透传不panic() {
    let env = Env::new("for-root-err");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let missing = env
        .data_dir
        .path()
        .join("absent-ws")
        .to_string_lossy()
        .into_owned();

    for err in [
        db_models(state.clone(), DbDimension::Workspace, missing.clone())
            .expect_err("db_models 应 Err"),
        db_records(
            state.clone(),
            DbDimension::Workspace,
            missing,
            "ChangeRecord".to_owned(),
            0,
            10,
        )
        .expect_err("db_records 应 Err"),
    ] {
        assert!(
            err.contains("canonicalize") && err.contains("absent-ws"),
            "Err 记因 canonicalize 失败与 root 线索，实际: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-3 分页信封薄透传（链路：命令面 → Store::scan → envelope::scan）
// ---------------------------------------------------------------------------

/// 组合链锚：workspace 库种 30 条 → `db_records` 返回 `{ records: 区间, total: 30 }`
/// 且与直连 `store.scan` 结果 serde 一致（薄透传不加工；records 区间 + total
/// 全量同快照经命令面贯通）。
#[test]
fn db_records分页信封薄透传_与直连store_scan结果serde一致() {
    let env = Env::new("records-page-envelope");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    for index in 0..30 {
        seed_session(&ws, &format!("ses-chain-{index}"));
    }

    let via_command = db_records(
        state.clone(),
        DbDimension::Workspace,
        root.clone(),
        "session".to_owned(),
        10,
        10,
    )
    .expect("db_records 应成功");
    let via_store = ws.scan("session", 10, 10).expect("直连 store.scan 应成功");

    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap(),
        "薄透传不加工：命令面与直连 store.scan serde 值一致"
    );
    assert_eq!(via_command.records.len(), 10, "records = offset/limit 区间");
    assert_eq!(
        via_command.total, 30,
        "total = 该模型全量计数（非本页条数），与 records 同一读事务快照"
    );
}

/// offset / limit int 边界过命令面不加工：offset 0 / 恰等总数 / 超总数 /
/// `u32::MAX`、limit 0 / 超 `MAX_SCAN_LIMIT` 截断透传；records 空页时 total
/// 恒全量。
#[test]
fn db_records分页int边界过命令面不加工_空页total恒全量() {
    let env = Env::new("records-page-int-edge");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    for index in 0..3 {
        seed_session(&ws, &format!("ses-edge-{index}"));
    }

    let scan = |offset: u32, limit: u32| -> store::RecordsPage {
        db_records(
            state.clone(),
            DbDimension::Workspace,
            root.clone(),
            "session".to_owned(),
            offset,
            limit,
        )
        .expect("db_records 应成功")
    };

    assert_eq!(scan(0, 2).records.len(), 2, "limit 小于总数返回恰 limit 条");
    assert_eq!(scan(0, 0).records.len(), 0, "limit=0 返回空 records");
    assert_eq!(
        scan(1, 100).records.len(),
        2,
        "limit 大于剩余记录数返回剩余全部"
    );
    assert_eq!(
        scan(3, 10).records.len(),
        0,
        "offset 恰等总数返回空 records"
    );
    assert_eq!(
        scan(3, 10).total,
        3,
        "offset 恰等总数：records 空而 total 恒全量"
    );
    assert_eq!(
        scan(u32::MAX, 10).records.len(),
        0,
        "offset 超总数（u32::MAX）返回空 records"
    );
    assert_eq!(
        scan(u32::MAX, 10).total,
        3,
        "offset 超总数：records 空而 total 恒全量"
    );

    // limit 超 MAX_SCAN_LIMIT 截断透传（505 条 > 上限 500）
    let big = env.root_of("big");
    let big_ws = state.for_root(&big).expect("for_root 应成功");
    for index in 0..505 {
        seed_session(&big_ws, &format!("ses-big-{index}"));
    }
    let cap = db_records(
        state.clone(),
        DbDimension::Workspace,
        big,
        "session".to_owned(),
        0,
        5000,
    )
    .expect("db_records 应成功");
    assert_eq!(cap.records.len(), 500, "limit 超上限截断为 500 透传不加工");
    assert_eq!(cap.total, 505, "截断后 total 仍全量计数");
}

// ---------------------------------------------------------------------------
// AC-3 scope 寻址与 blank root 空信封（DD2）
// ---------------------------------------------------------------------------

/// root 参数 str 边界：空串与纯空白（`is_blank_root` 口径）→ 空信封
/// `{ records: [], total: 0 }` 且不触发库解析；超长 root / 含特殊字符的非法
/// 路径 root → 按不存在库处理不 panic（不误判 blank）。
#[test]
fn db_records_blank_root返回空信封_非法路径root按不存在库reject不误判blank() {
    let env = Env::new("records-root-edge");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    // 全局库有注册记录、workspace 库有数据：blank root 仍不触发库解析
    add_workspace(&state, &env.ws("alpha"));
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    seed_session(&ws, "ses-blank-edge");

    let empty_envelope = serde_json::json!({ "records": [], "total": 0 });

    // 空串 / 纯空白 → 空信封（DD2 主锚），且模型名校验不触发（未知模型名仍空信封）
    for blank in [String::new(), "   \t\n".to_owned()] {
        let page = db_records(
            state.clone(),
            DbDimension::Workspace,
            blank,
            "nope".to_owned(),
            0,
            10,
        )
        .expect("blank root 返回空信封而非 Err");
        assert_eq!(
            serde_json::to_value(&page).unwrap(),
            empty_envelope,
            "blank root 返回 {{ records: [], total: 0 }} 空信封（DD2）"
        );
    }

    // 超长 root（>1000 字符）：非法路径 → canonicalize 失败 reject，不误判 blank
    let long_root = env
        .data_dir
        .path()
        .join(format!("long-{}", "x".repeat(1024)))
        .to_string_lossy()
        .into_owned();
    let err_long = db_records(
        state.clone(),
        DbDimension::Workspace,
        long_root.clone(),
        "session".to_owned(),
        0,
        10,
    )
    .expect_err("超长非法路径 root 应 Err");
    assert!(
        err_long.contains("canonicalize"),
        "超长 root 按不存在库 reject（不误判 blank），实际: {err_long}"
    );
    assert!(
        !err_long.contains("空信封"),
        "非法路径 root 不落 blank 分支，实际: {err_long}"
    );

    // 含换行 / NUL / emoji 的非法路径 root：不 panic，按不存在库 reject
    let weird_root = env
        .data_dir
        .path()
        .join("weird\n\u{0}🚀root")
        .to_string_lossy()
        .into_owned();
    let err_weird = db_records(
        state.clone(),
        DbDimension::Workspace,
        weird_root,
        "session".to_owned(),
        0,
        10,
    )
    .expect_err("含特殊字符的非法路径 root 应 Err");
    assert!(
        err_weird.contains("canonicalize"),
        "特殊字符 root 按不存在库 reject 不 panic，实际: {err_weird}"
    );
}

/// DbDimension 枚举两值 scope 寻址：`Workspace` 按 root 寻址 workspace 库、
/// `User` 走全局库忽略 root，两库记录互不混列（scope 寻址语义零改动）。
#[test]
fn db_records_scope两值寻址两库记录互不混列() {
    let env = Env::new("records-scope-mix");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    // 全局库：1 条 workspace 注册记录（root 键）；workspace 库：2 条 session
    let global_record = add_workspace(&state, &env.ws("alpha"));
    let root = env.root_of("alpha");
    let ws = state.for_root(&root).expect("for_root 应成功");
    seed_session(&ws, "ses-mix-0");
    seed_session(&ws, "ses-mix-1");

    // Workspace scope：按 root 寻址 workspace 库，只见 session 记录
    let ws_page = db_records(
        state.clone(),
        DbDimension::Workspace,
        root.clone(),
        "session".to_owned(),
        0,
        10,
    )
    .expect("Workspace scope 扫描应成功");
    assert_eq!(
        ws_page.records.len(),
        2,
        "Workspace scope 见 workspace 库记录"
    );
    assert_eq!(
        ws_page.total, 2,
        "Workspace scope total 为 workspace 库计数"
    );
    assert!(
        ws_page
            .records
            .iter()
            .all(|envelope| envelope.key.is_string()),
        "Workspace scope 记录 key 为 session id 字符串信封（非 workspace root）"
    );
    assert!(
        ws_page
            .records
            .iter()
            .any(|envelope| envelope.key.as_str() == Some("ses-mix-0")),
        "Workspace scope 记录 key 为 session id 字符串"
    );

    // User scope：走全局库忽略 root，只见 workspace 注册记录，与 workspace 库不混列
    let user_page = db_records(
        state.clone(),
        DbDimension::User,
        root,
        "workspace".to_owned(),
        0,
        10,
    )
    .expect("User scope 扫描应成功");
    assert_eq!(user_page.records.len(), 1, "User scope 见全局库记录");
    assert_eq!(user_page.total, 1, "User scope total 为全局库计数");
    assert_eq!(
        user_page.records[0].key,
        serde_json::json!(global_record.root),
        "User scope 记录 key 为 workspace 注册记录 root"
    );
}

/// model 参数 str 边界：空串 / 超长串（>1000 字符）/ 特殊字符（`\n` `\0`
/// emoji）→ `Err(String)` reject 不静默吞错、错误串与 store 层一致。
#[test]
fn db_records_model名str边界_未知模型err且错误串与store层一致() {
    let env = Env::new("records-model-edge");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let long_model = "m".repeat(1024);
    for model in ["", long_model.as_str(), "no\npe\u{0}🚀"] {
        let err = db_records(
            state.clone(),
            DbDimension::User,
            String::new(),
            model.to_owned(),
            0,
            10,
        )
        .expect_err(&format!("未知模型名 {model:?} 应 Err"));
        let via_store = state
            .global()
            .scan(model, 0, 10)
            .expect_err("直连 store 同样 Err")
            .to_string();
        assert_eq!(
            err, via_store,
            "命令面错误串与 store 层一致（不静默吞错），model={model:?}"
        );
        assert!(err.contains("未知模型"), "错误串含「未知模型」语境: {err}");
    }
}

/// workspace 库打开失败（坏文件）→ `db_records` / `db_models` 显式 `Err(String)`
/// reject 不 panic（打开失败先于模型名校验）。
#[test]
fn db_records_workspace库文件损坏_两命令err不panic() {
    let env = Env::new("records-corrupt-db");
    let root;
    {
        let app = app_with_stores(&env);
        let state = app.state::<WorkspaceStores>();
        root = env.root_of("victim");
        add_workspace(&state, Path::new(&root));
        // 触发 workspace 库创建（预写一个合法库文件，供后续覆写为坏字节）
        let _ = state.for_root(&root).expect("for_root 首次应成功");
        // tauri 托管值生命周期长于 App（redb 文件锁不随 App drop 释放）：显式
        // 取回并销毁，坏字节才能覆写
        #[allow(deprecated)]
        let _stores = app.unmanage::<WorkspaceStores>().expect("应处于托管中");
    }

    // 覆写 workspace 库文件为坏字节（非法 redb 格式）
    let workspaces_dir = env.data_dir.path().join("workspaces");
    let mut db_files: Vec<PathBuf> = std::fs::read_dir(&workspaces_dir)
        .expect("workspaces 子树应存在")
        .flatten()
        .map(|entry| entry.path())
        .collect();
    assert_eq!(db_files.len(), 1, "应恰一个 workspace 库文件");
    let corrupt = db_files.pop().expect("库文件在场");
    std::fs::write(&corrupt, b"not-a-valid-redb-file").expect("覆写坏字节应成功");

    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let err = db_records(
        state.clone(),
        DbDimension::Workspace,
        root.clone(),
        "session".to_owned(),
        0,
        10,
    )
    .expect_err("坏文件 db_records 应 Err");
    assert!(!err.is_empty(), "错误串非空（reject 不静默吞错）: {err}");

    let err_models = db_models(state.clone(), DbDimension::Workspace, root)
        .expect_err("坏文件 db_models 应 Err");
    assert!(
        !err_models.is_empty(),
        "错误串非空（reject 不静默吞错）: {err_models}"
    );
}

/// scope 参数 IPC 序列化面：`DbDimension` 线值 `"user"` / `"workspace"` 合法、
/// 未知变体反序列化 reject 不 panic（命令层无自定义兜底——错误由 Tauri 序列化
/// 面承载，不落 `Result<T, String>` 通道）。
#[test]
fn db_records_scope枚举线值反序列化_未知变体reject不panic() {
    let ok_user: DbDimension =
        serde_json::from_value(serde_json::json!("user")).expect("线值 user 可反序列化");
    assert_eq!(ok_user, DbDimension::User, "user 线值 → User 变体");
    let ok_workspace: DbDimension =
        serde_json::from_value(serde_json::json!("workspace")).expect("线值 workspace 可反序列化");
    assert_eq!(
        ok_workspace,
        DbDimension::Workspace,
        "workspace 线值 → Workspace 变体"
    );

    for illegal in ["User", "WORKSPACE", "global", "", "unknown"] {
        let result = serde_json::from_value::<DbDimension>(serde_json::json!(illegal));
        assert!(
            result.is_err(),
            "未知变体 {illegal:?} 应反序列化 Err（大小写敏感、无别名兜底）"
        );
    }
    let null = serde_json::from_value::<DbDimension>(serde_json::Value::Null);
    assert!(null.is_err(), "null（IPC 缺参形态）应反序列化 Err 不 panic");
}
