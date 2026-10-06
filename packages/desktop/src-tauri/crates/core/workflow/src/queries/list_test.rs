//! `queries::list_changes` 的单元测试（test-design「queries/list.rs ->
//! list_test.rs」节）：db 记录 ∪ 磁盘目录去重并集（同名以 db 为准）、文档
//! 形态入列（status / active_phase / created 全 None）、按月分组（db 取
//! `archived_at`、磁盘回退目录前缀、无前缀入未知时间组置尾）、D7 读时以
//! 磁盘事实归组且不回写 db（纯读纪律）、既有守卫持衡（active 扫描跳过
//! archive 目录本身、name 语义 = 磁盘目录名）。
//!
//! Mock策略（test-design 本节 Mock 表）：db 半边以进程内假件实现
//! [`ChangeStateStore`]（可编程记录序列 + 纯读断言观察面；真实 tempfile
//! Store 的 D7 / 并集组合行收 tests/corpus_golden_test.rs 集成面——workflow
//! 自环 dev-dep 在 lib-test 与普通 lib 双工件下类型不统一，见变更报告）；
//! 磁盘目录树真实 tempdir。时间戳全部确定性 i64 常量（time::Date 构造锚定
//! 日界），零 wall-clock 比较。

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use super::list_changes;
use crate::state::{
    ActivePhaseState, BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus,
    PhaseLogCommand, PhaseStateRecord, StepCommand, StepStateRecord, StoreFault,
};
use foundation::layout::{resolve, Layout};

/// 临时 workspace 根 RAII + 进程内假件 store。
struct Env {
    root: PathBuf,
    store: ListStore,
    layout: Layout,
}

impl Env {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "workflow-list-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self {
            root: dir.clone(),
            store: ListStore::new(),
            layout: resolve(&dir),
        }
    }

    /// 预置建档记录（active、active_phase 可选、时间戳全固定）。
    fn seed_record(&self, name: &str, created_at: i64, active_phase: Option<ActivePhaseState>) {
        self.store.push(ChangeStateRecord {
            name: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase,
        });
    }

    /// 预置归档记录（archived_at 固定）。
    fn seed_archived(&self, name: &str, created_at: i64, archived_at: i64) {
        self.store.push(ChangeStateRecord {
            name: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at,
            status: ChangeStatus::Archived,
            archived_at: Some(archived_at),
            active_phase: None,
        });
    }

    /// 在 workspace 内创建目录并写入文件。
    fn change(&self, rel_dir: &str, files: &[(&str, &str)]) {
        let dir = self.root.join(rel_dir);
        fs::create_dir_all(&dir).expect("创建 change 目录失败");
        for (name, content) in files {
            fs::write(dir.join(name), content).expect("写文件失败");
        }
    }

    fn list(&self) -> super::ChangeList {
        list_changes(&self.layout, &self.store)
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// ---------------------------------------------------------------------------
// 假件 store：list_change_records 返回可编程记录序列（纯读观察面 = 序列快照
// 比对），其余 unimplemented（越权触达即 panic）。
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
        self.records
            .lock()
            .expect("记录锁不可中毒")
            .push(record);
    }

    fn snapshot(&self) -> Vec<ChangeStateRecord> {
        self.records.lock().expect("记录锁不可中毒").clone()
    }
}

impl ChangeStateStore for ListStore {
    fn get_change(&self, _name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        if let Some(fault) = self.fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        Ok(self.records.lock().expect("记录锁不可中毒").clone())
    }

    fn list_phase_records(&self, _change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
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

fn by_name<'a>(active: &'a [super::ChangeSummary], name: &str) -> &'a super::ChangeSummary {
    active
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("active 中应含 {name}"))
}

fn archive_entry<'a>(
    groups: &'a [super::ArchiveGroup],
    name: &str,
) -> &'a super::ChangeSummary {
    groups
        .iter()
        .flat_map(|group| group.changes.iter())
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("archive 中应含 {name}"))
}

// ---------------------------------------------------------------------------
// 正向：并集与去重（同名以 db 为准）/ 文档形态入列
// ---------------------------------------------------------------------------

