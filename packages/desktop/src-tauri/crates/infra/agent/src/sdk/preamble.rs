//! AGENT.md 系统提示词读取（sdk 引擎内部特性，core 零触）：读 workspace root
//! 的 `AGENT.md` 单份文件作为请求 preamble 来源——存在逐字注入、缺席或任何
//! 读取失败一律 `None`、严格无 `CLAUDE.md` 等兜底回退、单文件不级联嵌套目录。
//! 每轮重读由 loop 调用点保证（bash 入工具面后 agent 可改自己的系统提示词，
//! 会话中改动下一轮生效，不做起播快照）。

use std::path::Path;

/// preamble 体量上限：超限截前 32KB（char 边界），尽力增强语义不炸 run。
const MAX_PREAMBLE_BYTES: usize = 32 * 1024;

/// 读 `<root>/AGENT.md`：存在逐字返回（超 32KB 截 char 边界前缀），缺席或
/// 任何读取失败（非 UTF-8、非 NotFound 的 IO 错误同列）返回 `None`——读取
/// 侧永不报错，`None` 即「本轮无系统提示词」的合法形态。
pub(crate) async fn load(root: &Path) -> Option<String> {
    let text = tokio::fs::read_to_string(root.join("AGENT.md"))
        .await
        .ok()?;
    Some(truncate(&text))
}

/// 32KB 截断：超限时回退至不越界的 char 边界（逐字语义 = 前缀原文，不包装
/// 不改写）。
fn truncate(text: &str) -> String {
    if text.len() <= MAX_PREAMBLE_BYTES {
        return text.to_owned();
    }
    let mut end = MAX_PREAMBLE_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}
