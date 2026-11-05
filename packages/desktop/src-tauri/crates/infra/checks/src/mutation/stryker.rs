use std::path::{Path, PathBuf};
use std::time::Duration;

use exec::SystemCommand;
use foundation::cancel::CancelFlag;

use super::report::{self, to_posix, STRYKER_TMP_DIR_NAME};
use super::{stryker_concurrency, SegmentFailure, SEGMENT_SPAWN_TIMEOUT_MS};

/// Stryker 单次执行的 spawn 硬超时**下限**（零文件 / 取不到单轮耗时时退回本值）。
const SPAWN_TIMEOUT_MS: u64 = SEGMENT_SPAWN_TIMEOUT_MS;

/// 单突变体测试耗时占单轮测试的比例：`coverageAnalysis: perTest` 下每个突变体
/// 只跑覆盖它的测试子集，平均约一成。
///
/// **业界通用值，不按仓库 calibrate**：perTest 的「覆盖子集规模」由测试与源码的
/// 覆盖关系决定，与仓库大小无关；改动本值只会让护栏变宽或变窄，不影响判定。
const PER_MUTANT_TEST_RATIO: f64 = 0.1;

/// js 族单文件行数密度（突变体 / 源码行）。
///
/// **业界通用值，不按仓库 calibrate**：Stryker 的突变是表达式级（条件取反、算术
/// 替换、字面量替换），密度由语法形态决定而非项目规模。本仓历史实测 0.603
/// （`stryker-mutation-cwd-lca`：3 文件 1243 行 → 750 个），与通用值吻合。
///
/// Stryker 无 `--list` 等价物，本密度是突变数的唯一可用来源。
const MUTANTS_PER_LINE: f64 = 0.6;

/// Stryker 单次执行的 spawn 硬超时。
///
/// 与 cargo-mutants 不同，Stryker **没有编译阶段**，单突变体的成本只由「覆盖它的
/// 测试子集」决定（`coverageAnalysis: perTest` 默认开着），不存在「每突变体撞满
/// per-mutant 超时」那种可折算的最坏上界。故按通用比例估算：
///
/// `超时 = max(下限, (单轮测试 + 突变体数 × 单轮测试 × PER_MUTANT_TEST_RATIO) × 2 × 1000)`
///
/// 单轮测试耗时取同 plan 的 test-execution `report.json` 的 `duration_ms`（那是
/// 一次真实全量测试的墙钟）；取不到时退回固定下限。
fn estimate_timeout_ms(mutants: f64, round_duration_ms: Option<f64>) -> u64 {
    let Some(round_ms) = round_duration_ms.filter(|value| *value > 0.0) else {
        return SPAWN_TIMEOUT_MS;
    };
    let estimated = (round_ms + mutants * round_ms * PER_MUTANT_TEST_RATIO) * 2.0;
    if !estimated.is_finite() || estimated <= 0.0 {
        return SPAWN_TIMEOUT_MS;
    }
    (estimated as u64).max(SPAWN_TIMEOUT_MS)
}

/// Windows 上补探测的可执行后缀（PATHEXT 主干；与 cmd 搜索口径一致——镜像
/// `testexec::runner` 的私有实现，不改其可见性）。
#[cfg(target_os = "windows")]
const WINDOWS_EXE_EXTS: [&str; 4] = [".com", ".exe", ".bat", ".cmd"];

/// Stryker 段执行入参（sandbox 共享面与段标识全显式——各段仅 `index` / `files`
/// / 报告落位不同，其余恒同）。
pub(crate) struct StrykerSegment<'a> {
    /// spawn cwd（suite 执行目录；同时为 `--prefix` 与 config 相对基准）
    pub suite_cwd: &'a Path,
    /// 突变报告子树（临时 config / 共享 sandbox / 段报告落位）
    pub mutation_dir: &'a Path,
    /// 段序号
    pub index: usize,
    /// 段清单（项目根相对 POSIX）
    pub files: &'a [String],
    /// 清单绝对化基准（workspace 根）
    pub project_root: &'a Path,
    /// 测试框架（jest / vitest / vite-plus）
    pub framework: &'a str,
    /// 框架配置文件（suite `tests[].config` 绝对路径；未设为 `None`）
    pub suite_config: Option<&'a Path>,
}

