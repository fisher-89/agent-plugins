//! `propose_config` 工具的单元测试：工具定义契约矩阵（名字 / 参数面 / 描述三面）、
//! `ConfigContext` 两模式执行体（只读盘面 / 校验回环）、字段级 `changes` 形态与
//! 零副作用双证。
//!
//! 零 mock（沿 test-design「Mock策略」表）：盘面读取走真实 `config::load_file_face`
//! （唯一读入口）、校验与归一化走真实 `config::validate` / `config::to_file_value`、
//! 输出收口走真实 `tools::cap_result`——替身会让「wire face 出线」「第二份上限实现」
//! 这类回潮不被发现。文件系统即真实 tempdir（RAII 清理）。

use std::fs;
use std::path::{Path, PathBuf};

use config::DiagnosticKind;
use serde_json::{json, Value};

use crate::sdk::config_propose::{definition, ConfigContext, PROPOSE_CONFIG_TOOL};
use crate::sdk::mount;
use crate::sdk::tools::MAX_RESULT_BYTES;

/// 临时 workspace 根（RAII 清理）。
struct TempRoot(tempfile::TempDir);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("config-propose-{tag}-"))
            .tempdir()
            .expect("创建临时目录失败");
        Self(dir)
    }

    fn path(&self) -> &Path {
        self.0.path()
    }

    /// 落一份盘面配置（自动建域目录）。
    fn write_config(&self, content: &str) {
        let path = foundation::layout::config_path(self.path());
        fs::create_dir_all(path.parent().expect("配置路径应有父目录")).expect("建域目录失败");
        fs::write(path, content).expect("写配置文件失败");
    }

    fn config_path(&self) -> PathBuf {
        foundation::layout::config_path(self.path())
    }
}

/// 上下文（`root` 为该临时根）。
fn context_of(root: &TempRoot) -> ConfigContext {
    ConfigContext::new(root.path().to_path_buf())
}

/// 执行并解析输出（输出恒单条 JSON 字符串）。
fn execute_json(context: &ConfigContext, input: &Value) -> Result<Value, String> {
    let text = context.execute(input)?;
    serde_json::from_str(&text).map_err(|error| format!("输出不是合法 JSON（{error}）：{text}"))
}

/// 满配盘面（八键全字段 + 多 suite + 多项目 + 多写保护规则）。
fn full_config_text() -> String {
    serde_json::to_string_pretty(&json!({
        "$schema": "https://example.com/spec-driven.schema.json",
        "schema": "spec-driven",
        "context": "桌面端插件仓库",
        "static_analysis": "clippy",
        "projects": [
            {"root": "packages/desktop", "language": "typescript", "depends_on": ["packages/shared"]},
            {"root": "packages/desktop/src-tauri", "language": "rust", "depends_on": ["packages/desktop"]},
        ],
        "rules": {"proposal": ["提案规则一"], "tasks": ["任务规则一"]},
        "tests": [
            {
                "root": "packages/desktop",
                "framework": "vite-plus",
                "cwd": "packages/desktop",
                "config": "vite-plus.config.ts",
                "includes": ["src/**/*.test.tsx"],
                "excludes": ["dist/**"],
                "coverage": {"lines": 90, "branches": 88, "functions": 85},
                "mutation": {"cwd": "packages/desktop", "score": 75},
            },
            {
                "root": "src-tauri",
                "framework": "rust",
                "cwd": "src-tauri",
                "coverage": {"lines": 80, "branches": 70, "functions": 75},
                "mutation": {"score": 70},
            },
        ],
        "write_protection": {
            "files": [
                {"glob": "tests/golden/**", "reason": "金样冻结"},
                {"glob": "Cargo.lock"},
            ],
        },
    }))
    .expect("满配盘面序列化失败")
}

/// 盘面文件字节 + mtime（零副作用双证的两个观测面）。
fn file_fingerprint(path: &Path) -> (Option<Vec<u8>>, Option<std::time::SystemTime>) {
    match fs::metadata(path) {
        Ok(metadata) => (
            Some(fs::read(path).expect("读配置文件失败")),
            metadata.modified().ok(),
        ),
        Err(_) => (None, None),
    }
}

