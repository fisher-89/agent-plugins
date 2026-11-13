use crate::model::{
    parse_sub_report, parse_summary_report, CaseSummary, Conclusion, CoverageBlock,
    CoverageMeasured, CoverageOverride, CoverageThresholds, MutationBlock, MutationMeasured,
    MutationOverride, Problem, ProblemType, SubReport, TestCaseResult, TestCaseStatus,
};

// ---------------------------------------------------------------------------
// fixture：真实形态全字段子报告（coverage 带 override 组、mutation 带组、
// findings 在场；errorType / errorMessage / stackTrace 为 CLI 原样驼峰键名）
// ---------------------------------------------------------------------------

const FULL_SUB_REPORT_JSON: &str = r#"{
  "framework": "vite-plus",
  "root": "packages/desktop",
  "timestamp": "2026-10-06T08:00:00.000Z",
  "exit_code": 1,
  "duration_ms": 1420.5,
  "summary": { "total": 12, "passed": 10, "failed": 2, "skipped": 0 },
  "error_cases": [
    {
      "name": "math > rejects NaN input",
      "file": "src/app/math.test.ts",
      "duration_ms": 3.2,
      "status": "failed",
      "line": 17,
      "errorType": "AssertionError",
      "errorMessage": "expected 1 to equal 2",
      "stackTrace": "AssertionError: expected 1 to equal 2\n    at test.js:17:5"
    },
    {
      "name": "math > times out on slow path",
      "status": "skipped"
    }
  ],
  "test_files": ["src/app/math.test.ts", "src/lib/util.test.ts"],
  "source_files": [
    {
      "file": "src/app/math.ts",
      "coverage": {
        "lines": 90.0, "branches": 75.0, "functions": 80.0,
        "total_lines": 20, "covered_lines": 18,
        "total_branches": 8, "covered_branches": 6,
        "total_functions": 10, "covered_functions": 8
      }
    },
    { "file": "src/lib/uncovered.ts" }
  ],
  "coverage": {
    "pass": false,
    "measured": { "lines": 86.67, "branches": 72.73, "functions": null },
    "thresholds": { "lines": 80, "branches": 70, "functions": 75 },
    "overrides": [
      {
        "glob": "src/app",
        "thresholds": { "lines": 85, "branches": 75, "functions": 90 },
        "measured": { "lines": 90.0, "branches": 75.0, "functions": 80.0 },
        "pass": true,
        "file_count": 1,
        "passed_count": 1
      }
    ]
  },
  "mutation": {
    "pass": true,
    "score": 82.35,
    "threshold": 70,
    "measured": {
      "killed": 11, "survived": 2, "timeout": 1, "noCoverage": 3,
      "compileError": 1, "runtimeError": 1, "ignored": 2,
      "total": 21, "detected": 12, "undetected": 5
    },
    "overrides": [
      {
        "glob": "src/app",
        "score": 82.35, "threshold": 70, "pass": true,
        "file_count": 3, "passed_count": 3
      }
    ]
  },
  "findings": ["「vite-plus」2 项测试失败——聚类失败（疑似同模块回归或共同假设变化）; 明细见子报告 error_cases"]
}"#;

const FULL_SUMMARY_JSON: &str = r#"{
  "phase": "test-execution",
  "command": "dev-team test-execution",
  "timestamp": "2026-10-06T08:01:00.000Z",
  "duration_seconds": 14,
  "total": 12,
  "passed": 10,
  "failed": 2,
  "skipped": 0,
  "conclusion": "fail",
  "problems": [
    {
      "framework": "vite-plus",
      "type": "test_failure",
      "message": "Test \"math > rejects NaN input\" failed: expected 1 to equal 2"
    },
    {
      "framework": "all",
      "type": "coverage_failure",
      "message": "Coverage below threshold: functions 78.9% < 75%"
    }
  ],
  "coverage": {
    "pass": true,
    "measured": { "lines": 86.67, "branches": 72.73, "functions": null },
    "thresholds": { "lines": 80, "branches": 70, "functions": 75 }
  },
  "mutation": null,
  "plans": [
    {
      "id": "vite-plus",
      "framework": "vite-plus",
      "root": ".",
      "path": "reports/test/vite-plus"
    }
  ],
  "findings": ["「vite-plus」2 项测试失败——聚类失败（疑似同模块回归或共同假设变化）; 明细见子报告 error_cases"]
}"#;

