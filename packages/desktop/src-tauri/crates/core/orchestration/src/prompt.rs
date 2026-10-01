//! prompt 组装：executor 角色要点前导（桌面内置静态表，剥离
//! `__CALL_AGENT:<role>__` 取角色名——不指示会话 Read 插件 `agents/<role>.md`，
//! 不依赖安装路径）+ evaluator 输出协议附录（禁调 MCP phase-log / 最终消息
//! 输出 checklist JSON）+ 决策 prompt（有界输入组装）。executor / evaluator
//! 双入口均组装 git diff 变更文件上下文段（AC-3：desktop run 的 file_log 零
//! 新增，变更文件上下文降级 git diff；change / phase 已由写面插值进 phase
//! prompt，协议附录不再携带 change / phase 参数）。

use crate::decision::DecisionInput;
use crate::verdict::MAX_REPORT_CHARS;

/// 剥离 `__CALL_AGENT:<role>__` 令牌取角色名；裸 agent_type 原样返回。
pub(crate) fn strip_call_agent(agent_type: &str) -> &str {
    let token = agent_type.trim();
    if let Some(rest) = token.strip_prefix("__CALL_AGENT:") {
        if let Some(role) = rest.strip_suffix("__") {
            return role;
        }
    }
    token
}

/// 角色要点静态表（桌面内置；插件 agents/<role>.md 章旨的浓缩镜像）。
fn role_brief(role: &str) -> &'static str {
    match role {
        "proposal-planner" => {
            "读项目上下文，产出 proposal.md（问题 / 提案 / 验收标准 / 风险）与 specs 能力基线增量"
        }
        "proposal-evaluator" => "以静态二项清单评估 proposal 的完整性与可验收性，不臆测未落盘内容",
        "dev-design-planner" => {
            "基于定稿 proposal 产出 design.md（架构组件 / 变更清单 / 数据模型 / 决策留痕）"
        }
        "dev-design-evaluator" => "以静态清单评估 design 对 proposal 的对齐与决策质量",
        "test-design-planner" => {
            "从 design 推导单元测试范围与公共 API 签名，产出 test-design.md（不写测试命令）"
        }
        "test-design-evaluator" => "以静态清单评估 test-design 的覆盖度与可执行性",
        "implementation-generator" => {
            "按 design.md / tasks.md 直接写实现代码到盘，遵循既有代码风格"
        }
        "implementation-evaluator" => "以静态清单评估实现与 design 变更清单的一致性",
        "test-gen-generator" => "按 test-design.md 写测试文件，只测自研层不测库语义",
        "test-gen-evaluator" => "以静态清单评估测试代码对 test-design 的符合度",
        "test-execution-executor" => "执行自动化测试并产出执行报告，修复阻断性执行错误",
        "test-execution-evaluator" => "校验测试执行报告完整性，应用诊断决策树给出 verdict 与根因",
        "code-review-evaluator" => "以安全 / 测试覆盖 / 错误处理静态清单评审代码",
        "acceptance-evaluator" => "按 proposal 验收标准以静态二项清单评估代码库",
        "code-analyze-planner" => "逆向现有代码架构，产出 test-only 工作流的 design.md",
        "code-analyze-evaluator" => "以静态清单评估逆向 design 与架构事实的一致性",
        _ => "完成相位 prompt 指定的工作；结论以事实为据，不臆测未验证内容",
    }
}

/// git diff 变更文件上下文段（executor / evaluator 双入口共用；空上下文 =
/// 工作区无未提交变更，留段声明而非省略——上下文面缺失显式可读）。
fn diff_section(diff_context: &str) -> String {
    let body = if diff_context.trim().is_empty() {
        "（当前工作区无未提交变更）".to_owned()
    } else {
        diff_context.trim().to_owned()
    };
    format!("\n\n---\n\n## 变更文件上下文（git diff，工作区当前状态快照）\n\n{body}")
}

/// executor prompt：内置角色要点前导 + 写面已插值 phase prompt + git diff
/// 变更文件上下文段。角色名自 `__CALL_AGENT:<role>__` 剥离；未收录角色落
/// 通用要点兜底。
pub fn executor_prompt(agent_type: &str, phase_prompt: &str, diff_context: &str) -> String {
    let role = strip_call_agent(agent_type);
    format!(
        "你以角色「{role}」执行本次相位工作。角色要点：{}\n\n---\n\n{phase_prompt}{}",
        role_brief(role),
        diff_section(diff_context)
    )
}

