//! `artifacts::markdown_doc` 的单元测试（test-design「artifacts/markdown_doc.rs
//! -> markdown_doc_test.rs」节）：清单类型判定——候选恒来自 phase 定义清单枚举，
//! kind 由清单声明（`spec.kind == "markdown-doc"`），「扩展名 == md」旧判据退役；
//! 代际名单 title / 排序逻辑保留（AC-7）。
//!
//! Mock策略（test-design 本节 Mock 表）：磁盘目录树（进程边界）= tempdir 真实目录
//!（.md 文件 + 清单命中的 markdown-doc 候选）；无（真实组合）= 插件 matcher 真实
//! 组合，`manifest` 由 registry 入参显式供给。

use std::fs;
use std::path::{Path, PathBuf};

use super::envelope::ArtifactCandidate;
use super::markdown_doc::{MarkdownDocPlugin, KIND};
use super::registry::{discover_artifacts, ArtifactInput, ArtifactPlugin};
use crate::state::ArtifactSpec;
use crate::write::{phase_table, PhaseDefinition};

/// 临时 change 目录 RAII：测试结束自动清理。
struct TempChange(PathBuf);

impl TempChange {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-mddoc-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn write(&self, rel_path: &str, content: &str) {
        let path = self.0.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建子目录失败");
        }
        fs::write(path, content).expect("写文件失败");
    }
}

impl Drop for TempChange {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// --- 清单声明 fixture（编译期静态项；`ArtifactSpec` 字段为 `&'static str`）----

/// 清单命中的 markdown-doc 声明（proposal.md）。
static PROPOSAL_DOC: ArtifactSpec = ArtifactSpec {
    kind: KIND,
    path: "proposal.md",
    title: "清单兜底标题",
};

/// 清单未命中的台账声明（kind ≠ markdown-doc——跨插件不串扰）。
static TASKS_SPEC: ArtifactSpec = ArtifactSpec {
    kind: "tasks-progress",
    path: "tasks.md",
    title: "任务进度",
};

/// legacy 名单候选声明（test-design.md 属 markdown-doc 清单）。
static LEGACY_DESIGN_DOC: ArtifactSpec = ArtifactSpec {
    kind: KIND,
    path: "test-design.md",
    title: "清单兜底标题",
};

/// 自定义清单：仅声明 proposal.md 一项（其余 .md 全不在清单内）。
static PROPOSAL_ONLY_MANIFEST: &[PhaseDefinition] = &[PhaseDefinition {
    id: "proposal",
    description: "清单 fixture",
    steps: &[],
    artifact_manifest: &[PROPOSAL_DOC],
    prerequisites: &[],
    backtrackable: true,
    executor: None,
    evaluator: None,
}];

/// 自定义清单：proposal.md + test-design.md（名单排序断言用）。
static TWO_DOC_MANIFEST: &[PhaseDefinition] = &[PhaseDefinition {
    id: "proposal",
    description: "清单 fixture",
    steps: &[],
    artifact_manifest: &[PROPOSAL_DOC, LEGACY_DESIGN_DOC],
    prerequisites: &[],
    backtrackable: true,
    executor: None,
    evaluator: None,
}];

/// requirement 真件清单（phase_table 只读供给）。
fn requirement_manifest() -> &'static [PhaseDefinition] {
    phase_table("requirement").expect("requirement 表恒可达")
}

/// 以清单上下文直接调用插件 matcher（`spec` 显式供给）。
fn matches_file(change_dir: &Path, rel_path: &str, spec: &'static ArtifactSpec) -> bool {
    let candidate = ArtifactCandidate::File {
        relative_path: PathBuf::from(rel_path),
    };
    let input = ArtifactInput {
        change_dir,
        phases: &[],
        candidate: &candidate,
        spec: Some(spec),
    };
    MarkdownDocPlugin.matches(&input)
}

/// 以清单上下文直接调用插件 parse。
fn parse_file(
    change_dir: &Path,
    rel_path: &str,
    spec: &'static ArtifactSpec,
) -> Option<super::ArtifactEnvelope> {
    let candidate = ArtifactCandidate::File {
        relative_path: PathBuf::from(rel_path),
    };
    let input = ArtifactInput {
        change_dir,
        phases: &[],
        candidate: &candidate,
        spec: Some(spec),
    };
    MarkdownDocPlugin.parse(&input)
}

