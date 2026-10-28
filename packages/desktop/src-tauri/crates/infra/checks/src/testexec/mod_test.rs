use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, MutexGuard};

use tempfile::TempDir;

use checks::model::{
    CaseSummary, Conclusion, CoverageBlock, CoverageMeasured, CoverageThresholds, Problem,
    ProblemType, SubReport, SummaryReport, TestCaseResult, TestCaseStatus,
};
use foundation::layout::{change_reports, change_test_reports, ReportSlot};
use orchestration::port::{
    RunFault, TestExecutionConclusion, TestExecutionOutcome, TestExecutionRunner, ToolStepOutput,
};

use super::detect::{to_posix, TestPlan};
use super::runner::PlanExecution;
use super::{
    build_sub_report, execute, findings_brief, findings_detail, newest_input_mtime_ms, outcome,
    push_plan_head, ProcessTestExecution, TESTEXEC_TOTAL_BUDGET,
};
use checks::parser::CoverageFormat;

/// 设置 shim 环境面（预置工件源 / 目的工件路径 / marker 路径 / 退出码；
/// None 侧摘除键）。dest 用 POSIX 形态（Copy-Item / cp 双侧可读）。
/// 缺省槽位（`ReportSlot::Test`）outcome 装配包装——既有用例零改动。
fn outcome_test(
    root: &Path,
    change: &str,
    summary: &SummaryReport,
    sub_reports: &[SubReport],
    reports_dir: &Path,
    merged_output: &[u8],
) -> ToolStepOutput {
    outcome(
        root,
        change,
        ReportSlot::Test,
        summary,
        sub_reports,
        reports_dir,
        merged_output,
    )
}

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

/// 第二产物目的路径注入（多 plan 面：一次执行臂同时喂两枚 plan 的工件锚；
/// 缺省不设即既有单工件事形态）。
fn set_shim_dest_2(dest: &Path) {
    std::env::set_var("SHIM_DEST_2", dest.to_string_lossy().replace('\\', "/"));
}

/// 摘除全部 shim 环境键（窗口收口——不污染并行用例）。
fn clear_shim_env() {
    std::env::remove_var("SHIM_RESULTS_SRC");
    std::env::remove_var("SHIM_DEST");
    std::env::remove_var("SHIM_DEST_2");
    std::env::remove_var("SHIM_MARKER");
    std::env::remove_var("SHIM_EXIT");
    std::env::remove_var("SHIM_SPIN_AFTER");
    std::env::remove_var("SHIM_PIDFILE");
    std::env::remove_var("SHIM_LOCK");
    std::env::remove_var("SHIM_LOCK_FIRST");
    std::env::remove_var("SHIM_LOCK_COUNT");
    std::env::remove_var("SHIM_SLEEP_MS");
    std::env::remove_var("NPX_SLEEP_MS");
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
        // 追加执行 marker、按 SHIM_EXIT 退出。
        //
        // **`--version` 臂在 .cmd 层短接**（MUST NOT 落到 PowerShell）：该臂由
        // `detect_plans` 逐 suite 各 spawn 一次，而跨 plan 总预算的起点在 detect
        // **之前**（生产口径如此），故探测耗时直接吃掉预算余量。PowerShell 单次
        // 冷启在高负载下实测 0.7–2.5 s（cmd 批处理仅数十毫秒），足以让 1 s 级
        // 预算测试在门禁并发下「首个 plan 边界即判超预算」——零 spawn 却断言
        // 首个 plan 已跑，即 flake 的根因。
        #[cfg(target_os = "windows")]
        {
            fs::write(
                root.join("app").join("node.cmd"),
                "@echo off\r\n\
                 if \"%~1\"==\"--version\" exit /b 0\r\n\
                 powershell -NoProfile -ExecutionPolicy Bypass -File \"%~dp0node-shim.ps1\" %*\r\n\
                 exit /b %errorlevel%\r\n",
            )
            .expect("写 node.cmd 失败");
            fs::write(
                root.join("app").join("node-shim.ps1"),
                "if ($args.Count -gt 0 -and $args[0] -eq '--version') { exit 0 }\n\
                 if ($env:SHIM_SLEEP_MS) { Start-Sleep -Milliseconds ([int]$env:SHIM_SLEEP_MS) }\n\
                 if ($env:SHIM_LOCK_FIRST) {\n  $locked = 0\n  if ($env:SHIM_LOCK_COUNT -and (Test-Path $env:SHIM_LOCK_COUNT)) { $locked = @(Get-Content $env:SHIM_LOCK_COUNT).Count }\n  if ($locked -lt [int]$env:SHIM_LOCK_FIRST) {\n    if ($env:SHIM_LOCK_COUNT) { Add-Content -Force -Path $env:SHIM_LOCK_COUNT -Value 'locked' }\n    Write-Output $env:SHIM_LOCK\n    exit 1\n  }\n}\n                 if ($env:SHIM_SPIN_AFTER) {\n\
                   $seen = 0\n\
                   if ($env:SHIM_MARKER -and (Test-Path $env:SHIM_MARKER)) { $seen = @(Get-Content $env:SHIM_MARKER).Count }\n\
                   if ($seen -ge [int]$env:SHIM_SPIN_AFTER) {\n\
                     if ($env:SHIM_PIDFILE) { Set-Content -Path $env:SHIM_PIDFILE -Value $PID -NoNewline -Encoding ascii }\n\
                     while ($true) { Start-Sleep -Milliseconds 100 }\n\
                   }\n\
                 }\n\
                 if ($env:SHIM_RESULTS_SRC -and $env:SHIM_DEST) { Copy-Item -Force $env:SHIM_RESULTS_SRC $env:SHIM_DEST }\n\
                 if ($env:SHIM_RESULTS_SRC -and $env:SHIM_DEST_2) { Copy-Item -Force $env:SHIM_RESULTS_SRC $env:SHIM_DEST_2 }\n\
                 if ($env:SHIM_DEST -and $env:SHIM_MARKER) { Add-Content -Force -Path $env:SHIM_MARKER -Value 'ran' }\n\
                 Write-Output 'shim-stdout-marker'\n\
                 if ($env:SHIM_LOCK) { Write-Output $env:SHIM_LOCK }\n\
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
                 if [ -n \"$SHIM_SLEEP_MS\" ]; then sleep $((SHIM_SLEEP_MS / 1000)); fi\n\
                 if [ -n \"$SHIM_LOCK_FIRST\" ]; then\n  locked=0\n  if [ -n \"$SHIM_LOCK_COUNT\" ] && [ -f \"$SHIM_LOCK_COUNT\" ]; then locked=$(wc -l < \"$SHIM_LOCK_COUNT\"); fi\n  if [ \"$locked\" -lt \"$SHIM_LOCK_FIRST\" ]; then\n    if [ -n \"$SHIM_LOCK_COUNT\" ]; then echo locked >> \"$SHIM_LOCK_COUNT\"; fi\n    echo \"$SHIM_LOCK\"\n    exit 1\n  fi\nfi\n                 if [ -n \"$SHIM_SPIN_AFTER\" ]; then\n\
                   seen=0\n\
                   if [ -n \"$SHIM_MARKER\" ] && [ -f \"$SHIM_MARKER\" ]; then seen=$(wc -l < \"$SHIM_MARKER\"); fi\n\
                   if [ \"$seen\" -ge \"$SHIM_SPIN_AFTER\" ]; then\n\
                     if [ -n \"$SHIM_PIDFILE\" ]; then printf '%s' \"$$\" > \"$SHIM_PIDFILE\"; fi\n\
                     while :; do sleep 0.1; done\n\
                   fi\n\
                 fi\n\
                 if [ -n \"$SHIM_RESULTS_SRC\" ] && [ -n \"$SHIM_DEST\" ]; then cp \"$SHIM_RESULTS_SRC\" \"$SHIM_DEST\"; fi\n\
                 if [ -n \"$SHIM_RESULTS_SRC\" ] && [ -n \"$SHIM_DEST_2\" ]; then cp \"$SHIM_RESULTS_SRC\" \"$SHIM_DEST_2\"; fi\n\
                 if [ -n \"$SHIM_DEST\" ] && [ -n \"$SHIM_MARKER\" ]; then echo ran >> \"$SHIM_MARKER\"; fi\n\
                 echo shim-stdout-marker\n\
                 if [ -n \"$SHIM_LOCK\" ]; then echo \"$SHIM_LOCK\"; fi\n\
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

    /// 槽位版报告树（baseline 分树断言面——check-before-flow-start）。
    fn reports_dir_slot(&self, slot: ReportSlot) -> PathBuf {
        change_reports(self.root(), CHANGE, slot)
    }

    /// 双 plan 变体（两个 node-test suite：app / app2——多 plan 中途取消面）：
    /// 覆写 config 并补第二 suite 输入文件（同一 `node` shim 经 PATH 解析，
    /// 执行臂计数经 SHIM_MARKER 行数区分 plan 序）。
    fn two_plan(self) -> Self {
        fs::write(
            self.root().join("openspec").join("config.json"),
            r#"{ "tests": [
  { "root": "app", "framework": "node-test", "coverage": { "lines": 60, "branches": 70, "functions": 75 } },
  { "root": "app2", "framework": "node-test", "coverage": { "lines": 60, "branches": 70, "functions": 75 } }
] }"#,
        )
        .expect("覆写双 plan config");
        fs::create_dir_all(self.root().join("app2").join("src")).expect("创建第二 suite src");
        fs::write(
            self.root().join("app2").join("src").join("b.test.mjs"),
            "test body",
        )
        .expect("写第二 suite 输入文件");
        self
    }

    /// 三 plan 混合变体（node-test 绿 / jest 环境错误 / node-test 绿——「`Environment`
    /// 之后继续跑完余下 plan」与「`sub_reports` 内部装配不泄漏到 outcome」的观测
    /// 装置）：覆写三 suite config、补第二 / 第三 suite 输入文件（各框架
    /// default_glob 可命中），并在 PATH 前置目录内落一枚恒锁 `npx` shim（jest 档
    /// 命令专用：执行臂逐次记账 → 输出锁文本 → 非零退出，瞬态签名命中后 5 次
    /// 耗尽转 `Environment`；版本探测臂不计账直接非零）。
    ///
    /// 执行臂支持 `NPX_SLEEP_MS` 睡眠键：jest 档耗时是「预算是否在第三 plan 边界
    /// 才触发」的唯一可控量，靠 PowerShell 冷启的真耗时不可控（空闲与高负载可差
    /// 5 倍以上），故由消费用例显式注入。
    fn three_plan_mixed(self) -> Self {
        fs::write(
            self.root().join("openspec").join("config.json"),
            r#"{ "tests": [
  { "root": "app", "framework": "node-test", "coverage": { "lines": 60, "branches": 70, "functions": 75 } },
  { "root": "app2", "framework": "jest", "coverage": { "lines": 60, "branches": 70, "functions": 75 } },
  { "root": "app3", "framework": "node-test", "coverage": { "lines": 60, "branches": 70, "functions": 75 } }
] }"#,
        )
        .expect("覆写三 plan config");
        fs::create_dir_all(self.root().join("app2").join("src")).expect("创建第二 suite src");
        fs::write(
            self.root().join("app2").join("src").join("b.test.js"),
            "test body",
        )
        .expect("写第二 suite 输入文件");
        fs::create_dir_all(self.root().join("app3").join("src")).expect("创建第三 suite src");
        fs::write(
            self.root().join("app3").join("src").join("c.test.mjs"),
            "test body",
        )
        .expect("写第三 suite 输入文件");

        let attempt_log = self.npx_attempt_log();
        #[cfg(target_os = "windows")]
        {
            fs::write(
                self.root().join("app").join("npx.cmd"),
                "@echo off\r\npowershell -NoProfile -ExecutionPolicy Bypass -File \"%~dp0npx-shim.ps1\" %*\r\nexit /b %errorlevel%\r\n",
            )
            .expect("写 npx.cmd 失败");
            fs::write(
                self.root().join("app").join("npx-shim.ps1"),
                format!(
                    "if ($args -contains '--version') {{ exit 1 }}\n\
                     if ($env:NPX_SLEEP_MS) {{ Start-Sleep -Milliseconds ([int]$env:NPX_SLEEP_MS) }}\n\
                     Add-Content -Force -Path '{}' -Value 'attempt'\n\
                     Write-Output 'error: failed to rename archive file: /t/liba.rlib: os error 32'\n\
                     exit 1\n",
                    attempt_log.display()
                ),
            )
            .expect("写 npx-shim.ps1 失败");
        }
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = self.root().join("app").join("npx");
            fs::write(
                &script,
                format!(
                    "#!/bin/sh\n\
                     for a in \"$@\"; do [ \"$a\" = \"--version\" ] && exit 1; done\n\
                     if [ -n \"$NPX_SLEEP_MS\" ]; then sleep $((NPX_SLEEP_MS / 1000)); fi\n\
                     echo attempt >> '{}'\n\
                     echo 'error: failed to rename archive file: /t/liba.rlib: os error 32'\n\
                     exit 1\n",
                    attempt_log.display()
                ),
            )
            .expect("写 npx shim 失败");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("补执行位");
        }
        self
    }

    /// jest 档执行臂记账文件（三 plan 混合面的 spawn 次数观测面）。
    fn npx_attempt_log(&self) -> PathBuf {
        self.root().join("npx-attempt.log")
    }

    /// 第三 suite（app3）的结果工件目的路径（一次执行臂双投面）。
    fn results_dest_third(&self) -> PathBuf {
        self.reports_dir()
            .join("app3_node-test")
            .join("results.txt")
    }

    /// PATH 前置 suite cwd（裸名 `node` 解析到 shim；须在 path_window 窗口内
    /// 调用——窗口收口恢复原 PATH）。多 plan / 取消面用例在 spawn 任务前置用。
    fn prepend_path(&self) {
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(self.root().join("app")).chain(
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
            ))
            .expect("join PATH"),
        );
    }

    /// 显式取消信号版执行（取消面用例的注入点；PATH 前置同 [`Self::run`]）。
    async fn run_with(
        &self,
        cancel: foundation::cancel::CancelFlag,
    ) -> Result<ToolStepOutput, String> {
        self.prepend_path();
        ProcessTestExecution::new()
            .run(&self.root_str(), CHANGE, ReportSlot::Test, cancel)
            .await
    }

    /// 槽位参数版执行（PATH 前置窗口 + 指定槽位 spawn——baseline 树断言面）。
    async fn run_slot(&self, slot: ReportSlot) -> Result<ToolStepOutput, String> {
        self.prepend_path();
        ProcessTestExecution::new()
            .run(
                &self.root_str(),
                CHANGE,
                slot,
                foundation::cancel::CancelFlag::never(),
            )
            .await
    }

    /// 执行臂的结果工件目的路径（planId 子目录 results.txt——SHIM_DEST 注入面）。
    fn results_dest(&self) -> PathBuf {
        self.reports_dir().join("app_node-test").join("results.txt")
    }

    /// 槽位版结果工件目的路径（runner 按槽位树读工件——SHIM_DEST 须同锚）。
    fn results_dest_slot(&self, slot: ReportSlot) -> PathBuf {
        self.reports_dir_slot(slot)
            .join("app_node-test")
            .join("results.txt")
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
        self.prepend_path();
        ProcessTestExecution::new()
            .run(
                &self.root_str(),
                CHANGE,
                ReportSlot::Test,
                foundation::cancel::CancelFlag::never(),
            )
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
        let shim_env = crate::lock_test_env();
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
/// planId/report.json 落 change_test_reports 推导目录（AC-5）。生产调用方
/// `ProcessTestExecution::run` 以 `TESTEXEC_TOTAL_BUDGET` 全预算执行（预算内零行为
/// 变化——本用例即该档的全链锚）。
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

    // 输出记录单点（AC-7）：record 内容 = 脚本真实输出合并流（stdout 标记在场），
    // 非 summary JSON 重序列化；单一 `.log` 落 report_dir 同锚
    assert!(!outcome.exec_id.is_empty(), "门禁产出恒回填 exec_id");
    let recorded = exec::read_exec_output(ws.root(), CHANGE, &outcome.exec_id)
        .expect("门禁引用可回查脚本输出");
    let text = String::from_utf8_lossy(&recorded);
    assert!(
        text.contains("shim-stdout-marker"),
        "record 内容为脚本真实输出（stdout 流），实际: {text:?}"
    );
    assert!(
        !text.trim_start().starts_with('{'),
        "record 内容非 summary JSON 重序列化，实际: {text:?}"
    );
    assert!(
        !reports_dir
            .join("exec-outputs")
            .join(format!("{}.err", outcome.exec_id))
            .exists(),
        ".err 分文件退役（单一 .log）"
    );
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
    // 复用产出与首跑逐字段一致（exec_id 为每次装配的落盘引用，恒新铸——
    // 记录单点面独立于复用门，余下字段零漂移）
    let (ToolStepOutput::TestExecution(first_out), ToolStepOutput::TestExecution(second_out)) =
        (&first, &second)
    else {
        panic!("两跑产出均应为 TestExecution 变体");
    };
    assert_ne!(
        first_out.exec_id, second_out.exec_id,
        "落盘引用每次装配新铸"
    );
    assert_eq!(
        TestExecutionOutcome {
            exec_id: String::new(),
            ..first_out.clone()
        },
        TestExecutionOutcome {
            exec_id: String::new(),
            ..second_out.clone()
        },
        "除落盘引用外复用产出与首跑逐字段一致"
    );

    // 复用分支输出记录面（AC-7）：复用零 spawn → 本次合并流空，落空 `.log`（引用
    // 每次新铸）；首跑 `.log` 不被复用跑覆写（记录单点面独立于复用门）
    let first_record =
        exec::read_exec_output(ws.root(), CHANGE, &first_out.exec_id).expect("首跑记录可回查");
    assert!(
        String::from_utf8_lossy(&first_record).contains("shim-stdout-marker"),
        "首跑记录 = 脚本真实输出: {:?}",
        String::from_utf8_lossy(&first_record)
    );
    let reused_record = exec::read_exec_output(ws.root(), CHANGE, &second_out.exec_id)
        .expect("复用跑引用同样可回查（每次新铸）");
    assert!(
        reused_record.is_empty(),
        "复用分支零 spawn → 空合并流落空 .log，实际: {:?}",
        String::from_utf8_lossy(&reused_record)
    );
    assert_eq!(
        exec::read_exec_output(ws.root(), CHANGE, &first_out.exec_id).expect("重复回查"),
        first_record,
        "首跑 .log 不被复用跑覆写"
    );
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
        ProcessTestExecution::new()
            .run(
                &root_str,
                CHANGE,
                ReportSlot::Test,
                foundation::cancel::CancelFlag::never(),
            )
            .await
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
    no_findings.problems = vec![Problem {
        framework: "node-test".to_owned(),
        problem_type: ProblemType::ExecutionError,
        message: "Exit code 1".to_owned(),
    }];
    assert_eq!(
        findings_brief(&no_findings),
        "Exit code 1",
        "回落 problems 消息面"
    );
}

