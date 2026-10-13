use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent::{AgentRunStatus, ModelLevel};

use crate::control::ChangeFlowControl;
use crate::port::{
    ArchiveVcsPort, BoxToolFuture, BoxTurnFuture, RebaseOutcome, RunHistoryPort,
    StaticCheckOutcome, StaticCheckRunner, TestExecutionConclusion, TestExecutionOutcome,
    TestExecutionRunner, ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest,
    WorkerAgentPort, WorkerRole, WorkerTurnOutcome, WorkerTurnRequest, WorkflowSnapshotPort,
    WorktreeSnapshot,
};
use crate::snapshot::StoreSnapshot;
use crate::state::{ArchivePreflight, ChangeRunStatus, RunNotice};
use crate::steps::LocalToolSteps;
use crate::walker::{new_run_id, walk_run, RunRequest};
use store::Store;
use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand, RunFinishCommand,
    RunNodeEntry, RunStartCommand, RunStatus, RunStepStatus,
};
use workflow::write::{
    phase_table, ArtifactSpec, BacktrackOutcome, DecisionLogOutcome, LastResult, OnFail,
    PhaseLogOutcome, PhaseNextError, PhaseNextOutcome, PhaseStartOutcome, SessionAnchors,
    StepDefinition, StepNode,
};

// ---------------------------------------------------------------------------
// 装置：tempdir fixture + 真实 workspace 库（store 种子装置——workflow.json
// 夹具随双向墙退役，状态种子改经 store change 域操作面）
// ---------------------------------------------------------------------------

/// 临时 workspace 根 RAII。
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "orchestration-walker-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs_remove(&dir);
        Self(dir)
    }

    fn root_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    /// 预置一个 change 目录（active 树定位面；workflow.json 零产出——db 状态
    /// 单源，磁盘仅产物发现）+ 归档树父级（归档双写目录改名目标父级）。
    fn change(&self, name: &str) {
        let dir = self.0.join("openspec/changes").join(name);
        fs_create(&dir);
        fs_create(&self.0.join("openspec/changes/archive"));
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs_remove(&self.0);
    }
}

fn fs_remove(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::remove_dir_all(dir)
}

fn fs_create(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).expect("创建 change 目录失败");
}

/// 种子基准时刻：2026-10-01T08:00:00Z 定值 UTC unix 毫秒（确定性断言面）。
const TS_BASE: i64 = 1_790_841_600_000;

/// 真实 workspace 库装置：tempfile db 文件（store crate dev-dep 真件组合，
/// `Store::open_workspace` 即 `ChangeStateStore` 实现——进程边界真实组合，
/// 沿 workflow crate dev-dep store 先例）。
struct TestDb {
    /// store 句柄（字段声明先于 db 目录：drop 序先关库再删目录，Windows 句柄
    /// 纪律）
    store: Arc<Store>,
    _db_dir: tempfile::TempDir,
}

impl TestDb {
    fn open(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("orchestration-walker-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let store =
            Store::open_workspace(&db_dir.path().join("ws.redb")).expect("打开 workspace 库应成功");
        Self {
            store: Arc::new(store),
            _db_dir: db_dir,
        }
    }

    /// store 缝注入面（`Arc<dyn ChangeStateStore>` 类型擦除——LocalToolSteps /
    /// StoreSnapshot 组合根同式装配）。
    fn store_arc(&self) -> Arc<dyn ChangeStateStore> {
        Arc::clone(&self.store) as Arc<dyn ChangeStateStore>
    }
}

/// 建档种子：workflow_type requirement、active 起步（created_at 取定值毫秒；
/// id 归键 / name 独立展示——磁盘目录名恒 name）。
fn seed_change(store: &Store, id: &str, name: &str) {
    store
        .create_change_record(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: TS_BASE,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("建档种子应成功");
}

/// 评估条目种子：开相 + 落账一条（store change 域操作面种子路径，active_phase
/// 随落账清位——链路末态与真实 run 一致）。
fn seed_entry(
    store: &Store,
    change: &str,
    phase: &str,
    verdict: Verdict,
    report: &str,
    checklist: Vec<ChecklistItem>,
    ts: i64,
) {
    let started = store
        .start_change_phase(change, phase, ts)
        .expect("开相种子应成功");
    store
        .log_change_phase(&PhaseLogCommand {
            change_id: change.to_owned(),
            phase: phase.to_owned(),
            verdict,
            report: report.to_owned(),
            skipped: false,
            checklist,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
            start_at: Some(started.start_at),
            timestamp: ts + 30_000,
        })
        .expect("落账种子应成功");
}

/// 决策分叉族种子（store 半边）：proposal pass / dev-design pass / implement
/// fail 带 fail checklist——决策输入内容与既往 workflow.json fixture 同源。
fn seed_decision_fixture(store: &Store) {
    seed_change(store, CHANGE_ID, NAME);
    seed_entry(
        store,
        CHANGE_ID,
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        TS_BASE,
    );
    seed_entry(
        store,
        CHANGE_ID,
        "dev-design",
        Verdict::Pass,
        "设计齐备",
        Vec::new(),
        TS_BASE + 60_000,
    );
    seed_entry(
        store,
        CHANGE_ID,
        "implement",
        Verdict::Fail,
        "首轮未过",
        vec![ChecklistItem {
            item: "写面保形".to_owned(),
            pass: false,
            evidence: "custom_note 丢失".to_owned(),
        }],
        TS_BASE + 120_000,
    );
}

/// 决策分叉族装置：tempdir change 目录（active 树定位面）+ 真实库种子
///（决策输入取自 ChangeDetail 只读装配——db 读源真实组合）。
fn decision_fixture(tag: &str) -> (TempRoot, TestDb) {
    let root = TempRoot::new(tag);
    root.change(NAME);
    let db = TestDb::open(tag);
    seed_decision_fixture(db.store.as_ref());
    (root, db)
}

// ---------------------------------------------------------------------------
// 装置：WorkerAgentPort 假引擎（预录转录 + 捕获请求 + 可编程 Err / 停止时序）
// ---------------------------------------------------------------------------

/// 评估器默认 pass JSON（evaluator 队列空时的缺省产出；checklist.phase 由
/// walker 以当前相位覆盖语义消费，载荷不限相位）。
const PASS_JSON: &str = r#"{ "phase": "any", "attempt": 1, "verdict": "pass", "report": "通过", "checklist": [ { "item": "项", "pass": true, "evidence": "据" } ], "skipped": false }"#;

/// fail checklist JSON 模板。
fn fail_json(report: &str) -> String {
    format!(
        r#"{{ "phase": "any", "attempt": 1, "verdict": "fail", "report": "{report}", "checklist": [ {{ "item": "覆盖缺口", "pass": false, "evidence": "缺失行" }} ], "skipped": false }}"#
    )
}

/// evaluator 停等门（停止收敛用例的时序控制：armed 期间 evaluator future
/// 挂起，测试置位停止后放行）。
struct EvaluatorGate {
    armed: Mutex<bool>,
    notify: tokio::sync::Notify,
}

impl EvaluatorGate {
    fn armed() -> Arc<Self> {
        Arc::new(Self {
            armed: Mutex::new(true),
            notify: tokio::sync::Notify::new(),
        })
    }
}

/// WorkerAgentPort 假引擎：预录密封产出 + WorkerTurnRequest 全量捕获 +
/// 时间线登记；角色感知（executor / evaluator / decision 各自缺省产出）。
struct FakeWorker {
    timeline: Arc<Mutex<Vec<String>>>,
    requests: Arc<Mutex<Vec<WorkerTurnRequest>>>,
    /// (role, 派发 session id) 序（continue_session 断言面）
    sessions: Arc<Mutex<Vec<(WorkerRole, String)>>>,
    evaluator_reports: Mutex<VecDeque<Result<String, String>>>,
    decision_reports: Mutex<VecDeque<String>>,
    /// 归档两会话角色产出队列（spec_sync / resolve_conflicts 共用；`Ok` 文本
    /// 为收口消息，`Err` 为会话失败注入，耗尽回落 None 正常收口）
    archive_reports: Mutex<VecDeque<Result<String, String>>>,
    /// executor 会话失败注入队列（耗尽回落正常收口）：`Some` 携 api_error 记因
    /// 载荷，`None` 为失败但无记因（`Failed` 收敛回落通用文案面）——两者均
    /// 返回 `Failed` 收敛的密封转录
    executor_failures: Mutex<VecDeque<Option<String>>>,
    counter: AtomicUsize,
    gate: Option<Arc<EvaluatorGate>>,
}

impl FakeWorker {
    fn new(timeline: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            timeline: Arc::clone(timeline),
            requests: Arc::new(Mutex::new(Vec::new())),
            sessions: Arc::new(Mutex::new(Vec::new())),
            evaluator_reports: Mutex::new(VecDeque::new()),
            decision_reports: Mutex::new(VecDeque::new()),
            archive_reports: Mutex::new(VecDeque::new()),
            executor_failures: Mutex::new(VecDeque::new()),
            counter: AtomicUsize::new(0),
            gate: None,
        }
    }

    /// 归档两会话角色产出预录（spec_sync / resolve_conflicts 共用队列）。
    fn with_archive_reports(mut self, reports: Vec<Result<String, String>>) -> Self {
        self.archive_reports = Mutex::new(reports.into());
        self
    }

    /// executor 会话失败注入预录（`Some` 携 api_error 记因载荷，`None` 无记因）。
    fn with_executor_failures(mut self, causes: Vec<Option<String>>) -> Self {
        self.executor_failures = Mutex::new(causes.into());
        self
    }

    fn with_evaluator_reports(mut self, reports: Vec<Result<String, String>>) -> Self {
        self.evaluator_reports = Mutex::new(reports.into());
        self
    }

    fn with_decision_reports(mut self, reports: Vec<String>) -> Self {
        self.decision_reports = Mutex::new(reports.into());
        self
    }

    fn with_gate(mut self, gate: Arc<EvaluatorGate>) -> Self {
        self.gate = Some(gate);
        self
    }

    fn assemble(
        self,
    ) -> (
        Arc<dyn WorkerAgentPort>,
        Arc<Mutex<Vec<WorkerTurnRequest>>>,
        Arc<Mutex<Vec<(WorkerRole, String)>>>,
    ) {
        let requests = Arc::clone(&self.requests);
        let sessions = Arc::clone(&self.sessions);
        (Arc::new(self), requests, sessions)
    }
}

impl WorkerAgentPort for FakeWorker {
    fn run(&self, turn: WorkerTurnRequest) -> BoxTurnFuture {
        self.requests
            .lock()
            .expect("请求捕获锁不可中毒")
            .push(turn.clone());
        let role = turn.role;
        // continue_session 同会话续注回声（真实引擎 resume 同一核心会话 id 的
        // 假件对应形态——槽位值续注不漂移断言的装置面）
        let session_id = match turn.continue_session.as_deref() {
            Some(reused) => reused.to_owned(),
            None => format!("sess-{}", self.counter.fetch_add(1, Ordering::SeqCst) + 1),
        };
        self.sessions
            .lock()
            .expect("会话登记锁不可中毒")
            .push((role, session_id.clone()));
        self.timeline
            .lock()
            .expect("时间线锁不可中毒")
            .push(role.as_str().to_owned());

        let mut timeline = Arc::clone(&self.timeline);
        // 停等门仅挂 evaluator（停止时序窗口；executor / decision / 归档两会话
        // 角色不挂）
        let gate = match role {
            WorkerRole::Evaluator => self.gate.as_ref().map(Arc::clone),
            _ => None,
        };
        let evaluator = match role {
            WorkerRole::Evaluator => Some(
                self.evaluator_reports
                    .lock()
                    .expect("evaluator 队列锁不可中毒")
                    .pop_front(),
            ),
            _ => None,
        };
        let decision = match role {
            WorkerRole::Decision => Some(
                self.decision_reports
                    .lock()
                    .expect("decision 队列锁不可中毒")
                    .pop_front(),
            ),
            _ => None,
        };
        // 归档两会话角色可编程产出（spec_sync / resolve_conflicts：耗尽回落
        // None = 会话正常收口，无 verdict 语义）
        let archive_message = match role {
            WorkerRole::SpecSync | WorkerRole::ResolveConflicts => Some(
                self.archive_reports
                    .lock()
                    .expect("归档会话队列锁不可中毒")
                    .pop_front(),
            ),
            _ => None,
        };

        // executor 会话失败注入（耗尽回落 None = 会话正常收口）
        let executor_failure = match role {
            WorkerRole::Executor => Some(
                self.executor_failures
                    .lock()
                    .expect("executor 失败队列锁不可中毒")
                    .pop_front(),
            ),
            _ => None,
        };

        Box::pin(async move {
            // evaluator 停等门：armed 期间挂起（停止时序窗口；notify_one 带许可
            // 语义——放行通知早于 future 首轮 poll 亦不丢）
            if let Some(gate) = gate {
                if *gate.armed.lock().expect("门锁不可中毒") {
                    gate.notify.notified().await;
                }
            }
            let _ = &mut timeline;
            // executor 失败注入：Failed 收敛 + 携成因的密封转录（真实引擎
            // api_error 记因 + 失败 TurnDone 的同形形态；无记因时 TurnDone
            // subtype 置空——成因提取显式缺席的形态）
            if let Some(cause) = executor_failure.flatten() {
                let mut transcript = Vec::new();
                let subtype = match cause {
                    Some(cause) => {
                        transcript.push(agent::AgentEvent::stamp(
                            0,
                            agent::AgentEventKind::SystemNotice {
                                subtype: "api_error".to_owned(),
                                payload: serde_json::json!({ "error": cause }),
                            },
                        ));
                        "api_error"
                    }
                    None => "",
                };
                transcript.push(agent::AgentEvent::stamp(
                    1,
                    agent::AgentEventKind::TurnDone {
                        subtype: subtype.to_owned(),
                        is_error: true,
                        num_turns: None,
                        duration_ms: None,
                        cost_usd: None,
                        usage: serde_json::Value::Null,
                        session_id: None,
                    },
                ));
                return Ok(WorkerTurnOutcome {
                    session_id,
                    status: AgentRunStatus::Failed,
                    final_message: None,
                    transcript,
                });
            }
            let final_message = match role {
                WorkerRole::Executor => None,
                WorkerRole::Evaluator => match evaluator.flatten() {
                    Some(Ok(text)) => Some(text),
                    Some(Err(message)) => return Err(message),
                    None => Some(PASS_JSON.to_owned()),
                },
                WorkerRole::Decision => Some(
                    decision
                        .flatten()
                        .unwrap_or_else(|| r#"{ "action": "retry" }"#.to_owned()),
                ),
                WorkerRole::SpecSync | WorkerRole::ResolveConflicts => {
                    match archive_message.flatten() {
                        Some(Err(message)) => return Err(message),
                        Some(Ok(text)) => Some(text),
                        None => None,
                    }
                }
            };
            Ok(WorkerTurnOutcome {
                session_id,
                status: AgentRunStatus::Completed,
                final_message,
                transcript: Vec::new(),
            })
        })
    }
}

// ---------------------------------------------------------------------------
// 装置：ToolStepPort 假写面（预录 ToolStepOutput + 命令捕获 + 可编程 Err）
// ---------------------------------------------------------------------------

/// ToolStepPort 假写面（AC-1「假写面」口径主装置）：预录写面原生产出、记录
/// ToolCommand 调用序与载荷、时间线登记、可编程 Err(String)。
struct FakeTools {
    timeline: Arc<Mutex<Vec<String>>>,
    commands: Arc<Mutex<Vec<ToolCommand>>>,
    /// 收到的 step root 序（RunRequest.root 透传锚的捕获面——exec root 换源
    /// 后写面 root 恒随 request 的防漂移断言）
    roots: Arc<Mutex<Vec<String>>>,
    phase_next: Mutex<VecDeque<PhaseNextOutcome>>,
    static_check: Mutex<VecDeque<StaticCheckOutcome>>,
    /// test-execution 可编程产出序列（pass / fail / error 四态——耗尽回落
    /// pass 恒过）
    test_execution: Mutex<VecDeque<TestExecutionOutcome>>,
    /// 归档前置载荷可编程产出序列（耗尽回落 archive_preflight_payload）
    archive_preflight: Mutex<VecDeque<ArchivePreflight>>,
    /// 归档双写产出可编程序列（耗尽回落 archive_seal_outcome）
    archive_seal: Mutex<VecDeque<workflow::write::ArchiveOutcome>>,
    /// 命中即 Err 的步标签（phase-next / phase-start / phase-log / backtrack /
    /// decision-log / static-check / test-execution / archive-preflight /
    /// archive-seal）
    fail_on: Mutex<Option<String>>,
    attempt_counter: AtomicUsize,
}

impl FakeTools {
    fn new(timeline: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            timeline: Arc::clone(timeline),
            commands: Arc::new(Mutex::new(Vec::new())),
            roots: Arc::new(Mutex::new(Vec::new())),
            phase_next: Mutex::new(VecDeque::new()),
            static_check: Mutex::new(VecDeque::new()),
            test_execution: Mutex::new(VecDeque::new()),
            archive_preflight: Mutex::new(VecDeque::new()),
            archive_seal: Mutex::new(VecDeque::new()),
            fail_on: Mutex::new(None),
            attempt_counter: AtomicUsize::new(0),
        }
    }

    /// 归档前置载荷预录（停等门 / Preflight 步消费面；耗尽回落 legacy 形态）。
    fn with_archive_preflight(mut self, payloads: Vec<ArchivePreflight>) -> Self {
        self.archive_preflight = Mutex::new(payloads.into());
        self
    }

    /// 归档双写产出预录（Seal 步消费面；耗尽回落当日形态）。
    fn with_archive_seal(mut self, outcomes: Vec<workflow::write::ArchiveOutcome>) -> Self {
        self.archive_seal = Mutex::new(outcomes.into());
        self
    }

    /// roots 捕获柄（assemble 消费 self 前取——exec root 透传锚用例的观察面）。
    fn roots_handle(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.roots)
    }

    fn with_phase_next(mut self, outcomes: Vec<PhaseNextOutcome>) -> Self {
        self.phase_next = Mutex::new(outcomes.into());
        self
    }

    fn with_static_check(mut self, outcomes: Vec<StaticCheckOutcome>) -> Self {
        self.static_check = Mutex::new(outcomes.into());
        self
    }

    /// test-execution 产出序列预录（pass / fail / error 四态；用例由
    /// test-gen 阶段 walker_test 扩展节承接——本装置先行为反馈边扩展备妥）。
    #[allow(dead_code)]
    fn with_test_execution(mut self, outcomes: Vec<TestExecutionOutcome>) -> Self {
        self.test_execution = Mutex::new(outcomes.into());
        self
    }

    fn fail_on(self, label: &str) -> Self {
        *self.fail_on.lock().expect("fail 锁不可中毒") = Some(label.to_owned());
        self
    }

    fn assemble(self) -> (Arc<dyn ToolStepPort>, Arc<Mutex<Vec<ToolCommand>>>) {
        let commands = Arc::clone(&self.commands);
        (Arc::new(self), commands)
    }
}

fn command_label(command: &ToolCommand) -> &'static str {
    match command {
        ToolCommand::PhaseNext { .. } => "phase-next",
        ToolCommand::PhaseStart { .. } => "phase-start",
        ToolCommand::PhaseLog { .. } => "phase-log",
        ToolCommand::Backtrack { .. } => "backtrack",
        ToolCommand::DecisionLog { .. } => "decision-log",
        ToolCommand::StaticCheck { .. } => "static-check",
        ToolCommand::TestExecution { .. } => "test-execution",
        ToolCommand::ArchivePreflight { .. } => "archive-preflight",
        ToolCommand::ArchiveSeal { .. } => "archive-seal",
    }
}

impl ToolStepPort for FakeTools {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture {
        let label = command_label(&step.command);
        self.commands
            .lock()
            .expect("命令捕获锁不可中毒")
            .push(step.command.clone());
        self.roots
            .lock()
            .expect("root 捕获锁不可中毒")
            .push(step.root.clone());
        self.timeline
            .lock()
            .expect("时间线锁不可中毒")
            .push(label.to_owned());

        // 可编程 Err：命中标签即 Err(String)
        let fail = self
            .fail_on
            .lock()
            .expect("fail 锁不可中毒")
            .as_deref()
            .map(|target| target == label)
            .unwrap_or(false);
        if fail {
            return Box::pin(async move { Err(format!("{label} 假写面注入失败")) });
        }

        match step.command {
            ToolCommand::PhaseNext { .. } => {
                let outcome = self
                    .phase_next
                    .lock()
                    .expect("phase-next 队列锁不可中毒")
                    .pop_front()
                    .unwrap_or_else(done_outcome);
                Box::pin(async move { Ok(ToolStepOutput::PhaseNext(Box::new(outcome))) })
            }
            ToolCommand::PhaseStart { phase, .. } => {
                let attempt = self.attempt_counter.fetch_add(1, Ordering::SeqCst) as u32 + 1;
                let outcome = PhaseStartOutcome {
                    phase: phase.clone(),
                    attempt,
                    // 开相时刻 i64 UTC unix 毫秒（定值——确定性断言面）
                    start_at: TS_BASE,
                };
                Box::pin(async move { Ok(ToolStepOutput::PhaseStart(outcome)) })
            }
            ToolCommand::PhaseLog { phase, .. } => {
                let attempt = self.attempt_counter.load(Ordering::SeqCst) as u32;
                let outcome = PhaseLogOutcome {
                    phase: phase.clone(),
                    attempt: attempt.max(1),
                };
                Box::pin(async move { Ok(ToolStepOutput::PhaseLog(outcome)) })
            }
            ToolCommand::Backtrack { phase, input, .. } => {
                let outcome = BacktrackOutcome {
                    phase: phase.clone(),
                    target: input.to.clone(),
                };
                Box::pin(async move { Ok(ToolStepOutput::Backtrack(outcome)) })
            }
            // 决策会话槽位挂账臂：预录 DecisionLogOutcome（phase 随行；命令
            // 载荷已经 commands 捕获——payload 断言的事实源）
            ToolCommand::DecisionLog { phase, .. } => {
                let outcome = DecisionLogOutcome { phase };
                Box::pin(async move { Ok(ToolStepOutput::DecisionLog(outcome)) })
            }
            ToolCommand::StaticCheck { .. } => {
                let outcome = self
                    .static_check
                    .lock()
                    .expect("static-check 队列锁不可中毒")
                    .pop_front()
                    .unwrap_or(StaticCheckOutcome {
                        passed: true,
                        diagnostics: String::new(),
                        output_ref: None,
                    });
                Box::pin(async move { Ok(ToolStepOutput::StaticCheck(outcome)) })
            }
            // test-execution 可编程臂：预录产出序列逐次弹出（pass / fail /
            // error 四态），耗尽回落 pass 恒过——绿跑路径零扰动
            ToolCommand::TestExecution { .. } => {
                let outcome = self
                    .test_execution
                    .lock()
                    .expect("test-execution 队列锁不可中毒")
                    .pop_front()
                    .unwrap_or(TestExecutionOutcome {
                        conclusion: TestExecutionConclusion::Pass,
                        total: 0,
                        passed: 0,
                        failed: 0,
                        skipped: 0,
                        findings_brief: String::new(),
                        findings_detail: String::new(),
                        report_dir: String::new(),
                        exec_id: String::new(),
                    });
                Box::pin(async move { Ok(ToolStepOutput::TestExecution(outcome)) })
            }
            // 归档前置只读载荷臂：预录载荷逐次弹出（耗尽回落 legacy 主 root
            // 形态——worktree / branch / merge_target 缺省 None）
            ToolCommand::ArchivePreflight { .. } => {
                let payload = self
                    .archive_preflight
                    .lock()
                    .expect("archive-preflight 队列锁不可中毒")
                    .pop_front()
                    .unwrap_or_else(|| ArchivePreflight {
                        name: NAME.to_owned(),
                        completed: true,
                        incomplete_phases: Vec::new(),
                        missing_artifacts: Vec::new(),
                        delta_specs: Vec::new(),
                        worktree: None,
                        branch: None,
                        merge_target: None,
                    });
                Box::pin(async move { Ok(ToolStepOutput::ArchivePreflight(payload)) })
            }
            // 归档双写收口臂：预录产出逐次弹出（耗尽回落定值形态）
            ToolCommand::ArchiveSeal { .. } => {
                let outcome = self
                    .archive_seal
                    .lock()
                    .expect("archive-seal 队列锁不可中毒")
                    .pop_front()
                    .unwrap_or_else(|| workflow::write::ArchiveOutcome {
                        name: NAME.to_owned(),
                        archived_date: "2026-10-01".to_owned(),
                    });
                Box::pin(async move { Ok(ToolStepOutput::ArchiveSeal(outcome)) })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 装置：预录产出构造器与 run 驱动
// ---------------------------------------------------------------------------

fn route_outcome(phase: &str, allowed: &[&str]) -> PhaseNextOutcome {
    route_with_round(phase, allowed, 1, None)
}

/// 预录路由产出装置：executor / evaluator prompt 为**已组装**的静态主体
/// （`change: c` 上下文头在场——与 phase_next 组装面同形，消费面断言的事实源）；
/// `ResolvedPhaseSpec` 去 `agent_type`（编译期结构锚），档位由装置给定。
fn route_with_round(
    phase: &str,
    allowed: &[&str],
    round: u32,
    last_result: Option<LastResult>,
) -> PhaseNextOutcome {
    PhaseNextOutcome {
        done: false,
        next_phase: Some(phase.to_owned()),
        round,
        steps: steps_of(phase),
        artifact_manifest: manifest_of(phase),
        executor: Some(workflow::write::ResolvedPhaseSpec {
            prompt: format!("change: c\n\nImplement the {phase} phase for change \"c\"."),
            model_level: ModelLevel::Low,
        }),
        evaluator: Some(workflow::write::ResolvedPhaseSpec {
            prompt: format!("change: c\n\nEvaluate the {phase} phase for change \"c\"."),
            model_level: ModelLevel::High,
        }),
        allowed_backtrack_phases: allowed.iter().map(|id| id.to_string()).collect(),
        last_result,
        error: None,
    }
}

fn done_outcome() -> PhaseNextOutcome {
    PhaseNextOutcome {
        done: true,
        next_phase: None,
        round: 1,
        steps: Vec::new(),
        artifact_manifest: Vec::new(),
        executor: None,
        evaluator: None,
        allowed_backtrack_phases: Vec::new(),
        last_result: None,
        error: None,
    }
}

fn max_retries_outcome(phase: &str, allowed: &[&str], last_report: &str) -> PhaseNextOutcome {
    PhaseNextOutcome {
        done: false,
        next_phase: None,
        round: 6,
        steps: Vec::new(),
        artifact_manifest: Vec::new(),
        executor: None,
        evaluator: None,
        allowed_backtrack_phases: allowed.iter().map(|id| id.to_string()).collect(),
        last_result: Some(LastResult {
            phase: phase.to_owned(),
            verdict: Verdict::Fail,
            report: last_report.to_owned(),
            timestamp: None,
        }),
        error: Some(PhaseNextError::MaxRetriesExceeded {
            phase: phase.to_owned(),
            round: 6,
        }),
    }
}

fn passing_check() -> StaticCheckOutcome {
    StaticCheckOutcome {
        passed: true,
        diagnostics: String::new(),
        output_ref: None,
    }
}

fn failing_check(diagnostics: &str) -> StaticCheckOutcome {
    StaticCheckOutcome {
        passed: false,
        diagnostics: diagnostics.to_owned(),
        output_ref: None,
    }
}

// ---------------------------------------------------------------------------
// 装置：归档 vcs 缝假件（第三执行缝——git 步直调与探测的可编程返回值 + 调用
// 捕获 + Err 注入；真实进程 spawn 行为归 vcs-runtime crate 真件测试面）
// ---------------------------------------------------------------------------

/// 可编程归档 vcs 假件：脏探测 / 提交 / 合入判定 / 重放 / 快进 / 快照各面可
/// 编程，调用序与入参捕获供「git 步走 vcs 缝」与短路形态断言（core 零 spawn
/// 的结构前提：归档 git 步恒经本缝直调）。
struct FakeVcs {
    /// 调用序（`{label}({arg})` 形态；步分派与短路断言的观察面）
    calls: Mutex<Vec<String>>,
    dirty: Mutex<bool>,
    dirty_paths: Mutex<Vec<Vec<String>>>,
    branch: Mutex<Result<String, String>>,
    merged: Mutex<Result<bool, String>>,
    rebase: Mutex<Result<RebaseOutcome, String>>,
    rebase_continue: Mutex<Result<RebaseOutcome, String>>,
    commit_result: Mutex<Result<(), String>>,
    commit_paths_result: Mutex<Result<(), String>>,
    ff_merge_result: Mutex<Result<(), String>>,
    snapshot: Mutex<Result<WorktreeSnapshot, String>>,
    snapshot_head: Mutex<Option<String>>,
}

impl FakeVcs {
    /// 干净且可归档的默认形态（current_branch=main / 未合入 / 无冲突 / 提交可用）。
    fn clean() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            dirty: Mutex::new(false),
            dirty_paths: Mutex::new(Vec::new()),
            branch: Mutex::new(Ok("main".to_owned())),
            merged: Mutex::new(Ok(false)),
            rebase: Mutex::new(Ok(RebaseOutcome::Rebased)),
            rebase_continue: Mutex::new(Ok(RebaseOutcome::Rebased)),
            commit_result: Mutex::new(Ok(())),
            commit_paths_result: Mutex::new(Ok(())),
            ff_merge_result: Mutex::new(Ok(())),
            snapshot: Mutex::new(Ok(snapshot_of("head-a", None))),
            snapshot_head: Mutex::new(None),
        }
    }

    fn with_dirty(self, dirty: bool) -> Self {
        *self.dirty.lock().expect("vcs 脏锁不可中毒") = dirty;
        self
    }

    fn with_merged(self, merged: Result<bool, String>) -> Self {
        *self.merged.lock().expect("vcs 合入锁不可中毒") = merged;
        self
    }

    fn with_rebase(self, outcome: Result<RebaseOutcome, String>) -> Self {
        *self.rebase.lock().expect("vcs 重放锁不可中毒") = outcome;
        self
    }

    fn with_rebase_continue(self, outcome: Result<RebaseOutcome, String>) -> Self {
        *self.rebase_continue.lock().expect("vcs 续走锁不可中毒") = outcome;
        self
    }

    fn with_commit_result(self, result: Result<(), String>) -> Self {
        *self.commit_result.lock().expect("vcs 提交锁不可中毒") = result;
        self
    }

    fn with_commit_paths_result(self, result: Result<(), String>) -> Self {
        *self.commit_paths_result.lock().expect("vcs 落盘锁不可中毒") = result;
        self
    }

    fn with_ff_merge_result(self, result: Result<(), String>) -> Self {
        *self.ff_merge_result.lock().expect("vcs 快进锁不可中毒") = result;
        self
    }

    /// 解冲突后验用：B 快照 HEAD 与 rebase 半程标记可编程（后验四判据的注入面）。
    fn with_snapshot_head(self, head: &str, rebase_head: Option<&str>) -> Self {
        *self.snapshot_head.lock().expect("vcs 快照头锁不可中毒") = Some(head.to_owned());
        *self.snapshot.lock().expect("vcs 快照锁不可中毒") =
            Ok(snapshot_of(rebase_head.map_or("head-a", |_| head), rebase_head));
        self
    }

    /// 调用序快照。
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("vcs 调用锁不可中毒").clone()
    }

