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
// flag 序列零回归（正向：三档全组合，尾禁 ask 两枚恒最末）
// ---------------------------------------------------------------------------

#[test]
fn bypass档组装恰含六要素flag尾禁ask两枚无多余项() {
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
            "--disallowedTools".to_owned(),
            "AskUserQuestion".to_owned(),
        ],
        "组装结果恰为六要素（含尾禁 ask 两枚），无额外 flag"
    );
}

#[test]
fn 三档全组合组装结果与既有基线逐项相等() {
    // Default（无 permission flag）/ AcceptEdits / BypassPermissions 三档穷尽：
    // 组装结果与现行基线逐项相等（尾禁 ask 两枚恒在最末）
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
            baseline_args("任务", mode),
            "组合 permission={mode:?} 与现行基线逐项相等"
        );
        // 档位段（基础五要素 -p/prompt/stream-json/verbose 之后、禁 ask 尾两枚
        // 之前）
        let head = &args[..args.len() - 2];
        match expected_flags {
            Some(expected) => assert_eq!(
                &head[5..],
                expected,
                "组合 permission={mode:?} 档位段逐字一致"
            ),
            None => assert!(
                head[5..].is_empty(),
                "组合 permission={mode:?} 无档位 flag，实际: {head:?}"
            ),
        }
        assert_eq!(
            &args[args.len() - 2..],
            ["--disallowedTools", "AskUserQuestion"],
            "组合 permission={mode:?} 尾禁 ask 两枚恒在最末"
        );
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
// --resume 段与禁 ask 尾两枚（prior_handle Some/None）
// ---------------------------------------------------------------------------

#[test]
fn prior_handle为some时resume段紧邻尾禁ask两枚且前缀与基线一致() {
    let args = build_args(&params_with_resume(
        "任务",
        AgentPermissionMode::BypassPermissions,
        Some("s-1"),
    ));

    // 禁 ask 两枚恒为参数序列最末两枚（Continue 组合同样成立）
    assert_eq!(
        &args[args.len() - 2..],
        ["--disallowedTools", "AskUserQuestion"],
        "禁 ask 两枚恒在最末，实际: {args:?}"
    );
    // --resume <id> 两枚紧邻禁 ask 尾两枚之前（variadic flag 之后的唯一安全位）
    assert_eq!(
        &args[args.len() - 4..args.len() - 2],
        ["--resume", "s-1"],
        "--resume <id> 紧邻禁 ask 尾两枚之前，实际: {args:?}"
    );
    // resume 段之外的前缀与无 resume 基线逐项相等
    let baseline = build_args(&params("任务", AgentPermissionMode::BypassPermissions));
    assert_eq!(
        &args[..args.len() - 4],
        &baseline[..baseline.len() - 2],
        "resume 段之外的前缀与既有序列逐项相等"
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
            baseline_args("任务", permission_mode),
            "None 组装与现行基线逐项相等（组合 permission={permission_mode:?}）"
        );
    }
}

#[test]
fn 组合下resume紧邻尾禁ask两枚且档位flag与相对次序不变() {
    for permission_mode in [
        AgentPermissionMode::Default,
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        let with_resume = build_args(&params_with_resume("任务", permission_mode, Some("s-1")));
        let without_resume = build_args(&params("任务", permission_mode));

        // 禁 ask 两枚恒最末（New / Continue 组合位置一致）
        assert_eq!(
            &with_resume[with_resume.len() - 2..],
            ["--disallowedTools", "AskUserQuestion"],
            "组合 permission={permission_mode:?}：禁 ask 两枚恒在最末"
        );
        // --resume 段紧邻禁 ask 尾两枚之前
        assert_eq!(
            &with_resume[with_resume.len() - 4..with_resume.len() - 2],
            ["--resume", "s-1"],
            "组合 permission={permission_mode:?}：--resume 恒在禁 ask 两枚之前"
        );
        // resume 段之前与无 resume 组装逐项相等（档位 flag 面与相对次序不变）
        assert_eq!(
            &with_resume[..with_resume.len() - 4],
            &without_resume[..without_resume.len() - 2],
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
        &args[args.len() - 4..args.len() - 2],
        ["--resume", ""],
        "空串 prior_handle 仍组装为 --resume + 空 arg 两项（禁 ask 两枚之前），实际: {args:?}"
    );
    assert_eq!(
        &args[args.len() - 2..],
        ["--disallowedTools", "AskUserQuestion"],
        "禁 ask 两枚恒在最末"
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

    // resume id 处于禁 ask 尾两枚之前的末位 arg
    assert_eq!(
        args.get(args.len() - 3).map(String::as_str),
        Some(long.as_str()),
        "prior_handle 原样保留为单个 arg，不被空白/特殊字符拆分或截断"
    );
    assert_eq!(
        args.iter().filter(|arg| *arg == "--resume").count(),
        1,
        "不因 id 内空白产生第二个 --resume flag"
    );
    assert_eq!(
        &args[args.len() - 2..],
        ["--disallowedTools", "AskUserQuestion"],
        "禁 ask 两枚恒在最末"
    );
}

// ---------------------------------------------------------------------------
// headless 禁 ask 尾两枚（--disallowedTools AskUserQuestion 恒最末）
// ---------------------------------------------------------------------------

#[test]
fn 禁ask两枚恒为参数序列最末两枚_三档乘resume全组合() {
    for permission_mode in [
        AgentPermissionMode::Default,
        AgentPermissionMode::AcceptEdits,
        AgentPermissionMode::BypassPermissions,
    ] {
        for resume_handle in [None, Some("s-1")] {
            let args = build_args(&params_with_resume("任务", permission_mode, resume_handle));
            assert_eq!(
                &args[args.len() - 2..],
                ["--disallowedTools", "AskUserQuestion"],
                "组合 permission={permission_mode:?} resume={resume_handle:?}：禁 ask 两枚恒为最末两枚，实际: {args:?}"
            );
            // 各恰一枚（variadic flag 无重复；工具名字面量 camelCase 逐字）
            assert_eq!(
                args.iter().filter(|arg| *arg == "--disallowedTools").count(),
                1,
                "组合 permission={permission_mode:?} resume={resume_handle:?}：--disallowedTools 恰一枚"
            );
            assert_eq!(
                args.iter().filter(|arg| *arg == "AskUserQuestion").count(),
                1,
                "组合 permission={permission_mode:?} resume={resume_handle:?}：AskUserQuestion 恰一枚"
            );
        }
    }
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
// 基线对照装置（现行组装序列，逐项镜像）
// ---------------------------------------------------------------------------

/// 无 resume 组装的现行基线：基础五要素 + 档位 flag + 尾禁 ask 两枚
/// （`--resume` 段由各用例按 prior_handle 侧断言，不在此镜像）。
fn baseline_args(prompt: &str, permission_mode: AgentPermissionMode) -> Vec<String> {
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
    args.push("--disallowedTools".to_owned());
    args.push("AskUserQuestion".to_owned());
    args
}
