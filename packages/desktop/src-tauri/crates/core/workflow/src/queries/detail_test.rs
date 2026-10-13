//! `queries::change_detail` 的单元测试（test-design「queries/detail.rs ->
//! detail_test.rs」节，重写主体）：id 寻址（db 无该 id 记录 → 恒 `None`——零
//! 磁盘目录解析回退、零空流水线文档形态）、归档 change 全状态面（status=
//! archived、9 站 pipeline、runs 全量出线）、`ChangeDetail.id` 首字段与裸名
//!（归档日期前缀零入名）、created 恒自 `created_at` 单源、db 重组 9 站流水线
//!（attempt 升序、checklist 内联、槽位三列直读、stale / backtrack 字段透出）、
//! 时间出线单点（i64 millis → RFC3339 ISO 串 + null；epoch 0 →
//! `1970-01-01T00:00:00Z` 口径）、槽位全缺（三会话槽位 + start_at 全 null 面）、
//! worktree 出线与定位 miss 恒可达、run 史投影与并列稳定序。原文档形态 /
//! 磁盘前缀回退 / 空流水线断言随分支删除整体退役。
//!
//! Mock策略（test-design 本节 Mock 表）：种子恒为固定 id 字面量的真实
//! [`ChangeStateRecord`] / [`PhaseStateRecord`] / [`RunStateRecord`] 值；db 半边
//! 以进程内假件实现 [`ChangeStateStore`]（id 键记录表 + 归属键过滤条目序列）
//! ——「真实 tempfile Store」行在 lib-test 目标结构上不可达：workflow 自环
//! dev-dep（store 普通 dep → workflow）在 lib-test 与普通 lib 双工件下类型不
//! 统一，`&Store` 无法满足 lib-test 视角的 `dyn ChangeStateStore`（rustc
//! E0277「multiple different versions of crate workflow」），真实 db 组合行收
//! tests/corpus_golden_test.rs 集成面（该文件「db 真件组合面」节明定）。磁盘树
//! 真实 tempdir（active 裸名目录 / archive `YYYY-MM-DD-` 前缀目录 + 产物文件
//! 与惰性字节样本）。时间戳全部确定性 i64 常量。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::change_detail;
use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    ActivePhaseState, BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus,
    PhaseLogCommand, PhaseStateRecord, RunEdgeKind, RunEdgeStateRecord, RunFinishCommand,
    RunNodeStateRecord, RunStartCommand, RunStateRecord, RunStatus, RunStepKind, RunStepStatus,
    StepCommand, StepStateRecord, StoreFault,
};
use foundation::layout::{resolve, Layout};

/// 固定 id 字面量（uuid v7 形态不透明串——一切寻址断言以 id 为键；name 仅作
/// 展示属性与磁盘目录 / worktree 目录供给值）。id 与 name 字面量各异：以
/// name 冒充 id 的寻址错位会被断言击穿。
const ID_MULTI: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c01";
const ID_ARCHIVED: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c02";
const ID_ZERO_TS: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c03";
const ID_BARE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c04";
const ID_FLAGS: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c05";
const ID_RUNNED: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c06";
const ID_GHOST: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c07";
const ID_LEGACY: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c08";
const ID_WORKTREE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c09";
const ID_WT_GONE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c10";
const ID_ARCH_GONE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4c11";
/// 未建档查询串（db 无此 id——零目录回退输入面）。
const ID_UNKNOWN: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4cff";

/// requirement 相位序夹具（D12：`detail.rs` 的 `PIPELINE_PHASES` 常量退役——
/// 列序改由 `phase_table(record.workflow_type)` 出线，测试自备 fixture）：9 列，
/// archive 为第 9 列、无 code-analyze 占位列。
const REQUIREMENT_PHASES: [&str; 9] = [
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

/// test-only 相位序夹具：6 列，code-analyze 为第 2 列、archive 为第 6 列。
const TEST_ONLY_PHASES: [&str; 6] = [
    "proposal",
    "code-analyze",
    "test-design",
    "test-gen",
    "test-execution",
    "archive",
];

/// 未知工作流类型（不在相位表内——读面显式 `None`，MUST NOT 静默当 requirement）。
const WORKFLOW_TYPE_UNKNOWN: &str = "bug-fix";

/// 临时 workspace 根 RAII + 进程内假件 store。
struct Env {
    root: PathBuf,
    store: DetailStore,
    layout: Layout,
}

impl Env {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-detail-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self {
            root: dir.clone(),
            store: DetailStore::new(),
            layout: resolve(&dir),
        }
    }

    /// 预置建档记录（active 起步；id / name 字面量各异）。
    fn seed(&self, id: &str, name: &str, created_at: i64, active_phase: Option<ActivePhaseState>) {
        self.seed_record(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase,
            worktree: None,
            base_commit: None,
        });
    }

    /// 预置归档记录（archived_at 固定）。
    fn seed_archived(&self, id: &str, name: &str, created_at: i64, archived_at: i64) {
        self.seed_record(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at,
            status: ChangeStatus::Archived,
            archived_at: Some(archived_at),
            active_phase: None,
            worktree: None,
            base_commit: None,
        });
    }

    /// 任意形态记录种子（worktree 执行锚面）。
    fn seed_record(&self, record: ChangeStateRecord) {
        self.store.set_record(record);
    }

    /// 任意 workflow_type 的建档记录种子（两值分派 / 未知类型断言面）。
    fn seed_typed(&self, id: &str, name: &str, workflow_type: &str) {
        self.seed_record(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: workflow_type.to_owned(),
            created_at: T0,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        });
    }

    fn push_entry(&self, entry: PhaseStateRecord) {
        self.store.push_entry(entry);
    }

    /// active 树内 change 目录写文件（目录名 = 记录 name 供给值——磁盘面零
    /// id 语义）。
    fn file(&self, dir_name: &str, rel: &str, content: &str) {
        self.write_dir(self.layout.changes_root.join(dir_name), rel, content);
    }

    /// archive 树内目录写文件（目录名 = `YYYY-MM-DD-<name>` 前缀形态）。
    fn archive_file(&self, dir_name: &str, rel: &str, content: &str) {
        self.write_dir(self.layout.archive_root.join(dir_name), rel, content);
    }

    fn write_dir(&self, dir: PathBuf, rel: &str, content: &str) {
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        if let Some(parent) = dir.join(rel).parent() {
            fs::create_dir_all(parent).expect("创建子目录失败");
        }
        fs::write(dir.join(rel), content).expect("写文件失败");
    }

    /// id 寻址取详情（未找到即 panic——正向断言面）。
    fn detail(&self, id: &str) -> super::ChangeDetail {
        self.detail_opt(id)
            .unwrap_or_else(|| panic!("应能定位 change id={id}"))
    }

    fn detail_opt(&self, id: &str) -> Option<super::ChangeDetail> {
        change_detail(&self.layout, &self.store, id)
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// ---------------------------------------------------------------------------
// 假件 store：id 键记录表 + 归属键过滤的条目 / run 读半边（可编程序列），其余
// unimplemented（越权触写即 panic）。
// ---------------------------------------------------------------------------

struct DetailStore {
    /// 建档记录表（id 键入——`get_change` 恒 id 直查，零 name 语义）。
    records: Mutex<Vec<ChangeStateRecord>>,
    entries: Mutex<Vec<PhaseStateRecord>>,
    /// run 运行史读半边（可编程序列——详情聚合 runs 投影的输入面）。
    runs: Mutex<Vec<RunStateRecord>>,
    /// 执行图节点读半边（可编程序列）。
    run_nodes: Mutex<Vec<RunNodeStateRecord>>,
    /// 执行图边读半边（可编程序列）。
    run_edges: Mutex<Vec<RunEdgeStateRecord>>,
    /// 可编程 Err 注入：命中即对应读面返回 Err（降级空数组断言的输入面）。
    runs_fault: Mutex<bool>,
    nodes_fault: Mutex<bool>,
    edges_fault: Mutex<bool>,
}

impl DetailStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            entries: Mutex::new(Vec::new()),
            runs: Mutex::new(Vec::new()),
            run_nodes: Mutex::new(Vec::new()),
            run_edges: Mutex::new(Vec::new()),
            runs_fault: Mutex::new(false),
            nodes_fault: Mutex::new(false),
            edges_fault: Mutex::new(false),
        }
    }

    fn set_record(&self, record: ChangeStateRecord) {
        self.records.lock().expect("记录锁不可中毒").push(record);
    }

    fn push_entry(&self, entry: PhaseStateRecord) {
        self.entries.lock().expect("条目锁不可中毒").push(entry);
    }

    /// run 运行史种子（乱序可注入——聚合排序断言面）。
    fn push_run(&self, record: RunStateRecord) {
        self.runs.lock().expect("run 锁不可中毒").push(record);
    }

    /// 执行图节点种子（run_id 圈定键）。
    fn push_run_node(&self, record: RunNodeStateRecord) {
        self.run_nodes
            .lock()
            .expect("run node 锁不可中毒")
            .push(record);
    }

    /// 执行图边种子（run_id 圈定键）。
    fn push_run_edge(&self, record: RunEdgeStateRecord) {
        self.run_edges
            .lock()
            .expect("run edge 锁不可中毒")
            .push(record);
    }

    /// 读面 Err 注入（list_runs / list_run_nodes / list_run_edges 各自独立）。
    fn arm_runs_fault(&self) {
        *self.runs_fault.lock().expect("fault 锁不可中毒") = true;
    }

    fn arm_nodes_fault(&self) {
        *self.nodes_fault.lock().expect("fault 锁不可中毒") = true;
    }

    fn arm_edges_fault(&self) {
        *self.edges_fault.lock().expect("fault 锁不可中毒") = true;
    }
}

