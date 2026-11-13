//! `drift::report` 的单元测试：三组封闭词汇（`status` 五值 / 漂移四条 / 观察
//! 两条）+ 豁免来源 / 模式两值的 `ALL` 长度与线形、完整报告 JSON 往返、camelCase
//! 字段面、`Option` 字段的 `null` 形态与空报告确定性。
//!
//! 零 mock：真实 `serde_json` 序列化 / 反序列化（线形即被测对象）；零 IO、零
//! 判定（全内存构造）。

use crate::dto::{ArchiDiagnostic, DiagnosticKind, GateTier};

use super::report::{
    ArchiApiFinding, ArchiApiFindingKind, ArchiCheckStatus, ArchiDriftEntry, ArchiDriftKind,
    ArchiDriftMode, ArchiDriftModel, ArchiDriftObservation, ArchiDriftOccurrence, ArchiDriftReport,
    ArchiDriftSuppressedEntry, ArchiExcludedFile, ArchiGateCount, ArchiMatchedElement,
    ArchiObservationKind, ArchiScanReport, ArchiSuppressVia, ArchiUnresolvedFile, Report,
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
        version: 3,
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
        api_findings: Vec::new(),
        status: ArchiCheckStatus::Errors,
    }
}

/// 全空数组 + `clean` 的报告。
fn empty_report() -> ArchiDriftReport {
    ArchiDriftReport {
        version: 3,
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
        api_findings: Vec::new(),
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

// ===========================================================================
// 契约判据词汇与 `apiFindings` 槽位（`module-api-contract-format` / AC-8）
// ===========================================================================

use super::report::ApiFindingSeverity;

/// 全字段填充的契约判据条目（`occurrences` 两条）。
fn api_finding() -> ArchiApiFinding {
    ArchiApiFinding {
        kind: ArchiApiFindingKind::ApiSignatureMismatch,
        category: "signature".to_owned(),
        element: Some("App.Backend.Infra.ArchiRuntime".to_owned()),
        target: Some("file_stem".to_owned()),
        message: "元素 App.Backend.Infra.ArchiRuntime 的契约签名与代码不一致".to_owned(),
        occurrences: vec![
            ArchiDriftOccurrence {
                file: ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md"
                    .to_owned(),
                line: 31,
                specifier: Some("file_stem".to_owned()),
                type_only: false,
            },
            ArchiDriftOccurrence {
                file: ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md"
                    .to_owned(),
                line: 32,
                specifier: Some("file_stem".to_owned()),
                type_only: false,
            },
        ],
    }
}

/// 正向（AC-8）：十值 `ALL` 长度 10、`as_str` 逐值 camelCase、无重复。
#[test]
fn 契约判据词汇十值封闭() {
    assert_eq!(ArchiApiFindingKind::ALL.len(), 10, "十值清单");
    let expected = [
        (ArchiApiFindingKind::ApiEntryMissing, "apiEntryMissing"),
        (ArchiApiFindingKind::ApiMissing, "apiMissing"),
        (ArchiApiFindingKind::ApiUndeclared, "apiUndeclared"),
        (
            ArchiApiFindingKind::ApiSignatureMismatch,
            "apiSignatureMismatch",
        ),
        (ArchiApiFindingKind::ApiUnexpected, "apiUnexpected"),
        (ArchiApiFindingKind::ApiShadowing, "apiShadowing"),
        (ArchiApiFindingKind::ApiRefMissing, "apiRefMissing"),
        (
            ArchiApiFindingKind::ApiNestedCrossCrate,
            "apiNestedCrossCrate",
        ),
        (ArchiApiFindingKind::ApiReexportDrift, "apiReexportDrift"),
        (
            ArchiApiFindingKind::ApiConstructorDeclared,
            "apiConstructorDeclared",
        ),
    ];
    assert_eq!(expected.len(), 10, "表驱动十值");
    for (kind, word) in expected {
        assert_eq!(kind.as_str(), word, "{kind:?} 的线词");
        assert_eq!(
            serde_json::to_value(kind).expect("序列化失败"),
            serde_json::json!(word),
            "{kind:?} 的 serde 线形与 as_str 一致"
        );
        let back: ArchiApiFindingKind =
            serde_json::from_value(serde_json::json!(word)).expect("反序列化失败");
        assert_eq!(back, kind, "{word} 往返逐值相等");
    }
    let mut words: Vec<&str> = ArchiApiFindingKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    let all_words = words.clone();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), 10, "ALL 无重复值");
    assert_eq!(
        all_words,
        expected
            .iter()
            .map(|(_, word)| *word)
            .collect::<Vec<&str>>(),
        "ALL 顺序与 spec 表一致"
    );
}

