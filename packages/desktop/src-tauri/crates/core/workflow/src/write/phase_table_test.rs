use agent::ModelLevel;
use serde::{Deserialize, Serialize};

use super::phase_table::{
    allowed_backtrack_phases, dependents, phase_table, PhaseAgentSpec, PhaseDefinition,
    MAX_RETRY_TIMES,
};
use crate::state::{OnFail, StepDefinition, StepNode};

/// requirement 相位表表序（相位机语义锚；prompt 静态化不触动相位序）。
const REQUIREMENT_PHASE_ORDER: [&str; 9] = [
    "proposal",
    "dev-design",
    "test-design",
    "implement",
    "test-gen",
    "test-execution",
    "code-review",
    "acceptance",
    "archive",
];

/// test-only 相位表表序（6 相位：5 个 agent 相位 + archive 确定性相位）。
const TEST_ONLY_PHASE_ORDER: [&str; 6] = [
    "proposal",
    "code-analyze",
    "test-design",
    "test-gen",
    "test-execution",
    "archive",
];

/// code-analyze 相位步序（P→E 分析相位；AC-2）。
const CODE_ANALYZE_STEP_ORDER: [StepNode; 5] = [
    StepNode::PhaseStart,
    StepNode::Executor,
    StepNode::Evaluator,
    StepNode::VerdictGate,
    StepNode::PhaseLog,
];

