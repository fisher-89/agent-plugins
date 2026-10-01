//! `config::load` 的单元测试（AC-2）：配置语义唯一入口的文件态四分支 + 顶层与
//! 基础字段 / tests suite / coverage / mutation / write_protection / passthrough
//! 组装校验矩阵 + 自洽不变量（「config 输出永远合法」的机械形态）。
//!
//! 文件系统不 mock：真实 tempdir fixture（RAII 清理）承载缺失 / 不可读（同名
//! 目录占据）/ 坏 JSON / 合法配置各形态；serde_json 只作 DTO 无 PartialEq 的
//! 线面对照载体，不逐项断言 serde / JSON 库自身语义（测试纪律）。`assemble`
//! 经 crate 内子模块路径访问（子模块可见父模块私有项），自洽不变量直接调用，
//! 不新增测试专用导出（CLAUDE.md no-test-only-exports）。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{assemble, load, ConfigReport, DiagnosticKind, TestSuite, WorkspaceConfig};
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
        r#"{"schema": "spec-driven", "context": "上下文", "rules": {"proposal": ["规则"]}, "tests": [{"root": "pkg", "framework": "vite-plus", "coverage": {"lines": 90.5, "branches": 80, "functions": 85}}], "write_protection": {"files": [{"glob": "tests/golden/**", "reason": "金样冻结"}]}, "custom": {"n": 1}}"#,
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
