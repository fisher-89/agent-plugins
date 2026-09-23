//! Store 持久化：表定义、事务读写、schema_version 写入/校验、错误面。
//!
//! 公共 API 只暴露自有类型，`redb::Database` / `Table` 不出现在任何公共
//! 签名（redb 类型不越 crate 公共面）；db 路径完全来自 [`Store::open`] 入参。

use std::fmt;
use std::path::Path;

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use crate::canonical;
use crate::model::{now_millis, AgentRunRecord, WorkspaceRecord};

/// user 维度注册表：key = canonical root path，value = `WorkspaceRecord` JSON。
const USER_WORKSPACES: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new("user_workspaces");

/// user 维度 agent 运行元数据：key = run id（写事务内 max+1 分配），
/// value = `AgentRunRecord` JSON。
const USER_AGENT_RUNS: TableDefinition<'static, i64, &[u8]> =
    TableDefinition::new("user_agent_runs");

/// user 维度 agent 运行事件流：key = `(run_id, seq)` 复合键（seq 取事件自带
/// 值，天然有序），value = 单个事件 JSON 字节串。事件以 `serde_json::Value`
/// 进出（store 禁依赖 core 契约 crate）。
const USER_AGENT_RUN_EVENTS: TableDefinition<'static, (i64, u64), &[u8]> =
    TableDefinition::new("user_agent_run_events");

/// META 表：独立于业务表，自第一天起存 `schema_version`（redb 无内建迁移）。
const USER_META: TableDefinition<'static, &str, u64> = TableDefinition::new("user_meta");

/// 当前库 schema 版本；将来演进时版本升级 + 迁移逻辑在 [`Store::init_schema`] 收口。
const SCHEMA_VERSION: u64 = 1;

const SCHEMA_VERSION_KEY: &str = "schema_version";

/// store 内部错误面：两变体对应两类故障模式；`Display` 恒带 `db:` /
/// `canonicalize:` 前缀，直接服务「清单丢失」的可排查性。
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

/// redb 事务/读写错误统一收敛为 [`StoreError::Db`]，错误串携带操作语境。
fn db_err<E: fmt::Display>(context: &str) -> impl Fn(E) -> StoreError + '_ {
    move |e| StoreError::Db(format!("{context}: {e}"))
}

/// redb 本地库句柄：私有持有 [`Database`]，`Send + Sync`（进程内 MVCC，
/// 单写多读），可安全挂 Tauri State，同步调用无需 async。
///
/// 单进程约束：双开（如 dev 与正式版指向同一 db 文件）不保证安全，见 crate 文档。
pub struct Store {
    db: Database,
}

