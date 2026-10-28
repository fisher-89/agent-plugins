//! DSL 语料 golden（crate 级集成测试，入口 = `parse_dir`）：逐 case 驱动
//! 解析面 → 图面 → 元素 / 关系清单，与测试内**单点登记**的期望逐项对比。
//!
//! 期望登记面（[`CASES`]）是「case 目录名 → 期望形态」的唯一映射：新增 case 未登记
//! 即红，登记了但不存在的 case 目录亦红。`DESKTOP_GOLDEN_REWRITE=1` 时把实际形态
//! 渲染为 `<case>/render.txt` 快照（供人工核对与评审），断言仍按登记面执行——快照
//! 不参与判定，避免「自证自己」。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use archi::dto::DiagnosticKind;
use archi::graph::ModelGraph;
use archi::parse;

/// 单 case 期望形态：解析面诊断 / 图面诊断 / 元素 FQN / 关系对。
struct Expectation {
    /// case 目录名
    case: &'static str,
    /// 解析面诊断 kind（`parse_dir` 直出，按文件字典序）
    parse_kinds: &'static [&'static str],
    /// 图面诊断 kind（`ModelGraph::diagnostics()` 四类自实现）
    graph_kinds: &'static [&'static str],
    /// 元素 FQN（聚合序 = 文件名字典序 × 文件内 FQN 序）
    elements: &'static [&'static str],
    /// 关系对（`(from, to)`，按声明序）
    relations: &'static [(&'static str, &'static str)],
}

/// 语料登记面（proposal 列出的 13 形态 + 成环 / 孤儿两类结构面）。
const CASES: &[Expectation] = &[
    Expectation {
        case: "comments",
        parse_kinds: &[],
        graph_kinds: &[],
        elements: &["App"],
        relations: &[],
    },
    Expectation {
        case: "multi-file",
        parse_kinds: &[],
        // 语料里 `App.Auth -> App` 是「下游依赖祖先」形态（关系行号与被引语义
        // 两处都有断言价值），图面照实报 dependsOnAncestor
        graph_kinds: &["dependsOnAncestor"],
        elements: &["App", "App.Auth"],
        relations: &[("App.Auth", "App")],
    },
    Expectation {
        case: "array-values",
        parse_kinds: &[],
        graph_kinds: &[],
        elements: &["App", "App.Auth"],
        relations: &[],
    },
    Expectation {
        case: "gate-docs-metadata",
        parse_kinds: &[],
        graph_kinds: &[],
        elements: &["App", "App.Auth"],
        relations: &[],
    },
    Expectation {
        case: "empty-model",
        parse_kinds: &[],
        graph_kinds: &[],
        elements: &[],
        relations: &[],
    },
    Expectation {
        case: "missing-kind",
        parse_kinds: &["missingField"],
        graph_kinds: &[],
        elements: &[],
        relations: &[],
    },
    Expectation {
        case: "invalid-kind",
        parse_kinds: &["invalidValue"],
        graph_kinds: &[],
        elements: &[],
        relations: &[],
    },
    Expectation {
        case: "invalid-gate",
        parse_kinds: &["invalidValue"],
        graph_kinds: &[],
        elements: &[],
        relations: &[],
    },
    Expectation {
        case: "misspelled-key",
        parse_kinds: &["unknownField"],
        graph_kinds: &[],
        elements: &[],
        relations: &[],
    },
    Expectation {
        case: "unclosed-quote",
        parse_kinds: &["syntaxError"],
        graph_kinds: &[],
        elements: &[],
        relations: &[],
    },
    Expectation {
        case: "unclosed-bracket",
        parse_kinds: &["syntaxError"],
        graph_kinds: &[],
        elements: &[],
        relations: &[],
    },
    Expectation {
        case: "duplicate-key",
        parse_kinds: &["duplicateElement"],
        graph_kinds: &[],
        elements: &[],
        relations: &[],
    },
    Expectation {
        case: "dup-cross-file",
        parse_kinds: &["duplicateElement"],
        graph_kinds: &[],
        elements: &["App"],
        relations: &[],
    },
    Expectation {
        case: "orphan",
        parse_kinds: &[],
        graph_kinds: &["orphanHierarchy"],
        elements: &["App.Auth.Token"],
        relations: &[],
    },
    Expectation {
        case: "cycle",
        parse_kinds: &[],
        graph_kinds: &["cycle"],
        elements: &["App", "App.A", "App.B"],
        relations: &[("App.A", "App.B"), ("App.B", "App.A")],
    },
];

/// 语料根目录（`*_test.rs` 不受路径字面量禁令约束）。
fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join("dsl")
}

