//! `run_history.rs` 的单元测试（test-design「run_history.rs ->
//! run_history_test.rs」节）：词汇直映射二十一值零过滤（`persisted_step` 过滤谓词整
//! 体退役——编译面锚 + 全词汇在案行为锚，AC-3；cleanup 尽力步新臂直映射在案）、
//! `finish_command` 组装（节点整包
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
        started_at: STARTED_AT,
        ended_at: None,
        token_input: None,
        token_output: None,
    }
}

// ---------------------------------------------------------------------------
// 词汇直映射：二十一值零过滤（`persisted_step` 过滤谓词整体退役）
// ---------------------------------------------------------------------------

/// mapped_step 新臂直映射（正向，check-before-flow-start）：`ChangeStepKind::Clean`
/// → `RunStepKind::Clean` 1:1 直映射——`as_str()` 同词面 `"clean"` 逐字（initialize
/// 基线恢复步落库零过滤单点）。
#[test]
fn mapped_step_clean直映射_同词面逐字() {
    let nodes = [step(ChangeStepKind::Clean, ChangeStepStatus::Passed, None)];
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &nodes,
        &[],
    );
    assert_eq!(command.nodes.len(), 1);
    assert_eq!(
        command.nodes[0].step,
        workflow::state::RunStepKind::Clean,
        "clean 直映射零过滤"
    );
    assert_eq!(
        command.nodes[0].step.as_str(),
        ChangeStepKind::Clean.as_str(),
        "两域 as_str 同词面逐字（clean）"
    );
    assert_eq!(command.nodes[0].step.as_str(), "clean");
}

/// initialize run 五节点落包（边界，check-before-flow-start）：phase_start / clean /
/// static_check / test_execution / phase_log 五节点零过滤入 `command.nodes`（无静默
/// 丢弃）且串行 forward 边逐位连接——initialize 步族全词汇经落包入口入史（AC-1 /
/// AC-8 落库半边）。
#[test]
fn finish_command_initialize五节点零过滤入包且串行forward边逐位() {
    let initialize_phase = "initialize";
    let kinds = [
        ChangeStepKind::PhaseStart,
        ChangeStepKind::Clean,
        ChangeStepKind::StaticCheck,
        ChangeStepKind::TestExecution,
        ChangeStepKind::PhaseLog,
    ];
    let nodes: Vec<ChangeStepState> = kinds
        .iter()
        .map(|kind| {
            let mut node = step(*kind, ChangeStepStatus::Passed, None);
            node.phase = initialize_phase.to_owned();
            node
        })
        .collect();
    // 串行 forward 边：相邻两节点逐位连接（0→1→2→3→4）
    let edges: Vec<ChangeStepEdge> = nodes
        .windows(2)
        .map(|pair| ChangeStepEdge {
            from: (
                pair[0].phase.clone(),
                pair[0].attempt,
                pair[0].step,
                pair[0].instance_no,
            ),
            to: (
                pair[1].phase.clone(),
                pair[1].attempt,
                pair[1].step,
                pair[1].instance_no,
            ),
            kind: EdgeKind::Forward,
        })
        .collect();

    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &nodes,
        &edges,
    );

    // 五节点零过滤（顺序 = 入参序、词面逐字）
    assert_eq!(command.nodes.len(), 5, "五节点无静默丢弃");
    let expect_words = ["phase_start", "clean", "static_check", "test_execution", "phase_log"];
    for (index, word) in expect_words.iter().enumerate() {
        assert_eq!(
            command.nodes[index].step.as_str(),
            *word,
            "第 {index} 节点词面逐字（{word}）"
        );
        assert_eq!(command.nodes[index].phase, initialize_phase);
    }
    assert_eq!(command.nodes[1].step, workflow::state::RunStepKind::Clean);

    // 串行 forward 边四条、端点逐位对齐
    assert_eq!(command.edges.len(), 4, "串行 forward 边四条");
    for (index, edge) in command.edges.iter().enumerate() {
        assert_eq!(edge.edge_kind, workflow::state::RunEdgeKind::Forward);
        assert_eq!(
            edge.from_node_key,
            format!("{initialize_phase}:1:{}:0", expect_words[index]),
            "第 {index} 边 from 端点身份串"
        );
        assert_eq!(
            edge.to_node_key,
            format!("{initialize_phase}:1:{}:0", expect_words[index + 1]),
            "第 {index} 边 to 端点身份串"
        );
    }
}

