//! `eval-checklist` 插件：逐条含非空 checklist 的相位评估条目产出信封。

use serde_json::json;

use super::envelope::{ArtifactCandidate, ArtifactEnvelope};
use super::registry::{ArtifactInput, ArtifactPlugin};
use crate::state::PhaseStateRecord;

pub const KIND: &str = "eval-checklist";
const VERSION: u32 = 1;

pub struct EvalChecklistPlugin;

/// 评估条目 checklist → 保底文本清单。
fn checklist_fallback(entry: &PhaseStateRecord) -> String {
    let lines: Vec<String> = entry
        .checklist
        .iter()
        .map(|item| {
            format!(
                "- [{}] {} — {}",
                if item.pass { "x" } else { " " },
                item.item,
                item.evidence
            )
        })
        .collect();
    lines.join("\n")
}

fn entry_title(entry: &PhaseStateRecord) -> String {
    format!("评估清单 · {} · 第 {} 次", entry.phase, entry.attempt)
}

impl ArtifactPlugin for EvalChecklistPlugin {
    fn kind(&self) -> &'static str {
        KIND
    }

    /// 排序键 = 评估条目下标：多个 checklist 信封按历史顺序排列。
    fn order(&self, input: &ArtifactInput) -> u32 {
        match input.candidate {
            ArtifactCandidate::EvalEntry { index } => *index as u32,
            ArtifactCandidate::File { .. } => 0,
        }
    }

    fn matches(&self, input: &ArtifactInput) -> bool {
        match input.candidate {
            ArtifactCandidate::EvalEntry { index } => input
                .phases
                .get(*index)
                .is_some_and(|entry| !entry.checklist.is_empty()),
            ArtifactCandidate::File { .. } => false,
        }
    }

    fn parse(&self, input: &ArtifactInput) -> Option<ArtifactEnvelope> {
        let index = match input.candidate {
            ArtifactCandidate::EvalEntry { index } => *index,
            ArtifactCandidate::File { .. } => return None,
        };
        let entry = input.phases.get(index)?;
        if entry.checklist.is_empty() {
            return None;
        }
        let fallback_text = checklist_fallback(entry);
        let title = entry_title(entry);
        Some(ArtifactEnvelope {
            kind: KIND.to_string(),
            version: VERSION,
            title,
            payload: json!({
                "phase": entry.phase,
                "attempt": entry.attempt,
                "verdict": entry.verdict.as_str(),
                "items": entry.checklist,
            }),
            fallback_text: Some(fallback_text),
        })
    }
}