/// db 建档条目 + 磁盘-only 条目同时入列，同名只出现一次且取 db 形态
///（status / active_phase 状态面在场——AC-4）。
#[test]
fn 并集与去重_同名共存以db形态为准() {
    let env = Env::new("union-dedup");
    env.seed_record(
        "db-change",
        T0,
        Some(ActivePhaseState {
            phase: "dev-design".to_owned(),
            attempt: 1,
            start_at: T0 + 1_000,
        }),
    );
    env.change("openspec/changes/db-change", &[("proposal.md", "# 提案")]);
    env.change("openspec/changes/disk-only", &[("proposal.md", "# 提案")]);
    // 同名共存：db 建档 + 磁盘目录
    env.seed_record("dual-change", T0, None);
    env.change("openspec/changes/dual-change", &[("proposal.md", "# 提案")]);

    let list = env.list();

    let names: Vec<&str> = list.active.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["db-change", "disk-only", "dual-change"],
        "并集按名排序，同名只出现一次"
    );

    // db 形态：状态面在场
    let db_entry = by_name(&list.active, "db-change");
    assert_eq!(db_entry.status, Some(ChangeStatus::Active));
    let active_phase = db_entry.active_phase.as_ref().expect("运行态应有 active_phase");
    assert_eq!(active_phase.phase, "dev-design");
    assert_eq!(active_phase.attempt, 1);
    assert!(active_phase.start_at.is_some(), "start_at 出 ISO 串");
    assert_eq!(
        db_entry.created.as_deref(),
        Some("2024-09-22"),
        "created 取 db created_at 日期（T0 = 2024-09-22 UTC）"
    );

    // 同名共存以 db 为准
    let dual = by_name(&list.active, "dual-change");
    assert_eq!(dual.status, Some(ChangeStatus::Active), "同名取 db 形态");
    assert!(dual.active_phase.is_none());

    // 磁盘-only：状态面全 None（文档形态）
    let disk = by_name(&list.active, "disk-only");
    assert_eq!(disk.status, None);
    assert!(disk.active_phase.is_none());
    assert_eq!(disk.created, None);
}

/// 磁盘目录在场 db 缺记录（存量 CLI change，workflow.json 惰性字节在场）→
/// 照常入列，status=None / active_phase=None（AC-3 / AC-4 文档形态半边）。
#[test]
fn 文档形态_存量cli目录照常入列无状态面() {
    let env = Env::new("document-form");
    env.change(
        "openspec/changes/legacy-cli-change",
        &[
            ("workflow.json", "{ \"workflow_type\": \"requirement\", \"eval\": [] }"),
            ("proposal.md", "# 存量提案"),
        ],
    );

    let list = env.list();

    assert_eq!(list.active.len(), 1, "文档形态照常入列");
    let entry = &list.active[0];
    assert_eq!(entry.name, "legacy-cli-change");
    assert_eq!(entry.source, super::ChangeSource::Active);
    assert_eq!(entry.status, None, "db 缺记录 → 无状态面");
    assert!(entry.active_phase.is_none());
    assert_eq!(entry.created, None);
    assert!(list.archive_groups.is_empty());
}

// ---------------------------------------------------------------------------
// 正向：按月分组（db 取 archived_at / 磁盘回退目录前缀）
// ---------------------------------------------------------------------------

/// db 归档条目按 archived_at 月份分组；磁盘 archive 条目按目录日期前缀分组
///（实现现状：前缀全日期为组键，与 db 侧 [..7] 月粒度不一致——discrepancy
/// 见变更报告）；组间新组在前。
#[test]
fn 按月分组_db取archived_at_磁盘回退目录前缀() {
    let env = Env::new("month-groups");
    // db 归档两条（archived_at 锚定 2026-05 / 2026-01）
    env.seed_archived("db-archived-may", T0, utc_millis(2026, time::Month::May, 20));
    env.seed_archived("db-archived-jan", T0, utc_millis(2026, time::Month::January, 5));
    // 磁盘 archive 条目（目录前缀 2026-09-15，db 缺记录）
    env.change(
        "openspec/changes/archive/2026-09-15-disk-archived",
        &[("proposal.md", "# 归档")],
    );

    let list = env.list();

    let months: Vec<Option<&str>> = list
        .archive_groups
        .iter()
        .map(|group| group.month.as_deref())
        .collect();
    assert_eq!(
        months,
        vec![Some("2026-09-15"), Some("2026-05"), Some("2026-01")],
        "组间新组在前（磁盘组键 = 前缀全日期，db 组键 = archived_at 月）"
    );

    // db 归档条目：按 archived_at 分组、无目录 → 条目名 = 建档名
    let may = &list.archive_groups[1];
    assert_eq!(may.changes.len(), 1);
    assert_eq!(may.changes[0].name, "db-archived-may");
    assert_eq!(may.changes[0].status, Some(ChangeStatus::Archived));
    assert_eq!(may.changes[0].source, super::ChangeSource::Archive);

    // 磁盘 archive 条目：按目录前缀分组、name 语义 = 磁盘目录名
    let september = &list.archive_groups[0];
    assert_eq!(september.changes[0].name, "2026-09-15-disk-archived");
    assert_eq!(september.changes[0].status, None);
    assert_eq!(
        september.changes[0].created.as_deref(),
        Some("2026-09-15"),
        "磁盘条目 created 回退目录前缀日期"
    );
}

