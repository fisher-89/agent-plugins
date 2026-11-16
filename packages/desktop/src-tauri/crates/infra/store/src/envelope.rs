//! 记录信封 API：穿透模型层类型壁垒的通用只读读面。
//!
//! 扩展机制 = [`MODEL_ENTRIES`] 静态注册表（`name → { count, scan, key_of }`
//! fn-pointer 三元组）：native_db 的 `len` / 扫描按类型静态分派，运行时反射
//! 不存在，注册表即「新模型 = 定义 struct + 登记一行」的最小机制，也是查看
//! 器零模型特定代码的落点。native_db 类型不越信封——信封值经 serde_json
//! 编解码（记录落库为 native_model bincode，此处解码回 JSON 值，人可读）。
//! 分页扫描随页返回 [`RecordsPage`] 分页信封（records + total 同一读事务
//! 快照），页界元数据是接口一等契约。

use native_db::{Database, ToInput};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use crate::describe;
use crate::model::{
    AgentInstanceRecord, AgentProviderRecord, AgentRunRecord, ChangeRecord, ChecklistItemRecord,
    ExploreRecord, PhaseRecord, RunEdgeRecord, RunNodeRecord, RunRecord, SessionEventRecord,
    SessionRecord, StepRecord, WorkspaceRecord,
};
use crate::query::{db_err as query_db_err, QueryError};
use crate::store::{db_err, DbDimension, StoreError};

/// 模型清单一行：模型名 + 记录计数（计数 0 也列出）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// 模型名（查看器展示与 `Store::scan` 入参同源）
    pub name: String,
    /// 记录计数
    pub count: u64,
}

/// 记录信封：key / value 均为 JSON 值（native_db 类型不越信封，无二进制）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecordEnvelope {
    /// 记录主键的 JSON 形态
    pub key: Value,
    /// 记录本体（serde 转 JSON 值）
    pub value: Value,
}

/// 分页信封：offset / limit 区间记录 + 该模型全量计数（二者同一读事务
/// 快照——并发写下页界与行集自洽，`total` 是页界唯一事实源）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecordsPage {
    /// 区间记录信封
    pub records: Vec<RecordEnvelope>,
    /// 该模型全量计数
    pub total: u64,
}

