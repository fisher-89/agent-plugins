use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use super::{
    decide_argv, execute_plan, expand_template, line_budget, non_zero_exit_error, ArgvDecision,
    ExpandMode, EXIT_CODE_NO_STATUS, SUITE_COMMAND_TIMEOUT, VERSION_DETECT_TIMEOUT,
};
use crate::testexec::detect::{to_posix, TestPlan};
use checks::parser::CoverageFormat;
use orchestration::port::RunFault;

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
        omit_files_safe: true,
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
        omit_files_safe: true,
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
        omit_files_safe: true,
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
        _guard: crate::lock_test_env(),
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
        ExpandMode::WithFiles,
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
        ExpandMode::WithFiles,
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
    let template =
        "cargo llvm-cov --workspace --json --output-path \"{coverage_file}\" > \"{results_file}\"";
    let expanded = expand_template(
        template,
        "/ws/reports/crates_rust",
        "/ws/reports/crates_rust/results.txt",
        "/ws/reports/crates_rust/coverage-summary.json",
        None,
        &["tests/it.rs".to_owned()],
        ExpandMode::WithFiles,
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
        ExpandMode::WithFiles,
    );
    assert_eq!(expanded, "run --files tests/it.rs", "单元素清单逐字展开");

    // 形态二：大清单（50 项）逐字空格连接
    let many: Vec<String> = (0..50)
        .map(|index| format!("tests/t{index:02}.rs"))
        .collect();
    let expanded = expand_template(
        "run --files {files}",
        "/ws/reports/plan",
        "/ws/reports/plan/results.txt",
        "/ws/reports/plan/coverage-summary.json",
        None,
        &many,
        ExpandMode::WithFiles,
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
        ExpandMode::WithFiles,
    );
    assert_eq!(
        expanded,
        "a.rs b.rs | a.rs b.rs | /ws/reports/plan/results.txt | /ws/reports/plan/results.txt",
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
        ExpandMode::WithFiles,
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
        let err = execute_plan(
            &plan,
            &ws.path().join("reports").join("blank"),
            foundation::cancel::CancelFlag::never(),
        )
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
        ExpandMode::WithFiles,
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
        ExpandMode::WithFiles,
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("it_node-test"),
        foundation::cancel::CancelFlag::never(),
    )
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
    let res_src = preset(ws.path(), "rewrite-results-preset.txt", RUST_RESULTS_OUTPUT);
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
        ExpandMode::WithFiles,
    );
    let results_posix = to_posix(&report_dir.join("results.txt"));
    assert_eq!(
        expected.matches(&format!("> \"{results_posix}\"")).count(),
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("rust-blocks"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("rust-red"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("rust-lock"),
        foundation::cancel::CancelFlag::never(),
    )
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
    let res_src = preset(
        ws.path(),
        "spectrum-results-preset.txt",
        RUST_RESULTS_OUTPUT,
    );

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
    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("spec-0"),
        foundation::cancel::CancelFlag::never(),
    )
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
        let execution = execute_plan(
            &plan,
            &ws.path().join("reports").join(format!("spec-{code}")),
            foundation::cancel::CancelFlag::never(),
        )
        .await
        .expect("非零退出走产出（不 Err）");
        assert_eq!(execution.exit_code, code, "退出码 {code} 直继承");
        assert!(
            execution.error.as_deref().is_some_and(
                |error| error.contains("非零退出") && error.contains(&code.to_string())
            ),
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
        format!(
            "cp \"{}\" \"{{results_file}}\"",
            empty_src.to_string_lossy()
        )
    };
    let plan = rust_plan(&cwd, empty);
    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("empty-results"),
        foundation::cancel::CancelFlag::never(),
    )
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
    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("missing-results"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("rust-no-cov"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("it_jest"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let err = execute_plan(
        &plan,
        &ws.path().join("reports").join("p"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let err = execute_plan(
        &plan,
        &ws.path().join("reports").join("p"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("p"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("p"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("p"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("m"),
        foundation::cancel::CancelFlag::never(),
    )
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

    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("e"),
        foundation::cancel::CancelFlag::never(),
    )
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
    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("rust-stream-red"),
        foundation::cancel::CancelFlag::never(),
    )
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
    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("empty"),
        foundation::cancel::CancelFlag::never(),
    )
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
    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("long"),
        foundation::cancel::CancelFlag::never(),
    )
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
    let results =
        std::fs::read_to_string(report_dir.join("results.txt")).expect("大输出结果工件落报告树");
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
// AC-7）：`SUITE_COMMAND_TIMEOUT` 预算到期 → `exit_code = -1` +
// `fault = Some(Environment("测试命令超时（1200 秒）强制终止"))`；取消 →
// `fault = Some(RunFault::Stopped)` 的正常产出（**不 Err**——MUST NOT 返回
// `Err("执行已取消")`）且不落 plan 级 `execution_error` 语义（不写 summary /
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

/// 正向（AC-7 / D13）：超时臂——自旋 suite + 虚拟时钟推进越过
/// `SUITE_COMMAND_TIMEOUT` → `exit_code == -1`（`EXIT_CODE_NO_STATUS` 哨兵）、
/// `fault == Some(Environment("测试命令超时（1200 秒）强制终止"))`、`error` 恒
/// `None`（**不落** plan 级 `execution_error`）、墙钟上界 + 目标 pid 消失。
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
    let runner =
        tokio::spawn(async move { execute_plan(&plan, &report_dir, CancelFlag::never()).await });
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;

    let started = Instant::now();
    tokio::time::advance(SUITE_COMMAND_TIMEOUT + Duration::from_secs(60)).await;
    let execution = runner
        .await
        .expect("execute_plan 任务应正常结束")
        .expect("超时走产出（不 Err）");
    let elapsed = started.elapsed();

    assert_eq!(execution.exit_code, -1, "超时记 EXIT_CODE_NO_STATUS 哨兵");
    assert_eq!(
        execution.fault,
        Some(RunFault::Environment(
            "测试命令超时（1200 秒）强制终止".to_owned()
        )),
        "超时文案（预算秒数由常量拼装）由 fault 承载"
    );
    assert!(
        execution.error.is_none(),
        "超时不落 plan 级 execution_error（env 面恒 None）: {:?}",
        execution.error
    );
    assert!(
        execution.cases.is_empty() && execution.measured.is_none(),
        "fault 在位即跳过工件解析（不读陈旧工件）"
    );
    assert!(
        elapsed < Duration::from_secs(120),
        "墙钟上界（不悬挂，实际 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "超时强杀后目标 pid {pid} 应消失（kill_tree 整树回收）"
    );
}

/// 正向（AC-7）：已置位 cancel 的入口短路——返回 `Ok(PlanExecution)` 且
/// `fault == Some(Stopped)`（**不 Err**）、`error == None`、零 spawn（shim 无
/// 痕迹）、不写 summary / 子报告。
#[tokio::test]
async fn 已置位cancel_fault为stopped且零spawn() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let marker = ws.path().join("suite-ran.txt");
    #[cfg(windows)]
    let template = format!("cmd /C \"echo ran > \\\"{}\\\"\"", marker.display());
    #[cfg(not(windows))]
    let template = format!("sh -c 'printf ran > \"{}\"'", marker.display());
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let cancel = CancelFlag::never();
    cancel.handle().set();
    let execution = execute_plan(&plan, &report_dir, cancel)
        .await
        .expect("取消不 Err（产出携 fault）");
    assert_eq!(
        execution.fault,
        Some(RunFault::Stopped),
        "取消 → fault = Stopped（run 收敛 stopped）"
    );
    assert_eq!(execution.exit_code, -1, "取消记 -1 哨兵");
    assert!(execution.error.is_none(), "取消不落 execution_error");
    assert!(
        execution.cases.is_empty()
            && execution.source_files.is_empty()
            && execution.measured.is_none(),
        "取消面载荷为空（跳过工件解析）"
    );
    assert!(!marker.exists(), "入口短路零 spawn（shim 零执行）");
    assert!(
        !report_dir.join("summary.json").exists(),
        "取消臂不写 summary（不走子报告写面）"
    );
}

/// 正向（AC-7）：运行中 cancel——自旋 suite 落 pid 后挂起，运行中置位 →
/// `Ok(PlanExecution)` + `fault == Some(Stopped)`（**不 Err**、不走
/// `execution_error`）+ 目标 pid 消失 + 墙钟上界（秒级收敛）+ 报告目录零
/// summary / 子报告写入（中断面不落部分结论）。
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
    let runner = tokio::spawn(async move { execute_plan(&plan, &report_dir, cancel).await });

    // 等 pid 落盘（进程已建立）后置位取消
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;
    let started = Instant::now();
    handle.set();

    let execution = runner
        .await
        .expect("execute_plan 任务应正常结束")
        .expect("取消不 Err（产出携 fault）");
    let elapsed = started.elapsed();

    assert_eq!(
        execution.fault,
        Some(RunFault::Stopped),
        "运行中取消 → fault = Stopped"
    );
    assert!(execution.error.is_none(), "取消不落 execution_error");
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
    let execution = execute_plan(
        &plan,
        &ws.path().join("reports").join("it_node-test"),
        CancelFlag::never(),
    )
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

// ---------------------------------------------------------------------------
// 组合锚：瞬态重试 → fault 装配（AC-1 / AC-3 / AC-7）
// ---------------------------------------------------------------------------

/// 虚拟时钟闸门：以常驻 `spawn_blocking` 任务抑制 tokio 的「空闲即自动推进」
/// （沿 `exec::run_test::ClockGate` 同型装置）。暂停时钟下等待真实子进程会让虚拟
/// 时间成百倍空转，使「退避 1/2/4/8 s 恰不真等」「1500 s 预算不提前耗尽」
/// 「1200 s 超时臂不被误触」失去确定性；闸门在位时虚拟时间**只**随
/// [`advance_until`] 前进。
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

/// 重试退避不真等：按 `step` 粒度推进虚拟时钟直到 `until` 为真，真实时间上界
/// `real_budget`（虚拟推进 ≈ 10 × 真实耗时，故 60 s 真实上界对应 ≤ 600 s 虚拟，
/// 既不触 1200 s 超时臂也不触 1500 s 总预算）。
async fn advance_until<F: Fn() -> bool>(until: F, step: Duration, real_budget: Duration) {
    let deadline = Instant::now() + real_budget;
    while !until() && Instant::now() < deadline {
        tokio::time::advance(step).await;
        // 真实让出：子进程排水 / 收尸在真实时间上进行（虚拟时钟不参与）
        let _ = tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(10))).await;
    }
    assert!(
        until(),
        "推进真实预算 {real_budget:?} 后仍未收敛（暂停时钟下虚拟时间不会自行前进）"
    );
}

/// 耗尽模板（每行一条 shell 语句；输出锁文本 + 非零退出，重试面恒命中签名）。
fn lock_always_template(locked: &Path) -> String {
    let text = format!(
        "error: failed to rename archive file: {}: os error 32",
        locked.display()
    );
    if cfg!(windows) {
        format!("cmd /C echo {text}& exit /b 1")
    } else {
        format!("echo {text}; exit 1")
    }
}

/// 自愈 shim（平台双形态）：首次拉起落标记并输出锁文本 + 非零，之后零退出
/// 自愈 shim（平台双形态）：首次拉起落标记并输出锁文本 + 非零，之后零退出
///（尝试次数由标记文件自身表达）；返回可直接作模板的 shim 绝对路径（tempdir
/// 路径无空格，首 token 即程序）。
fn write_selfheal_shim(dir: &Path, marker: &Path, locked: &Path) -> String {
    #[cfg(windows)]
    {
        let path = dir.join("selfheal-shim.cmd");
        let body = format!(
            "@echo off\r\nif exist \"{marker}\" exit /b 0\r\necho. > \"{marker}\"\r\necho error: failed to rename archive file: {locked}: os error 32\r\nexit /b 1\r\n",
            marker = marker.display(),
            locked = locked.display(),
        );
        std::fs::write(&path, body).expect("写自愈 shim");
        path.display().to_string()
    }
    #[cfg(not(windows))]
    {
        let path = dir.join("selfheal-shim.sh");
        let body = format!(
            "#!/bin/sh\nif [ -f \"{marker}\" ]; then exit 0; fi\n: > \"{marker}\"\necho \"error: failed to rename archive file: {locked}: os error 32\"\nexit 1\n",
            marker = marker.display(),
            locked = locked.display(),
        );
        std::fs::write(&path, body).expect("写自愈 shim");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("补执行位");
        path.display().to_string()
    }
}

/// 正向（AC-1 / AC-3）：瞬态耗尽 → `fault = Environment` 且零 `execution_error`
/// ——suite 恒锁文本 + `exit 1`（`transient()` 5 次耗尽）→ `fault ==
/// Some(Environment(text))`（含点名 / 被锁文件 / 已自动重试 4 次 / 指引）、
/// `error == None`、`exit_code == 1`、`output` 含 5 段分隔头（序号 1/5..5/5）。
/// 退避（1+2+4+8 s）随虚拟时钟推进，不真等。
#[tokio::test(start_paused = true)]
async fn 瞬态耗尽_fault为环境错误且零execution_error() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let locked = cwd.join("liba.rlib");
    std::fs::write(&locked, b"body").expect("落盘被锁文件");
    let plan = node_test_plan(&cwd, lock_always_template(&locked));
    let report_dir = ws.path().join("reports").join("it_node-test");

    // 运行体在任务内（主任务负责推进虚拟时钟：4 段退避 15 s 不真等）
    let runner =
        tokio::spawn(async move { execute_plan(&plan, &report_dir, CancelFlag::never()).await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(60),
    )
    .await;
    let execution = runner
        .await
        .expect("suite 任务应正常结束")
        .expect("耗尽面仍 Ok（执行面归因走 fault）");

    assert_eq!(execution.exit_code, 1, "末次非零退出直继承");
    let Some(RunFault::Environment(text)) = execution.fault.clone() else {
        panic!(
            "瞬态耗尽应产 Environment fault，实际: {:?}",
            execution.fault
        );
    };
    assert!(
        text.contains("已自动重试 4 次"),
        "重试次数按 attempts - 1 计: {text}"
    );
    assert!(
        text.contains(&locked.to_string_lossy().into_owned()),
        "被锁文件路径在案: {text}"
    );
    assert!(
        text.contains("未定位占用者") || text.contains("占用进程："),
        "占用者点名或降级说明在案: {text}"
    );
    assert!(
        text.contains("workspace 父目录") && text.contains("逐 worktree"),
        "环境侧指引在案: {text}"
    );
    assert!(
        execution.error.is_none(),
        "fault 在位 → plan 级 execution_error 恒 None: {:?}",
        execution.error
    );
    assert!(
        execution.cases.is_empty()
            && execution.source_files.is_empty()
            && execution.measured.is_none(),
        "fault 在位即跳过工件解析"
    );

    assert!(
        !text.contains("重试进展"),
        "非 cargo 形态（全 0 计数）不产推进序列行: {text}"
    );
    let output = String::from_utf8_lossy(&execution.output).into_owned();
    assert_eq!(
        output.matches("===== attempt ").count(),
        5,
        "耗尽面 5 段分隔头: {output}"
    );
    for index in 1..=5 {
        assert!(
            output.contains(&format!("===== attempt {index}/5")),
            "序号 {index}/5 在案: {output}"
        );
    }
}

/// 正向（AC-1）：瞬态自愈 → 正常解析——前 1 次锁文本、第 2 次 `exit 0` 且产出
/// 工件 → `fault == None`、`error == None`、用例解析产出与单次绿跑一致、
/// `output` 含 2 段分隔头。退避（1 s）随虚拟时钟推进，不真等。
#[tokio::test(start_paused = true)]
async fn 瞬态自愈_fault为空且正常解析() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let marker = ws.path().join("ran-once.txt");
    let locked = cwd.join("liba.rlib");
    std::fs::write(&locked, b"body").expect("落盘被锁文件");
    // 自愈后仍无结果工件（node-test 档 results.txt 由模板重定向产出；此处只验证
    // 解析失败面与 fault 解耦）
    let plan = node_test_plan(&cwd, write_selfheal_shim(ws.path(), &marker, &locked));
    let report_dir = ws.path().join("reports").join("it_node-test");

    // 运行体在任务内（主任务推进虚拟时钟：1 s 退避不真等）
    let runner =
        tokio::spawn(async move { execute_plan(&plan, &report_dir, CancelFlag::never()).await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(60),
    )
    .await;
    let execution = runner
        .await
        .expect("suite 任务应正常结束")
        .expect("自愈面应 Ok");

    assert_eq!(execution.exit_code, 0, "第 2 次尝试零退出");
    assert_eq!(execution.fault, None, "自愈后不产 fault");
    assert!(
        execution.cases.is_empty(),
        "无结果工件 → 零用例行（解析面照旧）"
    );
    let output = String::from_utf8_lossy(&execution.output).into_owned();
    assert_eq!(
        output.matches("===== attempt ").count(),
        2,
        "自愈面 2 段分隔头: {output}"
    );
    assert!(
        output.contains("===== attempt 1/5 · exit=1 · transient=os error 32 ====="),
        "首段携命中签名: {output}"
    );
    assert!(
        output.contains("===== attempt 2/5 · exit=0 · transient=- ====="),
        "末段无签名: {output}"
    );
}

/// 边界（R-1）：裸 `failed to write` 不产 fault——`exit 1` + 输出仅该措辞（工件
/// 不可解析）→ `fault == None`、`error == Some(..)`、单次尝试（零分隔头）。
#[tokio::test]
async fn 裸failed_to_write不产fault且单次尝试() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let target = cwd.join("libb.rmeta");
    let template = if cfg!(windows) {
        format!(
            "cmd /C echo error: failed to write {}& exit /b 1",
            target.display()
        )
    } else {
        format!("echo error: failed to write {}; exit 1", target.display())
    };
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let execution = execute_plan(&plan, &report_dir, CancelFlag::never())
        .await
        .expect("非零退出仍 Ok");

    assert_eq!(execution.exit_code, 1);
    assert_eq!(
        execution.fault, None,
        "裸措辞不命中签名 → 不产 fault（真红归 error 面）"
    );
    assert!(
        execution
            .error
            .as_deref()
            .is_some_and(|error| error.contains("测试命令非零退出（1）")),
        "非零且无签名 → execution_error 在案: {:?}",
        execution.error
    );
    assert_eq!(
        String::from_utf8_lossy(&execution.output)
            .matches("===== attempt ")
            .count(),
        0,
        "单次尝试零分隔头"
    );
}

