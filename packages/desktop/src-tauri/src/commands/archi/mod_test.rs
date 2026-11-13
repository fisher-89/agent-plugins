//! `commands::archi` 的单元测试：六命令薄包装 ↔ `*_inner` 领域组装纯函数一一对应、
//! root 有效性校验（空白 / 非目录 / 不存在 → `Err`）、只读命令零写盘、`archi_set_gate`
//! 的原子写回与「任一校验失败零写入」，以及 `archi_change_summary` 的 store 记录解析
//! （worktree / 归档回落）。
//!
//! 装置沿 `commands/changes/mod_test.rs` 先例：`tauri::test::mock_app()` + 真实
//! `WorkspaceStores`（tempdir 数据根）托管进 app，`archi_change_summary` 以
//! `app.state::<…>()` 取 State 直调；无 State 的五命令直调原签名。命令层零领域规则
//! 即被测语义——投影全在 `archi`，此处只验证「参数 → 调用 → 错误映射」。

use std::fs;
use std::path::{Path, PathBuf};

use foundation::layout::archi_models_dir;
use store::{Store, WorkspaceStores};
use tauri::{App, Manager};
use workflow::state::{ChangeStateRecord, ChangeStatus};

use archi::drift::ArchiDriftMode;
use archi::GateTier;

use super::{
    archi_api_check, archi_api_check_inner, archi_api_generate, archi_api_generate_inner,
    archi_change_summary, archi_change_summary_inner, archi_drift, archi_drift_inner,
    archi_file_tree, archi_file_tree_inner, archi_graph, archi_graph_inner, archi_module,
    archi_module_inner, archi_set_gate, archi_set_gate_inner,
};

/// 确定性时间戳（UTC unix 毫秒）。
const CREATED_AT: i64 = 1727000000000;

// ---------------------------------------------------------------------------
// 装置：tempdir workspace 根 + 真实模型 / 文档盘面
// ---------------------------------------------------------------------------

/// 一台 tempdir（std 自建；本 crate 有 tempfile dev-dep，但只在 shell 侧需要，
/// 这里保持零依赖的自建形态，用于同时表达「数据根」与「workspace 根」）。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("archi-cmd-test-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn root(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
        path
    }

    fn write_model(&self, name: &str, content: &str) -> PathBuf {
        let dir = archi_models_dir(&self.0);
        fs::create_dir_all(&dir).expect("创建模型目录失败");
        let path = dir.join(name);
        fs::write(&path, content).expect("写模型文件失败");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const CORE: &str = concat!(
    "# 核心包\n",
    "[element.\"App\"]\n",
    "kind = \"package\"\n",
    "\n",
    "[element.\"App.Auth\"]\n",
    "kind = \"domain\"\n",
    "gate = \"off\"\n",
    "docs = [\"./src/auth/README.md\"]\n",
    "\n",
    "[element.\"App.Billing\"]\n",
    "kind = \"module\"\n",
    "\n",
    "[[relation]]\n",
    "from = \"App.Auth\"\n",
    "to = \"App.Billing\"\n",
);

/// 健康 workspace 根：`01-core.toml` + 一份显式文档。
fn healthy(tag: &str) -> TempDir {
    let temp = TempDir::new(tag);
    temp.write_model("01-core.toml", CORE);
    temp.write("src/auth/README.md", "# 认证说明\n\n正文\n");
    temp
}

/// 模型 + 文档字节快照（「零写盘」的行为化断言）。
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn collect(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let Ok(reader) = fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<PathBuf> = reader.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect(&path, out);
            } else if let Ok(bytes) = fs::read(&path) {
                out.push((path, bytes));
            }
        }
    }
    let mut out: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    collect(root, &mut out);
    out
}

/// mock app 托管真实 `WorkspaceStores`（temp 数据根）。
fn app_with(data_root: &Path) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(data_root).expect("打开测试全局库失败");
    app.manage(stores);
    app
}

/// 命令同源的 workspace 库实例（`for_root` 缓存缝）。
fn store_of(app: &App<tauri::test::MockRuntime>, root: &str) -> std::sync::Arc<Store> {
    app.state::<WorkspaceStores>()
        .for_root(root)
        .expect("for_root 应成功")
}

/// db 建档一条 change 记录（active / archived 两形态）。
fn seed_record(store: &Store, id: &str, status: ChangeStatus, worktree: Option<String>) {
    store
        .create_change_record(ChangeStateRecord {
            id: id.to_owned(),
            name: id.to_owned(),
            title: id.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: CREATED_AT,
            status,
            archived_at: None,
            active_phase: None,
            worktree,
            base_commit: None,
        })
        .expect("建档 fixture 应成功");
}

// ---------------------------------------------------------------------------
// 包装 ↔ inner 一一对应
// ---------------------------------------------------------------------------

/// 正向：三命令与其 `*_inner` 结果经 `serde_json` 逐字段一致（命令层零加工）。
#[test]
fn 三命令与inner结果逐字段一致() {
    let temp = healthy("wrapper");

    let graph_outer = archi_graph(temp.root()).expect("archi_graph 应 Ok");
    let graph_inner = archi_graph_inner(temp.path()).expect("inner 应 Ok");
    assert_eq!(
        serde_json::to_value(&graph_outer).expect("序列化"),
        serde_json::to_value(&graph_inner).expect("序列化"),
        "archi_graph 与 _inner 逐字段一致"
    );

    let module_outer = archi_module(temp.root(), "App.Auth".to_owned()).expect("Ok");
    let module_inner = archi_module_inner(temp.path(), "App.Auth").expect("Ok");
    assert_eq!(
        serde_json::to_value(&module_outer).expect("序列化"),
        serde_json::to_value(&module_inner).expect("序列化"),
        "archi_module 与 _inner 逐字段一致"
    );

    // 写命令：包装与 inner 均 Ok，且结果同为 `()`
    archi_set_gate(temp.root(), "App.Auth".to_owned(), GateTier::Warn).expect("包装 Ok");
    archi_set_gate_inner(temp.path(), "App.Auth", GateTier::Warn).expect("inner Ok");
    let reread = archi_module_inner(temp.path(), "App.Auth").expect("Ok");
    assert_eq!(reread.element.gate, GateTier::Warn, "两次改档结果一致");
}

/// 正向：`archi_change_summary` 包装（持 State）与 `_inner` 结果一致。
#[test]
fn change_summary包装与inner一致() {
    let data = TempDir::new("summary-data");
    let main = TempDir::new("summary-main");
    let next = TempDir::new("summary-next");
    main.write_model("01.toml", "[element.\"App\"]\nkind = \"package\"\n");
    next.write_model(
        "01.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Billing\"]\nkind = \"module\"\n",
        ),
    );

    let app = app_with(data.path());
    let store = store_of(&app, &main.root());
    seed_record(
        &store,
        "add-billing",
        ChangeStatus::Active,
        Some(next.root()),
    );

    let outer = archi_change_summary(
        app.state::<WorkspaceStores>(),
        main.root(),
        "add-billing".to_owned(),
    )
    .expect("包装应 Ok");
    let inner = archi_change_summary_inner(main.path(), Some(&next.root())).expect("inner 应 Ok");
    assert_eq!(
        serde_json::to_value(&outer).expect("序列化"),
        serde_json::to_value(&inner).expect("序列化"),
        "包装与 _inner 逐字段一致"
    );
    let outer = outer.expect("有改动即 Some");
    assert_eq!(outer.elements_added, vec!["App.Billing".to_owned()]);
}

// ---------------------------------------------------------------------------
// root 校验（八命令共用）
// ---------------------------------------------------------------------------

/// 异常：root 为 `""` / 空白 / 指向文件 / 不存在 → 八命令（含 `archi_api_check` /
/// `archi_api_generate` 及其 `*_inner`）一律 `Err`，且不触达投影（写命令零写入）。
/// `archi_drift` / `archi_file_tree` / 两新命令的校验均前置，故 `Err` 时零 vcs
/// 零枚举零比对零写盘。
#[test]
fn 八命令root校验四形态一律err() {
    let temp = TempDir::new("root-check");
    let file_root = temp.write("not-a-dir.txt", "我是文件\n");
    let missing = temp.path().join("no-such-dir");
    let invalid_roots: [PathBuf; 4] = [
        PathBuf::from(""),
        PathBuf::from("   "),
        file_root.clone(),
        missing.clone(),
    ];

    let data = TempDir::new("root-check-data");
    let app = app_with(data.path());
    for root in &invalid_roots {
        let rendering = root.to_string_lossy().into_owned();
        assert!(
            archi_graph(rendering.clone()).is_err(),
            "archi_graph root 非法应 Err"
        );
        assert!(
            archi_module(rendering.clone(), "App".to_owned()).is_err(),
            "archi_module root 非法应 Err"
        );
        assert!(
            archi_set_gate(rendering.clone(), "App".to_owned(), GateTier::Strict).is_err(),
            "archi_set_gate root 非法应 Err"
        );
        assert!(
            archi_change_summary(
                app.state::<WorkspaceStores>(),
                rendering.clone(),
                "any".to_owned()
            )
            .is_err(),
            "archi_change_summary root 非法应 Err"
        );
        assert!(
            archi_drift(rendering.clone(), ArchiDriftMode::Full).is_err(),
            "archi_drift root 非法应 Err（校验前置，零 vcs）"
        );
        assert!(
            archi_file_tree(rendering.clone()).is_err(),
            "archi_file_tree root 非法应 Err（校验前置，零枚举）"
        );
        assert!(
            archi_api_check(rendering.clone()).is_err(),
            "archi_api_check root 非法应 Err（校验前置，零比对）"
        );
        assert!(
            archi_api_check_inner(root).is_err(),
            "archi_api_check_inner root 非法应 Err"
        );
        assert!(
            archi_api_generate(rendering.clone()).is_err(),
            "archi_api_generate root 非法应 Err（校验前置，零写盘）"
        );
        assert!(
            archi_api_generate_inner(root).is_err(),
            "archi_api_generate_inner root 非法应 Err"
        );
    }

    // 写命令零写入：非法 root 下目标文件不存在即证明未落盘
    assert!(
        !archi_models_dir(&file_root).exists(),
        "非法 root 下不产生任何模型目录（写命令零写入）"
    );
}

