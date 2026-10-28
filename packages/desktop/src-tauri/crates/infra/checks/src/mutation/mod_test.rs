use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use checks::model::MutationTool;
use config::{MutationConfig, TestFramework};
use orchestration::port::{
    ArchiCheckRunner, ArchiveVcsPort, BoxToolFuture, MutationRunner, MutationSkipReason,
    MutationTestConclusion, RebaseOutcome, RunFault, StaticCheckRunner, TestExecutionRunner,
    ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest, WorktreeSnapshot,
};
use orchestration::LocalToolSteps;
use tempfile::TempDir;
use workflow::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, PhaseStateRecord, RunEdgeStateRecord, RunFinishCommand, RunNodeStateRecord,
    RunStartCommand, RunStateRecord, StepCommand, StepStateRecord, StoreFault,
};
use workflow::write::SessionAnchors;

use super::report::to_posix;
use super::SegmentFailure;
use super::{
    bucket_suites, framework_name, mutation_tool, normalize_root, skip_outcome, ProcessMutation,
    EST_MUTANTS_PER_FILE, MUTATION_JOBS_DIVISOR, PER_MUTANT_TIMEOUT_SECS, SEGMENT_BUDGET_MS,
    SEGMENT_GUARD_MS,
};

// ---------------------------------------------------------------------------
// 假 spawn 装置（test-design Mock 表三节共享：记录 argv / env / cwd + 可编程
// 输出、超时、非零退出、ENOENT）
// ---------------------------------------------------------------------------

/// 假 spawn 行记录（`--- spawn ---` 语义下的 argv 逐参 + cwd + env 键值——
/// 模板展开 / env 共享 / spawn cwd 断言的原料）。
#[derive(Debug, Clone)]
pub(super) struct SpawnRecord {
    /// 逐参 argv（程序首 token 在内：`mutants` / `npx`）
    pub argv: Vec<String>,
    /// spawn cwd（平台原样：批处理 `%CD%` 反斜杠、`pwd -W` 正斜杠——断言侧归一）
    pub cwd: Option<String>,
    /// 环境面（`CARGO_TARGET_DIR` 等键值）
    pub env: Vec<(String, String)>,
}

/// 假 spawn 窗口：持 crate 级 `TEST_ENV_LOCK`（进程全局 env 串行化）+ PATH
/// 前置 shim 目录（Windows 附前置 msys 工具目录——假执行体外部件解析）+
/// `FAKE_SPAWN_*` 可编程键收口（Drop 恢复 PATH、摘除全部键、释放锁——与
/// testexec `PathWindowGuard` 同式先例）。
pub(super) struct SpawnWindow {
    /// 进 PATH 前原 PATH（Drop 还原面）
    original_path: Option<OsString>,
    /// shim 目录（假 cargo / npx 与记录、canned 工件落位）
    shim: TempDir,
    /// crate 环境锁（字段序最后——Drop 后最后释放，PATH 先还原）
    _lock: MutexGuard<'static, ()>,
}

impl SpawnWindow {
    /// 进入窗口：锁 env、建 shim 目录并前置 PATH、开记录文件键。
    pub(super) fn enter(tag: &str) -> Self {
        let lock = crate::lock_test_env();
        let shim = tempfile::Builder::new()
            .prefix(&format!("checks-mutation-fake-spawn-{tag}-"))
            .tempdir()
            .expect("创建假 spawn shim 目录失败");
        let original_path = std::env::var_os("PATH");
        let mut entries: Vec<PathBuf> = vec![shim.path().to_path_buf()];
        // msys coreutils 目录（`cp` / `mkdir`）紧随 shim 前置：假执行体脚本的
        // 外部件解析不依赖宿主 PATH 是否含 Git usr/bin——桌面应用门禁进程
        // （GUI 拉起）PATH 无 usr/bin，裸 shim 前置下 `mkdir` / `cp` 恒
        // command not found，`mutants.out/outcomes.json` 静默缺位（stderr 随
        // 零突变体臂丢弃）→ 全零计数 → score 恒 100 假绿。定位失败跳过
        // （bash 落位失败随后显式 panic 报因，不在此静默吞错）。
        #[cfg(windows)]
        if let Some(tools) = msys_tool_dir() {
            entries.push(tools);
        }
        if let Some(value) = &original_path {
            entries.extend(std::env::split_paths(value));
        }
        let joined = std::env::join_paths(entries).expect("拼接 PATH 应成功");
        std::env::set_var("PATH", &joined);
        std::env::set_var("FAKE_SPAWN_LOG", shim.path().join("spawn.log"));
        Self {
            original_path,
            shim,
            _lock: lock,
        }
    }

    /// 假 `cargo` 落位 + suite 执行目录假执行体（`mutants`——两平台同构：记录
    /// `argv|mutants|…`、cwd、`CARGO_TARGET_DIR`；可编程输出 / ALT 匹配 / 自旋
    /// 挂起 / 退出码）。Windows：msys bash 拷贝为 `cargo.exe`（bare spawn 只认
    /// PE）读 cwd 脚本；unix：PATH shim 委派 cwd 脚本——内层 `$0` / `$@` 面逐字
    /// 同构。
    pub(super) fn install_fake_cargo(&self, exec_dir: &Path) {
        fs::create_dir_all(exec_dir).expect("建 suite 执行目录");
        #[cfg(windows)]
        {
            let target = self.shim.path().join("cargo.exe");
            if !target.is_file() {
                let (bash, dll) = msys_bash_sources();
                fs::copy(&bash, &target).expect("拷贝 msys bash 为假 cargo.exe");
                fs::copy(&dll, self.shim.path().join("msys-2.0.dll")).expect("拷贝 msys-2.0.dll");
            }
        }
        #[cfg(not(windows))]
        {
            let target = self.shim.path().join("cargo");
            if !target.is_file() {
                fs::write(&target, UNIX_CARGO_SHIM).expect("写假 cargo shim");
                set_executable(&target);
            }
        }
        write_mutants_script(exec_dir);
    }

    /// 假 `npx` 落位（cmd /C 经 PATHEXT 解析 `.cmd`、unix 经 PATH 解析脚本；
    /// 记录 `argv|npx|…` + cwd，可编程自旋挂起 / 输出 / 段报告 / 退出码）。
    pub(super) fn install_npx(&self) {
        #[cfg(windows)]
        {
            // 自旋壳（记录自身 winpid 后恒活）——无内建 pid 变量的 cmd 面经
            // PowerShell 承接 winpid 落盘；`taskkill /T /F` 整树击杀后该 pid 消失。
            // 壳路径**绝对内联**（`%~dp0` 在 PATH 解析的裸名调用下退化为 cwd，
            // 不能用于定位同目录兄弟件）
            let spin = self.shim.path().join("npx-spin.ps1");
            if !spin.is_file() {
                fs::write(&spin, NPX_SPIN_PS1).expect("写假 npx 自旋壳");
            }
            let target = self.shim.path().join("npx.cmd");
            let body = NPX_CMD.replace("{spin_script}", &spin.display().to_string());
            if !target.is_file() {
                fs::write(&target, body).expect("写假 npx.cmd");
            }
        }
        #[cfg(not(windows))]
        {
            let target = self.shim.path().join("npx");
            if !target.is_file() {
                fs::write(&target, UNIX_NPX_SHIM).expect("写假 npx shim");
                set_executable(&target);
            }
        }
    }

    /// canned 输出源（stdout 面；段报告面在 `set_report` 时同源写出）。
    pub(super) fn set_out(&self, body: &str) -> PathBuf {
        let path = self.shim.path().join("fake-out.txt");
        fs::write(&path, body).expect("写 canned 输出");
        std::env::set_var("FAKE_SPAWN_OUT_SRC", &path);
        path
    }

    /// canned stderr 面（双管道大块并发用例的 stderr 源——stdout 面经
    /// [`Self::set_out`]，两路各由假执行体独立写出；未设 = stderr 零字节）。
    pub(super) fn set_err(&self, body: &str) -> PathBuf {
        let path = self.shim.path().join("fake-err.txt");
        fs::write(&path, body).expect("写 canned stderr 输出");
        std::env::set_var("FAKE_SPAWN_ERR_SRC", &path);
        path
    }

    /// canned ALT 面（argv 子串命中 → 换输出源与退出码——多 suite 差异编程）。
    pub(super) fn set_alt(&self, argv_match: &str, body: &str, exit: Option<i32>) {
        let path = self.shim.path().join("fake-out-alt.txt");
        fs::write(&path, body).expect("写 canned ALT 输出");
        std::env::set_var("FAKE_SPAWN_ARG_MATCH", argv_match);
        std::env::set_var("FAKE_SPAWN_OUT_ALT", &path);
        match exit {
            Some(code) => std::env::set_var("FAKE_SPAWN_EXIT_ALT", code.to_string()),
            None => std::env::remove_var("FAKE_SPAWN_EXIT_ALT"),
        }
    }

    /// 可编程非零退出（npx 面 / cargo 面共键）。
    pub(super) fn set_exit(&self, code: i32) {
        std::env::set_var("FAKE_SPAWN_EXIT", code.to_string());
    }

    /// 可编程超时：记录后自旋挂起（段 spawn 硬超时经虚拟时钟推进触发）。
    pub(super) fn set_sleep(&self, seconds: u64) {
        std::env::set_var("FAKE_SPAWN_SLEEP", seconds.to_string());
    }

    /// 可编程 pid 落位（假执行体记录自身 winpid——被杀后 pid 回收断言原料；
    /// 与取消 / 超时臂的「进程确已建立」握手面同源）。
    pub(super) fn set_pidfile(&self, path: &Path) {
        std::env::set_var("FAKE_SPAWN_PIDFILE", path);
    }

    /// 可编程段报告写出路径（Stryker jsonReporter 直写段报告面模拟）。
    pub(super) fn set_report(&self, path: &Path) {
        std::env::set_var("FAKE_SPAWN_REPORT", path);
    }

    /// cargo-mutants 段报告权威落位编程（真实工具在 cwd 重建
    /// `mutants.out/outcomes.json` 面模拟；缺省不写 = 真实工具 diff 零可突变交
    /// 叠 / 崩溃无报告面）。argv ALT 命中时不消费本源（换
    /// [`Self::set_cargo_outcomes_alt`]）。
    pub(super) fn set_cargo_outcomes(&self, body: &str) -> PathBuf {
        let path = self.shim.path().join("fake-cargo-outcomes.json");
        fs::write(&path, body).expect("写 canned cargo outcomes");
        std::env::set_var("FAKE_SPAWN_CARGO_OUTCOMES", &path);
        path
    }

    /// cargo-mutants 段报告 ALT 源（argv 子串命中 → 换 outcomes 源——多 suite
    /// 差异编程；与 [`Self::set_alt`] 共用 `FAKE_SPAWN_ARG_MATCH` 命中键，命中且
    /// 未设 ALT 源 = 不写报告，与真实工具零交叠面同形）。
    pub(super) fn set_cargo_outcomes_alt(&self, argv_match: &str, body: &str) {
        let path = self.shim.path().join("fake-cargo-outcomes-alt.json");
        fs::write(&path, body).expect("写 canned ALT cargo outcomes");
        std::env::set_var("FAKE_SPAWN_ARG_MATCH", argv_match);
        std::env::set_var("FAKE_SPAWN_CARGO_OUTCOMES_ALT", &path);
    }

    /// 记录清零（多 run 同窗序列化用例的记录分界）。
    pub(super) fn reset_records(&self) {
        let _ = fs::remove_file(self.log_path());
    }

    /// 记录回读（`argv|…` / `cwd|…` / `env|K=V` 行 → 逐段记录）。
    pub(super) fn records(&self) -> Vec<SpawnRecord> {
        let Ok(text) = fs::read_to_string(self.log_path()) else {
            return Vec::new();
        };
        let mut records: Vec<SpawnRecord> = Vec::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("argv|") {
                records.push(SpawnRecord {
                    argv: rest.split('|').map(str::to_owned).collect(),
                    cwd: None,
                    env: Vec::new(),
                });
            } else if let Some(cwd) = line.strip_prefix("cwd|") {
                if let Some(last) = records.last_mut() {
                    last.cwd = Some(cwd.to_owned());
                }
            } else if let Some(pair) = line.strip_prefix("env|") {
                if let Some((key, value)) = pair.split_once('=') {
                    if let Some(last) = records.last_mut() {
                        last.env.push((key.to_owned(), value.to_owned()));
                    }
                }
            }
        }
        records
    }

    fn log_path(&self) -> PathBuf {
        self.shim.path().join("spawn.log")
    }
}

impl Drop for SpawnWindow {
    fn drop(&mut self) {
        match &self.original_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
        for key in [
            "FAKE_SPAWN_LOG",
            "FAKE_SPAWN_OUT_SRC",
            "FAKE_SPAWN_ERR_SRC",
            "FAKE_SPAWN_OUT_ALT",
            "FAKE_SPAWN_ARG_MATCH",
            "FAKE_SPAWN_EXIT",
            "FAKE_SPAWN_EXIT_ALT",
            "FAKE_SPAWN_SLEEP",
            "FAKE_SPAWN_PIDFILE",
            "FAKE_SPAWN_REPORT",
            "FAKE_SPAWN_CARGO_OUTCOMES",
            "FAKE_SPAWN_CARGO_OUTCOMES_ALT",
        ] {
            std::env::remove_var(key);
        }
    }
}

