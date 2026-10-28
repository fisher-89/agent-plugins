//! `drift::report` 的单元测试：三组封闭词汇（`status` 五值 / 漂移四条 / 观察
//! 两条）+ 豁免来源 / 模式两值的 `ALL` 长度与线形、完整报告 JSON 往返、camelCase
//! 字段面、`Option` 字段的 `null` 形态与空报告确定性。
//!
//! 零 mock：真实 `serde_json` 序列化 / 反序列化（线形即被测对象）；零 IO、零
//! 判定（全内存构造）。

use crate::dto::{ArchiDiagnostic, DiagnosticKind, GateTier};

use super::report::{
    ArchiCheckStatus, ArchiDriftEntry, ArchiDriftKind, ArchiDriftMode, ArchiDriftModel,
    ArchiDriftObservation, ArchiDriftOccurrence, ArchiDriftReport, ArchiDriftSuppressedEntry,
    ArchiExcludedFile, ArchiGateCount, ArchiMatchedElement, ArchiObservationKind, ArchiScanReport,
    ArchiSuppressVia, ArchiUnresolvedFile, Report,
};

// ---------------------------------------------------------------------------
// 构造夹具
// ---------------------------------------------------------------------------

/// 单条命中点。
fn occurrence(file: &str, line: u32) -> ArchiDriftOccurrence {
    ArchiDriftOccurrence {
        file: file.to_owned(),
        line,
        specifier: Some("./x".to_owned()),
        type_only: false,
    }
}

/// 单条漂移条目（`undeclaredDependency`）。
fn entry(from: &str, to: &str) -> ArchiDriftEntry {
    ArchiDriftEntry {
        kind: ArchiDriftKind::UndeclaredDependency,
        from: Some(from.to_owned()),
        to: Some(to.to_owned()),
        gate: Some(GateTier::Strict),
        message: "未声明依赖：App.A → App.B".to_owned(),
        occurrences: vec![occurrence("src/a/x.ts", 3)],
    }
}

/// 结构诊断（`dto::ArchiDiagnostic` 复用，D11）。
fn diagnostic() -> ArchiDiagnostic {
    ArchiDiagnostic {
        kind: DiagnosticKind::DependsOnAncestor,
        message: "下游依赖祖先".to_owned(),
        file: ".dev-team/architecture/models/01.toml".to_owned(),
        line: 7,
        column: Some(3),
    }
}

/// 扫描面事实（全字段填一条）。
fn scan_report() -> ArchiScanReport {
    ArchiScanReport {
        mode: ArchiDriftMode::Full,
        changed_files: 1,
        scanned_files: 2,
        excluded: vec![ArchiExcludedFile {
            file: "src/a/x.test.ts".to_owned(),
            rule: "**/*.test.ts".to_owned(),
        }],
        unmatched_files: vec!["src/other/y.ts".to_owned()],
        matched: vec![ArchiMatchedElement {
            element: "App.A".to_owned(),
            files: vec!["src/a/x.ts".to_owned()],
        }],
        unresolved: vec![ArchiUnresolvedFile {
            file: "src/a/dyn.ts".to_owned(),
            specifier: None,
            message: "动态 import()".to_owned(),
        }],
    }
}

/// 十四字段全填充的报告。
fn full_report() -> ArchiDriftReport {
    ArchiDriftReport {
        version: 1,
        timestamp: "2026-10-20T08:31:12Z".to_owned(),
        mode: ArchiDriftMode::Incremental,
        root: "/tmp/workspace".to_owned(),
        change: Some("archi-check-and-gate".to_owned()),
        base: Some("HEAD".to_owned()),
        model_hash: "sha256:00ff".to_owned(),
        model: ArchiDriftModel {
            elements: 2,
            relations: 1,
            by_gate: ArchiGateCount {
                strict: 1,
                warn: 1,
                off: 0,
            },
            structural_errors: vec![diagnostic()],
            warnings: vec![ArchiDriftEntry {
                kind: ArchiDriftKind::NoPath,
                from: Some("App.B".to_owned()),
                to: None,
                gate: Some(GateTier::Warn),
                message: "元素 App.B 未声明 path".to_owned(),
                occurrences: Vec::new(),
            }],
        },
        scan: scan_report(),
        errors: vec![entry("App.A", "App.B")],
        warnings: vec![ArchiDriftEntry {
            kind: ArchiDriftKind::AmbiguousOwner,
            from: None,
            to: None,
            gate: None,
            message: "归属不确定".to_owned(),
            occurrences: Vec::new(),
        }],
        suppressed: vec![ArchiDriftSuppressedEntry {
            from: "App.A".to_owned(),
            to: "App.B".to_owned(),
            via: ArchiSuppressVia::Config,
            reason: "迁移期".to_owned(),
            file: None,
            line: None,
        }],
        observations: vec![ArchiDriftObservation {
            kind: ArchiObservationKind::UnfulfilledDeclaration,
            from: "App.A".to_owned(),
            to: "App.B".to_owned(),
            message: "本次扫描面内未见 App.A → App.B 的 import".to_owned(),
        }],
        status: ArchiCheckStatus::Errors,
    }
}

