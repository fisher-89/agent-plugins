//! 集成测试：queries::detail → artifacts::registry 产物清单与信封读取（AC-6 / AC-7 / AC-8 / AC-9 / AC-10）。
//!
//! 缝合验证：detail 给出的 Descriptor (kind, source) 必须能被 read_artifact
//! 按 (kind, source) 精确回放成信封；读取失败返回 None（前端 Fallback 的上游契约）。

use std::fs;
use std::path::PathBuf;

use foundation::layout::resolve;
use workflow::model::{Inventory, Verdict};
use workflow::queries::{change_detail, list_changes};

/// 入仓 fixtures 语料根。
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

/// 把 fixture 拷入临时 workspace（archive 保留目录名 / active 用 fixture 名）。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-it-envelope-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn install_fixture(&self, fixture: &str, name: &str) {
        let source = fixtures_dir().join(fixture);
        let target = self.0.join("openspec/changes").join(name);
        copy_dir(&source, &target);
    }

    fn layout(&self) -> foundation::layout::Layout {
        resolve(&self.0)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    fs::create_dir_all(dst).expect("创建目标目录失败");
    for entry in fs::read_dir(src).expect("读取源目录失败").flatten() {
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("拷贝文件失败");
        }
    }
}

// ---------------------------------------------------------------------------
// 场景：v2 change 的产物清单与逐信封读取
// ---------------------------------------------------------------------------

#[test]
fn v2_b的detail产物清单与discover输出一致() {
    let ws = TempWs::new("v2b-consistency");
    ws.install_fixture("v2-b", "v2-b");
    let layout = ws.layout();
    let change_dir = layout.changes_root.join("v2-b");

    let detail = change_detail(&layout, "v2-b").expect("v2-b 应可定位");
    // 以与 detail 相同的口径（detect + load_workflow）重放 discover
    let inventory = workflow::parse::detect_inventory(&change_dir);
    let workflow_model = workflow::parse::load_workflow(&change_dir);
    let discovered =
        workflow::artifacts::discover_artifacts(&change_dir, inventory, workflow_model.as_ref());

    // detail 内嵌清单与注册表直查一致（kind / source / title 逐项相等）
    let from_detail: Vec<(String, String, String)> = detail
        .artifacts
        .iter()
        .map(|d| (d.kind.clone(), d.source.clone(), d.title.clone()))
        .collect();
    let from_discover: Vec<(String, String, String)> = discovered
        .iter()
        .map(|d| (d.kind.clone(), d.source.clone(), d.title.clone()))
        .collect();
    assert_eq!(from_detail, from_discover);

    // 三个第一波 kind 全部产出
    for kind in ["markdown-doc", "eval-checklist", "tasks-progress"] {
        assert!(
            detail.artifacts.iter().any(|d| d.kind == kind),
            "v2-b 应产出 {kind} descriptor"
        );
    }
}

#[test]
fn v2_b的每个descriptor都能回放出五字段齐全的信封() {
    let ws = TempWs::new("v2b-roundtrip");
    ws.install_fixture("v2-b", "v2-b");
    let layout = ws.layout();
    let change_dir = layout.changes_root.join("v2-b");

    let detail = change_detail(&layout, "v2-b").expect("v2-b 应可定位");
    assert!(!detail.artifacts.is_empty());
    let workflow_model = workflow::parse::load_workflow(&change_dir);

    for descriptor in &detail.artifacts {
        let envelope = workflow::artifacts::read_artifact(
            &change_dir,
            detail.inventory,
            workflow_model.as_ref(),
            &descriptor.kind,
            &descriptor.source,
        )
        .unwrap_or_else(|| panic!("descriptor {descriptor:?} 应可回放成信封"));
        assert_eq!(envelope.kind, descriptor.kind);
        assert_eq!(envelope.version, 1, "payload 契约 version=1");
        assert_eq!(envelope.title, descriptor.title);
        assert!(!envelope.payload.is_null(), "payload 形状非空");
        assert!(envelope.fallback_text.is_some(), "五字段齐全");
    }
}

