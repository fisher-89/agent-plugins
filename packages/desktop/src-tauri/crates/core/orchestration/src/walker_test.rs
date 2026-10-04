use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent::AgentRunStatus;

use crate::control::ChangeFlowControl;
use crate::port::{
    BoxDiffFuture, BoxToolFuture, BoxTurnFuture, DiffContextPort, StaticCheckOutcome,
    StaticCheckRunner, ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest, WorkerAgentPort,
    WorkerRole, WorkerTurnOutcome, WorkerTurnRequest, WorkflowSnapshotPort,
};
use crate::snapshot::FsSnapshot;
use crate::state::{ChangeRunStatus, RunUpdate};
use crate::steps::LocalToolSteps;
use crate::walker::{
    new_run_id, walk_run, RunRequest, STATIC_CHECK_FEEDBACK_LIMIT, STATIC_CHECK_PHASES,
};
use workflow::model::Verdict;
use workflow::write::{
    BacktrackOutcome, DecisionLogOutcome, LastResult, PhaseLogOutcome, PhaseNextError,
    PhaseNextOutcome, PhaseStartOutcome, SessionAnchors,
};

// ---------------------------------------------------------------------------
// 装置：tempdir fixture（真实写面组合用例）
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

    fn change(&self, name: &str, workflow_json: &str) {
        let dir = self.0.join("openspec/changes").join(name);
        fs_create(&dir);
        std::fs::write(dir.join("workflow.json"), workflow_json).expect("写 workflow.json 失败");
    }

    fn workflow_json(&self, name: &str) -> serde_json::Value {
        let text = std::fs::read_to_string(
            self.0
                .join("openspec/changes")
                .join(name)
                .join("workflow.json"),
        )
        .expect("读 workflow.json 失败");
        serde_json::from_str(&text).expect("workflow.json 应可解析")
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
            counter: AtomicUsize::new(0),
            gate: None,
        }
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
        // 停等门仅挂 evaluator（停止时序窗口；executor / decision 不挂）
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

        Box::pin(async move {
            // evaluator 停等门：armed 期间挂起（停止时序窗口；notify_one 带许可
            // 语义——放行通知早于 future 首轮 poll 亦不丢）
            if let Some(gate) = gate {
                if *gate.armed.lock().expect("门锁不可中毒") {
                    gate.notify.notified().await;
                }
            }
            let _ = &mut timeline;
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
    phase_next: Mutex<VecDeque<PhaseNextOutcome>>,
    static_check: Mutex<VecDeque<StaticCheckOutcome>>,
    /// 命中即 Err 的步标签（phase-next / phase-start / phase-log / backtrack /
    /// decision-log / static-check）
    fail_on: Mutex<Option<String>>,
    attempt_counter: AtomicUsize,
}

impl FakeTools {
    fn new(timeline: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            timeline: Arc::clone(timeline),
            commands: Arc::new(Mutex::new(Vec::new())),
            phase_next: Mutex::new(VecDeque::new()),
            static_check: Mutex::new(VecDeque::new()),
            fail_on: Mutex::new(None),
            attempt_counter: AtomicUsize::new(0),
        }
    }

    fn with_phase_next(mut self, outcomes: Vec<PhaseNextOutcome>) -> Self {
        self.phase_next = Mutex::new(outcomes.into());
        self
    }

    fn with_static_check(mut self, outcomes: Vec<StaticCheckOutcome>) -> Self {
        self.static_check = Mutex::new(outcomes.into());
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
        ToolCommand::StaticCheck => "static-check",
    }
}

