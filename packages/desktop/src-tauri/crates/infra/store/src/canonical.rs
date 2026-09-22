//! canonical 口径（单一规范函数）：存库 key、去重比较、touch/remove 命中同源。

use std::path::{Path, PathBuf};

/// 主口径：`dunce::canonicalize`。
///
/// Windows 上安全时去掉 `\\?\` UNC 前缀（存库与展示均不带前缀），且返回盘上
/// 真实大小写——大小写不敏感去重免费获得；超长路径（>260 字符）保留前缀，
/// 仍为唯一 canonical 形态（已知退化，可接受）。失败即 Err（add 场景目录
/// 刚由选择器选定，正常不触发）。
pub(crate) fn canonical_key(input: &Path) -> Result<PathBuf, String> {
    dunce::canonicalize(input).map_err(|e| format!("{}: {e}", input.display()))
}

/// 回退匹配：目录已消失、canonicalize 必然失败时，用户仍须能移除残留清单项。
///
/// 入参词法归一化——分隔符统一 `\`、去 `\\?\` 前缀——与存量 key 逐个
/// ASCII 大小写不敏感比较。前端 touch/remove 回传的是 list 结果中的
/// canonical root，回退匹配必然命中；仍未命中由调用方按幂等 miss 处理。
pub(crate) fn matches_lexically(stored_key: &str, input: &Path) -> bool {
    let raw = input.to_string_lossy();
    let stripped = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let normalized = stripped.replace('/', "\\");
    normalized.eq_ignore_ascii_case(stored_key)
}
