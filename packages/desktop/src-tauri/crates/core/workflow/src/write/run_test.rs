//! `write::run_start` / `write::run_finish` 的单元测试（test-design
//! 「write/run.rs -> run_test.rs」节）：run 写面校验前置（run_id 非空 + change
//! 建档在案 + 终态三值且非 interrupted）保留——任一失败零写命令下发；委派透传
//! （命令逐字段等值，写面零改写——D2 词汇本体 = 命令类型本身）；真件 tempfile
//! workspace 库回环（running 行回读 / 三终态回读——AC-1/AC-2 第一写与收口写）。
//!
//! Mock策略（test-design 本节 Mock 表）：ChangeStateStore 进程内假件（Mutex
//! 捕获 run_start / run_finish 命令、可编程 Err、越权读面 panic）作被测函数
//! 显式入参注入（入参例外）；db 文件（进程边界）真实 workspace 库半边归
//! corpus_golden_test.rs 真件节（集成测试单实例解析，类型面自洽）。

use std::sync::Mutex;

use super::run::{run_finish, run_start};
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, RunFinishCommand, RunStartCommand, RunStateRecord, RunStatus, RunStepEntry,
    RunStepKind, RunStepStateRecord, RunStepStatus, StepCommand, StoreFault,
};

const CHANGE: &str = "demo-change";
const RUN_ID: &str = "run-1727000000000";
const STARTED_AT: i64 = 1_727_000_000_000;
const FINISHED_AT: i64 = 1_727_000_060_000;

// ---------------------------------------------------------------------------
// 假件 store：run 写命令捕获 + 可编程 Err（越权读面 unimplemented 即暴露）
// ---------------------------------------------------------------------------

struct RecordingStore {
    recorded: Mutex<Option<ChangeStateRecord>>,
    started: Mutex<Vec<RunStartCommand>>,
    finished: Mutex<Vec<RunFinishCommand>>,
    /// 可编程 store 半边 Err（run_start / run_finish 命中即 Err）。
    fault: Mutex<Option<StoreFault>>,
}

