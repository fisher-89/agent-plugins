use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, MutexGuard};

use tempfile::TempDir;

use checks::model::{Conclusion, SummaryReport};
use foundation::layout::change_test_reports;
use orchestration::port::{TestExecutionConclusion, TestExecutionRunner, ToolStepOutput};

use super::detect::to_posix;
use super::{findings_brief, newest_input_mtime_ms, outcome, ProcessTestExecution};

/// 设置 shim 环境面（预置工件源 / 目的工件路径 / marker 路径 / 退出码；
/// None 侧摘除键）。dest 用 POSIX 形态（Copy-Item / cp 双侧可读）。
fn set_shim_env(results_src: &Path, dest: &Path, marker: Option<&Path>, exit: Option<i32>) {
    std::env::set_var("SHIM_RESULTS_SRC", results_src);
    std::env::set_var("SHIM_DEST", dest.to_string_lossy().replace('\\', "/"));
    match marker {
        Some(path) => std::env::set_var("SHIM_MARKER", path),
        None => std::env::remove_var("SHIM_MARKER"),
    }
    match exit {
        Some(code) => std::env::set_var("SHIM_EXIT", code.to_string()),
        None => std::env::remove_var("SHIM_EXIT"),
    }
}

/// 摘除全部 shim 环境键（窗口收口——不污染并行用例）。
fn clear_shim_env() {
    std::env::remove_var("SHIM_RESULTS_SRC");
    std::env::remove_var("SHIM_DEST");
    std::env::remove_var("SHIM_MARKER");
    std::env::remove_var("SHIM_EXIT");
}

/// node-test spec 绿跑样本（五用例 4 pass 1 skipped + "All files" 覆盖汇总行；
/// node-test 档结果与覆盖工件同文件。覆盖表格行以管道起手——实测块提取按
/// `|` 切列后取第 2 列为 lines）。
const SPEC_OUTPUT: &str = "▶ math\n\
✔ adds numbers (1.2ms)\n\
✔ handles unicode 名称 (0.5ms)\n\
﹣ skips todo case (0.1ms)\n\
▶ nested\n\
✔ works nested (0.3ms)\n\
✔ math (2ms)\n\
✔ nested (1ms)\n\
✔ top level case (0.2ms)\n\
ℹ tests 5\n\
ℹ pass 4\n\
ℹ fail 0\n\
ℹ skipped 1\n\
| file | line % | branch % | funcs % |\n\
| All files | 82.5 | 74.1 | 90.3 | 12 |\n";

const CHANGE: &str = "demo-change";

// ---------------------------------------------------------------------------
// 装置：全链 workspace fixture + 命令 shim
// ---------------------------------------------------------------------------

struct ChainWs {
    tmp: TempDir,
}

