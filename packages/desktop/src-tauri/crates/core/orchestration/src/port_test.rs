use std::sync::{Arc, Mutex};

use agent::{AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance};
use foundation::cancel::CancelFlag;
use foundation::layout::ReportSlot;

use crate::port::{
    BoxDiffFuture, DiffContextPort, RunEventSink, StaticCheckOutcome, StaticCheckRunner,
    ToolCommand, ToolStepOutput, ToolStepPort, WorkerAgentPort, WorkerRole, WorkerTurnOutcome,
    WorkerTurnRequest, WorkflowSnapshotPort,
};
use crate::state::{ArchivePreflight, RunUpdate};
use workflow::write::{
    ArchiveOutcome, BacktrackInput, BacktrackOption, BacktrackOutcome, DecisionLogOutcome,
    PhaseLogInput, PhaseLogOutcome, PhaseNextOutcome, PhaseStartOutcome,
};

// ---------------------------------------------------------------------------
// 装置：写面原生载荷 fixture 与五假 port
// ---------------------------------------------------------------------------

/// 固定 change **id** 字面量（ToolCommand 各变体定位载荷——一切寻址以 id 为
/// 准；空串占位退役，D12）。
const CHANGE_ID: &str = "6b1d9e34-8f27-4c05-9a73-2e5b0c8d1f46";

fn sample_phase_next_outcome() -> PhaseNextOutcome {
    PhaseNextOutcome {
        done: false,
        next_phase: Some("implement".to_owned()),
        round: 2,
        steps: Vec::new(),
        artifact_manifest: Vec::new(),
        executor: None,
        evaluator: None,
        allowed_backtrack_phases: vec![
            BacktrackOption {
                phase: "proposal".to_owned(),
                description: "需求提案与规格说明".to_owned(),
            },
            BacktrackOption {
                phase: "dev-design".to_owned(),
                description: "详细设计与任务拆解".to_owned(),
            },
        ],
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
                output_ref: None,
                report_dir: String::new(),
            }))
        })
    }
}

/// 可编程产出 / Err 的假 StaticCheckRunner（`(root, name, slot)` 三元组透传断言面
/// ——磁盘面 port 收 name，解析在 steps 消费点；slot 报告槽位透传；calls 经 Arc
/// 共享供测试侧读）。
struct ProgrammableRunner {
    calls: Arc<Mutex<Vec<(String, String, ReportSlot)>>>,
    /// 收到的取消信号（AC-6 形参透传面：身份相等断言原料）
    cancels: Arc<Mutex<Vec<CancelFlag>>>,
    outcome: StaticCheckOutcome,
}

impl ProgrammableRunner {
    fn with_outcome(
        outcome: StaticCheckOutcome,
    ) -> (Self, Arc<Mutex<Vec<(String, String, ReportSlot)>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
                cancels: Arc::new(Mutex::new(Vec::new())),
                outcome,
            },
            calls,
        )
    }

    /// 取消信号记录句柄（在实例 move 进 `Arc<dyn Trait>` 之前取出）。
    fn cancel_log(&self) -> Arc<Mutex<Vec<CancelFlag>>> {
        Arc::clone(&self.cancels)
    }
}

impl StaticCheckRunner for ProgrammableRunner {
    fn run(
        &self,
        root: &str,
        name: &str,
        slot: ReportSlot,
        cancel: CancelFlag,
    ) -> crate::port::BoxToolFuture {
        self.cancels
            .lock()
            .expect("cancels 锁不可中毒")
            .push(cancel);
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push((root.to_owned(), name.to_owned(), slot));
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

/// ToolCommand 十变体封闭集：十变体构造 + match 穷尽分发编译期锚定；各变体定
/// 位载荷 `change_id` 原样承接（id 归键——字段改名置换）；载荷直载写面输入类
/// 型（PhaseLogInput / BacktrackInput 原样承接——AC-6 进程内缝命令面）；
/// StaticCheck / TestExecution 两变体携 `slot: ReportSlot` 报告槽位载荷（
/// slot=Test / Baseline 两值原样承载——check-before-flow-start）。
#[tokio::test]
async fn tool_command十变体封闭集且match穷尽分发() {
    // match 穷尽：缺任一变体即编译失败（封闭集收缩形态的编译期锚）
    fn describe(command: &ToolCommand) -> &'static str {
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

    // 定位载荷提取（match 穷尽第二锚）：十变体齐载 change_id（含 StaticCheck
    // 新字段——D12 归键，空串占位退役）
    fn change_id_of(command: &ToolCommand) -> &str {
        match command {
            ToolCommand::PhaseNext { change_id, .. }
            | ToolCommand::PhaseStart { change_id, .. }
            | ToolCommand::PhaseLog { change_id, .. }
            | ToolCommand::Backtrack { change_id, .. }
            | ToolCommand::DecisionLog { change_id, .. }
            | ToolCommand::StaticCheck { change_id, .. }
            | ToolCommand::TestExecution { change_id, .. }
            | ToolCommand::MutationTest { change_id }
            | ToolCommand::ArchivePreflight { change_id }
            | ToolCommand::ArchiveSeal { change_id } => change_id,
        }
    }

    let commands = vec![
        (
            ToolCommand::PhaseNext {
                change_id: CHANGE_ID.to_owned(),
                run_id: "run-1".to_owned(),
            },
            "phase-next",
        ),
        (
            ToolCommand::PhaseStart {
                change_id: CHANGE_ID.to_owned(),
                phase: "implement".to_owned(),
            },
            "phase-start",
        ),
        (
            ToolCommand::PhaseLog {
                change_id: CHANGE_ID.to_owned(),
                phase: "implement".to_owned(),
                input: sample_phase_log_input(),
            },
            "phase-log",
        ),
        (
            ToolCommand::Backtrack {
                change_id: CHANGE_ID.to_owned(),
                phase: "implement".to_owned(),
                input: sample_backtrack_input(),
            },
            "backtrack",
        ),
        (
            ToolCommand::DecisionLog {
                change_id: CHANGE_ID.to_owned(),
                phase: "implement".to_owned(),
                session_id: "sess-1".to_owned(),
            },
            "decision-log",
        ),
        (
            ToolCommand::StaticCheck {
                change_id: CHANGE_ID.to_owned(),
                slot: ReportSlot::Test,
            },
            "static-check",
        ),
        (
            ToolCommand::MutationTest {
                change_id: CHANGE_ID.to_owned(),
            },
            "mutation-test",
        ),
        (
            ToolCommand::ArchivePreflight {
                change_id: CHANGE_ID.to_owned(),
            },
            "archive-preflight",
        ),
        (
            ToolCommand::ArchiveSeal {
                change_id: CHANGE_ID.to_owned(),
            },
            "archive-seal",
        ),
    ];
    assert_eq!(commands.len(), 9, "封闭集九变体在场枚举（另 TestExecution 单测专行）");

    for (command, label) in &commands {
        assert_eq!(describe(command), *label, "变体 {label} 可辨");
        assert_eq!(
            change_id_of(command),
            CHANGE_ID,
            "变体 {label} 定位载荷 change_id 原样承接"
        );
    }

    // 载荷直载写面输入类型：PhaseLogInput / BacktrackInput 原样承接（PartialEq
    // 逐字段相等——AC-6 进程内缝命令面）
    let logged = ToolCommand::PhaseLog {
        change_id: CHANGE_ID.to_owned(),
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
        change_id: CHANGE_ID.to_owned(),
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
            cancel: foundation::cancel::CancelFlag::never(),
            command: logged,
        })
        .await
        .expect("步产出应可用");
    assert!(matches!(output, ToolStepOutput::StaticCheck(outcome) if outcome.passed));
    assert_eq!(seen.lock().expect("锁").len(), 1, "命令原样到达注入 port");
}

/// StaticCheck 命令 change_id 归键（D12 载荷锚）：携真实 change id 构造 → 定
/// 位载荷逐字承接且非空（空串占位退役——static_check 审计行按 change 可枚举的
/// 前提；行为断言归 steps_test 审计行）；slot 槽位载荷原样承载（Test 缺省 /
/// Baseline 基线两值可辨）。
#[test]
fn static_check命令change_id归键构造面() {
    let check = ToolCommand::StaticCheck {
        change_id: CHANGE_ID.to_owned(),
        slot: ReportSlot::Test,
    };
    match &check {
        ToolCommand::StaticCheck { change_id, slot } => {
            assert_eq!(change_id, CHANGE_ID, "定位载荷逐字承接");
            assert!(!change_id.is_empty(), "非空串占位（D12 归键前提）");
            assert_eq!(slot, &ReportSlot::Test, "slot 槽位载荷原样承载");
        }
        other => panic!("变体漂移: {other:?}"),
    }
    // 归档两新变体：描述穷尽 + 载荷归键可辨（封闭集扩臂锚）
    fn describe_archive(command: &ToolCommand) -> &'static str {
        match command {
            ToolCommand::ArchivePreflight { .. } => "archive-preflight",
            ToolCommand::ArchiveSeal { .. } => "archive-seal",
            other => panic!("非归档命令: {other:?}"),
        }
    }
    assert_eq!(
        describe_archive(&ToolCommand::ArchivePreflight {
            change_id: CHANGE_ID.to_owned()
        }),
        "archive-preflight"
    );
    assert_eq!(
        describe_archive(&ToolCommand::ArchiveSeal {
            change_id: CHANGE_ID.to_owned()
        }),
        "archive-seal"
    );
    assert_ne!(
        ToolCommand::ArchivePreflight {
            change_id: CHANGE_ID.to_owned()
        },
        ToolCommand::ArchiveSeal {
            change_id: CHANGE_ID.to_owned()
        },
        "归档两新变体互异"
    );
    // 载荷参与等值：id 不同即命令不同（审计行归桶可辨）；Clone/PartialEq 派生可用
    assert_ne!(
        check,
        ToolCommand::StaticCheck {
            change_id: "other-id".to_owned(),
            slot: ReportSlot::Test,
        }
    );
    assert_eq!(
        check,
        ToolCommand::StaticCheck {
            change_id: CHANGE_ID.to_owned(),
            slot: ReportSlot::Test,
        }
    );
}

