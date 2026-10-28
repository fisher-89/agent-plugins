use agent::ModelLevel;
use serde::{Deserialize, Serialize};

use super::phase_table::{
    allowed_backtrack_phases, backtrack_options, dependents, phase_table, PhaseAgentSpec,
    PhaseDefinition, MAX_RETRY_TIMES,
};
use crate::state::{OnFail, StepDefinition, StepNode};
use crate::write::INITIALIZE_PHASE_ID;

/// requirement 相位表表序（相位机语义锚；prompt 静态化不触动相位序；initialize
/// 一次性表头——check-before-flow-start 10 相位扩位）。
const REQUIREMENT_PHASE_ORDER: [&str; 10] = [
    "initialize",
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

/// test-only 相位表表序（7 相位：initialize 一次性表头 + 5 个 agent 相位 +
/// archive 确定性相位——check-before-flow-start 扩位）。
const TEST_ONLY_PHASE_ORDER: [&str; 7] = [
    "initialize",
    "proposal",
    "code-analyze",
    "test-design",
    "test-gen",
    "test-execution",
    "archive",
];

/// initialize 相位步序（两表表头共用同一 static；一次性基线检查相位——
/// `PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog` 五步序，两检查
/// 步 `on_fail=Terminal`——check-before-flow-start D5/D7）。
const INITIALIZE_STEP_ORDER: [StepNode; 5] = [
    StepNode::PhaseStart,
    StepNode::Clean,
    StepNode::StaticCheck,
    StepNode::TestExecution,
    StepNode::PhaseLog,
];

/// code-analyze 相位步序（P→E 分析相位；AC-2）。
const CODE_ANALYZE_STEP_ORDER: [StepNode; 5] = [
    StepNode::PhaseStart,
    StepNode::Executor,
    StepNode::Evaluator,
    StepNode::VerdictGate,
    StepNode::PhaseLog,
];

/// 归档相位步序（design 数据模型表；AC-2 / AC-4 十一步链序——Cleanup 介于
/// SpecSync 与 Seal）。
const ARCHIVE_STEP_ORDER: [StepNode; 11] = [
    StepNode::PhaseStart,
    StepNode::Preflight,
    StepNode::Commit,
    StepNode::Rebase,
    StepNode::ResolveConflicts,
    StepNode::Merge,
    StepNode::SpecSync,
    StepNode::Cleanup,
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
fn phase_table_test_only返回七相位且序稳定() {
    let table = phase_table("test-only").expect("test-only 应返回 Some");

    assert_eq!(
        table.len(),
        7,
        "test-only 七相位（initialize 表头 + 含 archive）"
    );
    let ids: Vec<&str> = table.iter().map(|def| def.id).collect();
    assert_eq!(ids, TEST_ONLY_PHASE_ORDER.to_vec(), "test-only 相位序稳定");

    // 首位 initialize 表头（一次性基线检查相位——check-before-flow-start）
    assert_eq!(
        table.first().expect("首位相位应在场").id,
        "initialize",
        "initialize 为表头（第 1）相位"
    );

    // 末位 archive 确定性相位（无 executor / evaluator、零清单）
    let archive = table.last().expect("末位相位应在场");
    assert_eq!(archive.id, "archive", "archive 为末位（第 7）相位");
    assert!(
        archive.executor.is_none(),
        "test-only archive executor 恒 None"
    );
    assert!(
        archive.evaluator.is_none(),
        "test-only archive evaluator 恒 None"
    );
    assert!(
        archive.artifact_manifest.is_empty(),
        "test-only archive 零清单"
    );

    // requirement 独有相位不在 test-only 表内（两表分派不串味）
    for absent in ["dev-design", "implement", "code-review", "acceptance"] {
        assert!(
            !ids.contains(&absent),
            "test-only 表不含 requirement 独有相位 {absent}"
        );
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
            lhs.description, rhs.description,
            "相位 {id} 复用 requirement 同名 description"
        );
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
    let archive_steps: Vec<StepNode> = test_only_archive
        .steps
        .iter()
        .map(|step| step.node)
        .collect();
    assert_eq!(
        archive_steps,
        ARCHIVE_STEP_ORDER.to_vec(),
        "test-only archive 复用 ARCHIVE_STEPS"
    );

    // code-analyze 用 CODE_ANALYZE_STEPS（P→E 五步形态）
    let code_analyze = phase_of(test_only, "code-analyze");
    let code_analyze_steps: Vec<StepNode> =
        code_analyze.steps.iter().map(|step| step.node).collect();
    assert_eq!(
        code_analyze_steps,
        CODE_ANALYZE_STEP_ORDER.to_vec(),
        "code-analyze 步序 = [PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]"
    );
}

/// code-analyze 相位声明（正向 / AC-2）：步序 P→E 五步、VerdictGate
/// `on_fail=Terminal`、executor / evaluator 均 `ModelLevel::High`、产物清单声明
/// `markdown-doc·design.md`（源码分析文档）、两常量静态非空零占位。
#[test]
fn code_analyze相位声明_p形态与design清单与双高角色() {
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
    assert_eq!(
        verdict.on_fail,
        Some(OnFail::Terminal),
        "VerdictGate 失败即终态"
    );
    for step in code_analyze.steps {
        if step.node != StepNode::VerdictGate {
            assert_eq!(
                step.on_fail, None,
                "步 {:?} 无失败语义（缺省 Terminal）",
                step.node
            );
        }
    }

    // 双角色均 High 档、prompt 静态非空零占位（`include_str!` 编译期装配）
    let executor = code_analyze
        .executor
        .as_ref()
        .expect("code-analyze 应有 executor");
    let evaluator = code_analyze
        .evaluator
        .as_ref()
        .expect("code-analyze 应有 evaluator");
    assert_eq!(
        executor.model_level,
        ModelLevel::High,
        "code-analyze executor 档位"
    );
    assert_eq!(
        evaluator.model_level,
        ModelLevel::High,
        "code-analyze evaluator 档位"
    );
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

    // 产物清单声明 design.md（分析相位铸源码分析文档，清单外文档零入列）
    let manifest: Vec<(&str, &str)> = code_analyze
        .artifact_manifest
        .iter()
        .map(|spec| (spec.kind, spec.path))
        .collect();
    assert_eq!(
        manifest,
        vec![("markdown-doc", "design.md")],
        "code-analyze 清单声明源码分析 design.md"
    );
    assert!(
        code_analyze.artifact_manifest[0].title == "源码分析",
        "清单标题随声明（源码分析）"
    );
}

/// 空白 / 大小写边界（边界）：`phase_table` 先 trim 再 `eq_ignore_ascii_case`
/// 命中——首尾空白与任意大小写不误判、不退化。
#[test]
fn phase_table_空白与大小写规范化命中() {
    let requirement = phase_table("requirement").expect("requirement 命中");
    let test_only = phase_table("test-only").expect("test-only 命中");

    // 大小写不敏感
    assert_eq!(
        phase_table("REQUIREMENT").map(|t| t.len()),
        Some(requirement.len())
    );
    assert_eq!(
        phase_table("Test-Only").map(|t| t.len()),
        Some(test_only.len())
    );
    assert_eq!(
        phase_table("test-ONLY").map(|t| t.len()),
        Some(test_only.len())
    );
    // 首尾空白 trim 后命中（" test-only " 不因空白误判为未知）
    assert_eq!(
        phase_table(" requirement ").map(|t| t.len()),
        Some(requirement.len())
    );
    assert_eq!(
        phase_table(" test-only ").map(|t| t.len()),
        Some(test_only.len())
    );
    assert_eq!(
        phase_table("\ttest-only\n").map(|t| t.len()),
        Some(test_only.len())
    );
    // 表序 / 表身同源（返回同一静态表）
    assert_eq!(
        phase_table("TEST-ONLY").map(|t| t.as_ptr()),
        Some(test_only.as_ptr())
    );
    // 空白+大小写混排仍不越集
    assert!(phase_table(" BUG-FIX ").is_none());
}

/// 表内自描述前置依赖（边界）：test-only 表 `prerequisites` 逐相位照表声明
///（initialize=[]、proposal=[]、code-analyze=[proposal]、test-design=[proposal,
/// code-analyze]、test-gen=[test-design]、test-execution=[test-gen]、archive=[]）；
/// requirement 表前置与既有 match 语义一致（集合语义对照，不依赖声明序）；任何
/// 相位前置不含 initialize（表头零入边——stale 传播 BFS 零波及的结构前提，AC-7）。
#[test]
fn 表内自描述prerequisites逐相位声明() {
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");
    let expect_test_only: &[(&str, &[&str])] = &[
        ("initialize", &[]),
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
        assert_eq!(
            def.prerequisites, *expected,
            "test-only 相位 {} 前置依赖",
            def.id
        );
    }

    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let expect_requirement: &[(&str, &[&str])] = &[
        ("initialize", &[]),
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
        assert_eq!(
            def.prerequisites.len(),
            expected.len(),
            "相位 {} 前置个数",
            def.id
        );
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
            assert!(
                ids.contains(prerequisite),
                "相位 {} 前置 {prerequisite} 应在表内",
                def.id
            );
            assert_ne!(
                *prerequisite, def.id,
                "相位 {} 前置不含自身（无自环）",
                def.id
            );
        }
    }
}

/// initialize 零入边（边界，枚举遍历两表）：两表 initialize `prerequisites == []`
/// 且任何相位 `prerequisites` 不含 initialize——`dependents("initialize")` 空集
///（stale 传播 BFS 零波及的结构前提，AC-7 半边）。
#[test]
fn initialize_prerequisites空且零入边_dependents空集() {
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");

        // initialize 自身前置空
        let initialize = phase_of(table, "initialize");
        assert!(
            initialize.prerequisites.is_empty(),
            "{workflow_type} initialize 无依赖前置（零入边）"
        );

        // 任何相位前置不含 initialize（枚举遍历）
        for def in table {
            assert!(
                !def.prerequisites.contains(&"initialize"),
                "{workflow_type} 相位 {} 前置不含 initialize（表头零入边）",
                def.id
            );
        }

        // dependents("initialize") 空集（stale 传播零波及）
        assert!(
            dependents(table, "initialize").is_empty(),
            "{workflow_type} initialize 无下游依赖（stale 传播零波及）"
        );
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
        vec![
            "dev-design".to_owned(),
            "test-design".to_owned(),
            "acceptance".to_owned()
        ],
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
    assert!(
        dependents(test_only, "implement").is_empty(),
        "test-only 表不含 implement"
    );
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
        expect(&TEST_ONLY_PHASE_ORDER[1..]),
        "test-only archive 白名单 = 除 initialize 外全表按表序（archive 含 agent 步 → 可重做自含）"
    );
    assert_eq!(
        allowed_backtrack_phases(test_only, "initialize"),
        Vec::<String>::new(),
        "test-only 表头 initialize 白名单为空（idx==0 早退）"
    );
    assert_eq!(
        allowed_backtrack_phases(test_only, "proposal"),
        vec!["proposal".to_owned()],
        "test-only proposal 白名单 = 恰自身（表头被属性过滤排除）"
    );
    assert!(
        !allowed_backtrack_phases(test_only, "test-gen")
            .iter()
            .any(|phase| phase == "implement"),
        "test-only 白名单不含表外相位 implement"
    );

    // requirement 表同语义（对照锚，防两表串味）：acceptance 白名单 = proposal …
    // acceptance 八相位（表头 initialize 被 backtrackable 过滤——非位置排除）
    assert_eq!(
        allowed_backtrack_phases(requirement, "acceptance"),
        expect(&REQUIREMENT_PHASE_ORDER[1..9]),
    );
}

