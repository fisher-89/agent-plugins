use std::path::Path;

use agent::{AgentEvent, AgentPermissionMode, AgentRunStatus};
use native_db::{native_db, ToKey};
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;

/// v3 历史形态的环境档位枚举（仅升级链解码用；线值与退役的 core 枚举一致，
/// `default` | `bare`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AgentEnvModeLegacy {
    /// 完整环境（页面默认档）
    Default,
    /// 纯净档（显式开关）
    Bare,
}

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

/// `AgentRunRecord` 的 version 3 历史形态（仅作 native_model 升级链的解码
/// 目标，不注册进库模型组、不出公共查询面）：会话化前的 run 全平文字段。
/// 存量 v3 行经版本机制自动升级为 v4 孤儿轮行（零迁移代码路径）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 2, version = 3)]
pub(crate) struct AgentRunRecordV3 {
    /// run id（主键，写事务内 max+1 分配）
    pub id: i64,
    /// 提示词原文
    pub prompt: String,
    /// 工作目录
    pub cwd: String,
    /// 环境档位（default | bare）
    pub env: AgentEnvModeLegacy,
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
    /// 引擎侧会话 id（来自 result / init 事件；会话化后归属以 `SessionRecord`
    /// 为准，升级时不平移）
    pub session_id: Option<String>,
    /// 失败原因（落库失败收敛 / 无 result 异常终止时填因）
    pub error: Option<String>,
    /// 来源受控字符串（debug | explore | …），缺省 debug
    #[serde(default = "default_run_source")]
    pub source: String,
    /// 来源内定位（explore 指向探索记录主键的十进制串；调试 run 为 None）
    #[serde(default)]
    pub source_ref: Option<String>,
    /// resume 链显式指针（本 run 的上游 run id；链首为 None）
    #[serde(default)]
    pub parent_run_id: Option<i64>,
}

/// `AgentRunRecord.source` 的 serde 缺省值：v3 升级链解码缺省（既有调试链路
/// 写入语义不变）。
fn default_run_source() -> String {
    "debug".to_owned()
}

/// agent 轮统计行（workspace 维度，落所属 workspace 的独立 db 文件
/// `workspaces/` 子树，cwd 恒为当前 workspace root 即归属键，见
/// desktop-data-dimensions）：会话一等公民落地后的 run 退化形态——每轮一行，
/// 挂 core 会话外键；同 session 的轮序列即链（`started_at` 有序），转录
/// 归属转录单表（`SessionEventRecord`）。
///
/// turn id 为所属 workspace 库域内自增（写事务内 max+1），跨 workspace 不
/// 假定全局唯一，跨库定位携 root。
///
/// 时间戳均为 UTC unix 毫秒 `i64`，与 `WorkspaceRecord` 同口径。
///
/// 字段演进：version 2 新增 `source` / `source_ref` / `parent_run_id`；
/// version 3 三字段 String → 枚举；version 4 轮统计行化（来源归属主平移至
/// `SessionRecord`、链指针语义由会话归属取代）——字段面收敛为 id /
/// `session_id`（core 会话外键）/ status / 起止时间戳 / TurnDone 统计三字段 /
/// error；存量 v3 行升级为无会话归属孤儿轮行（`session_id = None`，统计与
/// 时间戳保留），prompt / cwd / env / permission_mode / source / source_ref /
/// parent_run_id 随版本退役。IPC 面由 core `TurnSummary` 承载（port 映射出），
/// 本类型退为 store 持久化内部类型。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 2, version = 4, from = AgentRunRecordV3)]
#[native_db]
pub struct AgentRunRecord {
    /// turn id（主键，写事务内 max+1 分配）
    #[primary_key]
    pub id: i64,
    /// 所属 core 会话 id（`SessionRecord` 主键；新行恒 Some，存量升级行为
    /// None 孤儿轮行）
    pub session_id: Option<String>,
    /// 轮状态（running | completed | failed | stopped）
    pub status: AgentRunStatus,
    /// 开始时间（UTC unix 毫秒）
    pub started_at: i64,
    /// 结束时间；运行中为 None
    pub finished_at: Option<i64>,
    /// 收敛轮数（来自 TurnDone 事件）
    pub num_turns: Option<u64>,
    /// 总成本美元（来自 TurnDone 事件）
    pub cost_usd: Option<f64>,
    /// 运行时长毫秒（来自 TurnDone 事件）
    pub duration_ms: Option<u64>,
    /// 失败原因（落库失败收敛 / 无 TurnDone 异常终止时填因）
    pub error: Option<String>,
}

