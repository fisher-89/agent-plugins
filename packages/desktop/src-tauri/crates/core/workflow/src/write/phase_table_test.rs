use agent::ModelLevel;
use serde::{Deserialize, Serialize};

use super::phase_table::{
    allowed_backtrack_phases, phase_table, PhaseAgentSpec, PhaseDefinition, MAX_RETRY_TIMES,
};
use crate::state::{OnFail, StepDefinition, StepNode};

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

// ---------------------------------------------------------------------------
// 计划图单源：steps / artifact_manifest 声明（AC-1 / AC-2 / AC-7）
// ---------------------------------------------------------------------------

/// 计划步序列期望表（design 数据模型表逐相位声明；`OnFail` 三态按步绑定）。
fn expected_steps(phase: &str) -> Vec<StepNode> {
    match phase {
        "proposal" | "dev-design" | "test-design" => vec![
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
        other => panic!("对照表缺相位 {other}"),
    }
}

/// 相位 → `on_fail` 声明期望（门禁步 FeedbackLoop{5}、verdict 门 Terminal、其余 None）。
fn expected_on_fail(node: StepNode) -> Option<OnFail> {
    match node {
        StepNode::StaticCheck | StepNode::TestExecution => Some(OnFail::FeedbackLoop { limit: 5 }),
        StepNode::VerdictGate => Some(OnFail::Terminal),
        _ => None,
    }
}

/// 计划图单源 steps 声明：`phase_table("requirement")` 返回 8 相位，逐相位
/// `steps` 序列与设计数据模型表一致（门禁步由 step 声明、相位限定，AC-1 / AC-2）。
#[test]
fn 计划图steps声明逐相位与设计表一致() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");
    assert_eq!(table.len(), 8, "V1 唯一 workflow_type 八相位");

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

    // 无产物相位（implement / test-gen / code-review / acceptance 等）→ 空数组
    //（清单零占位不虚设；清单为唯一候选来源）
    for id in [
        "test-design",
        "implement",
        "test-gen",
        "code-review",
        "acceptance",
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

/// prompt 静态化：全部 agent 步的 `prompt_ref` 均为 `&'static str` 且非空；
/// 全表扫描无 `<role>` 间接读取 / `{{…}}` 占位（零占位红线，AC-1）。
#[test]
fn 计划图prompt静态非空且零占位() {
    let table = phase_table("requirement").expect("requirement 应返回 Some");

    let mut agent_step_prompts = 0usize;
    for def in table {
        for step in def.steps {
            match step.node.role() {
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
    // 评估-only evaluator 步 = 12 条 agent 步（test-execution 相位无 agent 步）
    assert_eq!(agent_step_prompts, 12, "12 条 agent 步 prompt 齐备");
}

/// on_fail 语义：三态穷举可构造且与步绑定——StaticCheck / TestExecution 携
/// `FeedbackLoop { limit: 5 }`、VerdictGate 携 `Terminal`、其余步恒 None
///（AC-2 语义前置；门禁 / 终态仅与特定步绑定，负向断言）。
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
            // 负向面：仅 implement / test-gen 的 StaticCheck 与 test-execution 的
            // TestExecution 携 FeedbackLoop；其余任何步不得携 FeedbackLoop
            if matches!(step.on_fail, Some(OnFail::FeedbackLoop { .. })) {
                let allowed = (matches!(def.id, "implement" | "test-gen")
                    && step.node == StepNode::StaticCheck)
                    || (def.id == "test-execution" && step.node == StepNode::TestExecution);
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
            executor,
            evaluator,
        } = def;
        assert_eq!(*id, def.id, "id 字段解构等值");
        assert!(!description.is_empty(), "相位 {id} description 非空");
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
    assert!(allowed_backtrack_phases(&[], "").is_empty());
    assert!(allowed_backtrack_phases(&[], "不存在的相位").is_empty());
}
