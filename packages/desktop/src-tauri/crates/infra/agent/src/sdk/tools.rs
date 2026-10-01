//! sdk 引擎六工具面：read / grep / glob / ls / write / edit 的定义（名称 /
//! 描述 / JSON schema 入参）与异步执行体。
//!
//! 边界：执行体不内嵌 policy / sandbox 检查（由 loop 统一插值——拒绝时
//! 不进执行体）；bash 不进 MVP 工具面（无进程执行）。错误统一 `Err(String)`
//! （is_error ToolResult 的内容来源）；成功 `Ok(String)` 为扁平文本结果。
//!
//! 路径约定：入参 `input` 的 `path` 字段已被 loop 经 sandbox 校验后改写为
//! workspace 内规范路径（[`crate::sdk::sandbox`]），执行体直接信任之；
//! glob 工具无 `path` 字段，以 root 相对 pattern 驱动（pattern 合法性同经
//! sandbox 的 [`crate::sdk::sandbox::check_pattern`] 预检）。

use std::path::Path;

use rig_core::completion::ToolDefinition;
use serde_json::Value;

/// glob 单次结果上限（防大目录扫描灌爆上下文）。
const MAX_GLOB_RESULTS: usize = 200;

/// grep 目录递归单次命中上限（同 glob 截断口径，防大目录扫描灌爆上下文）。
const MAX_GREP_RESULTS: usize = 200;

/// 六工具名（loop 的 RunStarted.tools 与 policy 决策表同源口径，见
/// [`crate::sdk::policy`]）。
pub const TOOL_NAMES: [&str; 6] = ["read", "grep", "glob", "ls", "write", "edit"];

/// 模型入参字符串字段提取（缺失或非字符串 → Err）。
fn string_field(input: &Value, field: &str) -> Result<String, String> {
    input
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("入参缺失或非字符串字段: {field}"))
}

/// 工具定义面：六工具的 rig `ToolDefinition`（loop 逐轮组装
/// `CompletionRequest.tools` 用）。
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read".to_owned(),
            description: "读取文件内容（行号前缀输出）。可选 offset（1 起始行号）与 limit（行数）"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件路径" },
                    "offset": { "type": "integer", "description": "起始行号（1 起始，缺省 1）" },
                    "limit": { "type": "integer", "description": "读取行数（缺省读到文件尾）" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "grep".to_owned(),
            description: "行级子串匹配：返回命中行（文件路径:行号: 内容）；path 传目录时递归扫描其下文件"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件或目录路径（目录时递归扫描）" },
                    "pattern": { "type": "string", "description": "子串匹配串（非正则）" }
                },
                "required": ["path", "pattern"]
            }),
        },
        ToolDefinition {
            name: "glob".to_owned(),
            description: "按 glob 模式列出 workspace 内匹配文件（root 相对模式，上限 200 条）"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "root 相对 glob 模式（如 src/**/*.rs）" }
                },
                "required": ["pattern"]
            }),
        },
        ToolDefinition {
            name: "ls".to_owned(),
            description: "列目录：一级条目（目录带 / 后缀），字母序".to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内目录路径" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "write".to_owned(),
            description: "覆写文件（全量覆盖；缺失父目录自动创建）".to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件路径" },
                    "content": { "type": "string", "description": "写入全文" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "edit".to_owned(),
            description: "精确串替换：old_string 在文件中必须唯一命中（多处需 replace_all）"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件路径" },
                    "old_string": { "type": "string", "description": "待替换的精确原文" },
                    "new_string": { "type": "string", "description": "替换后的文本" },
                    "replace_all": { "type": "boolean", "description": "全部替换（缺省仅首处且要求唯一命中）" }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
    ]
}

/// 沙箱校验对象提取：工具名 → 入参中的路径类字段值（glob 为 pattern，
/// 其余为 path；未知工具 None——policy 已先行拒绝，此处兜底）。
pub fn input_path<'a>(name: &str, input: &'a Value) -> Option<&'a str> {
    let field = if name == "glob" { "pattern" } else { "path" };
    input.get(field).and_then(Value::as_str)
}