/// 归档相位步序（design 数据模型表；AC-2）。
const ARCHIVE_STEP_ORDER: [StepNode; 10] = [
    StepNode::PhaseStart,
    StepNode::Preflight,
    StepNode::Commit,
    StepNode::Rebase,
    StepNode::ResolveConflicts,
    StepNode::Merge,
    StepNode::SpecSync,
    StepNode::Seal,
    StepNode::Finalize,
    StepNode::PhaseLog,
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

/// 按 id 取相位定义（缺表项即 panic——fixture 表漂移立即显形）。
fn phase_of(table: &'static [PhaseDefinition], id: &str) -> &'static PhaseDefinition {
    table
        .iter()
        .find(|def| def.id == id)
        .unwrap_or_else(|| panic!("相位 {id} 应在表内"))
}

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
fn phase_table_未知类型拒绝返回none() {
    // 两值分派：requirement / test-only 外的其余 workflow_type 一律 None（写面侧
    // 显式拒绝依据——不退化、不臆测、零兜底）
    for workflow_type in ["bug-fix", "refactor", "", " ", "requirement2", "test_only"] {
        assert!(
            phase_table(workflow_type).is_none(),
            "workflow_type {workflow_type:?} 应拒绝（None）"
        );
    }
}

/// 未知类型边界（边界 / 新增）：超长（>1000 字符）与特殊字符（NUL / 内嵌换行 /
/// emoji）类型值一律不命中——trim 后仍非两合法值 → `None`（不截断、不折叠、不
/// panic，零兜底）。
#[test]
fn phase_table超长与特殊字符类型值不命中() {
    let long_unknown = "x".repeat(1200);
    let long_known_prefix = format!("{}requirement", "y".repeat(1200));
    for workflow_type in [
        long_unknown.as_str(),
        long_known_prefix.as_str(),
        "requirement\u{0}",
        "test\u{0}-only",
        "require\nment",
        "test-only🚀",
        "\u{0}",
    ] {
        assert!(
            phase_table(workflow_type).is_none(),
            "超长 / 特殊字符类型值 {workflow_type:?} 应拒绝（None）"
        );
    }
    assert!(
        long_unknown.len() > 1000 && long_known_prefix.len() > 1000,
        "边界样本须越过 1000 字符刻度，实际 {} / {}",
        long_unknown.len(),
        long_known_prefix.len()
    );
}

#[test]
fn phase_table_test_only返回六相位且序稳定() {
    let table = phase_table("test-only").expect("test-only 应返回 Some");

    assert_eq!(table.len(), 6, "test-only 六相位（含 archive）");
    let ids: Vec<&str> = table.iter().map(|def| def.id).collect();
    assert_eq!(ids, TEST_ONLY_PHASE_ORDER.to_vec(), "test-only 相位序稳定");

    // 末位 archive 确定性相位（无 executor / evaluator、零清单）
    let archive = table.last().expect("末位相位应在场");
    assert_eq!(archive.id, "archive", "archive 为末位（第 6）相位");
    assert!(archive.executor.is_none(), "test-only archive executor 恒 None");
    assert!(archive.evaluator.is_none(), "test-only archive evaluator 恒 None");
    assert!(archive.artifact_manifest.is_empty(), "test-only archive 零清单");

    // requirement 独有相位不在 test-only 表内（两表分派不串味）
    for absent in ["dev-design", "implement", "code-review", "acceptance"] {
        assert!(!ids.contains(&absent), "test-only 表不含 requirement 独有相位 {absent}");
    }
    // code-analyze 唯 test-only 表独有（requirement 表零此相位）
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    assert!(
        !requirement.iter().any(|def| def.id == "code-analyze"),
        "code-analyze 为 test-only 独有相位"
    );
}

/// 表间复用矩阵（正向）：test-only 的 proposal / test-design / test-gen /
/// test-execution 直接复用 requirement 同名 static（steps 与角色 prompt 同源），
/// archive 复用 `ARCHIVE_STEPS`，code-analyze 用新增 `CODE_ANALYZE_STEPS`
///（复用即单源——不复制步序列）。
#[test]
fn test_only表复用requirement同名steps与prompt() {
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");

    for id in ["proposal", "test-design", "test-gen", "test-execution"] {
        let lhs = phase_of(requirement, id);
        let rhs = phase_of(test_only, id);
        assert_eq!(
            lhs.steps, rhs.steps,
            "相位 {id} 复用 requirement 同名 steps（步序列逐位等值）"
        );
        assert_eq!(
            lhs.executor, rhs.executor,
            "相位 {id} 复用 requirement 同名 executor prompt spec"
        );
        assert_eq!(
            lhs.evaluator, rhs.evaluator,
            "相位 {id} 复用 requirement 同名 evaluator prompt spec"
        );
        assert_eq!(
            lhs.artifact_manifest, rhs.artifact_manifest,
            "相位 {id} 复用 requirement 同名产物清单"
        );
    }

    // archive 复用 ARCHIVE_STEPS（步序列逐位等值）
    let test_only_archive = phase_of(test_only, "archive");
    let archive_steps: Vec<StepNode> = test_only_archive.steps.iter().map(|step| step.node).collect();
    assert_eq!(archive_steps, ARCHIVE_STEP_ORDER.to_vec(), "test-only archive 复用 ARCHIVE_STEPS");

    // code-analyze 用 CODE_ANALYZE_STEPS（P→E 五步形态）
    let code_analyze = phase_of(test_only, "code-analyze");
    let code_analyze_steps: Vec<StepNode> =
        code_analyze.steps.iter().map(|step| step.node).collect();
    assert_eq!(
        code_analyze_steps,
        CODE_ANALYZE_STEP_ORDER.to_vec(),
        "code-analyze 步序 = [PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]"
    );
    // 提案相位描述改「测试向提案」（复用 step 静态但人读语义随类型）
    assert_eq!(phase_of(test_only, "proposal").description, "测试向提案");
}

/// code-analyze 相位声明（正向 / AC-2）：步序 P→E 五步、VerdictGate
/// `on_fail=Terminal`、executor / evaluator 均 `ModelLevel::High`、产物清单空、
/// 两常量静态非空零占位。
#[test]
fn code_analyze相位声明_p形态与零清单与双高角色() {
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");
    let code_analyze = phase_of(test_only, "code-analyze");

    // 步序 P→E（与 proposal / dev-design 同形）
    let nodes: Vec<StepNode> = code_analyze.steps.iter().map(|step| step.node).collect();
    assert_eq!(nodes, CODE_ANALYZE_STEP_ORDER.to_vec());
    assert_eq!(
        code_analyze.steps.first().map(|step| step.node),
        Some(StepNode::PhaseStart)
    );
    assert_eq!(
        code_analyze.steps.last().map(|step| step.node),
        Some(StepNode::PhaseLog)
    );

    // VerdictGate 携 Terminal；其余步 on_fail 恒 None
    let verdict = code_analyze
        .steps
        .iter()
        .find(|step| step.node == StepNode::VerdictGate)
        .expect("code-analyze 应含 VerdictGate 步");
    assert_eq!(verdict.on_fail, Some(OnFail::Terminal), "VerdictGate 失败即终态");
    for step in code_analyze.steps {
        if step.node != StepNode::VerdictGate {
            assert_eq!(step.on_fail, None, "步 {:?} 无失败语义（缺省 Terminal）", step.node);
        }
    }

    // 双角色均 High 档、prompt 静态非空零占位（`include_str!` 编译期装配）
    let executor = code_analyze.executor.as_ref().expect("code-analyze 应有 executor");
    let evaluator = code_analyze.evaluator.as_ref().expect("code-analyze 应有 evaluator");
    assert_eq!(executor.model_level, ModelLevel::High, "code-analyze executor 档位");
    assert_eq!(evaluator.model_level, ModelLevel::High, "code-analyze evaluator 档位");
    for spec in [executor, evaluator] {
        assert!(!spec.prompt.is_empty(), "code-analyze prompt 静态非空");
        assert!(
            !spec.prompt.contains("<role>") && !spec.prompt.contains("{{"),
            "code-analyze prompt 零占位"
        );
    }
    // 步 prompt_ref 与角色 spec prompt 同源（防两处字面路径漂移）
    let executor_step = code_analyze
        .steps
        .iter()
        .find(|step| step.node == StepNode::Executor)
        .expect("含 Executor 步");
    let evaluator_step = code_analyze
        .steps
        .iter()
        .find(|step| step.node == StepNode::Evaluator)
        .expect("含 Evaluator 步");
    assert_eq!(executor_step.prompt_ref, Some(executor.prompt));
    assert_eq!(evaluator_step.prompt_ref, Some(evaluator.prompt));
    assert_ne!(
        executor.prompt, evaluator.prompt,
        "executor / evaluator 为两份各异角色知识（planner / evaluator）"
    );

    // 产物清单空数组（分析相位不铸产物）
    assert!(
        code_analyze.artifact_manifest.is_empty(),
        "code-analyze 清单空数组（不虚设占位项）"
    );
}

/// 空白 / 大小写边界（边界）：`phase_table` 先 trim 再 `eq_ignore_ascii_case`
/// 命中——首尾空白与任意大小写不误判、不退化。
#[test]
fn phase_table_空白与大小写规范化命中() {
    let requirement = phase_table("requirement").expect("requirement 命中");
    let test_only = phase_table("test-only").expect("test-only 命中");

    // 大小写不敏感
    assert_eq!(phase_table("REQUIREMENT").map(|t| t.len()), Some(requirement.len()));
    assert_eq!(phase_table("Test-Only").map(|t| t.len()), Some(test_only.len()));
    assert_eq!(phase_table("test-ONLY").map(|t| t.len()), Some(test_only.len()));
    // 首尾空白 trim 后命中（" test-only " 不因空白误判为未知）
    assert_eq!(phase_table(" requirement ").map(|t| t.len()), Some(requirement.len()));
    assert_eq!(phase_table(" test-only ").map(|t| t.len()), Some(test_only.len()));
    assert_eq!(phase_table("\ttest-only\n").map(|t| t.len()), Some(test_only.len()));
    // 表序 / 表身同源（返回同一静态表）
    assert_eq!(
        phase_table("TEST-ONLY").map(|t| t.as_ptr()),
        Some(test_only.as_ptr())
    );
    // 空白+大小写混排仍不越集
    assert!(phase_table(" BUG-FIX ").is_none());
}

/// 表内自描述前置依赖（边界）：test-only 表 `prerequisites` 逐相位照表声明
///（proposal=[]、code-analyze=[proposal]、test-design=[proposal, code-analyze]、
/// test-gen=[test-design]、test-execution=[test-gen]、archive=[]）；requirement
/// 表前置与既有 match 语义一致（集合语义对照，不依赖声明序）。
#[test]
fn 表内自描述prerequisites逐相位声明() {
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");
    let expect_test_only: &[(&str, &[&str])] = &[
        ("proposal", &[]),
        ("code-analyze", &["proposal"]),
        ("test-design", &["proposal", "code-analyze"]),
        ("test-gen", &["test-design"]),
        ("test-execution", &["test-gen"]),
        ("archive", &[]),
    ];
    for def in test_only {
        let (_, expected) = expect_test_only
            .iter()
            .find(|(id, _)| *id == def.id)
            .unwrap_or_else(|| panic!("对照表缺相位 {}", def.id));
        assert_eq!(def.prerequisites, *expected, "test-only 相位 {} 前置依赖", def.id);
    }

    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let expect_requirement: &[(&str, &[&str])] = &[
        ("proposal", &[]),
        ("dev-design", &["proposal"]),
        ("test-design", &["proposal", "dev-design"]),
        ("implement", &["dev-design"]),
        ("test-gen", &["test-design", "implement"]),
        ("test-execution", &["test-gen", "implement"]),
        ("code-review", &["test-gen", "implement"]),
        ("acceptance", &["proposal", "dev-design", "implement"]),
        ("archive", &[]),
    ];
    for def in requirement {
        let (_, expected) = expect_requirement
            .iter()
            .find(|(id, _)| *id == def.id)
            .unwrap_or_else(|| panic!("对照表缺相位 {}", def.id));
        assert_eq!(def.prerequisites.len(), expected.len(), "相位 {} 前置个数", def.id);
        for prerequisite in *expected {
            assert!(
                def.prerequisites.contains(prerequisite),
                "相位 {} 前置应含 {prerequisite}",
                def.id
            );
        }
        // 前置恒为表内已知相位 id（零悬空引用）、且不含自身
        let ids: Vec<&str> = requirement.iter().map(|phase| phase.id).collect();
        for prerequisite in def.prerequisites {
            assert!(ids.contains(prerequisite), "相位 {} 前置 {prerequisite} 应在表内", def.id);
            assert_ne!(*prerequisite, def.id, "相位 {} 前置不含自身（无自环）", def.id);
        }
    }
}

/// `dependents` 表内遍历（正向）：下游 = 表内 `prerequisites` 含本相位者，两表
/// 各自类型感知（test-only 不多推 dev-design 等表外相位）。
#[test]
fn dependents下游遍历随表分派() {
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");

    assert_eq!(
        dependents(requirement, "proposal"),
        vec!["dev-design".to_owned(), "test-design".to_owned(), "acceptance".to_owned()],
        "requirement 下 proposal 的下游按表序"
    );
    assert_eq!(
        dependents(requirement, "implement"),
        vec![
            "test-gen".to_owned(),
            "test-execution".to_owned(),
            "code-review".to_owned(),
            "acceptance".to_owned(),
        ],
        "requirement 下 implement 的下游按表序"
    );
    assert!(
        dependents(requirement, "archive").is_empty(),
        "archive 无下游（末相位不在任何相位前置集内）"
    );

    // test-only：code-analyze 下游 = test-design（其余相位前置不含 code-analyze）
    assert_eq!(
        dependents(test_only, "proposal"),
        vec!["code-analyze".to_owned(), "test-design".to_owned()],
        "test-only 下 proposal 的下游"
    );
    assert_eq!(
        dependents(test_only, "code-analyze"),
        vec!["test-design".to_owned()],
        "test-only 下 code-analyze 的下游仅 test-design"
    );
    assert_eq!(
        dependents(test_only, "test-gen"),
        vec!["test-execution".to_owned()],
        "test-only 下 test-gen 的下游仅 test-execution"
    );
    // 表外相位（requirement 独有 / 未知）→ 空集（不臆测）
    assert!(dependents(test_only, "implement").is_empty(), "test-only 表不含 implement");
    assert!(dependents(test_only, "dev-design").is_empty());
}

/// `dependents` 边界（异常）：未知 phase / 空 phase / 空表 → 空 Vec，不 panic。
#[test]
fn dependents未知相位与空表返回空vec() {
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");

    for table in [requirement, test_only] {
        assert!(dependents(table, "不存在的相位").is_empty());
        assert!(dependents(table, "").is_empty());
        assert!(dependents(table, " ").is_empty());
    }
    assert!(dependents(&[], "proposal").is_empty(), "空表零下游");
}

/// 回溯白名单随表分派（正向）：两表下 `allowed_backtrack_phases` 各按本表表序
/// 取前缀集（test-only 下 code-analyze 回溯含 proposal、test-design 含
/// proposal + code-analyze）。
#[test]
fn 回溯白名单随表分派两表各按本表序() {
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");

    let expect = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();

    assert_eq!(
        allowed_backtrack_phases(test_only, "code-analyze"),
        expect(&["proposal", "code-analyze"]),
        "test-only code-analyze 回溯含 proposal（第 2 相位）"
    );
    assert_eq!(
        allowed_backtrack_phases(test_only, "test-design"),
        expect(&["proposal", "code-analyze", "test-design"]),
        "test-only test-design 回溯含 proposal + code-analyze"
    );
    assert_eq!(
        allowed_backtrack_phases(test_only, "archive"),
        expect(&TEST_ONLY_PHASE_ORDER),
        "test-only archive 覆盖全表（6 相位）"
    );
    assert!(
        allowed_backtrack_phases(test_only, "proposal").is_empty(),
        "test-only 首相位无前置可回溯"
    );
    assert!(
        !allowed_backtrack_phases(test_only, "test-gen").iter().any(|phase| phase == "implement"),
        "test-only 白名单不含表外相位 implement"
    );

    // requirement 表同语义（对照锚，防两表串味）
    assert_eq!(
        allowed_backtrack_phases(requirement, "acceptance"),
        expect(&REQUIREMENT_PHASE_ORDER[..8]),
    );
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
        ("archive", None, None),
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
    // 末相位（archive）覆盖全表；acceptance 覆盖其前 8 相位（archive 不为前置）
    assert_eq!(
        allowed_backtrack_phases(table, "archive"),
        REQUIREMENT_PHASE_ORDER
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        allowed_backtrack_phases(table, "acceptance"),
        REQUIREMENT_PHASE_ORDER[..8]
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
        "acceptance 前置集不含 archive（末相位不在任何相位前置集内）"
    );
    assert!(
        !allowed_backtrack_phases(table, "acceptance")
            .iter()
            .any(|phase| phase == "archive"),
        "archive 仅前向：不在 acceptance 回溯白名单"
    );
}

#[test]
fn 白名单未知相位返回空vec不臆测() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // current_phase 不在表中 → 空 Vec（不崩、不臆测）
    assert!(allowed_backtrack_phases(table, "不存在的相位").is_empty());
    assert!(allowed_backtrack_phases(table, "").is_empty());
}

// ---------------------------------------------------------------------------
// 计划图单源：steps / artifact_manifest 声明（AC-1 / AC-2 / AC-7）
// ---------------------------------------------------------------------------

/// 计划步序列期望表（design 数据模型表逐相位声明；`OnFail` 三态按步绑定）。
fn expected_steps(phase: &str) -> Vec<StepNode> {
    match phase {
        "proposal" | "dev-design" | "test-design" | "code-analyze" => vec![
            StepNode::PhaseStart,
            StepNode::Executor,
            StepNode::Evaluator,
            StepNode::VerdictGate,
            StepNode::PhaseLog,
        ],
        "implement" | "test-gen" => vec![
            StepNode::PhaseStart,
            StepNode::Executor,
            StepNode::StaticCheck,
            StepNode::Evaluator,
            StepNode::VerdictGate,
            StepNode::PhaseLog,
        ],
        "test-execution" => vec![
            StepNode::PhaseStart,
            StepNode::TestExecution,
            StepNode::PhaseLog,
        ],
        "code-review" | "acceptance" => vec![
            StepNode::PhaseStart,
            StepNode::Evaluator,
            StepNode::VerdictGate,
            StepNode::PhaseLog,
        ],
        "archive" => ARCHIVE_STEP_ORDER.to_vec(),
        other => panic!("对照表缺相位 {other}"),
    }
}

/// 相位 → `on_fail` 声明期望（三门禁步 FeedbackLoop{5}——static-check /
/// test-execution / verdict-gate；其余 None）。
fn expected_on_fail(node: StepNode) -> Option<OnFail> {
    match node {
        StepNode::StaticCheck | StepNode::TestExecution | StepNode::VerdictGate => {
            Some(OnFail::FeedbackLoop { limit: 5 })
        }
        _ => None,
    }
}

/// 计划图单源 steps 声明：`phase_table("requirement")` 返回 9 相位（8 路由相位 +
/// archive 确定性相位），逐相位 `steps` 序列与设计数据模型表一致（门禁步由 step
/// 声明、相位限定，AC-1 / AC-2）。
#[test]
fn 计划图steps声明逐相位与设计表一致() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    assert_eq!(table.len(), 9, "requirement 九相位（含 archive）");

    for def in table {
        let actual: Vec<StepNode> = def.steps.iter().map(|step| step.node).collect();
        assert_eq!(
            actual,
            expected_steps(def.id),
            "相位 {} 的 steps 序列应与设计表一致",
            def.id
        );
        // steps 序列有序且非空（每相位至少 PhaseStart、末位 PhaseLog）
        assert_eq!(
            def.steps.first().map(|step| step.node),
            Some(StepNode::PhaseStart),
            "相位 {} 首步恒 PhaseStart",
            def.id
        );
        assert_eq!(
            def.steps.last().map(|step| step.node),
            Some(StepNode::PhaseLog),
            "相位 {} 末步恒 PhaseLog",
            def.id
        );
    }
}

