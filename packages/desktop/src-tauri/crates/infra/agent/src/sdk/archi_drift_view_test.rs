//! `archi_drift_view`（引擎侧漂移投影单点）的单元测试：三 scope 逐级追加
//! （`summary` ⊂ `entries` ⊂ `files`）、键集齐备、`durationMs` 透传、常态量大报告
//! 的「路径串只在 files 侧」纪律，以及 `DRIFT_SCOPES` 词表单点。
//!
//! 零 mock：`Report` 由真实 `archi::drift::check(Full, …)` 在真实 tempdir 盘面上产出
//! ——直构 30+ 字段的替身会掩盖真实计数形态（本模块只裁剪，报告形态即被测事实）。

use std::path::Path;

use archi::drift::{check, ArchiDriftMode, CheckInput, Report};
use foundation::cancel::CancelFlag;
use serde_json::Value;

use crate::sdk::archi_drift_view::{drift_entries, drift_files, drift_summary, DRIFT_SCOPES};

// ---------------------------------------------------------------------------
// 装置：真实 tempdir 盘面 + 真实全量判定
// ---------------------------------------------------------------------------

/// 落一个 repo 相对文件（自动建父目录）。
fn write_file(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建父目录失败");
    }
    std::fs::write(path, content).expect("写文件失败");
}

/// 落模型声明（`<archi_graph_model_dir>/01-test.toml`）。
fn write_model(root: &Path, content: &str) {
    let dir = foundation::layout::archi_graph_model_dir(root);
    std::fs::create_dir_all(&dir).expect("创建模型声明目录失败");
    std::fs::write(dir.join("01-test.toml"), content).expect("写模型文件失败");
}

/// 全量判定（唯一入口，两消费面同源）。
fn report_of(root: &Path) -> Report {
    check(CheckInput {
        root,
        mode: ArchiDriftMode::Full,
        changed: &[],
        change: None,
        cancel: CancelFlag::never(),
    })
    .expect("健康盘面应判定成功")
}

/// 对象键集（`json!` 的插入序）。
fn keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .expect("投影恒为 JSON 对象")
        .keys()
        .cloned()
        .collect()
}

/// 健康盘面：一元素认领 `src/auth/`，另有一处无主源文件（常态 unmatched）。
fn healthy_root(tag: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("sdk-archi-drift-view-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败");
    write_model(
        dir.path(),
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"module\"\n",
            "path = [\"./src/auth/\"]\n",
            "summary = \"认证模块\"\n",
        ),
    );
    write_file(dir.path(), "src/auth/lib.rs", "pub fn login() {}\n");
    write_file(dir.path(), "src/loose/orphan.ts", "export const x = 1;\n");
    dir
}

// ---------------------------------------------------------------------------
// `drift_summary`：缺省 scope 的投影
// ---------------------------------------------------------------------------

