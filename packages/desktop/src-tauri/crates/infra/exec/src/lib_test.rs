//! SystemCommand 组装形状测试：经 `into_std` 的 get_program / get_args
//! 断言壳包装与 argv 形态（无进程 spawn；`creation_flags` 进程级不可观测，
//! 由 release 手工验收覆盖）。

use std::ffi::OsStr;
use std::path::PathBuf;

use crate::{ShellBase, SystemCommand};

#[test]
fn script按平台默认壳组装() {
    let command = SystemCommand::script("echo hi").into_std();
    #[cfg(windows)]
    {
        assert_eq!(command.get_program(), OsStr::new("cmd"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [OsStr::new("/C"), OsStr::new("echo hi")]
        );
    }
    #[cfg(not(windows))]
    {
        assert_eq!(command.get_program(), OsStr::new("sh"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [OsStr::new("-c"), OsStr::new("echo hi")]
        );
    }
}

#[test]
fn script_raw形态与script同程序且末参原位() {
    // raw 标记仅 tokio 侧（raw_arg）表达，std 侧 argv 形状与 script 一致
    let command = SystemCommand::script_raw("echo hi").into_std();
    #[cfg(windows)]
    assert_eq!(command.get_program(), OsStr::new("cmd"));
    #[cfg(not(windows))]
    assert_eq!(command.get_program(), OsStr::new("sh"));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [OsStr::new(if cfg!(windows) { "/C" } else { "-c" }), OsStr::new("echo hi")]
    );
}

#[test]
fn script_with按底座组装() {
    let bash = PathBuf::from("C:\\Git\\bin\\bash.exe");
    let gitbash = SystemCommand::script_with(&ShellBase::GitBash(bash.clone()), "echo hi").into_std();
    assert_eq!(gitbash.get_program(), OsStr::new(&bash));
    assert_eq!(
        gitbash.get_args().collect::<Vec<_>>(),
        [OsStr::new("-c"), OsStr::new("echo hi")]
    );

    let cmd = SystemCommand::script_with(&ShellBase::Cmd, "echo hi").into_std();
    assert_eq!(cmd.get_program(), OsStr::new("cmd"));
    assert_eq!(
        cmd.get_args().collect::<Vec<_>>(),
        [OsStr::new("/C"), OsStr::new("echo hi")]
    );

    #[cfg(not(windows))]
    {
        let sh = SystemCommand::script_with(&ShellBase::Sh, "echo hi").into_std();
        assert_eq!(sh.get_program(), OsStr::new("sh"));
        assert_eq!(
            sh.get_args().collect::<Vec<_>>(),
            [OsStr::new("-c"), OsStr::new("echo hi")]
        );
    }
}

#[test]
fn bare逐参直传零壳包装() {
    let command = SystemCommand::bare("git")
        .arg("-C")
        .arg("/root")
        .args(["status", "--porcelain"])
        .into_std();
    assert_eq!(command.get_program(), OsStr::new("git"));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            OsStr::new("-C"),
            OsStr::new("/root"),
            OsStr::new("status"),
            OsStr::new("--porcelain")
        ]
    );
}
