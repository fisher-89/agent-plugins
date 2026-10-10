//! change 域命令组（读 + 记录面）：五条命令——三读（change 列表 / 详情聚合
//! 统一视图 / 产物信封读取）+ 二记录面（新建建域 / 归档双写）；change 域三
//! 分职责——本组（读 + 记录面，沿 explores 组同组先例）/ `change_flow`（run
//! 编排控制）/ `archive_flow`（归档编排流——带 agent 会话的第三面；本组裸
//! 双写 `archive_change` 保留零改动，链式归档入口归该组）。
//!
//! 读命令为薄包装——参数 → resolve → `for_root` 取 workspace 库实例 → core
//! 函数 → DTO：无直接文件系统访问、不缓存 workspace 状态（记录面带 State，
//! 沿 explores 组先例）。`get_change_detail` 为**统一视图合并装配**（读时合
//! 并零写路径，unify-run-state-persistence D11）：core `change_detail` 库读
//! 史后并入 `ChangeFlowControl` 注册表快照活面（steps / ask / 停等态）一面
//! 出；blank root / 开库失败 None 语义不变。记录面三件事纪律：参数转换 →
//! 调写面 → 错误映射；name / goal 校验权威在写面，命令层不过关。
//! **寻址面 id 化**：change 域三读一写的定位参数均为 change id（一切寻址以
//! id 为准）；磁盘面 name / worktree 由 core 经 id → 记录分辨率单点供给。
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
//! - `id`：change 身份锚（未建档 → 详情 `None` / 产物 `None`；写面显式
//!   `Err`）——未知 id 零磁盘目录解析回退；
//! - `source`：change 内相对 POSIX 路径或评估条目序号串（拒绝 `..`/绝对路径/
//!   反斜杠/盘符），由 core `read_artifact` 强制，另含 canonical 包含性兜底；
//! - `kind`：非空，且仅与静态注册表精确比对，由 core `read_artifact` 强制。
//!
//! 能力 spec：`specs/desktop-app-shell/spec.md`、
//! `specs/desktop-change-create/spec.md`、
//! `specs/desktop-change-state-store/spec.md`（路径相对域根）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use orchestration::control::ChangeFlowControl;
use tauri::{AppHandle, Manager, State};

use foundation::layout::resolve;
use store::WorkspaceStores;
use vcs_runtime::{worktree_dir, ProcessWorktree};
use workflow::artifacts::{read_artifact as core_read_artifact, ArtifactEnvelope};
use workflow::queries::{self, iso_from_millis, locate_change, ChangeDetail, ChangeList};
use workflow::write::{self, ArchiveOutcome, CreateOutcome};

/// 统一视图活面投影（unify-run-state-persistence D11）：注册表快照的线面像
/// ——`startedAt` ISO 化收本命令层单点（快照毫秒 → ISO 串），steps 全词汇
/// emit 序透传。
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRunView {
    pub run_id: String,
    pub status: orchestration::ChangeRunStatus,
    pub phase: Option<String>,
    pub attempt: Option<u32>,
    pub ask: Option<orchestration::AskPayload>,
    pub started_at: String,
    pub steps: Vec<orchestration::ChangeStepState>,
}

/// 统一视图信封：库读史 ∪ 在飞 run 活面一次返回（前端零双命令拼接；
/// `active_run` 终态即除名 → null 语义不变）。
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDetailUnified {
    pub detail: ChangeDetail,
    pub active_run: Option<ActiveRunView>,
}

#[cfg(test)]
mod mod_test;

/// root 显式格式检查：空/空白串不进入查询链路，直接给出空结果语义。
fn is_blank_root(root: &str) -> bool {
    root.trim().is_empty()
}

/// change 列表（db 单源全量；active + archive 按月分组）。IPC 签名不变
/// （Result 面不引入）：blank root 与开库失败均给出空列表（读命令空结果
/// 语义，不 panic）；零磁盘扫描触点（无 db 记录的存量 CLI change 零发现）。
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
    queries::list_changes(store.as_ref())
}

