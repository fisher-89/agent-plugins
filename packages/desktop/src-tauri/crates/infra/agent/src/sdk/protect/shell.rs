// ---------------------------------------------------------------------------
// shell 命令写入面提取（双变体等长掩码 + token 纪律 + 三态 + 档 B + 批量还原）
//
// **精度唯一来源是掩码**：
// - 变体 A（[`mask_heredocs`]）只掩 heredoc 体、保留引号 → 跑写入正则与参数
//   切片（引号内的真实目标仍可匹配，`rm 'a b.txt'` 照旧可 tokenize）；
// - 变体 B（[`mask_quoted_and_heredoc`]）掩 heredoc 体 + 引号串 → 定位命令名
//   （字符串 / heredoc 数据里的 `rm` / `git stash` 不得被当作命令）。
//
// 两变体按**字符**逐位替换（填充 `MASK_CHAR`），故 char 索引 1:1 互通——命令
// 名在 B 上定位、参数在 A 上同索引切片。掩码 / 切片 / 正则匹配全程在
// `Vec<char>` 空间：JS 版索引单位是 UTF-16 code unit、Rust 以字节为界，直接
// 照译会因非 ASCII 命令文本（中文路径 / 中文注释）错位，故正则命中的字节偏移
// 先折算成 char 索引再切片。
//
// 正则纪律：全部模式经 `OnceLock` 预编译（单量词线性形态，禁嵌套量词 / 回溯
// 爆炸）；**不设命令文本长度上限**（设上限等于送出一条绕过路径）。`regex`
// crate 无零宽断言，故插件端 `>&` 的「目标不是纯数字」前瞻改写成**捕获后
// 过滤**（目标 token 全为 ASCII 数字即丢弃该 op），匹配边界零改。
//
// 精度纪律（MUST NOT "顺手优化"）：`>` 前缀排除集如实保持「只排 `-` 与 `=`」——
// 把 `&` / `|` 一并排掉会让 `echo {} &> <受保护文件>` 从拒静默变放行。over-block
// 方向不安全：宁可漏掉 stdout 形态，也 MUST NOT 让 `echo foo > notes.txt` 被误拦。
//
// 知情边界：`xargs` / `sh -c` 递归拆解、`find -exec rm {} +` 的占位符解析、
// 生成脚本再执行均不实现（与「不可静态判别」同类）；`find` 只点名 `-delete` 族。
// ---------------------------------------------------------------------------

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use crate::sdk::protect::rules::{self, BulkGlobs};

/// 掩码填充字符：非空白、不属任一提取字符类（与插件端同值），故被掩码文本
/// 永远拼不出可匹配的命令名 / 操作符 / 路径。
const MASK_CHAR: char = '\u{0}';

/// 单态文件操作：`Write`（重定向 / cmdlet / cp 目标位 / 内联改写）、`Delete`、
/// `Revert`（撤销动作，不入保护判定，仅批量还原消费）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileOp {
    /// 操作态
    pub(super) kind: OpKind,
    /// 操作落点
    pub(super) target: OpTarget,
}

/// 操作态三值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpKind {
    /// 写入 / 覆盖 / 追加 / 清空
    Write,
    /// 删除 / 清空即毁的搜索根
    Delete,
    /// 撤销（`git restore <path>` 裸形态、`git checkout -- <path>`）
    Revert,
}

/// 操作落点：`Path` 走「路径 match 规则」，`Tree` 走子树相交判定（`find` 的
/// 搜索根——匹配项不可静态预知，删除场景按「保护整个搜索根」保守处理）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OpTarget {
    /// 普通路径（相对 / 绝对，未归一）
    Path(String),
    /// 目录树搜索根（未归一）
    Tree(String),
}

/// 半开区间 `[start, end)`（char 索引）。
#[derive(Debug, Clone, Copy)]
struct Span {
    start: usize,
    end: usize,
}

// ---------------------------------------------------------------------------
// 掩码
// ---------------------------------------------------------------------------