/// 子报告全字段 fixture 的 Rust 侧期望值。
fn expected_full_sub_report() -> SubReport {
    SubReport {
        framework: "vite-plus".to_owned(),
        root: "packages/desktop".to_owned(),
        timestamp: "2026-10-06T08:00:00.000Z".to_owned(),
        exit_code: 1,
        duration_ms: 1420.5,
        summary: CaseSummary {
            total: 12,
            passed: 10,
            failed: 2,
            skipped: 0,
        },
        error_cases: vec![
            TestCaseResult {
                name: "math > rejects NaN input".to_owned(),
                file: Some("src/app/math.test.ts".to_owned()),
                duration_ms: Some(3.2),
                status: TestCaseStatus::Failed,
                line: Some(17.0),
                error_type: Some("AssertionError".to_owned()),
                error_message: Some("expected 1 to equal 2".to_owned()),
                stack_trace: Some(
                    "AssertionError: expected 1 to equal 2\n    at test.js:17:5".to_owned(),
                ),
            },
            TestCaseResult {
                name: "math > times out on slow path".to_owned(),
                file: None,
                duration_ms: None,
                status: TestCaseStatus::Skipped,
                line: None,
                error_type: None,
                error_message: None,
                stack_trace: None,
            },
        ],
        test_files: vec![
            "src/app/math.test.ts".to_owned(),
            "src/lib/util.test.ts".to_owned(),
        ],
        source_files: vec![
            crate::model::SourceFileEntry {
                file: "src/app/math.ts".to_owned(),
                coverage: Some(crate::model::FileCoverageEntry {
                    lines: 90.0,
                    branches: Some(75.0),
                    functions: Some(80.0),
                    total_lines: Some(20),
                    covered_lines: Some(18),
                    total_branches: Some(8),
                    covered_branches: Some(6),
                    total_functions: Some(10),
                    covered_functions: Some(8),
                }),
            },
            crate::model::SourceFileEntry {
                file: "src/lib/uncovered.ts".to_owned(),
                coverage: None,
            },
        ],
        coverage: Some(CoverageBlock {
            pass: false,
            measured: CoverageMeasured {
                lines: Some(86.67),
                branches: Some(72.73),
                functions: None,
            },
            thresholds: CoverageThresholds {
                lines: 80.0,
                branches: 70.0,
                functions: 75.0,
            },
            overrides: Some(vec![CoverageOverride {
                glob: "src/app".to_owned(),
                thresholds: CoverageThresholds {
                    lines: 85.0,
                    branches: 75.0,
                    functions: 90.0,
                },
                measured: CoverageMeasured {
                    lines: Some(90.0),
                    branches: Some(75.0),
                    functions: Some(80.0),
                },
                pass: true,
                file_count: 1,
                passed_count: 1,
            }]),
        }),
        mutation: Some(MutationBlock {
            pass: true,
            score: 82.35,
            threshold: 70.0,
            measured: MutationMeasured {
                killed: 11,
                survived: 2,
                timeout: 1,
                no_coverage: 3,
                compile_error: 1,
                runtime_error: 1,
                ignored: 2,
                total: 21,
                detected: 12,
                undetected: 5,
            },
            overrides: Some(vec![MutationOverride {
                glob: "src/app".to_owned(),
                score: 82.35,
                threshold: 70.0,
                pass: true,
                file_count: 3,
                passed_count: 3,
            }]),
        }),
        findings: Some(vec![
            "「vite-plus」2 项测试失败——聚类失败（疑似同模块回归或共同假设变化）; 明细见子报告 error_cases"
                .to_owned(),
        ]),
    }
}

// ---------------------------------------------------------------------------
// parse_sub_report
// ---------------------------------------------------------------------------

/// 正向：真实形态子报告 JSON（framework / root / timestamp / exit_code /
/// duration_ms / summary / error_cases / test_files / source_files / coverage
/// 全字段）解析出 SubReport 逐字段相等。
#[test]
fn 真实形态子报告全字段解析逐字段相等() {
    let parsed = parse_sub_report(FULL_SUB_REPORT_JSON).expect("全字段子报告应解析成功");
    assert_eq!(
        parsed,
        expected_full_sub_report(),
        "逐字段相等（serde 全字段对齐）"
    );
}

/// 异常：非法 JSON 文本 → Err 记因（解析失败面可读，前缀标明子报告解析失败）。
#[test]
fn 非法json文本err记因() {
    for broken in [
        "{ not json at all",
        "{\"framework\": \"vite-plus\",", // 截断 JSON
        "",                               // 空文本
    ] {
        let err = parse_sub_report(broken).expect_err("非法 JSON 应 Err");
        assert!(
            err.starts_with("子报告解析失败:"),
            "Err 记因面可读（子报告解析失败前缀），实际: {err}"
        );
    }
}

/// 异常：必填字段缺失（缺 framework / 缺 summary 计数块 / 缺 exit_code）→
/// Err，required 集与 CLI zod schema 一致（必需字段不默认、不吞缺失）。
#[test]
fn 必填字段缺失err_required集与zod一致() {
    let missing_framework = r#"{
      "root": ".", "timestamp": "2026-10-06T08:00:00Z", "exit_code": 0,
      "duration_ms": 1.0, "summary": { "total": 0, "passed": 0, "failed": 0, "skipped": 0 }
    }"#;
    let err = parse_sub_report(missing_framework).expect_err("缺 framework 应 Err");
    assert!(err.contains("framework"), "记因指向缺失字段，实际: {err}");

    let missing_summary = r#"{
      "framework": "vite-plus", "root": ".", "timestamp": "2026-10-06T08:00:00Z",
      "exit_code": 0, "duration_ms": 1.0
    }"#;
    let err = parse_sub_report(missing_summary).expect_err("缺 summary 计数块应 Err");
    assert!(err.contains("summary"), "记因指向缺失字段，实际: {err}");

    let missing_exit_code = r#"{
      "framework": "vite-plus", "root": ".", "timestamp": "2026-10-06T08:00:00Z",
      "duration_ms": 1.0, "summary": { "total": 0, "passed": 0, "failed": 0, "skipped": 0 }
    }"#;
    let err = parse_sub_report(missing_exit_code).expect_err("缺 exit_code 应 Err");
    assert!(err.contains("exit_code"), "记因指向缺失字段，实际: {err}");

    // summary 计数块内部必填（zod summarySchema 四计数）同样不缺省
    let missing_count_field = r#"{
      "framework": "vite-plus", "root": ".", "timestamp": "2026-10-06T08:00:00Z",
      "exit_code": 0, "duration_ms": 1.0, "summary": { "total": 0, "passed": 0, "failed": 0 }
    }"#;
    let err = parse_sub_report(missing_count_field).expect_err("summary 缺 skipped 应 Err");
    assert!(
        err.contains("skipped"),
        "计数块必填集与 zod 一致，实际: {err}"
    );
}

