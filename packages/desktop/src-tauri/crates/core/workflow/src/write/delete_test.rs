//! `write::delete` 的单元测试（desktop-change-delete 写面）：按 change **id**
//! 寻址 → 可选 worktree 清理（目录 + 分支）→ 主仓产物目录清理 → 级联删库。
//! 顺序断言「磁盘先、库最后」；各步幂等（目录 / 分支缺席跳过）。
//!
//! Mock策略（沿 archive_test 先例）：db 半边以进程内假件实现
//! [`ChangeStateStore`]（记录可编程、删除调用捕获、可按臂注入故障）；git 半边
//! 以实现 [`WorktreePort`] 的假件（调用捕获 + 可按臂注入 Err）；文件系统真实
//! tempdir Layout（active / archive 两树真实删目录）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use foundation::layout::{resolve, Layout};

use super::delete::{delete, DeleteOutcome};
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, PhaseStateRecord, RunEdgeStateRecord, RunFinishCommand, RunNodeStateRecord,
    RunStartCommand, RunStateRecord, StepCommand, StepStateRecord, StoreFault,
};
use crate::write::worktree::{InstallRun, RepoProbe, WorktreePort};

/// 身份锚字面量（删除入参——一切寻址以 id 为准）。
const CHANGE_ID: &str = "0198f7a0-0000-7000-8000-0000000000d1";
/// change 名（磁盘目录名 / branch 名供给值）。
const CHANGE_NAME: &str = "seed-change";
/// 库内不存在的 id（未建档拒绝面）。
const UNKNOWN_ID: &str = "0198f7a0-0000-7000-8000-0000000000ff";
/// 归档时间锚（UTC unix millis → `2026-01-02`）。
const ARCHIVED_AT: i64 = 1_767_312_000_000;

// ---------------------------------------------------------------------------
// 假件：ChangeStateStore
// ---------------------------------------------------------------------------

/// 假 store：单记录可编程、级联删除调用捕获、可按臂注入 Err。
struct DeleteStore {
    record: Mutex<Option<ChangeStateRecord>>,
    cascade_calls: Mutex<Vec<String>>,
    cascade_fault: Mutex<Option<StoreFault>>,
    record_after_cascade: Mutex<bool>,
}

impl DeleteStore {
    fn with_record(record: ChangeStateRecord) -> Self {
        Self {
            record: Mutex::new(Some(record)),
            cascade_calls: Mutex::new(Vec::new()),
            cascade_fault: Mutex::new(None),
            record_after_cascade: Mutex::new(true),
        }
    }

    fn cascade_ids(&self) -> Vec<String> {
        self.cascade_calls.lock().expect("调用锁不可中毒").clone()
    }

    /// 删除调用时记录是否仍在（断言顺序：库删在最后一步，调用瞬间记录必在）。
    fn record_present_at_cascade(&self) -> bool {
        *self.record_after_cascade.lock().expect("标记锁不可中毒")
    }
}

/// 建一条最小建档记录。
fn record(
    status: ChangeStatus,
    worktree: Option<&str>,
    archived_at: Option<i64>,
) -> ChangeStateRecord {
    ChangeStateRecord {
        id: CHANGE_ID.to_owned(),
        name: CHANGE_NAME.to_owned(),
        title: CHANGE_NAME.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1_727_000_000_000,
        status,
        archived_at,
        active_phase: None,
        worktree: worktree.map(str::to_owned),
        base_commit: None,
    }
}