impl ChangeStateStore for DetailStore {
    fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self
            .records
            .lock()
            .expect("记录锁不可中毒")
            .iter()
            .find(|record| record.id == id)
            .cloned())
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_phase_records(&self, change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        Ok(self
            .entries
            .lock()
            .expect("条目锁不可中毒")
            .iter()
            .filter(|entry| entry.change_id == change_id)
            .cloned()
            .collect())
    }

    fn list_steps(
        &self,
        _change_id: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn create_change_record(&self, _record: ChangeStateRecord) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn delete_change_record(&self, _id: &str) -> Result<bool, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn start_phase(
        &self,
        _change_id: &str,
        _phase: &str,
        _now: i64,
    ) -> Result<crate::state::PhaseStartState, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn log_phase(&self, _command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn apply_backtrack(&self, _command: &BacktrackCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn amend_decision_session(
        &self,
        _change_id: &str,
        _phase: &str,
        _session_id: &str,
    ) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn set_archived(&self, _id: &str, _archived_at: i64) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn append_step(&self, _command: &StepCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_runs(&self, change_id: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
        if *self.runs_fault.lock().expect("fault 锁不可中毒") {
            return Err(StoreFault::Db("list_runs 注入失败".to_owned()));
        }
        Ok(self
            .runs
            .lock()
            .expect("run 锁不可中毒")
            .iter()
            .filter(|record| record.change_id == change_id)
            .cloned()
            .collect())
    }

    fn list_run_nodes(&self, run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreFault> {
        if *self.nodes_fault.lock().expect("fault 锁不可中毒") {
            return Err(StoreFault::Db("list_run_nodes 注入失败".to_owned()));
        }
        Ok(self
            .run_nodes
            .lock()
            .expect("run node 锁不可中毒")
            .iter()
            .filter(|record| record.run_id == run_id)
            .cloned()
            .collect())
    }

    fn list_run_edges(&self, run_id: &str) -> Result<Vec<RunEdgeStateRecord>, StoreFault> {
        if *self.edges_fault.lock().expect("fault 锁不可中毒") {
            return Err(StoreFault::Db("list_run_edges 注入失败".to_owned()));
        }
        Ok(self
            .run_edges
            .lock()
            .expect("run edge 锁不可中毒")
            .iter()
            .filter(|record| record.run_id == run_id)
            .cloned()
            .collect())
    }

    fn run_start(&self, _command: &RunStartCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_finish(&self, _command: &RunFinishCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }
}

/// 确定性时间戳基（UTC unix millis）。
const T0: i64 = 1_727_000_000_000;
fn t(n: u32) -> i64 {
    T0 + i64::from(n) * 1000
}

fn item(name: &str, pass: bool, evidence: &str) -> ChecklistItem {
    ChecklistItem {
        item: name.to_owned(),
        pass,
        evidence: evidence.to_owned(),
    }
}

/// 内存构造一条相位评估条目（聚合输入面全字段可控；change_id = 归属键）。
#[allow(clippy::too_many_arguments)]
fn entry(
    change_id: &str,
    phase: &str,
    attempt: u32,
    verdict: Verdict,
    report: &str,
    checklist: Vec<ChecklistItem>,
    executor: Option<&str>,
    evaluator: Option<&str>,
    start_at: Option<i64>,
    timestamp: i64,
) -> PhaseStateRecord {
    PhaseStateRecord {
        id: i64::from(attempt),
        change_id: change_id.to_owned(),
        phase: phase.to_owned(),
        attempt,
        verdict,
        report: report.to_owned(),
        checklist,
        skipped: false,
        stale: false,
        backtrack_to: None,
        backtrack_reason: None,
        executor_session_id: executor.map(str::to_owned),
        evaluator_session_id: evaluator.map(str::to_owned),
        decision_session_id: None,
        start_at,
        timestamp,
    }
}

/// 固定 9 站相位名序列（流水线站序断言直读；archive 为末列）。
fn pipeline_phases(detail: &super::ChangeDetail) -> Vec<&str> {
    detail
        .pipeline
        .iter()
        .map(|station| station.phase.as_str())
        .collect()
}

// ---------------------------------------------------------------------------
// 正向：归档 change 全状态面（id 寻址 → status / 9 站 / runs 全量出线）
// ---------------------------------------------------------------------------

/// 建档 archived（记录 id 与裸名）+ 磁盘目录带 `YYYY-MM-DD-` 前缀 →
/// change_detail(layout, store, id) → status=archived、9 站 pipeline、runs 全
/// 量出线（AC-2 主锚——记录恒可达、文档形态错配链结构性退役）。
#[test]
fn 归档change全状态面_十站流水线与runs全量出线() {
    let env = Env::new("archive-full-state");
    env.seed_archived(ID_ARCHIVED, "archived-change", T0, t(30));
    env.push_entry(entry(
        ID_ARCHIVED,
        "proposal",
        1,
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        Some("ses-exec-p1"),
        Some("ses-eval-p1"),
        Some(t(1)),
        t(1),
    ));
    env.push_entry(entry(
        ID_ARCHIVED,
        "implement",
        1,
        Verdict::Pass,
        "实现完成",
        Vec::new(),
        None,
        None,
        Some(t(4)),
        t(4),
    ));
    env.store.push_run(RunStateRecord {
        run_id: "run-arch-1".to_owned(),
        change_id: ID_ARCHIVED.to_owned(),
        status: RunStatus::Completed,
        reason: Some("All phases have passed.".to_owned()),
        started_at: t(10),
        finished_at: Some(t(19)),
    });
    env.store.push_run_node(RunNodeStateRecord {
        run_id: "run-arch-1".to_owned(),
        change_id: ID_ARCHIVED.to_owned(),
        phase: "implement".to_owned(),
        attempt: 1,
        step: RunStepKind::Executor,
        instance_no: 0,
        status: RunStepStatus::Passed,
        session_id: Some("ses-exec-1".to_owned()),
        detail: None,
        output_ref: None,
        timestamp: t(19),
    });
    env.store.push_run_edge(RunEdgeStateRecord {
        run_id: "run-arch-1".to_owned(),
        change_id: ID_ARCHIVED.to_owned(),
        from_node_key: "implement:1:executor:0".to_owned(),
        to_node_key: "implement:1:phase_log:0".to_owned(),
        edge_kind: RunEdgeKind::Forward,
        timestamp: t(19),
    });
    // 磁盘归档目录带日期前缀（目录名 = 前缀 + 记录裸名——磁盘面供给值）
    env.archive_file("2026-01-05-archived-change", "proposal.md", "# 归档提案");
    env.archive_file("2026-01-05-archived-change", "tasks.md", "- [x] 一步\n");

    let detail = env.detail(ID_ARCHIVED);

    assert_eq!(
        detail.status,
        Some(ChangeStatus::Archived),
        "status=archived"
    );
    assert_eq!(
        detail.source,
        super::ChangeSource::Archive,
        "前缀目录命中 → Archive"
    );
    assert_eq!(
        pipeline_phases(&detail),
        REQUIREMENT_PHASES.to_vec(),
        "9 站 pipeline 全量出线（MUST NOT 落文档形态空面）"
    );
    assert_eq!(detail.pipeline[0].attempts.len(), 1, "proposal 站条目在案");
    assert_eq!(detail.pipeline[3].attempts[0].report, "实现完成");
    assert_eq!(detail.runs.len(), 1, "runs 全量出线（非空面）");
    assert_eq!(detail.runs[0].run_id, "run-arch-1");
    assert_eq!(detail.runs[0].nodes.len(), 1, "run 节点随行投影");
    assert_eq!(detail.runs[0].edges.len(), 1, "run 边随行投影");
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "markdown-doc" && artifact.source == "proposal.md"),
        "产物自前缀归档目录发现，实际: {:?}",
        detail.artifacts
    );
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "tasks-progress"),
        "tasks.md 勾选计数照常可达"
    );
}

// ---------------------------------------------------------------------------
// 异常：未知 id 未找到（零目录回退 / 零空流水线文档形态）
// ---------------------------------------------------------------------------

/// db 无该 id（磁盘任意目录在场——裸名 / 前缀名形态，含惰性字节样本）→
/// `None`；MUST NOT 回退磁盘目录解析、MUST NOT 返回空流水线文档形态（AC-4
/// 主锚）；在档记录照常可达（反证 None 非全局空面）。
#[test]
fn 未知id未找到_磁盘目录在场亦none零目录回退() {
    let env = Env::new("unknown-id");
    env.file("disk-only-bare", "proposal.md", "# 存量提案");
    env.archive_file(
        "2026-01-05-disk-only-prefixed",
        "workflow.json",
        "{ CORRUPT_MARKER_桌面不解析此字节 }",
    );
    // 另一在档记录（id / name 均与查询串无关——记录在场不牵涉未知 id）
    env.seed(ID_GHOST, "recorded-change", T0, None);
    env.file("recorded-change", "proposal.md", "# 在档提案");

    for id in [
        ID_UNKNOWN,
        "disk-only-bare",
        "2026-01-05-disk-only-prefixed",
    ] {
        assert!(
            env.detail_opt(id).is_none(),
            "db 无该 id {id:?} → None（零目录回退零文档形态）"
        );
    }
    assert_eq!(
        env.detail(ID_GHOST).status,
        Some(ChangeStatus::Active),
        "在档记录照常可达"
    );
}

// ---------------------------------------------------------------------------
// 正向：ChangeDetail 身份面（id 首字段 + 裸名）与 created 单源
// ---------------------------------------------------------------------------

/// id 首字段出线（与寻址入参逐字一致）、name 恒自记录直读（归档前缀形态零
/// 出现在 name——磁盘目录名不参与出线）。
#[test]
fn detail身份面_id首字段出线与裸名且归档前缀不入名() {
    let env = Env::new("identity-face");
    env.seed_archived(ID_ARCHIVED, "archived-bare", T0, t(9));
    env.archive_file("2026-03-05-archived-bare", "proposal.md", "# 归档提案");

    let detail = env.detail(ID_ARCHIVED);

    assert_eq!(detail.id, ID_ARCHIVED, "id 与寻址入参逐字一致");
    assert_eq!(detail.name, "archived-bare", "name 恒裸名（自记录直读）");
    assert!(
        !detail.name.contains("2026-03-05"),
        "归档日期前缀零出现在 name，实际: {}",
        detail.name
    );
    let wire = serde_json::to_string(&detail).expect("详情序列化应成功");
    assert!(
        wire.starts_with(&format!("{{\"id\":\"{ID_ARCHIVED}\"")),
        "id 为首字段（DTO 字段序 = 线面键序）"
    );
}

/// change_detail title 直读（新增——AC-2）：假 store 注入建档记录（title ≠
/// name）→ 详情 `ChangeDetail.title == record.title`、`name` 恒裸名零污染；
/// serde 线面含 title 键（camelCase 不变量）。
#[test]
fn detail_title直读建档记录且name零污染() {
    let env = Env::new("detail-title");
    env.seed_record(ChangeStateRecord {
        id: ID_MULTI.to_owned(),
        name: "title-change".to_owned(),
        title: "人类可读标题".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: T0,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    });
    env.file("title-change", "proposal.md", "# 提案");

    let detail = env.detail(ID_MULTI);

    assert_eq!(detail.title, "人类可读标题", "title 自记录直读");
    assert_ne!(detail.title, detail.name, "title 与 name 双字段可辨");
    assert_eq!(detail.name, "title-change", "name 恒裸名（title 零污染）");
    assert_eq!(detail.id, ID_MULTI, "id 恒身份锚");
    let value = serde_json::to_value(&detail).expect("详情出线应成功");
    assert_eq!(
        value["title"],
        serde_json::json!("人类可读标题"),
        "线面含 title 键"
    );
    assert_eq!(value["name"], serde_json::json!("title-change"));
}

/// 未知 id None 降级（持衡）：db 无该 id → 详情恒 `None`（零磁盘回退零文档
/// 形态——既有语义零改动，title 字段面不影响寻址语义）。
#[test]
fn detail未知id恒none_零磁盘回退() {
    let env = Env::new("detail-unknown-title");
    env.seed(ID_BARE, "present-change", T0, None);
    env.file("present-change", "proposal.md", "# 提案");

    assert!(
        env.detail_opt(ID_UNKNOWN).is_none(),
        "未建档 id 恒 None（零目录回退）"
    );
}

/// created 恒自 `created_at`（单源）：归档记录 + 磁盘 `YYYY-MM-DD-` 前缀目录，
/// 前缀日与 created_at 日各异 → created = created_at 日（目录前缀日零回退）。
#[test]
fn created单源_恒取created_at() {
    let env = Env::new("created-single-source");
    env.seed_archived(ID_ARCHIVED, "created-change", T0, t(9));
    env.archive_file("2026-01-05-created-change", "proposal.md", "# 归档提案");

    let detail = env.detail(ID_ARCHIVED);

    assert_eq!(detail.status, Some(ChangeStatus::Archived));
    assert_eq!(
        detail.created.as_deref(),
        Some("2024-09-22"),
        "created 恒自 created_at（T0 = 2024-09-22 UTC）"
    );
    assert_ne!(
        detail.created.as_deref(),
        Some("2026-01-05"),
        "磁盘目录日期前缀零回退"
    );
}

// ---------------------------------------------------------------------------
// 正向：db 重组流水线（9 站 / attempt 升序 / checklist 内联 / 槽位与
// stale / backtrack 字段透出）
// ---------------------------------------------------------------------------

/// 记录序列（多相位多 attempt + checklist + 回跳 stale 标记）→ 9 站流水线
/// 重组：站序固定、attempt 升序、checklist 随行、槽位三列直读、stale /
/// backtrack_to / backtrack_reason 字段透出（id 寻址——记录供给语义重证）。
#[test]
fn db重组流水线_十站与attempt升序与checklist内联() {
    let env = Env::new("pipeline");
    env.seed(
        ID_MULTI,
        "multi-change",
        T0,
        Some(ActivePhaseState {
            phase: "test-gen".to_owned(),
            attempt: 1,
            start_at: t(5),
        }),
    );
    env.push_entry(entry(
        ID_MULTI,
        "proposal",
        1,
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        Some(t(1)),
        t(1),
    ));
    env.push_entry(entry(
        ID_MULTI,
        "dev-design",
        1,
        Verdict::Fail,
        "首轮未过",
        vec![item("组件表完整", false, "缺 renderers 职责")],
        Some("ses-exec-a1"),
        None,
        Some(t(2)),
        t(2),
    ));
    // attempt 2：回跳目标 → stale 翻转形态（stale 位由回跳落库半边写入）
    let mut pass_a2 = entry(
        ID_MULTI,
        "dev-design",
        2,
        Verdict::Pass,
        "第二次通过",
        vec![item("组件表完整", true, "五组件齐全")],
        Some("ses-exec-a2"),
        Some("ses-eval-a2"),
        Some(t(3)),
        t(3),
    );
    pass_a2.stale = true;
    env.push_entry(pass_a2);
    // implement：回跳发起相位 → backtrack 标记随条目
    let mut implement = entry(
        ID_MULTI,
        "implement",
        1,
        Verdict::Pass,
        "实现完成",
        Vec::new(),
        None,
        None,
        Some(t(4)),
        t(4),
    );
    implement.stale = true;
    implement.backtrack_to = Some("dev-design".to_owned());
    implement.backtrack_reason = Some("设计返工：缺产物区组件".to_owned());
    env.push_entry(implement);
    env.file("multi-change", "proposal.md", "# 提案");

    let detail = env.detail(ID_MULTI);

    // 9 站全量输出，顺序固定
    assert_eq!(pipeline_phases(&detail), REQUIREMENT_PHASES.to_vec());

    // dev-design 单站折叠两条 attempt，按 attempt 升序、checklist 随行
    let dev_design = &detail.pipeline[1];
    assert_eq!(dev_design.phase, "dev-design");
    let attempts: Vec<Option<u32>> = dev_design
        .attempts
        .iter()
        .map(|record| record.attempt)
        .collect();
    assert_eq!(attempts, vec![Some(1), Some(2)], "attempt 升序");
    assert_eq!(dev_design.attempts[0].report, "首轮未过");
    assert_eq!(dev_design.attempts[0].verdict, Verdict::Fail);
    assert_eq!(dev_design.attempts[0].checklist.len(), 1);
    assert_eq!(dev_design.attempts[0].checklist[0].item, "组件表完整");
    assert!(!dev_design.attempts[0].checklist[0].pass);
    assert_eq!(
        dev_design.attempts[0].executor_session_id.as_deref(),
        Some("ses-exec-a1")
    );
    assert_eq!(dev_design.attempts[0].evaluator_session_id, None);
    // attempt 2：stale 透出、槽位三列直读
    assert!(
        dev_design.attempts[1].stale,
        "回跳目标最新 pass 条目 stale 位透出"
    );
    assert_eq!(dev_design.attempts[1].report, "第二次通过");
    assert_eq!(
        dev_design.attempts[1].executor_session_id.as_deref(),
        Some("ses-exec-a2")
    );
    assert_eq!(
        dev_design.attempts[1].evaluator_session_id.as_deref(),
        Some("ses-eval-a2")
    );
    assert_eq!(dev_design.attempts[1].decision_session_id, None);

    // implement 站：backtrack 字段随条目透出
    let implement = &detail.pipeline[3];
    assert_eq!(implement.attempts.len(), 1);
    assert_eq!(
        implement.attempts[0].backtrack_to.as_deref(),
        Some("dev-design")
    );
    assert_eq!(
        implement.attempts[0].backtrack_reason.as_deref(),
        Some("设计返工：缺产物区组件")
    );

    // proposal 站零标记
    assert!(!detail.pipeline[0].attempts[0].stale);
    assert!(detail.pipeline[0].attempts[0].backtrack_to.is_none());

    // 运行态 / 状态面
    let active = detail.active_phase.as_ref().expect("active_phase 应在场");
    assert_eq!(active.phase, "test-gen");
    assert_eq!(active.attempt, 1);
    assert!(active.start_at.is_some(), "start_at 出 ISO 串");
    assert_eq!(detail.status, Some(ChangeStatus::Active));
    assert_eq!(detail.source, super::ChangeSource::Active);
    assert!(detail.created.is_some());
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "markdown-doc" && artifact.source == "proposal.md"),
        "产物清单随磁盘树在场"
    );
}

