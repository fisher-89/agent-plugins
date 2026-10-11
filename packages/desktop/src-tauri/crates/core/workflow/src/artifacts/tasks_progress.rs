//! `tasks-progress` 插件：清单声明的 tasks.md 勾选计数（total / done /
//! pending）。候选来源 = phase 定义清单（`kind == "tasks-progress"` 的相对路径枚
//! 举命中项）——「文件名 == tasks.md」旧判据退役（清单之外同名文件零候选）。

use std::fs;

use serde_json::json;

use super::envelope::{ArtifactCandidate, ArtifactEnvelope};
use super::registry::{ArtifactInput, ArtifactPlugin};

pub const KIND: &str = "tasks-progress";
const VERSION: u32 = 1;

/// 清单未供给标题时的兜底（相位表清单声明值同源）。
const DEFAULT_TITLE: &str = "任务进度";

pub struct TasksProgressPlugin;

/// 清单类型判定：候选恒来自清单枚举，kind 由清单声明（非文件名猜测）。
fn is_declared_tasks(input: &ArtifactInput) -> bool {
    input.spec.is_some_and(|spec| spec.kind == KIND)
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
        matches!(input.candidate, ArtifactCandidate::File { .. }) && is_declared_tasks(input)
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
            // 清单声明标题为准（清单缺席的 IPC 直查面回落定式标题）
            title: input
                .spec
                .map(|spec| spec.title)
                .filter(|title| !title.is_empty())
                .unwrap_or(DEFAULT_TITLE)
                .to_string(),
            payload: json!({ "total": total, "done": done, "pending": pending }),
            fallback_text: Some(format!("任务进度：已完成 {done} / {total}，待办 {pending}")),
        })
    }
}
