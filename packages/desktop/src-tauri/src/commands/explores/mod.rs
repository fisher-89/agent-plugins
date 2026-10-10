//! explore 命令轨道（查询 + 显式写）：九条命令——三读（单文件读取 / 导入
//! 扫描 / 文档路径派生）+ 六记录面（清单 / 建档 / 改名 / 删除 / 标题回填 /
//! promote）。
//!
//! 三件事纪律沿 `commands/changes` 模板：参数转换 → 调用（workflow 查询或
//! store 操作）→ 错误映射；blank root 纪律同口径（空/空白 root 不进入查询
//! 与 store 链路：查询直接空结果语义、写命令 `Err`——写无空结果语义，无
//! panic 无错误弹窗）。记录面经 `for_root(&root)` 路由至**所属 workspace 库**
//! （记录 / 名下会话链同库收敛，见 desktop-workspace-store 双库布局）。
//! 查询侧纯读零写入（笔记文件由 agent 会话流程创建，应用无落盘路径）；记录
//! 侧只写 DB（记录是身份，磁盘文件是可丢弃投影，删除记录不动文件，记录名下
//! 会话 runs+events 随记录在 store 层同事务级联删除）。
//!
//! 绑定过滤在命令层：workflow 扫描只列目录不认识 store，「未绑定」以 explore
//! 记录清单求差滤除（已绑定 stem 不再出现在导入清单），并再滤除非 kebab-case
//! stem（store 建档已拒非 kebab，过滤避免「点击后报错」；workflow 扫描口径保持
//! 宽松、旧非 kebab 文件仍可经 `read_explore` 读取）。
//!
//! `promote_explore` 为跨域写命令（move 语义）：读笔记全文 → 读记录 → 经写面
//! `workflow::write::promote_explore` 建 change（复用 create，worktree 内
//! explore.md = 笔记全文）并删主仓笔记 → store 打标 `promoted_to`（会话链保留、
//! MUST NOT 删记录）。命令层 MUST NOT 直接 `remove_file`——磁盘笔记删除只在
//! 写面发生。
//! 能力 spec：`specs/desktop-explore-queries/spec.md`、
//! `specs/desktop-explore-page/spec.md`（路径相对域根）。

use std::path::{Path, PathBuf};

#[cfg(test)]
mod mod_test;

use tauri::{AppHandle, Manager, State};

use foundation::layout::resolve;
use store::{ExploreRecord, WorkspaceStores};
use vcs_runtime::{worktree_dir, ProcessWorktree};
use workflow::queries::{self, ExploreDoc, ExploreScanEntry};
use workflow::write::{self, PromoteOutcome};

/// root 显式格式检查：空/空白串不进入查询链路（同 `commands::changes` 口径）。
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

/// explore 名称 kebab-case 判定（等价 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`，不引
/// regex）：与 store 建档校验同口径（promote 免转换冲突的前提——change 名
/// 恒等于 explore 名）。
fn is_kebab_case(name: &str) -> bool {
    let segment_ok = |first: bool, segment: &str| {
        let mut chars = segment.chars();
        match chars.next() {
            // 首段必须字母开头；后续段可数字开头
            Some(c) if c.is_ascii_lowercase() || (!first && c.is_ascii_digit()) => {}
            _ => return false,
        }
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    };
    let mut segments = name.split('-');
    match segments.next() {
        Some(first) => {
            if !segment_ok(true, first) {
                return false;
            }
        }
        // split 至少返回一段，防御式拒绝
        None => return false,
    }
    segments.all(|segment| segment_ok(false, segment))
}

/// 读取单篇笔记全文；未知 stem、穿越名或文件缺失返回 `None`（不报错）。
#[tauri::command]
#[specta::specta]
pub fn read_explore(root: String, name: String) -> Option<ExploreDoc> {
    if is_blank_root(&root) {
        return None;
    }
    let layout = resolve(Path::new(&root));
    queries::read_explore(&layout, &name)
}