/// 失败用例 fixture（名称 + 文件行号 + 类型 / 消息可编程，堆栈缺省无）。
fn failed_case(name: &str, message: &str) -> TestCaseResult {
    TestCaseResult {
        name: name.to_owned(),
        file: Some("src/export.test.mjs".to_owned()),
        duration_ms: None,
        status: TestCaseStatus::Failed,
        line: Some(23.0),
        error_type: Some("AssertionError".to_owned()),
        error_message: Some(message.to_owned()),
        stack_trace: None,
    }
}

/// 子报告 fixture（框架 / 失败计数 / 错误用例 / 覆盖块可编程）。
fn hand_sub_report(
    framework: &str,
    failed: u64,
    cases: Vec<TestCaseResult>,
    coverage: Option<CoverageBlock>,
) -> SubReport {
    SubReport {
        framework: framework.to_owned(),
        root: format!("packages/{framework}"),
        timestamp: "2026-10-06T08:00:00Z".to_owned(),
        exit_code: 1,
        duration_ms: 10.0,
        summary: CaseSummary {
            total: cases.len() as u64 + 3,
            passed: 3,
            failed,
            skipped: 0,
        },
        error_cases: cases,
        test_files: Vec::new(),
        source_files: Vec::new(),
        coverage,
        mutation: None,
        findings: None,
    }
}

/// 边界：findings_detail 装配口径——pass 恒空串；fail / error 三段齐装（诊断
/// 全量、suite 概览含覆盖对照（null 维度跳过）、失败用例明细携名称 / 文件
/// 行号 / 类型 / 消息）——反馈边修复 prompt 直嵌面。
#[test]
fn findings_detail装配口径_诊断概览与用例明细() {
    // pass 态恒空串（反馈边不消费）
    let pass = hand_summary(
        Conclusion::Pass,
        vec!["全部测试通过且覆盖率达阈值".to_owned()],
    );
    assert_eq!(findings_detail(&pass, &[]), "", "pass 态恒空串");

    let summary = hand_summary(
        Conclusion::Fail,
        vec!["「node-test」1 项测试失败——单点失败（疑似 flaky 用例或孤立回归）".to_owned()],
    );
    let sub = hand_sub_report(
        "node-test",
        1,
        vec![failed_case(
            "汇总导出_空清单回落",
            "expected '[]' to equal '[a]'",
        )],
        Some(CoverageBlock {
            pass: false,
            measured: CoverageMeasured {
                lines: Some(62.4),
                branches: None,
                functions: None,
            },
            thresholds: CoverageThresholds {
                lines: 80.0,
                branches: 70.0,
                functions: 75.0,
            },
            overrides: None,
        }),
    );

    let detail = findings_detail(&summary, &[sub]);
    assert!(detail.contains("## 诊断"), "诊断段在场: {detail}");
    assert!(
        detail.contains("单点失败"),
        "诊断 findings 全量（非 200 截断摘要）"
    );
    assert!(detail.contains("## suite 执行概览"), "概览段在场: {detail}");
    assert!(
        detail.contains(
            "- [node-test] root=packages/node-test exit=1 total=4 passed=3 failed=1 skipped=0"
        ),
        "概览行携框架 / 锚点 / 退出码 / 计数: {detail}"
    );
    assert!(
        detail.contains("行覆盖 62.4%（阈值 80%）"),
        "覆盖对照随概览行: {detail}"
    );
    assert!(!detail.contains("分支覆盖"), "null 维度跳过: {detail}");
    assert!(
        detail.contains("## 失败用例明细"),
        "用例明细段在场: {detail}"
    );
    assert!(
        detail.contains("1. 汇总导出_空清单回落（src/export.test.mjs:23）"),
        "用例定位行携名称 + 文件行号（整数值去小数尾）: {detail}"
    );
    assert!(
        detail.contains("类型: AssertionError"),
        "错误类型随行: {detail}"
    );
    assert!(
        detail.contains("消息: expected '[]' to equal '[a]'"),
        "错误消息随行: {detail}"
    );

    // findings 缺省回落 problems 消息面（摘要 / 明细双装配同回落）
    let mut no_findings = hand_summary(Conclusion::Error, Vec::new());
    no_findings.findings = None;
    no_findings.problems = vec![Problem {
        framework: "node-test".to_owned(),
        problem_type: ProblemType::ExecutionError,
        message: "Exit code 1".to_owned(),
    }];
    assert_eq!(
        findings_detail(&no_findings, &[]),
        "## 诊断\n\nExit code 1",
        "回落 problems 消息面"
    );
}

