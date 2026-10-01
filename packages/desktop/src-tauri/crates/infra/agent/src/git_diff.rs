//! [`DiffContextPort`] 的进程实现（spawn 不进 core——W5 红线的 infra 落点）：
//! `git status --porcelain` 文件清单 + `git diff HEAD` 补丁体拼接，
//! [`DIFF_CONTEXT_LIMIT`] 字符截断带省略标记。porcelain 清单补齐未跟踪新文件
//!（`git diff HEAD` 不含 untracked）；git 缺失 / 失败以 `Err` 上抛，walker
//! 侧降级为错误提示不阻断 run。

use std::process::Stdio;

use orchestration::port::{BoxDiffFuture, DiffContextPort};

/// 变更文件上下文字符截断上限（prompt 有界面；超长带省略标记收口）。
pub const DIFF_CONTEXT_LIMIT: usize = 20_000;

/// git diff 上下文源（无状态，组合根按需构造）。
pub struct GitDiffSource;

impl GitDiffSource {
    /// 构造（组合根装配）。
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitDiffSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffContextPort for GitDiffSource {
    fn diff_context(&self, root: &str) -> BoxDiffFuture {
        let root = root.to_owned();
        Box::pin(async move { execute(&root).await })
    }
}

/// 一次上下文组装：porcelain 清单 + diff HEAD 补丁拼接 + 截断。工作区干净
/// （双空）→ 空串（prompt 段留「无未提交变更」声明）。
async fn execute(root: &str) -> Result<String, String> {
    let status = git(root, &["status", "--porcelain"]).await?;
    let patch = git(root, &["diff", "HEAD"]).await?;
    if status.trim().is_empty() && patch.trim().is_empty() {
        return Ok(String::new());
    }
    let mut context = format!(
        "### 未提交文件清单（git status --porcelain）\n\n{}\n\n### 工作区补丁（git diff HEAD）\n\n{}",
        status.trim_end(),
        patch.trim_end()
    );
    if context.chars().count() > DIFF_CONTEXT_LIMIT {
        let truncated: String = context.chars().take(DIFF_CONTEXT_LIMIT).collect();
        context = format!("{truncated}\n\n…（git diff 上下文超长截断）");
    }
    Ok(context)
}

/// 一次 git 子命令执行：stdout 捕获；非零退出以 stderr 上抛（ walker 降级面
/// 的错误载体）。
async fn git(root: &str, args: &[&str]) -> Result<String, String> {
    let mut command = tokio::process::Command::new("git");
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command
        .output()
        .await
        .map_err(|error| format!("git 拉起失败: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git {} 失败（退出码 {}）: {}",
            args.join(" "),
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