/// 假执行体落盘的 pid（winpid 口径——`tasklist` / `kill -0` 可探测）。
pub(super) fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse().ok())
}

/// 等假执行体落 pid（真实时钟握手：虚拟时钟下进程启动不受影响，等待以真实
/// 睡眠 + 让出表达）。
pub(super) async fn wait_pid(path: &Path, budget: std::time::Duration) -> u32 {
    let deadline = std::time::Instant::now() + budget;
    loop {
        if let Some(pid) = read_pid(path) {
            return pid;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "等待假执行体落 pid 超时（{budget:?}）"
        );
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// pid 存活探测（Windows `tasklist`、unix `kill -0`）。
#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output();
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// 等待 pid 消失（杀树是尽力语义 + 平台回收时延，轮询到上界即判定仍存活）。
pub(super) fn wait_pid_gone(pid: u32, budget: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + budget;
    while std::time::Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    !pid_alive(pid)
}

/// msys coreutils 目录（Git for Windows `usr/bin`——`cp` / `mkdir` 等 POSIX
/// 外部件所在；假执行体脚本外部件的可靠解析源，Windows PATH 前置消费）：
/// bash 直查目录已携该二件（完整 `usr/bin` 形态）即用之，否则回退
/// `..\usr\bin` 兄弟目录。探测失败返回 `None`——调用方跳过 PATH 前置，bash
/// 落位失败随后以显式 panic 报因（不在此静默吞错）。
#[cfg(windows)]
fn msys_tool_dir() -> Option<PathBuf> {
    let has_tools = |dir: &Path| dir.join("cp.exe").is_file() && dir.join("mkdir.exe").is_file();
    let bash = match exec::resolve_shell() {
        exec::ShellBase::GitBash(path) => path,
        _ => return None,
    };
    let dir = bash.parent()?.to_path_buf();
    if has_tools(&dir) {
        return Some(dir);
    }
    let sibling = dir.join("..").join("usr").join("bin");
    if has_tools(&sibling) {
        return Some(sibling);
    }
    None
}

/// Windows：定位 Git for Windows msys bash（`cargo.exe` 拷贝源）——`resolve_shell`
/// 探到的 `Git\bin\bash.exe` 为 47KB 壳时回退其 `..\usr\bin` 兄弟目录（bash.exe +
/// msys-2.0.dll 同目录为完整运行单元，实测可独立拷贝运行）。
#[cfg(windows)]
fn msys_bash_sources() -> (PathBuf, PathBuf) {
    let bash = match exec::resolve_shell() {
        exec::ShellBase::GitBash(path) => path,
        _ => panic!("假 spawn 装置需要 Git for Windows bash（resolve_shell 未探测到 GitBash）"),
    };
    let dir = bash.parent().expect("bash 路径应有父目录").to_path_buf();
    let direct = (dir.join("bash.exe"), dir.join("msys-2.0.dll"));
    if direct.0.is_file() && direct.1.is_file() {
        return direct;
    }
    let sibling_dir = dir.join("..").join("usr").join("bin");
    let sibling = (
        sibling_dir.join("bash.exe"),
        sibling_dir.join("msys-2.0.dll"),
    );
    if sibling.0.is_file() && sibling.1.is_file() {
        return sibling;
    }
    panic!(
        "未在 {} 及其 usr/bin 兄弟目录找到 bash.exe + msys-2.0.dll（Git for Windows 完整安装？）",
        dir.display()
    );
}

/// cwd 假执行体 `mutants` 落位（两平台共用同一脚本——记录面 / 可编程面逐字同
/// 构；bash / sh 双解释器皆按 POSIX 解释）。
pub(super) fn write_mutants_script(exec_dir: &Path) {
    fs::create_dir_all(exec_dir).expect("建 suite 执行目录");
    let path = exec_dir.join("mutants");
    fs::write(&path, MUTANTS_SCRIPT).expect("写假 mutants 执行体");
    set_executable(&path);
}

/// 执行位补齐（unix 面；Windows 由解释器直接读脚本，无需执行位）。
#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("补执行位");
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) {}

/// 假 `mutants` 执行体
const MUTANTS_SCRIPT: &str = r#"#!/bin/sh
{
  printf 'argv'
  printf '|%s' "$0"
  for fake_arg in "$@"; do printf '|%s' "$fake_arg"; done
  printf '\n'
  if fake_win_cwd=$(pwd -W 2>/dev/null) && [ -n "$fake_win_cwd" ]; then
    printf 'cwd|%s\n' "$fake_win_cwd"
  else
    printf 'cwd|%s\n' "$(pwd)"
  fi
  printf 'env|CARGO_TARGET_DIR=%s\n' "${CARGO_TARGET_DIR:-<unset>}"
} >> "${FAKE_SPAWN_LOG:?}"
if [ -n "${FAKE_SPAWN_PIDFILE:-}" ]; then
  fake_winpid=$(cat "/proc/$$/winpid" 2>/dev/null || printf '%s' "$$")
  printf '%s' "$fake_winpid" > "$FAKE_SPAWN_PIDFILE"
fi
if [ -n "${FAKE_SPAWN_SLEEP:-}" ]; then
  while :; do :; done
fi
fake_matched=
if [ -n "${FAKE_SPAWN_ARG_MATCH:-}" ]; then
  case "$*" in
    *"$FAKE_SPAWN_ARG_MATCH"*) fake_matched=1 ;;
  esac
fi
fake_out=${FAKE_SPAWN_OUT_SRC:-}
fake_exit=${FAKE_SPAWN_EXIT:-0}
if [ -n "$fake_matched" ]; then
  if [ -n "${FAKE_SPAWN_OUT_ALT:-}" ]; then fake_out=$FAKE_SPAWN_OUT_ALT; fi
  if [ -n "${FAKE_SPAWN_EXIT_ALT:-}" ]; then fake_exit=$FAKE_SPAWN_EXIT_ALT; fi
fi
if [ -n "$fake_out" ] && [ -f "$fake_out" ]; then
  while IFS= read -r fake_line || [ -n "$fake_line" ]; do
    printf '%s\n' "$fake_line"
  done < "$fake_out"
  if [ -n "${FAKE_SPAWN_REPORT:-}" ]; then
    while IFS= read -r fake_line || [ -n "$fake_line" ]; do
      printf '%s\n' "$fake_line"
    done < "$fake_out" > "$FAKE_SPAWN_REPORT"
  fi
fi
if [ -n "${FAKE_SPAWN_ERR_SRC:-}" ] && [ -f "$FAKE_SPAWN_ERR_SRC" ]; then
  while IFS= read -r fake_line || [ -n "$fake_line" ]; do
    printf '%s\n' "$fake_line" >&2
  done < "$FAKE_SPAWN_ERR_SRC"
fi
fake_cargo_outcomes=${FAKE_SPAWN_CARGO_OUTCOMES:-}
if [ -n "$fake_matched" ]; then
  fake_cargo_outcomes=${FAKE_SPAWN_CARGO_OUTCOMES_ALT:-}
fi
if [ -n "$fake_cargo_outcomes" ] && [ -f "$fake_cargo_outcomes" ]; then
  mkdir -p mutants.out
  cp "$fake_cargo_outcomes" mutants.out/outcomes.json
fi
exit "$fake_exit"
"#;

/// Windows 假 `cargo.exe` 的 unix 对等物：`cargo mutants …` → 转交 cwd 假执行
/// 体（与 msys bash-as-cargo 同构——内层 `$0` = `mutants`、`$@` 其后 argv，记
/// 录格式跨平台一致）。
#[cfg(not(windows))]
const UNIX_CARGO_SHIM: &str = r#"#!/bin/sh
if [ "$1" = "mutants" ]; then shift; fi
exec sh mutants "$@"
"#;

/// unix 假 `npx`（记录 `argv|npx|…` 与 Windows 批处理同格式；可编程面同式）。
#[cfg(not(windows))]
const UNIX_NPX_SHIM: &str = r#"#!/bin/sh
{
  printf 'argv|npx'
  for fake_arg in "$@"; do printf '|%s' "$fake_arg"; done
  printf '\n'
  if fake_win_cwd=$(pwd -W 2>/dev/null) && [ -n "$fake_win_cwd" ]; then
    printf 'cwd|%s\n' "$fake_win_cwd"
  else
    printf 'cwd|%s\n' "$(pwd)"
  fi
} >> "${FAKE_SPAWN_LOG:?}"
if [ -n "${FAKE_SPAWN_PIDFILE:-}" ]; then
  fake_winpid=$(cat "/proc/$$/winpid" 2>/dev/null || printf '%s' "$$")
  printf '%s' "$fake_winpid" > "$FAKE_SPAWN_PIDFILE"
fi
if [ -n "${FAKE_SPAWN_SLEEP:-}" ]; then
  while :; do :; done
fi
fake_out=${FAKE_SPAWN_OUT_SRC:-}
fake_exit=${FAKE_SPAWN_EXIT:-0}
if [ -n "$fake_out" ] && [ -f "$fake_out" ]; then
  while IFS= read -r fake_line || [ -n "$fake_line" ]; do
    printf '%s\n' "$fake_line"
  done < "$fake_out"
  if [ -n "${FAKE_SPAWN_REPORT:-}" ]; then
    while IFS= read -r fake_line || [ -n "$fake_line" ]; do
      printf '%s\n' "$fake_line"
    done < "$fake_out" > "$FAKE_SPAWN_REPORT"
  fi
fi
if [ -n "${FAKE_SPAWN_ERR_SRC:-}" ] && [ -f "$FAKE_SPAWN_ERR_SRC" ]; then
  while IFS= read -r fake_line || [ -n "$fake_line" ]; do
    printf '%s\n' "$fake_line" >&2
  done < "$FAKE_SPAWN_ERR_SRC"
fi
exit "$fake_exit"
"#;

/// Windows 假 `npx.cmd`（cmd /C 经 PATHEXT 解析——bare spawn 只认 PE、壳面经
/// `cmd /C` 免此限；**CRLF 行终结**——cmd 对 LF-only 批处理解析不稳；逐参收集
/// 记录 `argv|npx|…`（`%~1` 去引号）、`%CD%` 记 cwd、可编程自旋挂起 / 输出 /
/// 段报告 / 退出码）。
#[cfg(windows)]
const NPX_CMD: &str = concat!(
    "@echo off\r\n",
    "setlocal EnableDelayedExpansion\r\n",
    "set \"line=argv|npx\"\r\n",
    ":collect\r\n",
    "if \"%~1\"==\"\" goto collected\r\n",
    "set \"line=!line!|%~1\"\r\n",
    "shift\r\n",
    "goto collect\r\n",
    ":collected\r\n",
    ">>\"%FAKE_SPAWN_LOG%\" echo(!line!\r\n",
    ">>\"%FAKE_SPAWN_LOG%\" echo(cwd^|!CD!\r\n",
    "if defined FAKE_SPAWN_SLEEP goto spin\r\n",
    "if defined FAKE_SPAWN_OUT_SRC type \"!FAKE_SPAWN_OUT_SRC!\"\r\n",
    "if defined FAKE_SPAWN_ERR_SRC type \"!FAKE_SPAWN_ERR_SRC!\" 1>&2\r\n",
    "if defined FAKE_SPAWN_REPORT if defined FAKE_SPAWN_OUT_SRC copy /y \"!FAKE_SPAWN_OUT_SRC!\" \"!FAKE_SPAWN_REPORT!\" >nul\r\n",
    "if defined FAKE_SPAWN_EXIT exit /b !FAKE_SPAWN_EXIT!\r\n",
    "exit /b 0\r\n",
    ":spin\r\n",
    "if defined FAKE_SPAWN_PIDFILE powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{spin_script}\"\r\n",
    "goto spin\r\n",
);

/// Windows 假 `npx.cmd` 的自旋壳（记录自身 winpid 后恒活）：cmd 无内建 pid
/// 变量，winpid 经 PowerShell 落盘——`taskkill /T /F` 整树击杀后该 pid 必消失
///（进程树回收的可稳定观测面）。
#[cfg(windows)]
const NPX_SPIN_PS1: &str =
    "if ($env:FAKE_SPAWN_PIDFILE) { Set-Content -Path $env:FAKE_SPAWN_PIDFILE -Value $PID -NoNewline -Encoding ascii }\nwhile ($true) { Start-Sleep -Milliseconds 100 }\n";

// ---------------------------------------------------------------------------
// canned 面与 config fixture
// ---------------------------------------------------------------------------

/// rust suite（app 锚点、阈 80）单 suite config（复用门与组合根链路面共用）。
const RUST_APP_CONFIG: &str = r#"{ "tests": [ { "root": "app", "framework": "rust", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 80 } } ] }"#;

/// 双 rust suite config（app 阈 80 / src-tauri 阈 70——多 suite 多阈值判定面）。
const TWO_RUST_CONFIG: &str = r#"{ "tests": [
  { "root": "app", "framework": "rust", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 80 } },
  { "root": "src-tauri", "framework": "rust", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 70 } }
] }"#;

/// vitest suite（app 锚点、阈 80）——js 族 Stryker 全链面。
const VITEST_CONFIG: &str = r#"{ "tests": [ { "root": "app", "framework": "vitest", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 80 } } ] }"#;

