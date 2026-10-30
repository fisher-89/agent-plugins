use std::fs;
use std::path::{Path, PathBuf};

use config::{MutationConfig, TestFramework};

use super::super::runner::execute_plan;
use super::{detect_plans, extract_semver, is_version_at_least, to_posix, REGISTRY};
use checks::parser::CoverageFormat;

/// jest `--randomize` 最低版本语义（与 CLI `isVersionAtLeast(version, '29.5.0')`
/// 同值对照）。
const JEST_RANDOMIZE_MINIMUM: &str = "29.5.0";

// ---------------------------------------------------------------------------
// 装置：tempdir fixture 树
// ---------------------------------------------------------------------------

struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "checks-runtime-detect-test-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建 workspace 失败");
        Self(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    /// 落文件（父目录自动创建）。
    fn write(&self, rel: &str, content: &str) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().expect("相对路径应有父目录")).expect("创建目录失败");
        fs::write(&path, content).expect("写文件失败");
        path
    }

    /// 落可执行脚本（Unix 补执行位）。
    #[cfg(not(target_os = "windows"))]
    fn write_executable(&self, rel: &str, content: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = self.write(rel, content);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("补执行位失败");
        path
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn config_thresholds(lines: f64, branches: f64, functions: f64) -> config::CoverageThresholds {
    config::CoverageThresholds {
        lines,
        branches,
        functions,
    }
}

fn suite(
    root: &str,
    framework: TestFramework,
    includes: Option<Vec<String>>,
    excludes: Option<Vec<String>>,
    config: Option<String>,
) -> config::TestSuite {
    config::TestSuite {
        root: root.to_owned(),
        framework,
        cwd: ".".to_owned(),
        config,
        includes,
        excludes,
        coverage: config_thresholds(80.0, 70.0, 75.0),
        mutation: MutationConfig {
            cwd: None,
            score: 70.0,
        },
    }
}

/// 标准 fixture 树：命中 / 排除 / dot 剪枝 / 非 glob 四形态齐备。
fn probe_tree(ws: &TempWs) {
    ws.write("app/src/add.test.ts", "test");
    ws.write("app/src/nested/sub.test.ts", "test");
    ws.write("app/src/notes.md", "not a test");
    ws.write("app/.cache/hidden.test.ts", "dot 目录剪枝面");
    ws.write("app/tools/legacy.test.ts", "excludes 面");
    ws.write("crates/tests/it.rs", "test");
    ws.write("crates/src/lib.rs", "not under tests glob");
}

/// 测试文件 glob（花括号交替形态——本仓库 config 同形；方言经
/// `checks::globmatch` 展开对齐 CLI picomatch 语义）。
const TS_TESTS_GLOB: &str = "**/*.{test,spec}.{ts,tsx}";

/// rust 档新单条命令模板（shell / cmd 两臂逐字同串，占位符原样保留由 runner
/// 展开）：一条 `cargo llvm-cov nextest --workspace` 同产覆盖率 JSON
/// （`--output-path`）与测试 stdout（模板自带 `>` 重定向）。
const RUST_TEMPLATE: &str = "cargo llvm-cov nextest --workspace --json --output-path \"{coverage_file}\" > \"{results_file}\"";

/// 组合锚覆盖工件样本（llvm-cov JSON：两文件 + totals 三档——branches 计数 0
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

/// 组合锚结果工件样本（cargo test 文本：两个 `running` 块 / 4 用例行 = 2 ok +
/// 1 FAILED + 1 ignored；失败详情块置末——`parse_cargo_test` 以块头切段）。
const CARGO_TEST_OUTPUT: &str = "   Compiling dev-team v0.1.0\n\
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

/// 文本归一：CRLF → LF、逐行去尾随空白、去末尾空行（壳重定向产物差异面；
/// 工件语义不受影响）。
fn plain_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_owned()
}

/// 组合锚替身：`cargo` 可执行 shim（`--version` 探测臂直接回版本；执行臂按
/// `--output-path` 写覆盖工件、stdout 写结果工件、argv 逐行记录以对拍「零改写」）。
///
/// 目的路径经环境变量注入（命令串内的引号形态零转义风险——生产模板以
/// `> "{results_file}"` 形式携带引号）。
fn install_cargo_shim(ws: &TempWs) -> PathBuf {
    let dir = ws.root().join("shim");
    fs::create_dir_all(&dir).expect("创建 shim 目录");
    #[cfg(target_os = "windows")]
    {
        // cmd 经 PATHEXT 解析 .cmd；批处理 CRLF 行终结（LF-only 解析不稳）。
        // PowerShell 承接参数展开与写盘——`--output-path` 后的正斜杠路径直接
        // 交给 .NET API（cmd 内建命令对正斜杠路径不稳）。
        ws.write(
            "shim/cargo.cmd",
            "@echo off\r\npowershell -NoProfile -ExecutionPolicy Bypass -File \"%~dp0cargo-shim.ps1\" %*\r\nexit /b %errorlevel%\r\n",
        );
        ws.write(
            "shim/cargo-shim.ps1",
            "if ($args.Count -gt 0 -and $args[0] -eq '--version') { [Console]::Out.Write(\"cargo 1.90.0`n\"); exit 0 }\n\
             if ($env:SHIM_ARGV_LOG) { Set-Content -Path $env:SHIM_ARGV_LOG -Value $args }\n\
             for ($i = 0; $i -lt $args.Count - 1; $i++) {\n\
               if ($args[$i] -eq '--output-path' -and $env:SHIM_COV_SRC) { Copy-Item -Force $env:SHIM_COV_SRC $args[$i + 1] }\n\
             }\n\
             if ($env:SHIM_RESULTS_SRC) { [Console]::Out.Write((Get-Content -Raw $env:SHIM_RESULTS_SRC)) }\n\
             exit 0\n",
        );
    }
    #[cfg(not(target_os = "windows"))]
    {
        ws.write_executable(
            "shim/cargo",
            "#!/bin/sh\n\
             if [ \"$1\" = \"--version\" ]; then echo \"cargo 1.90.0\"; exit 0; fi\n\
             if [ -n \"$SHIM_ARGV_LOG\" ]; then\n\
               : > \"$SHIM_ARGV_LOG\"\n\
               for arg in \"$@\"; do printf '%s\\n' \"$arg\" >> \"$SHIM_ARGV_LOG\"; done\n\
             fi\n\
             prev=\"\"\n\
             for arg in \"$@\"; do\n\
               if [ \"$prev\" = \"--output-path\" ] && [ -n \"$SHIM_COV_SRC\" ]; then cp \"$SHIM_COV_SRC\" \"$arg\"; fi\n\
               prev=\"$arg\"\n\
             done\n\
             if [ -n \"$SHIM_RESULTS_SRC\" ]; then cat \"$SHIM_RESULTS_SRC\"; fi\n\
             exit 0\n",
        );
    }
    dir
}

/// 替身入参注入（PATH / 环境变量窗口内调用，调用方持 `TEST_ENV_LOCK`）。
fn set_shim_env(argv_log: &Path, coverage_src: &Path, results_src: &Path) {
    std::env::set_var("SHIM_ARGV_LOG", argv_log.as_os_str());
    std::env::set_var("SHIM_COV_SRC", coverage_src.as_os_str());
    std::env::set_var("SHIM_RESULTS_SRC", results_src.as_os_str());
}

/// 替身入参清场（窗口退出后调用，避免污染并发用例）。
fn clear_shim_env() {
    for key in ["SHIM_ARGV_LOG", "SHIM_COV_SRC", "SHIM_RESULTS_SRC"] {
        std::env::remove_var(key);
    }
}