// ---------------------------------------------------------------------------
// 读命令：两态 / 停摆 / 越界 / 零写盘
// ---------------------------------------------------------------------------

/// 正向 / 异常：健康模型完整、坏模型停摆（两数组皆空 + 诊断非空）。
#[test]
fn graph正常态与停摆态() {
    let temp = healthy("graph");
    let graph = archi_graph(temp.root()).expect("Ok");
    assert_eq!(graph.elements.len(), 3);
    assert_eq!(graph.relations.len(), 1);
    assert!(graph.diagnostics.is_empty());

    let broken = TempDir::new("graph-broken");
    broken.write_model("01.toml", "[element.\"App.Auth\"]\nkind = \"domain\"\n");
    let graph = archi_graph(broken.root()).expect("停摆仍 Ok");
    assert!(graph.elements.is_empty() && graph.relations.is_empty());
    assert!(!graph.diagnostics.is_empty(), "停摆态带诊断");
}

/// 正向 / 异常：元素存在 → 全字段；元素缺席 → `Err` 含总量提示；root 非法 →
/// `Err`。
#[test]
fn module三形态() {
    let temp = healthy("module");
    let detail = archi_module(temp.root(), "App.Auth".to_owned()).expect("Ok");
    assert_eq!(detail.element.fqn, "App.Auth");
    assert_eq!(detail.depends_on.len(), 1);
    assert_eq!(detail.docs.len(), 1);

    let absent = archi_module(temp.root(), "App.Missing".to_owned()).expect_err("缺席应 Err");
    assert!(absent.contains("元素不存在"), "文案点名缺席: {absent}");
    assert!(absent.contains('3'), "文案含元素总数: {absent}");

    assert!(
        archi_module(String::new(), "App".to_owned()).is_err(),
        "root 非法应 Err"
    );
}

// ---------------------------------------------------------------------------
// 读命令零写盘 / 写命令：原子写回 / 零写入 / 往返
// ---------------------------------------------------------------------------
#[test]
fn 五条读命令零写盘() {
    let data = TempDir::new("nowrite-data");
    let temp = healthy("nowrite");
    // 文件树面需要进树源文件（`healthy` 语料只有模型与 md 文档）
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    let app = app_with(data.path());
    let store = store_of(&app, &temp.root());
    seed_record(&store, "nowrite", ChangeStatus::Active, Some(temp.root()));

    let before = snapshot(temp.path());
    let _ = archi_graph(temp.root());
    let _ = archi_module(temp.root(), "App.Auth".to_owned());
    let _ = archi_change_summary(
        app.state::<WorkspaceStores>(),
        temp.root(),
        "nowrite".to_owned(),
    );
    let file_tree = archi_file_tree(temp.root()).expect("文件树命令应 Ok");
    assert!(!file_tree.entries.is_empty(), "语料应产出条目");
    let api_check = archi_api_check(temp.root()).expect("检查即生成预览应 Ok");
    assert!(!api_check.meta.tool_version.is_empty(), "载元投影在案");
    let after = snapshot(temp.path());
    assert_eq!(before, after, "五条读命令零写盘（含 archi_api_check）");
}