/// 命令原文 → char 向量（掩码 / 切片 / 匹配的唯一索引空间）。
fn chars_of(command: &str) -> Vec<char> {
    command.chars().collect()
}

/// 区间合并（重叠 / 相邻合并，输出保序、互不重叠）。
fn merge_spans(spans: &[Span]) -> Vec<Span> {
    let mut sorted: Vec<Span> = spans.to_vec();
    sorted.sort_by_key(|span| span.start);
    let mut merged: Vec<Span> = Vec::new();
    for span in sorted {
        match merged.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    merged
}

/// 逐字符替换为 `MASK_CHAR`：输出字符数 == 输入字符数（两变体等长的机械保证）。
fn mask_chars(chars: &[char], spans: &[Span]) -> String {
    let mut masked = chars.to_vec();
    for span in merge_spans(spans) {
        let start = span.start.min(masked.len());
        let end = span.end.min(masked.len());
        for ch in &mut masked[start..end] {
            *ch = MASK_CHAR;
        }
    }
    masked.into_iter().collect()
}

/// 引号区间：`'…'`（bash 单引号无转义）、`"…"`（认 `\` 转义），单 / 双不嵌套。
/// 未闭合候选直接放弃并停止掩码（fail-open：其后一切按未掩码处理）。
fn quote_spans(chars: &[char]) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '\'' => {
                let mut close = index + 1;
                while close < chars.len() && chars[close] != '\'' {
                    close += 1;
                }
                if close >= chars.len() {
                    return spans;
                }
                spans.push(Span {
                    start: index,
                    end: close + 1,
                });
                index = close + 1;
            }
            '"' => {
                let mut close = index + 1;
                while close < chars.len() && chars[close] != '"' {
                    if chars[close] == '\\' {
                        close += 2;
                    } else {
                        close += 1;
                    }
                }
                if close >= chars.len() {
                    return spans;
                }
                spans.push(Span {
                    start: index,
                    end: close + 1,
                });
                index = close + 1;
            }
            _ => index += 1,
        }
    }
    spans
}

/// heredoc 开标记解析（marker 必须紧贴 `<<` / `<<-`；引号形态须闭合）：返回
/// `(marker, 开标记之后的位置)`；不成立即 `None`（候选丢弃）。
fn heredoc_marker(chars: &[char], from: usize) -> Option<(String, usize)> {
    let mut index = from;
    if index < chars.len() && chars[index] == '-' {
        index += 1;
    }
    let quote = if index < chars.len() && (chars[index] == '\'' || chars[index] == '"') {
        let quote = chars[index];
        index += 1;
        Some(quote)
    } else {
        None
    };
    let start = index;
    while index < chars.len() && (chars[index].is_ascii_alphanumeric() || chars[index] == '_') {
        index += 1;
    }
    if index == start {
        return None;
    }
    let marker: String = chars[start..index].iter().collect();
    if let Some(quote) = quote {
        if index < chars.len() && chars[index] == quote {
            index += 1;
        } else {
            return None;
        }
    }
    Some((marker, index))
}

/// heredoc 体终止行定位：逐行比对 `line.trim() == marker`（缩进形态同插件端），
/// 找不到终止行即 `None`（候选丢弃，不掩码）。
fn heredoc_body_end(chars: &[char], marker: &str, from: usize) -> Option<usize> {
    let mut pos = from;
    while pos <= chars.len() {
        let mut line_end = chars.len();
        let mut has_newline = false;
        for (offset, ch) in chars[pos..].iter().enumerate() {
            if *ch == '\n' {
                line_end = pos + offset;
                has_newline = true;
                break;
            }
        }
        let line: String = chars[pos..line_end].iter().collect();
        if line.trim() == marker {
            return Some(line_end);
        }
        if !has_newline {
            return None;
        }
        pos = line_end + 1;
    }
    None
}

