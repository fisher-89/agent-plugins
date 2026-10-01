use agent::AgentPermissionMode;

use crate::flags::{build_args, TurnParams};

fn params(prompt: &str, permission_mode: AgentPermissionMode) -> TurnParams {
    TurnParams {
        prompt: prompt.to_owned(),
        permission_mode,
        resume_handle: None,
    }
}

/// 带引擎侧先行句柄的组装入参（其余字段同 [`params`]）。
fn params_with_resume(
    prompt: &str,
    permission_mode: AgentPermissionMode,
    resume_handle: Option<&str>,
) -> TurnParams {
    TurnParams {
        resume_handle: resume_handle.map(str::to_owned),
        ..params(prompt, permission_mode)
    }
}

// ---------------------------------------------------------------------------
// 四要素 flag 序列零回归（正向：三档全组合）
// ---------------------------------------------------------------------------

#[test]
fn bypass档组装恰含四要素flag无多余项() {
    let args = build_args(&params("任务", AgentPermissionMode::BypassPermissions));

    assert_eq!(
        args,
        vec![
            "-p".to_owned(),
            "任务".to_owned(),
            "--output-format".to_owned(),
            "stream-json".to_owned(),
            "--verbose".to_owned(),
            "--dangerously-skip-permissions".to_owned(),
        ],
        "组装结果恰为四要素，无额外 flag"
    );
}

#[test]
fn 三档全组合组装结果与既有基线逐项相等() {
    // Default（无 permission flag）/ AcceptEdits / BypassPermissions 三档穷尽：
    // 组装结果与既有基线逐项相等（flag 输出序列零变化）
    for (mode, expected_flags) in [
        (AgentPermissionMode::Default, None),
        (
            AgentPermissionMode::AcceptEdits,
            Some(["--permission-mode", "acceptEdits"].as_slice()),
        ),
        (
            AgentPermissionMode::BypassPermissions,
            Some(["--dangerously-skip-permissions"].as_slice()),
        ),
    ] {
        let args = build_args(&params("任务", mode));
        assert_eq!(
            args,
            legacy_args("任务", mode),
            "组合 permission={mode:?} 与既有基线逐项相等"
        );
        // 档位段（基础五要素 -p/prompt/stream-json/verbose 之后、resume 之前）
        let tail = &args[5..];
        match expected_flags {
            Some(expected) => assert_eq!(tail, expected, "组合 permission={mode:?} 档位段逐字一致"),
            None => assert!(
                tail.is_empty(),
                "组合 permission={mode:?} 无档位 flag，实际: {tail:?}"
            ),
        }
    }
}

#[test]
fn accept_edits档位值与serde派生线值同源() {
    let derived =
        serde_json::to_value(AgentPermissionMode::AcceptEdits).expect("档位枚举序列化应成功");
    assert_eq!(
        derived,
        serde_json::json!("acceptEdits"),
        "serde 派生值与既有落库/线格式口径逐字一致"
    );

    let args = build_args(&params("任务", AgentPermissionMode::AcceptEdits));
    let position = args
        .iter()
        .position(|arg| arg == "--permission-mode")
        .expect("acceptEdits 组装含 --permission-mode flag");
    assert_eq!(
        args.get(position + 1).map(String::as_str),
        Some("acceptEdits"),
        "flag 值串逐字为 acceptEdits（单一来源，无字符串副本）"
    );
}

#[test]
fn default档无permission_flag() {
    let default = build_args(&params("任务", AgentPermissionMode::Default));
    assert!(
        !default.iter().any(|arg| arg.contains("permission")),
        "default 档无 permission flag（CLI -p 默认档），实际: {default:?}"
    );
}

// ---------------------------------------------------------------------------
// prompt 保真（边界）
// ---------------------------------------------------------------------------

#[test]
fn 空prompt仍组装空串arg作为单个参数() {
    let args = build_args(&params("", AgentPermissionMode::Default));

    assert_eq!(args.first().map(String::as_str), Some("-p"));
    assert_eq!(
        args.get(1).map(String::as_str),
        Some(""),
        "空 prompt 为单个空 arg"
    );
}

#[test]
fn prompt含空格引号换行中文emoji时原样保留不拆分() {
    let prompt = "两句话 第一句\n\"quoted\" 引号 🎉 中文";
    let args = build_args(&params(prompt, AgentPermissionMode::Default));

    assert_eq!(
        args.get(1).map(String::as_str),
        Some(prompt),
        "prompt 为单个 arg 原样保留"
    );
    assert_eq!(
        args.iter().filter(|arg| **arg == "-p").count(),
        1,
        "不因空白被拆分"
    );
}

#[test]
fn 超长prompt完整保留于arg() {
    let prompt = "长".repeat(1200);
    let args = build_args(&params(&prompt, AgentPermissionMode::Default));

    assert_eq!(args.get(1).map(String::as_str), Some(prompt.as_str()));
}

// ---------------------------------------------------------------------------
// --resume 尾追加（prior_handle Some/None）
// ---------------------------------------------------------------------------

