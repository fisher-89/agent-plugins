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
//!
//! **寻址面 id 化**（desktop-change-db-identity）：三读一写的定位参数均为
//! change id（未建档 → 读命令空结果语义 / 写命令显式 `Err`，零磁盘目录解析
//! 回退）；name 仅作展示属性与磁盘面供给值（list 归档条目恒裸名、磁盘目录名
//! 日期前缀不参与出线）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation::layout::{resolve, Layout};
use serde_json::json;
use store::{Store, WorkspaceStores};
use tauri::{App, Manager};
use workflow::model::{ChecklistItem, Verdict};
use workflow::queries as core_queries;
use workflow::state::{ChangeStateRecord, ChangeStatus, PhaseLogCommand};

use super::{
    archive_change_with, create_change, create_change_with, get_change_detail, list_changes,
    read_artifact, read_exec_output,
};

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

    /// 主仓初始化为含一个提交的 git 仓（create 建域命令面的真实 git worktree
    /// 依赖；测试环境 PATH 可达 git 与产品硬依赖同口径）。
    fn as_git_repo(&self) {
        fs::write(self.ws_root.path().join("README.md"), "# 主仓夹具\n")
            .expect("写主仓初始文件失败");
        for args in [
            vec!["init"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "desktop-test"],
            vec!["add", "README.md"],
            vec!["commit", "-m", "init"],
        ] {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(self.ws_root.path())
                .args(&args)
                .output()
                .expect("git 拉起失败");
            assert!(
                output.status.success(),
                "git {} 失败: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

/// 以 MockRuntime 建测用 app，并 manage 真实 WorkspaceStores（打开 env 数据
/// 根的全局库；workspace 库经 for_root 惰性开）。
fn app_with(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    // 数据根注入（worktrees 落位派生的注入面，与 main.rs setup 同构）
    app.manage(env.data_dir.path().to_path_buf());
    app.manage(stores);
    // run 控制注册表（get_change_detail 统一视图活面投影读取——main.rs 同构托管）
    app.manage(Arc::new(orchestration::control::ChangeFlowControl::new()));
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
            id: name.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: CREATED_AT,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("建档 fixture 应成功");
}

/// db 建档 archived 记录（归档翻转后的形态：status + archived_at 在位）。
fn seed_archived(store: &Store, name: &str) {
    store
        .create_change_record(ChangeStateRecord {
            id: name.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: CREATED_AT,
            status: ChangeStatus::Archived,
            archived_at: Some(ARCHIVED_AT),
            active_phase: None,
            worktree: None,
            base_commit: None,
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
            change_id: name.to_owned(),
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
// 读命令 id 透传（AC-2 :262 段 / AC-5 接线半边）：薄包装不加工，夹具 = db 种子
// + 真实目录树
// ---------------------------------------------------------------------------

/// 读命令定位参数 id 透传：list / detail / read_artifact 三命令与 core 直调
/// serde 等值；归档条目 name 恒裸名（磁盘目录名日期前缀零出线）且 id 恒在案；
/// 月分组 `2026-02` 自 `archived_at` 驱动。
#[test]
fn 读命令id透传_列表详情产物与core查询serde等值且归档条目恒裸名携id() {
    let env = Env::new("passthrough");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();

    // 种子两形态：db 建档 active（含相位落账）/ db archived（archive 树日期
    // 前缀目录在场——磁盘目录名与记录裸名相异，出线归属对拍）
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

    let layout = env.layout();
    let store = store_of(&app, &root);

    // list：命令面与 core 直调 serde 等值（同库同状态，零加工）
    let via_command = list_changes(state.clone(), root.clone());
    let via_core = core_queries::list_changes(store.as_ref());
    assert_eq!(
        serde_json::to_value(&via_command).expect("命令结果序列化失败"),
        serde_json::to_value(&via_core).expect("core 结果序列化失败"),
        "薄包装不加工"
    );

    // 内容抽查：active = db 全量，主键自然序；条目携身份锚 id
    let value = serde_json::to_value(&via_command).expect("序列化失败");
    let active_names: Vec<&str> = value["active"]
        .as_array()
        .expect("active 为数组")
        .iter()
        .map(|entry| entry["name"].as_str().expect("条目含 name"))
        .collect();
    assert_eq!(active_names, vec!["seeded-active"], "db 单源全量出线");
    let seeded = &value["active"][0];
    assert_eq!(seeded["id"], json!("seeded-active"), "条目携身份锚 id");
    assert_eq!(seeded["status"], json!("active"), "db 条目携状态面");
    assert_eq!(seeded["activePhase"], json!(null), "落账清位 → null");
    assert_eq!(seeded["created"], json!("2024-09-22"), "建档日期出线");

    // archive 月分组：db archived_at 驱动（2026-02 组可达）；条目 name 恒裸名
    // （磁盘目录名 `2026-02-02-` 前缀零出线）且 id 恒在案
    let groups = value["archiveGroups"].as_array().expect("月分组数组");
    assert_eq!(groups.len(), 1, "恰一个月组");
    assert_eq!(
        groups[0]["month"],
        json!("2026-02"),
        "月分组自 archived_at（磁盘目录名日期前缀不参与）"
    );
    assert_eq!(
        groups[0]["changes"][0]["name"],
        json!("seeded-archived"),
        "归档条目 name 恒裸名（磁盘目录名日期前缀零出线）"
    );
    assert_eq!(
        groups[0]["changes"][0]["id"],
        json!("seeded-archived"),
        "归档条目 id 恒在案"
    );
    assert_eq!(groups[0]["changes"][0]["status"], json!("archived"));

    // read_artifact：命令面与 core 直调 serde 等值（id → 记录 → name 目录定位）
    let record = store
        .find_change_record("seeded-active")
        .expect("查档应成功")
        .expect("建档在案");
    let location =
        core_queries::locate_change(&layout, record.worktree.as_deref(), record.name.as_str())
            .expect("记录供给定位应命中");
    let phases = store
        .list_phase_records("seeded-active")
        .expect("相位史应成功");
    let via_artifact = read_artifact(
        state.clone(),
        root.clone(),
        "seeded-active".to_owned(),
        "tasks-progress".to_owned(),
        "tasks.md".to_owned(),
    )
    .expect("产物读取应 Some");
    let via_core_artifact =
        workflow::artifacts::read_artifact(&location.dir, &phases, "tasks-progress", "tasks.md")
            .expect("core 产物读取应 Some");
    assert_eq!(
        serde_json::to_value(&via_artifact).expect("命令结果序列化失败"),
        serde_json::to_value(&via_core_artifact).expect("core 结果序列化失败"),
        "薄包装不加工（read_artifact 命令面）"
    );

    // detail：命令面与 core 直调 serde 等值；建档 change 全状态面
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();
    let via_detail = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "seeded-active".to_owned(),
    )
    .expect("应 Some");
    let via_core_detail =
        core_queries::change_detail(&layout, store.as_ref(), "seeded-active").expect("应 Some");
    assert_eq!(
        serde_json::to_value(&via_detail.detail).expect("命令结果序列化失败"),
        serde_json::to_value(&via_core_detail).expect("core 结果序列化失败"),
        "薄包装不加工"
    );
    assert!(
        via_detail.active_run.is_none(),
        "零运行 run → activeRun null"
    );
    let via_detail = &via_detail.detail;
    assert_eq!(via_detail.id, "seeded-active", "详情携身份锚 id");
    assert_eq!(via_detail.name, "seeded-active", "详情 name 自记录直读");
    assert_eq!(via_detail.status, Some(ChangeStatus::Active));
    assert_eq!(
        via_detail.pipeline.len(),
        9,
        "固定 9 站全量输出（末列 archive）"
    );
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
}

/// 目录树字节快照（相对路径 → 内容；空内容行记目录名）：命令链路零写证据。
fn tree_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("遍历目录失败").flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("相对路径")
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                files.push((relative, Vec::new()));
                stack.push(path);
            } else {
                files.push((relative, fs::read(&path).expect("读文件失败")));
            }
        }
    }
    files.sort();
    files
}

// ---------------------------------------------------------------------------
// 磁盘-only 零呈现（AC-3 命令面半边）：db 单源——无记录的存量 CLI 目录零入列
// ---------------------------------------------------------------------------

/// 主仓磁盘-only 目录（存量 CLI change，db 零记录）在位：list 命令零呈现该
/// 条目、detail / read_artifact 命令按该 id 均为 None（不可达）、目录字节零
/// 变化、零 db 建档。
#[test]
fn 磁盘only零呈现_命令面零入列不可达且目录字节零变化() {
    let env = Env::new("disk-only");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();

    // 磁盘-only 形态：主仓 change 目录 + 产物文件在场，db 零记录
    let docs_only = env.change_dir("docs-only");
    fs::write(docs_only.join("proposal.md"), "# 存量 CLI change\n").expect("写产物失败");
    let before = tree_snapshot(&env.changes_dir());

    // list 命令零呈现：空清单（不入列、不报错）
    let list = list_changes(state.clone(), root.clone());
    assert!(list.active.is_empty(), "磁盘-only 零入列（db 单源命令面）");
    assert!(list.archive_groups.is_empty(), "零月分组");
    // detail / read_artifact 命令按该 id → None（未建档；零磁盘目录解析回退）
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();
    assert!(
        get_change_detail(state.clone(), control, root.clone(), "docs-only".to_owned()).is_none(),
        "磁盘-only 详情 None（未建档）"
    );
    assert!(
        read_artifact(
            state,
            root,
            "docs-only".to_owned(),
            "markdown-doc".to_owned(),
            "proposal.md".to_owned()
        )
        .is_none(),
        "磁盘-only 产物读取 None（未建档）"
    );

    // 目录字节零变化 + 零 db 建档（读取链路零写零建档）
    assert_eq!(
        tree_snapshot(&env.changes_dir()),
        before,
        "磁盘目录字节零变化"
    );
    assert!(
        store_of(&app, &env.root())
            .list_change_records()
            .expect("查清单应成功")
            .is_empty(),
        "零 db 建档"
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
// create 接线（AC-1 命令半边）：写面三合一 + 返回 DTO 增 id + 立即可见可发起
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create接线_建域落位_返回dto增id且按id立即可见可发起() {
    // 真实 git 仓夹具 + ProcessWorktree spawn 均依赖 PATH 可达 git——与
    // PATH 隔离用例（change_flow / exec）经共享锁串行化（命令级先例）
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("create-wire");
    env.as_git_repo();
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let goal = "修复登录重试的竞态问题";

    let outcome = create_change_with(
        app.handle().clone(),
        root.clone(),
        "fix-bug".to_owned(),
        goal.to_owned(),
        "fix-bug".to_owned(),
        "requirement".to_owned(),
    )
    .await
    .expect("合法输入经命令层应 Ok");

    // 返回 DTO 恰五键：id（铸出的身份锚）/ name / created / worktree（刻意
    // 出线的执行锚）/ warnings
    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("序列化为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["created", "id", "name", "warnings", "worktree"],
        "恰五键面（id 为身份锚、worktree 为刻意出线的执行锚）"
    );
    assert!(!outcome.id.is_empty(), "铸出 id 非空（身份锚）");
    assert_ne!(
        outcome.id, outcome.name,
        "铸出 id 与 name 相异（主键非 name）"
    );
    assert_eq!(outcome.name, "fix-bug");
    assert_eq!(
        outcome.created.len(),
        10,
        "created 为 UTC YYYY-MM-DD（取 db 建档 created_at）"
    );
    let worktree = vcs_runtime::worktree_dir(env.data_dir.path(), &root, "fix-bug");
    assert_eq!(
        outcome.worktree,
        worktree.to_string_lossy(),
        "worktree 落位 = data_root/worktrees/{{身份段}}/<name>（vcs 单点派生）"
    );

    // 建域磁盘半边：worktree 内目录 + explore.md（goal 原文）；主仓 active 树
    // 零目录（编辑落点囚于 worktree）；零 workflow.json 产出
    let dir = resolve(&worktree).changes_root.join("fix-bug");
    assert!(dir.is_dir(), "worktree 内目录建树落位");
    assert_eq!(
        fs::read_to_string(dir.join("explore.md")).expect("读 explore.md 失败"),
        goal,
        "explore.md 落最初 goal 原文"
    );
    assert!(
        !env.changes_dir().join("fix-bug").exists(),
        "主仓 active 树不含该目录"
    );
    assert!(
        !dir.join("workflow.json").exists(),
        "零 workflow.json 产出（双向墙写半边）"
    );

    // 三合一 db 半边：建档记录在案（按铸出 id 寻址；id 与库内逐字一致）
    let record = store_of(&app, &root)
        .find_change_record(&outcome.id)
        .expect("查档应成功")
        .expect("建档记录在案");
    assert_eq!(record.id, outcome.id, "CreateOutcome.id 与库内逐字一致");
    assert_eq!(record.name, "fix-bug", "name 为可展示属性");
    assert_eq!(record.status, ChangeStatus::Active);
    assert_eq!(record.workflow_type, "requirement");
    assert!(record.active_phase.is_none(), "建档起步零开相");
    assert_eq!(
        record.worktree.as_deref(),
        Some(worktree.to_string_lossy().as_ref()),
        "建档记录携 worktree 执行锚"
    );
    assert!(record.base_commit.is_some(), "建档记录携 base_commit 基线");

    // 成功立即可见可发起：list / detail 按 id 即刻呈现
    let list = list_changes(state.clone(), root.clone());
    let entry = list
        .active
        .iter()
        .find(|entry| entry.id == outcome.id)
        .expect("清单即刻可见（按 id 寻址）");
    assert_eq!(entry.name, "fix-bug", "条目 name 为展示属性");
    assert_eq!(entry.status, Some(ChangeStatus::Active));
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();
    let unified = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        outcome.id.clone(),
    )
    .expect("详情即刻可见（按 id 寻址）");
    assert_eq!(unified.detail.id, outcome.id, "详情身份锚与铸出同源");
    assert_eq!(unified.detail.status, Some(ChangeStatus::Active));
    assert_eq!(
        unified.detail.pipeline.len(),
        9,
        "空流水线 9 站全量（发起面就绪，末列 archive）"
    );
    assert!(unified.active_run.is_none());
    // name 非寻址键：按 name 查询不可达（一切寻址以 id 为准）
    assert!(
        get_change_detail(state, control, root, "fix-bug".to_owned()).is_none(),
        "name 非寻址键（未建档 id → None）"
    );
}

// ---------------------------------------------------------------------------
// create title 透传（explore-name-file-binding AC-2 / AC-8 手动新建默认）：
// create_change IPC 保持三参、内部 title = name；create_change_with 增 title
// 参数并透传 write::create；库内建档记录 title 与清单出线逐字一致。
// ---------------------------------------------------------------------------

/// create title 透传：`create_change_with` 显式 title → 库内建档记录 title
/// 逐字一致（name 裸名零污染）；`create_change` IPC 三参内部口径（title =
/// name）形态亦逐字落库；清单条目 title 随 ChangeSummary 投影出线。
#[tokio::test]
async fn create_title透传_显式值落库且默认name形态() {
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("create-title");
    env.as_git_repo();
    let app = app_with(&env);
    let root = env.root();

    // 显式 title：透传 write::create 并落库
    let explicit = create_change_with(
        app.handle().clone(),
        root.clone(),
        "fix-title".to_owned(),
        "goal".to_owned(),
        "人类可读标题".to_owned(),
        "requirement".to_owned(),
    )
    .await
    .expect("显式 title 应 Ok");
    let record = store_of(&app, &root)
        .find_change_record(&explicit.id)
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(record.title, "人类可读标题", "建档记录 title 逐字透传");
    assert_ne!(record.title, record.name, "title 与 name 双字段可辨");
    assert_eq!(record.name, "fix-title", "name 恒裸名零污染");

    // 默认形态（create_change IPC 内部 title = name 的口径）
    let defaulted = create_change_with(
        app.handle().clone(),
        root.clone(),
        "fix-default".to_owned(),
        "goal".to_owned(),
        "fix-default".to_owned(),
        "requirement".to_owned(),
    )
    .await
    .expect("默认 title 应 Ok");
    let defaulted_record = store_of(&app, &root)
        .find_change_record(&defaulted.id)
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(
        defaulted_record.title, defaulted_record.name,
        "create_change 内部 title = name（手动新建默认）"
    );

    // 清单 title 出线（ChangeSummary 投影）
    let list = list_changes(app.state::<WorkspaceStores>(), root.clone());
    let entry = list
        .active
        .iter()
        .find(|entry| entry.id == explicit.id)
        .expect("清单即刻可见");
    assert_eq!(entry.title, "人类可读标题", "清单条目 title 出线");
    assert_eq!(entry.name, "fix-title", "条目 name 恒裸名");
}

/// create_change IPC 四参签面（边界 / 改写）：title 不扩 IPC 入参面，
/// workflow_type 扩为第四业务入参——`create_change` 恰 (app, root, name, goal,
/// workflow_type) 五形参、`create_change_with` 六形参（title 为泛型测试缝追加）。
/// 编译面锚（不实际调用，不起运行时）。
#[test]
fn create_change四参签面_workflow_type扩ipc入参() {
    fn assert_five<F, Fut>(_f: F)
    where
        F: Fn(tauri::AppHandle, String, String, String, String) -> Fut,
    {
    }
    fn assert_six<F, Fut>(_f: F)
    where
        F: Fn(
            tauri::AppHandle<tauri::test::MockRuntime>,
            String,
            String,
            String,
            String,
            String,
        ) -> Fut,
    {
    }
    assert_five(create_change);
    assert_six(create_change_with::<tauri::test::MockRuntime>);
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

    // db 半边：status 翻转 + archived_at 落库，id / name 均不随目录改名变
    let record = store_of(&app, &root)
        .find_change_record(name)
        .expect("查档应成功")
        .expect("建档记录在案");
    assert_eq!(record.status, ChangeStatus::Archived, "db status 翻转");
    assert!(record.archived_at.is_some(), "archived_at 落库");
    assert_eq!(record.id, name, "主键 id 不随目录改名变");
    assert_eq!(record.name, name, "name 不随目录改名变");

    // fs 半边：active 树目录改名入 archive 树（日期前缀）
    assert!(
        !env.changes_dir().join(name).exists(),
        "active 树目录已挪走"
    );
    let archived_dir = env
        .archive_dir()
        .join(format!("{}-{name}", outcome.archived_date));
    assert!(archived_dir.is_dir(), "archive 树日期前缀目录落位");

    // 随后 list 命令按月分组可达：月组自 archived_at、条目 name 恒裸名
    // （磁盘目录名日期前缀零出线）且 id 恒在案
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
    assert_eq!(group.changes[0].name, name, "归档条目 name 恒裸名");
    assert_eq!(group.changes[0].id, name, "归档条目 id 恒在案");
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

    // blank 参数：命令层显式文案（写无空结果语义；id 语境）
    let err = archive_change_with(app.handle().clone(), String::new(), "任意".to_owned())
        .expect_err("blank root 应 Err");
    assert_eq!(err, "非法 root: 不得为空白");
    let err = archive_change_with(app.handle().clone(), root.clone(), "   ".to_owned())
        .expect_err("blank id 应 Err");
    assert_eq!(err, "非法 id: 不得为空白");
}

// ---------------------------------------------------------------------------
// create 错误映射：同名 active 冲突 → Err 且零目录零建档（D5 前置检查可达）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create错误映射_同名active冲突err且零目录零建档() {
    let env = Env::new("create-conflict");
    let app = app_with(&env);
    let root = env.root();

    // 种子：db 同名 active 在场（零磁盘目录——纯 db 冲突形态）
    {
        let store = store_of(&app, &root);
        seed_active(&store, "fix-bug");
    }

    let err = create_change_with(
        app.handle().clone(),
        root.clone(),
        "fix-bug".to_owned(),
        "goal".to_owned(),
        "fix-bug".to_owned(),
        "requirement".to_owned(),
    )
    .await
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

#[tokio::test]
async fn blank_root双口径_读命令早退空结果_create显式err() {
    let env = Env::new("blank-root");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    for root in [String::new(), "   ".to_owned()] {
        // 读命令：空结果语义（不报错、不进入查询链路）
        let list = list_changes(state.clone(), root.clone());
        assert!(list.active.is_empty(), "空 root 清单为空");
        assert!(list.archive_groups.is_empty(), "空 root 月分组为空");
        assert!(
            get_change_detail(
                state.clone(),
                control.clone(),
                root.clone(),
                "任意".to_owned()
            )
            .is_none(),
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
        let err = create_change_with(
            app.handle().clone(),
            root.clone(),
            "fix-bug".to_owned(),
            "goal".to_owned(),
            "fix-bug".to_owned(),
            "requirement".to_owned(),
        )
        .await
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

// ---------------------------------------------------------------------------
// worktree 维度（design D2/D12/D13）：create 拒绝映射 / detail 与 read_artifact
// worktree 感知 / list 归组随动 / 归档引导命令面
// ---------------------------------------------------------------------------

/// db 建档携 worktree 执行锚的 active 记录 + worktree 内产物树（merge 前主仓
/// 两树未命中的读命令夹具）。`id` 与 `name` 刻意相异：磁盘目录名恒由记录
/// `name` 供给（id → 记录 → name 分辨率单点的比对锚）。
fn seed_worktree_change(
    env: &Env,
    app: &App<tauri::test::MockRuntime>,
    id: &str,
    name: &str,
) -> PathBuf {
    let worktree = env
        .data_dir
        .path()
        .join("worktrees")
        .join("fixture-segment")
        .join(name);
    let change_dir = resolve(&worktree).changes_root.join(name);
    fs::create_dir_all(&change_dir).expect("预置 worktree change 目录失败");
    fs::write(change_dir.join("proposal.md"), "# worktree 内提案").expect("预置产物失败");
    store_of(app, &env.root())
        .create_change_record(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: CREATED_AT,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: Some(worktree.to_string_lossy().into_owned()),
            base_commit: Some("0000000000000000000000000000000000000001".to_owned()),
        })
        .expect("worktree 建档 fixture 应成功");
    worktree
}

/// create 命令拒绝映射：非 git 仓 root → 写面 Err 透传前端（引导文案面——
/// 不静默回退主 root 创建）；零目录零建档。
#[tokio::test]
async fn create拒绝映射_非git仓err透传且零产生() {
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("create-not-git");
    // 普通目录形态（不调 as_git_repo——非 git 仓）
    let app = app_with(&env);

    let err = create_change_with(
        app.handle().clone(),
        env.root(),
        "fix-bug".to_owned(),
        "goal".to_owned(),
        "fix-bug".to_owned(),
        "requirement".to_owned(),
    )
    .await
    .expect_err("非 git 仓应 Err");

    assert!(
        err.contains("非 git 仓"),
        "写面 Err 透传前端（引导文案面），实际: {err}"
    );
    let records = store_of(&app, &env.root())
        .list_change_records()
        .expect("查清单应成功");
    assert!(records.is_empty(), "零建档");
    assert!(!env.changes_dir().exists(), "零目录（主仓 active 树未建）");
    let worktrees = env.data_dir.path().join("worktrees");
    assert!(
        !worktrees.exists() || worktrees.read_dir().map(|d| d.count()).unwrap_or(0) == 0,
        "零 worktree 目录"
    );
}

/// detail worktree 感知：记录携 worktree + 主仓两树未命中 + worktree 内产物
/// 树 → `get_change_detail` 按 **id** Some 且 `worktree` 出线、artifacts 命中
/// worktree 内文件（record 先读后定位的命令面证据；目录名由记录 name 供给）。
#[tokio::test]
async fn detail_worktree感知_出线且产物命中worktree内文件() {
    let env = Env::new("detail-worktree");
    let app = app_with(&env);
    let worktree = seed_worktree_change(&env, &app, "id-wt-detail", "wt-detail-change");

    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();
    let detail = get_change_detail(
        app.state::<WorkspaceStores>(),
        control,
        env.root(),
        "id-wt-detail".to_owned(),
    )
    .expect("worktree 记录详情应 Some")
    .detail;

    assert_eq!(detail.id, "id-wt-detail", "身份锚 id 出线");
    assert_eq!(
        detail.name, "wt-detail-change",
        "name 自记录直读（id → 记录 → name 分辨率单点）"
    );
    assert_eq!(
        detail.worktree.as_deref(),
        Some(worktree.to_string_lossy().as_ref()),
        "worktree 出线（与库内记录同源）"
    );
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "markdown-doc" && artifact.source == "proposal.md"),
        "artifacts 命中 worktree 内文件（主仓两树未命中），实际: {:?}",
        detail.artifacts
    );
    assert_eq!(detail.status, Some(ChangeStatus::Active), "状态面在");
}

/// read_artifact worktree 命中：同上夹具 → `read_artifact` 经 id → 记录 →
/// worktree / name 回退读取产物信封成功（主仓 miss 不致落空）；legacy 记录 →
/// 主仓两树既有解析持衡（miss → None）。
#[tokio::test]
async fn read_artifact_worktree命中_主仓miss不落空且legacy持衡() {
    let env = Env::new("artifact-worktree");
    let app = app_with(&env);
    seed_worktree_change(&env, &app, "id-wt-artifact", "wt-artifact-change");

    // worktree 回退命中：markdown-doc 信封读取成功（按 id 寻址，目录名自记录）
    let envelope = read_artifact(
        app.state::<WorkspaceStores>(),
        env.root(),
        "id-wt-artifact".to_owned(),
        "markdown-doc".to_owned(),
        "proposal.md".to_owned(),
    )
    .expect("worktree 回退应命中");
    let payload = serde_json::to_value(&envelope).expect("信封序列化应成功");
    assert!(
        payload.to_string().contains("# worktree 内提案"),
        "信封载荷命中 worktree 内文件内容，实际: {payload}"
    );

    // legacy 半边：无 worktree 记录 + 主仓两树未命中 → None（既有解析持衡）
    let store = store_of(&app, &env.root());
    seed_active(&store, "legacy-miss");
    assert!(
        read_artifact(
            app.state::<WorkspaceStores>(),
            env.root(),
            "legacy-miss".to_owned(),
            "markdown-doc".to_owned(),
            "proposal.md".to_owned(),
        )
        .is_none(),
        "legacy 记录无 worktree 回退（主仓 miss → None 持衡）"
    );
}

/// list 归组随动：worktree 条目（active + 两树未命中）经 `list_changes` 命令
/// 入 active 组且状态面完整（读命令透传持衡——与 core list 行对拍；条目 id 与
/// 磁盘目录名（name）相异）。
#[tokio::test]
async fn list归组随动_worktree条目入active组状态面完整() {
    let env = Env::new("list-worktree");
    let app = app_with(&env);
    let store = store_of(&app, &env.root());
    seed_worktree_change(&env, &app, "id-wt-list", "wt-list-change");
    seed_active(&store, "regular-change"); // 对照组（同 active 组）

    let list = list_changes(app.state::<WorkspaceStores>(), env.root());

    let names: Vec<&str> = list
        .active
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["regular-change", "wt-list-change"],
        "worktree 条目入 active 组（按名排序）"
    );
    let entry = list
        .active
        .iter()
        .find(|entry| entry.name == "wt-list-change")
        .expect("worktree 条目应在场");
    assert_eq!(entry.id, "id-wt-list", "条目携身份锚 id（非目录名）");
    assert_eq!(entry.status, Some(ChangeStatus::Active), "状态面 status 在");
    assert!(entry.created.is_some(), "状态面 created 在");
}

/// 归档引导命令面：记录携 worktree + 两树未命中 → `archive_change_with` Err
/// 引导 merge 文案透传（AC-10 命令半边；与 core archive_test 文案同锚）。
/// id 与 name 相异：引导文案恒 name 化（id → 记录 → name 分辨率单点）。
#[tokio::test]
async fn 归档引导命令面_unmerged_err透传merge文案() {
    let env = Env::new("archive-guide");
    let app = app_with(&env);
    seed_worktree_change(&env, &app, "id-9f3a2c", "wt-unmerged");

    let err = archive_change_with(app.handle().clone(), env.root(), "id-9f3a2c".to_owned())
        .expect_err("未 merge 归档应显式拒绝");

    assert!(
        err.contains("merge") && err.contains("change/wt-unmerged"),
        "merge 引导文案透传（name 自记录供给），实际: {err}"
    );
    assert!(
        !err.contains("id-9f3a2c"),
        "引导文案不含 id（磁盘 / git 面恒 name 化），实际: {err}"
    );
    let record = store_of(&app, &env.root())
        .find_change_record("id-9f3a2c")
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(record.status, ChangeStatus::Active, "db 零变更");
}

// ---------------------------------------------------------------------------
// 统一查询装配（unify-run-state-persistence D11）：单命令一次返回「库读史 ∪
// 在飞 run 活面」——前端零双命令拼接（AC-7）
// ---------------------------------------------------------------------------

/// 统一视图全形态对拍：无运行 run → detail.runs 全史 + activeRun=null（单命
/// 令一次返回）；运行中 run → activeRun 六值状态 + steps 全词汇 emit 序 + ask
/// 载荷 + phase / attempt；终态收口后 → activeRun 退 null、runs 尾行可达
///（D9 收口后状态面）；startedAt 毫秒 → ISO 串（转换单点在本命令层）。
#[test]
fn 统一查询装配_runs全史与active_run活面与终态退场() {
    let env = Env::new("unified-view");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    {
        let store = store_of(&app, &root);
        seed_active(&store, "unified-change");
        // 库读史：run-1 全史（completed + 记因 + 起止）+ run-2 起始行（running）
        use workflow::state::{
            RunEdgeEntry, RunEdgeKind, RunFinishCommand, RunNodeEntry, RunStartCommand, RunStatus,
            RunStepKind, RunStepStatus,
        };
        store
            .start_change_run(&RunStartCommand {
                run_id: "run-1".to_owned(),
                change_id: "unified-change".to_owned(),
                started_at: 1_727_000_000_000,
            })
            .expect("run-1 发起应成功");
        store
            .finish_change_run(&RunFinishCommand {
                run_id: "run-1".to_owned(),
                change_id: "unified-change".to_owned(),
                status: RunStatus::Completed,
                reason: Some("收口记因".to_owned()),
                finished_at: 1_727_000_060_000,
                nodes: vec![
                    RunNodeEntry {
                        phase: "proposal".to_owned(),
                        attempt: 1,
                        step: RunStepKind::PhaseStart,
                        instance_no: 0,
                        status: RunStepStatus::Passed,
                        session_id: None,
                        detail: None,
                        output_ref: None,
                    },
                    RunNodeEntry {
                        phase: "proposal".to_owned(),
                        attempt: 1,
                        step: RunStepKind::Executor,
                        instance_no: 0,
                        status: RunStepStatus::Passed,
                        session_id: Some("ses-1".to_owned()),
                        detail: None,
                        output_ref: None,
                    },
                ],
                edges: vec![RunEdgeEntry {
                    from_node_key: "proposal:1:phase_start:0".to_owned(),
                    to_node_key: "proposal:1:executor:0".to_owned(),
                    edge_kind: RunEdgeKind::Forward,
                }],
            })
            .expect("run-1 收口应成功");
        store
            .start_change_run(&RunStartCommand {
                run_id: "run-2".to_owned(),
                change_id: "unified-change".to_owned(),
                started_at: 1_727_000_100_000,
            })
            .expect("run-2 发起应成功");
    }

    // 无运行 run（注册表空）→ activeRun=null，detail.runs 两条全史
    let idle = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "unified-change".to_owned(),
    )
    .expect("应 Some");
    assert!(idle.active_run.is_none(), "零注册表条目 → activeRun null");
    assert_eq!(
        idle.detail.runs.len(),
        2,
        "库读史全史（run-1 + run-2 起始行）"
    );
    assert_eq!(idle.detail.runs[0].run_id, "run-1");
    assert_eq!(
        idle.detail.runs[0].status,
        workflow::state::RunStatus::Completed
    );
    assert_eq!(
        idle.detail.runs[1].status,
        workflow::state::RunStatus::Running
    );

    // 运行中 run：起点第一写（walker 落库缝——store running 行）+ begin_run
    // 注册表登记 + publish 多步（全词汇 emit 序）+ ask 载荷
    let started_at = 1_727_000_200_000_i64;
    {
        let store = store_of(&app, &root);
        use workflow::state::RunStartCommand as SeedStart;
        store
            .start_change_run(&SeedStart {
                run_id: "run-3".to_owned(),
                change_id: "unified-change".to_owned(),
                started_at,
            })
            .expect("run-3 起始行应成功");
    }
    let guard = control
        .begin_run(&root, "unified-change", "run-3".to_owned(), started_at)
        .expect("登记 run 应成功");
    use orchestration::state::{ChangeStepKind, ChangeStepState, ChangeStepStatus, RunUpdate};
    for (phase, attempt, kind, status) in [
        (
            "implement",
            1u32,
            ChangeStepKind::Executor,
            ChangeStepStatus::Running,
        ),
        (
            "implement",
            1,
            ChangeStepKind::Executor,
            ChangeStepStatus::Passed,
        ),
        (
            "implement",
            1,
            ChangeStepKind::StaticCheck,
            ChangeStepStatus::Failed,
        ),
        (
            "implement",
            1,
            ChangeStepKind::VerdictGate,
            ChangeStepStatus::Passed,
        ),
    ] {
        guard.emit(RunUpdate::Step {
            step: ChangeStepState {
                phase: phase.to_owned(),
                attempt,
                step: kind,
                instance_no: 0,
                status,
                session_id: None,
                detail: None,
                output_ref: None,
            },
        });
    }
    guard.emit(RunUpdate::Ask {
        question: "是否回溯?".to_owned(),
        options: vec!["dev-design".to_owned()],
    });

    let live = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "unified-change".to_owned(),
    )
    .expect("应 Some");
    let active = live.active_run.as_ref().expect("运行中 activeRun 在场");
    assert_eq!(active.run_id, "run-3");
    assert_eq!(
        active.status,
        orchestration::ChangeRunStatus::WaitingAsk,
        "六值状态机（waitingAsk 停等面）"
    );
    assert_eq!(active.phase.as_deref(), Some("implement"));
    assert_eq!(active.attempt, Some(1));
    let ask = active.ask.as_ref().expect("ask 载荷透传");
    assert_eq!(ask.question, "是否回溯?");
    // 同稳定身份键 emit 就地合并（executor running → passed 并为一行终态）
    assert_eq!(
        active.nodes.len(),
        3,
        "nodes 按稳定身份键合并（executor / static_check / verdict_gate）"
    );
    assert_eq!(active.nodes[0].step, ChangeStepKind::Executor);
    assert_eq!(
        active.nodes[0].status,
        ChangeStepStatus::Passed,
        "起手 running 被收口 passed 就地合并"
    );
    assert_eq!(active.nodes[1].step, ChangeStepKind::StaticCheck);
    assert_eq!(active.nodes[2].step, ChangeStepKind::VerdictGate);
    assert!(
        active.edges.is_empty(),
        "零边 run 活面 edges 空数组（无空边通知风暴）"
    );
    // startedAt 毫秒 → ISO 串（与 detail.runs 时间口径同式）
    let started_iso = active.started_at.clone();
    assert!(
        started_iso.ends_with('Z') && started_iso.contains('T'),
        "startedAt ISO 串（命令层转换单点）: {started_iso}"
    );
    assert!(
        live.detail
            .runs
            .iter()
            .all(|run| run.started_at.as_deref().map(|s| s.ends_with('Z')) == Some(true)),
        "库读史时间口径同式"
    );

    // 终态收口后：注册表除名 → activeRun 退 null、runs 尾行（run-3 起始行）可达
    guard.finish(orchestration::ChangeRunStatus::Completed, None);
    let closed = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "unified-change".to_owned(),
    )
    .expect("应 Some");
    assert!(closed.active_run.is_none(), "终态除名 → activeRun 退 null");
    assert_eq!(
        closed.detail.runs.len(),
        3,
        "run-3 起始行已在库（runs 尾行）"
    );
    let tail = closed.detail.runs.last().expect("尾行在场");
    assert_eq!(tail.run_id, "run-3");
    assert_eq!(tail.status, workflow::state::RunStatus::Running);
}