impl ChainWs {
    fn new(tag: &str) -> Self {
        let tmp = tempfile::Builder::new()
            .prefix(&format!("checks-runtime-chain-test-{tag}-"))
            .tempdir()
            .expect("创建临时 workspace 失败");
        let root = tmp.path();

        // config：node-test suite，root app，阈值 60/70/75（实测 82.5 达阈）；
        // includes 缺省吃注册表 default_glob 花括号形态
        // （`**/*.test.{mjs,js,cjs}`）——全链经 checks::globmatch 展开对齐
        // CLI picomatch 方言
        let config_dir = root.join("openspec");
        fs::create_dir_all(&config_dir).expect("创建 openspec 目录");
        fs::write(
            config_dir.join("config.json"),
            r#"{ "tests": [ { "root": "app", "framework": "node-test", "coverage": { "lines": 60, "branches": 70, "functions": 75 } } ] }"#,
        )
        .expect("写 config 失败");

        // 输入文件（复用门扫描面；{files} 占位符展开命中面）
        fs::create_dir_all(root.join("app").join("src")).expect("创建 suite src");
        fs::write(
            root.join("app").join("src").join("add.test.mjs"),
            "test body",
        )
        .expect("写输入文件");

        // 平台 shim：node.cmd（Windows）与 node 脚本（Unix）——参数无关设计：
        // 目的路径经 SHIM_DEST 环境变量注入（Rust 经 cmd /C 转引的 \" 序列会
        // 扰动带引号的命令行参数——env 面零转义风险）；首参 `--version`（
        // detect 版本探测臂）不拷贝不记 marker 恒退出 0；执行臂拷贝预置工件、
        // 追加执行 marker、按 SHIM_EXIT 退出
        #[cfg(target_os = "windows")]
        {
            fs::write(
                root.join("app").join("node.cmd"),
                "@echo off\r\npowershell -NoProfile -ExecutionPolicy Bypass -File \"%~dp0node-shim.ps1\" %*\r\nexit /b %errorlevel%\r\n",
            )
            .expect("写 node.cmd 失败");
            fs::write(
                root.join("app").join("node-shim.ps1"),
                "if ($args.Count -gt 0 -and $args[0] -eq '--version') { exit 0 }\n\
                 if ($env:SHIM_RESULTS_SRC -and $env:SHIM_DEST) { Copy-Item -Force $env:SHIM_RESULTS_SRC $env:SHIM_DEST }\n\
                 if ($env:SHIM_DEST -and $env:SHIM_MARKER) { Add-Content -Force -Path $env:SHIM_MARKER -Value 'ran' }\n\
                 if ($env:SHIM_EXIT) { exit ([int]$env:SHIM_EXIT) }\n\
                 exit 0\n",
            )
            .expect("写 node-shim.ps1 失败");
        }
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = root.join("app").join("node");
            fs::write(
                &script,
                "#!/bin/sh\n\
                 if [ \"$1\" = \"--version\" ]; then exit 0; fi\n\
                 if [ -n \"$SHIM_RESULTS_SRC\" ] && [ -n \"$SHIM_DEST\" ]; then cp \"$SHIM_RESULTS_SRC\" \"$SHIM_DEST\"; fi\n\
                 if [ -n \"$SHIM_DEST\" ] && [ -n \"$SHIM_MARKER\" ]; then echo ran >> \"$SHIM_MARKER\"; fi\n\
                 exit \"${SHIM_EXIT:-0}\"\n",
            )
            .expect("写 node shim 失败");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        }

        // change 目录（报告树推导锚）
        fs::create_dir_all(root.join("openspec").join("changes").join(CHANGE))
            .expect("创建 change 目录");

        Self { tmp }
    }

    fn root(&self) -> &Path {
        self.tmp.path()
    }

    fn root_str(&self) -> String {
        self.tmp.path().to_string_lossy().into_owned()
    }

    fn reports_dir(&self) -> PathBuf {
        change_test_reports(self.root(), CHANGE)
    }

    /// 执行臂的结果工件目的路径（planId 子目录 results.txt——SHIM_DEST 注入面）。
    fn results_dest(&self) -> PathBuf {
        self.reports_dir().join("app_node-test").join("results.txt")
    }

    fn marker_count(&self) -> usize {
        let marker = self.root().join("shim-marker.log");
        fs::read_to_string(marker)
            .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count())
            .unwrap_or(0)
    }

    fn summary_timestamp(&self) -> Option<String> {
        let text = fs::read_to_string(self.reports_dir().join("summary.json")).ok()?;
        checks::parse_summary_report(&text)
            .ok()
            .map(|summary| summary.timestamp)
    }

    /// PATH 窗口前置 suite cwd（裸名 `node` 解析到 shim）。全平台经 PATH 解析
    /// ——不依赖 cmd 的 cwd 搜索（NoDefaultCurrentDirectoryInExePath 环境下
    /// cmd 不搜 cwd）。须在 path_window 窗口内调用（窗口收口恢复原 PATH）。
    async fn run(&self) -> Result<ToolStepOutput, String> {
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(self.root().join("app")).chain(
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
            ))
            .expect("join PATH"),
        );
        ProcessTestExecution::new()
            .run(&self.root_str(), CHANGE)
            .await
    }
}

/// PATH / SHIM_* 环境窗口（进程全局变量边界串行化——持 crate 级
/// TEST_ENV_LOCK，与一切触 spawn / PATH 的用例互斥）。
fn path_window() -> PathWindowGuard {
    PathWindowGuard::enter()
}

struct PathWindowGuard {
    original_path: Option<std::ffi::OsString>,
    _shim_env: MutexGuard<'static, ()>,
}

impl PathWindowGuard {
    fn enter() -> Self {
        let shim_env = crate::TEST_ENV_LOCK.lock().expect("crate 环境锁不可中毒");
        Self {
            original_path: std::env::var_os("PATH"),
            _shim_env: shim_env,
        }
    }
}

