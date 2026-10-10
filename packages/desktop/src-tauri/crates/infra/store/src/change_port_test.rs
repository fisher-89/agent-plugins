//! [`ChangeStateStore`](workflow::state::ChangeStateStore) 的 store 适配器
//! （change_port.rs）行为测试：tempfile 真实 Store 上以 `&dyn` / `Arc<dyn>`
//! 类型擦除驱动 trait 十一方法，验证记录 ↔ 中性类型映射单点无加工（design
//! D2）、`StoreError` → `StoreFault` 三分支翻译、Send + Sync port 边界与补
//! 偿删除面。零假件——被测主体即真件，trait object 仅强转（沿 change_port
//! Mock 策略行）。

use std::sync::Arc;

use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, PhaseLogCommand, StepCommand, StepKind,
    StoreFault,
};

use crate::store::Store;

// ---------------------------------------------------------------------------
// 装置：tempfile 真实 workspace db（时间戳显式注入，等值断言与钟面无关）
// ---------------------------------------------------------------------------

struct PortEnv {
    db_dir: tempfile::TempDir,
}

impl PortEnv {
    fn new(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("store-test-port-{tag}-db-"))
            .tempdir()
            .expect("创建临时目录失败");
        Self { db_dir }
    }

    fn db_path(&self, tag: &str) -> std::path::PathBuf {
        self.db_dir.path().join(format!("{tag}.redb"))
    }
}

fn open_workspace_ok(path: &std::path::Path) -> Store {
    Store::open_workspace(path).unwrap_or_else(|e| panic!("open_workspace 应成功: {e}"))
}

/// 建档命令 fixture（active 起步；id 主键与 name 属性双入参——身份面用例双值
/// 可辨，既有用例可传同串便捷坐实）。
fn change_archive(id: &str, name: &str, created_at: i64) -> ChangeStateRecord {
    ChangeStateRecord {
        id: id.to_owned(),
        name: name.to_owned(),
        title: name.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at,
        status: workflow::state::ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    }
}

/// checklist 检查项 fixture（三面可区分）。
fn check_item(text: &str, pass: bool) -> ChecklistItem {
    ChecklistItem {
        item: text.to_owned(),
        pass,
        evidence: format!("证据-{text}"),
    }
}

/// 落账命令 fixture。
fn log_command(
    change: &str,
    phase: &str,
    verdict: Verdict,
    checklist: Vec<ChecklistItem>,
    start_at: Option<i64>,
    timestamp: i64,
) -> PhaseLogCommand {
    PhaseLogCommand {
        change_id: change.to_owned(),
        phase: phase.to_owned(),
        verdict,
        report: format!("{phase} 评估报告"),
        skipped: false,
        checklist,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
        start_at,
        timestamp,
    }
}

/// 步骤审计命令 fixture。
fn step_command(
    run_id: &str,
    change: &str,
    step_kind: StepKind,
    status: &str,
    summary: &str,
    timestamp: i64,
) -> StepCommand {
    StepCommand {
        run_id: run_id.to_owned(),
        change_id: change.to_owned(),
        step_kind,
        status: status.to_owned(),
        summary: summary.to_owned(),
        reference: None,
        timestamp,
    }
}

// ---------------------------------------------------------------------------
// trait object 全链映射：建档 → start_phase → log_phase → list_phase_records
// 经 `&dyn` 驱动，中性类型快照与直调 Store 方法逐字段一致（D2 映射单点无加工）
// ---------------------------------------------------------------------------

