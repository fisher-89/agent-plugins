//! 工作区配置命令轨道：单命令 `workspace_config`——工作区配置文件单次
//! 读取解析产出 `{ config, diagnostics }` 信封，无状态薄包装 +
//! `workspace_config_inner` 领域组装纯函数（`*_inner` app 层微形态先例，
//! 离 Tauri 运行时可测）。
//!
//! 三件事纪律沿 `commands/changes` 模板：参数转换 → 调用 → 错误映射。与
//! code_stats 裁定同式：无效 root（缺失 / 不可读 / 非目录 / 空白）统一
//! `Err`（壳态 root 恒有值，blank 只能来自调用 bug）；配置文件缺失不是
//! `Err`（多数 workspace 常态），以报告内 `fileMissing` 诊断标记、页面
//! 呈空态。解析语义（校验 / 默认值 / passthrough）全部委托 config crate
//! （工作区配置唯一出口），命令层零自有校验 / 默认值规则。无 State、无
//! 缓存、不落库——每次调用完整重读重校验。
//!
//! 能力 spec：`specs/desktop-workspace-config/spec.md`（路径相对域根）。

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

#[cfg(test)]
mod mod_test;
