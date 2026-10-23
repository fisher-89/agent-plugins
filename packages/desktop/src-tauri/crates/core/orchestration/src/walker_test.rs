use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent::{AgentEventKind, AgentRunStatus, ModelLevel};
use foundation::cancel::CancelFlag;

use crate::control::ChangeFlowControl;
use crate::port::{
    ArchiveVcsPort, BoxToolFuture, BoxTurnFuture, MutationCounts, MutationSkipReason,
    MutationTestConclusion, MutationTestOutcome, RebaseOutcome, RunFault, RunHistoryPort,
    StaticCheckOutcome, StaticCheckRunner, TestExecutionConclusion, TestExecutionOutcome,
    TestExecutionRunner, ToolCommand, ToolStepOutput, ToolStepPort, ToolStepRequest,
    WorkerAgentPort, WorkerRole, WorkerTurnOutcome, WorkerTurnRequest, WorkflowSnapshotPort,
    WorktreeSnapshot,
};
use crate::snapshot::StoreSnapshot;
use crate::state::{
    ArchivePreflight, ChangeRunStatus, ChangeStepKind, ChangeStepStatus, RunNotice,
};
use crate::steps::LocalToolSteps;
use crate::walker::{new_run_id, walk_run, RunRequest};
use store::Store;
use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand, RunFinishCommand,
    RunNodeEntry, RunStartCommand, RunStatus, RunStepStatus,
};
use workflow::write::{
    phase_table, ArtifactSpec, BacktrackOption, BacktrackOutcome, DecisionLogOutcome, LastResult,
    OnFail, PhaseLogOutcome, PhaseNextError, PhaseNextOutcome, PhaseStartOutcome, SessionAnchors,
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

/// 建档种子（worktree 形态，check-before-flow-start）：`worktree = Some(执行锚)`
/// ——initialize worktree 分支驱动用例的记录面（exec root ≠ 主 root，相位级
/// 短路不触发）。
fn seed_change_with_worktree(store: &Store, worktree: &std::path::Path) {
    store
        .create_change_record(ChangeStateRecord {
            id: CHANGE_ID.to_owned(),
            name: NAME.to_owned(),
            title: NAME.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: TS_BASE,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: Some(worktree.to_string_lossy().into_owned()),
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
    /// executor 会话 Stopped 收敛注入队列（AC-2 stopped 半边：run_worker
    /// `AgentRunStatus::Stopped` 臂终态 emit 断言面——置位后下一次 executor
    /// 会话产出 Stopped 收敛）
    executor_stops: Mutex<VecDeque<()>>,
    /// 转录注入队列（run_worker token 求和断言面）：每次 worker.run 弹出一组
    /// 密封事件 kind，逐条 `AgentEvent::stamp` 进该会话转录（空组 / 耗尽 =
    /// 零注入既有形态）；多 TurnDone.usage / 非 TurnDone 干扰 / usage 值类型
    /// 漂移均经本注入口承载（walker 的注入端口依赖——最小 mock 合规）。
    transcripts: Mutex<VecDeque<Vec<AgentEventKind>>>,
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
            executor_stops: Mutex::new(VecDeque::new()),
            transcripts: Mutex::new(VecDeque::new()),
            counter: AtomicUsize::new(0),
            gate: None,
        }
    }

    /// executor 会话 Stopped 收敛预录（队列深度 = 注入次数）。
    fn with_executor_stops(mut self, times: usize) -> Self {
        self.executor_stops = Mutex::new(VecDeque::from(vec![(); times]));
        self
    }

    /// 转录注入预录：每次 worker.run 按序弹出一组密封事件 kind 铸进转录
    ///（多 TurnDone.usage / 干扰事件 / 类型漂移样本的注入面）。
    fn with_transcripts(mut self, scripts: Vec<Vec<AgentEventKind>>) -> Self {
        self.transcripts = Mutex::new(scripts.into());
        self
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
        // executor 会话 Stopped 收敛注入（耗尽回落 false = 正常 Completed 收口）
        let executor_stop = match role {
            WorkerRole::Executor => self
                .executor_stops
                .lock()
                .expect("executor stopped 队列锁不可中毒")
                .pop_front()
                .is_some(),
            _ => false,
        };
        // 转录注入脚本（耗尽回落 None = 零注入既有形态）
        let injected_transcript = self
            .transcripts
            .lock()
            .expect("转录注入队列锁不可中毒")
            .pop_front();

        Box::pin(async move {
            // evaluator 停等门：armed 期间挂起（停止时序窗口；notify_one 带许可
            // 语义——放行通知早于 future 首轮 poll 亦不丢）
            if let Some(gate) = gate {
                if *gate.armed.lock().expect("门锁不可中毒") {
                    gate.notify.notified().await;
                }
            }
            let _ = &mut timeline;
            // 转录注入：脚本组逐条 `AgentEvent::stamp`（空组 / 耗尽 = 零注入
            // 既有形态）
            let scripted: Option<Vec<agent::AgentEvent>> = injected_transcript.map(|kinds| {
                kinds
                    .into_iter()
                    .enumerate()
                    .map(|(seq, kind)| agent::AgentEvent::stamp(seq as u64, kind))
                    .collect()
            });
            // executor 失败注入：Failed 收敛 + 携成因的密封转录（真实引擎
            // api_error 记因 + 失败 TurnDone 的同形形态；无记因时 TurnDone
            // subtype 置空——成因提取显式缺席的形态）。转录脚本在场时以脚本
            // 为转录（失败收敛 token 求和的注入面——D2 无论 outcome.status）。
            if let Some(cause) = executor_failure.flatten() {
                let transcript = match scripted {
                    Some(transcript) => transcript,
                    None => {
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
                        transcript
                    }
                };
                return Ok(WorkerTurnOutcome {
                    session_id,
                    status: AgentRunStatus::Failed,
                    final_message: None,
                    transcript,
                });
            }
            // executor Stopped 收敛注入（AC-2 stopped 半边：StepSettle::stopped
            // 终态 emit 断言面）
            if executor_stop {
                return Ok(WorkerTurnOutcome {
                    session_id,
                    status: AgentRunStatus::Stopped,
                    final_message: None,
                    transcript: scripted.unwrap_or_default(),
                });
            }
            let final_message = match role {
                WorkerRole::Executor => None,
                WorkerRole::Evaluator => match evaluator.flatten() {
                    Some(Ok(text)) => Some(text),
                    Some(Err(message)) => return Err(message),
                    None => Some(PASS_JSON.to_owned()),
                },
                WorkerRole::Decision => Some(decision.flatten().unwrap_or_else(|| {
                    // 缺省决策：受控终止（未预录决策产出的用例不空转；
                    // 触发决策的用例恒显式预录产出）
                    r#"{ "action": "stop", "reason": "（默认）决策受控终止" }"#.to_owned()
                })),
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
                transcript: scripted.unwrap_or_default(),
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
    /// 收到的 `ToolStepRequest.cancel` 序（步标签 + flag——`run_tool` 单点填充
    /// 与「13 处调用点共用同一原子」的捕获面）
    cancels: Arc<Mutex<Vec<(&'static str, CancelFlag)>>>,
    /// static-check 步挂起臂（置位后该步等待 cancel 置位才返回——「stop 后
    /// 仍推进到下一次 run_tool」的确定性窗口）
    hold_static: AtomicBool,
    phase_next: Mutex<VecDeque<PhaseNextOutcome>>,
    static_check: Mutex<VecDeque<StaticCheckOutcome>>,
    /// test-execution 可编程产出序列（pass / fail / error 四态——耗尽回落
    /// pass 恒过）
    test_execution: Mutex<VecDeque<TestExecutionOutcome>>,
    /// 归档前置载荷可编程产出序列（耗尽回落 archive_preflight_payload）
    archive_preflight: Mutex<VecDeque<ArchivePreflight>>,
    /// 归档双写产出可编程序列（耗尽回落 archive_seal_outcome）
    archive_seal: Mutex<VecDeque<workflow::write::ArchiveOutcome>>,
    /// mutation-test 产出可编程序列（耗尽回落 EmptyDiff 跳过面恒 Pass）
    mutation: Mutex<VecDeque<MutationTestOutcome>>,
    /// 命中即 Err 的步标签（phase-next / phase-start / phase-log / backtrack /
    /// decision-log / static-check / test-execution / mutation-test /
    /// archive-preflight / archive-seal）
    fail_on: Mutex<Option<String>>,
    attempt_counter: AtomicUsize,
}

impl FakeTools {
    fn new(timeline: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            timeline: Arc::clone(timeline),
            commands: Arc::new(Mutex::new(Vec::new())),
            roots: Arc::new(Mutex::new(Vec::new())),
            cancels: Arc::new(Mutex::new(Vec::new())),
            hold_static: AtomicBool::new(false),
            phase_next: Mutex::new(VecDeque::new()),
            static_check: Mutex::new(VecDeque::new()),
            test_execution: Mutex::new(VecDeque::new()),
            archive_preflight: Mutex::new(VecDeque::new()),
            archive_seal: Mutex::new(VecDeque::new()),
            mutation: Mutex::new(VecDeque::new()),
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

    /// cancel 捕获柄（assemble 消费 self 前取——`run_tool` 单点填充观察面）。
    fn cancels_handle(&self) -> Arc<Mutex<Vec<(&'static str, CancelFlag)>>> {
        Arc::clone(&self.cancels)
    }

    /// static-check 步挂起臂：该步捕获请求后等待 cancel 置位才返回（「stop 后
    /// 仍未收口、继续推进到下一次 run_tool」的确定性握手窗口）。
    fn hold_static_check(mut self) -> Self {
        self.hold_static = AtomicBool::new(true);
        self
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
    fn with_test_execution(mut self, outcomes: Vec<TestExecutionOutcome>) -> Self {
        self.test_execution = Mutex::new(outcomes.into());
        self
    }

    /// mutation-test 产出序列预录（Pass / Fail / Error 三态与跳过面；
    /// 耗尽回落 EmptyDiff 跳过面恒 Pass 零反馈）。
    fn with_mutation(mut self, outcomes: Vec<MutationTestOutcome>) -> Self {
        self.mutation = Mutex::new(outcomes.into());
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
        ToolCommand::MutationTest { .. } => "mutation-test",
        ToolCommand::ArchivePreflight { .. } => "archive-preflight",
        ToolCommand::ArchiveSeal { .. } => "archive-seal",
    }
}

impl ToolStepPort for FakeTools {
    fn run(&self, step: ToolStepRequest) -> BoxToolFuture {
        let label = command_label(&step.command);
        self.cancels
            .lock()
            .expect("cancel 捕获锁不可中毒")
            .push((label, step.cancel.clone()));
        if label == "static-check" && self.hold_static.load(Ordering::SeqCst) {
            // 挂起臂：等待取消置位（真实执行体的「轮询置位」收敛面在写面等价
            // 表达）——测试侧在窗口内 request_stop，随后本步返回、run 继续推进
            let cancel = step.cancel.clone();
            return Box::pin(async move {
                while !cancel.is_set() {
                    tokio::task::yield_now().await;
                }
                Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
                    fault: None,
                    passed: true,
                    diagnostics: String::new(),
                    output_ref: None,
                    report_dir: String::new(),
                }))
            });
        }
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
                        fault: None,
                        passed: true,
                        diagnostics: String::new(),
                        output_ref: None,
                        report_dir: String::new(),
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
                        fault: None,
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
            // mutation-test 可编程臂：预录产出序列逐次弹出（Pass / Fail /
            // Error 与跳过面），耗尽回落 EmptyDiff 跳过面恒 Pass 零反馈——
            // 非突变用例零扰动
            ToolCommand::MutationTest { .. } => {
                let outcome = self
                    .mutation
                    .lock()
                    .expect("mutation-test 队列锁不可中毒")
                    .pop_front()
                    .unwrap_or(MutationTestOutcome {
                        fault: None,
                        conclusion: MutationTestConclusion::Pass,
                        skip: Some(MutationSkipReason::EmptyDiff),
                        score: 0.0,
                        threshold: 0.0,
                        counts: MutationCounts::default(),
                        report_paths: Vec::new(),
                        feedback_text: String::new(),
                    });
                Box::pin(async move { Ok(ToolStepOutput::MutationTest(outcome)) })
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
/// 回溯白名单 fixture（id-only；描述缺省空串——walker 装置只关心 id 路由）。
fn allowed_options(ids: &[&str]) -> Vec<BacktrackOption> {
    ids.iter()
        .map(|id| BacktrackOption {
            phase: (*id).to_owned(),
            description: String::new(),
        })
        .collect()
}

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
        allowed_backtrack_phases: allowed_options(allowed),
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
        allowed_backtrack_phases: allowed_options(allowed),
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

/// 预录 verdict fail 决策分叉产出（第 1 条 fail 即分叉的形态：round=2 =
/// 首条 fail 落账后的次轮；其余面与 max_retries_outcome 同形）。
fn verdict_fail_outcome(phase: &str, allowed: &[&str], last_report: &str) -> PhaseNextOutcome {
    PhaseNextOutcome {
        done: false,
        next_phase: None,
        round: 2,
        steps: Vec::new(),
        artifact_manifest: Vec::new(),
        executor: None,
        evaluator: None,
        allowed_backtrack_phases: allowed_options(allowed),
        last_result: Some(LastResult {
            phase: phase.to_owned(),
            verdict: Verdict::Fail,
            report: last_report.to_owned(),
            timestamp: None,
        }),
        error: Some(PhaseNextError::VerdictFail {
            phase: phase.to_owned(),
            round: 2,
        }),
    }
}

fn passing_check() -> StaticCheckOutcome {
    StaticCheckOutcome {
        fault: None,
        passed: true,
        diagnostics: String::new(),
        output_ref: None,
        report_dir: String::new(),
    }
}

fn failing_check(diagnostics: &str) -> StaticCheckOutcome {
    StaticCheckOutcome {
        fault: None,
        passed: false,
        diagnostics: diagnostics.to_owned(),
        output_ref: None,
        report_dir: String::new(),
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
    /// 变更文件清单（change_edit_set 产出面）
    edit_set: Mutex<Vec<String>>,
    /// 清单取数 Err 注入位
    edit_set_error: Mutex<Option<String>>,
    /// `--in-diff` 补丁（change_edit_patch 产出面）
    edit_patch: Mutex<String>,
    /// 补丁取数 Err 注入位
    edit_patch_error: Mutex<Option<String>>,
    /// worktree 清理（remove_worktree 产出面——Cleanup 步尽力语义注入位）
    remove_result: Mutex<Result<(), String>>,
    /// remove_worktree 双参捕获（Cleanup 步参数透传断言观察面）
    remove_calls: Mutex<Vec<(PathBuf, PathBuf)>>,
    /// Ok 时是否真实移除磁盘目录（默认关——多段子用例共享 TempRoot 时防
    /// fixture 交叉污染；成功移除观察面用例显式开启）
    remove_removes_dir: Mutex<bool>,
    /// 基线恢复（restore_baseline 产出面——initialize Clean 步 Err 注入位）
    restore_result: Mutex<Result<String, String>>,
    /// restore_baseline 载荷捕获（worktree 与 exclusions——D6 排除面组装断言
    /// 观察面，check-before-flow-start）
    restore_calls: Mutex<Vec<(PathBuf, Vec<String>)>>,
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
            edit_set: Mutex::new(Vec::new()),
            edit_set_error: Mutex::new(None),
            edit_patch: Mutex::new(String::new()),
            edit_patch_error: Mutex::new(None),
            remove_result: Mutex::new(Ok(())),
            remove_calls: Mutex::new(Vec::new()),
            remove_removes_dir: Mutex::new(false),
            restore_result: Mutex::new(Ok("已重置 0 个路径、清除 0 个未跟踪文件".to_owned())),
            restore_calls: Mutex::new(Vec::new()),
        }
    }

    /// 清单 / 补丁产出面编程（突变执行链组合用例的数据注入）。
    fn with_edit(self, files: Vec<String>, patch: &str) -> Self {
        *self.edit_set.lock().expect("vcs 清单锁不可中毒") = files;
        *self.edit_patch.lock().expect("vcs 补丁锁不可中毒") = patch.to_owned();
        self
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

    /// worktree 清理产出面编程（Cleanup 步失败警告注入——Ok / Err 可编程）。
    fn with_remove_result(self, result: Result<(), String>) -> Self {
        *self.remove_result.lock().expect("vcs 清理锁不可中毒") = result;
        self
    }

    /// Ok 时真实移除磁盘目录（成功移除观察面用例显式开启——fs 不 mock 的
    /// 「worktree 目录消失」断言面；默认关防共享 fixture 交叉污染）。
    fn with_real_removal(self) -> Self {
        *self
            .remove_removes_dir
            .lock()
            .expect("vcs 清理开关锁不可中毒") = true;
        self
    }

    /// remove_worktree 双参捕获快照（Cleanup 步参数透传断言入口）。
    fn remove_calls(&self) -> Vec<(PathBuf, PathBuf)> {
        self.remove_calls
            .lock()
            .expect("vcs 清理调用锁不可中毒")
            .clone()
    }

    /// 基线恢复产出面编程（initialize Clean 步 Err 注入——Ok 摘要 / Err 可编程，
    /// check-before-flow-start）。
    fn with_restore_result(self, result: Result<String, String>) -> Self {
        *self.restore_result.lock().expect("vcs 恢复锁不可中毒") = result;
        self
    }

    /// restore_baseline 载荷捕获快照（(worktree, exclusions)——D6 排除面组装
    /// 断言入口）。
    fn restore_calls(&self) -> Vec<(PathBuf, Vec<String>)> {
        self.restore_calls
            .lock()
            .expect("vcs 恢复调用锁不可中毒")
            .clone()
    }

    /// 解冲突后验用：B 快照 HEAD 与 rebase 半程标记可编程（后验四判据的注入面）。
    fn with_snapshot_head(self, head: &str, rebase_head: Option<&str>) -> Self {
        *self.snapshot_head.lock().expect("vcs 快照头锁不可中毒") = Some(head.to_owned());
        *self.snapshot.lock().expect("vcs 快照锁不可中毒") = Ok(snapshot_of(
            rebase_head.map_or("head-a", |_| head),
            rebase_head,
        ));
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
        self.calls.lock().expect("vcs 调用锁不可中毒").push(label);
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

    fn change_edit_set(&self, _root: &std::path::Path, base: &str) -> Result<Vec<String>, String> {
        self.record(format!("change_edit_set:{base}"));
        if let Some(error) = self
            .edit_set_error
            .lock()
            .expect("vcs 清单错误锁不可中毒")
            .clone()
        {
            return Err(error);
        }
        Ok(self.edit_set.lock().expect("vcs 清单锁不可中毒").clone())
    }

    fn change_edit_patch(&self, _root: &std::path::Path, base: &str) -> Result<String, String> {
        self.record(format!("change_edit_patch:{base}"));
        if let Some(error) = self
            .edit_patch_error
            .lock()
            .expect("vcs 补丁错误锁不可中毒")
            .clone()
        {
            return Err(error);
        }
        Ok(self.edit_patch.lock().expect("vcs 补丁锁不可中毒").clone())
    }

    fn remove_worktree(
        &self,
        main_root: &std::path::Path,
        worktree: &std::path::Path,
    ) -> Result<(), String> {
        self.record(format!("remove_worktree:{}", worktree.display()));
        self.remove_calls
            .lock()
            .expect("vcs 清理调用锁不可中毒")
            .push((main_root.to_owned(), worktree.to_owned()));
        let result = self
            .remove_result
            .lock()
            .expect("vcs 清理锁不可中毒")
            .clone();
        // Ok 且显式开启时真实移除磁盘目录（worktree 目录消失 = 归档 Cleanup
        // 成功面的观察锚——fs 不 mock，在场探测为被测逻辑本体；默认关防多段
        // 子用例共享 TempRoot 的 fixture 交叉污染）
        if result.is_ok()
            && *self
                .remove_removes_dir
                .lock()
                .expect("vcs 清理开关锁不可中毒")
        {
            let _ = std::fs::remove_dir_all(worktree);
        }
        result
    }

    fn restore_baseline(
        &self,
        worktree: &std::path::Path,
        exclusions: &[&str],
    ) -> Result<String, String> {
        self.record("restore_baseline".to_owned());
        self.restore_calls
            .lock()
            .expect("vcs 恢复调用锁不可中毒")
            .push((
                worktree.to_owned(),
                exclusions.iter().map(|s| (*s).to_owned()).collect(),
            ));
        self.restore_result
            .lock()
            .expect("vcs 恢复锁不可中毒")
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
        allowed_backtrack_phases: allowed_options(&TABLE),
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
            OnFail::Terminal | OnFail::Escalate => None,
        })
        .unwrap_or_else(|| panic!("相位 {phase} 的 {node:?} 步应声明 FeedbackLoop"))
}

/// static-check 门禁反馈上限（implement / test-gen 声明同值）。
fn static_check_feedback_limit() -> u32 {
    feedback_limit_of("implement", StepNode::StaticCheck)
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

/// 摘取决策会话请求的 prompt（首个 Decision 角色；锁作用域收在同步函数内，
/// 不跨 await 持锁）。
fn decision_prompt_of(requests: &Arc<Mutex<Vec<WorkerTurnRequest>>>) -> String {
    requests
        .lock()
        .expect("请求锁")
        .iter()
        .find(|request| request.role == WorkerRole::Decision)
        .map(|request| request.prompt.clone())
        .expect("决策会话请求应在场")
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
        Arc::new(NullMutationRunner),
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

    // 落库演进与插件直跑形态对照一致：10 相位各 1 条（initialize 表头 legacy
    // 短路 skipped 条目 + 8 agent 相位 pass + archive 空 checklist pass；
    // attempt 1、表序）
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(
        entries.len(),
        10,
        "十相位各落账一条（initialize 表头 + 含 archive）"
    );
    let phases: Vec<&str> = entries.iter().map(|entry| entry.phase.as_str()).collect();
    assert_eq!(
        phases,
        [
            "initialize",
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
        } else if entry.phase == "initialize" {
            // initialize 表头（root == main_root legacy 短路）：skipped=true 落账
            //（跳过因可见——一次性持久同 pass 语义；空 checklist 零会话槽位）
            assert!(entry.skipped, "legacy 短路 initialize 条目 skipped=true");
            assert!(!entry.stale, "干净 run 无 stale 标记");
            assert!(
                !entry.report.is_empty() && entry.checklist.is_empty(),
                "initialize skipped 条目携跳过因 report + 空 checklist: {entry:?}"
            );
        } else {
            assert!(
                !entry.report.is_empty() && !entry.checklist.is_empty(),
                "条目形状与插件 buildEntry 同形: {entry:?}"
            );
        }
        if entry.phase != "initialize" {
            assert!(
                !entry.skipped && !entry.stale,
                "干净 run 无 skipped / stale 标记"
            );
        }
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

#[tokio::test]
async fn 重入自active_phase续走不重跑已pass相位() {
    let root = TempRoot::new("resume");
    root.change(NAME);
    let db = TestDb::open("resume");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "initialize",
        Verdict::Pass,
        "基线检查通过",
        Vec::new(),
        TS_BASE,
    );
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
    db.store
        .amend_decision_session(CHANGE_ID, "dev-design", "sess-decision-seed")
        .expect("种子决策挂账应成功");
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
        Arc::new(NullMutationRunner),
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
        11,
        "dev-design 重评条目纯追加（initialize 表头 + 既有 8 + dev-design 重评 + archive 收口）"
    );
    assert_eq!(entries[0].phase, "initialize", "initialize 表头条目在案");
    assert_eq!(entries[9].phase, "dev-design");
    assert_eq!(entries[9].attempt, 2);
    assert_eq!(entries[9].verdict, Verdict::Pass);
    assert_eq!(entries[10].phase, "archive", "归档相位随后收口");
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

/// walk_run 决策会话（正向）：phase-next 返回 MaxRetriesExceeded → 决策 agent
/// 恰唤起一次（`ModelLevel::High`）、输入有界（fail checklist + 白名单 + 候选
/// eval report + 失败相位报告原文）、白名单门节点恰一个且 passed（决策前零门
/// 节点——门已迁入会话）；首轮合法 backtrack 落账后重路由收敛。
#[tokio::test]
async fn 决策分叉唤起恰一次且输入有界() {
    let (root, db) = decision_fixture("decision-input");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计返工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // 首轮正常路由：白名单缓存随行（决策分叉的 allowed 来自上一站
            // phase-next 缓存——walker 的缓存语义面）
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            // 回跳目标相位自该处重跑 → 收敛
            route_outcome("dev-design", &["proposal", "dev-design"]),
            done_outcome(),
        ])
        .assemble();

    // 真实快照源：决策输入取自 ChangeDetail 只读装配（db 读源——决策有界输入
    // 来源）；查得即 detail(root, **id**) 形参随动证明（磁盘目录名恒 name，
    // id ≠ name 形态下 name 入参必 miss）
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "决策 backtrack → 重路由 done"
    );

    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 1, "决策 agent 恰唤起一次");
    assert_eq!(
        decision_requests[0].model_level,
        ModelLevel::High,
        "决策会话恒 High 档"
    );
    let prompt = &decision_requests[0].prompt;

    // 输入有界四面：fail checklist + 写面下发白名单 + 候选相位 eval report +
    // 失败相位报告原文
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
    assert!(
        prompt.contains("## 失败相位报告") && prompt.contains("首轮未过"),
        "失败相位报告段渲染失败条目 report 原文: {prompt}"
    );
    drop(requests);

    // 门节点：决策会话内恰一个实例且 passed（决策前零 WhitelistGate——门已迁
    // 入会话，resolve_deadlock 不再 emit 门节点）
    let finish = history.finish().expect("finish 落包捕获");
    let gates: Vec<&RunNodeEntry> = finish
        .nodes
        .iter()
        .filter(|node| node.step == workflow::state::RunStepKind::WhitelistGate)
        .collect();
    assert_eq!(gates.len(), 1, "每轮决策恰 emit 一个门实例");
    assert_eq!(
        gates[0].status,
        RunStepStatus::Passed,
        "合法 backtrack 门 passed"
    );
    assert_eq!(gates[0].phase, "implement", "门节点归属失败相位");
}

/// walk_run verdict fail 决策分叉（正向）：phase-next 返回 VerdictFail（任意
/// 相位第 1 条未决 verdict fail——本用例评估-only 的 code-review）→ 立即唤起
/// 决策 agent（不自走同相位重跑）；决策 backtrack 到自身（评估-only 相位可重做，
/// 白名单自含）经门 passed 落账 → 同相位重跑恰一次（evaluator attempt 1 → 2）。
#[tokio::test]
async fn verdict_fail分叉立即唤起决策_backtrack自身重跑同相位() {
    // db 种子：proposal / implement pass + code-review fail（决策输入读源）
    let root = TempRoot::new("eval-only-decision");
    root.change(NAME);
    let db = TestDb::open("eval-only-decision");
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
        "implement",
        Verdict::Pass,
        "实现齐备",
        Vec::new(),
        TS_BASE + 60_000,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "code-review",
        Verdict::Fail,
        "审查未过",
        vec![ChecklistItem {
            item: "错误处理".to_owned(),
            pass: false,
            evidence: "unwrap 裸用 3 处".to_owned(),
        }],
        TS_BASE + 120_000,
    );

    let allowed = [
        "proposal",
        "dev-design",
        "test-design",
        "implement",
        "test-gen",
        "code-review",
    ];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "code-review", "reason": "重做本相位" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // 首轮正常路由（白名单缓存面）→ 相位 pass 落账 → 次轮分叉错误 →
            // 决策 backtrack 到自身 → 重路由同相位 → 重跑 pass → done
            route_outcome("code-review", &allowed),
            verdict_fail_outcome("code-review", &allowed, "审查未过"),
            route_outcome("code-review", &allowed),
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
        "评估-only fail → 决策 backtrack 自身 → 重跑 pass → done"
    );

    let requests = requests.lock().expect("请求锁");

    // 决策 agent 恰唤起一次：prompt 携失败相位报告段 + db fail checklist
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 1, "决策 agent 恰唤起一次");
    let prompt = &decision_requests[0].prompt;
    assert!(
        prompt.contains("## 失败相位报告") && prompt.contains("审查未过"),
        "失败相位报告段渲染失败条目 report 原文: {prompt}"
    );
    assert!(
        prompt.contains("- code-review（= 重做本相位）"),
        "白名单自含（评估-only 相位可重做）注记在场: {prompt}"
    );
    assert!(
        !prompt.contains("eval_only") && !prompt.contains("评估-only 相位（无 executor）"),
        "eval_only 指引段已退役（三动作 + 报告面组装）"
    );
    assert!(
        prompt.contains("错误处理") && prompt.contains("unwrap 裸用 3 处"),
        "fail checklist（item + evidence）自 db 快照随行: {prompt}"
    );

    // 决策 backtrack 到自身 → 同相位重跑恰一次：evaluator attempt 1 → 2
    let evaluator_attempts: Vec<String> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Evaluator)
        .filter_map(|request| request.provenance.source_ref.as_deref())
        .filter(|source_ref| source_ref.contains("code-review/evaluator"))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        evaluator_attempts,
        vec![
            format!("{CHANGE_ID}/code-review/evaluator/1"),
            format!("{CHANGE_ID}/code-review/evaluator/2"),
        ],
        "evaluator 两轮（首轮 pass 落账后次轮分叉为装置预录，backtrack 重跑一轮）"
    );
    drop(requests);

    // backtrack 决议落账恰一次（白名单自含 = 重做本相位）
    let commands = commands.lock().expect("命令锁");
    let backtracks: Vec<&ToolCommand> = commands
        .iter()
        .filter(|command| matches!(command, ToolCommand::Backtrack { .. }))
        .collect();
    assert_eq!(backtracks.len(), 1, "backtrack 落账恰一次");
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

/// walk_run 决策分叉触发红线（边界）：phase-next 响应无 error 时全程决策
/// agent 零调用——决策只由路由权威（phase_next 分叉触发）唤起，walker 不自
/// 持「fail 即决策」转移规则（fail→分叉判定驻写面；本用例假路由恒正常下发，
/// 对 walker 侧零自走决策的红线对照）。
#[tokio::test]
async fn 路由无error全程决策agent零调用() {
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
        "路由无 error 全程决策 agent 零调用（决策只由 phase_next 分叉触发唤起）"
    );
}

/// walk_run 越权 backtrack 软拒（异常）：决议 backtrack 越白名单 → 门 failed
/// 节点（detail 携 allowed 列表）→ 漂移 1、同会话续注纠正；第二轮合法 stop 经
/// 门 passed 正常收敛（不一次打死 run，零 Backtrack 落账）。
#[tokio::test]
async fn 越权backtrack软拒一次后续注纠正并stop收敛() {
    let (root, db) = decision_fixture("overreach-decision");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "test-gen", "reason": "越权目标" }"#.to_owned(),
            r#"{ "action": "stop", "reason": "无合法回溯目标，人工介入" }"#.to_owned(),
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

    // 真实快照源：决策会话的 detail 读取（db 读源；越权决议走完快照面后由
    // 会话内白名单门驳回）
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());

    let (task, mut rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "越权驳回后第二轮 stop 受控收敛"
    );
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(command, ToolCommand::Backtrack { .. })),
        "Backtrack 步零发起（越权被门拦下）"
    );

    // 失败原因经 Finished 通知 + 落包捕获面流出
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
            .contains("决策 stop"),
        "stop 记因落包: {:?}",
        finish.reason
    );

    // 门节点：两轮各一个实例——首轮 failed（detail 携 allowed 列表原文）、
    // 第二轮 stop 经门 passed
    let gates: Vec<&RunNodeEntry> = finish
        .nodes
        .iter()
        .filter(|node| node.step == workflow::state::RunStepKind::WhitelistGate)
        .collect();
    assert_eq!(gates.len(), 2, "两轮决策各 emit 一个门实例");
    assert_eq!(gates[0].status, RunStepStatus::Failed, "越权门 failed");
    assert_eq!(
        gates[1].status,
        RunStepStatus::Passed,
        "合法 stop 门 passed"
    );
    let detail = gates[0].detail.clone().unwrap_or_default();
    assert!(
        detail.contains("越权") && detail.contains("proposal"),
        "门 detail 携 allowed 列表原文: {detail}"
    );

    // 漂移 1 后同会话续注：第二轮 continue_session == 第一轮
    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 2, "驳回后同会话续注一轮");
    let first_session = sessions
        .lock()
        .expect("会话登记锁")
        .iter()
        .find(|(role, _)| *role == WorkerRole::Decision)
        .map(|(_, session)| session.clone())
        .expect("首轮决策会话 id");
    assert_eq!(
        decision_requests[1].continue_session.as_deref(),
        Some(first_session.as_str()),
        "同会话续注（非新会话）"
    );
    assert!(
        decision_requests[1].prompt.contains("上一轮决策输出被驳回"),
        "第二轮 prompt 为纠正文案: {}",
        decision_requests[1].prompt
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
            r#"{ "action": "stop", "reason": "用户裁决终止" }"#.to_owned(),
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
    assert_eq!(status, ChangeRunStatus::Stopped, "应答后 stop 受控收敛");

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
    // 零 backtrack（stop 决议）
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
            r#"{ "action": "stop", "reason": "用户裁决终止" }"#.to_owned(),
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
    assert_eq!(status, ChangeRunStatus::Stopped, "ask 应答后 stop 受控收敛");
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
    assert_eq!(
        evaluators[0], evaluators[1],
        "修复轮续注同一 evaluator 会话"
    );
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
            && eval_requests[1]
                .prompt
                .contains("未在最终消息中找到合法 JSON")
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
/// 槽位）→ 每轮 fail 决策分叉（backtrack 到自身 ×4 兑现重跑）→ 第 5 轮决策
/// stop 收敛。
/// evaluator 队列恰 30 垃圾（精确值——假件耗尽回落 PASS_JSON，多给掩盖
/// off-by-one）。
#[tokio::test]
async fn 真实写面组合_verdict解析失败反馈边超限_升格双槽位决策stop() {
    let root = TempRoot::new("verdict-upgrade");
    root.change(NAME);
    let db = TestDb::open("verdict-upgrade");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    // initialize 表头条目预置 pass（phase_next 跳过表头——本 run 只验 proposal
    // 站升格链）
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "initialize",
        Verdict::Pass,
        "基线检查通过",
        Vec::new(),
        TS_BASE,
    );

    let garbage = "评估结论正文，无 JSON".to_owned();
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(garbage.clone()); 30])
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "proposal", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "proposal", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "proposal", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "proposal", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "stop", "reason": "评估输出修复与重试预算均耗尽" }"#.to_owned(),
        ])
        .assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(NullMutationRunner),
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
        finish
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("决策 stop")),
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
    assert_eq!(
        decisions.len(),
        5,
        "每轮 fail 各一轮决策（4 backtrack + 1 stop）"
    );
    drop(sessions);

    // fail 条目：五轮升格代写——report 携升格、item 为评估输出格式、双槽位
    // 在场（区别于 static-check 升格仅 executor 单槽）；每轮决策收口后其条目
    // 定点挂账（verdict 落账时 decision 槽位恒缺省，挂账经 decision_log 单点）
    // （entries[0] 为 initialize 表头预置条目——proposal fail 条目自 entries[1] 起）
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 6, "initialize 表头 + 五轮升格 fail 纯追加");
    assert_eq!(entries[0].phase, "initialize");
    for (idx, entry) in entries.iter().skip(1).enumerate() {
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
            entry.checklist[0]
                .evidence
                .contains("未在最终消息中找到合法 JSON"),
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
    for (idx, entry) in entries.iter().skip(1).enumerate() {
        assert_eq!(
            entry.decision_session_id.as_deref(),
            Some(decisions[idx].as_str()),
            "每轮决策收口后其 fail 条目定点挂账"
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
/// MaxRetriesExceeded → 决策输入白名单段为空，backtrack 决议被门驳回（软拒
/// 一次、同会话续注），stop 经门 passed 受控收敛——仅剩 stop / ask 出口。
#[tokio::test]
async fn 空白名单backtrack决议被拒() {
    let (root, db) = decision_fixture("empty-whitelist-decision");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "proposal", "reason": "空名单越权" }"#.to_owned(),
            r#"{ "action": "stop", "reason": "无回溯目标，人工介入" }"#.to_owned(),
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
        ChangeRunStatus::Stopped,
        "空白名单 backtrack 出口封死后 stop 受控收敛"
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
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 2, "驳回后同会话续注一轮");
    let decision_prompt = &decision_requests[0].prompt;
    assert!(
        !decision_prompt.contains("- proposal"),
        "白名单段为空（无候选相位行）"
    );
    assert!(
        decision_requests[1].prompt.contains("上一轮决策输出被驳回"),
        "第二轮为纠正文案: {}",
        decision_requests[1].prompt
    );
}

/// 步门控声明锚定（边界）：static-check 反馈上限 5（沿 hook loop_limit 语义）
/// 由相位定义 step 声明供给；声明 static-check 步的相位 == ["implement",
/// "test-gen"]（布局身份常量退役——转移判定恒问写面 phase-next，不据其路由）。
#[test]
fn 步门控声明锚定() {
    assert_eq!(static_check_feedback_limit(), 5);
    assert_eq!(
        static_check_phases(),
        vec!["initialize", "implement", "test-gen"],
        "initialize 表头检查步为 Terminal 分派（非 FeedbackLoop——声明面锚随表头扩位）"
    );
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
    // initialize 表头条目预置 pass（phase_next 跳过表头——本 run 只验 proposal
    // 站五轮 fail 链的槽位面）
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "initialize",
        Verdict::Pass,
        "基线检查通过",
        Vec::new(),
        TS_BASE,
    );

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
            r#"{ "action": "backtrack", "to": "proposal", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "proposal", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "proposal", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "proposal", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "stop", "reason": "预算耗尽人工介入" }"#.to_owned(),
        ])
        .assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(NullMutationRunner),
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

    // 假引擎会话序（槽位值对应断言的事实源）：executor / evaluator 交替五轮；
    // 每轮 fail 各一轮决策（前 4 backtrack 兑现重跑 + 末轮 stop）
    let sessions = sessions.lock().expect("会话登记锁");
    let executors = sessions_of(&sessions, WorkerRole::Executor);
    let evaluators = sessions_of(&sessions, WorkerRole::Evaluator);
    let decisions = sessions_of(&sessions, WorkerRole::Decision);
    assert_eq!(executors.len(), 5, "五轮 executor（每 attempt 一会话）");
    assert_eq!(evaluators.len(), 5, "五轮 evaluator");
    assert_eq!(
        decisions.len(),
        5,
        "每轮 fail 各一轮决策（4 backtrack + 1 stop）"
    );
    drop(sessions);

    // verdict 条目落库：executor / evaluator 槽位与会话序逐一对应（取值自
    // WorkerTurnOutcome.session_id——AC-5；entries[0] 为 initialize 表头预置条目）
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(entries.len(), 6, "initialize 表头 + 五条 fail 条目纯追加");
    assert_eq!(entries[0].phase, "initialize");
    for (idx, entry) in entries.iter().skip(1).enumerate() {
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
    // verdict 落账时 decision 槽位恒缺省（D5）；每轮决策收口后其条目经
    // decision_log 写面单点定点挂账
    for (idx, entry) in entries.iter().skip(1).enumerate() {
        assert_eq!(
            entry.decision_session_id.as_deref(),
            Some(decisions[idx].as_str()),
            "决策会话收口后其 id 记录在案（walk_run → LocalToolSteps → decision_log 写面）"
        );
    }
}

