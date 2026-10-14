use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use exec::SystemCommand;

use super::{
    mutation_jobs, SegmentFailure, PER_MUTANT_TIMEOUT_SECS, SEGMENT_SPAWN_TIMEOUT_MS,
};

/// 段执行：`cargo mutants --in-diff <seg patch> --json --baseline skip --jobs
/// <半 CPU> --timeout <PER_MUTANT_TIMEOUT_SECS>` spawn（超时 + 输出捕获，段补丁
/// 经 argv 传参零 shell 包装）；`CARGO_TARGET_DIR` 锚 suite 执行目录 `target/`
/// ——各段共享同一 target dir（增量编译跨段复用，MUST NOT 每段重置）。段原始
/// 报告 = `--json` 载荷（stdout 提取），由调用方写盘。
pub(crate) async fn run_segment(
    patch_path: &Path,
    exec_dir: &Path,
) -> Result<String, SegmentFailure> {
    let command = SystemCommand::bare("cargo")
        .args(["mutants", "--in-diff"])
        .arg(patch_path)
        .arg("--json")
        .args(["--baseline", "skip", "--jobs"])
        .arg(mutation_jobs().to_string())
        .args(["--timeout"])
        .arg(PER_MUTANT_TIMEOUT_SECS.to_string());
    let mut shell = command.into_tokio();
    shell
        .current_dir(exec_dir)
        .env("CARGO_TARGET_DIR", exec_dir.join("target"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = shell.spawn().map_err(|error| {
        SegmentFailure::Fatal(format!(
            "cargo mutants 拉起失败（程序不可达？）: {error}；请确认 cargo-mutants 已安装（cargo install cargo-mutants）"
        ))
    })?;
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
                "cargo-mutants 段执行超时（{} 秒）强制终止",
                SEGMENT_SPAWN_TIMEOUT_MS / 1000
            )));
        }
        Ok((Err(error), _, _)) => {
            return Err(SegmentFailure::Fatal(format!(
                "cargo-mutants 段执行等待失败: {error}"
            )));
        }
        Ok((Ok(status), stdout_bytes, stderr_bytes)) => {
            let mut output = stdout_bytes;
            output.extend_from_slice(&stderr_bytes);
            (status.code().unwrap_or(-1), output)
        }
    };

    // 段报告为权威：`--json` 载荷可解析 → 段完成（cargo-mutants 以非零退出表
    // 达未达标等语义不中断）；否则（崩溃 / 未产出报告）→ 中断即时反馈
    let output_text = String::from_utf8_lossy(&output);
    match json_payload(&output_text) {
        Some(payload) => Ok(payload.to_owned()),
        None => Err(SegmentFailure::Interrupted(segment_failure_message(
            exit_code,
            &output,
        ))),
    }
}

/// 段中断错误面（非零退出 / stdout 无 JSON 载荷；错误信息 + 输出末尾双段
/// ——exec `ProcessOutput` 统一装配）。
fn segment_failure_message(exit_code: i32, output: &[u8]) -> String {
    let head = format!("cargo-mutants 段执行失败（退出码 {exit_code}，未产出可解析 JSON 段报告）");
    exec::ProcessOutput::new(exit_code, output.to_vec()).failure_message(&head)
}

/// JSON 载荷提取（stdout 可能混杂人类可读进度输出——首个 `{` / `[` 起、末个
/// `}` / `]` 止的最宽候选可解析即取；整文本可解析优先）。
fn json_payload(text: &str) -> Option<&str> {
    if serde_json::from_str::<serde_json::Value>(text).is_ok() {
        return Some(text);
    }
    let start = text.find(['{', '['])?;
    let end = text.rfind(['}', ']'])?;
    if end < start {
        return None;
    }
    let candidate = &text[start..=end];
    serde_json::from_str::<serde_json::Value>(candidate)
        .ok()
        .map(|_| candidate)
}

/// 管道读尽（`None` 管道 = 未捕获臂，静默跳过）。
async fn read_all<R: tokio::io::AsyncRead + Unpin>(pipe: &mut Option<R>, buffer: &mut Vec<u8>) {
    if let Some(pipe) = pipe.as_mut() {
        use tokio::io::AsyncReadExt as _;
        let _ = pipe.read_to_end(buffer).await;
    }
}

#[cfg(test)]
#[path = "cargo_mutants_test.rs"]
mod cargo_mutants_test;