/// 语料目录内的 case 目录名（字典序）。
fn case_dirs() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(corpus_root())
        .expect("读取语料目录失败")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// 实际形态（解析面 + 图面 + 元素 / 关系清单）。
struct Actual {
    parse_kinds: Vec<String>,
    graph_kinds: Vec<String>,
    elements: Vec<String>,
    relations: Vec<(String, String)>,
}

/// 语料暂存根：把 `<case>/*.toml` 复制到 `<tmp>/.dev-team/architecture/models/`
/// 的仓库形态后驱动（`parse_dir` 的入口是 repo 根，模型目录经
/// `foundation::layout::archi_models_dir` 派生）。crate 无 dev-dependency，故以
/// std 自建 RAII 临时目录。
struct StagedRoot(PathBuf);

impl StagedRoot {
    fn new(case: &str) -> Self {
        // 序号保证并发用例（两个 test 函数同 case 各跑一遍）互不踩踏
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-golden-stage-{}-{seq}-{case}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let models = dir.join(".dev-team").join("architecture").join("models");
        fs::create_dir_all(&models).expect("建暂存模型目录失败");
        let source = corpus_root().join(case);
        for entry in fs::read_dir(&source).expect("读取 case 目录失败").flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().expect("语料文件名");
            fs::copy(&path, models.join(name)).expect("复制语料文件失败");
        }
        StagedRoot(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for StagedRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 驱动单个 case。
fn run_case(case: &str) -> Actual {
    let staged = StagedRoot::new(case);
    let outcome = parse::parse_dir(staged.path());
    let parse_kinds: Vec<String> = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.kind.as_str().to_owned())
        .collect();
    let elements: Vec<String> = outcome
        .model
        .elements
        .iter()
        .map(|element| element.fqn.clone())
        .collect();
    let relations: Vec<(String, String)> = outcome
        .model
        .relations
        .iter()
        .map(|relation| (relation.from.clone(), relation.to.clone()))
        .collect();
    let graph_kinds: Vec<String> = ModelGraph::build(outcome.model)
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.kind.as_str().to_owned())
        .collect();
    Actual {
        parse_kinds,
        graph_kinds,
        elements,
        relations,
    }
}

/// 把实际形态渲染为快照文本（`DESKTOP_GOLDEN_REWRITE=1` 时落盘）。
fn render(case: &str, actual: &Actual) -> String {
    let mut text = format!("# case: {case}\n\n");
    text.push_str("## 解析面诊断\n\n");
    for kind in &actual.parse_kinds {
        text.push_str(&format!("- {kind}\n"));
    }
    text.push_str("\n## 图面诊断\n\n");
    for kind in &actual.graph_kinds {
        text.push_str(&format!("- {kind}\n"));
    }
    text.push_str("\n## 元素\n\n");
    for fqn in &actual.elements {
        text.push_str(&format!("- {fqn}\n"));
    }
    text.push_str("\n## 关系\n\n");
    for (from, to) in &actual.relations {
        text.push_str(&format!("- {from} -> {to}\n"));
    }
    text
}

// ---------------------------------------------------------------------------
// 用例
// ---------------------------------------------------------------------------

/// 正向：逐 case 驱动 `parse_dir` → `ModelGraph::build`，与登记面逐项对比。
#[test]
fn 语料逐case匹配登记面() {
    for expectation in CASES {
        let actual = run_case(expectation.case);
        assert_eq!(
            actual.parse_kinds, expectation.parse_kinds,
            "case {}：解析面诊断不符",
            expectation.case
        );
        assert_eq!(
            actual.graph_kinds, expectation.graph_kinds,
            "case {}：图面诊断不符",
            expectation.case
        );
        assert_eq!(
            actual.elements,
            expectation
                .elements
                .iter()
                .map(|fqn| (*fqn).to_owned())
                .collect::<Vec<String>>(),
            "case {}：元素清单不符",
            expectation.case
        );
        assert_eq!(
            actual.relations,
            expectation
                .relations
                .iter()
                .map(|(from, to)| ((*from).to_owned(), (*to).to_owned()))
                .collect::<Vec<(String, String)>>(),
            "case {}：关系清单不符",
            expectation.case
        );
    }
}

/// 正反向登记：语料目录与登记面互为镜像（新增 case 未登记即红，登记缺失即红）。
#[test]
fn 语料目录与登记面互为镜像() {
    let on_disk: BTreeSet<String> = case_dirs().into_iter().collect();
    let registered: BTreeSet<String> = CASES
        .iter()
        .map(|expectation| expectation.case.to_owned())
        .collect();

    let unregistered: Vec<&String> = on_disk.difference(&registered).collect();
    assert!(
        unregistered.is_empty(),
        "新增 case 未登记：{unregistered:?}（须在 CASES 单点登记）"
    );
    let dangling: Vec<&String> = registered.difference(&on_disk).collect();
    assert!(dangling.is_empty(), "登记了不存在的 case：{dangling:?}");
    assert!(!CASES.is_empty(), "登记面非空");
}

