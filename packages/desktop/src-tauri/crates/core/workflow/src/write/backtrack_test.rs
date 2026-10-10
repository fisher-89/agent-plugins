//! `write::backtrack` 的单元测试（test-design「backtrack.rs ->
//! backtrack_test.rs」节）：回跳写操作 port 落库——白名单二次校验（越权 Err
//! 零写）、双端表位（目标不超前）、reason ≤500、发起相位条目在位校验，全部
//! 前置保留；stale 闭包计算自 persist 迁入本文件——[`BacktrackCommand
//! .stale_dependents`] = `dependents` BFS 全量闭包（不含目标自身），逐支核对
//! 与末端空闭包边界；`StoreFault` 故障传播。
//!
//! Mock策略（test-design 本节 Mock 表）：进程内假件实现 trait（捕获
//! BacktrackCommand 与 stale_dependents 闭包逐项比对 + 可编程故障）；
//! `phase_table` 真实组合（不变组件零 mock——闭包期望值按 requirement 前置
//! 表手工推导）。零写断言 = 假件写命令捕获表为空。

use std::sync::Mutex;

use super::backtrack::{backtrack, BacktrackInput};
use crate::model::Verdict;
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStateRecord, RunFinishCommand, RunStartCommand, RunStateRecord, RunStepStateRecord,
    StepCommand, StepStateRecord, StoreFault,
};

const CHANGE: &str = "demo-change";

/// 确定性时间戳基（UTC unix millis）。
const T0: i64 = 1_727_000_000_000;
fn t(n: u32) -> i64 {
    T0 + i64::from(n) * 1000
}

// ---------------------------------------------------------------------------
// 假件 store：get_change / list_phase_records 只读 + apply_backtrack 捕获
// 命令；可注入 StoreFault。
// ---------------------------------------------------------------------------

struct BacktrackStore {
    record: Mutex<Option<ChangeStateRecord>>,
    entries: Mutex<Vec<PhaseStateRecord>>,
    /// 捕获的回跳写命令（零写断言 + 闭包逐项比对观察面）。
    commands: Mutex<Vec<BacktrackCommand>>,
    fault: Mutex<Option<StoreFault>>,
}

