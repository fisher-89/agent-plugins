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

mod job;
mod lockprobe;
mod output;
mod run;
mod shell;
mod transient;

use job::JobTree;
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
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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

/// 命令 sidecar 后缀（与 [`EXEC_OUTPUT_DIR`] 同族、同目录同 id 前缀——`.log`
/// 与 `.cmd` 一一对应，派生单点在 [`exec_command_path_in`]）。
const EXEC_COMMAND_SUFFIX: &str = "cmd";

/// 实时输出落盘口（`RunOptions` 可选面；两路 drain 共用同一实例）。
///
/// 构造面 `pub(crate)`：唯一生产路径是 [`LiveExec::sink`]，不为尚未存在的消费者
/// 开公开构造。投递面按「一批就绪字节」粒度，`n` 为零不投递。
// `Arc<dyn Fn(&[u8]) + Send + Sync>` 是 newtype 的实质字段：抽类型别名只会把签名
// 摊成两处（本仓零 second-name 取向），故就地放行该 lint
#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct LiveSink(Arc<dyn Fn(&[u8]) + Send + Sync>);

impl LiveSink {
    /// 构造（crate 内唯一入口：`LiveExec::sink`）。
    pub(crate) fn new(append: impl Fn(&[u8]) + Send + Sync + 'static) -> Self {
        Self(Arc::new(append))
    }

    /// 投递一批字节给观察者。
    pub(crate) fn absorb(&self, bytes: &[u8]) {
        (self.0)(bytes);
    }
}

/// 手写 `Debug`：`Arc<dyn Fn>` 不实现 `Debug`，而 `RunOptions` 派生了 `Debug`。
impl std::fmt::Debug for LiveSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LiveSink(..)")
    }
}

/// 运行中可回查的输出记录：先建文件 → 边跑边追加 → 收口用**同一 id** 覆盖为权威
/// 内容。磁盘面与 [`record_exec_output`] 同锚同口径（`<report_dir>/exec-outputs/`）。
///
/// `writer` 的 `Some` / `None` 即「可追加 / 失效或已收口」单一状态位：追加写失败
/// 置 `None`（观测面不打断检查主流程），`commit` 亦先 `take` 句柄再覆盖落盘。
pub struct LiveExec {
    exec_id: String,
    log_path: PathBuf,
    writer: Arc<Mutex<Option<std::fs::File>>>,
}

impl LiveExec {
    /// 前置铸造：`create_dir_all` 输出目录 + 建空 `<exec_id>.log`（追加模式持句柄）。
    ///
    /// **文件与父目录必须先于 `exec_id` 对外可见**——watch 订阅可生效的前提（父目录
    /// 缺失会显式失败，见 `desktop-file-watch`）。创建失败 `Err`。
    pub fn begin(root: &Path, change: &str, slot: ReportSlot) -> Result<Self, String> {
        let dir = exec_output_dir(root, change, slot);
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建 exec 输出目录失败: {e}"))?;
        let exec_id = mint_exec_id();
        let log_path = dir.join(format!("{exec_id}.log"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("创建 exec 输出文件失败: {e}"))?;
        Ok(Self {
            exec_id,
            log_path,
            writer: Arc::new(Mutex::new(Some(file))),
        })
    }

    /// 铸造值只读（`commit` 返回同值——同 id 收口的物理载体）。
    pub fn exec_id(&self) -> &str {
        &self.exec_id
    }

    /// 布局派生 `.log` 绝对路径（与 [`exec_output_log_path_in`] 同值）。
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// 追加口（交 `RunOptions.live`）：内部 `Mutex` 支撑两路 drain 并发写，每次
    /// `write_all` 后立即 `flush`（同一 `File` 句柄、微秒级）。写失败即置失效
    /// （此后本闭包与后续 `absorb` 全为空操作）——**MUST NOT** 上抛 / panic。
    pub fn sink(&self) -> LiveSink {
        let writer = Arc::clone(&self.writer);
        LiveSink::new(move |bytes| {
            let mut guard = writer.lock().expect("live 写入锁不可中毒");
            let Some(file) = guard.as_mut() else {
                return;
            };
            if file.write_all(bytes).and_then(|()| file.flush()).is_err() {
                *guard = None;
            }
        })
    }

    /// 命令 sidecar（`<exec_id>.cmd`）的**原样追加**：零换行 / 零格式化 / 零改写
    /// ——分段与换行的组装责任在调用方（static-check 的 `.cmd` 须逐字等于命令串，
    /// test-execution 的段格式由 `testexec/mod.rs` 组装）。失败由调用方静默降级。
    pub fn write_command(&self, command: &str) -> Result<(), String> {
        let dir = self
            .log_path
            .parent()
            .ok_or_else(|| "exec 输出记录缺父目录".to_owned())?;
        let path = dir.join(format!("{}.{EXEC_COMMAND_SUFFIX}", self.exec_id));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("写入 exec 命令记录失败: {e}"))?;
        file.write_all(command.as_bytes())
            .map_err(|e| format!("写入 exec 命令记录失败: {e}"))
    }