/// 边界：findings_detail 有界执法——单条消息 500 / 堆栈 800 截断、每 suite
/// 20 条上限（余量记数）、总预算 12000 截断尾注指向报告目录。
#[test]
fn findings_detail装配口径_截断与条数上限() {
    let summary = hand_summary(Conclusion::Fail, vec!["失败措辞".to_owned()]);

    // 单条消息 500 / 堆栈 800 截断（chars 口径 + 省略号）
    let mut long_case = failed_case("超长面用例", &"错".repeat(600));
    long_case.stack_trace = Some("帧".repeat(900));
    let detail = findings_detail(
        &summary,
        &[hand_sub_report("rust", 1, vec![long_case], None)],
    );
    let message_line = detail
        .lines()
        .find(|line| line.trim_start().starts_with("消息: "))
        .expect("消息行在场");
    assert!(
        message_line.chars().count() <= "   消息: ".chars().count() + 501,
        "消息截 500 字符 + 省略号: {message_line}"
    );
    assert!(message_line.ends_with('…'), "截断带省略号");
    let stack_line = detail
        .lines()
        .find(|line| line.trim_start().starts_with("堆栈: "))
        .expect("堆栈行在场");
    assert!(
        stack_line.chars().count() <= "   堆栈: ".chars().count() + 801,
        "堆栈截 800 字符 + 省略号: {stack_line}"
    );

    // 每 suite 20 条上限 + 余量记数指向报告
    let cases: Vec<TestCaseResult> = (0..25)
        .map(|index| failed_case(&format!("用例{index}"), "m"))
        .collect();
    let detail = findings_detail(&summary, &[hand_sub_report("rust", 25, cases, None)]);
    assert!(detail.contains("20. 用例19"), "至多 20 条: {detail}");
    assert!(!detail.contains("21. "), "第 21 条起弃: {detail}");
    assert!(detail.contains("另有 5 条见报告"), "余量记数: {detail}");

    // 总预算 12000 截断尾注（20 条 × 满额消息 + 堆栈远超预算）
    let heavy: Vec<TestCaseResult> = (0..25)
        .map(|index| {
            let mut case = failed_case(&format!("重用例{index}"), &"错".repeat(600));
            case.stack_trace = Some("帧".repeat(900));
            case
        })
        .collect();
    let detail = findings_detail(&summary, &[hand_sub_report("rust", 25, heavy, None)]);
    assert!(
        detail.contains("明细超预算截断，全量见报告目录"),
        "总预算超限尾注在场: 尾部 = {}",
        detail.chars().rev().take(60).collect::<String>()
    );
    assert!(
        detail.chars().count() <= 12000 + "…\n（明细超预算截断，全量见报告目录）".chars().count(),
        "总长受预算 + 尾注约束，实际 {}",
        detail.chars().count()
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
        let dir = TempDir::new().expect("临时目录");
        let output = outcome_test(
            dir.path(),
            "demo-change",
            &hand_summary(checks_conclusion, Vec::new()),
            &[],
            reports_dir,
            &[],
        );
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
    let newest =
        newest_input_mtime_ms(std::slice::from_ref(&scan_root), &pruned).expect("有输入文件");
    assert!(newest > 0);

    // touch 深层文件 → 最新 mtime 递进可观测
    set_mtime(&scan_root.join("src").join("deep").join("c.txt"), 30);
    let touched =
        newest_input_mtime_ms(std::slice::from_ref(&scan_root), &pruned).expect("touch 后仍有输入");
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

    let baseline = newest_input_mtime_ms(std::slice::from_ref(&scan_root), &pruned).expect("基线");

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

/// 正向：一次门禁装配 → `exec_id` 与既有 `report_dir` 同口径（输出记录单点经
/// infra/exec 落单一 `.log`）；`read_exec_output` 回查内容 = 入参脚本真实输出
/// 合并流（多 suite 按 plan 序拼接保真），**非 summary JSON 重序列化**；`.err`
/// 分支退役（单一 `.log`）。
#[test]
fn 门禁exec_id落脚本输出且与report_dir同口径() {
    let dir = TempDir::new().expect("临时目录");
    let reports_dir = dir.path().join("openspec/changes/demo-change/reports/test");
    let summary = hand_summary(Conclusion::Pass, vec!["找不到问题".to_owned()]);
    // 两 suite 输出按 plan 序拼接后的合并流（聚合序在 execute 循环内，记录单点
    // 按传入序落盘保序）
    let merged = b"suite-a stdout\nsuite-a stderr\nsuite-b stdout\n".to_vec();

    let output = outcome_test(
        dir.path(),
        "demo-change",
        &summary,
        &[],
        &reports_dir,
        &merged,
    );
    let ToolStepOutput::TestExecution(mapped) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    assert!(!mapped.exec_id.is_empty(), "exec_id 铸造非空（记录成功）");
    assert!(
        mapped.exec_id.starts_with("exec-"),
        "exec_id 定式: {}",
        mapped.exec_id
    );
    // 与 report_dir 同锚：输出记录目录驻 report_dir 下的 exec-outputs/
    let expected_dir =
        foundation::layout::change_test_reports(dir.path(), "demo-change").join("exec-outputs");
    assert!(
        expected_dir
            .join(format!("{}.log", mapped.exec_id))
            .exists(),
        "落盘位于 report_dir 同锚的 exec-outputs/<exec_id>.log"
    );
    assert!(
        !expected_dir
            .join(format!("{}.err", mapped.exec_id))
            .exists(),
        ".err 分文件退役（单一 .log）"
    );

    let read_back =
        exec::read_exec_output(dir.path(), "demo-change", &mapped.exec_id).expect("回查应命中");
    assert_eq!(
        read_back, merged,
        "record 内容 = 脚本真实输出合并流（逐字节、保序）"
    );
    let text = String::from_utf8(read_back).expect("脚本输出为 UTF-8");
    assert!(
        text.contains("suite-a stdout") && text.contains("suite-b stdout"),
        "各 suite 输出逐段在场: {text:?}"
    );
}

/// 边界 / 正向：多 suite 空 `output` → `merged_output` 空，仍落空 `.log` 可回查
///（`Some(vec![])`）；回查内容 ≠ `serde_json::to_vec(summary)`（record 去
/// summary JSON 重序列化）。
#[test]
fn 空合并流落空log可回查且非summary序列化() {
    let dir = TempDir::new().expect("临时目录");
    let reports_dir = dir.path().join("openspec/changes/demo-change/reports/test");
    let summary = hand_summary(Conclusion::Error, vec!["执行错误归因".to_owned()]);

    let output = outcome_test(dir.path(), "demo-change", &summary, &[], &reports_dir, &[]);
    let ToolStepOutput::TestExecution(mapped) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    let read_back =
        exec::read_exec_output(dir.path(), "demo-change", &mapped.exec_id).expect("空流仍应可回查");
    assert_eq!(read_back, Vec::<u8>::new(), "空合并流 → 空 .log");
    let summary_json = serde_json::to_vec(&summary).expect("summary 可序列化");
    assert_ne!(
        read_back, summary_json,
        "record 内容非 summary JSON 重序列化（去 serde summary 面）"
    );
}

/// 幂等：重复装配（重跑 / 复用分支）→ 每次新铸 `exec_id`、单一 `.log` 独立落盘
/// 互不覆写；report_dir 结构化报告不被 record 干扰（零 summary JSON 残留、零
/// `.err` 残留）。
#[test]
fn 重复装配各自log互不覆写且零summary残留() {
    let dir = TempDir::new().expect("临时目录");
    let reports_dir = dir.path().join("openspec/changes/demo-change/reports/test");
    std::fs::create_dir_all(&reports_dir).expect("建报告目录");
    let structured = reports_dir.join("summary.json");
    std::fs::write(&structured, "{\"phase\":\"test-execution\"}").expect("写结构化报告占位");
    let summary = hand_summary(Conclusion::Pass, vec!["找不到问题".to_owned()]);

    let first = outcome_test(
        dir.path(),
        "demo-change",
        &summary,
        &[],
        &reports_dir,
        b"first-run-output\n",
    );
    let second = outcome_test(
        dir.path(),
        "demo-change",
        &summary,
        &[],
        &reports_dir,
        b"second-run-output\n",
    );
    let (ToolStepOutput::TestExecution(first), ToolStepOutput::TestExecution(second)) =
        (first, second)
    else {
        panic!("两跑产出均应为 TestExecution 变体");
    };

    assert_ne!(
        first.exec_id, second.exec_id,
        "每次装配新铸 exec_id（引用唯一面）"
    );
    assert_eq!(
        exec::read_exec_output(dir.path(), "demo-change", &first.exec_id).expect("首跑回查"),
        b"first-run-output\n".to_vec(),
        "首跑 .log 不被后次覆写"
    );
    assert_eq!(
        exec::read_exec_output(dir.path(), "demo-change", &second.exec_id).expect("次跑回查"),
        b"second-run-output\n".to_vec(),
        "次跑独立 .log"
    );
    // report_dir 面与 record 面隔离：结构化报告占位零改写、exec-outputs 仅 .log
    assert_eq!(
        std::fs::read_to_string(&structured).expect("结构化报告仍在"),
        "{\"phase\":\"test-execution\"}",
        "record 不干扰 report_dir 结构化报告"
    );
    let mut names: Vec<String> = std::fs::read_dir(reports_dir.join("exec-outputs"))
        .expect("读输出记录目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut expected = vec![
        format!("{}.log", first.exec_id),
        format!("{}.log", second.exec_id),
    ];
    expected.sort();
    assert_eq!(names, expected, "仅 .log 落盘（零 .err / 零 summary 残留）");
}

/// 边界 / 异常：报告目录不可写（父路径被文件占位）→ 记录降级空串（未记录），
/// 门禁结论面不受阻断（report_dir 报告仍为全量权威）。
#[test]
fn 记录失败降级空串不阻断门禁结论() {
    let dir = TempDir::new().expect("临时目录");
    let reports_dir = dir.path().join("openspec/changes/demo-change/reports/test");
    std::fs::create_dir_all(reports_dir.parent().expect("父目录")).expect("创建父目录失败");
    std::fs::write(&reports_dir, "占位文件").expect("占位写失败");

    let output = outcome_test(
        dir.path(),
        "demo-change",
        &hand_summary(Conclusion::Fail, vec!["用例失败".to_owned()]),
        &[],
        &reports_dir,
        b"suite output bytes\n",
    );
    let ToolStepOutput::TestExecution(mapped) = output else {
        panic!("产出应为 TestExecution 变体");
    };
    assert_eq!(
        mapped.conclusion,
        TestExecutionConclusion::Fail,
        "记录失败不阻断门禁结论"
    );
    assert!(mapped.exec_id.is_empty(), "落盘失败降级空串（未记录口径）");
    assert_eq!(
        mapped.report_dir,
        to_posix(&reports_dir),
        "report_dir 面不受记录失败影响"
    );
}

// ---------------------------------------------------------------------------
// slot 槽位（check-before-flow-start / AC-1 / AC-3）：报告树按槽位分树隔离、
// 复用门树内 / 跨树语义、三态载荷 slot 透传（Mock策略：真件 shim + PATH 窗口
// 沿既有装置——spawn 为被测本体不 mock；mtime 经文件系统直驱）
// ---------------------------------------------------------------------------

/// slot=Baseline 正向：报告落 baseline 树——`summary.json` + 逐 suite 子报告 +
/// `exec-outputs/<exec_id>.log` 落 `reports/baseline`（与 `reports/test` 零混居
/// ——目录树逐文件断言，AC-1）。
#[tokio::test]
async fn run_slot_baseline_报告落baseline树与test树零混居() {
    let ws = ChainWs::new("baseline-tree");
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");

    let output = {
        let _window = path_window();
        set_shim_env(
            &spec_src,
            &ws.results_dest_slot(ReportSlot::Baseline),
            Some(&ws.root().join("shim-marker.log")),
            None,
        );
        ws.run_slot(ReportSlot::Baseline).await
    }
    .expect("baseline run 应 Ok");

    let outcome = match output {
        ToolStepOutput::TestExecution(outcome) => outcome,
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    };
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Pass,
        "同 fixture 结论与槽位无关（SHIM_DEST 同锚 baseline 树）"
    );
    assert!(
        outcome.report_dir.ends_with("reports/baseline")
            || !outcome
                .report_dir
                .ends_with(&format!("reports{}", std::path::MAIN_SEPARATOR))
                && outcome.report_dir.contains("reports/baseline"),
        "载荷 report_dir = baseline 树锚: {}",
        outcome.report_dir
    );

    // baseline 树逐文件：summary.json + 逐 suite 子报告 + exec-outputs log
    let baseline = ws.reports_dir_slot(ReportSlot::Baseline);
    assert!(
        baseline.join("summary.json").is_file(),
        "summary.json 落 baseline 树"
    );
    assert!(
        baseline.join("app_node-test").join("report.json").is_file(),
        "planId 子报告落 baseline 树"
    );
    assert!(
        !outcome.exec_id.is_empty(),
        "exec_id 回填（record 亦按槽位同锚）"
    );
    let baseline_log = baseline
        .join("exec-outputs")
        .join(format!("{}.log", outcome.exec_id));
    assert!(baseline_log.is_file(), "输出记录落 baseline exec-outputs");

    // test 树零混居：同 run 零 reports/test 文件
    let test_tree = ws.reports_dir_slot(ReportSlot::Test);
    assert!(
        !test_tree.exists()
            || fs::read_dir(&test_tree)
                .expect("读 test 树")
                .next()
                .is_none(),
        "reports/test 树零混居（分树隔离）"
    );
}

/// run slot=Test 缺省槽位回归（正向）：`reports/test` 路径与本变更前逐字节一致
///（既有 reports_dir 断言面持衡——绿跑全链用例同 fixture 双验）。
#[tokio::test]
async fn run_slot_test_缺省槽位reports_test路径持衡() {
    let ws = ChainWs::new("test-tree");
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
    .expect("test run 应 Ok");

    let outcome = match output {
        ToolStepOutput::TestExecution(outcome) => outcome,
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    };
    assert_eq!(
        outcome.report_dir,
        to_posix(&change_test_reports(ws.root(), CHANGE)),
        "slot=Test 报告树与既有 change_test_reports 推导逐字节一致"
    );
    assert!(
        change_test_reports(ws.root(), CHANGE)
            .join("summary.json")
            .is_file(),
        "summary.json 落 reports/test（既有路径）"
    );
}

/// 复用门跨树隔离（异常）：`reports/test` 树存在可复用 summary 时 slot=Baseline
/// run 不复用（跨树零复用——reports_dir 槽位隔离天然成立）；反向同构：
/// baseline 树 summary 不被 slot=Test 相位步复用。
#[tokio::test]
async fn 复用门跨树零复用_test树summary不被baseline相位步复用() {
    // 正向半边：test 树预置可复用 summary → baseline run 不复用（重 spawn 留新
    // marker 记录）
    let ws = ChainWs::new("reuse-cross-tree");
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");

    // 预置 test 树可复用 summary（本变更前形态的落点——mtime 未变）
    let test_tree = change_test_reports(ws.root(), CHANGE);
    fs::create_dir_all(&test_tree).expect("建 test 树");
    fs::write(
        test_tree.join("summary.json"),
        r#"{ "conclusion": "pass", "total": 5, "passed": 4, "failed": 0, "skipped": 1, "coverage": null, "plans": [] }"#,
    )
    .expect("预置 test 树 summary");

    let marker = ws.root().join("shim-marker.log");
    let output = {
        let _window = path_window();
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        ws.run_slot(ReportSlot::Baseline).await
    }
    .expect("baseline run 应 Ok");
    let baseline_outcome = match output {
        ToolStepOutput::TestExecution(outcome) => outcome,
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    };
    assert!(
        marker.is_file(),
        "baseline run 真实 spawn（test 树 summary 未被跨树复用）"
    );
    assert!(
        baseline_outcome.report_dir.contains("reports/baseline"),
        "载荷锚 baseline 树: {}",
        baseline_outcome.report_dir
    );
    assert!(
        ws.reports_dir_slot(ReportSlot::Baseline)
            .join("summary.json")
            .is_file(),
        "baseline 树自产 summary（跨树零复用）"
    );

    // 反向半边：baseline 树已有 summary → slot=Test run 不复用（各跑各树）
    let ws2 = ChainWs::new("reuse-cross-tree-rev");
    let spec_src2 = ws2.root().join("preset-results.txt");
    fs::write(&spec_src2, SPEC_OUTPUT).expect("写预置工件");
    let baseline_tree = ws2.reports_dir_slot(ReportSlot::Baseline);
    fs::create_dir_all(&baseline_tree).expect("建 baseline 树");
    fs::write(
        baseline_tree.join("summary.json"),
        r#"{ "conclusion": "pass", "total": 5, "passed": 4, "failed": 0, "skipped": 1, "coverage": null, "plans": [] }"#,
    )
    .expect("预置 baseline 树 summary");

    let marker2 = ws2.root().join("shim-marker.log");
    let output2 = {
        let _window = path_window();
        set_shim_env(&spec_src2, &ws2.results_dest(), Some(&marker2), None);
        ws2.run().await
    }
    .expect("test run 应 Ok");
    let test_outcome = match output2 {
        ToolStepOutput::TestExecution(outcome) => outcome,
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    };
    assert!(
        marker2.is_file(),
        "slot=Test run 真实 spawn（baseline 树 summary 未被跨树复用）"
    );
    assert!(
        test_outcome.report_dir.contains("reports/test"),
        "载荷锚 test 树: {}",
        test_outcome.report_dir
    );
}

/// run 三态载荷 slot 透传（异常 / AC-3 半边）：`conclusion=Error`（shim 环境坏
/// ——SHIM_EXIT 非 0 且工件不可解析）→ outcome conclusion=Error + report_dir
/// baseline 锚；`conclusion=Fail` → 测试红 + baseline 锚（walker 分文案消费的
/// 载荷供给面——字段级断言）。
#[tokio::test]
async fn run三态载荷_slot透传_baseline锚与fail载荷() {
    // Fail 态：spec 计数行携失败 → conclusion=Fail + baseline 锚
    let ws = ChainWs::new("baseline-fail");
    let fail_spec = SPEC_OUTPUT.replacen("✔", "✖", 1);
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, fail_spec).expect("写失败 spec 工件");

    let output = {
        let _window = path_window();
        set_shim_env(
            &spec_src,
            &ws.results_dest_slot(ReportSlot::Baseline),
            Some(&ws.root().join("shim-marker.log")),
            None,
        );
        ws.run_slot(ReportSlot::Baseline).await
    }
    .expect("fail run 应 Ok");
    let outcome = match output {
        ToolStepOutput::TestExecution(outcome) => outcome,
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    };
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Fail,
        "spec 失败面 → Fail 结论"
    );
    assert!(
        outcome.failed >= 1,
        "失败计数在案（walker 测试红文案的计数供给面）"
    );
    assert!(
        outcome.report_dir.contains("reports/baseline"),
        "Fail 态 report_dir = baseline 锚: {}",
        outcome.report_dir
    );
    let baseline = ws.reports_dir_slot(ReportSlot::Baseline);
    assert!(
        baseline.join("summary.json").is_file(),
        "Fail 态 summary.json 落 baseline 树"
    );

    // Error 态：工件不可解析（坏输出 + 非零退出）→ conclusion=Error + baseline 锚
    let ws_err = ChainWs::new("baseline-error");
    let bad_src = ws_err.root().join("bad-results.txt");
    fs::write(&bad_src, "完全不可解析的输出").expect("写坏工件");
    let output = {
        let _window = path_window();
        set_shim_env(&bad_src, &ws_err.results_dest(), None, Some(3));
        ws_err.run_slot(ReportSlot::Baseline).await
    }
    .expect("error run 应 Ok");
    let outcome = match output {
        ToolStepOutput::TestExecution(outcome) => outcome,
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    };
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Error,
        "不可解析 → Error 结论"
    );
    assert!(
        outcome.report_dir.contains("reports/baseline"),
        "Error 态 report_dir = baseline 锚: {}",
        outcome.report_dir
    );
}

