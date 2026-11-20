//! 架构命令轨道：九命令薄包装（`archi_graph` / `archi_module` /
//! `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` /
//! `archi_api_check` / `archi_api_generate` / `archi_relation`）+ 九个 `*_inner` 领域
//! 组装纯函数（`*_inner` app 层微形态先例，离 Tauri 运行时可测）。文档正文读取归
//! `commands::fs::read_file`（通用读面，不再有架构专用读取命令）。
//!
//! `archi_drift` 是**架构漂移检查**的命令面（两模式）：全量模式直接调判定、增量
//! 模式经 vcs 缝取变更集后同源判定；判定规则全在 `archi::drift`——命令层零判定
//! （无归属匹配、无定级分支、无 `status` 推导），两条消费面（本命令与门禁 runner）
//! 共用同一实现（判定单源）。`archi_relation` 是**按需事实取样**：只调
//! `archi::relation::relation_detail`（模型面 + 定向扫描 + 两桶），不组装报告、不产出
//! `status` / `passed`、零缓存。
//!
//! 三件事纪律沿 `commands/config` 模板：参数转换 → 调用 → 错误映射。投影、诊断、
//! diff 与待落盘产物全在 `archi`——命令层零领域规则（不建图、不解析、不推导）。
//! 写命令两条（`archi_set_gate` / `archi_api_generate`）：前者的写前校验在
//! `archi::edit::plan_set_gate` 且落盘经命令层共用原子写
//! （`commands::atomic::write_atomic`），后者的写盘单点在
//! `archi_runtime::api_artifacts::write_artifacts`（只增只改、从不删）；其余七条
//! MUST NOT 写盘。
//!
//! 空白 / 不可读 root 一律 `Err`（画布无 root 画不出东西；静默空图会与「模型为
//! 空」的冷启动合法态不可辨）。
//!
//! 执行契约：四条取数命令（`archi_graph` / `archi_drift` / `archi_file_tree` /
//! `archi_api_check`）的成本随仓库规模增长（批量读子系统 / 全仓解析），故一律为
//! `pub async fn`，底经 `spawn_blocking` 落阻塞池——命令体不在 IPC 线程内联，窗口
//! 不随取数冻结。执行线程纪律与判据的唯一持有者是 `desktop-app-shell` 的「命令执行
//! 线程纪律（批量子系统 IO）」，本文件不复述。
//!
//! 能力 spec：`specs/desktop-archi-model/spec.md`（IPC 九命令契约）。

use std::fs;
use std::path::Path;

use archi::drift::{ArchiDriftMode, ArchiDriftReport, CheckInput};
use archi::dto::{
    self, ArchiApiCheck, ArchiApiWriteOutcome, ArchiChangeSummary, ArchiFileTree, ArchiGraph,
    ArchiModuleDetail, ArchiRelationDetail,
};
use archi::GateTier;
use foundation::cancel::CancelFlag;
use orchestration::port::ArchiveVcsPort;
use store::WorkspaceStores;
use tauri::State;
use vcs_runtime::ProcessArchiveVcs;
use workflow::state::ChangeStatus;

use crate::commands::atomic::write_atomic;

#[cfg(test)]
mod mod_test;

/// root 有效性检查（另四条命令共用）：空白 / 缺失 / 不可读 / 非目录一律 `Err`
/// （与 code_stats / workspace_config 的无效 root 裁定同口径）。
fn check_root(root: &Path) -> Result<(), String> {
    if root.as_os_str().is_empty() {
        return Err("root 无效：不得为空白（无工作根无从取数）".to_owned());
    }
    let metadata =
        fs::metadata(root).map_err(|e| format!("root 无效（{}）：{e}", root.display()))?;
    if !metadata.is_dir() {
        return Err(format!("root 不是目录（{}）", root.display()));
    }
    Ok(())
}

/// 画布数据（停摆 / 正常两态）。
#[tauri::command]
#[specta::specta]
pub async fn archi_graph(root: String) -> Result<ArchiGraph, String> {
    tauri::async_runtime::spawn_blocking(move || archi_graph_inner(Path::new(&root)))
        .await
        .map_err(|error| format!("架构图任务失败: {error}"))?
}

/// [`archi_graph`] 的领域组装纯函数：root 检查 → `archi::dto` 投影。
pub fn archi_graph_inner(root: &Path) -> Result<ArchiGraph, String> {
    check_root(root)?;
    dto::graph_view(root)
}