impl ToolStepPort for FakeTools {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture {
        let label = command_label(&step.command);
        self.commands
            .lock()
            .expect("命令捕获锁不可中毒")
            .push(step.command.clone());
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
                    start_at: time::OffsetDateTime::now_utc(),
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
            ToolCommand::StaticCheck => {
                let outcome = self
                    .static_check
                    .lock()
                    .expect("static-check 队列锁不可中毒")
                    .pop_front()
                    .unwrap_or(StaticCheckOutcome {
                        passed: true,
                        diagnostics: String::new(),
                    });
                Box::pin(async move { Ok(ToolStepOutput::StaticCheck(outcome)) })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 装置：DiffContextPort 假实现（每轮取新 / attempt 递进 / 可编程 Err）
// ---------------------------------------------------------------------------

struct FakeDiff {
    roots: Arc<Mutex<Vec<String>>>,
    texts: Mutex<VecDeque<String>>,
    fail: bool,
}

impl FakeDiff {
    fn new(texts: Vec<&str>) -> Self {
        Self {
            roots: Arc::new(Mutex::new(Vec::new())),
            texts: Mutex::new(
                texts
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<VecDeque<_>>(),
            ),
            fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            roots: Arc::new(Mutex::new(Vec::new())),
            texts: Mutex::new(VecDeque::new()),
            fail: true,
        }
    }

    fn assemble(self) -> (Arc<dyn DiffContextPort>, Arc<Mutex<Vec<String>>>) {
        let roots = Arc::clone(&self.roots);
        (Arc::new(self), roots)
    }
}

impl DiffContextPort for FakeDiff {
    fn diff_context(&self, root: &str) -> BoxDiffFuture {
        self.roots
            .lock()
            .expect("diff root 锁不可中毒")
            .push(root.to_owned());
        let text = self.texts.lock().expect("diff 队列锁不可中毒").pop_front();
        let fail = self.fail;
        Box::pin(async move {
            if fail {
                return Err("git 缺失".to_owned());
            }
            Ok(text.unwrap_or_else(|| "canned diff".to_owned()))
        })
    }
}

// ---------------------------------------------------------------------------
// 装置：预录产出构造器与 run 驱动
// ---------------------------------------------------------------------------

fn route_outcome(phase: &str, allowed: &[&str]) -> PhaseNextOutcome {
    route_with_round(phase, allowed, 1, None)
}

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
        executor: Some(workflow::write::PhaseAgentSpec {
            agent_type: "__CALL_AGENT:implementation-generator__".to_owned(),
            prompt: format!("Implement the code for change \"c\" ({phase})."),
        }),
        evaluator: Some(workflow::write::PhaseAgentSpec {
            agent_type: "__CALL_AGENT:implementation-evaluator__".to_owned(),
            prompt: format!("Evaluate {phase} phase for change \"c\"."),
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
    }
}

fn failing_check(diagnostics: &str) -> StaticCheckOutcome {
    StaticCheckOutcome {
        passed: false,
        diagnostics: diagnostics.to_owned(),
    }
}

const CHANGE: &str = "walker-change";

/// 决策分叉族 fixture（三 entry：proposal pass / dev-design pass / implement
/// fail 带 fail checklist）——真实 FsSnapshot 的决策输入来源（AC-2 有界输入）。
const DECISION_FIXTURE: &str = r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "dev-design", "attempt": 1, "verdict": "pass", "report": "设计齐备", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" },
    { "phase": "implement", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [
      { "item": "写面保形", "pass": false, "evidence": "custom_note 丢失" }
    ], "timestamp": "2026-10-01T08:20:00Z" }
  ]
}"#;

/// run 驱动
fn spawn_run(
    worker: Arc<dyn WorkerAgentPort>,
    tools: Arc<dyn ToolStepPort>,
    diff: Arc<dyn DiffContextPort>,
    snapshot: Arc<dyn WorkflowSnapshotPort>,
    control: &Arc<ChangeFlowControl>,
    auto_next_phase: bool,
    root: &str,
) -> (
    tokio::task::JoinHandle<ChangeRunStatus>,
    tokio::sync::broadcast::Receiver<RunUpdate>,
) {
    let guard = control
        .begin_run(CHANGE, "run-1".to_owned())
        .expect("登记 run 应成功");
    let rx = control.subscribe(CHANGE).expect("run 订阅应成功");
    let request = RunRequest {
        root: root.to_owned(),
        change: CHANGE.to_owned(),
        run_id: "run-1".to_owned(),
        auto_next_phase,
    };
    let task = tokio::spawn(walk_run(
        worker,
        tools,
        diff,
        snapshot,
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
    let control = Arc::clone(control);
    let mut rx = control.subscribe(CHANGE).expect("run 订阅应成功");
    tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            if matches!(update, RunUpdate::ConfirmWait { .. }) {
                let _ = control.confirm(CHANGE, proceed);
            }
        }
    })
}

/// 摘取 Step 更新的 (phase, attempt, kind 线格式, status 线格式) 列表。
fn step_rows(updates: &[RunUpdate]) -> Vec<(String, u32, String, String)> {
    updates
        .iter()
        .filter_map(|update| match update {
            RunUpdate::Step { step } => {
                let wire = serde_json::to_value(step).expect("步状态出线");
                Some((
                    step.phase.clone(),
                    step.attempt,
                    wire["step"].as_str().expect("step 串").to_owned(),
                    wire["status"].as_str().expect("status 串").to_owned(),
                ))
            }
            _ => None,
        })
        .collect()
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
                change,
                phase,
                session_id,
            } => Some((change.clone(), phase.clone(), session_id.clone())),
            _ => None,
        })
        .collect()
}

