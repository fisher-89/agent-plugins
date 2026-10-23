use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use checks::model::parse_cargo_mutants_seg;
use tempfile::TempDir;

use super::super::mod_test::{wait_pid, wait_pid_gone, SpawnRecord, SpawnWindow};
use super::super::report::to_posix;
use super::super::{
    mutation_jobs, SegmentFailure, MUTATION_JOBS_DIVISOR, PER_MUTANT_TIMEOUT_SECS,
    SEGMENT_SPAWN_TIMEOUT_MS,
};
use super::{run_segment, segment_failure_message};

/// cargo-mutants 段报告 canned（真实 `mutants.out/outcomes.json` 全形态——
/// outcomes 逐条 + 计数面：1 caught + 1 missed / total 2）。
const CARGO_SEG_CANNED: &str = r#"{
  "outcomes": [
    { "scenario": { "Mutant": { "name": "src/lib.rs:3:5: replace add with 0" } }, "summary": "CaughtMutant" },
    { "scenario": { "Mutant": { "name": "src/lib.rs:3:5: replace add with ()" } }, "summary": "MissedMutant" }
  ],
  "total_mutants": 2,
  "missed": 1,
  "caught": 1,
  "timeout": 0,
  "unviable": 0,
  "success": false,
  "cargo_mutants_version": "27.1.0"
}"#;

/// argv 模板面常量锚定（AC-4：`--baseline skip` / per-mutant 超时 / 段 spawn
/// 硬超时 / jobs 半 CPU 除数——D9 不进 config schema）。
#[test]
fn argv模板常量面锚定() {
    assert_eq!(PER_MUTANT_TIMEOUT_SECS, 300, "per-mutant 超时 300 秒");
    assert_eq!(MUTATION_JOBS_DIVISOR, 2, "并发除数 2（半 CPU）");
    assert_eq!(
        SEGMENT_SPAWN_TIMEOUT_MS, 3_600_000,
        "段 spawn 硬超时 60 分钟"
    );
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
    assert!(
        message.contains("error: build failed"),
        "携输出尾部: {message}"
    );

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
        foundation::cancel::CancelFlag::never(),
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
        Err(SegmentFailure::Cancelled(error)) => {
            panic!("PATH 隔离下应 Fatal（拉起失败），实际 Cancelled: {error}")
        }
        Ok(raw) => panic!("PATH 隔离下应 Err，实际 Ok: {raw}"),
    }
}

// ---------------------------------------------------------------------------
// spawn 面（假 spawn）用例：argv 模板展开 / CARGO_TARGET_DIR 多段共享 / 段报
// 告权威 / 零突变体段 / 陈旧清场 / 非零退出中断 / 超时中断 / 补丁入参边界
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
        let path = self
            .mutation_dir
            .join(format!("mutation-seg-{index}.patch"));
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

