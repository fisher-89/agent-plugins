//! `bootstrap` 的单元测试（test-design「bootstrap.rs -> bootstrap_test.rs」节）：
//! `run_install` 执行体真件半边（AC-5）——shell 包装 spawn、cwd = worktree、
//! 环境全继承零注入、退出态、摘要尾部 ≤300 字、拉起失败统一 Err 面。真实
//! 快命令子进程（cmd /C 与 sh -c 双平台口径，static_check 先例；平台分支互
//! 异覆盖归 CI 矩阵——单平台单测只触达本机分支）；环境变量注入以唯一命名键
//! 避免串扰、测毕恢复；拉起失败行以 PATH 隔离窗口注入（本 crate PATH 锁串
//! 行化）。900 s 到点 kill 路径见 test-design 不可测试项 3。

use std::path::Path;

use super::bootstrap;
use crate::test_path_lock as lock_path;

/// 本机 shell 的 cwd 回显命令（cmd `cd` / sh `pwd`——平台分支互异）。
fn echo_cwd_command() -> &'static str {
    if cfg!(windows) {
        "cd"
    } else {
        "pwd"
    }
}

/// 本机 shell 的环境变量回显命令串。
fn echo_env_command(var: &str) -> String {
    if cfg!(windows) {
        format!("echo %{var}%")
    } else {
        format!("echo ${var}")
    }
}

/// 非零退出 + stdout / stderr 各有输出的命令串（shell 连接符平台互异）。
fn failing_command() -> &'static str {
    if cfg!(windows) {
        "echo stdout-line & echo stderr-line 1>&2 & exit 3"
    } else {
        "echo stdout-line; echo stderr-line 1>&2; exit 3"
    }
}

fn worktree_dir(tag: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("vcs-bootstrap-{tag}-"))
        .tempdir()
        .expect("创建 worktree 临时目录失败");
    let path = dir.path().join("wt");
    std::fs::create_dir_all(&path).expect("预置 worktree 目录失败");
    (dir, path)
}

/// 成功退出：退出 0 的真实快命令（已知输出）→ `success: true`、summary 含
/// 输出尾部。
#[test]
fn 成功退出_success为true且summary含输出() {
    let _guard = lock_path();
    let (_dir, wt) = worktree_dir("ok");

    let run = bootstrap::run_install(&wt, "echo bootstrap-ok").expect("run_install 应 Ok");

    assert!(run.success, "退出 0 → success: true");
    assert!(
        run.summary.contains("bootstrap-ok"),
        "summary 含输出尾部，实际: {}",
        run.summary
    );
}

/// 非零退出：stdout + stderr 各有输出 → `success: false`、summary 含两侧输
/// 出尾部信息（失败语境所在）。
#[test]
fn 非零退出_success为false且summary含stdout与stderr() {
    let _guard = lock_path();
    let (_dir, wt) = worktree_dir("fail");

    let run = bootstrap::run_install(&wt, failing_command()).expect("run_install 应 Ok");

    assert!(!run.success, "非零退出 → success: false（非 Err 面）");
    assert!(
        run.summary.contains("stdout-line") && run.summary.contains("stderr-line"),
        "summary 含 stdout / stderr 尾部信息，实际: {}",
        run.summary
    );
}

/// cwd = worktree：命令以相对路径在 cwd 落文件 + 回显 cwd → 产物落在
/// worktree 内（AC-5「cwd = worktree」字面）。
#[test]
fn cwd恒worktree_相对路径产物落worktree内() {
    let _guard = lock_path();
    let (_dir, wt) = worktree_dir("cwd");

    let run = bootstrap::run_install(&wt, "echo probe > cwd-probe.txt").expect("run_install 应 Ok");
    assert!(run.success, "重定向命令退出 0");

    // 相对路径产物落在 worktree 内（cwd 字面证明——不依赖回显路径的 8.3 / 盘
    // 符形态差异）
    assert!(
        wt.join("cwd-probe.txt").is_file(),
        "相对路径产物落 worktree 内（cwd = worktree）"
    );

    // 回显 cwd：输出非空且即 shell 的当前目录回显（次级面；主证为上）
    let run = bootstrap::run_install(&wt, echo_cwd_command()).expect("run_install 应 Ok");
    assert!(
        !run.summary.trim().is_empty(),
        "cwd 回显非空，实际: {}",
        run.summary
    );
    assert!(
        paths_equivalent(run.summary.trim(), &wt),
        "回显 cwd = worktree 绝对路径（大小写 / 分隔符归一比对），实际: {}",
        run.summary
    );
}

/// 环境全继承：测试注入唯一命名临时环境变量 → 子进程回显可见该值（零注入
/// = 全继承证明；变量测毕恢复）。
#[test]
fn 环境全继承_注入变量子进程可见() {
    let _guard = lock_path();
    let (_dir, wt) = worktree_dir("env");

    let var = format!("VCS_BOOTSTRAP_INHERIT_{}", std::process::id());
    let marker = "inherit-probe-7f3a";
    std::env::set_var(&var, marker);

    let run = bootstrap::run_install(&wt, &echo_env_command(&var));

    std::env::remove_var(&var); // 测毕恢复（断言失败亦经 unwrap 前恢复）

    let run = run.expect("run_install 应 Ok");
    assert!(run.success, "回显命令退出 0");
    assert!(
        run.summary.contains(marker),
        "注入变量对子进程可见（环境全继承零注入），实际: {}",
        run.summary
    );
}