/// 信封出线（RunUpdate 无 PartialEq，等值经线面 JSON 比对）。
fn wire(update: &RunUpdate) -> serde_json::Value {
    serde_json::to_value(update).expect("RunUpdate 出线应成功")
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
    let (diff, _) = FakeDiff::new(vec!["canned diff"]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "全 pass 收敛 completed");

    // PhaseStart 载荷逐条对齐（change 名 + 相位序）
    let starts: Vec<(String, String)> = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter_map(|command| match command {
            ToolCommand::PhaseStart { change, phase } => Some((change.clone(), phase.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts,
        vec![
            (CHANGE.to_owned(), "proposal".to_owned()),
            (CHANGE.to_owned(), "dev-design".to_owned()),
        ],
        "PhaseStart 载荷与相位推进序逐条对齐"
    );

    // PhaseLog 载荷逐条对齐（桌面代写落账，checklist 随 evaluator 产出）
    let logs: Vec<(String, String, bool)> = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter_map(|command| match command {
            ToolCommand::PhaseLog { change, input, .. } => Some((
                change.clone(),
                input.phase.clone(),
                input.checklist.iter().all(|item| item.pass),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        logs,
        vec![
            (CHANGE.to_owned(), "proposal".to_owned(), true),
            (CHANGE.to_owned(), "dev-design".to_owned(), true),
        ],
        "PhaseLog 载荷（change / phase / 全 pass checklist）逐条对齐"
    );
}

/// walk_run 真实写面组合演进对照（边界）：walker + 假 WorkerAgentPort + 真实
/// LocalToolSteps + tempdir 真盘 fixture——全程推进后 workflow.json 的
/// active_phase / eval 演进与插件直跑形态对照一致（AC-1 尾句 + AC-9 对照口径
/// 的循环级承载；转移判定恒问写面 phase-next——9 次路由调用的权威面）。
#[tokio::test]
async fn 真实写面组合全程演进对照一致() {
    let root = TempRoot::new("real-compose");
    root.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    // 真实 LocalToolSteps：命令捕获无独立缝——经 workflow.json 演进断言承载
    //（进程内缝真实组合，最小 mock）
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
    ));
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(
        worker,
        steps,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "全相位 pass 收敛 completed"
    );

    // eval 演进与插件直跑形态对照一致：8 相位各 1 条 pass（attempt 1、表序）
    let doc = root.workflow_json(CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(eval.len(), 8, "八相位各落账一条");
    let phases: Vec<&str> = eval
        .iter()
        .map(|entry| entry["phase"].as_str().expect("phase 串"))
        .collect();
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
            "acceptance"
        ],
        "落账表序与插件相位表一致"
    );
    for entry in eval {
        assert_eq!(entry["verdict"], serde_json::json!("pass"));
        assert_eq!(entry["attempt"], serde_json::json!(1));
        assert!(
            entry["report"].is_string() && entry["checklist"].is_array(),
            "条目形状与插件 buildEntry 同形: {entry}"
        );
        assert!(
            entry.get("skipped").is_none() && entry.get("stale").is_none(),
            "干净 run 无 skipped / stale 扩展字段"
        );
    }

    // active_phase 演进：逐相位开跑、落账清除 → 终态 null
    assert!(
        doc.get("active_phase").map_or(true, |v| v.is_null()),
        "run 收口后 active_phase 清除"
    );
    // file_log 零触碰（AC-3 写面半边）：全程零新增
    assert!(
        doc.get("file_log").is_none(),
        "desktop run 全程 file_log 零新增"
    );
}

/// walk_run 重入自 active_phase 续走（AC-7 续走半边）：预置中段 fixture
///（前序相位已落 pass、active_phase 指向中段）——发起 run 首次 phase-next 即
/// 解析到 active_phase、已 pass 相位零 phase-start / executor 调用；中断相位
/// 承接重试（attempt 2）而非新开回合。
#[tokio::test]
async fn 重入自active_phase续走不重跑已pass相位() {
    let root = TempRoot::new("resume");
    // 中段 fixture：proposal 已 pass、test-design 及其后已 pass、dev-design
    // 中断残留（1 条 fail + active_phase attempt 2）
    root.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "中断前未过", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" },
    { "phase": "test-design", "attempt": 1, "verdict": "pass", "report": "测试设计通过", "checklist": [], "timestamp": "2026-10-01T08:20:00Z" },
    { "phase": "implement", "attempt": 1, "verdict": "pass", "report": "实现通过", "checklist": [], "timestamp": "2026-10-01T08:30:00Z" },
    { "phase": "test-gen", "attempt": 1, "verdict": "pass", "report": "测试通过", "checklist": [], "timestamp": "2026-10-01T08:40:00Z" },
    { "phase": "test-execution", "attempt": 1, "verdict": "pass", "report": "执行通过", "checklist": [], "timestamp": "2026-10-01T08:50:00Z" },
    { "phase": "code-review", "attempt": 1, "verdict": "pass", "report": "审查通过", "checklist": [], "timestamp": "2026-10-01T09:00:00Z" },
    { "phase": "acceptance", "attempt": 1, "verdict": "pass", "report": "验收通过", "checklist": [], "timestamp": "2026-10-01T09:10:00Z" }
  ],
  "active_phase": { "phase": "dev-design", "attempt": 2, "start_at": "2026-10-01T08:15:00Z" }
}"#,
    );

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
    ));
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(
        worker,
        steps,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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
        Some("walker-change/dev-design/executor/2"),
        "中断相位承接重试（attempt 2 = 既有条目 1 + 1），非新开回合"
    );
    assert_eq!(
        requests[1].provenance.source_ref.as_deref(),
        Some("walker-change/dev-design/evaluator/2")
    );
    drop(requests);

    // 落盘演进：dev-design 追加 pass 条目（attempt 2）、active_phase 清除
    let doc = root.workflow_json(CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(eval.len(), 9, "dev-design 重评条目纯追加（既有 8 + 1）");
    assert_eq!(eval[8]["phase"], serde_json::json!("dev-design"));
    assert_eq!(eval[8]["attempt"], serde_json::json!(2));
    assert_eq!(eval[8]["verdict"], serde_json::json!("pass"));
    assert!(doc.get("active_phase").map_or(true, |v| v.is_null()));
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    // attempt 递增重跑：executor / evaluator 各两轮，sourceRef attempt 1 → 2
    let requests = requests.lock().expect("请求锁");
    assert_eq!(requests.len(), 4, "executor / evaluator 各重跑一轮");
    assert_eq!(
        requests[0].provenance.source_ref.as_deref(),
        Some("walker-change/implement/executor/1")
    );
    assert_eq!(
        requests[2].provenance.source_ref.as_deref(),
        Some("walker-change/implement/executor/2"),
        "attempt 递增（provenance 定式随行）"
    );
}