#[test]
fn tasks_md双kind并存且各自读出正确信封() {
    let ws = TempWs::new("v2b-tasks");
    ws.install_fixture("v2-b", "v2-b");
    let layout = ws.layout();
    let change_dir = layout.changes_root.join("v2-b");

    let detail = change_detail(&layout, "v2-b").expect("v2-b 应可定位");
    let tasks_descriptors: Vec<_> = detail
        .artifacts
        .iter()
        .filter(|d| d.source == "tasks.md")
        .collect();
    let kinds: Vec<&str> = tasks_descriptors.iter().map(|d| d.kind.as_str()).collect();
    assert_eq!(
        kinds.len(),
        2,
        "tasks.md 同时产出两个 Descriptor（无排他）: {kinds:?}"
    );
    assert!(kinds.contains(&"markdown-doc"));
    assert!(kinds.contains(&"tasks-progress"));

    // 各自 read 出正确信封
    for descriptor in &tasks_descriptors {
        let envelope = workflow::artifacts::read_artifact(
            &change_dir,
            Inventory::V2,
            None,
            &descriptor.kind,
            "tasks.md",
        )
        .unwrap_or_else(|| panic!("{kind} 信封应可读取", kind = descriptor.kind));
        assert_eq!(envelope.title, descriptor.title);
        if descriptor.kind == "tasks-progress" {
            // v2-b tasks.md：3 done + 2 pending（含缩进嵌套两条）
            assert_eq!(envelope.payload["total"], 5);
            assert_eq!(envelope.payload["done"], 3);
            assert_eq!(envelope.payload["pending"], 2);
        }
    }
}

#[test]
fn eval_checklist信封payload契约形状符合设计表() {
    let ws = TempWs::new("v2b-checklist");
    ws.install_fixture("v2-b", "v2-b");
    let change_dir = ws.layout().changes_root.join("v2-b");
    let workflow_model = workflow::parse::load_workflow(&change_dir);

    let envelope = workflow::artifacts::read_artifact(
        &change_dir,
        Inventory::V2,
        workflow_model.as_ref(),
        "eval-checklist",
        "0",
    )
    .expect("eval[0] 含清单，应产出信封");
    assert_eq!(envelope.kind, "eval-checklist");
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.payload["phase"], "proposal");
    assert_eq!(envelope.payload["attempt"], 1);
    assert_eq!(envelope.payload["verdict"], "pass");
    let items = envelope.payload["items"].as_array().expect("items 数组");
    assert!(!items.is_empty());
    assert_eq!(items[0]["item"].as_str(), Some("问题描述清晰"));
    assert_eq!(items[0]["pass"], true);
    assert!(items[0]["evidence"].as_str().is_some());
    // verdict 枚举字串与模型一致
    assert_eq!(Verdict::Pass.as_str(), "pass");
}

// ---------------------------------------------------------------------------
// 场景：v0 纯文档形态的产物链路
// ---------------------------------------------------------------------------

#[test]
fn v0_change的产物清单等于全部md的markdown_doc清单() {
    let ws = TempWs::new("v0-docs");
    ws.install_fixture("v0-a", "v0-a");
    let layout = ws.layout();
    let change_dir = layout.changes_root.join("v0-a");

    let detail = change_detail(&layout, "v0-a").expect("v0-a 应可定位");
    assert_eq!(detail.inventory, Inventory::V0);
    assert!(detail.pipeline.is_empty(), "v0 无流水线区块");

    // 磁盘上全部 .md（递归）
    let mut expected: Vec<String> = Vec::new();
    collect_md(&change_dir, &mut expected);
    expected.sort();
    assert!(!expected.is_empty(), "v0-a 应含 markdown 四件套与 reports/");

    let doc_sources: Vec<String> = detail
        .artifacts
        .iter()
        .filter(|d| d.kind == "markdown-doc")
        .map(|d| d.source.clone())
        .collect();
    let mut doc_sorted = doc_sources.clone();
    doc_sorted.sort();
    assert_eq!(doc_sorted, expected, "产物清单 = 目录树内全部 .md");
    // v0 无 workflow → 无 eval-checklist 产出
    assert!(!detail.artifacts.iter().any(|d| d.kind == "eval-checklist"));

    // 直接读取 eval-checklist → None（无 workflow 数据源）
    let first_source = doc_sources.first().cloned().unwrap_or_default();
    assert!(
        workflow::artifacts::read_artifact(&change_dir, Inventory::V0, None, "eval-checklist", &first_source)
            .is_none(),
        "v0 下 eval-checklist 应无数据源产出"
    );
    // 逐个 markdown-doc 信封可读
    for source in &doc_sources {
        let envelope = workflow::artifacts::read_artifact(
            &change_dir,
            Inventory::V0,
            None,
            "markdown-doc",
            source,
        )
        .unwrap_or_else(|| panic!("{source} 应可读取"));
        assert_eq!(envelope.kind, "markdown-doc");
    }
    // 列表侧：v0-a 正常入列（扫描不因 v0 报错）
    let list = list_changes(&layout);
    assert_eq!(list.active.len(), 1);
}