#[test]
fn trait_object全链映射_建档开相落账读史与直调store逐字段一致() {
    let env = PortEnv::new("full-chain");
    let store = open_workspace_ok(&env.db_path("ws"));
    let port: &dyn ChangeStateStore = &store;
    const CHANGE_ID: &str = "chg-port-0001";

    // 写半边全链经 trait 入口（形参一律身份锚 id）
    port.create_change_record(change_archive(CHANGE_ID, "flow", 1000))
        .expect("建档应成功");
    let started = port
        .start_phase(CHANGE_ID, "proposal", 2000)
        .expect("开相应成功");
    let attempt = port
        .log_phase(&log_command(
            CHANGE_ID,
            "proposal",
            Verdict::Pass,
            vec![check_item("检查项一", true), check_item("检查项二", false)],
            Some(started.start_at),
            3000,
        ))
        .expect("落账应成功");
    assert_eq!(attempt, 1, "trait 入口返回事务内推导 attempt");

    // 读半边经 trait：中性类型快照与直调 Store 方法逐字段一致（映射单点无加工）
    assert_eq!(
        port.get_change(CHANGE_ID).unwrap(),
        store.find_change_record(CHANGE_ID).unwrap(),
        "建档快照映射无加工"
    );
    assert_eq!(
        port.list_change_records().unwrap(),
        store.list_change_records().unwrap(),
        "建档清单映射无加工"
    );
    let via_port = port.list_phase_records(CHANGE_ID).unwrap();
    let via_store = store.list_phase_records(CHANGE_ID).unwrap();
    assert_eq!(via_port, via_store, "相位史快照映射无加工");
    assert_eq!(via_port.len(), 1);
    assert_eq!(via_port[0].change_id, CHANGE_ID, "归属列 = 身份锚 id");
    assert_eq!(via_port[0].attempt, attempt);
    assert_eq!(via_port[0].start_at, Some(2000), "start_at 随行透出");
    assert_eq!(
        via_port[0].checklist,
        vec![check_item("检查项一", true), check_item("检查项二", false)],
        "checklist 内联重组随映射透出"
    );
    // 落账清位经 trait 快照可见（active_phase 匹配 / 步骤审计读半边同验）
    assert_eq!(
        port.get_change(CHANGE_ID).unwrap().unwrap().active_phase,
        None,
        "落账即清位经 trait 可见"
    );
    assert_eq!(
        port.list_steps(CHANGE_ID, None).unwrap(),
        store.list_change_steps(CHANGE_ID, None).unwrap(),
        "步骤审计读半边映射无加工"
    );
    // 身份面：id 为寻址键、name 仅展示属性
    let snapshot = port.get_change(CHANGE_ID).unwrap().expect("建档在案");
    assert_eq!(snapshot.id, CHANGE_ID, "trait 面 id 身份锚");
    assert_eq!(snapshot.name, "flow", "trait 面 name 普通属性");
    assert_eq!(
        port.get_change("flow").unwrap(),
        None,
        "name 串按 id 查询 miss（name 非寻址键，port 面零例外）"
    );
}

// ---------------------------------------------------------------------------
// 写命令翻译：PhaseLogCommand / BacktrackCommand / StepCommand 经 trait 入口
// 落库，与 Store 原生方法直调落库行逐字段等值（两库镜像序列对拍）
// ---------------------------------------------------------------------------

/// 镜像写序列用的 change 身份锚（id）与展示名（name）双值可辨：五写命令载荷
/// 一律携 id，name 仅为记录属性。
const MIRROR_CHANGE_ID: &str = "chg-mirror";
const MIRROR_CHANGE_NAME: &str = "flow";

