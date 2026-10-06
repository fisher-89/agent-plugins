use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use foundation::layout::resolve;

use crate::port::{
    BoxToolFuture, StaticCheckOutcome, StaticCheckRunner, TestExecutionConclusion,
    TestExecutionOutcome, TestExecutionRunner, ToolCommand, ToolStepOutput, ToolStepPort,
    ToolStepRequest,
};
use crate::steps::LocalToolSteps;
use workflow::write::{BacktrackInput, PhaseLogInput, SessionAnchors};

// ---------------------------------------------------------------------------
// 装置：tempdir fixture + 假 StaticCheckRunner
// ---------------------------------------------------------------------------

/// 临时 workspace 根 RAII（沿 snapshot_test 装置先例）。
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "orchestration-steps-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn root_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    fn change(&self, name: &str, workflow_json: &str) {
        let dir = self.0.join("openspec/changes").join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        fs::write(dir.join("workflow.json"), workflow_json).expect("写 workflow.json 失败");
    }

    fn workflow_json(&self, name: &str) -> serde_json::Value {
        let text = fs::read_to_string(
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
        let _ = fs::remove_dir_all(&self.0);
    }
}

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

/// 假 TestExecutionRunner：记录 root / change 调用、恒 pass 产出（分发臂
/// 委托面用；行为面归 walker_test 假 runner 装置）。
struct FakeTestExecutionRunner {
    calls: Mutex<Vec<(String, String)>>,
}

impl TestExecutionRunner for FakeTestExecutionRunner {
    fn run(&self, root: &str, change: &str) -> BoxToolFuture {
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push((root.to_owned(), change.to_owned()));
        Box::pin(async move {
            Ok(ToolStepOutput::TestExecution(TestExecutionOutcome {
                conclusion: TestExecutionConclusion::Pass,
                total: 0,
                passed: 0,
                failed: 0,
                skipped: 0,
                findings_brief: String::new(),
                report_dir: String::new(),
            }))
        })
    }
}

/// 组合根装配：run 级锚点 + 注入双 runner（与命令层同式）。
fn assemble(steps_static_check: FakeRunner) -> (Arc<LocalToolSteps>, Arc<FakeRunner>) {
    let runner = Arc::new(steps_static_check);
    let steps = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::clone(&runner) as Arc<dyn StaticCheckRunner>,
        Arc::new(FakeTestExecutionRunner {
            calls: Mutex::new(Vec::new()),
        }),
    ));
    (steps, runner)
}

/// 一次步执行（await 收口）。
async fn run_step(steps: &LocalToolSteps, root: &str, command: ToolCommand) -> ToolStepOutput {
    steps
        .run(ToolStepRequest {
            root: root.to_owned(),
            command,
        })
        .await
        .expect("步执行应成功")
}

const CHANGE: &str = "demo-change";

// ---------------------------------------------------------------------------
// 链路 ToolCommand → workflow::write → workflow.json 持久化
// ---------------------------------------------------------------------------

