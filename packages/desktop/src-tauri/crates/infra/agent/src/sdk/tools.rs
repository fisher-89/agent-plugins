use std::path::Path;

use regex::Regex;
use rig::completion::ToolDefinition;
use serde_json::Value;

use crate::sdk::bash;

/// glob 单次结果上限（防大目录扫描灌爆上下文）。
const MAX_GLOB_RESULTS: usize = 200;

/// grep 目录递归单次命中上限（同 glob 截断口径，防大目录扫描灌爆上下文）。
const MAX_GREP_RESULTS: usize = 200;

/// read 单次读取行数上限（缺省与显式 limit 同上限）：截断尾部留痕，offset
/// 翻页取回后续窗口。
const MAX_READ_LINES: usize = 2000;

/// L1 单结果字节上限（上下文防线第一层）：全工具统一收口（Ok / Err 双路），
/// 超限截断留痕；bash 输出同归此层。
const MAX_RESULT_BYTES: usize = 30_000;

/// 七工具名（loop 的 RunStarted.tools 与 policy 决策表同源口径，见
/// [`crate::sdk::policy`]）。
pub const TOOL_NAMES: [&str; 7] = ["read", "grep", "glob", "ls", "write", "edit", "bash"];

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
            description:
                "读取文件内容（行号前缀输出）。可选 offset（1 起始行号）与 limit（行数）；\
                          单次至多 2000 行，超限截断留痕，可用 offset 翻页读取后续窗口"
                    .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件路径" },
                    "offset": { "type": "integer", "description": "起始行号（1 起始，缺省 1）" },
                    "limit": { "type": "integer", "description": "读取行数（缺省至多 2000 行读到文件尾）" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "grep".to_owned(),
            description:
                "行级正则匹配：返回命中行（文件路径:行号: 内容）；path 传目录时递归扫描其下文件；\
                 可选 context 携带匹配行上下文（命中行 ± N 行）"
                    .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件或目录路径（目录时递归扫描）" },
                    "pattern": { "type": "string", "description": "正则表达式（regex 语法，非法正则报错）" },
                    "context": { "type": "integer", "description": "匹配行上下文行数（命中行 ± N，缺省 0；窗口合并去重，不连续组间以 -- 分隔）" }
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
        ToolDefinition {
            name: "bash".to_owned(),
            description: "执行 shell 命令并返回合并输出（stdout 段在前、stderr 段带 [stderr] \
                          标记在后）；cwd 为 workspace root，非零退出码报错并携带输出。超时为活性\
                          护栏，超时即终止进程树"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "待执行的命令行（git-bash / sh unix 语法优先，探测失败退 cmd）" },
                    "timeout_ms": { "type": "integer", "description": "超时毫秒（缺省 120000，钳位 1000-600000，超时终止进程树）" }
                },
                "required": ["command"]
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

/// 工具执行分发：按工具名调用执行体。root 仅 glob / bash 消费（模式拼接
/// 基座 / 进程 cwd）。出口统一 L1 收口：单结果超 [`MAX_RESULT_BYTES`] 截断
/// 留痕（Ok / Err 双路，全工具覆盖）。
pub async fn execute(root: &Path, name: &str, input: &Value) -> Result<String, String> {
    let outcome = match name {
        "read" => read(input).await,
        "grep" => grep(input).await,
        "glob" => glob(root, input).await,
        "ls" => list_dir(input).await,
        "write" => write(input).await,
        "edit" => edit(input).await,
        "bash" => bash::execute(root, input).await,
        other => Err(format!("未知工具: {other}")),
    };
    match outcome {
        Ok(text) => Ok(cap_result(text)),
        Err(error) => Err(cap_result(error)),
    }
}

/// L1 收口：超 30KB 截至字节上限（char 边界回退）并尾部留痕（is_error
/// ToolResult 的错误串同形态截断，事件面不炸）。
fn cap_result(text: String) -> String {
    if text.len() <= MAX_RESULT_BYTES {
        return text;
    }
    let mut end = MAX_RESULT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n（已截断：单结果超 {MAX_RESULT_BYTES} 字节上限，仅保留前 {end} 字节）",
        &text[..end]
    )
}

/// read：行号前缀输出（`行号\t内容`，cat -n 形态），offset（1 起始）与
/// limit 截取；单次至多 [`MAX_READ_LINES`] 行（缺省与显式 limit 同上限），
/// 截断时尾部留痕「已截断，可用 offset 翻页」。
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
        .map(|n| (n as usize).min(MAX_READ_LINES))
        .unwrap_or(MAX_READ_LINES);
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取失败: {e}"))?;
    let lines: Vec<&str> = text.lines().collect();
    let start = (offset - 1).min(lines.len());
    let end = (start + limit).min(lines.len());
    let numbered: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{:>6}\t{}", start + index + 1, line))
        .collect();
    if numbered.is_empty() {
        return Ok("(空区间)".to_owned());
    }
    let mut body = numbered.join("\n");
    if end < lines.len() {
        body.push_str(&format!(
            "\n（已截断：本段为第 {}-{} 行，文件共 {} 行，可用 offset={} 翻页读取后续窗口）",
            start + 1,
            end,
            lines.len(),
            end + 1
        ));
    }
    Ok(body)
}