/// 镜像写序列（change 域全写操作各一次；两侧分别经 trait 入口与原生直调执行
/// 同一命令面——命令载荷 change_id 恒为身份锚 id）。
fn mirror_sequence_port(port: &dyn ChangeStateStore) {
    port.create_change_record(change_archive(MIRROR_CHANGE_ID, MIRROR_CHANGE_NAME, 1000))
        .expect("建档应成功");
    port.start_phase(MIRROR_CHANGE_ID, "proposal", 2000)
        .expect("开相应成功");
    port.log_phase(&log_command(
        MIRROR_CHANGE_ID,
        "proposal",
        Verdict::Pass,
        vec![check_item("检查项一", true), check_item("检查项二", false)],
        Some(2000),
        2500,
    ))
    .expect("落账应成功");
    port.start_phase(MIRROR_CHANGE_ID, "proposal", 3000)
        .expect("重开相应成功");
    port.log_phase(&log_command(
        MIRROR_CHANGE_ID,
        "proposal",
        Verdict::Fail,
        vec![check_item("检查项三", false)],
        None,
        4000,
    ))
    .expect("落账应成功");
    port.start_phase(MIRROR_CHANGE_ID, "design", 5000)
        .expect("开相应成功");
    port.log_phase(&log_command(
        MIRROR_CHANGE_ID,
        "design",
        Verdict::Pass,
        vec![],
        None,
        6000,
    ))
    .expect("落账应成功");
    port.amend_decision_session(MIRROR_CHANGE_ID, "proposal", "ses-decision")
        .expect("挂账应成功");
    port.apply_backtrack(&BacktrackCommand {
        change_id: MIRROR_CHANGE_ID.to_owned(),
        phase: "design".to_owned(),
        to: "proposal".to_owned(),
        reason: "镜像回跳".to_owned(),
        stale_dependents: vec![],
    })
    .expect("回跳应成功");
    port.append_step(&step_command(
        "run-1",
        MIRROR_CHANGE_ID,
        StepKind::PhaseNext,
        "ok",
        "轮次推进",
        7000,
    ))
    .expect("步骤追加应成功");
    port.append_step(&step_command(
        "run-2",
        MIRROR_CHANGE_ID,
        StepKind::StaticCheck,
        "error",
        "静态检查失败",
        8000,
    ))
    .expect("步骤追加应成功");
}

/// 镜像写序列的 Store 原生直调半边（与 `mirror_sequence_port` 同一命令面）。
fn mirror_sequence_native(store: &Store) {
    store
        .create_change_record(change_archive(MIRROR_CHANGE_ID, MIRROR_CHANGE_NAME, 1000))
        .expect("建档应成功");
    store
        .start_change_phase(MIRROR_CHANGE_ID, "proposal", 2000)
        .expect("开相应成功");
    store
        .log_change_phase(&log_command(
            MIRROR_CHANGE_ID,
            "proposal",
            Verdict::Pass,
            vec![check_item("检查项一", true), check_item("检查项二", false)],
            Some(2000),
            2500,
        ))
        .expect("落账应成功");
    store
        .start_change_phase(MIRROR_CHANGE_ID, "proposal", 3000)
        .expect("重开相应成功");
    store
        .log_change_phase(&log_command(
            MIRROR_CHANGE_ID,
            "proposal",
            Verdict::Fail,
            vec![check_item("检查项三", false)],
            None,
            4000,
        ))
        .expect("落账应成功");
    store
        .start_change_phase(MIRROR_CHANGE_ID, "design", 5000)
        .expect("开相应成功");
    store
        .log_change_phase(&log_command(
            MIRROR_CHANGE_ID,
            "design",
            Verdict::Pass,
            vec![],
            None,
            6000,
        ))
        .expect("落账应成功");
    store
        .amend_change_decision_session(MIRROR_CHANGE_ID, "proposal", "ses-decision")
        .expect("挂账应成功");
    store
        .apply_change_backtrack(&BacktrackCommand {
            change_id: MIRROR_CHANGE_ID.to_owned(),
            phase: "design".to_owned(),
            to: "proposal".to_owned(),
            reason: "镜像回跳".to_owned(),
            stale_dependents: vec![],
        })
        .expect("回跳应成功");
    store
        .append_change_step(&step_command(
            "run-1",
            MIRROR_CHANGE_ID,
            StepKind::PhaseNext,
            "ok",
            "轮次推进",
            7000,
        ))
        .expect("步骤追加应成功");
    store
        .append_change_step(&step_command(
            "run-2",
            MIRROR_CHANGE_ID,
            StepKind::StaticCheck,
            "error",
            "静态检查失败",
            8000,
        ))
        .expect("步骤追加应成功");
}

