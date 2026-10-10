//! `write::decision_log` 的单元测试（test-design「decision_log.rs ->
//! decision_log_test.rs」节）：决策会话槽位挂账写操作（amend 语义，形参
//! change **id**）——经 [`ChangeStateStore::amend_decision_session`] 对该相位
//! 最新条目定点改写 `decision_session_id`；幂等覆写（D9）；无条目
//! `StoreFault::NotFound` 显式 `Err`；`StoreFault` 故障传播；不做表位校验、
//! 不新增条目语义保持。id 与 name 字面量各异：寻址断言以 id 为键、name 仅
//! 展示属性。
//!
//! Mock策略（test-design 本节 Mock 表）：进程内假件实现 trait（可编程条目
//! 序列与故障）；定点语义证据归 store_test 真件节。

use std::sync::Mutex;

use super::decision_log::{decision_log, DecisionLogOutcome};
use crate::model::Verdict;
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, PhaseLogCommand, PhaseStateRecord,
    RunFinishCommand, RunStartCommand, RunStateRecord, RunStepStateRecord, StepCommand,
    StepStateRecord, StoreFault,
};

/// 身份锚字面量（挂账入参——一切寻址以 id 为准；name 零寻址职能，本写面
/// 不读记录）。
const CHANGE_ID: &str = "0198f7a0-0000-7000-8000-0000000000e4";

/// 确定性时间戳基（UTC unix millis）。
const T0: i64 = 1_727_000_000_000;
fn t(n: u32) -> i64 {
    T0 + i64::from(n) * 1000
}

// ---------------------------------------------------------------------------
// 假件 store：amend_decision_session 捕获调用并镜像真件定点语义（该相位
// timestamp 最新条目改写 decision 槽位；无条目 NotFound）；可编程故障。
// ---------------------------------------------------------------------------

struct AmendStore {
    entries: Mutex<Vec<PhaseStateRecord>>,
    /// 捕获的挂账调用（change, phase, session_id）。
    calls: Mutex<Vec<(String, String, String)>>,
    fault: Mutex<Option<StoreFault>>,
}

