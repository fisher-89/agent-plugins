use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::model::{Conclusion, SummaryReport};

/// 复用门判定：三条件全过 → `true`（复用上次 summary，零 spawn 零重跑）。
///
/// ① 结论非 error（error 报告是部分执行，复用会以残缺结论顶替全量跑）；
/// ② 时间戳可解析（ISO 8601；缺失 / 非法格式不可判新 → 失效重跑）；
/// ③ 时间戳 ≥ 最新输入 mtime（`None` = 无输入文件可比较 → 视为新鲜）。
pub fn is_reusable(summary: &SummaryReport, newest_input_mtime_ms: Option<u64>) -> bool {
    if summary.conclusion == Conclusion::Error {
        return false;
    }
    let Some(summary_ms) = timestamp_millis(&summary.timestamp) else {
        return false;
    };
    match newest_input_mtime_ms {
        None => true,
        Some(newest_ms) => summary_ms >= newest_ms,
    }
}

/// ISO 8601 时间戳 → unix 毫秒（解析失败 → `None`）。
fn timestamp_millis(timestamp: &str) -> Option<u64> {
    let parsed = OffsetDateTime::parse(timestamp, &Rfc3339).ok()?;
    let millis = parsed.unix_timestamp_nanos() / 1_000_000;
    u64::try_from(millis).ok()
}
