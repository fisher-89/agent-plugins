use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use checks::model::{CoverageMeasured, SourceFileEntry, TestCaseResult};
use checks::parser::{
    parse_cargo_test, parse_coverage, parse_coverage_measured, parse_spec_report,
};
use foundation::cancel::CancelFlag;

use super::detect::{to_posix, TestPlan};

/// suite 命令超时：rust suite 实测 340–393 s 的约 3 倍余量（原 600 s 仅 1.7 倍
/// ——热态正常、冷态或多 change 并发即触线；「与 CLI `runCommand(cmd, cwd,
/// 600000)` 同值」的锚定口径已失效）。代码常量（不进 config schema）。
pub(crate) const SUITE_COMMAND_TIMEOUT: Duration = Duration::from_secs(1200);

/// 框架版本探测超时（CLI `timeout: 30_000` 同值）。
pub(crate) const VERSION_DETECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 退出码不可用时的哨兵值：进程无退出码（Unix 信号终止——Windows
/// `ExitStatus::code()` 恒 `Some`）与 suite 命令超时强杀同记 `-1`；非零 →
/// `error` 面显式失败，MUST NOT 回落 `0`（吞成假绿）。
const EXIT_CODE_NO_STATUS: i32 = -1;

/// 单 suite 执行结果：退出码 / 耗时 / 解析后的用例与覆盖原始计数 / 工件解析
/// 错误面（`error` 进子报告 findings → summary problems execution_error）/
/// 脚本真实输出合并流（进 record 单点，不进 error 尾部）。
#[derive(Debug)]
pub(crate) struct PlanExecution {
    /// 命令退出码（超时 / 收口缺码记 `EXIT_CODE_NO_STATUS` 哨兵）
    pub exit_code: i32,
    /// 执行耗时毫秒
    pub duration_ms: f64,
    /// 解析出的用例行
    pub cases: Vec<TestCaseResult>,
    /// 逐文件覆盖原始计数
    pub source_files: Vec<SourceFileEntry>,
    /// 全局覆盖实测块（采集失败 / 无数据为 `None`）
    pub measured: Option<CoverageMeasured>,
    /// 执行 / 解析错误面（非基础设施失败——进 findings 供诊断归因；非零退出
    /// 携合并输出有界尾部）
    pub error: Option<String>,
    /// 脚本真实输出合并流（stdout 前缀 + stderr 后缀，**不加 `2>&1`**——rust
    /// 模板 `cargo llvm-cov --workspace --json --output-path "{coverage_file}" >
    /// "{results_file}"` 已把 stdout 重定向进结果工件，`2>&1` 会把 stderr 混入
    /// 污染 `parse_cargo_test`）；`mod.rs` 按 plan 序聚合后落单一 `.log` 供人工
    /// 回查
    pub output: Vec<u8>,
}

