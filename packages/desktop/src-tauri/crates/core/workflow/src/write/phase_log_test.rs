//! `write::phase_log` 的单元测试（test-design「phase_log.rs ->
//! phase_log_test.rs」节）：落账写操作 port 落库——verdict 推导（空 checklist
//! → pass）/ report ≤2000 / 表位 / active_phase 匹配校验前置保留（任一失败
//! 零写命令下发），持久化经 [`ChangeStateStore::log_phase`]（PhaseLogCommand
//! 逐字段捕获比对），`StoreFault::Conflict`（重复 attempt）传播，形状随行
//!（skipped / 三会话槽位透传）。
//!
//! Mock策略（test-design 本节 Mock 表 = AC-2 七操作代表行）：进程内假件实现
//! trait——捕获 PhaseLogCommand 逐字段比对 + 可编程 `StoreFault`；零残留原子
//! 性证据归 store_test 真件节，本节不重复。

use std::sync::Mutex;

use super::phase_log::{phase_log, PhaseLogInput};
use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    ActivePhaseState, BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus,
    PhaseLogCommand, PhaseStateRecord, StepCommand, StepStateRecord, StoreFault,
};

const CHANGE: &str = "demo-change";

/// 确定性时间戳基（UTC unix millis）。
const T0: i64 = 1_727_000_000_000;

/// 开相 active_phase 形态（dev-design，start_at 固定值供继承断言）。
const ACTIVE_START_AT: i64 = 1_727_000_080_000;

// ---------------------------------------------------------------------------
// 假件 store：get_change 只读 + log_phase 捕获命令并镜像真件落库语义
//（attempt = 该相位既有条目数 + 1、条目追加、active_phase 清位）。
// ---------------------------------------------------------------------------

struct LogStore {
    record: Mutex<Option<ChangeStateRecord>>,
    entries: Mutex<Vec<PhaseStateRecord>>,
    /// 捕获的落账写命令（零写断言 + 逐字段比对观察面）。
    commands: Mutex<Vec<PhaseLogCommand>>,
    fault: Mutex<Option<StoreFault>>,
}