#[test]
fn 写命令翻译_trait入口与store原生直调落库行逐字段等值() {
    let env_port = PortEnv::new("translate-port");
    let env_native = PortEnv::new("translate-native");
    let store_via_port = open_workspace_ok(&env_port.db_path("ws"));
    let store_native = open_workspace_ok(&env_native.db_path("ws"));

    mirror_sequence_port(&store_via_port);
    mirror_sequence_native(&store_native);

    // 两库终态逐字段等值（行序 = id 升序，两侧 max+1 分配同序）
    assert_eq!(
        store_via_port.get_change(MIRROR_CHANGE_ID).unwrap(),
        store_native.find_change_record(MIRROR_CHANGE_ID).unwrap(),
        "PhaseLogCommand 翻译落库行等值（active_phase 终态含回跳面）"
    );
    assert_eq!(
        store_via_port.list_change_records().unwrap(),
        store_native.list_change_records().unwrap(),
    );
    assert_eq!(
        store_via_port.list_phase_records(MIRROR_CHANGE_ID).unwrap(),
        store_native.list_phase_records(MIRROR_CHANGE_ID).unwrap(),
        "落账 / 挂账 / 回跳翻译落库行逐字段等值（含 checklist 内联与 stale 位）"
    );
    assert_eq!(
        store_via_port.list_steps(MIRROR_CHANGE_ID, None).unwrap(),
        store_native
            .list_change_steps(MIRROR_CHANGE_ID, None)
            .unwrap(),
        "StepCommand 翻译落库行等值（归属列 = 身份锚 id）"
    );
    assert_eq!(
        store_via_port
            .list_steps(MIRROR_CHANGE_ID, Some("run-1"))
            .unwrap(),
        store_native
            .list_change_steps(MIRROR_CHANGE_ID, Some("run-1"))
            .unwrap(),
    );

    // 翻译语义抽验：amend 定点最新条目、backtrack 标记落发起相位且目标最新
    // pass 置 stale
    let phases = store_via_port.list_phase_records(MIRROR_CHANGE_ID).unwrap();
    let proposal_v2 = phases
        .iter()
        .rev()
        .find(|row| row.phase == "proposal")
        .unwrap();
    assert_eq!(
        proposal_v2.decision_session_id.as_deref(),
        Some("ses-decision"),
        "amend 经 trait 入口定点最新条目"
    );
    let design = phases.iter().find(|row| row.phase == "design").unwrap();
    assert_eq!(
        design.backtrack_to.as_deref(),
        Some("proposal"),
        "发起相位标记经 trait 入口落库"
    );
    let proposal_v1 = phases.iter().find(|row| row.phase == "proposal").unwrap();
    assert!(
        proposal_v1.stale,
        "目标最新 pass 经 trait 入口置 stale（fail 的第二轮不误伤）"
    );
    assert!(!proposal_v2.stale);
}

// ---------------------------------------------------------------------------
// StoreFault 映射：同 id 冲突 → Conflict；amend 无条目 / set_archived miss →
// NotFound；三分支 Display 前缀与 StoreError 一一对应不串型
// ---------------------------------------------------------------------------

