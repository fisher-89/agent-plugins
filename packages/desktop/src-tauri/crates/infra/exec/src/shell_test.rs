//! probe_windows_shell 纯函数直测（自 agent-runtime sdk/bash_test.rs 迁入）：
//! fabricated PATH 探测 git-bash 形态，无进程参与。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::{probe_windows_shell, ShellBase};

#[cfg(windows)]
fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("exec-shell-test-{tag}-"))
        .tempdir()
        .expect("创建合成目录失败")
}

/// 在 base 下落一个真实占位文件（探测仅 `is_file` 判定，零内容即可）。
#[cfg(windows)]
fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("占位路径应有父目录")).expect("创建父目录失败");
    std::fs::write(path, b"").expect("落占位文件失败");
}

#[cfg(windows)]
#[test]
fn probe命中git_bin形态条目返回gitbash全路径() {
    let dir = tempdir("probe-git-bin");
    let bash = dir.path().join("Git\\bin\\bash.exe");
    touch(&bash);

    let base = probe_windows_shell(OsStr::new(dir.path().as_os_str()));

    match base {
        ShellBase::GitBash(found) => assert_eq!(
            found, bash,
            "命中 <条目>\\Git\\bin\\bash.exe 形态，返回全路径"
        ),
        _ => panic!("Git\\bin 形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }
}

#[cfg(windows)]
#[test]
fn probe命中git_usr_bin形态条目且条目本身为git段时直接补bash_exe() {
    // 形态一：<条目>\Git\usr\bin\bash.exe
    let dir = tempdir("probe-git-usr-bin");
    let bash = dir.path().join("Git\\usr\\bin\\bash.exe");
    touch(&bash);
    match probe_windows_shell(OsStr::new(dir.path().as_os_str())) {
        ShellBase::GitBash(found) => assert_eq!(found, bash, "Git\\usr\\bin 形态同判"),
        _ => panic!("Git\\usr\\bin 形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }

    // 形态二：PATH 条目本身已是 …\Git\bin 段 → 直接补 <条目>\bash.exe
    let segment_root = tempdir("probe-segment");
    let segment_bash = segment_root.path().join("Git\\bin\\bash.exe");
    touch(&segment_bash);
    let segment = segment_root.path().join("Git\\bin");
    match probe_windows_shell(segment.as_os_str()) {
        ShellBase::GitBash(found) => assert_eq!(
            found, segment_bash,
            "条目本身为 Git\\bin 段时补 <条目>\\bash.exe 命中"
        ),
        _ => panic!("段形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }
}

#[cfg(windows)]
#[test]
fn probe命中git_cmd标准安装布局返回兄弟bin的bash全路径() {
    // 安装器标准布局：PATH 只写 `…\Git\cmd`，bash 落在兄弟 `bin\bash.exe`
    let dir = tempdir("probe-git-cmd");
    let bash = dir.path().join("Git\\bin\\bash.exe");
    touch(&bash);
    let entry = dir.path().join("Git\\cmd");

    match probe_windows_shell(entry.as_os_str()) {
        ShellBase::GitBash(found) => assert_eq!(
            found, bash,
            "Git\\cmd 条目命中兄弟 bin\\bash.exe（标准安装布局不退 Cmd）"
        ),
        _ => panic!("Git\\cmd 形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }

    // usr\bin 兄弟布局同判
    let dir2 = tempdir("probe-git-cmd-usr");
    let bash2 = dir2.path().join("Git\\usr\\bin\\bash.exe");
    touch(&bash2);
    let entry2 = dir2.path().join("Git\\cmd");
    match probe_windows_shell(entry2.as_os_str()) {
        ShellBase::GitBash(found) => {
            assert_eq!(found, bash2, "Git\\cmd 条目命中兄弟 usr\\bin\\bash.exe")
        }
        _ => panic!("Git\\cmd + usr\\bin 形态必须命中 GitBash"),
    }
}

#[cfg(windows)]
#[test]
fn probe命中git根条目返回子bin的bash全路径() {
    // PATH 直接写 Git 根：bash 落在子 `bin\bash.exe`
    let dir = tempdir("probe-git-root");
    let bash = dir.path().join("Git\\bin\\bash.exe");
    touch(&bash);
    let entry = dir.path().join("Git");

    match probe_windows_shell(entry.as_os_str()) {
        ShellBase::GitBash(found) => assert_eq!(found, bash, "Git 根条目命中子 bin\\bash.exe"),
        _ => panic!("Git 根形态必须命中 GitBash（实际退了 Cmd 兜底）"),
    }
}

#[cfg(windows)]
#[test]
fn probe排除system32同名误中_仅wsl形态条目时退cmd() {
    // 同名误中形态：条目顶层放 bash.exe（System32\bash.exe 的 WSL 同名语义，
    // 不在 Git 形态段内）→ 不误中
    let dir = tempdir("probe-system32");
    touch(&dir.path().join("bash.exe"));

    let base = probe_windows_shell(OsStr::new(dir.path().as_os_str()));

    assert!(
        matches!(base, ShellBase::Cmd),
        "System32 WSL bash 同名不命中，退 Cmd 兜底"
    );
}

#[cfg(windows)]
#[test]
fn probe无任何bash条目退cmd() {
    let dir = tempdir("probe-empty");

    let base = probe_windows_shell(OsStr::new(dir.path().as_os_str()));

    assert!(matches!(base, ShellBase::Cmd), "无 bash 条目退 Cmd");
}

#[cfg(windows)]
#[test]
fn shell_base枚举三变体调试形态可辨() {
    // GitBash / Cmd（Sh 为 unix 臂变体）形状可辨，探测产物按底座组装命令
    let cmd_base: ShellBase = ShellBase::Cmd;
    assert!(matches!(cmd_base, ShellBase::Cmd));
    let git_base: ShellBase = ShellBase::GitBash(PathBuf::from("C:\\Git\\bin\\bash.exe"));
    assert!(matches!(git_base, ShellBase::GitBash(_)));
}
