//! `propose_config` 工具：config 会话**唯一**的提案出口。
//!
//! 两模式（参数面 `{ candidate?: object }`，`required: []`）：
//! - 不给候选 = **只读模式**：返回当前盘面的 file face 投影（`candidate` 为 `null`、
//!   `changes` 为空）；
//! - 给候选 = **校验模式**：候选是整份文件面；`config::validate` 存在任一
//!   `InvalidValue` 即 `Err`（逐条 `path: message` + 修正后重交完整候选），零违例
//!   才把归一化候选与字段级差异一并返回。
//!
//! 三条不变量：输出恒为**单条** JSON 字符串（`ToolResult.content` 是扁平字符串），
//! `current` / `candidate` **一律 file face**（MUST NOT 用 wire face）；一次调用
//! **零副作用**（不写文件、不写库、不改会话状态——候选只活在会话转录里）；**无路径
//! 参数**（盘面由服务端按仓根解析，工具在 `tools::input_path` 的无路径白名单内）。

use std::collections::BTreeSet;
use std::path::PathBuf;

use config::{ConfigDiagnostic, DiagnosticKind};
use rig::completion::ToolDefinition;
use serde_json::Value;

use crate::sdk::tools;

/// `propose_config` 工具名（挂载清单 / 装配臂 / 沙箱白名单 / 执行上下文铸造四处
/// 引用同一常量）。
pub(crate) const PROPOSE_CONFIG_TOOL: &str = "propose_config";

/// 工具定义（挂载清单第 14 项的定义来源）：名字与描述同源一处，避免「名字在挂载、
/// 描述在执行体」两处分叉。
pub(crate) fn definition() -> ToolDefinition {
    ToolDefinition {
        name: PROPOSE_CONFIG_TOOL.to_owned(),
        description: concat!(
            "提交工作区配置 proposal（两模式）。不给 candidate = 只读模式：返回当前盘面",
            "（file face，candidate 为 null、changes 为空），先看盘再动手。给 candidate = 校验",
            "模式：候选必须是**整份文件面**（一次全量，不接受片段；多词键用 snake_case：",
            "static_analysis / write_protection / depends_on；未知顶层键直接写在顶层，不要包",
            "进 extra）。校验有任一违例即返回错误，文案逐条列 path: message——按诊断修正后",
            "**重交完整候选**。通过则返回归一化的 candidate 与字段级 changes（含 summary）。",
            "无路径参数：盘面由服务端按仓根解析；本工具零副作用，不写文件、不写库，落盘只在",
            "用户点「应用」后由应用进程完成"
        )
        .to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "candidate": {
                    "type": "object",
                    "description": "整份文件面候选（snake_case：static_analysis / write_protection / depends_on；省略即只读模式）"
                }
            },
            "required": []
        }),
    }
}

/// `propose_config` 执行上下文（per-turn 铸造：`root` = 主仓 root，配置权威所在）。
pub(crate) struct ConfigContext {
    root: PathBuf,
}

impl ConfigContext {
    /// 上下文构造（泵起播按名字清单含 `propose_config` 铸造）。
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// 两模式执行体：输出单条 file-face JSON 字符串（过 L1 收口），零副作用。
    pub(crate) fn execute(&self, input: &Value) -> Result<String, String> {
        let (current, diagnostics) = config::load_file_face(&self.root)?;
        let (candidate, changes, summary) = match candidate_of(input) {
            None => (Value::Null, Vec::new(), read_only_summary(&diagnostics)),
            Some(candidate) => {
                let report = config::validate(candidate);
                let violations: Vec<&ConfigDiagnostic> = report
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| matches!(diagnostic.kind, DiagnosticKind::InvalidValue))
                    .collect();
                if !violations.is_empty() {
                    let mut message = violations
                        .iter()
                        .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
                        .collect::<Vec<String>>()
                        .join("\n");
                    message
                        .push_str("\n请按上述诊断修正后重新提交完整候选（一次全量，不接受片段）。");
                    return Err(message);
                }
                let normalized = config::to_file_value(&report.config, candidate)?;
                let changes = diff(&current, &normalized);
                let summary = if changes.is_empty() {
                    "候选合法，与当前配置一致".to_owned()
                } else {
                    format!("候选合法，共 {} 处变更", changes.len())
                };
                (normalized, changes, summary)
            }
        };
        let payload = serde_json::json!({
            "current": current,
            "candidate": candidate,
            "changes": changes,
            "summary": summary,
        });
        let text = serde_json::to_string(&payload)
            .map_err(|error| format!("proposal 结果序列化失败: {error}"))?;
        Ok(tools::cap_result(text))
    }
}

