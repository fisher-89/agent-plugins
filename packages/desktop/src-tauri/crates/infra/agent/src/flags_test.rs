//! `flags` 的单元测试（AC-2 / AC-6）：`build_args` 纯函数的组装口径——
//! `-p` + stream-json + verbose 恒有、env/permission-mode 映射、prompt 保真、
//! cwd 不产生 flag、AC-6 禁用 flag 全组合负向断言。无外部依赖，不需要 Mock。

use std::path::PathBuf;

use agent::{AgentEnvMode, AgentPermissionMode, AgentRunParams};

use crate::flags::build_args;

fn params(prompt: &str, env: AgentEnvMode, permission_mode: AgentPermissionMode) -> AgentRunParams {
    AgentRunParams {
        prompt: prompt.to_owned(),
        cwd: PathBuf::from("C:\\work\\demo"),
        env,
        permission_mode,
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
fn 全组合九种档位组装均不含禁用flag() {
    // AC-6：无 --resume / --include-partial-messages / model flag（MVP 边界）
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

#[test]
fn cwd不产生任何flag且不影响组装结果() {
    let base = AgentRunParams {
        prompt: "任务".to_owned(),
        cwd: PathBuf::from("C:\\work\\demo"),
        env: AgentEnvMode::Bare,
        permission_mode: AgentPermissionMode::BypassPermissions,
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