// ---------------------------------------------------------------------------
// 正向（AC-5 组合锚）：合并流推进序列到 fault
// （`runner::execute_plan` → `exec::run_shell` → `environment::fault_text`（合并流）
// → `PlanExecution.fault`）
// ---------------------------------------------------------------------------

/// 按尝试序输出 `Compiling` 行数的 suite 模板（平台双形态）：第 n 次尝试
/// **stdout** 输出 `counts[n - 1]` 行 `   Compiling crate v0.1.0`，**stderr**
/// 输出 `lock_text` 并 `exit 1`（合并流面：末次 `stderr` 已并入 `output`，故
/// `Compiling` 行不得被计两次）；超出清单的尝试输出 0 行。返回可直接作模板的
/// shim 绝对路径（tempdir 路径无空格，首 token 即程序）。
fn write_counting_template(
    dir: &Path,
    name: &str,
    counter: &Path,
    counts: &[u32],
    lock_text: &str,
) -> String {
    #[cfg(windows)]
    {
        let counts_array = counts
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let ps1 = dir.join(format!("{name}-shim.ps1"));
        let body = format!(
            "$cf = '{cf}'\n$n = 0\nif (Test-Path $cf) {{ $n = [int](Get-Content $cf -Raw) }}\n$n = $n + 1\nSet-Content -Path $cf -Value $n -NoNewline -Encoding ascii\n$counts = @({counts})\n$count = 0\nif ($n -le $counts.Count) {{ $count = $counts[$n - 1] }}\nfor ($i = 0; $i -lt $count; $i++) {{ Write-Output '   Compiling crate v0.1.0' }}\n[Console]::Error.WriteLine('{lock}')\nexit 1\n",
            cf = counter.display(),
            counts = counts_array,
            lock = lock_text,
        );
        std::fs::write(&ps1, body).expect("写计数 shim 脚本失败");
        let wrapper = dir.join(format!("{name}.cmd"));
        std::fs::write(
            &wrapper,
            format!(
                "@echo off\r\npowershell -NoProfile -ExecutionPolicy Bypass -File \"%~dp0{name}-shim.ps1\" %*\r\nexit /b %errorlevel%\r\n"
            ),
        )
        .expect("写计数 shim 失败");
        wrapper.display().to_string()
    }
    #[cfg(not(windows))]
    {
        let mut arms = String::new();
        for (index, count) in counts.iter().enumerate() {
            arms.push_str(&format!("  {}) count={count} ;;\n", index + 1));
        }
        let path = dir.join(format!("{name}.sh"));
        let body = format!(
            "#!/bin/sh\ncf=\"{cf}\"\nn=$(cat \"$cf\" 2>/dev/null || printf '0')\nn=$((n+1))\nprintf '%s' \"$n\" > \"$cf\"\ncount=0\ncase \"$n\" in\n{arms}esac\ni=0\nwhile [ \"$i\" -lt \"$count\" ]; do\n  printf '   Compiling crate v0.1.0\\n'\n  i=$((i+1))\ndone\nprintf '%s\\n' '{lock}' >&2\nexit 1\n",
            cf = counter.display(),
            arms = arms,
            lock = lock_text,
        );
        std::fs::write(&path, body).expect("写计数 shim 失败");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("补执行位");
        path.display().to_string()
    }
}