/// walk_run 决策分叉唤起有界输入：phase-next 返回 MaxRetriesExceeded → 决策
/// agent 恰被唤起一次，WorkerTurnRequest 捕获断言输入有界（fail checklist +
/// 写面下发白名单 + 候选相位 eval report 取自 snapshot detail——AC-2 唤起半边）。
#[tokio::test]
async fn 决策分叉唤起恰一次且输入有界() {
    let root = TempRoot::new("decision-input");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    // 真实快照源：决策输入取自 ChangeDetail 只读装配（AC-2 有界输入来源）
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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
    let root = TempRoot::new("backtrack-decision");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    // 真实快照源：决策会话的 detail 读取（决策输入面）
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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
        ToolCommand::Backtrack { change, input, .. } => {
            assert_eq!(change, CHANGE);
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
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
    let root = TempRoot::new("overreach-decision");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    // 真实快照源：决策会话的 detail 读取（越权决议走完快照面后由白名单门拦下）
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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

    // 失败原因经 Finished 流出
    let mut finished = None;
    while let Ok(update) = rx.try_recv() {
        if let RunUpdate::Finished { .. } = &update {
            finished = Some(update);
        }
    }
    let finished = finished.expect("Finished 信封流出");
    let value = wire(&finished);
    assert_eq!(value["status"], serde_json::json!("failed"));
    assert!(
        value["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("越权"),
        "失败记因流出: {value}"
    );
}

/// walk_run ask 中断与应答回流（正向）：决议 ask → RunUpdate::Ask 流出 +
/// wait_answer 挂起；answer 回流后以应答文本 Continue 决策会话重出封闭集
///（AC-2 ask 半边）。
#[tokio::test]
async fn ask中断与应答回流continue决策会话() {
    let root = TempRoot::new("ask-decision");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    // 真实快照源：决策会话的 detail 读取（ask 挂起前的输入面）
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
    let _confirmer = spawn_confirmer(&control, true);

    // Ask 信封流出（UI 中断问题与选项；current_thread 运行时以 try_recv 轮询，
    // 30s 上限防挂死）
    let deadline = Instant::now() + Duration::from_secs(30);
    let ask = loop {
        if let Some(ask) = futures_poll(&mut rx) {
            break ask;
        }
        assert!(Instant::now() < deadline, "等待 Ask 信封超时（30s）");
        tokio::task::yield_now().await;
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(
        wire(&ask),
        serde_json::json!({
            "ipc": "ask",
            "question": "回溯目标选哪个?",
            "options": ["proposal", "dev-design"]
        }),
        "Ask 信封（question + options）流出"
    );

    // 应答回流：walker 以应答文本 Continue 决策会话重出封闭集
    control
        .answer(CHANGE, "采用方案 A".to_owned())
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "proceed=true 继续推进直至 done 收敛"
    );

    // ConfirmWait 信封逐相位流出（phase 载荷随行、推进间各一条）
    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }
    let confirms: Vec<serde_json::Value> = updates
        .iter()
        .filter(|update| wire(update)["ipc"] == serde_json::json!("confirmWait"))
        .map(wire)
        .collect();
    assert_eq!(
        confirms,
        vec![
            serde_json::json!({ "ipc": "confirmWait", "phase": "proposal" }),
            serde_json::json!({ "ipc": "confirmWait", "phase": "dev-design" }),
        ],
        "相位推进间 ConfirmWait 逐相位流出（确认卡片载荷面）"
    );
    // 确认后 run 全程仍可观测：步状态与终态照常流出
    assert!(updates
        .iter()
        .any(|update| matches!(update, RunUpdate::Step { .. })));
    assert!(updates
        .iter()
        .any(|update| matches!(update, RunUpdate::Finished { .. })));
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
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
    let mut finished = None;
    while let Ok(update) = rx.try_recv() {
        if let RunUpdate::Finished { .. } = &update {
            finished = Some(update);
        }
    }
    let value = wire(&finished.expect("Finished 信封流出"));
    assert_eq!(value["status"], serde_json::json!("stopped"));
    assert!(
        value["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("proposal"),
        "否决记因携停等相位: {value}"
    );
    assert!(control.snapshot(CHANGE).is_none(), "终态除名");
}

/// walk_run 停止收敛（异常）：request_stop 置位 → 当前会话收口后终态
/// stopped、不再发起新相位 / 新会话（停止寻址键 = change 名，经
/// RunGuard.cancelled 观测——AC-7 停止半边）。
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);

    // evaluator 会话挂起（gate armed）即运行中窗口：停止寻址键 = change 名
    wait_for("evaluator 会话发起", || {
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .any(|request| request.role == WorkerRole::Evaluator)
    })
    .await;
    assert!(control.request_stop(CHANGE), "运行中置位返回 true");

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

    // Finished 信封流出 + 注册表除名（run 仅进程内）
    let mut finished = None;
    while let Ok(update) = rx.try_recv() {
        if let RunUpdate::Finished { .. } = &update {
            finished = Some(update);
        }
    }
    let value = wire(&finished.expect("Finished 信封流出"));
    assert_eq!(value["status"], serde_json::json!("stopped"));
    assert!(control.snapshot(CHANGE).is_none(), "终态除名");
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(worker, tools, diff, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "auto 直通收敛 completed"
    );

    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }
    assert!(
        updates
            .iter()
            .all(|update| wire(update)["ipc"] != serde_json::json!("confirmWait")),
        "更新流零 ConfirmWait 信封（快照面全程不经 waitingConfirm）"
    );

    // 逐相位 phase-start 在场（确经逐相位推进、非一次收敛）
    let starts: Vec<String> = step_rows(&updates)
        .into_iter()
        .filter(|(_, _, kind, row_status)| kind == "phaseStart" && row_status == "passed")
        .map(|(phase, _, _, _)| phase)
        .collect();
    assert_eq!(
        starts,
        vec!["proposal".to_owned(), "dev-design".to_owned()],
        "逐相位 phase-start 推进（两相位各开跑一次）"
    );

    // 终态口径：completed + 全相位通过 reason（不随 auto 分支漂移）
    let finished = updates
        .iter()
        .find(|update| matches!(update, RunUpdate::Finished { .. }))
        .expect("Finished 信封流出");
    let value = wire(finished);
    assert_eq!(value["status"], serde_json::json!("completed"));
    assert_eq!(
        value["reason"],
        serde_json::json!("All phases have passed evaluation. Ready for archiving."),
        "终态收口文案不随 auto 分支漂移"
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    // 关键：零 spawn_confirmer 接线——手动档同 fixture 在此必停等挂起
    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, true, "/tmp/root");
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(worker, tools, diff, snapshot, &control, true, "/tmp/root");
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

    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }
    assert!(
        updates
            .iter()
            .all(|update| wire(update)["ipc"] != serde_json::json!("confirmWait")),
        "失败路径更新流同样零 ConfirmWait 信封"
    );
    let finished = updates
        .iter()
        .find(|update| matches!(update, RunUpdate::Finished { .. }))
        .expect("Finished 信封流出");
    let value = wire(finished);
    assert_eq!(value["status"], serde_json::json!("failed"));
    assert!(
        value["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("phase-start 失败"),
        "原因经 Finished 流出: {value}"
    );
}