/// Stryker 段报告 canned（metrics 面：9 killed + 1 survived / 10 → score 90）。
const STRYKER_SEG_PASS: &str =
    r#"{ "metrics": { "killed": 9, "survived": 1, "totalMutants": 10 }, "durationMs": 3000.0 }"#;

/// 段中断 canned（非 JSON 损坏输出——harness 崩面）。
const SEG_INTERRUPT_GARBAGE: &str = "harness crashed: boom-marker";

/// cargo-mutants 段报告 fixture（`mutants.out/outcomes.json` 真实形态——运行
/// 面计数面：9 caught + 1 missed / total 10 → score 90）。
const CARGO_SEG_PASS: &str = r#"{
  "outcomes": [],
  "total_mutants": 10,
  "missed": 1,
  "caught": 9,
  "timeout": 0,
  "unviable": 0,
  "success": true,
  "cargo_mutants_version": "27.1.0"
}"#;

/// cargo-mutants 段报告 fixture（5 caught + 5 missed / total 10——score 50 未达
/// 80 阈）。
const CARGO_SEG_FAIL: &str = r#"{
  "outcomes": [],
  "total_mutants": 10,
  "missed": 5,
  "caught": 5,
  "timeout": 0,
  "unviable": 0,
  "success": false,
  "cargo_mutants_version": "27.1.0"
}"#;

/// cargo-mutants 段报告 canned（`killed` caught + `survived` missed；score =
/// killed / total 两位舍入）。
fn cargo_seg(killed: usize, survived: usize) -> String {
    format!(
        r#"{{ "outcomes": [], "total_mutants": {total}, "missed": {survived}, "caught": {killed}, "timeout": 0, "unviable": 0, "success": true }}"#,
        total = killed + survived,
    )
}

/// 组合根链路 change **id**（≠ name——id → 记录 → name 解析面可辨）。
const COMBO_CHANGE_ID: &str = "combo-change-0001";

/// 组合根链路 fake vcs 补丁（单文件段；清单匹配用项目根相对路径）。
const COMBO_PATCH: &str = "diff --git a/app/src/lib.rs b/app/src/lib.rs\n--- a/app/src/lib.rs\n+++ b/app/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";

/// 组合根链路段补丁期望（vcs 补丁切片 + suite 根 `app/` 前缀剥离——落盘面路径
/// 相对 suite 执行目录，cargo mutants `--in-diff` 源树匹配口径）。
const COMBO_PATCH_STRIPPED: &str = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";

// ---------------------------------------------------------------------------
// 装置：workspace fixture
// ---------------------------------------------------------------------------

/// workspace fixture：config（suite 锚点 + 逐 suite 阈值）+ 可选段报告预置
/// （AC-8 复用门——零 spawn 路径）+ 可选 summary.json。
struct ChainWs {
    _tmp: TempDir,
    root: PathBuf,
    change: &'static str,
}

impl ChainWs {
    /// 复用门装置：rust suite（app 锚点、阈 80）+ 预置段报告（mtime 晚于输入
    /// 即复用——suite root 不存在 → newest_input 缺席 → 在位即复用）。
    fn new(tag: &str, seg_report: &str) -> Self {
        let ws = Self::with_config(tag, RUST_APP_CONFIG);
        let seg_dir = foundation::layout::change_test_reports(&ws.root, ws.change)
            .join("app_rust")
            .join("mutation");
        fs::create_dir_all(&seg_dir).expect("建突变报告目录");
        fs::write(seg_dir.join("mutation-seg-0.json"), seg_report).expect("写段报告");
        ws
    }

    /// 执行链装置：任意 config、零预置段报告（段复用门失效 → 假 spawn 真执行）。
    fn with_config(tag: &str, config_json: &str) -> Self {
        let tmp = tempfile::Builder::new()
            .prefix(&format!("checks-mutation-mod-test-{tag}-"))
            .tempdir()
            .expect("创建临时 workspace 失败");
        let root = tmp.path().to_path_buf();
        let change = "demo-change";

        let config_dir = root.join("openspec");
        fs::create_dir_all(&config_dir).expect("建 openspec 目录");
        fs::write(config_dir.join("config.json"), config_json).expect("写 config");

        Self {
            _tmp: tmp,
            root,
            change,
        }
    }

    /// 预置 summary.json（E11 回填在位面 / 中断不回填面）。
    fn with_summary(self) -> Self {
        let reports = foundation::layout::change_test_reports(&self.root, self.change);
        fs::create_dir_all(&reports).expect("建报告目录");
        fs::write(
            reports.join("summary.json"),
            r#"{ "phase": "test-execution", "total": 3, "passed": 3, "failed": 0, "skipped": 0, "mutation": null }"#,
        )
        .expect("写 summary");
        self
    }

    fn root_str(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
}

/// 经 `MutationRunner` trait 面驱动（生产装配同路径）。
async fn run(ws: &ChainWs, files: &[String]) -> Result<ToolStepOutput, String> {
    let runner = ProcessMutation::new();
    runner
        .run(
            &ws.root_str(),
            ws.change,
            files,
            "diff --git a/app/src/lib.rs b/app/src/lib.rs\n",
            foundation::cancel::CancelFlag::never(),
        )
        .await
}

/// 产出窄化：MutationTest 变体载荷提取。
fn outcome_of(output: ToolStepOutput) -> orchestration::port::MutationTestOutcome {
    match output {
        ToolStepOutput::MutationTest(outcome) => outcome,
        other => panic!("产出变体漂移: {other:?}"),
    }
}

/// summary.json 回读（mutation 位回填断言面）。
fn read_summary(ws: &ChainWs) -> serde_json::Value {
    let path = foundation::layout::change_test_reports(&ws.root, ws.change).join("summary.json");
    serde_json::from_str(&fs::read_to_string(path).expect("summary 在盘")).expect("summary 可解析")
}

/// 正向：`ProcessMutation::new` 与 `Default::default()` 等价可用（无状态空
/// 类型——可重复构造即断言面）。
#[test]
fn 构造等价_new与default可重复() {
    let a = ProcessMutation::new();
    let b = ProcessMutation;
    let c = ProcessMutation::new();
    // 零字段无状态：类型层等值面（构造零副作用由 move 语义承载）
    let _ = (a, b, c);
}

/// 正向：rust suite 复用门全链（组合：run → 归桶 → 复用零 spawn → 解析 →
/// 归并 → 阈值判定 → outcome 装配）——Pass 面：score 90 ≥ 80、counts 七字段、
/// report_paths 携段级 + 归并报告、feedback_text 恒空（AC-4 / AC-8）。
#[tokio::test]
async fn 复用门全链_rust_suite_pass面装配() {
    let ws = ChainWs::new("pass", CARGO_SEG_PASS);
    let files = vec!["app/src/lib.rs".to_owned()];

    let output = run(&ws, &files).await.expect("全链应成功");
    let outcome = outcome_of(output);
    assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);
    assert_eq!(outcome.skip, None, "已执行形态 skip 恒 None");
    assert_eq!(outcome.score, 90.0, "归并块口径得分（9/10）");
    assert_eq!(outcome.threshold, 80.0, "首块阈值 = suite 配置阈值");
    assert_eq!(
        (
            outcome.counts.killed,
            outcome.counts.survived,
            outcome.counts.timeout,
            outcome.counts.compile_error,
            outcome.counts.runtime_error,
            outcome.counts.ignored,
            outcome.counts.total,
        ),
        (9, 1, 0, 0, 0, 0, 10),
        "归并计数七字段"
    );
    assert!(
        outcome.feedback_text.is_empty(),
        "Pass 恒空串（达标零反馈）"
    );
    // report_paths 携段级 + 归并报告（POSIX 形态）
    assert_eq!(outcome.report_paths.len(), 2, "段级 + 归并报告路径清单");
    assert!(
        outcome.report_paths.iter().all(|path| !path.contains('\\')),
        "报告路径 POSIX 形态"
    );
    assert!(
        outcome.report_paths[0].contains("mutation-seg-0.json"),
        "段级报告在案: {:?}",
        outcome.report_paths
    );
    assert!(
        outcome.report_paths[1].ends_with("mutation.json"),
        "归并块报告在案: {:?}",
        outcome.report_paths
    );

    // 归并块真实落盘（mutation.json serde 回读等值）
    let merged_path = foundation::layout::change_test_reports(&ws.root, ws.change)
        .join("app_rust")
        .join("mutation")
        .join("mutation.json");
    let merged: checks::model::MutationBlock =
        serde_json::from_str(&fs::read_to_string(&merged_path).expect("归并块在盘"))
            .expect("归并块可回读");
    assert_eq!(merged.score, 90.0, "归并块落盘分值一致");
    assert!(merged.pass, "90.0 >= 80");
}

/// 正向：未达阈 → `conclusion = Fail`、`feedback_text` 携 score / threshold
/// 结论摘要 + 未达标 suite 明细 + 报告路径清单（报告指引式——MUST NOT 内联
/// 存活突变体清单）。
#[tokio::test]
async fn 复用门全链_未达阈fail反馈文本报告指引式() {
    let ws = ChainWs::new("fail", CARGO_SEG_FAIL);
    let files = vec!["app/src/lib.rs".to_owned()];

    let outcome = outcome_of(run(&ws, &files).await.expect("Fail 面应 Ok"));
    assert_eq!(outcome.conclusion, MutationTestConclusion::Fail);
    assert_eq!(outcome.score, 50.0, "5/10 = 50.0");
    assert_eq!(outcome.threshold, 80.0);

    let feedback = &outcome.feedback_text;
    assert!(
        feedback.contains("score 50") && feedback.contains("threshold 80"),
        "feedback_text 携 score / threshold 结论摘要: {feedback}"
    );
    assert!(
        feedback.contains("app_rust") && feedback.contains("未达标 suite"),
        "携未达标 suite 明细: {feedback}"
    );
    assert!(
        feedback.contains("原始报告路径") && feedback.contains("mutation.json"),
        "携报告路径清单（报告指引式）: {feedback}"
    );
    // MUST NOT 内联存活突变体清单——零 mutant 体明细词面
    assert!(
        !feedback.contains("\"outcome\": \"missed\"") && !feedback.contains("mutant #"),
        "零存活突变体内联: {feedback}"
    );
}

/// 正向：跳过面 `EmptyDiff`——files 空清单 → `skip = EmptyDiff`、结论恒 Pass、
/// score / threshold 0.0 占位、feedback_text 空、MUST NOT Err（AC-7 null 观察
/// 期）；桶外文件不突变（全桶空同样 EmptyDiff——E10 桶外面）。
#[tokio::test]
async fn 跳过面_empty_diff空清单与桶外文件() {
    let ws = ChainWs::new("empty", CARGO_SEG_PASS);

    // 空清单
    let outcome = outcome_of(run(&ws, &[]).await.expect("EmptyDiff 面恒 Ok"));
    assert_eq!(outcome.skip, Some(MutationSkipReason::EmptyDiff));
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Pass,
        "null 观察期不判 fail"
    );
    assert_eq!((outcome.score, outcome.threshold), (0.0, 0.0), "0.0 占位");
    assert!(outcome.report_paths.is_empty() && outcome.feedback_text.is_empty());

    // 桶外文件（不落 app 桶——零突变面同样 EmptyDiff）
    let outside = vec!["other/lib.rs".to_owned()];
    let outcome = outcome_of(run(&ws, &outside).await.expect("桶外面恒 Ok"));
    assert_eq!(
        outcome.skip,
        Some(MutationSkipReason::EmptyDiff),
        "桶外文件不突变（E10）"
    );
}

/// 正向：跳过面 `NoExecutor`——node-test suite（框架无突变执行器）+ 落桶
/// 文件 → `skip = NoExecutor` 记 evidence 不 Err（AC-4：「框架设计上无执行
/// 器」与「工具没装」语义不同）。
#[tokio::test]
async fn 跳过面_no_executor_无执行器框架记evidence不err() {
    let tmp = tempfile::Builder::new()
        .prefix("checks-mutation-mod-test-noexec-")
        .tempdir()
        .expect("建临时 workspace");
    let root = tmp.path();
    let config_dir = root.join("openspec");
    fs::create_dir_all(&config_dir).expect("建 openspec");
    fs::write(
        config_dir.join("config.json"),
        r#"{ "tests": [ { "root": "app", "framework": "node-test", "coverage": { "lines": 60, "branches": 70, "functions": 75 } } ] }"#,
    )
    .expect("写 config");

    let runner = ProcessMutation::new();
    let files = vec!["app/src/add.test.mjs".to_owned()];
    let output = runner
        .run(
            &root.to_string_lossy(),
            "demo-change",
            &files,
            "patch",
            foundation::cancel::CancelFlag::never(),
        )
        .await
        .expect("NoExecutor 面恒 Ok");
    let outcome = outcome_of(output);
    assert_eq!(outcome.skip, Some(MutationSkipReason::NoExecutor));
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Pass,
        "跳过面不判 fail"
    );
    assert_eq!(outcome.fault, None, "跳过面 fault = None（字面量随动锚）");
    assert!(
        outcome.feedback_text.is_empty(),
        "NoExecutor evidence 记 PhaseLog 面（本载荷零反馈）"
    );
}