/// heredoc 体区间（`<<MARKER` … 之后一行等于 MARKER）。`<<` 必须在引号外且
/// 左邻为行首 / 空白 / `;&|(`（故 JS 位移 `1 << 2` 永不构成开标记），且必须
/// 同时找到换行与终止 marker 行，否则候选丢弃（fail-open，不掩码）。
fn heredoc_spans(chars: &[char]) -> Vec<Span> {
    let quotes = quote_spans(chars);
    let in_quotes = |index: usize| {
        quotes
            .iter()
            .any(|span| index >= span.start && index < span.end)
    };
    let mut spans = Vec::new();
    let mut index = 0;
    while index + 1 < chars.len() {
        if chars[index] != '<' || chars[index + 1] != '<' {
            index += 1;
            continue;
        }
        let boundary = index == 0
            || matches!(
                chars[index - 1],
                ' ' | '\t' | '\r' | '\n' | ';' | '&' | '|' | '('
            );
        if !boundary || in_quotes(index) {
            index += 1;
            continue;
        }
        let Some((marker, after)) = heredoc_marker(chars, index + 2) else {
            index += 1;
            continue;
        };
        let Some(header) = chars[after..]
            .iter()
            .position(|ch| *ch == '\n')
            .map(|offset| after + offset)
        else {
            index += 1;
            continue;
        };
        if let Some(end) = heredoc_body_end(chars, &marker, header + 1) {
            spans.push(Span {
                start: header + 1,
                end,
            });
        }
        index += 1;
    }
    spans
}

/// 变体 A：只掩 heredoc 体、保留引号（写入正则与参数切片的运行面）。
pub(super) fn mask_heredocs(command: &str) -> String {
    let chars = chars_of(command);
    mask_chars(&chars, &heredoc_spans(&chars))
}

/// 变体 B：掩 heredoc 体 + 引号串（命令名定位面）。两变体等长，char 索引
/// 1:1 互通。
pub(super) fn mask_quoted_and_heredoc(command: &str) -> String {
    let chars = chars_of(command);
    let mut spans = heredoc_spans(&chars);
    spans.extend(quote_spans(&chars));
    mask_chars(&chars, &spans)
}

/// 两变体一次算齐（名称文本 / 参数文本）。
fn masked_variants(command: &str) -> (String, String) {
    (mask_quoted_and_heredoc(command), mask_heredocs(command))
}

// ---------------------------------------------------------------------------
// token 纪律
// ---------------------------------------------------------------------------

/// 空白切分 + 剥包裹引号 + 剔空。**不做** `$` 过滤：`git clean` 的零路径判定
/// 依赖原始 token 计数（`$` 过滤只在操作落点做）。
pub(super) fn tokenize(args: &str) -> Vec<String> {
    args.split_whitespace()
        .map(|token| token.trim_matches(['\'', '"']).to_owned())
        .filter(|token| !token.is_empty())
        .collect()
}

/// option token（`-f` / `-rf` / `--cached`）。
pub(super) fn is_flag(token: &str) -> bool {
    token.starts_with('-')
}

/// 未展开变量 token（含 `$`）在操作落点丢弃：执行前不可知，字面 token 永远
/// 命不中真实路径。**只在落点用，绝不进 [`tokenize`]**。
fn is_literal_path(token: &str) -> bool {
    !token.contains('$')
}

// ---------------------------------------------------------------------------
// 正则表（全部 OnceLock 预编译）
// ---------------------------------------------------------------------------

/// 静态模式 → 预编译表（编译失败按不命中收敛：静态字面量非法属编译期缺陷，
/// 不 panic、不拒绝启动）。
fn compile(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .collect()
}

/// 重定向写入族（在变体 A 上跑）：`>` / `>>` / `>|` / `tee`。
fn redirect_write_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        compile(&[
            // > 与 >>（前缀守卫 [^-=]：跳过 -> 与 =>）
            r#"(?:^|[^-=])>{1,2}\s+['"]?([^\s;|`$&()'"]+)['"]?"#,
            // >|（noclobber 覆盖）
            r#">\|\s+['"]?([^\s;|`$&()'"]+)['"]?"#,
            // tee（可带 -a 等 flag）
            r#"(?:^|[\s;|&(])\s*tee\s+(?:-[a-zA-Z]+\s+)?['"]?([^\s;|`$&()'"]+)['"]?"#,
        ])
    })
}