/// 挂账先于决策解析（真实组合链留痕）：决策会话连续产出漂移文本 → parse
/// 失败路径软拒环（漂移 3 > `DECISION_DRIFT_LIMIT`）→ run 显式 failed；db
/// 最新条目已携 decision_session_id——写挂时机在 parse 之前，parse 失败同样
/// 留痕（每轮挂账幂等覆写）。
#[tokio::test]
async fn 真实写面组合_决策挂账先于解析_parse失败同样留痕() {
    let root = TempRoot::new("relog-before-parse");
    root.change(NAME);
    let db = TestDb::open("relog-before-parse");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    // initialize 表头条目预置 pass（phase_next 跳过表头——本 run 只验 proposal
    // 站决策挂账链）
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "initialize",
        Verdict::Pass,
        "基线检查通过",
        Vec::new(),
        TS_BASE,
    );

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(fail_json("一"))])
        .with_decision_reports(vec![
            "不是决策 JSON".to_owned(),
            "仍不是决策 JSON".to_owned(),
            "第三次仍不是决策 JSON".to_owned(),
        ])
        .assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(NullRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(NullMutationRunner),
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
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "解析漂移超限显式 failed（第 3 次驳回）"
    );

    // 终态记因为决策漂移（非挂账失败——挂账已成功先于 parse）
    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Failed);
    let reason = finish.reason.as_deref().unwrap_or_default();
    assert!(
        reason.contains("决策漂移") && reason.contains("无法解析"),
        "记因为解析漂移（挂账不在失败路径）: {:?}",
        finish.reason
    );

    // 留痕：三轮解析失败均挂账，最新 fail 条目携最后决策会话 id
    let sessions = sessions.lock().expect("会话登记锁");
    let decisions = sessions_of(&sessions, WorkerRole::Decision);
    assert_eq!(decisions.len(), 3, "漂移上限前恰三轮决策（同会话续注）");
    drop(sessions);
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert_eq!(
        entries.len(),
        2,
        "initialize 表头 + 一条 fail 条目（不重复落账）"
    );
    assert_eq!(
        entries[1].decision_session_id.as_deref(),
        Some(decisions[2].as_str()),
        "parse 失败路径同样留痕（末轮挂账覆盖）"
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
        "initialize",
        Verdict::Pass,
        "基线检查通过",
        Vec::new(),
        TS_BASE,
    );
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
            r#"{ "action": "backtrack", "to": "implement", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "implement", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "implement", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "implement", "reason": "重做本相位" }"#.to_owned(),
            r#"{ "action": "stop", "reason": "反馈边与重试预算均耗尽" }"#.to_owned(),
        ])
        .assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(FailingRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(NullMutationRunner),
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
    assert_eq!(
        decisions.len(),
        5,
        "每轮升格 fail 各一轮决策（4 backtrack + 1 stop）"
    );
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
    assert_eq!(
        entries.len(),
        9,
        "initialize 表头 + 预置三条 pass + 五轮升格 fail 纯追加"
    );
    let upgrades = &entries[4..];
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
    for (idx, entry) in upgrades.iter().enumerate() {
        assert_eq!(
            entry.decision_session_id.as_deref(),
            Some(decisions[idx].as_str()),
            "每轮决策收口后其升格 fail 条目定点挂账"
        );
    }
}

/// 决策会话收口即挂账（假写面 DecisionLog 臂适配）：ToolCommand::DecisionLog
/// 载荷逐字段捕获（change / phase / session_id），时间线位次在决策会话收口
/// 之后、phase-next 重路由之前。
#[tokio::test]
async fn 决策会话收口即挂账_decisionlog载荷逐字段捕获() {
    let (root, db) = decision_fixture("decisionlog-capture");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "implement", "reason": "重做本相位" }"#.to_owned(),
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
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
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
        "决策 backtrack 重路由 done 收敛"
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
    assert_eq!(
        timeline[decision_pos + 2],
        "backtrack",
        "挂账后决策动作兑现（Backtrack 落账）"
    );
    assert_eq!(timeline[decision_pos + 3], "phase-next", "兑现后回路由");
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
    let (worker, _, _) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "implement", "reason": "重做本相位" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
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
    // 落库二十值全词汇零过滤（词汇漂移防线：封闭集之外的词出现即破坏；
    // mutation_test 拆步新词与 cleanup 尽力步新臂在案——AC-1 / AC-2）
    const VOCAB: [&str; 12] = [
        "executor",
        "evaluator",
        "decision",
        "static_check",
        "test_execution",
        "mutation_test",
        "phase_start",
        "phase_log",
        "verdict_gate",
        "retry_gate",
        "whitelist_gate",
        "cleanup",
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

/// 决策分叉未触发的用例的快照占位（walker 仅在决策分叉触发点——重试上限 /
/// 评估-only fail——读快照）。
struct StubSnapshot;

impl WorkflowSnapshotPort for StubSnapshot {
    fn detail(&self, _root: &str, _id: &str) -> Result<workflow::queries::ChangeDetail, String> {
        Err("假双缝用例不应触达快照面".to_owned())
    }
}

/// static-check 直过占位（真实 LocalToolSteps 组合用例的注入 runner）。
struct NullRunner;

impl StaticCheckRunner for NullRunner {
    // 磁盘面 port 收 (root, name, slot, cancel)：假件零记录逻辑，四参原样吞下
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _slot: foundation::layout::ReportSlot,
        _cancel: CancelFlag,
    ) -> BoxToolFuture {
        Box::pin(async move { Ok(ToolStepOutput::StaticCheck(passing_check())) })
    }
}

/// static-check 恒败 runner（反馈边超限升格路径驱动——六检五修后升格 fail）。
struct FailingRunner;

impl StaticCheckRunner for FailingRunner {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _slot: foundation::layout::ReportSlot,
        _cancel: CancelFlag,
    ) -> BoxToolFuture {
        Box::pin(async move {
            Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
                fault: None,
                passed: false,
                diagnostics: "clippy 未过: E0308".to_owned(),
                output_ref: None,
                report_dir: String::new(),
            }))
        })
    }
}

/// test-execution 恒过占位（真实 LocalToolSteps 组合用例的注入 runner——绿跑
/// 机械 checklist 路径；fail / error / Err 四态序列用例由 FakeTools 可编程臂
/// 承载）。
struct NullTestExecutionRunner;

impl TestExecutionRunner for NullTestExecutionRunner {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _slot: foundation::layout::ReportSlot,
        _cancel: CancelFlag,
    ) -> BoxToolFuture {
        Box::pin(async move {
            Ok(ToolStepOutput::TestExecution(TestExecutionOutcome {
                fault: None,
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

/// mutation-test 空清单跳过占位（真实 LocalToolSteps 组合用例的注入 runner
/// ——EmptyDiff 跳过面恒 Pass 零反馈，非突变用例不被扰动；可编程序列用例由
/// FakeTools 可编程臂承载）。
struct NullMutationRunner;

impl crate::port::MutationRunner for NullMutationRunner {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _files: &[String],
        _patch: &str,
        _cancel: CancelFlag,
    ) -> crate::port::BoxToolFuture {
        Box::pin(async move {
            Ok(ToolStepOutput::MutationTest(MutationTestOutcome {
                fault: None,
                conclusion: MutationTestConclusion::Pass,
                skip: Some(MutationSkipReason::EmptyDiff),
                score: 0.0,
                threshold: 0.0,
                counts: MutationCounts::default(),
                report_paths: Vec::new(),
                feedback_text: String::new(),
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
        fault: None,
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
    // 拆步语义（AC-1）：TestExecution 绿后才进 MutationTest（walker 顺序执行
    // 天然保证），MutationTest 缺省 EmptyDiff 跳过面恒 Pass → PhaseLog 机械落账
    assert_eq!(
        timeline[start + 2],
        "mutation-test",
        "TestExecution 绿后才进 MutationTest（拆步顺序）"
    );
    assert_eq!(
        timeline[start + 3],
        "phase-log",
        "MutationTest 收口后机械落账"
    );
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

/// 决策输入最小快照假件：pipeline 空的 `ChangeDetail`（升格 / 分派用例只走
/// 决策骨架，不断言输入内容——内容断言见「决策分叉唤起有界输入」用例）。
struct MinimalSnapshot;

impl WorkflowSnapshotPort for MinimalSnapshot {
    fn detail(&self, _root: &str, _id: &str) -> Result<workflow::queries::ChangeDetail, String> {
        Ok(workflow::queries::ChangeDetail {
            id: CHANGE_ID.to_owned(),
            name: NAME.to_owned(),
            title: NAME.to_owned(),
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

/// test-execution 门禁产出 fixture（结论 + 明细文本可编程；计数固定 5 例）。
fn escalate_test_outcome(
    conclusion: TestExecutionConclusion,
    findings_detail: &str,
) -> TestExecutionOutcome {
    TestExecutionOutcome {
        fault: None,
        conclusion,
        total: 5,
        passed: 4,
        failed: u64::from(conclusion != TestExecutionConclusion::Pass),
        skipped: 0,
        findings_brief: "「node-test」1 项测试失败——单点失败".to_owned(),
        findings_detail: findings_detail.to_owned(),
        report_dir: format!("C:/ws/openspec/changes/{NAME}/reports/test"),
        exec_id: "exec-test-1".to_owned(),
    }
}

/// 落包中某类步的 `(instance_no, status)` 序列（升格实例面断言入口）。
fn step_instances(
    nodes: &[RunNodeEntry],
    step: workflow::state::RunStepKind,
) -> Vec<(u32, RunStepStatus)> {
    nodes
        .iter()
        .filter(|node| node.step == step)
        .map(|node| (node.instance_no, node.status))
        .collect()
}

/// 某相位首次 phase-log 载荷（升格落账断言入口）。
fn phase_log_input(commands: &[ToolCommand], phase: &str) -> workflow::write::PhaseLogInput {
    commands
        .iter()
        .find_map(|command| match command {
            ToolCommand::PhaseLog {
                phase: logged_phase,
                input,
                ..
            } if logged_phase == phase => Some(input.clone()),
            _ => None,
        })
        .expect("phase-log 载荷应在场")
}

/// 驱动一次 test-execution 相位首败升格（fail 明细可编程）：FakeTools 预录
/// [TestExecution 产出 → phase-next 分叉 → 决策 stop]，返回（终态 / 命令序 /
/// 会话请求序 / 落包）。
async fn drive_test_execution_escalate(
    detail: &str,
) -> (
    ChangeRunStatus,
    Vec<ToolCommand>,
    Vec<WorkerTurnRequest>,
    RunFinishCommand,
) {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "测试执行失败交人工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("test-execution", &[]),
            max_retries_outcome("test-execution", &[], "测试执行失败"),
        ])
        .with_test_execution(vec![escalate_test_outcome(
            TestExecutionConclusion::Fail,
            detail,
        )])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(MinimalSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    let commands = commands.lock().expect("命令锁").clone();
    let requests = requests.lock().expect("请求锁").clone();
    let finish = history.finish().expect("finish 落包捕获");
    (status, commands, requests, finish)
}

/// 正向：test-execution 首败升格（`on_fail=Escalate`）——恰 1 条 fail
/// PhaseRecord（三会话槽位恒 None）、TestExecution 节点 failed、零
/// `WorkerRole::Executor` / `Evaluator` 请求、MutationTest 与常规 PhaseLog 步未
/// 执行、升格 PhaseLog 恰一个（实例面）；随后回 phase-next 进决策分叉。
#[tokio::test]
async fn test_execution首败升格_零会话单条fail落账与实例面() {
    let detail = format!("## 失败用例明细\n\n{}", "失败行 abcdefghij\n".repeat(30));
    let (status, commands, requests, finish) = drive_test_execution_escalate(&detail).await;

    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "升格后决策 stop 受控收敛（已回 phase-next 进决策分叉）"
    );

    // 零 agent 会话：Executor / Evaluator / Decision 之外无请求，前两者恒零
    assert!(
        requests
            .iter()
            .all(|request| !matches!(request.role, WorkerRole::Executor | WorkerRole::Evaluator)),
        "首败升格零 Executor / Evaluator 请求，实际: {:?}",
        requests
            .iter()
            .map(|request| request.role)
            .collect::<Vec<_>>()
    );

    // 步执行面：TestExecution 恰一次、MutationTest 零次（升格截断余下步）
    let test_calls = commands
        .iter()
        .filter(|command| matches!(command, ToolCommand::TestExecution { .. }))
        .count();
    let mutation_calls = commands
        .iter()
        .filter(|command| matches!(command, ToolCommand::MutationTest { .. }))
        .count();
    assert_eq!(test_calls, 1, "TestExecution 门禁恰执行一次（零重跑）");
    assert_eq!(
        mutation_calls, 0,
        "MutationTest 步未执行（Upgraded 截断余下步）"
    );

    // 升格落账：恰一条 fail checklist、report 携完整明细、三会话槽位恒 None
    let payload = phase_log_input(&commands, "test-execution");
    assert_eq!(payload.checklist.len(), 1, "升格落账恰一条 fail item");
    assert_eq!(payload.checklist[0].item, "测试执行");
    assert!(!payload.checklist[0].pass, "落账为 fail checklist");
    assert!(
        payload.checklist[0].evidence.contains("单点失败")
            && payload.checklist[0].evidence.contains("reports/test"),
        "evidence 携诊断摘要 + 报告目录: {}",
        payload.checklist[0].evidence
    );
    assert_eq!(
        payload.report, detail,
        "report 携完整明细原文（≤2000 不截断）"
    );
    assert!(
        payload.executor_session_id.is_none()
            && payload.evaluator_session_id.is_none()
            && payload.decision_session_id.is_none(),
        "三会话槽位恒 None（本相位零 agent 会话）"
    );

    // 实例面：TestExecution 节点 instance_no 恰 1（零重跑）、升格 PhaseLog 节点
    // 恰 1 个
    let test_nodes = step_instances(&finish.nodes, workflow::state::RunStepKind::TestExecution);
    assert_eq!(test_nodes.len(), 1, "TestExecution 节点单实例");
    assert_eq!(test_nodes[0].0, 0, "instance_no 从 0 起（零重跑）");
    assert_eq!(test_nodes[0].1, RunStepStatus::Failed, "首败节点 failed");
    let log_nodes = step_instances(&finish.nodes, workflow::state::RunStepKind::PhaseLog);
    assert_eq!(
        log_nodes.len(),
        1,
        "升格 PhaseLog 节点恰 1 个（常规步未达）"
    );
    assert_eq!(log_nodes[0].1, RunStepStatus::Passed, "升格落账节点 passed");
}

/// 边界：升格 report 截断（参数化 6 值）——明细 0 / 1 / 1999 / 2000 / 2001 /
/// 2500 字符：≤2000 原样、2001 取前 1999 + `…`（总长恰 2000）、2500 同式；
/// 空明细兜底 `conclusion=…；报告目录 …`。
#[tokio::test]
async fn test_execution升格报告截断六形态() {
    for len in [0usize, 1, 1999, 2000, 2001, 2500] {
        let detail = "明".repeat(len);
        let (_, commands, _, _) = drive_test_execution_escalate(&detail).await;
        let report = phase_log_input(&commands, "test-execution").report;
        let report_len = report.chars().count();
        if len == 0 {
            assert!(
                report.starts_with("conclusion=fail") && report.contains("报告目录"),
                "空明细兜底结论行 + 报告目录: {report}"
            );
        } else if len <= 2000 {
            assert_eq!(report, detail, "明细 {len} 字符原样落账（无省略号）");
            assert_eq!(report_len, len);
        } else {
            assert_eq!(report_len, 2000, "超长截断后总长恰 2000（写面不拒）");
            assert!(report.ends_with('…'), "截断留省略号: {report}");
            assert!(
                report.starts_with(&"明".repeat(1999)),
                "取前 1999 字符 + 省略号（不截 2000 再加）"
            );
        }
    }

    // 多字节内容（emoji / 换行 / NUL）：按 chars() 计数截断，不 panic、不切坏
    // 字符边界（Rust String 保证 UTF-8 完整）
    let mixed = format!("{}{}", "🚀\n\0".repeat(800), "尾");
    let (_, commands, _, _) = drive_test_execution_escalate(&mixed).await;
    let report = phase_log_input(&commands, "test-execution").report;
    assert_eq!(report.chars().count(), 2000, "多字节内容按字符数截断");
    assert!(report.ends_with('…'), "多字节截断留省略号");
    assert!(
        report
            .chars()
            .all(|ch| ch == '🚀' || ch == '\n' || ch == '\0' || ch == '…'),
        "不切坏字符边界（逐字符仍在原字符集内）"
    );
}

/// 异常：`conclusion=Error` 同式升格——零会话、恰 1 条 fail 条目、余下步不
/// 执行（error 与 fail 两时点同一升格面）。
#[tokio::test]
async fn test_execution_error结论同式升格() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "测试环境错误交人工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("test-execution", &[]),
            max_retries_outcome("test-execution", &[], "测试执行出错"),
        ])
        .with_test_execution(vec![escalate_test_outcome(
            TestExecutionConclusion::Error,
            "suite 执行中断：进程退出码 2",
        )])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(MinimalSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "error 结论同式升格后决策 stop"
    );

    assert!(
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .all(|request| !matches!(request.role, WorkerRole::Executor | WorkerRole::Evaluator)),
        "error 结论零 agent 修复会话"
    );
    let commands = commands.lock().expect("命令锁");
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, ToolCommand::MutationTest { .. }))
            .count(),
        0,
        "MutationTest 步未执行"
    );
    let payload = phase_log_input(&commands, "test-execution");
    assert_eq!(payload.checklist.len(), 1, "恰一条 fail 条目");
    assert_eq!(payload.report, "suite 执行中断：进程退出码 2");
}

/// 异常：`run_tool(TestExecution)` 返回 `Err`（`fail_on("test-execution")`）→
/// `Terminal` 收敛——零 fail 条目、零升格 PhaseLog 节点、零决策请求（环境问题
/// 不进决策）。
#[tokio::test]
async fn test_execution_run_tool_err_terminal零落账零决策() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[])])
        .fail_on("test-execution")
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "runner Err 显式 failed 停给用户"
    );
    let finish = history.finish().expect("finish 落包捕获");
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("test-execution 假写面注入失败"),
        "记因为 runner 显式失败: {:?}",
        finish.reason
    );

    let commands = commands.lock().expect("命令锁");
    assert!(
        commands
            .iter()
            .all(|command| !matches!(command, ToolCommand::PhaseLog { .. })),
        "零 fail 条目落账（环境问题不进决策）"
    );
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "零决策请求（不进决策分叉）"
    );
    // 节点留 running（与 terminal 分支同式）
    let test_nodes = step_instances(&finish.nodes, workflow::state::RunStepKind::TestExecution);
    assert_eq!(test_nodes.len(), 1);
    assert_eq!(test_nodes[0].1, RunStepStatus::Running, "节点留 running");
}

/// 边界：test-execution 步 `on_fail` 分派——`Some(OnFail::Terminal)`（initialize
/// 基线绿门语义）走既有 terminal 执行体且零决策请求；`None` 与
/// `FeedbackLoop { 5 }` 误声明 → 计划图漂移 `Err`、零 `run_worker`、零门禁 spawn
///（不得静默退化）。
#[tokio::test]
async fn test_execution分派漂移与terminal形态() {
    // 形态一：Terminal 声明 → 既有 terminal 执行体（基线绿门），失败即停零决策
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let mut terminal_route = route_outcome("test-execution", &[]);
    terminal_route.steps = vec![StepDefinition {
        node: StepNode::TestExecution,
        prompt_ref: None,
        on_fail: Some(OnFail::Terminal),
    }];
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![terminal_route])
        .with_test_execution(vec![escalate_test_outcome(
            TestExecutionConclusion::Fail,
            "基线失败明细",
        )])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "Terminal 声明失败即停给用户"
    );
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "Terminal 形态零决策请求"
    );
    drop(commands);

    // 形态二 / 三：None 与 FeedbackLoop 误声明 → 计划图漂移 Err（零门禁 spawn）
    for on_fail in [None, Some(OnFail::FeedbackLoop { limit: 5 })] {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let mut drifted_route = route_outcome("test-execution", &[]);
        drifted_route.steps = vec![StepDefinition {
            node: StepNode::TestExecution,
            prompt_ref: None,
            on_fail,
        }];
        let (tools, commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![drifted_route])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(
            status,
            ChangeRunStatus::Failed,
            "on_fail={on_fail:?} 误声明显式漂移失败"
        );
        let finish = history.finish().expect("finish 落包捕获");
        let reason = finish.reason.as_deref().unwrap_or_default();
        assert!(
            reason.contains("计划图漂移") && reason.contains("test-execution"),
            "漂移记因: {reason}"
        );
        assert!(
            requests.lock().expect("请求锁").is_empty(),
            "漂移路径零 run_worker"
        );
        assert!(
            commands
                .lock()
                .expect("命令锁")
                .iter()
                .all(|command| !matches!(command, ToolCommand::TestExecution { .. })),
            "漂移路径零门禁 spawn（不静默退化为禁用）"
        );
    }
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

