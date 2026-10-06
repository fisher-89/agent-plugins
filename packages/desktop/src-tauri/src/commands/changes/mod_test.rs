//! `commands::changes` 的单元测试（test-design「commands/changes/mod.rs ->
//! mod_test.rs」节）：本组五命令——三读（`list_changes` / `get_change_detail`
//! / `read_artifact`）为持 State 的读薄包装（参数 → blank root 早退 →
//! `for_root` 取 workspace 库实例 → core 查询 → DTO，零直接 fs 访问）；
//! 二记录面（`create_change` / `archive_change`）三件事纪律（参数校验 →
//! 调写面 → 错误映射，name / goal 校验权威在写面）。
//!
//! 装置沿 workspaces 组先例：`tauri::test::mock_app()` + 真实
//! `WorkspaceStores`（tempdir 数据根）托管进 app，命令以 `app.state::<…>()`
//! 取 State 直调（State 可 clone）；种子经 store change 域操作面（`for_root`
//! 实例建档 / 开相 / 落账 / 归档翻转），fs 与 db 双真实组合（无 mock）。
//!
//! blank root 双口径并存不互换（既有口径持衡）：读命令空/空白 root 早退空
//! 结果语义（空列表 / `None`）；`create_change` / `archive_change` 显式
//! `Err`（写无空结果语义，不进入写面链路）。
//!
//! `archive_change` 以 [`archive_change_with`] 泛型测试缝直测（MockRuntime
//! 句柄，沿 `change_flow_start_with` 先例，无真实 Wry）。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use foundation::layout::{resolve, Layout};
use serde_json::json;
use store::{Store, WorkspaceStores};
use tauri::{App, Manager};
use workflow::model::{ChecklistItem, Verdict};
use workflow::queries as core_queries;
use workflow::state::{ChangeStateRecord, ChangeStatus, PhaseLogCommand};

use super::{archive_change_with, create_change, get_change_detail, list_changes, read_artifact};

/// 确定性时间戳（UTC unix 毫秒；2024-09-22 / 2026-02-02）。
const CREATED_AT: i64 = 1727000000000;
const ARCHIVED_AT: i64 = 1769990400000;
const START_AT: i64 = 1727000100000;
const LOGGED_AT: i64 = 1727000200000;

// ---------------------------------------------------------------------------
// 装置：tempdir 双根 + mock app 托管 WorkspaceStores + store 种子
// ---------------------------------------------------------------------------

/// 数据根 + workspace 根临时环境（tempfile RAII，测试结束自动清理）。
struct Env {
    data_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("changes-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("changes-cmd-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_dir, ws_root }
    }

    fn root(&self) -> String {
        self.ws_root.path().to_string_lossy().into_owned()
    }

    fn layout(&self) -> Layout {
        resolve(self.ws_root.path())
    }

    /// active change 目录树根（`openspec/changes/`）。
    fn changes_dir(&self) -> PathBuf {
        self.layout().changes_root
    }

    /// archive change 目录树根（`openspec/changes/archive/`）。
    fn archive_dir(&self) -> PathBuf {
        self.layout().archive_root
    }

    /// 在 active 树下预置一个 change 目录（含一个产物文件）。
    fn change_dir(&self, name: &str) -> PathBuf {
        let dir = self.changes_dir().join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        dir
    }
}

/// 以 MockRuntime 建测用 app，并 manage 真实 WorkspaceStores（打开 env 数据
/// 根的全局库；workspace 库经 for_root 惰性开）。
fn app_with(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app
}

/// 命令同源的 workspace 库实例（`for_root` 缓存缝——命令与种子命中同一实例）。
fn store_of(app: &App<tauri::test::MockRuntime>, root: &str) -> Arc<Store> {
    app.state::<WorkspaceStores>()
        .for_root(root)
        .expect("for_root 应成功")
}

