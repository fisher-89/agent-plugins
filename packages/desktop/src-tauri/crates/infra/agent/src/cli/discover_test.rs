//! `discover` 的单元测试（AC-2 / D14）：`discover_in` 纯函数按平台优先序在
//! 合成目录内发现 CLI 入口；`discover` 薄包装读 PATH（env 修改以互斥锁串行
//! 化保护并恢复）。文件系统候选以 tempfile 真开合成目录，不依赖真实 CLI。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use agent::AgentStartError;

use crate::discover::{discover, discover_in};

/// PATH 环境变量修改串行化（同进程测试并行跑，set_var 为进程全局操作）。
static PATH_LOCK: Mutex<()> = Mutex::new(());

/// 当前平台应探测的候选文件名（与 discover_in 的平台分支对齐）。
const PLATFORM_CANDIDATES: [&str; 3] = if cfg!(windows) {
    ["claude.cmd", "claude.bat", "claude.exe"]
} else {
    ["claude", "claude", "claude"]
};

/// 合成目录：TempDir 由测试持有保活（drop 即删除，不得先行归还路径）。
fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("discover-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

fn write_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"").expect("写伪候选文件失败");
    path
}

// ---------------------------------------------------------------------------
// discover_in：正向与优先序
// ---------------------------------------------------------------------------

#[test]
fn 合成目录含平台首选候选时发现并返回该路径() {
    let dir = tempdir("hit");
    let candidate = PLATFORM_CANDIDATES[0];
    let expected = write_file(dir.path(), candidate);

    let found = discover_in([dir.path()]).expect("应发现候选");
    assert_eq!(found, expected, "返回完整路径");
}

#[cfg(windows)]
#[test]
fn 同目录三候选并存时按cmd_bat_exe优先序取cmd() {
    let dir = tempdir("priority");
    let cmd = write_file(dir.path(), "claude.cmd");
    write_file(dir.path(), "claude.bat");
    write_file(dir.path(), "claude.exe");

    let found = discover_in([dir.path()]).expect("应发现候选");
    assert_eq!(found, cmd, "D14：.cmd > .bat > .exe 优先序");
}

#[cfg(windows)]
#[test]
fn 仅bat或仅exe时依序降级发现对应文件() {
    // 仅 .bat
    let bat_dir = tempdir("only-bat");
    let bat = write_file(bat_dir.path(), "claude.bat");
    assert_eq!(discover_in([bat_dir.path()]).expect("应发现 .bat"), bat);

    // 仅 .exe
    let exe_dir = tempdir("only-exe");
    let exe = write_file(exe_dir.path(), "claude.exe");
    assert_eq!(discover_in([exe_dir.path()]).expect("应发现 .exe"), exe);
}

#[test]
fn 多目录各含候选时先声明目录优先() {
    let first = tempdir("first-dir");
    let second = tempdir("second-dir");
    let expected = write_file(first.path(), PLATFORM_CANDIDATES[0]);
    write_file(second.path(), PLATFORM_CANDIDATES[0]);

    let found = discover_in([first.path(), second.path()]).expect("应发现候选");
    assert_eq!(found, expected, "先声明目录优先");
}

// ---------------------------------------------------------------------------
// discover_in：边界与异常
// ---------------------------------------------------------------------------

#[test]
fn 目录列表含不存在目录时跳过不panic() {
    let dir = tempdir("skip-missing");
    let expected = write_file(dir.path(), PLATFORM_CANDIDATES[0]);
    let missing = dir.path().join("no-such-dir");

    let found = discover_in([&missing, dir.path()]).expect("失效 PATH 项被跳过后仍可发现");
    assert_eq!(found, expected);
}

#[test]
fn 目录列表为空或全部无候选时返回cli_missing且display含检索线索() {
    // 空目录列表
    let empty: Vec<PathBuf> = Vec::new();
    let err = discover_in(empty).expect_err("空列表必须显式失败");
    assert!(
        matches!(err, AgentStartError::CliMissing(_)),
        "变体为 CliMissing，实际: {err:?}"
    );

    // 目录存在但无任何候选（只有无关文件）
    let junk_dir = tempdir("junk");
    write_file(junk_dir.path(), "claude-shim.txt");
    let err = discover_in([junk_dir.path()]).expect_err("无候选必须显式失败");
    assert!(matches!(err, AgentStartError::CliMissing(_)));

    let text = err.to_string();
    assert!(
        text.contains("未发现"),
        "Display 携带检索线索（MUST NOT 静默空转），实际: {text}"
    );
}

// ---------------------------------------------------------------------------
// discover：读 PATH 薄包装
// ---------------------------------------------------------------------------

#[test]
fn 以合成path薄包装可发现伪cli且恢复原path() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let dir = tempdir("path-wrap");
    let candidate = PLATFORM_CANDIDATES[0];
    let expected = write_file(dir.path(), candidate);

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", dir.path());
    let result = discover();
    // 先恢复再断言：断言失败也不遗留污染的 PATH
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let found = result.expect("合成 PATH 应发现伪 CLI");
    assert_eq!(found, expected);
}

#[test]
fn path缺失视同cli缺失() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let original = std::env::var_os("PATH");
    std::env::remove_var("PATH");
    let result = discover();
    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    assert!(
        matches!(result, Err(AgentStartError::CliMissing(_))),
        "PATH 缺失视同 CLI 缺失，实际: {result:?}"
    );
}