    /// 收口：先 `take` 句柄（关文件）再 `std::fs::write` 覆盖为权威内容，返回**同一**
    /// `exec_id`。**MUST NOT** 另铸新 id（否则 running 拍与终态拍引用不相等）；此后
    /// 追加恒空操作。
    pub fn commit(&self, output: &[u8]) -> Result<String, String> {
        *self.writer.lock().expect("live 写入锁不可中毒") = None;
        std::fs::write(&self.log_path, output).map_err(|e| format!("落盘 exec 输出失败: {e}"))?;
        Ok(self.exec_id.clone())
    }
}

/// 增量读产物（**零 derive** 纯结构）：出线 DTO `ExecOutputChunk` 定义在壳层命令
/// 模块——本 crate 依赖面 MUST NOT 因出线需要而扩 serde / specta。
pub struct ExecOutputDelta {
    /// 自 `from_offset` 起的新增文本（UTF-8 字符边界收口）
    pub text: String,
    /// 下次调用应传的游标（`<=` 文件长度、跨调用单调不减）
    pub next_offset: u64,
}

/// `ExecOutput::Recorded` 落盘半边（输出记录单点）：降为
/// [`LiveExec::begin`] + [`LiveExec::commit`] 的薄包装——同铸造点、同落点
/// （`<report_dir>/exec-outputs/<exec_id>.log`，按槽位同锚派生）、同「零截断零改写」
/// 内容口径。单一流即完整还原终端——static-check 的 `2>&1` 交错流、test-execution
/// 的 stdout + stderr 合并流；stdout / stderr 分文件（`.err`）已退役。
pub fn record_exec_output(
    root: &Path,
    change: &str,
    slot: ReportSlot,
    output: &[u8],
) -> Result<String, String> {
    LiveExec::begin(root, change, slot)?.commit(output)
}

/// 命令 sidecar 回查（与 [`read_exec_output_in`] 同双锚探测：test 槽先 / baseline
/// 槽后）：无 sidecar 的老记录 → `None`（非错误）。
pub fn read_exec_command_in(change_dir: &Path, exec_id: &str) -> Option<String> {
    if !is_valid_exec_id(exec_id) {
        return None;
    }
    [ReportSlot::Test, ReportSlot::Baseline]
        .iter()
        .find_map(|slot| {
            std::fs::read_to_string(exec_command_path_in(change_dir, *slot, exec_id)).ok()
        })
}

/// 布局派生 `.log` 绝对路径（双锚探测，返回首个命中槽位的路径）：供前端 watch
/// 寻址——目录名知识 MUST NOT 下沉前端。
pub fn exec_output_log_path_in(change_dir: &Path, exec_id: &str) -> Option<PathBuf> {
    if !is_valid_exec_id(exec_id) {
        return None;
    }
    [ReportSlot::Test, ReportSlot::Baseline]
        .iter()
        .find_map(|slot| {
            let path = exec_output_dir_in(change_dir, *slot).join(format!("{exec_id}.log"));
            path.is_file().then_some(path)
        })
}

