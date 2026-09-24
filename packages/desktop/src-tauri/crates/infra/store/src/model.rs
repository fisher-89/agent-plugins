//! 清单记录模型：native_db 模型注册（`#[native_model]` + `#[native_db]`）、
//! 字段定义、name 提取、时间戳取值。纯层，无 IO。
//!
//! 编解码由 native_model 接管（id/version 封装 + 编码后端），模型不再手写
//! `encode` / `decode`；serde camelCase 线格式不变，仅落库载体换 native_model
//! 封装。字段面零变化——shape 演进经 native_model 版本机制治理。编码后端
//! 按模型选型：默认 bincode（紧凑），`AgentEventRecord` 用 serde_json（serde
//! flatten 要求自描述编码，见模型文档）。

use std::path::Path;

use agent::AgentEvent;
use native_db::{native_db, ToKey};
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};

/// user 维度注册表一行：主键即 `root`（canonical 完整路径）。
///
/// 时间戳为 UTC unix 毫秒 `i64`——零解析零格式歧义，且 store 不引入 time 依赖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 1, version = 1)]
#[native_db]
pub struct WorkspaceRecord {
    /// canonical 完整路径（主键）
    #[primary_key]
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
}

/// `AgentRunRecord.source` 的 serde 缺省值：既有调试链路写入语义不变。
fn default_run_source() -> String {
    "debug".to_owned()
}

/// agent 运行记录：全平文字段；`status` / `env` / `permission_mode` 为受控
/// 字符串（running | completed | failed 等），store 不引本地枚举。
///
/// 时间戳均为 UTC unix 毫秒 `i64`，与 `WorkspaceRecord` 同口径。
///
/// 字段演进（version 2）：新增 `source` / `source_ref` / `parent_run_id` 三
/// 字段——来源归属与 resume 链显式指针。落库编码为 bincode（非自描述），
/// v1 载荷无法直接反序列化为本结构，经 [`AgentRunRecordV1`] 版本化结构 +
/// `From` 转换由 native_model 读路径自动升级（无手工迁移、无 legacy 迁移层）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 2, version = 2, from = AgentRunRecordV1)]
#[native_db]
pub struct AgentRunRecord {
    /// run id（主键，写事务内 max+1 分配）
    #[primary_key]
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
    /// 会话 id（来自 result / init 事件，续会话入参来源）
    pub session_id: Option<String>,
    /// 失败原因（落库失败收敛 / 无 result 异常终止时填因）
    pub error: Option<String>,
    /// 来源受控字符串（debug | explore | …），缺省 debug（调试链路语义不变）
    #[serde(default = "default_run_source")]
    pub source: String,
    /// 来源内定位（explore 指向探索记录主键的十进制串；调试 run 为 None）
    #[serde(default)]
    pub source_ref: Option<String>,
    /// resume 链显式指针（本 run 的上游 run id；链首为 None）
    #[serde(default)]
    pub parent_run_id: Option<i64>,
}

/// `AgentRunRecord` 的 v1 版本化结构（13 字段，演进前形态）：不注册
/// `#[native_db]`（不参与模型定义），仅承载 v1 载荷解码与到 v2 的升级转换，
/// 供 native_model `from` 机制与 v1 fixture 测试使用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 2, version = 1)]
pub struct AgentRunRecordV1 {
    /// run id（主键）
    pub id: i64,
    /// 提示词原文
    pub prompt: String,
    /// 工作目录
    pub cwd: String,
    /// 环境档位受控字符串
    pub env: String,
    /// permission-mode 受控字符串
    pub permission_mode: String,
    /// run 状态受控字符串
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
    /// 会话 id
    pub session_id: Option<String>,
    /// 失败原因
    pub error: Option<String>,
}

impl From<AgentRunRecordV1> for AgentRunRecord {
    fn from(v1: AgentRunRecordV1) -> Self {
        Self {
            id: v1.id,
            prompt: v1.prompt,
            cwd: v1.cwd,
            env: v1.env,
            permission_mode: v1.permission_mode,
            status: v1.status,
            started_at: v1.started_at,
            finished_at: v1.finished_at,
            num_turns: v1.num_turns,
            cost_usd: v1.cost_usd,
            duration_ms: v1.duration_ms,
            session_id: v1.session_id,
            error: v1.error,
            source: default_run_source(),
            source_ref: None,
            parent_run_id: None,
        }
    }
}

/// 反向转换仅满足 native_model 宏生成的 downgrade 编码路径的 trait 约束
/// （应用只升级不降级，不调用 `encode_downgrade`）；转换即丢弃三个新字段。
impl From<AgentRunRecord> for AgentRunRecordV1 {
    fn from(v2: AgentRunRecord) -> Self {
        Self {
            id: v2.id,
            prompt: v2.prompt,
            cwd: v2.cwd,
            env: v2.env,
            permission_mode: v2.permission_mode,
            status: v2.status,
            started_at: v2.started_at,
            finished_at: v2.finished_at,
            num_turns: v2.num_turns,
            cost_usd: v2.cost_usd,
            duration_ms: v2.duration_ms,
            session_id: v2.session_id,
            error: v2.error,
        }
    }
}