/// 全空数组 + `clean` 的报告。
fn empty_report() -> ArchiDriftReport {
    ArchiDriftReport {
        version: 1,
        timestamp: "2026-10-20T08:31:12Z".to_owned(),
        mode: ArchiDriftMode::Full,
        root: "/tmp/workspace".to_owned(),
        change: None,
        base: None,
        model_hash: "sha256:e3b0".to_owned(),
        model: ArchiDriftModel {
            elements: 0,
            relations: 0,
            by_gate: ArchiGateCount::default(),
            structural_errors: Vec::new(),
            warnings: Vec::new(),
        },
        scan: ArchiScanReport {
            mode: ArchiDriftMode::Full,
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

// ---------------------------------------------------------------------------
// 正向：词汇封闭与线形
// ---------------------------------------------------------------------------

/// 正向（AC-8）：`ArchiCheckStatus` 五值——`ALL` 长度 5、`as_str` 与 serde 线词
/// 逐值一致（`noChanges` 为 camelCase 线形）、无重复。
#[test]
fn 状态词汇五值封闭() {
    assert_eq!(ArchiCheckStatus::ALL.len(), 5, "五值清单");
    let expected = [
        (ArchiCheckStatus::Clean, "clean"),
        (ArchiCheckStatus::Errors, "errors"),
        (ArchiCheckStatus::Warnings, "warnings"),
        (ArchiCheckStatus::NoChanges, "noChanges"),
        (ArchiCheckStatus::Skipped, "skipped"),
    ];
    for (status, word) in expected {
        assert_eq!(status.as_str(), word, "as_str 词面");
        let json = serde_json::to_string(&status).expect("序列化失败");
        assert_eq!(json, format!("\"{word}\""), "serde 线词与 as_str 一致");
        let back: ArchiCheckStatus = serde_json::from_str(&json).expect("反序列化失败");
        assert_eq!(back, status, "往返恒等");
    }
    let distinct: std::collections::BTreeSet<&str> =
        expected.iter().map(|(_, word)| *word).collect();
    assert_eq!(distinct.len(), 5, "五值线词互不相同");
}

/// 正向：`ArchiDriftMode` 二值 `ALL` 与线形往返。
#[test]
fn 模式词汇二值() {
    assert_eq!(ArchiDriftMode::ALL.len(), 2, "二值清单");
    for (mode, word) in [
        (ArchiDriftMode::Full, "full"),
        (ArchiDriftMode::Incremental, "incremental"),
    ] {
        assert_eq!(mode.as_str(), word, "as_str 词面");
        let json = serde_json::to_string(&mode).expect("序列化失败");
        assert_eq!(json, format!("\"{word}\""), "小写线形");
        assert_eq!(
            serde_json::from_str::<ArchiDriftMode>(&json).expect("反序列化失败"),
            mode,
            "往返恒等"
        );
    }
}

/// 正向：`ArchiDriftKind` 四值 `ALL` 与 camelCase 线形往返。
#[test]
fn 漂移词汇四值() {
    assert_eq!(ArchiDriftKind::ALL.len(), 4, "四值清单");
    for (kind, word) in [
        (ArchiDriftKind::UndeclaredDependency, "undeclaredDependency"),
        (ArchiDriftKind::PathNotFound, "pathNotFound"),
        (ArchiDriftKind::AmbiguousOwner, "ambiguousOwner"),
        (ArchiDriftKind::NoPath, "noPath"),
    ] {
        assert_eq!(kind.as_str(), word, "as_str 词面");
        let json = serde_json::to_string(&kind).expect("序列化失败");
        assert_eq!(json, format!("\"{word}\""), "camelCase 线形");
        assert_eq!(
            serde_json::from_str::<ArchiDriftKind>(&json).expect("反序列化失败"),
            kind,
            "往返恒等"
        );
    }
}

/// 正向：`ArchiObservationKind` 二值（`unusedDeclaration` 本期不产出但词汇位
/// 保留，D1）。
#[test]
fn 观察词汇二值() {
    assert_eq!(ArchiObservationKind::ALL.len(), 2, "二值清单");
    for (kind, word) in [
        (
            ArchiObservationKind::UnfulfilledDeclaration,
            "unfulfilledDeclaration",
        ),
        (ArchiObservationKind::UnusedDeclaration, "unusedDeclaration"),
    ] {
        assert_eq!(kind.as_str(), word, "as_str 词面");
        let json = serde_json::to_string(&kind).expect("序列化失败");
        assert_eq!(json, format!("\"{word}\""), "camelCase 线形");
        assert_eq!(
            serde_json::from_str::<ArchiObservationKind>(&json).expect("反序列化失败"),
            kind,
            "往返恒等"
        );
    }
}

/// 正向：`ArchiSuppressVia` 二值小写线形往返。
#[test]
fn 豁免来源二值() {
    for (via, word) in [
        (ArchiSuppressVia::Config, "config"),
        (ArchiSuppressVia::Inline, "inline"),
    ] {
        assert_eq!(via.as_str(), word, "as_str 词面");
        let json = serde_json::to_string(&via).expect("序列化失败");
        assert_eq!(json, format!("\"{word}\""), "小写线形");
        assert_eq!(
            serde_json::from_str::<ArchiSuppressVia>(&json).expect("反序列化失败"),
            via,
            "往返恒等"
        );
    }
}

// ---------------------------------------------------------------------------
// 异常：词汇外反序列化
// ---------------------------------------------------------------------------

/// 异常：词汇外 `status` 值（带空格 / 未知词 / 大驼峰 / 下划线形态）→ 全 `Err`
/// （封闭词汇严格，零宽松归一）。
#[test]
fn 词汇外状态值反序列化报错() {
    for raw in ["\"clean \"", "\"unknown\"", "\"Clean\"", "\"no_changes\""] {
        let outcome: Result<ArchiCheckStatus, _> = serde_json::from_str(raw);
        assert!(outcome.is_err(), "{raw} 不在封闭词汇内必须 Err");
    }
}

// ---------------------------------------------------------------------------
// 边界：别名与完整往返
// ---------------------------------------------------------------------------

/// 边界：`Report` 与 `ArchiDriftReport` 是同一类型（编译期可互相赋值 + 同值序列化
/// 字节相等）。
#[test]
fn 短名别名同一类型() {
    let wide: ArchiDriftReport = full_report();
    let narrow: Report = wide.clone();
    let back: ArchiDriftReport = narrow;
    assert_eq!(
        serde_json::to_vec(&wide).expect("序列化失败"),
        serde_json::to_vec(&back).expect("序列化失败"),
        "别名两侧序列化字节相等"
    );
}

/// 正向：十四字段全填充报告 JSON 往返逐字段相等（嵌套 `occurrences` 亦逐项）。
#[test]
fn 完整报告往返() {
    let report = full_report();
    let text = serde_json::to_string(&report).expect("序列化失败");
    let back: ArchiDriftReport = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back, report, "逐字段往返相等");
    assert_eq!(
        back.errors[0].occurrences, report.errors[0].occurrences,
        "嵌套 occurrences 逐项相等"
    );
    assert_eq!(back.scan, report.scan, "扫描段逐字段相等");
}

/// 边界：字段线形为 camelCase——含 `modelHash` / `scannedFiles` / `changedFiles` /
/// `structuralErrors` / `byGate` / `typeOnly` / `unmatchedFiles`，不含 snake_case
/// 形态。
#[test]
fn 字段线形camel_case() {
    let text = serde_json::to_string(&full_report()).expect("序列化失败");
    for needle in [
        "modelHash",
        "scannedFiles",
        "changedFiles",
        "structuralErrors",
        "byGate",
        "typeOnly",
        "unmatchedFiles",
    ] {
        assert!(text.contains(needle), "线面应含 camelCase 字段 {needle}");
    }
    for snake in [
        "model_hash",
        "structural_errors",
        "type_only",
        "unmatched_files",
    ] {
        assert!(
            !text.contains(snake),
            "线面 MUST NOT 出现 snake_case {snake}"
        );
    }
}

/// 边界：`Option` 族字段的 `null` 形态与往返（`change` / `base` / 条目
/// `from`·`to`·`gate` / `suppressed` 的 `file`·`line` / `unresolved.specifier`）。
#[test]
fn option字段null往返() {
    let text = serde_json::to_string(&full_report()).expect("序列化失败");
    assert!(text.contains("\"change\":"), "change 字段在案");
    assert!(
        text.contains("\"file\":null") || text.contains("\"line\":null"),
        "config 豁免的 file / line 为 null"
    );

    let report = empty_report();
    let text = serde_json::to_string(&report).expect("序列化失败");
    assert!(text.contains("\"change\":null"), "无 change 上下文为 null");
    assert!(text.contains("\"base\":null"), "全量模式 base 为 null");
    let back: ArchiDriftReport = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back.change, None, "null 往返为 None");
    assert_eq!(back.base, None, "null 往返为 None");

    let entry = ArchiDriftEntry {
        kind: ArchiDriftKind::NoPath,
        from: None,
        to: None,
        gate: None,
        message: "无归因".to_owned(),
        occurrences: Vec::new(),
    };
    let text = serde_json::to_string(&entry).expect("序列化失败");
    let back: ArchiDriftEntry = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back, entry, "三 Option 字段全 None 往返成立");

    let suppressed = ArchiDriftSuppressedEntry {
        from: "App.A".to_owned(),
        to: "App.B".to_owned(),
        via: ArchiSuppressVia::Config,
        reason: "迁移期".to_owned(),
        file: None,
        line: None,
    };
    let back: ArchiDriftSuppressedEntry =
        serde_json::from_str(&serde_json::to_string(&suppressed).expect("序列化失败"))
            .expect("反序列化失败");
    assert_eq!(back, suppressed, "file / line 双 None 往返成立");

    let unresolved = ArchiUnresolvedFile {
        file: "src/a.ts".to_owned(),
        specifier: None,
        message: "不可判定".to_owned(),
    };
    let back: ArchiUnresolvedFile =
        serde_json::from_str(&serde_json::to_string(&unresolved).expect("序列化失败"))
            .expect("反序列化失败");
    assert_eq!(back, unresolved, "specifier None 往返成立");
}