/// `>&` 族（独立成表：其目标可能是 fd 复制，需捕获后过滤纯数字）。
fn descriptor_dup_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r#">&\s*['"]?([^\s;|`$&()'"]+)['"]?"#]))
}

/// PowerShell 重定向（`*>` / `N>` / `>` / `>>`）。
fn powershell_redirect_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        compile(&[r#"(?:^|[\s;|&(])(?:\d*|\*)?>{1,2}\s+['"]?([^\s;|`$&()'",=]+)['"]?"#])
    })
}

/// PowerShell 文件写入 cmdlet（可带 `-Path` / `-FilePath`）。
fn powershell_cmdlet_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        const CMDLETS: [&str; 6] = [
            "Set-Content",
            "Out-File",
            "Add-Content",
            "Export-Csv",
            "Export-CliXml",
            "Tee-Object",
        ];
        let patterns: Vec<String> = CMDLETS
            .iter()
            .map(|cmdlet| {
                format!(
                    r#"(?i)\b{cmdlet}\s+(?:-(?:Path|FilePath)\s+)?['"]?([^\s;`$&()'",=]+)['"]?"#
                )
            })
            .collect();
        let borrowed: Vec<&str> = patterns.iter().map(String::as_str).collect();
        compile(&borrowed)
    })
}

/// .NET 静态写方法（首参为路径）。
fn dotnet_write_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        compile(&[
            r#"(?i)\[System\.IO\.File\]::WriteAllText\s*\(\s*['"]?([^,;)'"]+)['"]?"#,
            r#"(?i)\[System\.IO\.File\]::WriteAllLines\s*\(\s*['"]?([^,;)'"]+)['"]?"#,
            r#"(?i)\[System\.IO\.File\]::WriteAllBytes\s*\(\s*['"]?([^,;)'"]+)['"]?"#,
            r#"(?i)\[System\.IO\.File\]::AppendAllText\s*\(\s*['"]?([^,;)'"]+)['"]?"#,
        ])
    })
}

/// 删除族命令名。
fn delete_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        compile(&[r"(?i)(?:^|[\s;&|(])(?:Remove-Item|rmdir|unlink|rm|del|rd)\s+([^;&|\n]*)"])
    })
}

/// 双参移动族命令名（旧删 + 新写）。
fn move_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        compile(&[r"(?i)(?:^|[\s;&|(])(?:Move-Item|Rename-Item|ren|mv)\s+([^;&|\n]*)"])
    })
}

/// `git restore|checkout|rm` 子命令（组 1 = 子命令，组 2 = 参数串）。
fn git_subcommand_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r"(?i)\bgit\s+(restore|checkout|rm)\b([^\n;&|]*)"]))
}

/// 档 B：`cp` / `Copy-Item`（目标位 = 末个非 flag token）。
fn copy_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r"(?i)(?:^|[\s;&|(])(?:cp|Copy-Item)\s+([^;&|\n]*)"]))
}

/// 档 B：`dd`（取 `of=`）。
fn dd_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r"(?i)(?:^|[\s;&|(])dd\s+([^;&|\n]*)"]))
}

/// 档 B：`sed` / `perl`（仅 `-i` 形态）。
fn sed_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r"(?i)(?:^|[\s;&|(])(?:sed|perl)\s+([^;&|\n]*)"]))
}

/// 档 B：`truncate`（`-s` / `--size=` 之后的路径）。
fn truncate_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r"(?i)(?:^|[\s;&|(])truncate\s+([^;&|\n]*)"]))
}

/// 档 B：`find`（仅 `-delete` 形态，搜索根走子树相交判定）。
fn find_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r"(?i)(?:^|[\s;&|(])find\s+([^;&|\n]*)"]))
}