/// PATH 隔离窗口（RAII）
struct PathWindow {
    original: Option<std::ffi::OsString>,
    _inner: std::sync::MutexGuard<'static, ()>,
}

impl PathWindow {
    fn enter() -> Self {
        let inner = crate::lock_test_env();
        Self {
            original: std::env::var_os("PATH"),
            _inner: inner,
        }
    }

    /// 置空 PATH（隔离臂：裸名程序不可达）。
    fn clear(&self) {
        std::env::set_var("PATH", "");
    }

    /// 前置目录（成功臂：shim 目录优先解析）。
    fn prepend(&self, dir: &Path) {
        let rest =
            std::env::split_paths(self.original.as_deref().unwrap_or(std::ffi::OsStr::new("")));
        let joined = std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(rest))
            .expect("join PATH");
        std::env::set_var("PATH", joined);
    }
}

impl Drop for PathWindow {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }
}

// ---------------------------------------------------------------------------
// 五框架注册表常量（与 CLI test-framework.ts 逐字对齐锚定——防漂移）
// ---------------------------------------------------------------------------

/// 正向：五框架注册表常量表（命令模板 / coverage_format / coverage_output /
/// default_glob / config_flag / version_command）与 CLI `test-framework.ts`
/// 逐字对齐锚定（jest 模板以 {randomize} 哨兵空展开形态比对——CLI builder
/// 的特性不支持臂同形态）。
#[test]
#[allow(clippy::type_complexity)]
fn 五框架注册表常量与cli逐字对齐() {
    // CLI FRAMEWORK_REGISTRY 键序（jest / vitest / vite-plus / rust / node-test
    // 为 V1 支持面；bun / go / pytest 在 detect 显式 Err 臂覆盖）
    let jest_template = "npx jest --reporters=default --json --outputFile=\"{results_file}\" --silent --coverage --coverageDirectory=\"{report_dir}\" --coverageReporters=json-summary {config_args} {files}";
    let vitest_template = "npx vitest run --sequence.shuffle --reporter=json --outputFile.json=\"{results_file}\" --coverage --coverage.reportsDirectory=\"{report_dir}\" --coverage.reporter=json-summary {config_args} {files}";
    let vite_plus_template = "vp test --sequence.shuffle --reporter=json --outputFile.json=\"{results_file}\" --coverage --coverage.reportsDirectory=\"{report_dir}\" --coverage.reporter=json-summary {config_args} {files}";
    let rust_template = RUST_TEMPLATE;
    let node_test = "node --test --experimental-test-coverage --test-reporter=spec --test-reporter-destination=\"{results_file}\" {files}";

    let expected: [(
        &str,
        &str,
        &str,
        CoverageFormat,
        &str,
        &str,
        &str,
        Option<&str>,
    ); 5] = [
        (
            "jest",
            "npx jest --version",
            jest_template,
            CoverageFormat::Istanbul,
            "coverage-summary.json",
            "results.json",
            "**/*.{test,spec}.{js,ts,jsx,tsx}",
            Some("--config"),
        ),
        (
            "vitest",
            "npx vitest --version",
            vitest_template,
            CoverageFormat::Istanbul,
            "coverage-summary.json",
            "results.json",
            "**/*.{test,spec}.{js,ts,jsx,tsx}",
            Some("--config"),
        ),
        (
            "vite-plus",
            "vp --version",
            vite_plus_template,
            CoverageFormat::Istanbul,
            "coverage-summary.json",
            "results.json",
            "**/*.{test,spec}.{js,ts,jsx,tsx}",
            Some("--config"),
        ),
        (
            "rust",
            "cargo --version",
            rust_template,
            CoverageFormat::LlvmCov,
            "coverage-summary.json",
            "results.txt",
            "**/tests/**/*.rs",
            None,
        ),
        (
            "node-test",
            "node --version",
            node_test,
            CoverageFormat::NodeTest,
            "results.txt",
            "results.txt",
            "**/*.test.{mjs,js,cjs}",
            None,
        ),
    ];

    assert_eq!(REGISTRY.len(), 5, "V1 支持面五框架封闭集");
    for (entry, want) in REGISTRY.iter().zip(expected.iter()) {
        assert_eq!(entry.framework, want.0, "框架标识同串");
        assert_eq!(entry.version_command, want.1, "{} version_command", want.0);
        assert_eq!(
            entry.shell_template.replace("{randomize}", ""),
            want.2,
            "{} shell 模板逐字（randomize 空展开）",
            want.0
        );
        assert!(
            entry.cmd_template.contains("{results_file}")
                || entry.cmd_template.contains("{coverage_file}"),
            "{} cmd 模板占位符在场",
            want.0
        );
        match (entry.coverage_format, want.3) {
            (CoverageFormat::Istanbul, CoverageFormat::Istanbul)
            | (CoverageFormat::LlvmCov, CoverageFormat::LlvmCov)
            | (CoverageFormat::NodeTest, CoverageFormat::NodeTest) => {}
            _ => panic!("{} coverage_format 漂移", want.0),
        }
        assert_eq!(entry.coverage_output, want.4, "{} coverage_output", want.0);
        assert_eq!(entry.results_output, want.5, "{} results_output", want.0);
        assert_eq!(entry.default_glob, want.6, "{} default_glob", want.0);
        assert_eq!(entry.config_flag, want.7, "{} config_flag", want.0);
    }

    // jest cmd 模板与 shell 同串（AC-7 零改动锚：四档 JS / node-test 模板不受本变更波及）
    let jest = &REGISTRY[0];
    assert_eq!(
        jest.cmd_template, jest.shell_template,
        "jest 双形态模板一致（CLI jest shell/cmd 同 builder 同串）"
    );

    // rust 两臂逐字同串（AC-1）：注册表两份字面量零差异
    assert_eq!(
        REGISTRY[3].shell_template, REGISTRY[3].cmd_template,
        "rust 双形态模板逐字同串（单条 llvm-cov 命令）"
    );
    assert_eq!(REGISTRY[3].cmd_template, rust_template);
    assert!(
        !rust_template.contains('%') && !rust_template.contains("if "),
        "rust cmd 模板不含 cmd 单行解析期展开构造（%var% / if 体）"
    );
}

/// 正向（AC-1）：rust 条目两臂逐字同串，且两臂各含 `{results_file}` 与
/// `{coverage_file}` **恰一次**（防重复重定向 / 占位符漏替换）。
#[test]
fn rust两臂逐字同串且占位符各恰一次() {
    let entry = &REGISTRY[3];
    assert_eq!(
        entry.framework, "rust",
        "注册表 rust 条目定位（键序同 CLI）"
    );
    assert_eq!(
        entry.shell_template, entry.cmd_template,
        "shell / cmd 两臂逐字同串"
    );

    for (arm_name, arm) in [("shell", entry.shell_template), ("cmd", entry.cmd_template)] {
        assert_eq!(
            arm.matches("{results_file}").count(),
            1,
            "{arm_name} 臂 `{{results_file}}` 恰一次（防重复重定向）"
        );
        assert_eq!(
            arm.matches("{coverage_file}").count(),
            1,
            "{arm_name} 臂 `{{coverage_file}}` 恰一次（防覆盖工件路径重复）"
        );
        assert_eq!(arm, RUST_TEMPLATE, "{arm_name} 臂与新单条串逐字一致");
    }
}

