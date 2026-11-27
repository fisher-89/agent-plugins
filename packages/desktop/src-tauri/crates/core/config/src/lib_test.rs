//! `config::load` 的单元测试（AC-2）：配置语义唯一入口的文件态四分支 + 顶层与
//! 基础字段 / tests suite / coverage / mutation / write_protection / passthrough
//! 组装校验矩阵 + 自洽不变量（「config 输出永远合法」的机械形态）。
//!
//! 文件系统不 mock：真实 tempdir fixture（RAII 清理）承载缺失 / 不可读（同名
//! 目录占据）/ 坏 JSON / 合法配置各形态；serde_json 只作 DTO 无 PartialEq 的
//! 线面对照载体，不逐项断言 serde / JSON 库自身语义（测试纪律）。`assemble`
//! 经 crate 内子模块路径访问（子模块可见父模块私有项），自洽不变量直接调用，
//! 不新增测试专用导出（CLAUDE.md no-test-only-exports）。
//!
//! 末段为 config 契约镜像 ↔ 实现的一致性断言（A1 键集 / A2 枚举 / A3 往返 +
//! `wire_name` 穷尽 match 编译期守卫）：镜像以 `include_str!` 编译期绑定入仓
//! `../config.schema.json`（零运行时 IO、零 cwd 依赖、零路径拼装），断言体为
//! 入参化纯函数——正向行以入仓镜像驱动，异常 / 边界行以内存副本驱动，同样
//! 零新增导出。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use super::{
    assemble, known_keys, load, load_file_face, parse_framework, plan_apply_config, to_file_value,
    validate, ConfigExtraField, ConfigReport, DiagnosticKind, TestFramework, TestSuite,
    WorkspaceConfig, KNOWN_KEYS,
};
use foundation::layout::config_path;

/// 临时 workspace 根 RAII：测试结束自动清理。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("config-lib-test-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn ensure_dir(&self) {
        fs::create_dir_all(&self.0).expect("创建临时根目录失败");
    }

    /// 相对 root 落一份配置文件（自动建父目录域目录）。
    fn write_config(&self, content: &str) {
        let path = config_path(&self.0);
        fs::create_dir_all(path.parent().expect("配置路径应有父目录")).expect("建域目录失败");
        fs::write(path, content).expect("写配置文件失败");
    }

    /// 配置文件路径被同名目录占据（存在但不可读：read_to_string 必败且非
    /// NotFound，ReadFailed 分支 fixture）。
    fn occupy_config_path_as_dir(&self) {
        fs::create_dir_all(config_path(&self.0)).expect("占据配置路径失败");
    }

    fn remove_config(&self) {
        let _ = fs::remove_file(config_path(&self.0));
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// kind 同类判别（枚举无 PartialEq，以判别值序比对；不比较 message 文案）。
fn kind_rank(kind: &DiagnosticKind) -> u8 {
    match kind {
        DiagnosticKind::FileMissing => 0,
        DiagnosticKind::ReadFailed => 1,
        DiagnosticKind::JsonInvalid => 2,
        DiagnosticKind::InvalidValue => 3,
        DiagnosticKind::DefaultApplied => 4,
    }
}

/// 报告内是否存在「kind + path」对位的诊断。
fn has_diag(report: &ConfigReport, kind: &DiagnosticKind, path: &str) -> bool {
    report
        .diagnostics
        .iter()
        .any(|d| kind_rank(&d.kind) == kind_rank(kind) && d.path == path)
}

/// config 线面值（无 PartialEq DTO 的对照载体，非 serde 语义断言）。
fn value_of(config: &WorkspaceConfig) -> Value {
    serde_json::to_value(config).expect("config 序列化失败")
}

/// config 与全默认基线逐字段一致（线面值对照）。
fn assert_default_config(config: &WorkspaceConfig) {
    assert_eq!(
        value_of(config),
        value_of(&WorkspaceConfig::default()),
        "config 应为全默认基线，实际 {config:?}"
    );
}

/// suite framework 的线面字面量（枚举无 PartialEq 的比对载体）。
fn framework_of(suite: &TestSuite) -> String {
    match serde_json::to_value(&suite.framework).expect("framework 序列化失败") {
        Value::String(text) => text,
        other => panic!("framework 线面应为字符串，实际 {other}"),
    }
}

/// 自洽不变量的机械形态：config 重序列化回 `Value` 再过同一 [`assemble`]，
/// 诊断恒为空（「config 输出永远合法」的判据口径；线面 `extra: []` 回写后
/// 经 passthrough 再入场属组装配对语义，不在无损断言之列）。
fn assert_self_consistent(config: &WorkspaceConfig) {
    let value = value_of(config);
    let map = value.as_object().expect("config 线面恒为对象");
    let (_, diagnostics) = assemble(map);
    assert!(
        diagnostics.is_empty(),
        "回写组装应零诊断，实际 {diagnostics:?}"
    );
}

/// 最小合法 suite JSON 片段（root + framework 必需字段齐备）。
fn minimal_suite(root: &str, framework: &str) -> String {
    format!(r#"{{"root": "{root}", "framework": "{framework}"}}"#)
}

/// 完整合法 suite JSON（全字段显式设值：cwd / config / includes / excludes /
/// coverage / mutation 全给）——「零诊断」用例的合法基线。
fn full_suite(root: &str, framework: &str, coverage: &str) -> String {
    format!(
        r#"{{"root": "{root}", "framework": "{framework}", "cwd": "{root}", "config": null, "includes": [], "excludes": [], "coverage": {coverage}, "mutation": {{"cwd": null, "score": 70}}}}"#
    )
}

/// 合法 config 信封包裹（schema 显式设值 + tests 数组）。
fn config_with_tests(tests_json: &str) -> String {
    format!(r#"{{"schema": "spec-driven", "tests": [{tests_json}]}}"#)
}

// ---------------------------------------------------------------------------
// load：文件级四分支（缺失 / 读取失败 / 语法非法 / 空白内容）
// ---------------------------------------------------------------------------

#[test]
fn 有效root无配置文件_全默认config加_file_missing_诊断_path_为文件级根() {
    let ws = TempWs::new("file-missing");
    ws.ensure_dir();

    let report = load(ws.root());

    assert_default_config(&report.config);
    assert_eq!(report.diagnostics.len(), 1, "缺失恰一条文件级诊断");
    assert!(has_diag(&report, &DiagnosticKind::FileMissing, "$"));
    assert!(
        report.diagnostics[0].message.contains("不存在"),
        "文案应说明文件缺失，实际 {:?}",
        report.diagnostics[0].message
    );
}

#[test]
fn 配置路径被同名目录占据_全默认config加_read_failed_诊断() {
    let ws = TempWs::new("config-path-is-dir");
    ws.ensure_dir();
    ws.occupy_config_path_as_dir();

    let report = load(ws.root());

    assert_default_config(&report.config);
    assert!(has_diag(&report, &DiagnosticKind::ReadFailed, "$"));
}

#[test]
fn 坏json语法_全默认config加_json_invalid_诊断() {
    let ws = TempWs::new("bad-json");
    ws.ensure_dir();
    ws.write_config("{ 这不是合法 JSON ]");

    let report = load(ws.root());

    assert_default_config(&report.config);
    assert!(has_diag(&report, &DiagnosticKind::JsonInvalid, "$"));
}

#[test]
fn 空文件与仅空白字符落json_invalid分支不panic() {
    for (tag, content) in [("empty", ""), ("blank", "   \n\t  ")] {
        let ws = TempWs::new(&format!("blank-{tag}"));
        ws.ensure_dir();
        ws.write_config(content);

        let report = load(ws.root());

        assert_default_config(&report.config);
        assert!(
            has_diag(&report, &DiagnosticKind::JsonInvalid, "$"),
            "内容 {content:?} 应落 JsonInvalid 分支"
        );
    }
}

// ---------------------------------------------------------------------------
// load：顶层与基础字段（schema / $schema / context / static_analysis / rules）
// ---------------------------------------------------------------------------

#[test]
fn 全字段合法完整配置_逐字段原样保留且diagnostics为空() {
    let ws = TempWs::new("full-valid");
    ws.ensure_dir();
    ws.write_config(
        r#"{
            "$schema": "https://example.com/spec-driven.schema.json",
            "schema": "spec-driven",
            "context": "桌面端插件仓库",
            "static_analysis": "clippy",
            "projects": [{ "root": "packages/desktop" }, { "root": "packages/desktop/src-tauri" }],
            "rules": { "proposal": ["提案规则一", "提案规则二"], "tasks": ["任务规则一"] },
            "tests": [{
                "root": "packages/desktop",
                "framework": "vite-plus",
                "cwd": "packages/desktop",
                "config": "vite-plus.config.ts",
                "includes": ["src/**/*.test.tsx"],
                "excludes": [],
                "coverage": { "lines": 90.5, "branches": 80, "functions": 85 },
                "mutation": { "cwd": "packages/desktop", "score": 75 }
            }],
            "write_protection": { "files": [{ "glob": "tests/golden/**", "reason": "金样冻结" }] }
        }"#,
    );

    let report = load(ws.root());

    assert!(report.diagnostics.is_empty(), "合法配置应零诊断");
    let config = &report.config;
    assert_eq!(
        config.schema_ref.as_deref(),
        Some("https://example.com/spec-driven.schema.json")
    );
    assert_eq!(config.schema, "spec-driven");
    assert_eq!(config.context.as_deref(), Some("桌面端插件仓库"));
    assert_eq!(config.static_analysis.as_deref(), Some("clippy"));
    assert_eq!(config.projects.len(), 2, "projects 保序全保留");
    assert_eq!(config.projects[0].root, "packages/desktop");
    assert_eq!(config.projects[1].root, "packages/desktop/src-tauri");
    let rules = config.rules.as_ref().expect("rules 应保留");
    assert_eq!(
        rules.proposal.as_deref(),
        Some(&["提案规则一".to_owned(), "提案规则二".to_owned()][..])
    );
    assert_eq!(rules.tasks.as_deref(), Some(&["任务规则一".to_owned()][..]));
    assert_eq!(config.tests.len(), 1);
    let suite = &config.tests[0];
    assert_eq!(suite.root, "packages/desktop");
    assert_eq!(framework_of(suite), "vite-plus");
    assert_eq!(suite.cwd, "packages/desktop");
    assert_eq!(suite.config.as_deref(), Some("vite-plus.config.ts"));
    assert_eq!(
        suite.includes.as_deref(),
        Some(&["src/**/*.test.tsx".to_owned()][..])
    );
    assert_eq!(suite.excludes, Some(Vec::new()));
    assert_eq!(suite.coverage.lines, 90.5);
    assert_eq!(suite.coverage.branches, 80.0);
    assert_eq!(suite.coverage.functions, 85.0);
    assert_eq!(suite.mutation.cwd.as_deref(), Some("packages/desktop"));
    assert_eq!(suite.mutation.score, 75.0);
    let protection = config.write_protection.as_ref().expect("保护配置应保留");
    let files = protection.files.as_ref().expect("files 应保留");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].glob.as_deref(), Some("tests/golden/**"));
    assert_eq!(files[0].reason.as_deref(), Some("金样冻结"));
    assert!(config.extra.is_empty(), "已知字段不进 extra");
}

#[test]
fn 顶层非对象三形态_全默认config加_invalid_value_path_为文件级根不失败() {
    for (tag, content) in [("array", "[1, 2]"), ("number", "42"), ("null", "null")] {
        let ws = TempWs::new(&format!("top-{tag}"));
        ws.ensure_dir();
        ws.write_config(content);

        let report = load(ws.root());

        assert_default_config(&report.config);
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "$"),
            "顶层 {content} 应记 InvalidValue（path $）"
        );
    }
}