/// 步声明边界：8 相位均无空 steps；VerdictGate 仅出现于「有 evaluator 步」的
/// 相位；code-review / acceptance 无 StaticCheck / TestExecution（门禁由 step
/// 声明、相位限定——walker 零相位特判的声明面锚，AC-2）。
#[test]
fn 计划图steps边界_门禁相位限定() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    for def in table {
        assert!(!def.steps.is_empty(), "相位 {} steps 非空", def.id);
        let has_evaluator = def
            .steps
            .iter()
            .any(|step| step.node == StepNode::Evaluator);
        let has_verdict_gate = def
            .steps
            .iter()
            .any(|step| step.node == StepNode::VerdictGate);
        assert_eq!(
            has_verdict_gate, has_evaluator,
            "相位 {} 的 VerdictGate 与 evaluator 步同进退",
            def.id
        );
        // 检查步与 executor / 评估面角色的绑定：仅 implement / test-gen /
        // test-execution 含门禁检查步
        let has_check = def
            .steps
            .iter()
            .any(|step| matches!(step.node, StepNode::StaticCheck | StepNode::TestExecution));
        let expect_check = matches!(def.id, "implement" | "test-gen" | "test-execution");
        assert_eq!(has_check, expect_check, "相位 {} 门禁步声明面", def.id);
    }

    // code-review / acceptance：无 executor 步、无检查步（评估-only 相位）
    for id in ["code-review", "acceptance"] {
        let def = table
            .iter()
            .find(|def| def.id == id)
            .unwrap_or_else(|| panic!("相位 {id} 应在表内"));
        assert!(
            !def.steps.iter().any(|step| step.node == StepNode::Executor),
            "评估-only 相位 {id} 不含 Executor 步"
        );
        assert!(
            !def.steps
                .iter()
                .any(|step| matches!(step.node, StepNode::StaticCheck | StepNode::TestExecution)),
            "评估-only 相位 {id} 不含检查门禁步"
        );
    }
}