/// 导入扫描：列出笔记目录顶层 `*.md` 中**未被绑定且 stem 为 kebab-case** 的
/// stem（绑定过滤与 kebab 过滤均在命令层：前者以 store 清单求差，后者对齐
/// store 建档口径，避免「点击后报错」）；blank root → 空结果。
#[tauri::command]
#[specta::specta]
pub fn scan_explores(
    root: String,
    stores: State<'_, WorkspaceStores>,
) -> Result<Vec<ExploreScanEntry>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    let layout = resolve(Path::new(&root));
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let bound: Vec<String> = store
        .list_explore_records(&root)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|record| record.name)
        .collect();
    Ok(queries::scan_explores(&layout)
        .into_iter()
        .filter(|entry| !bound.iter().any(|name| name == &entry.name))
        .filter(|entry| is_kebab_case(&entry.name))
        .collect())
}

/// 布局派生当前笔记的完整磁盘路径（无 IO；stem 校验同读取口径）：供前端
/// watch 订阅取路径，目录名知识不下沉前端。
#[tauri::command]
#[specta::specta]
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
#[specta::specta]
pub fn list_explore_records(
    stores: State<'_, WorkspaceStores>,
    root: String,
) -> Result<Vec<ExploreRecord>, String> {
    if is_blank_root(&root) {
        return Ok(Vec::new());
    }
    stores
        .for_root(&root)
        .map_err(|e| e.to_string())?
        .list_explore_records(&root)
        .map_err(|e| e.to_string())
}

