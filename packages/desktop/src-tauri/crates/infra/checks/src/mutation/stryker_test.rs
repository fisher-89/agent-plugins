use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::super::mod_test::{wait_pid, wait_pid_gone, SpawnWindow};
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
    // 固定面：reporters / type checks / ignoreStatic / timeout
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

    let raw = run_segment(&ws.segment(&files), foundation::cancel::CancelFlag::never())
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

    let result = run_segment(&ws.segment(&files), foundation::cancel::CancelFlag::never()).await;
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
        Err(SegmentFailure::Cancelled(message)) => {
            panic!("非零退出应 Interrupted 而非 Cancelled: {message}")
        }
        Ok(raw) => panic!("零段报告应中断，实际 Ok: {raw}"),
    }

    // 假 spawn 记录面（模板展开同装置承接）
    let records = window.records();
    assert_eq!(records.len(), 1, "恰一次假 spawn");
    assert_eq!(records[0].argv[0], "npx");
    assert_eq!(records[0].argv.get(1).map(String::as_str), Some("--prefix"));
}

/// 异常：**超时 → 中断不挂死 + pid 回收**——假 spawn 可编程自旋挂起（管道恒
/// 开、读挂起）+ 虚拟时钟推进越过段 spawn 硬超时（`start_paused` + 手动
/// `advance`——子进程管道读挂起时暂停时钟不自动推进，手动推进触发超时臂）→
/// `kill_tree` 强制终止 + `Interrupted` 超时文案 + 目标 pid 消失，墙钟上界断言
/// 不挂死。
#[tokio::test(start_paused = true)]
async fn 超时中断_强制终止且pid消失不挂死() {
    let ws = SpawnWs::new("timeout");
    let window = SpawnWindow::enter("stryker-timeout");
    window.install_npx();
    window.set_sleep(1); // 记录后自旋挂起（stdout/stderr 恒开）
    let pid_file = ws.mutation_dir.join("stryker-timeout-pid.txt");
    window.set_pidfile(&pid_file);

    // 段执行在任务内（主任务负责「等 pid 落盘 → 推进虚拟时钟」的握手）
    let suite_cwd = ws.suite_cwd.clone();
    let mutation_dir = ws.mutation_dir.clone();
    let project_root = ws.root.clone();
    let runner = tokio::spawn(async move {
        let task_files = vec!["app/src/a.test.ts".to_owned()];
        let segment = StrykerSegment {
            suite_cwd: &suite_cwd,
            mutation_dir: &mutation_dir,
            index: 0,
            files: &task_files,
            project_root: &project_root,
            framework: "vitest",
            suite_config: None,
        };
        run_segment(&segment, foundation::cancel::CancelFlag::never()).await
    });
    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;

    let started = Instant::now();
    tokio::time::advance(Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS + 60_000)).await;
    let result = runner.await.expect("段任务应正常结束");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(30),
        "超时中断不挂死（墙钟上界 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "超时强杀后段进程 pid {pid} 应消失（kill_tree 生效）"
    );
    match result {
        Err(SegmentFailure::Interrupted(message)) => {
            assert!(
                message.contains("Stryker 段执行超时（3600 秒）强制终止"),
                "超时中断文案（双时点②即时反馈面）: {message}"
            );
        }
        Err(SegmentFailure::Fatal(message)) => panic!("应超时中断而非 Fatal: {message}"),
        Err(SegmentFailure::Cancelled(message)) => {
            panic!("时钟推进应超时中断而非 Cancelled: {message}")
        }
        Ok(raw) => panic!("自旋挂起应超时中断，实际 Ok: {raw}"),
    }
}

// ---------------------------------------------------------------------------
// 取消臂（test-design「mutation/stryker.rs -> stryker_test.rs」节，AC-4 / D13）
// ---------------------------------------------------------------------------

