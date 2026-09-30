//! 静态权限三档纯决策表：档位 × 工具名 → 允许/拒绝。
//!
//! 档位语义与 CLI 租户的 permission-mode 对齐（档位枚举为 core 契约
//! [`AgentPermissionMode`]，本表是 sdk 引擎侧的解释落点）：
//! - `default`：只读四工具放行，写面拒绝（无头下需审批工具直接被拒的同款
//!   口径）
//! - `acceptEdits`：只读 + write / edit 放行
//! - `bypassPermissions`：全放行（调试页默认档）
//!
//! bash 不在 sdk 引擎工具面（MVP 无进程执行），表中无 bash 行——将来引入
//! bash 时的拒绝语义即本表新增行的预留档位。拒绝不中断 run：由 loop 侧
//! 合成 `SystemNotice{permission_denied}` + is_error ToolResult 回灌续轮。

use agent::AgentPermissionMode;

/// 只读工具（default 档放行面）。
const READONLY_TOOLS: [&str; 4] = ["read", "grep", "glob", "ls"];

/// 写面工具（acceptEdits 档追加放行）。
const WRITE_TOOLS: [&str; 2] = ["write", "edit"];

/// 工具名是否在清单内。
fn listed(tool: &str, tools: &[&str]) -> bool {
    tools.contains(&tool)
}

/// 档位 × 工具名 → 允许与否（未知工具名一律拒绝——工具面封闭清单，模型
/// 幻觉出的工具名不进执行）。
pub fn allows(mode: AgentPermissionMode, tool: &str) -> bool {
    match mode {
        AgentPermissionMode::BypassPermissions => {
            listed(tool, &READONLY_TOOLS) || listed(tool, &WRITE_TOOLS)
        }
        AgentPermissionMode::AcceptEdits => {
            listed(tool, &READONLY_TOOLS) || listed(tool, &WRITE_TOOLS)
        }
        AgentPermissionMode::Default => listed(tool, &READONLY_TOOLS),
    }
}
