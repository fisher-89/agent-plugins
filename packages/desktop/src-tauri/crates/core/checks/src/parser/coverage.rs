use serde_json::Value;

use crate::model::{CoverageMeasured, SourceFileEntry};

/// 覆盖格式三值（V1 支持面封闭集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageFormat {
    /// istanbul 族（jest / vitest / vite-plus，`coverage-summary.json`）
    Istanbul,
    /// llvm-cov（cargo llvm-cov `--json`）
    LlvmCov,
    /// node-test 文本表格（`--experimental-test-coverage` spec 输出）
    NodeTest,
}

impl CoverageFormat {
    /// 线格式词（CLI `coverage_format` 字段同串）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Istanbul => "istanbul",
            Self::LlvmCov => "llvm-cov",
            Self::NodeTest => "node-test",
        }
    }
}

/// 按覆盖格式路由至子解析器，产出逐文件覆盖原始计数条目。
///
/// istanbul 路由 [`crate::parser::js::parse_istanbul_summary`]、llvm-cov 路
/// 由 [`crate::parser::rust::parse_llvm_cov`]；node-test 无逐文件计数、恒空
/// 集（实测块走 [`parse_coverage_measured`]）。路由层不吞错不换语义——结构
/// 残缺样本经各路由 `Err` 原样上抛。
pub fn parse_coverage(format: CoverageFormat, text: &str) -> Result<Vec<SourceFileEntry>, String> {
    match format {
        CoverageFormat::Istanbul => crate::parser::js::parse_istanbul_summary(text),
        CoverageFormat::LlvmCov => crate::parser::rust::parse_llvm_cov(text),
        CoverageFormat::NodeTest => Ok(Vec::new()),
    }
}

/// 按覆盖格式提取全局实测块（子报告 coverage 块的 measured 半边）。
///
/// istanbul 取 `total` 三维度 pct；llvm-cov 取 `data[0].totals` 三维度
/// percent（缺维度 `None`）；node-test 自文本表格 "All files" 汇总行提取
/// 百分比（`-` 列为 `None`）。无法提取（结构残缺 / 无汇总行）→ `Ok(None)`
///（覆盖采集失败语义，判定按 null 感知跳过）。
pub fn parse_coverage_measured(format: CoverageFormat, text: &str) -> Option<CoverageMeasured> {
    match format {
        CoverageFormat::Istanbul => istanbul_measured(text),
        CoverageFormat::LlvmCov => llvm_cov_measured(text),
        CoverageFormat::NodeTest => node_test_measured(text),
    }
}

/// istanbul `total` 键 → 实测块（结构残缺 → `None`）。
fn istanbul_measured(text: &str) -> Option<CoverageMeasured> {
    let raw: Value = serde_json::from_str(text).ok()?;
    let total = raw.get("total")?;
    Some(CoverageMeasured {
        lines: total.pointer("/lines/pct").and_then(Value::as_f64),
        branches: total.pointer("/branches/pct").and_then(Value::as_f64),
        functions: total.pointer("/functions/pct").and_then(Value::as_f64),
    })
}

/// llvm-cov `data[0].totals` → 实测块（结构残缺 → `None`）。
fn llvm_cov_measured(text: &str) -> Option<CoverageMeasured> {
    let raw: Value = serde_json::from_str(text).ok()?;
    let totals = raw.pointer("/data/0/totals")?;
    Some(CoverageMeasured {
        lines: totals.pointer("/lines/percent").and_then(Value::as_f64),
        branches: totals.pointer("/branches/percent").and_then(Value::as_f64),
        functions: totals.pointer("/functions/percent").and_then(Value::as_f64),
    })
}

/// node-test 文本表格 "All files" 汇总行 → 实测块。
///
/// 行形态：`| All files | 82.5 | 74.1 | 90.3 |`（`-` 列为该维度不支持）。
fn node_test_measured(text: &str) -> Option<CoverageMeasured> {
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("All files") {
            continue;
        }
        let columns: Vec<&str> = trimmed.split('|').map(str::trim).collect();
        if columns.len() < 4 {
            continue;
        }
        let pct_lines = parse_pct(columns[2])?;
        return Some(CoverageMeasured {
            lines: Some(pct_lines),
            branches: columns.get(3).and_then(|column| parse_pct(column)),
            functions: columns.get(4).and_then(|column| parse_pct(column)),
        });
    }
    None
}

/// 百分比单元格解析：剥 `%`、`-`（不支持维度）为 `None`。
fn parse_pct(value: &str) -> Option<f64> {
    let cleaned = value.replace('%', "").trim().to_owned();
    if cleaned.is_empty() || cleaned == "-" {
        return None;
    }
    cleaned.parse::<f64>().ok()
}