/// 工具执行分发：按工具名调用执行体。root 仅 glob 消费（模式拼接基座）。
pub async fn execute(root: &Path, name: &str, input: &Value) -> Result<String, String> {
    match name {
        "read" => read(input).await,
        "grep" => grep(input).await,
        "glob" => glob(root, input).await,
        "ls" => list_dir(input).await,
        "write" => write(input).await,
        "edit" => edit(input).await,
        other => Err(format!("未知工具: {other}")),
    }
}

/// read：行号前缀输出（`行号\t内容`，cat -n 形态），offset（1 起始）与
/// limit 截取。
async fn read(input: &Value) -> Result<String, String> {
    let path = string_field(input, "path")?;
    let offset = input
        .get("offset")
        .and_then(Value::as_u64)
        .map(|n| n.max(1) as usize)
        .unwrap_or(1);
    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n as usize);
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取失败: {e}"))?;
    let lines: Vec<&str> = text.lines().collect();
    let start = (offset - 1).min(lines.len());
    let end = limit
        .map(|l| (start + l).min(lines.len()))
        .unwrap_or(lines.len());
    let numbered: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{:>6}\t{}", start + index + 1, line))
        .collect();
    Ok(if numbered.is_empty() {
        "(空区间)".to_owned()
    } else {
        numbered.join("\n")
    })
}

/// grep：行级子串匹配，命中行以 `路径:行号: 内容` 输出。path 为文件时单文件
/// 匹配；为目录时递归扫描其下文件（不可读文件跳过、命中上限截断留痕）——
/// 目录入参曾以「读取失败: 拒绝访问 (os error 5)」形态误导为权限拒绝，
/// 递归分支即该形态的正解。metadata 失败（缺失路径）回落单文件分支，保留
/// 显式读取错误语义。
async fn grep(input: &Value) -> Result<String, String> {
    let path = string_field(input, "path")?;
    let pattern = string_field(input, "pattern")?;
    if pattern.is_empty() {
        return Err("pattern 不得为空".to_owned());
    }
    let is_dir = tokio::fs::metadata(&path)
        .await
        .map(|meta| meta.is_dir())
        .unwrap_or(false);
    if is_dir {
        return grep_dir(&path, &pattern).await;
    }
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取失败: {e}"))?;
    Ok(hits_body(match_lines(&path, &text, &pattern), &pattern))
}

/// 目录递归分支：`{dir}/**/*` 枚举（glob 字典序，与 glob 工具同 crate 同
/// 姿态），仅文件参与匹配；不可读文件（二进制 / 非 UTF-8）静默跳过——
/// 目录扫描不因个别文件中断（单文件显式指定的读取失败语义不弱化）。
async fn grep_dir(dir: &str, pattern: &str) -> Result<String, String> {
    let base = dir.replace('\\', "/");
    let full = format!("{}/**/*", base.trim_end_matches('/'));
    let entries = glob::glob(&full).map_err(|e| format!("非法 glob 模式: {e}"))?;
    let mut hits: Vec<String> = Vec::new();
    let mut truncated = false;
    for entry in entries.flatten() {
        if !entry.is_file() {
            continue;
        }
        // 输出路径统一 `/` 分隔（glob 条目在 Windows 上为字面前缀 + `\` 拼接
        // 段的混合形态；与 glob 工具的归一口径一致）
        let display = entry.to_string_lossy().replace('\\', "/");
        if let Ok(text) = tokio::fs::read_to_string(&entry).await {
            hits.extend(match_lines(&display, &text, pattern));
            if hits.len() > MAX_GREP_RESULTS {
                truncated = true;
                break;
            }
        }
    }
    hits.truncate(MAX_GREP_RESULTS);
    let mut body = hits_body(hits, pattern);
    if truncated {
        body.push_str(&format!("\n（结果超过 {MAX_GREP_RESULTS} 条已截断）"));
    }
    Ok(body)
}

