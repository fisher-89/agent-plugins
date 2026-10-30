use serde_json::Value;

use crate::model::{FileCoverageEntry, SourceFileEntry, TestCaseResult, TestCaseStatus};

use super::text::split_error_head;

/// 解析 istanbul `coverage-summary.json` 文本为逐文件覆盖原始计数条目。
pub fn parse_istanbul_summary(text: &str) -> Result<Vec<SourceFileEntry>, String> {
    let raw: Value =
        serde_json::from_str(text).map_err(|error| format!("istanbul 覆盖解析失败: {error}"))?;
    let map = raw
        .as_object()
        .ok_or_else(|| "istanbul 覆盖解析失败: 顶层必须为对象".to_owned())?;

    // total 键结构校验（三维度 pct 必需——全局实测块的解析前提）
    let total = map
        .get("total")
        .ok_or_else(|| "istanbul 覆盖解析失败: 缺 total 键".to_owned())?;
    for dimension in ["lines", "branches", "functions"] {
        total
            .get(dimension)
            .and_then(|metric| metric.get("pct"))
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("istanbul 覆盖解析失败: total.{dimension} 缺 pct 计数字段"))?;
    }

    let mut entries = Vec::new();
    for (file, metrics) in map {
        if file == "total" {
            continue;
        }
        entries.push(SourceFileEntry {
            file: file.clone(),
            coverage: Some(parse_file_metrics(file, metrics)?),
        });
    }
    Ok(entries)
}

/// 单文件键 → 覆盖计数条目：`lines` 块必需（残缺即 `Err`，不产半截条目），
/// `branches` / `functions` 块可选（缺省维度为 `None`）。
fn parse_file_metrics(file: &str, metrics: &Value) -> Result<FileCoverageEntry, String> {
    let invalid =
        || format!("istanbul 覆盖解析失败: 文件 \"{file}\" 键结构残缺（缺 lines 计数字段）");
    let lines = metrics.get("lines").ok_or_else(invalid)?;
    let pct = lines
        .get("pct")
        .and_then(Value::as_f64)
        .ok_or_else(invalid)?;
    let metric_count =
        |metric: Option<&Value>, key: &str| -> Option<u64> { metric?.get(key)?.as_u64() };

    Ok(FileCoverageEntry {
        lines: pct,
        branches: metrics
            .get("branches")
            .and_then(|metric| metric.get("pct"))
            .and_then(Value::as_f64),
        functions: metrics
            .get("functions")
            .and_then(|metric| metric.get("pct"))
            .and_then(Value::as_f64),
        total_lines: metric_count(Some(lines), "total"),
        covered_lines: metric_count(Some(lines), "covered"),
        total_branches: metric_count(metrics.get("branches"), "total"),
        covered_branches: metric_count(metrics.get("branches"), "covered"),
        total_functions: metric_count(metrics.get("functions"), "total"),
        covered_functions: metric_count(metrics.get("functions"), "covered"),
    })
}

/// 解析 istanbul 族（jest / vitest / vite-plus）`--reporter=json` 的
/// `results.json` 为用例行。
///
/// 读侧契约（jest 兼容 JSON 报告器，三家同形；样本见
/// `tests/fixtures/istanbul/results.json`）：
/// - `testResults[]` 每个元素是一个测试文件：`name` 为绝对路径、`status`
///   为文件级结论、`assertionResults[]` 为该文件的用例行；
/// - 用例行 `fullName` 是 `ancestorTitles + title` 的空格拼接（与 libtest 的
///   `{crate}::{binary}${test}` 同理，保留层级以便失败归因定位）；
/// - `status` 四值 → 三态：`passed` → Passed、`failed` → Failed、
///   `pending` / `skipped` / `todo` / `disabled` → Skipped；
/// - `duration` 已是毫秒（不是 libtest 的秒）；
/// - `failureMessages[]` 每条是一段多行文本：首行 `错误类型: 消息`，全量入
///   `stack_trace`。
///
/// 两条不并入用例行的**文件级失败**必须单独暴露，否则「加载即炸」的测试文件
/// （ts 转译失败 / 顶层抛错，`assertionResults` 为空）只会表现为零用例——这在
/// runner 的零用例守卫下会变成「工件零用例」的误归因，把「文件跑不起来」说成
/// 「报告器写空」：
/// - 文件 `status == "failed"` 且 `message` 非空 → [`IstanbulResults::file_failures`]；
/// - 文件 `status == "failed"` 但零用例且无 message → 同样记一条（兜底措辞）。
///
/// 顶层 `numTotalTests` 与逐用例计数对账：[`IstanbulResults::mismatch`] 非空
/// 表示报告自相矛盾（用例行数 ≠ 汇总数），交调用方记 finding。调用方按保守
/// 方向定型为 `execution_error`——计数漂移说明读侧与报告器口径已不一致，
/// 此时「全绿」的计数不可信，宁可判不可测（119 份归档真件与现网报告均一致，
/// 该分支只在格式漂移时触发）。
#[derive(Debug, Clone, PartialEq)]
pub struct IstanbulResults {
    /// 用例行（全量，按报告顺序）
    pub cases: Vec<TestCaseResult>,
    /// 文件级失败（加载 / 转译 / 顶层抛错）：`（文件路径, 首行消息）`
    pub file_failures: Vec<(String, String)>,
    /// 报告自相矛盾说明（`numTotalTests` ≠ 用例数）；`None` = 对账一致
    pub mismatch: Option<String>,
}

