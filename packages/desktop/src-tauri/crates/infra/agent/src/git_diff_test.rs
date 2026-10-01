//! `git_diff`（GitDiffSource spawn 缝）的单元测试（test-design「git_diff.rs ->
//! git_diff_test.rs」节）：porcelain 清单 + diff HEAD 补丁拼接（untracked 补齐
//! 盲区）、DIFF_CONTEXT_LIMIT 截断带省略标记、干净工作树空态、非 git 目录
//! Err、git 缺失 PATH Err（降级源头面）。
//!
//! Mock策略：无 mock（子进程边界真实组合）——真实 git 二进制 + tempdir 仓库
//! fixture（git init / add / commit 装置）；git 缺失用例以 PATH 目录替换 +
//! 共享互斥锁串行化、测毕恢复（沿既有 PATH 隔离装置先例）。

use std::fs;
use std::path::PathBuf;

use orchestration::port::DiffContextPort;

use crate::git_diff::{GitDiffSource, DIFF_CONTEXT_LIMIT};

/// PATH 环境变量修改串行化（进程全局变量边界；与 worker_test 的 CLI shim /
/// PATH 隔离窗口共用 crate 级锁——窗口期间系统 git 不可达）。
use crate::TEST_PATH_LOCK as PATH_LOCK;

/// 临时 git 仓库 RAII。
struct TempRepo(PathBuf);

impl TempRepo {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "agent-runtime-git-diff-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建仓库目录失败");
        let repo = Self(dir);
        repo.git(&["init"]);
        repo
    }

    fn root_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    fn write(&self, name: &str, content: &str) {
        fs::write(self.0.join(name), content).expect("写文件失败");
    }

    /// git 子命令执行（装置专用；带临时身份配置，不依赖全局 git 身份）。
    fn git(&self, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.com",
            ])
            .args(args)
            .current_dir(&self.0)
            .output()
            .expect("git 装置命令应可执行");
        assert!(
            output.status.success(),
            "git {:?} 装置失败: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// 装置：提交基线文件（index + commit）。
    fn commit_baseline(&self, name: &str, content: &str) {
        self.write(name, content);
        self.git(&["add", name]);
        self.git(&["commit", "-m", "baseline"]);
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 一次 diff 上下文组装。
async fn diff_context(root: &str) -> Result<String, String> {
    GitDiffSource::new().diff_context(root).await
}

#[tokio::test]
async fn 变更清单与补丁拼接_porcelain含untracked() {
    // PATH 隔离用例的进程全局窗口互斥（git 装置与断言均在窗口外执行）
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let repo = TempRepo::new("combined");
    repo.commit_baseline("tracked.rs", "fn main() {}\n");
    // 已跟踪修改（diff HEAD 盲区内的补丁体来源）
    repo.write("tracked.rs", "fn main() { /* 修改 */ }\n");
    // 未跟踪新文件（porcelain 补齐 `git diff HEAD` 盲区——W5 语义）
    repo.write("untracked_new.rs", "fn new_fn() {}\n");

    let context = diff_context(&repo.root_str())
        .await
        .expect("git 仓库上下文应成功");

    assert!(
        context.contains("### 未提交文件清单"),
        "porcelain 清单段在场: {context}"
    );
    assert!(
        context.contains("tracked.rs") && context.contains("untracked_new.rs"),
        "清单含已跟踪修改与未跟踪新文件（porcelain 盲区补齐）"
    );
    assert!(
        context.contains("??"),
        "untracked 以 porcelain `??` 形态在场"
    );
    assert!(context.contains("### 工作区补丁"), "diff HEAD 补丁段在场");
    assert!(
        context.contains("修改"),
        "补丁体携带已跟踪文件的工作区改动: {context}"
    );
}

#[tokio::test]
async fn 超长diff截断且带省略标记() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let repo = TempRepo::new("truncated");
    // 基线 10 行，随后追加大量行 → 补丁体远超 DIFF_CONTEXT_LIMIT
    let baseline: String = (0..10).map(|idx| format!("base line {idx}\n")).collect();
    repo.commit_baseline("big.txt", &baseline);

    let appended: String = (0..1600)
        .map(|idx| format!("appended padding line {idx:04} — 长行填充内容用于截断边界\n"))
        .collect();
    let mut grown = baseline.clone();
    grown.push_str(&appended);
    repo.write("big.txt", &grown);

    let context = diff_context(&repo.root_str()).await.expect("上下文应成功");

    assert!(context.contains("git diff 上下文超长截断"), "省略标记在场");
    assert!(
        context.chars().count() <= DIFF_CONTEXT_LIMIT + 200,
        "截断收口（上限 + 标记余量），实际 {}",
        context.chars().count()
    );
    assert!(
        context.chars().take(DIFF_CONTEXT_LIMIT).count() >= DIFF_CONTEXT_LIMIT - 1,
        "截断前保留 LIMIT 字符量级"
    );
}

#[tokio::test]
async fn 干净工作树空态为空串() {
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let repo = TempRepo::new("clean");
    repo.commit_baseline("only.rs", "fn main() {}\n");

    let context = diff_context(&repo.root_str()).await.expect("干净仓库应 Ok");

    assert_eq!(
        context, "",
        "双空（porcelain + patch）→ 空串（prompt 段缺席形态）"
    );
}

#[tokio::test]
async fn 非git目录err显式() {
    let dir = std::env::temp_dir().join(format!(
        "agent-runtime-git-diff-nogit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("创建普通目录失败");

    let err = diff_context(&dir.to_string_lossy())
        .await
        .expect_err("非 git 目录应 Err");
    assert!(
        err.contains("git"),
        "Err 显式（不 panic、不以空串冒充成功），实际: {err}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn git缺失path时err显式() {
    // PATH 隔离（共享互斥锁串行化、测毕恢复——进程全局变量边界；fixture 的
    // git 装置同在窗口外）
    let _guard = PATH_LOCK.lock().expect("PATH 锁不可中毒");
    let repo = TempRepo::new("path-isolated");
    repo.commit_baseline("only.rs", "fn main() {}\n");

    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let result = diff_context(&repo.root_str()).await;

    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(_guard);

    let err = result.expect_err("PATH 隔离无 git 应 Err");
    assert!(
        err.contains("拉起失败") || err.contains("git"),
        "降级源头面：Err 显式（walker 侧降级为错误提示），实际: {err}"
    );
}
