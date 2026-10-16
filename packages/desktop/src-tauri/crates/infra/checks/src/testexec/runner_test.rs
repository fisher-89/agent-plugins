use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use super::{
    execute_plan, expand_template, non_zero_exit_error,
    SUITE_COMMAND_TIMEOUT, VERSION_DETECT_TIMEOUT,
};
use crate::testexec::detect::TestPlan;
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

/// 手工 TestPlan 构造（结果 / 覆盖双工件分离形态——rust 档链形态语义锚用：
/// 覆盖段与测试段各写各的工件面，实测块读独立 coverage 工件）。
fn split_outputs_plan(cwd: &Path, template: String) -> TestPlan {
    TestPlan {
        id: "it_chain-shape".to_owned(),
        framework: "node-test".to_owned(),
        root: "app".to_owned(),
        cwd: cwd.to_path_buf(),
        files: vec!["src/add.test.mjs".to_owned()],
        config_args: None,
        shell_template: template.clone(),
        cmd_template: template,
        coverage_format: CoverageFormat::NodeTest,
        coverage_output: "coverage.txt",
        results_output: "results.txt",
    }
}

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

/// 结果工件文本归一：CRLF → LF，并去除逐行尾随空白。rust 档 stdout 经注入的
/// `cargo test > "{results_file}"` 重定向落工件（生产 spawn 为 `cmd /C` 双层包裹），
/// cmd 会把重定向符前的分隔空格计入 echo 参数——尾随空格是壳解析产物而非工件内容
/// 语义（stdout 全量落工件不变），故比对前归一。
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

    let execution = execute_plan(&plan, &report_dir)
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

    let execution = execute_plan(&plan, &ws.path().join("reports").join("it_node-test"))
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

