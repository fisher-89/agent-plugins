//! `write::phase_start` 的单元测试（test-design「phase_start.rs ->
//! phase_start_test.rs」节）：开相写操作 port 落库——表位校验保留在 core（校验
//! 前置零写命令）、持久化经 [`ChangeStateStore::start_phase`]（attempt 写事务
//! 内推导）、`PhaseStartOutcome.start_at` 出 i64 millis、StoreFault 故障传播、
//! 重开 attempt 自 db 推导递增。
//!
//! Mock策略（test-design 本节 Mock 表）：全部用例走进程内假件实现 trait
//!（记录写命令与调用序、可编程 `StoreFault`）；假件 start_phase 以「该相位
//! 既有条目数 + 1」推导 attempt 并落 active_phase（镜像真件事务内推导语义）。

use std::sync::Mutex;

use super::phase_start::phase_start;
use crate::state::{
    ActivePhaseState, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, PhaseStateRecord, RunFinishCommand, RunStartCommand, RunStateRecord,
    RunStepStateRecord, StepCommand, StepStateRecord, StoreFault,
};

const CHANGE: &str = "demo-change";

/// 确定性时间戳基（UTC unix millis）。
const T0: i64 = 1_727_000_000_000;

// ---------------------------------------------------------------------------
// 假件 store：get_change 只读 + start_phase 捕获调用并镜像真件语义
//（attempt = 该相位既有条目数 + 1、active_phase 定点写入）；其余写半边
// unimplemented（越权触达即 panic）。
// ---------------------------------------------------------------------------

struct StartStore {
    record: Mutex<Option<ChangeStateRecord>>,
    entries: Mutex<Vec<PhaseStateRecord>>,
    /// 捕获的 start_phase 调用（change, phase, now）。
    start_calls: Mutex<Vec<(String, String, i64)>>,
    fault: Mutex<Option<StoreFault>>,
}

impl StartStore {
    fn requirement() -> Self {
        Self::with_workflow_type("requirement")
    }