/// 异常：E5 显式 Err 面——报告目录不可建（`openspec/changes/<change>` 被文件
/// 占位）→ 写盘失败显式 `Err` 停给用户（不静默跳过不部分结论）。
#[tokio::test]
async fn 显式err面_写盘失败停给用户() {
    let tmp = tempfile::Builder::new()
        .prefix("checks-mutation-mod-test-e5-")
        .tempdir()
        .expect("建临时 workspace");
    let root = tmp.path();
    let config_dir = root.join("openspec");
    fs::create_dir_all(&config_dir).expect("建 openspec");
    fs::write(
        config_dir.join("config.json"),
        r#"{ "tests": [ { "root": "app", "framework": "rust", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 80 } } ] }"#,
    )
    .expect("写 config");
    // `openspec/changes/<change>` 占位为文件 → 报告目录 create_dir_all 必败
    let changes_dir = root.join("openspec").join("changes");
    fs::create_dir_all(&changes_dir).expect("建 changes 目录");
    fs::write(changes_dir.join("demo-change"), "占位文件").expect("写占位文件");

    let runner = ProcessMutation::new();
    let files = vec!["app/src/lib.rs".to_owned()];
    let error = runner
        .run(
            &root.to_string_lossy(),
            "demo-change",
            &files,
            "patch",
            foundation::cancel::CancelFlag::never(),
        )
        .await
        .expect_err("写盘失败应显式 Err");
    assert!(
        error.contains("突变报告目录创建失败"),
        "E5 写盘失败记因可修面: {error}"
    );
}

/// 边界：summary mutation 位回填（E11）——summary 在位（可解析对象）→ 回写
/// 归并块；不在位 → 不回填不崩（null 保持）。
#[tokio::test]
async fn summary回填_在位回写不在位不崩() {
    // 在位面：回写归并块
    let ws = ChainWs::new("backfill", CARGO_SEG_PASS).with_summary();
    let files = vec!["app/src/lib.rs".to_owned()];
    run(&ws, &files).await.expect("回填链应成功");
    let summary = read_summary(&ws);
    let mutation = summary["mutation"].as_object().expect("mutation 位已回填");
    assert_eq!(mutation["score"], 90.0, "回填归并块分值");
    assert_eq!(mutation["pass"], true);
    assert_eq!(mutation["threshold"], 80.0);

    // 不在位面：ChainWs 无 summary → 不回填不崩（run 已在 pass 用例覆盖——此处
    // 断言 summary 路径缺席下 run 仍 Ok）
    let ws = ChainWs::new("no-summary", CARGO_SEG_PASS);
    run(&ws, &files).await.expect("summary 不在位不崩");
    assert!(
        !foundation::layout::change_test_reports(&ws.root, ws.change)
            .join("summary.json")
            .exists(),
        "summary 不在位保持缺席（null 观察期语义不变）"
    );
}

// ---------------------------------------------------------------------------
// 执行链（假 spawn）用例：js suite 全链 / 段中断 / 多 suite 多阈值 / 组合根链路
// ---------------------------------------------------------------------------

/// 正向：**js suite 全链（Stryker）**——vitest suite 零预置段报告（不走复用门）
/// 经假 spawn 真执行 npx 段 → `ToolStepOutput::MutationTest` 载荷全字段 + 段级 /
/// 归并报告落盘 + summary mutation 位回填；记录面 argv 逐参（`npx --prefix
/// <suite cwd> stryker run <config>`）与 spawn cwd 断言（AC-4 js 族 → Stryker /
/// E11；执行链按 Mock 表走假 spawn）。
#[tokio::test]
async fn 执行链js_suite全链stryker假spawn产出() {
    let window = SpawnWindow::enter("js-chain");
    window.install_npx();
    let ws = ChainWs::with_config("js-chain", VITEST_CONFIG).with_summary();
    let suite_cwd = ws.root.join("app");
    fs::create_dir_all(&suite_cwd).expect("建 suite 执行目录");
    let reports = foundation::layout::change_test_reports(&ws.root, ws.change);
    let mutation_dir = reports.join("app_vitest").join("mutation");
    let report_path = mutation_dir.join("mutation-seg-0.json");
    window.set_out(STRYKER_SEG_PASS);
    window.set_report(&report_path);

    let files = vec!["app/src/a.ts".to_owned()];
    let output = run(&ws, &files).await.expect("js 全链应成功");
    let outcome = outcome_of(output);
    assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);
    assert_eq!(outcome.skip, None, "已执行形态 skip 恒 None");
    assert_eq!((outcome.score, outcome.threshold), (90.0, 80.0));
    assert_eq!(
        (
            outcome.counts.killed,
            outcome.counts.survived,
            outcome.counts.timeout,
            outcome.counts.compile_error,
            outcome.counts.runtime_error,
            outcome.counts.ignored,
            outcome.counts.total,
        ),
        (9, 1, 0, 0, 0, 0, 10),
        "Stryker metrics 面归位计数"
    );
    assert!(
        outcome.feedback_text.is_empty(),
        "Pass 恒空串（达标零反馈）"
    );
    assert_eq!(outcome.report_paths.len(), 2, "段级 + 归并报告");
    assert!(
        outcome.report_paths[0].ends_with("app_vitest/mutation/mutation-seg-0.json"),
        "段级报告落位: {:?}",
        outcome.report_paths
    );
    assert!(
        outcome.report_paths[1].ends_with("app_vitest/mutation/mutation.json"),
        "归并块报告落位: {:?}",
        outcome.report_paths
    );
    assert!(
        outcome.report_paths.iter().all(|path| !path.contains('\\')),
        "报告路径 POSIX 形态"
    );

    // summary mutation 位回填（E11 在位面）
    let summary = read_summary(&ws);
    let mutation = summary["mutation"].as_object().expect("mutation 位已回填");
    assert_eq!(mutation["score"], 90.0);
    assert_eq!(mutation["pass"], true);
    assert_eq!(mutation["threshold"], 80.0);

    // 假 spawn 执行链证据：恰一次 spawn（非复用门路径）+ argv 逐参 + cwd
    let records = window.records();
    assert_eq!(records.len(), 1, "js 段恰一次假 spawn（执行链非复用门）");
    let config_path = mutation_dir.join("mutation-stryker-seg-0.conf.json");
    assert_eq!(
        records[0].argv,
        vec![
            "npx".to_owned(),
            "--prefix".to_owned(),
            to_posix(&suite_cwd),
            "stryker".to_owned(),
            "run".to_owned(),
            to_posix(&config_path),
        ],
        "spawn 模板展开 argv 逐参等值"
    );
    let cwd = records[0]
        .cwd
        .clone()
        .expect("spawn cwd 捕获")
        .replace('\\', "/");
    assert_eq!(cwd, to_posix(&suite_cwd), "spawn cwd = suite 执行目录");
}

/// 异常：**段中断 → `conclusion = Error`**（双时点②即时反馈面）——suite1
/// （app）经假 spawn 正常完成产报告，suite2（src-tauri）假 spawn 非零退出且无
/// JSON 段报告 → 中断：错误摘要 + 已产报告路径清单进 `feedback_text`、不等汇总
/// （summary mutation 位保持 null、部分结论不落位）（AC-6 / AC-7）。
#[tokio::test]
async fn 段中断_conclusion_error即时反馈不等汇总() {
    let window = SpawnWindow::enter("interrupt");
    let ws = ChainWs::with_config("interrupt", TWO_RUST_CONFIG).with_summary();
    window.install_fake_cargo(&ws.root.join("app"));
    window.install_fake_cargo(&ws.root.join("src-tauri"));
    window.set_cargo_outcomes(CARGO_SEG_PASS); // suite1 面：90 ≥ 80 达标完成（报告文件落位）
    window.set_alt("src-tauri_rust", SEG_INTERRUPT_GARBAGE, Some(101)); // suite2 面：非零退出 + 零段报告（ALT 命中不写 outcomes）

    let files = vec![
        "app/src/lib.rs".to_owned(),
        "src-tauri/src/main.rs".to_owned(),
    ];
    let output = run(&ws, &files).await.expect("中断面产 Ok 载荷（非 Err）");
    let outcome = outcome_of(output);

    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Error,
        "段中断即时收束（不等汇总）"
    );
    assert_eq!(outcome.skip, None, "中断面非跳过");
    assert_eq!(
        (outcome.score, outcome.threshold),
        (90.0, 80.0),
        "已产部分仅 suite1 归并块、阈值取首块"
    );
    assert_eq!(
        (
            outcome.counts.killed,
            outcome.counts.survived,
            outcome.counts.total,
        ),
        (9, 1, 10)
    );

    let feedback = &outcome.feedback_text;
    assert!(
        feedback.contains("突变执行中断"),
        "错误摘要在案: {feedback}"
    );
    assert!(
        feedback.contains("退出码 101") && feedback.contains("未产出可解析 JSON 段报告"),
        "记因面（非零退出 + 无段报告）: {feedback}"
    );
    assert!(
        feedback.contains("已产报告路径")
            && feedback.contains("mutation-seg-0.json")
            && feedback.contains("mutation.json"),
        "已产报告路径清单（报告指引式）: {feedback}"
    );

    // report_paths 恰为 suite1 的段级 + 归并报告（suite2 中断不产）
    assert_eq!(outcome.report_paths.len(), 2, "{:?}", outcome.report_paths);
    assert!(
        outcome.report_paths.iter().all(|path| !path.contains('\\')),
        "报告路径 POSIX 形态"
    );

    // 不等汇总：summary mutation 位保持 null（E11 部分结论不落位）
    let summary = read_summary(&ws);
    assert!(
        summary["mutation"].is_null(),
        "中断面 MUST NOT 回填 summary mutation 位"
    );
    // rust 桶中断零保留（保留面只属 Stryker 失败桶的临时产物）
    assert!(
        !outcome.feedback_text.contains("保留的失败段"),
        "rust 中断无保留段: {}",
        outcome.feedback_text
    );

    // 假 spawn 执行链证据：两 suite 各一次、argv 携各自 plan 段补丁路径
    let records = window.records();
    assert_eq!(records.len(), 2, "两 suite 各恰一次假 spawn");
    assert_eq!(records[0].argv[0], "mutants");
    assert_eq!(
        records[0].argv[1], "--workspace",
        "workspace 突变面（混合 workspace 全成员）"
    );
    assert!(
        records[0].argv[3].contains("app_rust"),
        "suite1 段补丁路径: {:?}",
        records[0].argv
    );
    assert!(
        records[1].argv[3].contains("src-tauri_rust"),
        "suite2 段补丁路径: {:?}",
        records[1].argv
    );
}

/// 边界：**多 suite 多阈值（80 / 70）逐 suite 判定**——全过 → `Pass` 零反馈；
/// 有败 → `Fail` 即使归并块（24/30 = 80 ≥ 首块阈值 80）判 pass（门禁结论以逐
/// suite 全过为准，风险 6 留痕）；归并块阈值取首块（AC-4 / AC-5 归并口径）。
#[tokio::test]
async fn 逐suite多阈值判定_全过pass有败fail首块阈值() {
    let window = SpawnWindow::enter("multi-threshold");
    let files = vec![
        "app/src/lib.rs".to_owned(),
        "src-tauri/src/main.rs".to_owned(),
    ];

    // run1：全过（app 90 ≥ 80、src-tauri 75 ≥ 70）
    let ws1 = ChainWs::with_config("all-pass", TWO_RUST_CONFIG);
    window.install_fake_cargo(&ws1.root.join("app"));
    window.install_fake_cargo(&ws1.root.join("src-tauri"));
    window.set_cargo_outcomes(CARGO_SEG_PASS); // 90
    window.set_cargo_outcomes_alt("src-tauri_rust", &cargo_seg(15, 5)); // 75
    let all_pass = outcome_of(run(&ws1, &files).await.expect("全过面应 Ok"));
    assert_eq!(all_pass.conclusion, MutationTestConclusion::Pass);
    assert!(
        all_pass.feedback_text.is_empty(),
        "达标零反馈: {}",
        all_pass.feedback_text
    );
    assert_eq!(
        (all_pass.score, all_pass.threshold),
        (80.0, 80.0),
        "归并 (24+6)/30 = 80.0、阈值取首块 80"
    );
    assert_eq!(
        (
            all_pass.counts.killed,
            all_pass.counts.survived,
            all_pass.counts.total,
        ),
        (24, 6, 30),
        "归并计数 = 段计数求和"
    );
    assert_eq!(all_pass.report_paths.len(), 4, "两 suite 各段级 + 归并报告");
    assert_eq!(window.records().len(), 2, "两 suite 各一次假 spawn");

    // run2：app 50 < 80 → Fail（src-tauri 95 ≥ 70 过）
    window.reset_records();
    let ws2 = ChainWs::with_config("has-fail", TWO_RUST_CONFIG);
    window.install_fake_cargo(&ws2.root.join("app"));
    window.install_fake_cargo(&ws2.root.join("src-tauri"));
    window.set_cargo_outcomes(CARGO_SEG_FAIL); // 50
    window.set_cargo_outcomes_alt("src-tauri_rust", &cargo_seg(19, 1)); // 95
    let has_fail = outcome_of(run(&ws2, &files).await.expect("有败面应 Ok"));
    assert_eq!(
        has_fail.conclusion,
        MutationTestConclusion::Fail,
        "逐 suite 有败即 Fail"
    );
    assert_eq!((has_fail.score, has_fail.threshold), (80.0, 80.0));
    assert!(
        has_fail.score >= has_fail.threshold,
        "归并块判 pass 却仍 Fail——门禁以逐 suite 全过为准（风险 6 留痕）"
    );
    let feedback = &has_fail.feedback_text;
    assert!(
        feedback.contains("- app_rust: score 50 < threshold 80"),
        "未达标 suite 明细: {feedback}"
    );
    assert!(
        !feedback.contains("- src-tauri_rust"),
        "达标 suite 不入未达标清单: {feedback}"
    );
    assert!(
        feedback.contains("原始报告路径"),
        "报告路径清单: {feedback}"
    );
    assert_eq!(window.records().len(), 2, "两 suite 各一次假 spawn");
}

