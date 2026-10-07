//! `steps.rs` 的单元测试（test-design「steps.rs -> steps_test.rs」节）。
//!
//! 装置：TempRoot workflow.json 夹具随双向墙退役换血为 store 种子装置——
//! 真件组合行走真实 tempfile workspace 库（store crate dev-dep，进程边界真
//! 实组合），store 故障行走进程内假件注入 `StoreFault`（design D1 fake port
//! 先例）；FakeRunner / FakeTestExecutionRunner 假件装置沿用（记录调用 +
//! 可编程产出）。
//!
//! 覆盖面：七臂命令包络 StepRecord 审计落库（成功 / 失败皆落、run_id 串链、
//! summary ≤500 截断留痕、reference 携 checks 报告目录 / 会话 id）、全链落库
//! 组合（命令 → 写面 → db 双记录可查）、store 故障传播（业务 Err 上抛不静
//! 默，审计失败不阻断臂）、ToolStepPort 直调面持衡。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use store::Store;
use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    ActivePhaseState, BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus,
    PhaseLogCommand, PhaseStartState, PhaseStateRecord, StepCommand, StepKind, StepStateRecord,
    StoreFault,
};
use workflow::write::{BacktrackInput, PhaseLogInput, SessionAnchors};

use crate::port::{
    BoxToolFuture, StaticCheckOutcome, StaticCheckRunner, TestExecutionConclusion,
    TestExecutionOutcome, TestExecutionRunner, ToolCommand, ToolStepOutput, ToolStepPort,
    ToolStepRequest,
};
use crate::steps::LocalToolSteps;

const CHANGE: &str = "demo-change";

/// 种子基准时刻：2026-10-01T08:00:00Z 定值 UTC unix 毫秒（确定性断言面）。
const TS_BASE: i64 = 1_790_841_600_000;

/// 步请求 workspace 根（相位机臂已无 layout 触点；runner 臂仅透传断言面）。
const ROOT: &str = "/tmp/orchestration-steps-test";

// ---------------------------------------------------------------------------
// 装置：真实 workspace 库 + store 种子（真件组合）
// ---------------------------------------------------------------------------

/// 真实 workspace 库装置：tempfile db 文件（`Store::open_workspace` 即
/// `ChangeStateStore` 实现——全链落库组合行的落库证据真件）。
struct TestDb {
    /// store 句柄（字段声明先于 db 目录：drop 序先关库再删目录，Windows 句柄
    /// 纪律）
    store: Arc<Store>,
    _db_dir: tempfile::TempDir,
}

impl TestDb {
    fn open(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("orchestration-steps-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let store =
            Store::open_workspace(&db_dir.path().join("ws.redb")).expect("打开 workspace 库应成功");
        Self {
            store: Arc::new(store),
            _db_dir: db_dir,
        }
    }

    /// store 缝注入面（`Arc<dyn ChangeStateStore>` 类型擦除——组合根同式装配）。
    fn store_arc(&self) -> Arc<dyn ChangeStateStore> {
        Arc::clone(&self.store) as Arc<dyn ChangeStateStore>
    }
}

/// 建档种子：workflow_type requirement、active 起步（created_at 取定值毫秒）。
fn seed_change(store: &Store, name: &str, workflow_type: &str) {
    store
        .create_change_record(ChangeStateRecord {
            name: name.to_owned(),
            workflow_type: workflow_type.to_owned(),
            created_at: TS_BASE,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("建档种子应成功");
}

/// 评估条目种子：开相 + 落账一条（store change 域操作面种子路径）。
fn seed_entry(store: &Store, change: &str, phase: &str, verdict: Verdict, ts: i64) {
    let started = store
        .start_change_phase(change, phase, ts)
        .expect("开相种子应成功");
    store
        .log_change_phase(&PhaseLogCommand {
            change: change.to_owned(),
            phase: phase.to_owned(),
            verdict,
            report: format!("{phase} 种子条目"),
            skipped: false,
            checklist: Vec::new(),
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
            start_at: Some(started.start_at),
            timestamp: ts + 30_000,
        })
        .expect("落账种子应成功");
}

// ---------------------------------------------------------------------------
// 装置：进程内假件 store（store 故障行 + runner 臂审计捕获专用）
// ---------------------------------------------------------------------------

/// 假件内存态：建档记录 + 相位条目 + 审计行（`append_step` 捕获面）。
#[derive(Default)]
struct FakeState {
    changes: HashMap<String, ChangeStateRecord>,
    phases: Vec<PhaseStateRecord>,
    steps: Vec<StepStateRecord>,
}

/// 进程内假件 store（design D1 fake port 先例）：内存记录 + 可编程
/// `StoreFault` 注入位（读路径 / log_phase / append_step 三缝）——审计行经
/// `steps()` 回读（截断 / reference 断言面）。
struct FakeStore {
    db: Mutex<FakeState>,
    fail_read: Mutex<Option<StoreFault>>,
    fail_log_phase: Mutex<Option<StoreFault>>,
    fail_append_step: Mutex<Option<StoreFault>>,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            db: Mutex::new(FakeState::default()),
            fail_read: Mutex::new(None),
            fail_log_phase: Mutex::new(None),
            fail_append_step: Mutex::new(None),
        }
    }

    fn fail_read(&self, fault: StoreFault) -> &Self {
        *self.fail_read.lock().expect("锁不可中毒") = Some(fault);
        self
    }

    fn fail_log_phase(&self, fault: StoreFault) -> &Self {
        *self.fail_log_phase.lock().expect("锁不可中毒") = Some(fault);
        self
    }

    fn fail_append_step(&self, fault: StoreFault) -> &Self {
        *self.fail_append_step.lock().expect("锁不可中毒") = Some(fault);
        self
    }

    /// 审计行回读（append_step 捕获面）。
    fn steps(&self) -> Vec<StepStateRecord> {
        self.db.lock().expect("锁不可中毒").steps.clone()
    }

    fn fault_of(fault: &Mutex<Option<StoreFault>>) -> Option<StoreFault> {
        fault.lock().expect("锁不可中毒").clone()
    }

    /// 相位条目 attempt 事务内推导像（该相位既有条目数 + 1）。
    fn next_attempt(db: &FakeState, change: &str, phase: &str) -> u32 {
        db.phases
            .iter()
            .filter(|entry| entry.change == change && entry.phase == phase)
            .count() as u32
            + 1
    }
}

impl ChangeStateStore for FakeStore {
    fn get_change(&self, name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        if let Some(fault) = Self::fault_of(&self.fail_read) {
            return Err(fault);
        }
        Ok(self
            .db
            .lock()
            .expect("锁不可中毒")
            .changes
            .get(name)
            .cloned())
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        let db = self.db.lock().expect("锁不可中毒");
        let mut records: Vec<ChangeStateRecord> = db.changes.values().cloned().collect();
        records.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(records)
    }

