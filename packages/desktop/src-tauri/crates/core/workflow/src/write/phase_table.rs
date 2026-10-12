use agent::ModelLevel;

use crate::state::{ArtifactSpec, OnFail, StepDefinition, StepNode};

/// 相位角色的角色知识 prompt 与模型档位：prompt 为编译期静态文本
///（`include_str!` 显式字面路径装配产物），无 agent_type 字段、无占位符。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseAgentSpec {
    pub prompt: &'static str,
    pub model_level: ModelLevel,
}

/// 单相位定义（requirement 表静态项）：计划图节点（有序 step 序列 + 产物清单）
/// 与角色 prompt spec 的单源载体。code-review / acceptance 为评估-only 相位，
/// executor 为 `None`（其 step 序列不含 Executor 步）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseDefinition {
    pub id: &'static str,
    pub description: &'static str,
    /// 相位内有序 step 序列（计划图；walker 迭代本序列执行，MUST NOT 自持步序）
    pub steps: &'static [StepDefinition],
    /// 相位声明产物清单（相对路径 → phase 归属权威来源）
    pub artifact_manifest: &'static [ArtifactSpec],
    pub executor: Option<PhaseAgentSpec>,
    pub evaluator: Option<PhaseAgentSpec>,
}

/// 重试上限。
pub const MAX_RETRY_TIMES: u32 = 5;

/// 落账 report 的长度上限（与插件 `phaseLogSchema.report` max 2000 同源）。
pub(crate) const MAX_REPORT_CHARS: usize = 2000;

/// 回溯 reason 的长度上限（与插件 `backtrackInputSchema.backtrack_reason`
/// max 500 同源）。
pub(crate) const MAX_REASON_CHARS: usize = 500;

/// V1 唯一支持的相位表键（W8：其余 workflow_type 在发起前置校验显式拒绝）。
const REQUIREMENT_WORKFLOW_TYPE: &str = "requirement";

/// 会话窗口轮次上限（与插件 `checkRoundLimit` 同源；超限显式 `Err`）。
pub(crate) const MAX_ROUNDS: u32 = 20;

/// 门禁反馈边独立计数上限
pub(crate) const FEEDBACK_LOOP_LIMIT: u32 = 5;

// --- 相位 prompt 静态装配单源（`include_str!` 显式字面路径；executor /
// evaluator spec 与计划步 `prompt_ref` 同引本组常量，防两处字面路径漂移）----

const PROPOSAL_EXECUTOR_PROMPT: &str = include_str!("../prompts/proposal-planner.md");
const PROPOSAL_EVALUATOR_PROMPT: &str = include_str!("../prompts/proposal-evaluator.md");
const DEV_DESIGN_EXECUTOR_PROMPT: &str = include_str!("../prompts/dev-design-planner.md");
const DEV_DESIGN_EVALUATOR_PROMPT: &str = include_str!("../prompts/dev-design-evaluator.md");
const TEST_DESIGN_EXECUTOR_PROMPT: &str = include_str!("../prompts/test-design-planner.md");
const TEST_DESIGN_EVALUATOR_PROMPT: &str = include_str!("../prompts/test-design-evaluator.md");
const IMPLEMENT_EXECUTOR_PROMPT: &str = include_str!("../prompts/implementation-generator.md");
const IMPLEMENT_EVALUATOR_PROMPT: &str = include_str!("../prompts/implementation-evaluator.md");
const TEST_GEN_EXECUTOR_PROMPT: &str = include_str!("../prompts/test-gen-generator.md");
const TEST_GEN_EVALUATOR_PROMPT: &str = include_str!("../prompts/test-gen-evaluator.md");
const TEST_EXECUTION_EXECUTOR_PROMPT: &str = include_str!("../prompts/test-execution-executor.md");
const TEST_EXECUTION_EVALUATOR_PROMPT: &str =
    include_str!("../prompts/test-execution-evaluator.md");
const CODE_REVIEW_EVALUATOR_PROMPT: &str = include_str!("../prompts/code-review-evaluator.md");
const ACCEPTANCE_EVALUATOR_PROMPT: &str = include_str!("../prompts/acceptance-evaluator.md");

// --- 相位计划步序列（编译期 const 有序序列；kind / role 由 `StepNode` 派生，
// 门禁步由 step 声明而非 walker 常量注入）---------------------------------