    /// 命中某调用标签次数。
    fn call_count(&self, prefix: &str) -> usize {
        self.calls()
            .iter()
            .filter(|call| call.starts_with(prefix))
            .count()
    }

    /// 记录路径参数（Finalize / Commit 的 pathspec 面断言）。
    fn dirty_path_sets(&self) -> Vec<Vec<String>> {
        self.dirty_paths.lock().expect("vcs 路径锁不可中毒").clone()
    }

    fn record(&self, label: String) {
        self.calls
            .lock()
            .expect("vcs 调用锁不可中毒")
            .push(label);
    }
}

/// 工作区快照 fixture（后验对比面：HEAD + rebase 半程标记 + 空状态面）。
fn snapshot_of(head: &str, rebase_head: Option<&str>) -> WorktreeSnapshot {
    WorktreeSnapshot {
        head: head.to_owned(),
        rebase_head: rebase_head.map(str::to_owned),
        status: Vec::new(),
        index: Vec::new(),
    }
}

impl ArchiveVcsPort for FakeVcs {
    fn dirty(&self, _root: &std::path::Path, paths: &[&str]) -> bool {
        self.record(format!("dirty:{}", paths.len()));
        self.dirty_paths
            .lock()
            .expect("vcs 路径锁不可中毒")
            .push(paths.iter().map(|path| (*path).to_owned()).collect());
        *self.dirty.lock().expect("vcs 脏锁不可中毒")
    }

    fn commit_all(&self, worktree: &std::path::Path, message: &str) -> Result<(), String> {
        self.record(format!("commit_all:{}:{message}", worktree.display()));
        self.commit_result
            .lock()
            .expect("vcs 提交锁不可中毒")
            .clone()
    }

    fn branch_merged(&self, _main_root: &std::path::Path, branch: &str) -> Result<bool, String> {
        self.record(format!("branch_merged:{branch}"));
        self.merged.lock().expect("vcs 合入锁不可中毒").clone()
    }

    fn rebase_branch(
        &self,
        _worktree: &std::path::Path,
        onto: &str,
    ) -> Result<RebaseOutcome, String> {
        self.record(format!("rebase_branch:{onto}"));
        self.rebase.lock().expect("vcs 重放锁不可中毒").clone()
    }

    fn ff_merge(&self, _main_root: &std::path::Path, branch: &str) -> Result<(), String> {
        self.record(format!("ff_merge:{branch}"));
        self.ff_merge_result
            .lock()
            .expect("vcs 快进锁不可中毒")
            .clone()
    }

    fn worktree_snapshot(&self, _root: &std::path::Path) -> Result<WorktreeSnapshot, String> {
        self.record("worktree_snapshot".to_owned());
        self.snapshot.lock().expect("vcs 快照锁不可中毒").clone()
    }

    fn rebase_continue(&self, _worktree: &std::path::Path) -> Result<RebaseOutcome, String> {
        self.record("rebase_continue".to_owned());
        self.rebase_continue
            .lock()
            .expect("vcs 续走锁不可中毒")
            .clone()
    }

    fn rebase_abort(&self, _worktree: &std::path::Path) -> Result<(), String> {
        self.record("rebase_abort".to_owned());
        Ok(())
    }

    fn current_branch(&self, _main_root: &std::path::Path) -> Result<String, String> {
        self.record("current_branch".to_owned());
        self.branch.lock().expect("vcs 分支锁不可中毒").clone()
    }

    fn commit_paths(
        &self,
        _main_root: &std::path::Path,
        paths: &[&str],
        message: &str,
    ) -> Result<(), String> {
        self.record(format!("commit_paths:{}:{message}", paths.len()));
        self.commit_paths_result
            .lock()
            .expect("vcs 落盘锁不可中毒")
            .clone()
    }
}

/// 固定 change **id** 字面量（注册表键 / `RunRequest.change_id` / provenance
/// 身份段 / db 归键——一切寻址以 id 为准）。
const CHANGE_ID: &str = "5c7a2e91-4b38-4d6f-a052-9e1c8b3d7f24";

/// 展示名（记录 `name` 属性：磁盘面供给值——`openspec/changes/<name>` 目录与
/// 会话 cwd 路径；id ≠ name 形态下身份段与磁盘面逐点可辨）。
const NAME: &str = "walker-change";

/// 复合键 workspace root 段（测试固定值）
const ROOT: &str = "/ws/root-a";

/// run 落库缝假件（机械随动基设）：run_started / run_finished 命令捕获面——
/// 终态与记因断言的事实源（broadcast 已 kind-only 降位，载荷断言经本捕获面
/// 承载；写事务计数断言留测试相位）。
struct RecordingHistory {
    started: Mutex<Vec<RunStartCommand>>,
    finished: Mutex<Vec<RunFinishCommand>>,
    /// 可编程 Err 注入（fail-fast / best-effort 分流用例）：命中即 Err(串)。
    fail_started: Mutex<Option<String>>,
    fail_finished: Mutex<Option<String>>,
}

impl RecordingHistory {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Mutex::new(Vec::new()),
            finished: Mutex::new(Vec::new()),
            fail_started: Mutex::new(None),
            fail_finished: Mutex::new(None),
        })
    }

    /// 起点 first-write Err 注入（walker fail-fast 分流面）。
    fn arm_start_failure(&self, reason: &str) {
        *self.fail_started.lock().expect("fail 锁不可中毒") = Some(reason.to_owned());
    }

    /// 收口 second-write Err 注入（walker best-effort 分流面）。
    fn arm_finish_failure(&self, reason: &str) {
        *self.fail_finished.lock().expect("fail 锁不可中毒") = Some(reason.to_owned());
    }

    /// run 域写调用计数（「每 run 恰两写」断言面）。
    fn start_count(&self) -> usize {
        self.started.lock().expect("started 锁不可中毒").len()
    }

    fn finish_count(&self) -> usize {
        self.finished.lock().expect("finished 锁不可中毒").len()
    }

    /// 起始行捕获（started_at 同值锚断言面）。
    fn start(&self) -> Option<RunStartCommand> {
        self.started
            .lock()
            .expect("started 锁不可中毒")
            .last()
            .cloned()
    }

    /// 收口落包捕获（终态 / 记因 / 步整包断言入口）。
    fn finish(&self) -> Option<RunFinishCommand> {
        self.finished
            .lock()
            .expect("finish 锁不可中毒")
            .last()
            .cloned()
    }
}

impl RunHistoryPort for RecordingHistory {
    fn run_started(&self, command: &RunStartCommand) -> Result<(), String> {
        if let Some(reason) = self.fail_started.lock().expect("fail 锁不可中毒").clone() {
            return Err(reason);
        }
        self.started
            .lock()
            .expect("started 锁不可中毒")
            .push(command.clone());
        Ok(())
    }

    fn run_finished(&self, command: &RunFinishCommand) -> Result<(), String> {
        if let Some(reason) = self.fail_finished.lock().expect("fail 锁不可中毒").clone() {
            return Err(reason);
        }
        self.finished
            .lock()
            .expect("finished 锁不可中毒")
            .push(command.clone());
        Ok(())
    }
}

/// 发起时刻（UTC unix 毫秒，`begin_run` / `RunRequest` 加参后的机械随动固定值）。
const STARTED_AT: i64 = 1_726_000_000_000;

/// run 驱动（返回三元：任务 / kind-only 通知订阅 / 落库缝假件——终态与整包
/// 断言经假件捕获面承载）。
fn spawn_run(
    worker: Arc<dyn WorkerAgentPort>,
    tools: Arc<dyn ToolStepPort>,
    snapshot: Arc<dyn WorkflowSnapshotPort>,
    control: &Arc<ChangeFlowControl>,
    auto_next_phase: bool,
    root: &str,
) -> (
    tokio::task::JoinHandle<ChangeRunStatus>,
    tokio::sync::broadcast::Receiver<RunNotice>,
    Arc<RecordingHistory>,
) {
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("登记 run 应成功");
    let rx = control.subscribe(ROOT, CHANGE_ID).expect("run 订阅应成功");
    let request = RunRequest {
        root: root.to_owned(),
        main_root: root.to_owned(),
        change_id: CHANGE_ID.to_owned(),
        run_id: "run-1".to_owned(),
        auto_next_phase,
        stop_before: None,
        started_at: STARTED_AT,
    };
    let history = RecordingHistory::new();
    let task = tokio::spawn(walk_run(
        worker,
        tools,
        Arc::new(FakeVcs::clean()) as Arc<dyn ArchiveVcsPort>,
        snapshot,
        history.clone() as Arc<dyn RunHistoryPort>,
        Arc::clone(control),
        guard,
        request,
    ));
    (task, rx, history)
}

/// run 驱动（归档相位用例专用）：显式 vcs 缝 + `stop_before` 停止点 + 主 root
/// 拆分——归档步的 git 直调与停等门前置载荷观察面。
#[allow(clippy::too_many_arguments)]
fn spawn_run_with_vcs(
    worker: Arc<dyn WorkerAgentPort>,
    tools: Arc<dyn ToolStepPort>,
    vcs: Arc<FakeVcs>,
    snapshot: Arc<dyn WorkflowSnapshotPort>,
    control: &Arc<ChangeFlowControl>,
    auto_next_phase: bool,
    root: &str,
    main_root: &str,
    stop_before: Option<&str>,
) -> (
    tokio::task::JoinHandle<ChangeRunStatus>,
    tokio::sync::broadcast::Receiver<RunNotice>,
    Arc<RecordingHistory>,
) {
    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .expect("登记 run 应成功");
    let rx = control.subscribe(ROOT, CHANGE_ID).expect("run 订阅应成功");
    let request = RunRequest {
        root: root.to_owned(),
        main_root: main_root.to_owned(),
        change_id: CHANGE_ID.to_owned(),
        run_id: "run-1".to_owned(),
        auto_next_phase,
        stop_before: stop_before.map(str::to_owned),
        started_at: STARTED_AT,
    };
    let history = RecordingHistory::new();
    let task = tokio::spawn(walk_run(
        worker,
        tools,
        vcs as Arc<dyn ArchiveVcsPort>,
        snapshot,
        history.clone() as Arc<dyn RunHistoryPort>,
        Arc::clone(control),
        guard,
        request,
    ));
    (task, rx, history)
}

/// 真件组合轮驱动（unify-run-state-persistence 落库回验用例专用）：真实
/// StoreRunHistory（tempfile workspace 库委派）注入 walk_run——组合轮不注
/// 入假件，库读史即断言事实源。begin_run 携 run_id / started_at（与
/// RunRequest 同值锚），返回（任务 / 通知订阅 / store 句柄克隆）。
fn spawn_real_run(
    db: &TestDb,
    worker: Arc<dyn WorkerAgentPort>,
    tools: Arc<dyn ToolStepPort>,
    snapshot: Arc<dyn WorkflowSnapshotPort>,
    control: &Arc<ChangeFlowControl>,
    root: &str,
    run_id: &str,
) -> (
    tokio::task::JoinHandle<ChangeRunStatus>,
    tokio::sync::broadcast::Receiver<RunNotice>,
    Arc<Store>,
) {
    use crate::run_history::StoreRunHistory;

    let guard = control
        .begin_run(ROOT, CHANGE_ID, run_id.to_owned(), STARTED_AT)
        .expect("登记 run 应成功");
    let rx = control.subscribe(ROOT, CHANGE_ID).expect("run 订阅应成功");
    let request = RunRequest {
        root: root.to_owned(),
        main_root: root.to_owned(),
        change_id: CHANGE_ID.to_owned(),
        run_id: run_id.to_owned(),
        auto_next_phase: false,
        stop_before: None,
        started_at: STARTED_AT,
    };
    let store_port = db.store_arc();
    let history: Arc<dyn RunHistoryPort> = Arc::new(StoreRunHistory::new(store_port));
    let store = Arc::clone(&db.store);
    let task = tokio::spawn(walk_run(
        worker,
        tools,
        Arc::new(FakeVcs::clean()) as Arc<dyn ArchiveVcsPort>,
        snapshot,
        history,
        Arc::clone(control),
        guard,
        request,
    ));
    (task, rx, store)
}

/// 真件组合轮驱动（归档相位停等 + vcs 缝注入）：真实 StoreRunHistory 落库，
/// 归档收口后库读史即断言事实源。
#[allow(clippy::too_many_arguments)]
fn spawn_real_run_archive(
    db: &TestDb,
    worker: Arc<dyn WorkerAgentPort>,
    tools: Arc<dyn ToolStepPort>,
    vcs: Arc<FakeVcs>,
    snapshot: Arc<dyn WorkflowSnapshotPort>,
    control: &Arc<ChangeFlowControl>,
    root: &str,
    run_id: &str,
) -> (
    tokio::task::JoinHandle<ChangeRunStatus>,
    tokio::sync::broadcast::Receiver<RunNotice>,
) {
    use crate::run_history::StoreRunHistory;

    let guard = control
        .begin_run(ROOT, CHANGE_ID, run_id.to_owned(), STARTED_AT)
        .expect("登记 run 应成功");
    let rx = control.subscribe(ROOT, CHANGE_ID).expect("run 订阅应成功");
    let request = RunRequest {
        root: root.to_owned(),
        main_root: root.to_owned(),
        change_id: CHANGE_ID.to_owned(),
        run_id: run_id.to_owned(),
        auto_next_phase: true,
        stop_before: Some("archive".to_owned()),
        started_at: STARTED_AT,
    };
    let history: Arc<dyn RunHistoryPort> = Arc::new(StoreRunHistory::new(db.store_arc()));
    let task = tokio::spawn(walk_run(
        worker,
        tools,
        vcs as Arc<dyn ArchiveVcsPort>,
        snapshot,
        history,
        Arc::clone(control),
        guard,
        request,
    ));
    (task, rx)
}

/// 轮询至谓词成立（后台 run 的观测窗口；30s 上限防挂死）。
async fn wait_for(what: &str, mut probe: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !probe() {
        assert!(Instant::now() < deadline, "等待{what}超时（30s）");
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// phase 间停等确认的测试半边：真实注册表 confirm 回路——订阅更新流，每见
/// `ConfirmWait` 信封即应答 `proceed`（多相位 run 连续放行；run 收口除名后
/// 订阅流断开、任务自然退出）。凡驱动相位落账后继续推进的用例必须接线，
/// 否则 run 停等挂起；在 `spawn_run` 之后、首次 await 之前调用（订阅先行于
/// walker 首个 ConfirmWait 广播）。
fn spawn_confirmer(control: &Arc<ChangeFlowControl>, proceed: bool) -> tokio::task::JoinHandle<()> {
    spawn_confirmer_with_specs(control, proceed, true)
}

/// 停等回路（`sync_specs` 可编程）：归档前停等点回流的「是否执行 delta specs
/// 同步」选择面（归档停等 / 跳过同步用例的注入面）。
fn spawn_confirmer_with_specs(
    control: &Arc<ChangeFlowControl>,
    proceed: bool,
    sync_specs: bool,
) -> tokio::task::JoinHandle<()> {
    let control = Arc::clone(control);
    let mut rx = control.subscribe(ROOT, CHANGE_ID).expect("run 订阅应成功");
    tokio::spawn(async move {
        while let Ok(notice) = rx.recv().await {
            if matches!(notice, RunNotice::ConfirmWait) {
                let _ = control.confirm(ROOT, CHANGE_ID, proceed, sync_specs);
            }
        }
    })
}

/// 停等载荷捕获回路：每见 `ConfirmWait` 先快照注册表条目（phase + 归档前置
/// 载荷）后应答——`stop-before-archive` 停等面数据面断言的事实源。
fn spawn_confirmer_capturing(
    control: &Arc<ChangeFlowControl>,
    proceed: bool,
    sync_specs: bool,
) -> (
    tokio::task::JoinHandle<()>,
    Arc<Mutex<Vec<(String, Option<ArchivePreflight>)>>>,
) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let handle = {
        let seen = Arc::clone(&seen);
        let control = Arc::clone(control);
        let mut rx = control.subscribe(ROOT, CHANGE_ID).expect("run 订阅应成功");
        tokio::spawn(async move {
            while let Ok(notice) = rx.recv().await {
                if matches!(notice, RunNotice::ConfirmWait) {
                    if let Some(snapshot) = control.snapshot(ROOT, CHANGE_ID) {
                        seen.lock()
                            .expect("停等捕获锁不可中毒")
                            .push((snapshot.phase.unwrap_or_default(), snapshot.archive.clone()));
                    }
                    let _ = control.confirm(ROOT, CHANGE_ID, proceed, sync_specs);
                }
            }
        })
    };
    (handle, seen)
}

/// 归档相位路由产出（真件计划步序列；`executor=None` / `evaluator=None` 形态
/// ——`drive` 无 evaluator 硬检查的行为面锚）。
fn archive_route_outcome() -> PhaseNextOutcome {
    const TABLE: [&str; 9] = [
        "proposal",
        "dev-design",
        "test-design",
        "implement",
        "test-gen",
        "test-execution",
        "code-review",
        "acceptance",
        "archive",
    ];
    PhaseNextOutcome {
        done: false,
        next_phase: Some("archive".to_owned()),
        round: 1,
        steps: steps_of("archive"),
        artifact_manifest: manifest_of("archive"),
        executor: None,
        evaluator: None,
        allowed_backtrack_phases: TABLE.iter().map(|phase| (*phase).to_owned()).collect(),
        last_result: None,
        error: None,
    }
}

/// 归档前置载荷 fixture（worktree 形态：change 分支与合入目标在案）。
fn archive_preflight_with_worktree(worktree: &str, delta_specs: Vec<String>) -> ArchivePreflight {
    ArchivePreflight {
        name: NAME.to_owned(),
        completed: true,
        incomplete_phases: Vec::new(),
        missing_artifacts: Vec::new(),
        delta_specs,
        worktree: Some(worktree.to_owned()),
        branch: Some(format!("change/{NAME}")),
        merge_target: Some("main".to_owned()),
    }
}

/// 归档前置载荷预录两副本（停等门与 Preflight 步各消费一次——同一只读载荷）。
fn archive_preflight_twice(worktree: &str, delta_specs: Vec<String>) -> Vec<ArchivePreflight> {
    let payload = archive_preflight_with_worktree(worktree, delta_specs);
    vec![payload.clone(), payload]
}

/// 落包节点摘要（`(phase, step 线词, status 线词)`——执行图 emit 序）。
fn node_rows(command: &RunFinishCommand) -> Vec<(String, String, String)> {
    command
        .nodes
        .iter()
        .map(|node| {
            (
                node.phase.clone(),
                node.step.as_str().to_owned(),
                match node.status {
                    RunStepStatus::Running => "running",
                    RunStepStatus::Passed => "passed",
                    RunStepStatus::Failed => "failed",
                    RunStepStatus::Stopped => "stopped",
                }
                .to_owned(),
            )
        })
        .collect()
}

/// 落包节点中指定步的 detail（短路 / 跳过因断言入口）。
fn node_detail(command: &RunFinishCommand, step: workflow::state::RunStepKind) -> Option<String> {
    command
        .nodes
        .iter()
        .find(|node| node.step == step)
        .and_then(|node| node.detail.clone())
}

/// 归档用例的 worktree 目录字符串（`openspec/changes/<name>` 绝对路径）。
fn worktree_dir(root: &TempRoot) -> String {
    root.0
        .join("openspec/changes")
        .join(NAME)
        .to_string_lossy()
        .into_owned()
}

/// 相位定义（真件只读——计划步序列与产物清单的唯一来源）。
fn phase_def(phase: &str) -> &'static workflow::write::PhaseDefinition {
    phase_table("requirement")
        .expect("requirement 表恒可达")
        .iter()
        .find(|def| def.id == phase)
        .unwrap_or_else(|| panic!("相位 {phase} 应在表内"))
}

/// next 相位下发的计划步序列（真件克隆——walker 迭代本序列，MUST NOT 自持步序）。
fn steps_of(phase: &str) -> Vec<StepDefinition> {
    phase_def(phase).steps.to_vec()
}

/// next 相位下发的产物清单（真件克隆）。
fn manifest_of(phase: &str) -> Vec<ArtifactSpec> {
    phase_def(phase).artifact_manifest.to_vec()
}

/// 门禁反馈边上限（**步声明单源**：相位定义内该检查步 `on_fail` 声明的 limit
/// ——walker 侧 `*_FEEDBACK_LIMIT` 常量随 step 声明退役，AC-2）。
fn feedback_limit_of(phase: &str, node: StepNode) -> u32 {
    phase_def(phase)
        .steps
        .iter()
        .find(|step| step.node == node)
        .and_then(|step| step.on_fail)
        .and_then(|on_fail| match on_fail {
            OnFail::FeedbackLoop { limit } => Some(limit),
            OnFail::Terminal => None,
        })
        .unwrap_or_else(|| panic!("相位 {phase} 的 {node:?} 步应声明 FeedbackLoop"))
}

/// static-check 门禁反馈上限（implement / test-gen 声明同值）。
fn static_check_feedback_limit() -> u32 {
    feedback_limit_of("implement", StepNode::StaticCheck)
}

/// test-execution 门禁反馈上限（test-execution 相位声明）。
fn test_execution_feedback_limit() -> u32 {
    feedback_limit_of("test-execution", StepNode::TestExecution)
}

/// 声明 static-check 步的相位（布局身份常量退役——相位限定由 step 声明承载）。
fn static_check_phases() -> Vec<&'static str> {
    phase_table("requirement")
        .expect("requirement 表恒可达")
        .iter()
        .filter(|def| {
            def.steps
                .iter()
                .any(|step| step.node == StepNode::StaticCheck)
        })
        .map(|def| def.id)
        .collect()
}

/// 声明 test-execution 步的相位。
fn test_execution_phases() -> Vec<&'static str> {
    phase_table("requirement")
        .expect("requirement 表恒可达")
        .iter()
        .filter(|def| {
            def.steps
                .iter()
                .any(|step| step.node == StepNode::TestExecution)
        })
        .map(|def| def.id)
        .collect()
}

