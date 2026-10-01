//! [`StaticCheckRunner`] 的进程实现（spawn 不进 core——W4 红线的 infra 落点）：
//! `core::config` 读 `static_analysis` 命令 → shell 语义 spawn（cwd = workspace
//! root）→ 诊断捕获 + 退出码映射。无配置 / 空命令 = passed 直接过（与插件
//! `runStaticAnalysis` 同语义）；配置在位而命令程序不可达（PATH / root 上无
//! 对应可执行）= 显式 `Err`——shell 会把命令缺失吞成非零退出码
//!（passed=false 反馈边空转烧预算），spawn 前先解析程序，配置错误停给用户。

use std::path::{Path, PathBuf};
use std::process::Stdio;

use orchestration::port::{BoxToolFuture, StaticCheckOutcome, StaticCheckRunner, ToolStepOutput};

/// static-check 进程执行器（无状态，组合根按需构造）。
pub struct ProcessStaticCheck;

impl ProcessStaticCheck {
    /// 构造（组合根装配）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessStaticCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticCheckRunner for ProcessStaticCheck {
    fn run(&self, root: &str) -> BoxToolFuture {
        let root = root.to_owned();
        Box::pin(async move { execute(&root).await })
    }
}

/// 一次 static-check 执行全流程：配置读取 → spawn → 诊断拼接 → 退出码映射。
async fn execute(root: &str) -> Result<ToolStepOutput, String> {
    let report = config::load(Path::new(root));
    let Some(command) = report
        .config
        .static_analysis
        .map(|command| command.trim().to_owned())
        .filter(|command| !command.is_empty())
    else {
        // 无配置 / 空命令 = passed 直接过（不拉子进程）
        return Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
            passed: true,
            diagnostics: String::new(),
        }));
    };

    // 程序解析前置检查：配置在位而命令不可达 → 显式 `Err`（shell 会把命令
    // 缺失吞成非零退出码——passed=false 反馈边空转烧预算，spawn 前先解析）
    ensure_program_resolvable(root, &command)?;

    let mut command = shell_command(&command);
    command
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command
        .output()
        .await
        .map_err(|error| format!("static-check 命令拉起失败: {error}"))?;

    // 诊断捕获：stdout + stderr 原样拼接（插件把合并输出打 stderr、信封
    // 承载同文——此处诊断直接进产出， walker 反馈边定向注入）
    let mut diagnostics = String::from_utf8_lossy(&output.stdout).into_owned();
    diagnostics.push_str(&String::from_utf8_lossy(&output.stderr));
    let passed = output.status.success();
    Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
        passed,
        diagnostics: diagnostics.trim_end().to_owned(),
    }))
}

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

/// 配置命令的程序解析检查（shell 吞错误的前置防线）：裸名查 PATH
///（Windows 额外查 cwd——`cmd /C` 语义），带路径分隔符 / 绝对路径查相对
/// root 的显式路径（spawn cwd = root）。不可达 → `Err`（配置错误显式停给
/// 用户，不落 passed=false 反馈边）。
fn ensure_program_resolvable(root: &str, command: &str) -> Result<(), String> {
    let Some(program) = command.split_whitespace().next() else {
        return Err("static-check 命令拉起失败: static_analysis 命令为空白".to_owned());
    };
    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        if Path::new(root).join(program_path).is_file() {
            return Ok(());
        }
        return Err(format!(
            "static-check 命令拉起失败: 程序 \"{program}\" 不可达（相对 root 不存在）"
        ));
    }
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    // Windows 额外查 cwd（`cmd /C` 先搜当前目录）；其余平台仅 PATH
    #[cfg(target_os = "windows")]
    let dirs: Vec<PathBuf> = {
        let mut dirs = dirs;
        dirs.insert(0, PathBuf::from(root));
        dirs
    };
    if dirs.iter().any(|dir| reachable_in(dir, program)) {
        return Ok(());
    }
    Err(format!(
        "static-check 命令拉起失败: 程序 \"{program}\" 未找到（PATH 上无对应可执行）"
    ))
}

/// shell 语义包装（与插件 `execCommand` `shell: true` 同口径）：`static_analysis`
/// 命令串保留原样执行（参数 / 管道 / 重定向均可用）；Windows 经 cmd /C，其余
/// 经 sh -c。
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
