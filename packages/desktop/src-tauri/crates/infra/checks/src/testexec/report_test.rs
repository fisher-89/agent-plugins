use std::fs;

use checks::model::{CaseSummary, Conclusion, CoverageBlock, CoverageMeasured, CoverageThresholds};
use checks::model::{SubReport, SummaryReport};
use foundation::layout::change_test_reports;
use tempfile::TempDir;

use super::report::write_reports;

/// 子报告 fixture（vite-plus 绿态）。
fn sub_report(root: &str) -> SubReport {
    SubReport {
        framework: "vite-plus".to_owned(),
        root: root.to_owned(),
        timestamp: "2026-10-06T08:00:00.000Z".to_owned(),
        exit_code: 0,
        duration_ms: 1420.5,
        summary: CaseSummary {
            total: 12,
            passed: 12,
            failed: 0,
            skipped: 0,
        },
        error_cases: Vec::new(),
        test_files: vec!["src/app/math.test.ts".to_owned()],
        source_files: Vec::new(),
        coverage: Some(CoverageBlock {
            pass: true,
            measured: CoverageMeasured {
                lines: Some(86.67),
                branches: Some(72.73),
                functions: Some(85.71),
            },
            thresholds: CoverageThresholds {
                lines: 80.0,
                branches: 70.0,
                functions: 75.0,
            },
            overrides: None,
        }),
        mutation: None,
        findings: None,
    }
}

/// summary fixture（与子报告同形一致；root 仅用于 planId 对位——summary 无
/// root 字段，参数仅为与子报告 fixture 同签名可读性）。
fn summary(_root: &str) -> SummaryReport {
    SummaryReport {
        phase: "test-execution".to_owned(),
        command: "desktop change run: test-execution gate".to_owned(),
        timestamp: "2026-10-06T08:01:00.000Z".to_owned(),
        duration_seconds: 14.0,
        total: 12,
        passed: 12,
        failed: 0,
        skipped: 0,
        conclusion: Conclusion::Pass,
        problems: Vec::new(),
        coverage: Some(CoverageBlock {
            pass: true,
            measured: CoverageMeasured {
                lines: Some(86.67),
                branches: Some(72.73),
                functions: Some(85.71),
            },
            thresholds: CoverageThresholds {
                lines: 80.0,
                branches: 70.0,
                functions: 75.0,
            },
            overrides: None,
        }),
        mutation: None,
        plans: Vec::new(),
        findings: None,
    }
}

/// 正向：子报告批量 + summary 落盘——逐 planId 子目录 report.json 读回逐字
/// 段相等、summary.json 单次落盘、目录不存在自动创建。
#[test]
fn 子报告与summary落盘读回逐字段相等且目录自动创建() {
    let ws = TempDir::new().expect("临时目录");
    let report_dir = ws.path().join("deep").join("nested").join("reports");
    assert!(!report_dir.exists(), "前置：目录不存在");

    let sub = sub_report(".");
    let sum = summary(".");
    write_reports(&report_dir, std::slice::from_ref(&sub), &sum).expect("落盘应成功");

    // planId 子目录 report.json（derive_plan_id(".", "vite-plus") = "vite-plus"）
    let report_path = report_dir.join("vite-plus").join("report.json");
    let summary_path = report_dir.join("summary.json");
    assert!(
        report_path.is_file() && summary_path.is_file(),
        "双工件落盘且父目录自动创建"
    );

    let read_back = checks::parse_sub_report(&fs::read_to_string(&report_path).expect("读回"))
        .expect("report.json 应可解析");
    assert_eq!(read_back, sub, "子报告读回逐字段相等");

    let read_back = checks::parse_summary_report(&fs::read_to_string(&summary_path).expect("读回"))
        .expect("summary.json 应可解析");
    assert_eq!(read_back, sum, "summary 读回逐字段相等");
}

