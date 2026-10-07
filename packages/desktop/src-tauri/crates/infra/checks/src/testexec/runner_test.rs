use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use super::{
    execute_plan, expand_template, non_zero_exit_error, output_tail, OUTPUT_TAIL_LIMIT,
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

/// 把预置文本写到 workspace 外的临时工件源（拷贝命令的 src）。
fn preset(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("写预置工件失败");
    path
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

/// 异常：裸名程序不可达 → Err 显式记因（程序解析前置——shell 吞缺失不产假
/// 产出，复用 static_check 经验）。
#[tokio::test]
async fn 裸名程序不可达err显式() {
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
}

/// 异常：非零退出且 stderr 携进程报错文本 → 错误面携带该文本（输出捕获随
/// 非零退出进错误面——rust 模板仅重定向 stdout，编译错误面在 stderr；CLI
/// runCommand 捕获 stderr 同向）。
#[tokio::test]
async fn 非零退出错误面携带进程报错文本() {
    let ws = TempDir::new().expect("临时目录");
    let cwd = ws.path().join("app");
    std::fs::create_dir_all(&cwd).expect("创建 suite cwd");
    let template = if cfg!(windows) {
        "echo compile error detail>&2 & exit /b 7".to_owned()
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
}

/// 单元：非零退出错误面拼装——携输出时「退出码 + 有界尾部」、空输出回落裸
/// 退出码措辞。
#[test]
fn 非零退出错误面_尾部拼装与空回落() {
    assert_eq!(
        non_zero_exit_error(3, ""),
        "测试命令非零退出（3）",
        "空输出回落裸退出码措辞"
    );
    assert_eq!(
        non_zero_exit_error(7, "error: could not compile\n"),
        "测试命令非零退出（7）：error: could not compile",
        "输出尾部随错误面拼装"
    );
}

/// 单元：合并输出有界尾部——短文本去尾随空白原样、超长取末段 + 省略号前缀
///（字符有界防多字节切半）。
#[test]
fn 合并输出尾部_短原样超长尾部截断() {
    assert_eq!(
        output_tail("  行一\n行二\n"),
        "  行一\n行二",
        "短文本透传（仅尾随空白剪除——行首缩进保留）"
    );
    assert_eq!(
        output_tail("  \n"),
        "",
        "空白输出为空串（裸退出码回落判据）"
    );

    let long = format!("{}\n尾行", "x".repeat(600));
    let tail = output_tail(&long);
    assert!(tail.starts_with('…'), "超长截断带省略号前缀");
    assert_eq!(
        tail.chars().count(),
        OUTPUT_TAIL_LIMIT + 1,
        "尾部 = 上限字符 + 省略号"
    );
    assert!(tail.ends_with("尾行"), "输出末段完整保留");
}

/// 边界：命令成功退出但零工件（未写 results / coverage）→ 工件解析错误面
///（不 panic 不误判绿——error 面供诊断归因）。
#[tokio::test]
async fn 成功退出零工件_解析错误面() {
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