/// IPC 直查面（清单缺席，`spec = None`）：parse 只读文件本体，title 回落名单 /
/// 文件名主干。
fn parse_without_spec(change_dir: &Path, rel_path: &str) -> Option<super::ArtifactEnvelope> {
    let candidate = ArtifactCandidate::File {
        relative_path: PathBuf::from(rel_path),
    };
    let input = ArtifactInput {
        change_dir,
        phases: &[],
        candidate: &candidate,
        spec: None,
    };
    MarkdownDocPlugin.parse(&input)
}

// ---------------------------------------------------------------------------
// 正向：清单命中的 markdown-doc 候选收录 + parse
// ---------------------------------------------------------------------------

/// 清单命中的 markdown-doc 候选 → 插件收录并 parse（payload / fallback 同为文件
/// 原文；清单声明标题可兜底）。
#[test]
fn 清单命中的markdown候选收录并parse() {
    let change = TempChange::new("declared-hit");
    change.write("proposal.md", "# 提案标题\n\n正文内容。");

    assert!(matches_file(&change.0, "proposal.md", &PROPOSAL_DOC));
    let envelope = parse_file(&change.0, "proposal.md", &PROPOSAL_DOC)
        .expect("清单命中的 proposal.md 应产出信封");
    assert_eq!(envelope.kind, KIND);
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.title, "提案", "现役名单 title 标注优先于清单兜底");
    assert_eq!(envelope.payload["markdown"], "# 提案标题\n\n正文内容。");
    assert_eq!(
        envelope.fallback_text.as_deref(),
        Some("# 提案标题\n\n正文内容。"),
        "文件原文即保底"
    );
}

/// requirement 真件清单驱动的发现：proposal.md / design.md / specs/** 命中，
/// `ArtifactDescriptor.phase` 归属随清单声明（proposal / dev-design）。
#[test]
fn requirement清单驱动发现_phase归属随清单() {
    let change = TempChange::new("requirement-manifest");
    change.write("proposal.md", "# 提案");
    change.write("design.md", "# 设计");
    change.write("specs/alpha/spec.md", "# 规格 alpha");

    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());

    let pairs: Vec<(&str, &str)> = descriptors
        .iter()
        .filter(|d| d.kind == KIND)
        .map(|d| (d.source.as_str(), d.phase.as_str()))
        .collect();
    assert!(
        pairs.contains(&("proposal.md", "proposal")),
        "proposal.md 归属 proposal：{pairs:?}"
    );
    assert!(
        pairs.contains(&("design.md", "dev-design")),
        "design.md 归属 dev-design：{pairs:?}"
    );
    assert!(
        pairs.contains(&("specs/alpha/spec.md", "proposal")),
        "specs/** 前缀 glob 命中归属 proposal：{pairs:?}"
    );
    assert_eq!(
        descriptors
            .iter()
            .find(|d| d.source == "specs/alpha/spec.md")
            .map(|d| d.title.as_str()),
        Some("规格 · alpha"),
        "规格标题按 capability 标注"
    );

    // 枚举确定性：底序按相对 POSIX 路径升序，组内再按插件 order（代际名单）
    // 稳定排序——proposal（现役首位）先于 design，规格（capability 规则）最后
    let sources: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.kind == KIND)
        .map(|d| d.source.as_str())
        .collect();
    assert_eq!(
        sources,
        vec!["proposal.md", "design.md", "specs/alpha/spec.md"],
        "插件 order 排序（现役名单 → 规格规则路径）"
    );
}

/// 清单前缀 glob 深路径命中（`specs/**` 多级目录递归枚举）。
#[test]
fn specs前缀glob深路径命中() {
    let change = TempChange::new("specs-deep");
    change.write("specs/x/y/spec.md", "# 深层规格");
    change.write("specs/level1/level2/level3/spec.md", "# 更深规格");

    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    let sources: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.kind == KIND)
        .map(|d| d.source.as_str())
        .collect();
    assert!(
        sources.contains(&"specs/x/y/spec.md"),
        "多级深路径命中：{sources:?}"
    );
    assert!(sources.contains(&"specs/level1/level2/level3/spec.md"));
}

// ---------------------------------------------------------------------------
// 边界 / 异常：旧判据退役、跨插件不串扰
// ---------------------------------------------------------------------------

