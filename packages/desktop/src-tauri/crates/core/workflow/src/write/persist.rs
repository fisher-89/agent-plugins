//! 写面持久层（crate 内私有，无 crate 外导出）：workflow.json 以
//! `serde_json::Value` 原文载入作定点改写面（W2 保形策略——未知 / legacy
//! 字段原样保留），typed `Workflow` 经既有宽松解析作逻辑面。写触点收敛五类：
//! `active_phase` 置 / 清、`eval` 追加、`eval[i].stale` 翻转、
//! `eval[i].backtrack_to/backtrack_reason` 标记；`file_log` 零触点（AC-3，
//! 桌面 run 全程 file_log 零新增）。

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use foundation::layout::Layout;
use serde_json::Value;
use time::format_description::well_known::{Iso8601, Rfc3339};
use time::OffsetDateTime;

use super::phase_table::{dependents, PhaseDefinition};
use crate::model::Workflow;
use crate::parse::{parse_workflow_file, WorkflowFileParse, WORKFLOW_FILE_NAME};

/// 已载入的 workflow.json 工作文档：raw 定点改写面 + typed 逻辑面。
pub(crate) struct ChangeDoc {
    /// 磁盘路径（写回单点）
    path: PathBuf,
    /// 原文 JSON（定点改写面；未知 / legacy 字段原样保留）
    pub raw: Value,
    /// 宽松解析 typed 逻辑面（路由 / 落账判定的输入面）
    pub typed: Workflow,
}

/// 载入 change 的 workflow.json：文件缺失 / JSON 非法 / 顶层非对象 /
/// `eval` 键在位但非数组 → `Err`（`change_create` 是唯一创建者，写面从不
/// 创建文件，与插件 appendEntry / writeEvalJson 前置一致）。
pub(crate) fn load_doc(layout: &Layout, change: &str) -> Result<ChangeDoc, String> {
    let path = layout.changes_root.join(change).join(WORKFLOW_FILE_NAME);
    let typed = match parse_workflow_file(&path) {
        WorkflowFileParse::Parsed(typed) => typed,
        WorkflowFileParse::Unparsable { reason } => {
            return Err(format!("workflow.json 无法解析: {reason}"));
        }
    };
    let text =
        fs::read_to_string(&path).map_err(|error| format!("读取 workflow.json 失败: {error}"))?;
    let raw: Value =
        serde_json::from_str(&text).map_err(|error| format!("workflow.json 解析失败: {error}"))?;
    if let Some(eval) = raw.get("eval") {
        if !eval.is_array() {
            return Err(format!(
                "workflow.json.eval 必须是数组，但实际类型为 {}",
                json_type_name(eval)
            ));
        }
    }
    Ok(ChangeDoc { path, raw, typed })
}

/// pretty 写回（2 空格缩进 + 尾换行，与插件 `writeEvalJson` 输出形态一致；
/// serde 写出即 W3 移交后的 schema 权威实现）。
pub(crate) fn save(doc: &ChangeDoc) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(&doc.raw)
        .map_err(|error| format!("workflow.json 序列化失败: {error}"))?;
    text.push('\n');
    fs::write(&doc.path, text).map_err(|error| format!("写入 workflow.json 失败: {error}"))
}

/// raw 文档 eval 数组可变视图（缺键即建空数组；顶层非对象与非数组 eval 由
/// load_doc 排除）。
pub(crate) fn eval_entries_mut(raw: &mut Value) -> &mut Vec<Value> {
    let object = raw
        .as_object_mut()
        .expect("workflow.json 顶层为对象（load_doc 保证）");
    let eval = object
        .entry("eval".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()));
    eval.as_array_mut().expect("eval 键为数组（load_doc 保证）")
}

/// raw 条目的 phase 字段（非对象 / 缺键 / 非串 → `None`）。
pub(crate) fn entry_phase(entry: &Value) -> Option<&str> {
    entry.get("phase").and_then(Value::as_str)
}

