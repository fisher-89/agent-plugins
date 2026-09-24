//! explore 笔记读面：单文件读取与导入扫描（纯读，无任何写入路径）。
//!
//! 探索笔记是扁平 markdown 文件集合（无 active / archive 两棵树），路径一律
//! 经 [`foundation::layout::Layout::explores_root`] 派生，本模块零目录名知识。
//! 查询层纪律与 change 域一致：目录缺失返回空结果而非报错；文件由 agent 会话
//! 流程创建，本模块 MUST NOT 写入、移动或修改任何文件。

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use foundation::layout::Layout;

use super::is_single_component_name;

/// 单篇 explore 笔记内容（纯文本 DTO，命令层直出，不套 change 域产物信封）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExploreDoc {
    /// 笔记名（= 文件 stem）
    pub name: String,
    /// UTF-8 文本内容
    pub content: String,
}

/// 导入扫描条目：stem 名称 + 可得的修改时间。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExploreScanEntry {
    /// 笔记名（= 文件 stem）
    pub name: String,
    /// 修改时间（UTC unix 毫秒）；metadata 不可得时为 `None`
    pub modified_at: Option<i64>,
}

/// 读取单篇 explore 笔记：`explores_root/<name>.md` 的 UTF-8 文本。
///
/// `name` 必须为单个普通文件名 stem（校验口径与 `locate_change` 一致，拒绝
/// 路径穿越，不触碰笔记目录之外的任何路径）；文件缺失、不可读或非 UTF-8
/// 返回 `None` 而非报错。纯读。
pub fn read_explore(layout: &Layout, name: &str) -> Option<ExploreDoc> {
    if !is_single_component_name(name) {
        return None;
    }
    let content = fs::read_to_string(layout.explores_root.join(format!("{name}.md"))).ok()?;
    Some(ExploreDoc {
        name: name.to_owned(),
        content,
    })
}

/// 导入扫描：列出笔记目录下全部顶层 `*.md` 文件，stem 升序，并列按修改时间
/// 升序（同 stem 不可能并列，此为顺序确定的兜底键）。目录缺失返回空结果。
/// 只列目录、不做绑定过滤——绑定状态属 store 域，由命令层以清单求差完成。
pub fn scan_explores(layout: &Layout) -> Vec<ExploreScanEntry> {
    let Ok(entries) = fs::read_dir(&layout.explores_root) else {
        return Vec::new();
    };
    let mut scanned: Vec<ExploreScanEntry> = entries
        .flatten()
        .filter(|entry| {
            entry.file_type().map(|t| t.is_file()).unwrap_or(false)
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .map(|entry| ExploreScanEntry {
            name: stem_of(entry.path().as_path()),
            modified_at: modified_millis(entry.path().as_path()),
        })
        .filter(|entry| !entry.name.is_empty())
        .collect();
    scanned.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.modified_at.cmp(&b.modified_at))
    });
    scanned
}

/// 文件 stem（`api-design.md` → `api-design`）；无文件名段时回退空串由调用方滤除。
fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// 修改时间（UTC unix 毫秒）；metadata 不可得或时钟早于 epoch 时为 `None` / 0 兜底。
fn modified_millis(path: &Path) -> Option<i64> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let millis = modified.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64;
    Some(millis)
}