fn collect_md(dir: &std::path::Path, out: &mut Vec<String>) {
    collect_md_inner(dir, dir, out);
}

fn collect_md_inner(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md_inner(root, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let relative = path
                .strip_prefix(root)
                .expect("strip_prefix 失败")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(relative);
        }
    }
}

// ---------------------------------------------------------------------------
// 场景：读取失败路径与兜底上游契约
// ---------------------------------------------------------------------------

#[test]
fn 未注册kind与无效source返回none() {
    let ws = TempWs::new("read-failures");
    ws.install_fixture("v2-b", "v2-b");
    let change_dir = ws.layout().changes_root.join("v2-b");

    // 未注册 kind（如 file-log）→ None：前端"读取失败以 Fallback 呈现"的上游语义
    assert!(workflow::artifacts::read_artifact(
        &change_dir,
        Inventory::V2,
        None,
        "file-log",
        "proposal.md"
    )
    .is_none());

    // source 指向不存在的文件 → None 而非报错
    assert!(workflow::artifacts::read_artifact(
        &change_dir,
        Inventory::V2,
        None,
        "markdown-doc",
        "已被删除的.md"
    )
    .is_none());
}

#[test]
fn source指向已被删除的文件时返回none而非报错() {
    let ws = TempWs::new("deleted-file");
    ws.install_fixture("v2-b", "v2-b");
    let layout = ws.layout();
    let change_dir = layout.changes_root.join("v2-b");

    // 先确认可读，再删文件，读取 → None（清单与磁盘漂移时的上游契约）
    assert!(
        workflow::artifacts::read_artifact(&change_dir, Inventory::V2, None, "markdown-doc", "proposal.md")
            .is_some()
    );
    fs::remove_file(change_dir.join("proposal.md")).expect("删除文件失败");
    assert!(workflow::artifacts::read_artifact(
        &change_dir,
        Inventory::V2,
        None,
        "markdown-doc",
        "proposal.md"
    )
    .is_none());
}

#[test]
fn read_artifact敌意source与kind被拒绝不逃逸change目录() {
    let ws = TempWs::new("hostile-source");
    ws.install_fixture("v2-b", "v2-b");
    let change_dir = ws.layout().changes_root.join("v2-b");

    // 越权目标：change 目录树外的秘密文件（fixture 同级、workspace 根两处）
    let outside = ws.0.join("openspec/changes/outside");
    fs::create_dir_all(&outside).expect("创建外部目录失败");
    fs::write(outside.join("secret.md"), "# 不应被越权读取").expect("写 secret 失败");
    let ws_secret = ws.0.join("secret.md");
    fs::write(&ws_secret, "# 工作区根秘密").expect("写根 secret 失败");

    // 形态层拒绝：.. 分量 / 绝对路径 / 反斜杠 / 盘符 / 空分量 / 序号串外的恶意形态
    for hostile in [
        "../outside/secret.md",
        "outside/../outside/secret.md",
        "v2-b/../../outside/secret.md",
        "..\\..\\outside\\secret.md",
        "..",
        ".",
        "v2-b/../",
        "",
        "/outside/secret.md",
        "C:\\evil\\secret.md",
        "C:/evil/secret.md",
    ] {
        assert!(
            workflow::artifacts::read_artifact(&change_dir, Inventory::V2, None, "markdown-doc", hostile)
                .is_none(),
            "敌意 source {hostile:?} 必须被拒绝"
        );
    }

    // kind 参数显式格式检查：空 kind → None（未注册同样 → None，语义一致）
    assert!(workflow::artifacts::read_artifact(&change_dir, Inventory::V2, None, "", "proposal.md").is_none());

    // 越权目标确实存在且可读（排除"恰好读不到"的假阳性）
    assert!(fs::read_to_string(&ws_secret).is_ok());

    // 正向对照：合法相对路径不受校验影响
    assert!(
        workflow::artifacts::read_artifact(&change_dir, Inventory::V2, None, "markdown-doc", "proposal.md")
            .is_some(),
        "合法 source 不应被误伤"
    );
}