/// 回溯白名单项携表内特性描述（正向，两表对照）：`backtrack_options` 与
/// `allowed_backtrack_phases` 同序同 id 投影；每项 description 取本表
/// `PhaseDefinition.description`（不落第二份文案映射）。
#[test]
fn 回溯白名单项携表内特性描述() {
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");

    let options = backtrack_options(requirement, "acceptance");
    let option_ids: Vec<String> = options.iter().map(|option| option.phase.clone()).collect();
    assert_eq!(
        option_ids,
        allowed_backtrack_phases(requirement, "acceptance"),
        "选项 id 序与 id 投影逐项一致"
    );
    for option in &options {
        let def = requirement
            .iter()
            .find(|def| def.id == option.phase)
            .expect("选项 id 恒在表内");
        assert_eq!(
            option.description, def.description,
            "描述取表内自描述字段: {}",
            option.phase
        );
    }
    assert_eq!(
        options[0].description, "需求提案与规格说明",
        "requirement 表 proposal 描述"
    );

    // 空集面与 id 投影同语义
    assert!(backtrack_options(requirement, "initialize").is_empty());
    assert!(backtrack_options(&[], "proposal").is_empty());
}

#[test]
fn phase_table各相位模型档位分派对照() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let expected: &[(&str, Option<ModelLevel>, Option<ModelLevel>)] = &[
        ("initialize", None, None),
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
    let proposal = phase_of(table, "proposal");
    let spec = proposal
        .executor
        .clone()
        .unwrap_or_else(|| panic!("proposal 有 executor"));
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
fn 白名单表头相位为空集无前置可回溯() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // 表头 initialize → 空白名单（idx==0 早退——表首自身无前序语义，非排除机制）
    assert!(
        allowed_backtrack_phases(table, "initialize").is_empty(),
        "表头 initialize 白名单应为空（idx==0 早退）"
    );
    // 两表表头同构：test-only 的 initialize 亦空白名单
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");
    assert!(
        allowed_backtrack_phases(test_only, "initialize").is_empty(),
        "test-only 表头 initialize 白名单应为空"
    );
}

#[test]
fn 白名单返回表序前置集含自身且initialize恒排除() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // 表中段相位 → 全部前置相位按表序返回（含自身——白名单下发的内容面；
    // initialize 表头不入任何相位的白名单——backtrackable 属性过滤）
    assert_eq!(
        allowed_backtrack_phases(table, "test-gen"),
        vec![
            "proposal".to_owned(),
            "dev-design".to_owned(),
            "test-design".to_owned(),
            "implement".to_owned(),
            "test-gen".to_owned(),
        ],
        "中段相位白名单 = 表序前置集（含自身；零 initialize 入选）"
    );
    // 末相位（archive）白名单 = 除 initialize 外全部前序相位按表序（archive 含
    // SpecSync / ResolveConflicts agent 步 → 可重做自含；表头被 backtrackable 过滤）
    assert_eq!(
        allowed_backtrack_phases(table, "archive"),
        REQUIREMENT_PHASE_ORDER[1..]
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
        "archive 白名单 = 除 initialize 外全表按表序（自含可重做）"
    );
    // acceptance 白名单 = proposal … acceptance 八相位（表序前置集语义不因表头
    // 插入漂移——archive 不为前置）
    assert_eq!(
        allowed_backtrack_phases(table, "acceptance"),
        REQUIREMENT_PHASE_ORDER[1..9]
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
        "acceptance 前置集不含 archive（末相位不在任何相位前置集内）且零 initialize"
    );
    assert!(
        !allowed_backtrack_phases(table, "acceptance")
            .iter()
            .any(|phase| phase == "archive"),
        "archive 仅前向：不在 acceptance 回溯白名单"
    );

    // proposal 白名单 = 恰含自身（第二位含自身语义；initialize 被 backtrackable
    // 过滤排除——排除由表内属性承载非位置推导的证明面）
    assert_eq!(
        allowed_backtrack_phases(table, "proposal"),
        vec!["proposal".to_owned()],
        "proposal 白名单 = 恰自身（initialize 被属性过滤排除）"
    );
}

#[test]
fn 白名单任意后续相位均不含initialize两表枚举遍历() {
    // 任意后续相位白名单不含 initialize（枚举遍历两表——AC-7 半边）
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        for def in table.iter().skip(1) {
            assert!(
                !allowed_backtrack_phases(table, def.id)
                    .iter()
                    .any(|phase| phase == "initialize"),
                "{workflow_type} 相位 {} 白名单不含 initialize",
                def.id
            );
        }
    }
}

#[test]
fn 白名单未知相位返回空vec不臆测() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    // current_phase 不在表中 → 空 Vec（不崩、不臆测）
    assert!(allowed_backtrack_phases(table, "不存在的相位").is_empty());
    assert!(allowed_backtrack_phases(table, "").is_empty());
    // 空表 → 空 Vec（既有边界持衡）
    assert!(allowed_backtrack_phases(&[], "initialize").is_empty());
    // 超长 / 特殊字符相位名 → 空 Vec（扩表不改变分派拒绝面）
    assert!(allowed_backtrack_phases(table, &"x".repeat(1200)).is_empty());
    assert!(allowed_backtrack_phases(table, "🚀\n").is_empty());
}

// ---------------------------------------------------------------------------
// 计划图单源：steps / artifact_manifest 声明（AC-1 / AC-2 / AC-7）
// ---------------------------------------------------------------------------