/// 统一查询装配_运行中run多边活面（AC-6 / AC-9 通知桥消费面）：运行中 run 已
/// emit 多节点 + 四 kind 多边 → `activeRun.nodes` 全词汇 emit 序出线、
/// `activeRun.edges` 活面四 kind 逐条在场（端点身份串与节点身份键同式、包闭），
/// 与库读史起始行一次返回（查询结果权威，通知仅为触发信号）；终态收口后活面
/// 随注册表除名退场。
#[test]
fn 统一查询装配_运行中run多边活面() {
    let env = Env::new("unified-edges");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    let started_at = 1_727_000_400_000_i64;
    {
        let store = store_of(&app, &root);
        seed_active(&store, "edge-change");
        use workflow::state::RunStartCommand;
        store
            .start_change_run(&RunStartCommand {
                run_id: "run-5".to_owned(),
                change_id: "edge-change".to_owned(),
                started_at,
            })
            .expect("run-5 起始行应成功");
    }

    let guard = control
        .begin_run(&root, "edge-change", "run-5".to_owned(), started_at)
        .expect("登记 run 应成功");
    use orchestration::state::{
        ChangeStepEdge, ChangeStepKind, ChangeStepState, ChangeStepStatus, EdgeKind, RunUpdate,
    };
    // 节点累积：计划序 + 反馈重跑新实例 + 三门 + agent 步（emit 序断言面）
    for (phase, attempt, instance_no, kind, status) in [
        (
            "implement",
            1u32,
            0u32,
            ChangeStepKind::PhaseStart,
            ChangeStepStatus::Passed,
        ),
        (
            "implement",
            1,
            0,
            ChangeStepKind::Executor,
            ChangeStepStatus::Passed,
        ),
        (
            "implement",
            1,
            0,
            ChangeStepKind::StaticCheck,
            ChangeStepStatus::Failed,
        ),
        (
            "implement",
            1,
            1,
            ChangeStepKind::Executor,
            ChangeStepStatus::Passed,
        ),
        (
            "implement",
            1,
            0,
            ChangeStepKind::Evaluator,
            ChangeStepStatus::Failed,
        ),
        (
            "implement",
            1,
            0,
            ChangeStepKind::VerdictGate,
            ChangeStepStatus::Passed,
        ),
        (
            "implement",
            1,
            0,
            ChangeStepKind::Decision,
            ChangeStepStatus::Passed,
        ),
        (
            "implement",
            1,
            0,
            ChangeStepKind::WhitelistGate,
            ChangeStepStatus::Passed,
        ),
        (
            "implement",
            1,
            0,
            ChangeStepKind::RetryGate,
            ChangeStepStatus::Passed,
        ),
        (
            "implement",
            1,
            0,
            ChangeStepKind::PhaseLog,
            ChangeStepStatus::Failed,
        ),
        (
            "implement",
            2,
            0,
            ChangeStepKind::PhaseStart,
            ChangeStepStatus::Passed,
        ),
        (
            "dev-design",
            2,
            0,
            ChangeStepKind::PhaseStart,
            ChangeStepStatus::Passed,
        ),
    ] {
        guard.emit(RunUpdate::Step {
            step: ChangeStepState {
                phase: phase.to_owned(),
                attempt,
                step: kind,
                instance_no,
                status,
                session_id: None,
                detail: None,
                output_ref: None,
            },
        });
    }
    // 边累积：四 kind emit 序（forward / feedback / retry / backtrack）
    let key = |phase: &str, attempt: u32, kind: ChangeStepKind, instance_no: u32| {
        (phase.to_owned(), attempt, kind, instance_no)
    };
    for (from, to, kind) in [
        (
            key("implement", 1, ChangeStepKind::PhaseStart, 0),
            key("implement", 1, ChangeStepKind::Executor, 0),
            EdgeKind::Forward,
        ),
        (
            key("implement", 1, ChangeStepKind::StaticCheck, 0),
            key("implement", 1, ChangeStepKind::Executor, 1),
            EdgeKind::Feedback,
        ),
        (
            key("implement", 1, ChangeStepKind::PhaseLog, 0),
            key("implement", 2, ChangeStepKind::PhaseStart, 0),
            EdgeKind::Retry,
        ),
        (
            key("implement", 1, ChangeStepKind::RetryGate, 0),
            key("dev-design", 2, ChangeStepKind::PhaseStart, 0),
            EdgeKind::Backtrack,
        ),
    ] {
        guard.emit(RunUpdate::Edge {
            edge: ChangeStepEdge { from, to, kind },
        });
    }

    let unified = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "edge-change".to_owned(),
    )
    .expect("应 Some");
    let active = unified.active_run.as_ref().expect("运行中 activeRun 在场");
    assert_eq!(active.run_id, "run-5");

    // 节点面：全词汇 emit 序出线（feedback 重跑新实例独立成行，零串扰）
    let words: Vec<&str> = active.nodes.iter().map(|node| node.step.as_str()).collect();
    assert_eq!(
        words,
        vec![
            "phase_start",
            "executor",
            "static_check",
            "executor",
            "evaluator",
            "verdict_gate",
            "decision",
            "whitelist_gate",
            "retry_gate",
            "phase_log",
            "phase_start",
            "phase_start",
        ],
        "全词汇 emit 序出线（活面节点表非空且序稳定）"
    );
    let executors: Vec<u32> = active
        .nodes
        .iter()
        .filter(|node| node.step == ChangeStepKind::Executor)
        .map(|node| node.instance_no)
        .collect();
    assert_eq!(
        executors,
        vec![0, 1],
        "同 (phase, attempt, step) 多实例 instanceNo 递增可辨"
    );

    // 边面：四 kind emit 序逐条在场（活面边表非空——通知桥消费面）
    assert_eq!(active.edges.len(), 4, "活面边表 emit 序全量");
    let kinds: Vec<&str> = active
        .edges
        .iter()
        .map(|edge| edge.edge_kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        vec!["forward", "feedback", "retry", "backtrack"],
        "四 kind 边活面出线（emit 序）"
    );
    assert_eq!(active.edges[1].from_node_key, "implement:1:static_check:0");
    assert_eq!(active.edges[1].to_node_key, "implement:1:executor:1");
    assert_eq!(active.edges[3].from_node_key, "implement:1:retry_gate:0");
    assert_eq!(active.edges[3].to_node_key, "dev-design:2:phase_start:0");
    // 端点身份串恒落本 run 节点身份键集内（活面边表包闭）
    let node_keys: Vec<String> = active
        .nodes
        .iter()
        .map(|node| {
            format!(
                "{}:{}:{}:{}",
                node.phase,
                node.attempt,
                node.step.as_str(),
                node.instance_no
            )
        })
        .collect();
    for edge in &active.edges {
        assert!(
            node_keys.contains(&edge.from_node_key) && node_keys.contains(&edge.to_node_key),
            "活面边端点恒指本 run 节点身份键: {edge:?}"
        );
    }
    // 线面 camelCase 逐字（前端直读挂钩 fromNodeKey / toNodeKey / edgeKind）
    let wire = serde_json::to_value(&active.edges).expect("活面边表序列化失败");
    assert_eq!(
        wire[0]["fromNodeKey"],
        json!("implement:1:phase_start:0"),
        "活面边端点线面直出"
    );
    assert_eq!(
        wire[0]["edgeKind"],
        json!("forward"),
        "活面边 kind 线词直出"
    );

    // 库读史面：run-5 起始行同时在案（库面 ∪ 活面一次返回）
    assert!(
        unified
            .detail
            .runs
            .iter()
            .any(|run| run.run_id == "run-5" && run.status == workflow::state::RunStatus::Running),
        "库读史起始行在案（单命令一次返回）"
    );

    // 终态收口：注册表除名 → activeRun 退 null（活面边表随快照退场）
    guard.finish(orchestration::ChangeRunStatus::Completed, None);
    let closed = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "edge-change".to_owned(),
    )
    .expect("应 Some");
    assert!(closed.active_run.is_none(), "终态除名 → activeRun 退 null");
}

