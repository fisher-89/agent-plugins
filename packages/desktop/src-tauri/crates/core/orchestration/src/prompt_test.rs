use crate::decision::{CandidateReport, DecisionInput};
use crate::prompt::{
    decision_prompt, evaluator_prompt, merge_conflict_prompt, spec_sync_prompt, verdict_fix_prompt,
};
use workflow::model::{ChecklistItem, Verdict};

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
    assert!(prompt.contains("仅修正最终消息的输出格式"), "保留结论引导在场");
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

#[test]
fn decision_prompt有界组装五段在场() {
    let input = DecisionInput {
        phase: "test-gen".to_owned(),
        attempt: 3,
        fail_checklist: vec![ChecklistItem {
            item: "覆盖 test-design 全行".to_owned(),
            pass: false,
            evidence: "缺失废弃行承接".to_owned(),
        }],
        allowed: vec!["proposal".to_owned(), "dev-design".to_owned()],
        candidates: vec![CandidateReport {
            phase: "dev-design".to_owned(),
            verdict: Some(Verdict::Pass),
            report: Some("设计与任务拆解齐备".to_owned()),
        }],
        eval_only: false,
    };

    let prompt = decision_prompt(&input);

    // 五段逐段在场：失败相位信封 + fail checklist 行 + 白名单行 + 候选 eval
    // report + 四动作封闭集说明（有界输入半边）；评估-only 指引段缺席（非
    // 评估-only 相位不注入重试语义注记）
    assert!(
        prompt.contains("test-gen") && prompt.contains("3"),
        "失败相位信封"
    );
    assert!(
        prompt.contains("覆盖 test-design 全行") && prompt.contains("缺失废弃行承接"),
        "fail checklist 行（item + evidence）"
    );
    assert!(
        prompt.contains("proposal") && prompt.contains("dev-design"),
        "白名单行"
    );
    assert!(
        prompt.contains("设计与任务拆解齐备") && prompt.contains("pass"),
        "候选相位最近一次评估报告（verdict + report）"
    );
    for action in ["backtrack", "retry", "stop", "ask"] {
        assert!(prompt.contains(action), "四动作封闭集说明含 {action}");
    }
    assert!(prompt.contains("500"), "reason ≤500 约定在场");
    assert!(
        prompt.contains("白名单") && prompt.contains("拒绝"),
        "越权红线说明在场"
    );
    // 带 executor 相位指引段：retry 携失败清单重跑修复（根因上游仍应回退）
    assert!(
        prompt.contains("该相位带 executor：retry 将携失败清单重新执行本相位修复"),
        "带 executor 指引段在场（重试修复 vs 回退的真实语义）"
    );
    assert!(
        !prompt.contains("评估-only 相位"),
        "带 executor 相位不出评估-only 指引形态"
    );

    // 评估-only 相位（executor 缺席，如 code-review / acceptance fail）：指引段
    // 换形——retry 仅重评不修复、修复须回退前置相位（决策 agent 的语义锚）
    let eval_only_input = DecisionInput {
        eval_only: true,
        ..input
    };
    let eval_only_prompt = decision_prompt(&eval_only_input);
    assert!(
        eval_only_prompt.contains("评估-only 相位（无 executor）")
            && eval_only_prompt.contains("retry 仅会重新评估同一份产物、不会修复"),
        "评估-only 指引段在场（重试重评 vs 回退修复的真实语义）"
    );
    assert!(
        !eval_only_prompt.contains("该相位带 executor"),
        "评估-only 相位不出带 executor 指引形态"
    );
}

#[test]
fn decision_prompt空集三态组装不崩段落缺席稳定() {
    // fail_checklist 空 / 白名单空 / candidates 空三态
    let input = DecisionInput {
        phase: "implement".to_owned(),
        attempt: 1,
        fail_checklist: Vec::new(),
        allowed: Vec::new(),
        candidates: Vec::new(),
        eval_only: false,
    };
    let prompt = decision_prompt(&input);
    assert!(prompt.contains("implement"), "相位信封在场");
    assert!(prompt.contains("backtrack"), "封闭集说明不因空集缺席");
    assert!(!prompt.contains("覆盖"), "空 fail checklist 不虚增行");
    // 未评估候选形态：verdict 缺省降级文案
    let with_none = DecisionInput {
        phase: "implement".to_owned(),
        attempt: 1,
        fail_checklist: Vec::new(),
        allowed: vec!["proposal".to_owned()],
        candidates: vec![CandidateReport {
            phase: "proposal".to_owned(),
            verdict: None,
            report: None,
        }],
        eval_only: false,
    };
    let prompt = decision_prompt(&with_none);
    assert!(
        prompt.contains("无记录") && prompt.contains("（无 eval report）"),
        "候选 verdict / report 双 None 降级文案稳定"
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
        allowed: vec!["proposal".to_owned()],
        candidates: Vec::new(),
        eval_only: false,
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

    assert!(
        prompt.contains("禁止调用 MCP 工具"),
        "MCP 禁用红线显式声明"
    );
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
    let special = vec![
        long_path.clone(),
        "a\nb.md".to_owned(),
        "🚀.md".to_owned(),
    ];
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