/// 边界：未知字段混入（多余键，含嵌套层与顶层）→ 解析成功且忽略未知键、
/// 已知字段不丢（zod strip 宽容对齐）。
#[test]
fn 未知字段宽容忽略且已知字段不丢() {
    let with_unknown = r#"{
      "framework": "node-test",
      "root": ".",
      "timestamp": "2026-10-06T08:00:00Z",
      "exit_code": 0,
      "duration_ms": 12.0,
      "summary": { "total": 5, "passed": 5, "failed": 0, "skipped": 0, "flaky": 1 },
      "error_cases": [],
      "future_top_level": { "nested": { "unknown": true } },
      "another_unknown": "ignored"
    }"#;
    let parsed = parse_sub_report(with_unknown).expect("未知字段应宽容忽略");

    assert_eq!(parsed.framework, "node-test", "已知字段不丢");
    assert_eq!(parsed.root, ".");
    assert_eq!(parsed.exit_code, 0);
    assert_eq!(parsed.duration_ms, 12.0);
    assert_eq!(
        parsed.summary,
        CaseSummary {
            total: 5,
            passed: 5,
            failed: 0,
            skipped: 0
        },
        "summary 内未知键（flaky）不丢已知四计数"
    );
    assert!(parsed.error_cases.is_empty());
}

/// 边界：coverage / mutation / findings 缺省（显式 null 或缺键）→ None 形态
/// 解析与再出线往返无损（skip_serializing_if 缺键形态重解析等值）。
#[test]
fn coverage_mutation_findings缺省_none形态往返无损() {
    // 显式 null 形态（CLI 产出 coverage: null / mutation: null 常态）
    let explicit_null = r#"{
      "framework": "rust", "root": "crates", "timestamp": "2026-10-06T08:00:00Z",
      "exit_code": 0, "duration_ms": 900.0,
      "summary": { "total": 3, "passed": 3, "failed": 0, "skipped": 0 },
      "error_cases": [], "test_files": [], "source_files": [],
      "coverage": null, "mutation": null, "findings": null
    }"#;
    let parsed = parse_sub_report(explicit_null).expect("显式 null 应解析为 None");
    assert!(parsed.coverage.is_none() && parsed.mutation.is_none() && parsed.findings.is_none());

    // 缺键形态（宽容面：serde default）
    let keys_absent = r#"{
      "framework": "rust", "root": "crates", "timestamp": "2026-10-06T08:00:00Z",
      "exit_code": 0, "duration_ms": 900.0,
      "summary": { "total": 3, "passed": 3, "failed": 0, "skipped": 0 }
    }"#;
    let parsed = parse_sub_report(keys_absent).expect("缺键应解析为 None");
    assert!(parsed.coverage.is_none() && parsed.mutation.is_none());
    assert!(parsed.error_cases.is_empty() && parsed.source_files.is_empty());

    // 再出线往返无损：None 序列化为显式 null（模型层无 skip），重解析与原值逐字段相等
    let text = serde_json::to_string(&parsed).expect("序列化应成功");
    assert!(
        text.contains("\"coverage\":null") && text.contains("\"mutation\":null"),
        "None 出线 null 形态: {text}"
    );
    let roundtrip = parse_sub_report(&text).expect("再出线重解析应成功");
    assert_eq!(roundtrip, parsed, "None 形态往返无损");
}

/// 边界：含真实 mutation 块样本 → MutationBlock 保真解析不丢块（schema 保真
/// 半边——生产聚合恒 null 与解析保真两半分离）。
#[test]
fn 真实mutation块保真解析不丢块() {
    let parsed = parse_sub_report(FULL_SUB_REPORT_JSON).expect("解析应成功");
    let mutation = parsed
        .mutation
        .as_ref()
        .expect("mutation 块应在场（不丢块）");

    assert!(mutation.pass);
    assert_eq!(mutation.score, 82.35);
    assert_eq!(mutation.threshold, 70.0);
    assert_eq!(
        mutation.measured,
        MutationMeasured {
            killed: 11,
            survived: 2,
            timeout: 1,
            no_coverage: 3,
            compile_error: 1,
            runtime_error: 1,
            ignored: 2,
            total: 21,
            detected: 12,
            undetected: 5,
        },
        "计数逐字段保真（驼峰键 noCoverage / compileError / runtimeError 对位）"
    );
    let overrides = mutation.overrides.as_ref().expect("override 组不丢");
    assert_eq!(overrides.len(), 1);
    assert_eq!(overrides[0].glob, "src/app");
}

/// 边界：超长字符串与特殊字符（换行 / unicode / emoji）字段值保真往返。
#[test]
fn 超长与特殊字符字段值保真往返() {
    let long_message = "x".repeat(1200);
    let special = "错误: 断言失败 ✓\n第二行 🎯\ttab 制表";
    let json = format!(
        r#"{{
          "framework": "vitest",
          "root": ".",
          "timestamp": "2026-10-06T08:00:00Z",
          "exit_code": 1,
          "duration_ms": 5.0,
          "summary": {{ "total": 1, "passed": 0, "failed": 1, "skipped": 0 }},
          "error_cases": [
            {{
              "name": "长消息用例",
              "status": "failed",
              "errorMessage": "{error_message}",
              "stackTrace": "{stack_trace}"
            }}
          ]
        }}"#,
        error_message = serde_json::to_string(&long_message)
            .expect("转义")
            .trim_matches('"'),
        stack_trace = serde_json::to_string(&special)
            .expect("转义")
            .trim_matches('"'),
    );

    let parsed = parse_sub_report(&json).expect("特殊字符 JSON 应解析成功");
    let case = &parsed.error_cases[0];
    assert_eq!(
        case.error_message.as_deref(),
        Some(long_message.as_str()),
        "超长串保真"
    );
    assert_eq!(case.error_message.as_ref().map(String::len), Some(1200));
    assert_eq!(
        case.stack_trace.as_deref(),
        Some(special),
        "换行 / unicode / emoji 保真"
    );

    // 再出线往返无损
    let text = serde_json::to_string(&parsed).expect("序列化应成功");
    let roundtrip = parse_sub_report(&text).expect("再出线重解析应成功");
    assert_eq!(roundtrip, parsed, "特殊字符往返无损");
}

// ---------------------------------------------------------------------------
// parse_summary_report
// ---------------------------------------------------------------------------