/// ToolCommand slot 载荷封闭面（正向，check-before-flow-start）：StaticCheck /
/// TestExecution 两变体 slot=Test / Baseline 两值构造原样承载 + change_id 归键
/// 承接；`MutationTest` 变体零 slot 字段（编译面——基线检查明确不含 mutation：
/// 构造面无 slot 入参、match 解构无 slot 绑定）；slot 参与变体等值（同 id 异
/// slot 可辨）。
#[test]
fn tool_command_slot载荷两值承载且mutation变体零slot() {
    // slot=Test / Baseline 两值原样承载（枚举遍历）
    for slot in [ReportSlot::Test, ReportSlot::Baseline] {
        let check = ToolCommand::StaticCheck {
            change_id: CHANGE_ID.to_owned(),
            slot,
        };
        match &check {
            ToolCommand::StaticCheck { change_id, slot: s } => {
                assert_eq!(change_id, CHANGE_ID, "change_id 归键承接（slot={slot:?}）");
                assert_eq!(s, &slot, "slot 两值原样承载（slot={slot:?}）");
            }
            other => panic!("变体漂移: {other:?}"),
        }
        let execution = ToolCommand::TestExecution {
            change_id: CHANGE_ID.to_owned(),
            slot,
        };
        match &execution {
            ToolCommand::TestExecution { change_id, slot: s } => {
                assert_eq!(change_id, CHANGE_ID, "change_id 归键承接（slot={slot:?}）");
                assert_eq!(s, &slot, "slot 两值原样承载（slot={slot:?}）");
            }
            other => panic!("变体漂移: {other:?}"),
        }
    }

    // slot 参与等值：同 id 异 slot 两命令可辨（Payload 等值面）
    assert_ne!(
        ToolCommand::StaticCheck {
            change_id: CHANGE_ID.to_owned(),
            slot: ReportSlot::Test,
        },
        ToolCommand::StaticCheck {
            change_id: CHANGE_ID.to_owned(),
            slot: ReportSlot::Baseline,
        },
        "slot 参与变体等值（Test / Baseline 可辨）"
    );
    assert_ne!(
        ToolCommand::TestExecution {
            change_id: CHANGE_ID.to_owned(),
            slot: ReportSlot::Test,
        },
        ToolCommand::TestExecution {
            change_id: CHANGE_ID.to_owned(),
            slot: ReportSlot::Baseline,
        },
        "TestExecution slot 参与变体等值"
    );

    // MutationTest 变体零 slot 字段（编译面锚）：构造面仅 change_id、match 解构
    // 无 slot 绑定——基线检查明确不含 mutation
    let mutation = ToolCommand::MutationTest {
        change_id: CHANGE_ID.to_owned(),
    };
    match &mutation {
        ToolCommand::MutationTest { change_id } => {
            assert_eq!(change_id, CHANGE_ID, "MutationTest 仅归键载荷（零 slot）");
        }
        other => panic!("变体漂移: {other:?}"),
    }

    // slot 经 Copy 传递不耗损（ReportSlot 纯进程内枚举——Clone + Copy 派生可用，
    // 同值传递后原值仍可用）
    let slot = ReportSlot::Baseline;
    let _first = ToolCommand::StaticCheck {
        change_id: CHANGE_ID.to_owned(),
        slot,
    };
    let _second = ToolCommand::StaticCheck {
        change_id: CHANGE_ID.to_owned(),
        slot,
    };
    assert_eq!(slot, ReportSlot::Baseline, "Copy 语义：传递后原值不耗损");
    // ReportSlot 零 serde（不进 IPC 线面的类型锚）：无 Serialize 约束的消费面
    fn consume_in_process(slot: ReportSlot) -> ReportSlot {
        slot
    }
    assert_eq!(consume_in_process(ReportSlot::Test), ReportSlot::Test);
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
        // 开相时刻 i64 UTC unix 毫秒（db 时间戳原样直透——ISO 转换不在此层）
        start_at: 1_790_841_600_000,
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
        output_ref: None,
        report_dir: String::new(),
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

/// StaticCheckOutcome 字段面：passed true/false × diagnostics 空串 / 多行诊断 ×
/// `output_ref` Some（默认 recorded 回填）/ None（零 spawn 直过 / 落盘失败降级）
/// 两态构造与读取（passed=false 走定向反馈边的载荷面——AC-4 / AC-8）；`report_dir`
/// 字段 pass / fail 两态均携非空串（与 `TestExecutionOutcome.report_dir` 字段
/// 对称——baseline Terminal 文案供给面，check-before-flow-start）；空串构造不
/// panic（回填语义由 infra 真件节断言——port 面只锚字段在场可构造可读取）。
#[test]
fn static_check_outcome字段面双值双诊断形态() {
    let passed_clean = StaticCheckOutcome {
        passed: true,
        diagnostics: String::new(),
        output_ref: None,
        report_dir: String::new(),
    };
    assert!(passed_clean.passed);
    assert!(passed_clean.diagnostics.is_empty());

    let failed_loud = StaticCheckOutcome {
        passed: false,
        diagnostics: "error[E0308]: mismatched types\n  --> src/lib.rs:1:1\nwarning: unused"
            .to_owned(),
        output_ref: None,
        report_dir: String::new(),
    };
    assert!(!failed_loud.passed, "passed=false 走定向反馈边");
    assert!(
        failed_loud.diagnostics.lines().count() >= 2,
        "多行诊断原样承载（反馈边注入原料）"
    );
    assert_eq!(failed_loud.clone(), failed_loud, "Clone/PartialEq 派生可用");

    // 输出记录引用随产出透传：Some = 全量输出可经 infra/exec 回查（默认
    // recorded），None = 零 spawn 直过 / 落盘失败降级——两态可辨
    let recorded = StaticCheckOutcome {
        passed: false,
        diagnostics: "lint 失败".to_owned(),
        output_ref: Some("exec-1727000000000".to_owned()),
        report_dir: String::new(),
    };
    assert_eq!(
        recorded.output_ref.as_deref(),
        Some("exec-1727000000000"),
        "recorded 引用随产出透传（可回查）"
    );
    assert_ne!(
        recorded.output_ref, failed_loud.output_ref,
        "Some（可回查）/ None（零 spawn / 落盘失败降级）两态可辨"
    );
    assert_ne!(
        recorded, failed_loud,
        "output_ref 参与载荷等值（反馈边载荷可辨）"
    );

    // report_dir 字段对称面（check-before-flow-start）：pass / fail 两态各携非空
    // 报告目录串（与 TestExecutionOutcome.report_dir 对称——infra 按槽位派生回填）
    let passed_with_dir = StaticCheckOutcome {
        passed: true,
        diagnostics: String::new(),
        output_ref: Some("exec-1".to_owned()),
        report_dir: "C:/ws/openspec/changes/c/reports/baseline".to_owned(),
    };
    let failed_with_dir = StaticCheckOutcome {
        passed: false,
        diagnostics: "lint 红了".to_owned(),
        output_ref: Some("exec-2".to_owned()),
        report_dir: "C:/ws/openspec/changes/c/reports/baseline".to_owned(),
    };
    assert!(
        !passed_with_dir.report_dir.is_empty() && !failed_with_dir.report_dir.is_empty(),
        "pass / fail 两态 report_dir 均携非空串"
    );
    assert!(
        passed_with_dir.report_dir.ends_with("reports/baseline"),
        "baseline 槽位报告锚点（Terminal 文案 dir 占位供给面）"
    );
    // 对称面对拍：TestExecutionOutcome.report_dir 同为 String 字段、可同式构造读取
    let execution = TestExecutionOutcome {
        conclusion: TestExecutionConclusion::Pass,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        findings_brief: String::new(),
        findings_detail: String::new(),
        report_dir: passed_with_dir.report_dir.clone(),
        exec_id: String::new(),
    };
    assert_eq!(
        execution.report_dir, passed_with_dir.report_dir,
        "两 Outcome.report_dir 字段对称（同锚同口径）"
    );

    // 空串 / 缺省构造面不 panic（回填语义归 infra 真件节——port 面只锚字段在场）
    let empty_dir = StaticCheckOutcome {
        passed: true,
        diagnostics: String::new(),
        output_ref: None,
        report_dir: String::new(),
    };
    assert_eq!(empty_dir.report_dir, "", "空串构造不 panic（字段恒可读）");
}

// ---------------------------------------------------------------------------
// trait 面：StaticCheckRunner / DiffContextPort / BoxDiffFuture / 既有三契约
// ---------------------------------------------------------------------------

/// StaticCheckRunner trait 面：假实现经 Arc<dyn …> 注入、BoxToolFuture 返回
/// 可用、`(root, name, slot)` 三元组透传（object safety + Send+Sync 编译锚——
/// W4 缝成立前提；`run(root, name, slot)` 与 [`TestExecutionRunner`] 同型：spawn
/// cwd 与磁盘面 name 定位双参 + slot 报告槽位——check-before-flow-start；
/// slot=Test / Baseline 两值原样到达）。
#[tokio::test]
async fn static_check_runner假实现经arc注入且root_name_slot三元组透传() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn StaticCheckRunner>>();

    let (inner, calls) = ProgrammableRunner::with_outcome(StaticCheckOutcome {
        // 默认 recorded：引用随产出回填（载荷语义不变）
        passed: false,
        diagnostics: "lint 失败".to_owned(),
        output_ref: Some("exec-1727000000000-1".to_owned()),
        report_dir: "C:/ws/openspec/changes/demo-change/reports/test".to_owned(),
    });
    let runner: Arc<dyn StaticCheckRunner> = Arc::new(inner);

    // 三参透传 + BoxToolFuture resolve 可用（W4 缝；slot=Test 缺省槽位）
    let output = runner
        .run(
            "/tmp/workspace-root",
            "demo-change",
            ReportSlot::Test,
            CancelFlag::never(),
        )
        .await
        .expect("产出应可用");
    match output {
        ToolStepOutput::StaticCheck(outcome) => {
            assert!(!outcome.passed);
            assert_eq!(outcome.diagnostics, "lint 失败");
            assert_eq!(
                outcome.output_ref.as_deref(),
                Some("exec-1727000000000-1"),
                "默认 recorded 引用透传（caller 侧零改写）"
            );
            assert_eq!(
                outcome.report_dir, "C:/ws/openspec/changes/demo-change/reports/test",
                "report_dir 载荷透传（槽位派生回填面）"
            );
        }
        _ => panic!("产出变体漂移"),
    }
    assert_eq!(
        calls.lock().expect("锁").as_slice(),
        [(
            "/tmp/workspace-root".to_owned(),
            "demo-change".to_owned(),
            ReportSlot::Test
        )],
        "root / name / slot 三元组透传（spawn cwd、记录目录派生与报告槽位语义）"
    );

    // slot=Baseline 基线槽位同构透传（枚举遍历——两值原样到达假件）
    let (inner, calls) = ProgrammableRunner::with_outcome(StaticCheckOutcome {
        passed: true,
        diagnostics: String::new(),
        output_ref: Some("exec-1727000000000-2".to_owned()),
        report_dir: "C:/ws/openspec/changes/demo-change/reports/baseline".to_owned(),
    });
    let runner: Arc<dyn StaticCheckRunner> = Arc::new(inner);
    let output = runner
        .run(
            "/tmp/workspace-root",
            "demo-change",
            ReportSlot::Baseline,
            CancelFlag::never(),
        )
        .await
        .expect("产出应可用");
    assert!(
        matches!(output, ToolStepOutput::StaticCheck(ref outcome) if outcome.report_dir.ends_with("reports/baseline")),
        "baseline 槽位产出透传"
    );
    assert_eq!(
        calls.lock().expect("锁").as_slice(),
        [(
            "/tmp/workspace-root".to_owned(),
            "demo-change".to_owned(),
            ReportSlot::Baseline
        )],
        "slot=Baseline 原样到达（initialize 基线检查槽位）"
    );
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
                output_ref: None,
                report_dir: String::new(),
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
    // WorkerTurnRequest 中性类型构造（provenance / permission / model_level
    // / continue_session / agent / role 字段面）
    let request = WorkerTurnRequest {
        root: "/tmp/ws".to_owned(),
        prompt: "轮 prompt".to_owned(),
        provenance: SessionProvenance {
            source: "change".to_owned(),
            source_ref: Some("c/implement/executor/1".to_owned()),
        },
        permission: AgentPermissionMode::BypassPermissions,
        model_level: ModelLevel::Low,
        continue_session: Some("sess-1".to_owned()),
        agent: None,
        role: WorkerRole::Executor,
    };
    assert_eq!(request.provenance.source, "change");
    assert_eq!(request.permission, AgentPermissionMode::BypassPermissions);
    assert_eq!(request.model_level, ModelLevel::Low, "模型档位字段面");
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
// RunHistoryPort 缝（unify-run-state-persistence）：trait 声明面（`run_started`
// / `run_finished`，sync 零 tokio、Err 串语义与既有 port 同型）经消费侧承载
// ——进程内假件（计数 / 可编程 Err）断言挂 `walker_test.rs`（每 run 恰两写 /
// start fail-fast / finish best-effort），真件 `StoreRunHistory` 委派断言挂
// `run_history_test.rs`（tempfile 真库回环 + Err 串语义透传）。本文件不重复
// 行为用例（test-design 本节 Mock 表：既有 sink 直调装置，新 trait 行为断言
// 挂消费侧）。
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// test-execution 扩展节（desktop-checks-domain）
// ---------------------------------------------------------------------------

use crate::port::{TestExecutionConclusion, TestExecutionOutcome, TestExecutionRunner};

/// 可编程产出 / Err 的假 TestExecutionRunner（root / name / slot 三元组透传断言面
/// ——磁盘面 port 收 name；slot 报告槽位透传；calls 经 Arc 共享——测试侧持句柄
/// 读记录）。
struct ProgrammableTestExecutionRunner {
    calls: Arc<Mutex<Vec<(String, String, ReportSlot)>>>,
    /// 收到的取消信号（AC-6 形参透传面）
    cancels: Arc<Mutex<Vec<CancelFlag>>>,
    result: Result<TestExecutionOutcome, String>,
}

impl ProgrammableTestExecutionRunner {
    fn with_result(
        result: Result<TestExecutionOutcome, String>,
    ) -> (Self, Arc<Mutex<Vec<(String, String, ReportSlot)>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
                cancels: Arc::new(Mutex::new(Vec::new())),
                result,
            },
            calls,
        )
    }

    /// 取消信号记录句柄（在实例 move 进 `Arc<dyn Trait>` 之前取出）。
    fn cancel_log(&self) -> Arc<Mutex<Vec<CancelFlag>>> {
        Arc::clone(&self.cancels)
    }
}