    fn list_phase_records(&self, change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        if let Some(fault) = Self::fault_of(&self.fail_read) {
            return Err(fault);
        }
        let db = self.db.lock().expect("锁不可中毒");
        Ok(db
            .phases
            .iter()
            .filter(|entry| entry.change == change)
            .cloned()
            .collect())
    }

    fn list_steps(
        &self,
        change: &str,
        run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        let db = self.db.lock().expect("锁不可中毒");
        Ok(db
            .steps
            .iter()
            .filter(|row| row.change == change && run_id.is_none_or(|run| row.run_id == run))
            .cloned()
            .collect())
    }

    fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault> {
        let mut db = self.db.lock().expect("锁不可中毒");
        if db.changes.contains_key(&record.name) {
            return Err(StoreFault::Conflict(format!(
                "change 已存在同名建档记录: {}",
                record.name
            )));
        }
        db.changes.insert(record.name.clone(), record);
        Ok(())
    }

    fn delete_change_record(&self, name: &str) -> Result<bool, StoreFault> {
        Ok(self
            .db
            .lock()
            .expect("锁不可中毒")
            .changes
            .remove(name)
            .is_some())
    }

    fn start_phase(
        &self,
        change: &str,
        phase: &str,
        now: i64,
    ) -> Result<PhaseStartState, StoreFault> {
        let mut db = self.db.lock().expect("锁不可中毒");
        let attempt = Self::next_attempt(&db, change, phase);
        let record = db
            .changes
            .get_mut(change)
            .ok_or_else(|| StoreFault::NotFound(format!("change 不存在: {change}")))?;
        record.active_phase = Some(ActivePhaseState {
            phase: phase.to_owned(),
            attempt,
            start_at: now,
        });
        Ok(PhaseStartState {
            attempt,
            start_at: now,
        })
    }

    fn log_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        if let Some(fault) = Self::fault_of(&self.fail_log_phase) {
            return Err(fault);
        }
        let mut db = self.db.lock().expect("锁不可中毒");
        if !db.changes.contains_key(&command.change) {
            return Err(StoreFault::NotFound(format!(
                "change 不存在: {}",
                command.change
            )));
        }
        let attempt = Self::next_attempt(&db, &command.change, &command.phase);
        let id = db.phases.len() as i64 + 1;
        db.phases.push(PhaseStateRecord {
            id,
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
        let record = db
            .changes
            .get_mut(&command.change)
            .expect("建档记录在场（上文已核）");
        record.active_phase = None;
        Ok(attempt)
    }

    fn apply_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreFault> {
        let mut db = self.db.lock().expect("锁不可中毒");
        let latest = db
            .phases
            .iter_mut()
            .filter(|entry| entry.change == command.change && entry.phase == command.phase)
            .max_by_key(|entry| entry.id)
            .ok_or_else(|| {
                StoreFault::NotFound(format!("Phase \"{}\" 没有评估条目", command.phase))
            })?;
        latest.backtrack_to = Some(command.to.clone());
        latest.backtrack_reason = Some(command.reason.clone());
        for entry in db.phases.iter_mut().filter(|entry| {
            entry.change == command.change
                && entry.phase == command.to
                && entry.verdict == Verdict::Pass
        }) {
            entry.stale = true;
        }
        Ok(())
    }

    fn amend_decision_session(
        &self,
        change: &str,
        phase: &str,
        session_id: &str,
    ) -> Result<(), StoreFault> {
        let mut db = self.db.lock().expect("锁不可中毒");
        let latest = db
            .phases
            .iter_mut()
            .filter(|entry| entry.change == change && entry.phase == phase)
            .max_by_key(|entry| entry.id)
            .ok_or_else(|| StoreFault::NotFound(format!("Phase \"{phase}\" 没有评估条目")))?;
        latest.decision_session_id = Some(session_id.to_owned());
        Ok(())
    }

    fn set_archived(&self, name: &str, archived_at: i64) -> Result<(), StoreFault> {
        let mut db = self.db.lock().expect("锁不可中毒");
        let record = db
            .changes
            .get_mut(name)
            .ok_or_else(|| StoreFault::NotFound(format!("change 不存在: {name}")))?;
        record.status = ChangeStatus::Archived;
        record.archived_at = Some(archived_at);
        Ok(())
    }

    fn append_step(&self, command: &StepCommand) -> Result<(), StoreFault> {
        if let Some(fault) = Self::fault_of(&self.fail_append_step) {
            return Err(fault);
        }
        let mut db = self.db.lock().expect("锁不可中毒");
        let id = db.steps.len() as i64 + 1;
        db.steps.push(StepStateRecord {
            id,
            run_id: command.run_id.clone(),
            change: command.change.clone(),
            step_kind: command.step_kind,
            status: command.status.clone(),
            timestamp: command.timestamp,
            summary: command.summary.clone(),
            reference: command.reference.clone(),
        });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 装置：假 runner（StaticCheckRunner / TestExecutionRunner 既有假件沿用）
// ---------------------------------------------------------------------------

/// 假 StaticCheckRunner：记录 root 调用、可编程产出 / Err(String)。
struct FakeRunner {
    roots: Mutex<Vec<String>>,
    result: Result<StaticCheckOutcome, String>,
}

impl FakeRunner {
    fn passing() -> Self {
        Self {
            roots: Mutex::new(Vec::new()),
            result: Ok(StaticCheckOutcome {
                passed: true,
                diagnostics: String::new(),
            }),
        }
    }

    fn failing(diagnostics: &str) -> Self {
        Self {
            roots: Mutex::new(Vec::new()),
            result: Ok(StaticCheckOutcome {
                passed: false,
                diagnostics: diagnostics.to_owned(),
            }),
        }
    }

    /// Err 臂（臂产出 Err(String) 原样上抛 + 审计 error 行的注入面）。
    fn erring(message: &str) -> Self {
        Self {
            roots: Mutex::new(Vec::new()),
            result: Err(message.to_owned()),
        }
    }
}

impl StaticCheckRunner for FakeRunner {
    fn run(&self, root: &str) -> BoxToolFuture {
        self.roots
            .lock()
            .expect("root 锁不可中毒")
            .push(root.to_owned());
        let result = self.result.clone();
        Box::pin(async move { result.map(ToolStepOutput::StaticCheck) })
    }
}

/// 可编程 TestExecutionRunner：记录 root / change 调用、可编程产出 / Err。
struct FakeTestExecutionRunner {
    calls: Arc<Mutex<Vec<(String, String)>>>,
    result: Result<TestExecutionOutcome, String>,
}

impl FakeTestExecutionRunner {
    fn passing() -> Self {
        Self::with_result(Ok(TestExecutionOutcome {
            conclusion: TestExecutionConclusion::Pass,
            total: 5,
            passed: 4,
            failed: 0,
            skipped: 1,
            findings_brief: "全部测试通过且覆盖率达阈值".to_owned(),
            report_dir: "reports/test/app_node-test".to_owned(),
        }))
    }

    fn with_result(result: Result<TestExecutionOutcome, String>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            result,
        }
    }

    fn failing(message: &str) -> Self {
        Self::with_result(Err(message.to_owned()))
    }

    /// 调用记录快照（root / change 透传断言面）。
    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().expect("calls 锁不可中毒").clone()
    }
}

impl TestExecutionRunner for FakeTestExecutionRunner {
    fn run(&self, root: &str, change: &str) -> BoxToolFuture {
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push((root.to_owned(), change.to_owned()));
        let result = self.result.clone();
        Box::pin(async move { result.map(ToolStepOutput::TestExecution) })
    }
}

// ---------------------------------------------------------------------------
// 装置：组合根装配与步驱动
// ---------------------------------------------------------------------------

/// 组合根装配句柄：steps + 双 runner 假件（调用记录读面）。五参构造同式——
/// run 级锚点 + 双 spawn 缝 + store 缝 + run_id（组合根一次装配）。
struct Assembled {
    steps: Arc<LocalToolSteps>,
    static_check: Arc<FakeRunner>,
    execution: Arc<FakeTestExecutionRunner>,
}

/// 组合根装配（run 作用域一次：锚点随 run 铸新、runner / store 随进程复用）。
fn assemble(
    store: Arc<dyn ChangeStateStore>,
    run_id: &str,
    static_check: FakeRunner,
    execution: FakeTestExecutionRunner,
) -> Assembled {
    let static_check = Arc::new(static_check);
    let execution = Arc::new(execution);
    let steps = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::clone(&static_check) as Arc<dyn StaticCheckRunner>,
        Arc::clone(&execution) as Arc<dyn TestExecutionRunner>,
        store,
        run_id.to_owned(),
    ));
    Assembled {
        steps,
        static_check,
        execution,
    }
}

