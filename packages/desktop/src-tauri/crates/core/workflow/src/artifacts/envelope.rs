//! 产物信封契约：Rust 解析侧与 React 渲染侧之间唯一共享的知识。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 中间产物信封：所有产物以统一形状经 IPC 传递。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEnvelope {
    /// 契约 ID（如 "eval-checklist"），前端据此路由 renderer
    pub kind: String,
    /// 产物格式版本
    pub version: u32,
    pub title: String,
    /// kind 自描述结构化负载
    pub payload: serde_json::Value,
    /// 渲染器缺席时的保底文本
    pub fallback_text: Option<String>,
}

/// 产物寻址清单项：`source` 为 change 内相对 POSIX 路径或 eval 条目序号串。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDescriptor {
    pub kind: String,
    pub source: String,
    pub title: String,
}

/// 产物候选：文件树中的文件，或 workflow eval 历史中的条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactCandidate {
    File {
        relative_path: PathBuf,
    },
    EvalEntry {
        /// 解析后 `Workflow.eval` 的下标（损坏条目已被解析层跳过，索引稳定）
        index: usize,
    },
}
