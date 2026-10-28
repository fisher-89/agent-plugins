//! `report`（报告落盘单点）的单元测试：目录经 `change_reports(.., ReportSlot::
//! Archi)` 派生、文件名取时间戳紧凑形态（无冒号）、多级目录创建、内容往返、
//! 同时间戳覆盖 / 不同时间戳堆积、槽位隔离与不可写降级。
//!
//! 零 mock：真实 `tempfile::TempDir`（本 crate dev-dep 已声明）+ 真实
//! `foundation::layout` 派生 + 真实 `serde_json`；写盘失败以「同名普通文件占位」
//! 注入，不改权限、不依赖平台。

use std::fs;
use std::path::Path;

use archi::drift::report::{ArchiDriftModel, ArchiGateCount, ArchiScanReport};
use archi::drift::{
    ArchiCheckStatus, ArchiDriftEntry, ArchiDriftKind, ArchiDriftMode, ArchiDriftReport,
};
use foundation::layout::{change_reports, ReportSlot};
use tempfile::TempDir;

use crate::report::write_report;

// ---------------------------------------------------------------------------
// 报告夹具
// ---------------------------------------------------------------------------

/// 空报告（`clean`）。
fn empty_report(timestamp: &str) -> ArchiDriftReport {
    ArchiDriftReport {
        version: 1,
        timestamp: timestamp.to_owned(),
        mode: ArchiDriftMode::Incremental,
        root: "/tmp/workspace".to_owned(),
        change: Some("demo".to_owned()),
        base: Some("HEAD".to_owned()),
        model_hash: "sha256:00ff".to_owned(),
        model: ArchiDriftModel {
            elements: 0,
            relations: 0,
            by_gate: ArchiGateCount::default(),
            structural_errors: Vec::new(),
            warnings: Vec::new(),
        },
        scan: ArchiScanReport {
            mode: ArchiDriftMode::Incremental,
            changed_files: 0,
            scanned_files: 0,
            excluded: Vec::new(),
            unmatched_files: Vec::new(),
            matched: Vec::new(),
            unresolved: Vec::new(),
        },
        errors: Vec::new(),
        warnings: Vec::new(),
        suppressed: Vec::new(),
        observations: Vec::new(),
        status: ArchiCheckStatus::Clean,
    }
}

/// 带 `count` 条违例（各 `occurrences` 条命中点）的报告。
fn report_with_errors(timestamp: &str, count: usize, occurrences: usize) -> ArchiDriftReport {
    let mut report = empty_report(timestamp);
    report.status = ArchiCheckStatus::Errors;
    report.errors = (0..count)
        .map(|index| ArchiDriftEntry {
            kind: ArchiDriftKind::UndeclaredDependency,
            from: Some(format!("App.F{index}")),
            to: Some(format!("App.T{index}")),
            gate: None,
            message: format!("未声明依赖 {index}"),
            occurrences: (0..occurrences)
                .map(|_| archi::drift::report::ArchiDriftOccurrence {
                    file: "src/a/x.ts".to_owned(),
                    line: 1,
                    specifier: Some("./x".to_owned()),
                    type_only: false,
                })
                .collect(),
        })
        .collect();
    report
}

/// 报告目录（被测派生单点）。
fn report_dir(root: &Path, name: &str) -> std::path::PathBuf {
    change_reports(root, name, ReportSlot::Archi)
}

/// 目录内文件名清单（排序）。
fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("读取报告目录失败")
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// 正向：目录派生与文件名形态
// ---------------------------------------------------------------------------

/// 正向（AC-14）：落点逐字等于 `change_reports(root, "demo", ReportSlot::Archi)`。
#[test]
fn 目录派生单点() {
    let temp = TempDir::new().expect("建临时目录失败");
    let report = empty_report("2026-10-20T08:31:12Z");
    let written = write_report(temp.path(), "demo", &report).expect("写盘应成功");
    let expected = report_dir(temp.path(), "demo").join("20261020T083112Z.json");
    assert_eq!(
        Path::new(&written),
        expected.as_path(),
        "落点逐字等于槽位派生目录 + 时间戳文件名"
    );
    assert!(expected.exists(), "文件应真实存在");
}

/// 正向（D9）：三个时间戳各自的文件名形态（过滤出数字 + `T` + `Z`，无冒号）。
#[test]
fn 文件名形态() {
    let temp = TempDir::new().expect("建临时目录失败");
    let cases = [
        ("2026-10-20T08:31:12Z", "20261020T083112Z.json"),
        ("2027-01-02T03:04:05Z", "20270102T030405Z.json"),
        ("2026-12-31T23:59:59Z", "20261231T235959Z.json"),
    ];
    for (timestamp, expected) in cases {
        let written =
            write_report(temp.path(), "demo", &empty_report(timestamp)).expect("写盘成功");
        assert_eq!(
            Path::new(&written)
                .file_name()
                .expect("文件名")
                .to_string_lossy(),
            expected,
            "时间戳 {timestamp} 的文件名形态"
        );
    }
}

