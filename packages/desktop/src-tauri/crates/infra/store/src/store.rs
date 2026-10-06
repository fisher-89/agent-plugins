use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use agent::{
    AgentEvent, AgentEventKind, SessionProvenance, SessionRow, SessionStats, SessionSummary,
    TurnSummary,
};
use native_db::{Builder, Database, Models};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use specta::Type;

use crate::canonical;
use crate::envelope::{self, ModelInfo, RecordEnvelope};
use crate::model::{
    dir_name, now_millis, AgentEngineKind, AgentInstanceRecord, AgentProviderRecord,
    AgentRunRecord, ChangeActivePhase, ChangeRecord, ChecklistItemRecord, ExploreRecord,
    PhaseRecord, SessionEventRecord, SessionRecord, StepRecord, WorkspaceRecord,
};
use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStatus, PhaseLogCommand, PhaseStartState,
    PhaseStateRecord, StepCommand, StepStateRecord,
};

/// store 内部错误面：四变体对应四类故障模式；`Display` 恒带 `db:` /
/// `canonicalize:` / `conflict:` / `not_found:` 前缀，直接服务「清单丢失」
/// 的可排查性。迁移失败经 [`StoreError::Db`]（`迁移:` 语境前缀）呈现。
/// `Conflict` / `NotFound` 为 change 域操作面新增（唯一性查重与 miss 非幂
/// 等写的类型化呈现，port 适配器据此映射 [`workflow::state::StoreFault`]）。
///
/// 命令层以 `.to_string()` 转换为 `Err(String)`，本类型不进入命令签名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// db 打开 / 事务 / 读写失败
    Db(String),
    /// 路径 canonicalize 失败
    Canonicalize(String),
    /// 唯一性冲突（同名建档 / 重复落账）
    Conflict(String),
    /// 目标记录不存在（miss 非幂等写面）
    NotFound(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(msg) => write!(f, "db: {msg}"),
            Self::Canonicalize(msg) => write!(f, "canonicalize: {msg}"),
            Self::Conflict(msg) => write!(f, "conflict: {msg}"),
            Self::NotFound(msg) => write!(f, "not_found: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// 引擎错误（native_db `db_type::Error` 等 `Display` 错误）统一收敛为
/// [`StoreError::Db`]，错误串携带操作语境。
pub(crate) fn db_err<E: fmt::Display>(context: &str) -> impl Fn(E) -> StoreError + '_ {
    move |e| StoreError::Db(format!("{context}: {e}"))
}

/// 建档记录 → 中性快照（记录 ↔ 中性类型映射单点，design D2）。
fn change_state(record: &ChangeRecord) -> ChangeStateRecord {
    ChangeStateRecord {
        name: record.name.clone(),
        workflow_type: record.workflow_type.clone(),
        created_at: record.created_at,
        status: record.status,
        archived_at: record.archived_at,
        active_phase: record.active_phase.as_ref().map(|active| {
            workflow::state::ActivePhaseState {
                phase: active.phase.clone(),
                attempt: active.attempt,
                start_at: active.start_at,
            }
        }),
    }
}

/// 运行中 phase 中性快照 → 建档记录嵌套结构（映射单点）。
fn change_active_phase_stored(
    active: workflow::state::ActivePhaseState,
) -> ChangeActivePhase {
    ChangeActivePhase {
        phase: active.phase,
        attempt: active.attempt,
        start_at: active.start_at,
    }
}

/// store 记录 → core 会话契约行（快照出线为 JSON 形态，core 不解释）。
fn session_row(record: &SessionRecord) -> Result<SessionRow, String> {
    Ok(SessionRow {
        id: record.id.clone(),
        remote_session_id: record.engine_session_id.clone(),
        config_snapshot: serde_json::to_value(&record.config_snapshot)
            .map_err(|e| format!("会话快照出线失败: {e}"))?,
        provenance: SessionProvenance {
            source: record.source.clone(),
            source_ref: record.source_ref.clone(),
        },
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

/// 轮统计行 → core 轮行 DTO（IPC 面；孤儿行 `session_id` 为 None 落空串，
/// 孤儿行不出任何会话清单）。
fn turn_summary(record: &AgentRunRecord) -> TurnSummary {
    TurnSummary {
        turn_id: record.id,
        session_id: record.session_id.clone().unwrap_or_default(),
        status: record.status,
        started_at: record.started_at,
        finished_at: record.finished_at,
        num_turns: record.num_turns,
        cost_usd: record.cost_usd,
        duration_ms: record.duration_ms,
        error: record.error.clone(),
    }
}

/// 聚合统计现算：轮数自轮行行数、累计墙钟自轮行时长求和、累计 token 自转录
/// `TurnDone.usage` 鸭子类型求和——统计字段缺席合法缺省（降级不违约）。
fn aggregate_stats(
    turn_count: usize,
    turns: &[TurnSummary],
    events: &[AgentEvent],
) -> SessionStats {
    SessionStats {
        turn_count: turn_count as u64,
        total_duration_ms: sum_present(turns.iter().map(|turn| turn.duration_ms)),
        input_tokens: sum_usage_tokens(events, "inputTokens"),
        output_tokens: sum_usage_tokens(events, "outputTokens"),
    }
}

/// 可缺省数值求和：全缺席 → None（缺席合法缺省），部分在场合计在场的值。
fn sum_present(values: impl Iterator<Item = Option<u64>>) -> Option<u64> {
    values.fold(None, |acc, value| match (acc, value) {
        (None, None) => None,
        (acc, value) => Some(acc.unwrap_or(0).saturating_add(value.unwrap_or(0))),
    })
}

/// 转录 `TurnDone.usage` 鸭子类型求和：以数值键（serde camelCase 线值）取
/// token 口径，非对象 usage / 缺键 / 非数值不计——真实 usage 形状首次落库后
/// 复核键集是否需扩展（design 待决问题留痕）。
fn sum_usage_tokens(events: &[AgentEvent], key: &str) -> Option<u64> {
    let mut total: u64 = 0;
    let mut any = false;
    for event in events {
        if let AgentEventKind::TurnDone { usage, .. } = &event.kind {
            if let Some(value) = usage.get(key).and_then(Value::as_u64) {
                total = total.saturating_add(value);
                any = true;
            }
        }
    }
    any.then_some(total)
}

/// 记录名单分量校验（非空、非 `.` / `..`、不含 `/` `\` `:`）：与 workflow 查询
/// 层的 `is_single_component_name` 同口径（store 不依赖 workflow，校验各自
/// 持有、口径一致）。
fn is_single_component_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
}

/// explore 来源受控字符串：与前端 `EXPLORE_SOURCE` 口径一致（两处同字面量，
/// 改动需同步）。`delete_explore_record` 据此圈定级联删除的归属会话。
const EXPLORE_RUN_SOURCE: &str = "explore";

/// 数据维度（信封维度标签 + db 查看命令 scope 入参双职）：`User` 全局库 /
/// `Workspace` workspace 库。serde 线值为 `"user"` / `"workspace"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum DbDimension {
    /// user 维度（全局库，`WorkspaceRecord` 与 agent 管理两模型——user 维度
    /// 已落地代表，desktop-data-dimensions 留痕）
    User,
    /// workspace 维度（per-workspace 库，轮统计行 / 会话 / 转录 / explore /
    /// change 流程状态八模型）
    Workspace,
}

/// 全局库模型组
pub(crate) fn global_models() -> &'static Models {
    static MODELS: OnceLock<Models> = OnceLock::new();
    MODELS.get_or_init(|| {
        let mut models = Models::new();
        models
            .define::<WorkspaceRecord>()
            .expect("定义 WorkspaceRecord 失败");
        models
            .define::<AgentProviderRecord>()
            .expect("定义 AgentProviderRecord 失败");
        models
            .define::<AgentInstanceRecord>()
            .expect("定义 AgentInstanceRecord 失败");
        models
    })
}

/// workspace 库模型组（八模型：轮统计行 / 会话 / 转录 / explore 四既有模型
/// + change 流程状态四模型，additive 注册零迁移）
pub(crate) fn workspace_models() -> &'static Models {
    static MODELS: OnceLock<Models> = OnceLock::new();
    MODELS.get_or_init(|| {
        let mut models = Models::new();
        models
            .define::<AgentRunRecord>()
            .expect("定义 AgentRunRecord 失败");
        models
            .define::<SessionRecord>()
            .expect("定义 SessionRecord 失败");
        models
            .define::<SessionEventRecord>()
            .expect("定义 SessionEventRecord 失败");
        models
            .define::<ExploreRecord>()
            .expect("定义 ExploreRecord 失败");
        models
            .define::<ChangeRecord>()
            .expect("定义 ChangeRecord 失败");
        models
            .define::<PhaseRecord>()
            .expect("定义 PhaseRecord 失败");
        models
            .define::<ChecklistItemRecord>()
            .expect("定义 ChecklistItemRecord 失败");
        models
            .define::<StepRecord>()
            .expect("定义 StepRecord 失败");
        models
    })
}

