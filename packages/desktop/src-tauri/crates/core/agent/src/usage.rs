use serde_json::Value;

use crate::event::{AgentEvent, AgentEventKind};

/// token 求和单源：session 聚合面（会话统计）与编排步级 token 消耗同一实现。
/// 上移自 infra/store（desktop-step-running-snapshot D3）——依赖方向 infra →
/// core 不可反向，求和落 [`AgentEventKind::TurnDone`] 定义所在 crate；infra/store
/// 与 orchestration 两消费方同源引用。
///
/// 转录 `TurnDone.usage` 求和：鸭子类型键取值（`"input_tokens"` /
/// `"output_tokens"`），全缺席 → None（缺席合法缺省）、部分在场饱和求和。
pub fn sum_usage_tokens(events: &[AgentEvent], key: &str) -> Option<u64> {
    let mut total: u64 = 0;
    let mut any = false;
    for event in events {
        if let AgentEventKind::TurnDone { usage, .. } = &event.kind {
            if let Some(value) = usage.get(key).and_then(Value::as_u64) {
                total = total.saturating_add(value);
                any = true;
            }
        }
    }
    any.then_some(total)
}
