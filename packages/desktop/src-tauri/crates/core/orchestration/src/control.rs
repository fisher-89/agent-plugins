//! run 控制注册表（进程内，键 = `(workspace root, change)` 复合——design D10
//! 修掉两 workspace 同名 change 假冲突先例 bug，worktree 隔离解锁同 workspace
//! 多 change 真并行）：并行冲突检测、cancel watch、当前会话 id 槽、ask /
//! 确认单次应答通道、`RunUpdate` broadcast、重挂快照。run 生命周期由本注册
//! 表承载（不建 flow_runs 表）；run 终态即除名，桌面重启后 run 消失（重新
//! 发起自 active_phase 续走）。root 段口径同身份段（命令入参 canonical root
//! 契约，进程内无 IO）。发布单点：[`Self::publish`] 同步快照面并广播——
//! walker 经 [`RunGuard::emit`] 间达，WorkerAgent 会话事件经命令层 sink 桥
//! 直发。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, oneshot, watch};

use crate::state::{AskPayload, ChangeRunSnapshot, ChangeRunStatus, RunUpdate};

/// broadcast 通道容量（步状态 + 会话事件窗口；溢出即滞后，由订阅侧重挂
/// 快照兜底）。
const UPDATE_CAPACITY: usize = 512;

/// 注册表条目（per-run 运行态）。
struct RunEntry {
    run_id: String,
    status: ChangeRunStatus,
    /// 当前相位（停等定位）
    phase: Option<String>,
    /// 当前 attempt
    attempt: Option<u32>,
    /// waitingAsk 载荷
    ask: Option<AskPayload>,
    cancel: watch::Sender<bool>,
    /// 当前 WorkerAgent 会话 id 槽（停止寻址经既有 StopRegistry）
    session: Mutex<Option<String>>,
    confirm_tx: Mutex<Option<oneshot::Sender<bool>>>,
    answer_tx: Mutex<Option<oneshot::Sender<String>>>,
    updates: broadcast::Sender<RunUpdate>,
}

/// run 控制注册表：命令层读写、walker 持 [`RunGuard`] 写。
#[derive(Default)]
pub struct ChangeFlowControl {
    runs: Mutex<HashMap<(String, String), RunEntry>>,
}

impl ChangeFlowControl {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 发起登记：同 `(root, change)` 已有 run → `Err`（并行冲突；异 workspace
    /// 同名 change 不误拒）；否则登记 cancel watch 与 broadcast，返回 walker
    /// 控制柄（guard 内持注册表 Arc——命令层以 `Arc<ChangeFlowControl>` 托
    /// 管，与 StopRegistry 同型）。
    pub fn begin_run(
        self: &Arc<Self>,
        root: &str,
        change: &str,
        run_id: String,
    ) -> Result<RunGuard, String> {
        let key = (root.to_owned(), change.to_owned());
        let mut runs = self.runs.lock().expect("run 注册表锁不可中毒");
        if runs.contains_key(&key) {
            return Err(format!(
                "change \"{change}\" 已有运行中的 run，不可并行发起"
            ));
        }
        let (updates, _) = broadcast::channel(UPDATE_CAPACITY);
        let (cancel, _) = watch::channel(false);
        runs.insert(
            key.clone(),
            RunEntry {
                run_id,
                status: ChangeRunStatus::Running,
                phase: None,
                attempt: None,
                ask: None,
                cancel,
                session: Mutex::new(None),
                confirm_tx: Mutex::new(None),
                answer_tx: Mutex::new(None),
                updates,
            },
        );
        drop(runs);
        Ok(RunGuard {
            key,
            control: Arc::clone(self),
        })
    }