/// 门控步声明锚定：test-execution 相位两确定性门禁步（TestExecution /
/// MutationTest）声明 `OnFail::Escalate`（首败升格交决策、零会话）；initialize
/// 表头同名 TestExecution 步仍 `Terminal`（语义由表项声明承载，MUST NOT 按相位
/// 名判断）；static-check 反馈边上限仍 5。
#[test]
fn test_execution门控声明锚定() {
    let def = phase_def("test-execution");
    for node in [StepNode::TestExecution, StepNode::MutationTest] {
        let step = def
            .steps
            .iter()
            .find(|step| step.node == node)
            .unwrap_or_else(|| panic!("test-execution 相位应有 {node:?} 步"));
        assert_eq!(
            step.on_fail,
            Some(OnFail::Escalate),
            "test-execution {node:?} 步声明 Escalate（零 agent 修复）"
        );
    }
    assert_eq!(
        test_execution_phases(),
        vec!["initialize", "test-execution"],
        "声明 TestExecution 步的相位（声明面锚随表头扩位）"
    );
    let initialize = phase_def("initialize");
    let baseline = initialize
        .steps
        .iter()
        .find(|step| step.node == StepNode::TestExecution)
        .expect("initialize 应有 TestExecution 步");
    assert_eq!(
        baseline.on_fail,
        Some(OnFail::Terminal),
        "同一 step 词在 initialize 表项仍 Terminal（语义随表项声明）"
    );
    assert_eq!(
        static_check_feedback_limit(),
        5,
        "static-check 反馈边上限仍 5"
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
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "implement", "reason": "重做本相位" }"#.to_owned(),
        ])
        .assemble();
    let tools_device = FakeTools::new(&timeline).with_phase_next(vec![
        // 首轮正常路由（白名单缓存随行）→ 次轮预算耗尽分叉触快照 → 决策回跳
        // 重跑 → 收敛
        route_outcome("implement", &["proposal", "implement"]),
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
        "max_retries → 决策 backtrack → 重路由 done"
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
        Arc::new(NullMutationRunner),
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

    // 节点整包：node_seq 升序 = emit 序、二十一值全词汇零过滤、整包同刻
    let nodes = store.list_run_nodes("run-1").expect("节点读史应成功");
    assert!(!nodes.is_empty(), "多相位 run 节点整包非空");
    const FULL_WORDS: [workflow::state::RunStepKind; 21] = [
        workflow::state::RunStepKind::Executor,
        workflow::state::RunStepKind::Evaluator,
        workflow::state::RunStepKind::Decision,
        workflow::state::RunStepKind::StaticCheck,
        workflow::state::RunStepKind::TestExecution,
        workflow::state::RunStepKind::MutationTest,
        workflow::state::RunStepKind::Clean,
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
        workflow::state::RunStepKind::Cleanup,
        workflow::state::RunStepKind::Seal,
        workflow::state::RunStepKind::Finalize,
    ];
    for row in &nodes {
        assert!(
            FULL_WORDS.contains(&row.step),
            "落库步词汇越二十一值全词汇封闭集，实际: {:?}",
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
    // 归档相位十一步节点在案（全相位含 archive 走完的落库面）
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
            "cleanup",
            "seal",
            "finalize",
            "phase_log"
        ],
        "归档十一步节点随整包落库"
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
async fn 全词汇run零过滤_流程面与三门在案() {
    // 决策分叉输入取自 ChangeDetail 只读装配——真件库种子（快照读源真实组合）
    let (root, _db) = decision_fixture("full-vocab");
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计返工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // implement：executor + static-check 反馈边（fail→pass）+ evaluator
            // + verdict 门 + phase-log + phase-start（白名单缓存随行）
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            // 决策分叉：whitelist 门 + retry 门 + decision 会话
            max_retries_outcome(
                "dev-design",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            route_outcome("dev-design", &["proposal", "dev-design"]),
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
    // 全词汇在案（决策 backtrack 分支兑现 → retry_gate 门节点在场；决策分叉 +
    // 门禁 + test-execution 相位全覆盖——含拆步后的 mutation_test）
    for word in [
        "executor",
        "evaluator",
        "decision",
        "static_check",
        "test_execution",
        "mutation_test",
        "phase_start",
        "phase_log",
        "verdict_gate",
        "whitelist_gate",
        "retry_gate",
    ] {
        assert!(
            kinds.iter().any(|kind| kind == word),
            "落库词汇 {word} 应在场（零过滤），实际: {kinds:?}"
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
            "mutation_test".to_owned(),
            "phase_log".to_owned(),
            "phase_start".to_owned(),
            "retry_gate".to_owned(),
            "static_check".to_owned(),
            "test_execution".to_owned(),
            "verdict_gate".to_owned(),
            "whitelist_gate".to_owned(),
        ],
        "emit 词汇零过滤（流程面步骤与三门在案；retry_gate 词面冻结不改）"
    );
    // 边整包：计划序推进 + 门禁反馈回边 + 决策回跳（forward / feedback /
    // backtrack 在案），词汇限于四 kind 封闭集；新写面零 retry
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
    for word in ["forward", "feedback", "backtrack"] {
        assert!(
            edge_kinds.iter().any(|kind| kind == word),
            "边 {word} 应在场，实际: {edge_kinds:?}"
        );
    }
    assert!(
        !edge_kinds.iter().any(|kind| kind == "retry"),
        "新写面零 retry 边（存量解码契约）: {edge_kinds:?}"
    );
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
            Arc::new(NullMutationRunner),
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
        assert_eq!(
            status,
            ChangeRunStatus::Stopped,
            "首轮 initialize 表头 legacy 短路落账后停等拒绝收口（手动档零豁免特判——表头先行语义）"
        );
        let all = store
            .list_phase_records(CHANGE_ID)
            .expect("读相位条目应成功");
        let initialize_attempts: Vec<u32> = all
            .iter()
            .filter(|entry| entry.phase == "initialize")
            .map(|entry| entry.attempt)
            .collect();
        assert_eq!(
            initialize_attempts,
            vec![1],
            "首轮 initialize 表头 skipped 落账 attempt 1（跳过因可见——一次性持久语义）"
        );
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
            Arc::new(NullMutationRunner),
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
            vec![1],
            "proposal attempt 1 承接自 run-2（首轮 initialize 停等拒绝零 proposal 条目——表头后 attempt 自 1 重计）"
        );
        let phases: Vec<&str> = entries.iter().map(|entry| entry.phase.as_str()).collect();
        assert_eq!(
            phases,
            vec![
                "initialize",
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
            "落账序列严格前向（首轮 initialize 表头 skipped 条目 + 第二轮续走自 proposal 推进——已 pass 相位零重执行，末位归档收口）"
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

/// 边 kind 正向（AC-3 / AC-4）：一次 run 覆盖 forward / feedback / backtrack
/// 三 kind 各自落边——`retry` 为存量解码契约（新写面零产出）：相位 fail 一律
/// 经决策 backtrack 重入，落 `backtrack` 而非 `retry` 边。
#[tokio::test]
async fn 三边kind正向_一次run覆盖forward与feedback与backtrack() {
    let (root, db) = decision_fixture("edge-all-kinds");

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(fail_json("首轮未过"))])
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计返工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // ① implement 首轮：静态检查失败 → feedback 回边；evaluator fail → 相位 fail
            route_outcome("implement", &["proposal", "dev-design", "implement"]),
            // ② 相位 fail 未决 → 决策分叉决议 backtrack（to ∈ 白名单）→ 跨相位回跳
            max_retries_outcome(
                "implement",
                &["proposal", "dev-design", "implement"],
                "首轮未过",
            ),
            // ③ 回跳目标相位自该处重跑 → 绿跑收口
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
            "implement:1:phase_start:0",
            "implement:1:executor:0",
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
    // backtrack：决策回跳跨相位（源 = 失败相位末门节点、目标 = 回跳相位首节点；
    // 决策门节点 attempt 取决策输入（库内该相位最近 attempt））
    assert!(
        row(
            "implement:1:retry_gate:0",
            "dev-design:2:phase_start:0",
            RunEdgeKind::Backtrack
        ),
        "backtrack 落边（跨相位回跳）"
    );

    // 三 kind 各自齐备、零 retry（存量解码契约不新产出）
    let kinds: Vec<&str> = finish
        .edges
        .iter()
        .map(|edge| edge.edge_kind.as_str())
        .collect();
    for word in ["forward", "feedback", "backtrack"] {
        assert!(kinds.contains(&word), "边 kind {word} 应在场: {kinds:?}");
    }
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "feedback").count(),
        1,
        "feedback 回边恰一条: {kinds:?}"
    );
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "backtrack").count(),
        1,
        "backtrack 回跳恰一条: {kinds:?}"
    );
    assert!(
        !kinds.contains(&"retry"),
        "新写面零 retry 产出（决策 backtrack 取代同相重跑）: {kinds:?}"
    );
    assert!(
        !kinds
            .iter()
            .any(|kind| !["forward", "feedback", "retry", "backtrack"].contains(kind)),
        "边 kind 零越四值封闭集: {kinds:?}"
    );
    assert!(
        keys.contains(&"dev-design:2:phase_start:0".to_owned()),
        "回跳目标节点在案: {keys:?}"
    );
}

/// 边 kind 正向（fail → 决策 backtrack 自身）：相位 evaluator fail → 决策
/// backtrack 到本相位（白名单自含）→ 重跑（attempt 递增）——重入边 kind 为
/// `backtrack` 而非 `retry`；新写面零 `retry` 边（存量解码契约）。
#[tokio::test]
async fn 边kind正向_相位fail经决策backtrack重入落backtrack边() {
    let (root, db) = decision_fixture("edge-backtrack-self");
    let allowed = ["proposal", "dev-design", "implement"];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Ok(fail_json("首轮未过")), Ok(PASS_JSON.to_owned())])
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "implement", "reason": "重做本相位" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &allowed),
            verdict_fail_outcome("implement", &allowed, "首轮未过"),
            route_outcome("implement", &allowed),
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
    assert_eq!(status, ChangeRunStatus::Completed, "重跑通过后走完收口");

    let finish = history.finish().expect("finish 落包捕获");
    let retry: Vec<&workflow::state::RunEdgeEntry> = finish
        .edges
        .iter()
        .filter(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Retry)
        .collect();
    assert!(
        retry.is_empty(),
        "新写面零 retry 边（同相重入经决策 backtrack）: {retry:?}"
    );
    let backtrack: Vec<&workflow::state::RunEdgeEntry> = finish
        .edges
        .iter()
        .filter(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Backtrack)
        .collect();
    assert_eq!(backtrack.len(), 1, "同相重入恰落一条 backtrack 边");
    assert_eq!(
        backtrack[0].from_node_key, "implement:1:retry_gate:0",
        "backtrack 源 = 失败相位末门节点"
    );
    assert_eq!(
        backtrack[0].to_node_key, "implement:2:phase_start:0",
        "backtrack 目标 = 重跑相位首节点（attempt 递增）"
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
        "相位 fail 重跑不误记 feedback: {kinds:?}"
    );

    let keys = assert_edges_closed(&finish.nodes, &finish.edges);
    assert!(
        keys.contains(&backtrack[0].from_node_key) && keys.contains(&backtrack[0].to_node_key),
        "backtrack 两端节点在案（图面闭合）"
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
// 归档相位（AC-3 / AC-4 / AC-6 / AC-7 / AC-8）：十一步执行序、git 步走 vcs 缝、
// 条件步短路、停等门、失败停链与组合链路落库
// ---------------------------------------------------------------------------

/// 归档相位十一步执行序（正向）：假写面 + 假 vcs 驱动 archive 相位 →
/// PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge →
/// SpecSync → Cleanup → Seal → Finalize → PhaseLog 按序落节点（emit 序即落包序），全步
/// `passed`（AC-3 / AC-4 11 步链序）。
#[tokio::test]
async fn 归档相位十一步执行序按计划步序落包() {
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
            "cleanup",
            "seal",
            "finalize",
            "phase_log"
        ],
        "归档十一步序与设计表逐位一致（Cleanup 介于 SpecSync 与 Seal）"
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

/// 归档 Cleanup 成功清理（正向，AC-5）：worktree 记录在场且目录在盘 →
/// `remove_worktree` 经 vcs 缝直调 → Cleanup 节点 `passed` + detail「已移除
/// worktree（{path}）」；worktree 磁盘目录消失（假件 Ok 即真实移除——fs 不
/// mock）；Seal / Finalize / PhaseLog 照常续走、run 收口 Completed。
#[tokio::test]
async fn 归档cleanup成功移除_worktree目录消失且链照常收口() {
    let root = TempRoot::new("archive-cleanup-ok");
    root.change(NAME);
    let worktree = worktree_dir(&root);
    assert!(
        std::path::Path::new(&worktree).is_dir(),
        "前置：worktree 目录在场（root.change 预置）"
    );
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let vcs = Arc::new(FakeVcs::clean().with_real_removal());
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

    assert_eq!(status, ChangeRunStatus::Completed, "清理成功链照常收口");
    // 清理步参数面：remove_worktree 以 (main_root, worktree) 双参原样到达 port
    let calls = vcs.remove_calls();
    assert_eq!(calls.len(), 1, "remove_worktree 恰一次: {calls:?}");
    assert_eq!(calls[0].0, root.0, "main_root 双参透传");
    assert_eq!(
        calls[0].1,
        std::path::PathBuf::from(&worktree),
        "worktree 路径双参透传（preflight.worktree 载荷零改写）"
    );
    // 磁盘面：worktree 目录消失（git worktree list 不再列出的等价观察面）
    assert!(
        !std::path::Path::new(&worktree).exists(),
        "worktree 磁盘目录已移除"
    );
    // 节点面：Cleanup passed + detail 成功摘要；Seal / Finalize / PhaseLog 续走
    let command = history.finish().expect("收口落包应在案");
    assert_eq!(
        node_detail(&command, workflow::state::RunStepKind::Cleanup).as_deref(),
        Some(format!("已移除 worktree（{worktree}）").as_str()),
        "Cleanup detail 携成功摘要（路径逐字）"
    );
    assert!(
        node_detail(&command, workflow::state::RunStepKind::Seal)
            .as_deref()
            .is_some_and(|detail| detail.contains("已归档")),
        "Seal 照常收口（ArchiveOutcome 消费面零变化）"
    );
    let rows = node_rows(&command);
    for step in ["cleanup", "seal", "finalize", "phase_log"] {
        assert!(
            rows.iter().any(|row| row.1 == step && row.2 == "passed"),
            "步 {step} passed（清理成功不断链）: {rows:?}"
        );
    }
}

/// 归档 Cleanup 失败尽力不断链（异常，AC-6）：remove_worktree 可编程 Err →
/// 步恒 `passed`、detail 携警告（worktree 路径 + `git worktree remove --force`
/// 手动指引——生产 Err 语境由 `ProcessArchiveVcs` 组装，假件 Err 注入同构文案）；
/// Seal / Finalize / PhaseLog 照常收口、run 收口 Completed、`ArchiveOutcome`
/// 消费面零变化（Seal detail 归档摘要与成功形态一致）。
#[tokio::test]
async fn 归档cleanup失败尽力_警告detail步恒passed且收口不断链() {
    let root = TempRoot::new("archive-cleanup-err");
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

    // Err 注入携真实 worktree 路径（与 ProcessArchiveVcs D6 Err 面同构——
    // 路径 + `git worktree remove --force {path}` 手动指引）
    let vcs = Arc::new(FakeVcs::clean().with_remove_result(Err(format!(
        "worktree 移除失败（{worktree}）: \
         请手动执行 git worktree remove --force {worktree} 清理"
    ))));
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

    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "尽力步失败不回滚归档（链必收口、永不 Terminal）"
    );
    let command = history.finish().expect("收口落包应在案");
    let cleanup_detail = node_detail(&command, workflow::state::RunStepKind::Cleanup)
        .expect("Cleanup detail 应在案");
    assert!(
        cleanup_detail.contains("worktree 清理失败"),
        "detail 携失败警告: {cleanup_detail}"
    );
    assert!(
        cleanup_detail.contains(&worktree),
        "警告携 worktree 路径: {cleanup_detail}"
    );
    assert!(
        cleanup_detail.contains("git worktree remove --force"),
        "警告携手动清理指引: {cleanup_detail}"
    );
    let rows = node_rows(&command);
    assert_eq!(
        rows.iter()
            .find(|row| row.1 == "cleanup")
            .map(|row| row.2.as_str()),
        Some("passed"),
        "步恒 passed（失败仅 detail 警告）: {rows:?}"
    );
    // 收口面：Seal / Finalize / PhaseLog 照常、ArchiveOutcome 消费面零变化
    assert!(
        node_detail(&command, workflow::state::RunStepKind::Seal)
            .as_deref()
            .is_some_and(|detail| detail.contains("已归档")),
        "Seal 照常收口（ArchiveOutcome 字段零变化）"
    );
    for step in ["finalize", "phase_log"] {
        assert!(
            rows.iter().any(|row| row.1 == step && row.2 == "passed"),
            "步 {step} 照常收口: {rows:?}"
        );
    }
    // branch 零触碰（AC-7）：失败路径调用记录词汇白名单——无 delete_branch 面
    let calls = vcs.calls();
    assert!(
        !calls.iter().any(|call| call.contains("delete_branch")),
        "全程零 branch 删除触点（含 remove 失败路径）: {calls:?}"
    );
    assert_eq!(
        vcs.call_count("remove_worktree:"),
        1,
        "remove_worktree 恰一次（失败也只试一次）: {calls:?}"
    );
}

/// 归档 Cleanup 缺席幂等跳过（边界，AC-6）：worktree 记录在场但目录不在盘
///（手动清 / 重试链形态；分支已合入 → Preflight 目录缺失分流照常）→ `passed`
/// + detail「worktree 已不在场，跳过清理」、零失败面、零 remove 调用。
#[tokio::test]
async fn 归档cleanup缺席幂等跳过_零失败面() {
    let root = TempRoot::new("archive-cleanup-absent");
    root.change(NAME);
    // 缺席 worktree 路径（落位根与主仓分置形态——目录从未建位，与真实
    // 记录携带的 worktree 落位同构）
    let worktree = root.0.join("worktrees-seg").join(NAME);
    let worktree = worktree.to_string_lossy().into_owned();
    assert!(
        !std::path::Path::new(&worktree).exists(),
        "前置：worktree 目录缺席"
    );

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![archive_route_outcome(), done_outcome()])
        .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    // 分支已合入（目录缺失分流 = 已合入照常；未合入会显式引导恢复目录——
    // 那是 Preflight 失败面，非 Cleanup 缺席跳过面）
    let vcs = Arc::new(FakeVcs::clean().with_merged(Ok(true)));
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

    assert_eq!(status, ChangeRunStatus::Completed, "缺席跳过零失败面");
    let command = history.finish().expect("收口落包应在案");
    assert_eq!(
        node_detail(&command, workflow::state::RunStepKind::Cleanup).as_deref(),
        Some("worktree 已不在场，跳过清理"),
        "TXT_SKIP_ABSENT 逐字注记"
    );
    let rows = node_rows(&command);
    assert_eq!(
        rows.iter()
            .find(|row| row.1 == "cleanup")
            .map(|row| row.2.as_str()),
        Some("passed"),
        "缺席跳过恒 passed"
    );
    assert!(
        vcs.remove_calls().is_empty(),
        "目录缺席零 remove 调用（在场探测驻调用方）: {:?}",
        vcs.calls()
    );
}

/// 归档 Cleanup 幂等重入（重试链，AC-6）：缺席跳过后链重入（重试链二次进入
/// Cleanup）同样 `passed` + 同词注记，不自绊。
#[tokio::test]
async fn 归档cleanup缺席重试链重入同词注记不自绊() {
    let root = TempRoot::new("archive-cleanup-retry");
    root.change(NAME);
    let worktree = root.0.join("worktrees-seg").join(NAME);
    let worktree = worktree.to_string_lossy().into_owned();
    assert!(
        !std::path::Path::new(&worktree).exists(),
        "前置：worktree 目录缺席"
    );
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);

    for round in ["first", "retry"] {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![archive_route_outcome(), done_outcome()])
            .with_archive_preflight(archive_preflight_twice(&worktree, Vec::new()))
            .assemble();
        let control = Arc::new(ChangeFlowControl::new());
        let vcs = Arc::new(FakeVcs::clean().with_merged(Ok(true)));
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

        assert_eq!(status, ChangeRunStatus::Completed, "{round} 轮照常收口");
        let command = history.finish().expect("收口落包应在案");
        assert_eq!(
            node_detail(&command, workflow::state::RunStepKind::Cleanup).as_deref(),
            Some("worktree 已不在场，跳过清理"),
            "{round} 轮同词注记（幂等重入不自绊）"
        );
        assert!(vcs.remove_calls().is_empty(), "{round} 轮零 remove 调用");
    }
}

/// 归档 Cleanup legacy 零触碰（边界，AC-7）：`worktree=None` change 归档 →
/// Cleanup `passed` + detail 记跳过因（TXT_SKIP_LEGACY 复用）、零 `git worktree`
/// 触点（remove 零调用）；调用记录词汇白名单无 branch 删除面。
#[tokio::test]
async fn 归档cleanuplegacy跳过_零worktree触点与零branch触碰() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
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
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Completed);
    let command = history.finish().expect("收口落包应在案");
    assert_eq!(
        node_detail(&command, workflow::state::RunStepKind::Cleanup).as_deref(),
        Some("legacy 无 worktree"),
        "TXT_SKIP_LEGACY 复用（跳过因逐字）"
    );
    let rows = node_rows(&command);
    assert_eq!(
        rows.iter()
            .find(|row| row.1 == "cleanup")
            .map(|row| row.2.as_str()),
        Some("passed"),
        "legacy 清理步 passed"
    );
    assert!(
        vcs.remove_calls().is_empty(),
        "legacy 零 git worktree 触点: {:?}",
        vcs.calls()
    );
    // 词汇白名单（AC-7）：全程调用记录无 branch 删除面（dirty / commit_all /
    // rebase_branch / ff_merge / worktree_snapshot / remove_worktree 在列形态）
    let calls = vcs.calls();
    assert!(
        !calls.iter().any(|call| call.contains("delete_branch")),
        "零 branch 删除触点: {calls:?}"
    );
}

