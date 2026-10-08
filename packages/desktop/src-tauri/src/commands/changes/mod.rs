//! change 域命令组（读 + 记录面）：五条命令——三读（change 列表 / 详情
//! 聚合 / 产物信封读取）+ 二记录面（新建建域 / 归档双写）；change 域三分职
//! 责——本组（读 + 记录面，沿 explores 组同组先例）/ `change_flow`（run 编
//! 排控制）/ `archive_flow`（归档编排流——带 agent 会话的第三面；本组裸
//! 双写 `archive_change` 保留零改动，链式归档入口归该组）。
//!
//! 读命令为薄包装——参数 → resolve → `for_root` 取 workspace 库实例 → core
//! 函数 → DTO：无直接文件系统访问、不缓存 workspace 状态（记录面带 State，
//! 沿 explores 组先例）；blank root 早退空结果语义（空列表 / `None`），开库
//! 失败同口径（IPC 签名不变，Result 面不引入）。记录面三件事纪律：参数转
//! 换 → 调写面 → 错误映射；name / goal 校验权威在写面，命令层不过关。
//! worktree 维度：`create_change` 经 vcs 落位派生（data_root 状态注入）+
//! `ProcessWorktree` 装配，经 `spawn_blocking` 调 sync 写面（bootstrap 是
//! 分钟级 spawn，async 化使 UI 不冻结）；`get_change_detail` / `read_artifact`
//! 读 record.worktree 传 `locate_change` 回退参（merge 前主仓两树未命中仍
//! 可达）。
//!
//! 组内 blank root 双口径并存且 MUST NOT 互换：读命令空/空白 root 早退
//! 空结果语义（空列表 / `None`，见 [`is_blank_root`]）；`create_change` /
//! `archive_change` 显式 `Err`（写无空结果语义，不进入写面链路）。
//!
//! IPC 字符串入参的显式格式/包含性检查口径（读命令）：
//! - `root`：空/空白视作空 workspace（返回空结果而非报错），见 [`is_blank_root`]；
//! - `change`：单分量目录名（拒绝路径穿越），由 core `locate_change` /
//!   写面 `archive` 强制；
//! - `source`：change 内相对 POSIX 路径或评估条目序号串（拒绝 `..`/绝对路径/
//!   反斜杠/盘符），由 core `read_artifact` 强制，另含 canonical 包含性兜底；
//! - `kind`：非空，且仅与静态注册表精确比对，由 core `read_artifact` 强制。
//!
//! 能力 spec：`specs/desktop-app-shell/spec.md`、
//! `specs/desktop-change-create/spec.md`、
//! `specs/desktop-change-state-store/spec.md`（路径相对域根）。

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, State};

use foundation::layout::resolve;
use store::WorkspaceStores;
use vcs_runtime::{worktree_dir, ProcessWorktree};
use workflow::artifacts::{read_artifact as core_read_artifact, ArtifactEnvelope};
use workflow::queries::{self, locate_change, ChangeDetail, ChangeList};
use workflow::write::{self, ArchiveOutcome, CreateOutcome};

#[cfg(test)]
mod mod_test;

/// root 显式格式检查：空/空白串不进入查询链路，直接给出空结果语义。
fn is_blank_root(root: &str) -> bool {
    root.trim().is_empty()
}

/// change 列表（db 记录 ∪ 磁盘目录去重并集；active + archive 按月分组）。
/// IPC 签名不变（Result 面不引入）：blank root 与开库失败均给出空列表（与
/// 缺目录空结果同语义，不 panic）。
#[tauri::command]
#[specta::specta]
pub fn list_changes(stores: State<'_, WorkspaceStores>, root: String) -> ChangeList {
    let empty = || ChangeList {
        active: Vec::new(),
        archive_groups: Vec::new(),
    };
    if is_blank_root(&root) {
        return empty();
    }
    let Ok(store) = stores.for_root(&root) else {
        return empty(); // 开库失败：读命令空结果语义，不进入查询链路
    };
    let layout = resolve(Path::new(&root));
    queries::list_changes(&layout, store.as_ref())
}

