//! 记录信封 API：穿透模型层类型壁垒的通用只读读面。
//!
//! 扩展机制 = [`MODEL_ENTRIES`] 静态注册表（`name → { count, scan, key_of }`
//! fn-pointer 三元组）：native_db 的 `len` / 扫描按类型静态分派，运行时反射
//! 不存在，注册表即「新模型 = 定义 struct + 登记一行」的最小机制，也是查看
//! 器零模型特定代码的落点。native_db 类型不越信封——信封值经 serde_json
//! 编解码（记录落库为 native_model bincode，此处解码回 JSON 值，人可读）。

use native_db::{Database, ToInput};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{AgentEventRecord, AgentRunRecord, ExploreRecord, WorkspaceRecord};
use crate::store::{db_err, StoreError};

/// 模型清单一行：模型名 + 记录计数（计数 0 也列出）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// 模型名（查看器展示与 `Store::scan` 入参同源）
    pub name: String,
    /// 记录计数
    pub count: u64,
}

/// 记录信封：key / value 均为 JSON 值（native_db 类型不越信封，无二进制）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordEnvelope {
    /// 记录主键的 JSON 形态
    pub key: Value,
    /// 记录本体（serde 转 JSON 值）
    pub value: Value,
}

/// 记录计数 fn-pointer：走 `Database::len` 主键口径。
type CountFn = fn(&Database<'static>) -> Result<u64, StoreError>;
/// 分页扫描 fn-pointer：主键自然序 `skip(offset).take(limit)`；`key_of` 由
/// 分发方从注册表行传入。
type ScanFn = fn(&Database<'static>, u32, u32, KeyOfFn) -> Result<Vec<RecordEnvelope>, StoreError>;
/// 记录主键 JSON 构造 fn-pointer：入参为记录序列化后的 JSON 值。
type KeyOfFn = fn(&Value) -> Value;

/// 注册表一行（三元组）。
struct ModelEntry {
    name: &'static str,
    count: CountFn,
    scan: ScanFn,
    key_of: KeyOfFn,
}

/// 单页扫描上限：超出截断（查看器单页上限，防误传大 limit 拖垮 IPC）。
const MAX_SCAN_LIMIT: u32 = 500;

/// 静态模型注册表。新模型接入点 = 此处登记一行。
const MODEL_ENTRIES: &[ModelEntry] = &[
    ModelEntry {
        name: "workspace",
        count: count_model::<WorkspaceRecord>,
        scan: scan_workspaces,
        key_of: workspace_key,
    },
    ModelEntry {
        name: "agent_run",
        count: count_model::<AgentRunRecord>,
        scan: scan_agent_runs,
        key_of: agent_run_key,
    },
    ModelEntry {
        name: "agent_event",
        count: count_model::<AgentEventRecord>,
        scan: scan_agent_events,
        key_of: agent_event_key,
    },
    ModelEntry {
        name: "explore",
        count: count_model::<ExploreRecord>,
        scan: scan_explores,
        key_of: explore_key,
    },
];

/// 全部已注册模型清单与计数（注册表全量分发）。
pub(crate) fn list_models(db: &Database<'static>) -> Result<Vec<ModelInfo>, StoreError> {
    MODEL_ENTRIES
        .iter()
        .map(|entry| {
            (entry.count)(db).map(|count| ModelInfo {
                name: entry.name.to_owned(),
                count,
            })
        })
        .collect()
}

/// 按模型名分页扫描记录信封；未知模型名 Err。
pub(crate) fn scan(
    db: &Database<'static>,
    model: &str,
    offset: u32,
    limit: u32,
) -> Result<Vec<RecordEnvelope>, StoreError> {
    let entry = MODEL_ENTRIES
        .iter()
        .find(|entry| entry.name == model)
        .ok_or_else(|| StoreError::Db(format!("未知模型: {model}")))?;
    (entry.scan)(db, offset, limit.min(MAX_SCAN_LIMIT), entry.key_of)
}

/// 主键自然序分页扫描（`skip(offset).take(limit)`），记录经 serde 转 JSON
/// 值组装信封（key 由 `key_of` 从记录 JSON 构造）。
fn scan_model<T: ToInput + Serialize>(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<Vec<RecordEnvelope>, StoreError> {
    let r = db.r_transaction().map_err(db_err("开启读事务"))?;
    let page: Vec<T> = r
        .scan()
        .primary::<T>()
        .map_err(db_err("扫描记录"))?
        .all()
        .map_err(db_err("扫描记录"))?
        .skip(offset as usize)
        .take(limit as usize)
        .collect::<native_db::db_type::Result<Vec<_>>>()
        .map_err(db_err("扫描记录"))?;
    page.into_iter()
        .map(|record| {
            let value = serde_json::to_value(&record)
                .map_err(|e| StoreError::Db(format!("编码记录信封: {e}")))?;
            Ok(RecordEnvelope {
                key: key_of(&value),
                value,
            })
        })
        .collect()
}

/// 记录计数（`Database::len` 主键口径）。
fn count_model<T: ToInput>(db: &Database<'static>) -> Result<u64, StoreError> {
    let r = db.r_transaction().map_err(db_err("开启读事务"))?;
    r.len().primary::<T>().map_err(db_err("统计记录数"))
}

// --- per-model 包装：注册表行 = 通用实现 + 本模型 key 构造，一一对应 ---

fn scan_workspaces(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<Vec<RecordEnvelope>, StoreError> {
    scan_model::<WorkspaceRecord>(db, offset, limit, key_of)
}

fn scan_agent_runs(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<Vec<RecordEnvelope>, StoreError> {
    scan_model::<AgentRunRecord>(db, offset, limit, key_of)
}

fn scan_agent_events(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<Vec<RecordEnvelope>, StoreError> {
    scan_model::<AgentEventRecord>(db, offset, limit, key_of)
}

fn scan_explores(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<Vec<RecordEnvelope>, StoreError> {
    scan_model::<ExploreRecord>(db, offset, limit, key_of)
}

/// workspace 主键 = canonical root。
fn workspace_key(value: &Value) -> Value {
    value["root"].clone()
}

/// agent run 主键 = id。
fn agent_run_key(value: &Value) -> Value {
    value["id"].clone()
}

/// agent event 主键还原 `{runId, seq}` 形态（u128 打包键的 JSON 可读投影）。
fn agent_event_key(value: &Value) -> Value {
    serde_json::json!({ "runId": value["runId"], "seq": value["event"]["seq"] })
}

/// explore 主键 = id。
fn explore_key(value: &Value) -> Value {
    value["id"].clone()
}
