use agent::ModelLevel;

use crate::state::{ArtifactSpec, OnFail, StepDefinition, StepNode};

/// 相位角色的角色知识 prompt 与模型档位：prompt 为编译期静态文本
///（`include_str!` 显式字面路径装配产物），无 agent_type 字段、无占位符。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseAgentSpec {
    pub prompt: &'static str,
    pub model_level: ModelLevel,
}

/// 单相位定义（相位表静态项）：计划图节点（有序 step 序列 + 产物清单）、角色
/// prompt spec 与直接前置依赖的单源载体。code-review / acceptance 为评估-only
/// 相位，executor 为 `None`（其 step 序列不含 Executor 步）；archive 为确定性
/// 相位，executor / evaluator 均 `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseDefinition {
    pub id: &'static str,
    pub description: &'static str,
    /// 相位内有序 step 序列（计划图；walker 迭代本序列执行，MUST NOT 自持步序）
    pub steps: &'static [StepDefinition],
    /// 相位声明产物清单（相对路径 → phase 归属权威来源）
    pub artifact_manifest: &'static [ArtifactSpec],
    /// 相位直接前置依赖（表内自描述；stale 传播与下游推导的唯一事实源，
    /// MUST NOT 存在第二份独立 match）
    pub prerequisites: &'static [&'static str],
    /// 相位回溯资格（表内自描述，沿 `prerequisites` 先例；`allowed_backtrack_phases`
    /// 的唯一过滤依据——`false` 者不得出现在任何相位的回溯白名单；表头
    /// initialize 恒 `false`——基线检查不可回溯重做，过滤 MUST NOT 以「跳过
    /// idx==0」类位置推导实现——表结构变化即漂移）
    pub backtrackable: bool,
    pub executor: Option<PhaseAgentSpec>,
    pub evaluator: Option<PhaseAgentSpec>,
}

/// 重试上限。
pub const MAX_RETRY_TIMES: u32 = 5;

/// 落账 report 的长度上限（与插件 `phaseLogSchema.report` max 2000 同源）。
pub(crate) const MAX_REPORT_CHARS: usize = 2000;

/// 回溯 reason 的长度上限（桌面写面自有裁定 1000；决策 prompt 的提示文案
/// 仍按 500 引导，见 orchestration `decision::MAX_REASON_CHARS`，不受本常量
/// 影响；插件 `backtrackInputSchema.backtrack_reason` 的 max 500 亦不受影响）。
pub(crate) const MAX_REASON_CHARS: usize = 1000;

/// requirement 工作流类型键（相位表键之一；其余类型在写面 / 发起前置校验显式
/// 拒绝）。
const REQUIREMENT_WORKFLOW_TYPE: &str = "requirement";

/// test-only 工作流类型键（相位表键之二）。
const TEST_ONLY_WORKFLOW_TYPE: &str = "test-only";

/// initialize 相位名单点（两表表头一次性基线检查相位；walker legacy 短路判定
/// 消费——经 `write/mod.rs` 再导出）。
pub const INITIALIZE_PHASE_ID: &str = "initialize";

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
const CODE_ANALYZE_EXECUTOR_PROMPT: &str = include_str!("../prompts/code-analyze-planner.md");
const CODE_ANALYZE_EVALUATOR_PROMPT: &str = include_str!("../prompts/code-analyze-evaluator.md");

const INITIALIZE_DESCRIPTION: &str = "检查变更环境和基线代码";
const PROPOSAL_DESCRIPTION: &str = "需求提案与规格说明";
const DEV_DESIGN_DESCRIPTION: &str = "详细设计与任务拆解";
const TEST_DESIGN_DESCRIPTION: &str = "测试设计";
const IMPLEMENT_DESCRIPTION: &str = "生成功能代码";
const TEST_GEN_DESCRIPTION: &str = "生成测试代码";
const TEST_EXECUTION_DESCRIPTION: &str = "执行自动化测试，收集数据与诊断";
const CODE_REVIEW_DESCRIPTION: &str = "代码审查";
const ACCEPTANCE_DESCRIPTION: &str = "验收评估";
const ARCHIVE_DESCRIPTION: &str = "代码回合基线，设计归档";
const CODE_ANALYZE_DESCRIPTION: &str = "分析代码生成详细设计";

