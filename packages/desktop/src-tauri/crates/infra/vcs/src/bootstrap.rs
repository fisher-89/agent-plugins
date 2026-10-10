//! 依赖引导安装执行体（design D4）：shell 包装（Windows `cmd /C`、其余
//! `sh -c`，static_check 先例）、cwd = worktree、环境全继承零注入（PATH 为
//! pnpm/npm/cargo 可达前提）、900 s 超时轮询 `try_wait` 到点 kill、输出摘要
//! = stdout + stderr 尾部 ≤300 字。

use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use workflow::write::InstallRun;

/// 安装命令超时上限（秒）：bootstrap 是分钟级 spawn（依赖全量重建可观），
/// 到点 kill 统一 `Err` 面（design D4）。
const INSTALL_TIMEOUT_SECS: u64 = 900;

/// 输出摘要尾部截断上限（字符数）。
const INSTALL_SUMMARY_MAX_CHARS: usize = 300;

/// 轮询间隔（毫秒）：try_wait 自旋预算与 CPU 占用的折中。
const POLL_INTERVAL_MS: u64 = 200;

/// 一次安装命令执行：shell 包装 spawn → 轮询 try_wait（到点 kill → `Err`）
/// → stdout + stderr 尾部 ≤300 字摘要 + 退出态。管道读段以独立线程排空
/// （防子进程输出撑满管道缓冲阻塞，try_wait 永不返回的假超时）。环境全
/// 继承零注入（默认继承，显式零改动）。
pub(crate) fn run_install(worktree: &Path, command: &str) -> Result<InstallRun, String> {
    let mut child = exec::SystemCommand::script(command)
        .into_std()
        .current_dir(worktree)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("安装命令拉起失败（{command}）: {error}"))?;
    // 管道排空线程（join 前子进程已收口 / 已 kill，读端收敛 EOF）
    let stdout_reader = Reader::spawn(child.stdout.take());
    let stderr_reader = Reader::spawn(child.stderr.take());

    let deadline = Instant::now() + Duration::from_secs(INSTALL_TIMEOUT_SECS);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait(); // 回收残尸（kill 后必收敛）
                    return Err(format!(
                        "安装命令超时（{command}，上限 {INSTALL_TIMEOUT_SECS} 秒）已终止"
                    ));
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(error) => return Err(format!("安装命令等待失败（{command}）: {error}")),
        }
    };
    let stdout = stdout_reader.join();
    let stderr = stderr_reader.join();
    let mut output = stdout;
    output.push_str(&stderr);
    Ok(InstallRun {
        success: status.success(),
        summary: tail_chars(&output, INSTALL_SUMMARY_MAX_CHARS),
    })
}

/// 管道读段线程柄：join 收敛全部字节（UTF-8 有损转换——摘要面不因非法字节
/// 失败；读失败收敛空串不炸执行体）。
struct Reader {
    handle: Option<std::thread::JoinHandle<String>>,
}

impl Reader {
    fn spawn(mut pipe: Option<impl Read + Send + 'static>) -> Self {
        Self {
            handle: pipe.take().map(|mut pipe| {
                std::thread::spawn(move || {
                    let mut bytes = Vec::new();
                    let _ = pipe.read_to_end(&mut bytes);
                    String::from_utf8_lossy(&bytes).into_owned()
                })
            }),
        }
    }

    fn join(self) -> String {
        self.handle
            .map(|handle| handle.join().unwrap_or_default())
            .unwrap_or_default()
    }
}

/// 尾部截断（按 char，不悬切多字节）：超限取尾部（安装输出末段是失败语境
/// 所在），短输出全量保留。
fn tail_chars(output: &str, max_chars: usize) -> String {
    let chars: Vec<char> = output.chars().collect();
    if chars.len() <= max_chars {
        return output.trim_end().to_owned();
    }
    let tail: String = chars[chars.len() - max_chars..].iter().collect();
    tail.trim_end().to_owned()
}
