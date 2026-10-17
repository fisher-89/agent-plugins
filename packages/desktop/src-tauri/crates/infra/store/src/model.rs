use std::path::Path;

use agent::{AgentEvent, AgentPermissionMode, AgentRunStatus};
use native_db::{native_db, ToKey};
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    ChangeStatus, RunEdgeEntry, RunEdgeKind, RunNodeEntry, RunStartCommand, RunStatus, RunStepKind,
    RunStepStatus, StepCommand, StepKind,
};

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

/// explore 清单记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 4, version = 4, from = ExploreRecordV3)]
#[native_db]
pub struct ExploreRecord {
    /// 记录 id（主键，写事务内 max+1 分配；身份与文件名解耦）
    #[primary_key]
    pub id: i64,
    /// workspace 归属（canonical root，与 `WorkspaceRecord.root` 同口径）
    pub root: String,
    /// 笔记文件 stem 寻址键（kebab-case 口径）
    pub name: String,
    /// 人类可读标题，恒非空（创建与升级默认 = `name`，agent 产出笔记首行
    /// `# <标题>` 后经 `set_explore_title` 回填覆盖）
    pub title: String,
    /// 工作流类型（合法值 `requirement` | `test-only`；创建与升级默认 =
    /// `requirement`，agent 推算类型经 `set_explore_workflow_type` 回填覆盖）
    pub workflow_type: String,
    /// 指向已 promote 的 **change id** 身份锚；`None` = 草稿态
    pub promoted_to: Option<String>,
    /// 归属 agent 会话 id（建档同事务预建空会话并回写；存量记录经
    /// `ensure_explore_session` 归位补齐；None = 尚无会话归属）
    pub session_id: Option<String>,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒）
    pub updated_at: i64,
}

