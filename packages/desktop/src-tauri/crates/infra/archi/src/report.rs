//! 报告落盘单点（本 crate 唯一写盘面）：目录派生 + 目录创建 + 文件名 + 序列化
//! 写盘。
//!
//! 目录经 [`foundation::layout::change_reports`] 的 `ReportSlot::Archi` 派生（零
//! 行内路径字面量、MUST NOT 复用 `reports/test` / `reports/baseline`）；文件名取
//! 报告 `timestamp` 的紧凑形态（`<YYYYMMDDTHHMMSSZ>.json`，**无冒号**——跨平台
//! 合法）。

use std::path::Path;

use archi::drift::ArchiDriftReport;
use foundation::layout::{change_reports, ReportSlot};

/// 报告落盘：返回写成的文件路径。
///
/// 时序：先 `create_dir_all` 再整份 `serde_json::to_vec_pretty` 一次 `fs::write`
///（无半写拼接）。同一次门禁一份；重跑覆盖不同时间戳、不删旧报告（与
/// `reports/test` 同口径的堆积）。
pub(crate) fn write_report(
    root: &Path,
    name: &str,
    report: &ArchiDriftReport,
) -> Result<String, String> {
    let directory = change_reports(root, name, ReportSlot::Archi);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("架构报告目录创建失败（{}）：{error}", directory.display()))?;
    let file = directory.join(format!("{}.json", file_stem(&report.timestamp)));
    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("架构报告序列化失败：{error}"))?;
    std::fs::write(&file, bytes)
        .map_err(|error| format!("架构报告写入失败（{}）：{error}", file.display()))?;
    Ok(file.to_string_lossy().into_owned())
}

/// 时间戳 → 文件名主干：只保留 ASCII 数字与 `T` / `Z`（`2026-10-20T08:31:12Z` →
/// `20261020T083112Z`；冒号在 Windows 文件名上非法）。空输入退化为 `report`。
fn file_stem(timestamp: &str) -> String {
    let stem: String = timestamp
        .chars()
        .filter(|character| character.is_ascii_digit() || *character == 'T' || *character == 'Z')
        .collect();
    if stem.is_empty() {
        "report".to_owned()
    } else {
        stem
    }
}
