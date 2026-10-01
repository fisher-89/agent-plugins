//! `snapshot.rs` 的单元测试（test-design「snapshot.rs -> snapshot_test.rs」节）。
//!
//! 装置：无 store mock——tempdir 真实 change fixture（workflow.json 真盘落盘
//! 与 openspec 目录骨架），沿 core/workflow detail_test 的临时根 RAII 装置先例
//! （std::env::temp_dir + 进程 id + Drop 清理，不引新 dev-dependency）。
//!
//! 线面注记：对编排面严格——「change 不存在」与「workflow.json 无法解析」
//! 双 `Err` 出口（坏文档不进决策输入，编排停给用户而非带病续走）；`unparsable`
//! 旗标建模保留在 `change_detail` 读面、归 UI 展示面消费，FsSnapshot 端口
//! 层将旗标置位的 detail 显式转译为 `Err`。

use std::fs;
use std::path::PathBuf;

use workflow::model::{FileLogOp, Inventory, Verdict};
use workflow::queries::detail::PIPELINE_PHASES;
use workflow::queries::ChangeSource;

use crate::port::WorkflowSnapshotPort;
use crate::snapshot::FsSnapshot;

/// 合法 v2 形态 workflow.json（file_log 在场 → Inventory::V2；active_phase /
/// interrupted / eval 多站记录齐备）。
const VALID_WORKFLOW: &str = r#"{
  "workflow_type": "requirement",
  "created": "2026-10-01",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "提案通过", "checklist": [] },
    { "phase": "dev-design", "attempt": 1, "verdict": "fail", "report": "首轮未过", "checklist": [
      { "item": "组件表完整", "pass": false, "evidence": "缺产物区组件" }
    ] }
  ],
  "active_phase": { "phase": "dev-design", "attempt": 2, "start_at": "2026-10-01T09:00:00Z" },
  "interrupted": [ { "phase": "test-gen", "attempt": 1, "start_at": "2026-10-01T07:00:00Z", "end_at": "2026-10-01T07:30:00Z" } ],
  "file_log": [
    { "op": "write", "scope": "implement", "attempt": 2, "path": "src/lib.rs", "at": "2026-10-01T08:00:00Z" },
    { "op": "delete", "scope": "workflow", "path": "old.md" }
  ]
}"#;

/// 另一 workspace 的同位 change（root 寻址隔离断言的对照内容）。
const OTHER_WORKFLOW: &str = r#"{
  "workflow_type": "requirement",
  "created": "2026-05-05",
  "eval": [
    { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "另一工作区", "checklist": [] }
  ]
}"#;