impl From<AgentRunRecordV3> for AgentRunRecord {
    fn from(previous: AgentRunRecordV3) -> Self {
        Self {
            id: previous.id,
            // 存量行升级为无会话归属孤儿轮行（统计字段保留；旧引擎侧会话 id
            // 不平移——归属语义已由 core 会话取代）
            session_id: None,
            status: previous.status,
            started_at: previous.started_at,
            finished_at: previous.finished_at,
            num_turns: previous.num_turns,
            cost_usd: previous.cost_usd,
            duration_ms: previous.duration_ms,
            error: previous.error,
        }
    }
}

/// 降级半边（native_model `from` 属性要求双向 `From`；运行时无降级读取路径，
/// 退役字段以缺省占位——只保升级语义真实性，降级形态不作数据承诺）。
impl From<AgentRunRecord> for AgentRunRecordV3 {
    fn from(record: AgentRunRecord) -> Self {
        Self {
            id: record.id,
            prompt: String::new(),
            cwd: String::new(),
            env: AgentEnvModeLegacy::Default,
            permission_mode: AgentPermissionMode::Default,
            status: record.status,
            started_at: record.started_at,
            finished_at: record.finished_at,
            num_turns: record.num_turns,
            cost_usd: record.cost_usd,
            duration_ms: record.duration_ms,
            session_id: None,
            error: record.error,
            source: default_run_source(),
            source_ref: None,
            parent_run_id: None,
        }
    }
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

/// agent 会话记录（workspace 维度，落所属 workspace 的独立 db 文件
/// `workspaces/` 子树，与名下转录 / 轮统计行同库——级联删除与全史重放同实例
/// 收敛）：会话一等公民的落库形态，core 铸 id 直作字符串主键（`WorkspaceRecord.root`
/// 字符串主键既有先例）。
///
/// `config_snapshot` 存装配配置快照而非跨库引用（`AgentInstanceRecord` 在
/// 全局库、session 在 workspace 库，store 惯例禁跨库引用；实例改名 / 删除
/// 不伤历史会话）。`source` / `source_ref` 为会话级来源归属（会话化后来源
/// 圈定的主归属，explore 级联删除据此圈定）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 7, version = 1)]
#[native_db]
pub struct SessionRecord {
    /// core 铸会话 id（字符串主键，`ses-` 前缀铸造格式由 core 单点承载）
    #[primary_key]
    pub id: String,
    /// 引擎侧会话标识（双 id 映射落库半边；未上报为 None）
    pub engine_session_id: Option<String>,
    /// 装配配置快照（engine / model / permission；快照非引用）
    pub config_snapshot: SessionConfigSnapshot,
    /// 来源受控字符串（debug | explore | …），缺省 debug
    pub source: String,
    /// 来源内定位（explore 指向探索记录主键的十进制串；调试会话为 None）
    pub source_ref: Option<String>,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒，轮事件绑定引擎标识时刷新）
    pub updated_at: i64,
}

/// 装配配置快照（store 本地定型，嵌套 struct 不落独立模型——嵌装先例同
/// 转录单表的 `AgentEvent`）：engine / model / permission 三面。快照是记录
/// 自证——续会话引擎路由凭此比对（core 契约不解释装配概念，core 侧以
/// serde_json::Value 不透明承载）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfigSnapshot {
    /// 引擎二值（cli / sdk，store 本地枚举同 [`AgentEngineKind`]）
    pub engine: AgentEngineKind,
    /// 模型标识（cli 引擎无模型装配概念，None）
    pub model: Option<String>,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
}