impl TestExecutionRunner for ProgrammableTestExecutionRunner {
    fn run(
        &self,
        root: &str,
        name: &str,
        slot: ReportSlot,
        cancel: CancelFlag,
    ) -> crate::port::BoxToolFuture {
        self.cancels
            .lock()
            .expect("cancels 锁不可中毒")
            .push(cancel);
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push((root.to_owned(), name.to_owned(), slot));
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
        findings_detail: match conclusion {
            TestExecutionConclusion::Pass => String::new(),
            _ => "## 失败用例明细\n\n[vite-plus]\n1. 汇总导出_空清单回落（src/export.test.ts:23）"
                .to_owned(),
        },
        report_dir: "C:/ws/openspec/changes/c/reports/test".to_owned(),
        exec_id: String::new(),
    }
}

/// 正向：ToolCommand 第七变体封闭集——TestExecution 构造 + match 穷尽编译锚
/// + change_id 载荷与 name 入参透传 + slot 槽位载荷承载 + 门禁双命令互异（十变体
/// 封闭集下 TestExecution 专行，零改动持衡——slot 随 check-before-flow-start 扩载）。
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
            ToolCommand::StaticCheck { .. } => "static-check",
            ToolCommand::TestExecution { .. } => "test-execution",
            ToolCommand::MutationTest { .. } => "mutation-test",
            ToolCommand::ArchivePreflight { .. } => "archive-preflight",
            ToolCommand::ArchiveSeal { .. } => "archive-seal",
        }
    }
    assert_eq!(
        describe(&ToolCommand::TestExecution {
            change_id: CHANGE_ID.to_owned(),
            slot: ReportSlot::Test,
        }),
        "test-execution"
    );

    // change_id 载荷透传：命令定位键原样承接（磁盘面 name 由消费点解析）
    let execution = ToolCommand::TestExecution {
        change_id: CHANGE_ID.to_owned(),
        slot: ReportSlot::Test,
    };
    match &execution {
        ToolCommand::TestExecution { change_id, .. } => {
            assert_eq!(change_id, CHANGE_ID, "change_id 定位载荷逐字承接");
        }
        other => panic!("变体漂移: {other:?}"),
    }
    assert_ne!(
        execution,
        ToolCommand::TestExecution {
            change_id: "other-id".to_owned(),
            slot: ReportSlot::Test,
        },
        "定位载荷参与变体等值（归桶可辨）"
    );
    // name 入参透传：命令经 ToolStepRequest 原样到达注入 port
    let (inner, calls) = ProgrammableTestExecutionRunner::with_result(Ok(
        sample_execution_outcome(TestExecutionConclusion::Pass),
    ));
    let runner: Arc<dyn TestExecutionRunner> = Arc::new(inner);
    let output = runner
        .run(
            "/tmp/workspace-root",
            "demo-change",
            ReportSlot::Test,
            CancelFlag::never(),
        )
        .await
        .expect("产出应可用");
    assert!(
        matches!(output, ToolStepOutput::TestExecution(outcome) if outcome.conclusion == TestExecutionConclusion::Pass),
        "产出为 TestExecution 变体"
    );
    assert_eq!(
        calls.lock().expect("锁").as_slice(),
        [(
            "/tmp/workspace-root".to_owned(),
            "demo-change".to_owned(),
            ReportSlot::Test
        )],
        "root / name / slot 原样到达注入 port（磁盘面报告树定位 + 槽位语义）"
    );

    // 七变体互异（封闭集封闭性——第八臂不存在）
    let seventh = ToolCommand::TestExecution {
        change_id: CHANGE_ID.to_owned(),
        slot: ReportSlot::Test,
    };
    assert_ne!(
        describe(&seventh),
        describe(&ToolCommand::StaticCheck {
            change_id: CHANGE_ID.to_owned(),
            slot: ReportSlot::Test,
        }),
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
/// name / slot 三参透传、BoxToolFuture resolve 出 ToolStepOutput::TestExecution
///（object safety + Send + Sync 编译锚——StaticCheckRunner 同型；磁盘面 port
/// 恒收 name，解析在 steps 消费点；slot=Test / Baseline 两值枚举遍历）。
#[tokio::test]
async fn test_execution_runner假实现经arc注入且root_change透传() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn TestExecutionRunner>>();

    for slot in [ReportSlot::Test, ReportSlot::Baseline] {
        let (inner, calls) = ProgrammableTestExecutionRunner::with_result(Ok(
            sample_execution_outcome(TestExecutionConclusion::Pass),
        ));
        let runner: Arc<dyn TestExecutionRunner> = Arc::new(inner);

        let output = runner
            .run("/tmp/workspace-root", "walker-change", slot, CancelFlag::never())
            .await
            .expect("产出应可用");
        assert!(
            matches!(output, ToolStepOutput::TestExecution(outcome) if outcome.total == 12),
            "BoxToolFuture resolve 出 TestExecution 变体（slot={slot:?}）"
        );
        assert_eq!(
            calls.lock().expect("锁").as_slice(),
            [(
                "/tmp/workspace-root".to_owned(),
                "walker-change".to_owned(),
                slot
            )],
            "root / name / slot 三参透传（spawn cwd、报告树定位与槽位语义）"
        );
    }
}

/// 异常：TestExecutionRunner Err(String) 臂经 BoxToolFuture resolve 上抛
///（写盘失败类基础设施错误面——run_tool 映射输入；三参签名扩展不吞错误面）。
#[tokio::test]
async fn test_execution_runner_err臂上抛() {
    let (inner, calls) =
        ProgrammableTestExecutionRunner::with_result(Err("报告目录创建失败".to_owned()));
    let runner: Arc<dyn TestExecutionRunner> = Arc::new(inner);
    let err = runner
        .run("/tmp/root", "c", ReportSlot::Baseline, CancelFlag::never())
        .await
        .expect_err("Err 臂应上抛");
    assert_eq!(err, "报告目录创建失败", "Err(String) 原样（无再包装）");
    assert_eq!(
        calls.lock().expect("锁").as_slice(),
        [("/tmp/root".to_owned(), "c".to_owned(), ReportSlot::Baseline)],
        "Err 臂调用载荷同样捕获（透传不吞错误面）"
    );
}

/// 边界：TestExecutionOutcome 载荷字段面——conclusion + 四计数 +
/// findings_brief + findings_detail + report_dir 构造、Clone / PartialEq
/// 派生可用（载荷零 Serialize 不进 IPC：摘要面服务步 detail，明细面服务
/// 反馈边修复 prompt，全量 findings 留报告文件由 report_dir 定位）。
#[test]
fn test_execution_outcome载荷字段面() {
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
        outcome.findings_detail.contains("失败用例明细")
            && outcome.findings_detail.contains("汇总导出_空清单回落"),
        "修复边明细文本随载荷（有界装配，反馈边 prompt 直嵌面）"
    );
    assert!(
        sample_execution_outcome(TestExecutionConclusion::Pass)
            .findings_detail
            .is_empty(),
        "pass 态明细恒空串（反馈边不消费）"
    );
    assert!(
        outcome.report_dir.ends_with("reports/test"),
        "报告目录定位全量 findings"
    );

    // 输出引用两态与 report_dir 同锚同口径（AC-8）：`exec_id` 非空 = 脚本输出
    // 已记录（经 infra/exec 单点回查）、空串 = 未记录（落盘失败 / 零输出装配
    // 降级——report_dir 报告仍为全量权威），两态随 step 产出透传
    assert_eq!(
        outcome.exec_id, "",
        "未记录形态空串（落盘失败降级——报告仍为全量权威）"
    );
    let recorded = TestExecutionOutcome {
        exec_id: "exec-1727000000000".to_owned(),
        ..outcome.clone()
    };
    assert_eq!(
        recorded.exec_id, "exec-1727000000000",
        "Recorded 模式引用随产出透传（可回查）"
    );
    assert_ne!(recorded, outcome, "exec_id 两态参与载荷等值（可辨）");
    assert_eq!(
        recorded.report_dir, outcome.report_dir,
        "exec_id 与 report_dir 同锚（同一 change 报告树的引用与目录）"
    );
    match ToolStepOutput::TestExecution(recorded.clone()) {
        ToolStepOutput::TestExecution(extracted) => assert_eq!(
            extracted.exec_id, "exec-1727000000000",
            "引用随 ToolStepOutput::TestExecution 变体透传至 walker"
        ),
        other => panic!("变体漂移: {other:?}"),
    }

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

// ---------------------------------------------------------------------------
// 归档两会话角色与两新命令 / 产出变体（AC-2 / AC-3 / D3）
// ---------------------------------------------------------------------------

/// 归档前置只读载荷 fixture（八字段像；与 state / control / steps 消费同一面）。
fn sample_preflight() -> ArchivePreflight {
    ArchivePreflight {
        name: "demo-change".to_owned(),
        completed: true,
        incomplete_phases: Vec::new(),
        missing_artifacts: Vec::new(),
        delta_specs: vec!["specs/desktop-flow/spec.md".to_owned()],
        worktree: Some("/tmp/wt/demo-change".to_owned()),
        branch: Some("change/demo-change".to_owned()),
        merge_target: Some("main".to_owned()),
    }
}