/// 正向（AC-7 / AC-8 / D14）：`severity()` 九 `Warn` 一 `Off`（仅完备性判据 `off`）；
/// `ApiFindingSeverity` 二值 `ALL` 与线词；与 `GateTier` 类型分立（各自 `match` 穷尽）。
#[test]
fn 契约判据档位九warn一off() {
    let warn: Vec<ArchiApiFindingKind> = ArchiApiFindingKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.severity() == ApiFindingSeverity::Warn)
        .collect();
    let off: Vec<ArchiApiFindingKind> = ArchiApiFindingKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.severity() == ApiFindingSeverity::Off)
        .collect();
    assert_eq!(warn.len(), 9, "九条一致性判据 warn：{warn:?}");
    assert_eq!(off.len(), 1, "唯一完备性判据 off：{off:?}");
    assert_eq!(
        off,
        vec![ArchiApiFindingKind::ApiUndeclared],
        "off 档恒为 apiUndeclared"
    );

    assert_eq!(ApiFindingSeverity::ALL.len(), 2, "档位二值");
    assert_eq!(ApiFindingSeverity::Off.as_str(), "off");
    assert_eq!(ApiFindingSeverity::Warn.as_str(), "warn");

    // 类型分立：两枚举各自穷尽 match（无 `From`、无互通）
    let from_gate = match GateTier::Off {
        GateTier::Off => "off",
        GateTier::Warn => "warn",
        GateTier::Strict => "strict",
    };
    let from_severity = match ApiFindingSeverity::Off {
        ApiFindingSeverity::Off => "off",
        ApiFindingSeverity::Warn => "warn",
    };
    assert_eq!(from_gate, from_severity, "词面相同（`off`）但类型分立");
}

/// 异常（Enum N+1）：词汇外十形态 → 反序列化全 `Err`（封闭词汇严格，零宽松归一）。
#[test]
fn 契约判据词汇外反序列化即错() {
    for word in [
        "apiEntryMissing ",
        "unknown",
        "ApiEntryMissing",
        "api_entry_missing",
        "",
        "APIMISSING",
        "apiMissing\n",
        "apiFinding",
        "off",
        "\u{0}",
    ] {
        let result = serde_json::from_value::<ArchiApiFindingKind>(serde_json::json!(word));
        assert!(result.is_err(), "词汇外应 Err：{word:?}");
    }
}

/// 正向（AC-8）：`ArchiApiFinding` 六字段往返逐字段相等（`occurrences` 亦逐项）；
/// 文本含六键名。
#[test]
fn 契约判据条目六字段往返() {
    let finding = api_finding();
    let text = serde_json::to_string(&finding).expect("序列化失败");
    for key in [
        "kind",
        "category",
        "element",
        "target",
        "message",
        "occurrences",
    ] {
        assert!(
            text.contains(&format!("\"{key}\"")),
            "文本含键 `{key}`：{text}"
        );
    }
    let back: ArchiApiFinding = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back, finding, "逐字段往返相等");
    assert_eq!(back.occurrences.len(), 2, "occurrences 逐项保留");
    assert_eq!(back.occurrences[1].line, 32, "occurrences 的 line 保留");
}

