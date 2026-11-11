//! 静态清单表解析器（ID 契约面）：evaluator prompt 的 `## 静态清单` 段三列表
//!（`ID | 检查项 | 判断依据`）是 checklist 的 ID 与权威人读文案唯一来源。
//!
//! 三条规则（缺一即错，三条均有实测反例）：
//! 1. 只扫 `## 静态清单` 起、下一个 `## ` 标题止的段落——全文扫描会吃到子表
//!    （`### T8 格式检查详细项` 的 `T8-U1` / `T8-G1` 等子项）；
//! 2. 行首匹配 `| <ID> |`（`[A-Z]+[0-9]+`，两侧空格约束）才收——无边界匹配会把
//!    `T8-U1` 切成 `T8` / `U1` / `G1` 三个假 ID；
//! 3. 按出现序去重——正文重复引用不产生重复项。
//!
//! 零 `regex` 依赖（行首前缀 + 手写字符类判定足够，避免给本 crate 引新依赖）。

/// 静态清单表一行：ID 与第二列「检查项」权威正文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistEntry {
    /// 表首列 ID（形如 `C1` / `CA8`，`[A-Z]+[0-9]+`）
    pub id: String,
    /// 表第二列「检查项」正文（落账 `ChecklistItem.item` 的唯一来源）
    pub item: String,
}

/// 清单段标题（段落边界的起止判定单点）。
const SECTION_TITLE: &str = "## 静态清单";

/// 从 prompt 文本解析静态清单 `(id, 检查项)` 二元组（出现序去重）。
pub fn checklist_entries(prompt: &str) -> Vec<ChecklistEntry> {
    let mut entries: Vec<ChecklistEntry> = Vec::new();
    let mut in_section = false;
    for line in prompt.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim() == SECTION_TITLE {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        // 段落边界：下一个二级标题即止（`###` 子标题不终止段落，其行形态由
        // 表首列 ID 判定挡住）
        if line.starts_with("## ") {
            break;
        }
        let Some(entry) = parse_row(line) else {
            continue;
        };
        if entries.iter().any(|existing| existing.id == entry.id) {
            continue;
        }
        entries.push(entry);
    }
    entries
}

/// 表行解析：`| <ID> | <检查项> | …` → 入口（形态不符即 `None`）。
fn parse_row(line: &str) -> Option<ChecklistEntry> {
    let rest = line.strip_prefix("| ")?;
    let letters: String = rest.chars().take_while(char::is_ascii_uppercase).collect();
    if letters.is_empty() {
        return None;
    }
    let after_letters = &rest[letters.len()..];
    let digits: String = after_letters
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        return None;
    }
    let columns = after_letters[digits.len()..].strip_prefix(" |")?;
    // 第二列：到下一个 `|` 之前（第三列「判断依据」不在本解析面）
    let item = columns.split('|').next()?.trim();
    if item.is_empty() {
        return None;
    }
    Some(ChecklistEntry {
        id: format!("{letters}{digits}"),
        item: item.to_owned(),
    })
}