/// 计划图单源 artifact_manifest 声明：proposal 含 `markdown-doc·proposal.md` /
/// `markdown-doc·specs/**`；dev-design 含 `markdown-doc·design.md` /
/// `tasks-progress·tasks.md`；test-execution 含 `markdown-doc·test-reports/**`
///（AC-1 / AC-7 归属权威源）。
#[test]
fn 计划图产物清单声明逐相位与设计表一致() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    let expect_manifest = |phase: &str| -> Vec<(&str, &str)> {
        match phase {
            "proposal" => vec![
                ("markdown-doc", "proposal.md"),
                ("markdown-doc", "specs/**"),
            ],
            "dev-design" => vec![
                ("markdown-doc", "design.md"),
                ("tasks-progress", "tasks.md"),
            ],
            "test-execution" => vec![("markdown-doc", "test-reports/**")],
            _ => Vec::new(),
        }
    };

    for def in table {
        let actual: Vec<(&str, &str)> = def
            .artifact_manifest
            .iter()
            .map(|spec| (spec.kind, spec.path))
            .collect();
        assert_eq!(
            actual,
            expect_manifest(def.id),
            "相位 {} 产物清单应与设计表一致",
            def.id
        );
        for spec in def.artifact_manifest {
            assert!(!spec.title.is_empty(), "清单标题非空（{}）", spec.path);
        }
    }

    // 无产物相位（implement / test-gen / code-review / acceptance / archive 等）
    // → 空数组（清单零占位不虚设；清单为唯一候选来源）
    for id in [
        "test-design",
        "implement",
        "test-gen",
        "code-review",
        "acceptance",
        "archive",
    ] {
        let def = table
            .iter()
            .find(|def| def.id == id)
            .unwrap_or_else(|| panic!("相位 {id} 应在表内"));
        assert!(
            def.artifact_manifest.is_empty(),
            "相位 {id} 清单应为空数组（不虚设占位项）"
        );
    }
}

/// 清单项边界：各 `path` 均为相对 POSIX 路径模式且无越界（绝对路径 / `../`
/// 前缀 / 空 `path` 均不出现）；kind 与清单绑定一致（kind 由清单声明，插件不
/// 靠扩展名 / 文件名猜测，AC-7 / R6 单源去重）。
#[test]
fn 计划图产物清单路径面与kind绑定() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    for def in table {
        for spec in def.artifact_manifest {
            assert!(!spec.path.is_empty(), "清单 path 非空：{}", def.id);
            assert!(
                !spec.path.starts_with('/') && !spec.path.starts_with('\\'),
                "清单 path 恒相对：{}",
                spec.path
            );
            assert!(!spec.path.contains('\\'), "清单 path 为 POSIX 形态");
            assert!(
                !spec.path.contains(':') && !spec.path.starts_with(".."),
                "清单 path 无盘符 / 无越界前缀：{}",
                spec.path
            );
            let glob = spec.path.strip_suffix("/**");
            if let Some(prefix) = glob {
                assert!(!prefix.is_empty(), "前缀 glob 的前缀非空：{}", spec.path);
                assert!(!prefix.contains('*'), "前缀 glob 仅后缀通配：{}", spec.path);
            }
            assert!(
                !spec.kind.is_empty(),
                "清单 kind 非空（清单为 kind 唯一来源）：{}",
                spec.path
            );
        }
        // proposal 清单不声明 tasks-progress kind（跨相位 kind 不串扰）
        if def.id == "proposal" {
            assert!(
                !def.artifact_manifest
                    .iter()
                    .any(|spec| spec.kind == "tasks-progress"),
                "proposal 清单不声明 tasks-progress kind"
            );
        }
    }
}

