//! `watch`：通用单文件 watch 失效信号通道（与执行流通道并列的第二推送语义）。
//!
//! 职责边界：notify 订阅**单个文件**，产出「目标文件被修改」信号流（std
//! mpsc）——通知只携带信号不含任何内容字节，是失效信号通道而非数据通道；
//! 数据面维持显式取数纪律，前端收到信号后自行防抖再显式拉取，本 crate 零
//! 去抖、零内容读取。Tauri `Channel` 包装只出现在壳层命令（core/infra 禁
//! Tauri 纪律不变）。
//!
//! # 订阅语义
//!
//! - 目标不存在不报错：信号流就绪，目标（重）出现后事件生效；
//! - 同一路径重复订阅的幂等由壳层命令以 canonical path 键承载，本 crate
//!   不去重（notify 底层同路径双挂由 OS 合并，但订阅句柄各自独立）；
//! - [`Watcher`] drop 即停流退订：notify watcher 与信号 Sender 随句柄一起
//!   释放，退订后不再有信号到达。
//!
//! # 平台边界（留痕，不预建兼容层）
//!
//! 实现统一经**父目录**挂 notify watch（事件按目标路径过滤），而非直接挂
//! 文件本身：
//! - Windows（当前目标平台）：notify 底层为父目录 `ReadDirectoryChangesW`，
//!   编辑器 rename-replace 原子保存形态下事件不丢、监听不失效；
//! - 类 Linux/inotify：单文件 watch 存在 rename 失效陷阱（rename-replace 后
//!   监听指向旧 inode），父目录挂载同样免疫该陷阱，跨平台支持时再复核。
//!
//! # 两种「目标不可达」形态（可分辨，MUST NOT 合并为同一静默 `Ok`）
//!
//! - **目标文件缺失、父目录在**：挂父目录监听，目标（重）出现后事件生效
//!   （订阅成功，语义零变）；
//! - **父目录亦缺失**：显式失败 [`WatchError::ParentUnavailable`]（携带缺失父
//!   目录路径）——原形态下 `watcher.watch()` 压根未被调用而 `Result` 仍是 `Ok`，
//!   调用方无法分辨两者。壳层命令的既有错误串映射零改动即出线新因。
//!
//! 失败臂的三条 MUST NOT：**不替调用方造目录**（零 fs 副作用）、**不递归挂祖先
//! 目录**（无上界递归的成本不可控）、**不引入退避重试 / 重挂状态机**。恢复路径
//! 是调用方的**顺序不变量**：写入方先创建记录（目录与文件落盘）再向消费侧暴露
//! 可订阅地址，使「订阅时父目录必然已在」成为调用序列的性质而非运行期兜底。
//!
//! 边界限制（`watch_target_of` 休眠语义本体，零改动）：父目录缺失时本 crate 不
//! 预建目录也不挂祖先——只是该形态由「订阅成功但 OS 监听休眠」改为显式失败。

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use notify::{
    recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _,
};
use serde::{Deserialize, Serialize};

/// 失效信号：仅「目标文件被修改」事实，无任何内容字节（serde camelCase，
/// 与壳层桥接后的 `FileWatchEvent` 同构）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWatchSignal {
    /// 被修改目标的订阅路径（订阅时原样回传，供消费侧区分订阅）
    pub path: String,
}

/// 订阅失败面两分；目标**文件**缺失不算失败（见 crate 文档）。
#[derive(Debug)]
pub enum WatchError {
    /// notify 初始化或挂载失败
    Notify(notify::Error),
    /// 目标父目录缺失：本次订阅不可能生效（不预建目录、不递归挂祖先、不重试）
    ParentUnavailable(PathBuf),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Notify(error) => write!(f, "watch: {error}"),
            Self::ParentUnavailable(parent) => {
                write!(f, "watch: 目标父目录不存在 {}", parent.display())
            }
        }
    }
}

impl std::error::Error for WatchError {}

/// 订阅句柄：持 notify watcher 与信号 Sender——句柄存活即信号流通；drop 即
/// 停流退订（notify watcher 释放 + 通道发送端关闭）。
pub struct Watcher {
    /// notify 监听器（drop 即解除 OS 监听；存活价值即持有的释放语义，无读取方）
    #[allow(dead_code)]
    inner: RecommendedWatcher,
    /// 信号 Sender 存根：保住通道发送端不因回调闭包独占而提前关闭
    _sender: Sender<FileWatchSignal>,
}

/// 订阅单个文件的修改信号：事件经 `sender` 逐条投递（接收端消失时发送静默
/// 失败，退订语义由 [`Watcher`] drop 承载）。目标**文件**缺失不报错（父目录存在
/// 时事件在目标出现后生效）；**父目录亦缺失** → `Err(WatchError::ParentUnavailable)`
/// ——MUST NOT 返回一个永不产信号的 `Ok` 订阅。
pub fn subscribe(path: PathBuf, sender: Sender<FileWatchSignal>) -> Result<Watcher, WatchError> {
    let signal_path = path.to_string_lossy().into_owned();
    let Some(watch_target) = watch_target_of(&path) else {
        return Err(WatchError::ParentUnavailable(parent_of(&path)));
    };
    let callback_sender = sender.clone();
    let mut watcher = recommended_watcher(move |result: Result<Event, notify::Error>| {
        let Ok(event) = result else {
            return; // 单条事件读取失败：丢弃本条，信号流继续（无内容语义可丢）
        };
        if !is_target_change(&event, Path::new(&signal_path)) {
            return;
        }
        // 接收端已消失（页面退订后桥接线程退出）：静默丢弃，不 panic
        let _ = callback_sender.send(FileWatchSignal {
            path: signal_path.clone(),
        });
    })
    .map_err(WatchError::Notify)?;
    watcher
        .watch(&watch_target, RecursiveMode::NonRecursive)
        .map_err(WatchError::Notify)?;
    Ok(Watcher {
        inner: watcher,
        _sender: sender,
    })
}

/// 实际挂载目标：目标已存在挂目标本身亦可，但为跨平台统一语义（rename-
/// replace 原子保存不丢监听，见 crate 文档）一律挂**父目录**；父目录缺失
/// 返回 `None`（不预建目录、不挂祖先）。
///
/// 本函数判定本体零改动——仅其 `None` 出口在 [`subscribe`] 内改为
/// [`WatchError::ParentUnavailable`]（不再静默 `Ok`）。
fn watch_target_of(target: &Path) -> Option<PathBuf> {
    let parent = target.parent().filter(|p| !p.as_os_str().is_empty())?;
    parent.is_dir().then(|| parent.to_path_buf())
}

/// 失败面携带的「缺失父目录」路径（根路径无父时回退目标自身）。
fn parent_of(target: &Path) -> PathBuf {
    target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| target.to_path_buf())
}

/// 事件过滤：仅「目标路径 + 事实性变更」产信号——Create / Modify / Remove
/// （含 Any 兜底）命中目标路径才算；Access（读取）与 Other 不产信号。
fn is_target_change(event: &Event, target: &Path) -> bool {
    let factual = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any
    );
    factual && event.paths.iter().any(|path| path == target)
}

#[cfg(test)]
mod lib_test;