/// 统一查询装配_waitingconfirm停等形态（AC-7 活面六值状态机第二停等档）：步累
/// 积后 emit `ConfirmWait` 一拍 → activeRun.status=waitingConfirm、phase 为停
/// 等相位、attempt 承上一拍步行、ask=null（两停等形态互斥）、steps 累积器随
/// 行；同一 run 的库读史起始行同时在案（库面 ∪ 活面一面出）。
#[test]
fn 统一查询装配_waitingconfirm停等形态() {
    let env = Env::new("unified-confirm");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    let started_at = 1_727_000_300_000_i64;
    {
        let store = store_of(&app, &root);
        seed_active(&store, "confirm-change");
        use workflow::state::RunStartCommand;
        store
            .start_change_run(&RunStartCommand {
                run_id: "run-4".to_owned(),
                change_id: "confirm-change".to_owned(),
                started_at,
            })
            .expect("run-4 起始行应成功");
    }

    let guard = control
        .begin_run(&root, "confirm-change", "run-4".to_owned(), started_at)
        .expect("登记 run 应成功");
    use orchestration::state::{ChangeStepKind, ChangeStepState, ChangeStepStatus, RunUpdate};
    guard.emit(RunUpdate::Step {
        step: ChangeStepState {
            phase: "dev-design".to_owned(),
            attempt: 2,
            step: ChangeStepKind::Evaluator,
            instance_no: 0,
            status: ChangeStepStatus::Passed,
            session_id: Some("ses-eval".to_owned()),
            detail: None,
            output_ref: None,
        },
    });
    guard.emit(RunUpdate::ConfirmWait {
        phase: "test-design".to_owned(),
        archive: None,
    });

    let unified = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "confirm-change".to_owned(),
    )
    .expect("应 Some");
    let active = unified.active_run.as_ref().expect("停等中 activeRun 在场");
    assert_eq!(
        active.status,
        orchestration::ChangeRunStatus::WaitingConfirm,
        "六值状态机（waitingConfirm 停等面）"
    );
    assert_eq!(
        active.phase.as_deref(),
        Some("test-design"),
        "停等相位面在场（ConfirmWait 载荷投影）"
    );
    assert_eq!(active.attempt, Some(2), "attempt 承上一拍步行");
    assert!(active.ask.is_none(), "停等形态无 ask 载荷（两停等互斥）");
    assert_eq!(active.nodes.len(), 1, "节点累积器随行（停等不丢节点表）");
    assert!(
        active.started_at.ends_with('Z') && active.started_at.contains('T'),
        "startedAt ISO 串口径同式: {}",
        active.started_at
    );
    // 库读史与活面一面出：同一 run 的起始行在案（running 起始行，nodes / edges
    // 恒空数组）
    let same_run = unified
        .detail
        .runs
        .iter()
        .find(|run| run.run_id == "run-4")
        .expect("同一 run 库读史起始行在案");
    assert_eq!(same_run.status, workflow::state::RunStatus::Running);
    assert!(same_run.nodes.is_empty(), "在飞 run 起始行 nodes 恒空");
    assert!(same_run.edges.is_empty(), "在飞 run 起始行 edges 恒空");
}