/// grep：行级正则匹配（regex 语法，非法正则 `Err`），命中行以
/// `路径:行号: 内容` 输出。path 为文件时单文件匹配；为目录时递归扫描其下
/// 文件（不可读文件跳过、命中上限截断留痕）——目录入参曾以「读取失败:
/// 拒绝访问 (os error 5)」形态误导为权限拒绝，递归分支即该形态的正解。
/// metadata 失败（缺失路径）回落单文件分支，保留显式读取错误语义。
async fn grep(input: &Value) -> Result<String, String> {
    let path = string_field(input, "path")?;
    let pattern = string_field(input, "pattern")?;
    if pattern.is_empty() {
        return Err("pattern 不得为空".to_owned());
    }
    let matcher = Regex::new(&pattern).map_err(|e| format!("非法正则: {e}"))?;
    let context = input
        .get("context")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(0);
    let is_dir = tokio::fs::metadata(&path)
        .await
        .map(|meta| meta.is_dir())
        .unwrap_or(false);
    if is_dir {
        return grep_dir(&path, &matcher, context).await;
    }
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取失败: {e}"))?;
    let lines: Vec<&str> = text.lines().collect();
    let hits = match_line_indices(&lines, &matcher);
    Ok(hits_body(
        render_file_hits(&path, &lines, &hits, context),
        &pattern,
    ))
}

/// 目录递归分支：`{dir}/**/*` 枚举（glob 字典序，与 glob 工具同 crate 同
/// 姿态），仅文件参与匹配；不可读文件（二进制 / 非 UTF-8）静默跳过——
/// 目录扫描不因个别文件中断（单文件显式指定的读取失败语义不弱化）。命中
/// 上限以命中行数计，超限截断留痕（末文件仅保留未溢出的前段命中）。
async fn grep_dir(dir: &str, matcher: &Regex, context: usize) -> Result<String, String> {
    let base = dir.replace('\\', "/");
    let full = format!("{}/**/*", base.trim_end_matches('/'));
    let entries = glob::glob(&full).map_err(|e| format!("非法 glob 模式: {e}"))?;
    let mut output: Vec<String> = Vec::new();
    let mut hit_count = 0usize;
    let mut truncated = false;
    for entry in entries.flatten() {
        if !entry.is_file() {
            continue;
        }
        // 输出路径统一 `/` 分隔（glob 条目在 Windows 上为字面前缀 + `\` 拼接
        // 段的混合形态；与 glob 工具的归一口径一致）
        let display = entry.to_string_lossy().replace('\\', "/");
        if let Ok(text) = tokio::fs::read_to_string(&entry).await {
            let lines: Vec<&str> = text.lines().collect();
            let hits = match_line_indices(&lines, matcher);
            if hits.is_empty() {
                continue;
            }
            let kept = hit_count + hits.len();
            if kept > MAX_GREP_RESULTS {
                truncated = true;
                let fit = MAX_GREP_RESULTS - hit_count;
                output.extend(render_file_hits(&display, &lines, &hits[..fit], context));
                break;
            }
            hit_count = kept;
            output.extend(render_file_hits(&display, &lines, &hits, context));
        }
    }
    let mut body = hits_body(output, matcher.as_str());
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

/// 单文件命中行号收集（正则语义匹配，行号升序）。
fn match_line_indices(lines: &[&str], matcher: &Regex) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| matcher.is_match(line))
        .map(|(index, _)| index)
        .collect()
}

/// 命中行号 ± context 的半开区间列表（相邻 / 重叠窗口合并——窗口合并去重，
/// 输出每行恰一次）。
fn merged_windows(hits: &[usize], context: usize, total: usize) -> Vec<(usize, usize)> {
    let mut windows: Vec<(usize, usize)> = Vec::new();
    for &hit in hits {
        let start = hit.saturating_sub(context);
        let end = (hit + context + 1).min(total);
        match windows.last_mut() {
            Some(window) if start <= window.1 => window.1 = end,
            _ => windows.push((start, end)),
        }
    }
    windows
}

/// 单文件命中渲染：context = 0 即裸命中行；context > 0 时窗口展开，不连续
/// 组间以 `--` 分隔（`路径:行号: 内容` 形态不变）。
fn render_file_hits(path: &str, lines: &[&str], hits: &[usize], context: usize) -> Vec<String> {
    if hits.is_empty() {
        return Vec::new();
    }
    if context == 0 {
        return hits
            .iter()
            .map(|&index| format!("{}:{}: {}", path, index + 1, lines[index]))
            .collect();
    }
    let mut output: Vec<String> = Vec::new();
    for (group, (start, end)) in merged_windows(hits, context, lines.len())
        .into_iter()
        .enumerate()
    {
        if group > 0 {
            output.push("--".to_owned());
        }
        output
            .extend((start..end).map(|index| format!("{}:{}: {}", path, index + 1, lines[index])));
    }
    output
}

fn normalize_trailing_recursive(pattern: &str) -> String {
    if pattern == "**" || pattern.ends_with("/**") {
        format!("{pattern}/*")
    } else {
        pattern.to_owned()
    }
}

async fn glob(root: &Path, input: &Value) -> Result<String, String> {
    let pattern = string_field(input, "pattern")?;
    let base = root.to_string_lossy().replace('\\', "/");
    let full = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        normalize_trailing_recursive(&pattern.replace('\\', "/"))
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
