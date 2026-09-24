//! legacy redb 手写表库 → native_db 一次性迁移（crate 私有，经 [`crate::Store::open`]
//! 调用，`Store::open` 内不含 redb 代码）。
//!
//! 探测：native_db 打不开时以 redb `ReadOnlyDatabase` 探测 `user_meta` 表判定
//! legacy 格式；判定失败即「无法识别的 db 格式」，由调用方报错。迁移：逐表
//! read_only 读出 → 临时文件（`<name>.native-tmp`）构建 native_db 新库 →
//! 两段改名（旧 → `.bak` 留档；新 → 原路径）。
//!
//! 原子性：任何时点旧库不丢——读旧失败 / 写新失败时旧文件未动；第一段改名
//! 失败旧文件保持原样；第二段改名失败尽力回退（`.bak` → 原路径）并报错。
//! 无「迁移完成标记」需求——文件格式本身就是标记，成功后原路径即 native 格式。
//!
//! 本模块是 store 内 redb 直依赖的唯一使用点（迁移期专用，读旧库，由后续
//! 变更收掉）；legacy 表定义逐字平移自旧 store 手写表。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use agent::{AgentEvent, AgentEventKind};
use native_db::Builder;
use redb::{ReadOnlyDatabase, ReadableDatabase, ReadableTable, TableDefinition};

use crate::model::{AgentEventRecord, AgentRunRecord, WorkspaceRecord};
use crate::store::{db_err, models, StoreError};

/// legacy user 维度注册表（key = canonical root，value = `WorkspaceRecord` JSON）
const USER_WORKSPACES: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new("user_workspaces");

/// legacy agent 运行元数据（key = run id，value = `AgentRunRecord` JSON）
const USER_AGENT_RUNS: TableDefinition<'static, i64, &[u8]> =
    TableDefinition::new("user_agent_runs");

/// legacy agent 运行事件流（key = `(run_id, seq)` 复合键，value = 事件 JSON）
const USER_AGENT_RUN_EVENTS: TableDefinition<'static, (i64, u64), &[u8]> =
    TableDefinition::new("user_agent_run_events");

/// legacy META 表（`schema_version` 轮账随迁移退役，仅作格式判定）
const USER_META: TableDefinition<'static, &str, u64> = TableDefinition::new("user_meta");

/// legacy 格式探测：redb 可打开且 `user_meta` 表存在。判定失败（打不开 /
/// 无 META 表）即非 legacy，调用方按「无法识别的 db 格式」报错。
///
/// 探测 MUST 先于 native_db open 执行：底层 redb 双版本共存，native_db 内部
/// redb 2.x 对 4.x 写出的同头文件解析即 panic（`AllocatorStateKey` unreachable），
/// 而 4.x 的 read_only 端显式兼容 2.x 字节（`Deprecated` 变体）、是双格式安全
/// 读端。native 格式文件经此探测读到无 `user_meta` 表 → false → 交 native_db
/// 打开，行为正确且无 panic 风险。
pub(crate) fn is_legacy_format(path: &Path) -> bool {
    let Ok(db) = ReadOnlyDatabase::open(path) else {
        return false;
    };
    let Ok(txn) = db.begin_read() else {
        return false;
    };
    txn.open_table(USER_META).is_ok()
}

/// 一次性迁移：读旧 → 写新（临时文件）→ 两段改名。失败时旧文件保持原样
/// （未改名未截断），错误经 [`StoreError::Db`]（`迁移:` 语境前缀）呈现。
pub(crate) fn migrate(path: &Path) -> Result<(), StoreError> {
    let backup = append_suffix(path, ".bak");
    let tmp = append_suffix(path, ".native-tmp");
    // 清理上次失败迁移的残留临时文件（旧库不受影响）
    let _ = std::fs::remove_file(&tmp);

    let (workspaces, runs, events) = read_legacy(path)?;
    write_native(&tmp, &workspaces, &runs, &events)?;

    // 第一段改名：旧 → .bak 留档（已存在先移除，永不覆盖删除前未成功的新库）
    if backup.exists() {
        std::fs::remove_file(&backup).map_err(|e| {
            StoreError::Db(format!("迁移: 移除既有留档 {} 失败: {e}", backup.display()))
        })?;
    }
    std::fs::rename(path, &backup).map_err(|e| {
        StoreError::Db(format!(
            "迁移: 旧库改名 {} → {} 失败: {e}",
            path.display(),
            backup.display()
        ))
    })?;
    // 第二段改名：新 → 原路径；失败尽力回退（旧库还原原路径）并报错
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::rename(&backup, path);
        return Err(StoreError::Db(format!(
            "迁移: 新库改名 {} → {} 失败: {e}",
            tmp.display(),
            path.display()
        )));
    }
    Ok(())
}

/// legacy 三表读出载荷（workspace 清单 / run 清单 / 事件清单）。
type LegacyTables = (
    Vec<WorkspaceRecord>,
    Vec<AgentRunRecord>,
    Vec<AgentEventRecord>,
);

/// 逐表读出 legacy 三表（`user_meta` 无数据迁移，仅格式判定用）。
fn read_legacy(path: &Path) -> Result<LegacyTables, StoreError> {
    let db = ReadOnlyDatabase::open(path).map_err(|e| {
        StoreError::Db(format!("迁移: 打开 legacy 库 {} 失败: {e}", path.display()))
    })?;
    let txn = db.begin_read().map_err(db_err("迁移: 开启读事务"))?;
    let workspaces = read_workspaces(&txn)?;
    let runs = read_agent_runs(&txn)?;
    let events = read_agent_run_events(&txn)?;
    Ok((workspaces, runs, events))
}

