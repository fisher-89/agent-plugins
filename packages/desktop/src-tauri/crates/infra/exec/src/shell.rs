use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// shell 底座探测产物（执行命令的组装形态）。
pub enum ShellBase {
    /// git-bash 可执行文件全路径（`bash -c`）
    GitBash(PathBuf),
    /// Windows 内置兜底（`cmd /C`）
    Cmd,
    /// unix 底座（`sh -c`）
    #[cfg(not(windows))]
    Sh,
}

/// shell 底座解析：Windows 探测 git-bash（unix 语法成功率最高），未命中退
/// `cmd /C` 兜底；unix `sh -c`。
pub fn resolve_shell() -> ShellBase {
    #[cfg(windows)]
    {
        match std::env::var_os("PATH") {
            Some(path_var) => probe_windows_shell(&path_var),
            None => ShellBase::Cmd,
        }
    }
    #[cfg(not(windows))]
    {
        ShellBase::Sh
    }
}

/// Windows shell 底座纯探测：扫 `path_var` 的 PATH 条目，命中
/// `Git\bin\bash.exe` / `Git\usr\bin\bash.exe` 形态即返 `GitBash`（System32
/// 的 WSL bash 同名不在 Git 形态段内，天然排除）；未命中退 `Cmd`。纯函数
/// （fabricated PATH 可测，无进程 spawn）。
#[cfg(windows)]
pub fn probe_windows_shell(path_var: &OsStr) -> ShellBase {
    for dir in std::env::split_paths(path_var) {
        for candidate in git_bash_candidates(&dir) {
            if candidate.is_file() {
                return ShellBase::GitBash(candidate);
            }
        }
    }
    ShellBase::Cmd
}

/// 单个 PATH 条目的 git-bash 候选
#[cfg(windows)]
fn git_bash_candidates(dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let segment = dir.to_string_lossy().replace('/', "\\").to_lowercase();
    if segment.ends_with("\\git\\bin") || segment.ends_with("\\git\\usr\\bin") {
        candidates.push(dir.join("bash.exe"));
    }
    // `Git\cmd` 条目 → bash 落在兄弟 `bin` / `usr\bin`；`Git` 根条目 → 落在
    // 子 `bin` / `usr\bin`。两者统一为「Git 根目录」再拼子段。
    let git_root = if segment.ends_with("\\git\\cmd") {
        dir.parent().map(Path::to_path_buf)
    } else if segment.ends_with("\\git") {
        Some(dir.to_path_buf())
    } else {
        None
    };
    if let Some(root) = git_root {
        candidates.push(root.join("bin\\bash.exe"));
        candidates.push(root.join("usr\\bin\\bash.exe"));
    }
    candidates.push(dir.join("Git\\bin\\bash.exe"));
    candidates.push(dir.join("Git\\usr\\bin\\bash.exe"));
    candidates
}
