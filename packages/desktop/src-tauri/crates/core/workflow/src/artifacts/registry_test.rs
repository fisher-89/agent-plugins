//! `artifacts::registry` 的单元测试（test-design「artifacts/registry.rs ->
//! registry_test.rs」节）：**清单驱动发现**——`discover_artifacts` 按相位清单相对
//! 路径枚举命中（精确路径 / `<前缀>/**` 前缀 glob），候选喂给静态注册表，命中项
//! 去重、`phase` 归属随清单；eval-checklist 走 `(phase, attempt)` PhaseRecord 投影
//! 分支，与文件类候选在出口汇合同一 `Vec<ArtifactDescriptor>`；全树扫描
//!（`enumerate_candidates` / `collect_files`）退役——清单未命中的全树文件零入列
//!（AC-7）。
//!
//! Mock策略（test-design 本节 Mock 表）：磁盘目录树（进程边界）= tempdir 真实目录
//!（产物文件 + eval-checklist PhaseRecord 种子），真实枚举 + 真实插件 parse；
//! 无（真实组合）= `manifest` 以 `phase_table` 真件只读供给，插件 matcher 真实组合。

use std::fs;
use std::path::{Path, PathBuf};

use super::registry::{discover_artifacts, read_artifact};
use crate::model::{ChecklistItem, Verdict};
use crate::state::{ArtifactSpec, PhaseStateRecord};
use crate::write::{phase_table, PhaseDefinition};

/// 临时 change 目录 RAII：测试结束自动清理。
struct TempChange(PathBuf);

impl TempChange {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-registry-test-{}-{}",
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

    fn write_bytes(&self, rel_path: &str, content: &[u8]) {
        let path = self.0.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建子目录失败");
        }
        fs::write(path, content).expect("写字节失败");
    }
}

impl Drop for TempChange {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// requirement 真件清单（phase_table 只读供给；清单为唯一候选来源）。
fn requirement_manifest() -> &'static [PhaseDefinition] {
    phase_table("requirement").expect("requirement 表恒可达")
}

/// 空清单 fixture（零相位声明 → 零文件候选）。
static EMPTY_MANIFEST: &[PhaseDefinition] = &[PhaseDefinition {
    id: "proposal",
    description: "空清单 fixture",
    steps: &[],
    artifact_manifest: &[],
    prerequisites: &[],
    executor: None,
    evaluator: None,
}];

/// 精确路径 / 前缀 glob 双重命中同一文件的清单（去重断言用）。
static DUAL_HIT_MANIFEST: &[PhaseDefinition] = &[
    PhaseDefinition {
        id: "proposal",
        description: "精确路径声明",
        steps: &[],
        artifact_manifest: &[ArtifactSpec {
            kind: "markdown-doc",
            path: "specs/alpha/spec.md",
            title: "精确规格",
        }],
        prerequisites: &[],
        executor: None,
        evaluator: None,
    },
    PhaseDefinition {
        id: "dev-design",
        description: "前缀 glob 声明",
        steps: &[],
        artifact_manifest: &[ArtifactSpec {
            kind: "markdown-doc",
            path: "specs/**",
            title: "规格目录",
        }],
        prerequisites: &[],
        executor: None,
        evaluator: None,
    },
];

/// 内存构造一条相位评估条目（eval 投影分支入参装置）。
fn phase_entry(phase: &str, attempt: u32, checklist: Vec<ChecklistItem>) -> PhaseStateRecord {
    PhaseStateRecord {
        id: i64::from(attempt),
        change_id: "demo-change".to_owned(),
        phase: phase.to_owned(),
        attempt,
        verdict: Verdict::Pass,
        report: format!("{phase} 报告"),
        checklist,
        skipped: false,
        stale: false,
        backtrack_to: None,
        backtrack_reason: None,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
        start_at: None,
        timestamp: 1_727_000_000_000,
    }
}

