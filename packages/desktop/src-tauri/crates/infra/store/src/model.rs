//! 清单记录模型：字段定义、name 提取、JSON 编解码、时间戳取值。纯层，无 IO。

use std::path::Path;

use serde::{Deserialize, Serialize};

/// user 维度注册表一行：key 即 `root`（canonical 完整路径），value 以 JSON 编码入表。
///
/// 时间戳为 UTC unix 毫秒 `i64`——零解析零格式歧义，且 store 不引入 time 依赖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    /// canonical 完整路径（即表 key）
    pub root: String,
    /// 目录名最后一段（展示用；完整路径悬停展示）
    pub name: String,
    /// 入库时间（UTC unix 毫秒）
    pub added_at: i64,
}

impl WorkspaceRecord {
    /// 由 root 构造新记录：name 取目录名最后一段，`added_at` 取 `now`（新建语义）。
    pub fn from_root(root: &str, now: i64) -> Self {
        Self {
            root: root.to_owned(),
            name: dir_name(root),
            added_at: now,
        }
    }

    /// JSON 编码（value 落库形态）；编码失败视作 db 层错误。
    pub(crate) fn encode(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

    /// JSON 解码（value 出库形态）；解码失败说明库内容损坏，视作 db 层错误。
    pub(crate) fn decode(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }
}

/// agent 运行记录（`user_agent_runs` 表一行，value 以 JSON 编码入表）：
/// 全平文字段；`status` / `env` / `permission_mode` 为受控字符串（running |
/// completed | failed 等），store 不引本地枚举。事件行不经本模型——以
/// `serde_json::Value` 进出事件表（store 禁依赖 core 契约 crate）。
///
/// 时间戳均为 UTC unix 毫秒 `i64`，与 `WorkspaceRecord` 同口径。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunRecord {
    /// run id（表 key，写事务内 max+1 分配）
    pub id: i64,
    /// 提示词原文
    pub prompt: String,
    /// 工作目录
    pub cwd: String,
    /// 环境档位受控字符串（default | bare）
    pub env: String,
    /// permission-mode 受控字符串（default | acceptEdits | bypassPermissions）
    pub permission_mode: String,
    /// run 状态受控字符串（running | completed | failed）
    pub status: String,
    /// 开始时间（UTC unix 毫秒）
    pub started_at: i64,
    /// 结束时间；运行中为 None
    pub finished_at: Option<i64>,
    /// 收敛轮数（来自 result 事件）
    pub num_turns: Option<u64>,
    /// 总成本美元（来自 result 事件）
    pub cost_usd: Option<f64>,
    /// 运行时长毫秒（来自 result 事件）
    pub duration_ms: Option<u64>,
    /// 会话 id（来自 result / init 事件，续会话未来账的信封预留）
    pub session_id: Option<String>,
    /// 失败原因（落库失败收敛 / 无 result 异常终止时填因）
    pub error: Option<String>,
}

impl AgentRunRecord {
    /// JSON 编码（value 落库形态）；编码失败视作 db 层错误。
    pub(crate) fn encode(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

    /// JSON 解码（value 出库形态）；解码失败说明库内容损坏，视作 db 层错误。
    pub(crate) fn decode(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }
}

/// 目录名最后一段（`D:\work\my-project` → `my-project`）；无文件名段时回退整串。
fn dir_name(root: &str) -> String {
    Path::new(root)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_owned())
}

/// UTC unix 毫秒：std 唯一时间源（时钟早于 epoch 时取 0，不 panic）。
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