/// 正向：多 plan 批量——双子报告各落 planId 子目录、summary 一次落盘（子报
/// 告定稿后再落汇总，循环内不重写）。
#[test]
fn 多plan批量落盘_双子报告各就位summary单次落盘() {
    let ws = TempDir::new().expect("临时目录");
    let report_dir = ws.path().join("reports");
    let vite_plus = sub_report(".");
    let node_test = SubReport {
        framework: "node-test".to_owned(),
        ..sub_report("app")
    };
    write_reports(
        &report_dir,
        &[vite_plus.clone(), node_test.clone()],
        &summary("."),
    )
    .expect("落盘应成功");

    for (plan_id, expected) in [("vite-plus", &vite_plus), ("app_node-test", &node_test)] {
        let read_back = checks::parse_sub_report(
            &fs::read_to_string(report_dir.join(plan_id).join("report.json")).expect("读回"),
        )
        .expect("report.json 应可解析");
        assert_eq!(read_back, *expected, "{plan_id} 子报告读回逐字段相等");
    }
    assert!(
        report_dir.join("summary.json").is_file(),
        "summary 一次落盘（非逐子报告重写）"
    );
}

/// 正向：report_dir 由 change_test_reports 推导传入（tempdir change 树）——
/// 报告落 change 报告目录（layout 推导 → 写盘组合半边，AC-9）。
#[test]
fn report_dir由layout推导传入_报告落change报告目录() {
    let ws = TempDir::new().expect("临时目录");
    // 组合根同式：报告目录唯一经 foundation layout 推导（零磁盘路径字面量）
    let reports_dir = change_test_reports(ws.path(), "demo-change");

    write_reports(&reports_dir, &[sub_report(".")], &summary(".")).expect("落盘应成功");

    // <root>/openspec/changes/demo-change/reports/test/<planId>/report.json
    assert!(reports_dir.is_dir(), "layout 推导目录由写盘自动创建");
    assert!(
        reports_dir.join("vite-plus").join("report.json").is_file(),
        "planId 子目录落在 change 报告树下"
    );
    assert!(
        reports_dir.join("summary.json").is_file(),
        "summary 落在报告树根"
    );
}

/// 异常：写盘失败（report_dir 指向普通文件）→ Err 记因（基础设施失败停给
/// 用户——不产部分结论）。
#[test]
fn 写盘失败_report_dir指向普通文件_err记因() {
    let ws = TempDir::new().expect("临时目录");
    let file_root = ws.path().join("一个普通文件.txt");
    fs::write(&file_root, "占位").expect("写占位文件");

    let err = write_reports(&file_root, &[sub_report(".")], &summary("."))
        .expect_err("report_dir 为普通文件应 Err");
    assert!(
        err.contains("失败"),
        "Err 记因（报告子目录创建失败面），实际: {err}"
    );
}

/// 边界：重复写覆盖——同目录既有旧报告被完整替换无半写残留（重跑幂等）。
#[test]
fn 重复写完整替换无半写残留() {
    let ws = TempDir::new().expect("临时目录");
    let report_dir = ws.path().join("reports");
    let old_sub = sub_report(".");
    write_reports(&report_dir, std::slice::from_ref(&old_sub), &summary(".")).expect("首写应成功");

    // 二跑换失败形态 summary + 旧工件残留文件（半写检测面）
    let mut rerun_summary = summary(".");
    rerun_summary.conclusion = Conclusion::Fail;
    rerun_summary.failed = 2;
    rerun_summary.total = 12;
    rerun_summary.passed = 10;
    fs::write(report_dir.join("vite-plus").join("stale.txt"), "旧残留").expect("写残留");

    write_reports(&report_dir, std::slice::from_ref(&old_sub), &rerun_summary).expect("重写应成功");

    let read_back = checks::parse_summary_report(
        &fs::read_to_string(report_dir.join("summary.json")).expect("读回"),
    )
    .expect("重读应成功");
    assert_eq!(
        read_back, rerun_summary,
        "summary 完整替换（结论 / 计数刷新）"
    );
    // 注：报告目录整树清理（旧残留文件剪除）归 execute_plan 的清目录职责，
    // write_reports 的幂等面 = 双 JSON 工件完整替换无半写形态
    write_reports(&report_dir, std::slice::from_ref(&old_sub), &rerun_summary).expect("三写应成功");
    let third = checks::parse_summary_report(
        &fs::read_to_string(report_dir.join("summary.json")).expect("读回"),
    )
    .expect("重读应成功");
    assert_eq!(third, rerun_summary, "重复写幂等（无半写形态）");
}