/// 正向：健康盘面 → 键集逐项在场，`counts` 五个长度位 + `apiFindings` 两档 +
/// `scan` 六项计数齐全（键不缺席是投影契约，前端按固定键读）。
#[test]
fn drift_summary键集齐备() {
    let root = healthy_root("keys");
    let report = report_of(root.path());
    let summary = drift_summary(&report, 7);

    assert_eq!(
        keys(&summary),
        vec![
            "mode",
            "status",
            "passed",
            "modelHash",
            "model",
            "counts",
            "errors",
            "warnings",
            "modelWarnings",
            "apiFindings",
            "durationMs",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<String>>(),
        "summary 键集逐项在场"
    );
    assert_eq!(summary["mode"], "full", "恒全量模式");
    assert_eq!(summary["status"], report.status.as_str());
    assert_eq!(summary["passed"], report.passed());
    assert_eq!(summary["modelHash"], report.model_hash);

    for key in [
        "errors",
        "warnings",
        "suppressed",
        "observations",
        "modelWarnings",
    ] {
        assert!(summary["counts"][key].is_u64(), "counts.{key} 为计数字段");
    }
    assert_eq!(
        summary["counts"]["errors"],
        report.errors.len(),
        "errors 计数与报告同源"
    );
    assert_eq!(
        summary["counts"]["apiFindings"]["total"],
        report.api_findings.len(),
        "契约判据总数与报告同源"
    );
    assert!(
        summary["counts"]["apiFindings"]["bySeverity"].is_object(),
        "bySeverity 为对象（四档计数位恒在场）"
    );
    for key in [
        "changedFiles",
        "scannedFiles",
        "excluded",
        "unmatched",
        "matched",
        "unresolved",
    ] {
        assert!(
            summary["counts"]["scan"][key].is_u64(),
            "counts.scan.{key} 为计数字段"
        );
    }
    assert_eq!(
        summary["counts"]["scan"]["unmatched"],
        report.scan.unmatched_files.len(),
        "unmatched 只以计数位出现"
    );
    assert_eq!(
        summary["counts"]["scan"]["scannedFiles"], report.scan.scanned_files,
        "扫描文件数与报告同源"
    );
    assert!(
        summary["model"]["elements"].is_u64()
            && summary["model"]["relations"].is_u64()
            && summary["model"]["byGate"]["strict"].is_u64()
            && summary["model"]["byGate"]["warn"].is_u64()
            && summary["model"]["byGate"]["off"].is_u64()
            && summary["model"]["structuralErrorCount"].is_u64(),
        "model 面计数与 gate 三档齐备"
    );
}

/// 正向（参数化）：`duration_ms` 逐值透传，零改写零截断。
#[test]
fn drift_summary的duration_ms逐值透传() {
    let root = healthy_root("duration");
    let report = report_of(root.path());
    for duration in [0_u64, 1, 74, u64::MAX] {
        let summary = drift_summary(&report, duration);
        assert_eq!(
            summary["durationMs"],
            Value::from(duration),
            "durationMs 逐字等于入参"
        );
    }
}

/// 边界：空报告（无模型 → `skipped`）→ 各数组空、各计数 0，且键一个不少。
#[test]
fn drift_summary空报告键不缺席() {
    let root = tempfile::Builder::new()
        .prefix("sdk-archi-drift-view-empty-")
        .tempdir()
        .expect("创建合成目录失败");
    let report = report_of(root.path());
    assert_eq!(report.status.as_str(), "skipped", "无模型即 skipped");
    let summary = drift_summary(&report, 0);

    for key in ["errors", "warnings", "modelWarnings"] {
        assert_eq!(summary[key], Value::Array(Vec::new()), "{key} 为空数组");
    }
    assert_eq!(
        summary["counts"]["scan"]["unmatched"],
        Value::from(0),
        "空报告 unmatched 计数为 0"
    );
    assert_eq!(summary["counts"]["errors"], Value::from(0));
    assert_eq!(summary["counts"]["warnings"], Value::from(0));
    assert_eq!(summary["counts"]["observations"], Value::from(0));
    assert_eq!(
        summary["counts"]["apiFindings"]["total"],
        Value::from(0),
        "空报告零契约判据"
    );
    assert_eq!(summary["model"]["structuralErrorCount"], Value::from(0));
    assert_eq!(summary["status"], "skipped");
    assert!(
        summary.get("observations").is_none() && summary.get("files").is_none(),
        "缺省 scope 不含 entries / files 追加面"
    );
}

/// 边界（常态盘面）：`gate` 全 `off` 时 `unmatchedFiles` 是数百条量级的常态——计数位
/// 正确且投影文本零 unmatched 路径串（明细只在 `files` scope）。
#[test]
fn drift_summary常态unmatched只在计数位() {
    let root = tempfile::Builder::new()
        .prefix("sdk-archi-drift-view-unmatched-")
        .tempdir()
        .expect("创建合成目录失败");
    write_model(
        root.path(),
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Auth\"]\n",
            "kind = \"module\"\n",
            "path = [\"./src/auth/\"]\n",
        ),
    );
    write_file(root.path(), "src/auth/lib.rs", "pub fn login() {}\n");
    let mut orphans: Vec<String> = Vec::new();
    for index in 0..40 {
        let relative = format!("src/loose/orphan{index}.ts");
        write_file(root.path(), &relative, "export const x = 1;\n");
        orphans.push(relative);
    }
    let report = report_of(root.path());
    // `gate = off` 的元素不参与归属 ⇒ 它覆盖的 `src/auth/lib.rs` 同样计入 unmatched
    //（这正是「gate 全 off 时 unmatchedFiles 是常态」的形态）
    assert_eq!(
        report.scan.unmatched_files.len(),
        orphans.len() + 1,
        "无主源文件逐条进 unmatched 清单（含 off 档元素覆盖文件）"
    );
    assert_eq!(
        report.scan.matched.len(),
        0,
        "off 档元素零归属（matched 空）"
    );

    let summary = drift_summary(&report, 3);
    assert_eq!(
        summary["counts"]["scan"]["unmatched"],
        Value::from(orphans.len() + 1),
        "计数位等于清单长度（含 off 档元素覆盖文件）"
    );
    let text = serde_json::to_string(&summary).expect("序列化成功");
    for orphan in &orphans {
        assert!(
            !text.contains(orphan.as_str()),
            "summary 文本 MUST NOT 出现 unmatched 路径串 {orphan}"
        );
    }
}