/// 正向：改档后目标行变、其余内容与注释逐字保留；目录内无临时文件残留；读回为
/// 新档位。
#[test]
fn set_gate改档原子写回无残留() {
    let temp = healthy("set-gate");
    archi_set_gate(temp.root(), "App.Auth".to_owned(), GateTier::Strict).expect("改档 Ok");

    let source =
        fs::read_to_string(archi_models_dir(temp.path()).join("01-core.toml")).expect("读模型文件");
    assert!(source.contains("gate = \"strict\""), "目标行变为新档位");
    assert!(
        source.contains("# 核心包"),
        "注释逐字保留（toml_edit 渲染，非字符串替换）"
    );
    let reread = archi_module(temp.root(), "App.Auth".to_owned()).expect("Ok");
    assert_eq!(reread.element.gate, GateTier::Strict, "读回为新档位");

    let mut names: Vec<String> = fs::read_dir(archi_models_dir(temp.path()))
        .expect("读模型目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["01-core.toml".to_owned()], "无临时文件残留");
}

/// 异常：元素缺席 / root 非法 → `Err` 且该文件字节零变化。
#[test]
fn set_gate失败零写入() {
    let temp = healthy("set-gate-err");
    let path = archi_models_dir(temp.path()).join("01-core.toml");
    let before = fs::read(&path).expect("读模型文件");

    assert!(
        archi_set_gate(temp.root(), "App.Missing".to_owned(), GateTier::Strict).is_err(),
        "元素缺席应 Err"
    );
    assert_eq!(
        fs::read(&path).expect("读模型文件"),
        before,
        "校验失败零写入"
    );

    assert!(
        archi_set_gate(String::new(), "App.Auth".to_owned(), GateTier::Strict).is_err(),
        "root 非法应 Err"
    );
    assert_eq!(
        fs::read(&path).expect("读模型文件"),
        before,
        "root 非法零写入"
    );
}

/// 边界：三档逐值往返；目标元素在第二文件（其余文件零变化）；元素无显式 `gate`
/// 行 → 插入。
#[test]
fn set_gate往返与跨文件与插入() {
    let temp = TempDir::new("set-gate-往返");
    let core_path = temp.write_model("01-core.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let auth_path = temp.write_model(
        "02-auth.toml",
        "[element.\"App.Auth\"]\nkind = \"domain\"\n",
    );
    let core_before = fs::read(&core_path).expect("读 core");

    for gate in [
        GateTier::Strict,
        GateTier::Warn,
        GateTier::Off,
        GateTier::Strict,
    ] {
        archi_set_gate(temp.root(), "App.Auth".to_owned(), gate).expect("往返改档 Ok");
        let reread = archi_module(temp.root(), "App.Auth".to_owned()).expect("Ok");
        assert_eq!(reread.element.gate, gate, "三档逐值往返");
    }
    assert_eq!(
        fs::read(&core_path).expect("读 core"),
        core_before,
        "第二文件（非目标）零变化"
    );

    let auth_source = fs::read_to_string(&auth_path).expect("读 auth");
    assert!(
        auth_source.contains("gate = "),
        "无显式 gate 行的元素经插入获得该行: {auth_source}"
    );
}

/// 边界：目标目录只读（`cfg(unix)` 0o555）→ `Err` 且原文件不变、无临时文件残留。
#[cfg(unix)]
#[test]
fn write_atomic只读目录失败且无残留() {
    use std::os::unix::fs::PermissionsExt;

    let temp = healthy("write-atomic");
    let models = archi_models_dir(temp.path());
    let path = models.join("01-core.toml");
    let before = fs::read(&path).expect("读模型文件");

    fs::set_permissions(&models, fs::Permissions::from_mode(0o555)).expect("置只读");
    let result = archi_set_gate(temp.root(), "App.Auth".to_owned(), GateTier::Strict);
    fs::set_permissions(&models, fs::Permissions::from_mode(0o755)).expect("恢复权限");

    if result.is_ok() {
        // 以 root 运行时可写穿只读目录：此时只断言「无残留 + 文件可读」，不伪造失败
        let leftovers: Vec<String> = fs::read_dir(&models)
            .expect("读模型目录")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("archi-tmp"))
            .collect();
        assert!(leftovers.is_empty(), "无临时文件残留: {leftovers:?}");
        assert!(fs::read(&path).is_ok(), "目标文件仍可读");
        return;
    }
    assert_eq!(fs::read(&path).expect("读模型文件"), before, "原文件不变");
    let leftovers: Vec<String> = fs::read_dir(&models)
        .expect("读模型目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("archi-tmp"))
        .collect();
    assert!(leftovers.is_empty(), "无临时文件残留: {leftovers:?}");
}

// ---------------------------------------------------------------------------
// `archi_change_summary`：store 记录解析
// ---------------------------------------------------------------------------

/// 正向：worktree 在场 → 以 worktree 为改动侧做 diff 摘要。
#[test]
fn change_summary_worktree在场() {
    let data = TempDir::new("summary-wt-data");
    let main = TempDir::new("summary-wt-main");
    let next = TempDir::new("summary-wt-next");
    main.write_model("01.toml", "[element.\"App\"]\nkind = \"package\"\n");
    next.write_model(
        "01.toml",
        "[element.\"App\"]\nkind = \"package\"\ngate = \"strict\"\n",
    );
    let app = app_with(data.path());
    let store = store_of(&app, &main.root());
    seed_record(&store, "wt-change", ChangeStatus::Active, Some(next.root()));

    let summary = archi_change_summary(
        app.state::<WorkspaceStores>(),
        main.root(),
        "wt-change".to_owned(),
    )
    .expect("Ok")
    .expect("有改动即 Some");
    assert_eq!(summary.gate_changes.len(), 1, "档位变更一条");
    assert_eq!(summary.gate_changes[0].fqn, "App");
    assert_eq!(summary.gate_changes[0].from, GateTier::Off);
    assert_eq!(summary.gate_changes[0].to, GateTier::Strict);
}

/// 边界：归档 change（`worktree = None`）→ 回落主仓根（两侧同一 → `None`）；
/// 两侧模型目录俱缺席 → `None`。
#[test]
fn change_summary归档回落与双侧缺席() {
    let data = TempDir::new("summary-archive-data");
    let main = TempDir::new("summary-archive-main");
    main.write_model("01.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let app = app_with(data.path());
    let store = store_of(&app, &main.root());
    seed_record(&store, "archived", ChangeStatus::Archived, None);

    assert!(
        archi_change_summary(
            app.state::<WorkspaceStores>(),
            main.root(),
            "archived".to_owned()
        )
        .expect("Ok")
        .is_none(),
        "归档回落主仓根 → 两侧等价 → None（零占位）"
    );

    let bare = TempDir::new("summary-bare");
    let bare_data = TempDir::new("summary-bare-data");
    let bare_app = app_with(bare_data.path());
    let bare_store = store_of(&bare_app, &bare.root());
    seed_record(&bare_store, "no-models", ChangeStatus::Archived, None);
    assert!(
        archi_change_summary(
            bare_app.state::<WorkspaceStores>(),
            bare.root(),
            "no-models".to_owned()
        )
        .expect("Ok")
        .is_none(),
        "两侧模型目录俱缺席 → None"
    );
}

/// 异常：`change_id` 在 store 不可达 → `Err` 附 id；worktree 指向不存在路径 →
/// 「改动侧全为删除」的摘要（不 panic、不静默）。
#[test]
fn change_summary异常两形态() {
    let data = TempDir::new("summary-err-data");
    let main = TempDir::new("summary-err-main");
    main.write_model("01.toml", "[element.\"App\"]\nkind = \"package\"\n");
    let app = app_with(data.path());

    let error = archi_change_summary(
        app.state::<WorkspaceStores>(),
        main.root(),
        "no-such-change".to_owned(),
    )
    .expect_err("未建档应 Err");
    assert!(
        error.contains("no-such-change"),
        "错误文案附 change id: {error}"
    );

    let store = store_of(&app, &main.root());
    let missing_worktree = main.path().join("no-such-worktree");
    seed_record(
        &store,
        "missing-wt",
        ChangeStatus::Active,
        Some(missing_worktree.to_string_lossy().into_owned()),
    );
    let summary = archi_change_summary(
        app.state::<WorkspaceStores>(),
        main.root(),
        "missing-wt".to_owned(),
    )
    .expect("Ok")
    .expect("主仓有模型即 Some");
    assert_eq!(
        summary.elements_removed,
        vec!["App".to_owned()],
        "worktree 缺席 → 改动侧模型为空 → 元素全为删除"
    );
}

/// 边界：`_inner` 在 `worktree = None` 时以主仓根为改动侧（两侧等价 → `None`）。
#[test]
fn change_summary_inner回落主仓根() {
    let main = TempDir::new("summary-inner-main");
    main.write_model("01.toml", "[element.\"App\"]\nkind = \"package\"\n");
    assert!(
        archi_change_summary_inner(main.path(), None)
            .expect("Ok")
            .is_none(),
        "worktree 缺席 → 主仓根自身 → 零改动"
    );
    // root 非法 → Err
    assert!(archi_change_summary_inner(Path::new(""), None).is_err());
}

// ---------------------------------------------------------------------------
// archi_drift：两模式 / 变更集缝 / 错误面 / 与门禁 runner 同源（AC-7 / AC-10 / AC-15）
// ---------------------------------------------------------------------------

/// 假 vcs 缝：`change_edit_set` 可编程（清单 / `Err`），并捕获入参；其余方法只
/// 在命令面消费别的缝时才可达，故以 `panic!` 起「零意外调用」告警。
struct FakeVcs {
    outcome: Result<Vec<String>, String>,
    calls: std::sync::Mutex<Vec<(String, String)>>,
}

impl FakeVcs {
    fn with(changed: &[&str]) -> Self {
        FakeVcs {
            outcome: Ok(changed.iter().map(|item| (*item).to_owned()).collect()),
            calls: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn failing(message: &str) -> Self {
        FakeVcs {
            outcome: Err(message.to_owned()),
            calls: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().expect("调用记录锁").clone()
    }
}

impl orchestration::port::ArchiveVcsPort for FakeVcs {
    fn dirty(&self, _root: &Path, _paths: &[&str]) -> bool {
        panic!("命令面不应调用 dirty")
    }

    fn commit_all(&self, _worktree: &Path, _message: &str) -> Result<(), String> {
        panic!("命令面不应调用 commit_all")
    }

    fn branch_merged(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
        panic!("命令面不应调用 branch_merged")
    }

    fn rebase_branch(
        &self,
        _worktree: &Path,
        _onto: &str,
    ) -> Result<orchestration::port::RebaseOutcome, String> {
        panic!("命令面不应调用 rebase_branch")
    }

    fn ff_merge(&self, _main_root: &Path, _branch: &str) -> Result<(), String> {
        panic!("命令面不应调用 ff_merge")
    }

    fn worktree_snapshot(
        &self,
        _root: &Path,
    ) -> Result<orchestration::port::WorktreeSnapshot, String> {
        panic!("命令面不应调用 worktree_snapshot")
    }

    fn rebase_continue(
        &self,
        _worktree: &Path,
    ) -> Result<orchestration::port::RebaseOutcome, String> {
        panic!("命令面不应调用 rebase_continue")
    }

    fn rebase_abort(&self, _worktree: &Path) -> Result<(), String> {
        panic!("命令面不应调用 rebase_abort")
    }

    fn current_branch(&self, _main_root: &Path) -> Result<String, String> {
        panic!("命令面不应调用 current_branch")
    }

    fn commit_paths(
        &self,
        _main_root: &Path,
        _paths: &[&str],
        _message: &str,
    ) -> Result<(), String> {
        panic!("命令面不应调用 commit_paths")
    }

    fn change_edit_set(&self, root: &Path, base: &str) -> Result<Vec<String>, String> {
        self.calls
            .lock()
            .expect("调用记录锁")
            .push((root.to_string_lossy().into_owned(), base.to_owned()));
        self.outcome.clone()
    }

    fn change_edit_patch(&self, _root: &Path, _base: &str) -> Result<String, String> {
        panic!("命令面不应调用 change_edit_patch")
    }

    fn remove_worktree(&self, _main_root: &Path, _worktree: &Path) -> Result<(), String> {
        panic!("命令面不应调用 remove_worktree")
    }

    fn restore_baseline(&self, _worktree: &Path, _exclusions: &[&str]) -> Result<String, String> {
        panic!("命令面不应调用 restore_baseline")
    }
}

/// 判定语料 fixture：两元素（strict 引入方 / off 被引入方）+ 一条跨元素 import。
fn drift_fixture(tag: &str) -> TempDir {
    let temp = TempDir::new(tag);
    temp.write_model(
        "01-drift.toml",
        concat!(
            "[element.\"Auth\"]\nkind = \"module\"\ngate = \"off\"\npath = [\"./src/auth/\"]\n\n",
            "[element.\"Billing\"]\nkind = \"module\"\ngate = \"strict\"\npath = [\"./src/billing/\"]\n",
        ),
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "src/billing/x.ts",
        "import { token } from '../auth/token';\n",
    );
    temp
}

/// 正向（AC-7）：全量模式——`mode = full` / `base = None` / `change = None` /
/// `changed_files = 0`，且 vcs 缝零调用（全量不取变更集）。
#[test]
#[allow(non_snake_case)]
fn archi_drift全量模式零vcs调用() {
    let temp = drift_fixture("drift-full");
    let vcs = FakeVcs::with(&["src/billing/x.ts"]);
    let report = archi_drift_inner(temp.path(), ArchiDriftMode::Full, &vcs).expect("全量应成功");

    assert_eq!(report.mode, ArchiDriftMode::Full, "模式逐字");
    assert_eq!(report.base, None, "全量无基准");
    assert_eq!(report.change, None, "命令面无 change 上下文");
    assert_eq!(report.scan.changed_files, 0, "全量忽略变更集");
    assert!(vcs.calls().is_empty(), "全量 MUST NOT 取变更集");
    assert_eq!(report.errors.len(), 1, "strict 违例照报");
}

/// 正向（AC-15）：增量模式——`base = Some("HEAD")`，变更集经 vcs 缝注入且捕获
/// `(root, "HEAD")` 一次。
#[test]
#[allow(non_snake_case)]
fn archi_drift增量模式经vcs缝取变更集() {
    let temp = drift_fixture("drift-incremental");
    let cases: [Vec<&str>; 3] = [
        vec![],
        vec!["src/billing/x.ts"],
        vec!["src/billing/x.ts", "src/auth/token.ts"],
    ];
    for changed in cases {
        let vcs = FakeVcs::with(&changed);
        let report =
            archi_drift_inner(temp.path(), ArchiDriftMode::Incremental, &vcs).expect("增量应成功");
        assert_eq!(report.mode, ArchiDriftMode::Incremental, "模式逐字");
        assert_eq!(report.base.as_deref(), Some("HEAD"), "增量基准恒 HEAD");
        assert_eq!(
            report.scan.changed_files as usize,
            changed.len(),
            "changed_files 反映注入清单"
        );
        assert_eq!(
            vcs.calls(),
            vec![(temp.root(), "HEAD".to_owned())],
            "变更集唯一入口捕获 (root, HEAD) 恰一次"
        );
    }
}

/// 异常（AC-15）：变更集取数失败即 `Err`（不降级空清单、零报告）。
#[test]
#[allow(non_snake_case)]
fn archi_drift变更集失败即Err() {
    let temp = drift_fixture("drift-vcs-error");
    let vcs = FakeVcs::failing("git 不可用");
    let error = archi_drift_inner(temp.path(), ArchiDriftMode::Incremental, &vcs)
        .expect_err("取数失败必须 Err");
    assert_eq!(error, "git 不可用", "错误面原样透传");
}

/// 异常：root 校验四形态（空 / 纯空白 / 不存在 / 普通文件）→ `Err`，零 vcs 零判定。
#[test]
#[allow(non_snake_case)]
fn archi_drift_root校验() {
    let temp = TempDir::new("drift-root");
    let file = temp.write("plain.txt", "not a directory\n");
    let missing = temp.path().join("no-such-root");
    let vcs = FakeVcs::with(&[]);
    for root in [PathBuf::new(), PathBuf::from("   "), missing, file] {
        let error =
            archi_drift_inner(&root, ArchiDriftMode::Full, &vcs).expect_err("无效 root 必须 Err");
        assert!(!error.is_empty(), "错误文案非空（root {root:?}）");
    }
    assert!(vcs.calls().is_empty(), "root 校验前置 ⇒ 零 vcs 调用");
}

/// 边界（AC-8）：无 `<archi_dir>/models` → `Ok(skipped)`（命令层不把「没检查」
/// 映射成 `Err`）。
#[test]
#[allow(non_snake_case)]
fn archi_drift无模型skipped() {
    let temp = TempDir::new("drift-skipped");
    temp.write("src/a.ts", "export const a = 1;\n");
    let vcs = FakeVcs::with(&["src/a.ts"]);
    let report = archi_drift_inner(temp.path(), ArchiDriftMode::Incremental, &vcs).expect("不 Err");
    assert_eq!(
        report.status,
        archi::drift::ArchiCheckStatus::Skipped,
        "无模型 ⇒ skipped"
    );
}

/// 异常（AC-8 / AC-12）：坏模型（`cycle`）→ `Ok(report)` 且结构错非空、
/// `status = errors`（命令层不把结构错映射成 `Err`）。
#[test]
#[allow(non_snake_case)]
fn archi_drift坏模型结构化错误不panic() {
    let temp = TempDir::new("drift-structural");
    temp.write_model(
        "01-cycle.toml",
        concat!(
            "[element.\"A\"]\nkind = \"module\"\n\n",
            "[element.\"B\"]\nkind = \"module\"\n\n",
            "[[relation]]\nfrom = \"A\"\nto = \"B\"\n\n",
            "[[relation]]\nfrom = \"B\"\nto = \"A\"\n",
        ),
    );
    temp.write("src/a.ts", "export const a = 1;\n");
    let vcs = FakeVcs::with(&["src/a.ts"]);
    let report = archi_drift_inner(temp.path(), ArchiDriftMode::Incremental, &vcs).expect("不 Err");
    assert!(
        !report.model.structural_errors.is_empty(),
        "结构错进报告 model 段"
    );
    assert_eq!(
        report.status,
        archi::drift::ArchiCheckStatus::Errors,
        "结构错令 status = errors"
    );
}

/// 异常：看护配置非法 → `Err`，文案含 serde 原文（`CheckError::message` 透传）。
#[test]
#[allow(non_snake_case)]
fn archi_drift配置非法Err() {
    let temp = drift_fixture("drift-bad-config");
    let config = foundation::layout::archi_config_path(temp.path());
    fs::create_dir_all(config.parent().expect("父目录")).expect("建架构域目录失败");
    fs::write(&config, "{ \"rules\": [] }").expect("写配置失败");
    let vcs = FakeVcs::with(&["src/billing/x.ts"]);
    let error = archi_drift_inner(temp.path(), ArchiDriftMode::Incremental, &vcs)
        .expect_err("配置非法必须 Err");
    assert!(
        error.contains("unknown field"),
        "文案含 serde 原文：{error}"
    );
}

/// 边界（AC-7）：全量调用前后 tempdir 字节快照相等（命令面零写盘、`reports/`
/// 零创建）。
#[test]
#[allow(non_snake_case)]
fn archi_drift全量零写盘() {
    let temp = drift_fixture("drift-no-write");
    let vcs = FakeVcs::with(&[]);
    let before = snapshot(temp.path());
    let _ = archi_drift_inner(temp.path(), ArchiDriftMode::Full, &vcs).expect("全量应成功");
    let after = snapshot(temp.path());
    assert_eq!(before, after, "全量判定零写盘");
    assert!(
        !temp.path().join("openspec").exists(),
        "命令面不创建报告目录"
    );
}

/// 最小 noop-waker 执行器（不引 tokio 运行时：被测 future 全同步推进）。
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn noop(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// 正向（AC-10）：命令面报告与门禁 runner 落盘报告逐字段相等——除 `timestamp` /
/// `change` 外全等（判定单源）。
#[test]
#[allow(non_snake_case)]
fn archi_drift与门禁runner同源() {
    use orchestration::port::{ArchiCheckRunner, ToolStepOutput};

    let temp = drift_fixture("drift-same-source");
    let vcs = FakeVcs::with(&["src/billing/x.ts"]);
    let by_command =
        archi_drift_inner(temp.path(), ArchiDriftMode::Incremental, &vcs).expect("命令面应成功");

    let runner = archi_runtime::LocalArchiCheck::new();
    let future = runner.run(
        &temp.root(),
        "demo",
        vec!["src/billing/x.ts".to_owned()],
        foundation::cancel::CancelFlag::never(),
    );
    let output = block_on(future).expect("runner 不 Err");
    let outcome = match output {
        ToolStepOutput::ArchiCheck(outcome) => outcome,
        other => panic!("产出类型漂移：{other:?}"),
    };
    let reference = outcome.report_ref.expect("runner 应回填报告路径");
    let mut by_runner: archi::drift::ArchiDriftReport =
        serde_json::from_slice(&fs::read(&reference).expect("读报告失败")).expect("反序列化失败");

    assert_eq!(
        by_runner.change.as_deref(),
        Some("demo"),
        "门禁面携 change 名"
    );
    by_runner.timestamp = by_command.timestamp.clone();
    by_runner.change = by_command.change.clone();
    assert_eq!(
        by_runner, by_command,
        "除 timestamp / change 外逐字段相等（判定单源）"
    );
}

/// 边界（AC-10）：命令层零判定——`mod.rs` 源码不含归属 / 定级 / `status` 构造
/// 实现，只做「参数 → 调用 → 错误映射」。
#[test]
#[allow(non_snake_case)]
fn archi_drift命令层零判定() {
    let source = include_str!("mod.rs");
    for forbidden in [
        "owner_of",
        "OwnerIndex",
        "ExcludeSet",
        "ArchiCheckStatus::",
        "driftMarks",
    ] {
        assert!(
            !source.contains(forbidden),
            "命令层 MUST NOT 含判定实现 {forbidden}"
        );
    }
    assert!(
        source.contains("archi::drift::check("),
        "判定必须经 archi::drift::check 单点"
    );
}

/// 边界：root 含空格与非 ASCII → `Ok` 且报告 `root` 字段逐字回填入参（不归一）。
#[test]
#[allow(non_snake_case)]
fn archi_drift根路径边界逐字回填() {
    let temp = drift_fixture("drift 根 空格 与中文");
    let vcs = FakeVcs::with(&[]);
    let report = archi_drift_inner(temp.path(), ArchiDriftMode::Full, &vcs).expect("应成功");
    assert_eq!(
        report.root,
        temp.root(),
        "报告 root 字段为入参逐字（不归一）"
    );
}

// ---------------------------------------------------------------------------
// 包装 ↔ inner：`archi_drift`（新增命令；包装在体内装配真实 `ProcessArchiveVcs`）
// ---------------------------------------------------------------------------

/// 正向（AC-7 / AC-10）：`archi_drift` 包装与 `archi_drift_inner` 逐字段一致——
/// 包装只在体内构造真实 `ProcessArchiveVcs`（全量模式不取变更集 ⇒ 进程边零触达，
/// 结果确定性可对拍），`timestamp` 为 wall-clock，对拍前归一为同一值。
#[test]
#[allow(non_snake_case)]
fn archi_drift包装与inner一致() {
    let temp = drift_fixture("drift-wrapper");
    let outer = archi_drift(temp.root(), ArchiDriftMode::Full).expect("包装应 Ok");
    let inner = archi_drift_inner(temp.path(), ArchiDriftMode::Full, &FakeVcs::with(&[]))
        .expect("inner 应 Ok");

    assert_eq!(outer.mode, ArchiDriftMode::Full, "模式逐字");
    assert_eq!(outer.base, None, "全量无基准");
    assert_eq!(
        outer.change, None,
        "命令面无 change 上下文（包装不补 change）"
    );
    assert_eq!(outer.scan.changed_files, 0, "全量不取变更集");
    assert_eq!(outer.errors.len(), 1, "strict 违例照报（包装零加工）");
    assert_eq!(outer.status, inner.status, "status 逐字一致");
    assert_eq!(outer.root, inner.root, "root 逐字回填一致");

    let mut outer_value = serde_json::to_value(&outer).expect("序列化");
    let inner_value = serde_json::to_value(&inner).expect("序列化");
    outer_value["timestamp"] = inner_value["timestamp"].clone();
    assert_eq!(
        outer_value, inner_value,
        "包装与 _inner 除 timestamp 外逐字段一致"
    );
}

/// 异常：包装侧的 root 校验前置——空 root / 纯空白 / 普通文件 / 不存在目录一律
/// `Err`（`ProcessArchiveVcs` 已构造但判定与取数均不触达），且零写盘。
#[test]
#[allow(non_snake_case)]
fn archi_drift包装root校验前置() {
    let temp = TempDir::new("drift-wrapper-root");
    let file = temp.write("plain.txt", "not a directory\n");
    let missing = temp.path().join("no-such-root");
    let before = snapshot(temp.path());
    for root in [PathBuf::new(), PathBuf::from("   "), missing, file] {
        let error = archi_drift(root.to_string_lossy().into_owned(), ArchiDriftMode::Full)
            .expect_err("无效 root 必须 Err");
        assert!(!error.is_empty(), "错误文案非空（root {root:?}）");
    }
    assert_eq!(before, snapshot(temp.path()), "包装侧 root 校验失败零写盘");
}

// ===========================================================================
// `archi_module` 出参的 `api` 并列字段（`module-api-contract-format` / AC-10 / AC-13）
//
// 命令层仍是薄包装：`api` 由 `dto::module_detail` 填充，本层零特判、零路径拼装。
// ===========================================================================

/// 落一份元素契约 md（`<archi_api_dir>/<Domain>/<FQN>.md`）。
fn write_contract(root: &Path, fqn: &str, content: &str) {
    let relative = archi::api::paths::contract_path(root, fqn).expect("合法 FQN");
    let relative = relative
        .strip_prefix(root)
        .expect("repo 相对路径")
        .to_string_lossy()
        .replace('\\', "/");
    let path = root.join(&relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("建契约目录失败");
    }
    fs::write(&path, content).expect("写契约失败");
}

/// 正向（AC-10 / AC-13）：元素 + 契约 md 在案 → `archi_module_inner` 出参 `api.status`
/// 为 `declared`、三路径均 `Some` 且 repo 相对 POSIX；序列化文本含 `"api"` 并列键。
#[test]
fn archi_module出参含api并列字段() {
    let temp = healthy("module-api-field");
    // 追加一个带 `path` 的元素（三路径全 `Some` 的形态；`CORE` 内元素 path 为空）
    temp.write_model(
        "02-shop.toml",
        "[element.\"App.Shop\"]\nkind = \"module\"\npath = [\"./src/shop/\"]\n",
    );
    let fqn = "App.Shop";
    write_contract(temp.path(), fqn, &format!("# {fqn}\n\n## 职能\n\n正文。\n"));

    let detail = archi_module_inner(temp.path(), fqn).expect("archi_module_inner 应 Ok");
    assert_eq!(
        serde_json::to_value(&detail.api).expect("序列化成功")["status"],
        serde_json::json!("declared"),
        "md 在案 → status = declared"
    );
    let value = serde_json::to_value(&detail).expect("序列化成功");
    let api = &value["api"];
    for key in ["contractPath", "schemaPath", "generatedPath"] {
        let text = api[key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} 应为字符串：{api:?}"));
        assert!(!text.starts_with("./"), "{key} 零前导 `./`：{text}");
        assert!(!text.contains('\\'), "{key} 为 POSIX 分隔：{text}");
    }
    assert!(value.get("api").is_some(), "序列化文本含 api 并列键");
    // `api` 不在 `docs` 数组内（并列字段而非承重进 docs）
    for doc in value["docs"].as_array().expect("docs 是数组") {
        assert!(
            doc.get("status").is_none(),
            "契约投影 MUST NOT 进 docs 列表：{doc:?}"
        );
    }
}

/// 边界（AC-10）：元素存在但无 md → `status = none` 且三路径全 `null`（零配置、可枚举
/// 「有契约 / 无契约」）。
#[test]
fn archi_module无契约三路径null() {
    let temp = healthy("module-api-none");
    let detail = archi_module_inner(temp.path(), "App.Auth").expect("archi_module_inner 应 Ok");
    let value = serde_json::to_value(&detail).expect("序列化成功");
    assert_eq!(
        value["api"]["status"],
        serde_json::json!("none"),
        "无 md → status = none"
    );
    for key in ["contractPath", "schemaPath", "generatedPath"] {
        assert_eq!(value["api"][key], serde_json::json!(null), "{key} 为 null");
    }
}

/// 边界（AC-10）：容器元素（`path = []`）→ `status = None` 且三路径 `null`（天然无契约，
/// 命令层零特判）。
#[test]
fn archi_module容器元素零契约() {
    let temp = healthy("module-api-container");
    let detail = archi_module_inner(temp.path(), "App").expect("容器元素应 Ok");
    assert_eq!(
        detail.api.status,
        archi::dto::ApiStatus::None,
        "容器元素天然无契约"
    );
    assert!(detail.api.contract_path.is_none());
    assert!(detail.api.schema_path.is_none());
    assert!(detail.api.generated_path.is_none());
}

/// 边界（AC-1 / D12）：命令层零领域规则——两新命令只调 core 的单点出口（`artifact_check`
/// / `workspace_render` 各一处），零本地渲染 / 比对 / 归一 / 路径拼装；`fs::write` 与
/// `create_dir_all` 仍只属 `archi_set_gate` 的原子写回面。
#[test]
fn 命令层零领域规则() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("archi::api::artifact_check(").count(),
        1,
        "检查命令只调 `artifact_check`（DTO 投影单点在 core）"
    );
    assert_eq!(
        source.matches("archi::workspace_render(").count(),
        1,
        "生成命令只调 `workspace_render`（应生成集单点在 core）"
    );
    for forbidden in [
        "archi::api::compare(",
        "archi::api::meta(",
        "archi::api::display(",
        "archi::api::findings(",
    ] {
        assert!(
            !source.contains(forbidden),
            "命令层零第二份判定 / 归一 / 装配（`{forbidden}` 零命中）"
        );
    }
    assert!(
        !source.contains("contract_path") && !source.contains("schema_path"),
        "命令层零契约路径拼装"
    );
    assert!(
        source.contains("dto::module_detail"),
        "archi_module 经 dto::module_detail 投影"
    );
    // 写盘面：`fs::write` / `create_dir_all` 仍只属 `archi_set_gate` 的 `write_atomic`
    assert_eq!(
        source.matches("fs::write(&temporary").count(),
        1,
        "`fs::write` 恰一处（`write_atomic` 的临时文件写入）"
    );
    assert!(
        !source.contains("create_dir_all"),
        "生成命令经 `write_artifacts` 建目录（命令层零 `create_dir_all`）"
    );

    let commands: Vec<&str> = source
        .lines()
        .filter(|line| line.starts_with("pub fn archi_"))
        .collect();
    assert_eq!(
        commands.len(),
        16,
        "八条命令 与 八条 *_inner 各一件（实际：{commands:?}）"
    );
    assert_eq!(
        source.matches("#[tauri::command]").count(),
        8,
        "命令面八条（typed 包装各带一注解）"
    );
    assert_eq!(
        source.matches("#[specta::specta]").count(),
        8,
        "八条命令均走 specta 出线"
    );
}

/// 边界（AC-9）：只读命令零写盘——`archi_module` 调用前后 tempdir 字节快照相等（新增
/// `api` 字段只是一次 `exists()` 定位）。
#[test]
fn archi_module零写盘() {
    let temp = healthy("module-api-readonly");
    write_contract(temp.path(), "App.Auth", "# App.Auth\n\n## 职能\n\n正文。\n");
    let before = snapshot(temp.path());
    let _ = archi_module_inner(temp.path(), "App.Auth").expect("Ok");
    assert_eq!(snapshot(temp.path()), before, "只读命令零写盘");
}

/// 异常（既有口径零变化）：`root` 为空白 / 不存在 / 文件而非目录 → `Err`，且文案
/// **不含**契约面词汇（错误映射未被新字段污染）。
#[test]
fn archi_module_root异常文案零契约词汇() {
    let file_root = TempDir::new("module-api-file-root");
    let root_file = file_root.write("not-a-dir.txt", "x");
    let root_text = root_file.to_string_lossy().into_owned();
    for root in ["", "   ", "/definitely/does/not/exist", root_text.as_str()] {
        let error = archi_module_inner(Path::new(root), "App.Auth").expect_err("非法 root 应 Err");
        for forbidden in ["declared", "contractPath", "schemaPath", "$defs"] {
            assert!(
                !error.contains(forbidden),
                "错误文案不含契约面词汇 `{forbidden}`：{error}"
            );
        }
    }
}

/// 正向（AC-13，两命令交叉）：试点形态（元素 + 入口 + 契约 md，且签名与代码一致）下
/// `archi_module` 报 `declared`，同输入 `archi_drift` 的 `api_findings` 为空
/// （该元素十判据零命中）。
#[test]
fn archi_module试点declared且同输入判据零命中() {
    let temp = TempDir::new("module-api-pilot");
    let fqn = "App.Backend.Core.Archi";
    temp.write_model(
        "01-pilot.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\ngate = \"off\"\n\n",
            "[element.\"App.Backend\"]\nkind = \"domain\"\ngate = \"off\"\n\n",
            "[element.\"App.Backend.Core\"]\nkind = \"domain\"\ngate = \"off\"\n\n",
            "[element.\"App.Backend.Core.Archi\"]\nkind = \"module\"\ngate = \"off\"\n",
            "path = [\"crates/core/archi/\"]\n",
        ),
    );
    // 真实试点形态（D44 的两处取数面结构前提）：`file_stem` 的实现在生成物
    // `crates/core/archi.i.rs` 内，入口经 `#[path = "../../archi.i.rs"] mod api_gen;` +
    // `pub use api_gen::*;` 让它进符号集并升为 `External`——缺任一条则 `$defs` 的
    // `LocalArchiCheck` 与 op `file_stem` 都会报 `apiMissing`（基线由 13 变 14）
    temp.write(
        "crates/core/archi/src/lib.rs",
        "#[path = \"../../archi.i.rs\"]\nmod api_gen;\npub use api_gen::*;\n",
    );
    temp.write(
        "crates/core/archi.i.rs",
        "pub struct LocalArchiCheck;\n\nimpl LocalArchiCheck {\n    pub fn file_stem(timestamp: &str) -> String {\n        timestamp.to_owned()\n    }\n}\n",
    );
    write_contract(
        temp.path(),
        fqn,
        &format!(
            "# {fqn}\n\n## 职能\n\n门禁执行体。\n\n## 操作\n\n### file_stem\n\n```json api-op\n\
             {{\"params\": [{{\"name\": \"timestamp\", \"schema\": {{\"type\": \"string\"}}}}], \
             \"returns\": {{\"type\": \"string\"}}}}\n```\n\n## 类型\n\n```json api-types\n\
             {{\"LocalArchiCheck\": {{\"type\": \"object\", \"properties\": {{}}}}}}\n```\n"
        ),
    );

    let detail = archi_module_inner(temp.path(), fqn).expect("archi_module_inner 应 Ok");
    assert_eq!(
        detail.api.status,
        archi::dto::ApiStatus::Declared,
        "试点 declared"
    );
    assert!(detail.api.contract_path.is_some());
    assert!(detail.api.schema_path.is_some());
    assert!(
        detail.api.generated_path.is_some(),
        "有 path 即有生成物路径"
    );

    let vcs = FakeVcs::with(&[]);
    let report = archi_drift_inner(temp.path(), ArchiDriftMode::Full, &vcs).expect("全量应成功");
    let api_findings: Vec<&archi::drift::report::ArchiApiFinding> = report
        .api_findings
        .iter()
        .filter(|finding| finding.element.as_deref() == Some(fqn))
        .collect();
    assert!(
        api_findings.is_empty(),
        "试点元素十判据零命中：{:?}",
        api_findings
            .iter()
            .map(|finding| (finding.kind.as_str(), finding.message.as_str()))
            .collect::<Vec<_>>()
    );
}

/// 边界（AC-16 否定断言）：模块**文档关联集**不含 `<archi_api_dir>/**`——契约正
/// 文不经 `docs` 三源出线（契约权威在架构域 api 目录；正文由通用 `read_file`
/// 按路径自取，不再是架构命令的专属通路）。
#[test]
fn 文档关联集不含契约目录() {
    let temp = healthy("read-doc-no-api");
    let fqn = "App.Auth";
    write_contract(
        temp.path(),
        fqn,
        &format!("# {fqn}\n\n## 职能\n\n契约正文。\n"),
    );
    let contract_relative = archi::api::paths::contract_path(temp.path(), fqn).expect("合法 FQN");
    let contract_relative = contract_relative
        .strip_prefix(temp.path())
        .expect("repo 相对路径")
        .to_string_lossy()
        .replace('\\', "/");
    assert!(
        contract_relative.contains("api/"),
        "夹具确实落在契约目录：{contract_relative}"
    );

    // 关联集本身不含契约正文
    let linked = archi::dto::linked_doc_paths(temp.path()).expect("Ok");
    assert!(
        linked.iter().all(|path| !path.contains("api/")),
        "关联集零契约正文通路：{linked:?}"
    );
    assert!(
        !linked.contains(&contract_relative),
        "契约路径不在关联集内：{contract_relative}"
    );
}

// ---------------------------------------------------------------------------
// 文件树命令（archi_file_tree）：只读投影的第六命令
// ---------------------------------------------------------------------------

/// 健康模型 + 若干源文件的 tempdir（文件树用例的通用语料）。
fn tree_fixture(tag: &str) -> TempDir {
    let temp = TempDir::new(tag);
    temp.write_model(
        "01-tree.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Core\"]\nkind = \"domain\"\n\n",
            "[element.\"App.Core.Archi\"]\nkind = \"module\"\n",
            "path = [\"./crates/core/archi/\"]\n",
        ),
    );
    temp.write("crates/core/archi/src/tree.rs", "pub fn a() {}\n");
    temp.write("crates/core/archi/src/scan/owner.rs", "pub fn b() {}\n");
    temp.write("src/loose.ts", "export const loose = 1;\n");
    temp
}

/// 正向（AC-1 / AC-3）：包装与 `_inner` 各自 `Ok` 且经 `serde_json` 逐字段相等
/// （命令层零加工）。
#[test]
#[allow(non_snake_case)]
fn archi_file_tree包装与inner逐字段一致() {
    let temp = tree_fixture("file-tree-wrapper");

    let outer = archi_file_tree(temp.root()).expect("包装应 Ok");
    let inner = archi_file_tree_inner(temp.path()).expect("inner 应 Ok");
    assert_eq!(
        serde_json::to_value(&outer).expect("序列化"),
        serde_json::to_value(&inner).expect("序列化"),
        "archi_file_tree 与 _inner 逐字段一致"
    );
    assert!(!outer.entries.is_empty(), "健康语料应产出条目");
    // 命令层零加工：元素根节点的归属与后端投影逐字相同
    let root_entry = outer
        .entries
        .iter()
        .find(|entry| entry.path == "crates/core/archi")
        .expect("元素根目录条目");
    assert_eq!(
        root_entry.mark,
        archi::FileTreeMark::ElementRoot,
        "归属标记来自后端（命令层零推导）"
    );
    assert_eq!(root_entry.owners, vec!["App.Core.Archi".to_owned()]);
}

/// 异常（AC-1 / spec「空白 / 缺失 / 非目录 → Err」）：root 四形态 × 两入口一律
/// `Err`，文案沿用既有 `check_root` 口径。
#[test]
#[allow(non_snake_case)]
fn archi_file_tree_root校验四形态一律err() {
    let temp = TempDir::new("file-tree-root-check");
    let file_root = temp.write("not-a-dir.txt", "我是文件\n");
    let missing = temp.path().join("no-such-dir");
    let cases: [PathBuf; 4] = [
        PathBuf::from(""),
        PathBuf::from("   "),
        file_root.clone(),
        missing.clone(),
    ];
    for root in &cases {
        let rendering = root.to_string_lossy().into_owned();
        let outer = archi_file_tree(rendering.clone()).expect_err("包装 root 非法应 Err");
        assert!(
            outer.contains("root 无效") || outer.contains("root 不是目录"),
            "包装文案沿用 check_root 口径：{outer}"
        );
        let inner = archi_file_tree_inner(root).expect_err("inner root 非法应 Err");
        assert!(
            inner.contains("root 无效") || inner.contains("root 不是目录"),
            "inner 文案沿用 check_root 口径：{inner}"
        );
    }
}

/// 异常（AC-1 / spec「唯一写命令仍是 archi_set_gate」）：新命令体内零
/// `plan_set_gate` / 零 `write_atomic`；同盘调写命令才改变模型字节。
#[test]
#[allow(non_snake_case)]
fn archi_file_tree与唯一写命令的边界() {
    let temp = tree_fixture("file-tree-write-boundary");
    let before = snapshot(temp.path());
    let _ = archi_file_tree(temp.root()).expect("读命令 Ok");
    assert_eq!(snapshot(temp.path()), before, "读命令不改模型字节");

    archi_set_gate(temp.root(), "App.Core.Archi".to_owned(), GateTier::Strict).expect("写命令 Ok");
    assert_ne!(
        snapshot(temp.path()),
        before,
        "对照臂：唯一写命令改变模型字节"
    );

    let source = include_str!("mod.rs");
    let inner_start = source
        .find("pub fn archi_file_tree_inner")
        .expect("新命令在案");
    let body = &source[inner_start..];
    for forbidden in ["plan_set_gate", "write_atomic"] {
        assert!(
            !body.contains(forbidden),
            "`archi_file_tree_inner` 体内零 {forbidden}（只读命令）"
        );
    }
}

/// 异常 / 正向（AC-2 / spec Scenario「结构诊断非空即 Err」）：坏模型 → `Err`，文案含
/// 「模型结构错」与诊断条数；同盘换健康模型 → `Ok`。
#[test]
#[allow(non_snake_case)]
fn archi_file_tree结构诊断非空即err() {
    let broken = TempDir::new("file-tree-broken");
    broken.write_model(
        "01-cycle.toml",
        concat!(
            "[element.\"A\"]\nkind = \"module\"\n\n",
            "[element.\"B\"]\nkind = \"module\"\n\n",
            "[[relation]]\nfrom = \"A\"\nto = \"B\"\n\n",
            "[[relation]]\nfrom = \"B\"\nto = \"A\"\n",
        ),
    );
    broken.write("src/a.ts", "export const a = 1;\n");

    let error = archi_file_tree(broken.root()).expect_err("结构诊断非空应 Err");
    assert!(error.contains("模型结构错"), "文案形：{error}");
    assert!(error.contains("共 1 条"), "文案含诊断条数：{error}");

    let healthy = tree_fixture("file-tree-healthy");
    assert!(
        archi_file_tree(healthy.root()).is_ok(),
        "对照臂：同形态健康模型 Ok"
    );
}

/// 边界（AC-2 / spec Scenario「空模型返回全 none」）：模型目录缺席与零 `.toml`
/// 两形态均 `Ok`，全部条目 `mark = none` 且 `owners` 空。
#[test]
#[allow(non_snake_case)]
fn archi_file_tree空模型返回ok且全无主() {
    for tag in ["file-tree-models-absent", "file-tree-models-empty"] {
        let temp = TempDir::new(tag);
        if tag.ends_with("empty") {
            fs::create_dir_all(archi_models_dir(temp.path())).expect("建空模型目录");
        }
        temp.write("src/a.ts", "export const a = 1;\n");

        let tree = archi_file_tree(temp.root()).expect("空模型应 Ok（冷启动合法起点）");
        assert!(!tree.entries.is_empty(), "条目仍由磁盘枚举得出");
        for entry in &tree.entries {
            assert_eq!(
                entry.mark,
                archi::FileTreeMark::None,
                "{tag} 条目 {:?} 无主",
                entry.path
            );
            assert!(entry.owners.is_empty(), "{tag} owners 为空");
        }
    }
}

/// 异常 / 正向（AC-9 / AC-8）：配置非法 → `Err` 且文案沿用既有显式口径；合法配置
/// `Ok` 且排除生效。
#[test]
#[allow(non_snake_case)]
fn archi_file_tree配置非法err且合法生效() {
    let invalid = tree_fixture("file-tree-bad-config");
    let config = foundation::layout::archi_config_path(invalid.path());
    fs::create_dir_all(config.parent().expect("父目录")).expect("建架构域目录");
    fs::write(&config, "{ \"exclude\": [\"src/legacy\"] }").expect("写配置");
    let error = archi_file_tree(invalid.root()).expect_err("非法配置必须 Err");
    assert!(
        error.contains("不是文件级 glob"),
        "文案点名非法条目：{error}"
    );
    assert!(error.contains("/**"), "文案提示 `/**` 形态：{error}");

    let valid = tree_fixture("file-tree-good-config");
    valid.write("openspec/changes/x/prototype.tsx", "export const p = 1;\n");
    let config = foundation::layout::archi_config_path(valid.path());
    fs::create_dir_all(config.parent().expect("父目录")).expect("建架构域目录");
    fs::write(&config, "{ \"exclude\": [\"openspec/**\"] }").expect("写配置");
    let tree = archi_file_tree(valid.root()).expect("合法配置应 Ok");
    assert!(
        !tree
            .entries
            .iter()
            .any(|entry| entry.path == "openspec" || entry.path.starts_with("openspec/")),
        "配置 exclude 命中项不进树"
    );
}

/// 正向（AC-2 / AC-8 / R-6）：同根同模型上两命令互证——诊断空 ⇔ 树 `Ok`，
/// 且带 `path` 的元素 FQN 在树的 `owners` 中出现、树中归属必是模型内元素（两投影
/// 同源，零第二份模型）。
#[test]
#[allow(non_snake_case)]
fn archi_file_tree与archi_graph同根一致() {
    let temp = tree_fixture("file-tree-graph-consistent");
    let graph = archi_graph(temp.root()).expect("图命令 Ok");
    assert!(graph.diagnostics.is_empty(), "健康语料零诊断");
    let tree = archi_file_tree(temp.root()).expect("树命令 Ok");

    let path_elements: Vec<&str> = graph
        .elements
        .iter()
        .filter(|element| !element.path.is_empty())
        .map(|element| element.fqn.as_str())
        .collect();
    assert!(!path_elements.is_empty(), "语料含带 path 的元素");
    for element in &path_elements {
        assert!(
            tree.entries
                .iter()
                .any(|entry| entry.owners.iter().any(|fqn| fqn == element)),
            "带 path 的元素 {element} 应在树的某个 owners 中出现"
        );
    }
    for entry in &tree.entries {
        for fqn in &entry.owners {
            assert!(
                graph.elements.iter().any(|element| &element.fqn == fqn),
                "树中归属 {fqn} 必须是模型内元素"
            );
        }
    }
}

/// 异常（AC-6 / D11 / AC-1）：命令层零领域规则——`archi_file_tree_inner` 体内零归属判定 /
/// 零目录合成 / 零标记推导；文件头口径为「八命令」与「写命令两条」（零残留旧口径）。
#[test]
#[allow(non_snake_case)]
fn archi_file_tree命令层零领域规则() {
    let source = include_str!("mod.rs");
    let inner_start = source
        .find("pub fn archi_file_tree_inner")
        .expect("新命令在案");
    let body = &source[inner_start..];
    for forbidden in [
        "OwnerIndex::build",
        "starts_with",
        "module_roots",
        "contains_count",
        "FileTreeMark::",
        "FileEntryKind::",
        "read_dir",
    ] {
        assert!(
            !body.contains(forbidden),
            "`archi_file_tree_inner` 体内零 {forbidden}（归属 / 合成 / 标记全在后端）"
        );
    }
    assert!(
        body.contains("archi::tree::file_tree("),
        "命令层只调 `archi::tree::file_tree`（薄包装）"
    );
    assert!(
        source.contains("ArchiFileTree"),
        "`use` 区含 `ArchiFileTree`（返回类型在册）"
    );
    for stale in ["五命令", "四命令", "六命令", "其余五条"] {
        assert!(
            !source.contains(stale),
            "文件头口径已改符实（零残留 `{stale}`）"
        );
    }
    let header: String = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .collect::<Vec<&str>>()
        .join("\n");
    assert!(header.contains("八命令"), "文件头口径为八命令");
    assert!(header.contains("写命令两条"), "文件头写明命令两条");
}

// ---------------------------------------------------------------------------
// 本变更：archi_api_check / archi_api_generate（第二条写命令）
// ---------------------------------------------------------------------------

/// 契约语料（1 op `file_stem` + 1 类型；H1 = `App.Api.Subject`）。
const API_CONTRACT: &str = "# App.Api.Subject\n\n## 职能\n\n命令面语料。\n\n## 操作\n\n\
     ### file_stem\n\n```json api-op\n{\"owner\": \"LocalGate\", \
     \"params\": [{\"name\": \"timestamp\", \"schema\": {\"type\": \"string\"}}], \
     \"returns\": {\"type\": \"string\"}}\n```\n\n## 类型\n\n```json api-types\n\
     {\"LocalGate\": {\"type\": \"object\", \"properties\": {}}}\n```\n";

/// 契约面语料根：模型（`App` 容器 + `App.Api` 域 + 叶元素）+ 入口源文 + 契约 md。
fn api_fixture(tag: &str) -> TempDir {
    let temp = TempDir::new(tag);
    temp.write_model(
        "01-api.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.Api\"]\nkind = \"domain\"\ngate = \"off\"\n\n",
            "[element.\"App.Api.Subject\"]\nkind = \"module\"\ngate = \"warn\"\n",
            "path = [\"./crates/api/subject/\"]\n",
        ),
    );
    temp.write(
        "crates/api/subject/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );
    temp.write(
        ".dev-team/architecture/api/Api/App.Api.Subject.md",
        API_CONTRACT,
    );
    temp
}

/// 正向（AC-1 / AC-3）：包装与 `_inner` 各自 `Ok`，两者序列化逐字段相等（命令层零投影）。
#[test]
fn api_check包装与inner逐字段一致() {
    let temp = api_fixture("api-check-parity");
    let outer = archi_api_check(temp.root()).expect("包装应 Ok");
    let inner = archi_api_check_inner(temp.path()).expect("inner 应 Ok");
    assert_eq!(
        serde_json::to_value(&outer).expect("序列化"),
        serde_json::to_value(&inner).expect("序列化"),
        "命令层零投影：包装与 inner 逐字段相等"
    );
    assert!(outer.enabled, "契约面启用");
    assert!(
        !outer.missing.is_empty() || !outer.mismatched.is_empty(),
        "未生成时应报缺口：{outer:?}"
    );
}

/// 正向（AC-1 / AC-2）：`check` 后 `generate` → 两列与 `_inner` 逐字段相等；复检三列全空、
/// `enabled` 保持；`written + unchanged` = 应生成份数；二次 generate 幂等。
#[test]
fn api_generate写后复检三列全空() {
    let temp = api_fixture("api-generate");
    let before = archi_api_check(temp.root()).expect("检查应 Ok");
    let expected_total = before.mismatched.len() + before.orphaned.len() + before.missing.len();
    assert!(expected_total > 0, "首轮应有差异：{before:?}");

    // 包装与 inner 各在一份等价语料上首轮驱动：写盘有副作用，同一棵树连调两次第二次
    // 必然全 `unchanged`，不能拿来做逐字段对拍
    let twin = api_fixture("api-generate-twin");
    let outer = archi_api_generate(temp.root()).expect("生成应 Ok");
    let inner = archi_api_generate_inner(twin.path()).expect("inner 应 Ok");
    assert_eq!(
        serde_json::to_value(&outer).expect("序列化"),
        serde_json::to_value(&inner).expect("序列化"),
        "包装与 inner 逐字段相等"
    );
    assert_eq!(
        outer.written.len(),
        expected_total,
        "written + unchanged = 应生成份数"
    );
    assert!(outer.unchanged.is_empty(), "首轮零 unchanged");

    let after = archi_api_check(temp.root()).expect("复检应 Ok");
    assert!(after.mismatched.is_empty(), "复检零不符");
    assert!(after.orphaned.is_empty(), "复检零孤儿");
    assert!(after.missing.is_empty(), "复检零缺失");
    assert!(after.enabled, "enabled 保持");

    let again = archi_api_generate(temp.root()).expect("二次生成应 Ok");
    assert!(again.written.is_empty(), "二次生成零写入");
    assert_eq!(
        again.unchanged.len(),
        expected_total,
        "二次生成全部 unchanged（幂等）"
    );
}

/// 异常（AC-2 / 硬线二）：盘上预置孤儿 → `generate` 后孤儿字节不变、复检仍报同一条。
#[test]
fn api_generate孤儿仍在盘() {
    let temp = api_fixture("api-generate-orphan");
    temp.write("crates/api/orphan.i.rs", "// 历史孤儿\n");
    let orphan = temp.path().join("crates/api/orphan.i.rs");
    let before = fs::read(&orphan).expect("读孤儿失败");

    archi_api_generate(temp.root()).expect("生成应 Ok");
    assert!(orphan.is_file(), "孤儿仍在盘");
    assert_eq!(
        fs::read(&orphan).expect("读孤儿失败"),
        before,
        "孤儿字节不变"
    );

    let report = archi_api_check(temp.root()).expect("复检应 Ok");
    assert_eq!(
        report.orphaned,
        vec!["crates/api/orphan.i.rs".to_owned()],
        "复检仍报同一条孤儿"
    );
}

/// 异常（AC-2 / D12 / D1）：结构错模型 → 两命令均 `Err`（文案含「模型结构错」）；两次调用
/// 前后快照相等（零写盘）。
#[test]
fn 结构错模型两命令均err且零写盘() {
    let temp = TempDir::new("api-structural-error");
    temp.write_model(
        "01-cycle.toml",
        concat!(
            "[element.\"App\"]\nkind = \"package\"\n\n",
            "[element.\"App.A\"]\nkind = \"module\"\n\n",
            "[element.\"App.B\"]\nkind = \"module\"\n\n",
            "[[relation]]\nfrom = \"App.A\"\nto = \"App.B\"\n\n",
            "[[relation]]\nfrom = \"App.B\"\nto = \"App.A\"\n",
        ),
    );
    let before = snapshot(temp.path());
    let check_error = archi_api_check(temp.root()).expect_err("检查应 Err");
    assert!(
        check_error.contains("模型结构错"),
        "检查文案点名结构诊断：{check_error}"
    );
    let generate_error = archi_api_generate(temp.root()).expect_err("生成应 Err");
    assert!(
        generate_error.contains("模型结构错"),
        "生成文案点名结构诊断：{generate_error}"
    );
    assert_eq!(snapshot(temp.path()), before, "两次调用前后零写盘");
}

/// 边界（AC-10 的数据侧）：无 `<archi_api_dir>` → `check` 仍 `Ok`、`enabled == false`、
/// `missing` 恰 `INDEX.toml` 一条；`generate` 仍可写该份。
#[test]
fn 未启用契约面check仍ok且generate可写() {
    let temp = TempDir::new("api-not-enabled");
    temp.write_model(
        "01-api.toml",
        "[element.\"App\"]\nkind = \"package\"\n\n\
         [element.\"App.Api\"]\nkind = \"domain\"\ngate = \"off\"\n\n\
         [element.\"App.Api.Subject\"]\nkind = \"module\"\ngate = \"warn\"\n\
         path = [\"./crates/api/subject/\"]\n",
    );
    temp.write(
        "crates/api/subject/src/lib.rs",
        "pub fn file_stem(timestamp: &str) -> String { String::new() }\n",
    );

    let checked = archi_api_check(temp.root()).expect("未启用仍 Ok");
    assert!(!checked.enabled, "enabled == false");
    assert_eq!(checked.missing.len(), 1, "missing 恰一条");
    assert!(
        checked.missing[0].ends_with("INDEX.toml"),
        "缺口是索引：{:?}",
        checked.missing
    );

    let generated = archi_api_generate(temp.root()).expect("生成应 Ok");
    assert_eq!(generated.written.len(), 1, "应生成集只此一份");
    assert!(
        generated.written[0].ends_with("INDEX.toml"),
        "写入的是索引：{:?}",
        generated.written
    );
}

/// 异常（AC-2 / spec「非原子如实」）：目标落点被**目录**占位 → `generate` 返回 `Err`，
/// 文案原样透传写盘单点的错误（零包装、零补默认值），不含「全部成功」类措辞。
#[test]
fn 写失败文案透传不称全或无() {
    let temp = api_fixture("api-write-failure");
    // `INDEX.toml` 落点被同名目录占位 ⇒ 该份 `fs::write` 必失败（前序份已写）
    fs::create_dir_all(temp.path().join(".dev-team/architecture/api/INDEX.toml"))
        .expect("建同名目录失败");

    let error = archi_api_generate(temp.root()).expect_err("写盘失败应 Err");
    assert!(error.contains("写入失败"), "文案透传写盘单点原因：{error}");
    assert!(
        !error.contains("全部成功"),
        "MUST NOT 声称全部成功：{error}"
    );
    assert!(
        temp.path().join("crates/api/subject.i.rs").is_file(),
        "非原子：失败点之前已写（不回滚）"
    );
}

/// 异常（AC-1 / D12）：`mod.rs` 自证命令层零领域规则——检查命令只调投影单点、生成命令 =
/// 渲染单点 + 写盘单点；两新函数体零本地写盘；文件头「八命令」「写命令两条」在位。
#[test]
fn 命令层零领域规则与头注释口径() {
    let source = include_str!("mod.rs");
    let body_of = |name: &str| -> String {
        source
            .split_once(&format!("pub fn {name}("))
            .unwrap_or_else(|| panic!("{name} 在案"))
            .1
            .split_once("\n}\n")
            .expect("函数体收口")
            .0
            .to_owned()
    };

    let check_inner = body_of("archi_api_check_inner");
    assert!(
        check_inner.contains("archi::api::artifact_check("),
        "检查命令调投影单点：\n{check_inner}"
    );
    assert!(
        !check_inner.contains("workspace_render") && !check_inner.contains("compare("),
        "检查命令零第二份渲染 / 比对：\n{check_inner}"
    );

    let generate_inner = body_of("archi_api_generate_inner");
    assert!(
        generate_inner.contains("archi::workspace_render(")
            && generate_inner.contains("api_artifacts::write_artifacts("),
        "生成命令 = 渲染单点 + 写盘单点：\n{generate_inner}"
    );
    assert!(
        !generate_inner.contains("fs::write") && !generate_inner.contains("create_dir_all"),
        "新命令零本地写盘：\n{generate_inner}"
    );

    let header: Vec<&str> = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .collect();
    let header = header.join("\n");
    assert!(header.contains("八命令"), "文件头「八命令」在位");
    assert!(header.contains("写命令两条"), "文件头「写命令两条」在位");
    assert!(!header.contains("六命令"), "文件头零旧口径「六命令」");
}

/// 边界（边界 / str 映射）：`root` 超长（>1000 chars）/ 含空格 / 非 ASCII / emoji → 落
/// `Result` 内不 panic（空串形态已由 root 校验行覆盖）。
#[test]
fn api命令root字符串边界() {
    let cases: [String; 4] = [
        std::env::temp_dir()
            .join("x".repeat(1100))
            .to_string_lossy()
            .into_owned(),
        std::env::temp_dir()
            .join("archi cmd with space")
            .to_string_lossy()
            .into_owned(),
        std::env::temp_dir()
            .join("架构-命令-根")
            .to_string_lossy()
            .into_owned(),
        std::env::temp_dir()
            .join("archi-cmd-🎯")
            .to_string_lossy()
            .into_owned(),
    ];
    for root in cases {
        assert!(
            archi_api_check(root.clone()).is_err(),
            "不存在的 root 应 Err（形态边界不 panic）：{root}"
        );
        assert!(
            archi_api_generate(root.clone()).is_err(),
            "不存在的 root 应 Err（形态边界不 panic）：{root}"
        );
    }
}
