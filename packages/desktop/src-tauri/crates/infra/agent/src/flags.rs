//! flag 组装纯函数：`AgentRunParams` → CLI 参数序列。
//!
//! 组装口径（能力 spec `specs/desktop-agent-execution/spec.md`，路径相对域根）：
//! - `-p <prompt>` 与 `--output-format stream-json --verbose` 恒有（stream-json
//!   官方示例全部搭配 `--verbose`，是唯一线上格式）
//! - permission-mode=bypassPermissions → `--dangerously-skip-permissions`；
//!   acceptEdits → `--permission-mode acceptEdits`；default → 无 flag
//! - resume_session_id=Some(id) → 尾部追加 `--resume <id>`（显式续会话）；
//!   `None` → 无此 flag
//! - cwd 经 `Command::current_dir` 传递，非 flag；无 model /
//!   partial-messages / `--continue` flag（MVP 边界，续话一律显式 session id）

use agent::{AgentPermissionMode, AgentRunParams};

/// `--permission-mode` flag 值（serde 序列化派生：值单一来源 = 线格式值域，
/// 与落库 / IPC 命名逐字一致，无双轨字符串副本）。
fn permission_mode_flag_value() -> String {
    match serde_json::to_value(AgentPermissionMode::AcceptEdits) {
        Ok(serde_json::Value::String(text)) => text,
        _ => unreachable!("档位枚举序列化恒为 JSON 字符串"),
    }
}

/// 组装 CLI 参数序列（不含程序名本身，逐参传递）。
pub fn build_args(params: &AgentRunParams) -> Vec<String> {
    let mut args = vec![
        "-p".to_owned(),
        params.prompt.clone(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
    ];
    match params.permission_mode {
        AgentPermissionMode::BypassPermissions => {
            args.push("--dangerously-skip-permissions".to_owned());
        }
        AgentPermissionMode::AcceptEdits => {
            args.push("--permission-mode".to_owned());
            args.push(permission_mode_flag_value());
        }
        AgentPermissionMode::Default => {}
    }
    if let Some(session_id) = &params.resume_session_id {
        args.push("--resume".to_owned());
        args.push(session_id.clone());
    }
    args
}