fn item(name: &str, pass: bool, evidence: &str) -> ChecklistItem {
    ChecklistItem {
        item: name.to_owned(),
        pass,
        evidence: evidence.to_owned(),
    }
}

/// 某 source 的 descriptor 条数（去重断言用）。
fn count_source(descriptors: &[super::ArtifactDescriptor], source: &str) -> usize {
    descriptors.iter().filter(|d| d.source == source).count()
}

// ---------------------------------------------------------------------------
// 正向：清单命中枚举 + phase 归属随清单
// ---------------------------------------------------------------------------

/// requirement 真件清单命中：proposal.md / specs/** / design.md / test-reports/**
/// 各自枚举为候选，`ArtifactDescriptor.phase` 归属随清单声明（proposal /
/// dev-design / test-execution，AC-7）。
#[test]
fn requirement清单命中_phase归属随清单声明() {
    let change = TempChange::new("requirement-hit");
    change.write("proposal.md", "# 提案");
    change.write("specs/alpha/spec.md", "# 规格");
    change.write("design.md", "# 设计");
    change.write("tasks.md", "- [x] 一\n- [ ] 二\n");
    change.write("test-reports/run-1.md", "# 报告");

    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    let owned: Vec<(&str, &str)> = descriptors
        .iter()
        .map(|d| (d.source.as_str(), d.phase.as_str()))
        .collect();

    for expected in [
        ("proposal.md", "proposal"),
        ("specs/alpha/spec.md", "proposal"),
        ("design.md", "dev-design"),
        ("tasks.md", "dev-design"),
        ("test-reports/run-1.md", "test-execution"),
    ] {
        assert!(
            owned.contains(&expected),
            "清单命中项 {expected:?} 应在候选内：{owned:?}"
        );
    }
    // 五条清单项全部命中且无额外候选（tasks.md 的清单 kind 为 tasks-progress）
    assert_eq!(descriptors.len(), 5, "实际：{descriptors:?}");
}

/// 文件类候选经插件 parse 判 kind（markdown-doc）；eval-checklist 走 PhaseRecord
/// 投影分支，两来源在出口汇合同一清单（AC-7 D3）。
#[test]
fn 文件类与投影分支在出口汇合同一清单() {
    let change = TempChange::new("merged");
    change.write("proposal.md", "# 提案");
    change.write("tasks.md", "- [x] 完成\n- [ ] 待办\n");
    let phases = vec![phase_entry(
        "proposal",
        1,
        vec![item("问题清晰", true, "L1-10")],
    )];

    let descriptors = discover_artifacts(&change.0, &phases, requirement_manifest());
    let kinds: Vec<&str> = descriptors.iter().map(|d| d.kind.as_str()).collect();
    // tasks.md 首命中清单项为 dev-design 的 markdown-doc 声明（implement 的
    // tasks-progress 声明服务 phase_next 下发面，发现面同路径首命中去重）
    assert_eq!(
        kinds,
        vec!["eval-checklist", "markdown-doc", "markdown-doc"],
        "投影分支先于文件类 markdown-doc，tasks.md 走 markdown-doc"
    );

    // 插件顺序：特化 kind（eval-checklist）先于 markdown-doc
    let doc_pos = kinds.iter().position(|k| *k == "markdown-doc").unwrap();
    let eval_pos = kinds.iter().position(|k| *k == "eval-checklist").unwrap();
    assert!(eval_pos < doc_pos, "实际序：{kinds:?}");

    // 投影分支归属 = PhaseRecord 投影相位；文件类归属 = 清单声明相位
    let eval = descriptors
        .iter()
        .find(|d| d.kind == "eval-checklist")
        .expect("投影分支应在案");
    assert_eq!(
        (eval.source.as_str(), eval.phase.as_str()),
        ("0", "proposal")
    );
    let tasks = descriptors
        .iter()
        .find(|d| d.source == "tasks.md")
        .expect("tasks 候选应在案");
    assert_eq!(
        (tasks.kind.as_str(), tasks.phase.as_str()),
        ("markdown-doc", "dev-design"),
        "tasks.md 文档视图归属 dev-design（首命中清单项）"
    );
}

