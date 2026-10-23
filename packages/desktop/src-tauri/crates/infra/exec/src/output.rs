//! 进程输出解读面（脚本执行 step 统一抽象层：错误信息 + 末尾行双面）。
//!
//! spawn 点各自捕获合并 stdout+stderr 后，失败归因文本统一从
//! [`ProcessOutput`] 装配——不再各调用点自截尾部（旧「末 500 字符」口径会
//! 把 JS 错误首行截掉：错误消息在前、栈帧在后，尾部只剩栈帧尾巴）。

/// 错误行匹配集（小写化后行内包含即命中；封闭常量表，单测直驱——覆盖
/// rust `error:` / JS `Error:` / `Unable to` / `panic` / `throw` / 测试框架
/// `FAILED` / npm `ERR!` / python `Traceback` 等主流工具族）。
const ERROR_LINE_PATTERNS: [&str; 8] = [
    "error",
    "unable to",
    "panic",
    "exception",
    "throw",
    "failed",
    "npm err!",
    "traceback",
];

/// 错误信息最多组数（前 N 组——多错误场景取主因集合）。
const ERROR_GROUP_MAX: usize = 10;

/// 每组错误行后续续行上限（错误消息常跨多行，续行也含错误信息；遇空行
/// 即止——空行不入组）。
const ERROR_CONTINUATION_MAX: usize = 3;

/// 输出末尾行数（行口径——替代旧字符截断口径）。
const TAIL_LINES: usize = 50;

/// 进程输出解读面：退出码 + 合并 stdout/stderr 原始字节（零截断零改写）。
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    /// 进程退出码（超时等缺码场景记 -1）
    pub exit_code: i32,
    /// 合并 stdout+stderr 原始字节
    bytes: Vec<u8>,
}

impl ProcessOutput {
    /// 构造（`bytes` = 合并 stdout+stderr 原文）。
    pub fn new(exit_code: i32, bytes: Vec<u8>) -> Self {
        Self { exit_code, bytes }
    }

    /// 失败主因提取面：前 10 个错误行组（每组 = 错误行 + 后续至多 3 行续
    /// 行、遇空行止；已被前组消费的行不再开新组）。无命中 → 空串。超 10
    /// 组截前 10 并尾加 `…`；组间以 `…` 行分隔（标记不连续）。
    pub fn error_excerpt(&self) -> String {
        let lines = self.lines();
        let mut groups: Vec<Vec<&str>> = Vec::new();
        let mut index = 0usize;
        while index < lines.len() && groups.len() < ERROR_GROUP_MAX {
            let line = &lines[index];
            let lowered = line.to_lowercase();
            if !lowered.is_empty()
                && ERROR_LINE_PATTERNS
                    .iter()
                    .any(|pattern| lowered.contains(pattern))
            {
                let mut group = vec![line.as_str()];
                let mut next = index + 1;
                while next < lines.len()
                    && group.len() <= ERROR_CONTINUATION_MAX
                    && !lines[next].trim().is_empty()
                {
                    group.push(lines[next].as_str());
                    next += 1;
                }
                groups.push(group);
                index = next;
            } else {
                index += 1;
            }
        }
        if groups.is_empty() {
            return String::new();
        }
        let mut excerpt = groups
            .iter()
            .map(|group| group.join("\n"))
            .collect::<Vec<_>>()
            .join("\n…\n");
        // 截断标记（组数触上限且仍有未扫行——单列一行，不与组间分隔符叠拼）
        if index < lines.len() {
            excerpt.push_str("\n…");
        }
        excerpt
    }

    /// 输出末尾 50 行（行口径；不足全给、截断加 `…` 前缀行；空输出 → 空
    /// 串）。行剥尾随 `\r`（CRLF 输出正文化——contains 断言友好）。
    pub fn tail(&self) -> String {
        let text = String::from_utf8_lossy(&self.bytes);
        let trimmed = text.trim_end();
        if trimmed.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = trimmed
            .split('\n')
            .map(|line| line.trim_end_matches('\r').to_owned())
            .collect();
        if lines.len() <= TAIL_LINES {
            return lines.join("\n");
        }
        format!("…\n{}", lines[lines.len() - TAIL_LINES..].join("\n"))
    }

    /// 失败消息装配：`head`（调用方按各自语义格式化，含退出码字样）+
    /// （非空时）`错误信息` 段 +（非空时）`输出末尾` 段。空输出 → 逐字等于
    /// `head`（保住「空面无冒号缀」语义）；excerpt 与 tail 允许重叠，不去重。
    pub fn failure_message(&self, head: &str) -> String {
        let mut message = head.to_owned();
        let excerpt = self.error_excerpt();
        if !excerpt.is_empty() {
            message.push_str("\n错误信息：\n");
            message.push_str(&excerpt);
        }
        let tail = self.tail();
        if !tail.is_empty() {
            message.push_str("\n输出末尾（50 行）：\n");
            message.push_str(&tail);
        }
        message
    }

    /// 行集合（UTF-8 宽容 + 剥尾随 `\r`——CRLF 输出正文化）。
    fn lines(&self) -> Vec<String> {
        String::from_utf8_lossy(&self.bytes)
            .split('\n')
            .map(|line| line.trim_end_matches('\r').to_owned())
            .collect()
    }
}

/// rustc / clippy 诊断的源码摘录行判据（`285 |     b"…"` / `285 +     …`）。
///
/// 编译错误会把出错源码行原样渲染进诊断输出；该行中的字符串是被测源码文本，
/// MUST NOT 被瞬态签名匹配或被锁路径解析当作执行环境证据（测试 fixture 里的
/// `os error 32` / `failed to rename archive file` 曾令确定性编译错误重试 5 次
/// 并误报环境错误）。瞬态判定与锁归因共用本判据。
pub(crate) fn is_source_excerpt_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let digits = trimmed
        .char_indices()
        .find(|(_, ch)| !ch.is_ascii_digit())
        .map_or(trimmed.len(), |(index, _)| index);
    if digits == 0 {
        return false;
    }
    let rest = trimmed[digits..].trim_start();
    rest.starts_with('|') || rest.starts_with('+')
}

#[cfg(test)]
#[path = "output_test.rs"]
mod output_test;
