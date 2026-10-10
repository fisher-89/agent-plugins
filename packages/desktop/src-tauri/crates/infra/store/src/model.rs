use std::path::Path;

use agent::{AgentEvent, AgentPermissionMode, AgentRunStatus};
use native_db::{native_db, ToKey};
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    ChangeStatus, RunStartCommand, RunStatus, RunStepEntry, RunStepKind, RunStepStatus,
    StepCommand, StepKind,
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

// --- change 流程状态记录（workspace 维度，落所属 workspace 库）--------------

/// change 建档记录的 version 1 历史形态（仅作 native_model 升级链的解码
/// 目标，不注册进库模型组）：无 `worktree` / `base_commit` 列（worktree 之前
/// 的主 root 编辑形态）。存量 v1 行经版本机制自动升级为 v2（两字段 `None` =
/// legacy 主 root 语义，零迁移代码路径；provider `context_length` 先例同模式）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 9, version = 1)]
pub(crate) struct ChangeRecordV1 {
    /// change 名（主键，建档即定；归档改名只动磁盘目录，主键不变）
    pub name: String,
    /// 工作流类型（V1 恒 `requirement`，相位表键）
    pub workflow_type: String,
    /// 建档时间（UTC unix 毫秒）
    pub created_at: i64,
    /// change 状态（active | archived）
    pub status: ChangeStatus,
    /// 归档时间（UTC unix 毫秒；active 恒 None）
    pub archived_at: Option<i64>,
    /// 运行中 phase（开相在位、落账清位）
    pub active_phase: Option<ChangeActivePhase>,
}

/// 升级半边：缺列读兼容（旧记录无 worktree / 基线列，`None` = legacy 主
/// root change 语义）。
impl From<ChangeRecordV1> for ChangeRecord {
    fn from(previous: ChangeRecordV1) -> Self {
        Self {
            name: previous.name,
            workflow_type: previous.workflow_type,
            created_at: previous.created_at,
            status: previous.status,
            archived_at: previous.archived_at,
            active_phase: previous.active_phase,
            // 缺列读兼容：worktree 之前的存量记录读出 None = 主 root 执行
            worktree: None,
            base_commit: None,
        }
    }
}

/// 降级半边（native_model `from` 属性要求双向 `From`；运行时无降级读取路径，
/// 两新列丢弃占位——只保升级语义真实性，降级形态不作数据承诺）。
impl From<ChangeRecord> for ChangeRecordV1 {
    fn from(record: ChangeRecord) -> Self {
        Self {
            name: record.name,
            workflow_type: record.workflow_type,
            created_at: record.created_at,
            status: record.status,
            archived_at: record.archived_at,
            active_phase: record.active_phase,
        }
    }
}

/// change 建档记录（desktop-change-state-store 四模型之一）：change 流程状态
/// 的身份主行——`name` 即 change 名（身份主键，不随归档目录改名变），状态 /
/// 时间戳与运行中 phase 随行。markdown 产物（proposal / design / tasks /
/// specs / reports / explore）留磁盘 change 目录，双载体边界与 `ExploreRecord`
/// 先例同构（记录在 db、产物在磁盘）。
///
/// 时间戳为 UTC unix 毫秒 `i64`，与 `WorkspaceRecord` 同口径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 9, version = 2, from = ChangeRecordV1)]
#[native_db]
pub struct ChangeRecord {
    /// change 名（主键，建档即定；归档改名只动磁盘目录，主键不变）
    #[primary_key]
    pub name: String,
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
    /// change（存量记录升级读出，照旧主 root 执行）。执行锚引用，MUST NOT
    /// 反向参与库身份派生（`for_root` 恒以 workspace root 为锚）。
    #[serde(default)]
    pub worktree: Option<String>,
    /// 创建基线 fork 点（主仓 HEAD，git worktree 建域时铸出）；调试 / UI 价值。
    #[serde(default)]
    pub base_commit: Option<String>,
}

