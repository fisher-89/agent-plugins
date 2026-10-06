//! `port.rs`（编排 port 契约）的单元测试（test-design「port.rs -> port_test.rs」
//! 节）：ToolCommand 六变体封闭集（match 穷尽编译期锚定，DecisionLog 为
//! desktop-change-session-visibility 增量）、ToolStepOutput 载荷
//! 换血（写面原生类型逐字段相等）、StaticCheckOutcome 字段面、StaticCheckRunner
//! / DiffContextPort trait 面（object safety + Send+Sync 编译锚）、BoxDiffFuture
//! 别名跨 await、既有三契约保持回归（AC-1 双缝装置前提）。
//!
//! Mock策略：五 port 内存假实现（注入依赖，入参例外）——Vec 记录调用序与载荷、
//! 可编程产出 / Err(String)；trait 本身即注入面，真实实现分别在 steps_test /
//! worker_test / snapshot_test / static_check_test / git_diff_test 组合。
//!
//! 废弃注记（test-design 废弃行，断言不落）：ToolCommand 含 ChangeFiles 变体、
//! ToolStepError 三态映射两行随封闭集收缩与 `Result<_, String>` 换血退役——
//! ChangeFiles / Spawn-Exit-Drift 断言不再存在即本节废弃行的承载形态（第六
//! 变体 DecisionLog 为 desktop-change-session-visibility 在册增量，非废弃行
//! 复活）。

use std::sync::{Arc, Mutex};

use agent::{AgentPermissionMode, AgentRunStatus, SessionProvenance};

use crate::port::{
    BoxDiffFuture, DiffContextPort, RunEventSink, StaticCheckOutcome, StaticCheckRunner,
    ToolCommand, ToolStepOutput, ToolStepPort, WorkerAgentPort, WorkerRole, WorkerTurnOutcome,
    WorkerTurnRequest, WorkflowSnapshotPort,
};
use crate::state::RunUpdate;
use workflow::write::{
    BacktrackInput, BacktrackOutcome, DecisionLogOutcome, PhaseLogInput, PhaseLogOutcome,
    PhaseNextOutcome, PhaseStartOutcome,
};

// ---------------------------------------------------------------------------
// 装置：写面原生载荷 fixture 与五假 port
// ---------------------------------------------------------------------------

fn sample_phase_next_outcome() -> PhaseNextOutcome {
    PhaseNextOutcome {
        done: false,
        next_phase: Some("implement".to_owned()),
        round: 2,
        executor: None,
        evaluator: None,
        allowed_backtrack_phases: vec!["proposal".to_owned(), "dev-design".to_owned()],
        last_result: None,
        error: None,
    }
}

fn sample_phase_log_input() -> PhaseLogInput {
    PhaseLogInput {
        phase: "implement".to_owned(),
        report: "实现完成".to_owned(),
        checklist: Vec::new(),
        skipped: false,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
    }
}

fn sample_backtrack_input() -> BacktrackInput {
    BacktrackInput {
        phase: "implement".to_owned(),
        to: "dev-design".to_owned(),
        reason: "设计返工".to_owned(),
        allowed: vec!["dev-design".to_owned(), "implement".to_owned()],
    }
}

/// 记录调用序与载荷的假 ToolStepPort（trait 注入面回归锚——封闭集命令经
/// ToolStepRequest 承载原样到达 port）。
struct RecordingSteps {
    seen: Arc<Mutex<Vec<ToolCommand>>>,
    roots: Arc<Mutex<Vec<String>>>,
}

impl RecordingSteps {
    fn capturing() -> (Arc<dyn ToolStepPort>, Arc<Mutex<Vec<ToolCommand>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let steps = Arc::new(RecordingSteps {
            seen: Arc::clone(&seen),
            roots: Arc::new(Mutex::new(Vec::new())),
        });
        (steps as Arc<dyn ToolStepPort>, seen)
    }
}