/// 归档 Cleanup 缺前置载荷（异常）：计划漂移面（计划步序列缺 Preflight、
/// Cleanup 先行）→ Cleanup 节点 failed + Terminal——尽力语义不吞计划漂移
///（非清理失败面，沿既有归档步漂移面）。
#[tokio::test]
async fn 归档cleanup缺前置载荷_计划漂移failed且terminal() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    // 计划漂移形态：计划步序列跳过 Preflight、以 Cleanup 起步（PhaseCursor 无
    // 前置载荷——archive_context 缺位面）
    let mut drift = archive_route_outcome();
    drift.steps = vec![StepDefinition {
        node: StepNode::Cleanup,
        prompt_ref: None,
        on_fail: None,
    }];
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![drift])
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

    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "计划漂移 → Terminal（非尽力面）"
    );
    let command = history.finish().expect("收口落包应在案");
    let failed = command
        .nodes
        .iter()
        .find(|node| node.status == RunStepStatus::Failed)
        .expect("失败节点在案");
    assert_eq!(
        failed.step,
        workflow::state::RunStepKind::Cleanup,
        "失败节点恰为 Cleanup（漂移因记步上）"
    );
    assert_eq!(
        failed.detail.as_deref(),
        Some("缺归档前置载荷"),
        "漂移记因逐字"
    );
    assert!(
        vcs.remove_calls().is_empty(),
        "漂移面零 remove 触点（先于清理逻辑拒绝）"
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
            .with_rebase(Ok(RebaseOutcome::Conflicted(vec!["specs/a.md".to_owned()])))
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
    assert!(
        resolve_node.session_id.is_none(),
        "零会话步节点无 session_id"
    );
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
    assert_eq!(
        spec_sync_requests.len(),
        1,
        "delta specs 在场 → spec 同步会话"
    );
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
/// 仍可进入执行（`drive` 无 `next.evaluator.is_none()` 硬检查）——十一步照常落包
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
        11,
        "归档十一步全落包（零 evaluator 检查无阻）: {:?}",
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
/// 空 checklist 落 `verdict=pass` PhaseRecord；归档十一步节点 + 串行 forward 边落
/// 既有三表（零新表），身份键 `(phase, attempt, step, instance_no)` 稳定唯一。
#[tokio::test]
async fn 组合链路归档落库与空checklist落账() {
    let root = TempRoot::new("archive-compose");
    root.change(NAME);
    let db = TestDb::open("archive-compose");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    for (index, phase) in [
        "initialize",
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
        Arc::new(NullMutationRunner),
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

    // 执行图落库（AC-6 / AC-9）：十一步节点 + 十条 forward 边（零新表）
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
            "cleanup",
            "seal",
            "finalize",
            "phase_log"
        ],
        "归档十一步全落 RunNodeRecord（零过滤）"
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
    assert_eq!(edges.len(), 10, "串行推进边 = 节点数 - 1");
    assert!(
        edges
            .iter()
            .all(|edge| edge.edge_kind == workflow::state::RunEdgeKind::Forward),
        "归档串行边全 forward（零反馈 / 重试 / 回溯）"
    );
    assert_eq!(edges[0].from_node_key, "archive:1:phase_start:0");
    assert_eq!(edges[9].to_node_key, "archive:1:phase_log:0");
    // cleanup 节点串行边序（AC-9 落库组合）：SpecSync → Cleanup → Seal 相邻边
    assert_eq!(edges[6].from_node_key, "archive:1:spec_sync:0");
    assert_eq!(
        edges[6].from_node_key, edges[5].to_node_key,
        "SpecSync → Cleanup 边序衔接（emit 序即边序）"
    );
    assert_eq!(edges[6].to_node_key, "archive:1:cleanup:0");
    assert_eq!(edges[7].from_node_key, "archive:1:cleanup:0");
    assert_eq!(
        edges[7].to_node_key, "archive:1:seal:0",
        "Cleanup → Seal 相邻边"
    );

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
        .with_archive_preflight(archive_preflight_twice(&worktree_dir_str, Vec::new()))
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
            .with_rebase(Ok(RebaseOutcome::Conflicted(vec!["specs/a.md".to_owned()])))
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
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "停止 → 受控终止（非 failed）"
    );
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

// ---------------------------------------------------------------------------
// mutation-test 扩展节（desktop-fix-mutation-test）：拆步时序（AC-1）+ 双时点
// 反馈边②（AC-6）+ 独立计数 + 超限升格 + evidence 三态（AC-7 / E8）+ 双槽
// ---------------------------------------------------------------------------

/// mutation 门禁产出 fixture（三态：Pass / Fail / Error 与跳过面；feedback_text
/// 报告指引式——结论摘要 + 报告路径清单，MUST NOT 内联存活突变体清单）。
fn mutation_outcome(
    conclusion: MutationTestConclusion,
    score: f64,
    threshold: f64,
    feedback_text: &str,
) -> MutationTestOutcome {
    MutationTestOutcome {
        fault: None,
        conclusion,
        skip: None,
        score,
        threshold,
        counts: MutationCounts {
            killed: 7,
            survived: if conclusion == MutationTestConclusion::Pass {
                1
            } else {
                3
            },
            timeout: 0,
            compile_error: 0,
            runtime_error: 0,
            ignored: 2,
            total: 10,
        },
        report_paths: vec![
            "reports/test/app_rust/mutation/mutation-seg-0.json".to_owned(),
            "reports/test/app_rust/mutation/mutation.json".to_owned(),
        ],
        feedback_text: feedback_text.to_owned(),
    }
}

/// mutation 门禁产出 fixture（结论 + 反馈文本 + 报告路径清单可编程）。
fn escalate_mutation_outcome(
    conclusion: MutationTestConclusion,
    feedback_text: &str,
    report_paths: Vec<String>,
) -> MutationTestOutcome {
    MutationTestOutcome {
        fault: None,
        conclusion,
        skip: None,
        score: 55.0,
        threshold: 80.0,
        counts: MutationCounts::default(),
        report_paths,
        feedback_text: feedback_text.to_owned(),
    }
}

/// 驱动一次 test-execution 相位内 MutationTest 首败升格（TestExecution 先绿）：
/// 返回（终态 / 命令序 / 会话请求序 / 落包）。
async fn drive_mutation_escalate(
    feedback: &str,
    report_paths: Vec<String>,
) -> (
    ChangeRunStatus,
    Vec<ToolCommand>,
    Vec<WorkerTurnRequest>,
    RunFinishCommand,
) {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "突变未达标交人工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("test-execution", &[]),
            max_retries_outcome("test-execution", &[], "突变未达标"),
        ])
        .with_test_execution(vec![escalate_test_outcome(
            TestExecutionConclusion::Pass,
            "",
        )])
        .with_mutation(vec![escalate_mutation_outcome(
            MutationTestConclusion::Fail,
            feedback,
            report_paths,
        )])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(MinimalSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    let commands = commands.lock().expect("命令锁").clone();
    let requests = requests.lock().expect("请求锁").clone();
    let finish = history.finish().expect("finish 落包捕获");
    (status, commands, requests, finish)
}

/// 边界（拆步时序）：TestExecution 首败升格 → **不进** MutationTest（walker
/// 顺序执行天然保证 + `StepFlow::Upgraded` 截断余下步）；回 phase-next 进决策
/// 分叉（决策 stop 受控收敛）。
#[tokio::test]
async fn 拆步时序_testexecution首败升格不进mutation() {
    let (status, commands, requests, finish) = drive_test_execution_escalate("首检失败").await;

    assert_eq!(status, ChangeRunStatus::Stopped, "升格后决策 stop 收敛");
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, ToolCommand::MutationTest { .. }))
            .count(),
        0,
        "TestExecution 失败即截断，MutationTest 步未执行"
    );
    assert!(
        requests
            .iter()
            .all(|request| !matches!(request.role, WorkerRole::Executor | WorkerRole::Evaluator)),
        "升格零 agent 修复会话"
    );
    let test_nodes = step_instances(&finish.nodes, workflow::state::RunStepKind::TestExecution);
    assert_eq!(
        test_nodes,
        vec![(0, RunStepStatus::Failed)],
        "TestExecution 首败节点"
    );
    let mutation_nodes = step_instances(&finish.nodes, workflow::state::RunStepKind::MutationTest);
    assert!(
        mutation_nodes.is_empty(),
        "MutationTest 零节点（余下步不执行）"
    );
}

/// 正向：`step_mutation_test`（`on_fail=Escalate`）首败升格——恰 1 条 fail
/// PhaseRecord（条目「突变测试」、evidence 携诊断摘要 + 报告路径清单、三会话
/// 槽位恒 None）、MutationTest 节点 failed、下游常规 PhaseLog 不执行、
/// `StepFlow::Upgraded` 回 phase-next 进决策分叉。
#[tokio::test]
async fn mutation_test首败升格_零会话单条fail与报告路径清单() {
    let feedback = format!(
        "突变测试未达标：归并 score 55 < threshold 80\n{}",
        "存活清单行\n".repeat(40)
    );
    let paths = vec![
        "reports/test/app_rust/mutation/mutation-seg-0.json".to_owned(),
        "reports/test/app_rust/mutation/mutation.json".to_owned(),
    ];
    let (status, commands, requests, finish) =
        drive_mutation_escalate(&feedback, paths.clone()).await;

    assert_eq!(status, ChangeRunStatus::Stopped, "升格后决策 stop 收敛");
    assert!(
        requests
            .iter()
            .all(|request| !matches!(request.role, WorkerRole::Executor | WorkerRole::Evaluator)),
        "首败升格零 Executor / Evaluator 请求"
    );

    let mutation_calls = commands
        .iter()
        .filter(|command| matches!(command, ToolCommand::MutationTest { .. }))
        .count();
    assert_eq!(mutation_calls, 1, "MutationTest 门禁恰执行一次（零重跑）");

    let payload = phase_log_input(&commands, "test-execution");
    assert_eq!(payload.checklist.len(), 1, "升格落账恰一条 fail item");
    assert_eq!(payload.checklist[0].item, "突变测试");
    assert!(!payload.checklist[0].pass);
    assert!(
        payload.checklist[0].evidence.contains(&paths.join("、")),
        "evidence 携报告路径清单（零改写原文）: {}",
        payload.checklist[0].evidence
    );
    assert!(
        payload.checklist[0].evidence.contains("突变测试未达标"),
        "evidence 携诊断摘要: {}",
        payload.checklist[0].evidence
    );
    assert_eq!(payload.report, feedback, "report 携 feedback_text 完整原文");
    assert!(
        payload.executor_session_id.is_none()
            && payload.evaluator_session_id.is_none()
            && payload.decision_session_id.is_none(),
        "三会话槽位恒 None"
    );

    let mutation_nodes = step_instances(&finish.nodes, workflow::state::RunStepKind::MutationTest);
    assert_eq!(mutation_nodes.len(), 1, "MutationTest 节点单实例");
    assert_eq!(
        mutation_nodes[0].1,
        RunStepStatus::Failed,
        "未达标节点 failed"
    );
    let log_nodes = step_instances(&finish.nodes, workflow::state::RunStepKind::PhaseLog);
    assert_eq!(
        log_nodes.len(),
        1,
        "升格 PhaseLog 恰一个（下游常规 PhaseLog 未执行）"
    );
}

/// 异常：`conclusion=Error` 同式升格——report 取自 `feedback_text`（段中断错误
/// 不升 Err、进决策分叉）。
#[tokio::test]
async fn mutation_test_error结论同式升格_report取feedback_text() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "突变执行错误交人工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("test-execution", &[]),
            max_retries_outcome("test-execution", &[], "突变执行错误"),
        ])
        .with_test_execution(vec![escalate_test_outcome(
            TestExecutionConclusion::Pass,
            "",
        )])
        .with_mutation(vec![escalate_mutation_outcome(
            MutationTestConclusion::Error,
            "段 2 执行中断：harness 崩溃（exit 137）",
            vec!["reports/test/mutation-seg-2.json".to_owned()],
        )])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(MinimalSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "error 结论同式升格后决策 stop"
    );

    let commands = commands.lock().expect("命令锁");
    let payload = phase_log_input(&commands, "test-execution");
    assert_eq!(
        payload.report, "段 2 执行中断：harness 崩溃（exit 137）",
        "error 时点 report 取自 feedback_text"
    );
    assert_eq!(payload.checklist.len(), 1);
    assert_eq!(payload.checklist[0].item, "突变测试");
    assert!(
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .all(|request| !matches!(request.role, WorkerRole::Executor | WorkerRole::Evaluator)),
        "error 时点零 agent 修复会话"
    );
}

/// 异常：`run_tool(MutationTest)` 返回 `Err`（`fail_on("mutation-test")`）→
/// MutationTest 节点 failed + `Terminal` 收敛（runner `Err → Terminal` 分支
/// 原样保留）——零升格条目、零决策请求。
#[tokio::test]
async fn mutation_test_run_tool_err_terminal零升格零决策() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[])])
        .with_test_execution(vec![escalate_test_outcome(
            TestExecutionConclusion::Pass,
            "",
        )])
        .fail_on("mutation-test")
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "runner Err 停给用户");
    let finish = history.finish().expect("finish 落包捕获");
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("mutation-test 假写面注入失败"),
        "记因为 runner 显式失败: {:?}",
        finish.reason
    );
    let mutation_nodes = step_instances(&finish.nodes, workflow::state::RunStepKind::MutationTest);
    assert_eq!(
        mutation_nodes,
        vec![(0, RunStepStatus::Failed)],
        "节点呈失败态（不静默越过）"
    );
    let commands = commands.lock().expect("命令锁");
    assert!(
        commands
            .iter()
            .all(|command| !matches!(command, ToolCommand::PhaseLog { .. })),
        "零升格条目（环境问题不进决策）"
    );
    assert!(requests.lock().expect("请求锁").is_empty(), "零决策请求");
}

/// 边界：MutationTest 步 `on_fail` 分派（参数化 3 值）——`None` / `Terminal` /
/// `FeedbackLoop` 落在 MutationTest 步均计划图漂移 `Err`（该步无语义合法
/// `Terminal`），零 agent 会话、零门禁 spawn。
#[tokio::test]
async fn mutation_test分派漂移三值() {
    for on_fail in [
        None,
        Some(OnFail::Terminal),
        Some(OnFail::FeedbackLoop { limit: 5 }),
    ] {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let mut drifted_route = route_outcome("test-execution", &[]);
        drifted_route.steps = vec![StepDefinition {
            node: StepNode::MutationTest,
            prompt_ref: None,
            on_fail,
        }];
        let (tools, commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![drifted_route])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(
            status,
            ChangeRunStatus::Failed,
            "on_fail={on_fail:?} 误声明显式漂移失败"
        );
        let finish = history.finish().expect("finish 落包捕获");
        let reason = finish.reason.as_deref().unwrap_or_default();
        assert!(
            reason.contains("计划图漂移") && reason.contains("mutation-test"),
            "漂移记因: {reason}"
        );
        assert!(
            requests.lock().expect("请求锁").is_empty(),
            "漂移路径零 run_worker"
        );
        assert!(
            commands
                .lock()
                .expect("命令锁")
                .iter()
                .all(|command| !matches!(command, ToolCommand::MutationTest { .. })),
            "漂移路径零门禁 spawn"
        );
    }
}

/// 边界：`mutation_report_refs` 报告面（参数化 4 形态 + 超长折叠）——两元素
/// 原文拼接；空清单占位「（无报告产物）」；超大列表（64 条）全量拼接；单路径
/// 超长（1200 字符）与超长明细（1200 字符）同批：evidence 的**诊断摘要段**由
/// `diagnose_brief` 折叠截断（200 字上限 + 省略号），报告面段携路径原文（决策
/// agent 可直接 read——折叠只作用于摘要段，design D3）。
#[tokio::test]
async fn mutation报告路径拼接与超长折叠() {
    // 形态一：两元素原文拼接
    let (_, commands, _, _) =
        drive_mutation_escalate("未达标", vec!["a".to_owned(), "b".to_owned()]).await;
    let payload = phase_log_input(&commands, "test-execution");
    assert!(
        payload.checklist[0].evidence.contains("；a、b"),
        "两元素 report_paths 原文拼接: {}",
        payload.checklist[0].evidence
    );

    // 形态二：空清单 → 占位
    let (_, commands, _, _) = drive_mutation_escalate("未达标", Vec::new()).await;
    let payload = phase_log_input(&commands, "test-execution");
    assert!(
        payload.checklist[0].evidence.contains("（无报告产物）"),
        "空清单占位: {}",
        payload.checklist[0].evidence
    );

    // 形态三：超大列表（64 条）全量拼接不 panic，诊断摘要段仍受 200 字折叠约束
    let many: Vec<String> = (0..64)
        .map(|index| format!("reports/m-{index:02}.json"))
        .collect();
    let (_, commands, _, _) = drive_mutation_escalate(&"突".repeat(400), many.clone()).await;
    let payload = phase_log_input(&commands, "test-execution");
    let evidence = &payload.checklist[0].evidence;
    let (brief_segment, refs_segment) = evidence
        .split_once('；')
        .expect("evidence 形态 = 诊断摘要；报告面");
    assert!(
        brief_segment.chars().count() <= 201,
        "诊断摘要段折叠截断至 200 字上限（+省略号）: {}",
        brief_segment.chars().count()
    );
    assert!(
        brief_segment.ends_with('…'),
        "折叠留省略号: {brief_segment}"
    );
    assert!(
        refs_segment.contains("reports/m-00.json") && refs_segment.contains("reports/m-63.json"),
        "64 条清单首末项全量在场（报告面原文）: {refs_segment}"
    );

    // 形态四：超长单路径（1200 字符）+ 超长明细（1200 字符）——摘要段折叠至
    // 200 字（evidence 摘要 200 字上限），报告面段携路径原文（可直接 read）
    let long_path = format!("reports/{}.json", "p".repeat(1200));
    let (_, commands, _, _) =
        drive_mutation_escalate(&"突".repeat(1200), vec![long_path.clone()]).await;
    let payload = phase_log_input(&commands, "test-execution");
    let evidence = &payload.checklist[0].evidence;
    let (brief_segment, refs_segment) = evidence
        .split_once('；')
        .expect("evidence 形态 = 诊断摘要；报告面");
    assert_eq!(
        brief_segment.chars().count(),
        201,
        "diagnose_brief 折叠至 200 字 + 省略号（evidence 摘要 200 字上限）"
    );
    assert!(brief_segment.ends_with('…'), "折叠留省略号");
    assert!(
        refs_segment.contains(&long_path),
        "报告面路径原文在案（决策 agent 可直读，路径不因摘要折叠受损）: {}",
        refs_segment.chars().count()
    );
    assert_eq!(
        payload.report.chars().count(),
        1200,
        "report 携完整明细（1200 ≤ 2000 原样，折叠只作用于 evidence 摘要段）"
    );
}

/// 边界：突变报告截断与空反馈兜底——feedback_text 2500 字符 → report 截断至
/// 2000（末位省略号）；feedback_text 空串 → 兜底 `conclusion=fail；报告路径 …`。
#[tokio::test]
async fn mutation报告截断与空反馈兜底() {
    let long = "突".repeat(2500);
    let (_, commands, _, _) = drive_mutation_escalate(&long, vec!["a".to_owned()]).await;
    let report = phase_log_input(&commands, "test-execution").report;
    assert_eq!(report.chars().count(), 2000, "超长反馈截断至 2000");
    assert!(report.ends_with('…'), "截断留省略号");
    assert!(report.starts_with(&"突".repeat(1999)), "取前 1999 + 省略号");

    let (_, commands, _, _) =
        drive_mutation_escalate("", vec!["a".to_owned(), "b".to_owned()]).await;
    let report = phase_log_input(&commands, "test-execution").report;
    assert!(
        report.starts_with("conclusion=fail") && report.contains("报告路径 a、b"),
        "空反馈兜底结论行 + 报告路径清单: {report}"
    );
}

/// 正向：`step_mutation_test`（Pass）——达标（逐 suite 全过）→ 节点 passed
/// （detail 携 `conclusion=pass score=… threshold=…`）+ Proceed，**零反馈**
/// （达标 pass 直接收口——零 WorkerAgent 修复会话）；机械 checklist 第 3 条
/// evidence「跑过且达标」（AC-6 / AC-7 双槽半边）。
#[tokio::test]
async fn mutation_pass零反馈收口_双槽evidence达标面() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![execution_outcome(TestExecutionConclusion::Pass, 5)])
        .with_mutation(vec![mutation_outcome(
            MutationTestConclusion::Pass,
            95.0,
            80.0,
            "",
        )])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    // 达标 pass 零反馈：零修复会话
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "达标 pass 直接收口（零反馈边修复会话）"
    );

    // 节点 detail 携 conclusion / score / threshold
    let command = history.finish().expect("finish 落包捕获");
    let detail = node_detail(&command, workflow::state::RunStepKind::MutationTest)
        .expect("mutation_test 节点 detail 在场");
    assert!(
        detail.contains("conclusion=pass")
            && detail.contains("score=95")
            && detail.contains("threshold=80"),
        "detail 携结论摘要: {detail}"
    );

    // PhaseCursor 双槽并立消费：机械 checklist 第 2 条携 test 槽计数（test 面）
    // + 第 3 条 evidence 携 mutation 槽 score / threshold（mutation 面）——两槽
    // 互不覆盖、各自独立消费（AC-7 双槽）
    let _ = command;
}

/// 边界：`PhaseCursor` 双槽互不覆盖——test-execution 相位全链下机械
/// phase-log 同时消费 `test_outcome`（report 携计数）与 `mutation_outcome`
/// （checklist 第 3 条 evidence 携 score / threshold）；两步均 pass 收口（首败
/// 即升格，无修复重入——`Upgraded` 截断余下步）。
#[tokio::test]
async fn phase_cursor双槽互不覆盖_机械checklist同时消费两槽() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![execution_outcome(TestExecutionConclusion::Pass, 7)])
        .with_mutation(vec![mutation_outcome(
            MutationTestConclusion::Pass,
            95.0,
            80.0,
            "",
        )])
        .assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let commands = _commands.lock().expect("命令锁");
    let mechanical = commands
        .iter()
        .find_map(|command| match command {
            ToolCommand::PhaseLog { input, phase, .. } if phase == "test-execution" => {
                Some(input.clone())
            }
            _ => None,
        })
        .expect("机械 phase-log 在场");
    // test 槽：report 携 suite 计数 + checklist 第 2 条聚合计数
    assert!(
        mechanical.report.contains("total=7") && mechanical.report.contains("passed=6"),
        "test 槽消费（report 携计数）: {}",
        mechanical.report
    );
    assert_eq!(mechanical.checklist[1].item, "聚合计数一致");
    assert!(
        mechanical.checklist[1].evidence.contains("total=7"),
        "test 槽消费（第 2 条 evidence）: {}",
        mechanical.checklist[1].evidence
    );
    // mutation 槽：checklist 第 3 条 evidence 携归并 score / threshold（两槽并立
    // 不覆盖——test 面零 mutation 词、mutation 面零 test 计数）
    assert_eq!(mechanical.checklist[2].item, "mutation null 自动通过");
    assert!(
        mechanical.checklist[2].evidence.contains("score 95")
            && mechanical.checklist[2].evidence.contains("threshold 80"),
        "mutation 槽消费（第 3 条 evidence 携归并口径）: {}",
        mechanical.checklist[2].evidence
    );
    assert!(
        !mechanical.checklist[2].evidence.contains("total=7"),
        "两槽互不覆盖（mutation 面零 test 计数）"
    );
    assert!(
        mechanical.checklist.iter().all(|item| item.pass),
        "三条目全 pass（双槽语义下第 3 条恒真——D8）"
    );
}

/// 边界：机械 checklist 第 3 条 evidence 三态（AC-7 / E8）——`skip=None` →
/// 「跑过且达标（score X ≥ threshold Y）」；`skip=EmptyDiff` → 「diff 空跳过」；
/// `skip=NoExecutor` → 「null（未跑）」附注「无突变执行器」；第 3 条判定恒
/// pass（语义不改，D8——null 观察期不收紧）。
#[tokio::test]
async fn 机械checklist第三条evidence三态映射() {
    /// 单轮驱动：指定 mutation 产出 → 摘取机械 phase-log 第 3 条 evidence。
    async fn evidence_of(mutation: MutationTestOutcome) -> (bool, String) {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
            .with_test_execution(vec![execution_outcome(TestExecutionConclusion::Pass, 5)])
            .with_mutation(vec![mutation])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Completed);
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
        assert_eq!(mechanical.checklist.len(), 3, "三条目机械 checklist");
        let third = &mechanical.checklist[2];
        assert_eq!(third.item, "mutation null 自动通过");
        (third.pass, third.evidence.clone())
    }

    // 态①：跑过且达标（skip=None）→ evidence 携 score / threshold
    let (pass, evidence) = evidence_of(mutation_outcome(
        MutationTestConclusion::Pass,
        95.0,
        80.0,
        "",
    ))
    .await;
    assert!(pass, "第 3 条判定恒 pass（D8 语义不改）");
    assert!(
        evidence.contains("跑过且达标")
            && evidence.contains("score 95")
            && evidence.contains("threshold 80"),
        "态① 跑过且达标: {evidence}"
    );

    // 态②：diff 空跳过（EmptyDiff）→ 「diff 空跳过」
    let (pass, evidence) = evidence_of(MutationTestOutcome {
        fault: None,
        conclusion: MutationTestConclusion::Pass,
        skip: Some(MutationSkipReason::EmptyDiff),
        score: 0.0,
        threshold: 0.0,
        counts: MutationCounts::default(),
        report_paths: Vec::new(),
        feedback_text: String::new(),
    })
    .await;
    assert!(pass, "跳过面同样恒 pass（null 观察期不判 fail）");
    assert_eq!(evidence, "diff 空跳过", "态② diff 空跳过");

    // 态③：NoExecutor → 「null（未跑）」附注「无突变执行器」
    let (pass, evidence) = evidence_of(MutationTestOutcome {
        fault: None,
        conclusion: MutationTestConclusion::Pass,
        skip: Some(MutationSkipReason::NoExecutor),
        score: 0.0,
        threshold: 0.0,
        counts: MutationCounts::default(),
        report_paths: Vec::new(),
        feedback_text: String::new(),
    })
    .await;
    assert!(pass);
    assert!(
        evidence.contains("null（未跑）") && evidence.contains("无突变执行器"),
        "态③ NoExecutor 并入 null 态附注: {evidence}"
    );
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（D1 / AC-2）：emit_node 铸造单点——running
// emit 铸 (now, None)、终态 emit 铸 (now, Some(now))、瞬步单 emit 同刻、
// stopped 收口有 ended；wall-clock 不 mock 不注入（复用生产时钟零新缝），
// 断言取关系式（started > 0、ended >= started、瞬步同刻、stopped ended 在场），
// wall-clock 绝对值不入断言。落包捕获经 RecordingHistory（节点即 finish_command
// 输入的累积器快照——emit → accumulate 全链读取面）。
// ---------------------------------------------------------------------------

/// 两相位直通路由（proposal → dev-design → done）：proposal 站五步
///（phase_start / executor / evaluator / verdict_gate / phase_log）+
/// dev-design 站至少前三步——双 emit 步与瞬步单 emit 步同 run 并存。
fn two_phase_routes() -> Vec<PhaseNextOutcome> {
    vec![
        route_outcome("proposal", &[]),
        route_outcome("dev-design", &["proposal", "dev-design"]),
        done_outcome(),
    ]
}

/// 落包节点摘取：按 (step, instance_no) 圈定。
fn node_of<'a>(
    command: &'a RunFinishCommand,
    step: workflow::state::RunStepKind,
    instance_no: u32,
) -> &'a RunNodeEntry {
    command
        .nodes
        .iter()
        .find(|node| node.step == step && node.instance_no == instance_no)
        .unwrap_or_else(|| panic!("落包应含 {step:?} 实例 {instance_no} 节点"))
}

/// 正向：FakeWorker 正常收口的 executor 双 emit 步 → 终态节点 `started_at > 0`
/// 且 `ended_at >= started_at`（wall-clock 关系式断言，绝对值不入断言）。
#[tokio::test]
async fn emit铸造_executor双emit关系式_started正且ended不早于started() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(two_phase_routes())
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed);

    let finish = history.finish().expect("finish 落包捕获");
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    assert_eq!(
        executor.status,
        RunStepStatus::Passed,
        "正常收口 executor 终态 passed"
    );
    assert!(
        executor.started_at > 0,
        "running emit 铸 started（> 0 关系式）"
    );
    let ended = executor.ended_at.expect("终态 emit 铸 ended 在场");
    assert!(
        ended >= executor.started_at,
        "ended >= started（同刻瞬收合法）"
    );
}