/// 正向（AC-1 三不变量，参数化 2 臂 × 4 项）：rust 模板不含双段链的任何残留
/// 形态——`cargo test`（第二遍测试）、` & `（cmd 无条件链分隔符）、
/// `; _X=$?`（shell 退出码暂存）、`exit $_X`（shell 退出码透传）。
#[test]
fn rust模板三不变量_无链形态残留() {
    let entry = &REGISTRY[3];
    for (arm_name, arm) in [("shell", entry.shell_template), ("cmd", entry.cmd_template)] {
        for (token, why) in [
            ("cargo test", "第二遍全量测试（重复构建）"),
            (" & ", "cmd 无条件链分隔符（退出码取末段 → 覆盖率假绿）"),
            ("; _X=$?", "shell 退出码暂存段"),
            ("exit $_X", "shell 退出码透传段"),
        ] {
            assert!(
                !arm.contains(token),
                "{arm_name} 臂不得含 {token:?}（{why}），实际: {arm}"
            );
        }
    }
}

/// 正向（AC-1 正向形态，参数化 2 臂）：两臂均以 `cargo llvm-cov nextest
/// --workspace` 起手、含 `--json`、覆盖工件经 `--output-path "{coverage_file}"`
/// 落位、以 `> "{results_file}"` 收尾；且不含 `%` / `if `（cmd 单行解析期构造）
/// 与 `--target-dir`（`target/debug` 零触碰面）。
#[test]
fn rust模板正向形态_workspace与结果重定向() {
    let entry = &REGISTRY[3];
    for (arm_name, arm) in [("shell", entry.shell_template), ("cmd", entry.cmd_template)] {
        assert!(
            arm.starts_with("cargo llvm-cov nextest --workspace"),
            "{arm_name} 臂以 `cargo llvm-cov nextest --workspace` 起手（测试框架统一为 nextest），实际: {arm}"
        );
        assert!(arm.contains("--json"), "{arm_name} 臂 `--json` 在场");
        assert!(
            arm.contains("--output-path \"{coverage_file}\""),
            "{arm_name} 臂覆盖率 JSON 经 --output-path 落位（不进 stdout），实际: {arm}"
        );
        assert!(
            arm.ends_with("> \"{results_file}\""),
            "{arm_name} 臂以结果重定向收尾（模板自带，runner 零改写），实际: {arm}"
        );
        assert!(
            !arm.contains('%') && !arm.contains("if "),
            "{arm_name} 臂不含 cmd 单行解析期展开构造（%var% / if 体），实际: {arm}"
        );
        assert!(
            !arm.contains("--target-dir"),
            "{arm_name} 臂不钉 target 目录（llvm-cov 走 target/llvm-cov-target，不触碰 target/debug），实际: {arm}"
        );
    }
}

/// 边界（D9）：rust 模板占位符子集——零出现 `{files}`（suite 的 includes /
/// excludes 只决定「是否产 plan」，不参与命令构造）、零出现 `{randomize}`
/// （rust 无版本特性分支，`--randomize` 追加逻辑只对 jest 生效）、零出现
/// `{report_dir}`（覆盖工件经 `--output-path "{coverage_file}"` 落位，命令内不
/// 引用报告目录）。
#[test]
fn rust模板占位符子集_不含files_randomize_report_dir() {
    let entry = &REGISTRY[3];
    for (arm_name, arm) in [("shell", entry.shell_template), ("cmd", entry.cmd_template)] {
        for (placeholder, why) in [
            ("{files}", "D9：includes / excludes 不参与命令构造"),
            ("{randomize}", "rust 无版本特性分支"),
            (
                "{report_dir}",
                "覆盖工件经 --output-path 落位，命令不引用报告目录",
            ),
        ] {
            assert_eq!(
                arm.matches(placeholder).count(),
                0,
                "{arm_name} 臂不含 `{placeholder}`（{why}），实际: {arm}"
            );
        }
    }
}

/// 边界（表驱动 6 字段）：rust 条目字段面不随模板改造漂移——模板串换新，其余
/// 字段（版本命令 / 覆盖格式 / 双工件名 / 缺省 glob / 配置注入旗标）逐字段等值。
#[test]
fn rust条目字段面不变_模板改造零波及() {
    let entry = &REGISTRY[3];
    let fields: [(&str, &str, &str); 5] = [
        ("framework", entry.framework, "rust"),
        ("version_command", entry.version_command, "cargo --version"),
        (
            "coverage_output",
            entry.coverage_output,
            "coverage-summary.json",
        ),
        ("results_output", entry.results_output, "results.txt"),
        ("default_glob", entry.default_glob, "**/tests/**/*.rs"),
    ];
    for (field, actual, want) in fields {
        assert_eq!(actual, want, "rust 条目字段 {field} 不随模板改造漂移");
    }
    assert!(
        matches!(entry.coverage_format, CoverageFormat::LlvmCov),
        "rust 条目覆盖格式恒为 llvm-cov"
    );
    assert_eq!(
        entry.config_flag, None,
        "rust 不支持配置注入（旗标恒 None）"
    );
}

// ---------------------------------------------------------------------------
// detect_plans：探测矩阵
// ---------------------------------------------------------------------------

/// 正向：tempdir 树 + vite-plus 形态 suite 配置（花括号 includes 命中样本
/// 文件）→ TestPlan 产出（id 沿 colocated 命名映射、cwd、
/// coverage_format=istanbul、excludes 过滤 + dot 目录剪枝后的文件清单）。
#[tokio::test]
async fn vite_plus_suite探测_清单排除与剪枝() {
    // 版本探测臂 spawn 读 PATH——持环境锁与 PATH 改写用例互斥
    let _window = PathWindow::enter();
    let ws = TempWs::new("vite-plus");
    probe_tree(&ws);

    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        Some(vec![TS_TESTS_GLOB.to_owned()]),
        Some(vec!["tools/**".to_owned()]),
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("vite-plus suite 应探测成功");

    assert_eq!(plans.len(), 1, "单 suite 单 plan");
    let plan = &plans[0];
    assert_eq!(
        plan.id, "app_vite-plus",
        "plan id 沿 derive_plan_id 命名映射"
    );
    assert_eq!(plan.framework, "vite-plus");
    assert_eq!(plan.root, "app");
    assert_eq!(plan.cwd, ws.root().join("app"), "cwd = suite root 绝对路径");
    assert_eq!(
        plan.files,
        vec!["src/add.test.ts", "src/nested/sub.test.ts"],
        "includes 命中、tools/** 排除、.cache dot 目录剪枝、BTreeSet 字典序稳定"
    );
    assert!(
        matches!(plan.coverage_format, CoverageFormat::Istanbul),
        "istanbul 族覆盖格式"
    );
    assert_eq!(plan.coverage_output, "coverage-summary.json");
    assert_eq!(plan.results_output, "results.json");
    assert!(plan.config_args.is_none(), "未声明 config 不注入");
    assert!(plan.template().contains("vp test"), "平台模板选臂含命令头");
    assert!(
        plan.template().contains("{results_file}"),
        "占位符原样保留由 runner 展开"
    );
}

/// 正向：config 注入（vitest suite 声明 config）→ `{config_args}` 展开原料
/// 拼接（旗标 + suite root 下绝对 POSIX 路径）。
#[tokio::test]
async fn config注入_旗标与绝对posix路径拼接() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("config-inject");
    ws.write("app/vitest.config.ts", "export default {};");
    ws.write("app/src/a.test.ts", "test");

    let suites = vec![suite(
        "app",
        TestFramework::Vitest,
        Some(vec![TS_TESTS_GLOB.to_owned()]),
        None,
        Some("vitest.config.ts".to_owned()),
    )];
    let plans = detect_plans(ws.root(), &suites).expect("探测应成功");

    let plan = &plans[0];
    assert_eq!(plan.framework, "vitest");
    assert_eq!(
        plan.config_args.as_deref(),
        Some(format!("--config \"{}/app/vitest.config.ts\"", to_posix(ws.root())).as_str()),
        "旗标 + suite root 下配置文件绝对 POSIX 路径"
    );
}