/// 磁盘归档组内新名在前（同月多条磁盘目录倒序——既有断言持衡迁移至前缀
/// 组粒度的实现现状）。
#[test]
fn 磁盘归档组_同前缀月内新名在前() {
    let env = Env::new("within-group");
    env.change("openspec/changes/archive/2026-01-02-a", &[]);
    env.change("openspec/changes/archive/2026-01-15-b", &[]);
    env.change("openspec/changes/archive/2026-03-05-c", &[]);

    let list = env.list();

    let months: Vec<Option<&str>> = list
        .archive_groups
        .iter()
        .map(|group| group.month.as_deref())
        .collect();
    assert_eq!(
        months,
        vec![Some("2026-03-05"), Some("2026-01-15"), Some("2026-01-02")],
        "实现现状：磁盘条目按前缀全日期成组"
    );
    // 同前缀（同组）内多条时组内新名在前：补同日两条验证
    let env2 = Env::new("within-group-same-day");
    env2.change("openspec/changes/archive/2026-01-02-a", &[]);
    env2.change("openspec/changes/archive/2026-01-02-z", &[]);
    let list2 = env2.list();
    assert_eq!(list2.archive_groups.len(), 1);
    let names: Vec<&str> = list2.archive_groups[0]
        .changes
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(names, vec!["2026-01-02-z", "2026-01-02-a"], "组内按目录名倒序");
}

// ---------------------------------------------------------------------------
// 边界：D7 读时对账（纯读纪律）+ 未知时间组 + 空列表
// ---------------------------------------------------------------------------

/// D7：外部 CLI 归档（目录已改名入 archive 树）而 db 仍 active → 以磁盘事实
/// 归入 archive 月组，查询路径不回写 store（读后假件记录序列逐字段不变）；
/// 反向（db archived、目录仍在 active 树）→ 按 active 归组。
#[test]
fn d7读时对账_以磁盘事实归组且不回写store() {
    let env = Env::new("d7-reconcile");
    // 正向：db active + 磁盘目录已被改名入 archive 树
    env.seed_record(
        "d7-change",
        T0,
        Some(ActivePhaseState {
            phase: "proposal".to_owned(),
            attempt: 1,
            start_at: T0 + 1_000,
        }),
    );
    env.change(
        "openspec/changes/archive/2026-10-01-d7-change",
        &[("proposal.md", "# 归档")],
    );
    // 反向：db archived + 目录仍在 active 树
    env.seed_archived("d8-change", T0, utc_millis(2026, time::Month::February, 1));
    env.change("openspec/changes/d8-change", &[("proposal.md", "# 仍在场")]);

    let before = env.store.snapshot();
    let list = env.list();

    // 正向：磁盘事实归 archive（按目录前缀分组），状态面仍来自 db（active）
    let d7 = archive_entry(&list.archive_groups, "2026-10-01-d7-change");
    assert_eq!(d7.source, super::ChangeSource::Archive);
    assert_eq!(d7.status, Some(ChangeStatus::Active), "status 面 = db 记录");
    let d7_group = list
        .archive_groups
        .iter()
        .find(|group| {
            group
                .changes
                .iter()
                .any(|entry| entry.name == "2026-10-01-d7-change")
        })
        .expect("d7 条目应有分组");
    assert_eq!(
        d7_group.month.as_deref(),
        Some("2026-10"),
        "db 条目按目录前缀回退并截月（[..7]；磁盘-only 条目才用前缀全日期成组）"
    );

    // 纯读纪律：查询路径不回写 store（记录序列逐字段不变）
    assert_eq!(env.store.snapshot(), before, "读后 db 仍 active（零回写）");

    // 反向：db archived + 目录仍在 active 树 → 按 active 归组
    let d8 = by_name(&list.active, "d8-change");
    assert_eq!(d8.source, super::ChangeSource::Active);
    assert_eq!(d8.status, Some(ChangeStatus::Archived), "状态面随 db");
}

