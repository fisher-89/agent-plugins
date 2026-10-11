//! `run_history.rs` 的单元测试（test-design「run_history.rs ->
//! run_history_test.rs」节）：词汇直映射十值零过滤（`persisted_step` 过滤谓词整
//! 体退役——编译面锚 + 全词汇在案行为锚，AC-3）、`finish_command` 组装（节点整包
//! 全词汇 + 边整包四 kind 端点身份串 + 实例号递增直透 + 终态三值映射 +
//! session_id / detail / output_ref 两态透传 + 空整包含空集 + 悬空边不静默丢弃）、
//! `StoreRunHistory` 适配器真件委派（run_started 落 running 行 / run_finished
//! nodes + edges 整包落库回读逐字段一致 + Err(String) 串语义透传记因保留）。
//!
//! Mock策略（test-design 本节 Mock 表）：无（真实组合）——纯函数直测 +
//! tempfile 真件库（沿 walker_test TestDb 装置）承载 StoreRunHistory 委派。

use std::sync::Arc;

use store::Store;

use crate::port::RunHistoryPort;
use crate::run_history::{finish_command, StoreRunHistory};
use crate::state::{
    ChangeRunStatus, ChangeStepEdge, ChangeStepKind, ChangeStepState, ChangeStepStatus, EdgeKind,
};
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
        main_root: "/ws/root".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        run_id: RUN_ID.to_owned(),
        auto_next_phase: true,
        stop_before: Some("archive".to_owned()),
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
        instance_no: 0,
        status,
        session_id: session_id.map(str::to_owned),
        detail: None,
        output_ref: None,
    }
}

// ---------------------------------------------------------------------------
// 词汇直映射：十值零过滤（`persisted_step` 过滤谓词整体退役）
// ---------------------------------------------------------------------------

/// 十值全词汇直映射：`ChangeStepKind` 与 `RunStepKind` 1:1 同词面全映射——流程
/// 面三步（phase_start / phase_log / 三门）不再被弃落（编译面锚：`persisted_step`
/// 符号缺席，引用即不编译，AC-3 退役面）。
#[test]
fn 全词汇直映射十值零过滤() {
    let all = [
        (
            ChangeStepKind::Executor,
            workflow::state::RunStepKind::Executor,
        ),
        (
            ChangeStepKind::Evaluator,
            workflow::state::RunStepKind::Evaluator,
        ),
        (
            ChangeStepKind::Decision,
            workflow::state::RunStepKind::Decision,
        ),
        (
            ChangeStepKind::StaticCheck,
            workflow::state::RunStepKind::StaticCheck,
        ),
        (
            ChangeStepKind::TestExecution,
            workflow::state::RunStepKind::TestExecution,
        ),
        (
            ChangeStepKind::PhaseStart,
            workflow::state::RunStepKind::PhaseStart,
        ),
        (
            ChangeStepKind::PhaseLog,
            workflow::state::RunStepKind::PhaseLog,
        ),
        (
            ChangeStepKind::VerdictGate,
            workflow::state::RunStepKind::VerdictGate,
        ),
        (
            ChangeStepKind::RetryGate,
            workflow::state::RunStepKind::RetryGate,
        ),
        (
            ChangeStepKind::WhitelistGate,
            workflow::state::RunStepKind::WhitelistGate,
        ),
    ];

    let nodes: Vec<ChangeStepState> = all
        .iter()
        .map(|(kind, _)| step(*kind, ChangeStepStatus::Passed, None))
        .collect();
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &nodes,
        &[],
    );

    assert_eq!(command.nodes.len(), 10, "十值全词汇零过滤");
    for (index, (_, expect)) in all.iter().enumerate() {
        assert_eq!(command.nodes[index].step, *expect, "第 {index} 词汇直映射");
    }
    assert!(
        command
            .nodes
            .iter()
            .any(|node| node.step == workflow::state::RunStepKind::PhaseStart),
        "phase_start 不再被弃落"
    );
    assert!(
        command
            .nodes
            .iter()
            .any(|node| node.step == workflow::state::RunStepKind::WhitelistGate),
        "三门之 whitelist_gate 不再被弃落"
    );
}