static PROPOSAL_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Executor,
        prompt_ref: Some(PROPOSAL_EXECUTOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Evaluator,
        prompt_ref: Some(PROPOSAL_EVALUATOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::VerdictGate,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

static DEV_DESIGN_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Executor,
        prompt_ref: Some(DEV_DESIGN_EXECUTOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Evaluator,
        prompt_ref: Some(DEV_DESIGN_EVALUATOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::VerdictGate,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

static TEST_DESIGN_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Executor,
        prompt_ref: Some(TEST_DESIGN_EXECUTOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Evaluator,
        prompt_ref: Some(TEST_DESIGN_EVALUATOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::VerdictGate,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

static IMPLEMENT_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Executor,
        prompt_ref: Some(IMPLEMENT_EXECUTOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::StaticCheck,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::Evaluator,
        prompt_ref: Some(IMPLEMENT_EVALUATOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::VerdictGate,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

static TEST_GEN_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Executor,
        prompt_ref: Some(TEST_GEN_EXECUTOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::StaticCheck,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::Evaluator,
        prompt_ref: Some(TEST_GEN_EVALUATOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::VerdictGate,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

/// test-execution 相位步序列：确定性门禁步承接（绿跑零 agent——无 Executor /
/// Evaluator / VerdictGate 步），pass 后 PhaseLog 步携机械 checklist 落账。
static TEST_EXECUTION_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::TestExecution,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

/// archive 相位步序列（10 步；确定性相位第二例）
static ARCHIVE_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Preflight,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Commit,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Rebase,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::ResolveConflicts,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Merge,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::SpecSync,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Seal,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Finalize,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

static CODE_REVIEW_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Evaluator,
        prompt_ref: Some(CODE_REVIEW_EVALUATOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::VerdictGate,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

static ACCEPTANCE_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Evaluator,
        prompt_ref: Some(ACCEPTANCE_EVALUATOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::VerdictGate,
        prompt_ref: None,
        on_fail: Some(OnFail::FeedbackLoop {
            limit: FEEDBACK_LOOP_LIMIT,
        }),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

// --- 相位产物清单（相对路径 → phase 归属权威来源；无产物相位清单为空）-----

/// proposal 相位产物清单（`specs/**` 前缀 glob 覆盖多规格深路径）。
static PROPOSAL_MANIFEST: &[ArtifactSpec] = &[
    ArtifactSpec {
        kind: "markdown-doc",
        path: "proposal.md",
        title: "提案",
    },
    ArtifactSpec {
        kind: "markdown-doc",
        path: "specs/**",
        title: "规格",
    },
];

/// dev-design 相位产物清单（tasks.md 为 `tasks-progress` 特化视图）。
static DEV_DESIGN_MANIFEST: &[ArtifactSpec] = &[
    ArtifactSpec {
        kind: "markdown-doc",
        path: "design.md",
        title: "设计",
    },
    ArtifactSpec {
        kind: "tasks-progress",
        path: "tasks.md",
        title: "任务进度",
    },
];

/// test-execution 相位产物清单（测试报告树）。
static TEST_EXECUTION_MANIFEST: &[ArtifactSpec] = &[ArtifactSpec {
    kind: "markdown-doc",
    path: "test-reports/**",
    title: "测试报告",
}];

/// requirement 相位表（9 个路由相位 = 8 个 agent 相位 + archive 确定性相位；
/// 编译期静态装配：14 个角色知识 md 逐条 `include_str!` 显式字面路径；全静态零
/// 占位故全表 const 化，驻留 `'static` 只读共享）。每相位同时声明计划步序列
///（steps）与产物清单（artifact_manifest）——计划图单源，walker 不自持步序、
/// 产物发现不落第二份归属表。
static REQUIREMENT_TABLE: &[PhaseDefinition] = &[
    PhaseDefinition {
        id: "proposal",
        description: "需求提案与规格说明",
        steps: PROPOSAL_STEPS,
        artifact_manifest: PROPOSAL_MANIFEST,
        executor: Some(PhaseAgentSpec {
            prompt: PROPOSAL_EXECUTOR_PROMPT,
            model_level: ModelLevel::High,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: PROPOSAL_EVALUATOR_PROMPT,
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "dev-design",
        description: "详细设计与任务拆解",
        steps: DEV_DESIGN_STEPS,
        artifact_manifest: DEV_DESIGN_MANIFEST,
        executor: Some(PhaseAgentSpec {
            prompt: DEV_DESIGN_EXECUTOR_PROMPT,
            model_level: ModelLevel::High,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: DEV_DESIGN_EVALUATOR_PROMPT,
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "test-design",
        description: "测试设计",
        steps: TEST_DESIGN_STEPS,
        artifact_manifest: &[],
        executor: Some(PhaseAgentSpec {
            prompt: TEST_DESIGN_EXECUTOR_PROMPT,
            model_level: ModelLevel::High,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: TEST_DESIGN_EVALUATOR_PROMPT,
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "implement",
        description: "代码实现",
        steps: IMPLEMENT_STEPS,
        artifact_manifest: &[],
        executor: Some(PhaseAgentSpec {
            prompt: IMPLEMENT_EXECUTOR_PROMPT,
            model_level: ModelLevel::Low,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: IMPLEMENT_EVALUATOR_PROMPT,
            model_level: ModelLevel::Low,
        }),
    },
    PhaseDefinition {
        id: "test-gen",
        description: "测试代码生成",
        steps: TEST_GEN_STEPS,
        artifact_manifest: &[],
        executor: Some(PhaseAgentSpec {
            prompt: TEST_GEN_EXECUTOR_PROMPT,
            model_level: ModelLevel::Low,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: TEST_GEN_EVALUATOR_PROMPT,
            model_level: ModelLevel::Low,
        }),
    },
    PhaseDefinition {
        id: "test-execution",
        description: "测试执行与诊断",
        steps: TEST_EXECUTION_STEPS,
        artifact_manifest: TEST_EXECUTION_MANIFEST,
        executor: Some(PhaseAgentSpec {
            prompt: TEST_EXECUTION_EXECUTOR_PROMPT,
            model_level: ModelLevel::Low,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: TEST_EXECUTION_EVALUATOR_PROMPT,
            model_level: ModelLevel::Low,
        }),
    },
    PhaseDefinition {
        id: "code-review",
        description: "代码审查",
        steps: CODE_REVIEW_STEPS,
        artifact_manifest: &[],
        executor: None,
        evaluator: Some(PhaseAgentSpec {
            prompt: CODE_REVIEW_EVALUATOR_PROMPT,
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "acceptance",
        description: "验收评估",
        steps: ACCEPTANCE_STEPS,
        artifact_manifest: &[],
        executor: None,
        evaluator: Some(PhaseAgentSpec {
            prompt: ACCEPTANCE_EVALUATOR_PROMPT,
            model_level: ModelLevel::High,
        }),
    },
    // archive：确定性相位第二例（test-execution 为先例）——无 executor /
    // evaluator 会话、零 artifact_manifest；末尾 PhaseLog 以空 checklist 落账
    // 即「相位已 pass」标记（非评估证据面）。归档为固定停止点（stop-before-
    // archive），用户显式确认后才进入本相位。
    PhaseDefinition {
        id: "archive",
        description: "归档收口",
        steps: ARCHIVE_STEPS,
        artifact_manifest: &[],
        executor: None,
        evaluator: None,
    },
];

/// 按工作流类型取相位表；V1 仅 `requirement` 返回 `Some`，其余 `None`
///（W8 发起前置校验依据，显式拒绝优于相位机半途报错）。
pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]> {
    if !workflow_type
        .trim()
        .eq_ignore_ascii_case(REQUIREMENT_WORKFLOW_TYPE)
    {
        return None;
    }
    Some(REQUIREMENT_TABLE)
}

/// 当前相位在表中的全部前置相位（含自身；与插件 `computeAllowedBacktrackPhases`
/// 同语义）。相位不在表内 → 空集。
pub fn allowed_backtrack_phases(table: &[PhaseDefinition], current_phase: &str) -> Vec<String> {
    let Some(idx) = table.iter().position(|phase| phase.id == current_phase) else {
        return Vec::new();
    };
    if idx == 0 {
        return Vec::new();
    }
    table[..=idx]
        .iter()
        .map(|phase| phase.id.to_owned())
        .collect()
}

/// requirement 直接前置依赖表（stale 传播的单一事实源，与插件
/// `PHASE_PREREQUISITES` 一比一）。
fn prerequisites(phase: &str) -> &'static [&'static str] {
    match phase {
        "dev-design" => &["proposal"],
        "test-design" => &["proposal", "dev-design"],
        "test-gen" => &["test-design", "implement"],
        "implement" => &["dev-design"],
        "test-execution" => &["test-gen", "implement"],
        "code-review" => &["test-gen", "implement"],
        "acceptance" => &["proposal", "dev-design", "implement"],
        _ => &[],
    }
}

/// 依赖 `phase` 的全部下游相位（表序；与插件 `getDependents` 同源推导：
/// 表内前置含 `phase` 即为下游）。
pub(crate) fn dependents(table: &[PhaseDefinition], phase: &str) -> Vec<String> {
    table
        .iter()
        .filter(|candidate| prerequisites(candidate.id).contains(&phase))
        .map(|candidate| candidate.id.to_owned())
        .collect()
}