// ---------------------------------------------------------------------------
// 组合根链路（LocalToolSteps → vcs 假件 → ProcessMutation 假 spawn）
// ---------------------------------------------------------------------------

/// 组合链路假 vcs（`ArchiveVcsPort` 进程内假件——记录 `(方法, root, base)` 双参
/// 透传，清单 / 补丁可编程产出；归档 git 缝不触即炸）。
struct FakeVcs {
    files: Vec<String>,
    patch: String,
    calls: Mutex<Vec<(&'static str, String, String)>>,
}

impl FakeVcs {
    fn new(files: Vec<String>, patch: &str) -> Self {
        Self {
            files,
            patch: patch.to_owned(),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// 调用记录快照（方法名 / root POSIX / base 透传断言面）。
    fn calls(&self) -> Vec<(&'static str, String, String)> {
        self.calls.lock().expect("calls 锁不可中毒").clone()
    }
}

impl ArchiveVcsPort for FakeVcs {
    fn change_edit_set(&self, root: &Path, base: &str) -> Result<Vec<String>, String> {
        self.calls.lock().expect("calls 锁不可中毒").push((
            "change_edit_set",
            to_posix(root),
            base.to_owned(),
        ));
        Ok(self.files.clone())
    }

    fn change_edit_patch(&self, root: &Path, base: &str) -> Result<String, String> {
        self.calls.lock().expect("calls 锁不可中毒").push((
            "change_edit_patch",
            to_posix(root),
            base.to_owned(),
        ));
        Ok(self.patch.clone())
    }

    fn dirty(&self, _root: &Path, _paths: &[&str]) -> bool {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn commit_all(&self, _worktree: &Path, _message: &str) -> Result<(), String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn branch_merged(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn rebase_branch(&self, _worktree: &Path, _onto: &str) -> Result<RebaseOutcome, String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn ff_merge(&self, _main_root: &Path, _branch: &str) -> Result<(), String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn worktree_snapshot(&self, _root: &Path) -> Result<WorktreeSnapshot, String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn rebase_continue(&self, _worktree: &Path) -> Result<RebaseOutcome, String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn rebase_abort(&self, _worktree: &Path) -> Result<(), String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn current_branch(&self, _main_root: &Path) -> Result<String, String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn commit_paths(
        &self,
        _main_root: &Path,
        _paths: &[&str],
        _message: &str,
    ) -> Result<(), String> {
        unimplemented!("组合根链路不触归档 git缝")
    }
    fn remove_worktree(&self, _main_root: &Path, _worktree: &Path) -> Result<(), String> {
        unimplemented!("组合根链路不触归档 git 缝")
    }
    fn restore_baseline(&self, _worktree: &Path, _exclusions: &[&str]) -> Result<String, String> {
        unimplemented!("组合根链路不触基线恢复缝")
    }
}

/// 组合链路假 store（`ChangeStateStore` 进程内假件——MutationTest 分支只触
/// `get_change`（id → 记录 → name 解析）与 `append_step`（审计落行）两缝，
/// 其余读写面触发即用例错位显式炸出）。
struct FakeStore {
    change: Mutex<Option<ChangeStateRecord>>,
    steps: Mutex<Vec<StepStateRecord>>,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            change: Mutex::new(None),
            steps: Mutex::new(Vec::new()),
        }
    }

    /// 建档种子（id 归键、name 独立展示——id ≠ name 形态下解析面可辨）。
    fn seed_change(&self, id: &str, name: &str) {
        *self.change.lock().expect("change 锁不可中毒") = Some(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: 0,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        });
    }

    /// 审计行回读（`append_step` 捕获面）。
    fn audit_rows(&self) -> Vec<StepStateRecord> {
        self.steps.lock().expect("steps 锁不可中毒").clone()
    }
}

impl ChangeStateStore for FakeStore {
    fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self
            .change
            .lock()
            .expect("change 锁不可中毒")
            .clone()
            .filter(|record| record.id == id))
    }

    fn append_step(&self, command: &StepCommand) -> Result<(), StoreFault> {
        let mut steps = self.steps.lock().expect("steps 锁不可中毒");
        let id = steps.len() as i64 + 1;
        steps.push(StepStateRecord {
            id,
            run_id: command.run_id.clone(),
            change_id: command.change_id.clone(),
            step_kind: command.step_kind,
            status: command.status.clone(),
            timestamp: command.timestamp,
            summary: command.summary.clone(),
            reference: command.reference.clone(),
        });
        Ok(())
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("组合根链路不触该读面")
    }
    fn list_phase_records(&self, _change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        unimplemented!("组合根链路不触该读面")
    }
    fn list_steps(
        &self,
        _change_id: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        unimplemented!("组合根链路不触该读面")
    }
    fn create_change_record(&self, _record: ChangeStateRecord) -> Result<(), StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
    fn delete_change_record(&self, _id: &str) -> Result<bool, StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
    fn start_phase(
        &self,
        _change_id: &str,
        _phase: &str,
        _now: i64,
    ) -> Result<PhaseStartState, StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
    fn log_phase(&self, _command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
    fn apply_backtrack(&self, _command: &BacktrackCommand) -> Result<(), StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
    fn amend_decision_session(
        &self,
        _change_id: &str,
        _phase: &str,
        _session_id: &str,
    ) -> Result<(), StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
    fn set_archived(&self, _id: &str, _archived_at: i64) -> Result<(), StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
    fn list_runs(&self, _change_id: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
        unimplemented!("组合根链路不触该读面")
    }
    fn list_run_nodes(&self, _run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreFault> {
        unimplemented!("组合根链路不触该读面")
    }
    fn list_run_edges(&self, _run_id: &str) -> Result<Vec<RunEdgeStateRecord>, StoreFault> {
        unimplemented!("组合根链路不触该读面")
    }
    fn run_start(&self, _command: &RunStartCommand) -> Result<(), StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
    fn run_finish(&self, _command: &RunFinishCommand) -> Result<(), StoreFault> {
        unimplemented!("组合根链路不触该写面")
    }
}

/// 组合链路未触缝占位假件（MutationTest 分支不触发 static-check /
/// test-execution 两缝——触发即用例错位显式炸出）。
struct UnusedSeamRunner;

/// 组合链路未触缝占位假件（本变更新形参的编译级补位：MutationTest 分支不触发
/// archi 门禁缝——触发即用例错位显式炸出）。
struct UnusedArchiSeam;

impl ArchiCheckRunner for UnusedArchiSeam {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _changed: Vec<String>,
        _cancel: foundation::cancel::CancelFlag,
    ) -> BoxToolFuture {
        let error = "组合根链路不触架构漂移门禁缝".to_owned();
        Box::pin(async move { Err::<ToolStepOutput, String>(error) })
    }
}

impl StaticCheckRunner for UnusedSeamRunner {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _slot: foundation::layout::ReportSlot,
        _cancel: foundation::cancel::CancelFlag,
    ) -> BoxToolFuture {
        let error = "组合根链路不触静态检查缝".to_owned();
        Box::pin(async move { Err::<ToolStepOutput, String>(error) })
    }
}

impl TestExecutionRunner for UnusedSeamRunner {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _slot: foundation::layout::ReportSlot,
        _cancel: foundation::cancel::CancelFlag,
    ) -> BoxToolFuture {
        let error = "组合根链路不触测试执行缝".to_owned();
        Box::pin(async move { Err::<ToolStepOutput, String>(error) })
    }
}

/// 组合：**组合根链路**——`LocalToolSteps` MutationTest 分支 → fake vcs（清单 /
/// 补丁唯一入口假件，`base` 恒 `HEAD`）→ 真实 `ProcessMutation::run` 四参（假
/// spawn 主链）→ outcome 载荷逐字段断言；id → 记录 → name 解析（报告目录携
/// `demo-change`）、vcs 补丁切片 + suite 根前缀剥离注入 `--in-diff` 段补丁、
/// 审计行落账（AC-3 / E3 / E11）。
#[tokio::test]
async fn 组合根链路_localtoolsteps_vcs假件_processmutation假spawn() {
    let window = SpawnWindow::enter("combo-root");
    let ws = ChainWs::with_config("combo-root", RUST_APP_CONFIG);
    window.install_fake_cargo(&ws.root.join("app"));
    window.set_cargo_outcomes(CARGO_SEG_PASS);

    let vcs = Arc::new(FakeVcs::new(vec!["app/src/lib.rs".to_owned()], COMBO_PATCH));
    let store = Arc::new(FakeStore::new());
    store.seed_change(COMBO_CHANGE_ID, "demo-change");
    let steps = LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(UnusedSeamRunner),
        Arc::new(UnusedSeamRunner),
        Arc::new(ProcessMutation::new()),
        Arc::new(UnusedArchiSeam),
        Arc::clone(&vcs) as Arc<dyn ArchiveVcsPort>,
        Arc::clone(&store) as Arc<dyn ChangeStateStore>,
        "run-combo-0001".to_owned(),
    );

    let output = steps
        .run(ToolStepRequest {
            root: ws.root_str(),
            cancel: foundation::cancel::CancelFlag::never(),
            command: ToolCommand::MutationTest {
                change_id: COMBO_CHANGE_ID.to_owned(),
            },
        })
        .await
        .expect("组合根链路应成功");
    let outcome = outcome_of(output);

    // outcome 载荷逐字段（真实 execute 主链 + 假 spawn）
    assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);
    assert_eq!(outcome.skip, None, "fake vcs 清单入桶——非 EmptyDiff");
    assert_eq!((outcome.score, outcome.threshold), (90.0, 80.0));
    assert_eq!(
        (
            outcome.counts.killed,
            outcome.counts.survived,
            outcome.counts.timeout,
            outcome.counts.compile_error,
            outcome.counts.runtime_error,
            outcome.counts.ignored,
            outcome.counts.total,
        ),
        (9, 1, 0, 0, 0, 0, 10)
    );
    assert!(outcome.feedback_text.is_empty(), "Pass 恒空串");
    assert_eq!(outcome.report_paths.len(), 2, "段级 + 归并报告");

    // name 解析：报告目录经记录 name（demo-change）派生（id ≠ name 可辨）
    let reports = foundation::layout::change_test_reports(&ws.root, "demo-change");
    assert!(
        outcome.report_paths[0].starts_with(&to_posix(&reports))
            && outcome.report_paths[0].contains("/demo-change/"),
        "报告目录携 name: {:?}",
        outcome.report_paths
    );

    // vcs 双参透传（清单 / 补丁唯一入口、base 恒 HEAD，各恰一次）
    let root_posix = to_posix(&ws.root);
    assert_eq!(
        vcs.calls(),
        vec![
            ("change_edit_set", root_posix.clone(), "HEAD".to_owned()),
            ("change_edit_patch", root_posix, "HEAD".to_owned()),
        ],
        "vcs 取数双缝按序各一次"
    );

    // 段补丁 = vcs 补丁切片 + suite 根前缀剥离（清单匹配恒项目根相对、落盘段
    // 补丁路径剥 suite 根——cargo mutants 以 spawn cwd 相对路径匹配 `--in-diff`）
    let seg_patch = reports
        .join("app_rust")
        .join("mutation")
        .join("mutation-seg-0.patch");
    assert_eq!(
        fs::read_to_string(&seg_patch).expect("段补丁在盘"),
        COMBO_PATCH_STRIPPED,
        "vcs 补丁经切片 + suite 根前缀剥离注入 --in-diff 段补丁"
    );

    // 假 spawn 主链在位（执行链按 Mock 表走假 spawn）
    let records = window.records();
    assert_eq!(records.len(), 1, "真实 execute 主链恰一次假 spawn");
    assert_eq!(records[0].argv[0], "mutants", "argv 首 token");
    assert_eq!(records[0].argv[1], "--workspace", "workspace 突变面");
    assert_eq!(records[0].argv[2], "--in-diff", "段补丁入参位");

    // 审计行（StepKind::MutationTest，摘要携 conclusion / score / threshold）
    let rows = store.audit_rows();
    assert_eq!(rows.len(), 1, "恰一条审计行");
    assert_eq!(rows[0].step_kind.as_str(), "mutation_test");
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].run_id, "run-combo-0001");
    assert_eq!(rows[0].change_id, COMBO_CHANGE_ID);
    assert!(
        rows[0]
            .summary
            .contains("mutation_test conclusion=pass score=90 threshold=80"),
        "审计摘要: {}",
        rows[0].summary
    );
}

