//! `workflow.json` 相关领域类型（形状以 TS 侧 schema 为唯一真理源）。
//!
//! 序列化约定：对外（DTO → 前端）输出 camelCase；解析磁盘 JSON 时通过
//! `alias` 兼容其 snake_case 键名。字段一律宽松 Option / default，
//! 未知字段由 serde 自行忽略（含 legacy `files` 桶、`source` 旁挂映射）。

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// 宽松时间戳：字段缺失、null 或 ISO 8601 解析失败一律降级为 `None`，
/// 不触发所在条目的解析失败。
pub(crate) mod lenient_timestamp {
    use serde::{Deserialize, Deserializer, Serializer};
    use time::format_description::well_known::{Iso8601, Rfc3339};
    use time::OffsetDateTime;

    fn parse(value: &str) -> Option<OffsetDateTime> {
        OffsetDateTime::parse(value, &Rfc3339)
            .or_else(|_| OffsetDateTime::parse(value, &Iso8601::DEFAULT))
            .ok()
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<OffsetDateTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: Option<String> = Option::deserialize(deserializer)?;
        Ok(raw.as_deref().and_then(parse))
    }

    pub(crate) fn serialize<S>(
        value: &Option<OffsetDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(timestamp) => {
                let text = timestamp.format(&Rfc3339).unwrap_or_default();
                serializer.serialize_str(&text)
            }
            None => serializer.serialize_none(),
        }
    }
}

/// 评估 verdict。条目级严格：非法值触发该条降级跳过。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Pass,
    Fail,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
        }
    }
}

/// checklist 检查项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub item: String,
    pub pass: bool,
    pub evidence: String,
}

/// 一条评估历史（`workflow.json.eval[]` 条目）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseLog {
    pub phase: String,
    #[serde(default)]
    pub attempt: Option<u32>,
    pub verdict: Verdict,
    pub report: String,
    #[serde(default)]
    pub checklist: Vec<ChecklistItem>,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default)]
    pub stale: bool,
    #[serde(
        default,
        alias = "start_at",
        deserialize_with = "lenient_timestamp::deserialize",
        serialize_with = "lenient_timestamp::serialize"
    )]
    pub start_at: Option<OffsetDateTime>,
    #[serde(
        default,
        deserialize_with = "lenient_timestamp::deserialize",
        serialize_with = "lenient_timestamp::serialize"
    )]
    pub timestamp: Option<OffsetDateTime>,
    #[serde(default, alias = "backtrack_to")]
    pub backtrack_to: Option<String>,
    #[serde(default, alias = "backtrack_reason")]
    pub backtrack_reason: Option<String>,
}

/// file_log 记录的文件操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileLogOp {
    Write,
    Delete,
    Revert,
}

/// 一条日志式文件清单（`workflow.json.file_log[]` 条目）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileLogEntry {
    pub op: FileLogOp,
    pub scope: String,
    #[serde(default)]
    pub attempt: Option<u32>,
    pub path: String,
    #[serde(
        default,
        deserialize_with = "lenient_timestamp::deserialize",
        serialize_with = "lenient_timestamp::serialize"
    )]
    pub at: Option<OffsetDateTime>,
}

/// 运行中 phase 状态（`workflow.json.active_phase`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePhase {
    pub phase: String,
    pub attempt: u32,
    #[serde(
        default,
        alias = "start_at",
        deserialize_with = "lenient_timestamp::deserialize",
        serialize_with = "lenient_timestamp::serialize"
    )]
    pub start_at: Option<OffsetDateTime>,
}

/// 中断留档（`workflow.json.interrupted[]` 条目）：ActivePhase 加 `end_at` 的扩展。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedEntry {
    pub phase: String,
    pub attempt: u32,
    #[serde(
        default,
        alias = "start_at",
        deserialize_with = "lenient_timestamp::deserialize",
        serialize_with = "lenient_timestamp::serialize"
    )]
    pub start_at: Option<OffsetDateTime>,
    #[serde(
        default,
        alias = "end_at",
        deserialize_with = "lenient_timestamp::deserialize",
        serialize_with = "lenient_timestamp::serialize"
    )]
    pub end_at: Option<OffsetDateTime>,
}

/// `workflow.json` 的领域形状（v2）；v1 形状经宽松解析收敛到同一类型，
/// 差异仅体现为 `file_log` 为 `None`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub workflow_type: String,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub eval: Vec<PhaseLog>,
    #[serde(default, alias = "file_log")]
    pub file_log: Option<Vec<FileLogEntry>>,
    #[serde(default, alias = "active_phase")]
    pub active_phase: Option<ActivePhase>,
    #[serde(default)]
    pub interrupted: Vec<InterruptedEntry>,
}
