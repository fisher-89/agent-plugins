use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

/// 超时缺省与钳位边界（claude code 120s 缺省 / 600s 上限口径）：模型可申请
/// 长超时，活性护栏封顶。
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 600_000;

/// shell 底座探测产物（执行命令的组装形态）。
pub(crate) enum ShellBase {
    /// git-bash 可执行文件全路径（`bash -c`）
    GitBash(PathBuf),
    /// Windows 内置兜底（`cmd /C`）
    Cmd,
    /// unix 底座（`sh -c`）
    #[cfg(not(windows))]
    Sh,
}

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
    let shell = resolve_shell();
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
            kill_process_tree(pid);
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

/// shell 底座解析：Windows 探测 git-bash（unix 语法成功率最高），未命中退
/// `cmd /C` 兜底；unix `sh -c`。
fn resolve_shell() -> ShellBase {
    #[cfg(windows)]
    {
        match std::env::var_os("PATH") {
            Some(path_var) => probe_windows_shell(&path_var),
            None => ShellBase::Cmd,
        }
    }
    #[cfg(not(windows))]
    {
        ShellBase::Sh
    }
}

/// Windows shell 底座纯探测：扫 `path_var` 的 PATH 条目，命中
/// `Git\bin\bash.exe` / `Git\usr\bin\bash.exe` 形态即返 `GitBash`（System32
/// 的 WSL bash 同名不在 Git 形态段内，天然排除）；未命中退 `Cmd`。纯函数
/// （fabricated PATH 可测，无进程 spawn）。
#[cfg(windows)]
pub(crate) fn probe_windows_shell(path_var: &OsStr) -> ShellBase {
    for dir in std::env::split_paths(path_var) {
        for candidate in git_bash_candidates(&dir) {
            if candidate.is_file() {
                return ShellBase::GitBash(candidate);
            }
        }
    }
    ShellBase::Cmd
}

/// 单个 PATH 条目的 git-bash 候选
#[cfg(windows)]
fn git_bash_candidates(dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let segment = dir.to_string_lossy().replace('/', "\\").to_lowercase();
    if segment.ends_with("\\git\\bin") || segment.ends_with("\\git\\usr\\bin") {
        candidates.push(dir.join("bash.exe"));
    }
    // `Git\cmd` 条目 → bash 落在兄弟 `bin` / `usr\bin`；`Git` 根条目 → 落在
    // 子 `bin` / `usr\bin`。两者统一为「Git 根目录」再拼子段。
    let git_root = if segment.ends_with("\\git\\cmd") {
        dir.parent().map(Path::to_path_buf)
    } else if segment.ends_with("\\git") {
        Some(dir.to_path_buf())
    } else {
        None
    };
    if let Some(root) = git_root {
        candidates.push(root.join("bin\\bash.exe"));
        candidates.push(root.join("usr\\bin\\bash.exe"));
    }
    candidates.push(dir.join("Git\\bin\\bash.exe"));
    candidates.push(dir.join("Git\\usr\\bin\\bash.exe"));
    candidates
}

/// 按底座组装命令：GitBash → `bash -c`、Cmd → `cmd /C`、Sh → `sh -c`；
/// stdin 置空、stdout / stderr 按管道打开；Windows `CREATE_NO_WINDOW` 抑制
/// GUI 进程 spawn 控制台命令的窗口闪烁（env 继承缺省不動）。
fn build_command(shell: &ShellBase, command: &str) -> Command {
    let mut built = match shell {
        ShellBase::GitBash(bash) => {
            let mut built = Command::new(bash);
            built.arg("-c").arg(command);
            built
        }
        ShellBase::Cmd => {
            let mut built = Command::new("cmd");
            built.args(["/C", command]);
            built
        }
        #[cfg(not(windows))]
        ShellBase::Sh => {
            let mut built = Command::new("sh");
            built.arg("-c").arg(command);
            built
        }
    };
    built
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // tokio::process::Command 自带 creation_flags（cfg(windows) 固有方法）
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        built.creation_flags(CREATE_NO_WINDOW);
    }
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

/// 进程树击杀（形状复制自 `cli/runner.rs` 的 `kill_process_tree`，`cli/` 文件
/// 零 diff）：Windows `taskkill /PID <pid> /T /F` + `CREATE_NO_WINDOW`，unix
/// `kill -9`。尽力语义，静默不重试不阻塞收敛。
fn kill_process_tree(pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    #[cfg(windows)]
    {
        // GUI 进程 spawn 控制台命令须抑制窗口闪烁（CREATE_NO_WINDOW = 0x08000000）
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
}
