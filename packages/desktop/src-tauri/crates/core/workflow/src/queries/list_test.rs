//! `queries::list_changes` 的单元测试（test-design「queries/list.rs ->
//! list_test.rs」节，重写主体）：db 单源全量投影（active 组 + 归档月组，条目
//! id 恒在案、name 恒裸名）、磁盘-only 目录零发现（零呈现 / 零报错 / 目录树
//! 逐字节零变化）、归档月分组以 `archived_at` 唯一权威（缺失归「未知时间」
//! 组置尾，磁盘目录日期前缀零参与）、空 db 空列表零组、状态面透出与
//! worktree 记录条目照常归组。原磁盘扫描 / 文档形态 / 并集去重 / 磁盘回退月
//! 分组断言随扫描段删除整体退役（读侧反转实证）。
//!
//! Mock策略（test-design 本节 Mock 表）：种子恒为固定 id 字面量的真实
//! [`ChangeStateRecord`] 值；db 半边以进程内假件实现 [`ChangeStateStore`]
//!（id 键入全量记录序列 + 纯读观察面）——「真实 tempfile Store」行在 lib-test
//! 目标结构上不可达：workflow 自环 dev-dep（store 普通 dep → workflow）在
//! lib-test 与普通 lib 双工件下类型不统一，`&Store` 无法满足 lib-test 视角的
//! `dyn ChangeStateStore`（rustc E0277「multiple different versions of crate
//! workflow」），真实 db 组合行收 tests/corpus_golden_test.rs 集成面（该文件
//! 「db 真件组合面」节明定）。磁盘目录树真实 tempdir（零发现反例要求目录真实
//! 在场且字节零变化，逐字节快照对拍）。时间戳全部确定性 i64 常量（time::Date
//! 构造锚定日界），零 wall-clock 比较。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{list_changes, ArchiveGroup, ChangeList, ChangeSource, ChangeSummary};
use crate::state::{
    ActivePhaseState, BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus,
    PhaseLogCommand, PhaseStateRecord, RunFinishCommand, RunStartCommand, RunStateRecord,
    RunStepStateRecord, StepCommand, StepStateRecord, StoreFault,
};

/// 固定 id 字面量（uuid v7 形态不透明串——身份寻址断言恒以 id 为键，name 仅
/// 作展示属性与磁盘目录供给值）。id 与 name 字面量各异：任何以 name 冒充
/// id 的寻址错位都会被断言击穿。
const ID_ACTIVE_ALPHA: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a01";
const ID_ACTIVE_BETA: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a02";
const ID_ARCH_MAY: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a03";
const ID_ARCH_JAN: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a04";
const ID_ARCH_FEB: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a05";
const ID_ARCH_MAY_EARLY: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a06";
const ID_ARCH_UNKNOWN: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a07";
const ID_WORKTREE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a08";

/// 确定性时间戳基（UTC unix millis，2024-09-22 UTC）。
const T0: i64 = 1_727_000_000_000;

/// UTC 日界锚定的毫秒（`2026-MM-DD` 零点），分组月份断言的确定性来源。
fn utc_millis(year: i32, month: time::Month, day: u8) -> i64 {
    time::Date::from_calendar_date(year, month, day)
        .expect("日界应合法")
        .midnight()
        .assume_utc()
        .unix_timestamp()
        * 1000
}

/// 临时 workspace 根 RAII + 进程内假件 store。
struct Env {
    root: PathBuf,
    store: ListStore,
}