/// prompt 静态化：全部有静态 prompt 的 agent 步（8 路由相位）`prompt_ref` 均为
/// `&'static str` 且非空；archive 确定性相位的 spec_sync / resolve_conflicts 为
/// agent 步但 `prompt_ref` 恒 `None`（prompt 由 orchestration 运行时组装——workflow
/// 零 orchestration 依赖）；全表扫描无 `<role>` 间接读取 / `{{…}}` 占位（零占位
/// 红线，AC-1 / AC-2）。
#[test]
fn 计划图prompt静态非空且零占位() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    let mut agent_step_prompts = 0usize;
    for def in table {
        for step in def.steps {
            match step.node.role() {
                Some(_) if def.id == "archive" => {
                    // 归档两会话步的运行时空组装面：workflow 侧恒 None（AC-2）
                    assert_eq!(
                        step.prompt_ref, None,
                        "归档相位 agent 步 prompt_ref 恒 None（运行时组装）"
                    );
                }
                Some(_) => {
                    let prompt = step
                        .prompt_ref
                        .unwrap_or_else(|| panic!("相位 {} agent 步应有静态 prompt", def.id));
                    let _: &'static str = prompt;
                    assert!(
                        !prompt.is_empty(),
                        "相位 {} agent 步 prompt 静态非空",
                        def.id
                    );
                    assert!(
                        !prompt.contains("<role>") && !prompt.contains("{{"),
                        "相位 {} prompt 零占位（无 <role> / {{{{…}}}}）",
                        def.id
                    );
                    agent_step_prompts += 1;
                }
                None => {
                    // 非 agent 步（PhaseStart / 检查步 / PhaseLog / 三门）恒 None
                    assert_eq!(
                        step.prompt_ref, None,
                        "相位 {} 非 agent 步 prompt_ref 恒 None",
                        def.id
                    );
                }
            }
        }
        // agent 步 prompt 与角色 spec prompt 同源（防两处字面路径漂移）
        if let Some(spec) = &def.executor {
            let executor_step = def
                .steps
                .iter()
                .find(|step| step.node == StepNode::Executor);
            if let Some(step) = executor_step {
                assert_eq!(
                    step.prompt_ref,
                    Some(spec.prompt),
                    "相位 {} executor 步与 spec prompt 同源",
                    def.id
                );
            }
        }
        if let Some(spec) = &def.evaluator {
            let evaluator_step = def
                .steps
                .iter()
                .find(|step| step.node == StepNode::Evaluator);
            if let Some(step) = evaluator_step {
                assert_eq!(
                    step.prompt_ref,
                    Some(spec.prompt),
                    "相位 {} evaluator 步与 spec prompt 同源",
                    def.id
                );
            }
        }
    }
    // 5 相位 executor 步 + 5 相位 evaluator 步 + code-review / acceptance 两条
    // 评估-only evaluator 步 = 12 条 agent 步（test-execution / archive 相位无静态
    // prompt agent 步）
    assert_eq!(agent_step_prompts, 12, "12 条 agent 步 prompt 齐备");
}

#[test]
fn 计划图on_fail语义与步绑定() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    for def in table {
        for step in def.steps {
            assert_eq!(
                step.on_fail,
                expected_on_fail(step.node),
                "相位 {} 步 {:?} 的 on_fail 声明",
                def.id,
                step.node
            );
            if matches!(step.on_fail, Some(OnFail::FeedbackLoop { .. })) {
                let allowed = (matches!(def.id, "implement" | "test-gen")
                    && step.node == StepNode::StaticCheck)
                    || (def.id == "test-execution" && step.node == StepNode::TestExecution)
                    || step.node == StepNode::VerdictGate;
                assert!(
                    allowed,
                    "相位 {} 步 {:?} 不得携 FeedbackLoop",
                    def.id, step.node
                );
            }
            // 负向面：executor / evaluator 步不得携 Terminal
            if step.node.role().is_some() {
                assert!(
                    !matches!(step.on_fail, Some(OnFail::Terminal)),
                    "相位 {} agent 步 {:?} 不得携 Terminal",
                    def.id,
                    step.node
                );
            }
        }
    }

    // FeedbackLoop 计数上限恒为 5（u32 边界语义面；limit 逐值可辨由 state_test 承载）
    for def in table {
        for step in def.steps {
            if let Some(OnFail::FeedbackLoop { limit }) = step.on_fail {
                assert_eq!(limit, 5, "反馈边独立计数上限为 5");
                assert!(limit > 0, "上限为正（零上限反馈无意义）");
            }
        }
    }
}

/// 类型收敛：`PhaseDefinition` 穷尽解构含 `steps` / `artifact_manifest` 新字段
///（无 `..` 兜底——字段增删即编译失败）；`StepDefinition` 穷尽解构三字段；
/// 表项逐字段与本地期望表等值（防 const 表漂移，AC-1）。
#[test]
fn 计划图类型收敛与const表逐项等值() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    for def in table {
        let PhaseDefinition {
            id,
            description,
            steps,
            artifact_manifest,
            prerequisites,
            executor,
            evaluator,
        } = def;
        assert_eq!(*id, def.id, "id 字段解构等值");
        assert!(!description.is_empty(), "相位 {id} description 非空");
        // 前置依赖表内自描述（字段类型 `&'static [&'static str]`；无独立 match 兜底）
        let _: &&'static [&'static str] = prerequisites;
        assert_eq!(
            *prerequisites,
            def.prerequisites,
            "prerequisites 字段解构等值（表内自描述单源）"
        );
        assert_eq!(
            steps.len(),
            expected_steps(id).len(),
            "steps 长度与期望表一致"
        );
        assert_eq!(
            artifact_manifest
                .iter()
                .map(|spec| (spec.kind, spec.path))
                .collect::<Vec<_>>(),
            def.artifact_manifest
                .iter()
                .map(|spec| (spec.kind, spec.path))
                .collect::<Vec<_>>(),
            "清单逐项等值（防 const 表漂移）"
        );
        let StepDefinition {
            node: _,
            prompt_ref: _,
            on_fail: _,
        } = steps[0];
        // 角色齐备性回归：executor / evaluator 与相位角色绑定一致
        let has_executor_step = steps.iter().any(|step| step.node == StepNode::Executor);
        if has_executor_step {
            assert!(
                executor.is_some(),
                "相位 {id} 含 Executor 步则 executor spec 齐备"
            );
        }
        let has_evaluator_step = steps.iter().any(|step| step.node == StepNode::Evaluator);
        if has_evaluator_step {
            assert!(
                evaluator.is_some(),
                "相位 {id} 含 Evaluator 步则 evaluator spec 齐备"
            );
        }
        // test-execution 相位无 agent 步但仍声明双角色 spec（编排身份面保留）
        if *id == "test-execution" {
            assert!(executor.is_some(), "test-execution 保留 executor spec");
            assert!(!has_executor_step && !has_evaluator_step);
        }
    }
    // 全表 8 相位：executor 6 / evaluator 8（评估-only 两相位无 executor）
    assert_eq!(table.iter().filter(|def| def.executor.is_some()).count(), 6);
    assert_eq!(
        table.iter().filter(|def| def.evaluator.is_some()).count(),
        8
    );
}

/// 计划步声明的线面镜像（`StepDefinition` 本体为 `&'static str` / const 表静态
/// 载体，无 serde 派生——往返以同词汇面镜像承载：`node` / `on_fail` 直用封闭
/// 集类型，`prompt_ref` 以静态文本内容出线）。
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct StepDefinitionWire {
    node: StepNode,
    prompt_ref: Option<String>,
    on_fail: Option<OnFail>,
}