/// 正向：rust suite → plan（coverage_format=llvm-cov、命令模板为单条
/// `cargo llvm-cov nextest --workspace` 新串、default_glob 圈定 tests 树——rust
/// 档缺省 glob 天然无花括号；字段面（双工件名 / 版本命令 / 配置旗标）零漂移）。
#[tokio::test]
async fn rust_suite探测_llvm_cov档() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("rust");
    probe_tree(&ws);

    let suites = vec![suite("crates", TestFramework::Rust, None, None, None)];
    let plans = detect_plans(ws.root(), &suites).expect("rust suite 应探测成功");

    assert_eq!(plans.len(), 1);
    let plan = &plans[0];
    assert_eq!(plan.id, "crates_rust");
    assert_eq!(plan.framework, "rust");
    assert!(
        matches!(plan.coverage_format, CoverageFormat::LlvmCov),
        "rust 档 llvm-cov 覆盖格式"
    );
    assert_eq!(plan.coverage_output, "coverage-summary.json");
    assert_eq!(plan.results_output, "results.txt");
    assert_eq!(
        plan.files,
        vec!["tests/it.rs"],
        "default_glob **/tests/**/*.rs 圈定（src 树不命中）；cwd 相对 POSIX 清单"
    );
    assert_eq!(
        plan.shell_template, RUST_TEMPLATE,
        "shell 模板为新单条 llvm-cov 命令且占位符原样保留（由 runner 展开）"
    );
    assert_eq!(
        plan.cmd_template, RUST_TEMPLATE,
        "cmd 模板为同一串（两臂逐字同串，平台选臂零行为差异）"
    );
    assert!(
        plan.shell_template.contains("{coverage_file}")
            && plan.shell_template.contains("{results_file}"),
        "双工件占位符原样保留由 runner 展开"
    );
    assert!(
        !plan.shell_template.contains("cargo test") && !plan.shell_template.contains("; _X=$?"),
        "探测产出的 rust 模板已退役双段链形态，实际: {}",
        plan.shell_template
    );
    assert!(
        plan.config_args.is_none(),
        "rust 不支持配置注入（未声明即无）"
    );
}

/// 边界：平台选臂等价——`plan.template()` 在 Windows 取 `cmd_template`、其余取
/// `shell_template`；因两臂同串，rust 档平台切换零行为差异。
#[tokio::test]
async fn 平台选臂等价_rust两臂同串切换零差异() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("rust-arm");
    probe_tree(&ws);

    let suites = vec![suite("crates", TestFramework::Rust, None, None, None)];
    let plans = detect_plans(ws.root(), &suites).expect("rust suite 应探测成功");
    let plan = &plans[0];

    let expected_arm = if cfg!(windows) {
        plan.cmd_template.as_str()
    } else {
        plan.shell_template.as_str()
    };
    assert_eq!(
        plan.template(),
        expected_arm,
        "平台选臂语义（Windows → cmd 模板配 cmd /C，其余 → shell 模板配 sh -c）"
    );
    assert_eq!(
        plan.template(),
        RUST_TEMPLATE,
        "两臂同串 → 平台选臂结果恒等（切换零行为差异）"
    );
}

/// 回归锚（vite-plus 静默跳过缺陷面）：缺省 default_glob 花括号形态
/// （`**/*.{test,spec}.{js,ts,jsx,tsx}`）命中 .test / .spec 多扩展样本 →
/// suite 产 plan（修复前 `glob` crate 无交替方言恒零命中 → 静默不产 plan，
/// 与 CLI plan 集漂移）。
#[tokio::test]
async fn 缺省glob花括号形态命中_产plan不静默跳过() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("default-brace");
    ws.write("app/src/a.test.ts", "test");
    ws.write("app/src/b.spec.tsx", "test");
    ws.write("app/src/c.test.js", "test");
    ws.write("app/src/d.notes.md", "not a test");

    let suites = vec![suite("app", TestFramework::VitePlus, None, None, None)];
    let plans = detect_plans(ws.root(), &suites).expect("缺省 glob 花括号形态应探测成功");

    assert_eq!(plans.len(), 1, "命中样本在场 → 产 plan（修复前静默跳过）");
    assert_eq!(
        plans[0].files,
        vec!["src/a.test.ts", "src/b.spec.tsx", "src/c.test.js"],
        "交替选枝 .test / .spec 与 .ts / .tsx / .js 全命中；非测试扩展不命中"
    );
}

/// 花括号方言 includes / excludes 双面 + 依赖目录剪枝（node_modules / dist
/// ——CLI `collectFiles` excludedDirs 同集的非 dot 成员）。
#[tokio::test]
async fn 花括号includes与excludes_依赖目录剪枝() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("brace-dialect");
    ws.write("app/src/a.ts", "hit");
    ws.write("app/src/b.tsx", "hit");
    ws.write("app/src/legacy/x.ts", "brace excludes");
    ws.write("app/src/old/y.ts", "brace excludes");
    ws.write("app/node_modules/pkg/z.ts", "依赖目录剪枝");
    ws.write("app/dist/w.tsx", "依赖目录剪枝");
    ws.write("app/src/notes.md", "不命中");

    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        Some(vec!["**/*.{ts,tsx}".to_owned()]),
        Some(vec!["src/{legacy,old}/**".to_owned()]),
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("花括号方言应探测成功");

    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].files,
        vec!["src/a.ts", "src/b.tsx"],
        "includes 花括号命中；excludes 花括号剪除 legacy / old；node_modules / dist 剪枝"
    );
}

#[tokio::test]
async fn dot前缀祖先目录_glob结果不被误剪() {
    let _window = PathWindow::enter();
    let base = std::env::temp_dir().join(format!(
        ".hidden-ancestor-{}-checks-runtime-detect",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&base);
    let root = base.join("ws");
    fs::create_dir_all(root.join("app").join("src")).expect("创建 workspace");
    fs::write(root.join("app").join("src").join("a.test.ts"), "test").expect("写测试样本");
    fs::create_dir_all(root.join("app").join(".cache")).expect("创建树内 dot 目录");
    fs::write(root.join("app").join(".cache").join("b.test.ts"), "test").expect("写剪枝样本");

    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        Some(vec![TS_TESTS_GLOB.to_owned()]),
        None,
        None,
    )];
    let plans = detect_plans(&root, &suites).expect("dot 祖先目录下探测应成功");

    assert_eq!(plans.len(), 1, "祖先 dot 分量不得剪枝（修复前零 plan）");
    assert_eq!(
        plans[0].files,
        vec!["src/a.test.ts"],
        "树内 .cache 剪枝保持执法；清单仅含 src 样本"
    );
    let _ = fs::remove_dir_all(&base);
}

