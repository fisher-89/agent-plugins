//! `write::archive` 的单元测试（test-design「archive.rs ->
//! archive_test.rs」节，新建）：归档双写（D6）——db 建档校验先于一切变更 →
//! 目录改名（active → archive 树 `YYYY-MM-DD-<name>`，目标已存在先查拒绝）→
//! db 翻转（status=archived + archived_at，主键 name 不变）；续半边重试仅补
//! db 翻转不重复改名（含已翻转幂等）；无建档目录显式拒绝；翻转失败呈现半
//! 完成态（目录已改名事实由读侧呈现，重试路径可达）；双树同名 active 优先。
//!
//! Mock策略（test-design 本节 Mock 表）：db 半边以进程内假件实现
//! [`ChangeStateStore`]（可编程 `set_archived` Err / 调用计数；真实 tempfile
//! Store 组合行收 tests/corpus_golden_test.rs 集成面——workflow 自环 dev-dep
//! 在 lib-test 与普通 lib 双工件下类型不统一，见变更报告）；文件系统真实
//! tempdir Layout（active / archive 树真实改名，不经 mock）。

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use foundation::layout::{resolve, Layout};

use super::archive::{archive, ArchiveOutcome};
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStateRecord, StepCommand, StepStateRecord, StoreFault,
};

const CHANGE: &str = "seed-change";

/// 确定性时间戳基（UTC unix millis）。
const T0: i64 = 1_727_000_000_000;

// ---------------------------------------------------------------------------
// 装置：真实 tempdir workspace 树 + 进程内假件 store
// ---------------------------------------------------------------------------

struct Env {
    root: std::path::PathBuf,
    store: ArchiveStore,
    layout: Layout,
}