/// 边界（E10）：suite 归桶——文件按 suite.root POSIX 前缀归桶（最长前缀匹
/// 配）、跨 suite 文件只归一桶、桶外文件不入桶、全空桶 → EmptyDiff 判定归
/// 调用方；锚点归一（`.` / 空串 = 全匹配锚）。
#[test]
fn suite归桶_前缀匹配与跨桶单归与锚点归一() {
    let root = Path::new("/ws");
    let suites = vec![
        config::TestSuite {
            root: "app".to_owned(),
            framework: TestFramework::Rust,
            cwd: ".".to_owned(),
            config: None,
            includes: None,
            excludes: None,
            coverage: config::CoverageThresholds {
                lines: 80.0,
                branches: 70.0,
                functions: 75.0,
            },
            mutation: MutationConfig {
                cwd: None,
                score: 80.0,
            },
        },
        config::TestSuite {
            root: "app/nested".to_owned(),
            framework: TestFramework::Vitest,
            cwd: ".".to_owned(),
            config: None,
            includes: None,
            excludes: None,
            coverage: config::CoverageThresholds {
                lines: 80.0,
                branches: 70.0,
                functions: 75.0,
            },
            mutation: MutationConfig {
                cwd: None,
                score: 70.0,
            },
        },
    ];
    let files = vec![
        "app/src/lib.rs".to_owned(),
        "app/nested/deep.ts".to_owned(),
        "outside/other.rs".to_owned(),
        // Windows 反斜杠入参归一 POSIX 后归桶
        "app\\src\\win.rs".to_owned(),
    ];
    let buckets = bucket_suites(root, &suites, &files);

    // 最长前缀匹配：app/nested/deep.ts 归 nested 桶（最具体）、app/src/* 归 app 桶
    assert_eq!(
        buckets[0].files,
        vec!["app/src/lib.rs", "app/src/win.rs"],
        "app 桶（POSIX 归一后排序去重）"
    );
    assert_eq!(
        buckets[1].files,
        vec!["app/nested/deep.ts"],
        "nested 桶（最长前缀）"
    );
    // plan id / 框架 / 阈值 / 执行器绑定面
    assert_eq!(
        buckets[0].plan_id, "app_rust",
        "plan id = derive_plan_id(root, framework)"
    );
    assert_eq!(buckets[1].plan_id, "app_nested_vitest");
    assert_eq!(buckets[0].threshold, 80.0, "逐 suite 阈值");
    assert_eq!(buckets[1].threshold, 70.0);
    assert_eq!(
        buckets[0].tool,
        Some(MutationTool::CargoMutants),
        "rust → cargo-mutants"
    );
    assert_eq!(
        buckets[1].tool,
        Some(MutationTool::Stryker),
        "vitest → Stryker"
    );

    // 全空桶 → 调用方判 EmptyDiff（桶外文件不突变）
    let outside_buckets = bucket_suites(root, &suites, &["ghost/x.rs".to_owned()]);
    assert!(
        outside_buckets.iter().all(|bucket| bucket.files.is_empty()),
        "桶外文件零归桶（E10）"
    );

    // 锚点归一：`.` / 空串 / 尾斜杠 → 全匹配锚
    assert_eq!(normalize_root("."), ".", "`.` 恒全匹配锚");
    assert_eq!(normalize_root(""), ".", "空串归一 `.`");
    assert_eq!(normalize_root("app/"), "app", "尾斜杠剥离");
    assert_eq!(normalize_root("app\\"), "app", "反斜杠归一 POSIX");
}

/// 边界（AC-4）：突变执行框架矩阵——rust → cargo-mutants、js 族（jest /
/// vitest / vite-plus）→ Stryker、node-test / bun / go / pytest → None（跳过
/// 记 evidence 不 Err）；框架线格式名逐字。
#[test]
fn 执行框架矩阵_三族封闭() {
    assert_eq!(
        mutation_tool(&TestFramework::Rust),
        Some(MutationTool::CargoMutants)
    );
    assert_eq!(
        mutation_tool(&TestFramework::Jest),
        Some(MutationTool::Stryker)
    );
    assert_eq!(
        mutation_tool(&TestFramework::Vitest),
        Some(MutationTool::Stryker)
    );
    assert_eq!(
        mutation_tool(&TestFramework::VitePlus),
        Some(MutationTool::Stryker)
    );
    for framework in [
        TestFramework::NodeTest,
        TestFramework::Bun,
        TestFramework::Go,
        TestFramework::Pytest,
    ] {
        assert_eq!(
            mutation_tool(&framework),
            None,
            "{framework:?} 无突变执行器（跳过记 evidence 不 Err）"
        );
    }
    // 框架线格式名逐字（CLI 注册表键同串）
    assert_eq!(framework_name(&TestFramework::Rust), "rust");
    assert_eq!(framework_name(&TestFramework::VitePlus), "vite-plus");
    assert_eq!(framework_name(&TestFramework::NodeTest), "node-test");
}

/// 边界（E9）：突变常量值锚定（D9 不进 config schema——`MutationConfig` 仅
/// cwd / score 两字段，本用例同时锚定 schema 零新字段面）。
#[test]
fn 突变常量锚定与config_schema零新字段() {
    assert_eq!(MUTATION_JOBS_DIVISOR, 2, "并发除数 2");
    assert_eq!(PER_MUTANT_TIMEOUT_SECS, 300, "per-mutant 超时 300 秒");
    assert_eq!(SEGMENT_BUDGET_MS, 300_000, "段预算 5 分钟");
    assert_eq!(SEGMENT_GUARD_MS, 600_000, "软护栏 10 分钟");
    assert_eq!(EST_MUTANTS_PER_FILE, 10, "首段静态单文件密度");

    // MutationConfig schema 零新字段（穷尽解构无 `..`——D9 不进 config schema）
    let config = MutationConfig {
        cwd: None,
        score: 80.0,
    };
    let MutationConfig { cwd, score } = config;
    assert_eq!(cwd, None);
    assert_eq!(score, 80.0);
}

/// 跳过面产出形态（`skip_outcome` 直测）：结论恒 Pass、0.0 占位、零清单零
/// 反馈（null 观察期载荷面单点）。
#[test]
fn 跳过面产出形态_恒pass零占位() {
    for reason in [
        MutationSkipReason::EmptyDiff,
        MutationSkipReason::NoExecutor,
    ] {
        let output = skip_outcome(reason);
        let outcome = outcome_of(output);
        assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);
        assert_eq!(outcome.skip, Some(reason));
        assert_eq!((outcome.score, outcome.threshold), (0.0, 0.0));
        assert!(outcome.report_paths.is_empty() && outcome.feedback_text.is_empty());
    }
}

/// 段级解析路由（`parse_seg` 消费面）与产出模型面：`MutationSegReport` 三字段
/// / `MutationTool` 两值自持（纯层 fixtures 直驱——E4 零 spawn 零 Tauri）。
#[test]
fn 段级解析路由与产出模型面() {
    let cargo =
        super::parse_seg(MutationTool::CargoMutants, CARGO_SEG_PASS).expect("cargo 段解析应成功");
    assert_eq!(cargo.tool, MutationTool::CargoMutants);
    assert_eq!(cargo.measured.killed, 9);
    // 真实 outcomes.json 无 durationMs 键 → 0.0（自适应速率回落静态初值——
    // run_suite「工具未携 → 静态初值续用」既有口径）
    assert_eq!(cargo.duration_ms, 0.0);

    let stryker_raw = r#"{ "metrics": { "killed": 3, "survived": 1, "totalMutants": 4 } }"#;
    let stryker =
        super::parse_seg(MutationTool::Stryker, stryker_raw).expect("stryker 段解析应成功");
    assert_eq!(stryker.tool, MutationTool::Stryker);
    assert_eq!(stryker.measured.total, 4);

    // 解析失败显式 Err（结构面严格——路由不吞）
    assert!(super::parse_seg(MutationTool::CargoMutants, "broken").is_err());
}

// ---------------------------------------------------------------------------
// 归桶过滤（mutate 清单消费面）：excludes 全桶 + Stryker 桶可插桩/测试文件
// 两规则（tool-scoped）——空桶跳过语义与假绿防线
// ---------------------------------------------------------------------------

/// 夹具小助手：suite 构造（excludes 缺省 None）。
fn filter_suite(root: &str, framework: TestFramework, excludes: Vec<&str>) -> config::TestSuite {
    config::TestSuite {
        root: root.to_owned(),
        framework,
        cwd: ".".to_owned(),
        config: None,
        includes: None,
        excludes: (!excludes.is_empty()).then(|| excludes_into_strings(excludes)),
        coverage: config::CoverageThresholds {
            lines: 80.0,
            branches: 70.0,
            functions: 75.0,
        },
        mutation: MutationConfig {
            cwd: None,
            score: 80.0,
        },
    }
}

/// `Vec<&str>` → `Vec<String>`。
fn excludes_into_strings(values: Vec<&str>) -> Vec<String> {
    values.into_iter().map(str::to_owned).collect()
}

/// 纯层：可插桩扩展名 allowlist + 测试文件剔除**仅 Stryker 桶**生效——vitest
/// 桶滤掉不可插桩扩展名与 `.test./.spec./__tests__` 标记，源文件留桶；rust 桶
///（cargo-mutants `--in-diff` 切片自滤）不受扩展名/测试文件约束。
#[test]
fn 归桶过滤_可插桩扩展名与测试文件仅stryker桶剔除() {
    let root = Path::new("/ws");
    let suites = vec![filter_suite("app", TestFramework::Vitest, Vec::new())];
    let files = vec![
        "app/src/a.ts".to_owned(),
        "app/src/b.tsx".to_owned(),
        "app/src/c.vue".to_owned(),
        "app/README.md".to_owned(),
        "app/package.json".to_owned(),
        "app/src/styles.css".to_owned(),
        "app/src/logo.png".to_owned(),
        "app/src/a.test.ts".to_owned(),
        "app/src/b.spec.tsx".to_owned(),
        "app/src/__tests__/helper.ts".to_owned(),
    ];
    let buckets = bucket_suites(root, &suites, &files);
    assert_eq!(
        buckets[0].files,
        vec![
            "app/src/a.ts".to_owned(),
            "app/src/b.tsx".to_owned(),
            "app/src/c.vue".to_owned(),
        ],
        "可插桩源文件留桶、不可插桩扩展名与测试文件剔除"
    );

    // rust 桶不受约束（`.rs` / 测试文件照入——cargo-mutants --in-diff 自滤）
    let suites = vec![filter_suite("app", TestFramework::Rust, Vec::new())];
    let files = vec![
        "app/src/lib.rs".to_owned(),
        "app/README.md".to_owned(),
        "app/tests/x.rs".to_owned(),
    ];
    let buckets = bucket_suites(root, &suites, &files);
    assert_eq!(
        buckets[0].files,
        vec![
            "app/README.md".to_owned(),
            "app/src/lib.rs".to_owned(),
            "app/tests/x.rs".to_owned(),
        ],
        "rust 桶零扩展名/测试文件约束"
    );
}

/// 纯层：`suite.excludes` 全桶消费（CLI `isFileExcluded` 口径）——vitest 与
/// rust 桶各自的 excludes 命中即不入桶。
#[test]
fn 归桶过滤_excludes全桶消费() {
    let root = Path::new("/ws");
    let suites = vec![
        filter_suite("app", TestFramework::Vitest, vec!["src/generated/**"]),
        filter_suite("src-tauri", TestFramework::Rust, vec!["**/*_schema.rs"]),
    ];
    let files = vec![
        "app/src/keep.ts".to_owned(),
        "app/src/generated/bindings.ts".to_owned(),
        "src-tauri/src/keep.rs".to_owned(),
        "src-tauri/src/foo_schema.rs".to_owned(),
    ];
    let buckets = bucket_suites(root, &suites, &files);
    assert_eq!(
        buckets[0].files,
        vec!["app/src/keep.ts".to_owned()],
        "vitest 桶 excludes 命中剔除"
    );
    assert_eq!(
        buckets[1].files,
        vec!["src-tauri/src/keep.rs".to_owned()],
        "rust 桶 excludes 命中剔除（全桶口径）"
    );
}

/// 全链：Stryker 桶全滤空（只喂不可插桩 / 测试文件）→ `EmptyDiff` skip（Pass、
/// mutation 位 null、**零 spawn**——dry-run 0 mutant → score 100.0 假绿路径
/// 不可达）。
#[tokio::test]
async fn 全桶滤空_emptydiff零spawn假绿防线() {
    let window = SpawnWindow::enter("filter-empty");
    let ws = ChainWs::with_config("filter-empty", VITEST_CONFIG).with_summary();
    let files = vec![
        "app/README.md".to_owned(),
        "app/package.json".to_owned(),
        "app/src/a.test.ts".to_owned(),
    ];
    let output = run(&ws, &files).await.expect("空突变面产 Ok 载荷");
    let outcome = outcome_of(output);
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Pass,
        "跳过面恒 Pass"
    );
    assert_eq!(
        outcome.skip,
        Some(MutationSkipReason::EmptyDiff),
        "全滤空 → EmptyDiff（mutation 位 null 观察期）"
    );
    assert_eq!(
        (outcome.score, outcome.threshold),
        (0.0, 0.0),
        "跳过面占位计"
    );
    assert!(
        outcome.report_paths.is_empty(),
        "零段零报告: {:?}",
        outcome.report_paths
    );
    assert!(window.records().is_empty(), "零 spawn（假绿路径不可达）");
    let summary = read_summary(&ws);
    assert!(
        summary["mutation"].is_null(),
        "跳过面 MUST NOT 回填 summary mutation 位"
    );
}