/// `ToolCommand.PhaseNext → workflow::write → workflow.json`：真实
/// LocalToolSteps + tempdir 真盘 fixture——路由产出与直调 phase_next 一致
///（AC-6「进程内直调」的动态证据）。
#[tokio::test]
async fn phase_next步链路直调写面产出与直调一致() {
    let root = TempRoot::new("phase-next");
    root.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

    let (steps, _runner) = assemble(FakeRunner::passing());
    let anchors = Arc::new(SessionAnchors::new());

    let output = run_step(
        &steps,
        &root.root_str(),
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

    // 与直调写面逐字段一致（同 root、同锚点实例语义——进程内直调证据）
    let direct = workflow::write::phase_next(&resolve(&root.0), CHANGE, "run-direct", &anchors)
        .expect("直调应成功");
    assert_eq!(outcome.next_phase, direct.next_phase);
    assert_eq!(outcome.done, direct.done);
    assert_eq!(outcome.round, direct.round);
    assert_eq!(outcome.executor, direct.executor);
    assert_eq!(outcome.evaluator, direct.evaluator);
    assert_eq!(
        outcome.allowed_backtrack_phases, direct.allowed_backtrack_phases,
        "白名单随行下发一致"
    );

    // 只读路由：workflow.json 零变更
    let doc = root.workflow_json(CHANGE);
    assert!(doc["eval"].as_array().expect("eval").is_empty());
    assert!(doc.get("active_phase").map_or(true, |v| v.is_null()));
}

/// `PhaseStart → active_phase 落盘`：命令 → tempdir workflow.json active_phase
/// 定点写入、ToolStepOutput::PhaseStart 承接（AC-1 ②步链路同源）。
#[tokio::test]
async fn phase_start步链路落盘active_phase() {
    let root = TempRoot::new("phase-start");
    root.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);

    let (steps, _) = assemble(FakeRunner::passing());
    let output = run_step(
        &steps,
        &root.root_str(),
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

    let doc = root.workflow_json(CHANGE);
    assert_eq!(doc["active_phase"]["phase"], serde_json::json!("implement"));
    assert_eq!(doc["active_phase"]["attempt"], serde_json::json!(1));
    assert!(
        doc["active_phase"]["start_at"].as_str().is_some(),
        "start_at ISO 串落盘"
    );
}

/// `PhaseLog → eval 追加落盘`：命令携 PhaseLogInput → eval 条目落盘、
/// ToolStepOutput::PhaseLog 承接（W9 纯追加语义经缝透传不变形）。
#[tokio::test]
async fn phase_log步链路追加eval条目() {
    let root = TempRoot::new("phase-log");
    root.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "implement", "attempt": 1, "verdict": "pass", "report": "首轮已过", "checklist": [], "stale": true }
  ],
  "active_phase": { "phase": "implement", "attempt": 2, "start_at": "2026-10-01T08:00:00Z" }
}"#,
    );

    let (steps, _) = assemble(FakeRunner::passing());
    let output = run_step(
        &steps,
        &root.root_str(),
        ToolCommand::PhaseLog {
            change: CHANGE.to_owned(),
            phase: "implement".to_owned(),
            input: PhaseLogInput {
                phase: "implement".to_owned(),
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
    // W9 纯追加：既有 pass 条目在场仍追加、attempt = 既有条目数 + 1
    assert_eq!(outcome.attempt, 2, "经缝透传不变形（纯追加）");

    let doc = root.workflow_json(CHANGE);
    let eval = doc["eval"].as_array().expect("eval 数组");
    assert_eq!(eval.len(), 2, "纯追加：不覆盖历史条目");
    assert_eq!(eval[1]["report"], serde_json::json!("重评通过"));
    assert_eq!(eval[1]["verdict"], serde_json::json!("pass"));
    assert_eq!(
        eval[1]["start_at"],
        serde_json::json!("2026-10-01T08:00:00Z")
    );
    assert!(doc["active_phase"].is_null(), "落账后 active_phase 清除");
}

/// `Backtrack → stale 标记落盘`：命令携 BacktrackInput（allowed 随行）→ 落盘
/// 标记、ToolStepOutput::Backtrack 承接。
#[tokio::test]
async fn backtrack步链路落盘stale标记() {
    let root = TempRoot::new("backtrack");
    root.change(
        CHANGE,
        r#"{
  "workflow_type": "requirement",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [] },
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [] }
  ]
}"#,
    );

    let (steps, _) = assemble(FakeRunner::passing());
    let output = run_step(
        &steps,
        &root.root_str(),
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

    let outcome = match output {
        ToolStepOutput::Backtrack(outcome) => outcome,
        other => panic!("产出应为 Backtrack 变体，实际: {other:?}"),
    };
    assert_eq!(outcome.phase, "dev-design");
    assert_eq!(outcome.target, "proposal");

    let doc = root.workflow_json(CHANGE);
    assert_eq!(
        doc["eval"][1]["backtrack_to"],
        serde_json::json!("proposal"),
        "最新条目标记落盘"
    );
    assert_eq!(
        doc["eval"][0]["stale"],
        serde_json::json!(true),
        "目标相位 pass 条目标 stale"
    );
}