/// 产物清单项的线面镜像（`ArtifactSpec` 三字段逐项出线）。
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ArtifactSpecWire {
    kind: String,
    path: String,
    title: String,
}

/// 序列化往返逐字段可辨：相位表 `steps` / `artifact_manifest` 经线面镜像序列
/// 化 → 反序列化后与 const 表逐项等值（往返不失真即防 const 表漂移；步词
/// snake_case 线面与落库词汇同源，AC-1）。
#[test]
fn 计划图类型收敛_序列化往返与const表逐项等值() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    for def in table {
        // 序列化半边：const 表 → 线面镜像（节点词汇 / on_fail / prompt 内容）
        let steps: Vec<StepDefinitionWire> = def
            .steps
            .iter()
            .map(|step| StepDefinitionWire {
                node: step.node,
                prompt_ref: step.prompt_ref.map(str::to_owned),
                on_fail: step.on_fail,
            })
            .collect();
        let manifest: Vec<ArtifactSpecWire> = def
            .artifact_manifest
            .iter()
            .map(|spec| ArtifactSpecWire {
                kind: spec.kind.to_owned(),
                path: spec.path.to_owned(),
                title: spec.title.to_owned(),
            })
            .collect();
        let steps_wire = serde_json::to_string(&steps).expect("steps 出线应成功");
        let manifest_wire = serde_json::to_string(&manifest).expect("清单出线应成功");

        // 反序列化半边：线面 → 与 const 表逐项等值（同序同长同字段）
        let steps_back: Vec<StepDefinitionWire> =
            serde_json::from_str(&steps_wire).expect("steps 往返应成功");
        let manifest_back: Vec<ArtifactSpecWire> =
            serde_json::from_str(&manifest_wire).expect("清单往返应成功");
        assert_eq!(
            steps_back, steps,
            "相位 {} steps 序列化往返逐项等值",
            def.id
        );
        assert_eq!(
            manifest_back, manifest,
            "相位 {} artifact_manifest 序列化往返逐项等值",
            def.id
        );
        assert_eq!(
            steps_back.len(),
            expected_steps(def.id).len(),
            "相位 {} 往返后步数与计划图一致",
            def.id
        );
        assert_eq!(
            steps_back.iter().map(|step| step.node).collect::<Vec<_>>(),
            expected_steps(def.id),
            "相位 {} 往返后步序列与计划图逐项一致",
            def.id
        );

        // 词面往返不失真：节点逐字为 snake_case 线词（与执行图落库词汇同源）
        for (step, line) in steps_back.iter().zip(&steps) {
            let word = serde_json::to_value(line.node).expect("节点出线应成功");
            assert_eq!(
                word,
                serde_json::json!(step.node.as_str()),
                "相位 {} 节点线词逐字往返（snake_case 词汇单点）",
                def.id
            );
        }

        // 清单项三字段逐项等值（kind 与 path 绑定一致；相对路径无越界、title
        // 非空不虚设——清单为唯一候选来源）
        assert_eq!(
            manifest_back
                .iter()
                .map(|spec| (spec.kind.as_str(), spec.path.as_str(), spec.title.as_str()))
                .collect::<Vec<_>>(),
            def.artifact_manifest
                .iter()
                .map(|spec| (spec.kind, spec.path, spec.title))
                .collect::<Vec<_>>(),
            "相位 {} 清单项三字段往返逐项等值（const 表为唯一事实源）",
            def.id
        );
        for spec in &manifest_back {
            assert!(!spec.title.is_empty(), "相位 {} 清单项人读标题非空", def.id);
            assert!(
                !spec.path.starts_with('/') && !spec.path.starts_with("../"),
                "相位 {} 清单相对路径无越界（绝对路径 / `../` 零形态）: {}",
                def.id,
                spec.path
            );
        }
    }
}

/// `allowed_backtrack_phases` 边界：空表（`&[]`）+ 任意 current_phase → 空 Vec
///（无表不臆测，白名单语义零改动锚）。
#[test]
fn 白名单空表返回空vec() {
    assert!(allowed_backtrack_phases(&[], "proposal").is_empty());
    assert!(allowed_backtrack_phases(&[], "acceptance").is_empty());
    assert!(allowed_backtrack_phases(&[], "archive").is_empty());
    assert!(allowed_backtrack_phases(&[], "").is_empty());
    assert!(allowed_backtrack_phases(&[], "不存在的相位").is_empty());
}

// ---------------------------------------------------------------------------
// 归档相位声明（AC-1 / AC-2 / AC-8）
// ---------------------------------------------------------------------------

/// archive 相位声明（正向）：`phase_table("requirement")` 末位为 archive 确定性
/// 相位——`executor=None` / `evaluator=None`（无 agent 角色 spec）、
/// `artifact_manifest` 空数组、description 非空（AC-1）。
#[test]
fn archive相位声明_末位确定性相位无角色spec零清单() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    let archive = table.last().expect("末位相位应在场");
    assert_eq!(archive.id, "archive", "archive 为末位（第 9）路由相位");
    assert_eq!(
        table.iter().position(|def| def.id == "archive"),
        Some(table.len() - 1),
        "archive 相位序号 = 表尾"
    );
    assert_eq!(archive.id, "archive");
    assert!(
        !archive.description.is_empty(),
        "archive description 非空（人读相位说明）"
    );
    assert!(
        archive.executor.is_none(),
        "archive executor 恒 None（确定性相位）"
    );
    assert!(
        archive.evaluator.is_none(),
        "archive evaluator 恒 None（空 checklist 落账替代评估证据）"
    );
    assert!(
        archive.artifact_manifest.is_empty(),
        "archive 零产物清单（空数组，不虚设占位项）"
    );
}

/// archive 步序（正向）：archive `steps` 恰 10 步且与 design 数据模型表逐位一致
///（PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge →
/// SpecSync → Seal → Finalize → PhaseLog），首步 PhaseStart、末步 PhaseLog
///（AC-2）。
#[test]
fn archive相位十步序列与设计表一致() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let archive = table
        .iter()
        .find(|def| def.id == "archive")
        .expect("archive 相位应在表内");

    let actual: Vec<StepNode> = archive.steps.iter().map(|step| step.node).collect();
    assert_eq!(actual, ARCHIVE_STEP_ORDER.to_vec(), "archive 十步序列逐位一致");
    assert_eq!(actual.len(), 10, "archive 步数恰 10");
    assert_eq!(
        archive.steps.first().map(|step| step.node),
        Some(StepNode::PhaseStart),
        "archive 首步恒 PhaseStart"
    );
    assert_eq!(
        archive.steps.last().map(|step| step.node),
        Some(StepNode::PhaseLog),
        "archive 末步恒 PhaseLog"
    );
    // 十步逐位无重复（关键步各恰一次）
    for node in [
        StepNode::Preflight,
        StepNode::Commit,
        StepNode::Rebase,
        StepNode::ResolveConflicts,
        StepNode::Merge,
        StepNode::SpecSync,
        StepNode::Seal,
        StepNode::Finalize,
    ] {
        assert_eq!(
            actual.iter().filter(|candidate| **candidate == node).count(),
            1,
            "{node:?} 在 archive 步序中恰出现一次"
        );
    }
}