/// 未覆盖相位站点仍在且 attempts 为空（9 站全量输出，顺序不依赖落行排列）。
#[test]
fn 未覆盖相位站点仍在且attempts为空() {
    let env = Env::new("partial");
    env.seed(ID_MULTI, "partial-change", T0, None);
    env.push_entry(entry(
        ID_MULTI,
        "proposal",
        1,
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        Some(t(1)),
        t(1),
    ));
    env.file("partial-change", "tasks.md", "- [x] 完成\n");

    let detail = env.detail(ID_MULTI);

    assert_eq!(
        pipeline_phases(&detail),
        REQUIREMENT_PHASES.to_vec(),
        "顺序固定，不依赖条目排列"
    );
    let test_gen = &detail.pipeline[4]; // "test-gen"
    assert_eq!(test_gen.phase, "test-gen");
    assert!(test_gen.attempts.is_empty(), "未覆盖站 attempts 为空序列");
    assert_eq!(
        detail.pipeline[0].attempts.len(),
        1,
        "有记录的站不受空站影响"
    );
}

// ---------------------------------------------------------------------------
// 正向：时间出线单点（ISO 串 + null；epoch 边界）
// ---------------------------------------------------------------------------

/// created_at / start_at / timestamp 的 i64 millis → RFC3339 ISO 串（epoch 0
/// → `1970-01-01T00:00:00Z` 口径）；active_phase 缺席 → wire null（冻结契约
/// 半边，golden 守卫绿的前提断言）。转换收 queries 单点。
#[test]
fn 时间出线iso串_epoch零口径与null留位() {
    let env = Env::new("epoch-zero");
    env.seed(ID_ZERO_TS, "zero-ts", 0, None);
    env.push_entry(entry(
        ID_ZERO_TS,
        "proposal",
        1,
        Verdict::Pass,
        "通过",
        Vec::new(),
        None,
        None,
        Some(0),
        0,
    ));
    env.file("zero-ts", "proposal.md", "# 提案");

    let detail = env.detail(ID_ZERO_TS);
    assert_eq!(
        detail.created.as_deref(),
        Some("1970-01-01"),
        "created = epoch 日"
    );

    let record = &detail.pipeline[0].attempts[0];
    assert_eq!(record.timestamp.as_deref(), Some("1970-01-01T00:00:00Z"));
    assert_eq!(record.start_at.as_deref(), Some("1970-01-01T00:00:00Z"));

    let value = serde_json::to_value(&detail).expect("线面序列化应成功");
    assert_eq!(value["activePhase"], serde_json::Value::Null);
    assert_eq!(value["status"], "active", "状态面小写线值直出");
    assert_eq!(
        value["pipeline"][0]["attempts"][0]["timestamp"], "1970-01-01T00:00:00Z",
        "时间戳恒 ISO 串（timestamp 恒在位）"
    );
}