/// db 建档 active 记录（status=active 起步）。
fn seed_active(store: &Store, name: &str) {
    store
        .create_change_record(ChangeStateRecord {
            name: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: CREATED_AT,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
        })
        .expect("建档 fixture 应成功");
}

/// db 建档 archived 记录（归档翻转后的形态：status + archived_at 在位）。
fn seed_archived(store: &Store, name: &str) {
    store
        .create_change_record(ChangeStateRecord {
            name: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: CREATED_AT,
            status: ChangeStatus::Archived,
            archived_at: Some(ARCHIVED_AT),
            active_phase: None,
        })
        .expect("建档 fixture 应成功");
}

/// db 建档 + 开相 + 落账一条 proposal 评估（含 checklist 一项；落账后
/// active_phase 清位——列表状态面 activePhase=null 的种子形态）。
fn seed_with_phase(store: &Store, name: &str) {
    seed_active(store, name);
    store
        .start_change_phase(name, "proposal", START_AT)
        .expect("开相 fixture 应成功");
    store
        .log_change_phase(&PhaseLogCommand {
            change: name.to_owned(),
            phase: "proposal".to_owned(),
            verdict: Verdict::Pass,
            report: "OK".to_owned(),
            skipped: false,
            checklist: vec![ChecklistItem {
                item: "问题清晰".to_owned(),
                pass: true,
                evidence: "L1-10".to_owned(),
            }],
            executor_session_id: None,
            evaluator_session_id: Some("eval-ses".to_owned()),
            decision_session_id: None,
            start_at: Some(START_AT),
            timestamp: LOGGED_AT,
        })
        .expect("落账 fixture 应成功");
}

// ---------------------------------------------------------------------------
// 读命令透传持衡（AC-4 接线半边）：薄包装不加工，夹具 = db 种子 + 真实目录树
// ---------------------------------------------------------------------------

#[test]
fn 读命令透传持衡_list与detail命令结果与core查询serde一致() {
    let env = Env::new("passthrough");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();

    // 种子三形态：db 建档 active（含相位落账）/ db archived（archive 树日期
    // 前缀目录在场）/ 磁盘-only（存量 CLI change，db 缺记录）
    {
        let store = store_of(&app, &root);
        seed_with_phase(&store, "seeded-active");
        seed_archived(&store, "seeded-archived");
    }
    fs::write(
        env.change_dir("seeded-active").join("tasks.md"),
        "- [x] 已完成\n- [ ] 待办\n",
    )
    .expect("写 tasks.md 失败");
    fs::create_dir_all(env.archive_dir().join("2026-02-02-seeded-archived"))
        .expect("创建 archive 目录失败");
    fs::create_dir_all(env.change_dir("docs-only")).expect("创建磁盘-only 目录失败");

    let layout = env.layout();
    let store = store_of(&app, &root);

    // list：命令面与 core 直调 serde 等值（同库同状态，零加工）
    let via_command = list_changes(state.clone(), root.clone());
    let via_core = core_queries::list_changes(&layout, store.as_ref());
    assert_eq!(
        serde_json::to_value(&via_command).expect("命令结果序列化失败"),
        serde_json::to_value(&via_core).expect("core 结果序列化失败"),
        "薄包装不加工"
    );

    // 内容抽查：active 并集（db 条目 + 磁盘-only 文档形态），主键自然序
    let value = serde_json::to_value(&via_command).expect("序列化失败");
    let active_names: Vec<&str> = value["active"]
        .as_array()
        .expect("active 为数组")
        .iter()
        .map(|entry| entry["name"].as_str().expect("条目含 name"))
        .collect();
    assert_eq!(active_names, vec!["docs-only", "seeded-active"]);
    let seeded = &value["active"][1];
    assert_eq!(seeded["status"], json!("active"), "db 条目携状态面");
    assert_eq!(seeded["activePhase"], json!(null), "落账清位 → null");
    assert_eq!(seeded["created"], json!("2024-09-22"), "建档日期出线");
    assert_eq!(
        value["active"][0]["status"],
        json!(null),
        "磁盘-only 文档形态零状态面"
    );

    // archive 月分组：db archived_at 驱动（2026-02 组可达）
    let groups = value["archiveGroups"].as_array().expect("月分组数组");
    assert_eq!(groups.len(), 1, "恰一个月组");
    assert_eq!(groups[0]["month"], json!("2026-02"));
    assert_eq!(
        groups[0]["changes"][0]["name"],
        json!("2026-02-02-seeded-archived"),
        "归档条目 name 为磁盘目录名（含日期前缀）"
    );
    assert_eq!(groups[0]["changes"][0]["status"], json!("archived"));

    // detail：命令面与 core 直调 serde 等值；建档 change 全状态面
    let via_detail = get_change_detail(state.clone(), root.clone(), "seeded-active".to_owned())
        .expect("应 Some");
    let via_core_detail =
        core_queries::change_detail(&layout, store.as_ref(), "seeded-active").expect("应 Some");
    assert_eq!(
        serde_json::to_value(&via_detail).expect("命令结果序列化失败"),
        serde_json::to_value(&via_core_detail).expect("core 结果序列化失败"),
        "薄包装不加工"
    );
    assert_eq!(via_detail.status, Some(ChangeStatus::Active));
    assert_eq!(via_detail.pipeline.len(), 9, "固定 9 站全量输出");
    let proposal = via_detail
        .pipeline
        .iter()
        .find(|station| station.phase == "proposal")
        .expect("proposal 站在场");
    assert_eq!(proposal.attempts.len(), 1, "落账 attempt 重组入站");
    assert_eq!(
        proposal.attempts[0].checklist.len(),
        1,
        "checklist 子行内联"
    );
    assert!(
        !via_detail.artifacts.is_empty(),
        "产物清单非空（tasks.md 特化 kind）"
    );
    assert!(via_detail.active_phase.is_none(), "落账清位 → null");

    // 未知 change → None（非错误）
    assert!(
        get_change_detail(state.clone(), root, "不存在".to_owned()).is_none(),
        "未知 change 名返回 None"
    );
}

