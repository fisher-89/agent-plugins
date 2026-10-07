//! `artifacts::registry` 的单元测试（test-design「artifacts/registry.rs ->
//! registry_test.rs」节）：候选枚举（文件树遍历 + 相位条目序列）、静态注册表
//! 分发与信封读取（AC-7 / AC-8）；入参面演进——`discover_artifacts` /
//! `read_artifact` 改 `&[PhaseStateRecord]` 切片入参，eval-checklist 候选锚定
//! 自切片下标（序列序 = 行序，非 attempt 号——锚定平移断言）；空切片 = 文档
//! 形态仅文件候选；越界序号与敌意 source 守卫持衡。

use std::fs;
use std::path::{Path, PathBuf};

use super::registry::{discover_artifacts, read_artifact};
use crate::model::{ChecklistItem, Verdict};
use crate::state::PhaseStateRecord;

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
}

impl Drop for TempChange {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 内存构造一条相位评估条目（切片入参装置）。
fn phase_entry(phase: &str, attempt: u32, checklist: Vec<ChecklistItem>) -> PhaseStateRecord {
    PhaseStateRecord {
        id: i64::from(attempt),
        change: "demo-change".to_owned(),
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

// ---------------------------------------------------------------------------
// 正向：相位序列候选锚定（切片下标 = source 序号）+ 文件候选持衡
// ---------------------------------------------------------------------------

/// 多相位多 attempt PhaseStateRecord 切片 → eval-checklist 候选 source 序号
/// 串与序列下标一一对应（序列序 = 行序，非 attempt 号——锚定平移断言）；
/// read_artifact 按序号读出对应 checklist 信封。
#[test]
fn 相位序列候选_序号串与切片下标一一对应且可回放() {
    let change = TempChange::new("phase-sequence");
    change.write("proposal.md", "# 提案");

    // 切片行序 ≠ attempt 号序：attempt 乱序排布，候选锚定仍按切片下标
    let phases = vec![
        phase_entry(
            "dev-design",
            2,
            vec![item("组件表完整", true, "五组件齐全")],
        ),
        phase_entry("proposal", 1, Vec::new()), // 空 checklist → 不产出候选
        phase_entry(
            "dev-design",
            1,
            vec![item("组件表完整", false, "缺 renderers 职责")],
        ),
    ];

    let descriptors = discover_artifacts(&change.0, &phases);

    let eval_sources: Vec<&str> = descriptors
        .iter()
        .filter(|d| d.kind == "eval-checklist")
        .map(|d| d.source.as_str())
        .collect();
    assert_eq!(
        eval_sources,
        vec!["0", "2"],
        "候选序号 = 切片下标（行序），空 checklist 条目不产出候选但不占位漂移"
    );

    // read_artifact 按序号读出对应条目信封（下标 2 = dev-design attempt 1）
    let envelope =
        read_artifact(&change.0, &phases, "eval-checklist", "2").expect("序号候选应可回放成信封");
    assert_eq!(envelope.kind, "eval-checklist");
    assert_eq!(envelope.payload["phase"], "dev-design");
    assert_eq!(
        envelope.payload["attempt"], 1,
        "锚定按切片下标而非 attempt 排序"
    );
    assert_eq!(envelope.payload["verdict"], "pass");
    assert_eq!(envelope.payload["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        envelope.payload["items"][0]["pass"],
        serde_json::json!(false),
        "checklist 项逐字段透传"
    );

    // 空清单下标（1）经 read_artifact → None（matches 过滤后的另一面）
    assert!(
        read_artifact(&change.0, &phases, "eval-checklist", "1").is_none(),
        "空 checklist 条目不产出信封"
    );
}

/// 磁盘 markdown / tasks 文件遍历候选、跳过点前缀、子目录递归、特化 kind
/// 先于 markdown-doc 的输出序——既有断言零改动持衡。
#[test]
fn 文件候选持衡_遍历与跳过点前缀与kind顺序() {
    let change = TempChange::new("file-candidates");
    change.write("proposal.md", "# 提案");
    change.write("tasks.md", "- [x] 一\n- [ ] 二\n");
    change.write(".hidden.md", "# 不应被收录");
    change.write("reports/inner.md", "# 子目录内层");

    let descriptors = discover_artifacts(&change.0, &[]);

    let sources: Vec<&str> = descriptors.iter().map(|d| d.source.as_str()).collect();
    assert!(sources.contains(&"proposal.md"), "实际: {sources:?}");
    assert!(sources.contains(&"tasks.md"));
    assert!(sources.contains(&"reports/inner.md"), "子目录 .md 一并收录");
    assert!(
        !sources.iter().any(|source| source.contains(".hidden")),
        "点前缀项跳过"
    );

    // 三个字段齐全（title 非空）
    assert!(descriptors.iter().all(|d| !d.title.is_empty()));

    // 插件顺序：特化 kind 先于 markdown-doc
    let kinds: Vec<&str> = descriptors.iter().map(|d| d.kind.as_str()).collect();
    let tasks_pos = kinds.iter().position(|k| *k == "tasks-progress").unwrap();
    let doc_pos = kinds.iter().position(|k| *k == "markdown-doc").unwrap();
    assert!(
        tasks_pos < doc_pos,
        "tasks-progress 应排在 markdown-doc 之前"
    );
}

/// 同一候选多 kind 命中并存（无排他）：tasks.md 同时命中 tasks-progress 与
/// markdown-doc。
#[test]
fn 同一候选多kind命中并存() {
    let change = TempChange::new("multi-kind");
    change.write("tasks.md", "- [x] 完成\n- [ ] 待办\n");

    let descriptors = discover_artifacts(&change.0, &[]);

    let hits: Vec<&str> = descriptors.iter().map(|d| d.kind.as_str()).collect();
    assert!(hits.contains(&"tasks-progress"), "tasks-progress 命中");
    assert!(
        hits.contains(&"markdown-doc"),
        "同一 tasks.md 仍以 markdown-doc 并存命中"
    );
    assert_eq!(
        descriptors
            .iter()
            .filter(|d| d.kind == "tasks-progress")
            .count(),
        1
    );
}

// ---------------------------------------------------------------------------
// 边界：空切片（文档形态）→ 仅文件候选
// ---------------------------------------------------------------------------

/// phases 空（文档形态 db 零条目）→ 仅文件候选，不 panic 不 Err（AC-3 文档
/// 形态产物清单半边）；目录缺失同样空清单不报错。
#[test]
fn 空切片仅文件候选_文档形态不panic() {
    let change = TempChange::new("empty-phases");
    change.write("proposal.md", "# 提案");

    let descriptors = discover_artifacts(&change.0, &[]);
    assert!(
        descriptors
            .iter()
            .all(|d| d.kind == "markdown-doc" || d.kind == "tasks-progress"),
        "零 eval 候选，仅文件候选"
    );
    assert!(
        descriptors.iter().any(|d| d.source == "proposal.md"),
        "文件候选照常"
    );

    // 空目录
    let empty = TempChange::new("empty-dir");
    assert!(discover_artifacts(&empty.0, &[]).is_empty());

    // 目录不存在（ghost 路径）
    let ghost = std::env::temp_dir().join("workflow-registry-test-不存在的change目录");
    assert!(discover_artifacts(&ghost, &[]).is_empty());
}

/// 无插件命中时返回空清单（json / ts 不被任何插件命中）。
#[test]
fn 无插件命中时返回空清单() {
    let change = TempChange::new("no-hit");
    change.write("data.json", "{ \"not\": \"markdown\" }");
    change.write("code.ts", "const x = 1;");

    let descriptors = discover_artifacts(&change.0, &[]);
    assert!(descriptors.is_empty(), "json/ts 不被任何插件命中");
}

// ---------------------------------------------------------------------------
// 异常：越界序号 / 非法 source / 未注册 kind
// ---------------------------------------------------------------------------

/// 序号串越界（≥ phases.len()）→ None；不存在文件 source → None；未注册
/// kind 与空 kind → None。
#[test]
fn 越界序号与非法source与未注册kind返回none() {
    let change = TempChange::new("bad-source");
    change.write("proposal.md", "# 提案正文");
    let phases = vec![phase_entry(
        "proposal",
        1,
        vec![item("问题清晰", true, "L1-10")],
    )];

    // 越界 eval 序号（切片长度 1）
    assert!(read_artifact(&change.0, &phases, "eval-checklist", "99").is_none());
    // 空切片下任意序号均越界
    assert!(read_artifact(&change.0, &[], "eval-checklist", "0").is_none());

    // 不存在的文件 source
    assert!(
        read_artifact(&change.0, &phases, "markdown-doc", "不存在的.md").is_none(),
        "文件 source 指向缺失文件 → None"
    );

    // 未注册 kind（file-log 是第二刀规划的 kind）/ 空 kind
    assert!(read_artifact(&change.0, &phases, "file-log", "proposal.md").is_none());
    assert!(read_artifact(&change.0, &phases, "", "proposal.md").is_none());
}

/// 敌意 source 在注册表层被拒绝，不逃逸 change 目录（既有守卫持衡）。
#[test]
fn 敌意source在注册表层被拒绝_不逃逸change目录() {
    let change = TempChange::new("hostile-source");
    change.write("proposal.md", "# 提案正文");

    // 越权目标：change 目录树外的秘密文件（workspace 侧真实存在，
    // 排除"恰好读不到"的假阳性）
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

    // 越权目标确实存在（排除环境假阳性），但无法经 read_artifact 触达
    assert!(fs::read_to_string(&secret).is_ok());
    let _ = fs::remove_file(&secret);

    // 正向对照：合法相对路径不受校验误伤
    assert!(
        read_artifact(&change.0, &[], "markdown-doc", "proposal.md").is_some(),
        "合法 source 不应被包含性校验误伤"
    );
}

// ---------------------------------------------------------------------------
// 正向：信封五字段与 discover→read 寻址一致性
// ---------------------------------------------------------------------------

/// 已注册 kind 与有效 source 返回五字段齐全的信封。
#[test]
fn 已注册kind与有效source返回五字段齐全的信封() {
    let change = TempChange::new("read-ok");
    change.write("tasks.md", "- [x] 完成\n");
    change.write("proposal.md", "# 提案正文");

    let envelope = read_artifact(&change.0, &[], "tasks-progress", "tasks.md")
        .expect("tasks-progress 信封应可读取");
    assert_eq!(envelope.kind, "tasks-progress");
    assert_eq!(envelope.version, 1, "version 从 1 起");
    assert_eq!(envelope.title, "任务进度");
    assert!(envelope.payload.is_object());
    assert!(
        envelope.fallback_text.is_some(),
        "五字段之 fallback_text 齐全"
    );

    let envelope = read_artifact(&change.0, &[], "markdown-doc", "proposal.md")
        .expect("markdown-doc 信封应可读取");
    assert_eq!(envelope.kind, "markdown-doc");
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.payload["markdown"], "# 提案正文");
}

/// discover 给出的 (kind, source) 可被 read_artifact 原样回放（文件与 eval
/// 序号两种寻址形态一致性）。
#[test]
fn discover寻址与read回放一致() {
    let change = TempChange::new("roundtrip");
    change.write("tasks.md", "- [x] 完成\n");
    let phases = vec![phase_entry(
        "proposal",
        1,
        vec![item("问题清晰", true, "L1-10")],
    )];

    let descriptors = discover_artifacts(&change.0, &phases);
    assert!(
        descriptors.len() >= 3,
        "tasks.md 两 kind + eval 条目，实际: {descriptors:?}"
    );

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

    // eval 信封内容逐字段
    let eval = descriptors
        .iter()
        .find(|d| d.kind == "eval-checklist")
        .expect("应产出 eval-checklist descriptor");
    let envelope =
        read_artifact(&change.0, &phases, &eval.kind, &eval.source).expect("eval 信封应可读取");
    assert_eq!(envelope.payload["phase"], "proposal");
    assert_eq!(envelope.payload["attempt"], 1);
    assert_eq!(envelope.payload["verdict"], "pass");
    assert_eq!(envelope.payload["items"].as_array().map(Vec::len), Some(1));
}
