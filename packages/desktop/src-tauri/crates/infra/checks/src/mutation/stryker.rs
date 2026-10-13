use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use exec::SystemCommand;

use super::report::{self, to_posix, STRYKER_TMP_DIR_NAME};
use super::{mutation_jobs, output_tail, SegmentFailure, SEGMENT_SPAWN_TIMEOUT_MS};

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
/// <config>` spawn（超时 + 输出捕获）→ 读回段原始报告（jsonReporter 直写段报告
/// 路径）。sandbox 清理归调用方（全段后一次——段间零清理）。
pub(crate) async fn run_segment(segment: &StrykerSegment<'_>) -> Result<String, SegmentFailure> {
    // 程序解析前置（E5：npx 不可达显式 Err——壳会吞缺失程序，先查 PATH）
    ensure_on_path("npx").map_err(SegmentFailure::Fatal)?;

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
    let mut shell = SystemCommand::script_raw(&command).into_tokio();
    shell
        .current_dir(segment.suite_cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = shell
        .spawn()
        .map_err(|error| SegmentFailure::Fatal(format!("Stryker 命令拉起失败: {error}")))?;
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let waited = tokio::time::timeout(Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS), async {
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
    let (exit_code, output) = match waited {
        Err(_) => {
            // 超时：终止并收尸（不悬挂执行链）——段中断即时反馈面（双时点②）
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(SegmentFailure::Interrupted(format!(
                "Stryker 段执行超时（{} 秒）强制终止",
                SEGMENT_SPAWN_TIMEOUT_MS / 1000
            )));
        }
        Ok((Err(error), _, _)) => {
            return Err(SegmentFailure::Fatal(format!(
                "Stryker 段执行等待失败: {error}"
            )));
        }
        Ok((Ok(status), stdout_bytes, stderr_bytes)) => {
            let mut output = stdout_bytes;
            output.extend_from_slice(&stderr_bytes);
            (status.code().unwrap_or(-1), output)
        }
    };

    // 段报告为权威：在位且可解析 → 段完成（工具以非零退出表达未达标等语义不
    // 中断）；缺失 / 损坏 → 中断（错误摘要 + 输出尾部，即时反馈面）
    match std::fs::read_to_string(&report_path) {
        Ok(raw) if serde_json::from_str::<serde_json::Value>(&raw).is_ok() => Ok(raw),
        _ => Err(SegmentFailure::Interrupted(segment_failure_message(
            exit_code,
            &output,
        ))),
    }
}

/// 段中断错误面（非零退出 / 无段报告；携合并输出有界尾部——报告文件面归因原
/// 料，反馈边只取摘要）。
fn segment_failure_message(exit_code: i32, output: &[u8]) -> String {
    let tail = output_tail(output);
    if tail.is_empty() {
        format!("Stryker 段执行失败（退出码 {exit_code}，未产出可解析段报告）")
    } else {
        format!(
            "Stryker 段执行失败（退出码 {exit_code}，未产出可解析段报告）：{tail}"
        )
    }
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
        "$schema": "node_modules/@stryker-mutator/core/schema/stryker-schema.json",
        "mutate": mutate,
        "testRunner": runner,
        "plugins": [plugin_package(runner)],
        "disableTypeChecks": true,
        "ignoreStatic": true,
        "reporters": ["json"],
        "jsonReporter": { "fileName": to_posix(report_path) },
        "tempDirName": to_posix(&temp_dir),
        "concurrency": mutation_jobs(),
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

/// 管道读尽（`None` 管道 = 未捕获臂，静默跳过）。
async fn read_all<R: tokio::io::AsyncRead + Unpin>(pipe: &mut Option<R>, buffer: &mut Vec<u8>) {
    if let Some(pipe) = pipe.as_mut() {
        use tokio::io::AsyncReadExt as _;
        let _ = pipe.read_to_end(buffer).await;
    }
}

#[cfg(test)]
#[path = "stryker_test.rs"]
mod stryker_test;
