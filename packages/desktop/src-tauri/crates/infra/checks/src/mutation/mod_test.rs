//! `mutation/mod.rs` 的单元测试（test-design「mod.rs -> mod_test.rs」节）：
//! `ProcessMutation` 构造（new / Default 等价、无状态零副作用）+ execute 主链
//! 组合（suite 归桶 E10 → 跳过面 EmptyDiff / NoExecutor 记 evidence 不 Err →
//! 段级复用门 AC-8 零 spawn 全链 → js suite 全链（Stryker 面假 spawn 真执行）→
//! 段中断 Error 即时反馈 → 多 suite 多阈值（80 / 70）逐 suite 判定 → 计数归并 →
//! summary mutation 位回填 E11 → `MutationTestOutcome` 与 `feedback_text` 装配）
//! + 组合根链路（LocalToolSteps → vcs 假件 → ProcessMutation 假 spawn）+ E5
//! 显式 Err 面（写盘失败）+ 突变常量锚定（E9）。
//!
//! Mock策略（test-design 本节 Mock 表）：子进程 spawn（cargo-mutants / npx
//! stryker——进程边界）经**假 spawn 装置** [`SpawnWindow`] 承载——PATH 前置
//! shim + 记录 argv / env / cwd + 可编程输出、超时（自旋挂起 + 虚拟时钟推进）、
//! 非零退出、ENOENT（PATH 隔离使程序缺席）；装置驻本文件，`stryker_test` /
//! `cargo_mutants_test` 经 `super::super::mod_test` 共享引用。文件系统经真实
//! tempdir（`change_test_reports` 布局真实落盘——写盘 / 回填断言真实组合）；
//! 段级复用门用例零 spawn 复用（AC-8 复用语义自持，与假 spawn 执行链用例互补
//! ——非规避）；`available_parallelism` 不 mock（jobs / concurrency 期望按真实
//! CPU 重算关系式）。

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use checks::model::MutationTool;
use config::{MutationConfig, TestFramework};
use orchestration::port::{
    ArchiveVcsPort, BoxToolFuture, MutationRunner, MutationSkipReason, MutationTestConclusion,
    RebaseOutcome, StaticCheckRunner, TestExecutionRunner, ToolCommand, ToolStepOutput,
    ToolStepPort, ToolStepRequest, WorktreeSnapshot,
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
use super::{
    bucket_suites, framework_name, mutation_tool, normalize_root, output_tail, skip_outcome,
    ProcessMutation, EST_MUTANTS_PER_FILE, MUTATION_JOBS_DIVISOR, PER_MUTANT_TIMEOUT_SECS,
    SEGMENT_BUDGET_MS, SEGMENT_GUARD_MS,
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
/// 前置 shim 目录 + `FAKE_SPAWN_*` 可编程键收口（Drop 恢复 PATH、摘除全部键、
/// 释放锁——与 testexec `PathWindowGuard` 同式先例）。
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
        let lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let shim = tempfile::Builder::new()
            .prefix(&format!("checks-mutation-fake-spawn-{tag}-"))
            .tempdir()
            .expect("创建假 spawn shim 目录失败");
        let original_path = std::env::var_os("PATH");
        let mut entries: Vec<PathBuf> = vec![shim.path().to_path_buf()];
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
                fs::copy(&dll, self.shim.path().join("msys-2.0.dll"))
                    .expect("拷贝 msys-2.0.dll");
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
            let target = self.shim.path().join("npx.cmd");
            if !target.is_file() {
                fs::write(&target, NPX_CMD).expect("写假 npx.cmd");
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

    /// 可编程段报告写出路径（Stryker jsonReporter 直写段报告面模拟）。
    pub(super) fn set_report(&self, path: &Path) {
        std::env::set_var("FAKE_SPAWN_REPORT", path);
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
            "FAKE_SPAWN_OUT_ALT",
            "FAKE_SPAWN_ARG_MATCH",
            "FAKE_SPAWN_EXIT",
            "FAKE_SPAWN_EXIT_ALT",
            "FAKE_SPAWN_SLEEP",
            "FAKE_SPAWN_REPORT",
        ] {
            std::env::remove_var(key);
        }
    }
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
    let dir = bash
        .parent()
        .expect("bash 路径应有父目录")
        .to_path_buf();
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

/// 假 `mutants` 执行体（cargo mutants 面；`$0` = `mutants` 跨平台同构）：
/// 记录 argv / cwd / env → 可编程自旋挂起（超时面）→ 可编程输出（argv 子串命
/// 中换 ALT 源与退出码——多 suite 差异编程）→ canned 源逐行回写 stdout（段报
/// 告面在 `FAKE_SPAWN_REPORT` 在位时同源写出）。纯内建命令（零外部依赖——msys
/// bash 非交互 PATH 无 Git usr/bin）。
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
    "if defined FAKE_SPAWN_REPORT if defined FAKE_SPAWN_OUT_SRC copy /y \"!FAKE_SPAWN_OUT_SRC!\" \"!FAKE_SPAWN_REPORT!\" >nul\r\n",
    "if defined FAKE_SPAWN_EXIT exit /b !FAKE_SPAWN_EXIT!\r\n",
    "exit /b 0\r\n",
    ":spin\r\n",
    "goto spin\r\n",
);

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
const STRYKER_SEG_PASS: &str = r#"{ "metrics": { "killed": 9, "survived": 1, "totalMutants": 10 }, "durationMs": 3000.0 }"#;

/// 段中断 canned（非 JSON 损坏输出——harness 崩面）。
const SEG_INTERRUPT_GARBAGE: &str = "harness crashed: boom-marker";

/// cargo-mutants 段报告 fixture（9 killed + 1 survived / total 10——score 90）。
const CARGO_SEG_PASS: &str = r#"{
  "mutants": [
    { "outcome": "Caught" }, { "outcome": "Caught" }, { "outcome": "Caught" },
    { "outcome": "Caught" }, { "outcome": "Caught" }, { "outcome": "Caught" },
    { "outcome": "Caught" }, { "outcome": "Caught" }, { "outcome": "Caught" },
    { "outcome": "missed" }
  ],
  "durationMs": 1200.0
}"#;