/// archive 步 `on_fail` 语义（正向）：十步 `on_fail` 恒 `None`——步失败即
/// Terminal 收口，不唤决策 agent、不进相位重试（AC-8 失败停链）。
#[test]
fn archive步on_fail恒none_失败即终态() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let archive = table
        .iter()
        .find(|def| def.id == "archive")
        .expect("archive 相位应在表内");

    assert_eq!(archive.steps.len(), 10);
    for step in archive.steps {
        assert_eq!(
            step.on_fail, None,
            "archive 步 {:?} on_fail 恒 None（失败停链，无反馈边 / 无决策分支）",
            step.node
        );
        assert!(
            !matches!(step.on_fail, Some(OnFail::FeedbackLoop { .. })),
            "archive 步 {:?} 不得携 FeedbackLoop（归档步不回流 agent）",
            step.node
        );
        assert!(
            !matches!(step.on_fail, Some(OnFail::Terminal)),
            "archive 步 {:?} 不显式声明 Terminal（缺省即按步失败收场）",
            step.node
        );
    }
}

/// archive 步 `prompt_ref` 语义（边界）：十步 `prompt_ref` 恒 `None`——spec_sync /
/// resolve_conflicts 的 prompt 由 orchestration 运行时组装（workflow 零
/// orchestration 依赖），其余步非 agent 面亦恒 None（AC-2 / 数据模型）。
#[test]
fn archive步prompt_ref恒none_运行时空组装() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let archive = table
        .iter()
        .find(|def| def.id == "archive")
        .expect("archive 相位应在表内");

    for step in archive.steps {
        assert_eq!(
            step.prompt_ref, None,
            "archive 步 {:?} prompt_ref 恒 None（零静态 prompt 引用）",
            step.node
        );
    }
    // 归档两会话步虽属 Agent 分组，但静态 prompt 引用仍恒 None（运行时组装）
    for node in [StepNode::SpecSync, StepNode::ResolveConflicts] {
        let step = archive
            .steps
            .iter()
            .find(|step| step.node == node)
            .unwrap_or_else(|| panic!("{node:?} 应在 archive 步序内"));
        assert_eq!(step.node.group(), crate::state::StepGroup::Agent);
        assert!(step.node.role().is_some(), "归档会话步携 agent 角色");
        assert_eq!(
            step.prompt_ref, None,
            "{node:?} prompt_ref 恒 None（prompt 归 orchestration 单点）"
        );
    }
}

/// archive 步分组可派生（正向）：archive 步经 `StepNode::group()` 分派
///（commit / rebase / merge / finalize→Git、preflight / seal→Exec、
/// spec_sync / resolve_conflicts→Agent、phase_start / phase_log→Exec），
/// 与 orchestration 三分支执行缝分派前置一致（AC-2 / AC-3）。
#[test]
fn archive步分组可派生_三分支缝分派前置() {
    use crate::state::StepGroup;

    let expect_group = |node: StepNode| -> StepGroup {
        match node {
            StepNode::PhaseStart
            | StepNode::Preflight
            | StepNode::Seal
            | StepNode::PhaseLog => StepGroup::Exec,
            StepNode::Commit | StepNode::Rebase | StepNode::Merge | StepNode::Finalize => {
                StepGroup::Git
            }
            StepNode::SpecSync | StepNode::ResolveConflicts => StepGroup::Agent,
            other => panic!("archive 步序含越集节点 {other:?}"),
        }
    };

    for node in ARCHIVE_STEP_ORDER {
        assert_eq!(node.group(), expect_group(node), "{node:?} 分组派生一致");
    }
    // 四分类计数：Exec 4 / Git 4 / Agent 2（Gate 零——归档无门禁步）
    let groups: Vec<StepGroup> = ARCHIVE_STEP_ORDER.iter().map(|node| node.group()).collect();
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Exec).count(), 4);
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Git).count(), 4);
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Agent).count(), 2);
    assert_eq!(
        groups.iter().filter(|g| **g == StepGroup::Gate).count(),
        0,
        "归档零门禁步（无 verdict 预算分支）"
    );
}

/// ARCHIVE_STEPS 类型收敛（异常）：archive 十步均为 `StepNode` 十八值封闭集
/// 成员（线画 snake_case 词与执行图落库同源、不引用退役 `ArchiveStage` 词汇），
/// 长度恰 10、无越集节点（AC-2）。
#[test]
fn archive_steps类型收敛_封闭集取值且长度十() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let archive = table
        .iter()
        .find(|def| def.id == "archive")
        .expect("archive 相位应在表内");

    assert_eq!(archive.steps.len(), 10, "ARCHIVE_STEPS 长度恰 10");

    // 十步词面均在 StepNode 十八值封闭集内（逐词往返 = snake_case 线词）
    let closed_set: Vec<&'static str> = ARCHIVE_STEP_ORDER
        .iter()
        .map(|node| node.as_str())
        .collect();
    for step in archive.steps {
        let word = step.node.as_str();
        assert!(
            closed_set.contains(&word),
            "归档步词 {word:?} 应在归档十步闭集内"
        );
        // 线面往返（封闭集编码→解码无失真，越集词即败）
        let back: StepNode = serde_json::from_value(serde_json::json!(word))
            .unwrap_or_else(|_| panic!("归档步词 {word:?} 应可反序列化"));
        assert_eq!(back, step.node, "归档步词 {word:?} 往返逐词可辨");
        // 退役 ArchiveStage 词汇不在新步序内
        for retired in ["archive_stage", "stage", "prepare", "workflow", "archive"] {
            assert_ne!(word, retired, "archive 步序不沿用退役词汇 {retired:?}");
        }
    }
    // 归档步不得复用其它相位的执行图角色步（无 Executor / Evaluator / Decision /
    // 三门——归档无会话角色 spec 与门禁分支）
    for step in archive.steps {
        assert!(
            !matches!(
                step.node,
                StepNode::Executor
                    | StepNode::Evaluator
                    | StepNode::Decision
                    | StepNode::VerdictGate
                    | StepNode::RetryGate
                    | StepNode::WhitelistGate
                    | StepNode::StaticCheck
                    | StepNode::TestExecution
            ),
            "归档步 {:?} 不得为路由相位的角色 / 门禁步",
            step.node
        );
    }
}

// ---------------------------------------------------------------------------
// test-only 表（AC-1 / AC-2 / AC-3）：步序 / 清单复用 / prompt 静态面 / 分派边界
// ---------------------------------------------------------------------------

