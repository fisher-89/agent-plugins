//! 清单记录模型：native_db 模型注册（`#[native_model]` + `#[native_db]`）、
//! 字段定义、name 提取、时间戳取值。纯层，无 IO。
//!
//! 编解码由 native_model 接管（id/version 封装 + 编码后端），模型不再手写
//! `encode` / `decode`；serde camelCase 线格式不变，仅落库载体换 native_model
//! 封装。字段面零变化——shape 演进经 native_model 版本机制治理。编码后端
//! 按模型选型：默认 bincode（紧凑），`AgentEventRecord` 用 serde_json（serde
//! flatten 要求自描述编码，见模型文档）。

use std::path::Path;

use agent::{AgentEnvMode, AgentEvent, AgentPermissionMode, AgentRunStatus};
use native_db::{native_db, ToKey};
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};
use specta::Type;

/// user 维度注册表一行（落全局库 `desktop-global.redb`，见 desktop-data-dimensions）：
/// 主键即 `root`（canonical 完整路径）。
///
/// 时间戳为 UTC unix 毫秒 `i64`——零解析零格式歧义，且 store 不引入 time 依赖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
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

/// agent 运行记录（workspace 维度，落所属 workspace 的独立 db 文件
/// `workspaces/` 子树，cwd 恒为当前 workspace root 即归属键，见
/// desktop-data-dimensions）：全平文字段；`status` / `env` / `permission_mode`
/// 为 core/agent 契约枚举（serde camelCase 值域与枚举化前受控字符串逐字一致，
/// serde JSON 线格式零变化）。
///
/// run id 为所属 workspace 库域内自增（写事务内 max+1），跨 workspace 不假定
/// 全局唯一，跨库定位携 root。
///
/// 时间戳均为 UTC unix 毫秒 `i64`，与 `WorkspaceRecord` 同口径。
///
/// 字段演进：version 2 新增 `source` / `source_ref` / `parent_run_id` 三字段
/// （来源归属与 resume 链显式指针）；version 3 三字段 String → 枚举。v1 / v2
/// 历史版本化结构与升级链已移除（不做旧库兼容定夺）：v1 / v2 版本头存量
/// 载荷不再可读，native_model 读路径版本不支持直接报错。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 2, version = 3)]
#[native_db]
pub struct AgentRunRecord {
    /// run id（主键，写事务内 max+1 分配）
    #[primary_key]
    pub id: i64,
    /// 提示词原文
    pub prompt: String,
    /// 工作目录
    pub cwd: String,
    /// 环境档位（default | bare）
    pub env: AgentEnvMode,
    /// permission-mode 档位（default | acceptEdits | bypassPermissions）
    pub permission_mode: AgentPermissionMode,
    /// run 状态（running | completed | failed | stopped）
    pub status: AgentRunStatus,
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

/// explore 清单记录（workspace 维度，落所属 workspace 的独立 db 文件
/// `workspaces/` 子树，`root` 即归属键，与名下会话链同库——级联删除与链还原
/// 同实例收敛）：内容唯一真源在磁盘笔记文件（由 agent 会话流程懒创建），记录
/// 是身份、文件是可丢弃投影——文件被删记录保留，未落盘记录照常存在（数据三
/// 分：记录 / 内容 / 对话）。独立主键与文件名解耦：文件改名经 in-place 改
/// `name` 保主键，会话链绑定不破。
///
/// 时间戳均为 UTC unix 毫秒 `i64`，与 [`WorkspaceRecord`] 同口径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
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

/// agent 运行事件记录（workspace 维度，落所属 run 同一 workspace 库——同库
/// 内 N:1 引用，无跨库引用）：包装 struct 打 native_db derive，嵌装
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

// --- agent 管理记录（user 维度，全局库 desktop-global.redb）----------------

/// provider 三档模型档位（agent 管理域，纯嵌套 struct 不落独立模型——嵌装
/// 先例同 [`AgentEventRecord`] 的 `AgentEvent`）：high / medium / low 三档
/// 模型标识，运行发起解析消费固定取 high 档（effort 进 run 参数与档位选择
/// 器为后续迭代，本期只存不选）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentModelTiers {
    /// high 档模型标识（运行发起解析消费档）
    pub high: String,
    /// medium 档模型标识（本期只存不选）
    pub medium: String,
    /// low 档模型标识（本期只存不选）
    pub low: String,
}

/// agent 实例引擎二值（agent 管理域，store 本地枚举）：serde camelCase 线值
/// `"cli" | "sdk"` 与门面 `EngineKind` 同线值域。持久化的是配置值域而非引擎
/// 实现——store 禁拖引擎门面 crate（零新增依赖），`AgentEngineKind` →
/// `EngineKind` 映射收运行发起解析单点（两臂 match）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentEngineKind {
    /// 本机 claude CLI 租户
    Cli,
    /// 进程内 sdk 租户（rig-core 直连 openai 兼容端点）
    Sdk,
}