    /// 订阅 run 状态流（`change_flow_start` / `change_flow_watch` 共用入口）；
    /// 无运行 run → `None`。
    pub fn subscribe(&self, root: &str, change: &str) -> Option<broadcast::Receiver<RunUpdate>> {
        self.runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&(root.to_owned(), change.to_owned()))
            .map(|entry| entry.updates.subscribe())
    }

    /// 置 cancel 标志（停止不必先应答停等）；miss（无运行 run）幂等返回
    /// false。`send_replace` 直写 watch 槽位——无订阅者时置位不丢（`send`
    /// 在无 receiver 时是 no-op，迟滞订阅会读不到停止信号）。
    pub fn request_stop(&self, root: &str, change: &str) -> bool {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        match runs.get(&(root.to_owned(), change.to_owned())) {
            Some(entry) => {
                entry.cancel.send_replace(true);
                true
            }
            None => false,
        }
    }

    /// 当前 WorkerAgent 会话 id 槽读取（命令层停止寻址：经既有 StopRegistry
    /// 请求终止）；无槽位 → `None`。
    pub fn current_session(&self, root: &str, change: &str) -> Option<String> {
        self.runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&(root.to_owned(), change.to_owned()))
            .and_then(|entry| entry.session.lock().expect("会话槽锁不可中毒").clone())
    }

    /// 发布一条 run 状态更新（快照面同步 + broadcast）。发布单点：步推进
    /// / 停等态迁移 / 终态收口与 WorkerAgent 会话事件透传同由此进。
    pub fn publish(&self, root: &str, change: &str, update: RunUpdate) {
        {
            let mut runs = self.runs.lock().expect("run 注册表锁不可中毒");
            if let Some(entry) = runs.get_mut(&(root.to_owned(), change.to_owned())) {
                match &update {
                    RunUpdate::Step { step } => {
                        entry.phase = Some(step.phase.clone());
                        entry.attempt = Some(step.attempt);
                    }
                    RunUpdate::Ask { question, options } => {
                        entry.status = ChangeRunStatus::WaitingAsk;
                        entry.ask = Some(AskPayload {
                            question: question.clone(),
                            options: options.clone(),
                        });
                    }
                    RunUpdate::ConfirmWait { phase } => {
                        entry.phase = Some(phase.clone());
                        entry.status = ChangeRunStatus::WaitingConfirm;
                    }
                    RunUpdate::Finished { status, .. } => entry.status = *status,
                    RunUpdate::SessionEvent { .. } => {}
                }
            }
        }
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        if let Some(entry) = runs.get(&(root.to_owned(), change.to_owned())) {
            let _ = entry.updates.send(update);
        }
    }

    /// 当前 WorkerAgent 会话 id 槽写入（命令层 sink 桥在首个会话事件到达时
    /// 同步——adapter 事件早于 turn 收口，停止寻址由此先行可见）。
    pub fn set_session(&self, root: &str, change: &str, session_id: Option<String>) {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        if let Some(entry) = runs.get(&(root.to_owned(), change.to_owned())) {
            *entry.session.lock().expect("会话槽锁不可中毒") = session_id;
        }
    }

    /// ask 应答回流：walker 以应答文本 Continue 决策会话重出封闭集；
    /// 无运行 run / 无等待方 → `Err`。
    pub fn answer(&self, root: &str, change: &str, text: String) -> Result<(), String> {
        let sender = self
            .take_pending(root, change, |entry| &entry.answer_tx)
            .ok_or_else(|| format!("change \"{change}\" 无运行中的 run"))?;
        sender
            .ok_or_else(|| "当前无等待中的 ask".to_owned())?
            .send(text)
            .map_err(|_| "ask 应答通道已关闭".to_owned())
    }

    /// phase 间停等确认：proceed=false → walker 受控终态 stopped；
    /// 无运行 run / 无等待方 → `Err`。
    pub fn confirm(&self, root: &str, change: &str, proceed: bool) -> Result<(), String> {
        let sender = self
            .take_pending(root, change, |entry| &entry.confirm_tx)
            .ok_or_else(|| format!("change \"{change}\" 无运行中的 run"))?;
        sender
            .ok_or_else(|| "当前无等待中的 phase 确认".to_owned())?
            .send(proceed)
            .map_err(|_| "确认通道已关闭".to_owned())
    }

    /// 重挂快照查询（进程内；run 终态后除名 → `None`）。
    pub fn snapshot(&self, root: &str, change: &str) -> Option<ChangeRunSnapshot> {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        runs.get(&(root.to_owned(), change.to_owned()))
            .map(|entry| ChangeRunSnapshot {
                run_id: entry.run_id.clone(),
                status: entry.status,
                phase: entry.phase.clone(),
                attempt: entry.attempt,
                ask: entry.ask.clone(),
            })
    }

    /// 单次应答通道取走（take 语义：单次应答，重复应答落空）；无运行 run
    /// 返回 `None`（与通道空置 `Some(None)` 可辨）。
    fn take_pending<T>(
        &self,
        root: &str,
        change: &str,
        slot: impl Fn(&RunEntry) -> &Mutex<Option<oneshot::Sender<T>>>,
    ) -> Option<Option<oneshot::Sender<T>>> {
        let runs = self.runs.lock().expect("run 注册表锁不可中毒");
        runs.get(&(root.to_owned(), change.to_owned()))
            .map(|entry| slot(entry).lock().expect("应答通道锁不可中毒").take())
    }
}