/// `ActiveRunView.archive` 出线（正向）：归档前停等快照（`archive: Some`）经
/// `get_change_detail` 活面投影直出（`ChangeRunSnapshot.archive` → `ActiveRunView.
/// archive` 同值），普通停等 / 运行中恒 null（AC-7 / AC-10）。
#[test]
fn 统一查询装配_active_run_archive面两态出线() {
    let env = Env::new("unified-archive");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    let started_at = 1_727_000_400_000_i64;
    {
        let store = store_of(&app, &root);
        seed_active(&store, "archive-change");
        use workflow::state::RunStartCommand;
        store
            .start_change_run(&RunStartCommand {
                run_id: "run-5".to_owned(),
                change_id: "archive-change".to_owned(),
                started_at,
            })
            .expect("run-5 起始行应成功");
    }

    let guard = control
        .begin_run(&root, "archive-change", "run-5".to_owned(), started_at)
        .expect("登记 run 应成功");

    // ① 运行中：archive 槽恒 null（非停等点）
    let running = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "archive-change".to_owned(),
    )
    .expect("应 Some");
    let active = running.active_run.as_ref().expect("运行中 activeRun 在场");
    assert!(
        active.archive.is_none(),
        "运行中 archive 槽恒 null（非归档前停等点）"
    );

    // ② 归档前停等：前置只读载荷随行出线（完成度 / delta specs / 合入目标）
    let payload = orchestration::ArchivePreflight {
        name: "archive-change".to_owned(),
        completed: false,
        incomplete_phases: vec!["implement".to_owned()],
        missing_artifacts: vec!["design.md".to_owned()],
        delta_specs: vec!["desktop-flow".to_owned()],
        worktree: Some("/tmp/wt/archive-change".to_owned()),
        branch: Some("change/archive-change".to_owned()),
        merge_target: Some("main".to_owned()),
    };
    guard.emit(orchestration::state::RunUpdate::ConfirmWait {
        phase: "archive".to_owned(),
        archive: Some(payload.clone()),
    });

    let unified = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "archive-change".to_owned(),
    )
    .expect("应 Some");
    let active = unified.active_run.as_ref().expect("停等中 activeRun 在场");
    assert_eq!(
        active.status,
        orchestration::ChangeRunStatus::WaitingConfirm
    );
    assert_eq!(active.phase.as_deref(), Some("archive"));
    assert_eq!(
        active.archive.as_ref(),
        Some(&payload),
        "归档前置载荷自快照直出（逐字段同值）"
    );

    // 线面：camelCase 键 + null 留位（IPC 信封形状）
    let value = serde_json::to_value(&unified).expect("出线应成功");
    assert_eq!(value["activeRun"]["archive"]["name"], "archive-change");
    assert_eq!(value["activeRun"]["archive"]["completed"], false);
    assert_eq!(
        value["activeRun"]["archive"]["incompletePhases"],
        serde_json::json!(["implement"])
    );
    assert_eq!(
        value["activeRun"]["archive"]["deltaSpecs"],
        serde_json::json!(["desktop-flow"])
    );
    assert_eq!(value["activeRun"]["archive"]["mergeTarget"], "main");

    // 普通相位停等（archive=None）→ 槽清空；终态除名后 active_run 缺席
    guard.emit(orchestration::state::RunUpdate::ConfirmWait {
        phase: "acceptance".to_owned(),
        archive: None,
    });
    let unified = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "archive-change".to_owned(),
    )
    .expect("应 Some");
    let active = unified.active_run.as_ref().expect("停等中 activeRun 在场");
    assert!(
        active.archive.is_none(),
        "非归档停等点 archive 槽 null（不残留上一次载荷）"
    );
    guard.finish(orchestration::ChangeRunStatus::Completed, None);
    let closed = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "archive-change".to_owned(),
    )
    .expect("应 Some");
    assert!(
        closed.active_run.is_none(),
        "终态除名 → activeRun 退 null（archive 槽随之缺席）"
    );
}

