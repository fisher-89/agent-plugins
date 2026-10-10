use crate::decision::{DecisionInput, MAX_REASON_CHARS};
use crate::verdict::MAX_REPORT_CHARS;

/// evaluator 输出协议附录：写通道唯一红线（禁调 MCP phase-log / phase-next /
/// phase_start / backtrack，落账由桌面 walker 代写）+ 最终消息 checklist JSON
/// 形状（phase / attempt / skipped 由 walker 按 provenance 盖戳，evaluator 不回声）。
fn evaluator_protocol() -> String {
    format!(
        "\n\n---\n\n## 输出协议（必须遵守）\n\n\
1. 禁止调用 MCP 工具 phase_log / phase_next / phase_start / backtrack：评估落账由桌面编排代写，你只产出评估结论。\n\
2. 评估完成后，最终消息必须输出且仅输出一个 checklist JSON 对象（裸 JSON 或 ```json 围栏代码块均可），形状：\n\n\
{{\n  \"verdict\": \"pass\" | \"fail\",\n  \"report\": \"评估报告（不超过 {MAX_REPORT_CHARS} 字符）\",\n  \"checklist\": [{{\"item\": \"检查项名称\", \"pass\": true, \"evidence\": \"检查依据\"}}]\n}}\n\n\
3. checklist 至少一项，pass 为布尔值，evidence 必须给出事实依据；不要输出 phase / attempt / skipped 字段（由桌面编排按 provenance 盖戳）。"
    )
}

/// evaluator prompt：静态角色知识主体 + 输出协议附录（唯一动态 append）。
pub fn evaluator_prompt(phase_prompt: &str) -> String {
    format!("{phase_prompt}{}", evaluator_protocol())
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
- {{\"action\": \"backtrack\", \"to\": \"<白名单内相位 id>\", \"reason\": \"<≤{MAX_REASON_CHARS} 字符>\"}}：白名单内自主回溯，直接执行。\n\
- {{\"action\": \"retry\"}}：原相位重试。\n\
- {{\"action\": \"stop\", \"reason\": \"<≤{MAX_REASON_CHARS} 字符>\"}}：终止本次运行。\n\
- {{\"action\": \"ask\", \"question\": \"<问题>\", \"options\": [\"<选项>\"]}}：无法裁决时中断提问。\n\n\
红线：backtrack 目标不在白名单内的决议会被拒绝；reason 超长会被拒绝。",
        phase = input.phase,
        attempt = input.attempt,
    )
}
