use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::model::{Conclusion, SummaryReport};
use crate::reuse::is_reusable;

/// 内存构造 summary（复用门判定只消费 conclusion 与 timestamp 两字段）。
fn summary(conclusion: Conclusion, timestamp: &str) -> SummaryReport {
    SummaryReport {
        phase: "test-execution".to_owned(),
        command: "cmd".to_owned(),
        timestamp: timestamp.to_owned(),
        duration_seconds: 1.0,
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
        conclusion,
        problems: Vec::new(),
        coverage: None,
        mutation: None,
        plans: Vec::new(),
        findings: None,
    }
}

/// RFC 3339 串 → unix 毫秒（与实现同口径换算，供 mtime 入参构造）。
fn millis(timestamp: &str) -> u64 {
    let parsed = OffsetDateTime::parse(timestamp, &Rfc3339).expect("测试时间戳应可解析");
    u64::try_from(parsed.unix_timestamp_nanos() / 1_000_000).expect("毫秒应非负")
}

const FRESH: &str = "2026-10-06T08:00:00Z";
const OLDER_INPUT: &str = "2026-10-06T07:59:00Z";
const NEWER_INPUT: &str = "2026-10-06T08:00:01Z";

/// 正向：结论 pass + 时间戳可解析且 ≥ 最新输入 mtime → true（复用零 spawn 判据）。
#[test]
fn pass结论且时间戳新鲜复用() {
    // summary 时间戳晚于最新输入 1s
    let report = summary(Conclusion::Pass, FRESH);
    assert!(
        is_reusable(&report, Some(millis(OLDER_INPUT))),
        "时间戳晚于最新输入 → 复用（零 spawn 判据）"
    );
}

/// 正向：结论 fail + 时间戳新鲜 → true（fail 结论同属可复用——「结论非
/// error」语义对偶面）。
#[test]
fn fail结论且时间戳新鲜仍复用() {
    let report = summary(Conclusion::Fail, FRESH);
    assert!(
        is_reusable(&report, Some(millis(OLDER_INPUT))),
        "fail 报告是完整执行产出（非 error 残缺面）→ 同属可复用"
    );
}

/// 异常：结论 error → false（失效重跑——error 报告是部分执行，复用会以残缺
/// 结论顶替全量跑）。
#[test]
fn error结论不复用() {
    let report = summary(Conclusion::Error, FRESH);
    assert!(
        !is_reusable(&report, Some(millis(OLDER_INPUT))),
        "error 报告失效重跑"
    );
    // 时间戳再新鲜、甚至无输入可比，error 一票否决在先
    assert!(!is_reusable(&report, None), "结论门在时间戳门之前");
}

/// 异常：时间戳不可解析（缺失 / 非法格式）→ false（不可判新 → 失效重跑）。
#[test]
fn 时间戳不可解析不复用() {
    // 非法格式
    let report = summary(Conclusion::Pass, "not-a-timestamp");
    assert!(!is_reusable(&report, Some(millis(OLDER_INPUT))));
    assert!(
        !is_reusable(&report, None),
        "时间戳门与输入集无关，一律失效"
    );

    // 空串（CLI 产出缺失时刻的空缺省形态）
    let report = summary(Conclusion::Pass, "");
    assert!(!is_reusable(&report, Some(millis(OLDER_INPUT))));

    // 非 RFC 3339 的日期形态（缺时区）
    let report = summary(Conclusion::Pass, "2026-10-06 08:00:00");
    assert!(!is_reusable(&report, Some(millis(OLDER_INPUT))));
}

/// 边界：时间戳恰等于最新输入 mtime → true（≥ 边界含等号——同毫秒产出不算
/// 输入已变）。
#[test]
fn 时间戳恰等于输入mtime含等号复用() {
    let report = summary(Conclusion::Pass, FRESH);
    assert!(
        is_reusable(&report, Some(millis(FRESH))),
        "≥ 边界含等号：恰等 → 复用"
    );
}

/// 边界：输入 mtime 晚于时间戳 → false（输入已变失效——反馈边修复重入判据）。
#[test]
fn 输入mtime晚于时间戳失效重跑() {
    let report = summary(Conclusion::Pass, FRESH);
    assert!(
        !is_reusable(&report, Some(millis(NEWER_INPUT))),
        "输入晚于报告 1s → 失效重跑"
    );
    // 毫秒粒度（时钟偏移只会过度失效，不会误复用）
    let newer_by_1ms = millis(FRESH) + 1;
    assert!(!is_reusable(&report, Some(newer_by_1ms)), "晚 1ms 即失效");
}

/// 边界：newest_input_mtime 为 None（空输入集）→ true（CLI
/// tryReuseFreshSummary 逐字段对齐：无输入文件可比较 → 视为新鲜）。
#[test]
fn 空输入集视为新鲜() {
    let pass = summary(Conclusion::Pass, FRESH);
    let fail = summary(Conclusion::Fail, FRESH);
    assert!(is_reusable(&pass, None), "空输入集 → 视为新鲜复用");
    assert!(is_reusable(&fail, None), "fail 结论同面");

    // 对照：None 新鲜性与时间戳相对值无关（时间戳仅要求可解析）
    let older = summary(Conclusion::Pass, OLDER_INPUT);
    assert!(is_reusable(&older, None));
}
