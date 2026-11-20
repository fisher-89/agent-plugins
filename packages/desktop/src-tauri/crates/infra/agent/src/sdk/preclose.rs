//! 收口前的架构判据自检（引擎兜底读数）。
//!
//! 唯一调用点 = `loop` 的自然收敛支（**先于** `finish_success`）。三条硬约束：
//! 不回灌模型、不催轮次、不发新请求——本模块只发一条 `SystemNotice` 并写引擎日志。
//!
//! 触发 = T1（本轮写过架构域）∪ T2（架构资产哈希变了）；皆为假只在引擎日志留一行
//! skip（纯问答不产生转录噪声）。fail-open 三面：`CheckError` / `JoinError` 一律发
//! `archi_preclose_skipped`（带成因）后照常收口，MUST NOT 阻断、MUST NOT 改
//! `TurnDone`。
//!
//! 会话判据是 `preamble` 位（该位唯一生产者就是 archi 臂），MUST NOT 用
//! `turn.archi`（change 相位也下发 archi 工具，会误命中）。

use std::path::Path;

use agent::AgentEventKind;
use serde_json::Value;

use crate::sdk::archi_drift_view::drift_summary;
use crate::sdk::log::append_engine_log;
use crate::sdk::r#loop::LoopTurn;

/// 自检命中时的 notice 子类。
const ARCHI_PRECLOSE: &str = "archi_preclose";

/// 自检被跳过（失败面）时的 notice 子类。
const ARCHI_PRECLOSE_SKIPPED: &str = "archi_preclose_skipped";

/// 起手基线：非 archi 会话（`preamble` 缺席）直接 `None`（零 IO）；否则在阻塞池里
/// 求本轮的资产哈希起点值（T2 需要「本轮起点 vs 收口时」两侧）。
pub(crate) async fn baseline(turn: &LoopTurn) -> Option<String> {
    // 会话判据 = `preamble` 位（该位唯一生产者是 archi 臂；`turn.archi` 不可用——
    // change 相位也下发 archi 工具）
    let archi_session = turn.injections.preamble.is_some();
    if !archi_session {
        return None;
    }
    let cwd = turn.cwd.clone();
    tokio::task::spawn_blocking(move || archi::drift::assets_hash(&cwd))
        .await
        .ok()
}

/// 本次工具调用是否写了架构域（`write` / `edit` 取沙箱后的 `path`、`bash` 取
/// [`crate::sdk::protect::written_paths`] 的写面提取结果）。
///
/// 前缀由 `foundation::layout` 派生（本源码零目录名字面量）；`write` / `edit` 的入参
/// 是沙箱改写后的绝对路径，比对前按 `/` 归一后转 repo 相对。判据假阴面（盘符 / 大小写
/// 导致的 `cwd` 前缀失配、inline / 重定向之外的写面）一律由 T2 兜底。
pub(crate) fn touched(name: &str, input: &Value, cwd: &Path) -> bool {
    let prefixes = [
        format!("{}/", foundation::layout::archi_graph_rel_dir()),
        format!("{}/", foundation::layout::archi_models_rel_dir()),
    ];
    match name {
        "write" | "edit" => input
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| hits_prefix(path, cwd, &prefixes)),
        "bash" => input
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| {
                crate::sdk::protect::written_paths(command, cwd)
                    .iter()
                    .any(|path| hits_prefix(path, cwd, &prefixes))
            }),
        _ => false,
    }
}

/// 绝对（或已是 repo 相对）路径是否落在两个架构子目录之一内。
fn hits_prefix(path: &str, cwd: &Path, prefixes: &[String; 2]) -> bool {
    let normalized = path.replace('\\', "/");
    let root = cwd.to_string_lossy().replace('\\', "/");
    let relative = normalized
        .strip_prefix(&format!("{}/", root.trim_end_matches('/')))
        .unwrap_or(&normalized)
        .trim_start_matches("./");
    prefixes.iter().any(|prefix| relative.starts_with(prefix))
}

/// 收口自检执行体：判触发 → 全量判定 → notice / 引擎日志。三臂一律不抛错。
pub(crate) async fn run(
    sender: &tokio::sync::mpsc::Sender<AgentEventKind>,
    turn: &LoopTurn,
    touched: bool,
    baseline: Option<&str>,
) {
    // `preamble` 缺席已由 `baseline` 表达（非 archi 会话零 notice、零日志）
    let Some(before) = baseline else {
        return;
    };
    let changed_assets = archi::drift::assets_hash(&turn.cwd) != before;
    if !touched && !changed_assets {
        append_engine_log(&format!(
            "loop 收口自检跳过（T1/T2 皆假）session={}",
            turn.session_id
        ));
        return;
    }
    let cwd = turn.cwd.clone();
    let started = std::time::Instant::now();
    let outcome = tokio::task::spawn_blocking(move || {
        archi::drift::check(archi::drift::CheckInput {
            root: &cwd,
            mode: archi::drift::ArchiDriftMode::Full,
            changed: &[],
            change: None,
            cancel: foundation::cancel::CancelFlag::never(),
        })
    })
    .await;
    let duration_ms = started.elapsed().as_millis() as u64;
    let (subtype, payload) = match outcome {
        Ok(Ok(report)) => {
            let mut payload = drift_summary(&report, duration_ms);
            if let Value::Object(object) = &mut payload {
                object.insert("changedAssets".to_owned(), Value::Bool(changed_assets));
            }
            append_engine_log(&format!(
                "loop 收口自检 session={} status={} durationMs={duration_ms}",
                turn.session_id,
                report.status.as_str()
            ));
            (ARCHI_PRECLOSE, payload)
        }
        Ok(Err(error)) => {
            append_engine_log(&format!(
                "loop 收口自检失败 session={} reason={}",
                turn.session_id,
                error.message()
            ));
            (
                ARCHI_PRECLOSE_SKIPPED,
                serde_json::json!({ "reason": error.message() }),
            )
        }
        Err(join_error) => {
            append_engine_log(&format!(
                "loop 收口自检任务失败 session={} reason={join_error}",
                turn.session_id
            ));
            (
                ARCHI_PRECLOSE_SKIPPED,
                serde_json::json!({ "reason": format!("自检任务失败: {join_error}") }),
            )
        }
    };
    let _ = sender
        .send(AgentEventKind::SystemNotice {
            subtype: subtype.to_owned(),
            payload,
        })
        .await;
}