#[test]
fn store_fault映射_同id冲突与未找到分支对应store_error且display前缀不串型() {
    let env = PortEnv::new("fault-map");
    let store = open_workspace_ok(&env.db_path("ws"));
    let port: &dyn ChangeStateStore = &store;

    // 同 id 冲突 → StoreFault::Conflict（name 换串仍按身份锚 id 判重）
    port.create_change_record(change_archive("chg-dup", "甲档", 1000))
        .expect("建档应成功");
    let err = port
        .create_change_record(change_archive("chg-dup", "乙档", 2000))
        .expect_err("同 id 建档应 Conflict");
    assert!(
        matches!(err, StoreFault::Conflict(_)),
        "变体为 Conflict，实际: {err:?}"
    );
    assert!(
        err.to_string().starts_with("conflict:"),
        "错误串以 conflict: 前缀，实际: {err}"
    );

    // amend 无条目 → StoreFault::NotFound
    port.create_change_record(change_archive("chg-young", "young", 3000))
        .expect("建档应成功");
    let err = port
        .amend_decision_session("chg-young", "proposal", "ses-1")
        .expect_err("无条目挂账应 NotFound");
    assert!(matches!(err, StoreFault::NotFound(_)));
    assert!(
        err.to_string().starts_with("not_found:"),
        "错误串以 not_found: 前缀，实际: {err}"
    );

    // set_archived miss → StoreFault::NotFound
    let err = port
        .set_archived("chg-ghost", 1)
        .expect_err("miss 归档翻转应 NotFound");
    assert!(matches!(err, StoreFault::NotFound(_)));

    // 三分支 Display 前缀契约（Db 分支为 store 域故障的收敛形态，前缀单点
    // 断言；Conflict / NotFound 已由上方真件路径驱动）
    assert_eq!(
        StoreFault::Db("io 失败".to_owned()).to_string(),
        "db: io 失败"
    );
    assert_eq!(
        StoreFault::Conflict("重复".to_owned()).to_string(),
        "conflict: 重复"
    );
    assert_eq!(
        StoreFault::NotFound("缺失".to_owned()).to_string(),
        "not_found: 缺失"
    );
}

// ---------------------------------------------------------------------------
// trait 边界：Send + Sync 编译锚定（Arc 装入组合根 / LocalToolSteps 可达）；
// get_change 对未建档 id 返回 Ok(None)（None = 未建档语义——不含文档形态）
// ---------------------------------------------------------------------------

/// port 契约边界编译锚定：`Arc<dyn ChangeStateStore>` 须 Send + Sync。
const fn assert_send_sync<T: Send + Sync>() {}

/// 组合根消费形态（run 发起链跨线程注入的可达性证据）。
fn consume_across_threads(_store: Arc<dyn ChangeStateStore>) {}

#[test]
fn trait边界_send_sync锚定_arc装入与未建档get_change返回ok_none() {
    assert_send_sync::<Arc<dyn ChangeStateStore>>();

    let env = PortEnv::new("boundary");
    let store = open_workspace_ok(&env.db_path("ws"));
    let port: Arc<dyn ChangeStateStore> = Arc::new(store);
    consume_across_threads(port.clone());

    // Arc 擦除后写读可用
    port.create_change_record(change_archive("chg-bound", "boundary", 1000))
        .expect("建档应成功");
    assert_eq!(
        port.get_change("chg-unknown").unwrap(),
        None,
        "get_change 对未建档 id 返回 Ok(None)（未建档语义——不含文档形态）"
    );
    assert_eq!(
        port.get_change("boundary").unwrap(),
        None,
        "name 串非身份键：按 id get_change 同样 Ok(None)"
    );
    assert!(
        port.get_change("chg-bound").unwrap().is_some(),
        "已建档 id 命中 Some"
    );
}

// ---------------------------------------------------------------------------
// 补偿删除面（design D5）：delete_change_record 经 trait 入口命中 Ok(true)、
// miss 幂等 Ok(false)
// ---------------------------------------------------------------------------

#[test]
fn delete_change_record补偿删除_命中true_miss幂等false() {
    let env = PortEnv::new("compensate");
    let store = open_workspace_ok(&env.db_path("ws"));
    let port: &dyn ChangeStateStore = &store;

    port.create_change_record(change_archive("flow", "flow", 1000))
        .expect("建档应成功");
    assert_eq!(
        port.delete_change_record("flow").unwrap(),
        true,
        "命中删除返回 Ok(true)"
    );
    assert_eq!(
        port.get_change("flow").unwrap(),
        None,
        "补偿删除后建档行消失"
    );
    assert_eq!(
        port.delete_change_record("flow").unwrap(),
        false,
        "重复删除幂等 Ok(false)"
    );
    assert_eq!(
        port.delete_change_record("从未存在").unwrap(),
        false,
        "miss 幂等 Ok(false)"
    );
}