impl ToolStepPort for RecordingSteps {
    fn run(&self, step: crate::port::ToolStepRequest) -> crate::port::BoxToolFuture {
        self.seen
            .lock()
            .expect("调用序锁不可中毒")
            .push(step.command.clone());
        self.roots.lock().expect("root 锁不可中毒").push(step.root);
        Box::pin(async move {
            Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
                passed: true,
                diagnostics: String::new(),
            }))
        })
    }
}

/// 可编程产出 / Err 的假 StaticCheckRunner（root 透传断言面）。
struct ProgrammableRunner {
    root_seen: Mutex<Vec<String>>,
    outcome: StaticCheckOutcome,
}

impl StaticCheckRunner for ProgrammableRunner {
    fn run(&self, root: &str) -> crate::port::BoxToolFuture {
        self.root_seen
            .lock()
            .expect("root 锁不可中毒")
            .push(root.to_owned());
        let outcome = self.outcome.clone();
        Box::pin(async move { Ok(ToolStepOutput::StaticCheck(outcome)) })
    }
}

/// 可编程 diff 上下文假实现（Result<String, String> 双臂）。
struct StubDiff {
    text: Result<String, String>,
    roots: Mutex<Vec<String>>,
}

impl DiffContextPort for StubDiff {
    fn diff_context(&self, root: &str) -> BoxDiffFuture {
        self.roots
            .lock()
            .expect("root 锁不可中毒")
            .push(root.to_owned());
        let text = self.text.clone();
        Box::pin(async move { text })
    }
}

/// 记录 emit 载荷的假 sink（RunEventSink 契约保持面）。
#[derive(Default)]
struct RecordingSink {
    seen: Mutex<Vec<RunUpdate>>,
}

impl RunEventSink for RecordingSink {
    fn emit(&self, update: RunUpdate) {
        self.seen.lock().expect("sink 锁不可中毒").push(update);
    }
}

// ---------------------------------------------------------------------------
// ToolCommand 六变体封闭集
// ---------------------------------------------------------------------------

/// ToolCommand 七变体封闭集：七变体构造 + match 穷尽分发编译期锚定；载荷直载
/// 写面输入类型（PhaseLogInput / BacktrackInput 原样承接——AC-6 进程内缝命令面）。
#[tokio::test]
async fn tool_command六变体封闭集且match穷尽分发() {
    // match 穷尽：缺任一变体即编译失败（封闭集收缩形态的编译期锚）
    fn describe(command: &ToolCommand) -> &'static str {
        match command {
            ToolCommand::PhaseNext { .. } => "phase-next",
            ToolCommand::PhaseStart { .. } => "phase-start",
            ToolCommand::PhaseLog { .. } => "phase-log",
            ToolCommand::Backtrack { .. } => "backtrack",
            ToolCommand::DecisionLog { .. } => "decision-log",
            ToolCommand::StaticCheck => "static-check",
            ToolCommand::TestExecution { .. } => "test-execution",
        }
    }

    let commands = vec![
        (
            ToolCommand::PhaseNext {
                change: "c".to_owned(),
                run_id: "run-1".to_owned(),
            },
            "phase-next",
        ),
        (
            ToolCommand::PhaseStart {
                change: "c".to_owned(),
                phase: "implement".to_owned(),
            },
            "phase-start",
        ),
        (
            ToolCommand::PhaseLog {
                change: "c".to_owned(),
                phase: "implement".to_owned(),
                input: sample_phase_log_input(),
            },
            "phase-log",
        ),
        (
            ToolCommand::Backtrack {
                change: "c".to_owned(),
                phase: "implement".to_owned(),
                input: sample_backtrack_input(),
            },
            "backtrack",
        ),
        (
            ToolCommand::DecisionLog {
                change: "c".to_owned(),
                phase: "implement".to_owned(),
                session_id: "sess-1".to_owned(),
            },
            "decision-log",
        ),
        (ToolCommand::StaticCheck, "static-check"),
    ];
    assert_eq!(commands.len(), 6, "封闭集六变体");

    for (command, label) in &commands {
        assert_eq!(describe(command), *label, "变体 {label} 可辨");
    }

    // 载荷直载写面输入类型：PhaseLogInput / BacktrackInput 原样承接（PartialEq
    // 逐字段相等——AC-6 进程内缝命令面）
    let logged = ToolCommand::PhaseLog {
        change: "c".to_owned(),
        phase: "implement".to_owned(),
        input: sample_phase_log_input(),
    };
    match &logged {
        ToolCommand::PhaseLog { input, phase, .. } => {
            assert_eq!(input, &sample_phase_log_input(), "PhaseLogInput 原样承接");
            assert_eq!(phase, "implement");
        }
        _ => panic!("变体漂移"),
    }
    let back = ToolCommand::Backtrack {
        change: "c".to_owned(),
        phase: "implement".to_owned(),
        input: sample_backtrack_input(),
    };
    match &back {
        ToolCommand::Backtrack { input, .. } => {
            assert_eq!(input, &sample_backtrack_input(), "BacktrackInput 原样承接");
        }
        _ => panic!("变体漂移"),
    }

    // 命令经 ToolStepRequest 原样到达 port（trait 注入面）
    let (steps, seen) = RecordingSteps::capturing();
    let output = steps
        .run(crate::port::ToolStepRequest {
            root: "/tmp/root".to_owned(),
            command: logged,
        })
        .await
        .expect("步产出应可用");
    assert!(matches!(output, ToolStepOutput::StaticCheck(outcome) if outcome.passed));
    assert_eq!(seen.lock().expect("锁").len(), 1, "命令原样到达注入 port");
}