/// 清单未命中但扩展名 == .md 的任意文件（explore.md）→ 不再入列（「扩展名 ==
/// md」旧判据退役，AC-7）。
#[test]
fn 清单未命中的md文件不再入列() {
    let change = TempChange::new("undeclared-md");
    change.write("explore.md", "# 探索");
    change.write("reports/inner.md", "# 子目录内层");
    change.write("note.md", "# 随手记");

    // matcher 面：清单未命中（spec 为 None）→ 恒 false
    let candidate = ArtifactCandidate::File {
        relative_path: PathBuf::from("explore.md"),
    };
    let input = ArtifactInput {
        change_dir: &change.0,
        phases: &[],
        candidate: &candidate,
        spec: None,
    };
    assert!(
        !MarkdownDocPlugin.matches(&input),
        "清单缺席的候选不命中（旧扩展名判据退役）"
    );

    // 发现面：真件清单不含这些文件 → 零候选
    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    assert!(
        descriptors.is_empty(),
        "清单未命中的 .md 零入列：{descriptors:?}"
    );

    // 扩展名判据确已退役：同目录 .md 在清单内才入列（对照面）
    change.write("proposal.md", "# 提案");
    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    let sources: Vec<&str> = descriptors.iter().map(|d| d.source.as_str()).collect();
    assert_eq!(sources, vec!["proposal.md"], "仅清单命中的 md 入列");
}

/// 清单命中的非 markdown-doc kind（tasks.md 属 tasks-progress 清单）→
/// markdown-doc 不收（kind 由清单绑定，跨插件不串扰）。
#[test]
fn 清单命中的其它kind候选markdown不收() {
    let change = TempChange::new("cross-plugin");
    change.write("tasks.md", "- [x] 完成\n");

    assert!(
        !matches_file(&change.0, "tasks.md", &TASKS_SPEC),
        "kind = tasks-progress 的清单声明不被 markdown-doc 收录"
    );
    // 同一文件命中 markdown-doc 清单声明时才收录（kind 由清单绑定）
    let descriptors = discover_artifacts(&change.0, &[], PROPOSAL_ONLY_MANIFEST);
    assert!(
        descriptors.is_empty(),
        "tasks.md 不在这份清单里：{descriptors:?}"
    );
}

/// eval 候选（PhaseRecord 投影分支）永不命中 markdown-doc。
#[test]
fn eval候选不命中markdown() {
    let change = TempChange::new("eval-candidate");
    let candidate = ArtifactCandidate::EvalEntry { index: 0 };
    let input = ArtifactInput {
        change_dir: &change.0,
        phases: &[],
        candidate: &candidate,
        spec: Some(&PROPOSAL_DOC),
    };
    assert!(!MarkdownDocPlugin.matches(&input));
    assert!(MarkdownDocPlugin.parse(&input).is_none());
}

/// 清单命中但文件缺失 → parse 返回 None（best-effort，不 panic）。
#[test]
fn 清单命中但文件缺失parse为空() {
    let change = TempChange::new("missing-file");
    assert!(parse_file(&change.0, "proposal.md", &PROPOSAL_DOC).is_none());
}

// ---------------------------------------------------------------------------
// 代际名单 title / rank 逻辑保留
// ---------------------------------------------------------------------------

/// legacy 名单命中时 title 标注与排序生效（名单逻辑保留——非清单项不参与）。
#[test]
fn legacy名单命中时title标注与排序生效() {
    let change = TempChange::new("legacy-names");
    change.write("test-design.md", "# 旧测试设计");

    assert_eq!(
        parse_file(&change.0, "test-design.md", &LEGACY_DESIGN_DOC)
            .unwrap()
            .title,
        "测试设计",
        "legacy 名单 title 标注保留"
    );
    assert_eq!(
        parse_without_spec(&change.0, "test-design.md")
            .unwrap()
            .title,
        "测试设计",
        "IPC 直查面（spec = None）名单逻辑同样生效"
    );

    // 排序：现役名单（proposal）先于 legacy 名单（test-design）
    change.write("proposal.md", "# 现役提案");
    let descriptors = discover_artifacts(&change.0, &[], TWO_DOC_MANIFEST);
    let doc_titles: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.kind == KIND)
        .map(|d| d.title.as_str())
        .collect();
    assert_eq!(
        doc_titles,
        vec!["提案", "测试设计"],
        "现役名单先于 legacy 名单（kind 内 order 排序）"
    );
}

/// 清单声明标题作为名单未命中时的兜底（清单标题为空则回落文件名主干）。
#[test]
fn 清单标题兜底与文件名主干回落() {
    let change = TempChange::new("declared-title");
    change.write("custom-doc.md", "# 自定义");

    static CUSTOM_DOC: ArtifactSpec = ArtifactSpec {
        kind: KIND,
        path: "custom-doc.md",
        title: "清单声明标题",
    };
    static BLANK_TITLE_DOC: ArtifactSpec = ArtifactSpec {
        kind: KIND,
        path: "another-doc.md",
        title: "",
    };

    assert_eq!(
        parse_file(&change.0, "custom-doc.md", &CUSTOM_DOC)
            .unwrap()
            .title,
        "清单声明标题"
    );
    change.write("another-doc.md", "# 无名");
    assert_eq!(
        parse_file(&change.0, "another-doc.md", &BLANK_TITLE_DOC)
            .unwrap()
            .title,
        "another-doc",
        "清单标题为空 → 文件主干"
    );
    assert_eq!(
        parse_without_spec(&change.0, "custom-doc.md")
            .unwrap()
            .title,
        "custom-doc",
        "清单缺席 → 文件主干"
    );
}

