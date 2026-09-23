//! CLI 发现：PATH 目录扫描定位可执行入口。
//!
//! [`discover_in`] 为纯函数（入参目录列表，可用合成目录单测、不依赖真实
//! CLI）；[`discover`] 为读 `PATH` 环境变量的薄包装。Windows 上 `claude`
//! 通常为 npm 的 `.cmd` shim（不可直接 spawn，须经 `cmd /C` 包装——包装
//! 在 spawn 侧处理），按 `.cmd` / `.bat` / `.exe` 优先序探测；其余平台
//! 单一裸名。

use std::path::{Path, PathBuf};

use agent::AgentStartError;

/// Windows 上按优先序探测的候选文件名（`.cmd` shim 最常见，最先探测）。
const WINDOWS_CANDIDATES: [&str; 3] = ["claude.cmd", "claude.bat", "claude.exe"];

/// 其余平台的候选裸名。
const DEFAULT_CANDIDATE: &str = "claude";

/// CLI 缺失的显式错误文案（MUST NOT 静默空转）。
const CLI_MISSING: &str = "PATH 上未发现 claude 可执行入口";

/// 在给定目录列表内按平台优先序发现 CLI 入口：命中返回完整路径，全部
/// miss 返回 `AgentStartError::CliMissing`。
pub fn discover_in<I, D>(dirs: I) -> Result<PathBuf, AgentStartError>
where
    I: IntoIterator<Item = D>,
    D: AsRef<Path>,
{
    let candidates: &[&str] = if cfg!(windows) {
        &WINDOWS_CANDIDATES
    } else {
        &[DEFAULT_CANDIDATE]
    };
    for dir in dirs {
        for candidate in candidates {
            let path = dir.as_ref().join(candidate);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    Err(AgentStartError::CliMissing(CLI_MISSING.to_owned()))
}

/// 读 `PATH` 环境变量并委托 [`discover_in`]；`PATH` 缺失视同 CLI 缺失。
pub fn discover() -> Result<PathBuf, AgentStartError> {
    let dirs = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<PathBuf>>())
        .unwrap_or_default();
    discover_in(dirs)
}