/// 单 change 详情聚合；未知 change 名返回 `None`（db 缺记录 change 以文档
/// 形态返回：空流水线 + 产物清单）。IPC 签名不变：blank root 与开库失败均
/// `None`。worktree 感知在 core `change_detail` 内（record 先读后定位）。
#[tauri::command]
#[specta::specta]
pub fn get_change_detail(
    stores: State<'_, WorkspaceStores>,
    root: String,
    change: String,
) -> Option<ChangeDetail> {
    if is_blank_root(&root) {
        return None;
    }
    let Ok(store) = stores.for_root(&root) else {
        return None;
    };
    let layout = resolve(Path::new(&root));
    queries::change_detail(&layout, store.as_ref(), &change)
}

/// 按信封读取单个产物；kind 未注册、source 非法或解析失败返回 `None`。
/// IPC 签名不变：blank root 与开库失败均 `None`。worktree 感知：record 的
/// `worktree` 字段直传 `locate_change` 回退参（merge 前产物在 worktree 内）。
#[tauri::command]
#[specta::specta]
pub fn read_artifact(
    stores: State<'_, WorkspaceStores>,
    root: String,
    change: String,
    kind: String,
    source: String,
) -> Option<ArtifactEnvelope> {
    if is_blank_root(&root) {
        return None;
    }
    let Ok(store) = stores.for_root(&root) else {
        return None;
    };
    let layout = resolve(Path::new(&root));
    let worktree = store
        .find_change_record(&change)
        .ok()
        .flatten()
        .and_then(|record| record.worktree);
    let location = locate_change(&layout, worktree.as_deref(), &change)?;
    let phases = store.list_phase_records(&change).unwrap_or_default();
    core_read_artifact(&location.dir, &phases, &kind, &source)
}

/// 新建 change（建域四段：建档 + worktree add + worktree 内目录树与
/// explore.md + bootstrap）；blank root 显式 `Err`。async + `spawn_blocking`
/// 调 sync 写面（bootstrap 是分钟级 spawn——同步命令会冻结 UI，IPC 入参与
/// 返回类型面不变）。
#[tauri::command]
#[specta::specta]
pub async fn create_change(
    app: AppHandle,
    root: String,
    name: String,
    goal: String,
) -> Result<CreateOutcome, String> {
    create_change_with(app, root, name, goal).await
}

/// [`create_change`] 的泛型测试缝（生产注入 Wry 句柄、测试注入 MockRuntime
/// 句柄，沿 `archive_change_with` 先例）：装配 vcs 落位派生（data_root 状态）
/// + `ProcessWorktree`，经 `spawn_blocking` 调写面。
pub(crate) async fn create_change_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    name: String,
    goal: String,
) -> Result<CreateOutcome, String> {
    if root.trim().is_empty() {
        return Err("非法 root: 不得为空白".to_owned());
    }
    let stores = app.state::<WorkspaceStores>();
    let data_root = app.state::<PathBuf>();
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    // worktree 落位经 vcs 单点派生（data_root/worktrees/{身份段}/<name>）；
    // 写面 create 以父锚自拼 <name>，此处取其父目录
    let placement = worktree_dir(data_root.inner(), &root, &name);
    let Some(worktree_root) = placement.parent() else {
        return Err(format!(
            "worktree 落位派生异常（无父目录）: {}",
            placement.display()
        ));
    };
    let main_root = PathBuf::from(root);
    let worktree_root = worktree_root.to_path_buf();
    let vcs = ProcessWorktree::new();
    tauri::async_runtime::spawn_blocking(move || {
        write::create(
            &main_root,
            &worktree_root,
            store.as_ref(),
            &vcs,
            &name,
            &goal,
        )
    })
    .await
    .map_err(|e| format!("create 任务失败: {e}"))?
}

/// 归档 change（双写：目录改名 + db status 翻转，写面 `archive` 单点）；
/// blank root / change 显式 `Err`。IPC 薄命令（design D11：本轮无前端入口）。
#[tauri::command]
#[specta::specta]
pub fn archive_change(
    app: AppHandle,
    root: String,
    change: String,
) -> Result<ArchiveOutcome, String> {
    archive_change_with(app, root, change)
}

/// [`archive_change`] 的泛型测试缝（生产注入 Wry 句柄、测试注入 MockRuntime
/// 句柄，沿 `change_flow_start_with` 先例）。
pub(crate) fn archive_change_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    change: String,
) -> Result<ArchiveOutcome, String> {
    if root.trim().is_empty() {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if change.trim().is_empty() {
        return Err("非法 change: 不得为空白".to_owned());
    }
    let layout = resolve(Path::new(&root));
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .map_err(|e| e.to_string())?;
    write::archive(&layout, store.as_ref(), &change)
}