/// 边界：全空数组的 `clean` 报告两次 `to_vec_pretty` 字节相等（无 HashMap 序漂移），
/// 且序列化文本不含 `"passed"`（结论由门禁侧派生，不双写）。
#[test]
fn 空报告确定性与契约面() {
    let report = empty_report();
    let first = serde_json::to_vec_pretty(&report).expect("序列化失败");
    let second = serde_json::to_vec_pretty(&report).expect("序列化失败");
    assert_eq!(first, second, "两次序列化字节相等（确定性）");
    let text = String::from_utf8(first).expect("UTF-8");
    assert!(text.contains("\"status\": \"clean\""), "空报告为 clean");
    assert!(
        !text.contains("\"passed\""),
        "报告不携带 passed（门禁侧派生）"
    );
    let back: ArchiDriftReport = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back, report, "空报告往返恒等");
}

// ---------------------------------------------------------------------------
// 边界：结构诊断复用与计数 / 数值边界
// ---------------------------------------------------------------------------

/// 边界（D11）：`model.structural_errors` 元素为 `dto::ArchiDiagnostic`，其 `kind`
/// 走 `DiagnosticKind` 词汇（模型面零新增词，封闭九值）。
#[test]
fn 结构诊断复用dto词汇() {
    let report = full_report();
    let structural: &ArchiDiagnostic = &report.model.structural_errors[0];
    assert_eq!(
        structural.kind,
        DiagnosticKind::DependsOnAncestor,
        "结构诊断 kind 复用 dto 词汇"
    );
    assert_eq!(DiagnosticKind::ALL.len(), 9, "封闭九值不变");
    let back: ArchiDriftReport =
        serde_json::from_str(&serde_json::to_string(&report).expect("序列化失败"))
            .expect("反序列化失败");
    assert_eq!(back.model.structural_errors, report.model.structural_errors);
}

