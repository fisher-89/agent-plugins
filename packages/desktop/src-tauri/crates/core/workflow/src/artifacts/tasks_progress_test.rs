//! `artifacts::tasks_progress` 的单元测试（test-design「artifacts/tasks_progress.rs
//! -> tasks_progress_test.rs」节）：清单类型判定——候选恒来自 phase 定义清单枚举，
//! kind 由清单声明（`spec.kind == "tasks-progress"`），「文件名 == tasks.md」旧判据
//! 退役；勾选计数 parser 语义保留（AC-7）。
//!
//! Mock策略（test-design 本节 Mock 表）：磁盘目录树（进程边界）= tempdir 真实目录
//!（tasks.md + 清单命中的候选）；无（真实组合）= 插件 matcher 真实组合，`manifest`
//! 由 registry 入参显式供给。

use std::fs;
use std::path::{Path, PathBuf};

use super::envelope::ArtifactCandidate;
use super::registry::{discover_artifacts, ArtifactInput, ArtifactPlugin};
use super::tasks_progress::{TasksProgressPlugin, KIND};
use crate::state::ArtifactSpec;
use crate::write::{phase_table, PhaseDefinition};

/// 临时 change 目录 RAII：测试结束自动清理。
struct TempChange(PathBuf);

impl TempChange {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-taskprog-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn write(&self, rel_path: &str, content: &str) {
        let path = self.0.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建子目录失败");
        }
        fs::write(path, content).expect("写文件失败");
    }
}

impl Drop for TempChange {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 清单命中的 tasks-progress 声明（dev-design 相位清单项）。
static TASKS_SPEC: ArtifactSpec = ArtifactSpec {
    kind: KIND,
    path: "tasks.md",
    title: "任务进度",
};

/// 清单命中但 kind 属 markdown-doc（跨插件不串扰断言用）。
static DOC_SPEC: ArtifactSpec = ArtifactSpec {
    kind: "markdown-doc",
    path: "tasks.md",
    title: "任务",
};

/// 仅声明 tasks.md 的清单 fixture。
static TASKS_ONLY_MANIFEST: &[PhaseDefinition] = &[PhaseDefinition {
    id: "dev-design",
    description: "清单 fixture",
    steps: &[],
    artifact_manifest: &[TASKS_SPEC],
    prerequisites: &[],
            backtrackable: true,
    executor: None,
    evaluator: None,
}];

/// 空清单 fixture（无任何产物声明相位）。
static EMPTY_MANIFEST: &[PhaseDefinition] = &[PhaseDefinition {
    id: "proposal",
    description: "空清单 fixture",
    steps: &[],
    artifact_manifest: &[],
    prerequisites: &[],
            backtrackable: true,
    executor: None,
    evaluator: None,
}];

fn tasks_candidate() -> ArtifactCandidate {
    ArtifactCandidate::File {
        relative_path: PathBuf::from("tasks.md"),
    }
}

/// 清单上下文入参（`spec` 显式供给；`None` = 清单未命中）。
fn input_for<'a>(
    change_dir: &'a Path,
    candidate: &'a ArtifactCandidate,
    spec: Option<&'static ArtifactSpec>,
) -> ArtifactInput<'a> {
    ArtifactInput {
        change_dir,
        phases: &[],
        candidate,
        spec,
    }
}

// ---------------------------------------------------------------------------
// 正向：清单命中的 tasks.md 候选收录并解析勾选计数（parser 语义保留）
// ---------------------------------------------------------------------------

/// 清单命中的 tasks.md 候选 → plugin 收录并解析勾选计数（total / done /
/// pending 三者自洽）。
#[test]
fn 清单命中tasks候选收录并解析计数() {
    let change = TempChange::new("mixed");
    change.write(
        "tasks.md",
        "- [ ] 待办一\n- [x] 已办一\n- [ ] 待办二\n- [x] 已办二\n- [x] 已办三\n",
    );
    let candidate = tasks_candidate();
    let input = input_for(&change.0, &candidate, Some(&TASKS_SPEC));

    assert!(TasksProgressPlugin.matches(&input));
    let envelope = TasksProgressPlugin.parse(&input).expect("应产出信封");

    assert_eq!(envelope.kind, KIND);
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.title, "任务进度", "清单声明标题");
    assert_eq!(envelope.payload["total"], 5);
    assert_eq!(envelope.payload["done"], 3);
    assert_eq!(envelope.payload["pending"], 2);
    let total = envelope.payload["total"].as_u64().unwrap();
    let done = envelope.payload["done"].as_u64().unwrap();
    let pending = envelope.payload["pending"].as_u64().unwrap();
    assert_eq!(done + pending, total, "done + pending = total 自洽");

    let fallback = envelope.fallback_text.expect("fallback 非空");
    assert!(fallback.contains("3 / 5"), "实际: {fallback}");
    assert!(fallback.contains("待办 2"), "实际: {fallback}");
}

