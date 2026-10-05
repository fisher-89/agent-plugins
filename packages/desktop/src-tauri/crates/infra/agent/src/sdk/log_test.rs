use std::path::Path;

use crate::sdk::log::{append_engine_log, init_engine_log};

#[test]
fn 初始化后append落盘且带unix毫秒前缀() {
    // OnceLock 进程单点：首次 init 生效（测试进程内其它用例未先行初始化即
    // 本用例成立；先行初始化则本用例 append 落他目录、本目录无文件——以
    // 独立临时目录隔离，不与其它日志用例并行断言同目录）
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    init_engine_log(dir.path());
    append_engine_log("泵启动 session=sdk-0-1");

    let content = std::fs::read_to_string(dir.path().join("engine.log")).expect("日志文件应已创建");
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
