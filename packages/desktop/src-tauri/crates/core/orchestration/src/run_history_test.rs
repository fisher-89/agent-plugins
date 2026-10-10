//! `run_history.rs` 的单元测试（test-design「run_history.rs ->
//! run_history_test.rs」节）：`persisted_step` 10→5 步词汇过滤单点（五落 / 五
//! 弃 / 全集逐一钉死——词汇漂移即败，AC-3）、`finish_command` 组装（全词汇
//! emit 序盖戳产生库内空洞、终态三值映射、session_id / detail 两态透传、空步
//! 序空包、载荷 `change_id` 透传）、`StoreRunHistory` 适配器真件委派
//! （run_started 落 running 行 / run_finished 整包落库回读逐字段一致 +
//! Err(String) 串语义透传记因保留）。
//!
//! Mock策略（test-design 本节 Mock 表）：无（真实组合）——纯函数直测 +
//! tempfile 真件库（沿 walker_test TestDb 装置）承载 StoreRunHistory 委派。

use std::sync::Arc;

use store::Store;

use crate::port::RunHistoryPort;
use crate::run_history::{finish_command, persisted_step, StoreRunHistory};
use crate::state::{ChangeRunStatus, ChangeStepKind, ChangeStepState, ChangeStepStatus};
use crate::walker::RunRequest;
use workflow::state::{
    ChangeStateRecord, ChangeStatus, RunFinishCommand, RunStartCommand, RunStatus,
};

/// 固定 change **id** 字面量（`RunRequest.change_id` 与全部载荷身份段的转发
/// 源——id 归键 / 寻址入参）。
const CHANGE_ID: &str = "b7e2d410-9c58-4a3f-8e16-2f4a6c8b0d31";

/// 展示名（建档记录属性；id ≠ name 形态下与寻址键不混同）。
const NAME: &str = "history-change";

const RUN_ID: &str = "run-1727000000000";
const STARTED_AT: i64 = 1_727_000_000_000;

fn request() -> RunRequest {
    RunRequest {
        root: "/ws/root".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        run_id: RUN_ID.to_owned(),
        auto_next_phase: true,
        started_at: STARTED_AT,
    }
}

fn step(
    kind: ChangeStepKind,
    status: ChangeStepStatus,
    session_id: Option<&str>,
) -> ChangeStepState {
    ChangeStepState {
        phase: "implement".to_owned(),
        attempt: 1,
        step: kind,
        status,
        session_id: session_id.map(str::to_owned),
        detail: None,
    }
}

// ---------------------------------------------------------------------------
// persisted_step 单点：10→5 过滤（词汇漂移即败）
// ---------------------------------------------------------------------------

/// 五落词汇逐一映射到对应 RunStepKind（Some 等值，AC-3 落库半边）。
#[test]
fn persisted_step五落词汇逐一some等值() {
    assert_eq!(
        persisted_step(ChangeStepKind::Executor),
        Some(workflow::state::RunStepKind::Executor)
    );
    assert_eq!(
        persisted_step(ChangeStepKind::Evaluator),
        Some(workflow::state::RunStepKind::Evaluator)
    );
    assert_eq!(
        persisted_step(ChangeStepKind::Decision),
        Some(workflow::state::RunStepKind::Decision)
    );
    assert_eq!(
        persisted_step(ChangeStepKind::StaticCheck),
        Some(workflow::state::RunStepKind::StaticCheck)
    );
    assert_eq!(
        persisted_step(ChangeStepKind::TestExecution),
        Some(workflow::state::RunStepKind::TestExecution)
    );
}

/// 五弃词汇（PhaseStart / PhaseLog / VerdictGate / RetryGate / WhitelistGate）
/// 逐一 None（AC-3 忽略集——流程面步骤与三门不落库）。
#[test]
fn persisted_step五弃词汇逐一none() {
    assert_eq!(persisted_step(ChangeStepKind::PhaseStart), None);
    assert_eq!(persisted_step(ChangeStepKind::PhaseLog), None);
    assert_eq!(persisted_step(ChangeStepKind::VerdictGate), None);
    assert_eq!(persisted_step(ChangeStepKind::RetryGate), None);
    assert_eq!(persisted_step(ChangeStepKind::WhitelistGate), None);
}

