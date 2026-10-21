use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use super::{
    execute_plan, expand_template, non_zero_exit_error, EXIT_CODE_NO_STATUS, SUITE_COMMAND_TIMEOUT,
    VERSION_DETECT_TIMEOUT,
};
use crate::testexec::detect::{to_posix, TestPlan};
use checks::parser::CoverageFormat;

/// node-test spec 绿跑样本（五用例 4 pass 1 skipped + "All files" 覆盖汇总行；
/// 结果与覆盖工件同文件——node-test 档 results.txt 双读取面。覆盖表格行以
/// 管道起手——实测块提取按 `|` 切列后取第 2 列为 lines）。
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

/// 手工 TestPlan 构造（node-test 档：结果与覆盖工件同名同文件，模板由用例
/// 按平台注入；TestPlan 字段 pub(crate)——进程内构造装置）。
fn node_test_plan(cwd: &Path, template: String) -> TestPlan {
    TestPlan {
        id: "it_node-test".to_owned(),
        framework: "node-test".to_owned(),
        root: "app".to_owned(),
        cwd: cwd.to_path_buf(),
        files: vec!["src/add.test.mjs".to_owned()],
        config_args: None,
        shell_template: template.clone(),
        cmd_template: template,
        coverage_format: CoverageFormat::NodeTest,
        coverage_output: "results.txt",
        results_output: "results.txt",
    }
}

/// 手工 TestPlan 构造（结果 / 覆盖双工件分离形态——rust 档双工件读取面锚用：
/// 结果工件承载单条命令的 stdout、覆盖工件由同一命令 `--output-path` 落位）。
fn rust_plan(cwd: &Path, template: String) -> TestPlan {
    TestPlan {
        id: "it_rust-single".to_owned(),
        framework: "rust".to_owned(),
        root: "app".to_owned(),
        cwd: cwd.to_path_buf(),
        files: Vec::new(),
        config_args: None,
        shell_template: template.clone(),
        cmd_template: template,
        coverage_format: CoverageFormat::LlvmCov,
        coverage_output: "coverage-summary.json",
        results_output: "results.txt",
    }
}

/// 手工 TestPlan 构造（jest 档：istanbul 族结果工件不解析用例行、覆盖实测块读
/// `coverage-summary.json`——AC-7 runner 侧零改动锚）。
fn jest_plan(cwd: &Path, template: String) -> TestPlan {
    TestPlan {
        id: "it_jest".to_owned(),
        framework: "jest".to_owned(),
        root: "app".to_owned(),
        cwd: cwd.to_path_buf(),
        files: vec!["src/a.test.ts".to_owned()],
        config_args: None,
        shell_template: template.clone(),
        cmd_template: template,
        coverage_format: CoverageFormat::Istanbul,
        coverage_output: "coverage-summary.json",
        results_output: "results.json",
    }
}

/// rust 档结果工件样本（cargo test 文本：两个 `running` 块 / 4 用例行
/// = 2 ok + 1 FAILED + 1 ignored；失败详情块置末——解析以块头切段）。
const RUST_RESULTS_OUTPUT: &str = "   Compiling dev-team v0.1.0\n\
    Finished `test` profile [unoptimized + debuginfo] target(s)\n\
     Running unittests src/lib.rs (target/llvm-cov-target/debug/deps/dev_team-1a2b)\n\
running 2 tests\n\
test parser::parse_ok ... ok\n\
test parser::skipped_case ... ignored\n\
\n\
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out\n\
     Running tests/it.rs (target/llvm-cov-target/debug/deps/it-3c4d)\n\
running 2 tests\n\
test it_works ... ok\n\
test it_fails ... FAILED\n\
\n\
failures:\n\
\n\
---- it_fails stdout ----\n\
assertion failed: left == right\n\
\n\
failures:\n\
    it_fails\n\
\n\
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n";

/// rust 档覆盖工件样本（llvm-cov JSON：两文件 + totals 三档——branches 计数 0
/// 走 null 感知跳过）。
const LLVM_COV_OUTPUT: &str = r#"{
  "data": [
    {
      "files": [
        { "filename": "/ws/crates/src/lib.rs", "segments": [[1, 1, 3, true, true, false], [2, 1, 0, true, true, false], [3, 1, 2, true, true, false]] },
        { "filename": "/ws/crates/tests/it.rs", "segments": [[1, 1, 1, true, true, false]] }
      ],
      "totals": {
        "lines": { "count": 4, "covered": 3, "percent": 75.0 },
        "branches": { "count": 0, "covered": 0, "percent": 0.0 },
        "functions": { "count": 2, "covered": 2, "percent": 100.0 }
      }
    }
  ]
}"#;

/// istanbul 覆盖汇总样本（jest 档覆盖工件）。
const ISTANBUL_SUMMARY: &str = r#"{
  "total": {
    "lines": { "pct": 88.5 },
    "branches": { "pct": 70.0 },
    "functions": { "pct": 91.0 }
  }
}"#;

/// istanbul 族结果报告样本（JSON 报告器产物——V1 用例行面不解析）。
const JEST_RESULTS_JSON: &str = r#"{ "numTotalTests": 1, "numPassedTests": 1 }"#;

/// 把预置文本写到 workspace 外的临时工件源（拷贝命令的 src）。
fn preset(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("写预置工件失败");
    path
}

/// 进程环境锁窗口（RAII）
struct EnvWindow {
    _guard: std::sync::MutexGuard<'static, ()>,
}

fn env_window() -> EnvWindow {
    EnvWindow {
        _guard: crate::TEST_ENV_LOCK.lock().expect("crate 环境锁不可中毒"),
    }
}

/// 结果工件文本归一：CRLF → LF，并去除逐行尾随空白。rust 档 stdout 经模板自带的
/// `> "{results_file}"` 重定向落工件（生产 spawn 为 `cmd /C` 双层包裹），cmd 会把
/// 重定向符前的分隔空格计入 echo 参数——尾随空格是壳解析产物而非工件内容语义
/// （stdout 全量落工件不变），故比对前归一。
fn artifact_text(text: &str) -> String {
    let lf = text.replace("\r\n", "\n");
    let ends_with_newline = lf.ends_with('\n');
    let mut lines = lf.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
    if ends_with_newline {
        lines.push('\n');
    }
    lines
}

// ---------------------------------------------------------------------------
// 模板展开（占位符替换——spawn 前纯函数面）
// ---------------------------------------------------------------------------

/// 正向：五占位符全替换逐字断言——config_args 有值形态原样注入、空形态
///（None）连前导空白一并剥离（CLI 同语义）、files 空清单替换为空串。
#[test]
fn 模板展开五占位符替换_config_args双形态() {
    let template = "run --reporter=json --out=\"{results_file}\" --cov-out=\"{coverage_file}\" --dir=\"{report_dir}\" {config_args} --files {files}";

    // 有值形态：config_args 原样注入（占位符前导空格保留）
    let expanded = expand_template(
        template,
        "/ws/reports/plan",
        "/ws/reports/plan/results.json",
        "/ws/reports/plan/coverage.json",
        Some("--config \"/ws/vitest.config.ts\""),
        &["src/a.test.ts".to_owned(), "src/b.test.ts".to_owned()],
    );
    assert_eq!(
        expanded,
        "run --reporter=json --out=\"/ws/reports/plan/results.json\" --cov-out=\"/ws/reports/plan/coverage.json\" --dir=\"/ws/reports/plan\" --config \"/ws/vitest.config.ts\" --files src/a.test.ts src/b.test.ts",
        "五占位符全替换逐字（results_file / coverage_file / report_dir / config_args / files）"
    );

    // 空形态：None → {config_args} 连前导空白剥离；files 空 → 空串
    let expanded = expand_template(
        template,
        "/ws/reports/plan",
        "/ws/reports/plan/results.json",
        "/ws/reports/plan/coverage.json",
        None,
        &[],
    );
    assert_eq!(
        expanded,
        "run --reporter=json --out=\"/ws/reports/plan/results.json\" --cov-out=\"/ws/reports/plan/coverage.json\" --dir=\"/ws/reports/plan\" --files ",
        "None config_args 剥离前导空白、空 files 置空串"
    );
}

