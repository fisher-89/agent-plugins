use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::broadcast;

use crate::event::AgentEvent;
use crate::port::TurnOutcome;

/// per-session broadcast 容量：按「一轮回复的 delta 突发」定价（每 token 一条，
/// 密度比 change 域通知高一量级），覆盖 4k 量级 token 的整轮突发；溢出即
/// lagged，由订阅侧以断流重订兜底（不静默丢事件后继续投递）。
const SESSION_STREAM_CAPACITY: usize = 4096;

/// 会话实时流条目（core 中性载体）：事件（含 ephemeral delta）与轮终态两变体。
/// 非 IPC 信封、不序列化——信封投影归命令层（core MUST NOT 认识 IPC 形态）。
#[derive(Debug, Clone)]
pub enum SessionStreamItem {
    /// 盖戳后的观察事件（增量与密封同源）
    Event(AgentEvent),
    /// 轮终态（末条，投递后条目除名）
    Finished(TurnOutcome),
}

/// 订阅端收尾原因（两态均由订阅端断流收尾，不投伪终态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStreamRecvError {
    /// 广播窗口溢出（携丢弃条数）：消费慢于生产，客户端据断流重放重订
    Lagged(u64),
    /// 条目已除名且缓冲排空（轮终态已投递）
    Closed,
}

/// 会话级实时流注册表（内存态，进程生命周期，应用重启即清空）：键 = core
/// session id（core 铸 id 进程内全局唯一，免复合键），与 [`crate::StopRegistry`]
/// 同型托管为壳层 `Arc` 单例。条目生命周期 = 一轮：`begin` 登记、`publish`
/// 投递、`finish` 投终态即除名。零 store 依赖（订阅面纯实时，不补发库内快照）。
#[derive(Default)]
pub struct SessionStreamRegistry {
    sessions: Mutex<HashMap<String, broadcast::Sender<SessionStreamItem>>>,
}

impl SessionStreamRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 轮登记：为该会话建一条容量 [`SESSION_STREAM_CAPACITY`] 的 broadcast。
    /// 同键重复登记直接覆盖（无重入守卫，沿 `StopRegistry` 既存形态——并发两轮
    /// 同键不在本轮变更语义内）。
    pub fn begin(&self, session_id: &str) {
        let (sender, _) = broadcast::channel(SESSION_STREAM_CAPACITY);
        self.sessions
            .lock()
            .expect("会话流注册表锁不可中毒")
            .insert(session_id.to_owned(), sender);
    }

    /// 事件投递（含 delta）：无条目 / 无订阅者静默丢弃（不产生错误）。
    pub fn publish(&self, session_id: &str, event: AgentEvent) {
        let sessions = self.sessions.lock().expect("会话流注册表锁不可中毒");
        if let Some(sender) = sessions.get(session_id) {
            let _ = sender.send(SessionStreamItem::Event(event));
        }
    }

    /// 投终态 [`SessionStreamItem::Finished`] 后除名（条目移除；既有订阅者排空
    /// 缓冲后收到 `Closed`）。
    pub fn finish(&self, session_id: &str, outcome: TurnOutcome) {
        let mut sessions = self.sessions.lock().expect("会话流注册表锁不可中毒");
        if let Some(sender) = sessions.get(session_id) {
            let _ = sender.send(SessionStreamItem::Finished(outcome));
        }
        sessions.remove(session_id);
    }

    /// 订阅句柄：无运行轮（无条目）返回 `None`——「幂等立即结束」的判据。
    pub fn subscribe(
        &self,
        session_id: &str,
        since_seq: u64,
    ) -> Option<SessionStreamSubscription> {
        let sessions = self.sessions.lock().expect("会话流注册表锁不可中毒");
        sessions.get(session_id).map(|sender| SessionStreamSubscription {
            receiver: sender.subscribe(),
            since_seq,
        })
    }
}

/// 订阅句柄：`since_seq` 过滤与 lagged 判别在此内聚（core 单点，命令层零过滤）。
pub struct SessionStreamSubscription {
    receiver: broadcast::Receiver<SessionStreamItem>,
    /// 已观测事件的最大 seq 上界：只返严格更大者（终态恒返）
    since_seq: u64,
}

impl SessionStreamSubscription {
    /// 取下一条合格条目：`Event` 仅当 `seq > since_seq`（不合格条目丢弃后继续
    /// 等待），`Finished` 恒返；`Lagged` / `Closed` 显式区分。
    pub async fn recv(&mut self) -> Result<SessionStreamItem, SessionStreamRecvError> {
        loop {
            match self.receiver.recv().await {
                Ok(SessionStreamItem::Event(event)) => {
                    if event.seq > self.since_seq {
                        return Ok(SessionStreamItem::Event(event));
                    }
                }
                Ok(item @ SessionStreamItem::Finished(_)) => return Ok(item),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    return Err(SessionStreamRecvError::Lagged(skipped));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(SessionStreamRecvError::Closed);
                }
            }
        }
    }
}