/// 摘取落包步整包的 (phase, attempt, step 线格式, status 线格式) 列表
///（broadcast 已 kind-only 降位——步序列断言经落库整包捕获面承载，seq =
/// emit 序与既有 Step 更新序一致）。
fn step_rows(steps: &[RunNodeEntry]) -> Vec<(String, u32, String, String)> {
    fn status_word(status: &RunStepStatus) -> &'static str {
        match status {
            RunStepStatus::Running => "running",
            RunStepStatus::Passed => "passed",
            RunStepStatus::Failed => "failed",
            RunStepStatus::Stopped => "stopped",
        }
    }
    steps
        .iter()
        .map(|entry| {
            (
                entry.phase.clone(),
                entry.attempt,
                entry.step.as_str().to_owned(),
                status_word(&entry.status).to_owned(),
            )
        })
        .collect()
}

/// 通知流非阻塞排空（kind-only 通知的在场性断言入口）。
fn drain_notices(rx: &mut tokio::sync::broadcast::Receiver<RunNotice>) -> Vec<RunNotice> {
    let mut notices = Vec::new();
    while let Ok(notice) = rx.try_recv() {
        notices.push(notice);
    }
    notices
}

/// 会话序摘取：某角色的登记 id 列表（槽位值对应断言的事实源）。
fn sessions_of(sessions: &[(WorkerRole, String)], role: WorkerRole) -> Vec<String> {
    sessions
        .iter()
        .filter(|(entry_role, _)| *entry_role == role)
        .map(|(_, id)| id.clone())
        .collect()
}

/// DecisionLog 命令载荷摘取（(change, phase, session_id) 三元——假写面捕获
/// 面的载荷断言入口）。
fn decision_log_payloads(commands: &[ToolCommand]) -> Vec<(String, String, String)> {
    commands
        .iter()
        .filter_map(|command| match command {
            ToolCommand::DecisionLog {
                change_id,
                phase,
                session_id,
            } => Some((change_id.clone(), phase.clone(), session_id.clone())),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// AC-1：全循环 ①→⑦ / pass 自动推进 / 真实写面组合演进对照
// ---------------------------------------------------------------------------

/// walk_run 全循环（假引擎+假写面）①→⑦：一次 run 的统一时间线恰为
/// phase-next → phase-start → executor → static-check（implement 站）→
/// evaluator → phase-log → phase-next（补录步出局后由 ①→⑧ 收敛 ①→⑦）。
#[tokio::test]
async fn 全循环工具调用序恰为七步相位循环() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let worker = FakeWorker::new(&timeline).assemble().0;
    let tools = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            done_outcome(),
        ])
        .with_static_check(vec![passing_check()])
        .assemble()
        .0;
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    assert_eq!(
        timeline.lock().expect("时间线锁").as_slice(),
        [
            "phase-next",
            "phase-start",
            "executor",
            "static-check",
            "evaluator",
            "phase-log",
            "phase-next"
        ],
        "统一时间线恰为 ①→⑦（implement 站含 static-check 反馈边位）"
    );
}

/// walk_run pass 自动推进：每相位 verdict=pass → 依 phase-next 推进直至
/// done → completed；PhaseStart / PhaseLog 的 change / phase / checklist 载荷
/// 与假写面捕获序列逐条对齐（AC-1）。
#[tokio::test]
async fn pass自动推进至done且载荷逐条对齐() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &[]),
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "全 pass 收敛 completed");

    // PhaseStart 载荷逐条对齐（change id + 相位序）
    let starts: Vec<(String, String)> = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter_map(|command| match command {
            ToolCommand::PhaseStart { change_id, phase } => {
                Some((change_id.clone(), phase.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        starts,
        vec![
            (CHANGE_ID.to_owned(), "proposal".to_owned()),
            (CHANGE_ID.to_owned(), "dev-design".to_owned()),
        ],
        "PhaseStart 载荷与相位推进序逐条对齐"
    );

    // PhaseLog 载荷逐条对齐（桌面代写落账，checklist 随 evaluator 产出）
    let logs: Vec<(String, String, bool)> = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter_map(|command| match command {
            ToolCommand::PhaseLog {
                change_id, input, ..
            } => Some((
                change_id.clone(),
                input.phase.clone(),
                input.checklist.iter().all(|item| item.pass),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        logs,
        vec![
            (CHANGE_ID.to_owned(), "proposal".to_owned(), true),
            (CHANGE_ID.to_owned(), "dev-design".to_owned(), true),
        ],
        "PhaseLog 载荷（change / phase / 全 pass checklist）逐条对齐"
    );
}

/// walk_run 上下文头消费面（新增）：executor 分支直出
/// `ResolvedPhaseSpec.prompt` 静态主体（已携 `change: <name>` 上下文头，零
/// `__CALL_AGENT` 令牌残留——不再经 `executor_prompt` 二次包裹）；evaluator
/// 分支经 `evaluator_prompt` 在静态主体尾部 append 输出协议；两槽位
/// `model_level` 取自 `ResolvedPhaseSpec` 档位（装置适配的档位保真面——AC-5）。
#[tokio::test]
async fn 上下文头消费面_executor直出主体evaluator尾随协议附录() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let requests = requests.lock().expect("请求锁");
    let executor = requests
        .iter()
        .find(|request| request.role == WorkerRole::Executor)
        .expect("executor 会话在场");
    assert!(
        executor.prompt.contains("change: c"),
        "executor prompt 直出静态主体携上下文头 change: <name>: {}",
        executor.prompt
    );
    assert!(
        !executor.prompt.contains("__CALL_AGENT"),
        "executor prompt 零 __CALL_AGENT 令牌残留: {}",
        executor.prompt
    );
    assert_eq!(
        executor.model_level,
        ModelLevel::Low,
        "executor 档位取自 ResolvedPhaseSpec（装置档位保真）"
    );

    let evaluator = requests
        .iter()
        .find(|request| request.role == WorkerRole::Evaluator)
        .expect("evaluator 会话在场");
    assert!(
        !evaluator.prompt.contains("__CALL_AGENT"),
        "evaluator prompt 零 __CALL_AGENT 令牌残留"
    );
    assert_eq!(
        evaluator.model_level,
        ModelLevel::High,
        "evaluator 档位取自 ResolvedPhaseSpec（装置档位保真）"
    );
    let protocol_at = evaluator
        .prompt
        .find("## 输出协议")
        .expect("协议附录尾随在场");
    assert!(
        evaluator.prompt[..protocol_at].contains("change: c"),
        "协议附录仅 append 于静态主体之后（主体携上下文头）: {}",
        evaluator.prompt
    );
    assert!(
        evaluator.prompt[protocol_at..].contains("\"checklist\""),
        "协议附录携 checklist JSON 形状"
    );
}

/// walk_run 真实写面组合演进对照（边界）：walker + 假 WorkerAgentPort + 真实
/// LocalToolSteps（store 缝注入真实 workspace 库）——全程推进后 db 的
/// PhaseRecord 演进与插件直跑形态对照一致（AC-1 尾句 + AC-9 对照口径的循环级
/// 承载；转移判定恒问写面 phase-next——9 次路由调用的权威面）。
#[tokio::test]
async fn 真实写面组合全程演进对照一致() {
    let root = TempRoot::new("real-compose");
    root.change(NAME);
    let db = TestDb::open("real-compose");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    // 真实 LocalToolSteps：命令捕获无独立缝——经 db PhaseRecord 演进断言承载
    //（进程内缝真实组合，最小 mock）
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(FakeVcs::clean()),
        db.store_arc(),
        "run-1".to_owned(),
    ));

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) =
        spawn_run(worker, steps, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "全相位 pass 收敛 completed"
    );

    // 落库演进与插件直跑形态对照一致：9 相位各 1 条 pass（attempt 1、表序；
    // 末位 archive 以空 report + 空 checklist 落 pass）
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 9, "九相位各落账一条（含 archive）");
    let phases: Vec<&str> = entries.iter().map(|entry| entry.phase.as_str()).collect();
    assert_eq!(
        phases,
        [
            "proposal",
            "dev-design",
            "test-design",
            "implement",
            "test-gen",
            "test-execution",
            "code-review",
            "acceptance",
            "archive"
        ],
        "落账表序与相位表一致"
    );
    for entry in &entries {
        assert_eq!(entry.verdict, Verdict::Pass);
        assert_eq!(entry.attempt, 1);
        if entry.phase == "archive" {
            // 归档确定性相位：空 checklist 落账（`derive_verdict([]) = Pass`）
            assert!(entry.report.is_empty() && entry.checklist.is_empty());
        } else {
            assert!(
                !entry.report.is_empty() && !entry.checklist.is_empty(),
                "条目形状与插件 buildEntry 同形: {entry:?}"
            );
        }
        assert!(
            !entry.skipped && !entry.stale,
            "干净 run 无 skipped / stale 标记"
        );
    }

    // active_phase 演进：逐相位开跑、落账清除 → 终态清位
    let record = db
        .store
        .find_change_record(CHANGE_ID)
        .expect("读建档记录应成功")
        .expect("建档记录在场");
    assert!(
        record.active_phase.is_none(),
        "run 收口后 active_phase 清除"
    );
}

/// walk_run 重入自 active_phase 续走（AC-7 续走半边）：预置中段 db 种子
///（前序相位已落 pass、active_phase 指向中段）——发起 run 首次 phase-next 即
/// 解析到 active_phase、已 pass 相位零 phase-start / executor 调用；中断相位
/// 承接重试（attempt 2）而非新开回合。
#[tokio::test]
async fn 重入自active_phase续走不重跑已pass相位() {
    let root = TempRoot::new("resume");
    root.change(NAME);
    let db = TestDb::open("resume");
    // 中段种子：proposal 已 pass、test-design 及其后已 pass、dev-design
    // 中断残留（1 条 fail），再开相 dev-design（attempt 事务内推导 = 2）
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        TS_BASE,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "dev-design",
        Verdict::Fail,
        "中断前未过",
        Vec::new(),
        TS_BASE + 60_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "test-design",
        Verdict::Pass,
        "测试设计通过",
        Vec::new(),
        TS_BASE + 120_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "implement",
        Verdict::Pass,
        "实现通过",
        Vec::new(),
        TS_BASE + 180_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "test-gen",
        Verdict::Pass,
        "测试通过",
        Vec::new(),
        TS_BASE + 240_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "test-execution",
        Verdict::Pass,
        "执行通过",
        Vec::new(),
        TS_BASE + 300_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "code-review",
        Verdict::Pass,
        "审查通过",
        Vec::new(),
        TS_BASE + 360_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "acceptance",
        Verdict::Pass,
        "验收通过",
        Vec::new(),
        TS_BASE + 420_000,
    );
    db.store
        .start_change_phase(CHANGE_ID, "dev-design", TS_BASE + 450_000)
        .expect("中断相位开相种子应成功");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(FakeVcs::clean()),
        db.store_arc(),
        "run-1".to_owned(),
    ));

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) =
        spawn_run(worker, steps, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "中断相位重评 pass 后收敛"
    );

    // 已 pass 相位零 phase-start / executor 调用：会话步恰为 dev-design 的
    // executor + evaluator 两轮（重入续走——捕获断言）
    let requests = requests.lock().expect("请求锁");
    assert_eq!(
        requests.len(),
        2,
        "恰两轮会话（dev-design 承接重评），已 pass 相位零会话"
    );
    assert_eq!(requests[0].role, WorkerRole::Executor);
    assert_eq!(requests[1].role, WorkerRole::Evaluator);
    assert_eq!(
        requests[0].provenance.source_ref.as_deref(),
        Some(format!("{CHANGE_ID}/dev-design/executor/2").as_str()),
        "中断相位承接重试（attempt 2 = 既有条目 1 + 1），非新开回合"
    );
    assert_eq!(
        requests[1].provenance.source_ref.as_deref(),
        Some(format!("{CHANGE_ID}/dev-design/evaluator/2").as_str())
    );
    drop(requests);

    // 落库演进：dev-design 追加 pass 条目（attempt 2）、active_phase 清位
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(
        entries.len(),
        10,
        "dev-design 重评条目纯追加（既有 8 + dev-design 重评 + archive 收口）"
    );
    assert_eq!(entries[8].phase, "dev-design");
    assert_eq!(entries[8].attempt, 2);
    assert_eq!(entries[8].verdict, Verdict::Pass);
    assert_eq!(entries[9].phase, "archive", "归档相位随后收口");
    let record = db
        .store
        .find_change_record(CHANGE_ID)
        .expect("读建档记录应成功")
        .expect("建档记录在场");
    assert!(
        record.active_phase.is_none(),
        "run 收口后 active_phase 清位"
    );
}

// ---------------------------------------------------------------------------
// AC-1 fail 重试 / AC-2 决策分叉
// ---------------------------------------------------------------------------

/// walk_run fail 预算内重试：verdict=fail 且 phase-next 返回同相位重试 →
/// attempt 递增重跑 executor / evaluator；重试上限判定不自建（写面 phase-next
/// 权威返回——脚本直接给 done 收敛，walker 未自数 fail 次数）。
#[tokio::test]
async fn fail预算内重试attempt递增重跑() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(fail_json("首轮未过")), Ok(PASS_JSON.to_owned())])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_with_round("implement", &["implement"], 1, None),
            route_with_round(
                "implement",
                &["implement"],
                2,
                Some(LastResult {
                    phase: "implement".to_owned(),
                    verdict: Verdict::Fail,
                    report: "首轮未过".to_owned(),
                    timestamp: None,
                }),
            ),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    // attempt 递增重跑：executor / evaluator 各两轮，sourceRef attempt 1 → 2
    let requests = requests.lock().expect("请求锁");
    assert_eq!(requests.len(), 4, "executor / evaluator 各重跑一轮");
    assert_eq!(
        requests[0].provenance.source_ref.as_deref(),
        Some(format!("{CHANGE_ID}/implement/executor/1").as_str())
    );
    assert_eq!(
        requests[2].provenance.source_ref.as_deref(),
        Some(format!("{CHANGE_ID}/implement/executor/2").as_str()),
        "attempt 递增（provenance 定式随行）"
    );
}

/// walk_run 决策分叉唤起有界输入：phase-next 返回 MaxRetriesExceeded → 决策
/// agent 恰被唤起一次，WorkerTurnRequest 捕获断言输入有界（fail checklist +
/// 写面下发白名单 + 候选相位 eval report 取自 snapshot detail——AC-2 唤起半边）。
#[tokio::test]
async fn 决策分叉唤起恰一次且输入有界() {
    let (root, db) = decision_fixture("decision-input");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // 首轮正常路由：白名单缓存随行（决策分叉的 allowed 来自上一站
            // phase-next 缓存——walker 的缓存语义面）
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            done_outcome(),
        ])
        .assemble();

    // 真实快照源：决策输入取自 ChangeDetail 只读装配（db 读源——AC-2 有界输入
    // 来源）；查得即 detail(root, **id**) 形参随动证明（磁盘目录名恒 name，
    // id ≠ name 形态下 name 入参必 miss）
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "决策 retry → 重路由 done"
    );

    // 决策 agent 恰被唤起一次（预算内路径零决策——对照行见「retry 预算内自走」）
    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 1, "决策 agent 恰唤起一次");
    let prompt = &decision_requests[0].prompt;

    // 输入有界三面：fail checklist + 写面下发白名单 + 候选相位 eval report
    assert!(
        prompt.contains("写面保形") && prompt.contains("custom_note 丢失"),
        "fail checklist 行（item + evidence）在场"
    );
    assert!(
        prompt.contains("proposal") && prompt.contains("dev-design"),
        "写面下发白名单随行（不自相位表推导）"
    );
    assert!(
        prompt.contains("提案通过") && prompt.contains("设计齐备"),
        "候选相位最近 eval report 取自 snapshot detail"
    );
    assert!(
        !prompt.contains("test-design"),
        "候选面限于白名单（表序后续相位不入场）"
    );
    drop(requests);

    // 零 backtrack（决策 retry）且白名单门放行后直接重路由
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(command, ToolCommand::Backtrack { .. })),
        "retry 决议零 backtrack 步"
    );
    let _ = sessions;
}

/// walk_run backtrack 决议执行：决议 backtrack（to ∈ 白名单）→ ToolCommand::
/// Backtrack 携 allowed 白名单发起 → 目标相位自该处重跑（AC-2 执行半边）。
#[tokio::test]
async fn backtrack决议携白名单执行并重路由() {
    let (root, db) = decision_fixture("backtrack-decision");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计返工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // 首轮正常路由：白名单缓存随行
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    // 真实快照源：决策会话的 detail 读取（db 读源——决策输入面）
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "backtrack 重路由后收敛");

    // ToolCommand::Backtrack 携 allowed 白名单发起
    let commands = commands.lock().expect("命令锁");
    let backtracks: Vec<&ToolCommand> = commands
        .iter()
        .filter(|command| matches!(command, ToolCommand::Backtrack { .. }))
        .collect();
    assert_eq!(backtracks.len(), 1, "backtrack 步恰发起一次");
    match backtracks[0] {
        ToolCommand::Backtrack {
            change_id, input, ..
        } => {
            assert_eq!(change_id, CHANGE_ID);
            assert_eq!(input.to, "dev-design", "决议目标透传");
            assert_eq!(input.reason, "设计返工");
            assert_eq!(
                input.allowed,
                vec![
                    "proposal".to_owned(),
                    "dev-design".to_owned(),
                    "implement".to_owned(),
                ],
                "allowed 白名单随行走带（写面二次校验兜底的载荷面）"
            );
            assert_eq!(input.phase, "implement", "失败相位承载");
        }
        _ => unreachable!(),
    }
    drop(commands);

    // 目标相位自该处重跑：backtrack 后下一站为 dev-design 的 phase-start
    let timeline = timeline.lock().expect("时间线锁");
    let backtrack_pos = timeline
        .iter()
        .position(|label| label == "backtrack")
        .expect("backtrack 在时间线");
    assert_eq!(
        timeline[backtrack_pos + 1..].first(),
        Some(&"phase-next".to_owned()),
        "backtrack 落账后回 phase-next 重路由"
    );
    assert!(
        timeline[backtrack_pos + 2..]
            .iter()
            .any(|label| label == "phase-start"),
        "目标相位自该处重跑（新 phase-start）"
    );
}

/// walk_run retry 预算内自走（边界）：预算内 fail 重试全程决策 agent 零调用
///（phase-next 恒同相位重试，walker 自走——AC-2 尾句）。
#[tokio::test]
async fn retry预算内决策agent零调用() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![
            Ok(fail_json("一")),
            Ok(fail_json("二")),
            Ok(PASS_JSON.to_owned()),
        ])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_with_round("implement", &["implement"], 1, None),
            route_with_round("implement", &["implement"], 2, None),
            route_with_round("implement", &["implement"], 3, None),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    assert!(
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .all(|request| request.role != WorkerRole::Decision),
        "预算内 fail 重试全程决策 agent 零调用（walker 自走）"
    );
}

/// walk_run 越权 backtrack 预校验拒绝（异常）：决议 backtrack 越白名单 →
/// ensure_backtrack_allowed 预校验 Err → Backtrack 步零发起（写面二次校验
/// 兜底见 backtrack_test——AC-2）。
#[tokio::test]
async fn 越权backtrack预校验拒绝零发起() {
    let (root, db) = decision_fixture("overreach-decision");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "test-gen", "reason": "越权目标" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // 首轮正常路由：白名单缓存随行
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
        ])
        .assemble();

    // 真实快照源：决策会话的 detail 读取（db 读源；越权决议走完快照面后由白名单门拦下）
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Failed, "越权决议显式失败停给用户");
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(command, ToolCommand::Backtrack { .. })),
        "Backtrack 步零发起（walker 第一道闸拦下）"
    );

    // 失败原因经 Finished 通知 + 落包捕获面流出（kind-only 降位后记因随整包）
    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Failed);
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("越权"),
        "失败记因落包: {:?}",
        finish.reason
    );
}

/// walk_run ask 中断与应答回流（正向）：决议 ask → RunUpdate::Ask 流出 +
/// wait_answer 挂起；answer 回流后以应答文本 Continue 决策会话重出封闭集
///（AC-2 ask 半边）。
#[tokio::test]
async fn ask中断与应答回流continue决策会话() {
    let (root, db) = decision_fixture("ask-decision");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "ask", "question": "回溯目标选哪个?", "options": ["proposal", "dev-design"] }"#.to_owned(),
            r#"{ "action": "retry" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // 首轮正常路由：白名单缓存随行（决策分叉的 allowed 来自上一站
            // phase-next 缓存——walker 的缓存语义面）
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            done_outcome(),
        ])
        .assemble();

    // 真实快照源：决策会话的 detail 读取（db 读源——ask 挂起前的输入面）
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);

    // Ask 通知流出（kind-only 降位——中断载荷面归注册表快照；current_thread
    // 运行时以 try_recv 轮询，30s 上限防挂死）
    let deadline = Instant::now() + Duration::from_secs(30);
    let ask = loop {
        if let Some(ask) = futures_poll(&mut rx) {
            break ask;
        }
        assert!(Instant::now() < deadline, "等待 Ask 通知超时（30s）");
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(ask, RunNotice::Ask, "Ask 通知流出");

    // 应答回流：walker 以应答文本 Continue 决策会话重出封闭集
    control
        .answer(ROOT, CHANGE_ID, "采用方案 A".to_owned())
        .expect("应答回传应成功");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 2, "ask 应答后重出一次决策会话");
    assert!(
        decision_requests[0].continue_session.is_none(),
        "首次决策会话 New"
    );
    let first_decision_session = sessions
        .lock()
        .expect("会话登记锁")
        .iter()
        .find(|(role, _)| *role == WorkerRole::Decision)
        .map(|(_, session)| session.clone())
        .expect("首次决策会话 id");
    assert_eq!(
        decision_requests[1].continue_session.as_deref(),
        Some(first_decision_session.as_str()),
        "应答文本 Continue 同一决策会话（非新会话）"
    );
    assert!(
        decision_requests[1].prompt.contains("用户应答：采用方案 A"),
        "应答文本注入续会话 prompt"
    );
    // 零 backtrack（retry 决议）
    assert!(commands
        .lock()
        .expect("命令锁")
        .iter()
        .all(|command| !matches!(command, ToolCommand::Backtrack { .. })),);
}

/// walk_run phase 间停等确认（正向）：相位落账后 `RunUpdate::ConfirmWait`
/// 流出 + `wait_confirm` 挂起，proceed=true 继续——两相位 run 各流出一条携
/// 相位载荷的 ConfirmWait 信封、确认后依序推进直至 completed（phase 内自动、
/// phase 间停等的节奏锚；test-design「walk_run phase 间停等确认」行）。
#[tokio::test]
async fn 相位间停等确认confirm流出且proceed继续推进() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &[]),
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "proceed=true 继续推进直至 done 收敛"
    );

    // ConfirmWait 通知逐相位流出（kind-only 降位——相位载荷面由快照/控制面
    // 承载，此处机械保留逐相位计数断言）
    let notices = drain_notices(&mut rx);
    let confirms = notices
        .iter()
        .filter(|notice| **notice == RunNotice::ConfirmWait)
        .count();
    assert_eq!(
        confirms, 2,
        "相位推进间 ConfirmWait 逐相位流出（确认卡片通知面）"
    );
    // 确认后 run 全程仍可观测：步与终态通知照常流出（终态随落包捕获面）
    assert!(notices.iter().any(|notice| *notice == RunNotice::Step));
    assert!(notices.iter().any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Completed);
}

/// confirm=false 否决 → 受控终态 stopped：首个 phase 间确认点应答 proceed=
/// false，run 即刻收敛 stopped 且不再发起新相位 / 新会话（等待期间取消置位
/// 同样收敛——停止不必先应答，停止用例以取消路径承接）。
#[tokio::test]
async fn confirm否决收敛stopped且不再发起新相位() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &[]),
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, false);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "proceed=false 收敛 stopped"
    );

    // 不再发起新相位：proposal 落账后停等被否决，dev-design 未开跑
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .filter(|command| matches!(command, ToolCommand::PhaseStart { .. }))
            .count()
            == 1,
        "仅 proposal 开过相位，否决后零新相位"
    );
    assert!(
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .all(|request| request
                .provenance
                .source_ref
                .as_deref()
                .unwrap_or("")
                .contains("proposal")),
        "零 dev-design 会话（否决后不再发起新会话）"
    );

    // 终态与记因经 Finished 流出 + 注册表除名
    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Stopped);
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("proposal"),
        "否决记因携停等相位: {:?}",
        finish.reason
    );
    assert!(control.snapshot(ROOT, CHANGE_ID).is_none(), "终态除名");
}

#[tokio::test]
async fn 停止置位收敛stopped且不再发起新相位() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let gate = EvaluatorGate::armed();
    let (worker, requests, _) = FakeWorker::new(&timeline)
        .with_gate(Arc::clone(&gate))
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &[]),
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);

    // evaluator 会话挂起（gate armed）即运行中窗口：停止寻址键 = change id
    wait_for("evaluator 会话发起", || {
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .any(|request| request.role == WorkerRole::Evaluator)
    })
    .await;
    assert!(control.request_stop(ROOT, CHANGE_ID), "运行中置位返回 true");

    // 放行当前会话收口：proposal 落账后循环顶观测取消 → stopped
    *gate.armed.lock().expect("门锁不可中毒") = false;
    gate.notify.notify_one();
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Stopped, "受控终态 stopped");
    // 不再发起新相位 / 新会话：phase-start 仅 proposal，dev-design 未开跑
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .filter(|command| matches!(command, ToolCommand::PhaseStart { .. }))
            .count()
            == 1,
        "仅 proposal 开过相位，停止后零新相位"
    );
    assert!(
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .all(|request| request
                .provenance
                .source_ref
                .as_deref()
                .unwrap_or("")
                .contains("proposal")),
        "零 dev-design 会话（不再发起新会话）"
    );

    // Finished 通知流出 + 落包终态 + 注册表除名（run 仅进程内）
    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Stopped);
    assert!(control.snapshot(ROOT, CHANGE_ID).is_none(), "终态除名");
}