/// 正向：真实形态 summary JSON（phase / command / timestamp / 四计数 /
/// conclusion / problems[] / coverage / plans[] / findings[]）解析出
/// SummaryReport 逐字段相等。
#[test]
fn 真实形态summary全字段解析逐字段相等() {
    let parsed = parse_summary_report(FULL_SUMMARY_JSON).expect("全字段 summary 应解析成功");

    assert_eq!(parsed.phase, "test-execution");
    assert_eq!(parsed.command, "dev-team test-execution");
    assert_eq!(parsed.timestamp, "2026-10-06T08:01:00.000Z");
    assert_eq!(parsed.duration_seconds, 14.0);
    assert_eq!(parsed.total, 12);
    assert_eq!(parsed.passed, 10);
    assert_eq!(parsed.failed, 2);
    assert_eq!(parsed.skipped, 0);
    assert_eq!(parsed.conclusion, Conclusion::Fail);
    assert_eq!(
        parsed.problems,
        vec![
            Problem {
                framework: "vite-plus".to_owned(),
                problem_type: ProblemType::TestFailure,
                message: "Test \"math > rejects NaN input\" failed: expected 1 to equal 2"
                    .to_owned(),
            },
            Problem {
                framework: "all".to_owned(),
                problem_type: ProblemType::CoverageFailure,
                message: "Coverage below threshold: functions 78.9% < 75%".to_owned(),
            },
        ],
        "problems 归并保真（type 三值 snake_case 线格式对位）"
    );
    let plans = &parsed.plans;
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].id, "vite-plus");
    assert_eq!(plans[0].path, "reports/test/vite-plus");
    assert_eq!(
        parsed.findings,
        Some(vec![
            "「vite-plus」2 项测试失败——聚类失败（疑似同模块回归或共同假设变化）; 明细见子报告 error_cases"
                .to_owned()
        ])
    );
}

/// 异常：conclusion 非法枚举值 → Err（pass / fail / error 封闭集拒识）。
#[test]
fn conclusion非法枚举值err() {
    for bad in ["\"green\"", "\"PASS\"", "\"\"", "42"] {
        let json = format!(
            r#"{{
              "phase": "test-execution", "command": "cmd", "timestamp": "2026-10-06T08:00:00Z",
              "duration_seconds": 1.0, "total": 0, "passed": 0, "failed": 0, "skipped": 0,
              "conclusion": {bad}, "problems": [], "plans": []
            }}"#
        );
        let err = parse_summary_report(&json).expect_err("非法 conclusion 应 Err");
        assert!(
            err.starts_with("汇总报告解析失败:"),
            "Err 记因面（汇总报告解析失败前缀），实际: {err}"
        );
    }
}

/// 边界：空 problems[] / 空 plans[] / 四计数全 0 的最小合法 summary 解析成功。
#[test]
fn 最小合法summary解析成功() {
    let minimal = r#"{
      "phase": "test-execution",
      "command": "desktop change run: test-execution gate",
      "timestamp": "2026-10-06T08:00:00Z",
      "duration_seconds": 0,
      "total": 0, "passed": 0, "failed": 0, "skipped": 0,
      "conclusion": "pass",
      "problems": [], "plans": []
    }"#;
    let parsed = parse_summary_report(minimal).expect("最小合法 summary 应解析成功");
    assert_eq!(parsed.conclusion, Conclusion::Pass);
    assert!(parsed.problems.is_empty() && parsed.plans.is_empty());
    assert_eq!(parsed.total, 0);

    // 往返无损
    let text = serde_json::to_string(&parsed).expect("序列化应成功");
    let value: serde_json::Value = serde_json::from_str(&text).expect("线格式可回读");
    let object = value.as_object().expect("顶层对象");
    assert!(
        !object.contains_key("coverage"),
        "最小 summary 无 coverage 键"
    );
    assert!(
        !object.contains_key("mutation"),
        "最小 summary 无 mutation 键"
    );
    assert!(
        !object.contains_key("findings"),
        "findings 为 None 时整键不出现"
    );
    let roundtrip = parse_summary_report(&text).expect("再出线重解析应成功");
    assert_eq!(roundtrip, parsed, "最小 summary 往返无损");
}

// ---------------------------------------------------------------------------
// 段级 mutation 报告宽容解析（desktop-fix-mutation-test / AC-4 / AC-9）：
// parse_cargo_mutants_seg（映射表；未知 outcome 计 runtimeError 保留原文）
// + parse_stryker_seg（outcome 词与 MutationMeasured 1:1）
// ---------------------------------------------------------------------------

use crate::model::{parse_cargo_mutants_seg, parse_stryker_seg, MutationTool};

/// cargo-mutants 全映射表样本（五 outcome 逐个归位 + 未知 outcome + 缺字段
/// 条目；`--json` 顶层对象携 `mutants` 数组形态）。
const CARGO_SEG_FULL_MAPPING: &str = r#"{
  "mutants": [
    { "outcome": "Caught", "file": "src/lib.rs", "line": 12 },
    { "outcome": "missed", "file": "src/lib.rs", "line": 30 },
    { "outcome": "TIMEOUT", "file": "src/util.rs", "line": 8 },
    { "outcome": "unviable", "file": "src/util.rs", "line": 44 },
    { "outcome": "not-mutated", "file": "src/gen.rs", "line": 3 },
    { "outcome": "NOT_MUTATED", "file": "src/gen.rs", "line": 9 },
    { "outcome": "mystery-outcome", "file": "src/x.rs", "line": 1 },
    { "file": "src/y.rs", "line": 2 }
  ],
  "duration_ms": 1234.5
}"#;