impl ChangeRecord {
    /// 由建档档案构造新记录（`status` 恒 active 起步、无 active_phase；
    /// `worktree` / `base_commit` 建域组合随建档入列，legacy 形态传 `None`）。
    pub fn new(
        name: &str,
        workflow_type: &str,
        created_at: i64,
        worktree: Option<String>,
        base_commit: Option<String>,
    ) -> Self {
        Self {
            name: name.to_owned(),
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

/// 运行中 phase 快照（`ChangeRecord` 嵌套结构）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeActivePhase {
    pub phase: String,
    pub attempt: u32,
    /// 开相时刻（UTC unix 毫秒）
    pub start_at: i64,
}

/// 相位评估条目记录（desktop-change-state-store 四模型之二）：一次评估落账
/// 一行（原 workflow.json `eval[]` 条目的库形态）。checklist 子项不内嵌，落
/// [`ChecklistItemRecord`] 子表（写时机不变——phase_log 单事务内同落，拆存
/// 储不拆原子性）；evidence 长文本走独立版本链。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 10, version = 1)]
#[native_db]
pub struct PhaseRecord {
    /// 条目 id（主键，写事务内 max+1 分配）
    #[primary_key]
    pub id: i64,
    /// 所属 change 名（非唯一二级索引；名称引用非外键约束）
    #[secondary_key]
    pub change: String,
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

/// checklist 检查项子行（desktop-change-state-store 四模型之三）：挂
/// [`PhaseRecord`] 的独立子表，打包主键序即 evaluator 输出序。独立
/// native_model 版本链（evidence 长文本演进与 PhaseRecord 解耦）。
///
/// 主键为合成 u128 打包键（native_db 复合主键不受支持，`SessionEventRecord`
/// 先例同构）：高 64 位 phase_id、低 64 位 item_index，`to_key()` 大端字节序
/// 保证同相位内自然序即 item_index 升序。
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

/// 相位机步骤审计行（desktop-change-state-store 四模型之四）：编排七臂命令
/// 包络逐条落行，`run_id` 串链同 run 步骤序列。审计 only——MUST NOT 作为
/// run 恢复依据（run 状态驻内存 ChangeFlowControl，续走由 phase_next 依
/// eval 历史重算）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 12, version = 1)]
#[native_db]
pub struct StepRecord {
    /// 行 id（主键，写事务内 max+1 分配）
    #[primary_key]
    pub id: i64,
    /// 所属 change 名（非唯一二级索引）
    #[secondary_key]
    pub change: String,
    /// 所属 run（`run-<millis>` 铸造标识，同 run 步骤串链键）
    pub run_id: String,
    /// 步骤种类（封闭集七值，core 域枚举直用）
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
            change: command.change.clone(),
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

/// run 运行史主行（workspace 维度，落所属 workspace 库；unify-run-state-
/// persistence 翻案「不建 flow_runs 表」决策立项）：每 run 一行，发起建
/// running 行（支撑启动标定）、收口终态更新（单事务与步整包同落）。全史保留
/// 不截 last_run（运行史审计面）；attempt 经 `phase_start` max+1 分配跨 run
/// 不撞号，全史叠加在同一列面分层。
///
/// 状态词汇五值：`running` + 终态三值（completed / stopped / failed）+
/// `interrupted`（仅启动标定产生，运行期写路径不产生）；停等两态不落库（应
/// 答通道活在进程内）。时间戳为 UTC unix 毫秒 `i64`，与 `WorkspaceRecord`
/// 同口径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 13, version = 1)]
#[native_db]
pub struct RunRecord {
    /// run id（主键，walker `run-<millis>` 铸造标识）
    #[primary_key]
    pub run_id: String,
    /// 所属 change 名（非唯一二级索引；名称引用非外键约束）
    #[secondary_key]
    pub change: String,
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
            change: command.change.clone(),
            status: RunStatus::Running,
            reason: None,
            started_at: command.started_at,
            finished_at: None,
        }
    }
}

/// run 步节点史行（workspace 维度，落所属 workspace 库）：run 收口步整包的
/// 落行形态，图史面——与 `StepRecord` 审计职责分立（审计 vs 图节点史，词汇
/// 近互补不双写同一语义行；StepRecord 职责与词汇不变）。
///
/// 步词汇为封闭集五值（`RunStepKind`：executor / evaluator / decision /
/// static_check / test_execution）——词汇本体在 workflow::state，store 结构
/// 上收不到忽略集（流程面步骤与三门不可表达，过滤单点在编排侧落库写面）。
///
/// 主键为合成 u128 打包键（native_db 复合主键不受支持，`SessionEventRecord`
/// 先例同构）：高 64 位 `hash64(run_id)`、低 64 位 seq（emit 序），大端字节
/// 序保证同 run 内自然序即 emit 序（被过滤步占号产生的库内空洞为排序键语义
/// 合法形态，先例同源）。`run_id` 另立非唯一二级索引保查询形态。`timestamp`
/// = finish 的 finished_at（整包同刻，corpus 确定性）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[native_model(id = 14, version = 1)]
#[native_db]
pub struct RunStepRecord {
    /// 复合键打包：`(hash64(run_id) as u128) << 64 | seq`
    ///
    /// serde 定制为十六进制字符串（`event_key_serde` 先例复用——serde_json 无
    /// u128 数字面）。
    #[primary_key]
    #[serde(with = "event_key_serde")]
    pub step_key: u128,
    /// 所属 run id（非唯一二级索引，run 史重组查询入口）
    #[secondary_key]
    pub run_id: String,
    pub phase: String,
    pub attempt: u32,
    /// 步词汇（封闭集五值，core 域枚举直用）
    pub step: RunStepKind,
    /// 步状态（running | passed | failed | stopped；收口在途步可留 running）
    pub status: RunStepStatus,
    /// WorkerAgent 步所属会话 id（工具步为 None）
    #[serde(default)]
    pub session_id: Option<String>,
    /// 人读记因 / 摘要（有界，写面截断同 diagnose_brief 口径）
    #[serde(default)]
    pub detail: Option<String>,
    /// 落包时刻（UTC unix 毫秒，= finished_at 整包同刻）
    pub timestamp: i64,
}

impl RunStepRecord {
    /// 由收口整包条目构造记录：`step_key` 打包自 `hash64(run_id)` + `seq`
    ///（emit 序），`timestamp` 随整包统一（= finished_at）。
    pub fn new(run_id: &str, entry: &RunStepEntry, timestamp: i64) -> Self {
        Self {
            step_key: pack_run_step_key(run_id, entry.seq),
            run_id: run_id.to_owned(),
            phase: entry.phase.clone(),
            attempt: entry.attempt,
            step: entry.step,
            status: entry.status,
            session_id: entry.session_id.clone(),
            detail: entry.detail.clone(),
            timestamp,
        }
    }

    /// emit 序号（打包键低 64 位，与 `RunStepEntry.seq` 同源）。
    pub fn seq(&self) -> u64 {
        self.step_key as u64
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

/// agent 实例引擎二值（agent 管理域，store 本地枚举）：serde camelCase 线值
/// `"cli" | "sdk"` 与门面 `EngineKind` 同线值域。持久化的是配置值域而非引擎
/// 实现——store 禁拖引擎门面 crate（零新增依赖），`AgentEngineKind` →
/// `EngineKind` 映射收运行发起解析单点（两臂 match）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentEngineKind {
    /// 本机 claude CLI 租户
    Cli,
    /// 进程内 sdk 租户
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

/// 复合键打包：高 64 位 `hash64(run_id)`、低 64 位 seq（emit 序）。store 内
/// 唯一组装点（`pack_session_event_key` 同族，`RunStepRecord` 打包主键）。
pub(crate) fn pack_run_step_key(run_id: &str, seq: u64) -> u128 {
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