// ---------------------------------------------------------------------------
// auto_next_phase=true 直通（AC-1 零停等 / AC-5 ask·失败·终态语义边界）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auto确认多相位直通零confirmwait信封终态口径不漂移() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &[]),
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "auto 直通收敛 completed"
    );

    let notices = drain_notices(&mut rx);
    assert!(
        notices
            .iter()
            .all(|notice| *notice != RunNotice::ConfirmWait),
        "通知流零 ConfirmWait（快照面全程不经 waitingConfirm）"
    );

    // 逐相位推进（确经逐相位、非一次收敛）：落包整包涉两相位（kind-only
    // 降位后 phaseStart 词汇不落库——逐相位语义经整包相位集合承载）
    let phases: std::collections::BTreeSet<String> =
        step_rows(&history.finish().expect("finish 落包捕获").nodes)
            .into_iter()
            .map(|(phase, _, _, _)| phase)
            .collect();
    assert_eq!(
        phases,
        ["dev-design".to_owned(), "proposal".to_owned()]
            .into_iter()
            .collect(),
        "逐相位推进（两相位整包在案）"
    );

    // 终态口径：completed + 全相位通过 reason（不随 auto 分支漂移）
    assert!(notices.iter().any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Completed);
    assert_eq!(
        finish.reason.as_deref(),
        Some("All phases have passed, including archiving. Run completed."),
        "终态收口文案不随 auto 分支漂移（含归档相位）"
    );
}

#[tokio::test]
async fn auto确认不接confirmer不挂起自行推进completed() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &[]),
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    // 关键：零 spawn_confirmer 接线——手动档同 fixture 在此必停等挂起
    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task
        .await
        .expect("auto run 不挂起于 wait_confirm（挂起则超时失败）");
    assert_eq!(status, ChangeRunStatus::Completed, "自行推进收敛");

    // 逐相位推进：两相位各开跑一次（非一次收敛）
    let starts = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter(|command| matches!(command, ToolCommand::PhaseStart { .. }))
        .count();
    assert_eq!(starts, 2, "逐相位推进：两相位各开跑一次");
}

#[tokio::test]
async fn auto确认写面失败显式failed零confirmwait() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[])])
        .fail_on("phase-start")
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "写面失败显式 failed（不停等也不跳过）"
    );
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "失败先行（phase-start 失败零会话发起）"
    );

    let notices = drain_notices(&mut rx);
    assert!(
        notices
            .iter()
            .all(|notice| *notice != RunNotice::ConfirmWait),
        "失败路径通知流同样零 ConfirmWait"
    );
    assert!(notices.iter().any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Failed);
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("phase-start 失败"),
        "原因经落包捕获面流出: {:?}",
        finish.reason
    );
}

#[tokio::test]
async fn auto确认ask照常停等应答回流后收敛全程零confirmwait() {
    let (root, db) = decision_fixture("auto-ask");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "ask", "question": "回溯目标选哪个?", "options": ["proposal", "dev-design"] }"#.to_owned(),
            r#"{ "action": "retry" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // 首轮正常路由：白名单缓存随行（决策分叉的 allowed 来自上一站
            // phase-next 缓存——walker 的缓存语义面）
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            done_outcome(),
        ])
        .assemble();

    // 真实快照源：决策会话的 detail 读取真实组合（db 种子——StoreSnapshot 读源）
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, _history) =
        spawn_run(worker, tools, snapshot, &control, true, &root.root_str());

    // Ask 信封流出（更新流全量留档收集——零 ConfirmWait 断言面一并覆盖；
    // current_thread 运行时以 try_recv 轮询，30s 上限防挂死）
    let mut updates: Vec<RunNotice> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    let ask = loop {
        match rx.try_recv() {
            Ok(notice) => {
                let hit = matches!(notice, RunNotice::Ask);
                updates.push(notice);
                if hit {
                    break updates.pop().expect("Ask 通知在收集队尾");
                }
            }
            Err(_) => {
                assert!(Instant::now() < deadline, "等待 Ask 通知超时（30s）");
                tokio::task::yield_now().await;
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    };
    assert_eq!(
        ask,
        RunNotice::Ask,
        "auto 模式 ask 中断照常流出（不自动选择；载荷面归注册表快照）"
    );

    // 应答回流：Continue 决策会话重出封闭集 → retry → done 收敛
    control
        .answer(ROOT, CHANGE_ID, "采用方案 A".to_owned())
        .expect("应答回传应成功");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "ask 应答后收敛 completed"
    );
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }

    // 全程零 ConfirmWait（auto 与 ask 互不干涉——ask 停等不附带确认停等）
    while let Ok(notice) = rx.try_recv() {
        updates.push(notice);
    }
    assert!(
        updates
            .iter()
            .all(|notice| *notice != RunNotice::ConfirmWait),
        "auto 模式全程零 ConfirmWait 通知"
    );
    assert!(
        updates.iter().any(|notice| *notice == RunNotice::Finished),
        "终态收口照常流出"
    );

    // 决策会话两轮：ask 挂起前 New 会话 + 应答文本 Continue 同会话重出
    let first_decision_session = sessions
        .lock()
        .expect("会话登记锁")
        .iter()
        .find(|(role, _)| *role == WorkerRole::Decision)
        .map(|(_, session)| session.clone())
        .expect("首次决策会话 id");
    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 2, "ask 应答后重出一次决策会话");
    assert!(
        decision_requests[0].continue_session.is_none(),
        "首次决策会话 New"
    );
    assert_eq!(
        decision_requests[1].continue_session.as_deref(),
        Some(first_decision_session.as_str()),
        "应答文本 Continue 同一决策会话（非新会话）"
    );
    assert!(
        decision_requests[1].prompt.contains("用户应答：采用方案 A"),
        "应答文本注入续会话 prompt"
    );
}

// ---------------------------------------------------------------------------
// static-check 门控 / 反馈边 / 升格; 步状态流出
// ---------------------------------------------------------------------------

/// walk_run static-check 步门控
#[tokio::test]
async fn static_check步门控implement站必经非门控相位零调用() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            route_outcome("proposal", &[]),
            done_outcome(),
        ])
        .with_static_check(vec![passing_check()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let static_checks = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter(|command| matches!(command, ToolCommand::StaticCheck { .. }))
        .count();
    assert_eq!(static_checks, 1, "static-check 恰在 implement 站必经一次");

    // 时间线位置：implement executor 收口后、evaluator 前
    let timeline = timeline.lock().expect("时间线锁");
    let implement_executor = timeline
        .iter()
        .position(|label| label == "executor")
        .expect("executor 在时间线");
    assert_eq!(
        timeline[implement_executor + 1],
        "static-check",
        "executor 收口后 static-check 必经"
    );
    // proposal 段（第二 executor 之后）直接 evaluator，零 static-check
    let second_executor = timeline[implement_executor + 1..]
        .iter()
        .position(|label| label == "executor")
        .map(|offset| offset + implement_executor + 1)
        .expect("proposal executor 在时间线");
    assert_eq!(
        timeline[second_executor + 1],
        "evaluator",
        "非门控相位 executor 后直接 evaluator（零 static-check）"
    );
}

/// walk_run 反馈边同会话注入：static-check 失败诊断以 continue_session=当前
/// executor 会话 Continue 注入修复（同一会话，非新会话——AC-4）。
#[tokio::test]
async fn 反馈边诊断同会话注入修复() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(PASS_JSON.to_owned())])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            done_outcome(),
        ])
        .with_static_check(vec![
            failing_check("clippy error E0308: mismatched types"),
            passing_check(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    // 三轮 executor：首发 → 修复（Continue 同会话）→ evaluator
    let requests = requests.lock().expect("请求锁");
    assert_eq!(requests.len(), 3, "executor 首发与修复 + evaluator");
    assert!(requests[0].continue_session.is_none(), "首发 New");
    let executor_sessions: Vec<String> = sessions
        .lock()
        .expect("会话登记锁")
        .iter()
        .filter(|(role, _)| *role == WorkerRole::Executor)
        .map(|(_, session)| session.clone())
        .collect();
    assert_eq!(
        requests[1].continue_session.as_deref(),
        Some(executor_sessions[0].as_str()),
        "反馈边注入同一 executor 会话（非新会话）"
    );
    assert!(
        requests[1].prompt.contains("静态检查未通过")
            && requests[1].prompt.contains("clippy error E0308"),
        "失败诊断注入修复 prompt"
    );
    // 模型档位随行（相位表 spec → WorkerTurnRequest 穿线）：executor 首发与
    // 反馈边续注同 Low 档、evaluator High 档
    assert_eq!(
        requests[0].model_level,
        ModelLevel::Low,
        "executor 首发档位"
    );
    assert_eq!(requests[1].model_level, ModelLevel::Low, "反馈边续注不换档");
    assert_eq!(requests[2].model_level, ModelLevel::High, "evaluator 档位");
}

/// walk_run 反馈边独立计数与超限升格（边界）：反馈循环恰 ≤5 次（
/// static_check_feedback_limit()）且不消耗相位 retry 预算；第 5 次仍失败 →
/// 升格相位 fail：桌面代写 fail phase-log（不跑 evaluator）后进 phase-next。
#[tokio::test]
async fn 反馈边恰五次且超限升格相位fail不跑evaluator() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            done_outcome(),
        ])
        .with_static_check(vec![
            failing_check("clippy 未过");
            (static_check_feedback_limit() + 1) as usize
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "升格 fail 落账后 phase-next 分叉（脚本给 done）收敛"
    );

    // static-check 恰 6 次（首检 + 5 次反馈复查），修复会话恰 5 次
    let commands = commands.lock().expect("命令锁");
    let static_checks = commands
        .iter()
        .filter(|command| matches!(command, ToolCommand::StaticCheck { .. }))
        .count();
    assert_eq!(
        static_checks,
        static_check_feedback_limit() as usize + 1,
        "反馈边独立计数恰 ≤5（第 5 次仍失败即升格）"
    );
    let upgrade = commands
        .iter()
        .find_map(|command| match command {
            ToolCommand::PhaseLog { input, .. } if input.report.contains("升格") => {
                Some(input.clone())
            }
            _ => None,
        })
        .expect("升格代写 fail phase-log 在场");
    assert!(
        !upgrade.checklist.is_empty() && !upgrade.checklist[0].pass,
        "升格落账为 fail checklist（静态检查未过）"
    );
    drop(commands);

    // 不跑 evaluator（升格代写替代）；修复会话 prompt 计数 1/5..5/5
    let requests = requests.lock().expect("请求锁");
    assert!(
        requests
            .iter()
            .all(|request| request.role != WorkerRole::Evaluator),
        "升格路径零 evaluator 会话"
    );
    let executor_prompts: Vec<&str> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Executor)
        .map(|request| request.prompt.as_str())
        .collect();
    assert_eq!(executor_prompts.len(), 6, "executor 首发一次 + 修复五次");
    let fix_prompts: Vec<&str> = executor_prompts
        .iter()
        .copied()
        .filter(|prompt| prompt.contains("静态检查未通过"))
        .collect();
    assert_eq!(fix_prompts.len(), 5, "修复会话恰 5 次（反馈边独立预算）");
    for (idx, prompt) in fix_prompts.iter().enumerate() {
        assert!(
            prompt.contains(&format!("第 {}/{}", idx + 1, static_check_feedback_limit())),
            "修复 prompt 反馈计数递进: {prompt}"
        );
    }
}

/// verdict 解析失败反馈边：evaluator 首发产出不可解析文本 → 门 failed 节点 →
/// 解析错误诊断注入同一 evaluator 会话（Continue，feedback 边）修复 → 重解析
/// （instance_no 递增新节点）pass 收口。门步 `on_fail` 声明 FeedbackLoop{5}
/// ——VerdictGate 反馈边语义的假双缝全链。
#[tokio::test]
async fn verdict解析失败反馈边修复_续注同evaluator会话重解析() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![
            Ok("评估结论正文，无 JSON".to_owned()),
            Ok(PASS_JSON.to_owned()),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &["proposal"]),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "修复后 pass 收敛");

    // 会话面：evaluator 首发 + 修复轮 Continue 同会话（假件回声——槽位值续注
    // 不漂移）
    let sessions = sessions.lock().expect("会话登记锁");
    let evaluators = sessions_of(&sessions, WorkerRole::Evaluator);
    assert_eq!(evaluators.len(), 2, "首发 + 一轮修复");
    assert_eq!(evaluators[0], evaluators[1], "修复轮续注同一 evaluator 会话");
    drop(sessions);

    // 请求面：修复轮 Continue 会话 id = 首发会话 id，prompt 携解析错误与协议
    // 重申（verdict_fix_prompt 组装面）
    let requests = requests.lock().expect("请求锁");
    let eval_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Evaluator)
        .collect();
    assert_eq!(eval_requests.len(), 2);
    assert!(eval_requests[0].continue_session.is_none(), "首发 New");
    assert_eq!(
        eval_requests[1].continue_session.as_deref(),
        Some(evaluators[0].as_str()),
        "修复轮 Continue 同会话"
    );
    assert!(
        eval_requests[1].prompt.contains("第 1/5 次反馈修复")
            && eval_requests[1].prompt.contains("未在最终消息中找到合法 JSON")
            && eval_requests[1].prompt.contains("## 输出协议"),
        "修复 prompt 携解析错误与协议重申: {}",
        eval_requests[1].prompt
    );
    drop(requests);

    // 图面：verdict_gate 节点 instance_no 递增（0 failed → 1 passed）；feedback
    // 边自失败门节点指向 evaluator 修复节点
    let finish = history.finish().expect("finish 落包捕获");
    let gate_nodes: Vec<&RunNodeEntry> = finish
        .nodes
        .iter()
        .filter(|entry| entry.step.as_str() == "verdict_gate")
        .collect();
    assert_eq!(gate_nodes.len(), 2, "解析失败与重解析各一节点");
    assert_eq!(gate_nodes[0].instance_no, 0);
    assert_eq!(gate_nodes[0].status, RunStepStatus::Failed);
    assert_eq!(gate_nodes[1].instance_no, 1);
    assert_eq!(gate_nodes[1].status, RunStepStatus::Passed);
    let feedback_edges: Vec<&workflow::state::RunEdgeEntry> = finish
        .edges
        .iter()
        .filter(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Feedback)
        .collect();
    assert_eq!(feedback_edges.len(), 1, "恰一条 feedback 边");
    assert_eq!(
        feedback_edges[0].from_node_key, "proposal:1:verdict_gate:0",
        "feedback 边起于失败门节点"
    );
    assert_eq!(
        feedback_edges[0].to_node_key, "proposal:1:evaluator:1",
        "feedback 边终于 evaluator 修复节点"
    );

    // 落账面：修复后 pass checklist 落账（非升格代写）
    let commands = commands.lock().expect("命令锁");
    let payload = commands
        .iter()
        .find_map(|command| match command {
            ToolCommand::PhaseLog { input, .. } => Some(input.clone()),
            _ => None,
        })
        .expect("phase-log 落账在场");
    assert!(
        payload.checklist.iter().all(|item| item.pass),
        "修复后 verdict pass 落账"
    );
    drop(commands);
}

/// verdict 解析失败反馈边超限升格（真实写面组合）：5 attempt ×（首发 + 5 修复）
/// 连续解析失败 → 每 attempt 超限代写 fail phase-log（executor + evaluator 双
/// 槽位）→ 重试预算耗尽决策 stop 收敛。evaluator 队列恰 30 垃圾（精确值——
/// 假件耗尽回落 PASS_JSON，多给掩盖 off-by-one）。
#[tokio::test]
async fn 真实写面组合_verdict解析失败反馈边超限_升格双槽位决策stop() {
    let root = TempRoot::new("verdict-upgrade");
    root.change(NAME);
    let db = TestDb::open("verdict-upgrade");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let garbage = "评估结论正文，无 JSON".to_owned();
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(garbage.clone()); 30])
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "评估输出修复与重试预算均耗尽" }"#.to_owned(),
        ])
        .assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(FakeVcs::clean()),
        db.store_arc(),
        "run-1".to_owned(),
    ));

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) =
        spawn_run(worker, steps, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "反馈边与重试预算均耗尽后决策 stop 收敛"
    );
    let finish = history.finish().expect("finish 落包捕获");
    assert!(
        finish.reason.as_deref().is_some_and(|reason| reason.contains("决策 stop")),
        "决策 stop 收敛记因: {:?}",
        finish.reason
    );

    // 会话面：5 attempt ×（evaluator 首发 + 5 修复）30 轮，组内续注同会话
    let sessions = sessions.lock().expect("会话登记锁");
    let executor_sessions = sessions_of(&sessions, WorkerRole::Executor);
    assert_eq!(executor_sessions.len(), 5, "5 attempt 各一轮 executor");
    let evaluator_sessions = sessions_of(&sessions, WorkerRole::Evaluator);
    assert_eq!(evaluator_sessions.len(), 30, "5 attempt × 6 轮");
    let attempt_sessions: Vec<String> = evaluator_sessions.iter().step_by(6).cloned().collect();
    for (k, chunk) in evaluator_sessions.chunks(6).enumerate() {
        assert!(
            chunk.iter().all(|id| id == &attempt_sessions[k]),
            "同 attempt 反馈修复轮续注同 evaluator 会话（槽位值不漂移）"
        );
    }
    let decisions = sessions_of(&sessions, WorkerRole::Decision);
    assert_eq!(decisions.len(), 1, "决策会话恰一轮");
    drop(sessions);

    // fail 条目：五轮升格代写——report 携升格、item 为评估输出格式、双槽位
    // 在场（区别于 static-check 升格仅 executor 单槽）；最新条目携决策挂账键
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 5, "五轮升格 fail 纯追加");
    for (idx, entry) in entries.iter().enumerate() {
        assert_eq!(entry.phase, "proposal");
        assert_eq!(entry.verdict, Verdict::Fail);
        assert!(
            entry.report.contains("升格"),
            "升格代写 report 记因: {}",
            entry.report
        );
        assert_eq!(entry.checklist.len(), 1);
        assert_eq!(entry.checklist[0].item, "评估输出格式");
        assert!(!entry.checklist[0].pass);
        assert!(
            entry.checklist[0].evidence.contains("未在最终消息中找到合法 JSON"),
            "evidence 携解析错误全文: {}",
            entry.checklist[0].evidence
        );
        assert_eq!(
            entry.executor_session_id.as_deref(),
            Some(executor_sessions[idx].as_str()),
            "fail 条目携 executor 槽位"
        );
        assert_eq!(
            entry.evaluator_session_id.as_deref(),
            Some(attempt_sessions[idx].as_str()),
            "fail 条目携 evaluator 槽位、值续注该 attempt 首发会话"
        );
    }
    for entry in entries.iter().take(4) {
        assert!(entry.decision_session_id.is_none());
    }
    assert_eq!(
        entries[4].decision_session_id.as_deref(),
        Some(decisions[0].as_str()),
        "决策收口后最新 fail 条目定点挂账"
    );
}

/// walk_run 步状态流出上图（正向）：全程每步 emit RunUpdate::Step
///（phase / attempt / step / status 逐档可辨）：三类节点（WorkerAgent /
/// ToolStep / Gate）状态均经状态流出（图上可观测输入面——AC-4/AC-5）。
#[tokio::test]
async fn 步状态流出三类节点逐档可辨() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            done_outcome(),
        ])
        .with_static_check(vec![passing_check()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let rows = step_rows(&history.finish().expect("finish 落包捕获").nodes);

    // 落库五词汇齐备且可辨（kind-only 降位后步序列经落包整包承载；流程面
    // 步骤与三门不落库——缺席断言留测试相位）
    let kinds: Vec<&str> = rows.iter().map(|(_, _, kind, _)| kind.as_str()).collect();
    for expected in ["executor", "static_check", "evaluator"] {
        assert!(kinds.contains(&expected), "步词汇 {expected} 经整包落库");
    }
    assert!(kinds.contains(&"executor"), "WorkerAgent 类（executor）");
    assert!(
        kinds.contains(&"static_check"),
        "ToolStep 类（static_check）"
    );

    // phase / attempt 逐档对齐 + running→passed 状态推进
    for (phase, attempt, _, status) in &rows {
        assert_eq!(phase, "implement", "步状态行 phase 逐档");
        assert!([0, 1].contains(attempt), "attempt 逐档: {attempt}");
        assert!(
            status == "running" || status == "passed",
            "步状态四档可辨: {status}"
        );
    }
    // 每一步终态可辨（emit 序起手 running 经稳定身份键就地合并为收口 passed
    // ——整包恒一行一节点，节点身份键 `(phase, attempt, step, instance_no)`）
    let step_at = |kind: &str, status: &str| {
        rows.iter()
            .any(|(_, _, row_kind, row_status)| row_kind == kind && row_status == status)
    };
    for kind in ["executor", "static_check", "evaluator"] {
        assert!(step_at(kind, "passed"), "{kind} passed 收口");
        assert!(!step_at(kind, "running"), "{kind} 起手行已合并为终态");
    }
    // 同 (phase, attempt, step) 恰一行（稳定身份键唯一性）
    let mut identities: Vec<(String, u32, String)> = rows
        .iter()
        .map(|(phase, attempt, kind, _)| (phase.clone(), *attempt, kind.clone()))
        .collect();
    let total = identities.len();
    identities.sort();
    identities.dedup();
    assert_eq!(identities.len(), total, "节点身份键稳定唯一（无重复行）");
}

/// walk_run 工具步失败显式失败（异常）：假写面返回 Err(String) → run 收敛
/// failed 且原因经 Finished 流出（不静默空转——AC-6 失败面；ToolStepError
/// 出局后统一 Err 承载）。
#[tokio::test]
async fn 工具步失败显式failed且原因流出() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[])])
        .fail_on("phase-start")
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Failed, "工具步失败显式 failed");
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "失败先行（phase-start 失败零会话发起）"
    );

    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Failed);
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("phase-start 失败"),
        "原因经 Finished 流出: {:?}",
        finish.reason
    );
}

/// walk_run 会话失败收敛携真因（失败成因随行面）：executor 会话 `Failed` 收敛 +
/// api_error 记因 → 步 detail 与 run 终态 reason 同文携成因（诊断真因直给，
/// 不再只剩通用文案）。
#[tokio::test]
async fn 会话失败收敛携真因随节点与终态() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline)
        .with_executor_failures(vec![Some(
            "API 流失败: HttpError: Invalid content type was returned: \"text/html\"".to_owned(),
        )])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[])])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Failed, "会话失败收敛显式 failed");
    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Failed);
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid content type"),
        "run 终态 reason 携失败真因: {:?}",
        finish.reason
    );

    // 步 detail 同文携真因（executor 失败节点）
    let executor = finish
        .nodes
        .iter()
        .find(|node| node.step == workflow::state::RunStepKind::Executor)
        .expect("executor 节点在案");
    let detail = executor.detail.as_deref().unwrap_or_default();
    assert!(
        detail.contains("会话失败收敛") && detail.contains("Invalid content type"),
        "步 detail 携通用前缀 + 真因，实际: {detail}"
    );
}

/// walk_run 会话失败收敛成因缺席回落（边界）：`Failed` 收敛但转录无记因载荷
/// → 通用文案（调用点回落，不空 detail）。
#[tokio::test]
async fn 会话失败收敛成因缺席回落通用文案() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline)
        .with_executor_failures(vec![None])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[])])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Failed);
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(
        finish.reason.as_deref(),
        Some("会话失败收敛"),
        "成因缺席回落通用文案"
    );
    let executor = finish
        .nodes
        .iter()
        .find(|node| node.step == workflow::state::RunStepKind::Executor)
        .expect("executor 节点在案");
    assert_eq!(
        executor.detail.as_deref(),
        Some("会话失败收敛"),
        "步 detail 同口径回落"
    );
}

/// walk_run 空白名单 backtrack 出口（边界）：phase-next 白名单为空 +
/// MaxRetriesExceeded → 决策输入白名单段为空，backtrack 决议必被预校验拒绝
///（仅剩 retry / stop / ask 出口——AC-2）。
#[tokio::test]
async fn 空白名单backtrack决议被拒() {
    let (root, db) = decision_fixture("empty-whitelist-decision");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "proposal", "reason": "空名单越权" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // 首轮正常路由：白名单缓存为空集（空白名单出口的缓存面）
            route_outcome("implement", &[]),
            max_retries_outcome("implement", &[], "首轮未过"),
        ])
        .assemble();

    // 真实快照源：决策会话的 detail 读取（db 读源——空白名单出口的决策输入面）
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "空白名单 backtrack 出口封死"
    );
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(command, ToolCommand::Backtrack { .. })),
        "backtrack 步零发起"
    );

    // 决策输入白名单段为空（无任何白名单相位行）
    let requests = requests.lock().expect("请求锁");
    let decision_prompt = requests
        .iter()
        .find(|request| request.role == WorkerRole::Decision)
        .map(|request| request.prompt.clone())
        .expect("决策会话在场");
    assert!(
        !decision_prompt.contains("- proposal"),
        "白名单段为空（无候选相位行）"
    );
}

/// 步门控声明锚定（边界）：static-check 反馈上限 5（沿 hook loop_limit 语义）
/// 由相位定义 step 声明供给；声明 static-check 步的相位 == ["implement",
/// "test-gen"]（布局身份常量退役——转移判定恒问写面 phase-next，不据其路由）。
#[test]
fn 步门控声明锚定() {
    assert_eq!(static_check_feedback_limit(), 5);
    assert_eq!(static_check_phases(), vec!["implement", "test-gen"]);
    // run_id 铸造定式（每 run 一个会话窗口标识）
    let run_id = new_run_id();
    assert!(run_id.starts_with("run-"), "run-<millis> 定式: {run_id}");
}

// ---------------------------------------------------------------------------
// AC-5：会话槽位落账与决策挂账（desktop-change-session-visibility）
// ---------------------------------------------------------------------------

/// walk_run 真实写面组合（LocalToolSteps + store 缝真实 workspace 库）：verdict
/// 条目落库携 executor + evaluator 双槽位、值自 `WorkerTurnOutcome.session_id`
/// 与假引擎会话序逐一对应；decision 槽位在 verdict 落账时恒缺省（D5），决策会话
/// 收口后经 decision_log 写面单点定点挂账（AC-5 全链真实组合）。
#[tokio::test]
async fn 真实写面组合_会话槽位落账与决策挂账全链对应() {
    let root = TempRoot::new("slots-real-compose");
    root.change(NAME);
    let db = TestDb::open("slots-real-compose");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![
            Ok(fail_json("一")),
            Ok(fail_json("二")),
            Ok(fail_json("三")),
            Ok(fail_json("四")),
            Ok(fail_json("五")),
        ])
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "预算耗尽人工介入" }"#.to_owned(),
        ])
        .assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(FakeVcs::clean()),
        db.store_arc(),
        "run-1".to_owned(),
    ));

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) =
        spawn_run(worker, steps, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Stopped, "决策 stop 收敛受控停止");

    // 假引擎会话序（槽位值对应断言的事实源）：executor / evaluator 交替五轮 + 决策一轮
    let sessions = sessions.lock().expect("会话登记锁");
    let executors = sessions_of(&sessions, WorkerRole::Executor);
    let evaluators = sessions_of(&sessions, WorkerRole::Evaluator);
    let decisions = sessions_of(&sessions, WorkerRole::Decision);
    assert_eq!(executors.len(), 5, "五轮 executor（每 attempt 一会话）");
    assert_eq!(evaluators.len(), 5, "五轮 evaluator");
    assert_eq!(decisions.len(), 1, "决策会话恰一轮（收口即 completed）");
    drop(sessions);

    // verdict 条目落库：executor / evaluator 槽位与会话序逐一对应（取值自
    // WorkerTurnOutcome.session_id——AC-5）
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 5, "五条 fail 条目纯追加");
    for (idx, entry) in entries.iter().enumerate() {
        assert_eq!(entry.phase, "proposal");
        assert_eq!(entry.verdict, Verdict::Fail);
        assert_eq!(
            entry.executor_session_id.as_deref(),
            Some(executors[idx].as_str()),
            "executor 槽位与会话序逐一对应"
        );
        assert_eq!(
            entry.evaluator_session_id.as_deref(),
            Some(evaluators[idx].as_str()),
            "evaluator 槽位随行落账"
        );
    }
    // verdict 落账时 decision 槽位恒缺省（D5）；决策收口后最新条目定点挂账
    for entry in entries.iter().take(4) {
        assert!(
            entry.decision_session_id.is_none(),
            "verdict 条目落账无 decision 槽位"
        );
    }
    assert_eq!(
        entries[4].decision_session_id.as_deref(),
        Some(decisions[0].as_str()),
        "决策会话收口后其 id 记录在案（walk_run → LocalToolSteps → decision_log 写面）"
    );
}