/// 正向（AC-5 组合锚）：合并流推进序列到 fault——suite 模板按尝试分支输出
/// 264 / 5 / 5 行 `Compiling`（stdout）+ 锁文本（stderr）→ 卡死早停 →
/// `fault == Some(Environment(text))`，`text` 含
/// `重试进展：待编译 264 → 5 → 5（末两次无进展，判定为卡死）`（末次 `stderr`
/// 已并入 `output`，不重复计数）；`output` 段数与 `attempts` 一致（3 段）。
#[tokio::test(start_paused = true)]
async fn 合并流推进序列到fault文本() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let locked = cwd.join("liba.rlib");
    std::fs::write(&locked, b"body").expect("落盘被锁文件");
    let lock_text = format!(
        "error: failed to rename archive file: {}: os error 32",
        locked.display()
    );
    let counter = ws.path().join("attempt-count.txt");
    let template =
        write_counting_template(ws.path(), "counting", &counter, &[264, 5, 5], &lock_text);
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let runner =
        tokio::spawn(async move { execute_plan(&plan, &report_dir, CancelFlag::never()).await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(120),
    )
    .await;
    let execution = runner
        .await
        .expect("suite 任务应正常结束")
        .expect("卡死面仍 Ok（执行面归因走 fault）");

    assert_eq!(execution.exit_code, 1, "末次非零退出直继承");
    let Some(RunFault::Environment(text)) = execution.fault.clone() else {
        panic!(
            "卡死形态应产 Environment fault，实际: {:?}",
            execution.fault
        );
    };
    assert!(
        text.contains("重试进展：待编译 264 → 5 → 5（末两次无进展，判定为卡死）"),
        "合并流序列行与判定语在案（无重复计数）: {text}"
    );
    assert!(
        text.contains("已自动重试 2 次"),
        "attempts == 3 → 重试 2 次: {text}"
    );
    assert!(
        execution.error.is_none(),
        "fault 在位 → plan 级 execution_error 恒 None: {:?}",
        execution.error
    );
    let output = String::from_utf8_lossy(&execution.output).into_owned();
    assert_eq!(
        output.matches("===== attempt ").count(),
        3,
        "output 段数与 attempts（3）一致: {output}"
    );
    for index in 1..=3 {
        assert!(
            output.contains(&format!("===== attempt {index}/5")),
            "序号 {index}/5 在案: {output}"
        );
    }
}

/// 正向（AC-5）：卡死判定语（两次尝试）——两次尝试同为 50 行 `Compiling` +
/// 锁文本 → `text` 含 `（末两次无进展，判定为卡死）`；`execution.output` 恰 2 段。
#[tokio::test(start_paused = true)]
async fn 两次相同计数产卡死判定语() {
    let _env = env_window();
    let _gate = ClockGate::new();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let locked = cwd.join("libc.rlib");
    std::fs::write(&locked, b"body").expect("落盘被锁文件");
    let lock_text = format!(
        "error: failed to rename archive file: {}: os error 32",
        locked.display()
    );
    let counter = ws.path().join("attempt-count.txt");
    let template = write_counting_template(ws.path(), "stall", &counter, &[50, 50], &lock_text);
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let runner =
        tokio::spawn(async move { execute_plan(&plan, &report_dir, CancelFlag::never()).await });
    advance_until(
        || runner.is_finished(),
        Duration::from_millis(100),
        Duration::from_secs(120),
    )
    .await;
    let execution = runner
        .await
        .expect("suite 任务应正常结束")
        .expect("卡死面仍 Ok");

    let Some(RunFault::Environment(text)) = execution.fault.clone() else {
        panic!(
            "卡死形态应产 Environment fault，实际: {:?}",
            execution.fault
        );
    };
    assert!(
        text.contains("重试进展：待编译 50 → 50（末两次无进展，判定为卡死）"),
        "两次相同计数 → 判定语: {text}"
    );
    assert_eq!(
        String::from_utf8_lossy(&execution.output)
            .matches("===== attempt ")
            .count(),
        2,
        "output 恰 2 段"
    );
    assert_eq!(execution.exit_code, 1, "末次非零退出直继承");
    assert!(
        execution.cases.is_empty() && execution.measured.is_none(),
        "fault 在位即跳过工件解析"
    );
}

/// 边界（AC-8）：`PlanExecution` 字段面零变——八字段（`exit_code` /
/// `duration_ms` / `cases` / `source_files` / `measured` / `error` / `output` /
/// `fault`）在绿跑面逐字段可读且形态同改造前（字段集无新增）。
#[tokio::test]
async fn plan_execution字段面零变() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let spec_src = preset(ws.path(), "preset-results.txt", SPEC_OUTPUT);
    let template = if cfg!(windows) {
        format!(
            "cmd /C type {} > {{results_file}}",
            spec_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", spec_src.to_string_lossy())
    };
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let execution = execute_plan(&plan, &report_dir, CancelFlag::never())
        .await
        .expect("绿跑应 Ok");

    assert_eq!(execution.exit_code, 0, "exit_code 字段可读");
    assert!(execution.duration_ms >= 0.0, "duration_ms 字段可读");
    assert_eq!(execution.cases.len(), 5, "cases 字段可读（五用例行）");
    assert!(execution.source_files.is_empty(), "source_files 字段可读");
    assert!(
        execution.measured.is_some(),
        "measured 字段可读（node-test 覆盖实测块）"
    );
    assert_eq!(execution.error, None, "error 字段可读（绿面无错）");
    assert_eq!(
        execution.output,
        Vec::<u8>::new(),
        "output 字段可读（模板把 stdout 重定向进工件 → 合并流为空）"
    );
    assert_eq!(execution.fault, None, "fault 字段可读（绿面无归因）");
}

