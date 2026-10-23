use crate::decision::{CandidateReport, DecisionInput};
use crate::prompt::{
    decision_fix_prompt, decision_prompt, evaluator_prompt, merge_conflict_prompt,
    spec_sync_prompt, verdict_fix_prompt,
};
use workflow::model::{ChecklistItem, Verdict};
use workflow::write::BacktrackOption;

#[test]
fn evaluator_prompt协议附录瘦身形状与红线() {
    let phase_prompt = "Evaluate implement phase for change \"c\".";
    let prompt = evaluator_prompt(phase_prompt);

    // 协议附录：禁调 MCP 写通道四件 + 最终消息 checklist JSON 瘦身形状
    assert!(prompt.contains("## 输出协议"), "附录段在场");
    assert!(prompt.contains("禁止调用"), "禁调 MCP 红线前置语在场");
    for tool in ["phase_log", "phase_next", "phase_start", "backtrack"] {
        assert!(prompt.contains(tool), "写通道四件红线含 {tool}");
    }
    assert!(
        prompt.contains("\"verdict\"")
            && prompt.contains("\"report\"")
            && prompt.contains("\"checklist\""),
        "checklist JSON 瘦身形状（verdict / report / checklist）约定在场"
    );
    assert!(prompt.contains("2000"), "report 上限约定在场");
    assert!(
        !prompt.contains("\"phase\"")
            && !prompt.contains("\"attempt\"")
            && !prompt.contains("\"skipped\""),
        "MUST NOT 要求 phase / attempt / skipped 回声（walker 按 provenance 盖戳）"
    );
    // 静态角色知识主体逐字节保真：仅 append 协议附录（零重写 / 零插值）
    assert_eq!(
        prompt,
        format!("{phase_prompt}{}", evaluator_prompt("")),
        "phase_prompt 逐字节保真（仅追加协议附录）"
    );
}

#[test]
fn verdict_fix_prompt解析错误与协议重申在场() {
    let prompt = verdict_fix_prompt("checklist JSON 结构漂移: missing field `verdict`", 2, 5);

    // 反馈修复信封：轮次 + 解析错误 + 保留结论仅修格式的引导
    assert!(prompt.contains("第 2/5 次反馈修复"), "轮次信封在场");
    assert!(
        prompt.contains("checklist JSON 结构漂移: missing field `verdict`"),
        "解析错误原样插值"
    );
    assert!(
        prompt.contains("仅修正最终消息的输出格式"),
        "保留结论引导在场"
    );
    // 协议重申复用 evaluator_protocol 单源：红线与 JSON 形状同附录（不落第三份
    // 副本——与 evaluator_prompt 附录逐字节同尾）
    assert!(
        prompt.contains("## 输出协议")
            && prompt.contains("禁止调用")
            && prompt.contains("\"verdict\"")
            && prompt.contains("\"checklist\""),
        "输出协议附录随修复 prompt 重申"
    );
    assert_eq!(
        prompt,
        format!(
            "评估输出解析失败（第 2/5 次反馈修复）：checklist JSON 结构漂移: missing field `verdict`\n\n\
             请保留你上一轮的评估结论，仅修正最终消息的输出格式后重新收口。{}",
            evaluator_prompt("")
        ),
        "修复 prompt 与 evaluator_prompt 共用同一协议附录文本"
    );
}

/// 决策输入 fixture 构造器（逐用例按需覆写字段）。
fn decision_input(overrides: DecisionInput) -> DecisionInput {
    overrides
}

/// 回溯白名单项 fixture（相位 id + 表内特性描述）。
fn allowed_phase(phase: &str, description: &str) -> BacktrackOption {
    BacktrackOption {
        phase: phase.to_owned(),
        description: description.to_owned(),
    }
}