/// 计划步序列期望表（design 数据模型表逐相位声明；`OnFail` 三态按步绑定；
/// initialize 表头五步序——`Clean` 基线恢复步 + 两检查步 Terminal）。
fn expected_steps(phase: &str) -> Vec<StepNode> {
    match phase {
        "initialize" => INITIALIZE_STEP_ORDER.to_vec(),
        "proposal" | "dev-design" | "test-design" | "code-analyze" => vec![
            StepNode::PhaseStart,
            StepNode::Executor,
            StepNode::Evaluator,
            StepNode::VerdictGate,
            StepNode::PhaseLog,
        ],
        "implement" => vec![
            StepNode::PhaseStart,
            StepNode::Executor,
            StepNode::StaticCheck,
            StepNode::ArchiCheck,
            StepNode::Evaluator,
            StepNode::VerdictGate,
            StepNode::PhaseLog,
        ],
        "test-gen" => vec![
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
            StepNode::MutationTest,
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

/// 相位 → `on_fail` 声明期望（static-check 步 FeedbackLoop{5}——implement /
/// test-gen；verdict-gate 步 FeedbackLoop{5}（code-analyze 例外 Terminal）；
/// test-execution 相位两确定性门禁步（TestExecution / MutationTest）为 Escalate
/// ——首败升格交决策、零 agent 修复；initialize 表头两检查步（StaticCheck /
/// TestExecution）为 Terminal——基线绿门失败即停零反馈边（check-before-flow-start
/// D7）；其余 None）。
fn expected_on_fail(phase_id: &str, node: StepNode) -> Option<OnFail> {
    match node {
        StepNode::StaticCheck | StepNode::TestExecution if phase_id == "initialize" => {
            Some(OnFail::Terminal)
        }
        StepNode::TestExecution | StepNode::MutationTest if phase_id == "test-execution" => {
            Some(OnFail::Escalate)
        }
        StepNode::ArchiCheck if phase_id == "implement" => Some(OnFail::FeedbackLoop { limit: 3 }),
        StepNode::StaticCheck => Some(OnFail::FeedbackLoop { limit: 5 }),
        StepNode::VerdictGate if phase_id == "code-analyze" => Some(OnFail::Terminal),
        StepNode::VerdictGate => Some(OnFail::FeedbackLoop { limit: 5 }),
        _ => None,
    }
}

/// 计划图单源 steps 声明：`phase_table("requirement")` 返回 10 相位（initialize
/// 一次性表头 + 8 路由相位 + archive 确定性相位），逐相位 `steps` 序列与设计数据
/// 模型表一致（门禁步由 step 声明、相位限定，AC-1 / AC-2；initialize 表头五步序
/// 含 Clean——check-before-flow-start）。
#[test]
fn 计划图steps声明逐相位与设计表一致() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    assert_eq!(
        table.len(),
        10,
        "requirement 十相位（initialize 表头 + 含 archive）"
    );

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
        // 检查步与 executor / 评估面角色的绑定：initialize 表头（基线检查）与
        // implement / test-gen / test-execution 含门禁检查步
        let has_check = def
            .steps
            .iter()
            .any(|step| matches!(step.node, StepNode::StaticCheck | StepNode::TestExecution));
        let expect_check = matches!(
            def.id,
            "initialize" | "implement" | "test-gen" | "test-execution"
        );
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
/// `markdown-doc·tasks.md`；test-design 含 `markdown-doc·test-design.md`；
/// implement 含 `tasks-progress·tasks.md`（下发面：实现期进度视图声明）；
/// test-execution 含 `markdown-doc·test-reports/**`（AC-1 / AC-7 归属权威源）。
#[test]
fn 计划图产物清单声明逐相位与设计表一致() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    let expect_manifest = |phase: &str| -> Vec<(&str, &str)> {
        match phase {
            "proposal" => vec![
                ("markdown-doc", "proposal.md"),
                ("markdown-doc", "specs/**"),
            ],
            "dev-design" => vec![("markdown-doc", "design.md"), ("markdown-doc", "tasks.md")],
            "test-design" => vec![("markdown-doc", "test-design.md")],
            "implement" => vec![("tasks-progress", "tasks.md")],
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

    // 无产物相位（test-gen / code-review / acceptance / archive 等）→ 空数组
    //（清单零占位不虚设；清单为唯一候选来源）
    for id in ["test-gen", "code-review", "acceptance", "archive"] {
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
    use super::phase_table::{ARCHI_FEEDBACK_LIMIT, FEEDBACK_LOOP_LIMIT};

    let table = phase_table("requirement").expect("requirement 应返回 Some");

    for def in table {
        for step in def.steps {
            assert_eq!(
                step.on_fail,
                expected_on_fail(def.id, step.node),
                "相位 {} 步 {:?} 的 on_fail 声明",
                def.id,
                step.node
            );
            if matches!(step.on_fail, Some(OnFail::FeedbackLoop { .. })) {
                let allowed = (matches!(def.id, "implement" | "test-gen")
                    && step.node == StepNode::StaticCheck)
                    || (def.id == "implement" && step.node == StepNode::ArchiCheck)
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
            // 负向面：Escalate 不得落到 agent 步（升格截断余下步依赖步序，agent
            // 步误声明会静默吞掉本相位评估链）
            if step.node.role().is_some() {
                assert!(
                    !matches!(step.on_fail, Some(OnFail::Escalate)),
                    "相位 {} agent 步 {:?} 不得携 Escalate",
                    def.id,
                    step.node
                );
            }
        }
    }

    // FeedbackLoop 计数上限两档：既有 static-check / verdict-gate 为 5，架构漂移
    // 门禁步为独立的 3（三段计数互不占用；limit 逐值可辨由 state_test 承载）
    for def in table {
        for step in def.steps {
            if let Some(OnFail::FeedbackLoop { limit }) = step.on_fail {
                let expected = if step.node == StepNode::ArchiCheck {
                    ARCHI_FEEDBACK_LIMIT
                } else {
                    FEEDBACK_LOOP_LIMIT
                };
                assert_eq!(
                    limit, expected,
                    "相位 {} 步 {:?} 的反馈边计数上限",
                    def.id, step.node
                );
                assert!(limit > 0, "上限为正（零上限反馈无意义）");
            }
        }
    }
}

/// 正向（AC-11）：架构门禁反馈上限**单源直证**——`ARCHI_FEEDBACK_LIMIT == 3`
///（三次违例后升格），implement 的 `ArchiCheck` 步 `on_fail` 逐值取该常量（不是
/// 另一份重复字面量），且与 static-check / verdict-gate 的
/// `FEEDBACK_LOOP_LIMIT`（5）可辨（三段计数互不占用、互不顶替）。
#[test]
fn archi反馈上限常量直证且与implement步同源() {
    use super::phase_table::{ARCHI_FEEDBACK_LIMIT, FEEDBACK_LOOP_LIMIT};

    assert_eq!(
        ARCHI_FEEDBACK_LIMIT, 3,
        "架构门禁反馈上限 = 3（第 4 次违例升格，不无限回环）"
    );
    assert_ne!(
        ARCHI_FEEDBACK_LIMIT, FEEDBACK_LOOP_LIMIT,
        "与既有 static-check / verdict-gate 上限（5）分段独立"
    );
    const { assert!(ARCHI_FEEDBACK_LIMIT > 0, "上限为正（零上限反馈无意义）") };

    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let implement = phase_of(table, "implement");
    let archi = implement
        .steps
        .iter()
        .find(|step| step.node == StepNode::ArchiCheck)
        .expect("implement 应含 ArchiCheck 步");
    assert_eq!(
        archi.on_fail,
        Some(OnFail::FeedbackLoop {
            limit: ARCHI_FEEDBACK_LIMIT
        }),
        "表内 limit 与常量同值（增删一处即错位即红）"
    );

    // 计数边界（0 / 1 / 3 / 4 超限）：上限语义即「第 limit+1 次升格」——常量
    // 本身可作计数比较端（walker 侧的升格行为由 walker_test 承载）
    let mut attempts: u32 = 0;
    for _ in 0..ARCHI_FEEDBACK_LIMIT {
        attempts += 1;
        assert!(
            attempts <= ARCHI_FEEDBACK_LIMIT,
            "第 {attempts} 次违例仍走反馈边（未升格）"
        );
    }
    attempts += 1;
    assert!(
        attempts > ARCHI_FEEDBACK_LIMIT,
        "第 {} 次违例越限 → 升格（代写 fail、不跑 evaluator）",
        ARCHI_FEEDBACK_LIMIT + 1
    );
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
            backtrackable,
            executor,
            evaluator,
        } = def;
        assert_eq!(*id, def.id, "id 字段解构等值");
        assert!(
            *backtrackable == (*id != "initialize"),
            "相位 {id} backtrackable 自描述（仅 initialize false）"
        );
        assert!(!description.is_empty(), "相位 {id} description 非空");
        // 前置依赖表内自描述（字段类型 `&'static [&'static str]`；无独立 match 兜底）
        let _: &&'static [&'static str] = prerequisites;
        assert_eq!(
            *prerequisites, def.prerequisites,
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

/// archive 步序（正向）：archive `steps` 恰 11 步且与 design 数据模型表逐位一致
///（PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge →
/// SpecSync → Cleanup → Seal → Finalize → PhaseLog），首步 PhaseStart、末步
/// PhaseLog（AC-2 / AC-4 11 步链序）。
#[test]
fn archive相位十一步序列与设计表一致() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let archive = table
        .iter()
        .find(|def| def.id == "archive")
        .expect("archive 相位应在表内");

    let actual: Vec<StepNode> = archive.steps.iter().map(|step| step.node).collect();
    assert_eq!(
        actual,
        ARCHIVE_STEP_ORDER.to_vec(),
        "archive 十一步序列逐位一致"
    );
    assert_eq!(
        actual.len(),
        11,
        "archive 步数恰 11（ARCHIVE_STEP_ORDER 10 → 11 扩位）"
    );
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
    // Cleanup 位次锚：介于 SpecSync 与 Seal（… Merge → SpecSync → Cleanup →
    // Seal → …）
    let cleanup_pos = actual
        .iter()
        .position(|node| *node == StepNode::Cleanup)
        .expect("Cleanup 步应在 archive 步序内");
    assert_eq!(
        actual.get(cleanup_pos - 1),
        Some(&StepNode::SpecSync),
        "Cleanup 紧随 SpecSync"
    );
    assert_eq!(
        actual.get(cleanup_pos + 1),
        Some(&StepNode::Seal),
        "Cleanup 后继为 Seal"
    );
    // 十一步逐位无重复（关键步各恰一次）
    for node in [
        StepNode::Preflight,
        StepNode::Commit,
        StepNode::Rebase,
        StepNode::ResolveConflicts,
        StepNode::Merge,
        StepNode::SpecSync,
        StepNode::Cleanup,
        StepNode::Seal,
        StepNode::Finalize,
    ] {
        assert_eq!(
            actual
                .iter()
                .filter(|candidate| **candidate == node)
                .count(),
            1,
            "{node:?} 在 archive 步序中恰出现一次"
        );
    }
}

/// archive 步 `on_fail` 语义（正向）：十一步 `on_fail` 恒 `None`——步失败即
/// Terminal 收口，不唤决策 agent、不进相位重试（AC-8 失败停链；Cleanup 尽力
/// 语义驻 walker 执行体，声明面 `None` 与其余步一致）。
#[test]
fn archive步on_fail恒none_失败即终态() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let archive = table
        .iter()
        .find(|def| def.id == "archive")
        .expect("archive 相位应在表内");

    assert_eq!(archive.steps.len(), 11);
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

/// archive 步 `prompt_ref` 语义（边界）：十一步 `prompt_ref` 恒 `None`——spec_sync /
/// resolve_conflicts 的 prompt 由 orchestration 运行时组装（workflow 零
/// orchestration 依赖），其余步非 agent 面亦恒 None（AC-2 / 数据模型；cleanup
/// 尽力步同面）。
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
///（commit / rebase / merge / cleanup / finalize→Git、preflight / seal→Exec、
/// spec_sync / resolve_conflicts→Agent、phase_start / phase_log→Exec），
/// 与 orchestration 三分支执行缝分派前置一致（AC-2 / AC-3 / AC-4 cleanup 归
/// Git 分组）。
#[test]
fn archive步分组可派生_三分支缝分派前置() {
    use crate::state::StepGroup;

    let expect_group = |node: StepNode| -> StepGroup {
        match node {
            StepNode::PhaseStart | StepNode::Preflight | StepNode::Seal | StepNode::PhaseLog => {
                StepGroup::Exec
            }
            StepNode::Commit
            | StepNode::Rebase
            | StepNode::Merge
            | StepNode::Cleanup
            | StepNode::Finalize => StepGroup::Git,
            StepNode::SpecSync | StepNode::ResolveConflicts => StepGroup::Agent,
            other => panic!("archive 步序含越集节点 {other:?}"),
        }
    };

    for node in ARCHIVE_STEP_ORDER {
        assert_eq!(node.group(), expect_group(node), "{node:?} 分组派生一致");
    }
    // 四分类计数：Exec 4 / Git 5（含 cleanup）/ Agent 2（Gate 零——归档无门禁步）
    let groups: Vec<StepGroup> = ARCHIVE_STEP_ORDER.iter().map(|node| node.group()).collect();
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Exec).count(), 4);
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Git).count(), 5);
    assert_eq!(groups.iter().filter(|g| **g == StepGroup::Agent).count(), 2);
    assert_eq!(
        groups.iter().filter(|g| **g == StepGroup::Gate).count(),
        0,
        "归档零门禁步（无 verdict 预算分支）"
    );
}

/// ARCHIVE_STEPS 类型收敛（异常）：archive 十一步均为 `StepNode` 二十值封闭集
/// 成员（线画 snake_case 词与执行图落库同源、不引用退役 `ArchiveStage` 词汇），
/// 长度恰 11、无越集节点（AC-2 / AC-4）。
#[test]
fn archive_steps类型收敛_封闭集取值且长度十一() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let archive = table
        .iter()
        .find(|def| def.id == "archive")
        .expect("archive 相位应在表内");

    assert_eq!(archive.steps.len(), 11, "ARCHIVE_STEPS 长度恰 11");

    // 十一步词面均在 StepNode 二十值封闭集内（逐词往返 = snake_case 线词）
    let closed_set: Vec<&'static str> = ARCHIVE_STEP_ORDER
        .iter()
        .map(|node| node.as_str())
        .collect();
    for step in archive.steps {
        let word = step.node.as_str();
        assert!(
            closed_set.contains(&word),
            "归档步词 {word:?} 应在归档十一步闭集内"
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
// 归档 Cleanup 步声明（worktree-cleanup-and-active-cap / AC-4）：尽力步形态 /
// 两表同源 static / 词表与二十值封闭集对照
// ---------------------------------------------------------------------------

/// Cleanup 步 `StepDefinition` 声明形态（边界）：`node=StepNode::Cleanup`、
/// `prompt_ref=None`、`on_fail=None`（尽力语义驻 walker 执行体，声明面与其余
/// 步一致——非 agent 步零 prompt、无独立失败语义）；两表（requirement /
/// test-only）archive 相位同面在案。
#[test]
fn archive_cleanup步声明形态_node与prompt_ref与on_fail() {
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        let archive = table
            .iter()
            .find(|def| def.id == "archive")
            .expect("archive 相位应在表内");
        let cleanup = archive
            .steps
            .iter()
            .find(|step| step.node == StepNode::Cleanup)
            .unwrap_or_else(|| panic!("{workflow_type} 表 archive 应含 Cleanup 步"));
        assert_eq!(cleanup.node, StepNode::Cleanup, "{workflow_type} 表节点词");
        assert_eq!(
            cleanup.prompt_ref, None,
            "{workflow_type} 表 Cleanup prompt_ref 恒 None（非 agent 步）"
        );
        assert_eq!(
            cleanup.on_fail, None,
            "{workflow_type} 表 Cleanup on_fail 恒 None（尽力语义驻 walker）"
        );
        // 分组派生：Git 组（与 commit / rebase / merge / finalize 同族）
        assert_eq!(
            cleanup.node.group(),
            crate::state::StepGroup::Git,
            "{workflow_type} 表 Cleanup 归 Git 分组"
        );
        assert_eq!(cleanup.node.role(), None, "Cleanup 恒非 agent 步");
    }
}

/// 两 workflow_type 共用同一 static 单源（正向）：requirement 与 test-only 的
/// archive 步序逐位一致且 steps 切片指针等值（复用即单源——`ARCHIVE_STEPS`
/// 声明自然出线，零函数改动、零表复制）。
#[test]
fn archive两表共用同一static单源_步序逐位一致() {
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");
    let lhs = phase_of(requirement, "archive");
    let rhs = phase_of(test_only, "archive");

    assert_eq!(
        lhs.steps, rhs.steps,
        "两表 archive 步序逐位等值（同一 static 引用）"
    );
    assert!(
        std::ptr::eq(lhs.steps.as_ptr(), rhs.steps.as_ptr()),
        "两表 archive steps 共用同一 static（指针等值——扩步单源不漂移）"
    );
}

/// 十一步词表与 `StepNode` 二十值封闭集对照（枚举遍历）：archive 步词无漂移、
/// 无重复、全在封闭集内——十一词各恰出现一次且逐一经 serde 往返可辨（扩步不
/// 引入退役词 / 越集词）。
#[test]
fn archive十一步词表与step_node二十值封闭集对照() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let archive = table
        .iter()
        .find(|def| def.id == "archive")
        .expect("archive 相位应在表内");

    // 步词无重复：十一词去重后仍 11（无重复插入）
    let mut words: Vec<&str> = archive
        .steps
        .iter()
        .map(|step| step.node.as_str())
        .collect();
    assert_eq!(words.len(), 11, "十一步词表");
    let total = words.len();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), total, "步词无重复: {words:?}");

    // 全在 StepNode 二十值封闭集内：逐词 serde 往返可辨（越集词即败）+ 与
    // RunStepKind 同词面（落库零映射）
    for step in archive.steps {
        let word = step.node.as_str();
        let back: StepNode = serde_json::from_value(serde_json::json!(word))
            .unwrap_or_else(|_| panic!("归档步词 {word:?} 应在二十值封闭集内"));
        assert_eq!(back, step.node, "步词 {word:?} 往返逐词可辨");
        assert_eq!(
            serde_json::to_value(step.node).expect("出线应成功"),
            serde_json::json!(word),
            "wire 词面与 as_str 一致: {word}"
        );
    }
}