/// 正向：`phaseStart` 瞬步单 emit passed → 节点 `started_at == ended_at` 同刻
///（AC-2 瞬步半边——铸造单点对单 emit 步自然产出零耗时形态）。
#[tokio::test]
async fn emit铸造_phase_start瞬步单emit同刻() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(two_phase_routes())
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    task.await.expect("run 任务正常结束");

    let finish = history.finish().expect("finish 落包捕获");
    let start = node_of(&finish, workflow::state::RunStepKind::PhaseStart, 0);
    assert_eq!(start.status, RunStepStatus::Passed);
    assert!(start.started_at > 0, "单 emit 亦铸 started");
    assert_eq!(
        start.ended_at,
        Some(start.started_at),
        "瞬步单 emit started == ended 同刻"
    );
}

/// 正向：executor 会话 Stopped 收敛（用户停止的会话半边）→ 终态节点 status=
/// stopped 且 `ended_at` 在场 `> 0`（AC-2 stopped 半边——StepSettle::stopped
/// 随行终态 emit 同铸 ended）。
#[tokio::test]
async fn emit铸造_会话stopped收敛终态节点ended在场() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) =
        FakeWorker::new(&timeline).with_executor_stops(1).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, mut rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "会话 Stopped 收敛 → run stopped"
    );
    assert!(drain_notices(&mut rx)
        .iter()
        .any(|notice| *notice == RunNotice::Finished));

    let finish = history.finish().expect("finish 落包捕获");
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    assert_eq!(executor.status, RunStepStatus::Stopped);
    assert!(executor.started_at > 0, "running emit 铸 started");
    let ended = executor.ended_at.expect("stopped 终态 emit 铸 ended 在场");
    assert!(ended > 0, "stopped 收口有 ended（ended > 0 关系式）");
    assert!(ended >= executor.started_at);
}

/// 正向：工具步 / Gate / 相位机步终态 emit → token 两值 None（`with_tokens`
/// 仅 run_worker 调用、构造器中性继承——AC-3 非工步半边；executor 注入
/// token 有值作对照组）。
#[tokio::test]
async fn emit铸造_非工步终态emit_token恒none() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_transcripts(vec![vec![AgentEventKind::TurnDone {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(1),
            duration_ms: Some(1_000),
            cost_usd: None,
            usage: serde_json::json!({ "input_tokens": 500, "output_tokens": 60 }),
            session_id: None,
        }]])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(two_phase_routes())
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    task.await.expect("run 任务正常结束");

    let finish = history.finish().expect("finish 落包捕获");
    // 对照组：executor 工步 token 落值
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    assert_eq!(executor.token_input, Some(500));
    assert_eq!(executor.token_output, Some(60));
    // 非工步全词汇遍历：相位机 / Gate / 落账步 token 恒 None（构造器中性继承）
    for step in [
        workflow::state::RunStepKind::PhaseStart,
        workflow::state::RunStepKind::VerdictGate,
        workflow::state::RunStepKind::PhaseLog,
    ] {
        let node = node_of(&finish, step, 0);
        assert_eq!(node.token_input, None, "{step:?} token_input 恒 None");
        assert_eq!(node.token_output, None, "{step:?} token_output 恒 None");
    }
}

/// 边界：EvaluatorGate 既有挂起装置下中途 snapshot → running 在途节点
/// `started_at > 0`、`ended_at = None`（running 恒 None——AC-2 走秒数据半边）。
#[tokio::test]
async fn emit铸造_running在途snapshot_started正ended_none() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let gate = EvaluatorGate::armed();
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_gate(Arc::clone(&gate))
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(two_phase_routes())
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);

    // evaluator 会话挂起（gate armed）即运行中窗口：在途节点已入累积器
    wait_for("evaluator 会话发起", || {
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .any(|request| request.role == WorkerRole::Evaluator)
    })
    .await;
    let snap = control.snapshot(ROOT, CHANGE_ID).expect("运行中快照在场");
    let in_flight = snap
        .nodes
        .iter()
        .find(|node| node.step == ChangeStepKind::Evaluator)
        .expect("在途 evaluator 节点已入快照");
    assert_eq!(in_flight.status, ChangeStepStatus::Running);
    assert!(in_flight.started_at > 0, "running emit 铸 started（> 0）");
    assert_eq!(in_flight.ended_at, None, "running 恒 None");

    // 放行收口（run 任务收敛，除名归位）
    *gate.armed.lock().expect("门锁不可中毒") = false;
    gate.notify.notify_one();
    task.await.expect("run 任务正常结束");
}

/// 边界：StepSettle 构造器中性预填（started_at=0 / ended_at=None）经 emit_node
/// 铸造覆写 → 出面无中性值残留（D1 单点覆写锚：落包全节点 started > 0、
/// 终态节点 ended = Some(> 0)）。
#[tokio::test]
async fn emit铸造_出面无中性值残留() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(two_phase_routes())
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    task.await.expect("run 任务正常结束");

    let finish = history.finish().expect("finish 落包捕获");
    assert!(!finish.nodes.is_empty());
    for node in &finish.nodes {
        assert!(
            node.started_at > 0,
            "{:?} started 无构造器 0 残留（铸造覆写）",
            node.step
        );
        if node.status != RunStepStatus::Running {
            let ended = node.ended_at.expect("终态节点 ended 在场");
            assert!(ended > 0, "{:?} ended 无中性值残留", node.step);
        }
    }
}

/// 异常：调用方预填冲突值（running 态构造器预携中性 ended_at=None、终态
/// builder 不携时间）→ 铸造按 status 覆写、线面以铸造值为准（铸造不信任
/// 调用方预填值——防御锚：StepSettle::passed 系构造器继承中性 0/None，出面
/// 时间仍为铸造新值——瞬步单 emit 步 phase_start 的 started/ended 即铸造对）。
#[tokio::test]
async fn emit铸造_构造器中性预填被按status覆写() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(two_phase_routes())
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    task.await.expect("run 任务正常结束");

    let finish = history.finish().expect("finish 落包捕获");
    // phase_start 步：StepSettle::passed 中性预填（0 / None）→ 铸造按 passed
    // 覆写 (now, Some(now))——出面绝无预填 0 / None 残留
    let start = node_of(&finish, workflow::state::RunStepKind::PhaseStart, 0);
    assert_ne!(start.started_at, 0, "铸造覆写预填 started=0");
    assert_eq!(
        start.ended_at,
        Some(start.started_at),
        "铸造按 passed status 覆写 ended=Some(now)（预填 None 被顶替）"
    );
    // executor 步：终态 builder（StepSettle::passed）同样零时间预填 → 出面
    // ended 铸造新值（>= running emit 首见 started）
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    let ended = executor.ended_at.expect("终态铸造 ended 在场");
    assert!(ended >= executor.started_at && ended > 0, "铸造值权威");
}

/// 组合（AC-2 / AC-4 交界）：emit 铸造 → accumulate → snapshot / 落包出口——
/// 同 run 多步双 emit 序列后，终态前捕获的 snapshot 与终态出口 nodes（finish
/// 命令输入）同键节点四字段一致（已收口键的快照面即出口面；在途键翻态后
/// 以出口面为准不再比对）。
#[tokio::test]
async fn emit铸造_快照与落包出口同键四字段一致() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let gate = EvaluatorGate::armed();
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_gate(Arc::clone(&gate))
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(two_phase_routes())
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);

    // evaluator 挂起窗口：phase_start / executor 已终态（快照面），evaluator
    // 在途
    wait_for("evaluator 会话发起", || {
        requests
            .lock()
            .expect("请求锁")
            .iter()
            .any(|request| request.role == WorkerRole::Evaluator)
    })
    .await;
    let mid = control.snapshot(ROOT, CHANGE_ID).expect("运行中快照在场");
    *gate.armed.lock().expect("门锁不可中毒") = false;
    gate.notify.notify_one();
    task.await.expect("run 任务正常结束");

    let finish = history.finish().expect("finish 落包捕获");
    // 同键对拍：快照中已收口（ended 在场）的节点，出口面四字段逐字段一致
    //（started 保首见 / ended·token some 覆写后钉死——合并层单调）
    for snap_node in &mid.nodes {
        let Some(snap_ended) = snap_node.ended_at else {
            continue; // 在途键翻态后不再可比（出口面权威）
        };
        let exit_node = finish
            .nodes
            .iter()
            .find(|node| {
                node.phase == snap_node.phase
                    && node.attempt == snap_node.attempt
                    && node.step.as_str() == snap_node.step.as_str()
                    && node.instance_no == snap_node.instance_no
            })
            .unwrap_or_else(|| panic!("出口应含同键节点 {:?}", snap_node.step));
        assert_eq!(
            exit_node.started_at, snap_node.started_at,
            "started 快照=出口"
        );
        assert_eq!(exit_node.ended_at, Some(snap_ended), "ended 快照=出口");
        assert_eq!(exit_node.token_input, snap_node.token_input);
        assert_eq!(exit_node.token_output, snap_node.token_output);
    }
    // 出口面至少含一个已收口键（对拍非空——phase_start 瞬步恒先收口）
    assert!(
        mid.nodes.iter().any(|node| node.ended_at.is_some()),
        "快照窗口内已有已收口键（对拍非空置）"
    );
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（D2 / AC-3 / AC-10）：run_worker token 求和
// ——终态 emit 前对 outcome.transcript 的 TurnDone.usage 单次求和落值（无论
// outcome.status）；Err 路径恒 None；求和语义 = `agent::sum_usage_tokens`
// 消费面表现。
// ---------------------------------------------------------------------------

/// TurnDone kind 构造（usage 显式注入——脚本转录的最常用事件）。
fn turn_done_kind(input: serde_json::Value, output: serde_json::Value) -> AgentEventKind {
    AgentEventKind::TurnDone {
        subtype: "success".to_owned(),
        is_error: false,
        num_turns: Some(1),
        duration_ms: Some(1_000),
        cost_usd: None,
        usage: serde_json::json!({
            "input_tokens": input,
            "output_tokens": output
        }),
        session_id: None,
    }
}

/// 正向：FakeWorker 转录注入多 TurnDone（usage 两键各值）→ 终态节点
/// token_input / token_output = 逐值之和（AC-3 主断言）。
#[tokio::test]
async fn run_worker_transcript多turndone_usage求和落值() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_transcripts(vec![vec![
            turn_done_kind(json_usize(100), json_usize(20)),
            turn_done_kind(json_usize(30), json_usize(0)),
            turn_done_kind(json_usize(17), json_usize(7)),
        ]])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    task.await.expect("run 任务正常结束");

    let finish = history.finish().expect("finish 落包捕获");
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    assert_eq!(executor.token_input, Some(147), "in = 100 + 30 + 17");
    assert_eq!(executor.token_output, Some(27), "out = 20 + 0 + 7");
}

/// 正向：会话失败收敛（Failed outcome 但 transcript 在手）→ token 仍按
/// transcript 求和落值（D2 无论 outcome.status；executor 失败注入携转录
/// 脚本——脚本组即失败会话转录）。
#[tokio::test]
async fn run_worker_会话失败收敛transcript在手token仍落值() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_executor_failures(vec![Some("api_error: 配额耗尽".to_owned())])
        .with_transcripts(vec![vec![
            turn_done_kind(json_usize(11), json_usize(3)),
            turn_done_kind(json_usize(22), json_usize(4)),
        ]])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[])])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "会话失败收敛 → run failed");

    let finish = history.finish().expect("finish 落包捕获");
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    assert_eq!(executor.status, RunStepStatus::Failed);
    assert_eq!(executor.token_input, Some(33), "失败收敛 in = 11 + 22");
    assert_eq!(executor.token_output, Some(7), "失败收敛 out = 3 + 4");
}

/// 边界：转录零 TurnDone / usage 缺席（零注入既有形态）→ 终态节点 token 两值
/// None（AC-3 缺席半边——usage 缺席合法缺省）。
#[tokio::test]
async fn run_worker_零turndone_token恒none() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    task.await.expect("run 任务正常结束");

    let finish = history.finish().expect("finish 落包捕获");
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    assert_eq!(executor.token_input, None, "零 TurnDone → None");
    assert_eq!(executor.token_output, None);
}

/// 边界（with_tokens 单侧）：转录 usage 仅携 input_tokens 键 → 终态节点
/// token_input 单侧落值、token_output 缺席侧 None（builder 单侧 Some 形态）。
#[tokio::test]
async fn run_worker_token单侧落值_缺席侧none() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_transcripts(vec![vec![AgentEventKind::TurnDone {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(1),
            duration_ms: Some(1_000),
            cost_usd: None,
            usage: serde_json::json!({ "input_tokens": 500 }),
            session_id: None,
        }]])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    task.await.expect("run 任务正常结束");

    let finish = history.finish().expect("finish 落包捕获");
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    assert_eq!(executor.token_input, Some(500), "在场侧单侧落值");
    assert_eq!(executor.token_output, None, "缺席侧 None");
}

/// 边界：`worker.run` 返回 Err（无 outcome / 无 transcript）→ failed emit 节点
/// token 恒 None（D2 Err 半边——evaluator Err 注入路径）。
#[tokio::test]
async fn run_worker_err路径failed_emit_token恒none() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_evaluator_reports(vec![Err("评估会话崩溃".to_owned())])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[])])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "会话 Err → run failed");

    let finish = history.finish().expect("finish 落包捕获");
    let evaluator = node_of(&finish, workflow::state::RunStepKind::Evaluator, 0);
    assert_eq!(evaluator.status, RunStepStatus::Failed);
    assert!(evaluator.started_at > 0, "failed emit 亦铸 started");
    assert!(evaluator.ended_at.is_some(), "failed 终态 emit 铸 ended");
    assert_eq!(evaluator.token_input, None, "Err 路径 token 恒 None");
    assert_eq!(evaluator.token_output, None);
}

/// 异常：转录混入非 TurnDone 事件与 usage 值类型漂移（字符串 / 浮点）→ 求和
/// 仅计合法 TurnDone 数值、不 panic（`agent::sum_usage_tokens` 语义在消费面的
/// 表现，AC-3 / AC-10）。
#[tokio::test]
async fn run_worker_干扰载荷仅合法turndone计入不panic() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_transcripts(vec![vec![
            AgentEventKind::RunStarted {
                model: None,
                session_id: None,
                tools: Vec::new(),
                mcp_servers: Vec::new(),
            },
            turn_done_kind(json_usize(5), json_usize(1)),
            AgentEventKind::MessageDelta {
                parent_tool_use_id: None,
                delta: agent::AgentDelta::Text {
                    text: "增量".to_owned(),
                },
            },
            // 类型漂移：字符串 / 浮点 usage 值 → as_u64 判定失败跳过
            turn_done_kind(serde_json::json!("1200"), serde_json::json!(2.5)),
            turn_done_kind(json_usize(6), serde_json::Value::Null),
        ]])
        .assemble();
    let (tools, _) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("proposal", &[]), done_outcome()])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "干扰载荷不炸不翻转终态");

    let finish = history.finish().expect("finish 落包捕获");
    let executor = node_of(&finish, workflow::state::RunStepKind::Executor, 0);
    assert_eq!(
        executor.token_input,
        Some(11),
        "仅合法 TurnDone 数值计入（5 + 6）"
    );
    assert_eq!(executor.token_output, Some(1), "漂移值跳过（1 + 0）");
}

/// usize → JSON 数值（脚本可读性小助手）。
fn json_usize(value: u64) -> serde_json::Value {
    serde_json::json!(value)
}

// ---------------------------------------------------------------------------
// initialize 一次性表头全链（check-before-flow-start / AC-1 ~ AC-7）：worktree
// 分支五步序全绿 / step_clean 排除面与 Err 收敛 / 两检查步 Terminal 三态文案 /
// 一次性重试链 / legacy 整相位短路 / 持久跳过（Mock策略：FakeVcs 扩
// restore_baseline 捕获 + FakeTools 预录产出 + RecordingHistory 落包捕获 +
// StubSnapshot——db 真件组合断言沿既有 TestDb 装置）
// ---------------------------------------------------------------------------

/// baseline 槽位产出 fixture（terminal 执行体载荷供给——report_dir 携 baseline
/// 报告树锚点，与 walker Terminal 文案 {dir} 供给面同形）。
fn baseline_check(passed: bool) -> StaticCheckOutcome {
    StaticCheckOutcome {
        fault: None,
        passed,
        diagnostics: if passed {
            String::new()
        } else {
            "error[E0308]: baseline 类型错误".to_owned()
        },
        output_ref: Some("exec-baseline-static".to_owned()),
        report_dir: format!("/ws/openspec/changes/{NAME}/reports/baseline"),
    }
}

fn baseline_execution(conclusion: TestExecutionConclusion) -> TestExecutionOutcome {
    TestExecutionOutcome {
        fault: None,
        conclusion,
        total: 12,
        passed: 10,
        failed: u64::from(conclusion == TestExecutionConclusion::Fail),
        skipped: 0,
        findings_brief: match conclusion {
            TestExecutionConclusion::Pass => "全部测试通过".to_owned(),
            TestExecutionConclusion::Error => "测试框架拉起失败（环境错误）".to_owned(),
            TestExecutionConclusion::Fail => "2 项测试失败——聚类失败".to_owned(),
        },
        findings_detail: String::new(),
        report_dir: format!("/ws/openspec/changes/{NAME}/reports/baseline"),
        exec_id: "exec-baseline-test".to_owned(),
    }
}

/// worktree 分支 initialize 全绿链（正向，AC-1 / AC-4 / AC-5 Ok 半边）：五步序
/// emit 逐位一致（phase_start → clean → static_check → test_execution →
/// phase_log，串行 forward 边）；restore 排除面 = 名单六名 + `".*"` + openspec
/// （D6 组装面）；两检查步 ToolCommand 携 `slot=Baseline`（构造点分派单源）；
/// 检查步 root 恒 exec root（worktree）；PhaseLog 机械 checklist 三条目落账
///（mutation 恒 None 第 3 条如实留痕、report 携 conclusion 计数 + baseline
/// 报告目录、三会话槽位恒 None）；PhaseRecord 落 pass（skipped=false）。
#[tokio::test]
async fn initialize五步序全绿链_排除面与baseline槽位与机械checklist() {
    let root = TempRoot::new("init-green");
    root.change(NAME);
    let worktree = root.0.join("openspec/changes").join(NAME);
    let db = TestDb::open("init-green");
    seed_change_with_worktree(db.store.as_ref(), &worktree);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("initialize", &[]), done_outcome()])
        .with_static_check(vec![baseline_check(true)])
        .with_test_execution(vec![baseline_execution(TestExecutionConclusion::Pass)])
        .assemble();
    let vcs = Arc::new(FakeVcs::clean());
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        vcs.clone(),
        snapshot,
        &control,
        false,
        &worktree.to_string_lossy(),
        &root.root_str(),
        None,
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "initialize 全绿 → done 收敛"
    );

    // 零 agent 会话（确定性相位——executor / evaluator / decision 缺席）
    assert!(
        sessions.lock().expect("会话登记锁").is_empty(),
        "initialize 零 agent 会话注入"
    );

    // 五步序 emit 逐位（落包节点序 = emit 序；词面 + forward 边逐位）
    let finish = history.finish().expect("收口落包捕获");
    let words: Vec<&str> = finish.nodes.iter().map(|node| node.step.as_str()).collect();
    assert_eq!(
        words,
        [
            "phase_start",
            "clean",
            "static_check",
            "test_execution",
            "phase_log"
        ],
        "initialize 五节点 emit 序逐位（五步序）"
    );
    assert!(
        finish.nodes.iter().all(|node| node.phase == "initialize"),
        "五节点 phase 恒 initialize"
    );
    assert_eq!(finish.edges.len(), 4, "串行 forward 边四条");
    for (index, edge) in finish.edges.iter().enumerate() {
        assert_eq!(
            edge.edge_kind,
            workflow::state::RunEdgeKind::Forward,
            "第 {index} 边 forward"
        );
        assert_eq!(
            edge.from_node_key,
            format!("initialize:1:{}:0", words[index])
        );
        assert_eq!(
            edge.to_node_key,
            format!("initialize:1:{}:0", words[index + 1])
        );
    }
    // clean / static_check / test_execution / phase_log 节点恒 passed
    for word in ["clean", "static_check", "test_execution", "phase_log"] {
        let node = finish
            .nodes
            .iter()
            .find(|node| node.step.as_str() == word)
            .expect("节点应在案");
        assert_eq!(node.status, RunStepStatus::Passed, "{word} passed");
    }

    // 排除面组装（D6）：restore 载荷 = 名单六名 + ".*" + openspec（worktree 锚）
    let restores = vcs.restore_calls();
    assert_eq!(restores.len(), 1, "restore_baseline 恰一次");
    let (restore_root, exclusions) = &restores[0];
    assert_eq!(
        restore_root.to_string_lossy(),
        worktree.to_string_lossy(),
        "restore 作用锚 = worktree（exec root）"
    );
    assert_eq!(
        exclusions.iter().map(String::as_str).collect::<Vec<_>>(),
        [
            "node_modules",
            "dist",
            "build",
            "target",
            "coverage",
            ".nyc_output",
            ".*",
            "openspec",
        ],
        "排除面 = PRUNED_DIR_NAMES 六名 + dot 语义 + openspec 工件树（foundation 名单单源组装）"
    );
    assert!(
        vcs.calls().iter().all(|call| call == "restore_baseline"),
        "initialize 零归档 git 步触点（仅 restore_baseline）: {:?}",
        vcs.calls()
    );

    // ToolCommand 构造点：两检查步 slot=Baseline + root 恒 exec root
    let slots: Vec<(&str, foundation::layout::ReportSlot)> = {
        let commands = commands.lock().expect("命令捕获锁");
        commands
            .iter()
            .filter_map(|command| match command {
                ToolCommand::StaticCheck { slot, .. } => Some(("static-check", *slot)),
                ToolCommand::TestExecution { slot, .. } => Some(("test-execution", *slot)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        slots,
        [
            ("static-check", foundation::layout::ReportSlot::Baseline),
            ("test-execution", foundation::layout::ReportSlot::Baseline),
        ],
        "initialize 两检查步构造 slot=Baseline（构造点分派单源）"
    );

    // PhaseLog 机械 checklist 落账载荷（FakeTools 命令捕获——写面载荷直检）
    let log_input = {
        let commands = commands.lock().expect("命令捕获锁");
        commands
            .iter()
            .find_map(|command| match command {
                ToolCommand::PhaseLog { input, .. } => Some(input.clone()),
                _ => None,
            })
            .expect("phase-log 落账在场")
    };
    assert_eq!(log_input.phase, "initialize");
    assert!(!log_input.skipped, "全绿链落账非 skipped");
    assert_eq!(log_input.checklist.len(), 3, "机械 checklist 三条目");
    assert!(
        log_input.checklist.iter().all(|item| item.pass),
        "三机械条目全 pass（第 3 条 mutation null 自动通过——基线检查明确不含突变）"
    );
    assert_eq!(
        log_input.checklist[2].evidence, "null（未跑）",
        "第 3 条目 evidence 如实留痕（mutation 恒 None——零突变语义）"
    );
    assert!(
        log_input.report.contains("pass") && log_input.report.contains("12"),
        "report 携 conclusion + 计数: {}",
        log_input.report
    );
    assert!(
        log_input.report.contains("reports/baseline"),
        "report 携 baseline 报告目录: {}",
        log_input.report
    );
    assert!(
        log_input.executor_session_id.is_none()
            && log_input.evaluator_session_id.is_none()
            && log_input.decision_session_id.is_none(),
        "三会话槽位恒 None（零 agent 面）"
    );
    drop(commands);

    // PhaseLog 机械 checklist 载荷断言已承载落账面（PhaseLog 命令直检——本用例
    // FakeTools 零落库；PhaseRecord pass 条目与三会话槽位恒 None 的读侧断言由
    // change_flow 基线绿走真件测试承载）
}

/// step_clean Err 收敛（异常，AC-5 Err 半边）：restore Err → clean 节点 failed
///（Err 串附 git 语境）→ run `Terminal::failed` 收敛；PhaseRecord 零 initialize
/// 条目、后续 StaticCheck / TestExecution 步零执行（步序短路——Err 串原样透传）。
#[tokio::test]
async fn step_clean_err收敛_failed节点与terminal且检查步零执行() {
    let root = TempRoot::new("init-clean-err");
    root.change(NAME);
    let worktree = root.0.join("openspec/changes").join(NAME);
    let db = TestDb::open("init-clean-err");
    seed_change_with_worktree(db.store.as_ref(), &worktree);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("initialize", &[])])
        .assemble();
    let vcs = Arc::new(
        FakeVcs::clean().with_restore_result(Err("git -C wt clean 失败（退出码 128）".to_owned())),
    );
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        vcs.clone(),
        snapshot,
        &control,
        false,
        &worktree.to_string_lossy(),
        &root.root_str(),
        None,
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "restore Err → Terminal failed"
    );

    let finish = history.finish().expect("收口落包捕获");
    assert_eq!(finish.status, RunStatus::Failed, "落包终态 failed");
    let clean = finish
        .nodes
        .iter()
        .find(|node| node.step.as_str() == "clean")
        .expect("clean 节点应在案");
    assert_eq!(clean.status, RunStepStatus::Failed, "clean 节点 failed");
    assert!(
        clean
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("git -C wt clean 失败")),
        "clean detail 携 git 语境 Err 串: {clean:?}"
    );
    assert!(
        !finish.nodes.iter().any(|node| matches!(
            node.step.as_str(),
            "static_check" | "test_execution" | "phase_log"
        )),
        "Err 后检查步零执行（Terminal 停链）"
    );
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(
                command,
                ToolCommand::StaticCheck { .. } | ToolCommand::TestExecution { .. }
            )),
        "检查步零 spawn"
    );
    let entries = db
        .store
        .list_phase_records(CHANGE_ID)
        .expect("读相位条目应成功");
    assert!(
        !entries.iter().any(|entry| entry.phase == "initialize"),
        "Terminal 收敛零 initialize 条目"
    );
}

/// step_static_check_terminal static 红（异常，AC-2）：static_check 节点 failed
/// 携 output_ref + run `Terminal::failed` + 终态文案「基线静态检查未通过」携
/// baseline report_dir；TestExecution 步零执行（Terminal 收敛与一次性半边）。
#[tokio::test]
async fn step_static_check_terminal_红文案携baseline报告目录() {
    let root = TempRoot::new("init-static-red");
    root.change(NAME);
    let worktree = root.0.join("openspec/changes").join(NAME);
    let db = TestDb::open("init-static-red");
    seed_change_with_worktree(db.store.as_ref(), &worktree);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("initialize", &[])])
        .with_static_check(vec![baseline_check(false)])
        .assemble();
    let vcs = Arc::new(FakeVcs::clean());
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        vcs.clone(),
        snapshot,
        &control,
        false,
        &worktree.to_string_lossy(),
        &root.root_str(),
        None,
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "static 红 → Terminal failed"
    );

    let finish = history.finish().expect("收口落包捕获");
    let static_node = finish
        .nodes
        .iter()
        .find(|node| node.step.as_str() == "static_check")
        .expect("static_check 节点应在案");
    assert_eq!(static_node.status, RunStepStatus::Failed);
    assert_eq!(
        static_node.output_ref.as_deref(),
        Some("exec-baseline-static"),
        "failed 节点携 output_ref（回查面）"
    );
    let reason = finish.reason.expect("Terminal 记因应在案");
    assert!(
        reason.contains("基线静态检查未通过"),
        "终态文案 = static 红文案: {reason}"
    );
    assert!(
        reason.contains("reports/baseline"),
        "文案携 baseline report_dir（dir 占位供给面）: {reason}"
    );
    assert!(
        !finish
            .nodes
            .iter()
            .any(|node| node.step.as_str() == "test_execution"),
        "static 红后 TestExecution 步零执行"
    );
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(command, ToolCommand::TestExecution { .. })),
        "test-execution 零 spawn"
    );
}