/// test-only 表产物清单（正向）：proposal 复用 `PROPOSAL_MANIFEST`（proposal.md +
/// specs/**）、test-execution 复用 `TEST_EXECUTION_MANIFEST`（test-reports/**）、
/// code-analyze 空数组（分析相位零产物）、其余无产物相位空数组。
#[test]
fn test_only表产物清单复用与code_analyze零清单() {
    let table = phase_table("test-only").expect("test-only 应返回 Some");

    let expect: &[(&str, &[(&str, &str)])] = &[
        (
            "proposal",
            &[("markdown-doc", "proposal.md"), ("markdown-doc", "specs/**")],
        ),
        ("code-analyze", &[]),
        ("test-design", &[]),
        ("test-gen", &[]),
        (
            "test-execution",
            &[("markdown-doc", "test-reports/**")],
        ),
        ("archive", &[]),
    ];

    for def in table {
        let (_, expected) = expect
            .iter()
            .find(|(id, _)| *id == def.id)
            .unwrap_or_else(|| panic!("对照表缺相位 {}", def.id));
        let actual: Vec<(&str, &str)> = def
            .artifact_manifest
            .iter()
            .map(|spec| (spec.kind, spec.path))
            .collect();
        assert_eq!(actual, expected.to_vec(), "test-only 相位 {} 产物清单", def.id);
        for spec in def.artifact_manifest {
            assert!(!spec.title.is_empty(), "清单标题非空（{}）", spec.path);
            assert!(
                !spec.path.starts_with('/') && !spec.path.starts_with(".."),
                "清单路径恒相对且无越界：{}",
                spec.path
            );
        }
    }

    // code-analyze 零清单（空数组，不虚设占位项——分析相位不铸产物）
    let code_analyze = phase_of(table, "code-analyze");
    assert!(code_analyze.artifact_manifest.is_empty(), "code-analyze 清单空数组");
}

/// test-only 表 prompt 静态面（正向）：有静态 prompt 的 agent 步恒 `&'static str`
/// 且非空零占位、步 `prompt_ref` 与角色 spec 同源；archive 两会话步 `prompt_ref`
/// 恒 `None`（运行时组装）；test-execution 无 agent 步（确定性相位）。
#[test]
fn test_only表prompt静态非空且步spec同源() {
    let table = phase_table("test-only").expect("test-only 应返回 Some");

    let mut agent_step_prompts = 0usize;
    for def in table {
        for step in def.steps {
            match step.node.role() {
                Some(_) if def.id == "archive" => {
                    assert_eq!(
                        step.prompt_ref, None,
                        "归档 agent 步 prompt_ref 恒 None（prompt 归 orchestration）"
                    );
                }
                Some(_) => {
                    let prompt = step
                        .prompt_ref
                        .unwrap_or_else(|| panic!("相位 {} agent 步应有静态 prompt", def.id));
                    let _: &'static str = prompt;
                    assert!(!prompt.is_empty(), "相位 {} agent 步 prompt 静态非空", def.id);
                    assert!(
                        !prompt.contains("<role>") && !prompt.contains("{{"),
                        "相位 {} prompt 零占位",
                        def.id
                    );
                    agent_step_prompts += 1;
                }
                None => {
                    assert_eq!(
                        step.prompt_ref, None,
                        "相位 {} 非 agent 步 prompt_ref 恒 None",
                        def.id
                    );
                }
            }
        }

        // 步 prompt 与角色 spec prompt 同源（防两处字面路径漂移）
        if let Some(spec) = &def.executor {
            if let Some(step) = def.steps.iter().find(|step| step.node == StepNode::Executor) {
                assert_eq!(step.prompt_ref, Some(spec.prompt), "相位 {} executor 同源", def.id);
            }
        }
        if let Some(spec) = &def.evaluator {
            if let Some(step) = def.steps.iter().find(|step| step.node == StepNode::Evaluator) {
                assert_eq!(step.prompt_ref, Some(spec.prompt), "相位 {} evaluator 同源", def.id);
            }
        }
    }
    // proposal / code-analyze / test-design / test-gen 各 2 条 = 8 条 agent 步
    //（test-execution 确定性相位无静态 prompt agent 步、archive 运行时组装）
    assert_eq!(agent_step_prompts, 8, "8 条 agent 步 prompt 齐备");

    // test-execution 相位无 agent 步但保留双角色 spec（编排身份面）
    let test_execution = phase_of(table, "test-execution");
    assert!(!test_execution.steps.iter().any(|step| step.node.role().is_some()));
    assert!(test_execution.executor.is_some() && test_execution.evaluator.is_some());
}

/// test-only 表 steps / on_fail 全表扫描（正向 / 边界）：逐相位步序与本表期望
/// 一致、首步恒 PhaseStart、末步恒 PhaseLog、VerdictGate 与 Evaluator 步同进退、
/// on_fail 三态与步绑定（门禁反馈步仅 test-gen 的 StaticCheck 与 test-execution 的
/// TestExecution）。
#[test]
fn test_only表steps与on_fail全表扫描() {
    let table = phase_table("test-only").expect("test-only 应返回 Some");
    assert_eq!(table.len(), 6);

    for def in table {
        let actual: Vec<StepNode> = def.steps.iter().map(|step| step.node).collect();
        assert_eq!(actual, expected_steps(def.id), "test-only 相位 {} 步序", def.id);
        assert!(!def.steps.is_empty(), "相位 {} steps 非空", def.id);
        assert_eq!(
            def.steps.first().map(|step| step.node),
            Some(StepNode::PhaseStart),
            "相位 {} 首步恒 PhaseStart",
            def.id
        );
        assert_eq!(
            def.steps.last().map(|step| step.node),
            Some(StepNode::PhaseLog),
            "相位 {} 末步恒 PhaseLog",
            def.id
        );
        let has_evaluator = def.steps.iter().any(|step| step.node == StepNode::Evaluator);
        let has_verdict = def.steps.iter().any(|step| step.node == StepNode::VerdictGate);
        assert_eq!(has_verdict, has_evaluator, "相位 {} VerdictGate 与 Evaluator 同进退", def.id);

        for step in def.steps {
            assert_eq!(
                step.on_fail,
                expected_on_fail(step.node),
                "相位 {} 步 {:?} on_fail 声明",
                def.id,
                step.node
            );
            if matches!(step.on_fail, Some(OnFail::FeedbackLoop { .. })) {
                let allowed = (def.id == "test-gen" && step.node == StepNode::StaticCheck)
                    || (def.id == "test-execution" && step.node == StepNode::TestExecution);
                assert!(
                    allowed,
                    "test-only 相位 {} 步 {:?} 不得携 FeedbackLoop",
                    def.id, step.node
                );
            }
        }
    }
}

/// 分派边界（异常 / 负向）：未知类型不落任一表——`phase_table` 返回 `None` 而非
/// 退化 requirement（写面 / 详情读面显式拒绝的依据）。
#[test]
fn phase_table未知类型不退化requirement() {
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");
    assert_ne!(
        requirement.as_ptr(),
        test_only.as_ptr(),
        "两表为不同静态表（两值分派不共用同一份表身）"
    );

    for unknown in ["bug-fix", "Bug-Fix", "", " ", "\t", "requirement-s", " test-only2"] {
        assert!(
            phase_table(unknown).is_none(),
            "未知类型 {unknown:?} 恒 None（MUST NOT 静默当 requirement 跑）"
        );
    }
}