impl Drop for PathWindowGuard {
    fn drop(&mut self) {
        match &self.original_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
        clear_shim_env();
    }
}

// ---------------------------------------------------------------------------
// 绿跑全链（AC-5）
// ---------------------------------------------------------------------------

/// 正向：绿跑全链组合——tempdir workspace + 真实 config tests[]（node-test
/// suite）+ 真实命令 shim（拷贝预置 spec 工件到占位路径、退出 0）→
/// ToolStepOutput::TestExecution conclusion=pass、四计数正确、summary.json 与
/// planId/report.json 落 change_test_reports 推导目录（AC-5）。
#[tokio::test]
async fn 绿跑全链_pass结论_四计数与报告落盘() {
    let ws = ChainWs::new("green-chain");
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");

    let output = {
        let _window = path_window();
        set_shim_env(
            &spec_src,
            &ws.results_dest(),
            Some(&ws.root().join("shim-marker.log")),
            None,
        );
        ws.run().await
    }
    .expect("绿跑全链应 Ok");

    let outcome = match output {
        ToolStepOutput::TestExecution(outcome) => outcome,
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    };
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Pass,
        "绿跑 → pass"
    );
    assert_eq!(
        (
            outcome.total,
            outcome.passed,
            outcome.failed,
            outcome.skipped
        ),
        (5, 4, 0, 1),
        "四计数自 spec 文本解析（node-test 档计数面）"
    );
    assert_eq!(
        outcome.findings_brief, "全部测试通过且覆盖率达阈值",
        "全绿诊断单条正向"
    );
    assert_eq!(
        outcome.report_dir,
        to_posix(&ws.reports_dir()),
        "载荷 report_dir = layout 推导目录 POSIX 形态"
    );

    // 报告落盘：summary.json + planId/report.json（plan id = derive_plan_id）
    let reports_dir = ws.reports_dir();
    assert!(
        reports_dir.join("summary.json").is_file(),
        "summary.json 落 change 报告目录"
    );
    let report_path = reports_dir.join("app_node-test").join("report.json");
    assert!(report_path.is_file(), "planId 子目录 report.json 落盘");

    let summary: SummaryReport = checks::parse_summary_report(
        &fs::read_to_string(reports_dir.join("summary.json")).expect("读回"),
    )
    .expect("summary.json 应可解析");
    assert_eq!(summary.conclusion, Conclusion::Pass);
    assert_eq!(
        (
            summary.total,
            summary.passed,
            summary.failed,
            summary.skipped
        ),
        (5, 4, 0, 1)
    );
    let coverage = summary.coverage.as_ref().expect("覆盖块在场");
    assert!(coverage.pass, "82.5 ≥ 60 / 74.1 ≥ 70 / 90.3 ≥ 75 达阈");
    assert_eq!(coverage.measured.lines, Some(82.5));
    assert_eq!(summary.plans.len(), 1);
    assert_eq!(
        summary.plans[0].id, "app_node-test",
        "plan id 沿 derive_plan_id 命名映射"
    );

    let sub = checks::parse_sub_report(&fs::read_to_string(&report_path).expect("读回"))
        .expect("report.json 应可解析");
    assert_eq!(sub.framework, "node-test");
    assert_eq!(sub.root, "app");
    assert_eq!(
        (
            sub.summary.total,
            sub.summary.passed,
            sub.summary.failed,
            sub.summary.skipped
        ),
        (5, 4, 0, 1)
    );
    assert_eq!(sub.exit_code, 0);
}

/// 正向：复用门装配（复用分支）——首跑落 summary 后输入未变二次 run 零命令
/// 执行（marker 计数不增长）→ 复用上次 summary 结论（AC-6 复用半边，与
/// reuse_test 纯判定半边互为表里）。
#[tokio::test]
async fn 复用门装配_输入未变零spawn复用() {
    let ws = ChainWs::new("reuse-hit");
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");

    let first = {
        let _window = path_window();
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        ws.run().await
    }
    .expect("首跑应 Ok");
    let first_ts = ws.summary_timestamp().expect("首跑落 summary");
    assert_eq!(ws.marker_count(), 1, "首跑恰一次命令执行（marker 计 1）");

    // 输入未变二次 run：detect 探测臂零 marker（无 destination 参数不记账），
    // 执行臂跳过 → marker 计数不增长、summary 未重写
    let second = {
        let _window = path_window();
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        ws.run().await
    }
    .expect("复用跑应 Ok");
    assert_eq!(ws.marker_count(), 1, "复用零命令执行（marker 计数不增长）");
    assert_eq!(
        ws.summary_timestamp(),
        Some(first_ts),
        "复用上次 summary（未重写）"
    );
    assert_eq!(first, second, "复用产出与首跑逐字段一致");
}