/// 单 suite 执行全流程：清报告目录 → 模板展开 → 程序解析前置 → spawn
///（超时 + 捕获）→ 工件解析。
pub(crate) async fn execute_plan(
    plan: &TestPlan,
    report_dir: &Path,
    cancel: CancelFlag,
) -> Result<PlanExecution, String> {
    let started = Instant::now();

    // 报告目录清空重建（重跑幂等；清空失败 = 写盘面基础设施失败 → Err）
    if report_dir.exists() {
        std::fs::remove_dir_all(report_dir)
            .map_err(|error| format!("报告目录清理失败 {}: {error}", to_posix(report_dir)))?;
    }
    std::fs::create_dir_all(report_dir)
        .map_err(|error| format!("报告目录创建失败 {}: {error}", to_posix(report_dir)))?;

    // 占位符原料（绝对 POSIX 路径——对 config root / cwd 解析歧义的规避）
    let results_file = report_dir.join(plan.results_output);
    let coverage_file = report_dir.join(plan.coverage_output);
    let results_file_posix = to_posix(&results_file);
    let coverage_file_posix = to_posix(&coverage_file);
    let report_dir_posix = to_posix(report_dir);

    // 模板展开（平台选臂：Windows cmd 模板 + cmd /C，其余 shell 模板 +
    // sh -c——static_check shell spawn 同口径）；展开后零改写（结果重定向
    // 已进 rust 模板，不做任何后置拼装）
    let command = expand_template(
        plan.template(),
        &report_dir_posix,
        &results_file_posix,
        &coverage_file_posix,
        plan.config_args.as_deref(),
        &plan.files,
    );

    // 程序解析前置检查（spawn cwd = suite cwd 的可达性口径）
    ensure_program_resolvable(&plan.cwd, &command)?;

    // 执行体（超时 / 双管道并发排水 / 进程树击杀 / 取消单点驻 exec）；超时收敛
    // execution_error，不悬挂、不烧反馈边预算
    let run = exec::run_shell(
        exec::SystemCommand::script_raw(&command),
        exec::RunOptions {
            cwd: Some(plan.cwd.clone()),
            env: Vec::new(),
            timeout: SUITE_COMMAND_TIMEOUT,
            cancel,
        },
    )
    .await
    .map_err(|failure| format!("测试命令{failure}"))?;
    let (exit_code, exec_error, output) = match run.outcome {
        // 取消 → 基础设施失败面（停给用户 / 收敛 run）；MUST NOT 落 plan 级
        // `execution_error`（那会被当作被测代码问题而产生假失败归因）
        exec::RunOutcome::Cancelled => return Err("执行已取消".to_owned()),
        // 超时：无完整输出可记（`exit_code` 走哨兵，summary 记 execution_error）
        exec::RunOutcome::TimedOut => (
            EXIT_CODE_NO_STATUS,
            Some(format!(
                "测试命令超时（{} 秒）强制终止",
                SUITE_COMMAND_TIMEOUT.as_secs()
            )),
            Vec::new(),
        ),
        exec::RunOutcome::Completed => {
            let exit_code = run.exit_code;
            // 合并流（rust 模板 `cargo llvm-cov --workspace … > "{results_file}"`
            // 仅重定向 stdout——编译错误面留在 stderr）；不加 `2>&1`：手工拼接
            // 只服务 record 回查，结果工件解析仍只读 stdout 面
            let mut output = run.stdout;
            output.extend_from_slice(&run.stderr);
            let exec_error = if exit_code == 0 {
                None
            } else {
                Some(non_zero_exit_error(exit_code, &output))
            };
            (exit_code, exec_error, output)
        }
    };

    // 工件解析（结果 + 覆盖两个读取面；缺失 / 不可解析进错误面，不 Err）
    let parsed_cases = parse_results(plan, report_dir);
    let (source_files, measured) = parse_coverage_artifact(plan, report_dir);
    let parse_error = match parsed_cases.error {
        Some(message) => Some(message),
        None if exit_code != 0 && parsed_cases.cases.is_empty() => {
            Some("plan 目录结果文件缺失或不可解析".to_owned())
        }
        None => None,
    };

    Ok(PlanExecution {
        exit_code,
        duration_ms: started.elapsed().as_millis() as f64,
        cases: parsed_cases.cases,
        source_files,
        measured,
        error: exec_error.or(parse_error),
        output,
    })
}

/// 非零退出错误面：记因退出码并携错误信息 + 输出末尾双段（被测域自身报错
/// 文本——编译 / 断言失败归因原料、反馈边修复会话的唯一直接线索；exec
/// `ProcessOutput` 统一装配，不再自截尾部——旧字符截断口径会把 JS 错误首行
/// 截掉）。输出为空回落裸退出码措辞。
fn non_zero_exit_error(exit_code: i32, output: &[u8]) -> String {
    let head = format!("测试命令非零退出（{exit_code}）");
    exec::ProcessOutput::new(exit_code, output.to_vec()).failure_message(&head)
}