/// 单 change 详情聚合（统一视图，unify-run-state-persistence D11；按 change
/// **id** 寻址）：一次返回「库读史（detail.runs）∪ 在飞 run 活面
/// （activeRun）」——前端零双命令拼接。未知 id 返回 `None`（未建档——零磁盘
/// 目录解析回退、零文档形态空面）。IPC 签名不变：blank root 与开库失败均
/// `None`。worktree / name 感知在 core `change_detail` 内（record 先读后定位，
/// 磁盘面恒自记录供给）。
#[tauri::command]
#[specta::specta]
pub fn get_change_detail(
    stores: State<'_, WorkspaceStores>,
    control: State<'_, Arc<ChangeFlowControl>>,
    root: String,
    id: String,
) -> Option<ChangeDetailUnified> {
    if is_blank_root(&root) {
        return None;
    }
    let Ok(store) = stores.for_root(&root) else {
        return None;
    };
    let layout = resolve(Path::new(&root));
    let detail = queries::change_detail(&layout, store.as_ref(), &id)?;
    // 活面投影：注册表快照（终态即除名 → None 语义不变；键 id 化同式）；
    // startedAt ISO 化收命令层单点（快照毫秒 → 线面 ISO 串）
    let active_run = control.snapshot(&root, &id).map(|snapshot| ActiveRunView {
        run_id: snapshot.run_id,
        status: snapshot.status,
        phase: snapshot.phase,
        attempt: snapshot.attempt,
        ask: snapshot.ask,
        started_at: iso_from_millis(snapshot.started_at),
        steps: snapshot.steps,
    });
    Some(ChangeDetailUnified { detail, active_run })
}

/// 按信封读取单个产物（按 change **id** 寻址）；kind 未注册、source 非法、
/// 未建档或解析失败返回 `None`。IPC 签名不变：blank root 与开库失败均
/// `None`。worktree / name 感知：`find_change_record(id)` 供给记录，`worktree`
/// 直传 `locate_change` 回退参（merge 前产物在 worktree 内）、`name` 作目录
/// 定位键（id → 记录 → name 分辨率单点）。
#[tauri::command]
#[specta::specta]
pub fn read_artifact(
    stores: State<'_, WorkspaceStores>,
    root: String,
    id: String,
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
    let record = store.find_change_record(&id).ok().flatten()?;
    let location = locate_change(&layout, record.worktree.as_deref(), record.name.as_str())?;
    let phases = store.list_phase_records(&id).unwrap_or_default();
    core_read_artifact(&location.dir, &phases, &kind, &source)
}

/// 新建 change（建域四段：建档 + worktree add + worktree 内目录树与
/// explore.md + bootstrap）；blank root 显式 `Err`。async + `spawn_blocking`
/// 调 sync 写面（bootstrap 是分钟级 spawn——同步命令会冻结 UI，IPC 入参与
/// 返回类型面不变）。手动新建路径 `title = name`（不加输入框，D1）；promote
/// 路径经 [`create_change_with`] 泛型缝显式传 `explore.title`。
#[tauri::command]
#[specta::specta]
pub async fn create_change(
    app: AppHandle,
    root: String,
    name: String,
    goal: String,
) -> Result<CreateOutcome, String> {
    let title = name.clone();
    create_change_with(app, root, name, goal, title).await
}

/// [`create_change`] 的泛型测试缝（生产注入 Wry 句柄、测试注入 MockRuntime
/// 句柄，沿 `archive_change_with` 先例）：装配 vcs 落位派生（data_root 状态）
/// + `ProcessWorktree`，经 `spawn_blocking` 调写面。`title` 显式传入（空白
/// 回退 `name` 单点在写面）——promote 路径据此继承 `explore.title`。
pub(crate) async fn create_change_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    name: String,
    goal: String,
    title: String,
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
            &title,
        )
    })
    .await
    .map_err(|e| format!("create 任务失败: {e}"))?
}

/// 归档 change（双写：目录改名 + db status 翻转，写面 `archive` 单点；按
/// change **id** 寻址）；blank root / id 显式 `Err`。IPC 薄命令（design D11：
/// 本轮无前端入口）。
#[tauri::command]
#[specta::specta]
pub fn archive_change(app: AppHandle, root: String, id: String) -> Result<ArchiveOutcome, String> {
    archive_change_with(app, root, id)
}

/// [`archive_change`] 的泛型测试缝（生产注入 Wry 句柄、测试注入 MockRuntime
/// 句柄，沿 `change_flow_start_with` 先例）。
pub(crate) fn archive_change_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    id: String,
) -> Result<ArchiveOutcome, String> {
    if root.trim().is_empty() {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if id.trim().is_empty() {
        return Err("非法 id: 不得为空白".to_owned());
    }
    let layout = resolve(Path::new(&root));
    let store = app
        .state::<WorkspaceStores>()
        .for_root(&root)
        .map_err(|e| e.to_string())?;
    write::archive(&layout, store.as_ref(), &id)
}
