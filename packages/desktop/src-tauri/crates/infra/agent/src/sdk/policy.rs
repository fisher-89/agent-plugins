use agent::AgentPermissionMode;

/// 只读工具（default 档放行面；两个 archi 只读工具同列——读面口径一致）。
const READONLY_TOOLS: [&str; 6] = ["read", "grep", "glob", "ls", "archi_module", "archi_search"];

/// 写面工具（acceptEdits 档追加放行；`create_explore_note` 归写面档位——
/// Default 拒绝、AcceptEdits / BypassPermissions 放行）。
const WRITE_TOOLS: [&str; 3] = ["write", "edit", "create_explore_note"];

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