/// 摘要尾部截断：长输出（>300 字）→ summary `chars().count() ≤ 300` 且取自
/// 尾部（尾部标记在场、头部标记截去）；短输出全量保留。
#[test]
fn 摘要尾部截断_超300字取尾且短输出全量() {
    let _guard = lock_path();
    let (_dir, wt) = worktree_dir("tail");

    // 长输出：HEAD 标记 + 400 填充 + TAIL 标记（尾部是失败语境所在）
    let long = format!("HEAD-MARK{}TAIL-MARK", "x".repeat(400));
    let run = bootstrap::run_install(
        &wt,
        // shell 内单引号 / 双引号包裹（cmd 与 sh 均取双引号；字串无特殊字符）
        &format!("echo \"{long}\""),
    )
    .expect("run_install 应 Ok");

    assert!(
        run.summary.chars().count() <= 300,
        "summary ≤ 300 字，实际 {} 字: {}",
        run.summary.chars().count(),
        run.summary
    );
    assert!(
        run.summary.contains("TAIL-MARK") && !run.summary.contains("HEAD-MARK"),
        "超限取自尾部（尾部标记在、头部标记截去），实际: {}",
        run.summary
    );

    // 短输出全量保留（trim 尾换行，其余零丢失）
    let run = bootstrap::run_install(&wt, "echo short-ok").expect("run_install 应 Ok");
    assert_eq!(run.summary, "short-ok", "短输出全量保留（仅尾换行修剪）");
}

/// shell 包装语义：命令串含 shell 语法（连接符 / 重定向）可执行成功（cmd /C
/// 与 sh -c 双平台口径，static_check 先例）。
#[test]
fn shell包装语义_连接符与重定向可用() {
    let _guard = lock_path();
    let (_dir, wt) = worktree_dir("shell");

    // 重定向：echo > 文件（相对路径经 cwd 落 worktree）
    let run = bootstrap::run_install(&wt, "echo first > shell-probe.txt").expect("应 Ok");
    assert!(run.success, "重定向语法可执行");
    assert!(wt.join("shell-probe.txt").is_file(), "重定向产物在案");

    // 连接符：两条命令顺序执行（& / ; 平台互异由 failing_command 同款口径）
    let chained = if cfg!(windows) {
        "echo one & echo two"
    } else {
        "echo one; echo two"
    };
    let run = bootstrap::run_install(&wt, chained).expect("应 Ok");
    assert!(run.success, "连接符语法可执行");
    assert!(
        run.summary.contains("one") && run.summary.contains("two"),
        "两条命令均执行（stdout 连接面），实际: {}",
        run.summary
    );
}

/// 拉起失败（Unix 半边）：shell 本体不可 spawn（PATH 隔离窗口）→ `Err` 显式
///（拉起 / 超时统一 Err 面——D5「依赖引导未执行成功」的 error 来源）；测毕
/// 恢复 PATH。
#[cfg(unix)]
#[test]
fn 拉起失败_shell不可spawn统一err面() {
    let _guard = lock_path();
    let (_dir, wt) = worktree_dir("spawn-fail");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let result = bootstrap::run_install(&wt, "echo unreachable");
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let error = result.expect_err("shell 拉起失败应 Err");
    assert!(
        error.contains("拉起失败") && error.contains("echo unreachable"),
        "Err 显式记因拉起失败并携带命令串，实际: {error}"
    );
}

/// 拉起失败（Windows 半边）：CreateProcess 对 `cmd.exe` 有系统目录解析回退
///（PATH 隔离不可剥夺 shell 本体——Err 拉起面在本平台不可经公共 API 触达，
/// 跨平台行归 CI 矩阵，test-design 平台互异注记）；可达的「命令本体不可达」
/// 收敛 `Ok(success: false)` + stderr 入摘要——D5 依赖引导失败警告面的输入
/// 形态（create 侧「依赖引导失败（{command}）: {summary}」之源）。
#[cfg(windows)]
#[test]
fn 拉起失败_windows面_shell恒可spawn不可达名收敛success_false() {
    let _guard = lock_path();
    let (_dir, wt) = worktree_dir("spawn-fail-win");

    let run = bootstrap::run_install(&wt, "definitely-not-a-command-xyz-42").expect("应 Ok");

    assert!(
        !run.success,
        "不可达命令名经 cmd 解析失败 → success: false（非 panic 非 Err）"
    );
    assert!(
        !run.summary.trim().is_empty(),
        "解析失败 stderr 入摘要（失败语境），实际: {}",
        run.summary
    );
}

/// 回显 cwd 与 worktree 路径的归一比对（大小写不敏感；Windows 8.3 短名与盘
/// 符形态差异经 canonicalize 双侧归一）。
fn paths_equivalent(echoed: &str, worktree: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (
        std::fs::canonicalize(echoed),
        std::fs::canonicalize(worktree),
    ) {
        return a == b;
    }
    // canonicalize 不可用（形态差异不可归一）时退回大小写不敏感字符串比对
    echoed.to_lowercase() == worktree.to_string_lossy().to_lowercase()
}
