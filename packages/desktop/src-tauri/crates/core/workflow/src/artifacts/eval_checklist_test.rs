//! `artifacts::eval_checklist` 的单元测试：eval 条目 → checklist 信封（AC-8）。
//!
//! 全程以内存构造的 `Workflow` 经 `ArtifactInput` 注入，eval 候选不读文件、无需磁盘。

use std::path::{Path, PathBuf};

use super::eval_checklist::{EvalChecklistPlugin, KIND};
use super::registry::{discover_artifacts, ArtifactInput, ArtifactPlugin};
use super::ArtifactCandidate;
use crate::model::{ChecklistItem, PhaseLog, Verdict, Workflow};

/// 以第 `index` 条 eval 候选直接调用插件。
fn parse_entry(workflow: &Workflow, index: usize) -> Option<super::ArtifactEnvelope> {
    let candidate = ArtifactCandidate::EvalEntry { index };
    let input = ArtifactInput {
        change_dir: Path::new("/unused"),
        inventory: crate::model::Inventory::V2,
        workflow: Some(workflow),
        candidate: &candidate,
    };
    EvalChecklistPlugin.parse(&input)
}

/// 内存构造一条 eval 记录。
fn entry(phase: &str, attempt: Option<u32>, verdict: Verdict, checklist: Vec<ChecklistItem>) -> PhaseLog {
    PhaseLog {
        phase: phase.to_string(),
        attempt,
        verdict,
        report: format!("{phase} 报告"),
        checklist,
        skipped: false,
        stale: false,
        start_at: None,
        timestamp: None,
        backtrack_to: None,
        backtrack_reason: None,
    }
}

fn item(name: &str, pass: bool, evidence: &str) -> ChecklistItem {
    ChecklistItem {
        item: name.to_string(),
        pass,
        evidence: evidence.to_string(),
    }
}

fn workflow_with(eval: Vec<PhaseLog>) -> Workflow {
    Workflow {
        workflow_type: "requirement".to_string(),
        created: None,
        eval,
        file_log: None,
        active_phase: None,
        interrupted: Vec::new(),
    }
}

#[test]
fn 含非空checklist的条目产出payload全量信封() {
    let workflow = workflow_with(vec![entry(
        "dev-design",
        Some(2),
        Verdict::Pass,
        vec![
            item("组件表完整", true, "五组件齐全"),
            item("任务可执行", false, "阶段 9 为空泛描述"),
        ],
    )]);

    let envelope = parse_entry(&workflow, 0).expect("含清单条目应产出信封");
    assert_eq!(envelope.kind, KIND);
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.title, "评估清单 · dev-design · 第 2 次");
    assert_eq!(envelope.payload["phase"], "dev-design");
    assert_eq!(envelope.payload["attempt"], 2);
    assert_eq!(envelope.payload["verdict"], "pass");
    let items = envelope.payload["items"].as_array().expect("items 为数组");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["item"], "组件表完整");
    assert_eq!(items[0]["pass"], true);
    assert_eq!(items[0]["evidence"], "五组件齐全");
    assert_eq!(items[1]["pass"], false);
}

#[test]
fn fallback_text为checklist文本化清单且非空() {
    let workflow = workflow_with(vec![entry(
        "proposal",
        None,
        Verdict::Fail,
        vec![item("问题清晰", true, "L1-10"), item("范围明确", false, "缺删除清单")],
    )]);

    let envelope = parse_entry(&workflow, 0).expect("应产出信封");
    let fallback = envelope.fallback_text.expect("fallback 非空");
    // 无 attempt 的 title 形态
    assert_eq!(envelope.title, "评估清单 · proposal");
    assert!(fallback.contains("- [x] 问题清晰 — L1-10"), "实际: {fallback}");
    assert!(fallback.contains("- [ ] 范围明确 — 缺删除清单"));
}

#[test]
fn checklist为空的条目不产出信封() {
    let workflow = workflow_with(vec![entry("implement", Some(1), Verdict::Pass, vec![])]);

    assert!(parse_entry(&workflow, 0).is_none(), "空 checklist → 无信封");
}

#[test]
fn v0无workflow时不产出任何信封() {
    // workflow = None（v0）：File 候选与 EvalEntry 候选均不命中
    let candidate_file = ArtifactCandidate::File {
        relative_path: PathBuf::from("proposal.md"),
    };
    let input_file = ArtifactInput {
        change_dir: Path::new("/unused"),
        inventory: crate::model::Inventory::V0,
        workflow: None,
        candidate: &candidate_file,
    };
    assert!(!EvalChecklistPlugin.matches(&input_file));

    let candidate_eval = ArtifactCandidate::EvalEntry { index: 0 };
    let input_eval = ArtifactInput {
        change_dir: Path::new("/unused"),
        inventory: crate::model::Inventory::V0,
        workflow: None,
        candidate: &candidate_eval,
    };
    assert!(!EvalChecklistPlugin.matches(&input_eval));
    assert!(EvalChecklistPlugin.parse(&input_eval).is_none());
}

#[test]
fn 大量条目逐条产出_序号与条目一一对应() {
    let eval: Vec<PhaseLog> = (0..12)
        .map(|i| {
            entry(
                &format!("phase-{i}"),
                Some(1),
                Verdict::Pass,
                vec![item(&format!("检查项 {i}"), true, "依据")],
            )
        })
        .collect();
    let workflow = workflow_with(eval);

    // 经 discover 枚举（change 目录可为不存在路径——eval 候选不读文件）
    let ghost = Path::new("/unused-desktop-eval-checklist");
    let descriptors = discover_artifacts(ghost, crate::model::Inventory::V2, Some(&workflow));

    let checklist_descriptors: Vec<_> = descriptors
        .iter()
        .filter(|d| d.kind == KIND)
        .collect();
    assert_eq!(checklist_descriptors.len(), 12, "12 条全含清单 → 12 个信封");
    for (index, descriptor) in checklist_descriptors.iter().enumerate() {
        assert_eq!(descriptor.source, index.to_string(), "source 序号与条目一一对应");
    }
    // 排序键 = 条目下标：历史顺序排列
    let sources: Vec<&str> = checklist_descriptors.iter().map(|d| d.source.as_str()).collect();
    let expected: Vec<String> = (0..12).map(|i| i.to_string()).collect();
    let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
    assert_eq!(sources, expected);
}

#[test]
fn evidence长文本与特殊字符item名原样保留() {
    let long_evidence = "依据开头🎯".repeat(80) + "（远超百字符的依据文本✅）";
    let workflow = workflow_with(vec![entry(
        "code-review",
        Some(3),
        Verdict::Pass,
        vec![item("检查项：含冒号/斜杠\t与emoji🚀", true, &long_evidence)],
    )]);

    let envelope = parse_entry(&workflow, 0).expect("应产出信封");
    let items = envelope.payload["items"].as_array().expect("items 数组");
    assert_eq!(items[0]["item"], "检查项：含冒号/斜杠\t与emoji🚀");
    assert_eq!(items[0]["evidence"].as_str().map(str::len), Some(long_evidence.len()));
    assert!(envelope.fallback_text.as_deref().unwrap_or_default().contains(&long_evidence));
}
