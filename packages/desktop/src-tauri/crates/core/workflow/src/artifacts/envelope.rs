//! 产物信封契约：Rust 解析侧与 React 渲染侧之间唯一共享的知识。

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// 中间产物信封：所有产物以统一形状经 IPC 传递。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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

/// 产物寻址清单项：`source` 为 change 内相对 POSIX 路径或 eval 条目序号串；
/// `phase` 为**清单归属**（文件候选 = 清单声明相位；eval 候选 = PhaseRecord 投影
/// 的相位）——前端按本归属挂列，MUST NOT 重建第二份静态映射表。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDescriptor {
    pub kind: String,
    pub source: String,
    pub title: String,
    /// phase 归属（清单声明 / PhaseRecord 投影供给）
    pub phase: String,
}

/// 产物候选：**清单命中的文件**，或 phase 评估史中的条目（`(phase, attempt)` 投
/// 影）。文件候选来源恒为 phase 定义清单（白名单驱动），无清单之外的全树扫描
/// 候选。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactCandidate {
    /// 清单相对路径枚举命中的文件
    File { relative_path: PathBuf },
    /// phase 评估史条目（eval-checklist 投影分支）
    EvalEntry {
        /// 评估史切片下标（`(phase, attempt)` 关联，索引稳定）
        index: usize,
    },
}