/// 挂账先于决策解析（真实组合链留痕）：决策会话产出漂移文本 → parse 失败路
/// 径 run 显式 failed，但 db 最新条目已携 decision_session_id——写挂时机在
/// parse 之前，parse 失败同样留痕。
#[tokio::test]
async fn 真实写面组合_决策挂账先于解析_parse失败同样留痕() {
    let root = TempRoot::new("relog-before-parse");
    root.change(NAME);
    let db = TestDb::open("relog-before-parse");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![
            Ok(fail_json("一")),
            Ok(fail_json("二")),
            Ok(fail_json("三")),
            Ok(fail_json("四")),
            Ok(fail_json("五")),
        ])
        .with_decision_reports(vec!["不是决策 JSON".to_owned()])
        .assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(FakeVcs::clean()),
        db.store_arc(),
        "run-1".to_owned(),
    ));

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) =
        spawn_run(worker, steps, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "决策解析失败显式 failed");

    // 终态记因为解析失败（非挂账失败——挂账已成功先于 parse）
    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Failed);
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("决策解析失败"),
        "记因为解析失败（挂账不在失败路径）: {:?}",
        finish.reason
    );

    // 留痕：最新 fail 条目已携决策会话 id
    let sessions = sessions.lock().expect("会话登记锁");
    let decisions = sessions_of(&sessions, WorkerRole::Decision);
    assert_eq!(decisions.len(), 1);
    drop(sessions);
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 5);
    assert_eq!(
        entries[4].decision_session_id.as_deref(),
        Some(decisions[0].as_str()),
        "parse 失败路径同样留痕"
    );
}

/// static-check 反馈边超限升格（真实组合链）：fail 条目仅携 executor 单槽位
///（evaluator 未跑无会话可记）；反馈修复轮 Continue 同 executor 会话——槽位
/// 值续注不漂移；决策 stop 收口后最新 fail 条目携 decision 挂账键。前置三相
/// 位预置 pass（db 种子让 implement 站先行——升格门控相位）。
#[tokio::test]
async fn 真实写面组合_升格fail条目仅携executor槽位且修复轮续注同会话() {
    let root = TempRoot::new("upgrade-slots");
    root.change(NAME);
    let db = TestDb::open("upgrade-slots");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "proposal",
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        TS_BASE,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "dev-design",
        Verdict::Pass,
        "设计通过",
        Vec::new(),
        TS_BASE + 60_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "test-design",
        Verdict::Pass,
        "测试设计通过",
        Vec::new(),
        TS_BASE + 120_000,
    );

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "反馈边与重试预算均耗尽" }"#.to_owned(),
        ])
        .assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(FailingRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(FakeVcs::clean()),
        db.store_arc(),
        "run-1".to_owned(),
    ));

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) =
        spawn_run(worker, steps, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "五轮升格 fail 耗尽重试预算后决策 stop 收敛"
    );

    // 会话面：升格路径零 evaluator；executor 每轮首发 + 修复轮 Continue 同会话
    //（假件回声——同 attempt 六轮同 id，会话序即 attempt 分组）
    let sessions = sessions.lock().expect("会话登记锁");
    assert!(
        sessions_of(&sessions, WorkerRole::Evaluator).is_empty(),
        "升格路径零 evaluator 会话（前置相位已预置 pass，本 run 仅 implement 站）"
    );
    let decisions = sessions_of(&sessions, WorkerRole::Decision);
    assert_eq!(decisions.len(), 1);
    let executor_sessions = sessions_of(&sessions, WorkerRole::Executor);
    assert_eq!(
        executor_sessions.len(),
        30,
        "5 attempt ×（首发 1 + 修复 5）轮"
    );
    let attempt_sessions: Vec<String> = executor_sessions.iter().step_by(6).cloned().collect();
    for (k, chunk) in executor_sessions.chunks(6).enumerate() {
        assert!(
            chunk.iter().all(|id| id == &attempt_sessions[k]),
            "同 attempt 反馈修复轮续注同 executor 会话（槽位值不漂移）"
        );
    }
    drop(sessions);

    let requests = requests.lock().expect("请求锁");
    let executor_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Executor)
        .collect();
    assert_eq!(executor_requests.len(), 30);
    for (idx, request) in executor_requests.iter().enumerate() {
        let attempt_idx = idx / 6;
        let expected = if idx % 6 == 0 {
            None
        } else {
            Some(attempt_sessions[attempt_idx].as_str())
        };
        assert_eq!(
            request.continue_session.as_deref(),
            expected,
            "首发 New、修复轮 Continue 同会话"
        );
    }
    drop(requests);

    // fail 条目仅携 executor 单槽位（evaluator 未跑）；最新条目携决策挂账键
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 8, "预置三条 pass + 五轮升格 fail 纯追加");
    let upgrades = &entries[3..];
    for (idx, entry) in upgrades.iter().enumerate() {
        assert_eq!(entry.phase, "implement");
        assert_eq!(entry.verdict, Verdict::Fail);
        assert_eq!(
            entry.executor_session_id.as_deref(),
            Some(attempt_sessions[idx].as_str()),
            "fail 条目仅携 executor 槽位、值续注该轮首发会话"
        );
        assert!(entry.evaluator_session_id.is_none(), "evaluator 未跑无槽位");
    }
    for entry in upgrades.iter().take(4) {
        assert!(entry.decision_session_id.is_none());
    }
    assert_eq!(
        upgrades[4].decision_session_id.as_deref(),
        Some(decisions[0].as_str()),
        "决策收口后最新 fail 条目定点挂账"
    );
}

/// 决策会话收口即挂账（假写面 DecisionLog 臂适配）：ToolCommand::DecisionLog
/// 载荷逐字段捕获（change / phase / session_id），时间线位次在决策会话收口
/// 之后、phase-next 重路由之前。
#[tokio::test]
async fn 决策会话收口即挂账_decisionlog载荷逐字段捕获() {
    let (root, db) = decision_fixture("decisionlog-capture");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "决策 retry 重路由 done 收敛"
    );

    let commands = commands.lock().expect("命令锁");
    let payloads = decision_log_payloads(&commands);
    assert_eq!(payloads.len(), 1, "首轮 completed 收口即挂账恰一次");
    let decision_sessions = {
        let sessions = sessions.lock().expect("会话登记锁");
        sessions_of(&sessions, WorkerRole::Decision)
    };
    assert_eq!(decision_sessions.len(), 1);
    assert_eq!(payloads[0].0, CHANGE_ID, "change 载荷逐字段");
    assert_eq!(payloads[0].1, "implement", "失败相位承载");
    assert_eq!(
        payloads[0].2, decision_sessions[0],
        "session_id 取自 WorkerTurnOutcome.session_id"
    );
    drop(commands);

    // 时间线位次：decision-log 在 decision 收口后、phase-next 重路由前
    let timeline = timeline.lock().expect("时间线锁");
    let decision_pos = timeline
        .iter()
        .position(|label| label == "decision")
        .expect("decision 在时间线");
    assert_eq!(
        timeline[decision_pos + 1],
        "decision-log",
        "收口即挂账（先于决策解析与重路由）"
    );
    assert_eq!(timeline[decision_pos + 2], "phase-next", "挂账后回路由");
}

/// 挂账失败显式收敛 failed（假写面 fail_on 注入 Err）：决策动作未达
/// parse_decision——决策产出故意漂移，若错误抵达解析将呈现「决策解析失败」，
/// 记因为挂账失败即「挂账先于解析」的结构证明（写面严格语义，不静默吞）。
#[tokio::test]
async fn 决策挂账失败显式failed且未达解析() {
    let (root, db) = decision_fixture("decisionlog-fail");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec!["不是决策 JSON".to_owned()])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
        ])
        .fail_on("decision-log")
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "挂账失败显式失败停给用户");

    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    let reason = finish.reason.as_deref().unwrap_or_default();
    assert!(
        reason.contains("decision-log 失败"),
        "记因为挂账失败: {:?}",
        finish.reason
    );
    assert!(
        !reason.contains("决策解析失败"),
        "决策动作未达 parse_decision（挂账先行的结构证明）"
    );
    // 挂账确曾发起（fail_on 命中前命令已捕获）
    assert_eq!(
        decision_log_payloads(&commands.lock().expect("命令锁")).len(),
        1,
        "挂账命令恰发起一次"
    );
}

/// ask 续轮同会话同值重挂（D6）：应答回流 Continue 同会话收口后重挂——
/// DecisionLog 调用序两次且载荷逐字段相同（写面幂等覆写的调用方面）。
#[tokio::test]
async fn ask续轮同会话重挂同值幂等() {
    let (root, db) = decision_fixture("ask-relog");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "ask", "question": "回溯目标选哪个?", "options": ["proposal", "dev-design"] }"#.to_owned(),
            r#"{ "action": "stop", "reason": "用户裁决终止" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);

    // Ask 信封流出（current_thread 运行时以 try_recv 轮询，30s 上限防挂死）
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if futures_poll(&mut rx).is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "等待 Ask 信封超时（30s）");
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(5));
    }
    control
        .answer(ROOT, CHANGE_ID, "终止".to_owned())
        .expect("应答回传应成功");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Stopped, "决策 stop 收敛");

    // 同会话续注：第二轮 continue_session == 首轮决策会话 id
    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 2, "ask 应答后重出一次决策会话");
    let first_session = {
        let sessions = sessions.lock().expect("会话登记锁");
        sessions_of(&sessions, WorkerRole::Decision)
            .first()
            .cloned()
            .expect("首轮决策会话 id")
    };
    assert_eq!(
        decision_requests[1].continue_session.as_deref(),
        Some(first_session.as_str()),
        "应答文本 Continue 同一决策会话"
    );
    drop(decision_requests);

    // DecisionLog 调用序两次、载荷逐字段相同（同会话同值重挂）
    let payloads = decision_log_payloads(&commands.lock().expect("命令锁"));
    assert_eq!(payloads.len(), 2, "每轮收口即挂账");
    assert_eq!(payloads[0], payloads[1], "同会话同值重挂（幂等覆写）");
    assert_eq!(payloads[0].0, CHANGE_ID);
    assert_eq!(payloads[0].1, "implement");
    assert_eq!(payloads[0].2, first_session, "两轮挂账同会话 id");
}

/// emit_step 词汇零新增（结构证明——run-state.ts 零触点的 proposal「不要修
/// 改」口径）：决策挂账全程更新流的步词汇封闭于既有九词，无新步状态信封。
#[tokio::test]
async fn 步状态词汇零新增_决策挂账全程无新步信封() {
    let (root, db) = decision_fixture("vocab-closed");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, &root.root_str());
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let kinds: std::collections::BTreeSet<String> =
        step_rows(&history.finish().expect("finish 落包捕获").nodes)
            .into_iter()
            .map(|(_, _, kind, _)| kind)
            .collect();
    // 落库十值全词汇零过滤（词汇漂移防线：封闭集之外的词出现即破坏）
    const VOCAB: [&str; 10] = [
        "executor",
        "evaluator",
        "decision",
        "static_check",
        "test_execution",
        "phase_start",
        "phase_log",
        "verdict_gate",
        "retry_gate",
        "whitelist_gate",
    ];
    for kind in &kinds {
        assert!(
            VOCAB.contains(&kind.as_str()),
            "落库步词汇漂移（新增词 {kind}）——RunStepKind 封闭集出新词即破坏"
        );
    }
    assert!(
        kinds.contains("phase_start") && kinds.contains("phase_log"),
        "流程面步骤不再被过滤弃落：{kinds:?}"
    );
    // 决策挂账链路确经整包（决策角色步在落库子集）
    assert!(kinds.contains("decision"), "决策会话步在场");
}

// ---------------------------------------------------------------------------
// 装置：无产出快照占位（假双缝用例不触快照面——决策分叉用例另用真实 StoreSnapshot）
// ---------------------------------------------------------------------------

/// 决策分叉未触发的用例的快照占位（walker 仅在 MaxRetriesExceeded 分叉读快照）。
struct StubSnapshot;

impl WorkflowSnapshotPort for StubSnapshot {
    fn detail(&self, _root: &str, _id: &str) -> Result<workflow::queries::ChangeDetail, String> {
        Err("假双缝用例不应触达快照面".to_owned())
    }
}

/// static-check 直过占位（真实 LocalToolSteps 组合用例的注入 runner）。
struct NullRunner;

impl StaticCheckRunner for NullRunner {
    // 磁盘面 port 收 (root, name)：假件零记录逻辑，双参原样吞下
    fn run(&self, _root: &str, _name: &str) -> BoxToolFuture {
        Box::pin(async move { Ok(ToolStepOutput::StaticCheck(passing_check())) })
    }
}

/// static-check 恒败 runner（反馈边超限升格路径驱动——六检五修后升格 fail）。
struct FailingRunner;

impl StaticCheckRunner for FailingRunner {
    fn run(&self, _root: &str, _name: &str) -> BoxToolFuture {
        Box::pin(async move {
            Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
                passed: false,
                diagnostics: "clippy 未过: E0308".to_owned(),
                output_ref: None,
            }))
        })
    }
}

/// test-execution 恒过占位（真实 LocalToolSteps 组合用例的注入 runner——绿跑
/// 机械 checklist 路径；fail / error / Err 四态序列用例由 FakeTools 可编程臂
/// 承载）。
struct NullTestExecutionRunner;

impl TestExecutionRunner for NullTestExecutionRunner {
    fn run(&self, _root: &str, _name: &str) -> BoxToolFuture {
        Box::pin(async move {
            Ok(ToolStepOutput::TestExecution(TestExecutionOutcome {
                conclusion: TestExecutionConclusion::Pass,
                total: 0,
                passed: 0,
                failed: 0,
                skipped: 0,
                findings_brief: String::new(),
                findings_detail: String::new(),
                report_dir: String::new(),
                exec_id: String::new(),
            }))
        })
    }
}

/// broadcast receiver 的非阻塞轮询摘取（current_thread 运行时协作让位面）。
fn futures_poll(rx: &mut tokio::sync::broadcast::Receiver<RunNotice>) -> Option<RunNotice> {
    rx.try_recv()
        .ok()
        .filter(|notice| matches!(notice, RunNotice::Ask))
}

// ---------------------------------------------------------------------------
// test-execution 扩展节（desktop-checks-domain）
// ---------------------------------------------------------------------------

/// test-execution 门禁产出 fixture
fn execution_outcome(conclusion: TestExecutionConclusion, total: u64) -> TestExecutionOutcome {
    TestExecutionOutcome {
        conclusion,
        total,
        passed: total.saturating_sub(1),
        failed: u64::from(conclusion == TestExecutionConclusion::Fail),
        skipped: 0,
        findings_brief: match conclusion {
            TestExecutionConclusion::Pass => "全部测试通过且覆盖率达阈值".to_owned(),
            _ => "「node-test」1 项测试失败——单点失败（疑似 flaky 用例或孤立回归）".to_owned(),
        },
        findings_detail: match conclusion {
            TestExecutionConclusion::Pass => String::new(),
            _ => "## 诊断\n\n「node-test」1 项测试失败——单点失败（疑似 flaky 用例或孤立回归）\n\n\
                  ## suite 执行概览\n- [node-test] root=. exit=1 total=5 passed=4 failed=1 skipped=0\n\n\
                  ## 失败用例明细\n\n[node-test]\n1. 汇总导出_空清单回落（src/export.test.mjs:23）\n   消息: expected '[]' to equal '[a]'"
                .to_owned(),
        },
        report_dir: format!("C:/ws/openspec/changes/{NAME}/reports/test"),
        exec_id: String::new(),
    }
}

/// 正向：test-execution 站（TEST_EXECUTION_PHASES 命中）TestExecution 步必经
/// ——ChangeStepKind::TestExecution 步状态行流出、时间线位置在相位收敛点
///（phase-start 后）；非门控相位零调用（AC-7 上图半边）。
#[tokio::test]
async fn test_execution步门控_站必经_非门控相位零调用() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("test-execution", &["test-execution"]),
            route_outcome("implement", &["implement"]),
            done_outcome(),
        ])
        .with_test_execution(vec![execution_outcome(TestExecutionConclusion::Pass, 5)])
        .with_static_check(vec![passing_check()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let gate_calls = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter(|command| matches!(command, ToolCommand::TestExecution { .. }))
        .count();
    assert_eq!(gate_calls, 1, "test-execution 门禁恰在其站必经一次");

    // 时间线位置：phase-start 后直接门禁步（绿跑零 executor / evaluator）
    let timeline = timeline.lock().expect("时间线锁");
    let start = timeline
        .iter()
        .position(|label| label == "phase-start")
        .expect("phase-start 在时间线");
    assert_eq!(
        timeline[start + 1],
        "test-execution",
        "相位开跑后门禁步必经"
    );
    assert_eq!(timeline[start + 2], "phase-log", "门禁 pass 后机械落账");
    drop(timeline);

    // 步整包流出：testExecution 词汇 running → passed（seq = emit 序）
    let gate_rows: Vec<_> = step_rows(&history.finish().expect("finish 落包捕获").nodes)
        .into_iter()
        .filter(|(_, _, kind, _)| kind == "test_execution")
        .collect();
    assert_eq!(
        gate_rows.len(),
        1,
        "门禁步单节点（running → passed 经稳定身份键就地合并）"
    );
    assert_eq!(gate_rows[0].3, "passed", "合并后终态为 passed");
}

/// 正向：绿跑零 agent——pass 结论路径零 WorkerAgentPort 调用，机械 checklist
/// 代写 phase_log 三条全 pass（suite 结论一致 / 聚合 conclusion 与计数一致 /
/// mutation null 恒真）、report 携 conclusion + 计数 + 报告路径摘要、三会话
/// 槽位恒 None（AC-7 绿跑半边）。
#[tokio::test]
async fn 绿跑零agent_机械checklist代写落账() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![execution_outcome(TestExecutionConclusion::Pass, 5)])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    // 绿跑零 agent：零会话请求（executor / evaluator / decision 全跳过）
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "pass 结论路径零 WorkerAgentPort 调用"
    );
    assert!(
        sessions.lock().expect("会话锁").is_empty(),
        "零会话登记（三角色全跳过）"
    );

    // 机械 checklist 代写 phase_log：三条全 pass + report 摘要 + 三槽位 None
    let commands = commands.lock().expect("命令锁");
    let mechanical = commands
        .iter()
        .find_map(|command| match command {
            ToolCommand::PhaseLog { input, phase, .. } if phase == "test-execution" => {
                Some(input.clone())
            }
            _ => None,
        })
        .expect("机械 phase-log 在场");
    assert_eq!(mechanical.checklist.len(), 3, "三条目全 pass");
    assert!(
        mechanical.checklist.iter().all(|item| item.pass),
        "suite 结论一致 / 聚合计数一致 / mutation null 恒真三查全过"
    );
    assert_eq!(mechanical.checklist[2].item, "mutation null 自动通过");
    let report = &mechanical.report;
    assert!(
        report.contains("pass"),
        "report 携 conclusion 摘要: {report}"
    );
    assert!(
        report.contains("total=5") && report.contains("passed=4"),
        "report 携计数摘要: {report}"
    );
    assert!(
        report.contains("reports/test"),
        "report 携报告路径摘要: {report}"
    );
    assert!(
        mechanical.executor_session_id.is_none()
            && mechanical.evaluator_session_id.is_none()
            && mechanical.decision_session_id.is_none(),
        "三会话槽位恒 None（绿跑零 agent）"
    );
}

/// 正向：fail 结论反馈边——findings 摘要 + 报告路径 prompt 注入修复会话：
/// 首个 fail 新会话（continue_session=None）、provenance 沿
/// `<id>/test-execution/executor/<attempt>` 定式、修复后复跑 pass 机械
/// 落账收敛。
#[tokio::test]
async fn 反馈边fail_新会话与provenance定式() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![
            execution_outcome(TestExecutionConclusion::Fail, 5),
            execution_outcome(TestExecutionConclusion::Pass, 5),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let requests = requests.lock().expect("请求锁");
    assert_eq!(
        requests.len(),
        1,
        "恰一次修复会话（fail→fix→pass→机械落账）"
    );
    let fix = &requests[0];
    assert_eq!(fix.role, WorkerRole::Executor, "反馈边注入 executor 会话");
    assert!(
        fix.continue_session.is_none(),
        "首个 fail 新会话（continue_session=None）"
    );
    assert_eq!(
        fix.provenance.source_ref.as_deref(),
        Some(format!("{CHANGE_ID}/test-execution/executor/1").as_str()),
        "provenance 沿 <id>/test-execution/executor/<attempt> 定式"
    );
    let prompt = &fix.prompt;
    assert!(
        prompt.contains("测试执行未通过（第 1/5 次反馈修复，conclusion=fail）"),
        "修复 prompt 携反馈计数与结论: {prompt}"
    );
    assert!(
        prompt.contains("## 失败用例明细")
            && prompt.contains("汇总导出_空清单回落（src/export.test.mjs:23）")
            && prompt.contains("expected '[]' to equal '[a]'"),
        "结果明细直嵌修复 prompt（用例名 / 文件行号 / 错误消息——agent 免自读报告）: {prompt}"
    );
    assert!(
        prompt.contains("## 诊断") && prompt.contains("单点失败"),
        "诊断面随明细文本注入: {prompt}"
    );
    assert!(
        prompt.contains("全量报告见目录") && prompt.contains("reports/test"),
        "全量报告路径兜底注入: {prompt}"
    );
    drop(requests);

    // 同会话续注：修复会话 id 与首会话回声一致（FakeWorker continue 回声）
    let sessions = sessions.lock().expect("会话锁");
    assert_eq!(
        sessions.len(),
        1,
        "反馈边恰一次会话（修复后 pass 即机械落账）"
    );
}

/// 正向：error 结论同 fail 通路走反馈边（不升 Err、不静默——报告级 error
/// 是反馈边原料面，基础设施失败才 Err）。
#[tokio::test]
async fn error结论同fail通路走反馈边() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![
            execution_outcome(TestExecutionConclusion::Error, 5),
            execution_outcome(TestExecutionConclusion::Pass, 5),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "error 结论不升 Err（run 正常收敛）"
    );

    let requests = requests.lock().expect("请求锁");
    assert_eq!(requests.len(), 1, "error 结论走反馈边（恰一次修复会话）");
    assert!(
        requests[0].prompt.contains("conclusion=error"),
        "error 结论随修复 prompt 注入: {}",
        requests[0].prompt
    );
    drop(requests);

    let gate_calls = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter(|command| matches!(command, ToolCommand::TestExecution { .. }))
        .count();
    assert_eq!(gate_calls, 2, "error 不静默：修复后门禁复跑");
}

/// 异常：反馈边独立计数恰 ≤ test_execution_feedback_limit() 且不消耗相位
/// retry 预算；第 5 次仍失败 → 升格相位 fail（单条 pass=false item +
/// findings 摘要 evidence、executor 槽位携反馈会话 id、evaluator /
/// decision 恒 None、不跑 evaluator——static-check 超限用例同型）。
#[tokio::test]
async fn 反馈边恰五次且超限升格相位fail() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![
            execution_outcome(TestExecutionConclusion::Fail, 5);
            (test_execution_feedback_limit() + 1) as usize
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "升格 fail 落账后 phase-next 分叉收敛"
    );

    // 门禁恰 6 次（首检 + 5 次反馈复查）
    let gate_calls = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter(|command| matches!(command, ToolCommand::TestExecution { .. }))
        .count();
    assert_eq!(
        gate_calls,
        test_execution_feedback_limit() as usize + 1,
        "反馈边独立计数恰 ≤5（第 5 次仍失败即升格）"
    );

    // 升格代写 fail phase-log：单条 fail item + evidence 摘要 + executor 槽位
    let upgrade = commands
        .lock()
        .expect("命令锁")
        .iter()
        .find_map(|command| match command {
            ToolCommand::PhaseLog { input, phase, .. } if phase == "test-execution" => {
                Some(input.clone())
            }
            _ => None,
        })
        .expect("升格代写 fail phase-log 在场");
    assert_eq!(upgrade.checklist.len(), 1, "单条 fail item");
    assert!(!upgrade.checklist[0].pass, "升格落账为 fail checklist");
    assert_eq!(upgrade.checklist[0].item, "测试执行");
    assert!(
        !upgrade.checklist[0].evidence.is_empty(),
        "evidence 携 findings 摘要"
    );
    assert!(
        upgrade.report.contains("反馈边超限"),
        "report 记因升格: {}",
        upgrade.report
    );
    assert!(
        upgrade.executor_session_id.is_some(),
        "executor 槽位携反馈会话 id"
    );
    assert!(
        upgrade.evaluator_session_id.is_none() && upgrade.decision_session_id.is_none(),
        "evaluator / decision 恒 None（evaluator 未跑无会话可记）"
    );

    // 零 evaluator 会话；修复会话恰 5 次、计数递进；attempt 恒 1（反馈边不
    // 消耗相位 retry 预算——无 phase-start 重开）
    let requests = requests.lock().expect("请求锁");
    assert!(
        requests
            .iter()
            .all(|request| request.role == WorkerRole::Executor),
        "升格路径零 evaluator 会话"
    );
    assert_eq!(requests.len(), 5, "修复会话恰 5 次（反馈边独立预算）");
    for (idx, request) in requests.iter().enumerate() {
        assert!(
            request.prompt.contains(&format!(
                "第 {}/{}",
                idx + 1,
                test_execution_feedback_limit()
            )),
            "修复 prompt 反馈计数递进: {}",
            request.prompt
        );
        assert_eq!(
            request.provenance.source_ref.as_deref(),
            Some(format!("{CHANGE_ID}/test-execution/executor/1").as_str()),
            "反馈边不消耗相位 retry 预算（attempt 恒 1）"
        );
    }
}

