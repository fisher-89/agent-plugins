use workflow::write::BacktrackOption;

use crate::decision::{DecisionInput, MAX_REASON_CHARS};
use crate::verdict::MAX_REPORT_CHARS;

/// 白名单项单行标签：`<相位 id>（<特性描述>）`；描述空串退化为 id-only
/// （不渲染空括号）。决策 prompt 与纠正文案共用本单点，描述格式零副本。
fn backtrack_option_label(option: &BacktrackOption) -> String {
    if option.description.trim().is_empty() {
        option.phase.clone()
    } else {
        format!("{}（{}）", option.phase, option.description)
    }
}

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

/// verdict 解析失败反馈修复 prompt：解析错误 + 输出协议重申（复用
/// `evaluator_protocol`——形状文本单源，不落第三份副本）。续注同一 evaluator
/// 会话（Continue）：保留评估结论，仅修正最终消息格式后重新收口。
pub(crate) fn verdict_fix_prompt(error: &str, feedback: u32, limit: u32) -> String {
    format!(
        "评估输出解析失败（第 {feedback}/{limit} 次反馈修复）：{error}\n\n\
         请保留你上一轮的评估结论，仅修正最终消息的输出格式后重新收口。{}",
        evaluator_protocol()
    )
}

/// 决策 prompt：有界输入组装（失败报告 + fail checklist + 白名单 + 候选 eval
/// report）+ 三动作封闭集说明。输出协议同 evaluator——最终消息仅输出一个决策
/// JSON。
pub fn decision_prompt(input: &DecisionInput) -> String {
    let fail_items = input
        .fail_checklist
        .iter()
        .map(|item| format!("- {}：{}", item.item, item.evidence))
        .collect::<Vec<_>>()
        .join("\n");
    let fail_report = if input.fail_report.trim().is_empty() {
        "（无报告文本）".to_owned()
    } else {
        input.fail_report.clone()
    };
    // 白名单逐条渲染「相位 id（表内特性描述）」：描述供决策 agent 判断回溯
    // 目标；自注记：`allowed` 含失败相位自身 ⟺ 该相位可重做（零 agent 步相位
    // 自排除），重做本相位由白名单自含承载——零新字段
    let allowed = input
        .allowed
        .iter()
        .map(|option| {
            let redo_note = if option.phase == input.phase {
                "（= 重做本相位）"
            } else {
                ""
            };
            format!("- {}{redo_note}", backtrack_option_label(option))
        })
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
## 失败相位报告\n\n{fail_report}\n\n\
## 失败清单\n\n{fail_items}\n\n\
## 可回溯白名单（仅可从中选择 backtrack 目标）\n\n{allowed}\n\n\
## 候选相位最近一次评估报告\n\n{candidates}\n\n\
## 报告文件\n\n\
你可使用 `read` 工具读取报告树取全量明细：test-execution 的 `reports/test/summary.json`、突变的段级 / 归并报告（路径见失败明细）。\
决策输入有界——不得要求内联全量 findings / 存活突变体清单。\n\n\
## 决策协议（最终消息必须输出且仅输出一个 JSON 对象，三选一）\n\n\
- {{\"action\": \"backtrack\", \"to\": \"<白名单内相位 id>\", \"reason\": \"<≤{MAX_REASON_CHARS} 字符>\"}}：白名单内自主回溯，直接执行；目标为失败相位自身即重做本相位（仅当其在白名单内）。\n\
- {{\"action\": \"stop\", \"reason\": \"<≤{MAX_REASON_CHARS} 字符>\"}}：终止本次运行。\n\
- {{\"action\": \"ask\", \"question\": \"<问题>\", \"options\": [\"<选项>\"]}}：无法裁决时中断提问。\n\n\
红线：backtrack 目标不在白名单内的决议会被拒绝；reason 超长会被拒绝。",
        phase = input.phase,
        attempt = input.attempt,
    )
}

/// 决策软拒环纠正文案（每轮驳回单点）：驳回原因 + 三动作重申 + 白名单一行。
/// 不含全量失败明细——纠正面只重申协议与可选目标（续注同一决策会话）。
pub(crate) fn decision_fix_prompt(input: &DecisionInput, error: &str) -> String {
    format!(
        "上一轮决策输出被驳回：{error}\n\n\
         请仅输出一个决策 JSON（三动作封闭集：backtrack / stop / ask）。\n\n\
         可回溯白名单：{allowed}\n\n\
         回跳目标须在白名单内；无法裁决时用 ask。",
        allowed = input
            .allowed
            .iter()
            .map(backtrack_option_label)
            .collect::<Vec<_>>()
            .join(" / ")
    )
}

// ---------------------------------------------------------------------------
// 归档 agent prompt 单点（自 archive_flow.rs 搬移，文本零语义改动——增量合并语义
// 与解算纪律单源 = desktop-change-archive spec requirement 自述，零 CLI skill
// 溯源；零 MCP / __TOOL_ASK_USER__ / workflow.json 依赖；git 面收缩由模板明令
// ——归档编排代执行收口）
// ---------------------------------------------------------------------------

/// spec 同步 prompt（入参为记录供给的 change **name**——prompt 内文面向用户 /
/// 磁盘目录，恒 name 化）：增量合并语义自持（ADDED / MODIFIED / REMOVED /
/// RENAMED、保留未提及内容、幂等、capability 缺席创建）。
pub(crate) fn spec_sync_prompt(change: &str) -> String {
    format!(
        "你执行 change「{change}」归档链的 delta specs 同步段（完成度核对与归档确认已由桌面完成，无需重复）。

对每个 `openspec/changes/{change}/specs/` 下的 `<capability>/spec.md`：
1. 读 delta 与主基线 `openspec/specs/<capability>/spec.md`（主基线可能不存在）。
2. 按增量语义合并——delta 表达意图而非整体替换，保留 delta 未提及的主 spec 内容：
   - `## ADDED Requirements`：requirement 缺席则追加；已存在则更新为与 delta 一致。
   - `## MODIFIED Requirements`：只应用增量——新增 scenario、修改列中的 scenario、修订描述；不复制既有 scenario。
   - `## REMOVED Requirements`：整块移除该 requirement。
   - `## RENAMED Requirements`：把 FROM: requirement 改名为 TO:。
3. capability 主 spec 缺席时创建：简短 `## Purpose`（TBD 可）+ ADDED requirements。
4. 合并幂等：对已同步的主基线重跑本段应零变化。

约束（必须遵守）：
- 只编辑 `openspec/specs/**`；不修改 `openspec/changes/{change}/`（delta 原件由归档收口整体迁移）。
- 禁止调用 MCP 工具（change_list 等）、禁止 __TOOL_ASK_USER__、不读写 workflow.json。
- 不执行任何 git 命令——提交与合入由桌面编排代执行。
- 完成后最终消息简述各 capability 的合并动作（added / modified / removed / renamed）。"
    )
}

/// 解冲突 prompt（入参为记录供给的 change **name**——prompt 内文面向用户 /
/// 磁盘目录，恒 name 化）。
pub(crate) fn merge_conflict_prompt(change: &str, conflicts: &[String]) -> String {
    let list = conflicts.join("
");
    format!(
        "你执行 change「{change}」归档链的合入冲突解算段。worktree 内重放分支 change/{change} 至主仓当前分支时以下文件冲突（冲突态已保留，rebase 进行中）：

{list}

对每个冲突文件：
1. 读文件内的冲突标记区块（<<<<<<< / ======= / >>>>>>>），结合两侧语义裁决出正确的合并结果。
2. 以裁决结果编辑该文件，移除全部冲突标记。
3. 完成后对该文件执行 `git add <该文件路径>`（逐文件精确 add）。

约束（必须遵守）：
- 只编辑上面清单内的文件；清单外任何文件一律不动（含未提交 / staged 的无关内容——它们属于用户）。
- 禁止 `git add -A` / `git add .`；禁止 `git commit` / `git merge` / `git rebase`（含 `git rebase --continue` / `git rebase --abort`）/ `git reset` / `git stash` 等收口或改写历史的命令——rebase 续走由桌面编排执行。
- 禁止调用 MCP 工具、禁止 __TOOL_ASK_USER__。
- 全部冲突文件解算并 add 后，最终消息简述各文件的裁决要点。"
    )
}