#[test]
fn 可选字符串字段非字符串_none加逐字段_invalid_value() {
    let ws = TempWs::new("opt-string-invalid");
    ws.ensure_dir();
    ws.write_config(r#"{"$schema": 42, "context": [], "static_analysis": true}"#);

    let report = load(ws.root());

    assert!(report.config.schema_ref.is_none());
    assert!(report.config.context.is_none());
    assert!(report.config.static_analysis.is_none());
    assert!(has_diag(&report, &DiagnosticKind::InvalidValue, "$schema"));
    assert!(has_diag(&report, &DiagnosticKind::InvalidValue, "context"));
    assert!(
        has_diag(&report, &DiagnosticKind::InvalidValue, "static_analysis"),
        "实际诊断 {:?}",
        report.diagnostics
    );
}

#[test]
fn 可选字符串字段显式null与缺失同处置_无诊断_null位语义() {
    let ws = TempWs::new("opt-string-null");
    ws.ensure_dir();
    ws.write_config(r#"{"schema": "spec-driven", "tests": [], "$schema": null, "context": null, "static_analysis": null}"#);

    let report = load(ws.root());

    assert!(report.config.schema_ref.is_none());
    assert!(report.config.context.is_none());
    assert!(report.config.static_analysis.is_none());
    assert!(
        report.diagnostics.is_empty(),
        "null 位无诊断，实际 {:?}",
        report.diagnostics
    );
}

#[test]
fn schema字段非字面量落默认并记_invalid_value_缺失记_default_applied() {
    // literal 端口：空串 / 大小写变体 / 任意串 / null / 数字 / 数组 一律落默认
    for value in [
        r#""""#,
        r#""Spec-Driven""#,
        r#""SPEC-DRIVEN""#,
        r#""spec-driven-x""#,
        "null",
        "3",
        "[]",
    ] {
        let ws = TempWs::new("schema-literal");
        ws.ensure_dir();
        ws.write_config(&format!(r#"{{"schema": {value}}}"#));

        let report = load(ws.root());

        assert_eq!(report.config.schema, "spec-driven");
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "schema"),
            "schema={value} 应记 InvalidValue，实际 {:?}",
            report.diagnostics
        );
    }

    // 缺失 → DefaultApplied（同默认值、不同 kind）
    let ws = TempWs::new("schema-defaulted");
    ws.ensure_dir();
    ws.write_config("{}");
    let report = load(ws.root());
    assert_eq!(report.config.schema, "spec-driven");
    assert!(has_diag(&report, &DiagnosticKind::DefaultApplied, "schema"));
}

#[test]
fn rules非对象_none加_invalid_value_显式null同缺失无诊断() {
    for (tag, content) in [
        ("array", r#"{"rules": []}"#),
        ("string", r#"{"rules": "x"}"#),
    ] {
        let ws = TempWs::new(&format!("rules-{tag}"));
        ws.ensure_dir();
        ws.write_config(content);

        let report = load(ws.root());

        assert!(report.config.rules.is_none());
        assert!(has_diag(&report, &DiagnosticKind::InvalidValue, "rules"));
    }

    let ws = TempWs::new("rules-null");
    ws.ensure_dir();
    ws.write_config(r#"{"schema": "spec-driven", "tests": [], "rules": null}"#);
    let report = load(ws.root());
    assert!(report.config.rules.is_none());
    assert!(report.diagnostics.is_empty());
}

#[test]
fn rules子字段元素混入非字符串_该子字段none加_invalid_value_path对位() {
    let ws = TempWs::new("rules-subfield");
    ws.ensure_dir();
    ws.write_config(r#"{"rules": {"proposal": ["合法", 1], "tasks": [2]}}"#);

    let report = load(ws.root());

    let rules = report.config.rules.as_ref().expect("rules 对象本体保留");
    assert!(rules.proposal.is_none());
    assert!(rules.tasks.is_none());
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "rules.proposal"
    ));
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "rules.tasks"
    ));
}

#[test]
fn rules子字段空数组与多元素列表合法原样保留_无诊断() {
    let ws = TempWs::new("rules-list-edge");
    ws.ensure_dir();
    ws.write_config(
        r#"{"schema": "spec-driven", "tests": [], "rules": {"proposal": [], "tasks": ["规则一", "规则二", "规则三"]}}"#,
    );

    let report = load(ws.root());

    assert!(report.diagnostics.is_empty());
    let rules = report.config.rules.as_ref().expect("rules 应保留");
    assert_eq!(rules.proposal, Some(Vec::new()), "空数组合法保留");
    assert_eq!(
        rules.tasks.as_deref(),
        Some(
            &[
                "规则一".to_owned(),
                "规则二".to_owned(),
                "规则三".to_owned()
            ][..]
        )
    );
}

// ---------------------------------------------------------------------------
// load：projects（依赖引导项目根）
// ---------------------------------------------------------------------------

#[test]
fn projects缺失与null静默空列表_显式空数组无诊断() {
    for content in [
        r#"{"schema": "spec-driven", "tests": []}"#,
        r#"{"schema": "spec-driven", "tests": [], "projects": null}"#,
        r#"{"schema": "spec-driven", "tests": [], "projects": []}"#,
    ] {
        let ws = TempWs::new("projects-absent");
        ws.ensure_dir();
        ws.write_config(content);

        let report = load(ws.root());

        assert!(report.config.projects.is_empty());
        assert!(
            report.diagnostics.is_empty(),
            "content={content} 应零诊断，实际 {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn projects非数组落空列表并记invalid_value() {
    for value in ["{}", r#""x""#, "3", "true"] {
        let ws = TempWs::new("projects-not-array");
        ws.ensure_dir();
        ws.write_config(&format!(r#"{{"projects": {value}}}"#));

        let report = load(ws.root());

        assert!(report.config.projects.is_empty());
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "projects"),
            "projects={value} 应记 InvalidValue，实际 {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn projects合法多元素保序保留_点号根与未知子键合法() {
    let ws = TempWs::new("projects-legal");
    ws.ensure_dir();
    ws.write_config(
        r#"{"schema": "spec-driven", "tests": [], "projects": [
            {"root": "packages/desktop"},
            {"root": "."},
            {"root": "a/b", "note": "未知子键忽略"}
        ]}"#,
    );

    let report = load(ws.root());

    assert!(
        report.diagnostics.is_empty(),
        "合法 projects 应零诊断，实际 {:?}",
        report.diagnostics
    );
    let roots: Vec<&str> = report
        .config
        .projects
        .iter()
        .map(|project| project.root.as_str())
        .collect();
    assert_eq!(roots, ["packages/desktop", ".", "a/b"]);
}

#[test]
fn projects元素非对象或root非法逐条剔除_合法保序() {
    let cases: [(&str, &str); 11] = [
        ("5", "projects[0]"),
        (r#"{"root": ""}"#, "projects[0].root"),
        (r#"{"root": 5}"#, "projects[0].root"),
        (r#"{"other": "x"}"#, "projects[0].root"),
        (r#"{"root": "src/**"}"#, "projects[0].root"),
        (r#"{"root": "../x"}"#, "projects[0].root"),
        (r#"{"root": "a/../b"}"#, "projects[0].root"),
        (r#"{"root": "a\\..\\b"}"#, "projects[0].root"),
        (r#"{"root": "/abs"}"#, "projects[0].root"),
        (r#"{"root": "C:\\x"}"#, "projects[0].root"),
        (r#"{"root": "foo:bar"}"#, "projects[0].root"),
    ];
    for (item, path) in cases {
        let ws = TempWs::new("projects-invalid");
        ws.ensure_dir();
        ws.write_config(&format!(r#"{{"projects": [{item}]}}"#));

        let report = load(ws.root());

        assert!(report.config.projects.is_empty(), "item={item} 应整条剔除");
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, path),
            "item={item} 应记 path={path}，实际 {:?}",
            report.diagnostics
        );
    }

    // 混合形态：违例剔除、合法保序保留、诊断不殃及同侪
    let ws = TempWs::new("projects-mixed");
    ws.ensure_dir();
    ws.write_config(
        r#"{"projects": [{"root": "keep"}, {"root": "../drop"}, {"root": "also-keep"}]}"#,
    );
    let report = load(ws.root());
    let roots: Vec<&str> = report
        .config
        .projects
        .iter()
        .map(|project| project.root.as_str())
        .collect();
    assert_eq!(roots, ["keep", "also-keep"]);
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "projects[1].root"
    ));
    assert!(
        !has_diag(&report, &DiagnosticKind::InvalidValue, "projects[0].root"),
        "违例处置不殃及合法条目"
    );
}

// ---------------------------------------------------------------------------
// load：projects[].language / projects[].depends_on（协议扩展两字段）
// ---------------------------------------------------------------------------

/// 合法信封（schema 显式设值 + 空 tests）包裹 projects 数组片段——零诊断基线。
fn config_with_projects(projects_json: &str) -> String {
    format!(r#"{{"schema": "spec-driven", "tests": [], "projects": {projects_json}}}"#)
}

/// 单元素 projects 的 load 观察面（`project_json` 为元素对象原文）。
fn load_single_project(tag: &str, project_json: &str) -> ConfigReport {
    let ws = TempWs::new(tag);
    ws.ensure_dir();
    ws.write_config(&config_with_projects(&format!("[{project_json}]")));
    load(ws.root())
}

/// 单元素 projects 的 config（项目元素原文）——自洽不变量 / 线面形态用例复用。
fn load_single_project_config(tag: &str, project_json: &str) -> WorkspaceConfig {
    load_single_project(tag, project_json).config
}

#[test]
fn 项目language缺失与显式null均none且零诊断() {
    for project in [r#"{"root": "web"}"#, r#"{"root": "web", "language": null}"#] {
        let report = load_single_project("language-absent", project);

        assert!(
            report.diagnostics.is_empty(),
            "project={project} 应零诊断，实际 {:?}",
            report.diagnostics
        );
        assert_eq!(report.config.projects.len(), 1);
        assert!(
            report.config.projects[0].language.is_none(),
            "project={project} 的 language 应为 None"
        );
    }
}

#[test]
fn 项目language合法字符串原样保留不trim不归一() {
    let long = "l".repeat(1200);
    for value in [
        "typescript",
        "  rust  ",
        "TypeScript",
        "C++",
        "🙂",
        long.as_str(),
    ] {
        let encoded = serde_json::to_string(value).expect("language 原文应可序列化进 JSON 夹具");
        let report = load_single_project(
            "language-legal",
            &format!(r#"{{"root": "web", "language": {encoded}}}"#),
        );

        assert!(
            report.diagnostics.is_empty(),
            "language={value:?} 应零诊断，实际 {:?}",
            report.diagnostics
        );
        assert_eq!(
            report.config.projects[0].language.as_deref(),
            Some(value),
            "language 应逐字原样保留（不 trim / 不归一）"
        );
    }
}

#[test]
fn 项目language空串保留为some空串且零诊断() {
    let report = load_single_project("language-empty", r#"{"root": "web", "language": ""}"#);

    assert!(
        report.diagnostics.is_empty(),
        "空串 language 零内容校验，应零诊断，实际 {:?}",
        report.diagnostics
    );
    assert_eq!(
        report.config.projects[0].language.as_deref(),
        Some(""),
        "空串不得退化为 None（Some(\"\") 与 None 可区分）"
    );
}

#[test]
fn 项目language非字符串落none并记invalid_value且元素仍在案() {
    for value in ["5", "true", "{}", "[]"] {
        let report = load_single_project(
            "language-invalid",
            &format!(r#"{{"root": "web", "language": {value}}}"#),
        );

        assert_eq!(
            report.config.projects.len(),
            1,
            "language={value} 不参与元素去留，元素应仍在案"
        );
        assert_eq!(
            report.config.projects[0].root, "web",
            "同元素合法字段不受牵连"
        );
        assert!(
            report.config.projects[0].language.is_none(),
            "language={value} 应落 None"
        );
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::InvalidValue,
                "projects[0].language"
            ),
            "language={value} 应记 path=projects[0].language，实际 {:?}",
            report.diagnostics
        );
        assert_eq!(
            report.diagnostics.len(),
            1,
            "language={value} 恰一条诊断，实际 {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn 项目depends_on缺失与显式null均none且零诊断() {
    for project in [
        r#"{"root": "web"}"#,
        r#"{"root": "web", "depends_on": null}"#,
    ] {
        let report = load_single_project("deps-absent", project);

        assert!(
            report.diagnostics.is_empty(),
            "project={project} 应零诊断，实际 {:?}",
            report.diagnostics
        );
        assert!(
            report.config.projects[0].depends_on.is_none(),
            "project={project} 的 depends_on 应为 None"
        );
    }
}

#[test]
fn 项目depends_on保序不去重且悬空项在案() {
    let report = load_single_project(
        "deps-order",
        r#"{"root": "web", "depends_on": ["b", "a", "b", "ghost"]}"#,
    );

    assert!(
        report.diagnostics.is_empty(),
        "悬空依赖零引用校验，应零诊断，实际 {:?}",
        report.diagnostics
    );
    assert_eq!(
        report.config.projects[0].depends_on.as_deref(),
        Some(
            &[
                "b".to_owned(),
                "a".to_owned(),
                "b".to_owned(),
                "ghost".to_owned()
            ][..]
        ),
        "depends_on 应保序、不去重、悬空项原样在案"
    );
}

#[test]
fn 项目depends_on空数组单元素与超大列表原样保序() {
    // 空数组 → Some(vec![])，与 None 可区分
    let report = load_single_project("deps-empty", r#"{"root": "web", "depends_on": []}"#);
    assert!(
        report.diagnostics.is_empty(),
        "显式空数组应零诊断，实际 {:?}",
        report.diagnostics
    );
    let empty = report.config.projects[0]
        .depends_on
        .clone()
        .expect("空数组应落 Some(vec![]) 而非 None");
    assert!(empty.is_empty(), "空数组应保空，实际 {empty:?}");

    // 单元素
    let report = load_single_project(
        "deps-single",
        r#"{"root": "web", "depends_on": ["single"]}"#,
    );
    assert_eq!(
        report.config.projects[0].depends_on.as_deref(),
        Some(&["single".to_owned()][..]),
        "单元素列表原样保留"
    );

    // 1000 元素大列表 → 原样保序
    let big: Vec<String> = (0..1000).map(|index| format!("p{index}")).collect();
    let project = json!({"root": "web", "depends_on": big}).to_string();
    let report = load_single_project("deps-big", &project);
    assert!(
        report.diagnostics.is_empty(),
        "大列表应零诊断，实际 {:?}",
        report.diagnostics
    );
    assert_eq!(
        report.config.projects[0].depends_on.as_deref(),
        Some(&big[..]),
        "1000 元素列表应原样保序"
    );
}

#[test]
fn 项目depends_on元素内容零校验_empty_padded_emoji_long() {
    let long = "x".repeat(1200);
    let elements = ["", "  padded  ", "emoji🙂", long.as_str()];
    for element in elements {
        let encoded = serde_json::to_string(element).expect("depends_on 元素应可序列化进 JSON");
        let report = load_single_project(
            "deps-content",
            &format!(r#"{{"root": "web", "depends_on": [{encoded}]}}"#),
        );

        assert!(
            report.diagnostics.is_empty(),
            "element={element:?} 零内容校验，应零诊断，实际 {:?}",
            report.diagnostics
        );
        assert_eq!(
            report.config.projects[0].depends_on.as_deref(),
            Some(&[element.to_owned()][..]),
            "element={element:?} 应原样保留（空串项不触发诊断）"
        );
    }
}

#[test]
fn 项目depends_on混入非字符串整字段none并记invalid_value() {
    for tail in ["5", "null", "true", "[\"c\"]"] {
        let report = load_single_project(
            "deps-mixed",
            &format!(r#"{{"root": "web", "depends_on": ["b", {tail}]}}"#),
        );

        assert_eq!(
            report.config.projects.len(),
            1,
            "depends_on={tail} 混入非字符串不参与元素去留，元素应仍在案"
        );
        assert_eq!(report.config.projects[0].root, "web");
        assert!(
            report.config.projects[0].depends_on.is_none(),
            "混入 {tail} 时整字段应落 None（不部分保留）"
        );
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::InvalidValue,
                "projects[0].depends_on"
            ),
            "混入 {tail} 应记 path=projects[0].depends_on，实际 {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn 项目depends_on非数组落none并记invalid_value() {
    for value in ["{}", r#""x""#, "5", "true"] {
        let report = load_single_project(
            "deps-not-array",
            &format!(r#"{{"root": "web", "depends_on": {value}}}"#),
        );

        assert_eq!(
            report.config.projects.len(),
            1,
            "depends_on={value} 不参与元素去留，元素应仍在案"
        );
        assert!(
            report.config.projects[0].depends_on.is_none(),
            "depends_on={value} 应落 None"
        );
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::InvalidValue,
                "projects[0].depends_on"
            ),
            "depends_on={value} 应记 path=projects[0].depends_on，实际 {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn 两字段同时违例_元素在案且language诊断先于depends_on() {
    let report = load_single_project(
        "two-fields-invalid",
        r#"{"root": "web", "language": 5, "depends_on": [7]}"#,
    );

    assert_eq!(report.config.projects.len(), 1, "仅 root 决定元素去留");
    assert_eq!(report.diagnostics.len(), 2, "实际 {:?}", report.diagnostics);
    assert_eq!(
        report.diagnostics[0].path, "projects[0].language",
        "诊断追加序固定 language 先于 depends_on（DD2）"
    );
    assert_eq!(report.diagnostics[1].path, "projects[0].depends_on");
    for diagnostic in &report.diagnostics {
        assert!(
            kind_rank(&diagnostic.kind) == kind_rank(&DiagnosticKind::InvalidValue),
            "两字段违例均归 InvalidValue，实际 {:?}",
            diagnostic.kind
        );
    }
}

#[test]
fn 项目root非法时两字段合法_整条剔除且只记root一条() {
    for project in [
        r#"{"root": "../escape", "language": "rust", "depends_on": ["a"]}"#,
        r#"{"language": "rust", "depends_on": ["a"]}"#,
    ] {
        let report = load_single_project("root-wins", project);

        assert!(
            report.config.projects.is_empty(),
            "root 缺失 / 非法即整条剔除（两字段不参与去留），project={project}"
        );
        assert_eq!(
            report.diagnostics.len(),
            1,
            "只应记一条 root 诊断（两字段不追加），project={project}，实际 {:?}",
            report.diagnostics
        );
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "projects[0].root"),
            "project={project} 应记 path=projects[0].root"
        );
    }
}

#[test]
fn 三元素混合_全保留且诊断下标对位互不殃及() {
    let ws = TempWs::new("projects-two-fields-mixed");
    ws.ensure_dir();
    ws.write_config(&config_with_projects(
        r#"[
            {"root": "web", "language": "typescript", "depends_on": ["api"]},
            {"root": "api", "language": 5},
            {"root": "svc", "depends_on": [7]}
        ]"#,
    ));

    let report = load(ws.root());

    let roots: Vec<&str> = report
        .config
        .projects
        .iter()
        .map(|project| project.root.as_str())
        .collect();
    assert_eq!(roots, ["web", "api", "svc"], "三条全部保留且保序");
    assert_eq!(report.diagnostics.len(), 2, "实际 {:?}", report.diagnostics);
    assert!(
        has_diag(
            &report,
            &DiagnosticKind::InvalidValue,
            "projects[1].language"
        ),
        "违例下标对位 projects[1].language，实际 {:?}",
        report.diagnostics
    );
    assert!(
        has_diag(
            &report,
            &DiagnosticKind::InvalidValue,
            "projects[2].depends_on"
        ),
        "违例下标对位 projects[2].depends_on，实际 {:?}",
        report.diagnostics
    );
    assert_eq!(
        report.config.projects[0].depends_on.as_deref(),
        Some(&["api".to_owned()][..]),
        "合法条目的两字段不受他条违例殃及"
    );
}

#[test]
fn 两字段合法加未知子键_零诊断() {
    let report = load_single_project(
        "two-fields-unknown-key",
        r#"{"root": "web", "language": "rust", "depends_on": ["a"], "custom": 1}"#,
    );

    assert!(
        report.diagnostics.is_empty(),
        "项目元素未知子键静默忽略，应零诊断，实际 {:?}",
        report.diagnostics
    );
    assert_eq!(report.config.projects[0].language.as_deref(), Some("rust"));
    assert_eq!(
        report.config.projects[0].depends_on.as_deref(),
        Some(&["a".to_owned()][..])
    );
}

#[test]
fn 两新字段四形态自洽不变量_回写再组装零诊断() {
    for project in [
        r#"{"root": "web"}"#,
        r#"{"root": "web", "language": ""}"#,
        r#"{"root": "web", "depends_on": []}"#,
        r#"{"root": "web", "language": "rust", "depends_on": ["a", "b"]}"#,
    ] {
        let config = load_single_project_config("two-fields-consistent", project);
        assert_eq!(config.projects.len(), 1, "project={project}");
        assert_self_consistent(&config);
    }
}

#[test]
fn 两新字段线面形态_null位键恒在且camel_case出线() {
    let value = value_of(&load_single_project_config(
        "two-fields-wire-null",
        r#"{"root": "web"}"#,
    ));
    let project = value["projects"][0]
        .as_object()
        .expect("项目元素线面恒为对象");
    assert!(
        project.contains_key("language"),
        "None 位键恒在（不出线缺键）"
    );
    assert!(
        project.contains_key("dependsOn"),
        "None 位键恒在（不出线缺键）"
    );
    assert_eq!(project["language"], Value::Null, "None 序列化为 null");
    assert_eq!(project["dependsOn"], Value::Null, "None 序列化为 null");
    assert!(
        !project.contains_key("depends_on"),
        "线名为 camelCase（不出线下划线形态）"
    );

    let value = value_of(&load_single_project_config(
        "two-fields-wire-values",
        r#"{"root": "web", "language": "C++", "depends_on": ["a", "b"]}"#,
    ));
    assert_eq!(
        value["projects"][0]["language"],
        json!("C++"),
        "language 原样出线"
    );
    assert_eq!(
        value["projects"][0]["dependsOn"],
        json!(["a", "b"]),
        "depends_on 以 dependsOn 出线且保序"
    );
}

// ---------------------------------------------------------------------------
// load：tests suite（必需字段 / 通配符禁令 / 枚举 / prefault 字段）
// ---------------------------------------------------------------------------

#[test]
fn 合法多suite八框架枚举逐值各一_全部原样保留diagnostics为空() {
    let frameworks = [
        "jest",
        "vitest",
        "vite-plus",
        "bun",
        "rust",
        "node-test",
        "go",
        "pytest",
    ];
    let suites: Vec<String> = frameworks
        .iter()
        .enumerate()
        .map(|(index, framework)| {
            full_suite(
                &format!("pkg-{index}-{framework}"),
                framework,
                r#"{"lines": 80, "branches": 70, "functions": 75}"#,
            )
        })
        .collect();
    let ws = TempWs::new("suites-all-frameworks");
    ws.ensure_dir();
    ws.write_config(&config_with_tests(&suites.join(",")));

    let report = load(ws.root());

    assert!(
        report.diagnostics.is_empty(),
        "实际诊断 {:?}",
        report.diagnostics
    );
    assert_eq!(report.config.tests.len(), 8, "八框架逐值全保留");
    for (suite, framework) in report.config.tests.iter().zip(frameworks) {
        assert_eq!(framework_of(suite), framework);
        assert!(suite.root.ends_with(framework));
    }
}

#[test]
fn tests显式空数组保留且无default_applied_与缺省形态可区分() {
    // 显式 []：prefault 不触发（无 tests 位 DefaultApplied）
    let ws = TempWs::new("tests-explicit-empty");
    ws.ensure_dir();
    ws.write_config(r#"{"schema": "spec-driven", "tests": []}"#);
    let report = load(ws.root());
    assert!(report.config.tests.is_empty());
    assert!(report.diagnostics.is_empty(), "显式 [] 无 DefaultApplied");

    // 缺省：DefaultApplied（path tests）
    let ws = TempWs::new("tests-absent");
    ws.ensure_dir();
    ws.write_config("{}");
    let report = load(ws.root());
    assert!(report.config.tests.is_empty());
    assert!(has_diag(&report, &DiagnosticKind::DefaultApplied, "tests"));
}

#[test]
fn tests非数组与null落空列表并记_invalid_value() {
    for value in ["{}", r#""x""#, "3", "true", "null"] {
        let ws = TempWs::new("tests-not-array");
        ws.ensure_dir();
        ws.write_config(&format!(r#"{{"tests": {value}}}"#));

        let report = load(ws.root());

        assert!(report.config.tests.is_empty());
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "tests"),
            "tests={value} 应记 InvalidValue，实际 {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn suite元素非对象剔除并按索引记_invalid_value() {
    let ws = TempWs::new("suite-not-object");
    ws.ensure_dir();
    ws.write_config(r#"{"tests": ["str", 3, [], null, {"root": "keep", "framework": "bun"}]}"#);

    let report = load(ws.root());

    assert_eq!(report.config.tests.len(), 1, "合法 suite 保留");
    assert_eq!(report.config.tests[0].root, "keep");
    for index in 0..4 {
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::InvalidValue,
                &format!("tests[{index}]")
            ),
            "tests[{index}] 应按索引记 InvalidValue"
        );
    }
}

#[test]
fn suite必需字段root或framework缺失剔除整个suite() {
    let ws = TempWs::new("suite-required-missing");
    ws.ensure_dir();
    ws.write_config(r#"{"tests": [{"framework": "rust"}, {"root": "a"}, {}]}"#);

    let report = load(ws.root());

    assert!(report.config.tests.is_empty());
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].root"
    ));
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[1].framework"
    ));
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[2].root"
    ));
}

#[test]
fn suite_root空串或含通配符剔除整个suite() {
    for root in [
        r#""""#,
        r#""src/**""#,
        r#""a?b""#,
        r#""{a,b}""#,
        r#""x[0]""#,
        // content 为带 JSON 引号的根值（普通转义串，避免 raw 定界符与结尾引号相撞）
        "\"mix*?[{\"",
    ] {
        let ws = TempWs::new("suite-root-wildcard");
        ws.ensure_dir();
        ws.write_config(&format!(
            r#"{{"tests": [{{"root": {root}, "framework": "bun"}}]}}"#
        ));

        let report = load(ws.root());

        assert!(
            report.config.tests.is_empty(),
            "root={root} 应剔除整个 suite"
        );
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "tests[0].root"),
            "root={root} 应记 InvalidValue，实际 {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn suite_root超长换行emoji中文无通配符即合法保留() {
    let long_root = "a".repeat(1200);
    let suites = [
        full_suite(
            &long_root,
            "bun",
            r#"{"lines": 80, "branches": 70, "functions": 75}"#,
        ),
        full_suite(
            "src\\n线",
            "rust",
            r#"{"lines": 80, "branches": 70, "functions": 75}"#,
        ),
        full_suite(
            "目录/🚀",
            "go",
            r#"{"lines": 80, "branches": 70, "functions": 75}"#,
        ),
    ];
    let ws = TempWs::new("suite-root-edge-legal");
    ws.ensure_dir();
    ws.write_config(&config_with_tests(&suites.join(",")));

    let report = load(ws.root());

    assert!(
        report.diagnostics.is_empty(),
        "str 边界合法侧，实际 {:?}",
        report.diagnostics
    );
    assert_eq!(report.config.tests.len(), 3);
    assert_eq!(report.config.tests[0].root, long_root);
    // JSON "\n" 转义解析为真实换行字符
    assert_eq!(report.config.tests[1].root, "src\n线");
    assert_eq!(report.config.tests[2].root, "目录/🚀");
}

#[test]
fn suite_framework枚举外值剔除整个suite() {
    for framework in [
        r#""unknown-fw""#,
        r#""Jest""#,
        r#""VITE-PLUS""#,
        r#""""#,
        "3",
        "null",
        "[]",
    ] {
        let ws = TempWs::new("suite-framework-invalid");
        ws.ensure_dir();
        ws.write_config(&format!(
            r#"{{"tests": [{{"root": "a", "framework": {framework}}}]}}"#
        ));

        let report = load(ws.root());

        assert!(
            report.config.tests.is_empty(),
            "framework={framework} 应剔除整个 suite"
        );
        assert!(has_diag(
            &report,
            &DiagnosticKind::InvalidValue,
            "tests[0].framework"
        ));
    }
}

#[test]
fn suite_cwd缺失吃默认记default_applied_非法吃默认记invalid_value() {
    // 缺失 → "." + DefaultApplied
    let ws = TempWs::new("cwd-defaulted");
    ws.ensure_dir();
    ws.write_config(&format!(r#"{{"tests": [{}]}}"#, minimal_suite("a", "bun")));
    let report = load(ws.root());
    assert_eq!(report.config.tests[0].cwd, ".");
    assert!(has_diag(
        &report,
        &DiagnosticKind::DefaultApplied,
        "tests[0].cwd"
    ));

    // 非字符串 → "." + InvalidValue
    let ws = TempWs::new("cwd-invalid");
    ws.ensure_dir();
    ws.write_config(r#"{"tests": [{"root": "a", "framework": "bun", "cwd": 7}]}"#);
    let report = load(ws.root());
    assert_eq!(report.config.tests[0].cwd, ".");
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].cwd"
    ));

    // 合法原样保留、无 cwd 诊断
    let ws = TempWs::new("cwd-legal");
    ws.ensure_dir();
    ws.write_config(r#"{"tests": [{"root": "a", "framework": "bun", "cwd": "packages/desktop"}]}"#);
    let report = load(ws.root());
    assert_eq!(report.config.tests[0].cwd, "packages/desktop");
    assert!(!has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].cwd"
    ));
    assert!(!has_diag(
        &report,
        &DiagnosticKind::DefaultApplied,
        "tests[0].cwd"
    ));
}

#[test]
fn suite_config空串或非字符串none加诊断_缺失与null无诊断_合法保留() {
    // 空串 / 非字符串 → None + InvalidValue（nonempty 边界）
    for config_value in [r#""""#, "5"] {
        let ws = TempWs::new("suite-config-invalid");
        ws.ensure_dir();
        ws.write_config(&format!(
            r#"{{"tests": [{{"root": "a", "framework": "bun", "config": {config_value}}}]}}"#
        ));

        let report = load(ws.root());

        assert_eq!(report.config.tests[0].config, None);
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "tests[0].config"),
            "config={config_value} 应记 InvalidValue"
        );
    }

    // 合法非空串保留
    let ws = TempWs::new("suite-config-legal");
    ws.ensure_dir();
    ws.write_config(
        r#"{"tests": [{"root": "a", "framework": "bun", "config": "vitest.config.ts"}]}"#,
    );
    let report = load(ws.root());
    assert_eq!(
        report.config.tests[0].config.as_deref(),
        Some("vitest.config.ts")
    );
    assert!(!has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].config"
    ));

    // 缺失 / 显式 null → None 无诊断
    for config_value in ["", "null"] {
        let ws = TempWs::new("suite-config-absent");
        ws.ensure_dir();
        let field = if config_value.is_empty() {
            String::new()
        } else {
            format!(r#", "config": {config_value}"#)
        };
        ws.write_config(&format!(
            r#"{{"tests": [{{"root": "a", "framework": "bun"{field}}}]}}"#
        ));

        let report = load(ws.root());

        assert_eq!(report.config.tests[0].config, None);
        assert!(
            !has_diag(&report, &DiagnosticKind::InvalidValue, "tests[0].config"),
            "config={config_value:?} 无诊断"
        );
    }
}

#[test]
fn suite_includes_excludes类型违例none加诊断_空数组与缺失合法() {
    // includes 非数组
    let ws = TempWs::new("includes-not-array");
    ws.ensure_dir();
    ws.write_config(r#"{"tests": [{"root": "a", "framework": "bun", "includes": "src/**"}]}"#);
    let report = load(ws.root());
    assert_eq!(report.config.tests[0].includes, None);
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].includes"
    ));

    // excludes 元素混入非字符串
    let ws = TempWs::new("excludes-non-string");
    ws.ensure_dir();
    ws.write_config(r#"{"tests": [{"root": "a", "framework": "bun", "excludes": [1]}]}"#);
    let report = load(ws.root());
    assert_eq!(report.config.tests[0].excludes, None);
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].excludes"
    ));

    // 空数组合法保留、无诊断（全字段显式设值，零 prefault 诊断）
    let ws = TempWs::new("includes-empty-legal");
    ws.ensure_dir();
    ws.write_config(&config_with_tests(&full_suite(
        "a",
        "bun",
        r#"{"lines": 80, "branches": 70, "functions": 75}"#,
    )));
    let report = load(ws.root());
    assert!(
        report.diagnostics.is_empty(),
        "实际 {:?}",
        report.diagnostics
    );
    assert_eq!(report.config.tests[0].includes, Some(Vec::new()));
    assert_eq!(report.config.tests[0].excludes, Some(Vec::new()));

    // 缺失 → None 无诊断
    let ws = TempWs::new("includes-absent");
    ws.ensure_dir();
    ws.write_config(&format!(r#"{{"tests": [{}]}}"#, minimal_suite("a", "bun")));
    let report = load(ws.root());
    assert_eq!(report.config.tests[0].includes, None);
    assert_eq!(report.config.tests[0].excludes, None);
    assert!(!has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].includes"
    ));
    assert!(!has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].excludes"
    ));
}

// ---------------------------------------------------------------------------
// load：coverage 阈值（0–100 值域、prefault 80/70/75）
// ---------------------------------------------------------------------------

#[test]
fn coverage缺失三阈值吃默认并逐字段记default_applied() {
    let ws = TempWs::new("coverage-defaulted");
    ws.ensure_dir();
    ws.write_config(&format!(r#"{{"tests": [{}]}}"#, minimal_suite("a", "bun")));

    let report = load(ws.root());

    let coverage = &report.config.tests[0].coverage;
    assert_eq!(coverage.lines, 80.0);
    assert_eq!(coverage.branches, 70.0);
    assert_eq!(coverage.functions, 75.0);
    for key in ["lines", "branches", "functions"] {
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::DefaultApplied,
                &format!("tests[0].coverage.{key}")
            ),
            "tests[0].coverage.{key} 应记 DefaultApplied"
        );
    }
}

#[test]
fn coverage非对象与null三阈值全默认并记invalid_value() {
    for value in ["[80]", "90", r#""x""#, "null"] {
        let ws = TempWs::new("coverage-not-object");
        ws.ensure_dir();
        ws.write_config(&format!(
            r#"{{"tests": [{{"root": "a", "framework": "bun", "coverage": {value}}}]}}"#
        ));

        let report = load(ws.root());

        let coverage = &report.config.tests[0].coverage;
        assert_eq!(coverage.lines, 80.0);
        assert_eq!(coverage.branches, 70.0);
        assert_eq!(coverage.functions, 75.0);
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "tests[0].coverage"),
            "coverage={value} 应记 InvalidValue"
        );
    }
}

#[test]
fn coverage三阈值值域端点与浮点合法保留_浮点忠实出线() {
    let ws = TempWs::new("coverage-endpoints");
    ws.ensure_dir();
    ws.write_config(&config_with_tests(&full_suite(
        "a",
        "bun",
        r#"{"lines": 0, "branches": 100, "functions": 80.5}"#,
    )));

    let report = load(ws.root());

    assert!(report.diagnostics.is_empty());
    let coverage = &report.config.tests[0].coverage;
    assert_eq!(coverage.lines, 0.0);
    assert_eq!(coverage.branches, 100.0);
    assert_eq!(coverage.functions, 80.5, "浮点阈值忠实出线");
}

#[test]
fn coverage阈值越界该字段吃默认记invalid_value_合法同侪不受波及() {
    // （coverage 对象 JSON 片段, 越界字段, 该字段默认值）——lines / branches /
    // functions 三字段各覆盖，值域外侧逐点
    let cases: [(&str, &str, f64); 5] = [
        (r#"{"lines": -1}"#, "lines", 80.0),
        (r#"{"lines": -0.5}"#, "lines", 80.0),
        (r#"{"branches": 100.1}"#, "branches", 70.0),
        (r#"{"functions": 101}"#, "functions", 75.0),
        (r#"{"lines": 200}"#, "lines", 80.0),
    ];
    for (coverage, key, default) in cases {
        let ws = TempWs::new("coverage-out-of-range");
        ws.ensure_dir();
        ws.write_config(&format!(
            r#"{{"tests": [{{"root": "a", "framework": "bun", "coverage": {coverage}}}]}}"#
        ));

        let report = load(ws.root());

        let actual = match key {
            "lines" => report.config.tests[0].coverage.lines,
            "branches" => report.config.tests[0].coverage.branches,
            _ => report.config.tests[0].coverage.functions,
        };
        assert_eq!(actual, default, "coverage={coverage} 的 {key} 应落默认");
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::InvalidValue,
                &format!("tests[0].coverage.{key}")
            ),
            "coverage={coverage} 的 {key} 应逐字段记 InvalidValue"
        );
    }

    // 合法同侪不受波及：lines 越界落默认，branches / functions 显式合法值保留
    let ws = TempWs::new("coverage-mixed");
    ws.ensure_dir();
    ws.write_config(&config_with_tests(
        r#"{"root": "a", "framework": "bun", "cwd": "a", "mutation": {"cwd": null, "score": 70}, "coverage": {"lines": -1, "branches": 88, "functions": 0}}"#,
    ));
    let report = load(ws.root());
    let coverage = &report.config.tests[0].coverage;
    assert_eq!(coverage.lines, 80.0);
    assert_eq!(coverage.branches, 88.0, "合法同侪原样保留");
    assert_eq!(coverage.functions, 0.0, "端点 0 合法保留");
    assert_eq!(report.diagnostics.len(), 1, "仅 lines 一条违例");
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].coverage.lines"
    ));
}

#[test]
fn coverage阈值非数值吃默认并逐字段记invalid_value() {
    let ws = TempWs::new("coverage-non-number");
    ws.ensure_dir();
    ws.write_config(
        r#"{"tests": [{"root": "a", "framework": "bun", "coverage": {"lines": "80", "branches": null, "functions": true}}]}"#,
    );

    let report = load(ws.root());

    let coverage = &report.config.tests[0].coverage;
    assert_eq!(coverage.lines, 80.0);
    assert_eq!(coverage.branches, 70.0);
    assert_eq!(coverage.functions, 75.0);
    for key in ["lines", "branches", "functions"] {
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::InvalidValue,
                &format!("tests[0].coverage.{key}")
            ),
            "非数值 {key} 应记 InvalidValue"
        );
    }
}

