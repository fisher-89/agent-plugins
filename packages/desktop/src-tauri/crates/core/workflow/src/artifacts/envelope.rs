//! 产物信封契约：Rust 解析侧与 React 渲染侧之间唯一共享的知识。

use serde::{Deserialize, Serialize};
use specta::Type;

/// 中间产物信封：所有产物以统一形状经 IPC 传递。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEnvelope {
    /// 契约 ID（如 "tasks-progress"），前端据此路由 renderer
    pub kind: String,
    /// 产物格式版本
    pub version: u32,
    pub title: String,
    /// kind 自描述结构化负载
    pub payload: serde_json::Value,
    /// 渲染器缺席时的保底文本
    pub fallback_text: Option<String>,
}

/// 产物寻址清单项：`source` 为 change 内相对 POSIX 路径；
/// `phase` 为**清单归属**（清单声明的相位）——前端按本归属挂列，MUST NOT 重建
/// 第二份静态映射表。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDescriptor {
    pub kind: String,
    pub source: String,
    pub title: String,
    /// phase 归属（清单声明供给）
    pub phase: String,
}