#[tokio::test]
async fn auto确认ask照常停等应答回流后收敛全程零confirmwait() {
    let root = TempRoot::new("auto-ask");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    // 真实快照源：决策会话的 detail 读取真实组合（tempdir 真盘 DECISION_FIXTURE）
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        true,
        &root.root_str(),
    );

    // Ask 信封流出（更新流全量留档收集——零 ConfirmWait 断言面一并覆盖；
    // current_thread 运行时以 try_recv 轮询，30s 上限防挂死）
    let mut updates: Vec<RunUpdate> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    let ask = loop {
        match rx.try_recv() {
            Ok(update) => {
                let hit = matches!(update, RunUpdate::Ask { .. });
                updates.push(update);
                if hit {
                    break updates.pop().expect("Ask 信封在收集队尾");
                }
            }
            Err(_) => {
                assert!(Instant::now() < deadline, "等待 Ask 信封超时（30s）");
                tokio::task::yield_now().await;
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    };
    assert_eq!(
        wire(&ask),
        serde_json::json!({
            "ipc": "ask",
            "question": "回溯目标选哪个?",
            "options": ["proposal", "dev-design"]
        }),
        "auto 模式 ask 中断照常流出（不自动选择）"
    );

    // 应答回流：Continue 决策会话重出封闭集 → retry → done 收敛
    control
        .answer(CHANGE, "采用方案 A".to_owned())
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
    assert!(
        updates
            .iter()
            .all(|update| wire(update)["ipc"] != serde_json::json!("confirmWait")),
        "auto 模式全程零 ConfirmWait 信封"
    );
    assert!(
        updates
            .iter()
            .any(|update| matches!(update, RunUpdate::Finished { .. })),
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
// AC-4：static-check 门控 / 反馈边 / 升格；AC-3 diff；AC-5 步状态流出
// ---------------------------------------------------------------------------

/// walk_run static-check 步门控：implement 站（STATIC_CHECK_PHASES 命中）
/// executor 收口后 static-check 必经；非门控相位零 static-check 调用（AC-4
/// 必经半边）。
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let static_checks = commands
        .lock()
        .expect("命令锁")
        .iter()
        .filter(|command| matches!(command, ToolCommand::StaticCheck))
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
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
}

/// walk_run 反馈边独立计数与超限升格（边界）：反馈循环恰 ≤5 次（
/// STATIC_CHECK_FEEDBACK_LIMIT）且不消耗相位 retry 预算；第 5 次仍失败 →
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
            (STATIC_CHECK_FEEDBACK_LIMIT + 1) as usize
        ])
        .assemble();
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
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
        .filter(|command| matches!(command, ToolCommand::StaticCheck))
        .count();
    assert_eq!(
        static_checks,
        STATIC_CHECK_FEEDBACK_LIMIT as usize + 1,
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
            prompt.contains(&format!("第 {}/{}", idx + 1, STATIC_CHECK_FEEDBACK_LIMIT)),
            "修复 prompt 反馈计数递进: {prompt}"
        );
    }
}