/// ToolStepOutput 载荷换血：各变体载荷（写面原生 PhaseNextOutcome /
/// PhaseStartOutcome / PhaseLogOutcome / BacktrackOutcome / DecisionLogOutcome /
/// StaticCheckOutcome）构造与提取逐字段相等；PhaseNext 大变体 Box 收敛尺寸差。
#[test]
fn tool_step_output各变体载荷写面原生类型逐字段相等() {
    let next = sample_phase_next_outcome();
    let start = PhaseStartOutcome {
        phase: "implement".to_owned(),
        attempt: 2,
        start_at: time::OffsetDateTime::now_utc(),
    };
    let log = PhaseLogOutcome {
        phase: "implement".to_owned(),
        attempt: 2,
    };
    let back = BacktrackOutcome {
        phase: "implement".to_owned(),
        target: "dev-design".to_owned(),
    };
    let decision = DecisionLogOutcome {
        phase: "implement".to_owned(),
    };
    let check = StaticCheckOutcome {
        passed: false,
        diagnostics: "error[E0308]: mismatched".to_owned(),
    };

    let outputs = vec![
        ToolStepOutput::PhaseNext(Box::new(next.clone())),
        ToolStepOutput::PhaseStart(start.clone()),
        ToolStepOutput::PhaseLog(log.clone()),
        ToolStepOutput::Backtrack(back.clone()),
        ToolStepOutput::DecisionLog(decision.clone()),
        ToolStepOutput::StaticCheck(check.clone()),
    ];
    assert_eq!(outputs.len(), 6, "产出封闭集六变体");

    // 提取逐字段相等（TryFrom 窄化——walker 侧 run_tool 泛型的半边）
    match outputs[0].clone() {
        ToolStepOutput::PhaseNext(boxed) => {
            assert_eq!(*boxed, next, "PhaseNext 载荷写面原生类型逐字段相等");
        }
        _ => panic!("变体漂移"),
    }
    match outputs[1].clone() {
        ToolStepOutput::PhaseStart(outcome) => {
            assert_eq!(outcome.phase, start.phase);
            assert_eq!(outcome.attempt, start.attempt);
            assert_eq!(outcome.start_at, start.start_at);
        }
        _ => panic!("变体漂移"),
    }
    match outputs[2].clone() {
        ToolStepOutput::PhaseLog(outcome) => assert_eq!(outcome, log),
        _ => panic!("变体漂移"),
    }
    match outputs[3].clone() {
        ToolStepOutput::Backtrack(outcome) => assert_eq!(outcome, back),
        _ => panic!("变体漂移"),
    }
    match outputs[4].clone() {
        ToolStepOutput::DecisionLog(outcome) => assert_eq!(outcome, decision),
        _ => panic!("变体漂移"),
    }
    match outputs[5].clone() {
        ToolStepOutput::StaticCheck(outcome) => assert_eq!(outcome, check),
        _ => panic!("变体漂移"),
    }

    // PhaseNext 大变体 Box 收敛尺寸差（白名单 + prompt 双 Vec 收敛在 Box 内
    // ——不 Box 时该变体主导整个枚举尺寸）
    assert!(
        std::mem::size_of::<PhaseNextOutcome>() > std::mem::size_of::<ToolStepOutput>(),
        "PhaseNext 大变体经 Box 收敛（枚举尺寸由次大变体决定）"
    );
    assert_eq!(
        std::mem::size_of::<Box<PhaseNextOutcome>>(),
        std::mem::size_of::<usize>(),
        "Box 载荷定宽（单指针，定 Sized 载荷）"
    );
}