/// 统一视图既有字段回归（回归）：`ActiveRunView` 既有字段（run_id / status /
/// phase / attempt / ask / started_at / nodes / edges）投影不变（AC-10 零改动锚）。
#[test]
fn 统一查询装配_active_run既有字段回归() {
    let env = Env::new("unified-legacy-fields");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    let started_at = 1_727_000_500_000_i64;
    {
        let store = store_of(&app, &root);
        seed_active(&store, "legacy-change");
        use workflow::state::RunStartCommand;
        store
            .start_change_run(&RunStartCommand {
                run_id: "run-6".to_owned(),
                change_id: "legacy-change".to_owned(),
                started_at,
            })
            .expect("run-6 起始行应成功");
    }
    let guard = control
        .begin_run(&root, "legacy-change", "run-6".to_owned(), started_at)
        .expect("登记 run 应成功");
    guard.emit(orchestration::state::RunUpdate::Ask {
        question: "越权回溯是否放行?".to_owned(),
        options: vec!["拒绝".to_owned(), "放行".to_owned()],
    });

    let unified = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        "legacy-change".to_owned(),
    )
    .expect("应 Some");
    let active = unified
        .active_run
        .as_ref()
        .expect("ask 停等中 activeRun 在场");
    assert_eq!(active.run_id, "run-6");
    assert_eq!(active.status, orchestration::ChangeRunStatus::WaitingAsk);
    assert_eq!(
        active.ask.as_ref().map(|ask| ask.question.as_str()),
        Some("越权回溯是否放行?"),
        "ask 载荷投影不变"
    );
    assert!(
        active.archive.is_none(),
        "ask 停等 archive 槽恒 null（新增字段零污染既有面）"
    );
    assert!(
        active.started_at.ends_with('Z') && active.started_at.contains('T'),
        "startedAt ISO 串口径不变: {}",
        active.started_at
    );
    assert!(active.nodes.is_empty() && active.edges.is_empty());
    assert!(active.phase.is_none() && active.attempt.is_none());
}