/// `specs/**` 前缀 glob 深路径命中（specs/x/y/spec.md）——相对路径枚举面。
#[test]
fn specs前缀glob深路径命中() {
    let change = TempChange::new("glob-deep");
    change.write("specs/x/y/spec.md", "# 深层规格");
    change.write("specs/a/b/c/d/spec.md", "# 更深规格");

    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    let sources: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.kind == "markdown-doc")
        .map(|d| d.source.as_str())
        .collect();
    assert!(sources.contains(&"specs/x/y/spec.md"), "实际：{sources:?}");
    assert!(sources.contains(&"specs/a/b/c/d/spec.md"));
}

/// discover 给出的 (kind, source) 可被 read_artifact 原样回放（文件与 eval 序号两
/// 种寻址形态一致性）。
#[test]
fn discover寻址与read回放一致() {
    let change = TempChange::new("roundtrip");
    change.write("tasks.md", "- [x] 完成\n");
    let phases = vec![phase_entry(
        "proposal",
        1,
        vec![item("问题清晰", true, "L1-10")],
    )];

    let descriptors = discover_artifacts(&change.0, &phases, requirement_manifest());
    assert!(descriptors.len() >= 2, "实际: {descriptors:?}");

    for descriptor in &descriptors {
        let envelope = read_artifact(&change.0, &phases, &descriptor.kind, &descriptor.source)
            .unwrap_or_else(|| {
                panic!(
                    "descriptor ({}, {}) 应可回放",
                    descriptor.kind, descriptor.source
                )
            });
        assert_eq!(envelope.kind, descriptor.kind);
        assert_eq!(envelope.title, descriptor.title);
    }
}

// ---------------------------------------------------------------------------
// 边界：空清单 / 空 phases / 空目录 / 去重
// ---------------------------------------------------------------------------

/// manifest 空 → 零文件候选；phases 空 → 零 eval-checklist 候选（两空态零伪候选）。
#[test]
fn 空清单与空phases零伪候选() {
    let change = TempChange::new("empty-both");
    change.write("proposal.md", "# 提案");
    change.write("tasks.md", "- [x] 一\n");

    // 空清单 + 空 phases：全树文件在场但零候选（清单为唯一来源）
    assert!(
        discover_artifacts(&change.0, &[], EMPTY_MANIFEST).is_empty(),
        "清单空 → 零文件候选"
    );

    // 真件清单 + 空 phases：文件候选照常、零 eval 候选
    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    assert!(!descriptors.is_empty(), "文件候选照常");
    assert!(
        !descriptors.iter().any(|d| d.kind == "eval-checklist"),
        "phases 空 → 零投影候选"
    );

    // 真件清单 + 有 phases 但 checklist 全空：同样零投影候选
    let phases = vec![phase_entry("proposal", 1, Vec::new())];
    let descriptors = discover_artifacts(&change.0, &phases, requirement_manifest());
    assert!(!descriptors.iter().any(|d| d.kind == "eval-checklist"));
}

/// 目录不存在 / 空目录（ghost 路径）→ 空清单不报错（miss 非错误）。
#[test]
fn 目录缺失或空目录返回空清单() {
    let empty = TempChange::new("empty-dir");
    assert!(discover_artifacts(&empty.0, &[], requirement_manifest()).is_empty());

    let ghost = std::env::temp_dir().join("workflow-registry-test-不存在的change目录");
    assert!(discover_artifacts(&ghost, &[], requirement_manifest()).is_empty());
}

