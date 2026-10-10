//! requirement 工作流相位表单源：相位定义（executor / evaluator 的角色知识
//! prompt 与模型档位）、backtrack 白名单计算、依赖推导与重试上限。角色知识
//! prompt 单源自 `crates/core/workflow/src/prompts/` 下 14 个全静态 md，经
//! `include_str!` 显式字面路径编译期装配（无逐角色动态读取、无运行时文件 IO、
//! 零占位符）；动态面（上下文头 / 回溯原因）由编排层 append，本表零模板替换。

use agent::ModelLevel;

/// 相位角色的角色知识 prompt 与模型档位：prompt 为编译期静态文本
///（`include_str!` 显式字面路径装配产物），无 agent_type 字段、无占位符。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseAgentSpec {
    pub prompt: &'static str,
    pub model_level: ModelLevel,
}

/// 单相位定义（requirement 表静态项）。code-review / acceptance 为评估-only
/// 相位，executor 为 `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseDefinition {
    pub id: &'static str,
    pub description: &'static str,
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

/// requirement 相位表（编译期静态装配：14 个角色知识 md 逐条 `include_str!`
/// 显式字面路径；全静态零占位故全表 const 化，驻留 `'static` 只读共享）。
static REQUIREMENT_TABLE: &[PhaseDefinition] = &[
    PhaseDefinition {
        id: "proposal",
        description: "需求提案与规格说明",
        executor: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/proposal-planner.md"),
            model_level: ModelLevel::High,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/proposal-evaluator.md"),
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "dev-design",
        description: "详细设计与任务拆解",
        executor: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/dev-design-planner.md"),
            model_level: ModelLevel::High,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/dev-design-evaluator.md"),
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "test-design",
        description: "测试设计",
        executor: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/test-design-planner.md"),
            model_level: ModelLevel::High,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/test-design-evaluator.md"),
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "implement",
        description: "代码实现",
        executor: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/implementation-generator.md"),
            model_level: ModelLevel::Low,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/implementation-evaluator.md"),
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "test-gen",
        description: "测试代码生成",
        executor: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/test-gen-generator.md"),
            model_level: ModelLevel::Low,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/test-gen-evaluator.md"),
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "test-execution",
        description: "测试执行与诊断",
        executor: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/test-execution-executor.md"),
            model_level: ModelLevel::Low,
        }),
        evaluator: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/test-execution-evaluator.md"),
            model_level: ModelLevel::Low,
        }),
    },
    PhaseDefinition {
        id: "code-review",
        description: "代码审查",
        executor: None,
        evaluator: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/code-review-evaluator.md"),
            model_level: ModelLevel::High,
        }),
    },
    PhaseDefinition {
        id: "acceptance",
        description: "验收评估",
        executor: None,
        evaluator: Some(PhaseAgentSpec {
            prompt: include_str!("../prompts/acceptance-evaluator.md"),
            model_level: ModelLevel::High,
        }),
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
