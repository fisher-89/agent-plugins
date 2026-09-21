//! 集成测试：磁盘语料 → detect_inventory → parse_workflow_file → Workflow 协同（AC-3 / AC-4）。
//!
//! 钉死判定与解析对临界事实的口径：detect 说 v2 时解析必须还原出 file_log；
//! detect 说 v1 时 file_log 必须是 None；损坏样本的降级不改变代际判定。

use std::fs;
use std::path::PathBuf;

use workflow::model::{FileLogOp, Inventory, Verdict};
use workflow::parse::{detect_inventory, parse_workflow_file, WorkflowFileParse};

/// 入仓 fixtures 语料根。
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

fn fixture_workflow(fixture: &str) -> PathBuf {
    fixtures_dir().join(fixture).join("workflow.json")
}

fn parse(fixture: &str) -> WorkflowFileParse {
    parse_workflow_file(&fixture_workflow(fixture))
}

/// 临时目录 RAII（合成样本用）。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-it-genparse-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// 场景：三代 fixture 的判定与解析贯通
// ---------------------------------------------------------------------------

#[test]
fn v2_fixtures判定v2且解析还原file_log() {
    for fixture in ["v2-a", "v2-b"] {
        assert_eq!(detect_inventory(&fixtures_dir().join(fixture)), Inventory::V2, "{fixture}");
        let WorkflowFileParse::Parsed(workflow) = parse(fixture) else {
            panic!("{fixture} 应解析成功");
        };
        assert!(workflow.file_log.is_some(), "{fixture} 的 file_log 必须还原");
        assert!(!workflow.eval.is_empty(), "{fixture} 的 eval 必须还原");
    }

    // v2-b 专属：active_phase 与三种 op 完整还原
    let WorkflowFileParse::Parsed(v2b) = parse("v2-b") else {
        panic!("v2-b 应解析成功");
    };
    let active = v2b.active_phase.expect("v2-b 应有 active_phase");
    assert_eq!(active.phase, "implement");
    let file_log = v2b.file_log.unwrap();
    let ops: Vec<FileLogOp> = file_log.iter().map(|entry| entry.op).collect();
    assert_eq!(ops, vec![FileLogOp::Write, FileLogOp::Delete, FileLogOp::Revert]);
}

#[test]
fn v1_fixtures判定v1且file_log严格为none() {
    for fixture in ["v1-a", "v1-b", "v1-c"] {
        assert_eq!(detect_inventory(&fixtures_dir().join(fixture)), Inventory::V1, "{fixture}");
        let WorkflowFileParse::Parsed(workflow) = parse(fixture) else {
            panic!("{fixture} 应解析成功");
        };
        assert!(
            workflow.file_log.is_none(),
            "{fixture} 判定为 v1，解析出的 file_log 必须是 None（v2 标注配 v1 数据的组合错误在此暴露）"
        );
    }

    // v1-b：eval 还原 + backtrack 字段随条目还原
    let WorkflowFileParse::Parsed(v1b) = parse("v1-b") else {
        panic!("v1-b 应解析成功");
    };
    assert!(!v1b.eval.is_empty());
    assert!(v1b.eval.iter().any(|entry| entry.backtrack_to.is_some()));
    assert!(v1b.eval.iter().any(|entry| entry.stale));
}

#[test]
fn v1_c的未知键被忽略且eval正常还原() {
    let WorkflowFileParse::Parsed(v1c) = parse("v1-c") else {
        panic!("v1-c（含 legacy files 桶与 source 未知键）应解析成功");
    };
    assert_eq!(v1c.workflow_type, "requirement");
    assert_eq!(v1c.eval.len(), 12);
    assert!(v1c.created.is_some());
}

#[test]
fn v0_fixtures判定v0且无workflow可解析() {
    for fixture in ["v0-a", "v0-b"] {
        assert_eq!(detect_inventory(&fixtures_dir().join(fixture)), Inventory::V0, "{fixture}");
        // v0 无 workflow.json：load 语义为 None
        assert!(workflow::parse::load_workflow(&fixtures_dir().join(fixture)).is_none());
    }
    // v0-a 的遗留 eval.json 不干扰判定
    assert!(fixtures_dir().join("v0-a").join("eval.json").is_file());
}

