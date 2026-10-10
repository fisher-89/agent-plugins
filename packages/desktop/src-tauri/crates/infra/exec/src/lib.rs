//! 系统脚本执行统一实现（infra 层唯一 spawn 配置点）：
//! - 壳包装：`SystemCommand::script` / `script_raw` / `script_with` 编码
//!   Windows `cmd /C`、unix `sh -c`、GitBash `bash -c`、Windows `raw_arg`
//!   保真四形态（平台壳探测见 `shell`）；
//! - 平台标志：`into_std` / `into_tokio` 统一施加（Windows `CREATE_NO_WINDOW`
//!   抑制 GUI 进程 spawn 控制台命令的窗口闪烁）——新平台在此加 cfg 臂，
//!   调用方零改动；
//! - 进程树击杀：`kill_tree`（Windows `taskkill` / unix `kill`）。

mod shell;

pub use shell::{resolve_shell, ShellBase};
#[cfg(windows)]
pub use shell::probe_windows_shell;

use std::ffi::{OsStr, OsString};

/// spawn 命令规格：程序 + 逐参 argv。`raw` 为 Windows 命令串原样入线标记
/// （作用于末参，`raw_arg` 保真——仅 tokio 侧可表达；std 调用点不使用
/// `script_raw`）。
#[derive(Debug)]
pub struct SystemCommand {
    program: OsString,
    args: Vec<OsString>,
    raw: bool,
}

impl SystemCommand {
    /// 裸程序直 spawn（git 等 argv 直传，零 shell 包装）。
    pub fn bare(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            raw: false,
        }
    }

    /// 平台默认壳包装脚本行：Windows `cmd /C`、其余 `sh -c`。
    pub fn script(script: &str) -> Self {
        #[cfg(windows)]
        {
            Self::bare("cmd").arg("/C").arg(script)
        }
        #[cfg(not(windows))]
        {
            Self::bare("sh").arg("-c").arg(script)
        }
    }

    /// 平台默认壳 + Windows 命令串原样入线（`raw_arg` 保真——脚本行是 shell
    /// 行而非单 argv，`arg` 的 MSVC 转引会把模板内引号写成 `\"`，cmd 解析为
    /// 反斜杠附着路径；注册表模板的 `> "{results_file}"` 重定向与
    /// `--outputFile="{results_file}"` 参数依赖此语义）。unix 臂与 `script`
    /// 等价。
    pub fn script_raw(script: &str) -> Self {
        #[cfg(windows)]
        {
            let mut command = Self::bare("cmd").arg("/C").arg(script);
            command.raw = true;
            command
        }
        #[cfg(not(windows))]
        {
            Self::script(script)
        }
    }

    /// 指定壳包装脚本行（`resolve_shell` 探测产物直用）：GitBash → `bash -c`、
    /// Cmd → `cmd /C`、Sh → `sh -c`。
    pub fn script_with(shell: &ShellBase, script: &str) -> Self {
        match shell {
            ShellBase::GitBash(bash) => Self::bare(bash).arg("-c").arg(script),
            ShellBase::Cmd => Self::bare("cmd").arg("/C").arg(script),
            #[cfg(not(windows))]
            ShellBase::Sh => Self::bare("sh").arg("-c").arg(script),
        }
    }

    /// 追加单个 argv。
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// 批量追加 argv。
    pub fn args<I>(mut self, args: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    /// 物化为同步 `std::process::Command`：argv 逐参入线 + 平台标志统一施加
    /// （Windows `CREATE_NO_WINDOW`）。
    pub fn into_std(self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.program);
        command.args(&self.args);
        #[cfg(windows)]
        {
            // GUI 进程 spawn 控制台命令须抑制窗口闪烁（CREATE_NO_WINDOW = 0x08000000）
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }

    /// 物化为 `tokio::process::Command`：argv 逐参入线（`raw` 时末参经
    /// `raw_arg` 原样入线）+ 平台标志统一施加（Windows `CREATE_NO_WINDOW`）。
    pub fn into_tokio(self) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(&self.program);
        #[cfg(windows)]
        {
            if self.raw {
                let split = self.args.len().saturating_sub(1);
                command.args(&self.args[..split]);
                if let Some(last) = self.args.last() {
                    command.raw_arg(last);
                }
            } else {
                command.args(&self.args);
            }
        }
        #[cfg(not(windows))]
        {
            command.args(&self.args);
        }
        #[cfg(windows)]
        {
            // tokio::process::Command 自带 creation_flags（cfg(windows) 固有方法）
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }
}

/// 进程树击杀（尽力语义，静默不重试不阻塞收敛）：Windows `taskkill /PID
/// <pid> /T /F`，unix `kill -9`。
pub fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    #[cfg(windows)]
    {
        let _ = SystemCommand::bare("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .into_std()
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = SystemCommand::bare("kill")
            .args(["-9", &pid.to_string()])
            .into_std()
            .output();
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod lib_test;

#[cfg(all(test, windows))]
#[path = "shell_test.rs"]
mod shell_test;
