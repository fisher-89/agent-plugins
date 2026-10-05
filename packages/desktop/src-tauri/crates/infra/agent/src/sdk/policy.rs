//! 静态权限三档纯决策表：档位 × 工具名 → 允许/拒绝。
//!
//! 档位语义与 CLI 租户的 permission-mode 对齐（档位枚举为 core 契约
//! [`AgentPermissionMode`]，本表是 sdk 引擎侧的解释落点）：
//! - `default`：只读四工具放行，写面与执行面拒绝（无头下需审批工具直接被拒
//!   的同款口径——只读档不给执行权）
//! - `acceptEdits`：只读 + write / edit + bash 放行
//! - `bypassPermissions`：全放行（调试页默认档）
//!
//! bash 行为三档首次真实分化（此前 acceptEdits 与 bypassPermissions 两臂
//! 行为完全一致）：执行面仅两写档放行，default 档拒绝。**bash 无沙箱知情
//! 边界**：命令执行不在路径沙箱射程内——bypassPermissions 默认档下 bash 即
//! 模型可执行任意命令、安全护栏为零（超时 + 进程树清理仅为活性护栏）；命令
//! 级白/黑名单、真沙箱、交互审批留后续 change（spec 边界条款留痕，非安全
//! 缺口遗漏）。拒绝不中断 run：由 loop 侧合成
//! `SystemNotice{permission_denied}` + is_error ToolResult 回灌续轮。

use agent::AgentPermissionMode;

/// 只读工具（default 档放行面）。
const READONLY_TOOLS: [&str; 4] = ["read", "grep", "glob", "ls"];

/// 写面工具（acceptEdits 档追加放行）。
const WRITE_TOOLS: [&str; 2] = ["write", "edit"];

/// 执行面工具（acceptEdits / bypassPermissions 档追加放行；default 档拒绝）。
const EXECUTE_TOOLS: [&str; 1] = ["bash"];

/// 工具名是否在清单内。
fn listed(tool: &str, tools: &[&str]) -> bool {
    tools.contains(&tool)
}

/// 档位 × 工具名 → 允许与否（未知工具名一律拒绝——工具面封闭清单，模型
/// 幻觉出的工具名不进执行）。
pub fn allows(mode: AgentPermissionMode, tool: &str) -> bool {
    match mode {
        AgentPermissionMode::BypassPermissions => {
            listed(tool, &READONLY_TOOLS)
                || listed(tool, &WRITE_TOOLS)
                || listed(tool, &EXECUTE_TOOLS)
        }
        AgentPermissionMode::AcceptEdits => {
            listed(tool, &READONLY_TOOLS)
                || listed(tool, &WRITE_TOOLS)
                || listed(tool, &EXECUTE_TOOLS)
        }
        AgentPermissionMode::Default => listed(tool, &READONLY_TOOLS),
    }
}