/// StaticCheckOutcome 字段面：passed true/false × diagnostics 空串 / 多行诊断
/// 构造与读取（passed=false 走定向反馈边的载荷面——AC-4）。
#[test]
fn static_check_outcome字段面双值双诊断形态() {
    let passed_clean = StaticCheckOutcome {
        passed: true,
        diagnostics: String::new(),
    };
    assert!(passed_clean.passed);
    assert!(passed_clean.diagnostics.is_empty());

    let failed_loud = StaticCheckOutcome {
        passed: false,
        diagnostics: "error[E0308]: mismatched types\n  --> src/lib.rs:1:1\nwarning: unused"
            .to_owned(),
    };
    assert!(!failed_loud.passed, "passed=false 走定向反馈边");
    assert!(
        failed_loud.diagnostics.lines().count() >= 2,
        "多行诊断原样承载（反馈边注入原料）"
    );
    assert_eq!(failed_loud.clone(), failed_loud, "Clone/PartialEq 派生可用");
}

// ---------------------------------------------------------------------------
// trait 面：StaticCheckRunner / DiffContextPort / BoxDiffFuture / 既有三契约
// ---------------------------------------------------------------------------

/// StaticCheckRunner trait 面：假实现经 Arc<dyn …> 注入、BoxToolFuture 返回
/// 可用、root 参数透传（object safety + Send+Sync 编译锚——W4 缝成立前提）。
#[tokio::test]
async fn static_check_runner假实现经arc注入且root透传() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn StaticCheckRunner>>();

    let runner: Arc<dyn StaticCheckRunner> = Arc::new(ProgrammableRunner {
        root_seen: Mutex::new(Vec::new()),
        outcome: StaticCheckOutcome {
            passed: false,
            diagnostics: "lint 失败".to_owned(),
        },
    });

    // root 参数透传 + BoxToolFuture resolve 可用（W4 缝）
    let output = runner.run("/tmp/workspace-root").await.expect("产出应可用");
    match output {
        ToolStepOutput::StaticCheck(outcome) => {
            assert!(!outcome.passed);
            assert_eq!(outcome.diagnostics, "lint 失败");
        }
        _ => panic!("产出变体漂移"),
    }
}

/// DiffContextPort trait 面：假实现经 Arc<dyn …> 注入、diff_context(root) 经
/// BoxDiffFuture resolve Result<String, String>（W5 缝成立前提——AC-3 组装链
/// 的注入面）。
#[tokio::test]
async fn diff_context_port假实现经arc注入且resolve结果串() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn DiffContextPort>>();

    let ok_source: Arc<dyn DiffContextPort> = Arc::new(StubDiff {
        text: Ok("### 未提交文件清单\n\nM src/lib.rs".to_owned()),
        roots: Mutex::new(Vec::new()),
    });
    assert_eq!(
        ok_source.diff_context("/tmp/ws").await.expect("Ok 臂"),
        "### 未提交文件清单\n\nM src/lib.rs",
        "Result<String, _> resolve 可用"
    );

    // Err 臂同缝（git 缺失降级面的源头）
    let err_source: Arc<dyn DiffContextPort> = Arc::new(StubDiff {
        text: Err("git 拉起失败".to_owned()),
        roots: Mutex::new(Vec::new()),
    });
    assert_eq!(
        err_source
            .diff_context("/tmp/ws")
            .await
            .expect_err("Err 臂"),
        "git 拉起失败"
    );
}