impl AmendStore {
    fn with_entries(entries: Vec<PhaseStateRecord>) -> Self {
        Self {
            entries: Mutex::new(entries),
            calls: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn set_fault(&self, fault: StoreFault) {
        *self.fault.lock().expect("故障锁不可中毒") = Some(fault);
    }

    fn call_count(&self) -> usize {
        self.calls.lock().expect("调用锁不可中毒").len()
    }

    fn log(&self, phase: &str, session_id: &str) -> Result<DecisionLogOutcome, String> {
        decision_log(self, CHANGE_ID, phase, session_id)
    }

    /// 捕获的挂账调用序列（change_id / phase / session_id 逐字段断言面）。
    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().expect("调用锁不可中毒").clone()
    }
}

impl ChangeStateStore for AmendStore {
    fn get_change(&self, _id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_phase_records(&self, _change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        Ok(self.entries.lock().expect("条目锁不可中毒").clone())
    }

    fn list_steps(
        &self,
        _change_id: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn create_change_record(&self, _record: ChangeStateRecord) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn delete_change_record(&self, _id: &str) -> Result<bool, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn start_phase(
        &self,
        _change_id: &str,
        _phase: &str,
        _now: i64,
    ) -> Result<crate::state::PhaseStartState, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn log_phase(&self, _command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn apply_backtrack(&self, _command: &BacktrackCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn amend_decision_session(
        &self,
        change_id: &str,
        phase: &str,
        session_id: &str,
    ) -> Result<(), StoreFault> {
        self.calls.lock().expect("调用锁不可中毒").push((
            change_id.to_owned(),
            phase.to_owned(),
            session_id.to_owned(),
        ));
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        let mut entries = self.entries.lock().expect("条目锁不可中毒");
        let latest = entries
            .iter_mut()
            .filter(|entry| entry.phase == phase)
            .max_by_key(|entry| (entry.timestamp, entry.id))
            .ok_or_else(|| {
                StoreFault::NotFound(format!("Phase \"{phase}\" 没有评估条目，无法挂账决策会话"))
            })?;
        latest.decision_session_id = Some(session_id.to_owned());
        Ok(())
    }

    fn set_archived(&self, _id: &str, _archived_at: i64) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn append_step(&self, _command: &StepCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_runs(&self, _change: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_run_steps(&self, _run_id: &str) -> Result<Vec<RunStepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_start(&self, _command: &RunStartCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_finish(&self, _command: &RunFinishCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }
}

fn entry(phase: &str, attempt: u32, verdict: Verdict, ts: i64) -> PhaseStateRecord {
    PhaseStateRecord {
        id: i64::from(attempt),
        change_id: CHANGE_ID.to_owned(),
        phase: phase.to_owned(),
        attempt,
        verdict,
        report: format!("{phase} 报告"),
        checklist: Vec::new(),
        skipped: false,
        stale: false,
        backtrack_to: None,
        backtrack_reason: None,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
        start_at: None,
        timestamp: ts,
    }
}

// ---------------------------------------------------------------------------
// 正向：最新条目定点改写
// ---------------------------------------------------------------------------

/// 该相位最新 PhaseRecord decision 槽位改写为 session_id（多 attempt 在场时
/// 仅最新条目被改——定点锚定语义保持；跨相位不串锚）。
#[test]
fn amend定点改写_多attempt仅最新条目被改() {
    let fake = AmendStore::with_entries(vec![
        entry("dev-design", 1, Verdict::Fail, t(1)),
        entry("dev-design", 2, Verdict::Fail, t(2)),
        entry("proposal", 1, Verdict::Pass, t(3)), // 全局最新、他相位
    ]);

    let outcome = fake
        .log("dev-design", "ses-decision-1")
        .expect("挂账应成功");

    assert_eq!(
        outcome,
        DecisionLogOutcome {
            phase: "dev-design".to_owned(),
        },
        "outcome {{ phase }} 逐字段（沿 PhaseLogOutcome / BacktrackOutcome 惯例）"
    );
    assert_eq!(fake.call_count(), 1, "恰一次 amend 调用");
    assert_eq!(
        fake.calls()[0].0,
        CHANGE_ID,
        "amend_decision_session 收到本次 change id（形参 id 化，非 name）"
    );

    let entries = fake
        .list_phase_records(CHANGE_ID)
        .expect("假件读半边应可用");
    assert_eq!(
        entries[1].decision_session_id.as_deref(),
        Some("ses-decision-1"),
        "该相位 timestamp 最新条目（attempt 2）定点改写"
    );
    assert_eq!(
        entries[0].decision_session_id, None,
        "更早 attempt 条目零触碰"
    );
    assert_eq!(
        entries[2].decision_session_id, None,
        "他相位条目零触碰（跨相位不串锚）"
    );
    assert_eq!(entries.len(), 3, "挂账不新增条目（amend 语义）");
}

/// 重复挂账同 session_id → 幂等覆写不报错、值不重复追加（D9）。
#[test]
fn 幂等同值二连挂账值不追加() {
    let fake = AmendStore::with_entries(vec![entry("implement", 1, Verdict::Fail, t(1))]);

    fake.log("implement", "ses-decision-9")
        .expect("首次挂账应成功");
    fake.log("implement", "ses-decision-9")
        .expect("同值重挂应成功");

    let entries = fake
        .list_phase_records(CHANGE_ID)
        .expect("假件读半边应可用");
    assert_eq!(entries.len(), 1, "不新增条目");
    assert_eq!(
        entries[0].decision_session_id.as_deref(),
        Some("ses-decision-9"),
        "同值幂等覆写（单值，非追加）"
    );
}

/// 挂账不新增条目语义保持：fresh 条目序列长度与字段在挂账前后一致。
#[test]
fn 挂账不新增条目且其余字段零触碰() {
    let mut seeded = entry("code-review", 1, Verdict::Fail, t(1));
    seeded.stale = true;
    seeded.backtrack_to = Some("implement".to_owned());
    let fake = AmendStore::with_entries(vec![seeded]);

    fake.log("code-review", "ses-decision-2")
        .expect("挂账应成功");

    let entries = fake
        .list_phase_records(CHANGE_ID)
        .expect("假件读半边应可用");
    let after = &entries[0];
    assert_eq!(after.decision_session_id.as_deref(), Some("ses-decision-2"));
    assert!(after.stale, "stale 标记原样保留（只改槽位列）");
    assert_eq!(
        after.backtrack_to.as_deref(),
        Some("implement"),
        "回跳键零触碰"
    );
    assert_eq!(after.report, "code-review 报告", "其余字段零触碰");
}

// ---------------------------------------------------------------------------
// 异常：无条目 / 故障传播 / 不做表位校验
// ---------------------------------------------------------------------------

/// 该相位无任何条目 → `Err` 显式（`StoreFault::NotFound` 面）。
#[test]
fn 该相位无条目err显式() {
    let fake = AmendStore::with_entries(vec![entry("proposal", 1, Verdict::Pass, t(1))]);

    let err = fake
        .log("dev-design", "ses-decision-1")
        .expect_err("无条目必须 Err");

    assert!(
        err.contains("not_found") && err.contains("dev-design"),
        "错误显式携带相位与 NotFound 记因，实际: {err}"
    );
}

/// 假件 store 注入 `StoreFault` → `Err` 传播；调用已捕获（不静默吞）。
#[test]
fn store故障传播err记因() {
    let fake = AmendStore::with_entries(vec![entry("implement", 1, Verdict::Fail, t(1))]);
    fake.set_fault(StoreFault::Db("注入的挂账故障".to_owned()));

    let err = fake
        .log("implement", "ses-decision-3")
        .expect_err("StoreFault 应传播");

    assert!(
        err.contains("db:") && err.contains("注入的挂账故障"),
        "Err 记因携带 fault 语境，实际: {err}"
    );
    assert_eq!(fake.call_count(), 1, "调用已捕获（不静默吞）");
    let entries = fake
        .list_phase_records(CHANGE_ID)
        .expect("假件读半边应可用");
    assert_eq!(
        entries[0].decision_session_id, None,
        "故障路径不新增条目不改写槽位"
    );
}

/// 不做表位校验语义保持：表外相位名照常透传挂账（有条目即改写）。
#[test]
fn 不做表位校验_表外相位名照常透传() {
    let fake = AmendStore::with_entries(vec![entry("幽灵相位", 1, Verdict::Fail, t(1))]);

    let outcome = fake
        .log("幽灵相位", "ses-decision-4")
        .expect("表外相位应照常挂账");

    assert_eq!(outcome.phase, "幽灵相位");
    let entries = fake
        .list_phase_records(CHANGE_ID)
        .expect("假件读半边应可用");
    assert_eq!(
        entries[0].decision_session_id.as_deref(),
        Some("ses-decision-4"),
        "decision_log 不做表位校验（挂账单点语义）"
    );
}
