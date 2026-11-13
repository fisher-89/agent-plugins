//! 模块 API 契约生成器的 CLI 入口：`generate` / `--check` / `bootstrap` 三子命令编排 +
//! 退出码。判定（契约解析 / `$defs` 规则 / 十条判据 / 渲染 / 比对）全在
//! `crates/core/archi`，生成物写盘单点在 `crates/infra/archi::api_artifacts`——本文件
//! 零判定、零文件系统写调用、零本地目录遍历实现。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

// 判定面一律经 crate 根出口（`lib.rs` 的逐名 `pub use` 是唯一出口面，D60）；
// `api::` 只是本文件对该出口面的本地别名。
use archi as api;
use archi::ArtifactDiff;

/// CLI 子命令（封闭四值；未知参数归 `Usage`）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    /// 写四类生成物（幂等）
    Generate,
    /// 生成到内存后与磁盘比对，零写入
    Check,
    /// 输出候选草稿到 stdout
    Bootstrap(String),
    /// 打印用法并退出 2
    Usage,
}

/// 进程退出码（`generate` / `--check` 无差异 → 0；有差异或失败 → 1；用法错 → 2）。
///
/// 零子进程：本文件只读 argv 并调库，MUST NOT 派生任何外部进程（无 `Command`、无 shell）。
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = repository_root();
    match run(parse_args(&args), &root) {
        Ok(code) => ExitCode::from(code as u8),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// argv → 子命令（`cargo run -p archi-api-gen -- --check` 的第一个 `--` 由 cargo 剥掉）。
fn parse_args(args: &[String]) -> Command {
    match args {
        [first] if first == "generate" => Command::Generate,
        [first] if first == "--check" || first == "check" => Command::Check,
        [first, element] if first == "bootstrap" => Command::Bootstrap(element.clone()),
        _ => Command::Usage,
    }
}

/// 子命令实现体（离 `main` 便于 tempdir 驱动；返回退出码）。
pub(crate) fn run(command: Command, root: &Path) -> Result<i32, String> {
    match command {
        Command::Generate => {
            let files = api::workspace_render(root).map_err(|error| error.message())?;
            let outcome = archi_runtime::api_artifacts::write_artifacts(root, &files)?;
            println!(
                "{} 份生成物已写入（{} 份未变）",
                outcome.written.len(),
                outcome.unchanged.len()
            );
            Ok(0)
        }
        Command::Check => {
            let diff = api::compare(root).map_err(|error| error.message())?;
            if diff.mismatched.is_empty() && diff.orphaned.is_empty() && diff.missing.is_empty() {
                return Ok(0);
            }
            println!("{}", diff_json(&diff));
            Ok(1)
        }
        Command::Bootstrap(element) => {
            let text = api::bootstrap(root, &element).map_err(|error| error.message())?;
            println!("{text}");
            Ok(0)
        }
        Command::Usage => {
            println!(
                "用法：archi-api-gen <子命令>\n\
                 \x20 generate            写四类生成物（schema.json / INDEX.toml / .i.rs / .i.ts）\n\
                 \x20 --check             读盘比对，零写入；有差异即非零退出并列清单\n\
                 \x20 bootstrap <元素>    输出契约草稿到 stdout（不写盘）"
            );
            Ok(2)
        }
    }
}

/// 观察根锚定：从当前工作目录向上取**最近的、含架构域的祖先**（契约与模型都住在
/// 架构域里，故这是唯一有意义的锚）；一个都不命中 → 退化为当前目录。
///
/// 不做行内路径字面量：判据本身经 [`foundation::layout`] 派生（脚本可从
/// `packages/desktop` 起手调用，无需在脚本里写 `cd ../..`）。
fn repository_root() -> PathBuf {
    let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut candidate = current.as_path();
    loop {
        if foundation::layout::archi_dir(candidate).is_dir() {
            return candidate.to_path_buf();
        }
        match candidate.parent() {
            Some(parent) => candidate = parent,
            None => return current,
        }
    }
}

/// `--check` 的 JSON 差异清单（stdout 单行，便于脚本解析）：三键名与序是脚本面契约。
fn diff_json(diff: &ArtifactDiff) -> String {
    serde_json::json!({
        "mismatched": diff.mismatched,
        "orphaned": diff.orphaned,
        "missing": diff.missing,
    })
    .to_string()
}

#[cfg(test)]
mod main_test;