// ---------------------------------------------------------------------------
// 边界：槽位全缺 + start_at None → null 留位
// ---------------------------------------------------------------------------

/// PhaseRecord 三会话槽位 None 且 start_at None → AttemptRecord 四值 null 不
/// 报错且 wire 四键恒在场（纯 derive 零字段属性口径；既有语义持衡）。
/// 注记：`start_at=None` 仅假件可达——真实 store 落账恒回填 active start_at
///（log_change_phase `command.start_at.or(Some(active_start_at))`）。
#[test]
fn 槽位与start_at全缺_wire四键null恒在场() {
    let env = Env::new("slots-null");
    env.seed(ID_BARE, "bare", T0, None);
    env.push_entry(entry(
        ID_BARE,
        "proposal",
        1,
        Verdict::Pass,
        "提案通过",
        Vec::new(),
        None,
        None,
        None, // start_at=None（缺省形态）
        T0,
    ));
    env.file("bare", "proposal.md", "# 提案");

    let detail = env.detail(ID_BARE);
    let record = &detail.pipeline[0].attempts[0];
    assert_eq!(record.executor_session_id, None);
    assert_eq!(record.evaluator_session_id, None);
    assert_eq!(record.decision_session_id, None);
    assert_eq!(record.start_at, None, "start_at=None → null 不报错");

    let value = serde_json::to_value(&detail).expect("线面序列化应成功");
    let wire = &value["pipeline"][0]["attempts"][0];
    for key in [
        "executorSessionId",
        "evaluatorSessionId",
        "decisionSessionId",
        "startAt",
    ] {
        assert_eq!(
            wire.get(key),
            Some(&serde_json::Value::Null),
            "wire 键 {key} 恒在场（null 不省略）"
        );
    }
}

/// skipped 与 stale 标记随条目透出（聚合直读，不派生不改写）。
#[test]
fn skipped与stale标记随条目透出() {
    let env = Env::new("flags");
    env.seed(ID_FLAGS, "flags", T0, None);
    let mut skipped = entry(
        ID_FLAGS,
        "test-gen",
        1,
        Verdict::Pass,
        "跳过项",
        Vec::new(),
        None,
        None,
        Some(t(1)),
        t(1),
    );
    skipped.skipped = true;
    env.push_entry(skipped);
    let mut stale = entry(
        ID_FLAGS,
        "test-execution",
        1,
        Verdict::Pass,
        "过期项",
        Vec::new(),
        None,
        None,
        Some(t(2)),
        t(2),
    );
    stale.stale = true;
    env.push_entry(stale);
    env.file("flags", "proposal.md", "# 提案");

    let detail = env.detail(ID_FLAGS);
    // test-gen = 第 5 站（下标 4），test-execution = 第 6 站（下标 5）
    assert!(detail.pipeline[4].attempts[0].skipped);
    assert!(!detail.pipeline[4].attempts[0].stale);
    assert!(detail.pipeline[5].attempts[0].stale);
    assert!(!detail.pipeline[5].attempts[0].skipped);
}

// ---------------------------------------------------------------------------
// 边界：建档记录恒可达（定位 miss 面）
// ---------------------------------------------------------------------------

/// db 有档但定位全 miss（建档记录恒可达——worktree 未 merge / 被删，主仓两树
/// 亦未命中）→ `Some`（dir 缺席、产物清单空、状态面在）。
#[test]
fn 建档记录恒可达_定位miss状态面在() {
    let env = Env::new("ghost-record");
    env.file("real", "proposal.md", "# 提案");

    env.seed(ID_GHOST, "ghost-with-record", T0, None);
    let detail = env
        .detail_opt(ID_GHOST)
        .expect("建档记录恒可达详情（定位 miss 不虚构 None）");
    assert_eq!(detail.status, Some(ChangeStatus::Active), "状态面在");
    assert_eq!(
        detail.source,
        super::ChangeSource::Active,
        "source 自记录映射"
    );
    assert!(detail.artifacts.is_empty(), "定位 miss 产物清单空");
    assert_eq!(detail.pipeline.len(), 9, "建档 9 站流水线照常");
}

// ---------------------------------------------------------------------------
// worktree 维度：worktree 路径出线与产物自 worktree 发现 / 手动删恒可达
// （source 映射）/ legacy null 出线（serde 键恒在场）
// ---------------------------------------------------------------------------

impl Env {
    /// 带 worktree 执行锚的建档种子（merge 前主仓两树未命中的形态）。
    fn seed_worktree_record(&self, id: &str, name: &str, worktree: &Path) {
        self.seed_record(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: T0,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: Some(worktree.to_string_lossy().into_owned()),
            base_commit: Some("0000000000000000000000000000000000000001".to_owned()),
        });
    }

    /// worktree 内 change 目录写文件（worktree 树夹具；目录名 = 记录 name）。
    fn worktree_file(&self, worktree: &Path, dir_name: &str, rel: &str, content: &str) {
        let dir = resolve(worktree).changes_root.join(dir_name);
        fs::create_dir_all(&dir).expect("创建 worktree change 目录失败");
        if let Some(parent) = dir.join(rel).parent() {
            fs::create_dir_all(parent).expect("创建 worktree 子目录失败");
        }
        fs::write(dir.join(rel), content).expect("写 worktree 文件失败");
    }
}

/// worktree 路径出线：记录 `worktree=Some(绝对路径)` + worktree 目录树在场
///（主仓两树未命中）→ detail Some、`worktree` 与库内记录值逐字一致、artifacts
/// 自 worktree 目录发现（目录名 / worktree 路径恒自记录供给——id 寻址）。
#[test]
fn worktree路径出线_与库内记录逐字一致且产物自worktree发现() {
    let env = Env::new("wt-detail");
    let worktree =
        std::env::temp_dir().join(format!("workflow-detail-wt-{}-out", std::process::id()));
    let _ = fs::remove_dir_all(&worktree);
    env.seed_worktree_record(ID_WORKTREE, "wt-change", &worktree);
    env.worktree_file(&worktree, "wt-change", "proposal.md", "# worktree 内提案");
    env.worktree_file(&worktree, "wt-change", "tasks.md", "- [x] 一步\n");

    let detail = env.detail(ID_WORKTREE);

    // worktree 与库内记录值逐字一致（直读透出零改写）
    let record = env
        .store
        .get_change(ID_WORKTREE)
        .expect("假件读半边应可用")
        .expect("建档在案");
    assert_eq!(
        detail.worktree.as_deref(),
        record.worktree.as_deref(),
        "detail.worktree 与库内记录逐字一致"
    );
    assert_eq!(
        detail.worktree.as_deref(),
        Some(worktree.to_string_lossy().as_ref()),
        "worktree 绝对路径原样出线"
    );
    // 产物自 worktree 目录发现（主仓两树未命中——清单来源唯一 worktree）
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "markdown-doc" && artifact.source == "proposal.md"),
        "产物清单命中 worktree 内文件，实际: {:?}",
        detail.artifacts
    );
    assert_eq!(
        detail.source,
        super::ChangeSource::Active,
        "worktree 回退命中 → Active"
    );
    let _ = fs::remove_dir_all(&worktree);
}

/// worktree 手动删恒可达：记录在场（worktree=Some）+ 定位全 miss（worktree
/// 目录被删、未 merge）→ 仍 `Some`：流水线 9 站、产物清单空、状态面在、
/// `source` 自 record.status 映射（active → Active）——「建档记录恒可达详情」
/// 字面（worktree=Some 形态）。
#[test]
fn worktree手动删_建档记录恒可达source自status映射() {
    let env = Env::new("wt-gone");
    let worktree =
        std::env::temp_dir().join(format!("workflow-detail-wt-{}-gone", std::process::id()));
    let _ = fs::remove_dir_all(&worktree); // 目录被删形态（不存在）
    env.seed_worktree_record(ID_WT_GONE, "gone-wt-change", &worktree);

    let detail = env
        .detail_opt(ID_WT_GONE)
        .expect("建档记录恒可达详情（worktree 被删不虚构 None）");

    assert_eq!(detail.status, Some(ChangeStatus::Active), "状态面在");
    assert_eq!(
        detail.pipeline.len(),
        REQUIREMENT_PHASES.len(),
        "建档 9 站流水线照常"
    );
    assert!(detail.artifacts.is_empty(), "定位 miss 产物清单空");
    assert_eq!(
        detail.source,
        super::ChangeSource::Active,
        "source 自 record.status 映射（active → Active）"
    );
    assert_eq!(
        detail.worktree.as_deref(),
        Some(worktree.to_string_lossy().as_ref()),
        "worktree 执行锚照常出线（删除事实不改写记录）"
    );
}