/// `archive_change` 裸双写逃生口回归（回归）：薄命令直调写面 `archive` 双写
/// （Seal 步之外的既有逃生口），随后列表归档组可达（AC-10 零改动锚）。
#[test]
fn archive_change裸双写逃生口回归() {
    let env = Env::new("archive-escape-hatch");
    let app = app_with(&env);
    let root = env.root();
    let store = store_of(&app, &root);
    seed_active(&store, "seed-change");
    fs::create_dir_all(env.archive_dir()).expect("创建 archive 树失败");
    fs::create_dir_all(env.change_dir("seed-change")).expect("创建 change 目录失败");

    let outcome = archive_change_with(app.handle().clone(), root.clone(), "seed-change".to_owned())
        .expect("裸双写应成功");
    assert_eq!(outcome.name, "seed-change");
    assert_eq!(outcome.archived_date.len(), 10, "归档日期 YYYY-MM-DD");

    let record = store
        .find_change_record("seed-change")
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(record.status, workflow::state::ChangeStatus::Archived);
    assert!(record.archived_at.is_some(), "archived_at 落值");

    // 列表归档组可达（裸名 + 月分组不变）
    let list = list_changes(app.state::<WorkspaceStores>(), root.clone());
    let names: Vec<&str> = list
        .archive_groups
        .iter()
        .flat_map(|group| group.changes.iter().map(|entry| entry.name.as_str()))
        .collect();
    assert!(
        names.contains(&"seed-change"),
        "归档条目入归档组: {names:?}"
    );
}

/// `archive_change` 写面异常（异常）：两树未命中 → `Err(String)` 透传不 panic，
/// 其余详情面不受阻断（AC-10 / D7 逃生口守卫）。
#[test]
fn archive_change写面异常err透传零阻断() {
    let env = Env::new("archive-escape-err");
    let app = app_with(&env);
    let root = env.root();
    let store = store_of(&app, &root);
    seed_active(&store, "pending-change");

    // 无磁盘目录（两树未命中）→ 显式 Err
    let err = archive_change_with(
        app.handle().clone(),
        root.clone(),
        "pending-change".to_owned(),
    )
    .expect_err("两树未命中应显式 Err");
    assert!(
        err.contains("目录未找到"),
        "写面记因透传（不 panic）: {err}"
    );

    // 其余详情面不受阻断：详情仍可查、状态未翻转
    let unified = get_change_detail(
        app.state::<WorkspaceStores>(),
        app.state::<Arc<orchestration::control::ChangeFlowControl>>(),
        root.clone(),
        "pending-change".to_owned(),
    )
    .expect("详情仍应可查");
    assert_eq!(
        unified.detail.status,
        Some(workflow::state::ChangeStatus::Active),
        "写面失败零 db 翻转"
    );
}

/// 统一视图早退与未知 id 未找到（既有早退口径保留）：blank root → None；
/// 磁盘目录在场而 db 缺记录的 id → None（未找到降级——MUST NOT 回退磁盘目录
/// 解析、MUST NOT 虚构活面）；未知 id（record 与定位双缺）→ None。
#[test]
fn 统一查询装配_早退与未知id未找到() {
    let env = Env::new("unified-edge");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    // blank root → None（既有早退语义，零改写）
    assert!(
        get_change_detail(
            state.clone(),
            control.clone(),
            "   ".to_owned(),
            "any".to_owned()
        )
        .is_none(),
        "blank root → None"
    );

    // 未建档 id：磁盘目录在场（存量 CLI change 形态）→ None（未找到降级——
    // 注册表即使有条目也按 id 寻址落空，不虚构活面）
    env.change_dir("legacy-docs");
    assert!(
        get_change_detail(
            state.clone(),
            control.clone(),
            root.clone(),
            "legacy-docs".to_owned(),
        )
        .is_none(),
        "磁盘目录在场而 db 缺记录 → None（零磁盘目录解析回退）"
    );

    // 未知 id（record 与定位双缺）→ None（既有语义不变）
    assert!(
        get_change_detail(state, control, root, "不存在".to_owned()).is_none(),
        "未知 id → None"
    );
}

// ---------------------------------------------------------------------------
// read_exec_output（change-flow-show-step-only，AC-5）：IPC 薄命令暴露
// `infra/exec` 输出记录回查——磁盘面按 change **name** 定位（与
// `record_exec_output` 同口径），读单一 `.log` 全量字节并 UTF-8 lossy 成串；
// miss（未知 / 非法 exec_id / blank root）恒 None，非错误。worktree / 归档
// 感知（read_artifact 同式）：record 以 exec root 落盘，worktree-backed
// change 的记录在 worktree 内、归档 change 的记录在 archive 树（日期前缀），
// 均 name → 记录 → locate_change 三态定位后回读。
// ---------------------------------------------------------------------------