/// `user_workspaces` 读出：key=root 原样，value=`WorkspaceRecord` JSON。
fn read_workspaces(txn: &redb::ReadTransaction) -> Result<Vec<WorkspaceRecord>, StoreError> {
    let table = txn
        .open_table(USER_WORKSPACES)
        .map_err(db_err("迁移: 打开 user_workspaces 表"))?;
    let mut records = Vec::new();
    for entry in table.iter().map_err(db_err("迁移: 遍历 user_workspaces"))? {
        let (_, value) = entry.map_err(db_err("迁移: 读取 workspace 记录"))?;
        let record: WorkspaceRecord =
            serde_json::from_slice(value.value()).map_err(db_err("迁移: 解码 workspace 记录"))?;
        records.push(record);
    }
    Ok(records)
}

/// `user_agent_runs` 读出：key=id 原样，value=`AgentRunRecord` JSON。
fn read_agent_runs(txn: &redb::ReadTransaction) -> Result<Vec<AgentRunRecord>, StoreError> {
    let table = txn
        .open_table(USER_AGENT_RUNS)
        .map_err(db_err("迁移: 打开 user_agent_runs 表"))?;
    let mut records = Vec::new();
    for entry in table.iter().map_err(db_err("迁移: 遍历 user_agent_runs"))? {
        let (_, value) = entry.map_err(db_err("迁移: 读取 run 记录"))?;
        let record: AgentRunRecord =
            serde_json::from_slice(value.value()).map_err(db_err("迁移: 解码 run 记录"))?;
        records.push(record);
    }
    Ok(records)
}

/// `user_agent_run_events` 读出：复合键 `(run_id, seq)` 打包为 `event_key`，
/// value 解出 `AgentEvent` 嵌装；解不出的行包 `AgentEventKind::Raw` 兜底
/// （不丢行不中断）。`seq` 恒取旧表键——新库主键空间与原键一一对应。
fn read_agent_run_events(txn: &redb::ReadTransaction) -> Result<Vec<AgentEventRecord>, StoreError> {
    let table = txn
        .open_table(USER_AGENT_RUN_EVENTS)
        .map_err(db_err("迁移: 打开 user_agent_run_events 表"))?;
    let mut records = Vec::new();
    for entry in table
        .iter()
        .map_err(db_err("迁移: 遍历 user_agent_run_events"))?
    {
        let (key, value) = entry.map_err(db_err("迁移: 读取事件行"))?;
        let (run_id, seq) = key.value();
        let bytes: &[u8] = value.value();
        let mut event = match serde_json::from_slice::<AgentEvent>(bytes) {
            Ok(event) => event,
            Err(_) => raw_fallback(bytes),
        };
        // 归一：seq 以旧表键为准（正常行 value 内 seq 与键同源，此为幂等；
        // 兜底行本就取键值），保证 event_key 打包与原键一致、无碰撞
        event.seq = seq;
        records.push(AgentEventRecord::new(run_id, event));
    }
    Ok(records)
}

/// 损坏事件行的 `Raw` 兜底：`event_type` 取 JSON `kind` 字段或 "unknown"，
/// `timestamp_ms` 取 JSON `timestampMs` 或 0，原文经 lossy UTF-8 完整保留。
fn raw_fallback(bytes: &[u8]) -> AgentEvent {
    let parsed: serde_json::Value =
        serde_json::from_slice(bytes).unwrap_or(serde_json::Value::Null);
    let event_type = parsed
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let timestamp_ms = parsed
        .get("timestampMs")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let raw_json = String::from_utf8_lossy(bytes).into_owned();
    AgentEvent {
        seq: 0,
        timestamp_ms,
        kind: AgentEventKind::Raw {
            event_type,
            raw_json,
        },
    }
}

/// 临时文件建新库并单事务写入全部记录；写完提交落盘并 drop 句柄（Windows
/// 改名前置条件）。
fn write_native(
    tmp: &Path,
    workspaces: &[WorkspaceRecord],
    runs: &[AgentRunRecord],
    events: &[AgentEventRecord],
) -> Result<(), StoreError> {
    let db = Builder::new()
        .create(models(), tmp)
        .map_err(|e| StoreError::Db(format!("迁移: 建新库 {} 失败: {e}", tmp.display())))?;
    let rw = db.rw_transaction().map_err(db_err("迁移: 开启写事务"))?;
    for record in workspaces {
        rw.insert(record.clone())
            .map_err(db_err("迁移: 写入 workspace 记录"))?;
    }
    for record in runs {
        rw.insert(record.clone())
            .map_err(db_err("迁移: 写入 run 记录"))?;
    }
    for record in events {
        rw.insert(record.clone())
            .map_err(db_err("迁移: 写入事件记录"))?;
    }
    rw.commit().map_err(db_err("迁移: 提交迁移事务"))?;
    drop(db);
    Ok(())
}

/// 原路径追加后缀（`desktop-store.redb` → `desktop-store.redb.bak`）。
fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| OsStr::new("").to_os_string(), OsStr::to_os_string);
    name.push(suffix);
    path.with_file_name(name)
}
