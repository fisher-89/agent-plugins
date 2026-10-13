//! `cargo_mutants.rs` 的单元测试（test-design「cargo_mutants.rs ->
//! cargo_mutants_test.rs」节）：argv 模板展开逐参等值（`mutants --in-diff <段
//! 补丁> --json --baseline skip --jobs <半 CPU> --timeout 300`）+ 常量锚定 +
//! `CARGO_TARGET_DIR` 多段共享 env 断言 + 补丁入参边界 + JSON 载荷提取（整文本
//! 本可解析优先 / 首 `{`[` 起末 `}`]` 止最宽候选）+ 段中断错误面 + 失败面
//! （ENOENT / 非零退出 / 超时→中断不挂死）。
//!
//! Mock策略（test-design 本节 Mock 表）：子进程 spawn（cargo mutants——进程边
//! 界）经**假 spawn 装置**承载——`mod_test::SpawnWindow`（PATH 前置假 `cargo`
//! shim + 记录 argv / env / cwd + 可编程输出、超时（自旋挂起 + 虚拟时钟推进）、
//! 非零退出；ENOENT 臂 = PATH 隔离使程序缺席——`TEST_ENV_LOCK` 串行化，
//! testexec EnvWindow 先例同款）；`available_parallelism` 不 mock（jobs 期望值
//! 按真实 CPU 重算关系式 `max(1, n/2)`）。

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::super::mod_test::{SpawnRecord, SpawnWindow};
use super::super::report::to_posix;
use super::super::{
    mutation_jobs, output_tail, SegmentFailure, MUTATION_JOBS_DIVISOR, PER_MUTANT_TIMEOUT_SECS,
    SEGMENT_SPAWN_TIMEOUT_MS,
};
use super::{json_payload, run_segment, segment_failure_message};

/// cargo-mutants 段报告 canned（`--json` stdout 面——1 killed + 1 missed）。
const CARGO_SEG_CANNED: &str =
    r#"{ "mutants": [ { "outcome": "Caught" }, { "outcome": "missed" } ], "durationMs": 500.0 }"#;

/// argv 模板面常量锚定（AC-4：`--baseline skip` / per-mutant 超时 / 段 spawn
/// 硬超时 / jobs 半 CPU 除数——D9 不进 config schema）。
#[test]
fn argv模板常量面锚定() {
    assert_eq!(PER_MUTANT_TIMEOUT_SECS, 300, "per-mutant 超时 300 秒");
    assert_eq!(MUTATION_JOBS_DIVISOR, 2, "并发除数 2（半 CPU）");
    assert_eq!(SEGMENT_SPAWN_TIMEOUT_MS, 1_800_000, "段 spawn 硬超时 30 分钟");
}

/// jobs 计算：`max(1, available_parallelism() / 2)` 关系式（CPU 1 → 1、2 → 1、
/// 3 → 1、4 → 2、16 → 8 抽样口径；期望值按真实 CPU 重算——不 mock 运行环境）。
#[test]
fn jobs计算_半cpu关系式抽样口径() {
    let samples = [1usize, 2, 3, 4, 8, 16, 32, 64];
    for cpu in samples {
        let expected = (cpu / MUTATION_JOBS_DIVISOR).max(1);
        assert_eq!(
            expected,
            (cpu / 2).max(1),
            "CPU={cpu} → jobs={expected}（半 CPU 下限 1）"
        );
    }
    // 真实环境断言：jobs 与 available_parallelism 同式
    let parallelism = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    assert_eq!(
        mutation_jobs(),
        (parallelism / MUTATION_JOBS_DIVISOR).max(1),
        "mutation_jobs 与真实 CPU 半数关系式一致（下限 1）"
    );
    assert!(mutation_jobs() >= 1, "jobs 下限 1（单核 CPU）");
}

