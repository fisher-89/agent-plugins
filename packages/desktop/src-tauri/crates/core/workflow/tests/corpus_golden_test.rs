//! 集成测试：fixtures 全量语料 → 判定/解析/查询/产物投影 → golden 快照（AC-14 / AC-3 / AC-4 / AC-5 / AC-6）。
//!
//! 防线机制：对每个 fixture 跑"判定 → 解析 → 详情投影 → 产物清单"全链路，
//! 序列化为键排序规范化的 JSON 与入仓 golden 全量对比；TS zod schema 漂移
//! 由此表现为可读 diff。损坏样本的投影记录 unparsable 标记与跳过统计，
//! 使"宽松解析悄悄吞数据"在 golden 上可见。
//!
//! ## golden 重写开关
//!
//! 环境变量 `DESKTOP_GOLDEN_REWRITE=1` 时，投影不再对比而是覆写入仓 golden
//! （写入本文件的 `golden_dir()` 规范路径，不存在回退路径）。预期工作流：
//!
//! ```text
//! DESKTOP_GOLDEN_REWRITE=1 cargo test -p workflow --test corpus_golden_test   # 重写
//! cargo test -p workflow --test corpus_golden_test                            # 复核：与现 golden 等价
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use foundation::layout::resolve;
use workflow::queries::{change_detail, list_changes, ChangeDetail};

/// golden 重写开关环境变量（test-gen 阶段定名）。
const REWRITE_ENV: &str = "DESKTOP_GOLDEN_REWRITE";

/// change 语料 fixture（对应 fixtures/README.md 清单表）。
const CHANGE_FIXTURES: &[&str] = &[
    "v0-a",
    "v0-b",
    "v1-a",
    "v1-b",
    "v1-c",
    "v2-a",
    "v2-b",
    "corrupt-bad-eval-entry",
    "corrupt-bad-filelog-entry",
    "corrupt-bad-timestamp",
    "corrupt-bad-verdict",
    "corrupt-invalid-json",
];

/// layout 语料 fixture：合成 workspace 树，在临时目录确定性搭建（见 fixtures/README.md）。
const LAYOUT_FIXTURES: &[&str] = &["layout-empty", "layout-mixed", "layout-workspace"];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures_root() -> PathBuf {
    manifest_dir().join("tests").join("fixtures")
}

fn golden_dir() -> PathBuf {
    manifest_dir().join("tests").join("golden")
}

fn rewrite_mode() -> bool {
    std::env::var(REWRITE_ENV).is_ok_and(|value| value != "0" && !value.is_empty())
}

/// 临时 workspace 根 RAII。
struct TempWs(PathBuf);

impl TempWs {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("workflow-it-golden-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("创建目标目录失败");
    for entry in fs::read_dir(src).expect("读取源目录失败").flatten() {
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("拷贝文件失败");
        }
    }
}

/// 单个 change fixture 的完整投影：判定 + 解析摘要（含降级统计）+ 详情全量。
fn project_change_fixture(fixture: &str) -> serde_json::Value {
    // tag 用 fixture 名：并行测试各自独占临时 workspace，避免互删目录
    let ws = TempWs::new(fixture);
    let target = ws.0.join("openspec/changes").join(fixture);
    copy_dir(&fixtures_root().join(fixture), &target);

    let layout = resolve(&ws.0);
    let change_dir = layout.changes_root.join(fixture);

    let detect = workflow::parse::detect_inventory(&change_dir);
    let parse_outcome = match workflow::parse::parse_workflow_file(
        &change_dir.join("workflow.json"),
    ) {
        workflow::parse::WorkflowFileParse::Parsed(workflow) => {
            serde_json::json!({
                "outcome": "parsed",
                "workflowType": workflow.workflow_type,
                "created": workflow.created,
                "evalCount": workflow.eval.len(),
                "evalWithChecklist": workflow.eval.iter().filter(|e| !e.checklist.is_empty()).count(),
                "evalWithBacktrack": workflow.eval.iter().filter(|e| e.backtrack_to.is_some()).count(),
                "evalSkipped": workflow.eval.iter().filter(|e| e.skipped).count(),
                "evalStale": workflow.eval.iter().filter(|e| e.stale).count(),
                "fileLog": match &workflow.file_log {
                    None => "absent",
                    Some(entries) if entries.is_empty() => "emptyArray",
                    Some(_) => "entries",
                },
                "fileLogCount": workflow.file_log.as_ref().map(Vec::len).unwrap_or(0),
                "activePhasePresent": workflow.active_phase.is_some(),
                "interruptedCount": workflow.interrupted.len(),
            })
        }
        workflow::parse::WorkflowFileParse::Unparsable { .. } => {
            // 原因文案含 serde 版本信息，不进快照；仅记录降级事实
            serde_json::json!({ "outcome": "unparsable" })
        }
    };

    let detail = change_detail(&layout, fixture).expect("语料 fixture 应可定位");
    let detail_value = detail_to_value(&detail);

    serde_json::json!({
        "fixture": fixture,
        "detect": detect,
        "parse": parse_outcome,
        "detail": detail_value,
    })
}