/// 正向：复用门装配（失效分支）——touch 输入文件推进 mtime 后 run → 失效
/// 重跑（marker 计数增长、summary 刷新，AC-6 失效半边）。
#[tokio::test]
async fn 复用门装配_输入失效重跑() {
    let ws = ChainWs::new("reuse-miss");
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");

    {
        let _window = path_window();
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        ws.run().await.expect("首跑应 Ok");
    }
    let first_ts = ws.summary_timestamp().expect("首跑落 summary");
    assert_eq!(ws.marker_count(), 1);

    // touch 输入文件：mtime 推进到未来（确定性越过首跑 summary 时间戳）
    let input = ws.root().join("app").join("src").join("add.test.mjs");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&input)
        .expect("打开输入文件")
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(30))
        .expect("推进 mtime");

    let _second = {
        let _window = path_window();
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        ws.run().await
    }
    .expect("失效重跑应 Ok");
    assert_eq!(ws.marker_count(), 2, "输入已变失效重跑（marker 计数增长）");
    assert_ne!(
        ws.summary_timestamp(),
        Some(first_ts),
        "summary 刷新（非复用）"
    );
}

// ---------------------------------------------------------------------------
// 异常：基础设施 Err 与报告级 error 两态分立
// ---------------------------------------------------------------------------

/// 异常：测试程序不可达 → run 返回 Err（基础设施失败经 run_tool 映射 run
/// 显式失败终态，不产部分结论——Err 与 error 两态分立）。suite cwd 指向空
/// 目录且 PATH 隔离：裸名 `node` cwd / PATH 双不可达 → 程序解析前置 Err。
#[tokio::test]
async fn 测试程序不可达_run_err() {
    let tmp = tempfile::Builder::new()
        .prefix("checks-runtime-unreachable-test-")
        .tempdir()
        .expect("创建临时 workspace 失败");
    let root = tmp.path();
    // suite cwd = app/empty（无 shim）；文件清单命中 app/empty 内样本
    fs::create_dir_all(root.join("openspec")).expect("创建 openspec");
    fs::write(
        root.join("openspec").join("config.json"),
        r#"{ "tests": [ { "root": "app", "framework": "node-test", "cwd": "empty", "includes": ["**/*.mjs"], "coverage": { "lines": 60, "branches": 70, "functions": 75 } } ] }"#,
    )
    .expect("写 config");
    fs::create_dir_all(root.join("app").join("empty")).expect("创建空 suite cwd");
    fs::write(
        root.join("app").join("empty").join("x.test.mjs"),
        "test body",
    )
    .expect("写样本");

    let root_str = root.to_string_lossy().into_owned();
    let result = {
        let _window = path_window();
        std::env::set_var("PATH", "");
        // await 须在窗口块内（窗口 guard 持有至执行收口——块尾即恢复原 PATH）
        ProcessTestExecution::new().run(&root_str, CHANGE).await
    };

    let err = result.expect_err("程序不可达应 Err（基础设施失败显式停给用户）");
    assert!(
        err.contains("node") && err.contains("未找到"),
        "Err 记因裸名与解析失败面（不产部分结论），实际: {err}"
    );
}

/// 异常：命令 fixture 非零退出且工件不可解析 → conclusion=error 产出 +
/// findings_brief 在场（报告级 error 走反馈边原料面，不 Err）。
#[tokio::test]
async fn 非零退出且工件不可解析_error结论产出() {
    let ws = ChainWs::new("error-chain");
    let marker = ws.root().join("shim-marker.log");
    // 零工件源（shim 不拷贝）+ 退出码 1
    let absent = ws.root().join("absent-results.txt");

    let output = {
        let _window = path_window();
        set_shim_env(&absent, &ws.results_dest(), Some(&marker), Some(1));
        ws.run().await
    }
    .expect("报告级 error 走产出（不 Err）");

    let outcome = match output {
        ToolStepOutput::TestExecution(outcome) => outcome,
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    };
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Error,
        "非零退出零工件 → error"
    );
    assert_eq!(
        outcome.failed, 0,
        "零失败用例（error 来自执行错误面而非用例失败）"
    );
    assert!(
        outcome.findings_brief.contains("执行错误归因"),
        "findings_brief 在场（反馈边原料面），实际: {}",
        outcome.findings_brief
    );
    assert_eq!(ws.marker_count(), 1, "error 结论非复用（恰一次执行）");
}