// ---------------------------------------------------------------------------
// load：mutation 阈值
// ---------------------------------------------------------------------------

#[test]
fn mutation缺失_score吃默认记default_applied_cwd为none无诊断() {
    let ws = TempWs::new("mutation-defaulted");
    ws.ensure_dir();
    ws.write_config(&format!(r#"{{"tests": [{}]}}"#, minimal_suite("a", "bun")));

    let report = load(ws.root());

    let mutation = &report.config.tests[0].mutation;
    assert_eq!(mutation.cwd, None, "cwd 无默认：缺失为 None");
    assert_eq!(mutation.score, 70.0);
    assert!(
        has_diag(
            &report,
            &DiagnosticKind::DefaultApplied,
            "tests[0].mutation.score"
        ),
        "score 应记 DefaultApplied"
    );
    assert!(
        !has_diag(
            &report,
            &DiagnosticKind::DefaultApplied,
            "tests[0].mutation.cwd"
        ),
        "cwd 缺失无诊断"
    );
}

#[test]
fn mutation非对象与null同走默认处置加invalid_value() {
    for value in ["5", r#""x""#, "[]", "null"] {
        let ws = TempWs::new("mutation-not-object");
        ws.ensure_dir();
        ws.write_config(&format!(
            r#"{{"tests": [{{"root": "a", "framework": "bun", "mutation": {value}}}]}}"#
        ));

        let report = load(ws.root());

        let mutation = &report.config.tests[0].mutation;
        assert_eq!(mutation.cwd, None);
        assert_eq!(mutation.score, 70.0);
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "tests[0].mutation"),
            "mutation={value} 应记 InvalidValue"
        );
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::DefaultApplied,
                "tests[0].mutation.score"
            ),
            "mutation={value} 的 score 同缺失处置记 DefaultApplied"
        );
    }
}