/// 批量还原：`git stash`。
fn stash_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r"(?i)\bgit\s+stash\b([^\n;&|]*)"]))
}

/// 批量还原：`git clean`。
fn clean_command_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| compile(&[r"(?i)\bgit\s+clean\b([^\n;&|]*)"]))
}

// ---------------------------------------------------------------------------
// 命令名定位 + 参数切片
// ---------------------------------------------------------------------------

/// `name_text` 的字节偏移 → char 索引（两变体等长，故 char 索引直接用于变体 A）。
fn byte_to_char(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .map(|prefix| prefix.chars().count())
        .unwrap_or_else(|| text.chars().count())
}

/// 在变体 B 上定位命令名、按同索引从变体 A 切参数串（组 1 = 参数串）。
fn scan_args(regexes: &[Regex], name_text: &str, arg_text: &str) -> Vec<String> {
    let arg_chars: Vec<char> = arg_text.chars().collect();
    let mut results = Vec::new();
    for regex in regexes {
        for captures in regex.captures_iter(name_text) {
            let Some(group) = captures.get(1) else {
                continue;
            };
            let start = byte_to_char(name_text, group.start()).min(arg_chars.len());
            let end = byte_to_char(name_text, group.end()).min(arg_chars.len());
            if start > end {
                continue;
            }
            results.push(arg_chars[start..end].iter().collect::<String>());
        }
    }
    results
}

/// [`scan_args`] 的双组变体（组 1 = 子命令，组 2 = 参数串）。
fn scan_groups(regexes: &[Regex], name_text: &str, arg_text: &str) -> Vec<(String, String)> {
    let arg_chars: Vec<char> = arg_text.chars().collect();
    let mut results = Vec::new();
    for regex in regexes {
        for captures in regex.captures_iter(name_text) {
            let (Some(lead), Some(args)) = (captures.get(1), captures.get(2)) else {
                continue;
            };
            let start = byte_to_char(name_text, args.start()).min(arg_chars.len());
            let end = byte_to_char(name_text, args.end()).min(arg_chars.len());
            if start > end {
                continue;
            }
            results.push((
                lead.as_str().to_owned(),
                arg_chars[start..end].iter().collect::<String>(),
            ));
        }
    }
    results
}

/// 操作落点构造（`$` / 空串 token 在落点丢弃 → 该 op 不产出）。
fn path_op(kind: OpKind, token: &str) -> Option<FileOp> {
    let token = token.trim();
    if token.is_empty() || !is_literal_path(token) {
        return None;
    }
    Some(FileOp {
        kind,
        target: OpTarget::Path(token.to_owned()),
    })
}

// ---------------------------------------------------------------------------
// 三态提取
// ---------------------------------------------------------------------------

/// 重定向写入族（`>` / `>>` / `>|` / `tee` + `>&`）；`>&` 的纯数字目标在这里
/// 过滤（替代 JS 前瞻：`2>&1` / `1>&2` / `>&2` 是 fd 复制，非文件）。
fn redirect_write_ops(arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for regex in redirect_write_regexes() {
        for captures in regex.captures_iter(arg_text) {
            if let Some(target) = captures.get(1) {
                ops.extend(path_op(OpKind::Write, target.as_str()));
            }
        }
    }
    for regex in descriptor_dup_regexes() {
        for captures in regex.captures_iter(arg_text) {
            let Some(target) = captures.get(1) else {
                continue;
            };
            let target = target.as_str();
            // 裸整数目标 = 描述符复制（`2>&1`），不是文件
            if target.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            ops.extend(path_op(OpKind::Write, target));
        }
    }
    ops
}