// ---------------------------------------------------------------------------
// 命令行预算裁决（`decide_argv` / `line_budget` / `CMD_LINE_BUDGET`——
// test-exec-argv-budget，AC-1 / AC-2 / AC-4 / AC-5）：纯函数零 io 零 spawn
// 零 async，预算与展开串全作入参（平台无关），故本节点全部用例平台无关。
// ---------------------------------------------------------------------------

/// 裁决装置：plan 纯数据直构（模板 / 清单 / 降级安全位由用例给值；`expanded`
/// 亦作入参——`decide_argv` 只取展开串长度，`Omit` 臂再按 `plan.template()`
/// 重新展开）。
fn argv_plan(template: &str, files: &[&str], omit_files_safe: bool) -> TestPlan {
    TestPlan {
        id: "it_node-test".to_owned(),
        framework: "node-test".to_owned(),
        root: "app".to_owned(),
        cwd: PathBuf::from("app"),
        files: files.iter().map(|file| (*file).to_owned()).collect(),
        config_args: None,
        shell_template: template.to_owned(),
        cmd_template: template.to_owned(),
        coverage_format: CoverageFormat::NodeTest,
        coverage_output: "coverage-summary.json",
        results_output: "results.txt",
        omit_files_safe,
    }
}

/// 裁决入参固定路径（与 `execute_plan` 同口径的绝对 POSIX 三元组）。
const PLAN_REPORT_DIR: &str = "/ws/reports/it_node-test";
const PLAN_RESULTS_FILE: &str = "/ws/reports/it_node-test/results.txt";
const PLAN_COVERAGE_FILE: &str = "/ws/reports/it_node-test/coverage-summary.json";

/// 单点裁决调用（预算作入参 → 全平台可测）。
fn decide(plan: &TestPlan, expanded: &str, budget: Option<usize>) -> ArgvDecision {
    decide_argv(
        plan,
        PLAN_REPORT_DIR,
        PLAN_RESULTS_FILE,
        PLAN_COVERAGE_FILE,
        expanded,
        budget,
    )
}

/// 定长填充串（预算边界构造：把展开串恰好撑到 `target` 字符）。
fn filler(target: usize) -> String {
    "x".repeat(target)
}

/// 正向：未超预算 → `Keep`，`command` 与入参 `expanded` **逐字符相等**（原串
/// 回填、零二次展开——命令与改造前逐字符一致，AC-2）。
#[test]
fn 裁决_未超预算原串逐字符回填() {
    // 87 条文件清单形态的实测长度量级（4527 字符 << 7000 预算）
    let files: Vec<String> = (0..87)
        .map(|index| format!("src/case{index:02}.test.mjs"))
        .collect();
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    let plan = argv_plan("node --test {files}", &refs, true);
    let expanded = filler(4527);
    assert_eq!(expanded.chars().count(), 4527, "构造恰 4527 字符");

    let ArgvDecision::Keep { command } = decide(&plan, &expanded, Some(7000)) else {
        panic!("未超预算应落 Keep 臂");
    };
    assert_eq!(command, expanded, "原串回填（逐字符相等）");
    assert_eq!(command.chars().count(), 4527, "长度不变（零二次展开）");
    assert!(
        !command.contains("{files}"),
        "Keep 臂不做占位符再展开: {command}"
    );
}

/// 边界（表驱动）：预算边界口径——`len == budget` 与 `len == budget - 1`
/// 不降级；`len == budget + 1` 进裁决（可安全降级 → `Omit`、不可降级 →
/// `Reject`）。
#[test]
fn 裁决_预算边界等值不降级加一进裁决() {
    let safe = argv_plan("run {files}", &["src/a.test.mjs"], true);

    let ArgvDecision::Keep { command } = decide(&safe, &filler(7000), Some(7000)) else {
        panic!("len == 预算 → Keep（不降级）");
    };
    assert_eq!(command.chars().count(), 7000, "恰等预算原样执行");

    let ArgvDecision::Keep { .. } = decide(&safe, &filler(6999), Some(7000)) else {
        panic!("len == 预算 - 1 → Keep");
    };

    let ArgvDecision::Omit { command, note } = decide(&safe, &filler(7001), Some(7000)) else {
        panic!("len == 预算 + 1 且可安全降级 → Omit 臂");
    };
    assert_eq!(command, "run", "OmitFiles 展开（{{files}} 连前导空白剥离）");
    assert!(note.contains("已降级为框架自发现"), "降级注在场: {note}");

    let unsafe_plan = argv_plan("run {files}", &["src/a.test.mjs"], false);
    let ArgvDecision::Reject { message } = decide(&unsafe_plan, &filler(7001), Some(7000)) else {
        panic!("len == 预算 + 1 且不可安全降级 → Reject 臂");
    };
    assert!(
        message.contains("7001") && message.contains("7000"),
        "文案载展开长度与预算两数字: {message}"
    );
}

/// 正向 / 边界（表驱动 3 形态）：超预算 + `omit_files_safe == true` →
/// `Omit`——`command` 为 `OmitFiles` 展开串（不含文件清单、零 `{files}` 残留），
/// `note` 载三数字与文件数。
#[test]
fn 裁决_超预算可降级则省略files() {
    let cases: [(&str, Vec<&str>, &str); 3] = [
        ("node --test {files}", vec!["src/a.test.mjs"], "node --test"),
        (
            "node --test {files} --reporter=spec",
            vec!["src/a.test.mjs", "src/b.test.mjs"],
            "node --test --reporter=spec",
        ),
        ("node --test {files}", Vec::new(), "node --test"),
    ];

    for (template, files, expected) in cases {
        let plan = argv_plan(template, &files, true);
        let ArgvDecision::Omit { command, note } = decide(&plan, &filler(8230), Some(7000)) else {
            panic!("{template}: 超预算 + 可安全降级 → Omit");
        };
        assert_eq!(
            command, expected,
            "{template}: command 为 OmitFiles 展开串（文件清单已剥离）"
        );
        assert!(
            !command.contains('{') && !command.contains("test.mjs"),
            "{template}: 零占位符 / 零文件清单残留，实际: {command}"
        );
        assert!(
            note.contains("展开 8230 字符超平台预算 7000 字符"),
            "{template}: note 载原始长度与预算: {note}"
        );
        assert!(
            note.contains(&format!("{} 个测试文件", files.len())),
            "{template}: note 载文件数（{}）: {note}",
            files.len()
        );
    }
}

/// 边界：超预算 + 可安全降级 + `OmitFiles` 展开后**恰 == budget** → `Omit`
/// （降级有效性边界——不得误判为拒绝）。
#[test]
fn 裁决_降级后恰等预算仍走降级() {
    let template = format!("{} {{files}}", filler(7000));
    let plan = argv_plan(&template, &["src/a.test.mjs"], true);

    let ArgvDecision::Omit { command, note } = decide(&plan, &filler(7500), Some(7000)) else {
        panic!("降级后恰等预算 → Omit（边界不降级为拒绝）");
    };
    assert_eq!(command.chars().count(), 7000, "降级后长度恰等预算");
    assert!(
        note.contains("展开 7500 字符超平台预算 7000 字符"),
        "注载原始长度与预算: {note}"
    );
}

/// 异常（表驱动）：超预算 + 可安全降级 + `OmitFiles` 展开后仍 `> budget` →
/// `Reject`，`message` 同时含原始长度与降级后长度两个数字（AC-5：降级无效也
/// 拒绝，且两次长度可回查）。
#[test]
fn 裁决_降级后仍超预算则拒绝且载两长度() {
    for omitted_len in [7001usize, 7100, 9000] {
        let template = format!("{} {{files}}", filler(omitted_len));
        let plan = argv_plan(&template, &["src/a.test.mjs"], true);

        let ArgvDecision::Reject { message } = decide(&plan, &filler(9500), Some(7000)) else {
            panic!("降级后 {omitted_len} 字符仍超预算 → Reject");
        };
        assert!(
            message.contains("展开 9500 字符"),
            "文案载原始展开长度（{omitted_len}）: {message}"
        );
        assert!(
            message.contains(&format!("降级为框架自发现后仍为 {omitted_len} 字符")),
            "文案载降级后长度（两数字并存）: {message}"
        );
        assert!(
            message.contains("7000") && message.contains("1 个测试文件"),
            "预算与文件数照常: {message}"
        );
    }
}

/// 异常：超预算 + `omit_files_safe == false` → `Reject`——文案含三数字与动作，
/// 且零误引安全软件 / 文件锁 / 重试措辞（成因是命令长度，非环境锁）。
#[test]
fn 裁决_不可安全降级则拒绝且文案三数字与动作() {
    let plan = argv_plan(
        "run {files}",
        &["src/a.test.mjs", "src/b.test.mjs", "src/c.test.mjs"],
        false,
    );

    let ArgvDecision::Reject { message } = decide(&plan, &filler(8230), Some(7000)) else {
        panic!("不可安全降级 → Reject");
    };
    for expected in ["8230", "7000", "3 个测试文件", "收窄", "includes", "拆分"] {
        assert!(
            message.contains(expected),
            "文案含 {expected}（定性 / 三数字 / 动作三项依序）: {message}"
        );
    }
    for forbidden in [
        "火绒",
        "云枢",
        "排除 workspace 父目录",
        "已自动重试",
        "占用",
        "被锁文件",
    ] {
        assert!(
            !message.contains(forbidden),
            "不可降级臂零误引 {forbidden}: {message}"
        );
    }
    assert!(
        !message.contains("降级为框架自发现后仍为"),
        "不可降级臂不带「降级后仍超」噪声: {message}"
    );
}