/// StaticCheck 委托注入 runner：命令 → 注入假 runner 记录 root 透传、
/// StaticCheckOutcome 原样包装（AC-4 步词汇不变的实现端换血——W4）。
#[tokio::test]
async fn static_check步委托注入runner且root透传() {
    let root = TempRoot::new("static-check");
    let (steps, runner) = assemble(FakeRunner::failing("clippy: 3 warnings\n1 error"));

    let output = run_step(&steps, &root.root_str(), ToolCommand::StaticCheck).await;

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
        runner.roots.lock().expect("锁").as_slice(),
        [root.root_str().as_str()],
        "root 透传注入 runner（spawn cwd 语义）"
    );
}

// ---------------------------------------------------------------------------
// 异常：写面 Err 统一上抛 / runner Err 透传
// ---------------------------------------------------------------------------

/// steps 写面 Err 统一上抛：change 不存在 / 相位非法 → 步返回 Err(String)
///（无 ToolStepError 包装——port 换血对端）。
#[tokio::test]
async fn 写面err统一以err_string上抛() {
    let root = TempRoot::new("err-passthrough");
    root.change(CHANGE, r#"{ "workflow_type": "requirement", "eval": [] }"#);
    let (steps, _) = assemble(FakeRunner::passing());

    // change 不存在
    let err = steps
        .run(ToolStepRequest {
            root: root.root_str(),
            command: ToolCommand::PhaseNext {
                change: "不存在的-change".to_owned(),
                run_id: "run-1".to_owned(),
            },
        })
        .await
        .expect_err("未知 change 应 Err");
    assert!(!err.is_empty(), "Err(String) 显式：{err}");

    // 相位非法（workflow_type 合法但 phase 不在表）
    let err = steps
        .run(ToolStepRequest {
            root: root.root_str(),
            command: ToolCommand::PhaseStart {
                change: CHANGE.to_owned(),
                phase: "幽灵相位".to_owned(),
            },
        })
        .await
        .expect_err("非法相位应 Err");
    assert!(err.contains("幽灵相位"), "错误透传写面记因：{err}");

    // workflow_type 非 requirement 同以 Err(String) 透传
    root.change(
        "bad-type-change",
        r#"{ "workflow_type": "bug-fix", "eval": [] }"#,
    );
    let err = steps
        .run(ToolStepRequest {
            root: root.root_str(),
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
    let root = TempRoot::new("runner-err");
    let (steps, _) = assemble(FakeRunner {
        roots: Mutex::new(Vec::new()),
        result: Err("static-check 命令拉起失败".to_owned()),
    });

    let err = steps
        .run(ToolStepRequest {
            root: root.root_str(),
            command: ToolCommand::StaticCheck,
        })
        .await
        .expect_err("runner Err 应透传");
    assert_eq!(
        err, "static-check 命令拉起失败",
        "Err(String) 原样（无再包装）"
    );
}

// ---------------------------------------------------------------------------
// 边界：锚点实例复用语义（W7 per-run 装配锚）
// ---------------------------------------------------------------------------

/// 锚点实例复用语义：同一 LocalToolSteps 连续 PhaseNext（同 change, run_id）→
/// SessionAnchors 基线共享；换 run_id 基线独立（W7 per-run 装配锚）。
#[tokio::test]
async fn 锚点实例随steps复用且run_id隔离() {
    let root = TempRoot::new("anchor-reuse");
    root.change(
        CHANGE,
        r#"{ "workflow_type": "requirement", "eval": [ { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" } ] }"#,
    );
    let (steps, _) = assemble(FakeRunner::passing());
    let root_str = root.root_str();

    // 同 (change, run_id)：基线 1 复用
    let first = match run_step(
        &steps,
        &root_str,
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
    fs::write(
        root.0.join("openspec/changes").join(CHANGE).join("workflow.json"),
        r#"{ "workflow_type": "requirement", "eval": [
            { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [], "timestamp": "2026-10-01T08:00:00Z" },
            { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "未过", "checklist": [], "timestamp": "2026-10-01T08:10:00Z" }
        ] }"#,
    )
    .expect("改写 workflow.json 失败");
    let second = match run_step(
        &steps,
        &root_str,
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
        &steps,
        &root_str,
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

// ---------------------------------------------------------------------------
// test-execution 扩展节（desktop-checks-domain）
// ---------------------------------------------------------------------------

/// 可编程 TestExecutionRunner 假实现（记录 root / change 调用、可编程产出 /
/// Err——分发臂透传与 Err 上抛双形态驱动）。
struct ProgrammableExecutionRunner {
    calls: Arc<Mutex<Vec<(String, String)>>>,
    result: Result<TestExecutionOutcome, String>,
}

impl ProgrammableExecutionRunner {
    fn passing() -> (Arc<Self>, Arc<Mutex<Vec<(String, String)>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Arc::new(Self {
                calls: Arc::clone(&calls),
                result: Ok(TestExecutionOutcome {
                    conclusion: TestExecutionConclusion::Pass,
                    total: 5,
                    passed: 4,
                    failed: 0,
                    skipped: 1,
                    findings_brief: "全部测试通过且覆盖率达阈值".to_owned(),
                    report_dir: "reports/test/app_node-test".to_owned(),
                }),
            }),
            calls,
        )
    }

    fn failing(message: &str) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            result: Err(message.to_owned()),
        }
    }
}

impl TestExecutionRunner for ProgrammableExecutionRunner {
    fn run(&self, root: &str, change: &str) -> BoxToolFuture {
        self.calls
            .lock()
            .expect("calls 锁不可中毒")
            .push((root.to_owned(), change.to_owned()));
        let result = self.result.clone();
        Box::pin(async move { result.map(ToolStepOutput::TestExecution) })
    }
}

/// 正向：三参构造——锚点 + static_check + test_execution 双 runner 注入
///（组合根同式装配的进程内证据半边；既有 assemble 装置以恒过假件占位，本
/// 用例以可编程假件显式注入并驱动分发臂）。
#[tokio::test]
async fn 三参构造双runner注入_分发臂委托() {
    let root = TempRoot::new("three-arg");
    let (execution_runner, calls) = ProgrammableExecutionRunner::passing();
    let steps = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(FakeRunner::passing()) as Arc<dyn StaticCheckRunner>,
        Arc::clone(&execution_runner) as Arc<dyn TestExecutionRunner>,
    ));

    let output = run_step(
        &steps,
        &root.root_str(),
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
        calls.lock().expect("锁").as_slice(),
        [(root.root_str().as_str().to_owned(), CHANGE.to_owned())],
        "root / change 透传注入 runner（与 StaticCheck 臂同型——AC-3 port 消费面）"
    );
}

/// 异常：TestExecutionRunner Err(String) 臂原样上抛（步层不吞错——run 显式
/// 失败面，经 run_tool 映射 run 终态的输入）。
#[tokio::test]
async fn test_execution_runner_err原样上抛() {
    let root = TempRoot::new("execution-err");
    let steps = Arc::new(LocalToolSteps::new(
        Arc::new(SessionAnchors::new()),
        Arc::new(FakeRunner::passing()) as Arc<dyn StaticCheckRunner>,
        Arc::new(ProgrammableExecutionRunner::failing("报告子目录创建失败"))
            as Arc<dyn TestExecutionRunner>,
    ));

    let err = steps
        .run(ToolStepRequest {
            root: root.root_str(),
            command: ToolCommand::TestExecution {
                change: CHANGE.to_owned(),
            },
        })
        .await
        .expect_err("runner Err 应透传");
    assert_eq!(err, "报告子目录创建失败", "Err(String) 原样（无再包装）");
}