// --- 相位计划步序列（编译期 const 有序序列；kind / role 由 `StepNode` 派生，
// 门禁步由 step 声明而非 walker 常量注入）---------------------------------

/// initialize 相位步序列（两表表头共用同一 static；一次性基线检查相位——
/// `PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`：Clean 为 vcs
/// 直调基线恢复步，两检查步 `on_fail=Terminal`（失败即停——无反馈边、零 agent
/// 会话注入，基线的债不属于本次变更），PhaseLog 机械 checklist 纯桌面代写）。
static INITIALIZE_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Clean,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::StaticCheck,
        prompt_ref: None,
        on_fail: Some(OnFail::Terminal),
    },
    StepDefinition {
        node: StepNode::TestExecution,
        prompt_ref: None,
        on_fail: Some(OnFail::Terminal),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

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

/// code-analyze 相位步序列（P→E，与 proposal / dev-design 同形的分析设计相位：
/// PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog；VerdictGate
/// `on_fail=Terminal`）。
static CODE_ANALYZE_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Executor,
        prompt_ref: Some(CODE_ANALYZE_EXECUTOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::Evaluator,
        prompt_ref: Some(CODE_ANALYZE_EVALUATOR_PROMPT),
        on_fail: None,
    },
    StepDefinition {
        node: StepNode::VerdictGate,
        prompt_ref: None,
        on_fail: Some(OnFail::Terminal),
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
/// Evaluator / VerdictGate 步），pass 后 PhaseLog 步携机械 checklist 落账。拆步
/// 语义：`PhaseStart → TestExecution → MutationTest → PhaseLog`——测试 / 覆盖率
/// 修绿后才跑突变（walker 顺序执行天然保证），两步失败即升格交决策（零修复
/// 会话）；requirement / test-only 两表共用同一 static 天然同式。
static TEST_EXECUTION_STEPS: &[StepDefinition] = &[
    StepDefinition {
        node: StepNode::PhaseStart,
        prompt_ref: None,
        on_fail: None,
    },
    // 确定性门禁步：首败即升格交决策，零 agent 修复
    StepDefinition {
        node: StepNode::TestExecution,
        prompt_ref: None,
        on_fail: Some(OnFail::Escalate),
    },
    // 确定性门禁步：首败即升格交决策，零 agent 修复
    StepDefinition {
        node: StepNode::MutationTest,
        prompt_ref: None,
        on_fail: Some(OnFail::Escalate),
    },
    StepDefinition {
        node: StepNode::PhaseLog,
        prompt_ref: None,
        on_fail: None,
    },
];

/// archive 相位步序列（11 步；确定性相位第二例；Cleanup 为尽力步——失败仅
/// step detail 警告、步恒 passed，尽力语义驻 walker 执行体，`on_fail = None`
/// 形态与其余步一致）
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
        node: StepNode::Cleanup,
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
        title: "开发设计",
    },
    ArtifactSpec {
        kind: "markdown-doc",
        path: "tasks.md",
        title: "任务拆分",
    },
];

static CODE_ANALYZE_MANIFEST: &[ArtifactSpec] = &[ArtifactSpec {
    kind: "markdown-doc",
    path: "design.md",
    title: "源码分析",
}];

static TEST_DESIGN_MANIFEST: &[ArtifactSpec] = &[ArtifactSpec {
    kind: "markdown-doc",
    path: "test-design.md",
    title: "测试设计",
}];

static IMPLEMENT_MANIFEST: &[ArtifactSpec] = &[ArtifactSpec {
    kind: "tasks-progress",
    path: "tasks.md",
    title: "任务进度",
}];

/// test-execution 相位产物清单（测试报告树）。
static TEST_EXECUTION_MANIFEST: &[ArtifactSpec] = &[ArtifactSpec {
    kind: "markdown-doc",
    path: "test-reports/**",
    title: "测试报告",
}];