// ---------------------------------------------------------------------------
// 取消贯通（test-design「testexec/mod.rs -> mod_test.rs」节，AC-4 / AC-6 /
// AC-7）：`ProcessTestExecution::run` 的 `cancel` 形参透传 `execute` → 逐 plan
// `execute_plan(plan, &dir, cancel.clone())`（循环内 clone 共享同一原子）→
// `exec::run_shell` 入口短路 / 轮询置位两径收敛为 `fault = Some(Stopped)` 的
// 正常产出（**不 Err**），聚合层据 fault 早停、不写 summary。
// ---------------------------------------------------------------------------

/// shim 落盘的 pid（winpid 口径——`tasklist` / `kill -0` 可探测）。
fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse().ok())
}

/// 等 shim 落 pid（真实时钟握手：虚拟时钟下进程启动不受影响，等待以真实睡眠
/// + 让出表达）。
async fn wait_pid(path: &Path, budget: std::time::Duration) -> u32 {
    let deadline = std::time::Instant::now() + budget;
    loop {
        if let Some(pid) = read_pid(path) {
            return pid;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "等待自旋 shim 落 pid 超时（{budget:?}）"
        );
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// pid 存活探测（Windows `tasklist`、unix `kill -0`）。
#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output();
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// 等待 pid 消失（杀树是尽力语义 + 平台回收时延，轮询到上界即判定仍存活）。
fn wait_pid_gone(pid: u32, budget: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + budget;
    while std::time::Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    !pid_alive(pid)
}

/// 多 plan 装置：双 suite workspace + 预置工件源 + 第 2 个执行臂起自旋（落
/// pid）——返回（workspace、marker 路径、pid 落位路径）。
fn two_plan_ws(tag: &str) -> (ChainWs, PathBuf, PathBuf) {
    let ws = ChainWs::new(tag).two_plan();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    let pid_file = ws.root().join("plan-pid.txt");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    // 第 1 个执行臂正常退出（marker 落 1 行）→ 第 2 个执行臂自旋挂起
    std::env::set_var("SHIM_SPIN_AFTER", "1");
    std::env::set_var("SHIM_PIDFILE", &pid_file);
    (ws, marker, pid_file)
}

/// 正向（AC-7 / AC-5）：已置位 cancel 的两形态——① 无既有 summary：首个 plan
/// 即以 `fault = Some(Stopped)` 收口（**不 Err**）、`conclusion = Fail` 占位、
/// 四项计数 0、诊断双双空串、零 spawn（marker 零增长）、不写 summary；② 既有
/// summary 新鲜且输入未变：复用门先于 spawn 判定 → 用例照旧 `Ok`（取消不影响
/// 零 spawn 复用语义）。
#[tokio::test]
async fn 已置位cancel_首个plan即stopped且复用门先于spawn判定() {
    // 形态一：无既有 summary → 首个 plan 的 execute_plan 即取消（零 spawn）
    {
        let _window = path_window();
        let ws = ChainWs::new("cancel-pre-fresh");
        let spec_src = ws.root().join("preset-results.txt");
        fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
        let marker = ws.root().join("shim-marker.log");
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        let cancel = foundation::cancel::CancelFlag::never();
        cancel.handle().set();

        let output = ws
            .run_with(cancel)
            .await
            .expect("已置位 cancel 以 fault 收口（不 Err）");
        let ToolStepOutput::TestExecution(outcome) = output else {
            panic!("产出应为 TestExecution 变体");
        };
        assert_eq!(
            outcome.fault,
            Some(RunFault::Stopped),
            "取消 → fault = Stopped（run 收敛 stopped，非 failed）"
        );
        assert_eq!(
            outcome.conclusion,
            TestExecutionConclusion::Fail,
            "fault 面结论取 Fail 占位（不取 Error / Pass）"
        );
        assert_eq!(
            (
                outcome.total,
                outcome.passed,
                outcome.failed,
                outcome.skipped
            ),
            (0, 0, 0, 0),
            "fault 面四项计数全 0（不装配半套计数）"
        );
        assert!(
            outcome.findings_brief.is_empty() && outcome.findings_detail.is_empty(),
            "fault 面诊断双双空串"
        );
        assert!(!outcome.exec_id.is_empty(), "exec_id 照旧回填");
        assert_eq!(
            ws.marker_count(),
            0,
            "首个 plan 入口短路零 spawn（marker 零增长）"
        );
        assert!(
            !ws.reports_dir().join("summary.json").exists(),
            "取消臂不落 summary（半套报告不进复用门）"
        );
    }

    // 形态二：既有新鲜 summary + 输入未变 → 复用分支在 spawn 之前返回 Ok
    {
        let _window = path_window();
        let ws = ChainWs::new("cancel-pre-reuse");
        let spec_src = ws.root().join("preset-results.txt");
        fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
        let marker = ws.root().join("shim-marker.log");
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        ws.run().await.expect("首跑应 Ok");
        let first_ts = ws.summary_timestamp().expect("首跑落 summary");
        assert_eq!(ws.marker_count(), 1, "首跑恰一次执行");

        let cancel = foundation::cancel::CancelFlag::never();
        cancel.handle().set();
        let output = ws
            .run_with(cancel)
            .await
            .expect("复用门先于 spawn 判定 → 已置位 cancel 仍走复用分支");
        let outcome = match output {
            ToolStepOutput::TestExecution(outcome) => outcome,
            other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
        };
        assert_eq!(
            outcome.conclusion,
            TestExecutionConclusion::Pass,
            "复用上次结论（取消未触达 spawn 面）"
        );
        assert_eq!(ws.marker_count(), 1, "零 spawn（marker 计数不增长）");
        assert_eq!(
            ws.summary_timestamp(),
            Some(first_ts),
            "summary 未重写（复用分支）"
        );
    }
}

/// 正向（AC-7 / D14）：多 plan 中途取消——第 1 个 plan 完成后置位取消，第 2 个
/// plan 观测取消 → 整体 `fault = Some(Stopped)`（**不 Err**）、`conclusion =
/// Fail` 占位、零 summary 落盘、第 2 个 plan 进程被回收。
#[tokio::test]
async fn 多plan中途取消_第二plan取消且整体stopped不落summary() {
    let _window = path_window();
    let (ws, marker, pid_file) = two_plan_ws("cancel-multi-plan");
    let cancel = foundation::cancel::CancelFlag::never();
    let handle = cancel.handle();
    let started = std::time::Instant::now();
    ws.prepend_path();
    let ws_root = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessTestExecution::new()
            .run(&ws_root, CHANGE, ReportSlot::Test, cancel)
            .await
    });

    // 第 2 个 plan 落 pid（进程已建立且自旋）后置位——此时第 1 个 plan 已完成
    let pid = wait_pid(&pid_file, std::time::Duration::from_secs(60)).await;
    handle.set();

    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("取消以 fault 收口（不 Err）");
    let elapsed = started.elapsed();
    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "第 2 个 plan 取消 → fault = Stopped、`break` 早停"
    );
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Fail,
        "fault 面结论占位 Fail"
    );
    assert_eq!(
        fs::read_to_string(&marker)
            .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count())
            .unwrap_or(0),
        1,
        "第 1 个 plan 执行完成（marker 1 行）；第 2 个 plan 停在自旋臂（未走到 marker 追加）"
    );
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "取消臂不落 summary（不吞成部分结论）"
    );
    assert!(
        wait_pid_gone(pid, std::time::Duration::from_secs(15)),
        "第 2 个 plan 进程被回收（pid {pid} 消失——run_shell 杀树生效）"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "秒级收敛（实际 {elapsed:?}）"
    );
}

/// 边界（AC-6 逐层下传锚）：**循环内 clone 共享同一原子**——`run → execute →
/// 逐 plan execute_plan(plan, &dir, cancel.clone())` 三层形参皆同一原子；第 1
/// 个 plan 完成后置位，第 2 个 plan 的 `exec::run_shell` 观测到置位并杀树。若
/// 任一层退化为重新铸造（`never()`），第 2 个 plan 将不受置位影响（自旋进程
/// 存活、用例挂死）——故 pid 消失 + 秒级收敛即「同一原子跨 plan 可见」的行为面
/// 证据。
#[tokio::test]
async fn 循环内clone共享原子_置位在第二plan链尾可见() {
    let _window = path_window();
    let (ws, _marker, pid_file) = two_plan_ws("cancel-clone-identity");
    let cancel = foundation::cancel::CancelFlag::never();
    let probe = cancel.clone();
    let handle = cancel.handle();
    let started = std::time::Instant::now();
    ws.prepend_path();
    let ws_root = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessTestExecution::new()
            .run(&ws_root, CHANGE, ReportSlot::Test, cancel)
            .await
    });

    let pid = wait_pid(&pid_file, std::time::Duration::from_secs(60)).await;
    handle.set();

    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("取消以 fault 收口（不 Err）");
    let elapsed = started.elapsed();
    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "链尾执行体观测到置位 → fault = Stopped"
    );
    assert!(probe.is_set(), "置位对外部 clone 可见（同一原子）");
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "链尾执行体观测到置位（秒级收敛，实际 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, std::time::Duration::from_secs(15)),
        "第 2 个 plan 的进程被回收（pid {pid} 消失——任一层重新铸造则不会消失）"
    );
}

/// 正向（AC-5 / D14）：多 plan 的 `Environment` **继续**跑余下 plan——两 plan 均
/// 以锁文本 + 非零退出收场（`transient()` 各自 5 次耗尽）→ 两个 plan 都被拉起
/// （marker 2 行，`break` 不发生）、最终 `fault == Some(Environment(..))`（首个）、
/// `conclusion == Fail` 占位、四项计数 0、不写 summary。
#[tokio::test(start_paused = true)]
async fn 多plan环境错误继续跑余下plan() {
    let _gate = ClockGate::new();
    let _window = path_window();
    let ws = ChainWs::new("fault-env-multi-plan").two_plan();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    // 前 5 次 spawn（第 1 个 plan 的 5 次尝试）输出锁文本并非零退出 → 第 1 个 plan
    // 瞬态耗尽转 fault；第 6 次 spawn（第 2 个 plan）走绿跑面（拷贝工件 + 零退出）
    std::env::set_var(
        "SHIM_LOCK",
        "error: failed to rename archive file: /t/liba.rlib: os error 32",
    );
    std::env::set_var("SHIM_LOCK_FIRST", "5");
    let lock_count = ws.root().join("shim-lock-count.log");
    std::env::set_var("SHIM_LOCK_COUNT", &lock_count);
    ws.prepend_path();
    let ws_root = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessTestExecution::new()
            .run(
                &ws_root,
                CHANGE,
                ReportSlot::Test,
                foundation::cancel::CancelFlag::never(),
            )
            .await
    });
    advance_until(
        || runner.is_finished(),
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(120),
    )
    .await;
    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("环境错误以 fault 收口（不 Err）");

    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    let marker_rows = fs::read_to_string(&marker)
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0);
    // 瞬态锁定面 spawn 记数：第 1 个 plan 5 次尝试（耗尽）；绿跑面 marker 1 行
    //（第 2 个 plan 在 Environment 之后仍被拉起 → `Environment` 不 break）
    let locked_spawns = fs::read_to_string(&lock_count)
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0);
    assert_eq!(locked_spawns, 5, "第 1 个 plan 恰 5 次尝试（瞬态耗尽）");
    assert_eq!(
        marker_rows, 1,
        "第 2 个 plan 在 Environment 之后仍被拉起（不 break）"
    );

    let Some(RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!(
            "两个 plan 的环境错误应收口为 Environment fault: {:?}",
            outcome.fault
        );
    };
    assert!(
        text.contains("已自动重试 4 次"),
        "归因文本携重试次数: {text}"
    );
    assert!(
        text.contains("workspace 父目录") && text.contains("逐 worktree"),
        "环境侧指引在案: {text}"
    );
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Fail,
        "fault 面结论占位 Fail"
    );
    assert_eq!(
        (
            outcome.total,
            outcome.passed,
            outcome.failed,
            outcome.skipped
        ),
        (0, 0, 0, 0),
        "fault 在位不装配半套计数"
    );
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "fault 面不写 summary（半套报告不进复用门）"
    );
    assert!(!outcome.exec_id.is_empty(), "exec_id 照旧回填");
}