/// 正向：**spawn 模板展开 + 段报告权威落位**——argv 逐参等值（`mutants
/// --workspace --in-diff <段补丁> --baseline skip --jobs <半 CPU> --timeout
/// 300`，AC-4）+ spawn cwd 捕获 + `CARGO_TARGET_DIR` env 面捕获 + 段报告 =
/// `mutants.out/outcomes.json` 原样（真实工具全形态 canned，解析对拍 1 killed +
/// 1 missed）。
#[tokio::test]
async fn spawn模板展开_argv逐参等值与报告权威落位() {
    let ws = SpawnWs::new("argv");
    let window = SpawnWindow::enter("cargo-argv");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_cargo_outcomes(CARGO_SEG_CANNED);
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    let raw = run_segment(
        &patch_path,
        &ws.exec_dir,
        foundation::cancel::CancelFlag::never(),
    )
    .await
    .expect("假 spawn 段执行应 Ok");
    assert_eq!(
        raw.trim_end(),
        CARGO_SEG_CANNED.trim_end(),
        "段报告 = canned 原样（mutants.out/outcomes.json 权威落位）"
    );
    let parsed = parse_cargo_mutants_seg(&raw).expect("真实 outcomes 形态可解析");
    assert_eq!(parsed.measured.killed, 1, "caught → killed");
    assert_eq!(parsed.measured.survived, 1, "missed → survived");
    assert_eq!(parsed.measured.total, 2, "计数面直读 total");

    let records = window.records();
    assert_eq!(records.len(), 1, "恰一次假 spawn");
    let argv = &records[0].argv;
    assert_eq!(argv.len(), 10, "argv 十参封闭: {argv:?}");
    assert_eq!(argv[0], "mutants", "argv 首 token（cargo 子命令面）");
    assert_eq!(
        argv[1], "--workspace",
        "workspace 突变面（混合 workspace 全成员扫描）"
    );
    assert_eq!(argv[2], "--in-diff", "段补丁入参位");
    assert!(
        argv[3].replace('\\', "/").ends_with("mutation-seg-0.patch"),
        "--in-diff 携段补丁路径: {argv:?}"
    );
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
    window.set_cargo_outcomes(CARGO_SEG_CANNED);

    let patch0 = ws.write_patch(0, "diff --git a/app/src/a.rs b/app/src/a.rs\n");
    let patch1 = ws.write_patch(1, "diff --git a/app/src/b.rs b/app/src/b.rs\n");
    run_segment(
        &patch0,
        &ws.exec_dir,
        foundation::cancel::CancelFlag::never(),
    )
    .await
    .expect("段 0 应 Ok");
    run_segment(
        &patch1,
        &ws.exec_dir,
        foundation::cancel::CancelFlag::never(),
    )
    .await
    .expect("段 1 应 Ok");

    let records = window.records();
    assert_eq!(records.len(), 2, "两段各恰一次假 spawn");
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.argv[0], "mutants", "段 {index} argv 首 token");
        assert_eq!(record.argv[2], "--in-diff");
        assert!(
            record.argv[3]
                .replace('\\', "/")
                .ends_with(&format!("mutation-seg-{index}.patch")),
            "段 {index} --in-diff 携各自段补丁: {:?}",
            record.argv
        );
    }

    let dir0 = env_of(&records[0], "CARGO_TARGET_DIR").expect("段 0 env 捕获");
    let dir1 = env_of(&records[1], "CARGO_TARGET_DIR").expect("段 1 env 捕获");
    assert_eq!(
        dir0, dir1,
        "多段共享同一 CARGO_TARGET_DIR（MUST NOT 每段重置）"
    );
    assert_eq!(
        dir0.replace('\\', "/"),
        to_posix(&ws.exec_dir.join("target")),
        "CARGO_TARGET_DIR = exec_dir/target"
    );
}

/// 正向（零突变体段）：**退出 0 且无报告 = diff 零可突变交叠**——cargo-mutants
/// 对 EmptyDiff / NoSourceFiles / NoMutants 恒退出 0 且不产报告（非故障语义）
/// → 合成空计数面载荷（解析全零、不中断）。
#[tokio::test]
async fn 退出零无报告_零突变体段合成空计数面() {
    let ws = SpawnWs::new("zero-mutants");
    let window = SpawnWindow::enter("cargo-zero");
    window.install_fake_cargo(&ws.exec_dir);
    // 不编程 outcomes —— 假执行体不写报告（真实工具零交叠面同形）
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    let raw = run_segment(
        &patch_path,
        &ws.exec_dir,
        foundation::cancel::CancelFlag::never(),
    )
    .await
    .expect("退出 0 无报告 = 零突变体段（非中断）");
    let parsed = parse_cargo_mutants_seg(&raw).expect("空计数面载荷可解析");
    assert_eq!(
        (
            parsed.measured.killed,
            parsed.measured.survived,
            parsed.measured.timeout,
            parsed.measured.compile_error,
            parsed.measured.runtime_error,
            parsed.measured.ignored,
            parsed.measured.total,
        ),
        (0, 0, 0, 0, 0, 0, 0),
        "零突变体段计数全零（空段 valid == 0 → 归并 score 100 恒过）"
    );
    assert_eq!(window.records().len(), 1, "恰一次假 spawn");
}

