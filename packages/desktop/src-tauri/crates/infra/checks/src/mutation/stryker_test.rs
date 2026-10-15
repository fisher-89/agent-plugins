//! `stryker.rs` 的单元测试（test-design「stryker.rs -> stryker_test.rs」
//! 节）：临时 config 组装（`mutate` = 段清单、`tempDirName` = 共享 sandbox 全段
//! 同路径、`concurrency` = 半 CPU、`jsonReporter` 直写段报告、runner / 插件包
//! 映射）+ spawn 模板展开 argv 逐参等值（`npx --prefix <suite cwd> stryker run
//! <config>`）+ 失败面（非零退出 → 输出捕获 + 中断；超时 → 中断不挂死）+
//! `ensure_on_path` 前置（E5：不可达显式 Err）+ 清单边界。
//!
//! Mock策略（test-design 本节 Mock 表）：子进程 spawn（npx stryker——进程边界）
//! 经**假 spawn 装置**承载——`mod_test::SpawnWindow`（PATH 前置 `npx` shim +
//! 记录 argv / cwd + 可编程输出 / 超时自旋挂起 / 非零退出 / ENOENT）；config
//! 组装 / 模板面为纯函数直驱（StrykerSegment 显式入参）；文件系统经真实
//! tempdir（清理 / 保留断言真实组合）；`available_parallelism` 不 mock
//! （concurrency 按真实 CPU 重算关系式）。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::super::mod_test::SpawnWindow;
use super::super::mutation_jobs;
use super::super::report::to_posix;
use super::super::{SegmentFailure, SEGMENT_SPAWN_TIMEOUT_MS};
use super::{
    build_config, ensure_on_path, plugin_package, relative_to_cwd, run_segment, test_runner,
    StrykerSegment,
};

/// Stryker 段报告 canned（metrics 面：9 killed + 1 survived / total 10——
/// score 90；假 spawn 经 `FAKE_SPAWN_REPORT` 直写段报告模拟 jsonReporter）。
const STRYKER_SEG_CANNED: &str =
    r#"{ "metrics": { "killed": 9, "survived": 1, "totalMutants": 10 }, "durationMs": 3000.0 }"#;

/// Stryker 段入参 fixture（sandbox 共享面与段标识全显式——各段仅 index /
/// files / 报告落位不同）。
fn segment<'a>(
    suite_cwd: &'a Path,
    mutation_dir: &'a Path,
    index: usize,
    files: &'a [String],
    project_root: &'a Path,
    framework: &'a str,
) -> StrykerSegment<'a> {
    StrykerSegment {
        suite_cwd,
        mutation_dir,
        index,
        files,
        project_root,
        framework,
        suite_config: None,
    }
}