/// 临时 workspace 根 RAII（沿 core/workflow detail_test 装置先例）：测试结束
/// 自动清理。
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "orchestration-snapshot-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// 根路径字符串（`detail` 的 root 寻址入参形态）。
    fn root_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    /// 预置一个 change 目录：workflow.json + openspec 目录骨架（proposal /
    /// tasks / specs 能力树）。
    fn change(&self, name: &str, workflow_json: &str) {
        let dir = self.0.join("openspec/changes").join(name);
        fs::create_dir_all(dir.join("specs/demo-capability")).expect("创建 change 骨架目录失败");
        fs::write(dir.join("workflow.json"), workflow_json).expect("写 workflow.json 失败");
        fs::write(dir.join("proposal.md"), "# 提案\n").expect("写 proposal.md 失败");
        fs::write(dir.join("tasks.md"), "- [ ] 任务\n").expect("写 tasks.md 失败");
        fs::write(
            dir.join("specs/demo-capability/spec.md"),
            "# 能力\n## 需求\n",
        )
        .expect("写 spec.md 失败");
    }

    /// 读取 change 的 workflow.json 原始字节（只读性比对面）。
    fn workflow_json_bytes(&self, name: &str) -> Vec<u8> {
        fs::read(
            self.0
                .join("openspec/changes")
                .join(name)
                .join("workflow.json"),
        )
        .expect("读 workflow.json 失败")
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// detail 只读装配：tempdir 真实 change fixture（合法 workflow.json + openspec
/// 目录骨架）→ ChangeDetail 逐字段装配（决策输入与前置校验的输入面——AC-2
/// 有界输入来源）。
#[test]
fn detail_assembles_fields_from_real_fs_fixture() {
    let root = TempRoot::new("assemble");
    root.change("demo-change", VALID_WORKFLOW);
    let root_str = root.root_str();

    let snapshot = FsSnapshot::new(root_str.clone());
    assert_eq!(snapshot.root, root_str, "构造绑定根原样承接");

    let detail = snapshot
        .detail(&root_str, "demo-change")
        .expect("合法 change 应装配成功");

    // 身份与代际面
    assert_eq!(detail.name, "demo-change");
    assert_eq!(detail.source, ChangeSource::Active);
    assert_eq!(detail.inventory, Inventory::V2);
    assert_eq!(detail.created.as_deref(), Some("2026-10-01"));
    assert!(!detail.unparsable);

    // 9 站全量流水线，顺序固定
    let phases: Vec<&str> = detail.pipeline.iter().map(|s| s.phase.as_str()).collect();
    assert_eq!(phases, PIPELINE_PHASES.to_vec());

    // proposal 站：一条 pass 记录
    let proposal = &detail.pipeline[0];
    assert_eq!(proposal.phase, "proposal");
    assert_eq!(proposal.attempts.len(), 1);
    assert_eq!(proposal.attempts[0].verdict, Verdict::Pass);
    assert_eq!(proposal.attempts[0].report, "提案通过");
    assert!(proposal.attempts[0].checklist.is_empty());

    // dev-design 站：一条 fail 记录携 checklist 行
    let dev_design = &detail.pipeline[1];
    assert_eq!(dev_design.attempts[0].verdict, Verdict::Fail);
    assert_eq!(dev_design.attempts[0].report, "首轮未过");
    assert_eq!(dev_design.attempts[0].checklist.len(), 1);
    assert_eq!(dev_design.attempts[0].checklist[0].item, "组件表完整");
    assert!(!dev_design.attempts[0].checklist[0].pass);
    assert_eq!(dev_design.attempts[0].checklist[0].evidence, "缺产物区组件");

    // 未覆盖站 attempts 为空
    assert!(detail.pipeline[4].attempts.is_empty(), "test-gen 站无记录");

    // 运行态：active_phase + interrupted 留档
    let active = detail.active_phase.as_ref().expect("v2 应有 active_phase");
    assert_eq!(active.phase, "dev-design");
    assert_eq!(active.attempt, 2);
    assert_eq!(active.start_at.as_deref(), Some("2026-10-01T09:00:00Z"));
    assert_eq!(detail.interrupted.len(), 1);
    assert_eq!(detail.interrupted[0].phase, "test-gen");

    // file_log：条目字段面（op / scope / attempt / path / at）
    let file_log = detail.file_log.as_ref().expect("v2 应有 file_log");
    assert_eq!(file_log.len(), 2);
    assert_eq!(file_log[0].op, FileLogOp::Write);
    assert_eq!(file_log[0].scope, "implement");
    assert_eq!(file_log[0].attempt, Some(2));
    assert_eq!(file_log[0].path, "src/lib.rs");
    assert!(file_log[0].at.is_some());
    assert_eq!(file_log[1].op, FileLogOp::Delete);
    assert_eq!(file_log[1].scope, "workflow");
    assert_eq!(file_log[1].attempt, None);

    // 产物清单：骨架文档树进入（markdown-doc 可辨）
    assert!(
        detail.artifacts.iter().any(|a| a.kind == "markdown-doc"),
        "openspec 骨架文档应进入产物清单"
    );
}

/// change 不存在：detail(root, 不存在 change) → Err 显式携 change 名
/// （change_flow_start 前置校验第二分支输入面）。
#[test]
fn missing_change_yields_explicit_error() {
    let root = TempRoot::new("missing");
    root.change("real-change", VALID_WORKFLOW);
    let root_str = root.root_str();

    let snapshot = FsSnapshot::new(root_str.clone());
    let err = snapshot
        .detail(&root_str, "不存在的-change")
        .expect_err("未知 change 应显式 Err");
    assert!(
        err.contains("不存在的-change"),
        "错误显式携带 change 名: {err}"
    );
    assert!(err.contains("change 不存在"), "miss 记因: {err}");

    // 在位 change 不受误伤
    assert!(snapshot.detail(&root_str, "real-change").is_ok());
}

/// workflow.json 不可解析：预置损坏 workflow.json → 端口层显式 `Err`
/// 「无法解析」（不静默空 detail、不把旗标 detail 带进决策输入——坏文档
/// 停给用户；test-design「Err 显式」行在端口面的直接承载）。
#[test]
fn corrupt_workflow_json_yields_explicit_error() {
    let root = TempRoot::new("corrupt");
    root.change("broken-change", "{ not valid json !!!");
    let root_str = root.root_str();

    let snapshot = FsSnapshot::new(root_str.clone());
    let err = snapshot
        .detail(&root_str, "broken-change")
        .expect_err("损坏 workflow.json 应显式 Err（不静默降级）");

    assert!(
        err.contains("broken-change") && err.contains("无法解析"),
        "错误显式携带 change 名与无法解析记因: {err}"
    );
}

/// 只读性：调用前后 workflow.json 字节不变（读不属写通道约束的锚——AC-6）。
#[test]
fn detail_call_leaves_workflow_json_bytes_unchanged() {
    let root = TempRoot::new("readonly");
    root.change("demo-change", VALID_WORKFLOW);
    let root_str = root.root_str();

    let bytes_before = root.workflow_json_bytes("demo-change");

    let snapshot = FsSnapshot::new(root_str.clone());
    for _ in 0..3 {
        snapshot
            .detail(&root_str, "demo-change")
            .expect("重复只读装配应成功");
    }

    let bytes_after = root.workflow_json_bytes("demo-change");
    assert_eq!(
        bytes_after, bytes_before,
        "只读：调用前后 workflow.json 字节不变"
    );
}

/// root 寻址：root 指向不同 workspace 目录时各取各 change（跨 workspace 隔离），
/// 仅存在于单一 root 的 change 在另一 root 寻址显式 Err。
#[test]
fn two_roots_resolve_changes_independently() {
    let root_a = TempRoot::new("root-a");
    let root_b = TempRoot::new("root-b");
    root_a.change("shared-change", VALID_WORKFLOW);
    root_a.change("only-in-a", VALID_WORKFLOW);
    root_b.change("shared-change", OTHER_WORKFLOW);

    let a_str = root_a.root_str();
    let b_str = root_b.root_str();
    let snapshot_a = FsSnapshot::new(a_str.clone());
    let snapshot_b = FsSnapshot::new(b_str.clone());

    // 同名 change 各取各内容
    let detail_a = snapshot_a
        .detail(&a_str, "shared-change")
        .expect("root A 应命中自身 change");
    assert_eq!(detail_a.created.as_deref(), Some("2026-10-01"));
    assert_eq!(detail_a.pipeline[0].attempts[0].report, "提案通过");

    let detail_b = snapshot_b
        .detail(&b_str, "shared-change")
        .expect("root B 应命中自身 change");
    assert_eq!(detail_b.created.as_deref(), Some("2026-05-05"));
    assert_eq!(detail_b.pipeline[0].attempts[0].report, "另一工作区");
    assert_ne!(
        detail_a.pipeline[0].attempts[0].report, detail_b.pipeline[0].attempts[0].report,
        "同名 change 内容按 root 隔离"
    );

    // 仅存在于 A 的 change 在 B 寻址 → Err
    let err = snapshot_b
        .detail(&b_str, "only-in-a")
        .expect_err("root B 不应跨树取到 A 的 change");
    assert!(err.contains("only-in-a"), "错误显式携带 change 名: {err}");
}