/// 落一条输出记录（真实 infra/exec 落盘——命令面取数直调 State 直测）。
fn record(root: &str, change_name: &str, bytes: &[u8]) -> String {
    exec::record_exec_output(Path::new(root), change_name, bytes).expect("落盘应成功")
}

#[test]
fn read_exec_output命中_内容为落盘log字节() {
    let env = Env::new("exec-hit");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let exec_id = record(&root, "add-feature", b"suite stdout\nsuite stderr\n");

    let text = read_exec_output(
        state.clone(),
        root.clone(),
        "add-feature".to_owned(),
        exec_id.clone(),
    )
    .expect("已知 exec_id 应命中");
    assert_eq!(
        text, "suite stdout\nsuite stderr\n",
        "内容 = 落盘 .log 字节（UTF-8 直读）"
    );
    // 落盘面：单一 .log（.err 退役）
    let dir = foundation::layout::change_test_reports(Path::new(&root), "add-feature")
        .join("exec-outputs");
    assert!(dir.join(format!("{exec_id}.log")).is_file());
    assert!(!dir.join(format!("{exec_id}.err")).exists());
}

#[test]
fn read_exec_output非utf8字节_lossy替换符不panic() {
    let env = Env::new("exec-lossy");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    // 非法 UTF-8 序列（0xFF / 0xFE / 0xC3 后缺续字节）——不 panic 不报错
    let exec_id = record(&root, "add-feature", b"head\xff\xfemid\xc3tail");
    let text = read_exec_output(state, root, "add-feature".to_owned(), exec_id)
        .expect("非法字节仍应命中（值非 UTF-8 不构成 miss）");
    assert_eq!(
        text, "head\u{FFFD}\u{FFFD}mid\u{FFFD}tail",
        "非法字节以 lossy 替换符呈现"
    );
}

#[test]
fn read_exec_output按change_name定位_与id相异时仍命中() {
    let env = Env::new("exec-name-keyed");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let change_id = "0199a2f0-2010-7e45-8a9b-000000002010";
    let exec_id = record(&root, "add-feature", b"name-keyed\n");

    // name 命中（与 record_exec_output 同口径）
    assert_eq!(
        read_exec_output(
            state.clone(),
            root.clone(),
            "add-feature".to_owned(),
            exec_id.clone()
        )
        .as_deref(),
        Some("name-keyed\n")
    );
    // id 形态寻址 miss（磁盘面恒按 name，非 id——id 只作命令面寻址键）
    assert!(
        read_exec_output(state, root, change_id.to_owned(), exec_id).is_none(),
        "按 id 寻址 miss（name 化磁盘面）"
    );
}

#[test]
fn read_exec_output未命中与非法exec_id恒none() {
    let env = Env::new("exec-miss");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let exec_id = record(&root, "add-feature", b"payload");

    assert!(read_exec_output(
        state.clone(),
        root.clone(),
        "add-feature".to_owned(),
        "exec-unknown".to_owned()
    )
    .is_none());
    for hostile in ["", "..", "../secret", "a/b", r"a\b", "C:evil", "exec-1.log"] {
        assert!(
            read_exec_output(
                state.clone(),
                root.clone(),
                "add-feature".to_owned(),
                hostile.to_owned()
            )
            .is_none(),
            "非法 exec_id {hostile:?} 应拒绝（miss 非错误，零路径逃逸）"
        );
    }
    assert!(
        read_exec_output(state, root, "ghost-change".to_owned(), exec_id).is_none(),
        "未定位到 change 目录 → None"
    );
}

#[test]
fn read_exec_output_blank_root早退none() {
    let env = Env::new("exec-blank");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let exec_id = record(&root, "add-feature", b"payload");

    for blank in ["", "   ", "\t"] {
        assert!(
            read_exec_output(
                state.clone(),
                blank.to_owned(),
                "add-feature".to_owned(),
                exec_id.clone()
            )
            .is_none(),
            "blank root 早退 None: {blank:?}"
        );
    }
}

#[test]
fn read_exec_output空输出与超长输出边界() {
    let env = Env::new("exec-boundary");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();

    // 空输出 → Some("")（命中语义，非 miss）
    let empty = record(&root, "add-feature", b"");
    assert_eq!(
        read_exec_output(state.clone(), root.clone(), "add-feature".to_owned(), empty),
        Some(String::new()),
        "空输出 → Some(\"\")"
    );

    // 超长输出（>1000 chars）→ 完整返回不截断
    let long = format!("{}\n尾行标记", "行".repeat(1200));
    let exec_id = record(&root, "add-feature", long.as_bytes());
    let text =
        read_exec_output(state, root, "add-feature".to_owned(), exec_id).expect("超长应命中");
    assert_eq!(text, long, "超长输出完整返回不截断");
    assert!(text.ends_with("尾行标记"));
}

/// db 建档带 worktree 执行锚的 active 记录（worktree-backed change 形态——
/// record 落盘以 exec root 即此锚）。
fn seed_active_with_worktree(store: &Store, name: &str, worktree: &Path) {
    store
        .create_change_record(ChangeStateRecord {
            id: name.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: CREATED_AT,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: Some(worktree.to_string_lossy().into_owned()),
            base_commit: None,
        })
        .expect("建档 fixture 应成功");
}

/// worktree 感知（回归锚：record 以 exec root 落盘——worktree-backed change
/// 的 `.log` 只在 worktree 内，主仓按 name 直读恒 miss 的原缺陷面）：name →
/// 记录供给 worktree → locate_change 定位 worktree 目录 → 回读命中。
#[test]
fn read_exec_output_worktree记录经name定位回读命中() {
    let env = Env::new("exec-worktree");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    // worktree 锚（数据根下落位，形态与产品 `data_root/worktrees/…/<name>` 同构）
    let worktree = env.data_dir.path().join("worktrees").join("add-feature");
    seed_active_with_worktree(&store_of(&app, &root), "add-feature", &worktree);
    // 落盘以 worktree 为锚（record_exec_output 同式——exec root 口径）
    let exec_id = exec::record_exec_output(&worktree, "add-feature", b"worktree stdout\n")
        .expect("worktree 落盘应成功");
    // 主仓 active 树无该 change 目录（worktree-backed change 主仓零目录）
    assert!(!env.changes_dir().join("add-feature").exists());

    let text = read_exec_output(
        state.clone(),
        root.clone(),
        "add-feature".to_owned(),
        exec_id.clone(),
    )
    .expect("worktree 内记录经 name 定位应命中");
    assert_eq!(text, "worktree stdout\n", "内容 = worktree 落盘字节");
    // 未知 exec_id 仍 miss（定位命中 ≠ 记录命中）
    assert!(read_exec_output(
        state,
        root,
        "add-feature".to_owned(),
        "exec-unknown".to_owned()
    )
    .is_none());
}

/// 归档树感知：change 目录归档改名（`archive/<date>-<name>`）后，裸 name 定位
/// 仍命中（locate_change 日期前缀后缀匹配覆盖）；记录归档形态（worktree 清位
/// 感知——stale worktree 目录缺席走 archive 态）。
#[test]
fn read_exec_output_归档日期前缀目录命中() {
    let env = Env::new("exec-archive");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let exec_id = record(&root, "add-feature", b"archived stdout\n");
    seed_archived(&store_of(&app, &root), "add-feature");
    // 归档形态：active 目录改名进 archive 树（日期前缀目录名）
    fs::create_dir_all(env.archive_dir()).expect("创建 archive 根失败");
    fs::rename(
        env.changes_dir().join("add-feature"),
        env.archive_dir().join("2026-10-13-add-feature"),
    )
    .expect("归档改名失败");

    let text = read_exec_output(state, root, "add-feature".to_owned(), exec_id)
        .expect("归档树日期前缀目录应命中");
    assert_eq!(text, "archived stdout\n", "内容随 change 目录迁移仍可回读");
}