// ---------------------------------------------------------------------------
// 单 plan 环境错误上传 / 聚合优先级 / 三 plan 混合（test-design
// 「testexec/mod.rs -> mod_test.rs」用例表：AC-5 组合锚 + D12 / D13 / D14）
// ---------------------------------------------------------------------------

/// 虚拟时钟闸门：以常驻 `spawn_blocking` 任务抑制 tokio 的「空闲即自动推进」
/// （沿 `runner_test` / `static_check_test` 同型装置）。暂停时钟下等待真实子进程
/// 会让虚拟时间成百倍空转；闸门在位时虚拟时间**只**随 [`advance_until`] 前进
/// ——重试慢档（5 / 10 / 20 / 40 s，两轮 150 s）因此不真等。
struct ClockGate {
    release: Option<std::sync::mpsc::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl ClockGate {
    fn new() -> Self {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let task = tokio::task::spawn_blocking(move || {
            let _ = wait.recv();
        });
        Self {
            release: Some(release),
            task: Some(task),
        }
    }
}

impl Drop for ClockGate {
    fn drop(&mut self) {
        self.release.take();
        self.task.take();
    }
}

/// 推进虚拟时钟直到 `until` 为真（真实时间上界 `real_budget`）：每步 `step`
/// 虚拟 + 10 ms 真实让出（子进程排水 / 收尸在真实时间上进行）。
async fn advance_until<F: Fn() -> bool>(
    until: F,
    step: std::time::Duration,
    real_budget: std::time::Duration,
) {
    let deadline = std::time::Instant::now() + real_budget;
    while !until() && std::time::Instant::now() < deadline {
        tokio::time::advance(step).await;
        let _ = tokio::task::spawn_blocking(|| {
            std::thread::sleep(std::time::Duration::from_millis(10))
        })
        .await;
    }
    assert!(
        until(),
        "推进真实预算 {real_budget:?} 后仍未收敛（暂停时钟下虚拟时间不会自行前进）"
    );
}

/// 记账文件行数（shim 逐次追加——spawn 次数观测面）。
fn line_count(path: &Path) -> usize {
    fs::read_to_string(path)
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0)
}

/// 正向（AC-5 组合锚）：**单 plan 的 `Environment` → fault 上传**——单 suite 恒
/// 锁文本（`transient()` 真 5 次耗尽，全链 `execute_plan`）→
/// `TestExecutionOutcome.fault == Some(Environment(text))`（文本含「已自动重试
/// 4 次」与被锁文件原料）、`conclusion == Fail` 占位、四项计数全 0、`findings_*`
/// 空串、`exec_id` 非空、`report_dir` 为本槽位锚点、不写 summary。
#[tokio::test(start_paused = true)]
async fn 单plan环境错误_fault上传与占位装配() {
    let _gate = ClockGate::new();
    let _window = path_window();
    let ws = ChainWs::new("fault-env-single-plan");
    let unused_src = ws.root().join("unused-results.txt");
    let lock_count = ws.root().join("shim-lock-count.log");
    set_shim_env(&unused_src, &ws.results_dest(), None, None);
    std::env::set_var(
        "SHIM_LOCK",
        "error: failed to rename archive file: /t/liba.rlib: os error 32",
    );
    std::env::set_var("SHIM_LOCK_FIRST", "9");
    std::env::set_var("SHIM_LOCK_COUNT", &lock_count);
    ws.prepend_path();
    let ws_root = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessTestExecution::new()
            .run(
                &ws_root,
                CHANGE,
                ReportSlot::Test,
                foundation::cancel::CancelFlag::never(),
            )
            .await
    });
    advance_until(
        || runner.is_finished(),
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(120),
    )
    .await;
    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("环境错误以 fault 收口（不 Err）");

    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    let Some(RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!(
            "单 plan 环境错误应收口为 Environment fault: {:?}",
            outcome.fault
        );
    };
    assert!(
        text.contains("已自动重试 4 次"),
        "归因文本携重试次数（attempts - 1 = 4）: {text}"
    );
    assert!(
        text.contains("liba.rlib"),
        "被锁文件原料在案（lockprobe 解析命中）: {text}"
    );
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Fail,
        "fault 面结论取 Fail 占位（不取 Error / Pass）"
    );
    assert_eq!(
        (
            outcome.total,
            outcome.passed,
            outcome.failed,
            outcome.skipped
        ),
        (0, 0, 0, 0),
        "fault 面四项计数全 0（不装配半套计数）"
    );
    assert!(
        outcome.findings_brief.is_empty() && outcome.findings_detail.is_empty(),
        "fault 面诊断双双空串"
    );
    assert!(!outcome.exec_id.is_empty(), "exec_id 照旧回填");
    assert_eq!(
        outcome.report_dir,
        to_posix(&ws.reports_dir()),
        "report_dir 为本槽位锚点（不因 fault 面漂移）"
    );
    assert_eq!(
        line_count(&lock_count),
        5,
        "单 plan 恰 5 次尝试（瞬态策略上限，不再增长）"
    );
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "fault 面不写 summary（半套报告不进复用门）"
    );
}

/// 正向（AC-5 / D13）：**不写 summary → 复用门未命中而重跑**——同输入连续两次
/// `run`（每跑 5 次瞬态耗尽）：两次均 `fault = Some(Environment(..))`、
/// `<report_dir>/summary.json` 恒不存在（坏 plan 不被当权威永久复用）、第二次
/// 仍真 spawn（记账 5 → 10）。
#[tokio::test(start_paused = true)]
async fn 单plan环境错误_不写summary且重跑不复用() {
    let _gate = ClockGate::new();
    let _window = path_window();
    let ws = ChainWs::new("fault-env-no-summary");
    let unused_src = ws.root().join("unused-results.txt");
    let lock_count = ws.root().join("shim-lock-count.log");
    set_shim_env(&unused_src, &ws.results_dest(), None, None);
    std::env::set_var(
        "SHIM_LOCK",
        "error: failed to rename archive file: /t/liba.rlib: os error 32",
    );
    std::env::set_var("SHIM_LOCK_FIRST", "99");
    std::env::set_var("SHIM_LOCK_COUNT", &lock_count);
    ws.prepend_path();

    for round in 1..=2usize {
        let ws_root = ws.root_str();
        let runner = tokio::spawn(async move {
            ProcessTestExecution::new()
                .run(
                    &ws_root,
                    CHANGE,
                    ReportSlot::Test,
                    foundation::cancel::CancelFlag::never(),
                )
                .await
        });
        advance_until(
            || runner.is_finished(),
            std::time::Duration::from_millis(100),
            std::time::Duration::from_secs(120),
        )
        .await;
        let output = runner
            .await
            .expect("run 任务应正常结束")
            .expect("环境错误以 fault 收口（不 Err）");

        let ToolStepOutput::TestExecution(outcome) = output else {
            panic!("产出应为 TestExecution 变体");
        };
        assert!(
            matches!(outcome.fault, Some(RunFault::Environment(_))),
            "第 {round} 跑仍收口为 Environment fault: {:?}",
            outcome.fault
        );
        assert!(
            !ws.reports_dir().join("summary.json").exists(),
            "第 {round} 跑不写 summary（无 summary 可复用）"
        );
        assert_eq!(
            line_count(&lock_count),
            round * 5,
            "第 {round} 跑真 spawn（复用门未命中 → 重跑坏 plan，不永久复用）"
        );
    }
}

/// 边界（D14）：**`Stopped` 优先于 `Environment`**——两 plan：plan1 恒锁耗尽收
/// `Environment`、plan2 自旋后置位取消收 `Stopped` → 最终 `fault == Some(Stopped)`
/// （优先级规则可观测，非「首个胜出」）、`conclusion` 占位 `Fail`、不写 summary、
/// plan2 进程被回收。
#[tokio::test(start_paused = true)]
async fn stopped优先于environment_环境错误后取消收口stopped() {
    let _gate = ClockGate::new();
    let _window = path_window();
    let ws = ChainWs::new("fault-stopped-priority").two_plan();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    let pid_file = ws.root().join("plan-pid.txt");
    let lock_count = ws.root().join("shim-lock-count.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    // plan1（app）：前 5 次 spawn 恒锁文本（瞬态耗尽 → Environment）；
    // plan2（app2）：锁计数已满 → 走自旋臂（落 pid 后挂起，等取消）
    std::env::set_var(
        "SHIM_LOCK",
        "error: failed to rename archive file: /t/liba.rlib: os error 32",
    );
    std::env::set_var("SHIM_LOCK_FIRST", "5");
    std::env::set_var("SHIM_LOCK_COUNT", &lock_count);
    std::env::set_var("SHIM_SPIN_AFTER", "0");
    std::env::set_var("SHIM_PIDFILE", &pid_file);

    let cancel = foundation::cancel::CancelFlag::never();
    let handle = cancel.handle();
    let started = std::time::Instant::now();
    ws.prepend_path();
    let ws_root = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessTestExecution::new()
            .run(&ws_root, CHANGE, ReportSlot::Test, cancel)
            .await
    });

    advance_until(
        || pid_file.exists(),
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(120),
    )
    .await;
    let pid = read_pid(&pid_file).expect("自旋 shim 应落 pid");
    assert_eq!(line_count(&lock_count), 5, "plan1 已瞬态耗尽（5 次尝试）");
    handle.set();
    // 取消观测是 200 ms 轮询（虚拟时钟）：置位后继续推进，使轮询就绪
    advance_until(
        || runner.is_finished(),
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(120),
    )
    .await;

    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("取消以 fault 收口（不 Err）");
    let elapsed = started.elapsed();
    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "`Stopped` 优先于首个 `Environment`（聚合规则非「首个胜出」）"
    );
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Fail,
        "fault 面结论占位 Fail"
    );
    assert_eq!(
        (
            outcome.total,
            outcome.passed,
            outcome.failed,
            outcome.skipped
        ),
        (0, 0, 0, 0),
        "fault 面四项计数全 0"
    );
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "取消臂不写 summary"
    );
    assert!(
        wait_pid_gone(pid, std::time::Duration::from_secs(15)),
        "第 2 个 plan 进程被回收（pid {pid} 消失）"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(120),
        "秒级收敛（plan1 的 5 次重试按真实时钟，实际 {elapsed:?}）"
    );
}

/// 边界（D14）：**三 plan 混合面**——plan1（node-test）绿（`sub_report` 入列）、
/// plan2（jest）恒锁耗尽收 `Environment`、plan3（node-test）绿 → `Environment`
/// 不 break（两枚绿 plan 的执行臂记账均在案）、最终 `fault == Some(Environment)`
/// （首个）、`conclusion == Fail` 占位、四项计数 0（`sub_reports` 内部装配不泄漏
/// 到 outcome）、不写 summary / 不落 plan 报告文件。
#[tokio::test(start_paused = true)]
async fn 三plan混合_环境错误在中间继续跑完且计数不泄漏() {
    let _gate = ClockGate::new();
    let _window = path_window();
    let ws = ChainWs::new("fault-env-three-plan").three_plan_mixed();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    // 一次执行臂双投：plan1（app）与 plan3（app3）各得一枚同源工件
    set_shim_dest_2(&ws.results_dest_third());
    ws.prepend_path();
    let ws_root = ws.root_str();
    let runner = tokio::spawn(async move {
        ProcessTestExecution::new()
            .run(
                &ws_root,
                CHANGE,
                ReportSlot::Test,
                foundation::cancel::CancelFlag::never(),
            )
            .await
    });
    advance_until(
        || runner.is_finished(),
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(120),
    )
    .await;
    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("环境错误以 fault 收口（不 Err）");

    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    assert_eq!(
        ws.marker_count(),
        2,
        "plan1 与 plan3 均被拉起（Environment 之后仍继续跑完余下 plan）"
    );
    assert_eq!(
        line_count(&ws.npx_attempt_log()),
        5,
        "plan2 恰 5 次尝试（jest 档瞬态耗尽；版本探测臂不计账）"
    );

    let Some(RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!("混合面应收口为 Environment fault: {:?}", outcome.fault);
    };
    assert!(
        text.contains("已自动重试 4 次"),
        "首个 Environment 的归因文本上传: {text}"
    );
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Fail,
        "fault 面结论占位 Fail"
    );
    assert_eq!(
        (
            outcome.total,
            outcome.passed,
            outcome.failed,
            outcome.skipped
        ),
        (0, 0, 0, 0),
        "sub_reports 内部装配不泄漏到 outcome（计数仍 0）"
    );
    assert!(
        outcome.findings_brief.is_empty() && outcome.findings_detail.is_empty(),
        "fault 面诊断双双空串"
    );
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "fault 面不写 summary"
    );
    assert!(
        !ws.reports_dir()
            .join("app_node-test")
            .join("report.json")
            .exists(),
        "fault 面不落 plan 报告文件（半套报告不进复用门）"
    );
}