/// 正向：全映射表样本逐个归位（大小写 / `_` / `-` 不敏归一）；未知 outcome
/// （含缺 outcome 字段条目）计 `runtimeError`；`tool = CargoMutants`、
/// `duration_ms` 取段耗时、`total` 与各计数一致（AC-4 / AC-9 映射表样本）。
#[test]
fn parse_cargo_mutants_seg全映射表样本逐个归位() {
    let report = parse_cargo_mutants_seg(CARGO_SEG_FULL_MAPPING).expect("映射表样本应解析成功");
    assert_eq!(report.tool, MutationTool::CargoMutants, "执行器标识");
    assert_eq!(
        report.tool.as_str(),
        "cargo_mutants",
        "线格式 snake_case 词"
    );
    let measured = report.measured;
    assert_eq!(measured.killed, 1, "caught → killed（Caught 大小写归一）");
    assert_eq!(measured.survived, 1, "missed → survived");
    assert_eq!(measured.timeout, 1, "timeout（TIMEOUT 大小写归一）");
    assert_eq!(measured.compile_error, 1, "unviable → compileError");
    assert_eq!(
        measured.ignored, 2,
        "not-mutated → ignored（`-` / `_` 归一：NOT_MUTATED 同词）"
    );
    assert_eq!(
        measured.runtime_error, 2,
        "未知 outcome + 缺 outcome 字段条目均计 runtimeError（宽容不丢数据）"
    );
    assert_eq!(measured.total, 8, "total = 条目总数");
    assert_eq!(
        measured.killed
            + measured.survived
            + measured.timeout
            + measured.compile_error
            + measured.ignored
            + measured.runtime_error,
        measured.total,
        "各计数求和与 total 一致"
    );
    assert_eq!(
        measured.detected, 2,
        "detected = killed + timeout（一致化回填：1+1）"
    );
    assert_eq!(measured.undetected, 1, "undetected = survived + noCoverage");
    assert_eq!(report.duration_ms, 1234.5, "段耗时取报告 duration_ms");
}

/// 正向：计数面直读形态（顶层对象携 outcome 计数键，`not_mutated` 别名）
/// + summary 对象形态 + 顶层数组形态；`total` 缺失按各计数求和回填。
#[test]
fn parse_cargo_mutants_seg计数面与summary面与数组面三形态() {
    let top_face = r#"{
      "caught": 7, "missed": 2, "timeout": 1,
      "unviable": 0, "not_mutated": 3, "runtime_error": 1
    }"#;
    let report = parse_cargo_mutants_seg(top_face).expect("顶层计数面应解析成功");
    let measured = report.measured;
    assert_eq!(
        (
            measured.killed,
            measured.survived,
            measured.timeout,
            measured.compile_error,
            measured.ignored,
            measured.runtime_error,
        ),
        (7, 2, 1, 0, 3, 1),
        "顶层计数键直读（not_mutated 别名在案）"
    );
    assert_eq!(measured.total, 14, "total 缺失 → 按各计数求和回填");

    let summary_face = r#"{
      "summary": { "caught": 4, "missed": 1, "timeout": 0, "unviable": 1, "not-mutated": 2 },
      "totalMutants": 8,
      "durationMs": 99.5
    }"#;
    let report = parse_cargo_mutants_seg(summary_face).expect("summary 面应解析成功");
    assert_eq!(report.measured.killed, 4);
    assert_eq!(report.measured.ignored, 2);
    assert_eq!(report.measured.total, 8, "totalMutants 显式在案优先");
    assert_eq!(report.duration_ms, 99.5, "durationMs 驼峰键在案");

    let array_face = r#"[ { "outcome": "Caught" }, { "outcome": "missed" } ]"#;
    let report = parse_cargo_mutants_seg(array_face).expect("顶层数组应解析成功");
    assert_eq!(report.measured.total, 2);
    assert_eq!(report.duration_ms, 0.0, "未携耗时 → 0.0 缺省");
}

/// 异常：`raw` 非 JSON / 空串 / 顶层标量 / 结构破坏（无 mutants 数组且无
/// 计数面）→ 显式 `Err`（结构面严格、词面宽容的口径分工）。
#[test]
fn parse_cargo_mutants_seg结构破坏族逐一样本err() {
    for bad in [
        "",
        "not json at all",
        "{ \"mutants\": ",
        "42",
        "true",
        r#"{ "duration_ms": 10.0 }"#,
    ] {
        let result = parse_cargo_mutants_seg(bad);
        assert!(result.is_err(), "结构破坏样本 {bad:?} 应 Err");
        let error = result.expect_err("上一行已断言 Err");
        assert!(!error.is_empty(), "Err 记因非空");
    }
}

/// 边界：超长（1200 mutants）解析不炸；特殊字符（emoji 文件名 / 换行与 NUL
/// 转义 / Windows 反斜杠路径）保真；未知字段多余 → 忽略不 Err。
#[test]
fn parse_cargo_mutants_seg超长与特殊字符与未知字段宽容() {
    let mut huge = String::from("{\"mutants\":[");
    for index in 0..1200 {
        if index > 0 {
            huge.push(',');
        }
        let outcome = if index % 2 == 0 { "Caught" } else { "missed" };
        huge.push_str(&format!(
            "{{\"outcome\":\"{outcome}\",\"file\":\"src/f{index}.rs\"}}"
        ));
    }
    huge.push_str("]}");
    assert!(huge.len() > 10_000, "样本体量在案（>10KB）");
    let report = parse_cargo_mutants_seg(&huge).expect("超长样本应解析成功");
    assert_eq!(report.measured.total, 1200);
    assert_eq!(report.measured.killed, 600);
    assert_eq!(report.measured.survived, 600);

    let special = r#"{
      "unknownExtra": { "nested": [1, 2, 3] },
      "mutants": [
        { "outcome": "Caught", "file": "src/文件 🚀.rs", "note": "line\\nbreak zero" },
        { "outcome": "missed", "file": "C:\\win\\path.rs", "extra": true }
      ],
      "durationMs": 1.5
    }"#;
    let report = parse_cargo_mutants_seg(special).expect("特殊字符样本应解析成功");
    assert_eq!(report.measured.total, 2, "特殊字符与未知字段不扰计数");
    assert_eq!(report.duration_ms, 1.5);
}