impl ExploreRecord {
    pub fn new(root: &str, name: &str, now: i64) -> Self {
        Self {
            id: 0,
            root: root.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            promoted_to: None,
            session_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// `ExploreRecord` 的 version 1 历史形态（仅作 native_model 升级链的解码
/// 目标，不注册进库模型组、不出公共查询面）：无 `title` / `promoted_to` /
/// `workflow_type` 列。存量 v1 行经版本机制自动升级（零迁移代码路径）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 4, version = 1)]
pub(crate) struct ExploreRecordV1 {
    /// 记录 id（主键）
    pub id: i64,
    /// workspace 归属（canonical root）
    pub root: String,
    /// 展示名（= 笔记文件 stem，磁盘寻址键）
    pub name: String,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒）
    pub updated_at: i64,
}

/// `ExploreRecord` 的 version 2 历史形态（同 v1，仅作升级链中转的解码目标，
/// 不注册进库模型组、不出公共查询面）：有 `title` / `promoted_to`，无
/// `workflow_type` 列。存量 v2 行经版本机制自动升级（`workflow_type` 读出默认
/// `"requirement"`，零迁移代码路径）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 4, version = 2, from = ExploreRecordV1)]
pub(crate) struct ExploreRecordV2 {
    /// 记录 id（主键）
    pub id: i64,
    /// workspace 归属（canonical root）
    pub root: String,
    /// 笔记文件 stem 寻址键
    pub name: String,
    /// 人类可读标题
    pub title: String,
    /// 指向已 promote 的 change id 身份锚
    pub promoted_to: Option<String>,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒）
    pub updated_at: i64,
}

/// v1 → v2 升级半边（原 v1 → 当前形态升级语义平移：缺列读兼容）。
impl From<ExploreRecordV1> for ExploreRecordV2 {
    fn from(previous: ExploreRecordV1) -> Self {
        let name = previous.name;
        Self {
            id: previous.id,
            root: previous.root,
            // 缺列读兼容：旧记录无标题列，展示语义与现状一致（title = name）
            title: name.clone(),
            promoted_to: None,
            name,
            created_at: previous.created_at,
            updated_at: previous.updated_at,
        }
    }
}

/// v2 → v1 降级半边（native_model `from` 属性要求双向 `From`；运行时无降级读取
/// 路径，v2 新增字段丢弃——只保升级语义真实性，降级形态不作数据承诺）。
impl From<ExploreRecordV2> for ExploreRecordV1 {
    fn from(record: ExploreRecordV2) -> Self {
        Self {
            id: record.id,
            root: record.root,
            name: record.name,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

/// v2 → v3 升级半边：新增列读出默认 `"requirement"`（缺列读兼容——存量记录
/// 自建档起恒 requirement 工作流），其余直透。
impl From<ExploreRecordV2> for ExploreRecordV3 {
    fn from(previous: ExploreRecordV2) -> Self {
        Self {
            id: previous.id,
            root: previous.root,
            name: previous.name,
            title: previous.title,
            // 缺列读兼容：旧记录无工作流类型列，缺省语义 = requirement
            workflow_type: "requirement".to_owned(),
            promoted_to: previous.promoted_to,
            created_at: previous.created_at,
            updated_at: previous.updated_at,
        }
    }
}

/// v3 → v2 降级半边（新字段丢弃——只保升级语义真实性）。
impl From<ExploreRecordV3> for ExploreRecordV2 {
    fn from(record: ExploreRecordV3) -> Self {
        Self {
            id: record.id,
            root: record.root,
            name: record.name,
            title: record.title,
            promoted_to: record.promoted_to,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

/// `ExploreRecord` 的 version 3 历史形态（同 v2+`workflow_type`，仅作升级链
/// 中转的解码目标，不注册进库模型组、不出公共查询面）：有 `title` /
/// `promoted_to` / `workflow_type`，无 `session_id`。存量 v3 行经版本机制
/// 自动升级（`session_id` 读出 `None`，零迁移代码路径）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 4, version = 3, from = ExploreRecordV2)]
pub(crate) struct ExploreRecordV3 {
    /// 记录 id（主键）
    pub id: i64,
    /// workspace 归属（canonical root）
    pub root: String,
    /// 笔记文件 stem 寻址键
    pub name: String,
    /// 人类可读标题
    pub title: String,
    /// 工作流类型
    pub workflow_type: String,
    /// 指向已 promote 的 change id 身份锚
    pub promoted_to: Option<String>,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒）
    pub updated_at: i64,
}

/// v3 → v4 升级半边：新增 `session_id` 列读出默认 `None`（缺列读兼容）。
impl From<ExploreRecordV3> for ExploreRecord {
    fn from(previous: ExploreRecordV3) -> Self {
        Self {
            id: previous.id,
            root: previous.root,
            name: previous.name,
            title: previous.title,
            workflow_type: previous.workflow_type,
            promoted_to: previous.promoted_to,
            // 缺列读兼容：旧记录无会话 id 列
            session_id: None,
            created_at: previous.created_at,
            updated_at: previous.updated_at,
        }
    }
}

/// v4 → v3 降级半边（`session_id` 丢弃——只保升级语义真实性）。
impl From<ExploreRecord> for ExploreRecordV3 {
    fn from(record: ExploreRecord) -> Self {
        Self {
            id: record.id,
            root: record.root,
            name: record.name,
            title: record.title,
            workflow_type: record.workflow_type,
            promoted_to: record.promoted_to,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

/// agent 会话记录（version 2：`config_snapshot` 增实例外键 `agent_instance_id`，
/// 经 `SessionRecordV1` 版本链自动升级——旧快照读出 `None`，其余字段原样，
/// 零迁移代码路径）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 7, version = 2, from = SessionRecordV1)]
#[native_db]
pub struct SessionRecord {
    /// core 铸会话 id（字符串主键，`ses-` 前缀铸造格式由 core 单点承载）
    #[primary_key]
    pub id: String,
    /// 引擎侧会话标识（双 id 映射落库半边；未上报为 None）
    pub engine_session_id: Option<String>,
    /// 装配配置快照（engine / 实例外键 / model / permission；快照非引用）
    pub config_snapshot: SessionConfigSnapshot,
    /// 来源受控字符串（debug | explore | …），缺省 debug
    pub source: String,
    /// 来源内定位
    pub source_ref: Option<String>,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒，轮事件绑定引擎标识时刷新）
    pub updated_at: i64,
}

/// 装配配置快照（store 本地定型，嵌套 struct 不落独立模型——嵌装先例同
/// 转录单表的 `AgentEvent`）：engine / 实例外键 / model / permission 四面。
/// 快照是记录自证——续会话引擎路由凭此比对（core 契约不解释装配概念，
/// core 侧以 serde_json::Value 不透明承载）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfigSnapshot {
    /// 引擎二值（cli / sdk，store 本地枚举同 [`AgentEngineKind`]；`cli` 为
    /// 退役态历史事实，字段保留不删）
    pub engine: AgentEngineKind,
    /// 装配时点实例 id 的快照复制值（会话 ↔ 实例的显式外键，多 agent 时代的
    /// 过滤 / 聚合锚）：复制值非跨库引用（`AgentInstanceRecord` 在全局库、
    /// session 在 workspace 库，MUST NOT 引申为跨库 join 或存在性约束）；
    /// `None` = 存量行缺列（v1 → v2 升级链读出）
    #[serde(default)]
    pub agent_instance_id: Option<i64>,
    /// 模型标识（无模型装配概念的引擎为 None）
    pub model: Option<String>,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
}

/// `SessionConfigSnapshot` 的旧三字段形态（仅作 `SessionRecordV1` 升级链的
/// 解码目标，不注册进库模型组、不出公共查询面）：bincode 嵌套结构变布局必须
/// 整结构换型（serde `default` 不救 bincode 缺字段），故旧快照需独立 V1 形态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionConfigSnapshotV1 {
    /// 引擎二值
    pub engine: AgentEngineKind,
    /// 模型标识
    pub model: Option<String>,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
}

/// v1 → v2 升级半边（缺列读兼容：旧快照无实例外键列）。
impl From<SessionConfigSnapshotV1> for SessionConfigSnapshot {
    fn from(previous: SessionConfigSnapshotV1) -> Self {
        Self {
            engine: previous.engine,
            agent_instance_id: None,
            model: previous.model,
            permission_mode: previous.permission_mode,
        }
    }
}

/// 降级半边（native_model `from` 属性要求双向 `From`；运行时无降级读取路径，
/// `agent_instance_id` 丢弃——只保升级语义真实性，降级形态不作数据承诺）。
impl From<SessionConfigSnapshot> for SessionConfigSnapshotV1 {
    fn from(snapshot: SessionConfigSnapshot) -> Self {
        Self {
            engine: snapshot.engine,
            model: snapshot.model,
            permission_mode: snapshot.permission_mode,
        }
    }
}

/// `SessionRecord` 的 version 1 历史形态（仅作 native_model 升级链的解码
/// 目标，不注册进库模型组、不出公共查询面）：`config_snapshot` 为旧三字段
/// 快照形态。存量 v1 行经版本机制自动升级（`agent_instance_id` 读出 `None`、
/// 其余字段原样，零迁移代码路径）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 7, version = 1)]
pub(crate) struct SessionRecordV1 {
    /// core 铸会话 id（字符串主键）
    pub id: String,
    /// 引擎侧会话标识（双 id 映射落库半边；未上报为 None）
    pub engine_session_id: Option<String>,
    /// 装配配置快照（旧三字段形态）
    pub config_snapshot: SessionConfigSnapshotV1,
    /// 来源受控字符串（debug | explore | …），缺省 debug
    pub source: String,
    /// 来源内定位（explore 指向探索记录主键的十进制串；调试会话为 None）
    pub source_ref: Option<String>,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// 最近更新时间（UTC unix 毫秒）
    pub updated_at: i64,
}

/// v1 → v2 升级半边（缺列读兼容：旧快照 `agent_instance_id` 读出 `None`，
/// 其余字段原样平移）。
impl From<SessionRecordV1> for SessionRecord {
    fn from(previous: SessionRecordV1) -> Self {
        Self {
            id: previous.id,
            engine_session_id: previous.engine_session_id,
            config_snapshot: previous.config_snapshot.into(),
            source: previous.source,
            source_ref: previous.source_ref,
            created_at: previous.created_at,
            updated_at: previous.updated_at,
        }
    }
}

/// 降级半边（native_model `from` 属性要求双向 `From`；运行时无降级读取
/// 路径，`agent_instance_id` 随嵌套快照降级丢弃——只保升级语义真实性，
/// 降级形态不作数据承诺）。
impl From<SessionRecord> for SessionRecordV1 {
    fn from(record: SessionRecord) -> Self {
        Self {
            id: record.id,
            engine_session_id: record.engine_session_id,
            config_snapshot: record.config_snapshot.into(),
            source: record.source,
            source_ref: record.source_ref,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

/// agent 会话转录单表
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

// --- change 流程状态记录（workspace 维度，落所属 workspace 库）--------------

/// change 建档记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 9, version = 4, from = ChangeRecordV3)]
#[native_db]
pub struct ChangeRecord {
    /// 建档铸出的稳定唯一身份锚（主键，UUID 形态；一切寻址键——库查询 /
    /// 命令面 / 注册表 / provenance；终身恒定，MUST NOT 复用）
    #[primary_key]
    pub id: String,
    /// change 名（恒裸名；可变属性，无唯一约束，MUST NOT 作身份键）
    pub name: String,
    /// 人类可读标题，恒非空（创建与升级默认 = `name`，promote 路径继承
    /// `explore.title`）
    pub title: String,
    /// 工作流类型（V1 恒 `requirement`，相位表键）
    pub workflow_type: String,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// change 状态（active | archived）
    pub status: ChangeStatus,
    /// 归档时间（UTC unix 毫秒；active 恒 None）
    #[serde(default)]
    pub archived_at: Option<i64>,
    /// 运行中 phase（开相在位、落账清位；嵌套 struct 不落独立模型，先例
    /// `SessionConfigSnapshot`）
    #[serde(default)]
    pub active_phase: Option<ChangeActivePhase>,
    /// 该 change 分配的 worktree 绝对路径（执行锚）；`None` = legacy 主 root
    /// change。执行锚引用，MUST NOT 反向参与库身份派生（`for_root` 恒以
    /// workspace root 为锚）。
    #[serde(default)]
    pub worktree: Option<String>,
    /// 创建基线 fork 点（主仓 HEAD，git worktree 建域时铸出）；调试 / UI 价值。
    #[serde(default)]
    pub base_commit: Option<String>,
}

impl ChangeRecord {
    pub fn new(
        id: &str,
        name: &str,
        workflow_type: &str,
        created_at: i64,
        worktree: Option<String>,
        base_commit: Option<String>,
    ) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: workflow_type.to_owned(),
            created_at,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree,
            base_commit,
        }
    }
}

/// `ChangeRecord` 的 version 3 历史形态（仅作 native_model 升级链的解码
/// 目标，不注册进库模型组、不出公共查询面）：id 主键 + name 属性形态，无
/// `title` 列。存量 v3 行经版本机制自动升级（零迁移代码路径）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 9, version = 3)]
pub(crate) struct ChangeRecordV3 {
    /// 建档铸出的稳定唯一身份锚（主键）
    pub id: String,
    /// change 名（恒裸名）
    pub name: String,
    /// 工作流类型
    pub workflow_type: String,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// change 状态
    pub status: ChangeStatus,
    /// 归档时间（UTC unix 毫秒）
    #[serde(default)]
    pub archived_at: Option<i64>,
    /// 运行中 phase
    #[serde(default)]
    pub active_phase: Option<ChangeActivePhase>,
    /// worktree 绝对路径
    #[serde(default)]
    pub worktree: Option<String>,
    /// 创建基线 fork 点
    #[serde(default)]
    pub base_commit: Option<String>,
}

impl From<ChangeRecordV3> for ChangeRecord {
    fn from(previous: ChangeRecordV3) -> Self {
        let name = previous.name;
        Self {
            id: previous.id,
            // 缺列读兼容：旧记录无标题列，展示语义与现状一致（title = name）
            title: name.clone(),
            name,
            workflow_type: previous.workflow_type,
            created_at: previous.created_at,
            status: previous.status,
            archived_at: previous.archived_at,
            active_phase: previous.active_phase,
            worktree: previous.worktree,
            base_commit: previous.base_commit,
        }
    }
}

/// 降级半边（native_model `from` 属性要求双向 `From`；运行时无降级读取路径，
/// `title` 列丢弃——只保升级语义真实性，降级形态不作数据承诺）。
impl From<ChangeRecord> for ChangeRecordV3 {
    fn from(record: ChangeRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            workflow_type: record.workflow_type,
            created_at: record.created_at,
            status: record.status,
            archived_at: record.archived_at,
            active_phase: record.active_phase,
            worktree: record.worktree,
            base_commit: record.base_commit,
        }
    }
}

/// workspace 库格式版本标记记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 15, version = 1)]
#[native_db]
pub(crate) struct StoreMetaRecord {
    /// 固定单键（恒 `"format"`）
    #[primary_key]
    pub key: String,
    /// 库级格式版本（当前写 [`WORKSPACE_STORE_FORMAT_VERSION`]）
    pub format_version: u32,
}

/// workspace 库格式版本常量（[`StoreMetaRecord`] 固定单键 `"format"` 的写入
/// 值）：1 = name 主键形态时代（实机从未写标记，探测以「缺失」判定）；2 =
/// change 身份锚 id 形态；3 = run 执行图节点 / 边模型（旧步史模型退役，
/// 节点 / 边两模型取代——旧形态行经版本 bump 整体作废重建，零迁移链）。库打开
/// 探测规则：标记缺失 / 低于当前 → 旧库整体作废重建（见 `Store::open_workspace`）。
pub(crate) const WORKSPACE_STORE_FORMAT_VERSION: u32 = 3;

/// workspace 库格式版本标记的固定单键。
pub(crate) const WORKSPACE_STORE_FORMAT_KEY: &str = "format";

/// 运行中 phase 快照（`ChangeRecord` 嵌套结构）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeActivePhase {
    pub phase: String,
    pub attempt: u32,
    /// 开相时刻（UTC unix 毫秒）
    pub start_at: i64,
}

/// 相位评估条目记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 10, version = 2)]
#[native_db]
pub struct PhaseRecord {
    /// 条目 id（主键，写事务内 max+1 分配）
    #[primary_key]
    pub id: i64,
    /// 所属 change id（非唯一二级索引，指向 `ChangeRecord.id`；归属键换锚
    /// 经版本段演进：10:v1 → 10:v2，物理表 `10_1_change` → `10_2_change_id`）
    #[secondary_key]
    pub change_id: String,
    pub phase: String,
    pub attempt: u32,
    /// 评估 verdict（pass | fail，core 域枚举直用）
    pub verdict: Verdict,
    /// 评估报告（内联 ≤2000 字，写面校验）
    pub report: String,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default)]
    pub stale: bool,
    /// 回跳目标（backtrack 写面在该相位最新条目定点标记）
    #[serde(default)]
    pub backtrack_to: Option<String>,
    #[serde(default)]
    pub backtrack_reason: Option<String>,
    /// 会话槽位三列（与 `SessionRecord` 同库 join；缺省落账恒 None）
    #[serde(default)]
    pub executor_session_id: Option<String>,
    #[serde(default)]
    pub evaluator_session_id: Option<String>,
    #[serde(default)]
    pub decision_session_id: Option<String>,
    /// 开相时刻（UTC unix 毫秒；缺省 None）
    #[serde(default)]
    pub start_at: Option<i64>,
    /// 落账时刻（UTC unix 毫秒）
    pub timestamp: i64,
}

/// checklist 检查项子行
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 11, version = 1)]
#[native_db]
pub struct ChecklistItemRecord {
    /// 复合键打包：`(phase_id as u128) << 64 | item_index`
    ///
    /// serde 定制为十六进制字符串：serde_json 无 u128 数字面（信封 API 要把
    /// 记录转 JSON），字符串形态保住 JSON 可表达性（`event_key_serde` 先例
    /// 复用）。
    #[primary_key]
    #[serde(with = "event_key_serde")]
    pub item_key: u128,
    /// 所属相位条目 id（非唯一二级索引，子表重组查询入口）
    #[secondary_key]
    pub phase_id: i64,
    pub item: String,
    pub pass: bool,
    pub evidence: String,
}

impl ChecklistItemRecord {
    /// 由所属相位条目 id 与检查项构造记录：`item_key` 打包自 phase_id +
    /// item_index（evaluator 输出序）。
    pub fn new(phase_id: i64, item_index: u32, item: ChecklistItem) -> Self {
        Self {
            item_key: pack_checklist_item_key(phase_id, item_index),
            phase_id,
            item: item.item,
            pass: item.pass,
            evidence: item.evidence,
        }
    }
}

/// 复合键打包：高 64 位 phase_id、低 64 位 item_index。store 内唯一组装点。
pub(crate) fn pack_checklist_item_key(phase_id: i64, item_index: u32) -> u128 {
    ((phase_id as u128) << 64) | (item_index as u128)
}

/// 相位机步骤审计行（desktop-change-state-store 四模型之四）：编排八臂命令
/// 包络逐条落行，`run_id` 串链同 run 步骤序列。审计 only——MUST NOT 作为
/// run 恢复依据（run 状态驻内存 ChangeFlowControl，续走由 phase_next 依
/// eval 历史重算）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 12, version = 2)]
#[native_db]
pub struct StepRecord {
    /// 行 id（主键，写事务内 max+1 分配）
    #[primary_key]
    pub id: i64,
    /// 所属 change id（非唯一二级索引，指向 `ChangeRecord.id`；归属键换锚经
    /// 版本段演进：12:v1 → 12:v2，物理表 `12_1_change` → `12_2_change_id`）
    #[secondary_key]
    pub change_id: String,
    /// 所属 run（`run-<millis>` 铸造标识，同 run 步骤串链键）
    pub run_id: String,
    /// 步骤种类（封闭集八值，core 域枚举直用）
    pub step_kind: StepKind,
    /// 步终态（线格式词：ok | error）
    pub status: String,
    /// 落行时刻（UTC unix 毫秒）
    pub timestamp: i64,
    /// 有界输出摘要（≤500 字截断留痕，编排侧截断）
    pub summary: String,
    /// 全量输出引用（checks 报告目录 / 会话 id）
    #[serde(default)]
    pub reference: Option<String>,
}

impl StepRecord {
    /// 由审计载荷构造新记录：`id` 置 0（写事务内 max+1 分配覆盖）。
    pub fn new(command: &StepCommand) -> Self {
        Self {
            id: 0,
            change_id: command.change_id.clone(),
            run_id: command.run_id.clone(),
            step_kind: command.step_kind,
            status: command.status.clone(),
            timestamp: command.timestamp,
            summary: command.summary.clone(),
            reference: command.reference.clone(),
        }
    }
}

// --- run 运行史记录（unify-run-state-persistence：决策翻案「不建 flow_runs
// 表」，run 运行史落库两表；与 StepRecord 审计职责分立）---------------------

/// run 运行史主行
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 13, version = 2)]
#[native_db]
pub struct RunRecord {
    /// run id（主键，walker `run-<millis>` 铸造标识）
    #[primary_key]
    pub run_id: String,
    /// 所属 change id（非唯一二级索引，指向 `ChangeRecord.id`；归属键换锚经
    /// 版本段演进：13:v1 → 13:v2，物理表 `13_1_change` → `13_2_change_id`）
    #[secondary_key]
    pub change_id: String,
    /// run 状态（running | completed | stopped | failed | interrupted）
    pub status: RunStatus,
    /// 终态记因 / 标定记因（running 恒 None）
    #[serde(default)]
    pub reason: Option<String>,
    /// 发起时刻（UTC unix 毫秒，run_start 命令携带）
    pub started_at: i64,
    /// 收口 / 标定时刻（UTC unix 毫秒；running 恒 None）
    #[serde(default)]
    pub finished_at: Option<i64>,
}

impl RunRecord {
    /// 由发起命令构造新记录（running 起步、无 reason、无收口时刻）。
    pub fn new(command: &RunStartCommand) -> Self {
        Self {
            run_id: command.run_id.clone(),
            change_id: command.change_id.clone(),
            status: RunStatus::Running,
            reason: None,
            started_at: command.started_at,
            finished_at: None,
        }
    }
}

/// run 执行图节点行
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 16, version = 1)]
#[native_db]
pub struct RunNodeRecord {
    /// 复合键打包：`(hash64(run_id) as u128) << 64 | node_seq`
    ///
    /// serde 定制为十六进制字符串（`event_key_serde` 先例复用——serde_json 无
    /// u128 数字面）。
    #[primary_key]
    #[serde(with = "event_key_serde")]
    pub node_key: u128,
    /// 所属 run id（非唯一二级索引，run 图史重组查询入口）
    #[secondary_key]
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    pub phase: String,
    pub attempt: u32,
    /// 步词汇（二十一值全词汇封闭集，core 域枚举直用）
    pub step: RunStepKind,
    /// 同 `(phase, attempt, step)` 内的执行实例号（feedback 重跑为递增新节点）
    pub instance_no: u32,
    /// 步状态（running | passed | failed | stopped；收口在途步可留 running）
    pub status: RunStepStatus,
    /// WorkerAgent 步所属会话 id（工具步为 None）
    #[serde(default)]
    pub session_id: Option<String>,
    /// 人读记因 / 摘要（有界，写面截断同 diagnose_brief 口径）
    #[serde(default)]
    pub detail: Option<String>,
    /// 全量输出引用（`ExecOutput::Recorded { exec_id }`；MUST NOT 全文进本行）
    #[serde(default)]
    pub output_ref: Option<String>,
    /// 步开始时刻（UTC unix 毫秒）。旧库行缺省 0 = 步级时间缺席（前端判定收
    /// 格式化模块单点）；步级时间独立成列，MUST NOT 顶替不复用 `timestamp`
    #[serde(default)]
    pub started_at: i64,
    /// 步收口时刻（UTC unix 毫秒；running 收口在途步 None）
    #[serde(default)]
    pub ended_at: Option<i64>,
    /// 本步 turn token 增量（`TurnDone.usage` 求和；仅 WorkerAgent 步终态有值）
    #[serde(default)]
    pub token_input: Option<u64>,
    /// 本步 turn token 增量（输出向；口径同 `token_input`）
    #[serde(default)]
    pub token_output: Option<u64>,
    /// 落包时刻（UTC unix 毫秒，= finished_at 整包同刻）
    pub timestamp: i64,
}

impl RunNodeRecord {
    pub fn new(
        run_id: &str,
        change_id: &str,
        node_seq: u64,
        entry: &RunNodeEntry,
        timestamp: i64,
    ) -> Self {
        Self {
            node_key: pack_run_seq_key(run_id, node_seq),
            run_id: run_id.to_owned(),
            change_id: change_id.to_owned(),
            phase: entry.phase.clone(),
            attempt: entry.attempt,
            step: entry.step,
            instance_no: entry.instance_no,
            status: entry.status,
            session_id: entry.session_id.clone(),
            detail: entry.detail.clone(),
            output_ref: entry.output_ref.clone(),
            started_at: entry.started_at,
            ended_at: entry.ended_at,
            token_input: entry.token_input,
            token_output: entry.token_output,
            timestamp,
        }
    }