impl ChangeStateStore for DeleteStore {
    fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self
            .record
            .lock()
            .expect("记录锁不可中毒")
            .clone()
            .filter(|record| record.id == id))
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
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

    fn delete_change_cascade(&self, id: &str) -> Result<bool, StoreFault> {
        let mut calls = self.cascade_calls.lock().expect("调用锁不可中毒");
        // 顺序探测：级联调用瞬间记录仍在（磁盘步全在它之前）
        *self.record_after_cascade.lock().expect("标记锁不可中毒") =
            self.record.lock().expect("记录锁不可中毒").is_some();
        calls.push(id.to_owned());
        drop(calls);
        if let Some(fault) = self.cascade_fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        *self.record.lock().expect("记录锁不可中毒") = None;
        Ok(true)
    }

    fn start_phase(
        &self,
        _change_id: &str,
        _phase: &str,
        _now: i64,
    ) -> Result<PhaseStartState, StoreFault> {
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

    fn list_run_nodes(&self, _run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_run_edges(&self, _run_id: &str) -> Result<Vec<RunEdgeStateRecord>, StoreFault> {
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
// 假件：WorktreePort
// ---------------------------------------------------------------------------

/// 假 vcs：调用捕获；分支在场表可编程；移除 / 删分支按臂注入 Err。
#[derive(Default)]
struct FakeVcs {
    removed: Mutex<Vec<PathBuf>>,
    deleted_branches: Mutex<Vec<String>>,
    existing_branches: Mutex<Vec<String>>,
    remove_fault: Mutex<Option<String>>,
    branch_fault: Mutex<Option<String>>,
}

impl FakeVcs {
    fn with_branch(branch: &str) -> Self {
        let vcs = Self::default();
        vcs.existing_branches
            .lock()
            .expect("分支锁不可中毒")
            .push(branch.to_owned());
        vcs
    }

    fn removed(&self) -> Vec<PathBuf> {
        self.removed.lock().expect("调用锁不可中毒").clone()
    }

    fn deleted_branches(&self) -> Vec<String> {
        self.deleted_branches
            .lock()
            .expect("调用锁不可中毒")
            .clone()
    }
}

impl WorktreePort for FakeVcs {
    fn worktree_root(&self, _main_root: &Path) -> Result<PathBuf, String> {
        unimplemented!("本用例不可达")
    }

    fn probe(&self, _main_root: &Path) -> Result<RepoProbe, String> {
        unimplemented!("本用例不可达")
    }

    fn branch_exists(&self, _main_root: &Path, branch: &str) -> Result<bool, String> {
        Ok(self
            .existing_branches
            .lock()
            .expect("分支锁不可中毒")
            .iter()
            .any(|existing| existing == branch))
    }

    fn add_worktree(
        &self,
        _main_root: &Path,
        _worktree: &Path,
        _branch: &str,
    ) -> Result<(), String> {
        unimplemented!("本用例不可达")
    }

    fn remove_worktree(&self, _main_root: &Path, worktree: &Path) -> Result<(), String> {
        if let Some(fault) = self.remove_fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        // 假件按真实 git 语义移除目录（下游断言目录消失）
        let _ = fs::remove_dir_all(worktree);
        self.removed
            .lock()
            .expect("调用锁不可中毒")
            .push(worktree.to_path_buf());
        Ok(())
    }

    fn delete_branch(&self, _main_root: &Path, branch: &str) -> Result<(), String> {
        if let Some(fault) = self.branch_fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        self.existing_branches
            .lock()
            .expect("分支锁不可中毒")
            .retain(|existing| existing != branch);
        self.deleted_branches
            .lock()
            .expect("调用锁不可中毒")
            .push(branch.to_owned());
        Ok(())
    }

    fn run_install(&self, _cwd: &Path, _command: &str) -> Result<InstallRun, String> {
        unimplemented!("本用例不可达")
    }
}

// ---------------------------------------------------------------------------
// 装置：真实 tempdir 双树
// ---------------------------------------------------------------------------

/// workspace 根 + 布局 + 主仓 worktree 父锚（测试内自建目录）。
struct Env {
    root: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            root: tempfile::Builder::new()
                .prefix("delete-write-test-")
                .tempdir()
                .expect("创建 workspace 临时目录失败"),
        }
    }

    fn main_root(&self) -> PathBuf {
        self.root.path().to_path_buf()
    }

    fn layout(&self) -> Layout {
        resolve(self.root.path())
    }

    /// 在主仓 active 树建 `changes/<name>/`（含一个产物文件）。
    fn make_active_dir(&self) -> PathBuf {
        let dir = self.layout().changes_root.join(CHANGE_NAME);
        fs::create_dir_all(&dir).expect("建 active 目录失败");
        fs::write(dir.join("proposal.md"), "x").expect("写产物失败");
        dir
    }

    /// 在主仓 archive 树建带日期前缀目录 `archive/<date>-<name>/`。
    fn make_archive_dir(&self, date: &str) -> PathBuf {
        let dir = self
            .layout()
            .archive_root
            .join(format!("{date}-{CHANGE_NAME}"));
        fs::create_dir_all(&dir).expect("建 archive 目录失败");
        fs::write(dir.join("proposal.md"), "x").expect("写产物失败");
        dir
    }

    /// worktree 目录（主仓同级临时子树，真实目录）。
    fn make_worktree(&self) -> PathBuf {
        let dir = self.root.path().join(".worktrees").join(CHANGE_NAME);
        fs::create_dir_all(dir.join("openspec/changes").join(CHANGE_NAME))
            .expect("建 worktree 目录失败");
        dir
    }
}

// ---------------------------------------------------------------------------
// 用例
// ---------------------------------------------------------------------------

#[test]
fn 删除勾选清空_worktree与分支移除且产物目录消失记录级联() {
    let env = Env::new();
    let active = env.make_active_dir();
    let worktree = env.make_worktree();
    let store = DeleteStore::with_record(record(
        ChangeStatus::Active,
        Some(&worktree.to_string_lossy()),
        None,
    ));
    let vcs = FakeVcs::with_branch("change/seed-change");

    let outcome: DeleteOutcome = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        true,
    )
    .expect("删除应成功");

    assert_eq!(outcome.name, CHANGE_NAME);
    assert!(outcome.removed_worktree, "勾选清空 → 目录实际移除");
    assert!(outcome.warnings.is_empty(), "勾选清空无保留注记");
    assert!(!worktree.exists(), "worktree 目录应消失");
    assert_eq!(vcs.removed(), vec![worktree.clone()]);
    assert_eq!(
        vcs.deleted_branches(),
        vec!["change/seed-change".to_owned()]
    );
    assert!(!active.exists(), "主仓产物目录应消失");
    assert_eq!(store.cascade_ids(), vec![CHANGE_ID.to_owned()]);
    assert!(
        store.record_present_at_cascade(),
        "级联删库是最后一步（调用瞬间记录仍在）"
    );
}