impl RecordingStore {
    fn with_change() -> Self {
        Self {
            recorded: Mutex::new(Some(ChangeStateRecord {
                name: CHANGE.to_owned(),
                workflow_type: "requirement".to_owned(),
                created_at: STARTED_AT,
                status: ChangeStatus::Active,
                archived_at: None,
                active_phase: None,
                worktree: None,
                base_commit: None,
            })),
            started: Mutex::new(Vec::new()),
            finished: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn missing_change() -> Self {
        Self {
            recorded: Mutex::new(None),
            started: Mutex::new(Vec::new()),
            finished: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn start_count(&self) -> usize {
        self.started.lock().expect("start 锁不可中毒").len()
    }

    fn finish_count(&self) -> usize {
        self.finished.lock().expect("finish 锁不可中毒").len()
    }

    fn last_start(&self) -> RunStartCommand {
        self.started
            .lock()
            .expect("start 锁不可中毒")
            .last()
            .expect("run_start 命令应在场")
            .clone()
    }

    fn last_finish(&self) -> RunFinishCommand {
        self.finished
            .lock()
            .expect("finish 锁不可中毒")
            .last()
            .expect("run_finish 命令应在场")
            .clone()
    }
}

impl ChangeStateStore for RecordingStore {
    fn get_change(&self, _name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self.recorded.lock().expect("记录锁不可中毒").clone())
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_phase_records(
        &self,
        _change: &str,
    ) -> Result<Vec<crate::state::PhaseStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_steps(
        &self,
        _change: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<crate::state::StepStateRecord>, StoreFault> {
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
        _change: &str,
        _phase: &str,
        _now: i64,
    ) -> Result<PhaseStartState, StoreFault> {
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

    fn run_start(&self, command: &RunStartCommand) -> Result<(), StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        self.started
            .lock()
            .expect("start 锁不可中毒")
            .push(command.clone());
        Ok(())
    }

    fn run_finish(&self, command: &RunFinishCommand) -> Result<(), StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        self.finished
            .lock()
            .expect("finish 锁不可中毒")
            .push(command.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 输入工厂
// ---------------------------------------------------------------------------

fn start_command() -> RunStartCommand {
    RunStartCommand {
        run_id: RUN_ID.to_owned(),
        change: CHANGE.to_owned(),
        started_at: STARTED_AT,
    }
}

fn finish_command(status: RunStatus, steps: Vec<RunStepEntry>) -> RunFinishCommand {
    RunFinishCommand {
        run_id: RUN_ID.to_owned(),
        change: CHANGE.to_owned(),
        status,
        reason: Some("收口记因".to_owned()),
        finished_at: FINISHED_AT,
        steps,
    }
}

fn executor_entry(seq: u64) -> RunStepEntry {
    RunStepEntry {
        seq,
        phase: "implement".to_owned(),
        attempt: 1,
        step: RunStepKind::Executor,
        status: RunStepStatus::Passed,
        session_id: Some("ses-1".to_owned()),
        detail: None,
    }
}

// ---------------------------------------------------------------------------
// 异常：校验前置（零写命令下发 / 零库写）
// ---------------------------------------------------------------------------

/// run_start run_id 空串 → Err 且零库写（假件捕获面断言未触 store）。
#[test]
fn run_start空白run_id拒绝且零库写() {
    let fake = RecordingStore::with_change();
    let command = RunStartCommand {
        run_id: "   ".to_owned(),
        change: CHANGE.to_owned(),
        started_at: STARTED_AT,
    };

    let err = run_start(&fake, &command).expect_err("空白 run_id 应 Err");

    assert!(err.contains("run_id"), "错误记因身份段缺失，实际: {err}");
    assert_eq!(fake.start_count(), 0, "校验前置：store 零写入");
}

/// run_start / run_finish 对未建档 change → Err（NotFound 语义记因，AC-1）。
#[test]
fn run写面未建档change显式err() {
    let fake = RecordingStore::missing_change();

    let start_err = run_start(&fake, &start_command()).expect_err("未建档 run_start 应 Err");
    assert!(
        start_err.contains(CHANGE) && start_err.contains("未建档"),
        "run_start 记因携 change 名与建档语义，实际: {start_err}"
    );

    let finish_err = run_finish(&fake, &finish_command(RunStatus::Completed, Vec::new()))
        .expect_err("未建档 run_finish 应 Err");
    assert!(
        finish_err.contains(CHANGE) && finish_err.contains("未建档"),
        "run_finish 记因携 change 名与建档语义，实际: {finish_err}"
    );

    assert_eq!(fake.start_count(), 0, "两路校验前置：store 零写入");
    assert_eq!(fake.finish_count(), 0);
}

/// run_finish status=Running → Err；status=Interrupted → Err 且记因含
/// 「运行期写路径不产生 interrupted」（AC-4 live 写路径防线）。
#[test]
fn run_finish非终态三值拒绝_interrupted记因定式() {
    let fake = RecordingStore::with_change();

    let running_err = run_finish(&fake, &finish_command(RunStatus::Running, Vec::new()))
        .expect_err("status=Running 应 Err");
    assert!(
        running_err.contains("running") && running_err.contains("终态非法"),
        "Running 拒绝记因，实际: {running_err}"
    );

    let interrupted_err = run_finish(&fake, &finish_command(RunStatus::Interrupted, Vec::new()))
        .expect_err("status=Interrupted 应 Err");
    assert!(
        interrupted_err.contains("运行期写路径") && interrupted_err.contains("interrupted"),
        "interrupted 拒绝记因定式（仅启动标定产生），实际: {interrupted_err}"
    );

    assert_eq!(fake.finish_count(), 0, "两路拒绝零写命令");
}

// ---------------------------------------------------------------------------
// 边界：空步整包合法 / 委派透传逐字段
// ---------------------------------------------------------------------------

/// run_finish steps 空包（零步 run 收口形态）→ Ok 委派（AC-2 边界：整包含
/// 空集合法）。
#[test]
fn run_finish空步整包委派成功() {
    let fake = RecordingStore::with_change();

    run_finish(&fake, &finish_command(RunStatus::Completed, Vec::new()))
        .expect("零步 run 收口应成功");

    let command = fake.last_finish();
    assert!(command.steps.is_empty(), "空包透传（零步整包合法形态）");
}

/// 假件捕获：run_start / run_finish 收到的命令与入参逐字段等值（写面零改写
/// 透传——D2 词汇本体 = 命令类型本身）。
#[test]
fn 写面委派命令逐字段透传() {
    let fake = RecordingStore::with_change();
    let start = start_command();

    run_start(&fake, &start).expect("run_start 应成功");
    assert_eq!(fake.last_start(), start, "start 命令零改写透传");

    let finish = finish_command(
        RunStatus::Stopped,
        vec![executor_entry(0), executor_entry(2)],
    );
    run_finish(&fake, &finish.clone()).expect("run_finish 应成功");
    assert_eq!(
        fake.last_finish(),
        finish,
        "finish 命令（含步整包）零改写透传"
    );
}