// ---------------------------------------------------------------------------
// findings_brief 装配口径与 Conclusion → port 三值映射
// ---------------------------------------------------------------------------

/// 手工 summary fixture（装配口径定向输入面）。
fn hand_summary(conclusion: Conclusion, findings: Vec<String>) -> SummaryReport {
    SummaryReport {
        phase: "test-execution".to_owned(),
        command: "cmd".to_owned(),
        timestamp: "2026-10-06T08:00:00Z".to_owned(),
        duration_seconds: 1.0,
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
        conclusion,
        problems: Vec::new(),
        coverage: None,
        mutation: None,
        plans: Vec::new(),
        findings: Some(findings),
    }
}

/// 边界：findings_brief 装配口径——单条 200 字符截断、至多 10 条、report_dir
/// 在载荷中可定位全量 findings 报告文件（walker diagnose_brief 口径对齐）。
#[test]
fn findings_brief装配口径_截断与条数上限() {
    let long = "长".repeat(250);
    let findings: Vec<String> = (0..12).map(|index| format!("{index:02} {long}")).collect();
    let summary = hand_summary(Conclusion::Fail, findings);

    let brief = findings_brief(&summary);
    let lines: Vec<&str> = brief.lines().collect();
    assert_eq!(lines.len(), 10, "至多 10 条（第 11 条起弃）");
    for line in &lines {
        assert!(
            line.chars().count() <= 201,
            "单条 ≤ 200 字符 + 截断省略号，实际 {} 字符",
            line.chars().count()
        );
    }
    assert!(lines[0].ends_with('…'), "超长条目截断带省略号");

    // 短条目原样透传（不截断不填充）
    let short = hand_summary(Conclusion::Fail, vec!["行覆盖 62.4% < 阈值 80%".to_owned()]);
    assert_eq!(findings_brief(&short), "行覆盖 62.4% < 阈值 80%");

    // findings 缺省回落 problems 消息面（CLI 产出的复用 summary 诊断面不空转）
    let mut no_findings = hand_summary(Conclusion::Error, Vec::new());
    no_findings.findings = None;
    no_findings.problems = vec![checks::model::Problem {
        framework: "node-test".to_owned(),
        problem_type: checks::model::ProblemType::ExecutionError,
        message: "Exit code 1".to_owned(),
    }];
    assert_eq!(
        findings_brief(&no_findings),
        "Exit code 1",
        "回落 problems 消息面"
    );
}

/// 边界：checks Conclusion → port TestExecutionConclusion 三值映射逐字
///（pass / fail / error 小写线格式——checks-runtime 装配点唯一映射面）。
#[test]
fn conclusion三值映射逐字() {
    let reports_dir = Path::new("/tmp/reports");
    for (checks_conclusion, wire) in [
        (Conclusion::Pass, "pass"),
        (Conclusion::Fail, "fail"),
        (Conclusion::Error, "error"),
    ] {
        let output = outcome(&hand_summary(checks_conclusion, Vec::new()), reports_dir);
        match output {
            ToolStepOutput::TestExecution(mapped) => {
                assert_eq!(
                    mapped.conclusion.as_str(),
                    wire,
                    "三值映射线格式逐字 {wire}"
                );
                assert_eq!(
                    mapped.report_dir,
                    to_posix(reports_dir),
                    "report_dir 随载荷"
                );
            }
            other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
        }
    }
}

/// 正向：无状态构造可经 Arc<dyn TestExecutionRunner> 注入（组合根同式装配
/// 编译锚——Send + Sync + object safety）。
#[test]
fn 无状态构造经arc注入编译锚() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn TestExecutionRunner>>();
    let runner: Arc<dyn TestExecutionRunner> = Arc::new(ProcessTestExecution::new());
    // trait 面编译锚（run 返回 BoxToolFuture——进程内可 await）
    let _ = runner as Arc<dyn TestExecutionRunner>;
}