/// 异常：结构性 case 必在——`dup-cross-file` 与 `orphan` 两目录必须存在（explore
/// 低估两类，R-2 的机械锚）。
#[test]
fn 结构性case必在语料内() {
    let on_disk = case_dirs();
    for required in ["dup-cross-file", "orphan"] {
        assert!(
            on_disk.iter().any(|case| case == required),
            "结构性 case {required} 缺失（机械锚不可省）"
        );
    }
}

/// 边界：渲染面确定性与重写开关——同一 case 两次渲染逐字相同；`DESKTOP_GOLDEN_
/// REWRITE=1` 时把快照落到临时目录（供人工核对），判定输入不变（开关不改变断言）。
#[test]
fn 渲染面确定性与重写开关() {
    let rewrite = std::env::var("DESKTOP_GOLDEN_REWRITE").ok().as_deref() == Some("1");
    let snapshot_dir = std::env::temp_dir().join(format!("archi-golden-{}", std::process::id()));
    if rewrite {
        fs::create_dir_all(&snapshot_dir).expect("建快照目录失败");
    }
    for expectation in CASES {
        let first = render(expectation.case, &run_case(expectation.case));
        let second = render(expectation.case, &run_case(expectation.case));
        assert_eq!(
            first, second,
            "case {}：渲染面两次结果应逐字相同（纯确定性）",
            expectation.case
        );
        for kind in expectation
            .parse_kinds
            .iter()
            .chain(expectation.graph_kinds.iter())
        {
            assert!(
                first.contains(kind),
                "case {}：渲染面应含诊断 {kind}",
                expectation.case
            );
        }
        for fqn in expectation.elements {
            assert!(
                first.contains(fqn),
                "case {}：渲染面应含元素 {fqn}",
                expectation.case
            );
        }
        if rewrite {
            fs::write(
                snapshot_dir.join(format!("{}.txt", expectation.case)),
                &first,
            )
            .expect("写快照失败");
        }
    }
    if rewrite {
        let _ = fs::remove_dir_all(&snapshot_dir);
    }
}

/// 边界：语料文件面——每个 case 目录至少一个 `*.toml`，且无残留临时文件。
#[test]
fn 语料文件面收敛() {
    for case in case_dirs() {
        let dir = corpus_root().join(&case);
        let files: Vec<String> = fs::read_dir(&dir)
            .expect("读取 case 目录失败")
            .flatten()
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            files.iter().any(|name| name.ends_with(".toml")),
            "case {case} 至少一个 *.toml 语料"
        );
        assert!(
            !files.iter().any(|name| name.contains("archi-tmp")),
            "case {case} 无残留临时文件: {files:?}"
        );
    }
}

/// 回归：`DiagnosticKind` 线值覆盖登记面（语料覆盖的 kind 均属封闭九值）。
#[test]
fn 登记面kind均属封闭词汇() {
    let known: BTreeSet<&str> = [
        DiagnosticKind::SyntaxError,
        DiagnosticKind::MissingField,
        DiagnosticKind::InvalidValue,
        DiagnosticKind::UnknownField,
        DiagnosticKind::DuplicateElement,
        DiagnosticKind::Cycle,
        DiagnosticKind::DependsOnAncestor,
        DiagnosticKind::UnknownReference,
        DiagnosticKind::OrphanHierarchy,
    ]
    .into_iter()
    .map(DiagnosticKind::as_str)
    .collect();
    for expectation in CASES {
        for kind in expectation
            .parse_kinds
            .iter()
            .chain(expectation.graph_kinds.iter())
        {
            assert!(
                known.contains(*kind),
                "case {} 的期望 kind {kind} 不在封闭词汇内",
                expectation.case
            );
        }
    }
}

// ===========================================================================
// 判定面语料 golden（`desktop-archi-drift`「运行期预算与语料」/ AC-7 / AC-8 /
// AC-13）：`tests/corpus/drift/<case>/` 逐 case 驱动
// `scan → drift::check → report` 链路，与**单点登记面**（[`DRIFT_CASES`]）逐字段
// 对比；快照落 `tests/corpus/drift/expected/<case>.<mode>.json`，
// `DESKTOP_GOLDEN_REWRITE=1` 时显式重写（断言仍按登记面执行，快照不参与判定）。
//
// 语料形态（一 case 一目录）：
//   `models/*.toml`  → 暂存到 `<tmp>/.dev-team/architecture/models/`
//   `config.json`    → 暂存到 `<tmp>/.dev-team/architecture/config.json`（可选）
//   `repo/**`        → 暂存到 `<tmp>/**`（源码观察面）
// ===========================================================================

