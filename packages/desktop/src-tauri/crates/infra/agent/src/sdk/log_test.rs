use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::sdk::log::{append_engine_log, init_engine_log};

// ---------------------------------------------------------------------------
// 装置：进程级共享日志目录 + 命中轮询读取
//
// `init_engine_log` 是一次性 `OnceLock`（首个调用生效、不可重置）：若各测试
// 各用独立临时目录，谁先 init 就把路径钉死在他处，其余读侧只能看到空文件。
// 故本进程内所有读日志的测试（本文件 + `runner_test` 的泵启动日志断言）共用
// 同一目录——目录对象由静态持有、不随单个用例结束删除。
// ---------------------------------------------------------------------------

/// 引擎日志共享目录（进程内单点；`tempfile` 只在进程生命周期内有效）。
///
/// 取值即顺带 init：`init_engine_log` 幂等（首个调用生效），全部调用点同指本
/// 目录，故任一用例先跑都成立——不依赖 `初始化后append落盘…` 先行执行（按名
/// 过滤单跑亦成立）。
pub(crate) fn engine_log_dir() -> &'static Path {
    static DIR: OnceLock<TempDir> = OnceLock::new();
    let dir = DIR
        .get_or_init(|| tempfile::tempdir().expect("创建日志临时目录失败"))
        .path();
    init_engine_log(dir);
    dir
}

/// 读回共享日志全文，直到命中判据成立（超时即失败）。
///
/// 写侧按格式片段多次 syscall，且测试并行——瞬时读到半行属预期，故命中判据
/// 轮询而非固定 sleep；判据恒不成立时携带实际内容失败（不静默放过）。
pub(crate) fn read_engine_log_until(hit: impl Fn(&str) -> bool) -> String {
    let path = engine_log_dir().join("engine.log");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if hit(&content) {
            return content;
        }
        assert!(
            Instant::now() < deadline,
            "日志未在超时内出现期望内容，实际内容: {content}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn 初始化后append落盘且带unix毫秒前缀() {
    // 共享目录（取值即 init，先于写）：OnceLock 进程单点下多处 init 恒指同一文件
    engine_log_dir();
    append_engine_log("泵启动 session=sdk-0-1");

    let content = read_engine_log_until(|content| content.contains("泵启动 session=sdk-0-1"));
    assert!(
        content.starts_with('['),
        "行首为 unix 毫秒前缀，实际: {content}"
    );
    assert!(
        content.contains("泵启动 session=sdk-0-1"),
        "正文逐字落盘: {content}"
    );
}

#[test]
fn 未初始化或初始化后append均不panic() {
    // 未初始化（或已初始化）两态下 append 恒安全（尽力而为语义的健壮半边；
    // 不对落盘位置做顺序敏感断言——OnceLock 进程单点不可重置）
    append_engine_log("两态恒安全");
}

#[test]
fn 公开面函数指针签名锚定() {
    let _append: fn(&str) = append_engine_log;
    let _init: fn(&Path) = init_engine_log;
}