// ---------------------------------------------------------------------------
// 双字段 trait 往返（design D7 / AC-1 中性映射半边的 trait 面）
// ---------------------------------------------------------------------------

/// 双字段 trait 往返：经 `&dyn ChangeStateStore` 类型擦除建档（携 Some 两字
/// 段）→ get / list 中性快照逐字段一致（映射单点无加工——worktree /
/// base_commit 不在 port 面丢失）。
#[test]
fn 双字段trait往返_类型擦除建档get与list逐字段一致() {
    let env = PortEnv::new("worktree-fields");
    let store = open_workspace_ok(&env.db_path("ws"));
    let port: &dyn ChangeStateStore = &store;

    let mut record = change_archive("wt-port-change", "wt-port-change", 1000);
    record.worktree = Some(r"C:\app-data\worktrees\seg\wt-port-change".to_owned());
    record.base_commit = Some("0000000000000000000000000000000000000001".to_owned());
    port.create_change_record(record)
        .expect("trait 建档（携两字段）应成功");

    // get 半边：中性快照逐字段一致
    let got = port
        .get_change("wt-port-change")
        .expect("trait 查档应成功")
        .expect("建档在案");
    assert_eq!(
        got.worktree.as_deref(),
        Some(r"C:\app-data\worktrees\seg\wt-port-change"),
        "worktree 不在 port 映射面丢失（get）"
    );
    assert_eq!(
        got.base_commit.as_deref(),
        Some("0000000000000000000000000000000000000001"),
        "base_commit 不在 port 映射面丢失（get）"
    );

    // list 半边：同往返
    let records = port.list_change_records().expect("trait 清单应成功");
    let listed = records
        .iter()
        .find(|record| record.name == "wt-port-change")
        .expect("清单应含建档行");
    assert_eq!(
        listed.worktree, got.worktree,
        "worktree trait 往返一致（list）"
    );
    assert_eq!(
        listed.base_commit, got.base_commit,
        "base_commit trait 往返一致（list）"
    );
}

// ---------------------------------------------------------------------------
// run 域 port 委托（unify-run-state-persistence）：list_runs / list_run_steps /
// run_start / run_finish 四方法经 trait 面 = 直调操作面
// ---------------------------------------------------------------------------

use workflow::state::{
    RunFinishCommand, RunStartCommand, RunStatus, RunStepEntry, RunStepKind, RunStepStatus,
};

/// run 发起命令 fixture。
fn run_start_cmd(run_id: &str, change: &str, started_at: i64) -> RunStartCommand {
    RunStartCommand {
        run_id: run_id.to_owned(),
        change_id: change.to_owned(),
        started_at,
    }
}

/// run 收口整包条目 fixture。
fn run_step_entry(seq: u64, step: RunStepKind) -> RunStepEntry {
    RunStepEntry {
        seq,
        phase: "implement".to_owned(),
        attempt: 1,
        step,
        status: RunStepStatus::Passed,
        session_id: Some("ses-1".to_owned()),
        detail: None,
    }
}

/// run 收口命令 fixture。
fn run_finish_cmd(
    run_id: &str,
    change: &str,
    finished_at: i64,
    steps: Vec<RunStepEntry>,
) -> RunFinishCommand {
    RunFinishCommand {
        run_id: run_id.to_owned(),
        change_id: change.to_owned(),
        status: RunStatus::Completed,
        reason: Some("收口记因".to_owned()),
        finished_at,
        steps,
    }
}