use archi::drift::{check, ArchiDriftMode, ArchiDriftReport, CheckError, CheckInput};
use foundation::cancel::CancelFlag;

/// 单 case 期望形态（判定面：`status` / 计数 / 元素对级条目 / 留痕面）。
struct DriftExpectation {
    /// case 目录名（`expected/` 之外）
    case: &'static str,
    /// 检查模式
    mode: &'static str,
    /// 增量模式变更集（全量模式恒空）
    changed: &'static [&'static str],
    /// `status` 线词（`clean` / `errors` / `warnings` / `noChanges` / `skipped`）
    status: &'static str,
    /// `scan.changedFiles`
    changed_files: u32,
    /// `scan.scannedFiles`
    scanned_files: u32,
    /// `scan.excluded[]`（file → 命中的具体 glob）
    excluded: &'static [(&'static str, &'static str)],
    /// `scan.unmatchedFiles[]`
    unmatched: &'static [&'static str],
    /// `scan.unresolved[]` 条数
    unresolved: u32,
    /// `model.structuralErrors[]` 条数
    structural_errors: u32,
    /// `errors[]`（from → to，occurrences 条数）
    errors: &'static [(&'static str, &'static str, u32)],
    /// `warnings[]`（from → to）
    warnings: &'static [(&'static str, &'static str)],
    /// `suppressed[]`（from → to → via）
    suppressed: &'static [(&'static str, &'static str, &'static str)],
    /// `observations[]`（from → to）
    observations: &'static [(&'static str, &'static str)],
    /// `errors[]` 中 `typeOnly = true` 的 occurrence 总数
    type_only_occurrences: u32,
}

