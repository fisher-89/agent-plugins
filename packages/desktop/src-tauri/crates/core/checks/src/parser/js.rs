use serde_json::Value;

use crate::model::{FileCoverageEntry, SourceFileEntry};

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
