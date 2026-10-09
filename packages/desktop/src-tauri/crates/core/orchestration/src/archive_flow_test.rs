//! `archive_flow` 的单元测试（test-design「archive_flow.rs -> archive_flow_test.rs」
//! 节）：假 vcs / 假 worker / 假 store（进程内脚本化假件）+ 真实 tempdir fs
//! 直驱六段阶段机——编排断言留 core，进程执行断言归 vcs git_test 真件节
//!（design D8 分工）。覆盖：全链正向与 vcs 调用序锚、幂等跳过词汇面（无
//! delta specs / 用户跳过 / 干净 / 已合入 / legacy / 已落盘）、零提交分支死法 A
//! 与目录缺失已合入归置（D2 调用序锚）、失败停等与重试续走（D2 幂等空走——
//! 已成功段零重复调用）、合入冲突分支编排（D3–D7：快照 A/B 请求捕获序 /
//! 解冲突会话定式 / 链代收口 / lean 收敛族九形态参数化 / lean 后重试幂等）、
//! 后验纯函数分支穷尽（`verify_resolution` / `residual_markers`——D4）、解冲突
//! prompt 语义锚与 spec 同步 prompt 溯源摘除负断言（AC-9）、SpecSync 段位与
//! 探测源（D8）、Finalize 扩围（D9）、停止收敛（D12 词汇锚）、preflight 读面
//! 聚合（None 三态 / 完成度核算与 `phase_next` done 等价对拍——D6 谓词镜像
//! 佐证）、警告与摘要词汇单点（D12）、ArchiveControl 控制面（登记重入 / 订阅 /
//! 会话槽 / 快照）与信封 serde 线面（tag `ipc` / camelCase 线词）。

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use agent::{
    AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunStatus, ModelLevel, SessionProvenance,
};

use crate::archive_flow::{
    merge_conflict_prompt, preflight, residual_markers, run_archive_flow, spec_sync_prompt,
    verify_resolution, ArchiveControl, ArchiveRequest, ArchiveSpecsStatus, ArchiveStage,
    ArchiveStageState, ArchiveStageStatus, ArchiveSummary, ArchiveUpdate,
};
use crate::port::{
    ArchiveVcsPort, BoxTurnFuture, IndexEntry, MergeOutcome, StatusEntry, WorkerAgentPort,
    WorkerRole, WorkerTurnOutcome, WorkerTurnRequest, WorktreeSnapshot,
};
use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, PhaseStateRecord, StepCommand, StepStateRecord, StoreFault,
};
use workflow::write::{phase_next, phase_table, SessionAnchors};

// ---------------------------------------------------------------------------
// 装置：假 store（record + 相位行内存账 + set_archived 故障注入）
// ---------------------------------------------------------------------------

/// 建档记录 fixture（active 起步；worktree 形态携 worktree 路径）。
fn active_record(name: &str, worktree: Option<String>) -> ChangeStateRecord {
    ChangeStateRecord {
        name: name.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1_790_841_600_000,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree,
        base_commit: None,
    }
}

/// 相位行 fixture（宽松默认值；verdict / skipped / stale 逐形态控制——完成度
/// 谓词对拍的种子面）。
fn phase_row(
    change: &str,
    phase: &str,
    verdict: Verdict,
    skipped: bool,
    stale: bool,
) -> PhaseStateRecord {
    PhaseStateRecord {
        id: 0,
        change: change.to_owned(),
        phase: phase.to_owned(),
        attempt: 1,
        verdict,
        report: String::new(),
        checklist: Vec::<ChecklistItem>::new(),
        skipped,
        stale,
        backtrack_to: None,
        backtrack_reason: None,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
        start_at: None,
        timestamp: 0,
    }
}

/// [`ChangeStateStore`] 进程内假件：record + 相位行内存账；`set_archived` 故障
/// 注入（seal 续半边用例——首次翻转失败呈现半完成态）。链只触达 `get_change`
/// / `list_phase_records` / `set_archived`（含写面 archive 的读半边），其余
/// 方法返回 Ok 缺省。
struct FakeStore {
    record: Mutex<Option<ChangeStateRecord>>,
    phases: Mutex<Vec<PhaseStateRecord>>,
    /// set_archived 剩余失败次数（注入 Err——「改名已完成而翻转失败」半完成）
    seal_faults: AtomicUsize,
    archived_calls: AtomicUsize,
}

impl FakeStore {
    fn new(record: ChangeStateRecord) -> Self {
        Self {
            record: Mutex::new(Some(record)),
            phases: Mutex::new(Vec::new()),
            seal_faults: AtomicUsize::new(0),
            archived_calls: AtomicUsize::new(0),
        }
    }

    fn set_archived_faults(&self, times: usize) {
        self.seal_faults.store(times, Ordering::SeqCst);
    }

    fn archived_calls(&self) -> usize {
        self.archived_calls.load(Ordering::SeqCst)
    }

    fn status(&self) -> ChangeStatus {
        self.record
            .lock()
            .expect("record 锁不可中毒")
            .as_ref()
            .map(|record| record.status)
            .expect("record 应在案")
    }

    fn set_status(&self, status: ChangeStatus) {
        self.record
            .lock()
            .expect("record 锁不可中毒")
            .as_mut()
            .expect("record 应在案")
            .status = status;
    }
}

impl ChangeStateStore for FakeStore {
    fn get_change(&self, name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self
            .record
            .lock()
            .expect("record 锁不可中毒")
            .clone()
            .filter(|record| record.name == name))
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        Ok(self
            .record
            .lock()
            .expect("record 锁不可中毒")
            .clone()
            .into_iter()
            .collect())
    }

    fn list_phase_records(&self, change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        Ok(self
            .phases
            .lock()
            .expect("phases 锁不可中毒")
            .iter()
            .filter(|row| row.change == change)
            .cloned()
            .collect())
    }

    fn list_steps(
        &self,
        _change: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        Ok(Vec::new())
    }

    fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault> {
        let mut guard = self.record.lock().expect("record 锁不可中毒");
        if guard.is_some() {
            return Err(StoreFault::Conflict(format!(
                "同名建档已存在: {}",
                record.name
            )));
        }
        *guard = Some(record);
        Ok(())
    }

    fn delete_change_record(&self, name: &str) -> Result<bool, StoreFault> {
        let mut guard = self.record.lock().expect("record 锁不可中毒");
        let existed = guard.as_ref().is_some_and(|record| record.name == name);
        if existed {
            *guard = None;
        }
        Ok(existed)
    }

    fn start_phase(
        &self,
        _change: &str,
        _phase: &str,
        now: i64,
    ) -> Result<PhaseStartState, StoreFault> {
        Ok(PhaseStartState {
            attempt: 1,
            start_at: now,
        })
    }

    fn log_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        self.phases
            .lock()
            .expect("phases 锁不可中毒")
            .push(phase_row(
                &command.change,
                &command.phase,
                command.verdict,
                command.skipped,
                false,
            ));
        Ok(1)
    }

    fn apply_backtrack(&self, _command: &BacktrackCommand) -> Result<(), StoreFault> {
        Ok(())
    }

    fn amend_decision_session(
        &self,
        _change: &str,
        _phase: &str,
        _session_id: &str,
    ) -> Result<(), StoreFault> {
        Ok(())
    }

    fn set_archived(&self, name: &str, archived_at: i64) -> Result<(), StoreFault> {
        self.archived_calls.fetch_add(1, Ordering::SeqCst);
        if self.seal_faults.load(Ordering::SeqCst) > 0 {
            self.seal_faults.fetch_sub(1, Ordering::SeqCst);
            return Err(StoreFault::Db("注入的翻转失败".to_owned()));
        }
        let mut guard = self.record.lock().expect("record 锁不可中毒");
        match guard.as_mut() {
            Some(record) if record.name == name => {
                record.status = ChangeStatus::Archived;
                record.archived_at = Some(archived_at);
                Ok(())
            }
            _ => Err(StoreFault::NotFound(format!("change 不在案: {name}"))),
        }
    }

    fn append_step(&self, _command: &StepCommand) -> Result<(), StoreFault> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 装置：假 vcs（六方法可编程产出 + Arc 共享捕获句柄——port move 后断言面仍可达）
// ---------------------------------------------------------------------------

/// 假 vcs 的捕获句柄（`FakeVcs::assemble` 出线——全调用与关键参数断言面）。
type VcsCallLog = Arc<Mutex<Vec<String>>>;
type CommitAllLog = Arc<Mutex<Vec<(String, String)>>>;
type CommitPathsLog = Arc<Mutex<Vec<(Vec<String>, String)>>>;
type DirtyLog = Arc<Mutex<Vec<(String, Vec<String>)>>>;

struct VcsCapture {
    calls: VcsCallLog,
    commit_all_calls: CommitAllLog,
    commit_paths_calls: CommitPathsLog,
    dirty_calls: DirtyLog,
}

impl VcsCapture {
    /// 调用序日志句柄（FakeWorker 共享挂载——请求捕获序对拍面）。
    fn calls_handle(&self) -> VcsCallLog {
        Arc::clone(&self.calls)
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("调用序锁不可中毒").clone()
    }

    fn commit_all_calls(&self) -> Vec<(String, String)> {
        self.commit_all_calls
            .lock()
            .expect("commit_all 锁不可中毒")
            .clone()
    }

    fn commit_paths_calls(&self) -> Vec<(Vec<String>, String)> {
        self.commit_paths_calls
            .lock()
            .expect("commit_paths 锁不可中毒")
            .clone()
    }

    fn dirty_calls(&self) -> Vec<(String, Vec<String>)> {
        self.dirty_calls.lock().expect("dirty 锁不可中毒").clone()
    }
}

/// [`ArchiveVcsPort`] 进程内假件：九方法可编程产出 + 全调用捕获（阶段机编排 /
/// 幂等跳过 / 冲突分支 / lean 收敛 / pathspec 参 / preflight 聚合的断言锚）。
/// `merge_arrives_from` 模拟 git merge 把 worktree change 目录带进主仓（seal
/// 改名前的真实 fs 前提——假件不产生真实文件编辑，链内 fs 进度由此面承载）；
/// `with_merge_conflicts` 冲突编程面（`Conflicted` 冲突态保留语义——D3）；
/// `with_snapshots` 快照结果队列（A/B 两拍按调用序消费——后验对比基面）；收
/// 口 / abort Err 注入（D6 收口失败 / D10⑧ abort 二次失败面）。
struct FakeVcs {
    /// dirty 探测产出队列（按调用序消费；耗尽回落 `dirty_default`）
    dirty_results: Mutex<VecDeque<bool>>,
    dirty_default: bool,
    /// 已合入旗（Commit / Merge 段与 Preflight worktree 缺失分支读取；重试用
    /// 例换新假件置位——模拟用户手动解冲突后合入）
    merged: AtomicBool,
    /// merge 注入 Err（Some = 合入失败——冲突 / 状态不允许的 git 语境）
    merge_error: Mutex<Option<String>>,
    /// merge 冲突编程（Some = 该次合入返回 `Conflicted(list)`——冲突态保留，
    /// 裁决归解冲突 agent / lean 收口）
    merge_conflicts: Mutex<Option<Vec<String>>>,
    /// merge 成功后主仓 change 目录到场源（模拟 merge 带入的 worktree 树）
    merge_arrives_from: Mutex<Option<PathBuf>>,
    /// worktree_snapshot 结果队列（`Ok(快照)` / `Err(语境)` 按调用序消费——
    /// A/B 两拍编程；耗尽回落占位快照）
    snapshot_results: Mutex<VecDeque<Result<WorktreeSnapshot, String>>>,
    /// commit_merge 注入 Err（收口提交失败——链侧附人工收口引导）
    commit_merge_error: Mutex<Option<String>>,
    /// abort_merge 注入 Err（lean 收敛序的二次失败面——D10 不静默吞）
    abort_merge_error: Mutex<Option<String>>,
    /// current_branch 固定产出（preflight mergeTarget 面）
    branch: Mutex<Result<String, String>>,
    calls: VcsCallLog,
    commit_all_calls: CommitAllLog,
    commit_paths_calls: CommitPathsLog,
    dirty_calls: DirtyLog,
}

impl FakeVcs {
    fn new() -> Self {
        Self {
            dirty_results: Mutex::new(VecDeque::new()),
            dirty_default: true,
            merged: AtomicBool::new(false),
            merge_error: Mutex::new(None),
            merge_conflicts: Mutex::new(None),
            merge_arrives_from: Mutex::new(None),
            snapshot_results: Mutex::new(VecDeque::new()),
            commit_merge_error: Mutex::new(None),
            abort_merge_error: Mutex::new(None),
            branch: Mutex::new(Ok("main".to_owned())),
            calls: Arc::new(Mutex::new(Vec::new())),
            commit_all_calls: Arc::new(Mutex::new(Vec::new())),
            commit_paths_calls: Arc::new(Mutex::new(Vec::new())),
            dirty_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_dirty(self, results: Vec<bool>) -> Self {
        self.dirty_results
            .lock()
            .expect("dirty 队列锁不可中毒")
            .extend(results);
        self
    }

    fn with_merged(self, merged: bool) -> Self {
        self.merged.store(merged, Ordering::SeqCst);
        self
    }

    fn with_merge_error(self, error: &str) -> Self {
        *self.merge_error.lock().expect("merge 错误锁不可中毒") = Some(error.to_owned());
        self
    }

    /// merge 冲突编程（该次合入返回 `Conflicted(list)`——D3 冲突态保留语义）。
    fn with_merge_conflicts(self, conflicts: Vec<&str>) -> Self {
        *self.merge_conflicts.lock().expect("merge 冲突锁不可中毒") =
            Some(conflicts.into_iter().map(str::to_owned).collect());
        self
    }

    /// worktree_snapshot 结果队列（A/B 两拍——按调用序消费，耗尽回落占位快照）。
    fn with_snapshots(self, results: Vec<Result<WorktreeSnapshot, String>>) -> Self {
        self.snapshot_results
            .lock()
            .expect("快照队列锁不可中毒")
            .extend(results);
        self
    }

    /// commit_merge Err 注入（收口提交失败面——D6 人工收口引导的触发前提）。
    fn with_commit_merge_error(self, error: &str) -> Self {
        *self.commit_merge_error.lock().expect("收口错误锁不可中毒") = Some(error.to_owned());
        self
    }

    /// abort_merge Err 注入（lean 收敛序的二次失败——D10⑧ 附注面）。
    fn with_abort_merge_error(self, error: &str) -> Self {
        *self.abort_merge_error.lock().expect("abort 错误锁不可中毒") = Some(error.to_owned());
        self
    }

    fn with_merge_arrives_from(self, dir: PathBuf) -> Self {
        *self
            .merge_arrives_from
            .lock()
            .expect("merge 到场锁不可中毒") = Some(dir);
        self
    }

    fn with_branch(self, branch: &str) -> Self {
        *self.branch.lock().expect("branch 锁不可中毒") = Ok(branch.to_owned());
        self
    }

    /// 转 port + 捕获句柄（Arc 共享——句柄克隆先于 self move）。
    fn assemble(self) -> (Arc<dyn ArchiveVcsPort>, VcsCapture) {
        let capture = VcsCapture {
            calls: Arc::clone(&self.calls),
            commit_all_calls: Arc::clone(&self.commit_all_calls),
            commit_paths_calls: Arc::clone(&self.commit_paths_calls),
            dirty_calls: Arc::clone(&self.dirty_calls),
        };
        (Arc::new(self), capture)
    }
}

/// 目录树递归拷贝（假 merge 的 worktree → 主仓到场面）。
fn copy_dir(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("假 merge 建目标目录失败");
    for entry in std::fs::read_dir(source)
        .expect("假 merge 读源目录失败")
        .flatten()
    {
        let path = entry.path();
        let dest = target.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest);
        } else {
            std::fs::copy(&path, &dest).expect("假 merge 拷贝文件失败");
        }
    }
}