/// 异常：`errors` 非空 → `status = "errors"` 且 `passed = false`（与
/// `report.passed()` 同值——投影不另算一份门禁布尔）。
#[test]
fn drift_summary错误面与passed同源() {
    let root = tempfile::Builder::new()
        .prefix("sdk-archi-drift-view-errors-")
        .tempdir()
        .expect("创建合成目录失败");
    write_model(
        root.path(),
        concat!(
            "[element.\"App\"]\n",
            "kind = \"package\"\n",
            "\n",
            "[element.\"App.Lost\"]\n",
            "kind = \"module\"\n",
            "gate = \"strict\"\n",
            "path = [\"./src/not-there/\"]\n",
        ),
    );
    write_file(root.path(), "src/other/thing.ts", "export const x = 1;\n");
    let report = report_of(root.path());
    assert!(!report.errors.is_empty(), "strict 档 path 失效进 errors");

    let summary = drift_summary(&report, 1);
    assert_eq!(summary["status"], "errors");
    assert_eq!(summary["passed"], Value::Bool(false));
    assert_eq!(
        summary["passed"],
        Value::Bool(report.passed()),
        "passed 与 report.passed() 同源"
    );
    assert!(
        summary["errors"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "errors 明细在场"
    );
}

/// 异常：坏模型（`model.structuralErrors` 非空）→ 结构错明细与计数在场、`errors`
/// 数组仍空（结构错是并列槽位，不混入 `errors[]`），`status = "errors"`。
#[test]
fn drift_summary坏模型结构错并列槽位() {
    let root = tempfile::Builder::new()
        .prefix("sdk-archi-drift-view-structural-")
        .tempdir()
        .expect("创建合成目录失败");
    write_model(root.path(), "[element.\"App]\nkind = \"package\"\n");
    let report = report_of(root.path());
    assert!(
        !report.model.structural_errors.is_empty(),
        "坏模型产出结构诊断"
    );

    let summary = drift_summary(&report, 2);
    assert_eq!(
        summary["model"]["structuralErrors"]
            .as_array()
            .map(Vec::len),
        Some(report.model.structural_errors.len()),
        "结构错明细逐条在场"
    );
    assert_eq!(
        summary["model"]["structuralErrorCount"],
        Value::from(report.model.structural_errors.len())
    );
    assert_eq!(
        summary["errors"],
        Value::Array(Vec::new()),
        "结构错不进 errors[]"
    );
    assert_eq!(summary["status"], "errors");
    assert_eq!(summary["passed"], Value::Bool(false));
}

// ---------------------------------------------------------------------------
// `drift_entries` / `drift_files`：逐级追加
// ---------------------------------------------------------------------------

/// 正向：`entries` 含 `summary` 全部键 ∧ 追加 `observations` 与契约判据条目明细
/// （`byKind` 计数仍只在 `summary` 侧）。
#[test]
fn drift_entries在summary之上追加观察项与判据条目() {
    let root = healthy_root("entries");
    let report = report_of(root.path());
    let summary = drift_summary(&report, 5);
    let entries = drift_entries(&report, 5);

    for key in keys(&summary) {
        assert!(entries.get(&key).is_some(), "entries 应含 summary 键 {key}");
    }
    assert_eq!(
        entries["observations"].as_array().map(Vec::len),
        Some(report.observations.len()),
        "观察项全量追加"
    );
    assert_eq!(
        entries["apiFindings"]["items"].as_array().map(Vec::len),
        Some(report.api_findings.len()),
        "契约判据条目全量追加"
    );
    assert!(
        entries["apiFindings"]["byKind"].is_object(),
        "byKind 计数位仍在（增补不重算）"
    );
    assert!(entries.get("files").is_none(), "entries 不含 files 清单");
}

/// 组合：三份盘面下 `entries` 与 `summary` 的同名计数字段逐字段相等（追加不重算）。
#[test]
fn drift_entries与summary计数逐字段相等() {
    let healthy = healthy_root("same-healthy");
    let empty = tempfile::Builder::new()
        .prefix("sdk-archi-drift-view-same-empty-")
        .tempdir()
        .expect("创建合成目录失败");
    let broken = tempfile::Builder::new()
        .prefix("sdk-archi-drift-view-same-broken-")
        .tempdir()
        .expect("创建合成目录失败");
    write_model(broken.path(), "[element.\"App]\nkind = \"package\"\n");

    for root in [healthy.path(), empty.path(), broken.path()] {
        let report = report_of(root);
        let summary = drift_summary(&report, 11);
        let entries = drift_entries(&report, 11);
        for key in ["counts", "model", "errors", "warnings", "modelWarnings"] {
            assert_eq!(entries[key], summary[key], "{key} 逐字段相等（不重算）");
        }
        assert_eq!(entries["durationMs"], summary["durationMs"]);
        assert_eq!(entries["status"], summary["status"]);
    }
}

/// 正向：`files` 含 `entries` 全部键 ∧ 追加 `files` 的 `unmatched` / `matched` /
/// `excluded` / `unresolved` 四类清单（逐条全量）。
#[test]
fn drift_files在entries之上追加四类清单() {
    let root = healthy_root("files");
    let report = report_of(root.path());
    let entries = drift_entries(&report, 9);
    let files = drift_files(&report, 9);

    for key in keys(&entries) {
        assert!(files.get(&key).is_some(), "files 应含 entries 键 {key}");
    }
    for key in ["unmatched", "matched", "excluded", "unresolved"] {
        assert!(
            files["files"][key].is_array(),
            "files.{key} 为数组（空也是数组）"
        );
    }
    assert_eq!(
        files["files"]["unmatched"].as_array().map(Vec::len),
        Some(report.scan.unmatched_files.len()),
        "unmatched 清单全量"
    );
    assert_eq!(
        files["files"]["matched"].as_array().map(Vec::len),
        Some(report.scan.matched.len()),
        "matched 清单全量"
    );
    assert!(
        files["files"]["unmatched"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == "src/loose/orphan.ts")),
        "files scope 才出路径串"
    );
}

