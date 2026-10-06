use std::fs;
use std::path::{Path, PathBuf};

use config::{MutationConfig, TestFramework};

use super::{detect_plans, extract_semver, is_version_at_least, to_posix, REGISTRY};
use checks::parser::CoverageFormat;

/// jest `--randomize` 最低版本语义（与 CLI `isVersionAtLeast(version, '29.5.0')`
/// 同值对照）。
const JEST_RANDOMIZE_MINIMUM: &str = "29.5.0";

// ---------------------------------------------------------------------------
// 装置：tempdir fixture 树
// ---------------------------------------------------------------------------

/// 临时 workspace 根 RAII。根目录名不以 `.` 起手（detect 的 dot 目录剪枝按
/// 任意路径分量执法，dot 前缀临时目录会让全部样本被剪）。
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

/// brace-free 测试文件 glob（`glob` crate 不展开花锳——见模块注记）。
const TS_TESTS_GLOB: &str = "**/*.test.ts";

/// PATH 隔离窗口（RAII）：持 crate 级 TEST_ENV_LOCK 期间改写进程 PATH、drop
/// 恢复（static_check / testexec 各测试文件的环境窗口经同锁互斥，防并行
/// 用例互踩进程 PATH）。Unix 侧另与 static_check_test 的 crate 级
/// TEST_PATH_LOCK 互斥（拉起本体失败臂共享进程 PATH）。
struct PathWindow {
    original: Option<std::ffi::OsString>,
    _inner: std::sync::MutexGuard<'static, ()>,
    #[cfg(not(target_os = "windows"))]
    _outer: std::sync::MutexGuard<'static, ()>,
}

impl PathWindow {
    fn enter() -> Self {
        #[cfg(not(target_os = "windows"))]
        let outer = crate::TEST_PATH_LOCK.lock().expect("crate PATH 锁不可中毒");
        let inner = crate::TEST_ENV_LOCK.lock().expect("crate 环境锁不可中毒");
        Self {
            original: std::env::var_os("PATH"),
            _inner: inner,
            #[cfg(not(target_os = "windows"))]
            _outer: outer,
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
fn 五框架注册表常量与cli逐字对齐() {
    // CLI FRAMEWORK_REGISTRY 键序（jest / vitest / vite-plus / rust / node-test
    // 为 V1 支持面；bun / go / pytest 在 detect 显式 Err 臂覆盖）
    let jest_template = "npx jest --reporters=default --json --outputFile=\"{results_file}\" --silent --coverage --coverageDirectory=\"{report_dir}\" --coverageReporters=json-summary {config_args} {files}";
    let vitest_template = "npx vitest run --sequence.shuffle --reporter=json --outputFile.json=\"{results_file}\" --coverage --coverage.reportsDirectory=\"{report_dir}\" --coverage.reporter=json-summary {config_args} {files}";
    let vite_plus_template = "vp test --sequence.shuffle --reporter=json --outputFile.json=\"{results_file}\" --coverage --coverage.reportsDirectory=\"{report_dir}\" --coverage.reporter=json-summary {config_args} {files}";
    let rust_shell = "cargo test --workspace; _X=$?; cargo llvm-cov --json --output-path \"{coverage_file}\"; exit $_X";
    let rust_cmd = "cargo test --workspace & if errorlevel 1 set _X=%errorlevel% & cargo llvm-cov --json --output-path \"{coverage_file}\" & exit /b %_X%";
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
            rust_shell,
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

    // jest cmd 模板与 shell 同串（CLI 双形态一致）；rust cmd 模板逐字（CLI
    // cmd 形态 & 链 / %errorlevel% 语法）
    let jest = &REGISTRY[0];
    assert_eq!(
        jest.cmd_template, jest.shell_template,
        "jest 双形态模板一致（CLI jest shell/cmd 同 builder 同串）"
    );
    assert_eq!(REGISTRY[3].cmd_template, rust_cmd);
}

// ---------------------------------------------------------------------------
// detect_plans：探测矩阵
// ---------------------------------------------------------------------------

/// 正向：tempdir 树 + vite-plus 形态 suite 配置（brace-free includes 命中
/// 样本文件）→ TestPlan 产出（id 沿 colocated 命名映射、cwd、
/// coverage_format=istanbul、excludes 过滤 + dot 目录剪枝后的文件清单）。
#[tokio::test]
async fn vite_plus_suite探测_清单排除与剪枝() {
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

/// 正向：rust suite → plan（coverage_format=llvm-cov、命令模板与 CLI 同源、
/// default_glob 圈定 tests 树——brace-free 缺省 glob 可直接命中）。
#[tokio::test]
async fn rust_suite探测_llvm_cov档() {
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
    assert!(
        plan.shell_template.starts_with("cargo test --workspace"),
        "命令模板与 CLI 同源（cargo test + llvm-cov 收口）"
    );
    assert!(
        plan.shell_template.contains("{coverage_file}"),
        "占位符原样保留由 runner 展开"
    );
    assert!(
        plan.config_args.is_none(),
        "rust 不支持配置注入（未声明即无）"
    );
}

/// 异常：bun / go / pytest suite 配置 → 显式 Err 记因（无静默跳过分支——
/// AC-4 反向半边）；不支持注入的框架声明 config 同族显式 Err。
#[tokio::test]
async fn 不支持框架显式err不静默() {
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

    // 配置注入声明校验同族：不支持注入的框架（rust）声明 config → 显式 Err
    let suites = vec![suite(
        "crates",
        TestFramework::Rust,
        None,
        None,
        Some("Cargo.toml".to_owned()),
    )];
    let err = detect_plans(ws.root(), &suites).expect_err("不支持注入声明 config 应 Err");
    assert!(
        err.contains("不支持配置注入"),
        "配置注入 Err 记因（CLI validateSuiteConfig 同语义），实际: {err}"
    );
}

/// 边界：includes 全不命中 → 该 suite 不产 plan（空清单面）且整体不 panic。
#[tokio::test]
async fn includes全不命中_不产plan不panic() {
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