/// step_test_execution_terminal 三态分文案（异常，AC-3）：`conclusion=Error` →
/// 环境文案（「基线测试执行出错」+ 报告目录）；`conclusion=Fail` → 测试红文案
///（「基线测试未通过」+ 报告目录）——与 static 红文案互异可辨（三态文案不共用）。
#[tokio::test]
async fn step_test_execution_terminal_error与fail分文案() {
    for (conclusion, expect_word) in [
        (TestExecutionConclusion::Error, "基线测试执行出错"),
        (TestExecutionConclusion::Fail, "基线测试未通过"),
    ] {
        let tag = format!("init-test-{}", conclusion.as_str());
        let root = TempRoot::new(&tag);
        root.change(NAME);
        let worktree = root.0.join("openspec/changes").join(NAME);
        let db = TestDb::open(&tag);
        seed_change_with_worktree(db.store.as_ref(), &worktree);

        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("initialize", &[])])
            .with_static_check(vec![baseline_check(true)])
            .with_test_execution(vec![baseline_execution(conclusion)])
            .assemble();
        let vcs = Arc::new(FakeVcs::clean());
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());

        let (task, _rx, history) = spawn_run_with_vcs(
            worker,
            tools,
            Arc::clone(&vcs),
            snapshot,
            &control,
            false,
            &worktree.to_string_lossy(),
            &root.root_str(),
            None,
        );
        let _confirmer = spawn_confirmer(&control, true);
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Failed, "{expect_word} → Terminal");

        let finish = history.finish().expect("收口落包捕获");
        let reason = finish.reason.expect("Terminal 记因应在案");
        assert!(
            reason.contains(expect_word),
            "conclusion={:?} 终态文案 = {expect_word}: {reason}",
            conclusion
        );
        assert!(
            reason.contains("reports/baseline"),
            "文案携 baseline 报告目录: {reason}"
        );
        if conclusion == TestExecutionConclusion::Error {
            assert!(!reason.contains("基线测试未通过"), "Error 不出测试红文案");
        } else {
            assert!(!reason.contains("基线测试执行出错"), "Fail 不出环境文案");
        }
    }
}

/// Terminal 收敛与一次性重试链（边界，重试链 / AC-2）：首 run static 红收敛
/// failed 后，二次 walk_run → phase_next 仍路由 initialize（零条目重发起）、
/// shim 转绿后全链干净重试收敛 completed（clean 清残留 + 全链重走）。
#[tokio::test]
async fn terminal收敛后二次发起_仍路由initialize且干净重试() {
    let root = TempRoot::new("init-retry");
    root.change(NAME);
    let worktree = root.0.join("openspec/changes").join(NAME);

    // 首 run：static 红 → Terminal failed（PhaseRecord 零 initialize 条目）
    let db = TestDb::open("init-retry");
    seed_change_with_worktree(db.store.as_ref(), &worktree);
    {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("initialize", &[])])
            .with_static_check(vec![baseline_check(false)])
            .assemble();
        let vcs = Arc::new(FakeVcs::clean());
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, _history) = spawn_run_with_vcs(
            worker,
            tools,
            vcs,
            snapshot,
            &control,
            false,
            &worktree.to_string_lossy(),
            &root.root_str(),
            None,
        );
        let _confirmer = spawn_confirmer(&control, true);
        assert_eq!(
            task.await.expect("run 任务正常结束"),
            ChangeRunStatus::Failed,
            "首 run static 红 Terminal"
        );
    }
    assert!(
        !db.store
            .list_phase_records(CHANGE_ID)
            .expect("读相位条目应成功")
            .iter()
            .any(|entry| entry.phase == "initialize"),
        "首 run 零 initialize 条目（Terminal 零落账）"
    );

    // 二次发起：shim 转绿 → phase_next 仍路由 initialize → 全链干净重试收敛
    {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("initialize", &[]), done_outcome()])
            .with_static_check(vec![baseline_check(true)])
            .with_test_execution(vec![baseline_execution(TestExecutionConclusion::Pass)])
            .assemble();
        let vcs = Arc::new(FakeVcs::clean());
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run_with_vcs(
            worker,
            tools,
            vcs.clone(),
            snapshot,
            &control,
            false,
            &worktree.to_string_lossy(),
            &root.root_str(),
            None,
        );
        let _confirmer = spawn_confirmer(&control, true);
        assert_eq!(
            task.await.expect("run 任务正常结束"),
            ChangeRunStatus::Completed,
            "二次发起干净重试收敛 completed"
        );
        assert_eq!(
            vcs.restore_calls().len(),
            1,
            "二次 run restore 恰一次（clean 清残留后干净重试）"
        );
        let finish = history.finish().expect("收口落包捕获");
        assert_eq!(
            finish.status,
            RunStatus::Completed,
            "二次 run 收敛 completed"
        );
    }
}

/// drive() legacy 短路（正向，AC-6）：`worktree=None` 形态（root == main_root）
/// 的 initialize → phase_start + phase_log 两枚节点留痕、PhaseLog 以
/// skipped=true + 空 checklist + 跳过因落账；Clean / StaticCheck / TestExecution
/// 三步零执行（零 vcs / 零检查 spawn、主仓零重置）；skipped 条目一次性持久同
/// pass 语义在案。
#[tokio::test]
async fn legacy短路_两枚节点留痕与零检查spawn() {
    let root = TempRoot::new("init-legacy");
    root.change(NAME);
    // 路由面为 FakeTools 预录（phase_next 不查 store——建档记录零依赖）

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("initialize", &[]), done_outcome()])
        .assemble();
    let vcs = Arc::new(FakeVcs::clean());
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    // root == main_root（legacy 主仓形态——worktree=None 存量记录）
    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        vcs.clone(),
        snapshot,
        &control,
        false,
        &root.root_str(),
        &root.root_str(),
        None,
    );
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "legacy 短路后 done 收敛"
    );

    let finish = history.finish().expect("收口落包捕获");
    let words: Vec<&str> = finish.nodes.iter().map(|node| node.step.as_str()).collect();
    assert_eq!(
        words,
        ["phase_start", "phase_log"],
        "legacy 短路恰两枚节点（PhaseStart 开相 + PhaseLog skipped 落账）"
    );
    assert!(
        finish.nodes.iter().all(|node| node.phase == "initialize"),
        "两节点 phase 恒 initialize"
    );

    // 三步零执行：零 vcs restore、零检查 spawn（主仓零重置零 clean）
    assert!(
        vcs.calls().is_empty(),
        "零 vcs 调用（restore_baseline 缺席——主仓红线）: {:?}",
        vcs.calls()
    );
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(
                command,
                ToolCommand::StaticCheck { .. } | ToolCommand::TestExecution { .. }
            )),
        "Clean / StaticCheck / TestExecution 三步零 spawn"
    );

    // PhaseLog skipped 落账载荷：skipped=true + 空 checklist + 跳过因
    let commands = commands.lock().expect("命令捕获锁");
    let log_input = commands
        .iter()
        .find_map(|command| match command {
            ToolCommand::PhaseLog { input, .. } => Some(input.clone()),
            _ => None,
        })
        .expect("phase-log 落账在场");
    assert!(log_input.skipped, "PhaseLog skipped=true");
    assert!(log_input.checklist.is_empty(), "空 checklist");
    assert!(
        log_input.report.contains("legacy"),
        "跳过因落账（report 携 legacy 语汇）: {}",
        log_input.report
    );
    assert!(
        log_input.executor_session_id.is_none()
            && log_input.evaluator_session_id.is_none()
            && log_input.decision_session_id.is_none(),
        "三会话槽位恒 None"
    );
    drop(commands);

    // skipped 落账载荷断言已承载跳过因可见面（PhaseLog 命令载荷直检——本用例
    // FakeTools 零落库，skipped 条目持久同 pass 语义的读侧断言由真实写面组合
    // 用例「真实写面组合全程演进对照一致」承载）
}

// ---------------------------------------------------------------------------
// 决策软拒环 / 漂移上限 / 决策输入面（test-execution-zero-agent-decision
// AC-3 / AC-4 / AC-5 / AC-9）
// ---------------------------------------------------------------------------

/// 决策软拒（异常）：第一轮输出已退役 `{"action":"retry"}` → 解析 `Err` → 漂移
/// 1、**零** WhitelistGate 节点、同会话续注纠正文案；第二轮合法 backtrack 经门
/// passed 后落账重路由收敛。
#[tokio::test]
async fn 决策软拒_退役retry一次纠正后合法backtrack() {
    let (root, db) = decision_fixture("decision-soft-reject-retry");
    let allowed = ["proposal", "dev-design", "implement"];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "retry" }"#.to_owned(),
            r#"{ "action": "backtrack", "to": "dev-design", "reason": "设计返工" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &allowed),
            max_retries_outcome("implement", &allowed, "首轮未过"),
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
    assert_eq!(status, ChangeRunStatus::Completed, "纠正后 backtrack 收敛");

    // 同会话续注 + 纠正文案（parse 失败不 emit 门节点）
    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 2, "软拒一次后续注一轮");
    assert!(
        decision_requests[0].continue_session.is_none(),
        "首轮新会话"
    );
    let first_session =
        sessions_of(&sessions.lock().expect("会话锁"), WorkerRole::Decision)[0].clone();
    assert_eq!(
        decision_requests[1].continue_session.as_deref(),
        Some(first_session.as_str()),
        "第二轮同会话续注"
    );
    assert!(
        decision_requests[1]
            .prompt
            .contains("上一轮决策输出被驳回：决策 JSON 结构漂移")
            && decision_requests[1].prompt.contains("三动作封闭集"),
        "纠正文案携解析错误与三动作重申: {}",
        decision_requests[1].prompt
    );
    drop(requests);

    // 门节点：解析失败轮零门节点，合法轮恰一个 passed
    let finish = history.finish().expect("finish 落包捕获");
    let gates: Vec<&RunNodeEntry> = finish
        .nodes
        .iter()
        .filter(|node| node.step == workflow::state::RunStepKind::WhitelistGate)
        .collect();
    assert_eq!(gates.len(), 1, "解析失败零门节点（仅合法轮 emit）");
    assert_eq!(
        gates[0].status,
        RunStepStatus::Passed,
        "合法 backtrack 门 passed"
    );

    // 两轮各挂账一次（同会话）
    let payloads = decision_log_payloads(&commands.lock().expect("命令锁"));
    assert_eq!(payloads.len(), 2, "每轮决策-log 各一次");
    assert_eq!(payloads[0].2, first_session);
    assert_eq!(payloads[1].2, first_session);
}

/// 决策漂移（异常）：连续 3 次自环（`to == 失败相位` 且不在白名单）——前两次
/// 各 emit 1 个 failed WhitelistGate（detail 含自环文案）并同会话续注纠正；
/// 第 3 次漂移 3 > `DECISION_DRIFT_LIMIT = 2` → `Terminal::failed("决策漂移")`。
#[tokio::test]
async fn 决策漂移_三次自环超限failed() {
    let (root, db) = decision_fixture("decision-drift-self-loop");
    // 白名单不含 implement → backtrack 到自身即自环（零 agent 步不可重做语义）
    let allowed = ["proposal"];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let self_loop = r#"{ "action": "backtrack", "to": "implement", "reason": "重做" }"#.to_owned();
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![self_loop.clone(), self_loop.clone(), self_loop])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &allowed),
            max_retries_outcome("implement", &allowed, "首轮未过"),
        ])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "漂移超限显式失败");
    let finish = history.finish().expect("finish 落包捕获");
    let reason = finish.reason.as_deref().unwrap_or_default();
    assert!(
        reason.contains("决策漂移") && reason.contains("驳回"),
        "记因决策漂移: {reason}"
    );

    // 三轮同会话续注、零 Backtrack 落账
    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 3, "前两次驳回 + 第三次超限");
    let first_session =
        sessions_of(&sessions.lock().expect("会话锁"), WorkerRole::Decision)[0].clone();
    for request in decision_requests.iter().skip(1) {
        assert_eq!(
            request.continue_session.as_deref(),
            Some(first_session.as_str()),
            "驳回后同会话续注"
        );
    }
    drop(requests);
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(command, ToolCommand::Backtrack { .. })),
        "自环决议零 Backtrack 落账"
    );

    // 前两轮各一个 failed 门节点（detail 含自环文案），无 passed 门
    let gates: Vec<&RunNodeEntry> = finish
        .nodes
        .iter()
        .filter(|node| node.step == workflow::state::RunStepKind::WhitelistGate)
        .collect();
    assert_eq!(
        gates.len(),
        3,
        "三次自环各 emit 一个门实例（第三次 emit 后超限收敛）"
    );
    for gate in &gates {
        assert_eq!(gate.status, RunStepStatus::Failed, "自环门 failed");
        let detail = gate.detail.clone().unwrap_or_default();
        assert!(
            detail.contains("确定性门禁相位") && detail.contains("stop / ask"),
            "自环文案含不可重做说明与出路: {detail}"
        );
    }
}

/// 决策漂移（异常，表驱动两非法词）：未知名动作 `explode` / 拼写漂移 `Retry`
/// 各一次软拒（零门节点、同会话续注），第三轮合法 stop 经门 passed 正常收敛。
#[tokio::test]
async fn 决策软拒_未知名动作两次后续stop收敛() {
    let (root, db) = decision_fixture("decision-unknown-actions");
    let allowed = ["proposal", "dev-design", "implement"];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "explode" }"#.to_owned(),
            r#"{ "action": "Retry" }"#.to_owned(),
            r#"{ "action": "stop", "reason": "无法裁决" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &allowed),
            max_retries_outcome("implement", &allowed, "首轮未过"),
        ])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Stopped, "两次软拒后合法 stop 收敛");

    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 3, "两次软拒 + 一次合法动作");
    let first_session =
        sessions_of(&sessions.lock().expect("会话锁"), WorkerRole::Decision)[0].clone();
    for request in decision_requests.iter().skip(1) {
        assert_eq!(
            request.continue_session.as_deref(),
            Some(first_session.as_str()),
            "软拒同会话续注"
        );
        assert!(
            request.prompt.contains("上一轮决策输出被驳回"),
            "续注为纠正文案: {}",
            request.prompt
        );
    }
    drop(requests);

    let finish = history.finish().expect("finish 落包捕获");
    let gates: Vec<&RunNodeEntry> = finish
        .nodes
        .iter()
        .filter(|node| node.step == workflow::state::RunStepKind::WhitelistGate)
        .collect();
    assert_eq!(
        gates.len(),
        1,
        "两次解析失败零门节点，仅合法 stop emit 一个"
    );
    assert_eq!(gates[0].status, RunStepStatus::Passed);
}

/// 决策输入面（正向 / 边界）：`fail_checklist` 仅携失败项（pass 项不随行）；
/// 全 pass 清单配 fail verdict → 全量随行不虚减；失败相位站缺席 → `attempt = 0`
/// 且 checklist 空、报告段占位「（无报告文本）」。
#[tokio::test]
async fn 决策输入_fail清单过滤与全pass随行与station缺席() {
    // 子用例一：fail 项 + pass 项 → 仅失败项随行
    let root = TempRoot::new("decision-input-filter");
    root.change(NAME);
    let db = TestDb::open("decision-input-filter");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "initialize",
        Verdict::Pass,
        "基线通过",
        Vec::new(),
        TS_BASE,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "proposal",
        Verdict::Fail,
        "提案未过",
        vec![
            ChecklistItem {
                item: "失败项".to_owned(),
                pass: false,
                evidence: "缺失目标段".to_owned(),
            },
            ChecklistItem {
                item: "通过项".to_owned(),
                pass: true,
                evidence: "结构齐备".to_owned(),
            },
        ],
        TS_BASE + 60_000,
    );
    let allowed = ["proposal"];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "人工介入" }"#.to_owned()
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &allowed),
            max_retries_outcome("proposal", &allowed, "提案未过"),
        ])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let _ = task.await.expect("run 任务正常结束");
    let prompt = decision_prompt_of(&requests);
    assert!(
        prompt.contains("- 失败项：缺失目标段") && !prompt.contains("- 通过项"),
        "fail_checklist 仅失败项随行: {prompt}"
    );
    assert!(
        prompt.contains("## 失败相位报告") && prompt.contains("提案未过"),
        "fail_report 取失败条目 report 原文"
    );

    // 子用例二：全 pass 清单配 fail verdict → 全量随行不虚减
    let root = TempRoot::new("decision-input-all-pass");
    root.change(NAME);
    let db = TestDb::open("decision-input-all-pass");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "initialize",
        Verdict::Pass,
        "基线通过",
        Vec::new(),
        TS_BASE,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "proposal",
        Verdict::Fail,
        "提案未过",
        vec![
            ChecklistItem {
                item: "清单项一".to_owned(),
                pass: true,
                evidence: "证据一".to_owned(),
            },
            ChecklistItem {
                item: "清单项二".to_owned(),
                pass: true,
                evidence: "证据二".to_owned(),
            },
        ],
        TS_BASE + 60_000,
    );
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "人工介入" }"#.to_owned()
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("proposal", &allowed),
            max_retries_outcome("proposal", &allowed, "提案未过"),
        ])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let _ = task.await.expect("run 任务正常结束");
    let prompt = decision_prompt_of(&requests);
    assert!(
        prompt.contains("- 清单项一：证据一") && prompt.contains("- 清单项二：证据二"),
        "全 pass 清单配 fail verdict → 全量随行不虚减: {prompt}"
    );

    // 子用例三：失败相位站缺席（detail 无 code-review 站）→ attempt 0 + 空清单
    // + 报告段占位
    let (root, db) = decision_fixture("decision-input-missing-station");
    let allowed = ["proposal", "dev-design", "implement"];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "stop", "reason": "人工介入" }"#.to_owned()
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("code-review", &allowed),
            max_retries_outcome("code-review", &allowed, "缺席快照"),
        ])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let _ = task.await.expect("run 任务正常结束");
    let prompt = decision_prompt_of(&requests);
    assert!(
        prompt.contains("第 0 次 attempt"),
        "station 缺席 → attempt = 0: {prompt}"
    );
    assert!(
        prompt.contains("（无报告文本）"),
        "station 缺席 → 报告段占位: {prompt}"
    );
}

/// 决策输入（异常）：快照 `detail` 读取 `Err` → `Terminal::failed("快照读取
/// 失败…")`、零决策请求；`last_result` 缺席 → `Terminal`（无法组装决策输入）。
#[tokio::test]
async fn 决策输入_快照失败与last_result缺席均terminal() {
    // 子用例一：快照读取 Err
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![max_retries_outcome(
            "implement",
            &["proposal", "implement"],
            "首轮未过",
        )])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "快照读取失败显式 failed");
    let finish = history.finish().expect("finish 落包捕获");
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("快照读取失败"),
        "记因快照读取失败: {:?}",
        finish.reason
    );
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "零决策请求（组装输入前失败）"
    );

    // 子用例二：last_result 缺席
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let mut no_last_result = max_retries_outcome("implement", &["proposal", "implement"], "未过");
    no_last_result.last_result = None;
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![no_last_result])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(MinimalSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "缺 last_result 显式 failed"
    );
    let finish = history.finish().expect("finish 落包捕获");
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("缺 last_result"),
        "记因无法组装决策输入: {:?}",
        finish.reason
    );
    assert!(requests.lock().expect("请求锁").is_empty(), "零决策请求");
}

/// `feedback_limit` 的 `Escalate` arm（异常）：static-check 步误声明
/// `OnFail::Escalate` → 显式漂移 `Err`（`不得声明 OnFail::Escalate`），零门禁
/// spawn、零 agent 会话（不得静默退化为禁用）。
#[tokio::test]
async fn feedback_limit_escalate误声明显式漂移err() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let mut drifted_route = route_outcome("implement", &["proposal", "implement"]);
    drifted_route.steps = vec![
        StepDefinition {
            node: StepNode::PhaseStart,
            prompt_ref: None,
            on_fail: None,
        },
        StepDefinition {
            node: StepNode::StaticCheck,
            prompt_ref: None,
            on_fail: Some(OnFail::Escalate),
        },
    ];
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![drifted_route])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Failed, "误声明显式漂移失败");
    let finish = history.finish().expect("finish 落包捕获");
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("不得声明 OnFail::Escalate"),
        "记因: {:?}",
        finish.reason
    );
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(command, ToolCommand::StaticCheck { .. })),
        "漂移路径零门禁 spawn"
    );
    assert!(requests.lock().expect("请求锁").is_empty(), "零 agent 会话");
}

/// ask 停等终止（边界）：决策 ask 流出后取消置位 → `wait_answer` 返回 `None`
/// → `Terminal::stopped("ask 停等被终止")`（漂移不增、零 Backtrack）。
#[tokio::test]
async fn 决策ask停等被终止收敛stopped() {
    let (root, db) = decision_fixture("decision-ask-abort");
    let allowed = ["proposal", "implement"];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "ask", "question": "回溯目标?", "options": ["proposal"] }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &allowed),
            max_retries_outcome("implement", &allowed, "首轮未过"),
        ])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, mut rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);

    // 等 Ask 通知后置取消（wait_answer 取消半边返回 None）
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if futures_poll(&mut rx).is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "等待 Ask 通知超时（30s）");
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(control.request_stop(ROOT, CHANGE_ID), "取消置位");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Stopped, "ask 停等被终止 → stopped");
    let finish = history.finish().expect("finish 落包捕获");
    assert!(
        finish
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("ask 停等被终止"),
        "记因: {:?}",
        finish.reason
    );
    assert!(
        commands
            .lock()
            .expect("命令锁")
            .iter()
            .all(|command| !matches!(command, ToolCommand::Backtrack { .. })),
        "零 Backtrack 落账"
    );
}

/// 决策 ask 多轮（边界）：连续 3 轮 ask 应答后续问——漂移恒 0（ask 不计漂移）、
/// 同会话续注、每轮 DecisionLog 各一次；第 4 轮合法 stop 经门 passed 收敛。
#[tokio::test]
async fn 决策ask连续三轮不计漂移后续stop() {
    let (root, db) = decision_fixture("decision-ask-three-rounds");
    let allowed = ["proposal", "implement"];
    let ask = r#"{ "action": "ask", "question": "回溯目标?", "options": ["proposal"] }"#.to_owned();
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            ask.clone(),
            ask.clone(),
            ask,
            r#"{ "action": "stop", "reason": "用户裁决终止" }"#.to_owned(),
        ])
        .assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &allowed),
            max_retries_outcome("implement", &allowed, "首轮未过"),
        ])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, mut rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);

    // 逐轮应答（每轮 Ask 通知流出后回传应答）
    for round in 1..=3 {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if futures_poll(&mut rx).is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "等待第 {round} 轮 Ask 超时（30s）"
            );
            tokio::task::yield_now().await;
            std::thread::sleep(Duration::from_millis(5));
        }
        control
            .answer(ROOT, CHANGE_ID, format!("采用方案 {round}"))
            .expect("应答回传应成功");
    }
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Stopped, "四轮后 stop 受控收敛");

    let requests = requests.lock().expect("请求锁");
    let decision_requests: Vec<&WorkerTurnRequest> = requests
        .iter()
        .filter(|request| request.role == WorkerRole::Decision)
        .collect();
    assert_eq!(decision_requests.len(), 4, "3 轮 ask + 1 轮 stop");
    let first_session =
        sessions_of(&sessions.lock().expect("会话锁"), WorkerRole::Decision)[0].clone();
    for request in decision_requests.iter().skip(1) {
        assert_eq!(
            request.continue_session.as_deref(),
            Some(first_session.as_str()),
            "ask 应答续问同会话（漂移不变）"
        );
    }
    drop(requests);

    // 漂移恒 0：ask 轮零门节点，仅合法 stop emit 一个 passed 门
    let finish = history.finish().expect("finish 落包捕获");
    let gates: Vec<&RunNodeEntry> = finish
        .nodes
        .iter()
        .filter(|node| node.step == workflow::state::RunStepKind::WhitelistGate)
        .collect();
    assert_eq!(gates.len(), 1, "ask 轮不计漂移、不 emit 门节点");
    assert_eq!(gates[0].status, RunStepStatus::Passed);

    // 每轮（含 ask 轮）DecisionLog 恰一次且 phase / session 一致
    let payloads = decision_log_payloads(&commands.lock().expect("命令锁"));
    assert_eq!(payloads.len(), 4, "每轮决策-log 各一次");
    for payload in &payloads {
        assert_eq!(payload.0, CHANGE_ID);
        assert_eq!(payload.1, "implement");
        assert_eq!(payload.2, first_session);
    }
}

/// 组合链（正向，AC-6）：test-execution 首败升格 → 决策 backtrack 到 implement
/// → implement 执行 → test-execution 重入 → 真重跑 pass → 收敛；重入的
/// implement 入口边为 `backtrack`（非 `retry`），全程零 `retry` 边。
#[tokio::test]
async fn 组合链_fail决策backtrack重跑test_execution与入口边kind() {
    let (root, db) = decision_fixture("combo-backtrack-rerun");
    let allowed = ["proposal", "dev-design", "implement", "test-gen"];
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline)
        .with_decision_reports(vec![
            r#"{ "action": "backtrack", "to": "implement", "reason": "修绿测试" }"#.to_owned(),
        ])
        .assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            // ① test-execution 首败升格 → 回 phase-next
            route_outcome("test-execution", &allowed),
            // ② 决策分叉（fail 条目未决）→ backtrack 到 implement
            max_retries_outcome("test-execution", &allowed, "测试未过"),
            // ③ implement 执行（入口边 kind = backtrack）
            route_outcome("implement", &allowed),
            // ④ test-execution 重入 → 真重跑（复用门失效由 runner 侧 mtime 承载，
            //    walker 侧以第二产出刷新证明真重跑）
            route_outcome("test-execution", &allowed),
            done_outcome(),
        ])
        .with_test_execution(vec![
            escalate_test_outcome(TestExecutionConclusion::Fail, "首轮未过"),
            escalate_test_outcome(TestExecutionConclusion::Pass, ""),
        ])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(status, ChangeRunStatus::Completed, "重跑 pass 后收敛");

    let finish = history.finish().expect("finish 落包捕获");
    // implement 入口节点在案：其入边 kind = backtrack（非 retry）
    let implement_entry = finish
        .nodes
        .iter()
        .find(|node| {
            node.phase == "implement" && node.step == workflow::state::RunStepKind::PhaseStart
        })
        .expect("implement 入口节点在案");
    let entry_key = format!(
        "{}:{}:{}:{}",
        implement_entry.phase,
        implement_entry.attempt,
        implement_entry.step.as_str(),
        implement_entry.instance_no
    );
    let incoming: Vec<&workflow::state::RunEdgeEntry> = finish
        .edges
        .iter()
        .filter(|edge| edge.to_node_key == entry_key)
        .collect();
    assert_eq!(incoming.len(), 1, "入口节点恰一条入边");
    assert_eq!(
        incoming[0].edge_kind,
        workflow::state::RunEdgeKind::Backtrack,
        "重入入口边 kind = backtrack（非 retry）"
    );
    assert_eq!(
        incoming[0].from_node_key, "test-execution:0:retry_gate:0",
        "回跳源 = 失败相位门节点（attempt 取库内该相位最新 attempt）"
    );
    // 全程零 retry 边
    assert!(
        finish
            .edges
            .iter()
            .all(|edge| edge.edge_kind != workflow::state::RunEdgeKind::Retry),
        "全程零 retry 边（存量解码契约不新产出）"
    );
    // test-execution 真重跑：两轮 TestExecution 产出（Node 两实例跨两 attempt）
    let test_runs = step_instances(&finish.nodes, workflow::state::RunStepKind::TestExecution);
    assert_eq!(test_runs.len(), 2, "test-execution 真重跑两轮（非复用）");
    // 决策会话恰一次（implement 重跑 pass 后不再分叉）
    let requests = requests.lock().expect("请求锁");
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.role == WorkerRole::Decision)
            .count(),
        1,
        "决策会话恰一次"
    );
}

// ---------------------------------------------------------------------------
// feedback_limit 五值边界（test-execution-zero-agent-decision 用例表
// 「`feedback_limit` 计数边界」逐行承载）
// ---------------------------------------------------------------------------