/// 边界：位置未知口径（`line = 0`）与 `u32::MAX` 越界值往返不溢出。
#[test]
fn 行号边界往返() {
    for line in [0u32, u32::MAX] {
        let item = occurrence("src/a/x.ts", line);
        let back: ArchiDriftOccurrence =
            serde_json::from_str(&serde_json::to_string(&item).expect("序列化失败"))
                .expect("反序列化失败");
        assert_eq!(back.line, line, "line = {line} 往返保持");
    }
}

/// 边界：`ArchiGateCount` 零值与 `u32::MAX` 两形态往返相等（无饱和 / 无符号翻转）。
#[test]
fn 计数边界往返() {
    for count in [
        ArchiGateCount {
            strict: 0,
            warn: 0,
            off: 0,
        },
        ArchiGateCount {
            strict: u32::MAX,
            warn: u32::MAX,
            off: u32::MAX,
        },
    ] {
        let back: ArchiGateCount =
            serde_json::from_str(&serde_json::to_string(&count).expect("序列化失败"))
                .expect("反序列化失败");
        assert_eq!(back, count, "计数边界往返相等");
    }
}

/// 边界：1000 条 `errors`（各带 100 occurrence）序列化 / 反序列化成功且长度不变；
/// `message` >1000 chars 且含 emoji 与 NUL 的转义往返逐字相等。
#[test]
fn 超大与超长面() {
    let mut report = empty_report();
    report.status = ArchiCheckStatus::Errors;
    report.errors = (0..1000)
        .map(|index| ArchiDriftEntry {
            kind: ArchiDriftKind::UndeclaredDependency,
            from: Some(format!("App.F{index}")),
            to: Some(format!("App.T{index}")),
            gate: Some(GateTier::Strict),
            message: format!("未声明依赖 {index}"),
            occurrences: (0..100).map(|_| occurrence("src/a/x.ts", 1)).collect(),
        })
        .collect();
    let text = serde_json::to_string(&report).expect("序列化失败");
    let back: ArchiDriftReport = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back.errors.len(), 1000, "千条 errors 长度不变");
    assert_eq!(back.errors[999].occurrences.len(), 100, "每条一百个命中点");

    let long_message = format!("{}🎯\u{0}", "长".repeat(1100));
    let entry = ArchiDriftEntry {
        kind: ArchiDriftKind::UndeclaredDependency,
        from: Some("App.A".to_owned()),
        to: Some("App.B".to_owned()),
        gate: Some(GateTier::Strict),
        message: long_message.clone(),
        occurrences: Vec::new(),
    };
    let back: ArchiDriftEntry =
        serde_json::from_str(&serde_json::to_string(&entry).expect("序列化失败"))
            .expect("反序列化失败");
    assert_eq!(back.message, long_message, "超长 / 特殊字符文案逐字往返");
}

// ---------------------------------------------------------------------------
// 边界：模型面字段面
// ---------------------------------------------------------------------------

/// 边界：`ArchiDriftModel` 计数与 `by_gate` 逐值往返；`metadata` 无关键不入报告
/// 结构（报告不引 `metadata`，防词汇扩散）。
#[test]
fn 模型面字段面往返() {
    let model = ArchiDriftModel {
        elements: 300,
        relations: 1000,
        by_gate: ArchiGateCount {
            strict: 100,
            warn: 150,
            off: 50,
        },
        structural_errors: Vec::new(),
        warnings: vec![entry("App.A", "App.B")],
    };
    let text = serde_json::to_string(&model).expect("序列化失败");
    assert!(!text.contains("metadata"), "报告模型段不含 metadata 键");
    let back: ArchiDriftModel = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back, model, "模型面逐字段往返相等");
    assert_eq!(
        back.by_gate.strict + back.by_gate.warn + back.by_gate.off,
        300,
        "档位分布之和等于元素数"
    );
}
