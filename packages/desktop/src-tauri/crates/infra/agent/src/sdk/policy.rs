use agent::AgentPermissionMode;

use crate::sdk::mount::ToolTier;

/// 档位 × 挂载 tier → 允许与否（单处四行映射，与改造前三张按名常量表逐格
/// 等价）：`Protocol` 与 `Read` 三档恒放行，`Write` / `Execute` 仅 Default 档
/// 拒绝。MUST NOT 恢复按工具名的准入清单。
pub fn allows(mode: AgentPermissionMode, tier: ToolTier) -> bool {
    match tier {
        ToolTier::Protocol | ToolTier::Read => true,
        ToolTier::Write | ToolTier::Execute => mode != AgentPermissionMode::Default,
    }
}