/// 命中行清单 → 输出体（空清单的非错误占位与单文件分支同口径）。
fn hits_body(hits: Vec<String>, pattern: &str) -> String {
    if hits.is_empty() {
        format!("无匹配行: {pattern}")
    } else {
        hits.join("\n")
    }
}

/// 单文件命中行收集（`路径:行号: 内容` 形态）。
fn match_lines(path: &str, text: &str, pattern: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(pattern))
        .map(|(index, line)| format!("{}:{}: {}", path, index + 1, line))
        .collect()
}

/// glob：root 相对模式扫描（分隔符统一 `/`），路径字典序，结果截断留痕。
async fn glob(root: &Path, input: &Value) -> Result<String, String> {
    let pattern = string_field(input, "pattern")?;
    let base = root.to_string_lossy().replace('\\', "/");
    let full = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        pattern.replace('\\', "/")
    );
    let entries = glob::glob(&full).map_err(|e| format!("非法 glob 模式: {e}"))?;
    let mut paths: Vec<String> = Vec::new();
    for entry in entries.take(MAX_GLOB_RESULTS + 1).flatten() {
        paths.push(entry.to_string_lossy().replace('\\', "/"));
    }
    let truncated = paths.len() > MAX_GLOB_RESULTS;
    paths.truncate(MAX_GLOB_RESULTS);
    paths.sort();
    let mut body = if paths.is_empty() {
        format!("无匹配: {pattern}")
    } else {
        paths.join("\n")
    };
    if truncated {
        body.push_str(&format!("\n（结果超过 {MAX_GLOB_RESULTS} 条已截断）"));
    }
    Ok(body)
}

/// ls：一级条目，目录带 `/` 后缀，字母序。
async fn list_dir(input: &Value) -> Result<String, String> {
    let path = string_field(input, "path")?;
    let mut reader = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| format!("读目录失败: {e}"))?;
    let mut entries: Vec<String> = Vec::new();
    while let Some(entry) = reader
        .next_entry()
        .await
        .map_err(|e| format!("遍历目录失败: {e}"))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry
            .file_type()
            .await
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false);
        entries.push(if is_dir { format!("{name}/") } else { name });
    }
    entries.sort();
    Ok(if entries.is_empty() {
        "(空目录)".to_owned()
    } else {
        entries.join("\n")
    })
}

/// write：全量覆写；父目录缺失自动创建（sandbox 的宽容 canonicalize 已
/// 允许缺失尾段，两侧口径一致）。
async fn write(input: &Value) -> Result<String, String> {
    let path = string_field(input, "path")?;
    let content = string_field(input, "content")?;
    if let Some(parent) = Path::new(&path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建父目录失败: {e}"))?;
    }
    tokio::fs::write(&path, &content)
        .await
        .map_err(|e| format!("写入失败: {e}"))?;
    Ok(format!("已写入 {path}（{} 字节）", content.len()))
}

/// edit：精确串替换——old_string 必须命中；多处命中需 replace_all 或唯一化
/// 串；替换后回写。
async fn edit(input: &Value) -> Result<String, String> {
    let path = string_field(input, "path")?;
    let old = string_field(input, "old_string")?;
    let new = string_field(input, "new_string")?;
    if old.is_empty() {
        return Err("old_string 不得为空".to_owned());
    }
    let replace_all = input
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取失败: {e}"))?;
    let hits = text.matches(&old).count();
    if hits == 0 {
        return Err("old_string 未命中（先 read 确认原文）".to_owned());
    }
    if hits > 1 && !replace_all {
        return Err(format!(
            "old_string 命中 {hits} 处：改用唯一更长的串，或传 replace_all"
        ));
    }
    let (updated, replaced) = if replace_all {
        (text.replace(&old, &new), hits)
    } else {
        (text.replacen(&old, &new, 1), 1)
    };
    tokio::fs::write(&path, updated)
        .await
        .map_err(|e| format!("写回失败: {e}"))?;
    Ok(format!("已替换 {replaced} 处"))
}