/// 边界：独立计数与 static-check 计数分立——同 run 双门禁各自计满各自升格
/// 互不挤占（static-check 失败反馈与 test-execution fail 反馈并存不串账）。
#[tokio::test]
async fn 双门禁独立计数互不挤占() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            route_outcome("test-execution", &[]),
            done_outcome(),
        ])
        .with_static_check(vec![
            failing_check("clippy 未过");
            (static_check_feedback_limit() + 1) as usize
        ])
        .with_test_execution(vec![
            execution_outcome(TestExecutionConclusion::Fail, 5);
            (test_execution_feedback_limit() + 1) as usize
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let commands = commands.lock().expect("命令锁");
    let static_calls = commands
        .iter()
        .filter(|command| matches!(command, ToolCommand::StaticCheck { .. }))
        .count();
    let gate_calls = commands
        .iter()
        .filter(|command| matches!(command, ToolCommand::TestExecution { .. }))
        .count();
    assert_eq!(
        static_calls,
        static_check_feedback_limit() as usize + 1,
        "static-check 门禁独立计满（6 检）"
    );
    assert_eq!(
        gate_calls,
        test_execution_feedback_limit() as usize + 1,
        "test-execution 门禁独立计满（6 检）——互不挤占"
    );

    // 双升格各自落账、各自相位
    let upgrades: Vec<(&str, &str)> = commands
        .iter()
        .filter_map(|command| match command {
            ToolCommand::PhaseLog { input, phase, .. } if input.report.contains("反馈边超限") =>
            {
                let kind = if input.report.contains("静态检查") {
                    "static"
                } else {
                    "execution"
                };
                Some((kind, phase.as_str()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(upgrades.len(), 2, "双门禁各一次升格落账");
    assert!(
        upgrades.contains(&("static", "implement"))
            && upgrades.contains(&("execution", "test-execution")),
        "双升格各自相位归属（不串账），实际: {upgrades:?}"
    );
    drop(commands);

    // 全程零 evaluator（双门禁均走升格，evaluator 未跑）
    let sessions = sessions.lock().expect("会话锁");
    assert!(
        sessions
            .iter()
            .all(|(role, _)| *role != WorkerRole::Evaluator),
        "双门禁升格路径零 evaluator 会话"
    );
}

/// 边界：反馈边修复重入门禁步产出随 runner 刷新（runner 链内复用门随输入
/// mtime 推进自动失效重跑——装配语义直测在 mod_test，本节断言 walker 重入
/// 步以第二产出落机械 checklist，不复用首产出）。
#[tokio::test]
async fn 反馈边修复重入_门禁步产出随runner刷新() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _, _) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![
            execution_outcome(TestExecutionConclusion::Fail, 5),
            execution_outcome(TestExecutionConclusion::Pass, 9),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    // 门禁恰两入（fail → 修复 → 重入 pass）
    let gate_calls = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter(|command| matches!(command, ToolCommand::TestExecution { .. }))
        .count();
    assert_eq!(gate_calls, 2, "修复重入门禁复跑");

    // 机械 checklist report 以第二产出刷新（total=9 非 5——runner 产出随重入刷新）
    let commands = commands.lock().expect("命令锁");
    let mechanical = commands
        .iter()
        .find_map(|command| match command {
            ToolCommand::PhaseLog { input, phase, .. } if phase == "test-execution" => {
                Some(input.clone())
            }
            _ => None,
        })
        .expect("机械 phase-log 在场");
    assert!(
        mechanical.report.contains("total=9"),
        "机械 checklist 以重入产出刷新（total=9），实际: {}",
        mechanical.report
    );
    assert!(
        !mechanical.report.contains("total=5"),
        "首产出未残留（复用门失效语义的 walker 侧对偶）"
    );
}

/// 边界：TryFrom<ToolStepOutput> for TestExecutionOutcome——TestExecution
/// 变体窄化逐字段、非 TestExecution 变体不匹配面（run_tool 泛型半边）。
#[test]
fn test_execution窄化tryfrom逐字段与非匹配面() {
    let outcome = execution_outcome(TestExecutionConclusion::Fail, 7);
    let narrowed = TestExecutionOutcome::try_from(ToolStepOutput::TestExecution(outcome.clone()))
        .expect("TestExecution 变体应窄化成功");
    assert_eq!(narrowed, outcome, "窄化逐字段相等");

    // 非 TestExecution 变体不匹配 → Err（输出漂移显式失败面的源头）
    assert!(
        TestExecutionOutcome::try_from(ToolStepOutput::StaticCheck(passing_check())).is_err(),
        "StaticCheck 变体不匹配"
    );
}

#[test]
fn test_execution门控声明锚定() {
    assert_eq!(test_execution_feedback_limit(), 5);
    assert_eq!(test_execution_phases(), vec!["test-execution"]);
    // 双门禁步声明分立：反馈上限同值但彼此独立计数（各自计满各自升格）
    assert_eq!(
        static_check_feedback_limit(),
        test_execution_feedback_limit(),
        "取值对齐（计数分立）"
    );
}

// ---------------------------------------------------------------------------
// exec root 透传锚（design D9 / AC-6）：RunRequest.root 语义换 exec root 后
// 全链透传的防漂移钉——walker 对 root 透明（零改动组件的语义演进锚）
// ---------------------------------------------------------------------------

/// 快照源 root 捕获假件（detail 入参 root 的记录面；返回最小合法
/// ChangeDetail——决策输入组装只消费 pipeline / attempts 骨架）。
struct RootCaptureSnapshot {
    roots: Arc<Mutex<Vec<String>>>,
}

impl RootCaptureSnapshot {
    fn new(roots: Arc<Mutex<Vec<String>>>) -> Self {
        Self { roots }
    }
}

impl WorkflowSnapshotPort for RootCaptureSnapshot {
    fn detail(&self, root: &str, _change: &str) -> Result<workflow::queries::ChangeDetail, String> {
        self.roots
            .lock()
            .expect("快照 root 锁不可中毒")
            .push(root.to_owned());
        Ok(workflow::queries::ChangeDetail {
            id: "exec-root-anchor".to_owned(),
            name: "exec-root-anchor".to_owned(),
            title: "exec-root-anchor".to_owned(),
            source: workflow::queries::ChangeSource::Active,
            status: None,
            created: None,
            pipeline: Vec::new(),
            active_phase: None,
            runs: Vec::new(),
            artifacts: Vec::new(),
            worktree: None,
        })
    }
}

#[tokio::test]
async fn exec_root透传锚_四缝root恒等于request_root() {
    // worktree 执行锚路径按 change **name** 目录名（磁盘面命名——id 不入路径）
    const EXEC_ROOT: &str = r"C:\app-data\worktrees\demo-segment\walker-change";

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let tools_device = FakeTools::new(&timeline).with_phase_next(vec![
        // 首轮正常路由 → 预算内 fail（evaluator 缺省 PASS_JSON，改注入 fail 报
        // 告走 max_retries 分叉触快照）用 max_retries 直驱
        max_retries_outcome("implement", &["proposal", "implement"], "首轮未过"),
        route_outcome("implement", &["proposal", "implement"]),
        done_outcome(),
    ]);
    let tool_roots = tools_device.roots_handle();
    let (tools, _commands) = tools_device.assemble();
    let snapshot_roots = Arc::new(Mutex::new(Vec::new()));
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(RootCaptureSnapshot::new(Arc::clone(&snapshot_roots)));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, EXEC_ROOT);
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "max_retries → 决策 retry → 重路由 done"
    );

    // 四缝捕获的 root 恒等于 exec root（worker turn / 工具步 / 快照源）
    let turn_roots: Vec<String> = requests
        .lock()
        .expect("请求捕获锁不可中毒")
        .iter()
        .map(|request| request.root.clone())
        .collect();
    assert!(!turn_roots.is_empty(), "worker turn 捕获面非空（前置）");
    assert!(
        turn_roots.iter().all(|root| root == EXEC_ROOT),
        "worker turn root 恒 = exec root，实际: {turn_roots:?}"
    );

    let step_roots = tool_roots.lock().expect("root 捕获锁不可中毒").clone();
    assert!(!step_roots.is_empty(), "工具步 root 捕获面非空（前置）");
    assert!(
        step_roots.iter().all(|root| root == EXEC_ROOT),
        "工具步 root 恒 = exec root（写面 layout / 检查器执行目录随 exec root），实际: {step_roots:?}"
    );

    let snap_roots = snapshot_roots.lock().expect("快照 root 锁不可中毒").clone();
    assert_eq!(
        snap_roots,
        vec![EXEC_ROOT.to_owned()],
        "快照源 detail root 恒 = exec root（恰一次——决策分叉读源）"
    );
}

// ---------------------------------------------------------------------------
// 每 run 两写时序（unify-run-state-persistence D5/D6）：假件计数 + 真件回验
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 每run恰两次落库调用_一相位绿跑零额外run域写() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    assert_eq!(history.start_count(), 1, "起点第一写恰一次");
    assert_eq!(history.finish_count(), 1, "终态第二写恰一次");
    let start = history.start().expect("起始行捕获");
    assert_eq!(start.run_id, "run-1");
    assert_eq!(start.change_id, CHANGE_ID, "第一写载荷 change_id 透传");
    assert_eq!(start.started_at, STARTED_AT, "started_at 命令携带原值");
    let finish = history.finish().expect("收口落包捕获");
    assert_eq!(finish.status, RunStatus::Completed);
    assert_eq!(finish.run_id, "run-1");
    assert_eq!(finish.change_id, CHANGE_ID, "第二写载荷 change_id 透传");
}

#[tokio::test]
async fn 真件组合_多相位run落库回验_emit序与终态逐字段一致() {
    let root = TempRoot::new("run-persist");
    root.change(NAME);
    let db = TestDb::open("run-persist");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(FakeVcs::clean()),
        db.store_arc(),
        "run-1".to_owned(),
    ));
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, store) = spawn_real_run(
        &db,
        worker,
        steps,
        Arc::clone(&snapshot),
        &control,
        &root.root_str(),
        "run-1",
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    // RunRecord 回验：终态 / reason / 起止（started_at 与 RunRequest 同值）
    let runs = store.list_change_runs(CHANGE_ID).expect("库读史应成功");
    assert_eq!(runs.len(), 1, "恰一条 run 行");
    assert_eq!(runs[0].run_id, "run-1");
    assert_eq!(runs[0].status, RunStatus::Completed);
    assert_eq!(
        runs[0].started_at, STARTED_AT,
        "started_at 同值（命令铸造面锚 D5）"
    );
    assert!(runs[0].finished_at.is_some(), "收口时刻在位");
    assert_eq!(
        runs[0].reason.as_deref(),
        Some("All phases have passed, including archiving. Run completed."),
        "completed 收口记因"
    );

    // 节点整包：node_seq 升序 = emit 序、十八值全词汇零过滤、整包同刻
    let nodes = store.list_run_nodes("run-1").expect("节点读史应成功");
    assert!(!nodes.is_empty(), "多相位 run 节点整包非空");
    const FULL_WORDS: [workflow::state::RunStepKind; 18] = [
        workflow::state::RunStepKind::Executor,
        workflow::state::RunStepKind::Evaluator,
        workflow::state::RunStepKind::Decision,
        workflow::state::RunStepKind::StaticCheck,
        workflow::state::RunStepKind::TestExecution,
        workflow::state::RunStepKind::PhaseStart,
        workflow::state::RunStepKind::PhaseLog,
        workflow::state::RunStepKind::VerdictGate,
        workflow::state::RunStepKind::RetryGate,
        workflow::state::RunStepKind::WhitelistGate,
        workflow::state::RunStepKind::Preflight,
        workflow::state::RunStepKind::Commit,
        workflow::state::RunStepKind::Rebase,
        workflow::state::RunStepKind::ResolveConflicts,
        workflow::state::RunStepKind::Merge,
        workflow::state::RunStepKind::SpecSync,
        workflow::state::RunStepKind::Seal,
        workflow::state::RunStepKind::Finalize,
    ];
    for row in &nodes {
        assert!(
            FULL_WORDS.contains(&row.step),
            "落库步词汇越十八值全词汇封闭集，实际: {:?}",
            row.step
        );
        assert_eq!(
            row.timestamp,
            runs[0].finished_at.expect("终态收口时刻在位"),
            "整包同刻 = finished_at"
        );
    }
    // 流程面步骤与三门在案（零过滤的行为锚：phase_start / phase_log 不再被弃落）
    assert!(
        nodes
            .iter()
            .any(|row| row.step == workflow::state::RunStepKind::PhaseStart),
        "phase_start 随整包落库（零过滤）"
    );
    assert!(
        nodes
            .iter()
            .any(|row| row.step == workflow::state::RunStepKind::PhaseLog),
        "phase_log 随整包落库（零过滤）"
    );
    // 稳定身份键：同 (phase, attempt, step) 多实例经 instance_no 递增可辨
    for row in &nodes {
        assert!(
            row.instance_no < u32::MAX,
            "实例号有限递增（emit 单点分配）"
        );
    }
    // 会话步（executor / evaluator）session_id 在场——emit 序与库读史一致性的锚
    assert!(
        nodes.iter().any(|row| row.session_id.is_some()),
        "WorkerAgent 步 session_id 透传落库"
    );
    // 归档相位十步节点在案（全相位含 archive 走完的落库面）
    let archive_words: Vec<&str> = nodes
        .iter()
        .filter(|row| row.phase == "archive")
        .map(|row| row.step.as_str())
        .collect();
    assert_eq!(
        archive_words,
        [
            "phase_start",
            "preflight",
            "commit",
            "rebase",
            "resolve_conflicts",
            "merge",
            "spec_sync",
            "seal",
            "finalize",
            "phase_log"
        ],
        "归档十步节点随整包落库"
    );

    // 边整包：四 kind 词汇落库、端点身份串指向本 run 节点身份键
    let edges = store.list_run_edges("run-1").expect("边读史应成功");
    assert!(!edges.is_empty(), "多相位 run 边整包非空");
    let node_keys: Vec<String> = nodes
        .iter()
        .map(|row| {
            format!(
                "{}:{}:{}:{}",
                row.phase,
                row.attempt,
                row.step.as_str(),
                row.instance_no
            )
        })
        .collect();
    for edge in &edges {
        assert!(
            matches!(
                edge.edge_kind,
                workflow::state::RunEdgeKind::Forward
                    | workflow::state::RunEdgeKind::Feedback
                    | workflow::state::RunEdgeKind::Retry
                    | workflow::state::RunEdgeKind::Backtrack
            ),
            "边 kind 限于四值封闭集，实际: {:?}",
            edge.edge_kind
        );
        assert!(
            node_keys.contains(&edge.from_node_key) && node_keys.contains(&edge.to_node_key),
            "边端点恒指向本 run 节点身份串（整包不变量）：{:?}",
            edge
        );
        assert_eq!(
            edge.timestamp,
            runs[0].finished_at.expect("终态收口时刻在位"),
            "边整包同刻"
        );
    }
}

#[tokio::test]
async fn 全词汇run十值零过滤_流程面与三门在案() {
    // 决策分叉输入取自 ChangeDetail 只读装配——真件库种子（快照读源真实组合）
    let (root, _db) = decision_fixture("full-vocab");
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![r#"{ "action": "retry" }"#.to_owned()])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // implement：executor + static-check 反馈边（fail→pass）+ evaluator
            // + verdict 门 + phase-log + phase-start
            route_outcome("implement", &["proposal", "implement"]),
            // 决策分叉：whitelist 门 + retry 门 + decision 会话
            max_retries_outcome("dev-design", &["proposal", "dev-design"], "首轮未过"),
            // test-execution 相位：确定性门禁步（零 agent）
            route_outcome("test-execution", &["test-execution"]),
            done_outcome(),
        ])
        .with_static_check(vec![failing_check("诊断：类型错误"), passing_check()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), _db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let finish = history.finish().expect("收口落包捕获");
    let kinds: Vec<String> = finish
        .nodes
        .iter()
        .map(|entry| entry.step.as_str().to_owned())
        .collect();
    // 全词汇在案（本 fixture 决策动作为 retry → retry_gate 不经手，其在场断言
    // 由决策 backtrack 分支用例承载；决策分叉 + 门禁 + test-execution 相位全覆盖）
    for word in [
        "executor",
        "evaluator",
        "decision",
        "static_check",
        "test_execution",
        "phase_start",
        "phase_log",
        "verdict_gate",
        "whitelist_gate",
    ] {
        assert!(
            kinds.iter().any(|kind| kind == word),
            "落库十值词汇 {word} 应在场（零过滤），实际: {kinds:?}"
        );
    }
    // camelCase 等非 snake 词零行（线面词汇单点）
    for word in ["verdictGate", "retryGate", "whitelistGate", "phaseStart"] {
        assert!(
            !kinds.iter().any(|kind| kind == word),
            "非 snake 线面词 {word} 零行，实际: {kinds:?}"
        );
    }
    let mut distinct = kinds.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct,
        vec![
            "decision".to_owned(),
            "evaluator".to_owned(),
            "executor".to_owned(),
            "phase_log".to_owned(),
            "phase_start".to_owned(),
            "static_check".to_owned(),
            "test_execution".to_owned(),
            "verdict_gate".to_owned(),
            "whitelist_gate".to_owned(),
        ],
        "emit 词汇零过滤（流程面步骤与三门在案；retry_gate 归 backtrack 分支）"
    );
    // 边整包：本 fixture 经计划序推进 + 门禁反馈回边（forward / feedback 在案），
    // 词汇限于四 kind 封闭集；retry / backtrack 仅经相位 fail 重跑与决策 backtrack
    // 分支 emit（各自专用用例承载，四 kind 组装面由 run_history_test 断言）
    let edge_kinds: Vec<String> = finish
        .edges
        .iter()
        .map(|edge| edge.edge_kind.as_str().to_owned())
        .collect();
    const EDGE_VOCAB: [&str; 4] = ["forward", "feedback", "retry", "backtrack"];
    for word in &edge_kinds {
        assert!(
            EDGE_VOCAB.contains(&word.as_str()),
            "边 kind 漂移（越四值封闭集）: {edge_kinds:?}"
        );
    }
    for word in ["forward", "feedback"] {
        assert!(
            edge_kinds.iter().any(|kind| kind == word),
            "边 {word} 应在场，实际: {edge_kinds:?}"
        );
    }
}

#[tokio::test]
async fn 三死亡路径同一出口_第二写恰一次且终态记因各异() {
    // completed：全相位 pass
    {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
        let _confirmer = spawn_confirmer(&control, true);
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Completed);
        assert_eq!(history.finish_count(), 1, "completed 第二写恰一次");
        let finish = history.finish().unwrap();
        assert_eq!(finish.status, RunStatus::Completed);
        assert!(
            finish
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("All phases have passed"),
            "completed 记因与 stopped / failed 各異"
        );
    }
    // stopped：停等确认否决（confirm=false）
    {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("proposal", &[])])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
        let _confirmer = spawn_confirmer(&control, false);
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Stopped);
        assert_eq!(history.finish_count(), 1, "stopped 第二写恰一次");
        let finish = history.finish().unwrap();
        assert_eq!(finish.status, RunStatus::Stopped);
        assert!(
            finish
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("停等未获继续确认"),
            "stopped 记因（停等拒绝语境）"
        );
    }
    // failed：写面失败（phase-start 注入 Err）
    {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("proposal", &[])])
            .fail_on("phase-start")
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Failed);
        assert_eq!(history.finish_count(), 1, "failed 第二写恰一次");
        let finish = history.finish().unwrap();
        assert_eq!(finish.status, RunStatus::Failed);
        assert!(
            finish
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("phase-start"),
            "failed 记因（写面失败语境）"
        );
    }
}

#[tokio::test]
async fn 中途停止收口_active_phase清位() {
    let root = TempRoot::new("hang-clear");
    root.change(NAME);
    let db = TestDb::open("hang-clear");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    // 悬挂形态：开相未落账（active_phase 在位——中途死亡近似构造）
    db.store
        .start_change_phase(CHANGE_ID, "implement", STARTED_AT)
        .expect("开相种子应成功");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[])])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, store) = spawn_real_run(
        &db,
        worker,
        tools,
        Arc::clone(&snapshot),
        &control,
        &root.root_str(),
        "run-1",
    );
    let _confirmer = spawn_confirmer(&control, false); // 首相位后停等拒绝 → stopped 收口
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Stopped);

    let record = store
        .find_change_record(CHANGE_ID)
        .expect("读建档记录应成功")
        .expect("建档记录在场");
    assert!(
        record.active_phase.is_none(),
        "收口事务清位 active_phase（悬挂杀除——stopped 亦然）"
    );
    let runs = store.list_change_runs(CHANGE_ID).expect("库读史应成功");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, RunStatus::Stopped);
}

#[tokio::test]
async fn 续走不重头_第二轮run已pass相位零重执行attempt延续() {
    let root = TempRoot::new("resume-run");
    root.change(NAME);
    let db = TestDb::open("resume-run");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    // 第一轮：proposal attempt 1 fail（fail verdict 落账）→ 停等拒绝收口
    // stopped（中断形态——active_phase 已随落账清位，续走锚点 = 库内 fail 史）
    {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
            .with_evaluator_reports(vec![Ok(fail_json("首轮未过"))])
            .assemble();
        let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
            Arc::new(SessionAnchors::new()),
            Arc::new(NullRunner),
            Arc::new(NullTestExecutionRunner),
            Arc::new(FakeVcs::clean()),
            db.store_arc(),
            "run-1".to_owned(),
        ));
        let snapshot: Arc<dyn WorkflowSnapshotPort> =
            Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, store) = spawn_real_run(
            &db,
            worker,
            steps,
            snapshot,
            &control,
            &root.root_str(),
            "run-1",
        );
        let _confirmer = spawn_confirmer(&control, false);
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Stopped, "首轮 fail 后停等拒绝收口");
        let attempts: Vec<u32> = store
            .list_phase_records(CHANGE_ID)
            .expect("读相位条目应成功")
            .iter()
            .map(|entry| entry.attempt)
            .collect();
        assert_eq!(attempts, vec![1], "首轮 proposal attempt 1（fail 落账）");
    }

    // 第二轮：proposal 承接 attempt 2（跨 run max+1）→ pass 后自后续相位推进
    // 至 done（已走站零重执行——落账序列严格前向）
    {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
            Arc::new(SessionAnchors::new()),
            Arc::new(NullRunner),
            Arc::new(NullTestExecutionRunner),
            Arc::new(FakeVcs::clean()),
            db.store_arc(),
            "run-2".to_owned(),
        ));
        let snapshot: Arc<dyn WorkflowSnapshotPort> =
            Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, store) = spawn_real_run(
            &db,
            worker,
            steps,
            snapshot,
            &control,
            &root.root_str(),
            "run-2",
        );
        let _confirmer = spawn_confirmer(&control, true);
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Completed, "第二轮续走至全 pass");

        let entries = store
            .list_phase_records(CHANGE_ID)
            .expect("读相位条目应成功");
        // attempt 序跨 run 延续递增：proposal [1(fail, run-1), 2(pass, run-2)]，
        // 后续相位各 1 attempt（零重头重执行——attempt 不重开）
        let proposal: Vec<u32> = entries
            .iter()
            .filter(|entry| entry.phase == "proposal")
            .map(|entry| entry.attempt)
            .collect();
        assert_eq!(
            proposal,
            vec![1, 2],
            "proposal attempt 2 承接自 run-2（跨 run max+1 递增不撞号）"
        );
        let phases: Vec<&str> = entries.iter().map(|entry| entry.phase.as_str()).collect();
        assert_eq!(
            phases,
            vec![
                "proposal",
                "proposal",
                "dev-design",
                "test-design",
                "implement",
                "test-gen",
                "test-execution",
                "code-review",
                "acceptance",
                "archive",
            ],
            "落账序列严格前向（续走锚点推进——已 pass 相位零重执行，末位归档收口）"
        );
        // 两轮 run 全史：库面恰两行、run-1 stopped / run-2 completed
        let runs = store.list_change_runs(CHANGE_ID).expect("库读史应成功");
        assert_eq!(
            runs.iter()
                .map(|row| row.run_id.as_str())
                .collect::<Vec<_>>(),
            vec!["run-1", "run-2"],
            "两轮 run 全史"
        );
        assert_eq!(runs[0].status, RunStatus::Stopped);
        assert_eq!(runs[1].status, RunStatus::Completed);
    }
}

#[tokio::test]
async fn run_started_err_fail_fast_零相位执行() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[])])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let guard_history = RecordingHistory::new();
    guard_history.arm_start_failure("store 已坏注入");

    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    let request = RunRequest {
        root: "/tmp/root".to_owned(),
        main_root: "/tmp/root".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        run_id: "run-1".to_owned(),
        auto_next_phase: false,
        stop_before: None,
        started_at: STARTED_AT,
    };
    let task = tokio::spawn(walk_run(
        worker,
        tools,
        Arc::new(FakeVcs::clean()) as Arc<dyn ArchiveVcsPort>,
        snapshot,
        guard_history.clone() as Arc<dyn RunHistoryPort>,
        Arc::clone(&control),
        guard,
        request,
    ));
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Failed, "fail-fast 收敛 failed");
    assert_eq!(
        commands.lock().expect("命令锁").len(),
        0,
        "工具步零调用（零相位执行）"
    );
    assert_eq!(requests.lock().expect("请求锁").len(), 0, "会话零调用");
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "注册表除名（guard.finish 仍达）"
    );
    drop(rx);
    let _ = guard_history.start_count(); // 起始行注入失败不入捕获（0 为合法）
}

