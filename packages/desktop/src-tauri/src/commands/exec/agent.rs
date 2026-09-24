//! `run_agent()` 编排函数（app 层微形态，与 `commands::workspaces` 的
//! `*_inner` 同列）：组装 runner → 事件流 tee 双 sink（store 逐事件落库 +
//! Channel 实时流）→ 状态机收敛 → 终态落库。
//!
//! 命令体保持三件事纪律（组装不内联回命令体）；[`run_agent_with`] 泛型缝
//! 让编排路径可被假 runner 测试。本文件独立于 mod.rs，将来抽 app crate 时
//! 单文件平移复用、不重写。
//!
//! 来源归属与链指针（`RunProvenance`）是 store 记录面元数据，经编排填充进
//! run 记录初值，不进 [`AgentRunner`] 契约（trait 面只认逻辑运行参数，见
//! agent 契约 crate 文档）；`resume_session_id` 改变 CLI 行为，已在
//! `AgentRunParams` 契约内。
//!
//! tee 循环节奏（背压策略）：`recv → 状态机 apply → store 逐事件单事务追加
//! → Channel 发送`。Channel 发送失败（页面已关闭）不中断落库；store 写入
//! 失败立即收敛 run 为 failed（error 记因）并终止 tee——落库是兜底路径，
//! 失败不可静默。

use tauri::ipc::Channel;

use agent::{
    AgentEvent, AgentEventKind, AgentRunParams, AgentRunState, AgentRunner, RunStateMachine,
};
use agent_cli::ClaudeCliRunner;
use store::{AgentRunRecord, Store};

/// run 状态受控字符串（store 不引本地枚举，见 store 模型文档）。
const STATUS_RUNNING: &str = "running";
/// run 状态受控字符串：正常收敛。
const STATUS_COMPLETED: &str = "completed";
/// run 状态受控字符串：失败收敛。
const STATUS_FAILED: &str = "failed";

/// run 记录来源与链字段（app 层微形态，store 记录面元数据）：`source` 来源
/// 受控字符串、`source_ref` 来源内定位、`parent_run_id` 链上游 run。编排
/// 填充点单点——`running` 记录初值即携带，终态替换不改写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunProvenance {
    /// 来源受控字符串（debug | explore | …）
    pub source: String,
    /// 来源内定位（explore 指向探索记录主键的十进制串）
    pub source_ref: Option<String>,
    /// 链上游 run id（链首为 None）
    pub parent_run_id: Option<i64>,
}

impl RunProvenance {
    /// 调试链路缺省来源（不携带定位与链指针，与演进前写入语义一致）。
    pub(crate) fn debug() -> Self {
        Self {
            source: "debug".to_owned(),
            source_ref: None,
            parent_run_id: None,
        }
    }
}

/// `RunResult` 汇总字段摘取（终态记录填充用；is_error 由状态机承载，不在此重复）。
struct RunSummary {
    num_turns: Option<u64>,
    cost_usd: Option<f64>,
    duration_ms: Option<u64>,
    session_id: Option<String>,
}

/// UTC unix 毫秒：std 唯一时间源（时钟早于 epoch 时取 0，不 panic）。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 薄入口：组装真实 CLI runner 后委托 [`run_agent_with`]。
pub(crate) async fn run_agent(
    store: &Store,
    on_event: Channel<AgentEvent>,
    params: AgentRunParams,
    provenance: RunProvenance,
) -> Result<AgentRunRecord, String> {
    let runner = ClaudeCliRunner::new();
    run_agent_with(store, &runner, on_event, params, provenance).await
}

/// 泛型编排：runner 启动（启动阶段失败 → `Err`，不留 run 行）→ begin 落
/// `running` 行（初值携带 provenance 来源与链字段）→ tee 双 sink 循环 →
/// EOF 按状态机收敛终态并落库 → 返回最终记录（in-band 失败返回
/// `Ok(failed 记录)`，仅启动阶段失败返回 `Err`）。
pub(crate) async fn run_agent_with<R: AgentRunner>(
    store: &Store,
    runner: &R,
    on_event: Channel<AgentEvent>,
    params: AgentRunParams,
    provenance: RunProvenance,
) -> Result<AgentRunRecord, String> {
    let running = AgentRunRecord {
        id: 0,
        prompt: params.prompt.clone(),
        cwd: params.cwd.to_string_lossy().into_owned(),
        env: params.env.as_str().to_owned(),
        permission_mode: params.permission_mode.as_str().to_owned(),
        status: STATUS_RUNNING.to_owned(),
        started_at: now_millis(),
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
        source: provenance.source,
        source_ref: provenance.source_ref,
        parent_run_id: provenance.parent_run_id,
    };
    let mut run = runner.start(params).map_err(|e| e.to_string())?;
    let mut record = store.begin_agent_run(&running).map_err(|e| e.to_string())?;
    let mut machine = RunStateMachine::new();
    let mut summary: Option<RunSummary> = None;

    while let Some(event) = run.events.recv().await {
        machine.apply(&event);
        if let AgentEventKind::RunResult {
            num_turns,
            cost_usd,
            duration_ms,
            session_id,
            ..
        } = &event.kind
        {
            summary = Some(RunSummary {
                num_turns: *num_turns,
                cost_usd: *cost_usd,
                duration_ms: *duration_ms,
                session_id: session_id.clone(),
            });
        }
        // store sink（兜底路径）：类型化事件直写；失败立即收敛 failed 并终止 tee
        if let Err(store_error) =
            store.append_agent_run_events(record.id, std::slice::from_ref(&event))
        {
            return Ok(abort_with_store_failure(
                store,
                record,
                format!("事件落库失败: {store_error}"),
            ));
        }
        // Channel sink（实时流）：发送失败（页面已关闭）不中断落库
        let _ = on_event.send(event);
    }

    // EOF：按状态机收敛终态（收敛恒由 RunResult 驱动；泵异常终止无 result
    // 时不留 running 终态，按 failed 记因收敛）
    record.status = match machine.current() {
        AgentRunState::Completed => STATUS_COMPLETED.to_owned(),
        AgentRunState::Failed => STATUS_FAILED.to_owned(),
        AgentRunState::Running => {
            record.error = Some("进程结束但未产出 result 事件".to_owned());
            STATUS_FAILED.to_owned()
        }
    };
    if let Some(result) = summary {
        record.num_turns = result.num_turns;
        record.cost_usd = result.cost_usd;
        record.duration_ms = result.duration_ms;
        record.session_id = result.session_id;
    }
    record.finished_at = Some(now_millis());
    store
        .finish_agent_run(record.id, &record)
        .map_err(|e| e.to_string())?;
    Ok(record)
}

/// store 写失败的失败收敛：run 收敛为 failed、error 记因，并尽力落终态行
/// （终态落库也失败时，错误串并入 error 字段保留，不再向上传播）。
fn abort_with_store_failure(
    store: &Store,
    mut record: AgentRunRecord,
    cause: String,
) -> AgentRunRecord {
    record.status = STATUS_FAILED.to_owned();
    record.error = Some(cause);
    record.finished_at = Some(now_millis());
    if let Err(store_error) = store.finish_agent_run(record.id, &record) {
        record.error = Some(format!(
            "{}（终态落库亦失败: {store_error}）",
            record.error.clone().unwrap_or_default()
        ));
    }
    record
}

#[cfg(test)]
#[path = "agent_test.rs"]
pub(crate) mod agent_test;