#[test]
fn mutation_score越界或非数值吃默认记invalid_value_cwd非法none加诊断() {
    for score in ["-1", "200", r#""70""#, "null", "true"] {
        let ws = TempWs::new("mutation-score-invalid");
        ws.ensure_dir();
        ws.write_config(&format!(
            r#"{{"tests": [{{"root": "a", "framework": "bun", "mutation": {{"score": {score}}}}}]}}"#
        ));

        let report = load(ws.root());

        assert_eq!(report.config.tests[0].mutation.score, 70.0);
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::InvalidValue,
                "tests[0].mutation.score"
            ),
            "score={score} 应记 InvalidValue"
        );
    }

    // cwd 非字符串 → None + InvalidValue（score 缺失同轮吃默认记 DefaultApplied）
    let ws = TempWs::new("mutation-cwd-invalid");
    ws.ensure_dir();
    ws.write_config(r#"{"tests": [{"root": "a", "framework": "bun", "mutation": {"cwd": 5}}]}"#);
    let report = load(ws.root());
    assert_eq!(report.config.tests[0].mutation.cwd, None);
    assert_eq!(report.config.tests[0].mutation.score, 70.0);
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[0].mutation.cwd"
    ));
    assert!(has_diag(
        &report,
        &DiagnosticKind::DefaultApplied,
        "tests[0].mutation.score"
    ));
}

// ---------------------------------------------------------------------------
// load：write_protection
// ---------------------------------------------------------------------------

#[test]
fn write_protection合法_逐条glob与reason原样保留diagnostics为空() {
    let ws = TempWs::new("protection-legal");
    ws.ensure_dir();
    ws.write_config(
        r#"{"schema": "spec-driven", "tests": [], "write_protection": {"files": [{"glob": "tests/golden/**", "reason": "金样冻结"}, {"glob": "*.lock"}]}}"#,
    );

    let report = load(ws.root());

    assert!(report.diagnostics.is_empty());
    let files = report
        .config
        .write_protection
        .as_ref()
        .expect("保护配置保留")
        .files
        .as_ref()
        .expect("files 保留");
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].glob.as_deref(), Some("tests/golden/**"));
    assert_eq!(files[0].reason.as_deref(), Some("金样冻结"));
    assert_eq!(files[1].glob.as_deref(), Some("*.lock"));
    assert_eq!(files[1].reason, None, "reason 缺失为 None");
}

#[test]
fn write_protection缺失与null无诊断_非对象与files非数组none加invalid_value() {
    // 缺失 / 显式 null → None 无诊断
    for content in [
        r#"{"schema": "spec-driven", "tests": []}"#,
        r#"{"schema": "spec-driven", "tests": [], "write_protection": null}"#,
    ] {
        let ws = TempWs::new("protection-absent");
        ws.ensure_dir();
        ws.write_config(content);
        let report = load(ws.root());
        assert!(report.config.write_protection.is_none());
        assert!(
            report.diagnostics.is_empty(),
            "content={content} 无诊断，实际 {:?}",
            report.diagnostics
        );
    }

    // 非对象 → None + InvalidValue
    for content in [
        r#"{"write_protection": "x"}"#,
        r#"{"write_protection": []}"#,
    ] {
        let ws = TempWs::new("protection-not-object");
        ws.ensure_dir();
        ws.write_config(content);
        let report = load(ws.root());
        assert!(report.config.write_protection.is_none());
        assert!(has_diag(
            &report,
            &DiagnosticKind::InvalidValue,
            "write_protection"
        ));
    }

    // files 非数组 → files None + InvalidValue（path write_protection.files）
    let ws = TempWs::new("protection-files-not-array");
    ws.ensure_dir();
    ws.write_config(r#"{"write_protection": {"files": "x"}}"#);
    let report = load(ws.root());
    let protection = report.config.write_protection.as_ref().expect("本体保留");
    assert!(protection.files.is_none());
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "write_protection.files"
    ));

    // files 显式 null → None 无诊断（null 位语义）
    let ws = TempWs::new("protection-files-null");
    ws.ensure_dir();
    ws.write_config(
        r#"{"schema": "spec-driven", "tests": [], "write_protection": {"files": null}}"#,
    );
    let report = load(ws.root());
    let protection = report.config.write_protection.as_ref().expect("本体保留");
    assert!(protection.files.is_none());
    assert!(report.diagnostics.is_empty());
}

#[test]
fn 写保护规则glob空串或非字符串剔除该条_其余合法条目保留() {
    let ws = TempWs::new("file-rule-invalid");
    ws.ensure_dir();
    ws.write_config(
        r#"{"write_protection": {"files": [
            {"glob": ""},
            {"glob": 3},
            {"glob": "keep/**", "reason": 1},
            {"glob": "ok/**", "reason": "合法理由"}
        ]}}"#,
    );

    let report = load(ws.root());

    let files = report
        .config
        .write_protection
        .as_ref()
        .expect("保护配置保留")
        .files
        .as_ref()
        .expect("files 保留");
    assert_eq!(files.len(), 2, "两条违例规则剔除、两条合法保留");
    assert_eq!(files[0].glob.as_deref(), Some("keep/**"));
    assert_eq!(files[0].reason, None, "reason 非字符串落 None");
    assert_eq!(files[1].glob.as_deref(), Some("ok/**"));
    assert_eq!(files[1].reason.as_deref(), Some("合法理由"));
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "write_protection.files[0].glob"
    ));
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "write_protection.files[1].glob"
    ));
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "write_protection.files[2].reason"
    ));
}

// ---------------------------------------------------------------------------
// load：顶层未知字段 passthrough
// ---------------------------------------------------------------------------

#[test]
fn 顶层未知字段passthrough进extra_已知字段不进() {
    let ws = TempWs::new("passthrough-shapes");
    ws.ensure_dir();
    ws.write_config(
        r#"{"schema": "spec-driven", "$schema": "https://x", "tests": [], "customObject": {"n": 1}, "customArray": [1, 2], "customText": "值", "customNull": null}"#,
    );

    let report = load(ws.root());

    assert!(report.diagnostics.is_empty(), "passthrough 无诊断");
    let keys: Vec<&str> = report.config.extra.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys.len(), 4);
    for key in ["customObject", "customArray", "customText", "customNull"] {
        assert!(keys.contains(&key), "未知字段 {key} 应进 extra");
    }
    assert!(
        !keys.iter().any(|key| *key == "schema" || *key == "$schema"),
        "已知字段不进 extra，实际 {keys:?}"
    );
    let value_of_key = |key: &str| {
        report
            .config
            .extra
            .iter()
            .find(|field| field.key == key)
            .map(|field| field.value.clone())
            .expect("未知字段应在场")
    };
    assert_eq!(value_of_key("customObject"), serde_json::json!({"n": 1}));
    assert_eq!(value_of_key("customArray"), serde_json::json!([1, 2]));
    assert_eq!(value_of_key("customText"), serde_json::json!("值"));
    assert_eq!(value_of_key("customNull"), serde_json::json!(null));
}

#[test]
fn 多个未知字段完整保留且两次加载顺序稳定() {
    let ws = TempWs::new("passthrough-stable");
    ws.ensure_dir();
    ws.write_config(r#"{"zeta": 1, "alpha": 2, "中间": 3, "beta": [true]}"#);

    let first = load(ws.root());
    let second = load(ws.root());

    let keys: Vec<&str> = first.config.extra.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys.len(), 4, "四个未知字段全保留");
    for key in ["zeta", "alpha", "中间", "beta"] {
        assert!(keys.contains(&key));
    }
    let keys_again: Vec<String> = second.config.extra.iter().map(|f| f.key.clone()).collect();
    assert_eq!(
        keys,
        keys_again
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>(),
        "同一输入两次加载 extra 顺序稳定"
    );
}

#[test]
fn 未知字段值为超长字符串或深嵌套对象时原样无损保留() {
    let long = "长".repeat(1200);
    let deep = serde_json::json!({"a": {"b": [1, 2, {"c": true}]}});
    let ws = TempWs::new("passthrough-lossless");
    ws.ensure_dir();
    ws.write_config(
        &serde_json::json!({ "schema": "spec-driven", "tests": [], "long": long, "deep": deep })
            .to_string(),
    );

    let report = load(ws.root());

    assert!(report.diagnostics.is_empty());
    let value_of_key = |key: &str| {
        report
            .config
            .extra
            .iter()
            .find(|field| field.key == key)
            .map(|field| field.value.clone())
            .expect("未知字段应在场")
    };
    assert_eq!(value_of_key("long"), Value::String(long), "超长字符串无损");
    assert_eq!(value_of_key("deep"), deep, "深嵌套对象无损");
}

// ---------------------------------------------------------------------------
// load：自洽不变量与默认基线
// ---------------------------------------------------------------------------

#[test]
fn 违例矩阵抽样产出的config重序列化回写再组装恒零诊断() {
    let ws = TempWs::new("self-consistent-matrix");
    ws.ensure_dir();
    let matrix = [
        // 顶层非对象
        "42",
        // 坏 JSON
        "{ 这不是合法 JSON ]",
        // suite 必需字段违例 + 越界阈值 + cwd / score 非法 混合
        r#"{"tests": [{"root": "bad*", "framework": "bun", "cwd": 5, "coverage": {"lines": 101}, "mutation": {"score": -1}}]}"#,
        // 合法完整配置 + 未知字段
        r#"{"schema": "spec-driven", "context": "上下文", "rules": {"proposal": ["规则"]}, "projects": [{"root": "packages/desktop"}], "tests": [{"root": "pkg", "framework": "vite-plus", "coverage": {"lines": 90.5, "branches": 80, "functions": 85}}], "write_protection": {"files": [{"glob": "tests/golden/**", "reason": "金样冻结"}]}, "custom": {"n": 1}}"#,
    ];
    for content in matrix {
        ws.write_config(content);
        let report = load(ws.root());
        assert_self_consistent(&report.config);
    }

    // 文件缺失分支同过不变量
    ws.remove_config();
    let report = load(ws.root());
    assert!(has_diag(&report, &DiagnosticKind::FileMissing, "$"));
    assert_self_consistent(&report.config);
}

#[test]
fn 默认基线逐字段对位且过assemble零诊断() {
    let config = WorkspaceConfig::default();

    assert!(config.schema_ref.is_none(), "默认 $schema 未设");
    assert_eq!(config.schema, "spec-driven");
    assert!(config.context.is_none());
    assert!(config.rules.is_none());
    assert!(config.static_analysis.is_none());
    assert!(config.projects.is_empty(), "默认 projects 为空列表");
    assert!(config.tests.is_empty(), "默认 tests 为空列表");
    assert!(config.write_protection.is_none());
    assert!(config.extra.is_empty(), "默认 extra 为空");

    assert_self_consistent(&config);
}

#[test]
fn 两个suite一违一合_违例剔除记诊断_合法保序保留不殃及() {
    let ws = TempWs::new("suite-isolation");
    ws.ensure_dir();
    ws.write_config(
        r#"{"tests": [{"root": "ok-pkg", "framework": "bun"}, {"root": "bad*", "framework": "bun"}]}"#,
    );

    let report = load(ws.root());

    assert_eq!(report.config.tests.len(), 1, "仅合法 suite 保留");
    assert_eq!(report.config.tests[0].root, "ok-pkg");
    assert!(has_diag(
        &report,
        &DiagnosticKind::InvalidValue,
        "tests[1].root"
    ));
    assert!(
        !has_diag(&report, &DiagnosticKind::InvalidValue, "tests[0].root"),
        "剔除处置不殃及合法 suite"
    );
}

// ---------------------------------------------------------------------------
// schema 镜像 ↔ 实现一致性（A1 键集 / A2 枚举 / A3 往返 + wire_name 编译期守卫）
//
// 镜像以 `include_str!` 编译期绑定入仓 `../config.schema.json`：零运行时 IO、
// 零 cwd 依赖、零路径拼装。三个断言体均以 `&Value` 入参化（正向行喂入仓
// 镜像，异常 / 边界行喂内存副本），`wire_name` 与断言体同属测试内私有项，
// 不作生产导出（CLAUDE.md no-test-only-exports）。
// ---------------------------------------------------------------------------

/// 入仓 schema 镜像（AC-4）：编译期字面量绑定真实入仓文件。
fn mirror() -> Value {
    serde_json::from_str::<Value>(include_str!("../config.schema.json"))
        .expect("入仓 config.schema.json 必须可解析为 JSON")
}

/// 入仓镜像的内存副本：`mirror()` 每次调用独立解析出全新的 `Value`（无别名），
/// 异常 / 边界行在副本上就地改写目标字段，磁盘文件本体零改动（镜像变体不以
/// mock 框架实现），也不需要 `to_value` 往返或多余克隆。
fn mirror_copy() -> Value {
    mirror()
}

/// 镜像根对象（类型违例给出可指认文案，不越界 unwrap）。
fn mirror_root(schema: &Value) -> &Map<String, Value> {
    match schema {
        Value::Object(map) => map,
        other => panic!("schema 镜像根类型违例：期望对象，实际 {other}"),
    }
}

/// 镜像 `properties` 对象（缺失 / `null` 与类型违例分文案）。
fn mirror_properties(schema: &Value) -> &Map<String, Value> {
    match mirror_root(schema).get("properties") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) => panic!("镜像缺 `properties`：期望对象，实际 null"),
        Some(other) => panic!("镜像 `properties` 类型违例：期望对象，实际 {other}"),
        None => panic!("镜像缺 `properties`：顶层未取到该键"),
    }
}

/// 镜像 `properties.tests.items.properties.framework.enum` 数组（路径逐段取用，
/// 缺失 / `null` 与类型违例分文案）。
fn mirror_framework_enum(schema: &Value) -> &Vec<Value> {
    let properties = mirror_properties(schema);
    let suite = match properties.get("tests") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) | None => panic!("镜像缺 `tests`：期望对象"),
        Some(other) => panic!("镜像 `tests` 类型违例：期望对象，实际 {other}"),
    };
    let items = match suite.get("items") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) | None => panic!("镜像缺 `tests.items`：期望对象"),
        Some(other) => panic!("镜像 `tests.items` 类型违例：期望对象，实际 {other}"),
    };
    let suite_properties = match items.get("properties") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) | None => panic!("镜像缺 `tests.items.properties`：期望对象"),
        Some(other) => panic!("镜像 `tests.items.properties` 类型违例：期望对象，实际 {other}"),
    };
    let framework = match suite_properties.get("framework") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) | None => panic!("镜像缺 `tests.items.properties.framework`：期望对象"),
        Some(other) => {
            panic!("镜像 `tests.items.properties.framework` 类型违例：期望对象，实际 {other}")
        }
    };
    match framework.get("enum") {
        Some(Value::Array(values)) => values,
        Some(Value::Null) => {
            panic!("镜像缺 `enum`：`tests.items.properties.framework.enum` 为 null")
        }
        Some(other) => panic!("镜像 `enum` 类型违例：期望数组，实际 {other}"),
        None => panic!("镜像缺 `enum`：`tests.items.properties.framework` 未取到该键"),
    }
}

/// 副本内 `properties` 对象的可变借用（异常 / 边界行就地改写）。
fn properties_mut(schema: &mut Value) -> &mut Map<String, Value> {
    schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("入仓镜像 `properties` 必须为对象")
}

