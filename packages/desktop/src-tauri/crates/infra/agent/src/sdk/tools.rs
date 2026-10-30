use std::path::Path;

use archi::dto::{ElementKind, GateTier, WarningKind};
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
///
/// MUST 高于 L2 单结果 prune 门槛（8k tokens ≈ 32KB）——两层是「新结果封顶 /
/// 老结果淘汰」的关系，上限低于门槛会让 L2 第一层永不命中。
const MAX_RESULT_BYTES: usize = 50_000;

/// 大文件预览阈值（字节）：未给 `offset` / `limit` 且文件超此体量时，read 只
/// 返回前 [`PREVIEW_LINES`] 行 + 分页提示（避免一次把整份大文件灌进上下文）。
const LARGE_FILE_BYTES: u64 = 100 * 1024;

/// 大文件预览行数上限（`LARGE_FILE_BYTES` 触发时的缺省窗口）。
const PREVIEW_LINES: usize = 100;

/// 七通用工具封闭集基准面：policy 决策表与 compose 注入子集校验的基准；
/// `RunStarted.tools` 与 loop 成员校验自 [`definitions`] 本体派生（下发面
/// 单源——零第二份名单）。
pub const TOOL_NAMES: [&str; 7] = ["read", "grep", "glob", "ls", "write", "edit", "bash"];

/// `create_explore_note` 专用工具名（`toolset == Explore` 会话受控追加；执行
/// 体在 [`crate::sdk::explore_note`]，经 `LoopTurn` 上下文路由——不经
/// `tools::execute` 纯函数）。
pub(crate) const EXPLORE_NOTE_TOOL: &str = "create_explore_note";

/// `archi_module` 专用只读工具名（定位：已知在看哪个模块；`toolset == Archi
/// { module: true, .. }` 时受控追加）。执行体在 [`crate::sdk::archi_tool`]，下发
/// 判据单源在注入矩阵。
pub(crate) const ARCHI_MODULE_TOOL: &str = "archi_module";

/// `archi_search` 专用只读工具名（发现：还不知道功能归谁管；`toolset == Archi
/// { search: true, .. }` 时受控追加）。执行体同上。
pub(crate) const ARCHI_SEARCH_TOOL: &str = "archi_search";

/// 模型入参字符串字段提取（缺失或非字符串 → Err）。
fn string_field(input: &Value, field: &str) -> Result<String, String> {
    input
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("入参缺失或非字符串字段: {field}"))
}

/// 工具定义面（按会话注入面组装）：七通用工具按 `injections.tools` 保序过滤
/// （`None` = 全七——默认会话与上线前行为一致；`Some` = 子集，按
/// [`TOOL_NAMES`] 序过滤），`toolset` 命中时追加专用工具定义——`Explore` 追加
/// `create_explore_note`，`Archi { module, search }` 按位追加两个只读工具。
/// `RunStarted.tools` 与成员校验自本函数返回值派生（下发面单源）。
pub fn definitions(injections: &agent::SessionInjections) -> Vec<ToolDefinition> {
    let selected: Vec<&str> = match &injections.tools {
        None => TOOL_NAMES.to_vec(),
        Some(names) => TOOL_NAMES
            .iter()
            .filter(|name| names.iter().any(|n| n == *name))
            .copied()
            .collect(),
    };
    let mut definitions: Vec<ToolDefinition> =
        selected.into_iter().map(generic_definition).collect();
    match &injections.toolset {
        Some(agent::SessionToolset::Explore) => {
            definitions.push(ToolDefinition {
                name: EXPLORE_NOTE_TOOL.to_owned(),
                description:
                    "为当前探索会话的笔记落盘并定名。无路径参数——服务端按当前会话解析目标：\
                     占位会话（未定名）原子完成文件创建与记录定名（一次调用即最终名，勿先用 bash 造\
                     文件）；已定名会话仅接受与记录同名且文件缺失的补建。返回笔记路径（repo 相对、\
                     `/` 分隔），供 edit 续用"
                        .to_owned(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "本次探索的 kebab-case 最终名（小写字母/数字，可用 `-` 连接；已定名会话必须与记录同名）" },
                        "content": { "type": "string", "description": "笔记初稿全文（原样落盘）" }
                    },
                    "required": ["name", "content"]
                }),
            });
        }
        Some(agent::SessionToolset::Archi { module, search }) => {
            if *module {
                definitions.push(archi_module_definition());
            }
            if *search {
                definitions.push(archi_search_definition());
            }
        }
        None => {}
    }
    definitions
}

/// 词表拼接（`/` 分隔；值域清单由词汇单点派生，工具描述与响应共用同一份词表）。
fn vocabulary(words: impl IntoIterator<Item = &'static str>) -> String {
    words.into_iter().collect::<Vec<&str>>().join(" / ")
}

/// `archi_module` 定义（定位：邻接 / 直接子元素 / 文档引用 / 声明位置）。
fn archi_module_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_MODULE_TOOL.to_owned(),
        description: format!(
            "查询架构模型里的模块（定位：已知道在看哪个模块）。指定 element 返回该元素的\
             kind（{}）/ gate（{}）/ path / metadata、声明位置（modelFile + modelLine，可直接 edit 到该行）、\
             出边与入边（对端 FQN + 说明）、直接子元素与关联文档（四字段条目，正文用 read 自取）；\
             省略 element 返回全部元素的薄概览。模型写坏时返回结构化诊断（先修模型）；\
             docs 指向的文件不存在时以 {} 警告给出（只报不拦）",
            vocabulary(ElementKind::ALL.map(ElementKind::as_str)),
            vocabulary(GateTier::ALL.map(GateTier::as_str)),
            WarningKind::DocNotFound.as_str(),
        ),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "element": { "type": "string", "description": "元素 FQN（如 App.Auth）；省略时返回全部元素的薄概览" }
            },
            "required": []
        }),
    }
}