/// 正向（生产模板形态）：rust 新单条串展开——两工件路径逐字替换、`{config_args}`
/// 未注入时连前导空白剥离、`{files}` 不在模板内、`> "{results_file}"` 重定向恰一次。
#[test]
fn rust模板展开_双工件路径逐字且重定向恰一次() {
    let template = "cargo llvm-cov --workspace --json --output-path \"{coverage_file}\" > \"{results_file}\"";
    let expanded = expand_template(
        template,
        "/ws/reports/crates_rust",
        "/ws/reports/crates_rust/results.txt",
        "/ws/reports/crates_rust/coverage-summary.json",
        None,
        &["tests/it.rs".to_owned()],
    );
    assert_eq!(
        expanded,
        "cargo llvm-cov --workspace --json --output-path \"/ws/reports/crates_rust/coverage-summary.json\" > \"/ws/reports/crates_rust/results.txt\"",
        "双工件路径逐字替换（report_dir 不参与本模板、files 不出现）"
    );
    assert_eq!(
        expanded.matches("> \"").count(),
        1,
        "结果重定向恰一次（单条命令双工件）"
    );
    assert!(
        !expanded.contains('{'),
        "零占位符残留（五占位符展开齐备），实际: {expanded}"
    );
}

/// 边界（表驱动 4 形态）：`{files}` 清单规模与占位符形态——单元素 / 大清单
/// （50 项）空格连接逐字；同一占位符多次出现 → 全量替换（`replace` 全量语义）；
/// 未知占位符（`{unknown}`）逐字保留且不 panic（展开面不做校验拦截）。
#[test]
fn 模板展开清单规模与占位符形态() {
    // 形态一：单元素清单
    let expanded = expand_template(
        "run --files {files}",
        "/ws/reports/plan",
        "/ws/reports/plan/results.txt",
        "/ws/reports/plan/coverage-summary.json",
        None,
        &["tests/it.rs".to_owned()],
    );
    assert_eq!(expanded, "run --files tests/it.rs", "单元素清单逐字展开");

    // 形态二：大清单（50 项）逐字空格连接
    let many: Vec<String> = (0..50).map(|index| format!("tests/t{index:02}.rs")).collect();
    let expanded = expand_template(
        "run --files {files}",
        "/ws/reports/plan",
        "/ws/reports/plan/results.txt",
        "/ws/reports/plan/coverage-summary.json",
        None,
        &many,
    );
    assert_eq!(
        expanded,
        format!("run --files {}", many.join(" ")),
        "大清单（50 项）空格连接逐字"
    );
    assert_eq!(
        expanded.matches("tests/t").count(),
        50,
        "50 项全量在场（不截断不丢项）"
    );

    // 形态三：同一占位符多次出现 → 全量替换
    let expanded = expand_template(
        "{files} | {files} | {results_file} | {results_file}",
        "/ws/reports/plan",
        "/ws/reports/plan/results.txt",
        "/ws/reports/plan/coverage-summary.json",
        None,
        &["a.rs".to_owned(), "b.rs".to_owned()],
    );
    assert_eq!(
        expanded, "a.rs b.rs | a.rs b.rs | /ws/reports/plan/results.txt | /ws/reports/plan/results.txt",
        "同一占位符多次出现全量替换（replace 全量语义）"
    );

    // 形态四：未知占位符逐字保留且不 panic
    let expanded = expand_template(
        "run {unknown} {files}",
        "/ws/reports/plan",
        "/ws/reports/plan/results.txt",
        "/ws/reports/plan/coverage-summary.json",
        None,
        &[],
    );
    assert_eq!(
        expanded, "run {unknown} ",
        "未知占位符逐字保留（不 panic、不清空命令）、空清单置空串"
    );
}

/// 异常 / 边界（参数化 4 形态）：空模板 / 全空白模板 → `execute_plan` 前置检查
/// 显式 `Err` 记因「展开后的命令为空白」（不 spawn、不 panic）；含特殊字符
///（双引号 / 换行 / emoji）与超长（>1000 字符）的模板经展开后逐字保留（占位符
/// 替换不做转义、不改写、不截断）。
#[tokio::test]
async fn 模板串边界_空与空白err_特殊字符逐字保留() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");

    for (shape, template) in [("空模板", ""), ("全空白模板", "   \n\t  ")] {
        let plan = rust_plan(&cwd, template.to_owned());
        let err = execute_plan(&plan, &ws.path().join("reports").join("blank"), foundation::cancel::CancelFlag::never())
            .await
            .expect_err(&format!("{shape} 应显式 Err（展开后的命令为空白）"));
        assert!(
            err.contains("展开后的命令为空白"),
            "{shape} Err 记因空白命令（前置检查零改动），实际: {err}"
        );
    }

    // 特殊字符形态：展开面逐字保留（无转义、无改写）
    let template = "runner \"{results_file}\" --note \"line1\nline2\" --cov \"{coverage_file}\" 🚀 \"{report_dir}\"";
    let expanded = expand_template(
        template,
        "/ws/reports/plan",
        "/ws/reports/plan/results.txt",
        "/ws/reports/plan/coverage-summary.json",
        None,
        &[],
    );
    assert_eq!(
        expanded,
        "runner \"/ws/reports/plan/results.txt\" --note \"line1\nline2\" --cov \"/ws/reports/plan/coverage-summary.json\" 🚀 \"/ws/reports/plan\"",
        "特殊字符（引号 / 换行 / emoji）逐字保留"
    );
    assert_eq!(
        expanded.matches('\n').count(),
        1,
        "换行原样保留（未被转义成 `\\n` 字面量）"
    );
    assert!(
        !expanded.contains("\\n") && !expanded.contains("\\\""),
        "无反斜杠转义注入（占位符替换不做转义），实际: {expanded}"
    );
    assert!(
        expanded.contains('🚀'),
        "emoji 逐字保留（多字节字符不被切碎），实际: {expanded}"
    );

    // 超长模板形态：>1000 字符填充段逐字保留（展开面不截断、不误命中填充内
    // 的伪占位符片段）
    let filler = "x".repeat(1200);
    let long_template = format!("runner {filler} \"{{results_file}}\"");
    let expanded = expand_template(
        &long_template,
        "/ws/reports/plan",
        "/ws/reports/plan/results.txt",
        "/ws/reports/plan/coverage-summary.json",
        None,
        &[],
    );
    assert_eq!(
        expanded,
        format!("runner {filler} \"/ws/reports/plan/results.txt\""),
        "超长模板（>1000 字符）逐字展开（填充段零改写）"
    );
    assert!(
        expanded.len() > 1200,
        "超长模板展开不截断，实际 {} 字符",
        expanded.len()
    );
    assert!(
        !expanded.contains('{') && !expanded.contains('}'),
        "占位符在超长模板中同样替换干净，实际尾部: {:?}",
        expanded.chars().rev().take(40).collect::<String>()
    );
}

// ---------------------------------------------------------------------------
// 真实 spawn：预置工件 → 解析产出
// ---------------------------------------------------------------------------