#[test]
fn read_artifact信封出线持衡_五字段往返与负路径越权source拒绝() {
    let env = Env::new("read-artifact");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();

    // 种子：db 建档 + 磁盘 tasks.md（产物文件真实组合）
    {
        let store = store_of(&app, &root);
        seed_active(&store, "with-tasks");
    }
    fs::write(
        env.change_dir("with-tasks").join("tasks.md"),
        "- [x] 已完成\n- [ ] 待办\n",
    )
    .expect("写 tasks.md 失败");
    // 越权目标：change 目录树外的秘密文件（workspace 根）
    fs::write(env.ws_root.path().join("secret.md"), "# 不应被越权读取").expect("写根 secret 失败");

    // 正向：五字段信封往返（信封出线持衡）
    let envelope = read_artifact(
        state.clone(),
        root.clone(),
        "with-tasks".into(),
        "tasks-progress".into(),
        "tasks.md".into(),
    )
    .expect("有效读取应返回 Some");
    assert_eq!(envelope.kind, "tasks-progress");
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.payload["total"], 2);
    assert_eq!(envelope.payload["done"], 1);
    assert!(envelope.fallback_text.is_some(), "保底文本在场");

    // 负路径：未注册 kind / 不存在 source → None
    assert!(
        read_artifact(
            state.clone(),
            root.clone(),
            "with-tasks".into(),
            "file-log".into(),
            "tasks.md".into()
        )
        .is_none(),
        "未注册 kind → None"
    );
    assert!(
        read_artifact(
            state.clone(),
            root.clone(),
            "with-tasks".into(),
            "markdown-doc".into(),
            "没有这个.md".into()
        )
        .is_none(),
        "不存在 source → None"
    );

    // 越权 source 在命令边界被拒绝（core 守卫经命令链可达，不逃逸 change 目录）
    for hostile in [
        "../secret.md",
        "with-tasks/../../secret.md",
        "..\\..\\secret.md",
        "C:\\evil\\secret.md",
        "/secret.md",
        "..",
        "",
    ] {
        assert!(
            read_artifact(
                state.clone(),
                root.clone(),
                "with-tasks".into(),
                "markdown-doc".into(),
                hostile.to_string()
            )
            .is_none(),
            "命令边界必须拒绝敌意 source {hostile:?}"
        );
    }
    assert!(
        read_artifact(
            state.clone(),
            root,
            "with-tasks".into(),
            "markdown-doc".into(),
            "tasks.md".into()
        )
        .is_some(),
        "合法 source 不应被误伤"
    );
}