/// 记录计数 fn-pointer：走 `Database::len` 主键口径。
type CountFn = fn(&Database<'static>) -> Result<u64, StoreError>;
/// 分页扫描 fn-pointer：主键自然序 `skip(offset).take(limit)`，随页返回
/// 分页信封（records + total 同一读事务快照）；`key_of` 由分发方从注册表行
/// 传入。
type ScanFn = fn(&Database<'static>, u32, u32, KeyOfFn) -> Result<RecordsPage, StoreError>;
/// 记录主键 JSON 构造 fn-pointer：入参为记录序列化后的 JSON 值。
type KeyOfFn = fn(&Value) -> Value;

/// 类型图注册 fn-pointer：把本模型（及其依赖的命名类型）灌进 `Types`，返回其
/// 根 `DataType`——`describe` 据此取表介绍 / 字段线名 / 字段类型 / 字段说明。
pub(crate) type SchemaFn = fn(&mut specta::Types) -> specta::datatype::DataType;

/// 记录流式遍历 fn-pointer：**一个读事务内**逐条解码 → JSON → 交访问器（入参
/// `(key, value)`，`key` 由 `key_of` 从记录 JSON 构造，与信封同形；返回 `false`
/// 即提前终止）。返回遍历到的总行数（提前终止时为下界）。
pub(crate) type WalkFn = fn(
    &Database<'static>,
    KeyOfFn,
    &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError>;

/// 注册表一行：`dimension` 维度标签——`list_models` / `scan` 据实例维度过滤
/// 本库模型，分维度是数据行非分支代码（查看器零模型特定代码不变）；`schema` /
/// `walk` 是「表 ↔ 类型」与「表 ↔ 遍历」两组映射，与 `count` / `scan` / `key_of`
/// 同住一行——同一份登记喂全部读面，MUST NOT 出现第二份字段表或 per-table
/// 手写清单。
pub(crate) struct ModelEntry {
    pub(crate) name: &'static str,
    pub(crate) dimension: DbDimension,
    pub(crate) count: CountFn,
    pub(crate) scan: ScanFn,
    pub(crate) key_of: KeyOfFn,
    pub(crate) schema: SchemaFn,
    pub(crate) walk: WalkFn,
}

/// 单页扫描上限：超出截断（查看器单页上限，防误传大 limit 拖垮 IPC）。
const MAX_SCAN_LIMIT: u32 = 500;

/// 静态模型注册表（dimension 与模型注册分组一一对应：全局库组仅 user 维度，
/// workspace 库组仅 workspace 维度，无交叉）。新模型接入点 = 此处登记一行。
const MODEL_ENTRIES: &[ModelEntry] = &[
    ModelEntry {
        name: "workspace",
        dimension: DbDimension::User,
        count: count_model::<WorkspaceRecord>,
        scan: scan_workspaces,
        key_of: workspace_key,
        schema: describe::schema_of::<WorkspaceRecord>,
        walk: walk_workspaces,
    },
    ModelEntry {
        name: "agent_provider",
        dimension: DbDimension::User,
        count: count_model::<AgentProviderRecord>,
        scan: scan_agent_providers,
        key_of: agent_provider_key,
        schema: describe::schema_of::<AgentProviderRecord>,
        walk: walk_agent_providers,
    },
    ModelEntry {
        name: "agent_instance",
        dimension: DbDimension::User,
        count: count_model::<AgentInstanceRecord>,
        scan: scan_agent_instances,
        key_of: agent_instance_key,
        schema: describe::schema_of::<AgentInstanceRecord>,
        walk: walk_agent_instances,
    },
    ModelEntry {
        name: "agent_run",
        dimension: DbDimension::Workspace,
        count: count_model::<AgentRunRecord>,
        scan: scan_agent_runs,
        key_of: agent_run_key,
        schema: describe::schema_of::<AgentRunRecord>,
        walk: walk_agent_runs,
    },
    ModelEntry {
        name: "session",
        dimension: DbDimension::Workspace,
        count: count_model::<SessionRecord>,
        scan: scan_sessions,
        key_of: session_key,
        schema: describe::schema_of::<SessionRecord>,
        walk: walk_sessions,
    },
    ModelEntry {
        name: "session_event",
        dimension: DbDimension::Workspace,
        count: count_model::<SessionEventRecord>,
        scan: scan_session_events,
        key_of: session_event_key,
        schema: describe::schema_session_event,
        walk: walk_session_events,
    },
    ModelEntry {
        name: "explore",
        dimension: DbDimension::Workspace,
        count: count_model::<ExploreRecord>,
        scan: scan_explores,
        key_of: explore_key,
        schema: describe::schema_of::<ExploreRecord>,
        walk: walk_explores,
    },
    ModelEntry {
        name: "change",
        dimension: DbDimension::Workspace,
        count: count_model::<ChangeRecord>,
        scan: scan_changes,
        key_of: change_key,
        schema: describe::schema_of::<ChangeRecord>,
        walk: walk_changes,
    },
    ModelEntry {
        name: "phase",
        dimension: DbDimension::Workspace,
        count: count_model::<PhaseRecord>,
        scan: scan_phases,
        key_of: phase_key,
        schema: describe::schema_of::<PhaseRecord>,
        walk: walk_phases,
    },
    ModelEntry {
        name: "checklist_item",
        dimension: DbDimension::Workspace,
        count: count_model::<ChecklistItemRecord>,
        scan: scan_checklist_items,
        key_of: checklist_item_key,
        schema: describe::schema_of::<ChecklistItemRecord>,
        walk: walk_checklist_items,
    },
    ModelEntry {
        name: "step",
        dimension: DbDimension::Workspace,
        count: count_model::<StepRecord>,
        scan: scan_steps,
        key_of: step_key,
        schema: describe::schema_of::<StepRecord>,
        walk: walk_steps,
    },
    ModelEntry {
        name: "run",
        dimension: DbDimension::Workspace,
        count: count_model::<RunRecord>,
        scan: scan_runs,
        key_of: run_key,
        schema: describe::schema_of::<RunRecord>,
        walk: walk_runs,
    },
    ModelEntry {
        name: "run_node",
        dimension: DbDimension::Workspace,
        count: count_model::<RunNodeRecord>,
        scan: scan_run_nodes,
        key_of: run_node_key,
        schema: describe::schema_of::<RunNodeRecord>,
        walk: walk_run_nodes,
    },
    ModelEntry {
        name: "run_edge",
        dimension: DbDimension::Workspace,
        count: count_model::<RunEdgeRecord>,
        scan: scan_run_edges,
        key_of: run_edge_key,
        schema: describe::schema_of::<RunEdgeRecord>,
        walk: walk_run_edges,
    },
];

/// 注册表只读出口（`describe` / `query` 按维度过滤用；「表 ↔ 类型」与
/// 「表 ↔ 遍历」两组映射同源于此，MUST NOT 另立第二份清单）。
pub(crate) fn entries() -> &'static [ModelEntry] {
    MODEL_ENTRIES
}

/// 按表名直查注册表行（跨维度——表名跨维度唯一由注册表保证）。
pub(crate) fn entry(table: &str) -> Option<&'static ModelEntry> {
    MODEL_ENTRIES.iter().find(|entry| entry.name == table)
}

/// 表名 → 维度（跨维度 miss → `None`）：`db_tables` 下钻派库与 `db_query`
/// 的维度自纠文案同源。
pub fn dimension_of(table: &str) -> Option<DbDimension> {
    entry(table).map(|entry| entry.dimension)
}

/// 本维度已注册模型清单与计数（注册表按实例维度过滤分发——全局库只列 user
/// 维度模型，workspace 库只列 workspace 维度模型，两库清单互不混列）。
pub(crate) fn list_models(
    db: &Database<'static>,
    dimension: DbDimension,
) -> Result<Vec<ModelInfo>, StoreError> {
    MODEL_ENTRIES
        .iter()
        .filter(|entry| entry.dimension == dimension)
        .map(|entry| {
            (entry.count)(db).map(|count| ModelInfo {
                name: entry.name.to_owned(),
                count,
            })
        })
        .collect()
}

/// 按模型名分页扫描记录信封并随页返回分页信封（维度随实例锁定：跨维度
/// 模型名按「未知模型」Err）；未知模型名 Err。
pub(crate) fn scan(
    db: &Database<'static>,
    dimension: DbDimension,
    model: &str,
    offset: u32,
    limit: u32,
) -> Result<RecordsPage, StoreError> {
    let entry = MODEL_ENTRIES
        .iter()
        .find(|entry| entry.dimension == dimension && entry.name == model)
        .ok_or_else(|| StoreError::Db(format!("未知模型: {model}")))?;
    (entry.scan)(db, offset, limit.min(MAX_SCAN_LIMIT), entry.key_of)
}

/// 主键自然序分页扫描（`skip(offset).take(limit)`），同一读事务内先取
/// 全量计数再取区间记录组装分页信封（records + total 同一快照）；记录经
/// serde 转 JSON 值组装信封（key 由 `key_of` 从记录 JSON 构造）。
fn scan_model<T: ToInput + Serialize>(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    let r = db.r_transaction().map_err(db_err("开启读事务"))?;
    let total = r.len().primary::<T>().map_err(db_err("统计记录数"))?;
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
    let records = page
        .into_iter()
        .map(|record| {
            let value = serde_json::to_value(&record)
                .map_err(|e| StoreError::Db(format!("编码记录信封: {e}")))?;
            Ok(RecordEnvelope {
                key: key_of(&value),
                value,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(RecordsPage { records, total })
}

/// 记录计数（`Database::len` 主键口径）。
fn count_model<T: ToInput>(db: &Database<'static>) -> Result<u64, StoreError> {
    let r = db.r_transaction().map_err(db_err("开启读事务"))?;
    r.len().primary::<T>().map_err(db_err("统计记录数"))
}

/// 记录流式遍历（**一个读事务**内完成：同一快照逐条解码 → JSON → 交访问器）：
/// 逐条回调而非整表物化，内存上界 = 访问器自己保留的窗口。返回遍历到的总行数
/// （访问器提前终止时为下界）。`key_of` 从记录 JSON 构造主键，与信封同形。
fn walk_model<T: ToInput + Serialize>(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    let r = db.r_transaction().map_err(query_db_err("开启读事务"))?;
    let mut total = 0u64;
    for record in r
        .scan()
        .primary::<T>()
        .map_err(query_db_err("遍历记录"))?
        .all()
        .map_err(query_db_err("遍历记录"))?
    {
        let record = record.map_err(query_db_err("遍历记录"))?;
        total += 1;
        let value = serde_json::to_value(&record)
            .map_err(|e| QueryError::Db(format!("编码记录信封: {e}")))?;
        if !visit(&(key_of)(&value), &value)? {
            break;
        }
    }
    Ok(total)
}

// --- per-model 包装：注册表行 = 通用实现 + 本模型 key 构造，一一对应 ---

fn scan_workspaces(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<WorkspaceRecord>(db, offset, limit, key_of)
}

fn scan_agent_providers(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<AgentProviderRecord>(db, offset, limit, key_of)
}

fn scan_agent_instances(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<AgentInstanceRecord>(db, offset, limit, key_of)
}

fn scan_agent_runs(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<AgentRunRecord>(db, offset, limit, key_of)
}

fn scan_sessions(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<SessionRecord>(db, offset, limit, key_of)
}

fn scan_session_events(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<SessionEventRecord>(db, offset, limit, key_of)
}

fn scan_explores(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<ExploreRecord>(db, offset, limit, key_of)
}

fn scan_changes(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<ChangeRecord>(db, offset, limit, key_of)
}

fn scan_phases(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<PhaseRecord>(db, offset, limit, key_of)
}

fn scan_checklist_items(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<ChecklistItemRecord>(db, offset, limit, key_of)
}

fn scan_steps(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<StepRecord>(db, offset, limit, key_of)
}

fn scan_runs(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<RunRecord>(db, offset, limit, key_of)
}

fn scan_run_nodes(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<RunNodeRecord>(db, offset, limit, key_of)
}

fn scan_run_edges(
    db: &Database<'static>,
    offset: u32,
    limit: u32,
    key_of: KeyOfFn,
) -> Result<RecordsPage, StoreError> {
    scan_model::<RunEdgeRecord>(db, offset, limit, key_of)
}

/// workspace 主键 = canonical root。
fn workspace_key(value: &Value) -> Value {
    value["root"].clone()
}

/// agent provider 主键 = id。
fn agent_provider_key(value: &Value) -> Value {
    value["id"].clone()
}

/// agent instance 主键 = id。
fn agent_instance_key(value: &Value) -> Value {
    value["id"].clone()
}

/// agent run 主键 = id。
fn agent_run_key(value: &Value) -> Value {
    value["id"].clone()
}

/// session 主键 = core 铸会话 id。
fn session_key(value: &Value) -> Value {
    value["id"].clone()
}

/// session event 主键还原 `{sessionId, seq}` 形态（u128 打包键的 JSON 可读
/// 投影）。
fn session_event_key(value: &Value) -> Value {
    serde_json::json!({ "sessionId": value["sessionId"], "seq": value["event"]["seq"] })
}

/// explore 主键 = id。
fn explore_key(value: &Value) -> Value {
    value["id"].clone()
}

/// change 主键 = change id（身份锚随换锚改呈 id；`StoreMetaRecord` 为内部治理
/// 记录，不入注册表——无查看面需求）。
fn change_key(value: &Value) -> Value {
    value["id"].clone()
}

/// phase 主键 = id。
fn phase_key(value: &Value) -> Value {
    value["id"].clone()
}

/// checklist item 主键还原 `{phaseId, itemKey}` 形态（u128 打包键的 JSON 可
/// 读投影——键为十六进制串原值，`session_event_key` 先例同型）。
fn checklist_item_key(value: &Value) -> Value {
    serde_json::json!({ "phaseId": value["phaseId"], "itemKey": value["itemKey"] })
}

/// step 主键 = id。
fn step_key(value: &Value) -> Value {
    value["id"].clone()
}

/// run 主键 = run id。
fn run_key(value: &Value) -> Value {
    value["runId"].clone()
}

/// run 节点主键还原 `{runId, nodeKey}` 形态（u128 打包键的 JSON 可读投影，
/// `checklist_item_key` 先例同型）。
fn run_node_key(value: &Value) -> Value {
    serde_json::json!({ "runId": value["runId"], "nodeKey": value["nodeKey"] })
}

/// run 边主键还原 `{runId, edgeKey}` 形态（同 `run_node_key` 的打包键投影）。
fn run_edge_key(value: &Value) -> Value {
    serde_json::json!({ "runId": value["runId"], "edgeKey": value["edgeKey"] })
}

// --- per-model 遍历包装：注册表 `walk` 行 = 通用实现 + 本模型类型 ---

fn walk_workspaces(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<WorkspaceRecord>(db, key_of, visit)
}

fn walk_agent_providers(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<AgentProviderRecord>(db, key_of, visit)
}

fn walk_agent_instances(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<AgentInstanceRecord>(db, key_of, visit)
}

fn walk_agent_runs(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<AgentRunRecord>(db, key_of, visit)
}

fn walk_sessions(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<SessionRecord>(db, key_of, visit)
}

fn walk_session_events(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<SessionEventRecord>(db, key_of, visit)
}

fn walk_explores(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<ExploreRecord>(db, key_of, visit)
}

fn walk_changes(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<ChangeRecord>(db, key_of, visit)
}

fn walk_phases(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<PhaseRecord>(db, key_of, visit)
}

fn walk_checklist_items(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<ChecklistItemRecord>(db, key_of, visit)
}

fn walk_steps(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<StepRecord>(db, key_of, visit)
}

fn walk_runs(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<RunRecord>(db, key_of, visit)
}

fn walk_run_nodes(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<RunNodeRecord>(db, key_of, visit)
}

fn walk_run_edges(
    db: &Database<'static>,
    key_of: KeyOfFn,
    visit: &mut dyn FnMut(&Value, &Value) -> Result<bool, QueryError>,
) -> Result<u64, QueryError> {
    walk_model::<RunEdgeRecord>(db, key_of, visit)
}