/// requirement 相位表
static REQUIREMENT_TABLE: &[PhaseDefinition] = &[
    // initialize：两表表头一次性基线检查相位（确定性相位——executor /
    // evaluator 双缺席、零 artifact_manifest；`INITIALIZE_STEPS` 五步两表共享；
    // backtrackable=false——基线检查不可回溯重做）。
    PhaseDefinition {
        id: INITIALIZE_PHASE_ID,
        description: INITIALIZE_DESCRIPTION,
        steps: INITIALIZE_STEPS,
        artifact_manifest: &[],
        prerequisites: &[],
        backtrackable: false,
        executor: None,
        evaluator: None,
    },
    PhaseDefinition {
        id: "proposal",
        description: PROPOSAL_DESCRIPTION,
        steps: PROPOSAL_STEPS,
        artifact_manifest: PROPOSAL_MANIFEST,
        prerequisites: &[],
        backtrackable: true,
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
        description: DEV_DESIGN_DESCRIPTION,
        steps: DEV_DESIGN_STEPS,
        artifact_manifest: DEV_DESIGN_MANIFEST,
        prerequisites: &["proposal"],
        backtrackable: true,
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
        description: TEST_DESIGN_DESCRIPTION,
        steps: TEST_DESIGN_STEPS,
        artifact_manifest: TEST_DESIGN_MANIFEST,
        prerequisites: &["proposal", "dev-design"],
        backtrackable: true,
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
        description: IMPLEMENT_DESCRIPTION,
        steps: IMPLEMENT_STEPS,
        artifact_manifest: IMPLEMENT_MANIFEST,
        prerequisites: &["dev-design"],
        backtrackable: true,
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
        description: TEST_GEN_DESCRIPTION,
        steps: TEST_GEN_STEPS,
        artifact_manifest: &[],
        prerequisites: &["test-design", "implement"],
        backtrackable: true,
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
        description: TEST_EXECUTION_DESCRIPTION,
        steps: TEST_EXECUTION_STEPS,
        artifact_manifest: TEST_EXECUTION_MANIFEST,
        prerequisites: &["test-gen", "implement"],
        backtrackable: true,
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
        description: CODE_REVIEW_DESCRIPTION,
        steps: CODE_REVIEW_STEPS,
        artifact_manifest: &[],
        prerequisites: &["test-gen", "implement"],
        backtrackable: true,
        executor: None,
        evaluator: Some(PhaseAgentSpec {
            prompt: CODE_REVIEW_EVALUATOR_PROMPT,
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "acceptance",
        description: ACCEPTANCE_DESCRIPTION,
        steps: ACCEPTANCE_STEPS,
        artifact_manifest: &[],
        prerequisites: &["proposal", "dev-design", "implement"],
        backtrackable: true,
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
        description: ARCHIVE_DESCRIPTION,
        steps: ARCHIVE_STEPS,
        artifact_manifest: &[],
        prerequisites: &[],
        backtrackable: true,
        executor: None,
        evaluator: None,
    },
];

/// test-only 相位表
static TEST_ONLY_TABLE: &[PhaseDefinition] = &[
    // initialize：两表表头一次性基线检查相位（与 requirement 表头同表项形态，
    // `INITIALIZE_STEPS` 两表共享同一 static）。
    PhaseDefinition {
        id: INITIALIZE_PHASE_ID,
        description: INITIALIZE_DESCRIPTION,
        steps: INITIALIZE_STEPS,
        artifact_manifest: &[],
        prerequisites: &[],
        backtrackable: false,
        executor: None,
        evaluator: None,
    },
    PhaseDefinition {
        id: "proposal",
        description: PROPOSAL_DESCRIPTION,
        steps: PROPOSAL_STEPS,
        artifact_manifest: PROPOSAL_MANIFEST,
        prerequisites: &[],
        backtrackable: true,
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
        id: "code-analyze",
        description: CODE_ANALYZE_DESCRIPTION,
        steps: CODE_ANALYZE_STEPS,
        artifact_manifest: CODE_ANALYZE_MANIFEST,
        prerequisites: &["proposal"],
        backtrackable: true,
        executor: Some(PhaseAgentSpec {
            prompt: CODE_ANALYZE_EXECUTOR_PROMPT,
            model_level: ModelLevel::High,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: CODE_ANALYZE_EVALUATOR_PROMPT,
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "test-design",
        description: TEST_DESIGN_DESCRIPTION,
        steps: TEST_DESIGN_STEPS,
        artifact_manifest: TEST_DESIGN_MANIFEST,
        prerequisites: &["proposal", "code-analyze"],
        backtrackable: true,
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
        id: "test-gen",
        description: TEST_GEN_DESCRIPTION,
        steps: TEST_GEN_STEPS,
        artifact_manifest: &[],
        prerequisites: &["test-design"],
        backtrackable: true,
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
        description: TEST_EXECUTION_DESCRIPTION,
        steps: TEST_EXECUTION_STEPS,
        artifact_manifest: TEST_EXECUTION_MANIFEST,
        prerequisites: &["test-gen"],
        backtrackable: true,
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
        id: "archive",
        description: ARCHIVE_DESCRIPTION,
        steps: ARCHIVE_STEPS,
        artifact_manifest: &[],
        prerequisites: &[],
        backtrackable: true,
        executor: None,
        evaluator: None,
    },
];

/// 按工作流类型取相位表：`requirement` → `REQUIREMENT_TABLE`（10 相位，initialize
/// 表头）、`test-only` → `TEST_ONLY_TABLE`（7 相位，initialize 表头），其余（含
/// 空白 / 未知 / bug-fix）
/// → `None`（不退化——写面 / 发起前置校验依据，显式拒绝优于相位机半途报错）。
pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]> {
    let workflow_type = workflow_type.trim();
    if workflow_type.eq_ignore_ascii_case(REQUIREMENT_WORKFLOW_TYPE) {
        return Some(REQUIREMENT_TABLE);
    }
    if workflow_type.eq_ignore_ascii_case(TEST_ONLY_WORKFLOW_TYPE) {
        return Some(TEST_ONLY_TABLE);
    }
    None
}

/// 回溯白名单项：相位 id + 表内自描述的特性文案（决策 agent 判断回溯目标的
/// 上下文；描述单一来源 = `PhaseDefinition.description`，调用方零第二份文案
/// 映射）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacktrackOption {
    pub phase: String,
    pub description: String,
}

/// 当前相位在表中的全部前置相位项
pub fn backtrack_options(table: &[PhaseDefinition], current_phase: &str) -> Vec<BacktrackOption> {
    let Some(idx) = table.iter().position(|phase| phase.id == current_phase) else {
        return Vec::new();
    };
    if idx == 0 {
        return Vec::new();
    }
    let redoable = table[idx]
        .steps
        .iter()
        .any(|step| step.node.role().is_some());
    table[..=idx]
        .iter()
        .filter(|phase| phase.backtrackable)
        .filter(|phase| redoable || phase.id != current_phase)
        .map(|phase| BacktrackOption {
            phase: phase.id.to_owned(),
            description: phase.description.to_owned(),
        })
        .collect()
}

/// 当前相位回溯白名单的相位 id 投影（既有 id-only 消费面；过滤语义单源 =
/// [`backtrack_options`]）。
pub fn allowed_backtrack_phases(table: &[PhaseDefinition], current_phase: &str) -> Vec<String> {
    backtrack_options(table, current_phase)
        .into_iter()
        .map(|option| option.phase)
        .collect()
}

/// 依赖 `phase` 的全部下游相位（表序；与插件 `getDependents` 同源推导：
/// 表内前置含 `phase` 即为下游——前置依赖表内自描述，无第二份独立 match，
/// 类型感知随表分派天然成立）。
pub(crate) fn dependents(table: &[PhaseDefinition], phase: &str) -> Vec<String> {
    table
        .iter()
        .filter(|candidate| candidate.prerequisites.contains(&phase))
        .map(|candidate| candidate.id.to_owned())
        .collect()
}