/// 段执行：临时 config 写盘（`mutate` = 段清单、`tempDirName` = 共享 sandbox
/// 全段共用、`concurrency` = 半 CPU）→ `npx --prefix <suite cwd> stryker run
/// <config>` 经 `exec::run_shell` 执行（超时 / 双管道 / 进程树击杀 / 取消单点驻
/// exec）→ 读回段原始报告（jsonReporter 直写段报告路径）。sandbox 清理归调用方
///（全段后一次——段间零清理）。
pub(crate) async fn run_suite(
    segment: &StrykerSegment<'_>,
    round_duration_ms: Option<f64>,
    cancel: CancelFlag,
) -> Result<(String, bool), SegmentFailure> {
    // 程序解析前置（E5：npx 不可达显式 Err——壳会吞缺失程序，先查 PATH）
    ensure_on_path("npx").map_err(SegmentFailure::Fatal)?;

    // 突变数估算：文件总行数 × 密度（Stryker 无 `--list` 等价物，只能靠密度）。
    let total_lines: usize = segment
        .files
        .iter()
        .filter_map(|file| std::fs::read_to_string(segment.project_root.join(file)).ok())
        .map(|source| source.lines().count())
        .sum();
    let estimated_mutants = total_lines as f64 * MUTANTS_PER_LINE;
    let spawn_timeout_ms = estimate_timeout_ms(estimated_mutants, round_duration_ms);

    let report_path = report::segment_report_path(segment.mutation_dir, segment.index);
    let config_path = segment
        .mutation_dir
        .join(report::stryker_config_name(segment.index));
    let config = build_config(segment, &report_path);
    let config_json = serde_json::to_string_pretty(&config)
        .map_err(|error| SegmentFailure::Fatal(format!("Stryker config 序列化失败: {error}")))?;
    std::fs::write(&config_path, &config_json).map_err(|error| {
        SegmentFailure::Fatal(format!(
            "Stryker 临时 config 写盘失败 {}: {error}",
            to_posix(&config_path)
        ))
    })?;

    // 模板展开（CLI `npx --prefix "{prefix}" stryker run "{config}"` 同款口径）
    let command = format!(
        "npx --prefix \"{}\" stryker run \"{}\"",
        to_posix(segment.suite_cwd),
        to_posix(&config_path)
    );
    let run = exec::run_shell(
        SystemCommand::script_raw(&command),
        exec::RunOptions {
            cwd: Some(segment.suite_cwd.to_path_buf()),
            env: Vec::new(),
            timeout: Duration::from_millis(spawn_timeout_ms),
            cancel,
            // 突变面零重试（同 cargo-mutants 口径）
            retry: exec::RetryPolicy::none(),
            // 突变面零 live 接入（同 cargo-mutants 口径）
            live: None,
        },
    )
    .await;
    let (exit_code, output) = match run {
        Err(exec::RunError::Spawn(error)) => {
            return Err(SegmentFailure::Fatal(format!(
                "Stryker 命令拉起失败: {error}"
            )));
        }
        Err(exec::RunError::Wait(error)) => {
            return Err(SegmentFailure::Fatal(format!(
                "Stryker 段执行等待失败: {error}"
            )));
        }
        // 超时：整树已杀 + 收尸完成。Stryker 无增量成果文件（报告只在正常退出时
        // 由 jsonReporter 写），故无部分成果可回读——直接走中断面。
        Ok(report) if report.outcome == exec::RunOutcome::TimedOut => {
            return Err(SegmentFailure::Interrupted(format!(
                "Stryker 执行超时（{} 秒）强制终止",
                spawn_timeout_ms / 1000
            )));
        }
        // 取消：中断面（MUST NOT 走 Fatal——不把用户停止呈现为环境损坏）
        Ok(report) if report.outcome == exec::RunOutcome::Cancelled => {
            return Err(SegmentFailure::Cancelled("Stryker 执行已取消".to_owned()));
        }
        Ok(report) => {
            let mut output = report.stdout;
            output.extend_from_slice(&report.stderr);
            (report.exit_code, output)
        }
    };

    // 报告为权威：在位且可解析 → 完成（工具以非零退出表达未达标等语义不中断）；
    // 缺失 / 损坏 → 中断（错误摘要 + 输出尾部，即时反馈面）
    match std::fs::read_to_string(&report_path) {
        Ok(raw) if serde_json::from_str::<serde_json::Value>(&raw).is_ok() => Ok((raw, false)),
        _ => Err(SegmentFailure::Interrupted(segment_failure_message(
            exit_code, &output,
        ))),
    }
}

