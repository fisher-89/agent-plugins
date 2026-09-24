//! explore 命令轨道（查询 + 显式写）：七条命令——三读（单文件读取 / 导入
//! 扫描 / 文档路径派生）+ 四记录面（清单 / 建档 / 改名 / 删除）。
//!
//! 三件事纪律沿 `commands/queries` 模板：参数转换 → 调用（workflow 查询或
//! store 操作）→ 错误映射；blank root 空结果纪律同口径（空/空白 root 不进入
//! 查询与 store 链路，直接空结果语义，无 panic 无错误弹窗）。查询侧纯读零
//! 写入（笔记文件由 agent 会话流程创建，应用无落盘路径）；记录侧只写 DB
//! （记录是身份，磁盘文件是可丢弃投影，删除记录不动文件）。
//!
//! 绑定过滤在命令层：workflow 扫描只列目录不认识 store，「未绑定」以 explore
//! 记录清单求差滤除（已绑定 stem 不再出现在导入清单）。
//! 能力 spec：`specs/desktop-explore-queries/spec.md`、
//! `specs/desktop-explore-page/spec.md`（路径相对域根）。

use std::path::Path;

#[cfg(test)]
mod mod_test;

use tauri::State;

use foundation::layout::resolve;
use store::{ExploreRecord, Store};
use workflow::queries::{self, ExploreDoc, ExploreScanEntry};

/// root 显式格式检查：空/空白串不进入查询链路（同 `commands::queries` 口径）。
fn is_blank_root(root: &str) -> bool {
    root.trim().is_empty()
}

/// 记录名单分量校验（非空、非 `.` / `..`、不含 `/` `\` `:`）：口径同
/// workflow 查询层 `is_single_component_name`（其为本 crate 内单点，壳层
/// 复制口径防穿越）。
fn is_single_component_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
}

/// 读取单篇笔记全文；未知 stem、穿越名或文件缺失返回 `None`（不报错）。
#[tauri::command]
pub fn read_explore(root: String, name: String) -> Option<ExploreDoc> {
    if is_blank_root(&root) {
        return None;
    }
    let layout = resolve(Path::new(&root));
    queries::read_explore(&layout, &name)
}

/// 导入扫描：列出笔记目录顶层 `*.md` 中**未被绑定**的 stem（绑定过滤在
/// 命令层以 store 清单求差）；blank root → 空结果。
#[tauri::command]
pub fn scan_explores(
    root: String,
    store: State<'_, Store>,
) -> Result<Vec<ExploreScanEntry>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    let layout = resolve(Path::new(&root));
    let bound: Vec<String> = store
        .list_explore_records(&root)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|record| record.name)
        .collect();
    Ok(queries::scan_explores(&layout)
        .into_iter()
        .filter(|entry| !bound.iter().any(|name| name == &entry.name))
        .collect())
}

/// 布局派生当前笔记的完整磁盘路径（无 IO；stem 校验同读取口径）：供前端
/// watch 订阅取路径，目录名知识不下沉前端。
#[tauri::command]
pub fn explore_doc_path(root: String, name: String) -> Option<String> {
    if is_blank_root(&root) || !is_single_component_name(&name) {
        return None;
    }
    let layout = resolve(Path::new(&root));
    Some(
        layout
            .explores_root
            .join(format!("{name}.md"))
            .to_string_lossy()
            .into_owned(),
    )
}

/// explore 记录清单（按当前 workspace root 过滤，id 升序）；blank root → 空结果。
#[tauri::command]
pub fn list_explore_records(
    store: State<'_, Store>,
    root: String,
) -> Result<Vec<ExploreRecord>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    store.list_explore_records(&root).map_err(|e| e.to_string())
}

/// 新建 explore 记录（导入绑定 / 新话题建档共用）：只写 DB，不触磁盘文件。
#[tauri::command]
pub fn create_explore_record(
    store: State<'_, Store>,
    root: String,
    name: String,
) -> Result<ExploreRecord, String> {
    store
        .create_explore_record(&root, &name)
        .map_err(|e| e.to_string())
}

/// in-place 改名（保主键 → 保会话链绑定）：文件改名后的记录重关联入口。
#[tauri::command]
pub fn rename_explore_record(
    store: State<'_, Store>,
    root: String,
    name: String,
    new_name: String,
) -> Result<ExploreRecord, String> {
    store
        .rename_explore_record(&root, &name, &new_name)
        .map_err(|e| e.to_string())
}

/// 删除 explore 记录（不动磁盘文件）；miss 幂等返回 `false`。
#[tauri::command]
pub fn delete_explore_record(
    store: State<'_, Store>,
    root: String,
    name: String,
) -> Result<bool, String> {
    store
        .delete_explore_record(&root, &name)
        .map_err(|e| e.to_string())
}