/// requirement 真件清单驱动：tasks.md 唯一候选归属 dev-design——kind 随首命中
/// 清单项绑定（dev-design 的 `markdown-doc·tasks.md`）；implement 的
/// `tasks-progress·tasks.md` 声明服务 phase_next 下发面，发现面同路径按首命中
/// 去重不重复出列。
#[test]
fn requirement清单驱动发现_phase归属随清单() {
    let change = TempChange::new("requirement-manifest");
    change.write("tasks.md", "- [x] 一\n- [ ] 二\n");

    let manifest = phase_table("requirement").expect("requirement 表恒可达");
    let descriptors = discover_artifacts(&change.0, &[], manifest);

    let tasks: Vec<(&str, &str, &str)> = descriptors
        .iter()
        .filter(|d| d.source == "tasks.md")
        .map(|d| (d.kind.as_str(), d.source.as_str(), d.phase.as_str()))
        .collect();
    assert_eq!(
        tasks,
        vec![("markdown-doc", "tasks.md", "dev-design")],
        "tasks.md 唯一候选：kind / 归属随首命中清单项（dev-design）"
    );
}

// ---------------------------------------------------------------------------
// 边界 / 异常：旧判据退役与 best-effort
// ---------------------------------------------------------------------------

/// 清单未命中但名为 tasks.md 的任意文件 → 不入列（旧判据退役，AC-7）。
#[test]
fn 清单未命中的tasks文件不入列() {
    let change = TempChange::new("undeclared-tasks");
    change.write("tasks.md", "- [x] 一\n");
    let candidate = tasks_candidate();

    // 清单缺席（spec = None）→ matcher false
    let input = input_for(&change.0, &candidate, None);
    assert!(
        !TasksProgressPlugin.matches(&input),
        "清单未命中的 tasks.md 不命中（旧文件名判据退役）"
    );

    // 清单声明 kind 为其它插件 → 本插件不收（kind 由清单绑定）
    let input = input_for(&change.0, &candidate, Some(&DOC_SPEC));
    assert!(!TasksProgressPlugin.matches(&input));

    // 发现面：清单外的 tasks.md 零候选（同目录对照）
    let descriptors = discover_artifacts(&change.0, &[], EMPTY_MANIFEST);
    assert!(
        !descriptors.iter().any(|d| d.kind == KIND),
        "空清单下零 tasks-progress 候选：{descriptors:?}"
    );
    let descriptors = discover_artifacts(&change.0, &[], TASKS_ONLY_MANIFEST);
    assert_eq!(descriptors.len(), 1, "清单命中恰一条");
}

/// 清单命中但勾选语法不可解析（无 `- [ ]` / `- [x]` 行）→ 空进度信封
///（best-effort：候选不跳过、计数零，其余候选不受阻断）。
#[test]
fn 清单命中但语法不可解析时零进度best_effort() {
    let change = TempChange::new("unparsable");
    change.write(
        "tasks.md",
        "任务列表（未使用复选框语法）\n1. 第一条\n2. 第二条\n",
    );
    let candidate = tasks_candidate();
    let input = input_for(&change.0, &candidate, Some(&TASKS_SPEC));

    assert!(TasksProgressPlugin.matches(&input), "matcher 只看清单绑定");
    let envelope = TasksProgressPlugin
        .parse(&input)
        .expect("解析仍产出信封（空进度）");
    assert_eq!(envelope.payload["total"], 0);
    assert_eq!(envelope.payload["done"], 0);
    assert_eq!(envelope.payload["pending"], 0);

    // 其余候选不受阻断：同清单另有可解析 tasks 时照常产出
    let other = TempChange::new("unparsable-other");
    other.write("design.md", "# 设计");
    other.write("tasks.md", "无复选框内容\n");
    let descriptors = discover_artifacts(&other.0, &[], TASKS_ONLY_MANIFEST);
    assert!(
        descriptors.iter().any(|d| d.kind == KIND),
        "best-effort：其余候选不受阻断"
    );
}