// ---------------------------------------------------------------------------
// newest_input_mtime_ms：输入最新 mtime 递归扫描
// ---------------------------------------------------------------------------

/// 扫描 fixture 树：命中 / dot 剪枝 / 排除目录名剪枝 / 剪枝目录树四形态。
fn scan_tree(root: &Path) {
    for rel in [
        "a.txt",
        "src/b.txt",
        "src/deep/c.txt",
        ".dot/hidden.txt",
        "node_modules/x.txt",
        "coverage/y.txt",
        "pruned/inside/z.txt",
    ] {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("有父目录")).expect("创建目录");
        fs::write(&path, rel).expect("写文件");
    }
}

fn set_mtime(path: &Path, offset_secs: u64) {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("打开文件")
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(offset_secs))
        .expect("推进 mtime");
}

/// 正向：多层文件树最新 mtime 提取——touch 递进可观测（剪枝后仍取到输入
/// 最新值）。
#[test]
fn 多层文件树最新mtime提取() {
    let dir = TempDir::new().expect("临时目录");
    let scan_root = dir.path().to_path_buf();
    scan_tree(&scan_root);

    let pruned = vec![dir.path().join("pruned")];
    // 剪枝目录树外三层文件均在扫描面
    let newest = newest_input_mtime_ms(&[scan_root.clone()], &pruned).expect("有输入文件");
    assert!(newest > 0);

    // touch 深层文件 → 最新 mtime 递进可观测
    set_mtime(&scan_root.join("src").join("deep").join("c.txt"), 30);
    let touched = newest_input_mtime_ms(&[scan_root.clone()], &pruned).expect("touch 后仍有输入");
    assert!(touched > newest, "touch 递进最新值（剪枝后仍取到输入最新）");

    // 多扫描根并集取最大
    let other = dir.path().join("other-root");
    fs::create_dir_all(&other).expect("创建第二扫描根");
    fs::write(other.join("late.txt"), "x").expect("写文件");
    set_mtime(&other.join("late.txt"), 60);
    let merged = newest_input_mtime_ms(&[scan_root, other], &pruned).expect("并集非空");
    assert!(merged >= touched, "多扫描根并集取最大");
}

/// 边界：报告目录 / dot 目录 / 排除目录名 / 剪枝目录树内改动不计入（剪枝
/// 生效——复用门不被自身报告误失效）。
#[test]
fn 剪枝面目录内改动不计入() {
    let dir = TempDir::new().expect("临时目录");
    let scan_root = dir.path().to_path_buf();
    scan_tree(&scan_root);
    let pruned = vec![dir.path().join("pruned"), dir.path().join("reports-dir")];

    let baseline = newest_input_mtime_ms(&[scan_root.clone()], &pruned).expect("基线");

    // 报告目录（剪枝目录）、dot 目录、node_modules / coverage / target 内推进
    // mtime → 不计入
    let reports_dir = dir.path().join("reports-dir");
    fs::create_dir_all(&reports_dir).expect("创建报告目录");
    fs::write(reports_dir.join("summary.json"), "{}").expect("写报告");
    set_mtime(&reports_dir.join("summary.json"), 90);
    set_mtime(&scan_root.join(".dot").join("hidden.txt"), 90);
    set_mtime(&scan_root.join("node_modules").join("x.txt"), 90);
    set_mtime(&scan_root.join("coverage").join("y.txt"), 90);
    set_mtime(&scan_root.join("pruned").join("inside").join("z.txt"), 90);

    let after = newest_input_mtime_ms(&[scan_root], &pruned).expect("扫描面非空");
    assert_eq!(
        after, baseline,
        "剪枝面目录内改动不计入（复用门不被自身报告误失效）"
    );
}

/// 边界：空扫描根（无文件）→ None（视为新鲜——复用门空输入集语义）。
#[test]
fn 空扫描根返回none() {
    let dir = TempDir::new().expect("临时目录");
    // 空目录
    assert_eq!(
        newest_input_mtime_ms(&[dir.path().to_path_buf()], &[]),
        None
    );
    // 不存在的扫描根
    assert_eq!(
        newest_input_mtime_ms(&[dir.path().join("ghost")], &[]),
        None,
        "不存在根不 panic 返回 None"
    );
    // 空扫描根集
    assert_eq!(newest_input_mtime_ms(&[], &[]), None);
}