/// 单元素投影（与 agent 工具同一投影）。
#[tauri::command]
#[specta::specta]
pub fn archi_module(root: String, element: String) -> Result<ArchiModuleDetail, String> {
    archi_module_inner(Path::new(&root), &element)
}

/// [`archi_module`] 的领域组装纯函数。
pub fn archi_module_inner(root: &Path, element: &str) -> Result<ArchiModuleDetail, String> {
    check_root(root)?;
    dto::module_detail(root, element)
}

/// 归档确认面的模型结构 diff 摘要（`null` = 无改动 / 无可比基线）。
#[tauri::command]
#[specta::specta]
pub fn archi_change_summary(
    stores: State<'_, WorkspaceStores>,
    root: String,
    change_id: String,
) -> Result<Option<ArchiChangeSummary>, String> {
    check_root(Path::new(&root))?;
    let store = stores.for_root(&root).map_err(|e| e.to_string())?;
    let record = store
        .find_change_record(&change_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("change \"{change_id}\" 未建档（无 ChangeRecord）"))?;
    // 归档 change 的执行根恒回落主仓根（与会话 cwd 同口径）；active 且 worktree
    // 记录在场才以 worktree 为改动侧
    let worktree = match record.status {
        ChangeStatus::Archived => None,
        ChangeStatus::Active => record.worktree,
    };
    archi_change_summary_inner(Path::new(&root), worktree.as_deref())
}

/// [`archi_change_summary`] 的领域组装纯函数：主仓基线 + 改动侧（worktree 或主仓
/// 本身）两侧模型结构 diff。
pub fn archi_change_summary_inner(
    main_root: &Path,
    worktree: Option<&str>,
) -> Result<Option<ArchiChangeSummary>, String> {
    check_root(main_root)?;
    let next_root = match worktree {
        Some(worktree) => Path::new(worktree),
        None => main_root,
    };
    dto::change_summary(main_root, next_root)
}

/// 改档写回（唯一写命令）。
#[tauri::command]
#[specta::specta]
pub fn archi_set_gate(root: String, element: String, gate: GateTier) -> Result<(), String> {
    archi_set_gate_inner(Path::new(&root), &element, gate)
}

/// [`archi_set_gate`] 的领域组装纯函数：root 检查 → 写前校验与待落盘产物（
/// `archi::edit`）→ 原子写回。任一校验失败零写入。
pub fn archi_set_gate_inner(root: &Path, element: &str, gate: GateTier) -> Result<(), String> {
    check_root(root)?;
    let plan = archi::edit::plan_set_gate(root, element, gate)?;
    write_atomic(&plan.file, &plan.content)
}

/// 架构漂移检查（两模式；命令层零判定）。
#[tauri::command]
#[specta::specta]
pub async fn archi_drift(root: String, mode: ArchiDriftMode) -> Result<ArchiDriftReport, String> {
    let vcs = ProcessArchiveVcs::new();
    tauri::async_runtime::spawn_blocking(move || archi_drift_inner(Path::new(&root), mode, &vcs))
        .await
        .map_err(|error| format!("漂移检查任务失败: {error}"))?
}

/// [`archi_drift`] 的领域组装纯函数：root 检查 → 增量经 vcs 缝取变更集 → 调判定
/// 纯函数 → 错误串映射。
///
/// - 模式分流：`full` → 空变更集（无基准）；`incremental` →
///   `change_edit_set(root, "HEAD")`（git 失败显式 `Err`，MUST NOT 降级空清单——
///   空清单会静默变成 `noChanges` = 假通过）；
/// - 无 change 上下文：命令面恒 `change = None`（与门禁面的区别仅此与
///   `timestamp`）；取消不可达（`CancelFlag::never()`）；
/// - 错误面：模型结构错**不** `Err`（进报告 `model.structuralErrors` 并令
///   `status = errors`）；配置非法 / 扫描硬失败 → `Err`（文案指明条目）。
pub fn archi_drift_inner(
    root: &Path,
    mode: ArchiDriftMode,
    vcs: &dyn ArchiveVcsPort,
) -> Result<ArchiDriftReport, String> {
    check_root(root)?;
    let changed = match mode {
        ArchiDriftMode::Full => Vec::new(),
        ArchiDriftMode::Incremental => vcs.change_edit_set(root, "HEAD")?,
    };
    archi::drift::check(CheckInput {
        root,
        mode,
        changed: &changed,
        change: None,
        cancel: CancelFlag::never(),
    })
    .map_err(|error| error.message())
}

