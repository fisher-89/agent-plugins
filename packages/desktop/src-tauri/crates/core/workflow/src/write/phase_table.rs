//! requirement 工作流相位表单源：相位定义（executor / evaluator 的 agent
//! 引用与 prompt 模板）、`<change>` / `<phase>` 占位符插值、backtrack 白名单
//! 计算、依赖推导与重试上限。与插件 `lib/workflow.ts` 的 requirement 表一比
//! 一对应（AC-9 对照口径）；prompt 模板单源自此，编排层不另备相位词汇。

use std::sync::OnceLock;

use foundation::layout::domain_dir_name;

/// 相位角色的 agent 引用与 prompt 模板（`__CALL_AGENT:<role>__` 约定原样
/// 保留，剥离归 prompt 组装）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseAgentSpec {
    pub agent_type: String,
    pub prompt: String,
}

/// 单相位定义（requirement 表静态项）。code-review / acceptance 为评估-only
/// 相位，executor 为 `None`（与插件表一致）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseDefinition {
    pub id: &'static str,
    pub description: &'static str,
    pub executor: Option<PhaseAgentSpec>,
    pub evaluator: Option<PhaseAgentSpec>,
}

/// 重试上限（与插件同名常量一致）。
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

/// explore→proposal 交接行（文件式交接）。域根目录名经 layout 单点取用，
/// 产品源码不落磁盘目录名字面量（layout 命名隔离双禁令）。
fn proposal_explore_handoff() -> String {
    format!(
        "If {domain}/changes/<change>/explore.md exists, Read it as free-form explore context. \
         If proposal.md already exists, Read it and merge new explore insights (do not rewrite \
         from scratch). Do not expect inline EXPLORE_CONTEXT_SUMMARY.",
        domain = domain_dir_name()
    )
}

/// requirement 相位表（prompt 模板静态构建；explore 交接行含 layout 单点
/// 目录名，故经 `OnceLock` 首见初始化后驻留 `'static`）。
fn requirement_table() -> Vec<PhaseDefinition> {
    let explore_handoff = proposal_explore_handoff();
    let spec = |agent_type: &str, prompt: String| PhaseAgentSpec {
        agent_type: agent_type.to_owned(),
        prompt,
    };
    vec![
        PhaseDefinition {
            id: "proposal",
            description: "需求提案与规格说明",
            executor: Some(spec(
                "__CALL_AGENT:proposal-planner__",
                format!("Write or update proposal.md and specs/ for change \"<change>\". {explore_handoff}"),
            )),
            evaluator: Some(spec(
                "__CALL_AGENT:proposal-evaluator__",
                "Evaluate <phase> phase for change \"<change>\". Call phase_log with phase=\"<phase>\"."
                    .to_owned(),
            )),
        },
        PhaseDefinition {
            id: "dev-design",
            description: "详细设计与任务拆解",
            executor: Some(spec(
                "__CALL_AGENT:dev-design-planner__",
                "Write design.md and tasks.md for change \"<change>\".".to_owned(),
            )),
            evaluator: Some(spec(
                "__CALL_AGENT:dev-design-evaluator__",
                "Evaluate <phase> phase: design.md and tasks.md for change \"<change>\" against \
                 proposal.md. Call phase_log with phase=\"<phase>\"."
                    .to_owned(),
            )),
        },
        PhaseDefinition {
            id: "test-design",
            description: "测试设计",
            executor: Some(spec(
                "__CALL_AGENT:test-design-planner__",
                "Write test design for change \"<change>\".".to_owned(),
            )),
            evaluator: Some(spec(
                "__CALL_AGENT:test-design-evaluator__",
                "Evaluate <phase> phase: test design for change \"<change>\" against design.md. \
                 Call phase_log with phase=\"<phase>\"."
                    .to_owned(),
            )),
        },
        PhaseDefinition {
            id: "implement",
            description: "代码实现",
            executor: Some(spec(
                "__CALL_AGENT:implementation-generator__",
                "Implement the code for change \"<change>\".".to_owned(),
            )),
            evaluator: Some(spec(
                "__CALL_AGENT:implementation-evaluator__",
                "Evaluate <phase> phase: implementation for change \"<change>\" against design. \
                 Call phase_log with phase=\"<phase>\"."
                    .to_owned(),
            )),
        },
        PhaseDefinition {
            id: "test-gen",
            description: "测试代码生成",
            executor: Some(spec(
                "__CALL_AGENT:test-gen-generator__",
                "Generate test code for change \"<change>\".".to_owned(),
            )),
            evaluator: Some(spec(
                "__CALL_AGENT:test-gen-evaluator__",
                "Evaluate <phase> phase: generated tests for change \"<change>\". Call phase_log \
                 with phase=\"<phase>\"."
                    .to_owned(),
            )),
        },
        PhaseDefinition {
            id: "test-execution",
            description: "测试执行与诊断",
            executor: Some(spec(
                "__CALL_AGENT:test-execution-executor__",
                "Run and fix all tests for change \"<change>\".".to_owned(),
            )),
            evaluator: Some(spec(
                "__CALL_AGENT:test-execution-evaluator__",
                "Evaluate <phase> phase: test execution results for change \"<change>\". Call \
                 phase_log with phase=\"<phase>\"."
                    .to_owned(),
            )),
        },
        PhaseDefinition {
            id: "code-review",
            description: "代码审查",
            executor: None,
            evaluator: Some(spec(
                "__CALL_AGENT:code-review-evaluator__",
                "Evaluate <phase> phase: code review for change \"<change>\". Call phase_log with \
                 phase=\"<phase>\"."
                    .to_owned(),
            )),
        },
        PhaseDefinition {
            id: "acceptance",
            description: "验收评估",
            executor: None,
            evaluator: Some(spec(
                "__CALL_AGENT:acceptance-evaluator__",
                "Evaluate <phase> phase: acceptance for change \"<change>\". Call phase_log with \
                 phase=\"<phase>\"."
                    .to_owned(),
            )),
        },
    ]
}

/// 相位表驻留位（首见初始化后全程 `'static`）。
static REQUIREMENT_TABLE: OnceLock<Vec<PhaseDefinition>> = OnceLock::new();

/// 按工作流类型取相位表；V1 仅 `requirement` 返回 `Some`，其余 `None`
///（W8 发起前置校验依据，显式拒绝优于相位机半途报错）。
pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]> {
    if !workflow_type
        .trim()
        .eq_ignore_ascii_case(REQUIREMENT_WORKFLOW_TYPE)
    {
        return None;
    }
    Some(REQUIREMENT_TABLE.get_or_init(requirement_table))
}

/// `<change>` / `<phase>` 占位符插值（与插件 `interpolatePrompt` 语义一致：
/// `<change>` 恒替换，`<phase>` 仅在相位在位时替换）。
pub fn interpolate(template: &str, change: &str, phase: Option<&str>) -> String {
    let mut result = template.replace("<change>", change);
    if let Some(phase) = phase {
        result = result.replace("<phase>", phase);
    }
    result
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