#[tokio::test]
async fn run_finished_err_best_effort_静默收口() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let history = RecordingHistory::new();
    history.arm_finish_failure("收口落包注入失败");

    let guard = control
        .begin_run(ROOT, CHANGE_ID, "run-1".to_owned(), STARTED_AT)
        .unwrap();
    let mut rx = control.subscribe(ROOT, CHANGE_ID).unwrap();
    let request = RunRequest {
        root: "/tmp/root".to_owned(),
        main_root: "/tmp/root".to_owned(),
        change_id: CHANGE_ID.to_owned(),
        run_id: "run-1".to_owned(),
        auto_next_phase: false,
        stop_before: None,
        started_at: STARTED_AT,
    };
    let task = tokio::spawn(walk_run(
        worker,
        tools,
        Arc::new(FakeVcs::clean()) as Arc<dyn ArchiveVcsPort>,
        snapshot,
        history.clone() as Arc<dyn RunHistoryPort>,
        Arc::clone(&control),
        guard,
        request,
    ));
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "best-effort：终态照常返回"
    );
    assert_eq!(
        history.finish_count(),
        0,
        "Err 注入下零成功捕获（单次尝试不重试）"
    );
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_none(),
        "除名不受阻（注册表无条目）"
    );
    // 订阅释放：Finished 通知照常流出（guard.finish 广播面不受落包失败影响）
    let mut saw_finished = false;
    while let Ok(notice) = rx.try_recv() {
        if matches!(notice, RunNotice::Finished) {
            saw_finished = true;
        }
    }
    assert!(saw_finished, "终态通知照常广播（收口语义不受损）");
}

// ---------------------------------------------------------------------------
// feedback 重跑新节点（stable instance key）+ 会话窗口铸造边界
// ---------------------------------------------------------------------------

/// feedback 重跑新节点（正向）：static-check 失败 → executor 修复重跑 → 相位通过：
/// 图上 executor 首次 / 修复各为独立节点（`instance_no` 递增），边 kind =
/// feedback（phase 内回边，区别于 retry）；phase-log 步收口 attempt 判据
///（AC-4 主锚）。
#[tokio::test]
async fn feedback重跑executor两独立节点且边为feedback() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            done_outcome(),
        ])
        .with_static_check(vec![failing_check("类型错误"), passing_check()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let finish = history.finish().expect("finish 落包捕获");

    // executor 两节点：同 (phase, attempt, step) 而 instance_no 递增
    let executors: Vec<&workflow::state::RunNodeEntry> = finish
        .nodes
        .iter()
        .filter(|node| node.step == workflow::state::RunStepKind::Executor)
        .collect();
    assert_eq!(executors.len(), 2, "首次 + 修复重跑各为独立节点");
    assert_eq!(executors[0].instance_no, 0);
    assert_eq!(executors[1].instance_no, 1, "instance_no 递增");
    assert_eq!(executors[0].phase, executors[1].phase);
    assert_eq!(executors[0].attempt, executors[1].attempt);
    assert_eq!(
        executors[1].status,
        workflow::state::RunStepStatus::Passed,
        "修复轮通过收口"
    );
    assert!(
        finish.nodes.iter().any(|node| node
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("类型错误"))),
        "static-check 失败记因随节点落包（反馈边诊断原料）"
    );

    // 边 kind：static_check:0 → executor:1 为 feedback（phase 内回边）
    let feedback: Vec<&workflow::state::RunEdgeEntry> = finish
        .edges
        .iter()
        .filter(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Feedback)
        .collect();
    assert!(!feedback.is_empty(), "feedback 边在案（区别于 retry）");
    assert!(
        feedback.iter().any(|edge| {
            edge.from_node_key == "implement:1:static_check:0"
                && edge.to_node_key == "implement:1:executor:1"
        }),
        "feedback 端点指向门禁步与新实例，实际: {:?}",
        feedback
            .iter()
            .map(|edge| (edge.from_node_key.clone(), edge.to_node_key.clone()))
            .collect::<Vec<_>>()
    );
    // 零 retry 边（本 run 无相位 fail 重跑——feedback 与 retry 不混淆）
    assert!(
        !finish
            .edges
            .iter()
            .any(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Retry),
        "相位内回边不误记为 retry"
    );
}

// ---------------------------------------------------------------------------
// 四边 kind（AC-3 / AC-4）：forward / feedback / retry / backtrack 各自落边 +
// 整包包闭性（悬空端点零落边、组合边界显式拒绝）
// ---------------------------------------------------------------------------

/// 节点身份串集合（`{phase}:{attempt}:{step}:{instance_no}`——与边端点身份串同
/// 式，跨侧逐字可比）。
fn node_key_set(nodes: &[RunNodeEntry]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| {
            format!(
                "{}:{}:{}:{}",
                node.phase,
                node.attempt,
                node.step.as_str(),
                node.instance_no
            )
        })
        .collect()
}

/// 边整包包闭断言：每条边两端恒落在本 run 节点身份集内且无自环（悬空端点 =
/// 整包不变量破例）；返回节点身份串集合供逐边断言复用。
fn assert_edges_closed(
    nodes: &[RunNodeEntry],
    edges: &[workflow::state::RunEdgeEntry],
) -> Vec<String> {
    let keys = node_key_set(nodes);
    for edge in edges {
        assert!(
            keys.contains(&edge.from_node_key) && keys.contains(&edge.to_node_key),
            "边端点恒指本 run 节点身份串（整包包闭，无悬空）: {edge:?}"
        );
        assert_ne!(edge.from_node_key, edge.to_node_key, "自环边零落边");
    }
    keys
}

/// 四边 kind 正向（一次 run 覆盖四边 emit）：计划序推进落 `forward`、静态检查
/// 失败回同名 agent 步落 `feedback`、相位 fail 同相重跑落 `retry`、决策 backtrack
/// 跨相位回跳落 `backtrack`——同一 run 的四类边各自可辨（AC-3 / AC-4）。
#[tokio::test]
async fn 四边kind正向_一次run覆盖四边kind各自落边() {
    let (root, db) = decision_fixture("edge-all-kinds");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(fail_json("首轮未过")), Ok(fail_json("次轮仍未过"))])
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计返工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // ① implement 首轮：静态检查失败 → feedback 回边；evaluator fail → 相位 fail
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            // ② implement 同相重跑（上一轮 fail 记因在案）→ retry 入边
            route_with_round(
                "implement",
                &["proposal", "dev-design", "implement"],
                2,
                Some(LastResult {
                    phase: "implement".to_owned(),
                    verdict: Verdict::Fail,
                    report: "首轮未过".to_owned(),
                    timestamp: None,
                }),
            ),
            // ③ 预算耗尽 → 决策分叉决议 backtrack（to ∈ 白名单）→ 跨相位回跳
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "次轮仍未过",
            ),
            // ④ 回跳目标相位自该处重跑 → 绿跑收口
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .with_static_check(vec![failing_check("类型错误"), passing_check()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "回跳重跑后走完收口");

    let finish = history.finish().expect("finish 落包捕获");
    let keys = assert_edges_closed(&finish.nodes, &finish.edges);
    let row = |from: &str, to: &str, kind: workflow::state::RunEdgeKind| {
        finish.edges.iter().any(|edge| {
            edge.from_node_key == from && edge.to_node_key == to && edge.edge_kind == kind
        })
    };
    use workflow::state::RunEdgeKind;
    // forward：计划序推进（相位内步序 + 相位间推进）
    assert!(
        row(
            "implement:2:phase_start:0",
            "implement:2:executor:0",
            RunEdgeKind::Forward
        ),
        "forward 落边（计划序推进），实际边集: {:?}",
        finish
            .edges
            .iter()
            .map(|edge| (
                edge.from_node_key.clone(),
                edge.to_node_key.clone(),
                edge.edge_kind
            ))
            .collect::<Vec<_>>()
    );
    // feedback：静态检查失败回本相位 executor（新 instance_no 修复重跑）
    assert!(
        row(
            "implement:1:static_check:0",
            "implement:1:executor:1",
            RunEdgeKind::Feedback
        ),
        "feedback 落边（相位内回边）"
    );
    // retry：相位 fail 后同相位重跑（attempt 递增）
    assert!(
        row(
            "implement:1:phase_log:0",
            "implement:2:phase_start:0",
            RunEdgeKind::Retry
        ),
        "retry 落边（同相重跑）"
    );
    // backtrack：决策回跳跨相位（源 = 失败相位末节点、目标 = 回跳相位首节点；
    // 决策门节点 attempt 取决策输入（库内该相位最近 attempt））
    assert!(
        row(
            "implement:1:retry_gate:0",
            "dev-design:3:phase_start:0",
            RunEdgeKind::Backtrack
        ),
        "backtrack 落边（跨相位回跳）"
    );

    // 四 kind 各自齐备且三条特殊边各恰一条（forward 计划序多条）
    let kinds: Vec<&str> = finish
        .edges
        .iter()
        .map(|edge| edge.edge_kind.as_str())
        .collect();
    for word in ["forward", "feedback", "retry", "backtrack"] {
        assert!(kinds.contains(&word), "四 kind 边 {word} 应在场: {kinds:?}");
    }
    for word in ["feedback", "retry", "backtrack"] {
        assert_eq!(
            kinds.iter().filter(|kind| **kind == word).count(),
            1,
            "特殊边 {word} 恰一条: {kinds:?}"
        );
    }
    assert!(
        !kinds
            .iter()
            .any(|kind| !["forward", "feedback", "retry", "backtrack"].contains(kind)),
        "边 kind 零越四值封闭集: {kinds:?}"
    );
    assert!(
        keys.contains(&"dev-design:3:phase_start:0".to_owned()),
        "回跳目标节点在案: {keys:?}"
    );
}

/// 四边 kind 正向（retry）：相位 evaluator fail → 同相位重跑（attempt 递增）
/// 落 `retry` 边（源 = 上一轮相位末节点、目标 = 重跑相位首节点）；计划序推进
/// 落 `forward` 边，相位内回边（feedback）与跨相位回跳（backtrack）零误记
///（AC-3 / AC-4）。
#[tokio::test]
async fn 四边kind正向_相位fail同相重跑落retry边() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(fail_json("首轮未过")), Ok(PASS_JSON.to_owned())])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_with_round("implement", &["implement"], 1, None),
            route_with_round(
                "implement",
                &["implement"],
                2,
                Some(LastResult {
                    phase: "implement".to_owned(),
                    verdict: Verdict::Fail,
                    report: "首轮未过".to_owned(),
                    timestamp: None,
                }),
            ),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "重跑通过后走完收口");

    let finish = history.finish().expect("finish 落包捕获");
    let retry: Vec<&workflow::state::RunEdgeEntry> = finish
        .edges
        .iter()
        .filter(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Retry)
        .collect();
    assert_eq!(retry.len(), 1, "相位 fail 同相重跑恰落一条 retry 边");
    assert_eq!(
        retry[0].from_node_key, "implement:1:phase_log:0",
        "retry 源 = 上一轮相位末节点（fail 收口处）"
    );
    assert_eq!(
        retry[0].to_node_key, "implement:2:phase_start:0",
        "retry 目标 = 重跑相位首节点（attempt 递增）"
    );

    let kinds: Vec<&str> = finish
        .edges
        .iter()
        .map(|edge| edge.edge_kind.as_str())
        .collect();
    assert!(
        kinds.contains(&"forward"),
        "计划序推进落 forward 边: {kinds:?}"
    );
    assert!(
        !kinds.contains(&"feedback"),
        "相位 fail 重跑不误记 feedback（相位内回边另有其边）: {kinds:?}"
    );
    assert!(
        !kinds.contains(&"backtrack"),
        "同相重跑不误记 backtrack（跨相位回跳另有其边）: {kinds:?}"
    );

    let keys = assert_edges_closed(&finish.nodes, &finish.edges);
    assert!(
        keys.contains(&retry[0].from_node_key) && keys.contains(&retry[0].to_node_key),
        "retry 两端节点在案（图面闭合）"
    );
}

/// 四边 kind 正向（backtrack）：决策分叉决议 backtrack（to ∈ 白名单）→ 跨相位
/// 回跳落 `backtrack` 边（源 = 失败相位末节点、目标 = 回跳目标相位首节点）；
/// 同相 retry 零误记（AC-3 / AC-4）。
#[tokio::test]
async fn 四边kind正向_决策回跳跨相位落backtrack边() {
    let (root, db) = decision_fixture("edge-backtrack");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计返工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "backtrack 重路由后收敛");

    let finish = history.finish().expect("finish 落包捕获");
    let backtrack: Vec<&workflow::state::RunEdgeEntry> = finish
        .edges
        .iter()
        .filter(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Backtrack)
        .collect();
    assert_eq!(backtrack.len(), 1, "跨相位回跳恰落一条 backtrack 边");
    assert_eq!(
        backtrack[0].from_node_key, "implement:1:retry_gate:0",
        "backtrack 源 = 失败相位末节点（决策门收口处）"
    );
    assert_eq!(
        backtrack[0].to_node_key, "dev-design:2:phase_start:0",
        "backtrack 目标 = 回跳目标相位首节点（跨相位身份串）"
    );

    let kinds: Vec<&str> = finish
        .edges
        .iter()
        .map(|edge| edge.edge_kind.as_str())
        .collect();
    assert!(
        kinds.contains(&"forward"),
        "计划序与决策门内推进落 forward 边: {kinds:?}"
    );
    assert!(
        !kinds.contains(&"retry"),
        "跨相位回跳不误记 retry（同相重跑另有其边）: {kinds:?}"
    );

    let keys = assert_edges_closed(&finish.nodes, &finish.edges);
    assert!(
        keys.contains(&backtrack[0].to_node_key),
        "回跳目标节点在案（图面闭合）"
    );
}

/// 四边 kind 异常（整包不变量）：边端点悬空（from / to 不在本 run 节点集内）
/// → walker 侧零悬空落边（端点恒自节点身份分配单点派生、同源双端），组合边界
/// （真件 store 收口单事务）显式拒绝悬空包且零半包入库（AC-3 / AC-5）。
#[tokio::test]
async fn 四边kind异常_悬空端点零落边与整包拒绝零半包() {
    // ① walker 侧：静态检查反馈边 run（forward + feedback 多边）→ 落包边整包
    //    包闭、零自环（悬空端点不可构造）
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            done_outcome(),
        ])
        .with_static_check(vec![failing_check("类型错误"), passing_check()])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let finish = history.finish().expect("finish 落包捕获");
    assert!(!finish.edges.is_empty(), "本案多边在前（包闭断言非空转）");
    let keys = assert_edges_closed(&finish.nodes, &finish.edges);
    assert!(
        keys.contains(&"implement:1:executor:1".to_owned()),
        "反馈重跑新实例节点在案: {keys:?}"
    );

    // ② 组合边界：真件 store 收口单事务——悬空端点整包显式拒绝、终态零写、
    //    节点 / 边零落库（无半包），run 行仍 running
    let db = TestDb::open("dangling-edge");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    db.store
        .start_change_run(&RunStartCommand {
            run_id: "run-dangling".to_owned(),
            change_id: CHANGE_ID.to_owned(),
            started_at: STARTED_AT,
        })
        .expect("起始行应成功");

    let mut corrupted = finish.clone();
    corrupted.run_id = "run-dangling".to_owned();
    corrupted.edges.push(workflow::state::RunEdgeEntry {
        from_node_key: "implement:1:retry_gate:0".to_owned(),
        to_node_key: keys[0].clone(),
        edge_kind: workflow::state::RunEdgeKind::Backtrack,
    });
    assert!(
        !keys.contains(&"implement:1:retry_gate:0".to_owned()),
        "注入端点为悬空形态（不在本包节点集内）"
    );

    let error = db
        .store
        .finish_change_run(&corrupted)
        .expect_err("悬空端点整包应收口拒绝");
    assert!(
        matches!(&error, store::StoreError::Conflict(_)),
        "整包不变量破例 → 冲突面显式拒绝: {error}"
    );
    assert!(
        error.to_string().contains("悬空"),
        "记因点名悬空端点: {error}"
    );
    assert!(
        db.store
            .list_run_nodes("run-dangling")
            .expect("节点读试应成功")
            .is_empty(),
        "半包节点零落库（整事务回滚）"
    );
    assert!(
        db.store
            .list_run_edges("run-dangling")
            .expect("边读试应成功")
            .is_empty(),
        "半包边零落库（整事务回滚）"
    );
    let runs = db
        .store
        .list_change_runs(CHANGE_ID)
        .expect("run 清单应成功");
    assert_eq!(runs.len(), 1, "起始行在案");
    assert_eq!(
        runs[0].status,
        RunStatus::Running,
        "半包拒绝不写终态（run 行仍 running）"
    );
}

/// `new_run_id` 铸造面（边界 / 异常）：恒 `run-<millis>` 形态、两次铸造互异；
/// 同毫秒连铸不 panic（不依赖全局单调计数）。并发铸造（多线程）逐值可辨。
#[test]
fn new_run_id铸造定式与互异不panic() {
    let first = new_run_id();
    let second = new_run_id();
    assert!(first.starts_with("run-"), "run-<millis> 定式: {first}");
    assert!(
        first[4..].chars().all(|ch| ch.is_ascii_digit()),
        "毫秒段为十进制数字: {first}"
    );
    assert!(first.len() > 4, "毫秒段非空");

    // 同毫秒连铸：不 panic 且形态持衡（毫秒级唯一铸造面为 store 主键兜底）
    let burst: Vec<String> = (0..64).map(|_| new_run_id()).collect();
    for id in &burst {
        assert!(id.starts_with("run-"));
    }

    // 并发铸造（时钟回拨 / 同毫秒形态不 panic）
    let handles: Vec<std::thread::JoinHandle<String>> =
        (0..4).map(|_| std::thread::spawn(new_run_id)).collect();
    let mut concurrent: Vec<String> = handles
        .into_iter()
        .map(|handle| handle.join().expect("铸造线程不 panic"))
        .collect();
    concurrent.push(first);
    concurrent.push(second);
    assert!(
        concurrent.iter().all(|id| id.starts_with("run-")),
        "并发铸造形态持衡"
    );
}

// ---------------------------------------------------------------------------
// 归档相位（AC-3 / AC-4 / AC-6 / AC-7 / AC-8）：十步执行序、git 步走 vcs 缝、
// 条件步短路、停等门、失败停链与组合链路落库
// ---------------------------------------------------------------------------

/// 归档相位十步执行序（正向）：假写面 + 假 vcs 驱动 archive 相位 →
/// PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge →
/// SpecSync → Seal → Finalize → PhaseLog 按序落节点（emit 序即落包序），全步
/// `passed`（AC-3）。
#[tokio::test]
async fn 归档相位十步执行序按计划步序落包() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let vcs = Arc::new(FakeVcs::clean());
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::clone(&vcs),
        Arc::clone(&snapshot),
        &control,
        true,
        "/tmp/root",
        "/tmp/root",
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Completed, "归档完成 → run 收口");
    let command = history.finish().expect("收口落包应在案");
    let rows = node_rows(&command);
    let steps: Vec<&str> = rows.iter().map(|row| row.1.as_str()).collect();
    assert_eq!(
        steps,
        [
            "phase_start",
            "preflight",
            "commit",
            "rebase",
            "resolve_conflicts",
            "merge",
            "spec_sync",
            "seal",
            "finalize",
            "phase_log"
        ],
        "归档十步序与设计表逐位一致"
    );
    assert!(
        rows.iter().all(|row| row.0 == "archive"),
        "归档步节点恒 phase=archive: {rows:?}"
    );
    assert!(
        rows.iter().all(|row| row.2 == "passed"),
        "全步 passed（legacy 形态各步内短路）: {rows:?}"
    );
    let labels: Vec<&str> = commands
        .lock()
        .expect("命令锁")
        .iter()
        .map(command_label)
        .collect();
    assert_eq!(
        labels,
        [
            "phase-next",
            "archive-preflight",
            "phase-start",
            "archive-preflight",
            "archive-seal",
            "phase-log",
            "phase-next"
        ],
        "归档相位工具命令序（停等门 + Preflight 步各一次；legacy 形态零 git 触点）"
    );
    let timeline_roles: Vec<String> = timeline
        .lock()
        .expect("时间线锁")
        .iter()
        .filter(|entry| {
            matches!(
                entry.as_str(),
                "executor" | "evaluator" | "decision" | "spec_sync" | "resolve_conflicts"
            )
        })
        .cloned()
        .collect();
    assert!(
        timeline_roles.is_empty(),
        "零 agent 会话（归档确定性相位）: {timeline_roles:?}"
    );
}

/// git 步走 vcs 缝（正向）：worktree 形态 + 脏工作区 → Commit / Rebase /
/// Merge / Finalize 四步经 `vcs` 缝直调（`commit_all` / `rebase_branch` /
/// `ff_merge` / `commit_paths` 逐一捕获——core 零 spawn 的行为面锚，AC-3 / R2）。
#[tokio::test]
async fn 归档git四步经vcs缝直调() {
    let root = TempRoot::new("archive-git-seam");
    root.change(NAME);
    let worktree = worktree_dir(&root);
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let vcs = Arc::new(FakeVcs::clean().with_dirty(true));
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::clone(&vcs),
        Arc::clone(&snapshot),
        &control,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Completed);
    let calls = vcs.calls();
    assert_eq!(
        vcs.call_count("commit_all:"),
        1,
        "Commit 步经 vcs 直调: {calls:?}"
    );
    assert_eq!(
        vcs.call_count("rebase_branch:"),
        1,
        "Rebase 步经 vcs 直调: {calls:?}"
    );
    assert_eq!(
        vcs.call_count("ff_merge:"),
        1,
        "Merge 步经 vcs 直调: {calls:?}"
    );
    assert_eq!(
        vcs.call_count("commit_paths:"),
        1,
        "Finalize 步经 vcs 直调: {calls:?}"
    );
    assert_eq!(
        vcs.call_count("current_branch"),
        1,
        "合入目标分支探测同走 vcs 缝: {calls:?}"
    );
    let path_sets = vcs.dirty_path_sets();
    assert!(
        path_sets
            .iter()
            .flatten()
            .any(|path| path.contains("changes/archive/")),
        "Finalize 脏探测携归档目录 pathspec: {path_sets:?}"
    );

    let command = history.finish().expect("收口落包应在案");
    let commit_detail = node_detail(&command, workflow::state::RunStepKind::Commit);
    assert!(
        commit_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("worktree 已提交")),
        "Commit 步 detail 携提交摘要: {commit_detail:?}"
    );
    let seal_detail = node_detail(&command, workflow::state::RunStepKind::Seal);
    assert!(
        seal_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("已归档")),
        "Seal 步 detail 携归档产出摘要: {seal_detail:?}"
    );
}

/// Rebase → ResolveConflicts → Merge 三段（正向）：Rebase 产出冲突清单 →
/// ResolveConflicts 有冲突即唤解冲突 agent（`resolve_conflicts` 角色）并经
/// `rebase_continue` 收口 → Merge 独立 `ff_merge`（AC-3 / D4）。
#[tokio::test]
async fn 归档冲突三段agent解算与独立快进() {
    let root = TempRoot::new("archive-conflict");
    root.change(NAME);
    let worktree = worktree_dir(&root);
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let vcs = Arc::new(
        FakeVcs::clean()
            .with_rebase(Ok(RebaseOutcome::Conflicted(vec![
                "specs/a.md".to_owned()
            ])))
            .with_snapshot_head("head-b", Some("onto-commit")),
    );
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::clone(&vcs),
        Arc::clone(&snapshot),
        &control,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Completed, "冲突解算 + 快进收口");
    let roles: Vec<WorkerRole> = sessions
        .lock()
        .expect("会话锁")
        .iter()
        .map(|(role, _)| *role)
        .collect();
    assert_eq!(
        roles,
        vec![WorkerRole::ResolveConflicts],
        "有冲突 → 唤解冲突 agent（无其它会话）"
    );
    assert!(
        requests
            .lock()
            .expect("请求锁")
            .first()
            .is_some_and(|request| request.prompt.contains("specs/a.md")),
        "解冲突 prompt 携逐条冲突路径"
    );
    assert_eq!(
        vcs.call_count("rebase_continue"),
        1,
        "解算后由链代续走: {:?}",
        vcs.calls()
    );
    assert_eq!(vcs.call_count("ff_merge:"), 1, "Merge 独立快进");
    assert_eq!(vcs.call_count("worktree_snapshot"), 2, "A / B 双快照后验");

    let command = history.finish().expect("收口落包应在案");
    let rebase_detail = node_detail(&command, workflow::state::RunStepKind::Rebase);
    assert!(
        rebase_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("冲突待解算")),
        "Rebase 步 detail 携冲突交据: {rebase_detail:?}"
    );
}

/// ResolveConflicts 条件执行（边界）：`rebase_conflicts` 为空（legacy 跳过 /
/// 重放无冲突）→ ResolveConflicts `passed` + detail 携「无冲突」，零 agent 会话
///（AC-3）。
#[tokio::test]
async fn 归档无冲突时解算步passed且零会话() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let vcs = Arc::new(FakeVcs::clean());
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::clone(&vcs),
        Arc::clone(&snapshot),
        &control,
        true,
        "/tmp/root",
        "/tmp/root",
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    assert_eq!(
        task.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );

    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "无冲突 → 零 agent 会话"
    );
    assert!(sessions.lock().expect("会话锁").is_empty());
    assert_eq!(vcs.call_count("rebase_continue"), 0, "零冲突不解算");
    let command = history.finish().expect("收口落包应在案");
    let resolve_node = command
        .nodes
        .iter()
        .find(|node| node.step == workflow::state::RunStepKind::ResolveConflicts)
        .expect("ResolveConflicts 节点在场");
    assert_eq!(resolve_node.status, RunStepStatus::Passed);
    assert_eq!(
        resolve_node.detail.as_deref(),
        Some("无冲突"),
        "跳过因单点词汇"
    );
    assert!(resolve_node.session_id.is_none(), "零会话步节点无 session_id");
}