/// 正向（AC-4）：运行中 cancel——自旋假 `npx` 运行中置位 → `Cancelled` 文案
///「Stryker 段执行已取消」+ 临时 config 已写盘（取消不清理临时面，交由调用方
/// 保留纪律）+ 墙钟上界。
#[tokio::test]
async fn 运行中cancel_段取消变体且临时config保留() {
    let ws = SpawnWs::new("cancel");
    let window = SpawnWindow::enter("stryker-cancel");
    window.install_npx();
    window.set_sleep(1);
    let pid_file = ws.mutation_dir.join("stryker-cancel-pid.txt");
    window.set_pidfile(&pid_file);
    let files = vec!["app/src/add.test.mjs".to_owned()];

    // 段入参以自有路径在任务内重建（TempDir 归测试侧持有——临时面保留断言在
    // 任务结束后仍可观测）
    let mutation_dir = ws.mutation_dir.clone();
    // 任务内用的副本（保留断言仍读测试侧句柄）
    let task_mutation_dir = mutation_dir.clone();
    let suite_cwd = ws.suite_cwd.clone();
    let project_root = ws.root.clone();
    let cancel = foundation::cancel::CancelFlag::never();
    let handle = cancel.handle();
    let runner = tokio::spawn(async move {
        let segment = StrykerSegment {
            suite_cwd: &suite_cwd,
            mutation_dir: &task_mutation_dir,
            index: 0,
            files: &files,
            project_root: &project_root,
            framework: "vitest",
            suite_config: None,
        };
        run_segment(&segment, cancel).await
    });

    let pid = wait_pid(&pid_file, std::time::Duration::from_secs(60)).await;
    let started = std::time::Instant::now();
    handle.set();

    let result = runner.await.expect("段任务应正常结束");
    let elapsed = started.elapsed();
    match result {
        Err(SegmentFailure::Cancelled(message)) => {
            assert_eq!(message, "Stryker 段执行已取消", "取消文案逐字");
        }
        Err(SegmentFailure::Interrupted(message)) => {
            panic!("置位取消应为 Cancelled，实际 Interrupted: {message}")
        }
        Err(SegmentFailure::Fatal(message)) => {
            panic!("置位取消应为 Cancelled，实际 Fatal: {message}")
        }
        Ok(raw) => panic!("取消应中断，实际 Ok: {raw}"),
    }
    assert!(
        mutation_dir.exists(),
        "取消不清理临时面（config / sandbox 保留，交由调用方纪律）"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "取消秒级收敛（墙钟 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, std::time::Duration::from_secs(15)),
        "取消强杀后段进程 pid {pid} 应消失（kill_tree 生效）"
    );
}

/// 正向（AC-4 / D4）：已置位 cancel 的入口短路——临时 config 已写盘但进程未
/// 拉起（假 `npx` 零记录），直接 `Cancelled("Stryker 段执行已取消")`。
#[tokio::test]
async fn 已置位cancel_入口短路零spawn且临时config在位() {
    let ws = SpawnWs::new("cancel-pre");
    let window = SpawnWindow::enter("stryker-cancel-pre");
    window.install_npx();
    let files = vec!["app/src/add.test.mjs".to_owned()];

    let cancel = foundation::cancel::CancelFlag::never();
    cancel.handle().set();
    let result = run_segment(&ws.segment(&files), cancel).await;

    match result {
        Err(SegmentFailure::Cancelled(message)) => {
            assert_eq!(message, "Stryker 段执行已取消", "取消文案逐字");
        }
        Err(SegmentFailure::Interrupted(message)) => {
            panic!("已置位 cancel 应为 Cancelled，实际 Interrupted: {message}")
        }
        Err(SegmentFailure::Fatal(message)) => {
            panic!("已置位 cancel 应为 Cancelled，实际 Fatal: {message}")
        }
        Ok(raw) => panic!("已置位 cancel 应中断，实际 Ok: {raw}"),
    }
    assert!(
        window.records().is_empty(),
        "入口短路零 spawn（取消不拉起 npx）"
    );
    assert!(
        ws.mutation_dir
            .join("mutation-stryker-seg-0.conf.json")
            .is_file(),
        "临时 config 已写盘（写盘先于 spawn，取消不清理临时面）"
    );
}

// ---------------------------------------------------------------------------
// 合并流与大输出（test-design「mutation/stryker.rs -> stryker_test.rs」节「合并流与
// 大输出 | 边界 | 双管道各 >64 KiB → `output` 全长「stdout ++ stderr」逐字节 |
// 表驱动（2 形态：空 / 大块）」）：`Completed` 臂的合并输出面（`stdout` 前缀 +
// `stderr` 后缀）经段中断错误文案（`exec::ProcessOutput` 的输出末尾段——行数
// ≤ 50 时不截断）逐字节观测。
// ---------------------------------------------------------------------------