/// 正向：Windows .cmd / Unix cp 真实 spawn——命令把预置 spec 工件拷到
/// {results_file} 占位路径 → PlanExecution 携解析后用例行与覆盖实测（退出码
/// 0、诊断无 error 面、报告目录清空重建——重跑幂等）。注：本用例走免引号
/// 形态；生产注册表模板的引号形态由
/// `windows引号占位模板_重定向工件落位` 回归锚定。
#[tokio::test]
async fn 真实spawn_预置工件拷贝到占位路径_解析产出() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let spec_src = preset(ws.path(), "preset-results.txt", SPEC_OUTPUT);

    // 预置旧报告目录内容（清理重建面：旧残留须被清掉）
    let report_dir = ws.path().join("reports").join("it_node-test");
    std::fs::create_dir_all(&report_dir).expect("创建旧报告目录");
    let stale = report_dir.join("stale-artifact.txt");
    std::fs::write(&stale, "旧残留").expect("写旧残留");

    // 平台模板：首 token 均为 PATH / 系统目录可达程序；{results_file} 占位符
    // 由 execute_plan 展开（node-test 档 results 与 coverage 同文件）
    let template = if cfg!(windows) {
        format!(
            "cmd /C type {} > {{results_file}}",
            spec_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", spec_src.to_string_lossy())
    };
    let plan = node_test_plan(&cwd, template);

    let execution = execute_plan(&plan, &report_dir, foundation::cancel::CancelFlag::never())
        .await
        .expect("退出 0 + 工件在位应 Ok");

    assert_eq!(execution.exit_code, 0, "退出码透传");
    assert!(
        execution.error.is_none(),
        "成功跑零 error 面，实际: {:?}",
        execution.error
    );
    assert!(execution.duration_ms >= 0.0, "耗时毫秒面在场");
    assert!(!stale.exists(), "报告目录清空重建（旧残留清除——重跑幂等）");

    // 工件解析面：spec 文本用例行（4 passed / 1 skipped）+ "All files" 实测块
    let passed = execution
        .cases
        .iter()
        .filter(|case| checks::model::TestCaseStatus::Passed == case.status)
        .count();
    let skipped = execution
        .cases
        .iter()
        .filter(|case| checks::model::TestCaseStatus::Skipped == case.status)
        .count();
    assert_eq!(
        execution.cases.len(),
        5,
        "五用例行解析（套件行 / 汇总行不产用例）"
    );
    assert_eq!(passed, 4);
    assert_eq!(skipped, 1);
    let measured = execution.measured.expect("覆盖实测块在场");
    assert_eq!(measured.lines, Some(82.5));
    assert_eq!(measured.branches, Some(74.1));
    assert_eq!(measured.functions, Some(90.3));
    assert!(
        execution.source_files.is_empty(),
        "node-test 无逐文件原始计数（V1 形态）"
    );
}

/// 回归：引号占位形态（生产注册表模板形——源路径与 `{results_file}` 重定向
/// 目标均带引号）。Windows 臂曾因 `arg` 的 MSVC 转引把 `"` 写成 `\"`、cmd
/// 解析为反斜杠附着路径而全工件缺失（错误面「结果工件缺失」+ 零用例——
/// 2026-10 真实 gate 故障形态）；`raw_arg` 原样入线后引号形态必须正常产出。
#[tokio::test]
async fn windows引号占位模板_重定向工件落位() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let spec_src = preset(ws.path(), "quoted-preset.txt", SPEC_OUTPUT);

    // 双臂同走引号形态：Windows 是回归面（cmd 解析），Unix 是同语义锚
    //（sh -c 对引号本就透明）
    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\"",
            spec_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", spec_src.to_string_lossy())
    };
    let plan = node_test_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("it_node-test"), foundation::cancel::CancelFlag::never())
        .await
        .expect("引号形态应正常执行");

    assert_eq!(execution.exit_code, 0, "退出码透传（引号模板免转引破坏）");
    assert!(
        execution.error.is_none(),
        "引号路径工件须落位零错误面，实际: {:?}",
        execution.error
    );
    assert_eq!(execution.cases.len(), 5, "引号路径工件解析产出五用例行");
    assert!(execution.measured.is_some(), "覆盖实测块随工件解析在场");
}

// ---------------------------------------------------------------------------
// rust 档单条命令（rust-suite-single-build）：零改写 / 双工件同落位 / 退出码直继承
// ---------------------------------------------------------------------------

/// 正向（AC-2 零改写锚）：模板自带 `> "{results_file}"` 重定向——展开后的命令串
/// 与 `expand_template` 直接展开逐字相等，runner 不再插入 `cargo test > …`；
/// 结果工件全量承载 stdout（重定向恰一次），捕获面只余 stderr。
#[tokio::test]
async fn rust档零改写_模板自带重定向_命令串原样展开() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let res_src = preset(
        ws.path(),
        "rewrite-results-preset.txt",
        RUST_RESULTS_OUTPUT,
    );
    let report_dir = ws.path().join("reports").join("rust-rewrite");

    // 替身模板：生产形态（模板自带结果重定向）+ stderr 打点
    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\" & 1>&2 echo build-error",
            res_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!(
            "cp \"{}\" \"{{results_file}}\"; echo build-error >&2",
            res_src.to_string_lossy()
        )
    };
    let plan = rust_plan(&cwd, template.clone());

    // 执行串即同一纯函数展开的结果：五占位符逐字替换、零后置拼装
    let expected = expand_template(
        &template,
        &to_posix(&report_dir),
        &to_posix(&report_dir.join("results.txt")),
        &to_posix(&report_dir.join("coverage-summary.json")),
        None,
        &[],
    );
    let results_posix = to_posix(&report_dir.join("results.txt"));
    assert_eq!(
        expected
            .matches(&format!("> \"{results_posix}\""))
            .count(),
        1,
        "结果重定向在展开串中恰一次（无 `cargo test >` 二次注入），实际: {expected}"
    );
    assert!(
        !expected.contains("cargo test"),
        "展开串无 `cargo test` 注入锚点（重定向已进模板），实际: {expected}"
    );

    let execution = execute_plan(&plan, &report_dir, foundation::cancel::CancelFlag::never())
        .await
        .expect("替身命令应 Ok");

    assert_eq!(execution.exit_code, 0, "退出码透传");
    assert!(
        execution.error.is_none(),
        "绿跑零 error 面，实际: {:?}",
        execution.error
    );
    let results = std::fs::read_to_string(report_dir.join("results.txt"))
        .expect("结果工件落报告树（模板自带重定向面）");
    assert_eq!(
        artifact_text(&results),
        artifact_text(RUST_RESULTS_OUTPUT),
        "stdout 全量进结果工件（单次重定向，逐字）"
    );
    let merged = String::from_utf8_lossy(&execution.output).replace("\r\n", "\n");
    assert_eq!(
        merged, "build-error\n",
        "output 只余 stderr（stdout 已进工件，未加 `2>&1`）"
    );
    assert_eq!(
        execution.cases.len(),
        4,
        "工件解析面不受改写面影响（多 running 块照常解析）"
    );
}

/// 正向（AC-4）：一条替身命令同时写 `{results_file}`（stdout 重定向）与
/// `{coverage_file}`（模拟 `--output-path`）→ 双工件同落位、两读取面均从同一
/// 命令产物解析（用例行 + 逐文件覆盖 + 实测块）。
#[tokio::test]
async fn 单条命令双工件同落位_结果与覆盖同源解析() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let res_src = preset(ws.path(), "dual-results-preset.txt", RUST_RESULTS_OUTPUT);
    let cov_src = preset(ws.path(), "dual-coverage-preset.json", LLVM_COV_OUTPUT);
    let report_dir = ws.path().join("reports").join("rust-dual");

    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{coverage_file}}\" & cmd /C type \"{}\" > \"{{results_file}}\"",
            cov_src.to_string_lossy().replace('/', "\\"),
            res_src.to_string_lossy().replace('/', "\\"),
        )
    } else {
        format!(
            "cp \"{}\" \"{{coverage_file}}\"; cat \"{}\" > \"{{results_file}}\"",
            cov_src.to_string_lossy(),
            res_src.to_string_lossy(),
        )
    };
    let plan = rust_plan(&cwd, template);

    let execution = execute_plan(&plan, &report_dir, foundation::cancel::CancelFlag::never())
        .await
        .expect("双工件替身命令应 Ok");

    assert_eq!(execution.exit_code, 0, "退出码直继承");
    assert!(
        execution.error.is_none(),
        "双工件齐备零 error 面，实际: {:?}",
        execution.error
    );
    assert!(
        report_dir.join("results.txt").is_file()
            && report_dir.join("coverage-summary.json").is_file(),
        "同一命令产出双工件（results.txt + coverage-summary.json）"
    );
    assert_eq!(execution.cases.len(), 4, "结果工件解析出四用例行");
    assert_eq!(
        execution.source_files.len(),
        2,
        "覆盖工件解析出两文件覆盖原始计数"
    );
    let measured = execution.measured.expect("覆盖实测块在场");
    assert_eq!(measured.lines, Some(75.0), "lines 取 totals 实测");
    assert_eq!(measured.branches, None, "分支计数 0 → null 感知跳过");
    assert_eq!(measured.functions, Some(100.0), "functions 取 totals 实测");
}