#[test]
fn file_log空数组的v2样本与v1的none严格区分() {
    // 合成临界样本：file_log 键存在且为空数组
    let temp = TempDir::new("v2-empty-filelog");
    let workflow_path = temp.0.join("workflow.json");
    fs::write(
        &workflow_path,
        r#"{ "workflow_type": "requirement", "file_log": [], "eval": [] }"#,
    )
    .expect("写样本失败");

    // 判定与解析对"键存在但为空"口径一致：detect=V2 且 parse 出 Some(空)
    assert_eq!(detect_inventory(&temp.0), Inventory::V2);
    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&workflow_path) else {
        panic!("空 file_log 样本应解析成功");
    };
    let file_log = workflow.file_log.expect("键存在 → Some");
    assert!(file_log.is_empty(), "空数组 → Some(空 Vec)");

    // 对照：键缺失 → detect=V1 且 file_log=None
    let temp_v1 = TempDir::new("v1-no-filelog");
    fs::write(
        temp_v1.0.join("workflow.json"),
        r#"{ "workflow_type": "requirement", "eval": [] }"#,
    )
    .expect("写样本失败");
    assert_eq!(detect_inventory(&temp_v1.0), Inventory::V1);
    let WorkflowFileParse::Parsed(v1) = parse_workflow_file(&temp_v1.0.join("workflow.json")) else {
        panic!("v1 样本应解析成功");
    };
    assert!(v1.file_log.is_none());
}

// ---------------------------------------------------------------------------
// 场景：损坏条目降级的组合行为
// ---------------------------------------------------------------------------

#[test]
fn 整体非法json的fixture降级且detect仍按文件存在性判定代际() {
    // 损坏不改变代际判定：文件存在 → 仍按现役 v2 假设（配合 unparsable 标记展示）
    assert_eq!(
        detect_inventory(&fixtures_dir().join("corrupt-invalid-json")),
        Inventory::V2
    );
    let WorkflowFileParse::Unparsable { reason } = parse("corrupt-invalid-json") else {
        panic!("整体非法 JSON 应 Unparsable");
    };
    assert!(!reason.is_empty());
}

#[test]
fn 单条eval损坏的fixture跳过该条其余条目与区块完整() {
    let fixture = "corrupt-bad-eval-entry";
    // 合法 JSON 且无 file_log 键 → 判定 v1；条目级损坏只影响解析投影
    assert_eq!(detect_inventory(&fixtures_dir().join(fixture)), Inventory::V1);
    let WorkflowFileParse::Parsed(workflow) = parse(fixture) else {
        panic!("单条损坏不应整体降级");
    };
    let phases: Vec<&str> = workflow.eval.iter().map(|entry| entry.phase.as_str()).collect();
    assert_eq!(phases, vec!["proposal", "test-design"], "损坏条目跳过，其余完整");
}

#[test]
fn 单条file_log损坏的fixture跳过该条其余保留() {
    let fixture = "corrupt-bad-filelog-entry";
    assert_eq!(detect_inventory(&fixtures_dir().join(fixture)), Inventory::V2);
    let WorkflowFileParse::Parsed(workflow) = parse(fixture) else {
        panic!("单条损坏不应整体降级");
    };
    let file_log = workflow.file_log.expect("file_log 键存在");
    let paths: Vec<&str> = file_log.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, vec!["src/keep.rs", "src/also-keep.rs"]);
}

#[test]
fn 非法verdict条目跳过_合法条目保留() {
    let WorkflowFileParse::Parsed(workflow) = parse("corrupt-bad-verdict") else {
        panic!("单条损坏不应整体降级");
    };
    assert_eq!(workflow.eval.len(), 1);
    assert_eq!(workflow.eval[0].verdict, Verdict::Fail);
}

#[test]
fn 非法时间戳降级none条目保留_且active_phase同步降级() {
    let WorkflowFileParse::Parsed(workflow) = parse("corrupt-bad-timestamp") else {
        panic!("非法时间戳不应炸条目");
    };
    assert_eq!(workflow.eval.len(), 1, "条目本身保留");
    assert_eq!(workflow.eval[0].timestamp, None);
    assert_eq!(workflow.eval[0].checklist.len(), 1);
    let active = workflow.active_phase.expect("active_phase 结构保留");
    assert_eq!(active.phase, "implement");
    assert_eq!(active.start_at, None, "start_at 非法 → None");
}

#[test]
fn 非法时间戳与非法verdict叠加的条目只被跳过一次() {
    // 合成样本：同一条同时含非法 verdict 与非法 timestamp → 整条跳过且仅一次（不重复计数）
    let temp = TempDir::new("double-corrupt");
    let workflow_path = temp.0.join("workflow.json");
    fs::write(
        &workflow_path,
        r#"{
          "workflow_type": "requirement",
          "file_log": [],
          "eval": [
            { "phase": "proposal", "attempt": 1, "verdict": "bogus", "report": "双重损坏",
              "checklist": [], "timestamp": "完全不是时间" },
            { "phase": "dev-design", "attempt": 1, "verdict": "pass", "report": "完好", "checklist": [] }
          ]
        }"#,
    )
    .expect("写样本失败");

    let WorkflowFileParse::Parsed(workflow) = parse_workflow_file(&workflow_path) else {
        panic!("双重损坏条目不应整体降级");
    };
    let phases: Vec<&str> = workflow.eval.iter().map(|entry| entry.phase.as_str()).collect();
    assert_eq!(phases, vec!["dev-design"], "损坏条目恰好跳过一次");
    // 降级不改变 Inventory 判定
    assert_eq!(detect_inventory(&temp.0), Inventory::V2);
}
