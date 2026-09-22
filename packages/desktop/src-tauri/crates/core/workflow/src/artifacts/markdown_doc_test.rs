//! `artifacts::markdown_doc` 的单元测试：目录树 .md 收录、代际名单 title/排序（AC-8）。

use std::fs;
use std::path::{Path, PathBuf};

use super::markdown_doc::{MarkdownDocPlugin, KIND};
use super::registry::{discover_artifacts, ArtifactInput, ArtifactPlugin};
use super::ArtifactCandidate;
use crate::model::Inventory;

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

/// 以单个文件候选直接调用插件。
fn parse_file(change_dir: &Path, rel_path: &str) -> Option<super::ArtifactEnvelope> {
    let candidate = ArtifactCandidate::File {
        relative_path: PathBuf::from(rel_path),
    };
    let input = ArtifactInput {
        change_dir,
        inventory: Inventory::V0,
        workflow: None,
        candidate: &candidate,
    };
    MarkdownDocPlugin.parse(&input)
}

fn matches_file(change_dir: &Path, rel_path: &str) -> bool {
    let candidate = ArtifactCandidate::File {
        relative_path: PathBuf::from(rel_path),
    };
    let input = ArtifactInput {
        change_dir,
        inventory: Inventory::V0,
        workflow: None,
        candidate: &candidate,
    };
    MarkdownDocPlugin.matches(&input)
}

#[test]
fn 目录树内md命中_payload与fallback同为文件原文() {
    let change = TempChange::new("proposal-hit");
    change.write("proposal.md", "# 提案标题\n\n正文内容。");

    assert!(matches_file(&change.0, "proposal.md"));
    let envelope = parse_file(&change.0, "proposal.md").expect("proposal.md 应产出信封");
    assert_eq!(envelope.kind, KIND);
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.title, "提案", "现役名单 title 标注");
    assert_eq!(envelope.payload["markdown"], "# 提案标题\n\n正文内容。");
    assert_eq!(
        envelope.fallback_text.as_deref(),
        Some("# 提案标题\n\n正文内容。"),
        "文件原文即保底"
    );
}

#[test]
fn legacy名单命中时title标注与排序生效() {
    let change = TempChange::new("legacy-names");
    change.write("gan-design.md", "# 旧设计稿");
    change.write("test-reports/unit-run.md", "# 旧测试报告");
    change.write("test-design.md", "# 旧测试设计");
    change.write("README.md", "# 说明");

    assert_eq!(
        parse_file(&change.0, "gan-design.md").unwrap().title,
        "设计稿"
    );
    assert_eq!(
        parse_file(&change.0, "test-reports/unit-run.md")
            .unwrap()
            .title,
        "测试报告 · unit-run"
    );
    assert_eq!(
        parse_file(&change.0, "test-design.md").unwrap().title,
        "测试设计"
    );
    assert_eq!(parse_file(&change.0, "README.md").unwrap().title, "README");

    // 排序：legacy 名单整体位于现役名单之后（经 discover 的 kind 内 order 断言）
    change.write("proposal.md", "# 现役提案");
    let descriptors = discover_artifacts(&change.0, Inventory::V0, None);
    let doc_titles: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.kind == KIND)
        .map(|d| d.title.as_str())
        .collect();
    let proposal_pos = doc_titles.iter().position(|t| *t == "提案").unwrap();
    let legacy_pos = doc_titles.iter().position(|t| *t == "设计稿").unwrap();
    assert!(
        proposal_pos < legacy_pos,
        "现役名单先于 legacy 名单：{doc_titles:?}"
    );

    // specs/ 目录的规格标注（现役名单之外的规则路径仍在本插件内，无并行探测）
    change.write("specs/glob-matching/spec.md", "# 规格");
    let descriptors = discover_artifacts(&change.0, Inventory::V0, None);
    assert!(descriptors
        .iter()
        .any(|d| d.title == "规格 · glob-matching"));
}

#[test]
fn 非md文件不命中matches() {
    let change = TempChange::new("non-md");
    change.write("data.json", "{}");
    change.write("code.ts", "const x = 1;");
    change.write("README.TXT", "大写扩展名");

    assert!(!matches_file(&change.0, "data.json"));
    assert!(!matches_file(&change.0, "code.ts"));
    assert!(!matches_file(&change.0, "README.TXT"));
    // eval 候选永不命中 markdown-doc
    let candidate = ArtifactCandidate::EvalEntry { index: 0 };
    let input = ArtifactInput {
        change_dir: &change.0,
        inventory: Inventory::V2,
        workflow: None,
        candidate: &candidate,
    };
    assert!(!MarkdownDocPlugin.matches(&input));
}

#[test]
fn 空md文件命中且markdown为空串() {
    let change = TempChange::new("empty-md");
    change.write("empty.md", "");

    assert!(matches_file(&change.0, "empty.md"));
    let envelope = parse_file(&change.0, "empty.md").expect("空 .md 仍命中");
    assert_eq!(envelope.payload["markdown"], "");
}

#[test]
fn 超长markdown完整保留不截断() {
    let change = TempChange::new("long-md");
    let mut content = String::new();
    for i in 0..1200 {
        content.push_str(&format!("- 第 {i} 行：超长文档内容行，验证不截断。\n"));
    }
    change.write("long.md", &content);

    let envelope = parse_file(&change.0, "long.md").expect("超长 .md 应产出信封");
    assert_eq!(
        envelope.payload["markdown"].as_str().map(str::len),
        Some(content.len())
    );
    assert_eq!(envelope.fallback_text.as_deref(), Some(content.as_str()));
}

#[test]
fn 子目录内md一并收录_目录树全量() {
    let change = TempChange::new("nested-md");
    change.write("proposal.md", "# 顶层");
    change.write("reports/inner.md", "# 子目录内层");
    change.write("reports/deep/deeper.md", "# 更深层");

    let descriptors = discover_artifacts(&change.0, Inventory::V0, None);
    let sources: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.kind == KIND)
        .map(|d| d.source.as_str())
        .collect();
    assert!(sources.contains(&"proposal.md"));
    assert!(sources.contains(&"reports/inner.md"), "子目录 .md 一并收录");
    assert!(
        sources.contains(&"reports/deep/deeper.md"),
        "更深子目录同样收录"
    );
}

#[test]
fn 文件名含中文空格特殊字符时source为相对posix路径() {
    let change = TempChange::new("special-names");
    change.write("会议纪要 备忘.md", "# 中文与空格");
    change.write("带(括号)&符号.md", "# 特殊字符");

    let descriptors = discover_artifacts(&change.0, Inventory::V0, None);
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
    // source 不含反斜杠（POSIX 相对路径）
    assert!(sources.iter().all(|s| !s.contains('\\')));

    // 按 source 可回放读取
    let envelope = parse_file(&change.0, "会议纪要 备忘.md").expect("中文文件名应可读取");
    assert_eq!(envelope.payload["markdown"], "# 中文与空格");
}