/// api_key 遮蔽形态（`sk-***abc`：末 3 字符；空 / 过短恒 `sk-***`）。Debug
/// 遮蔽实现与前端 `MaskedApiKey` 组件同口径（展示层遮蔽，非脱敏存储）。
pub(crate) fn mask_api_key(api_key: &str) -> String {
    let chars: Vec<char> = api_key.chars().collect();
    if chars.len() > 3 {
        let suffix: String = chars[chars.len() - 3..].iter().collect();
        format!("sk-***{suffix}")
    } else {
        "sk-***".to_owned()
    }
}

/// agent provider 记录（agent 管理域，user 维度落全局库，见
/// desktop-data-dimensions）：openai 兼容端点连接档案（base_url / api_key /
/// 三档 model），被 [`AgentInstanceRecord`] 按 `provider_id` N:1 引用（删除
/// 阻止），亦是后续 workspace→agent 关联链的引用锚点之一（稳定 id 主键）。
///
/// // 机密面有意放宽:api_key 全链路明文（IPC body / 全局库文件 / 进程内存），
/// 边界表见 specs/desktop-agent-management ——读写单 DTO 即记录本体（无遮蔽
/// 信封臂），遮蔽只在前端展示层；结构保证明文不进日志：**不 derive
/// `Debug`**，手写遮蔽 impl（api_key 位 [`mask_api_key`] 形态），测试比较走
/// [`PartialEq`]。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 5, version = 1)]
#[native_db]
pub struct AgentProviderRecord {
    /// 记录 id（主键，写事务内 max+1 分配）
    #[primary_key]
    pub id: i64,
    /// 展示名（唯一 = 写事务内查重）
    pub name: String,
    /// openai 兼容端点 base_url（含版本段，如 `https://…/v1`）
    pub base_url: String,
    /// 认证凭据（bearer token，明文）
    // 机密面有意放宽:明文存储与传输（读写单 DTO 即记录本体，无遮蔽信封臂），
    // 遮蔽只在前端展示层，边界表见 specs/desktop-agent-management
    pub api_key: String,
    /// 三档模型标识（运行发起解析消费固定取 high 档）
    pub models: AgentModelTiers,
}

impl AgentProviderRecord {
    /// 由连接档案构造新记录：`id` 置 0（写事务内 max+1 分配覆盖）；api_key
    /// 传空即空（新建语义无原值可保）。
    pub fn new(name: String, base_url: String, api_key: String, models: AgentModelTiers) -> Self {
        Self {
            id: 0,
            name,
            base_url,
            api_key,
            models,
        }
    }
}

/// 手写遮蔽 Debug（字段面与 derive 形态逐位对齐，仅 api_key 位遮蔽）：明文
/// 不进任何日志 / 错误串（错误串只含 name / id / 计数）。
impl std::fmt::Debug for AgentProviderRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentProviderRecord")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("api_key", &mask_api_key(&self.api_key))
            .field("models", &self.models)
            .finish()
    }
}

/// agent 实例记录（agent 管理域，user 维度落全局库）：引擎选择（cli / sdk）
/// 与 provider 引用（sdk 必填且引用存在、cli 可空）+ 默认标记（全局恒至多
/// 一；写入只发生在 store 的 `set_default_agent_instance` 与删除清标记两处，
/// upsert 不参与写）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 6, version = 1)]
#[native_db]
pub struct AgentInstanceRecord {
    /// 记录 id（主键，写事务内 max+1 分配；后续 workspace 关联引用锚点）
    #[primary_key]
    pub id: i64,
    /// 展示名（唯一 = 写事务内查重）
    pub name: String,
    /// 引擎二值（cli / sdk；映射到门面 `EngineKind` 收运行发起解析单点）
    pub engine: AgentEngineKind,
    /// 引用 provider id（同库 [`AgentProviderRecord`] 主键；sdk 必填、cli 可空）
    pub provider_id: Option<i64>,
    /// 默认标记（缺省运行解析入口；全局恒至多一）
    pub is_default: bool,
}

impl AgentInstanceRecord {
    /// 由实例档案构造新记录：`id` 置 0（写事务内 max+1 分配覆盖）、
    /// `is_default = false`（默认标记唯一写口为 `set_default_agent_instance`，
    /// 新建恒非默认）。
    pub fn new(name: String, engine: AgentEngineKind, provider_id: Option<i64>) -> Self {
        Self {
            id: 0,
            name,
            engine,
            provider_id,
            is_default: false,
        }
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
/// 展示名（`WorkspaceRecord::from_root`）与 workspace 库文件名可读段（store
/// 路径派生）同源取末段。
pub(crate) fn dir_name(root: &str) -> String {
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
