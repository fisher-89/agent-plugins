//! 退出关库钩子：Tauri 在 Windows 上以 `std::process::exit` 结束进程（tao 的
//! `EventLoop::run`），MUST NOT 依赖 App / managed state 的析构清理 redb。
//! 不在退出路径显式关库时，文件头的 `recovery_required` 永远留脏，下次启动
//! 必做全库修复扫描（GB 级库秒级）——本模块负责在 `RunEvent::Exit` 主动关库，
//! 让下一次打开走 allocator-state 快路径。
//!
//! 有界等待：后台事务（agent 流 / 通知 sink）可能仍在收尾，关库线程不无限
//! 阻塞退出；超时即放弃（本次退出留脏，下次修复；UI 不冻结由命令线程纪律
//! 兜底）。

use std::time::Duration;

use store::WorkspaceStores;
use tauri::{AppHandle, Manager};

/// 退出关库等待上限（关库本身是毫秒级；上限只兜住卡住的事务）。
pub const CLOSE_WAIT: Duration = Duration::from_secs(3);

/// 主动关掉全局库与全部已缓存 workspace 库（幂等；不依赖任何析构顺序）。
pub fn shutdown_stores<R: tauri::Runtime>(handle: &AppHandle<R>) {
    let handle = handle.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        handle.state::<WorkspaceStores>().close_all();
        let _ = tx.send(());
    });
    if rx.recv_timeout(CLOSE_WAIT).is_err() {
        eprintln!(
            "[store] 退出关库超时（{}s），跳过；下次启动将做一次修复扫描",
            CLOSE_WAIT.as_secs()
        );
    }
}

#[cfg(test)]
#[path = "shutdown_test.rs"]
mod shutdown_test;