/// source 映射双态：记录 archived + 定位 miss → `source == Archive`（archived
/// 记录主仓 miss 的呈现面）。
#[test]
fn source映射_archived记录定位miss映射archive() {
    let env = Env::new("wt-source-map");
    let worktree =
        std::env::temp_dir().join(format!("workflow-detail-wt-{}-map", std::process::id()));
    let _ = fs::remove_dir_all(&worktree);
    env.seed_archived(ID_ARCH_GONE, "archived-gone", T0, T0 + 1_000);
    // 归档记录带 worktree 的存量形态（归档前建域的 change 手动删目录）
    env.seed_record(ChangeStateRecord {
        worktree: Some(worktree.to_string_lossy().into_owned()),
        ..env
            .store
            .get_change(ID_ARCH_GONE)
            .expect("假件读半边应可用")
            .expect("建档在案")
    });

    let detail = env.detail(ID_ARCH_GONE);

    assert_eq!(
        detail.source,
        super::ChangeSource::Archive,
        "archived 记录定位 miss → source 映射 Archive"
    );
    assert_eq!(detail.status, Some(ChangeStatus::Archived), "状态面随记录");
}

/// legacy null 出线：记录 `worktree=None` → detail `worktree` 出线 null、产物
/// 解析走主仓两树既有语义（目录名 = 记录 name 供给值）；serde 线面 `worktree`
/// 键恒在场（null 不省键——冻结契约）。
#[test]
fn legacy记录worktree出线null且serde键恒在场() {
    // legacy 建档：worktree=None + 主仓目录在场（既有语义解析）
    let env = Env::new("legacy-null");
    env.seed(ID_LEGACY, "legacy-change", T0, None);
    env.file("legacy-change", "proposal.md", "# 主仓提案");

    let detail = env.detail(ID_LEGACY);
    assert_eq!(detail.worktree, None, "legacy 记录 worktree 出线 None");

    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert!(
        value.get("worktree").is_some(),
        "serde 线面 worktree 键恒在场（None → null 不省键）"
    );
    assert_eq!(value.get("worktree"), Some(&serde_json::Value::Null));
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.source == "proposal.md"),
        "产物解析走主仓两树既有语义"
    );
}

// ---------------------------------------------------------------------------
// 详情 run 史出线：runs / nodes / edges 投影（AC-3 / AC-6 / AC-9）
// ---------------------------------------------------------------------------

/// run 运行史主行 fixture（started_at / finished_at / reason / status 显式注入）。
fn run_row(change_id: &str, run_id: &str, started_at: i64, status: RunStatus) -> RunStateRecord {
    RunStateRecord {
        run_id: run_id.to_owned(),
        change_id: change_id.to_owned(),
        status,
        reason: None,
        started_at,
        finished_at: None,
    }
}

/// 执行图节点行 fixture（默认 implement 相位首次执行；字段经结构体更新覆盖）。
fn run_node_row(run_id: &str, change_id: &str, step: RunStepKind) -> RunNodeStateRecord {
    RunNodeStateRecord {
        run_id: run_id.to_owned(),
        change_id: change_id.to_owned(),
        phase: "implement".to_owned(),
        attempt: 1,
        step,
        instance_no: 0,
        status: RunStepStatus::Passed,
        session_id: None,
        detail: None,
        output_ref: None,
        timestamp: 1_727_000_060_000,
    }
}

/// 执行图边行 fixture（端点 = 节点身份串）。
fn run_edge_row(
    run_id: &str,
    change_id: &str,
    from: &str,
    to: &str,
    kind: RunEdgeKind,
) -> RunEdgeStateRecord {
    RunEdgeStateRecord {
        run_id: run_id.to_owned(),
        change_id: change_id.to_owned(),
        from_node_key: from.to_owned(),
        to_node_key: to.to_owned(),
        edge_kind: kind,
        timestamp: 1_727_000_060_000,
    }
}

/// 十九值全词汇（节点出线零过滤断言面；含归档 8 步词与拆步新词 mutation_test）。
const ALL_STEP_KINDS: [RunStepKind; 19] = [
    RunStepKind::Executor,
    RunStepKind::Evaluator,
    RunStepKind::Decision,
    RunStepKind::StaticCheck,
    RunStepKind::TestExecution,
    RunStepKind::MutationTest,
    RunStepKind::PhaseStart,
    RunStepKind::PhaseLog,
    RunStepKind::VerdictGate,
    RunStepKind::RetryGate,
    RunStepKind::WhitelistGate,
    RunStepKind::Preflight,
    RunStepKind::Commit,
    RunStepKind::Rebase,
    RunStepKind::ResolveConflicts,
    RunStepKind::Merge,
    RunStepKind::SpecSync,
    RunStepKind::Seal,
    RunStepKind::Finalize,
];

/// 四 kind 全词汇（边出线零过滤断言面）。
const ALL_EDGE_KINDS: [RunEdgeKind; 4] = [
    RunEdgeKind::Forward,
    RunEdgeKind::Feedback,
    RunEdgeKind::Retry,
    RunEdgeKind::Backtrack,
];

/// 两 run 全史出线：runs 两条全史、runId / status / reason / startedAt /
/// finishedAt 逐项投影、时间戳 ISO 串口径与既有字段一致（同 iso_from_millis
/// 单点）；节点按库读序（emit 序）、session_id / detail / output_ref 两态
///（Some 透传、None → null）——归属键 = change id（记录供给语义）。
#[test]
fn 详情run史出线_两run全史投影与时间iso口径() {
    let env = Env::new("run-history-detail");
    env.seed(ID_RUNNED, "runned", T0, None);
    // run-1（completed、有记因）+ run-2（running 在飞、finished_at=None）
    env.store.push_run(RunStateRecord {
        reason: Some("All phases have passed.".to_owned()),
        finished_at: Some(t(19)),
        status: RunStatus::Completed,
        ..run_row(ID_RUNNED, "run-1", t(10), RunStatus::Completed)
    });
    env.store
        .push_run(run_row(ID_RUNNED, "run-2", t(20), RunStatus::Running));
    // run-1 节点：按注入序即库读序（emit 序）
    let mut evaluator = run_node_row("run-1", ID_RUNNED, RunStepKind::Evaluator);
    evaluator.status = RunStepStatus::Failed;
    evaluator.session_id = Some("ses-eval-1".to_owned());
    evaluator.detail = Some("首轮评估 fail".to_owned());
    env.store.push_run_node(evaluator);
    let mut executor = run_node_row("run-1", ID_RUNNED, RunStepKind::Executor);
    executor.session_id = Some("ses-exec-1".to_owned());
    env.store.push_run_node(executor);
    let mut static_check = run_node_row("run-1", ID_RUNNED, RunStepKind::StaticCheck);
    static_check.output_ref = Some("exec-7".to_owned());
    env.store.push_run_node(static_check);
    env.store.push_run_edge(run_edge_row(
        "run-1",
        ID_RUNNED,
        "implement:1:executor:0",
        "implement:1:static_check:0",
        RunEdgeKind::Forward,
    ));
    env.file("runned", "proposal.md", "# 提案");

    let detail = env.detail(ID_RUNNED);

    assert_eq!(detail.runs.len(), 2, "两 run 全史不截");
    // runs 按 started_at 升序（AC-9 前端分层序的前置语义）
    assert_eq!(detail.runs[0].run_id, "run-1");
    assert_eq!(detail.runs[1].run_id, "run-2");

    // run-1 投影：逐项 + ISO 串口径（与 pipeline timestamp 同式）
    let run1 = &detail.runs[0];
    assert_eq!(run1.status, RunStatus::Completed);
    assert_eq!(run1.reason.as_deref(), Some("All phases have passed."));
    let started_iso = run1.started_at.as_deref().expect("startedAt 出线");
    assert!(
        started_iso.ends_with('Z') && started_iso.contains('T'),
        "ISO 串口径（RFC3339），实际: {started_iso}"
    );
    let finished_iso = run1.finished_at.as_deref().expect("finishedAt 出线");
    assert!(
        finished_iso.ends_with("Z") && finished_iso != started_iso,
        "finishedAt 同一转换单点（RFC3339 iso 串，且异于 startedAt）"
    );
    assert_eq!(
        run1.started_at.as_deref(),
        Some("2024-09-22T10:13:30Z"),
        "iso_from_millis 单点数值口径（t(10) = T0+10s）"
    );

    // run-1 节点：库读序 = emit 序 + 两态透传
    let nodes = &run1.nodes;
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].step, RunStepKind::Evaluator);
    assert_eq!(nodes[0].phase, "implement");
    assert_eq!(nodes[0].attempt, 1);
    assert_eq!(nodes[0].instance_no, 0);
    assert_eq!(nodes[0].status, RunStepStatus::Failed);
    assert_eq!(nodes[0].session_id.as_deref(), Some("ses-eval-1"));
    assert_eq!(nodes[0].detail.as_deref(), Some("首轮评估 fail"));
    assert_eq!(nodes[1].step, RunStepKind::Executor);
    assert_eq!(nodes[1].detail, None, "None → null 透传");
    assert_eq!(
        nodes[2].output_ref.as_deref(),
        Some("exec-7"),
        "outputRef 出线（ExecOutput::Recorded 引用）"
    );

    // run-1 边：四 kind 边表直出、端点身份串
    assert_eq!(run1.edges.len(), 1);
    assert_eq!(run1.edges[0].edge_kind, RunEdgeKind::Forward);
    assert_eq!(run1.edges[0].from_node_key, "implement:1:executor:0");
    assert_eq!(run1.edges[0].to_node_key, "implement:1:static_check:0");

    // run-2（在飞起始行）：status=running、finishedAt=null、nodes / edges 恒空
    let run2 = &detail.runs[1];
    assert_eq!(run2.status, RunStatus::Running);
    assert_eq!(run2.finished_at, None, "running 行 finished_at → null");
    assert!(
        run2.nodes.is_empty(),
        "在飞 run 起始行 nodes 恒空（run 清单面运行中可见）"
    );
    assert!(run2.edges.is_empty());

    // wire 面：camelCase 键 + null 不省键（纯 derive 零字段属性口径）
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    let wire_runs = value["runs"].as_array().expect("runs 为数组");
    assert_eq!(wire_runs[1]["runId"], "run-2");
    assert_eq!(
        wire_runs[1]["finishedAt"],
        serde_json::Value::Null,
        "null 不省键"
    );
    assert!(wire_runs[1]["nodes"].as_array().is_some_and(Vec::is_empty));
    assert!(wire_runs[1]["edges"].as_array().is_some_and(Vec::is_empty));
    assert_eq!(wire_runs[0]["nodes"][0]["instanceNo"], 0);
    assert_eq!(wire_runs[0]["nodes"][2]["outputRef"], "exec-7");
    assert_eq!(
        wire_runs[0]["nodes"][1]["outputRef"],
        serde_json::Value::Null,
        "outputRef null 留位不省键"
    );
    assert!(
        !value["runs"][0].as_object().unwrap().contains_key("steps"),
        "steps 键退役（nodes / edges 取代）"
    );
}