/// 边界（表驱动 4 形态）：`budget == None`（unix 语义）→ 恒 `Keep`——覆盖展开
/// 0 字符 / 100000 字符 / `files` 空清单 / `files` 500 条。
#[test]
fn 裁决_无预算恒keep四形态() {
    let long = filler(100_000);
    let many: Vec<String> = (0..500)
        .map(|index| format!("src/case{index:03}.test.mjs"))
        .collect();
    let many_refs: Vec<&str> = many.iter().map(String::as_str).collect();

    let cases: [(&str, Vec<&str>, &str); 4] = [
        ("展开 0 字符", Vec::new(), ""),
        ("展开 100000 字符", vec!["src/a.test.mjs"], long.as_str()),
        ("files 空清单", Vec::new(), "x"),
        ("files 500 条", many_refs, "xx"),
    ];

    for (label, files, expanded) in cases {
        let plan = argv_plan("run {files}", &files, true);
        let ArgvDecision::Keep { command } = decide(&plan, expanded, None) else {
            panic!("{label}: 无预算（unix 语义）→ 恒 Keep");
        };
        assert_eq!(command, expanded, "{label}: 原串回填（零降级）");
    }
}

/// 边界（表驱动）：`budget == Some(0)`——展开空串（`len 0 <= 0`）→ `Keep`；
/// 展开 1 字符 → 进裁决（safe → `Omit`、unsafe → `Reject`）。
#[test]
fn 裁决_零预算边界三形态() {
    let safe = argv_plan("", &[], true);

    let ArgvDecision::Keep { command } = decide(&safe, "", Some(0)) else {
        panic!("len 0 <= 0 → Keep");
    };
    assert_eq!(command, "", "空展开串原样回填");

    let ArgvDecision::Omit { command, note } = decide(&safe, "x", Some(0)) else {
        panic!("1 字符超零预算 + 可安全降级 → Omit");
    };
    assert_eq!(command, "", "OmitFiles 展开空模板 → 空串（0 <= 0 有效）");
    assert!(
        note.contains("展开 1 字符超平台预算 0 字符"),
        "注载两数字（1 / 0）: {note}"
    );

    let unsafe_plan = argv_plan("", &[], false);
    let ArgvDecision::Reject { message } = decide(&unsafe_plan, "x", Some(0)) else {
        panic!("1 字符超零预算 + 不可安全降级 → Reject");
    };
    assert!(
        message.contains("展开 1 字符 / 平台预算 0 字符"),
        "文案三数字（1 / 0 / 0）: {message}"
    );
}

/// 边界：`budget == usize::MAX` → 任意展开恒 `Keep`（无溢出 / 无 panic）。
#[test]
fn 裁决_预算最大值恒keep无溢出() {
    let long = filler(100_000);
    let mid = filler(7000);
    for expanded in ["", mid.as_str(), long.as_str()] {
        let plan = argv_plan("run {files}", &["src/a.test.mjs"], true);
        let ArgvDecision::Keep { command } = decide(&plan, expanded, Some(usize::MAX)) else {
            panic!("usize::MAX 预算 → 恒 Keep（无溢出 / 无 panic）");
        };
        assert_eq!(command, expanded, "原串回填");
    }
}

/// 边界：长度口径为 `chars().count()`——展开串含 emoji / 中文路径（字符数
/// 7000 而字节数 > 7000）→ `Keep`；字符数 7001 → 进裁决。
#[test]
fn 裁决_长度口径为字符数非字节数() {
    let plan = argv_plan("run {files}", &["src/中文路径.test.mjs"], true);
    let emoji = "🎉".repeat(7000);
    assert_eq!(emoji.chars().count(), 7000, "字符数 7000");
    assert!(
        emoji.len() > 7000,
        "字节数 {} > 预算（字节口径会误判）",
        emoji.len()
    );

    let ArgvDecision::Keep { command } = decide(&plan, &emoji, Some(7000)) else {
        panic!("字符口径 7000 == 预算 → Keep（不按字节降级）");
    };
    assert_eq!(command, emoji, "多字节串原样回填（不切碎）");

    let plus_one = format!("{emoji}x");
    assert_eq!(plus_one.chars().count(), 7001, "字符数 7001");
    let ArgvDecision::Omit { .. } = decide(&plan, &plus_one, Some(7000)) else {
        panic!("字符数 7001 → 进裁决（可安全降级 → Omit）");
    };
}

/// 边界（表驱动两态）：`plan.files == []`（含 `[]` 空清单）且长度来自模板静态
/// 填充——safe → `Omit`（`note` 载 `0 个测试文件`）；unsafe → `Reject`（文案载
/// `0` 个文件）。
#[test]
fn 裁决_空清单降级注载零文件() {
    let template = format!("{} {{files}}", filler(6990));
    let safe = argv_plan(&template, &[], true);
    let expanded = filler(7010);
    assert!(
        expanded.chars().count() > 7000,
        "展开超预算（静态填充口径）"
    );

    let ArgvDecision::Omit { command, note } = decide(&safe, &expanded, Some(7000)) else {
        panic!("空清单 + 可安全降级 → Omit");
    };
    assert_eq!(
        command.chars().count(),
        6990,
        "降级后长度来自模板静态填充（{{files}} 连前导空白剥离）"
    );
    assert!(note.contains("0 个测试文件"), "降级注载零文件数: {note}");

    let unsafe_plan = argv_plan(&template, &[], false);
    let ArgvDecision::Reject { message } = decide(&unsafe_plan, &expanded, Some(7000)) else {
        panic!("空清单 + 不可安全降级 → Reject");
    };
    assert!(
        message.contains("0 个测试文件"),
        "拒绝文案载零文件数: {message}"
    );
}

/// 正向：裁决纯数据产物三变体经 `decide_argv` 逐一可达，`command` / `note` /
/// `message` 字段经 `super::` 构造与匹配可断言（不为其可测性加 `pub`、不加仅
/// 测试用 `PartialEq` 派生）。
#[test]
fn 裁决产物三变体字段可断言() {
    let plan = argv_plan("run {files}", &["src/a.test.mjs"], true);

    let ArgvDecision::Keep { command } = decide(&plan, "run", Some(7000)) else {
        panic!("Keep 变体可达");
    };
    assert_eq!(command, "run", "Keep.command = 入参展开串");

    let ArgvDecision::Omit { command, note } = decide(&plan, &filler(7001), Some(7000)) else {
        panic!("Omit 变体可达");
    };
    assert_eq!(command, "run", "Omit.command = OmitFiles 展开串");
    assert!(!note.is_empty(), "Omit.note 非空（降级说明单点装配）");

    let reject_plan = argv_plan("run {files}", &["src/a.test.mjs"], false);
    let ArgvDecision::Reject { message } = decide(&reject_plan, &filler(7001), Some(7000)) else {
        panic!("Reject 变体可达");
    };
    assert!(
        message.contains("命令行长度超平台上限"),
        "Reject.message = 环境面摘要（定性起手）: {message}"
    );
}

/// 正向（cfg 门控 ×2）：`line_budget()` 取臂——Windows 臂 `Some(7000)`、非
/// Windows 臂 `None`（平台分叉只此一处，裁决逻辑全经预算入参流转）。
#[test]
fn 预算取臂_平台双态() {
    #[cfg(windows)]
    {
        assert_eq!(line_budget(), Some(7000), "Windows 臂恒 Some(7000)");
        assert_eq!(
            line_budget(),
            Some(super::CMD_LINE_BUDGET),
            "取臂值即常量本身（单源）"
        );
    }
    #[cfg(not(windows))]
    {
        assert_eq!(line_budget(), None, "非 Windows 臂恒 None（无预算）");
    }
}

/// 边界（cfg 门控）：`CMD_LINE_BUDGET` 常量锚定 `== 7000`（`cmd.exe` 硬上限
/// 8191 − 1191 余量）；非 Windows 该常量不可见（`#[cfg(windows)]` 编译期隔离
/// 即断言面）。
#[test]
fn 预算常量锚定七千() {
    #[cfg(windows)]
    {
        // 取值经局部绑定断言（避免 `assertions_on_constants` 噪声）
        let budget = super::CMD_LINE_BUDGET;
        assert_eq!(budget, 7000, "常量锚定 7000（非 8191）");
        assert!(budget < 8191, "留出 cmd /C 前缀 + raw_arg 转引放大的余量");
    }
    #[cfg(not(windows))]
    {
        // 非 Windows 该常量不可见（编译期隔离），平台无预算语义
        assert_eq!(line_budget(), None, "非 Windows 零预算常量");
    }
}