/// `archi_search` 定义（发现：还不知道功能归谁管）。
fn archi_search_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_SEARCH_TOOL.to_owned(),
        description: "在架构模型里按关键词检索元素与文档路径（发现：还不知道这功能归谁管时先搜）。\
             匹配对象为元素 FQN / summary / metadata 值 / 文档路径与文件名（不搜文档正文——\
             正文用 grep），大小写不敏感且支持驼峰分词（AuthToken → auth / token）。返回元素\
             薄条目与四字段文档条目"
            .to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "关键词 / 元素名片段 / 文档路径片段（不得为空白）" },
                "scope": { "type": "string", "enum": ["elements", "docs", "all"], "description": "检索范围（缺省 all）" }
            },
            "required": ["query"]
        }),
    }
}

/// 七通用工具定义（[`TOOL_NAMES`] 逐名 match；内容与参数化前的字面量逐字
/// 一致）。
fn generic_definition(name: &str) -> ToolDefinition {
    match name {
        "read" => ToolDefinition {
            name: "read".to_owned(),
            description:
                "读取文件内容（行号前缀输出）。可选 offset（1 起始行号）与 limit（行数）；\
                          单次至多 2000 行，超限截断留痕，可用 offset 翻页读取后续窗口。\
                          未指定 offset / limit 且文件较大时只预览前 100 行并提示分页——\
                          需要整读请显式给 offset / limit"
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
        "grep" => ToolDefinition {
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
        "glob" => ToolDefinition {
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
        "ls" => ToolDefinition {
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
        "write" => ToolDefinition {
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
        "edit" => ToolDefinition {
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
        "bash" => ToolDefinition {
            name: "bash".to_owned(),
            description: "执行 shell 命令并返回合并输出（stdout 段在前、stderr 段带 [stderr] \
                          标记在后）；cwd 为 workspace root，非零退出码报错并携带输出。超时为活性\
                          护栏，超时即终止进程树"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "待执行的命令行（git-bash / sh unix 语法优先，探测失败退 cmd）" },
                    "timeout_ms": { "type": "integer", "description": "超时毫秒（缺省 120000；须在 1000-1800000 内，越界报错不执行，超时终止进程树）" }
                },
                "required": ["command"]
            }),
        },
        other => unreachable!("generic_definition 仅承接 TOOL_NAMES 封闭集: {other}"),
    }
}

/// 沙箱校验对象提取：工具名 → 入参中的路径类字段值（glob 为 pattern，
/// 其余为 path；未知工具 None——policy 已先行拒绝，此处兜底）。两个架构只读工具
/// 无路径参数，显式返回 None（不进路径沙箱改写面——未来入参改名也不引入改写）。
pub fn input_path<'a>(name: &str, input: &'a Value) -> Option<&'a str> {
    if name == ARCHI_MODULE_TOOL || name == ARCHI_SEARCH_TOOL {
        return None;
    }
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
/// ToolResult 的错误串同形态截断，事件面不炸）。架构工具执行体复用同一上限
/// （[`crate::sdk::archi_tool`]）——MUST NOT 出现第二份上限实现。
pub(crate) fn cap_result(text: String) -> String {
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
///
/// **大文件预览**：未给 `offset` / `limit` 且文件体量超 [`LARGE_FILE_BYTES`]
/// 时，缺省窗口收窄为前 [`PREVIEW_LINES`] 行，并在尾部明示总行数与翻页式
/// ——大文件（如上千行 spec / 生成产物）一次整读会在后续每个请求里整段重发，
/// 是上下文体积最大的单点来源。显式 offset / limit 恒按调用方窗口执行。
async fn read(input: &Value) -> Result<String, String> {
    let path = string_field(input, "path")?;
    let explicit_offset = input.get("offset").and_then(Value::as_u64);
    let explicit_limit = input.get("limit").and_then(Value::as_u64);
    let offset = explicit_offset.map(|n| n.max(1) as usize).unwrap_or(1);
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取失败: {e}"))?;
    let lines: Vec<&str> = text.lines().collect();
    // 大文件预览：仅「两参皆缺」的整读形态收窄（显式窗口恒按调用方意图）
    let preview = explicit_offset.is_none()
        && explicit_limit.is_none()
        && text.len() as u64 > LARGE_FILE_BYTES;
    let default_limit = if preview {
        PREVIEW_LINES
    } else {
        MAX_READ_LINES
    };
    let limit = explicit_limit
        .map(|n| (n as usize).min(MAX_READ_LINES))
        .unwrap_or(default_limit);
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
        if preview {
            body.push_str(&format!(
                "\n（大文件预览：本段仅第 {}-{} 行，文件共 {} 行（{} 字节，超 {} 字节预览阈值）；\
                 请按需用 offset / limit 分页读取，或改用 grep 定位目标行）",
                start + 1,
                end,
                lines.len(),
                text.len(),
                LARGE_FILE_BYTES
            ));
        } else {
            body.push_str(&format!(
                "\n（已截断：本段为第 {}-{} 行，文件共 {} 行，可用 offset={} 翻页读取后续窗口）",
                start + 1,
                end,
                lines.len(),
                end + 1
            ));
        }
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