/// PowerShell 方言写入（重定向 / 六 cmdlet / `[System.IO.File]::WriteAll*`）。
fn powershell_write_ops(arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for regexes in [
        powershell_redirect_regexes(),
        powershell_cmdlet_regexes(),
        dotnet_write_regexes(),
    ] {
        for regex in regexes {
            for captures in regex.captures_iter(arg_text) {
                if let Some(target) = captures.get(1) {
                    ops.extend(path_op(OpKind::Write, target.as_str()));
                }
            }
        }
    }
    ops
}

/// 删除族（`rm` / `rmdir` / `unlink` / `del` / `rd` / `Remove-Item`；`git rm`
/// 由 git 子命令面另记，判定侧首个命中即止消化）。
fn delete_ops(name_text: &str, arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for args in scan_args(delete_command_regexes(), name_text, arg_text) {
        for token in tokenize(&args) {
            if is_flag(&token) {
                continue;
            }
            ops.extend(path_op(OpKind::Delete, &token));
        }
    }
    ops
}

/// 双参移动族：旧路径 delete + 新路径 write（恰两个字面路径 token 才成立）。
fn move_ops(name_text: &str, arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for args in scan_args(move_command_regexes(), name_text, arg_text) {
        let paths: Vec<String> = tokenize(&args)
            .into_iter()
            .filter(|token| !is_flag(token) && is_literal_path(token))
            .collect();
        if paths.len() == 2 {
            ops.extend(path_op(OpKind::Delete, &paths[0]));
            ops.extend(path_op(OpKind::Write, &paths[1]));
        }
    }
    ops
}

/// `git rm <paths>` → delete（跳过 flag）。
fn git_rm_ops(tokens: &[String]) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for token in tokens {
        if is_flag(token) {
            continue;
        }
        ops.extend(path_op(OpKind::Delete, token));
    }
    ops
}

/// `git restore` 参数 → ops（`--source=` / `-s <ref>` → write，裸形态 → revert）。
fn restore_ops(tokens: &[String]) -> Vec<FileOp> {
    let mut from_source = false;
    let mut ops = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        if token.starts_with("--source=") || token == "-s" {
            from_source = true;
            // 分离值形态（`-s <ref>`）：跳过 commit-ish
            index += if token == "-s" { 2 } else { 1 };
            continue;
        }
        if !is_flag(token) {
            let kind = if from_source {
                OpKind::Write
            } else {
                OpKind::Revert
            };
            ops.extend(path_op(kind, token));
        }
        index += 1;
    }
    ops
}

/// `git checkout -- <paths>` → ops（`--` 之前的非 flag token 决定 write / revert；
/// commit-ish 侧不做 `$` 过滤，故 `git checkout $REF -- src/a.ts` 仍判 write）。
fn checkout_ops(tokens: &[String]) -> Vec<FileOp> {
    let Some(sep) = tokens.iter().position(|token| token == "--") else {
        return Vec::new();
    };
    let from_source = tokens[..sep].iter().any(|token| !is_flag(token.as_str()));
    let mut ops = Vec::new();
    for token in &tokens[sep + 1..] {
        if is_flag(token.as_str()) {
            continue;
        }
        let kind = if from_source {
            OpKind::Write
        } else {
            OpKind::Revert
        };
        ops.extend(path_op(kind, token.as_str()));
    }
    ops
}

/// `git restore` / `checkout` / `rm` 的 write / revert / delete ops。
fn git_ops(name_text: &str, arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for (sub, args) in scan_groups(git_subcommand_regexes(), name_text, arg_text) {
        let tokens = tokenize(&args);
        match sub.to_ascii_lowercase().as_str() {
            "rm" => ops.extend(git_rm_ops(&tokens)),
            "restore" => ops.extend(restore_ops(&tokens)),
            "checkout" => ops.extend(checkout_ops(&tokens)),
            _ => {}
        }
    }
    ops
}

/// 档 B：`cp` / `Copy-Item` 的目标位（末个非 flag 非 `$` token）→ write。
fn copy_ops(name_text: &str, arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for args in scan_args(copy_command_regexes(), name_text, arg_text) {
        let tokens = tokenize(&args);
        let target = tokens
            .iter()
            .rev()
            .find(|token| !is_flag(token.as_str()) && is_literal_path(token.as_str()));
        if let Some(target) = target {
            ops.extend(path_op(OpKind::Write, target.as_str()));
        }
    }
    ops
}