    /// emit 序号（打包键低 64 位 = 整包下标，与 `node_key` 低半同源）。
    pub fn node_seq(&self) -> u64 {
        self.node_key as u64
    }

    /// 节点身份串（边表端点口径）：`"{phase}:{attempt}:{step}:{instance_no}"`。
    pub fn node_id(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.phase,
            self.attempt,
            self.step.as_str(),
            self.instance_no
        )
    }
}

/// run 执行图边行（workspace 维度，落所属 workspace 库）：run 收口边整包的落行
/// 形态；`edge_kind` 四值封闭集（`RunEdgeKind`：forward / feedback / retry /
/// backtrack），端点恒为节点身份串（`RunNodeRecord::node_id()` 组装口径）。
///
/// 主键为合成 u128 打包键（同节点行先例）：高 64 位 `hash64(run_id)`、低 64 位
/// edge_seq（run 内 emit 序）。两二级索引：`run_id`（按 run 全量读边）与
/// `from_node_key`（按源节点读后继）；`timestamp` 同整包刻。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 17, version = 1)]
#[native_db]
pub struct RunEdgeRecord {
    /// 复合键打包：`(hash64(run_id) as u128) << 64 | edge_seq`
    #[primary_key]
    #[serde(with = "event_key_serde")]
    pub edge_key: u128,
    /// 所属 run id（非唯一二级索引，run 全量读边入口）
    #[secondary_key]
    pub run_id: String,
    /// 所属 change id（归属键——一切寻址以 id 为准）
    pub change_id: String,
    /// 源节点身份串（非唯一二级索引，按源节点读后继入口）
    #[secondary_key]
    pub from_node_key: String,
    /// 目标节点身份串
    pub to_node_key: String,
    /// 边 kind（四值封闭集，core 域枚举直用）
    pub edge_kind: RunEdgeKind,
    /// 落包时刻（UTC unix 毫秒，= finished_at 整包同刻）
    pub timestamp: i64,
}