/// 一次步执行（await 收口）。
async fn run_step(steps: &LocalToolSteps, command: ToolCommand) -> ToolStepOutput {
    steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command,
        })
        .await
        .expect("步执行应成功")
}

/// 七臂驱动（真实 store 落库链）：建档种子后按状态机合法序各驱动一命令，
/// 全臂 Ok 收口后回读本 run 审计行（store 读半边）。
async fn drive_seven_arms(db: &TestDb, run_id: &str) -> Vec<StepStateRecord> {
    let assembled = assemble(
        db.store_arc(),
        run_id,
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );
    // ① phase_next（只读路由；建档零条目 → 首相位 proposal）
    let next = run_step(
        &assembled.steps,
        ToolCommand::PhaseNext {
            change: CHANGE.to_owned(),
            run_id: run_id.to_owned(),
        },
    )
    .await;
    assert!(matches!(next, ToolStepOutput::PhaseNext(_)));
    // ② phase_start proposal → ③ phase_log（evaluator 槽位在场）
    run_step(
        &assembled.steps,
        ToolCommand::PhaseStart {
            change: CHANGE.to_owned(),
            phase: "proposal".to_owned(),
        },
    )
    .await;
    run_step(
        &assembled.steps,
        ToolCommand::PhaseLog {
            change: CHANGE.to_owned(),
            phase: "proposal".to_owned(),
            input: PhaseLogInput {
                phase: "proposal".to_owned(),
                report: "提案已过".to_owned(),
                checklist: vec![ChecklistItem {
                    item: "验收标准在场".to_owned(),
                    pass: true,
                    evidence: "proposal.md 含验收节".to_owned(),
                }],
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: Some("sess-eval-1".to_owned()),
                decision_session_id: None,
            },
        },
    )
    .await;
    // ④ phase_start dev-design → ⑤ phase_log（仅 executor 槽位）
    run_step(
        &assembled.steps,
        ToolCommand::PhaseStart {
            change: CHANGE.to_owned(),
            phase: "dev-design".to_owned(),
        },
    )
    .await;
    run_step(
        &assembled.steps,
        ToolCommand::PhaseLog {
            change: CHANGE.to_owned(),
            phase: "dev-design".to_owned(),
            input: PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "设计已过".to_owned(),
                checklist: vec![ChecklistItem {
                    item: "任务拆解齐备".to_owned(),
                    pass: true,
                    evidence: "tasks.md 全勾选".to_owned(),
                }],
                skipped: false,
                executor_session_id: Some("sess-exec-2".to_owned()),
                evaluator_session_id: None,
                decision_session_id: None,
            },
        },
    )
    .await;
    // ⑥ backtrack dev-design → proposal（白名单随行）
    run_step(
        &assembled.steps,
        ToolCommand::Backtrack {
            change: CHANGE.to_owned(),
            phase: "dev-design".to_owned(),
            input: BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "proposal".to_owned(),
                reason: "提案缺验收标准".to_owned(),
                allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
            },
        },
    )
    .await;
    // ⑦ decision_log（决策会话槽位挂账）
    run_step(
        &assembled.steps,
        ToolCommand::DecisionLog {
            change: CHANGE.to_owned(),
            phase: "dev-design".to_owned(),
            session_id: "sess-decision".to_owned(),
        },
    )
    .await;
    // ⑧ static_check / ⑨ test_execution（spawn 缝委托）
    run_step(&assembled.steps, ToolCommand::StaticCheck).await;
    run_step(
        &assembled.steps,
        ToolCommand::TestExecution {
            change: CHANGE.to_owned(),
        },
    )
    .await;

    // StaticCheck 臂命令载荷无 change 位：审计行 change 以空串占位（run_id 仍
    // 串链），合并两桶回读后按行 id 还原时间序
    let mut rows = db
        .store
        .list_change_steps(CHANGE, Some(run_id))
        .expect("读审计行应成功");
    rows.extend(
        db.store
            .list_change_steps("", Some(run_id))
            .expect("读审计行应成功"),
    );
    rows.sort_by_key(|row| row.id);
    rows
}

// ---------------------------------------------------------------------------
// 七臂审计落库：每臂一条 StepRecord、封闭集、run_id 串链、失败臂皆落行
// ---------------------------------------------------------------------------