/// 目录条目数（无临时文件残留的观测面）。
fn dir_entries(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| entries.count())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 工具定义契约（名字常量 / 参数面 / 描述三面）
// ---------------------------------------------------------------------------

/// 正向：常量值、定义名与挂载清单三面同源（定义单源在 `config_propose`）。
#[test]
fn 常量三面同源_定义名与挂载命中逐字相同() {
    assert_eq!(PROPOSE_CONFIG_TOOL, "propose_config");
    assert_eq!(
        definition().name,
        PROPOSE_CONFIG_TOOL,
        "定义名与常量逐字相同"
    );
    let mounted = mount::lookup(PROPOSE_CONFIG_TOOL).expect("挂载清单应命中该名");
    assert_eq!(mounted.name, PROPOSE_CONFIG_TOOL);
    assert_eq!(
        mounted.parameters,
        definition().parameters,
        "挂载定义与执行体模块定义同源（零分叉）"
    );
}

/// 边界：常量在挂载清单末项（第 19 项）出现恰一次（无重复登记）。
#[test]
fn 常量在挂载清单末项恰一处() {
    let names: Vec<&str> = mount::registry()
        .iter()
        .map(|mount| mount.definition.name.as_str())
        .collect();
    assert_eq!(names.len(), 19, "挂载清单十九项");
    assert_eq!(
        names.last().copied(),
        Some(PROPOSE_CONFIG_TOOL),
        "末项为提案工具"
    );
    assert_eq!(
        names
            .iter()
            .filter(|name| **name == PROPOSE_CONFIG_TOOL)
            .count(),
        1,
        "名字清单无重复登记"
    );
}

/// 正向：参数面 `{ candidate?: object }` + `required: []`，顶层无路径属性。
#[test]
fn 定义参数面_candidate可选对象且无路径属性() {
    let parameters = definition().parameters;
    assert_eq!(parameters["type"], json!("object"), "顶层类型为对象");
    let properties = parameters["properties"]
        .as_object()
        .expect("properties 恒为对象");
    assert_eq!(properties.len(), 1, "参数面只有 candidate 一项");
    assert_eq!(properties["candidate"]["type"], json!("object"));
    assert_eq!(
        parameters["required"],
        json!([]),
        "required 为空（候选可省 = 只读模式）"
    );
    for absent in ["path", "root", "file", "workspace"] {
        assert!(
            properties.get(absent).is_none(),
            "参数面不得出现路径属性 {absent}"
        );
    }
}

/// 正向：描述含两模式语义与「修正后重交完整候选」指引。
#[test]
fn 描述含两模式与重交完整候选指引() {
    let description = definition().description;
    for anchor in [
        "只读模式",
        "校验模式",
        "整份文件面",
        "重交完整候选",
        "changes",
    ] {
        assert!(description.contains(anchor), "描述缺语义锚 {anchor}");
    }
}

/// 边界：描述点明「无路径参数」与三处 snake_case 键形（防模型自造 `path` / camelCase）。
#[test]
fn 描述点明无路径参数与三处snake_case键形() {
    let description = definition().description;
    assert!(description.contains("无路径参数"), "描述须点明无路径参数");
    for key in ["static_analysis", "write_protection", "depends_on"] {
        assert!(
            description.contains(key),
            "描述缺点名的 snake_case 键 {key}"
        );
    }
}

/// 异常：描述不得暗示「可只传片段」（否定式表述在场、`partial` 类词零命中）。
#[test]
fn 描述不含可只传片段的暗示() {
    let description = definition().description;
    assert!(
        description.contains("不接受片段"),
        "描述须以否定式显式排除片段候选"
    );
    for hint in ["partial", "可以只传", "只需传改动"] {
        assert!(
            !description.contains(hint),
            "描述不得含片段候选的暗示 {hint}"
        );
    }
}

// ---------------------------------------------------------------------------
// `ConfigContext::new`：root 持有
// ---------------------------------------------------------------------------