/// 正向（陈旧清场）：**spawn 前清除陈旧 `mutants.out`**——上一段 / 上次运行的
/// 陈旧 outcomes.json MUST NOT 被本段误读：预置陈旧报告（caught 5）+ 假执行体
/// 零交叠面（不写报告、退出 0）→ 返回空计数面而非陈旧内容，且陈旧报告已被清除。
#[tokio::test]
async fn 陈旧报告清场_上一段残留不误读() {
    let ws = SpawnWs::new("stale");
    let window = SpawnWindow::enter("cargo-stale");
    window.install_fake_cargo(&ws.exec_dir);
    // 预置陈旧报告（上一段残留面）
    let stale_dir = ws.exec_dir.join("mutants.out");
    fs::create_dir_all(&stale_dir).expect("建陈旧报告目录");
    fs::write(
        stale_dir.join("outcomes.json"),
        r#"{ "caught": 5, "missed": 0, "timeout": 0, "unviable": 0 }"#,
    )
    .expect("写陈旧 outcomes");

    // 假执行体不写报告（本段零交叠面）——陈旧报告必须被清场而非误读
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");
    let raw = run_segment(
        &patch_path,
        &ws.exec_dir,
        foundation::cancel::CancelFlag::never(),
    )
    .await
    .expect("清场后零交叠段应空计数面");
    let parsed = parse_cargo_mutants_seg(&raw).expect("空计数面载荷可解析");
    assert_eq!(
        parsed.measured.killed, 0,
        "陈旧 caught=5 MUST NOT 被本段误读（spawn 前清场）"
    );
    assert!(
        !ws.exec_dir.join("mutants.out").exists(),
        "陈旧 mutants.out 已被 spawn 前清场移除"
    );
}

/// 异常：**非零退出 → 输出捕获 + 中断**——假 spawn 可编程非零退出（退出码
/// 101）且零段报告 → `SegmentFailure::Interrupted` 携退出码与合并输出有界尾
/// 部（双时点②中断面原料）。
#[tokio::test]
async fn 非零退出中断_携退出码与输出尾部捕获() {
    let ws = SpawnWs::new("exit101");
    let window = SpawnWindow::enter("cargo-exit");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_out("cargo-mutants harness boom-marker"); // 非 JSON 损坏输出
    window.set_exit(101);
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    let result = run_segment(
        &patch_path,
        &ws.exec_dir,
        foundation::cancel::CancelFlag::never(),
    )
    .await;
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
    assert_eq!(records[0].argv[0], "mutants");
    assert_eq!(
        records[0].argv.get(2).map(String::as_str),
        Some("--in-diff")
    );
}