/// 七臂审计落库：假 runner + 真实 store 驱动七臂各一命令 → 每臂一条
/// StepRecord（step_kind 封闭集七值齐、run_id 串链、timestamp / status /
/// summary 齐），成功臂全 `ok`（AC-5 审计半边）。
#[tokio::test]
async fn 七臂审计落库_每臂一条steprecord且run_id串链() {
    let db = TestDb::open("seven-arms");
    seed_change(db.store.as_ref(), CHANGE, "requirement");

    let rows = drive_seven_arms(&db, "run-audit").await;

    assert_eq!(
        rows.len(),
        9,
        "九命令（七臂、phase_start / phase_log 各两入）"
    );
    let kinds: Vec<StepKind> = rows.iter().map(|row| row.step_kind).collect();
    assert_eq!(
        kinds,
        [
            StepKind::PhaseNext,
            StepKind::PhaseStart,
            StepKind::PhaseLog,
            StepKind::PhaseStart,
            StepKind::PhaseLog,
            StepKind::Backtrack,
            StepKind::DecisionLog,
            StepKind::StaticCheck,
            StepKind::TestExecution,
        ],
        "臂序与命令序逐一对应"
    );
    // 封闭集七值齐备（step_kind 词汇面）
    let mut words: Vec<&str> = rows.iter().map(|row| row.step_kind.as_str()).collect();
    words.sort_unstable();
    words.dedup();
    assert_eq!(
        words,
        [
            "backtrack",
            "decision_log",
            "phase_log",
            "phase_next",
            "phase_start",
            "static_check",
            "test_execution",
        ],
        "step_kind 封闭集七值齐备"
    );
    for row in &rows {
        assert_eq!(
            row.run_id,
            "run-audit",
            "run_id 串链: {}",
            row.step_kind.as_str()
        );
        assert_eq!(
            row.status,
            "ok",
            "成功臂 status ok: {}",
            row.step_kind.as_str()
        );
        assert!(
            row.timestamp > 0,
            "落行时刻在场: {}",
            row.step_kind.as_str()
        );
        assert!(
            !row.summary.is_empty(),
            "摘要在场: {}",
            row.step_kind.as_str()
        );
    }
    // 摘要词汇抽查（臂产出摘要格式面）
    assert!(
        rows[0].summary.starts_with("phase_next → "),
        "{}",
        rows[0].summary
    );
    assert_eq!(rows[7].summary, "static_check passed=true");
    assert_eq!(
        rows[8].summary, "test_execution conclusion=pass total=5 passed=4 failed=0 skipped=1",
        "test_execution 臂携结论 + 四计数摘要"
    );
}

/// 成功臂与失败臂皆落行：runner Err 臂与写面 Err 臂（未建档 change 开相）各
/// 落 status `error` 审计行，摘要携错误串、reference 缺位（AC-5 失败半边）。
#[tokio::test]
async fn 成功臂与失败臂皆落行_error行携记因() {
    let db = TestDb::open("fail-rows");
    seed_change(db.store.as_ref(), CHANGE, "requirement");
    let assembled = assemble(
        db.store_arc(),
        "run-fail",
        FakeRunner::erring("static-check 命令拉起失败"),
        FakeTestExecutionRunner::passing(),
    );

    // 失败臂一：runner Err → 步 Err，审计行 status error 摘要携错误串
    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::StaticCheck,
        })
        .await
        .expect_err("runner Err 应透传");
    assert_eq!(err, "static-check 命令拉起失败");

    // 失败臂二：写面 Err（未建档 change 开相）→ 步 Err，审计行同式落行
    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::PhaseStart {
                change: "不存在的-change".to_owned(),
                phase: "proposal".to_owned(),
            },
        })
        .await
        .expect_err("未建档 change 应 Err");
    assert!(err.contains("未建档"), "写面记因透传: {err}");

    // 错误行按 change 归位（StaticCheck 臂命令无 change 位以空串占位）
    let check_rows = db
        .store
        .list_change_steps("", Some("run-fail"))
        .expect("读审计行应成功");
    assert_eq!(check_rows.len(), 1);
    assert_eq!(check_rows[0].step_kind, StepKind::StaticCheck);
    assert_eq!(check_rows[0].status, "error");
    assert_eq!(check_rows[0].summary, "static-check 命令拉起失败");
    assert_eq!(check_rows[0].reference, None);

    let start_rows = db
        .store
        .list_change_steps("不存在的-change", Some("run-fail"))
        .expect("读审计行应成功");
    assert_eq!(start_rows.len(), 1);
    assert_eq!(start_rows[0].step_kind, StepKind::PhaseStart);
    assert_eq!(start_rows[0].status, "error");
    assert!(
        start_rows[0].summary.contains("未建档"),
        "错误串摘要留痕: {}",
        start_rows[0].summary
    );
}

// ---------------------------------------------------------------------------
// 全链落库组合：命令 → 写面 → db（PhaseRecord 与 StepRecord 同库可查）
// ---------------------------------------------------------------------------

/// 真实 Store + 真实写面：phase_log 臂驱动后 PhaseRecord 与 StepRecord 同库
/// 可查；phase_next / phase_start / backtrack 臂落库可达；backtrack 的 stale
/// 翻转与回跳标记同库在场（AC-5「全部落库」进程内证据——链路入口组合用例）。
#[tokio::test]
async fn 全链落库组合_phaserecord与steprecord同库可查() {
    let db = TestDb::open("full-chain");
    seed_change(db.store.as_ref(), CHANGE, "requirement");
    let assembled = assemble(
        db.store_arc(),
        "run-chain",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    // 命令链：phase_next → phase_start → phase_log（proposal）→ phase_start →
    // phase_log（dev-design）→ backtrack（回跳目标 proposal）
    run_step(
        &assembled.steps,
        ToolCommand::PhaseNext {
            change: CHANGE.to_owned(),
            run_id: "run-chain".to_owned(),
        },
    )
    .await;
    run_step(
        &assembled.steps,
        ToolCommand::PhaseStart {
            change: CHANGE.to_owned(),
            phase: "proposal".to_owned(),
        },
    )
    .await;
    run_step(
        &assembled.steps,
        ToolCommand::PhaseLog {
            change: CHANGE.to_owned(),
            phase: "proposal".to_owned(),
            input: PhaseLogInput {
                phase: "proposal".to_owned(),
                report: "提案已过".to_owned(),
                checklist: Vec::new(),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: Some("sess-eval-1".to_owned()),
                decision_session_id: None,
            },
        },
    )
    .await;
    run_step(
        &assembled.steps,
        ToolCommand::PhaseStart {
            change: CHANGE.to_owned(),
            phase: "dev-design".to_owned(),
        },
    )
    .await;
    run_step(
        &assembled.steps,
        ToolCommand::PhaseLog {
            change: CHANGE.to_owned(),
            phase: "dev-design".to_owned(),
            input: PhaseLogInput {
                phase: "dev-design".to_owned(),
                report: "设计已过".to_owned(),
                checklist: Vec::new(),
                skipped: false,
                executor_session_id: Some("sess-exec-2".to_owned()),
                evaluator_session_id: None,
                decision_session_id: None,
            },
        },
    )
    .await;
    run_step(
        &assembled.steps,
        ToolCommand::Backtrack {
            change: CHANGE.to_owned(),
            phase: "dev-design".to_owned(),
            input: BacktrackInput {
                phase: "dev-design".to_owned(),
                to: "proposal".to_owned(),
                reason: "提案缺验收标准".to_owned(),
                allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
            },
        },
    )
    .await;

    // PhaseRecord 同库可查：两条 pass 条目 + 回跳翻转（发起相位 dev-design 亦
    // 在目标 proposal 的 dependents 闭包内——当前实现闭包翻转行后写覆写同主键
    // 标记行，backtrack_to 标记不保留，此处只锚定 stale 稳定面；差异注记见
    // test-gen 报告「实现差异」节）
    let entries = db
        .store
        .list_phase_records(CHANGE)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 2, "两相位各一条");
    assert_eq!(entries[0].phase, "proposal");
    assert_eq!(entries[0].verdict, Verdict::Pass);
    assert!(entries[0].stale, "回跳目标 pass 条目置 stale");
    assert_eq!(entries[1].phase, "dev-design");
    assert!(entries[1].stale, "闭包内发起相位条目随回跳翻转");
    let record = db
        .store
        .find_change_record(CHANGE)
        .expect("读建档记录应成功")
        .expect("建档记录在场");
    assert!(record.active_phase.is_none(), "落账清位随命令链在场");

    // StepRecord 同库可查：六命令六行，臂序与命令序一致
    let rows = db
        .store
        .list_change_steps(CHANGE, None)
        .expect("读审计行应成功");
    let kinds: Vec<StepKind> = rows.iter().map(|row| row.step_kind).collect();
    assert_eq!(
        kinds,
        [
            StepKind::PhaseNext,
            StepKind::PhaseStart,
            StepKind::PhaseLog,
            StepKind::PhaseStart,
            StepKind::PhaseLog,
            StepKind::Backtrack,
        ],
        "phase_next / phase_start / phase_log / backtrack 臂落库可达"
    );
}