/// 节点全词汇出线：一次覆盖十九值词汇的 run → 十九值全词汇在案（含
/// phaseStart / phaseLog / 三门与 mutation_test 拆步新词），零过滤（AC-3 /
/// AC-6 / AC-2 十九值零过滤）。
#[test]
fn 详情节点出线_十九值全词汇零过滤() {
    let env = Env::new("run-nodes-vocab");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.store.push_run(RunStateRecord {
        status: RunStatus::Completed,
        finished_at: Some(t(19)),
        ..run_row(ID_RUNNED, "run-vocab", t(10), RunStatus::Completed)
    });
    for kind in ALL_STEP_KINDS {
        env.store
            .push_run_node(run_node_row("run-vocab", ID_RUNNED, kind));
    }

    let detail = env.detail(ID_RUNNED);
    let nodes = &detail.runs[0].nodes;
    assert_eq!(nodes.len(), 19, "十九值全词汇零过滤（含归档 8 步词与 mutation_test）");
    let projected: Vec<RunStepKind> = nodes.iter().map(|node| node.step).collect();
    assert_eq!(projected, ALL_STEP_KINDS.to_vec());

    // wire 面 snake_case 词逐字（与落库词汇同源单点）
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    let wire_steps: Vec<&str> = value["runs"][0]["nodes"]
        .as_array()
        .expect("nodes 数组")
        .iter()
        .map(|node| node["step"].as_str().expect("step 为串"))
        .collect();
    assert_eq!(
        wire_steps,
        vec![
            "executor",
            "evaluator",
            "decision",
            "static_check",
            "test_execution",
            "mutation_test",
            "phase_start",
            "phase_log",
            "verdict_gate",
            "retry_gate",
            "whitelist_gate",
            "preflight",
            "commit",
            "rebase",
            "resolve_conflicts",
            "merge",
            "spec_sync",
            "seal",
            "finalize",
        ]
    );
}

/// 稳定实例键出线：feedback 重跑两节点同 `(phase, attempt, step)` 而
/// `instanceNo` 递增可辨（AC-4 / AC-6）。
#[test]
fn 详情节点出线_feedback重跑实例号递增() {
    let env = Env::new("run-nodes-instance");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.store.push_run(RunStateRecord {
        status: RunStatus::Completed,
        finished_at: Some(t(19)),
        ..run_row(ID_RUNNED, "run-fb", t(10), RunStatus::Completed)
    });
    // executor 首次（失败）→ static-check 反馈 → executor 修复（新实例）
    let mut first = run_node_row("run-fb", ID_RUNNED, RunStepKind::Executor);
    first.status = RunStepStatus::Failed;
    first.detail = Some("static-check 未过".to_owned());
    env.store.push_run_node(first);
    let mut repaired = run_node_row("run-fb", ID_RUNNED, RunStepKind::Executor);
    repaired.instance_no = 1;
    repaired.detail = Some("修复重跑".to_owned());
    env.store.push_run_node(repaired);

    let detail = env.detail(ID_RUNNED);
    let nodes = &detail.runs[0].nodes;
    assert_eq!(nodes.len(), 2, "同相位同步两实例为独立节点");
    assert_eq!(nodes[0].phase, nodes[1].phase);
    assert_eq!(nodes[0].attempt, nodes[1].attempt);
    assert_eq!(nodes[0].step, nodes[1].step);
    assert_eq!(nodes[0].instance_no, 0);
    assert_eq!(nodes[1].instance_no, 1, "instanceNo 递增可辨");
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert_eq!(value["runs"][0]["nodes"][1]["instanceNo"], 1);
}

/// 边整包出线：四 kind（forward / feedback / retry / backtrack）齐备，
/// `fromNodeKey` / `toNodeKey` 指向节点身份串（AC-3 / AC-6）。
#[test]
fn 详情边出线_四kind齐备与端点身份串() {
    let env = Env::new("run-edges");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.store.push_run(RunStateRecord {
        status: RunStatus::Completed,
        finished_at: Some(t(19)),
        ..run_row(ID_RUNNED, "run-edges", t(10), RunStatus::Completed)
    });
    for (index, kind) in ALL_EDGE_KINDS.iter().enumerate() {
        env.store.push_run_edge(run_edge_row(
            "run-edges",
            ID_RUNNED,
            "proposal:1:executor:0",
            &format!("proposal:1:static_check:{index}"),
            *kind,
        ));
    }

    let detail = env.detail(ID_RUNNED);
    let edges = &detail.runs[0].edges;
    assert_eq!(edges.len(), 4, "四 kind 齐备");
    let kinds: Vec<RunEdgeKind> = edges.iter().map(|edge| edge.edge_kind).collect();
    assert_eq!(kinds, ALL_EDGE_KINDS.to_vec());
    assert_eq!(
        edges[0].from_node_key, "proposal:1:executor:0",
        "端点 = 节点身份串（phase:attempt:step:instance_no）"
    );

    let value = serde_json::to_value(&detail).expect("序列化应成功");
    let wire_kinds: Vec<&str> = value["runs"][0]["edges"]
        .as_array()
        .expect("edges 数组")
        .iter()
        .map(|edge| edge["edgeKind"].as_str().expect("edgeKind 为串"))
        .collect();
    assert_eq!(
        wire_kinds,
        vec!["forward", "feedback", "retry", "backtrack"]
    );
}

/// 在档记录无 run 行 → runs 空数组且 wire 键恒在场；单节点零边 run → edges 空
/// 数组（miss 非错误）；终态 run reason=None → null 出线。
#[test]
fn 详情run史出线_零run空数组与单节点零边() {
    let env = Env::new("run-history-empty");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.file("runned", "proposal.md", "# 提案");

    let detail = env.detail(ID_RUNNED);
    assert!(detail.runs.is_empty(), "在档记录无 run 行 → 空数组");
    assert_eq!(
        detail.pipeline.len(),
        REQUIREMENT_PHASES.len(),
        "记录在档：9 站流水线照常（非文档形态空面）"
    );
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert!(
        value["runs"].as_array().is_some_and(Vec::is_empty),
        "runs 键恒在场（空数组不省键）"
    );

    // 单节点零边 run + reason=None（无记因收口）→ null 出线
    let env2 = Env::new("run-history-no-reason");
    env2.seed(ID_RUNNED, "runned", T0, None);
    env2.store.push_run(RunStateRecord {
        reason: None,
        finished_at: Some(t(19)),
        status: RunStatus::Completed,
        ..run_row(ID_RUNNED, "run-1", t(10), RunStatus::Completed)
    });
    env2.store
        .push_run_node(run_node_row("run-1", ID_RUNNED, RunStepKind::PhaseStart));
    let detail = env2.detail(ID_RUNNED);
    assert_eq!(
        detail.runs[0].reason, None,
        "reason=None → null（口径不变）"
    );
    assert_eq!(detail.runs[0].nodes.len(), 1);
    assert!(detail.runs[0].edges.is_empty(), "零边 run → edges 空数组");
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert_eq!(
        value["runs"][0]["reason"],
        serde_json::Value::Null,
        "reason null 留位不省键"
    );
}