// ---------------------------------------------------------------------------
// test-only 表（AC-1 / AC-2 / AC-3）：步序 / 清单复用 / prompt 静态面 / 分派边界
// ---------------------------------------------------------------------------

/// test-only 表产物清单（正向）：proposal 复用 `PROPOSAL_MANIFEST`（proposal.md +
/// specs/**）、test-execution 复用 `TEST_EXECUTION_MANIFEST`（test-reports/**）、
/// code-analyze 声明 `markdown-doc·design.md`（源码分析）、test-design 声明
/// `markdown-doc·test-design.md`、其余无产物相位空数组。
#[test]
fn test_only表产物清单复用与code_analyze清单声明() {
    let table = phase_table("test-only").expect("test-only 应返回 Some");

    let expect: &[(&str, &[(&str, &str)])] = &[
        ("initialize", &[]),
        (
            "proposal",
            &[
                ("markdown-doc", "proposal.md"),
                ("markdown-doc", "specs/**"),
            ],
        ),
        ("code-analyze", &[("markdown-doc", "design.md")]),
        ("test-design", &[("markdown-doc", "test-design.md")]),
        ("test-gen", &[]),
        ("test-execution", &[("markdown-doc", "test-reports/**")]),
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
        assert_eq!(
            actual,
            expected.to_vec(),
            "test-only 相位 {} 产物清单",
            def.id
        );
        for spec in def.artifact_manifest {
            assert!(!spec.title.is_empty(), "清单标题非空（{}）", spec.path);
            assert!(
                !spec.path.starts_with('/') && !spec.path.starts_with(".."),
                "清单路径恒相对且无越界：{}",
                spec.path
            );
        }
    }

    // code-analyze 清单声明 design.md（源码分析文档，分析相位铸产物在案）
    let code_analyze = phase_of(table, "code-analyze");
    assert_eq!(
        code_analyze.artifact_manifest[0].title, "源码分析",
        "code-analyze 清单标题随声明"
    );
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
                    assert!(
                        !prompt.is_empty(),
                        "相位 {} agent 步 prompt 静态非空",
                        def.id
                    );
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
            if let Some(step) = def
                .steps
                .iter()
                .find(|step| step.node == StepNode::Executor)
            {
                assert_eq!(
                    step.prompt_ref,
                    Some(spec.prompt),
                    "相位 {} executor 同源",
                    def.id
                );
            }
        }
        if let Some(spec) = &def.evaluator {
            if let Some(step) = def
                .steps
                .iter()
                .find(|step| step.node == StepNode::Evaluator)
            {
                assert_eq!(
                    step.prompt_ref,
                    Some(spec.prompt),
                    "相位 {} evaluator 同源",
                    def.id
                );
            }
        }
    }
    // proposal / code-analyze / test-design / test-gen 各 2 条 = 8 条 agent 步
    //（test-execution 确定性相位无静态 prompt agent 步、archive 运行时组装）
    assert_eq!(agent_step_prompts, 8, "8 条 agent 步 prompt 齐备");

    // test-execution 相位无 agent 步但保留双角色 spec（编排身份面）
    let test_execution = phase_of(table, "test-execution");
    assert!(!test_execution
        .steps
        .iter()
        .any(|step| step.node.role().is_some()));
    assert!(test_execution.executor.is_some() && test_execution.evaluator.is_some());
}