/// walker 持有的单 run 控制柄：emit / 当前会话槽 / 停等 / 取消观测。终态
/// 收口在 walker 主入口单点 [`RunGuard::finish`]（消费 self——终态出口唯一）。
pub struct RunGuard {
    /// 复合键（workspace root, change）
    key: (String, String),
    control: Arc<ChangeFlowControl>,
}

impl RunGuard {
    /// 广播一条 run 状态更新（无订阅者时静默——broadcast 语义），快照面
    /// 随 [`ChangeFlowControl::publish`] 同步。
    pub fn emit(&self, update: RunUpdate) {
        self.control.publish(&self.key.0, &self.key.1, update);
    }

    /// 当前 WorkerAgent 会话 id 槽写入。
    pub fn set_session(&self, session_id: Option<String>) {
        self.control
            .set_session(&self.key.0, &self.key.1, session_id);
    }

    /// phase 间停等：挂起至 `change_flow_confirm` 应答；等待期间取消信号
    /// 置位即以 proceed=false 收敛（停止不必先应答）。
    pub async fn wait_confirm(&self) -> bool {
        let (tx, rx) = oneshot::channel::<bool>();
        if !self.register_pending(|entry| &entry.confirm_tx, tx) {
            return false;
        }
        tokio::select! {
            proceed = rx => proceed.unwrap_or(false),
            _ = self.cancelled_wait() => false,
        }
    }

    /// ask 停等：挂起至 `change_flow_answer` 应答回流；取消信号置位 →
    /// `None`（walker 收敛 stopped）。
    pub async fn wait_answer(&self) -> Option<String> {
        let (tx, rx) = oneshot::channel::<String>();
        if !self.register_pending(|entry| &entry.answer_tx, tx) {
            return None;
        }
        tokio::select! {
            answer = rx => answer.ok(),
            _ = self.cancelled_wait() => None,
        }
    }

    /// 取消观测（同步）。
    pub fn cancelled(&self) -> bool {
        self.cancel_rx()
            .map(|mut rx| *rx.borrow_and_update())
            .unwrap_or(false)
    }

    /// 终态收口：`Finished` 广播（快照面随 publish 落终值）+ 除名。消费
    /// self——终态出口唯一（walker 主入口单点调用）。
    pub fn finish(self, status: ChangeRunStatus, reason: Option<String>) {
        self.emit(RunUpdate::Finished { status, reason });
        self.control
            .runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .remove(&self.key);
    }

    fn cancel_rx(&self) -> Option<watch::Receiver<bool>> {
        self.control
            .runs
            .lock()
            .expect("run 注册表锁不可中毒")
            .get(&self.key)
            .map(|entry| entry.cancel.subscribe())
    }

    /// 取消信号等待半边（watch 异步等待；除名后的迟滞等待即刻返回——调用方
    /// 随后的注册表访问自回落空路径）。
    async fn cancelled_wait(&self) {
        let Some(mut rx) = self.cancel_rx() else {
            return;
        };
        loop {
            if *rx.borrow_and_update() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// 登记单次应答发送端（注册表条目在位为前提；run 已收口 → false）。
    fn register_pending<T>(
        &self,
        slot: impl Fn(&RunEntry) -> &Mutex<Option<oneshot::Sender<T>>>,
        tx: oneshot::Sender<T>,
    ) -> bool {
        let runs = self.control.runs.lock().expect("run 注册表锁不可中毒");
        let Some(entry) = runs.get(&self.key) else {
            return false;
        };
        *slot(entry).lock().expect("应答通道锁不可中毒") = Some(tx);
        true
    }
}