/// 全链：单桶滤空掉出 runnable（vitest 桶滤空、rust 桶照跑）——spawn 恰一次
/// 且 argv 只含 cargo mutants 面。
#[tokio::test]
async fn 单桶滤空掉出runnable_余桶照跑() {
    let window = SpawnWindow::enter("filter-partial");
    let config = r#"{ "tests": [
  { "root": "app", "framework": "vitest", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 80 } },
  { "root": "src-tauri", "framework": "rust", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 70 } }
] }"#;
    let ws = ChainWs::with_config("filter-partial", config).with_summary();
    window.install_fake_cargo(&ws.root.join("src-tauri"));
    window.set_cargo_outcomes(CARGO_SEG_PASS);

    // vitest 桶只喂不可插桩文件（滤空）；rust 桶正常
    let files = vec![
        "app/styles.css".to_owned(),
        "src-tauri/src/main.rs".to_owned(),
    ];
    let output = run(&ws, &files).await.expect("余桶照跑应成功");
    let outcome = outcome_of(output);
    assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);
    assert_eq!(outcome.skip, None, "有可跑桶非跳过");
    assert_eq!(
        (outcome.score, outcome.threshold),
        (90.0, 70.0),
        "rust 桶归并"
    );

    let records = window.records();
    assert_eq!(records.len(), 1, "恰一次 spawn（vitest 桶滤空零 spawn）");
    assert_eq!(
        records[0].argv[0], "mutants",
        "唯一 spawn 为 cargo-mutants 面"
    );
}

/// 全链：**Stryker 段失败 → 失败桶临时面保留**——临时 config + `_stryker-tmp`
/// 在盘、反馈面附保留段与两条路径（供单段复现）。
#[tokio::test]
async fn 中断保留失败段产物_反馈附路径() {
    let window = SpawnWindow::enter("retain-failure");
    let ws = ChainWs::with_config("retain-failure", VITEST_CONFIG).with_summary();
    window.install_npx();
    fs::create_dir_all(ws.root.join("app")).expect("建 suite 执行目录");
    // 假 npx 非零退出 + 无段报告（run_segment 写盘 config 后 spawn 崩面）
    window.set_out(SEG_INTERRUPT_GARBAGE);
    window.set_exit(101);
    // 预置 sandbox 目录（真 Stryker 建置面模拟）
    let mutation_dir = foundation::layout::change_test_reports(&ws.root, ws.change)
        .join("app_vitest")
        .join("mutation");
    fs::create_dir_all(mutation_dir.join("_stryker-tmp").join("sandbox-x")).expect("预置 sandbox");

    let files = vec!["app/src/a.ts".to_owned()];
    let output = run(&ws, &files).await.expect("中断面产 Ok 载荷");
    let outcome = outcome_of(output);
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Error,
        "段中断收束"
    );

    // 临时面在盘（config 由 run_segment 写盘、sandbox 预置——cleanup 跳过保留）
    let config_path = mutation_dir.join("mutation-stryker-seg-0.conf.json");
    assert!(
        config_path.is_file(),
        "失败段临时 config 在盘: {}",
        to_posix(&config_path)
    );
    assert!(
        mutation_dir.join("_stryker-tmp").is_dir(),
        "失败桶 sandbox 在盘: {}",
        to_posix(&mutation_dir)
    );
    // 反馈面：保留段 + 两条路径（sandbox 在前 + config）
    let feedback = &outcome.feedback_text;
    assert!(
        feedback.contains("保留的失败段调试产物"),
        "保留段在案: {feedback}"
    );
    assert!(
        feedback.contains("_stryker-tmp") && feedback.contains("mutation-stryker-seg-0.conf.json"),
        "保留产物路径清单: {feedback}"
    );
    assert!(
        feedback.contains("已产报告路径"),
        "已产报告路径段保留: {feedback}"
    );
}

/// 全链：成功收束照清（保留面反面）——全段成功后临时 config 与 `_stryker-tmp`
/// 不在盘、报告工件原样在位。
#[tokio::test]
async fn 成功全段_临时产物照清() {
    let window = SpawnWindow::enter("retain-clean");
    let ws = ChainWs::with_config("retain-clean", VITEST_CONFIG);
    window.install_npx();
    fs::create_dir_all(ws.root.join("app")).expect("建 suite 执行目录");
    window.set_out(STRYKER_SEG_PASS);
    let mutation_dir = foundation::layout::change_test_reports(&ws.root, ws.change)
        .join("app_vitest")
        .join("mutation");
    let report_path = mutation_dir.join("mutation-seg-0.json");
    window.set_report(&report_path);
    // 预置 sandbox 目录（成功后应被清理）
    fs::create_dir_all(mutation_dir.join("_stryker-tmp").join("sandbox-x")).expect("预置 sandbox");

    let files = vec!["app/src/a.ts".to_owned()];
    let output = run(&ws, &files).await.expect("成功面应成功");
    let outcome = outcome_of(output);
    assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);

    assert!(
        !mutation_dir.join("_stryker-tmp").exists(),
        "成功收束 sandbox 照清"
    );
    let leftover: Vec<_> = std::fs::read_dir(&mutation_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with("mutation-stryker-seg-")
        })
        .collect();
    assert!(leftover.is_empty(), "成功收束临时 config 照清");
    assert!(report_path.is_file(), "段级报告工件原样在位");
    assert!(
        mutation_dir.join("mutation.json").is_file(),
        "归并报告工件原样在位"
    );
}

// ---------------------------------------------------------------------------
// 取消面（test-design「mutation/mod.rs -> mod_test.rs」节，AC-7 / D17）：
// `SegmentFailure::Cancelled` 第三变体（`Display` 三臂同型）+ 取消 → 执行面归因
// `fault = Some(RunFault::Stopped)` + `conclusion = Fail` 占位 + `feedback_text`
// 空串（stopped 面不被误读为「需修复」）。取消不走 `Fatal`（不把用户停止呈现为
// 「环境坏了停给用户」），也不走 `Interrupted` 的 `conclusion = Error` + 「突变
// 执行中断」文案（变体独立，两套文案互不混用）。
// ---------------------------------------------------------------------------

/// 经 `MutationRunner` trait 面带取消信号驱动（生产装配同路径，仅 flag 不同）。
async fn run_with_cancel(
    ws: &ChainWs,
    files: &[String],
    cancel: foundation::cancel::CancelFlag,
) -> Result<ToolStepOutput, String> {
    let runner = ProcessMutation::new();
    runner
        .run(
            &ws.root_str(),
            ws.change,
            files,
            "diff --git a/app/src/lib.rs b/app/src/lib.rs\n",
            cancel,
        )
        .await
}

/// 正向（AC-7 / 类型定义表 `SegmentFailure` 行，遍历枚举 3 变体）：`Display` 三臂
/// 同型——逐个 `to_string()` 等于载荷串（单串同型，无变体前缀）。
#[test]
fn segment_failure_display三臂同型() {
    let cases = [
        SegmentFailure::Fatal("a".to_owned()),
        SegmentFailure::Interrupted("b".to_owned()),
        SegmentFailure::Cancelled("c".to_owned()),
    ];
    for failure in cases {
        let expected = match &failure {
            SegmentFailure::Fatal(message)
            | SegmentFailure::Interrupted(message)
            | SegmentFailure::Cancelled(message) => message.clone(),
        };
        assert_eq!(
            failure.to_string(),
            expected,
            "Display 只呈现载荷串（无变体前缀，三臂同型）"
        );
    }
    // 空载荷边界：三臂一致地呈现空串（无 panic / 无占位改写）
    assert_eq!(SegmentFailure::Cancelled(String::new()).to_string(), "");
}

/// 正向（AC-7 组合锚，旧「收敛 failed / Error」断言随裁决改写）：取消 →
/// `fault = Some(RunFault::Stopped)` + `conclusion = Fail` 占位 + `feedback_text`
/// 空串（stopped 面不被误读为「需修复」）——自旋假 spawn 运行中置位；非 Fatal
/// `Err` 面，段报告 / summary 面不落位（中断不回填）。
#[tokio::test]
async fn 取消_fault为stopped且结论占位fail() {
    let window = SpawnWindow::enter("cancel-run");
    let ws = ChainWs::with_config("cancel-run", RUST_APP_CONFIG).with_summary();
    window.install_fake_cargo(&ws.root.join("app"));
    window.set_sleep(1); // 记录后自旋挂起（段执行体恒活）

    let files = vec!["app/src/lib.rs".to_owned()];
    let cancel = foundation::cancel::CancelFlag::never();
    let handle = cancel.handle();
    let runner_task = tokio::spawn(async move {
        let ws_root = ws.root_str();
        let ws_change = ws.change;
        let _keep = &ws;
        ProcessMutation::new()
            .run(
                &ws_root,
                ws_change,
                &files,
                "diff --git a/app/src/lib.rs b/app/src/lib.rs\n",
                cancel,
            )
            .await
    });

    // 等假 spawn 记录出现（运行中窗口）后置位取消
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while window.records().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "等待假 spawn 记录超时（20s）"
        );
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    handle.set();

    let output = runner_task
        .await
        .expect("run 任务应正常结束")
        .expect("取消面产 Ok 载荷（非 Err）");
    let outcome = outcome_of(output);
    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "取消 → 执行面归因 `Stopped`（节点 stopped、run stopped）"
    );
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Fail,
        "结论取 `Fail` 占位（MUST NOT 取 Error / Pass；fault 在位不参与门禁分派）"
    );
    assert_eq!(outcome.skip, None, "取消面非跳过");
    assert!(
        outcome.feedback_text.is_empty(),
        "stopped 面清空反馈文本（不误读为需修复）: {:?}",
        outcome.feedback_text
    );
    assert!(
        outcome.report_paths.is_empty(),
        "取消段零段块产出 → 无归并报告路径: {:?}",
        outcome.report_paths
    );
}

/// 边界（AC-7 入口短路）：已置位 cancel 再 `run` → 首段即 `Cancelled`、
/// `fault = Some(Stopped)` + `conclusion = Fail` 占位、假 spawn 零记录（入口
/// 短路不 spawn）、段串行 break（后续桶不执行）。
#[tokio::test]
async fn 已置位cancel_入口短路零spawn且收敛stopped() {
    let window = SpawnWindow::enter("cancel-pre");
    let ws = ChainWs::with_config("cancel-pre", TWO_RUST_CONFIG);
    window.install_fake_cargo(&ws.root.join("app"));
    window.install_fake_cargo(&ws.root.join("src-tauri"));

    let files = vec![
        "app/src/lib.rs".to_owned(),
        "src-tauri/src/main.rs".to_owned(),
    ];
    let cancel = foundation::cancel::CancelFlag::never();
    cancel.handle().set();
    let output = run_with_cancel(&ws, &files, cancel)
        .await
        .expect("取消面产 Ok 载荷");
    let outcome = outcome_of(output);

    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "首段即取消 → fault = Stopped"
    );
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Fail,
        "结论占位 Fail（不取 Error）"
    );
    assert!(
        outcome.feedback_text.is_empty(),
        "stopped 面反馈文本清空: {:?}",
        outcome.feedback_text
    );
    assert!(
        window.records().is_empty(),
        "入口短路零 spawn（取消不拉起进程）"
    );
}

/// 边界：跳过面顺序零改——`EmptyDiff` 先于取消判定返回 `skip`（取消已置位也
/// 不影响空 diff 的跳过语义）。
#[tokio::test]
async fn 已置位cancel但空diff仍先返回skip() {
    let _window = SpawnWindow::enter("cancel-skip");
    let ws = ChainWs::with_config("cancel-skip", RUST_APP_CONFIG);

    let cancel = foundation::cancel::CancelFlag::never();
    cancel.handle().set();
    let output = run_with_cancel(&ws, &[], cancel)
        .await
        .expect("空清单应 Ok");
    let outcome = outcome_of(output);
    assert_eq!(
        outcome.skip,
        Some(MutationSkipReason::EmptyDiff),
        "跳过面先于取消判定（顺序零改）"
    );
    assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);
}

/// 双桶 config（rust app + vitest web——多桶中途取消面：第 1 桶 rust 走段级
/// 复用门零 spawn，第 2 桶 vitest 走 Stryker 假 spawn 自旋）。
const RUST_AND_VITEST_CONFIG: &str = r#"{ "tests": [
  { "root": "app", "framework": "rust", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 80 } },
  { "root": "web", "framework": "vitest", "coverage": { "lines": 80, "branches": 70, "functions": 75 }, "mutation": { "score": 80 } }
] }"#;

/// 双桶中途取消装置：第 1 桶（rust）预置新鲜段报告走复用门（零 spawn），第 2
/// 桶（vitest）假 `npx` 自旋挂起并落 pid——返回（workspace、pid 落位路径）。
fn two_bucket_ws(tag: &str) -> (ChainWs, PathBuf) {
    let ws = ChainWs::with_config(tag, RUST_AND_VITEST_CONFIG).with_summary();
    let rust_seg_dir = foundation::layout::change_test_reports(&ws.root, ws.change)
        .join("app_rust")
        .join("mutation");
    fs::create_dir_all(&rust_seg_dir).expect("建 rust 突变报告目录");
    fs::write(rust_seg_dir.join("mutation-seg-0.json"), CARGO_SEG_PASS)
        .expect("写第 1 桶复用段报告");
    // 第 2 桶 suite 执行目录（spawn cwd 前置——`current_dir` 施加面）
    fs::create_dir_all(ws.root.join("web")).expect("建 web suite 执行目录");
    let pid_file = ws.root.join("npx-pid.txt");
    (ws, pid_file)
}