impl Env {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("workflow-list-test-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        Self {
            root: dir.clone(),
            store: ListStore::new(),
        }
    }

    /// 预置建档记录（active、active_phase 可选、时间戳全固定；id / name 字面
    /// 量各异）。
    fn seed_active(
        &self,
        id: &str,
        name: &str,
        created_at: i64,
        active_phase: Option<ActivePhaseState>,
    ) {
        self.store.push(ChangeStateRecord {
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

    /// 预置归档记录（`archived_at` 可缺——未知时间组的唯一输入面）。
    fn seed_archived(&self, id: &str, name: &str, created_at: i64, archived_at: Option<i64>) {
        self.store.push(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at,
            status: ChangeStatus::Archived,
            archived_at,
            active_phase: None,
            worktree: None,
            base_commit: None,
        });
    }

    /// 在 workspace 内创建目录并写入文件（磁盘半边——目录名恒为记录 name
    /// 或归档前缀形态）。
    fn change(&self, rel_dir: &str, files: &[(&str, &str)]) {
        let dir = self.root.join(rel_dir);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        for (name, content) in files {
            fs::write(dir.join(name), content).expect("写文件失败");
        }
    }

    /// 目录树逐字节快照（相对 root 的路径 → 文件字节；目录以 `<rel>/` 空值
    /// 在场标记，含空目录）。
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        collect_bytes(&self.root, &self.root, &mut out);
        out
    }

    fn list(&self) -> ChangeList {
        list_changes(&self.store)
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// 递归收集目录树字节快照（目录在场标记 + 文件全字节）。
fn collect_bytes(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            out.insert(format!("{rel}/"), Vec::new());
            collect_bytes(root, &path, out);
        } else {
            out.insert(rel, fs::read(&path).expect("读文件字节失败"));
        }
    }
}

// ---------------------------------------------------------------------------
// 假件 store：id 键入的全量记录序列（`list_change_records` 读面 + 可编程故障
// 注入），其余 unimplemented（越权触达即 panic——list_changes 零 extra 触点
// 的执法面）。
// ---------------------------------------------------------------------------

struct ListStore {
    records: Mutex<Vec<ChangeStateRecord>>,
    fault: Mutex<Option<StoreFault>>,
}

impl ListStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            fault: Mutex::new(None),
        }
    }

    fn push(&self, record: ChangeStateRecord) {
        self.records.lock().expect("记录锁不可中毒").push(record);
    }

    /// db 半边故障注入（读面 Err → 空列表降级断言的输入面）。
    fn arm_fault(&self) {
        *self.fault.lock().expect("故障锁不可中毒") =
            Some(StoreFault::Db("list_change_records 注入失败".to_owned()));
    }
}

impl ChangeStateStore for ListStore {
    fn get_change(&self, _id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        Ok(self.records.lock().expect("记录锁不可中毒").clone())
    }

    fn list_phase_records(&self, _change_id: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
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

    fn list_runs(&self, _change_id: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_run_steps(&self, _run_id: &str) -> Result<Vec<RunStepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_start(&self, _command: &RunStartCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn run_finish(&self, _command: &RunFinishCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }
}

// ---------------------------------------------------------------------------
// 断言辅助
// ---------------------------------------------------------------------------

/// active 组内按 id 取条目（身份寻址断言恒以 id 为键）。
fn active_by_id<'a>(list: &'a ChangeList, id: &str) -> &'a ChangeSummary {
    list.active
        .iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("active 中应含 id={id}"))
}

/// 归档组内按 id 取条目。
fn archive_by_id<'a>(groups: &'a [ArchiveGroup], id: &str) -> &'a ChangeSummary {
    groups
        .iter()
        .flat_map(|group| group.changes.iter())
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("archive 中应含 id={id}"))
}

/// 组键序列（月份面断言直读）。
fn group_months(groups: &[ArchiveGroup]) -> Vec<Option<&str>> {
    groups.iter().map(|group| group.month.as_deref()).collect()
}

/// 组内条目 name 序列。
fn group_names(group: &ArchiveGroup) -> Vec<&str> {
    group
        .changes
        .iter()
        .map(|entry| entry.name.as_str())
        .collect()
}

/// 归档条目总数（全量投影对账用）。
fn archive_total(groups: &[ArchiveGroup]) -> usize {
    groups.iter().map(|group| group.changes.len()).sum()
}

// ---------------------------------------------------------------------------
// 正向：db 单源全量（active / archived 异 id → 列表 = db 记录全量投影）
// ---------------------------------------------------------------------------