/// 边界（AC-8）：`element` / `target` 为 `None` → 文本含 `null` 且往返回 `None`；
/// `occurrences = []` → 文本含 `[]`（不是省略字段）；`category` 为开放值集（六值 +
/// 域外字符串均可往返）。
#[test]
fn 契约判据条目空字段与开放分类() {
    let bare = ArchiApiFinding {
        kind: ArchiApiFindingKind::ApiEntryMissing,
        category: "entry".to_owned(),
        element: None,
        target: None,
        message: "元素无入口".to_owned(),
        occurrences: Vec::new(),
    };
    let text = serde_json::to_string(&bare).expect("序列化失败");
    assert!(
        text.contains("\"element\":null"),
        "element 显式 null：{text}"
    );
    assert!(text.contains("\"target\":null"), "target 显式 null：{text}");
    assert!(
        text.contains("\"occurrences\":[]"),
        "空 occurrences 为 []：{text}"
    );
    let back: ArchiApiFinding = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back.element, None, "element 往返回 None");
    assert_eq!(back.target, None, "target 往返回 None");

    for category in [
        "entry",
        "contract",
        "signature",
        "reexport",
        "nesting",
        "constructor",
        "未来新分类",
    ] {
        let finding = ArchiApiFinding {
            category: category.to_owned(),
            ..api_finding()
        };
        let back: ArchiApiFinding =
            serde_json::from_str(&serde_json::to_string(&finding).expect("序列化失败"))
                .expect("反序列化失败");
        assert_eq!(back.category, category, "开放值集往返：{category}");
    }
}

/// 边界（AC-8）：`api_findings = []` → 文本含 `"apiFindings":[]`；既有字段面零变化
/// （16 键 = 既有 15 + `apiFindings`）；`status` 恒为末字段、`apiFindings` 紧邻其前。
#[test]
fn 报告api_findings槽位与字段面() {
    let bare = empty_report();
    let text = serde_json::to_string(&bare).expect("序列化失败");
    assert!(text.contains("\"apiFindings\":[]"), "空槽位显式 []：{text}");
    let back: ArchiDriftReport = serde_json::from_str(&text).expect("反序列化失败");
    assert!(back.api_findings.is_empty(), "空槽位往返为空");

    let keys: Vec<String> = serde_json::to_value(&bare)
        .expect("序列化失败")
        .as_object()
        .expect("对象")
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys.len(), 15, "既有 14 字段 + apiFindings");
    let api_position = keys
        .iter()
        .position(|key| key == "apiFindings")
        .expect("apiFindings 在案");
    let status_position = keys
        .iter()
        .position(|key| key == "status")
        .expect("status 在案");
    assert_eq!(
        status_position,
        keys.len() - 1,
        "status 恒为末字段（结论面收口）"
    );
    assert_eq!(
        api_position + 1,
        status_position,
        "apiFindings 紧邻 status（末位追加的槽位）"
    );
    for key in [
        "version",
        "timestamp",
        "mode",
        "root",
        "change",
        "base",
        "modelHash",
        "model",
        "scan",
        "errors",
        "warnings",
        "suppressed",
        "observations",
        "apiFindings",
        "status",
    ] {
        assert!(keys.iter().any(|item| item == key), "既有键在案：{key}");
    }
}

/// 边界（AC-8）：`version` 可写 `3` / `2` / `0` / `u32::MAX` 四态往返；`version = 1` 的报告
/// 缺 `apiFindings` 时**反序列化失败**（零 `serde(default)`——消费方按版本分支，
/// MUST NOT 补默认值）。
#[test]
fn 报告版本档面与旧报告不兼容() {
    for version in [3_u32, 2, 0, u32::MAX] {
        let report = ArchiDriftReport {
            version,
            ..empty_report()
        };
        let back: ArchiDriftReport =
            serde_json::from_str(&serde_json::to_string(&report).expect("序列化失败"))
                .expect("反序列化失败");
        assert_eq!(back.version, version, "版本三态往返：{version}");
    }

    let mut value = serde_json::to_value(empty_report()).expect("序列化失败");
    value.as_object_mut().expect("对象").remove("apiFindings");
    value["version"] = serde_json::json!(1);
    let result = serde_json::from_value::<ArchiDriftReport>(value);
    assert!(
        result.is_err(),
        "version = 1 且缺 apiFindings 的报告 MUST 反序列化失败（零默认值补全）"
    );
}