/// 边整包组装：四 kind 显式落边，`fromNodeKey` / `toNodeKey` 端点指向节点身份串
/// `"{phase}:{attempt}:{step}:{instance_no}"`（AC-3 / AC-4）。
#[test]
fn finish_command边整包四kind端点身份串() {
    let from = ("implement".to_owned(), 1, ChangeStepKind::StaticCheck, 0);
    let to = ("implement".to_owned(), 1, ChangeStepKind::Executor, 1);
    let edges = vec![
        ChangeStepEdge {
            from: ("implement".to_owned(), 1, ChangeStepKind::PhaseStart, 0),
            to: ("implement".to_owned(), 1, ChangeStepKind::Executor, 0),
            kind: EdgeKind::Forward,
        },
        ChangeStepEdge {
            from: from.clone(),
            to: to.clone(),
            kind: EdgeKind::Feedback,
        },
        ChangeStepEdge {
            from: ("implement".to_owned(), 1, ChangeStepKind::Executor, 1),
            to: ("implement".to_owned(), 1, ChangeStepKind::RetryGate, 0),
            kind: EdgeKind::Retry,
        },
        ChangeStepEdge {
            from: ("implement".to_owned(), 1, ChangeStepKind::WhitelistGate, 0),
            to: ("dev-design".to_owned(), 1, ChangeStepKind::Executor, 0),
            kind: EdgeKind::Backtrack,
        },
    ];

    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &[],
        &edges,
    );

    assert_eq!(command.edges.len(), 4, "四 kind 齐备");
    assert_eq!(
        command.edges[0].edge_kind,
        workflow::state::RunEdgeKind::Forward
    );
    assert_eq!(
        command.edges[1].edge_kind,
        workflow::state::RunEdgeKind::Feedback
    );
    assert_eq!(
        command.edges[2].edge_kind,
        workflow::state::RunEdgeKind::Retry
    );
    assert_eq!(
        command.edges[3].edge_kind,
        workflow::state::RunEdgeKind::Backtrack
    );
    assert_eq!(
        command.edges[1].from_node_key, "implement:1:static_check:0",
        "端点 = 节点身份串（feedback 回边区分 retry）"
    );
    assert_eq!(command.edges[1].to_node_key, "implement:1:executor:1");
    assert_eq!(
        command.edges[3].to_node_key, "dev-design:1:executor:0",
        "跨相位回跳端点"
    );
    assert_ne!(
        command.edges[1].from_node_key, command.edges[2].from_node_key,
        "feedback 与 retry 端点各异不混淆"
    );
}

/// 实例号分配：同 `(phase, attempt, step)` 的 feedback 重跑 → `instance_no`
/// 递增稳定直透（AC-4 身份键）。
#[test]
fn finish_command实例号递增直透() {
    let first = step(ChangeStepKind::Executor, ChangeStepStatus::Failed, None);
    let mut repaired = step(ChangeStepKind::Executor, ChangeStepStatus::Passed, None);
    repaired.instance_no = 1;
    repaired.detail = Some("按反馈修复重跑".to_owned());
    let other_phase = ChangeStepState {
        phase: "dev-design".to_owned(),
        ..step(ChangeStepKind::Executor, ChangeStepStatus::Passed, None)
    };

    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &[first, repaired, other_phase],
        &[],
    );

    let instances: Vec<u32> = command
        .nodes
        .iter()
        .filter(|node| node.phase == "implement")
        .map(|node| node.instance_no)
        .collect();
    assert_eq!(instances, vec![0, 1], "同相位同步实例号递增");
    assert_eq!(
        command.nodes[2].instance_no, 0,
        "跨相位实例号互不串台（各自从 0 起）"
    );
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
            &[],
        );
        assert_eq!(command.run_id, RUN_ID);
        assert_eq!(command.change_id, CHANGE_ID, "载荷 change_id 透传等值");
        assert_eq!(command.status, expect, "终态三值同名词汇映射");
        assert_eq!(command.reason.as_deref(), Some("收口记因"));
        assert_eq!(command.finished_at, STARTED_AT + 1_000);
    }
}

