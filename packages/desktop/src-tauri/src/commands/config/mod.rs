//! 工作区配置命令轨道（**双命令**）：读命令 `workspace_config`（配置文件单次读取
//! 解析产出 `{ config, diagnostics }` 信封）与写命令 `workspace_config_apply`
//! （用户点提案卡「应用」触发的写回）。两条命令各配一个 `*_inner` 领域组装纯函数
//! （`*_inner` app 层微形态先例，离 Tauri 运行时可测）。
//!
//! **写权边界**：工作区配置文件是写保护内置锁定的第一条（agent 永远不可写）；
//! 写回唯一入口 = 提案卡上的「应用」按钮 → `workspace_config_apply`，它是应用进程
//! 自写、不经 agent 工具面（模型不可调用、不可注册为工具）、不落库、无缓存。
//!
//! 三件事纪律沿 `commands/changes` 模板：参数转换 → 调用 → 错误映射。与
//! code_stats 裁定同式：无效 root（缺失 / 不可读 / 非目录 / 空白）统一
//! `Err`（壳态 root 恒有值，blank 只能来自调用 bug）；配置文件缺失不是
//! `Err`（多数 workspace 常态），以报告内 `fileMissing` 诊断标记、页面
//! 呈空态。解析语义（校验 / 默认值 / passthrough）全部委托 config crate
//! （工作区配置唯一出口），命令层零自有校验 / 默认值规则。无 State、无
//! 缓存、不落库——每次调用完整重读重校验。
//!
//! 能力 spec：`specs/desktop-workspace-config/spec.md`（路径相对架构域顶层目录）。

use std::{fs, path::Path};

use config::{ConfigDiagnostic, WorkspaceConfig};
use serde::Serialize;
use specta::Type;

/// 工作区配置报告（命令线面信封）：`config` 永远合法（违例字段以默认值
/// 填充），`diagnostics` 逐条记录文件态 / 违例 / 吃默认项；配置文件缺失以
/// `fileMissing` 诊断在案（空态标记，非错误）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfigReport {
    /// 永远合法的完整配置
    pub config: WorkspaceConfig,
    /// 逐条诊断
    pub diagnostics: Vec<ConfigDiagnostic>,
}

/// root 有效性检查（config 轨道私有；不与 `commands::archi::check_root` 合并——
/// 两者的 root 文案已分叉）：空白 / 缺失 / 不可读 / 非目录一律 `Err`。
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

/// 工作区配置：root 有效性检查 → config crate 单次读取解析 → 信封平移。
/// 缺失 / 不可读 / 非目录 root 返回 `Err`（MUST NOT panic、MUST NOT 静默
/// 空报告）；配置文件缺失返回 `Ok` 报告（`fileMissing` 诊断在案）。
#[tauri::command]
#[specta::specta]
pub fn workspace_config(root: String) -> Result<WorkspaceConfigReport, String> {
    workspace_config_inner(Path::new(&root))
}

/// 领域组装纯函数（无 Tauri State）：`fs::metadata` 有效性检查为 Err 通道
/// 唯一来源 → `config::load` → `ConfigReport` → `WorkspaceConfigReport`
/// 字段平移（命令层零加工）。
pub fn workspace_config_inner(root: &Path) -> Result<WorkspaceConfigReport, String> {
    // Err 通道唯一来源：metadata 失败即缺失 / 不可读，非目录同拒
    let metadata =
        fs::metadata(root).map_err(|e| format!("root 无效（{}）：{e}", root.display()))?;
    if !metadata.is_dir() {
        return Err(format!("root 不是目录（{}）", root.display()));
    }

    let report = config::load(root);
    Ok(WorkspaceConfigReport {
        config: report.config,
        diagnostics: report.diagnostics,
    })
}

/// 配置写回（唯一写命令）：root 检查 → `config::plan_apply_config`（纯函数、零
/// `InvalidValue` 门）→ 命令层原子写 → 重读报告。入参是 **file face**
/// `serde_json::Value`（出线 TS `unknown`）——MUST NOT 收 typed `WorkspaceConfig`
/// （两面不同名，typed 入参等于要求调用方按 camelCase 反向拼装）。
///
/// 双校验不是冗余：propose 侧的校验可能发生在很久以前、可能来自历史会话重放，
/// 前端载荷也可伪造——apply 侧 MUST NOT 信任调用方。
#[tauri::command]
#[specta::specta]
pub fn workspace_config_apply(
    root: String,
    candidate: serde_json::Value,
) -> Result<WorkspaceConfigReport, String> {
    workspace_config_apply_inner(Path::new(&root), &candidate)
}

/// 领域组装纯函数（无 Tauri State）：`plan_apply_config` 失败时 `write_atomic`
/// 根本不在调用链上——「失败零写入」是结构性的，不靠一条 `if`；成功路径返回
/// **写后重读**的报告（页面直接替换，省一次往返）。
pub fn workspace_config_apply_inner(
    root: &Path,
    candidate: &serde_json::Value,
) -> Result<WorkspaceConfigReport, String> {
    check_root(root)?;
    let plan = config::plan_apply_config(root, candidate)?;
    crate::commands::atomic::write_atomic(&plan.file, &plan.content)?;
    let report = config::load(root);
    Ok(WorkspaceConfigReport {
        config: report.config,
        diagnostics: report.diagnostics,
    })
}

#[cfg(test)]
mod mod_test;
