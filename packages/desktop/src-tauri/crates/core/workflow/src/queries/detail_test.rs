//! `queries::change_detail` 的单元测试（test-design「queries/detail.rs ->
//! detail_test.rs」节）：db 重组 9 站流水线（attempt 升序、checklist 内联、
//! 槽位三列直读、stale / backtrack 字段透出）、时间出线单点（i64 millis →
//! RFC3339 ISO 串 + null；epoch 0 → `1970-01-01T00:00:00Z` 口径）、文档形态
//!（磁盘目录在场 db 缺记录 → 空流水线 + 产物清单，workflow.json 字节零进
//! 投影零读取）、槽位全缺（三会话槽位 + start_at 全 null 面）、未找到 `None`。
//!
//! Mock策略（test-design 本节 Mock 表）：磁盘树真实 tempdir（产物文件 + 惰性
//! workflow.json 字节样本）；db 半边以进程内假件实现 [`ChangeStateStore`]
//!（可编程记录 / 条目序列——`start_at=None` 形态仅此可达：真实 store 落账恒
//! 回填 active start_at，见变更报告；真实 tempfile Store 的重组 / ISO 组合行
//! 收 tests/corpus_golden_test.rs 集成面）。时间戳全部确定性 i64 常量。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::change_detail;
use super::detail::PIPELINE_PHASES;
use crate::model::{ChecklistItem, Verdict};
use crate::state::{
    ActivePhaseState, BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus,
    PhaseLogCommand, PhaseStateRecord, StepCommand, StepStateRecord, StoreFault,
};
use foundation::layout::{resolve, Layout};

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

    fn seed_record(&self, name: &str, created_at: i64, active_phase: Option<ActivePhaseState>) {
        self.store.set_record(ChangeStateRecord {
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

    fn seed_archived_record(&self, name: &str, created_at: i64, archived_at: i64) {
        self.store.set_record(ChangeStateRecord {
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

    fn push_entry(&self, entry: PhaseStateRecord) {
        self.store.push_entry(entry);
    }

    fn file(&self, name: &str, rel: &str, content: &str) {
        let dir = self.layout.changes_root.join(name);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        if let Some(parent) = dir.join(rel).parent() {
            fs::create_dir_all(parent).expect("创建子目录失败");
        }
        fs::write(dir.join(rel), content).expect("写文件失败");
    }

    fn detail(&self, name: &str) -> super::ChangeDetail {
        change_detail(&self.layout, &self.store, name)
            .unwrap_or_else(|| panic!("应能定位 change {name}"))
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// ---------------------------------------------------------------------------
// 假件 store：get_change / list_phase_records 读半边（可编程序列），其余
// unimplemented（纯读面——越权触写即 panic）。
// ---------------------------------------------------------------------------

struct DetailStore {
    record: Mutex<Option<ChangeStateRecord>>,
    entries: Mutex<Vec<PhaseStateRecord>>,
}

impl DetailStore {
    fn new() -> Self {
        Self {
            record: Mutex::new(None),
            entries: Mutex::new(Vec::new()),
        }
    }

    fn set_record(&self, record: ChangeStateRecord) {
        *self.record.lock().expect("记录锁不可中毒") = Some(record);
    }

    fn push_entry(&self, entry: PhaseStateRecord) {
        self.entries.lock().expect("条目锁不可中毒").push(entry);
    }
}

impl ChangeStateStore for DetailStore {
    fn get_change(&self, _name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self.record.lock().expect("记录锁不可中毒").clone())
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_phase_records(&self, _change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        Ok(self.entries.lock().expect("条目锁不可中毒").clone())
    }

    fn list_steps(
        &self,
        _change: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn create_change_record(&self, _record: ChangeStateRecord) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn delete_change_record(&self, _name: &str) -> Result<bool, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn start_phase(
        &self,
        _change: &str,
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
        _change: &str,
        _phase: &str,
        _session_id: &str,
    ) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn set_archived(&self, _name: &str, _archived_at: i64) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn append_step(&self, _command: &StepCommand) -> Result<(), StoreFault> {
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

/// 内存构造一条相位评估条目（聚合输入面全字段可控）。
#[allow(clippy::too_many_arguments)]
fn entry(
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
        change: "multi".to_owned(),
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

// ---------------------------------------------------------------------------
// 正向：db 重组流水线（9 站 / attempt 升序 / checklist 内联 / 槽位与
// stale / backtrack 字段透出）
// ---------------------------------------------------------------------------

/// 记录序列（多相位多 attempt + checklist + 回跳 stale 标记）→ 9 站流水线
/// 重组：站序固定、attempt 升序、checklist 随行、槽位三列直读、stale /
/// backtrack_to / backtrack_reason 字段透出（AttemptRecord 形状不变——AC-4）。
#[test]
fn db重组流水线_九站与attempt升序与checklist内联() {
    let env = Env::new("pipeline");
    env.seed_record(
        "multi",
        T0,
        Some(ActivePhaseState {
            phase: "test-gen".to_owned(),
            attempt: 1,
            start_at: t(5),
        }),
    );
    env.push_entry(entry(
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
    env.file("multi", "proposal.md", "# 提案");

    let detail = env.detail("multi");

    // 9 站全量输出，顺序固定
    let phases: Vec<&str> = detail
        .pipeline
        .iter()
        .map(|station| station.phase.as_str())
        .collect();
    assert_eq!(phases, PIPELINE_PHASES.to_vec());

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
    env.seed_record("partial", T0, None);
    env.push_entry(entry(
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
    env.file("partial", "tasks.md", "- [x] 完成\n");

    let detail = env.detail("partial");

    let phases: Vec<&str> = detail
        .pipeline
        .iter()
        .map(|station| station.phase.as_str())
        .collect();
    assert_eq!(phases, PIPELINE_PHASES.to_vec(), "顺序固定，不依赖条目排列");
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
/// 半边，golden 守卫绿的前提断言——AC-4）。转换收 queries 单点。
#[test]
fn 时间出线iso串_epoch零口径与null留位() {
    let env = Env::new("epoch-zero");
    env.seed_record("zero-ts", 0, None);
    env.push_entry(entry(
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

    let detail = env.detail("zero-ts");
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
// 边界：槽位全缺 + start_at None → null 留位 / 文档形态 / 未找到
// ---------------------------------------------------------------------------

/// PhaseRecord 三会话槽位 None 且 start_at None → AttemptRecord 四值 null 不
/// 报错且 wire 四键恒在场（纯 derive 零字段属性口径；既有语义持衡）。
/// 注记：`start_at=None` 仅假件可达——真实 store 落账恒回填 active start_at
///（log_change_phase `command.start_at.or(Some(active_start_at))`）。
#[test]
fn 槽位与start_at全缺_wire四键null恒在场() {
    let env = Env::new("slots-null");
    env.seed_record("bare", T0, None);
    env.push_entry(entry(
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

    let detail = env.detail("bare");
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
    env.seed_record("flags", T0, None);
    let mut skipped = entry(
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

    let detail = env.detail("flags");
    // test-gen = 第 5 站（下标 4），test-execution = 第 6 站（下标 5）
    assert!(detail.pipeline[4].attempts[0].skipped);
    assert!(!detail.pipeline[4].attempts[0].stale);
    assert!(detail.pipeline[5].attempts[0].stale);
    assert!(!detail.pipeline[5].attempts[0].skipped);
}

/// 磁盘目录在场（含 workflow.json 惰性字节样本）db 缺记录 → 空流水线 + 产物
/// 清单，字节零进投影；全程无 workflow.json 读取（损坏字节样本即证据）。
#[test]
fn 文档形态_空流水线与产物清单且字节零进投影() {
    let env = Env::new("document-form");
    env.file("legacy-docs", "proposal.md", "# v0 提案");
    env.file("legacy-docs", "tasks.md", "- [x] 完成\n- [ ] 待办\n");
    env.file(
        "legacy-docs",
        "workflow.json",
        "{ CORRUPT_MARKER_桌面不解析此字节 }",
    );

    let detail = env.detail("legacy-docs");

    assert!(detail.pipeline.is_empty(), "文档形态空流水线");
    assert_eq!(detail.status, None, "db 缺记录 → 无状态面");
    assert!(detail.active_phase.is_none());
    assert_eq!(detail.created, None, "active 树文档形态无 created 回退");
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "markdown-doc" && artifact.source == "proposal.md"),
        "文档产物照常进入产物清单，实际: {:?}",
        detail.artifacts
    );
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "tasks-progress"),
        "tasks.md 勾选计数照常可达"
    );

    // 字节零进投影：序列化全量不含惰性样本字节
    let serialized = serde_json::to_string(&detail).expect("序列化应成功");
    assert!(
        !serialized.contains("CORRUPT_MARKER"),
        "workflow.json 字节零进投影"
    );
}

/// archive 树文档形态：created 回退查询名的日期前缀——实现语义为「回退查询
/// 名」（`archive_prefix_date(name)`）：以 db 名（裸名）查询时回退为 None；
/// 以带前缀目录名查询时出前缀日期（定位经 archive 精确名命中）。
#[test]
fn archive树文档形态_created回退查询名前缀() {
    let env = Env::new("archive-document");
    let dir = env.layout.archive_root.join("2026-07-06-archived-docs");
    fs::create_dir_all(&dir).expect("创建 archive 目录失败");
    fs::write(dir.join("proposal.md"), "# 归档提案").expect("写产物失败");

    // db 名（裸名）查询：经日期前缀后缀匹配定位到 archive 目录，但 created
    // 回退以裸名判前缀 → None
    let by_bare_name = env.detail("archived-docs");
    assert!(by_bare_name.pipeline.is_empty());
    assert_eq!(by_bare_name.source, super::ChangeSource::Archive);
    assert_eq!(by_bare_name.status, None);
    assert_eq!(by_bare_name.created, None);

    // 带前缀目录名查询：archive 精确名命中、created 出目录前缀日期
    let by_prefixed_name = env.detail("2026-07-06-archived-docs");
    assert!(by_prefixed_name.pipeline.is_empty());
    assert_eq!(by_prefixed_name.source, super::ChangeSource::Archive);
    assert_eq!(
        by_prefixed_name.created.as_deref(),
        Some("2026-07-06"),
        "created 回退查询名的日期前缀"
    );
}

/// db archived 建档：created 取 db created_at（优先于目录前缀）。
#[test]
fn db归档建档_created优先取created_at() {
    let env = Env::new("archive-db-record");
    env.seed_archived_record("archived-record", T0, t(9));
    let dir = env.layout.archive_root.join("2026-01-01-archived-record");
    fs::create_dir_all(&dir).expect("创建 archive 目录失败");

    let detail = env.detail("archived-record");

    assert_eq!(detail.status, Some(ChangeStatus::Archived));
    assert_eq!(
        detail.created.as_deref(),
        Some("2024-09-22"),
        "db created_at 优先（T0 = 2024-09-22 UTC）"
    );
}

/// 两树均无目录且 db 无记录 → `None`（不 Err 不虚构）；空白 / 穿越名 →
/// None；db 有档但定位全 miss（D12 建档记录恒可达）→ Some（dir 缺席、产物
/// 清单空、状态面在——worktree 未 merge / 被删的呈现形态）。
#[test]
fn 未找到_none与建档恒可达() {
    let env = Env::new("not-found");
    env.file("real", "proposal.md", "# 提案");

    assert!(change_detail(&env.layout, &env.store, "不存在的change").is_none());
    assert!(change_detail(&env.layout, &env.store, "").is_none());
    assert!(change_detail(&env.layout, &env.store, "a/b").is_none());

    // db 有档但定位全 miss → 恒可达（状态面在、产物清单空）
    env.seed_record("ghost-with-record", T0, None);
    let detail = change_detail(&env.layout, &env.store, "ghost-with-record")
        .expect("建档记录恒可达详情（D12）");
    assert_eq!(detail.status, Some(ChangeStatus::Active), "状态面在");
    assert!(detail.artifacts.is_empty(), "定位 miss 产物清单空");
    assert_eq!(detail.source, super::ChangeSource::Active);
}

// ---------------------------------------------------------------------------
// worktree 维度（design D12）：worktree 路径出线与产物自 worktree 发现 /
// 手动删恒可达（source 映射）/ legacy null 出线（serde 键恒在场）
// ---------------------------------------------------------------------------

impl Env {
    /// 带 worktree 执行锚的建档种子（merge 前主仓两树未命中的形态）。
    fn seed_worktree_record(&self, name: &str, worktree: &Path) {
        self.store.set_record(ChangeStateRecord {
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

    /// worktree 内 change 目录写文件（worktree 树夹具）。
    fn worktree_file(&self, worktree: &Path, name: &str, rel: &str, content: &str) {
        let dir = resolve(worktree).changes_root.join(name);
        fs::create_dir_all(&dir).expect("创建 worktree change 目录失败");
        if let Some(parent) = dir.join(rel).parent() {
            fs::create_dir_all(parent).expect("创建 worktree 子目录失败");
        }
        fs::write(dir.join(rel), content).expect("写 worktree 文件失败");
    }
}

/// worktree 路径出线：记录 `worktree=Some(绝对路径)` + worktree 目录树在场
///（主仓两树未命中）→ detail Some、`worktree` 与库内记录值**逐字**一致、
/// artifacts 自 worktree 目录发现（产物清单命中 worktree 内文件）。
#[test]
fn worktree路径出线_与库内记录逐字一致且产物自worktree发现() {
    let env = Env::new("wt-detail");
    let worktree =
        std::env::temp_dir().join(format!("workflow-detail-wt-{}-out", std::process::id()));
    let _ = fs::remove_dir_all(&worktree);
    env.seed_worktree_record("wt-change", &worktree);
    env.worktree_file(&worktree, "wt-change", "proposal.md", "# worktree 内提案");
    env.worktree_file(&worktree, "wt-change", "tasks.md", "- [x] 一步\n");

    let detail = env.detail("wt-change");

    // worktree 与库内记录值逐字一致（直读透出零改写）
    let record = env
        .store
        .record
        .lock()
        .expect("记录锁不可中毒")
        .clone()
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
    env.seed_worktree_record("gone-wt-change", &worktree);

    let detail = change_detail(&env.layout, &env.store, "gone-wt-change")
        .expect("建档记录恒可达详情（D12——worktree 被删不虚构 None）");

    assert_eq!(detail.status, Some(ChangeStatus::Active), "状态面在");
    assert_eq!(
        detail.pipeline.len(),
        super::detail::PIPELINE_PHASES.len(),
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
    env.seed_archived_record("archived-gone", T0, T0 + 1_000);
    // 归档记录带 worktree 的存量形态（归档前建域的 change 手动删目录）
    env.store
        .record
        .lock()
        .expect("记录锁不可中毒")
        .as_mut()
        .map(|record| {
            record.worktree = Some(worktree.to_string_lossy().into_owned());
            record
        });

    let detail = change_detail(&env.layout, &env.store, "archived-gone").expect("建档记录恒可达");

    assert_eq!(
        detail.source,
        super::ChangeSource::Archive,
        "archived 记录定位 miss → source 映射 Archive"
    );
    assert_eq!(detail.status, Some(ChangeStatus::Archived), "状态面随记录");
}

/// legacy null 出线：记录 `worktree=None` → detail `worktree` 出线 null、产物
/// 解析走主仓两树既有语义；serde 线面 `worktree` 键**恒在场**（null 不省键
/// ——冻结契约；文档形态 null 留位同面）。
#[test]
fn legacy记录worktree出线null且serde键恒在场() {
    // legacy 建档：worktree=None + 主仓目录在场（既有语义解析）
    let env = Env::new("legacy-null");
    env.seed_record("legacy-change", T0, None);
    env.file("legacy-change", "proposal.md", "# 主仓提案");

    let detail = env.detail("legacy-change");
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

    // 文档形态（无记录）同面：worktree 键 null 留位
    env.file("doc-only", "proposal.md", "# 文档形态");
    let detail = env.detail("doc-only");
    let value = serde_json::to_value(&detail).expect("序列化应成功");
    assert_eq!(
        value.get("worktree"),
        Some(&serde_json::Value::Null),
        "文档形态 worktree 键 null 留位（恒在场）"
    );
}