/// 边界（D18）：`occurrences` 复用 `ArchiDriftOccurrence`（与 `ArchiDriftEntry.occurrences`
/// 同一类型，可互相赋值）；`line = 0` / `u32::MAX` 往返不溢出；1000 条 occurrence 往返
/// 长度不变。
#[test]
fn 契约判据occurrences复用既有形状() {
    let occurrence = ArchiDriftOccurrence {
        file: "src/a.ts".to_owned(),
        line: 0,
        specifier: None,
        type_only: true,
    };
    let as_entry: ArchiDriftEntry = ArchiDriftEntry {
        kind: ArchiDriftKind::UndeclaredDependency,
        from: None,
        to: None,
        gate: None,
        message: "同形状".to_owned(),
        occurrences: vec![occurrence.clone()],
    };
    let as_finding: ArchiApiFinding = ArchiApiFinding {
        occurrences: vec![occurrence],
        ..api_finding()
    };
    assert_eq!(
        as_entry.occurrences[0], as_finding.occurrences[0],
        "两结构共用同一 occurrence 类型"
    );

    for line in [0_u32, u32::MAX] {
        let finding = ArchiApiFinding {
            occurrences: vec![ArchiDriftOccurrence {
                line,
                ..api_finding().occurrences[0].clone()
            }],
            ..api_finding()
        };
        let back: ArchiApiFinding =
            serde_json::from_str(&serde_json::to_string(&finding).expect("序列化失败"))
                .expect("反序列化失败");
        assert_eq!(back.occurrences[0].line, line, "line 边界往返：{line}");
    }

    let many = ArchiApiFinding {
        occurrences: (0..1000)
            .map(|index| ArchiDriftOccurrence {
                file: format!("src/f{index}.ts"),
                line: index,
                specifier: None,
                type_only: false,
            })
            .collect(),
        ..api_finding()
    };
    let back: ArchiApiFinding =
        serde_json::from_str(&serde_json::to_string(&many).expect("序列化失败"))
            .expect("反序列化失败");
    assert_eq!(back.occurrences.len(), 1000, "1000 条 occurrence 完整往返");
}

/// 边界（AC-8）：1000 条 `api_findings`（各 100 occurrence）往返长度不变；`message`
/// 超 1000 chars 且含 emoji / 换行 / NUL 转义 → 往返逐字相等；两次 `to_vec_pretty` 字节
/// 相等（无映射序漂移）。
#[test]
fn 契约判据规模与特殊字符() {
    let findings: Vec<ArchiApiFinding> = (0..1000)
        .map(|index| ArchiApiFinding {
            target: Some(format!("op{index}")),
            occurrences: (0..100)
                .map(|line| ArchiDriftOccurrence {
                    file: format!("src/f{index}.ts"),
                    line,
                    specifier: Some(format!("op{index}")),
                    type_only: line % 2 == 0,
                })
                .collect(),
            ..api_finding()
        })
        .collect();
    let report = ArchiDriftReport {
        api_findings: findings,
        ..empty_report()
    };
    let back: ArchiDriftReport =
        serde_json::from_str(&serde_json::to_string(&report).expect("序列化失败"))
            .expect("反序列化失败");
    assert_eq!(back.api_findings.len(), 1000, "1000 条条目完整往返");
    assert_eq!(
        back.api_findings[999].occurrences.len(),
        100,
        "末端条目的 100 项 occurrence 完整"
    );

    let long = format!("{}\u{1F3AF}\n\u{0}", "长".repeat(1200));
    let finding = ArchiApiFinding {
        message: long.clone(),
        ..api_finding()
    };
    let back: ArchiApiFinding =
        serde_json::from_str(&serde_json::to_string(&finding).expect("序列化失败"))
            .expect("反序列化失败");
    assert_eq!(back.message, long, "超长 / 特殊字符文案逐字往返");

    let first = serde_json::to_vec_pretty(&report).expect("序列化失败");
    let second = serde_json::to_vec_pretty(&report).expect("序列化失败");
    assert_eq!(first, second, "两次 pretty 序列化字节相等");
}