// ---------------------------------------------------------------------------
// reference 随行：报告目录 / 会话 id 随臂携带
// ---------------------------------------------------------------------------

/// reference 随行：phase_log 臂携会话 id（evaluator 槽位优先，缺位回退
/// executor）、decision_log 臂携决策会话 id、test_execution 臂携报告目录；
/// static_check 与相位机无引用臂 reference=None。
#[tokio::test]
async fn reference随行_报告目录与会话id随臂携带() {
    let db = TestDb::open("reference");
    seed_change(db.store.as_ref(), CHANGE, "requirement");

    let rows = drive_seven_arms(&db, "run-ref").await;

    // 相位机无引用臂（phase_next / phase_start / backtrack）与 static_check
    for row in rows.iter().filter(|row| {
        matches!(
            row.step_kind,
            StepKind::PhaseNext
                | StepKind::PhaseStart
                | StepKind::Backtrack
                | StepKind::StaticCheck
        )
    }) {
        assert_eq!(row.reference, None, "{} 臂无引用", row.step_kind.as_str());
    }
    // phase_log 臂：evaluator 槽位优先（首条），缺位回退 executor（次条）
    let log_rows: Vec<&StepStateRecord> = rows
        .iter()
        .filter(|row| row.step_kind == StepKind::PhaseLog)
        .collect();
    assert_eq!(
        log_rows[0].reference.as_deref(),
        Some("sess-eval-1"),
        "evaluator 会话 id 优先随行"
    );
    assert_eq!(
        log_rows[1].reference.as_deref(),
        Some("sess-exec-2"),
        "evaluator 缺位回退 executor 会话 id"
    );
    // decision_log 臂：决策会话 id 随行
    let decision = rows
        .iter()
        .find(|row| row.step_kind == StepKind::DecisionLog)
        .expect("decision_log 行在场");
    assert_eq!(decision.reference.as_deref(), Some("sess-decision"));
    // test_execution 臂：checks 报告目录随行（全量 findings 定位引用）
    let execution = rows
        .iter()
        .find(|row| row.step_kind == StepKind::TestExecution)
        .expect("test_execution 行在场");
    assert_eq!(
        execution.reference.as_deref(),
        Some("reports/test/app_node-test")
    );
}

// ---------------------------------------------------------------------------
// 摘要截断（D10）：≤500 原样透传，超长截断留痕
// ---------------------------------------------------------------------------

/// 臂产出摘要超 500 字符（假 runner 注入超长 Err 记因）→ 落库行截断留痕
///（`chars().count()` 口径，前 500 字符 + `…（截断，共 N 字符）` 后缀）；恰
/// 500 字符不截断。
#[tokio::test]
async fn 摘要截断_超五百字符截断留痕且恰五百不截断() {
    let fake = Arc::new(FakeStore::new());
    let long_message = "错".repeat(600);
    let boundary_message = "x".repeat(500);
    let assembled = assemble(
        Arc::clone(&fake) as Arc<dyn ChangeStateStore>,
        "run-clip",
        FakeRunner::erring(&boundary_message),
        FakeTestExecutionRunner::failing(&long_message),
    );

    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::TestExecution {
                change: CHANGE.to_owned(),
            },
        })
        .await
        .expect_err("runner Err 应透传");
    assert_eq!(err, long_message);
    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::StaticCheck,
        })
        .await
        .expect_err("runner Err 应透传");
    assert_eq!(err, boundary_message);

    let rows = fake.steps();
    let execution = rows
        .iter()
        .find(|row| row.step_kind == StepKind::TestExecution)
        .expect("test_execution 审计行在场");
    assert_eq!(execution.status, "error");
    assert_eq!(
        execution.summary,
        format!("{}…（截断，共 600 字符）", "错".repeat(500)),
        "超长摘要截断留痕（前 500 字符 + N 全长后缀）"
    );
    assert_eq!(
        execution.summary.chars().count(),
        500 + "…（截断，共 600 字符）".chars().count(),
        "截断按 chars().count() 口径"
    );

    let check = rows
        .iter()
        .find(|row| row.step_kind == StepKind::StaticCheck)
        .expect("static_check 审计行在场");
    assert_eq!(check.summary, boundary_message, "恰 500 字符原样透传不截断");
    assert_eq!(check.summary.chars().count(), 500);
}

// ---------------------------------------------------------------------------
// store 故障：业务 Err 记因上抛不静默，审计失败不阻断臂
// ---------------------------------------------------------------------------