/// 副本内 `properties.tests.items.properties.framework` 对象的可变借用。
fn framework_mut(schema: &mut Value) -> &mut Map<String, Value> {
    properties_mut(schema)
        .get_mut("tests")
        .and_then(Value::as_object_mut)
        .and_then(|suite| suite.get_mut("items"))
        .and_then(Value::as_object_mut)
        .and_then(|items| items.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut("framework"))
        .and_then(Value::as_object_mut)
        .expect("入仓镜像 `framework` 节点必须为对象")
}

/// 副本内 `framework.enum` 数组的可变借用。
fn framework_enum_mut(schema: &mut Value) -> &mut Vec<Value> {
    framework_mut(schema)
        .get_mut("enum")
        .and_then(Value::as_array_mut)
        .expect("入仓镜像 `framework.enum` 必须为数组")
}

/// 镜像 `properties.projects.items` 对象（缺失 / `null` 与类型违例分文案）。
fn mirror_project_item(schema: &Value) -> &Map<String, Value> {
    let projects = match mirror_properties(schema).get("projects") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) => panic!("镜像缺 `projects`：期望对象，实际 null"),
        Some(other) => panic!("镜像 `projects` 类型违例：期望对象，实际 {other}"),
        None => panic!("镜像缺 `properties.projects`：未取到该键"),
    };
    match projects.get("items") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) => panic!("镜像缺 `projects.items`：期望对象，实际 null"),
        Some(other) => panic!("镜像 `projects.items` 类型违例：期望对象，实际 {other}"),
        None => panic!("镜像缺 `properties.projects.items`：未取到该键"),
    }
}

/// 镜像 `properties.projects.items.properties` 对象（缺失 / `null` 与类型违例分文案）。
fn project_item_properties(schema: &Value) -> &Map<String, Value> {
    match mirror_project_item(schema).get("properties") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) => panic!("镜像缺 `projects.items.properties`：期望对象，实际 null"),
        Some(other) => panic!("镜像 `projects.items.properties` 类型违例：期望对象，实际 {other}"),
        None => panic!("镜像缺 `projects.items.properties`：未取到该键"),
    }
}

/// 镜像 `properties.projects.items.required` 数组（缺失 / `null` 与类型违例分文案）。
fn project_item_required(schema: &Value) -> &Vec<Value> {
    match mirror_project_item(schema).get("required") {
        Some(Value::Array(values)) => values,
        Some(Value::Null) => panic!("镜像缺 `projects.items.required`：期望数组，实际 null"),
        Some(other) => panic!("镜像 `projects.items.required` 类型违例：期望数组，实际 {other}"),
        None => panic!("镜像缺 `projects.items.required`：未取到该键"),
    }
}

/// 项目元素单属性节点（缺失 / `null` 与类型违例分文案，不越界 `unwrap`）。
fn project_item_field<'a>(properties: &'a Map<String, Value>, key: &str) -> &'a Map<String, Value> {
    match properties.get(key) {
        Some(Value::Object(map)) => map,
        Some(Value::Null) => {
            panic!("镜像 `projects.items.properties.{key}` 类型违例：期望对象，实际 null")
        }
        Some(other) => {
            panic!("镜像 `projects.items.properties.{key}` 类型违例：期望对象，实际 {other}")
        }
        None => panic!("镜像缺 `projects.items.properties.{key}`：未取到该键"),
    }
}

/// 副本内 `properties.projects.items` 对象的可变借用（异常 / 边界行就地改写）。
fn mirror_project_item_mut(schema: &mut Value) -> &mut Map<String, Value> {
    schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut("projects"))
        .and_then(Value::as_object_mut)
        .and_then(|projects| projects.get_mut("items"))
        .and_then(Value::as_object_mut)
        .expect("入仓镜像 `properties.projects.items` 必须为对象")
}

/// 副本内 `properties.projects.items.properties` 对象的可变借用。
fn project_item_properties_mut(schema: &mut Value) -> &mut Map<String, Value> {
    mirror_project_item_mut(schema)
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("入仓镜像 `properties.projects.items.properties` 必须为对象")
}

/// 副本内项目元素单属性节点的可变借用（异常 / 边界行就地改写）。
fn project_item_field_mut<'a>(schema: &'a mut Value, key: &str) -> &'a mut Map<String, Value> {
    project_item_properties_mut(schema)
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("入仓镜像 `projects.items.properties.{key}` 必须为对象"))
}

/// AC-6 项目元素镜像断言：键集恰 `root` / `language` / `depends_on` 三键、
/// `required` 仅 `root`、两新属性只带 `type` + `description` 而不含严于实现的
/// 约束（`minLength` / `uniqueItems` / `enum` 等）；失败文案指认具体缺失 /
/// 越界项。
fn assert_project_item_mirror(schema: &Value) {
    let properties = project_item_properties(schema);
    let mut keys: Vec<&str> = properties.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        ["depends_on", "language", "root"],
        "项目元素镜像键集失配：期望 root / language / depends_on 三键（排序后比较），实际 {keys:?}"
    );

    let required: Vec<&str> = project_item_required(schema)
        .iter()
        .map(|value| match value {
            Value::String(name) => name.as_str(),
            other => {
                panic!("镜像 `projects.items.required` 元素类型违例：期望字符串，实际 {other}")
            }
        })
        .collect();
    assert_eq!(
        required,
        ["root"],
        "required 应仅含 root（language / depends_on 为可选字段），实际 {required:?}"
    );

    let language = project_item_field(properties, "language");
    assert_eq!(
        language.get("type"),
        Some(&json!("string")),
        "`language` 镜像 type 应为 string，实际 {:?}",
        language.get("type")
    );
    for constraint in [
        "minLength",
        "maxLength",
        "enum",
        "pattern",
        "default",
        "const",
    ] {
        assert!(
            !language.contains_key(constraint),
            "`language` 镜像不得含 `{constraint}` 约束（镜像不得比实现严，DD3）"
        );
    }

    let depends_on = project_item_field(properties, "depends_on");
    assert_eq!(
        depends_on.get("type"),
        Some(&json!("array")),
        "`depends_on` 镜像 type 应为 array，实际 {:?}",
        depends_on.get("type")
    );
    let items = match depends_on.get("items") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) => panic!("镜像缺 `depends_on.items`：期望对象，实际 null"),
        Some(other) => panic!("镜像 `depends_on.items` 类型违例：期望对象，实际 {other}"),
        None => panic!("镜像缺 `projects.items.properties.depends_on.items`：未取到该键"),
    };
    assert_eq!(
        items.get("type"),
        Some(&json!("string")),
        "`depends_on.items.type` 应为 string，实际 {:?}",
        items.get("type")
    );
    for constraint in ["uniqueItems", "minItems", "maxItems", "default", "const"] {
        assert!(
            !depends_on.contains_key(constraint),
            "`depends_on` 镜像不得含 `{constraint}` 约束（镜像不得比实现严，DD3）"
        );
    }
    let description = match depends_on.get("description") {
        Some(Value::String(text)) => text,
        Some(Value::Null) => panic!("镜像 `depends_on.description` 为 null"),
        Some(other) => {
            panic!("镜像 `depends_on.description` 类型违例：期望字符串，实际 {other}")
        }
        None => panic!("镜像缺 `projects.items.properties.depends_on.description`"),
    };
    assert!(
        !description.is_empty(),
        "`depends_on.description` 应为非空提示文案"
    );
    assert!(
        description.contains("值宜为"),
        "`depends_on.description` 应为提示文案（含「值宜为」而非强制约束措辞），实际 {description:?}"
    );
}

/// A1 键集断言：`properties` 键集（排序去重）== `KNOWN_KEYS`（排序去重）；
/// 失败文案指认双向差异。
fn assert_key_set(schema: &Value) {
    let mut mirror_keys: Vec<String> = mirror_properties(schema).keys().cloned().collect();
    mirror_keys.sort();
    mirror_keys.dedup();
    let mut known_keys: Vec<String> = KNOWN_KEYS.iter().map(|key| (*key).to_string()).collect();
    known_keys.sort();
    known_keys.dedup();

    let only_schema: Vec<&String> = mirror_keys
        .iter()
        .filter(|key| !known_keys.contains(key))
        .collect();
    let only_impl: Vec<&String> = known_keys
        .iter()
        .filter(|key| !mirror_keys.contains(key))
        .collect();

    assert!(
        only_schema.is_empty() && only_impl.is_empty(),
        "schema 镜像键集与 KNOWN_KEYS 漂移：仅 schema 有: {only_schema:?}；仅实现有: {only_impl:?}"
    );
}

/// 八变体按 `TestFramework` 声明序（= wire 序）返回。
fn framework_variants() -> [TestFramework; 8] {
    [
        TestFramework::Jest,
        TestFramework::Vitest,
        TestFramework::VitePlus,
        TestFramework::Bun,
        TestFramework::Rust,
        TestFramework::NodeTest,
        TestFramework::Go,
        TestFramework::Pytest,
    ]
}

/// 变体 → wire 串（实现 → schema 方向）。
///
/// **穷尽 match 即编译期守卫**：`TestFramework` 新增第 9 变体而未补臂时，
/// `cargo test --no-run` / `cargo clippy --workspace --all-targets` 即编译失败，
/// 早于任何运行时断言（该判据不设运行时用例）。
fn wire_name(framework: &TestFramework) -> &'static str {
    match framework {
        TestFramework::Jest => "jest",
        TestFramework::Vitest => "vitest",
        TestFramework::VitePlus => "vite-plus",
        TestFramework::Bun => "bun",
        TestFramework::Rust => "rust",
        TestFramework::NodeTest => "node-test",
        TestFramework::Go => "go",
        TestFramework::Pytest => "pytest",
    }
}

/// A2 枚举断言：镜像 `framework.enum`（8 串，同序）== 八变体按声明序的
/// `wire_name`；集合差异优先指认（增 / 删 / 改串），集合相同而异序则指认
/// 顺序漂移（MUST NOT 静默排序后比较）。
fn assert_framework_enum(schema: &Value) {
    let mut mirror_strings: Vec<String> = Vec::new();
    for value in mirror_framework_enum(schema) {
        match value {
            Value::String(text) => mirror_strings.push(text.clone()),
            other => panic!("镜像 `enum` 元素类型违例：期望字符串，实际 {other}"),
        }
    }
    let impl_strings: Vec<&str> = framework_variants().iter().map(wire_name).collect();

    let mut only_schema: Vec<&str> = Vec::new();
    for text in &mirror_strings {
        if !impl_strings.contains(&text.as_str()) {
            only_schema.push(text.as_str());
        }
    }
    let mut only_impl: Vec<&str> = Vec::new();
    for impl_text in &impl_strings {
        if !mirror_strings
            .iter()
            .any(|text| text.as_str() == *impl_text)
        {
            only_impl.push(*impl_text);
        }
    }
    assert!(
        only_schema.is_empty() && only_impl.is_empty(),
        "schema 镜像 framework 枚举与实现漂移：仅 schema 有: {only_schema:?}；仅实现有: {only_impl:?}"
    );

    let mirror_order: Vec<&str> = mirror_strings.iter().map(String::as_str).collect();
    assert_eq!(
        mirror_order, impl_strings,
        "schema 镜像 framework 枚举与 TestFramework 声明序存在顺序漂移（同序比较，不静默排序）"
    );
}

/// A3 往返断言：`enum` 每串经 `parse_framework` 解回变体、再 `wire_name` 必回
/// 同串（两方向闭合）；失败文案含首个不往返的串与解回结果。
fn assert_framework_roundtrip(schema: &Value) {
    for value in mirror_framework_enum(schema) {
        let text = match value {
            Value::String(text) => text,
            other => panic!("镜像 `enum` 元素类型违例：期望字符串，实际 {other}"),
        };
        let parsed = parse_framework(text);
        let back = parsed.as_ref().map(wire_name);
        assert!(
            back == Some(text.as_str()),
            "schema 镜像 framework 枚举串不往返：{text:?} 解回 {parsed:?}、再 wire 得 {back:?}"
        );
    }
}

/// 期待断言体 panic 并取回 panic 文案（文案本身是判据的一部分：双向差异 /
/// 首个不往返项，故不能只断言「是否 panic」）。
fn panic_text(body: impl FnOnce()) -> String {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(()) => panic!("期望一致性断言失败 panic，实际通过"),
        Err(payload) => match payload.downcast_ref::<String>() {
            Some(text) => text.clone(),
            None => match payload.downcast_ref::<&'static str>() {
                Some(text) => (*text).to_string(),
                None => panic!("panic 载荷既非 String 也非 &str，无法核对文案"),
            },
        },
    }
}

#[test]
fn 入仓镜像可解析为对象且顶层四键齐备() {
    let schema = mirror();

    let root = mirror_root(&schema);
    for key in ["$schema", "type", "properties", "additionalProperties"] {
        assert!(root.contains_key(key), "镜像顶层缺 `{key}`");
    }
    assert_eq!(
        root.get("type"),
        Some(&json!("object")),
        "镜像顶层 type 应为 object"
    );
}

#[test]
fn 镜像结构四锚点与设计表一致() {
    let schema = mirror();
    let root = mirror_root(&schema);
    let properties = mirror_properties(&schema);

    let properties_key_count = properties.len();
    let additional_is_empty_object = root.get("additionalProperties") == Some(&json!({}));
    let schema_const_and_default = {
        let node = properties
            .get("schema")
            .expect("镜像缺 `properties.schema`");
        node.get("const") == Some(&json!("spec-driven"))
            && node.get("default") == Some(&json!("spec-driven"))
    };
    let required_has_pair = {
        let required = properties
            .get("tests")
            .and_then(|tests| tests.get("items"))
            .and_then(|items| items.get("required"))
            .and_then(Value::as_array)
            .expect("镜像缺 `properties.tests.items.required` 数组");
        let has = |name: &str| required.iter().any(|value| value == &json!(name));
        has("root") && has("framework")
    };

    let anchors: [(&str, bool); 4] = [
        (
            "`properties` 恰 8 键（与 KNOWN_KEYS 同规模）",
            properties_key_count == 8,
        ),
        (
            "`additionalProperties` 为空对象（passthrough 表达）",
            additional_is_empty_object,
        ),
        (
            "`properties.schema` 的 `const` 与 `default` 同为 spec-driven",
            schema_const_and_default,
        ),
        (
            "`properties.tests.items.required` 含 root 与 framework",
            required_has_pair,
        ),
    ];
    for (anchor, ok) in anchors {
        assert!(ok, "镜像结构锚点失配：{anchor}");
    }
}

// ---------------------------------------------------------------------------
// schema 镜像 projects.items（协议扩展两属性，AC-6）
// ---------------------------------------------------------------------------

#[test]
fn 项目元素镜像三键集与required仅root正向通过() {
    let schema = mirror();

    assert_project_item_mirror(&schema);

    let mut keys: Vec<&str> = project_item_properties(&schema)
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        ["depends_on", "language", "root"],
        "项目元素镜像键集应恰为三键（root 之后增两属性）"
    );
    assert_eq!(
        project_item_required(&schema),
        &vec![json!("root")],
        "required 应仍只含 root（两新键不入 required）"
    );
}

#[test]
fn 项目元素镜像language只带type无严于实现的约束() {
    let schema = mirror();
    let language = project_item_field(project_item_properties(&schema), "language");

    assert_eq!(
        language.get("type"),
        Some(&json!("string")),
        "`language` 镜像 type 应为 string，实际 {:?}",
        language.get("type")
    );
    assert!(
        language.contains_key("description"),
        "`language` 镜像应带 description 提示文案"
    );
    for constraint in [
        "minLength",
        "maxLength",
        "enum",
        "pattern",
        "default",
        "const",
        "minimum",
        "maximum",
    ] {
        assert!(
            !language.contains_key(constraint),
            "`language` 镜像不得含 `{constraint}`（镜像不得严于实现的零校验语义）"
        );
    }
}

#[test]
fn 项目元素镜像depends_on为字符串数组且带值宜为提示文案() {
    let schema = mirror();
    let depends_on = project_item_field(project_item_properties(&schema), "depends_on");

    assert_eq!(
        depends_on.get("type"),
        Some(&json!("array")),
        "`depends_on` 镜像 type 应为 array"
    );
    let items = depends_on
        .get("items")
        .and_then(Value::as_object)
        .expect("`depends_on.items` 应为对象");
    assert_eq!(
        items.get("type"),
        Some(&json!("string")),
        "`depends_on.items.type` 应为 string"
    );
    for constraint in ["uniqueItems", "minItems", "maxItems", "default", "const"] {
        assert!(
            !depends_on.contains_key(constraint),
            "`depends_on` 镜像不得含 `{constraint}`（保序 / 不去重 / 零引用校验）"
        );
    }
    let description = depends_on
        .get("description")
        .and_then(Value::as_str)
        .expect("`depends_on.description` 应为字符串");
    assert!(
        description.contains("值宜为"),
        "`depends_on.description` 应为提示文案而非强制约束措辞，实际 {description:?}"
    );
}