/// 新建 explore 记录（导入绑定 / 新话题建档共用）：只写 DB，不触磁盘文件；
/// blank root → `Err`（写无空结果语义）。
#[tauri::command]
#[specta::specta]
pub fn create_explore_record(
    stores: State<'_, WorkspaceStores>,
    root: String,
    name: String,
) -> Result<ExploreRecord, String> {
    if is_blank_root(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    stores
        .for_root(&root)
        .map_err(|e| e.to_string())?
        .create_explore_record(&root, &name)
        .map_err(|e| e.to_string())
}

/// in-place 改名（保主键 → 保会话链绑定）：文件改名后的记录重关联入口；
/// blank root → `Err`。
#[tauri::command]
#[specta::specta]
pub fn rename_explore_record(
    stores: State<'_, WorkspaceStores>,
    root: String,
    name: String,
    new_name: String,
) -> Result<ExploreRecord, String> {
    if is_blank_root(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    stores
        .for_root(&root)
        .map_err(|e| e.to_string())?
        .rename_explore_record(&root, &name, &new_name)
        .map_err(|e| e.to_string())
}

/// 删除 explore 记录（不动磁盘文件；名下会话 runs+events 随记录同事务级联
/// 删除，级联语义见 store `delete_explore_record`）；miss 幂等返回 `false`；
/// blank root → `Err`。
#[tauri::command]
#[specta::specta]
pub fn delete_explore_record(
    stores: State<'_, WorkspaceStores>,
    root: String,
    name: String,
) -> Result<bool, String> {
    if is_blank_root(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    stores
        .for_root(&root)
        .map_err(|e| e.to_string())?
        .delete_explore_record(&root, &name)
        .map_err(|e| e.to_string())
}

/// 回填 explore 标题（title 回填唯一写入口——读路径 `read_explore` 纯读零
/// 写入）：blank root / 空白 title 显式 `Err`；经 `for_root` 路由所属
/// workspace 库并调 store `set_explore_title`（in-place 写 + 刷新 `updated_at`），
/// 返回更新后的记录。
#[tauri::command]
#[specta::specta]
pub fn update_explore_title(
    stores: State<'_, WorkspaceStores>,
    root: String,
    name: String,
    title: String,
) -> Result<ExploreRecord, String> {
    if is_blank_root(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if title.trim().is_empty() {
        return Err("非法 title: 不得为空白".to_owned());
    }
    stores
        .for_root(&root)
        .map_err(|e| e.to_string())?
        .set_explore_title(&root, &name, &title)
        .map_err(|e| e.to_string())
}

/// 启动变更（promote，move 语义）：blank root / 非 kebab name 显式 `Err`。
/// 步序见 [`promote_explore_with`]；命令层 MUST NOT 直接磁盘删除。
#[tauri::command]
#[specta::specta]
pub async fn promote_explore(
    app: AppHandle,
    root: String,
    name: String,
) -> Result<PromoteOutcome, String> {
    promote_explore_with(app, root, name).await
}

/// [`promote_explore`] 的泛型测试缝（生产注入 Wry 句柄、测试注入 MockRuntime
/// 句柄）：① 读主仓笔记全文（未落盘 / 空白 → 显式 `Err` 引导先完成探索）；
/// ② 读 `ExploreRecord`（不存在 / 已 promoted → 显式 `Err`）；③ 经 vcs 落位
/// 派生父锚后 `spawn_blocking` 调写面 `workflow::write::promote_explore`
///（复用 create 建 change + 删主仓笔记——move 半边）；④ 成功后 store
/// `mark_explore_promoted` 打标（失败 → `Err` 携 change id 与手动恢复指引；
/// 会话链保留、MUST NOT 删记录）。
pub(crate) async fn promote_explore_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    name: String,
) -> Result<PromoteOutcome, String> {
    if is_blank_root(&root) {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if !is_kebab_case(&name) {
        return Err(format!(
            "name 必须为 kebab-case（小写字母/数字，可用 `-` 连接），收到: {name:?}"
        ));
    }
    let stores = app.state::<WorkspaceStores>();
    let data_root = app.state::<PathBuf>();
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;

    // ① 主仓笔记全文（命中且非空白）
    let layout = resolve(Path::new(&root));
    let note = queries::read_explore(&layout, &name)
        .filter(|doc| !doc.content.trim().is_empty())
        .ok_or_else(|| {
            format!("探索笔记 {name:?} 尚未落盘或内容为空白：请先在会话中完成探索再启动变更")
        })?;
    // ② 探索记录（存在且未 promote）
    let record = store
        .find_explore_record(&root, &name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("探索记录不存在: {name:?}"))?;
    if let Some(change_id) = record.promoted_to.as_deref() {
        return Err(format!(
            "探索 {name:?} 已转变更（change id={change_id}），不可重复启动变更"
        ));
    }

    // ③ 写面：复用 create 建 change（goal = 笔记全文、title = explore.title）
    // 成功后删主仓笔记（move 半边）
    let placement = worktree_dir(data_root.inner(), &root, &name);
    let Some(worktree_root) = placement.parent() else {
        return Err(format!(
            "worktree 落位派生异常（无父目录）: {}",
            placement.display()
        ));
    };
    let main_root = PathBuf::from(root.clone());
    let worktree_root = worktree_root.to_path_buf();
    let vcs = ProcessWorktree::new();
    let promote_name = name.clone();
    let note_text = note.content;
    let title = record.title;
    let store_for_promote = store.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        write::promote_explore(
            &main_root,
            &worktree_root,
            store_for_promote.as_ref(),
            &vcs,
            &promote_name,
            &note_text,
            &title,
        )
    })
    .await
    .map_err(|e| format!("promote 任务失败: {e}"))??;

    // ④ 打标（记录保留，仅补 promoted_to；失败显式呈现残留对象与恢复指引）
    store
        .mark_explore_promoted(&root, &name, &outcome.change_id)
        .map_err(|e| {
            format!(
                "探索笔记已搬入 change \"{}\"（id={}），但探索记录打标失败: {e}；\
                 请手动核对记录 promoted_to 是否为该 change id",
                outcome.change_name, outcome.change_id
            )
        })?;
    Ok(outcome)
}