/// 边界：`ExpandMode` 两态语义与变体封闭性——穷尽 `match` 编译通过即「仅两
/// 变体」（无第三态、无 `Default` / 宽松 `From` 入口），两态在同一输入上必须
/// 可区分（无回落臂）。
#[test]
fn 展开模式_两态语义与变体封闭() {
    fn describe(mode: ExpandMode) -> &'static str {
        match mode {
            ExpandMode::WithFiles => "with",
            ExpandMode::OmitFiles => "omit",
        }
    }
    assert_eq!(describe(ExpandMode::WithFiles), "with");
    assert_eq!(describe(ExpandMode::OmitFiles), "omit");

    let files = vec!["src/a.test.mjs".to_owned()];
    let with = expand_template(
        "run {files}",
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &files,
        ExpandMode::WithFiles,
    );
    let omit = expand_template(
        "run {files}",
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &files,
        ExpandMode::OmitFiles,
    );
    assert_eq!(with, "run src/a.test.mjs", "WithFiles 保留清单");
    assert_eq!(omit, "run", "OmitFiles 剥离清单与前导空白");
    assert_ne!(with, omit, "两态在可区分输入上必不同串");
}

/// 正向（表驱动 1 / 2 / 50 清单规模）：`WithFiles` 逐字同改造前（空格连接），
/// 其余四占位符两态同义——`OmitFiles` 结果 = `WithFiles` 结果去掉文件清单与其
/// 前导空白（AC-2 / AC-3）。
#[test]
fn 模板展开_withfiles清单规模逐字且omit同义() {
    let template =
        "node --test --test-reporter=spec --test-reporter-destination=\"{results_file}\" {files}";
    let prefix =
        "node --test --test-reporter=spec --test-reporter-destination=\"/ws/plan/results.txt\"";
    let one = vec!["src/a.test.mjs".to_owned()];
    let two = vec!["src/a.test.mjs".to_owned(), "src/b.test.mjs".to_owned()];
    let many: Vec<String> = (0..50)
        .map(|index| format!("tests/t{index:02}.test.mjs"))
        .collect();

    for files in [one, two, many] {
        let with = expand_template(
            template,
            "/ws/plan",
            "/ws/plan/results.txt",
            "/ws/plan/coverage-summary.json",
            None,
            &files,
            ExpandMode::WithFiles,
        );
        assert_eq!(
            with,
            format!("{prefix} {}", files.join(" ")),
            "WithFiles 逐字（清单 {} 项，空格连接）",
            files.len()
        );

        let omit = expand_template(
            template,
            "/ws/plan",
            "/ws/plan/results.txt",
            "/ws/plan/coverage-summary.json",
            None,
            &files,
            ExpandMode::OmitFiles,
        );
        assert_eq!(
            omit,
            prefix,
            "OmitFiles = WithFiles 去掉清单与前导空白（其余四占位符逐字同义，{} 项）",
            files.len()
        );
        assert!(!omit.contains('{'), "零占位符残留: {omit}");
    }
}

/// 正向（表驱动 4 形态）：`OmitFiles` 把 `{files}` **连其前导空白**一并剥离
/// ——单空格 / 多空格 / 制表符 / 行首无空白四形态；其余占位符展开同义。
#[test]
fn 模板展开_omitfiles前导空白四形态() {
    let files = vec!["src/a.test.mjs".to_owned()];
    let cases: [(&str, &str); 4] = [
        ("run {files}", "run"),
        ("run   {files}", "run"),
        ("run\t{files}", "run"),
        ("{files}", ""),
    ];

    for (template, expected) in cases {
        let omit = expand_template(
            template,
            "/ws/plan",
            "/ws/plan/results.txt",
            "/ws/plan/coverage-summary.json",
            None,
            &files,
            ExpandMode::OmitFiles,
        );
        assert_eq!(
            omit, expected,
            "前导空白剥离（模板 {:?}）：{omit:?}",
            template
        );
        assert!(!omit.contains("{files}"), "零占位符残留: {omit}");
        assert!(!omit.contains("test.mjs"), "零文件清单残留: {omit}");
    }

    // 其余四占位符两态同义（前导空白剥离只作用于 {files}）
    let template = "runner --out \"{results_file}\" {files}";
    let omit = expand_template(
        template,
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &files,
        ExpandMode::OmitFiles,
    );
    assert_eq!(
        omit, "runner --out \"/ws/plan/results.txt\"",
        "占位符替换与空白剥离各司其职（结果逐字）"
    );
}

/// 边界：模板不含 `{files}`（rust 档模板）→ 两态展开**逐字相同**（剥离算法只
/// 作用于在场占位符）。
#[test]
fn 模板展开_无files模板两态同串() {
    let template =
        "cargo llvm-cov --workspace --json --output-path \"{coverage_file}\" > \"{results_file}\"";
    let files = vec!["tests/it.rs".to_owned()];

    let with = expand_template(
        template,
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &files,
        ExpandMode::WithFiles,
    );
    let omit = expand_template(
        template,
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &files,
        ExpandMode::OmitFiles,
    );

    assert_eq!(with, omit, "rust 档模板两态逐字相同");
    assert_eq!(
        with,
        "cargo llvm-cov --workspace --json --output-path \"/ws/plan/coverage-summary.json\" > \"/ws/plan/results.txt\"",
        "两态展开串即改造前同串（零改写）"
    );
}

/// 边界：`files == []` 空清单——`WithFiles` 保留前导空白（`{files}` → 空串）、
/// `OmitFiles` 剥离前导空白 → 两态**不同串**（两态判据的锚用例）。
#[test]
fn 模板展开_空清单两态不同串() {
    let template = "run {files}";
    let with = expand_template(
        template,
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &[],
        ExpandMode::WithFiles,
    );
    let omit = expand_template(
        template,
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &[],
        ExpandMode::OmitFiles,
    );

    assert_eq!(with, "run ", "WithFiles 空清单保留前导空白（同改造前）");
    assert_eq!(omit, "run", "OmitFiles 剥离前导空白");
    assert_ne!(with, omit, "空清单形态两态必不同串");
}

/// 边界（表驱动两形态）：`{files}` 一处 vs 多处出现 → 两态均**全量处理**
/// （逐处替换 / 逐处剥离），零 `{files}` 残留。
#[test]
fn 模板展开_多处files两态全量处理() {
    let files = vec!["a.rs".to_owned(), "b.rs".to_owned()];

    for (label, template) in [
        ("一处", "run {files}"),
        ("两处", "run {files} | {files}"),
        ("三处混排", "{files} run {files} --out {files}"),
    ] {
        let with = expand_template(
            template,
            "/ws/plan",
            "/ws/plan/results.txt",
            "/ws/plan/coverage-summary.json",
            None,
            &files,
            ExpandMode::WithFiles,
        );
        assert_eq!(
            with.matches("a.rs b.rs").count(),
            template.matches("{files}").count(),
            "{label}: WithFiles 逐处全量替换: {with}"
        );
        assert!(!with.contains('{'), "{label}: 零占位符残留: {with}");

        let omit = expand_template(
            template,
            "/ws/plan",
            "/ws/plan/results.txt",
            "/ws/plan/coverage-summary.json",
            None,
            &files,
            ExpandMode::OmitFiles,
        );
        assert!(
            !omit.contains("{files}") && !omit.contains("a.rs"),
            "{label}: OmitFiles 逐处剥离（零残留）: {omit}"
        );
    }
}

/// 异常（表驱动）：空模板 `""` → 两态均返回 `""`；模板含超长填充（> 1000
/// 字符）与特殊字符（emoji / 换行 / 制表符）→ 逐字保留、不 panic。
#[test]
fn 模板展开_空模板与特殊字符两态() {
    // 形态一：空模板
    for mode in [ExpandMode::WithFiles, ExpandMode::OmitFiles] {
        let expanded = expand_template(
            "",
            "/ws/plan",
            "/ws/plan/results.txt",
            "/ws/plan/coverage-summary.json",
            None,
            &["a.rs".to_owned()],
            mode,
        );
        assert_eq!(expanded, "", "空模板两态均返回空串");
    }

    // 形态二：特殊字符 + 超长填充（>1000 字符）
    let filler_segment = "y".repeat(1200);
    let template = format!("runner \"line1\nline2\" --tab\t--emoji 🎉 {filler_segment} {{files}}");
    let files = vec!["src/中文.test.mjs".to_owned()];

    let with = expand_template(
        &template,
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &files,
        ExpandMode::WithFiles,
    );
    assert!(
        with.contains("line1\nline2"),
        "换行逐字保留（未转义）: {}",
        &with[..20]
    );
    assert!(with.contains('\t'), "制表符逐字保留");
    assert!(with.contains("🎉"), "emoji 逐字保留（多字节不切碎）");
    assert!(with.contains("src/中文.test.mjs"), "中文清单项逐字保留");
    assert!(with.chars().count() > 1200, "超长填充不截断");
    assert!(!with.contains('{'), "零占位符残留");

    let omit = expand_template(
        &template,
        "/ws/plan",
        "/ws/plan/results.txt",
        "/ws/plan/coverage-summary.json",
        None,
        &files,
        ExpandMode::OmitFiles,
    );
    assert!(
        omit.chars().count() < with.chars().count(),
        "OmitFiles 更短（清单与空白已剥离）"
    );
    assert!(
        omit.contains("🎉") && omit.contains('\t'),
        "特殊字符不受剥离影响"
    );
    assert!(
        !omit.contains("src/中文.test.mjs"),
        "清单已剥离: 尾部 {:?}",
        &omit[omit.len().saturating_sub(20)..]
    );
}

// ---------------------------------------------------------------------------
// 预算裁决接入执行面（`execute_plan` 三臂——test-exec-argv-budget，AC-2 / AC-3 /
// AC-4 / AC-5）：真实 spawn（不 mock），观测面 = 退出码 / 工件 / 替身 argv 记录 /
// marker；`Omit` / `Reject` 臂以 `cfg` 真实取臂门控（平台无关覆盖全经
// `decide_argv` 的预算入参）。
// ---------------------------------------------------------------------------