/// 解析 istanbul 族结果报告（语法 / 必需键错误 → `Err`；语义面见
/// [`IstanbulResults`]）。
pub fn parse_istanbul_results(text: &str) -> Result<IstanbulResults, String> {
    let raw: Value =
        serde_json::from_str(text).map_err(|error| format!("istanbul 结果解析失败: {error}"))?;
    let suites = raw
        .get("testResults")
        .ok_or_else(|| "istanbul 结果解析失败: 缺 testResults 键".to_owned())?
        .as_array()
        .ok_or_else(|| "istanbul 结果解析失败: testResults 必须为数组".to_owned())?;

    let mut cases: Vec<TestCaseResult> = Vec::new();
    let mut file_failures: Vec<(String, String)> = Vec::new();

    for suite in suites {
        let file = suite
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let assertions = suite
            .get("assertionResults")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for assertion in assertions {
            cases.push(assertion_case(assertion, file));
        }
        let suite_failed = suite.get("status").and_then(Value::as_str) == Some("failed");
        if suite_failed {
            let message = suite
                .get("message")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|message| !message.is_empty());
            match message {
                Some(message) => {
                    let head = message.lines().next().unwrap_or_default().trim().to_owned();
                    file_failures.push((file.to_owned(), head));
                }
                // 文件判红却零用例且无 message：报告自身缺归因原料，仍必须
                // 让「这个文件没跑起来」可见
                None if assertions.is_empty() => file_failures.push((
                    file.to_owned(),
                    "测试文件失败但报告未给出错误消息".to_owned(),
                )),
                None => {}
            }
        }
    }

    let mismatch = raw
        .get("numTotalTests")
        .and_then(Value::as_u64)
        .and_then(|total| {
            let parsed = cases.len() as u64;
            (total != parsed).then(|| {
                format!("istanbul 结果计数漂移: numTotalTests = {total}，用例行 = {parsed}")
            })
        });

    Ok(IstanbulResults {
        cases,
        file_failures,
        mismatch,
    })
}

/// 单条断言 → 用例行。
fn assertion_case(assertion: &Value, file: &str) -> TestCaseResult {
    let status = match assertion.get("status").and_then(Value::as_str) {
        Some("passed") => TestCaseStatus::Passed,
        Some("failed") => TestCaseStatus::Failed,
        // pending / skipped / todo / disabled 与未知值一律按跳过（不产假绿：
        // 跳过不参与 failed，但也不计入 passed）
        _ => TestCaseStatus::Skipped,
    };
    let name = assertion
        .get("fullName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .or_else(|| assertion.get("title").and_then(Value::as_str))
        .unwrap_or_default()
        .to_owned();
    let duration_ms = assertion.get("duration").and_then(Value::as_f64);
    let failure = assertion
        .get("failureMessages")
        .and_then(Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .filter_map(Value::as_str)
                .filter(|message| !message.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|message| !message.is_empty());
    let (error_type, error_message, stack_trace) = match (status, failure) {
        (TestCaseStatus::Failed, Some(message)) => {
            let first = message.lines().next().unwrap_or_default().trim();
            let (error_type, error_message) = split_error_head(first);
            (Some(error_type), Some(error_message), Some(message))
        }
        (TestCaseStatus::Failed, None) => (None, None, None),
        _ => (None, None, None),
    };

    TestCaseResult {
        name,
        file: (!file.is_empty()).then(|| file.to_owned()),
        duration_ms,
        status,
        line: None,
        error_type,
        error_message,
        stack_trace,
    }
}