/// 十词汇全集逐一断言恰五 Some 五 None（映射表全量钉死——新增 / 删除词汇即
/// 败，AC-3 单点纪律的结构性防线）。
#[test]
fn persisted_step十词汇全集恰五some五none() {
    let all = [
        ChangeStepKind::Executor,
        ChangeStepKind::Evaluator,
        ChangeStepKind::Decision,
        ChangeStepKind::PhaseStart,
        ChangeStepKind::StaticCheck,
        ChangeStepKind::TestExecution,
        ChangeStepKind::PhaseLog,
        ChangeStepKind::VerdictGate,
        ChangeStepKind::RetryGate,
        ChangeStepKind::WhitelistGate,
    ];
    let mapped: Vec<_> = all.iter().map(|kind| persisted_step(*kind)).collect();
    assert_eq!(
        mapped.iter().filter(|entry| entry.is_some()).count(),
        5,
        "恰五落（agent 三角色 + 脚本两步）"
    );
    assert_eq!(
        mapped.iter().filter(|entry| entry.is_none()).count(),
        5,
        "恰五弃（相位机两步 + 三门）"
    );
}

// ---------------------------------------------------------------------------
// finish_command 组装：emit 序盖戳空洞 / 透传 / 空包
// ---------------------------------------------------------------------------

/// 全词汇 10 步 emit 序列（含被过滤步穿插）输入 → RunFinishCommand.steps 恰
/// 5 条且 seq 保留全词汇 emit 序产生空洞（0,1,3,6,9 形态）、session_id /
/// detail 透传（AC-2/AC-3/AC-10 seq = emit 序）；timestamp = finished_at 同刻
/// 语义由落库侧承载（RunStepRecord::new 随整包统一，真件回环节断言）。
#[test]
fn finish_command全词汇十步恰五落_emit序空洞() {
    // 10 步全词汇 emit 序：executor(0) phaseStart(1) staticCheck(2) 穿插三门
    let steps = vec![
        step(
            ChangeStepKind::Executor,
            ChangeStepStatus::Passed,
            Some("ses-exec"),
        ),
        step(ChangeStepKind::PhaseStart, ChangeStepStatus::Passed, None),
        step(ChangeStepKind::StaticCheck, ChangeStepStatus::Passed, None),
        step(
            ChangeStepKind::Evaluator,
            ChangeStepStatus::Failed,
            Some("ses-eval"),
        ),
        step(ChangeStepKind::VerdictGate, ChangeStepStatus::Passed, None),
        step(ChangeStepKind::PhaseLog, ChangeStepStatus::Passed, None),
        step(
            ChangeStepKind::Decision,
            ChangeStepStatus::Passed,
            Some("ses-decision"),
        ),
        step(ChangeStepKind::RetryGate, ChangeStepStatus::Passed, None),
        step(
            ChangeStepKind::WhitelistGate,
            ChangeStepStatus::Failed,
            None,
        ),
        step(
            ChangeStepKind::TestExecution,
            ChangeStepStatus::Passed,
            None,
        ),
    ];

    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        Some("All phases have passed.".to_owned()),
        1_727_000_060_000,
        &steps,
    );

    assert_eq!(command.steps.len(), 5, "恰五落（10 词汇序列过滤后）");
    let seqs: Vec<u64> = command.steps.iter().map(|entry| entry.seq).collect();
    assert_eq!(
        seqs,
        vec![0, 2, 3, 6, 9],
        "seq = 全词汇 emit 序盖戳（被过滤步占号产生库内空洞，合法形态）"
    );
    // 词汇与状态逐条对应（两透传面）
    assert_eq!(
        command.steps[0].step,
        workflow::state::RunStepKind::Executor
    );
    assert_eq!(
        command.steps[0].session_id.as_deref(),
        Some("ses-exec"),
        "session_id 透传"
    );
    assert_eq!(
        command.steps[1].step,
        workflow::state::RunStepKind::StaticCheck
    );
    assert_eq!(
        command.steps[2].step,
        workflow::state::RunStepKind::Evaluator
    );
    assert_eq!(
        command.steps[2].status,
        workflow::state::RunStepStatus::Failed,
        "状态四值同形映射"
    );
    assert_eq!(
        command.steps[3].step,
        workflow::state::RunStepKind::Decision
    );
    assert_eq!(
        command.steps[4].step,
        workflow::state::RunStepKind::TestExecution
    );
    assert_eq!(command.finished_at, 1_727_000_060_000, "收口时刻命令携带");
}