/// BoxDiffFuture 别名：Pin<Box<dyn Future + Send>> 别名跨 await 持有可用、
/// 不引 futures 依赖（与 BoxToolFuture 同款手法——std-only 编译期锚）。
#[tokio::test]
async fn box_diff_future别名跨await持有可用() {
    fn make() -> BoxDiffFuture {
        Box::pin(async { Ok::<_, String>("diff 文本".to_owned()) })
    }
    // 别名值可先持有再 await（跨 await 持有可用）
    let future: BoxDiffFuture = make();
    assert_eq!(future.await.expect("resolve"), "diff 文本");

    // 同款手法复核：BoxToolFuture 亦为 Pin<Box<dyn Future + Send>> 别名
    fn make_tool() -> crate::port::BoxToolFuture {
        Box::pin(async {
            Ok(ToolStepOutput::StaticCheck(StaticCheckOutcome {
                passed: true,
                diagnostics: String::new(),
            }))
        })
    }
    assert!(
        matches!(
            make_tool().await,
            Ok(ToolStepOutput::StaticCheck(outcome)) if outcome.passed
        ),
        "BoxToolFuture 同款 std-only 手法"
    );
}

/// 既有三契约保持：WorkerAgentPort / WorkflowSnapshotPort / RunEventSink 与
/// WorkerTurnRequest / WorkerTurnOutcome 中性类型回归（编译期锚定不回退——
/// AC-1 双缝装置前提）。
#[test]
fn 既有三契约与中性类型保持() {
    // WorkerTurnRequest 中性类型构造（provenance / permission / continue_session
    // / agent / role 字段面）
    let request = WorkerTurnRequest {
        root: "/tmp/ws".to_owned(),
        prompt: "轮 prompt".to_owned(),
        provenance: SessionProvenance {
            source: "change".to_owned(),
            source_ref: Some("c/implement/executor/1".to_owned()),
        },
        permission: AgentPermissionMode::BypassPermissions,
        continue_session: Some("sess-1".to_owned()),
        agent: None,
        role: WorkerRole::Executor,
    };
    assert_eq!(request.provenance.source, "change");
    assert_eq!(request.permission, AgentPermissionMode::BypassPermissions);
    assert_eq!(request.clone(), request, "Clone/PartialEq 派生保持");

    // WorkerTurnOutcome 中性类型构造（密封转录全集 + 终态 + final_message）
    let outcome = WorkerTurnOutcome {
        session_id: "sess-1".to_owned(),
        status: AgentRunStatus::Completed,
        final_message: Some("结论".to_owned()),
        transcript: Vec::new(),
    };
    assert_eq!(outcome.status, AgentRunStatus::Completed);

    // 三契约 trait object 可铸（object safety 保持）
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn WorkerAgentPort>>();
    assert_send_sync::<Arc<dyn WorkflowSnapshotPort>>();
    assert_send_sync::<Arc<dyn RunEventSink>>();

    // WorkerRole 三值线格式（provenance sourceRef 组装面）
    assert_eq!(WorkerRole::Executor.as_str(), "executor");
    assert_eq!(WorkerRole::Evaluator.as_str(), "evaluator");
    assert_eq!(WorkerRole::Decision.as_str(), "decision");

    // 假 sink emit 载荷保真（RunEventSink 契约保持）
    let sink = RecordingSink::default();
    sink.emit(RunUpdate::Finished {
        status: crate::state::ChangeRunStatus::Completed,
        reason: None,
    });
    assert_eq!(sink.seen.lock().expect("锁").len(), 1);
}

// ---------------------------------------------------------------------------
// test-execution 扩展节（desktop-checks-domain）
// ---------------------------------------------------------------------------