fn base_input() -> DecisionInput {
    DecisionInput {
        phase: "test-gen".to_owned(),
        attempt: 3,
        fail_checklist: vec![ChecklistItem {
            item: "覆盖 test-design 全行".to_owned(),
            pass: false,
            evidence: "缺失废弃行承接".to_owned(),
        }],
        fail_report: "## 失败用例明细\n\nexpected '[]' to equal '[a]'".to_owned(),
        allowed: vec![
            allowed_phase("proposal", "需求提案与规格说明"),
            allowed_phase("dev-design", "详细设计与任务拆解"),
        ],
        candidates: vec![CandidateReport {
            phase: "dev-design".to_owned(),
            verdict: Some(Verdict::Pass),
            report: Some("设计与任务拆解齐备".to_owned()),
        }],
    }
}

/// `decision_prompt` 段落结构（正向，单值）：失败清单 / 可回溯白名单 / 候选
/// 报告 / 失败相位报告 / 报告文件五段在场；协议段恰列 backtrack / stop / ask
/// 三动作，且不含 `{"action": "retry"}` 行（退役动作零泄漏）。
#[test]
fn decision_prompt五段在场与三动作协议() {
    let prompt = decision_prompt(&base_input());

    for section in [
        "## 失败清单",
        "## 可回溯白名单",
        "## 候选相位最近一次评估报告",
        "## 失败相位报告",
        "## 报告文件",
    ] {
        assert!(prompt.contains(section), "段 {section} 在场");
    }
    // 相位信封 + fail checklist 行 + 候选行
    assert!(
        prompt.contains("test-gen") && prompt.contains("3"),
        "失败相位信封（相位 id 与 attempt）在场"
    );
    assert!(
        prompt.contains("- 覆盖 test-design 全行：缺失废弃行承接"),
        "fail checklist 行（item + evidence）在场"
    );
    assert!(
        prompt.contains("设计与任务拆解齐备") && prompt.contains("pass"),
        "候选相位最近一次评估报告（verdict + report）在场"
    );
    // 三动作协议：逐动作在案，退役 retry 行零残留
    for action in ["backtrack", "stop", "ask"] {
        assert!(
            prompt.contains(&format!("{{\"action\": \"{action}\"")),
            "协议行含动作 {action}"
        );
    }
    assert!(
        !prompt.contains("\"action\": \"retry\""),
        "退役 retry 动作行零残留"
    );
    assert!(prompt.contains("500"), "reason ≤500 约定在场");
    assert!(
        prompt.contains("红线") && prompt.contains("拒绝"),
        "越权红线说明在场"
    );
}

/// `decision_prompt` 失败报告段（正向，单值）：「失败相位报告」段渲染
/// `input.fail_report` 原文（500 字符长文逐字在内）；「报告文件」段含 `read`
/// 与 `reports/test/summary.json`、突变段级 / 归并报告指引。
#[test]
fn decision_prompt失败报告段原文与读报告指引() {
    let fail_report = format!("## 失败明细\n\n{}", "明细行 abcdefghij\n".repeat(60));
    assert!(fail_report.chars().count() > 500);
    let prompt = decision_prompt(&decision_input(DecisionInput {
        fail_report: fail_report.clone(),
        ..base_input()
    }));

    assert!(
        prompt.contains(&fail_report),
        "fail_report 原文逐字渲染（不二次拼装）"
    );
    let section = prompt
        .split("## 失败相位报告")
        .nth(1)
        .expect("失败相位报告段在场");
    assert!(
        section.contains("明细行 abcdefghij"),
        "报告正文落在「失败相位报告」段内"
    );

    assert!(prompt.contains("`read`"), "读报告工具指引在场（read）");
    assert!(
        prompt.contains("reports/test/summary.json"),
        "test-execution 报告路径指引在场"
    );
    assert!(
        prompt.contains("段级") && prompt.contains("归并报告"),
        "突变段级 / 归并报告指引在场"
    );
    assert!(
        prompt.contains("有界") && prompt.contains("不得要求内联"),
        "决策输入有界声明在场"
    );
}