/// 边界（AC-8 源码自证）：`api_findings` 在结构体内**末位追加**（`observations` 与
/// `status` 之间）、无 `#[serde(default)]`（旧报告不补默认值）、`category` 是开放值集
/// 字符串（无 `ALL`）、档位唯一来源是本文件的 `severity()`。
#[test]
fn 报告结构源码自证槽位形态() {
    let source = include_str!("report.rs");
    let struct_start = source
        .find("pub struct ArchiDriftReport {")
        .expect("报告结构在案");
    let body = &source[struct_start..];
    let body = &body[..body.find("\n}").expect("结构体收口")];

    let observations = body.find("pub observations:").expect("observations 字段");
    let slot = body.find("pub api_findings:").expect("api_findings 字段");
    let status = body.find("pub status:").expect("status 字段");
    assert!(
        observations < slot && slot < status,
        "api_findings 位于 observations 与 status 之间（末位追加、status 收口）"
    );
    let field_count = body
        .lines()
        .filter(|line| line.trim_start().starts_with("pub ") && line.trim_end().ends_with(','))
        .count();
    assert_eq!(
        field_count, 15,
        "报告字段面 = 既有 14 + api_findings（零插入、零改名字段）"
    );

    let slot_block = &body[slot..status];
    assert!(
        !slot_block.contains("serde(default)"),
        "api_findings MUST NOT 带 serde(default)（旧报告按 version 分支，不补默认值）"
    );

    let category = source
        .find("pub category: String")
        .expect("category 为开放字符串");
    assert!(category > 0);
    assert!(
        !source.contains("impl ArchiApiFindingCategory"),
        "category 无封闭枚举（开放值集，无 ALL 常量）"
    );

    assert!(
        source.contains("ArchiApiFindingKind::ApiUndeclared => ApiFindingSeverity::Off"),
        "判据级档位唯一来源在 report.rs 的 severity()"
    );
    assert!(
        !source.contains("GateTier::Strict => ApiFindingSeverity"),
        "判据档位与元素 gate 解耦（MUST NOT 由 GateTier 派生）"
    );
}

