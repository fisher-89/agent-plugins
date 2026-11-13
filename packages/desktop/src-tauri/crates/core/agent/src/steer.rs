use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::{mpsc, oneshot};

/// 补充说明载荷：`text` 为原样文本，`ack` 是「已入本轮史」的逐条确认。
/// 每条消息自带一个 ack，是「不合并」在类型上的载体——三条在途 = 三个 ack。
#[derive(Debug)]
pub struct SteerMessage {
    pub text: String,
    pub ack: oneshot::Sender<()>,
}

/// steer 注入注册表（内存态，进程生命周期，应用重启即清空）：会话级单向
/// 通道，键 = core 铸会话 id（与 [`crate::StopRegistry`] / [`crate::AskRegistry`]
/// 同址同键空间）。
///
/// **零编排域类型**：不认识 workspace root 与 change——两段寻址由壳层翻译为
/// 会话 id 后投递；编排状态广播不在本表内。
#[derive(Default)]
pub struct SteerRegistry {
    slots: Mutex<HashMap<String, mpsc::UnboundedSender<SteerMessage>>>,
}

impl SteerRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记本轮补充说明接收入端（内核开轮时、`streams.begin` 之后调用）。
    /// **同键覆盖**即上一轮的接收入端已 drop（其残留项随 drop 收 `Err`）。
    pub fn register(&self, session_id: &str, tx: mpsc::UnboundedSender<SteerMessage>) {
        self.slots
            .lock()
            .expect("steer 注册表锁不可中毒")
            .insert(session_id.to_owned(), tx);
    }

    /// 投递一条补充说明。miss（无在跑轮 / 已收口）与 `send` 失败（接收入端
    /// 已 drop）归同一臂同一文案：从调用方视角两者都是「该会话当前没有在跑的
    /// 轮」。
    ///
    /// **不是** `AskRegistry::answer` 的受理语义——steer 是送达语义（自带逐条
    /// ack），静默 `Ok` 会让用户以为消息已入史。
    pub fn request(&self, session_id: &str, msg: SteerMessage) -> Result<(), String> {
        let slots = self.slots.lock().expect("steer 注册表锁不可中毒");
        let tx = slots
            .get(session_id)
            .ok_or_else(|| "会话已收口，补充说明不可送达".to_owned())?;
        // 接收端已 drop（会话在轮内被抛弃）：送达失败不可静默——与 miss 同臂
        tx.send(msg)
            .map_err(|_| "会话已收口，补充说明不可送达".to_owned())
    }

    /// 除名（轮收敛时兜底清理；miss 幂等）。
    pub fn remove(&self, session_id: &str) {
        self.slots
            .lock()
            .expect("steer 注册表锁不可中毒")
            .remove(session_id);
    }
}
