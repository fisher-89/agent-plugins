use std::path::Path;

/// preamble 体量上限：超限截前 32KB（char 边界），尽力增强语义不炸 run。
const MAX_PREAMBLE_BYTES: usize = 32 * 1024;

/// 冻结指引常量（快照正文末段）：告知模型「系统提示词已冻结、规则真身在
/// `AGENT.md`、规则更新时用 `read` 重读」三点。
///
/// 三条硬约束：体量 ≤ 512 B（char 计，实际 107 char / 273 B）；MUST NOT 含路径
/// 字面量（只用文件名 `AGENT.md`——worktree 场景路径逐会话不同，写死即错）；
/// MUST NOT 含 `<>` 一类占位符（装配层静态常量，无注入位）。MUST NOT 单独成条
/// （只随非空正文出现）。
pub(crate) const GUIDANCE: &str = "（本会话的系统提示词已在会话开始时冻结，此后每轮逐字复用，不因文件变更而改写。项目规则的真实出处是 AGENT.md；若用户提示规则已更新，请用 read 工具重新读取 AGENT.md，并按其中的最新规则执行。）";

/// 读 `<root>/AGENT.md`：存在逐字返回（超 32KB 截 char 边界前缀），缺席或
/// 任何读取失败（非 UTF-8、非 NotFound 的 IO 错误同列）返回 `None`——读取
/// 侧永不报错，`None` 即「本轮无系统提示词」的合法形态。
///
/// 调用方是组合根的会话首轮与存量补拍（每会话至多一次读取）；引擎执行体不读
/// 文件。
pub(crate) fn load(root: &Path) -> Option<String> {
    let agent_file_path_buf = root.join("AGENT.md");
    let text = std::fs::read_to_string(&agent_file_path_buf).ok()?;
    let agent_file_path = agent_file_path_buf.to_str().unwrap_or("AGENT.md");
    let truncate_text = truncate(&text);
    Some(format!(
        "<agent_md path=\"{agent_file_path}\">\n{truncate_text}\n</agent_md>"
    ))
}

/// 快照正文合成单点：非空段按「知识 → `AGENT.md` 段 → [`GUIDANCE`]」以单空行
/// 连接（空串视同缺席，不 trim——`AGENT.md` 段逐字语义不许改动空白）；知识与
/// `AGENT.md` 段皆空 → 空串（无正文可冻结，指引常量 MUST NOT 单独成文）。
///
/// 32KB 截断已在 [`load`] 内完成，本函数 MUST NOT 再截一次（两处截断必然在
/// 32KB 边界上漂移）。
pub(crate) fn freeze(knowledge: Option<&str>, agent_md: Option<&str>) -> String {
    let mut segments: Vec<&str> = Vec::with_capacity(3);
    segments.extend(knowledge.filter(|text| !text.is_empty()));
    segments.extend(agent_md.filter(|text| !text.is_empty()));
    if segments.is_empty() {
        return String::new();
    }
    segments.push(GUIDANCE);
    segments.join("\n\n")
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