/// 判定语料登记面（13 个 case 目录 / 15 行驱动；`unfulfilled-declaration` 与
/// `two-modes-consistent` 各以全量 + 增量双行驱动同一目录——① 观察项在增量模式
/// 同样产出、两模式元素对级一致两条断言的事实源）。
const DRIFT_CASES: &[DriftExpectation] = &[
    // 1. 未声明三档：strict → errors / warn → warnings / off → 零记录（+ unmatched）
    DriftExpectation {
        case: "undeclared-three-gates",
        mode: "full",
        changed: &[],
        status: "errors",
        changed_files: 0,
        scanned_files: 4,
        excluded: &[],
        unmatched: &["src/off/a.ts"],
        unresolved: 0,
        structural_errors: 0,
        errors: &[("App.Strict", "App.Shared", 1)],
        warnings: &[("App.Warn", "App.Shared")],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 2. occurrences 按元素对聚合（3 文件各 1 处 → 1 条 + 3 项）
    DriftExpectation {
        case: "occurrences-aggregated",
        mode: "full",
        changed: &[],
        status: "errors",
        changed_files: 0,
        scanned_files: 4,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[("App.Billing", "App.Auth", 3)],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 3. 目标不可归属：丢边 + 来源面留痕（零违例，MUST NOT 冒充 warning）
    DriftExpectation {
        case: "target-unowned",
        mode: "full",
        changed: &[],
        status: "clean",
        changed_files: 0,
        scanned_files: 2,
        excluded: &[],
        unmatched: &["src/shared/token.ts"],
        unresolved: 0,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 4. 测试文件排除留痕（跨元素 import 零违例 + excluded 逐条）
    DriftExpectation {
        case: "test-file-excluded",
        mode: "full",
        changed: &[],
        status: "clean",
        changed_files: 0,
        scanned_files: 2,
        excluded: &[("src/auth/auth.test.ts", "**/*.test.ts")],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 5. import type 同权（只标注 typeOnly，不豁免）
    DriftExpectation {
        case: "import-type",
        mode: "full",
        changed: &[],
        status: "errors",
        changed_files: 0,
        scanned_files: 2,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[("App.Billing", "App.Auth", 1)],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 1,
    },
    // 6. 内联豁免留痕（via = inline，带 file / line / reason）
    DriftExpectation {
        case: "inline-suppressed",
        mode: "full",
        changed: &[],
        status: "clean",
        changed_files: 0,
        scanned_files: 2,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[("App.Billing", "App.Auth", "inline")],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 7. 配置豁免留痕（via = config，file / line 双 null）
    DriftExpectation {
        case: "config-suppressed",
        mode: "full",
        changed: &[],
        status: "clean",
        changed_files: 0,
        scanned_files: 2,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[("App.Billing", "App.Auth", "config")],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 8. 解析失败只留痕（动态 import() 不可静态判定）
    DriftExpectation {
        case: "unresolved-dynamic",
        mode: "full",
        changed: &[],
        status: "clean",
        changed_files: 0,
        scanned_files: 2,
        excluded: &[],
        unmatched: &[],
        unresolved: 1,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 9. 声明未兑现 → observations（全量 + 增量双模式驱动；status 不受影响）
    DriftExpectation {
        case: "unfulfilled-declaration",
        mode: "full",
        changed: &[],
        status: "clean",
        changed_files: 0,
        scanned_files: 2,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[],
        observations: &[("App.Billing", "App.Auth")],
        type_only_occurrences: 0,
    },
    DriftExpectation {
        case: "unfulfilled-declaration",
        mode: "incremental",
        changed: &["src/billing/x.ts"],
        status: "clean",
        changed_files: 1,
        scanned_files: 1,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[],
        observations: &[("App.Billing", "App.Auth")],
        type_only_occurrences: 0,
    },
    // 10. 结构错即不通过（errors[] 空、structuralErrors 非空；元素面停摆 →
    //     源文件无人认领，照实进 unmatchedFiles）
    DriftExpectation {
        case: "structural-error",
        mode: "full",
        changed: &[],
        status: "errors",
        changed_files: 0,
        scanned_files: 1,
        excluded: &[],
        unmatched: &["src/auth/token.ts"],
        unresolved: 0,
        structural_errors: 1,
        errors: &[],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 11. 空模型合法（冷启动 clean，区别于无模型 skipped）
    DriftExpectation {
        case: "empty-model",
        mode: "full",
        changed: &[],
        status: "clean",
        changed_files: 0,
        scanned_files: 1,
        excluded: &[],
        unmatched: &["src/a.ts"],
        unresolved: 0,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 12. 无变更（增量零变更集 → noChanges，MUST NOT 为 clean）
    DriftExpectation {
        case: "no-changes",
        mode: "incremental",
        changed: &[],
        status: "noChanges",
        changed_files: 0,
        scanned_files: 0,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    // 13. 两模式元素对级一致（增量 ⊆ 全量，同一元素对同级别）
    DriftExpectation {
        case: "two-modes-consistent",
        mode: "full",
        changed: &[],
        status: "errors",
        changed_files: 0,
        scanned_files: 2,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[("App.Billing", "App.Auth", 1)],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
    DriftExpectation {
        case: "two-modes-consistent",
        mode: "incremental",
        changed: &["src/billing/x.ts"],
        status: "errors",
        changed_files: 1,
        scanned_files: 1,
        excluded: &[],
        unmatched: &[],
        unresolved: 0,
        structural_errors: 0,
        errors: &[("App.Billing", "App.Auth", 1)],
        warnings: &[],
        suppressed: &[],
        observations: &[],
        type_only_occurrences: 0,
    },
];

/// 判定语料根目录（`tests/corpus/drift`）。
fn drift_corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join("drift")
}

/// 判定语料 case 目录名（字典序，排除 `expected/` 快照目录）。
fn drift_case_dirs() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(drift_corpus_root())
        .expect("读取判定语料目录失败")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "expected")
        .collect();
    names.sort();
    names
}

/// 递归复制目录（std 自建：本 crate 无 dev-dependency）。
fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("建目标目录失败");
    for entry in fs::read_dir(source).expect("读源目录失败").flatten() {
        let from = entry.path();
        let to = target.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to);
        } else {
            fs::copy(&from, &to).expect("复制语料文件失败");
        }
    }
}

/// 语料暂存根（模型 / 配置 / 源码按仓库形态落位；RAII 清理）。
struct DriftStage(PathBuf);

impl DriftStage {
    fn new(case: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-drift-golden-{}-{seq}-{case}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("建暂存根失败");
        let source = drift_corpus_root().join(case);
        // 模型 → <tmp>/.dev-team/architecture/models/
        let models = dir.join(".dev-team").join("architecture").join("models");
        let source_models = source.join("models");
        if source_models.is_dir() {
            copy_tree(&source_models, &models);
        }
        // 架构域配置 → <tmp>/.dev-team/architecture/config.json
        let config = source.join("config.json");
        if config.is_file() {
            fs::copy(
                &config,
                dir.join(".dev-team")
                    .join("architecture")
                    .join("config.json"),
            )
            .expect("复制架构域配置失败");
        }
        // 源码 → <tmp>/**
        let repo = source.join("repo");
        if repo.is_dir() {
            copy_tree(&repo, &dir);
        }
        DriftStage(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    /// 全量字节快照（判定面零写盘对拍）。
    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            let Ok(read) = fs::read_dir(dir) else {
                return;
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, out);
                    continue;
                }
                let relative = path
                    .strip_prefix(base)
                    .expect("相对路径")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, fs::read(&path).expect("读文件失败")));
            }
        }
        let mut entries = Vec::new();
        walk(&self.0, &self.0, &mut entries);
        entries.sort();
        entries
    }
}

impl Drop for DriftStage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 模式解析（登记面线词 → 枚举；词汇外即红）。
fn drift_mode(word: &str) -> ArchiDriftMode {
    match word {
        "full" => ArchiDriftMode::Full,
        "incremental" => ArchiDriftMode::Incremental,
        other => panic!("登记面的 mode 越集：{other}"),
    }
}

/// 驱动单个 case（暂存 → `drift::check`）。
fn run_drift_case(expectation: &DriftExpectation) -> (DriftStage, ArchiDriftReport) {
    let stage = DriftStage::new(expectation.case);
    let changed: Vec<String> = expectation
        .changed
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    let report = check(CheckInput {
        root: stage.root(),
        mode: drift_mode(expectation.mode),
        changed: &changed,
        change: Some(expectation.case),
        cancel: CancelFlag::never(),
    })
    .unwrap_or_else(|error: CheckError| {
        panic!(
            "case {}：判定应成功，实际 Err：{}",
            expectation.case,
            error.message()
        )
    });
    (stage, report)
}

/// 元素对（`errors[]` / `warnings[]` 共用形态）。
fn entry_pairs(entries: &[archi::drift::ArchiDriftEntry]) -> Vec<(String, String)> {
    entries
        .iter()
        .filter_map(|entry| match (&entry.from, &entry.to) {
            (Some(from), Some(to)) => Some((from.clone(), to.clone())),
            _ => None,
        })
        .collect()
}

/// 快照相对路径（`expected/<case>.<mode>.json`）。
fn drift_expected_path(expectation: &DriftExpectation) -> PathBuf {
    drift_corpus_root()
        .join("expected")
        .join(format!("{}.{}.json", expectation.case, expectation.mode))
}

/// 报告快照文本（易变字段归一：`root` / `timestamp` 与绝对路径无关）。
fn drift_snapshot(report: &ArchiDriftReport) -> String {
    let mut value = serde_json::to_value(report).expect("报告应可序列化");
    if let Some(object) = value.as_object_mut() {
        object.insert("root".to_owned(), serde_json::json!("<root>"));
        object.insert("timestamp".to_owned(), serde_json::json!("<timestamp>"));
    }
    let mut text = serde_json::to_string_pretty(&value).expect("快照应可序列化");
    text.push('\n');
    text
}

/// 读取已登记快照（缺失 / 非法 JSON 均以明确文案失败——不静默新建、不以空期望
/// 放过）。
fn read_expected_snapshot(path: &Path) -> Result<serde_json::Value, String> {
    let text = fs::read_to_string(path).map_err(|_| {
        format!(
            "golden {} 缺失（先以 DESKTOP_GOLDEN_REWRITE=1 生成）",
            path.display()
        )
    })?;
    serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|error| format!("golden {} 非法 JSON：{error}", path.display()))
}