#[test]
fn 删除不勾选_worktree与分支保留且有注记_产物目录照删() {
    let env = Env::new();
    let active = env.make_active_dir();
    let worktree = env.make_worktree();
    let store = DeleteStore::with_record(record(
        ChangeStatus::Active,
        Some(&worktree.to_string_lossy()),
        None,
    ));
    let vcs = FakeVcs::with_branch("change/seed-change");

    let outcome = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        false,
    )
    .expect("删除应成功");

    assert!(!outcome.removed_worktree);
    assert_eq!(outcome.warnings.len(), 1, "保留注记恰一条");
    assert!(outcome.warnings[0].contains("change/seed-change"));
    assert!(worktree.exists(), "未勾选 → worktree 目录保留");
    assert!(vcs.removed().is_empty(), "零 worktree 移除调用");
    assert!(vcs.deleted_branches().is_empty(), "零分支删除调用");
    assert!(!active.exists());
    assert_eq!(store.cascade_ids(), vec![CHANGE_ID.to_owned()]);
}

#[test]
fn 删除legacy记录_零vcs触点且主仓active目录消失() {
    let env = Env::new();
    let active = env.make_active_dir();
    let store = DeleteStore::with_record(record(ChangeStatus::Active, None, None));
    let vcs = FakeVcs::default();

    let outcome = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        true,
    )
    .expect("legacy 删除应成功");

    assert!(!outcome.removed_worktree, "legacy 记录无 worktree 可移除");
    assert!(outcome.warnings.is_empty(), "legacy 无保留注记");
    assert!(vcs.removed().is_empty());
    assert!(vcs.deleted_branches().is_empty());
    assert!(!active.exists());
    assert_eq!(store.cascade_ids(), vec![CHANGE_ID.to_owned()]);
}

#[test]
fn 删除归档记录_按archived_at前缀命中自身目录并删除() {
    let env = Env::new();
    let archived = env.make_archive_dir("2026-01-02");
    let store = DeleteStore::with_record(record(ChangeStatus::Archived, None, Some(ARCHIVED_AT)));
    let vcs = FakeVcs::default();

    let outcome = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        false,
    )
    .expect("归档删除应成功");

    assert_eq!(outcome.name, CHANGE_NAME);
    assert!(!archived.exists(), "归档前缀目录应消失");
    assert_eq!(store.cascade_ids(), vec![CHANGE_ID.to_owned()]);
}

