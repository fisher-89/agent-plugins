//! 决策会话槽位挂账写操作（amend 语义）：定位该相位最新 eval 条目（与
//! [`backtrack`](super::backtrack) 同锚定——`latest_entry_index` +
//! `entry_phase`），raw 定点改写 `decision_session_id` 键，幂等覆写（同值
//! 重挂条目字节不变），无条目显式 `Err`（不做表位校验——不新增条目、不触
//! 相位路由语义）。槽位值由调用方透传，本写面不解释不改写；decision 会话
//! 产生于 fail 条目落账之后，故走本显式挂账操作而非 `phase_log` 随行。

use serde_json::Value;

use crate::write::persist::{entry_phase, eval_entries_mut, latest_entry_index, load_doc, save};
use foundation::layout::Layout;

/// 挂账产出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionLogOutcome {
    pub phase: String,
}

/// 决策会话槽位挂账：该相位最新 eval 条目定点改写 `decision_session_id`
///（幂等覆写，无条目显式 `Err`），保形写回。
pub fn decision_log(
    layout: &Layout,
    change: &str,
    phase: &str,
    session_id: &str,
) -> Result<DecisionLogOutcome, String> {
    let mut doc = load_doc(layout, change)?;
    let entries = eval_entries_mut(&mut doc.raw);
    let latest = latest_entry_index(entries, |entry| entry_phase(entry) == Some(phase))
        .ok_or_else(|| {
            format!(
                "Phase \"{phase}\" 没有评估条目，无法挂账决策会话。请先执行该 phase 并记录评估结果。"
            )
        })?;
    if let Some(object) = entries[latest].as_object_mut() {
        object.insert(
            "decision_session_id".to_owned(),
            Value::String(session_id.to_owned()),
        );
    }
    save(&doc)?;
    Ok(DecisionLogOutcome {
        phase: phase.to_owned(),
    })
}