/// 正向（解析面零改动锚）：结果工件含多个 `running` 块 + `ok` / `ignored` /
/// `FAILED` 行 → 用例行全量解析（真件 17 块形态的替身锚），失败用例的错误面自
/// `---- <name> stdout ----` 块回填。
#[tokio::test]
async fn 多running块结果工件解析_用例行全量且失败归因() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let res_src = preset(ws.path(), "multi-block-preset.txt", RUST_RESULTS_OUTPUT);

    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\"",
            res_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", res_src.to_string_lossy())
    };
    let plan = rust_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("rust-blocks"), foundation::cancel::CancelFlag::never())
        .await
        .expect("结果工件替身命令应 Ok");

    assert_eq!(execution.exit_code, 0);
    let passed = execution
        .cases
        .iter()
        .filter(|case| case.status == checks::model::TestCaseStatus::Passed)
        .count();
    let failed = execution
        .cases
        .iter()
        .filter(|case| case.status == checks::model::TestCaseStatus::Failed)
        .count();
    let skipped = execution
        .cases
        .iter()
        .filter(|case| case.status == checks::model::TestCaseStatus::Skipped)
        .count();
    assert_eq!(execution.cases.len(), 4, "两 running 块四用例行");
    assert_eq!(
        (passed, failed, skipped),
        (2, 1, 1),
        "状态计数逐档正确（ok / FAILED / ignored）"
    );
    let failing = execution
        .cases
        .iter()
        .find(|case| case.name == "it_fails")
        .expect("失败用例在场");
    assert_eq!(
        failing.error_message.as_deref(),
        Some("left == right"),
        "失败详情块回填错误消息"
    );
    assert!(
        failing
            .stack_trace
            .as_deref()
            .is_some_and(|trace| trace.contains("assertion failed")),
        "失败详情块全块入堆栈，实际: {:?}",
        failing.stack_trace
    );
    assert!(
        execution.error.is_none(),
        "退出 0 + 结果工件可解析 → 零 error 面，实际: {:?}",
        execution.error
    );
}

/// 异常（AC-5 退出码直继承）：替身命令先产双工件再以 3 退出 → `exit_code = 3`
/// 原样透传、error 面记因同值；已产工件的 `cases` / `measured` 照常解析（不被
/// 链式末段洗 0、不产假用例）。
#[tokio::test]
async fn 退出码直继承_产双工件后非零() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let res_src = preset(ws.path(), "red-results-preset.txt", RUST_RESULTS_OUTPUT);
    let cov_src = preset(ws.path(), "red-coverage-preset.json", LLVM_COV_OUTPUT);

    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{coverage_file}}\" & cmd /C type \"{}\" > \"{{results_file}}\" & cmd /C exit /b 3",
            cov_src.to_string_lossy().replace('/', "\\"),
            res_src.to_string_lossy().replace('/', "\\"),
        )
    } else {
        format!(
            "cp \"{}\" \"{{coverage_file}}\"; cp \"{}\" \"{{results_file}}\"; exit 3",
            cov_src.to_string_lossy(),
            res_src.to_string_lossy(),
        )
    };
    let plan = rust_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("rust-red"), foundation::cancel::CancelFlag::never())
        .await
        .expect("被测域失败走产出（不 Err）");

    assert_eq!(execution.exit_code, 3, "退出码直继承（不被前置段洗 0）");
    assert!(
        execution
            .error
            .as_deref()
            .is_some_and(|error| error.contains("非零退出") && error.contains('3')),
        "error 面记因退出码 3，实际: {:?}",
        execution.error
    );
    assert_eq!(
        execution.cases.len(),
        4,
        "已产结果工件照常解析（不产假用例）"
    );
    assert!(
        execution.measured.is_some(),
        "已产覆盖工件照常解析（覆盖维度不因非零退出丢失）"
    );
}

/// 异常（AC-5 假绿路径封闭）：llvm-cov 文件锁失败形态——只产结果工件、以 101
/// 退出（覆盖率工件缺失）→ `exit_code = 101`、error 记因并携输出尾段、
/// `measured = None`、`source_files` 为空；**`error.is_some()`**（「工件缺失而
/// error 为空」不可达，不产假绿）。
#[tokio::test]
async fn llvm_cov工件缺失且非零_error面记因不出假绿() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let res_src = preset(ws.path(), "lock-results-preset.txt", RUST_RESULTS_OUTPUT);

    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\" & 1>&2 echo error: failed to build archive: target\\llvm-cov-target\\x.rlib & cmd /C exit /b 101",
            res_src.to_string_lossy().replace('/', "\\"),
        )
    } else {
        format!(
            "cp \"{}\" \"{{results_file}}\"; echo \"error: failed to build archive: target/llvm-cov-target/x.rlib\" >&2; exit 101",
            res_src.to_string_lossy(),
        )
    };
    let plan = rust_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("rust-lock"), foundation::cancel::CancelFlag::never())
        .await
        .expect("被测域失败走产出（不 Err）");

    assert_eq!(execution.exit_code, 101, "cargo 退出码 101 直继承");
    let error = execution.error.expect("覆盖率工件缺失而 error 必须记因");
    assert!(
        error.contains("非零退出") && error.contains("101"),
        "error 面记因退出码，实际: {error}"
    );
    assert!(
        error.contains("failed to build archive"),
        "error 面携输出尾段（文件锁失败归因原料），实际: {error}"
    );
    assert!(
        execution.measured.is_none(),
        "覆盖率工件未产出 → 实测块 None（null 感知跳过）"
    );
    assert!(
        execution.source_files.is_empty(),
        "覆盖率工件未产出 → 逐文件覆盖为空（不虚构条目）"
    );
    assert_eq!(
        execution.cases.len(),
        4,
        "已产结果工件照常解析（不产假用例）"
    );
}

/// 边界（参数化 5 值）：退出码谱 `0` / `1` / `3` / `7` / `255` 逐一透传——
/// 非零值 `exit_code` 等值且 error 记因同值，`0` 形态零 error。
#[tokio::test]
async fn 退出码谱边界_逐一透传与零error() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let res_src = preset(ws.path(), "spectrum-results-preset.txt", RUST_RESULTS_OUTPUT);

    // 零形态：产结果工件 → 零 error（无工件缺失噪音）
    let green = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\"",
            res_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", res_src.to_string_lossy())
    };
    let plan = rust_plan(&cwd, green);
    let execution = execute_plan(&plan, &ws.path().join("reports").join("spec-0"), foundation::cancel::CancelFlag::never())
        .await
        .expect("零形态应 Ok");
    assert_eq!(execution.exit_code, 0, "退出码 0 透传");
    assert!(
        execution.error.is_none(),
        "退出 0 + 工件可解析 → 零 error，实际: {:?}",
        execution.error
    );

    // 非零谱：1 / 3 / 7 / 255 逐值等值透传（首 token 须为 PATH 上可解析程序，
    // 前置检查面故经 `true;` 承接退出码）
    for code in [1, 3, 7, 255] {
        let template = if cfg!(windows) {
            format!("cmd /C exit /b {code}")
        } else {
            format!("true; exit {code}")
        };
        let plan = rust_plan(&cwd, template);
        let execution = execute_plan(&plan, &ws.path().join("reports").join(format!("spec-{code}")), foundation::cancel::CancelFlag::never())
            .await
            .expect("非零退出走产出（不 Err）");
        assert_eq!(execution.exit_code, code, "退出码 {code} 直继承");
        assert!(
            execution
                .error
                .as_deref()
                .is_some_and(|error| error.contains("非零退出")
                    && error.contains(&code.to_string())),
            "error 面记因退出码 {code}，实际: {:?}",
            execution.error
        );
    }
}

