//! `markdown-doc` 插件：收录**清单命中的** markdown 产物候选。
//!
//! 候选来源 = phase 定义清单（`ArtifactSpec.kind == "markdown-doc"` 的相对路径
//! 枚举命中项）——「扩展名 == md」旧判据退役（清单之外的文件零候选零信封）。现役
//! 文件名名单优先做 title 标注与排序，legacy 名单其次，其余文件以清单声明标题
//!（无则文件名主干）兜底——Docs 探测即本插件，注册表之外不存在并行探测路径。

use std::fs;
use std::path::Path;

use serde_json::json;

use super::envelope::{ArtifactCandidate, ArtifactEnvelope};
use super::registry::{ArtifactInput, ArtifactPlugin};

pub const KIND: &str = "markdown-doc";
const VERSION: u32 = 1;

pub struct MarkdownDocPlugin;

/// 现役文件名名单 → title 标注。
const CURRENT_DOCS: &[(&str, &str)] = &[
    ("proposal.md", "提案"),
    ("design.md", "设计"),
    ("tasks.md", "任务"),
    ("explore.md", "探索"),
];

/// legacy 文件名名单 → title 标注（旧代际顶层快照等）。
const LEGACY_DOCS: &[(&str, &str)] = &[
    ("test-design.md", "测试设计"),
    ("gan-design.md", "设计稿"),
    ("README.md", "README"),
];

/// 把相对路径规范化为 POSIX 风格字符串。
fn posix_path(relative_path: &Path) -> String {
    relative_path.to_string_lossy().replace('\\', "/")
}

/// 清单类型判定：候选恒来自清单枚举，kind 由清单声明（非扩展名猜测）。
fn is_declared_doc(input: &ArtifactInput) -> bool {
    input.spec.is_some_and(|spec| spec.kind == KIND)
}

/// `specs/<capability>/spec.md` → capability 名。
fn spec_capability(posix: &str) -> Option<&str> {
    posix
        .strip_prefix("specs/")
        .and_then(|rest| rest.strip_suffix("/spec.md"))
}

/// 代际名单决定 title 标注（`declared` = 清单声明的标题，名单未命中时兜底）。
fn doc_title(posix: &str, declared: Option<&str>) -> String {
    if let Some(capability) = spec_capability(posix) {
        return format!("规格 · {capability}");
    }
    for (name, title) in CURRENT_DOCS {
        if posix == *name {
            return (*title).to_string();
        }
    }
    for (name, title) in LEGACY_DOCS {
        if posix == *name {
            return (*title).to_string();
        }
    }
    if let Some(rest) = posix.strip_prefix("phases/") {
        let stem = rest.trim_end_matches(".md");
        return format!("阶段快照 · {stem}");
    }
    if let Some(rest) = posix.strip_prefix("test-reports/") {
        let stem = rest.trim_end_matches(".md");
        return format!("测试报告 · {stem}");
    }
    if let Some(declared) = declared.filter(|title| !title.is_empty()) {
        return declared.to_string();
    }
    // 其余文件：取文件名主干
    posix
        .rsplit('/')
        .next()
        .unwrap_or(posix)
        .trim_end_matches(".md")
        .to_string()
}

/// 代际名单决定排序：现役名单优先、legacy 名单其次、其余最后；
/// 同排序键内保持候选枚举（路径）顺序。
fn doc_rank(posix: &str) -> u32 {
    if spec_capability(posix).is_some() {
        return 4;
    }
    for (index, (name, _)) in CURRENT_DOCS.iter().enumerate() {
        if posix == *name {
            return index as u32;
        }
    }
    for (index, (name, _)) in LEGACY_DOCS.iter().enumerate() {
        if posix == *name {
            return 10 + index as u32;
        }
    }
    if posix.starts_with("phases/") {
        return 11;
    }
    if posix.starts_with("test-reports/") {
        return 12;
    }
    100
}

impl ArtifactPlugin for MarkdownDocPlugin {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn order(&self, input: &ArtifactInput) -> u32 {
        match input.candidate {
            ArtifactCandidate::File { relative_path } => doc_rank(&posix_path(relative_path)),
            ArtifactCandidate::EvalEntry { .. } => 0,
        }
    }

    fn matches(&self, input: &ArtifactInput) -> bool {
        matches!(input.candidate, ArtifactCandidate::File { .. }) && is_declared_doc(input)
    }

    fn parse(&self, input: &ArtifactInput) -> Option<ArtifactEnvelope> {
        // 文件候选非清单上下文（IPC 直查面）：按 source 重建的候选亦可解析（信封
        // 读取不依赖清单，只有 matcher 依赖）
        let relative_path = match input.candidate {
            ArtifactCandidate::File { relative_path } => relative_path,
            ArtifactCandidate::EvalEntry { .. } => return None,
        };
        let markdown_text = fs::read_to_string(input.change_dir.join(relative_path)).ok()?;
        let title = doc_title(
            &posix_path(relative_path),
            input.spec.map(|spec| spec.title),
        );
        Some(ArtifactEnvelope {
            kind: KIND.to_string(),
            version: VERSION,
            title,
            payload: json!({ "markdown": markdown_text.as_str() }),
            // 文件原文即保底
            fallback_text: Some(markdown_text),
        })
    }
}