/// 边界（D12）：**`conclusion` 占位不参与门禁**——fault 面直构
/// `TestExecutionOutcome { conclusion: Pass, fault: Some(Environment(..)) }` →
/// 载荷原样携带（本层不做「fault + Pass」纠偏，判定归 `walker.rs` 的 fault 前置
/// 分派）；`conclusion` 三值面零扩展（`as_str` 仍只 `pass` / `fail` / `error`）。
#[test]
fn conclusion占位不参与门禁_直构fault不纠偏() {
    let fault = RunFault::Environment("占用进程：HipsDaemon.exe (pid 1234)".to_owned());
    let outcome = TestExecutionOutcome {
        conclusion: TestExecutionConclusion::Pass,
        total: 3,
        passed: 3,
        failed: 0,
        skipped: 0,
        findings_brief: "全部测试通过且覆盖率达阈值".to_owned(),
        findings_detail: String::new(),
        report_dir: "C:/ws/reports/test".to_owned(),
        exec_id: "exec-direct".to_owned(),
        fault: Some(fault.clone()),
    };
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Pass,
        "本层不纠偏「fault + Pass」（原样携带）"
    );
    assert_eq!(outcome.fault, Some(fault), "fault 载荷逐字携带");
    assert_eq!(
        (
            TestExecutionConclusion::Pass.as_str(),
            TestExecutionConclusion::Fail.as_str(),
            TestExecutionConclusion::Error.as_str()
        ),
        ("pass", "fail", "error"),
        "conclusion 三值面零扩展（无新变体 / 无 fault 专属值）"
    );
}

// ---------------------------------------------------------------------------
// 跨 plan 总预算（AC-6 / D9–D10：`execute(.., total_budget)` 第五参注入）
// ---------------------------------------------------------------------------

/// 正向（AC-6）：`TESTEXEC_TOTAL_BUDGET` 常量锚定——`2400 s`（改值即被击杀），
/// 且大于单 plan 单次尝试预算（1200 s）两倍（留余量）。生产唯一调用方
/// `ProcessTestExecution::run` 传该常量（由 `绿跑全链` 等全链用例表达）。
#[test]
fn 测试执行总预算常量锚定() {
    let budget = TESTEXEC_TOTAL_BUDGET;
    assert_eq!(
        budget,
        std::time::Duration::from_secs(2400),
        "跨 plan 总预算 2400 s"
    );
    assert!(
        budget >= std::time::Duration::from_secs(1200) * 2,
        "预算不小于单 plan 单次尝试预算的两倍（实际 {budget:?}）"
    );
}

/// 正向（AC-6 组合锚）：超预算余下 plan 零 spawn——两 plan 装置 + 3 s 预算
/// （首个 plan 的真实 spawn 耗时 6 s ≫ 预算，故「首边界不触发 / 第二边界必越」
/// 确定成立）→ marker 仅 1 行（余下 plan **零 spawn**）、`fault =
/// Some(Environment(text))` 且 `text` 含「测试执行总时长超预算」与「余下 1 个
/// suite 未执行」、`conclusion == Fail` 占位、四计数 0、诊断双双空串、
/// `report_dir` / `exec_id` 回填、`reports/test` 无 `summary.json`。
///
/// **预算档位选值口径**：预算起点在 detect **之前**（见 `execute` 文档），故
/// 「首边界不触发」要求 `预算 > detect 耗时`。detect 逐 suite 各 spawn 一次版本
/// 探测，冷启成本受机器负载支配（.cmd 短接后数十毫秒级，此前经 PowerShell 时
/// 高负载实测 0.7–2.5 s）——本用例取 3 s 预算，为首边界留出两个数量级余量，
/// MUST NOT 回退到 1 s 档（门禁并发下必 flake）。
#[tokio::test]
async fn 超预算余下plan零spawn() {
    let _window = path_window();
    let ws = ChainWs::new("budget-overrun").two_plan();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    set_shim_dest_2(&ws.reports_dir().join("app2_node-test").join("results.txt"));
    // 首个 plan 的真实 spawn 耗时 ≫ 预算（6 s vs 3 s）——不依赖时序抖动
    std::env::set_var("SHIM_SLEEP_MS", "6000");
    ws.prepend_path();

    let output = execute(
        &ws.root_str(),
        CHANGE,
        ReportSlot::Test,
        foundation::cancel::CancelFlag::never(),
        std::time::Duration::from_secs(3),
    )
    .await
    .expect("超预算仍 Ok（沿既有 fault 路径收口）");

    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    assert_eq!(
        ws.marker_count(),
        1,
        "余下 1 个 plan 零 spawn（marker 仅 1 行）"
    );
    let Some(RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!("超预算应产 Environment fault，实际: {:?}", outcome.fault);
    };
    assert!(
        text.contains("测试执行总时长超预算"),
        "fault 文案点名预算: {text}"
    );
    assert!(
        text.contains("余下 1 个 suite 未执行"),
        "fault 文案点名未执行数量: {text}"
    );
    assert_eq!(
        outcome.conclusion,
        TestExecutionConclusion::Fail,
        "fault 面结论取 Fail 占位"
    );
    assert_eq!(
        (
            outcome.total,
            outcome.passed,
            outcome.failed,
            outcome.skipped
        ),
        (0, 0, 0, 0),
        "fault 面四项计数全 0"
    );
    assert!(
        outcome.findings_brief.is_empty() && outcome.findings_detail.is_empty(),
        "fault 面诊断双双空串"
    );
    assert_eq!(
        outcome.report_dir,
        to_posix(&ws.reports_dir()),
        "report_dir 为本槽位锚点"
    );
    assert!(!outcome.exec_id.is_empty(), "exec_id 照旧回填");
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "预算收口不写 summary（半套报告不进复用门）"
    );

    // 已完成 plan 的工件可回查（工件不删，仅 summary 缺失）
    let first_results = ws.results_dest();
    assert!(
        first_results.exists(),
        "首个 plan 的 results.txt 仍在报告树"
    );
    assert_eq!(
        fs::read_to_string(&first_results).unwrap_or_default(),
        artifact_normalized(),
        "首个 plan 工件内容完整"
    );
    assert!(
        !ws.reports_dir()
            .join("app_node-test")
            .join("report.json")
            .exists(),
        "预算收口不落 plan 子报告"
    );
}

/// 工件文本归一（CRLF / 去尾随空白——shim 拷贝可能引入平台行尾差异）。
fn artifact_normalized() -> String {
    SPEC_OUTPUT
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// 边界：零预算边界——`execute(.., Duration::ZERO)` → 首个 plan 边界即触发
/// （生产常量下 `elapsed ≈ 0 ≪ 2400 s` 恒不触发）：全部 plan 零 spawn（marker
/// 0 行）、`text` 含「余下 N 个 suite 未执行」且 `N == plans.len()`、无 summary、
/// `exec_id` 仍回填（空合并流落空 `.log` 可回查）。
#[tokio::test]
async fn 零预算边界全部plan零spawn() {
    let _window = path_window();
    let ws = ChainWs::new("budget-zero").two_plan();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    ws.prepend_path();

    let output = execute(
        &ws.root_str(),
        CHANGE,
        ReportSlot::Test,
        foundation::cancel::CancelFlag::never(),
        std::time::Duration::ZERO,
    )
    .await
    .expect("零预算仍 Ok（fault 收口）");

    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };

    assert_eq!(ws.marker_count(), 0, "全部 plan 零 spawn");
    let Some(RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!("零预算应产 Environment fault，实际: {:?}", outcome.fault);
    };
    assert!(
        text.contains("余下 2 个 suite 未执行"),
        "N == plans.len()（2）: {text}"
    );
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "零预算不写 summary"
    );
    assert!(!outcome.exec_id.is_empty(), "exec_id 仍回填");
    let empty_log = exec::read_exec_output(ws.root(), CHANGE, &outcome.exec_id)
        .expect("空合并流落空 .log 可回查");
    assert!(empty_log.is_empty(), "零 spawn → 合并流为空");
}

/// 异常（AC-6）：预算不覆盖既有 `Stopped`——预置 cancel + 一个量级不触发首边界
/// 的预算（5 s）→ `fault == Some(Stopped)`（不是超预算文案）；入口短路零 spawn
/// （`Stopped` 面既有 break 先于后续 plan 的预算判定）。
#[tokio::test]
async fn 预算不覆盖既有stopped() {
    let _window = path_window();
    let ws = ChainWs::new("budget-stopped");
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    ws.prepend_path();

    let cancel = foundation::cancel::CancelFlag::never();
    cancel.handle().set();
    let output = execute(
        &ws.root_str(),
        CHANGE,
        ReportSlot::Test,
        cancel,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect("取消以 fault 收口（不 Err）");

    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };
    assert_eq!(
        outcome.fault,
        Some(RunFault::Stopped),
        "`Stopped` 优先于预算文案（既有 fault 不被覆盖）"
    );
    assert_eq!(ws.marker_count(), 0, "入口短路零 spawn");
}

/// 异常（AC-6 异常臂）：预算不覆盖既有 `Environment`——三 plan 混合装置（中间
/// jest 档瞬态耗尽 → `Environment(锁文案)`，`fault.is_none()` 守卫使其不被覆盖）
/// + 8 s 预算（jest 档 5 次尝试的睡眠注入 5 × 2 s ≫ 预算，第三 plan 边界必越）
/// → 最终 `fault` 仍为首个 `Environment`（文案含锁文本、**不含**「超预算」）；
/// 进入第三 plan 边界时零 spawn。
///
/// **两个边界的判据都靠注入量而非真耗时**：第二 plan 边界 MUST NOT 越预算
/// （「既有 fault 未被预算覆盖」可观测性的前提），第三 plan 边界 MUST 越——
/// 前者由 `预算(8 s) > detect + 首个 plan 的真实成本` 保证，后者由 jest 档
/// 10 s 注入睡眠保证；两侧各留 3 s 以上余量，不随机器负载翻转。
#[tokio::test(start_paused = true)]
async fn 预算不覆盖既有environment() {
    let _gate = ClockGate::new();
    let _window = path_window();
    let ws = ChainWs::new("budget-env").three_plan_mixed();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    set_shim_dest_2(&ws.results_dest_third());
    std::env::set_var("NPX_SLEEP_MS", "2000");
    ws.prepend_path();

    let ws_root = ws.root_str();
    let runner = tokio::spawn(async move {
        execute(
            &ws_root,
            CHANGE,
            ReportSlot::Test,
            foundation::cancel::CancelFlag::never(),
            std::time::Duration::from_secs(8),
        )
        .await
    });
    advance_until(
        || runner.is_finished(),
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(120),
    )
    .await;
    let output = runner
        .await
        .expect("run 任务应正常结束")
        .expect("环境错误以 fault 收口（不 Err）");

    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };
    let Some(RunFault::Environment(text)) = outcome.fault.clone() else {
        panic!("应保持 Environment fault，实际: {:?}", outcome.fault);
    };
    assert!(
        text.contains("os error 32"),
        "既有环境文案（锁文本）在案: {text}"
    );
    assert!(
        !text.contains("超预算"),
        "既有 Environment 不被预算文案覆盖: {text}"
    );
    assert_eq!(line_count(&ws.npx_attempt_log()), 5, "jest 档 5 次尝试");
    assert_eq!(ws.marker_count(), 1, "第三 plan 零 spawn（预算收口）");
    assert!(
        !ws.results_dest_third().exists(),
        "第三 plan 工件未产（零 spawn）"
    );
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "fault 面不写 summary"
    );
}

/// 边界（AC-6 连带 + AC-8）：复用门不进——预算收口后 `reports/test` 无 summary
/// → 同一输入再跑（全预算常量）→ 不复用（marker 增长，重新 spawn）并写 summary。
///
/// 预算档位与睡眠档位同 [`超预算余下plan零spawn`]（3 s / 6 s）：预算须大于
/// detect 的版本探测耗时，而首个 plan 的 spawn 须大于预算——两者之间的窗口由
/// detect 的探测成本决定，1 s 档在门禁并发下不足（flake 根因）。
#[tokio::test]
async fn 预算收口后复用门不进() {
    let _window = path_window();
    let ws = ChainWs::new("budget-reuse").two_plan();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
    set_shim_dest_2(&ws.reports_dir().join("app2_node-test").join("results.txt"));
    std::env::set_var("SHIM_SLEEP_MS", "6000");
    ws.prepend_path();

    let first = execute(
        &ws.root_str(),
        CHANGE,
        ReportSlot::Test,
        foundation::cancel::CancelFlag::never(),
        std::time::Duration::from_secs(3),
    )
    .await
    .expect("超预算仍 Ok");
    let ToolStepOutput::TestExecution(first_outcome) = first else {
        panic!("产出应为 TestExecution 变体");
    };
    assert!(
        matches!(first_outcome.fault, Some(RunFault::Environment(_))),
        "第一跑超预算收口为 Environment fault: {:?}",
        first_outcome.fault
    );
    assert_eq!(ws.marker_count(), 1, "第一跑只跑首个 plan");
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "第一跑无 summary（复用门无可复用面）"
    );

    // 第二跑：全预算（生产常量）→ 不复用，重新 spawn 两个 plan 并写 summary
    std::env::remove_var("SHIM_SLEEP_MS");
    let second = execute(
        &ws.root_str(),
        CHANGE,
        ReportSlot::Test,
        foundation::cancel::CancelFlag::never(),
        TESTEXEC_TOTAL_BUDGET,
    )
    .await
    .expect("全预算应 Ok");
    let ToolStepOutput::TestExecution(second_outcome) = second else {
        panic!("产出应为 TestExecution 变体");
    };
    assert_eq!(
        ws.marker_count(),
        3,
        "第二跑真 spawn 两个 plan（1 + 2 行，未复用半套报告）"
    );
    assert_eq!(second_outcome.fault, None, "全预算面无 fault");
    assert_eq!(
        second_outcome.conclusion,
        TestExecutionConclusion::Pass,
        "全预算绿跑结论 Pass"
    );
    assert!(
        ws.reports_dir().join("summary.json").exists(),
        "全预算写 summary（此后可进复用门）"
    );
}

