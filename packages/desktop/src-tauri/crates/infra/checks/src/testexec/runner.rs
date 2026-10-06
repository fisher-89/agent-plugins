use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use checks::model::{CoverageMeasured, SourceFileEntry, TestCaseResult};
use checks::parser::{
    parse_cargo_test, parse_coverage, parse_coverage_measured, parse_spec_report,
};

use super::detect::{to_posix, TestPlan};

/// suite 命令超时（CLI `runCommand(cmd, cwd, 600000)` 同值）。
pub(crate) const SUITE_COMMAND_TIMEOUT: Duration = Duration::from_secs(600);

/// 框架版本探测超时（CLI `timeout: 30_000` 同值）。
pub(crate) const VERSION_DETECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 非零退出错误面携带的合并输出尾部字符上限（CLI `logCommandFailure` stderr
/// 500 字截断同量级——报告文件面归因原料；反馈边载荷再经 findings_brief
/// 200 字符截断收口）。
const OUTPUT_TAIL_LIMIT: usize = 500;

/// 单 suite 执行结果：退出码 / 耗时 / 解析后的用例与覆盖原始计数 / 工件解析
/// 错误面（`error` 进子报告 findings → summary problems execution_error）。
#[derive(Debug)]
pub(crate) struct PlanExecution {
    /// 命令退出码（超时 / 收口缺码记 -1）
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
}

/// 原生写结果文件的框架（经 CLI outputFile / reporter destination 落盘，
/// 无需 shell `>` 重定向）。
fn native_output_file(framework: &str) -> bool {
    matches!(framework, "jest" | "vitest" | "vite-plus" | "node-test")
}

/// 单 suite 执行全流程：清报告目录 → 模板展开 → 程序解析前置 → spawn
///（超时 + 捕获）→ 工件解析。
pub(crate) async fn execute_plan(
    plan: &TestPlan,
    report_dir: &Path,
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
    // sh -c——static_check shell spawn 同口径）
    let mut command = expand_template(
        plan.template(),
        &report_dir_posix,
        &results_file_posix,
        &coverage_file_posix,
        plan.config_args.as_deref(),
        &plan.files,
    );

    // 结果重定向：rust 段 `cargo test` stdout 重定向进结果工件（非原生
    // output file 框架的 CLI applyResultsRedirect 同语义——`cargo test` 后
    // 插入重定向，其余文本原样保留）
    if !native_output_file(&plan.framework) {
        command = command.replacen(
            "cargo test",
            &format!("cargo test > \"{results_file_posix}\""),
            1,
        );
    }

    // 程序解析前置检查（spawn cwd = suite cwd 的可达性口径）
    ensure_program_resolvable(&plan.cwd, &command)?;

    // spawn + 超时 + 输出捕获（stdout / stderr 并行读防管道填满互锁；
    // 超时杀子进程收敛 execution_error，不悬挂、不烧反馈边预算）
    let mut shell = shell_command(&command);
    shell
        .current_dir(&plan.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = shell
        .spawn()
        .map_err(|error| format!("测试命令拉起失败: {error}"))?;
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let waited = tokio::time::timeout(SUITE_COMMAND_TIMEOUT, async {
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let _ = tokio::join!(
            read_all(&mut stdout_pipe, &mut stdout_bytes),
            read_all(&mut stderr_pipe, &mut stderr_bytes),
        );
        let status = child.wait().await;
        (status, stdout_bytes, stderr_bytes)
    })
    .await;
    let (exit_code, exec_error) = match waited {
        Err(_) => {
            // 超时：终止并收尸（子进程残留不悬挂执行链）
            let _ = child.start_kill();
            let _ = child.wait().await;
            (
                -1,
                Some(format!(
                    "测试命令超时（{} 秒）强制终止",
                    SUITE_COMMAND_TIMEOUT.as_secs()
                )),
            )
        }
        Ok((Err(error), _, _)) => return Err(format!("测试命令等待失败: {error}")),
        Ok((Ok(status), stdout_bytes, stderr_bytes)) => {
            let exit_code = status.code().unwrap_or(-1);
            // 合并输出（rust 模板仅重定向 stdout——编译错误面留在 stderr）
            let mut output_text = String::from_utf8_lossy(&stdout_bytes).into_owned();
            output_text.push_str(&String::from_utf8_lossy(&stderr_bytes));
            let exec_error = if status.success() {
                None
            } else {
                Some(non_zero_exit_error(exit_code, &output_text))
            };
            (exit_code, exec_error)
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
    })
}

/// 管道读尽（`None` 管道 = 未捕获臂，静默跳过）。
async fn read_all<R: tokio::io::AsyncRead + Unpin>(pipe: &mut Option<R>, buffer: &mut Vec<u8>) {
    if let Some(pipe) = pipe.as_mut() {
        use tokio::io::AsyncReadExt as _;
        let _ = pipe.read_to_end(buffer).await;
    }
}

/// 非零退出错误面：记因退出码并携带合并输出有界尾部（被测域自身报错文本
/// ——编译 / 断言失败归因原料、反馈边修复会话的唯一直接线索；CLI
/// `formatMutationCommandError` 首条非空 stderr 归因同向）。输出为空回落裸
/// 退出码措辞。
fn non_zero_exit_error(exit_code: i32, output_text: &str) -> String {
    let tail = output_tail(output_text);
    if tail.is_empty() {
        format!("测试命令非零退出（{exit_code}）")
    } else {
        format!("测试命令非零退出（{exit_code}）：{tail}")
    }
}

/// 合并输出的有界尾部（按字符截取防多字节切半，末段优先——编译 / 断言错误
/// 通常在输出末段；超限以省略号前缀示意截断）。
fn output_tail(text: &str) -> String {
    let trimmed = text.trim_end();
    let total = trimmed.chars().count();
    if total <= OUTPUT_TAIL_LIMIT {
        return trimmed.to_owned();
    }
    let tail: String = trimmed.chars().skip(total - OUTPUT_TAIL_LIMIT).collect();
    format!("…{tail}")
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

/// shell 语义包装：Windows 经 cmd /C，其余经 sh -c（static_check spawn 同
/// 口径）。
fn shell_command(command: &str) -> tokio::process::Command {
    #[cfg(target_os = "windows")]
    {
        let mut shell = tokio::process::Command::new("cmd");
        shell.arg("/C").arg(command);
        shell
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut shell = tokio::process::Command::new("sh");
        shell.arg("-c").arg(command);
        shell
    }
}

#[cfg(test)]
#[path = "runner_test.rs"]
mod runner_test;