/// 正向：构造后读到的盘面来自该 root（A 根落 A 配置、B 根落 B 配置）。
#[test]
fn 构造后盘面只来自该root() {
    let a = TempRoot::new("root-a");
    a.write_config(r#"{"context": "根 A"}"#);
    let b = TempRoot::new("root-b");
    b.write_config(r#"{"context": "根 B"}"#);

    let from_a = execute_json(&context_of(&a), &json!({})).expect("只读模式应成功");
    let from_b = execute_json(&context_of(&b), &json!({})).expect("只读模式应成功");

    assert_eq!(from_a["current"]["context"], json!("根 A"));
    assert_eq!(from_b["current"]["context"], json!("根 B"));
}

/// 边界（矩阵 2 形态）：root 不存在 / 空路径 → 构造不 panic，只读模式返回空盘面。
#[test]
fn root无效时构造不panic且盘面为空() {
    let missing = TempRoot::new("root-missing").path().join("absent");
    for root in [PathBuf::new(), missing] {
        let context = ConfigContext::new(root);
        let payload = execute_json(&context, &json!({})).expect("只读模式应成功");
        assert_eq!(payload["current"], json!({}), "不存在 root → 空盘面");
        let summary = payload["summary"].as_str().expect("summary 为字符串");
        assert!(
            summary.contains("诊断 1 条"),
            "FileMissing 折进 summary，实际 {summary}"
        );
    }
}

// ---------------------------------------------------------------------------
// 只读模式
// ---------------------------------------------------------------------------

/// 正向：`{}` → 四键输出、`candidate` 为 null、`changes` 为空、`current` 为 file face。
#[test]
fn 只读模式输出恰四键且candidate为null() {
    let root = TempRoot::new("readonly");
    root.write_config(&full_config_text());

    let payload = execute_json(&context_of(&root), &json!({})).expect("只读模式应成功");

    let mut keys: Vec<&str> = payload
        .as_object()
        .expect("输出恒对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["candidate", "changes", "current", "summary"],
        "恰四键"
    );
    assert_eq!(
        payload["candidate"],
        Value::Null,
        "只读模式 candidate 为 null"
    );
    assert_eq!(payload["changes"], json!([]), "只读模式无差异");
    assert!(
        payload["current"].get("static_analysis").is_some(),
        "current 为 file face（snake_case）"
    );
    assert!(
        payload["current"].get("staticAnalysis").is_none(),
        "current 不得为 wire face"
    );
}

/// 正向：`current` 逐值等于 `config::load_file_face` 输出（同源，`extra` 顶层展开）。
#[test]
fn 只读模式current与读入口同源() {
    let root = TempRoot::new("readonly-same");
    let mut raw: Value = serde_json::from_str(&full_config_text()).expect("满配可解析");
    raw["customFlag"] = json!({"nested": true});
    root.write_config(&serde_json::to_string(&raw).expect("序列化失败"));

    let (expected, diagnostics) = config::load_file_face(root.path()).expect("盘面投影应成功");
    let payload = execute_json(&context_of(&root), &json!({})).expect("只读模式应成功");

    assert_eq!(payload["current"], expected, "current 与读入口逐值相同");
    assert!(diagnostics.is_empty(), "满配盘面零诊断");
    assert_eq!(
        payload["current"]["customFlag"],
        json!({"nested": true}),
        "extra 条目展开回顶层"
    );
    assert!(
        payload["current"]["projects"][0]
            .get("depends_on")
            .is_some(),
        "projects[0].depends_on 键形"
    );
}

/// 边界（矩阵 2 形态）：`{"candidate": null}` 与 `{}` 等价；`{"candidate": {}}` 走校验模式。
#[test]
fn 候选null视同缺省而空对象走校验() {
    let root = TempRoot::new("readonly-null");
    root.write_config(r#"{"context": "仓库"}"#);
    let context = context_of(&root);

    let absent = execute_json(&context, &json!({})).expect("缺省应成功");
    let explicit_null = execute_json(&context, &json!({"candidate": null})).expect("null 应成功");
    assert_eq!(absent, explicit_null, "null 视同缺省（只读模式）");

    let empty_object = execute_json(&context, &json!({"candidate": {}})).expect("空对象应成功");
    assert_eq!(
        empty_object["candidate"],
        json!({}),
        "空对象是合法候选（非缺省）：按模板裁剪后为空对象"
    );
    assert_ne!(
        empty_object["candidate"],
        Value::Null,
        "空对象候选不得落回只读模式"
    );
}

/// 边界（矩阵 2 形态）：诊断折进 `summary`（不新增第五键）。
#[test]
fn 诊断折进summary两形态() {
    let missing = TempRoot::new("summary-missing");
    let payload = execute_json(&context_of(&missing), &json!({})).expect("只读模式应成功");
    let summary = payload["summary"].as_str().expect("summary 为字符串");
    assert!(summary.contains("只读模式"), "summary 保持只读模式语义");
    assert!(summary.contains("诊断 1 条"), "含诊断条数，实际 {summary}");
    assert!(summary.contains('$'), "含对位 path，实际 {summary}");
    assert_eq!(
        payload.as_object().map(|map| map.len()),
        Some(4),
        "诊断不新增第五键"
    );

    let clean = TempRoot::new("summary-clean");
    clean.write_config(&full_config_text());
    let payload = execute_json(&context_of(&clean), &json!({})).expect("只读模式应成功");
    let summary = payload["summary"].as_str().expect("summary 为字符串");
    assert!(summary.contains("无诊断"), "无诊断形态，实际 {summary}");
}

/// 边界（矩阵 4 形态）：入参非对象 → 只读模式（不 panic、四键输出）。
#[test]
fn 入参非对象四形态落只读模式() {
    let root = TempRoot::new("input-shapes");
    root.write_config(r#"{"context": "仓库"}"#);
    let context = context_of(&root);

    for input in [json!([]), json!("str"), json!(1), json!(null)] {
        let payload = execute_json(&context, &input)
            .unwrap_or_else(|error| panic!("入参 {input} 应落只读模式: {error}"));
        assert_eq!(payload["candidate"], Value::Null, "入参 {input}: 只读模式");
        assert_eq!(payload["changes"], json!([]), "入参 {input}: 无差异");
        assert!(payload["current"].is_object(), "入参 {input}: 盘面仍为对象");
    }
}

// ---------------------------------------------------------------------------
// 校验模式
// ---------------------------------------------------------------------------

/// 正向：干净 file face 候选 → `candidate` 为 file face、`changes` 形态齐、`summary` 一句中文。
#[test]
fn 校验模式干净候选_归一化与差异齐备() {
    let root = TempRoot::new("validate-clean");
    root.write_config(r#"{"schema": "spec-driven"}"#);
    let candidate = json!({
        "projects": [{"root": "packages/desktop", "language": "typescript", "depends_on": ["packages/shared"]}],
        "write_protection": {"files": [{"glob": "tests/golden/**"}]},
    });

    let payload =
        execute_json(&context_of(&root), &json!({"candidate": candidate})).expect("干净候选应成功");

    let normalized = payload["candidate"]
        .as_object()
        .expect("candidate 恒为对象");
    assert!(
        normalized.contains_key("projects"),
        "projects 键形 snake_case"
    );
    assert!(
        normalized.contains_key("write_protection"),
        "write_protection 键形（无 camelCase）"
    );
    assert!(normalized["projects"][0]
        .as_object()
        .expect("项目对象")
        .contains_key("depends_on"));
    let changes = payload["changes"].as_array().expect("changes 恒数组");
    assert!(!changes.is_empty(), "干净但有差异 → 非空");
    for change in changes {
        assert!(change["path"].is_string(), "差异项含 path");
        assert!(change["kind"].is_string(), "差异项含 kind");
        assert!(change.get("before").is_some(), "差异项含 before 位");
        assert!(change.get("after").is_some(), "差异项含 after 位");
    }
    let summary = payload["summary"].as_str().expect("summary 为字符串");
    assert!(
        summary.contains("候选合法"),
        "summary 一句中文，实际 {summary}"
    );
}

/// 正向（矩阵 3 形态）：`added` / `removed` / `changed` 三 kind 各一。
#[test]
fn 差异三kind覆盖() {
    let root = TempRoot::new("diff-kinds");
    root.write_config(r#"{"context": "旧上下文", "schema": "spec-driven"}"#);

    let payload = execute_json(
        &context_of(&root),
        &json!({"candidate": {"context": "新上下文", "newKey": 1}}),
    )
    .expect("候选应成功");

    let changes = payload["changes"].as_array().expect("changes 恒数组");
    let kinds: Vec<&str> = changes
        .iter()
        .map(|change| change["kind"].as_str().expect("kind 为字符串"))
        .collect();
    assert!(kinds.contains(&"added"), "新增顶层未知键 → added");
    assert!(kinds.contains(&"removed"), "删去盘面 context → removed");
    assert!(kinds.contains(&"changed"), "改 context 值 → changed");

    let added = changes
        .iter()
        .find(|change| change["path"] == json!("newKey"))
        .expect("新增项在案");
    assert_eq!(added["kind"], json!("added"));
    assert_eq!(added["before"], Value::Null, "新增 before 为 null");
    let removed = changes
        .iter()
        .find(|change| change["kind"] == json!("removed"))
        .expect("删除项在案");
    assert_eq!(removed["after"], Value::Null, "删除 after 为 null");
    let changed = changes
        .iter()
        .find(|change| change["path"] == json!("context"))
        .expect("变更项在案");
    assert_eq!(changed["kind"], json!("changed"));
    assert_eq!(changed["before"], json!("旧上下文"));
    assert_eq!(changed["after"], json!("新上下文"));
}

/// 正向（矩阵 2 形态）：数组作叶子——项目 / suite 内变化只产一条顶层 `changed`。
#[test]
fn 数组作叶子只产一条顶层changed() {
    let root = TempRoot::new("diff-leaf");
    root.write_config(
        r#"{
            "projects": [{"root": "pkg", "language": "typescript"}],
            "tests": [{"root": "pkg", "framework": "jest"}]
        }"#,
    );
    let contexts = context_of(&root);

    for (tag, candidate, path) in [
        (
            "projects",
            json!({"projects": [{"root": "pkg", "language": "rust"}], "tests": [{"root": "pkg", "framework": "jest"}]}),
            "projects",
        ),
        (
            "tests",
            json!({"projects": [{"root": "pkg", "language": "typescript"}], "tests": [{"root": "pkg", "framework": "rust"}]}),
            "tests",
        ),
    ] {
        let payload = execute_json(&contexts, &json!({"candidate": candidate}))
            .unwrap_or_else(|error| panic!("{tag}: 候选应成功: {error}"));
        let changes = payload["changes"].as_array().expect("changes 恒数组");
        assert_eq!(changes.len(), 1, "{tag}: 数组作叶子只产一条差异");
        assert_eq!(changes[0]["path"], json!(path), "{tag}: path 为数组键名");
        assert_eq!(changes[0]["kind"], json!("changed"));
    }
}

/// 边界：`changes` 序为字母序，`path` 为点分 file-face 键路径（无 `$` 前缀）。
#[test]
fn 差异序字母序与点分路径() {
    let root = TempRoot::new("diff-order");
    root.write_config(r#"{"projects": [{"root": "pkg", "language": "a"}]}"#);

    let payload = execute_json(
        &context_of(&root),
        &json!({"candidate": {
            "zeta": 1,
            "alpha": 2,
            "projects": [{"root": "pkg", "language": "b"}],
        }}),
    )
    .expect("候选应成功");

    let paths: Vec<&str> = payload["changes"]
        .as_array()
        .expect("changes 恒数组")
        .iter()
        .map(|change| change["path"].as_str().expect("path 为字符串"))
        .collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted, "changes 序为字母序（零额外排序）");
    assert!(paths.contains(&"alpha"), "顶层 path 即键名");
    assert!(
        paths.iter().all(|path| !path.starts_with('$')),
        "path 无 `$` 前缀"
    );
}

/// 边界：候选与盘面一致 → `changes` 为空且 summary 逐字。
#[test]
fn 候选与盘面一致时零差异与固定文案() {
    let root = TempRoot::new("diff-same");
    let face = json!({"schema": "spec-driven", "context": "仓库"});
    root.write_config(&serde_json::to_string(&face).expect("序列化失败"));

    let payload =
        execute_json(&context_of(&root), &json!({"candidate": face})).expect("一致候选应成功");

    assert_eq!(payload["changes"], json!([]), "零差异");
    assert_eq!(
        payload["summary"],
        json!("候选合法，与当前配置一致"),
        "summary 固定文案"
    );
}

/// 边界（矩阵 2 形态）：wire 面键名不进第二个 snake_case 字段——元素内未知子键丢弃、
/// 顶层未知键原文保留在顶层。
#[test]
fn wire面键名两形态零第二蛇形字段() {
    let root = TempRoot::new("wire-face");
    root.write_config(r#"{"schema": "spec-driven"}"#);
    let context = context_of(&root);

    let element = execute_json(
        &context,
        &json!({"candidate": {"projects": [{"root": "pkg", "dependsOn": ["other"]}]}}),
    )
    .expect("元素级候选应成功");
    let project = element["candidate"]["projects"][0]
        .as_object()
        .expect("项目对象");
    assert!(
        project.get("dependsOn").is_none() && project.get("depends_on").is_none(),
        "元素级未知子键（dependsOn）被丢弃，零第二个 snake_case 字段：{project:?}"
    );

    let top = execute_json(
        &context,
        &json!({"candidate": {"staticAnalysis": "clippy"}}),
    )
    .expect("顶层候选应成功");
    let projected = top["candidate"].as_object().expect("candidate 恒对象");
    assert_eq!(
        projected.get("staticAnalysis"),
        Some(&json!("clippy")),
        "顶层未知键原文保留在顶层（passthrough）"
    );
    assert!(
        projected.get("static_analysis").is_none(),
        "不得产出第二个 snake_case 字段"
    );
}

/// 异常：脏候选 → `Err`，文案逐条 `path: message` + 修正指引，零落盘。
#[test]
fn 脏候选err逐条诊断与修正指引() {
    let root = TempRoot::new("dirty");
    root.write_config(r#"{"schema": "spec-driven"}"#);
    let before = file_fingerprint(&root.config_path());
    let candidate = json!({
        "tests": [
            {"root": "a", "framework": "jest2"},
            {"root": "pkg", "framework": "jest", "coverage": {"lines": 101}},
            {"root": "src/*", "framework": "rust"},
        ],
    });

    let error = context_of(&root)
        .execute(&json!({"candidate": candidate}))
        .expect_err("脏候选必须 Err");

    for path in [
        "tests[0].framework",
        "tests[1].coverage.lines",
        "tests[2].root",
    ] {
        assert!(error.contains(path), "文案缺逐条 path {path}，实际 {error}");
    }
    assert!(
        error.contains("修正后重新提交完整候选"),
        "尾行含修正指引，实际 {error}"
    );
    assert_eq!(
        file_fingerprint(&root.config_path()),
        before,
        "脏候选零落盘（字节 + mtime 双证）"
    );
}

/// 异常（矩阵 3 形态）：候选非对象 → `Err`（`InvalidValue($)` 逐条），零 panic。
#[test]
fn 候选非对象三形态err() {
    let root = TempRoot::new("candidate-shapes");
    root.write_config(r#"{"schema": "spec-driven"}"#);
    let context = context_of(&root);

    for candidate in [json!([]), json!("s"), json!(1)] {
        let error = context
            .execute(&json!({"candidate": candidate}))
            .expect_err("非对象候选必须 Err");
        assert!(error.contains('$'), "文案 path 为文件级根，实际 {error}");
    }
}

// ---------------------------------------------------------------------------
// 边界：体量收口与副作用面
// ---------------------------------------------------------------------------

/// 边界（矩阵 2 模式）：满配 fixture 两模式输出均 < 50 000 字节且为可解析 JSON。
#[test]
fn 满配输出两模式均在字节上限内且可解析() {
    let root = TempRoot::new("cap-full");
    root.write_config(&full_config_text());
    let context = context_of(&root);

    let readonly = context.execute(&json!({})).expect("只读模式应成功");
    let validated = context
        .execute(&json!({"candidate": serde_json::from_str::<Value>(&full_config_text()).expect("满配可解析")}))
        .expect("校验模式应成功");

    for (tag, text) in [("只读", readonly), ("校验", validated)] {
        assert!(
            text.len() < MAX_RESULT_BYTES,
            "{tag}: 满配输出应在 L1 上限内，实测 {} 字节",
            text.len()
        );
        assert!(
            serde_json::from_str::<Value>(&text).is_ok(),
            "{tag}: 输出须为可解析 JSON"
        );
        assert!(!text.contains("已截断"), "{tag}: 不得有截断留痕");
    }
}

/// 边界：超大候选 → 输出经 `cap_result` 截断留痕（可辨识、不 panic）。
#[test]
fn 超大候选输出截断留痕() {
    let root = TempRoot::new("cap-huge");
    root.write_config(r#"{"schema": "spec-driven"}"#);
    let projects: Vec<Value> = (0..500)
        .map(|index| {
            json!({
                "root": format!("project-{index}"),
                "language": "rust",
                "depends_on": (0..20).map(|dep| format!("project-{dep}")).collect::<Vec<String>>(),
            })
        })
        .collect();
    let suites: Vec<Value> = (0..20)
        .map(|index| json!({"root": format!("suite-{index}"), "framework": "rust"}))
        .collect();
    let candidate = json!({"projects": projects, "tests": suites});

    let text = context_of(&root)
        .execute(&json!({"candidate": candidate}))
        .expect("超大候选仍应成功（收口层截断）");

    assert!(
        text.contains("已截断"),
        "超上限必须留截断痕迹（MUST NOT 冒充合法 JSON）"
    );
    assert!(text.len() > MAX_RESULT_BYTES, "截断文本 = 上限 + 留痕");
}

/// 边界：候选含顶层未知键 `path` → 输出原文保留、内容零改写（无路径沙箱改写面）。
#[test]
fn 候选顶层未知键path原文保留() {
    let root = TempRoot::new("unknown-path");
    root.write_config(r#"{"schema": "spec-driven"}"#);

    let payload = execute_json(
        &context_of(&root),
        &json!({"candidate": {"path": "src/a.ts"}}),
    )
    .expect("未知键候选应成功");

    assert_eq!(
        payload["candidate"]["path"],
        json!("src/a.ts"),
        "名为 path 的数据键原文保留（不被路径沙箱改写）"
    );
}

/// 异常：零副作用双证——两模式各一次调用前后，配置文件字节 + mtime 不变、
/// 目录条目零新增（无临时文件残留）；且零 `ConfigWrite` 落盘点参与。
#[test]
fn 两模式零副作用双证() {
    let root = TempRoot::new("no-side-effect");
    root.write_config(&full_config_text());
    let path = root.config_path();
    let before_file = file_fingerprint(&path);
    let before_entries = dir_entries(root.path());
    let context = context_of(&root);

    let candidate: Value = serde_json::from_str(&full_config_text()).expect("满配可解析");

    context.execute(&json!({})).expect("只读模式应成功");
    context
        .execute(&json!({"candidate": candidate}))
        .expect("校验模式应成功");
    context
        .execute(&json!({"candidate": json!({"context": "改一下"})}))
        .expect("校验模式（有差异）应成功");

    assert_eq!(
        file_fingerprint(&path),
        before_file,
        "配置文件字节 + mtime 双证不变"
    );
    assert_eq!(dir_entries(root.path()), before_entries, "无临时文件残留");
}

/// 正向：诊断类别集合与目录不存在时的 `FileMissing` 一致（执行体不造诊断）。
#[test]
fn 执行体透出读入口诊断而不自造() {
    let missing = TempRoot::new("diag-passthrough");
    let (_, diagnostics) = config::load_file_face(missing.path()).expect("盘面投影应成功");
    assert_eq!(diagnostics.len(), 1);
    assert!(matches!(&diagnostics[0].kind, DiagnosticKind::FileMissing));

    let payload = execute_json(&context_of(&missing), &json!({})).expect("只读模式应成功");
    let summary = payload["summary"].as_str().expect("summary 为字符串");
    assert!(
        summary.contains("诊断 1 条"),
        "条数与读入口一致（执行体零自造），实际 {summary}"
    );
}