/// JSON 载荷提取：整文本可解析优先（stdout 零杂质形态）；混杂人类可读进度
/// 输出时取首 `{`/`[` 起、末 `}`/`]` 止最宽候选；不可解析 → `None`（段中断
/// 错误面输入）。
#[test]
fn json载荷提取_整文本优先与最宽候选() {
    // 整文本可解析优先
    let pure = r#"{"mutants":[{"outcome":"Caught"}]}"#;
    assert_eq!(json_payload(pure), Some(pure), "整文本可解析优先");

    // 混杂人类可读进度输出（零括号杂质）：首个 { 起、末个 } 止最宽候选
    let mixed = "Compiling crate v0.1.0\nRunning 12/12 mutants\n{\n  \"outcome\": \"Caught\"\n}\nFinished in 3.2s";
    assert_eq!(
        json_payload(mixed),
        Some("{\n  \"outcome\": \"Caught\"\n}"),
        "最宽候选（首个左花括号到末个右花括号）"
    );

    // 数组形态候选
    let array_mixed = "progress...\n[{\"outcome\":\"missed\"}]\ndone";
    assert_eq!(json_payload(array_mixed), Some("[{\"outcome\":\"missed\"}]"));

    // 方括号进度行干扰（`[====]` 先于 JSON 出线）：最宽候选自 `[==========]`
    // 起不可解析 → 保守 None（不吞半截——段中断错误面承接收）
    let bracket_noise = "[==========] 12/12 mutants\n{\"outcome\":\"Caught\"}\ndone";
    assert_eq!(json_payload(bracket_noise), None, "括号杂质下保守 None（不吞）");

    // 不可解析 → None
    assert_eq!(json_payload("no json at all"), None, "零 JSON 候选 → None");
    assert_eq!(json_payload("{ broken ]"), None, "首末错配 → None");
    assert_eq!(json_payload(""), None, "空输出 → None");
}

/// 段中断错误面：非零退出且无可解析报告 → 错误文案携退出码与输出有界尾部；
/// 空输出 → 无尾部缀形。
#[test]
fn 段中断错误面_携退出码与有界尾部() {
    let message = segment_failure_message(101, b"error: build failed\nsee details");
    assert!(message.contains("退出码 101"), "携退出码: {message}");
    assert!(
        message.contains("未产出可解析 JSON 段报告"),
        "记因无报告: {message}"
    );
    assert!(message.contains("error: build failed"), "携输出尾部: {message}");

    // 空输出 → 无尾部缀形（文案仍完整）
    let empty = segment_failure_message(1, b"");
    assert!(empty.contains("退出码 1"), "{empty}");
    assert!(!empty.contains('：'), "空输出无尾部冒号缀形: {empty}");
}

/// spawn 失败面（E5）：PATH 隔离窗口下 `cargo` 不可达 → spawn ENOENT →
/// 显式 `SegmentFailure::Fatal`（程序不可达可修面，停给用户不静默）。
#[tokio::test]
async fn spawn失败面_路径隔离下显式fatal() {
    let _guard = crate::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let result = run_segment(
        std::path::Path::new("mutation-seg-0.patch"),
        std::path::Path::new("."),
    )
    .await;

    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    match result {
        Err(SegmentFailure::Fatal(error)) => {
            assert!(
                error.contains("cargo mutants 拉起失败") && error.contains("程序不可达"),
                "Fatal 记因可修面（E5 停给用户）: {error}"
            );
            assert!(
                error.contains("cargo install cargo-mutants"),
                "Err 携安装引导: {error}"
            );
        }
        Err(SegmentFailure::Interrupted(error)) => {
            panic!("PATH 隔离下应 Fatal（拉起失败），实际 Interrupted: {error}")
        }
        Ok(raw) => panic!("PATH 隔离下应 Err，实际 Ok: {raw}"),
    }
}

// ---------------------------------------------------------------------------
// spawn 面（假 spawn）用例：argv 模板展开 / CARGO_TARGET_DIR 多段共享 /
// 非零退出中断 / 超时中断 / 补丁入参边界
// ---------------------------------------------------------------------------

/// spawn 面 fixture：tempdir workspace（suite 执行目录 + 突变报告目录——
/// `current_dir` spawn 前置与 `--in-diff` 段补丁落位面）。
struct SpawnWs {
    _tmp: TempDir,
    exec_dir: PathBuf,
    mutation_dir: PathBuf,
}

