use std::path::Path;

/// preamble 体量上限：超限截前 32KB（char 边界），尽力增强语义不炸 run。
const MAX_PREAMBLE_BYTES: usize = 32 * 1024;

/// 读 `<root>/AGENT.md`：存在逐字返回（超 32KB 截 char 边界前缀），缺席或
/// 任何读取失败（非 UTF-8、非 NotFound 的 IO 错误同列）返回 `None`——读取
/// 侧永不报错，`None` 即「本轮无系统提示词」的合法形态。
pub(crate) async fn load(root: &Path) -> Option<String> {
    let agent_file_path_buf = root.join("AGENT.md");
    let text = tokio::fs::read_to_string(&agent_file_path_buf).await.ok()?;
    let agent_file_path = agent_file_path_buf.to_str().unwrap_or("AGENT.md");
    let truncate_text = truncate(&text);
    Some(format!(
        "<agent_md path=\"{agent_file_path}\">\n{truncate_text}\n</agent_md>"
    ))
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
