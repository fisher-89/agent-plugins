use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use exec::ShellBase;
use serde_json::Value;
use tokio::process::Command;

/// 超时缺省与钳位边界（claude code 120s 缺省 / 600s 上限口径）：模型可申请
/// 长超时，活性护栏封顶。
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 600_000;

/// bash 工具执行体：`command` 必填、`timeout_ms` 可选（钳位缺省）。流程 =
/// shell 底座解析 → `tokio::process` 执行（cwd = root）→ 双管道并发排水 →
/// 超时收口（超时即杀进程树）→ 成功合并输出 / 非零退出码 `Err(退出码 + 输出)`。
pub(crate) async fn execute(root: &Path, input: &Value) -> Result<String, String> {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "入参缺失或非字符串字段: command".to_owned())?;
    if command.trim().is_empty() {
        return Err("command 不得为空".to_owned());
    }
    let timeout = Duration::from_millis(resolve_timeout_ms(input));
    let shell = exec::resolve_shell();
    let mut child = build_command(&shell, &command)
        .current_dir(root)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("进程启动失败: {e}"))?;
    let pid = child.id();
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| "stdout 未按管道打开".to_owned())?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| "stderr 未按管道打开".to_owned())?;
    // 双管道并发排水 + 退出码等待（超时罩全段：管道被孙进程继承不放时
    // read_to_end 不返回，超时即杀进程树解堵）
    let run = async {
        let (stdout, stderr) = tokio::join!(drain(&mut stdout_pipe), drain(&mut stderr_pipe));
        let status = child
            .wait()
            .await
            .map_err(|e| format!("等待进程退出失败: {e}"))?;
        Ok::<_, String>((stdout, stderr, status))
    };
    match tokio::time::timeout(timeout, run).await {
        Err(_) => {
            exec::kill_tree(pid);
            Err(format!(
                "命令超时（{} ms），进程树已清理: {command}",
                timeout.as_millis()
            ))
        }
        Ok(Err(message)) => Err(message),
        Ok(Ok((stdout, stderr, status))) => {
            let output = merge_output(stdout, stderr);
            if !status.success() {
                let code = status
                    .code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "signal".to_owned());
                return Err(format!("命令退出码 {code}:\n{output}"));
            }
            Ok(if output.is_empty() {
                "(无输出)".to_owned()
            } else {
                output
            })
        }
    }
}

/// `timeout_ms` 入参解析：缺省 120s，显式值钳位 `[1s, 600s]`（活性护栏封顶）。
fn resolve_timeout_ms(input: &Value) -> u64 {
    input
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .map(|n| n.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS))
        .unwrap_or(DEFAULT_TIMEOUT_MS)
}

/// 按底座组装命令：壳包装（GitBash → `bash -c`、Cmd → `cmd /C`、Sh →
/// `sh -c`）与平台标志（Windows `CREATE_NO_WINDOW` 抑制 GUI 进程 spawn
/// 控制台命令的窗口闪烁）由 exec 统一施加；此处仅补 stdin 置空、
/// stdout / stderr 按管道打开（env 继承缺省不動）。
fn build_command(shell: &ShellBase, command: &str) -> Command {
    let mut built = exec::SystemCommand::script_with(shell, command).into_tokio();
    built
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    built
}

/// 管道排水至 EOF（非 UTF-8 输出 lossy 保留，二进制不炸读取面）。
async fn drain<R: tokio::io::AsyncRead + Unpin>(pipe: &mut R) -> String {
    let mut bytes = Vec::new();
    let _ = tokio::io::AsyncReadExt::read_to_end(pipe, &mut bytes).await;
    String::from_utf8_lossy(&bytes).into_owned()
}

/// 双管道输出合并：stdout 段在前、stderr 段带 `[stderr]` 标记在后。
fn merge_output(stdout: String, stderr: String) -> String {
    if stderr.is_empty() {
        return stdout;
    }
    let mut merged = stdout;
    if !merged.is_empty() {
        merged.push('\n');
    }
    merged.push_str("[stderr]\n");
    merged.push_str(&stderr);
    merged
}