/// `decision_prompt` 白名单自注记（正向，表驱动两形态）：`allowed` 含 `phase`
/// 自身时该条目追加「（= 重做本相位）」，其余条目零注记（两两对照）。
#[test]
fn decision_prompt白名单自注记两形态() {
    // 形态一：白名单自含（可重做相位 → 重做本相位注记）
    let self_included = decision_prompt(&decision_input(DecisionInput {
        phase: "implement".to_owned(),
        allowed: vec![
            allowed_phase("proposal", "需求提案与规格说明"),
            allowed_phase("dev-design", "详细设计与任务拆解"),
            allowed_phase("implement", "代码实现"),
        ],
        ..base_input()
    }));
    assert!(
        self_included.contains("- implement（代码实现）（= 重做本相位）"),
        "自含条目在特性描述后追加注记: {self_included}"
    );
    assert!(
        self_included.contains("- proposal（需求提案与规格说明）\n")
            && self_included.contains("- dev-design（详细设计与任务拆解）\n"),
        "其余条目携描述且零注记"
    );
    assert!(
        !self_included.contains("- proposal（需求提案与规格说明）（= 重做本相位）"),
        "非自身条目不得误注记"
    );

    // 形态二：白名单不含自身（零 agent 步相位）→ 零注记
    let not_included = decision_prompt(&decision_input(DecisionInput {
        phase: "test-execution".to_owned(),
        allowed: vec![allowed_phase("test-gen", "测试代码生成")],
        ..base_input()
    }));
    assert!(
        !not_included.contains("（= 重做本相位）"),
        "白名单不含自身 → 零注记"
    );
}

/// `decision_prompt` 白名单携相位特性描述（正向，表驱动 2 形态）：每条白名单
/// 行渲染 `- <id>（<description>）`，描述取白名单项随行值；描述空串退化为
/// id-only 行（不渲染空括号）。
#[test]
fn decision_prompt白名单逐条携特性描述() {
    // 形态一：描述在场 → id + 全角括号描述
    let described = decision_prompt(&decision_input(DecisionInput {
        allowed: vec![
            allowed_phase("proposal", "需求提案与规格说明"),
            allowed_phase("dev-design", "详细设计与任务拆解"),
        ],
        ..base_input()
    }));
    assert!(
        described.contains("- proposal（需求提案与规格说明）\n"),
        "白名单首条携特性描述: {described}"
    );
    assert!(
        described.contains("- dev-design（详细设计与任务拆解）\n"),
        "白名单次条携特性描述"
    );

    // 形态二：描述空串 → id-only 行，不渲染空括号
    let bare = decision_prompt(&decision_input(DecisionInput {
        allowed: vec![allowed_phase("proposal", "")],
        ..base_input()
    }));
    assert!(
        bare.contains("- proposal\n"),
        "空描述退化为 id-only 行: {bare}"
    );
    assert!(!bare.contains("- proposal（）"), "空描述不得渲染空括号");
}

/// `decision_prompt` 空 / 超长报告（边界，参数化 3 形态）：`fail_report` 空串
/// → 「（无报告文本）」；恰 2000 字符原样；直构 3000 字符异常输入 → 原样插值
/// 不 panic（截断在 walker 侧单点）。
#[test]
fn decision_prompt空与超长报告三形态() {
    let empty = decision_prompt(&decision_input(DecisionInput {
        fail_report: String::new(),
        ..base_input()
    }));
    assert!(empty.contains("（无报告文本）"), "空报告占位形态");
    let section = empty
        .split("## 失败相位报告")
        .nth(1)
        .expect("失败相位报告段在场");
    assert!(section.contains("（无报告文本）"), "占位落在报告段内");

    let exact = "报".repeat(2000);
    let prompt = decision_prompt(&decision_input(DecisionInput {
        fail_report: exact.clone(),
        ..base_input()
    }));
    assert!(prompt.contains(&exact), "恰 2000 字符原样");

    let overlong = "超".repeat(3000);
    let prompt = decision_prompt(&decision_input(DecisionInput {
        fail_report: overlong.clone(),
        ..base_input()
    }));
    assert!(
        prompt.contains(&overlong),
        "3000 字符原样插值不 panic（截断在 walker）"
    );
    assert!(prompt.contains("## 报告文件"), "段结构不因超长报告破坏");
}

