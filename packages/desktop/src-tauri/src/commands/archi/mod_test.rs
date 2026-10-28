//! `commands::archi` 的单元测试：六命令薄包装 ↔ `*_inner` 领域组装纯函数一一对应、
//! root 有效性校验（空白 / 非目录 / 不存在 → `Err`）、只读命令零写盘、`archi_set_gate`
//! 的原子写回与「任一校验失败零写入」，以及 `archi_change_summary` 的 store 记录解析
//! （worktree / 归档回落）。
//!
//! 装置沿 `commands/changes/mod_test.rs` 先例：`tauri::test::mock_app()` + 真实
//! `WorkspaceStores`（tempdir 数据根）托管进 app，`archi_change_summary` 以
//! `app.state::<…>()` 取 State 直调；无 State 的四命令直调原签名。命令层零领域规则
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
    archi_change_summary, archi_change_summary_inner, archi_drift, archi_drift_inner, archi_graph,
    archi_graph_inner, archi_module, archi_module_inner, archi_read_doc, archi_read_doc_inner,
    archi_set_gate, archi_set_gate_inner,
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

/// 正向：四命令与其 `*_inner` 结果经 `serde_json` 逐字段一致（命令层零加工）。
#[test]
fn 四命令与inner结果逐字段一致() {
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

    let doc_outer = archi_read_doc(temp.root(), "src/auth/README.md".to_owned()).expect("Ok");
    let doc_inner = archi_read_doc_inner(temp.path(), "src/auth/README.md").expect("Ok");
    assert_eq!(doc_outer, doc_inner, "archi_read_doc 与 _inner 一致");
    assert_eq!(doc_outer, "# 认证说明\n\n正文\n", "正文逐字节返回");

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
// root 校验（五命令共用）
// ---------------------------------------------------------------------------

/// 异常：root 为 `""` / 空白 / 指向文件 / 不存在 → 一律 `Err`，且不触达投影
/// （写命令零写入）。
#[test]
fn 五命令root校验四形态一律err() {
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
            archi_read_doc(rendering.clone(), "src/auth/README.md".to_owned()).is_err(),
            "archi_read_doc root 非法应 Err"
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

/// 正向：关联集内路径 → 返回正文原文（逐字节等于文件内容）。
#[test]
fn read_doc关联集内返回原文() {
    let temp = healthy("read-doc");
    let body = archi_read_doc(temp.root(), "src/auth/README.md".to_owned()).expect("Ok");
    let expected = fs::read_to_string(temp.path().join("src/auth/README.md")).expect("读文件");
    assert_eq!(body, expected, "正文逐字节等于文件内容");
}

/// 异常：越界拒绝——changes 产物 / 绝对路径 / `../` 逃逸 / 关联集内但文件已删
/// → `Err` 且零内容返回。
#[test]
fn read_doc越界四形态拒绝() {
    let temp = healthy("read-doc-deny");
    temp.write("openspec/changes/x/proposal.md", "# 提案\n");

    for path in [
        "openspec/changes/x/proposal.md",
        "../outside.md",
        "/etc/passwd",
    ] {
        let error = archi_read_doc(temp.root(), path.to_owned())
            .err()
            .unwrap_or_else(|| panic!("越界路径 {path:?} 应 Err（零内容返回）"));
        assert!(!error.is_empty(), "错误文案非空");
    }

    // 关联集内但文件已删除：关联集校验通过后读盘失败 → Err（不返回空串冒充正文）
    fs::remove_file(temp.path().join("src/auth/README.md")).expect("删文档");
    assert!(
        archi_read_doc(temp.root(), "src/auth/README.md".to_owned()).is_err(),
        "文件已删应 Err（零内容返回）"
    );
}

/// 边界：`path` 为空 / 超长 / emoji；关联集为空（空模型）时任意路径 `Err`；目录
/// 路径 `Err`。
#[test]
fn read_doc边界形态() {
    let temp = healthy("read-doc-bounds");
    let long = "x".repeat(1200);
    for path in [String::new(), long, "src/auth/🚀.md".to_owned()] {
        assert!(
            archi_read_doc(temp.root(), path.clone()).is_err(),
            "非法 path {path:?} 应 Err"
        );
    }
    // 目录路径（关联集内无该条目）
    assert!(
        archi_read_doc(temp.root(), "src/auth".to_owned()).is_err(),
        "目录路径应 Err"
    );

    // 关联集为空：空模型下任意路径一律 Err
    let empty = TempDir::new("read-doc-empty");
    fs::create_dir_all(archi_models_dir(empty.path())).expect("建空模型目录");
    empty.write("docs/a.md", "# 文档\n");
    assert!(
        archi_read_doc(empty.root(), "docs/a.md".to_owned()).is_err(),
        "关联集为空时任意路径均 Err（越界校验兜底）"
    );
}

/// 回归：四条读命令调用前后模型与文档字节全等（读命令零写盘）。
#[test]
fn 四条读命令零写盘() {
    let data = TempDir::new("nowrite-data");
    let temp = healthy("nowrite");
    let app = app_with(data.path());
    let store = store_of(&app, &temp.root());
    seed_record(&store, "nowrite", ChangeStatus::Active, Some(temp.root()));

    let before = snapshot(temp.path());
    let _ = archi_graph(temp.root());
    let _ = archi_module(temp.root(), "App.Auth".to_owned());
    let _ = archi_read_doc(temp.root(), "src/auth/README.md".to_owned());
    let _ = archi_change_summary(
        app.state::<WorkspaceStores>(),
        temp.root(),
        "nowrite".to_owned(),
    );
    let after = snapshot(temp.path());
    assert_eq!(before, after, "四条读命令零写盘");
}

// ---------------------------------------------------------------------------
// 写命令：原子写回 / 零写入 / 往返
// ---------------------------------------------------------------------------

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
