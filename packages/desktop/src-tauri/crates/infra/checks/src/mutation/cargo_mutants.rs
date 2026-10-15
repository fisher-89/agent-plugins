use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use exec::SystemCommand;

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

/// 段执行：`cargo mutants --workspace --in-diff <seg patch> --baseline skip
/// --jobs <半 CPU> --timeout <PER_MUTANT_TIMEOUT_SECS>` spawn（超时 + 输出捕获，
/// 段补丁经 argv 传参零 shell 包装）。`--workspace` 必带：suite 根是混合
/// workspace（根包 + `crates/**` 成员），缺省只扫根包——`crates/**`（业务逻辑
/// 主场）将永不在突变面内；`--in-diff` 行级过滤把运行面收窄到 diff 交叠突变体。
/// `CARGO_TARGET_DIR` 锚 suite 执行目录 `target/`——各段共享同一 target dir（增
/// 量编译跨段复用，MUST NOT 每段重置）。
///
/// 段原始报告 = `mutants.out/outcomes.json`（spawn 前清场防陈旧误读；报告在位
/// 且可解析 → 段完成——工具以非零退出表达未达标等语义不中断，Stryker 报告文件
/// 面同约定），由调用方写盘。退出 0 且无报告 = diff 与可突变面零交叠
/// （cargo-mutants 对 EmptyDiff / NoSourceFiles / NoMutants 恒退出 0 且不产报告
/// ——「无突变体可测」非故障）→ 零突变体段（合成空计数面载荷）。
pub(crate) async fn run_segment(
    patch_path: &Path,
    exec_dir: &Path,
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
