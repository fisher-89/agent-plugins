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
use super::detail::PIPELINE_PHASES;
use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    ActivePhaseState, BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus,
    PhaseLogCommand, PhaseStateRecord, RunFinishCommand, RunStartCommand, RunStateRecord,
    RunStatus, RunStepKind, RunStepStateRecord, RunStepStatus, StepCommand, StepStateRecord,
    StoreFault,
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
    run_steps: Mutex<Vec<RunStepStateRecord>>,
    /// 可编程 Err 注入：命中即 list_runs / list_run_steps 返回 Err（降级空
    /// 数组断言的输入面）。
    runs_fault: Mutex<bool>,
    steps_fault: Mutex<bool>,
}

impl DetailStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            entries: Mutex::new(Vec::new()),
            runs: Mutex::new(Vec::new()),
            run_steps: Mutex::new(Vec::new()),
            runs_fault: Mutex::new(false),
            steps_fault: Mutex::new(false),
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

    /// run 步史种子（run_id 圈定键）。
    fn push_run_step(&self, record: RunStepStateRecord) {
        self.run_steps
            .lock()
            .expect("run step 锁不可中毒")
            .push(record);
    }

    /// 读面 Err 注入（list_runs / list_run_steps 各自独立）。
    fn arm_runs_fault(&self) {
        *self.runs_fault.lock().expect("fault 锁不可中毒") = true;
    }

    fn arm_steps_fault(&self) {
        *self.steps_fault.lock().expect("fault 锁不可中毒") = true;
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

    fn list_run_steps(&self, run_id: &str) -> Result<Vec<RunStepStateRecord>, StoreFault> {
        if *self.steps_fault.lock().expect("fault 锁不可中毒") {
            return Err(StoreFault::Db("list_run_steps 注入失败".to_owned()));
        }
        Ok(self
            .run_steps
            .lock()
            .expect("run step 锁不可中毒")
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

/// 固定 9 站相位名序列（流水线站序断言直读）。
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
fn 归档change全状态面_九站流水线与runs全量出线() {
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
    env.store.push_run_step(RunStepStateRecord {
        seq: 1,
        run_id: "run-arch-1".to_owned(),
        phase: "implement".to_owned(),
        attempt: 1,
        step: RunStepKind::Executor,
        status: RunStepStatus::Passed,
        session_id: Some("ses-exec-1".to_owned()),
        detail: None,
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
        PIPELINE_PHASES.to_vec(),
        "9 站 pipeline 全量出线（MUST NOT 落文档形态空面）"
    );
    assert_eq!(detail.pipeline[0].attempts.len(), 1, "proposal 站条目在案");
    assert_eq!(detail.pipeline[3].attempts[0].report, "实现完成");
    assert_eq!(detail.runs.len(), 1, "runs 全量出线（非空面）");
    assert_eq!(detail.runs[0].run_id, "run-arch-1");
    assert_eq!(detail.runs[0].steps.len(), 1, "run 步史随行投影");
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
fn db重组流水线_九站与attempt升序与checklist内联() {
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
    assert_eq!(pipeline_phases(&detail), PIPELINE_PHASES.to_vec());

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
        PIPELINE_PHASES.to_vec(),
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
        PIPELINE_PHASES.len(),
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
// 详情 run 史出线：runs / steps 投影
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

/// run 步史行 fixture。
fn run_step_row(
    run_id: &str,
    seq: u64,
    step: RunStepKind,
    status: RunStepStatus,
    session_id: Option<&str>,
    detail: Option<&str>,
) -> RunStepStateRecord {
    RunStepStateRecord {
        seq,
        run_id: run_id.to_owned(),
        phase: "implement".to_owned(),
        attempt: 1,
        step,
        status,
        session_id: session_id.map(str::to_owned),
        detail: detail.map(str::to_owned),
        timestamp: 1_727_000_060_000,
    }
}

/// 两 run 全史出线：runs 两条全史、runId / status / reason / startedAt /
/// finishedAt 逐项投影、时间戳 ISO 串口径与既有字段一致（同 iso_from_millis
/// 单点）；steps 按 seq 升序、session_id / detail 两态（Some 透传、None →
/// null）——归属键 = change id（记录供给语义）。
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
    // run-1 步史：乱序种子（聚合面 seq 升序）+ 两态字段
    env.store.push_run_step(run_step_row(
        "run-1",
        3,
        RunStepKind::Evaluator,
        RunStepStatus::Failed,
        Some("ses-eval-1"),
        Some("首轮评估 fail"),
    ));
    env.store.push_run_step(run_step_row(
        "run-1",
        1,
        RunStepKind::Executor,
        RunStepStatus::Passed,
        Some("ses-exec-1"),
        None,
    ));
    env.store.push_run_step(run_step_row(
        "run-1",
        2,
        RunStepKind::StaticCheck,
        RunStepStatus::Passed,
        None,
        None,
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

    // run-1 steps：seq 升序 + 两态透传
    let steps = &run1.steps;
    let seqs: Vec<u64> = steps.iter().map(|row| row.seq).collect();
    assert_eq!(seqs, vec![1, 2, 3], "steps 按 seq 升序出线（= emit 序）");
    assert_eq!(steps[0].step, RunStepKind::Executor);
    assert_eq!(steps[0].session_id.as_deref(), Some("ses-exec-1"));
    assert_eq!(steps[0].detail, None, "None → null 透传");
    assert_eq!(steps[1].step, RunStepKind::StaticCheck);
    assert_eq!(steps[1].session_id, None);
    assert_eq!(steps[2].step, RunStepKind::Evaluator);
    assert_eq!(steps[2].detail.as_deref(), Some("首轮评估 fail"));

    // run-2（在飞起始行）：status=running、finishedAt=null、steps 恒空
    let run2 = &detail.runs[1];
    assert_eq!(run2.status, RunStatus::Running);
    assert_eq!(run2.finished_at, None, "running 行 finished_at → null");
    assert!(
        run2.steps.is_empty(),
        "在飞 run 起始行 steps 恒空（run 清单面运行中可见）"
    );

    // wire 面：camelCase 键 + null 不省键（纯 derive 零字段属性口径）
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    let wire_runs = value["runs"].as_array().expect("runs 为数组");
    assert_eq!(wire_runs[1]["runId"], "run-2");
    assert_eq!(
        wire_runs[1]["finishedAt"],
        serde_json::Value::Null,
        "null 不省键"
    );
    assert!(wire_runs[1]["steps"].as_array().is_some_and(Vec::is_empty));
}

/// 在档记录无 run 行 → runs 空数组且 wire 键恒在场（记录在档、9 站流水线照常
/// ——非文档形态）；终态 run reason=None → null 出线（口径不变面）。
#[test]
fn 详情run史出线_无run记录空数组与reason_none() {
    let env = Env::new("run-history-empty");
    env.seed(ID_RUNNED, "runned", T0, None);
    env.file("runned", "proposal.md", "# 提案");

    let detail = env.detail(ID_RUNNED);
    assert!(detail.runs.is_empty(), "在档记录无 run 行 → 空数组");
    assert_eq!(
        detail.pipeline.len(),
        PIPELINE_PHASES.len(),
        "记录在档：9 站流水线照常（非文档形态空面）"
    );
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert!(
        value["runs"].as_array().is_some_and(Vec::is_empty),
        "runs 键恒在场（空数组不省键）"
    );

    // 终态 run reason=None（无记因收口）→ null 出线
    let env2 = Env::new("run-history-no-reason");
    env2.seed(ID_RUNNED, "runned", T0, None);
    env2.store.push_run(RunStateRecord {
        reason: None,
        finished_at: Some(t(19)),
        status: RunStatus::Completed,
        ..run_row(ID_RUNNED, "run-1", t(10), RunStatus::Completed)
    });
    let detail = env2.detail(ID_RUNNED);
    assert_eq!(
        detail.runs[0].reason, None,
        "reason=None → null（口径不变）"
    );
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert_eq!(
        value["runs"][0]["reason"],
        serde_json::Value::Null,
        "reason null 留位不省键"
    );
}

/// 同 started_at 并列两 run → run_id 稳定并列序（排序全确定性，AC-9 叠加层
/// 稳定序的前置）；list 面读 Err → runs 降级空数组、详情其余面不受阻断。
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

    // list_run_steps Err → 该 run steps 空数组（同样降级）
    let env_steps_err = Env::new("run-history-steps-err");
    env_steps_err.seed(ID_RUNNED, "runned", T0, None);
    env_steps_err
        .store
        .push_run(run_row(ID_RUNNED, "run-1", t(10), RunStatus::Completed));
    env_steps_err.store.arm_steps_fault();
    let detail = env_steps_err.detail(ID_RUNNED);
    assert_eq!(detail.runs.len(), 1, "runs 行仍在");
    assert!(
        detail.runs[0].steps.is_empty(),
        "list_run_steps Err → steps 降级空数组"
    );
}