/// 边界（表驱动 2 形态）：命令成功但结果工件异常——空工件 → error 含「结果
/// 工件为空」；工件完全缺失 → error 含「结果工件缺失」；两者 `cases` 为空且
/// 不 panic。
#[tokio::test]
async fn 结果工件空与缺失_error面分化() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let empty_src = preset(ws.path(), "empty-preset.txt", "");

    // 形态一：命令成功但结果工件为空
    let empty = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\"",
            empty_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", empty_src.to_string_lossy())
    };
    let plan = rust_plan(&cwd, empty);
    let execution = execute_plan(&plan, &ws.path().join("reports").join("empty-results"), foundation::cancel::CancelFlag::never())
        .await
        .expect("空工件形态应 Ok");
    assert_eq!(execution.exit_code, 0);
    assert!(
        execution
            .error
            .as_deref()
            .is_some_and(|error| error.contains("结果工件为空")),
        "空工件 → error 记因「结果工件为空」，实际: {:?}",
        execution.error
    );
    assert!(execution.cases.is_empty(), "空工件零用例（不产假用例）");

    // 形态二：命令成功但结果工件完全缺失
    let missing = if cfg!(windows) {
        "cmd /C exit /b 0".to_owned()
    } else {
        "true".to_owned()
    };
    let plan = rust_plan(&cwd, missing);
    let execution = execute_plan(&plan, &ws.path().join("reports").join("missing-results"), foundation::cancel::CancelFlag::never())
        .await
        .expect("缺工件形态应 Ok");
    assert_eq!(execution.exit_code, 0);
    assert!(
        execution
            .error
            .as_deref()
            .is_some_and(|error| error.contains("结果工件缺失")),
        "缺工件 → error 记因「结果工件缺失」，实际: {:?}",
        execution.error
    );
    assert!(execution.cases.is_empty(), "缺工件零用例（不产假用例）");
    assert!(execution.measured.is_none(), "覆盖工件同缺 → 实测块 None");
}

/// 边界（R5 红跑 / 未产覆盖形态）：退出 0 但仅产结果工件 → `measured = None`、
/// `source_files` 为空、`cases` 正常解析、`error = None`（null 感知跳过——非假绿
/// 亦非误红）。
#[tokio::test]
async fn 退出零但覆盖率工件缺失_null感知跳过() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let res_src = preset(ws.path(), "no-cov-results-preset.txt", RUST_RESULTS_OUTPUT);

    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\"",
            res_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", res_src.to_string_lossy())
    };
    let plan = rust_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("rust-no-cov"), foundation::cancel::CancelFlag::never())
        .await
        .expect("仅产结果工件应 Ok");

    assert_eq!(execution.exit_code, 0, "退出码透传");
    assert!(
        execution.error.is_none(),
        "覆盖率工件缺失按 null 感知跳过（非 error 面），实际: {:?}",
        execution.error
    );
    assert_eq!(execution.cases.len(), 4, "结果工件照常解析四用例行");
    assert!(
        execution.measured.is_none(),
        "覆盖维度 None（判定侧 null 感知跳过）"
    );
    assert!(
        execution.source_files.is_empty(),
        "逐文件覆盖为空（不虚构条目）"
    );
}

/// 边界（AC-7 runner 侧锚）：jest 档手工 `TestPlan` 走既有 spawn 解析面——行为
/// 与改造前一致：istanbul 族结果工件不产用例行（零用例零 error），实测块读
/// 覆盖工件。
#[tokio::test]
async fn jest档执行面零改动_istanbul结果不产用例行() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let cov_src = preset(ws.path(), "jest-coverage-preset.json", ISTANBUL_SUMMARY);
    let res_src = preset(ws.path(), "jest-results-preset.json", JEST_RESULTS_JSON);

    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{coverage_file}}\" & cmd /C type \"{}\" > \"{{results_file}}\"",
            cov_src.to_string_lossy().replace('/', "\\"),
            res_src.to_string_lossy().replace('/', "\\"),
        )
    } else {
        format!(
            "cp \"{}\" \"{{coverage_file}}\"; cp \"{}\" \"{{results_file}}\"",
            cov_src.to_string_lossy(),
            res_src.to_string_lossy(),
        )
    };
    let plan = jest_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("it_jest"), foundation::cancel::CancelFlag::never())
        .await
        .expect("jest 档替身命令应 Ok");

    assert_eq!(execution.exit_code, 0, "退出码透传");
    assert!(
        execution.error.is_none(),
        "jest 档非空结果工件零 error，实际: {:?}",
        execution.error
    );
    assert!(
        execution.cases.is_empty(),
        "istanbul 族结果为 JSON 报告形态（V1 用例行面不解析）"
    );
    let measured = execution.measured.expect("istanbul 实测块在场");
    assert_eq!(measured.lines, Some(88.5));
    assert_eq!(measured.branches, Some(70.0));
    assert_eq!(measured.functions, Some(91.0));
}

/// 异常：裸名程序不可达 → Err 显式记因（程序解析前置——shell 吞缺失不产假
/// 产出，复用 static_check 经验）。
#[tokio::test]
async fn 裸名程序不可达err显式() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let plan = node_test_plan(&cwd, "no-such-test-runner-xyz --run {files}".to_owned());

    let err = execute_plan(&plan, &ws.path().join("reports").join("p"), foundation::cancel::CancelFlag::never())
        .await
        .expect_err("不可达程序应 Err（基础设施失败显式停给用户）");
    assert!(
        err.contains("no-such-test-runner-xyz") && err.contains("未找到"),
        "Err 记因裸名与解析失败面，实际: {err}"
    );
}

/// 异常：相对 cwd 程序不可达 → Err 记因「相对 suite cwd 不存在」（带路径
/// 分隔符的命令查相对 spawn cwd 的可达性口径）。
#[tokio::test]
async fn 相对cwd程序不可达err显式() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "tools/absent-runner.cmd --run {files}".to_owned()
    } else {
        "tools/absent-runner --run {files}".to_owned()
    };
    let plan = node_test_plan(&cwd, template);

    let err = execute_plan(&plan, &ws.path().join("reports").join("p"), foundation::cancel::CancelFlag::never())
        .await
        .expect_err("相对 cwd 不可达应 Err");
    assert!(
        err.contains("不可达") && err.contains("相对 suite cwd"),
        "Err 记因相对路径解析面，实际: {err}"
    );
}

/// 异常：命令非零退出且工件不可解析 → Ok 携 error 面（Err 与 conclusion=
/// error 两态分立——被测域失败走产出供反馈边，基础设施失败才 Err）。
#[tokio::test]
async fn 非零退出且工件不可解析_error面产出() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "cmd /C exit /b 3".to_owned()
    } else {
        "false".to_owned()
    };
    let plan = node_test_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("p"), foundation::cancel::CancelFlag::never())
        .await
        .expect("被测域失败走产出（不 Err）");

    assert_eq!(execution.exit_code, 3, "退出码透传");
    assert!(
        execution
            .error
            .as_deref()
            .is_some_and(|error| error.contains("非零退出") && error.contains("3")),
        "error 面记因非零退出，实际: {:?}",
        execution.error
    );
    assert!(execution.cases.is_empty(), "零工件零用例（不产假用例）");
    assert!(execution.measured.is_none());
    assert!(
        execution.output.is_empty(),
        "零输出命令 → output 空（合并流逐字，不虚构内容）"
    );
}