/// db 多条（active / archived 异 id 异名）→ 列表 = db 记录全量投影：active 组
/// + 归档月组逐条出线、条目 id 恒在案（首字段）、name 恒裸名；磁盘目录零参与
/// 并集（同名不再去重、磁盘-only 零入列——原「并集与去重」断言反转）。
#[test]
fn db单源全量_异id全量投影且id恒在案与name恒裸名() {
    let env = Env::new("db-single-source");
    env.seed_active(
        ID_ACTIVE_ALPHA,
        "alpha-active",
        T0,
        Some(ActivePhaseState {
            phase: "implement".to_owned(),
            attempt: 2,
            start_at: T0 + 1_000,
        }),
    );
    env.seed_active(ID_ACTIVE_BETA, "beta-active", T0, None);
    env.seed_archived(
        ID_ARCH_MAY,
        "may-archived",
        T0,
        Some(utc_millis(2026, time::Month::May, 20)),
    );
    env.seed_archived(
        ID_ARCH_JAN,
        "jan-archived",
        T0,
        Some(utc_millis(2026, time::Month::January, 5)),
    );
    // 磁盘目录在场（同名裸名目录 + 磁盘-only 目录 + 归档前缀目录）：零参与
    env.change(
        "openspec/changes/alpha-active",
        &[("proposal.md", "# 磁盘副本")],
    );
    env.change(
        "openspec/changes/disk-only",
        &[("proposal.md", "# 存量 CLI")],
    );
    env.change(
        "openspec/changes/archive/2026-05-20-may-archived",
        &[("proposal.md", "# 归档副本")],
    );

    let list = env.list();

    // active 组 = db active 全量（名升序——磁盘-only 零入列、同名零重复）
    let names: Vec<&str> = list
        .active
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["alpha-active", "beta-active"],
        "active = db active 全量（磁盘目录零参与并集）"
    );
    let alpha = active_by_id(&list, ID_ACTIVE_ALPHA);
    assert_eq!(alpha.name, "alpha-active", "条目 name = 记录裸名");
    assert_eq!(alpha.source, ChangeSource::Active);
    assert_eq!(alpha.status, Some(ChangeStatus::Active));
    assert_eq!(
        alpha
            .active_phase
            .as_ref()
            .map(|active| active.phase.as_str()),
        Some("implement"),
        "状态面 active_phase 透出"
    );
    assert_eq!(
        alpha.created.as_deref(),
        Some("2024-09-22"),
        "状态面 created 取 created_at（T0 = 2024-09-22 UTC）"
    );
    // 归档月组 = db archived 全量（组间新月份在前——磁盘前缀不参与）
    assert_eq!(
        group_months(&list.archive_groups),
        vec![Some("2026-05"), Some("2026-01")],
        "归档组 = db archived 按月分组"
    );
    let may = archive_by_id(&list.archive_groups, ID_ARCH_MAY);
    assert_eq!(may.name, "may-archived", "name 恒裸名（磁盘前缀名零入线）");
    assert_eq!(may.source, ChangeSource::Archive);
    assert_eq!(may.status, Some(ChangeStatus::Archived));
    assert_eq!(
        archive_total(&list.archive_groups),
        2,
        "归档条目全量不重不漏"
    );
    assert_eq!(list.active.len() + archive_total(&list.archive_groups), 4);

    // id 首字段出线（DTO 字段序 = 线面键序——行键 / 路由的身份锚）
    let wire = serde_json::to_string(alpha).expect("条目序列化应成功");
    assert!(
        wire.starts_with(&format!("{{\"id\":\"{ID_ACTIVE_ALPHA}\"")),
        "id 为首字段，实际: {wire}"
    );
}

// ---------------------------------------------------------------------------
// 异常：磁盘-only 目录零发现（零呈现 / 零报错 / 目录字节零变化）
// ---------------------------------------------------------------------------