/// Stryker fixture：metrics 聚合面（全 outcome 词面在案）。
const STRYKER_SEG_METRICS: &str = r#"{
  "metrics": {
    "killed": 5, "survived": 2, "timeout": 1,
    "noCoverage": 3, "compileErrors": 1, "runtimeErrors": 1,
    "ignored": 1, "totalMutants": 14,
    "totalDetected": 6, "totalUndetected": 5
  },
  "durationMs": 2500.0
}"#;

/// 正向：Stryker metrics 面 outcome 词与 `MutationMeasured` 1:1 归位（含
/// `compileError` / `runtimeError` / `ignored` / `noCoverage`），`tool =
/// Stryker`（AC-4 Stryker 面）。
#[test]
fn parse_stryker_seg_metrics面1比1归位() {
    let report = parse_stryker_seg(STRYKER_SEG_METRICS).expect("metrics 面应解析成功");
    assert_eq!(report.tool, MutationTool::Stryker, "执行器标识");
    assert_eq!(report.tool.as_str(), "stryker", "线格式 snake_case 词");
    let measured = report.measured;
    assert_eq!(measured.killed, 5);
    assert_eq!(measured.survived, 2);
    assert_eq!(measured.timeout, 1);
    assert_eq!(measured.no_coverage, 3, "noCoverage 键在案");
    assert_eq!(measured.compile_error, 1, "compileErrors 复数键在案");
    assert_eq!(measured.runtime_error, 1, "runtimeErrors 复数键在案");
    assert_eq!(measured.ignored, 1);
    assert_eq!(measured.total, 14, "totalMutants 显式在案");
    assert_eq!(measured.detected, 6, "totalDetected 显式在案");
    assert_eq!(measured.undetected, 5, "totalUndetected 显式在案");
    assert_eq!(report.duration_ms, 2500.0, "durationMs 顶层面在案");
}

/// 正向：files 逐 mutant 计数面（metrics 缺位回落）——status 词 1:1，未知
/// status 计 runtimeError、缺 status 条目同样计 total；耗时取 mutant duration
/// 求和（未携 durationMs 时）。
#[test]
fn parse_stryker_seg_files逐mutant计数面与未知status宽容() {
    let files_face = r#"{
      "files": {
        "src/lib.rs": {
          "mutants": [
            { "status": "Killed", "duration": 100.5 },
            { "status": "survived", "duration": 50.0 },
            { "status": "Timeout" },
            { "status": "NoCoverage" },
            { "status": "CompileError" },
            { "status": "RuntimeError" },
            { "status": "Ignored" },
            { "status": "weird-status", "duration": 7.5 },
            { "duration": 3.0 }
          ]
        },
        "src/util.rs": {
          "mutants": [
            { "status": "Killed", "duration": 20.0 }
          ]
        }
      }
    }"#;
    let report = parse_stryker_seg(files_face).expect("files 面应解析成功");
    let measured = report.measured;
    assert_eq!(measured.killed, 2, "Killed 大小写归一（两文件求和）");
    assert_eq!(measured.survived, 1);
    assert_eq!(measured.timeout, 1);
    assert_eq!(measured.no_coverage, 1);
    assert_eq!(measured.compile_error, 1);
    assert_eq!(
        measured.runtime_error, 3,
        "RuntimeError 词 + 未知 status + 缺 status 合计"
    );
    assert_eq!(measured.ignored, 1);
    assert_eq!(measured.total, 10, "逐条计数（缺 status 条目同样计 total）");
    assert_eq!(
        report.duration_ms, 181.0,
        "mutant duration 求和（100.5+50+7.5+3+20）"
    );
}

/// 异常：Stryker 非 JSON / 顶层非对象 / 两计数面（metrics / files）俱缺 →
/// 显式 `Err`（结构面严格、词面宽容同口径）。
#[test]
fn parse_stryker_seg结构破坏族逐一样本err() {
    for bad in [
        "",
        "garbage text",
        "[1, 2, 3]",
        r#"{ "metrics": {} }"#,
        r#"{ "durationMs": 1.0 }"#,
    ] {
        let result = parse_stryker_seg(bad);
        assert!(result.is_err(), "结构破坏样本 {bad:?} 应 Err");
        let error = result.expect_err("上一行已断言 Err");
        assert!(!error.is_empty(), "Err 记因非空");
    }
}

/// 边界：Stryker 超长（1100 mutants）解析不炸；特殊字符（CRLF / emoji /
/// 反斜杠路径）保真；未知字段忽略不 Err。
#[test]
fn parse_stryker_seg超长与特殊字符宽容() {
    let mut huge = String::from("{\"files\":{\"src/lib.rs\":{\"mutants\":[");
    for index in 0..1100 {
        if index > 0 {
            huge.push(',');
        }
        let status = if index % 2 == 0 { "killed" } else { "survived" };
        huge.push_str(&format!("{{\"status\":\"{status}\"}}"));
    }
    huge.push_str("]}}}");
    let report = parse_stryker_seg(&huge).expect("超长样本应解析成功");
    assert_eq!(report.measured.total, 1100);
    assert_eq!(report.measured.killed, 550);

    let special = r#"{
      "unexpected": "field",
      "files": {
        "src/文件 🚀.ts": {
          "mutants": [ { "status": "killed", "note": "crlf\r\nemoji 🚀" } ]
        }
      }
    }"#;
    let report = parse_stryker_seg(special).expect("特殊字符样本应解析成功");
    assert_eq!(report.measured.total, 1, "特殊字符与未知字段不扰计数");
}