/// 档 B：`dd` 的 `of=<path>` → write（`if=` 是源，天然不取）。
fn dd_ops(name_text: &str, arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for args in scan_args(dd_command_regexes(), name_text, arg_text) {
        for token in tokenize(&args) {
            let Some(value) = token.strip_prefix("of=") else {
                continue;
            };
            ops.extend(path_op(OpKind::Write, value.trim_matches(['\'', '"'])));
        }
    }
    ops
}

/// 原地改写 flag 判据：`-i` / `-i.bak` / `-pie` 紧贴形态与 `--in-place[=…]`。
fn is_in_place_flag(token: &str) -> bool {
    if token == "--in-place" || token.starts_with("--in-place=") {
        return true;
    }
    if !token.starts_with('-') || token.starts_with("--") {
        return false;
    }
    let cluster = token[1..].split('.').next().unwrap_or_default();
    cluster.contains('i')
}

/// 档 B：`sed -i` / `perl -i` 的末个非 flag 非 `$` token → write（无 `-i`
/// 是纯读，恒零 op——纯读 MUST NOT 误拦）。
fn sed_ops(name_text: &str, arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for args in scan_args(sed_command_regexes(), name_text, arg_text) {
        let tokens = tokenize(&args);
        if !tokens.iter().any(|token| is_in_place_flag(token)) {
            continue;
        }
        let target = tokens
            .iter()
            .rev()
            .find(|token| !is_flag(token.as_str()) && is_literal_path(token.as_str()));
        if let Some(target) = target {
            ops.extend(path_op(OpKind::Write, target.as_str()));
        }
    }
    ops
}

/// 档 B：`truncate` 的 `-s` / `--size=` 取值之后的第一个非 flag 非 `$` token
/// → write（`-r` 引用形态不覆盖）。
fn truncate_ops(name_text: &str, arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for args in scan_args(truncate_command_regexes(), name_text, arg_text) {
        let tokens = tokenize(&args);
        let mut rest = None;
        let mut index = 0;
        while index < tokens.len() {
            let token = tokens[index].as_str();
            if token == "-s" || token == "--size" {
                rest = Some(index + 2); // 跳过 flag 与其取值
                break;
            }
            if token.starts_with("--size=") {
                rest = Some(index + 1);
                break;
            }
            index += 1;
        }
        let Some(from) = rest else {
            continue;
        };
        let target = tokens
            .iter()
            .skip(from)
            .find(|token| !is_flag(token.as_str()) && is_literal_path(token.as_str()));
        if let Some(target) = target {
            ops.extend(path_op(OpKind::Write, target.as_str()));
        }
    }
    ops
}

/// 档 B：`find … -delete` 的搜索根（首个非 flag 非 `$` token，缺省 `.`）→
/// delete + `Tree`（匹配项不可静态预知，删除场景按「保护整个搜索根」保守处理）。
fn find_ops(name_text: &str, arg_text: &str) -> Vec<FileOp> {
    let mut ops = Vec::new();
    for args in scan_args(find_command_regexes(), name_text, arg_text) {
        let tokens = tokenize(&args);
        if !tokens.iter().any(|token| token == "-delete") {
            continue;
        }
        let root = tokens
            .iter()
            .find(|token| !is_flag(token.as_str()) && is_literal_path(token.as_str()))
            .cloned()
            .unwrap_or_else(|| ".".to_owned());
        ops.push(FileOp {
            kind: OpKind::Delete,
            target: OpTarget::Tree(root),
        });
    }
    ops
}