    fn with_workflow_type(workflow_type: &str) -> Self {
        Self {
            record: Mutex::new(Some(ChangeStateRecord {
                name: CHANGE.to_owned(),
                workflow_type: workflow_type.to_owned(),
                created_at: T0,
                status: ChangeStatus::Active,
                archived_at: None,
                active_phase: None,
                worktree: None,
                base_commit: None,
            })),
            entries: Mutex::new(Vec::new()),
            start_calls: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn missing() -> Self {
        Self {
            record: Mutex::new(None),
            entries: Mutex::new(Vec::new()),
            start_calls: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn set_fault(&self, fault: StoreFault) {
        *self.fault.lock().expect("故障锁不可中毒") = Some(fault);
    }

    /// 模拟 run 期间落账：追加条目 + 清位 active_phase（重开 attempt 用例的
    /// 节奏步，沿 start → 落账 → 再 start 的真实重试节奏）。
    fn log_entry(&self, phase: &str, ts: i64) {
        let mut entries = self.entries.lock().expect("条目锁不可中毒");
        let attempt = entries.iter().filter(|entry| entry.phase == phase).count() as u32 + 1;
        entries.push(PhaseStateRecord {
            id: i64::from(attempt),
            change: CHANGE.to_owned(),
            phase: phase.to_owned(),
            attempt,
            verdict: crate::model::Verdict::Fail,
            report: "首轮未过".to_owned(),
            checklist: Vec::new(),
            skipped: false,
            stale: false,
            backtrack_to: None,
            backtrack_reason: None,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
            start_at: Some(ts),
            timestamp: ts,
        });
        drop(entries);
        if let Some(record) = self.record.lock().expect("记录锁不可中毒").as_mut() {
            record.active_phase = None;
        }
    }

    fn start_call_count(&self) -> usize {
        self.start_calls.lock().expect("调用锁不可中毒").len()
    }

    fn start(&self, phase: &str) -> Result<super::phase_start::PhaseStartOutcome, String> {
        phase_start(self, CHANGE, phase)
    }
}

impl ChangeStateStore for StartStore {
    fn get_change(&self, _name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self.record.lock().expect("记录锁不可中毒").clone())
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_phase_records(&self, _change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        Ok(self.entries.lock().expect("条目锁不可中毒").clone())
    }

    fn list_steps(
        &self,
        _change: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn create_change_record(&self, _record: ChangeStateRecord) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn delete_change_record(&self, _name: &str) -> Result<bool, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn start_phase(
        &self,
        change: &str,
        phase: &str,
        now: i64,
    ) -> Result<PhaseStartState, StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        let attempt = self
            .entries
            .lock()
            .expect("条目锁不可中毒")
            .iter()
            .filter(|entry| entry.phase == phase)
            .count() as u32
            + 1;
        {
            let mut guard = self.record.lock().expect("记录锁不可中毒");
            let record = guard.as_mut().expect("建档记录应在场");
            record.active_phase = Some(ActivePhaseState {
                phase: phase.to_owned(),
                attempt,
                start_at: now,
            });
        }
        self.start_calls.lock().expect("调用锁不可中毒").push((
            change.to_owned(),
            phase.to_owned(),
            now,
        ));
        Ok(PhaseStartState {
            attempt,
            start_at: now,
        })
    }

    fn log_phase(&self, _command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn apply_backtrack(&self, _command: &crate::state::BacktrackCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn amend_decision_session(
        &self,
        _change: &str,
        _phase: &str,
        _session_id: &str,
    ) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn set_archived(&self, _name: &str, _archived_at: i64) -> Result<(), StoreFault> {
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

/// 当前 UTC unix 毫秒（start_at 上界断言用；不与 wall-clock 比等值）。
fn now_upper() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// 正向：开相落库（outcome 与 active_phase 一致）
// ---------------------------------------------------------------------------

/// 合法表位开相 → `store.start_phase` 落 active_phase；outcome.phase /
/// attempt / start_at（i64）与假件落库的 active_phase 逐字段一致。
#[test]
fn 开相落库_outcome与active_phase逐字段一致() {
    let fake = StartStore::requirement();
    let before = now_upper();

    let outcome = fake.start("proposal").expect("开启阶段应成功");

    let after = now_upper();
    assert_eq!(outcome.phase, "proposal");
    assert_eq!(outcome.attempt, 1, "无既有条目 → attempt 1");
    assert!(
        outcome.start_at >= before && outcome.start_at <= after,
        "start_at 为写事务内铸出的 i64 millis（边界内），实际: {}",
        outcome.start_at
    );

    // 假件落库侧：active_phase 与 outcome 一致、调用恰一次
    assert_eq!(fake.start_call_count(), 1, "start_phase 恰一次落库调用");
    let record = fake
        .get_change(CHANGE)
        .expect("假件读半边应可用")
        .expect("建档记录应在场");
    let active = record.active_phase.expect("active_phase 应已写入");
    assert_eq!(active.phase, "proposal");
    assert_eq!(active.attempt, 1);
    assert_eq!(active.start_at, outcome.start_at, "outcome 与落库一致");
}

// ---------------------------------------------------------------------------
// 异常：表位校验保留（零写入）/ 故障传播
// ---------------------------------------------------------------------------

/// 相位不在相位表 → `Err` 且 store 零写入（校验前置——假件记录零写命令）。
#[test]
fn 非法相位拒绝且store零写入() {
    let fake = StartStore::requirement();

    let err = fake.start("不存在的相位").expect_err("非法相位应 Err");

    assert!(
        err.contains("不存在的相位") && err.contains("requirement"),
        "错误显式携带相位与 workflow_type 语境，实际: {err}"
    );
    assert_eq!(fake.start_call_count(), 0, "校验前置：零 start_phase 调用");
    let record = fake
        .get_change(CHANGE)
        .expect("假件读半边应可用")
        .expect("建档记录应在场");
    assert!(record.active_phase.is_none(), "active_phase 未被写入");
}

/// workflow_type 非 requirement → Err 且零写入（W8 分层出口）。
#[test]
fn workflow_type非requirement拒绝零写入() {
    let fake = StartStore::with_workflow_type("test-only");

    let err = fake.start("proposal").expect_err("非 requirement 应 Err");

    assert!(err.contains("test-only"), "W8 分层出口记因，实际: {err}");
    assert_eq!(fake.start_call_count(), 0, "零写入");
}

/// change 未建档 → Err 显式（假件 get_change 返回 None）。
#[test]
fn change未建档显式err() {
    let fake = StartStore::missing();

    let err = fake.start("proposal").expect_err("未建档应 Err");

    assert!(err.contains("未建档"), "Err 显式记因，实际: {err}");
    assert_eq!(fake.start_call_count(), 0, "零写入");
}

/// 假件 store 注入 `StoreFault` → `Err` 记因传播不静默。
#[test]
fn store故障传播记因不静默() {
    let fake = StartStore::requirement();
    fake.set_fault(StoreFault::Db("注入的开相写故障".to_owned()));

    let err = fake.start("proposal").expect_err("StoreFault 应传播");

    assert!(
        err.contains("db:") && err.contains("注入的开相写故障"),
        "Err 记因携带 fault 语境，实际: {err}"
    );
}

// ---------------------------------------------------------------------------
// 边界：重开 attempt（自 db 推导递增）
// ---------------------------------------------------------------------------

/// start → 落账清位 → 再 start 同相位 → attempt=2（自既有条目数推导）；
/// start_at 出 i64 millis 直透（重入即新一轮计时）。
#[test]
fn 重开attempt自既有条目数推导递增() {
    let fake = StartStore::requirement();

    let first = fake.start("implement").expect("首次 start 应成功");
    assert_eq!(first.attempt, 1);

    // 模拟 run 期间 phase-log 落账一条 fail（清位 + 条目追加）
    fake.log_entry("implement", T0);

    let before = now_upper();
    let second = fake.start("implement").expect("重入 start 应成功");
    let after = now_upper();

    assert_eq!(second.attempt, 2, "attempt = 该相位既有条目数 1 + 1");
    assert!(
        second.start_at >= before && second.start_at <= after,
        "start_at 刷新（重入即新一轮计时，i64 millis 直透），实际: {}",
        second.start_at
    );
    assert!(
        second.start_at >= first.start_at,
        "重入 start_at 不早于首次"
    );
    // 落库侧 active_phase 同步为 attempt 2
    let active = fake
        .get_change(CHANGE)
        .expect("假件读半边应可用")
        .expect("建档记录应在场")
        .active_phase
        .expect("active_phase 应在位");
    assert_eq!(active.attempt, 2);
    assert_eq!(active.phase, "implement");
}