/// 全局库文件名（全局数据目录根直下；与旧单库 `desktop-store.redb` 换名使
/// 新旧布局文件面彻底无歧义，免格式探测——旧文件惰性废弃零迁移）。
pub(crate) const GLOBAL_DB_FILE_NAME: &str = "desktop-global.redb";

/// workspace 库子树目录名（全局数据目录根直下，每 workspace 恰一个 db 文件）。
pub(crate) const WORKSPACES_DIR_NAME: &str = "workspaces";

/// 可读段截断上限（字符数）：文件名防超长的清洗预算。
const READABLE_SEGMENT_MAX_CHARS: usize = 24;

/// 文件名非法字符（`/` `\` `:` 路径分隔与盘符、`< > " | ? *` Windows 保留、
/// 控制字符不可见）：可读段清洗时逐字置换 `_`。
fn is_illegal_name_char(c: char) -> bool {
    matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
}

/// workspace 库文件名可读段（与 `WorkspaceRecord::name` 同源取 `dir_name`
/// 末段）清洗：非法字符置换 `_` → 按 char boundary 截断 ≤24 字符 → 去尾部
/// `.` 与空格（Windows 保留语义）；清洗后为空返回 `None`。
fn readable_segment(canonical_root: &str) -> Option<String> {
    let cleaned: String = dir_name(canonical_root)
        .chars()
        .map(|c| if is_illegal_name_char(c) { '_' } else { c })
        .collect();
    let truncated: String = cleaned.chars().take(READABLE_SEGMENT_MAX_CHARS).collect();
    let trimmed = truncated.trim_end_matches(['.', ' ']);
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// workspace 库文件名派生（**单点**，消费侧零派生逻辑）：`{可读段}-{hash}.redb`
/// ——哈希 = SHA-256(canonical root UTF-8 字节) 前 16 字节的 32 位小写 hex
/// （128-bit 抗碰撞，异根必不同名）；可读段 = [`readable_segment`] 清洗结果，
/// 为空回退纯哈希名。纯函数确定性：同根恒同名、跨重启可复现；文件名不含路径
/// 分隔符与 OS 非法字符。
pub(crate) fn workspace_db_file_name(canonical_root: &str) -> String {
    let digest = Sha256::digest(canonical_root.as_bytes());
    let hash: String = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    match readable_segment(canonical_root) {
        Some(readable) => format!("{readable}-{hash}.redb"),
        None => format!("{hash}.redb"),
    }
}

/// workspace 库文件路径派生（收口单点）：`workspaces/` 子树 + 文件名单点。
pub(crate) fn workspace_db_path(workspaces_dir: &Path, canonical_root: &str) -> PathBuf {
    workspaces_dir.join(workspace_db_file_name(canonical_root))
}

/// native_db 本地库句柄：单文件句柄语义（注入式路径打开，模型组在打开点
/// 锁定），私有持有 [`Database`] 与实例维度，可安全挂 Tauri State，同步调用
/// 无需 async。全局库经 [`Store::open_global`]（workspace 注册表 + agent
/// 管理两模型）、workspace 库经 [`Store::open_workspace`]（轮统计行 / 会话 /
/// 转录 / explore / change 流程状态八模型）。
///
/// 单进程约束：双开（如 dev 与正式版指向同一 db 文件）不保证安全，见 crate 文档。
pub struct Store {
    db: Database<'static>,
    /// 实例维度（打开点锁定；信封 API 过滤依据）
    dimension: DbDimension,
}

impl Store {
    /// 打开全局库（user 维度模型组：workspace 注册表 + agent 管理两模型）。
    pub fn open_global(path: &Path) -> Result<Self, StoreError> {
        Self::open_with(path, global_models(), DbDimension::User)
    }

    /// 打开 workspace 库（workspace 维度模型组，轮统计行 / 会话 / 转录 /
    /// explore / change 流程状态八模型）。
    pub fn open_workspace(path: &Path) -> Result<Self, StoreError> {
        Self::open_with(path, workspace_models(), DbDimension::Workspace)
    }

    /// 打开（不存在则创建）db 收口：`create_dir_all` 父目录 → 不存在（或空
    /// 文件）则 native_db create → 存在则以 native_db open。打不开即 Err
    /// （dev-team 据此 fail fast）。「空文件视同不存在」语义保留——全新文件组
    /// 冷启动零迁移，无任何旧格式探测路径。
    fn open_with(
        path: &Path,
        models: &'static Models,
        dimension: DbDimension,
    ) -> Result<Self, StoreError> {
        // native_db 建文件不建父目录，首启必须补齐；裸文件名（无父目录）跳过
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    StoreError::Db(format!("创建 db 父目录 {} 失败: {e}", parent.display()))
                })?;
            }
        }
        // 空文件视同不存在（redb 语义：空文件初始化为新库；中断首启的残照）
        let blank = !path.exists()
            || std::fs::metadata(path)
                .map(|m| m.len() == 0)
                .unwrap_or(false);
        if blank {
            let db = Builder::new()
                .create(models, path)
                .map_err(|e| StoreError::Db(format!("创建 {} 失败: {e}", path.display())))?;
            return Ok(Self { db, dimension });
        }
        let db = Builder::new().open(models, path).map_err(|e| {
            StoreError::Db(format!(
                "打开 {} 失败: {e}（无法识别的 db 格式）",
                path.display()
            ))
        })?;
        Ok(Self { db, dimension })
    }

    /// canonicalize + upsert：已存在 → 原记录原样返回（保留 `added_at`，
    /// 不刷新任何时间戳）；新建 → `added_at` 取 now。返回落库后的记录
    /// （canonical root，前端以此为当前根，展示与库内 key 同源）。
    pub fn add_workspace(&self, root: &Path) -> Result<WorkspaceRecord, StoreError> {
        let key = canonical::canonical_key(root)
            .map_err(StoreError::Canonicalize)?
            .to_string_lossy()
            .into_owned();
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let stored: Option<WorkspaceRecord> = rw
            .get()
            .primary(key.as_str())
            .map_err(db_err("读取已有记录"))?;
        let record = match stored {
            // 等价路径已入库：原记录原样返回（无时间戳可刷新）
            Some(record) => record,
            None => {
                let record = WorkspaceRecord::from_root(&key, now_millis());
                rw.insert(record.clone()).map_err(db_err("写入记录"))?;
                record
            }
        };
        rw.commit().map_err(db_err("提交 add_workspace 事务"))?;
        Ok(record)
    }

    /// 清单：主键（canonical root）自然序（native_db 主键迭代序，字典序升序）。
    /// 顺序与打开/添加时间无关，稳定可复现——sidebar 清单不因使用而重排。
    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceRecord>, StoreError> {
        Ok(self
            .read_all::<WorkspaceRecord>("遍历清单")?
            .into_iter()
            .collect())
    }

    /// 按 canonical key 删除（含消失目录的回退匹配）；miss 幂等 `Ok(false)`。
    pub fn remove_workspace(&self, root: &Path) -> Result<bool, StoreError> {
        let Some(key) = self.resolve_key(root)? else {
            return Ok(false);
        };
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let stored: Option<WorkspaceRecord> =
            rw.get().primary(key.as_str()).map_err(db_err("读取记录"))?;
        let Some(record) = stored else {
            return Ok(false); // 解析与删除之间被移除：miss 幂等
        };
        rw.remove(record).map_err(db_err("删除记录"))?;
        rw.commit().map_err(db_err("提交 remove_workspace 事务"))?;
        Ok(true)
    }

    // --- agent 会话域（会话一等公民：write-through 原子操作 + 查询/聚合现算
    // + 对账重导显式入口）-----------------------------------------------

    /// 会话行落库（id 来自 core 铸造，字符串主键直用）。
    pub fn create_session(&self, session: &SessionRecord) -> Result<SessionRecord, StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        rw.insert(session.clone()).map_err(db_err("写入会话记录"))?;
        rw.commit().map_err(db_err("提交 create_session 事务"))?;
        Ok(session.clone())
    }

    /// 主键直查会话（Continue 校验与装配消费）。
    pub fn find_session(&self, session_id: &str) -> Result<Option<SessionRecord>, StoreError> {
        let r = self.db.r_transaction().map_err(db_err("开启读事务"))?;
        let hit: Option<SessionRecord> = r
            .get()
            .primary(session_id)
            .map_err(db_err("读取会话记录"))?;
        Ok(hit)
    }

    /// 轮统计行 begin：写事务内 `max(id)+1` 分配（与插入原子，首行 id=1），
    /// `session_id` 挂 core 会话、`running` 初值，返回含 id 的记录。
    pub fn begin_agent_turn(
        &self,
        session_id: &str,
        started_at: i64,
    ) -> Result<AgentRunRecord, StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        // 主键自然序表尾即最大 id（双向迭代 next_back，与旧 last()+1 同口径）
        let next_id = match rw
            .scan()
            .primary::<AgentRunRecord>()
            .map_err(db_err("读取最大 id"))?
            .all()
            .map_err(db_err("读取最大 id"))?
            .next_back()
        {
            Some(Ok(record)) => record.id + 1,
            Some(Err(e)) => return Err(StoreError::Db(format!("读取最大 id: {e}"))),
            None => 1,
        };
        let record = AgentRunRecord {
            id: next_id,
            session_id: Some(session_id.to_owned()),
            status: agent::AgentRunStatus::Running,
            started_at,
            finished_at: None,
            num_turns: None,
            cost_usd: None,
            duration_ms: None,
            error: None,
        };
        rw.insert(record.clone()).map_err(db_err("写入轮记录"))?;
        rw.commit().map_err(db_err("提交 begin_agent_turn 事务"))?;
        Ok(record)
    }

    /// 密封转录追加：单事务写入；`event_key` 由 `hash64(session_id) + seq`
    /// 打包（seq 取自事件本体，不存在「缺 seq」错误路径）。增量防御性忽略
    /// 不产生记录（内核泵只送密封事件，此处为纵深防御——store 中不存在任何
    /// delta 行）。
    pub fn append_session_events(
        &self,
        session_id: &str,
        events: &[AgentEvent],
    ) -> Result<(), StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        for event in events {
            if event.kind.is_delta() {
                continue; // 增量永不见（碎事件根因的落库半边修复）
            }
            let record = SessionEventRecord::new(session_id, event.clone());
            rw.insert(record).map_err(db_err("写入转录事件"))?;
        }
        rw.commit().map_err(db_err("提交转录追加事务"))?;
        Ok(())
    }

    /// 轮行终态收口：按 turn id 取行，status / finished_at / 统计 / error
    /// 整组替换（`session_id` / `started_at` 沿用存量行——终态装配侧不携带）。
    pub fn finish_agent_turn(
        &self,
        turn_id: i64,
        record: &AgentRunRecord,
    ) -> Result<(), StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let mut stored: AgentRunRecord =
            rw.get()
                .primary(turn_id)
                .map_err(db_err("读取轮记录"))?
                .ok_or_else(|| StoreError::Db(format!("轮记录不存在: id={turn_id}")))?;
        stored.status = record.status;
        stored.finished_at = record.finished_at;
        stored.num_turns = record.num_turns;
        stored.cost_usd = record.cost_usd;
        stored.duration_ms = record.duration_ms;
        stored.error = record.error.clone();
        rw.upsert(stored).map_err(db_err("写入轮记录终态"))?;
        rw.commit().map_err(db_err("提交 finish_agent_turn 事务"))?;
        Ok(())
    }

    /// 双 id 映射落库半边 + `updated_at` 刷新（引擎侧标识上报时调用；remote
    /// 为 None 仅刷新时间戳）。会话不存在 `Err`。
    pub fn bind_session_remote(
        &self,
        session_id: &str,
        remote: Option<&str>,
        updated_at: i64,
    ) -> Result<(), StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let mut stored: SessionRecord = rw
            .get()
            .primary(session_id)
            .map_err(db_err("读取会话记录"))?
            .ok_or_else(|| StoreError::Db(format!("会话不存在: id={session_id}")))?;
        if let Some(remote) = remote {
            stored.engine_session_id = Some(remote.to_owned());
        }
        stored.updated_at = updated_at;
        rw.upsert(stored).map_err(db_err("写入会话记录"))?;
        rw.commit()
            .map_err(db_err("提交 bind_session_remote 事务"))?;
        Ok(())
    }

    /// 会话清单：来源过滤（`source` / `source_ref` 各自可选）+ `updated_at`
    /// 降序稳定序（并列按 id 降序，与运行清单同哲学）+ 聚合统计现算（轮数 /
    /// 累计墙钟自轮统计行、累计 token 自转录 TurnDone usage 鸭子类型求和，
    /// 缺席合法缺省——MVP 不维护累计列）+ 轮统计行随行返回（发起顺序）。
    /// 全表读 + 内存过滤——调试页数据量小，与清单读面同哲学。
    pub fn list_sessions(
        &self,
        source: Option<&str>,
        source_ref: Option<&str>,
    ) -> Result<Vec<SessionSummary>, String> {
        let sessions: Vec<SessionRecord> = self
            .read_all::<SessionRecord>("遍历会话清单")
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|record| source.is_none_or(|src| record.source == src))
            .filter(|record| {
                source_ref.is_none_or(|reference| record.source_ref.as_deref() == Some(reference))
            })
            .collect();
        let runs = self
            .read_all::<AgentRunRecord>("遍历轮统计行")
            .map_err(|e| e.to_string())?;
        let mut summaries = Vec::with_capacity(sessions.len());
        for session in &sessions {
            let mut turns: Vec<TurnSummary> = runs
                .iter()
                .filter(|run| run.session_id.as_deref() == Some(session.id.as_str()))
                .map(turn_summary)
                .collect();
            // 发起顺序：`started_at` 升序，并列按轮 id 升序（确定可复现）
            turns.sort_by(|a, b| {
                a.started_at
                    .cmp(&b.started_at)
                    .then(a.turn_id.cmp(&b.turn_id))
            });
            let events = self.list_session_events(&session.id)?;
            let stats = aggregate_stats(turns.len(), &turns, &events);
            summaries.push(SessionSummary {
                row: session_row(session)?,
                stats,
                turns,
            });
        }
        summaries.sort_by(|a, b| {
            b.row
                .updated_at
                .cmp(&a.row.updated_at)
                .then_with(|| b.row.id.cmp(&a.row.id))
        });
        Ok(summaries)
    }

    /// 转录重放：全表读 + 内存过滤出本会话（`session_id` 非唯一二级索引保
    /// 查询形态），seq 升序返回（打包主键大端序保证同会话内自然序即重放序；
    /// 增量占 seq 产生的库内空洞不破坏有序性），不要求运行进程存活。
    pub fn list_session_events(&self, session_id: &str) -> Result<Vec<AgentEvent>, String> {
        let mut events: Vec<AgentEvent> = self
            .read_all::<SessionEventRecord>("遍历会话转录")
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|record| record.session_id == session_id)
            .map(|record| record.event)
            .collect();
        events.sort_by_key(|event| event.seq);
        Ok(events)
    }

    /// 对账纠偏重导显式入口：从转录 `TurnDone` 事件重算聚合（轮数 / 累计
    /// 墙钟 / 累计 token），不改密封转录、不隐式挂读路径。轮统计行缺席或
    /// 偏差时以转录为准的校正口径。
    pub fn reconcile_session_stats(&self, session_id: &str) -> Result<SessionStats, String> {
        let events = self.list_session_events(session_id)?;
        let mut turn_count: u64 = 0;
        let mut durations: Vec<Option<u64>> = Vec::new();
        for event in &events {
            if let AgentEventKind::TurnDone { duration_ms, .. } = &event.kind {
                turn_count += 1;
                durations.push(*duration_ms);
            }
        }
        let total_duration_ms = sum_present(durations.into_iter());
        Ok(SessionStats {
            turn_count,
            total_duration_ms,
            input_tokens: sum_usage_tokens(&events, "inputTokens"),
            output_tokens: sum_usage_tokens(&events, "outputTokens"),
        })
    }

    /// explore 清单：按 root 过滤，主键 id 升序（读出自然序，稳定可复现）。
    /// root 为记录归属键（调用方持有 canonical root，store 不二次 canonicalize）。
    pub fn list_explore_records(&self, root: &str) -> Result<Vec<ExploreRecord>, StoreError> {
        Ok(self
            .read_all::<ExploreRecord>("遍历探索清单")?
            .into_iter()
            .filter(|record| record.root == root)
            .collect())
    }

    /// 按归属与名称寻址单条 explore 记录（详情页按展示名寻址）。
    pub fn find_explore_record(
        &self,
        root: &str,
        name: &str,
    ) -> Result<Option<ExploreRecord>, StoreError> {
        Ok(self
            .list_explore_records(root)?
            .into_iter()
            .find(|record| record.name == name))
    }

    /// 新建 explore 记录：写事务内 `max(id)+1` 分配（与插入原子，与
    /// [`Store::begin_agent_run`] 同语义）；同 `(root, name)` 已存在 → `Err`。
    /// 只写 DB——磁盘笔记文件由 agent 会话流程懒创建，本方法不触磁盘。
    pub fn create_explore_record(
        &self,
        root: &str,
        name: &str,
    ) -> Result<ExploreRecord, StoreError> {
        if !is_single_component_name(name) {
            return Err(StoreError::Db(format!(
                "非法记录名: {name:?}（须为单分量名）"
            )));
        }
        if self.find_explore_record(root, name)?.is_some() {
            return Err(StoreError::Db(format!(
                "记录已存在: root={root:?} name={name:?}"
            )));
        }
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let next_id = match rw
            .scan()
            .primary::<ExploreRecord>()
            .map_err(db_err("读取最大 id"))?
            .all()
            .map_err(db_err("读取最大 id"))?
            .next_back()
        {
            Some(Ok(record)) => record.id + 1,
            Some(Err(e)) => return Err(StoreError::Db(format!("读取最大 id: {e}"))),
            None => 1,
        };
        let mut record = ExploreRecord::new(root, name, now_millis());
        record.id = next_id;
        rw.insert(record.clone()).map_err(db_err("写入探索记录"))?;
        rw.commit()
            .map_err(db_err("提交 create_explore_record 事务"))?;
        Ok(record)
    }

    /// in-place 改名（保主键 → 保 `source_ref` 会话链绑定），刷新 `updated_at`；
    /// 目标名已存在 → `Err`。删 + 重建会分配新主键导致链断，改名必须 in-place。
    pub fn rename_explore_record(
        &self,
        root: &str,
        name: &str,
        new_name: &str,
    ) -> Result<ExploreRecord, StoreError> {
        if !is_single_component_name(new_name) {
            return Err(StoreError::Db(format!(
                "非法记录名: {new_name:?}（须为单分量名）"
            )));
        }
        if new_name != name && self.find_explore_record(root, new_name)?.is_some() {
            return Err(StoreError::Db(format!(
                "目标名已存在: root={root:?} name={new_name:?}"
            )));
        }
        let stored = self
            .find_explore_record(root, name)?
            .ok_or_else(|| StoreError::Db(format!("记录不存在: root={root:?} name={name:?}")))?;
        let mut updated = stored;
        updated.name = new_name.to_owned();
        updated.updated_at = now_millis();
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        rw.upsert(updated.clone()).map_err(db_err("写入探索记录"))?;
        rw.commit()
            .map_err(db_err("提交 rename_explore_record 事务"))?;
        Ok(updated)
    }

    /// 删除 explore 记录：MUST NOT 触碰磁盘文件（文件是记录的可丢弃投影，
    /// 删除方向亦然）；记录名下的**归属会话及其转录与轮统计行**随记录**同
    /// 事务级联删除**（会话化后级联圈定自主平移至会话归属）——id 是幸存行上
    /// max+1 的可复用计数，悬空 `source_ref` 不清则下次建档复用 id 时旧聊天
    /// 经 `(source, source_ref)` 匹配错挂到新记录。miss 幂等 `Ok(false)`。
    pub fn delete_explore_record(&self, root: &str, name: &str) -> Result<bool, StoreError> {
        let Some(record) = self.find_explore_record(root, name)? else {
            return Ok(false);
        };
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        // 级联圈定：explore 来源且 source_ref 指向本记录 id 的会话（全表读 +
        // 内存过滤，与清单读面同哲学——调试页数据量小）
        let source_ref = record.id.to_string();
        let bound_sessions: Vec<SessionRecord> = rw
            .scan()
            .primary::<SessionRecord>()
            .map_err(db_err("扫描会话清单"))?
            .all()
            .map_err(db_err("扫描会话清单"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描会话清单"))?
            .into_iter()
            .filter(|session| {
                session.source == EXPLORE_RUN_SOURCE
                    && session.source_ref.as_deref() == Some(&source_ref)
            })
            .collect();
        for session in &bound_sessions {
            let events: Vec<SessionEventRecord> = rw
                .scan()
                .primary::<SessionEventRecord>()
                .map_err(db_err("扫描会话转录"))?
                .all()
                .map_err(db_err("扫描会话转录"))?
                .collect::<native_db::db_type::Result<Vec<_>>>()
                .map_err(db_err("扫描会话转录"))?
                .into_iter()
                .filter(|event_record| event_record.session_id() == session.id)
                .collect();
            for event_record in events {
                rw.remove(event_record).map_err(db_err("删除转录事件"))?;
            }
            let turns: Vec<AgentRunRecord> = rw
                .scan()
                .primary::<AgentRunRecord>()
                .map_err(db_err("扫描轮统计行"))?
                .all()
                .map_err(db_err("扫描轮统计行"))?
                .collect::<native_db::db_type::Result<Vec<_>>>()
                .map_err(db_err("扫描轮统计行"))?
                .into_iter()
                .filter(|run| run.session_id.as_deref() == Some(session.id.as_str()))
                .collect();
            for turn in turns {
                rw.remove(turn).map_err(db_err("删除轮统计行"))?;
            }
            rw.remove(session.clone()).map_err(db_err("删除会话记录"))?;
        }
        rw.remove(record).map_err(db_err("删除探索记录"))?;
        rw.commit()
            .map_err(db_err("提交 delete_explore_record 事务"))?;
        Ok(true)
    }

    /// provider 清单：主键 id 升序自然序（稳定可复现）。
    pub fn list_agent_providers(&self) -> Result<Vec<AgentProviderRecord>, StoreError> {
        self.read_all::<AgentProviderRecord>("遍历 provider 清单")
    }

    /// 主键直查 provider（运行发起解析与 save 回填原值消费）。
    pub fn find_agent_provider(&self, id: i64) -> Result<Option<AgentProviderRecord>, StoreError> {
        let r = self.db.r_transaction().map_err(db_err("开启读事务"))?;
        let hit: Option<AgentProviderRecord> =
            r.get().primary(id).map_err(db_err("读取 provider 记录"))?;
        Ok(hit)
    }

    /// provider upsert：id=0 新建（单写事务内 name 查重 + max+1 分配）/
    /// id>0 整行替换（查重排除自身，行须存在）。name 空白 `Err`；重名 `Err`。
    /// 返回落库记录。
    pub fn upsert_agent_provider(
        &self,
        provider: AgentProviderRecord,
    ) -> Result<AgentProviderRecord, StoreError> {
        if provider.name.trim().is_empty() {
            return Err(StoreError::Db("provider 名不得为空白".to_owned()));
        }
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let all: Vec<AgentProviderRecord> = rw
            .scan()
            .primary::<AgentProviderRecord>()
            .map_err(db_err("扫描 provider 清单"))?
            .all()
            .map_err(db_err("扫描 provider 清单"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描 provider 清单"))?;
        if let Some(hit) = all
            .iter()
            .find(|record| record.name == provider.name && record.id != provider.id)
        {
            return Err(StoreError::Db(format!("provider 名已存在: {}", hit.name)));
        }
        let mut record = provider;
        if record.id == 0 {
            // 新建臂：主键自然序表尾 max+1（与插入原子）
            record.id = all.last().map_or(1, |last| last.id + 1);
        } else if !all.iter().any(|stored| stored.id == record.id) {
            // 更新臂：整行替换要求行存在（不隐式插入任意 id）
            return Err(StoreError::Db(format!("provider 不存在: id={}", record.id)));
        }
        rw.upsert(record.clone())
            .map_err(db_err("写入 provider 记录"))?;
        rw.commit()
            .map_err(db_err("提交 upsert_agent_provider 事务"))?;
        Ok(record)
    }

    /// 删除 provider：被任一 agent `provider_id` 引用 → `Err`（含引用方 name
    /// 提示，不级联不删除——引用完整性）；miss 幂等 `Ok(false)`。
    pub fn remove_agent_provider(&self, id: i64) -> Result<bool, StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let stored: Option<AgentProviderRecord> =
            rw.get().primary(id).map_err(db_err("读取 provider 记录"))?;
        let Some(record) = stored else {
            return Ok(false); // miss 幂等
        };
        let referencing: Vec<String> = rw
            .scan()
            .primary::<AgentInstanceRecord>()
            .map_err(db_err("扫描 agent 实例"))?
            .all()
            .map_err(db_err("扫描 agent 实例"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描 agent 实例"))?
            .into_iter()
            .filter(|agent| agent.provider_id == Some(id))
            .map(|agent| agent.name)
            .collect();
        if !referencing.is_empty() {
            return Err(StoreError::Db(format!(
                "provider 被 agent 引用，禁止删除: id={id} 被 [{}] 引用",
                referencing.join("、")
            )));
        }
        rw.remove(record).map_err(db_err("删除 provider 记录"))?;
        rw.commit()
            .map_err(db_err("提交 remove_agent_provider 事务"))?;
        Ok(true)
    }

    /// agent 实例清单：主键 id 升序自然序。
    pub fn list_agent_instances(&self) -> Result<Vec<AgentInstanceRecord>, StoreError> {
        self.read_all::<AgentInstanceRecord>("遍历 agent 实例清单")
    }

    /// 主键直查 agent 实例（显式路径解析消费）。
    pub fn find_agent_instance(&self, id: i64) -> Result<Option<AgentInstanceRecord>, StoreError> {
        let r = self.db.r_transaction().map_err(db_err("开启读事务"))?;
        let hit: Option<AgentInstanceRecord> =
            r.get().primary(id).map_err(db_err("读取 agent 记录"))?;
        Ok(hit)
    }

    /// 默认 agent 实例（缺省运行解析入口）：清单扫 `is_default`，恒零或一
    /// （标记唯一写口保证）。
    pub fn default_agent_instance(&self) -> Result<Option<AgentInstanceRecord>, StoreError> {
        Ok(self
            .read_all::<AgentInstanceRecord>("遍历 agent 实例清单")?
            .into_iter()
            .find(|record| record.is_default))
    }

    /// agent 实例 upsert：id=0 新建（单写事务内 name 查重 + max+1 分配，
    /// **强制 `is_default = false`**——默认标记唯一写口为
    /// [`Store::set_default_agent_instance`]）/ id>0 整行替换（查重排除自身、
    /// **保留存量默认标记**——入参标记不参与写，不变式免受前端入参影响）。
    /// `engine == Sdk` 时 `provider_id` 必填且引用 provider 须存在（缺失 /
    /// 悬空均 `Err`）；cli 可空（携引用同样校验悬空）。返回落库记录。
    pub fn upsert_agent_instance(
        &self,
        agent: AgentInstanceRecord,
    ) -> Result<AgentInstanceRecord, StoreError> {
        if agent.name.trim().is_empty() {
            return Err(StoreError::Db("agent 名不得为空白".to_owned()));
        }
        if agent.engine == AgentEngineKind::Sdk && agent.provider_id.is_none() {
            return Err(StoreError::Db(format!(
                "sdk agent 须选择 provider: {}",
                agent.name
            )));
        }
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        if let Some(provider_id) = agent.provider_id {
            let referenced: Option<AgentProviderRecord> = rw
                .get()
                .primary(provider_id)
                .map_err(db_err("读取 provider 记录"))?;
            if referenced.is_none() {
                return Err(StoreError::Db(format!(
                    "agent 引用的 provider 不存在: id={provider_id}"
                )));
            }
        }
        let all: Vec<AgentInstanceRecord> = rw
            .scan()
            .primary::<AgentInstanceRecord>()
            .map_err(db_err("扫描 agent 实例"))?
            .all()
            .map_err(db_err("扫描 agent 实例"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描 agent 实例"))?;
        if let Some(hit) = all
            .iter()
            .find(|record| record.name == agent.name && record.id != agent.id)
        {
            return Err(StoreError::Db(format!("agent 名已存在: {}", hit.name)));
        }
        let mut record = agent;
        match record.id {
            0 => {
                // 新建臂：表尾 max+1 分配 + 恒非默认
                record.id = all.last().map_or(1, |last| last.id + 1);
                record.is_default = false;
            }
            id => {
                // 更新臂：整行替换保留存量默认标记
                let stored = all
                    .iter()
                    .find(|record| record.id == id)
                    .ok_or_else(|| StoreError::Db(format!("agent 不存在: id={id}")))?;
                record.is_default = stored.is_default;
            }
        }
        rw.upsert(record.clone())
            .map_err(db_err("写入 agent 记录"))?;
        rw.commit()
            .map_err(db_err("提交 upsert_agent_instance 事务"))?;
        Ok(record)
    }

    /// 删除 agent 实例：默认 agent 同事务先清标记再删（标记唯一写口纪律；
    /// 删除后全局无默认，无顺延）；miss 幂等 `Ok(false)`。
    pub fn remove_agent_instance(&self, id: i64) -> Result<bool, StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let stored: Option<AgentInstanceRecord> =
            rw.get().primary(id).map_err(db_err("读取 agent 记录"))?;
        let Some(mut record) = stored else {
            return Ok(false); // miss 幂等
        };
        if record.is_default {
            record.is_default = false;
            rw.upsert(record.clone()).map_err(db_err("清除默认标记"))?;
        }
        rw.remove(record).map_err(db_err("删除 agent 记录"))?;
        rw.commit()
            .map_err(db_err("提交 remove_agent_instance 事务"))?;
        Ok(true)
    }

    /// 标记即切换（默认标记唯一写口）：单写事务内清全部既有默认 → 置目标
    /// （全局恒至多一）；miss `Err`；返回更新后记录。
    pub fn set_default_agent_instance(&self, id: i64) -> Result<AgentInstanceRecord, StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let all: Vec<AgentInstanceRecord> = rw
            .scan()
            .primary::<AgentInstanceRecord>()
            .map_err(db_err("扫描 agent 实例"))?
            .all()
            .map_err(db_err("扫描 agent 实例"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描 agent 实例"))?;
        let mut target = all
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .ok_or_else(|| StoreError::Db(format!("agent 不存在: id={id}")))?;
        for mut record in all {
            if record.is_default && record.id != id {
                record.is_default = false;
                rw.upsert(record).map_err(db_err("清除旧默认标记"))?;
            }
        }
        target.is_default = true;
        rw.upsert(target.clone()).map_err(db_err("写入默认标记"))?;
        rw.commit()
            .map_err(db_err("提交 set_default_agent_instance 事务"))?;
        Ok(target)
    }

    // --- change 流程状态域（desktop-change-state-store：建档 / 相位落账 /
    // 回跳 / 步骤审计 / status 翻转；记录 ↔ `workflow::state` 中性类型映射收
    // 本文件单点，design D2）---------------------------------------------

    /// 建档：同名记录已存在 → [`StoreError::Conflict`]（active 记录为建档冲
    /// 突；archived 记录为名字占用——主键 name 不复用）。返回落库记录。
    pub fn create_change_record(
        &self,
        record: ChangeStateRecord,
    ) -> Result<ChangeStateRecord, StoreError> {
        if let Some(existing) = self.find_change_record(&record.name)? {
            return Err(match existing.status {
                ChangeStatus::Active => StoreError::Conflict(format!(
                    "change 已存在同名建档记录: {}",
                    existing.name
                )),
                ChangeStatus::Archived => StoreError::Conflict(format!(
                    "change 名已被归档记录占用（主键 name 不复用）: {}",
                    existing.name
                )),
            });
        }
        let stored = ChangeRecord {
            name: record.name.clone(),
            workflow_type: record.workflow_type,
            created_at: record.created_at,
            status: record.status,
            archived_at: record.archived_at,
            active_phase: record.active_phase.map(change_active_phase_stored),
        };
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        rw.insert(stored.clone()).map_err(db_err("写入建档记录"))?;
        rw.commit()
            .map_err(db_err("提交 create_change_record 事务"))?;
        Ok(change_state(&stored))
    }

    /// 补偿删除（create 双写 fs 半边失败回滚面，design D5）：按主键删除本次
    /// 自插行；miss 幂等 `Ok(false)`。
    pub fn delete_change_record(&self, name: &str) -> Result<bool, StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let stored: Option<ChangeRecord> =
            rw.get().primary(name).map_err(db_err("读取建档记录"))?;
        let Some(record) = stored else {
            return Ok(false); // miss 幂等
        };
        rw.remove(record).map_err(db_err("删除建档记录"))?;
        rw.commit()
            .map_err(db_err("提交 delete_change_record 事务"))?;
        Ok(true)
    }

    /// 主键直查建档记录（None = 文档形态）。
    pub fn find_change_record(&self, name: &str) -> Result<Option<ChangeStateRecord>, StoreError> {
        let r = self.db.r_transaction().map_err(db_err("开启读事务"))?;
        let hit: Option<ChangeRecord> =
            r.get().primary(name).map_err(db_err("读取建档记录"))?;
        Ok(hit.as_ref().map(change_state))
    }

    /// 建档全量：主键 name 自然序。
    pub fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreError> {
        Ok(self
            .read_all::<ChangeRecord>("遍历建档清单")?
            .iter()
            .map(change_state)
            .collect())
    }

    /// 开相：单写事务内 attempt 推导（该相位既有条目数 + 1）+ active_phase
    /// 写入；change miss → [`StoreError::NotFound`]。active_phase 无条件覆写
    ///（与既往 phase_start 定点写语义一致）。
    pub fn start_change_phase(
        &self,
        change: &str,
        phase: &str,
        now: i64,
    ) -> Result<PhaseStartState, StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let mut stored: ChangeRecord = rw
            .get()
            .primary(change)
            .map_err(db_err("读取建档记录"))?
            .ok_or_else(|| StoreError::NotFound(format!("change 不存在: {change}")))?;
        let attempt = rw
            .scan()
            .primary::<PhaseRecord>()
            .map_err(db_err("扫描评估条目"))?
            .all()
            .map_err(db_err("扫描评估条目"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描评估条目"))?
            .into_iter()
            .filter(|record| record.change == change && record.phase == phase)
            .count() as u32
            + 1;
        stored.active_phase = Some(ChangeActivePhase {
            phase: phase.to_owned(),
            attempt,
            start_at: now,
        });
        rw.upsert(stored).map_err(db_err("写入开相状态"))?;
        rw.commit()
            .map_err(db_err("提交 start_change_phase 事务"))?;
        Ok(PhaseStartState {
            attempt,
            start_at: now,
        })
    }

    /// 相位落账单事务原子（design AC-2）：PhaseRecord 落行（id 写事务内
    /// max+1）+ checklist 子行落行 + active_phase 清位 + `(change, phase,
    /// attempt)` 写事务内查重；change miss / active_phase 不匹配 →
    /// [`StoreError::NotFound`]，重复落账 → [`StoreError::Conflict`]。任一环
    /// 节失败整体回滚零残留。
    pub fn log_change_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let mut change_row: ChangeRecord = rw
            .get()
            .primary(command.change.as_str())
            .map_err(db_err("读取建档记录"))?
            .ok_or_else(|| StoreError::NotFound(format!("change 不存在: {}", command.change)))?;
        // active_phase 匹配复核（事务内权威；start_at 随行兜底）
        let active_start_at = change_row
            .active_phase
            .as_ref()
            .filter(|active| active.phase == command.phase)
            .map(|active| active.start_at)
            .ok_or_else(|| {
                StoreError::NotFound(format!(
                    "phase \"{}\" 未开启（active_phase 不匹配），不可落账",
                    command.phase
                ))
            })?;
        let all: Vec<PhaseRecord> = rw
            .scan()
            .primary::<PhaseRecord>()
            .map_err(db_err("扫描评估条目"))?
            .all()
            .map_err(db_err("扫描评估条目"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描评估条目"))?;
        // attempt 事务内推导（该相位既有条目数 + 1）+ 查重（沿 store 唯一性惯例）
        let attempt = all
            .iter()
            .filter(|record| {
                record.change == command.change && record.phase == command.phase
            })
            .count() as u32
            + 1;
        if all.iter().any(|record| {
            record.change == command.change
                && record.phase == command.phase
                && record.attempt == attempt
        }) {
            return Err(StoreError::Conflict(format!(
                "评估条目已存在: change={} phase={} attempt={attempt}",
                command.change, command.phase
            )));
        }
        // 主键自然序表尾即最大 id（max+1 分配，与插入原子）
        let next_id = all.last().map_or(1, |last| last.id + 1);
        let record = PhaseRecord {
            id: next_id,
            change: command.change.clone(),
            phase: command.phase.clone(),
            attempt,
            verdict: command.verdict,
            report: command.report.clone(),
            skipped: command.skipped,
            stale: false,
            backtrack_to: None,
            backtrack_reason: None,
            executor_session_id: command.executor_session_id.clone(),
            evaluator_session_id: command.evaluator_session_id.clone(),
            decision_session_id: command.decision_session_id.clone(),
            start_at: command.start_at.or(Some(active_start_at)),
            timestamp: command.timestamp,
        };
        rw.insert(record).map_err(db_err("写入评估条目"))?;
        for (index, item) in command.checklist.iter().enumerate() {
            rw.insert(ChecklistItemRecord::new(next_id, index as u32, item.clone()))
                .map_err(db_err("写入检查项子行"))?;
        }
        // 落账即收相位（开相才可落账）
        change_row.active_phase = None;
        rw.upsert(change_row).map_err(db_err("清位 active_phase"))?;
        rw.commit()
            .map_err(db_err("提交 log_change_phase 事务"))?;
        Ok(attempt)
    }

    /// 回跳单事务：发起相位最新条目落 backtrack 标记 + 目标相位最新 pass 置
    /// stale（无 pass 条目 no-op 且不传播，沿既往 `mark_phase_stale` 语义）+
    /// `stale_dependents` 闭包全条目置 stale；发起相位无条目 →
    /// [`StoreError::NotFound`]。
    ///
    /// 多处标记落同一行时按主键组装逐位叠加（回跳发起相位常在目标下游闭包
    /// 内——标记行的 `backtrack_to` / `backtrack_reason` 与闭包 stale 翻转同
    /// 行并存，与既往 JSON 载体上标记 + 传播两次读改写的合成终态一致），禁止
    /// 快照克隆整行后推覆写先写标记。
    pub fn apply_change_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let rows: Vec<PhaseRecord> = rw
            .scan()
            .primary::<PhaseRecord>()
            .map_err(db_err("扫描评估条目"))?
            .all()
            .map_err(db_err("扫描评估条目"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描评估条目"))?
            .into_iter()
            .filter(|record| record.change == command.change)
            .collect();
        // 待写集按主键 id 组装：同 id 多处标记在同一克隆行上叠加，后写不再
        // 整行覆写先写；id 序写出（确定性，审计可读）
        let mut updates: BTreeMap<i64, PhaseRecord> = BTreeMap::new();
        // 回跳标记：发起相位最新条目（timestamp 降序取首，并列取落行序后者）
        let latest = rows
            .iter()
            .filter(|record| record.phase == command.phase)
            .max_by_key(|record| (record.timestamp, record.id))
            .ok_or_else(|| {
                StoreError::NotFound(format!(
                    "Phase \"{}\" 没有评估条目，无法设置回溯",
                    command.phase
                ))
            })?;
        let marked = updates.entry(latest.id).or_insert_with(|| latest.clone());
        marked.backtrack_to = Some(command.to.clone());
        marked.backtrack_reason = Some(command.reason.clone());
        // 目标相位最新 pass 置 stale；无 pass 条目 no-op 且不传播
        let target_pass = rows
            .iter()
            .filter(|record| record.phase == command.to && record.verdict == Verdict::Pass)
            .max_by_key(|record| (record.timestamp, record.id));
        if let Some(target) = target_pass {
            updates
                .entry(target.id)
                .or_insert_with(|| target.clone())
                .stale = true;
            for dependent in &command.stale_dependents {
                for row in rows.iter().filter(|record| &record.phase == dependent) {
                    updates.entry(row.id).or_insert_with(|| row.clone()).stale = true;
                }
            }
        }
        for update in updates.into_values() {
            rw.upsert(update).map_err(db_err("写入回跳标记"))?;
        }
        rw.commit()
            .map_err(db_err("提交 apply_change_backtrack 事务"))?;
        Ok(())
    }

    /// decision 槽位幂等挂账：该相位最新条目定点改写；无条目 →
    /// [`StoreError::NotFound`]。
    pub fn amend_change_decision_session(
        &self,
        change: &str,
        phase: &str,
        session_id: &str,
    ) -> Result<(), StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let latest = rw
            .scan()
            .primary::<PhaseRecord>()
            .map_err(db_err("扫描评估条目"))?
            .all()
            .map_err(db_err("扫描评估条目"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描评估条目"))?
            .into_iter()
            .filter(|record| record.change == change && record.phase == phase)
            .max_by_key(|record| (record.timestamp, record.id))
            .ok_or_else(|| {
                StoreError::NotFound(format!(
                    "Phase \"{phase}\" 没有评估条目，无法挂账决策会话"
                ))
            })?;
        let mut marked = latest;
        marked.decision_session_id = Some(session_id.to_owned());
        rw.upsert(marked).map_err(db_err("写入决策槽位"))?;
        rw.commit()
            .map_err(db_err("提交 amend_change_decision_session 事务"))?;
        Ok(())
    }

    /// status 翻转（归档 db 半边）：status=archived + archived_at；主键 name
    /// 不变；miss → [`StoreError::NotFound`]。
    pub fn set_change_archived(&self, name: &str, archived_at: i64) -> Result<(), StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        let mut stored: ChangeRecord = rw
            .get()
            .primary(name)
            .map_err(db_err("读取建档记录"))?
            .ok_or_else(|| StoreError::NotFound(format!("change 不存在: {name}")))?;
        stored.status = ChangeStatus::Archived;
        stored.archived_at = Some(archived_at);
        rw.upsert(stored).map_err(db_err("写入归档状态"))?;
        rw.commit()
            .map_err(db_err("提交 set_change_archived 事务"))?;
        Ok(())
    }

    /// 步骤审计行追加（行 id 写事务内 max+1）。
    pub fn append_change_step(&self, command: &StepCommand) -> Result<(), StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        // 主键自然序表尾即最大 id（max+1 分配，与插入原子）
        let next_id = match rw
            .scan()
            .primary::<StepRecord>()
            .map_err(db_err("读取最大 id"))?
            .all()
            .map_err(db_err("读取最大 id"))?
            .next_back()
        {
            Some(Ok(record)) => record.id + 1,
            Some(Err(e)) => return Err(StoreError::Db(format!("读取最大 id: {e}"))),
            None => 1,
        };
        let mut record = StepRecord::new(command);
        record.id = next_id;
        rw.insert(record).map_err(db_err("写入步骤审计行"))?;
        rw.commit()
            .map_err(db_err("提交 append_change_step 事务"))?;
        Ok(())
    }

    /// 相位评估史：按 change 过滤，落行序（id 升序）返回；checklist 子行按
    /// 打包键自然序（= item_index 升序 = evaluator 输出序）内联重组。
    pub fn list_phase_records(&self, change: &str) -> Result<Vec<PhaseStateRecord>, StoreError> {
        let mut items: HashMap<i64, Vec<ChecklistItem>> = HashMap::new();
        // 主键自然序读出：同相位子行自然序即打包键序，重组保序直插
        for record in self.read_all::<ChecklistItemRecord>("遍历检查项子行")? {
            items.entry(record.phase_id).or_default().push(ChecklistItem {
                item: record.item,
                pass: record.pass,
                evidence: record.evidence,
            });
        }
        Ok(self
            .read_all::<PhaseRecord>("遍历评估条目")?
            .into_iter()
            .filter(|record| record.change == change)
            .map(|record| PhaseStateRecord {
                checklist: items.remove(&record.id).unwrap_or_default(),
                change: record.change,
                phase: record.phase,
                attempt: record.attempt,
                verdict: record.verdict,
                report: record.report,
                skipped: record.skipped,
                stale: record.stale,
                backtrack_to: record.backtrack_to,
                backtrack_reason: record.backtrack_reason,
                executor_session_id: record.executor_session_id,
                evaluator_session_id: record.evaluator_session_id,
                decision_session_id: record.decision_session_id,
                start_at: record.start_at,
                timestamp: record.timestamp,
                id: record.id,
            })
            .collect())
    }

    /// 步骤审计枚举：按 change（可选 run 圈定）过滤，落行序（id 升序）返回。
    pub fn list_change_steps(
        &self,
        change: &str,
        run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreError> {
        Ok(self
            .read_all::<StepRecord>("遍历步骤审计行")?
            .into_iter()
            .filter(|record| record.change == change)
            .filter(|record| run_id.is_none_or(|run| record.run_id == run))
            .map(|record| StepStateRecord {
                id: record.id,
                run_id: record.run_id,
                change: record.change,
                step_kind: record.step_kind,
                status: record.status,
                timestamp: record.timestamp,
                summary: record.summary,
                reference: record.reference,
            })
            .collect())
    }

    /// 本库已注册模型清单与记录计数
    pub fn list_models(&self) -> Result<Vec<ModelInfo>, StoreError> {
        envelope::list_models(&self.db, self.dimension)
    }

    /// 按模型主键自然序分页扫描（`skip(offset).take(limit)`；`limit` 上限
    /// 500 超出截断；未知模型名 Err——含跨维度模型名，维度由实例锁定）。
    /// key/value 均为 JSON 值，native_db 类型不越信封。
    pub fn scan(
        &self,
        model: &str,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<RecordEnvelope>, StoreError> {
        envelope::scan(&self.db, self.dimension, model, offset, limit)
    }

    /// 主键自然序全表读出（workspace / run 清单共用）。
    fn read_all<T: native_db::ToInput + serde::de::DeserializeOwned>(
        &self,
        context: &'static str,
    ) -> Result<Vec<T>, StoreError> {
        let r = self.db.r_transaction().map_err(db_err("开启读事务"))?;
        r.scan()
            .primary::<T>()
            .map_err(db_err(context))?
            .all()
            .map_err(db_err(context))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err(context))
    }

    /// 解析输入路径为库中已有 key（D1）：canonicalize 主口径；目录已消失
    /// （canonicalize 失败）时回退词法归一化匹配存量 key。未命中返回 None。
    fn resolve_key(&self, root: &Path) -> Result<Option<String>, StoreError> {
        match canonical::canonical_key(root) {
            Ok(canonical) => {
                let key = canonical.to_string_lossy().into_owned();
                Ok(self.contains_key(&key)?.then_some(key))
            }
            Err(_) => Ok(self
                .list_workspaces()?
                .into_iter()
                .map(|record| record.root)
                .find(|stored_key| canonical::matches_lexically(stored_key, root))),
        }
    }

    /// canonical key 是否已入库（命中判定，不取值）。
    fn contains_key(&self, key: &str) -> Result<bool, StoreError> {
        let r = self.db.r_transaction().map_err(db_err("开启读事务"))?;
        let hit: Option<WorkspaceRecord> = r.get().primary(key).map_err(db_err("读取记录"))?;
        Ok(hit.is_some())
    }
}

/// 两级库注册表（挂 Tauri State）：[`WorkspaceStores::global`] → 全局库（user
/// 维度注册表操作面），[`WorkspaceStores::for_root`] → workspace 库（per-root
/// 实例缓存复用）。缓存策略为**进程生命周期常开**（不引入 LRU）：workspace
/// 数量本机个位数，常开使重加同 root 即读即得，并免去驱逐后二次打开的锁竞争
/// 面；`remove_workspace` 不驱逐缓存实例（db 文件保留语义的进程内对应面）。
pub struct WorkspaceStores {
    /// 全局库实例（`data_root/GLOBAL_DB_FILE_NAME`，打开点 fail fast）
    global: Store,
    /// `workspaces/` 子树根（workspace 库文件派生基准，由数据目录根注入）
    workspaces_dir: PathBuf,
    /// per-root 打开实例缓存（canonical root → 实例；缓存锁跨开库持有——
    /// 并发 `for_root` 串行化，同一 db 文件进程内单开硬保证）
    cache: Mutex<HashMap<String, Arc<Store>>>,
}

impl WorkspaceStores {
    /// 打开全局库并记录 `workspaces/` 子树根：数据目录根由 desktop-app 注入
    /// （本类型零环境解析）；全局库打开失败 `Err`（setup fail fast 口径同
    /// 既有 `Store::open`）。
    pub fn open(data_root: &Path) -> Result<Self, StoreError> {
        let global = Store::open_global(&data_root.join(GLOBAL_DB_FILE_NAME))?;
        Ok(Self {
            global,
            workspaces_dir: data_root.join(WORKSPACES_DIR_NAME),
            cache: Mutex::new(HashMap::new()),
        })
    }

    /// 全局库实例（user 维度注册表操作面：workspace 三命令等全局轨）。
    pub fn global(&self) -> &Store {
        &self.global
    }

    /// 按 canonical root 解析 workspace 库实例：root 先经 canonical 口径归一
    /// （dunce，大小写 / 尾分隔符等价路径收敛同键，同根跨重开恒同名），路径
    /// 派生收口单点后开库并缓存复用——同 root 恒返回同一实例（不触发 redb
    /// 文件锁冲突），异根各自独立实例。不校验全局注册表归属：root 恒来自前端
    /// 清单（canonical root），与 cwd 同一信任级别（blank root 在命令层已拦）。
    pub fn for_root(&self, root: &str) -> Result<Arc<Store>, StoreError> {
        let key = canonical::canonical_key(Path::new(root))
            .map_err(StoreError::Canonicalize)?
            .to_string_lossy()
            .into_owned();
        let mut cache = self.cache.lock().expect("workspace 库缓存锁不可中毒");
        if let Some(store) = cache.get(&key) {
            return Ok(store.clone());
        }
        let store = Arc::new(Store::open_workspace(&workspace_db_path(
            &self.workspaces_dir,
            &key,
        ))?);
        cache.insert(key, store.clone());
        Ok(store)
    }
}

/// `Database<'static>` 与 [`WorkspaceStores`] 必须均 `Send + Sync` 才能挂
/// Tauri State（进程内 MVCC、单写多读 + per-root 缓存跨线程复用）；编译期硬
/// 校验，回归即编译失败。
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Database<'static>>();
    assert_send_sync::<WorkspaceStores>();
};
