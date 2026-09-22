//! 清单记录模型：字段定义、name 提取、JSON 编解码、时间戳取值。纯层，无 IO。

use std::path::Path;

use serde::{Deserialize, Serialize};

/// user 维度注册表一行：key 即 `root`（canonical 完整路径），value 以 JSON 编码入表。
///
/// 时间戳均为 UTC unix 毫秒 `i64`——排序零解析零格式歧义，且 store 不引入 time 依赖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    /// canonical 完整路径（即表 key）
    pub root: String,
    /// 目录名最后一段（展示用；完整路径悬停展示）
    pub name: String,
    /// 入库时间（UTC unix 毫秒）
    pub added_at: i64,
    /// 最近打开时间（UTC unix 毫秒），清单排序依据
    pub last_opened_at: i64,
}

impl WorkspaceRecord {
    /// 由 root 构造新记录：name 取目录名最后一段，两时间戳同为 `now`（新建语义）。
    pub fn from_root(root: &str, now: i64) -> Self {
        Self {
            root: root.to_owned(),
            name: dir_name(root),
            added_at: now,
            last_opened_at: now,
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