/// run_id / change_id / status / reason 自 RunRequest 与收口参数透传等值；
/// 终态三值同名词汇映射（停等两态不可达——防御性收敛 Failed 的形态注记）。
#[test]
fn finish_command身份与终态透传等值() {
    for (status, expect) in [
        (ChangeRunStatus::Completed, RunStatus::Completed),
        (ChangeRunStatus::Stopped, RunStatus::Stopped),
        (ChangeRunStatus::Failed, RunStatus::Failed),
    ] {
        let command = finish_command(
            &request(),
            status,
            Some("收口记因".to_owned()),
            STARTED_AT + 1_000,
            &[],
        );
        assert_eq!(command.run_id, RUN_ID);
        assert_eq!(command.change_id, CHANGE_ID, "载荷 change_id 透传等值");
        assert_eq!(command.status, expect, "终态三值同名词汇映射");
        assert_eq!(command.reason.as_deref(), Some("收口记因"));
        assert_eq!(command.finished_at, STARTED_AT + 1_000);
    }
}

/// 空步序列 → steps 空包；session_id / detail None 与 Some 两态透传（AC-11
/// 两态样本语义前置）。
#[test]
fn finish_command空步序与两态透传() {
    let empty = finish_command(&request(), ChangeRunStatus::Stopped, None, STARTED_AT, &[]);
    assert!(
        empty.steps.is_empty(),
        "空步序列 → 空包（零步 run 收口合法）"
    );
    assert_eq!(empty.reason, None, "reason None 透传");

    let none_step = step(ChangeStepKind::StaticCheck, ChangeStepStatus::Passed, None);
    let some_step = ChangeStepState {
        session_id: Some("ses-9".to_owned()),
        detail: Some("静态检查通过".to_owned()),
        ..none_step.clone()
    };
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &[none_step, some_step],
    );
    assert_eq!(command.steps.len(), 2);
    assert_eq!(command.steps[0].session_id, None, "None 两态透传");
    assert_eq!(command.steps[0].detail, None);
    assert_eq!(command.steps[1].session_id.as_deref(), Some("ses-9"));
    assert_eq!(command.steps[1].detail.as_deref(), Some("静态检查通过"));
}

// ---------------------------------------------------------------------------
// StoreRunHistory 委派：真件库回环 + Err(String) 串语义
// ---------------------------------------------------------------------------

/// 真件 workspace 库装置（tempfile RAII；沿 walker_test TestDb 先例——字段序
/// 先关库再删目录）。
struct TestDb {
    store: Arc<Store>,
    _dir: tempfile::TempDir,
}

impl TestDb {
    fn open(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("orchestration-run-history-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let store =
            Store::open_workspace(&dir.path().join("ws.redb")).expect("打开 workspace 库应成功");
        store
            .create_change_record(ChangeStateRecord {
                id: CHANGE_ID.to_owned(),
                name: NAME.to_owned(),
                title: NAME.to_owned(),
                workflow_type: "requirement".to_owned(),
                created_at: STARTED_AT,
                status: ChangeStatus::Active,
                archived_at: None,
                active_phase: None,
                worktree: None,
                base_commit: None,
            })
            .expect("建档种子应成功");
        Self {
            store: Arc::new(store),
            _dir: dir,
        }
    }
}

/// 真件组合：run_started 落 running 行、run_finished 整包落库——`list_runs` /
/// `list_run_steps` 回读与命令逐字段一致（AC-1 链路半边 / AC-2 第二写实装）。
#[test]
fn store_run_history委派真件回环_逐字段一致() {
    let db = TestDb::open("delegate");
    let history =
        StoreRunHistory::new(Arc::clone(&db.store) as Arc<dyn workflow::state::ChangeStateStore>);

    history
        .run_started(&RunStartCommand {
            run_id: RUN_ID.to_owned(),
            change_id: CHANGE_ID.to_owned(),
            started_at: STARTED_AT,
        })
        .expect("run_started 应成功");

    let command = finish_command(
        &request(),
        ChangeRunStatus::Failed,
        Some("verdict 解析失败".to_owned()),
        STARTED_AT + 5_000,
        &[
            step(
                ChangeStepKind::Executor,
                ChangeStepStatus::Passed,
                Some("ses-1"),
            ),
            step(
                ChangeStepKind::Evaluator,
                ChangeStepStatus::Failed,
                Some("ses-2"),
            ),
        ],
    );
    history.run_finished(&command).expect("run_finished 应成功");

    // 回读与命令逐字段一致（经写面校验 → store 单事务落库全链）
    let runs = db
        .store
        .list_change_runs(CHANGE_ID)
        .expect("list_change_runs 应成功");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_id, command.run_id);
    assert_eq!(runs[0].status, RunStatus::Failed);
    assert_eq!(runs[0].reason.as_deref(), Some("verdict 解析失败"));
    assert_eq!(runs[0].started_at, STARTED_AT);
    assert_eq!(runs[0].finished_at, Some(STARTED_AT + 5_000));