#[tokio::test]
async fn rust档链形态_覆盖段恒执行_尾段退出码透传() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let cov_src = preset(ws.path(), "chain-cov-preset.txt", SPEC_OUTPUT);
    let res_src = preset(ws.path(), "chain-res-preset.txt", SPEC_OUTPUT);

    // 绿跑形态：覆盖段 + 测试段双产出，终码 0
    let green = if cfg!(windows) {
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
    let plan = split_outputs_plan(&cwd, green);
    let execution = execute_plan(&plan, &ws.path().join("reports").join("g"))
        .await
        .expect("绿跑形态应 Ok");
    assert_eq!(execution.exit_code, 0, "绿跑终码 0");
    assert!(
        execution.error.is_none(),
        "绿跑零错误面，实际: {:?}",
        execution.error
    );
    assert_eq!(execution.cases.len(), 5, "测试段结果工件解析五用例");
    assert!(
        execution.measured.is_some(),
        "覆盖段产出实测块（& 无条件链——绿跑不产覆盖的旧病免疫）"
    );

    // 红跑形态：测试段替身退 3——覆盖段仍执行、终码 3 原样透传
    let red = if cfg!(windows) {
        format!(
            "cmd /C type \"{}\" > \"{{coverage_file}}\" & cmd /C exit /b 3",
            cov_src.to_string_lossy().replace('/', "\\"),
        )
    } else {
        format!(
            "cp \"{}\" \"{{coverage_file}}\"; exit 3",
            cov_src.to_string_lossy(),
        )
    };
    let plan = split_outputs_plan(&cwd, red);
    let execution = execute_plan(&plan, &ws.path().join("reports").join("r"))
        .await
        .expect("红跑形态走产出（不 Err）");
    assert_eq!(
        execution.exit_code, 3,
        "终码 = 尾段（测试步）退出码，不被前置段洗 0"
    );
    assert!(
        execution.measured.is_some(),
        "红跑覆盖段仍执行（if 体吞尾段旧病免疫），覆盖工件在场"
    );
    assert!(
        execution
            .error
            .as_deref()
            .is_some_and(|error| error.contains("非零退出") && error.contains("3")),
        "错误面记因尾段退出码，实际: {:?}",
        execution.error
    );
    assert!(execution.cases.is_empty(), "零结果工件零用例（不产假用例）");
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

    let err = execute_plan(&plan, &ws.path().join("reports").join("p"))
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

    let err = execute_plan(&plan, &ws.path().join("reports").join("p"))
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

    let execution = execute_plan(&plan, &ws.path().join("reports").join("p"))
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

    let execution = execute_plan(&plan, &ws.path().join("reports").join("p"))
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

    let execution = execute_plan(&plan, &ws.path().join("reports").join("p"))
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

/// 边界：超时常量锚定——suite 命令 600s / 版本探测 30s 与 CLI runCommand
/// 600000 / 30_000 同值（真实超时触发臂见 test-design 不可测试项）。
#[test]
fn 超时常量锚定() {
    assert_eq!(
        SUITE_COMMAND_TIMEOUT,
        Duration::from_secs(600),
        "CLI runCommand(cmd, cwd, 600000) 同值"
    );
    assert_eq!(
        VERSION_DETECT_TIMEOUT,
        Duration::from_secs(30),
        "CLI 版本探测 timeout 30_000 同值"
    );
}

// ---------------------------------------------------------------------------
// PlanExecution.output（change-flow-show-step-only，AC-7 / D7）：脚本真实输出
// **合并流**（stdout 字节前缀 + stderr 字节后缀，逐字；**不加 `2>&1`**——rust
// 档 `cargo test > results_file` 已把 stdout 重定向进结果工件，`2>&1` 会把
// stderr 混入污染 `parse_cargo_test`）。手工拼接只服务 record 回查，结果工件
// 解析仍只读 stdout 面；`error` 面仍经合并文本派生。
// ---------------------------------------------------------------------------

/// 手工 TestPlan 构造（rust 档：`cargo test > {results_file}` 重定向注入面 +
/// 覆盖工件独立面——output 承载与工件落位两读面）。
fn rust_plan(cwd: &Path, template: String) -> TestPlan {
    TestPlan {
        id: "it_rust-shape".to_owned(),
        framework: "rust".to_owned(),
        root: "app".to_owned(),
        cwd: cwd.to_path_buf(),
        files: Vec::new(),
        config_args: None,
        shell_template: template.clone(),
        cmd_template: template,
        coverage_format: CoverageFormat::LlvmCov,
        coverage_output: "coverage.txt",
        results_output: "results.txt",
    }
}

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

    let execution = execute_plan(&plan, &ws.path().join("reports").join("m"))
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

    let execution = execute_plan(&plan, &ws.path().join("reports").join("e"))
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

/// 正向：rust 档不加 `2>&1`——`cargo test > results.txt` 已重定向 stdout 进
/// 结果工件（工件落报告树、捕获面为空），`output` 主要为 stderr（编译错误面）。
#[tokio::test]
async fn rust档不加重定向合并_stdout进工件而output承载stderr() {
    let _env = env_window();
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "cmd /C echo cargo test& 1>&2 echo build-error".to_owned()
    } else {
        "echo cargo test; echo build-error >&2".to_owned()
    };
    let plan = rust_plan(&cwd, template);
    let report_dir = ws.path().join("reports").join("rust-shape");

    let execution = execute_plan(&plan, &report_dir)
        .await
        .expect("重定向注入形态应 Ok");

    assert_eq!(execution.exit_code, 0, "退出码透传");
    // 重定向注入：首个 `cargo test` 后插 `> "{results_file}"`——stdout 进工件
    let results = std::fs::read_to_string(report_dir.join("results.txt"))
        .expect("结果工件落报告树（stdout 重定向面）");
    assert_eq!(
        artifact_text(&results),
        "cargo test\n",
        "stdout 全量进结果工件（解析面单读面保持）"
    );
    // 捕获面：加上 `2>&1` 会把 stderr 混入工件面——未加即 output 只余 stderr
    let merged = String::from_utf8_lossy(&execution.output).replace("\r\n", "\n");
    assert_eq!(
        merged, "build-error\n",
        "output 承载 stderr（编译错误面），stdout 已进工件不进捕获面"
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
    let execution = execute_plan(&plan, &ws.path().join("reports").join("empty"))
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
    let execution = execute_plan(&plan, &ws.path().join("reports").join("long"))
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
}