/// 异常：非零退出且 stderr 携进程报错文本 → 错误面携带该文本
#[tokio::test]
async fn 非零退出错误面携带进程报错文本() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "cmd /C \"echo compile error detail>&2 & exit /b 7\"".to_owned()
    } else {
        "echo compile error detail >&2; exit 7".to_owned()
    };
    let plan = node_test_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("p"), foundation::cancel::CancelFlag::never())
        .await
        .expect("被测域失败走产出（不 Err）");

    assert_eq!(execution.exit_code, 7, "退出码透传");
    let error = execution.error.expect("非零退出错误面在场");
    assert!(
        error.contains("非零退出") && error.contains("7"),
        "错误面记因退出码，实际: {error}"
    );
    assert!(
        error.contains("compile error detail"),
        "错误面携带进程自身报错文本（stderr 捕获面），实际: {error}"
    );
    // output 独立承载合并流：stderr 文本随 output 全量在场，error 面不外溢进 output
    let merged = String::from_utf8_lossy(&execution.output).into_owned();
    assert!(
        merged.contains("compile error detail"),
        "output 承载 stderr 合并流，实际: {merged:?}"
    );
    assert!(
        !merged.contains("非零退出"),
        "output 不并入 error 尾部语义，实际: {merged:?}"
    );
}

/// 单元：非零退出错误面拼装——错误信息 + 输出末尾双段（exec `ProcessOutput`
/// 统一装配），空输出回落裸退出码措辞。
#[test]
fn 非零退出错误面_双段拼装与空回落() {
    assert_eq!(
        non_zero_exit_error(3, b""),
        "测试命令非零退出（3）",
        "空输出回落裸退出码措辞"
    );
    assert_eq!(
        non_zero_exit_error(7, b"error: could not compile\n"),
        "测试命令非零退出（7）\n错误信息：\nerror: could not compile\n输出末尾（50 行）：\nerror: could not compile",
        "错误信息段在前、输出末尾段在后"
    );
    assert_eq!(
        non_zero_exit_error(7, b"plain output line\n"),
        "测试命令非零退出（7）\n输出末尾（50 行）：\nplain output line",
        "无错误模式命中时仅输出末尾段"
    );
}

/// 边界：命令成功退出但零工件（未写 results / coverage）→ 工件解析错误面
///（不 panic 不误判绿——error 面供诊断归因）。
#[tokio::test]
async fn 成功退出零工件_解析错误面() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "cmd /C exit /b 0".to_owned()
    } else {
        "true".to_owned()
    };
    let plan = node_test_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("p"), foundation::cancel::CancelFlag::never())
        .await
        .expect("成功退出应 Ok");

    assert_eq!(execution.exit_code, 0);
    assert!(
        execution
            .error
            .as_deref()
            .is_some_and(|error| error.contains("结果工件缺失")),
        "零工件 → 解析错误面（不误判绿），实际: {:?}",
        execution.error
    );
    assert!(execution.cases.is_empty() && execution.measured.is_none());
}

/// 边界：超时常量锚定——suite 命令 **1200s**（rust suite 实测 340–393 s × 约
/// 3 倍余量；原 600 s 仅 1.7 倍，CLI `runCommand(…, 600000)` 同值锚定口径退役）
/// / 版本探测 30s 不变（未涉及面零变）。
#[test]
fn 超时常量锚定() {
    assert_eq!(
        SUITE_COMMAND_TIMEOUT,
        Duration::from_secs(1200),
        "suite 命令 1200s（实测 340–393 s 的约 3 倍余量）"
    );
    assert_eq!(
        VERSION_DETECT_TIMEOUT,
        Duration::from_secs(30),
        "CLI 版本探测 timeout 30_000 同值"
    );
}

/// 边界：退出码缺码哨兵锚定——`-1` 与超时强杀同值（非零 → 子报告
/// `execution_error` 显式失败，MUST NOT 回落 `0` 造出假绿）。真实缺码面仅
/// Unix 信号终止可达（Windows `ExitStatus::code()` 恒 `Some`，`unwrap_or` 臂
/// 不可触达），故按值锚定哨兵。
#[test]
fn 退出码缺码哨兵锚定_负一与超时同值() {
    assert_eq!(
        EXIT_CODE_NO_STATUS, -1,
        "缺码哨兵 -1（非零 → error 面显式失败）"
    );
}

// ---------------------------------------------------------------------------
// PlanExecution.output（脚本真实输出合并流）：stdout 字节前缀 + stderr 字节后缀，
// 逐字，**不加 `2>&1`**——rust 模板
// `cargo llvm-cov --workspace --json --output-path "{coverage_file}" >
// "{results_file}"` 已把 stdout 重定向进结果工件，`2>&1` 会把 stderr 混入污染
// `parse_cargo_test`。手工拼接只服务 record 回查，结果工件解析仍只读 stdout 面；
// `error` 面仍经合并文本派生。
// ---------------------------------------------------------------------------

/// 正向：spawn 成功 → `PlanExecution.output` = `stdout_bytes` 前缀 + `stderr_bytes`
/// 后缀的合并流（逐字，不加 `2>&1`）；工件解析面不受合并流影响（stdout 单独
/// 落工件语义保持）。
#[tokio::test]
async fn 合并流_stdout前缀stderr后缀逐字() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "cmd /C echo stdout-marker& 1>&2 echo stderr-marker".to_owned()
    } else {
        "echo stdout-marker; echo stderr-marker >&2".to_owned()
    };
    let plan = node_test_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("m"), foundation::cancel::CancelFlag::never())
        .await
        .expect("绿跑形态应 Ok");

    assert_eq!(execution.exit_code, 0, "退出码透传");
    let merged = String::from_utf8_lossy(&execution.output).replace("\r\n", "\n");
    assert_eq!(
        merged, "stdout-marker\nstderr-marker\n",
        "output = stdout 前缀 + stderr 后缀逐字拼接（非 2>&1 交错，序恒前缀-后缀）"
    );
}

/// 正向：`output` 独立承载合并流——`error` 仍经 `output_text` 派生（携合并输出
/// 有界尾部），`output` 字段不并入 error 尾部语义。
#[tokio::test]
async fn 合并流独立承载_error经合并文本派生() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "cmd /C echo artifact-stdout& 1>&2 echo compile-error& exit /b 7".to_owned()
    } else {
        "echo artifact-stdout; echo compile-error >&2; exit 7".to_owned()
    };
    let plan = node_test_plan(&cwd, template);

    let execution = execute_plan(&plan, &ws.path().join("reports").join("e"), foundation::cancel::CancelFlag::never())
        .await
        .expect("被测域失败走产出（不 Err）");

    assert_eq!(execution.exit_code, 7, "退出码透传");
    let merged = String::from_utf8_lossy(&execution.output).replace("\r\n", "\n");
    assert_eq!(
        merged, "artifact-stdout\ncompile-error\n",
        "output 逐字承载合并流（stdout 段 + stderr 段）"
    );
    let error = execution.error.expect("非零退出错误面在场");
    assert!(
        error.contains("非零退出") && error.contains("7"),
        "error 面记因退出码，实际: {error}"
    );
    assert!(
        error.contains("artifact-stdout") && error.contains("compile-error"),
        "error 面经合并文本派生（有界尾部），实际: {error}"
    );
    assert!(
        !merged.contains("非零退出"),
        "output 不并入 error 尾部语义，实际: {merged:?}"
    );
}