/// 假件 store 注入 `StoreFault::Conflict`（log_phase 缝）→ 落账臂 Err 记因
/// 上抛不静默，审计 error 行照常落行（append_step 缝未故障）。
#[tokio::test]
async fn store故障_落账臂err记因上抛不静默() {
    let fake = Arc::new(FakeStore::new());
    fake.fail_log_phase(StoreFault::Conflict(
        "评估条目已存在: demo-change/proposal/1".to_owned(),
    ));
    fake.create_change_record(ChangeStateRecord {
        name: CHANGE.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: TS_BASE,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    })
    .expect("建档种子应成功");
    let assembled = assemble(
        Arc::clone(&fake) as Arc<dyn ChangeStateStore>,
        "run-fault",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    run_step(
        &assembled.steps,
        ToolCommand::PhaseStart {
            change: CHANGE.to_owned(),
            phase: "proposal".to_owned(),
        },
    )
    .await;
    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::PhaseLog {
                change: CHANGE.to_owned(),
                phase: "proposal".to_owned(),
                input: PhaseLogInput {
                    phase: "proposal".to_owned(),
                    report: "落账应失败".to_owned(),
                    checklist: Vec::new(),
                    skipped: false,
                    executor_session_id: None,
                    evaluator_session_id: None,
                    decision_session_id: None,
                },
            },
        })
        .await
        .expect_err("store fault 应记因上抛");
    assert!(err.contains("conflict"), "StoreFault 记因透传: {err}");

    // 审计行照常落行（故障臂 status error + 记因摘要）
    let rows = fake.steps();
    assert_eq!(rows.len(), 2, "phase_start ok 行 + phase_log error 行");
    assert_eq!(rows[1].step_kind, StepKind::PhaseLog);
    assert_eq!(rows[1].status, "error");
    assert!(
        rows[1].summary.contains("conflict"),
        "记因摘要: {}",
        rows[1].summary
    );
}

/// 审计落行失败不吞业务结果：假件 store `append_step` 缝注入 `StoreFault::
/// Db` → 臂命令仍返回自身 Ok 产出（审计 best-effort——`let _ =` 语义的进程内
/// 证据），审计行缺席。
#[tokio::test]
async fn store故障_审计落行失败不阻断臂业务结果() {
    let fake = Arc::new(FakeStore::new());
    fake.fail_append_step(StoreFault::Db("写步骤审计行失败".to_owned()));
    fake.create_change_record(ChangeStateRecord {
        name: CHANGE.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: TS_BASE,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    })
    .expect("建档种子应成功");
    let assembled = assemble(
        Arc::clone(&fake) as Arc<dyn ChangeStateStore>,
        "run-audit-fault",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    let next = run_step(
        &assembled.steps,
        ToolCommand::PhaseNext {
            change: CHANGE.to_owned(),
            run_id: "run-audit-fault".to_owned(),
        },
    )
    .await;
    match next {
        ToolStepOutput::PhaseNext(outcome) => {
            assert_eq!(outcome.next_phase.as_deref(), Some("proposal"));
        }
        other => panic!("产出应为 PhaseNext 变体，实际: {other:?}"),
    }
    let check = run_step(&assembled.steps, ToolCommand::StaticCheck).await;
    assert!(
        matches!(check, ToolStepOutput::StaticCheck(ref outcome) if outcome.passed),
        "runner 臂业务结果不受审计失败影响"
    );

    assert!(fake.steps().is_empty(), "审计行全部缺席（写入失败被吞）");
}

/// 假件 store 读路径注入 `StoreFault`（get_change / list_phase_records 缝）
/// → phase_next 臂 Err 记因上抛不静默（只读路由的故障传播面）。
#[tokio::test]
async fn store故障_读路径fault经臂err上抛() {
    let fake = Arc::new(FakeStore::new());
    fake.fail_read(StoreFault::Db("读取建档记录失败".to_owned()));
    let assembled = assemble(
        Arc::clone(&fake) as Arc<dyn ChangeStateStore>,
        "run-read-fault",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::PhaseNext {
                change: CHANGE.to_owned(),
                run_id: "run-read-fault".to_owned(),
            },
        })
        .await
        .expect_err("读路径 fault 应记因上抛");
    assert!(err.contains("db:"), "StoreFault Display 记因透传: {err}");
}

// ---------------------------------------------------------------------------
// 直调面持衡：ToolStepPort 直调语义不变（五参构造迁移）
// ---------------------------------------------------------------------------