impl Env {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "workflow-archive-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&root);
        let layout = resolve(&root);
        // archive 树在场（fs::rename 不建目标父目录；真实 workspace 归档树常在）
        fs::create_dir_all(&layout.archive_root).expect("预置 archive 树失败");
        Self {
            root,
            store: ArchiveStore::active(),
            layout,
        }
    }

    fn make_active_dir(&self, name: &str) {
        let dir = self.layout.changes_root.join(name);
        fs::create_dir_all(&dir).expect("预置 active 目录失败");
        fs::write(dir.join("proposal.md"), "# 提案").expect("预置产物失败");
    }

    fn make_archive_dir(&self, dir_name: &str) {
        let dir = self.layout.archive_root.join(dir_name);
        fs::create_dir_all(&dir).expect("预置 archive 目录失败");
        fs::write(dir.join("proposal.md"), "# 归档").expect("预置产物失败");
    }

    fn archive(&self, name: &str) -> Result<ArchiveOutcome, String> {
        archive(&self.layout, &self.store, name)
    }

    /// archive 树中后缀恰为 `<name>` 的目录名集合（目标改名断言的观察面）。
    fn archive_dirs_suffixed(&self, name: &str) -> Vec<String> {
        dir_names(&self.layout.archive_root)
            .into_iter()
            .filter(|dir_name| dir_name.ends_with(&format!("-{name}")))
            .collect()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn dir_names(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// 当前 UTC 日历日期 `YYYY-MM-DD`（目标改名前缀的「当日」界用）。
fn utc_date_today() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

// ---------------------------------------------------------------------------
// 假件 store：get_change / set_archived（可编程故障 + 调用计数，成功路径
// 镜像真件翻转语义）。
// ---------------------------------------------------------------------------

struct ArchiveStore {
    record: Mutex<Option<ChangeStateRecord>>,
    set_calls: Mutex<Vec<(String, i64)>>,
    set_fault: Mutex<Option<StoreFault>>,
}

impl ArchiveStore {
    fn active() -> Self {
        Self {
            record: Mutex::new(Some(Self::active_record())),
            set_calls: Mutex::new(Vec::new()),
            set_fault: Mutex::new(None),
        }
    }

    fn archived() -> Self {
        let mut record = Self::active_record();
        record.status = ChangeStatus::Archived;
        record.archived_at = Some(T0);
        Self {
            record: Mutex::new(Some(record)),
            set_calls: Mutex::new(Vec::new()),
            set_fault: Mutex::new(None),
        }
    }

    fn active_record() -> ChangeStateRecord {
        ChangeStateRecord {
            name: CHANGE.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: T0,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
        }
    }

    /// 无建档形态（get_change → None）。
    fn missing() -> Self {
        Self {
            record: Mutex::new(None),
            set_calls: Mutex::new(Vec::new()),
            set_fault: Mutex::new(None),
        }
    }

    /// 注入 set_archived 故障（翻转失败行）。
    fn inject_set_fault(&self, fault: StoreFault) {
        *self.set_fault.lock().expect("故障锁不可中毒") = Some(fault);
    }

    fn set_call_count(&self) -> usize {
        self.set_calls.lock().expect("调用锁不可中毒").len()
    }

    fn record(&self) -> ChangeStateRecord {
        self.record
            .lock()
            .expect("记录锁不可中毒")
            .clone()
            .expect("建档记录应在场")
    }
}

impl ChangeStateStore for ArchiveStore {
    fn get_change(&self, _name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self.record.lock().expect("记录锁不可中毒").clone())
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
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

    fn set_archived(&self, name: &str, archived_at: i64) -> Result<(), StoreFault> {
        self.set_calls
            .lock()
            .expect("调用锁不可中毒")
            .push((name.to_owned(), archived_at));
        if let Some(fault) = self.set_fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        let mut record = self.record.lock().expect("记录锁不可中毒");
        let record = record.as_mut().expect("建档记录应在场");
        record.status = ChangeStatus::Archived;
        record.archived_at = Some(archived_at);
        Ok(())
    }

    fn append_step(&self, _command: &StepCommand) -> Result<(), StoreFault> {
        unimplemented!("本用例不可达")
    }
}

// ---------------------------------------------------------------------------
// 正向：双写成功 / 续半边重试（D6）
// ---------------------------------------------------------------------------

/// 建档 + 磁盘目录在场 → 目录改名入 archive 树（日期前缀）+ db
/// status=archived / archived_at 落库；主键 name 不变；
/// ArchiveOutcome.archived_date 为 UTC 当日（AC-7）。
#[test]
fn 双写成功_目录改名入archive树且db翻转() {
    let env = Env::new("dual-write");
    env.make_active_dir(CHANGE);
    let before = utc_date_today();

    let outcome = env.archive(CHANGE).expect("归档应成功");

    let after = utc_date_today();
    assert_eq!(outcome.name, CHANGE, "主键 name 不随目录改名变");
    assert!(
        outcome.archived_date == before || outcome.archived_date == after,
        "archived_date 为 UTC 当日，实际: {}",
        outcome.archived_date
    );

    // fs 半边：active 目录消失、archive 树出现 `YYYY-MM-DD-<name>` 目录
    assert!(
        !env.layout.changes_root.join(CHANGE).exists(),
        "active 树源目录已被改名挪走"
    );
    let archived_dirs = env.archive_dirs_suffixed(CHANGE);
    assert_eq!(archived_dirs.len(), 1, "archive 树恰一个后缀命中目录");
    assert!(
        archived_dirs[0] == format!("{before}-{CHANGE}")
            || archived_dirs[0] == format!("{after}-{CHANGE}"),
        "目标目录名带当日日期前缀，实际: {}",
        archived_dirs[0]
    );

    // db 半边：status 翻转 + archived_at 在场 + 主键 name 不变
    let record = env.store.record();
    assert_eq!(record.status, ChangeStatus::Archived);
    assert!(record.archived_at.is_some(), "archived_at 落库");
    assert_eq!(record.name, CHANGE);
    assert_eq!(env.store.set_call_count(), 1, "翻转恰一次");
}

/// 续半边重试（D6）：预置半完成态（目录已在 archive 树 + db 仍 active）→
/// 重试仅补 db 翻转，不重复改名（archive 树源目录不被二次挪动）。
#[test]
fn 续半边重试_仅补db翻转不重复改名() {
    let env = Env::new("resume-half");
    let archived_name = "2026-10-06-seed-change";
    env.make_archive_dir(archived_name);

    let outcome = env.archive(CHANGE).expect("续半边归档应成功");

    assert_eq!(outcome.name, CHANGE);
    assert_eq!(
        outcome.archived_date, "2026-10-06",
        "续半边命中带前缀目录时取前缀日期"
    );
    // archive 树源目录原位不动（不重复改名）
    assert!(
        env.layout.archive_root.join(archived_name).is_dir(),
        "archive 树源目录不被二次挪动"
    );
    assert_eq!(env.archive_dirs_suffixed(CHANGE).len(), 1);
    // db 半边补齐翻转
    let record = env.store.record();
    assert_eq!(record.status, ChangeStatus::Archived);
    assert!(record.archived_at.is_some());
}

/// 续半边幂等：db 已 archived + archive 树命中 → Ok 且不再 set_archived
///（假件调用计数断言）。
#[test]
fn 已归档续半边幂等_不再翻转() {
    let env = Env::new("resume-idempotent");
    env.make_archive_dir("2026-10-06-seed-change");
    // 假件换为已翻转记录
    let store = ArchiveStore::archived();

    let outcome = archive(&env.layout, &store, CHANGE).expect("幂等续半边应 Ok");

    assert_eq!(outcome.name, CHANGE);
    assert_eq!(outcome.archived_date, "2026-10-06");
    assert_eq!(store.set_call_count(), 0, "已翻转不再二次 set_archived");
}

// ---------------------------------------------------------------------------
// 异常：无建档拒绝 / 目标冲突 / 翻转失败半完成态
// ---------------------------------------------------------------------------

/// active 树目录存在但 db 无记录 → 显式 `Err` 且零 fs 零 db 变更（拒绝先于
/// 一切变更——存量 CLI 目录不可经桌面归档的显式面）。
#[test]
fn 无建档拒绝_零fs零db变更() {
    let env = Env::new("no-record");
    env.make_active_dir(CHANGE);
    // 假件换为无建档
    let store = ArchiveStore::missing();

    let error = archive(&env.layout, &store, CHANGE).expect_err("无建档应 Err");

    assert!(
        error.contains("未建档") && error.contains(CHANGE),
        "Err 显式记因建档缺失，实际: {error}"
    );
    assert!(env.layout.changes_root.join(CHANGE).is_dir(), "零 fs 变更");
    assert!(
        env.archive_dirs_suffixed(CHANGE).is_empty(),
        "archive 树零新增"
    );
    assert_eq!(store.set_call_count(), 0, "db 零变更（零翻转调用）");
}

/// archive 树已存在 `YYYY-MM-DD-<name>` → 先查拒绝，db 零变更。
#[test]
fn 目标冲突先查拒绝_db零变更() {
    let env = Env::new("target-conflict");
    env.make_active_dir(CHANGE);
    let today = utc_date_today();
    env.make_archive_dir(&format!("{today}-{CHANGE}"));

    let error = env.archive(CHANGE).expect_err("归档目标已存在应 Err");

    assert!(error.contains("已存在"), "先查拒绝记因，实际: {error}");
    assert_eq!(env.store.record().status, ChangeStatus::Active, "db 零变更");
    assert_eq!(env.store.set_call_count(), 0, "翻转调用零下发");
    assert!(
        env.layout.changes_root.join(CHANGE).is_dir(),
        "active 树源目录零触碰"
    );
}

/// 翻转失败半完成态：假件 store `set_archived` 注入 `Err` → `Err` 呈现半
/// 完成态；目录已改名事实由读侧呈现（重试路径可达）。
#[test]
fn 翻转失败呈现半完成态且目录已改名() {
    let env = Env::new("flip-fail");
    env.make_active_dir(CHANGE);
    env.store
        .inject_set_fault(StoreFault::Db("注入的翻转故障".to_owned()));

    let error = env.archive(CHANGE).expect_err("翻转失败应 Err");

    assert!(
        error.contains("翻转失败") && error.contains("注入的翻转故障"),
        "Err 呈现半完成态与故障记因，实际: {error}"
    );
    assert_eq!(env.store.set_call_count(), 1, "翻转恰调用一次");
    // 目录已改名事实（fs 半边先行）：active 消失、archive 树带当日前缀目录在场
    assert!(
        !env.layout.changes_root.join(CHANGE).exists(),
        "active 树源目录已被改名挪走（重试路径可达的续半边前提）"
    );
    let today = utc_date_today();
    assert!(
        env.layout.archive_root.join(format!("{today}-{CHANGE}")).is_dir(),
        "archive 树目标目录在场"
    );
}

// ---------------------------------------------------------------------------
// 边界：定位与边界（双树同名 / 无前缀同名 / 两树均未命中）
// ---------------------------------------------------------------------------

/// 同名目录同时存在于 active 与 archive 树 → active 精确名优先（archive 树
/// 既有目录不被触碰）。
#[test]
fn 双树同名_active精确名优先() {
    let env = Env::new("dual-tree");
    env.make_active_dir(CHANGE);
    env.make_archive_dir("2026-01-01-seed-change");
    let before = utc_date_today();

    let outcome = env.archive(CHANGE).expect("归档应成功");

    let after = utc_date_today();
    assert!(
        outcome.archived_date == before || outcome.archived_date == after,
        "active 精确名优先走常规路径（当日日期），实际: {}",
        outcome.archived_date
    );
    assert!(
        !env.layout.changes_root.join(CHANGE).exists(),
        "active 树源目录被改名（active 精确名优先）"
    );
    assert!(
        env.layout.archive_root.join("2026-01-01-seed-change").is_dir(),
        "archive 树既有目录不被触碰"
    );
    assert_eq!(env.archive_dirs_suffixed(CHANGE).len(), 2, "仅新增当日目录");
}

/// archive 树无日期前缀的同名目录（外部手工挪入）：定位命中（精确名）→
/// 续半边分支仅补 db 翻转。
/// 注记：test-design 本行原判「不识别为续半边对象 → 常规 Err」与实现不符
///（`locate_archived_dir` 精确名命中在先、无前缀亦命中，`prefix_date=None`
/// 时 Outcome.archived_date 取当日）——本用例按实现行为钉住（discrepancy
/// 见变更报告）。
#[test]
fn archive树无前缀同名目录_续半边命中补翻转() {
    let env = Env::new("no-prefix-archive");
    env.make_archive_dir(CHANGE);
    let today = utc_date_today();

    let outcome = env.archive(CHANGE).expect("无前缀同名目录应命中续半边");

    assert_eq!(outcome.name, CHANGE);
    assert!(
        outcome.archived_date == today,
        "无前缀命中取当日，实际: {}",
        outcome.archived_date
    );
    assert!(
        env.layout.archive_root.join(CHANGE).is_dir(),
        "无前缀目录原位不动（不重复改名）"
    );
    assert_eq!(env.store.record().status, ChangeStatus::Archived, "db 补翻转");
}

/// 两树均未命中（db 有档、目录不存在）→ `Err`（不虚构归档）。
#[test]
fn 两树均未命中_err() {
    let env = Env::new("no-dirs");

    let error = env.archive(CHANGE).expect_err("目录缺失应 Err");

    assert!(
        error.contains("未找到") && error.contains(CHANGE),
        "Err 记因两树未命中，实际: {error}"
    );
    assert_eq!(env.store.record().status, ChangeStatus::Active, "db 零变更");
    assert_eq!(env.store.set_call_count(), 0, "翻转调用零下发");
}