/// 执行失败面（非零退出 / 无报告；错误信息 + 输出末尾双段——exec
/// `ProcessOutput` 统一装配，报告文件面归因原料交反馈边取摘要）。
fn segment_failure_message(exit_code: i32, output: &[u8]) -> String {
    let head = format!("Stryker 执行失败（退出码 {exit_code}，未产出可解析报告）");
    exec::ProcessOutput::new(exit_code, output.to_vec()).failure_message(&head)
}

/// 临时 config 组装（CLI `generateTempConfig` 口径的桌面段化形态：`mutate` = 段
/// 清单、`tempDirName` = 共享 sandbox（全段共用同一路径——sandbox 建置只发生一
/// 次）、`jsonReporter` 直写段报告路径、`concurrency` = 半 CPU）。
fn build_config(segment: &StrykerSegment<'_>, report_path: &Path) -> serde_json::Value {
    let runner = test_runner(segment.framework);
    let mutate: Vec<String> = segment
        .files
        .iter()
        .map(|file| relative_to_cwd(segment.project_root, segment.suite_cwd, file))
        .collect();
    let temp_dir = segment.mutation_dir.join(STRYKER_TMP_DIR_NAME);
    let mut config = serde_json::json!({
        "mutate": mutate,
        "testRunner": runner,
        "plugins": [plugin_package(runner)],
        "disableTypeChecks": true,
        "ignoreStatic": true,
        "reporters": ["json"],
        "jsonReporter": { "fileName": to_posix(report_path) },
        "tempDirName": to_posix(&temp_dir),
        "concurrency": stryker_concurrency(),
        "timeoutFactor": 1.5,
        "timeoutMS": 10000,
    });
    if let Some(suite_config) = segment.suite_config {
        let config_file = suite_config
            .strip_prefix(segment.suite_cwd)
            .map(to_posix)
            .unwrap_or_else(|_| to_posix(suite_config));
        config[runner] = serde_json::json!({ "configFile": config_file });
    }
    config
}

/// 测试框架 → Stryker runner（CLI `resolveTestRunner` 同语义；js 族三值封闭）。
fn test_runner(framework: &str) -> &'static str {
    match framework {
        "jest" => "jest",
        // vitest / vite-plus 共用 vitest runner（CLI 现状口径）
        _ => "vitest",
    }
}

/// runner → npm 插件包名（Stryker v9 vitest runner 注册名与包名错位——CLI
/// `resolvePluginPackage` 同口径）。
fn plugin_package(runner: &str) -> String {
    match runner {
        "jest" => "@stryker-mutator/jest-runner".to_owned(),
        _ => "@stryker-mutator/vitest-runner".to_owned(),
    }
}

/// 清单项 → suite cwd 相对 POSIX（abs 归一后 strip cwd；cwd 外文件退绝对路径
/// POSIX 形态）。
fn relative_to_cwd(project_root: &Path, suite_cwd: &Path, file: &str) -> String {
    let absolute = project_root.join(file);
    absolute
        .strip_prefix(suite_cwd)
        .map(to_posix)
        .unwrap_or_else(|_| to_posix(&absolute))
}

/// 程序 PATH 可达前置检查（裸名查 PATH，Windows 补可执行后缀候选）：不可达 →
/// `Err`（E5——环境缺工具是显式可修问题，停给用户）。
fn ensure_on_path(program: &str) -> Result<(), String> {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    if dirs.iter().any(|dir| reachable_in(dir, program)) {
        return Ok(());
    }
    Err(format!(
        "突变执行器不可达：程序 \"{program}\" 未找到（PATH 上无对应可执行），请安装后重试"
    ))
}

/// 目录内程序可达性探测（裸名直接命中，Windows 补可执行后缀候选）。
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

#[cfg(test)]
#[path = "stryker_test.rs"]
mod stryker_test;