#[test]
fn 项目元素镜像缺键与required越界三副本被抓且文案指认() {
    // 副本一：删除 language
    let mut copy = mirror_copy();
    project_item_properties_mut(&mut copy).remove("language");
    let text = panic_text(|| assert_project_item_mirror(&copy));
    assert!(
        text.contains("language") && text.contains("键集"),
        "删 language 的副本应被键集断言抓且指认缺键，实际文案 {text:?}"
    );

    // 副本二：删除 depends_on
    let mut copy = mirror_copy();
    project_item_properties_mut(&mut copy).remove("depends_on");
    let text = panic_text(|| assert_project_item_mirror(&copy));
    assert!(
        text.contains("depends_on") && text.contains("键集"),
        "删 depends_on 的副本应被键集断言抓且指认缺键，实际文案 {text:?}"
    );

    // 副本三：两新键写进 required（越界）
    let mut copy = mirror_copy();
    let item = mirror_project_item_mut(&mut copy);
    item.insert(
        "required".to_owned(),
        json!(["root", "language", "depends_on"]),
    );
    let text = panic_text(|| assert_project_item_mirror(&copy));
    assert!(
        text.contains("required"),
        "两新键进 required 的副本应被 required 断言抓，实际文案 {text:?}"
    );
}

#[test]
fn 项目元素镜像类型违例两副本被抓且不越界unwrap() {
    // 副本一：language 写成对象（缺 type）
    let mut copy = mirror_copy();
    project_item_field_mut(&mut copy, "language").insert("type".to_owned(), json!({ "a": 1 }));
    let text = panic_text(|| assert_project_item_mirror(&copy));
    assert!(
        text.contains("language") && text.contains("type"),
        "language 类型漂移的副本应给出可指认文案，实际 {text:?}"
    );

    // 副本二：depends_on.items 被取走
    let mut copy = mirror_copy();
    project_item_field_mut(&mut copy, "depends_on").remove("items");
    let text = panic_text(|| assert_project_item_mirror(&copy));
    assert!(
        text.contains("depends_on.items"),
        "取走 depends_on.items 的副本应给出可指认文案（不越界 unwrap），实际 {text:?}"
    );
}

#[test]
fn 项目元素镜像属性顺序无关且language为null归类型违例() {
    // 边界一：副本按倒序重建三属性 → 排序后比较仍通过（比较与入仓顺序无关）
    let mut copy = mirror_copy();
    let properties = project_item_properties_mut(&mut copy);
    let root = properties.remove("root").expect("入仓镜像应有 root");
    let language = properties
        .remove("language")
        .expect("入仓镜像应有 language");
    let depends_on = properties
        .remove("depends_on")
        .expect("入仓镜像应有 depends_on");
    properties.insert("depends_on".to_owned(), depends_on);
    properties.insert("language".to_owned(), language);
    properties.insert("root".to_owned(), root);
    assert_project_item_mirror(&copy);

    // 边界二：language 为 null → 归类型违例文案
    let mut copy = mirror_copy();
    project_item_properties_mut(&mut copy).insert("language".to_owned(), Value::Null);
    let text = panic_text(|| assert_project_item_mirror(&copy));
    assert!(
        text.contains("类型违例"),
        "language 为 null 的副本应归类型违例文案，实际 {text:?}"
    );
}

#[test]
fn 键集断言_入仓镜像正向通过且八键逐项对位() {
    let schema = mirror();

    assert_key_set(&schema);

    let mut mirror_keys: Vec<&str> = mirror_properties(&schema)
        .keys()
        .map(String::as_str)
        .collect();
    mirror_keys.sort();
    assert_eq!(
        mirror_keys,
        [
            "$schema",
            "context",
            "projects",
            "rules",
            "schema",
            "static_analysis",
            "tests",
            "write_protection",
        ],
        "镜像键集应为 8 键全集"
    );

    // 与 `KNOWN_KEYS` 的对位由 `assert_key_set` 承担（上方调用），此处只钉字面量
    // 8 键全集，避免同一键集比较写两遍。
}

#[test]
fn 键集断言_缺键漂移被抓且文案指认双向差异() {
    let mut schema = mirror_copy();
    properties_mut(&mut schema).remove("tests");

    let text = panic_text(|| assert_key_set(&schema));

    assert!(
        text.contains("KNOWN_KEYS"),
        "失败文案应指认 KNOWN_KEYS，实际 {text}"
    );
    assert!(
        text.contains("仅实现有: [\"tests\"]"),
        "失败文案应指认缺键，实际 {text}"
    );
    assert!(
        text.contains("仅 schema 有: []"),
        "缺键形态下 schema 侧无独有键，实际 {text}"
    );
}

#[test]
fn 键集断言_多键漂移被抓且文案指认单向差异() {
    let mut schema = mirror_copy();
    properties_mut(&mut schema).insert("extra".to_string(), json!({"type": "string"}));

    let text = panic_text(|| assert_key_set(&schema));

    assert!(
        text.contains("仅 schema 有: [\"extra\"]"),
        "失败文案应指认多键，实际 {text}"
    );
    assert!(
        text.contains("仅实现有: []"),
        "多键形态下实现侧无独有键，实际 {text}"
    );
}

#[test]
fn 键集断言_容器四形态边界() {
    // 形态 1：`properties: {}`（空对象）—— 8 项全落「仅实现有」。
    let mut empty_properties = mirror_copy();
    *empty_properties
        .get_mut("properties")
        .expect("镜像缺 `properties`") = json!({});
    let text = panic_text(|| assert_key_set(&empty_properties));
    assert!(
        text.contains("仅 schema 有: []"),
        "空对象形态下 schema 侧无独有键，实际 {text}"
    );
    for key in KNOWN_KEYS {
        assert!(
            text.contains(key),
            "空对象形态应把 `{key}` 全列入「仅实现有」，实际 {text}"
        );
    }

    // 形态 2：`properties` 缺失 / `null` —— 文案指认镜像缺 `properties`，
    // MUST NOT 越界 unwrap。
    let mut missing_properties = mirror_copy();
    missing_properties
        .get_mut("properties")
        .expect("镜像缺 `properties`")
        .take();
    let text = panic_text(|| assert_key_set(&missing_properties));
    assert!(
        text.contains("镜像缺 `properties`"),
        "缺失形态应指认缺键而非越界 unwrap，实际 {text}"
    );

    let mut null_properties = mirror_copy();
    *null_properties
        .get_mut("properties")
        .expect("镜像缺 `properties`") = Value::Null;
    let text = panic_text(|| assert_key_set(&null_properties));
    assert!(
        text.contains("镜像缺 `properties`"),
        "null 形态应指认缺键而非越界 unwrap，实际 {text}"
    );

    // 形态 3：`properties` 为数组 / 字符串（非对象）—— 文案指认类型违例。
    let mut array_properties = mirror_copy();
    array_properties["properties"] = json!([{"tests": {}}]);
    let text = panic_text(|| assert_key_set(&array_properties));
    assert!(
        text.contains("类型违例"),
        "数组形态应指认类型违例，实际 {text}"
    );

    let mut string_properties = mirror_copy();
    string_properties["properties"] = json!("tests");
    let text = panic_text(|| assert_key_set(&string_properties));
    assert!(
        text.contains("类型违例"),
        "字符串形态应指认类型违例，实际 {text}"
    );

    // 形态 4：键序打乱（`write_protection` 挪位）—— 排序后比较、顺序无关 → 通过。
    let mut reordered = mirror_copy();
    let write_protection = properties_mut(&mut reordered)
        .remove("write_protection")
        .expect("镜像缺 `write_protection`");
    properties_mut(&mut reordered).insert("write_protection".to_string(), write_protection);
    assert_key_set(&reordered);
}

#[test]
fn 框架枚举断言_入仓镜像八值同序且与serde线面对位() {
    let schema = mirror();

    assert_framework_enum(&schema);

    for framework in framework_variants() {
        let rename = match serde_json::to_value(&framework).expect("framework 序列化失败") {
            Value::String(text) => text,
            other => panic!("framework 线面应为字符串，实际 {other}"),
        };
        assert_eq!(
            wire_name(&framework),
            rename,
            "wire_name 应与 `#[serde(rename)]` 串逐字相等"
        );
    }
}

#[test]
fn 框架枚举断言_串漂移与增删三形态被抓() {
    // 形态 1：改串 `jest` → `jest2`。
    let mut renamed = mirror_copy();
    framework_enum_mut(&mut renamed)[0] = json!("jest2");
    let text = panic_text(|| assert_framework_enum(&renamed));
    assert!(
        text.contains("仅 schema 有: [\"jest2\"]"),
        "改串形态应指认 schema 侧独有串，实际 {text}"
    );
    assert!(
        text.contains("仅实现有: [\"jest\"]"),
        "改串形态应指认实现侧独有串，实际 {text}"
    );

    // 形态 2：删 `pytest`（7 值）。
    let mut dropped = mirror_copy();
    let last = framework_enum_mut(&mut dropped)
        .pop()
        .expect("镜像枚举非空");
    assert_eq!(last, json!("pytest"), "末位应为 pytest");
    let text = panic_text(|| assert_framework_enum(&dropped));
    assert!(
        text.contains("仅实现有: [\"pytest\"]"),
        "删值形态应指认缺串，实际 {text}"
    );
    assert!(
        text.contains("仅 schema 有: []"),
        "删值形态下 schema 侧无独有串，实际 {text}"
    );

    // 形态 3：追加 `vite`（9 值）。
    let mut appended = mirror_copy();
    framework_enum_mut(&mut appended).push(json!("vite"));
    let text = panic_text(|| assert_framework_enum(&appended));
    assert!(
        text.contains("仅 schema 有: [\"vite\"]"),
        "多值形态应指认多串，实际 {text}"
    );
    assert!(
        text.contains("仅实现有: []"),
        "多值形态下实现侧无独有串，实际 {text}"
    );
}

#[test]
fn 框架枚举断言_容器四形态边界() {
    // 形态 1：`enum: []`（空数组）—— 8 变体全落「仅实现有」。
    let mut empty_enum = mirror_copy();
    framework_enum_mut(&mut empty_enum).clear();
    let text = panic_text(|| assert_framework_enum(&empty_enum));
    assert!(
        text.contains("仅 schema 有: []"),
        "空数组形态下 schema 侧无独有串，实际 {text}"
    );
    for name in [
        "jest",
        "vitest",
        "vite-plus",
        "bun",
        "rust",
        "node-test",
        "go",
        "pytest",
    ] {
        assert!(
            text.contains(name),
            "空数组形态应把 `{name}` 全列入「仅实现有」，实际 {text}"
        );
    }

    // 形态 2：`enum` 缺失 / `null` —— 文案指认镜像缺 `enum`。
    let mut missing_enum = mirror_copy();
    framework_mut(&mut missing_enum).remove("enum");
    let text = panic_text(|| assert_framework_enum(&missing_enum));
    assert!(
        text.contains("镜像缺 `enum`"),
        "缺失形态应指认缺 `enum`，实际 {text}"
    );

    let mut null_enum = mirror_copy();
    framework_mut(&mut null_enum).insert("enum".to_string(), Value::Null);
    let text = panic_text(|| assert_framework_enum(&null_enum));
    assert!(
        text.contains("镜像缺 `enum`"),
        "null 形态应指认缺 `enum`，实际 {text}"
    );

    // 形态 3：`enum` 为字符串 `"jest"`（非数组）—— 文案指认类型违例。
    let mut string_enum = mirror_copy();
    framework_mut(&mut string_enum).insert("enum".to_string(), json!("jest"));
    let text = panic_text(|| assert_framework_enum(&string_enum));
    assert!(
        text.contains("类型违例"),
        "字符串形态应指认类型违例，实际 {text}"
    );

    // 形态 4：同集合异序（`pytest` 提前）—— 指认顺序漂移，MUST NOT 静默排序。
    let mut reordered = mirror_copy();
    let last = framework_enum_mut(&mut reordered)
        .pop()
        .expect("镜像枚举非空");
    framework_enum_mut(&mut reordered).insert(0, last);
    let text = panic_text(|| assert_framework_enum(&reordered));
    assert!(
        text.contains("顺序漂移"),
        "异序形态应指认顺序漂移，实际 {text}"
    );
}

#[test]
fn 往返断言_入仓镜像八值逐一往返闭合() {
    // A3 正向行：入仓镜像的 8 串逐一经 `parse_framework` 解回、再由 `wire_name`
    // 回到同一串；该断言体已逐串闭合，不再在用例内重复同一趟遍历。
    assert_framework_roundtrip(&mirror());
}

#[test]
fn 往返断言_非法串不往返被抓且文案含首个失败项() {
    let long_text = "j".repeat(1200);
    let special_text = "je\nst\0\u{1F600}";
    let cases: [(&str, &str); 5] = [
        ("改串", "jest2"),
        ("大写连字符", "Node-Test"),
        ("空串", ""),
        ("超长串", long_text.as_str()),
        ("特殊字符", special_text),
    ];

    for (label, illegal) in cases {
        let mut schema = mirror_copy();
        framework_enum_mut(&mut schema).push(json!(illegal));

        let text = panic_text(|| assert_framework_roundtrip(&schema));

        assert!(text.contains("不往返"), "{label}形态应被抓，实际 {text}");
        assert!(
            text.contains(&format!("{illegal:?}")),
            "{label}形态文案应含首个不往返串，实际 {text}"
        );
        assert!(
            text.contains("None"),
            "{label}形态文案应含解回结果 None，实际 {text}"
        );
    }
}

#[test]
fn wire_name八变体逐串对位且连字符形态保留() {
    let expected: [(&str, TestFramework); 8] = [
        ("jest", TestFramework::Jest),
        ("vitest", TestFramework::Vitest),
        ("vite-plus", TestFramework::VitePlus),
        ("bun", TestFramework::Bun),
        ("rust", TestFramework::Rust),
        ("node-test", TestFramework::NodeTest),
        ("go", TestFramework::Go),
        ("pytest", TestFramework::Pytest),
    ];

    // `wire_name` 是无状态纯函数：一次调用即对位判据（幂等性由「每次返回同一
    // 字面量」自证，不再写 `wire_name(x) == wire_name(x)` 之类的恒真断言）。
    // 连字符形态（`vite-plus` / `node-test`）逐字保留，MUST NOT 归一为下划线或驼峰。
    for (wire, framework) in &expected {
        assert_eq!(
            wire_name(framework),
            *wire,
            "变体 wire 串对位失配（连字符形态 MUST NOT 归一为下划线或驼峰）"
        );
    }
}

// ===========================================================================
// 本变更：`known_keys` / `TestFramework::ALL` / `as_str`
// ===========================================================================

/// `known_keys()` 的声明序（本变更契约字面量：顺序即输出序）。
const KNOWN_KEYS_DECLARED: [&str; 8] = [
    "$schema",
    "schema",
    "context",
    "rules",
    "static_analysis",
    "projects",
    "tests",
    "write_protection",
];

/// 键集对位断言体（入参化：正向喂入仓镜像，异常行喂内存副本）——差异时优先
/// 指认两侧独有键，零静默通过。
fn keys_match(mirror_value: &Value) -> Result<(), String> {
    let Some(Value::Object(properties)) = mirror_value.get("properties") else {
        return Err("镜像缺 `properties`".to_owned());
    };
    let mut mirror_keys: Vec<&str> = properties.keys().map(String::as_str).collect();
    mirror_keys.sort_unstable();
    let mut impl_keys: Vec<&str> = known_keys().to_vec();
    impl_keys.sort_unstable();
    if impl_keys == mirror_keys {
        return Ok(());
    }
    let only_impl: Vec<&str> = impl_keys
        .iter()
        .copied()
        .filter(|key| !mirror_keys.contains(key))
        .collect();
    let only_mirror: Vec<&str> = mirror_keys
        .iter()
        .copied()
        .filter(|key| !impl_keys.contains(key))
        .collect();
    Err(format!(
        "键集漂移：仅实现有 {only_impl:?}；仅 schema 有 {only_mirror:?}"
    ))
}

/// 排序后的对象键集（键面断言的无序对照载体）。
fn sorted_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("被测值恒为对象")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// 键名零大写字母（P1 锚；`extra` 已展开回顶层，故无需例外分支）。
fn assert_no_uppercase_key(value: &Value, context: &str) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                assert!(
                    !key.chars().any(char::is_uppercase),
                    "{context}: 键名 {key} 含大写字母（file face 必须 snake_case）"
                );
                assert_no_uppercase_key(child, context);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_no_uppercase_key(item, context);
            }
        }
        _ => {}
    }
}