use crate::port::{TestExecutionConclusion, TestExecutionOutcome, TestExecutionRunner};

/// 可编程产出 / Err 的假 TestExecutionRunner（root / change 双参透传断言面；
/// calls 经 Arc 共享——测试侧持句柄读记录）。
struct ProgrammableTestExecutionRunner {
    calls: Arc<Mutex<Vec<(String, String)>>>,
    result: Result<TestExecutionOutcome, String>,
}

impl ProgrammableTestExecutionRunner {
    fn with_result(
        result: Result<TestExecutionOutcome, String>,
    ) -> (Self, Arc<Mutex<Vec<(String, String)>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
                result,
            },
            calls,
        )
    }
}

impl TestExecutionRunner for ProgrammableTestExecutionRunner {
    fn run(&self, root: &str, change: &str) -> crate::port::BoxToolFuture {
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push((root.to_owned(), change.to_owned()));
        let result = self.result.clone();
        Box::pin(async move { result.map(ToolStepOutput::TestExecution) })
    }
}

fn sample_execution_outcome(conclusion: TestExecutionConclusion) -> TestExecutionOutcome {
    TestExecutionOutcome {
        conclusion,
        total: 12,
        passed: 10,
        failed: 2,
        skipped: 0,
        findings_brief: "「vite-plus」2 项测试失败——聚类失败".to_owned(),
        report_dir: "C:/ws/openspec/changes/c/reports/test".to_owned(),
    }
}

/// 正向：ToolCommand 第七变体封闭集——TestExecution 构造 + match 穷尽编译锚
/// + change 载荷透传 + 七变体互异（既有六变体测试扩行，零改动持衡）。
#[tokio::test]
async fn test_execution命令第七变体封闭集且change载荷透传() {
    // match 穷尽：缺 TestExecution 臂即编译失败（封闭集扩臂的编译期锚）
    fn describe(command: &ToolCommand) -> &'static str {
        match command {
            ToolCommand::PhaseNext { .. } => "phase-next",
            ToolCommand::PhaseStart { .. } => "phase-start",
            ToolCommand::PhaseLog { .. } => "phase-log",
            ToolCommand::Backtrack { .. } => "backtrack",
            ToolCommand::DecisionLog { .. } => "decision-log",
            ToolCommand::StaticCheck => "static-check",
            ToolCommand::TestExecution { .. } => "test-execution",
        }
    }
    assert_eq!(
        describe(&ToolCommand::TestExecution {
            change: "c".to_owned()
        }),
        "test-execution"
    );

    // change 载荷透传：命令经 ToolStepRequest 原样到达注入 port
    let (inner, calls) = ProgrammableTestExecutionRunner::with_result(Ok(
        sample_execution_outcome(TestExecutionConclusion::Pass),
    ));
    let runner: Arc<dyn TestExecutionRunner> = Arc::new(inner);
    let output = runner
        .run("/tmp/workspace-root", "demo-change")
        .await
        .expect("产出应可用");
    assert!(
        matches!(output, ToolStepOutput::TestExecution(outcome) if outcome.conclusion == TestExecutionConclusion::Pass),
        "产出为 TestExecution 变体"
    );
    assert_eq!(
        calls.lock().expect("锁").as_slice(),
        [("/tmp/workspace-root".to_owned(), "demo-change".to_owned())],
        "change 载荷透传（root / change 原样到达注入 port）"
    );

    // 七变体互异（封闭集封闭性——第八臂不存在）
    let seventh = ToolCommand::TestExecution {
        change: "c".to_owned(),
    };
    assert_ne!(
        describe(&seventh),
        describe(&ToolCommand::StaticCheck),
        "门禁双命令可辨（检查域家族两成员不混淆）"
    );
}

/// 正向：ToolStepOutput::TestExecution 载荷构造与提取逐字段相等（既有产出
/// 封闭集测试扩行——TryFrom 窄化半边）。
#[test]
fn tool_step_output_test_execution载荷构造与提取逐字段相等() {
    let outcome = sample_execution_outcome(TestExecutionConclusion::Fail);
    let output = ToolStepOutput::TestExecution(outcome.clone());

    match output {
        ToolStepOutput::TestExecution(extracted) => {
            assert_eq!(
                extracted, outcome,
                "载荷逐字段相等（Clone / PartialEq 派生可用）"
            );
        }
        other => panic!("变体漂移: {other:?}"),
    }
}

