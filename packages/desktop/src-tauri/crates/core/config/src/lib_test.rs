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
    assemble, load, parse_framework, ConfigReport, DiagnosticKind, TestFramework, TestSuite,
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
            "`properties` 恰 7 键（与 KNOWN_KEYS 同规模）",
            properties_key_count == 7,
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

#[test]
fn 键集断言_入仓镜像正向通过且七键逐项对位() {
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
            "rules",
            "schema",
            "static_analysis",
            "tests",
            "write_protection",
        ],
        "镜像键集应为 7 键全集"
    );

    // 与 `KNOWN_KEYS` 的对位由 `assert_key_set` 承担（上方调用），此处只钉字面量
    // 7 键全集，避免同一键集比较写两遍。
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
    // 形态 1：`properties: {}`（空对象）—— 7 项全落「仅实现有」。
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