/// 正向（AC-7 / AC-8 / AC-13）：逐 case 逐字段对拍登记面——`status` / 两模式计数 /
/// `excluded` / `unmatched` / `unresolved` / 结构错 / 元素对级 `errors` / `warnings`
/// / `suppressed` / `observations` / `typeOnly`。
#[test]
fn 判定语料逐case匹配登记面() {
    for expectation in DRIFT_CASES {
        let (stage, report) = run_drift_case(expectation);
        let label = format!("case {}（{}）", expectation.case, expectation.mode);

        assert_eq!(
            report.status.as_str(),
            expectation.status,
            "{label}：status 不符"
        );
        assert_eq!(
            report.mode,
            drift_mode(expectation.mode),
            "{label}：报告 mode 随输入"
        );
        assert_eq!(
            report.scan.changed_files, expectation.changed_files,
            "{label}：changedFiles"
        );
        assert_eq!(
            report.scan.scanned_files, expectation.scanned_files,
            "{label}：scannedFiles"
        );

        // excluded 逐条（file + 命中规则原文）
        let excluded: Vec<(String, String)> = report
            .scan
            .excluded
            .iter()
            .map(|entry| (entry.file.clone(), entry.rule.clone()))
            .collect();
        assert_eq!(
            excluded,
            expectation
                .excluded
                .iter()
                .map(|(file, rule)| ((*file).to_owned(), (*rule).to_owned()))
                .collect::<Vec<(String, String)>>(),
            "{label}：excluded 留痕"
        );
        assert_eq!(
            report.scan.unmatched_files,
            expectation
                .unmatched
                .iter()
                .map(|file| (*file).to_owned())
                .collect::<Vec<String>>(),
            "{label}：unmatchedFiles"
        );
        assert_eq!(
            report.scan.unresolved.len() as u32,
            expectation.unresolved,
            "{label}：unresolved 条数"
        );
        assert_eq!(
            report.model.structural_errors.len() as u32,
            expectation.structural_errors,
            "{label}：结构错条数"
        );

        // errors：元素对 + occurrences 条数（聚合不铺开）
        let errors: Vec<(String, String, usize)> = report
            .errors
            .iter()
            .filter_map(|entry| match (&entry.from, &entry.to) {
                (Some(from), Some(to)) => Some((from.clone(), to.clone(), entry.occurrences.len())),
                _ => None,
            })
            .collect();
        assert_eq!(
            errors,
            expectation
                .errors
                .iter()
                .map(|(from, to, count)| ((*from).to_owned(), (*to).to_owned(), *count as usize))
                .collect::<Vec<(String, String, usize)>>(),
            "{label}：errors 元素对与聚合条数"
        );
        assert_eq!(
            entry_pairs(&report.warnings),
            expectation
                .warnings
                .iter()
                .map(|(from, to)| ((*from).to_owned(), (*to).to_owned()))
                .collect::<Vec<(String, String)>>(),
            "{label}：warnings 元素对"
        );
        let suppressed: Vec<(String, String, String)> = report
            .suppressed
            .iter()
            .map(|entry| {
                (
                    entry.from.clone(),
                    entry.to.clone(),
                    entry.via.as_str().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            suppressed,
            expectation
                .suppressed
                .iter()
                .map(|(from, to, via)| {
                    ((*from).to_owned(), (*to).to_owned(), (*via).to_owned())
                })
                .collect::<Vec<(String, String, String)>>(),
            "{label}：suppressed 留痕（via）"
        );
        let observations: Vec<(String, String)> = report
            .observations
            .iter()
            .map(|observation| (observation.from.clone(), observation.to.clone()))
            .collect();
        assert_eq!(
            observations,
            expectation
                .observations
                .iter()
                .map(|(from, to)| ((*from).to_owned(), (*to).to_owned()))
                .collect::<Vec<(String, String)>>(),
            "{label}：observations 元素对"
        );
        // typeOnly 同权（只标注，不豁免）
        let type_only: u32 = report
            .errors
            .iter()
            .flat_map(|entry| entry.occurrences.iter())
            .filter(|occurrence| occurrence.type_only)
            .count() as u32;
        assert_eq!(
            type_only, expectation.type_only_occurrences,
            "{label}：typeOnly occurrence 计数"
        );

        // 零写盘：判定纯读盘（同一暂存树字节快照前后相等）
        let before = stage.snapshot();
        let changed_set: Vec<String> = expectation
            .changed
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect();
        check(CheckInput {
            root: stage.root(),
            mode: drift_mode(expectation.mode),
            changed: &changed_set,
            change: Some(expectation.case),
            cancel: CancelFlag::never(),
        })
        .expect("复跑判定应成功");
        assert_eq!(stage.snapshot(), before, "{label}：判定 MUST NOT 写盘");
    }
}

/// 正反向登记：语料目录与登记面互为镜像——新增 case 未登记即红、登记了不存在的
/// case 亦红（`expected/` 为快照目录，不入登记面）。
#[test]
fn 判定语料目录与登记面互为镜像() {
    let on_disk: BTreeSet<String> = drift_case_dirs().into_iter().collect();
    let registered: BTreeSet<String> = DRIFT_CASES
        .iter()
        .map(|expectation| expectation.case.to_owned())
        .collect();

    let unregistered: Vec<&String> = on_disk.difference(&registered).collect();
    assert!(
        unregistered.is_empty(),
        "新增判定 case 未登记：{unregistered:?}（须在 DRIFT_CASES 单点登记）"
    );
    let dangling: Vec<&String> = registered.difference(&on_disk).collect();
    assert!(dangling.is_empty(), "登记了不存在的 case：{dangling:?}");
    assert_eq!(registered.len(), 13, "判定语料 13 个 case（语料维度封闭）");
}

/// 边界（快照对拍 + 显式重写闸）：未置位 `DESKTOP_GOLDEN_REWRITE=1` 时只读比对
/// （字节零变化），置位时把归一化快照写盘且断言仍按登记面执行（快照不参与判定）。
#[test]
fn 判定语料快照对拍与显式重写闸() {
    let rewrite = std::env::var("DESKTOP_GOLDEN_REWRITE").ok().as_deref() == Some("1");
    for expectation in DRIFT_CASES {
        let (_stage, report) = run_drift_case(expectation);
        let actual = drift_snapshot(&report);
        let path = drift_expected_path(expectation);
        let label = format!("case {}（{}）", expectation.case, expectation.mode);

        if rewrite {
            fs::create_dir_all(path.parent().expect("快照目录")).expect("建快照目录失败");
            fs::write(&path, &actual).expect("写快照失败");
            // 重写轮同样按登记面复核（快照不参与判定）
            assert_eq!(
                report.status.as_str(),
                expectation.status,
                "{label}：重写轮仍按登记面断言"
            );
            continue;
        }

        let before = fs::read(&path).ok();
        let expected =
            read_expected_snapshot(&path).unwrap_or_else(|error| panic!("{label}：{error}"));
        let actual_value: serde_json::Value =
            serde_json::from_str(&actual).expect("实际快照应可解析");
        assert_eq!(
            actual_value, expected,
            "{label}：快照不符（差异须以 DESKTOP_GOLDEN_REWRITE=1 重写并 review diff）"
        );
        assert_eq!(
            fs::read(&path).ok(),
            before,
            "{label}：未置位重写开关时快照字节零变化"
        );
    }
}

/// 边界（golden 缺失 / 非法即红）：`read_expected_snapshot` 对缺失与非法 JSON
/// 均以明确文案失败——不静默新建 golden、不以空期望放过。
#[test]
fn golden缺失或非法即以明确文案失败() {
    let missing = std::env::temp_dir().join(format!(
        "archi-drift-golden-missing-{}.json",
        std::process::id()
    ));
    let _ = fs::remove_file(&missing);
    let error = read_expected_snapshot(&missing).expect_err("缺失即 Err");
    assert!(error.contains("缺失"), "缺失文案：{error}");
    assert!(
        error.contains("DESKTOP_GOLDEN_REWRITE=1"),
        "缺失文案须提示重写开关：{error}"
    );

    let broken = std::env::temp_dir().join(format!(
        "archi-drift-golden-broken-{}.json",
        std::process::id()
    ));
    fs::write(&broken, "{ not json").expect("写非法快照失败");
    let error = read_expected_snapshot(&broken).expect_err("非法 JSON 即 Err");
    assert!(error.contains("非法 JSON"), "非法文案：{error}");
    let _ = fs::remove_file(&broken);
}

/// 边界（AC-7 / AC-13）：两模式元素对级一致——同一语料的全量与增量报告里，
/// 增量的 `errors` / `warnings` 元素对集合 ⊆ 全量的同集合（不因模式改级别）。
#[test]
fn 两模式元素对级一致() {
    let full = DRIFT_CASES
        .iter()
        .find(|expectation| {
            expectation.case == "two-modes-consistent" && expectation.mode == "full"
        })
        .expect("全量行在册");
    let incremental = DRIFT_CASES
        .iter()
        .find(|expectation| {
            expectation.case == "two-modes-consistent" && expectation.mode == "incremental"
        })
        .expect("增量行在册");

    let (_full_stage, full_report) = run_drift_case(full);
    let (_incremental_stage, incremental_report) = run_drift_case(incremental);

    let full_errors: BTreeSet<(String, String)> =
        entry_pairs(&full_report.errors).into_iter().collect();
    let incremental_errors: BTreeSet<(String, String)> = entry_pairs(&incremental_report.errors)
        .into_iter()
        .collect();
    assert!(
        incremental_errors.is_subset(&full_errors),
        "增量 errors 元素对应为全量子集：{incremental_errors:?} ⊄ {full_errors:?}"
    );
    assert!(
        !incremental_errors.is_empty(),
        "该语料两模式都命中同一元素对（级别一致）"
    );
    let full_warnings: BTreeSet<(String, String)> =
        entry_pairs(&full_report.warnings).into_iter().collect();
    let incremental_warnings: BTreeSet<(String, String)> =
        entry_pairs(&incremental_report.warnings)
            .into_iter()
            .collect();
    assert!(
        incremental_warnings.is_subset(&full_warnings),
        "增量 warnings 元素对应为全量子集"
    );
    assert_eq!(
        incremental_report.base.as_deref(),
        Some("HEAD"),
        "增量基准恒 HEAD"
    );
    assert!(full_report.base.is_none(), "全量模式无基准（base = null）");
    // status 同级别（errors 对 errors）
    assert_eq!(
        incremental_report.status.as_str(),
        full_report.status.as_str(),
        "同一元素对不因模式不同而级别不同"
    );
}