/// 同 started_at 并列两 run → run_id 稳定并列序（排序全确定性，AC-9 叠加层
/// 稳定序的前置）；list 读面 Err（runs / nodes / edges 三面各自）→ 对应面降
/// 级空数组、详情其余面不受阻断。
#[test]
fn 详情run史出线_并列稳定序与读err降级() {
    let env = Env::new("run-history-tie");
    env.seed(ID_RUNNED, "runned", T0, None);
    // 同 started_at 并列 + 乱序插入（聚合排序兜底 run_id 稳定序）
    env.store
        .push_run(run_row(ID_RUNNED, "run-b", t(10), RunStatus::Completed));
    env.store
        .push_run(run_row(ID_RUNNED, "run-a", t(10), RunStatus::Completed));
    env.file("runned", "proposal.md", "# 提案");

    let detail = env.detail(ID_RUNNED);
    let ids: Vec<&str> = detail.runs.iter().map(|run| run.run_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["run-a", "run-b"],
        "并列 started_at → run_id 稳定并列序"
    );

    // store.list_runs Err → runs 降级空数组、流水线/产物面不受阻断
    let env_err = Env::new("run-history-err");
    env_err.seed(ID_RUNNED, "runned", T0, None);
    env_err
        .store
        .push_run(run_row(ID_RUNNED, "run-1", t(10), RunStatus::Completed));
    env_err.store.arm_runs_fault();
    env_err.file("runned", "proposal.md", "# 提案");
    let detail = env_err.detail(ID_RUNNED);
    assert!(detail.runs.is_empty(), "list_runs Err → runs 降级空数组");
    assert_eq!(detail.pipeline.len(), 9, "详情其余面不受阻断");
    assert!(
        !detail.artifacts.is_empty(),
        "产物清单不受 runs 读面失败影响"
    );

    // list_run_nodes Err → 该 run nodes 空数组（edges 降级面独立）
    let env_nodes_err = Env::new("run-history-nodes-err");
    env_nodes_err.seed(ID_RUNNED, "runned", T0, None);
    env_nodes_err
        .store
        .push_run(run_row(ID_RUNNED, "run-1", t(10), RunStatus::Completed));
    env_nodes_err
        .store
        .push_run_node(run_node_row("run-1", ID_RUNNED, RunStepKind::Executor));
    env_nodes_err.store.arm_nodes_fault();
    let detail = env_nodes_err.detail(ID_RUNNED);
    assert_eq!(detail.runs.len(), 1, "runs 行仍在");
    assert!(
        detail.runs[0].nodes.is_empty(),
        "list_run_nodes Err → nodes 降级空数组"
    );

    // list_run_edges Err → 该 run edges 空数组
    let env_edges_err = Env::new("run-history-edges-err");
    env_edges_err.seed(ID_RUNNED, "runned", T0, None);
    env_edges_err
        .store
        .push_run(run_row(ID_RUNNED, "run-1", t(10), RunStatus::Completed));
    env_edges_err.store.push_run_edge(run_edge_row(
        "run-1",
        ID_RUNNED,
        "implement:1:executor:0",
        "implement:1:phase_log:0",
        RunEdgeKind::Forward,
    ));
    env_edges_err.store.arm_edges_fault();
    let detail = env_edges_err.detail(ID_RUNNED);
    assert_eq!(detail.runs.len(), 1, "runs 行仍在");
    assert!(
        detail.runs[0].edges.is_empty(),
        "list_run_edges Err → edges 降级空数组"
    );
}

// ---------------------------------------------------------------------------
// 归档列与归档节点投影（AC-1 / AC-4 / AC-6）
// ---------------------------------------------------------------------------

/// 九列 pipeline（正向 / 改写）：requirement 记录 → `phase_table("requirement")`
/// 恰 9 列且末列为 `archive`（第 9 列）；`detail.rs` 不再持有 `PIPELINE_PHASES`
/// 常量（列序唯一来源 = 相位表）；无 attempt 的 archive 站为空序列（miss 非错误）。
#[test]
fn pipeline九列含archive且空站序列() {
    let env = Env::new("pipeline-nine-columns");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.file("runned", "proposal.md", "# 提案");

    let detail = env.detail(ID_RUNNED);
    let phases = pipeline_phases(&detail);
    assert_eq!(phases.len(), 9, "requirement 九列布局");
    assert_eq!(
        phases,
        REQUIREMENT_PHASES.to_vec(),
        "站序与 requirement 相位表逐位一致"
    );
    assert!(
        !phases.iter().any(|phase| *phase == "code-analyze"),
        "requirement 表无 code-analyze（零占位列）"
    );
    assert_eq!(phases[8], "archive", "archive 恒为末列（第 9 列）");
    assert_eq!(
        phases.iter().filter(|phase| **phase == "archive").count(),
        1,
        "archive 列恰一次（零重复）"
    );

    let archive_station = detail
        .pipeline
        .iter()
        .find(|station| station.phase == "archive")
        .expect("archive 站在场");
    assert!(
        archive_station.attempts.is_empty(),
        "archive 站零 attempt → 空序列（miss 非错误，空列不报错）"
    );
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert!(
        value["pipeline"][8]["attempts"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "空站 attempts 恒为空数组（不省键）"
    );
    assert_eq!(value["pipeline"][8]["phase"], "archive");
    assert_eq!(
        value["pipeline"].as_array().map(Vec::len),
        Some(9),
        "线形 pipeline 恰 9 列"
    );
}

/// test-only 六列 pipeline（正向 / 新增）：`workflow_type="test-only"` 记录 →
/// `phase_table("test-only")` 恰 6 列（`proposal / code-analyze / test-design /
/// test-gen / test-execution / archive`），code-analyze 第 2 列（`colIndex=1`）、
/// archive 第 6 列（`colIndex=5`）。
#[test]
fn pipeline_test_only六列含code_analyze与archive末列() {
    let env = Env::new("pipeline-test-only");
    env.seed_typed(ID_RUNNED, "runned", "test-only");
    env.file("runned", "proposal.md", "# 测试向提案");

    let detail = env.detail(ID_RUNNED);
    let phases = pipeline_phases(&detail);
    assert_eq!(phases.len(), 6, "test-only 六列布局");
    assert_eq!(
        phases,
        TEST_ONLY_PHASES.to_vec(),
        "站序与 test-only 相位表逐位一致"
    );
    assert_eq!(phases[1], "code-analyze", "code-analyze 为第 2 列");
    assert_eq!(phases[5], "archive", "archive 为第 6 列（末列）");
    // requirement 独有相位不在 test-only 列序内（零占位）
    for absent in ["dev-design", "implement", "code-review", "acceptance"] {
        assert!(
            !phases.iter().any(|phase| *phase == absent),
            "test-only 列序不含 {absent}"
        );
    }
    // 全列 attempts 空序列（零 attempt 列恒空数组，不报错）
    assert!(
        detail.pipeline.iter().all(|station| station.attempts.is_empty()),
        "未落账六列 attempts 全空"
    );

    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert_eq!(
        value["pipeline"].as_array().map(Vec::len),
        Some(6),
        "线形 pipeline 恰 6 列"
    );
    assert_eq!(value["pipeline"][1]["phase"], "code-analyze");
    assert_eq!(value["pipeline"][5]["phase"], "archive");
}

/// 两表 archive 列位置边界（边界 / 新增）：requirement（9）与 test-only（6）两
/// 表 `archive` 恒为 `pipeline` 末列（`len - 1`，archive 第 9 / 第 6 列）。
#[test]
fn pipeline两表archive恒为末列() {
    for (tag, workflow_type, expected_len) in [
        ("archive-pos-req", "requirement", 9usize),
        ("archive-pos-test", "test-only", 6usize),
    ] {
        let env = Env::new(tag);
        env.seed_typed(ID_RUNNED, "runned", workflow_type);

        let detail = env.detail(ID_RUNNED);
        let phases = pipeline_phases(&detail);
        assert_eq!(phases.len(), expected_len, "{workflow_type} 列数");
        assert_eq!(phases[expected_len - 1], "archive", "{workflow_type} 末列恒 archive");
        assert_eq!(
            phases.iter().position(|phase| *phase == "archive"),
            Some(expected_len - 1),
            "{workflow_type} archive 位置 = 末下标"
        );
    }
}

/// 未知工作流类型（异常 / 新增）：`workflow_type="bug-fix"` 记录 →
/// `change_detail` 返回 `None`（不 fallback requirement、不臆造流水线）。
#[test]
fn detail未知workflow_type恒none零fallback() {
    let env = Env::new("unknown-workflow-type");
    env.seed_typed(ID_RUNNED, "runned", WORKFLOW_TYPE_UNKNOWN);
    // 磁盘目录与产物齐备：仍不得以磁盘树 / 目录名冒充 requirement 表
    env.file("runned", "proposal.md", "# 提案");

    assert!(
        env.detail_opt(ID_RUNNED).is_none(),
        "未知 workflow_type → None（MUST NOT 静默当 requirement 跑）"
    );

    // 空白 / 空串 / 超长 / 特殊字符类型同语义（相位表不命中，零 null 兜底）
    let long_unknown = "x".repeat(1200);
    let cases = [
        "",
        " ",
        "\t",
        long_unknown.as_str(),
        "requirement\u{0}",
        "require\nment",
        "test-only🚀",
    ];
    for (idx, workflow_type) in cases.iter().enumerate() {
        let env2 = Env::new(&format!("blank-workflow-type-{idx}"));
        env2.seed_typed(ID_RUNNED, "runned", workflow_type);
        assert!(
            env2.detail_opt(ID_RUNNED).is_none(),
            "空白 / 超长 / 特殊字符 workflow_type {workflow_type:?} → None"
        );
    }
}

/// 产物清单随相位表（正向 / 改写）：清单 = `phase_table(record.workflow_type)`
/// 的 artifact_manifest——requirement 下 proposal / dev-design / test-execution
/// 产物各自挂相位；test-only 下 code-analyze 零清单（不虚设占位项）。
#[test]
fn detail产物清单随workflow_type相位表() {
    // requirement：proposal.md / design.md / tasks.md / test-reports 产物齐备
    let env = Env::new("manifest-requirement");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.file("runned", "proposal.md", "# 提案");
    env.file("runned", "design.md", "# 设计");
    env.file("runned", "tasks.md", "- [x] 一步\n");
    env.file("runned", "test-reports/report.md", "# 报告");

    let detail = env.detail(ID_RUNNED);
    let paths: Vec<&str> = detail
        .artifacts
        .iter()
        .map(|artifact| artifact.source.as_str())
        .collect();
    for expected in ["proposal.md", "design.md", "tasks.md"] {
        assert!(
            paths.contains(&expected),
            "requirement 清单发现 {expected}，实际: {paths:?}"
        );
    }
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "tasks-progress"),
        "tasks.md 走 tasks-progress 特化视图"
    );

    // test-only：proposal / test-execution 产物照常、code-analyze 零清单归属
    let env2 = Env::new("manifest-test-only");
    env2.seed_typed(ID_RUNNED, "runned", "test-only");
    env2.file("runned", "proposal.md", "# 测试向提案");
    env2.file("runned", "test-reports/report.md", "# 报告");

    let detail2 = env2.detail(ID_RUNNED);
    let paths2: Vec<&str> = detail2
        .artifacts
        .iter()
        .map(|artifact| artifact.source.as_str())
        .collect();
    assert!(paths2.contains(&"proposal.md"), "test-only proposal 产物照常挂列");
    assert!(
        detail2
            .artifacts
            .iter()
            .all(|artifact| artifact.phase != "code-analyze"),
        "code-analyze 零清单 → 无产物归属该列，实际: {paths2:?}"
    );
    assert!(
        detail2
            .artifacts
            .iter()
            .all(|artifact| TEST_ONLY_PHASES.contains(&artifact.phase.as_str())),
        "产物归属相位均在 test-only 表内"
    );
}

