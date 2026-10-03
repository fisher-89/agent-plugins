use agent::AgentPermissionMode;

/// 协议轮参数的 CLI 投影（ask 阶段组装输入；cwd 经 `current_dir` 传递不进
/// 本形状）。
pub struct TurnParams {
    /// 轮提问
    pub prompt: String,
    /// permission-mode 档位
    pub permission_mode: AgentPermissionMode,
    /// 引擎侧先行句柄（`--resume <id>` 尾追加规则；None 即全新会话）
    pub resume_handle: Option<String>,
}

/// `--permission-mode` flag 值（serde 序列化派生：值单一来源 = 线格式值域，
/// 与落库 / IPC 命名逐字一致，无双轨字符串副本）。
fn permission_mode_flag_value() -> String {
    match serde_json::to_value(AgentPermissionMode::AcceptEdits) {
        Ok(serde_json::Value::String(text)) => text,
        _ => unreachable!("档位枚举序列化恒为 JSON 字符串"),
    }
}

/// 组装 CLI 参数序列（不含程序名本身，逐参传递）。
pub fn build_args(params: &TurnParams) -> Vec<String> {
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
    if let Some(session_id) = &params.resume_handle {
        args.push("--resume".to_owned());
        args.push(session_id.clone());
    }
    args.push("--disallowedTools".to_owned());
    args.push("AskUserQuestion".to_owned());
    args
}