#[test]
fn known_keys返回八键且声明序逐值对位() {
    assert_eq!(known_keys().len(), 8, "顶层已知键封闭八项");
    assert_eq!(known_keys(), KNOWN_KEYS_DECLARED, "声明序即输出序");
    assert_eq!(known_keys(), KNOWN_KEYS, "只读出口与常量同源");
}

#[test]
fn known_keys键集与入仓镜像properties相等() {
    assert_eq!(keys_match(&mirror()), Ok(()), "八键与契约镜像键集对位");
}

#[test]
fn known_keys跨两次调用指针同一() {
    let first = known_keys();
    let second = known_keys();
    assert!(
        std::ptr::eq(first, second),
        "返回值是 `&'static` 常量切片的同一份内存（无二次构造）"
    );
}

#[test]
fn known_keys镜像副本删一键_比对失败且文案指认缺键() {
    let mut broken = mirror_copy();
    properties_mut(&mut broken)
        .remove("tests")
        .expect("入仓镜像含 `tests`");
    let message = keys_match(&broken).expect_err("删键后键集必不相等");
    assert!(
        message.contains("tests"),
        "文案应指认缺键（仅实现有 tests），实际 {message}"
    );
}

#[test]
fn framework枚举清单八值逐值_as_str与serde线面逐字相同() {
    assert_eq!(TestFramework::ALL.len(), 8, "八值封闭");
    for framework in TestFramework::ALL {
        let wire = match serde_json::to_value(framework).expect("framework 序列化失败") {
            Value::String(text) => text,
            other => panic!("framework 线面应为字符串，实际 {other}"),
        };
        assert_eq!(
            framework.as_str(),
            wire,
            "`as_str` 与 `#[serde(rename)]` 逐字对拍"
        );
    }
}

#[test]
fn framework枚举清单八值双向互证_parse往返闭合() {
    for framework in TestFramework::ALL {
        assert_eq!(
            parse_framework(framework.as_str()),
            Some(framework.clone()),
            "{} 的 as_str 应能解回同一变体",
            framework.as_str()
        );
    }
}

#[test]
fn framework枚举清单长度去重与声明序_且为静态切片() {
    let mut wires: Vec<&str> = TestFramework::ALL.iter().map(|f| f.as_str()).collect();
    assert_eq!(wires.len(), 8, "长度恰八");
    let declared = wires.clone();
    wires.sort_unstable();
    wires.dedup();
    assert_eq!(wires.len(), 8, "八值互异");
    assert_eq!(
        declared,
        vec![
            "jest",
            "vitest",
            "vite-plus",
            "bun",
            "rust",
            "node-test",
            "go",
            "pytest",
        ],
        "声明序即 wire 序（as_str 逐值对位）"
    );
    assert!(
        std::ptr::eq(TestFramework::ALL, TestFramework::ALL),
        "`ALL` 为 `&'static` 切片（零 clone 访问）"
    );
}

#[test]
fn parse_framework八形态全miss_大小写分隔符空白与emoji都不归一() {
    let long_text = "r".repeat(1200);
    let special = "je\nst\0\u{1F600}";
    for illegal in [
        "",
        "Jest",
        "vite_plus",
        "node_test",
        " rust",
        "rust ",
        "🙂",
        long_text.as_str(),
        special,
    ] {
        assert_eq!(
            parse_framework(illegal),
            None,
            "枚举外字面量 {illegal:?} 必须 miss（零归一化）"
        );
    }
}

// ===========================================================================
// 本变更：`validate`（任意 JSON 值 → 同一套组装校验）
// ===========================================================================

#[test]
fn validate合法满配对象_逐字段相等且零诊断() {
    let candidate = full_file_face();
    let report = validate(&candidate);
    assert!(report.diagnostics.is_empty(), "满配合法候选应零诊断");
    assert_eq!(
        value_of(&report.config),
        value_of(&config_of_file_face(&candidate)),
        "与 `assemble` 同源（无第二套校验）"
    );
    assert_eq!(report.config.tests.len(), 2, "两条 suite 全保留");
    assert_eq!(report.config.projects.len(), 2, "两条项目全保留");
}

#[test]
fn validate与load同源_同一份字节两入口线面值逐字段相等() {
    let ws = TempWs::new("validate-load-parity");
    ws.ensure_dir();
    let text = serde_json::to_string_pretty(&full_file_face()).expect("fixture 序列化失败");
    ws.write_config(&text);

    let via_load = load(ws.root());
    let via_validate = validate(&serde_json::from_str::<Value>(&text).expect("fixture 可解析"));

    assert_eq!(
        value_of(&via_load.config),
        value_of(&via_validate.config),
        "两入口的 config 线面值逐字段相等（零分叉）"
    );
    assert_eq!(
        via_load.diagnostics.len(),
        via_validate.diagnostics.len(),
        "诊断条数一致"
    );
}

#[test]
fn validate顶层非对象四形态_恰一条invalid_path为根且全默认() {
    for (tag, candidate) in [
        ("null", json!(null)),
        ("array", json!([1, 2])),
        ("string", json!("spec-driven")),
        ("number", json!(1)),
    ] {
        let report = validate(&candidate);
        assert_default_config(&report.config);
        assert_eq!(report.diagnostics.len(), 1, "{tag}: 恰一条诊断");
        assert!(
            has_diag(&report, &DiagnosticKind::InvalidValue, "$"),
            "{tag}: InvalidValue path $"
        );
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| kind_rank(&d.kind) == kind_rank(&DiagnosticKind::DefaultApplied)),
            "{tag}: 不产生 defaultApplied 噪声（与 load 非对象分支同口径）"
        );
    }
}

#[test]
fn validate空对象_吃默认并逐条default_applied_零文件态诊断() {
    let report = validate(&json!({}));
    assert_default_config(&report.config);
    for kind in [
        DiagnosticKind::FileMissing,
        DiagnosticKind::ReadFailed,
        DiagnosticKind::JsonInvalid,
        DiagnosticKind::InvalidValue,
    ] {
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| kind_rank(&d.kind) == kind_rank(&kind)),
            "空对象不得产文件态 / 违例诊断"
        );
    }
    assert!(
        has_diag(&report, &DiagnosticKind::DefaultApplied, "schema"),
        "schema 未设 → defaultApplied"
    );
    assert!(
        has_diag(&report, &DiagnosticKind::DefaultApplied, "tests"),
        "tests 未设 → defaultApplied"
    );
    assert!(!report.diagnostics.is_empty(), "吃默认逐条在案");
}

#[test]
fn validate多余顶层字段进extra且零诊断_四形态原文保留() {
    for value in [
        json!(1),
        json!([1, {"nested": true}]),
        json!({"deep": {"deeper": null}}),
        json!(null),
    ] {
        let mut candidate = full_file_face();
        candidate["custom"] = value.clone();
        let report = validate(&candidate);
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| kind_rank(&d.kind) == kind_rank(&DiagnosticKind::InvalidValue)),
            "未知顶层键不产违例诊断"
        );
        let extra = report.config.extra;
        assert_eq!(extra.len(), 1, "未知键恰一条");
        assert_eq!(extra[0].key, "custom");
        assert_eq!(extra[0].value, value, "原文值原样保留");
    }
}

#[test]
fn validate数值边界_合法越界与缺省各自处置() {
    fn lines_of(candidate: &Value) -> f64 {
        validate(candidate).config.tests[0].coverage.lines
    }
    fn suite(lines: Value) -> Value {
        json!({"tests": [{"root": "pkg", "framework": "jest", "coverage": {"lines": lines}}]})
    }

    assert_eq!(lines_of(&suite(json!(0))), 0.0, "下界 0 合法");
    assert_eq!(lines_of(&suite(json!(90.0))), 90.0, "整数写法的浮点原样");
    assert_eq!(lines_of(&suite(json!(-0.0))), 0.0, "-0.0 落在 [0,100]");
    assert_eq!(lines_of(&suite(json!(-1))), 80.0, "负值越界吃默认");
    assert_eq!(lines_of(&suite(json!(101))), 80.0, "上界外吃默认");
    assert_eq!(lines_of(&suite(json!(null))), 80.0, "显式 null 吃默认");

    for (tag, candidate) in [
        ("negative", suite(json!(-1))),
        ("overflow", suite(json!(101))),
        ("null", suite(json!(null))),
    ] {
        let report = validate(&candidate);
        assert!(
            has_diag(
                &report,
                &DiagnosticKind::InvalidValue,
                "tests[0].coverage.lines"
            ),
            "{tag}: 越界 / 非数值记 InvalidValue 且 path 对位"
        );
    }
}

// ===========================================================================
// 本变更：file face 投影（`load_file_face` / `to_file_value` / `plan_apply_config`）
// ===========================================================================

/// 满配 file face fixture（八键 + 多 suite + 多项目 + 多写保护规则，字段字母序、
/// 数值写整数）——P1–P4 与投影用例的共用底座。
fn full_file_face() -> Value {
    json!({
        "$schema": "https://example.com/spec-driven.schema.json",
        "context": "桌面端插件仓库",
        "projects": [
            {
                "depends_on": ["packages/desktop"],
                "language": "typescript",
                "root": "packages/app",
            },
            {
                "depends_on": [],
                "language": "rust",
                "root": "packages/desktop/src-tauri",
            },
        ],
        "rules": {"proposal": ["提案规则一"], "tasks": ["任务规则一"]},
        "schema": "spec-driven",
        "static_analysis": "clippy",
        "tests": [
            {
                "config": "vite-plus.config.ts",
                "coverage": {"branches": 88, "functions": 85, "lines": 90},
                "cwd": "packages/desktop",
                "excludes": ["dist/**"],
                "framework": "vite-plus",
                "includes": ["src/**/*.test.tsx"],
                "mutation": {"cwd": "packages/desktop", "score": 75},
                "root": "packages/desktop",
            },
            {
                "coverage": {"branches": 70, "functions": 75, "lines": 80},
                "cwd": "src-tauri",
                "framework": "rust",
                "mutation": {"score": 70},
                "root": "src-tauri",
            },
        ],
        "write_protection": {
            "files": [
                {"glob": "tests/golden/**", "reason": "金样冻结"},
                {"glob": "Cargo.lock"},
            ],
        },
    })
}

/// file face → 组装产物（`assemble` 为 crate 内私有项，测试经同源路径直用）。
fn config_of_file_face(face: &Value) -> WorkspaceConfig {
    let map = face.as_object().expect("file face fixture 恒为对象");
    let (config, diagnostics) = assemble(map);
    assert!(
        !diagnostics
            .iter()
            .any(|d| kind_rank(&d.kind) == kind_rank(&DiagnosticKind::InvalidValue)),
        "fixture 不得产违例诊断（defaultApplied 属正常形态），实际 {diagnostics:?}"
    );
    config
}

/// 落一份 file face fixture 到临时根。
fn write_face(ws: &TempWs, face: &Value) {
    ws.ensure_dir();
    ws.write_config(&serde_json::to_string_pretty(face).expect("fixture 序列化失败"));
}

#[test]
fn to_file_value_p1满配输出键名零大写() {
    let face = full_file_face();
    let projected = to_file_value(&config_of_file_face(&face), &face).expect("投影应成功");
    assert_no_uppercase_key(&projected, "P1");
    let text = serde_json::to_string(&projected).expect("序列化失败");
    for camel in ["staticAnalysis", "writeProtection", "dependsOn"] {
        assert!(
            !text.contains(camel),
            "P1: 输出不得含 camelCase 多词键 {camel}"
        );
    }
}

#[test]
fn to_file_value_p2满配顶层键集等于known_keys且无extra键() {
    let face = full_file_face();
    let projected = to_file_value(&config_of_file_face(&face), &face).expect("投影应成功");
    let mut expected: Vec<String> = known_keys().iter().map(|key| (*key).to_owned()).collect();
    expected.sort();
    assert_eq!(sorted_keys(&projected), expected, "P2: 顶层键集恰八键");
    assert!(
        projected.get("extra").is_none(),
        "P2: `extra` 包装键展开回顶层后不得残留"
    );
}

#[test]
fn to_file_value_p3_assemble回读与config逐字段相等() {
    let face = full_file_face();
    let config = config_of_file_face(&face);
    let projected = to_file_value(&config, &face).expect("投影应成功");
    let round_trip = config_of_file_face(&projected);
    assert_eq!(
        value_of(&round_trip),
        value_of(&config),
        "P3: `assemble ∘ to_file_value` 幂等于线面值"
    );
}

#[test]
fn to_file_value_p4全字段显式字面量往返逐字相等() {
    let face = full_file_face();
    let projected = to_file_value(&config_of_file_face(&face), &face).expect("投影应成功");
    assert_eq!(projected, face, "P4: 全字段显式字面量往返逐字相等");
}

#[test]
fn to_file_value模板递归裁剪_只写候选出现的键() {
    let candidate = json!({
        "schema": "spec-driven",
        "tests": [{"root": "pkg", "framework": "jest"}],
    });
    let config = config_of_file_face(&candidate);
    let projected = to_file_value(&config, &candidate).expect("投影应成功");
    assert_eq!(
        sorted_keys(&projected),
        vec!["schema".to_owned(), "tests".to_owned()],
        "顶层只留候选两键（默认字段不写）"
    );
    assert_eq!(
        sorted_keys(&projected["tests"][0]),
        vec!["framework".to_owned(), "root".to_owned()],
        "suite 也只留候选两键（递归裁剪，不是只裁顶层）"
    );
}

#[test]
fn to_file_value三处重命名递归到全层级_depends_on在元素内() {
    let face = full_file_face();
    let projected = to_file_value(&config_of_file_face(&face), &face).expect("投影应成功");
    assert!(
        projected["projects"][0].get("depends_on").is_some(),
        "projects[i].dependsOn 递归重命名为 depends_on"
    );
    assert!(
        projected.get("static_analysis").is_some(),
        "staticAnalysis 已重命名"
    );
    assert!(
        projected.get("write_protection").is_some(),
        "writeProtection 已重命名"
    );
}

#[test]
fn to_file_value模板非对象四形态_空对象取空_其余取整值() {
    let face = full_file_face();
    let config = config_of_file_face(&face);
    assert_eq!(
        to_file_value(&config, &json!({})).expect("空模板应成功"),
        json!({}),
        "空对象模板 → 空对象"
    );
    let via_null = to_file_value(&config, &json!(null)).expect("null 模板应成功");
    for template in [json!([1]), json!("schema")] {
        let projected = to_file_value(&config, &template).expect("非对象模板应成功");
        assert_eq!(
            projected, via_null,
            "模板 {template} 与 null 同走 trim 的「其余」分支：取 normalized 侧整值"
        );
    }
    assert_eq!(
        via_null["tests"][1]["config"],
        json!(null),
        "整值形态保留未裁剪的 null 位（未被模板裁剪）"
    );
}

#[test]
fn to_file_value数组按下标配对_短模板保越界项长模板只出值侧() {
    let two = json!({"tests": [
        {"root": "a", "framework": "jest"},
        {"root": "b", "framework": "rust"},
    ]});
    let config = config_of_file_face(&two);
    let short = to_file_value(&config, &json!({"tests": [{"root": "a"}]})).expect("短模板应成功");
    assert_eq!(
        short["tests"][0],
        json!({"root": "a"}),
        "模板覆盖的项按模板裁剪（只留 root）"
    );
    assert_eq!(
        short["tests"][1]["cwd"],
        json!("."),
        "模板短于值：越界项整值保留（未被裁剪的默认字段仍在）"
    );
    assert_eq!(
        short["tests"][1]["framework"],
        json!("rust"),
        "越界项保序保留"
    );
    let long = to_file_value(
        &config,
        &json!({"tests": [{"root": "a"}, {"root": "b"}, {"root": "c"}]}),
    )
    .expect("长模板应成功");
    assert_eq!(
        long["tests"].as_array().map(Vec::len),
        Some(2),
        "模板长于值：只输出值侧项"
    );
}

#[test]
fn to_file_value整数化四形态_整数归一与浮点保留() {
    let face = json!({
        "tests": [{
            "root": "pkg",
            "framework": "jest",
            "coverage": {"lines": 90.0, "branches": 90.5, "functions": -0.0},
            "mutation": {"score": 0.0},
        }],
    });
    let projected = to_file_value(&config_of_file_face(&face), &face).expect("投影应成功");
    let coverage = &projected["tests"][0]["coverage"];
    assert_eq!(coverage["lines"], json!(90), "90.0 → 90（整数形态）");
    assert_eq!(coverage["branches"], json!(90.5), "90.5 保浮点");
    assert_eq!(coverage["functions"], json!(0), "-0.0 → 0（fract 为零）");
    assert_eq!(
        projected["tests"][0]["mutation"]["score"],
        json!(0),
        "0.0 → 0（阈值面全为整数化域内数值；超 2^53 的阈值先被组装层吃默认，故不入判据）"
    );
}