/// walk_run diff 上下文每轮取新（正向）：每次 WorkerAgent prompt 组装前恰一次
/// diff_context 调用（executor / evaluator 双入口）；attempt 递进可见（第 2
/// attempt 收到更新的 canned diff——W5/AC-3）。
#[tokio::test]
async fn diff上下文每轮取新且attempt递进可见() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(fail_json("首轮未过")), Ok(PASS_JSON.to_owned())])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_with_round("implement", &["implement"], 1, None),
            route_with_round("implement", &["implement"], 2, None),
            done_outcome(),
        ])
        .assemble();
    let (diff, diff_roots) = FakeDiff::new(vec![
        "第一轮 diff A",
        "第一轮 diff B",
        "更新 diff A",
        "更新 diff B",
    ])
    .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    // 双入口各恰一次（attempt 1 executor/evaluator + attempt 2 executor/evaluator）
    assert_eq!(
        diff_roots.lock().expect("锁").len(),
        4,
        "每轮组装前恰一次取新"
    );

    let requests = requests.lock().expect("请求锁");
    let executor_one = &requests[0];
    let evaluator_one = &requests[1];
    let executor_two = &requests[2];
    assert!(
        executor_one.prompt.contains("第一轮 diff A"),
        "第 1 attempt executor 收到首轮 canned diff"
    );
    assert!(
        evaluator_one.prompt.contains("第一轮 diff B"),
        "同轮 evaluator 组装前另行取新（携带当轮 diff 段）"
    );
    assert!(
        executor_two.prompt.contains("更新 diff A"),
        "第 2 attempt 收到更新的 canned diff（attempt 递进可见）"
    );
    assert!(
        !executor_two.prompt.contains("第一轮"),
        "不复用旧 diff（每轮取新）"
    );
}

/// walk_run diff 源 Err 降级不阻断（异常）：diff_context 返回 Err → prompt
/// diff 段降级为错误提示、run 不阻断（运行时依赖节「git 缺失降级」——AC-3）。
#[tokio::test]
async fn diff源err降级不阻断run() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();
    let (diff, _) = FakeDiff::failing().assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "git 缺失降级不阻断 run");

    let requests = requests.lock().expect("请求锁");
    for request in requests.iter() {
        assert!(
            request
                .prompt
                .contains("（git diff 上下文不可用: git 缺失）"),
            "prompt diff 段降级为错误提示（显式可读）: {}",
            request.prompt
        );
    }
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }
    let rows = step_rows(&updates);

    // 三类节点步词汇齐备且可辨
    let kinds: Vec<&str> = rows.iter().map(|(_, _, kind, _)| kind.as_str()).collect();
    for expected in [
        "phaseStart",
        "executor",
        "staticCheck",
        "evaluator",
        "verdictGate",
        "phaseLog",
    ] {
        assert!(kinds.contains(&expected), "步词汇 {expected} 经状态流出");
    }
    // Gate 三门中 verdictGate 在场；WorkerAgent / ToolStep / Gate 三类可辨
    assert!(kinds.contains(&"executor"), "WorkerAgent 类（executor）");
    assert!(kinds.contains(&"staticCheck"), "ToolStep 类（staticCheck）");
    assert!(kinds.contains(&"verdictGate"), "Gate 类（verdictGate）");

    // phase / attempt 逐档对齐 + running→passed 状态推进（phase-start 的
    // running 行以 attempt 0 先行流出，outcome 到达后以实值收口）
    for (phase, attempt, _, status) in &rows {
        assert_eq!(phase, "implement", "步状态行 phase 逐档");
        assert!([0, 1].contains(attempt), "attempt 逐档: {attempt}");
        assert!(
            status == "running" || status == "passed",
            "步状态四档可辨: {status}"
        );
    }
    // 每一步均 running 起手、passed 收口（phaseStart 缺失 attempt 0 行为
    // emit(0) 后以 outcome.attempt 收口——两类行均携带同一步词汇）
    let step_at = |kind: &str, status: &str| {
        rows.iter()
            .any(|(_, _, row_kind, row_status)| row_kind == kind && row_status == status)
    };
    for kind in [
        "phaseStart",
        "executor",
        "staticCheck",
        "evaluator",
        "verdictGate",
        "phaseLog",
    ] {
        assert!(step_at(kind, "running"), "{kind} running 起手");
        assert!(step_at(kind, "passed"), "{kind} passed 收口");
    }
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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(worker, tools, diff, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Failed, "工具步失败显式 failed");
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "失败先行（phase-start 失败零会话发起）"
    );

    let mut finished = None;
    while let Ok(update) = rx.try_recv() {
        if let RunUpdate::Finished { .. } = &update {
            finished = Some(update);
        }
    }
    let value = wire(&finished.expect("Finished 信封流出"));
    assert_eq!(value["status"], serde_json::json!("failed"));
    assert!(
        value["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("phase-start 失败"),
        "原因经 Finished 流出: {value}"
    );
}