/// mapped_step 新臂直映射（正向，worktree-cleanup-and-active-cap）：
/// `ChangeStepKind::Cleanup` → `RunStepKind::Cleanup` 1:1 直映射——`as_str()`
/// 同词面 `"cleanup"` 逐字（归档尽力步落库零过滤单点）。
#[test]
fn mapped_step_cleanup直映射_同词面逐字() {
    let nodes = [step(
        ChangeStepKind::Cleanup,
        ChangeStepStatus::Passed,
        None,
    )];
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT,
        &nodes,
        &[],
    );
    assert_eq!(command.nodes.len(), 1);
    assert_eq!(
        command.nodes[0].step,
        workflow::state::RunStepKind::Cleanup,
        "cleanup 直映射零过滤"
    );
    assert_eq!(
        command.nodes[0].step.as_str(),
        ChangeStepKind::Cleanup.as_str(),
        "两域 as_str 同词面逐字（cleanup）"
    );
    assert_eq!(command.nodes[0].step.as_str(), "cleanup");
}

/// 二十一值全词汇直映射：`ChangeStepKind` 与 `RunStepKind` 1:1 同词面全映射——流程
/// 面三步（phase_start / phase_log / 三门）不再被弃落，mutation_test 拆步新词
/// 在案、cleanup 尽力步新臂在案、clean 基线恢复步新臂在案（编译面锚：`persisted_step`
/// 符号缺席，引用即不编译，AC-3 退役面；AC-1 / AC-2 二十一值零过滤——finish_command
/// 落包 `nodes.len()==21` 零过滤，cleanup / clean 节点在案、无静默丢弃）。
#[test]
fn 全词汇直映射二十一值零过滤() {
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
            ChangeStepKind::MutationTest,
            workflow::state::RunStepKind::MutationTest,
        ),
        (ChangeStepKind::Clean, workflow::state::RunStepKind::Clean),
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
        (
            ChangeStepKind::Preflight,
            workflow::state::RunStepKind::Preflight,
        ),
        (ChangeStepKind::Commit, workflow::state::RunStepKind::Commit),
        (ChangeStepKind::Rebase, workflow::state::RunStepKind::Rebase),
        (
            ChangeStepKind::ResolveConflicts,
            workflow::state::RunStepKind::ResolveConflicts,
        ),
        (ChangeStepKind::Merge, workflow::state::RunStepKind::Merge),
        (
            ChangeStepKind::SpecSync,
            workflow::state::RunStepKind::SpecSync,
        ),
        (
            ChangeStepKind::Cleanup,
            workflow::state::RunStepKind::Cleanup,
        ),
        (ChangeStepKind::Seal, workflow::state::RunStepKind::Seal),
        (
            ChangeStepKind::Finalize,
            workflow::state::RunStepKind::Finalize,
        ),
    ];
    assert_eq!(all.len(), 21, "二十一值封闭集全枚举");

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

    assert_eq!(
        command.nodes.len(),
        21,
        "二十一值全词汇零过滤（cleanup / clean 节点在案）"
    );
    for (index, (_, expect)) in all.iter().enumerate() {
        assert_eq!(command.nodes[index].step, *expect, "第 {index} 词汇直映射");
    }
    assert!(
        command
            .nodes
            .iter()
            .any(|node| node.step == workflow::state::RunStepKind::MutationTest),
        "mutation_test 拆步新词直映射在案（AC-1 / AC-2）"
    );
    assert!(
        command
            .nodes
            .iter()
            .any(|node| node.step == workflow::state::RunStepKind::Cleanup),
        "cleanup 尽力步新臂直映射在案（零静默丢弃）"
    );
    assert!(
        command
            .nodes
            .iter()
            .any(|node| node.step == workflow::state::RunStepKind::Clean),
        "clean 基线恢复步新臂直映射在案（零静默丢弃）"
    );
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

    // 双射校验（异常面 / 词汇同源不漂移）：无两个 ChangeStepKind 映到同一
    // RunStepKind（21 对互异即双射——两侧封闭集同势）
    let mut mapped: Vec<&'static str> = command
        .nodes
        .iter()
        .map(|node| node.step.as_str())
        .collect();
    let total = mapped.len();
    mapped.sort_unstable();
    mapped.dedup();
    assert_eq!(
        mapped.len(),
        total,
        "双射无两词映一（词汇同源不漂移），实际: {mapped:?}"
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

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（AC-1 / AC-2 / AC-4）：finish_command 节点四
// 字段（started_at / ended_at / token_input / token_output）直复制——零过滤
// 零改写零铸造（时间与 token 铸造归 walker emit_node 单点，组装面纯复制）
// ---------------------------------------------------------------------------

/// 全值节点 fixture（四字段全值形态——StartedAt 定值 / ended +200_000 /
/// token 12_000 与 3_400，D10 词面断言同源定值）。
fn full_metrics_step(kind: ChangeStepKind) -> ChangeStepState {
    let mut node = step(kind, ChangeStepStatus::Passed, Some("ses-1"));
    node.detail = Some("实现完成".to_owned());
    node.started_at = STARTED_AT;
    node.ended_at = Some(STARTED_AT + 200_000);
    node.token_input = Some(12_000);
    node.token_output = Some(3_400);
    node
}

/// 正向：节点四字段全值 → `RunNodeEntry` 逐字段等值直复制（单节点）。
#[test]
fn finish_command节点四字段全值逐字段直复制() {
    let worker = full_metrics_step(ChangeStepKind::Executor);
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT + 400_000,
        std::slice::from_ref(&worker),
        &[],
    );
    assert_eq!(command.nodes.len(), 1);
    let entry = &command.nodes[0];
    assert_eq!(entry.started_at, STARTED_AT, "started_at 直复制");
    assert_eq!(
        entry.ended_at,
        Some(STARTED_AT + 200_000),
        "ended_at 直复制"
    );
    assert_eq!(entry.token_input, Some(12_000), "token_input 直复制");
    assert_eq!(entry.token_output, Some(3_400), "token_output 直复制");
    assert_eq!(entry.session_id.as_deref(), Some("ses-1"));
    assert_eq!(entry.detail.as_deref(), Some("实现完成"));
}