/// 精确路径与 `<前缀>/**` glob 双重命中同一文件 → 去重（候选唯一，无重复挂载）。
#[test]
fn 精确路径与glob双重命中同一文件去重() {
    let change = TempChange::new("dual-hit");
    change.write("specs/alpha/spec.md", "# 规格");

    let descriptors = discover_artifacts(&change.0, &[], DUAL_HIT_MANIFEST);
    assert_eq!(
        count_source(&descriptors, "specs/alpha/spec.md"),
        1,
        "同路径双重命中只留一条：{descriptors:?}"
    );
    // 先到先得：精确路径声明（proposal 相位）先于该相位之后的 glob 声明
    let descriptor = descriptors
        .iter()
        .find(|d| d.source == "specs/alpha/spec.md")
        .expect("候选在案");
    assert_eq!(descriptor.phase, "proposal", "清单声明序首个命中定归属");
}

/// 清单命中数量面：多规格深路径逐条枚举（无遗漏无重复）。
#[test]
fn 多规格深路径逐条枚举() {
    let change = TempChange::new("multi-spec");
    for index in 0..8 {
        change.write(&format!("specs/cap-{index}/spec.md"), "# 规格");
        change.write(&format!("specs/cap-{index}/notes.md"), "# 附注");
    }

    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    let spec_sources: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.source.contains("cap-") && d.source.ends_with("spec.md"))
        .map(|d| d.source.as_str())
        .collect();
    assert_eq!(
        spec_sources.len(),
        8,
        "8 条 spec.md 全命中：{spec_sources:?}"
    );
    assert_eq!(
        descriptors
            .iter()
            .filter(|d| d.source.contains("cap-"))
            .count(),
        16,
        "8 spec.md + 8 notes.md 全量命中（前缀 glob 递归全量）"
    );
}

// ---------------------------------------------------------------------------
// 异常：清单未命中的全树文件零入列 / parse 失败跳过 best-effort
// ---------------------------------------------------------------------------

/// 清单未命中的全树文件（explore.md / 旧代际 phases/** / 任意路径）零入列
///（白名单取代全树扫描，AC-7 退役面）。
#[test]
fn 清单未命中的全树文件零入列() {
    let change = TempChange::new("whitelist-only");
    change.write("explore.md", "# 探索");
    change.write("phases/old/snapshot.md", "# 旧代际快照");
    change.write("notes/any.md", "# 任意路径");
    change.write("data.json", "{}");

    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    assert!(
        descriptors.is_empty(),
        "清单外文件零候选（全树扫描退役）：{descriptors:?}"
    );

    // 对照：清单内文件同时在场 → 只有它入列
    change.write("proposal.md", "# 提案");
    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    let sources: Vec<&str> = descriptors.iter().map(|d| d.source.as_str()).collect();
    assert_eq!(sources, vec!["proposal.md"]);
}

/// 插件 parse 失败（markdown-doc 内容非 UTF-8 不可解析）→ 该候选跳过、其余候选
/// 不受阻断（best-effort）。
#[test]
fn parse失败候选跳过_其余候选不受阻断() {
    let change = TempChange::new("parse-fail");
    change.write("proposal.md", "# 正常提案");
    // 非 UTF-8 字节 → read_to_string 失败 → parse 返回 None
    change.write_bytes("design.md", &[0xff, 0xfe, 0x00, 0x80]);

    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    let sources: Vec<&str> = descriptors.iter().map(|d| d.source.as_str()).collect();
    assert!(sources.contains(&"proposal.md"), "其余候选不受阻断");
    assert!(
        !sources.contains(&"design.md"),
        "不可解析候选被跳过：{sources:?}"
    );
    // 该候选单查同样 None（回放面一致）
    assert!(read_artifact(&change.0, &[], "markdown-doc", "design.md").is_none());
}

/// 清单命中但文件缺失 → 零候选（不产出半包 descriptor）。
#[test]
fn 清单命中但文件缺失时零候选() {
    let change = TempChange::new("missing-hit");
    change.write("tasks.md", "- [x] 一\n");

    let descriptors = discover_artifacts(&change.0, &[], requirement_manifest());
    let sources: Vec<&str> = descriptors.iter().map(|d| d.source.as_str()).collect();
    assert!(sources.contains(&"tasks.md"));
    assert!(
        !sources.contains(&"proposal.md"),
        "缺失的清单项零候选：{sources:?}"
    );
}