/// `ToolCommand.PhaseNext → workflow::write`：真实 LocalToolSteps + 真实
/// store——路由产出与直调 phase_next 逐字段一致（进程内直调的动态证据）。
#[tokio::test]
async fn phase_next步链路直调写面产出与直调一致() {
    let db = TestDb::open("direct-next");
    seed_change(db.store.as_ref(), CHANGE, "requirement");
    let assembled = assemble(
        db.store_arc(),
        "run-1",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    let output = run_step(
        &assembled.steps,
        ToolCommand::PhaseNext {
            change: CHANGE.to_owned(),
            run_id: "run-1".to_owned(),
        },
    )
    .await;
    let outcome = match output {
        ToolStepOutput::PhaseNext(boxed) => *boxed,
        other => panic!("产出应为 PhaseNext 变体，实际: {other:?}"),
    };
    assert_eq!(outcome.next_phase.as_deref(), Some("proposal"));
    assert!(!outcome.done);

    // 与直调写面逐字段一致（同 store、独立锚点实例——进程内直调证据）
    let direct = workflow::write::phase_next(
        db.store.as_ref(),
        CHANGE,
        "run-direct",
        &SessionAnchors::new(),
    )
    .expect("直调应成功");
    assert_eq!(outcome, direct, "经缝透传不变形");
}

/// `PhaseStart → active_phase 落库`：命令 → store active_phase 定点写入、
/// ToolStepOutput::PhaseStart 承接（attempt 事务内推导直透）。
#[tokio::test]
async fn phase_start步链路开相落库active_phase() {
    let db = TestDb::open("direct-start");
    seed_change(db.store.as_ref(), CHANGE, "requirement");
    let assembled = assemble(
        db.store_arc(),
        "run-1",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    let output = run_step(
        &assembled.steps,
        ToolCommand::PhaseStart {
            change: CHANGE.to_owned(),
            phase: "implement".to_owned(),
        },
    )
    .await;
    let outcome = match output {
        ToolStepOutput::PhaseStart(outcome) => outcome,
        other => panic!("产出应为 PhaseStart 变体，实际: {other:?}"),
    };
    assert_eq!(outcome.phase, "implement");
    assert_eq!(outcome.attempt, 1);
    assert!(outcome.start_at > 0, "start_at i64 毫秒直透");

    let record = db
        .store
        .find_change_record(CHANGE)
        .expect("读建档记录应成功")
        .expect("建档记录在场");
    let active = record.active_phase.expect("active_phase 落库");
    assert_eq!(active.phase, "implement");
    assert_eq!(active.attempt, 1);
    assert_eq!(
        active.start_at, outcome.start_at,
        "start_at 与 outcome 一致"
    );
}

/// `PhaseLog → 评估条目追加落库`：命令携 PhaseLogInput → PhaseRecord 行落库、
/// ToolStepOutput::PhaseLog 承接（attempt 纯追加语义经缝透传不变形）。
#[tokio::test]
async fn phase_log步链路追加评估条目() {
    let db = TestDb::open("direct-log");
    seed_change(db.store.as_ref(), CHANGE, "requirement");
    seed_entry(
        db.store.as_ref(),
        CHANGE,
        "proposal",
        Verdict::Pass,
        TS_BASE,
    );
    let assembled = assemble(
        db.store_arc(),
        "run-1",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    run_step(
        &assembled.steps,
        ToolCommand::PhaseStart {
            change: CHANGE.to_owned(),
            phase: "proposal".to_owned(),
        },
    )
    .await;
    let output = run_step(
        &assembled.steps,
        ToolCommand::PhaseLog {
            change: CHANGE.to_owned(),
            phase: "proposal".to_owned(),
            input: PhaseLogInput {
                phase: "proposal".to_owned(),
                report: "重评通过".to_owned(),
                checklist: Vec::new(),
                skipped: false,
                executor_session_id: None,
                evaluator_session_id: None,
                decision_session_id: None,
            },
        },
    )
    .await;
    let outcome = match output {
        ToolStepOutput::PhaseLog(outcome) => outcome,
        other => panic!("产出应为 PhaseLog 变体，实际: {other:?}"),
    };
    // 纯追加：既有 pass 条目在场仍追加、attempt = 既有条目数 + 1
    assert_eq!(outcome.attempt, 2, "经缝透传不变形（纯追加）");

    let entries = db
        .store
        .list_phase_records(CHANGE)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 2, "纯追加：不覆盖历史条目");
    assert_eq!(entries[1].report, "重评通过");
    assert_eq!(entries[1].verdict, Verdict::Pass, "空 checklist 推导 pass");
    assert_eq!(entries[1].attempt, 2);
    let record = db
        .store
        .find_change_record(CHANGE)
        .expect("读建档记录应成功")
        .expect("建档记录在场");
    assert!(record.active_phase.is_none(), "落账后 active_phase 清位");
}

/// `Backtrack → 回跳标记与 stale 翻转落库`：命令携 BacktrackInput（allowed
/// 随行）→ 发起相位最新条目 backtrack_to / backtrack_reason 标记落库、目标
/// 相位 pass 条目置 stale、ToolStepOutput::Backtrack 承接。（发起相位取
/// acceptance / 目标取 test-design：二者不在彼此 stale 闭包内——标记行与
/// 翻转行不重叠，语义可逐字锚定；重叠形态的实现差异见全链落库组合行注记。）
#[tokio::test]
async fn backtrack步链路落库stale标记() {
    let db = TestDb::open("direct-backtrack");
    seed_change(db.store.as_ref(), CHANGE, "requirement");
    seed_entry(
        db.store.as_ref(),
        CHANGE,
        "proposal",
        Verdict::Pass,
        TS_BASE,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE,
        "dev-design",
        Verdict::Pass,
        TS_BASE + 60_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE,
        "test-design",
        Verdict::Pass,
        TS_BASE + 120_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE,
        "acceptance",
        Verdict::Fail,
        TS_BASE + 180_000,
    );
    let assembled = assemble(
        db.store_arc(),
        "run-1",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    let output = run_step(
        &assembled.steps,
        ToolCommand::Backtrack {
            change: CHANGE.to_owned(),
            phase: "acceptance".to_owned(),
            input: BacktrackInput {
                phase: "acceptance".to_owned(),
                to: "test-design".to_owned(),
                reason: "测试设计缺诊断树分支".to_owned(),
                allowed: vec![
                    "proposal".to_owned(),
                    "dev-design".to_owned(),
                    "test-design".to_owned(),
                    "implement".to_owned(),
                    "test-gen".to_owned(),
                    "test-execution".to_owned(),
                    "code-review".to_owned(),
                    "acceptance".to_owned(),
                ],
            },
        },
    )
    .await;
    let outcome = match output {
        ToolStepOutput::Backtrack(outcome) => outcome,
        other => panic!("产出应为 Backtrack 变体，实际: {other:?}"),
    };
    assert_eq!(outcome.phase, "acceptance");
    assert_eq!(outcome.target, "test-design");

    let entries = db
        .store
        .list_phase_records(CHANGE)
        .expect("读相位条目应成功");
    let acceptance = entries
        .iter()
        .find(|entry| entry.phase == "acceptance")
        .expect("发起相位条目在场");
    assert_eq!(
        acceptance.backtrack_to.as_deref(),
        Some("test-design"),
        "最新条目标记落库"
    );
    assert_eq!(
        acceptance.backtrack_reason.as_deref(),
        Some("测试设计缺诊断树分支")
    );
    let test_design = entries
        .iter()
        .find(|entry| entry.phase == "test-design")
        .expect("目标相位条目在场");
    assert!(test_design.stale, "目标相位 pass 条目置 stale");
    // 闭包外条目不受误伤（proposal / dev-design 非 test-design 下游）
    assert!(!entries[0].stale && !entries[1].stale, "无 stale 误伤");
}

/// StaticCheck 委托注入 runner：命令 → 注入假 runner 记录 root 透传、
/// StaticCheckOutcome 原样包装。
#[tokio::test]
async fn static_check步委托注入runner且root透传() {
    let fake = Arc::new(FakeStore::new());
    let assembled = assemble(
        Arc::clone(&fake) as Arc<dyn ChangeStateStore>,
        "run-1",
        FakeRunner::failing("clippy: 3 warnings\n1 error"),
        FakeTestExecutionRunner::passing(),
    );

    let output = run_step(&assembled.steps, ToolCommand::StaticCheck).await;
    match output {
        ToolStepOutput::StaticCheck(outcome) => {
            assert!(!outcome.passed);
            assert_eq!(
                outcome.diagnostics, "clippy: 3 warnings\n1 error",
                "产出原样包装"
            );
        }
        other => panic!("产出应为 StaticCheck 变体，实际: {other:?}"),
    }
    assert_eq!(
        assembled.static_check.roots.lock().expect("锁").as_slice(),
        [ROOT],
        "root 透传注入 runner（spawn cwd 语义）"
    );
}

/// steps 写面 Err 统一上抛：change 未建档 / 相位非法 / workflow_type 不受支持
/// → 步返回 Err(String)（无 ToolStepError 包装——port 换血对端）。
#[tokio::test]
async fn 写面err统一以err_string上抛() {
    let db = TestDb::open("direct-err");
    seed_change(db.store.as_ref(), CHANGE, "requirement");
    let assembled = assemble(
        db.store_arc(),
        "run-1",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    // change 不存在（db 无建档记录）
    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::PhaseNext {
                change: "不存在的-change".to_owned(),
                run_id: "run-1".to_owned(),
            },
        })
        .await
        .expect_err("未知 change 应 Err");
    assert!(!err.is_empty(), "Err(String) 显式：{err}");

    // 相位非法（workflow_type 合法但 phase 不在表）
    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::PhaseStart {
                change: CHANGE.to_owned(),
                phase: "幽灵相位".to_owned(),
            },
        })
        .await
        .expect_err("非法相位应 Err");
    assert!(err.contains("幽灵相位"), "错误透传写面记因：{err}");

    // workflow_type 非 requirement 同以 Err(String) 透传
    seed_change(db.store.as_ref(), "bad-type-change", "bug-fix");
    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::PhaseLog {
                change: "bad-type-change".to_owned(),
                phase: "proposal".to_owned(),
                input: PhaseLogInput {
                    phase: "proposal".to_owned(),
                    report: "r".to_owned(),
                    checklist: Vec::new(),
                    skipped: false,
                    executor_session_id: None,
                    evaluator_session_id: None,
                    decision_session_id: None,
                },
            },
        })
        .await
        .expect_err("非 requirement 应 Err");
    assert!(err.contains("bug-fix"), "W8 分层出口记因：{err}");
}