/// `decision_prompt` 集合面边界（边界，表驱动 5 形态）：`allowed` 空 / 单元素
/// / 50 项；`candidates` 空 / verdict 全 `None` → 段落结构稳定、占位形态一致。
#[test]
fn decision_prompt集合面边界五形态() {
    // allowed 空 + candidates 空
    let empty = decision_prompt(&decision_input(DecisionInput {
        allowed: Vec::new(),
        candidates: Vec::new(),
        ..base_input()
    }));
    for section in [
        "## 失败相位报告",
        "## 失败清单",
        "## 报告文件",
        "## 决策协议",
    ] {
        assert!(empty.contains(section), "空集形态段 {section} 仍在场");
    }

    // allowed 单元素
    let single = decision_prompt(&decision_input(DecisionInput {
        allowed: vec![allowed_phase("proposal", "需求提案与规格说明")],
        candidates: Vec::new(),
        ..base_input()
    }));
    assert!(
        single.contains("- proposal（需求提案与规格说明）"),
        "单元素白名单行在场"
    );

    // allowed 50 项（逐条出线；描述空串退化为 id-only 行）
    let many: Vec<BacktrackOption> = (0..50)
        .map(|index| allowed_phase(&format!("phase-{index}"), ""))
        .collect();
    let prompt = decision_prompt(&decision_input(DecisionInput {
        allowed: many.clone(),
        ..base_input()
    }));
    for option in &many {
        assert!(
            prompt.contains(&format!("- {}\n", option.phase)),
            "50 项逐条出线: {}",
            option.phase
        );
    }

    // candidates verdict 全 None（占位形态稳定）
    let none_candidates = decision_prompt(&decision_input(DecisionInput {
        candidates: vec![
            CandidateReport {
                phase: "proposal".to_owned(),
                verdict: None,
                report: None,
            },
            CandidateReport {
                phase: "dev-design".to_owned(),
                verdict: None,
                report: Some("报告正文".to_owned()),
            },
        ],
        ..base_input()
    }));
    assert!(
        none_candidates.contains("无记录") && none_candidates.contains("（无 eval report）"),
        "候选 verdict / report 缺省占位形态稳定"
    );
    assert!(
        none_candidates.contains("报告正文"),
        "单侧缺省（report 在场）逐条可辨"
    );
}

/// `decision_prompt` checklist 边界（边界，参数化 3 形态）：`fail_checklist`
/// 空 / 单条 / 含换行、emoji、`\0` 的 evidence → 段落不被破坏、条目逐条出线。
#[test]
fn decision_prompt_checklist边界三形态() {
    let empty = decision_prompt(&decision_input(DecisionInput {
        fail_checklist: Vec::new(),
        ..base_input()
    }));
    assert!(empty.contains("## 失败清单"), "空 checklist 段仍在场");
    assert!(
        !empty.contains("- 覆盖 test-design 全行"),
        "空 checklist 不虚增行"
    );

    let single = decision_prompt(&decision_input(DecisionInput {
        fail_checklist: vec![ChecklistItem {
            item: "单条".to_owned(),
            pass: false,
            evidence: "依据".to_owned(),
        }],
        ..base_input()
    }));
    assert!(single.contains("- 单条：依据"), "单条逐条出线");

    let special = decision_prompt(&decision_input(DecisionInput {
        fail_checklist: vec![
            ChecklistItem {
                item: "换行项".to_owned(),
                pass: false,
                evidence: "第一行\n第二行".to_owned(),
            },
            ChecklistItem {
                item: "emoji 项".to_owned(),
                pass: false,
                evidence: "🚀 findings: a\0b".to_owned(),
            },
        ],
        ..base_input()
    }));
    assert!(
        special.contains("第一行\n第二行") && special.contains("🚀 findings: a\0b"),
        "特殊字符 evidence 原样出线"
    );
    assert!(
        special.contains("## 可回溯白名单"),
        "后续段结构不被特殊字符破坏"
    );
}