/// 展开口径复算（与 `execute_plan` 同一纯函数同一入参——`argv_len` 断言面）。
fn expanded_for(plan: &TestPlan, report_dir: &Path) -> String {
    expand_template(
        plan.template(),
        &to_posix(report_dir),
        &to_posix(&report_dir.join(plan.results_output)),
        &to_posix(&report_dir.join(plan.coverage_output)),
        plan.config_args.as_deref(),
        &plan.files,
        ExpandMode::WithFiles,
    )
}

/// `OmitFiles` 展开口径复算（降级后长度断言面——与 `decide_argv` 同一纯函数）。
#[cfg(windows)]
fn omitted_for(plan: &TestPlan, report_dir: &Path) -> String {
    expand_template(
        plan.template(),
        &to_posix(report_dir),
        &to_posix(&report_dir.join(plan.results_output)),
        &to_posix(&report_dir.join(plan.coverage_output)),
        plan.config_args.as_deref(),
        &plan.files,
        ExpandMode::OmitFiles,
    )
}

/// 正向（`Keep` 臂）：未超预算 → 零行为变化——`exit_code == 0`、
/// `error` / `fault` / `argv_note` 全 `None`、`argv_len` == `WithFiles` 展开字符
/// 数、既有工件解析面零变化。
#[tokio::test]
async fn 执行臂_未超预算零降级且argv_len同展开口径() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let spec_src = preset(ws.path(), "argv-keep-preset.txt", SPEC_OUTPUT);

    let template = if cfg!(windows) {
        format!(
            "cmd /C type {} > {{results_file}}",
            spec_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", spec_src.to_string_lossy())
    };
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let execution = execute_plan(&plan, &report_dir, CancelFlag::never())
        .await
        .expect("绿跑应 Ok");

    let expanded = expanded_for(&plan, &report_dir);
    assert_eq!(
        execution.argv_len,
        expanded.chars().count(),
        "argv_len = WithFiles 展开字符数（与 expand_template 同口径）"
    );
    assert!(
        execution.argv_len < 7000,
        "fixture 展开远低于预算（Keep 臂前提），实际 {}",
        execution.argv_len
    );
    assert_eq!(
        execution.argv_note, None,
        "Keep 臂零降级注（argv_note 三态之一）"
    );
    assert_eq!(execution.exit_code, 0, "退出码透传");
    assert!(
        execution.error.is_none(),
        "绿面零 error: {:?}",
        execution.error
    );
    assert!(execution.fault.is_none(), "绿面零 fault");
    assert_eq!(execution.cases.len(), 5, "既有工件解析面零变化（五用例行）");
    assert!(execution.measured.is_some(), "覆盖实测块照常解析");
}

/// 正向 / 边界（`argv_note` 第三态）：fault 早返（取消臂，进程未拉起）→
/// `argv_note == None` 且 `argv_len` 仍为展开长度（长度可回查）。
#[tokio::test]
async fn 执行臂_取消臂零降级注且长度可回查() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "cmd /C echo ran".to_owned()
    } else {
        "sh -c 'echo ran'".to_owned()
    };
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let cancel = CancelFlag::never();
    cancel.handle().set();
    let execution = execute_plan(&plan, &report_dir, cancel)
        .await
        .expect("取消不 Err（产出携 fault）");

    assert_eq!(execution.fault, Some(RunFault::Stopped), "取消 → fault");
    assert_eq!(
        execution.argv_note, None,
        "fault 早返面零降级注（argv_note 三态：Keep / fault → None，Omit → Some）"
    );
    assert_eq!(
        execution.argv_len,
        expanded_for(&plan, &report_dir).chars().count(),
        "fault 早返亦留展开长度（头可回查）"
    );
}

/// PATH 前置窗口（RAII：drop 还原原 PATH——裸名程序经 cmd 的 PATH 解析命中
/// cwd 内替身；须与 `env_window` 的进程环境锁配合使用）。
#[cfg(windows)]
struct PathPrefix(Option<std::ffi::OsString>);

#[cfg(windows)]
impl PathPrefix {
    fn prepend(dir: &std::path::Path) -> Self {
        let original = std::env::var_os("PATH");
        let joined = std::env::join_paths(
            std::iter::once(dir.to_path_buf())
                .chain(std::env::split_paths(&original.clone().unwrap_or_default())),
        )
        .expect("join PATH");
        std::env::set_var("PATH", joined);
        Self(original)
    }
}

#[cfg(windows)]
impl Drop for PathPrefix {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }
}

/// 降级臂替身（Windows）：cwd 内 `.cmd` 程序（裸名 `argvshim`，经 PATH 前置
/// 命中），逐字节记录 argv（日志路径经环境变量注入——命令串内引号形态零转义
/// 风险）后恒零退出。返回模板前缀（填充段长度按预算差值取）。
#[cfg(windows)]
fn install_argv_shim(
    cwd: &std::path::Path,
    argv_log: &std::path::Path,
    spec_src: &std::path::Path,
    results_dest: &std::path::Path,
) -> String {
    std::fs::write(
        cwd.join("argvshim.cmd"),
        "@echo off\r\n\
         if defined ARGV_SHIM_LOG echo %* > \"%ARGV_SHIM_LOG%\"\r\n\
         if defined ARGV_SHIM_RESULTS copy /y \"%ARGV_SHIM_SPEC%\" \"%ARGV_SHIM_RESULTS%\" >nul\r\n\
         echo shim-ran\r\n\
         exit /b 0\r\n",
    )
    .expect("写 argvshim.cmd 失败");
    std::env::set_var("ARGV_SHIM_LOG", argv_log.as_os_str());
    std::env::set_var("ARGV_SHIM_SPEC", spec_src.as_os_str());
    std::env::set_var("ARGV_SHIM_RESULTS", results_dest.as_os_str());
    // 模板前缀（无引号填充段——`echo %*` 记录面不被引号配对吞掉重定向）
    "argvshim --filler ".to_owned()
}

/// 正向（`Omit` 臂，AC-3）：超预算 + `omit_files_safe == true` → 省略 `{files}`
/// 后执行成功——`exit_code == 0`、`error == None`、`argv_note` 为
/// `argv_degrade_note` 同串、`argv_len` 仍为 `WithFiles` 展开长度、替身 argv
/// 记录不含文件清单（命令确已降级）。
#[cfg(windows)]
#[tokio::test]
async fn 执行臂_超预算可降级则省略files执行成功() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let argv_log = ws.path().join("argv-shim.log");
    let spec_src = preset(ws.path(), "argv-omit-preset.txt", SPEC_OUTPUT);
    let report_dir = ws.path().join("reports").join("it_node-test");
    let prefix = install_argv_shim(&cwd, &argv_log, &spec_src, &report_dir.join("results.txt"));
    let _path = PathPrefix::prepend(&cwd);

    // 降级后长度恰 7000（== 预算，边界有效）：填充 = 预算 - 前缀长度
    let pad = "x".repeat(7000 - prefix.chars().count());
    let template = format!("{prefix}{pad} {{files}}");
    let plan = node_test_plan(&cwd, template);
    assert_eq!(
        omitted_for(&plan, &report_dir).chars().count(),
        7000,
        "降级后长度恰等预算（Omit 有效性边界）"
    );

    let execution = execute_plan(&plan, &report_dir, CancelFlag::never())
        .await
        .expect("降级臂应正常执行（不 Err）");
    for key in ["ARGV_SHIM_LOG", "ARGV_SHIM_SPEC", "ARGV_SHIM_RESULTS"] {
        std::env::remove_var(key);
    }

    let expanded = expanded_for(&plan, &report_dir);
    assert!(
        expanded.chars().count() > 7000,
        "WithFiles 展开超预算（{} 字符）",
        expanded.chars().count()
    );
    assert_eq!(
        execution.exit_code,
        0,
        "降级后命令执行成功（退出码透传）：fault={:?} error={:?} output={:?}",
        execution.fault,
        execution.error,
        String::from_utf8_lossy(&execution.output)
    );
    assert!(
        execution.error.is_none(),
        "降级面零 error（降级不是失败）: {:?}",
        execution.error
    );
    assert!(execution.fault.is_none(), "降级面零 fault");
    assert_eq!(
        execution.argv_len,
        expanded.chars().count(),
        "argv_len 仍为 WithFiles 展开长度（原始长度可回查）"
    );
    assert_eq!(
        execution.argv_note,
        Some(crate::environment::argv_degrade_note(
            7000,
            expanded.chars().count(),
            plan.files.len()
        )),
        "argv_note 与 argv_degrade_note 同串（单点装配）"
    );

    let recorded = std::fs::read_to_string(&argv_log).expect("替身 argv 记录落盘");
    assert!(
        !recorded.contains("test.mjs"),
        "降级后命令不含文件清单（实参: {recorded}）"
    );
    assert!(
        recorded.contains("--filler"),
        "命令确已执行（替身记录在案）: {recorded}"
    );
    assert!(
        !recorded.contains("{files}"),
        "零占位符残留（展开完备）: {recorded}"
    );
}