impl SpawnWs {
    fn new(tag: &str) -> Self {
        let tmp = tempfile::Builder::new()
            .prefix(&format!("checks-mutation-cargo-{tag}-"))
            .tempdir()
            .expect("创建临时 workspace 失败");
        let exec_dir = tmp.path().join("app");
        let mutation_dir = tmp
            .path()
            .join("reports")
            .join("test")
            .join("app_rust")
            .join("mutation");
        fs::create_dir_all(&exec_dir).expect("建 suite 执行目录");
        fs::create_dir_all(&mutation_dir).expect("建突变报告目录");
        Self {
            _tmp: tmp,
            exec_dir,
            mutation_dir,
        }
    }

    /// 段补丁写盘（`mutation-seg-<index>.patch`——`--in-diff` 传参面）。
    fn write_patch(&self, index: usize, content: &str) -> PathBuf {
        let path = self.mutation_dir.join(format!("mutation-seg-{index}.patch"));
        fs::write(&path, content).expect("写段补丁");
        path
    }
}

/// 假 spawn 记录面 env 键取值（`CARGO_TARGET_DIR` 断言原料）。
fn env_of(record: &SpawnRecord, key: &str) -> Option<String> {
    record
        .env
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
}

/// 正向：**spawn 模板展开**——argv 逐参等值（`mutants --in-diff <段补丁> --json
/// --baseline skip --jobs <半 CPU> --timeout 300`，AC-4）+ spawn cwd 捕获 +
/// `CARGO_TARGET_DIR` env 面捕获（假 spawn 记录面——Mock 表装置，执行链真实
/// 组合）。
#[tokio::test]
async fn spawn模板展开_argv逐参等值与cwd和env捕获() {
    let ws = SpawnWs::new("argv");
    let window = SpawnWindow::enter("cargo-argv");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_out(CARGO_SEG_CANNED);
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    let raw = run_segment(&patch_path, &ws.exec_dir)
        .await
        .expect("假 spawn 段执行应 Ok");
    let parsed: serde_json::Value =
        serde_json::from_str(raw.trim_end()).expect("段报告 = canned 可解析");
    assert_eq!(
        parsed["mutants"].as_array().expect("mutants 数组").len(),
        2,
        "段报告 = canned 原样（1 killed + 1 missed）"
    );

    let records = window.records();
    assert_eq!(records.len(), 1, "恰一次假 spawn");
    let argv = &records[0].argv;
    assert_eq!(argv.len(), 10, "argv 十参封闭: {argv:?}");
    assert_eq!(argv[0], "mutants", "argv 首 token（cargo 子命令面）");
    assert_eq!(argv[1], "--in-diff", "段补丁入参位");
    assert!(
        argv[2].replace('\\', "/").ends_with("mutation-seg-0.patch"),
        "--in-diff 携段补丁路径: {argv:?}"
    );
    assert_eq!(argv[3], "--json", "JSON 报告面");
    assert_eq!(argv[4], "--baseline", "baseline 位");
    assert_eq!(argv[5], "skip", "baseline skip（AC-4）");
    assert_eq!(argv[6], "--jobs", "jobs 位");
    assert_eq!(argv[7], mutation_jobs().to_string(), "jobs = max(1, CPU/2)");
    assert_eq!(argv[8], "--timeout", "timeout 位");
    assert_eq!(argv[9], "300", "per-mutant 超时 300（argv 逐字）");

    let cwd = records[0]
        .cwd
        .clone()
        .expect("spawn cwd 捕获")
        .replace('\\', "/");
    assert_eq!(cwd, to_posix(&ws.exec_dir), "spawn cwd = suite 执行目录");

    let target_dir = env_of(&records[0], "CARGO_TARGET_DIR").expect("env 捕获");
    assert_eq!(
        target_dir.replace('\\', "/"),
        to_posix(&ws.exec_dir.join("target")),
        "CARGO_TARGET_DIR = exec_dir/target"
    );
}

