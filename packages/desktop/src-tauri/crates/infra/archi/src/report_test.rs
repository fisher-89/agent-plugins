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
        version: 2,
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
        api_findings: Vec::new(),
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

// ===========================================================================
// `write_report` 经 `LocalArchiCheck::file_stem` 取文件名主干（`module-api-contract-format` /
// AC-13 / AC-16 / D23 / 事实校准 11）
//
// 实现体已从本文件迁进 `runner.rs` 的 `impl LocalArchiCheck` 并提升为 `pub`；
// `write_report` 的调用点改为 `LocalArchiCheck::file_stem(&report.timestamp)`——
// 两处共用同一实现，**本文件零 `fn file_stem` 定义**。行为断言（紧凑形态 / 退化 /
// 字符集）挂在 `runner_test` 侧（单一事实源，禁两份等价实现）。
// ===========================================================================

use crate::LocalArchiCheck;

/// 正向（AC-13 / D23）：三个时间戳各自落盘文件名逐字等于
/// `LocalArchiCheck::file_stem(timestamp)` 的产物（迁移前后行为零变化）。
#[test]
fn write_report文件名取自契约实现() {
    let temp = TempDir::new().expect("建临时目录失败");
    for (timestamp, expected) in [
        ("2026-10-20T08:31:12Z", "20261020T083112Z.json"),
        ("2027-01-02T03:04:05Z", "20270102T030405Z.json"),
        ("2026-12-31T23:59:59Z", "20261231T235959Z.json"),
    ] {
        let report = empty_report(timestamp);
        let written = write_report(temp.path(), "demo", &report).expect("写盘成功");
        assert_eq!(
            Path::new(&written)
                .file_name()
                .expect("文件名")
                .to_string_lossy(),
            expected,
            "文件名主干 = LocalArchiCheck::file_stem({timestamp})"
        );
        assert_eq!(
            Path::new(&written)
                .file_name()
                .expect("文件名")
                .to_string_lossy()
                .trim_end_matches(".json"),
            LocalArchiCheck::file_stem(timestamp),
            "落盘文件名与契约实现逐字同源"
        );
        assert!(Path::new(&written).is_file(), "文件真实在案：{written}");
    }
}

/// 边界（AC-16 / D23）：`report.rs` 内零 `fn file_stem` 定义（实现体已迁往 `runner.rs`，
/// 非复制）——源码自证，第二条等价实现的入口必须不存在。
#[test]
fn report内零file_stem定义() {
    let source = include_str!("report.rs");
    assert!(
        !source.contains("fn file_stem"),
        "report.rs MUST NOT 保留 file_stem 定义（实现体已迁移，禁副本）"
    );
    assert!(
        source.contains("LocalArchiCheck::file_stem(&report.timestamp)"),
        "调用点经契约实现单点：{source}"
    );
    assert!(
        source.contains("pub(crate) fn write_report("),
        "write_report 的 pub(crate) 可见性零变化"
    );

    let lib = include_str!("lib.rs");
    assert!(
        !lib.contains("pub use report::write_report"),
        "lib.rs MUST NOT 再导出 write_report（E0364 反例不在案）"
    );
    assert!(
        !lib.contains("pub use crate::report::write_report"),
        "lib.rs MUST NOT 再导出 write_report（第二种写形态同样禁止）"
    );
}