/// evaluator 输出协议附录：写通道唯一红线（禁调 MCP phase-log / phase-next /
/// phase_start / backtrack，落账由桌面 walker 代写）+ 最终消息 checklist JSON
/// 形状与 attempt 信封约定（phase / attempt 以 prompt 标注的相位工作为准）。
fn evaluator_protocol() -> String {
    format!(
        "\n\n---\n\n## 输出协议（必须遵守）\n\n\
1. 禁止调用 MCP 工具 phase_log / phase_next / phase_start / backtrack：评估落账由桌面编排代写，你只产出评估结论。\n\
2. 评估完成后，最终消息必须输出且仅输出一个 checklist JSON 对象（裸 JSON 或 ```json 围栏代码块均可），形状：\n\n\
{{\n  \"phase\": \"<评估的相位 id>\",\n  \"attempt\": <本相位 attempt 号>,\n  \"verdict\": \"pass\" | \"fail\",\n  \"report\": \"评估报告（不超过 {MAX_REPORT_CHARS} 字符）\",\n  \"checklist\": [{{\"item\": \"检查项名称\", \"pass\": true, \"evidence\": \"检查依据\"}}],\n  \"skipped\": false\n}}\n\n\
3. phase / attempt 以你实际评估的相位工作为准；checklist 至少一项，pass 为布尔值，evidence 必须给出事实依据。"
    )
}

/// evaluator prompt：写面已插值 phase prompt + 输出协议附录 + git diff 变更
/// 文件上下文段。
pub fn evaluator_prompt(phase_prompt: &str, diff_context: &str) -> String {
    format!(
        "{phase_prompt}{}{}",
        evaluator_protocol(),
        diff_section(diff_context)
    )
}

/// 决策 prompt：有界输入组装（fail checklist + 白名单 + 候选 eval report）
/// + 四动作封闭集说明。输出协议同 evaluator——最终消息仅输出一个决策 JSON。
pub fn decision_prompt(input: &DecisionInput) -> String {
    let fail_items = input
        .fail_checklist
        .iter()
        .map(|item| format!("- {}：{}", item.item, item.evidence))
        .collect::<Vec<_>>()
        .join("\n");
    let allowed = input
        .allowed
        .iter()
        .map(|phase| format!("- {phase}"))
        .collect::<Vec<_>>()
        .join("\n");
    let candidates = input
        .candidates
        .iter()
        .map(|candidate| {
            format!(
                "- {}（verdict: {}）：{}",
                candidate.phase,
                candidate
                    .verdict
                    .map(|verdict| verdict.as_str().to_owned())
                    .unwrap_or_else(|| "无记录".to_owned()),
                candidate.report.as_deref().unwrap_or("（无 eval report）")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "相位「{phase}」第 {attempt} 次 attempt 评估失败，需要你做出路由决策。\n\n\
## 失败清单\n\n{fail_items}\n\n\
## 可回溯白名单（仅可从中选择 backtrack 目标）\n\n{allowed}\n\n\
## 候选相位最近一次评估报告\n\n{candidates}\n\n\
## 决策协议（最终消息必须输出且仅输出一个 JSON 对象，四选一）\n\n\
- {{\"action\": \"backtrack\", \"to\": \"<白名单内相位 id>\", \"reason\": \"<≤{MAX_REPORT_CHARS} 字符>\"}}：白名单内自主回溯，直接执行。\n\
- {{\"action\": \"retry\"}}：原相位重试。\n\
- {{\"action\": \"stop\", \"reason\": \"<≤{MAX_REPORT_CHARS} 字符>\"}}：终止本次运行。\n\
- {{\"action\": \"ask\", \"question\": \"<问题>\", \"options\": [\"<选项>\"]}}：无法裁决时中断提问。\n\n\
红线：backtrack 目标不在白名单内的决议会被拒绝；reason 超长会被拒绝。",
        phase = input.phase,
        attempt = input.attempt,
    )
}