#[test]
fn prior_handle为some时尾部恰追加resume与id且前缀与基线一致() {
    let args = build_args(&params_with_resume(
        "任务",
        AgentPermissionMode::BypassPermissions,
        Some("s-1"),
    ));

    assert_eq!(
        &args[args.len() - 2..],
        ["--resume", "s-1"],
        "--resume <id> 恰在尾部，实际: {args:?}"
    );
    let baseline = build_args(&params("任务", AgentPermissionMode::BypassPermissions));
    assert_eq!(
        &args[..args.len() - 2],
        baseline.as_slice(),
        "resume 之外的前缀与既有四要素序列逐项相等"
    );
}

#[test]
fn prior_handle为none时组装结果与既有基线逐项相等且无resume() {
    for permission_mode in [
        AgentPermissionMode::Default,
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        let args = build_args(&params("任务", permission_mode));
        assert!(
            !args.iter().any(|arg| arg == "--resume"),
            "组合 permission={permission_mode:?} 无 --resume"
        );
        assert_eq!(
            args,
            legacy_args("任务", permission_mode),
            "None 组装与既有基线逐项相等（组合 permission={permission_mode:?}）"
        );
    }
}

#[test]
fn 组合下resume恒在尾部且档位flag与相对次序不变() {
    for permission_mode in [
        AgentPermissionMode::Default,
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        let with_resume = build_args(&params_with_resume("任务", permission_mode, Some("s-1")));
        let without_resume = build_args(&params("任务", permission_mode));

        assert_eq!(
            &with_resume[with_resume.len() - 2..],
            ["--resume", "s-1"],
            "组合 permission={permission_mode:?}：--resume 恒在尾部"
        );
        assert_eq!(
            &with_resume[..with_resume.len() - 2],
            without_resume.as_slice(),
            "组合 permission={permission_mode:?}：档位 flag 面与相对次序不变"
        );
    }
}

// ---------------------------------------------------------------------------
// resume id 保真（边界）
// ---------------------------------------------------------------------------

#[test]
fn prior_handle空串仍组装resume与空arg() {
    let args = build_args(&params_with_resume(
        "任务",
        AgentPermissionMode::Default,
        Some(""),
    ));

    assert_eq!(
        &args[args.len() - 2..],
        ["--resume", ""],
        "空串 prior_handle 仍组装为 --resume + 空 arg 两项，实际: {args:?}"
    );
}

#[test]
fn prior_handle含空格引号换行emoji超长时原样保留单个arg() {
    let handle = "s 1 \"quoted\"\nnext 🎉 中文";
    let long = format!("{handle}{}", "x".repeat(1000));
    let args = build_args(&params_with_resume(
        "任务",
        AgentPermissionMode::Default,
        Some(&long),
    ));

    assert_eq!(
        args.last().map(String::as_str),
        Some(long.as_str()),
        "prior_handle 原样保留为单个 arg，不被空白/特殊字符拆分或截断"
    );
    assert_eq!(
        args.iter().filter(|arg| *arg == "--resume").count(),
        1,
        "不因 id 内空白产生第二个 --resume flag"
    );
}

// ---------------------------------------------------------------------------
// 禁用 flag 负向与 cwd（边界）
// ---------------------------------------------------------------------------

#[test]
fn 全组合不含禁用flag() {
    let forbidden = ["--resume", "--include-partial-messages", "--model"];
    for permission_mode in [
        AgentPermissionMode::Default,
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        let args = build_args(&params("任务", permission_mode));
        for flag in forbidden {
            assert!(
                !args.iter().any(|arg| arg.starts_with(flag)),
                "组合 permission={permission_mode:?} 不得含 {flag}，实际: {args:?}"
            );
        }
    }
}

#[test]
fn cwd不进协议轮参数形状_组装结果不含工作目录片段() {
    // 协议轮参数形状（TurnParams）无 cwd 字段：cwd 仅经 Command::current_dir
    // 传递（编译期锚定——本构造若需 cwd 即编译失败）；组装结果亦不含路径片段
    let turn_params = TurnParams {
        prompt: "任务".to_owned(),
        permission_mode: AgentPermissionMode::BypassPermissions,
        resume_handle: None,
    };
    let args = build_args(&turn_params);

    assert!(
        !args
            .iter()
            .any(|arg| arg.contains("work") || arg.contains("demo") || arg.contains('\\')),
        "组装结果不含 cwd 片段，实际: {args:?}"
    );
    // 三档组装互异性（档位 flag 面各自成立）
    let default_args = build_args(&params("任务", AgentPermissionMode::Default));
    let accept_args = build_args(&params("任务", AgentPermissionMode::AcceptEdits));
    assert_ne!(args, default_args);
    assert_ne!(args, accept_args);
}

// ---------------------------------------------------------------------------
// 基线对照装置（演进前组装序列，逐项镜像）
// ---------------------------------------------------------------------------

fn legacy_args(prompt: &str, permission_mode: AgentPermissionMode) -> Vec<String> {
    let mut args = vec![
        "-p".to_owned(),
        prompt.to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
    ];
    match permission_mode {
        AgentPermissionMode::BypassPermissions => {
            args.push("--dangerously-skip-permissions".to_owned());
        }
        AgentPermissionMode::AcceptEdits => {
            args.push("--permission-mode".to_owned());
            args.push("acceptEdits".to_owned());
        }
        AgentPermissionMode::Default => {}
    }
    args
}