/// 边界：文件名跨平台合法（无 `:` / `/` / `\` / `*` / `?` / `"` / `<` / `>` /
/// `|`，长度 < 255，前 8 位为日期、末位 `Z`）。
#[test]
fn 文件名跨平台合法性() {
    let temp = TempDir::new().expect("建临时目录失败");
    for timestamp in [
        "2026-10-20T08:31:12Z",
        "2027-01-02T03:04:05Z",
        "2026-12-31T23:59:59Z",
    ] {
        let written =
            write_report(temp.path(), "demo", &empty_report(timestamp)).expect("写盘成功");
        let name = Path::new(&written)
            .file_name()
            .expect("文件名")
            .to_string_lossy()
            .into_owned();
        for forbidden in [":", "/", "\\", "*", "?", "\"", "<", ">", "|"] {
            assert!(
                !name.contains(forbidden),
                "文件名 {name} MUST NOT 含 {forbidden}"
            );
        }
        assert!(name.len() < 255, "文件名长度受限：{name}");
        let stem = name.trim_end_matches(".json");
        assert_eq!(
            stem.chars().take(8).collect::<String>(),
            timestamp
                .chars()
                .take(10)
                .collect::<String>()
                .replace('-', ""),
            "前 8 位为日期：{stem}"
        );
        assert!(stem.ends_with('Z'), "末位为 Z：{stem}");
    }
}

/// 边界：多级目录创建——`<changes_root>/demo` 与 `reports/archi` 两级均缺失时写盘
/// 成功且目录建成。
#[test]
fn 多级目录创建() {
    let temp = TempDir::new().expect("建临时目录失败");
    assert!(
        !temp.path().join("openspec").exists(),
        "前置：连域根目录都不存在"
    );
    let written = write_report(temp.path(), "demo", &empty_report("2026-10-20T08:31:12Z"))
        .expect("多级目录应一次建成");
    assert!(Path::new(&written).is_file(), "文件写出");
    assert!(
        temp.path()
            .join("openspec/changes/demo/reports/archi")
            .is_dir(),
        "整条目录链建成"
    );
}

// ---------------------------------------------------------------------------
// 正向：内容往返 / 返回值 / 覆盖与堆积
// ---------------------------------------------------------------------------

/// 正向：写入后读回反序列化与入参报告逐字段相等（`to_vec_pretty` 一次整写）。
#[test]
fn 内容往返() {
    let temp = TempDir::new().expect("建临时目录失败");
    let report = report_with_errors("2026-10-20T08:31:12Z", 3, 2);
    let written = write_report(temp.path(), "demo", &report).expect("写盘成功");
    let bytes = fs::read(&written).expect("读回失败");
    let back: ArchiDriftReport = serde_json::from_slice(&bytes).expect("反序列化失败");
    assert_eq!(back, report, "落盘内容与入参报告逐字段相等");
    let text = String::from_utf8(bytes).expect("UTF-8");
    assert!(text.contains("\n  "), "落盘为 pretty 形态（可读差异）");
}

/// 正向：返回路径 `exists()` 为真且等于派生目录 + 文件名。
#[test]
fn 返回值可用() {
    let temp = TempDir::new().expect("建临时目录失败");
    let report = empty_report("2026-10-20T08:31:12Z");
    let written = write_report(temp.path(), "demo", &report).expect("写盘成功");
    assert!(Path::new(&written).exists(), "返回路径存在");
    assert_eq!(
        Path::new(&written),
        report_dir(temp.path(), "demo")
            .join("20261020T083112Z.json")
            .as_path(),
        "返回路径 = 派生目录 + 文件名"
    );
}

/// 边界：同时间戳写两次 → 单文件且内容为后写者（不堆积同秒副本）。
#[test]
fn 同时间戳覆盖() {
    let temp = TempDir::new().expect("建临时目录失败");
    write_report(
        temp.path(),
        "demo",
        &report_with_errors("2026-10-20T08:31:12Z", 1, 1),
    )
    .expect("第一次写盘成功");
    write_report(
        temp.path(),
        "demo",
        &report_with_errors("2026-10-20T08:31:12Z", 5, 1),
    )
    .expect("第二次写盘成功");
    let names = file_names(&report_dir(temp.path(), "demo"));
    assert_eq!(names, vec!["20261020T083112Z.json".to_owned()], "单文件");
    let back: ArchiDriftReport = serde_json::from_slice(
        &fs::read(report_dir(temp.path(), "demo").join("20261020T083112Z.json")).expect("读回失败"),
    )
    .expect("反序列化失败");
    assert_eq!(back.errors.len(), 5, "内容为后写者");
}

