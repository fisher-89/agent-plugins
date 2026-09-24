//! watch 命令轨道：通用单文件 watch 失效信号通道的壳层桥接（第二条被认可
//! 的推送语义，与 exec 执行流通道并列）。
//!
//! notify 订阅与信号流归 `watch` infra crate（零 Tauri）；本轨道只做两件事：
//! `State` 托管订阅注册表（canonical path 键幂等——同一路径重复订阅返回既有
//! subscription_id，不产生重复信号）+ std mpsc → Tauri `Channel` 桥接线程。
//! 通知只携带「目标文件被修改」信号（无内容字节），数据面维持显式取数纪律：
//! 前端防抖后经既有查询命令显式拉取，后端零去抖。
//!
//! 生命周期：桥接线程随通道关闭退出（[`watch::Watcher`] drop → 发送端关闭 →
//! `recv` 出错 → 线程结束）；订阅生命周期即消费页面生命周期。
//! 能力 spec：`specs/desktop-file-watch/spec.md`（路径相对域根）。

use std::collections::HashMap;
use std::path::Path;

#[cfg(test)]
mod mod_test;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use watch::{FileWatchSignal, Watcher};

/// 桥接后的前端信号载荷（与 `FileWatchSignal` 同构，camelCase；无内容字节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWatchEvent {
    /// 被修改目标的订阅路径
    pub path: String,
}

/// 单条订阅：watch 句柄（drop 即停流退订）+ 幂等键（canonical 路径）。
struct WatchSubscription {
    _watcher: Watcher,
    /// canonical 路径键（canonicalize 失败时回退原串，如目标尚未落盘）
    key: String,
}

/// 订阅注册表（`State` 托管，setup `manage`）：subscription_id → 订阅。
pub struct WatchRegistry {
    next_id: AtomicU64,
    subscriptions: Mutex<HashMap<u64, WatchSubscription>>,
}

impl Default for WatchRegistry {
    fn default() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            subscriptions: Mutex::new(HashMap::new()),
        }
    }
}

/// 幂等键：canonicalize 主口径（同文件不同书写形式归一）；目标缺失等失败
/// 回退原串（订阅目标允许尚未存在，不得因此报错）。
fn idempotency_key(path: &str) -> String {
    std::fs::canonicalize(Path::new(path))
        .map(|canonical| canonical.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_owned())
}

/// 订阅单个文件：建 infra 订阅 + 桥接线程（mpsc 信号 → Channel 事件推送），
/// 返回 subscription_id。同一路径重复订阅幂等（命中既有订阅原样返回 id，
/// 不新建、不产生重复信号）；目标缺失不报错（信号流就绪，目标出现后生效）。
#[tauri::command]
pub fn watch_subscribe(
    registry: State<'_, WatchRegistry>,
    on_event: Channel<FileWatchEvent>,
    path: String,
) -> Result<u64, String> {
    let key = idempotency_key(&path);
    let mut subscriptions = registry
        .subscriptions
        .lock()
        .map_err(|_| "订阅注册表锁不可恢复".to_owned())?;
    if let Some((id, _)) = subscriptions.iter().find(|(_, sub)| sub.key == key) {
        return Ok(*id);
    }
    let (sender, receiver) = mpsc::channel::<FileWatchSignal>();
    let watcher =
        watch::subscribe(Path::new(&path).to_path_buf(), sender).map_err(|e| e.to_string())?;
    // 桥接线程：信号 → Channel 推送；发送端随 Watcher drop 关闭后线程自然退出
    std::thread::spawn(move || {
        while let Ok(signal) = receiver.recv() {
            let _ = on_event.send(FileWatchEvent { path: signal.path });
        }
    });
    let id = registry.next_id.fetch_add(1, Ordering::Relaxed);
    subscriptions.insert(
        id,
        WatchSubscription {
            _watcher: watcher,
            key,
        },
    );
    Ok(id)
}

/// 解除订阅：移除并 drop `Watcher`（停流 + 桥接线程随通道关闭退出）；
/// miss 幂等返回 `false`。
#[tauri::command]
pub fn watch_unsubscribe(
    registry: State<'_, WatchRegistry>,
    subscription_id: u64,
) -> Result<bool, String> {
    let mut subscriptions = registry
        .subscriptions
        .lock()
        .map_err(|_| "订阅注册表锁不可恢复".to_owned())?;
    Ok(subscriptions.remove(&subscription_id).is_some())
}
