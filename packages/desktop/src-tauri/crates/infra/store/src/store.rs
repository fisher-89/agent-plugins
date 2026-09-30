//! Store 持久化：native_db 模型层操作面、双库布局打开流程、workspace 库路径
//! 派生单点与错误面。
//!
//! 公共 API 只暴露自有类型，`native_db::Database` 等 native_db / native_model
//! 类型不出现在任何公共签名（native_db 类型不越 crate 公共面）；两库路径均由
//! 调用方注入——全局库路径来自 [`WorkspaceStores::open`] 的数据目录根入参，
//! workspace 库路径经本模块派生单点从 canonical root 确定性导出（消费侧零
//! 派生逻辑）。

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use agent::AgentEvent;
use native_db::{Builder, Database, Models};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;

use crate::canonical;
use crate::envelope::{self, ModelInfo, RecordEnvelope};
use crate::model::{
    dir_name, now_millis, AgentEngineKind, AgentEventRecord, AgentEventRecordKey,
    AgentInstanceRecord, AgentProviderRecord, AgentRunRecord, ExploreRecord, WorkspaceRecord,
};

/// store 内部错误面：两变体对应两类故障模式；`Display` 恒带 `db:` /
/// `canonicalize:` 前缀，直接服务「清单丢失」的可排查性。迁移失败经
/// [`StoreError::Db`]（`迁移:` 语境前缀）呈现，不扩变体。
///
/// 命令层以 `.to_string()` 转换为 `Err(String)`，本类型不进入命令签名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// db 打开 / 事务 / 读写失败
    Db(String),
    /// 路径 canonicalize 失败
    Canonicalize(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(msg) => write!(f, "db: {msg}"),
            Self::Canonicalize(msg) => write!(f, "canonicalize: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// 引擎错误（native_db `db_type::Error` 等 `Display` 错误）统一收敛为
/// [`StoreError::Db`]，错误串携带操作语境。
pub(crate) fn db_err<E: fmt::Display>(context: &str) -> impl Fn(E) -> StoreError + '_ {
    move |e| StoreError::Db(format!("{context}: {e}"))
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

/// explore 来源受控字符串：与 app 层 `RunProvenance` 及前端 `EXPLORE_SOURCE`
/// 口径一致（三处同字面量，改动需同步）。`delete_explore_record` 据此圈定
/// 级联删除的 runs。
const EXPLORE_RUN_SOURCE: &str = "explore";

/// 数据维度（信封维度标签 + db 查看命令 scope 入参双职）：`User` 全局库 /
/// `Workspace` workspace 库。serde 线值为 `"user"` / `"workspace"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum DbDimension {
    /// user 维度（全局库，`WorkspaceRecord` 与 agent 管理两模型——user 维度
    /// 已落地代表，desktop-data-dimensions 留痕）
    User,
    /// workspace 维度（per-workspace 库，run / 事件 / explore 三模型）
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

/// workspace 库模型组
pub(crate) fn workspace_models() -> &'static Models {
    static MODELS: OnceLock<Models> = OnceLock::new();
    MODELS.get_or_init(|| {
        let mut models = Models::new();
        models
            .define::<AgentRunRecord>()
            .expect("定义 AgentRunRecord 失败");
        models
            .define::<AgentEventRecord>()
            .expect("定义 AgentEventRecord 失败");
        models
            .define::<ExploreRecord>()
            .expect("定义 ExploreRecord 失败");
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
/// 管理三模型）、workspace 库经 [`Store::open_workspace`]（run / 事件 /
/// explore 三模型）。
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

    /// 打开 workspace 库（workspace 维度模型组，run / 事件 / explore 三模型）。
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

    /// 新开一次 agent 运行：写事务内 `max(id)+1` 分配 id（与插入原子，首行
    /// id=1），落 `running` 行，返回含 id 的记录。调用方填充 prompt / cwd /
    /// env / permission_mode / status / started_at。
    pub fn begin_agent_run(&self, run: &AgentRunRecord) -> Result<AgentRunRecord, StoreError> {
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
        let mut record = run.clone();
        record.id = next_id;
        rw.insert(record.clone()).map_err(db_err("写入 run 记录"))?;
        rw.commit().map_err(db_err("提交 begin_agent_run 事务"))?;
        Ok(record)
    }

    /// 收敛 run 终态：以传入记录整行替换（status / finished_at / 汇总 /
    /// error 由调用方填充）。
    pub fn finish_agent_run(&self, run_id: i64, record: &AgentRunRecord) -> Result<(), StoreError> {
        if record.id != run_id {
            return Err(StoreError::Db(format!(
                "run id 不匹配: 记录 id {} ≠ 目标 run id {run_id}",
                record.id
            )));
        }
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        rw.upsert(record.clone()).map_err(db_err("写入 run 记录"))?;
        rw.commit().map_err(db_err("提交 finish_agent_run 事务"))?;
        Ok(())
    }

    /// 运行清单：`started_at` 降序，并列按 id 降序（顺序确定，与 workspace
    /// 清单同哲学）。全表读 + 内存排序——调试页数据量小，行为零变化优先；
    /// 二级索引查询形态由 `AgentEventRecord.run_id` 兑现。
    pub fn list_agent_runs(&self) -> Result<Vec<AgentRunRecord>, StoreError> {
        let mut records = self.read_all::<AgentRunRecord>("遍历运行清单")?;
        records.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(records)
    }

    /// 批量追加运行事件：单事务写入；`event_key` 由 `(run_id, seq)` 打包
    /// （seq 取自事件本体，不存在「缺 seq」错误路径）。
    pub fn append_agent_run_events(
        &self,
        run_id: i64,
        events: &[AgentEvent],
    ) -> Result<(), StoreError> {
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        for event in events {
            let record = AgentEventRecord::new(run_id, event.clone());
            rw.insert(record).map_err(db_err("写入事件"))?;
        }
        rw.commit().map_err(db_err("提交事件追加事务"))?;
        Ok(())
    }

    /// 单 run 事件重放：经 `run_id` 非唯一二级索引扫描，seq 升序返回
    /// （打包主键大端序保证同 run 内自然序即重放序）。
    pub fn list_agent_run_events(&self, run_id: i64) -> Result<Vec<AgentEvent>, StoreError> {
        let r = self.db.r_transaction().map_err(db_err("开启读事务"))?;
        let records: Vec<AgentEventRecord> = r
            .scan()
            .secondary(crate::model::AgentEventRecordKey::run_id)
            .map_err(db_err("扫描运行事件"))?
            .range(run_id..run_id.saturating_add(1))
            .map_err(db_err("扫描运行事件"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描运行事件"))?;
        Ok(records.into_iter().map(|record| record.event).collect())
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
    /// 删除方向亦然）；记录名下的会话 runs 及其事件随记录**同事务级联删除**
    /// ——id 是幸存行上 max+1 的可复用计数，悬空 `source_ref` 不清则下次建档
    /// 复用 id 时旧聊天经 `(source, source_ref)` 匹配错挂到新记录。miss 幂等
    /// `Ok(false)`。
    pub fn delete_explore_record(&self, root: &str, name: &str) -> Result<bool, StoreError> {
        let Some(record) = self.find_explore_record(root, name)? else {
            return Ok(false);
        };
        let rw = self.db.rw_transaction().map_err(db_err("开启写事务"))?;
        // 级联圈定：explore 来源且 source_ref 指向本记录 id 的 runs（全表读 +
        // 内存过滤，与清单读面同哲学——调试页数据量小）
        let source_ref = record.id.to_string();
        let bound_runs: Vec<AgentRunRecord> = rw
            .scan()
            .primary::<AgentRunRecord>()
            .map_err(db_err("扫描运行清单"))?
            .all()
            .map_err(db_err("扫描运行清单"))?
            .collect::<native_db::db_type::Result<Vec<_>>>()
            .map_err(db_err("扫描运行清单"))?
            .into_iter()
            .filter(|run| {
                run.source == EXPLORE_RUN_SOURCE && run.source_ref.as_deref() == Some(&source_ref)
            })
            .collect();
        for run in &bound_runs {
            let events: Vec<AgentEventRecord> = rw
                .scan()
                .secondary(AgentEventRecordKey::run_id)
                .map_err(db_err("扫描运行事件"))?
                .range(run.id..run.id.saturating_add(1))
                .map_err(db_err("扫描运行事件"))?
                .collect::<native_db::db_type::Result<Vec<_>>>()
                .map_err(db_err("扫描运行事件"))?;
            for event in events {
                rw.remove(event).map_err(db_err("删除运行事件"))?;
            }
            rw.remove(run.clone()).map_err(db_err("删除运行记录"))?;
        }
        rw.remove(record).map_err(db_err("删除探索记录"))?;
        rw.commit()
            .map_err(db_err("提交 delete_explore_record 事务"))?;
        Ok(true)
    }

    /// 单链还原（链查询收口单点，前端 hook 不拼链）：按 `(source, source_ref)`
    /// 过滤 → `(started_at, id)` 最新为链头 → 沿 `parent_run_id` 回溯整链
    /// （visited 集防环）→ 反转为发起顺序。无链返回空 `Vec`。
    pub fn restore_run_chain(
        &self,
        source: &str,
        source_ref: &str,
    ) -> Result<Vec<AgentRunRecord>, StoreError> {
        let all = self.read_all::<AgentRunRecord>("遍历运行清单")?;
        let by_id: HashMap<i64, &AgentRunRecord> =
            all.iter().map(|record| (record.id, record)).collect();
        let head = all
            .iter()
            .filter(|record| {
                record.source == source && record.source_ref.as_deref() == Some(source_ref)
            })
            .max_by_key(|record| (record.started_at, record.id));
        let Some(head) = head else {
            return Ok(Vec::new());
        };
        let mut chain = Vec::new();
        let mut visited: HashSet<i64> = HashSet::new();
        let mut current = Some(head);
        while let Some(record) = current {
            if !visited.insert(record.id) {
                break; // 环防御：指针成环时截断，不无限回溯
            }
            chain.push(record.clone());
            current = record
                .parent_run_id
                .and_then(|pid| by_id.get(&pid).copied());
        }
        chain.reverse();
        Ok(chain)
    }

    /// provider 清单：主键 id 升序自然序（稳定可复现）。
    pub fn list_agent_providers(&self) -> Result<Vec<AgentProviderRecord>, StoreError> {
        self.read_all::<AgentProviderRecord>("遍历 provider 清单")
    }

    /// 主键直查 provider（运行发起解析与 save 回填原值消费）。
    pub fn find_agent_provider(&self, id: i64) -> Result<Option<AgentProviderRecord>, StoreError> {
        let r = self.db.r_transaction().map_err(db_err("开启读事务"))?;
        let hit: Option<AgentProviderRecord> = r
            .get()
            .primary(id)
            .map_err(db_err("读取 provider 记录"))?;
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
        let hit: Option<AgentInstanceRecord> = r
            .get()
            .primary(id)
            .map_err(db_err("读取 agent 记录"))?;
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