/// walk_run 空白名单 backtrack 出口（边界）：phase-next 白名单为空 +
/// MaxRetriesExceeded → 决策输入白名单段为空，backtrack 决议必被预校验拒绝
///（仅剩 retry / stop / ask 出口——AC-2）。
#[tokio::test]
async fn 空白名单backtrack决议被拒() {
    let root = TempRoot::new("empty-whitelist-decision");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    // 真实快照源：决策会话的 detail 读取（空白名单出口的决策输入面）
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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

/// 步门控常量锚定（边界）：STATIC_CHECK_FEEDBACK_LIMIT == 5（沿 hook
/// loop_limit 语义）；STATIC_CHECK_PHASES == ["implement", "test-gen"]（布局
/// 身份常量——转移判定恒问写面 phase-next，不据其路由，红线锚）。
#[test]
fn 步门控常量锚定() {
    assert_eq!(STATIC_CHECK_FEEDBACK_LIMIT, 5);
    assert_eq!(STATIC_CHECK_PHASES, ["implement", "test-gen"]);
    // run_id 铸造定式（每 run 一个会话窗口标识）
    let run_id = new_run_id();
    assert!(run_id.starts_with("run-"), "run-<millis> 定式: {run_id}");
}

// ---------------------------------------------------------------------------
// AC-5：会话槽位落账与决策挂账（desktop-change-session-visibility）
// ---------------------------------------------------------------------------

/// walk_run 真实写面组合（LocalToolSteps + TempRoot 真盘）：verdict 条目落账
/// 携 executor + evaluator 双槽位、值自 `WorkerTurnOutcome.session_id` 与假
/// 引擎会话序逐一对应；decision 槽位在 verdict 落账时恒缺省（D5），决策会话
/// 收口后经 decision_log 写面单点定点挂账（AC-5 全链真实组合）。
#[tokio::test]
async fn 真实写面组合_会话槽位落账与决策挂账全链对应() {
    let root = TempRoot::new("slots-real-compose");
    root.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

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
    ));
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(
        worker,
        steps,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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

    // verdict 条目落账：executor / evaluator 槽位与会话序逐一对应（取值自
    // WorkerTurnOutcome.session_id——AC-5）
    let doc = root.workflow_json(CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(eval.len(), 5, "五条 fail 条目纯追加");
    for (idx, entry) in eval.iter().enumerate() {
        assert_eq!(entry["phase"], serde_json::json!("proposal"));
        assert_eq!(entry["verdict"], serde_json::json!("fail"));
        assert_eq!(
            entry["executor_session_id"],
            serde_json::json!(executors[idx]),
            "executor 槽位与会话序逐一对应"
        );
        assert_eq!(
            entry["evaluator_session_id"],
            serde_json::json!(evaluators[idx]),
            "evaluator 槽位随行落账"
        );
    }
    // verdict 落账时 decision 槽位恒缺省（D5）；决策收口后最新条目定点挂账
    for entry in eval.iter().take(4) {
        assert!(
            entry.get("decision_session_id").is_none(),
            "verdict 条目落账无 decision 槽位键"
        );
    }
    assert_eq!(
        eval[4]["decision_session_id"],
        serde_json::json!(decisions[0]),
        "决策会话收口后其 id 记录在案（walk_run → LocalToolSteps → decision_log 写面）"
    );
}

/// 挂账先于决策解析（真实组合链留痕）：决策会话产出漂移文本 → parse 失败路
/// 径 run 显式 failed，但 workflow.json 最新条目已携 decision_session_id——
/// 写挂时机在 parse 之前，parse 失败同样留痕。
#[tokio::test]
async fn 真实写面组合_决策挂账先于解析_parse失败同样留痕() {
    let root = TempRoot::new("relog-before-parse");
    root.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

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
    ));
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(
        worker,
        steps,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "决策解析失败显式 failed");

    // 终态记因为解析失败（非挂账失败——挂账已成功先于 parse）
    let mut finished = None;
    while let Ok(update) = rx.try_recv() {
        if let RunUpdate::Finished { .. } = &update {
            finished = Some(update);
        }
    }
    let value = wire(&finished.expect("Finished 信封流出"));
    assert_eq!(value["status"], serde_json::json!("failed"));
    assert!(
        value["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("决策解析失败"),
        "记因为解析失败（挂账不在失败路径）: {value}"
    );

    // 留痕：最新 fail 条目已携决策会话 id
    let sessions = sessions.lock().expect("会话登记锁");
    let decisions = sessions_of(&sessions, WorkerRole::Decision);
    assert_eq!(decisions.len(), 1);
    drop(sessions);
    let doc = root.workflow_json(CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(eval.len(), 5);
    assert_eq!(
        eval[4]["decision_session_id"],
        serde_json::json!(decisions[0]),
        "parse 失败路径同样留痕"
    );
}

/// static-check 反馈边超限升格（真实组合链）：fail 条目仅携 executor 单键
///（evaluator 未跑无会话可记）；反馈修复轮 Continue 同 executor 会话——槽位
/// 值续注不漂移；决策 stop 收口后最新 fail 条目携 decision 挂账键。前置三相
/// 位预置 pass（fixture 让 implement 站先行——升格门控相位）。
#[tokio::test]
async fn 真实写面组合_升格fail条目仅携executor槽位且修复轮续注同会话() {
    let root = TempRoot::new("upgrade-slots");
    root.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
    { "phase": "dev-design", "attempt": 1, "verdict": "pass", "report": "设计通过", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" },
    { "phase": "test-design", "attempt": 1, "verdict": "pass", "report": "测试设计通过", "checklist": [], "timestamp": "2026-10-01T08:20:00Z" }
  ]
}"#,
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
    ));
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(
        worker,
        steps,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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

    // fail 条目仅携 executor 单键（evaluator 未跑）；最新条目携决策挂账键
    let doc = root.workflow_json(CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(eval.len(), 8, "预置三条 pass + 五轮升格 fail 纯追加");
    let upgrades = &eval[3..];
    for (idx, entry) in upgrades.iter().enumerate() {
        assert_eq!(entry["phase"], serde_json::json!("implement"));
        assert_eq!(entry["verdict"], serde_json::json!("fail"));
        assert_eq!(
            entry["executor_session_id"],
            serde_json::json!(attempt_sessions[idx]),
            "fail 条目仅携 executor 槽位、值续注该轮首发会话"
        );
        assert!(
            entry.get("evaluator_session_id").is_none(),
            "evaluator 未跑无槽位键"
        );
    }
    for entry in upgrades.iter().take(4) {
        assert!(entry.get("decision_session_id").is_none());
    }
    assert_eq!(
        upgrades[4]["decision_session_id"],
        serde_json::json!(decisions[0]),
        "决策收口后最新 fail 条目定点挂账"
    );
}

/// 决策会话收口即挂账（假写面 DecisionLog 臂适配）：ToolCommand::DecisionLog
/// 载荷逐字段捕获（change / phase / session_id），时间线位次在决策会话收口
/// 之后、phase-next 重路由之前。
#[tokio::test]
async fn 决策会话收口即挂账_decisionlog载荷逐字段捕获() {
    let root = TempRoot::new("decisionlog-capture");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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
    assert_eq!(payloads[0].0, CHANGE, "change 载荷逐字段");
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
    let root = TempRoot::new("decisionlog-fail");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "挂账失败显式失败停给用户");

    let mut finished = None;
    while let Ok(update) = rx.try_recv() {
        if let RunUpdate::Finished { .. } = &update {
            finished = Some(update);
        }
    }
    let value = wire(&finished.expect("Finished 信封流出"));
    let reason = value["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("decision-log 失败"),
        "记因为挂账失败: {value}"
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
    let root = TempRoot::new("ask-relog");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        false,
        &root.root_str(),
    );
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
        .answer(CHANGE, "终止".to_owned())
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
    assert_eq!(payloads[0].0, CHANGE);
    assert_eq!(payloads[0].1, "implement");
    assert_eq!(payloads[0].2, first_session, "两轮挂账同会话 id");
}