impl BacktrackStore {
    fn with_entries(entries: Vec<PhaseStateRecord>) -> Self {
        Self {
            record: Mutex::new(Some(ChangeStateRecord {
                name: CHANGE.to_owned(),
                workflow_type: "requirement".to_owned(),
                created_at: T0,
                status: ChangeStatus::Active,
                archived_at: None,
                active_phase: None,
                worktree: None,
                base_commit: None,
            })),
            entries: Mutex::new(entries),
            commands: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn missing() -> Self {
        Self {
            record: Mutex::new(None),
            entries: Mutex::new(Vec::new()),
            commands: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn set_fault(&self, fault: StoreFault) {
        *self.fault.lock().expect("故障锁不可中毒") = Some(fault);
    }

    fn command_count(&self) -> usize {
        self.commands.lock().expect("命令锁不可中毒").len()
    }

    fn run(&self, input: &BacktrackInput) -> Result<super::backtrack::BacktrackOutcome, String> {
        backtrack(self, CHANGE, input)
    }
}

impl ChangeStateStore for BacktrackStore {
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
        _change: &str,
        _phase: &str,
        _now: i64,
    ) -> Result<crate::state::PhaseStartState, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn log_phase(&self, _command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn apply_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        self.commands
            .lock()
            .expect("命令锁不可中毒")
            .push(command.clone());
        Ok(())
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

// ---------------------------------------------------------------------------
// 种子工厂
// ---------------------------------------------------------------------------

fn pass_entry(phase: &str, attempt: u32, ts: i64) -> PhaseStateRecord {
    PhaseStateRecord {
        id: i64::from(attempt),
        change: CHANGE.to_owned(),
        phase: phase.to_owned(),
        attempt,
        verdict: Verdict::Pass,
        report: format!("{phase} 通过"),
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

/// 全相位 pass 历史（stale 传播断言的底座；条目按表序、时间戳递进）。
fn full_pass_history() -> Vec<PhaseStateRecord> {
    [
        "proposal",
        "dev-design",
        "test-design",
        "implement",
        "test-gen",
        "test-execution",
        "code-review",
        "acceptance",
    ]
    .iter()
    .enumerate()
    .map(|(idx, phase)| pass_entry(phase, 1, t(idx as u32 + 1)))
    .collect()
}

/// 白名单输入构造（allowed = 表序前置全量至发起相位）。
fn input_at(phase: &str, to: &str, reason: &str, allowed: &[&str]) -> BacktrackInput {
    BacktrackInput {
        phase: phase.to_owned(),
        to: to.to_owned(),
        reason: reason.to_owned(),
        allowed: allowed.iter().map(|id| (*id).to_owned()).collect(),
    }
}

/// 闭包集合断言（顺序无关——闭包按相位名集合语义下发，store 按名置 stale）。
fn assert_closure_set(command: &BacktrackCommand, expected: &[&str]) {
    let mut actual = command.stale_dependents.clone();
    actual.sort();
    let mut expected_sorted: Vec<String> = expected.iter().map(|id| id.to_string()).collect();
    expected_sorted.sort();
    assert_eq!(
        actual, expected_sorted,
        "stale 闭包集合不符，实际: {:?}",
        command.stale_dependents
    );
}

// ---------------------------------------------------------------------------
// 正向：白名单内回跳落库 + stale 闭包计算（迁入本文件后的行为断言）
// ---------------------------------------------------------------------------

/// 白名单内目标回跳 → BacktrackCommand（change / phase / to / reason）经
/// `store.apply_backtrack` 落库，outcome 与命令一致。
#[test]
fn 白名单内回跳_command逐字段落库且outcome一致() {
    let fake = BacktrackStore::with_entries(vec![
        pass_entry("proposal", 1, t(1)),
        pass_entry("dev-design", 1, t(2)),
        pass_entry("test-gen", 1, t(3)),
    ]);

    let outcome = fake
        .run(&input_at(
            "test-gen",
            "proposal",
            "需求基线返工",
            &[
                "proposal",
                "dev-design",
                "test-design",
                "implement",
                "test-gen",
            ],
        ))
        .expect("白名单内回溯应成功");

    assert_eq!(
        outcome,
        super::backtrack::BacktrackOutcome {
            phase: "test-gen".to_owned(),
            target: "proposal".to_owned(),
        },
        "outcome {{ phase, target }} 与输入一致"
    );
    assert_eq!(fake.command_count(), 1, "恰一条回跳写命令");
    let command = &fake.commands.lock().expect("命令锁不可中毒")[0];
    assert_eq!(command.change, CHANGE);
    assert_eq!(command.phase, "test-gen", "回跳发起相位");
    assert_eq!(command.to, "proposal");
    assert_eq!(command.reason, "需求基线返工");
}

/// stale 闭包（迁入）：目标 dev-design → 全部依赖相位闭包
///（test-design → implement → test-gen → test-execution / code-review →
/// acceptance 多支逐支核对），目标自身不在闭包内。
#[test]
fn stale闭包_target为dev_design_多支逐支核对() {
    let fake = BacktrackStore::with_entries(full_pass_history());

    fake.run(&input_at(
        "test-gen",
        "dev-design",
        "设计返工",
        &[
            "proposal",
            "dev-design",
            "test-design",
            "implement",
            "test-gen",
        ],
    ))
    .expect("回溯应成功");

    let command = &fake.commands.lock().expect("命令锁不可中毒")[0];
    assert_closure_set(
        command,
        &[
            "test-design",
            "implement",
            "test-gen",
            "test-execution",
            "code-review",
            "acceptance",
        ],
    );
    assert!(
        !command.stale_dependents.iter().any(|id| id == "dev-design"),
        "目标自身不在闭包内（其「最新 pass 置 stale」由 store 落账半边单独承接）"
    );
    assert!(
        !command.stale_dependents.iter().any(|id| id == "proposal"),
        "上游相位不进闭包（传播方向自洽）"
    );
}

/// stale 闭包（迁入）：目标 test-design → 闭包仅 test-gen 一支及其下游
///（test-execution / code-review）；implement / acceptance 不依赖
/// test-design，不入闭包（表依赖链多支时逐支核对）。
#[test]
fn stale闭包_target为test_design_支链不含implement与acceptance() {
    let fake = BacktrackStore::with_entries(full_pass_history());

    fake.run(&input_at(
        "implement",
        "test-design",
        "测试设计返工",
        &["proposal", "dev-design", "test-design", "implement"],
    ))
    .expect("回溯应成功");

    let command = &fake.commands.lock().expect("命令锁不可中毒")[0];
    assert_closure_set(command, &["test-gen", "test-execution", "code-review"]);
}

/// stale 闭包（迁入）：目标为末端相位 acceptance → 空闭包（无下游依赖）。
#[test]
fn stale闭包_target为末端acceptance_空闭包() {
    let fake = BacktrackStore::with_entries(full_pass_history());

    fake.run(&input_at(
        "acceptance",
        "acceptance",
        "验收口径返工",
        &[
            "proposal",
            "dev-design",
            "test-design",
            "implement",
            "test-gen",
            "test-execution",
            "code-review",
            "acceptance",
        ],
    ))
    .expect("自回溯（同相位）应成功");

    let command = &fake.commands.lock().expect("命令锁不可中毒")[0];
    assert!(
        command.stale_dependents.is_empty(),
        "末端相位无下游 → 空闭包，实际: {:?}",
        command.stale_dependents
    );
}

/// reason 恰 500 字符 → 成功（≤500 边界含端点）。
#[test]
fn reason恰500字符成功() {
    let fake = BacktrackStore::with_entries(vec![
        pass_entry("proposal", 1, t(1)),
        pass_entry("dev-design", 1, t(2)),
    ]);

    let outcome = fake
        .run(&input_at(
            "dev-design",
            "proposal",
            &"因".repeat(500),
            &["proposal", "dev-design"],
        ))
        .expect("恰 500 字符应成功（边界含端点）");

    assert_eq!(outcome.target, "proposal");
    assert_eq!(
        fake.commands.lock().expect("命令锁不可中毒")[0]
            .reason
            .chars()
            .count(),
        500
    );
}

// ---------------------------------------------------------------------------
// 异常：校验前置保留（零写）
// ---------------------------------------------------------------------------

/// 目标不在白名单 → `Err` 零写（写面二次校验兜底——坏决议损坏不了状态）。
#[test]
fn 越权回溯拒绝且零写() {
    let fake = BacktrackStore::with_entries(full_pass_history());

    let err = fake
        .run(&input_at(
            "dev-design",
            "proposal",
            "任意",
            &["dev-design", "test-design"],
        ))
        .expect_err("越权目标应 Err");

    assert!(
        err.contains("proposal") && err.contains("越权"),
        "错误显式携带目标与越权记因，实际: {err}"
    );
    assert_eq!(fake.command_count(), 0, "零写命令");
}

/// reason 超 500 → `Err` 零写（≤500 约定的写面承载）。
#[test]
fn reason超长501拒绝零写() {
    let fake = BacktrackStore::with_entries(full_pass_history());

    let err = fake
        .run(&input_at(
            "dev-design",
            "proposal",
            &"因".repeat(501),
            &["proposal", "dev-design"],
        ))
        .expect_err("501 字符应 Err");

    assert!(
        err.contains("500") && err.contains("501"),
        "错误携带上限与实际值，实际: {err}"
    );
    assert_eq!(fake.command_count(), 0, "零写命令");
}

/// 非法目标相位与未来目标 → `Err` 零写（双端表位校验：双端在表内、目标不
/// 超前）。
#[test]
fn 非法目标相位与未来目标拒绝零写() {
    let fake = BacktrackStore::with_entries(full_pass_history());

    // to 不在表（allowed 随行含该目标——walker 预校验 normally 拦下，写面
    // 二次校验兜底直达表位门）
    let err = fake
        .run(&input_at(
            "dev-design",
            "幽灵相位",
            "任意",
            &["幽灵相位", "dev-design"],
        ))
        .expect_err("表外目标应 Err");
    assert!(err.contains("幽灵相位"), "记因: {err}");
    assert_eq!(fake.command_count(), 0, "零写命令");

    // 回溯到未来相位（target 在发起相位之后）→ Err
    let err = fake
        .run(&input_at(
            "proposal",
            "implement",
            "任意",
            &["implement", "proposal"],
        ))
        .expect_err("未来相位应 Err");
    assert!(
        err.contains("未来") || err.contains("不支持"),
        "记因: {err}"
    );
    assert_eq!(fake.command_count(), 0, "零写命令");
}

/// 发起相位无评估条目 → `Err` 零写（无可标记条目不可回溯——语义与既往一致）。
#[test]
fn 发起相位无条目拒绝零写() {
    let fake = BacktrackStore::with_entries(vec![pass_entry("proposal", 1, t(1))]);

    let err = fake
        .run(&input_at(
            "dev-design",
            "proposal",
            "任意",
            &["proposal", "dev-design"],
        ))
        .expect_err("发起相位无条目应 Err");

    assert!(
        err.contains("dev-design") && (err.contains("评估") || err.contains("条目")),
        "记因: {err}"
    );
    assert_eq!(fake.command_count(), 0, "零写命令");
}

/// change 未建档 → `Err` 显式（零写）。
#[test]
fn change未建档显式err零写() {
    let fake = BacktrackStore::missing();

    let err = fake
        .run(&input_at(
            "dev-design",
            "proposal",
            "任意",
            &["proposal", "dev-design"],
        ))
        .expect_err("未建档应 Err");

    assert!(err.contains("未建档"), "记因: {err}");
    assert_eq!(fake.command_count(), 0, "零写命令");
}

// ---------------------------------------------------------------------------
// 异常：StoreFault 故障传播（stale 翻转与回跳同事务由 store 节承载）
// ---------------------------------------------------------------------------

/// 假件 store 注入 `StoreFault` → `Err` 传播（命令不下发成功面）。
#[test]
fn store故障传播err记因() {
    let fake = BacktrackStore::with_entries(vec![
        pass_entry("proposal", 1, t(1)),
        pass_entry("dev-design", 1, t(2)),
    ]);
    fake.set_fault(StoreFault::Db("注入的回跳写故障".to_owned()));

    let err = fake
        .run(&input_at(
            "dev-design",
            "proposal",
            "任意",
            &["proposal", "dev-design"],
        ))
        .expect_err("StoreFault 应传播");

    assert!(
        err.contains("db:") && err.contains("注入的回跳写故障"),
        "Err 记因携带 fault 语境，实际: {err}"
    );
}