#[test]
fn test_only表steps与on_fail全表扫描() {
    let table = phase_table("test-only").expect("test-only 应返回 Some");
    assert_eq!(table.len(), 7, "test-only 七相位（initialize 表头扩位）");

    for def in table {
        let actual: Vec<StepNode> = def.steps.iter().map(|step| step.node).collect();
        assert_eq!(
            actual,
            expected_steps(def.id),
            "test-only 相位 {} 步序",
            def.id
        );
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
        let has_evaluator = def
            .steps
            .iter()
            .any(|step| step.node == StepNode::Evaluator);
        let has_verdict = def
            .steps
            .iter()
            .any(|step| step.node == StepNode::VerdictGate);
        assert_eq!(
            has_verdict, has_evaluator,
            "相位 {} VerdictGate 与 Evaluator 同进退",
            def.id
        );

        for step in def.steps {
            assert_eq!(
                step.on_fail,
                expected_on_fail(def.id, step.node),
                "相位 {} 步 {:?} on_fail 声明",
                def.id,
                step.node
            );
            if matches!(step.on_fail, Some(OnFail::FeedbackLoop { .. })) {
                let allowed = (def.id == "test-gen" && step.node == StepNode::StaticCheck)
                    || step.node == StepNode::VerdictGate;
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

    for unknown in [
        "bug-fix",
        "Bug-Fix",
        "",
        " ",
        "\t",
        "requirement-s",
        " test-only2",
    ] {
        assert!(
            phase_table(unknown).is_none(),
            "未知类型 {unknown:?} 恒 None（MUST NOT 静默当 requirement 跑）"
        );
    }
}

// ---------------------------------------------------------------------------
// test-execution 拆步（desktop-fix-mutation-test / AC-1）：四步序 + MutationTest
// on_fail 反馈边② + 两表同式 + 序列边界
// ---------------------------------------------------------------------------

/// 拆步四步序（正向 / AC-1）：`TEST_EXECUTION_STEPS` 恰 `[PhaseStart,
/// TestExecution, MutationTest, PhaseLog]`——测试 / 覆盖率修绿后才跑突变
/// （walker 顺序执行天然保证）；requirement 与 test-only 两表复用同一 static
/// （`steps` 指针等值即天然同式）。
#[test]
fn test_execution拆步四步序且两表同式() {
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        let def = phase_of(table, "test-execution");
        let nodes: Vec<StepNode> = def.steps.iter().map(|step| step.node).collect();
        assert_eq!(
            nodes,
            [
                StepNode::PhaseStart,
                StepNode::TestExecution,
                StepNode::MutationTest,
                StepNode::PhaseLog,
            ],
            "{workflow_type} 表 test-execution 拆步四步序（AC-1）"
        );
        assert_eq!(def.steps.len(), 4, "步数 3 → 4（拆步插入 MutationTest）");
    }

    // 两表同式：同一 static 引用（`steps` 切片指针逐字等值——天然同式不漂移）
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");
    let lhs = phase_of(requirement, "test-execution");
    let rhs = phase_of(test_only, "test-execution");
    assert!(
        std::ptr::eq(lhs.steps.as_ptr(), rhs.steps.as_ptr()),
        "两表 test-execution steps 共用同一 static（指针等值）"
    );
    assert_eq!(lhs.steps, rhs.steps, "逐元素等值双断言（Copy 面）");
}

/// MutationTest `on_fail` 形态（正向 / AC-6）：`on_fail =
/// Some(OnFail::FeedbackLoop { limit: FEEDBACK_LOOP_LIMIT })`（独立反馈边②，
/// 与 test-execution 反馈边①分立计数）；`prompt_ref = None`（确定性工具步零
/// agent prompt）。
#[test]
fn mutation_test步on_fail升格与prompt_ref恒none() {
    use super::phase_table::FEEDBACK_LOOP_LIMIT;

    assert_eq!(
        FEEDBACK_LOOP_LIMIT, 5,
        "static-check / verdict-gate 反馈上限常量保留"
    );

    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        let def = phase_of(table, "test-execution");
        let mutation = def
            .steps
            .iter()
            .find(|step| step.node == StepNode::MutationTest)
            .unwrap_or_else(|| panic!("{workflow_type} 表应含 MutationTest 步"));
        assert_eq!(
            mutation.on_fail,
            Some(OnFail::Escalate),
            "{workflow_type} 表 MutationTest on_fail = Escalate（首败升格交决策）"
        );
        assert_eq!(mutation.prompt_ref, None, "确定性工具步 prompt_ref 恒 None");
    }
}

/// 序列边界（改写 / AC-1）：`PhaseLog` 恒末位（落账步收尾不变量）、MutationTest
/// 恰一次（拆步不重复插入）、TestExecution 与 MutationTest 相邻且序保持
///（绿后才突变）。
#[test]
fn test_execution拆步序列边界_phaselog末位且mutation恰一次() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    let def = phase_of(table, "test-execution");
    let nodes: Vec<StepNode> = def.steps.iter().map(|step| step.node).collect();

    assert_eq!(
        nodes.last(),
        Some(&StepNode::PhaseLog),
        "PhaseLog 恒末位（落账步收尾不变量）"
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| **node == StepNode::MutationTest)
            .count(),
        1,
        "MutationTest 恰一次（拆步不重复插入）"
    );
    let test_pos = nodes
        .iter()
        .position(|node| *node == StepNode::TestExecution)
        .expect("TestExecution 在场");
    let mutation_pos = nodes
        .iter()
        .position(|node| *node == StepNode::MutationTest)
        .expect("MutationTest 在场");
    assert_eq!(
        mutation_pos,
        test_pos + 1,
        "MutationTest 紧随 TestExecution（绿后才突变，顺序保证）"
    );

    // 其它相位零改动：MutationTest 仅出现于 test-execution 相位（拆步不影响
    // 静态检查 / 归档等相位）
    for def in table.iter().filter(|def| def.id != "test-execution") {
        assert!(
            !def.steps
                .iter()
                .any(|step| step.node == StepNode::MutationTest),
            "相位 {} 不含 MutationTest 步（拆步仅作用 test-execution）",
            def.id
        );
    }
}

// ---------------------------------------------------------------------------
// initialize 一次性表头（check-before-flow-start / AC-1 / AC-4 / AC-6 / AC-7）：
// 表头确定性相位全字段锚 + 两表同源 static + backtrackable 属性过滤 + 常量 /
// 再导出编译锚（Mock策略：无——编译期 const static 表直读断言，零 IO）
// ---------------------------------------------------------------------------

/// initialize 表项确定性声明全字段锚（正向，单次）：`description == "基线检查"`、
/// steps 五步序（PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog；
/// PhaseStart / Clean / PhaseLog 均 `on_fail=None`，StaticCheck / TestExecution
/// 均 `on_fail=Some(OnFail::Terminal)`，全 `prompt_ref=None`）、artifact_manifest
/// 空、prerequisites=[]、executor=None、evaluator=None、backtrackable=false。
#[test]
fn initialize表项确定性声明全字段锚() {
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        let initialize = phase_of(table, "initialize");

        assert_eq!(initialize.id, "initialize", "{workflow_type} 表头 id 逐字");
        assert_eq!(
            initialize.description, "检查变更环境和基线代码",
            "{workflow_type} initialize description 逐字（人读标签源）"
        );

        // steps 五步序逐位
        let nodes: Vec<StepNode> = initialize.steps.iter().map(|step| step.node).collect();
        assert_eq!(
            nodes,
            INITIALIZE_STEP_ORDER.to_vec(),
            "{workflow_type} initialize 五步序逐位（PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog）"
        );
        assert_eq!(
            initialize.steps.len(),
            5,
            "{workflow_type} initialize 步数恰五"
        );

        // 步声明逐字段（on_fail 三态按步绑定、prompt_ref 恒 None）
        for step in initialize.steps {
            assert_eq!(
                step.prompt_ref, None,
                "{workflow_type} initialize 步 {:?} prompt_ref 恒 None（零 agent 面非 agent 步）",
                step.node
            );
            let expect_on_fail = match step.node {
                StepNode::StaticCheck | StepNode::TestExecution => Some(OnFail::Terminal),
                StepNode::PhaseStart | StepNode::Clean | StepNode::PhaseLog => None,
                other => panic!("initialize 不含步 {other:?}"),
            };
            assert_eq!(
                step.on_fail, expect_on_fail,
                "{workflow_type} initialize 步 {:?} on_fail 声明（两检查步 Terminal——基线绿门失败即停）",
                step.node
            );
        }

        // 确定性相位其余字段：零清单、零前置、双角色缺席、不可回溯
        assert!(
            initialize.artifact_manifest.is_empty(),
            "{workflow_type} initialize artifact_manifest 空"
        );
        assert!(
            initialize.prerequisites.is_empty(),
            "{workflow_type} initialize 零前置"
        );
        assert!(
            initialize.executor.is_none(),
            "{workflow_type} initialize executor None（确定性相位零 agent）"
        );
        assert!(
            initialize.evaluator.is_none(),
            "{workflow_type} initialize evaluator None"
        );
        assert!(
            !initialize.backtrackable,
            "{workflow_type} initialize backtrackable=false（基线检查不可回溯重做）"
        );
    }
}

