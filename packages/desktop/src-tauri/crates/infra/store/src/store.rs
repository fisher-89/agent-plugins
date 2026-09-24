//! Store 持久化：native_db 模型层操作面、打开流程与错误面。
//!
//! 公共 API 只暴露自有类型，`native_db::Database` 等 native_db / native_model
//! 类型不出现在任何公共签名（native_db 类型不越 crate 公共面）；db 路径完全
//! 来自 [`Store::open`] 入参。

use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

use agent::AgentEvent;
use native_db::{Builder, Database, Models};

use crate::canonical;
use crate::envelope::{self, ModelInfo, RecordEnvelope};
use crate::model::{now_millis, AgentEventRecord, AgentRunRecord, WorkspaceRecord};

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

/// 全部已注册模型（静态）：`Database` 借用 `&'static Models`，进程内初始化
/// 一次。define 仅在编程错误（模型 id / version 重复）失败，expect 与
/// native_db 文档口径一致。
pub(crate) fn models() -> &'static Models {
    static MODELS: OnceLock<Models> = OnceLock::new();
    MODELS.get_or_init(|| {
        let mut models = Models::new();
        models
            .define::<WorkspaceRecord>()
            .expect("定义 WorkspaceRecord 失败");
        models
            .define::<AgentRunRecord>()
            .expect("定义 AgentRunRecord 失败");
        models
            .define::<AgentEventRecord>()
            .expect("定义 AgentEventRecord 失败");
        models
    })
}

/// `Database`（`'static` 借用静态 Models）必须 `Send + Sync` 才能挂 Tauri
/// State（进程内 MVCC、单写多读）；编译期硬校验，回归即编译失败。
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Database<'static>>();
};

/// native_db 本地库句柄：私有持有 [`Database`]，可安全挂 Tauri State，同步
/// 调用无需 async。
///
/// 单进程约束：双开（如 dev 与正式版指向同一 db 文件）不保证安全，见 crate 文档。
pub struct Store {
    db: Database<'static>,
}

impl Store {
    /// 打开（不存在则创建）db：`create_dir_all` 父目录 → 不存在（或空文件）
    /// 则 native_db create → 存在则以 native_db open。打不开即 Err（dev-team
    /// 据此 fail fast）。
    pub fn open(path: &Path) -> Result<Self, StoreError> {
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
                .create(models(), path)
                .map_err(|e| StoreError::Db(format!("创建 {} 失败: {e}", path.display())))?;
            return Ok(Self { db });
        }
        let db = Builder::new().open(models(), path).map_err(|e| {
            StoreError::Db(format!(
                "打开 {} 失败: {e}（无法识别的 db 格式）",
                path.display()
            ))
        })?;
        Ok(Self { db })
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

    /// 全部已注册模型清单与记录计数（注册表驱动，计数 0 也列出；新模型登记
    /// 注册表一行即覆盖，见 [`crate::envelope`]）。
    pub fn list_models(&self) -> Result<Vec<ModelInfo>, StoreError> {
        envelope::list_models(&self.db)
    }

    /// 按模型主键自然序分页扫描（`skip(offset).take(limit)`；`limit` 上限
    /// 500 超出截断；未知模型名 Err）。key/value 均为 JSON 值，native_db
    /// 类型不越信封。
    pub fn scan(
        &self,
        model: &str,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<RecordEnvelope>, StoreError> {
        envelope::scan(&self.db, model, offset, limit)
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
