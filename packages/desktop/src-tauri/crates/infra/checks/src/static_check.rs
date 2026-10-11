use std::path::{Path, PathBuf};
use std::process::Stdio;

use orchestration::port::{BoxToolFuture, StaticCheckOutcome, StaticCheckRunner, ToolStepOutput};

/// static-check 进程执行器（组合根按需构造）：输出记录模式默认
/// [`exec::ExecOutput::Transient`]（反馈边只消费诊断文本，全量即弃）；`recorded` 装配
/// 升 [`exec::ExecOutput::Recorded`]（全量落盘 change 报告目录树并经 `output_ref` 回填
/// 引用，审计 / 回查面）。**输出记录单点驻 infra/exec**——本层零私有落盘分支。
pub struct ProcessStaticCheck {
    /// 输出记录锚（change 名，报告目录树定位用）；`None` = Transient 默认
    record_anchor: Option<String>,
}

impl ProcessStaticCheck {
    /// 构造（组合根装配）：默认 `exec::ExecOutput::Transient`。
    pub fn new() -> Self {
        Self {
            record_anchor: None,
        }
    }

    /// `Recorded` 模式装配：stdout / stderr 全量落盘并回填 `output_ref`。
    pub fn recorded(change: impl Into<String>) -> Self {
        Self {
            record_anchor: Some(change.into()),
        }
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
        let record_anchor = self.record_anchor.clone();
        Box::pin(async move { execute(&root, record_anchor.as_deref()).await })
    }
}

/// 一次 static-check 执行全流程：配置读取 → spawn → 诊断拼接 → 退出码映射。
async fn execute(root: &str, record_anchor: Option<&str>) -> Result<ToolStepOutput, String> {
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
            output_ref: None,
        }));
    };

    // 程序解析前置检查：配置在位而命令不可达 → 显式 `Err`（shell 会把命令
    // 缺失吞成非零退出码——passed=false 反馈边空转烧预算，spawn 前先解析）
    ensure_program_resolvable(root, &command)?;

    let mut command = exec::SystemCommand::script(&command).into_tokio();
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
    // 输出记录：默认 Transient（无 id 即弃）；Recorded 模式经 infra/exec 单点
    // 落盘并回填引用（落盘失败降级 Transient——不因记录失败阻断检查主流程）
    let output_ref = record_anchor.and_then(|change| {
        exec::record_exec_output(Path::new(root), change, &output.stdout, &output.stderr).ok()
    });
    Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
        passed,
        diagnostics: diagnostics.trim_end().to_owned(),
        output_ref,
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
