use std::path::Path;

use agent::AgentEventKind;
use futures::future::BoxFuture;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::sdk::r#loop::LoopTurn;

/// 收口前钩子：单 turn 一个实例（runner per-turn 铸造，状态不跨轮复用）。
pub(crate) trait BeforeDoneHook: Send + Sync {
    /// loop 起手（首请求前一次）：建立观测起点。
    fn start<'a>(&'a self, turn: &'a LoopTurn) -> BoxFuture<'a, ()>;

    /// 工具调用观察（沙箱 ∧ 保护闸通过后、执行前；零额外 IO）。
    fn observe_tool(&self, name: &str, input: &Value, cwd: &Path);

    /// 自然收敛前一次：只发 notice / 日志，不追加请求、不改史、不催轮次。
    fn before_done<'a>(
        &'a self,
        sender: &'a mpsc::Sender<AgentEventKind>,
        turn: &'a LoopTurn,
    ) -> BoxFuture<'a, ()>;
}
