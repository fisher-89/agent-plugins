//! flag 组装纯函数：`AgentRunParams` → CLI 参数序列。
//!
//! 组装口径（能力 spec `specs/desktop-agent-execution/spec.md`，路径相对域根）：
//! - `-p <prompt>` 与 `--output-format stream-json --verbose` 恒有（stream-json
//!   官方示例全部搭配 `--verbose`，是唯一线上格式）
//! - env=bare → `--bare`；env=default → 无 flag
//! - permission-mode=bypassPermissions → `--dangerously-skip-permissions`；
//!   acceptEdits → `--permission-mode acceptEdits`；default → 无 flag
//! - resume_session_id=Some(id) → 尾部追加 `--resume <id>`（显式续会话）；
//!   `None` → 无此 flag
//! - cwd 经 `Command::current_dir` 传递，非 flag；无 model /
//!   partial-messages / `--continue` flag（MVP 边界，续话一律显式 session id）

use agent::{AgentEnvMode, AgentPermissionMode, AgentRunParams};

/// 组装 CLI 参数序列（不含程序名本身，逐参传递）。
pub fn build_args(params: &AgentRunParams) -> Vec<String> {
    let mut args = vec![
        "-p".to_owned(),
        params.prompt.clone(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
    ];
    match params.env {
        AgentEnvMode::Bare => args.push("--bare".to_owned()),
        AgentEnvMode::Default => {}
    }
    match params.permission_mode {
        AgentPermissionMode::BypassPermissions => {
            args.push("--dangerously-skip-permissions".to_owned());
        }
        AgentPermissionMode::AcceptEdits => {
            args.push("--permission-mode".to_owned());
            args.push(AgentPermissionMode::AcceptEdits.as_str().to_owned());
        }
        AgentPermissionMode::Default => {}
    }
    if let Some(session_id) = &params.resume_session_id {
        args.push("--resume".to_owned());
        args.push(session_id.clone());
    }
    args
}