/// 同名共存（D11：归档同名合法）候选探测序：非 archived 记录的 worktree 先探
///（stale 不抢定位），miss 后逐候选下探——两同名 change 各自的 exec_id 均可
/// 回读（exec_id 全局唯一，探测永不取回他 change 的记录）。
#[test]
fn read_exec_output_同名共存候选探测两change各自可回读() {
    let env = Env::new("exec-dup-name");
    let app = app_with(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root();
    let store = store_of(&app, &root);
    // archived 同名记录 + 仍在盘的 stale worktree（归档不清理 worktree，D13 手动边界）
    let stale = env.data_dir.path().join("worktrees").join("stale");
    let archived = ChangeStateRecord {
        id: "archived-id".to_owned(),
        name: "add-feature".to_owned(),
        title: "add-feature".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: CREATED_AT,
        status: ChangeStatus::Archived,
        archived_at: Some(ARCHIVED_AT),
        active_phase: None,
        worktree: Some(stale.to_string_lossy().into_owned()),
        base_commit: None,
    };
    store
        .create_change_record(archived)
        .expect("archived 建档 fixture 应成功");
    // active 同名记录（id 相异、worktree 在场）
    let live = env.data_dir.path().join("worktrees").join("live");
    seed_active_with_worktree(&store, "add-feature", &live);
    let stale_exec =
        exec::record_exec_output(&stale, "add-feature", b"stale\n").expect("stale 落盘应成功");
    let live_exec =
        exec::record_exec_output(&live, "add-feature", b"live\n").expect("live 落盘应成功");

    // active 记录 worktree 先探命中
    let text = read_exec_output(
        state.clone(),
        root.clone(),
        "add-feature".to_owned(),
        live_exec,
    )
    .expect("active 记录 worktree 定位应命中");
    assert_eq!(
        text, "live\n",
        "非 archived 记录 worktree 先探（stale 不抢定位）"
    );
    // live 锚 miss 后下探 stale 锚 → archived change 的记录同样可回读
    let text = read_exec_output(state, root, "add-feature".to_owned(), stale_exec)
        .expect("探测序下探 stale worktree 应命中");
    assert_eq!(text, "stale\n", "archived 记录的 stale worktree 次探可回读");
}

// ---------------------------------------------------------------------------
// workflow_type 四参透传（desktop-test-only-flow AC-7）：create_change /
// create_change_with 的 workflow_type 入参落库 + 详情列序出线 + 非法值拒绝
// ---------------------------------------------------------------------------

/// create_change_with 透传 test-only 类型（正向 / 新增）：落库
/// `record.workflow_type == "test-only"`，返回 DTO 仍恰五键
///（created / id / name / warnings / worktree）。
#[tokio::test]
async fn create_workflow_type透传test_only且dto五键不变() {
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("create-wf-test-only");
    env.as_git_repo();
    let app = app_with(&env);
    let root = env.root();

    let outcome = create_change_with(
        app.handle().clone(),
        root.clone(),
        "only-tests".to_owned(),
        "给既有模块补测试用例".to_owned(),
        "only-tests".to_owned(),
        "test-only".to_owned(),
    )
    .await
    .expect("合法 test-only 应 Ok");

    let record = store_of(&app, &root)
        .find_change_record(&outcome.id)
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(
        record.workflow_type, "test-only",
        "workflow_type 透传写面并落库"
    );
    assert_eq!(record.name, "only-tests", "name 为可展示属性");

    // DTO 恰五键（类型入参不扩出线面）
    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("序列化为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["created", "id", "name", "warnings", "worktree"],
        "CreateOutcome 恰五键"
    );
}

/// create_change 四参 IPC 的 title=name 口径（正向 / 新增）：IPC 命令以
/// `title = name` 调用泛型缝——以 `create_change_with` 同口径显式传
/// `title = name` 复现该链路，落库 title 与 name 逐字相等且 workflow_type
/// 不丢参（`create_change` 本体为 Wry 句柄命令，MockRuntime 不可直调，签面由
/// `create_change四参签面_workflow_type扩ipc入参` 编译期锚定）。
#[tokio::test]
async fn create_change四参title口径透传workflow_type() {
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("create-wf-ipc");
    env.as_git_repo();
    let app = app_with(&env);
    let root = env.root();

    let outcome = create_change_with(
        app.handle().clone(),
        root.clone(),
        "ipc-only-tests".to_owned(),
        "四参链路 goal".to_owned(),
        "ipc-only-tests".to_owned(),
        "test-only".to_owned(),
    )
    .await
    .expect("四参链路应 Ok");

    let record = store_of(&app, &root)
        .find_change_record(&outcome.id)
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(record.workflow_type, "test-only", "workflow_type 不丢参");
    assert_eq!(
        record.title, record.name,
        "create_change 内部 title = name（手动新建默认）"
    );
    assert_eq!(record.title, "ipc-only-tests");
}

/// 大小写 / 空白 canonical（边界 / 新增）：`"Test-Only "` → 落库 `"test-only"`。
#[tokio::test]
async fn create_workflow_type大小写空白canonical落库() {
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("create-wf-canonical");
    env.as_git_repo();
    let app = app_with(&env);
    let root = env.root();

    let outcome = create_change_with(
        app.handle().clone(),
        root.clone(),
        "canonical-wf".to_owned(),
        "canonical goal".to_owned(),
        "canonical-wf".to_owned(),
        "Test-Only ".to_owned(),
    )
    .await
    .expect("规范化后命中应 Ok");

    let record = store_of(&app, &root)
        .find_change_record(&outcome.id)
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(record.workflow_type, "test-only", "落 canonical 小写值");
}

/// 非法 workflow_type 拒绝（异常 / 新增）：显式 `Err`（文案通吃两值）、零建档
/// 零建域（无目录、无 worktree 落位）。
#[tokio::test]
async fn create非法workflow_type拒绝_零建档零建域() {
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("create-wf-invalid");
    env.as_git_repo();
    let app = app_with(&env);
    let root = env.root();

    for (idx, workflow_type) in ["bug-fix", "", " ", "\t", "refactor"].iter().enumerate() {
        let name = format!("bad-wf-{idx}");
        let err = create_change_with(
            app.handle().clone(),
            root.clone(),
            name.clone(),
            "非法类型 goal".to_owned(),
            name.clone(),
            (*workflow_type).to_owned(),
        )
        .await
        .expect_err("非法 workflow_type 应 Err");

        assert!(
            err.contains("workflow_type")
                && err.contains("requirement")
                && err.contains("test-only"),
            "写面校验 Err 透传（列出合法值集），实际: {err}"
        );
        assert!(
            !env.changes_dir().join(&name).exists(),
            "零目录创建（校验全 IO 前置）"
        );
        assert!(
            !env.data_dir.path().join("worktrees").join(&name).exists(),
            "零 worktree 落位"
        );
    }
    // 边界：超长（>1000 字符）与特殊字符（NUL / 内嵌换行 / emoji）类型值同拒
    //（写面字符集与长度零兜底，不截断、不折叠，校验全 IO 前置）
    let long_unknown = "x".repeat(1200);
    let edge_cases = [
        long_unknown,
        "requirement\u{0}".to_owned(),
        "require\nment".to_owned(),
        "test-only🚀".to_owned(),
    ];
    for (idx, workflow_type) in edge_cases.iter().enumerate() {
        let name = format!("bad-wf-edge-{idx}");
        let err = create_change_with(
            app.handle().clone(),
            root.clone(),
            name.clone(),
            "非法类型 goal".to_owned(),
            name.clone(),
            workflow_type.clone(),
        )
        .await
        .expect_err("超长 / 特殊字符 workflow_type 应 Err");

        assert!(
            err.contains("workflow_type"),
            "Err 记因携字段名，实际: {err}"
        );
        assert!(
            !env.changes_dir().join(&name).exists(),
            "零目录创建（校验全 IO 前置）"
        );
    }
    assert!(
        store_of(&app, &root)
            .list_change_records()
            .expect("查清单应成功")
            .is_empty(),
        "拒绝面 db 零建档"
    );
}

/// 跨模块组合（详情出线 / 正向）：`get_change_detail` 对 test-only 记录 →
/// `pipeline` 6 列（code-analyze 第 2 列、archive 第 6 列）；对 requirement
/// 记录 → 9 列（末列 archive、无 code-analyze）。
#[tokio::test]
async fn get_change_detail列序随workflow_type出线() {
    let _path_guard = crate::commands::TEST_PATH_LOCK
        .lock()
        .expect("PATH 锁不可中毒");
    let env = Env::new("detail-wf-columns");
    env.as_git_repo();
    let app = app_with(&env);
    let root = env.root();
    let state = app.state::<WorkspaceStores>();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    let test_only = create_change_with(
        app.handle().clone(),
        root.clone(),
        "only-tests".to_owned(),
        "补测试 goal".to_owned(),
        "only-tests".to_owned(),
        "test-only".to_owned(),
    )
    .await
    .expect("test-only 建档应 Ok");
    let requirement = create_change_with(
        app.handle().clone(),
        root.clone(),
        "full-flow".to_owned(),
        "常规需求 goal".to_owned(),
        "full-flow".to_owned(),
        "requirement".to_owned(),
    )
    .await
    .expect("requirement 建档应 Ok");

    let only = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        test_only.id.clone(),
    )
    .expect("test-only 详情应可达");
    let phases: Vec<&str> = only
        .detail
        .pipeline
        .iter()
        .map(|station| station.phase.as_str())
        .collect();
    assert_eq!(
        phases,
        vec![
            "proposal",
            "code-analyze",
            "test-design",
            "test-gen",
            "test-execution",
            "archive"
        ],
        "test-only 六列出线（code-analyze 第 2 列 / archive 第 6 列）"
    );

    let full = get_change_detail(
        state.clone(),
        control.clone(),
        root.clone(),
        requirement.id.clone(),
    )
    .expect("requirement 详情应可达");
    let phases: Vec<&str> = full
        .detail
        .pipeline
        .iter()
        .map(|station| station.phase.as_str())
        .collect();
    assert_eq!(phases.len(), 9, "requirement 九列出线");
    assert_eq!(phases[8], "archive", "archive 为末列");
    assert!(
        !phases.iter().any(|phase| *phase == "code-analyze"),
        "requirement 列序零 code-analyze 占位"
    );
}

/// 跨模块组合（详情出线 / 异常）：`workflow_type="bug-fix"` 记录 →
/// `get_change_detail` 返回 `None`（读面不 fallback requirement）。
#[test]
fn get_change_detail未知workflow_type恒none() {
    let env = Env::new("detail-wf-unknown");
    let app = app_with(&env);
    let root = env.root();
    let state = app.state::<WorkspaceStores>();
    let control = app.state::<Arc<orchestration::control::ChangeFlowControl>>();

    // 种子：db 直插未知类型记录（写面已拒——存量 / 异常形态单点可达）
    store_of(&app, &root)
        .create_change_record(ChangeStateRecord {
            id: "legacy-unknown".to_owned(),
            name: "legacy-unknown".to_owned(),
            title: "legacy-unknown".to_owned(),
            workflow_type: "bug-fix".to_owned(),
            created_at: CREATED_AT,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("建档 fixture 应成功");

    assert!(
        get_change_detail(
            state.clone(),
            control.clone(),
            root.clone(),
            "legacy-unknown".to_owned()
        )
        .is_none(),
        "未知 workflow_type → 详情 None（不 fallback requirement）"
    );
    // 对照：未知 id 同语义（既有持衡面）
    assert!(
        get_change_detail(state, control, root, "ghost-id".to_owned()).is_none(),
        "未建档 id → None"
    );
}