/// 边界（表驱动 2 形态）：段合并输出的「空」与「大块」两形态——空 → 裸 head
/// （无尾部段）；双管道各 >64 KiB → `output` 全长「stdout ++ stderr」逐字节
/// （零丢字节、零重排、零截断），且并发排水不互锁（墙钟上界）。
#[tokio::test]
async fn 合并流与大输出_双管道大块逐字节() {
    let files = vec!["app/src/a.test.ts".to_owned()];

    // 形态一：双空输出（非零退出且无段报告 → 合并输出为空 → 裸 head）
    {
        let ws = SpawnWs::new("merge-empty");
        let window = SpawnWindow::enter("stryker-merge-empty");
        window.install_npx();
        window.set_exit(1);

        let result = tokio::time::timeout(
            Duration::from_secs(60),
            run_segment(&ws.segment(&files), foundation::cancel::CancelFlag::never()),
        )
        .await
        .expect("空输出形态不应挂死（墙钟上界 60s）");
        match result {
            Err(SegmentFailure::Interrupted(message)) => {
                assert_eq!(
                    message, "Stryker 段执行失败（退出码 1，未产出可解析段报告）",
                    "空输出 → 裸 head（无「错误信息」/「输出末尾」段）"
                );
            }
            Err(SegmentFailure::Fatal(message)) => {
                panic!("非零退出应 Interrupted 而非 Fatal: {message}")
            }
            Err(SegmentFailure::Cancelled(message)) => {
                panic!("非零退出应 Interrupted 而非 Cancelled: {message}")
            }
            Ok(raw) => panic!("零段报告应中断，实际 Ok: {raw}"),
        }
        assert_eq!(window.records().len(), 1, "空形态恰一次假 spawn");
    }

    // 形态二：双管道各 >64 KiB（单路一行超管道缓冲——顺序读会互锁/丢字节）
    {
        // 管道缓冲上限 64 KiB：单路 70_000 字节显著越过
        const BIG: usize = 70_000;
        let stdout_line = "S".repeat(BIG);
        let stderr_line = "E".repeat(BIG);

        let ws = SpawnWs::new("merge-big");
        let window = SpawnWindow::enter("stryker-merge-big");
        window.install_npx();
        window.set_out(&format!("{stdout_line}\n"));
        window.set_err(&format!("{stderr_line}\n"));
        window.set_exit(101);

        let started = Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(60),
            run_segment(&ws.segment(&files), foundation::cancel::CancelFlag::never()),
        )
        .await
        .expect("双管道大块输出不应互锁（并发排水，墙钟上界 60s）");
        let elapsed = started.elapsed();

        let message = match result {
            Err(SegmentFailure::Interrupted(message)) => message,
            Err(SegmentFailure::Fatal(message)) => {
                panic!("非零退出应 Interrupted 而非 Fatal: {message}")
            }
            Err(SegmentFailure::Cancelled(message)) => {
                panic!("非零退出应 Interrupted 而非 Cancelled: {message}")
            }
            Ok(raw) => panic!("零段报告应中断，实际 Ok: {raw}"),
        };
        // 行数 = 2 ≤ 50 → 输出末尾段逐字全量（逐字节等值即「不丢字节」）
        let expected = format!(
            "Stryker 段执行失败（退出码 101，未产出可解析段报告）\n输出末尾（50 行）：\n{stdout_line}\n{stderr_line}"
        );
        assert_eq!(
            message, expected,
            "output = 「stdout 前缀 + stderr 后缀」全长逐字节（两路各 {BIG} 字节）"
        );
        assert_eq!(
            message.len(),
            expected.len(),
            "输出全量在案（零截断：stdout {BIG} + stderr {BIG} + head 段）"
        );
        assert!(
            elapsed < Duration::from_secs(40),
            "双管道并发排水不互锁（墙钟 {elapsed:?}）"
        );
        assert_eq!(window.records().len(), 1, "大块形态恰一次假 spawn");
    }
}

/// 正向（AC-2）：零重试——`npx` 输出含瞬态签名（`resource busy or locked`）+
/// `exit 1` → 段尝试次数恰 1（假 spawn 记录仅一条）、分流仍为 `Interrupted`
///（段层 `RetryPolicy::none()`：签名命中不追加尝试）。
#[tokio::test]
async fn 输出含瞬态签名不触发段重试() {
    let ws = SpawnWs::new("no-retry");
    let window = SpawnWindow::enter("stryker-no-retry");
    window.install_npx();
    window.set_out("Error: resource busy or locked");
    window.set_exit(1);

    let files = vec!["app/src/a.ts".to_owned()];
    let result = run_segment(&ws.segment(&files), foundation::cancel::CancelFlag::never()).await;

    assert!(
        matches!(result, Err(SegmentFailure::Interrupted(_))),
        "签名命中不改变分流（仍 Interrupted）: {result:?}"
    );
    assert_eq!(
        window.records().len(),
        1,
        "段层零重试：签名命中也不追加尝试（spawn 恰 1 次）"
    );
}
