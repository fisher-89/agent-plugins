use std::path::{Path, PathBuf};

use crate::dto::GateTier;
use crate::parse::toml::TomlParser;
use crate::parse::{self, SourceParser};

/// 待落盘产物（绝对路径 + 全文）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateWrite {
    /// 目标模型文件（绝对路径）
    pub file: PathBuf,
    /// 待落盘全文
    pub content: String,
}

/// `gate` 写回纯函数：元素存在性 + 渲染自证两道校验，任一失败即 `Err` 且零文本
/// 产出（MUST NOT 把坏文本交给调用方落盘）。无显式 `gate` 行的元素允许插入该行
/// （缺省 `off` 的显式化是 ratchet 的正当动作）。
pub fn set_gate(source: &str, fqn: &str, gate: GateTier) -> Result<String, String> {
    let mut document = source
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("模型源解析失败：{error}"))?;
    {
        let table = document
            .as_table_mut()
            .get_mut("element")
            .and_then(toml_edit::Item::as_table_like_mut)
            .ok_or_else(|| "模型源缺少 element 表：无从定位元素".to_owned())?;
        if !table.contains_key(fqn) {
            let existing: Vec<String> = table.iter().map(|(key, _)| key.to_owned()).collect();
            return Err(format!(
                "元素不存在：{fqn}（现有元素：{}）",
                existing.join(" / ")
            ));
        }
        let entry = table
            .get_mut(fqn)
            .and_then(toml_edit::Item::as_table_like_mut)
            .ok_or_else(|| format!("元素 {fqn} 的声明不是表形态：无法改档"))?;
        entry.insert("gate", toml_edit::value(gate.as_str()));
    }
    let rendered = document.to_string();
    // 自证可解析：重解析零诊断且目标档位回读一致（否则零文本产出）
    let outcome = TomlParser::parse(&rendered);
    if !outcome.diagnostics.is_empty() {
        return Err(format!(
            "改档渲染自证失败（重解析出 {} 条诊断），未产出文本",
            outcome.diagnostics.len()
        ));
    }
    match outcome.model.element(fqn) {
        Some(element) if element.gate == gate => Ok(rendered),
        Some(element) => Err(format!(
            "改档渲染自证失败：回读档位为 {}，目标为 {}",
            element.gate.as_str(),
            gate.as_str()
        )),
        None => Err(format!("改档渲染自证失败：回读找不到元素 {fqn}")),
    }
}

/// 模型定位 + 源读取 + [`set_gate`]：结构错模型不写（与「写坏模型 = 读出错」同
/// 构）→ 元素缺席即 `Err` → 返回待落盘产物。**零写盘**。
pub fn plan_set_gate(root: &Path, fqn: &str, gate: GateTier) -> Result<GateWrite, String> {
    let outcome = parse::parse_dir(root);
    if !outcome.diagnostics.is_empty() {
        return Err(format!(
            "模型存在 {} 条结构诊断，拒绝改档（先修好模型）：{}",
            outcome.diagnostics.len(),
            outcome
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.kind.as_str())
                .collect::<Vec<&str>>()
                .join(" / ")
        ));
    }
    let element = outcome.model.element(fqn).ok_or_else(|| {
        format!(
            "元素不存在：{fqn}（模型共 {} 个元素）",
            outcome.model.elements.len()
        )
    })?;
    let file = root.join(&element.file);
    let source = std::fs::read_to_string(&file)
        .map_err(|error| format!("模型文件读取失败（{}）：{error}", file.display()))?;
    let content = set_gate(&source, fqn, gate)?;
    Ok(GateWrite { file, content })
}