impl ArchiveVcsPort for FakeVcs {
    fn dirty(&self, root: &Path, paths: &[&str]) -> bool {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("dirty".to_owned());
        self.dirty_calls.lock().expect("dirty 锁不可中毒").push((
            root.to_string_lossy().into_owned(),
            paths.iter().map(|path| path.to_string()).collect(),
        ));
        self.dirty_results
            .lock()
            .expect("dirty 队列锁不可中毒")
            .pop_front()
            .unwrap_or(self.dirty_default)
    }

    fn commit_all(&self, worktree: &Path, message: &str) -> Result<(), String> {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("commit_all".to_owned());
        self.commit_all_calls
            .lock()
            .expect("commit_all 锁不可中毒")
            .push((worktree.to_string_lossy().into_owned(), message.to_owned()));
        Ok(())
    }

    fn branch_merged(&self, _main_root: &Path, _branch: &str) -> Result<bool, String> {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("branch_merged".to_owned());
        Ok(self.merged.load(Ordering::SeqCst))
    }

    fn merge_branch(&self, main_root: &Path, branch: &str) -> Result<MergeOutcome, String> {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("merge_branch".to_owned());
        if let Some(error) = self
            .merge_error
            .lock()
            .expect("merge 错误锁不可中毒")
            .take()
        {
            return Err(error);
        }
        if let Some(conflicts) = self
            .merge_conflicts
            .lock()
            .expect("merge 冲突锁不可中毒")
            .take()
        {
            // 冲突态保留（不消费到场源——冲突时主仓树由真实 fs 夹具承载）
            return Ok(MergeOutcome::Conflicted(conflicts));
        }
        if let Some(source) = self
            .merge_arrives_from
            .lock()
            .expect("merge 到场锁不可中毒")
            .take()
        {
            let target = main_root
                .join("openspec/changes")
                .join(source.file_name().expect("到场源应含 change 目录名"));
            copy_dir(&source, &target);
        }
        let _ = branch;
        Ok(MergeOutcome::Merged)
    }

    fn worktree_snapshot(&self, main_root: &Path) -> Result<WorktreeSnapshot, String> {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("worktree_snapshot".to_owned());
        let _ = main_root;
        self.snapshot_results
            .lock()
            .expect("快照队列锁不可中毒")
            .pop_front()
            .unwrap_or_else(|| {
                // 占位快照（merge 态在案形态——未编程队列时的合成基面；冲突
                // 编程面用例经 `with_snapshots` 注入 A/B 两拍）
                Ok(WorktreeSnapshot {
                    head: "fake-head".to_owned(),
                    merge_head: Some("fake-merge-head".to_owned()),
                    status: Vec::new(),
                    index: Vec::new(),
                })
            })
    }