/// explore 清单记录（user 维度，registry / 绑定元数据，与 [`WorkspaceRecord`]
/// 同先例落 app data dir user db）：内容唯一真源在磁盘笔记文件（由 agent 会话
/// 流程懒创建），记录是身份、文件是可丢弃投影——文件被删记录保留，未落盘记录
/// 照常存在（数据三分：记录 / 内容 / 对话）。独立主键与文件名解耦：文件改名
/// 经 in-place 改 `name` 保主键，会话链绑定不破。
///
/// 时间戳均为 UTC unix 毫秒 `i64`，与 [`WorkspaceRecord`] 同口径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 4, version = 1)]
#[native_db]
pub struct ExploreRecord {
    /// 记录 id（主键，写事务内 max+1 分配；身份与文件名解耦）
    #[primary_key]
    pub id: i64,
    /// workspace 归属（canonical root，与 `WorkspaceRecord.root` 同口径）
    pub root: String,
    /// 展示名（= 笔记文件 stem，磁盘寻址键）
    pub name: String,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒）
    pub updated_at: i64,
}

impl ExploreRecord {
    /// 由归属与名称构造新记录：`id` 置 0（写事务内 max+1 分配覆盖），
    /// `created_at = updated_at = now`（新建语义）。
    pub fn new(root: &str, name: &str, now: i64) -> Self {
        Self {
            id: 0,
            root: root.to_owned(),
            name: name.to_owned(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// agent 运行事件记录（类型化新建）：包装 struct 打 native_db derive，嵌装
/// core `agent::AgentEvent` 纯类型作载荷——core 保持 derive-free，native_model
/// 版本治理全部留在 infra 侧。
///
/// 编码后端为 serde_json（[`SerdeJsonCodec`]）：`AgentEvent` 内部 tag 枚举
/// 经 `#[serde(flatten)]` 扁平进信封，serde 的 flatten 语义要求自描述编码，
/// bincode 1.3 的定长 map 不支持；JSON 与 core「serde camelCase 线格式即
/// 落库形态」口径一致。
///
/// 主键为合成 u128 打包键（native_db 复合主键不受支持，见 design Spike①）：
/// 高 64 位 run_id、低 64 位 seq，`to_key()` 大端字节序保证字典序即数值序，
/// 同 run 内扫描自然序即重放序。`run_id` 另立非唯一二级索引承载重放查询。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 3, version = 1, with = SerdeJsonCodec)]
#[native_db]
pub struct AgentEventRecord {
    /// 复合键打包：`(run_id as u128) << 64 | seq`
    ///
    /// serde 定制为十六进制字符串：serde_json 无 u128 支持且打包值必超
    /// u64 上界（信封 API 要把记录转 JSON），字符串形态保住 JSON 可表达性。
    #[primary_key]
    #[serde(with = "event_key_serde")]
    pub event_key: u128,
    /// 所属 run id（非唯一二级索引，重放查询入口）
    #[secondary_key]
    pub run_id: i64,
    /// 事件载荷（嵌装 core 纯类型，含 `Raw` 逃生舱）
    pub event: AgentEvent,
}

impl AgentEventRecord {
    /// 由 run id 与事件构造记录：`event_key` 打包自 `run_id` + `event.seq`。
    pub fn new(run_id: i64, event: AgentEvent) -> Self {
        Self {
            event_key: pack_event_key(run_id, event.seq),
            run_id,
            event,
        }
    }

    /// 所属 run id（打包键还原口径，重放扫描方免解载荷）。
    pub fn run_id(&self) -> i64 {
        self.run_id
    }

    /// 事件序号（同 run 内单调递增；打包键低 64 位，与 `event.seq` 同源）。
    pub fn seq(&self) -> u64 {
        self.event.seq
    }
}

/// 复合键打包：高 64 位 run_id、低 64 位 seq。store 内唯一组装点。
pub(crate) fn pack_event_key(run_id: i64, seq: u64) -> u128 {
    ((run_id as u128) << 64) | (seq as u128)
}

/// `event_key` 的 serde 定制：u128 ↔ 十六进制字符串（serde_json 数字面不收
/// u128，见字段文档）。
mod event_key_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: &u128, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{value:#034x}"))
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<u128, D::Error> {
        let text = String::deserialize(deserializer)?;
        let digits = text.trim_start_matches("0x");
        u128::from_str_radix(digits, 16).map_err(serde::de::Error::custom)
    }
}

/// serde_json 编码后端（native_model 自定义 codec，`with = SerdeJsonCodec`）：
/// 仅 [`AgentEventRecord`] 使用——serde flatten 要求自描述编码（bincode 不
/// 支持，见模型文档）；其余模型保持默认 bincode（紧凑，字段面平直）。
pub(crate) struct SerdeJsonCodec;

impl<T: Serialize> native_model::Encode<T> for SerdeJsonCodec {
    type Error = serde_json::Error;

    fn encode(obj: &T) -> Result<Vec<u8>, Self::Error> {
        serde_json::to_vec(obj)
    }
}

impl<T: serde::de::DeserializeOwned> native_model::Decode<T> for SerdeJsonCodec {
    type Error = serde_json::Error;

    fn decode(data: Vec<u8>) -> Result<T, Self::Error> {
        serde_json::from_slice(&data)
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
