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

/// 建档命令 fixture（active 起步）。
fn change_archive(name: &str, created_at: i64) -> ChangeStateRecord {
    ChangeStateRecord {
        name: name.to_owned(),
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
        change: change.to_owned(),
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
        change: change.to_owned(),
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

    // 写半边全链经 trait 入口
    port.create_change_record(change_archive("flow", 1000))
        .expect("建档应成功");
    let started = port
        .start_phase("flow", "proposal", 2000)
        .expect("开相应成功");
    let attempt = port
        .log_phase(&log_command(
            "flow",
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
        port.get_change("flow").unwrap(),
        store.find_change_record("flow").unwrap(),
        "建档快照映射无加工"
    );
    assert_eq!(
        port.list_change_records().unwrap(),
        store.list_change_records().unwrap(),
        "建档清单映射无加工"
    );
    let via_port = port.list_phase_records("flow").unwrap();
    let via_store = store.list_phase_records("flow").unwrap();
    assert_eq!(via_port, via_store, "相位史快照映射无加工");
    assert_eq!(via_port.len(), 1);
    assert_eq!(via_port[0].attempt, attempt);
    assert_eq!(via_port[0].start_at, Some(2000), "start_at 随行透出");
    assert_eq!(
        via_port[0].checklist,
        vec![check_item("检查项一", true), check_item("检查项二", false)],
        "checklist 内联重组随映射透出"
    );
    // 落账清位经 trait 快照可见（active_phase 匹配 / 步骤审计读半边同验）
    assert_eq!(
        port.get_change("flow").unwrap().unwrap().active_phase,
        None,
        "落账即清位经 trait 可见"
    );
    assert_eq!(
        port.list_steps("flow", None).unwrap(),
        store.list_change_steps("flow", None).unwrap(),
        "步骤审计读半边映射无加工"
    );
}

// ---------------------------------------------------------------------------
// 写命令翻译：PhaseLogCommand / BacktrackCommand / StepCommand 经 trait 入口
// 落库，与 Store 原生方法直调落库行逐字段等值（两库镜像序列对拍）
// ---------------------------------------------------------------------------

/// 镜像写序列（change 域全写操作各一次；两侧分别经 trait 入口与原生直调执行
/// 同一命令面）。
fn mirror_sequence_port(port: &dyn ChangeStateStore) {
    port.create_change_record(change_archive("flow", 1000))
        .expect("建档应成功");
    port.start_phase("flow", "proposal", 2000)
        .expect("开相应成功");
    port.log_phase(&log_command(
        "flow",
        "proposal",
        Verdict::Pass,
        vec![check_item("检查项一", true), check_item("检查项二", false)],
        Some(2000),
        2500,
    ))
    .expect("落账应成功");
    port.start_phase("flow", "proposal", 3000)
        .expect("重开相应成功");
    port.log_phase(&log_command(
        "flow",
        "proposal",
        Verdict::Fail,
        vec![check_item("检查项三", false)],
        None,
        4000,
    ))
    .expect("落账应成功");
    port.start_phase("flow", "design", 5000)
        .expect("开相应成功");
    port.log_phase(&log_command(
        "flow",
        "design",
        Verdict::Pass,
        vec![],
        None,
        6000,
    ))
    .expect("落账应成功");
    port.amend_decision_session("flow", "proposal", "ses-decision")
        .expect("挂账应成功");
    port.apply_backtrack(&BacktrackCommand {
        change: "flow".to_owned(),
        phase: "design".to_owned(),
        to: "proposal".to_owned(),
        reason: "镜像回跳".to_owned(),
        stale_dependents: vec![],
    })
    .expect("回跳应成功");
    port.append_step(&step_command(
        "run-1",
        "flow",
        StepKind::PhaseNext,
        "ok",
        "轮次推进",
        7000,
    ))
    .expect("步骤追加应成功");
    port.append_step(&step_command(
        "run-2",
        "flow",
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
        .create_change_record(change_archive("flow", 1000))
        .expect("建档应成功");
    store
        .start_change_phase("flow", "proposal", 2000)
        .expect("开相应成功");
    store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Pass,
            vec![check_item("检查项一", true), check_item("检查项二", false)],
            Some(2000),
            2500,
        ))
        .expect("落账应成功");
    store
        .start_change_phase("flow", "proposal", 3000)
        .expect("重开相应成功");
    store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Fail,
            vec![check_item("检查项三", false)],
            None,
            4000,
        ))
        .expect("落账应成功");
    store
        .start_change_phase("flow", "design", 5000)
        .expect("开相应成功");
    store
        .log_change_phase(&log_command(
            "flow",
            "design",
            Verdict::Pass,
            vec![],
            None,
            6000,
        ))
        .expect("落账应成功");
    store
        .amend_change_decision_session("flow", "proposal", "ses-decision")
        .expect("挂账应成功");
    store
        .apply_change_backtrack(&BacktrackCommand {
            change: "flow".to_owned(),
            phase: "design".to_owned(),
            to: "proposal".to_owned(),
            reason: "镜像回跳".to_owned(),
            stale_dependents: vec![],
        })
        .expect("回跳应成功");
    store
        .append_change_step(&step_command(
            "run-1",
            "flow",
            StepKind::PhaseNext,
            "ok",
            "轮次推进",
            7000,
        ))
        .expect("步骤追加应成功");
    store
        .append_change_step(&step_command(
            "run-2",
            "flow",
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
        store_via_port.get_change("flow").unwrap(),
        store_native.find_change_record("flow").unwrap(),
        "PhaseLogCommand 翻译落库行等值（active_phase 终态含回跳面）"
    );
    assert_eq!(
        store_via_port.list_change_records().unwrap(),
        store_native.list_change_records().unwrap(),
    );
    assert_eq!(
        store_via_port.list_phase_records("flow").unwrap(),
        store_native.list_phase_records("flow").unwrap(),
        "落账 / 挂账 / 回跳翻译落库行逐字段等值（含 checklist 内联与 stale 位）"
    );
    assert_eq!(
        store_via_port.list_steps("flow", None).unwrap(),
        store_native.list_change_steps("flow", None).unwrap(),
        "StepCommand 翻译落库行等值"
    );
    assert_eq!(
        store_via_port.list_steps("flow", Some("run-1")).unwrap(),
        store_native
            .list_change_steps("flow", Some("run-1"))
            .unwrap(),
    );

    // 翻译语义抽验：amend 定点最新条目、backtrack 标记落发起相位且目标最新
    // pass 置 stale
    let phases = store_via_port.list_phase_records("flow").unwrap();
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
// StoreFault 映射：同名 active 冲突 → Conflict；amend 无条目 / set_archived
// miss → NotFound；三分支 Display 前缀与 StoreError 一一对应不串型
// ---------------------------------------------------------------------------

#[test]
fn store_fault映射_冲突与未找到分支对应store_error且display前缀不串型() {
    let env = PortEnv::new("fault-map");
    let store = open_workspace_ok(&env.db_path("ws"));
    let port: &dyn ChangeStateStore = &store;

    // 同名 active 冲突 → StoreFault::Conflict
    port.create_change_record(change_archive("flow", 1000))
        .expect("建档应成功");
    let err = port
        .create_change_record(change_archive("flow", 2000))
        .expect_err("同名建档应 Conflict");
    assert!(
        matches!(err, StoreFault::Conflict(_)),
        "变体为 Conflict，实际: {err:?}"
    );
    assert!(
        err.to_string().starts_with("conflict:"),
        "错误串以 conflict: 前缀，实际: {err}"
    );

    // amend 无条目 → StoreFault::NotFound
    port.create_change_record(change_archive("young", 3000))
        .expect("建档应成功");
    let err = port
        .amend_decision_session("young", "proposal", "ses-1")
        .expect_err("无条目挂账应 NotFound");
    assert!(matches!(err, StoreFault::NotFound(_)));
    assert!(
        err.to_string().starts_with("not_found:"),
        "错误串以 not_found: 前缀，实际: {err}"
    );

    // set_archived miss → StoreFault::NotFound
    let err = port
        .set_archived("ghost", 1)
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
// get_change 对未建档名返回 Ok(None)（None = 文档形态契约）
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
    port.create_change_record(change_archive("flow", 1000))
        .expect("建档应成功");
    assert_eq!(
        port.get_change("未建档").unwrap(),
        None,
        "get_change 对未建档名返回 Ok(None)（文档形态契约）"
    );
    assert!(
        port.get_change("flow").unwrap().is_some(),
        "已建档名命中 Some"
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

    port.create_change_record(change_archive("flow", 1000))
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

    let mut record = change_archive("wt-port-change", 1000);
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