/// 异常（遍历枚举 3 值）：bun / go / pytest suite 配置 → 显式 Err 记因（无静默
/// 跳过分支；rust 模板改造未波及 `registry_entry` 路径）。
#[tokio::test]
async fn 不支持框架显式err不静默() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("unsupported");
    probe_tree(&ws);

    for (framework, name) in [
        (TestFramework::Bun, "bun"),
        (TestFramework::Go, "go"),
        (TestFramework::Pytest, "pytest"),
    ] {
        let suites = vec![suite("app", framework, None, None, None)];
        let err = detect_plans(ws.root(), &suites)
            .expect_err(&format!("{name} 应显式 Err（不静默跳过）"));
        assert!(
            err.contains(&format!("\"{name}\"")) && err.contains("不被支持"),
            "Err 记因指认框架名（{name}），实际: {err}"
        );
    }
}

/// 异常：rust suite 声明 config → 显式 Err 记因（`config_flag` 恒 `None`——
/// 模板改造未放宽配置注入校验）。
#[tokio::test]
async fn rust_suite声明config_显式err记因root与框架() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("rust-config");
    probe_tree(&ws);

    let suites = vec![suite(
        "crates",
        TestFramework::Rust,
        None,
        None,
        Some("Cargo.toml".to_owned()),
    )];
    let err = detect_plans(ws.root(), &suites).expect_err("不支持注入声明 config 应 Err");
    assert!(
        err.contains("不支持配置注入")
            && err.contains("crates")
            && err.contains("rust")
            && err.contains("Cargo.toml"),
        "配置注入 Err 记因 suite root / 框架名 / config 值（CLI validateSuiteConfig 同语义），实际: {err}"
    );
}

/// 边界（表驱动 2 形态）：rust suite 零命中不产 plan 且不 panic——目录下无
/// `**/tests/**/*.rs`（src 树不命中 rust 缺省 glob）与显式 includes 全不命中两
/// 形态；混合 suite 时仅命中 suite 产 plan 且 rust 模板仍为新串。
#[tokio::test]
async fn rust_suite零命中_不产plan_混合形态仅命中产plan() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("rust-zero-hit");
    // crates 树只有 src（rust 缺省 glob `**/tests/**/*.rs` 不命中）
    ws.write("crates/src/lib.rs", "pub fn add() {}");

    let suites = vec![suite("crates", TestFramework::Rust, None, None, None)];
    let plans = detect_plans(ws.root(), &suites).expect("零命中应 Ok（空清单面）");
    assert!(
        plans.is_empty(),
        "目录下无 tests/**/*.rs → 不产 plan（整体不 panic）"
    );

    // 显式 includes 全不命中同族
    let suites = vec![suite(
        "crates",
        TestFramework::Rust,
        Some(vec!["**/*.nope".to_owned()]),
        None,
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("显式 includes 零命中应 Ok");
    assert!(plans.is_empty(), "显式 includes 全不命中同样不产 plan");

    // 混合：rust 命中 + vite-plus 零命中 → 仅 rust 产 plan，模板仍为新串
    probe_tree(&ws);
    let suites = vec![
        suite(
            "app",
            TestFramework::VitePlus,
            Some(vec!["**/*.nope".to_owned()]),
            None,
            None,
        ),
        suite("crates", TestFramework::Rust, None, None, None),
    ];
    let plans = detect_plans(ws.root(), &suites).expect("混合形态应 Ok");
    assert_eq!(plans.len(), 1, "仅命中 suite 产 plan");
    assert_eq!(plans[0].framework, "rust");
    assert_eq!(
        plans[0].shell_template, RUST_TEMPLATE,
        "混合形态下 rust 模板仍为新单条串"
    );
    assert_eq!(plans[0].files, vec!["tests/it.rs"], "rust 命中清单");
}

/// 边界：includes 全不命中 → 该 suite 不产 plan（空清单面）且整体不 panic。
#[tokio::test]
async fn includes全不命中_不产plan不panic() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("no-match");
    probe_tree(&ws);

    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        Some(vec!["**/*.nope".to_owned()]),
        None,
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("全不命中应 Ok（空清单面）");
    assert!(plans.is_empty(), "零命中 suite 不产 plan");

    // 多 suite 混合：零命中 suite 跳过、命中 suite 照常产出
    let suites = vec![
        suite(
            "app",
            TestFramework::VitePlus,
            Some(vec!["**/*.nope".to_owned()]),
            None,
            None,
        ),
        suite("crates", TestFramework::Rust, None, None, None),
    ];
    let plans = detect_plans(ws.root(), &suites).expect("混合应 Ok");
    assert_eq!(plans.len(), 1, "仅命中 suite 产 plan");
    assert_eq!(plans[0].framework, "rust");
}

// ---------------------------------------------------------------------------
// 组合锚：detect_plans → execute_plan（真注册表模板 + cargo 替身）
// ---------------------------------------------------------------------------

/// 正向（AC-2 + AC-4 链路锚）：真实 `detect_plans` 产出的 rust `TestPlan` →
/// PATH 前置 `cargo` 替身真实执行 → 展开后命令 argv 与模板展开逐字一致
/// （零注入：无 `cargo test`、无二次重定向）→ `results.txt`（stdout 重定向）与
/// `coverage-summary.json`（`--output-path`）由同一条命令落位 → 两读取面各自
/// 解析出用例行与实测块。
#[tokio::test]
async fn detect_plans到execute_plan组合_单条命令零改写双工件同落位() {
    let ws = TempWs::new("rust-chain");
    ws.write("crates/tests/it.rs", "#[test] fn it_works() {}");
    let cov_src = ws.write("preset/coverage-summary.json", LLVM_COV_OUTPUT);
    let res_src = ws.write("preset/results.txt", CARGO_TEST_OUTPUT);
    let argv_log = ws.root().join("shim").join("argv.log");
    let shim_dir = install_cargo_shim(&ws);
    let report_dir = ws.root().join("reports").join("crates_rust");
    let suites = vec![suite("crates", TestFramework::Rust, None, None, None)];

    // 探测臂：版本探测经替身（`cargo --version`）真实 spawn
    let plans = {
        let window = PathWindow::enter();
        window.prepend(&shim_dir);
        set_shim_env(&argv_log, &cov_src, &res_src);
        let plans = detect_plans(ws.root(), &suites);
        drop(window);
        clear_shim_env();
        plans.expect("rust suite 应探测成功")
    };
    assert_eq!(plans.len(), 1, "单 suite 单 plan");
    let plan = &plans[0];
    assert_eq!(plan.framework, "rust");
    assert_eq!(plan.id, "crates_rust");
    assert_eq!(
        (plan.shell_template.as_str(), plan.cmd_template.as_str()),
        (RUST_TEMPLATE, RUST_TEMPLATE),
        "探测产出即新单条模板（两臂逐字同串）"
    );

    // 执行臂：同一条命令由 PATH 上的替身承接
    let execution = {
        let window = PathWindow::enter();
        window.prepend(&shim_dir);
        set_shim_env(&argv_log, &cov_src, &res_src);
        let execution =
            execute_plan(plan, &report_dir, foundation::cancel::CancelFlag::never()).await;
        drop(window);
        clear_shim_env();
        execution.expect("替身命令应成功退出")
    };

    assert_eq!(execution.exit_code, 0, "退出码直继承（单条命令）");
    assert!(
        execution.error.is_none(),
        "绿跑零 error 面，实际: {:?}",
        execution.error
    );

    // 零改写：展开后的命令 argv = program + 模板逐参（无注入 token、重定向仅模板一次）
    let expected_argv = vec![
        "llvm-cov".to_owned(),
        "nextest".to_owned(),
        "--workspace".to_owned(),
        "--json".to_owned(),
        "--output-path".to_owned(),
        to_posix(&report_dir.join("coverage-summary.json")),
    ];
    let observed = fs::read_to_string(&argv_log).expect("替身 argv 记录落盘");
    let observed: Vec<String> = observed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        observed, expected_argv,
        "展开后命令串零改写（无 `cargo test` 注入；`> \"{{results_file}}\"` 由壳承载不入 argv）"
    );

    // 双工件同落位 + 两读取面解析
    let results = fs::read_to_string(report_dir.join("results.txt")).expect("结果工件落报告树");
    assert_eq!(
        plain_text(&results),
        plain_text(CARGO_TEST_OUTPUT),
        "stdout 全量进结果工件（模板自带重定向，单次承载）"
    );
    assert!(
        report_dir.join("coverage-summary.json").is_file(),
        "覆盖工件由同一命令 `--output-path` 落位"
    );
    assert_eq!(execution.cases.len(), 4, "多 running 块用例行全量解析");
    assert_eq!(
        execution.source_files.len(),
        2,
        "llvm-cov 逐文件覆盖原始计数（两文件条目）"
    );
    let measured = execution.measured.expect("覆盖实测块在场");
    assert_eq!(measured.lines, Some(75.0), "lines 取 totals 实测");
    assert_eq!(measured.functions, Some(100.0), "functions 取 totals 实测");
    assert_eq!(
        measured.branches, None,
        "llvm-cov 分支计数 0 → null 感知跳过（非假绿）"
    );
    assert!(
        execution.output.is_empty(),
        "stdout 已进工件、stderr 无输出 → 捕获面为空，实际: {:?}",
        String::from_utf8_lossy(&execution.output)
    );
}