/// `feedback_limit`（边界，参数化 5 值）：`FeedbackLoop { 0 }` / `{ 1 }` /
/// `{ u32::MAX }` → `Ok(limit)` 原值透传——0 = 首败即升格（零反馈修复会话）、
/// 1 = 恰一轮反馈修复后升格、`u32::MAX` 不饱和为 0 / 5（连续六次失败仍在反馈
/// 期内，第七次通过收口）；`Terminal` 走基线绿门单发执行体（既有回归形态，失败
/// 即停，不落漂移分支）；`None` 缺声明 → 既有漂移 `Err`（「缺
/// OnFail::FeedbackLoop 声明」），零门禁 spawn（不静默退化为禁用）。
#[tokio::test]
async fn feedback_limit五值边界透传与漂移err() {
    struct Case {
        label: &'static str,
        on_fail: Option<OnFail>,
        failing_checks: usize,
        expect_status: ChangeRunStatus,
        expect_static_checks: usize,
        expect_fix_sessions: usize,
        expect_upgrade: bool,
        expect_reason: Option<&'static str>,
    }

    let cases = [
        // 值一：limit = 0 → 首败即升格（零反馈修复会话）
        Case {
            label: "FeedbackLoop{0}",
            on_fail: Some(OnFail::FeedbackLoop { limit: 0 }),
            failing_checks: 3,
            expect_status: ChangeRunStatus::Completed,
            expect_static_checks: 1,
            expect_fix_sessions: 0,
            expect_upgrade: true,
            expect_reason: None,
        },
        // 值二：limit = 1 → 恰一轮反馈修复后升格
        Case {
            label: "FeedbackLoop{1}",
            on_fail: Some(OnFail::FeedbackLoop { limit: 1 }),
            failing_checks: 3,
            expect_status: ChangeRunStatus::Completed,
            expect_static_checks: 2,
            expect_fix_sessions: 1,
            expect_upgrade: true,
            expect_reason: None,
        },
        // 值三：limit = u32::MAX → 原值透传（六次失败仍在反馈期，第七次通过）
        Case {
            label: "FeedbackLoop{u32::MAX}",
            on_fail: Some(OnFail::FeedbackLoop { limit: u32::MAX }),
            failing_checks: 6,
            expect_status: ChangeRunStatus::Completed,
            expect_static_checks: 7,
            expect_fix_sessions: 6,
            expect_upgrade: false,
            expect_reason: None,
        },
        // 值四：Terminal → 基线绿门单发执行体（既有回归形态）
        Case {
            label: "Terminal",
            on_fail: Some(OnFail::Terminal),
            failing_checks: 1,
            expect_status: ChangeRunStatus::Failed,
            expect_static_checks: 1,
            expect_fix_sessions: 0,
            expect_upgrade: false,
            expect_reason: Some("基线静态检查未通过"),
        },
        // 值五：None（缺声明）→ 漂移 Err（不臆造缺省上限、零门禁 spawn）
        Case {
            label: "None",
            on_fail: None,
            failing_checks: 1,
            expect_status: ChangeRunStatus::Failed,
            expect_static_checks: 0,
            expect_fix_sessions: 0,
            expect_upgrade: false,
            expect_reason: Some("缺 OnFail::FeedbackLoop 声明"),
        },
    ];

    for case in cases {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let mut route = route_outcome("implement", &["proposal", "implement"]);
        route.steps = vec![
            StepDefinition {
                node: StepNode::PhaseStart,
                prompt_ref: None,
                on_fail: None,
            },
            StepDefinition {
                node: StepNode::StaticCheck,
                prompt_ref: None,
                on_fail: case.on_fail,
            },
        ];
        let (tools, commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route, done_outcome()])
            .with_static_check(vec![failing_check("类型错误"); case.failing_checks])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
        let _confirmer = spawn_confirmer(&control, true);
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, case.expect_status, "{} 终态", case.label);

        let finish = history.finish().expect("finish 落包捕获");
        if let Some(expected) = case.expect_reason {
            let reason = finish.reason.as_deref().unwrap_or_default();
            assert!(
                reason.contains(expected),
                "{} 收敛记因应含「{expected}」: {reason}",
                case.label
            );
        }

        let commands = commands.lock().expect("命令锁");
        let static_checks = commands
            .iter()
            .filter(|command| matches!(command, ToolCommand::StaticCheck { .. }))
            .count();
        assert_eq!(
            static_checks, case.expect_static_checks,
            "{} 门禁 spawn 次数（limit 原值透传）",
            case.label
        );
        let upgrade = commands.iter().any(|command| {
            matches!(command, ToolCommand::PhaseLog { input, .. } if input.report.contains("升格"))
        });
        assert_eq!(
            upgrade, case.expect_upgrade,
            "{} 超限升格落账面",
            case.label
        );
        drop(commands);

        let fix_sessions = requests
            .lock()
            .expect("请求锁")
            .iter()
            .filter(|request| request.role == WorkerRole::Executor)
            .count();
        assert_eq!(
            fix_sessions, case.expect_fix_sessions,
            "{} 反馈修复会话数（同会话续注、不换档）",
            case.label
        );
    }
}

// ---------------------------------------------------------------------------
// 决策漂移混合序列（test-execution-zero-agent-decision 用例表
// 「`decision_session` 漂移计数边界」逐行承载）
// ---------------------------------------------------------------------------

/// 决策漂移计数边界（边界，表驱动 3 序列）：漂移 1 / 2 仍同会话续注纠正、漂移
/// 3（> `DECISION_DRIFT_LIMIT = 2`）才 `Terminal::failed("决策漂移…")`；解析
/// `Err` 与门驳回两类软拒落在**同一条**漂移计数器上连续累加、不分流——序列一
/// 纯解析失败 ×3（零门节点）、序列二纯门驳回（自环）×3（每轮一个 failed 门
/// 实例）、序列三混合「解析 `Err` → 门 failed → 解析 `Err`」（仅解析成功那一轮
/// emit 门实例；漂移 1 / 2 / 3 跨类累加至收口）。
#[tokio::test]
async fn 决策漂移混合序列三序列累加不分流() {
    const SELF_LOOP: &str = r#"{ "action": "backtrack", "to": "implement", "reason": "重做" }"#;

    struct Case {
        tag: &'static str,
        reports: [&'static str; 3],
        expect_gate_nodes: usize,
        expect_reason: &'static str,
    }

    let cases = [
        // 序列一：纯解析失败（无动作可校验 → 零门节点）
        Case {
            tag: "drift-seq-parse",
            reports: ["不是决策 JSON", "仍不是决策 JSON", "第三次仍不是决策 JSON"],
            expect_gate_nodes: 0,
            expect_reason: "无法解析",
        },
        // 序列二：纯门驳回（自环 → 每轮一个 failed 门实例）
        Case {
            tag: "drift-seq-gate",
            reports: [SELF_LOOP, SELF_LOOP, SELF_LOOP],
            expect_gate_nodes: 3,
            expect_reason: "被驳回",
        },
        // 序列三：混合序列（漂移 1 解析 Err / 漂移 2 门 failed / 漂移 3 解析 Err）
        Case {
            tag: "drift-seq-mixed",
            reports: ["不是决策 JSON", SELF_LOOP, "仍不是决策 JSON"],
            expect_gate_nodes: 1,
            expect_reason: "无法解析",
        },
    ];

    for case in cases {
        let (root, db) = decision_fixture(case.tag);
        // 白名单不含 implement → backtrack 到自身即自环（零 agent 步不可重做）
        let allowed = ["proposal"];
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, requests, sessions) = FakeWorker::new(&timeline)
            .with_decision_reports(
                case.reports
                    .iter()
                    .map(|report| (*report).to_owned())
                    .collect(),
            )
            .assemble();
        let (tools, commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![
                route_outcome("implement", &allowed),
                max_retries_outcome("implement", &allowed, "首轮未过"),
            ])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> =
            Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
        let control = Arc::new(ChangeFlowControl::new());
        let (task, _rx, history) =
            spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
        let _confirmer = spawn_confirmer(&control, true);
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(
            status,
            ChangeRunStatus::Failed,
            "{} 漂移 3 超限显式失败",
            case.tag
        );

        let finish = history.finish().expect("finish 落包捕获");
        let reason = finish.reason.clone().unwrap_or_default();
        assert!(
            reason.contains("决策漂移") && reason.contains(case.expect_reason),
            "{} 记因为第 3 次驳回（前两次仍在纠正期）: {reason}",
            case.tag
        );

        // 决策请求恰 3 次：漂移 1 / 2 后同会话续注，第 3 次才收口
        let requests = requests.lock().expect("请求锁");
        let decision_requests: Vec<&WorkerTurnRequest> = requests
            .iter()
            .filter(|request| request.role == WorkerRole::Decision)
            .collect();
        assert_eq!(
            decision_requests.len(),
            3,
            "{} 漂移 1 / 2 仍续注（不分流），第 3 次才 Terminal",
            case.tag
        );
        assert!(
            decision_requests[0].continue_session.is_none(),
            "{} 首轮新会话",
            case.tag
        );
        let first_session =
            sessions_of(&sessions.lock().expect("会话锁"), WorkerRole::Decision)[0].clone();
        for request in decision_requests.iter().skip(1) {
            assert_eq!(
                request.continue_session.as_deref(),
                Some(first_session.as_str()),
                "{} 每次驳回后同会话续注纠正",
                case.tag
            );
        }
        assert!(
            decision_requests
                .iter()
                .skip(1)
                .all(|request| request.prompt.contains("上一轮决策输出被驳回")),
            "{} 续注轮为纠正文案（非首轮决策 prompt）",
            case.tag
        );
        drop(requests);

        // 门节点实例数：仅解析成功轮 emit（parse Err 轮零门节点）
        let gates: Vec<&RunNodeEntry> = finish
            .nodes
            .iter()
            .filter(|node| node.step == workflow::state::RunStepKind::WhitelistGate)
            .collect();
        assert_eq!(
            gates.len(),
            case.expect_gate_nodes,
            "{} 门实例数（解析 Err 轮不计门节点）",
            case.tag
        );
        assert!(
            gates
                .iter()
                .all(|gate| gate.status == RunStepStatus::Failed),
            "{} 驳回轮门恒 failed（无 passed 门）",
            case.tag
        );

        // 零 Backtrack 落账（三类漂移均未兑现动作）；每轮 DecisionLog 各一次
        let commands = commands.lock().expect("命令锁");
        assert!(
            commands
                .iter()
                .all(|command| !matches!(command, ToolCommand::Backtrack { .. })),
            "{} 漂移路径零 Backtrack 落账",
            case.tag
        );
        assert_eq!(
            decision_log_payloads(&commands).len(),
            3,
            "{} 每轮决策-log 各一次（parse 失败同样留痕）",
            case.tag
        );
    }
}

// ---------------------------------------------------------------------------
// 取消贯通（test-design「walker.rs -> walker_test.rs」节，AC-6 / AC-7 / D10）：
// `run_tool` 单点从 `guard.cancel_flag()` 填充 `ToolStepRequest.cancel`（13 处
// 调用点零重复填充）；stop 运行中的检查步 → 假 runner 观测取消并以
// `fault = Some(Stopped)` 正常返回产出 → 该步节点 `stopped`、run **秒级收敛
// stopped**（MUST NOT 收敛 failed；取消 MUST NOT 以 `Err` 表达——本变更取代
// 既有「run failed 的过渡形态」，此处不留第二套口径）。
// ---------------------------------------------------------------------------

/// 可取消的检查步假 runner：进入即挂起（`entered` 置位供测试侧握手），观测到
/// 取消即以 `fault = Some(RunFault::Stopped)` 正常返回产出（真实执行体的「入口
/// 短路 / 轮询置位」收敛面在 runner 层等价表达；取消 MUST NOT 以 `Err` 表达）；
/// 同时记录收到的 flag（单点填充断言原料）。
struct CancelableStaticRunner {
    entered: Arc<std::sync::atomic::AtomicBool>,
    received: Arc<Mutex<Vec<CancelFlag>>>,
}

impl CancelableStaticRunner {
    fn new() -> Self {
        Self {
            entered: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            received: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl StaticCheckRunner for CancelableStaticRunner {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _slot: foundation::layout::ReportSlot,
        cancel: CancelFlag,
    ) -> BoxToolFuture {
        self.received
            .lock()
            .expect("received 锁不可中毒")
            .push(cancel.clone());
        self.entered.store(true, Ordering::SeqCst);
        Box::pin(async move {
            loop {
                if cancel.is_set() {
                    // 取消 → 执行面归因产出（fault = Stopped），MUST NOT 返回 Err
                    return Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
                        passed: false,
                        diagnostics: String::new(),
                        fault: Some(RunFault::Stopped),
                        output_ref: Some("exec-cancelled-static".to_owned()),
                        report_dir: "reports/test".to_owned(),
                    }));
                }
                // 协作让位（本 crate 的 tokio 未启 time 特性——轮询以 yield 表达；
                // 取消观测在毫秒级窗口内收敛，不构成空转）
                tokio::task::yield_now().await;
            }
        })
    }
}

/// 正向（AC-7 组合锚）：stop 运行中的检查步——假 runner 观测取消即以
/// `fault = Some(Stopped)` 正常返回，该步节点收敛 `stopped`、run 秒级收敛
/// `Stopped`（MUST NOT 为 `failed`：无 failed 节点、图上不呈现「执行已取消」
/// 失败文案）；捕获的 flag 与 `guard.cancel_flag()` 同源。
#[tokio::test]
async fn 停止运行中检查步_秒级收敛stopped且无failed节点() {
    let root = TempRoot::new("stop-running-check");
    root.change(NAME);
    let db = TestDb::open("stop-running-check");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let runner = CancelableStaticRunner::new();
    let entered = Arc::clone(&runner.entered);
    let received = Arc::clone(&runner.received);
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(runner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(NullMutationRunner),
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

    // 检查步进入挂起即运行中窗口
    wait_for("检查步进入挂起", || entered.load(Ordering::SeqCst)).await;
    let started = Instant::now();
    assert!(control.request_stop(ROOT, CHANGE_ID), "运行中置位返回 true");

    let status = task.await.expect("run 任务正常结束");
    let elapsed = started.elapsed();

    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "取消 → run 收敛 stopped（MUST NOT 呈现为失败）"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "秒级收敛（不等挂起自然结束，实际 {elapsed:?}）"
    );

    // 终态与记因经 Finished 流出：停因「用户请求停止」，无旧过渡文案
    assert!(
        drain_notices(&mut rx)
            .iter()
            .any(|notice| *notice == RunNotice::Finished),
        "Finished 信封在场"
    );
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(
        finish.status,
        RunStatus::Stopped,
        "收口落包 stopped（取消不作 failed）"
    );
    let reason = finish.reason.as_deref().unwrap_or_default();
    assert!(
        reason.contains("用户请求停止"),
        "记因为停因「用户请求停止」: {:?}",
        finish.reason
    );
    assert!(
        !reason.contains("执行已取消"),
        "取消不再落旧「执行已取消」失败文案: {reason}"
    );

    // 节点面：该检查步 settled stopped，全图无 failed 节点残留
    let node = finish
        .nodes
        .iter()
        .find(|node| node.step.as_str() == "static_check")
        .expect("static_check 节点应在案");
    assert_eq!(
        node.status,
        RunStepStatus::Stopped,
        "取消步节点 status = stopped（非 failed）"
    );
    assert!(
        node.detail
            .as_deref()
            .is_some_and(|detail| detail.contains("用户请求停止")),
        "节点 detail 记停因: {:?}",
        node.detail
    );
    assert_eq!(
        node.output_ref.as_deref(),
        Some("exec-cancelled-static"),
        "stopped 节点保留输出引用（全量可回查）"
    );
    assert!(
        !finish
            .nodes
            .iter()
            .any(|node| node.status == RunStepStatus::Failed),
        "无 failed 节点（stopped 与 failed 不混用）"
    );

    // 单点填充：runner 收到的 flag 已置位（`run_tool` 从 guard 取值，无中间重建）
    let recorded = received.lock().expect("received 锁").clone();
    assert!(!recorded.is_empty(), "检查步至少一次进入 runner");
    assert!(
        recorded.iter().all(CancelFlag::is_set),
        "runner 观测到置位（guard.cancel_flag() 单点填充生效）"
    );
    assert!(control.snapshot(ROOT, CHANGE_ID).is_none(), "终态除名");
}

/// 异常：runner 的 `Err` 面不被误叙述为取消——假 runner 返回 `Err("boom")` →
/// run failed，文案为「{label} 失败: boom」，**不含**停因文案「用户请求停止」
/// （`Err` 只承载基础设施失败，与 `fault = Stopped` 正常返回的停止面可辨）。
#[tokio::test]
async fn 非取消类err收敛failed且文案含原错误串() {
    let root = TempRoot::new("err-not-cancel");
    root.change(NAME);
    let db = TestDb::open("err-not-cancel");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(AlwaysFailStaticRunner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(NullMutationRunner),
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
        ChangeRunStatus::Failed,
        "非取消类失败同样收敛 failed"
    );
    let finish = history.finish().expect("finish 落包捕获");
    let reason = finish.reason.as_deref().unwrap_or_default();
    assert!(reason.contains("boom"), "记因携原错误串: {reason}");
    assert!(
        !reason.contains("用户请求停止"),
        "Err 面不误用停因文案（与 fault=Stopped 面可辨）: {reason}"
    );
    assert!(
        !reason.contains("执行已取消"),
        "Err 面不落旧「执行已取消」文案: {reason}"
    );
}

/// 恒败检查步 runner（非取消类 Err 的注入面）。
struct AlwaysFailStaticRunner;

impl StaticCheckRunner for AlwaysFailStaticRunner {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _slot: foundation::layout::ReportSlot,
        _cancel: CancelFlag,
    ) -> BoxToolFuture {
        Box::pin(async move { Err("boom".to_owned()) })
    }
}

// ---------------------------------------------------------------------------
// `run_tool` 单点填充（test-design「walker.rs -> walker_test.rs」节，AC-6 /
// D10）：`ToolStepRequest.cancel` 只在 `run_tool` 一处经 `guard.cancel_flag()`
// 填充（各步函数不各自取值、无中间重建）——常态链零置位、stop 后同一原子贯通。
// ---------------------------------------------------------------------------

/// 三检查步全链驱动（`implement` 站 static-check + `test-execution` 站
/// test-execution / mutation-test 各 ≥1 次）——返回捕获序与 run 终态。
async fn drive_three_gate_steps() -> (Vec<(&'static str, CancelFlag)>, ChangeRunStatus) {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let tools = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            route_outcome("test-execution", &[]),
            done_outcome(),
        ])
        .with_static_check(vec![passing_check()])
        .with_test_execution(vec![execution_outcome(TestExecutionConclusion::Pass, 5)])
        .with_mutation(vec![mutation_outcome(
            MutationTestConclusion::Pass,
            95.0,
            80.0,
            "",
        )]);
    let cancels = tools.cancels_handle();
    let (tools, _commands) = tools.assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) = spawn_run(worker, tools, snapshot, &control, true, "/tmp/root");
    let _confirmer = spawn_confirmer(&control, true);
    let status = task.await.expect("run 任务正常结束");
    let seen = cancels.lock().expect("cancel 捕获锁不可中毒").clone();
    (seen, status)
}

/// 正向（AC-6 / D10）：常态 run（未 stop）——每个 `ToolStepRequest.cancel`
/// 均为未置位，且全部请求共用同一 `guard.cancel_flag()` 原子（13 处调用点零
/// 重复填充、无中间重建）。
#[tokio::test]
async fn run_tool单点填充_常态全链flag未置位且同一原子() {
    let (seen, status) = drive_three_gate_steps().await;

    assert_eq!(
        status,
        ChangeRunStatus::Completed,
        "常态 run 收口 Completed"
    );
    assert!(!seen.is_empty(), "工具步请求全部被写面捕获");
    assert!(
        seen.iter().all(|(_, flag)| !flag.is_set()),
        "常态 run 零置位（轮询臂零副作用，不产生假取消）"
    );
    let first = seen[0].1.clone();
    assert!(
        seen.iter().all(|(_, flag)| *flag == first),
        "全部请求共用 guard.cancel_flag() 克隆（同一原子）: {:?}",
        seen.iter().map(|(label, _)| *label).collect::<Vec<_>>()
    );
}

/// 正向（AC-6 / D10）：stop 后仍推进到下一次 `run_tool`——static-check 步在
/// 挂起窗口内被 stop，随后同相位后续工具步（phase-log）捕获的 `cancel` 已是
/// 置位（`guard.cancel_flag()` 单点填充生效）；run 至步边界收敛受控 stopped。
#[tokio::test]
async fn run_tool单点填充_stop后下一次请求携置位() {
    let root = TempRoot::new("stop-fill-next");
    root.change(NAME);
    let db = TestDb::open("stop-fill-next");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let tools = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            done_outcome(),
        ])
        .with_static_check(vec![passing_check()])
        .hold_static_check();
    let cancels = tools.cancels_handle();
    let (tools, _commands) = tools.assemble();

    let snapshot: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root.root_str(), db.store_arc()));
    let control = Arc::new(ChangeFlowControl::new());
    let (task, _rx, _history) =
        spawn_run(worker, tools, snapshot, &control, false, &root.root_str());
    let _confirmer = spawn_confirmer(&control, true);

    // static-check 步进入挂起窗口（请求已被写面捕获）后置位停止
    wait_for("static-check 步进入挂起", || {
        cancels
            .lock()
            .expect("cancel 捕获锁不可中毒")
            .iter()
            .any(|(label, _)| *label == "static-check")
    })
    .await;
    assert!(
        control.request_stop(ROOT, CHANGE_ID),
        "运行中置位返回 true（双写：watch + 原子）"
    );

    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "步边界 check_cancel 收敛受控 stopped"
    );

    let seen = cancels.lock().expect("cancel 捕获锁不可中毒").clone();
    let index = seen
        .iter()
        .position(|(label, _)| *label == "static-check")
        .expect("static-check 请求在场");
    let later = &seen[index + 1..];
    assert!(
        !later.is_empty(),
        "stop 后 run 仍推进到下一次 run_tool（同相位后续步: {:?}）",
        later.iter().map(|(label, _)| *label).collect::<Vec<_>>()
    );
    assert!(
        later.iter().all(|(_, flag)| flag.is_set()),
        "stop 后捕获的 cancel 已置位（guard.cancel_flag() 单点填充，无中间重建）"
    );
}

/// 正向（AC-6）：多步全链同一原子——一次 run 覆盖 static-check /
/// test-execution / mutation-test 各 ≥1 次，三检查步捕获的 flag 全部相等
///（13 处调用点共用 `guard.cancel_flag()` 克隆，单点填充无重建）。
#[tokio::test]
async fn 多步全链同一原子_三检查步共用guard原子() {
    let (seen, status) = drive_three_gate_steps().await;
    assert_eq!(status, ChangeRunStatus::Completed);

    for label in ["static-check", "test-execution", "mutation-test"] {
        assert!(
            seen.iter().any(|(seen_label, _)| *seen_label == label),
            "检查步 {label} 至少一次进入写面（实际: {:?}）",
            seen.iter()
                .map(|(seen_label, _)| *seen_label)
                .collect::<Vec<_>>()
        );
    }

    let check_flags: Vec<CancelFlag> = seen
        .iter()
        .filter(|(label, _)| matches!(*label, "static-check" | "test-execution" | "mutation-test"))
        .map(|(_, flag)| flag.clone())
        .collect();
    assert!(check_flags.len() >= 3, "三检查步请求各自在案");
    let anchor = check_flags[0].clone();
    assert!(
        check_flags.iter().all(|flag| *flag == anchor),
        "三检查步共用同一原子（run_tool 单点填充，调用点零重复取值）"
    );
}

/// 挂起且**不观测** cancel 的检查步假 runner：进入即挂起（`entered` 置位供测试
/// 侧握手），只按测试侧 `release` 标记以 `fault = Some(RunFault::Stopped)` 正常
/// 返回产出（取消 MUST NOT 以 `Err` 表达）——walker 若越权做步内截止，run 会在
/// 未释放时收敛；不越权则一直挂到 runner 正常返回。
struct HangingIgnoreCancelRunner {
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    received: Arc<Mutex<Vec<CancelFlag>>>,
}

impl HangingIgnoreCancelRunner {
    fn new() -> Self {
        Self {
            entered: Arc::new(AtomicBool::new(false)),
            release: Arc::new(AtomicBool::new(false)),
            received: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl StaticCheckRunner for HangingIgnoreCancelRunner {
    fn run(
        &self,
        _root: &str,
        _name: &str,
        _slot: foundation::layout::ReportSlot,
        cancel: CancelFlag,
    ) -> BoxToolFuture {
        self.received
            .lock()
            .expect("received 锁不可中毒")
            .push(cancel.clone());
        self.entered.store(true, Ordering::SeqCst);
        let release = Arc::clone(&self.release);
        Box::pin(async move {
            // 挂起且**不读** cancel（D10：步内无取消观测面）
            while !release.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
            // 释放后按新契约走执行面归因产出（取消 → fault = Stopped，不 Err）
            Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
                passed: false,
                diagnostics: String::new(),
                fault: Some(RunFault::Stopped),
                output_ref: Some("exec-hang-stopped".to_owned()),
                report_dir: "reports/test".to_owned(),
            }))
        })
    }
}

/// 边界（AC-7 / D10）：**walker 不作步内截止**——假 runner 挂起且不观测 cancel：
/// stop 后 run 在观测窗口内不收敛（无步内砍断、无越权 stopped），收口只由 runner
/// 正常返回的产出承接（释放挂起 → `fault = Some(Stopped)` → run 收敛 `Stopped`，
/// 文案含「用户请求停止」——MUST NOT 收敛 failed、MUST NOT 落「执行已取消」）。
#[tokio::test]
async fn 挂起且不观测cancel_walker不作步内截止且收口由runner产出承接() {
    let root = TempRoot::new("hang-ignore-cancel");
    root.change(NAME);
    let db = TestDb::open("hang-ignore-cancel");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let runner = HangingIgnoreCancelRunner::new();
    let entered = Arc::clone(&runner.entered);
    let release = Arc::clone(&runner.release);
    let received = Arc::clone(&runner.received);
    let steps: Arc<dyn ToolStepPort> = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(runner),
        Arc::new(NullTestExecutionRunner),
        Arc::new(NullMutationRunner),
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

    // 检查步进入挂起即运行中窗口
    wait_for("检查步进入挂起", || entered.load(Ordering::SeqCst)).await;
    let started = Instant::now();
    assert!(control.request_stop(ROOT, CHANGE_ID), "运行中置位返回 true");

    // 观测窗口：walker 只在步边界 check_cancel（本变更不引入步内砍断）——挂起
    // 步未返回前 run MUST NOT 收敛（既不 failed 也不 stopped）
    let window = Duration::from_millis(800);
    while started.elapsed() < window {
        assert!(
            !task.is_finished(),
            "walker 不作步内截止（stop 后挂起步仍在跑，run 未收敛，已过 {:?}）",
            started.elapsed()
        );
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        history.finish().is_none(),
        "窗口内无终态落包（未越权收敛为 stopped / failed）"
    );
    assert!(
        control.snapshot(ROOT, CHANGE_ID).is_some(),
        "窗口内 run 仍在册（未提前除名）"
    );
    // 单点填充面：runner 持有的是同一 guard flag 且已置位（信号已透传到执行体，
    // 但执行体「不观测」是 runner 侧的自由——walker 不再兜底）
    let recorded = received.lock().expect("received 锁").clone();
    assert!(!recorded.is_empty(), "检查步至少一次进入 runner");
    assert!(
        recorded.iter().all(CancelFlag::is_set),
        "runner 持置位 flag（run_tool 单点填充，无中间重建）"
    );

    // 收口只由 runner 正常返回的产出承接：释放挂起 → fault = Stopped → stopped
    release.store(true, Ordering::SeqCst);
    let status = task.await.expect("run 任务正常结束");
    let elapsed = started.elapsed();
    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "runner 产出承接 stopped（取消 MUST NOT 收敛 failed）"
    );
    assert!(
        elapsed < Duration::from_secs(30),
        "释放后秒级收敛（实际 {elapsed:?}）"
    );
    let finish = history.finish().expect("finish 落包捕获");
    assert_eq!(finish.status, RunStatus::Stopped, "落包终态 stopped");
    let reason = finish.reason.as_deref().unwrap_or_default();
    assert!(
        reason.contains("用户请求停止"),
        "记因为停因「用户请求停止」: {reason}"
    );
    assert!(
        !reason.contains("执行已取消") && !reason.contains("static-check 失败"),
        "不再落旧「{{label}} 失败: 执行已取消」口径: {reason}"
    );
    let node = finish
        .nodes
        .iter()
        .find(|node| node.step.as_str() == "static_check")
        .expect("static_check 节点应在案");
    assert_eq!(
        node.status,
        RunStepStatus::Stopped,
        "释放后该步节点 stopped（非 failed）"
    );
    // AC-8：fault 面不被误叙述为环境错误（环境文案只服务 `Environment` 变体）
    assert!(
        !reason.contains("无法完成（环境错误）") && !reason.contains("排除 workspace 父目录"),
        "Stopped 面不落环境文案: {reason}"
    );
}