/// 占位符模板展开：`{report_dir}` / `{results_file}` / `{coverage_file}` /
/// `{config_args}`（未注入时连前导空白一并剥离，CLI 同语义）/ `{files}`。
fn expand_template(
    template: &str,
    report_dir: &str,
    results_file: &str,
    coverage_file: &str,
    config_args: Option<&str>,
    files: &[String],
) -> String {
    let mut result = template.replace("{report_dir}", report_dir);
    result = result.replace("{results_file}", results_file);
    result = result.replace("{coverage_file}", coverage_file);
    match config_args {
        Some(args) => result = result.replace("{config_args}", args),
        None => {
            while let Some(index) = result.find("{config_args}") {
                let before = result[..index].trim_end();
                let after = &result[index + "{config_args}".len()..];
                result = format!("{before}{after}");
            }
        }
    }
    if files.is_empty() {
        result.replace("{files}", "")
    } else {
        result.replace("{files}", &files.join(" "))
    }
}

/// 结果工件解析（框架路由：rust / node-test 文本引擎；istanbul 族结果为
/// JSON 报告器产物，V1 用例行面不解析、计数交退出码与覆盖面判定）。
struct ParsedResults {
    cases: Vec<TestCaseResult>,
    error: Option<String>,
}

fn parse_results(plan: &TestPlan, report_dir: &Path) -> ParsedResults {
    let path = report_dir.join(plan.results_output);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => {
            return ParsedResults {
                cases: Vec::new(),
                error: Some(format!("结果工件为空: {}", plan.results_output)),
            };
        }
        Err(_) => {
            return ParsedResults {
                cases: Vec::new(),
                error: Some(format!("结果工件缺失: {}", plan.results_output)),
            };
        }
    };
    let parse = match plan.framework.as_str() {
        "rust" => parse_cargo_test,
        "node-test" => parse_spec_report,
        // istanbul 族（jest / vitest / vite-plus）：JSON 结果报告形态，V1
        // 用例行面不解析（零用例行计数面交退出码与覆盖判定）
        _ => {
            return ParsedResults {
                cases: Vec::new(),
                error: None,
            }
        }
    };
    match parse(&text) {
        Ok(cases) => ParsedResults { cases, error: None },
        Err(error) => ParsedResults {
            cases: Vec::new(),
            error: Some(error),
        },
    }
}

/// 覆盖工件解析：全局实测块 + 逐文件原始计数（node-test 覆盖表与结果同
/// 文件；istanbul / llvm-cov 读 coverage_output）。
fn parse_coverage_artifact(
    plan: &TestPlan,
    report_dir: &Path,
) -> (Vec<SourceFileEntry>, Option<CoverageMeasured>) {
    let path = report_dir.join(plan.coverage_output);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (Vec::new(), None);
    };
    let measured = parse_coverage_measured(plan.coverage_format, &text);
    let entries = parse_coverage(plan.coverage_format, &text).unwrap_or_default();
    (entries, measured)
}

// ---------------------------------------------------------------------------
// 程序解析前置检查（static_check 经验复用：shell 吞缺失的前置防线）
// ---------------------------------------------------------------------------

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

/// 命令的程序解析检查：裸名查 PATH（Windows 额外查 cwd——`cmd /C` 语义），
/// 带路径分隔符 / 绝对路径查相对 spawn cwd 的显式路径。不可达 → `Err`
///（基础设施失败显式停给用户，不落报告级 error 空转烧反馈边预算）。
fn ensure_program_resolvable(cwd: &Path, command: &str) -> Result<(), String> {
    let Some(program) = command.split_whitespace().next() else {
        return Err("测试命令拉起失败: 展开后的命令为空白".to_owned());
    };
    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        if cwd.join(program_path).is_file() {
            return Ok(());
        }
        return Err(format!(
            "测试命令拉起失败: 程序 \"{program}\" 不可达（相对 suite cwd 不存在）"
        ));
    }
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    #[cfg(target_os = "windows")]
    let dirs: Vec<PathBuf> = {
        let mut dirs = dirs;
        dirs.insert(0, cwd.to_path_buf());
        dirs
    };
    if dirs.iter().any(|dir| reachable_in(dir, program)) {
        return Ok(());
    }
    Err(format!(
        "测试命令拉起失败: 程序 \"{program}\" 未找到（PATH 上无对应可执行）"
    ))
}

#[cfg(test)]
#[path = "runner_test.rs"]
mod runner_test;