/// 边界：两个不同时间戳 → 两文件并存（不删旧报告）。
#[test]
fn 不同时间戳堆积() {
    let temp = TempDir::new().expect("建临时目录失败");
    write_report(temp.path(), "demo", &empty_report("2026-10-20T08:31:12Z"))
        .expect("第一次写盘成功");
    write_report(temp.path(), "demo", &empty_report("2026-10-20T09:00:00Z"))
        .expect("第二次写盘成功");
    assert_eq!(
        file_names(&report_dir(temp.path(), "demo")),
        vec![
            "20261020T083112Z.json".to_owned(),
            "20261020T090000Z.json".to_owned()
        ],
        "两文件并存（旧报告不删）"
    );
}

// ---------------------------------------------------------------------------
// 边界：槽位隔离与降级
// ---------------------------------------------------------------------------

/// 边界（AC-14）：写盘后 `reports/test` / `reports/baseline` 零创建（三槽位互不
/// 混居）。
#[test]
fn 槽位隔离() {
    let temp = TempDir::new().expect("建临时目录失败");
    write_report(temp.path(), "demo", &empty_report("2026-10-20T08:31:12Z")).expect("写盘成功");
    assert!(
        !change_reports(temp.path(), "demo", ReportSlot::Test).exists(),
        "reports/test 零创建"
    );
    assert!(
        !change_reports(temp.path(), "demo", ReportSlot::Baseline).exists(),
        "reports/baseline 零创建"
    );
    assert!(
        change_reports(temp.path(), "demo", ReportSlot::Archi).exists(),
        "只有 reports/archi 被创建"
    );
}

/// 异常（AC-14 前置）：`reports/archi` 位置被普通文件占位 → `Err`，且零半写残留
/// （失败前置到 `create_dir_all`）。
#[test]
fn 目录不可写降级面() {
    let temp = TempDir::new().expect("建临时目录失败");
    let blocking = report_dir(temp.path(), "demo");
    fs::create_dir_all(blocking.parent().expect("父目录")).expect("建父目录失败");
    fs::write(&blocking, "占位普通文件\n").expect("写占位文件失败");

    let error = write_report(temp.path(), "demo", &empty_report("2026-10-20T08:31:12Z"))
        .expect_err("目录位置被文件占位必须 Err");
    assert!(error.contains("架构报告"), "错误文案指明报告面：{error}");
    assert!(blocking.is_file(), "占位文件未被替换");
    assert_eq!(
        fs::read_to_string(&blocking).expect("读占位失败"),
        "占位普通文件\n",
        "零半写（占位文件字节零变化）"
    );
}

/// 异常：`name` 边界（空串 / 含 `..` / 含空格与非 ASCII）→ 不 panic，返回型可判，
/// 且不写出报错路径之外的文件。
#[test]
fn name边界不panic() {
    let temp = TempDir::new().expect("建临时目录失败");
    for name in ["", "..", "含 空格 与中文"] {
        let outcome = write_report(temp.path(), name, &empty_report("2026-10-20T08:31:12Z"));
        match outcome {
            Ok(path) => {
                assert!(
                    Path::new(&path).starts_with(temp.path()),
                    "写出的文件仍在 tempdir 内：{path}"
                );
            }
            Err(error) => assert!(!error.is_empty(), "错误文案非空"),
        }
    }
}

// ---------------------------------------------------------------------------
// 边界：规模与空报告
// ---------------------------------------------------------------------------

/// 边界：1000 条 `errors`（各 10 个命中点）→ 写盘成功、读回长度不变（无截断）。
#[test]
fn 大报告() {
    let temp = TempDir::new().expect("建临时目录失败");
    let report = report_with_errors("2026-10-20T08:31:12Z", 1000, 10);
    let written = write_report(temp.path(), "demo", &report).expect("写盘成功");
    let back: ArchiDriftReport =
        serde_json::from_slice(&fs::read(&written).expect("读回失败")).expect("反序列化失败");
    assert_eq!(back.errors.len(), 1000, "千条违例不截断");
    assert_eq!(back.errors[999].occurrences.len(), 10, "命中点完整");
}

/// 边界：全空数组 + `clean` 报告 → 写盘成功且读回逐字段相等。
#[test]
fn 空报告() {
    let temp = TempDir::new().expect("建临时目录失败");
    let report = empty_report("2026-10-20T08:31:12Z");
    let written = write_report(temp.path(), "demo", &report).expect("写盘成功");
    let back: ArchiDriftReport =
        serde_json::from_slice(&fs::read(&written).expect("读回失败")).expect("反序列化失败");
    assert_eq!(back, report, "空报告逐字段往返相等");
    assert_eq!(back.status, ArchiCheckStatus::Clean, "status 原样保留");
}