/// 正向：多节点混排（WorkerAgent / 工具 / Gate / Git 步）→ 四字段各自直复制、
/// 整包序 = 入参序。
#[test]
fn finish_command多节点混排四字段各自直复制整包序保持入参序() {
    let worker = full_metrics_step(ChangeStepKind::Executor);
    let tool = step(ChangeStepKind::StaticCheck, ChangeStepStatus::Passed, None);
    let gate = step(ChangeStepKind::VerdictGate, ChangeStepStatus::Passed, None);
    let mut git = step(ChangeStepKind::Commit, ChangeStepStatus::Passed, None);
    git.started_at = STARTED_AT + 10;
    git.ended_at = Some(STARTED_AT + 20);
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT + 30,
        &[worker, tool, gate, git],
        &[],
    );
    assert_eq!(command.nodes.len(), 4, "整包序 = 入参序（零过滤零重排）");
    assert_eq!(
        command.nodes[0].step,
        workflow::state::RunStepKind::Executor
    );
    assert_eq!(command.nodes[0].token_input, Some(12_000));
    assert_eq!(
        command.nodes[1].step,
        workflow::state::RunStepKind::StaticCheck
    );
    assert_eq!(command.nodes[1].token_input, None, "工具步 token None 直透");
    assert_eq!(
        command.nodes[2].step,
        workflow::state::RunStepKind::VerdictGate
    );
    assert_eq!(
        command.nodes[2].token_output, None,
        "Gate 步 token None 直透"
    );
    assert_eq!(command.nodes[3].step, workflow::state::RunStepKind::Commit);
    assert_eq!(command.nodes[3].started_at, STARTED_AT + 10);
    assert_eq!(command.nodes[3].ended_at, Some(STARTED_AT + 20));
}