#[test]
fn to_file_value_extra三坑_null值保留_键序重排_值不二次整数化() {
    let shape = json!({"schema": "spec-driven"});
    let config = WorkspaceConfig {
        extra: vec![
            ConfigExtraField {
                key: "zNull".to_owned(),
                value: json!(null),
            },
            ConfigExtraField {
                key: "aFloat".to_owned(),
                value: json!(1.5),
            },
        ],
        ..config_of_file_face(&shape)
    };
    let projected = to_file_value(&config, &json!(null)).expect("投影应成功");
    let map = projected.as_object().expect("输出恒对象");
    assert!(
        map.contains_key("zNull") && map.contains_key("aFloat"),
        "两条 extra 条目全部展开回顶层，实际键集 {:?}",
        map.keys().collect::<Vec<&String>>()
    );
    assert!(map["zNull"].is_null(), "null 值条目原文保留");
    // 整数化先于 `extra` 展开，故条目值同在这道递归里（1.0 → 1）；保真面是
    // 「非整数值不被改写」——1.5 逐字保留。
    assert_eq!(
        map["aFloat"],
        json!(1.5),
        "非整数条目值逐字保留（不被整数化改写）"
    );
    // 键序不作断言：`serde_json` 的 Map 后端随 `preserve_order` feature 在
    // workspace 级 feature 统一后变化（单包构建排序、工作区构建保插入序），
    // 键序漂移是接受面而非契约面。
}

#[test]
fn to_file_value_extra条目键与已知字段同名_显式失败零静默覆盖() {
    let config = WorkspaceConfig {
        extra: vec![ConfigExtraField {
            key: "schema".to_owned(),
            value: json!(1),
        }],
        ..config_of_file_face(&json!({"schema": "spec-driven"}))
    };
    let error = to_file_value(&config, &json!(null)).expect_err("键冲突必须显式失败");
    assert!(error.contains("schema"), "文案应含冲突键，实际 {error}");
}

#[test]
fn to_file_value_extra超深嵌套不panic() {
    let mut deep = json!(0);
    for _ in 0..1000 {
        deep = Value::Array(vec![deep]);
    }
    let config = WorkspaceConfig {
        extra: vec![ConfigExtraField {
            key: "deep".to_owned(),
            value: deep.clone(),
        }],
        ..config_of_file_face(&json!({"schema": "spec-driven"}))
    };
    let projected = to_file_value(&config, &json!({"deep": true})).expect("深嵌套应成功");
    assert_eq!(projected["deep"], deep, "保真优先（不做深度裁剪）");
}

#[test]
fn load_file_face满配盘面_键形全snake_case且零诊断() {
    let ws = TempWs::new("face-full");
    write_face(&ws, &full_file_face());

    let (projected, diagnostics) = load_file_face(ws.root()).expect("盘面投影应成功");

    assert!(diagnostics.is_empty(), "满配盘面零诊断");
    assert_no_uppercase_key(&projected, "load_file_face");
    assert!(
        projected.get("static_analysis").is_some(),
        "static_analysis 键形"
    );
    assert!(
        projected.get("write_protection").is_some(),
        "write_protection 键形"
    );
    assert!(
        projected["projects"][0].get("depends_on").is_some(),
        "projects[0].depends_on 键形"
    );
    let text = serde_json::to_string(&projected).expect("序列化失败");
    assert!(!text.contains("staticAnalysis"), "零 camelCase 残留");
}

#[test]
fn load_file_face与propose_current公式同源_不等于wire面() {
    let ws = TempWs::new("face-formula");
    let face = full_file_face();
    write_face(&ws, &face);

    let (projected, _) = load_file_face(ws.root()).expect("盘面投影应成功");
    let expected = to_file_value(&config_of_file_face(&face), &face).expect("投影应成功");

    assert_eq!(
        projected, expected,
        "输出 == to_file_value(assemble(盘上值).config, 盘上值)"
    );
    let wire = serde_json::to_value(config_of_file_face(&face)).expect("wire 序列化失败");
    assert_ne!(
        projected, wire,
        "MUST NOT 等于 wire face（to_value(&WorkspaceConfig)）"
    );
}

#[test]
fn load_file_face文件缺失_ok空对象加_file_missing() {
    let ws = TempWs::new("face-missing");
    ws.ensure_dir();

    let (projected, diagnostics) = load_file_face(ws.root()).expect("缺失不得 Err");

    assert_eq!(projected, json!({}), "缺失 → 空对象");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        kind_rank(&diagnostics[0].kind),
        kind_rank(&DiagnosticKind::FileMissing)
    );
}

#[test]
fn load_file_face空对象盘面_空file_face加default_applied() {
    let ws = TempWs::new("face-empty");
    ws.write_config("{}");

    let (projected, diagnostics) = load_file_face(ws.root()).expect("空对象应成功");

    assert_eq!(
        projected,
        json!({}),
        "模板裁剪：不写默认键（file face 为空对象）"
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| kind_rank(&d.kind) == kind_rank(&DiagnosticKind::DefaultApplied)),
        "空对象吃默认并逐条在案"
    );
}

#[test]
fn load_file_face坏json与非对象五形态_ok空对象加该条诊断() {
    for (tag, content, expected) in [
        ("bad-json", "{", DiagnosticKind::JsonInvalid),
        ("null", "null", DiagnosticKind::InvalidValue),
        ("array", "[]", DiagnosticKind::InvalidValue),
        ("string", "\"x\"", DiagnosticKind::InvalidValue),
        ("number", "1", DiagnosticKind::InvalidValue),
    ] {
        let ws = TempWs::new(&format!("face-{tag}"));
        ws.write_config(content);

        let (projected, diagnostics) = load_file_face(ws.root()).expect("文件态诊断不 Err");

        assert_eq!(projected, json!({}), "{tag}: 空对象");
        assert_eq!(diagnostics.len(), 1, "{tag}: 恰一条诊断");
        assert_eq!(
            kind_rank(&diagnostics[0].kind),
            kind_rank(&expected),
            "{tag}: 诊断类别对位"
        );
        assert_eq!(diagnostics[0].path, "$", "{tag}: path 为文件级根");
    }
}

#[test]
fn load_file_face配置路径被同名目录占据_read_failed() {
    let ws = TempWs::new("face-read-failed");
    ws.ensure_dir();
    ws.occupy_config_path_as_dir();

    let (projected, diagnostics) = load_file_face(ws.root()).expect("读取失败不 Err");

    assert_eq!(projected, json!({}));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        kind_rank(&diagnostics[0].kind),
        kind_rank(&DiagnosticKind::ReadFailed)
    );
}

#[test]
fn load_file_face键冲突不可达_以直接构造覆盖同一失败面() {
    // 盘面 `extra` 键与已知字段同名在组装期被 passthrough 吸收（不可达分支），
    // 故冲突判据只在写入器的直接构造面观察——同一 `expand_extra` 单点。
    let conflict = WorkspaceConfig {
        extra: vec![ConfigExtraField {
            key: "tests".to_owned(),
            value: json!([]),
        }],
        ..config_of_file_face(&json!({"schema": "spec-driven"}))
    };
    let error = to_file_value(&conflict, &json!(null)).expect_err("冲突必须 Err");
    assert!(error.contains("tests"), "文案含冲突键，实际 {error}");
}

#[test]
fn plan_apply_config合法候选_路径与内容双证() {
    let ws = TempWs::new("plan-ok");
    ws.ensure_dir();
    let candidate = full_file_face();

    let plan = plan_apply_config(ws.root(), &candidate).expect("合法候选应成功");

    assert_eq!(plan.file, config_path(ws.root()), "目标文件按 layout 派生");
    assert!(plan.content.ends_with('\n'), "尾换行在场");
    assert!(plan.content.contains("\n  "), "pretty 分隔符在场（非紧凑）");
    let reparsed: Value = serde_json::from_str(&plan.content).expect("内容必须可解析回 JSON");
    let report = validate(&candidate);
    assert_eq!(
        reparsed,
        to_file_value(&report.config, &candidate).expect("投影应成功"),
        "内容 == to_file_value(&report.config, candidate)"
    );
}

#[test]
fn plan_apply_config零io_两种起点前后零变化() {
    fn snapshot(path: &Path) -> (Option<String>, Option<std::time::SystemTime>) {
        match fs::metadata(path) {
            Ok(metadata) => (
                Some(fs::read_to_string(path).expect("读目标文件失败")),
                metadata.modified().ok(),
            ),
            Err(_) => (None, None),
        }
    }

    let ws = TempWs::new("plan-zero-io");
    ws.ensure_dir();
    let path = config_path(ws.root());

    let before = snapshot(&path);
    plan_apply_config(ws.root(), &full_file_face()).expect("规划应成功");
    assert_eq!(snapshot(&path), before, "目标不存在起点：调用后仍不存在");

    ws.write_config("{\"schema\": \"spec-driven\"}");
    let before = snapshot(&path);
    plan_apply_config(ws.root(), &full_file_face()).expect("规划应成功");
    assert_eq!(
        snapshot(&path),
        before,
        "目标存在起点：内容与 mtime 双证不变"
    );
}

#[test]
fn plan_apply_config任一invalid_value_逐条文案且不产content() {
    let ws = TempWs::new("plan-invalid");
    ws.ensure_dir();
    let cases = [
        (
            "overflow",
            json!({"tests": [{"root": "pkg", "framework": "jest", "coverage": {"lines": 101}}]}),
        ),
        (
            "framework",
            json!({"tests": [{"root": "pkg", "framework": "jest2"}]}),
        ),
        (
            "glob-root",
            json!({"tests": [{"root": "src/*", "framework": "jest"}]}),
        ),
    ];
    for (tag, candidate) in cases {
        let error = plan_apply_config(ws.root(), &candidate).expect_err("违例必须 Err");
        assert!(
            error.contains("tests"),
            "{tag}: 文案应逐条含 path，实际 {error}"
        );
        assert!(
            error.contains("重新提交完整候选"),
            "{tag}: 尾行含修正指引，实际 {error}"
        );
    }
}

#[test]
fn plan_apply_config仅default_applied_不拦写回且内容为空对象形态() {
    let ws = TempWs::new("plan-defaulted");
    ws.ensure_dir();

    let plan = plan_apply_config(ws.root(), &json!({})).expect("defaultApplied 不拦写回");

    assert_eq!(plan.content, "{}\n", "空对象 pretty 形态 + 尾换行");
}

#[test]
fn plan_apply_config顶层非对象err_未知顶层键保留() {
    let ws = TempWs::new("plan-top");
    ws.ensure_dir();

    for candidate in [json!(null), json!([])] {
        let error = plan_apply_config(ws.root(), &candidate).expect_err("顶层非对象必须 Err");
        assert!(error.contains('$'), "文案 path 为文件级根，实际 {error}");
    }

    let plan = plan_apply_config(ws.root(), &json!({"customFlag": true})).expect("未知键应 Ok");
    let reparsed: Value = serde_json::from_str(&plan.content).expect("内容可解析");
    assert_eq!(
        reparsed["customFlag"],
        json!(true),
        "未知顶层键在 content 顶层（extra 展开）"
    );
}

#[test]
fn plan_apply_config_root有效性不归本函数_两形态仍按layout派生() {
    let empty_root = Path::new("");
    let missing_root = Path::new(r"Z:\definitely-missing-workspace-root");
    for root in [empty_root, missing_root] {
        let plan = plan_apply_config(root, &json!({"schema": "spec-driven"}))
            .expect("root 校验归命令层，本函数零校验");
        assert_eq!(
            plan.file,
            config_path(root),
            "file 仍按 layout::config_path 派生"
        );
    }
}

// ---------------------------------------------------------------------------
// projects 文案口径（「相对 worktree 根」→「相对执行根」）：zero-code 词汇变
// 更的镜像面自证（谓词 / 键集 / 值域 / default / minLength 零改）
// ---------------------------------------------------------------------------

/// 镜像 `properties.projects` 对象（缺失 / `null` 与类型违例分文案——文案变更
/// 断言与损坏形态断言的定位面）。
fn mirror_projects(schema: &Value) -> &Map<String, Value> {
    match mirror_properties(schema).get("projects") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) => panic!("镜像缺 `projects`：期望对象，实际 null"),
        Some(other) => panic!("镜像 `projects` 类型违例：期望对象，实际 {other}"),
        None => panic!("镜像缺 `projects`：`properties` 未取到该键"),
    }
}

/// 镜像 `properties.projects.items.properties.root` 对象（同分文案口径）。
fn mirror_project_root(schema: &Value) -> &Map<String, Value> {
    let items = match mirror_projects(schema).get("items") {
        Some(Value::Object(map)) => map,
        Some(Value::Null) | None => panic!("镜像缺 `projects.items`：期望对象"),
        Some(other) => panic!("镜像 `projects.items` 类型违例：期望对象，实际 {other}"),
    };
    let root = match items.get("properties").and_then(|value| value.get("root")) {
        Some(Value::Object(map)) => map,
        Some(Value::Null) | None => panic!("镜像缺 `projects.items.properties.root`：期望对象"),
        Some(other) => {
            panic!("镜像 `projects.items.properties.root` 类型违例：期望对象，实际 {other}")
        }
    };
    root
}

/// schema 文案（正向）：`projects.description` 与
/// `projects.items.properties.root.description` 两处含「执行根」且镜像内零
/// 「相对 worktree 根」残留。
#[test]
fn schema_projects文案改执行根口径() {
    let schema = mirror();
    let text = serde_json::to_string(&schema).expect("序列化应成功");
    assert!(
        !text.contains("相对 worktree 根"),
        "镜像内零「相对 worktree 根」残留"
    );
    assert!(
        !text.contains("worktree"),
        "镜像内零 worktree 词汇（配置面文案口径改执行根）"
    );

    let projects = mirror_projects(&schema);
    let description = projects
        .get("description")
        .and_then(Value::as_str)
        .expect("projects.description 应在场");
    assert!(
        description.contains("相对执行根"),
        "projects.description 改为执行根口径，实际: {description}"
    );

    let root = mirror_project_root(&schema);
    let root_description = root
        .get("description")
        .and_then(Value::as_str)
        .expect("root.description 应在场");
    assert!(
        root_description.contains("相对执行根"),
        "root.description 改为执行根口径，实际: {root_description}"
    );
}

/// schema 契约镜像（异常）：镜像内除两处 `description` 外零 diff——
/// `projects.type` / `default: []` / `items.properties.root.minLength: 1` /
/// `items.properties.root.type` 逐字不变。
#[test]
fn schema_projects文案变更外字段逐字不变() {
    let schema = mirror();
    let projects = mirror_projects(&schema);

    assert_eq!(
        projects.get("type"),
        Some(&Value::String("array".to_owned())),
        "projects.type 零改"
    );
    assert_eq!(
        projects.get("default"),
        Some(&Value::Array(Vec::new())),
        "projects.default 仍为空数组"
    );

    let root = mirror_project_root(&schema);
    assert_eq!(
        root.get("type"),
        Some(&Value::String("string".to_owned())),
        "root.type 零改"
    );
    assert_eq!(
        root.get("minLength"),
        Some(&json!(1)),
        "root.minLength 仍为 1"
    );
    assert!(
        !root.contains_key("pattern"),
        "root 值域未因文案变更新增 pattern（谓词仍在实现侧）"
    );
}

/// schema 契约镜像（异常，表驱动两形态）：副本删 `projects` / 把 `projects` 改
/// 成数组 → 既有取值器给出可指认 panic 文案（不静默通过）。
#[test]
fn schema_镜像损坏两形态_取值器可指认panic() {
    let panic_message = |payload: Box<dyn std::any::Any + Send>| -> String {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
            })
            .unwrap_or_else(|| "（非字符串 payload）".to_owned())
    };

    // 形态一：删 `projects` 键
    let mut missing = mirror_copy();
    properties_mut(&mut missing).remove("projects");
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = mirror_projects(&missing);
    }))
    .expect_err("缺 projects 应 panic");
    assert!(
        panic_message(payload).contains("镜像缺 `projects`"),
        "缺键形态 panic 文案可指认"
    );

    // 形态二：`projects` 改成数组（类型违例）
    let mut wrong = mirror_copy();
    properties_mut(&mut wrong).insert("projects".to_owned(), json!([]));
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = mirror_projects(&wrong);
    }))
    .expect_err("projects 为数组应 panic");
    assert!(
        panic_message(payload).contains("类型违例"),
        "类型违例形态 panic 文案可指认"
    );
}

/// `is_valid_project_root()` 注释口径（边界）：源码文本不含「逃出 worktree」，
/// 含「逃出执行根」（`env!` 读源文件，`steps_test` 先例）。
#[test]
fn is_valid_project_root注释口径改执行根() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("lib.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取 {} 失败: {error}", path.display()));

    assert!(
        !source.contains("逃出 worktree"),
        "注释零「逃出 worktree」残留"
    );
    assert!(
        source.contains("会逃出") && source.contains("执行根，故在解析期词法拒绝"),
        "注释口径改为逃出执行根"
    );
}