/// 空节点 / 空边 → 空包（零步 run 收口合法，AC-5 整包含空集）；session_id /
/// detail / output_ref None 与 Some 两态透传（AC-8 输出引用口径）。
#[test]
fn finish_command空包与两态透传() {
    let empty = finish_command(
        &request(),
        ChangeRunStatus::Stopped,
        None,
        STARTED_AT,
        &[],
        &[],
    );
    assert!(empty.nodes.is_empty(), "空节点 → 空包（零步 run 收口合法）");
    assert!(empty.edges.is_empty(), "空边 → 空包");
    assert_eq!(empty.reason, None, "reason None 透传");

    let none_step = step(ChangeStepKind::StaticCheck, ChangeStepStatus::Passed, None);
    let mut some_step = ChangeStepState {
        session_id: Some("ses-9".to_owned()),
        detail: Some("静态检查通过".to_owned()),
        output_ref: Some("exec-1727000000000".to_owned()),
        ..none_step.clone()
    };
    some_step.instance_no = 1;
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &[none_step, some_step],
        &[],
    );
    assert_eq!(command.nodes.len(), 2);
    assert_eq!(command.nodes[0].session_id, None, "None 两态透传");
    assert_eq!(command.nodes[0].detail, None);
    assert_eq!(command.nodes[0].output_ref, None, "Transient 恒 None");
    assert_eq!(command.nodes[1].session_id.as_deref(), Some("ses-9"));
    assert_eq!(command.nodes[1].detail.as_deref(), Some("静态检查通过"));
    assert_eq!(
        command.nodes[1].output_ref.as_deref(),
        Some("exec-1727000000000"),
        "Recorded 输出引用透传（AC-8）"
    );
}

/// 边端点引用不存在的节点身份串 → 组装仍透传（不静默丢弃悬空边；整包语义
/// 校验归上层 store 单事务面，AC-3 / AC-5）。
#[test]
fn finish_command悬空边端点原样透传不静默丢弃() {
    let dangling = ChangeStepEdge {
        from: ("implement".to_owned(), 1, ChangeStepKind::Executor, 0),
        to: (
            "ghost-phase".to_owned(),
            9,
            ChangeStepKind::WhitelistGate,
            7,
        ),
        kind: EdgeKind::Backtrack,
    };
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &[step(
            ChangeStepKind::Executor,
            ChangeStepStatus::Passed,
            None,
        )],
        std::slice::from_ref(&dangling),
    );

    assert_eq!(command.edges.len(), 1, "悬空边不静默丢弃");
    assert_eq!(
        command.edges[0].to_node_key,
        "ghost-phase:9:whitelist_gate:7"
    );
    assert_eq!(
        command.edges[0].from_node_key,
        dangling.from_node_key(),
        "端点组装单点同式"
    );
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

    let mut repaired = step(ChangeStepKind::Executor, ChangeStepStatus::Passed, None);
    repaired.instance_no = 1;
    repaired.output_ref = Some("exec-1727000005000".to_owned());
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
            repaired,
        ],
        &[ChangeStepEdge {
            from: ("implement".to_owned(), 1, ChangeStepKind::Executor, 0),
            to: ("implement".to_owned(), 1, ChangeStepKind::Evaluator, 0),
            kind: EdgeKind::Forward,
        }],
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

    let nodes = db
        .store
        .list_run_nodes(RUN_ID)
        .expect("list_run_nodes 应成功");
    assert_eq!(nodes.len(), 3, "节点整包三行");
    assert_eq!(nodes[0].session_id.as_deref(), Some("ses-1"));
    assert_eq!(nodes[0].instance_no, 0);
    assert_eq!(
        nodes[1].status,
        workflow::state::RunStepStatus::Failed,
        "状态透传"
    );
    assert_eq!(nodes[2].instance_no, 1, "feedback 重跑实例号随节点整包落库");
    assert_eq!(
        nodes[2].output_ref.as_deref(),
        Some("exec-1727000005000"),
        "output_ref 随节点整包落库（AC-8 引用可回查）"
    );
    for row in &nodes {
        assert_eq!(row.timestamp, STARTED_AT + 5_000, "整包同刻 = finished_at");
    }

    let edges = db
        .store
        .list_run_edges(RUN_ID)
        .expect("list_run_edges 应成功");
    assert_eq!(edges.len(), 1, "边整包随收口落库");
    assert_eq!(
        edges[0].edge_kind,
        workflow::state::RunEdgeKind::Forward,
        "边 kind 四值词汇直落"
    );
    assert_eq!(edges[0].from_node_key, "implement:1:executor:0");
    assert_eq!(edges[0].to_node_key, "implement:1:evaluator:0");
    assert_eq!(edges[0].timestamp, STARTED_AT + 5_000, "整包同刻");
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
            nodes: Vec::new(),
            edges: Vec::new(),
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