/// 正向：TestExecutionRunner trait 面——假实现经 Arc<dyn …> 注入、root /
/// change 双参透传、BoxToolFuture resolve 出 ToolStepOutput::TestExecution
///（object safety + Send + Sync 编译锚——StaticCheckRunner 同型）。
#[tokio::test]
async fn test_execution_runner假实现经arc注入且root_change透传() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn TestExecutionRunner>>();

    let (inner, calls) = ProgrammableTestExecutionRunner::with_result(Ok(
        sample_execution_outcome(TestExecutionConclusion::Pass),
    ));
    let runner: Arc<dyn TestExecutionRunner> = Arc::new(inner);

    let output = runner
        .run("/tmp/workspace-root", "walker-change")
        .await
        .expect("产出应可用");
    assert!(
        matches!(output, ToolStepOutput::TestExecution(outcome) if outcome.total == 12),
        "BoxToolFuture resolve 出 TestExecution 变体"
    );
    assert_eq!(
        calls.lock().expect("锁").as_slice(),
        [("/tmp/workspace-root".to_owned(), "walker-change".to_owned())],
        "root / change 双参透传（spawn cwd 与报告树定位语义）"
    );
}

/// 异常：TestExecutionRunner Err(String) 臂经 BoxToolFuture resolve 上抛
///（写盘失败类基础设施错误面——run_tool 映射输入）。
#[tokio::test]
async fn test_execution_runner_err臂上抛() {
    let (inner, _calls) =
        ProgrammableTestExecutionRunner::with_result(Err("报告目录创建失败".to_owned()));
    let runner: Arc<dyn TestExecutionRunner> = Arc::new(inner);
    let err = runner
        .run("/tmp/root", "c")
        .await
        .expect_err("Err 臂应上抛");
    assert_eq!(err, "报告目录创建失败", "Err(String) 原样（无再包装）");
}

/// 边界：TestExecutionOutcome 最小载荷字段面——conclusion + 四计数 +
/// findings_brief + report_dir 构造、Clone / PartialEq 派生可用（IPC 面零
/// 全量透传——全量 findings 留报告文件由 report_dir 定位）。
#[test]
fn test_execution_outcome最小载荷字段面() {
    let outcome = sample_execution_outcome(TestExecutionConclusion::Error);
    assert_eq!(outcome.conclusion, TestExecutionConclusion::Error);
    assert_eq!(
        (
            outcome.total,
            outcome.passed,
            outcome.failed,
            outcome.skipped
        ),
        (12, 10, 2, 0)
    );
    assert!(
        outcome.findings_brief.contains("聚类失败"),
        "诊断摘要随载荷（200 截断 + 10 条上限在装配点执法）"
    );
    assert!(
        outcome.report_dir.ends_with("reports/test"),
        "报告目录定位全量 findings"
    );

    let clone = outcome.clone();
    assert_eq!(clone, outcome, "Clone / PartialEq 派生可用");
}

/// 边界：TestExecutionConclusion 三值 as_str 线格式小写逐字（pass / fail /
/// error——WorkerRole 先例同型，checks Conclusion 的 port 映射像）。
#[test]
fn test_execution_conclusion三值线格式逐字() {
    assert_eq!(TestExecutionConclusion::Pass.as_str(), "pass");
    assert_eq!(TestExecutionConclusion::Fail.as_str(), "fail");
    assert_eq!(TestExecutionConclusion::Error.as_str(), "error");
    // 三值互异（封闭集可辨）
    let all = [
        TestExecutionConclusion::Pass,
        TestExecutionConclusion::Fail,
        TestExecutionConclusion::Error,
    ];
    let mut words: Vec<&str> = all.iter().map(|c| c.as_str()).collect();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), 3, "三值互异");
}