impl Store {
    /// 打开（不存在则创建）db：`create_dir_all` 父目录 + `Database::create`
    /// + `schema_version` 写入/校验。打不开即 Err（dev-team 据此 fail fast）。
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        // redb 建文件不建父目录，首启必须补齐；裸文件名（无父目录）跳过
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    StoreError::Db(format!("创建 db 父目录 {} 失败: {e}", parent.display()))
                })?;
            }
        }
        let db = Database::create(path)
            .map_err(|e| StoreError::Db(format!("打开 {} 失败: {e}", path.display())))?;
        let store = Self { db };
        store.init_schema()?;
        Ok(store)
    }

    /// `schema_version` 写入/校验：absent → 写入当前版本；已存在且高于支持
    /// 版本 → Err（由更新版本应用创建的库，拒绝降版本打开）。
    /// 同一事务顺手打开业务表，保证后续 list 的读事务恒可打开 `user_workspaces`。
    fn init_schema(&self) -> Result<(), StoreError> {
        let write_txn = self.db.begin_write().map_err(db_err("开启写事务"))?;
        {
            let _ = write_txn
                .open_table(USER_WORKSPACES)
                .map_err(db_err("打开 user_workspaces 表"))?;
            // 顺手打开 agent 两表：保证后续 list / 重放的读事务恒可打开
            let _ = write_txn
                .open_table(USER_AGENT_RUNS)
                .map_err(db_err("打开 user_agent_runs 表"))?;
            let _ = write_txn
                .open_table(USER_AGENT_RUN_EVENTS)
                .map_err(db_err("打开 user_agent_run_events 表"))?;
            let mut meta = write_txn
                .open_table(USER_META)
                .map_err(db_err("打开 user_meta 表"))?;
            // 先拷出值结束借用，再做写入分支（scrutinee 临时借用不得跨 insert）
            let stored = meta
                .get(SCHEMA_VERSION_KEY)
                .map_err(db_err("读取 schema_version"))?
                .map(|guard| guard.value());
            match stored {
                Some(found) => {
                    if found > SCHEMA_VERSION {
                        return Err(StoreError::Db(format!(
                            "schema_version {found} 高于支持版本 {SCHEMA_VERSION}（db 由更新版本的应用创建）"
                        )));
                    }
                    // 低于当前版本的存量库：将来迁移在此收口；当前单版本无迁移动作
                }
                None => {
                    meta.insert(SCHEMA_VERSION_KEY, SCHEMA_VERSION)
                        .map_err(db_err("写入 schema_version"))?;
                }
            }
        }
        write_txn
            .commit()
            .map_err(db_err("提交 schema_version 事务"))?;
        Ok(())
    }

    /// canonicalize + upsert + touch：已存在 → 保留 `added_at`、刷新
    /// `last_opened_at`；新建 → 两值同为 now。返回落库后的记录
    /// （canonical root，前端以此为当前根，展示与库内 key 同源）。
    pub fn add_workspace(&self, root: &Path) -> Result<WorkspaceRecord, StoreError> {
        let key = canonical::canonical_key(root)
            .map_err(StoreError::Canonicalize)?
            .to_string_lossy()
            .into_owned();
        let now = now_millis();
        let write_txn = self.db.begin_write().map_err(db_err("开启写事务"))?;
        let record = {
            let mut table = write_txn
                .open_table(USER_WORKSPACES)
                .map_err(db_err("打开 user_workspaces 表"))?;
            let record = match table.get(key.as_str()).map_err(db_err("读取已有记录"))? {
                // 等价路径已入库：保留 added_at，仅刷新 last_opened_at（insert 即 upsert）
                Some(guard) => {
                    let mut existing =
                        WorkspaceRecord::decode(guard.value()).map_err(db_err("解码已有记录"))?;
                    existing.last_opened_at = now;
                    existing
                }
                None => WorkspaceRecord::from_root(&key, now),
            };
            let bytes = record.encode().map_err(db_err("编码记录"))?;
            table
                .insert(key.as_str(), bytes.as_slice())
                .map_err(db_err("写入记录"))?;
            record
        };
        write_txn
            .commit()
            .map_err(db_err("提交 add_workspace 事务"))?;
        Ok(record)
    }

    /// 清单：`last_opened_at` 降序，并列按 root 字典序升序（顺序确定）；
    /// 第一名即「上次打开」，不为此单设 API。
    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceRecord>, StoreError> {
        let read_txn = self.db.begin_read().map_err(db_err("开启读事务"))?;
        let table = read_txn
            .open_table(USER_WORKSPACES)
            .map_err(db_err("打开 user_workspaces 表"))?;
        let mut records = Vec::new();
        for entry in table.iter().map_err(db_err("遍历清单"))? {
            let (_, value) = entry.map_err(db_err("读取记录"))?;
            records.push(WorkspaceRecord::decode(value.value()).map_err(db_err("解码记录"))?);
        }
        records.sort_by(|a, b| {
            b.last_opened_at
                .cmp(&a.last_opened_at)
                .then_with(|| a.root.cmp(&b.root))
        });
        Ok(records)
    }

    /// 按 canonical key 删除（含消失目录的回退匹配）；miss 幂等 `Ok(false)`。
    pub fn remove_workspace(&self, root: &Path) -> Result<bool, StoreError> {
        let Some(key) = self.resolve_key(root)? else {
            return Ok(false);
        };
        let write_txn = self.db.begin_write().map_err(db_err("开启写事务"))?;
        let hit = {
            let mut table = write_txn
                .open_table(USER_WORKSPACES)
                .map_err(db_err("打开 user_workspaces 表"))?;
            let removed = table.remove(key.as_str()).map_err(db_err("删除记录"))?;
            removed.is_some()
        };
        write_txn
            .commit()
            .map_err(db_err("提交 remove_workspace 事务"))?;
        Ok(hit)
    }

    /// 刷新 `last_opened_at`；miss 幂等 `Ok(false)`，不算错误。
    pub fn touch_workspace(&self, root: &Path) -> Result<bool, StoreError> {
        let Some(key) = self.resolve_key(root)? else {
            return Ok(false);
        };
        let write_txn = self.db.begin_write().map_err(db_err("开启写事务"))?;
        let hit = {
            let mut table = write_txn
                .open_table(USER_WORKSPACES)
                .map_err(db_err("打开 user_workspaces 表"))?;
            // 先读后写分两步：读守卫的借用不得跨入 insert（scrutinee 临时借用）
            let updated = match table.get(key.as_str()).map_err(db_err("读取记录"))? {
                Some(guard) => {
                    let mut record =
                        WorkspaceRecord::decode(guard.value()).map_err(db_err("解码记录"))?;
                    record.last_opened_at = now_millis();
                    Some(record.encode().map_err(db_err("编码记录"))?)
                }
                None => None,
            };
            let hit = updated.is_some();
            if let Some(bytes) = updated {
                table
                    .insert(key.as_str(), bytes.as_slice())
                    .map_err(db_err("写入记录"))?;
            }
            hit
        };
        write_txn
            .commit()
            .map_err(db_err("提交 touch_workspace 事务"))?;
        Ok(hit)
    }

    /// 新开一次 agent 运行：写事务内 `max(id)+1` 分配 id（与插入原子，首行
    /// id=1），落 `running` 行，返回含 id 的记录。调用方填充 prompt / cwd /
    /// env / permission_mode / status / started_at。
    pub fn begin_agent_run(&self, run: &AgentRunRecord) -> Result<AgentRunRecord, StoreError> {
        let write_txn = self.db.begin_write().map_err(db_err("开启写事务"))?;
        let record = {
            let mut table = write_txn
                .open_table(USER_AGENT_RUNS)
                .map_err(db_err("打开 user_agent_runs 表"))?;
            let next_id = match table.last().map_err(db_err("读取最大 id"))? {
                Some((key, _)) => key.value() + 1,
                None => 1,
            };
            let mut record = run.clone();
            record.id = next_id;
            let bytes = record.encode().map_err(db_err("编码 run 记录"))?;
            table
                .insert(next_id, bytes.as_slice())
                .map_err(db_err("写入 run 记录"))?;
            record
        };
        write_txn
            .commit()
            .map_err(db_err("提交 begin_agent_run 事务"))?;
        Ok(record)
    }

    /// 批量追加运行事件：单事务写入；key `(run_id, seq)` 复合键，seq 取事件
    /// 自带值。事件以 `serde_json::Value` 进出（缺失 seq 视作编码层损坏）。
    pub fn append_agent_run_events(
        &self,
        run_id: i64,
        events: &[serde_json::Value],
    ) -> Result<(), StoreError> {
        let write_txn = self.db.begin_write().map_err(db_err("开启写事务"))?;
        {
            let mut table = write_txn
                .open_table(USER_AGENT_RUN_EVENTS)
                .map_err(db_err("打开 user_agent_run_events 表"))?;
            for event in events {
                let seq = event
                    .get("seq")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| StoreError::Db("事件缺少 seq 字段".to_owned()))?;
                let bytes = serde_json::to_vec(event).map_err(db_err("编码事件"))?;
                table
                    .insert((run_id, seq), bytes.as_slice())
                    .map_err(db_err("写入事件"))?;
            }
        }
        write_txn.commit().map_err(db_err("提交事件追加事务"))?;
        Ok(())
    }

    /// 收敛 run 终态：以传入记录整行替换（status / finished_at / 汇总 /
    /// error 由调用方填充）。
    pub fn finish_agent_run(&self, run_id: i64, record: &AgentRunRecord) -> Result<(), StoreError> {
        let write_txn = self.db.begin_write().map_err(db_err("开启写事务"))?;
        {
            let mut table = write_txn
                .open_table(USER_AGENT_RUNS)
                .map_err(db_err("打开 user_agent_runs 表"))?;
            let bytes = record.encode().map_err(db_err("编码 run 记录"))?;
            table
                .insert(run_id, bytes.as_slice())
                .map_err(db_err("写入 run 记录"))?;
        }
        write_txn
            .commit()
            .map_err(db_err("提交 finish_agent_run 事务"))?;
        Ok(())
    }

    /// 运行清单：`started_at` 降序，并列按 id 降序（顺序确定，与 workspace
    /// 清单同哲学）。
    pub fn list_agent_runs(&self) -> Result<Vec<AgentRunRecord>, StoreError> {
        let read_txn = self.db.begin_read().map_err(db_err("开启读事务"))?;
        let table = read_txn
            .open_table(USER_AGENT_RUNS)
            .map_err(db_err("打开 user_agent_runs 表"))?;
        let mut records = Vec::new();
        for entry in table.iter().map_err(db_err("遍历运行清单"))? {
            let (_, value) = entry.map_err(db_err("读取 run 记录"))?;
            records.push(AgentRunRecord::decode(value.value()).map_err(db_err("解码 run 记录"))?);
        }
        records.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(records)
    }

    /// 单 run 事件重放：`(run_id, seq)` 半开区间扫描，seq 升序返回。
    pub fn list_agent_run_events(&self, run_id: i64) -> Result<Vec<serde_json::Value>, StoreError> {
        let read_txn = self.db.begin_read().map_err(db_err("开启读事务"))?;
        let table = read_txn
            .open_table(USER_AGENT_RUN_EVENTS)
            .map_err(db_err("打开 user_agent_run_events 表"))?;
        let mut events = Vec::new();
        let range_start = (run_id, 0u64);
        let range_end = (run_id.saturating_add(1), 0u64);
        for entry in table
            .range(range_start..range_end)
            .map_err(db_err("扫描运行事件"))?
        {
            let (_, value) = entry.map_err(db_err("读取事件"))?;
            let event = serde_json::from_slice(value.value()).map_err(db_err("解码事件"))?;
            events.push(event);
        }
        Ok(events)
    }

    /// 解析输入路径为库中已有 key（D1）：canonicalize 主口径；目录已消失
    /// （canonicalize 失败）时回退词法归一化匹配存量 key。未命中返回 None。
    fn resolve_key(&self, root: &Path) -> Result<Option<String>, StoreError> {
        match canonical::canonical_key(root) {
            Ok(canonical) => {
                let key = canonical.to_string_lossy().into_owned();
                Ok(self.contains_key(&key)?.then_some(key))
            }
            Err(_) => {
                let read_txn = self.db.begin_read().map_err(db_err("开启读事务"))?;
                let table = read_txn
                    .open_table(USER_WORKSPACES)
                    .map_err(db_err("打开 user_workspaces 表"))?;
                for entry in table.iter().map_err(db_err("遍历清单"))? {
                    let (stored_key, _) = entry.map_err(db_err("读取记录"))?;
                    if canonical::matches_lexically(stored_key.value(), root) {
                        return Ok(Some(stored_key.value().to_owned()));
                    }
                }
                Ok(None)
            }
        }
    }

    /// canonical key 是否已入库（命中判定，不取值）。
    fn contains_key(&self, key: &str) -> Result<bool, StoreError> {
        let read_txn = self.db.begin_read().map_err(db_err("开启读事务"))?;
        let table = read_txn
            .open_table(USER_WORKSPACES)
            .map_err(db_err("打开 user_workspaces 表"))?;
        Ok(table.get(key).map_err(db_err("读取记录"))?.is_some())
    }
}