/// agent 会话转录单表（workspace 维度，落所属会话同一 workspace 库——同库
/// 内 N:1 引用，无跨库引用）：密封事件**直挂 session** 的转录载体（会话全史
/// = 重放与 resume 重建唯一来源），run 退化为轮统计行不再挂事件。包装 struct
/// 打 native_db derive，嵌装 core `agent::AgentEvent` 纯类型作载荷——core
/// 保持 derive-free，native_model 版本治理全部留在 infra 侧。**密封事件
/// only**：`MessageDelta` 永不落库（sink 防御性忽略）。
///
/// 编码后端为 serde_json（[`SerdeJsonCodec`]）：`AgentEvent` 内部 tag 枚举
/// 经 `#[serde(flatten)]` 扁平进信封，serde 的 flatten 语义要求自描述编码，
/// bincode 1.3 的定长 map 不支持；JSON 与 core「serde camelCase 线格式即
/// 落库形态」口径一致。
///
/// 主键为合成 u128 打包键（native_db 复合主键不受支持，既有 Spike① 留痕）：
/// 高 64 位 `hash64(session_id)`、低 64 位 seq，`to_key()` 大端字节序保证
/// 字典序即数值序，同会话内自然序即重放序（delta 占 seq 产生库内空洞，排序
/// 键语义合法）。`session_id` 另立非唯一二级索引保查询形态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 8, version = 1, with = SerdeJsonCodec)]
#[native_db]
pub struct SessionEventRecord {
    /// 复合键打包：`(hash64(session_id) as u128) << 64 | seq`
    ///
    /// serde 定制为十六进制字符串：serde_json 无 u128 支持且打包值必超
    /// u64 上界（信封 API 要把记录转 JSON），字符串形态保住 JSON 可表达性。
    #[primary_key]
    #[serde(with = "event_key_serde")]
    pub event_key: u128,
    /// 所属会话 id（非唯一二级索引，重放查询入口）
    #[secondary_key]
    pub session_id: String,
    /// 密封事件载荷（嵌装 core 纯类型，含 `Raw` 逃生舱；增量永不见）
    pub event: AgentEvent,
}

impl SessionEventRecord {
    /// 由会话 id 与事件构造记录：`event_key` 打包自 `hash64(session_id)` +
    /// `event.seq`。
    pub fn new(session_id: &str, event: AgentEvent) -> Self {
        Self {
            event_key: pack_session_event_key(session_id, event.seq),
            session_id: session_id.to_owned(),
            event,
        }
    }

    /// 所属会话 id（重放扫描方免解载荷）。
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 事件序号（同会话内单调递增；打包键低 64 位，与 `event.seq` 同源）。
    pub fn seq(&self) -> u64 {
        self.event.seq
    }
}

// --- agent 管理记录（user 维度，全局库 desktop-global.redb）----------------

/// provider 三档模型档位（agent 管理域，纯嵌套 struct 不落独立模型——嵌装
/// 先例同转录单表的 `AgentEvent`）：high / medium / low 三档
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

/// 复合键打包：高 64 位 `hash64(session_id)`、低 64 位 seq。store 内唯一
/// 组装点。
pub(crate) fn pack_session_event_key(session_id: &str, seq: u64) -> u128 {
    ((session_key_hash(session_id) as u128) << 64) | (seq as u128)
}

/// 会话键 64 位哈希：SHA-256(session_id UTF-8 字节) 前 8 字节大端 u64
/// （sha2 既有依赖复用，与 workspace 库文件名哈希成分同源同族）。碰撞域
/// 2^-64，同库会话量级下不可达；打包键内同会话高 64 位恒一致。
fn session_key_hash(session_id: &str) -> u64 {
    let digest = Sha256::digest(session_id.as_bytes());
    u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 摘要前 8 字节定长"))
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
/// 仅转录单表 `SessionEventRecord` 使用——serde flatten 要求自描述编码（bincode 不
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
