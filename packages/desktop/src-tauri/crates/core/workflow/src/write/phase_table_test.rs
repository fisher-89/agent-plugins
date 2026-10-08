use agent::ModelLevel;

use super::phase_table::{
    allowed_backtrack_phases, interpolate, phase_table, PhaseAgentSpec, MAX_RETRY_TIMES,
};

/// 插件 requirement 相位表表序（与 `lib/workflow.ts` PHASES 一比一——AC-9
/// 逐项对照口径）。
const PLUGIN_PHASE_ORDER: [&str; 8] = [
    "proposal",
    "dev-design",
    "test-design",
    "implement",
    "test-gen",
    "test-execution",
    "code-review",
    "acceptance",
];

/// executor / evaluator 两角色齐备的相位（评估-only 相位除外）。
const DUAL_ROLE_PHASES: [&str; 6] = [
    "proposal",
    "dev-design",
    "test-design",
    "implement",
    "test-gen",
    "test-execution",
];

/// 评估-only 相位（executor 为 None，与插件表一致）。
const EVAL_ONLY_PHASES: [&str; 2] = ["code-review", "acceptance"];

#[test]
fn phase_table_requirement返回全表且相位序与插件逐项对照一致() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // 相位序逐项对照（移植面——AC-9 对照口径）
    let ids: Vec<&str> = table.iter().map(|def| def.id).collect();
    assert_eq!(ids, PLUGIN_PHASE_ORDER.to_vec(), "相位序与插件相位表一致");

    // 前六相位 executor / evaluator 两角色 PhaseAgentSpec 齐备
    for def in table
        .iter()
        .filter(|def| DUAL_ROLE_PHASES.contains(&def.id))
    {
        let executor = def
            .executor
            .as_ref()
            .unwrap_or_else(|| panic!("相位 {} 应有 executor 定义", def.id));
        let evaluator = def
            .evaluator
            .as_ref()
            .unwrap_or_else(|| panic!("相位 {} 应有 evaluator 定义", def.id));
        assert!(
            executor.agent_type.starts_with("__CALL_AGENT:") && executor.agent_type.ends_with("__"),
            "executor agent_type 保留 __CALL_AGENT:<role>__ 约定: {}",
            executor.agent_type
        );
        assert!(
            evaluator.agent_type.starts_with("__CALL_AGENT:")
                && evaluator.agent_type.ends_with("__"),
            "evaluator agent_type 保留 __CALL_AGENT:<role>__ 约定: {}",
            evaluator.agent_type
        );
        assert!(!executor.prompt.is_empty(), "executor prompt 模板在场");
        assert!(!evaluator.prompt.is_empty(), "evaluator prompt 模板在场");
    }

    // 评估-only 相位（code-review / acceptance）：evaluator 在场、executor None
    // （与插件表一比一，不虚设 executor）
    for def in table
        .iter()
        .filter(|def| EVAL_ONLY_PHASES.contains(&def.id))
    {
        assert!(
            def.executor.is_none(),
            "评估-only 相位 {} executor 应为 None（与插件表一致）",
            def.id
        );
        assert!(
            def.evaluator.is_some(),
            "评估-only 相位 {} evaluator 应在场",
            def.id
        );
    }
}

#[test]
fn phase_table_非requirement类型拒绝返回none() {
    // V1 仅 requirement：其余 workflow_type 一律 None（W8 写面侧显式拒绝依据）
    for workflow_type in ["bug-fix", "test-only", "refactor", ""] {
        assert!(
            phase_table(workflow_type).is_none(),
            "workflow_type \"{workflow_type}\" 应拒绝（None）"
        );
    }
}