/// 边界：`MutationSegReport` / `MutationTool` 类型面——`MutationMeasured` 既有
/// 字段零扩展（穷尽解构无 `..` 兜底——字段增删即编译失败，corpus `mutation-
/// block` fixture 口径不变）；计数全 0 / `u64` 大值构造不 panic。
#[test]
fn mutation_seg_report类型面_零扩展与大值构造() {
    let mut words = vec![
        MutationTool::CargoMutants.as_str(),
        MutationTool::Stryker.as_str(),
    ];
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), 2, "执行器标识两值互异");

    let measured = MutationMeasured {
        killed: 1,
        survived: 2,
        timeout: 3,
        no_coverage: 4,
        compile_error: 5,
        runtime_error: 6,
        ignored: 7,
        total: 28,
        detected: 4,
        undetected: 6,
    };
    let MutationMeasured {
        killed,
        survived,
        timeout,
        no_coverage,
        compile_error,
        runtime_error,
        ignored,
        total,
        detected,
        undetected,
    } = measured;
    assert_eq!((killed, survived, timeout, no_coverage), (1, 2, 3, 4));
    assert_eq!(
        (compile_error, runtime_error, ignored, total),
        (5, 6, 7, 28)
    );
    assert_eq!((detected, undetected), (4, 6));

    assert_eq!(MutationMeasured::default().total, 0, "Default 零占位");
    let huge = MutationMeasured {
        killed: u64::MAX,
        survived: u64::MAX,
        timeout: u64::MAX,
        no_coverage: u64::MAX,
        compile_error: u64::MAX,
        runtime_error: u64::MAX,
        ignored: u64::MAX,
        total: u64::MAX,
        detected: u64::MAX,
        undetected: u64::MAX,
    };
    assert_eq!(huge.total, u64::MAX, "u64 大值构造不 panic");

    let report = crate::model::MutationSegReport {
        tool: MutationTool::Stryker,
        measured,
        duration_ms: 0.0,
    };
    assert_eq!(report.duration_ms, 0.0, "段耗时缺省 0.0");
    assert_eq!(report.tool, MutationTool::Stryker);
}

// ---------------------------------------------------------------------------
// SummaryReport 瘦身面（AC-2 / AC-13）：两字段退役后的序列化键集与旧报告宽容
// ---------------------------------------------------------------------------

/// 汇总报告内存构造（全部字段显式——序列化键面 fixture）。
fn summary_report(
    problems: Vec<Problem>,
    plans: Vec<crate::model::PlanIndexEntry>,
    findings: Option<Vec<String>>,
) -> crate::model::SummaryReport {
    crate::model::SummaryReport {
        phase: "test-execution".to_owned(),
        command: "desktop change run: test-execution gate".to_owned(),
        timestamp: "2026-10-06T08:00:00Z".to_owned(),
        duration_seconds: 0.0,
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
        conclusion: Conclusion::Pass,
        problems,
        plans,
        findings,
    }
}

/// 正向（AC-2）：序列化瘦身——顶层键集恰 11 个必在键，`findings` 为 `None`
/// 时整键不出现，`coverage` / `mutation` 两键缺席。
#[test]
fn 汇总序列化瘦身键集() {
    let summary = summary_report(Vec::new(), Vec::new(), None);
    let value = serde_json::to_value(&summary).expect("汇总应可序列化");
    let object = value.as_object().expect("顶层对象");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();

    assert_eq!(
        keys,
        vec![
            "command",
            "conclusion",
            "duration_seconds",
            "failed",
            "passed",
            "phase",
            "plans",
            "problems",
            "skipped",
            "timestamp",
            "total",
        ],
        "12 字段中 findings 缺省不出现 → 11 键"
    );
    assert!(!object.contains_key("coverage"));
    assert!(!object.contains_key("mutation"));

    let with_findings = summary_report(Vec::new(), Vec::new(), Some(vec!["正向锚".to_owned()]));
    let value = serde_json::to_value(&with_findings).expect("序列化应成功");
    assert_eq!(
        value["findings"],
        serde_json::json!(["正向锚"]),
        "findings 在场时序列化"
    );
}

/// 正向：往返无损——三类 problems + 一条 plans + findings 在场的汇总经出线
/// 再解析逐字段相等。
#[test]
fn 汇总往返无损() {
    let summary = summary_report(
        vec![
            Problem {
                framework: "vite-plus".to_owned(),
                problem_type: ProblemType::TestFailure,
                message: "Test \"math > a\" failed: boom".to_owned(),
            },
            Problem {
                framework: "rust".to_owned(),
                problem_type: ProblemType::CoverageFailure,
                message: "Coverage below threshold: functions 87.6% < 90%".to_owned(),
            },
            Problem {
                framework: "go".to_owned(),
                problem_type: ProblemType::ExecutionError,
                message: "Exit code 2".to_owned(),
            },
        ],
        vec![crate::model::PlanIndexEntry {
            id: "crates_rust".to_owned(),
            framework: "rust".to_owned(),
            root: "crates".to_owned(),
            path: "reports/test/crates_rust".to_owned(),
        }],
        Some(vec!["覆盖判定未通过 [rust]: 缺口留痕".to_owned()]),
    );

    let text = serde_json::to_string(&summary).expect("序列化应成功");
    let roundtrip = parse_summary_report(&text).expect("再出线重解析应成功");
    assert_eq!(roundtrip, summary, "汇总往返逐字段相等");
}

