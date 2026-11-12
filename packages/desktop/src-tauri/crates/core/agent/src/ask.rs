use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::oneshot;

/// ask 应答注册表（内存态，进程生命周期，应用重启即清空）：会话级阻塞应答
/// 通道，键 = core 铸会话 id（进程内唯一，与 [`crate::StopRegistry`] 同型）。
///
/// **零编排域类型**：不认识 workspace root 与 change——流程侧的两段寻址由壳层
/// 翻译为会话 id 后投递；编排状态广播不在本表内（见 `AskPort`）。
#[derive(Default)]
pub struct AskRegistry {
    slots: Mutex<HashMap<String, oneshot::Sender<String>>>,
}

impl AskRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记等待方（ask 执行体进入阻塞前调用）。**恒写入**：同键覆盖即上一问的
    /// 等待方已被放弃（旧 `Sender` 随覆盖 drop，旧等待方收 `RecvError`）。
    ///
    /// 返回值是**诊断值**，不是失败面：`true` = 登记时该键原本为空（无等待方
    /// 被顶替）；`false` = 顶替了上一问的等待方（模型两次调用之间用户未答，
    /// 属正常路径）。
    pub fn register(&self, session_id: &str, tx: oneshot::Sender<String>) -> bool {
        self.slots
            .lock()
            .expect("ask 注册表锁不可中毒")
            .insert(session_id.to_owned(), tx)
            .is_none()
    }

    /// 投递应答（take 语义：单次投递，先到者胜）。无等待方（未登记 / 已应答 /
    /// 轮已收敛除名）→ `Err`，不产生第二次回灌。
    pub fn answer(&self, session_id: &str, text: String) -> Result<(), String> {
        let sender = self
            .slots
            .lock()
            .expect("ask 注册表锁不可中毒")
            .remove(session_id)
            .ok_or_else(|| "当前无等待中的 ask".to_owned())?;
        // 槽已被 take：等待方已 drop（会话终止）时 send 失败仍按受理返回——
        // 幂等语义由「槽已移除」承载，不因对端消失改写受理结论
        let _ = sender.send(text);
        Ok(())
    }

    /// 除名（轮收敛时兜底清理；miss 幂等）。
    pub fn remove(&self, session_id: &str) {
        self.slots
            .lock()
            .expect("ask 注册表锁不可中毒")
            .remove(session_id);
    }
}
