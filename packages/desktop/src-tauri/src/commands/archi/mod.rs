//! 架构命令轨道：六命令薄包装（`archi_graph` / `archi_module` / `archi_read_doc` /
//! `archi_change_summary` / `archi_set_gate` / `archi_drift`）+ 六个 `*_inner` 领域
//! 组装纯函数（`*_inner` app 层微形态先例，离 Tauri 运行时可测）。
//!
//! `archi_drift` 是**架构漂移检查**的命令面（两模式）：全量模式直接调判定、增量
//! 模式经 vcs 缝取变更集后同源判定；判定规则全在 `archi::drift`——命令层零判定
//! （无归属匹配、无定级分支、无 `status` 推导），两条消费面（本命令与门禁 runner）
//! 共用同一实现（判定单源）。
//!
//! 三件事纪律沿 `commands/config` 模板：参数转换 → 调用 → 错误映射。投影、诊断、
//! diff 与待落盘产物全在 `archi`——命令层零领域规则（不建图、不解析、不推导）。
//! 唯一写命令是 `archi_set_gate`（写前校验在 `archi::edit::plan_set_gate`，落盘
//! 在本文件的原子写回）；其余四条 MUST NOT 写盘。
//!
//! 空白 / 不可读 root 一律 `Err`（画布无 root 画不出东西；静默空图会与「模型为
//! 空」的冷启动合法态不可辨）。
//!
//! 能力 spec：`specs/desktop-archi-model/spec.md`（IPC 五命令契约）。

use std::fs;
use std::path::Path;

use archi::drift::{ArchiDriftMode, ArchiDriftReport, CheckInput};
use archi::dto::{self, ArchiChangeSummary, ArchiGraph, ArchiModuleDetail};
use archi::GateTier;
use foundation::cancel::CancelFlag;
use orchestration::port::ArchiveVcsPort;
use store::WorkspaceStores;
use tauri::State;
use vcs_runtime::ProcessArchiveVcs;
use workflow::state::ChangeStatus;

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

/// 原子写回：同目录临时文件 + rename 覆盖（失败清临时文件）。模型写面的唯一落盘
/// 点——本文件之外零 `fs::write`（`archi` crate 全程只读盘）。
fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("目标文件无父目录：{}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("目标文件名不可用：{}", path.display()))?;
    let temporary = parent.join(format!("{name}.archi-tmp"));
    if let Err(error) = fs::write(&temporary, content) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "写入临时文件失败（{}）：{error}",
            temporary.display()
        ));
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("替换目标文件失败（{}）：{error}", path.display()));
    }
    Ok(())
}

/// 画布数据（停摆 / 正常两态）。
#[tauri::command]
#[specta::specta]
pub fn archi_graph(root: String) -> Result<ArchiGraph, String> {
    archi_graph_inner(Path::new(&root))
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

/// 文档正文（仅关联集内路径）。
#[tauri::command]
#[specta::specta]
pub fn archi_read_doc(root: String, path: String) -> Result<String, String> {
    archi_read_doc_inner(Path::new(&root), &path)
}

/// [`archi_read_doc`] 的领域组装纯函数：关联集成员校验 → 归一 → 只读读盘。越界
/// 显式 `Err`（MUST NOT 成为任意文件读取通道）。
pub fn archi_read_doc_inner(root: &Path, path: &str) -> Result<String, String> {
    check_root(root)?;
    let linked = dto::linked_doc_paths(root)?;
    let normalized = path.replace('\\', "/");
    let normalized = normalized.trim_start_matches("./");
    if !linked.contains(normalized) {
        return Err(format!("路径不在模块文档关联集内：{path}"));
    }
    let full = root.join(normalized);
    fs::read_to_string(&full).map_err(|e| format!("文档读取失败（{}）：{e}", full.display()))
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
pub fn archi_drift(root: String, mode: ArchiDriftMode) -> Result<ArchiDriftReport, String> {
    let vcs = ProcessArchiveVcs::new();
    archi_drift_inner(Path::new(&root), mode, &vcs)
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