    let steps = db
        .store
        .list_run_steps(RUN_ID)
        .expect("list_run_steps 应成功");
    assert_eq!(steps.len(), 2, "步整包两行");
    assert_eq!(steps[0].seq, 0);
    assert_eq!(steps[0].session_id.as_deref(), Some("ses-1"));
    assert_eq!(steps[1].seq, 1);
    assert_eq!(
        steps[1].status,
        workflow::state::RunStepStatus::Failed,
        "状态透传"
    );
    for row in &steps {
        assert_eq!(row.timestamp, STARTED_AT + 5_000, "整包同刻 = finished_at");
    }
}

/// 委派 Err（未建档 change / 重复 run_id 冲突）→ `Err(String)` 串语义透传且
/// 记因语境保留（walker fail-fast / best-effort 分流的输入面）。
#[test]
fn store_run_history委派err串语义记因保留() {
    let db = TestDb::open("delegate-err");
    let history =
        StoreRunHistory::new(Arc::clone(&db.store) as Arc<dyn workflow::state::ChangeStateStore>);

    // 未建档 change：run_started Err 携建档语义记因（walker fail-fast 输入面）
    let err = history
        .run_started(&RunStartCommand {
            run_id: "run-no-change".to_owned(),
            change_id: "ghost-change".to_owned(),
            started_at: STARTED_AT,
        })
        .expect_err("未建档 change 应 Err");
    assert!(
        err.contains("ghost-change") && err.contains("未建档"),
        "Err 串语境保留，实际: {err}"
    );

    // 同 run_id 冲突：第二次 run_started Err 携冲突记因
    history
        .run_started(&RunStartCommand {
            run_id: RUN_ID.to_owned(),
            change_id: CHANGE_ID.to_owned(),
            started_at: STARTED_AT,
        })
        .expect("首次 run_started 应成功");
    let err = history
        .run_started(&RunStartCommand {
            run_id: RUN_ID.to_owned(),
            change_id: CHANGE_ID.to_owned(),
            started_at: STARTED_AT,
        })
        .expect_err("同 run_id 二次发起应 Err");
    assert!(
        err.contains(RUN_ID) && (err.contains("冲突") || err.contains("已存在")),
        "冲突记因语境保留，实际: {err}"
    );

    // 未建档 change 收口：run_finished Err 透传（best-effort 输入面）
    let err = history
        .run_finished(&RunFinishCommand {
            run_id: "run-ghost".to_owned(),
            change_id: "ghost-change".to_owned(),
            status: RunStatus::Completed,
            reason: None,
            finished_at: STARTED_AT,
            steps: Vec::new(),
        })
        .expect_err("未建档收口应 Err");
    assert!(
        err.contains("ghost-change"),
        "收口 Err 语境保留，实际: {err}"
    );
}

/// port 契约 Send + Sync 硬校验（跨线程经 Arc<dyn RunHistoryPort> 注入 walker
/// 的编译前提，沿 port 内 const 块同款手法）。
#[test]
fn store_run_history_port契约send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<StoreRunHistory>();
    assert_send_sync::<Arc<dyn RunHistoryPort>>();
}