/// 正向：config JSON 形态——`mutate` = 段清单逐项（cwd 相对化）、
/// `tempDirName` = 共享 sandbox（全段共用同一路径——sandbox 建置只发生一
/// 次）、`concurrency` = 半 CPU 计算值、`jsonReporter.fileName` = 段报告路径、
/// `disableTypeChecks` / `ignoreStatic` / `reporters` 面在案（AC-5 / D6）。
#[test]
fn 临时config组装_共享sandbox与半cpu与json直写() {
    let project = Path::new("/ws/root");
    let suite_cwd = Path::new("/ws/root/app");
    let mutation_dir = Path::new("/ws/root/reports/test/app_vite-plus/mutation");
    let report_path = mutation_dir.join("mutation-seg-0.json");
    let files = vec![
        "app/src/a.test.ts".to_owned(),
        "app/src/b.test.ts".to_owned(),
    ];

    let config = build_config(
        &segment(suite_cwd, mutation_dir, 0, &files, project, "vite-plus"),
        &report_path,
    );

    // mutate = 段清单逐项（cwd 相对化）
    assert_eq!(
        config["mutate"],
        serde_json::json!(["src/a.test.ts", "src/b.test.ts"]),
        "mutate = 段清单（suite cwd 相对 POSIX）"
    );
    // tempDirName = 共享 sandbox（全段共用同一路径——不随 index 变化）
    assert_eq!(
        config["tempDirName"],
        serde_json::json!("/ws/root/reports/test/app_vite-plus/mutation/_stryker-tmp"),
        "tempDirName = 共享 sandbox（全段同路径——D6）"
    );
    // concurrency = 半 CPU
    assert_eq!(
        config["concurrency"],
        serde_json::json!(mutation_jobs()),
        "concurrency = max(1, CPU/2)"
    );
    // jsonReporter 直写段报告路径（jsonReporter 面——报告落位单点）
    assert_eq!(
        config["jsonReporter"]["fileName"],
        serde_json::json!("/ws/root/reports/test/app_vite-plus/mutation/mutation-seg-0.json"),
        "jsonReporter 直写段报告路径"
    );
    // 固定面：schema / reporters / type checks / ignoreStatic / timeout
    assert_eq!(
        config["$schema"],
        serde_json::json!("node_modules/@stryker-mutator/core/schema/stryker-schema.json")
    );
    assert_eq!(config["reporters"], serde_json::json!(["json"]));
    assert_eq!(config["disableTypeChecks"], serde_json::json!(true));
    assert_eq!(config["ignoreStatic"], serde_json::json!(true));
    assert_eq!(config["timeoutFactor"], serde_json::json!(1.5));

    // 多段共享面：不同 index 的 tempDirName 逐字相同（sandbox 跨段复用锚）
    let seg1_report = mutation_dir.join("mutation-seg-1.json");
    let config1 = build_config(
        &segment(suite_cwd, mutation_dir, 1, &files, project, "vite-plus"),
        &seg1_report,
    );
    assert_eq!(
        config1["tempDirName"], config["tempDirName"],
        "多段共用同一 tempDirPath（段间不重建不重置——AC-5）"
    );
    assert_eq!(
        config1["jsonReporter"]["fileName"],
        serde_json::json!("/ws/root/reports/test/app_vite-plus/mutation/mutation-seg-1.json"),
        "jsonReporter 随段落位"
    );
}

/// 正向：runner / 插件包映射（CLI `resolveTestRunner` / `resolvePluginPackage`
/// 同语义）——jest → jest runner + jest 插件包；vitest / vite-plus 共用 vitest
/// runner + vitest 插件包（Stryker v9 注册名与包名错位口径）。
#[test]
fn runner与插件包映射_三框架封闭() {
    assert_eq!(test_runner("jest"), "jest");
    assert_eq!(test_runner("vitest"), "vitest");
    assert_eq!(
        test_runner("vite-plus"),
        "vitest",
        "vite-plus 共用 vitest runner"
    );
    // 兜底面：未知框架退 vitest（js 族三值封闭——非 js 不进 Stryker 面）
    assert_eq!(test_runner("unknown"), "vitest");

    assert_eq!(plugin_package("jest"), "@stryker-mutator/jest-runner");
    assert_eq!(plugin_package("vitest"), "@stryker-mutator/vitest-runner");
    assert_eq!(
        plugin_package("vite-plus"),
        "@stryker-mutator/vitest-runner",
        "非 jest 一律 vitest 插件包"
    );

    // config 内 testRunner / plugins 绑定面
    let project = Path::new("/ws/root");
    let suite_cwd = Path::new("/ws/root");
    let mutation_dir = Path::new("/ws/reports/mutation");
    let files = vec!["src/a.test.ts".to_owned()];
    let jest_config = build_config(
        &segment(suite_cwd, mutation_dir, 0, &files, project, "jest"),
        &mutation_dir.join("mutation-seg-0.json"),
    );
    assert_eq!(jest_config["testRunner"], serde_json::json!("jest"));
    assert_eq!(
        jest_config["plugins"],
        serde_json::json!(["@stryker-mutator/jest-runner"]),
        "plugins = 对应 runner 插件包"
    );
}