/// 候选提取：缺键 / 显式 `null` 即只读模式（`None`）。
fn candidate_of(input: &Value) -> Option<&Value> {
    input
        .get("candidate")
        .filter(|candidate| !candidate.is_null())
}

/// 只读模式摘要：诊断条数 + 前若干 `path`（诊断折进 `summary`，不新增第五键）。
fn read_only_summary(diagnostics: &[ConfigDiagnostic]) -> String {
    const MAX_PATHS: usize = 5;
    if diagnostics.is_empty() {
        return "只读模式：返回当前盘面（无诊断）".to_owned();
    }
    let paths = diagnostics
        .iter()
        .take(MAX_PATHS)
        .map(|diagnostic| diagnostic.path.as_str())
        .collect::<Vec<&str>>()
        .join("、");
    let ellipsis = if diagnostics.len() > MAX_PATHS {
        "、…"
    } else {
        ""
    };
    format!(
        "只读模式：返回当前盘面（诊断 {} 条：{paths}{ellipsis}）",
        diagnostics.len()
    )
}

/// 单条字段级差异（`path` 为 file-face 键路径）。
#[derive(Debug, serde::Serialize)]
struct Change {
    /// 点分键路径（顶层即键名）
    path: String,
    /// `added` / `removed` / `changed`（封闭三值）
    kind: &'static str,
    /// 变更前值（新增即 `null`）
    before: Option<Value>,
    /// 变更后值（删除即 `null`）
    after: Option<Value>,
}

/// 字段级差异：**对象递归**（键并集、字母序）、**数组与标量作叶子**（整体替换）。
/// 数组逐下标会把「插一条 suite」后面的全部项标成 changed——噪声灌满卡片，且
/// `changes == []` 的「与盘面一致」判据随之失效。
fn diff(current: &Value, candidate: &Value) -> Vec<Change> {
    let mut changes = Vec::new();
    diff_into("", current, candidate, &mut changes);
    changes
}

/// 递归差异（`prefix` 为空表示顶层）。
fn diff_into(prefix: &str, current: &Value, candidate: &Value, changes: &mut Vec<Change>) {
    match (current, candidate) {
        (Value::Object(left), Value::Object(right)) => {
            let keys = left
                .keys()
                .chain(right.keys())
                .collect::<BTreeSet<&String>>();
            for key in keys {
                let path = join_path(prefix, key);
                match (left.get(key), right.get(key)) {
                    (Some(before), Some(after)) => diff_into(&path, before, after, changes),
                    (Some(before), None) => changes.push(Change {
                        path,
                        kind: "removed",
                        before: Some(before.clone()),
                        after: None,
                    }),
                    (None, Some(after)) => changes.push(Change {
                        path,
                        kind: "added",
                        before: None,
                        after: Some(after.clone()),
                    }),
                    (None, None) => {}
                }
            }
        }
        _ => {
            if current != candidate {
                changes.push(Change {
                    path: prefix.to_owned(),
                    kind: "changed",
                    before: Some(current.clone()),
                    after: Some(candidate.clone()),
                });
            }
        }
    }
}

/// 键路径拼接（顶层无前缀）。
fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}