/// 两表 initialize 同源（边界，单次）：requirement 与 test-only 的 initialize
/// steps 逐位等值且共用同一 static（`INITIALIZE_STEPS` 两表共享——沿
/// `PROPOSAL_STEPS` 复用先例断言形态：指针等值 + 逐元素等值双锚）；步序首
/// PhaseStart / 末 PhaseLog / Clean 相位内独占（initialize 之外相位零 Clean 步）。
#[test]
fn 两表initialize共用同一static单源且clean步独占() {
    let requirement = phase_table("requirement").expect("requirement 应返回 Some");
    let test_only = phase_table("test-only").expect("test-only 应返回 Some");
    let lhs = phase_of(requirement, "initialize");
    let rhs = phase_of(test_only, "initialize");

    // 同一 static（指针等值）+ 逐元素等值双断言
    assert!(
        std::ptr::eq(lhs.steps.as_ptr(), rhs.steps.as_ptr()),
        "两表 initialize steps 共用同一 static（INITIALIZE_STEPS 单源）"
    );
    assert_eq!(lhs.steps, rhs.steps, "逐元素等值（Copy 面）");

    // Clean 相位内独占：initialize 之外任何相位零 Clean 步（枚举遍历两表）
    for table in [requirement, test_only] {
        for def in table.iter().filter(|def| def.id != "initialize") {
            assert!(
                !def.steps.iter().any(|step| step.node == StepNode::Clean),
                "相位 {} 不含 Clean 步（基线恢复步 initialize 独占）",
                def.id
            );
        }
    }
}

/// backtrackable 属性全表对拍（边界，枚举遍历）：既有 15 条目全部
/// `backtrackable: true`（两表逐条断言——requirement 8 路由 + archive、
/// test-only 5 agent + archive）；initialize 为唯一 `false` 条目（表内自描述
/// 属性全量对拍——两表合计 15 true + 2 false）。
#[test]
fn backtrackable全表对拍_initialize唯一false() {
    for (workflow_type, expect_true_count) in [("requirement", 9usize), ("test-only", 6usize)] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        let true_count = table.iter().filter(|def| def.backtrackable).count();
        let false_count = table.iter().filter(|def| !def.backtrackable).count();
        assert_eq!(
            true_count, expect_true_count,
            "{workflow_type} backtrackable=true 条目数（既有条目全补 true）"
        );
        assert_eq!(
            false_count, 1,
            "{workflow_type} backtrackable=false 恰一条（initialize 唯一）"
        );
        for def in table {
            assert_eq!(
                def.backtrackable,
                def.id != "initialize",
                "{workflow_type} 相位 {} backtrackable 属性（仅 initialize false）",
                def.id
            );
        }
    }
}

/// INITIALIZE_PHASE_ID 常量锚（正向，单次）：逐字 `"initialize"` 且与两表表头
/// 相位 id 相等（单源同值断言）；write/mod.rs 再导出面编译锚——
/// `workflow::write::INITIALIZE_PHASE_ID` 路径引用与 `phase_table::INITIALIZE_
/// PHASE_ID` 同值（再导出零漂移——walker import 面出口存在性证明）。
#[test]
fn initialize_phase_id常量逐字且再导出同值() {
    use super::phase_table::INITIALIZE_PHASE_ID as TABLE_CONST;

    // 常量逐字
    assert_eq!(TABLE_CONST, "initialize", "phase_table 常量逐字");
    assert_eq!(INITIALIZE_PHASE_ID, "initialize", "write 再导出逐字");

    // 单源同值（再导出零漂移）
    assert_eq!(
        INITIALIZE_PHASE_ID, TABLE_CONST,
        "write::INITIALIZE_PHASE_ID 与 phase_table::INITIALIZE_PHASE_ID 同值"
    );

    // 与两表表头相位 id 相等
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        assert_eq!(
            table.first().expect("表头应在场").id,
            INITIALIZE_PHASE_ID,
            "{workflow_type} 表头相位 id 与常量同值"
        );
    }
}

// ---------------------------------------------------------------------------
// 回溯白名单「可重做自排除」（test-execution-zero-agent-decision AC-1）
// ---------------------------------------------------------------------------

/// `allowed_backtrack_phases` 自排除（正向）：零 agent 步相位（test-execution）
/// 自身不入白名单——决策上下文里不存在「重做本相位」选项；可重做相位（步序列
/// 含 agent 步：implement / test-gen）自含。两表同式（表驱动两表）。「自含 ⟺
/// 可重做」由步序列 `role()` 派生，零相位名硬编码。
#[test]
fn 白名单零agent步相位自排除_可重做相位自含() {
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");

        let allowed = allowed_backtrack_phases(table, "test-execution");
        assert!(
            !allowed.iter().any(|phase| phase == "test-execution"),
            "{workflow_type} test-execution 白名单不含自身（零 agent 步不可重做）: {allowed:?}"
        );
        assert!(
            allowed.iter().any(|phase| phase == "test-gen"),
            "{workflow_type} test-execution 白名单含前序 test-gen（对照行）"
        );

        // archive：「自含 ⟺ 可重做」由步序列派生（零相位名硬编码）——含 agent 步
        // 则自含、零 agent 步则自排除；其白名单仍含前序 test-execution（零 agent 步
        // 相位可作为目标，只是不可重做自身——自排除只作用于 current_phase）
        let archive = phase_of(table, "archive");
        let archive_redoable = archive.steps.iter().any(|step| step.node.role().is_some());
        let allowed = allowed_backtrack_phases(table, "archive");
        assert_eq!(
            allowed.iter().any(|phase| phase == "archive"),
            archive_redoable,
            "{workflow_type} archive 白名单自含 ⟺ 步序列含 agent 步: {allowed:?}"
        );
        assert!(
            allowed.iter().any(|phase| phase == "test-execution"),
            "{workflow_type} archive 白名单含前序 test-execution（目标可选）: {allowed:?}"
        );

        // 可重做 agent 步相位自含（test-only 无 implement，按其表内相位对照）
        for phase in ["proposal", "test-design", "test-gen"] {
            let allowed = allowed_backtrack_phases(table, phase);
            assert!(
                allowed.iter().any(|id| id == phase),
                "{workflow_type} 相位 {phase} 白名单含自身（agent 步相位可重做）: {allowed:?}"
            );
        }
    }
}

/// 边界：`archive` 零 agent 步形态自排除（白名单自含 ⟺ 可重做，两表同式，3 例
/// 对照）——把真实表内 `archive` 的步序列替换为确定性门禁序列（`PhaseStart` /
/// `TestExecution` / `PhaseLog`，`role()` 全 `None`）后 `backtrackable` 仍为真，
/// 自身照旧不入白名单（决策上下文里不存在「重做本相位」选项）；同表 `test-gen`
/// （含 `Executor` 步）自含、`test-execution`（零 agent 步）自排除形成对照。
///
/// 口径说明：判定按步序列 `role()` 派生、不认相位名——真实相位表的 archive 复用
/// `ARCHIVE_STEPS`（含 SpecSync / ResolveConflicts agent 步）故落「可重做 → 自含」
/// 一侧；本用例以两表各自的 archive 表项零 agent 步化锁定「零 agent 步 → 自排除」
/// 一侧的边界，防按相位名硬编码回潮（spec「白名单自含即重做、零 agent 步自排除」
/// 的判定面本身）。
#[test]
fn 白名单archive零agent步自排除边界() {
    // archive 零 agent 步形态：两表同式——真实表项仅替换步序列，`backtrackable`
    // 仍 `true`（自排除不得靠资格位实现，只能由步序列派生）
    static ZERO_AGENT_ARCHIVE_STEPS: &[StepDefinition] = &[
        StepDefinition {
            node: StepNode::PhaseStart,
            prompt_ref: None,
            on_fail: None,
        },
        StepDefinition {
            node: StepNode::TestExecution,
            prompt_ref: None,
            on_fail: None,
        },
        StepDefinition {
            node: StepNode::PhaseLog,
            prompt_ref: None,
            on_fail: None,
        },
    ];

    for workflow_type in ["requirement", "test-only"] {
        let table: Vec<PhaseDefinition> = phase_table(workflow_type)
            .expect("相位表应可达")
            .iter()
            .map(|def| {
                if def.id == "archive" {
                    PhaseDefinition {
                        steps: ZERO_AGENT_ARCHIVE_STEPS,
                        ..def.clone()
                    }
                } else {
                    def.clone()
                }
            })
            .collect();

        // 例一：archive（零 agent 步）白名单不含自身
        let allowed = allowed_backtrack_phases(&table, "archive");
        assert!(
            !allowed.iter().any(|phase| phase == "archive"),
            "{workflow_type} archive（零 agent 步）白名单不含自身（不可重做本相位）: {allowed:?}"
        );
        // 例二：可重做相位仍作为回溯目标在列（自排除只作用于 current_phase）
        assert!(
            allowed.iter().any(|phase| phase == "test-gen"),
            "{workflow_type} archive 白名单含前序 test-gen（可重做相位作为目标）: {allowed:?}"
        );
        // 例三：同表 test-execution（零 agent 步）同样不含自身
        assert!(
            !allowed_backtrack_phases(&table, "test-execution")
                .iter()
                .any(|phase| phase == "test-execution"),
            "{workflow_type} test-execution 白名单不含自身（零 agent 步）"
        );

        // 对照：同表 test-gen 含 Executor 步 → 自含（自含 ⟺ 可重做）
        let allowed = allowed_backtrack_phases(&table, "test-gen");
        assert!(
            allowed.iter().any(|phase| phase == "test-gen"),
            "{workflow_type} test-gen 白名单含自身（可重做本相位）: {allowed:?}"
        );
    }
}