/// 正向：清单项 → suite cwd 相对 POSIX（abs 归一后 strip cwd）；cwd 外文件退
/// 绝对路径 POSIX 形态（suite cwd 含空格 / 中文面不破坏）。
#[test]
fn 清单相对化_cwd内外与特殊目录名() {
    // cwd 内 → 相对 POSIX
    assert_eq!(
        relative_to_cwd(
            Path::new("/ws/root"),
            Path::new("/ws/root/app"),
            "app/src/x.ts"
        ),
        "src/x.ts",
        "cwd 内清单项相对化"
    );
    // cwd 外 → 绝对路径 POSIX 形态
    assert_eq!(
        relative_to_cwd(
            Path::new("/ws/root"),
            Path::new("/ws/root/app"),
            "other/lib.rs"
        ),
        "/ws/root/other/lib.rs",
        "cwd 外退绝对路径"
    );

    // suite cwd 含空格 / 中文（Windows 反斜杠入参归一 POSIX）
    let spaced_root = Path::new("C:\\ws\\工作 区");
    let spaced_cwd = Path::new("C:\\ws\\工作 区\\app 目录");
    let relative = relative_to_cwd(spaced_root, spaced_cwd, "app 目录\\src\\x.ts");
    assert_eq!(
        relative, "src/x.ts",
        "空格 / 中文 cwd 相对化 + 反斜杠归一: {relative}"
    );
}

/// 边界：`mutate` 空 / 单项 / 超大（1000 glob）清单均可组装（config 面不
/// 破坏）；suite_config 在位时 runner 配置块携 configFile 相对路径。
#[test]
fn 清单边界_空单超大与suite_config块() {
    let project = Path::new("/ws/root");
    let suite_cwd = Path::new("/ws/root");
    let mutation_dir = Path::new("/ws/reports/mutation");
    let report_path = mutation_dir.join("mutation-seg-0.json");

    // 空清单
    let empty = build_config(
        &segment(suite_cwd, mutation_dir, 0, &[], project, "vitest"),
        &report_path,
    );
    assert_eq!(empty["mutate"], serde_json::json!([]), "空清单面");

    // 单项
    let single = vec!["src/only.test.ts".to_owned()];
    let config = build_config(
        &segment(suite_cwd, mutation_dir, 0, &single, project, "vitest"),
        &report_path,
    );
    assert_eq!(config["mutate"], serde_json::json!(["src/only.test.ts"]));

    // 超大清单（1000 glob）
    let huge: Vec<String> = (0..1000)
        .map(|index| format!("src/f{index:03}.test.ts"))
        .collect();
    let config = build_config(
        &segment(suite_cwd, mutation_dir, 0, &huge, project, "vitest"),
        &report_path,
    );
    assert_eq!(
        config["mutate"].as_array().expect("mutate 为数组").len(),
        1000,
        "超大清单（1000 glob）config 面不破坏"
    );

    // suite_config 在位 → runner 配置块携 configFile（suite cwd 相对）
    let suite_config: PathBuf = Path::new("/ws/root/app/vitest.workspace.ts").to_path_buf();
    let with_config = StrykerSegment {
        suite_config: Some(suite_config.as_path()),
        ..segment(
            Path::new("/ws/root/app"),
            mutation_dir,
            0,
            &single,
            project,
            "vitest",
        )
    };
    let config = build_config(&with_config, &report_path);
    assert_eq!(
        config["vitest"],
        serde_json::json!({ "configFile": "vitest.workspace.ts" }),
        "suite_config 相对 suite cwd 出线"
    );
}

