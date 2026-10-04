use std::fs;
use std::path::Path;

use serde_json::Value;

use super::WORKFLOW_FILE_NAME;
use crate::model::{ActivePhase, FileLogEntry, PhaseLog, Workflow};

/// 单份 workflow.json 的解析结果：正常解析或整体降级标记。
#[derive(Debug, Clone)]
pub enum WorkflowFileParse {
    Parsed(Workflow),
    Unparsable { reason: String },
}

/// 读取并宽松解析一份 workflow.json。
pub fn parse_workflow_file(path: &Path) -> WorkflowFileParse {
    match fs::read_to_string(path) {
        Ok(text) => parse_workflow_text(&text),
        Err(err) => WorkflowFileParse::Unparsable {
            reason: format!("读取失败: {err}"),
        },
    }
}

/// change 目录内存在 workflow.json 时解析之；不存在返回 `None`，
/// 存在但整体损坏返回 Unparsable 标记。
pub fn load_workflow(change_dir: &Path) -> Option<Workflow> {
    let workflow_path = change_dir.join(WORKFLOW_FILE_NAME);
    if !workflow_path.is_file() {
        return None;
    }
    match parse_workflow_file(&workflow_path) {
        WorkflowFileParse::Parsed(workflow) => Some(workflow),
        WorkflowFileParse::Unparsable { .. } => None,
    }
}

fn parse_workflow_text(text: &str) -> WorkflowFileParse {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(err) => {
            return WorkflowFileParse::Unparsable {
                reason: format!("JSON 解析失败: {err}"),
            }
        }
    };
    let Some(object) = value.as_object() else {
        return WorkflowFileParse::Unparsable {
            reason: "顶层不是 JSON 对象".to_string(),
        };
    };

    // 必填核心字段：缺失或非字符串 → 整体降级
    let Some(workflow_type) = object.get("workflow_type").and_then(Value::as_str) else {
        return WorkflowFileParse::Unparsable {
            reason: "缺少或非法的 workflow_type 字段".to_string(),
        };
    };

    // 可选字段：类型不符时宽松降级，不炸整份记录
    let created = object
        .get("created")
        .and_then(Value::as_str)
        .map(str::to_string);

    let eval = match object.get("eval") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| serde_json::from_value::<PhaseLog>(item.clone()).ok())
            .collect(),
        _ => Vec::new(),
    };

    let file_log = match object.get("file_log") {
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(|item| serde_json::from_value::<FileLogEntry>(item.clone()).ok())
                .collect(),
        ),
        _ => None,
    };

    let active_phase = match object.get("active_phase") {
        Some(value) if !value.is_null() => {
            serde_json::from_value::<ActivePhase>(value.clone()).ok()
        }
        _ => None,
    };

    WorkflowFileParse::Parsed(Workflow {
        workflow_type: workflow_type.to_string(),
        created,
        eval,
        file_log,
        active_phase,
    })
}