/// 正向（AC-13）：旧报告仍可读——历史 `summary.json`（含 `coverage` 块与
/// `"mutation": null`）解析成功，两键被忽略（零残留、零 `Err`），problems /
/// plans / findings 逐字保真。
#[test]
fn 旧报告带两键仍可解析() {
    let parsed = parse_summary_report(FULL_SUMMARY_JSON).expect("旧报告应宽容解析成功");
    assert_eq!(parsed.conclusion, Conclusion::Fail);
    assert_eq!(parsed.problems.len(), 2, "两类 problem 逐字保真");
    assert_eq!(
        parsed.problems[1].message,
        "Coverage below threshold: functions 78.9% < 75%"
    );
    assert_eq!(parsed.plans.len(), 1);
    assert_eq!(parsed.plans[0].path, "reports/test/vite-plus");
    assert!(parsed.findings.is_some(), "findings 保真");

    let text = serde_json::to_string(&parsed).expect("序列化应成功");
    let value: serde_json::Value = serde_json::from_str(&text).expect("线格式可回读");
    let object = value.as_object().expect("顶层对象");
    assert!(!object.contains_key("coverage"), "旧键不再出线");
    assert!(!object.contains_key("mutation"), "旧键不再出线");
}

/// 异常（AC-13 宽容边界，4 形态）：旧报告两键畸形（字符串 / 数组 / 数值 /
/// 对象）仍 `Ok`——未知字段宽容不因旧键形态翻面。
#[test]
fn 旧报告两键畸形仍宽容() {
    for legacy in [
        r#" "coverage": "oops", "#,
        r#" "coverage": [], "#,
        r#" "mutation": 42, "#,
        r#" "mutation": {}, "#,
    ] {
        let json = format!(
            r#"{{
              "phase": "test-execution", "command": "cmd", "timestamp": "2026-10-06T08:00:00Z",
              "duration_seconds": 1.0, "total": 1, "passed": 1, "failed": 0, "skipped": 0,
              "conclusion": "pass",
              {legacy}
              "problems": [], "plans": []
            }}"#
        );
        let parsed = parse_summary_report(&json)
            .unwrap_or_else(|error| panic!("旧键形态 {legacy} 应宽容解析: {error}"));
        assert_eq!(parsed.total, 1);
    }
}

/// 边界（2 形态）：极值字段——`duration_seconds = 0` 与四计数 `u64::MAX` 两
/// 形态解析与往返保真、零饱和。
#[test]
fn 极值字段往返保真() {
    let mut zero_duration = summary_report(Vec::new(), Vec::new(), None);
    zero_duration.duration_seconds = 0.0;
    let text = serde_json::to_string(&zero_duration).expect("序列化应成功");
    let parsed = parse_summary_report(&text).expect("解析应成功");
    assert_eq!(parsed.duration_seconds, 0.0);

    let mut saturated = summary_report(Vec::new(), Vec::new(), None);
    saturated.total = u64::MAX;
    saturated.passed = u64::MAX;
    saturated.failed = u64::MAX;
    saturated.skipped = u64::MAX;
    let text = serde_json::to_string(&saturated).expect("序列化应成功");
    let parsed = parse_summary_report(&text).expect("解析应成功");
    assert_eq!(parsed, saturated, "极大计数往返无损（零饱和）");
}

/// 边界（3 形态）：新形态反序列化缺省臂——缺 `problems` / `plans` /
/// `findings` 的 JSON → `Vec::new()` / `Vec::new()` / `None`。
#[test]
fn 新形态缺省臂三形态() {
    let base = r#""phase": "test-execution", "command": "cmd", "timestamp": "2026-10-06T08:00:00Z", "duration_seconds": 1.0, "total": 0, "passed": 0, "failed": 0, "skipped": 0, "conclusion": "pass""#;
    for extra in [
        r#", "plans": []"#,
        r#", "problems": []"#,
        r#", "problems": [], "plans": []"#,
    ] {
        let json = format!("{{ {base}{extra} }}");
        let parsed = parse_summary_report(&json).expect("缺省字段应取默认值");
        assert!(parsed.problems.is_empty(), "problems 缺省为空集: {extra}");
        assert!(parsed.plans.is_empty(), "plans 缺省为空集: {extra}");
        assert_eq!(parsed.findings, None, "findings 缺省为 None: {extra}");
    }
}

/// 正向：`Problem` 线格式恰三键（`framework` / `type` / `message`），三类
/// 问题类型 snake_case 对位往返保真。
#[test]
fn problem序列化恰三键() {
    for (kind, wire) in [
        (ProblemType::TestFailure, "test_failure"),
        (ProblemType::CoverageFailure, "coverage_failure"),
        (ProblemType::ExecutionError, "execution_error"),
    ] {
        let problem = Problem {
            framework: "rust".to_owned(),
            problem_type: kind,
            message: "m".to_owned(),
        };
        let value = serde_json::to_value(&problem).expect("序列化应成功");
        let object = value.as_object().expect("对象");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["framework", "message", "type"]);
        assert_eq!(value["type"], serde_json::json!(wire));
        let roundtrip: Problem = serde_json::from_value(value).expect("回读应成功");
        assert_eq!(roundtrip, problem);
    }
}

/// 正向：`SourceFileEntry.coverage` 两态（`None` / `Some`）与
/// `CoverageOverride` 七字段往返保真。
#[test]
fn 逐文件条目与override组往返() {
    let no_coverage = crate::model::SourceFileEntry {
        file: "src/app/main.rs".to_owned(),
        coverage: None,
    };
    let text = serde_json::to_string(&no_coverage).expect("序列化应成功");
    let roundtrip: crate::model::SourceFileEntry = serde_json::from_str(&text).expect("回读应成功");
    assert_eq!(roundtrip, no_coverage, "coverage 缺省态往返");

    let override_group = CoverageOverride {
        glob: "src/**/*.rs".to_owned(),
        thresholds: CoverageThresholds {
            lines: 80.0,
            branches: 70.0,
            functions: 75.0,
        },
        measured: CoverageMeasured {
            lines: Some(90.0),
            branches: None,
            functions: Some(88.0),
        },
        pass: true,
        file_count: 7,
        passed_count: 6,
    };
    let text = serde_json::to_string(&override_group).expect("序列化应成功");
    let roundtrip: CoverageOverride = serde_json::from_str(&text).expect("回读应成功");
    assert_eq!(roundtrip, override_group, "override 组七字段逐项保真");
}