/// 增量读：自 `from_offset` 起的新增文本 + 下次游标（**offset 唯一产出方在后端**）。
///
/// 口径：`from_offset = 0` 即全量（与 [`read_exec_output`] 逐字相等）；`next_offset`
/// 单调不减且 `<= 文件长度`，块末半字符留待下次（落在 UTF-8 字符边界）；覆盖变短
/// （`from_offset > 长度`）夹 0 重读，MUST NOT panic / MUST NOT 返回空串；空文件
/// 返回 `Some("")`（与「文件缺失 → `None`」天然可分）。
pub fn read_exec_output_delta_in(
    change_dir: &Path,
    exec_id: &str,
    from_offset: u64,
) -> Option<ExecOutputDelta> {
    if !is_valid_exec_id(exec_id) {
        return None;
    }
    let bytes = [ReportSlot::Test, ReportSlot::Baseline]
        .iter()
        .find_map(|slot| {
            std::fs::read(exec_output_dir_in(change_dir, *slot).join(format!("{exec_id}.log"))).ok()
        })?;
    let len = bytes.len() as u64;
    let from = if from_offset > len {
        0
    } else {
        from_offset as usize
    };
    let end = from + complete_prefix_len(&bytes[from..]);
    Some(ExecOutputDelta {
        text: String::from_utf8_lossy(&bytes[from..end]).into_owned(),
        next_offset: end as u64,
    })
}

/// 块内**完整** UTF-8 前缀长度（纯函数零 IO）：末尾若落在**尚可补齐**的多字节序列
/// 内则退到该序列起点（该字符留待下次），否则返回全长；非法首字节按单字节计（⇒ 以
/// 替换符呈现且游标越过，不卡死调用方）。
///
/// 「尚可补齐」与「永远等不到补齐」的分界即首字节的合法性：`0xC0` / `0xC1`（过长
/// 编码，最短形态要求 ≥ `0xC2`）与 `0xF5..=0xFF`（超出 U+10FFFF）不是合法首字节——
/// 它们退位后也补不齐，只会让游标永不推进，故与「已完整」同处置（返回全长）。
///
/// `from_utf8` 的 `valid_up_to` / `error_len` 两臂在此收敛——调用侧只见「长度」，
/// 不必各自重写字符边界规则。
fn complete_prefix_len(bytes: &[u8]) -> usize {
    let len = bytes.len();
    // 回看至多 3 字节（UTF-8 序列最长 4 字节）
    for back in 1..=len.min(3) {
        let index = len - back;
        let byte = bytes[index];
        if byte & 0b1100_0000 == 0b1000_0000 {
            continue; // 续接字节：继续前看首字节
        }
        let need = match byte {
            0x00..=0x7F => 1,
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            // 非法首字节（`0xC0` / `0xC1` / `0xF5..=0xFF`）：按单字节计
            //（lossy 替换符 + 游标越过——退位会让游标永不推进）
            _ => return len,
        };
        return if index + need <= len { len } else { index };
    }
    // 回看窗口内全是续接字节（无首字节在前 3 字节内）⇒ 非法序列，按单字节推进
    len
}

/// 命令 sidecar 路径派生（sidecar 与 `.log` 同目录同 id 前缀的单一派生点）。
fn exec_command_path_in(change_dir: &Path, slot: ReportSlot, exec_id: &str) -> PathBuf {
    exec_output_dir_in(change_dir, slot).join(format!("{exec_id}.{EXEC_COMMAND_SUFFIX}"))
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
///
/// Windows 执行体的主回收手段是作业对象（见 [`crate::job`]）——本函数只作幂等
/// 补刀与「作业挂载失败」的回落面：`/T` 按调用时刻的父子快照向下遍历，msys fork
/// 副本可能换过父 pid 而被报成功却漏杀。
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