/// 可重做相位全枚举（正向，遍历枚举）：agent 步相位在各自表中均含自身——
/// 自含 ⟺ 可重做（自步序列派生，零相位名硬编码）；同式全表全枚举：零 agent 步
/// 相位（test-execution）落自排除侧。
#[test]
fn 白名单可重做相位全枚举自含() {
    let cases: [(&str, &[&str]); 2] = [
        (
            "requirement",
            &[
                "proposal",
                "dev-design",
                "test-design",
                "implement",
                "test-gen",
            ],
        ),
        (
            "test-only",
            &["proposal", "code-analyze", "test-design", "test-gen"],
        ),
    ];
    for (workflow_type, phases) in cases {
        let table = phase_table(workflow_type).expect("相位表应可达");
        for phase in phases {
            let def = phase_of(table, phase);
            let redoable = def.steps.iter().any(|step| step.node.role().is_some());
            assert!(
                redoable,
                "{workflow_type} 相位 {phase} 应为可重做（含 agent 步）"
            );
            let allowed = allowed_backtrack_phases(table, phase);
            assert!(
                allowed.iter().any(|id| id == phase),
                "{workflow_type} 可重做相位 {phase} 白名单自含: {allowed:?}"
            );
        }
    }
    // 全表属性（遍历枚举）：白名单自含 ⟺ 该相位可重做（步序列含 agent 步）——
    // 评估-only 相位（code-review / acceptance）与 archive（含 SpecSync /
    // ResolveConflicts agent 步）同属可重做侧，零 agent 步相位（test-execution）
    // 落自排除侧；判定零相位名硬编码
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        for def in table.iter().skip(1) {
            let redoable = def.steps.iter().any(|step| step.node.role().is_some());
            let allowed = allowed_backtrack_phases(table, def.id);
            assert_eq!(
                allowed.iter().any(|id| id == def.id),
                redoable,
                "{workflow_type} 相位 {} 白名单自含 ⟺ 可重做: {allowed:?}",
                def.id
            );
        }
    }
}

/// 未知 / 非法 `current_phase`（异常，参数化 5 值）：空 Vec 不臆测——不自相位
/// 表推导近似匹配、不做 trim / 大小写折叠。
#[test]
fn 白名单非法current_phase五形态空vec() {
    let table = phase_table("requirement").expect("相位表应可达");
    for illegal in ["不存在的相位", "", &"x".repeat(1200), "🚀\n", " implement "] {
        assert!(
            allowed_backtrack_phases(table, illegal).is_empty(),
            "非法 current_phase {illegal:?} 应得空 Vec 不臆测"
        );
    }
    // 大小写差异不折叠（"Implement" ≠ "implement"）
    assert!(allowed_backtrack_phases(table, "Implement").is_empty());
}

/// 表形态边界（边界，表驱动 3 形态）：空表 → 空 Vec；单元素表（仅 initialize）
/// 任意相位 → 空；`backtrackable = false` 的相位不进任何白名单。
#[test]
fn 白名单表形态边界三态() {
    // 形态一：空表
    assert!(allowed_backtrack_phases(&[], "initialize").is_empty());
    assert!(allowed_backtrack_phases(&[], "implement").is_empty());

    // 形态二：单元素表（仅 initialize）——表头早退
    let single: &[PhaseDefinition] = &[PhaseDefinition {
        id: "initialize",
        description: "表头",
        steps: &[],
        artifact_manifest: &[],
        prerequisites: &[],
        backtrackable: false,
        executor: None,
        evaluator: None,
    }];
    assert!(allowed_backtrack_phases(single, "initialize").is_empty());
    assert!(allowed_backtrack_phases(single, "implement").is_empty());

    // 形态三：backtrackable = false 的相位不进任何白名单（属性承载而非位置）
    let non_backtrackable: &[PhaseDefinition] = &[
        PhaseDefinition {
            id: "initialize",
            description: "表头",
            steps: &[],
            artifact_manifest: &[],
            prerequisites: &[],
            backtrackable: false,
            executor: None,
            evaluator: None,
        },
        PhaseDefinition {
            id: "frozen",
            description: "不可回溯相位",
            steps: &[],
            artifact_manifest: &[],
            prerequisites: &[],
            backtrackable: false,
            executor: None,
            evaluator: None,
        },
        PhaseDefinition {
            id: "tail",
            description: "表尾（含 agent 步 → 可重做）",
            steps: &[StepDefinition {
                node: StepNode::Executor,
                prompt_ref: None,
                on_fail: None,
            }],
            artifact_manifest: &[],
            prerequisites: &[],
            backtrackable: true,
            executor: None,
            evaluator: None,
        },
    ];
    assert!(
        !allowed_backtrack_phases(non_backtrackable, "tail")
            .iter()
            .any(|phase| phase == "frozen"),
        "backtrackable=false 相位不进白名单"
    );
    assert_eq!(
        allowed_backtrack_phases(non_backtrackable, "tail"),
        vec!["tail".to_owned()],
        "可重做且 == 自身项留存（frozen / initialize 被过滤）"
    );

    // 零 agent 步的 current_phase 自排除（表内最后一格只余自身时 → 空 Vec）
    let tool_only: &[PhaseDefinition] = &[
        PhaseDefinition {
            id: "initialize",
            description: "表头",
            steps: &[],
            artifact_manifest: &[],
            prerequisites: &[],
            backtrackable: false,
            executor: None,
            evaluator: None,
        },
        PhaseDefinition {
            id: "gate-only",
            description: "零 agent 步相位",
            steps: &[StepDefinition {
                node: StepNode::TestExecution,
                prompt_ref: None,
                on_fail: Some(OnFail::Escalate),
            }],
            artifact_manifest: &[],
            prerequisites: &[],
            backtrackable: true,
            executor: None,
            evaluator: None,
        },
    ];
    assert!(
        allowed_backtrack_phases(tool_only, "gate-only").is_empty(),
        "零 agent 步 current_phase 自排除后无剩余项"
    );
}

/// `TEST_EXECUTION_STEPS` 两步声明（正向，表驱动四步）：TestExecution /
/// MutationTest 两步 `on_fail == Some(OnFail::Escalate)`；PhaseStart / PhaseLog
/// 为 `None`（首败升格交决策的声明面单点，零会话修复）。
#[test]
fn test_execution步序列四步on_fail声明() {
    let table = phase_table("requirement").expect("相位表应可达");
    let def = phase_of(table, "test-execution");
    let steps: Vec<(StepNode, Option<OnFail>)> = def
        .steps
        .iter()
        .map(|step| (step.node, step.on_fail))
        .collect();
    assert_eq!(
        steps,
        vec![
            (StepNode::PhaseStart, None),
            (StepNode::TestExecution, Some(OnFail::Escalate)),
            (StepNode::MutationTest, Some(OnFail::Escalate)),
            (StepNode::PhaseLog, None),
        ],
        "test-execution 四步序与 on_fail 声明"
    );
}

/// `Escalate` 归属守卫（正向，遍历枚举全表所有步）：`Escalate` 仅允许出现在
/// test-execution 相位的 TestExecution / MutationTest 步——四个执行体只按
/// `on_fail` 分派，误声明会静默失效（断言即守卫）。
#[test]
fn escalate归属守卫全表遍历() {
    for workflow_type in ["requirement", "test-only"] {
        let table = phase_table(workflow_type).expect("相位表应可达");
        for def in table {
            for step in def.steps {
                if step.on_fail == Some(OnFail::Escalate) {
                    assert_eq!(
                        def.id, "test-execution",
                        "{workflow_type} Escalate 仅允许 test-execution 相位，实际相位 {}",
                        def.id
                    );
                    assert!(
                        matches!(step.node, StepNode::TestExecution | StepNode::MutationTest),
                        "{workflow_type} Escalate 仅允许 TestExecution / MutationTest 步，实际 {:?}",
                        step.node
                    );
                }
            }
        }
    }
}