/// runner Err 透传：注入假 runner 返回 Err → StaticCheck 步 Err(String) 原样
///（反馈边升格的记因面）。
#[tokio::test]
async fn runner_err原样透传() {
    let fake = Arc::new(FakeStore::new());
    let assembled = assemble(
        Arc::clone(&fake) as Arc<dyn ChangeStateStore>,
        "run-1",
        FakeRunner::erring("static-check 命令拉起失败"),
        FakeTestExecutionRunner::passing(),
    );

    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::StaticCheck,
        })
        .await
        .expect_err("runner Err 应透传");
    assert_eq!(
        err, "static-check 命令拉起失败",
        "Err(String) 原样（无再包装）"
    );
}

/// 锚点实例复用语义：同一 LocalToolSteps 连续 PhaseNext（同 change, run_id）→
/// SessionAnchors 基线共享；换 run_id 基线独立（per-run 装配锚）。
#[tokio::test]
async fn 锚点实例随steps复用且run_id隔离() {
    let db = TestDb::open("direct-anchor");
    seed_change(db.store.as_ref(), CHANGE, "requirement");
    seed_entry(
        db.store.as_ref(),
        CHANGE,
        "proposal",
        Verdict::Pass,
        TS_BASE,
    );
    let assembled = assemble(
        db.store_arc(),
        "run-1",
        FakeRunner::passing(),
        FakeTestExecutionRunner::passing(),
    );

    // 同 (change, run_id)：基线 1 复用
    let first = match run_step(
        &assembled.steps,
        ToolCommand::PhaseNext {
            change: CHANGE.to_owned(),
            run_id: "run-a".to_owned(),
        },
    )
    .await
    {
        ToolStepOutput::PhaseNext(outcome) => *outcome,
        other => panic!("变体漂移: {other:?}"),
    };
    assert_eq!(first.round, 1, "首见登记基线 1");

    // run 期间落账 1 条 fail → 同 run 复用基线：round = 2
    seed_entry(
        db.store.as_ref(),
        CHANGE,
        "dev-design",
        Verdict::Fail,
        TS_BASE + 60_000,
    );
    let second = match run_step(
        &assembled.steps,
        ToolCommand::PhaseNext {
            change: CHANGE.to_owned(),
            run_id: "run-a".to_owned(),
        },
    )
    .await
    {
        ToolStepOutput::PhaseNext(outcome) => *outcome,
        other => panic!("变体漂移: {other:?}"),
    };
    assert_eq!(second.round, 2, "同 run 复用基线（steps 内锚点单例）");

    // 换 run_id：新窗口基线取当前条目数 → round 归位 1
    let fresh = match run_step(
        &assembled.steps,
        ToolCommand::PhaseNext {
            change: CHANGE.to_owned(),
            run_id: "run-b".to_owned(),
        },
    )
    .await
    {
        ToolStepOutput::PhaseNext(outcome) => *outcome,
        other => panic!("变体漂移: {other:?}"),
    };
    assert_eq!(fresh.round, 1, "换 run_id 基线独立（per-run 装配锚）");
}

/// 五参构造双 runner 注入：锚点 + static_check + test_execution + store + run_id
///（组合根同式装配的进程内证据半边；分发臂委托 root / change 透传）。
#[tokio::test]
async fn 五参构造双runner注入_分发臂委托() {
    let fake = Arc::new(FakeStore::new());
    let execution = FakeTestExecutionRunner::with_result(Ok(TestExecutionOutcome {
        conclusion: TestExecutionConclusion::Pass,
        total: 5,
        passed: 4,
        failed: 0,
        skipped: 1,
        findings_brief: "全部测试通过且覆盖率达阈值".to_owned(),
        report_dir: "reports/test/app_node-test".to_owned(),
    }));
    let assembled = assemble(
        Arc::clone(&fake) as Arc<dyn ChangeStateStore>,
        "run-1",
        FakeRunner::passing(),
        execution,
    );

    let output = run_step(
        &assembled.steps,
        ToolCommand::TestExecution {
            change: CHANGE.to_owned(),
        },
    )
    .await;

    match output {
        ToolStepOutput::TestExecution(outcome) => {
            assert_eq!(outcome.conclusion, TestExecutionConclusion::Pass);
            assert_eq!(
                (
                    outcome.total,
                    outcome.passed,
                    outcome.failed,
                    outcome.skipped
                ),
                (5, 4, 0, 1)
            );
            assert_eq!(outcome.report_dir, "reports/test/app_node-test");
        }
        other => panic!("产出应为 TestExecution 变体，实际: {other:?}"),
    }
    assert_eq!(
        assembled.execution.calls(),
        [(ROOT.to_owned(), CHANGE.to_owned())],
        "root / change 透传注入 runner（与 StaticCheck 臂同型）"
    );
}

/// TestExecutionRunner Err(String) 臂原样上抛（步层不吞错——run 显式失败面，
/// 经 run_tool 映射 run 终态的输入）。
#[tokio::test]
async fn test_execution_runner_err原样上抛() {
    let fake = Arc::new(FakeStore::new());
    let assembled = assemble(
        Arc::clone(&fake) as Arc<dyn ChangeStateStore>,
        "run-1",
        FakeRunner::passing(),
        FakeTestExecutionRunner::failing("报告子目录创建失败"),
    );

    let err = assembled
        .steps
        .run(ToolStepRequest {
            root: ROOT.to_owned(),
            command: ToolCommand::TestExecution {
                change: CHANGE.to_owned(),
            },
        })
        .await
        .expect_err("runner Err 应透传");
    assert_eq!(err, "报告子目录创建失败", "Err(String) 原样（无再包装）");
}
