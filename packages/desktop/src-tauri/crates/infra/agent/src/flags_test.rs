//! `flags` 的单元测试（AC-2 / AC-6）：`build_args` 纯函数的组装口径——
//! `-p` + stream-json + verbose 恒有、env/permission-mode 映射、prompt 保真、
//! cwd 不产生 flag、`--resume` 条件组装（`resume_session_id` 显式续会话）与
//! 禁用 flag 全组合负向断言。无外部依赖，不需要 Mock。

use std::path::PathBuf;

use agent::{AgentEnvMode, AgentPermissionMode, AgentRunParams};

use crate::flags::build_args;

fn params(prompt: &str, env: AgentEnvMode, permission_mode: AgentPermissionMode) -> AgentRunParams {
    AgentRunParams {
        prompt: prompt.to_owned(),
        cwd: PathBuf::from("C:\\work\\demo"),
        env,
        permission_mode,
        resume_session_id: None,
    }
}

/// 带续会话入参的组装入参（其余字段同 [`params`]）。
fn params_with_resume(
    prompt: &str,
    env: AgentEnvMode,
    permission_mode: AgentPermissionMode,
    resume_session_id: Option<&str>,
) -> AgentRunParams {
    AgentRunParams {
        resume_session_id: resume_session_id.map(str::to_owned),
        ..params(prompt, env, permission_mode)
    }
}

// ---------------------------------------------------------------------------
// 组装口径（正向）
// ---------------------------------------------------------------------------