/// 正向：**`CARGO_TARGET_DIR` 多段共享（D6 / AC-5）**——同一 suite 执行目录下
/// 两段（seg-0 / seg-1）各一次假 spawn，env 断言同一 `CARGO_TARGET_DIR`（增量
/// 编译跨段复用，MUST NOT 每段重置）。
#[tokio::test]
async fn cargo_target_dir多段共享_env断言同一不重置() {
    let ws = SpawnWs::new("shared-target");
    let window = SpawnWindow::enter("cargo-target");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_out(CARGO_SEG_CANNED);

    let patch0 = ws.write_patch(0, "diff --git a/app/src/a.rs b/app/src/a.rs\n");
    let patch1 = ws.write_patch(1, "diff --git a/app/src/b.rs b/app/src/b.rs\n");
    run_segment(&patch0, &ws.exec_dir)
        .await
        .expect("段 0 应 Ok");
    run_segment(&patch1, &ws.exec_dir)
        .await
        .expect("段 1 应 Ok");

    let records = window.records();
    assert_eq!(records.len(), 2, "两段各恰一次假 spawn");
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.argv[0], "mutants", "段 {index} argv 首 token");
        assert_eq!(record.argv[1], "--in-diff");
        assert!(
            record.argv[2]
                .replace('\\', "/")
                .ends_with(&format!("mutation-seg-{index}.patch")),
            "段 {index} --in-diff 携各自段补丁: {:?}",
            record.argv
        );
    }

    let dir0 = env_of(&records[0], "CARGO_TARGET_DIR").expect("段 0 env 捕获");
    let dir1 = env_of(&records[1], "CARGO_TARGET_DIR").expect("段 1 env 捕获");
    assert_eq!(dir0, dir1, "多段共享同一 CARGO_TARGET_DIR（MUST NOT 每段重置）");
    assert_eq!(
        dir0.replace('\\', "/"),
        to_posix(&ws.exec_dir.join("target")),
        "CARGO_TARGET_DIR = exec_dir/target"
    );
}

/// 异常：**非零退出 → 输出捕获 + 中断**——假 spawn 可编程非零退出（退出码
/// 101）且 stdout 无 JSON 载荷 → `SegmentFailure::Interrupted` 携退出码与合并输
/// 出有界尾部（双时点②中断面原料）。
#[tokio::test]
async fn 非零退出中断_携退出码与输出尾部捕获() {
    let ws = SpawnWs::new("exit101");
    let window = SpawnWindow::enter("cargo-exit");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_out("cargo-mutants harness boom-marker"); // 非 JSON 损坏输出
    window.set_exit(101);
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    let result = run_segment(&patch_path, &ws.exec_dir).await;
    match result {
        Err(SegmentFailure::Interrupted(message)) => {
            assert!(message.contains("退出码 101"), "携退出码: {message}");
            assert!(
                message.contains("未产出可解析 JSON 段报告"),
                "记因无段报告: {message}"
            );
            assert!(
                message.contains("cargo-mutants harness boom-marker"),
                "输出捕获（合并输出有界尾部）: {message}"
            );
        }
        Err(SegmentFailure::Fatal(message)) => panic!("非零退出应 Interrupted 而非 Fatal: {message}"),
        Ok(raw) => panic!("零段报告应中断，实际 Ok: {raw}"),
    }

    // 假 spawn 记录面（模板展开同装置承接）
    let records = window.records();
    assert_eq!(records.len(), 1, "恰一次假 spawn");
    assert_eq!(records[0].argv[0], "mutants");
    assert_eq!(records[0].argv.get(1).map(String::as_str), Some("--in-diff"));
}