/// 边界：token 两值与 ended_at 全 None（工具步 / Gate 形态）→ None 直透
///（不铸造不默认——组装面零铸造单点锚）。
#[test]
fn finish_command全none字段直透不铸造不默认() {
    let gate = step(
        ChangeStepKind::WhitelistGate,
        ChangeStepStatus::Stopped,
        None,
    );
    assert_eq!(gate.ended_at, None, "fixture 形态：ended / token 全 None");
    let command = finish_command(
        &request(),
        ChangeRunStatus::Stopped,
        None,
        STARTED_AT,
        std::slice::from_ref(&gate),
        &[],
    );
    assert_eq!(command.nodes[0].ended_at, None, "ended None 直透");
    assert_eq!(command.nodes[0].token_input, None);
    assert_eq!(command.nodes[0].token_output, None);
}

/// 边界：started_at = 0（缺席语义值）→ 原样复制零改写。
#[test]
fn finish_command_started_at零值原样复制零改写() {
    let mut zero = step(ChangeStepKind::Executor, ChangeStepStatus::Running, None);
    zero.started_at = 0;
    let command = finish_command(
        &request(),
        ChangeRunStatus::Stopped,
        None,
        STARTED_AT,
        std::slice::from_ref(&zero),
        &[],
    );
    assert_eq!(command.nodes[0].started_at, 0, "0 缺席语义值原样直复制");
}

/// 异常：running 残留步入包（status=Running、ended_at=None）→ None 直透不补
/// 值不报错（收口在途步合法形态——收口在途步可留 running）。
#[test]
fn finish_command_running残留步none直透不补值不报错() {
    let running = step(
        ChangeStepKind::Evaluator,
        ChangeStepStatus::Running,
        Some("ses-2"),
    );
    let command = finish_command(
        &request(),
        ChangeRunStatus::Stopped,
        None,
        STARTED_AT,
        std::slice::from_ref(&running),
        &[],
    );
    assert_eq!(
        command.nodes[0].status,
        workflow::state::RunStepStatus::Running,
        "running 状态原样落包"
    );
    assert_eq!(command.nodes[0].ended_at, None, "在途步 ended None 不补值");
    assert_eq!(command.nodes[0].token_input, None);
}

/// 异常：瞬步 started_at == ended_at 同刻值 → 两字段独立直复制（互不推导不
/// 合并——单 emit 瞬步同刻形态）。
#[test]
fn finish_command瞬步同刻两字段独立直复制() {
    let mut instant = step(ChangeStepKind::PhaseStart, ChangeStepStatus::Passed, None);
    instant.started_at = STARTED_AT + 5_000;
    instant.ended_at = Some(STARTED_AT + 5_000);
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT + 6_000,
        std::slice::from_ref(&instant),
        &[],
    );
    assert_eq!(command.nodes[0].started_at, STARTED_AT + 5_000);
    assert_eq!(
        command.nodes[0].ended_at,
        Some(STARTED_AT + 5_000),
        "同刻值独立直复制（零推导零合并）"
    );
}

/// 组合（透传链上半程锚，AC-1）：四字段经 finish_command 组装后喂 store 落库
///（`finish_change_run` 内部即 `RunNodeRecord::new` 消费点——下半程见
/// model_test.rs 节）→ `list_run_nodes` 读回与源 `ChangeStepState` 逐字段一致。
#[test]
fn finish_command四字段经落库读回与源步状态行逐字段一致() {
    let db = TestDb::open("four-field-chain");
    let history =
        StoreRunHistory::new(Arc::clone(&db.store) as Arc<dyn workflow::state::ChangeStateStore>);
    history
        .run_started(&RunStartCommand {
            run_id: RUN_ID.to_owned(),
            change_id: CHANGE_ID.to_owned(),
            started_at: STARTED_AT,
        })
        .expect("run_started 应成功");

    let source = full_metrics_step(ChangeStepKind::Executor);
    let command = finish_command(
        &request(),
        ChangeRunStatus::Completed,
        None,
        STARTED_AT + 300_000,
        std::slice::from_ref(&source),
        &[],
    );
    history.run_finished(&command).expect("run_finished 应成功");

    let rows = db
        .store
        .list_run_nodes(RUN_ID)
        .expect("list_run_nodes 应成功");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].started_at, source.started_at);
    assert_eq!(rows[0].ended_at, source.ended_at);
    assert_eq!(rows[0].token_input, source.token_input);
    assert_eq!(rows[0].token_output, source.token_output);
    assert_eq!(
        rows[0].timestamp,
        STARTED_AT + 300_000,
        "整包同刻 = finished_at"
    );
}