#[test]
fn default加bypass组装恰含四要素flag() {
    let args = build_args(&params(
        "任务",
        AgentEnvMode::Default,
        AgentPermissionMode::BypassPermissions,
    ));

    // AC-2 四 flag 断言：-p <prompt>、--output-format stream-json、--verbose、
    // --dangerously-skip-permissions；且无多余项
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
fn env为bare时追加bare而default不含bare() {
    let bare = build_args(&params(
        "任务",
        AgentEnvMode::Bare,
        AgentPermissionMode::Default,
    ));
    assert!(
        bare.contains(&"--bare".to_owned()),
        "AC-2：bare 追加 --bare，实际: {bare:?}"
    );

    let default = build_args(&params(
        "任务",
        AgentEnvMode::Default,
        AgentPermissionMode::Default,
    ));
    assert!(
        !default.contains(&"--bare".to_owned()),
        "default 档不含 --bare，实际: {default:?}"
    );
}

#[test]
fn permission_mode三档分别映射acceptedits与dangerously与无flag() {
    let accept_edits = build_args(&params(
        "任务",
        AgentEnvMode::Default,
        AgentPermissionMode::AcceptEdits,
    ));
    let position = accept_edits
        .iter()
        .position(|arg| arg == "--permission-mode")
        .expect("acceptEdits 组装含 --permission-mode flag");
    assert_eq!(
        accept_edits.get(position + 1).map(String::as_str),
        Some("acceptEdits"),
        "档位值与 as_str 落库口径一致"
    );

    let default = build_args(&params(
        "任务",
        AgentEnvMode::Default,
        AgentPermissionMode::Default,
    ));
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
    let args = build_args(&params(
        "",
        AgentEnvMode::Default,
        AgentPermissionMode::Default,
    ));

    // 组装层恒有 -p（必填把关在 UI 层）：-p 后紧跟空串 arg
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
    let args = build_args(&params(
        prompt,
        AgentEnvMode::Default,
        AgentPermissionMode::Default,
    ));

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
    let args = build_args(&params(
        &prompt,
        AgentEnvMode::Default,
        AgentPermissionMode::Default,
    ));

    assert_eq!(args.get(1).map(String::as_str), Some(prompt.as_str()));
}

// ---------------------------------------------------------------------------
// AC-6 负向断言与 cwd（边界）
// ---------------------------------------------------------------------------

#[test]
fn 全组合九种档位未传resume时均不含resume与禁用flag() {
    // AC-6：resume 未传时无 --resume；--include-partial-messages / model flag
    // 维持禁用（MVP 边界：--resume 随本变更解禁为条件 flag，其余禁用项不变）
    let forbidden = ["--resume", "--include-partial-messages", "--model"];
    for env in [AgentEnvMode::Default, AgentEnvMode::Bare] {
        for permission_mode in [
            AgentPermissionMode::Default,
            AgentPermissionMode::AcceptEdits,
            AgentPermissionMode::BypassPermissions,
        ] {
            let args = build_args(&params("任务", env, permission_mode));
            for flag in forbidden {
                assert!(
                    !args.iter().any(|arg| arg.starts_with(flag)),
                    "组合 env={env:?} × permission={permission_mode:?} 不得含 {flag}，实际: {args:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// resume 组装（AC-6：显式续会话条件 flag）
// ---------------------------------------------------------------------------

/// 演进前组装基线（不含 resume 字段时代的组装序列，逐项镜像）：
/// 向后兼容回归的对照基准。
fn legacy_args(
    prompt: &str,
    env: AgentEnvMode,
    permission_mode: AgentPermissionMode,
) -> Vec<String> {
    let mut args = vec![
        "-p".to_owned(),
        prompt.to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
    ];
    match env {
        AgentEnvMode::Bare => args.push("--bare".to_owned()),
        AgentEnvMode::Default => {}
    }
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

#[test]
fn resume为some时尾部恰追加resume与id且前缀与基线一致() {
    let args = build_args(&params_with_resume(
        "任务",
        AgentEnvMode::Default,
        AgentPermissionMode::BypassPermissions,
        Some("s-1"),
    ));

    assert_eq!(
        &args[args.len() - 2..],
        ["--resume", "s-1"],
        "--resume <id> 恰在尾部，实际: {args:?}"
    );
    let baseline = build_args(&params(
        "任务",
        AgentEnvMode::Default,
        AgentPermissionMode::BypassPermissions,
    ));
    assert_eq!(
        &args[..args.len() - 2],
        baseline.as_slice(),
        "resume 之外的前缀与既有四要素序列逐项相等（其余 flag 组装不变）"
    );
}

#[test]
fn resume为none时组装结果与既有基线逐项相等且无resume() {
    for env in [AgentEnvMode::Default, AgentEnvMode::Bare] {
        for permission_mode in [
            AgentPermissionMode::Default,
            AgentPermissionMode::AcceptEdits,
            AgentPermissionMode::BypassPermissions,
        ] {
            let args = build_args(&params("任务", env, permission_mode));
            assert!(
                !args.iter().any(|arg| arg == "--resume"),
                "组合 env={env:?} × permission={permission_mode:?} 无 --resume"
            );
            assert_eq!(
                args,
                legacy_args("任务", env, permission_mode),
                "None 组装与既有基线逐项相等（组合 env={env:?} × permission={permission_mode:?}）"
            );
        }
    }
}

#[test]
fn none时与改动前基线序列完全一致向后兼容回归() {
    // AC-6 第三分句：组装层对 None 与「无 resume 字段的旧 params」不可区分——
    // 九种档位组合下逐项等于演进前组装序列
    for env in [AgentEnvMode::Default, AgentEnvMode::Bare] {
        for permission_mode in [
            AgentPermissionMode::Default,
            AgentPermissionMode::AcceptEdits,
            AgentPermissionMode::BypassPermissions,
        ] {
            assert_eq!(
                build_args(&params("回归", env, permission_mode)),
                legacy_args("回归", env, permission_mode),
                "组合 env={env:?} × permission={permission_mode:?} 与改动前行为完全一致"
            );
        }
    }
}

#[test]
fn resume为空串仍组装resume与空arg单参数() {
    let args = build_args(&params_with_resume(
        "任务",
        AgentEnvMode::Default,
        AgentPermissionMode::Default,
        Some(""),
    ));

    // 组装层不把关（与空 prompt 同哲学）：--resume 后紧跟空串 arg
    assert_eq!(
        &args[args.len() - 2..],
        ["--resume", ""],
        "空串 resume 仍组装为 --resume + 空 arg 两项，实际: {args:?}"
    );
}

#[test]
fn resume含空格引号换行emoji超长时原样保留单个arg不拆分() {
    let session_id = "s 1 \"quoted\"\nnext 🎉 中文";
    let long = format!("{session_id}{}", "x".repeat(1000));
    let args = build_args(&params_with_resume(
        "任务",
        AgentEnvMode::Default,
        AgentPermissionMode::Default,
        Some(&long),
    ));

    assert_eq!(
        args.last().map(String::as_str),
        Some(long.as_str()),
        "resume id 原样保留为单个 arg，不被空白/特殊字符拆分或截断"
    );
    assert_eq!(
        args.iter().filter(|arg| *arg == "--resume").count(),
        1,
        "不因 id 内空白产生第二个 --resume flag"
    );
}

#[test]
fn 九种档位组合下resume恒在尾部且档位flag与相对次序不变() {
    // resume 组装与 env × permission-mode 档位正交：追加恒在尾部，
    // 前缀（档位 flag 面与相对次序）与 None 组装逐项相等
    for env in [AgentEnvMode::Default, AgentEnvMode::Bare] {
        for permission_mode in [
            AgentPermissionMode::Default,
            AgentPermissionMode::AcceptEdits,
            AgentPermissionMode::BypassPermissions,
        ] {
            let with_resume = build_args(&params_with_resume(
                "任务",
                env,
                permission_mode,
                Some("s-1"),
            ));
            let without_resume = build_args(&params("任务", env, permission_mode));

            assert_eq!(
                &with_resume[with_resume.len() - 2..],
                ["--resume", "s-1"],
                "组合 env={env:?} × permission={permission_mode:?}：--resume 恒在尾部"
            );
            assert_eq!(
                &with_resume[..with_resume.len() - 2],
                without_resume.as_slice(),
                "组合 env={env:?} × permission={permission_mode:?}：档位 flag 面与相对次序不变"
            );
        }
    }
}

#[test]
fn cwd不产生任何flag且不影响组装结果() {
    let base = AgentRunParams {
        prompt: "任务".to_owned(),
        cwd: PathBuf::from("C:\\work\\demo"),
        env: AgentEnvMode::Bare,
        permission_mode: AgentPermissionMode::BypassPermissions,
        resume_session_id: None,
    };
    let other_cwd = AgentRunParams {
        cwd: PathBuf::from("D:\\另一 个\\目录 🎉"),
        ..base.clone()
    };

    let args_a = build_args(&base);
    let args_b = build_args(&other_cwd);
    assert_eq!(
        args_a, args_b,
        "cwd 仅经 Command::current_dir 传递，不进 flag 序列"
    );
    assert!(
        !args_a
            .iter()
            .any(|arg| arg.contains("work") || arg.contains("demo")),
        "组装结果不含 cwd 片段，实际: {args_a:?}"
    );
}