/// `WorkerRole` 五值（正向）：`as_str()` 逐字出线（executor / evaluator /
/// decision / spec_sync / resolve_conflicts）——与 workflow `AgentRole` 同源
/// 词面（AC-2 / D3）。
#[test]
fn worker_role五值as_str逐字且与agent_role同源() {
    assert_eq!(WorkerRole::Executor.as_str(), "executor");
    assert_eq!(WorkerRole::Evaluator.as_str(), "evaluator");
    assert_eq!(WorkerRole::Decision.as_str(), "decision");
    assert_eq!(
        WorkerRole::SpecSync.as_str(),
        "spec_sync",
        "归档 spec 同步会话角色词"
    );
    assert_eq!(
        WorkerRole::ResolveConflicts.as_str(),
        "resolve_conflicts",
        "归档冲突解算会话角色词"
    );

    // 五值互异（source_ref 第三段无撞号）
    let mut words = vec![
        WorkerRole::Executor.as_str(),
        WorkerRole::Evaluator.as_str(),
        WorkerRole::Decision.as_str(),
        WorkerRole::SpecSync.as_str(),
        WorkerRole::ResolveConflicts.as_str(),
    ];
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), 5, "五角色词互异（零撞号）");

    // 与 workflow AgentRole 同源词面（跨 crate 逐字可比）
    for (role, agent_role) in [
        (WorkerRole::Executor, workflow::state::AgentRole::Executor),
        (WorkerRole::Evaluator, workflow::state::AgentRole::Evaluator),
        (WorkerRole::Decision, workflow::state::AgentRole::Decision),
        (WorkerRole::SpecSync, workflow::state::AgentRole::SpecSync),
        (
            WorkerRole::ResolveConflicts,
            workflow::state::AgentRole::ResolveConflicts,
        ),
    ] {
        let wire = serde_json::to_value(agent_role).expect("出线应成功");
        assert_eq!(
            role.as_str(),
            wire.as_str().expect("串值"),
            "{agent_role:?} 两 crate 词面逐字同源"
        );
    }
}

/// provenance `source_ref` 组装面（正向）：归档两会话角色词互异且与 executor
/// 不撞号——同 phase 同 attempt 内 source_ref 唯一（D3 / R4 会话归属可辨）。
#[test]
fn worker_role_provenance_role段互异不撞号() {
    let source_ref = |role: WorkerRole| SessionProvenance {
        source: "change".to_owned(),
        source_ref: Some(format!("{CHANGE_ID}/archive/{}/1", role.as_str())),
    };
    let spec_sync = source_ref(WorkerRole::SpecSync);
    let resolve = source_ref(WorkerRole::ResolveConflicts);
    let executor = source_ref(WorkerRole::Executor);

    let spec_sync_ref = spec_sync.source_ref.clone().expect("归档会话携 source_ref");
    let resolve_ref = resolve.source_ref.clone().expect("归档会话携 source_ref");
    let executor_ref = executor.source_ref.clone().expect("会话携 source_ref");
    let mut sorted = vec![
        spec_sync_ref.clone(),
        resolve_ref.clone(),
        executor_ref.clone(),
    ];
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 3, "同 phase 同 attempt 内三角色 source_ref 互异");
    assert!(spec_sync_ref.ends_with("/archive/spec_sync/1"));
    assert!(resolve_ref.ends_with("/archive/resolve_conflicts/1"));
    assert_ne!(
        spec_sync_ref, resolve_ref,
        "归档两会话不撞号（provenance 归属段逐字可辨）"
    );
    // 组装定式与 walker 消费面同式（change id / phase / role / attempt 四段）
    assert_eq!(
        spec_sync_ref,
        format!("{CHANGE_ID}/archive/spec_sync/1"),
        "source_ref 定式逐字"
    );
}

/// `ToolCommand` 两新变体（正向）：`ArchivePreflight { change_id }` /
/// `ArchiveSeal { change_id }` 构造 + PartialEq 可辨（change_id 载荷归键，
/// AC-3）。
#[test]
fn tool_command归档两新变体构造与等值面() {
    let preflight = ToolCommand::ArchivePreflight {
        change_id: CHANGE_ID.to_owned(),
    };
    let seal = ToolCommand::ArchiveSeal {
        change_id: CHANGE_ID.to_owned(),
    };
    assert_ne!(preflight, seal, "两新变体可辨（非同一命令）");
    assert_eq!(
        preflight,
        ToolCommand::ArchivePreflight {
            change_id: CHANGE_ID.to_owned()
        },
        "PartialEq 自反"
    );
    assert_ne!(
        preflight,
        ToolCommand::ArchivePreflight {
            change_id: "other-id".to_owned()
        },
        "change_id 载荷参与等值（归键可辨）"
    );
    assert_ne!(
        seal,
        ToolCommand::ArchiveSeal {
            change_id: "other-id".to_owned()
        }
    );

    // Clone 派生可用（walker 侧命令组装与捕获面）
    let cloned = preflight.clone();
    assert_eq!(cloned, preflight);
}

/// `ToolStepOutput` 两新变体（正向）：`ArchivePreflight(ArchivePreflight)` /
/// `ArchiveSeal(ArchiveOutcome)` 封闭集可构造、载荷可透传（AC-3）。
#[tokio::test]
async fn tool_step_output归档两新变体载荷透传() {
    let preflight = sample_preflight();
    let outcome = ArchiveOutcome {
        name: "demo-change".to_owned(),
        archived_date: "2026-02-02".to_owned(),
    };

    let outputs = vec![
        ToolStepOutput::ArchivePreflight(preflight.clone()),
        ToolStepOutput::ArchiveSeal(outcome.clone()),
    ];
    assert_eq!(outputs.len(), 2, "归档两新产出变体");

    match outputs[0].clone() {
        ToolStepOutput::ArchivePreflight(payload) => {
            assert_eq!(payload, preflight, "前置只读载荷原样透传");
            assert_eq!(payload.name, "demo-change");
            assert!(payload.completed);
        }
        other => panic!("变体漂移: {other:?}"),
    }
    match outputs[1].clone() {
        ToolStepOutput::ArchiveSeal(payload) => {
            assert_eq!(payload, outcome, "双写产出原样透传");
            assert_eq!(payload.name, "demo-change");
            assert_eq!(payload.archived_date, "2026-02-02");
        }
        other => panic!("变体漂移: {other:?}"),
    }
    assert_ne!(
        outputs[0], outputs[1],
        "两新变体互异（线面判别可辨）"
    );

    // 命令经 ToolStepRequest 原样到达注入 port（命令面契约）
    let (steps, seen) = RecordingSteps::capturing();
    let output = steps
        .run(crate::port::ToolStepRequest {
            root: "/tmp/root".to_owned(),
            cancel: foundation::cancel::CancelFlag::never(),
            command: ToolCommand::ArchiveSeal {
                change_id: CHANGE_ID.to_owned(),
            },
        })
        .await;
    assert!(output.is_ok(), "归档命令经注入 port 可达");
    let captured = seen.lock().expect("锁").clone();
    assert_eq!(captured.len(), 1, "归档命令抵达注入 port");
    assert!(matches!(
        captured[0],
        ToolCommand::ArchiveSeal { .. }
    ));
}

/// `ToolStepOutput` 归档两变体异常（异常）：载荷类型不匹配的窄化分支不吞错
///（`match` 穷尽 + 非目标变体显式 panic 的线面契约守卫——进程内 seam 无入站
/// 反序列化面，类型收敛由编译期锚定，运行期以窄化提取承载）。
#[test]
fn tool_step_output归档变体窄化提取类型守卫() {
    let outputs = [
        ToolStepOutput::ArchivePreflight(sample_preflight()),
        ToolStepOutput::ArchiveSeal(ArchiveOutcome {
            name: "demo-change".to_owned(),
            archived_date: "2026-02-02".to_owned(),
        }),
    ];

    // 正确窄化：两分支各自可提（载荷类型固定）
    let preflight = outputs
        .iter()
        .find_map(|output| match output {
            ToolStepOutput::ArchivePreflight(payload) => Some(payload.clone()),
            _ => None,
        })
        .expect("ArchivePreflight 变体应在场");
    assert_eq!(preflight, sample_preflight());

    let seal = outputs
        .iter()
        .find_map(|output| match output {
            ToolStepOutput::ArchiveSeal(payload) => Some(payload.clone()),
            _ => None,
        })
        .expect("ArchiveSeal 变体应在场");
    assert_eq!(seal.archived_date, "2026-02-02");

    // 错配窄化：以 ArchiveSeal 面提取 ArchivePreflight 变体 → None（不误取）
    let mismatched = outputs
        .iter()
        .filter(|output| matches!(output, ToolStepOutput::ArchiveSeal(_)))
        .find_map(|output| match output {
            ToolStepOutput::ArchivePreflight(payload) => Some(payload.clone()),
            _ => None,
        });
    assert!(mismatched.is_none(), "变体错配不误取（类型面封闭）");
}