/// 预置磁盘-only active 目录 + archive 树前缀 / 无前缀目录（含惰性字节样本，
/// 磁盘-only = 存量 CLI 建、无 db 记录）→ 清单零呈现该等条目、零报错、目录树
/// 逐字节零变化（AC-3 主锚——原「并集与去重」「文档形态照常入列」断言反转）。
#[test]
fn 磁盘目录零发现_零呈现零报错且字节零变化() {
    let env = Env::new("disk-blind");
    env.change(
        "openspec/changes/legacy-cli-change",
        &[
            (
                "workflow.json",
                "{ \"workflow_type\": \"requirement\", \"eval\": [] }",
            ),
            ("proposal.md", "# 存量提案"),
        ],
    );
    env.change(
        "openspec/changes/archive/2026-09-15-disk-archived",
        &[("proposal.md", "# 归档提案")],
    );
    env.change(
        "openspec/changes/archive/no-date-prefix",
        &[("workflow.json", "{ CORRUPT_MARKER_桌面不解析此字节 }")],
    );
    // archive 树下散落文件（非目录形态）与 active 树下混入文件
    fs::write(
        env.root.join("openspec/changes/archive/loose.txt"),
        "archive 树下散落文件",
    )
    .expect("写散落文件失败");
    fs::write(
        env.root.join("openspec/changes/stray.md"),
        "changes 树下混入文件",
    )
    .expect("写混入文件失败");

    let before = env.snapshot();
    let list = env.list();
    let after = env.snapshot();

    assert!(list.active.is_empty(), "磁盘-only active 目录零呈现");
    assert!(
        list.archive_groups.is_empty(),
        "磁盘-only 归档目录（前缀 / 无前缀 / 散落文件）零呈现"
    );
    assert_eq!(
        before.get("openspec/changes/legacy-cli-change/workflow.json"),
        Some(
            &"{ \"workflow_type\": \"requirement\", \"eval\": [] }"
                .as_bytes()
                .to_vec()
        ),
        "快照含真实在场字节（零变化断言非真空），实际键: {:?}",
        before.keys().collect::<Vec<_>>()
    );
    assert_eq!(before, after, "查询零 fs 触点：目录树逐字节零变化");
    // 零报错：签名无错误面（正常返回即证据）；惰性样本字节零进投影
    let wire = serde_json::to_string(&list).expect("列表序列化应成功");
    assert!(
        !wire.contains("CORRUPT_MARKER"),
        "workflow.json 惰性字节零进投影"
    );
}

// ---------------------------------------------------------------------------
// 边界：归档月分组以 archived_at 唯一权威（目录前缀零参与）
// ---------------------------------------------------------------------------

/// db archived 记录按 `archived_at` 分组：同月多条并入同组、组间新月份在前、
/// `archived_at` 缺失 → 「未知时间」组置尾；磁盘目录日期前缀不参与分组
///（前缀月 2026-12 的记录按 archived_at 归 2026-02；前缀目录在场而
/// archived_at 缺失 → 仍归未知时间组——原「磁盘回退目录前缀」断言退役）。
#[test]
fn 月分组_archived_at唯一权威_目录前缀零参与且缺失置尾() {
    let env = Env::new("month-authority");
    env.seed_archived(
        ID_ARCH_FEB,
        "feb-change",
        T0,
        Some(utc_millis(2026, time::Month::February, 10)),
    );
    env.seed_archived(
        ID_ARCH_MAY,
        "may-late",
        T0,
        Some(utc_millis(2026, time::Month::May, 20)),
    );
    env.seed_archived(
        ID_ARCH_MAY_EARLY,
        "may-early",
        T0,
        Some(utc_millis(2026, time::Month::May, 3)),
    );
    env.seed_archived(ID_ARCH_UNKNOWN, "no-archived-at", T0, None);
    // 磁盘前缀目录与 archived_at 月冲突（2026-12-31 前缀不得牵引归组）
    env.change(
        "openspec/changes/archive/2026-12-31-feb-change",
        &[("proposal.md", "# 归档")],
    );
    // 未知时间组条目亦有前缀目录在场（不得因此入组）
    env.change(
        "openspec/changes/archive/2026-11-01-no-archived-at",
        &[("proposal.md", "# 归档")],
    );

    let list = env.list();

    assert_eq!(
        group_months(&list.archive_groups),
        vec![Some("2026-05"), Some("2026-02"), None],
        "组间新月份在前；未知时间组（archived_at 缺失）固定置尾"
    );
    // 同月并入同组（组内名降序）
    assert_eq!(
        group_names(&list.archive_groups[0]),
        vec!["may-late", "may-early"],
        "同 archived_at 月多条并入同组"
    );
    // 目录前缀零参与：feb-change 归 archived_at 月（2026-02），非磁盘前缀月
    let feb_group = list
        .archive_groups
        .iter()
        .find(|group| group.changes.iter().any(|entry| entry.id == ID_ARCH_FEB))
        .expect("feb 条目应有分组");
    assert_eq!(
        feb_group.month.as_deref(),
        Some("2026-02"),
        "分组月份 = archived_at（磁盘前缀 2026-12 零参与）"
    );
    assert_eq!(
        archive_by_id(&list.archive_groups, ID_ARCH_FEB).name,
        "feb-change",
        "name 恒裸名（磁盘前缀名零入线）"
    );
    // 未知时间组置尾且不丢弃
    let unknown = list.archive_groups.last().expect("末组应为未知时间组");
    assert_eq!(unknown.month, None);
    assert_eq!(
        group_names(unknown),
        vec!["no-archived-at"],
        "archived_at 缺失条目入未知时间组（前缀目录在场不牵引）"
    );
}