/// 异常：**超时 → 中断不挂死 + pid 回收**——假 spawn 可编程自旋挂起（管道恒
/// 开、读挂起）+ 虚拟时钟推进越过段 spawn 硬超时（`start_paused` + 手动
/// `advance`——子进程管道读挂起时暂停时钟不自动推进，手动推进触发超时臂）→
/// `kill_tree` 强制终止 + `Interrupted` 超时文案 + 目标 pid 消失，墙钟上界断言
/// 不挂死。
#[tokio::test(start_paused = true)]
async fn 超时中断_强制终止且pid消失不挂死() {
    let ws = SpawnWs::new("timeout");
    let window = SpawnWindow::enter("cargo-timeout");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_sleep(1); // 记录后自旋挂起（stdout/stderr 恒开）
    let pid_file = ws.mutation_dir.join("cargo-timeout-pid.txt");
    window.set_pidfile(&pid_file);
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    // 段执行在任务内（主任务负责「等 pid 落盘 → 推进虚拟时钟」的握手）
    let exec_dir = ws.exec_dir.clone();
    let runner = tokio::spawn(async move {
        run_segment(
            &patch_path,
            &exec_dir,
            foundation::cancel::CancelFlag::never(),
        )
        .await
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
                message.contains("cargo-mutants 段执行超时（3600 秒）强制终止"),
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

/// 边界：**补丁入参边界**——段补丁空串 / 超大（>100KB）/ 含特殊字符（`\n` /
/// CRLF / 中文 / emoji）→ 补丁经文件传参（argv 只携路径），写盘往返逐字节不破
/// 坏、`--in-diff` 路径完整、段执行照常 Ok。
#[tokio::test]
async fn 补丁入参边界_空串超大与特殊字符不破坏() {
    let ws = SpawnWs::new("patch-bounds");
    let window = SpawnWindow::enter("cargo-patch");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_cargo_outcomes(CARGO_SEG_CANNED);

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
        let raw = run_segment(
            &patch_path,
            &ws.exec_dir,
            foundation::cancel::CancelFlag::never(),
        )
        .await
        .unwrap_or_else(|error| panic!("段 {index} 边界补丁段执行应 Ok: {error}"));
        assert_eq!(
            raw.trim_end(),
            CARGO_SEG_CANNED.trim_end(),
            "段 {index} canned 原样（补丁面不扰报告落位）"
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
        assert_eq!(records[0].argv[2], "--in-diff");
        assert!(
            records[0].argv[3]
                .replace('\\', "/")
                .ends_with(&format!("mutation-seg-{index}.patch")),
            "段 {index} --in-diff 路径完整: {:?}",
            records[0].argv
        );
    }
}

/// 段中断错误面双段形态（exec `ProcessOutput` 装配——行口径末尾与错误信息
/// 段；覆盖面主体在 exec `output_test`，此处只对拍本模块 head 接线）。
#[test]
fn 段中断错误面_双段装配接线() {
    let message = segment_failure_message(
        101,
        b"unmutated
error: build failed
tail-marker
"
        .as_ref(),
    );
    assert!(message.contains("cargo-mutants 段执行失败（退出码 101，未产出可解析 JSON 段报告）"));
    assert!(
        message.contains(
            "错误信息：
error: build failed"
        ),
        "错误信息段在案"
    );
    assert!(message.contains("输出末尾（50 行）："), "输出末尾段在案");
    assert!(message.ends_with("tail-marker"), "末段留痕");
    // 空输出 → 裸 head（无冒号缀）
    assert_eq!(
        segment_failure_message(1, b""),
        "cargo-mutants 段执行失败（退出码 1，未产出可解析 JSON 段报告）"
    );
}

// ---------------------------------------------------------------------------
// 取消臂（test-design「mutation/cargo_mutants.rs -> cargo_mutants_test.rs」节，
// AC-4 / D13）：段执行中置位取消 → `SegmentFailure::Cancelled`（**不是**
// `Fatal` / `Interrupted`）+ 取消文案；入口短路（已置位）不 spawn。
// ---------------------------------------------------------------------------

/// 正向（AC-4）：运行中 cancel——自旋假 cargo 运行中置位 → `Cancelled` 文案
///「cargo-mutants 段执行已取消」（与超时 `Interrupted` / 拉起失败 `Fatal` 三变体
/// 可辨）+ 墙钟上界。
#[tokio::test]
async fn 运行中cancel_段取消变体与文案且秒级收敛() {
    let ws = SpawnWs::new("cancel");
    let window = SpawnWindow::enter("cargo-cancel");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_sleep(1); // 记录后自旋挂起
    let pid_file = ws.mutation_dir.join("cargo-cancel-pid.txt");
    window.set_pidfile(&pid_file);
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    let cancel = foundation::cancel::CancelFlag::never();
    let handle = cancel.handle();
    let runner = tokio::spawn(async move { run_segment(&patch_path, &ws.exec_dir, cancel).await });

    let pid = wait_pid(&pid_file, Duration::from_secs(60)).await;
    let started = Instant::now();
    handle.set();

    let result = runner.await.expect("段任务应正常结束");
    let elapsed = started.elapsed();
    match result {
        Err(SegmentFailure::Cancelled(message)) => {
            assert_eq!(
                message, "cargo-mutants 段执行已取消",
                "取消文案逐字（与 Interrupted / Fatal 文案独立）"
            );
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
        elapsed < Duration::from_secs(20),
        "取消秒级收敛（墙钟 {elapsed:?}）"
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(15)),
        "取消强杀后段进程 pid {pid} 应消失（kill_tree 生效）"
    );
}

/// 正向（AC-4 / D4）：已置位 cancel 的入口短路——`Cancelled` 且假 cargo 零
/// 记录（段执行体不拉起进程）。
#[tokio::test]
async fn 已置位cancel_入口短路零spawn() {
    let ws = SpawnWs::new("cancel-pre");
    let window = SpawnWindow::enter("cargo-cancel-pre");
    window.install_fake_cargo(&ws.exec_dir);
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    let cancel = foundation::cancel::CancelFlag::never();
    cancel.handle().set();
    let result = run_segment(&patch_path, &ws.exec_dir, cancel).await;
    match result {
        Err(SegmentFailure::Cancelled(message)) => {
            assert_eq!(message, "cargo-mutants 段执行已取消");
        }
        other => panic!("已置位 cancel 应 Cancelled，实际 {other:?}"),
    }
    assert!(
        window.records().is_empty(),
        "入口短路零 spawn（取消不拉起 cargo）"
    );
}

// ---------------------------------------------------------------------------
// 合并流与大输出（test-design「mutation/cargo_mutants.rs -> cargo_mutants_test.rs」
// 节「合并流与大输出 | 边界 | 双管道各 >64 KiB → `output` 全长「stdout ++
// stderr」逐字节（取消 / 超时臂不消费输出）| 表驱动（2 形态：空 / 大块）」）：
// `Completed` 臂的合并输出面（`stdout` 前缀 + `stderr` 后缀）经段中断错误文案
// （`exec::ProcessOutput` 的输出末尾段——行数 ≤ 50 时不截断）逐字节观测。
// ---------------------------------------------------------------------------

/// 边界（表驱动 2 形态）：段合并输出的「空」与「大块」两形态——空 → 裸 head
/// （无尾部段、无冒号缀）；双管道各 >64 KiB → `output` 全长「stdout ++ stderr」
/// 逐字节（零丢字节、零重排、零截断），且并发排水不互锁（墙钟上界）。
///
/// 环境面注记：Windows 下假 `cargo` 由 msys bash 承载，宿主缺 `/tmp` 时 bash 会
/// 在 **stderr 首行** 打启动告警（`bash.exe: warning: could not find /tmp…`）——
/// 属假执行体的环境噪声，不属本层合流语义，故大块形态以「stdout 载荷整段在前 +
/// stderr 载荷整段收尾」为准（两路各 70,000 字节逐字节在位 = 不丢字节），空形态以
/// 「head 逐字 + 载荷零在案」为准（真正空输出的裸 head 语义由 `segment_failure_message`
/// 直驱同名用例钉死）。
#[tokio::test]
async fn 合并流与大输出_双管道大块逐字节() {
    const HEAD_EMPTY: &str = "cargo-mutants 段执行失败（退出码 1，未产出可解析 JSON 段报告）";
    const HEAD_BIG: &str = "cargo-mutants 段执行失败（退出码 101，未产出可解析 JSON 段报告）";
    const TAIL_MARKER: &str = "输出末尾（50 行）：\n";

    // 形态一：双空输出（非零退出且无段报告 → 合并输出为空 → 裸 head）
    {
        let ws = SpawnWs::new("merge-empty");
        let window = SpawnWindow::enter("cargo-merge-empty");
        window.install_fake_cargo(&ws.exec_dir);
        window.set_exit(1);
        let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

        let result = tokio::time::timeout(
            Duration::from_secs(60),
            run_segment(
                &patch_path,
                &ws.exec_dir,
                foundation::cancel::CancelFlag::never(),
            ),
        )
        .await
        .expect("空输出形态不应挂死（墙钟上界 60s）");
        match result {
            Err(SegmentFailure::Interrupted(message)) => {
                assert!(
                    message.starts_with(HEAD_EMPTY),
                    "空输出 → head 逐字（退出码 + 记因）: {message}"
                );
                assert!(
                    !message.contains(&"S".repeat(64)),
                    "空输出无任何 stdout 载荷: {message}"
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
        // 真空输出的「裸 head」语义（无错误摘要段 / 无尾部段）直驱钉死
        assert_eq!(
            segment_failure_message(1, b""),
            HEAD_EMPTY,
            "空输出 → 裸 head（无「错误信息」/「输出末尾」段）"
        );
        assert_eq!(window.records().len(), 1, "空形态恰一次假 spawn");
    }

    // 形态二：双管道各 >64 KiB（单路一行超管道缓冲——顺序读会互锁/丢字节）
    {
        // 管道缓冲上限 64 KiB：单路 70_000 字节显著越过
        const BIG: usize = 70_000;
        let stdout_line = "S".repeat(BIG);
        let stderr_line = "E".repeat(BIG);

        let ws = SpawnWs::new("merge-big");
        let window = SpawnWindow::enter("cargo-merge-big");
        window.install_fake_cargo(&ws.exec_dir);
        window.set_out(&format!("{stdout_line}\n"));
        window.set_err(&format!("{stderr_line}\n"));
        window.set_exit(101);
        let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

        let started = Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(60),
            run_segment(
                &patch_path,
                &ws.exec_dir,
                foundation::cancel::CancelFlag::never(),
            ),
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
        assert!(
            message.starts_with(HEAD_BIG),
            "头行段逐字: {}",
            &message[..message.len().min(80)]
        );
        let tail = message
            .split_once(TAIL_MARKER)
            .map(|(_, tail)| tail)
            .expect("输出末尾段在案（合并输出非空）");
        // 两路载荷整段在位（各 70,000 字节逐字节——不丢字节、不截断）
        let stdout_at = tail
            .find(&stdout_line)
            .unwrap_or_else(|| panic!("stdout 载荷整段在位（{BIG} 字节，无截断/交错）"));
        let stderr_at = tail
            .find(&stderr_line)
            .unwrap_or_else(|| panic!("stderr 载荷整段在位（{BIG} 字节，无截断/交错）"));
        assert!(
            stdout_at < stderr_at,
            "output 序 = 「stdout 前缀 + stderr 后缀」（stdout@{stdout_at} vs stderr@{stderr_at}）"
        );
        assert_eq!(
            tail.matches(stdout_line.as_str()).count(),
            1,
            "stdout 载荷整段恰一次（无重复 / 无交错重排）"
        );
        assert_eq!(
            tail.matches(stderr_line.as_str()).count(),
            1,
            "stderr 载荷整段恰一次（无重复 / 无交错重排）"
        );
        assert!(
            tail.ends_with(stderr_line.as_str()),
            "stderr 载荷收尾（末行全长未被截断）"
        );
        assert!(
            message.len() >= HEAD_BIG.len() + TAIL_MARKER.len() + 2 * BIG,
            "合并输出全量在案（≥ head + 两路各 {BIG} 字节，实际 {}）",
            message.len()
        );
        assert!(
            elapsed < Duration::from_secs(40),
            "双管道并发排水不互锁（墙钟 {elapsed:?}）"
        );
        assert_eq!(window.records().len(), 1, "大块形态恰一次假 spawn");
    }
}

/// 正向（AC-2）：零重试——段输出含瞬态签名（`os error 32`）+ `exit 1` →
/// 段尝试次数恰 1（假 spawn 记录仅一条）、分流仍为 `Interrupted`（段层
/// `RetryPolicy::none()`：签名命中不追加尝试）。
#[tokio::test]
async fn 输出含瞬态签名不触发段重试() {
    let ws = SpawnWs::new("no-retry");
    let window = SpawnWindow::enter("cargo-no-retry");
    window.install_fake_cargo(&ws.exec_dir);
    window.set_out("error: failed to rename archive file: /t/liba.rlib: os error 32");
    window.set_exit(1);
    let patch_path = ws.write_patch(0, "diff --git a/app/src/lib.rs b/app/src/lib.rs\n");

    let result = run_segment(
        &patch_path,
        &ws.exec_dir,
        foundation::cancel::CancelFlag::never(),
    )
    .await;

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