// ---------------------------------------------------------------------------
// plan 分隔头装配（`push_plan_head`——记录元数据面单点，D10 / D11 / AC-7；
// Mock策略：非 mock——`TestPlan` / `runner::PlanExecution` 结构体字面量直构，
// `argv_len` / `argv_note` 由用例显式给值）
// ---------------------------------------------------------------------------

/// `push_plan_head` 装置：plan 字面量（纯数据；`id` / `framework` / `files` 由
/// 用例给值——头行三字段同源于此）。
fn head_plan(id: &str, framework: &str, files: &[&str]) -> TestPlan {
    TestPlan {
        id: id.to_owned(),
        framework: framework.to_owned(),
        root: "app".to_owned(),
        cwd: PathBuf::from("app"),
        files: files.iter().map(|file| (*file).to_owned()).collect(),
        config_args: None,
        shell_template: "run {files}".to_owned(),
        cmd_template: "run {files}".to_owned(),
        coverage_format: CoverageFormat::NodeTest,
        coverage_output: "results.txt",
        results_output: "results.txt",
        omit_files_safe: true,
    }
}

/// `push_plan_head` 装置：执行结果字面量（纯数据；`output` / `argv_len` /
/// `argv_note` 由用例给值）。
fn head_execution(output: &[u8], argv_len: usize, argv_note: Option<&str>) -> PlanExecution {
    PlanExecution {
        exit_code: 0,
        duration_ms: 1.0,
        cases: Vec::new(),
        source_files: Vec::new(),
        measured: None,
        error: None,
        output: output.to_vec(),
        fault: None,
        argv_len,
        argv_note: argv_note.map(str::to_owned),
    }
}

/// 正向：单 plan——写入 `\n=== plan {id} ({framework}) · files={n} · argv={m}
/// 字符 ===\n`，`output` 逐字节紧随其后；头行以换行起手与收尾、恰一行。
#[test]
fn 计划头装配_单plan头行逐字节与正文紧随() {
    let plan = head_plan("app_node-test", "node-test", &["src/add.test.mjs"]);
    let execution = head_execution(b"shim-stdout-marker\n", 123, None);

    let mut merged = Vec::new();
    push_plan_head(&mut merged, &plan, &execution);
    merged.extend_from_slice(&execution.output);

    assert_eq!(
        String::from_utf8(merged.clone()).expect("UTF-8 记录"),
        "\n=== plan app_node-test (node-test) · files=1 · argv=123 字符 ===\nshim-stdout-marker\n",
        "头行 + 正文逐字节（换行起手、换行收尾）"
    );
    assert_eq!(
        String::from_utf8_lossy(&merged)
            .matches("=== plan ")
            .count(),
        1,
        "单 plan 恰一行头"
    );
    assert!(
        String::from_utf8_lossy(&merged).starts_with('\n'),
        "头以换行起手（与前置正文分段）"
    );
}

/// 正向：多 plan（3 个）——头按推入序（`plans` 迭代序，`BTreeSet` 字典序稳定）
/// 排列、每 plan 恰一行头、各段正文随自身头。
#[test]
fn 计划头装配_多plan按序且各一行头() {
    let cases = [
        ("a_jest", "jest", vec!["src/a.test.ts"]),
        ("b_rust", "rust", Vec::new()),
        ("c_node-test", "node-test", vec!["src/add.test.mjs"]),
    ];
    let mut merged = Vec::new();
    for (index, (id, framework, files)) in cases.iter().enumerate() {
        let plan = head_plan(id, framework, files);
        let execution = head_execution(format!("body-{index}\n").as_bytes(), 100 + index, None);
        push_plan_head(&mut merged, &plan, &execution);
        merged.extend_from_slice(&execution.output);
    }
    let text = String::from_utf8(merged).expect("UTF-8 记录");

    for (index, (id, framework, files)) in cases.iter().enumerate() {
        let head = format!(
            "\n=== plan {id} ({framework}) · files={} · argv={} 字符 ===\nbody-{index}\n",
            files.len(),
            100 + index
        );
        assert!(
            text.contains(&head),
            "第 {index} 段头与自身正文紧邻: {head:?} | {text}"
        );
        assert_eq!(
            text.matches(&format!("=== plan {id} ")).count(),
            1,
            "plan {id} 恰一行头"
        );
    }
    assert_eq!(text.matches("=== plan ").count(), 3, "三 plan 三行头");

    // 头序 = 推入序（第一头在第二头之前、第二在第三之前）
    let order: Vec<usize> = cases
        .iter()
        .map(|(id, _, _)| {
            text.find(&format!("=== plan {id} "))
                .unwrap_or_else(|| panic!("plan {id} 头缺失"))
        })
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "头序严格递增（{order:?}）"
    );
}

/// 正向（表驱动两态）：降级 plan（`argv_note == Some(note)`）→ 头后紧跟
/// `[argv] {note}\n`；非降级 plan（`None`）无 `[argv]` 行（记录零噪声）。
#[test]
fn 计划头装配_降级注行两态() {
    let plan = head_plan("app_node-test", "node-test", &["src/add.test.mjs"]);
    let note = "展开 8230 字符超平台预算 7000 字符，已降级为框架自发现（1 个测试文件）";

    // 形态一：降级面
    let mut merged = Vec::new();
    push_plan_head(&mut merged, &plan, &head_execution(b"", 8230, Some(note)));
    let text = String::from_utf8(merged).expect("UTF-8 记录");
    assert_eq!(
        text,
        format!(
            "\n=== plan app_node-test (node-test) · files=1 · argv=8230 字符 ===\n[argv] {note}\n"
        ),
        "降级注行紧随风头（头 + [argv] 两行）"
    );
    assert_eq!(text.matches("[argv] ").count(), 1, "[argv] 行恰一行");
    assert!(
        text.find("[argv] ").expect("[argv] 行在案") > text.find("=== plan ").expect("头在案"),
        "[argv] 行在头之后（先头后注）"
    );

    // 形态二：未降级面
    let mut merged = Vec::new();
    push_plan_head(&mut merged, &plan, &head_execution(b"", 4527, None));
    let text = String::from_utf8(merged).expect("UTF-8 记录");
    assert_eq!(
        text, "\n=== plan app_node-test (node-test) · files=1 · argv=4527 字符 ===\n",
        "未降级 → 记录仅头行"
    );
    assert!(!text.contains("[argv] "), "未降级零 [argv] 行: {text}");
}

/// 边界（表驱动两形态）：`output` 空（fault 拒绝臂 / 零输出的已执行 plan）→
/// 记录**仅头**（含可选 `[argv]` 行）、非空且可回查；多 plan 全空 → 记录 = 头
/// 序列（非空、序稳定）。
#[test]
fn 计划头装配_空输出仅头非空可回查() {
    // 形态一：单 plan 空输出（拒绝臂口径）
    let plan = head_plan("app_node-test", "node-test", &["src/add.test.mjs"]);
    let mut merged = Vec::new();
    push_plan_head(&mut merged, &plan, &head_execution(b"", 8000, None));
    assert!(!merged.is_empty(), "空输出仍留头（被拒绝的展开长度可回查）");
    assert!(
        String::from_utf8_lossy(&merged).contains("argv=8000 字符"),
        "拒绝臂长度在案: {:?}",
        String::from_utf8_lossy(&merged)
    );

    // 形态二：三 plan 全空输出（fault 面的「仅头序列」形态）
    let mut merged = Vec::new();
    for (index, id) in ["a_jest", "b_rust", "c_node-test"].iter().enumerate() {
        let plan = head_plan(id, "node-test", &["src/x.test.mjs"]);
        push_plan_head(&mut merged, &plan, &head_execution(b"", index, None));
    }
    let text = String::from_utf8(merged).expect("UTF-8 记录");
    assert_eq!(text.matches("=== plan ").count(), 3, "头序列三行");
    assert_eq!(text.matches('\n').count(), 6, "每头一行 + 起手换行各一");
    assert!(
        !text.contains("shim-stdout-marker"),
        "空输出面无脚本正文: {text}"
    );
}

/// 边界：`plan.files == []`（rust 档）与 `argv_len == 0` → 头格式仍逐字
/// （`files=0 · argv=0 字符`），零字段省略。
#[test]
fn 计划头装配_零文件零长度仍逐字() {
    let plan = head_plan("crates_rust", "rust", &[]);
    let mut merged = Vec::new();
    push_plan_head(&mut merged, &plan, &head_execution(b"", 0, None));

    assert_eq!(
        String::from_utf8(merged).expect("UTF-8 记录"),
        "\n=== plan crates_rust (rust) · files=0 · argv=0 字符 ===\n",
        "零文件 / 零长度逐字（零值不省略字段）"
    );
}

/// 边界（表驱动）：非 ASCII `id` / `framework`（中文 root 推导 id）与超长
/// `note`（>1000 字符）→ 逐字节 UTF-8 写入、记录面不截断、不 panic。
#[test]
fn 计划头装配_非ascii与超长注不截断() {
    let note_body = "降级".repeat(600); // 1200 字符（> 1000）
    let plan = head_plan("根目录_node-test", "中文框架", &["src/名称.test.mjs"]);
    let mut merged = Vec::new();
    let execution = head_execution("正文".as_bytes(), 8230, Some(&note_body));
    push_plan_head(&mut merged, &plan, &execution);
    merged.extend_from_slice(&execution.output);

    let text = String::from_utf8(merged.clone()).expect("逐字节 UTF-8（不切碎多字节）");
    assert!(
        text.contains("=== plan 根目录_node-test (中文框架) · files=1 · argv=8230 字符 ==="),
        "非 ASCII id / framework 逐字: {text}"
    );
    assert!(
        text.contains(&format!("[argv] {note_body}")),
        "超长注全量在案（不截断）"
    );
    assert_eq!(
        merged.len(),
        "\n=== plan 根目录_node-test (中文框架) · files=1 · argv=8230 字符 ===\n".len()
            + "[argv] ".len()
            + note_body.len()
            + "\n".len()
            + "正文".len(),
        "字节长度 == 头 + [argv] 行 + 正文（多字节按 UTF-8 字节计）"
    );
}

/// 边界：头行分隔符字面口径——分隔符为 ` · `（U+00B7 MIDDLE DOT，非 ASCII
/// `|` / 连字符）；`files=` / `argv=` 为十进制整数。
#[test]
fn 计划头装配_分隔符为中间点且数值十进制() {
    let plan = head_plan("app_node-test", "node-test", &["a", "b"]);
    let mut merged = Vec::new();
    push_plan_head(&mut merged, &plan, &head_execution(b"", 7001, None));
    let text = String::from_utf8(merged).expect("UTF-8 记录");

    assert!(
        text.contains(" · "),
        "分隔符为 ` · `（U+00B7 两侧空格）: {text}"
    );
    assert!(text.contains('\u{00B7}'), "分隔符码点为 U+00B7: {text:?}");
    for wrong in [" | ", " - ", " – ", " — ", "•", "*"] {
        assert!(!text.contains(wrong), "不得用 {wrong:?} 作分隔符: {text}");
    }
    assert!(
        text.contains("files=2") && text.contains("argv=7001 字符"),
        "两数值十进制逐字（无 0x / 无千分位）: {text}"
    );
    assert!(
        !text.contains("files=0x") && !text.contains("7_001") && !text.contains("7,001"),
        "十进制整数面无其他进制 / 分隔: {text}"
    );
}

/// 正向（留痕三数一致）：头中 `files=` 来自 `plan.files.len()`、`argv=` 来自
/// `execution.argv_len`——两值互不推导（各自单源，表驱动两形态），与
/// `expand_template` / `decide_argv` 同口径。
#[test]
fn 计划头装配_三数各自单源() {
    let cases: [(&str, &str, Vec<&str>, usize); 2] = [
        ("it_node-test", "node-test", vec!["src/a.test.mjs"], 4527),
        ("it_rust", "rust", Vec::new(), 7000),
    ];
    for (id, framework, files, argv_len) in cases {
        let plan = head_plan(id, framework, &files);
        let mut merged = Vec::new();
        push_plan_head(&mut merged, &plan, &head_execution(b"", argv_len, None));
        let text = String::from_utf8(merged).expect("UTF-8 记录");

        assert!(
            text.contains(&format!(
                "files={} · argv={argv_len} 字符 ===",
                plan.files.len()
            )),
            "files 取 plan.files.len()、argv 取 execution.argv_len（{id}），实际: {text}"
        );
        assert_eq!(
            plan.files.len(),
            files.len(),
            "files= 与清单长度同源（{id}）"
        );
    }
}