/// `ensure_on_path` 前置（E5）：不可达程序名 → 显式 `Err` 携安装引导（环境
/// 缺工具是显式可修问题）；可达程序（PATH 真实探测）→ `Ok`。
#[test]
fn ensure_on_path_不可达显式err可达ok() {
    let error = ensure_on_path("definitely-not-a-real-program-xyz").expect_err("不可达程序应 Err");
    assert!(
        error.contains("突变执行器不可达") && error.contains("definitely-not-a-real-program-xyz"),
        "Err 记因可修面（E5）: {error}"
    );
    assert!(error.contains("请安装后重试"), "携修复引导: {error}");

    // 可达面：git 为 dev 环境必有程序（真实 PATH 探测）
    ensure_on_path("git").expect("git 应 PATH 可达（dev 环境恒装）");
}

// ---------------------------------------------------------------------------
// spawn 面（假 spawn）用例：模板展开 argv / 非零退出中断 / 超时中断
// ---------------------------------------------------------------------------

/// spawn 面 fixture：tempdir workspace（suite cwd 与突变报告目录在位——
/// `current_dir` spawn 前置与 config / 段报告写盘面）。
struct SpawnWs {
    _tmp: TempDir,
    root: PathBuf,
    suite_cwd: PathBuf,
    mutation_dir: PathBuf,
}

impl SpawnWs {
    fn new(tag: &str) -> Self {
        let tmp = tempfile::Builder::new()
            .prefix(&format!("checks-mutation-stryker-{tag}-"))
            .tempdir()
            .expect("创建临时 workspace 失败");
        let root = tmp.path().to_path_buf();
        let suite_cwd = root.join("app");
        let mutation_dir = root
            .join("reports")
            .join("test")
            .join("app_vitest")
            .join("mutation");
        fs::create_dir_all(&suite_cwd).expect("建 suite 执行目录");
        fs::create_dir_all(&mutation_dir).expect("建突变报告目录");
        Self {
            _tmp: tmp,
            root,
            suite_cwd,
            mutation_dir,
        }
    }

    /// 段入参（恒 index 0 / vitest framework——各 spawn 用例共用形态）。
    fn segment<'a>(&'a self, files: &'a [String]) -> StrykerSegment<'a> {
        StrykerSegment {
            suite_cwd: &self.suite_cwd,
            mutation_dir: &self.mutation_dir,
            index: 0,
            files,
            project_root: &self.root,
            framework: "vitest",
            suite_config: None,
        }
    }
}

/// 正向：**spawn 模板展开**——argv 逐参等值（`npx --prefix <suite cwd> stryker
/// run <config>`，config = `mutation-stryker-seg-0.conf.json`）+ spawn cwd 捕获
/// + 段报告经假 spawn `jsonReporter` 面直写落位（假 spawn 记录面——Mock 表装
/// 置，执行链真实组合）。
#[tokio::test]
async fn spawn模板展开_argv逐参等值与cwd捕获() {
    let ws = SpawnWs::new("argv");
    let files = vec![
        "app/src/a.test.ts".to_owned(),
        "app/src/b.test.ts".to_owned(),
    ];
    let window = SpawnWindow::enter("stryker-argv");
    window.install_npx();
    window.set_out(STRYKER_SEG_CANNED);
    let report_path = ws.mutation_dir.join("mutation-seg-0.json");
    window.set_report(&report_path);

    let raw = run_segment(&ws.segment(&files))
        .await
        .expect("假 spawn 段执行应 Ok");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("段报告可解析");
    assert_eq!(parsed["metrics"]["killed"], 9, "段报告 = canned 原样");

    let records = window.records();
    assert_eq!(records.len(), 1, "恰一次假 spawn");
    let config_path = ws.mutation_dir.join("mutation-stryker-seg-0.conf.json");
    assert_eq!(
        records[0].argv,
        vec![
            "npx".to_owned(),
            "--prefix".to_owned(),
            to_posix(&ws.suite_cwd),
            "stryker".to_owned(),
            "run".to_owned(),
            to_posix(&config_path),
        ],
        "spawn 模板展开 argv 逐参等值"
    );
    let cwd = records[0]
        .cwd
        .clone()
        .expect("spawn cwd 捕获")
        .replace('\\', "/");
    assert_eq!(cwd, to_posix(&ws.suite_cwd), "spawn cwd = suite 执行目录");

    // config 写盘面（spawn 前置产物）：mutate = 段清单逐项（cwd 相对化）
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config_path).expect("config 在盘"))
            .expect("config 可解析");
    assert_eq!(
        config["mutate"],
        serde_json::json!(["src/a.test.ts", "src/b.test.ts"])
    );
}