/// 左栏文件树数据（仓库源文件与模块根的扁平条目投影）。
#[tauri::command]
#[specta::specta]
pub async fn archi_file_tree(root: String) -> Result<ArchiFileTree, String> {
    tauri::async_runtime::spawn_blocking(move || archi_file_tree_inner(Path::new(&root)))
        .await
        .map_err(|error| format!("文件树任务失败: {error}"))?
}

/// [`archi_file_tree`] 的领域组装纯函数：root 检查 → `archi::tree::file_tree`。
///
/// 只读、零写盘、零 spawn；结构诊断非空 → `Err`（与 `archi_module` 同口径）、空模型
/// → `Ok`（全条目 `mark = none`）；命令层零归属判定 / 零目录合成 / 零标记推导。
pub fn archi_file_tree_inner(root: &Path) -> Result<ArchiFileTree, String> {
    check_root(root)?;
    archi::tree::file_tree(root)
}

/// 契约生成物新鲜度的检查（零写盘）。
#[tauri::command]
#[specta::specta]
pub async fn archi_api_check(root: String) -> Result<ArchiApiCheck, String> {
    tauri::async_runtime::spawn_blocking(move || archi_api_check_inner(Path::new(&root)))
        .await
        .map_err(|error| format!("契约检查任务失败: {error}"))?
}

/// [`archi_api_check`] 的领域组装纯函数：root 检查 → `archi::api::artifact_check`
/// （三类差异 + `enabled` + `meta` 的投影单点在 `archi`）。
///
/// 零写盘、零 spawn、零 watch；`enabled = false` 仍 `Ok`（应生成集只含 `INDEX.toml`，
/// `missing` 照实报）；结构错 / 契约资产非法 → `Err`（文案透传 `ApiError::message()`）。
pub fn archi_api_check_inner(root: &Path) -> Result<ArchiApiCheck, String> {
    check_root(root)?;
    archi::api::artifact_check(root).map_err(|error| error.message())
}

/// 契约生成物写盘（只增只改、从不删）。
#[tauri::command]
#[specta::specta]
pub fn archi_api_generate(root: String) -> Result<ArchiApiWriteOutcome, String> {
    archi_api_generate_inner(Path::new(&root))
}

/// [`archi_api_generate`] 的领域组装纯函数：root 检查 → 应生成集
/// （`archi::workspace_render`）→ 写盘单点
/// （`archi_runtime::api_artifacts::write_artifacts`）。
///
/// 结构错即零写盘（应生成集不可得）；孤儿生成物只报告不删；写盘**非原子**——单份
/// 失败即 `Err` 且已写部分不回滚（文案如实，不称「全或无」）。
pub fn archi_api_generate_inner(root: &Path) -> Result<ArchiApiWriteOutcome, String> {
    check_root(root)?;
    let files = archi::workspace_render(root).map_err(|error| error.message())?;
    archi_runtime::api_artifacts::write_artifacts(root, &files)
}

/// 单条关系的详情（模型面 + 实际使用 + 两桶 + 命中点；点开即取）。
#[tauri::command]
#[specta::specta]
pub fn archi_relation(
    root: String,
    from: String,
    to: String,
) -> Result<ArchiRelationDetail, String> {
    archi_relation_inner(Path::new(&root), &from, &to)
}

/// [`archi_relation`] 的领域组装纯函数：root 检查 → `archi::relation`（同一装载取声明
/// 与声明行、按 `from` 侧白名单定向取样、求差调 `drift` 的同一纯函数）。
///
/// 只读取数（零写盘、零缓存）；模型结构错 / 关系或元素缺席 → `Err` 并点名。命令层
/// 零判定：两桶与命中点都由 `archi` 出，`used ∖ declared` 的档位语义归
/// `desktop-archi-drift`（本命令不出 `status` / `passed`）。
pub fn archi_relation_inner(
    root: &Path,
    from: &str,
    to: &str,
) -> Result<ArchiRelationDetail, String> {
    check_root(root)?;
    archi::relation::relation_detail(root, from, to)
}
