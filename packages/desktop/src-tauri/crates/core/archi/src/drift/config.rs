//! 架构域看护配置读取与校验：架构域根下的配置文件（经
//! [`foundation::layout::archi_config_path`] 定位——本 crate 零文件名字面量）。
//!
//! **归属面写死**：工作区配置（openspec 域根）管执行（命令 / scope / 阈值 /
//! 保护），本文件管看护策略（豁免 / 扫描排除）——MUST NOT 出现第三个配置文件，
//! MUST NOT 把 `architecture` 键塞进工作区配置。
//!
//! **缺失即空配置**：文件不存在视为 `{ ignore: [], exclude: [] }`（冷启动合法
//! 起点），MUST NOT 报错。

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

/// 架构域看护配置（两键封闭）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckConfig {
    /// 元素对豁免（第 2 层忽略规则；粒度封闭——只允许 `(from 元素, to 元素)`）
    pub ignore: Vec<IgnoreRule>,
    /// 文件级排除 glob（**追加**于七条默认约定之后，不替换）
    pub exclude: Vec<String>,
}

/// 元素对豁免条目（`from` / `to` 均为模型内元素 FQN；`reason` 必填非空白）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreRule {
    /// 引入方元素 FQN
    pub from: String,
    /// 被引入元素 FQN
    pub to: String,
    /// 豁免理由（必填非空白——「为什么这条依赖是对的」必须可审计）
    pub reason: String,
}

/// 线面形态（严格性经 `deny_unknown_fields` 承载：拼错键即 `Err`，MUST NOT 静默
/// 忽略而让用户以为豁免生效）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    ignore: Vec<RawIgnoreRule>,
    #[serde(default)]
    exclude: Vec<String>,
}

/// 线面豁免条目（`to` / `reason` 缺失可辨——分别是「元素对不完整」与「豁免必须
/// 带理由」两条错误文案）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIgnoreRule {
    from: String,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

/// 读取并校验看护配置：文件缺失 → 空配置；未知键 / `reason` 缺失或空白 /
/// `from` / `to` 不在模型内 / 非法 `exclude` → `Err`（文案指明条目）。
///
/// `known` 为模型内元素 FQN 集合（配置是声明，指向已删除元素即配置漂移 → 显式
/// `Err`）。
pub fn load_config(root: &Path, known: &BTreeSet<String>) -> Result<CheckConfig, String> {
    let path = foundation::layout::archi_config_path(root);
    if !path.is_file() {
        return Ok(CheckConfig::default());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("架构域配置读取失败（{}）：{error}", path.display()))?;
    let raw: RawConfig = serde_json::from_str(&text)
        .map_err(|error| format!("架构域配置解析失败（{}）：{error}", path.display()))?;

    let mut ignore: Vec<IgnoreRule> = Vec::new();
    for (index, entry) in raw.ignore.into_iter().enumerate() {
        let position = index + 1;
        let to = entry.to.ok_or_else(|| {
            format!(
                "架构域配置 ignore 第 {position} 条缺 `to`：豁免只允许 (元素, 元素) 元素对，\
                 MUST NOT 支持 glob 或单边形态"
            )
        })?;
        let reason = entry.reason.unwrap_or_default();
        if reason.trim().is_empty() {
            return Err(format!(
                "架构域配置 ignore 第 {position} 条（{} → {}）缺 `reason`：豁免必须带理由",
                entry.from, to
            ));
        }
        if !known.contains(&entry.from) {
            return Err(format!(
                "架构域配置 ignore 第 {position} 条的 `from` 不是模型内元素 FQN：\"{}\"\
                 （只允许 (元素, 元素) 元素对，`src/legacy/**` 一类 glob 形态被拒）",
                entry.from
            ));
        }
        if !known.contains(&to) {
            return Err(format!(
                "架构域配置 ignore 第 {position} 条的 `to` 不是模型内元素 FQN：\"{to}\"\
                 （只允许 (元素, 元素) 元素对，`src/legacy/**` 一类 glob 形态被拒）"
            ));
        }
        ignore.push(IgnoreRule {
            from: entry.from,
            to,
            reason: reason.trim().to_owned(),
        });
    }

    for entry in &raw.exclude {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return Err("架构域配置 exclude 含空白条目（文件级 glob 不得为空）".to_owned());
        }
        if !trimmed.contains('*') && !trimmed.contains('?') && !trimmed.contains('[') {
            return Err(format!(
                "架构域配置 exclude 条目不是文件级 glob：\"{trimmed}\"（目录请写 \
                 \"{trimmed}/**\" 形态）"
            ));
        }
    }

    Ok(CheckConfig {
        ignore,
        exclude: raw.exclude,
    })
}