impl LogStore {
    fn with_active_phase(active: Option<ActivePhaseState>) -> Self {
        Self {
            record: Mutex::new(Some(ChangeStateRecord {
                name: CHANGE.to_owned(),
                workflow_type: "requirement".to_owned(),
                created_at: T0,
                status: ChangeStatus::Active,
                archived_at: None,
                active_phase: active,
            })),
            entries: Mutex::new(Vec::new()),
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

    /// 重开 dev-design 相位（落账清位后再次落账用例的开相步，镜像
    /// start → 落账 → 再 start 节奏）。
    fn reopen_active(&self) {
        if let Some(record) = self.record.lock().expect("记录锁不可中毒").as_mut() {
            record.active_phase = active_dev_design();
        }
    }

    fn command_count(&self) -> usize {
        self.commands.lock().expect("命令锁不可中毒").len()
    }

    fn last_command(&self) -> PhaseLogCommand {
        self.commands
            .lock()
            .expect("命令锁不可中毒")
            .last()
            .expect("落账命令应在场")
            .clone()
    }

    fn log(&self, input: &PhaseLogInput) -> Result<super::phase_log::PhaseLogOutcome, String> {
        phase_log(self, CHANGE, input)
    }
}

impl ChangeStateStore for LogStore {
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

    fn log_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        let attempt = self
            .entries
            .lock()
            .expect("条目锁不可中毒")
            .iter()
            .filter(|entry| entry.phase == command.phase)
            .count() as u32
            + 1;
        self.entries
            .lock()
            .expect("条目锁不可中毒")
            .push(PhaseStateRecord {
                id: i64::from(attempt),
                change: command.change.clone(),
                phase: command.phase.clone(),
                attempt,
                verdict: command.verdict,
                report: command.report.clone(),
                checklist: command.checklist.clone(),
                skipped: command.skipped,
                stale: false,
                backtrack_to: None,
                backtrack_reason: None,
                executor_session_id: command.executor_session_id.clone(),
                evaluator_session_id: command.evaluator_session_id.clone(),
                decision_session_id: command.decision_session_id.clone(),
                start_at: command.start_at,
                timestamp: command.timestamp,
            });
        if let Some(record) = self.record.lock().expect("记录锁不可中毒").as_mut() {
            record.active_phase = None; // 落账即收相位（开相才可落账）
        }
        self.commands
            .lock()
            .expect("命令锁不可中毒")
            .push(command.clone());
        Ok(attempt)
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
}

// ---------------------------------------------------------------------------
// 输入工厂
// ---------------------------------------------------------------------------

/// dev-design 的 active_phase 开态（attempt 2、start_at 固定）。
fn active_dev_design() -> Option<ActivePhaseState> {
    Some(ActivePhaseState {
        phase: "dev-design".to_owned(),
        attempt: 2,
        start_at: ACTIVE_START_AT,
    })
}

/// pass checklist 行构造。
fn pass_items(n: usize) -> Vec<ChecklistItem> {
    (0..n)
        .map(|idx| ChecklistItem {
            item: format!("检查项{idx}"),
            pass: true,
            evidence: "事实依据".to_owned(),
        })
        .collect()
}

/// 带 fail 项的 checklist。
fn with_fail_item(mut items: Vec<ChecklistItem>) -> Vec<ChecklistItem> {
    items.push(ChecklistItem {
        item: "组件表完整".to_owned(),
        pass: false,
        evidence: "缺 renderers 职责".to_owned(),
    });
    items
}

/// 基础落账输入（dev-design、无槽位）。
fn base_input() -> PhaseLogInput {
    PhaseLogInput {
        phase: "dev-design".to_owned(),
        report: "设计与任务拆解齐备".to_owned(),
        checklist: pass_items(2),
        skipped: false,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
    }
}

/// 当前 UTC unix 毫秒（timestamp 上界断言用；不与 wall-clock 比等值）。
fn now_upper() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// 正向：pass / fail 落账（命令逐字段 + active_phase 清位）
// ---------------------------------------------------------------------------

/// checklist 全 pass → verdict=pass；PhaseLogCommand 逐字段与输入一致
///（start_at 自匹配 active_phase 继承、timestamp 写面铸出），outcome.phase /
/// attempt 与命令一致，落账后 active_phase 清位。
#[test]
fn pass落账_command逐字段一致且active_phase清位() {
    let fake = LogStore::with_active_phase(active_dev_design());
    let before = now_upper();

    let outcome = fake
        .log(&PhaseLogInput {
            executor_session_id: Some("ses-exec-1".to_owned()),
            evaluator_session_id: Some("ses-eval-1".to_owned()),
            ..base_input()
        })
        .expect("落账应成功");

    let after = now_upper();
    assert_eq!(outcome.phase, "dev-design");
    assert_eq!(outcome.attempt, 1, "无既有条目 → attempt 1");

    let command = fake.last_command();
    assert_eq!(command.change, CHANGE);
    assert_eq!(command.phase, "dev-design");
    assert_eq!(command.verdict, Verdict::Pass, "checklist 全 pass → pass");
    assert_eq!(command.report, "设计与任务拆解齐备");
    assert_eq!(command.checklist, pass_items(2), "checklist 逐项透传");
    assert!(!command.skipped);
    assert_eq!(
        command.executor_session_id.as_deref(),
        Some("ses-exec-1"),
        "会话槽位逐字透传"
    );
    assert_eq!(command.evaluator_session_id.as_deref(), Some("ses-eval-1"));
    assert_eq!(
        command.decision_session_id, None,
        "decision 槽位不走本输入，恒 None"
    );
    assert_eq!(
        command.start_at,
        Some(ACTIVE_START_AT),
        "start_at 自匹配的 active_phase 继承"
    );
    assert!(
        command.timestamp >= before && command.timestamp <= after,
        "timestamp 写面铸出（边界内 i64 millis），实际: {}",
        command.timestamp
    );

    // 假件落库侧：active_phase 清位（匹配才清）
    let record = fake
        .get_change(CHANGE)
        .expect("假件读半边应可用")
        .expect("建档记录应在场");
    assert!(record.active_phase.is_none(), "落账后 active_phase 清除");
    let entries = fake
        .list_phase_records(CHANGE)
        .expect("假件读半边应可用");
    assert_eq!(entries.len(), 1, "落账追加一条评估条目");
}

/// 含 fail 项 → verdict=fail（verdict 推导前置保留；进重试 / 决策分叉的
/// 输入面）。
#[test]
fn fail落账verdict推导为fail() {
    let fake = LogStore::with_active_phase(active_dev_design());

    let outcome = fake
        .log(&PhaseLogInput {
            report: "首轮未过".to_owned(),
            checklist: with_fail_item(pass_items(1)),
            ..base_input()
        })
        .expect("落账应成功");

    assert_eq!(outcome.attempt, 1);
    let command = fake.last_command();
    assert_eq!(command.verdict, Verdict::Fail, "含 fail 项 → fail");
    assert_eq!(command.checklist.len(), 2);
    assert!(!command.checklist[1].pass);
    assert_eq!(command.checklist[1].evidence, "缺 renderers 职责");
}

/// checklist 空 vec → verdict 推导 pass（与插件 resolveVerdict 的 every 语义
/// 一致——空集全真）。
#[test]
fn checklist空vec_verdict推导为pass() {
    let fake = LogStore::with_active_phase(active_dev_design());

    fake.log(&PhaseLogInput {
        checklist: Vec::new(),
        ..base_input()
    })
    .expect("空 checklist 落账应成功");

    assert_eq!(fake.last_command().verdict, Verdict::Pass);
}

// ---------------------------------------------------------------------------
// 异常：校验前置保留（零写命令下发）
// ---------------------------------------------------------------------------

/// report 超 2000 字符 → `Err` 且零写命令下发（长度校验保留）。
#[test]
fn report超长2001拒绝且零写命令() {
    let fake = LogStore::with_active_phase(active_dev_design());

    let err = fake
        .log(&PhaseLogInput {
            report: "评".repeat(2001),
            ..base_input()
        })
        .expect_err("2001 字符应 Err");

    assert!(
        err.contains("2000") && err.contains("2001"),
        "错误携带上限与实际值，实际: {err}"
    );
    assert_eq!(fake.command_count(), 0, "校验前置：零写命令下发");
}

/// report 恰 2000 字符 → 通过（chars().count() 口径边界含端点）。
#[test]
fn report恰2000字符落账成功() {
    let fake = LogStore::with_active_phase(active_dev_design());

    let outcome = fake
        .log(&PhaseLogInput {
            report: "评".repeat(2000),
            ..base_input()
        })
        .expect("恰 2000 字符应落账成功（边界含端点）");

    assert_eq!(outcome.attempt, 1);
    assert_eq!(fake.last_command().report.chars().count(), 2000);
}

/// active_phase 缺失 → `Err` 且零写（开相前置保留在 core，不在 store）。
#[test]
fn 无active_phase落账显式拒绝零写入() {
    let fake = LogStore::with_active_phase(None);

    let err = fake.log(&base_input()).expect_err("无 active_phase 应 Err");

    assert!(
        err.contains("dev-design") && err.contains("active_phase"),
        "错误记因相位与开相前置，实际: {err}"
    );
    assert_eq!(fake.command_count(), 0, "零写命令");
}

/// active_phase 停留他相 → 跨相落账显式 `Err` 且零写。
#[test]
fn active_phase停留他相落账拒绝零写入() {
    let fake = LogStore::with_active_phase(Some(ActivePhaseState {
        phase: "proposal".to_owned(),
        attempt: 1,
        start_at: ACTIVE_START_AT,
    }));

    let err = fake.log(&base_input()).expect_err("跨相落账应 Err");

    assert!(
        err.contains("proposal") && err.contains("dev-design"),
        "错误记因停留相位与目标相位，实际: {err}"
    );
    assert_eq!(fake.command_count(), 0, "零写命令");
}

/// 相位不在 requirement 表 → `Err` 且零写（表位前置在开相比对之前）。
#[test]
fn 表外相位落账拒绝零写入() {
    let fake = LogStore::with_active_phase(active_dev_design());

    let err = fake
        .log(&PhaseLogInput {
            phase: "幽灵相位".to_owned(),
            ..base_input()
        })
        .expect_err("表外相位应 Err");

    assert!(
        err.contains("幽灵相位") && err.contains("不包含"),
        "错误记因表位缺失，实际: {err}"
    );
    assert_eq!(fake.command_count(), 0, "零写命令");
}

/// skipped 约束两面：skipped=true 配全 pass 清单放行且 skipped 透传落库；
/// skipped=true 配 fail 清单显式 `Err` 零写。
#[test]
fn skipped约束两面各就位() {
    // 合法形态：skipped=true + 全 pass 清单
    let fake = LogStore::with_active_phase(Some(ActivePhaseState {
        phase: "test-gen".to_owned(),
        attempt: 1,
        start_at: ACTIVE_START_AT,
    }));
    let outcome = fake
        .log(&PhaseLogInput {
            phase: "test-gen".to_owned(),
            report: "相位跳过".to_owned(),
            checklist: pass_items(1),
            skipped: true,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
        })
        .expect("skipped=true 配 pass 清单不误拒");
    assert_eq!(outcome.phase, "test-gen");
    let command = fake.last_command();
    assert!(command.skipped, "skipped 透传落库");
    assert_eq!(command.verdict, Verdict::Pass);

    // 约束另一面：skipped=true + 含 fail 项 → Err 零写
    let fake_fail = LogStore::with_active_phase(Some(ActivePhaseState {
        phase: "test-gen".to_owned(),
        attempt: 1,
        start_at: ACTIVE_START_AT,
    }));
    let err = fake_fail
        .log(&PhaseLogInput {
            phase: "test-gen".to_owned(),
            report: "跳过但清单未过".to_owned(),
            checklist: with_fail_item(Vec::new()),
            skipped: true,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
        })
        .expect_err("skipped=true 配 fail 清单应 Err");
    assert!(
        err.contains("pass") && err.contains("fail"),
        "约束记因，实际: {err}"
    );
    assert_eq!(fake_fail.command_count(), 0, "约束拒绝零写命令");
}

/// workflow_type 非 requirement → Err 且零写（W8 分层出口）。
#[test]
fn workflow_type非requirement拒绝零写入() {
    let missing_active = LogStore::with_active_phase(None);
    if let Some(record) = missing_active.record.lock().expect("记录锁").as_mut() {
        record.workflow_type = "refactor".to_owned();
    }

    let err = missing_active
        .log(&PhaseLogInput {
            phase: "proposal".to_owned(),
            ..base_input()
        })
        .expect_err("非 requirement 应 Err");

    assert!(err.contains("refactor"), "W8 分层出口记因，实际: {err}");
    assert_eq!(missing_active.command_count(), 0, "零写命令");
}

/// change 未建档 → Err 显式（零写命令）。
#[test]
fn change未建档显式err零写入() {
    let fake = LogStore::missing();

    let err = fake.log(&base_input()).expect_err("未建档应 Err");

    assert!(err.contains("未建档"), "Err 显式记因，实际: {err}");
    assert_eq!(fake.command_count(), 0, "零写命令");
}

// ---------------------------------------------------------------------------
// 异常：StoreFault 传播；边界：三槽位 None / Some 形状随行
// ---------------------------------------------------------------------------

/// 假件 store 返回 `StoreFault::Conflict`（重复 attempt）→ `Err` 记因传播。
#[test]
fn store_conflict故障传播记因() {
    let fake = LogStore::with_active_phase(active_dev_design());
    fake.set_fault(StoreFault::Conflict("重复落账注入".to_owned()));

    let err = fake.log(&base_input()).expect_err("StoreFault 应传播");

    assert!(
        err.contains("conflict:") && err.contains("重复落账注入"),
        "Err 记因携带 fault 语境，实际: {err}"
    );
}

/// 三槽位全 None：命令槽位三列恒 None（缺省落账恒 None——中性命令层不再有
/// 「不落键」形态，null 面由 db 列缺省承接）。
#[test]
fn 三槽位全none命令三列恒none() {
    let fake = LogStore::with_active_phase(active_dev_design());

    fake.log(&base_input()).expect("落账应成功");

    let command = fake.last_command();
    assert_eq!(command.executor_session_id, None);
    assert_eq!(command.evaluator_session_id, None);
    assert_eq!(command.decision_session_id, None);
}

/// 双槽位 Some / decision None 与三槽位全 Some：槽位值逐字透传（写面不解释
/// 不改写）。
#[test]
fn 槽位some值逐字透传() {
    let fake = LogStore::with_active_phase(active_dev_design());

    fake.log(&PhaseLogInput {
        executor_session_id: Some("ses-1-1727000000001".to_owned()),
        evaluator_session_id: Some("ses-2-1727000000002".to_owned()),
        decision_session_id: None,
        ..base_input()
    })
    .expect("落账应成功");
    let command = fake.last_command();
    assert_eq!(command.executor_session_id.as_deref(), Some("ses-1-1727000000001"));
    assert_eq!(command.evaluator_session_id.as_deref(), Some("ses-2-1727000000002"));
    assert_eq!(command.decision_session_id, None);

    fake.reopen_active(); // 首次落账已清位（镜像 start → 落账节奏），重开再落账
    fake.log(&PhaseLogInput {
        decision_session_id: Some("ses-3-1727000000003".to_owned()),
        ..base_input()
    })
    .expect("携 decision 槽位的输入同样透传（形状不变）");
    assert_eq!(
        fake.last_command().decision_session_id.as_deref(),
        Some("ses-3-1727000000003"),
        "decision 槽位透传（表位不校验、值不改写）"
    );
}