/// 清单命中但文件缺失 → parse 返回 None（不产出候选，不 panic）。
#[test]
fn 清单命中但文件缺失时解析为空() {
    let change = TempChange::new("missing");
    change.write("proposal.md", "# 提案");
    let candidate = tasks_candidate();
    let input = input_for(&change.0, &candidate, Some(&TASKS_SPEC));

    assert!(TasksProgressPlugin.matches(&input), "matcher 只看清单绑定");
    assert!(
        TasksProgressPlugin.parse(&input).is_none(),
        "文件缺失 → 解析 None"
    );
}

// ---------------------------------------------------------------------------
// 边界：计数面（空文件 / 全勾 / 全未勾 / 缩进）
// ---------------------------------------------------------------------------

/// 空 tasks.md 产出零计数信封（清单命中在位）。
#[test]
fn 空tasks文件产出零计数信封() {
    let change = TempChange::new("empty");
    change.write("tasks.md", "");
    let candidate = tasks_candidate();
    let input = input_for(&change.0, &candidate, Some(&TASKS_SPEC));

    assert!(TasksProgressPlugin.matches(&input));
    let envelope = TasksProgressPlugin.parse(&input).expect("空文件仍产出信封");
    assert_eq!(envelope.payload["total"], 0);
    assert_eq!(envelope.payload["done"], 0);
    assert_eq!(envelope.payload["pending"], 0);
}

/// 全部勾选与全部未勾两个极端。
#[test]
fn 全部勾选与全部未勾两极端() {
    let all_done = TempChange::new("all-done");
    all_done.write("tasks.md", "- [x] 一\n- [X] 二\n");
    let done_candidate = tasks_candidate();
    let envelope = TasksProgressPlugin
        .parse(&input_for(&all_done.0, &done_candidate, Some(&TASKS_SPEC)))
        .expect("信封");
    assert_eq!(envelope.payload["done"], 2);
    assert_eq!(envelope.payload["pending"], 0);
    assert_eq!(envelope.payload["total"], 2);

    let all_pending = TempChange::new("all-pending");
    all_pending.write("tasks.md", "- [ ] 一\n- [ ] 二\n- [ ] 三\n");
    let pending_candidate = tasks_candidate();
    let envelope = TasksProgressPlugin
        .parse(&input_for(
            &all_pending.0,
            &pending_candidate,
            Some(&TASKS_SPEC),
        ))
        .expect("信封");
    assert_eq!(envelope.payload["done"], 0);
    assert_eq!(envelope.payload["pending"], 3);
    assert_eq!(envelope.payload["total"], 3);
}

/// 缩进嵌套复选框计入、普通行与行内字样不计入。
#[test]
fn 缩进嵌套复选框计数_普通行与行内字样不计() {
    let change = TempChange::new("nested");
    change.write(
        "tasks.md",
        "- [x] 顶层\n  - [ ] 嵌套待办\n\t- [x] 制表符缩进已完成\n- 普通列表行\n正文里出现 [x] 与 - [ ] 字样的段落不算\n> 引用块行首不是列表标记，不计\n",
    );
    let candidate = tasks_candidate();

    let envelope = TasksProgressPlugin
        .parse(&input_for(&change.0, &candidate, Some(&TASKS_SPEC)))
        .expect("信封");
    // 计入：顶层 [x]、空格缩进 [ ]、制表符缩进 [x] → done=2 pending=1
    assert_eq!(envelope.payload["done"], 2);
    assert_eq!(envelope.payload["pending"], 1);
    assert_eq!(envelope.payload["total"], 3);
}

/// 超长 tasks.md（> 1000 行）计数不截断。
#[test]
fn 超长tasks文件计数不截断() {
    let change = TempChange::new("long-tasks");
    let mut content = String::new();
    for index in 0..1500 {
        if index % 3 == 0 {
            content.push_str(&format!("- [x] 完成项 {index}\n"));
        } else {
            content.push_str(&format!("- [ ] 待办项 {index}\n"));
        }
    }
    change.write("tasks.md", &content);
    let candidate = tasks_candidate();

    let envelope = TasksProgressPlugin
        .parse(&input_for(&change.0, &candidate, Some(&TASKS_SPEC)))
        .expect("信封");
    assert_eq!(envelope.payload["total"], 1500);
    assert_eq!(envelope.payload["done"], 500);
    assert_eq!(envelope.payload["pending"], 1000);
}