// ---------------------------------------------------------------------------
// read_artifact 回归：kind + source 命中 / 未注册 kind / 越界序号 / 敌意 source
// ---------------------------------------------------------------------------

/// 按 kind + source 命中 → 解析单信封 Some（五字段齐全）；未知 source → None
///（source 校验口径不变，AC-7）。
#[test]
fn read_artifact按kind与source命中或none() {
    let change = TempChange::new("read-ok");
    change.write("tasks.md", "- [x] 完成\n");
    change.write("proposal.md", "# 提案正文");

    let envelope = read_artifact(&change.0, &[], "tasks-progress", "tasks.md")
        .expect("tasks-progress 信封应可读取");
    assert_eq!(envelope.kind, "tasks-progress");
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.title, "任务进度");
    assert!(envelope.payload.is_object());
    assert!(
        envelope.fallback_text.is_some(),
        "五字段之 fallback_text 齐全"
    );

    let envelope = read_artifact(&change.0, &[], "markdown-doc", "proposal.md")
        .expect("markdown-doc 信封应可读取");
    assert_eq!(envelope.kind, "markdown-doc");
    assert_eq!(envelope.payload["markdown"], "# 提案正文");

    // 未知 source / 未注册 kind / 空 kind
    assert!(read_artifact(&change.0, &[], "markdown-doc", "不存在的.md").is_none());
    assert!(read_artifact(&change.0, &[], "file-log", "proposal.md").is_none());
    assert!(read_artifact(&change.0, &[], "", "proposal.md").is_none());
}

/// 越界序号（≥ phases.len()）→ None；空 phases 下任意序号均越界。
#[test]
fn 越界序号与空切片返回none() {
    let change = TempChange::new("bad-source");
    change.write("proposal.md", "# 提案正文");
    let phases = vec![phase_entry(
        "proposal",
        1,
        vec![item("问题清晰", true, "L1-10")],
    )];

    assert!(read_artifact(&change.0, &phases, "eval-checklist", "99").is_none());
    assert!(read_artifact(&change.0, &[], "eval-checklist", "0").is_none());
    assert!(read_artifact(&change.0, &phases, "eval-checklist", "0").is_some());
}

/// 敌意 source 在注册表层被拒绝，不逃逸 change 目录（既有守卫持衡）。
#[test]
fn 敌意source在注册表层被拒绝_不逃逸change目录() {
    let change = TempChange::new("hostile-source");
    change.write("proposal.md", "# 提案正文");

    // 越权目标：change 目录树外的秘密文件（真实存在，排除「恰好读不到」假阳性）
    let outside = change.0.parent().unwrap_or_else(|| Path::new("/"));
    let secret = outside.join("registry-test-secret.md");
    fs::write(&secret, "# 不应被越权读取").expect("写外部 secret 失败");

    // 形态层拒绝：.. 分量 / 绝对路径 / 反斜杠 / 盘符 / 空串 / 空分量 / 点分量
    for hostile in [
        "../registry-test-secret.md",
        "proposal.md/../registry-test-secret.md",
        "..\\..\\registry-test-secret.md",
        "..",
        ".",
        "proposal.md/../",
        "",
        "/registry-test-secret.md",
        "C:\\evil\\secret.md",
        "C:/evil/secret.md",
    ] {
        assert!(
            read_artifact(&change.0, &[], "markdown-doc", hostile).is_none(),
            "敌意 source {hostile:?} 必须在 read_artifact 入口被拒绝"
        );
    }

    assert!(fs::read_to_string(&secret).is_ok());
    let _ = fs::remove_file(&secret);

    // 正向对照：合法相对路径不受校验误伤
    assert!(
        read_artifact(&change.0, &[], "markdown-doc", "proposal.md").is_some(),
        "合法 source 不应被包含性校验误伤"
    );
}
