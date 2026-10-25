//! 系统脚本执行统一实现（infra 层唯一 spawn 配置点）：
//! - 壳包装：`SystemCommand::script` / `script_raw` / `script_with` 编码
//!   Windows `cmd /C`、unix `sh -c`、GitBash `bash -c`、Windows `raw_arg`
//!   保真四形态（平台壳探测见 `shell`）；
//! - 平台标志：`into_std` / `into_tokio` 统一施加（Windows `CREATE_NO_WINDOW`
//!   抑制 GUI 进程 spawn 控制台命令的窗口闪烁）——新平台在此加 cfg 臂，
//!   调用方零改动；
//! - 进程树击杀：`kill_tree`（Windows `taskkill` / unix `kill`）；
//! - 进程执行体：[`run_shell`]（超时必填 / 双管道并发排水 / 进程树击杀 / 取消
//!   四能力单点——检查域四类外部进程的唯一 spawn 收口）；
//! - 瞬态签名与有限重试：`transient`（封闭签名表 + [`RetryPolicy`]——构建产物被
//!   安全软件占用时的自愈面，字典扩展只改该模块一处；含增量推进判据（`Compiling`
//!   行计数 / 序列 / 卡死判定 [`is_progress_stalled`]））；
//! - 锁占用诊断：`lockprobe`（[`locked_paths`] / [`lock_holders`] /
//!   [`attribution_text`]——被锁文件 + 占用者的归因原料，供检查域装配环境错误
//!   文案）；
//! - 进程输出解读面：[`ProcessOutput`]（错误信息 + 末尾行双面——失败归因
//!   文本统一装配，替代各调用点自截尾部）。

mod lockprobe;
mod output;
mod run;
mod shell;
mod transient;

pub use lockprobe::{attribution_text, lock_holders, locked_paths, LockHolder};
pub use output::ProcessOutput;
pub use run::{run_shell, RunError, RunOptions, RunOutcome, RunReport};
#[cfg(windows)]
pub use shell::probe_windows_shell;
pub use shell::{resolve_shell, ShellBase};
pub use transient::{
    compiling_count, is_progress_stalled, is_transient, progress_series, AttemptTrace, RetryPolicy,
};

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use foundation::layout::ReportSlot;

/// spawn 命令规格：程序 + 逐参 argv。`raw` 为 Windows 命令串原样入线标记
/// （作用于末参，`raw_arg` 保真——仅 tokio 侧可表达；std 调用点不使用
/// `script_raw`）。`Clone` 供执行体形式复用（`cwd` / `env` 由
/// [`RunOptions`] 施加，命令规格本身与目标机无关）。
#[derive(Debug, Clone)]
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

/// spawn 产物的两种输出记录模式（输出记录职责单点驻本 crate；调用方零私有
/// 捕获 / 落盘分支）：
/// - [`ExecOutput::Transient`]：spawn 即得、即时消费（反馈边注入）、即弃——无
///   id，全量不进库、不落盘引用；
/// - [`ExecOutput::Recorded`]：输出全量落盘 change 报告目录树
///   （`<report_dir>/exec-outputs/<exec_id>.log`），`exec_id` 为回查引用
///   （`RunNodeRecord.output_ref` 载体；MUST NOT 全文进节点行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecOutput {
    /// 即弃（无 id；反馈边消费即够）
    Transient,
    /// 落盘可回查（有 id 可查）
    Recorded {
        /// 输出记录引用（[`record_exec_output`] 铸造）
        exec_id: String,
    },
}

/// exec 输出记录目录名（change 报告目录树内）。
const EXEC_OUTPUT_DIR: &str = "exec-outputs";

/// `ExecOutput::Recorded` 落盘半边（输出记录单点）：铸造 `exec_id` 并把**单一输出
/// 流**全量落盘到 change 报告目录树（`<report_dir>/exec-outputs/<exec_id>.log`，
/// 按槽位同锚派生），返回 `exec_id` 引用。单一流即完整还原终端——static-check
/// 的 `2>&1` 交错流、test-execution 的 stdout + stderr 合并流；stdout / stderr
/// 分文件（`.err`）已退役（分离快照会丢失交错顺序、与终端不一致）。零截断零改写。
pub fn record_exec_output(
    root: &Path,
    change: &str,
    slot: ReportSlot,
    output: &[u8],
) -> Result<String, String> {
    let dir = exec_output_dir(root, change, slot);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 exec 输出目录失败: {e}"))?;
    let exec_id = mint_exec_id();
    std::fs::write(dir.join(format!("{exec_id}.log")), output)
        .map_err(|e| format!("落盘 exec 输出失败: {e}"))?;
    Ok(exec_id)
}

pub fn read_exec_output(root: &Path, change: &str, exec_id: &str) -> Option<Vec<u8>> {
    read_exec_output_in(
        &foundation::layout::resolve(root).changes_root.join(change),
        exec_id,
    )
}

/// 同 change 目录双锚探测回查（test 槽先 / baseline 槽后）：`exec_id` 进程内
/// 铸造跨树唯一，首中即返、miss 仍 `None`——IPC `read_exec_output` 命令与前端
/// stdout 面板零改动即覆盖两报告树。
pub fn read_exec_output_in(change_dir: &Path, exec_id: &str) -> Option<Vec<u8>> {
    if !is_valid_exec_id(exec_id) {
        return None;
    }
    [ReportSlot::Test, ReportSlot::Baseline]
        .iter()
        .find_map(|slot| {
            let dir = exec_output_dir_in(change_dir, *slot);
            std::fs::read(dir.join(format!("{exec_id}.log"))).ok()
        })
}

/// 输出记录目录派生（与 test-execution 报告树同锚：`change_reports(root, change,
/// slot)/`——`report_dir` 与 `exec_id` 同口径、按槽位派生，见
/// desktop-checks-domain）。
fn exec_output_dir(root: &Path, change: &str, slot: ReportSlot) -> PathBuf {
    exec_output_dir_in(
        &foundation::layout::resolve(root).changes_root.join(change),
        slot,
    )
}

/// [`exec_output_dir`] 的 change 目录变体（`reports/<test|baseline>` 后缀经
/// layout 槽位单点）。
fn exec_output_dir_in(change_dir: &Path, slot: ReportSlot) -> PathBuf {
    foundation::layout::change_reports_in(change_dir, slot).join(EXEC_OUTPUT_DIR)
}

/// `exec_id` 形态校验（路径分量安全：非空且仅 ASCII 字母数字与 `-`——防
/// `..` / 分隔符类路径逃逸）。
fn is_valid_exec_id(exec_id: &str) -> bool {
    !exec_id.is_empty()
        && exec_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

/// 铸造 `exec_id`：`exec-<millis>-<进程内序号>`（同刻多次铸造仍互异；时钟早于
/// epoch 取 0，不 panic）。
fn mint_exec_id() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    format!("exec-{millis}-{seq}")
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