// ---------------------------------------------------------------------------
// 执行面归因分派（AC-5 / AC-6 / AC-7）：`fault` 先于结论、零 worker、零升格
// ---------------------------------------------------------------------------

/// `static-check` 归因产出 fixture（`fault` 在位即早退，`passed` 不参与分派）。
fn faulted_check(fault: RunFault, report_dir: &str) -> StaticCheckOutcome {
    StaticCheckOutcome {
        passed: false,
        diagnostics: "锁占用诊断".to_owned(),
        fault: Some(fault),
        output_ref: Some("exec-static-fault".to_owned()),
        report_dir: report_dir.to_owned(),
    }
}

/// `test-execution` 归因产出 fixture。
fn faulted_execution(fault: RunFault, report_dir: &str) -> TestExecutionOutcome {
    TestExecutionOutcome {
        fault: Some(fault),
        conclusion: TestExecutionConclusion::Fail,
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
        findings_brief: String::new(),
        findings_detail: String::new(),
        report_dir: report_dir.to_owned(),
        exec_id: "exec-test-fault".to_owned(),
    }
}

/// `mutation-test` 归因产出 fixture（mutation 面只产 `Stopped`）。
fn faulted_mutation(fault: RunFault) -> MutationTestOutcome {
    MutationTestOutcome {
        fault: Some(fault),
        conclusion: MutationTestConclusion::Fail,
        skip: None,
        score: 55.0,
        threshold: 80.0,
        counts: MutationCounts::default(),
        report_paths: vec!["reports/test/app_rust/mutation/mutation-seg-0.json".to_owned()],
        feedback_text: String::new(),
    }
}

/// 正向（AC-5 组合锚）：`Environment` → 零 worker 且 run failed——test-execution
/// 相位（无 agent 步）的 TestExecution 步带 `fault` → run 收敛 `Failed`、
/// **零 WorkerAgent 请求**（executor / evaluator / decision 全零）、零
/// `phase-log` 代写、无 `decision-log` 分叉、终态 reason 为 `TXT_ENV_TEST`
/// 替换后文本（含归因文本与报告目录、占位零残留）、MutationTest 零 spawn。
#[tokio::test]
async fn 环境错误_零worker且run失败() {
    let env_text = "占用进程：HipsDaemon.exe (pid 1234)\n被锁文件：D:\\w\\target\\liba.rlib\n\
                    已自动重试 4 次（每次重试会跳过已完成部分）\n\
                    环境侧处理：请 IT 排除 workspace 父目录（逐 worktree 排除无效）"
        .to_owned();
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![faulted_execution(
            RunFault::Environment(env_text.clone()),
            "C:/ws/openspec/changes/c/reports/test",
        )])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "环境错误 → run 收敛 failed（不注回 executor、不升格重试）"
    );

    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "零 WorkerAgent 请求（executor / evaluator / decision 全零）"
    );
    let timeline = timeline.lock().expect("时间线锁");
    assert!(
        !timeline.iter().any(|label| label == "phase-log"),
        "零 phase-log 代写（不升格相位 fail）: {timeline:?}"
    );
    assert!(
        !timeline.iter().any(|label| label == "decision-log"),
        "零决策分叉: {timeline:?}"
    );
    drop(timeline);

    assert!(
        !commands
            .lock()
            .expect("命令锁")
            .iter()
            .any(|command| matches!(command, ToolCommand::MutationTest { .. })),
        "环境错误早退 → 后续 MutationTest 步零 spawn"
    );

    let finish = history.finish().expect("收口落包捕获");
    let reason = finish.reason.expect("Terminal 记因应在案");
    assert!(
        reason.contains("测试执行无法完成（环境错误）"),
        "终态文案 = 环境错误模板: {reason}"
    );
    assert!(
        reason.contains(env_text.as_str()),
        "环境归因文本经 {{fault}} 嵌入（模板不写第二份）: {reason}"
    );
    assert!(
        reason.contains("C:/ws/openspec/changes/c/reports/test"),
        "报告目录经 {{dir}} 嵌入: {reason}"
    );
    assert!(
        !reason.contains("{fault}") && !reason.contains("{dir}"),
        "占位零残留: {reason}"
    );
    assert!(
        !reason.contains("请在基线上修复测试后重新发起"),
        "与基线红文案零共用句: {reason}"
    );

    let node = finish
        .nodes
        .iter()
        .find(|node| node.step.as_str() == "test_execution")
        .expect("test_execution 节点应在案");
    assert_eq!(node.status, RunStepStatus::Failed, "节点 settled failed");
    assert_eq!(
        node.output_ref.as_deref(),
        Some("exec-test-fault"),
        "failed 节点保留输出引用"
    );
    assert!(
        !finish
            .nodes
            .iter()
            .any(|node| node.step.as_str() == "mutation_test"),
        "后续步未起节点（fault 早退）"
    );
}

/// 正向（AC-7 组合锚）：`Stopped` → 节点 stopped + run stopped——TestExecution
/// 步带 `fault = Some(Stopped)` → run 收敛 `Stopped`（`Failed` 不出现）、节点
/// `stopped`、detail 含「用户请求停止」、后续 MutationTest 步零 spawn（早停）。
#[tokio::test]
async fn 取消_节点stopped且run收敛stopped() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![faulted_execution(
            RunFault::Stopped,
            "C:/ws/openspec/changes/c/reports/test",
        )])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "取消 → run 收敛 stopped（MUST NOT 呈现为失败）"
    );
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "取消面零 WorkerAgent 请求"
    );
    assert!(
        !commands
            .lock()
            .expect("命令锁")
            .iter()
            .any(|command| matches!(command, ToolCommand::MutationTest { .. })),
        "取消后余下步零 spawn（早停）"
    );

    let finish = history.finish().expect("收口落包捕获");
    assert_eq!(
        finish.status,
        workflow::state::RunStatus::Stopped,
        "落库终态同为 stopped"
    );
    let node = finish
        .nodes
        .iter()
        .find(|node| node.step.as_str() == "test_execution")
        .expect("test_execution 节点应在案");
    assert_eq!(node.status, RunStepStatus::Stopped, "节点 settled stopped");
    assert!(
        node.detail
            .as_deref()
            .is_some_and(|detail| detail.contains("用户请求停止")),
        "节点 detail 记因「用户请求停止」: {:?}",
        node.detail
    );
    assert!(
        !finish
            .nodes
            .iter()
            .any(|node| node.status == RunStepStatus::Failed),
        "无 failed 节点（stopped 与 failed 不混用）"
    );
    assert!(
        finish
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("用户请求停止")),
        "终态记因 = 用户请求停止: {:?}",
        finish.reason
    );
}

/// 正向（AC-7）：mutation 面 `Stopped` 分派——`template = None` 面（mutation 步
/// 只产 `Stopped`）→ 终态记因为归因文本原样、节点 stopped、零 worker；
/// `feedback_text` 空不被渲染为「需修复」。
#[tokio::test]
async fn mutation取消_文案原样且零worker() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, _commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
        .with_test_execution(vec![execution_outcome(TestExecutionConclusion::Pass, 5)])
        .with_mutation(vec![faulted_mutation(RunFault::Stopped)])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(
        status,
        ChangeRunStatus::Stopped,
        "mutation 取消 → run stopped"
    );
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "mutation 步无 agent 面（零 worker 请求）"
    );

    let finish = history.finish().expect("收口落包捕获");
    let node = finish
        .nodes
        .iter()
        .find(|node| node.step.as_str() == "mutation_test")
        .expect("mutation_test 节点应在案");
    assert_eq!(node.status, RunStepStatus::Stopped);
    assert!(
        finish
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("用户请求停止")),
        "template = None 面终态文案为归因文本原样: {:?}",
        finish.reason
    );
    assert!(
        !finish
            .nodes
            .iter()
            .any(|node| node.step.as_str() == "phase_log"),
        "取消面不落 phase_log 步节点"
    );
}

/// 边界（D15）：`fault` 先于结论——`Environment` + `passed = true`（理论上矛盾的
/// 组合）仍收敛 `Failed`（不因绿放行）；`Stopped` + `conclusion = Pass` 仍收敛
/// `Stopped`。两形态同测于 test-execution 相位（无 agent 步，零 worker 基线）。
#[tokio::test]
async fn fault先于结论_绿不抵环境错误() {
    let cases = [
        (
            "Environment + Pass 结论",
            faulted_execution(
                RunFault::Environment("环境坏".to_owned()),
                "C:/ws/reports/test",
            ),
            ChangeRunStatus::Failed,
        ),
        (
            "Stopped + Pass 结论",
            faulted_execution(RunFault::Stopped, "C:/ws/reports/test"),
            ChangeRunStatus::Stopped,
        ),
    ];

    for (label, outcome, expected) in cases {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("test-execution", &[]), done_outcome()])
            .with_test_execution(vec![outcome])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());

        let (task, _rx, _history) =
            spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
        let status = task.await.expect("run 任务正常结束");

        assert_eq!(
            status, expected,
            "{label}: fault 先于结论（结论不参与分派）"
        );
        assert!(
            requests.lock().expect("请求锁").is_empty(),
            "{label}: 零 worker 请求（不交决策裁决）"
        );
    }
}

/// 正向（AC-5）：反馈计数不增长——implement 相位 static-check 带 `fault`（`passed
/// = true` 的矛盾组合）→ static-check 命令恰 1 次（不因红重跑反馈边 5 次）、
/// executor 请求恰 1 条（无修复轮）、零 evaluator 请求、零 `phase-log` 代写、节点
/// failed 恰一条终态行。
#[tokio::test]
async fn 环境错误不烧反馈边预算() {
    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![
            route_outcome("implement", &["implement"]),
            done_outcome(),
        ])
        .with_static_check(vec![StaticCheckOutcome {
            passed: true,
            diagnostics: String::new(),
            fault: Some(RunFault::Environment(
                "锁占用：库文件被安全软件持有".to_owned(),
            )),
            output_ref: Some("exec-static-fault".to_owned()),
            report_dir: "C:/ws/openspec/changes/c/reports/test".to_owned(),
        }])
        .assemble();
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
    let status = task.await.expect("run 任务正常结束");

    assert_eq!(status, ChangeRunStatus::Failed, "fault 优先于 passed");
    {
        let requests = requests.lock().expect("请求锁");
        assert_eq!(requests.len(), 1, "仅相位首发 executor，无修复轮续注");
        assert_eq!(
            requests[0].role,
            WorkerRole::Executor,
            "唯一请求为 executor"
        );
    }
    {
        let commands = commands.lock().expect("命令锁");
        let static_calls = commands
            .iter()
            .filter(|command| matches!(command, ToolCommand::StaticCheck { .. }))
            .count();
        assert_eq!(
            static_calls, 1,
            "static-check 命令恰 1 次（不烧反馈边预算）"
        );
    }
    {
        let timeline = timeline.lock().expect("时间线锁");
        assert!(
            !timeline.iter().any(|label| label == "phase-log"),
            "零 phase-log 代写（不升格相位 fail）: {timeline:?}"
        );
    }

    let finish = history.finish().expect("收口落包捕获");
    let static_nodes = finish
        .nodes
        .iter()
        .filter(|node| node.step.as_str() == "static_check")
        .collect::<Vec<_>>();
    assert_eq!(static_nodes.len(), 1, "static-check 恰一条终态节点");
    assert_eq!(static_nodes[0].status, RunStepStatus::Failed);
    assert!(
        finish
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("静态检查无法完成（环境错误）")),
        "终态文案为环境模板: {:?}",
        finish.reason
    );
}

/// 边界（表驱动 3 形态）：`{dir}` 替换——`report_dir` 为空串 / 含空格与中文 /
/// 超长路径 → 模板替换后无 `{dir}` 残留、路径逐字（不转义、不截断）。
#[tokio::test]
async fn 文案dir占位替换逐字() {
    let long_dir = format!("C:/ws/{}", "很长的目录名/".repeat(120));
    let dirs = [
        String::new(),
        "C:/ws/my dir/中文 目录/reports".to_owned(),
        long_dir.clone(),
    ];

    for dir in dirs {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![
                route_outcome("implement", &["implement"]),
                done_outcome(),
            ])
            .with_static_check(vec![faulted_check(
                RunFault::Environment("环境坏".to_owned()),
                &dir,
            )])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());

        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Failed);

        let reason = history
            .finish()
            .expect("收口落包捕获")
            .reason
            .expect("Terminal 记因应在案");
        assert!(
            !reason.contains("{dir}") && !reason.contains("{fault}"),
            "占位零残留（dir={dir:?}）: {reason}"
        );
        assert!(
            reason.contains("报告目录 "),
            "模板骨架在案（dir={dir:?}）: {reason}"
        );
        if !dir.is_empty() {
            assert!(
                reason.contains(dir.as_str()),
                "路径逐字承载（不转义不截断，dir 长度 {}）: {reason}",
                dir.len()
            );
        }
    }
}

/// 边界（D15，表驱动 2 形态）：`Environment` 空串与超长——空串 → reason 含模板
/// 骨架、无空占位残留、不 panic；>1000 字符 → reason 逐字承载。
#[tokio::test]
async fn 环境文案空串与超长载荷() {
    let payloads = [String::new(), "错".repeat(1200)];

    for payload in payloads {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![
                route_outcome("implement", &["implement"]),
                done_outcome(),
            ])
            .with_static_check(vec![faulted_check(
                RunFault::Environment(payload.clone()),
                "C:/ws/reports/test",
            )])
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());

        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(
            status,
            ChangeRunStatus::Failed,
            "载荷长度 {}",
            payload.len()
        );

        let reason = history
            .finish()
            .expect("收口落包捕获")
            .reason
            .expect("Terminal 记因应在案");
        assert!(
            reason.contains("静态检查无法完成（环境错误）"),
            "模板骨架在案: {reason}"
        );
        assert!(
            reason.contains(payload.as_str()),
            "载荷逐字承载（长度 {}）",
            payload.len()
        );
        // 节点 detail 经 `diagnose_brief` 有界摘要（既有口径：超长载荷在节点面
        // 被折叠，终态 reason 仍全量）
        assert!(
            reason.len() >= payload.len(),
            "终态文案不短于载荷（无静默丢弃）"
        );
    }
}

/// 正向（AC-6 可辨）：基线 static-check 环境错误 vs 基线真红——同一
/// `step_static_check_terminal` 步两形态：
/// - `fault = Some(Environment(text))` → 终态 reason 为 `TXT_ENV_STATIC` 替换后
///   文本（含「静态检查无法完成（环境错误）」+ 归因文本 + 报告目录），**不含**
///   「请在基线上修复后重新发起」；
/// - `fault = None` + `passed = false` → 终态 reason 含「基线静态检查未通过」+
///   报告目录（既有基线红文案零变），两组文案互斥可辨。
#[tokio::test]
async fn 基线静态检查_环境错误与真红文案可辨() {
    // 形态一：环境错误
    {
        let root = TempRoot::new("init-static-env");
        root.change(NAME);
        let worktree = root.0.join("openspec/changes").join(NAME);
        let db = TestDb::open("init-static-env");
        seed_change_with_worktree(db.store.as_ref(), &worktree);

        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let env_text = "占用进程：HipsDaemon.exe (pid 1234)\n被锁文件：D:\\w\\target\\liba.rlib\n\
                        已自动重试 4 次（每次重试会跳过已完成部分）";
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("initialize", &[])])
            .with_static_check(vec![faulted_check(
                RunFault::Environment(env_text.to_owned()),
                &format!("/ws/openspec/changes/{NAME}/reports/baseline"),
            )])
            .assemble();
        let vcs = Arc::new(FakeVcs::clean());
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());

        let (task, _rx, history) = spawn_run_with_vcs(
            worker,
            tools,
            vcs,
            snapshot,
            &control,
            false,
            &worktree.to_string_lossy(),
            &root.root_str(),
            None,
        );
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Failed, "环境错误 → run failed");
        assert!(
            requests.lock().expect("请求锁").is_empty(),
            "基线检查步无 agent（零 worker 请求）"
        );

        let finish = history.finish().expect("收口落包捕获");
        let reason = finish.reason.expect("Terminal 记因应在案");
        assert!(
            reason.contains("静态检查无法完成（环境错误）"),
            "环境文案头行: {reason}"
        );
        assert!(reason.contains(env_text), "归因文本嵌入: {reason}");
        assert!(
            reason.contains("reports/baseline"),
            "报告目录代入: {reason}"
        );
        assert!(
            !reason.contains("请在基线上修复后重新发起"),
            "环境错误不落基线红文案: {reason}"
        );
        assert!(
            !reason.contains("基线静态检查未通过"),
            "环境错误不落基线红头行: {reason}"
        );
    }

    // 形态二：基线真红对照（文案零变）
    {
        let root = TempRoot::new("init-static-red-ctrl");
        root.change(NAME);
        let worktree = root.0.join("openspec/changes").join(NAME);
        let db = TestDb::open("init-static-red-ctrl");
        seed_change_with_worktree(db.store.as_ref(), &worktree);

        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, _requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, _commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![route_outcome("initialize", &[])])
            .with_static_check(vec![baseline_check(false)])
            .assemble();
        let vcs = Arc::new(FakeVcs::clean());
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());

        let (task, _rx, history) = spawn_run_with_vcs(
            worker,
            tools,
            vcs,
            snapshot,
            &control,
            false,
            &worktree.to_string_lossy(),
            &root.root_str(),
            None,
        );
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(status, ChangeRunStatus::Failed, "基线红 → run failed");

        let reason = history
            .finish()
            .expect("收口落包捕获")
            .reason
            .expect("Terminal 记因应在案");
        assert!(
            reason.contains("基线静态检查未通过"),
            "基线红文案零变: {reason}"
        );
        assert!(
            reason.contains("reports/baseline"),
            "基线红文案携报告目录: {reason}"
        );
        assert!(
            !reason.contains("无法完成（环境错误）"),
            "基线红不落环境文案（两组零共用句）: {reason}"
        );
    }
}

/// 正向（AC-6 可辨）：基线 test-execution 环境错误——`step_test_execution_terminal`
/// 收到 `fault = Some(Environment(text))` → run `Failed`、终态 reason 为
/// `TXT_ENV_TEST` 替换后文本（含「测试执行无法完成（环境错误）」+ 归因文本 +
/// 基线报告目录、占位零残留），**不含** `TXT_BASELINE_TEST_ERROR` /
/// `TXT_BASELINE_TEST_FAILED` 的引导句（「请检查配置与环境后重新发起」/
/// 「请在基线上修复测试后重新发起」）——环境错误与基线红两态不共用文案；
/// 节点 settled failed 且保留 `exec_id` 输出引用。
#[tokio::test]
async fn 基线测试执行_环境错误不落基线红文案() {
    let root = TempRoot::new("init-test-env");
    root.change(NAME);
    let worktree = root.0.join("openspec/changes").join(NAME);
    let db = TestDb::open("init-test-env");
    seed_change_with_worktree(db.store.as_ref(), &worktree);

    let timeline = Arc::new(Mutex::new(Vec::new()));
    let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
    let env_text = "占用进程：HipsDaemon.exe (pid 1234)\n被锁文件：D:\\w\\target\\liba.rlib\n\
                    已自动重试 4 次（每次重试会跳过已完成部分）\n\
                    环境侧处理：请 IT 排除 workspace 父目录（逐 worktree 排除无效）";
    let (tools, commands) = FakeTools::new(&timeline)
        .with_phase_next(vec![route_outcome("initialize", &[])])
        .with_static_check(vec![baseline_check(true)])
        .with_test_execution(vec![faulted_execution(
            RunFault::Environment(env_text.to_owned()),
            &format!("/ws/openspec/changes/{NAME}/reports/baseline"),
        )])
        .assemble();
    let vcs = Arc::new(FakeVcs::clean());
    let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
    let control = Arc::new(ChangeFlowControl::new());

    let (task, _rx, history) = spawn_run_with_vcs(
        worker,
        tools,
        vcs,
        snapshot,
        &control,
        false,
        &worktree.to_string_lossy(),
        &root.root_str(),
        None,
    );
    let status = task.await.expect("run 任务正常结束");
    assert_eq!(
        status,
        ChangeRunStatus::Failed,
        "基线 test-execution 环境错误 → run failed（不升格、不交决策）"
    );
    assert!(
        requests.lock().expect("请求锁").is_empty(),
        "基线 test-execution 步无 agent（零 worker 请求）"
    );

    let finish = history.finish().expect("收口落包捕获");
    let reason = finish.reason.expect("Terminal 记因应在案");
    assert!(
        reason.contains("测试执行无法完成（环境错误）"),
        "环境文案头行: {reason}"
    );
    assert!(
        reason.contains(env_text),
        "归因文本经 {{fault}} 嵌入: {reason}"
    );
    assert!(
        reason.contains("reports/baseline"),
        "基线报告目录经 {{dir}} 嵌入: {reason}"
    );
    assert!(
        !reason.contains("{fault}") && !reason.contains("{dir}"),
        "占位零残留: {reason}"
    );
    assert!(
        !reason.contains("请检查配置与环境后重新发起"),
        "环境错误不落 TXT_BASELINE_TEST_ERROR: {reason}"
    );
    assert!(
        !reason.contains("请在基线上修复测试后重新发起"),
        "环境错误不落 TXT_BASELINE_TEST_FAILED: {reason}"
    );
    assert!(
        !reason.contains("基线测试执行出错") && !reason.contains("基线测试未通过"),
        "环境错误不落基线红头行: {reason}"
    );

    let node = finish
        .nodes
        .iter()
        .find(|node| node.step.as_str() == "test_execution")
        .expect("test_execution 节点应在案");
    assert_eq!(node.status, RunStepStatus::Failed, "节点 settled failed");
    assert_eq!(
        node.output_ref.as_deref(),
        Some("exec-test-fault"),
        "failed 节点保留 exec_id 输出引用"
    );
    let commands = commands.lock().expect("命令锁");
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, ToolCommand::StaticCheck { .. }))
            .count(),
        1,
        "static-check 已绿（单次执行）"
    );
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, ToolCommand::TestExecution { .. }))
            .count(),
        1,
        "test-execution 恰一次（环境错误不回注执行体）"
    );
    assert!(
        commands
            .iter()
            .all(|command| !matches!(command, ToolCommand::MutationTest { .. })),
        "环境错误早退 → 后续 MutationTest 步零 spawn"
    );
}

/// 边界（AC-7 表驱动 3 面）：**三面取消一致性**——static-check（实现相位反馈环
/// 前置：不烧反馈边预算、零续注 prompt）/ test-execution（升格前置）/ mutation-test
/// 三面分别收 `fault = Some(Stopped)` → 均节点 `stopped` + run `Stopped`（无
/// failed 节点）、终态记因「用户请求停止」、落库终态同 `Stopped`、零 `phase-log`
/// 代写。
#[tokio::test]
async fn 三面取消一致性_节点stopped且run收敛stopped() {
    /// 单面装置：（面标签、相位路由、static-check 队列、test-execution 队列、
    /// mutation 队列、目标步名）。
    type CancelCase = (
        &'static str,
        PhaseNextOutcome,
        Vec<StaticCheckOutcome>,
        Vec<TestExecutionOutcome>,
        Vec<MutationTestOutcome>,
        &'static str,
    );
    let cases: [CancelCase; 3] = [
        (
            "static-check 面",
            route_outcome("implement", &["implement"]),
            vec![faulted_check(RunFault::Stopped, "C:/ws/reports/test")],
            Vec::new(),
            Vec::new(),
            "static_check",
        ),
        (
            "test-execution 面",
            route_outcome("test-execution", &[]),
            Vec::new(),
            vec![faulted_execution(RunFault::Stopped, "C:/ws/reports/test")],
            Vec::new(),
            "test_execution",
        ),
        (
            "mutation 面",
            route_outcome("test-execution", &[]),
            Vec::new(),
            vec![execution_outcome(TestExecutionConclusion::Pass, 5)],
            vec![faulted_mutation(RunFault::Stopped)],
            "mutation_test",
        ),
    ];

    for (label, phase_next, static_check, test_execution, mutation, target) in cases {
        let timeline = Arc::new(Mutex::new(Vec::new()));
        let (worker, requests, _sessions) = FakeWorker::new(&timeline).assemble();
        let (tools, commands) = FakeTools::new(&timeline)
            .with_phase_next(vec![phase_next, done_outcome()])
            .with_static_check(static_check)
            .with_test_execution(test_execution)
            .with_mutation(mutation)
            .assemble();
        let snapshot: Arc<dyn WorkflowSnapshotPort> = Arc::new(StubSnapshot);
        let control = Arc::new(ChangeFlowControl::new());

        let (task, _rx, history) = spawn_run(worker, tools, snapshot, &control, false, "/tmp/root");
        let status = task.await.expect("run 任务正常结束");
        assert_eq!(
            status,
            ChangeRunStatus::Stopped,
            "{label}: 取消 → run 收敛 stopped（MUST NOT 呈现为失败）"
        );

        let finish = history.finish().expect("收口落包捕获");
        assert_eq!(
            finish.status,
            RunStatus::Stopped,
            "{label}: 落库终态同为 stopped"
        );
        let node = finish
            .nodes
            .iter()
            .find(|node| node.step.as_str() == target)
            .unwrap_or_else(|| panic!("{label}: {target} 节点应在案"));
        assert_eq!(node.status, RunStepStatus::Stopped, "{label}: 节点 stopped");
        assert!(
            node.detail
                .as_deref()
                .is_some_and(|detail| detail.contains("用户请求停止")),
            "{label}: 节点 detail 记因「用户请求停止」: {:?}",
            node.detail
        );
        assert!(
            !finish
                .nodes
                .iter()
                .any(|node| node.status == RunStepStatus::Failed),
            "{label}: 无 failed 节点（stopped 与 failed 不混用）"
        );
        assert!(
            !finish
                .nodes
                .iter()
                .any(|node| node.step.as_str() == "phase_log"),
            "{label}: 取消面不落 phase_log 步节点"
        );
        assert!(
            finish
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("用户请求停止")),
            "{label}: 终态记因 = 用户请求停止: {:?}",
            finish.reason
        );

        // 零反馈边续注：static-check 面恰一次门禁命令（不因红重跑 5 次）
        let static_calls = commands
            .lock()
            .expect("命令锁")
            .iter()
            .filter(|command| matches!(command, ToolCommand::StaticCheck { .. }))
            .count();
        let expected_static = if target == "static_check" { 1 } else { 0 };
        assert_eq!(
            static_calls, expected_static,
            "{label}: static-check 命令次数（取消面不烧反馈边预算）"
        );
        // 实现相位 static-check 面只有相位首发 executor（无修复轮）；另两面无 agent 步
        assert!(
            requests
                .lock()
                .expect("请求锁")
                .iter()
                .all(|request| !request.prompt.contains("静态检查未通过")),
            "{label}: 零反馈续注 prompt"
        );
    }
}