/// 正向（AC-8）：非空 `api_findings`（三判据 / 三种分类 / occurrences 逐项）经
/// `ArchiDriftReport` 序列化 → 反序列化逐字段相等；文本含 `"apiFindings":[` 且 `kind`
/// 与 `category` 双在（槽位进出线一致）。
#[test]
fn 报告非空api_findings往返() {
    let findings = vec![
        ArchiApiFinding {
            kind: ArchiApiFindingKind::ApiEntryMissing,
            category: "entry".to_owned(),
            element: Some("App.Backend.Infra.ArchiRuntime".to_owned()),
            target: None,
            message: "元素无实现入口".to_owned(),
            occurrences: Vec::new(),
        },
        ArchiApiFinding {
            kind: ArchiApiFindingKind::ApiReexportDrift,
            category: "reexport".to_owned(),
            element: Some("App.Backend.Infra.ArchiRuntime".to_owned()),
            target: Some("App.Backend.Infra.ArchiRuntime".to_owned()),
            message: "透传键集不一致".to_owned(),
            occurrences: Vec::new(),
        },
        ArchiApiFinding {
            kind: ArchiApiFindingKind::ApiConstructorDeclared,
            category: "constructor".to_owned(),
            element: Some("App.Backend.Infra.ArchiRuntime".to_owned()),
            target: Some("new".to_owned()),
            message: "契约声明了构造器".to_owned(),
            occurrences: vec![ArchiDriftOccurrence {
                file: "Infra/App.Backend.Infra.ArchiRuntime.md".to_owned(),
                line: 3,
                specifier: Some("new".to_owned()),
                type_only: false,
            }],
        },
    ];
    let report = ArchiDriftReport {
        api_findings: findings,
        ..empty_report()
    };
    let text = serde_json::to_string(&report).expect("序列化失败");
    assert!(text.contains("\"apiFindings\":["), "非空槽位线形：{text}");
    assert!(
        text.contains("\"kind\":\"apiConstructorDeclared\"")
            && text.contains("\"category\":\"constructor\""),
        "非空条目 kind 与 category 双在：{text}"
    );
    let back: ArchiDriftReport = serde_json::from_str(&text).expect("反序列化失败");
    assert_eq!(back, report, "非空槽位逐字段往返相等");
    assert_eq!(back.api_findings.len(), 3, "三条条目完整往返");
    assert_eq!(
        back.api_findings[2].occurrences.len(),
        1,
        "op 级条目的 occurrences 逐项保留"
    );

    // 逐条两种线面：`element` / `target` 的 Some / None 两态均不省略字段
    for finding in &back.api_findings {
        let value = serde_json::to_value(finding).expect("序列化失败");
        let object = value.as_object().expect("对象");
        assert_eq!(object.len(), 6, "六字段面：{value}");
        for key in [
            "kind",
            "category",
            "element",
            "target",
            "message",
            "occurrences",
        ] {
            assert!(object.contains_key(key), "键 {key} 不被省略：{value}");
        }
    }
}

/// 边界（AC-8 金样对拍）：以既有金样 `tests/corpus/drift/expected/empty-model.full.json`
/// 为基线（`include_str!` 编译期内嵌）——新增字段之外的所有键名集合与值逐字相等，
/// 字段顺序上 `apiFindings` 追加在既有 14 字段之后、`status` 收口于末位。
#[test]
fn 既有金样字段面基线与末位追加() {
    const GOLDEN: &str = include_str!("../../tests/corpus/drift/expected/empty-model.full.json");
    let golden: serde_json::Value = serde_json::from_str(GOLDEN).expect("金样应为合法 JSON");
    let keys: Vec<&str> = golden
        .as_object()
        .expect("金样为对象")
        .keys()
        .map(String::as_str)
        .collect();

    // 既有 14 字段逐字在案（键名与顺序零变化）
    let existing = [
        "version",
        "timestamp",
        "mode",
        "root",
        "change",
        "base",
        "modelHash",
        "model",
        "scan",
        "errors",
        "warnings",
        "suppressed",
        "observations",
    ];
    for (index, key) in existing.iter().enumerate() {
        assert_eq!(keys[index], *key, "既有字段第 {index} 位逐字不变");
    }
    assert_eq!(keys.len(), 15, "既有 13 + apiFindings + status");
    let slot = keys
        .iter()
        .position(|key| *key == "apiFindings")
        .expect("金样含 apiFindings（旧金样未重写即红）");
    assert_eq!(slot, 13, "apiFindings 追加在既有字段之后");
    assert_eq!(keys[14], "status", "status 收口于末位");
    assert_eq!(
        golden["version"],
        serde_json::json!(3),
        "金样版本随 REPORT_VERSION = 3"
    );
    assert_eq!(
        golden["apiFindings"],
        serde_json::json!([]),
        "这些语料无契约面 ⇒ 空数组（不是省略字段）"
    );

    // 既有字段的**键名集合**与同形态报告逐字相等（字段面零变化：嵌套对象不因新增槽位
    // 增删任何键；值面由该语料自身的判定事实决定，不在此重算）
    let value = serde_json::to_value(empty_report()).expect("序列化失败");
    for key in ["model", "scan"] {
        let mut golden_keys: Vec<&str> = golden[key]
            .as_object()
            .expect("嵌套对象")
            .keys()
            .map(String::as_str)
            .collect();
        let mut report_keys: Vec<&str> = value[key]
            .as_object()
            .expect("嵌套对象")
            .keys()
            .map(String::as_str)
            .collect();
        golden_keys.sort_unstable();
        report_keys.sort_unstable();
        assert_eq!(
            golden_keys, report_keys,
            "嵌套字段面 {key} 的键名集合零变化"
        );
    }

    // 四槽位在本语料为空数组（既有语义零变化，不因新增槽位被改写）
    for key in ["errors", "warnings", "suppressed", "observations"] {
        assert_eq!(golden[key], serde_json::json!([]), "{key} 在本语料为空数组");
    }
}