// ---------------------------------------------------------------------------
// 边界：内容面保真（空串 / 超长 / 特殊字节）
// ---------------------------------------------------------------------------

/// 空 .md 文件命中且 markdown 为空串（清单命中在位）。
#[test]
fn 空md文件命中且markdown为空串() {
    let change = TempChange::new("empty-md");
    change.write("empty.md", "");

    static EMPTY_DOC: ArtifactSpec = ArtifactSpec {
        kind: KIND,
        path: "empty.md",
        title: "",
    };
    assert!(matches_file(&change.0, "empty.md", &EMPTY_DOC));
    let envelope = parse_file(&change.0, "empty.md", &EMPTY_DOC).expect("空 .md 仍命中");
    assert_eq!(envelope.payload["markdown"], "");
}

/// 超长 markdown 完整保留不截断（> 1000 字符；含换行与特殊字符）。
#[test]
fn 超长markdown完整保留不截断() {
    let change = TempChange::new("long-md");
    let mut content = String::new();
    for i in 0..1200 {
        content.push_str(&format!("- 第 {i} 行：超长文档内容行，验证不截断。\n"));
    }
    change.write("long.md", &content);

    static LONG_DOC: ArtifactSpec = ArtifactSpec {
        kind: KIND,
        path: "long.md",
        title: "",
    };
    let envelope = parse_file(&change.0, "long.md", &LONG_DOC).expect("超长 .md 应产出信封");
    assert_eq!(
        envelope.payload["markdown"].as_str().map(str::len),
        Some(content.len())
    );
    assert_eq!(envelope.fallback_text.as_deref(), Some(content.as_str()));
}

/// 特殊字符（emoji / \0 / \n）内容保真出线。
#[test]
fn 特殊字符内容保真() {
    let change = TempChange::new("special-bytes");
    let content = "# 章标题 🚀\n\n含制表\t与零宽\u{200b}字符，以及空字符后段\u{0}尾。\n";
    change.write("special.md", content);

    static SPECIAL_DOC: ArtifactSpec = ArtifactSpec {
        kind: KIND,
        path: "special.md",
        title: "",
    };
    let envelope = parse_file(&change.0, "special.md", &SPECIAL_DOC).expect("应产出信封");
    assert_eq!(envelope.payload["markdown"].as_str(), Some(content));
    assert_eq!(envelope.fallback_text.as_deref(), Some(content));
}

/// 文件名含中文 / 空格 / 括号时 source 为相对 POSIX 路径（清单命中）。
#[test]
fn 文件名特殊字符时source为相对posix路径() {
    let change = TempChange::new("special-names");
    change.write("会议纪要 备忘.md", "# 中文与空格");
    change.write("带(括号)&符号.md", "# 特殊字符");

    static CJK_DOC: ArtifactSpec = ArtifactSpec {
        kind: KIND,
        path: "会议纪要 备忘.md",
        title: "",
    };
    static SYMBOL_MANIFEST: &[PhaseDefinition] = &[PhaseDefinition {
        id: "proposal",
        description: "清单 fixture",
        steps: &[],
        artifact_manifest: &[
            ArtifactSpec {
                kind: KIND,
                path: "会议纪要 备忘.md",
                title: "",
            },
            ArtifactSpec {
                kind: KIND,
                path: "带(括号)&符号.md",
                title: "",
            },
        ],
        prerequisites: &[],
        backtrackable: true,
        executor: None,
        evaluator: None,
    }];

    let descriptors = discover_artifacts(&change.0, &[], SYMBOL_MANIFEST);
    let sources: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.kind == KIND)
        .map(|d| d.source.as_str())
        .collect();
    assert!(
        sources.contains(&"会议纪要 备忘.md"),
        "实际 sources: {sources:?}"
    );
    assert!(sources.contains(&"带(括号)&符号.md"));
    assert!(sources.iter().all(|s| !s.contains('\\')), "POSIX 相对路径");

    let envelope = parse_file(&change.0, "会议纪要 备忘.md", &CJK_DOC).expect("中文文件名应可读取");
    assert_eq!(envelope.payload["markdown"], "# 中文与空格");
}
