use crate::decision::{CandidateReport, DecisionInput};
use crate::prompt::{
    decision_answer_prompt, decision_prompt, evaluator_prompt, merge_conflict_prompt,
    spec_sync_prompt,
};
use workflow::model::{ChecklistItem, Verdict};
use workflow::write::{BacktrackOption, BacktrackWindowEntry};

#[test]
fn evaluator_prompt协议附录改指finish_step与红线() {
    let phase_prompt = "Evaluate implement phase for change \"c\".";
    let prompt = evaluator_prompt(phase_prompt);

    // 协议附录：禁调 MCP 写通道四件 + 「调 finish_step 收口」；不再定义 JSON 形状
    assert!(prompt.contains("## 输出协议"), "附录段在场");
    assert!(prompt.contains("禁止调用"), "禁调 MCP 红线前置语在场");
    for tool in ["phase_log", "phase_next", "phase_start", "backtrack"] {
        assert!(prompt.contains(tool), "写通道四件红线含 {tool}");
    }
    assert!(
        prompt.contains("`finish_step`"),
        "收口方式为调用 finish_step 工具"
    );
    assert!(prompt.contains("2000"), "report 上限约定在场");
    assert!(
        !prompt.contains("\"checklist\"") && !prompt.contains("```json"),
        "MUST NOT 再定义 checklist JSON 形状（形状归工具 schema）"
    );
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

/// 决策输入 fixture 构造器（逐用例按需覆写字段）。
fn decision_input(overrides: DecisionInput) -> DecisionInput {
    overrides
}

/// 回溯白名单项 fixture（id + 表内特性描述 + 真实声明产物清单；产物清单取
/// `phase_table` 同名相位的 `artifact_manifest`，与写面 `backtrack_options`
/// 下发形态同源——用例断言的是真实决策 prompt 的形状，不是退化占位）。
fn allowed_phase(phase: &str, description: &str) -> BacktrackOption {
    let table = workflow::write::phase_table("requirement").expect("requirement 表可达");
    let artifacts = table
        .iter()
        .find(|def| def.id == phase)
        .map(|def| def.artifact_manifest.to_vec())
        .unwrap_or_default();
    BacktrackOption {
        phase: phase.to_owned(),
        description: description.to_owned(),
        artifacts,
    }
}

/// 零声明产物的白名单项 fixture（`test-gen` / `code-review` 等相位形态；渲染为
/// 「无声明产物」）。
fn allowed_phase_without_artifacts(phase: &str, description: &str) -> BacktrackOption {
    BacktrackOption {
        phase: phase.to_owned(),
        description: description.to_owned(),
        artifacts: Vec::new(),
    }
}

/// 覆写产物清单的白名单项 fixture（产物段渲染用例面）。
fn allowed_phase_with_artifacts(
    phase: &str,
    description: &str,
    artifacts: &[(&'static str, &'static str, &'static str)],
) -> BacktrackOption {
    BacktrackOption {
        phase: phase.to_owned(),
        description: description.to_owned(),
        artifacts: artifacts
            .iter()
            .map(|(kind, path, title)| workflow::write::ArtifactSpec { kind, path, title })
            .collect(),
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
        backtrack_window: Vec::new(),
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
    // 三动作协议：工具协议形态（动作名逐条在案），退役 retry 行零残留
    for action in ["backtrack", "stop", "ask"] {
        assert!(
            prompt.contains(&format!("`{action}`")),
            "协议行含动作 {action}"
        );
    }
    assert!(
        prompt.contains("finish_step"),
        "协议段要求经 finish_step 提交动作"
    );
    assert!(
        !prompt.contains("\"action\": \"retry\""),
        "退役 retry 动作行零残留"
    );
    assert!(
        !prompt.contains("{\"action\": \"backtrack\"") && !prompt.contains("```json"),
        "MUST NOT 再定义决策 JSON 形状（形状归工具 schema）"
    );
    assert!(prompt.contains("500"), "reason ≤500 约定在场");
    assert!(
        prompt.contains("白名单"),
        "越权红线说明在场（目标须取白名单）"
    );
    assert!(
        prompt.contains("红线") && prompt.contains("拒绝"),
        "拒绝语义在场"
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
        self_included.contains(
            "- proposal（需求提案与规格说明）：`proposal.md`（提案）、`specs/**`（规格）\n"
        ) && self_included.contains(
            "- dev-design（详细设计与任务拆解）：`design.md`（开发设计）、`tasks.md`（任务拆分）\n"
        ),
        "其余条目携描述与真实声明产物且零注记"
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
    assert!(
        not_included.contains("- test-gen（测试代码生成）：无声明产物\n"),
        "test-gen 零声明产物（真实表项）：{not_included}"
    );
}

/// `decision_prompt` 白名单携相位特性描述与可操作产物（正向，表驱动 3 形态）：
/// 每条白名单行渲染 `- <id>（<description>）：<产物段>`，描述与产物取白名单项
/// 随行值；描述空串退化为 id-only 行（不渲染空括号）；零声明产物相位渲染
/// 「无声明产物」。
#[test]
fn decision_prompt白名单逐条携特性描述与可操作产物() {
    // 形态一：真实表项产物（proposal.md + specs/**）与零产物相位对照
    let described = decision_prompt(&decision_input(DecisionInput {
        allowed: vec![
            allowed_phase("proposal", "需求提案与规格说明"),
            allowed_phase("code-review", "代码审查"),
        ],
        ..base_input()
    }));
    assert!(
        described.contains(
            "- proposal（需求提案与规格说明）：`proposal.md`（提案）、`specs/**`（规格）\n"
        ),
        "白名单首条携特性描述与声明产物（路径 + 标题）: {described}"
    );
    assert!(
        described.contains("- code-review（代码审查）：无声明产物\n"),
        "零声明产物相位显式标注"
    );

    // 形态二：多产物逐项渲染（「、」分隔，顺序随清单）
    let multi = decision_prompt(&decision_input(DecisionInput {
        allowed: vec![
            allowed_phase("dev-design", "详细设计与任务拆解"),
            allowed_phase_with_artifacts(
                "test-design",
                "测试设计",
                &[
                    ("markdown-doc", "a.md", "甲"),
                    ("markdown-doc", "b.md", "乙"),
                ],
            ),
        ],
        ..base_input()
    }));
    assert!(
        multi.contains(
            "- dev-design（详细设计与任务拆解）：`design.md`（开发设计）、`tasks.md`（任务拆分）\n"
        ),
        "内置多产物逐项出线"
    );
    assert!(
        multi.contains("- test-design（测试设计）：`a.md`（甲）、`b.md`（乙）\n"),
        "覆写多产物逐项出线"
    );

    // 形态三：描述空串 → id-only 行，不渲染空括号
    let bare = decision_prompt(&decision_input(DecisionInput {
        allowed: vec![allowed_phase_without_artifacts("proposal", "")],
        ..base_input()
    }));
    assert!(
        bare.contains("- proposal：无声明产物\n"),
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
            prompt.contains(&format!("- {}：无声明产物\n", option.phase)),
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
        backtrack_window: Vec::new(),
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
        decision_answer_prompt("用户选了 proposal"),
        decision_answer_prompt("用户选了 proposal"),
        "ask 应答再提面确定性"
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

// ---------------------------------------------------------------------------
// 决策 prompt 的回溯重做窗口段（AC-8 / AC-9 / AC-10）：文案与渲染只调写面单点
// （`workflow::write::backtrack_window_section`），本侧零第二份文案 / 零第二份
// 判据 / 零二次裁剪；空集整段省略。
// ---------------------------------------------------------------------------

/// 窗口记录 fixture：`from_phase` / `to_phase` 取与白名单 / 候选段任何相位 id
/// 均不重名的可辨合成串（使「零泄漏」成为可整篇断言的判据）。
fn window_entry(index: usize, reason: &str) -> BacktrackWindowEntry {
    BacktrackWindowEntry {
        from_phase: format!("phase-from-{index}"),
        from_index: 10 + index,
        to_phase: format!("phase-to-{index}"),
        to_index: index,
        reason: reason.to_owned(),
    }
}

/// 窗口条数（渲染块数的计数面）。
fn block_count(prompt: &str, needle: &str) -> usize {
    prompt.matches(needle).count()
}

/// 含指定窗口集合的决策输入（其余面取 `base_input()`）。
fn input_with_windows(windows: Vec<BacktrackWindowEntry>) -> DecisionInput {
    DecisionInput {
        backtrack_window: windows,
        ..base_input()
    }
}

/// 多条按创建序渲染（正向，单次，AC-10）：2 条窗口 → 段标题在场、两条原因按
/// 入参序各自被 `<backtrack reason>` / `</backtrack>` 定界、块间空行、渲染块数
/// == 2。
#[test]
fn decision_prompt窗口段多条按创建序渲染() {
    let windows = vec![
        window_entry(0, "设计返工：缺产物区组件"),
        window_entry(1, "需求返工：验收口径漂移"),
    ];
    let prompt = decision_prompt(&input_with_windows(windows.clone()));

    assert!(
        prompt.contains("## 回溯重做窗口"),
        "窗口段标题在场: {prompt}"
    );
    assert_eq!(
        block_count(&prompt, "<backtrack reason>"),
        2,
        "渲染块数 == 入参条数"
    );
    assert_eq!(
        block_count(&prompt, "</backtrack>"),
        2,
        "定界收尾行逐条在场"
    );
    for window in &windows {
        assert!(
            prompt.contains(&format!(
                "<backtrack reason>\n{}\n</backtrack>",
                window.reason
            )),
            "每条原因被定界块包裹: {}",
            window.reason
        );
    }
    let first = prompt.find(&windows[0].reason).expect("首条在场");
    let second = prompt.find(&windows[1].reason).expect("次条在场");
    assert!(first < second, "创建序（入参序）渲染，不重排");
    // 块间空行（两定界块以内为空行分隔，非同行拼接）
    assert!(
        !prompt.contains("</backtrack><backtrack reason>"),
        "块间不以零空白相接"
    );
}

/// 段位置断言（正向，单次，AC-10）：窗口段索引 <「失败相位报告」索引，且 >
/// 开头句索引（段序最前、紧跟开头句）。
#[test]
fn decision_prompt窗口段位置先于失败相位报告紧随开头句() {
    let prompt = decision_prompt(&input_with_windows(vec![window_entry(0, "设计返工")]));

    let opening = prompt.find("需要你做出路由决策").expect("开头句在场");
    let window_at = prompt.find("## 回溯重做窗口").expect("窗口段在场");
    let report_at = prompt.find("## 失败相位报告").expect("失败相位报告段在场");
    assert!(
        window_at < report_at,
        "窗口段在「失败相位报告」之前（{window_at} < {report_at}）"
    );
    assert!(
        opening < window_at,
        "窗口段紧跟开头句之后（{opening} < {window_at}）"
    );
    // 其余段序不变：失败清单 < 白名单 < 候选报告 < 报告文件 < 决策协议
    let order = [
        "## 失败相位报告",
        "## 失败清单",
        "## 可回溯白名单",
        "## 候选相位最近一次评估报告",
        "## 报告文件",
        "## 决策协议",
    ];
    let positions: Vec<usize> = order
        .iter()
        .map(|section| {
            prompt
                .find(section)
                .unwrap_or_else(|| panic!("段 {section} 在场"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "其余六段序逐条不变: {positions:?}"
    );
}

/// 文案单点跨 crate（正向，单次，AC-9）：prompt 中的窗口段片段与
/// `workflow::write::backtrack_window_section(&input.backtrack_window)` 逐字节
/// 相等（orchestration 侧零第二份文案的机械判据）。
#[test]
fn decision_prompt窗口段与写面渲染逐字节相等() {
    let input = input_with_windows(vec![
        window_entry(0, "设计返工：缺产物区组件"),
        window_entry(1, "需求返工：验收口径漂移"),
    ]);
    let prompt = decision_prompt(&input);
    let section = workflow::write::backtrack_window_section(&input.backtrack_window);

    assert!(!section.is_empty(), "非空窗口渲非空段");
    assert!(
        prompt.contains(&section),
        "prompt 内窗口段与写面渲染产物逐字节相等（零第二份文案）"
    );
    // 标题字面量在 orchestration 侧零副本：产物片段自写面取得，非本侧拼接
    assert_eq!(
        block_count(&prompt, "## 回溯重做窗口"),
        1,
        "prompt 恰有一处窗口段标题（非两处拼接）"
    );
    // 渲染确定性：同输入两次组装 + 片段位置稳定
    assert_eq!(prompt, decision_prompt(&input), "同输入逐字节一致");
    let at = prompt.find(&section).expect("片段可定位");
    assert_eq!(
        &prompt[at..at + section.len()],
        section.as_str(),
        "定位到的片段逐字节等于写面产物"
    );
}

/// 其余段零扰动（正向，参数化迭代既有 5 段 + 2 注记）：窗口段引入后失败清单 /
/// 白名单 / 候选报告 / 失败相位报告 / 报告文件五段仍在场，协议段仍恰列
/// backtrack / stop / ask 三动作、零退役 `retry`；白名单自注记两形态照旧。
#[test]
fn decision_prompt窗口段引入后其余段零扰动() {
    let prompt = decision_prompt(&input_with_windows(vec![window_entry(0, "设计返工")]));

    for section in [
        "## 失败清单",
        "## 可回溯白名单",
        "## 候选相位最近一次评估报告",
        "## 失败相位报告",
        "## 报告文件",
        "## 决策协议",
    ] {
        assert!(prompt.contains(section), "段 {section} 仍在场");
    }
    // 三动作协议：工具协议形态（动作名逐条在案），退役 retry 行零残留
    for action in ["backtrack", "stop", "ask"] {
        assert!(
            prompt.contains(&format!("`{action}`")),
            "协议行含动作 {action}"
        );
    }
    assert!(
        prompt.contains("finish_step"),
        "协议段要求经 finish_step 提交动作"
    );
    assert!(
        !prompt.contains("\"action\": \"retry\""),
        "退役 retry 动作行零残留"
    );
    assert!(
        !prompt.contains("{\"action\": \"backtrack\"") && !prompt.contains("```json"),
        "MUST NOT 再定义决策 JSON 形状（形状归工具 schema）"
    );
    assert!(
        prompt.contains("- 覆盖 test-design 全行：缺失废弃行承接"),
        "fail checklist 行照旧"
    );
    assert!(
        prompt.contains("设计与任务拆解齐备"),
        "候选 eval report 照旧"
    );
    assert!(
        prompt.contains("## 失败用例明细\n\nexpected '[]' to equal '[a]'"),
        "fail_report 原文照旧"
    );

    // 白名单自注记两形态：自含追加注记、不含零注记
    let self_included = decision_prompt(&DecisionInput {
        phase: "implement".to_owned(),
        allowed: vec![
            allowed_phase("proposal", "需求提案与规格说明"),
            allowed_phase("implement", "代码实现"),
        ],
        backtrack_window: vec![window_entry(0, "设计返工")],
        ..base_input()
    });
    assert!(
        self_included.contains("- implement（代码实现）（= 重做本相位）"),
        "自含条目照旧注记"
    );
    let not_included = decision_prompt(&DecisionInput {
        phase: "test-execution".to_owned(),
        allowed: vec![allowed_phase("test-gen", "测试代码生成")],
        backtrack_window: vec![window_entry(0, "设计返工")],
        ..base_input()
    });
    assert!(
        !not_included.contains("（= 重做本相位）"),
        "白名单不含自身 → 零注记照旧"
    );
}

/// 空集整段省略（异常，单次，AC-10）：`backtrack_window == []` → prompt 零窗口段
/// 字样、零空标题残留；组装结果与「窗口段引入前」逐字节等同（以既有五段 / 协议
/// / 确定性用例零改动全绿为侧证，此处以「空集片段为空串 + 零标题」承载）。
#[test]
fn decision_prompt窗口段空集整段省略() {
    let empty_section = workflow::write::backtrack_window_section(&[]);
    assert_eq!(empty_section, "", "空集片段逐字节空串（插值即省略）");

    let prompt = decision_prompt(&input_with_windows(Vec::new()));
    assert!(
        !prompt.contains("## 回溯重做窗口"),
        "空集零窗口段标题: {prompt}"
    );
    assert!(!prompt.contains("<backtrack"), "空集零定界块残留: {prompt}");
    // 段序不因空集破坏：开头句紧跟「失败相位报告」
    let opening = prompt.find("需要你做出路由决策").expect("开头句在场");
    let report_at = prompt.find("## 失败相位报告").expect("报告段在场");
    assert_eq!(
        &prompt[opening..report_at],
        "需要你做出路由决策。\n\n",
        "空集时开头句与报告段之间恰为原有分隔（窗口段零残留）"
    );
}

/// 零二次裁剪（异常，单次，AC-10 / spec「条目数与双分支一致」）：直接构造 3 条
/// 窗口的 `DecisionInput` → 三条原因全部逐条渲染（块数 == 3）、零「另有 N 条」/
/// 「仅显示最近」类元信息——上限裁剪唯一点在写面。
#[test]
fn decision_prompt窗口段零二次裁剪() {
    let windows: Vec<BacktrackWindowEntry> = (0..3)
        .map(|index| window_entry(index, &format!("R{index}")))
        .collect();
    let prompt = decision_prompt(&input_with_windows(windows.clone()));

    assert_eq!(
        block_count(&prompt, "<backtrack reason>"),
        3,
        "3 条入参 → 3 块（本侧零裁剪）"
    );
    for window in &windows {
        assert!(
            prompt.contains(&window.reason),
            "三条原因逐条渲染: {}",
            window.reason
        );
    }
    for meta in ["另有", "仅显示最近", "更多记录", "剩余"] {
        assert!(!prompt.contains(meta), "零裁剪元信息: {meta}");
    }
    // 与写面渲染逐字节相等（零二次裁剪的机械判据：条数与内容均由写面产物决定）
    assert!(
        prompt.contains(&workflow::write::backtrack_window_section(&windows)),
        "3 条形态与写面产物逐字节相等"
    );
}

/// 零相位 id 泄漏（异常，参数化迭代 2 形态，AC-8）：fixture 的 `from_phase` /
/// `to_phase` 取与白名单 / 候选段任何相位 id 均不重名的可辨合成串 → 二者在整篇
/// prompt 零命中（「零命中」因此可整篇断言）。
#[test]
fn decision_prompt窗口段零相位id泄漏() {
    for count in [1usize, 2] {
        let windows: Vec<BacktrackWindowEntry> = (0..count)
            .map(|index| window_entry(index, &format!("原因{index}")))
            .collect();
        let prompt = decision_prompt(&input_with_windows(windows.clone()));
        for window in &windows {
            assert!(
                !prompt.contains(&window.from_phase),
                "整篇 prompt 对 from_phase \"{}\" 零命中（{count} 条形态）",
                window.from_phase
            );
            assert!(
                !prompt.contains(&window.to_phase),
                "整篇 prompt 对 to_phase \"{}\" 零命中（{count} 条形态）",
                window.to_phase
            );
            // 两个 index 以独立数字形态零泄漏（窗口段只取 reason）
            assert!(
                !prompt.contains(&format!("from_index: {}", window.from_index)),
                "零 index 元信息"
            );
        }
        // 标签行零属性（逐行断言）
        for line in prompt.lines().filter(|line| line.starts_with("<backtrack")) {
            assert_eq!(line, "<backtrack reason>", "标签行逐字、零属性");
        }
    }
}

/// 入参条数四形态（边界，参数化迭代 0 / 1 / 2 / 3）：渲染块数恒等于入参长度
///（1 条不做「退化一行」短式、3 条不被裁），段标题在场性 = 入参是否非空。
#[test]
fn decision_prompt窗口段入参条数四形态() {
    for count in 0..=3usize {
        let windows: Vec<BacktrackWindowEntry> = (0..count)
            .map(|index| window_entry(index, &format!("R{index}")))
            .collect();
        let prompt = decision_prompt(&input_with_windows(windows));
        assert_eq!(
            block_count(&prompt, "<backtrack reason>"),
            count,
            "{count} 条 → 渲染块数 == 入参数"
        );
        assert_eq!(
            block_count(&prompt, "## 回溯重做窗口"),
            if count == 0 { 0 } else { 1 },
            "{count} 条 → 段标题在场性 = 入参是否非空"
        );
        assert_eq!(
            block_count(&prompt, "</backtrack>"),
            count,
            "{count} 条 → 定界收尾行数 == 入参数"
        );
    }
}

/// 原因原文保真（边界，参数化迭代 5 形态，AC-8）：`""` / `"  R  "`（首尾空白）
/// / `"a\nb"` / `"a\0b🚀"` / 恰写面上限 1000 字符（该常量 workflow 侧
/// `pub(crate)`，本侧取不到，故以字面 1000 构造）→ 逐字在 prompt 中，不 trim 不
/// 截断不压平。
#[test]
fn decision_prompt窗口段原因原文保真五形态() {
    let long = "字".repeat(1000);
    let shapes = [
        String::new(),
        "  R  ".to_owned(),
        "a\nb".to_owned(),
        "a\0b🚀".to_owned(),
        long.clone(),
    ];
    for reason in shapes {
        let prompt = decision_prompt(&input_with_windows(vec![window_entry(0, &reason)]));
        assert!(
            prompt.contains(&format!("<backtrack reason>\n{reason}\n</backtrack>")),
            "原因原文逐字定界（不 trim / 不截断 / 不压平）: {reason:?}"
        );
        if reason.chars().count() > 1 {
            assert!(prompt.contains(&reason), "原因本体逐字在场: {reason:?}");
        }
    }
    assert!(
        prompt_contains_exact_reason(&long, 1000),
        "恰 1000 字符原因全文承接（本侧零长度门）"
    );
}

/// 1000 字符原因全文承接（原文保真的长度面判据；避免在断言消息里展开长文）。
fn prompt_contains_exact_reason(reason: &str, expected_chars: usize) -> bool {
    let prompt = decision_prompt(&input_with_windows(vec![window_entry(0, reason)]));
    prompt.contains(reason) && reason.chars().count() == expected_chars
}

/// 信封插值与窗口段并存（边界，参数化迭代 4 形态）：`attempt` 取 `0` 与
/// `u32::MAX`、`phase` 取 `""` 与超长串 → 开头句插值不受窗口段影响、不 panic、
/// 段序稳定。
#[test]
fn decision_prompt信封插值与窗口段并存() {
    let long_phase = "p".repeat(1200);
    for attempt in [0u32, u32::MAX] {
        for phase in [String::new(), long_phase.clone()] {
            let input = DecisionInput {
                phase,
                attempt,
                backtrack_window: vec![window_entry(0, "设计返工")],
                ..base_input()
            };
            let prompt = decision_prompt(&input);
            assert!(
                prompt.contains(&format!("第 {attempt} 次 attempt")),
                "attempt = {attempt} 插值原样"
            );
            assert!(
                prompt.contains("## 回溯重做窗口"),
                "窗口段在场（attempt {attempt}）"
            );
            let window_at = prompt.find("## 回溯重做窗口").expect("窗口段在场");
            let report_at = prompt.find("## 失败相位报告").expect("报告段在场");
            assert!(window_at < report_at, "段序稳定（attempt {attempt}）");
        }
    }
    // 超长 phase 原样插值（不截断）
    assert!(
        prompt_contains_phase(&long_phase),
        "1200 字符 phase 名原样插值（不 panic 不截断）"
    );
}

/// 超长 phase 名原样插值（信封插值面判据）。
fn prompt_contains_phase(phase: &str) -> bool {
    let prompt = decision_prompt(&DecisionInput {
        phase: phase.to_owned(),
        attempt: 1,
        backtrack_window: vec![window_entry(0, "R")],
        ..base_input()
    });
    prompt.contains(&format!("相位「{phase}」"))
}

/// archive 两步与 ask 应答再提面零窗口段（异常，参数化迭代 3 形态，AC-14 行为侧
/// 证）：`spec_sync_prompt` / `merge_conflict_prompt` 产物零窗口段字样；
/// `decision_answer_prompt` 只携应答原文 + 收口指引、零窗口段。
#[test]
fn archive两步与再提面零窗口段() {
    let spec_sync = spec_sync_prompt("demo-change");
    let merge_conflict = merge_conflict_prompt("demo-change", &["specs/a.md".to_owned()]);
    let answer = decision_answer_prompt("越权 backtrack：目标 \"test-gen\" 不在白名单内");
    for (label, prompt) in [
        ("spec_sync_prompt", &spec_sync),
        ("merge_conflict_prompt", &merge_conflict),
        ("decision_answer_prompt", &answer),
    ] {
        assert!(
            !prompt.contains("## 回溯重做窗口"),
            "{label} 零窗口段标题（archive / 再提面恒在窗口外）"
        );
        assert!(!prompt.contains("<backtrack reason>"), "{label} 零定界块");
        assert!(!prompt.contains("设计返工"), "{label} 零窗口原因原文泄漏");
    }
    // 再提面仍只携应答原文 + 一句收口指引（不复述协议形状）
    assert!(
        answer.contains("越权 backtrack：目标 \"test-gen\" 不在白名单内"),
        "应答原文逐字透传"
    );
    assert!(
        answer.contains("finish_step") && !answer.contains("可回溯白名单："),
        "只携收口指引（零协议复述 / 零白名单行）"
    );
    assert!(!answer.contains("## 失败清单"), "再提面不含全量失败明细");
}

// ---------------------------------------------------------------------------
// `decision_answer_prompt`：ask 应答的唯一再提示面（AC-8）
// ---------------------------------------------------------------------------

/// 应答原文逐字在场 + 一句 `finish_step` 收口指引 + 决策语义（正向）。
#[test]
fn decision_answer_prompt应答原文逐字在场且携收口指引() {
    let prompt = decision_answer_prompt("选指数退避");
    assert!(prompt.contains("选指数退避"), "应答原文逐字在场");
    assert!(prompt.contains("finish_step"), "携 finish_step 收口指引");
    assert!(prompt.contains("决策"), "携决策语义");
    assert!(prompt.contains("ask"), "允许再次 ask（应答不足时）");
}

/// 边界（表驱动 3 形态）：空串 → 非空不 panic；两不同应答产出不相等。
#[test]
fn decision_answer_prompt边界三形态() {
    let empty = decision_answer_prompt("");
    assert!(!empty.is_empty(), "空应答仍产出非空提示（不 panic）");
    assert!(empty.contains("finish_step"), "空应答形态仍携收口指引");

    let first = decision_answer_prompt("甲");
    let second = decision_answer_prompt("乙");
    assert_ne!(first, second, "原文进提示（非固定模板）");
    assert!(first.contains("甲") && !first.contains("乙"));
}

/// 文本特殊字符（表驱动 5 形态）：换行 / NUL / emoji / 超长应答逐字保真，
/// 协议字样原样透传（不转义、不解析）。
#[test]
fn decision_answer_prompt特殊字符逐字保真且原样透传() {
    for answer in [
        "第一行\n第二行",
        "含\0空字节",
        "🚀 emoji",
        &"长".repeat(5000),
    ] {
        let prompt = decision_answer_prompt(answer);
        assert!(prompt.contains(answer), "应答原文逐字保真（不截断）");
    }

    let protocol_like = r#"{"action": "stop", "reason": "…"}"#;
    let prompt = decision_answer_prompt(protocol_like);
    assert!(
        prompt.contains(protocol_like),
        "应答含协议字样时原样透传（它不是协议载体）"
    );
    assert!(!prompt.contains("\\\""), "不做 JSON 转义");
}

// ---------------------------------------------------------------------------
// `evaluator_protocol`：删 JSON 形状块与回声禁令（AC-12 反向面）
// ---------------------------------------------------------------------------

/// 产出文本内零 JSON 形状块要素 + 零 phase / attempt / skipped 措辞。
#[test]
fn evaluator_protocol删json形状与回声禁令() {
    let prompt = evaluator_prompt("主体");
    for absent in ["\"verdict\"", "\"checklist\"", "\"report\":", "```json"] {
        assert!(!prompt.contains(absent), "JSON 形状块要素 {absent} 零命中");
    }
    for word in ["phase /", "\"phase\"", "\"attempt\"", "\"skipped\""] {
        assert!(!prompt.contains(word), "回声禁令词 {word} 零命中");
    }
    assert!(
        prompt.contains("verdict") && prompt.contains("推导"),
        "verdict 由推导的说明仍在（不是静默省略）"
    );
    assert!(prompt.contains("evidence"), "逐项 evidence 语义在场");
}

/// 拼接契约（回归锚定）：主体开头 + `\n\n---\n\n` 分隔 + 空主体不 panic。
#[test]
fn evaluator_prompt拼接契约三断言() {
    let prompt = evaluator_prompt("主体");
    assert!(prompt.starts_with("主体"), "以 phase_prompt 开头");
    assert!(
        prompt.contains("\n\n---\n\n## 输出协议"),
        "协议段以分隔符唯一动态 append"
    );

    let empty = evaluator_prompt("");
    assert!(!empty.is_empty(), "空主体产出非空（不 panic）");
    assert!(
        empty.starts_with("---") || empty.starts_with('\n'),
        "空主体只余协议段"
    );
    assert_eq!(
        empty,
        format!("{}", evaluator_protocol_text()),
        "空主体 = 纯协议段"
    );
}

/// 协议段单取（`evaluator_prompt("")` 即纯协议段）。
fn evaluator_protocol_text() -> String {
    evaluator_prompt("")
}

// ---------------------------------------------------------------------------
// `decision_prompt`：删 JSON 代码块（AC-12 反向面）
// ---------------------------------------------------------------------------

/// 产出文本内零围栏 / 零 `"action":` 键面，且 `MAX_REASON_CHARS` 以数值形态提示。
#[test]
fn decision_prompt删json代码块且上限数值在场() {
    let prompt = decision_prompt(&base_input());
    assert!(!prompt.contains("```json"), "围栏零命中");
    assert!(!prompt.contains("\"action\":"), "action 键面零命中");
    assert!(
        prompt.contains(&crate::decision::MAX_REASON_CHARS.to_string()),
        "reason 上限数值仍以 500 形态提示（不退化为字面占位）"
    );
    assert!(
        !prompt.contains("<MAX_REASON_CHARS>"),
        "上限不退化为字面占位"
    );
}

// ---------------------------------------------------------------------------
// 两个 `*_fix_prompt` 退役（AC-7 / AC-8 源码自证）
// ---------------------------------------------------------------------------

#[test]
fn 两个修正prompt退役且prompt源内零残留() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/prompt.rs"),
    )
    .expect("读取 prompt.rs");
    for retired in ["verdict_fix_prompt", "decision_fix_prompt"] {
        assert!(
            !own.contains(retired),
            "prompt.rs 内 {retired} 零命中（退役面）"
        );
    }
    assert!(
        own.contains("fn decision_answer_prompt"),
        "唯一再提示面在 prompt.rs 内"
    );
}