/// archive 空 checklist 落账投影（正向）：archive 以 `report=""` + 空 checklist
/// 落 `verdict=pass` 后 → pipeline archive 站含一条 AttemptRecord（report 空串、
/// checklist 空表、三会话槽位 null）——空 checklist 落账被识别为 pass
///（AC-4 / D6）。
#[test]
fn archive空checklist落账投影为pass尝试记录() {
    let env = Env::new("pipeline-archive-attempt");
    env.seed(ID_RUNNED, "runned", T0, None);
    let archive_entry = entry(
        ID_RUNNED,
        "archive",
        1,
        Verdict::Pass,
        "",
        Vec::new(),
        None,
        None,
        Some(T0),
        t(1),
    );
    env.push_entry(archive_entry);

    let detail = env.detail(ID_RUNNED);
    let station = detail
        .pipeline
        .iter()
        .find(|station| station.phase == "archive")
        .expect("archive 站在场");
    assert_eq!(station.attempts.len(), 1, "归档落账后 archive 站一条尝试");
    let attempt = &station.attempts[0];
    assert_eq!(
        attempt.verdict,
        Verdict::Pass,
        "空 checklist 落账 verdict=pass"
    );
    assert_eq!(attempt.attempt, Some(1));
    assert!(attempt.report.is_empty(), "归档落账 report 空串");
    assert!(attempt.checklist.is_empty(), "归档落账 checklist 空表");
    assert!(!attempt.skipped && !attempt.stale);
    assert!(
        attempt.executor_session_id.is_none()
            && attempt.evaluator_session_id.is_none()
            && attempt.decision_session_id.is_none(),
        "归档三会话槽位恒 null"
    );
    assert!(attempt.backtrack_to.is_none() && attempt.backtrack_reason.is_none());

    let value = serde_json::to_value(&detail).expect("序列化应成功");
    let wire = &value["pipeline"][8]["attempts"][0];
    assert_eq!(wire["verdict"], "pass");
    assert_eq!(wire["report"], "");
    assert!(
        wire["checklist"].as_array().is_some_and(Vec::is_empty),
        "checklist 空表出线"
    );
    assert_eq!(wire["executorSessionId"], serde_json::Value::Null);
    assert_eq!(wire["evaluatorSessionId"], serde_json::Value::Null);
    assert_eq!(wire["decisionSessionId"], serde_json::Value::Null);
}

/// 归档节点会话与跳过因投影（正向）：归档十步节点全落库时，`session_id` 仅
/// SpecSync / ResolveConflicts 在案、其余 None；`detail` 携跳过因（干净 / 已落盘 /
/// 无 delta / 用户选择 / 双写续半边）（AC-6 / 数据模型）。
#[test]
fn 归档节点会话面与跳过因投影() {
    let env = Env::new("run-archive-nodes");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.store.push_run(RunStateRecord {
        status: RunStatus::Completed,
        finished_at: Some(t(19)),
        ..run_row(ID_RUNNED, "run-archive", t(10), RunStatus::Completed)
    });

    // 归档十步节点（phase = archive；仅两个 agent 会话步携 session_id）
    let archive_nodes: &[(RunStepKind, Option<&str>, Option<&str>)] = &[
        (RunStepKind::PhaseStart, None, None),
        (RunStepKind::Preflight, None, None),
        (
            RunStepKind::Commit,
            None,
            Some("worktree 干净，无待提交变更（跳过）"),
        ),
        (RunStepKind::Rebase, None, None),
        (
            RunStepKind::ResolveConflicts,
            Some("ses-resolve-conflicts"),
            Some("无冲突，无需解算（跳过）"),
        ),
        (RunStepKind::Merge, None, None),
        (
            RunStepKind::SpecSync,
            Some("ses-spec-sync"),
            Some("用户选择跳过 spec 同步（跳过）"),
        ),
        (RunStepKind::Seal, None, Some("双写收口（落盘续半边）")),
        (
            RunStepKind::Finalize,
            None,
            Some("归档产物已落盘，无待提交路径（跳过）"),
        ),
        (RunStepKind::PhaseLog, None, Some("空 checklist 落账 pass")),
    ];
    for (step, session, detail_text) in archive_nodes {
        let mut node = run_node_row("run-archive", ID_RUNNED, *step);
        node.phase = "archive".to_owned();
        node.session_id = session.map(str::to_owned);
        node.detail = detail_text.map(str::to_owned);
        env.store.push_run_node(node);
    }

    let detail = env.detail(ID_RUNNED);
    let nodes = &detail.runs[0].nodes;
    assert_eq!(nodes.len(), 10, "归档十步节点全量在案");
    let steps: Vec<RunStepKind> = nodes.iter().map(|node| node.step).collect();
    assert_eq!(
        steps,
        vec![
            RunStepKind::PhaseStart,
            RunStepKind::Preflight,
            RunStepKind::Commit,
            RunStepKind::Rebase,
            RunStepKind::ResolveConflicts,
            RunStepKind::Merge,
            RunStepKind::SpecSync,
            RunStepKind::Seal,
            RunStepKind::Finalize,
            RunStepKind::PhaseLog,
        ],
        "归档十步序逐位在案"
    );
    // 会话面：仅两个 agent 会话步携 session_id
    let with_session: Vec<RunStepKind> = nodes
        .iter()
        .filter(|node| node.session_id.is_some())
        .map(|node| node.step)
        .collect();
    assert_eq!(
        with_session,
        vec![RunStepKind::ResolveConflicts, RunStepKind::SpecSync],
        "仅 SpecSync / ResolveConflicts 携 session_id"
    );
    // 跳过因逐节点透出（记录面）
    let commit_node = nodes
        .iter()
        .find(|node| node.step == RunStepKind::Commit)
        .expect("Commit 节点在场");
    assert!(commit_node.session_id.is_none(), "git 步零会话");
    assert!(
        commit_node
            .detail
            .as_deref()
            .is_some_and(|text| text.contains("干净")),
        "Commit 短路跳过因在案"
    );
    let finalize_node = nodes
        .iter()
        .find(|node| node.step == RunStepKind::Finalize)
        .expect("Finalize 节点在场");
    assert!(
        finalize_node
            .detail
            .as_deref()
            .is_some_and(|text| text.contains("已落盘")),
        "Finalize 已落盘跳过因在案"
    );
}

/// 归档边投影（正向）：归档串行推进边全为 `Forward`（零 feedback / retry /
/// backtrack），端点指向节点身份串 `phase:attempt:step:instance_no`（AC-6）。
#[test]
fn 归档串行边全forward且端点身份串() {
    let env = Env::new("run-archive-edges");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.store.push_run(RunStateRecord {
        status: RunStatus::Completed,
        finished_at: Some(t(19)),
        ..run_row(ID_RUNNED, "run-archive-edges", t(10), RunStatus::Completed)
    });
    let keys = [
        "archive:1:phase_start:0",
        "archive:1:preflight:0",
        "archive:1:commit:0",
        "archive:1:rebase:0",
        "archive:1:resolve_conflicts:0",
        "archive:1:merge:0",
        "archive:1:spec_sync:0",
        "archive:1:seal:0",
        "archive:1:finalize:0",
        "archive:1:phase_log:0",
    ];
    for pair in keys.windows(2) {
        env.store.push_run_edge(run_edge_row(
            "run-archive-edges",
            ID_RUNNED,
            pair[0],
            pair[1],
            RunEdgeKind::Forward,
        ));
    }

    let detail = env.detail(ID_RUNNED);
    let edges = &detail.runs[0].edges;
    assert_eq!(edges.len(), 9, "串行推进边 = 节点数 - 1");
    assert!(
        edges
            .iter()
            .all(|edge| edge.edge_kind == RunEdgeKind::Forward),
        "归档串行推进边全 forward（零反馈 / 重试 / 回溯）"
    );
    assert_eq!(edges[0].from_node_key, "archive:1:phase_start:0");
    assert_eq!(edges[8].to_node_key, "archive:1:phase_log:0");

    let value = serde_json::to_value(&detail).expect("序列化应成功");
    let wire_kinds: Vec<&str> = value["runs"][0]["edges"]
        .as_array()
        .expect("edges 数组")
        .iter()
        .map(|edge| edge["edgeKind"].as_str().expect("edgeKind 为串"))
        .collect();
    assert!(
        wire_kinds.iter().all(|kind| *kind == "forward"),
        "线面 edgeKind 全为 forward: {wire_kinds:?}"
    );
}