/// 正向（AC-8 交汇）：写完读回与入参报告逐字段相等，含非空 `api_findings` 与
/// `version == 2`（契约槽位随报告一并往返，零裁剪、零默认值补齐）。
#[test]
fn 内容往返含非空api_findings() {
    use archi::drift::report::ArchiDriftOccurrence;
    use archi::drift::report::{ApiFindingSeverity, ArchiApiFinding, ArchiApiFindingKind};

    let temp = TempDir::new().expect("建临时目录失败");

    // 形态一：空槽位
    let empty = empty_report("2026-10-20T08:31:12Z");
    let written = write_report(temp.path(), "demo", &empty).expect("写盘成功");
    let back: ArchiDriftReport =
        serde_json::from_slice(&fs::read(&written).expect("读回失败")).expect("反序列化失败");
    assert_eq!(back.api_findings.len(), 0, "空槽位往返仍为空");
    assert_eq!(back.version, 2, "版本 2 往返");

    // 形态二：非空槽位（三判据三种 category，occurrences 逐项）
    let mut filled = empty_report("2026-10-20T08:31:13Z");
    filled.api_findings = vec![
        ArchiApiFinding {
            kind: ArchiApiFindingKind::ApiMissing,
            category: "contract".to_owned(),
            element: Some("App.Backend.Infra.ArchiRuntime".to_owned()),
            target: None,
            message: "契约文件缺失".to_owned(),
            occurrences: Vec::new(),
        },
        ArchiApiFinding {
            kind: ArchiApiFindingKind::ApiSignatureMismatch,
            category: "signature".to_owned(),
            element: Some("App.Backend.Infra.ArchiRuntime".to_owned()),
            target: Some("file_stem".to_owned()),
            message: "契约签名与代码不一致".to_owned(),
            occurrences: vec![
                ArchiDriftOccurrence {
                    file: "Infra/App.Backend.Infra.ArchiRuntime.md".to_owned(),
                    line: 18,
                    specifier: Some("file_stem".to_owned()),
                    type_only: false,
                },
                ArchiDriftOccurrence {
                    file: "Infra/App.Backend.Infra.ArchiRuntime.md".to_owned(),
                    line: 19,
                    specifier: Some("file_stem".to_owned()),
                    type_only: true,
                },
            ],
        },
        ArchiApiFinding {
            kind: ArchiApiFindingKind::ApiUndeclared,
            category: "contract".to_owned(),
            element: Some("App.Backend.Infra.ArchiRuntime".to_owned()),
            target: Some("extra".to_owned()),
            message: "未在契约声明".to_owned(),
            occurrences: Vec::new(),
        },
    ];
    assert_eq!(
        ArchiApiFindingKind::ApiUndeclared.severity(),
        ApiFindingSeverity::Off,
        "本用例同时锁定 off 档判据的词汇归属（不因落盘而变）"
    );

    let written = write_report(temp.path(), "demo", &filled).expect("写盘成功");
    let back: ArchiDriftReport =
        serde_json::from_slice(&fs::read(&written).expect("读回失败")).expect("反序列化失败");
    assert_eq!(back, filled, "含非空 api_findings 的报告逐字段往返相等");
    assert_eq!(back.api_findings.len(), 3, "三条条目完整往返");
    assert_eq!(
        back.api_findings[1].occurrences.len(),
        2,
        "occurrences 逐项保留"
    );
    assert_eq!(
        back.api_findings[1].occurrences[1].line, 19,
        "occurrence 的行号保留"
    );
    assert_eq!(back.version, 2, "版本号随报告往返");

    // 落盘文本含契约槽位键（线面名 camelCase）
    let text = fs::read_to_string(&written).expect("读文本失败");
    assert!(
        text.contains("\"apiFindings\""),
        "落盘文本含契约判据槽位：{text}"
    );
}

/// 边界（交接清单版本值面）：夹具按 `REPORT_VERSION = 2` 构造可编译，且序列化往返
/// 成立（值面本身由 `drift/mod.rs` 的常量单点裁定，此处只断言写下即读得回）。
#[test]
fn 夹具版本值面与序列化往返() {
    let temp = TempDir::new().expect("建临时目录失败");
    let report = empty_report("2026-10-20T08:31:12Z");
    assert_eq!(report.version, 2, "夹具版本值与 REPORT_VERSION 同源");
    let value = serde_json::to_value(&report).expect("序列化成功");
    assert_eq!(value["version"], serde_json::json!(2), "线面版本为数字 2");
    let written = write_report(temp.path(), "demo", &report).expect("写盘成功");
    let text = fs::read_to_string(&written).expect("读文本失败");
    assert!(text.contains("\"version\": 2"), "落盘文本版本为 2：{text}");
    let back: ArchiDriftReport = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back.version, 2, "读回版本仍为 2");
}