// ---------------------------------------------------------------------------
// jest 版本探测（真实组合：成功臂 shim / 失败臂 PATH 隔离）
// ---------------------------------------------------------------------------

/// 边界：is_version_at_least / extract_semver 直接对齐（CLI 同语义——任一侧
/// 不可解析 → false、逐段数值比较、恰等达标）。
#[test]
fn jest版本比较语义与cli同型() {
    assert!(
        is_version_at_least("29.7.0", JEST_RANDOMIZE_MINIMUM),
        "高版本达标"
    );
    assert!(
        is_version_at_least("29.5.0", JEST_RANDOMIZE_MINIMUM),
        "恰等达标（≥）"
    );
    assert!(
        !is_version_at_least("29.4.9", JEST_RANDOMIZE_MINIMUM),
        "低版本不达标"
    );
    assert!(
        !is_version_at_least("28.9.9", JEST_RANDOMIZE_MINIMUM),
        "跨主版本不达标"
    );
    assert!(
        !is_version_at_least("", JEST_RANDOMIZE_MINIMUM),
        "空版本按特性不支持"
    );
    assert!(
        !is_version_at_least("not-a-version", JEST_RANDOMIZE_MINIMUM),
        "无 semver 不可解析"
    );
    assert_eq!(
        extract_semver("jest 30.1.2 (survey)").as_deref(),
        Some("30.1.2"),
        "文本中首个 semver 提取"
    );
    assert_eq!(extract_semver("no digits here"), None);
}

/// jest suite 版本探测端到端：版本 ≥ 29.5.0（真实 shim 输出 29.7.0）→ 模板
/// 追加 ` --randomize`；探测失败（PATH 隔离臂，shim / 真实 npx 均不可达）→
/// 空版本按「特性不支持」走基础模板，不失败不报错。
#[tokio::test]
async fn jest版本探测_randomize追加与失败基础模板() {
    // 成功臂：Windows 在 suite cwd 落 npx.cmd（cmd /C 先搜 cwd）；Unix 经 PATH
    // 窗口前置 shim 目录（sh -c 按 PATH 解析裸名）
    let ws = TempWs::new("jest-randomize");
    ws.write("app/src/a.test.ts", "test");
    #[cfg(target_os = "windows")]
    ws.write("app/npx.cmd", "@echo 29.7.0\r\n");
    #[cfg(not(target_os = "windows"))]
    ws.write_executable("app/npx", "#!/bin/sh\necho 29.7.0\n");

    let suites = vec![suite(
        "app",
        TestFramework::Jest,
        Some(vec![TS_TESTS_GLOB.to_owned()]),
        None,
        None,
    )];
    // 全平台 PATH 窗口前置 shim 目录：Unix sh -c 按 PATH 解析裸名；Windows
    // cmd 按 PATH（含 PATHEXT .cmd 候选）解析——不依赖 cwd 搜索（
    // NoDefaultCurrentDirectoryInExePath 环境下 cmd 不搜 cwd）
    let plans = {
        let window = PathWindow::enter();
        window.prepend(&ws.root().join("app"));
        let plans = detect_plans(ws.root(), &suites);
        drop(window);
        plans.expect("jest suite 应探测成功")
    };
    assert_eq!(plans.len(), 1);
    assert!(
        plans[0].shell_template.contains(" --randomize"),
        "版本 29.7.0 ≥ 29.5.0 → 模板追加 --randomize，实际: {}",
        plans[0].shell_template
    );
    assert!(
        !plans[0].shell_template.contains("{randomize}"),
        "哨兵已展开（不残留）"
    );

    // 失败臂：PATH 隔离（裸名不可达）→ 空版本 → 基础模板（不失败不报错）
    let bare = TempWs::new("jest-no-version");
    bare.write("app/src/a.test.ts", "test");
    let suites = vec![suite(
        "app",
        TestFramework::Jest,
        Some(vec![TS_TESTS_GLOB.to_owned()]),
        None,
        None,
    )];
    let plans = {
        let window = PathWindow::enter();
        window.clear();
        let plans = detect_plans(bare.root(), &suites);
        drop(window);
        plans.expect("探测失败臂应 Ok（特性不支持语义，不失败不报错）")
    };
    assert_eq!(plans.len(), 1);
    assert!(
        !plans[0].shell_template.contains("--randomize"),
        "探测失败 → 基础模板（无 --randomize），实际: {}",
        plans[0].shell_template
    );
    assert!(
        plans[0].shell_template.contains("{config_args}"),
        "占位符完整保留"
    );
}

// ---------------------------------------------------------------------------
// 剪枝单源化（check-before-flow-start）：is_pruned_path 组件判定委托
// foundation::prune::is_pruned_dir_name（私有名单退役零行为漂移）；两名单边界
// 锚（Mock策略：无——纯函数 + fixture 树直测）
// ---------------------------------------------------------------------------

/// 单源对拍（边界）：同一批路径组件两侧对拍——detect 剪枝结果与
/// `foundation::prune::is_pruned_dir_name` 逐组件等值（无第二份名单的结构证明；
/// 路径组件迭代语义驻 detect 消费点——嵌套路径逐组件 any 语义在案）。
#[test]
fn is_pruned_path单源对拍_与foundation名单逐组件等值() {
    // 逐组件对拍：名单六员 + dot 形态 + 普通名 + 形态近邻
    for component in [
        "node_modules",
        "dist",
        "build",
        "target",
        "coverage",
        ".nyc_output",
        ".git",
        ".cache",
        "src",
        "app",
        "openspec",
        "node_module",
        "Build",
    ] {
        let foundation_verdict = foundation::prune::is_pruned_dir_name(component);
        let detect_verdict = super::is_pruned_path(Path::new(component));
        assert_eq!(
            foundation_verdict, detect_verdict,
            "组件 {component:?} 两面判定等值（单源对拍）"
        );
    }

    // 路径形态对拍：嵌套组件迭代语义（`app/node_modules/x.ts` → node_modules
    // 组件命中 → 整路径剪枝）
    assert!(
        super::is_pruned_path(Path::new("app/node_modules/pkg/z.ts")),
        "嵌套路径组件迭代命中（依赖目录不入探测面）"
    );
    assert!(
        !super::is_pruned_path(Path::new("app/src/z.test.ts")),
        "普通路径零剪枝"
    );

    // 封闭集等值：foundation 名单即 detect 全部非 dot 剪枝词（六员逐员经
    // detect 面同判 true）
    for name in foundation::prune::PRUNED_DIR_NAMES {
        assert!(
            super::is_pruned_path(Path::new(name)),
            "foundation 名单成员 {name} 经 detect 面同判剪枝"
        );
    }
}