/// `decision_prompt` 静态与确定性（异常）：组装结果零模板占位残留（`{phase}`
/// 等花括号不泄漏）；同输入重复组装逐字节一致。
#[test]
fn decision_prompt零占位残留与确定性() {
    let input = base_input();
    let prompt = decision_prompt(&input);

    for placeholder in ["{phase}", "{attempt}", "{fail_report}", "{allowed}", "{{"] {
        assert!(
            !prompt.contains(placeholder),
            "零模板占位残留: {placeholder}"
        );
    }
    assert_eq!(prompt, decision_prompt(&input), "同输入逐字节一致");
}

/// `decision_fix_prompt` 组装（正向）：含驳回原因原文 + 三动作重申 + 白名单一行
///（`id（特性描述） / id（特性描述）` 形式）。
#[test]
fn decision_fix_prompt组装含驳回原因三动作与白名单行() {
    let input = decision_input(DecisionInput {
        allowed: vec![
            allowed_phase("proposal", "需求提案与规格说明"),
            allowed_phase("dev-design", "详细设计与任务拆解"),
            allowed_phase("implement", "代码实现"),
        ],
        ..base_input()
    });
    let prompt = decision_fix_prompt(&input, "越权 backtrack：目标 \"test-gen\" 不在白名单内");

    assert!(
        prompt.contains("上一轮决策输出被驳回：")
            && prompt.contains("目标 \"test-gen\" 不在白名单内"),
        "驳回原因原文在场"
    );
    assert!(
        prompt.contains("三动作封闭集：backtrack / stop / ask"),
        "三动作重申在场"
    );
    assert!(
        prompt.contains(
            "可回溯白名单：proposal（需求提案与规格说明） / dev-design（详细设计与任务拆解） / implement（代码实现）"
        ),
        "白名单一行（id + 特性描述）在场"
    );
    assert!(
        prompt.contains("回跳目标须在白名单内") && prompt.contains("ask"),
        "回跳纪律与 ask 出路在场"
    );
    // 纠正面不含全量失败明细（单点：只重申协议与可选目标）
    assert!(
        !prompt.contains("覆盖 test-design 全行") && !prompt.contains("## 失败清单"),
        "纠正文案不含全量失败明细"
    );
}

/// `decision_fix_prompt` 边界（边界，参数化 4 形态）：`error` 空串 / 500 字符 /
/// 含换行；`allowed` 空（白名单行空占位）→ 组装不 panic 且段结构稳定。
#[test]
fn decision_fix_prompt边界四形态() {
    let cases = [
        String::new(),
        "错".repeat(500),
        "第一行错误\n第二行细节".to_owned(),
        "a\0b🚀".to_owned(),
    ];
    for error in cases {
        let prompt = decision_fix_prompt(&base_input(), &error);
        assert!(
            prompt.contains(&format!("上一轮决策输出被驳回：{error}")),
            "error 原样插值: {error:?}"
        );
        assert!(prompt.contains("三动作封闭集"), "段结构稳定");
    }

    // allowed 空：白名单行空占位、不 panic
    let prompt = decision_fix_prompt(
        &decision_input(DecisionInput {
            allowed: Vec::new(),
            ..base_input()
        }),
        "任意错误",
    );
    assert!(
        prompt.contains("可回溯白名单：\n") || prompt.contains("可回溯白名单："),
        "空白名单行在场（空占位）"
    );
    assert!(prompt.contains("ask"), "空白名单仍给 ask 出路");
}

/// `decision_fix_prompt` 协议一致（异常）：三动作清单与红线句与
/// `decision_prompt` 同源，不含退役 `retry` 动作。
#[test]
fn decision_fix_prompt协议一致无退役动作() {
    let input = base_input();
    let fix = decision_fix_prompt(&input, "任意错误");
    let full = decision_prompt(&input);

    for action in ["backtrack", "stop", "ask"] {
        assert!(fix.contains(action), "纠正文案重申动作 {action}");
        assert!(full.contains(action), "完整 prompt 三动作在案 {action}");
    }
    assert!(!fix.contains("retry"), "纠正文案零退役 retry 动作: {fix}");
    assert!(
        full.contains("红线") && fix.contains("回跳目标须在白名单内"),
        "红线语义同源（完整 prompt 红线句 ↔ 纠正文案回跳纪律）"
    );
}