/// ChangeDetail → serde JSON（camelCase、键排序规范化、时间戳 RFC3339 原样保留）。
fn detail_to_value(detail: &ChangeDetail) -> serde_json::Value {
    serde_json::to_value(detail).expect("ChangeDetail 序列化失败")
}

/// layout fixture 的确定性合成 workspace 树（落盘于临时目录）。
fn build_layout_workspace(fixture: &str) -> TempWs {
    let ws = TempWs::new(fixture);
    let base = |rel: &str| ws.0.join(rel);
    match fixture {
        "layout-workspace" => {
            write_files(
                &base("openspec/changes/2026-09-01-style-active"),
                &[
                    (
                        "workflow.json",
                        r#"{ "workflow_type": "bug-fix", "created": "2026-09-01", "eval": [
                         { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "OK", "checklist": [] } ] }"#,
                    ),
                    ("proposal.md", "# 提案"),
                ],
            );
            write_files(
                &base("openspec/changes/plain-active"),
                &[("proposal.md", "# v0")],
            );
            write_files(
                &base("openspec/changes/archive/2026-05-15-archived-old"),
                &[("proposal.md", "# v0 归档"), ("design.md", "# 设计")],
            );
            write_files(
                &base("openspec/changes/archive/2026-09-01-archived-new"),
                &[
                    (
                        "workflow.json",
                        r#"{ "workflow_type": "requirement", "eval": [
                         { "phase": "proposal", "attempt": 1, "verdict": "pass", "report": "回退 created", "checklist": [
                           { "item": "回退规则生效", "pass": true, "evidence": "目录名前缀" } ] } ] }"#,
                    ),
                    ("proposal.md", "# v1 归档"),
                ],
            );
            write_files(
                &base("openspec/changes/archive/unknown-date-archived"),
                &[
                    (
                        "workflow.json",
                        r#"{ "workflow_type": "refactor", "eval": [] }"#,
                    ),
                    ("proposal.md", "# 无日期前缀"),
                ],
            );
            write_files(&base("openspec/explores"), &[("scratch.md", "# 探索笔记")]);
        }
        "layout-empty" => {
            fs::create_dir_all(base("openspec/changes")).expect("创建空树失败");
        }
        "layout-mixed" => {
            write_files(
                &base("openspec/changes"),
                &[("stray-file.md", "changes 树下混入的普通文件")],
            );
            write_files(
                &base("openspec/changes/real-change"),
                &[
                    (
                        "workflow.json",
                        r#"{ "workflow_type": "requirement", "file_log": [] }"#,
                    ),
                    ("proposal.md", "# v2"),
                ],
            );
            write_files(
                &base("openspec/changes/archive"),
                &[("loose-file.md", "archive 树下混入的普通文件")],
            );
            write_files(
                &base("openspec/changes/archive/2026-01-02-file-inside"),
                &[
                    ("proposal.md", "# v0"),
                    ("not-a-change-note.txt", "内部混入文件"),
                ],
            );
        }
        other => panic!("未知的 layout fixture: {other}"),
    }
    ws
}

fn write_files(dir: &Path, files: &[(&str, &str)]) {
    fs::create_dir_all(dir).expect("创建目录失败");
    for (name, content) in files {
        fs::write(dir.join(name), content).expect("写文件失败");
    }
}