/// 边界：rust 档合并流语义保持（模板自带重定向、runner 零改写）——绿跑臂
/// stdout 进结果工件、`output` 只余 stderr（编译 / `info:` 面）；红跑臂 stderr
/// 驱动 `error` 面记因，但不倒灌 `output`（`output` 仍逐字只承载脚本真实输出）。
#[tokio::test]
async fn 合并流语义保持_stdout进工件而output只载stderr() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let res_src = preset(ws.path(), "stream-results-preset.txt", RUST_RESULTS_OUTPUT);

    // 绿跑臂：stdout 经模板 `>` 进工件、stderr 留捕获面
    let green = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\" & 1>&2 echo build-error",
            res_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!(
            "cp \"{}\" \"{{results_file}}\"; echo build-error >&2",
            res_src.to_string_lossy()
        )
    };
    let plan = rust_plan(&cwd, green);
    let report_dir = ws.path().join("reports").join("rust-stream");
    let execution = execute_plan(&plan, &report_dir, foundation::cancel::CancelFlag::never())
        .await
        .expect("绿跑臂应 Ok");

    assert_eq!(execution.exit_code, 0, "退出码透传");
    let results = std::fs::read_to_string(report_dir.join("results.txt"))
        .expect("结果工件落报告树（模板自带重定向面）");
    assert_eq!(
        artifact_text(&results),
        artifact_text(RUST_RESULTS_OUTPUT),
        "stdout 全量进结果工件（解析面单读面保持）"
    );
    let merged = String::from_utf8_lossy(&execution.output).replace("\r\n", "\n");
    assert_eq!(
        merged, "build-error\n",
        "output 只载 stderr（编译错误面），stdout 已进工件不进捕获面"
    );

    // 红跑臂：stderr 驱动 error 面（合并文本派生），output 不倒灌 error 文案
    let red = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{results_file}}\" & 1>&2 echo compile-error& cmd /C exit /b 7",
            res_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!(
            "cp \"{}\" \"{{results_file}}\"; echo compile-error >&2; exit 7",
            res_src.to_string_lossy()
        )
    };
    let plan = rust_plan(&cwd, red);
    let execution = execute_plan(&plan, &ws.path().join("reports").join("rust-stream-red"), foundation::cancel::CancelFlag::never())
        .await
        .expect("红跑臂走产出（不 Err）");

    assert_eq!(execution.exit_code, 7, "退出码透传（不被前置段洗 0）");
    let error = execution.error.expect("非零退出错误面在场");
    assert!(
        error.contains("非零退出") && error.contains('7'),
        "error 面记因退出码，实际: {error}"
    );
    assert!(
        error.contains("compile-error"),
        "error 面经合并文本派生（携 stderr 尾段），实际: {error}"
    );
    let merged = String::from_utf8_lossy(&execution.output).replace("\r\n", "\n");
    assert_eq!(
        merged, "compile-error\n",
        "output 逐字只承载脚本真实输出（stderr），不并入 error 文案"
    );
    assert!(
        !merged.contains("非零退出"),
        "output 不并入 error 尾部语义，实际: {merged:?}"
    );
}

/// 边界：空 stdout + 空 stderr → `output` 空；大输出（>1000 bytes）→ `output`
/// 全量承载不截断（stdout 长段前缀 + stderr 尾段后缀）。
#[tokio::test]
async fn 边界_空输出与大输出全量承载() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let quiet = if cfg!(windows) {
        "cmd /C exit /b 0".to_owned()
    } else {
        "true".to_owned()
    };
    let plan = node_test_plan(&cwd, quiet);
    let execution = execute_plan(&plan, &ws.path().join("reports").join("empty"), foundation::cancel::CancelFlag::never())
        .await
        .expect("静默命令应 Ok");
    assert_eq!(
        execution.output,
        Vec::<u8>::new(),
        "空 stdout + 空 stderr → output 空"
    );

    let long = "x".repeat(1500);
    let verbose = if cfg!(windows) {
        format!("cmd /C echo {long}& 1>&2 echo tail-error")
    } else {
        format!("echo {long}; echo tail-error >&2")
    };
    let plan = node_test_plan(&cwd, verbose);
    let execution = execute_plan(&plan, &ws.path().join("reports").join("long"), foundation::cancel::CancelFlag::never())
        .await
        .expect("大输出形态应 Ok");

    let merged = String::from_utf8_lossy(&execution.output).into_owned();
    assert!(
        merged.len() > 1500,
        "大输出全量承载不截断，实际 {} 字节",
        merged.len()
    );
    assert!(merged.contains(&long), "1500 字符 stdout 段完整保留");
    assert!(
        merged.trim_end().ends_with("tail-error"),
        "stderr 尾段随合并流在场，实际尾部: {:?}",
        merged.chars().rev().take(20).collect::<String>()
    );
    assert!(
        merged.find(&long).expect("stdout 段在场")
            < merged.find("tail-error").expect("stderr 段在场"),
        "合并序恒 stdout 前缀 → stderr 后缀"
    );

    // rust 档形态：1500 字符 stdout 全量落结果工件（模板 `>` 重定向不截断），
    // stderr 尾段随 `output` 承载（序恒前缀 → 后缀）
    let verbose_rust = if cfg!(windows) {
        format!("cmd /C echo {long} > \"{{results_file}}\" & 1>&2 echo tail-error")
    } else {
        format!("echo {long} > \"{{results_file}}\"; echo tail-error >&2")
    };
    let plan = rust_plan(&cwd, verbose_rust);
    let report_dir = ws.path().join("reports").join("long-rust");
    let execution = execute_plan(&plan, &report_dir, foundation::cancel::CancelFlag::never())
        .await
        .expect("rust 大输出形态应 Ok");
    let results = std::fs::read_to_string(report_dir.join("results.txt"))
        .expect("大输出结果工件落报告树");
    assert_eq!(
        artifact_text(&results),
        format!("{long}\n"),
        "1500 字符 stdout 全量落工件（不截断）"
    );
    let merged = String::from_utf8_lossy(&execution.output).replace("\r\n", "\n");
    assert_eq!(
        merged, "tail-error\n",
        "stderr 尾段随 output 承载（stdout 已进工件）"
    );
}

// ---------------------------------------------------------------------------
// 超时 / 取消（test-design「testexec/runner.rs -> runner_test.rs」节，AC-3 /
// AC-4）：`SUITE_COMMAND_TIMEOUT` 预算到期 → `exit_code = -1` + 超时文案；
// 取消 → `Err("执行已取消")` 且不落 `execution_error` 语义（不写 summary /
// 子报告）；入口短路零 spawn。
// ---------------------------------------------------------------------------

use std::time::Instant;

use foundation::cancel::CancelFlag;

/// 自旋 suite 命令串（平台壳 `script_raw` 展开；先落自身 pid 再恒活，供超时 /
/// 取消臂观测「进程已建立且未退出」与被杀后的 pid 回收）。
///
/// Windows 臂的直接子进程是 `cmd.exe`（无内建 pid 变量），故委派 PowerShell
/// 自旋壳记录自身 winpid——`taskkill /T /F` 整树击杀后该 pid 必消失（整树语义
/// 的可稳定观测面）；unix 臂直接以 `sh` 进程 pid 为直接子进程面（`kill -9`）。
fn spin_template(cwd: &Path, pid_file: &Path) -> String {
    #[cfg(windows)]
    {
        let script = cwd.join("spin-suite.ps1");
        std::fs::write(
            &script,
            "param([string]$PidFile)\n\
             Set-Content -Path $PidFile -Value $PID -NoNewline -Encoding ascii\n\
             while ($true) { Start-Sleep -Milliseconds 100 }\n",
        )
        .expect("写自旋 PowerShell 壳失败");
        format!(
            "cmd /C powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\" \"{}\"",
            script.display(),
            pid_file.display()
        )
    }
    #[cfg(not(windows))]
    {
        let _ = cwd;
        format!(
            "sh -c 'printf %s $$ > \"{}\"; while :; do sleep 0.1; done'",
            pid_file.display()
        )
    }
}