/// 异常（结构性断言，AC-3 / AC-7）：降级 plan（`argv_note == Some(…)`、
/// `error == None`）→ `SubReport.findings == None`；`summary.findings` 不含降级
/// 注文本、`conclusion` 仍 `Pass`（降级说明只走记录通路，MUST NOT 进 findings
/// ——findings 非空 + failed 0 会被判成 execution_error，绿跑翻红）。
#[test]
fn 子报告findings_降级注零注入且结论仍pass() {
    let note = "展开 8230 字符超平台预算 7000 字符，已降级为框架自发现（1 个测试文件）";
    let plan = head_plan("app_node-test", "node-test", &["src/add.test.mjs"]);
    let mut execution = head_execution(b"", 8230, Some(note));
    execution.cases = vec![TestCaseResult {
        name: "adds numbers".to_owned(),
        file: Some("src/add.test.mjs".to_owned()),
        duration_ms: None,
        status: TestCaseStatus::Passed,
        line: None,
        error_type: None,
        error_message: None,
        stack_trace: None,
    }];

    let sub = build_sub_report(&plan, execution, &[]);
    assert_eq!(
        sub.findings, None,
        "降级面零 findings 注入（note 不走 findings 通路）"
    );
    assert_eq!(sub.error_cases.len(), 0, "绿跑零失败用例");
    assert_eq!(
        (sub.summary.total, sub.summary.failed),
        (1, 0),
        "一用例零失败"
    );

    let summary = checks::build_summary_report(
        std::slice::from_ref(&sub),
        &[],
        "desktop change run: test-execution gate",
        time::OffsetDateTime::now_utc(),
    );
    assert_eq!(
        summary.conclusion,
        Conclusion::Pass,
        "降级不改变结论（绿跑仍 Pass）"
    );
    let findings_text = summary
        .findings
        .as_ref()
        .map(|items| items.join("\n"))
        .unwrap_or_default();
    assert!(
        !findings_text.contains("已降级为框架自发现") && !findings_text.contains("展开 8230 字符"),
        "summary.findings 不含降级注文本（降级说明只进记录头），实际: {findings_text:?}"
    );
    let diagnoses = checks::diagnose_findings(&summary);
    assert!(
        !diagnoses
            .iter()
            .any(|item| item.contains("降级为框架自发现") || item.contains("[argv]")),
        "诊断面不含降级注（结构性不可达，非纪律），实际: {diagnoses:?}"
    );
}

/// 回归（边界）：真红 plan（`error == Some(…)`）+ `argv_note == None` →
/// `findings` 仍为该错误（零改注入面，既有语义不回退）。
#[test]
fn 子报告findings_真红plan错误面不回退() {
    let plan = head_plan("app_node-test", "node-test", &["src/add.test.mjs"]);
    let mut execution = head_execution(b"", 4527, None);
    execution.exit_code = 1;
    execution.error = Some("结果工件缺失: results.txt".to_owned());
    execution.cases = vec![failed_case("adds numbers", "expected 1 to be 2")];

    let sub = build_sub_report(&plan, execution, &[]);
    assert_eq!(
        sub.findings,
        Some(vec!["结果工件缺失: results.txt".to_owned()]),
        "真红 error 面逐字进 findings（未被 argv_note 面替换）"
    );
    assert_eq!(sub.summary.failed, 1, "失败用例计数在案");

    let summary = checks::build_summary_report(
        std::slice::from_ref(&sub),
        &[],
        "desktop change run: test-execution gate",
        time::OffsetDateTime::now_utc(),
    );
    assert_ne!(
        summary.conclusion,
        Conclusion::Pass,
        "真红结论不取 Pass（findings 注入面零改）"
    );
    assert!(
        !summary
            .findings
            .as_ref()
            .map(|items| items.join("\n"))
            .unwrap_or_default()
            .contains("[argv]"),
        "findings 不含记录头元数据行"
    );
}

// ---------------------------------------------------------------------------
// 记录聚合面（`execute` 全链：先写头再拼 output、每已执行 plan 恰一行头、
// fault 在位亦写头——Mock策略：文件系统 / 记录单点不 mock，子进程走既有 node
// shim，`exec_id` 回查即被测语义）
// ---------------------------------------------------------------------------

/// node-test 注册表模板（与 detect `REGISTRY` 逐字同串——`argv=` 口径的独立
/// 构造面：头中数字须等于该模板按 `expand_template` 规则展开后的字符数）。
const NODE_TEST_TEMPLATE: &str = "node --test --experimental-test-coverage --test-reporter=spec --test-reporter-destination=\"{results_file}\" {files}";

/// 正向（AC-7）：全链两 plan——记录以第一 plan 头起手、两 plan 头按 plan 序
/// 排列、**去掉头与 `[argv]` 行后正文与脚本输出逐字节一致**（plan 序保序）。
#[tokio::test]
async fn 记录聚合面_两头起手且去头后正文逐字节一致() {
    let ws = ChainWs::new("head-two-plan").two_plan();
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    // 第 2 个 plan 的报告目录预建：`SHIM_DEST_2` 在第 1 个 plan 执行期即被拷贝
    // （此刻第 2 个 plan 的目录尚未由 `execute_plan` 创建）——预建使该次拷贝
    // 不产 stderr 噪声（正文逐字节断言面不被壳侧噪音污染）
    fs::create_dir_all(ws.reports_dir().join("app2_node-test")).expect("预建第二 plan 报告目录");
    let output = {
        let _window = path_window();
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        set_shim_dest_2(&ws.reports_dir().join("app2_node-test").join("results.txt"));
        ws.run().await
    }
    .expect("双 plan 全链应 Ok");
    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };
    assert_eq!(ws.marker_count(), 2, "两 plan 各一次执行");

    let recorded = exec::read_exec_output(ws.root(), CHANGE, &outcome.exec_id).expect("记录可回查");
    let text = String::from_utf8_lossy(&recorded).into_owned();

    assert!(
        text.starts_with("\n=== plan app_node-test (node-test) · files=1 · argv="),
        "记录以第一 plan 头起手: {text:?}"
    );
    assert_eq!(
        text.matches("=== plan ").count(),
        2,
        "两已执行 plan 恰两行头"
    );
    let first = text.find("=== plan app_node-test ").expect("第一头在案");
    let second = text.find("=== plan app2_node-test ").expect("第二头在案");
    assert!(
        first < second,
        "头按 plan 迭代序（字典序稳定）排列（{first} < {second}）"
    );

    // 去头（含可选 `[argv]` 行）→ 正文 = 各 plan 脚本输出按序拼接
    let body: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("=== plan ") && !line.starts_with("[argv] "))
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(
        body,
        vec!["shim-stdout-marker", "shim-stdout-marker"],
        "去头后正文与脚本真实输出逐字节一致、plan 序保序（实际: {text:?}）"
    );
    assert!(!text.contains("[argv] "), "未降级面零 [argv] 行: {text:?}");
}

/// 正向：留痕三数一致——头中 `files=` 等于 detect 出的清单长度、`argv=` 等于
/// `WithFiles` 展开字符数（与 `expand_template` / `decide_argv` 同口径的独立
/// 构造）；未超预算面零 `[argv]` 行。
#[tokio::test]
async fn 记录聚合面_头中files与argv与展开同口径() {
    let ws = ChainWs::new("head-numbers");
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    let output = {
        let _window = path_window();
        set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);
        ws.run().await
    }
    .expect("单 plan 全链应 Ok");
    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };
    let recorded = exec::read_exec_output(ws.root(), CHANGE, &outcome.exec_id).expect("记录可回查");
    let text = String::from_utf8_lossy(&recorded).into_owned();
    let head = text
        .lines()
        .find(|line| line.starts_with("=== plan app_node-test "))
        .unwrap_or_else(|| panic!("plan 头缺失: {text:?}"));

    // files= 与 fixture 树命中清单同源（app/src/add.test.mjs 一枚）
    assert!(
        head.contains("(node-test) · files=1 · argv="),
        "files= 等于 plan.files.len()（fixture 树单文件）: {head}"
    );

    // argv= 与展开字符数同口径（独立构造：注册表模板 + 同展开规则）
    let results_file = to_posix(&ws.reports_dir().join("app_node-test").join("results.txt"));
    let expanded_len = NODE_TEST_TEMPLATE
        .replace("{results_file}", &results_file)
        .replace("{files}", "src/add.test.mjs")
        .chars()
        .count();
    assert!(
        head.contains(&format!("argv={expanded_len} 字符 ===")),
        "argv= 等于 WithFiles 展开字符数（期望 {expanded_len}），实际头: {head}"
    );
    assert!(
        expanded_len < 7000,
        "fixture 展开远低于预算（Keep 臂——零降级前提）"
    );
    assert!(!text.contains("[argv] "), "Keep 臂零降级注行: {text:?}");
}

/// 异常：fault 收口路径（`fault_outcome`）——该 plan 的头仍在记录中（展开长度
/// 可回查）、记录正文除头外为空（进程未拉起）、summary / 子报告零落盘。
#[tokio::test]
async fn 记录聚合面_fault收口仍留头且零报告落盘() {
    let _window = path_window();
    let ws = ChainWs::new("head-fault-cancel");
    let spec_src = ws.root().join("preset-results.txt");
    fs::write(&spec_src, SPEC_OUTPUT).expect("写预置工件");
    let marker = ws.root().join("shim-marker.log");
    set_shim_env(&spec_src, &ws.results_dest(), Some(&marker), None);

    let cancel = foundation::cancel::CancelFlag::never();
    cancel.handle().set();
    let output = ws.run_with(cancel).await.expect("fault 收口（不 Err）");
    let ToolStepOutput::TestExecution(outcome) = output else {
        panic!("产出应为 TestExecution 变体");
    };
    assert_eq!(outcome.fault, Some(RunFault::Stopped), "取消 → fault");

    let recorded =
        exec::read_exec_output(ws.root(), CHANGE, &outcome.exec_id).expect("fault 面记录可回查");
    let text = String::from_utf8_lossy(&recorded).into_owned();
    assert!(
        text.starts_with("\n=== plan app_node-test (node-test) · files=1 · argv="),
        "fault 面仍写头（该 plan 的展开长度可回查）: {text:?}"
    );
    assert_eq!(text.matches("=== plan ").count(), 1, "单 plan 单头");
    let body: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("=== plan ") && !line.starts_with("[argv] "))
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert!(
        body.is_empty(),
        "进程未拉起 → 记录除头外无正文（实际: {text:?}）"
    );
    assert_eq!(ws.marker_count(), 0, "入口短路零 spawn");
    assert!(
        !ws.reports_dir().join("summary.json").exists(),
        "fault 面不写 summary（半套报告不进复用门）"
    );
    assert!(
        !ws.reports_dir()
            .join("app_node-test")
            .join("report.json")
            .exists(),
        "fault 面零子报告落盘"
    );
}

// ---------------------------------------------------------------------------
// archi 槽位守卫（D19 / AC-14）：test-execution runner 同式拒收
// ---------------------------------------------------------------------------

/// 异常（D19）：`ReportSlot::Archi` → `Err`，文案含槽位词与归属方；守卫先于
/// detect（空 tempdir 无任何项目清单，仍返槽位 `Err` 而非 detect 错）。
#[tokio::test]
#[allow(non_snake_case)]
async fn archi槽位显式Err先于detect() {
    let temp = TempDir::new().expect("建临时目录失败");
    let root = temp.path().to_string_lossy().into_owned();
    let error = ProcessTestExecution::new()
        .run(
            &root,
            "demo",
            ReportSlot::Archi,
            foundation::cancel::CancelFlag::never(),
        )
        .await
        .expect_err("checks 域 MUST NOT 接受 archi 槽位");
    assert!(
        error.contains("archi") && error.contains("架构门禁"),
        "文案含槽位词与归属方：{error}"
    );
}

/// 边界：幂等 + 零写盘 + 槽位隔离（`reports/archi` 零创建，`reports/test` /
/// `reports/baseline` 零变化）。
#[tokio::test]
#[allow(non_snake_case)]
async fn archi槽位守卫幂等且零写盘() {
    let temp = TempDir::new().expect("建临时目录失败");
    let root = temp.path().to_string_lossy().into_owned();
    let before = fs::read_dir(temp.path())
        .expect("读目录失败")
        .flatten()
        .count();
    let first = ProcessTestExecution::new()
        .run(
            &root,
            "demo",
            ReportSlot::Archi,
            foundation::cancel::CancelFlag::never(),
        )
        .await
        .expect_err("拒收");
    let second = ProcessTestExecution::new()
        .run(
            &root,
            "demo",
            ReportSlot::Archi,
            foundation::cancel::CancelFlag::never(),
        )
        .await
        .expect_err("拒收");
    assert_eq!(first, second, "幂等：两次文案逐字相等");
    assert_eq!(
        fs::read_dir(temp.path())
            .expect("读目录失败")
            .flatten()
            .count(),
        before,
        "零写盘：根目录条目数不变"
    );
    assert!(!temp.path().join("openspec").exists(), "零报告目录创建");
}

/// 边界：零 spawn——`PATH` 隔离窗口（清空，任何外部程序不可达）下 `Archi` 槽位
/// 仍返回槽位 `Err`（守卫先于执行链，不进入计划执行，也不短路成 detect /
/// spawn 类错误）。
#[tokio::test]
#[allow(non_snake_case)]
async fn archi槽位零spawn_PATH隔离仍返槽位err() {
    let window = PathWindowGuard::enter();
    let temp = TempDir::new().expect("建临时目录失败");
    let root = temp.path().to_string_lossy().into_owned();
    std::env::set_var("PATH", "");

    let result = ProcessTestExecution::new()
        .run(
            &root,
            "demo",
            ReportSlot::Archi,
            foundation::cancel::CancelFlag::never(),
        )
        .await;
    drop(window);

    let error = result.expect_err("PATH 隔离下仍应返回槽位 Err");
    assert!(
        error.contains("archi") && error.contains("架构门禁"),
        "文案仍为槽位归属（守卫先于执行链）：{error}"
    );
    assert!(
        !error.contains("未识别项目") && !error.contains("拉起失败") && !error.contains("不可达"),
        "不因 PATH 隔离短路成 detect / spawn 类错误：{error}"
    );
    assert!(
        !temp.path().join("openspec").exists(),
        "零 spawn ⇒ 零报告目录创建"
    );
}