/// 种子（start + finish）后经 port 面读回与直调操作面等值（AC-1 port 面）：
/// list_runs / list_run_steps 逐字段一致。
#[test]
fn run域port委托_读回与直调操作面等值() {
    let env = PortEnv::new("run-port");
    let store = open_workspace_ok(&env.db_path("ws"));
    store
        .create_change_record(change_archive("port-run", "port-run", 1000))
        .expect("建档应成功");

    let port: Arc<dyn ChangeStateStore> = Arc::new(store);
    port.run_start(&run_start_cmd("run-100", "port-run", 2000))
        .expect("port run_start 应成功");
    port.run_finish(&run_finish_cmd(
        "run-100",
        "port-run",
        3000,
        vec![
            run_step_entry(0, RunStepKind::Executor),
            run_step_entry(2, RunStepKind::StaticCheck),
        ],
    ))
    .expect("port run_finish 应成功");

    // port 读面逐字段回读
    let runs = port.list_runs("port-run").expect("port list_runs 应成功");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_id, "run-100");
    assert_eq!(runs[0].status, RunStatus::Completed);
    assert_eq!(runs[0].reason.as_deref(), Some("收口记因"));
    assert_eq!(runs[0].started_at, 2000);
    assert_eq!(runs[0].finished_at, Some(3000));

    let steps = port
        .list_run_steps("run-100")
        .expect("port list_run_steps 应成功");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].seq, 0);
    assert_eq!(steps[0].step, RunStepKind::Executor);
    assert_eq!(steps[1].seq, 2);
    assert_eq!(steps[1].step, RunStepKind::StaticCheck);

    // port 面 = 直调操作面（同一 store 实例双入口等值——trait 委托零加工）
}

/// Conflict（同 run_id 重复 start）与 NotFound（未建档 finish）经 port 面映
/// 射为 StoreFault::Conflict / NotFound，Display 记因语境保留。
#[test]
fn run域port委托_fault映射_conflict与notfound() {
    let env = PortEnv::new("run-port-fault");
    let store = open_workspace_ok(&env.db_path("ws"));
    store
        .create_change_record(change_archive("port-run", "port-run", 1000))
        .expect("建档应成功");
    let port: Arc<dyn ChangeStateStore> = Arc::new(store);

    port.run_start(&run_start_cmd("run-100", "port-run", 2000))
        .expect("首次发起应成功");
    let fault = port
        .run_start(&run_start_cmd("run-100", "port-run", 3000))
        .expect_err("同 run_id 重复发起应 Err");
    match &fault {
        StoreFault::Conflict(message) => {
            assert!(
                message.contains("run-100"),
                "Conflict 记因语境保留: {message}"
            );
        }
        other => panic!("应为 Conflict 变体: {other:?}"),
    }
    assert!(fault.to_string().contains("conflict"), "Display 前缀不串型");

    let fault = port
        .run_finish(&run_finish_cmd("run-ghost", "port-run", 3000, Vec::new()))
        .expect_err("未建档收口应 Err");
    assert!(
        matches!(fault, StoreFault::NotFound(_)),
        "NotFound 变体: {fault:?}"
    );
    assert!(
        fault.to_string().contains("not_found"),
        "Display 前缀不串型"
    );
}

/// 空库形态（边界）：list_runs 空数组、list_run_steps 空数组（miss 非错误经
/// port 面不变）。
#[test]
fn run域port委托_空库miss空数组() {
    let env = PortEnv::new("run-port-miss");
    let store = open_workspace_ok(&env.db_path("ws"));
    let port: Arc<dyn ChangeStateStore> = Arc::new(store);

    assert!(
        port.list_runs("ghost-change")
            .expect("port list_runs 应成功")
            .is_empty(),
        "空库 list_runs → 空数组"
    );
    assert!(
        port.list_run_steps("run-ghost")
            .expect("port list_run_steps 应成功")
            .is_empty(),
        "空库 list_run_steps → 空数组"
    );
}