/// `ArchiveVcsPort` 面（边界 / 回归）：归档第三缝 trait 全方法签名编译面在位
///（dirty / commit_all / branch_merged / rebase_branch / ff_merge /
/// worktree_snapshot / rebase_continue / rebase_abort / current_branch /
/// commit_paths / remove_worktree / restore_baseline），实现可经
/// `Arc<dyn ArchiveVcsPort>` 作为 walker 显式入参注入（AC-3 / R2 core 零 spawn
/// 的结构前提；trait 面扩 restore_baseline 零探测方法锚——worktree 在场判定驻
/// 调用方）。
#[test]
fn archive_vcs_port面全方法签名编译锚() {
    use std::path::Path;

    use crate::port::{ArchiveVcsPort, RebaseOutcome, WorktreeSnapshot};

    /// 编译面假件：全方法逐一实现（签名漂移即编译失败）。
    struct VcsProbe {
        branch: String,
    }

    impl ArchiveVcsPort for VcsProbe {
        fn dirty(&self, _root: &Path, _paths: &[&str]) -> bool {
            false
        }
        fn commit_all(&self, _worktree: &Path, _message: &str) -> Result<(), String> {
            Ok(())
        }
        fn branch_merged(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
            Ok(false)
        }
        fn rebase_branch(&self, _worktree: &Path, _onto: &str) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn ff_merge(&self, _main_root: &Path, _branch: &str) -> Result<(), String> {
            Ok(())
        }
        fn worktree_snapshot(&self, _root: &Path) -> Result<WorktreeSnapshot, String> {
            Ok(WorktreeSnapshot {
                head: "abc123".to_owned(),
                rebase_head: None,
                status: Vec::new(),
                index: Vec::new(),
            })
        }
        fn rebase_continue(&self, _worktree: &Path) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn rebase_abort(&self, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
        fn current_branch(&self, _main_root: &Path) -> Result<String, String> {
            Ok(self.branch.clone())
        }
        fn commit_paths(
            &self,
            _main_root: &Path,
            _paths: &[&str],
            _message: &str,
        ) -> Result<(), String> {
            Ok(())
        }
        fn change_edit_set(&self, _root: &Path, _base: &str) -> Result<Vec<String>, String> {
            Ok(Vec::new())
        }
        fn change_edit_patch(&self, _root: &Path, _base: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn remove_worktree(&self, _main_root: &Path, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
        fn restore_baseline(&self, _worktree: &Path, _exclusions: &[&str]) -> Result<String, String> {
            Ok("已恢复基线".to_owned())
        }
    }

    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VcsProbe>();

    let port: Arc<dyn ArchiveVcsPort> = Arc::new(VcsProbe {
        branch: "main".to_owned(),
    });
    assert_eq!(
        port.current_branch(Path::new("/tmp/root")).expect("探测应可用"),
        "main",
        "合入目标分支探测（Preflight / Rebase 消费）"
    );
    assert_eq!(
        port.branch_merged(Path::new("/tmp/root"), "change/demo")
            .expect("判定应可用"),
        false,
        "合入段幂等跳过判定（Finalize 消费）"
    );
    assert!(!port.dirty(Path::new("/tmp/root"), &[]), "干净探测 → false（Commit 跳过）");
    assert!(!port.dirty(Path::new("/tmp/root"), &["openspec"]));
    assert_eq!(
        port.rebase_branch(Path::new("/tmp/wt"), "main")
            .expect("重放应可用"),
        RebaseOutcome::Rebased,
        "重放结果封闭集两态（Rebased / Conflicted）"
    );
    assert_ne!(
        RebaseOutcome::Rebased,
        RebaseOutcome::Conflicted(vec!["specs/a.md".to_owned()]),
        "冲突清单随变体承载（无冲突 / 有冲突可辨）"
    );

    // 探测失败面（detached HEAD 空输出形态）：显式 Err 透传，调用方决定降级
    struct DetachedVcs;
    impl ArchiveVcsPort for DetachedVcs {
        fn dirty(&self, _root: &Path, _paths: &[&str]) -> bool {
            false
        }
        fn commit_all(&self, _worktree: &Path, _message: &str) -> Result<(), String> {
            Ok(())
        }
        fn branch_merged(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
            Err("detached HEAD：无法判定合入".to_owned())
        }
        fn rebase_branch(&self, _worktree: &Path, _onto: &str) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn ff_merge(&self, _main_root: &Path, _branch: &str) -> Result<(), String> {
            Ok(())
        }
        fn worktree_snapshot(&self, _root: &Path) -> Result<WorktreeSnapshot, String> {
            Err("vcs 调用失败".to_owned())
        }
        fn rebase_continue(&self, _worktree: &Path) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn rebase_abort(&self, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
        fn current_branch(&self, _main_root: &Path) -> Result<String, String> {
            Err("detached HEAD：无当前分支".to_owned())
        }
        fn commit_paths(
            &self,
            _main_root: &Path,
            _paths: &[&str],
            _message: &str,
        ) -> Result<(), String> {
            Err("提交失败".to_owned())
        }
        fn change_edit_set(&self, _root: &Path, _base: &str) -> Result<Vec<String>, String> {
            Err("变更文件清单取数失败".to_owned())
        }
        fn change_edit_patch(&self, _root: &Path, _base: &str) -> Result<String, String> {
            Err("补丁取数失败".to_owned())
        }
        fn remove_worktree(&self, _main_root: &Path, _worktree: &Path) -> Result<(), String> {
            Err("worktree 移除失败".to_owned())
        }
        fn restore_baseline(&self, _worktree: &Path, _exclusions: &[&str]) -> Result<String, String> {
            Err("git 语境失败".to_owned())
        }
    }
    let detached: Arc<dyn ArchiveVcsPort> = Arc::new(DetachedVcs);
    let error = detached
        .current_branch(Path::new("/tmp/root"))
        .expect_err("detached HEAD 应显式 Err（不 panic）");
    assert!(error.contains("detached HEAD"), "Err 语汇透传: {error}");
    assert!(detached.branch_merged(Path::new("/tmp/root"), "change/x").is_err());
    assert!(detached.worktree_snapshot(Path::new("/tmp/wt")).is_err());
    assert!(detached
        .commit_paths(Path::new("/tmp/root"), &["openspec"], "msg")
        .is_err());
}

// ---------------------------------------------------------------------------
// mutation-test 扩展节（desktop-fix-mutation-test）：载荷类型自持（零
// Serialize / 零 checks 依赖）+ MutationRunner 四参缝 + ArchiveVcsPort 清单 /
// 补丁两方法 trait 面
// ---------------------------------------------------------------------------

use crate::port::{
    ArchiveVcsPort, BoxToolFuture, MutationCounts, MutationRunner, MutationSkipReason,
    MutationTestConclusion, MutationTestOutcome,
};

/// 已执行形态 mutation 产出 fixture（skip=None——归并块口径 score / threshold
/// 在位；feedback_text 非空携报告路径指引）。
fn sample_mutation_outcome(conclusion: MutationTestConclusion) -> MutationTestOutcome {
    MutationTestOutcome {
        conclusion,
        skip: None,
        score: 87.5,
        threshold: 80.0,
        counts: MutationCounts {
            killed: 7,
            survived: 1,
            timeout: 0,
            compile_error: 0,
            runtime_error: 0,
            ignored: 2,
            total: 10,
        },
        report_paths: vec![
            "C:/ws/openspec/changes/c/reports/test/app_rust/mutation/mutation-seg-0.json"
                .to_owned(),
            "C:/ws/openspec/changes/c/reports/test/app_rust/mutation/mutation.json".to_owned(),
        ],
        feedback_text: match conclusion {
            MutationTestConclusion::Pass => String::new(),
            MutationTestConclusion::Fail => {
                "突变测试未达标（逐 suite 判定未全过）：归并 score 87.5 < threshold 90.0\n\
                 原始报告路径：\n- mutation/mutation-seg-0.json"
                    .to_owned()
            }
            MutationTestConclusion::Error => {
                "突变执行中断：cargo-mutants 段执行超时（3600 秒）强制终止\n\
                 已产报告路径：\n- mutation/mutation-seg-0.json"
                    .to_owned()
            }
        },
    }
}

/// 可编程 MutationRunner 假件（ProgrammableRunner 同式）：记录四参
/// `(root, name, files, patch)` 逐字透传 + 可编程产出 / Err(String)。
struct FakeMutationRunner {
    calls: Arc<Mutex<Vec<(String, String, Vec<String>, String)>>>,
    /// 收到的取消信号（AC-6 末位形参透传面）
    cancels: Arc<Mutex<Vec<CancelFlag>>>,
    result: Result<MutationTestOutcome, String>,
}

impl FakeMutationRunner {
    fn with_result(
        result: Result<MutationTestOutcome, String>,
    ) -> (
        Self,
        Arc<Mutex<Vec<(String, String, Vec<String>, String)>>>,
    ) {
        let calls: Arc<Mutex<Vec<(String, String, Vec<String>, String)>>> =
            Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: calls.clone(),
                cancels: Arc::new(Mutex::new(Vec::new())),
                result,
            },
            calls,
        )
    }

    /// 取消信号记录句柄（在实例 move 进 `Arc<dyn Trait>` 之前取出）。
    fn cancel_log(&self) -> Arc<Mutex<Vec<CancelFlag>>> {
        Arc::clone(&self.cancels)
    }
}

impl MutationRunner for FakeMutationRunner {
    fn run(
        &self,
        root: &str,
        name: &str,
        files: &[String],
        patch: &str,
        cancel: CancelFlag,
    ) -> BoxToolFuture {
        self.cancels
            .lock()
            .expect("cancels 锁不可中毒")
            .push(cancel);
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push((
                root.to_owned(),
                name.to_owned(),
                files.to_vec(),
                patch.to_owned(),
            ));
        let result = self.result.clone();
        Box::pin(async move { result.map(ToolStepOutput::MutationTest) })
    }
}

/// 正向：`ToolStepOutput::MutationTest` 载荷写面原生类型逐字段相等（全字段
/// conclusion / skip / score / threshold / counts / report_paths /
/// feedback_text——E3 载荷自持面）。
#[test]
fn tool_step_output_mutation_test载荷全字段相等() {
    let fail = sample_mutation_outcome(MutationTestConclusion::Fail);
    let output = ToolStepOutput::MutationTest(fail.clone());
    match output {
        ToolStepOutput::MutationTest(extracted) => {
            assert_eq!(
                extracted, fail,
                "载荷逐字段相等（Clone / PartialEq 派生可用）"
            );
            assert_eq!(extracted.conclusion, MutationTestConclusion::Fail);
            assert_eq!(extracted.skip, None, "已执行形态 skip 恒 None");
            assert_eq!(extracted.score, 87.5, "归并块口径得分");
            assert_eq!(extracted.threshold, 80.0, "首块阈值");
            assert_eq!(
                (
                    extracted.counts.killed,
                    extracted.counts.survived,
                    extracted.counts.timeout,
                    extracted.counts.compile_error,
                    extracted.counts.runtime_error,
                    extracted.counts.ignored,
                    extracted.counts.total,
                ),
                (7, 1, 0, 0, 0, 2, 10),
                "七字段计数逐位在案"
            );
            assert_eq!(extracted.report_paths.len(), 2, "段级 + 归并报告路径清单");
            assert!(
                extracted.feedback_text.contains("score 87.5 < threshold 90.0"),
                "feedback_text 携结论摘要（报告指引式）: {}",
                extracted.feedback_text
            );
        }
        other => panic!("变体漂移: {other:?}"),
    }
}

/// 正向：`MutationTestOutcome` 形态——`skip = None`（已执行）时 score /
/// threshold 为归并块口径值；`feedback_text` 非空（Fail / Error）携报告路径
/// 指引、Pass 恒空串（达标零反馈语义）。
#[test]
fn mutation_test_outcome形态_已执行与feedback_text面() {
    let pass = sample_mutation_outcome(MutationTestConclusion::Pass);
    assert_eq!(pass.skip, None);
    assert!(
        pass.feedback_text.is_empty(),
        "Pass 恒空串（达标 pass 零反馈）"
    );
    let fail = sample_mutation_outcome(MutationTestConclusion::Fail);
    assert!(
        fail.feedback_text.contains("原始报告路径"),
        "Fail feedback_text 携报告路径清单"
    );
    let error = sample_mutation_outcome(MutationTestConclusion::Error);
    assert!(
        error.feedback_text.contains("已产报告路径")
            && error.feedback_text.contains("mutation-seg-0.json"),
        "Error feedback_text 携中断错误摘要 + 已产报告路径清单（双时点②）"
    );
    // 三值互异（conclusion 参与载荷等值）
    assert_ne!(pass, fail, "Pass / Fail 可辨");
    assert_ne!(fail, error, "Fail / Error 可辨");
    assert_eq!(pass.clone(), pass, "PartialEq 自反");
}

/// 边界：`MutationTestOutcome` 边界形态——`skip = Some(…)` 时 score /
/// threshold 置 0.0 占位（null 观察期不判分）；`report_paths` POSIX（反斜杠
/// 不出线——装配点 `to_posix` 口径）。
#[test]
fn mutation_test_outcome边界_skip占位与posix报告路径() {
    for skip in [MutationSkipReason::EmptyDiff, MutationSkipReason::NoExecutor] {
        let skipped = MutationTestOutcome {
            conclusion: MutationTestConclusion::Pass,
            skip: Some(skip),
            score: 0.0,
            threshold: 0.0,
            counts: MutationCounts::default(),
            report_paths: Vec::new(),
            feedback_text: String::new(),
        };
        assert_eq!(
            skipped.conclusion,
            MutationTestConclusion::Pass,
            "跳过面结论恒 Pass（null 观察期不判 fail）"
        );
        assert_eq!(skipped.score, 0.0, "skip 时 score 0.0 占位");
        assert_eq!(skipped.threshold, 0.0, "skip 时 threshold 0.0 占位");
        assert_eq!(
            (skipped.counts.killed, skipped.counts.total),
            (0, 0),
            "skip 面计数零占位"
        );
    }

    // POSIX 报告路径（反斜杠不出线）：装配口径以 to_posix 归一，载荷面断言无反斜杠
    let executed = sample_mutation_outcome(MutationTestConclusion::Fail);
    for path in &executed.report_paths {
        assert!(
            !path.contains('\\'),
            "报告路径 POSIX 形态（反斜杠不出线）: {path}"
        );
    }
}

/// 正向：`MutationTestConclusion` 三值 / `MutationSkipReason` 两值 as_str
/// 线格式小写词逐值断言（`TestExecutionConclusion` 同型）+ 互异。
#[test]
fn mutation_conclusion与skip_reason线格式逐字() {
    assert_eq!(MutationTestConclusion::Pass.as_str(), "pass");
    assert_eq!(MutationTestConclusion::Fail.as_str(), "fail");
    assert_eq!(MutationTestConclusion::Error.as_str(), "error");
    let mut conclusions = vec![
        MutationTestConclusion::Pass.as_str(),
        MutationTestConclusion::Fail.as_str(),
        MutationTestConclusion::Error.as_str(),
    ];
    conclusions.sort_unstable();
    conclusions.dedup();
    assert_eq!(conclusions.len(), 3, "三值互异");

    assert_eq!(MutationSkipReason::EmptyDiff.as_str(), "empty_diff");
    assert_eq!(MutationSkipReason::NoExecutor.as_str(), "no_executor");
    assert_ne!(
        MutationSkipReason::EmptyDiff.as_str(),
        MutationSkipReason::NoExecutor.as_str(),
        "两跳过因互异"
    );
}

/// 边界：`MutationCounts` 七字段逐字段写面 + `u64` 大值（`u64::MAX`）构造不
/// panic（计数面无饱和运算，纯载荷直存）。
#[test]
fn mutation_counts七字段与u64大值构造不panic() {
    let counts = MutationCounts {
        killed: 1,
        survived: 2,
        timeout: 3,
        compile_error: 4,
        runtime_error: 5,
        ignored: 6,
        total: 21,
    };
    // 穷尽解构（无 `..` 兜底——字段增删即编译失败）
    let MutationCounts {
        killed,
        survived,
        timeout,
        compile_error,
        runtime_error,
        ignored,
        total,
    } = counts;
    assert_eq!((killed, survived, timeout), (1, 2, 3));
    assert_eq!((compile_error, runtime_error, ignored, total), (4, 5, 6, 21));

    let huge = MutationCounts {
        killed: u64::MAX,
        survived: u64::MAX,
        timeout: u64::MAX,
        compile_error: u64::MAX,
        runtime_error: u64::MAX,
        ignored: u64::MAX,
        total: u64::MAX,
    };
    assert_eq!(huge.total, u64::MAX, "u64 大值构造不 panic（纯载荷直存）");
    assert_eq!(MutationCounts::default().total, 0, "Default 零占位");
}

/// 边界：Mutation 族载荷**零 Serialize 不进 IPC**——类型面不派生 Serialize
/// （编译期锚：消费面泛型仅要求 Clone + PartialEq，Serialize 约束缺席即证明
/// 载荷不经 bindings / `RunUpdate` 出线）；载荷类型自持（零 checks 依赖）。
#[test]
fn mutation载荷零serialize不进ipc_类型面自持() {
    /// 仅要求 Clone + PartialEq 的消费面（walker 反馈边②进程内消费同型）
    /// ——Serialize 约束缺席即零 IPC 通道。
    fn consume_in_process(outcome: MutationTestOutcome) -> MutationTestOutcome {
        outcome.clone()
    }
    let outcome = consume_in_process(sample_mutation_outcome(MutationTestConclusion::Fail));
    assert_eq!(outcome.conclusion, MutationTestConclusion::Fail);
    // 类型自持：port 流量像自持七字段计数（不经 checks 模型转换出线）
    assert_eq!(outcome.counts.total, 10, "port 流量像自持（零 checks 依赖）");
}

/// 正向：`MutationRunner` trait 面——假实现经 `Arc<dyn …>` 注入、四参
/// `(root, name, files, patch)` 逐字透传（files = 清单入参、patch = 补丁入参，
/// E3）、`Send + Sync` 编译锚、BoxToolFuture resolve 出 `ToolStepOutput::
/// MutationTest`。
#[tokio::test]
async fn mutation_runner假实现经arc注入且四参逐字透传() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn MutationRunner>>();

    let (inner, calls) = FakeMutationRunner::with_result(Ok(sample_mutation_outcome(
        MutationTestConclusion::Pass,
    )));
    let runner: Arc<dyn MutationRunner> = Arc::new(inner);

    let files = vec!["src/lib.rs".to_owned(), "src/util.rs".to_owned()];
    let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n"
        .to_owned();
    let output = runner
        .run(
            "/tmp/workspace-root",
            "demo-change",
            &files,
            &patch,
            CancelFlag::never(),
        )
        .await
        .expect("产出应可用");
    assert!(
        matches!(output, ToolStepOutput::MutationTest(outcome) if outcome.conclusion == MutationTestConclusion::Pass),
        "BoxToolFuture resolve 出 MutationTest 变体"
    );

    let captured = calls.lock().expect("锁").clone();
    assert_eq!(captured.len(), 1, "恰一次调用");
    let (root, name, captured_files, captured_patch) = &captured[0];
    assert_eq!(
        root, "/tmp/workspace-root",
        "root 逐字透传（exec root / worktree）"
    );
    assert_eq!(name, "demo-change", "name 逐字透传（磁盘面 change 名）");
    assert_eq!(
        captured_files, &files,
        "files = 清单入参逐字透传（不截断不改写）"
    );
    assert_eq!(
        captured_patch, &patch,
        "patch = --in-diff 补丁入参逐字透传"
    );
}

/// 异常：假实现产出 `Err` → `BoxToolFuture` 透传不吞（E5 显式 Err 面的缝侧
/// 半边）；`Ok(ToolStepOutput::MutationTest)` 分发正确。
#[tokio::test]
async fn mutation_runner_err臂透传不吞且ok臂分发正确() {
    let (inner, _calls) =
        FakeMutationRunner::with_result(Err("cargo mutants 拉起失败（程序不可达？）".to_owned()));
    let runner: Arc<dyn MutationRunner> = Arc::new(inner);
    let err = runner
        .run("/tmp/root", "c", &[], "", CancelFlag::never())
        .await
        .expect_err("Err 臂应上抛");
    assert_eq!(
        err, "cargo mutants 拉起失败（程序不可达？）",
        "Err(String) 原样（无再包装）"
    );

    let (inner, _calls) = FakeMutationRunner::with_result(Ok(sample_mutation_outcome(
        MutationTestConclusion::Fail,
    )));
    let runner: Arc<dyn MutationRunner> = Arc::new(inner);
    match runner.run("/tmp/root", "c", &[], "", CancelFlag::never()).await {
        Ok(ToolStepOutput::MutationTest(outcome)) => {
            assert_eq!(outcome.conclusion, MutationTestConclusion::Fail);
        }
        other => panic!("分发漂移: {other:?}"),
    }
}

/// 边界：`files` 空 / 单元素 / 超大列表（1000 项）、`patch` 空串 / 超长
/// （>100KB）/ 含特殊字符（`\n` / emoji 文件名）均逐字透传不截断（缝零加工
/// 纪律——突变面裁决归 runner 侧）。
#[tokio::test]
async fn mutation_runner边界入参逐字透传不截断() {
    // 空清单 + 空补丁
    let (inner, calls) = FakeMutationRunner::with_result(Ok(sample_mutation_outcome(
        MutationTestConclusion::Pass,
    )));
    let runner: Arc<dyn MutationRunner> = Arc::new(inner);
    runner
        .run("/tmp/root", "c", &[], "", CancelFlag::never())
        .await
        .expect("空入参应可用");
    {
        let guard = calls.lock().expect("锁");
        assert_eq!(
            guard[0].2,
            Vec::<String>::new(),
            "空清单原样透传（EmptyDiff 判定归 runner）"
        );
        assert_eq!(guard[0].3, "", "空补丁原样透传");
    }

    // 单元素 + emoji 文件名（POSIX 形态）
    let single = vec!["src/文件 🚀.rs".to_owned()];
    runner
        .run("/tmp/root", "c", &single, "patch 文本", CancelFlag::never())
        .await
        .expect("单元素应可用");
    {
        let guard = calls.lock().expect("锁");
        assert_eq!(guard[1].2, single, "单元素清单 + emoji 文件名逐字透传");
    }

    // 超大清单（1000 项）
    let huge: Vec<String> = (0..1000).map(|index| format!("src/f{index:04}.rs")).collect();
    runner
        .run("/tmp/root", "c", &huge, "p", CancelFlag::never())
        .await
        .expect("超大清单应可用");
    {
        let guard = calls.lock().expect("锁");
        assert_eq!(guard[2].2.len(), 1000, "超大清单（1000 项）不截断");
        assert_eq!(guard[2].2, huge, "逐项等值");
    }

    // 超长补丁（>100KB）+ 特殊字符
    let long_patch = format!(
        "{}\npatch 行 🚀 结束",
        "diff --git a/x b/x\n+加行".repeat(4000)
    );
    assert!(long_patch.len() > 100_000, "补丁须越过 100KB 刻度");
    runner
        .run("/tmp/root", "c", &[], &long_patch, CancelFlag::never())
        .await
        .expect("超长补丁应可用");
    {
        let guard = calls.lock().expect("锁");
        assert_eq!(guard[3].3, long_patch, "超长补丁逐字透传（>100KB 不截断）");
        assert!(
            guard[3].3.contains('\n') && guard[3].3.contains('🚀'),
            "特殊字符保真"
        );
    }
}

/// 正向：`ArchiveVcsPort` 清单 / 补丁两方法 trait 面——假实现经 trait object
/// 动态分发记录 `(root, base)` 两参透传（`base = "HEAD"` 形态）；Err 面原文
/// 透传不降级；空清单 / 空补丁 Ok 面透传（与 Err 面严格区分）。
#[tokio::test]
async fn archive_vcs_port清单与补丁两方法trait面() {
    use std::path::Path;

    use crate::port::{RebaseOutcome, WorktreeSnapshot};

    /// 记录两参 + 可编程产出的假件（清单 / 补丁缝观察面）。
    struct RecordingVcs {
        calls: Mutex<Vec<(String, String)>>,
        set: Result<Vec<String>, String>,
        patch: Result<String, String>,
    }
    impl ArchiveVcsPort for RecordingVcs {
        fn dirty(&self, _root: &Path, _paths: &[&str]) -> bool {
            false
        }
        fn commit_all(&self, _worktree: &Path, _message: &str) -> Result<(), String> {
            Ok(())
        }
        fn branch_merged(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
            Ok(false)
        }
        fn rebase_branch(&self, _worktree: &Path, _onto: &str) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn ff_merge(&self, _main_root: &Path, _branch: &str) -> Result<(), String> {
            Ok(())
        }
        fn worktree_snapshot(&self, _root: &Path) -> Result<WorktreeSnapshot, String> {
            Ok(WorktreeSnapshot {
                head: "abc".to_owned(),
                rebase_head: None,
                status: Vec::new(),
                index: Vec::new(),
            })
        }
        fn rebase_continue(&self, _worktree: &Path) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn rebase_abort(&self, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
        fn current_branch(&self, _main_root: &Path) -> Result<String, String> {
            Ok("main".to_owned())
        }
        fn commit_paths(
            &self,
            _main_root: &Path,
            _paths: &[&str],
            _message: &str,
        ) -> Result<(), String> {
            Ok(())
        }
        fn change_edit_set(&self, root: &Path, base: &str) -> Result<Vec<String>, String> {
            self.calls
                .lock()
                .expect("calls 锁")
                .push((root.display().to_string(), base.to_owned()));
            self.set.clone()
        }
        fn change_edit_patch(&self, root: &Path, base: &str) -> Result<String, String> {
            self.calls
                .lock()
                .expect("calls 锁")
                .push((root.display().to_string(), base.to_owned()));
            self.patch.clone()
        }
        fn remove_worktree(&self, _main_root: &Path, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
        fn restore_baseline(&self, _worktree: &Path, _exclusions: &[&str]) -> Result<String, String> {
            Ok(String::new())
        }
    }

    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RecordingVcs>();

    let port: Arc<dyn ArchiveVcsPort> = Arc::new(RecordingVcs {
        calls: Mutex::new(Vec::new()),
        set: Ok(vec!["src/lib.rs".to_owned()]),
        patch: Ok("diff --git a/src/lib.rs b/src/lib.rs\n".to_owned()),
    });
    let root = Path::new("/tmp/ws/root");
    assert_eq!(
        port.change_edit_set(root, "HEAD")
            .expect("清单取数应可用"),
        vec!["src/lib.rs".to_owned()],
        "trait object 动态分发结果透传"
    );
    assert_eq!(
        port.change_edit_patch(root, "HEAD")
            .expect("补丁取数应可用"),
        "diff --git a/src/lib.rs b/src/lib.rs\n",
        "补丁透传"
    );

    // Err 面：显式 Err 原文透传（MUST NOT 降级空清单 / 空补丁）
    let failing: Arc<dyn ArchiveVcsPort> = Arc::new(RecordingVcs {
        calls: Mutex::new(Vec::new()),
        set: Err("git diff 失败（退出码 128）".to_owned()),
        patch: Err("git add -N 失败".to_owned()),
    });
    assert_eq!(
        failing
            .change_edit_set(root, "HEAD")
            .expect_err("Err 面应上抛"),
        "git diff 失败（退出码 128）",
        "Err 原文透传不包装不吞"
    );
    assert_eq!(
        failing
            .change_edit_patch(root, "HEAD")
            .expect_err("Err 面应上抛"),
        "git add -N 失败"
    );

    // 边界：空清单 / 空补丁 Ok 面透传（干净仓形态——与 Err 面严格区分）
    let empty: Arc<dyn ArchiveVcsPort> = Arc::new(RecordingVcs {
        calls: Mutex::new(Vec::new()),
        set: Ok(Vec::new()),
        patch: Ok(String::new()),
    });
    assert_eq!(
        empty.change_edit_set(root, "HEAD").expect("空清单 Ok 面"),
        Vec::<String>::new(),
        "空清单透传（干净仓合法形态）"
    );
    assert_eq!(
        empty.change_edit_patch(root, "HEAD").expect("空补丁 Ok 面"),
        "",
        "空补丁透传（干净仓零 diff 合法形态）"
    );
}

// ---------------------------------------------------------------------------
// ArchiveVcsPort.remove_worktree trait 面（worktree-cleanup-and-active-cap /
// AC-5 / AC-6 / AC-7）：清理步执行缝签名编译锚 + 可编程注入 + Err 面契约 +
// 零探测方法边界 + Send + Sync 契约持衡
// ---------------------------------------------------------------------------

/// `remove_worktree` trait 面可注入（正向 / 异常 / 边界）：测试假件实现
///「既有方法族 + remove_worktree」即编译通过（port 面零探测方法——在场探测驻
/// 调用方 fs 判定，trait 无 probe / exists 方法，编译面锚定）；Ok / Err 可编程
/// 且 `(main_root, worktree)` 双参原样捕获；经 `Arc<dyn ArchiveVcsPort>` 注入
/// 后按可编程结果出线（walker 消费面契约，AC-5 / AC-6）。
#[test]
fn archive_vcs_port_remove_worktree可注入且双参捕获与err契约() {
    use std::path::{Path, PathBuf};

    use crate::port::{ArchiveVcsPort, RebaseOutcome, WorktreeSnapshot};

    /// 清理面假件：remove_worktree / restore_baseline Ok / Err 可编程 + 双参捕获
    ///（git 子进程边界以注入 port 假件替代——本节只锚签名与透传，生产实现的路径
    /// + 手动指引组装内容由 infra/vcs 真件节断言）。
    struct CleanupVcs {
        result: Result<(), String>,
        calls: std::sync::Mutex<Vec<(PathBuf, PathBuf)>>,
        restore: Option<Result<String, String>>,
        restore_calls: std::sync::Mutex<Vec<(PathBuf, Vec<String>)>>,
    }
    impl ArchiveVcsPort for CleanupVcs {
        fn dirty(&self, _root: &Path, _paths: &[&str]) -> bool {
            false
        }
        fn commit_all(&self, _worktree: &Path, _message: &str) -> Result<(), String> {
            Ok(())
        }
        fn branch_merged(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
            Ok(false)
        }
        fn rebase_branch(&self, _worktree: &Path, _onto: &str) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn ff_merge(&self, _main_root: &Path, _branch: &str) -> Result<(), String> {
            Ok(())
        }
        fn worktree_snapshot(&self, _root: &Path) -> Result<WorktreeSnapshot, String> {
            Ok(WorktreeSnapshot {
                head: "abc".to_owned(),
                rebase_head: None,
                status: Vec::new(),
                index: Vec::new(),
            })
        }
        fn rebase_continue(&self, _worktree: &Path) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn rebase_abort(&self, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
        fn current_branch(&self, _main_root: &Path) -> Result<String, String> {
            Ok("main".to_owned())
        }
        fn commit_paths(
            &self,
            _main_root: &Path,
            _paths: &[&str],
            _message: &str,
        ) -> Result<(), String> {
            Ok(())
        }
        fn change_edit_set(&self, _root: &Path, _base: &str) -> Result<Vec<String>, String> {
            Ok(Vec::new())
        }
        fn change_edit_patch(&self, _root: &Path, _base: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String> {
            self.calls
                .lock()
                .expect("清理调用锁")
                .push((main_root.to_owned(), worktree.to_owned()));
            self.result.clone()
        }
        fn restore_baseline(&self, worktree: &Path, exclusions: &[&str]) -> Result<String, String> {
            self.restore_calls
                .lock()
                .expect("restore 调用锁")
                .push((worktree.to_owned(), exclusions.iter().map(|s| (*s).to_owned()).collect()));
            self.restore.clone().expect("restore 可编程面应在场")
        }
    }

    // Send + Sync 契约持衡（跨线程经 Arc<dyn ArchiveVcsPort> 注入 walker 的
    // 编译前提——trait 对象契约随新方法扩展不破）
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CleanupVcs>();

    let main_root = PathBuf::from("/tmp/ws/main");
    let worktree = PathBuf::from("/tmp/ws/worktrees/seg/fix-bug");

    // Ok 面可编程：trait object 动态分发按可编程结果出线，双参原样抵达
    let ok_port: Arc<dyn ArchiveVcsPort> = Arc::new(CleanupVcs {
        result: Ok(()),
        calls: std::sync::Mutex::new(Vec::new()),
        restore: Some(Ok(String::new())),
        restore_calls: std::sync::Mutex::new(Vec::new()),
    });
    ok_port
        .remove_worktree(&main_root, &worktree)
        .expect("Ok 面应可用");
    // 双参捕获断言（经内部假件直查——Arc<dyn> 不可 downcast，另铸同形假件取捕获）
    let probe = CleanupVcs {
        result: Ok(()),
        calls: std::sync::Mutex::new(Vec::new()),
        restore: Some(Ok(String::new())),
        restore_calls: std::sync::Mutex::new(Vec::new()),
    };
    probe.remove_worktree(&main_root, &worktree).expect("Ok 面");
    assert_eq!(
        probe.calls.lock().expect("清理调用锁").as_slice(),
        [(main_root.clone(), worktree.clone())],
        "remove_worktree 双参原样捕获（零改写零换序）"
    );

    // Err 面契约：String 错误串原样透传（port 面零改写零吞）
    let err_port: Arc<dyn ArchiveVcsPort> = Arc::new(CleanupVcs {
        result: Err("git worktree remove 失败（退出码 128）".to_owned()),
        calls: std::sync::Mutex::new(Vec::new()),
        restore: Some(Ok(String::new())),
        restore_calls: std::sync::Mutex::new(Vec::new()),
    });
    let error = err_port
        .remove_worktree(&main_root, &worktree)
        .expect_err("Err 面应上抛");
    assert_eq!(
        error, "git worktree remove 失败（退出码 128）",
        "Err(String) 串契约原样透传（组装内容由 infra/vcs 真件节断言）"
    );
}

/// `ArchiveVcsPort.restore_baseline` trait 面（正向 / 异常 / 边界，
/// check-before-flow-start）：测试假件实现「既有方法族 + restore_baseline」即
/// 编译通过（port 面零探测方法——worktree 在场判定驻调用方，trait 无 probe /
/// exists 方法，编译面锚定）；Ok 恢复摘要串 / Err 可编程 + `(worktree,
/// exclusions)` 载荷捕获——多元素切片（D6 排除面：名单 + `".*"` + 域目录名形态）
/// 与空切片两态原样到达假件；经 `Arc<dyn ArchiveVcsPort>` 注入后按可编程结果
/// 分派（walker 消费面契约）；`Send + Sync` trait 对象契约随新方法持衡。
#[test]
fn archive_vcs_port_restore_baseline可注入且载荷捕获与err契约() {
    use std::path::{Path, PathBuf};

    use crate::port::{ArchiveVcsPort, RebaseOutcome, WorktreeSnapshot};

    /// restore 面假件：可编程 Ok 摘要 / Err + `(worktree, exclusions)` 载荷捕获。
    struct RestoreVcs {
        result: Result<String, String>,
        calls: std::sync::Mutex<Vec<(PathBuf, Vec<String>)>>,
    }
    impl ArchiveVcsPort for RestoreVcs {
        fn dirty(&self, _root: &Path, _paths: &[&str]) -> bool {
            false
        }
        fn commit_all(&self, _worktree: &Path, _message: &str) -> Result<(), String> {
            Ok(())
        }
        fn branch_merged(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
            Ok(false)
        }
        fn rebase_branch(&self, _worktree: &Path, _onto: &str) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn ff_merge(&self, _main_root: &Path, _branch: &str) -> Result<(), String> {
            Ok(())
        }
        fn worktree_snapshot(&self, _root: &Path) -> Result<WorktreeSnapshot, String> {
            Ok(WorktreeSnapshot {
                head: "abc".to_owned(),
                rebase_head: None,
                status: Vec::new(),
                index: Vec::new(),
            })
        }
        fn rebase_continue(&self, _worktree: &Path) -> Result<RebaseOutcome, String> {
            Ok(RebaseOutcome::Rebased)
        }
        fn rebase_abort(&self, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
        fn current_branch(&self, _main_root: &Path) -> Result<String, String> {
            Ok("main".to_owned())
        }
        fn commit_paths(
            &self,
            _main_root: &Path,
            _paths: &[&str],
            _message: &str,
        ) -> Result<(), String> {
            Ok(())
        }
        fn change_edit_set(&self, _root: &Path, _base: &str) -> Result<Vec<String>, String> {
            Ok(Vec::new())
        }
        fn change_edit_patch(&self, _root: &Path, _base: &str) -> Result<String, String> {
            Ok(String::new())
        }
        fn remove_worktree(&self, _main_root: &Path, _worktree: &Path) -> Result<(), String> {
            Ok(())
        }
        fn restore_baseline(&self, worktree: &Path, exclusions: &[&str]) -> Result<String, String> {
            self.calls
                .lock()
                .expect("restore 调用锁")
                .push((
                    worktree.to_owned(),
                    exclusions.iter().map(|s| (*s).to_owned()).collect(),
                ));
            self.result.clone()
        }
    }

    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RestoreVcs>();

    let worktree = PathBuf::from("/tmp/ws/worktrees/seg/fix-bug");

    // Ok 面：恢复摘要串原样出线（step detail 消费面）；D6 排除面多元素切片
    //（名单六名 + ".*" + 域目录名）原样捕获零改写零换序
    let summary = "已重置 3 个路径、清除 2 个未跟踪文件";
    let exclusions = [
        "node_modules",
        "dist",
        "build",
        "target",
        "coverage",
        ".nyc_output",
        ".*",
        "openspec",
    ];
    let ok_port: Arc<dyn ArchiveVcsPort> = Arc::new(RestoreVcs {
        result: Ok(summary.to_owned()),
        calls: std::sync::Mutex::new(Vec::new()),
    });
    let got = ok_port
        .restore_baseline(&worktree, &exclusions)
        .expect("Ok 面应可用");
    assert_eq!(got, summary, "恢复摘要串随 trait object 动态分发出线");

    // 载荷捕获（另铸同形假件直查——Arc<dyn> 不可 downcast）
    let probe = RestoreVcs {
        result: Ok(String::new()),
        calls: std::sync::Mutex::new(Vec::new()),
    };
    probe
        .restore_baseline(&worktree, &exclusions)
        .expect("捕获面应可用");
    assert_eq!(
        probe.calls.lock().expect("restore 调用锁").as_slice(),
        [(
            worktree.clone(),
            exclusions.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()
        )],
        "(worktree, exclusions) 载荷原样捕获（D6 排除面透传契约——零改写零换序）"
    );

    // 空切片形态：纯载荷透传（调用方组装面空集合法形态）
    let probe = RestoreVcs {
        result: Ok(String::new()),
        calls: std::sync::Mutex::new(Vec::new()),
    };
    probe.restore_baseline(&worktree, &[]).expect("空切片应可用");
    assert_eq!(
        probe.calls.lock().expect("restore 调用锁")[0].1,
        Vec::<String>::new(),
        "空切片原样到达（零元素排除面）"
    );

    // Err 面契约：Result<String, String> 错误串原样透传（错误串组装内容归
    // infra/vcs 真件节——port 面锚签名与透传，不重复断言文案）
    let err_port: Arc<dyn ArchiveVcsPort> = Arc::new(RestoreVcs {
        result: Err("git -C /tmp/ws/worktrees/seg/fix-bug clean 失败（退出码 128）".to_owned()),
        calls: std::sync::Mutex::new(Vec::new()),
    });
    let error = err_port
        .restore_baseline(&worktree, &exclusions)
        .expect_err("Err 面应上抛");
    assert!(
        error.contains("clean"),
        "Err(String) 串契约原样透传（git 语境不吞不静默）: {error}"
    );
}

// ---------------------------------------------------------------------------
// 取消贯通（test-design「port.rs -> port_test.rs」节，AC-6）：`ToolStepRequest.cancel`
// 字段与三 runner trait 的 `cancel` 形参。`CancelFlag` 的相等语义是身份比较
//（`Arc::ptr_eq`）——形参透传断言因此以「同一原子」为准，而非状态值。
// ---------------------------------------------------------------------------

/// 构造只含取消面的最小请求（`root` / `command` 固定，唯一变量是 flag）。
fn request_with_cancel(cancel: CancelFlag) -> crate::port::ToolStepRequest {
    crate::port::ToolStepRequest {
        root: "/tmp/root".to_owned(),
        command: ToolCommand::StaticCheck {
            change_id: CHANGE_ID.to_owned(),
            slot: ReportSlot::Test,
        },
        cancel,
    }
}

/// 正向：同一 flag（clone）的两份请求相等——`cancel` 的 identity 语义使
/// `derive(PartialEq)` 原样成立。
#[test]
fn tool_step_request同一flag的克隆相等() {
    let cancel = CancelFlag::never();
    let first = request_with_cancel(cancel.clone());
    let second = request_with_cancel(cancel.clone());
    assert_eq!(first, second, "同 root / command / flag → 请求相等");
    assert_eq!(first.cancel, second.cancel, "flag 身份相等（同一原子）");
}

/// 边界：同 root / command 但各自 `never()` → 请求不等（身份比较，非状态值
/// 比较——两个都未置位也不相等）。
#[test]
fn tool_step_request独立flag不相等() {
    let first = request_with_cancel(CancelFlag::never());
    let second = request_with_cancel(CancelFlag::never());
    assert_ne!(first, second, "独立铸造的 flag 使请求不等（身份比较）");
    assert!(!first.cancel.is_set() && !second.cancel.is_set(), "两者状态相同");
}

/// 边界：`clone` 保持身份——`let b = a.clone(); assert_eq!(a.cancel, b.cancel);`
///（clone 共享同一原子，非重新铸造）。
#[test]
fn tool_step_request_clone保持flag身份() {
    let request = request_with_cancel(CancelFlag::never());
    let cloned = request.clone();
    assert_eq!(request.cancel, cloned.cancel, "clone 后 flag 身份相等");
    assert_eq!(request, cloned, "clone 与原件整体相等");
    cloned.cancel.handle().set();
    assert!(request.cancel.is_set(), "clone 侧置位对原件可见（同一原子）");
}

/// 边界（参数化 2 形态）：`Debug` 渲染——`ToolStepRequest` 的派生 `Debug` 与
/// `CancelFlag` 的手写 `Debug` 协同，含 `cancel: CancelFlag(set = false|true)`。
#[test]
fn tool_step_request_debug渲染含cancel形态() {
    let request = request_with_cancel(CancelFlag::never());
    let before = format!("{request:?}");
    assert!(
        before.contains("cancel: CancelFlag(set = false)"),
        "未置位渲染: {before}"
    );

    request.cancel.handle().set();
    let after = format!("{request:?}");
    assert!(
        after.contains("cancel: CancelFlag(set = true)"),
        "置位后渲染: {after}"
    );
}

/// 正向（AC-6）：三 trait 各一假实现接收 `cancel` —— 调用后记录的 flag 与传入
/// 的 `assert_eq!`（同一原子），三 trait 逐个验证形参在位且原样透传。
#[tokio::test]
async fn 三runner_trait接收cancel形参且原样透传() {
    // StaticCheckRunner
    let (static_inner, _static_calls) = ProgrammableRunner::with_outcome(StaticCheckOutcome {
        passed: true,
        diagnostics: String::new(),
        output_ref: None,
        report_dir: String::new(),
    });
    let static_log = static_inner.cancel_log();
    let static_runner: Arc<dyn StaticCheckRunner> = Arc::new(static_inner);
    let static_cancel = CancelFlag::never();
    static_runner
        .run("/tmp/root", "c", ReportSlot::Test, static_cancel.clone())
        .await
        .expect("StaticCheckRunner 假件应 Ok");
    let recorded = static_log.lock().expect("static cancel 锁").clone();
    assert_eq!(recorded.len(), 1, "恰一次调用");
    assert_eq!(recorded[0], static_cancel, "第四形参原样透传（同一原子）");

    // TestExecutionRunner
    let (exec_inner, _exec_calls) = ProgrammableTestExecutionRunner::with_result(Ok(
        sample_execution_outcome(TestExecutionConclusion::Pass),
    ));
    let exec_log = exec_inner.cancel_log();
    let exec_runner: Arc<dyn TestExecutionRunner> = Arc::new(exec_inner);
    let exec_cancel = CancelFlag::never();
    exec_runner
        .run("/tmp/root", "c", ReportSlot::Test, exec_cancel.clone())
        .await
        .expect("TestExecutionRunner 假件应 Ok");
    let recorded = exec_log.lock().expect("exec cancel 锁").clone();
    assert_eq!(recorded.len(), 1, "恰一次调用");
    assert_eq!(recorded[0], exec_cancel, "第四形参原样透传（同一原子）");

    // MutationRunner
    let (mutation_inner, _mutation_calls) = FakeMutationRunner::with_result(Ok(
        sample_mutation_outcome(MutationTestConclusion::Pass),
    ));
    let mutation_log = mutation_inner.cancel_log();
    let mutation_runner: Arc<dyn MutationRunner> = Arc::new(mutation_inner);
    let mutation_cancel = CancelFlag::never();
    mutation_runner
        .run("/tmp/root", "c", &[], "", mutation_cancel.clone())
        .await
        .expect("MutationRunner 假件应 Ok");
    let recorded = mutation_log.lock().expect("mutation cancel 锁").clone();
    assert_eq!(recorded.len(), 1, "恰一次调用");
    assert_eq!(recorded[0], mutation_cancel, "末位形参原样透传（同一原子）");
}

/// 边界（表驱动 3 trait）：已置位信号不被丢弃——传入 `never()` 后先 `set()`
/// 再调 → 假实现内观测 `is_set() == true`（信号原样透传，不重建、不清零）。
#[tokio::test]
async fn 三runner_trait已置位信号不被丢弃() {
    let cancel = CancelFlag::never();
    cancel.handle().set();

    let (static_inner, _) = ProgrammableRunner::with_outcome(StaticCheckOutcome {
        passed: true,
        diagnostics: String::new(),
        output_ref: None,
        report_dir: String::new(),
    });
    let static_log = static_inner.cancel_log();
    let static_runner: Arc<dyn StaticCheckRunner> = Arc::new(static_inner);
    static_runner
        .run("/tmp/root", "c", ReportSlot::Test, cancel.clone())
        .await
        .expect("应 Ok");
    assert!(
        static_log.lock().expect("锁")[0].is_set(),
        "StaticCheckRunner 收到已置位信号（不重建不清零）"
    );

    let (exec_inner, _) = ProgrammableTestExecutionRunner::with_result(Ok(
        sample_execution_outcome(TestExecutionConclusion::Pass),
    ));
    let exec_log = exec_inner.cancel_log();
    let exec_runner: Arc<dyn TestExecutionRunner> = Arc::new(exec_inner);
    exec_runner
        .run("/tmp/root", "c", ReportSlot::Test, cancel.clone())
        .await
        .expect("应 Ok");
    assert!(
        exec_log.lock().expect("锁")[0].is_set(),
        "TestExecutionRunner 收到已置位信号"
    );

    let (mutation_inner, _) = FakeMutationRunner::with_result(Ok(sample_mutation_outcome(
        MutationTestConclusion::Pass,
    )));
    let mutation_log = mutation_inner.cancel_log();
    let mutation_runner: Arc<dyn MutationRunner> = Arc::new(mutation_inner);
    mutation_runner
        .run("/tmp/root", "c", &[], "", cancel.clone())
        .await
        .expect("应 Ok");
    assert!(
        mutation_log.lock().expect("锁")[0].is_set(),
        "MutationRunner 收到已置位信号"
    );
}