/// `on_fail` 回归锚（AC-10，边界，表驱动 4 组）：initialize 两检查步仍
/// `Terminal`；implement / test-gen 的 StaticCheck 仍 `FeedbackLoop { 5 }`；
/// archive 11 步全 `None`；`FEEDBACK_LOOP_LIMIT == 5`。
#[test]
fn on_fail回归锚_ac10() {
    use super::phase_table::FEEDBACK_LOOP_LIMIT;

    let table = phase_table("requirement").expect("相位表应可达");

    // 组一：initialize 基线两检查步仍 Terminal
    let initialize = phase_of(table, "initialize");
    for node in [StepNode::StaticCheck, StepNode::TestExecution] {
        let step = initialize
            .steps
            .iter()
            .find(|step| step.node == node)
            .unwrap_or_else(|| panic!("initialize 应有 {node:?} 步"));
        assert_eq!(
            step.on_fail,
            Some(OnFail::Terminal),
            "initialize {node:?} 仍 Terminal（基线绿门失败即停）"
        );
    }

    // 组二 / 组三：implement / test-gen 的 static-check 反馈边仍 5 次循环
    for phase in ["implement", "test-gen"] {
        let def = phase_of(table, phase);
        let step = def
            .steps
            .iter()
            .find(|step| step.node == StepNode::StaticCheck)
            .unwrap_or_else(|| panic!("{phase} 应有 StaticCheck 步"));
        assert_eq!(
            step.on_fail,
            Some(OnFail::FeedbackLoop {
                limit: FEEDBACK_LOOP_LIMIT
            }),
            "{phase} static-check 反馈边上限 5 不变"
        );
    }

    // 组四：archive 11 步 on_fail 全 None
    let archive = phase_of(table, "archive");
    assert_eq!(archive.steps.len(), 11, "archive 11 步");
    for step in archive.steps {
        assert_eq!(
            step.on_fail, None,
            "archive 步 {:?} 零升格 / 零反馈声明",
            step.node
        );
    }

    assert_eq!(FEEDBACK_LOOP_LIMIT, 5, "反馈循环常量锚定 5");
}

// ---------------------------------------------------------------------------
// archi 章节：7 份矩阵命中 md 的工具引用纪律（逐文件扫描）
// ---------------------------------------------------------------------------

/// prompts 目录（`include_str!` 的源目录；`*_test.rs` 不受路径字面量禁令约束）。
fn prompts_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("prompts")
}

/// 读一份 prompt md（缺失即红——装配面 `include_str!` 的编译期守卫之外的人工锚）。
fn prompt_text(name: &str) -> String {
    let path = prompts_dir().join(format!("{name}.md"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取 prompt {} 失败: {error}", path.display()))
}

/// 16 份 prompt md 的文件名（去扩展名，字典序）。
fn prompt_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(prompts_dir())
        .expect("读取 prompts 目录失败")
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "md"))
        .map(|entry| {
            entry
                .path()
                .file_stem()
                .expect("文件名")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// 矩阵命中面（7 份 md × 各自的 archi 工具勾选集合——矩阵单源逐文件对拍）。
const ARCHI_PROMPT_MATRIX: [(&str, bool, bool); 7] = [
    ("proposal-planner", false, true),
    ("dev-design-planner", true, true),
    ("dev-design-evaluator", true, true),
    ("test-design-planner", false, true),
    ("implementation-generator", true, false),
    ("code-review-evaluator", true, false),
    ("acceptance-evaluator", true, false),
];

/// 正向：7 份命中 md 逐文件含其勾选工具名、且不含未勾选工具名。
#[test]
fn archi工具引用与注入矩阵逐文件一致() {
    for (name, module, search) in ARCHI_PROMPT_MATRIX {
        let text = prompt_text(name);
        assert_eq!(
            text.contains("archi_module"),
            module,
            "{name} 的 archi_module 勾选状态应一致"
        );
        assert_eq!(
            text.contains("archi_search"),
            search,
            "{name} 的 archi_search 勾选状态应一致"
        );
    }
}

/// 正向：每份命中 md 的工具表述含「何时用」与「返回什么」两类信息。
///
/// 判据取真实表述锚：「何时用」= 使用相位标注（`（发现` / `（定位`，即矩阵两工具的
/// 语义角色）；「返回什么」= 返回面词汇（邻接 / 子元素 / 声明位置 / `modelFile` /
/// 薄条目 / 文档路径 / 文档条目）。定位类必须能直接 `edit`，故事实上要求声明位置类
/// 词汇在场——由各文件的 `modelFile` + `modelLine` 逐文件守住。
#[test]
fn archi工具表述含何时用与返回什么() {
    for (name, _, _) in ARCHI_PROMPT_MATRIX {
        let text = prompt_text(name);
        let has_when = text.contains("（发现") || text.contains("（定位");
        assert!(
            has_when,
            "{name} 应说明「何时用」该工具（发现 / 定位语义标注）"
        );
        let has_what = text.contains("邻接")
            || text.contains("子元素")
            || text.contains("声明位置")
            || text.contains("modelFile")
            || text.contains("薄条目")
            || text.contains("文档路径")
            || text.contains("文档条目");
        assert!(has_what, "{name} 应说明「返回什么」");
    }
}

/// 异常：其余 9 份 md 零 `archi` 字样（不许诺未下发工具面）。
#[test]
fn 非命中md零archi字样() {
    let non_matching = [
        "proposal-evaluator",
        "test-design-evaluator",
        "implementation-evaluator",
        "test-gen-generator",
        "test-gen-evaluator",
        "test-execution-executor",
        "test-execution-evaluator",
        "code-analyze-planner",
        "code-analyze-evaluator",
    ];
    for name in non_matching {
        let text = prompt_text(name);
        assert!(
            !text.contains("archi"),
            "{name} 不应出现 archi 字样（未下发该工具面）"
        );
    }
}

/// 正向：两工具名的并集恰为 7 份命中文件，且不存在第三个 archi 工具名。
#[test]
fn 两工具名并集恰七份且无第三工具() {
    let hit: Vec<String> = prompt_names()
        .into_iter()
        .filter(|name| {
            let text = prompt_text(name);
            text.contains("archi_module") || text.contains("archi_search")
        })
        .collect();
    let mut expected: Vec<String> = ARCHI_PROMPT_MATRIX
        .iter()
        .map(|(name, _, _)| (*name).to_owned())
        .collect();
    expected.sort();
    assert_eq!(hit, expected, "命中面恰为矩阵七份（无遗漏 / 无越权）");

    // 越权扫描：`archi_` 系列只出现两个名字
    for name in prompt_names() {
        let text = prompt_text(&name);
        for candidate in [
            "archi_write",
            "archi_set_gate",
            "archi_graph",
            "archi_read_doc",
        ] {
            assert!(
                !text.contains(candidate),
                "{name} 不应出现第三个 archi 工具名 {candidate}"
            );
        }
    }
}

/// 异常：7 份命中 md 不含架构域路径字面量、不含「用专用工具写模型」指令（路径
/// 知识归工具返回值）。
#[test]
fn 命中md零路径模板() {
    for (name, _, _) in ARCHI_PROMPT_MATRIX {
        let text = prompt_text(name);
        assert!(
            !text.contains(".dev-team/architecture"),
            "{name} 不应含架构域路径字面量"
        );
        assert!(
            !text.contains("architecture/models"),
            "{name} 不应含模型目录路径模板"
        );
        assert!(
            !text.contains("archi_write") && !text.contains("写入模型"),
            "{name} 不应指示用专用工具写模型"
        );
    }
}

/// 边界：16 份 md 零占位 / 零 `__MCP:*` 片段（既有扫描随动）。
#[test]
fn 十六份md零占位与零mcp() {
    let names = prompt_names();
    assert_eq!(names.len(), 16, "prompt md 恰 16 份");
    for name in names {
        let text = prompt_text(&name);
        for placeholder in [
            "<change>",
            "<phase>",
            "{{",
            "__INCLUDE:",
            "__MODEL_",
            "__DEV_TEAM_ROOT__",
            "__MCP:",
        ] {
            assert!(
                !text.contains(placeholder),
                "{name} 不应含占位片段 {placeholder}"
            );
        }
        assert!(!text.trim().is_empty(), "{name} 静态非空");
    }
}

/// 回归：装配面逐字不变——16 条 `include_str!` 常量与盘面 md **逐字一致**
/// （工具名属 prompt 正文的既定事实，工具**清单 / schema** 仍不落 prompt：真源为
/// `SessionToolset` 注入）。
#[test]
fn 装配面include_str逐字不变且工具面不落prompt() {
    let mut table_prompts: Vec<&'static str> = Vec::new();
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    for phase in table {
        for agent in [&phase.executor, &phase.evaluator].into_iter().flatten() {
            let PhaseAgentSpec { prompt, .. } = agent;
            assert!(!prompt.trim().is_empty(), "{} 的 prompt 非空", phase.id);
            assert!(
                !prompt.contains("\"type\": \"object\"") && !prompt.contains("\"required\": ["),
                "{} 的 prompt 不承载工具 schema（清单真源在 SessionToolset）",
                phase.id
            );
            table_prompts.push(prompt);
        }
    }
    // requirement 表 agent 槽位 = 7 执行 + 7 评估 = 14（archive 确定性相位无 agent；
    // code-analyze 两 prompt 属 test-only 表与代码分析步骤，不在本表 agent 面）
    assert_eq!(
        table_prompts.len(),
        14,
        "requirement 表 14 条 agent 装配常量（7 执行 + 7 评估）"
    );
    let unique: std::collections::BTreeSet<&str> = table_prompts.iter().copied().collect();
    assert_eq!(
        unique.len(),
        table_prompts.len(),
        "各相位独立装配（无重复 include_str! 引用）"
    );

    // 逐字不变：每条装配常量都能在盘面 md 中逐字命中（`include_str!` 未漂移）
    let disk: Vec<String> = prompt_names()
        .iter()
        .map(|name| prompt_text(name))
        .collect();
    for prompt in table_prompts {
        assert!(
            disk.iter().any(|text| text == prompt),
            "装配常量与盘面 md 逐字一致（16 条 include_str! 零漂移）"
        );
    }
}