/// 正反向组合锚（AC-7 / D17）：**多桶中途取消**——第 1 桶（rust 复用门）完成
/// 后置位 → 第 2 桶（Stryker 假 spawn 自旋）观测取消 → 整体
/// `fault = Some(Stopped)` + `conclusion = Fail` 占位、`feedback_text` 空、
/// 第 1 桶报告路径仍在 `report_paths`、`summary.json` mutation 位不回填（取消面与
/// `Interrupted` 同型：不落部分结论）+ 第 2 桶进程被回收（pid 消失）。
#[tokio::test]
async fn 多桶中途取消_第二桶取消且首桶报告路径在案() {
    let window = SpawnWindow::enter("cancel-multi-bucket");
    window.install_npx();
    window.set_sleep(1); // 第 2 桶自旋挂起（第 1 桶零 spawn，不受此键影响）
    let (ws, pid_file) = two_bucket_ws("cancel-multi-bucket");
    window.set_pidfile(&pid_file);

    let files = vec!["app/src/lib.rs".to_owned(), "web/src/panel.ts".to_owned()];
    let cancel = foundation::cancel::CancelFlag::never();
    let handle = cancel.handle();
    let ws_root = ws.root_str();
    let ws_change = ws.change;
    let runner = tokio::spawn(async move {
        ProcessMutation::new()
            .run(
                &ws_root,
                ws_change,
                &files,
                "diff --git a/app/src/lib.rs b/app/src/lib.rs\n",
                cancel,
            )
            .await
    });

    // 第 2 桶进程建立（落 pid）后置位——此时第 1 桶（复用门，零 spawn）已完成
    let pid = wait_pid(&pid_file, std::time::Duration::from_secs(60)).await;
    handle.set();

    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("取消面产 Ok 载荷（非 Err）");
    let outcome = outcome_of(output);
    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "第 2 桶取消 → fault = Stopped"
    );
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Fail,
        "结论占位 Fail（不取 Error）"
    );
    assert!(
        outcome.feedback_text.is_empty(),
        "stopped 面反馈文本清空: {:?}",
        outcome.feedback_text
    );
    assert!(
        outcome
            .report_paths
            .iter()
            .any(|path| path.contains("app_rust")),
        "第 1 桶已完成段的报告路径保留在案（中断不回滚已产报告）: {:?}",
        outcome.report_paths
    );
    assert!(
        wait_pid_gone(pid, std::time::Duration::from_secs(15)),
        "第 2 桶进程被回收（pid {pid} 消失——kill_tree 生效）"
    );
    let summary = read_summary(&ws);
    assert!(
        summary["mutation"].is_null(),
        "中断面不回填 summary mutation 位（部分结论不落位）: {summary}"
    );
    assert!(
        !foundation::layout::change_test_reports(&ws.root, ws.change)
            .join("web_vitest")
            .join("mutation")
            .join("mutation-seg-0.json")
            .exists(),
        "取消段零段报告落位（未产出被误读为段完成）"
    );
}

/// 边界（AC-7 逐层下传锚）：**循环内 clone 共享同一原子**——`execute →
/// run_suite → execute_segment → stryker::run_segment` 四层形参皆经
/// `cancel.clone()` 下传；置位后链尾执行体（`exec::run_shell`）观测到同一原子并
/// 杀树。若任一层退化为重新铸造（`never()`），段进程将不受置位影响（pid 存活至
/// 自旋自然结束、段报告落位）——故 pid 消失 + 零段报告即「同一原子逐层可见」的
/// 行为面证据。
#[tokio::test]
async fn 循环内clone共享原子_置位在链尾执行体可见() {
    let window = SpawnWindow::enter("cancel-clone-identity");
    window.install_npx();
    window.set_sleep(1);
    let (ws, pid_file) = two_bucket_ws("cancel-clone-identity");
    window.set_pidfile(&pid_file);

    let files = vec!["app/src/lib.rs".to_owned(), "web/src/panel.ts".to_owned()];
    let cancel = foundation::cancel::CancelFlag::never();
    let probe = cancel.clone(); // 置位后仍与链内各层持有者同一原子（身份可观测面）
    let handle = cancel.handle();
    let started = std::time::Instant::now();
    let ws_root = ws.root_str();
    let ws_change = ws.change;
    let runner = tokio::spawn(async move {
        ProcessMutation::new()
            .run(
                &ws_root,
                ws_change,
                &files,
                "diff --git a/app/src/lib.rs b/app/src/lib.rs\n",
                cancel,
            )
            .await
    });

    let pid = wait_pid(&pid_file, std::time::Duration::from_secs(60)).await;
    handle.set();

    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("取消面产 Ok 载荷");
    let elapsed = started.elapsed();
    let outcome = outcome_of(output);

    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "链尾观测到置位 → fault = Stopped"
    );
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Fail,
        "结论占位 Fail"
    );
    assert!(probe.is_set(), "置位对外部 clone 可见（同一原子）");
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "秒级收敛（不等段自旋自然结束，实际 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, std::time::Duration::from_secs(15)),
        "链尾执行体观测到置位并杀树（pid {pid} 消失——任一层重新铸造则不会消失）"
    );
}

/// 正向（D17 保留纪律）：**取消后 Stryker 临时面保留**——Stryker 桶段取消 →
/// `fault = Some(Stopped)` + `conclusion = Fail` 占位 + `feedback_text` 空串（保留
/// 清单的呈现面由 `Interrupted` 分支承载，取消面不写反馈文本），但 `_stryker-tmp`
/// 与临时 config 仍留在盘上（与 `Interrupted` 保留纪律同型：取消不清理失败桶
/// 临时面，供单段复现）。
#[tokio::test]
async fn 取消后stryker临时面保留_保留段与路径在案() {
    let window = SpawnWindow::enter("cancel-retain");
    let ws = ChainWs::with_config("cancel-retain", VITEST_CONFIG).with_summary();
    window.install_npx();
    fs::create_dir_all(ws.root.join("app")).expect("建 suite 执行目录");
    window.set_sleep(1); // 记录后自旋挂起（段执行体恒活，运行中窗口可置位）
    let pid_file = ws.root.join("npx-cancel-retain-pid.txt");
    window.set_pidfile(&pid_file);
    // 预置 sandbox 目录（真 Stryker 建置面模拟——保留断言原料在盘）
    let mutation_dir = foundation::layout::change_test_reports(&ws.root, ws.change)
        .join("app_vitest")
        .join("mutation");
    fs::create_dir_all(mutation_dir.join("_stryker-tmp").join("sandbox-x")).expect("预置 sandbox");

    let files = vec!["app/src/a.ts".to_owned()];
    let cancel = foundation::cancel::CancelFlag::never();
    let handle = cancel.handle();
    let started = std::time::Instant::now();
    let ws_root = ws.root_str();
    let ws_change = ws.change;
    let runner = tokio::spawn(async move {
        ProcessMutation::new()
            .run(
                &ws_root,
                ws_change,
                &files,
                "diff --git a/app/src/a.ts b/app/src/a.ts\n",
                cancel,
            )
            .await
    });

    // 段进程建立（落 pid）后置位——取消发生在段执行体运行中
    let pid = wait_pid(&pid_file, std::time::Duration::from_secs(60)).await;
    handle.set();

    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("取消面产 Ok 载荷（非 Err）");
    let outcome = outcome_of(output);
    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "取消 → fault = Stopped（非 Fatal Err 面）"
    );
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Fail,
        "结论占位 Fail（不取 Error）"
    );
    assert!(
        outcome.feedback_text.is_empty(),
        "stopped 面反馈文本清空: {:?}",
        outcome.feedback_text
    );

    // 临时面在盘（保留纪律：取消不清理失败桶临时 config 与 sandbox）
    let config_path = mutation_dir.join("mutation-stryker-seg-0.conf.json");
    assert!(
        config_path.is_file(),
        "取消段临时 config 在盘: {}",
        to_posix(&config_path)
    );
    assert!(
        mutation_dir.join("_stryker-tmp").is_dir(),
        "取消桶 sandbox `_stryker-tmp` 在盘: {}",
        to_posix(&mutation_dir)
    );

    // 保留纪律与 `Interrupted` 同型：临时面在盘（供单段复现），stopped 面不写
    // 反馈文本（保留清单的呈现面由 `Interrupted` 分支承载）
    assert!(
        !outcome.feedback_text.contains("突变执行中断"),
        "取消不混用中断文案: {:?}",
        outcome.feedback_text
    );
    assert!(
        outcome.report_paths.is_empty(),
        "取消段零归并报告（已产路径仅为完成段）: {:?}",
        outcome.report_paths
    );
    assert!(
        wait_pid_gone(pid, std::time::Duration::from_secs(15)),
        "取消强杀后段进程 pid {pid} 应消失（kill_tree 生效）"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "秒级收敛（不等段自旋自然结束，实际 {:?}）",
        started.elapsed()
    );
}

// ---------------------------------------------------------------------------
// 零重试与 `fault: None` 字面量面（AC-2 / AC-8）
// ---------------------------------------------------------------------------

/// 边界（AC-2）：段零重试——段输出含瞬态签名（`os error 32`）+ 非零退出 →
/// **假 spawn 记录恰 1 次**（段层 `RetryPolicy::none()` 语义：签名命中不追加
/// 尝试）、分流仍为既有 `Interrupted`（`conclusion = Error` + 中断文案）、
/// `fault == None`（段层无 fault——`Interrupted` 是产品语义，不是环境坏）。
#[tokio::test]
async fn 段输出含签名不触发段重试() {
    let window = SpawnWindow::enter("segment-no-retry");
    let ws = ChainWs::with_config("segment-no-retry", RUST_APP_CONFIG).with_summary();
    window.install_fake_cargo(&ws.root.join("app"));
    window.set_out("error: failed to rename archive file: /t/liba.rlib: os error 32\n");
    window.set_exit(1);

    let files = vec!["app/src/lib.rs".to_owned()];
    let output = run(&ws, &files).await.expect("中断面产 Ok 载荷（非 Err）");
    let outcome = outcome_of(output);

    assert_eq!(
        window.records().len(),
        1,
        "段层零重试：签名命中也不追加尝试（spawn 恰 1 次）"
    );
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Error,
        "分流仍为既有 Interrupted（非 fault）"
    );
    assert_eq!(
        outcome.fault, None,
        "段层不产 fault（段超时 / 非零退出属 Interrupted 产品语义）"
    );
    assert!(
        outcome.feedback_text.contains("突变执行中断"),
        "中断文案在案: {}",
        outcome.feedback_text
    );
}

/// 边界（AC-8，表驱动 4 形态）：`skip_outcome` / pass / fail / `EmptyDiff` 四
/// 形态产出经穷尽解构断言 `fault == None`（字面量随动 + 运行期锚）。
#[tokio::test]
async fn 四形态产出fault均为none() {
    // 形态一：EmptyDiff 跳过面
    {
        let _window = SpawnWindow::enter("fault-none-empty-diff");
        let ws = ChainWs::with_config("fault-none-empty-diff", RUST_APP_CONFIG);
        let output = run(&ws, &[]).await.expect("空清单应 Ok");
        let outcome = outcome_of(output);
        assert_eq!(outcome.skip, Some(MutationSkipReason::EmptyDiff));
        assert_eq!(outcome.fault, None, "跳过面 fault = None");
        assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);
    }

    // 形态二：全过（pass）
    {
        let window = SpawnWindow::enter("fault-none-pass");
        let ws = ChainWs::with_config("fault-none-pass", RUST_APP_CONFIG).with_summary();
        window.install_fake_cargo(&ws.root.join("app"));
        window.set_cargo_outcomes(CARGO_SEG_PASS);
        let output = run(&ws, &["app/src/lib.rs".to_owned()])
            .await
            .expect("达标段应 Ok");
        let outcome = outcome_of(output);
        assert_eq!(outcome.conclusion, MutationTestConclusion::Pass);
        assert_eq!(outcome.fault, None, "pass 面 fault = None");
    }

    // 形态三：未达标（fail）
    {
        let window = SpawnWindow::enter("fault-none-fail");
        let ws = ChainWs::with_config("fault-none-fail", RUST_APP_CONFIG).with_summary();
        window.install_fake_cargo(&ws.root.join("app"));
        window.set_cargo_outcomes(CARGO_SEG_FAIL);
        let output = run(&ws, &["app/src/lib.rs".to_owned()])
            .await
            .expect("未达标段应 Ok");
        let outcome = outcome_of(output);
        assert_eq!(outcome.conclusion, MutationTestConclusion::Fail);
        assert_eq!(outcome.fault, None, "fail 面 fault = None");
    }

    // 形态四：无执行器跳过面（NoExecutor）——由既有
    // `跳过面_no_executor_无执行器框架记evidence不err` 用例的 `fault == None`
    // 断言承接（同一 `skip_outcome` 装配面，避免重复装置）
}