/// 两名单不串味（边界）：复用门扫描 `EXCLUDED_DIR_NAMES`（mod.rs 私有 const——
/// CLI `REUSE_SCAN_EXCLUDED_DIRS` 同源独立声明）与 foundation `PRUNED_DIR_NAMES`
/// 单源名单**独立集**：交集五员（依赖树目录双面排除语义一致）、各有独有成员
///（复用门独有 `_stryker-tmp` 突变临时目录、foundation 独有 `.nyc_output` dot
/// 形态成员）——两名单各自封闭、零共享引用（detect 内两名单边界锚）。
#[test]
fn 复用门扫描名单与foundation名单独立集() {
    // 复用门名单（mod.rs 私有 const——同 crate 后代子模块可读）
    let reuse_scan: [&str; 6] = super::super::EXCLUDED_DIR_NAMES;
    let pruned = foundation::prune::PRUNED_DIR_NAMES;

    // 交集五员（依赖树目录双面排除语义一致）
    let shared = ["node_modules", "dist", "build", "target", "coverage"];
    for name in shared {
        assert!(
            reuse_scan.contains(&name) && pruned.contains(&name),
            "交集成员 {name:?} 双名单同在（依赖目录双面排除）"
        );
    }

    // 各有独有成员（互异边界——集合不等值）
    assert!(
        reuse_scan.contains(&"_stryker-tmp") && !pruned.contains(&"_stryker-tmp"),
        "复用门独有 _stryker-tmp（突变临时目录不入剪枝名单）"
    );
    assert!(
        pruned.contains(&".nyc_output") && !reuse_scan.contains(&".nyc_output"),
        "foundation 独有 .nyc_output（dot 形态成员——dot 判定外显式收录）"
    );

    // foundation 集完整在案（六员锚随单源化持衡）
    assert_eq!(pruned.len(), 6);
    assert_eq!(reuse_scan.len(), 6);
}

// ---------------------------------------------------------------------------
// 降级安全判据（`TestPlan.omit_files_safe`——detect 期一次算定的对照枚举，
// D7 / AC-3 / AC-4；Mock策略：不 mock——「同一管线枚举」正是被测语义，mock 会
// 抹掉判据，故全部用例走 `TempWs` 真实 tempdir 树 + 真实 glob + 真实剪枝名单）
// ---------------------------------------------------------------------------

/// 对照面样本树：缺省 glob 两形态命中 + 依赖目录剪枝 + dot 目录剪枝 + 非测试
/// 文件（对照集合与 `files` 集合的唯一差异来源是 `excludes`，故本树不含 excludes
/// 命中面）。
fn omit_tree(ws: &TempWs) {
    ws.write("app/src/a.test.ts", "test");
    ws.write("app/src/b.spec.tsx", "test");
    ws.write("app/src/notes.md", "not a test");
    ws.write("app/node_modules/pkg/x.test.ts", "依赖目录剪枝面");
    ws.write("app/.cache/hidden.test.ts", "dot 目录剪枝面");
}

/// 正向：`includes` 未设（吃注册表 `default_glob`）→ 对照集合 == `files` 集合
/// → `omit_files_safe == true`；树内 `node_modules` / `.cache` 剪枝目录两侧同剪
/// （列入对照面不产生差异）。
#[tokio::test]
async fn 降级安全判据_缺省glob同管线枚举为可降级() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("omit-default");
    omit_tree(&ws);

    let suites = vec![suite("app", TestFramework::VitePlus, None, None, None)];
    let plans = detect_plans(ws.root(), &suites).expect("缺省 glob 探测应成功");

    assert_eq!(plans.len(), 1, "单 suite 单 plan");
    assert_eq!(
        plans[0].files,
        vec!["src/a.test.ts", "src/b.spec.tsx"],
        "缺省 glob 命中 .test / .spec 两形态；剪枝目录不入清单"
    );
    assert!(
        plans[0].omit_files_safe,
        "两侧同管线（同 glob 方言 + 同剪枝）且无 excludes → 集合相等 → 可安全降级"
    );
}

/// 正向：`includes` 显式写成与 `default_glob` 等价的同一花括号串
/// （`**/*.{test,spec}.{ts,tsx}`）→ 集合相等 → `true`（显式声明不改变判据
/// ——判据比的是集合而非配置形态）。
#[tokio::test]
async fn 降级安全判据_显式等价includes仍可降级() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("omit-explicit");
    omit_tree(&ws);

    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        Some(vec![TS_TESTS_GLOB.to_owned()]),
        None,
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("显式 includes 探测应成功");

    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].files,
        vec!["src/a.test.ts", "src/b.spec.tsx"],
        "显式花括号 includes 命中同一批文件"
    );
    assert!(
        plans[0].omit_files_safe,
        "显式 includes 与 default_glob 集合相等 → 仍可安全降级"
    );
}

/// 异常：`includes` 为**真子集**（仅 `**/*.test.ts`，同目录另有 `src/b.spec.tsx`
/// 命中 `default_glob`）→ 集合不等 → `false`（保守方向：降级会少跑文件，故拒）
#[tokio::test]
async fn 降级安全判据_includes真子集拒降级() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("omit-subset");
    omit_tree(&ws);

    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        Some(vec!["**/*.test.ts".to_owned()]),
        None,
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("子集 includes 探测应成功");

    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].files,
        vec!["src/a.test.ts"],
        "发现面仅 .test.ts（真子集）"
    );
    assert!(
        !plans[0].omit_files_safe,
        "对照面（default_glob）仍含 src/b.spec.tsx → 集合不等 → 拒降级"
    );
}

/// 异常：`includes` 等价但 `excludes` **实际剔除**一个文件 → `files` 少一项、
/// 对照面（**不施加 excludes**）仍含该文件 → `false`。
#[tokio::test]
async fn 降级安全判据_excludes实际剔除拒降级() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("omit-excluded");
    omit_tree(&ws);

    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        Some(vec![TS_TESTS_GLOB.to_owned()]),
        Some(vec!["**/*.spec.tsx".to_owned()]),
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("excludes 探测应成功");

    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].files,
        vec!["src/a.test.ts"],
        "发现面施加 excludes：b.spec.tsx 被剔除"
    );
    assert!(
        !plans[0].omit_files_safe,
        "对照面 MUST NOT 施加 excludes（仍含 b.spec.tsx）→ 集合不等 → 拒降级"
    );
}

/// 边界：`excludes` 已声明但**零剔除** → 集合仍相等 → `true`（「excludes 在位」
/// 不等于「必然不安全」——判据是集合 equality，不是配置存在性）。
#[tokio::test]
async fn 降级安全判据_excludes零剔除仍可降级() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("omit-excludes-noop");
    omit_tree(&ws);

    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        None,
        Some(vec!["src/**/*.nope.ts".to_owned()]),
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("零剔除 excludes 探测应成功");

    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].files,
        vec!["src/a.test.ts", "src/b.spec.tsx"],
        "零剔除 excludes 不改清单"
    );
    assert!(
        plans[0].omit_files_safe,
        "excludes 在位但零剔除 → 集合仍相等 → 可安全降级"
    );
}