/// 异常：**非零退出 → 输出捕获 + 中断**——假 spawn 可编程非零退出（退出码 3）
/// 且零段报告 → `SegmentFailure::Interrupted` 携退出码与合并输出有界尾部（双时
/// 点②中断面原料）。
#[tokio::test]
async fn 非零退出中断_携退出码与输出尾部捕获() {
    let ws = SpawnWs::new("exit3");
    let files = vec!["app/src/a.test.ts".to_owned()];
    let window = SpawnWindow::enter("stryker-exit");
    window.install_npx();
    window.set_out("stryker harness boom-marker"); // 非 JSON 损坏输出（零段报告面）
    window.set_exit(3);

    let result = run_segment(&ws.segment(&files)).await;
    match result {
        Err(SegmentFailure::Interrupted(message)) => {
            assert!(message.contains("退出码 3"), "携退出码: {message}");
            assert!(
                message.contains("未产出可解析段报告"),
                "记因无段报告: {message}"
            );
            assert!(
                message.contains("stryker harness boom-marker"),
                "输出捕获（合并输出有界尾部）: {message}"
            );
        }
        Err(SegmentFailure::Fatal(message)) => {
            panic!("非零退出应 Interrupted 而非 Fatal: {message}")
        }
        Ok(raw) => panic!("零段报告应中断，实际 Ok: {raw}"),
    }

    // 假 spawn 记录面（模板展开同装置承接）
    let records = window.records();
    assert_eq!(records.len(), 1, "恰一次假 spawn");
    assert_eq!(records[0].argv[0], "npx");
    assert_eq!(records[0].argv.get(1).map(String::as_str), Some("--prefix"));
}

/// 异常：**超时 → 中断不挂死**——假 spawn 可编程自旋挂起（管道恒开、读挂
/// 起）+ 虚拟时钟推进越过段 spawn 硬超时（`start_paused` + 手动 `advance`——
/// 子进程管道读挂起时暂停时钟不自动推进，手动推进触发超时臂）→
/// `start_kill` 强制终止 + `Interrupted` 超时文案，墙钟上界断言不挂死。
#[tokio::test(start_paused = true)]
async fn 超时中断_强制终止不挂死() {
    let ws = SpawnWs::new("timeout");
    let files = vec!["app/src/a.test.ts".to_owned()];
    let window = SpawnWindow::enter("stryker-timeout");
    window.install_npx();
    window.set_sleep(1); // 记录后自旋挂起（stdout/stderr 恒开）

    // 虚拟时钟推进：越过首个让出点（run_segment 已注册段 spawn 硬超时定时器）
    // 后手动 advance。
    tokio::spawn(async {
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS + 60_000)).await;
    });

    let started = Instant::now();
    let result = run_segment(&ws.segment(&files)).await;
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(30),
        "超时中断不挂死（墙钟上界 {elapsed:?}）"
    );
    match result {
        Err(SegmentFailure::Interrupted(message)) => {
            assert!(
                message.contains("Stryker 段执行超时（1800 秒）强制终止"),
                "超时中断文案（双时点②即时反馈面）: {message}"
            );
        }
        Err(SegmentFailure::Fatal(message)) => panic!("应超时中断而非 Fatal: {message}"),
        Ok(raw) => panic!("自旋挂起应超时中断，实际 Ok: {raw}"),
    }
}
