use std::path::{Path, PathBuf};
use std::sync::Arc;

use orchestration::control::ChangeFlowControl;
use tauri::{AppHandle, Manager, State};

use foundation::layout::resolve;
use store::WorkspaceStores;
use vcs_runtime::ProcessWorktree;
use workflow::artifacts::{read_artifact as core_read_artifact, ArtifactEnvelope};
use workflow::queries::{
    self, iso_from_millis, locate_change, ChangeDetail, ChangeList, ChangeRunEdgeRecord,
};
use workflow::state::ChangeStatus;
use workflow::write::{self, ArchiveOutcome, CreateOutcome, DeleteOutcome};

/// 统一视图活面投影
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRunView {
    pub run_id: String,
    pub status: orchestration::ChangeRunStatus,
    pub phase: Option<String>,
    pub attempt: Option<u32>,
    pub ask: Option<orchestration::AskPayload>,
    /// 归档前停等点的前置只读载荷（非该停等点恒 null；停等面数据面）
    pub archive: Option<orchestration::ArchivePreflight>,
    pub started_at: String,
    pub nodes: Vec<orchestration::ChangeStepState>,
    pub edges: Vec<ChangeRunEdgeRecord>,
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
/// **id** 寻址）：一次返回「库读史（`detail.runs[].nodes` / `runs[].edges`）∪
/// 在飞 run 活面（activeRun.nodes / activeRun.edges）」——前端零双命令拼接。
/// IPC Result 面：`Ok(None)` = 真「未找到」（未知 id——零磁盘目录解析回退、
/// 零文档形态空面）与 blank root / 开库失败（空结果语义维持）；`Err` = 读失
/// 败（store 错误串经 reject 通道出线——TS 类型面零漂移）。worktree / name
/// 感知在 core `change_detail` 内（record 先读后定位，磁盘面恒自记录供给）。
#[tauri::command]
#[specta::specta]
pub fn get_change_detail(
    stores: State<'_, WorkspaceStores>,
    control: State<'_, Arc<ChangeFlowControl>>,
    root: String,
    id: String,
) -> Result<Option<ChangeDetailUnified>, String> {
    if is_blank_root(&root) {
        return Ok(None);
    }
    let Ok(store) = stores.for_root(&root) else {
        return Ok(None); // 开库失败：空结果语义维持，不进入查询链路
    };
    let layout = resolve(Path::new(&root));
    // StoreFault → 错误串映射收命令层单点（D4：kind 前缀已在 Display 单点）；
    // None（未知 id，真「未找到」）→ 空结果语义
    let Some(detail) =
        queries::change_detail(&layout, store.as_ref(), &id).map_err(|fault| fault.to_string())?
    else {
        return Ok(None);
    };
    // 活面投影：注册表快照（终态即除名 → None 语义不变；键 id 化同式）；
    // startedAt ISO 化收命令层单点（快照毫秒 → 线面 ISO 串）
    let active_run = control.snapshot(&root, &id).map(|snapshot| ActiveRunView {
        run_id: snapshot.run_id,
        status: snapshot.status,
        phase: snapshot.phase,
        attempt: snapshot.attempt,
        ask: snapshot.ask,
        archive: snapshot.archive,
        started_at: iso_from_millis(snapshot.started_at),
        nodes: snapshot.nodes,
        // 活面边投影：端点身份串与 kind 自快照边表直取（身份串组装单点驻
        // orchestration::state::node_id，与库读史边表同式）
        edges: snapshot
            .edges
            .iter()
            .map(|edge| ChangeRunEdgeRecord {
                from_node_key: edge.from_node_key(),
                to_node_key: edge.to_node_key(),
                edge_kind: edge.kind,
            })
            .collect(),
    });
    Ok(Some(ChangeDetailUnified { detail, active_run }))
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

/// 运行中增量读的出线载荷（`exec::ExecOutputDelta` 的 1:1 映射）。
///
/// `u64` 经 workspace 既有 bigint 配置（`dangerously_cast_bigints_to_number`）
/// 出线为 TS `number`，与 `tokenInput` / `tokenOutput` 同口径——不引入 `BigInt`
/// 面。落位壳层（MUST NOT 定义在 `crates/infra/exec`：exec 的依赖面不得因出线
/// 需要而扩 serde / specta）。
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecOutputChunk {
    /// 自 `from_offset` 起的新增文本（UTF-8 字符边界收口）
    pub text: String,
    /// 下次调用应传的游标（唯一产出方在后端）
    pub next_offset: u64,
}

/// name → 记录 → worktree 候选供给序（四条 exec 读命令共享的定位单点）。
///
/// 序：非 archived 同名记录优先（归档同名共存合法 D11——active 的 worktree 先
/// 探），archived 记录的 stale worktree 次之，终回 `None`（主仓 active / archive
/// 两态）；按 `locate_change` 过滤为实际在场目录。
fn change_dirs(stores: &WorkspaceStores, root: &str, change_name: &str) -> Vec<std::path::PathBuf> {
    let Ok(store) = stores.for_root(root) else {
        return Vec::new();
    };
    let Ok(records) = store.list_change_records() else {
        return Vec::new();
    };
    let mut candidates: Vec<Option<String>> = Vec::new();
    for record in records
        .iter()
        .filter(|record| record.name == change_name && record.status != ChangeStatus::Archived)
    {
        push_worktree_candidate(&mut candidates, record.worktree.clone());
    }
    for record in records.iter().filter(|record| record.name == change_name) {
        push_worktree_candidate(&mut candidates, record.worktree.clone());
    }
    push_worktree_candidate(&mut candidates, None);
    let layout = resolve(Path::new(root));
    candidates
        .into_iter()
        .filter_map(|worktree| locate_change(&layout, worktree.as_deref(), change_name))
        .map(|location| location.dir)
        .collect()
}

/// 读回某 exec 的输出记录
#[tauri::command]
#[specta::specta]
pub fn read_exec_output(
    stores: State<'_, WorkspaceStores>,
    root: String,
    change_name: String,
    exec_id: String,
) -> Option<String> {
    if is_blank_root(&root) {
        return None;
    }
    // exec_id 全局唯一，逐候选探测直至命中——永不取回他 change 的记录（miss 即
    // None，非错读）；blank root / 开库失败 / miss 三条 None 语义逐条不变
    change_dirs(&stores, &root, &change_name)
        .into_iter()
        .find_map(|dir| exec::read_exec_output_in(&dir, &exec_id))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// 布局派生的 `.log` 绝对路径（供前端 watch 寻址；目录名知识 MUST NOT 下沉
/// 前端）。照 `explore_doc_path` 同构：blank root / 开库失败 / 非法或未知
/// `exec_id` → `None`。
#[tauri::command]
#[specta::specta]
pub fn exec_output_path(
    stores: State<'_, WorkspaceStores>,
    root: String,
    change_name: String,
    exec_id: String,
) -> Option<String> {
    if is_blank_root(&root) {
        return None;
    }
    change_dirs(&stores, &root, &change_name)
        .into_iter()
        .find_map(|dir| exec::exec_output_log_path_in(&dir, &exec_id))
        .map(|path| path.to_string_lossy().into_owned())
}

/// 读回某 exec 的执行命令（sidecar `<exec_id>.cmd`）；老记录无 sidecar ⇒ `None`
/// （面板优雅降级「命令未记录」，非错误）。
#[tauri::command]
#[specta::specta]
pub fn read_exec_command(
    stores: State<'_, WorkspaceStores>,
    root: String,
    change_name: String,
    exec_id: String,
) -> Option<String> {
    if is_blank_root(&root) {
        return None;
    }
    change_dirs(&stores, &root, &change_name)
        .into_iter()
        .find_map(|dir| exec::read_exec_command_in(&dir, &exec_id))
}

/// 增量读输出记录：`from_offset = 0` 即全量；游标唯一产出方在后端（前端零字节
/// 计数）。运行中零字节 → `Some { text: "", nextOffset: 0 }`（与 miss 的 `None`
/// 可分辨：「运行中，等待输出…」vs「（无输出记录）」）。
#[tauri::command]
#[specta::specta]
pub fn read_exec_output_delta(
    stores: State<'_, WorkspaceStores>,
    root: String,
    change_name: String,
    exec_id: String,
    from_offset: u64,
) -> Option<ExecOutputChunk> {
    if is_blank_root(&root) {
        return None;
    }
    change_dirs(&stores, &root, &change_name)
        .into_iter()
        .find_map(|dir| exec::read_exec_output_delta_in(&dir, &exec_id, from_offset))
        .map(|delta| ExecOutputChunk {
            text: delta.text,
            next_offset: delta.next_offset,
        })
}

/// worktree 候选去重入列（探测序保持——首个入列位次即探测优先级）。
fn push_worktree_candidate(candidates: &mut Vec<Option<String>>, worktree: Option<String>) {
    if !candidates.contains(&worktree) {
        candidates.push(worktree);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn create_change(
    app: AppHandle,
    root: String,
    name: String,
    goal: String,
    workflow_type: String,
) -> Result<CreateOutcome, String> {
    let title = name.clone();
    create_change_with(app, root, name, goal, title, workflow_type).await
}

/// [`create_change`] 的泛型测试缝（生产注入 Wry 句柄、测试注入 MockRuntime
/// 句柄，沿 `archive_change_with` 先例）：装配 vcs 落位派生（git 根同级
/// `.worktrees/<仓库目录名>/`）与 `ProcessWorktree`，经 `spawn_blocking` 调写面。
/// `title` 显式传入（空白回退 `name` 单点在写面）——promote 路径据此继承
/// `explore.title`；`workflow_type` 原样透传写面（合法值校验亦在写面）。
pub(crate) async fn create_change_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    name: String,
    goal: String,
    title: String,
    workflow_type: String,
) -> Result<CreateOutcome, String> {
    if root.trim().is_empty() {
        return Err("非法 root: 不得为空白".to_owned());
    }
    let stores = app.state::<WorkspaceStores>();
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let main_root = PathBuf::from(root);
    let vcs = ProcessWorktree::new();
    tauri::async_runtime::spawn_blocking(move || {
        // 依赖引导项目根经 config crate 读取（主 workspace root 当前配置；
        // 未配置 / 空列表 → legacy worktree 根扫描由写面分派）。
        let project_roots: Vec<String> = config::load(&main_root)
            .config
            .projects
            .into_iter()
            .map(|project| project.root)
            .collect();
        // worktree 落位父锚由写面经 vcs port 单点派生（git 根同级
        // `.worktrees/<仓库目录名>/`），派生时序与前置校验对齐。
        write::create(
            &main_root,
            store.as_ref(),
            &vcs,
            &project_roots,
            &name,
            &goal,
            &title,
            &workflow_type,
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

/// 用户删除 change（desktop-change-delete）：按 **id** 寻址——可选清空
/// worktree（目录 + 分支 `change/<name>`）→ 删主仓产物目录 → 级联删库记录；
/// blank root / id 显式 `Err`。
///
/// **运行中拒绝**：`ChangeFlowControl` 注册表是「此刻是否有 run 在跑」的唯一
/// 权威（`active_phase` 可能是中断残留），有条目即拒绝且零副作用——删除面
/// MUST NOT 自行停止 run。该判定驻命令层：core/workflow 不得依赖
/// orchestration。
#[tauri::command]
#[specta::specta]
pub async fn delete_change(
    app: AppHandle,
    root: String,
    id: String,
    clean_worktree: bool,
) -> Result<DeleteOutcome, String> {
    delete_change_with(app, root, id, clean_worktree).await
}

/// [`delete_change`] 的泛型测试缝（沿 `archive_change_with` 先例）：运行中
/// 判定 → 装配 `ProcessWorktree` → `spawn_blocking` 调写面。
pub(crate) async fn delete_change_with<R: tauri::Runtime>(
    app: AppHandle<R>,
    root: String,
    id: String,
    clean_worktree: bool,
) -> Result<DeleteOutcome, String> {
    if root.trim().is_empty() {
        return Err("非法 root: 不得为空白".to_owned());
    }
    if id.trim().is_empty() {
        return Err("非法 id: 不得为空白".to_owned());
    }
    if app
        .state::<Arc<ChangeFlowControl>>()
        .snapshot(&root, &id)
        .is_some()
    {
        return Err(format!("change \"{id}\" 正在运行中，请先停止运行后再删除"));
    }
    let stores = app.state::<WorkspaceStores>();
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let main_root = PathBuf::from(&root);
    let vcs = ProcessWorktree::new();
    tauri::async_runtime::spawn_blocking(move || {
        let layout = resolve(&main_root);
        write::delete(
            &main_root,
            &layout,
            store.as_ref(),
            &vcs,
            &id,
            clean_worktree,
        )
    })
    .await
    .map_err(|e| format!("delete 任务失败: {e}"))?
}