/// 边界（表驱动两形态）：清单规模——1 个文件 / 200 个文件 → 均 `true`
/// （两侧 `BTreeSet` 序导出后逐项比较，规模不影响判据）。
#[tokio::test]
async fn 降级安全判据_清单规模不影响() {
    let _window = PathWindow::enter();

    // 形态一：单文件
    let single = TempWs::new("omit-size-one");
    single.write("app/src/only.test.ts", "test");
    let suites = vec![suite("app", TestFramework::VitePlus, None, None, None)];
    let plans = detect_plans(single.root(), &suites).expect("单文件探测应成功");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].files.len(), 1, "单文件清单");
    assert!(plans[0].omit_files_safe, "单文件两侧集合相等 → 可降级");

    // 形态二：200 文件（字典序稳定、逐项等值）
    let bulk = TempWs::new("omit-size-bulk");
    for index in 0..200 {
        bulk.write(&format!("app/src/case{index:03}.test.ts"), "test");
    }
    let mut ordered: Vec<String> = (0..200)
        .map(|index| format!("src/case{index:03}.test.ts"))
        .collect();
    ordered.sort();
    let plans = detect_plans(bulk.root(), &suites).expect("大清单探测应成功");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].files, ordered, "200 项全量（BTreeSet 字典序）");
    assert!(plans[0].omit_files_safe, "大清单两侧集合相等 → 可降级");
}

/// 边界：`includes` 零命中（`files == []`）→ 沿既有早跳**不产 plan**（无
/// `omit_files_safe` 面、零 panic；对照枚举不执行）。
#[tokio::test]
async fn 降级安全判据_零命中早跳不产plan() {
    let _window = PathWindow::enter();

    // 形态一：显式 includes 零命中（树内有测试文件但都不匹配）
    let ws = TempWs::new("omit-zero-hit");
    omit_tree(&ws);
    let suites = vec![suite(
        "app",
        TestFramework::VitePlus,
        Some(vec!["**/*.nope".to_owned()]),
        None,
        None,
    )];
    let plans = detect_plans(ws.root(), &suites).expect("零命中应 Ok（空清单面）");
    assert!(
        plans.is_empty(),
        "显式 includes 零命中 → files 空 → 早跳不产 plan（无 omit_files_safe 面）"
    );

    // 形态二：default_glob 零命中（树内无任何测试扩展文件）
    let bare = TempWs::new("omit-zero-hit-default");
    bare.write("app/src/notes.md", "not a test");
    let suites = vec![suite("app", TestFramework::VitePlus, None, None, None)];
    let plans = detect_plans(bare.root(), &suites).expect("default_glob 零命中应 Ok");
    assert!(
        plans.is_empty(),
        "default_glob 零命中 → 同样不产 plan（零 panic、零对照枚举）"
    );
}

/// 边界（组合面）：花括号多形态 + dot 目录剪枝 + 依赖目录剪枝 + 项目根相对
/// 口径全部在场 → 对照集合与 `files` 同源（零新 glob 方言、零新剪枝）。
#[tokio::test]
async fn 降级安全判据_花括号与剪枝组合同源() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("omit-combined");
    ws.write("app/src/a.test.js", "test");
    ws.write("app/src/b.spec.jsx", "test");
    ws.write("app/src/deep/c.test.ts", "test");
    ws.write("app/src/deep/d.spec.tsx", "test");
    ws.write("app/.cache/hidden.test.ts", "dot 目录剪枝面");
    ws.write("app/node_modules/pkg/dep.spec.js", "依赖目录剪枝面");
    ws.write("app/dist/built.test.js", "工具产物目录剪枝面");
    ws.write("app/src/e.helper.js", "非测试扩展");

    let suites = vec![suite("app", TestFramework::VitePlus, None, None, None)];
    let plans = detect_plans(ws.root(), &suites).expect("组合面探测应成功");

    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].files,
        vec![
            "src/a.test.js",
            "src/b.spec.jsx",
            "src/deep/c.test.ts",
            "src/deep/d.spec.tsx",
        ],
        "四扩展 × 两形态 × 多层目录全命中；dot / 依赖 / 产物目录三面全剪"
    );
    assert!(
        plans[0].omit_files_safe,
        "对照面同源（同花括号展开 + 同剪枝）→ 集合相等 → 可降级"
    );
}

/// 正向（`collect_files` 的 `apply_excludes` 两态，经 `detect_plans` 观测）：
/// 同一树上 excludes 命中的文件在发现面（`apply_excludes = true`）被剔除、在
/// 对照面（`apply_excludes = false`）仍在案——两态共用同一 glob 方言与剪枝，
/// 差异只由末位开关产生（构成「excludes 实际剔除 → 拒降级」的差异来源）。
#[tokio::test]
async fn 收集主体_excludes开关两态同源() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("collect-switch");
    ws.write("app/src/a.test.ts", "test");
    ws.write("app/src/legacy.test.ts", "excludes 命中面");

    // 两 suite 仅 excludes 一位不同（root / framework / includes 全同）
    let suites = vec![
        suite("app", TestFramework::VitePlus, None, None, None),
        suite(
            "app",
            TestFramework::VitePlus,
            None,
            Some(vec!["src/legacy.test.ts".to_owned()]),
            None,
        ),
    ];
    let plans = detect_plans(ws.root(), &suites).expect("两位差异 suite 探测应成功");
    assert_eq!(plans.len(), 2, "两 suite 各产 plan");

    assert_eq!(
        plans[0].files,
        vec!["src/a.test.ts", "src/legacy.test.ts"],
        "无 excludes 臂：清单含 legacy（对照面同源口径）"
    );
    assert!(
        plans[0].omit_files_safe,
        "无 excludes → 两侧集合相等 → 可降级"
    );

    assert_eq!(
        plans[1].files,
        vec!["src/a.test.ts"],
        "apply_excludes = true 臂：legacy 被剔除"
    );
    assert!(
        !plans[1].omit_files_safe,
        "apply_excludes = false 臂仍含 legacy → 集合不等 → 拒降级"
    );
}

/// 异常（表驱动 3 形态）：非法 includes glob → 显式 `Err` 沿既有文案
/// 「…includes glob 非法…」（不 panic；发现面与对照面经同一 `collect_files`
/// 单点，故同文案同出口）。
#[tokio::test]
async fn 非法includes_glob显式err不panic() {
    let _window = PathWindow::enter();
    let ws = TempWs::new("omit-bad-glob");
    omit_tree(&ws);

    for (label, pattern) in [
        ("未闭合字符类", "**/*.test.ts["),
        ("非法字符类", "**/[!a.test.ts"),
        (
            "花括号展开超限",
            "{a,b,c,d,e,f,g,h}{a,b,c,d,e,f,g,h}{a,b,c,d,e,f,g,h}{a,b,c,d,e,f,g,h}",
        ),
    ] {
        let suites = vec![suite(
            "app",
            TestFramework::VitePlus,
            Some(vec![pattern.to_owned()]),
            None,
            None,
        )];
        let err = detect_plans(ws.root(), &suites)
            .expect_err(&format!("{label}: 非法 glob 应显式 Err（不 panic）"));
        assert!(
            err.contains("includes glob 非法"),
            "{label}: 沿既有文案（实际: {err}）"
        );
        assert!(
            err.contains("suite root \"app\""),
            "{label}: Err 记因 suite root（实际: {err}）"
        );
    }
}