/// archive 目录无日期前缀 → 入未知时间组置尾不丢弃；空 db + 空磁盘 → 空
/// 列表零组。
#[test]
fn 无日期前缀入未知时间组置尾_空输入零组() {
    let env = Env::new("unknown-month");
    env.change("openspec/changes/archive/2026-04-01-dated", &[]);
    env.change("openspec/changes/archive/no-date-prefix", &[]);
    env.change("openspec/changes/archive/not-a-date-2026", &[]);

    let list = env.list();
    assert_eq!(list.archive_groups.len(), 2);
    assert_eq!(
        list.archive_groups[0].month.as_deref(),
        Some("2026-04-01"),
        "磁盘条目组键 = 目录前缀全日期（实现现状）"
    );
    let unknown = &list.archive_groups[1];
    assert_eq!(unknown.month, None, "无前缀 → 未知时间组");
    assert_eq!(
        list.archive_groups.last().map(|group| group.month.clone()),
        Some(None),
        "未知时间组固定置尾"
    );
    let mut names: Vec<&str> = unknown
        .changes
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    names.sort();
    assert_eq!(names, vec!["no-date-prefix", "not-a-date-2026"]);

    // 空 db + 空磁盘 → 空列表零组
    let empty = Env::new("empty-everything");
    let empty_list = empty.list();
    assert!(empty_list.active.is_empty());
    assert!(empty_list.archive_groups.is_empty());
}

// ---------------------------------------------------------------------------
// 既有守卫持衡
// ---------------------------------------------------------------------------

/// active 扫描不把 archive 目录本身误当名为 archive 的 active change；混入
/// 的普通文件被忽略；db 条目目录缺失时条目名回退建档名。
#[test]
fn 既有守卫_active扫描跳过archive目录与混入文件() {
    let env = Env::new("guards");
    env.change("openspec/changes/real-change", &[("proposal.md", "# 提案")]);
    // archive 树在场（含一条散落文件）
    env.change("openspec/changes/archive/2026-02-03-archived", &[]);
    fs::write(
        env.layout.archive_root.join("loose.txt"),
        "archive 树下散落文件",
    )
    .expect("写散落文件失败");
    // changes 树下混入普通文件
    fs::create_dir_all(&env.layout.changes_root).expect("创建 changes_root 失败");
    fs::write(env.layout.changes_root.join("stray.md"), "散落文件")
        .expect("写散落文件失败");
    // db active 条目无任何目录 → 以磁盘事实归 archive（未知时间组），条目名
    // 回退建档名（磁盘目录名回退链）
    env.seed_record("db-no-dir", T0, None);

    let list = env.list();

    let names: Vec<&str> = list.active.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["real-change"],
        "active 扫描跳过 archive 目录本身与散落文件"
    );
    assert!(!names.contains(&"archive"), "archive 目录不得误入 active 列表");

    // 目录缺失的 db active 条目 → 未知时间组、条目名回退建档名
    let orphan = archive_entry(&list.archive_groups, "db-no-dir");
    assert_eq!(orphan.source, super::ChangeSource::Archive);
    assert_eq!(orphan.status, Some(ChangeStatus::Active));
    assert_eq!(list.archive_groups.len(), 2, "仅当月组与未知时间组");

    // archive 树只有目录条目入组（散落文件不入组）
    let archived = archive_entry(&list.archive_groups, "2026-02-03-archived");
    assert_eq!(archived.name, "2026-02-03-archived", "name 语义 = 磁盘目录名");
    assert_eq!(list.archive_groups.len(), 2, "当月组 + 未知时间组");
}
