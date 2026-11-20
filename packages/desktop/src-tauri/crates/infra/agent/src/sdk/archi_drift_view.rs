//! 引擎侧漂移投影单点：`archi_drift` 工具响应与收口 notice 共用 [`drift_summary`]，
//! 零第二份装配。本模块零判定、零 IO——报告由 `archi::drift::check` 产出，这里只裁剪。
//!
//! 三 scope 逐级追加（`summary` ⊂ `entries` ⊂ `files`）：`summary` 只出计数与全量
//! 条目（`unmatchedFiles` 这类常态量**只出现在计数位**），`entries` 追加观察项与
//! 契约判据条目，`files` 追加四类扫描清单（只在排查归属问题时用）。

use std::collections::BTreeMap;

use archi::drift::report::ApiFindingSeverity;
use archi::drift::Report;
use serde_json::{json, Value};

/// `scope` 词表（单源：`mount` 的 JSON `enum` 与执行体的 `Err` 文案都取它，MUST NOT
/// 手抄第二份）。
pub(crate) const DRIFT_SCOPES: [&str; 3] = ["summary", "entries", "files"];

/// 缺省 scope 的投影（计数 + 全量条目 + `durationMs`）。
pub(crate) fn drift_summary(report: &Report, duration_ms: u64) -> Value {
    let mut by_severity: BTreeMap<&'static str, usize> = BTreeMap::new();
    for severity in ApiFindingSeverity::ALL {
        by_severity.insert(severity.as_str(), 0);
    }
    let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
    for finding in &report.api_findings {
        *by_severity
            .entry(finding.kind.severity().as_str())
            .or_insert(0) += 1;
        *by_kind.entry(finding.kind.as_str()).or_insert(0) += 1;
    }
    json!({
        "mode": report.mode.as_str(),
        "status": report.status.as_str(),
        "passed": report.passed(),
        "modelHash": report.model_hash,
        "model": {
            "elements": report.model.elements,
            "relations": report.model.relations,
            "byGate": {
                "strict": report.model.by_gate.strict,
                "warn": report.model.by_gate.warn,
                "off": report.model.by_gate.off,
            },
            "structuralErrors": report.model.structural_errors,
            "structuralErrorCount": report.model.structural_errors.len(),
        },
        "counts": {
            "errors": report.errors.len(),
            "warnings": report.warnings.len(),
            "suppressed": report.suppressed.len(),
            "observations": report.observations.len(),
            "modelWarnings": report.model.warnings.len(),
            "apiFindings": {
                "total": report.api_findings.len(),
                "bySeverity": by_severity,
            },
            "scan": {
                "changedFiles": report.scan.changed_files,
                "scannedFiles": report.scan.scanned_files,
                "excluded": report.scan.excluded.len(),
                "unmatched": report.scan.unmatched_files.len(),
                "matched": report.scan.matched.len(),
                "unresolved": report.scan.unresolved.len(),
            },
        },
        "errors": report.errors,
        "warnings": report.warnings,
        "modelWarnings": report.model.warnings,
        "apiFindings": { "byKind": by_kind },
        "durationMs": duration_ms,
    })
}

/// `summary` 之上追加观察项与契约判据条目全量（计数仍由 `summary` 持有）。
pub(crate) fn drift_entries(report: &Report, duration_ms: u64) -> Value {
    let value = drift_summary(report, duration_ms);
    let Value::Object(mut object) = value else {
        return value;
    };
    object.insert("observations".to_owned(), json!(report.observations));
    if let Some(Value::Object(api_findings)) = object.get_mut("apiFindings") {
        api_findings.insert("items".to_owned(), json!(report.api_findings));
    }
    Value::Object(object)
}

/// `entries` 之上追加四类扫描清单（`unmatched` / `matched` / `excluded` /
/// `unresolved` 逐条）；只在排查归属问题时用。
pub(crate) fn drift_files(report: &Report, duration_ms: u64) -> Value {
    let value = drift_entries(report, duration_ms);
    let Value::Object(mut object) = value else {
        return value;
    };
    object.insert(
        "files".to_owned(),
        json!({
            "unmatched": report.scan.unmatched_files,
            "matched": report.scan.matched,
            "excluded": report.scan.excluded,
            "unresolved": report.scan.unresolved,
        }),
    );
    Value::Object(object)
}
