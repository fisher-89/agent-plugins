//! `write::run_start` / `write::run_finish` 的单元测试（test-design
//! 「write/run.rs -> run_test.rs」节）：run 写面校验前置（run_id 非空 + change
//! 建档在案 + 终态三值且非 interrupted）保留——任一失败零写命令下发；校验读
//! 记录面随动（载荷 `change_id` 为寻址键——按 id 读记录，未建档 id 显式 Err）；
//! 委派透传（命令逐字段等值，写面零改写——D2 词汇本体 = 命令类型本身）；真件
//! tempfile workspace 库回环（running 行回读 / 三终态回读——AC-1/AC-2 第一写
//! 与收口写）。
//!
//! Mock策略（test-design 本节 Mock 表）：ChangeStateStore 进程内假件（Mutex
//! 捕获 run_start / run_finish 命令与读记录 id、可编程 Err、越权读面 panic）
//! 作被测函数显式入参注入（入参例外）；db 文件（进程边界）真实 workspace 库
//! 半边归 corpus_golden_test.rs 真件节（集成测试单实例解析，类型面自洽）。
//! id 与 name 字面量各异：寻址断言以 id 为键、name 仅展示属性。

use std::sync::Mutex;

use super::run::{run_finish, run_start};
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, RunFinishCommand, RunStartCommand, RunStateRecord, RunStatus, RunStepEntry,
    RunStepKind, RunStepStateRecord, RunStepStatus, StepCommand, StoreFault,
};

/// 身份锚字面量（run 写面寻址键——载荷 `change_id`）。
const CHANGE_ID: &str = "0198f7a0-0000-7000-8000-0000000000e5";
/// change 名（零寻址职能，仅记录展示属性）。
const CHANGE_NAME: &str = "demo-change";
/// 库内不存在的 id（未建档拒绝面）。
const UNKNOWN_ID: &str = "0198f7a0-0000-7000-8000-0000000000fb";
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
    /// 捕获的读记录 id（校验读记录面按 change_id 寻址断言面）。
    read_ids: Mutex<Vec<String>>,
    /// 可编程 store 半边 Err（run_start / run_finish 命中即 Err）。
    fault: Mutex<Option<StoreFault>>,
}

impl RecordingStore {
    fn with_change() -> Self {
        Self {
            recorded: Mutex::new(Some(ChangeStateRecord {
                id: CHANGE_ID.to_owned(),
                name: CHANGE_NAME.to_owned(),
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
            read_ids: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn missing_change() -> Self {
        Self {
            recorded: Mutex::new(None),
            started: Mutex::new(Vec::new()),
            finished: Mutex::new(Vec::new()),
            read_ids: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    /// 捕获的读记录 id 序列（按 id 寻址断言面）。
    fn read_ids(&self) -> Vec<String> {
        self.read_ids.lock().expect("读锁不可中毒").clone()
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
    fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        self.read_ids
            .lock()
            .expect("读锁不可中毒")
            .push(id.to_owned());
        Ok(self
            .recorded
            .lock()
            .expect("记录锁不可中毒")
            .clone()
            .filter(|record| record.id == id))
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_phase_records(
        &self,
        _change_id: &str,
    ) -> Result<Vec<crate::state::PhaseStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_steps(
        &self,
        _change_id: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<crate::state::StepStateRecord>, StoreFault> {
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
        _change_id: &str,
        _phase: &str,
        _session_id: &str,
    ) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
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
        change_id: CHANGE_ID.to_owned(),
        started_at: STARTED_AT,
    }
}

/// 携指定 change_id 的发起命令（未建档 id 拒绝面）。
fn start_command_for(change_id: &str) -> RunStartCommand {
    RunStartCommand {
        change_id: change_id.to_owned(),
        ..start_command()
    }
}

fn finish_command(status: RunStatus, steps: Vec<RunStepEntry>) -> RunFinishCommand {
    RunFinishCommand {
        run_id: RUN_ID.to_owned(),
        change_id: CHANGE_ID.to_owned(),
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
        change_id: CHANGE_ID.to_owned(),
        started_at: STARTED_AT,
    };

    let err = run_start(&fake, &command).expect_err("空白 run_id 应 Err");

    assert!(err.contains("run_id"), "错误记因身份段缺失，实际: {err}");
    assert_eq!(fake.start_count(), 0, "校验前置：store 零写入");
}

/// run_start / run_finish 对未建档 id → Err（读记录面随动：按载荷 change_id
/// 寻址，未登记 id 记因携 id 语境、不回落 name——AC-1）。
#[test]
fn 未建档id显式err() {
    let fake = RecordingStore::missing_change();

    let start_err = run_start(&fake, &start_command()).expect_err("未建档 run_start 应 Err");
    assert!(
        start_err.contains(CHANGE_ID) && start_err.contains("未建档"),
        "run_start 记因携 change id 与建档语义，实际: {start_err}"
    );

    let finish_err = run_finish(&fake, &finish_command(RunStatus::Completed, Vec::new()))
        .expect_err("未建档 run_finish 应 Err");
    assert!(
        finish_err.contains(CHANGE_ID) && finish_err.contains("未建档"),
        "run_finish 记因携 change id 与建档语义，实际: {finish_err}"
    );

    assert_eq!(fake.start_count(), 0, "两路校验前置：store 零写入");
    assert_eq!(fake.finish_count(), 0);

    // 库内有建档但载荷 id 未登记：按 id 读记录 miss（name 不作寻址回退）
    let fake = RecordingStore::with_change();
    let err = run_start(&fake, &start_command_for(UNKNOWN_ID)).expect_err("未登记 id 应 Err");
    assert!(
        err.contains(UNKNOWN_ID) && err.contains("未建档"),
        "Err 记因携未登记 id 语境，实际: {err}"
    );
    assert!(
        !err.contains(CHANGE_NAME),
        "拒绝面零 name 感知（未解析到记录），实际: {err}"
    );
    assert_eq!(fake.start_count(), 0, "零写入");
    assert_eq!(
        fake.read_ids(),
        vec![UNKNOWN_ID.to_owned()],
        "读记录面按载荷 change_id 寻址（id 化，非 name）"
    );
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
/// 透传——D2 词汇本体 = 命令类型本身；`change_id` 载荷 id 化后断言面零改动）；
/// 校验读记录面按载荷 change_id 寻址。
#[test]
fn 写面委派命令逐字段透传() {
    let fake = RecordingStore::with_change();
    let start = start_command();

    run_start(&fake, &start).expect("run_start 应成功");
    assert_eq!(fake.last_start(), start, "start 命令零改写透传");
    assert_eq!(
        fake.last_start().change_id,
        CHANGE_ID,
        "载荷 change_id = 寻址键（id 化）"
    );

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
    assert_eq!(
        fake.read_ids(),
        vec![CHANGE_ID.to_owned(), CHANGE_ID.to_owned()],
        "两路校验读记录均按载荷 change_id 寻址"
    );
}