/// emit_step 词汇零新增（结构证明——run-state.ts 零触点的 proposal「不要修
/// 改」口径）：决策挂账全程更新流的步词汇封闭于既有九词，无新步状态信封。
#[tokio::test]
async fn 步状态词汇零新增_决策挂账全程无新步信封() {
    let root = TempRoot::new("vocab-closed");
    root.change(CHANGE, DECISION_FIXTURE);

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
    let (diff, _) = FakeDiff::new(vec![]).assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(FsSnapshot::new(root.root_str()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx) = spawn_run(
        worker,
        tools,
        diff,
        snapshot,
        &control,
        true,
        &root.root_str(),
    );
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }
    let kinds: std::collections::BTreeSet<String> = step_rows(&updates)
        .into_iter()
        .map(|(_, _, kind, _)| kind)
        .collect();
    const VOCAB: [&str; 9] = [
        "executor",
        "evaluator",
        "decision",
        "phaseStart",
        "staticCheck",
        "phaseLog",
        "verdictGate",
        "retryGate",
        "whitelistGate",
    ];
    for kind in &kinds {
        assert!(
            VOCAB.contains(&kind.as_str()),
            "步状态词汇漂移（新增词 {kind}）——RunUpdate / ChangeStepKind 出新词汇即破坏零触点"
        );
    }
    // 决策挂账链路确经更新流（决策角色 + 落账步 + verdict 门在场）
    assert!(kinds.contains("decision"), "决策会话步在场");
    assert!(kinds.contains("phaseLog"), "落账步在场");
    assert!(kinds.contains("verdictGate"), "verdict 门在场");
}

// ---------------------------------------------------------------------------
// 装置：无产出快照占位（假双缝用例不触快照面——决策分叉用例另用真实 FsSnapshot）
// ---------------------------------------------------------------------------

/// 决策分叉未触发的用例的快照占位（walker 仅在 MaxRetriesExceeded 分叉读快照）。
struct StubSnapshot;

impl WorkflowSnapshotPort for StubSnapshot {
    fn detail(
        &self,
        _root: &str,
        _change: &str,
    ) -> Result<workflow::queries::ChangeDetail, String> {
        Err("假双缝用例不应触达快照面".to_owned())
    }
}

/// static-check 直过占位（真实 LocalToolSteps 组合用例的注入 runner）。
struct NullRunner;

impl StaticCheckRunner for NullRunner {
    fn run(&self, _root: &str) -> BoxToolFuture {
        Box::pin(async move { Ok(ToolStepOutput::StaticCheck(passing_check())) })
    }
}

/// static-check 恒败 runner（反馈边超限升格路径驱动——六检五修后升格 fail）。
struct FailingRunner;

impl StaticCheckRunner for FailingRunner {
    fn run(&self, _root: &str) -> BoxToolFuture {
        Box::pin(async move {
            Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
                passed: false,
                diagnostics: "clippy 未过: E0308".to_owned(),
            }))
        })
    }
}

/// broadcast receiver 的非阻塞轮询摘取（current_thread 运行时协作让位面）。
fn futures_poll(rx: &mut tokio::sync::broadcast::Receiver<RunUpdate>) -> Option<RunUpdate> {
    rx.try_recv()
        .ok()
        .filter(|update| matches!(update, RunUpdate::Ask { .. }))
}