#[test]
fn phase_table各相位模型档位分派对照() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let expected: &[(&str, Option<ModelLevel>, Option<ModelLevel>)] = &[
        ("proposal", Some(ModelLevel::High), Some(ModelLevel::High)),
        ("dev-design", Some(ModelLevel::High), Some(ModelLevel::High)),
        (
            "test-design",
            Some(ModelLevel::High),
            Some(ModelLevel::High),
        ),
        ("implement", Some(ModelLevel::Low), Some(ModelLevel::High)),
        ("test-gen", Some(ModelLevel::Low), Some(ModelLevel::High)),
        (
            "test-execution",
            Some(ModelLevel::Low),
            Some(ModelLevel::Low),
        ),
        ("code-review", None, Some(ModelLevel::High)),
        ("acceptance", None, Some(ModelLevel::High)),
    ];
    for def in table {
        let (_, executor_level, evaluator_level) = expected
            .iter()
            .find(|(phase, _, _)| *phase == def.id)
            .unwrap_or_else(|| panic!("对照表缺相位 {}", def.id));
        assert_eq!(
            def.executor.as_ref().map(|spec| spec.model_level),
            *executor_level,
            "相位 {} executor 档位",
            def.id
        );
        assert_eq!(
            def.evaluator.as_ref().map(|spec| spec.model_level),
            *evaluator_level,
            "相位 {} evaluator 档位",
            def.id
        );
    }
    // PhaseAgentSpec 档位字段随 Clone 派生保持（相位表驻留 'static 只读共享）
    let spec = table[0]
        .executor
        .clone()
        .unwrap_or_else(|| panic!("首相位有 executor"));
    let PhaseAgentSpec { model_level, .. } = &spec;
    assert_eq!(*model_level, ModelLevel::High, "Clone 后档位保真");
}

#[test]
fn 重试上限常量锚定为5与插件一致() {
    // 与插件 `MAX_RETRY_TIMES` 一致——AC-1 重试上限语义的常量锚
    assert_eq!(MAX_RETRY_TIMES, 5);
}

#[test]
fn interpolate双占位符全量替换且其余字节保真() {
    // 双占位符各自替换、多次出现全替换
    let out = interpolate(
        "<change>/<phase> 与 <change> 再见 <phase>",
        "my-change",
        Some("implement"),
    );
    assert_eq!(out, "my-change/implement 与 my-change 再见 implement");

    // 模板其余字节（换行 / 中文 / markdown 标记）保真
    let template = "# 提案\n\n为 change \"<change>\" 撰写 **proposal.md**。\n\n- 相位：<phase>\n";
    let out = interpolate(template, "桌面变更", Some("test-design"));
    assert_eq!(
        out, "# 提案\n\n为 change \"桌面变更\" 撰写 **proposal.md**。\n\n- 相位：test-design\n",
        "换行 / 中文 / markdown 标记不损"
    );
}

#[test]
fn interpolate_phase缺席时占位符保留而change照常替换() {
    // phase=None：<phase> 按 None 语义保留不崩、<change> 照常替换
    //（与插件 interpolatePrompt None 语义一致）
    let out = interpolate("change=<change> phase=<phase>", "c1", None);
    assert_eq!(out, "change=c1 phase=<phase>");
}

#[test]
fn interpolate无占位符原样返回且未知占位符不误替换() {
    // 无占位符模板原样返回
    assert_eq!(interpolate("纯文本模板", "c", Some("p")), "纯文本模板");

    // 未知占位符（<other>）不误替换
    assert_eq!(
        interpolate("<other> 与 <change>", "c", Some("p")),
        "<other> 与 c"
    );
}

#[test]
fn 白名单首相位为空集无前置可回溯() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // 表首相位 → 空白名单（无前置可回溯——AC-2 空白名单出口的相位表面）
    assert!(
        allowed_backtrack_phases(table, "proposal").is_empty(),
        "首相位白名单应为空"
    );
}

#[test]
fn 白名单返回表序前置集含自身() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // 表中段相位 → 全部前置相位按表序返回（含自身——AC-2 白名单下发的内容面）
    assert_eq!(
        allowed_backtrack_phases(table, "test-gen"),
        vec![
            "proposal".to_owned(),
            "dev-design".to_owned(),
            "test-design".to_owned(),
            "implement".to_owned(),
            "test-gen".to_owned(),
        ],
        "中段相位白名单 = 表序前置集（含自身）"
    );
    // 末相位覆盖全表
    assert_eq!(
        allowed_backtrack_phases(table, "acceptance"),
        PLUGIN_PHASE_ORDER
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn 白名单未知相位返回空vec不臆测() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // current_phase 不在表中 → 空 Vec（不崩、不臆测）
    assert!(allowed_backtrack_phases(table, "不存在的相位").is_empty());
    assert!(allowed_backtrack_phases(table, "").is_empty());
}