/// raw 条目的 verdict 字段（线格式小写词）。
pub(crate) fn entry_verdict(entry: &Value) -> Option<&str> {
    entry.get("verdict").and_then(Value::as_str)
}

/// raw 条目的宽松时间戳：缺失、null 或 ISO 8601 解析失败一律 `None`
///（排序视为最旧，与 model 层 lenient_timestamp 同语义）。
pub(crate) fn entry_time(entry: &Value) -> Option<OffsetDateTime> {
    entry
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(parse_timestamp)
}

/// 宽松时间戳解析（Rfc3339 优先，Iso8601 兜底）。
pub(crate) fn parse_timestamp(raw: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Rfc3339)
        .or_else(|_| OffsetDateTime::parse(raw, &Iso8601::DEFAULT))
        .ok()
}

/// 当前时刻 ISO 8601 串（与 model 层时间戳出线同式）。
pub(crate) fn now_iso() -> String {
    format_timestamp(OffsetDateTime::now_utc())
}

/// 时间戳 ISO 8601 出线（与 model 层 lenient_timestamp serialize 同式）。
pub(crate) fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp.format(&Rfc3339).unwrap_or_default()
}

/// eval 数组内满足谓词的最新条目下标（timestamp 降序取首；时间戳缺失 /
/// 非法视为最旧；同时间戳保序取先——与插件 stable sort desc + `[0]` 同语义）。
pub(crate) fn latest_entry_index(
    entries: &[Value],
    predicate: impl Fn(&Value) -> bool,
) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_time: Option<OffsetDateTime> = None;
    for (idx, entry) in entries.iter().enumerate() {
        if !predicate(entry) {
            continue;
        }
        let time = entry_time(entry);
        let replace = match best_time {
            None => true,
            Some(current) => time.is_some_and(|candidate| candidate > current),
        };
        if replace {
            best = Some(idx);
            best_time = time;
        }
    }
    best
}

/// 目标相位最新 pass 条目标记 stale + 依赖向后传播（与插件 `markPhaseStale`
/// 同语义：timestamp 降序首个 pass 条目置 stale，随后 `dependents` BFS 将
/// 下游相位全部条目置 stale；无 pass 条目 no-op 不传播）。
pub(crate) fn mark_phase_stale(raw: &mut Value, table: &[PhaseDefinition], target: &str) {
    let found = {
        let entries = eval_entries_mut(raw);
        let latest = latest_entry_index(entries, |entry| {
            entry_phase(entry) == Some(target) && entry_verdict(entry) == Some("pass")
        });
        if let Some(idx) = latest {
            if let Some(object) = entries[idx].as_object_mut() {
                object.insert("stale".to_owned(), Value::Bool(true));
            }
            true
        } else {
            false
        }
    };
    if found {
        let mut visited = HashSet::from([target.to_owned()]);
        propagate_stale(raw, table, target, &mut visited);
    }
}

/// stale 依赖传播：下游相位全部条目（pass 与 fail）置 stale；visited 集防环
///（依赖图为 DAG，防御式保留，与插件 `propagateStale` 一致）。
fn propagate_stale(
    raw: &mut Value,
    table: &[PhaseDefinition],
    phase_id: &str,
    visited: &mut HashSet<String>,
) {
    for dependent in dependents(table, phase_id) {
        if visited.contains(&dependent) {
            continue;
        }
        visited.insert(dependent.clone());
        for entry in eval_entries_mut(raw) {
            if entry_phase(entry) == Some(dependent.as_str()) {
                if let Some(object) = entry.as_object_mut() {
                    object.insert("stale".to_owned(), Value::Bool(true));
                }
            }
        }
        propagate_stale(raw, table, &dependent, visited);
    }
}

/// JSON 值类型人读名（错误消息面）。
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "布尔",
        Value::Number(_) => "数字",
        Value::String(_) => "字符串",
        Value::Array(_) => "数组",
        Value::Object(_) => "对象",
    }
}