/// 三态提取总入口（保序、不折叠、不去重：`git rm` 与 `rm` 双记由判定侧
/// 「首个命中即止」消化）：重定向族 → PS 方言 → 删除族 → 移动族 → git 子命令
/// → 档 B 五族。
pub(super) fn extract_file_ops(command: &str) -> Vec<FileOp> {
    let (name_text, arg_text) = masked_variants(command);
    let mut ops = Vec::new();
    ops.extend(redirect_write_ops(&arg_text));
    ops.extend(powershell_write_ops(&arg_text));
    ops.extend(delete_ops(&name_text, &arg_text));
    ops.extend(move_ops(&name_text, &arg_text));
    ops.extend(git_ops(&name_text, &arg_text));
    ops.extend(copy_ops(&name_text, &arg_text));
    ops.extend(dd_ops(&name_text, &arg_text));
    ops.extend(sed_ops(&name_text, &arg_text));
    ops.extend(truncate_ops(&name_text, &arg_text));
    ops.extend(find_ops(&name_text, &arg_text));
    ops
}

/// `git restore` / `checkout` / `rm` 的 ops（批量还原判定复用；`revert` 态只在
/// 这里被消费）。
pub(super) fn extract_git_ops(command: &str) -> Vec<FileOp> {
    let (name_text, arg_text) = masked_variants(command);
    git_ops(&name_text, &arg_text)
}

// ---------------------------------------------------------------------------
// 批量还原拦截
// ---------------------------------------------------------------------------

/// `git clean` 的预览形态（`--dry-run` 或 `-[^-]*n`）：放行。
fn is_dry_run(token: &str) -> bool {
    if token == "--dry-run" {
        return true;
    }
    token.starts_with('-') && !token.starts_with("--") && token.chars().skip(1).any(|ch| ch == 'n')
}

/// 批量还原 / 清理判定：命中即返回**中间事实串**（由
/// `reason::bulk_revert_reason` 包成完整文案）。
///
/// 三条规则：`git stash`（除 `list` / `show`）→ 拒；`git clean`（无路径参数 /
/// 任一目录参数命中域目录树）→ 拒（`-n` / `--dry-run` 放行）；`git restore` /
/// `git checkout --` 的 revert 目标命中 changes 子树 → 拒（源码目标放行——逐
/// 路径撤销是规范动作）。
///
/// `cwd` 只作路径解析与子树判定锚定基座，内部零 IO。
pub(super) fn bulk_revert_detail(command: &str, globs: &BulkGlobs, cwd: &Path) -> Option<String> {
    let (name_text, arg_text) = masked_variants(command);

    for args in scan_args(stash_command_regexes(), &name_text, &arg_text) {
        let first = tokenize(&args).into_iter().next();
        let readonly = first
            .as_deref()
            .map(|token| token.eq_ignore_ascii_case("list") || token.eq_ignore_ascii_case("show"))
            .unwrap_or(false);
        if !readonly {
            return Some("git stash 会批量改写工作区".to_owned());
        }
    }

    for args in scan_args(clean_command_regexes(), &name_text, &arg_text) {
        let tokens = tokenize(&args);
        if tokens.iter().any(|token| is_dry_run(token)) {
            continue;
        }
        let paths: Vec<&String> = tokens
            .iter()
            .filter(|token| !is_flag(token.as_str()))
            .collect();
        if paths.is_empty() {
            return Some("git clean 会批量删除文件（未限定路径）".to_owned());
        }
        for path in paths {
            let Some(dir) = rules::resolve(cwd, path) else {
                continue;
            };
            if rules::glob_dir_hit(&dir, &globs.artifacts, cwd) {
                return Some(format!("git clean 会批量删除文件: {dir}"));
            }
        }
    }

    for op in extract_git_ops(command) {
        if op.kind != OpKind::Revert {
            continue;
        }
        let OpTarget::Path(path) = op.target else {
            continue;
        };
        let Some(resolved) = rules::resolve(cwd, &path) else {
            continue;
        };
        if rules::glob_dir_hit(&resolved, &globs.changes, cwd) {
            return Some(format!("git restore 作用于工作流产物: {resolved}"));
        }
    }

    None
}