/// 异常：**超时 → 中断不挂死**——假 spawn 可编程自旋挂起（管道恒开、读挂
/// 起）+ 虚拟时钟推进越过段 spawn 硬超时（`start_paused` + 手动 `advance`——
/// 子进程管道读挂起时暂停时钟不自动推进，手动推进触发超时臂）→ `start_kill`
/// 强制终止 + `Interrupted` 超时文案，墙钟上界断言不挂死。
#[tokio::test(start_paused = true)]
async fn 超时中断_强制终止不挂死() {
    let ws = SpawnWs::new("timeout");
    let window = SpawnWindow::enter("cargo-timeout");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_sleep(1); // 记录后自旋挂起（stdout/stderr 恒开）
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    // 虚拟时钟推进：越过首个让出点（run_segment 已注册段 spawn 硬超时定时器）
    // 后手动 advance。
    tokio::spawn(async {
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS + 60_000)).await;
    });

    let started = Instant::now();
    let result = run_segment(&patch_path, &ws.exec_dir).await;
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(30),
        "超时中断不挂死（墙钟上界 {elapsed:?}）"
    );
    match result {
        Err(SegmentFailure::Interrupted(message)) => {
            assert!(
                message.contains("cargo-mutants 段执行超时（1800 秒）强制终止"),
                "超时中断文案（双时点②即时反馈面）: {message}"
            );
        }
        Err(SegmentFailure::Fatal(message)) => panic!("应超时中断而非 Fatal: {message}"),
        Ok(raw) => panic!("自旋挂起应超时中断，实际 Ok: {raw}"),
    }
}

/// 边界：**补丁入参边界**——段补丁空串 / 超大（>100KB）/ 含特殊字符（`\n` /
/// CRLF / 中文 / emoji）→ 补丁经文件传参（argv 只携路径），写盘往返逐字节不破
/// 坏、`--in-diff` 路径完整、段执行照常 Ok。
#[tokio::test]
async fn 补丁入参边界_空串超大与特殊字符不破坏() {
    let ws = SpawnWs::new("patch-bounds");
    let window = SpawnWindow::enter("cargo-patch");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_out(CARGO_SEG_CANNED);

    let patches: Vec<(usize, String)> = vec![
        // 空串补丁
        (0, String::new()),
        // 超大补丁（>100KB）
        (1, format!("diff --git a/f b/f\n+{}\n", "x".repeat(120_000))),
        // 特殊字符：CRLF / 中文 / emoji 内容
        (
            2,
            "diff --git a/文件 b/文件\r\n+加行 🎉\r\n-old 行\r\n+new 行 🎉\r\n".to_owned(),
        ),
    ];
    for (index, content) in &patches {
        window.reset_records();
        let patch_path = ws.write_patch(*index, content);
        let raw = run_segment(&patch_path, &ws.exec_dir)
            .await
            .unwrap_or_else(|error| panic!("段 {index} 边界补丁段执行应 Ok: {error}"));
        assert_eq!(
            raw.trim_end(),
            CARGO_SEG_CANNED,
            "段 {index} canned 原样（补丁面不扰输出捕获）"
        );

        // 文件传参面：写盘往返逐字节不破坏（补丁内容不进 argv——只携路径）
        assert_eq!(
            fs::read_to_string(&patch_path).expect("读回段补丁"),
            *content,
            "段 {index} 补丁内容往返不破坏"
        );

        // argv 面：路径完整（超大 / 特殊字符内容零进 argv）
        let records = window.records();
        assert_eq!(records.len(), 1, "段 {index} 恰一次假 spawn");
        assert_eq!(records[0].argv[1], "--in-diff");
        assert!(
            records[0].argv[2]
                .replace('\\', "/")
                .ends_with(&format!("mutation-seg-{index}.patch")),
            "段 {index} --in-diff 路径完整: {:?}",
            records[0].argv
        );
    }
}

/// 合并输出有界尾部（`output_tail` 消费面）：≤500 字原样、超限省略号前缀截断
/// （段报告面归因原料口径——mod.rs 单点、本模块引用对拍）。
#[test]
fn 合并输出有界尾部_超限省略号截断() {
    let short = "short output";
    assert_eq!(output_tail(short.as_bytes()), "short output", "≤500 原样");

    let long = "x".repeat(600);
    let tail = output_tail(long.as_bytes());
    assert_eq!(tail.chars().count(), 501, "截断至 500 + 省略号");
    assert!(tail.starts_with('…'), "省略号前缀示意截断");

    // 尾部优先（末段留痕——编译 / 执行错误通常在输出末段）
    let graded = format!("{}tail-marker", "a".repeat(600));
    assert!(
        output_tail(graded.as_bytes()).ends_with("tail-marker"),
        "末段优先留痕"
    );
}