// ---------------------------------------------------------------------------
// 边界：空输入（空 db → 空列表零组）
// ---------------------------------------------------------------------------

/// 空 db（磁盘目录在场——同断言）→ 空列表零组；db 读面故障 → 降级空列表
///（签名无错误面：读命令面空结果语义），两态均零报错。
#[test]
fn 空db_空列表零组_读面故障降级空列表() {
    let env = Env::new("empty-db");
    env.change(
        "openspec/changes/legacy-cli-change",
        &[("proposal.md", "# 存量提案")],
    );
    env.change("openspec/changes/archive/2026-09-15-disk-archived", &[]);

    let list = env.list();
    assert!(list.active.is_empty(), "空 db → active 空组");
    assert!(list.archive_groups.is_empty(), "空 db → 零归档组");

    // db 半边故障 → 降级空列表（零 panic 零错误面）
    env.store.arm_fault();
    let degraded = env.list();
    assert!(
        degraded.active.is_empty() && degraded.archive_groups.is_empty(),
        "读面故障降级空列表（空结果语义）"
    );
}

// ---------------------------------------------------------------------------
// 正向：状态面透出（worktree 记录条目照常归组——记录态演绎零改动）
// ---------------------------------------------------------------------------

/// worktree 条目归组（AC-9 scenario 字面）：db active（worktree=Some）+ 主仓
/// 两树均未命中 → 进行中组、状态面完整（status / created / active_phase）、
/// 目录名 = 建档名——不报错、不丢弃、不误归未知时间组（记录态演绎零改动）。
#[test]
fn worktree条目入进行中组_状态面完整且不误归未知时间组() {
    let env = Env::new("wt-entry");
    env.store.push(ChangeStateRecord {
        id: ID_WORKTREE.to_owned(),
        name: "wt-change".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: T0,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: Some(ActivePhaseState {
            phase: "implement".to_owned(),
            attempt: 2,
            start_at: T0 + 5_000,
        }),
        worktree: Some(r"C:\app-data\worktrees\seg\wt-change".to_owned()),
        base_commit: Some("0000000000000000000000000000000000000001".to_owned()),
    });
    // 主仓两树零目录（merge 前形态）

    let list = env.list();

    let entry = active_by_id(&list, ID_WORKTREE);
    assert_eq!(entry.name, "wt-change", "目录名 = 建档裸名（未命中回退）");
    assert_eq!(entry.source, ChangeSource::Active, "进行中组");
    assert_eq!(entry.status, Some(ChangeStatus::Active), "状态面 status 在");
    assert_eq!(
        entry.created.as_deref(),
        Some("2024-09-22"),
        "状态面 created 在"
    );
    assert!(
        entry.active_phase.is_some(),
        "状态面 active_phase 在（运行态透出）"
    );
    // 不误归未知时间组（archive_groups 空——不因目录缺席虚构归档形态）
    assert!(
        list.archive_groups.is_empty(),
        "worktree 条目不误归任何归档组（含未知时间组）"
    );
}