impl RunEdgeRecord {
    /// 由收口整包条目构造记录：`edge_key` 打包自 `hash64(run_id)` + `edge_seq`
    ///（整包下标 = run 内 emit 序）。
    pub fn new(
        run_id: &str,
        change_id: &str,
        edge_seq: u64,
        entry: &RunEdgeEntry,
        timestamp: i64,
    ) -> Self {
        Self {
            edge_key: pack_run_seq_key(run_id, edge_seq),
            run_id: run_id.to_owned(),
            change_id: change_id.to_owned(),
            from_node_key: entry.from_node_key.clone(),
            to_node_key: entry.to_node_key.clone(),
            edge_kind: entry.edge_kind,
            timestamp,
        }
    }

    /// emit 序号（打包键低 64 位 = 整包下标）。
    pub fn edge_seq(&self) -> u64 {
        self.edge_key as u64
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
    pub high: String,
    /// medium 档模型标识（本期只存不选）
    pub medium: String,
    pub low: String,
}

/// 「CLI 租户已下线」共享拒收文案（design DD3）：`runner_for` /
/// `resolve_agent_engine` / `upsert_agent_instance` /
/// `set_default_agent_instance` 四拒点逐字复用、不加前后缀——同一产品事实
/// 同一文案，防多处措辞漂移；常量落 store（依赖方向 agent-runtime → store，
/// store 不反向取词）。文案含退役事实 + 管理页引导 + 改默认引导。
pub const CLI_TENANT_RETIRED: &str = "CLI 租户已下线：请前往 Agent 管理页（/agents）新建或改用 sdk agent，并将其设为默认（存量 cli 实例可读可删不可改）";

/// agent 实例引擎二值（agent 管理域，store 本地枚举）：serde camelCase 线值
/// `"cli" | "sdk"` 与门面 `EngineKind` 同线值域。持久化的是配置值域而非引擎
/// 实现——store 禁拖引擎门面 crate（零新增依赖），`AgentEngineKind` →
/// `EngineKind` 映射收运行发起解析单点（两臂 match）。**变体集与 bincode
/// 序号（`Cli`=0 / `Sdk`=1）为解码锚，MUST NOT 增删重排**（删/重排即
/// `AgentInstanceRecord` + `SessionRecord` 两表报废含 sdk 数据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentEngineKind {
    /// 本机 claude CLI 租户（解码兼容，不再可选：写入面与运行发起解析均拒，
    /// 存量行只读；bincode 序号 0 永久保留为历史数据解码锚）
    Cli,
    /// 进程内 sdk 租户（唯一活档）
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

/// `AgentProviderRecord` 的 version 1 历史形态（仅作 native_model 升级链的
/// 解码目标，不注册进库模型组）：无 `context_length` 列。存量 v1 行经版本
/// 机制自动升级为 v2（`context_length = None` 未配置语义，零迁移代码路径）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 5, version = 1)]
pub(crate) struct AgentProviderRecordV1 {
    /// 记录 id（主键）
    pub id: i64,
    /// 展示名
    pub name: String,
    /// openai 兼容端点 base_url
    pub base_url: String,
    /// 认证凭据（明文）
    pub api_key: String,
    /// 三档模型标识
    pub models: AgentModelTiers,
}

impl From<AgentProviderRecordV1> for AgentProviderRecord {
    fn from(previous: AgentProviderRecordV1) -> Self {
        Self {
            id: previous.id,
            name: previous.name,
            base_url: previous.base_url,
            api_key: previous.api_key,
            models: previous.models,
            // 缺列读兼容：旧记录无窗长配置，未配置语义入列
            context_length: None,
        }
    }
}

/// 降级半边（native_model `from` 属性要求双向 `From`；运行时无降级读取路径，
/// `context_length` 以缺省占位——只保升级语义真实性，降级形态不作数据承诺）。
impl From<AgentProviderRecord> for AgentProviderRecordV1 {
    fn from(record: AgentProviderRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            base_url: record.base_url,
            api_key: record.api_key,
            models: record.models,
        }
    }
}