#[test]
fn prompt确定性_同输入重复组装逐字节一致() {
    let phase_prompt = "固定静态主体（零占位）";
    let input = DecisionInput {
        phase: "implement".to_owned(),
        attempt: 2,
        fail_checklist: vec![ChecklistItem {
            item: "项".to_owned(),
            pass: false,
            evidence: "据".to_owned(),
        }],
        fail_report: "失败明细".to_owned(),
        allowed: vec![allowed_phase("proposal", "需求提案与规格说明")],
        candidates: Vec::new(),
    };

    assert_eq!(
        evaluator_prompt(phase_prompt),
        evaluator_prompt(phase_prompt),
        "evaluator prompt 确定性"
    );
    assert_eq!(
        decision_prompt(&input),
        decision_prompt(&input),
        "决策 prompt 确定性"
    );
    assert_eq!(
        decision_fix_prompt(&input, "错误"),
        decision_fix_prompt(&input, "错误"),
        "决策纠正文案确定性"
    );
}

// ---------------------------------------------------------------------------
// 归档 agent prompt 单点（自 archive_flow.rs 搬移：组装 / 边界 / 静态红线；
// AC-3。文本与搬移前逐字节一致的 diff 面归 code-review，行为面以组装与确定性锚定）
// ---------------------------------------------------------------------------

/// spec 同步 prompt 组装（正向）：change 名插值（name 化）+ 增量合并语义主体
///（ADDED / MODIFIED / REMOVED / RENAMED）+ 约束段（只编辑主 specs / 零 git）。
#[test]
fn spec_sync_prompt组装含change名与增量合并语义() {
    let prompt = spec_sync_prompt("demo-change");

    assert!(prompt.contains("demo-change"), "change 名插值在场");
    assert!(
        prompt.contains("openspec/changes/demo-change/specs/"),
        "delta specs 路径插值"
    );
    for semantic in [
        "## ADDED Requirements",
        "## MODIFIED Requirements",
        "## REMOVED Requirements",
        "## RENAMED Requirements",
    ] {
        assert!(prompt.contains(semantic), "增量合并语义段 {semantic} 在场");
    }
    assert!(prompt.contains("幂等"), "幂等约束在场");
    assert!(prompt.contains("## Purpose"), "capability 缺席创建指引在场");
    assert!(
        prompt.contains("只编辑 `openspec/specs/**`"),
        "主 specs 单点编辑约束在场（delta 原件零改写）"
    );
    assert!(
        prompt.contains("不执行任何 git 命令"),
        "git 面收缩（归档编排代执行收口）"
    );
}

/// spec 同步 prompt 边界（边界）：空串 / 超长（>1000 chars）/ 特殊字符
///（换行 / NUL / emoji）→ 不 panic，插值原样保留。
#[test]
fn spec_sync_prompt边界空串超长与特殊字符原样插值() {
    // 空串：静态模板照常产出（插值位为空）
    let empty = spec_sync_prompt("");
    assert!(
        empty.contains("openspec/changes//specs/"),
        "空 change 名原样插值"
    );
    assert!(
        empty.contains("## ADDED Requirements"),
        "静态主体不因空串缺席"
    );

    // 超长（>1000 chars）：原样插值不截断不 panic
    let long = "a".repeat(1200);
    let long_prompt = spec_sync_prompt(&long);
    assert!(
        long_prompt.contains(&format!("openspec/changes/{long}/specs/")),
        "超长 change 名原样插值"
    );

    // 特殊字符：换行 / NUL / emoji 原样保留
    for special in ["a\nb", "a\0b", "change-🚀-emoji", "  "] {
        let prompt = spec_sync_prompt(special);
        assert!(
            prompt.contains(&format!("openspec/changes/{special}/specs/")),
            "特殊字符 {special:?} 原样插值"
        );
    }
}

