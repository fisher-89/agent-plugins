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

/// 语料暂存根：把 `<case>/*.toml` 复制到 `<tmp>/openspec/architecture/models/`
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
        let models = dir.join("openspec").join("architecture").join("models");
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