/// cargo-mutants 段报告 fixture（5 killed + 5 survived / total 10——score 50
/// 未达 80 阈）。
const CARGO_SEG_FAIL: &str = r#"{
  "mutants": [
    { "outcome": "Caught" }, { "outcome": "Caught" }, { "outcome": "Caught" },
    { "outcome": "Caught" }, { "outcome": "Caught" },
    { "outcome": "missed" }, { "outcome": "missed" }, { "outcome": "missed" },
    { "outcome": "missed" }, { "outcome": "missed" }
  ],
  "durationMs": 2400.0
}"#;

/// cargo-mutants 段报告 canned（`killed` caught + `survived` missed；score =
/// killed / total 两位舍入）。
fn cargo_seg(killed: usize, survived: usize) -> String {
    let mut entries: Vec<&str> = Vec::with_capacity(killed + survived);
    entries.extend((0..killed).map(|_| r#"{ "outcome": "Caught" }"#));
    entries.extend((0..survived).map(|_| r#"{ "outcome": "missed" }"#));
    format!(
        r#"{{ "mutants": [{}], "durationMs": 1000.0 }}"#,
        entries.join(", ")
    )
}

/// 组合根链路 change **id**（≠ name——id → 记录 → name 解析面可辨）。
const COMBO_CHANGE_ID: &str = "combo-change-0001";

/// 组合根链路 fake vcs 补丁（单文件段——切片逐字等值注入 `--in-diff` 段补丁）。
const COMBO_PATCH: &str = "diff --git a/app/src/lib.rs b/app/src/lib.rs\n--- a/app/src/lib.rs\n+++ b/app/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";

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
    let b = ProcessMutation::default();
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
        .run(&root.to_string_lossy(), "demo-change", &files, "patch")
        .await
        .expect("NoExecutor 面恒 Ok");
    let outcome = outcome_of(output);
    assert_eq!(outcome.skip, Some(MutationSkipReason::NoExecutor));
    assert_eq!(
        outcome.conclusion,
        MutationTestConclusion::Pass,
        "跳过面不判 fail"
    );
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
        .run(&root.to_string_lossy(), "demo-change", &files, "patch")
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

    let files = vec!["app/src/a.test.ts".to_owned()];
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
    assert!(outcome.feedback_text.is_empty(), "Pass 恒空串（达标零反馈）");
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
    window.set_out(CARGO_SEG_PASS); // suite1 面：90 ≥ 80 达标完成
    window.set_alt("src-tauri_rust", SEG_INTERRUPT_GARBAGE, Some(101)); // suite2 面：损坏输出 + 非零退出

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
    assert!(feedback.contains("突变执行中断"), "错误摘要在案: {feedback}");
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

    // 假 spawn 执行链证据：两 suite 各一次、argv 携各自 plan 段补丁路径
    let records = window.records();
    assert_eq!(records.len(), 2, "两 suite 各恰一次假 spawn");
    assert_eq!(records[0].argv[0], "mutants");
    assert!(
        records[0].argv[2].contains("app_rust"),
        "suite1 段补丁路径: {:?}",
        records[0].argv
    );
    assert!(
        records[1].argv[2].contains("src-tauri_rust"),
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
    window.set_out(CARGO_SEG_PASS); // 90
    window.set_alt("src-tauri_rust", &cargo_seg(15, 5), None); // 75
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
    window.set_out(CARGO_SEG_FAIL); // 50
    window.set_alt("src-tauri_rust", &cargo_seg(19, 1), None); // 95
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
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push(("change_edit_set", to_posix(root), base.to_owned()));
        Ok(self.files.clone())
    }

    fn change_edit_patch(&self, root: &Path, base: &str) -> Result<String, String> {
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push(("change_edit_patch", to_posix(root), base.to_owned()));
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
    fn rebase_branch(
        &self,
        _worktree: &Path,
        _onto: &str,
    ) -> Result<RebaseOutcome, String> {
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

impl StaticCheckRunner for UnusedSeamRunner {
    fn run(&self, _root: &str, _name: &str) -> BoxToolFuture {
        let error = "组合根链路不触静态检查缝".to_owned();
        Box::pin(async move { Err::<ToolStepOutput, String>(error) })
    }
}

impl TestExecutionRunner for UnusedSeamRunner {
    fn run(&self, _root: &str, _name: &str) -> BoxToolFuture {
        let error = "组合根链路不触测试执行缝".to_owned();
        Box::pin(async move { Err::<ToolStepOutput, String>(error) })
    }
}

/// 组合：**组合根链路**——`LocalToolSteps` MutationTest 分支 → fake vcs（清单 /
/// 补丁唯一入口假件，`base` 恒 `HEAD`）→ 真实 `ProcessMutation::run` 四参（假
/// spawn 主链）→ outcome 载荷逐字段断言；id → 记录 → name 解析（报告目录携
/// `demo-change`）、vcs 补丁切片逐字注入 `--in-diff` 段补丁、审计行落账
/// （AC-3 / E3 / E11）。
#[tokio::test]
async fn 组合根链路_localtoolsteps_vcs假件_processmutation假spawn() {
    let window = SpawnWindow::enter("combo-root");
    let ws = ChainWs::with_config("combo-root", RUST_APP_CONFIG);
    window.install_fake_cargo(&ws.root.join("app"));
    window.set_out(CARGO_SEG_PASS);

    let vcs = Arc::new(FakeVcs::new(vec!["app/src/lib.rs".to_owned()], COMBO_PATCH));
    let store = Arc::new(FakeStore::new());
    store.seed_change(COMBO_CHANGE_ID, "demo-change");
    let steps = LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(UnusedSeamRunner),
        Arc::new(UnusedSeamRunner),
        Arc::new(ProcessMutation::new()),
        Arc::clone(&vcs) as Arc<dyn ArchiveVcsPort>,
        Arc::clone(&store) as Arc<dyn ChangeStateStore>,
        "run-combo-0001".to_owned(),
    );

    let output = steps
        .run(ToolStepRequest {
            root: ws.root_str(),
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

    // 段补丁 = vcs 补丁切片（清单 / 补丁注入 `--in-diff` 面逐字透传）
    let seg_patch = reports
        .join("app_rust")
        .join("mutation")
        .join("mutation-seg-0.patch");
    assert_eq!(
        fs::read_to_string(&seg_patch).expect("段补丁在盘"),
        COMBO_PATCH,
        "vcs 补丁经切片逐字注入 --in-diff 段补丁"
    );

    // 假 spawn 主链在位（执行链按 Mock 表走假 spawn）
    let records = window.records();
    assert_eq!(records.len(), 1, "真实 execute 主链恰一次假 spawn");
    assert_eq!(records[0].argv[0], "mutants", "argv 首 token");
    assert_eq!(records[0].argv[1], "--in-diff", "段补丁入参位");

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
        "app/nested/deep.rs".to_owned(),
        "outside/other.rs".to_owned(),
        // Windows 反斜杠入参归一 POSIX 后归桶
        "app\\src\\win.rs".to_owned(),
    ];
    let buckets = bucket_suites(root, &suites, &files);

    // 最长前缀匹配：app/nested/deep.rs 归 nested 桶（最具体）、app/src/* 归 app 桶
    assert_eq!(
        buckets[0].files,
        vec!["app/src/lib.rs", "app/src/win.rs"],
        "app 桶（POSIX 归一后排序去重）"
    );
    assert_eq!(buckets[1].files, vec!["app/nested/deep.rs"], "nested 桶（最长前缀）");
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
    for reason in [MutationSkipReason::EmptyDiff, MutationSkipReason::NoExecutor] {
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
    let cargo = super::parse_seg(MutationTool::CargoMutants, CARGO_SEG_PASS)
        .expect("cargo 段解析应成功");
    assert_eq!(cargo.tool, MutationTool::CargoMutants);
    assert_eq!(cargo.measured.killed, 9);
    assert_eq!(cargo.duration_ms, 1200.0);

    let stryker_raw = r#"{ "metrics": { "killed": 3, "survived": 1, "totalMutants": 4 } }"#;
    let stryker =
        super::parse_seg(MutationTool::Stryker, stryker_raw).expect("stryker 段解析应成功");
    assert_eq!(stryker.tool, MutationTool::Stryker);
    assert_eq!(stryker.measured.total, 4);

    // 解析失败显式 Err（结构面严格——路由不吞）
    assert!(super::parse_seg(MutationTool::CargoMutants, "broken").is_err());

    // 合并输出尾部单点（mod.rs 消费面）与 cargo_mutants_test 对拍
    assert_eq!(output_tail("ok".as_bytes()), "ok");
}