/// 边界（表驱动）：`summary ⊂ entries ⊂ files`——键集严格包含、逐级只增不减。
#[test]
fn 三scope键集严格包含() {
    let root = healthy_root("containment");
    let report = report_of(root.path());
    let summary = drift_summary(&report, 0);
    let entries = drift_entries(&report, 0);
    let files = drift_files(&report, 0);

    for (inner, outer, context) in [
        (&summary, &entries, "summary ⊂ entries"),
        (&entries, &files, "entries ⊂ files"),
        (&summary, &files, "summary ⊂ files"),
    ] {
        let inner_keys = keys(inner);
        let outer_keys = keys(outer);
        for key in &inner_keys {
            assert!(outer_keys.contains(key), "{context}：缺键 {key}");
        }
        if inner_keys.len() == outer_keys.len() {
            assert_ne!(
                std::ptr::from_ref(inner),
                std::ptr::from_ref(outer),
                "{context}：非同一份投影"
            );
        }
        assert!(
            outer_keys.len() > inner_keys.len(),
            "{context}：逐级只增不减（内 {} 键 / 外 {} 键）",
            inner_keys.len(),
            outer_keys.len()
        );
    }
}

// ---------------------------------------------------------------------------
// `DRIFT_SCOPES` 词表单点
// ---------------------------------------------------------------------------

/// 正向：常量逐值 `["summary", "entries", "files"]`、长度恰 3（值集封闭）。
#[test]
fn drift_scopes词表封闭且有序() {
    assert_eq!(DRIFT_SCOPES, ["summary", "entries", "files"]);
    assert_eq!(DRIFT_SCOPES.len(), 3, "三值封闭");
    let mut unique = DRIFT_SCOPES.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 3, "无重复值");
}

/// 边界：三 scope 的产物逐级以「上一层产物」为起点——`files` 的每个键都能在
/// `entries` 中找到同值（纯投影纪律的可观测面：零第二次计数装配）。
#[test]
fn 三scope的值不重算() {
    let root = healthy_root("purity");
    let report = report_of(root.path());
    let entries = drift_entries(&report, 13);
    let files = drift_files(&report, 13);
    for key in keys(&entries) {
        assert_eq!(
            files[&key], entries[&key],
            "{key} 在 files 中与 entries 逐字段相等（增补不重算）"
        );
    }
}
