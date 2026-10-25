use std::path::{Path, PathBuf};
use std::time::Duration;

use foundation::cancel::CancelFlag;
use foundation::layout::ReportSlot;
use orchestration::port::{
    BoxToolFuture, RunFault, StaticCheckOutcome, StaticCheckRunner, ToolStepOutput,
};

use crate::environment;

/// static-check 命令超时预算（此前**零超时**——cargo 卡在 `Blocking waiting for
/// file lock` 时 run 永久悬挂）：冷构建约 90 s + `vite-plus` 约 10 s 的约 3 倍余量。
/// 代码常量（不进 config schema、非用户可配项）。
pub(crate) const STATIC_CHECK_TIMEOUT: Duration = Duration::from_secs(1200);

/// static-check 进程执行器（组合根按需构造）：输出记录模式恒
/// [`exec::ExecOutput::Recorded`]（`run(root, name, slot, cancel)` 收 change name
/// 与报告槽位 + 取消信号，单一交错 `.log` 落盘并回填 `output_ref` 引用，审计 /
/// 回查面）。
/// **输出记录单点驻 infra/exec**——本层零私有落盘分支。
pub struct ProcessStaticCheck;

impl ProcessStaticCheck {
    /// 构造（组合根装配）：默认即 recorded（无装配分支）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessStaticCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticCheckRunner for ProcessStaticCheck {
    fn run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag) -> BoxToolFuture {
        let root = root.to_owned();
        let name = name.to_owned();
        Box::pin(async move { execute(&root, &name, slot, cancel).await })
    }
}

/// 一次 static-check 执行全流程：配置读取 → spawn（`2>&1` 交错、瞬态重试）→
/// 执行面归因（超时 / 取消 / 瞬态耗尽 → `fault`）→ 诊断拼接 → 退出码映射 →
/// 单一 `.log` 输出记录。报告目录 / 输出记录按槽位经 layout 单点派生
///（`report_dir` 三分支均回填——无配置分支语义为「本槽位报告树锚点」）；
/// 执行语义零分叉。
async fn execute(
    root: &str,
    name: &str,
    slot: ReportSlot,
    cancel: CancelFlag,
) -> Result<ToolStepOutput, String> {
    let report_dir = foundation::layout::change_reports(Path::new(root), name, slot)
        .to_string_lossy()
        .into_owned();
    let report = config::load(Path::new(root));
    let Some(command) = report
        .config
        .static_analysis
        .map(|command| command.trim().to_owned())
        .filter(|command| !command.is_empty())
    else {
        // 无配置 / 空命令 = passed 直接过（不拉子进程）；report_dir 回填本槽位
        // 报告树锚点
        return Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
            passed: true,
            diagnostics: String::new(),
            fault: None,
            output_ref: None,
            report_dir,
        }));
    };

    // 程序解析前置检查：配置在位而命令不可达 → 显式 `Err`（shell 会把命令
    // 缺失吞成非零退出码——passed=false 反馈边空转烧预算，spawn 前先解析）
    ensure_program_resolvable(root, &command)?;

    // 命令内 `2>&1` 保留：两 fd 指向同一写端，交错序在 OS 层合并、`stderr` 恒空
    //——`diagnostics` / record 单流逐字还原终端（执行体只给两路原始字节，不合流）
    let run = exec::run_shell(
        exec::SystemCommand::script(&format!("{command} 2>&1")),
        exec::RunOptions {
            cwd: Some(Path::new(root).to_path_buf()),
            env: Vec::new(),
            timeout: STATIC_CHECK_TIMEOUT,
            cancel,
            // 检查域开瞬态重试（构建产物被安全软件占用时自愈）
            retry: exec::RetryPolicy::transient(),
        },
    )
    .await
    .map_err(|failure| format!("static-check 命令{failure}"))?;

    // 执行面归因（fault）先于结论：`Err` 只留给「进程拉不起 / 收尸失败」等基础
    // 设施失败；超时 / 取消 / 瞬态耗尽走 `fault`，`passed` 口径仍为
    // `exit_code == 0`（MUST NOT 升三值）
    let fault = match run.outcome {
        exec::RunOutcome::Completed => exec::is_transient(&run).map(|_| {
            RunFault::Environment(environment::fault_text(
                run.attempts,
                run.exit_code,
                &run.stdout,
                &run.retried,
            ))
        }),
        exec::RunOutcome::TimedOut => Some(RunFault::Environment(format!(
            "static-check 命令超时（{} 秒）强制终止",
            STATIC_CHECK_TIMEOUT.as_secs()
        ))),
        exec::RunOutcome::Cancelled => Some(RunFault::Stopped),
    };

    // 诊断捕获：交错 stdout 原样（`run.stderr` 恒空且不读——2>&1 已在 OS 层
    // 合并）；插件把合并输出打 stderr、信封承载同文——此处诊断直接进产出，walker
    // 反馈边定向注入
    let diagnostics = String::from_utf8_lossy(&run.stdout).into_owned();
    let passed = run.exit_code == 0;
    // 输出记录：默认 recorded——经 infra/exec 单点落单一 `.log`（同锚本槽位报告
    // 树）并回填引用（落盘失败降级 None，不因记录失败阻断检查主流程）；重试场景
    // 下正文为全部尝试按序拼接（每段前加 attempt 分隔头，审计不丢上下文）
    let output_ref = exec::record_exec_output(
        Path::new(root),
        name,
        slot,
        &environment::attempts_log(&run),
    )
    .ok();
    Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
        passed,
        diagnostics: diagnostics.trim_end().to_owned(),
        fault,
        output_ref,
        report_dir,
    }))
}

/// Windows 上补探测的可执行后缀（PATHEXT 主干；与 cmd 搜索口径一致）。
#[cfg(target_os = "windows")]
const WINDOWS_EXE_EXTS: [&str; 4] = [".com", ".exe", ".bat", ".cmd"];

/// 目录内程序可达性探测：裸名直接命中，Windows 补可执行后缀候选。
fn reachable_in(dir: &Path, program: &str) -> bool {
    if dir.join(program).is_file() {
        return true;
    }
    #[cfg(target_os = "windows")]
    for ext in WINDOWS_EXE_EXTS {
        if dir.join(format!("{program}{ext}")).is_file() {
            return true;
        }
    }
    false
}

/// 配置命令的程序解析检查（shell 吞错误的前置防线）：裸名查 PATH
///（Windows 额外查 cwd——`cmd /C` 语义），带路径分隔符 / 绝对路径查相对
/// root 的显式路径（spawn cwd = root）。不可达 → `Err`（配置错误显式停给
/// 用户，不落 passed=false 反馈边）。
fn ensure_program_resolvable(root: &str, command: &str) -> Result<(), String> {
    let Some(program) = command.split_whitespace().next() else {
        return Err("static-check 命令拉起失败: static_analysis 命令为空白".to_owned());
    };
    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        if Path::new(root).join(program_path).is_file() {
            return Ok(());
        }
        return Err(format!(
            "static-check 命令拉起失败: 程序 \"{program}\" 不可达（相对 root 不存在）"
        ));
    }
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    // Windows 额外查 cwd（`cmd /C` 先搜当前目录）；其余平台仅 PATH
    #[cfg(target_os = "windows")]
    let dirs: Vec<PathBuf> = {
        let mut dirs = dirs;
        dirs.insert(0, PathBuf::from(root));
        dirs
    };
    if dirs.iter().any(|dir| reachable_in(dir, program)) {
        return Ok(());
    }
    Err(format!(
        "static-check 命令拉起失败: 程序 \"{program}\" 未找到（PATH 上无对应可执行）"
    ))
}