/// 自旋 shim 落盘的 pid（winpid 口径——`taskkill` / `kill -0` 可探测）。
fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse().ok())
}

/// 等自旋 shim 落 pid（真实时钟握手：虚拟时钟下进程启动不受影响，等待须以
/// 真实睡眠 + 让出表达）。
async fn wait_pid(path: &Path, budget: Duration) -> u32 {
    let deadline = Instant::now() + budget;
    loop {
        if let Some(pid) = read_pid(path) {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "等待自旋 suite 落 pid 超时（{budget:?}）"
        );
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(10));
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
fn wait_pid_gone(pid: u32, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !pid_alive(pid)
}

/// 正向（AC-3）：超时臂——自旋 suite + 虚拟时钟推进越过 `SUITE_COMMAND_TIMEOUT`
/// → `exit_code == -1`（`EXIT_CODE_NO_STATUS` 哨兵）、`error` 含「测试命令超时
/// （1200 秒）强制终止」、`output` 为空、墙钟上界（不悬挂）+ 目标 pid 消失
///（`kill_tree` 生效锚：pid 先落盘 = 进程确已建立，收口后必消失）。
#[tokio::test(start_paused = true)]
async fn 超时臂_负一哨兵与超时文案且目标pid消失() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let pid_file = ws.path().join("suite-pid.txt");
    let plan = node_test_plan(&cwd, spin_template(&cwd, &pid_file));
    let report_dir = ws.path().join("reports").join("it_node-test");

    // 运行体在任务内（主任务负责「等 pid 落盘 → 推进虚拟时钟」的握手）
    let runner = tokio::spawn(async move { execute_plan(&plan, &report_dir, CancelFlag::never()).await });
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;

    let started = Instant::now();
    tokio::time::advance(SUITE_COMMAND_TIMEOUT + Duration::from_secs(60)).await;
    let execution = runner
        .await
        .expect("execute_plan 任务应正常结束")
        .expect("超时走产出（不 Err）");
    let elapsed = started.elapsed();

    assert_eq!(execution.exit_code, -1, "超时记 EXIT_CODE_NO_STATUS 哨兵");
    let error = execution.error.as_deref().unwrap_or_default();
    assert!(
        error.contains("测试命令超时（1200 秒）强制终止"),
        "超时文案（预算秒数由常量拼装）: {error}"
    );
    assert!(execution.output.is_empty(), "超时臂不消费输出");
    assert!(
        elapsed < Duration::from_secs(120),
        "墙钟上界（不悬挂，实际 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "超时强杀后目标 pid {pid} 应消失（kill_tree 整树回收）"
    );
}

/// 正向（AC-4）：已置位 cancel 的入口短路——直接 `Err("执行已取消")`，零 spawn
///（shim 无痕迹），且不落 `execution_error` 语义（不写 summary / 子报告）。
#[tokio::test]
async fn 已置位cancel_入口短路零spawn() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let marker = ws.path().join("suite-ran.txt");
    #[cfg(windows)]
    let template = format!(
        "cmd /C \"echo ran > \\\"{}\\\"\"",
        marker.display()
    );
    #[cfg(not(windows))]
    let template = format!("sh -c 'printf ran > \"{}\"'", marker.display());
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let cancel = CancelFlag::never();
    cancel.handle().set();
    let err = execute_plan(&plan, &report_dir, cancel)
        .await
        .expect_err("取消应 Err（不落 execution_error 语义）");
    assert_eq!(err, "执行已取消", "取消文案（过渡语义）");
    assert!(!marker.exists(), "入口短路零 spawn（shim 零执行）");
    assert!(
        !report_dir.join("summary.json").exists(),
        "取消臂不写 summary（不走子报告写面）"
    );
}

/// 正向（AC-4）：运行中 cancel——自旋 suite 落 pid 后挂起，运行中置位 →
/// `Err("执行已取消")`（取消不走 `execution_error` 语义）+ 目标 pid 消失 +
/// 墙钟上界（秒级收敛，不等 `SUITE_COMMAND_TIMEOUT`）+ 报告目录零 summary /
/// 子报告写入（中断面不落部分结论）。
#[tokio::test]
async fn 运行中cancel_秒级收敛且pid消失零报告写入() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let pid_file = ws.path().join("suite-pid-running.txt");
    let plan = node_test_plan(&cwd, spin_template(&cwd, &pid_file));
    let report_dir = ws.path().join("reports").join("it_node-test");

    let cancel = CancelFlag::never();
    let handle = cancel.handle();
    let runner =
        tokio::spawn(async move { execute_plan(&plan, &report_dir, cancel).await });

    // 等 pid 落盘（进程已建立）后置位取消
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;
    let started = Instant::now();
    handle.set();

    let err = runner
        .await
        .expect("execute_plan 任务应正常结束")
        .expect_err("取消应 Err（不落 execution_error 语义）");
    let elapsed = started.elapsed();

    assert_eq!(err, "执行已取消", "取消文案（过渡语义）");
    assert!(
        elapsed < Duration::from_secs(30),
        "秒级收敛（不等 1200 s 预算，实际 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "取消强杀后目标 pid {pid} 应消失（kill_tree 整树回收）"
    );
    let reports = ws.path().join("reports").join("it_node-test");
    assert!(
        !reports.join("summary.json").exists() && !reports.join("report.json").exists(),
        "取消臂不落 summary / 子报告（不吞成部分结论）"
    );
}

/// 边界（AC-5 消费面锚）：双管道大块并发——suite 命令同时向 stdout 与 stderr
/// 各写 >64 KiB（超管道缓冲；单线程顺序读会互锁）→ `output` 全长拼接
/// 「stdout 前缀 + stderr 后缀」逐字节（不丢字节、顺序稳定）+ 墙钟上界
/// （`run_shell` 并发排水不互锁的执行体保证在消费面可观测）。
#[tokio::test]
async fn 双管道大块并发_output全长拼接且不互锁() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");

    // 两路各 80 KiB（> 64 KiB 管道缓冲）
    const CHUNK: usize = 80 * 1024;
    let out_src = ws.path().join("pipe-stdout.txt");
    let err_src = ws.path().join("pipe-stderr.txt");
    std::fs::write(&out_src, "O".repeat(CHUNK)).expect("写 stdout 大块源");
    std::fs::write(&err_src, "E".repeat(CHUNK)).expect("写 stderr 大块源");

    let template = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" & type \"{}\" 1>&2",
            out_src.display(),
            err_src.display()
        )
    } else {
        format!(
            "cat \"{}\"; cat \"{}\" >&2",
            out_src.display(),
            err_src.display()
        )
    };
    let plan = node_test_plan(&cwd, template);

    let started = Instant::now();
    let execution = execute_plan(&plan, &ws.path().join("reports").join("it_node-test"), CancelFlag::never())
        .await
        .expect("双管道并发应 Ok");
    let elapsed = started.elapsed();

    assert_eq!(execution.exit_code, 0, "退出码透传");
    assert_eq!(
        execution.output.len(),
        CHUNK * 2,
        "两路大块全长拼接（不丢字节——单路 {CHUNK} 字节）"
    );
    assert!(
        execution.output[..CHUNK].iter().all(|byte| *byte == b'O'),
        "stdout 段完整在前缀（{CHUNK} 字节）"
    );
    assert!(
        execution.output[CHUNK..].iter().all(|byte| *byte == b'E'),
        "stderr 段完整在后缀（{CHUNK} 字节）"
    );
    assert!(
        elapsed < Duration::from_secs(90),
        "并发排水不互锁（墙钟上界 {elapsed:?}）"
    );
}
