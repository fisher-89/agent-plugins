use std::path::Path;

use checks::aggregate::derive_plan_id;
use checks::model::{SubReport, SummaryReport};

use super::detect::to_posix;

/// 子报告批量 + summary 原子落盘：逐 planId 子目录 `report.json` 后报告目录
/// 根 `summary.json` 写一次（子报告定稿后再落汇总——聚合同一终态；目录不存
/// 在自动创建；重复写完整替换无半写残留——重跑幂等）。写盘失败 → `Err`
///（基础设施失败停给用户，不产部分结论）。
pub(crate) fn write_reports(
    report_dir: &Path,
    sub_reports: &[SubReport],
    summary: &SummaryReport,
) -> Result<(), String> {
    for sub_report in sub_reports {
        let plan_id = derive_plan_id(&sub_report.root, &sub_report.framework);
        let plan_dir = report_dir.join(&plan_id);
        std::fs::create_dir_all(&plan_dir)
            .map_err(|error| format!("报告子目录创建失败 {}: {error}", to_posix(&plan_dir)))?;
        let sub_report_json = serde_json::to_vec_pretty(sub_report)
            .map_err(|error| format!("子报告序列化失败: {error}"))?;
        std::fs::write(plan_dir.join("report.json"), sub_report_json)
            .map_err(|error| format!("子报告写盘失败 {}: {error}", to_posix(&plan_dir)))?;
    }

    std::fs::create_dir_all(report_dir)
        .map_err(|error| format!("报告目录创建失败 {}: {error}", to_posix(report_dir)))?;
    let summary_json =
        serde_json::to_vec_pretty(summary).map_err(|error| format!("汇总序列化失败: {error}"))?;
    std::fs::write(report_dir.join("summary.json"), summary_json)
        .map_err(|error| format!("汇总写盘失败 {}: {error}", to_posix(report_dir)))?;
    Ok(())
}