#[test]
fn 删除worktree已缺席_目录跳过但分支照删_幂等重试() {
    let env = Env::new();
    let active = env.make_active_dir();
    let absent = env.root.path().join(".worktrees").join("absent");
    let store = DeleteStore::with_record(record(
        ChangeStatus::Active,
        Some(&absent.to_string_lossy()),
        None,
    ));
    let vcs = FakeVcs::with_branch("change/seed-change");

    let outcome = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        true,
    )
    .expect("目录缺席应跳过而非失败");

    assert!(!outcome.removed_worktree, "目录缺席 → 零移除");
    assert!(vcs.removed().is_empty());
    // 残留分支必须照删（否则撞 create 前置⑥）
    assert_eq!(
        vcs.deleted_branches(),
        vec!["change/seed-change".to_owned()]
    );
    assert!(!active.exists());
}

#[test]
fn 分支不在场_零删除调用且删除成功() {
    let env = Env::new();
    env.make_active_dir();
    let worktree = env.make_worktree();
    let store = DeleteStore::with_record(record(
        ChangeStatus::Active,
        Some(&worktree.to_string_lossy()),
        None,
    ));
    let vcs = FakeVcs::default();

    delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        true,
    )
    .expect("分支缺席应跳过");

    assert_eq!(vcs.removed(), vec![worktree]);
    assert!(vcs.deleted_branches().is_empty(), "分支不在场 → 零删除调用");
}

#[test]
fn worktree移除失败_删除中断且记录与产物目录原样() {
    let env = Env::new();
    let active = env.make_active_dir();
    let worktree = env.make_worktree();
    let store = DeleteStore::with_record(record(
        ChangeStatus::Active,
        Some(&worktree.to_string_lossy()),
        None,
    ));
    let vcs = FakeVcs::with_branch("change/seed-change");
    *vcs.remove_fault.lock().expect("故障锁不可中毒") =
        Some("git worktree remove 非零退出".to_owned());

    let error = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        true,
    )
    .expect_err("移除失败应显式 Err");

    assert!(error.contains("worktree 移除失败"), "错误文案: {error}");
    assert!(store.cascade_ids().is_empty(), "失败即零库删除");
    assert!(active.exists(), "产物目录原样未动");
    assert!(vcs.deleted_branches().is_empty(), "分支零触碰");
}

#[test]
fn 分支删除失败_删除中断且库记录与产物目录原样() {
    let env = Env::new();
    let active = env.make_active_dir();
    let worktree = env.make_worktree();
    let store = DeleteStore::with_record(record(
        ChangeStatus::Active,
        Some(&worktree.to_string_lossy()),
        None,
    ));
    let vcs = FakeVcs::with_branch("change/seed-change");
    *vcs.branch_fault.lock().expect("故障锁不可中毒") = Some("git branch -D 非零退出".to_owned());

    let error = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        true,
    )
    .expect_err("分支删除失败应显式 Err");

    assert!(error.contains("branch 删除失败"), "错误文案: {error}");
    assert!(store.cascade_ids().is_empty());
    assert!(active.exists(), "库删在最后——产物目录仍应在场");
}

#[test]
fn 级联删库失败_err透出且错误原样() {
    let env = Env::new();
    env.make_active_dir();
    let store = DeleteStore::with_record(record(ChangeStatus::Active, None, None));
    *store.cascade_fault.lock().expect("故障锁不可中毒") =
        Some(StoreFault::Db("写事务失败".to_owned()));
    let vcs = FakeVcs::default();

    let error = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        CHANGE_ID,
        false,
    )
    .expect_err("级联失败应透出");

    assert!(error.contains("写事务失败"), "错误文案: {error}");
    assert_eq!(store.cascade_ids(), vec![CHANGE_ID.to_owned()]);
}

#[test]
fn 未建档id_显式拒绝且零磁盘零库变更() {
    let env = Env::new();
    let active = env.make_active_dir();
    let store = DeleteStore::with_record(record(ChangeStatus::Active, None, None));
    let vcs = FakeVcs::default();

    let error = delete(
        &env.main_root(),
        &env.layout(),
        &store,
        &vcs,
        UNKNOWN_ID,
        false,
    )
    .expect_err("未建档应显式拒绝");

    assert!(error.contains("未建档"), "错误文案: {error}");
    assert!(active.exists());
    assert!(store.cascade_ids().is_empty());
    assert!(vcs.removed().is_empty());
}