/// spec 同步 prompt 静态红线（异常）：零 `<role>` / `{{…}}` 占位、零 CLI skill
/// 溯源；MCP 与 workflow.json 面恒为禁用声明（零读写依赖）。
#[test]
fn spec_sync_prompt静态红线零占位零外部依赖() {
    let prompt = spec_sync_prompt("demo-change");

    assert!(prompt.contains("禁止调用 MCP 工具"), "MCP 禁用红线显式声明");
    assert!(!prompt.contains("<role>"), "零 <role> 占位");
    assert!(!prompt.contains("{{"), "零 {{…}} 占位");
    assert!(!prompt.contains("skill"), "零 CLI skill 溯源");
    assert!(!prompt.contains("include_str"), "零编译期占位标记残留");
    assert!(
        prompt.contains("不读写 workflow.json"),
        "workflow.json 面为禁用声明"
    );
}

/// 解冲突 prompt 组装（正向）：change 名与逐条冲突路径插值 + 解算纪律（标记
/// 裁决 / 逐文件精确 add / 清单外零改动 / 收口命令禁用）。
#[test]
fn merge_conflict_prompt组装含change名与逐条冲突路径() {
    let conflicts = vec![
        "specs/a.md".to_owned(),
        "openspec/changes/c/tasks.md".to_owned(),
    ];
    let prompt = merge_conflict_prompt("demo-change", &conflicts);

    assert!(prompt.contains("demo-change"), "change 名插值在场");
    assert!(
        prompt.contains("change/demo-change"),
        "归档分支名插值（change/<name>）"
    );
    for path in &conflicts {
        assert!(prompt.contains(path), "冲突路径逐条插值: {path}");
    }
    assert!(prompt.contains("<<<<<<<"), "冲突标记形态说明在场");
    assert!(prompt.contains("git add"), "逐文件精确 add 纪律在场");
    assert!(
        prompt.contains("禁止 `git add -A`") && prompt.contains("git commit"),
        "收口命令禁用纪律在场"
    );
    assert!(
        prompt.contains("清单外任何文件一律不动"),
        "清单外零改动约束在场"
    );
}

/// 解冲突 prompt 边界（边界）：conflicts 空列表 / 单元素 / 超大列表；单条路径
/// 超长 / 含特殊字符 → 不 panic、逐条可辨。
#[test]
fn merge_conflict_prompt边界空列表单元素超大列表与特殊路径() {
    // 空列表：清单段为空，静态纪律照常（零 panic）
    let empty = merge_conflict_prompt("demo-change", &[]);
    assert!(empty.contains("demo-change"));
    assert!(
        empty.contains("约束（必须遵守）"),
        "静态纪律段不因空清单缺席"
    );

    // 单元素
    let single = merge_conflict_prompt("demo-change", &["a.md".to_owned()]);
    assert!(single.contains("a.md"));

    // 超大列表（128 条）：逐条透出、不截断
    let many: Vec<String> = (0..128)
        .map(|index| format!("specs/file-{index:03}.md"))
        .collect();
    let prompt = merge_conflict_prompt("demo-change", &many);
    for path in &many {
        assert!(prompt.contains(path), "超大列表逐条透出: {path}");
    }

    // 超长单条路径（>1000 chars）与特殊字符：原样保留
    let long_path = format!("{}/a.md", "d".repeat(1200));
    let special = vec![long_path.clone(), "a\nb.md".to_owned(), "🚀.md".to_owned()];
    let prompt = merge_conflict_prompt("demo-change", &special);
    assert!(prompt.contains(&long_path), "超长冲突路径原样保留");
    assert!(prompt.contains("a\nb.md"), "换行字符原样保留（逐条可辨）");
    assert!(prompt.contains("🚀.md"), "emoji 路径原样保留");
}

/// 两 prompt 确定性（回归）：同输入重复组装逐字节一致（搬移保真的行为面锚）。
#[test]
fn 归档两prompt确定性_同输入重复组装逐字节一致() {
    assert_eq!(
        spec_sync_prompt("demo-change"),
        spec_sync_prompt("demo-change")
    );
    let conflicts = vec!["specs/a.md".to_owned()];
    assert_eq!(
        merge_conflict_prompt("demo-change", &conflicts),
        merge_conflict_prompt("demo-change", &conflicts)
    );
}
