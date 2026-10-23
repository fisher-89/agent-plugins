use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use exec::SystemCommand;
use foundation::cancel::CancelFlag;

use super::{mutation_jobs, SegmentFailure, PER_MUTANT_TIMEOUT_SECS, SEGMENT_SPAWN_TIMEOUT_MS};

/// cargo-mutants 段报告目录（spawn cwd 相对——默认写 `<cwd>/mutants.out`，
/// `--output` 未传）。
const CARGO_MUTANTS_OUT_DIR: &str = "mutants.out";

/// cargo-mutants 运行结果报告文件名（`mutants.out/outcomes.json`——逐突变体
/// outcome 与计数面的权威落位；运行面 stdout 只有人类可读进度行，`--json` 仅
/// 作用于 `--list`）。
const CARGO_OUTCOMES_FILE: &str = "outcomes.json";

/// 零突变体段合成载荷（计数面直读形态——`parse_cargo_mutants_seg` 计数面同式；
/// 全零计数 → 归并块 valid == 0 → score 100 恒过，空段不拖累 suite 判定）。
const ZERO_MUTANTS_PAYLOAD: &str = r#"{ "caught": 0, "missed": 0, "timeout": 0, "unviable": 0 }"#;

/// 段执行
pub(crate) async fn run_segment(
    patch_path: &Path,
    exec_dir: &Path,
    cancel: CancelFlag,
) -> Result<String, SegmentFailure> {
    // 段报告落位清场：上一段 / 上次运行的陈旧 outcomes.json MUST NOT 被本段误读
    // （best-effort——目录缺席等失败不阻断，spawn 后报告在位即权威）
    let _ = std::fs::remove_dir_all(exec_dir.join(CARGO_MUTANTS_OUT_DIR));
    let command = SystemCommand::bare("cargo")
        .args(["mutants", "--workspace", "--in-diff"])
        .arg(patch_path)
        .args(["--baseline", "skip", "--jobs"])
        .arg(mutation_jobs().to_string())
        .args(["--timeout"])
        .arg(PER_MUTANT_TIMEOUT_SECS.to_string());
    let run = exec::run_shell(
        command,
        exec::RunOptions {
            cwd: Some(exec_dir.to_path_buf()),
            // 段共享 target dir（增量编译跨段复用，MUST NOT 每段重置）
            env: vec![(
                OsString::from("CARGO_TARGET_DIR"),
                exec_dir.join("target").into_os_string(),
            )],
            timeout: Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS),
            cancel,
            // 突变面零重试（长跑是产品语义，段内重试会放大墙钟；断点续跑已由段级
            // 复用门承担）
            retry: exec::RetryPolicy::none(),
        },
    )
    .await;
    let (exit_code, output) = match run {
        Err(exec::RunError::Spawn(error)) => {
            return Err(SegmentFailure::Fatal(format!(
                "cargo mutants 拉起失败（程序不可达？）: {error}；请确认 cargo-mutants 已安装（cargo install cargo-mutants）"
            )));
        }
        Err(exec::RunError::Wait(error)) => {
            return Err(SegmentFailure::Fatal(format!(
                "cargo-mutants 段执行等待失败: {error}"
            )));
        }
        // 超时：整树已杀 + 收尸完成——段中断即时反馈面（双时点②）
        Ok(report) if report.outcome == exec::RunOutcome::TimedOut => {
            return Err(SegmentFailure::Interrupted(format!(
                "cargo-mutants 段执行超时（{} 秒）强制终止",
                SEGMENT_SPAWN_TIMEOUT_MS / 1000
            )));
        }
        // 取消：段中断面（MUST NOT 走 Fatal——不把用户停止呈现为环境损坏）
        Ok(report) if report.outcome == exec::RunOutcome::Cancelled => {
            return Err(SegmentFailure::Cancelled(
                "cargo-mutants 段执行已取消".to_owned(),
            ));
        }
        Ok(report) => {
            let mut output = report.stdout;
            output.extend_from_slice(&report.stderr);
            (report.exit_code, output)
        }
    };

    // 段报告为权威：`outcomes.json` 在位且可解析 → 段完成（cargo-mutants 以非零
    // 退出表达未达标等语义不中断）；退出 0 且无报告 → 零突变体段（合成空计数
    // 面）；其余（崩溃 / 无报告非零退出）→ 中断即时反馈
    let outcomes_path = exec_dir
        .join(CARGO_MUTANTS_OUT_DIR)
        .join(CARGO_OUTCOMES_FILE);
    match std::fs::read_to_string(&outcomes_path) {
        Ok(raw) if serde_json::from_str::<serde_json::Value>(&raw).is_ok() => Ok(raw),
        _ if exit_code == 0 => Ok(ZERO_MUTANTS_PAYLOAD.to_owned()),
        _ => Err(SegmentFailure::Interrupted(segment_failure_message(
            exit_code, &output,
        ))),
    }
}

/// 段中断错误面（非零退出 / stdout 无 JSON 载荷；错误信息 + 输出末尾双段
/// ——exec `ProcessOutput` 统一装配）。
fn segment_failure_message(exit_code: i32, output: &[u8]) -> String {
    let head = format!("cargo-mutants 段执行失败（退出码 {exit_code}，未产出可解析 JSON 段报告）");
    exec::ProcessOutput::new(exit_code, output.to_vec()).failure_message(&head)
}

#[cfg(test)]
#[path = "cargo_mutants_test.rs"]
mod cargo_mutants_test;