    fn commit_merge(&self, main_root: &Path) -> Result<(), String> {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("commit_merge".to_owned());
        let _ = main_root;
        match self
            .commit_merge_error
            .lock()
            .expect("收口错误锁不可中毒")
            .take()
        {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn abort_merge(&self, main_root: &Path) -> Result<(), String> {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("abort_merge".to_owned());
        let _ = main_root;
        match self
            .abort_merge_error
            .lock()
            .expect("abort 错误锁不可中毒")
            .take()
        {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn current_branch(&self, _main_root: &Path) -> Result<String, String> {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("current_branch".to_owned());
        self.branch.lock().expect("branch 锁不可中毒").clone()
    }

    fn commit_paths(&self, _main_root: &Path, paths: &[&str], message: &str) -> Result<(), String> {
        self.calls
            .lock()
            .expect("调用序锁不可中毒")
            .push("commit_paths".to_owned());
        self.commit_paths_calls
            .lock()
            .expect("commit_paths 锁不可中毒")
            .push((
                paths.iter().map(|path| path.to_string()).collect(),
                message.to_owned(),
            ));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 装置：假 worker（status 可编程 + 请求全字段捕获 + 可编程停止注入）
// ---------------------------------------------------------------------------

/// [`WorkerAgentPort`] 假引擎：预录产出队列（status 按调用序消费，耗尽回落
/// Completed）+ `WorkerTurnRequest` 全字段捕获（provenance / cwd / prompt 断
/// 言面）；`stop_handle` 在场时 future 收口前置位停止旗——模拟用户在同步会话
/// 进行中点停止（agent 段后检查点的取消旗观测面）。
struct FakeWorker {
    outcomes: Mutex<VecDeque<AgentRunStatus>>,
    stop_handle: Mutex<Option<(Arc<ArchiveControl>, String, String)>>,
    /// 共享调用序日志（与 FakeVcs.calls 同源——vcs 调用与 worker 会话的请求
    /// 捕获序对拍面：A 快照先于 agent、B 快照后于 agent 的编排序锚）
    call_log: Mutex<Option<VcsCallLog>>,
    requests: Arc<Mutex<Vec<WorkerTurnRequest>>>,
    counter: AtomicUsize,
}

impl FakeWorker {
    fn new() -> Self {
        Self {
            outcomes: Mutex::new(VecDeque::new()),
            stop_handle: Mutex::new(None),
            call_log: Mutex::new(None),
            requests: Arc::new(Mutex::new(Vec::new())),
            counter: AtomicUsize::new(0),
        }
    }

    fn with_outcomes(self, outcomes: Vec<AgentRunStatus>) -> Self {
        self.outcomes
            .lock()
            .expect("产出队列锁不可中毒")
            .extend(outcomes);
        self
    }

    /// 挂共享调用序日志（每次 `run` 在日志写 `worker_run` 标记）。
    fn with_call_log(self, log: &VcsCallLog) -> Self {
        *self.call_log.lock().expect("调用序日志锁不可中毒") = Some(Arc::clone(log));
        self
    }

    fn with_stop_handle(self, control: &Arc<ArchiveControl>, root: &str, change: &str) -> Self {
        *self.stop_handle.lock().expect("停止柄锁不可中毒") =
            Some((Arc::clone(control), root.to_owned(), change.to_owned()));
        self
    }

    /// 转 port + 请求捕获句柄（Arc 共享——port move 后断言面仍可达）。
    fn assemble(self) -> (Arc<dyn WorkerAgentPort>, Arc<Mutex<Vec<WorkerTurnRequest>>>) {
        let requests = Arc::clone(&self.requests);
        (Arc::new(self), requests)
    }
}

impl WorkerAgentPort for FakeWorker {
    fn run(&self, turn: WorkerTurnRequest) -> BoxTurnFuture {
        if let Some(log) = self.call_log.lock().expect("调用序日志锁不可中毒").as_ref() {
            log.lock()
                .expect("调用序锁不可中毒")
                .push("worker_run".to_owned());
        }
        self.requests
            .lock()
            .expect("请求捕获锁不可中毒")
            .push(turn.clone());
        let status = self
            .outcomes
            .lock()
            .expect("产出队列锁不可中毒")
            .pop_front()
            .unwrap_or(AgentRunStatus::Completed);
        let stop = self.stop_handle.lock().expect("停止柄锁不可中毒").clone();
        let session_id = format!(
            "sess-archive-{}",
            self.counter.fetch_add(1, Ordering::SeqCst) + 1
        );
        Box::pin(async move {
            if let Some((control, root, change)) = stop {
                control.request_stop(&root, &change);
            }
            Ok(WorkerTurnOutcome {
                session_id,
                status,
                final_message: Some("同步完成".to_owned()),
                transcript: Vec::new(),
            })
        })
    }
}

// ---------------------------------------------------------------------------
// 装置：链驱动环境（tempdir 主仓树 + worktree 树 + 假 store + 空注册表）
// ---------------------------------------------------------------------------

const CHANGE: &str = "archive-flow";

/// 产物三件写盘（目标 change 目录内；`present` 圈定在场子集）。
fn seed_artifacts(change_dir: &Path, present: &[&str]) {
    std::fs::create_dir_all(change_dir).expect("布置 change 目录失败");
    for file in ["proposal.md", "design.md", "tasks.md"] {
        if present.contains(&file) {
            std::fs::write(change_dir.join(file), format!("{file} 产物\n")).expect("布置产物失败");
        }
    }
}

/// 链驱动环境：tempdir 主仓树（active / archive 两树）+ 假 store + 空注册表；
/// worktree 形态另置 tempdir worktree 树（产物三件落 worktree——merge 前的
/// 现实位，D7 探测与产物读取同位）。
struct ChainEnv {
    root_dir: tempfile::TempDir,
    worktree_dir: Option<tempfile::TempDir>,
    store: Arc<FakeStore>,
    control: Arc<ArchiveControl>,
}

impl ChainEnv {
    /// worktree 形态：db 记录携 worktree，产物三件落 worktree 树；主仓两树空
    /// 起步（merge 段假件到场或用例预置主仓目录）。
    fn worktree(tag: &str) -> Self {
        Self::build(tag, true)
    }

    /// legacy 形态：worktree=None，产物三件落主仓 active 树。
    fn legacy(tag: &str) -> Self {
        Self::build(tag, false)
    }

    /// worktree 记录指向不存在目录（未合入形态——前置在场性校验的异常分支）。
    fn worktree_absent(tag: &str) -> Self {
        let root_dir = tempfile::Builder::new()
            .prefix(&format!("archive-flow-{tag}-root-"))
            .tempdir()
            .expect("创建主仓临时目录失败");
        let absent = root_dir.path().join("absent-worktree");
        let store = Arc::new(FakeStore::new(active_record(
            CHANGE,
            Some(absent.to_string_lossy().into_owned()),
        )));
        Self {
            root_dir,
            worktree_dir: None,
            store,
            control: Arc::new(ArchiveControl::new()),
        }
    }

    fn build(tag: &str, with_worktree: bool) -> Self {
        let root_dir = tempfile::Builder::new()
            .prefix(&format!("archive-flow-{tag}-root-"))
            .tempdir()
            .expect("创建主仓临时目录失败");
        let worktree_dir = if with_worktree {
            Some(
                tempfile::Builder::new()
                    .prefix(&format!("archive-flow-{tag}-wt-"))
                    .tempdir()
                    .expect("创建 worktree 临时目录失败"),
            )
        } else {
            None
        };
        let worktree = worktree_dir
            .as_ref()
            .map(|dir| dir.path().to_string_lossy().into_owned());
        // 产物三件布置位：worktree 形态落 worktree 树，legacy 落主仓 active 树
        let base = worktree_dir
            .as_ref()
            .map(|dir| dir.path().join("openspec/changes"))
            .unwrap_or_else(|| root_dir.path().join("openspec/changes"));
        seed_artifacts(
            &base.join(CHANGE),
            &["proposal.md", "design.md", "tasks.md"],
        );
        // archive 树预置（真实 rename 的父目录前提——write::archive_test 同式）
        std::fs::create_dir_all(root_dir.path().join("openspec/changes/archive"))
            .expect("预置 archive 树失败");
        let store = Arc::new(FakeStore::new(active_record(CHANGE, worktree)));
        Self {
            root_dir,
            worktree_dir,
            store,
            control: Arc::new(ArchiveControl::new()),
        }
    }

    fn root(&self) -> String {
        self.root_dir.path().to_string_lossy().into_owned()
    }

    fn worktree_path(&self) -> PathBuf {
        self.worktree_dir
            .as_ref()
            .expect("worktree 形态才有 worktree 树")
            .path()
            .to_path_buf()
    }

    /// worktree 树内的 change 目录（假 merge 到场源）。
    fn worktree_change_dir(&self) -> PathBuf {
        self.worktree_path().join("openspec/changes").join(CHANGE)
    }

    fn main_change_dir(&self) -> PathBuf {
        self.root_dir.path().join("openspec/changes").join(CHANGE)
    }

    fn archive_root(&self) -> PathBuf {
        self.root_dir.path().join("openspec/changes/archive")
    }

    /// archive 树目录名清单（落盘面断言锚）。
    fn archived_dirs(&self) -> Vec<String> {
        match std::fs::read_dir(self.archive_root()) {
            Ok(entries) => entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// delta specs 布置（specs/ 子树 capability 清单；worktree 形态落 worktree
    /// 树、legacy 落主仓 active 树——locate_change 同位）。
    fn seed_specs(&self, capabilities: &[&str]) {
        let base = self
            .worktree_dir
            .as_ref()
            .map(|dir| dir.path().to_path_buf())
            .unwrap_or_else(|| self.root_dir.path().to_path_buf());
        for capability in capabilities {
            let dir = base
                .join("openspec/changes")
                .join(CHANGE)
                .join("specs")
                .join(capability);
            std::fs::create_dir_all(&dir).expect("布置 delta specs 失败");
            std::fs::write(dir.join("spec.md"), "## ADDED Requirements\n")
                .expect("布置 spec.md 失败");
        }
    }

    /// 主仓 active 树到场（已合入形态 / 手动 merge-first 路径预置）。
    fn seed_main_change_dir(&self) {
        seed_artifacts(
            &self.main_change_dir(),
            &["proposal.md", "design.md", "tasks.md"],
        );
    }

    /// 主仓主 specs 目录到场（同步段产物落盘后的 Finalize 扩围前提面——真件
    /// 流程由同步 agent 创建，假 agent 不写 fs，扩围用例预置于此）。
    fn seed_main_specs(&self, capabilities: &[&str]) {
        for capability in capabilities {
            let dir = self.root_dir.path().join("openspec/specs").join(capability);
            std::fs::create_dir_all(&dir).expect("布置主 specs 失败");
            std::fs::write(dir.join("spec.md"), "## ADDED Requirements\n")
                .expect("布置主 spec.md 失败");
        }
    }

    /// 全相位 pass 种子（完成度全绿）。
    fn seed_all_pass(&self) {
        let table = phase_table("requirement").expect("requirement 相位表应在案");
        *self.store.phases.lock().expect("phases 锁不可中毒") = table
            .iter()
            .map(|definition| phase_row(CHANGE, definition.id, Verdict::Pass, false, false))
            .collect();
    }

    fn request(&self, sync_specs: bool) -> ArchiveRequest {
        ArchiveRequest {
            root: self.root(),
            change: CHANGE.to_owned(),
            sync_specs,
        }
    }
}

/// 链驱动：begin 登记 + 订阅先行 + 同步走完 + 全信封回收（链同步驱动——单链
/// 信封总量远小于 broadcast 窗口 512）。
async fn run_chain(
    env: &ChainEnv,
    worker: Arc<dyn WorkerAgentPort>,
    vcs: Arc<dyn ArchiveVcsPort>,
    request: ArchiveRequest,
) -> Vec<ArchiveUpdate> {
    let guard = env
        .control
        .begin(&request.root, &request.change)
        .expect("链登记应成功");
    let mut rx = env
        .control
        .subscribe(&request.root, &request.change)
        .expect("链订阅应成功");
    let store: Arc<dyn ChangeStateStore> = Arc::clone(&env.store) as Arc<dyn ChangeStateStore>;
    run_archive_flow(worker, vcs, store, Arc::clone(&env.control), guard, request).await;
    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }
    updates
}

/// Stage 信封流提取（阶段序与终态断言锚）。
fn stage_states(updates: &[ArchiveUpdate]) -> Vec<ArchiveStageState> {
    updates
        .iter()
        .filter_map(|update| match update {
            ArchiveUpdate::Stage { stage } => Some(stage.clone()),
            _ => None,
        })
        .collect()
}

/// Finished 终态信封提取（summary / error 互斥两态）。
fn finished_of(updates: &[ArchiveUpdate]) -> (Option<ArchiveSummary>, Option<String>) {
    updates
        .iter()
        .find_map(|update| match update {
            ArchiveUpdate::Finished { summary, error } => Some((summary.clone(), error.clone())),
            _ => None,
        })
        .expect("链应收 Finished 终态信封")
}

/// 某阶段的终态行（running 信封之外的最后一次发布——skipped / passed / failed）。
fn terminal_of(stages: &[ArchiveStageState], stage: ArchiveStage) -> ArchiveStageState {
    stages
        .iter()
        .rfind(|state| state.stage == stage && state.status != ArchiveStageStatus::Running)
        .cloned()
        .unwrap_or_else(|| panic!("阶段 {stage:?} 应有终态信封"))
}

// ---------------------------------------------------------------------------
// 全链正向与阶段机编排
// ---------------------------------------------------------------------------

/// 全链正向（worktree + delta specs）：六段终态依序全 passed（新执行序——
/// commit、merge 前置于 specSync）；worker 恰一次（provenance / role /
/// permission / cwd=主 workspace root——D8）；vcs 调用序恰 dirty(worktree) →
/// commit_all → branch_merged → merge_branch → dirty(主仓 pathspec) →
/// commit_paths；seal 落盘 + store 翻转；Finished summary 四字段
///（specs=synced、warnings 空）。
#[tokio::test]
async fn 全链正向_六段全passed且调用序与落盘面齐备() {
    let env = ChainEnv::worktree("full-chain");
    env.seed_specs(&["spec-sync-b", "spec-sync-a"]);
    env.seed_main_specs(&["spec-sync-a", "spec-sync-b"]); // 扩围子树到场（D9 fs 门）
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new()
        .with_merge_arrives_from(env.worktree_change_dir())
        .assemble();
    let (worker, requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    // 阶段序：六段终态依序全 passed（执行序 = 呈现序——D1：commit、merge
    // 前置于 specSync；每段 running → 终态成对由信封行另测）
    let stages = stage_states(&updates);
    let terminal: Vec<ArchiveStage> = stages
        .iter()
        .filter(|state| state.status != ArchiveStageStatus::Running)
        .map(|state| state.stage)
        .collect();
    assert_eq!(
        terminal,
        vec![
            ArchiveStage::Preflight,
            ArchiveStage::Commit,
            ArchiveStage::Merge,
            ArchiveStage::SpecSync,
            ArchiveStage::Seal,
            ArchiveStage::Finalize,
        ],
        "六段终态依序全到（D1 执行序）"
    );
    for stage in terminal {
        assert_eq!(
            terminal_of(&stages, stage).status,
            ArchiveStageStatus::Passed,
            "全链正向 {stage:?} 段应 passed"
        );
    }

    // worker 恰一次：provenance（D4）/ role / permission / cwd（D8：恒主 root
    //——delta specs 已随合入进入主仓 active 树）
    let requests = requests.lock().expect("请求捕获锁不可中毒");
    assert_eq!(requests.len(), 1, "worker 恰一次会话");
    let turn = &requests[0];
    assert_eq!(
        turn.root,
        env.root(),
        "cwd = 主 workspace root（D8：同步基线恒主仓当前 specs）"
    );
    assert_eq!(
        turn.provenance,
        SessionProvenance {
            source: "change".to_owned(),
            source_ref: Some(format!("{CHANGE}/archive/spec-sync")),
        },
        "provenance 定式（D4）"
    );
    assert_eq!(turn.permission, AgentPermissionMode::BypassPermissions);
    assert_eq!(turn.role, WorkerRole::Executor);
    assert_eq!(turn.agent, None, "agent=None（默认解析）");
    assert_eq!(
        turn.continue_session, None,
        "新会话（SessionRef::New 形态）"
    );
    assert_eq!(turn.prompt, spec_sync_prompt(CHANGE), "prompt 单点模板");
    drop(requests);

    // vcs 调用序（编排断言锚：六调用恰序——提交段零 merged 查询，D2）
    assert_eq!(
        vcs_capture.calls(),
        vec![
            "dirty".to_owned(),
            "commit_all".to_owned(),
            "branch_merged".to_owned(),
            "merge_branch".to_owned(),
            "dirty".to_owned(),
            "commit_paths".to_owned(),
        ],
        "vcs 调用序：worktree 脏探 → 提交 → 合入判定 → 合入 → 落盘脏探 → pathspec 提交"
    );
    // 调用参：worktree 提交信息（D10 固定前缀 + change 名）
    let commit_all = vcs_capture.commit_all_calls();
    assert_eq!(commit_all.len(), 1, "worktree 提交恰一次");
    assert_eq!(commit_all[0].1, format!("archive: {CHANGE}"));
    assert_eq!(
        commit_all[0].0,
        env.worktree_path().to_string_lossy(),
        "commit_all 锚 worktree 全境"
    );
    // 调用参：finalize 脏探以已知归档路径 + delta capability specs 子树圈定
    //（pathspec 纪律——D9 扩围；specs 段按探测字母序随行）
    let dirty_calls = vcs_capture.dirty_calls();
    assert_eq!(
        dirty_calls.len(),
        2,
        "dirty 恰两探（worktree 全域 + 主仓 pathspec 并集）"
    );
    assert_eq!(
        dirty_calls[0].1,
        Vec::<String>::new(),
        "worktree 脏探 = 全域（空 paths）"
    );
    let domain = foundation::layout::domain_dir_name();
    let finalize_dirty = &dirty_calls[1];
    assert_eq!(
        finalize_dirty.1[0],
        format!("{domain}/changes/{CHANGE}"),
        "pathspec 一 = <domain>/changes/<name>（domain_dir_name 单点拼）"
    );

    // seal 落盘：主仓 active 树改名进 archive 树；store 翻转 archived
    assert!(
        !env.main_change_dir().exists(),
        "主仓 active 树 change 目录已迁移"
    );
    let archived_dirs = env.archived_dirs();
    assert_eq!(archived_dirs.len(), 1, "恰一个归档目录");
    let archived_dir = archived_dirs[0].clone();
    assert!(
        archived_dir.ends_with(&format!("-{CHANGE}")) && archived_dir.len() > CHANGE.len() + 6,
        "归档目录名 = YYYY-MM-DD-<name> 日期前缀形态: {archived_dir}"
    );
    assert_eq!(env.store.status(), ChangeStatus::Archived, "db 翻转落账");
    assert_eq!(env.store.archived_calls(), 1, "翻转恰一次");
    // finalize pathspec 全量对拍：归档两路径 + delta capability 主 specs 子树
    let expected_second = format!("{domain}/changes/archive/{archived_dir}");
    assert_eq!(
        finalize_dirty.1,
        vec![
            format!("{domain}/changes/{CHANGE}"),
            expected_second.clone(),
            format!("{domain}/specs/spec-sync-a"),
            format!("{domain}/specs/spec-sync-b"),
        ],
        "pathspec = 归档两路径 + delta capability specs 子树（D9 扩围）"
    );
    assert_eq!(finalize_dirty.0, env.root(), "落盘脏探锚主仓 root");

    // commit_paths 调用参：扩围 pathspec + 落盘提交信息（D10）
    let commit_paths = vcs_capture.commit_paths_calls();
    assert_eq!(commit_paths.len(), 1, "pathspec 提交恰一次");
    assert_eq!(
        commit_paths[0].0,
        vec![
            format!("{domain}/changes/{CHANGE}"),
            expected_second.clone(),
            format!("{domain}/specs/spec-sync-a"),
            format!("{domain}/specs/spec-sync-b"),
        ],
        "pathspec = 归档两路径 + delta capability specs 子树（扩围内容不换词汇）"
    );
    assert_eq!(
        commit_paths[0].1,
        format!("archive: move {CHANGE} to archive"),
        "落盘提交信息（D10）"
    );

    // Finished summary 四字段（specs=synced；全绿种子 → warnings 空）
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "成功收口零 error");
    let summary = summary.expect("成功收口携 summary");
    assert_eq!(summary.name, CHANGE);
    assert_eq!(summary.archived_dir, archived_dir, "摘要归档目录与落盘一致");
    assert_eq!(summary.specs, ArchiveSpecsStatus::Synced);
    assert!(summary.warnings.is_empty(), "全 pass 齐备 → warnings 空");
}

/// 无 delta specs：SpecSync skipped（detail「无 delta specs」）+ worker 零调
/// 用 + commit 段照常执行 + summary.specs=none（AC-5 scenario 字面）。
#[tokio::test]
async fn 无delta_specs_同步段skipped且零agent会话() {
    let env = ChainEnv::worktree("no-delta");
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new()
        .with_merge_arrives_from(env.worktree_change_dir())
        .assemble();
    let (worker, requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    let spec_sync = terminal_of(&stages, ArchiveStage::SpecSync);
    assert_eq!(spec_sync.status, ArchiveStageStatus::Skipped);
    assert_eq!(
        spec_sync.detail.as_deref(),
        Some("无 delta specs"),
        "跳过因词汇单点（D12）"
    );
    assert!(
        requests.lock().expect("请求捕获锁不可中毒").is_empty(),
        "零 agent 会话"
    );
    assert_eq!(
        terminal_of(&stages, ArchiveStage::Commit).status,
        ArchiveStageStatus::Passed,
        "commit 段照常执行"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    assert_eq!(summary.expect("成功收口").specs, ArchiveSpecsStatus::None);
    assert_eq!(vcs_capture.commit_all_calls().len(), 1, "worktree 提交照常");
}

/// sync_specs=false（用户跳过同步）：delta specs 在场 + SpecSync skipped
///（detail「用户选择」）+ worker 零调用 + summary.specs=skipped（D13「跳过
/// delta specs 同步，直接归档」checkbox 对译的链侧语义）。
#[tokio::test]
async fn sync_specs_false_同步段skipped用户选择() {
    let env = ChainEnv::worktree("skip-by-user");
    env.seed_specs(&["cap-a"]);
    env.seed_all_pass();
    let (vcs, _capture) = FakeVcs::new()
        .with_merge_arrives_from(env.worktree_change_dir())
        .assemble();
    let (worker, requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(false)).await;

    let stages = stage_states(&updates);
    let spec_sync = terminal_of(&stages, ArchiveStage::SpecSync);
    assert_eq!(spec_sync.status, ArchiveStageStatus::Skipped);
    assert_eq!(
        spec_sync.detail.as_deref(),
        Some("用户选择"),
        "跳过因词汇单点（D12）"
    );
    assert!(
        requests.lock().expect("请求捕获锁不可中毒").is_empty(),
        "用户跳过 → 零 agent 会话"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    assert_eq!(
        summary.expect("成功收口").specs,
        ArchiveSpecsStatus::Skipped
    );
}

/// 干净 worktree：Commit skipped（detail「干净」）+ `commit_all` 零调用（无空
/// 提交）+ merge 照常推进（AC-3 幂等跳过面）。
#[tokio::test]
async fn 干净worktree_提交段skipped且零commit_all() {
    let env = ChainEnv::worktree("clean-wt");
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new()
        .with_dirty(vec![false]) // worktree 脏探 false（finalize 脏探回落 default true）
        .with_merge_arrives_from(env.worktree_change_dir())
        .assemble();
    let (worker, _requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    let commit = terminal_of(&stages, ArchiveStage::Commit);
    assert_eq!(commit.status, ArchiveStageStatus::Skipped);
    assert_eq!(
        commit.detail.as_deref(),
        Some("干净"),
        "跳过因词汇单点（D12）"
    );
    assert!(
        vcs_capture.commit_all_calls().is_empty(),
        "干净 worktree 零提交（无空提交）"
    );
    assert_eq!(
        terminal_of(&stages, ArchiveStage::Merge).status,
        ArchiveStageStatus::Passed,
        "merge 照常推进"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "干净跳过后链照常收口");
    assert!(summary.is_some());
}

/// 已合入双跳过直达收口：worktree 干净 + branch_merged=true → Commit
///（detail「干净」——干净探测唯一跳过依据，D2）/ Merge（detail「已合入」）
/// 双 skipped + `commit_all` / `merge_branch` 零调用 + 链径直 seal→finalize
/// 收口（手动 merge 等价路径——AC-2 scenario）。
#[tokio::test]
async fn 已合入_提交合入双skipped直达收口() {
    let env = ChainEnv::worktree("already-merged");
    env.seed_main_change_dir(); // 手动 merge 已把 change 目录带进主仓
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new()
        .with_merged(true)
        .with_dirty(vec![false])
        .assemble();
    let (worker, _requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    let commit = terminal_of(&stages, ArchiveStage::Commit);
    assert_eq!(
        commit.status,
        ArchiveStageStatus::Skipped,
        "提交段应跳过（干净探测唯一跳过依据）"
    );
    assert_eq!(
        commit.detail.as_deref(),
        Some("干净"),
        "提交段跳过因 = 干净（可达性不构成跳过分支——D2）"
    );
    let merge = terminal_of(&stages, ArchiveStage::Merge);
    assert_eq!(merge.status, ArchiveStageStatus::Skipped, "合入段应跳过");
    assert_eq!(
        merge.detail.as_deref(),
        Some("已合入"),
        "合入段跳过因词汇单点（D12）"
    );
    assert!(
        vcs_capture.commit_all_calls().is_empty()
            && vcs_capture
                .calls()
                .iter()
                .all(|call| call != "merge_branch"),
        "commit_all / merge_branch 零调用"
    );
    assert_eq!(
        env.store.status(),
        ChangeStatus::Archived,
        "直达收口（seal 翻转）"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "已合入形态链照常收口");
    assert!(summary.is_some());
}

/// worktree 缺失且未合入：Preflight failed（Err 引导恢复目录或手动处置）+
/// 后续五段零执行 + vcs 除合入判定外零调用 + store 零写。
#[tokio::test]
async fn worktree缺失且未合入_preflight失败引导() {
    let env = ChainEnv::worktree_absent("missing-wt");
    let (vcs, vcs_capture) = FakeVcs::new().with_merged(false).assemble();
    let (worker, requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    assert_eq!(
        stages
            .iter()
            .filter(|state| state.status != ArchiveStageStatus::Running)
            .count(),
        1,
        "仅 Preflight 一个终态（后续五段零执行）"
    );
    let preflight = terminal_of(&stages, ArchiveStage::Preflight);
    assert_eq!(preflight.status, ArchiveStageStatus::Failed);
    let detail = preflight.detail.expect("失败记因");
    assert!(
        detail.contains("worktree 目录不存在") && detail.contains("请恢复目录"),
        "Err 引导恢复目录或手动处置: {detail}"
    );
    let (_, error) = finished_of(&updates);
    assert_eq!(
        error.as_deref(),
        Some(detail.as_str()),
        "Finished error 与阶段记因一致"
    );
    assert_eq!(
        vcs_capture.calls(),
        vec!["branch_merged".to_owned()],
        "仅合入判定一次（worktree 缺失分支）"
    );
    assert!(
        requests.lock().expect("请求捕获锁不可中毒").is_empty(),
        "零 agent 会话"
    );
    assert_eq!(env.store.status(), ChangeStatus::Active, "store 零写");
    assert_eq!(env.store.archived_calls(), 0, "零翻转");
    assert!(env.archived_dirs().is_empty(), "零归档落盘");
}

/// 前置重校验拒绝两态：未建档 / status=archived → Preflight failed 显式 Err；
/// 零 vcs 调用、零 store 写。
#[tokio::test]
async fn 前置重校验拒绝两态_未建档与已归档() {
    // 未建档：record 缺席
    let env = ChainEnv::legacy("no-record");
    *env.store.record.lock().expect("record 锁不可中毒") = None;
    let (vcs, vcs_capture) = FakeVcs::new().assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let stages = stage_states(&updates);
    let preflight = terminal_of(&stages, ArchiveStage::Preflight);
    assert_eq!(preflight.status, ArchiveStageStatus::Failed);
    assert!(
        preflight
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("未建档"),
        "未建档显式记因: {:?}",
        preflight.detail
    );
    assert!(vcs_capture.calls().is_empty(), "零 vcs 调用");
    assert_eq!(env.store.archived_calls(), 0, "零 store 写");

    // 已归档：status=archived（重复归档拒绝）
    let env = ChainEnv::legacy("archived-record");
    env.store.set_status(ChangeStatus::Archived);
    let (vcs, vcs_capture) = FakeVcs::new().assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let stages = stage_states(&updates);
    let preflight = terminal_of(&stages, ArchiveStage::Preflight);
    assert_eq!(preflight.status, ArchiveStageStatus::Failed);
    assert!(
        preflight
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("已归档"),
        "已归档显式记因: {:?}",
        preflight.detail
    );
    assert!(vcs_capture.calls().is_empty(), "零 vcs 调用");
    assert_eq!(env.store.archived_calls(), 0, "零 store 写");
}

/// merge 非冲突失败停链与重试续走（AC-8 面维持）：`merge_branch` 注入 git
/// 语境 Err（冲突清单空 = 非冲突失败）→ Merge failed + Finished error 含该
/// 语境；seal 零执行；重试（用户手动解冲突后合入 → branch_merged=true +
/// 主仓目录到场 + worktree 已随首轮提交转净）→ Commit 干净跳过 / Merge 已
/// 合入跳过、seal→finalize 收口，`commit_all` 零重复（干净跳过——D2）。
#[tokio::test]
async fn merge冲突_合入段失败停链且重试续走() {
    let env = ChainEnv::worktree("merge-conflict");
    env.seed_specs(&["cap-a"]);
    env.seed_all_pass();

    // 第一轮：merge 注入冲突语境 → 停在 Merge 段
    let (vcs, vcs_capture) = FakeVcs::new()
        .with_merge_error("git merge 失败（退出码 1）: CONFLICT (content): Merge conflict in a.txt")
        .assemble();
    let (worker, requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    let merge = terminal_of(&stages, ArchiveStage::Merge);
    assert_eq!(merge.status, ArchiveStageStatus::Failed, "停在合入段");
    // Err 零加工：Merge failed detail = port Err 串逐字（AC-8——git 语境与手动
    // 处置引导已在 port 侧铸好，链侧不二次包装）
    let port_error = "git merge 失败（退出码 1）: CONFLICT (content): Merge conflict in a.txt";
    assert_eq!(
        merge.detail.as_deref(),
        Some(port_error),
        "detail = port Err 串逐字（零加工）"
    );
    let (_, error) = finished_of(&updates);
    assert_eq!(
        error.as_deref(),
        Some(port_error),
        "Finished error 与 port Err 串逐字一致"
    );
    // 非冲突失败分支面：零 agent 会话、零快照、链侧不二次 abort（port 内尽力
    // 已执行）——冲突分支编排零触达
    assert!(
        requests.lock().expect("请求捕获锁不可中毒").is_empty(),
        "零 agent 会话"
    );
    assert!(
        vcs_capture
            .calls()
            .iter()
            .all(|call| call != "worktree_snapshot"),
        "worktree_snapshot 零调用"
    );
    assert!(
        vcs_capture.calls().iter().all(|call| call != "abort_merge"),
        "链侧不二次 abort（port 内尽力已执行）"
    );
    assert!(
        vcs_capture
            .calls()
            .iter()
            .all(|call| call != "commit_paths"),
        "seal 前停链 → 零落盘提交"
    );
    assert_eq!(
        env.store.status(),
        ChangeStatus::Active,
        "store 零翻转（零归档变更）"
    );
    assert_eq!(env.store.archived_calls(), 0);
    assert!(!env.main_change_dir().exists(), "主仓零改名");
    assert!(env.archived_dirs().is_empty(), "零归档落盘");
    assert_eq!(
        vcs_capture.commit_all_calls().len(),
        1,
        "worktree 提交已成功（第一轮）"
    );

    // 重试：用户手动解冲突并合入（branch_merged=true + 主仓目录到场 + worktree
    // 首轮已提交转净）→ 干净 / 已合入双跳过直达收口；commit_all 零重复（D2
    // 干净探测唯一跳过依据）
    env.seed_main_change_dir();
    let (vcs, retry_capture) = FakeVcs::new()
        .with_merged(true)
        .with_dirty(vec![false])
        .assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    for stage in [ArchiveStage::Commit, ArchiveStage::Merge] {
        assert_eq!(
            terminal_of(&stages, stage).status,
            ArchiveStageStatus::Skipped,
            "重试 {stage:?} 段跳过"
        );
    }
    assert!(
        retry_capture.commit_all_calls().is_empty(),
        "已成功段零重复调用（干净探测跳过）"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "重试续走收口");
    assert!(summary.is_some());
    assert_eq!(env.store.status(), ChangeStatus::Archived, "收口翻转落账");
}

/// agent 失败停链可重试（D8 段位后移形态）：假 worker outcome status ≠
/// Completed → SpecSync failed；已成功的提交与合入不回滚（commit_all /
/// merge_branch 各一——新链形下两段前置于同步），归档变更零（commit_paths /
/// store / fs 改名零）；重试 worker Completed + merge 假件到场 → 全链收口
///（specs=synced）。
#[tokio::test]
async fn agent失败_同步段停链零归档变更_重试续走() {
    let env = ChainEnv::worktree("agent-fail");
    env.seed_specs(&["cap-a"]);
    env.seed_all_pass();

    let (vcs, vcs_capture) = FakeVcs::new().assemble();
    let (worker, _requests) = FakeWorker::new()
        .with_outcomes(vec![AgentRunStatus::Failed])
        .assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    let spec_sync = terminal_of(&stages, ArchiveStage::SpecSync);
    assert_eq!(spec_sync.status, ArchiveStageStatus::Failed, "停在同步段");
    assert!(
        spec_sync
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("spec 同步会话失败"),
        "失败记因: {:?}",
        spec_sync.detail
    );
    // 提交与合入已成功且不回滚（新链形两段前置于同步——信封在场且 passed）；
    // 归档变更零
    assert_eq!(
        terminal_of(&stages, ArchiveStage::Commit).status,
        ArchiveStageStatus::Passed,
        "提交段已成功（信封在场）"
    );
    assert_eq!(
        terminal_of(&stages, ArchiveStage::Merge).status,
        ArchiveStageStatus::Passed,
        "合入段已成功（信封在场——R4 半面：已落主仓产物保留不回滚）"
    );
    assert_eq!(
        vcs_capture.commit_all_calls().len(),
        1,
        "worktree 提交已成功（保留不回滚）"
    );
    assert!(
        vcs_capture
            .calls()
            .iter()
            .all(|call| call != "commit_paths"),
        "零落盘提交（seal 前停链）"
    );
    assert_eq!(env.store.status(), ChangeStatus::Active, "store 零写");
    assert_eq!(env.store.archived_calls(), 0);
    assert!(
        !env.main_change_dir().exists() && env.archived_dirs().is_empty(),
        "fs 零改名（零归档变更）"
    );

    // 重试：worker Completed + merge 假件到场 → 全链收口（specs=synced）
    let (vcs, _capture) = FakeVcs::new()
        .with_merge_arrives_from(env.worktree_change_dir())
        .assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let stages = stage_states(&updates);
    assert_eq!(
        terminal_of(&stages, ArchiveStage::SpecSync).status,
        ArchiveStageStatus::Passed,
        "重试自同步段续走成功"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    assert_eq!(summary.expect("收口").specs, ArchiveSpecsStatus::Synced);
}

/// 停止收敛：取消旗置位（agent 段后检查点）→ 链以 Finished error「归档链已
/// 停止」收敛（D12 词汇锚）；停止位前的提交 / 合入已成功（新链形前置于同步
/// ——产物保留零回滚），Seal / Finalize 零执行、store 零写。
#[tokio::test]
async fn 停止旗置位_链以已停止词汇收敛() {
    let env = ChainEnv::worktree("stop");
    env.seed_specs(&["cap-a"]);
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new().assemble();
    let (worker, _requests) = FakeWorker::new()
        .with_stop_handle(&env.control, &env.root(), CHANGE)
        .assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let (_, error) = finished_of(&updates);
    assert_eq!(
        error.as_deref(),
        Some("归档链已停止"),
        "停止收敛词汇单点（D12）"
    );
    let stages = stage_states(&updates);
    assert_eq!(
        terminal_of(&stages, ArchiveStage::SpecSync).status,
        ArchiveStageStatus::Passed,
        "同步段自身完成（停止旗在 agent 段后检查点生效）"
    );
    for stage in [ArchiveStage::Commit, ArchiveStage::Merge] {
        assert_eq!(
            terminal_of(&stages, stage).status,
            ArchiveStageStatus::Passed,
            "{stage:?} 段已成功（前置于同步——停止不回滚已成功段）"
        );
    }
    assert!(
        stages
            .iter()
            .all(|state| state.stage != ArchiveStage::Seal && state.stage != ArchiveStage::Finalize),
        "后续阶段零执行（Seal / Finalize）"
    );
    assert_eq!(
        vcs_capture.calls(),
        vec![
            "dirty".to_owned(),
            "commit_all".to_owned(),
            "branch_merged".to_owned(),
            "merge_branch".to_owned(),
        ],
        "vcs 调用恰序（停止位前：脏探 → 提交 → 合入判定 → 合入）"
    );
    assert_eq!(env.store.status(), ChangeStatus::Active, "store 零写");
}

/// legacy 跳 git 段：worktree=None → Commit / Merge 双 skipped（detail
/// 「legacy 无 worktree」）+ git 段零调用（仅 finalize 的 dirty / commit_paths
/// 半边）+ delta specs 在场时 agent cwd = 主 workspace root（D7 legacy）。
#[tokio::test]
async fn legacy_提交合入双skipped且cwd落主root() {
    let env = ChainEnv::legacy("legacy");
    env.seed_specs(&["cap-a"]);
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new().assemble();
    let (worker, requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    for stage in [ArchiveStage::Commit, ArchiveStage::Merge] {
        let state = terminal_of(&stages, stage);
        assert_eq!(
            state.status,
            ArchiveStageStatus::Skipped,
            "{stage:?} 段应整段跳过"
        );
        assert_eq!(
            state.detail.as_deref(),
            Some("legacy 无 worktree"),
            "跳过因词汇单点（D12）"
        );
    }
    let calls = vcs_capture.calls();
    assert!(
        calls
            .iter()
            .all(|call| call != "commit_all" && call != "merge_branch" && call != "branch_merged"),
        "git 段零调用，实际: {calls:?}"
    );
    assert_eq!(
        calls,
        vec!["dirty".to_owned(), "commit_paths".to_owned()],
        "仅 finalize 的 dirty / commit_paths 半边"
    );
    let requests = requests.lock().expect("请求捕获锁不可中毒");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].root,
        env.root(),
        "agent cwd = 主 workspace root（D7 legacy）"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "legacy 链照常收口");
    assert!(summary.is_some());
}

/// seal 续半边仅补翻转：假 store `set_archived` 首次注入 Err（改名已落盘）→
/// Seal failed 呈现半完成；重试：fs 已在 archive 树 + 翻转成功 → 续半边仅补
/// 翻转（不重复改名——archive 目录名不变）；Finalize dirty=false → skipped
///（已落盘）。
#[tokio::test]
async fn seal续半边_重试仅补翻转() {
    let env = ChainEnv::legacy("seal-half");
    env.seed_all_pass();
    env.store.set_archived_faults(1);
    let (vcs, first_capture) = FakeVcs::new().with_dirty(vec![false]).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();

    // 第一轮：改名落盘而翻转失败 → Seal failed 半完成态
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let stages = stage_states(&updates);
    let seal = terminal_of(&stages, ArchiveStage::Seal);
    assert_eq!(seal.status, ArchiveStageStatus::Failed, "停在收口段");
    let detail = seal.detail.expect("半完成记因");
    assert!(
        detail.contains("改名已完成") && detail.contains("翻转失败"),
        "半完成态呈现（改名 + 翻转两半边各自记因）: {detail}"
    );
    assert_eq!(env.store.status(), ChangeStatus::Active, "db 仍 active");
    assert_eq!(env.store.archived_calls(), 1, "翻转已尝试一次");
    let archived_dirs = env.archived_dirs();
    assert_eq!(
        archived_dirs.len(),
        1,
        "fs 半边已落盘（目录已在 archive 树）"
    );
    let archived_dir = archived_dirs[0].clone();

    // 重试：fs 已在 archive 树 → 续半边仅补翻转；finalize 脏探 false → skipped
    env.store.set_archived_faults(0);
    let (vcs, retry_capture) = FakeVcs::new().with_dirty(vec![false]).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let stages = stage_states(&updates);
    let seal = terminal_of(&stages, ArchiveStage::Seal);
    assert_eq!(seal.status, ArchiveStageStatus::Passed, "续半边补翻转成功");
    assert_eq!(
        terminal_of(&stages, ArchiveStage::Finalize).status,
        ArchiveStageStatus::Skipped,
        "finalize 已落盘跳过"
    );
    assert_eq!(
        env.archived_dirs(),
        vec![archived_dir.clone()],
        "archive 目录名不变（不重复改名）"
    );
    assert_eq!(env.store.archived_calls(), 2, "翻转共两次（失败 + 补齐）");
    assert_eq!(env.store.status(), ChangeStatus::Archived);
    assert!(
        retry_capture.commit_paths_calls().is_empty()
            && first_capture.commit_paths_calls().is_empty(),
        "零 pathspec 提交（已落盘跳过）"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    let summary = summary.expect("重试收口");
    assert_eq!(
        summary.archived_dir, archived_dir,
        "摘要归档目录与续半边落盘一致"
    );
}

/// finalize 已落盘跳过：假 vcs `dirty(main_root, [old, new])`=false →
/// Finalize skipped（detail「已落盘」）+ `commit_paths` 零调用（重复收口重试
/// 的幂等面——D11 脏探测守卫）。
#[tokio::test]
async fn finalize已落盘_skipped且零commit_paths() {
    let env = ChainEnv::worktree("finalize-done");
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new()
        .with_dirty(vec![true, false]) // worktree 脏 → 提交；主仓 pathspec 探 false → 跳过
        .with_merge_arrives_from(env.worktree_change_dir())
        .assemble();
    let (worker, _requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    let finalize = terminal_of(&stages, ArchiveStage::Finalize);
    assert_eq!(finalize.status, ArchiveStageStatus::Skipped);
    assert_eq!(
        finalize.detail.as_deref(),
        Some("已落盘"),
        "跳过因词汇单点（D12）"
    );
    assert!(
        vcs_capture.commit_paths_calls().is_empty(),
        "脏探 false → 零 pathspec 提交（幂等空走）"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "落盘跳过不阻断收口");
    assert!(summary.is_some());
}

// ---------------------------------------------------------------------------
// preflight 读面聚合（design D6/D7）
// ---------------------------------------------------------------------------

/// preflight 读面聚合全字段：全 pass 相位 + 产物三件 + 两 capability delta
/// specs + worktree 记录 → Some 且 completed=true / incompletePhases 空 /
/// missingArtifacts 空 / deltaSpecs 两 capability 名（字母序）/ worktree 出线 /
/// mergeTarget=注入名 / runActive 两态透传。
#[test]
fn preflight读面聚合全字段() {
    let env = ChainEnv::worktree("preflight-full");
    env.seed_specs(&["cap-b", "cap-a"]);
    env.seed_all_pass();
    let (vcs, _capture) = FakeVcs::new().with_branch("main").assemble();
    let worktree = env.worktree_path().to_string_lossy().into_owned();

    let read_active_false = preflight(
        env.root_dir.path(),
        env.store.as_ref(),
        Some(&worktree),
        CHANGE,
        vcs.as_ref(),
        false,
    )
    .expect("可归档 → Some");
    assert_eq!(read_active_false.name, CHANGE);
    assert!(read_active_false.completed, "全 pass → completed=true");
    assert!(read_active_false.incomplete_phases.is_empty());
    assert!(read_active_false.missing_artifacts.is_empty(), "三件齐备");
    assert_eq!(
        read_active_false.delta_specs,
        vec!["cap-a".to_owned(), "cap-b".to_owned()],
        "delta specs capability 清单（字母序）"
    );
    assert_eq!(
        read_active_false.worktree.as_deref(),
        Some(worktree.as_str())
    );
    assert_eq!(
        read_active_false.branch.as_deref(),
        Some("change/archive-flow"),
        "branch 定式 change/<name>"
    );
    assert_eq!(
        read_active_false.merge_target.as_deref(),
        Some("main"),
        "合入目标 = 主仓当前分支"
    );
    assert!(!read_active_false.run_active, "runActive 透传（false 态）");

    let read_active_true = preflight(
        env.root_dir.path(),
        env.store.as_ref(),
        Some(&worktree),
        CHANGE,
        vcs.as_ref(),
        true,
    )
    .expect("可归档 → Some");
    assert!(read_active_true.run_active, "runActive 透传（true 态）");
}

/// preflight None 三态：未建档 / status=archived / 未知名 → None（AC-1「不可
/// 归档」读面兜底——前端据此不呈现入口路径）。
#[test]
fn preflight_none三态_不可归档兜底() {
    let (vcs, _capture) = FakeVcs::new().assemble();

    // 未知名：库内查无记录
    let env = ChainEnv::legacy("none-unknown");
    assert!(
        preflight(
            env.root_dir.path(),
            env.store.as_ref(),
            None,
            "no-such-change",
            vcs.as_ref(),
            false
        )
        .is_none(),
        "未知名 → None"
    );

    // 未归档不可归档兜底之外的前置：preflight 对 archived 记录同样 None（下行复用）
    let env = ChainEnv::legacy("none-archived");
    env.store.set_status(ChangeStatus::Archived);
    assert!(
        preflight(
            env.root_dir.path(),
            env.store.as_ref(),
            None,
            CHANGE,
            vcs.as_ref(),
            false
        )
        .is_none(),
        "status=archived → None"
    );

    // 未建档：db 零记录（文档形态）
    let env = ChainEnv::legacy("none-no-record");
    *env.store.record.lock().expect("record 锁不可中毒") = None;
    assert!(
        preflight(
            env.root_dir.path(),
            env.store.as_ref(),
            None,
            CHANGE,
            vcs.as_ref(),
            false
        )
        .is_none(),
        "未建档 → None"
    );
}

/// 完成度核算与 phase_next 等价：同一组 PhaseRecord 种子（全 pass / stale
/// pass / fail 条目 / skipped 补位 / 缺相位五形态）驱动 `preflight().completed`
/// 与 `workflow::write::phase_next(...).done` 对拍一致；incompletePhases = 未过
/// 相位清单（D6 谓词镜像等价性佐证——相位机零改动红线的两头锚）。
#[test]
fn 完成度核算与phase_next_done等价对拍() {
    let env = ChainEnv::legacy("predicate-parity");
    let table = phase_table("requirement").expect("requirement 相位表应在案");
    let anchors = SessionAnchors::new();

    let preflight_completed = |env: &ChainEnv| {
        let (vcs, _capture) = FakeVcs::new().assemble();
        preflight(
            env.root_dir.path(),
            env.store.as_ref(),
            None,
            CHANGE,
            vcs.as_ref(),
            false,
        )
        .expect("active 建档 → Some")
    };
    let phase_next_done = |env: &ChainEnv| {
        phase_next(env.store.as_ref(), CHANGE, "run-parity", &anchors)
            .expect("phase_next 应 Ok")
            .done
    };

    // 形态一：全 pass → 双 true
    env.seed_all_pass();
    assert!(
        preflight_completed(&env).completed && phase_next_done(&env),
        "全 pass → 双 true"
    );

    // 形态二：stale pass（某相位最新条目 stale）→ 双 false
    *env.store.phases.lock().expect("phases 锁不可中毒") = table
        .iter()
        .map(|definition| {
            phase_row(
                CHANGE,
                definition.id,
                Verdict::Pass,
                false,
                definition.id == "implement",
            )
        })
        .collect();
    let read = preflight_completed(&env);
    assert!(
        !read.completed && !phase_next_done(&env),
        "stale pass → 双 false"
    );
    assert_eq!(
        read.incomplete_phases,
        vec!["implement".to_owned()],
        "未过相位清单"
    );

    // 形态三：fail 条目（implement 非 stale fail）→ 双 false
    *env.store.phases.lock().expect("phases 锁不可中毒") = table
        .iter()
        .map(|definition| {
            if definition.id == "implement" {
                phase_row(CHANGE, definition.id, Verdict::Fail, false, false)
            } else {
                phase_row(CHANGE, definition.id, Verdict::Pass, false, false)
            }
        })
        .collect();
    assert!(
        !preflight_completed(&env).completed && !phase_next_done(&env),
        "fail 条目 → 双 false"
    );

    // 形态四：skipped 补位（全相位 skipped 非 stale）→ 双 true
    env.seed_all_pass();
    *env.store.phases.lock().expect("phases 锁不可中毒") = table
        .iter()
        .map(|definition| phase_row(CHANGE, definition.id, Verdict::Fail, true, false))
        .collect();
    assert!(
        preflight_completed(&env).completed && phase_next_done(&env),
        "skipped 补位（pass OR skipped 谓词）→ 双 true"
    );

    // 形态五：缺相位（仅 proposal pass）→ 双 false + incomplete 列出 dev-design
    *env.store.phases.lock().expect("phases 锁不可中毒") =
        vec![phase_row(CHANGE, "proposal", Verdict::Pass, false, false)];
    let read = preflight_completed(&env);
    assert!(
        !read.completed && !phase_next_done(&env),
        "缺相位 → 双 false"
    );
    assert_eq!(
        read.incomplete_phases.first().map(String::as_str),
        Some("dev-design"),
        "incompletePhases = 相位表序的首个未过相位"
    );
}

/// workflow_type 不支持：phase_table None 的类型 → preflight completed=false；
/// 链收口 summary.warnings 含「工作流类型 "<type>" 无相位表，完成度不可核算」
///（D12 词汇锚）。
#[tokio::test]
async fn workflow_type不支持_完成度不可核算词汇() {
    let env = ChainEnv::legacy("unsupported-type");
    env.store
        .record
        .lock()
        .expect("record 锁不可中毒")
        .as_mut()
        .expect("record 应在案")
        .workflow_type = "bug-fix".to_owned();
    let (vcs, _capture) = FakeVcs::new().with_dirty(vec![false]).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();

    // 读面：completed=false + 空 incomplete（不可核算）
    let read = preflight(
        env.root_dir.path(),
        env.store.as_ref(),
        None,
        CHANGE,
        vcs.as_ref(),
        false,
    )
    .expect("active 建档 → Some");
    assert!(!read.completed, "无相位表 → completed=false");
    assert!(
        read.incomplete_phases.is_empty(),
        "不可核算 → 空 incomplete"
    );

    // 链收口：专项警告词汇随 summary 流出
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "警告不阻断链（AC-2）");
    let summary = summary.expect("照常收口");
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.contains("工作流类型 \"bug-fix\" 无相位表，完成度不可核算")),
        "workflow_type 专项警告词汇（D12）: {:?}",
        summary.warnings
    );
}

/// 摘要警告词汇：未完成 change 收口 → warnings 含「工作流未全部通过：<未过
/// 相位清单>」；缺产物 → 含「缺少产物文档：<三件子集>」；全 pass 且三件齐备 →
/// warnings 空清单（D12 单点 + AC-2 无警告面）。
#[tokio::test]
async fn 摘要警告词汇_未通过与缺产物两行() {
    // 未完成 + 缺产物：全部相位 fail（连锁未过）+ design.md 缺席
    let env = ChainEnv::legacy("warn-both");
    let table = phase_table("requirement").expect("requirement 相位表应在案");
    *env.store.phases.lock().expect("phases 锁不可中毒") = table
        .iter()
        .map(|definition| phase_row(CHANGE, definition.id, Verdict::Fail, false, false))
        .collect();
    std::fs::remove_file(env.main_change_dir().join("design.md")).expect("构造缺产物失败");
    let (vcs, _capture) = FakeVcs::new().with_dirty(vec![false]).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "警告不阻断（inform + confirm——AC-2）");
    let warnings = &summary.expect("照常收口").warnings;
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("工作流未全部通过：")),
        "完成度警告行: {:?}",
        warnings
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("proposal") && warning.contains("dev-design")),
        "未过相位清单随行: {:?}",
        warnings
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("缺少产物文档：") && warning.contains("design.md")),
        "产物缺失警告行: {:?}",
        warnings
    );

    // 全 pass 且三件齐备：warnings 空清单
    let env = ChainEnv::legacy("warn-clean");
    env.seed_all_pass();
    let (vcs, _capture) = FakeVcs::new().with_dirty(vec![false]).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    assert!(
        summary.expect("收口").warnings.is_empty(),
        "全绿齐备 → 无警告（轻量确认面）"
    );
}

/// prompt 语义锚：假 worker 捕获 `turn.prompt`——含 change 名插值、
/// `## ADDED/MODIFIED/REMOVED/RENAMED Requirements` 增量语义四行、「保留 delta
/// 未提及的主 spec 内容」与「合并幂等」指令逐字在场；禁止段——禁 MCP 工具
///（change_list 等）、禁 `__TOOL_ASK_USER__`、禁读写 workflow.json、禁 git
/// 命令逐字在场（AC-5 prompt MUST NOT 依赖面的文本锚）。
#[tokio::test]
async fn prompt语义锚_增量语义与禁令逐字在场() {
    let env = ChainEnv::worktree("prompt-anchor");
    env.seed_specs(&["cap-a"]);
    let (vcs, _capture) = FakeVcs::new()
        .with_merge_arrives_from(env.worktree_change_dir())
        .assemble();
    let (worker, requests) = FakeWorker::new().assemble();
    run_chain(&env, worker, vcs, env.request(true)).await;

    let requests = requests.lock().expect("请求捕获锁不可中毒");
    assert_eq!(requests.len(), 1);
    let prompt = &requests[0].prompt;

    // change 名插值
    assert!(prompt.contains(CHANGE), "change 名插值在场");
    // 增量语义四行（design 数据模型逐字）
    for section in [
        "## ADDED Requirements",
        "## MODIFIED Requirements",
        "## REMOVED Requirements",
        "## RENAMED Requirements",
    ] {
        assert!(prompt.contains(section), "增量语义行在场: {section}");
    }
    // 保留未提及内容 + 幂等指令逐字
    assert!(
        prompt.contains("保留 delta 未提及的主 spec 内容"),
        "保留未提及内容指令逐字在场"
    );
    assert!(prompt.contains("合并幂等"), "合并幂等指令逐字在场");
    // 禁止段（MUST NOT 依赖面）
    assert!(prompt.contains("禁止调用 MCP 工具"), "禁 MCP 工具禁令在场");
    assert!(prompt.contains("change_list"), "禁 change_list 等点名在场");
    assert!(prompt.contains("__TOOL_ASK_USER__"), "禁 ask 占位禁令在场");
    assert!(
        prompt.contains("workflow.json"),
        "禁读写 workflow.json 禁令在场"
    );
    assert!(
        prompt.contains("不执行任何 git 命令"),
        "禁 git 命令禁令在场"
    );
    // 溯源摘除负断言（AC-9——增量合并语义自持，零 skill 溯源措辞）
    assert!(
        !prompt.contains("openspec-archive-change"),
        "整串不含 skill 名（溯源摘除）"
    );
    assert!(!prompt.contains("skill"), "整串不含 skill 措辞（溯源摘除）");
}

// ---------------------------------------------------------------------------
// ArchiveControl 控制面与信封形态
// ---------------------------------------------------------------------------

/// ArchiveControl 登记与重入：`begin` 在案同键二次 `begin` → Err（含 change
/// 名——发起幂等防护）；`ArchiveGuard::finish` 后 is_active=false / snapshot=
/// None（终态除名）；异键互不误拒。
#[test]
fn archive_control登记与重入防护() {
    let control = Arc::new(ArchiveControl::new());
    let guard = control.begin("root-a", CHANGE).expect("首次登记应成功");
    assert!(control.is_active("root-a", CHANGE), "登记后在案");

    let error = match control.begin("root-a", CHANGE) {
        Err(error) => error,
        Ok(_) => panic!("同键重入应 Err（发起幂等防护）"),
    };
    assert!(error.contains(CHANGE), "重入 Err 含 change 名: {error}");

    // 异键互不误拒：异 root 同名 / 同 root 异名均放行
    let _other_root = control.begin("root-b", CHANGE).expect("异 root 同名放行");
    let _other_change = control
        .begin("root-a", "other-change")
        .expect("同 root 异名放行");

    guard.finish(None, Some("终态".to_owned()));
    assert!(!control.is_active("root-a", CHANGE), "终态除名");
    assert!(
        control.snapshot("root-a", CHANGE).is_none(),
        "终态后快照 None"
    );
    let _re = control.begin("root-a", CHANGE).expect("除名后可再登记");
}

/// 控制面订阅与会话槽：publish → 订阅者收 ArchiveUpdate 信封；set_session /
/// current_session 往返；request_stop miss（无在案）幂等 false；双复合键
/// (root, change) 互不串台。
#[test]
fn 控制面订阅与会话槽_复合键隔离() {
    let control = Arc::new(ArchiveControl::new());
    let guard_a = control.begin("root-a", "change-a").expect("登记 a");
    let guard_b = control.begin("root-a", "change-b").expect("登记 b");
    let mut rx_a = control.subscribe("root-a", "change-a").expect("订阅 a");
    let mut rx_b = control.subscribe("root-a", "change-b").expect("订阅 b");
    let stage = ArchiveStageState {
        stage: ArchiveStage::Preflight,
        status: ArchiveStageStatus::Running,
        detail: None,
    };
    control.publish("root-a", "change-a", ArchiveUpdate::Stage { stage });

    // a 收到、b 不串台
    assert!(
        matches!(rx_a.try_recv(), Ok(ArchiveUpdate::Stage { .. })),
        "订阅 a 收信封"
    );
    assert!(
        rx_b.try_recv().is_err(),
        "订阅 b 不收 a 的信封（复合键隔离）"
    );

    // 会话槽往返（同槽互不串台）
    control.set_session("root-a", "change-a", Some("sess-a".to_owned()));
    assert_eq!(
        control.current_session("root-a", "change-a").as_deref(),
        Some("sess-a")
    );
    assert_eq!(
        control.current_session("root-a", "change-b"),
        None,
        "b 槽位独立"
    );

    // request_stop：在案 true + 取消旗可观测；miss 幂等 false
    assert!(!guard_b.cancelled(), "b 未停止");
    assert!(control.request_stop("root-a", "change-a"), "在案停止置位");
    assert!(guard_a.cancelled(), "a 取消旗可观测");
    assert!(
        !control.request_stop("root-miss", "change-miss"),
        "miss 幂等 false"
    );

    guard_a.finish(None, Some("已停止".to_owned()));
    guard_b.finish(None, Some("已停止".to_owned()));
}

/// 快照与信封形态：阶段推进中每段恰 running→终态两信封（后写覆盖同段）；
/// ArchiveSnapshot.stages 累积 + sessionId 随会话槽；Finished 后 snapshot None
///（快照只覆盖运行期——D5）；serde 线面 ArchiveStage 线词 / tag ipc 三变体出线。
#[tokio::test]
async fn 快照与信封形态_阶段成对与serde线面() {
    let env = ChainEnv::legacy("envelope-shape");
    env.seed_all_pass();
    let (vcs, _capture) = FakeVcs::new().with_dirty(vec![false]).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();

    let guard = env
        .control
        .begin(&env.root(), CHANGE)
        .expect("链登记应成功");
    let mut rx = env
        .control
        .subscribe(&env.root(), CHANGE)
        .expect("订阅应成功");
    // 快照累积与会话槽随行（阶段推进中——发布即落快照）
    env.control
        .set_session(&env.root(), CHANGE, Some("sess-shape".to_owned()));
    let store: Arc<dyn ChangeStateStore> = Arc::clone(&env.store) as Arc<dyn ChangeStateStore>;
    run_archive_flow(
        worker,
        vcs,
        store,
        Arc::clone(&env.control),
        guard,
        env.request(true),
    )
    .await;
    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }

    // 每段恰 running → 终态两信封（终态非 running）
    let stages = stage_states(&updates);
    for stage in [
        ArchiveStage::Preflight,
        ArchiveStage::SpecSync,
        ArchiveStage::Commit,
        ArchiveStage::Merge,
        ArchiveStage::Seal,
        ArchiveStage::Finalize,
    ] {
        let rows: Vec<&ArchiveStageState> =
            stages.iter().filter(|state| state.stage == stage).collect();
        assert_eq!(
            rows.len(),
            2,
            "{stage:?} 恰 running→终态两信封，实际: {rows:?}"
        );
        assert_eq!(
            rows[0].status,
            ArchiveStageStatus::Running,
            "{stage:?} 首信封 running"
        );
        assert_ne!(
            rows[1].status,
            ArchiveStageStatus::Running,
            "{stage:?} 次信封为终态"
        );
    }

    // serde 线面：tag ipc 三变体 + ArchiveStage 六值线词 + camelCase 字段
    let wire_stage = serde_json::to_value(&updates[0]).expect("Stage 信封出线");
    assert_eq!(wire_stage["ipc"], "stage", "Stage 变体 tag");
    assert_eq!(wire_stage["stage"]["stage"], "preflight", "阶段线词");
    assert_eq!(wire_stage["stage"]["status"], "running", "状态线词");
    assert!(
        wire_stage["stage"].get("detail").is_some(),
        "camelCase detail 字段在场"
    );

    for (stage, wire) in [
        (ArchiveStage::SpecSync, "specSync"),
        (ArchiveStage::Commit, "commit"),
        (ArchiveStage::Merge, "merge"),
        (ArchiveStage::Seal, "seal"),
        (ArchiveStage::Finalize, "finalize"),
    ] {
        let value = serde_json::to_value(stage).expect("阶段出线");
        assert_eq!(value, wire, "ArchiveStage 线词（camelCase）");
    }

    let event = AgentEvent::stamp(
        0,
        AgentEventKind::Raw {
            event_type: "system".to_owned(),
            raw_json: "{}".to_owned(),
        },
    );
    let wire_event = serde_json::to_value(ArchiveUpdate::SessionEvent {
        session_id: "sess-1".to_owned(),
        event,
    })
    .expect("SessionEvent 信封出线");
    assert_eq!(
        wire_event["ipc"], "sessionEvent",
        "SessionEvent 变体 tag（camelCase）"
    );
    assert_eq!(wire_event["sessionId"], "sess-1", "camelCase sessionId");

    let wire_finished = serde_json::to_value(ArchiveUpdate::Finished {
        summary: None,
        error: Some("x".to_owned()),
    })
    .expect("Finished 信封出线");
    assert_eq!(wire_finished["ipc"], "finished", "Finished 变体 tag");
    assert_eq!(wire_finished["summary"], serde_json::Value::Null);
    assert_eq!(wire_finished["error"], "x");

    // Finished 后 snapshot None（终态除名——快照只覆盖运行期）
    assert!(env.control.snapshot(&env.root(), CHANGE).is_none());

    // 快照形态：登记 + 发布两段 + 会话槽 → stages 累积 + sessionId 透传
    let guard = env
        .control
        .begin(&env.root(), CHANGE)
        .expect("再登记应成功");
    env.control
        .set_session(&env.root(), CHANGE, Some("sess-shape".to_owned()));
    env.control.publish(
        &env.root(),
        CHANGE,
        ArchiveUpdate::Stage {
            stage: ArchiveStageState {
                stage: ArchiveStage::Preflight,
                status: ArchiveStageStatus::Passed,
                detail: None,
            },
        },
    );
    env.control.publish(
        &env.root(),
        CHANGE,
        ArchiveUpdate::Stage {
            stage: ArchiveStageState {
                stage: ArchiveStage::SpecSync,
                status: ArchiveStageStatus::Running,
                detail: None,
            },
        },
    );
    let snapshot = env
        .control
        .snapshot(&env.root(), CHANGE)
        .expect("运行期快照在场");
    assert_eq!(snapshot.stages.len(), 2, "stages 累积");
    assert_eq!(snapshot.stages[0].stage, ArchiveStage::Preflight);
    assert_eq!(
        snapshot.session_id.as_deref(),
        Some("sess-shape"),
        "sessionId 随会话槽"
    );
    // 同段后写覆盖（单槽终值）
    env.control.publish(
        &env.root(),
        CHANGE,
        ArchiveUpdate::Stage {
            stage: ArchiveStageState {
                stage: ArchiveStage::SpecSync,
                status: ArchiveStageStatus::Skipped,
                detail: Some("无 delta specs".to_owned()),
            },
        },
    );
    let snapshot = env
        .control
        .snapshot(&env.root(), CHANGE)
        .expect("快照仍在案");
    assert_eq!(snapshot.stages.len(), 2, "后写覆盖不追加");
    assert_eq!(
        snapshot.stages[1].status,
        ArchiveStageStatus::Skipped,
        "同段单槽终值"
    );
    guard.finish(None, Some("收口".to_owned()));
}

// ---------------------------------------------------------------------------
// 合入冲突分支编排（D3–D7）与 lean 收敛族（D10）：冲突编程面 + 快照队列 +
// 合成快照 fixture（纯函数直驱零假件）+ 真实 tempdir fs（residual_markers
// 行首标记扫描）；零提交分支死法 A 与目录缺失已合入归置（D2 调用序锚）、
// SpecSync 段位与探测源（D8）、Finalize 扩围（D9）
// ---------------------------------------------------------------------------

/// 合成快照的 HEAD / MERGE_HEAD 定值（⑤ 违约收口串的 `git reset --hard` 引导
/// 对拍锚）。
const SNAP_HEAD: &str = "snap-head-aaa";
const SNAP_MERGE_HEAD: &str = "snap-merge-head-bbb";

/// 无关路径（非冲突面的 staged 条目——④ 规则 A/B 逐字一致性的对拍锚）。
const UNRELATED_PATH: &str = "docs/note.txt";

/// porcelain 状态条目直构（合成快照 fixture 面——纯函数直驱零假件）。
fn status_entry(x: char, y: char, path: &str) -> StatusEntry {
    StatusEntry {
        x,
        y,
        path: path.to_owned(),
    }
}

/// 索引条目直构（mode 固定 100644；hash / stage 逐参——D4 对比基面）。
fn index_entry(path: &str, hash: &str, stage: u8) -> IndexEntry {
    IndexEntry {
        mode: "100644".to_owned(),
        hash: hash.to_owned(),
        stage,
        path: path.to_owned(),
    }
}

/// A 基线快照（冲突进行中形态）：冲突路径 src/a.txt（UU + 索引三方 1/2/3）+
/// 无关 staged 条目。单冲突路径——与 lean 共同装置的冲突编程清单（仅 a.txt）
/// 同口径；解冲突链用例的清单扩至 a+b 时 b 以缺席面参与对比（合法消失）。
fn conflict_baseline() -> WorktreeSnapshot {
    WorktreeSnapshot {
        head: SNAP_HEAD.to_owned(),
        merge_head: Some(SNAP_MERGE_HEAD.to_owned()),
        status: vec![
            status_entry('U', 'U', "src/a.txt"),
            status_entry('A', ' ', UNRELATED_PATH),
        ],
        index: vec![
            index_entry("src/a.txt", "base-a", 1),
            index_entry("src/a.txt", "ours-a", 2),
            index_entry("src/a.txt", "theirs-a", 3),
            index_entry(UNRELATED_PATH, "staged-hash", 0),
        ],
    }
}

/// B 通过形态派生：冲突路径单条 stage-0（resolved hash）且 y=' '、无关面逐字
/// 保持、MERGE_HEAD 在场（后验取 B 在 commit_merge 之前——merge 态仍在案）。
fn resolved_after(baseline: &WorktreeSnapshot) -> WorktreeSnapshot {
    let mut snapshot = baseline.clone();
    let conflicts: Vec<String> = snapshot
        .status
        .iter()
        .filter(|entry| entry.x == 'U')
        .map(|entry| entry.path.clone())
        .collect();
    snapshot.status = snapshot
        .status
        .iter()
        .map(|entry| {
            if entry.x == 'U' {
                status_entry('M', ' ', &entry.path)
            } else {
                entry.clone()
            }
        })
        .collect();
    snapshot.index.retain(|entry| entry.stage == 0);
    for path in &conflicts {
        snapshot.index.push(index_entry(path, "resolved", 0));
    }
    snapshot
}

/// 快照变体助手：指定路径的 porcelain y 码改写（逐规则破坏变体面）。
fn with_status_y(mut snapshot: WorktreeSnapshot, path: &str, y: char) -> WorktreeSnapshot {
    for entry in snapshot.status.iter_mut() {
        if entry.path == path {
            entry.y = y;
        }
    }
    snapshot
}

/// 快照变体助手：指定路径的 status / index 双面移除（删除缺席变体面）。
fn without_path(mut snapshot: WorktreeSnapshot, path: &str) -> WorktreeSnapshot {
    snapshot.status.retain(|entry| entry.path != path);
    snapshot.index.retain(|entry| entry.path != path);
    snapshot
}

/// lean 收敛族共同装置：冲突编程 + 各失败面注入 → 链停在 Merge 段。返回信封
/// 流与 vcs 捕获供逐形态断言（store / fs 随 env 出线——收敛面断言锚）。
struct LeanFixture {
    updates: Vec<ArchiveUpdate>,
    capture: VcsCapture,
    env: ChainEnv,
}

async fn run_lean(
    tag: &str,
    snapshots: Vec<Result<WorktreeSnapshot, String>>,
    outcomes: Vec<AgentRunStatus>,
    conflict_file_content: Option<&str>,
    abort_error: Option<&str>,
    commit_merge_error: Option<&str>,
) -> LeanFixture {
    let env = ChainEnv::worktree(tag);
    env.seed_main_change_dir();
    env.seed_all_pass();
    if let Some(content) = conflict_file_content {
        let dir = env.root_dir.path().join("src");
        std::fs::create_dir_all(&dir).expect("布置冲突目录失败");
        std::fs::write(dir.join("a.txt"), content).expect("布置冲突文件失败");
    }
    let mut vcs = FakeVcs::new().with_merge_conflicts(vec!["src/a.txt"]);
    if !snapshots.is_empty() {
        vcs = vcs.with_snapshots(snapshots);
    }
    if let Some(error) = abort_error {
        vcs = vcs.with_abort_merge_error(error);
    }
    if let Some(error) = commit_merge_error {
        vcs = vcs.with_commit_merge_error(error);
    }
    let (vcs, capture) = vcs.assemble();
    let (worker, _requests) = FakeWorker::new().with_outcomes(outcomes).assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    LeanFixture {
        updates,
        capture,
        env,
    }
}

/// lean 停链公共断言面：Merge failed（lean 串模板 + 原因词 + 冲突清单 + abort
/// 告知 + 手动 merge 引导 + 重试幂等说明）且 Finished error 同串；store 零翻转
/// / seal 零执行 / fs 零改名 / `abort_merge` 恰一次尽力执行。返回 lean 串供
/// 逐形态追加断言。
fn assert_lean_stopped(fixture: &LeanFixture, reason: &str) -> String {
    let stages = stage_states(&fixture.updates);
    let merge = terminal_of(&stages, ArchiveStage::Merge);
    assert_eq!(merge.status, ArchiveStageStatus::Failed, "停在 Merge 段");
    let detail = merge.detail.expect("lean 串在场");
    assert!(
        detail.contains("合入冲突无法自动裁决"),
        "lean 串模板在场: {detail}"
    );
    assert!(detail.contains(reason), "原因词在场: {detail}");
    assert!(
        detail.contains("src/a.txt"),
        "冲突文件逐行清单在场: {detail}"
    );
    assert!(
        detail.contains("已执行 git merge --abort 恢复主仓干净态"),
        "abort 告知在场: {detail}"
    );
    assert!(
        detail.contains(&format!("请手动将分支 change/{CHANGE} 合入主仓")),
        "手动裁决引导在场: {detail}"
    );
    assert!(
        detail.contains("重试将识别已合入并续走收口"),
        "重试幂等说明在场: {detail}"
    );
    let (_, error) = finished_of(&fixture.updates);
    assert_eq!(
        error.as_deref(),
        Some(detail.as_str()),
        "Finished error 同串"
    );
    assert_eq!(
        fixture.env.store.status(),
        ChangeStatus::Active,
        "store 零翻转"
    );
    assert_eq!(fixture.env.store.archived_calls(), 0, "零翻转调用");
    assert!(fixture.env.archived_dirs().is_empty(), "fs 零改名");
    assert!(
        fixture
            .capture
            .calls()
            .iter()
            .all(|call| call != "commit_paths"),
        "seal 前停链 → 零落盘提交"
    );
    assert_eq!(
        fixture
            .capture
            .calls()
            .iter()
            .filter(|call| *call == "abort_merge")
            .count(),
        1,
        "abort_merge 恰一次尽力执行"
    );
    detail
}

/// 零提交分支误判形态（死法 A 消亡，AC-1）：merged=true（基线可达 HEAD 的误判
/// 现场）+ 脏 worktree → 调用序首探 dirty 非 branch_merged、`commit_all` 照常
/// 恰一次（可达性不构成跳过分支——D2）；Merge 段已合入跳过（幂等维持）；
/// Seal passed = merge-first 守卫零触发；链续走收口。
#[tokio::test]
async fn 零提交分支误判_提交照常且链续走收口() {
    let env = ChainEnv::worktree("dead-branch-a");
    env.seed_main_change_dir();
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new().with_merged(true).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let calls = vcs_capture.calls();
    assert_eq!(
        calls.first().map(String::as_str),
        Some("dirty"),
        "调用序首探 dirty 非 branch_merged（可达性不构成跳过分支——D2）: {calls:?}"
    );
    assert_eq!(calls[1], "commit_all", "脏 worktree → 提交紧随脏探");
    assert_eq!(
        vcs_capture.commit_all_calls().len(),
        1,
        "提交照常恰一次（死法 A 消亡——AC-1）"
    );
    let stages = stage_states(&updates);
    let merge = terminal_of(&stages, ArchiveStage::Merge);
    assert_eq!(merge.status, ArchiveStageStatus::Skipped, "合入段幂等跳过");
    assert_eq!(merge.detail.as_deref(), Some("已合入"));
    assert_eq!(
        terminal_of(&stages, ArchiveStage::Seal).status,
        ArchiveStageStatus::Passed,
        "Seal 执行 = merge-first 守卫零触发（AC-1 字面）"
    );
    assert_eq!(env.store.status(), ChangeStatus::Archived, "链续走收口");
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    assert!(summary.is_some());
}

/// worktree 目录缺失且已合入（D2 归置）：record 携缺失路径 + merged=true + 主
/// 仓目录预置 → Preflight 通过（缺失+已合入不 Err）；Commit skipped detail
/// 「已合入」且 `commit_all` 零调用；`branch_merged` 恰 Preflight + Merge 各
/// 一次（Commit 段不设第二次查询）；链续走收口。
#[tokio::test]
async fn worktree目录缺失且已合入_提交段已合入归置续走收口() {
    let env = ChainEnv::worktree_absent("missing-merged");
    env.seed_main_change_dir();
    std::fs::create_dir_all(env.archive_root()).expect("预置 archive 树失败");
    env.seed_all_pass();
    let (vcs, vcs_capture) = FakeVcs::new().with_merged(true).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    assert_eq!(
        terminal_of(&stages, ArchiveStage::Preflight).status,
        ArchiveStageStatus::Passed,
        "缺失 + 已合入 → Preflight 通过（前置分支不 Err）"
    );
    let commit = terminal_of(&stages, ArchiveStage::Commit);
    assert_eq!(commit.status, ArchiveStageStatus::Skipped);
    assert_eq!(
        commit.detail.as_deref(),
        Some("已合入"),
        "目录缺失跳过复用「已合入」词汇（D2 归置）"
    );
    assert!(
        vcs_capture.commit_all_calls().is_empty(),
        "commit_all 零调用"
    );
    assert_eq!(
        vcs_capture
            .calls()
            .iter()
            .filter(|call| *call == "branch_merged")
            .count(),
        2,
        "branch_merged 恰 Preflight + Merge 各一次（不设第二次 merged 查询）"
    );
    assert_eq!(env.store.status(), ChangeStatus::Archived, "链续走收口");
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    assert!(summary.is_some());
}

/// merge 冲突 agent 解冲突续链（AC-6 编排半边）：Conflicted(["src/a.txt",
/// "src/b.txt"]) + 快照队列 [A, B] + 主仓冲突文件预置无标记 + worker Completed
/// → Merge 信封流 running（无 detail）→ running（解算中）→ passed（已解冲突
/// 2 文件）；worker 恰一次且 prompt / provenance / cwd / 档位全定式（D5）；
/// vcs 调用序：A 快照先于 agent、B 快照后于 agent（请求捕获序对拍）、
/// `commit_merge` 恰一次、`abort_merge` 零调用；链续走收口。
#[tokio::test]
async fn merge冲突_agent解冲突续链_收口与快照序齐备() {
    let env = ChainEnv::worktree("conflict-resolve");
    env.seed_main_change_dir();
    env.seed_all_pass();
    // 主仓冲突文件预置（无标记——residual_markers 扫描的真实 tempdir 面）
    let src = env.root_dir.path().join("src");
    std::fs::create_dir_all(&src).expect("布置 src 目录失败");
    std::fs::write(src.join("a.txt"), "裁决结果 a\n").expect("布置冲突文件失败");
    std::fs::write(src.join("b.txt"), "裁决结果 b\n").expect("布置冲突文件失败");
    let conflicts = vec!["src/a.txt".to_owned(), "src/b.txt".to_owned()];
    let baseline = conflict_baseline();
    let (vcs, vcs_capture) = FakeVcs::new()
        .with_merge_conflicts(vec!["src/a.txt", "src/b.txt"])
        .with_snapshots(vec![Ok(baseline.clone()), Ok(resolved_after(&baseline))])
        .assemble();
    let (worker, requests) = FakeWorker::new()
        .with_call_log(&vcs_capture.calls_handle())
        .assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    // Merge 信封流：running（无 detail）→ running（detail 解算中——D7 同段后
    // 写覆盖）→ passed（detail 已解冲突 2 文件）
    let stages = stage_states(&updates);
    let merge_rows: Vec<&ArchiveStageState> = stages
        .iter()
        .filter(|state| state.stage == ArchiveStage::Merge)
        .collect();
    assert_eq!(merge_rows.len(), 3, "恰三信封（双 running + 终态）");
    assert_eq!(merge_rows[0].status, ArchiveStageStatus::Running);
    assert_eq!(merge_rows[0].detail, None, "首 running 无 detail");
    assert_eq!(merge_rows[1].status, ArchiveStageStatus::Running);
    assert_eq!(
        merge_rows[1].detail.as_deref(),
        Some("合入冲突，解冲突 agent 裁决中"),
        "同段后写覆盖（D7）"
    );
    assert_eq!(merge_rows[2].status, ArchiveStageStatus::Passed);
    assert_eq!(
        merge_rows[2].detail.as_deref(),
        Some("已解冲突 2 文件"),
        "链代收口后的通过摘要"
    );

    // worker 恰一次：prompt 逐字单点模板 + 会话定式（D5：cwd = 主 root、
    // bypassPermissions、High、Executor、新会话）
    let requests = requests.lock().expect("请求捕获锁不可中毒");
    assert_eq!(requests.len(), 1, "恰一次解冲突会话");
    let turn = &requests[0];
    assert_eq!(
        turn.prompt,
        merge_conflict_prompt(CHANGE, &conflicts),
        "prompt = merge_conflict_prompt 单点模板逐字"
    );
    assert_eq!(
        turn.provenance,
        SessionProvenance {
            source: "change".to_owned(),
            source_ref: Some(format!("{CHANGE}/archive/merge-conflict")),
        },
        "provenance 定式（归档语义段 merge-conflict）"
    );
    assert_eq!(turn.root, env.root(), "cwd = 主 workspace root（D5）");
    assert_eq!(turn.permission, AgentPermissionMode::BypassPermissions);
    assert_eq!(turn.model_level, ModelLevel::High);
    assert_eq!(turn.role, WorkerRole::Executor);
    assert_eq!(turn.agent, None);
    assert_eq!(turn.continue_session, None, "新会话");
    drop(requests);

    // vcs 调用序（请求捕获序对拍：A 快照 → agent → B 快照 → 代收口）
    assert_eq!(
        vcs_capture.calls(),
        vec![
            "dirty".to_owned(),
            "commit_all".to_owned(),
            "branch_merged".to_owned(),
            "merge_branch".to_owned(),
            "worktree_snapshot".to_owned(),
            "worker_run".to_owned(),
            "worktree_snapshot".to_owned(),
            "commit_merge".to_owned(),
            "dirty".to_owned(),
            "commit_paths".to_owned(),
        ],
        "A 快照先于 agent、B 快照后于 agent、commit_merge 恰一次、abort_merge 零调用"
    );

    // 链续走 SpecSync（无 delta specs → skipped）→ Seal → Finalize 收口
    for stage in [
        ArchiveStage::SpecSync,
        ArchiveStage::Seal,
        ArchiveStage::Finalize,
    ] {
        assert_ne!(
            terminal_of(&stages, stage).status,
            ArchiveStageStatus::Running,
            "{stage:?} 段有终态（链续走）"
        );
    }
    assert!(
        !env.main_change_dir().exists() && env.archived_dirs().len() == 1,
        "seal 落盘（主仓 active 树改名进 archive 树）"
    );
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none(), "Finished 成功（AC-6 编排半边）");
    assert!(summary.is_some());
}

/// lean 收敛族（D7/D10 参数化九形态）：各失败面 → Merge failed detail = lean
/// 串且 Finished error 同串、`abort_merge` 恰一次尽力执行、store 零翻转 /
/// seal 零执行 / fs 零改名。九形态覆盖六原因词 + 被停止注记特例 + abort 二次
/// 失败附注 + 收口提交失败的人工收口引导（D6——不走 lean abort）。
#[tokio::test]
async fn lean收敛族_九形态参数化停链() {
    // ① worker outcome Failed → 原因词「agent 会话失败」
    let fixture = run_lean(
        "lean-1-failed",
        vec![Ok(conflict_baseline())],
        vec![AgentRunStatus::Failed],
        None,
        None,
        None,
    )
    .await;
    assert_lean_stopped(&fixture, "agent 会话失败");

    // ② worker outcome Stopped → 「agent 会话被停止」+ 注记「（归档链已停止）」
    // ——error ≠ 裸「归档链已停止」（D7 特例：TXT_STOPPED 保留给非冲突段）
    let fixture = run_lean(
        "lean-2-stopped",
        vec![Ok(conflict_baseline())],
        vec![AgentRunStatus::Stopped],
        None,
        None,
        None,
    )
    .await;
    let detail = assert_lean_stopped(&fixture, "agent 会话被停止");
    assert!(
        detail.contains("（归档链已停止）"),
        "被停止注记附于 lean 串尾: {detail}"
    );
    let (_, error) = finished_of(&fixture.updates);
    assert_ne!(
        error.as_deref(),
        Some("归档链已停止"),
        "error ≠ 裸停止词（TXT_STOPPED 保留面——D7）"
    );

    // ③ 主仓冲突文件行首标记在场 → 「残留冲突未解」（residual_markers 真实
    // tempdir fs 扫描）
    let fixture = run_lean(
        "lean-3-residual",
        vec![
            Ok(conflict_baseline()),
            Ok(resolved_after(&conflict_baseline())),
        ],
        vec![],
        Some("<<<<<<< HEAD\n主仓版本\n=======\n分支版本\n>>>>>>> change/archive-flow\n"),
        None,
        None,
    )
    .await;
    assert_lean_stopped(&fixture, "残留冲突未解");

    // ④ B 快照非冲突路径 porcelain 漂移（新 untracked 路径）→ 「清单外新改动」
    let mut drifted = resolved_after(&conflict_baseline());
    drifted.status.push(status_entry('?', '?', "rogue.txt"));
    let fixture = run_lean(
        "lean-4-outside",
        vec![Ok(conflict_baseline()), Ok(drifted)],
        vec![],
        Some("裁决结果\n"),
        None,
        None,
    )
    .await;
    assert_lean_stopped(&fixture, "清单外新改动");

    // ⑤ B.merge_head=None → 「agent 违约自行收口 merge」+ A.head 与
    // `git reset --hard` 引导（链不自动改写历史）
    let mut closed = resolved_after(&conflict_baseline());
    closed.merge_head = None;
    let fixture = run_lean(
        "lean-5-closed",
        vec![Ok(conflict_baseline()), Ok(closed)],
        vec![],
        Some("裁决结果\n"),
        None,
        None,
    )
    .await;
    let detail = assert_lean_stopped(&fixture, "agent 违约自行收口 merge");
    assert!(
        detail.contains(SNAP_HEAD) && detail.contains("git reset --hard"),
        "串附 A.head 与 reset --hard 引导: {detail}"
    );

    // ⑥ B 快照 Err → 「后验探测失败」
    let fixture = run_lean(
        "lean-6-b-err",
        vec![Ok(conflict_baseline()), Err("注入 B 探测失败".to_owned())],
        vec![],
        Some("裁决结果\n"),
        None,
        None,
    )
    .await;
    assert_lean_stopped(&fixture, "后验探测失败");

    // ⑦ A 快照 Err → lean（后验探测失败——后验无从对比的单一收敛序）
    let fixture = run_lean(
        "lean-7-a-err",
        vec![Err("注入 A 探测失败".to_owned())],
        vec![],
        None,
        None,
        None,
    )
    .await;
    assert_lean_stopped(&fixture, "后验探测失败");

    // ⑧ abort_merge 注入 Err → lean 串附「abort 未成功…」且原原因词不被掩盖
    //（D10 不静默吞二次失败）
    let fixture = run_lean(
        "lean-8-abort-err",
        vec![Ok(conflict_baseline())],
        vec![AgentRunStatus::Failed],
        None,
        Some("注入 abort 失败"),
        None,
    )
    .await;
    let detail = assert_lean_stopped(&fixture, "agent 会话失败");
    assert!(
        detail.contains("abort 未成功，请手动核验主仓 git 状态"),
        "abort 二次失败附注在场: {detail}"
    );

    // ⑨ commit_merge 注入 Err → 收口提交失败停链（D6：冲突解已验通过不走
    // lean abort——不丢弃解算成果），串附主仓 merge 态人工收口引导（
    // `git commit --no-edit` / `git merge --abort`）与重试前半截态须人工清
    let fixture = run_lean(
        "lean-9-commit-err",
        vec![
            Ok(conflict_baseline()),
            Ok(resolved_after(&conflict_baseline())),
        ],
        vec![],
        Some("裁决结果\n"),
        None,
        Some("git commit 失败（退出码 1）: 注入收口失败"),
    )
    .await;
    let stages = stage_states(&fixture.updates);
    let merge = terminal_of(&stages, ArchiveStage::Merge);
    assert_eq!(merge.status, ArchiveStageStatus::Failed, "停在 Merge 段");
    let detail = merge.detail.expect("失败记因在场");
    assert!(
        detail.contains("git commit --no-edit") && detail.contains("git merge --abort"),
        "人工收口 / 回退二选一引导在场: {detail}"
    );
    assert!(
        detail.contains("重试前半截态须人工清"),
        "半截态人工清理说明在场: {detail}"
    );
    assert!(
        detail.contains("注入收口失败"),
        "port Err 语境零加工随串: {detail}"
    );
    let (_, error) = finished_of(&fixture.updates);
    assert_eq!(
        error.as_deref(),
        Some(detail.as_str()),
        "Finished error 同串"
    );
    assert!(
        fixture
            .capture
            .calls()
            .iter()
            .all(|call| call != "abort_merge"),
        "收口失败不走 lean abort（D6 不丢弃已验解算成果）"
    );
    assert_eq!(
        fixture.env.store.status(),
        ChangeStatus::Active,
        "store 零翻转"
    );
    assert!(fixture.env.archived_dirs().is_empty(), "fs 零改名");
}

/// lean 后重试幂等续走（AC-7 尾半边）：lean 停链（形态①）后换新假件
/// merged=true + 主仓目录预置 → Commit skipped（干净）/ Merge skipped（已合
/// 入）→ 链续走 SpecSync→收口；`commit_all` 零调用（已成功段零重复）。
#[tokio::test]
async fn lean停链后重试_手动合入幂等续走() {
    let fixture = run_lean(
        "lean-retry",
        vec![Ok(conflict_baseline())],
        vec![AgentRunStatus::Failed],
        None,
        None,
        None,
    )
    .await;
    assert_lean_stopped(&fixture, "agent 会话失败");
    let env = fixture.env;

    // 重试：用户手动解冲突合入（merged=true + 主仓目录已在场——共同装置预置）
    let (vcs, retry_capture) = FakeVcs::new()
        .with_merged(true)
        .with_dirty(vec![false])
        .assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    let stages = stage_states(&updates);
    let commit = terminal_of(&stages, ArchiveStage::Commit);
    assert_eq!(commit.status, ArchiveStageStatus::Skipped, "提交段跳过");
    assert_eq!(commit.detail.as_deref(), Some("干净"));
    let merge = terminal_of(&stages, ArchiveStage::Merge);
    assert_eq!(merge.status, ArchiveStageStatus::Skipped, "合入段跳过");
    assert_eq!(merge.detail.as_deref(), Some("已合入"));
    assert!(
        retry_capture.commit_all_calls().is_empty(),
        "已成功段零重复调用（commit_all 零调用——AC-7 尾半边）"
    );
    assert_eq!(env.store.status(), ChangeStatus::Archived, "重试续走收口");
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    assert!(summary.is_some());
}

/// SpecSync 段位与探测源（D8 / AC-3 结构面）：delta specs 仅布置于主仓 active
/// 树（worktree 树零 specs——旧形态「worktree 内探测」的否定锚）→ Merge 终态
/// 信封先于 SpecSync running 信封（段位后移锚）、worker 恰一次 spec-sync 且
/// cwd = 主 workspace root。
#[tokio::test]
async fn specsync段位后移_探测源恒主仓且cwd主root() {
    let env = ChainEnv::worktree("sync-position");
    env.seed_main_change_dir();
    let specs_dir = env.main_change_dir().join("specs/cap-only-main");
    std::fs::create_dir_all(&specs_dir).expect("布置主仓 delta specs 失败");
    std::fs::write(specs_dir.join("spec.md"), "## ADDED Requirements\n")
        .expect("布置 spec.md 失败");
    assert!(
        !env.worktree_change_dir().join("specs").exists(),
        "worktree 树零 specs 布置（死法 B 否定锚前置）"
    );
    env.seed_main_specs(&["cap-only-main"]);
    env.seed_all_pass();
    let (vcs, _capture) = FakeVcs::new().assemble();
    let (worker, requests) = FakeWorker::new().assemble();

    let updates = run_chain(&env, worker, vcs, env.request(true)).await;

    // 段位后移锚：Merge 终态信封先于 SpecSync running 信封（D8）
    let stages = stage_states(&updates);
    let merge_passed = stages
        .iter()
        .position(|state| {
            state.stage == ArchiveStage::Merge && state.status == ArchiveStageStatus::Passed
        })
        .expect("Merge passed 信封在场");
    let sync_running = stages
        .iter()
        .position(|state| {
            state.stage == ArchiveStage::SpecSync && state.status == ArchiveStageStatus::Running
        })
        .expect("SpecSync running 信封在场");
    assert!(
        merge_passed < sync_running,
        "Merge 终态信封先于 SpecSync running 信封（段位后移——D8）"
    );
    // 探测源恒主仓：worktree 零 specs 布置下仍恰一次 spec-sync 会话，cwd 主 root
    let requests = requests.lock().expect("请求捕获锁不可中毒");
    assert_eq!(requests.len(), 1, "探测源主仓命中 → 恰一次 spec-sync 会话");
    assert_eq!(
        requests[0].root,
        env.root(),
        "cwd = 主 workspace root（D8——同步基线恒主仓当前 specs）"
    );
}

/// Finalize 扩围（D9）：①（delta 在场 + 同步执行 → 三拼 pathspec）由全链正向
/// 行承载；本行补 ② delta 在场 + 用户跳过（sync_specs=false）→ 同样扩围（跳过
/// 不收缩 pathspec 集）；③ delta 缺席 → 归档两路径既有形态。落盘提交信息维持
/// `archive: move <name> to archive`。
#[tokio::test]
async fn finalize扩围_用户跳过同扩围且delta缺席两pathspec() {
    let domain = foundation::layout::domain_dir_name();

    // 形态②：delta 在场 + 用户跳过 → pathspec 集同样扩围
    let env = ChainEnv::worktree("fin-skip");
    env.seed_main_change_dir();
    let specs_dir = env.main_change_dir().join("specs/cap-skip");
    std::fs::create_dir_all(&specs_dir).expect("布置主仓 delta specs 失败");
    std::fs::write(specs_dir.join("spec.md"), "## ADDED Requirements\n")
        .expect("布置 spec.md 失败");
    env.seed_main_specs(&["cap-skip"]);
    env.seed_all_pass();
    let (vcs, capture) = FakeVcs::new().assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(false)).await;
    let stages = stage_states(&updates);
    assert_eq!(
        terminal_of(&stages, ArchiveStage::SpecSync)
            .detail
            .as_deref(),
        Some("用户选择"),
        "同步段用户跳过"
    );
    let commit_paths = capture.commit_paths_calls();
    assert_eq!(commit_paths.len(), 1, "跳过同步仍执行落盘段");
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    let archived_dir = summary.expect("收口").archived_dir;
    assert_eq!(
        commit_paths[0].0,
        vec![
            format!("{domain}/changes/{CHANGE}"),
            format!("{domain}/changes/archive/{archived_dir}"),
            format!("{domain}/specs/cap-skip"),
        ],
        "用户跳过不收缩 pathspec 集（D9——子树通常干净，语义无害）"
    );
    assert_eq!(
        commit_paths[0].1,
        format!("archive: move {CHANGE} to archive"),
        "落盘提交信息维持"
    );

    // 形态③：delta 缺席 → 归档两路径既有形态
    let env = ChainEnv::worktree("fin-no-delta");
    env.seed_main_change_dir();
    env.seed_all_pass();
    let (vcs, capture) = FakeVcs::new().with_dirty(vec![true, true]).assemble();
    let (worker, _requests) = FakeWorker::new().assemble();
    let updates = run_chain(&env, worker, vcs, env.request(true)).await;
    let commit_paths = capture.commit_paths_calls();
    assert_eq!(commit_paths.len(), 1);
    let (summary, error) = finished_of(&updates);
    assert!(error.is_none());
    let archived_dir = summary.expect("收口").archived_dir;
    assert_eq!(
        commit_paths[0].0,
        vec![
            format!("{domain}/changes/{CHANGE}"),
            format!("{domain}/changes/archive/{archived_dir}"),
        ],
        "delta 缺席 → 两 pathspec 既有形态"
    );
    assert_eq!(
        commit_paths[0].1,
        format!("archive: move {CHANGE} to archive"),
        "落盘提交信息维持"
    );
}

/// verify_resolution 纯函数分支穷尽（D4）：合成快照直驱零假件——Ok 基线 +
/// ① 违约收口 ② 残留三方条目 ③ worktree 脏 ③' 删除缺席 ④ porcelain 漂移 /
/// 新增路径 / 消失路径越界 / 索引 mode-hash-stage 漂移各变体。
#[test]
fn verify_resolution纯函数_分支穷尽与违约收口串() {
    let baseline = conflict_baseline();
    let conflicts: Vec<String> = vec!["src/a.txt".to_owned(), "src/b.txt".to_owned()];
    let resolved = resolved_after(&baseline);

    // Ok 基线：冲突路径单条 stage-0 + y=' '、非冲突路径 A/B 逐字一致
    assert_eq!(
        verify_resolution(&baseline, &resolved, &conflicts),
        Ok(()),
        "通过形态 → Ok"
    );

    // ① after.merge_head 缺席 → Err 违约收口（串附 A.head 与 reset --hard 引导）
    let mut closed = resolved.clone();
    closed.merge_head = None;
    let error =
        verify_resolution(&baseline, &closed, &conflicts).expect_err("MERGE_HEAD 缺席应 Err");
    assert!(
        error.contains("agent 违约自行收口 merge")
            && error.contains(SNAP_HEAD)
            && error.contains("git reset --hard"),
        "违约收口语境串: {error}"
    );

    // ② 冲突路径索引残留 stage>0 条目 → Err 残留冲突
    let mut residual = resolved.clone();
    residual.index.push(index_entry("src/a.txt", "ours-a", 2));
    assert_eq!(
        verify_resolution(&baseline, &residual, &conflicts),
        Err("残留冲突未解".to_owned()),
        "三方残留 → Err 残留冲突"
    );

    // ③ 冲突路径 y≠' '（worktree 脏）→ Err 残留冲突
    let dirty = with_status_y(resolved.clone(), "src/a.txt", 'M');
    assert_eq!(
        verify_resolution(&baseline, &dirty, &conflicts),
        Err("残留冲突未解".to_owned()),
        "冲突路径 worktree 脏 → Err 残留冲突"
    );

    // ③' 冲突路径删除缺席（status 与 index 双缺席；清单内 b.txt 本就以缺席面
    // 在场）→ Ok（解算收敛至与 HEAD 一致而离开 status 是合法消失）
    let deleted = without_path(resolved.clone(), "src/a.txt");
    assert_eq!(
        verify_resolution(&baseline, &deleted, &conflicts),
        Ok(()),
        "冲突路径删除缺席 → Ok"
    );

    // ④ 非冲突路径 porcelain 漂移 → Err 清单外新改动
    let drifted = with_status_y(resolved.clone(), UNRELATED_PATH, 'M');
    assert!(
        verify_resolution(&baseline, &drifted, &conflicts)
            .expect_err("porcelain 漂移应 Err")
            .starts_with("清单外新改动"),
        "非冲突路径漂移 → Err 清单外新改动"
    );

    // ④ B 新增路径（agent 新建 / 新 add）→ Err 清单外新改动（porcelain 集合
    // 漂移先行命中泛型串）
    let mut added = resolved.clone();
    added.status.push(status_entry('?', '?', "rogue.txt"));
    assert!(
        verify_resolution(&baseline, &added, &conflicts)
            .expect_err("新增路径应 Err")
            .starts_with("清单外新改动"),
        "新增路径 → Err 清单外新改动"
    );

    // ④ 消失路径 ⊄ 冲突清单 → Err 清单外新改动（无关 staged 条目离场）
    let vanished = without_path(resolved.clone(), UNRELATED_PATH);
    assert!(
        verify_resolution(&baseline, &vanished, &conflicts)
            .expect_err("无关路径离场应 Err")
            .starts_with("清单外新改动"),
        "消失路径越界 → Err 清单外新改动"
    );

    // ④ 索引条目 hash / mode / stage 任一漂移 → 各自 Err 清单外新改动
    let mut hash_drift = resolved.clone();
    for entry in hash_drift.index.iter_mut() {
        if entry.path == UNRELATED_PATH {
            entry.hash = "drifted-hash".to_owned();
        }
    }
    assert!(
        verify_resolution(&baseline, &hash_drift, &conflicts)
            .expect_err("索引 hash 漂移应 Err")
            .starts_with("清单外新改动"),
        "索引 hash 漂移 → Err 清单外新改动"
    );
    let mut mode_drift = resolved.clone();
    for entry in mode_drift.index.iter_mut() {
        if entry.path == UNRELATED_PATH {
            entry.mode = "100755".to_owned();
        }
    }
    assert!(
        verify_resolution(&baseline, &mode_drift, &conflicts)
            .expect_err("索引 mode 漂移应 Err")
            .starts_with("清单外新改动"),
        "索引 mode 漂移 → Err 清单外新改动"
    );
    let mut stage_drift = resolved.clone();
    for entry in stage_drift.index.iter_mut() {
        if entry.path == UNRELATED_PATH {
            entry.stage = 1;
        }
    }
    assert!(
        verify_resolution(&baseline, &stage_drift, &conflicts)
            .expect_err("索引 stage 漂移应 Err")
            .starts_with("清单外新改动"),
        "索引 stage 漂移 → Err 清单外新改动"
    );
}

/// residual_markers 扫描（D4）：行首 `<<<<<<< ` / `>>>>>>> ` 逐行命中（`文件:行`
/// 形态、多文件聚合）；缩进（非行首）形态不命中；文件缺席 / 读取失败按无标记
/// 处理（容错面）。
#[test]
fn residual_markers扫描_行首命中与容错() {
    let root = tempfile::Builder::new()
        .prefix("archive-flow-markers-")
        .tempdir()
        .expect("创建标记临时目录失败");
    std::fs::write(
        root.path().join("a.txt"),
        "第一行\n<<<<<<< HEAD\n中间\n>>>>>>> branch\n尾行\n",
    )
    .expect("布置标记文件失败");
    std::fs::write(root.path().join("b.txt"), "  <<<<<<< 缩进不命中\n正文\n")
        .expect("布置缩进文件失败");
    std::fs::create_dir_all(root.path().join("dir.txt")).expect("布置读取失败形态失败");

    let hits = residual_markers(
        root.path(),
        &["a.txt".to_owned(), "b.txt".to_owned(), "dir.txt".to_owned()],
    );
    assert_eq!(
        hits,
        vec!["a.txt:2".to_owned(), "a.txt:4".to_owned()],
        "行首标记逐行命中（1 基行号、多文件聚合）；缩进与读取失败形态不命中"
    );

    // 文件缺席 → 空（无标记处理）
    assert!(
        residual_markers(root.path(), &["absent.txt".to_owned()]).is_empty(),
        "文件缺席按无标记处理（D4 容错面）"
    );
}

/// merge_conflict_prompt 语义锚（AC-6 prompt MUST 面文本锚）：change 名插值、
/// 冲突清单逐行在场、「只编辑清单内 / 清单外一律不动（含未提交 / staged 无关
/// 内容）」逐字、逐文件 `git add` 指令、禁令逐字（`git add -A` / `git add .` /
/// commit / merge / rebase / reset / stash / MCP 工具 / `__TOOL_ASK_USER__`）。
#[test]
fn merge_conflict_prompt语义锚_清单插值与禁令逐字() {
    let prompt = merge_conflict_prompt(
        "demo-change",
        &["src/a.txt".to_owned(), "src/b.txt".to_owned()],
    );

    assert!(prompt.contains("demo-change"), "change 名插值在场");
    assert!(
        prompt.contains("src/a.txt") && prompt.contains("src/b.txt"),
        "冲突清单逐行在场"
    );
    assert!(
        prompt.contains("只编辑上面清单内的文件"),
        "只编辑清单内文件逐字在场"
    );
    assert!(
        prompt.contains("清单外任何文件一律不动")
            && prompt.contains("含未提交 / staged 的无关内容"),
        "清单外一律不动（含无关内容归属）逐字在场"
    );
    assert!(
        prompt.contains("git add <该文件路径>"),
        "逐文件精确 add 指令在场"
    );
    for banned in [
        "git add -A",
        "git add .",
        "git commit",
        "git merge",
        "git rebase",
        "git reset",
        "git stash",
    ] {
        assert!(
            prompt.contains(&format!("`{banned}`")),
            "禁令逐字在场（反引号包命令）: {banned}"
        );
    }
    assert!(prompt.contains("禁止调用 MCP 工具"), "禁 MCP 工具禁令在场");
    assert!(prompt.contains("__TOOL_ASK_USER__"), "禁 ask 占位禁令在场");
}
