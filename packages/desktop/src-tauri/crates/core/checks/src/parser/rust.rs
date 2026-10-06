use serde_json::Value;

use crate::model::{FileCoverageEntry, SourceFileEntry};

/// 解析 llvm-cov JSON 文本为逐文件覆盖原始计数条目（文件名原样保留）。
pub fn parse_llvm_cov(text: &str) -> Result<Vec<SourceFileEntry>, String> {
    let raw: Value =
        serde_json::from_str(text).map_err(|error| format!("llvm-cov 覆盖解析失败: {error}"))?;
    let data = raw
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "llvm-cov 覆盖解析失败: 缺 data 键".to_owned())?;

    let mut entries = Vec::new();
    for window in data {
        let files = window
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(|| "llvm-cov 覆盖解析失败: data 窗口缺 files 键".to_owned())?;
        for file in files {
            entries.push(parse_file_entry(file)?);
        }
    }
    Ok(entries)
}

/// 单文件条目 → 覆盖计数：segments 推导行覆盖（无 segments → 计数 0 保真，
/// 不 panic）；分支 / 函数维度不产逐文件计数，恒 `None`。
fn parse_file_entry(file: &Value) -> Result<SourceFileEntry, String> {
    let filename = file
        .get("filename")
        .and_then(Value::as_str)
        .ok_or_else(|| "llvm-cov 覆盖解析失败: 文件条目缺 filename".to_owned())?
        .to_owned();
    let segments = file
        .get("segments")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("llvm-cov 覆盖解析失败: 文件 \"{filename}\" 缺 segments 键"))?;

    let mut total_lines: u64 = 0;
    let mut covered_lines: u64 = 0;
    let mut counted: Vec<u64> = Vec::new();
    let mut covered_marked: Vec<u64> = Vec::new();
    for segment in segments {
        let Some(row) = segment.get(0).and_then(Value::as_u64) else {
            continue;
        };
        let has_count = segment.get(3).and_then(Value::as_bool).unwrap_or(false);
        if !has_count {
            continue;
        }
        let count = segment.get(2).and_then(Value::as_u64).unwrap_or(0);
        if !counted.contains(&row) {
            counted.push(row);
            total_lines += 1;
        }
        if count > 0 && !covered_marked.contains(&row) {
            covered_marked.push(row);
            covered_lines += 1;
        }
    }
    let pct = if total_lines > 0 {
        round_two(covered_lines as f64 / total_lines as f64 * 100.0)
    } else {
        0.0
    };

    Ok(SourceFileEntry {
        file: filename,
        coverage: Some(FileCoverageEntry {
            lines: pct,
            branches: None,
            functions: None,
            total_lines: Some(total_lines),
            covered_lines: Some(covered_lines),
            total_branches: None,
            covered_branches: None,
            total_functions: None,
            covered_functions: None,
        }),
    })
}

/// 百分点两位小数舍入（与 CLI 覆盖聚合同口径）。
pub(crate) fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