// ---------------------------------------------------------------------------
// create 接线（AC-6 命令半边）：写面三合一 + 返回 DTO + 立即可见可发起
// ---------------------------------------------------------------------------

#[test]
fn create接线_三合一落位_零workflow_json_返回dto且立即可见可发起() {
    let env = Env::new("create-wire");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let goal = "修复登录重试的竞态问题";

    let outcome = create_change(
        state.clone(),
        root.clone(),
        "fix-bug".to_owned(),
        goal.to_owned(),
    )
    .expect("合法输入经命令层应 Ok");

    // 返回 DTO 仅两字段：恰 `{"name":…,"created":…}` 零磁盘路径字段
    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("序列化为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["created", "name"], "恰两键且零磁盘路径字段");
    assert_eq!(outcome.name, "fix-bug");
    assert_eq!(
        outcome.created.len(),
        10,
        "created 为 UTC YYYY-MM-DD（取 db 建档 created_at）"
    );

    // 三合一磁盘半边：目录 + explore.md（goal 原文）；零 workflow.json 产出
    let dir = env.changes_dir().join("fix-bug");
    assert!(dir.is_dir(), "目录建树落位");
    assert_eq!(
        fs::read_to_string(dir.join("explore.md")).expect("读 explore.md 失败"),
        goal,
        "explore.md 落最初 goal 原文"
    );
    assert!(
        !dir.join("workflow.json").exists(),
        "零 workflow.json 产出（双向墙写半边）"
    );

    // 三合一 db 半边：建档记录在案（requirement + active）
    let record = store_of(&app, &root)
        .find_change_record("fix-bug")
        .expect("查档应成功")
        .expect("建档记录在案");
    assert_eq!(record.status, ChangeStatus::Active);
    assert_eq!(record.workflow_type, "requirement");
    assert!(record.active_phase.is_none(), "建档起步零开相");

    // 成功立即可见可发起：list / detail 即刻呈现
    let list = list_changes(state.clone(), root.clone());
    let entry = list
        .active
        .iter()
        .find(|entry| entry.name == "fix-bug")
        .expect("清单即刻可见");
    assert_eq!(entry.status, Some(ChangeStatus::Active));
    let detail =
        get_change_detail(state.clone(), root, "fix-bug".to_owned()).expect("详情即刻可见");
    assert_eq!(detail.status, Some(ChangeStatus::Active));
    assert_eq!(detail.pipeline.len(), 9, "空流水线 9 站全量（发起面就绪）");
}

// ---------------------------------------------------------------------------
// archive 接线（AC-7 命令半边）：双写成功 + ArchiveOutcome + 月分组可达
// ---------------------------------------------------------------------------

#[test]
fn archive接线_双写成功返回outcome且随后list按月分组可达() {
    let env = Env::new("archive-wire");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let name = "to-archive";

    // 种子：db 建档 + 磁盘目录（双载体真实组合；archive 树预建——真实
    // workspace 内 OpenSpec CLI 归档树恒在场，rename 目标父目录须存在）
    {
        let store = store_of(&app, &root);
        seed_active(&store, name);
    }
    fs::create_dir_all(env.archive_dir()).expect("创建 archive 树失败");
    fs::write(
        env.change_dir(name).join("proposal.md"),
        "# 提案\n\n- [x] 完成\n",
    )
    .expect("写 proposal 失败");

    let outcome = archive_change_with(app.handle().clone(), root.clone(), name.to_owned())
        .expect("归档应成功");

    // 返回 ArchiveOutcome：name + UTC 日期
    assert_eq!(outcome.name, name, "主键 name 不变");
    assert_eq!(
        outcome.archived_date.len(),
        10,
        "archived_date 为 UTC YYYY-MM-DD"
    );

    // db 半边：status 翻转 + archived_at 落库，主键 name 不变
    let record = store_of(&app, &root)
        .find_change_record(name)
        .expect("查档应成功")
        .expect("建档记录在案");
    assert_eq!(record.status, ChangeStatus::Archived, "db status 翻转");
    assert!(record.archived_at.is_some(), "archived_at 落库");
    assert_eq!(record.name, name, "主键 name 不随目录改名变");

    // fs 半边：active 树目录改名入 archive 树（日期前缀）
    assert!(
        !env.changes_dir().join(name).exists(),
        "active 树目录已挪走"
    );
    let archived_dir = env
        .archive_dir()
        .join(format!("{}-{name}", outcome.archived_date));
    assert!(archived_dir.is_dir(), "archive 树日期前缀目录落位");

    // 随后 list 命令按月分组可达（归档月组 + 前缀目录名条目）
    let list = list_changes(state.clone(), root);
    assert!(
        list.active.iter().all(|entry| entry.name != name),
        "active 清单不再含已归档条目"
    );
    assert_eq!(list.archive_groups.len(), 1, "恰一个月组（分组可达）");
    let group = &list.archive_groups[0];
    assert_eq!(
        group.month.as_deref(),
        Some(&outcome.archived_date[..7]),
        "db archived_at 驱动月分组"
    );
    assert_eq!(
        group.changes[0].name,
        format!("{}-{name}", outcome.archived_date),
        "归档条目 name 为磁盘目录名"
    );
    assert_eq!(group.changes[0].status, Some(ChangeStatus::Archived));
}

// ---------------------------------------------------------------------------
// archive 错误映射（三件事薄包装不吞错不加工）：无建档 / 目标冲突 / blank 参数
// ---------------------------------------------------------------------------

#[test]
fn archive错误映射_无建档与目标冲突与blank参数各自显式err() {
    let env = Env::new("archive-err");
    let app = app_with(&env);
    let root = env.root();

    // 无建档：active 树目录在场、db 零记录 → 显式 Err（拒绝先于一切变更）
    fs::create_dir_all(env.change_dir("legacy-only")).expect("创建 legacy 目录失败");
    let err = archive_change_with(app.handle().clone(), root.clone(), "legacy-only".to_owned())
        .expect_err("无建档应 Err");
    assert!(
        err.contains("legacy-only") && err.contains("未建档"),
        "无建档记因: {err}"
    );
    assert!(
        env.changes_dir().join("legacy-only").is_dir(),
        "零 fs 变更（目录未挪动）"
    );
    assert!(
        store_of(&app, &root)
            .list_change_records()
            .expect("查清单应成功")
            .is_empty(),
        "零 db 变更"
    );

    // 目标冲突：预置 archive 树 `YYYY-MM-DD-<name>`（日期取自一次先行归档的
    // 返回值——同 UTC 日内确定性）→ 先查拒绝，db 零变更
    {
        let store = store_of(&app, &root);
        seed_active(&store, "conflict-first");
        seed_active(&store, "conflict-second");
    }
    fs::create_dir_all(env.archive_dir()).expect("创建 archive 树失败");
    env.change_dir("conflict-first");
    env.change_dir("conflict-second");
    let today = archive_change_with(
        app.handle().clone(),
        root.clone(),
        "conflict-first".to_owned(),
    )
    .expect("先行归档应成功")
    .archived_date;
    fs::create_dir_all(env.archive_dir().join(format!("{today}-conflict-second")))
        .expect("预置冲突目标失败");
    let err = archive_change_with(
        app.handle().clone(),
        root.clone(),
        "conflict-second".to_owned(),
    )
    .expect_err("目标冲突应 Err");
    assert!(err.contains("归档目标已存在"), "目标冲突记因: {err}");
    assert!(
        env.changes_dir().join("conflict-second").is_dir(),
        "active 树目录未被挪动"
    );
    assert_eq!(
        store_of(&app, &root)
            .find_change_record("conflict-second")
            .expect("查档应成功")
            .expect("建档在案")
            .status,
        ChangeStatus::Active,
        "db 零变更（status 未翻转）"
    );

    // blank 参数：命令层显式文案（写无空结果语义）
    let err = archive_change_with(app.handle().clone(), String::new(), "任意".to_owned())
        .expect_err("blank root 应 Err");
    assert_eq!(err, "非法 root: 不得为空白");
    let err = archive_change_with(app.handle().clone(), root.clone(), "   ".to_owned())
        .expect_err("blank change 应 Err");
    assert_eq!(err, "非法 change: 不得为空白");
}

// ---------------------------------------------------------------------------
// create 错误映射：同名 active 冲突 → Err 且零目录零建档（D5 前置检查可达）
// ---------------------------------------------------------------------------

#[test]
fn create错误映射_同名active冲突err且零目录零建档() {
    let env = Env::new("create-conflict");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();

    // 种子：db 同名 active 在场（零磁盘目录——纯 db 冲突形态）
    {
        let store = store_of(&app, &root);
        seed_active(&store, "fix-bug");
    }

    let err = create_change(
        state.clone(),
        root.clone(),
        "fix-bug".to_owned(),
        "goal".to_owned(),
    )
    .expect_err("同名 active 冲突应 Err");
    assert!(
        err.contains("fix-bug") && err.contains("已存在同名建档记录"),
        "冲突记因（db 半边先于 IO）: {err}"
    );

    // 零目录创建 + db 零新建档
    assert!(!env.changes_dir().join("fix-bug").exists(), "零目录创建");
    let records = store_of(&app, &root)
        .list_change_records()
        .expect("查清单应成功");
    assert_eq!(records.len(), 1, "零新建档（仅种子一行）");
    assert_eq!(records[0].name, "fix-bug");
}

// ---------------------------------------------------------------------------
// blank root 双口径（既有口径持衡）：读命令早退空结果 / create 显式 Err
// ---------------------------------------------------------------------------

#[test]
fn blank_root双口径_读命令早退空结果_create显式err() {
    let env = Env::new("blank-root");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();

    for root in [String::new(), "   ".to_owned()] {
        // 读命令：空结果语义（不报错、不进入查询链路）
        let list = list_changes(state.clone(), root.clone());
        assert!(list.active.is_empty(), "空 root 清单为空");
        assert!(list.archive_groups.is_empty(), "空 root 月分组为空");
        assert!(
            get_change_detail(state.clone(), root.clone(), "任意".to_owned()).is_none(),
            "空 root 详情为 None"
        );
        assert!(
            read_artifact(
                state.clone(),
                root.clone(),
                "任意".into(),
                "markdown-doc".into(),
                "x.md".into()
            )
            .is_none(),
            "空 root 产物读取为 None"
        );

        // 写命令：显式 Err（不进入写面链路）
        let err = create_change(
            state.clone(),
            root.clone(),
            "fix-bug".to_owned(),
            "goal".to_owned(),
        )
        .expect_err("blank root create 应 Err");
        assert_eq!(
            err, "非法 root: 不得为空白",
            "命令层拦截文案（不进入写面链路）"
        );
    }

    assert!(
        !env.layout().changes_root.exists(),
        "零产生（拒绝面无目录无文件）"
    );
}
