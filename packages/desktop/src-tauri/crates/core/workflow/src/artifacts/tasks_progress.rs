//! `tasks-progress` 插件：tasks.md 勾选计数（total / done / pending）。

use std::fs;

use serde_json::json;

use super::envelope::{ArtifactCandidate, ArtifactEnvelope};
use super::registry::{ArtifactInput, ArtifactPlugin};

pub const KIND: &str = "tasks-progress";
const VERSION: u32 = 1;
const TASKS_FILE_NAME: &str = "tasks.md";

pub struct TasksProgressPlugin;

fn is_tasks_file(candidate: &ArtifactCandidate) -> bool {
    match candidate {
        ArtifactCandidate::File { relative_path } => relative_path
            .file_name()
            .is_some_and(|name| name == TASKS_FILE_NAME),
        ArtifactCandidate::EvalEntry { .. } => false,
    }
}

/// 统计 `- [ ]` / `- [x]`（含 `- [X]`）行数。
fn count_checkboxes(text: &str) -> (u32, u32) {
    let mut done = 0u32;
    let mut pending = 0u32;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("- [ ]") {
            pending += 1;
        } else if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
            done += 1;
        }
    }
    (done, pending)
}

impl ArtifactPlugin for TasksProgressPlugin {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn matches(&self, input: &ArtifactInput) -> bool {
        is_tasks_file(input.candidate)
    }

    fn parse(&self, input: &ArtifactInput) -> Option<ArtifactEnvelope> {
        let relative_path = match input.candidate {
            ArtifactCandidate::File { relative_path } => relative_path,
            ArtifactCandidate::EvalEntry { .. } => return None,
        };
        let text = fs::read_to_string(input.change_dir.join(relative_path)).ok()?;
        let (done, pending) = count_checkboxes(&text);
        let total = done + pending;
        Some(ArtifactEnvelope {
            kind: KIND.to_string(),
            version: VERSION,
            title: "任务进度".to_string(),
            payload: json!({ "total": total, "done": done, "pending": pending }),
            fallback_text: Some(format!("任务进度：已完成 {done} / {total}，待办 {pending}")),
        })
    }
}