/// 异常（`Reject` 臂，AC-4）：超预算 + `omit_files_safe == false` → 环境面归因
/// 早返、零 spawn——`fault == Some(Environment(..))`、`error == None`、
/// `exit_code == EXIT_CODE_NO_STATUS`、载荷全空、`argv_note == None`、报告目录
/// 无 summary；**零 spawn 双证据** = ①程序名故意不可达仍返回 `Ok`（裁决先于
/// `ensure_program_resolvable` 与 spawn）②替身 marker 不存在。
#[cfg(windows)]
#[tokio::test]
async fn 执行臂_超预算不可降级则环境归因零spawn() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let marker = ws.path().join("argv-reject-marker.log");
    // 替身在场（若被拉起即写 marker）——但模板程序名故意拼错，故不可达
    std::fs::write(
        cwd.join("argv-reject-shim.cmd"),
        "@echo off\r\necho ran > \"%ARGV_REJECT_MARKER%\"\r\nexit /b 0\r\n",
    )
    .expect("写 argv-reject-shim.cmd 失败");
    std::env::set_var("ARGV_REJECT_MARKER", marker.as_os_str());

    let _path = PathPrefix::prepend(&cwd);
    let pad = "x".repeat(7500);
    let template = format!("argv-reject-shim-typo --filler \"{pad}\" {{files}}");
    let mut plan = node_test_plan(&cwd, template);
    plan.omit_files_safe = false;
    let report_dir = ws.path().join("reports").join("it_node-test");

    let execution = execute_plan(&plan, &report_dir, CancelFlag::never())
        .await
        .expect("拒绝降级不 Err（产出携 fault）");
    std::env::remove_var("ARGV_REJECT_MARKER");

    let Some(RunFault::Environment(message)) = execution.fault.clone() else {
        panic!("拒绝臂 → fault = Environment，实际: {:?}", execution.fault);
    };
    let expanded_len = expanded_for(&plan, &report_dir).chars().count();
    assert!(
        message.contains("命令行长度超平台上限"),
        "文案定性起手: {message}"
    );
    assert!(
        message.contains(&format!("展开 {expanded_len} 字符")),
        "文案载展开长度（{expanded_len}）: {message}"
    );
    assert!(
        message.contains("收窄") && message.contains("拆分 suite"),
        "文案载动作建议: {message}"
    );
    assert_eq!(execution.error, None, "环境面不走 error（MUST NOT 假失败）");
    assert_eq!(
        execution.exit_code, EXIT_CODE_NO_STATUS,
        "进程未拉起 → 无退出码哨兵"
    );
    assert!(
        execution.cases.is_empty()
            && execution.source_files.is_empty()
            && execution.measured.is_none(),
        "载荷全空（跳过工件解析）"
    );
    assert_eq!(execution.argv_note, None, "拒绝臂零降级注");
    assert_eq!(
        execution.argv_len, expanded_len,
        "拒绝臂亦留展开长度（记录头可回查）"
    );
    assert!(
        !report_dir.join("summary.json").exists(),
        "拒绝臂不写 summary"
    );
    assert!(!marker.exists(), "零 spawn（替身 marker 未落）");
    // 证据一：程序名不可达却返回 Ok（裁决先于可达性检查与 spawn）
    assert!(
        !cwd.join("argv-reject-shim-typo.cmd").exists(),
        "模板程序名确实不可达（若裁决靠后则 ensure_program_resolvable 会 Err）"
    );
}

/// 异常（`Reject` 第二出口，AC-5）：超预算 + 可安全降级 + `OmitFiles` 展开后仍
/// 超预算 → 与不安全臂**同一出口**（环境面归因、零 spawn），文案同时载原始长度
/// 与降级后长度。
#[cfg(windows)]
#[tokio::test]
async fn 执行臂_降级后仍超预算与不可降级同出口() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    // 降级后长度仍 7100（> 7000）：填充 = 7100 - 前缀长度
    let pad = "x".repeat(7100 - "argvshim --filler ".chars().count());
    let template = format!("argvshim --filler {pad} {{files}}");
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let omitted_len = omitted_for(&plan, &report_dir).chars().count();
    assert_eq!(omitted_len, 7100, "降级后仍超预算（构造）");

    let execution = execute_plan(&plan, &report_dir, CancelFlag::never())
        .await
        .expect("降级无效不 Err（产出携 fault）");

    let Some(RunFault::Environment(message)) = execution.fault.clone() else {
        panic!(
            "降级无效 → fault = Environment（与不可降级同出口），实际: {:?}",
            execution.fault
        );
    };
    let expanded_len = expanded_for(&plan, &report_dir).chars().count();
    assert!(
        expanded_len > omitted_len,
        "原始长度（{expanded_len}）> 降级后长度（{omitted_len}）"
    );
    assert!(
        message.contains(&format!("展开 {expanded_len} 字符")),
        "文案载原始长度（{expanded_len}）: {message}"
    );
    assert!(
        message.contains(&format!("降级为框架自发现后仍为 {omitted_len} 字符")),
        "文案载降级后长度（{omitted_len}）: {message}"
    );
    assert_eq!(execution.error, None, "环境面不走 error");
    assert_eq!(
        execution.exit_code, EXIT_CODE_NO_STATUS,
        "零 spawn → 无退出码哨兵"
    );
    assert_eq!(execution.argv_note, None, "拒绝臂零降级注");
    assert!(
        !report_dir.join("summary.json").exists(),
        "拒绝臂不写 summary"
    );
}

/// 正向（非 Windows 臂，零降级语义）：`line_budget() == None` → 展开超 7000
/// 字符仍原样执行——`fault == None`、`argv_note == None`、命令含文件清单。
#[cfg(not(windows))]
#[tokio::test]
async fn 执行臂_unix无预算超预算原样执行() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let pad = "y".repeat(7500);
    // `printf '%s\n' "<pad>" {files}`：首行填充、次行文件清单（命令确含清单）
    let template = format!("printf '%s\n' \"{pad}\" {{files}}");
    let plan = node_test_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("it_node-test");

    let expanded = expanded_for(&plan, &report_dir);
    assert!(
        expanded.chars().count() > 7000,
        "展开超平台预算（{} 字符）",
        expanded.chars().count()
    );

    let execution = execute_plan(&plan, &report_dir, CancelFlag::never())
        .await
        .expect("unix 无预算 → 原样执行（不 Err）");

    assert!(
        execution.fault.is_none(),
        "unix 臂零 fault（不引入第二套降级语义）: {:?}",
        execution.fault
    );
    assert_eq!(execution.argv_note, None, "unix 臂零降级注");
    assert_eq!(
        execution.argv_len,
        expanded.chars().count(),
        "argv_len 与展开口径一致"
    );
    let output = String::from_utf8_lossy(&execution.output).into_owned();
    assert!(
        output.contains("src/add.test.mjs"),
        "命令原样携带文件清单（未降级）: 尾部 {:?}",
        &output[output.len().saturating_sub(40)..]
    );
}

/// 边界（表驱动两形态）：`PlanExecution::argv_len` 口径为 `chars().count()`
/// ——多字节路径展开 → 等于字符数而非字节数；`files` 空 + 模板静态填充 → 等于
/// 模板展开字符数。
#[tokio::test]
async fn 执行臂_argv_len口径为字符数() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let spec_src = preset(ws.path(), "argv-len-preset.txt", SPEC_OUTPUT);

    // 形态一：报告目录含中文（多字节路径进入展开串）
    let report_dir = ws.path().join("reports").join("中文计划");
    let template = if cfg!(windows) {
        format!(
            "cmd /C type {} > {{results_file}}",
            spec_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", spec_src.to_string_lossy())
    };
    let plan = node_test_plan(&cwd, template);
    let execution = execute_plan(&plan, &report_dir, CancelFlag::never())
        .await
        .expect("多字节路径绿跑应 Ok");
    let expanded = expanded_for(&plan, &report_dir);
    assert_eq!(
        execution.argv_len,
        expanded.chars().count(),
        "argv_len 取字符数"
    );
    assert!(
        expanded.len() > expanded.chars().count(),
        "字节数（{}）> 字符数（{}）——多字节路径确实在场",
        expanded.len(),
        expanded.chars().count()
    );
    assert!(
        execution.error.is_none(),
        "多字节报告路径工件落位零错误: {:?}",
        execution.error
    );

    // 形态二：`files` 空 + 模板静态填充 → argv_len == 模板展开字符数
    let static_report_dir = ws.path().join("reports").join("rust-static");
    let static_template = if cfg!(windows) {
        format!(
            "cmd /C type {} > {{results_file}}",
            spec_src.to_string_lossy().replace('/', "\\")
        )
    } else {
        format!("cp \"{}\" \"{{results_file}}\"", spec_src.to_string_lossy())
    };
    let rust = rust_plan(&cwd, static_template);
    let execution = execute_plan(&rust, &static_report_dir, CancelFlag::never())
        .await
        .expect("空清单档绿跑应 Ok");
    let expanded = expanded_for(&rust, &static_report_dir);
    assert_eq!(
        execution.argv_len,
        expanded.chars().count(),
        "空清单 + 静态填充：argv_len == 模板展开字符数"
    );
    assert!(
        !expanded.contains("test.mjs"),
        "空清单档展开串不含文件清单: {expanded}"
    );
}