/// layout fixture 的投影：list_changes 全量输出。
fn project_layout_fixture(fixture: &str) -> serde_json::Value {
    let ws = build_layout_workspace(fixture);
    let list = list_changes(&resolve(&ws.0));
    serde_json::json!({
        "fixture": fixture,
        "list": serde_json::to_value(&list).expect("ChangeList 序列化失败"),
    })
}

/// 对比或覆写单份 golden。
fn check_or_rewrite(golden_name: &str, projection: &serde_json::Value) {
    let normalized = format!(
        "{}\n",
        serde_json::to_string_pretty(projection).expect("投影序列化失败")
    );
    let golden_path = golden_dir().join(format!("{golden_name}.json"));
    if rewrite_mode() {
        fs::create_dir_all(golden_dir()).expect("创建 golden 目录失败");
        fs::write(&golden_path, &normalized).expect("写 golden 失败");
        return;
    }
    let expected = fs::read_to_string(&golden_path).unwrap_or_else(|err| {
        panic!("golden {golden_name}.json 缺失（先以 {REWRITE_ENV}=1 生成）: {err}")
    });
    assert_eq!(
        normalized, expected,
        "fixture {golden_name} 的投影与 golden 漂移；若为有意的 schema 演进，以 {REWRITE_ENV}=1 重写并 review diff"
    );
}

macro_rules! change_fixture_golden_test {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            check_or_rewrite($fixture, &project_change_fixture($fixture));
        }
    };
}

change_fixture_golden_test!(golden_v0_a, "v0-a");
change_fixture_golden_test!(golden_v0_b, "v0-b");
change_fixture_golden_test!(golden_v1_a, "v1-a");
change_fixture_golden_test!(golden_v1_b, "v1-b");
change_fixture_golden_test!(golden_v1_c, "v1-c");
change_fixture_golden_test!(golden_v2_a, "v2-a");
change_fixture_golden_test!(golden_v2_b, "v2-b");
change_fixture_golden_test!(golden_corrupt_bad_eval_entry, "corrupt-bad-eval-entry");
change_fixture_golden_test!(
    golden_corrupt_bad_filelog_entry,
    "corrupt-bad-filelog-entry"
);
change_fixture_golden_test!(golden_corrupt_bad_timestamp, "corrupt-bad-timestamp");
change_fixture_golden_test!(golden_corrupt_bad_verdict, "corrupt-bad-verdict");
change_fixture_golden_test!(golden_corrupt_invalid_json, "corrupt-invalid-json");

#[test]
fn golden_layout_workspace_workspace树全量() {
    check_or_rewrite(
        "layout-workspace",
        &project_layout_fixture("layout-workspace"),
    );
}

#[test]
fn golden_layout_empty_空目录树() {
    check_or_rewrite("layout-empty", &project_layout_fixture("layout-empty"));
}

#[test]
fn golden_layout_mixed_混入文件的目录树() {
    check_or_rewrite("layout-mixed", &project_layout_fixture("layout-mixed"));
}

#[test]
fn 语料完整性_fixtures目录与清单表逐项对应() {
    // 磁盘语料 = change fixture 全集（layout-* 在测试运行时确定性搭建，不落盘）
    let mut actual: Vec<String> = fs::read_dir(fixtures_root())
        .expect("fixtures 目录应存在")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    actual.sort();
    let mut expected: Vec<String> = CHANGE_FIXTURES.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(
        actual, expected,
        "fixtures 目录与清单表不一致（防样本被静默删减）；README.md 清单需同步维护"
    );
    // 语料说明文档在位
    assert!(
        fixtures_root().join("README.md").is_file(),
        "fixtures/README.md 应存在"
    );
}

#[test]
fn 语料完整性_golden目录与语料集合一致() {
    if rewrite_mode() {
        // 重写模式下 golden 目录正在被覆写，跳过该一致性断言（复核轮再验）
        return;
    }
    let mut expected: Vec<String> = CHANGE_FIXTURES
        .iter()
        .chain(LAYOUT_FIXTURES.iter())
        .map(|s| format!("{s}.json"))
        .collect();
    expected.push("README.md".to_string());
    expected.sort();

    let mut actual: Vec<String> = fs::read_dir(golden_dir())
        .expect("golden 目录应存在（先以 DESKTOP_GOLDEN_REWRITE=1 生成）")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    actual.sort();
    assert_eq!(actual, expected, "golden 文件集应与语料集合一致");
}
