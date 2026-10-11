use agent::ModelLevel;

use super::phase_table::{allowed_backtrack_phases, phase_table, PhaseAgentSpec, MAX_RETRY_TIMES};

/// requirement 相位表表序（相位机语义锚；prompt 静态化不触动相位序）。
const REQUIREMENT_PHASE_ORDER: [&str; 8] = [
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

/// 评估-only 相位（executor 为 None）。
const EVAL_ONLY_PHASES: [&str; 2] = ["code-review", "acceptance"];

#[test]
fn phase_table_requirement返回全表且相位序稳定() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // 相位序稳定（prompt 静态化不漂移相位序 / 路由语义）
    let ids: Vec<&str> = table.iter().map(|def| def.id).collect();
    assert_eq!(ids, REQUIREMENT_PHASE_ORDER.to_vec(), "相位序稳定");

    // 前六相位 executor / evaluator 两角色 PhaseAgentSpec 齐备：
    // 结构锁 = 恰好 prompt / model_level 两字段（无 agent_type），prompt 为静态非空文本
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
        let PhaseAgentSpec {
            prompt,
            model_level,
        } = executor;
        assert!(
            !prompt.is_empty(),
            "executor prompt 静态文本在场: {}",
            def.id
        );
        assert!(
            matches!(model_level, ModelLevel::High | ModelLevel::Low),
            "executor 档位在闭集内: {}",
            def.id
        );
        let PhaseAgentSpec {
            prompt,
            model_level,
        } = evaluator;
        assert!(
            !prompt.is_empty(),
            "evaluator prompt 静态文本在场: {}",
            def.id
        );
        assert!(
            matches!(model_level, ModelLevel::High | ModelLevel::Low),
            "evaluator 档位在闭集内: {}",
            def.id
        );
    }

    // 评估-only 相位（code-review / acceptance）：evaluator 在场、executor None
    //（不虚设 executor）
    for def in table
        .iter()
        .filter(|def| EVAL_ONLY_PHASES.contains(&def.id))
    {
        assert!(
            def.executor.is_none(),
            "评估-only 相位 {} executor 应为 None",
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
        ("implement", Some(ModelLevel::Low), Some(ModelLevel::Low)),
        ("test-gen", Some(ModelLevel::Low), Some(ModelLevel::Low)),
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
    let PhaseAgentSpec {
        prompt: _,
        model_level,
    } = &spec;
    assert_eq!(*model_level, ModelLevel::High, "Clone 后档位保真");
}

/// PhaseAgentSpec 结构收敛（编译期锚）：穷尽解构恰 `prompt` / `model_level`
/// 两字段（无 `..` 兜底——`agent_type` 等字段回归即编译失败）；`prompt` 类型
/// 收窄为 `&'static str` 且静态非空。全表 14 条目（6 executor + 8 evaluator）
/// 逐条覆盖——prompt 为 `include_str!` 编译期装配的静态角色知识，不因相位缺席。
#[test]
fn phase_table_agent_spec结构收敛且全表prompt静态非空() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    let mut executor_count = 0usize;
    let mut evaluator_count = 0usize;
    for def in table {
        if let Some(spec) = &def.executor {
            let PhaseAgentSpec {
                prompt,
                model_level,
            } = spec;
            let _: &&'static str = prompt;
            assert!(!prompt.is_empty(), "executor prompt 静态非空: {}", def.id);
            assert!(
                matches!(model_level, ModelLevel::High | ModelLevel::Low),
                "executor 档位在闭集内: {}",
                def.id
            );
            executor_count += 1;
        }
        if let Some(spec) = &def.evaluator {
            let PhaseAgentSpec {
                prompt,
                model_level,
            } = spec;
            let _: &&'static str = prompt;
            assert!(!prompt.is_empty(), "evaluator prompt 静态非空: {}", def.id);
            assert!(
                matches!(model_level, ModelLevel::High | ModelLevel::Low),
                "evaluator 档位在闭集内: {}",
                def.id
            );
            evaluator_count += 1;
        }
    }
    assert_eq!(executor_count, 6, "6 executor 角色 prompt 齐备");
    assert_eq!(evaluator_count, 8, "8 evaluator 角色 prompt 齐备");
}

#[test]
fn 重试上限常量锚定为5与插件一致() {
    // 与插件 `MAX_RETRY_TIMES` 一致——重试上限语义的常量锚
    assert_eq!(MAX_RETRY_TIMES, 5);
}

#[test]
fn 白名单首相位为空集无前置可回溯() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // 表首相位 → 空白名单（无前置可回溯——空白名单出口的相位表面）
    assert!(
        allowed_backtrack_phases(table, "proposal").is_empty(),
        "首相位白名单应为空"
    );
}

#[test]
fn 白名单返回表序前置集含自身() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // 表中段相位 → 全部前置相位按表序返回（含自身——白名单下发的内容面）
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
        REQUIREMENT_PHASE_ORDER
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