/// agent provider 记录
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 5, version = 2, from = AgentProviderRecordV1)]
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
    /// provider 上下文窗长（token 数；sdk 引擎上下文防线消费）：可空列，
    /// `None` = 未配置（缺省走 128K 启发式；编辑语义留空 = 未配置，MUST NOT
    /// 落 0 或缺省字面）
    #[serde(default)]
    pub context_length: Option<u64>,
}

impl AgentProviderRecord {
    /// 由连接档案构造新记录：`id` 置 0（写事务内 max+1 分配覆盖）；api_key
    /// 传空即空（新建语义无原值可保）；`context_length` 传 `None` 即未配置。
    pub fn new(
        name: String,
        base_url: String,
        api_key: String,
        models: AgentModelTiers,
        context_length: Option<u64>,
    ) -> Self {
        Self {
            id: 0,
            name,
            base_url,
            api_key,
            models,
            context_length,
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
            .field("context_length", &self.context_length)
            .finish()
    }
}

/// agent 实例记录（agent 管理域，user 维度落全局库）：引擎选择（线值
/// `cli` | `sdk` 双值保留，写入面仅收 `sdk`——存量 cli 行可读可删不可改）
/// 与 provider 引用（`sdk` 必填且引用存在；「cli 可空」语义随 CLI 租户退役，
/// 存量 cli 行 provider 可空只读保留）+ 默认标记（全局恒至多一；写入只发生
/// 在 store 的 `set_default_agent_instance` 与删除清标记两处，upsert 不参与
/// 写；默认标记仅可落 sdk 实例）。
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
    /// 引用 provider id（同库 [`AgentProviderRecord`] 主键；`sdk` 必填且引用
    /// 存在——`sdk` 唯一活档，存量 `cli` 行可空只读保留）
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

/// 复合键打包：高 64 位 `hash64(run_id)`、低 64 位 run 内 emit 序。store 内唯一
/// 组装点（`pack_session_event_key` 同族；节点行 `node_key` 与边行 `edge_key` 同
/// 式——两键分属不同模型，序域各自独立不冲突）。
pub(crate) fn pack_run_seq_key(run_id: &str, seq: u64) -> u128 {
    ((session_key_hash(run_id) as u128) << 64) | (seq as u128)
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