// ---------------------------------------------------------------------------
// 本变更：版本 3 的线形与 `status` 五值（回归登记 + 新增口径）
// ---------------------------------------------------------------------------

/// 正向（AC-7 / D8）：`version = 3` 的报告往返后仍 3，且为 `number` 类型（非字符串）；
/// `api_findings` 槽位形状不变（空数组恰 `[]`、非空逐项六字段 camelCase）。
#[test]
fn 版本3线形与api_findings槽位() {
    let report = empty_report();
    assert_eq!(report.version, 3, "夹具版本 = REPORT_VERSION");
    let value = serde_json::to_value(&report).expect("序列化失败");
    assert!(
        value["version"].is_number(),
        "version 为 number（非字符串）：{:?}",
        value["version"]
    );
    assert_eq!(value["version"], serde_json::json!(3), "版本 3");
    assert_eq!(
        value["apiFindings"],
        serde_json::json!([]),
        "空槽位恰 []（非 null）"
    );

    let filled = ArchiDriftReport {
        api_findings: vec![ArchiApiFinding {
            kind: ArchiApiFindingKind::ApiSignatureMismatch,
            category: "signature".to_owned(),
            element: Some("App.Core.Alpha".to_owned()),
            target: None,
            message: "契约签名与代码签名不一致".to_owned(),
            occurrences: Vec::new(),
        }],
        ..empty_report()
    };
    let filled_value = serde_json::to_value(&filled).expect("序列化失败");
    let entry = &filled_value["apiFindings"][0];
    let keys: Vec<&str> = entry
        .as_object()
        .expect("条目为对象")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec![
            "kind",
            "category",
            "element",
            "target",
            "message",
            "occurrences"
        ],
        "六字段声明序（camelCase）"
    );
    assert_eq!(entry["target"], serde_json::json!(null), "target 可为 null");
    assert_eq!(entry["element"], serde_json::json!("App.Core.Alpha"));

    let back: ArchiDriftReport = serde_json::from_value(filled_value).expect("反序列化失败");
    assert_eq!(back, filled, "往返逐字段相等");
}

/// 边界（AC-6 / 回归登记）：`ArchiCheckStatus::ALL` 长度 5、`as_str` 逐值不变；词汇外
/// 单数形态 `"warning"` → `Err`（零宽松归一）。
#[test]
fn 状态词五值线形不变() {
    let values: Vec<&str> = ArchiCheckStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect();
    assert_eq!(values.len(), 5, "五值");
    assert_eq!(
        values,
        vec!["clean", "errors", "warnings", "noChanges", "skipped"],
        "词序与词形逐字不变（ALL 声明序）"
    );
    for status in ArchiCheckStatus::ALL {
        let back: ArchiCheckStatus =
            serde_json::from_value(serde_json::json!(status.as_str())).expect("往返失败");
        assert_eq!(back, status, "逐值往返");
    }
    assert!(
        serde_json::from_value::<ArchiCheckStatus>(serde_json::json!("warning")).is_err(),
        "词汇外单数形态 → Err（零宽松归一）"
    );
}