/// SpecSync 条件执行（边界 / 正向）：delta specs 在场且用户未跳过 → agent 会话
///（`WorkerRole::SpecSync`、provenance `<id>/archive/spec_sync/<attempt>`、cwd 恒
/// 主 root）；无 delta / 用户勾选跳过 → `passed` + detail 携跳过因（AC-3 / D2 / D3）。
#[tokio::test]
async fn 归档spec同步条件执行与用户跳过选择() {
    let root = TempRoot::new("archive-spec-sync");
    root.change(NAME);
    let worktree = worktree_dir(&root);
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);

    // ① delta specs 在场 + 用户未跳过 → agent 会话
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(
            &worktree,
            vec!["desktop-flow".to_owned()],
        ))
        .assemble();
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer = spawn_confirmer_with_specs(&control, true, true);
    assert_eq!(
        task.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );

    let spec_sync_requests: Vec<WorkerTurnRequest> = requests
        .lock()
        .expect("请求锁")
        .iter()
        .filter(|request| request.role == WorkerRole::SpecSync)
        .cloned()
        .collect();
    assert_eq!(spec_sync_requests.len(), 1, "delta specs 在场 → spec 同步会话");
    assert_eq!(
        spec_sync_requests[0].root,
        root.root_str(),
        "cwd 恒主 workspace root"
    );
    assert_eq!(
        spec_sync_requests[0].provenance.source_ref.as_deref(),
        Some(format!("{CHANGE_ID}/archive/spec_sync/1").as_str()),
        "provenance 角色段 = spec_sync（同 phase 同 attempt 唯一）"
    );
    let command = history.finish().expect("收口落包应在案");
    let spec_node = command
        .nodes
        .iter()
        .find(|node| node.step == workflow::state::RunStepKind::SpecSync)
        .expect("SpecSync 节点在场");
    assert!(spec_node.session_id.is_some(), "会话步节点携 session_id");

    // ② delta specs 在场 + 用户勾选跳过 → passed + 跳过因（零会话）
    let timeline_b = Arc::new(Mutex::new(Vec::new()));
    let (worker_b, requests_b, _sessions_b) = FakeWorker::new(&timeline_b).assemble();
    let (tools_b, _commands_b) = FakeTools::new(&timeline_b)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(
            &worktree,
            vec!["desktop-flow".to_owned()],
        ))
        .assemble();
    let control_b = Arc::new(ChangeFlowControl::new());
    let (task_b, _rx_b, history_b) = spawn_run_with_vcs(
        worker_b,
        tools_b,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control_b,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer_b = spawn_confirmer_with_specs(&control_b, true, false);
    assert_eq!(
        task_b.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );
    assert!(
        requests_b.lock().expect("请求锁").is_empty(),
        "用户跳过 → 零 spec 同步会话"
    );
    let command_b = history_b.finish().expect("收口落包应在案");
    assert_eq!(
        node_detail(&command_b, workflow::state::RunStepKind::SpecSync).as_deref(),
        Some("用户选择")
    );

    // ③ 无 delta specs → passed + 无 delta 跳过因
    let timeline_c = Arc::new(Mutex::new(Vec::new()));
    let (worker_c, _requests_c, _sessions_c) = FakeWorker::new(&timeline_c).assemble();
    let (tools_c, _commands_c) = FakeTools::new(&timeline_c)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .assemble();
    let control_c = Arc::new(ChangeFlowControl::new());
    let (task_c, _rx_c, history_c) = spawn_run_with_vcs(
        worker_c,
        tools_c,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control_c,
        true,
        "/tmp/root",
        "/tmp/root",
        Some("archive"),
    );
    let _confirmer_c = spawn_confirmer(&control_c, true);
    assert_eq!(
        task_c.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );
    let command_c = history_c.finish().expect("收口落包应在案");
    assert_eq!(
        node_detail(&command_c, workflow::state::RunStepKind::SpecSync).as_deref(),
        Some("无 delta specs")
    );
}

/// 步内短路形态（边界 / 幂等重试）：干净 worktree → Commit `passed` + detail
/// 携「干净」（不调 `commit_all`）；已落盘 → Finalize `passed` + 跳过因（不调
/// `commit_paths`）；已合入 → Rebase / Merge `passed` + 跳过因（不调
/// `rebase_branch` / `ff_merge`）；legacy 记录 → 三步跳过因（AC-8 幂等重试）。
#[tokio::test]
async fn 归档步内短路形态与幂等跳过() {
    let root = TempRoot::new("archive-short-circuit");
    root.change(NAME);
    let worktree = worktree_dir(&root);
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);

    // ① 干净 worktree（dirty=false）：Commit / Finalize 跳过
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
        .assemble();
    let control = Arc::new(ChangeFlowControl::new());
    let vcs = Arc::new(FakeVcs::clean().with_dirty(false));
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::clone(&vcs),
        Arc::clone(&snapshot),
        &control,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    assert_eq!(
        task.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );
    assert_eq!(vcs.call_count("commit_all:"), 0, "干净 → 不调 commit_all");
    assert_eq!(
        vcs.call_count("commit_paths:"),
        0,
        "已落盘 → 不调 commit_paths"
    );
    let command = history.finish().expect("收口落包应在案");
    assert_eq!(
        node_detail(&command, workflow::state::RunStepKind::Commit).as_deref(),
        Some("干净")
    );
    assert_eq!(
        node_detail(&command, workflow::state::RunStepKind::Finalize).as_deref(),
        Some("已落盘")
    );

    // ② 已合入（branch_merged=true）：Rebase / Merge 跳过
    let timeline_b = Arc::new(Mutex::new(Vec::new()));
    let (worker_b, _requests_b, _sessions_b) = FakeWorker::new(&timeline_b).assemble();
    let (tools_b, _commands_b) = FakeTools::new(&timeline_b)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
        .assemble();
    let control_b = Arc::new(ChangeFlowControl::new());
    let vcs_b = Arc::new(FakeVcs::clean().with_merged(Ok(true)));
    let (task_b, _rx_b, history_b) = spawn_run_with_vcs(
        worker_b,
        tools_b,
        Arc::clone(&vcs_b),
        Arc::clone(&snapshot),
        &control_b,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer_b = spawn_confirmer(&control_b, true);
    assert_eq!(
        task_b.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );
    assert_eq!(vcs_b.call_count("rebase_branch:"), 0, "已合入 → 不重放");
    assert_eq!(vcs_b.call_count("ff_merge:"), 0, "已合入 → 不快进");
    let command_b = history_b.finish().expect("收口落包应在案");
    assert_eq!(
        node_detail(&command_b, workflow::state::RunStepKind::Rebase).as_deref(),
        Some("已合入")
    );
    assert_eq!(
        node_detail(&command_b, workflow::state::RunStepKind::Merge).as_deref(),
        Some("已合入")
    );

    // ③ legacy 记录（worktree=None）：Commit / Rebase / Merge 跳过因
    let timeline_c = Arc::new(Mutex::new(Vec::new()));
    let (worker_c, _requests_c, _sessions_c) = FakeWorker::new(&timeline_c).assemble();
    let (tools_c, _commands_c) = FakeTools::new(&timeline_c)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .assemble();
    let control_c = Arc::new(ChangeFlowControl::new());
    let vcs_c = Arc::new(FakeVcs::clean());
    let (task_c, _rx_c, history_c) = spawn_run_with_vcs(
        worker_c,
        tools_c,
        Arc::clone(&vcs_c),
        snapshot,
        &control_c,
        true,
        "/tmp/root",
        "/tmp/root",
        Some("archive"),
    );
    let _confirmer_c = spawn_confirmer(&control_c, true);
    assert_eq!(
        task_c.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );
    let legacy_calls = vcs_c.calls();
    assert_eq!(
        vcs_c.call_count("commit_all:"),
        0,
        "legacy → 不调 commit_all: {legacy_calls:?}"
    );
    assert_eq!(vcs_c.call_count("rebase_branch:"), 0, "legacy → 不重放");
    assert_eq!(vcs_c.call_count("ff_merge:"), 0, "legacy → 不快进");
    assert_eq!(vcs_c.call_count("commit_paths:"), 0, "已落盘 → 不落盘提交");
    assert_eq!(
        legacy_calls,
        vec!["dirty:2".to_owned()],
        "legacy 形态仅 Finalize pathspec 脏探测一次"
    );
    let command_c = history_c.finish().expect("收口落包应在案");
    for step in [
        workflow::state::RunStepKind::Commit,
        workflow::state::RunStepKind::Rebase,
        workflow::state::RunStepKind::Merge,
    ] {
        assert_eq!(
            node_detail(&command_c, step).as_deref(),
            Some("legacy 无 worktree"),
            "{step:?} legacy 跳过因"
        );
    }
}

/// stop-before-archive 停等（正向）：`stop_before=Some("archive")` → 发
/// `ConfirmWait { phase: "archive", archive: Some(preflight) }` 并停等；
/// `proceed=false` → stopped（未进归档相位）、`proceed=true` → 进 archive
///（AC-7 / D2）。
#[tokio::test]
async fn stop_before_archive停等载荷与双向选择() {
    // ① proceed=false：停在归档前（零 archive 步执行）
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome()])
        .with_archive_preflight(archive_preflight_twice(
            "/tmp/wt/walker-change",
            vec!["desktop-flow".to_owned()],
        ))
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control,
        false,
        "/tmp/root",
        "/tmp/root",
        Some("archive"),
    );
    let (confirmer, seen) = spawn_confirmer_capturing(&control, false, true);
    let status = task.await.expect("run 任务正常结束");
    confirmer.abort();

    assert_eq!(status, ChangeRunStatus::Stopped, "proceed=false → 受控终止");
    let seen = seen.lock().expect("停等捕获锁不可中毒").clone();
    assert_eq!(seen.len(), 1, "归档前恰一道停等确认");
    assert_eq!(seen[0].0, "archive", "停等相位 = archive");
    let payload = seen[0].1.clone().expect("归档前停等随行前置只读载荷");
    assert_eq!(payload.name, NAME);
    assert!(payload.completed, "完成度结论随行");
    assert_eq!(payload.delta_specs, vec!["desktop-flow".to_owned()]);
    assert_eq!(payload.merge_target.as_deref(), Some("main"));
    let labels: Vec<&str> = commands
        .lock()
        .expect("命令锁")
        .iter()
        .map(command_label)
        .collect();
    assert_eq!(
        labels,
        ["phase-next", "archive-preflight"],
        "停等未确认 → 归档相位零执行"
    );

    // ② proceed=true：进 archive 相位并收口 completed
    let timeline_b = Arc::new(Mutex::new(Vec::new()));
    let (worker_b, _requests_b, _sessions_b) = FakeWorker::new(&timeline_b).assemble();
    let (tools_b, commands_b) = FakeTools::new(&timeline_b)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .assemble();
    let control_b = Arc::new(ChangeFlowControl::new());
    let (task_b, _rx_b, history_b) = spawn_run_with_vcs(
        worker_b,
        tools_b,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control_b,
        false,
        "/tmp/root",
        "/tmp/root",
        Some("archive"),
    );
    let _confirmer_b = spawn_confirmer(&control_b, true);
    assert_eq!(
        task_b.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );
    let labels_b: Vec<&str> = commands_b
        .lock()
        .expect("命令锁")
        .iter()
        .map(command_label)
        .collect();
    assert!(
        labels_b.contains(&"phase-start") && labels_b.contains(&"archive-seal"),
        "确认后归档相位执行: {labels_b:?}"
    );
    let command_b = history_b.finish().expect("收口落包应在案");
    assert!(
        command_b.nodes.iter().any(|node| {
            node.phase == "archive" && node.step == workflow::state::RunStepKind::PhaseLog
        }),
        "归档相位 PhaseLog 落包（archive 完成才 Completed）"
    );
}

/// auto 模式同样停等（边界）：`auto_next_phase` 两态下归档前仍停等一次（停止门
/// 不因 auto 跳过）；归档收口后不再补确认（归档后零确认，AC-7 / D1）。
#[tokio::test]
async fn auto模式归档前仍停等且归档后零确认() {
    for auto in [true, false] {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![archive_route_outcome(), done_outcome()])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, _history) = spawn_run_with_vcs(
            worker,
            tools,
            Arc::new(FakeVcs::clean()),
            snapshot,
            &control,
            auto,
            "/tmp/root",
            "/tmp/root",
            Some("archive"),
        );
        let (confirmer, seen) = spawn_confirmer_capturing(&control, true, true);
        let status = task.await.expect("run 任务正常结束");
        confirmer.abort();

        assert_eq!(
            status,
            ChangeRunStatus::Completed,
            "auto={auto} 归档收口即 Completed"
        );
        let seen = seen.lock().expect("停等捕获锁不可中毒").clone();
        assert_eq!(
            seen.len(),
            1,
            "auto={auto} 归档前恰一道停等（归档后零确认）"
        );
        assert_eq!(seen[0].0, "archive", "停止门相位 = archive");
    }
}

/// evaluator 检查移除（正向）：archive 相位产出 `executor=None` / `evaluator=None`
/// 仍可进入执行（`drive` 无 `next.evaluator.is_none()` 硬检查）——十步照常落包
/// 并收口（AC-5）。
#[tokio::test]
async fn 归档相位零角色spec仍可进入执行() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control,
        true,
        "/tmp/root",
        "/tmp/root",
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "executor / evaluator 双 None 不拦相位进入"
    );
    assert!(requests.lock().expect("请求锁").is_empty(), "零会话角色");
    let command = history.finish().expect("收口落包应在案");
    assert_eq!(
        command.nodes.len(),
        10,
        "归档十步全落包（零 evaluator 检查无阻）: {:?}",
        node_rows(&command)
    );
    let labels: Vec<&str> = commands
        .lock()
        .expect("命令锁")
        .iter()
        .map(command_label)
        .collect();
    assert!(labels.contains(&"phase-log"), "PhaseLog 照常落账");
    assert!(!labels.contains(&"backtrack"), "零决策 / 回溯分支");
}

/// 失败停链（异常）：归档任一步失败 → `Terminal` → run 直接 `Failed` 收口，不唤
/// 决策 agent、不触发 phase retry（AC-8）。
#[tokio::test]
async fn 归档步失败即failed收口不唤决策() {
    for label in ["archive-seal", "archive-preflight", "phase-log"] {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, requests, sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![archive_route_outcome()])
            .fail_on(label)
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run_with_vcs(
            worker,
            tools,
            Arc::new(FakeVcs::clean()),
            snapshot,
            &control,
            true,
            "/tmp/root",
            "/tmp/root",
            Some("archive"),
        );
        let _confirmer = spawn_confirmer(&control, true);
        let status = task.await.expect("run 任务正常结束");

        assert_eq!(
            status,
            ChangeRunStatus::Failed,
            "归档步 {label} 失败 → run Failed"
        );
        let roles: Vec<WorkerRole> = sessions
            .lock()
            .expect("会话锁")
            .iter()
            .map(|(role, _)| *role)
            .collect();
        assert!(
            !roles.contains(&WorkerRole::Decision),
            "失败停链不唤决策 agent: {roles:?}"
        );
        assert!(
            requests.lock().expect("请求锁").is_empty(),
            "零会话（无相位重试）"
        );
        let command = history.finish().expect("收口落包应在案");
        if label == "phase-log" {
            // 落账臂失败：PhaseLog 节点已在途（running），终态收口不再改写节点面
            assert!(
                command
                    .nodes
                    .iter()
                    .any(|node| node.step == workflow::state::RunStepKind::PhaseLog),
                "落账臂失败时节点在案（在途 running）: {:?}",
                node_rows(&command)
            );
        } else {
            assert!(
                command
                    .nodes
                    .iter()
                    .any(|node| node.status == RunStepStatus::Failed),
                "失败节点在案（步失败可见）"
            );
        }
    }
}

/// 归档 git 步失败停链（异常）：脏 worktree 而 `commit_all` Err / 重放 Err /
/// 合入判定 Err → 步失败携记因 + run Failed（AC-8）。
#[tokio::test]
async fn 归档git步vcs失败即停链() {
    let cases: [(&str, FakeVcs); 3] = [
        (
            "提交失败：钩子拒绝",
            FakeVcs::clean()
                .with_dirty(true)
                .with_commit_result(Err("提交失败：钩子拒绝".to_owned())),
        ),
        (
            "重放失败：非冲突错误",
            FakeVcs::clean().with_rebase(Err("重放失败：非冲突错误".to_owned())),
        ),
        (
            "分支合入判定失败",
            FakeVcs::clean().with_merged(Err("分支合入判定失败".to_owned())),
        ),
    ];
    for (expect, vcs) in cases {
        let root = TempRoot::new("archive-git-fail");
        root.change(NAME);
        let worktree = worktree_dir(&root);
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![archive_route_outcome()])
            .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run_with_vcs(
            worker,
            tools,
            Arc::new(vcs),
            snapshot,
            &control,
            true,
            &root.root_str(),
            &root.root_str(),
            Some("archive"),
        );
        let _confirmer = spawn_confirmer(&control, true);
        assert_eq!(
            task.await.expect("run 任务正常结束"),
            ChangeRunStatus::Failed,
            "{expect} → 步失败停链"
        );
        let command = history.finish().expect("收口落包应在案");
        let failed = command
            .nodes
            .iter()
            .find(|node| node.status == RunStepStatus::Failed)
            .expect("失败节点在案");
        assert!(
            failed
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains(expect)),
            "失败节点记因携 vcs Err: {failed:?}"
        );
    }
}

/// 组合链路落库与空 checklist 落账（正向，AC-4 / AC-6）：真件 store + 真件
/// `LocalToolSteps`（假 vcs 缝）驱动归档相位收口 → archive 相位以 `report=""` +
/// 空 checklist 落 `verdict=pass` PhaseRecord；归档十步节点 + 串行 forward 边落
/// 既有三表（零新表），身份键 `(phase, attempt, step, instance_no)` 稳定唯一。
#[tokio::test]
async fn 组合链路归档落库与空checklist落账() {
    let root = TempRoot::new("archive-compose");
    root.change(NAME);
    let db = TestDb::open("archive-compose");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    for (index, phase) in [
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
    {
        seed_entry(
            db.store.as_ref(),
            CHANGE_ID,
            phase,
            Verdict::Pass,
            "通过",
            Vec::new(),
            TS_BASE + (index as i64) * 60_000,
        );
    }

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(FakeVcs::clean()),
        db.store_arc(),
        "run-archive".to_owned(),
    ));
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_real_run_archive(
        &db,
        worker,
        steps,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control,
        &root.root_str(),
        "run-archive",
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "归档完成 → run 收口");

    // 空 checklist 落账（AC-4）
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    let archive = entries
        .iter()
        .find(|entry| entry.phase == "archive")
        .expect("archive 相位落账应在案");
    assert_eq!(archive.verdict, Verdict::Pass, "空 checklist 推导 pass");
    assert_eq!(archive.report, "", "归档落账 report 为空串");
    assert!(archive.checklist.is_empty(), "归档落账 checklist 空表");
    assert!(!archive.skipped, "归档落账非跳过语义");
    assert!(
        archive.executor_session_id.is_none()
            && archive.evaluator_session_id.is_none()
            && archive.decision_session_id.is_none(),
        "归档三会话槽位恒 None"
    );

    // 执行图落库（AC-6）：十步节点 + 九条 forward 边（零新表）
    let nodes = db
        .store
        .list_run_nodes("run-archive")
        .expect("读节点史应成功");
    let steps_wire: Vec<String> = nodes
        .iter()
        .map(|node| node.step.as_str().to_owned())
        .collect();
    assert_eq!(
        steps_wire,
        [
            "phase_start",
            "preflight",
            "commit",
            "rebase",
            "resolve_conflicts",
            "merge",
            "spec_sync",
            "seal",
            "finalize",
            "phase_log"
        ],
        "归档十步全落 RunNodeRecord（零过滤）"
    );
    assert!(
        nodes
            .iter()
            .all(|node| node.phase == "archive" && node.attempt == 1 && node.instance_no == 0),
        "身份键 (phase, attempt, step, instance_no) 稳定"
    );
    let mut keys: Vec<String> = nodes
        .iter()
        .map(|node| {
            format!(
                "{}:{}:{}:{}",
                node.phase,
                node.attempt,
                node.step.as_str(),
                node.instance_no
            )
        })
        .collect();
    let total = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), total, "节点身份键稳定唯一（零撞号）");

    let edges = db
        .store
        .list_run_edges("run-archive")
        .expect("读边史应成功");
    assert_eq!(edges.len(), 9, "串行推进边 = 节点数 - 1");
    assert!(
        edges
            .iter()
            .all(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Forward),
        "归档串行边全 forward（零反馈 / 重试 / 回溯）"
    );
    assert_eq!(edges[0].from_node_key, "archive:1:phase_start:0");
    assert_eq!(edges[8].to_node_key, "archive:1:phase_log:0");

    let runs = db
        .store
        .list_change_runs(CHANGE_ID)
        .expect("读 run 史应成功");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, RunStatus::Completed);
    assert!(
        runs[0]
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("archiving")),
        "收口文案体现含归档相位: {:?}",
        runs[0].reason
    );
}

/// Seal 产出驱动 Finalize pathspec（正向）：Seal 步经写面双写产出
/// `ArchiveOutcome`（`archived_date` + name）→ Finalize 派生归档目录路径做
/// pathspec 圈定提交（产出跨步传递，AC-3）。
#[tokio::test]
async fn 归档seal产出驱动finalize落盘pathspec() {
    let root = TempRoot::new("archive-seal-pathspec");
    root.change(NAME);
    let worktree_dir_str = worktree_dir(&root);
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(
            &worktree_dir_str,
            Vec::new(),
        ))
        .with_archive_seal(vec![workflow::write::ArchiveOutcome {
            name: NAME.to_owned(),
            archived_date: "2026-02-02".to_owned(),
        }])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let vcs = Arc::new(FakeVcs::clean().with_dirty(true));
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::clone(&vcs),
        Arc::clone(&snapshot),
        &control,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    assert_eq!(
        task.await.expect("run 任务正常结束"),
        ChangeRunStatus::Completed
    );

    let path_sets = vcs.dirty_path_sets();
    let finalize_paths = path_sets.last().expect("Finalize 脏探测应在案");
    assert!(
        finalize_paths
            .iter()
            .any(|path| path.ends_with(&format!("changes/archive/2026-02-02-{NAME}"))),
        "Finalize pathspec 携 Seal 产出的归档日期目录: {finalize_paths:?}"
    );
    assert_eq!(
        vcs.call_count("commit_paths:"),
        1,
        "脏 → Finalize 落盘提交: {:?}",
        vcs.calls()
    );
    // Seal 命令载荷携 change id（写面定位键）
    let seal_payloads: Vec<String> = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter_map(|command| match command {
            ToolCommand::ArchiveSeal { change_id } => Some(change_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(seal_payloads, vec![CHANGE_ID.to_owned()]);
    let command = history.finish().expect("收口落包应在案");
    assert_eq!(
        node_detail(&command, workflow::state::RunStepKind::Finalize).as_deref(),
        Some(format!("openspec/changes/archive/2026-02-02-{NAME}")).as_deref(),
        "Finalize 步 detail 携归档落盘路径（Seal 产出 + name）"
    );
}

/// 归档会话失败即停链（异常）：spec 同步会话 Err → 步失败 + run Failed（零归档
/// 变更，重试自本步续走；AC-8）。
#[tokio::test]
async fn 归档会话失败即停链() {
    let root = TempRoot::new("archive-spec-sync-fail");
    root.change(NAME);
    let worktree = worktree_dir(&root);
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_archive_reports(vec![Err("spec 同步会话失败".to_owned())])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome()])
        .with_archive_preflight(archive_preflight_twice(
            &worktree,
            vec!["desktop-flow".to_owned()],
        ))
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    assert_eq!(
        task.await.expect("run 任务正常结束"),
        ChangeRunStatus::Failed,
        "归档会话失败 → 步失败停链"
    );
    let command = history.finish().expect("收口落包应在案");
    let failed = command
        .nodes
        .iter()
        .find(|node| node.status == RunStepStatus::Failed)
        .expect("失败节点在案");
    assert!(
        failed
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("spec 同步会话失败")),
        "失败节点携会话记因: {failed:?}"
    );
}

/// 冲突解算续走失败（异常）：解算已验通过而 `rebase_continue` Err → 步失败携
/// 人工续走引导（MUST NOT 自动 abort 已验成果，AC-8）。
#[tokio::test]
async fn 归档冲突解算续走失败携引导() {
    let root = TempRoot::new("archive-continue-fail");
    root.change(NAME);
    let worktree = worktree_dir(&root);
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome()])
        .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let vcs = Arc::new(
        FakeVcs::clean()
            .with_rebase(Ok(RebaseOutcome::Conflicted(vec![
                "specs/a.md".to_owned()
            ])))
            .with_rebase_continue(Err("rebase --continue 失败".to_owned()))
            .with_snapshot_head("head-b", Some("onto-commit")),
    );
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::clone(&vcs),
        Arc::clone(&snapshot),
        &control,
        true,
        &root.root_str(),
        &root.root_str(),
        Some("archive"),
    );
    let _confirmer = spawn_confirmer(&control, true);
    assert_eq!(
        task.await.expect("run 任务正常结束"),
        ChangeRunStatus::Failed,
        "续走失败 → 步失败停链"
    );
    assert_eq!(vcs.call_count("rebase_abort"), 0, "已验成果不自动回退");
    let command = history.finish().expect("收口落包应在案");
    let failed = command
        .nodes
        .iter()
        .find(|node| node.status == RunStepStatus::Failed)
        .expect("失败节点在案");
    assert!(
        failed
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("rebase --continue 失败")),
        "失败节点携续走失败记因: {failed:?}"
    );
}

/// 归档落盘 / 快进 vcs 失败（异常）：`ff_merge` Err / `commit_paths` Err → 步失败
/// 停链（AC-8）。
#[tokio::test]
async fn 归档快进与落盘失败即停链() {
    let cases: [(&str, FakeVcs); 2] = [
        (
            "主仓快进失败：非快进可合并",
            FakeVcs::clean().with_ff_merge_result(Err("主仓快进失败：非快进可合并".to_owned())),
        ),
        (
            "落盘提交失败",
            FakeVcs::clean()
                .with_dirty(true)
                .with_commit_paths_result(Err("落盘提交失败".to_owned())),
        ),
    ];
    for (expect, vcs) in cases {
        let root = TempRoot::new("archive-vcs-fail");
        root.change(NAME);
        let worktree = worktree_dir(&root);
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![archive_route_outcome()])
            .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run_with_vcs(
            worker,
            tools,
            Arc::new(vcs),
            Arc::clone(&snapshot),
            &control,
            true,
            &root.root_str(),
            &root.root_str(),
            Some("archive"),
        );
        let _confirmer = spawn_confirmer(&control, true);
        assert_eq!(
            task.await.expect("run 任务正常结束"),
            ChangeRunStatus::Failed,
            "{expect} → 步失败停链"
        );
        let command = history.finish().expect("收口落包应在案");
        let failed = command
            .nodes
            .iter()
            .find(|node| node.status == RunStepStatus::Failed)
            .expect("失败节点在案");
        assert!(
            failed
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains(expect)),
            "失败节点记因: {failed:?}"
        );
    }
}

/// 归档载体停等后停止（边界）：停止信号先于确认置位 → 归档相位零执行并以
/// stopped 收敛（非 failed），零失败节点（AC-7 / AC-8 分界）。
#[tokio::test]
async fn 归档相位中途停止收敛stopped() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        Arc::new(FakeVcs::clean()),
        Arc::clone(&snapshot),
        &control,
        true,
        "/tmp/root",
        "/tmp/root",
        Some("archive"),
    );
    assert!(control.request_stop(ROOT, CHANGE_ID), "停止置位");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Stopped, "停止 → 受控终止（非 failed）");
    let command = history.finish().expect("收口落包应在案");
    assert!(
        command
            .nodes
            .iter()
            .all(|node| node.status != RunStepStatus::Failed),
        "停止不收尾为失败节点: {:?}",
        node_rows(&command)
    );
    assert!(
        commands.lock().expect("命令锁").is_empty(),
        "停止先于确认置位 → 归档相位零工具命令"
    );
}
