use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use foundation::layout::resolve;
use time::OffsetDateTime;

use super::create;
use super::BootstrapProject;
use super::CreateOutcome;
use super::{InstallRun, RepoProbe, WorktreePlacement, WorktreePort};
use crate::queries::list_changes;
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStateRecord, RunEdgeStateRecord, RunFinishCommand, RunNodeStateRecord, RunStartCommand,
    RunStateRecord, StepCommand, StepStateRecord, StoreFault,
};

// ---------------------------------------------------------------------------
// 装置：真实 tempdir 双根（主仓根 + worktree 落位父锚）+ 进程内假件
// store / vcs
// ---------------------------------------------------------------------------

/// 项目锚点简写（无依赖声明）。
fn proj(root: &str) -> BootstrapProject {
    BootstrapProject {
        root: root.to_owned(),
        depends_on: Vec::new(),
    }
}

struct Env {
    root: PathBuf,
    worktree_root: PathBuf,
    store: CreateStore,
    vcs: FakeVcs,
}

impl Env {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "workflow-create-test-{}-{}",
            std::process::id(),
            tag
        ));
        let worktree_root = std::env::temp_dir().join(format!(
            "workflow-create-worktrees-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&worktree_root);
        Self {
            root,
            vcs: FakeVcs::new(worktree_root.clone()),
            worktree_root,
            store: CreateStore::new(),
        }
    }

    fn create(&self, name: &str, goal: &str) -> Result<CreateOutcome, String> {
        self.create_full(name, goal, name, "requirement")
    }

    /// 全参形态（显式 title + workflow_type 载荷断言面）。
    fn create_full(
        &self,
        name: &str,
        goal: &str,
        title: &str,
        workflow_type: &str,
    ) -> Result<CreateOutcome, String> {
        create(
            &self.root,
            &self.store,
            &self.vcs,
            &[],
            name,
            goal,
            title,
            workflow_type,
        )
    }

    /// 显式 title 入参形态（title 载荷断言面；blank title 回退用例传空串 / 空白）。
    fn create_with_title(
        &self,
        name: &str,
        goal: &str,
        title: &str,
    ) -> Result<CreateOutcome, String> {
        self.create_full(name, goal, title, "requirement")
    }

    /// 依赖引导项目根形态（roots 由命令层从配置解析后注入）。
    fn create_with_projects(
        &self,
        name: &str,
        goal: &str,
        projects: &[BootstrapProject],
    ) -> Result<CreateOutcome, String> {
        create(
            &self.root,
            &self.store,
            &self.vcs,
            projects,
            name,
            goal,
            name,
            "requirement",
        )
    }

    /// worktree 内 change 目录（目录树 / explore.md 的落点）。
    fn change_dir(&self, name: &str) -> PathBuf {
        resolve(&self.worktree_root.join(name))
            .changes_root
            .join(name)
    }

    fn explore_bytes(&self, name: &str) -> Vec<u8> {
        fs::read(self.change_dir(name).join("explore.md")).expect("读 explore.md 失败")
    }

    /// 主仓 changes_root 下现存目录名（根不存在即空集）——「主仓 active 树零
    /// 目录」观察面。
    fn active_dir_names(&self) -> Vec<String> {
        dir_names(&resolve(&self.root).changes_root)
    }

    /// worktree 全树下文件名集合（递归）——「零 workflow.json 产出」观察面。
    fn worktree_tree_file_names(&self, name: &str) -> Vec<String> {
        let mut names = Vec::new();
        collect_file_names(&self.worktree_root.join(name), &mut names);
        names
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir_all(&self.worktree_root);
    }
}

/// 主仓根下预置文件（自动建根目录——lockfile / 占位夹具共用）。
fn seed_main_file(env: &Env, name: &str, content: &str) {
    fs::create_dir_all(&env.root).expect("创建主仓根失败");
    fs::write(env.root.join(name), content).expect("预置主仓文件失败");
}

/// 主仓根下按相对路径预置文件（自动建父目录——项目根 lockfile 夹具）。
fn seed_main_path(env: &Env, relative: &str, content: &str) {
    let path = env.root.join(relative);
    fs::create_dir_all(path.parent().expect("预置路径应有父目录")).expect("创建预置父目录失败");
    fs::write(path, content).expect("预置主仓文件失败");
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

fn collect_file_names(root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_file_names(&entry.path(), out);
        } else {
            out.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
}

/// 递归复制目录内容（假件 add_worktree 的「HEAD 检出」最小模拟——主仓树
/// 内容镜像进 worktree）。
fn mirror_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("假件建 worktree 失败: {error}"))?;
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(()); // 空主仓 → 空 worktree 检出
    };
    for entry in entries.flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            mirror_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)
                .map(|_| ())
                .map_err(|error| format!("假件镜像检出失败: {error}"))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 假件 vcs（WorktreePort 脚本化）：probe / branch_exists / 各方法 Err 注入
// + 调用与参数捕获；add_worktree 成功即真实建目录并镜像主仓树。
// ---------------------------------------------------------------------------

/// 假件缺省探测产出（干净仓、HEAD 恒定）。
const FAKE_HEAD: &str = "0000000000000000000000000000000000000001";

#[derive(Default)]
struct VcsProgram {
    probe: Option<Result<RepoProbe, String>>,
    branch_exists: bool,
    add_fault: Option<String>,
    remove_fault: Option<String>,
    delete_branch_fault: Option<String>,
    install_result: Option<Result<InstallRun, String>>,
    /// placement 的偏移段（缺省 `""` = workspace root 即 git 根）
    rel: Option<String>,
    /// placement 派生失败注入（非 git 仓 / 裸仓 / 空输出三档引导文案）
    placement_fault: Option<String>,
    /// `HEAD:<rel>` 在案判据（缺省 `None` = 在案 true；判据自身文案供注入）
    tracked_at_head: Option<Result<bool, String>>,
    /// 检出「不镜像偏移子树」形态：镜像后删除 `rel` 子树（后置 `is_dir` 断言
    /// 与补偿链入参的注入面）
    drop_exec_root: bool,
}

struct FakeVcs {
    /// worktree 落位父锚（create 经 port 派生；夹具根由 Env 注入）。
    worktree_root: PathBuf,
    program: Mutex<VcsProgram>,
    calls: Mutex<Vec<String>>,
    adds: Mutex<Vec<(PathBuf, PathBuf, String)>>,
    installs: Mutex<Vec<(PathBuf, String)>>,
    /// `tracked_at_head` 调用面（每次调用的 `rel` 实参；根 workspace 零调用
    /// 的判据）
    tracked_calls: Mutex<Vec<String>>,
    /// `remove_worktree` 实参捕获（补偿链锚点 = host 的判据面）
    removes: Mutex<Vec<PathBuf>>,
}

impl FakeVcs {
    fn new(worktree_root: PathBuf) -> Self {
        Self {
            worktree_root,
            program: Mutex::new(VcsProgram::default()),
            calls: Mutex::new(Vec::new()),
            adds: Mutex::new(Vec::new()),
            installs: Mutex::new(Vec::new()),
            tracked_calls: Mutex::new(Vec::new()),
            removes: Mutex::new(Vec::new()),
        }
    }

    /// 子目录 workspace 形态：placement 给出非空偏移（`/` 分隔、无尾分隔符）。
    /// 三档 setter（偏移 / 不在案 / 询问失败）供前置第十道与后置断言的六形态
    /// 用例注入。
    fn set_rel(&self, rel: &str) {
        self.program.lock().expect("程序锁不可中毒").rel = Some(rel.to_owned());
    }

    /// placement 派生失败（非 git 仓 / 裸仓 / 空输出三档引导文案注入）。
    fn set_placement_error(&self, error: &str) {
        self.program.lock().expect("程序锁不可中毒").placement_fault = Some(error.to_owned());
    }

    /// 检出「不镜像偏移子树」形态：镜像后删掉执行根（后置断言 / 补偿链锚点
    /// 的注入面）。
    fn set_drop_exec_root(&self) {
        self.program.lock().expect("程序锁不可中毒").drop_exec_root = true;
    }

    /// `HEAD:<rel>` 不在案（前置第十道的拒绝面）。
    fn set_untracked(&self) {
        self.program.lock().expect("程序锁不可中毒").tracked_at_head = Some(Ok(false));
    }

    /// `HEAD:<rel>` 询问失败（fail-closed 面：与「不在案」可辨）。
    fn set_tracked_error(&self, error: &str) {
        self.program.lock().expect("程序锁不可中毒").tracked_at_head = Some(Err(error.to_owned()));
    }

    /// `tracked_at_head` 调用面（每次调用的 `rel` 实参）——准入判据的「根
    /// workspace 零调用」与六形态断言读取本面。
    fn tracked_calls(&self) -> Vec<String> {
        self.tracked_calls
            .lock()
            .expect("判据调用锁不可中毒")
            .clone()
    }

    /// `remove_worktree` 实参捕获（补偿链锚点 = host 的判据面）。
    fn removes(&self) -> Vec<PathBuf> {
        self.removes.lock().expect("回收锁不可中毒").clone()
    }

    fn record(&self, call: &str) {
        self.calls
            .lock()
            .expect("vcs 调用锁不可中毒")
            .push(call.to_owned());
    }

    fn set_probe_error(&self, error: &str) {
        self.program.lock().expect("程序锁不可中毒").probe = Some(Err(error.to_owned()));
    }

    fn set_dirty(&self) {
        self.program.lock().expect("程序锁不可中毒").probe = Some(Ok(RepoProbe {
            head: FAKE_HEAD.to_owned(),
            dirty: true,
        }));
    }

    fn set_branch_exists(&self) {
        self.program.lock().expect("程序锁不可中毒").branch_exists = true;
    }

    fn set_add_fault(&self, error: &str) {
        self.program.lock().expect("程序锁不可中毒").add_fault = Some(error.to_owned());
    }

    fn set_remove_fault(&self, error: &str) {
        self.program.lock().expect("程序锁不可中毒").remove_fault = Some(error.to_owned());
    }

    fn set_delete_branch_fault(&self, error: &str) {
        self.program
            .lock()
            .expect("程序锁不可中毒")
            .delete_branch_fault = Some(error.to_owned());
    }

    fn set_install_failure(&self, summary: &str) {
        self.program.lock().expect("程序锁不可中毒").install_result = Some(Ok(InstallRun {
            success: false,
            summary: summary.to_owned(),
        }));
    }

    fn set_install_error(&self, error: &str) {
        self.program.lock().expect("程序锁不可中毒").install_result = Some(Err(error.to_owned()));
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("vcs 调用锁不可中毒").clone()
    }

    fn install_commands(&self) -> Vec<String> {
        self.installs
            .lock()
            .expect("安装锁不可中毒")
            .iter()
            .map(|(_, command)| command.clone())
            .collect()
    }

    /// 安装调用全量捕获（cwd, command）——projects 模式断言安装目录。
    fn installs(&self) -> Vec<(PathBuf, String)> {
        self.installs.lock().expect("安装锁不可中毒").clone()
    }
}

impl WorktreePort for FakeVcs {
    fn placement(&self, _main_root: &Path) -> Result<WorktreePlacement, String> {
        self.record("placement");
        let program = self.program.lock().expect("程序锁不可中毒");
        if let Some(fault) = program.placement_fault.clone() {
            return Err(fault);
        }
        Ok(WorktreePlacement {
            root: self.worktree_root.clone(),
            rel: program.rel.clone().unwrap_or_default(),
        })
    }

    fn tracked_at_head(&self, _main_root: &Path, rel: &str) -> Result<bool, String> {
        self.record(&format!("tracked_at_head:{rel}"));
        self.tracked_calls
            .lock()
            .expect("判据调用锁不可中毒")
            .push(rel.to_owned());
        self.program
            .lock()
            .expect("程序锁不可中毒")
            .tracked_at_head
            .clone()
            .unwrap_or(Ok(true))
    }

    fn probe(&self, _main_root: &Path) -> Result<RepoProbe, String> {
        self.record("probe");
        self.program
            .lock()
            .expect("程序锁不可中毒")
            .probe
            .clone()
            .unwrap_or_else(|| {
                Ok(RepoProbe {
                    head: FAKE_HEAD.to_owned(),
                    dirty: false,
                })
            })
    }

    fn branch_exists(&self, _main_root: &Path, branch: &str) -> Result<bool, String> {
        self.record(&format!("branch_exists:{branch}"));
        Ok(self.program.lock().expect("程序锁不可中毒").branch_exists)
    }

    fn add_worktree(&self, main_root: &Path, worktree: &Path, branch: &str) -> Result<(), String> {
        self.record(&format!("add:{branch}"));
        self.adds.lock().expect("add 锁不可中毒").push((
            main_root.to_owned(),
            worktree.to_owned(),
            branch.to_owned(),
        ));
        if let Some(fault) = self
            .program
            .lock()
            .expect("程序锁不可中毒")
            .add_fault
            .clone()
        {
            return Err(fault);
        }
        mirror_tree(main_root, worktree)?;
        // 「偏移子树未随检出」注入：镜像后删掉执行根（后置 is_dir 断言与补偿
        // 链入参的观察面）
        let (drop_exec_root, rel) = {
            let program = self.program.lock().expect("程序锁不可中毒");
            (
                program.drop_exec_root,
                program.rel.clone().unwrap_or_default(),
            )
        };
        if drop_exec_root && !rel.is_empty() {
            let exec_root = worktree.join(&rel);
            fs::remove_dir_all(&exec_root).map_err(|error| format!("假件删执行根失败: {error}"))?;
        }
        Ok(())
    }

    fn remove_worktree(&self, _main_root: &Path, worktree: &Path) -> Result<(), String> {
        self.record("remove_worktree");
        self.removes
            .lock()
            .expect("回收锁不可中毒")
            .push(worktree.to_owned());
        if let Some(fault) = self
            .program
            .lock()
            .expect("程序锁不可中毒")
            .remove_fault
            .clone()
        {
            return Err(fault);
        }
        fs::remove_dir_all(worktree).map_err(|error| format!("假件移除失败: {error}"))
    }

    fn delete_branch(&self, _main_root: &Path, branch: &str) -> Result<(), String> {
        self.record(&format!("delete_branch:{branch}"));
        if let Some(fault) = self
            .program
            .lock()
            .expect("程序锁不可中毒")
            .delete_branch_fault
            .clone()
        {
            return Err(fault);
        }
        Ok(())
    }

    fn run_install(&self, worktree: &Path, command: &str) -> Result<InstallRun, String> {
        self.record(&format!("install:{command}"));
        self.installs
            .lock()
            .expect("安装锁不可中毒")
            .push((worktree.to_owned(), command.to_owned()));
        self.program
            .lock()
            .expect("程序锁不可中毒")
            .install_result
            .clone()
            .unwrap_or_else(|| {
                Ok(InstallRun {
                    success: true,
                    summary: String::new(),
                })
            })
    }
}

// ---------------------------------------------------------------------------
// 假件 store：get_change / list_change_records / create_change_record /
// delete_change_record 四个 min 操作镜像真件语义（主键 id 存取、补偿删除
// 可编程故障），其余 unimplemented（越权触达即 panic）。
// ---------------------------------------------------------------------------

struct CreateStore {
    /// 建档记录（建档即插入、补偿删除即移除——db 存活的内存像）。
    records: Mutex<Vec<ChangeStateRecord>>,
    /// create_change_record 调用捕获（零建档断言观察面）。
    creates: Mutex<Vec<ChangeStateRecord>>,
    /// delete_change_record 调用捕获（补偿路径断言观察面）。
    deletes: Mutex<Vec<String>>,
    /// 补偿删除注入故障（双故障角落行）。
    delete_fault: Mutex<Option<StoreFault>>,
}

impl CreateStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            creates: Mutex::new(Vec::new()),
            deletes: Mutex::new(Vec::new()),
            delete_fault: Mutex::new(None),
        }
    }

    /// 建档记录预置（id 与 name 双值可辨的种子面——同名 active 拒绝 / 同名
    /// 归档共存各行共用）。
    fn seed(&self, id: &str, name: &str, status: ChangeStatus) {
        self.records
            .lock()
            .expect("记录锁不可中毒")
            .push(ChangeStateRecord {
                id: id.to_owned(),
                name: name.to_owned(),
                title: name.to_owned(),
                workflow_type: "requirement".to_owned(),
                created_at: 1_727_000_000_000,
                status,
                archived_at: matches!(status, ChangeStatus::Archived).then_some(1_727_000_000_000),
                active_phase: None,
                worktree: None,
                exec_root: None,
                base_commit: None,
            });
    }

    fn set_delete_fault(&self, fault: StoreFault) {
        *self.delete_fault.lock().expect("故障锁不可中毒") = Some(fault);
    }

    fn create_call_count(&self) -> usize {
        self.creates.lock().expect("建档锁不可中毒").len()
    }

    /// 主键 id 直查（一切寻址以 id 为准——name 非身份键）。
    fn find_by_id(&self, id: &str) -> Option<ChangeStateRecord> {
        self.records
            .lock()
            .expect("记录锁不可中毒")
            .iter()
            .find(|record| record.id == id)
            .cloned()
    }

    /// 最近一次建档捕获（本次铸出 id / 建档行逐字一致的观察面）。
    fn created_record(&self) -> ChangeStateRecord {
        self.creates
            .lock()
            .expect("建档锁不可中毒")
            .last()
            .expect("建档记录应已捕获")
            .clone()
    }

    /// 本次铸出 id（补偿链按 id 断言 / 未建档 miss 断言的观察面）。
    fn created_id(&self) -> String {
        self.created_record().id
    }

    /// 库内存活行数（补偿链零残留断言面）。
    fn record_count(&self) -> usize {
        self.records.lock().expect("记录锁不可中毒").len()
    }

    /// 库内存活行快照（同名共存行断言面）。
    fn records(&self) -> Vec<ChangeStateRecord> {
        self.records.lock().expect("记录锁不可中毒").clone()
    }
}

impl ChangeStateStore for CreateStore {
    fn get_change(&self, id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self.find_by_id(id))
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        Ok(self.records.lock().expect("记录锁不可中毒").clone())
    }

    fn list_phase_records(&self, _change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn list_steps(
        &self,
        _change_id: &str,
        _run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        unimplemented!("本用例不可达")
    }

    fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault> {
        self.creates
            .lock()
            .expect("建档锁不可中毒")
            .push(record.clone());
        self.records.lock().expect("记录锁不可中毒").push(record);
        Ok(())
    }

    fn delete_change_record(&self, id: &str) -> Result<bool, StoreFault> {
        self.deletes
            .lock()
            .expect("删除锁不可中毒")
            .push(id.to_owned());
        if let Some(fault) = self.delete_fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        let mut records = self.records.lock().expect("记录锁不可中毒");
        let before = records.len();
        records.retain(|record| record.id != id);
        Ok(records.len() < before)
    }

    fn delete_change_cascade(&self, _id: &str) -> Result<bool, StoreFault> {
        Ok(false)
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

    fn list_runs(&self, _change: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
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

/// 当前 UTC 日历日期 `YYYY-MM-DD`（与写面同式；「当日」断言的界用）。
fn utc_date_today() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

/// created 落在调用前后的 UTC 日期并集内（防御测试恰跨 UTC 午夜）。
fn assert_today(created: &str, before: &str, after: &str) {
    assert!(
        created == before || created == after,
        "created 应为 UTC 当日日期（YYYY-MM-DD），实际: {created}"
    );
}

// ---------------------------------------------------------------------------
// 正向：建域四件套成功
// ---------------------------------------------------------------------------

/// 成功 → worktree 目录建域 + worktree 内 change 目录 + explore.md 含 goal
/// 原文 + ChangeRecord 建档（携本次铸出 id / worktree / base_commit）；
/// `outcome.id` 非空且与库内记录逐字一致（id 主键直查命中）；`record.name`
/// 为裸名属性；add_worktree 收到 (main_root, worktree, change/<name>)；主仓
/// active 树不含该目录；目录树内零 workflow.json 产出（双向墙写半边）。
#[test]
fn 建域四件套成功_worktree内目录与explore与db建档且主仓零目录() {
    let env = Env::new("happy");
    let before = utc_date_today();

    let outcome = env
        .create("fix-bug", "修复登录重试的竞态问题")
        .expect("合法输入应 Ok");

    let after = utc_date_today();
    assert_eq!(outcome.name, "fix-bug");
    assert!(
        !outcome.id.is_empty(),
        "本次铸出 id 非空（uuid 形态身份锚）"
    );
    assert_today(&outcome.created, &before, &after);
    assert_eq!(
        outcome.worktree,
        env.worktree_root.join("fix-bug").to_string_lossy(),
        "worktree 绝对路径出线"
    );
    assert_eq!(
        outcome.warnings,
        vec!["未识别依赖管理器，跳过依赖引导".to_owned()],
        "空检出无已知管理器 → 注记入 warnings（干净 + 顺利 → 空清单见 bootstrap 行）"
    );

    // vcs 半边：add_worktree 收到 (main_root, worktree, change/<name>)
    let adds = env.vcs.adds.lock().expect("add 锁不可中毒").clone();
    assert_eq!(adds.len(), 1, "add_worktree 恰一次");
    assert_eq!(adds[0].0, env.root, "add 收到主仓根");
    assert_eq!(
        adds[0].1,
        env.worktree_root.join("fix-bug"),
        "add 收到 worktree 落位目录"
    );
    assert_eq!(adds[0].2, "change/fix-bug", "add 收到铸分支名");

    // fs 半边：worktree 内目录 + explore.md 落 goal 原文
    assert!(
        env.change_dir("fix-bug").is_dir(),
        "worktree 内 change 目录创建"
    );
    assert_eq!(
        env.explore_bytes("fix-bug"),
        "修复登录重试的竞态问题".as_bytes()
    );

    // db 半边：id 主键直查命中且与出线逐字一致（id 身份锚；name 裸名属性）
    let record = env
        .store
        .find_by_id(&outcome.id)
        .expect("find_change_record(outcome.id) 应命中");
    assert_eq!(record.id, outcome.id, "库内 id 与出线逐字一致");
    assert_eq!(
        record.id,
        env.store.created_id(),
        "建档载荷即本次铸出 id（铸出路径行为锚）"
    );
    assert_eq!(record.name, "fix-bug", "record.name 为裸名属性");
    assert_eq!(record.workflow_type, "requirement");
    assert_eq!(record.status, ChangeStatus::Active);
    assert_eq!(record.archived_at, None);
    assert!(record.active_phase.is_none(), "建档默认零 active_phase");
    assert_eq!(
        record.worktree.as_deref(),
        Some(outcome.worktree.as_str()),
        "建档记录携 worktree 执行锚"
    );
    assert_eq!(
        record.base_commit.as_deref(),
        Some(FAKE_HEAD),
        "基线 = probe.head"
    );

    // 主仓 active 树不含该目录（编辑落点囚于 worktree）
    assert!(
        env.active_dir_names().is_empty(),
        "主仓 active 树零目录，实际: {:?}",
        env.active_dir_names()
    );

    // 双向墙：worktree 树内零 workflow.json 产出
    let files = env.worktree_tree_file_names("fix-bug");
    assert!(
        !files.iter().any(|name| name == "workflow.json"),
        "目录树零 workflow.json 产出，实际: {files:?}"
    );
    assert_eq!(files, vec!["explore.md"], "目录树仅 explore.md 一文件");
}

/// 空白树深层建树：调用前 worktree 全链不存在，建树后目录链完整。
#[test]
fn 空白树深层建树_全链不存在时建全树() {
    let env = Env::new("blank-tree");
    assert!(
        !env.change_dir("deep-root").exists(),
        "前置：worktree 内 changes_root 全链不存在"
    );

    env.create("deep-root", "空白树首个 change")
        .expect("空白树应建树成功");

    assert!(env.change_dir("deep-root").join("explore.md").is_file());
}

/// 显式 title 入参（新增）：title ≠ name 传入 → 假 store 捕获建档载荷
/// `title == 显式值`（name 裸名零污染）；worktree 内 explore.md = goal 原文不变
/// （title 为纯投影零行为）。
#[test]
fn create显式title_建档载荷携显式值且explore原文不变() {
    let env = Env::new("explicit-title");

    let outcome = env
        .create_with_title("fix-bug", "修复登录重试的竞态问题", "修复登录竞态")
        .expect("合法输入应 Ok");

    let record = env.store.created_record();
    assert_eq!(record.title, "修复登录竞态", "建档载荷 title == 显式值");
    assert_eq!(record.name, "fix-bug", "name 恒裸名（title 独立字段）");
    assert_ne!(record.title, record.name, "title 与 name 双字段可辨");
    assert_eq!(record.id, outcome.id, "建档载荷即本次铸出 id");
    assert_eq!(
        env.explore_bytes("fix-bug"),
        "修复登录重试的竞态问题".as_bytes(),
        "title 为纯投影：explore.md 仍为 goal 原文"
    );
}

/// 空 / 空白 title 回退 name（边界）：title 空串 / 纯空白 → 建档载荷
/// `title == name`（回退单点在写面，恒非空）。
#[test]
fn create空title回退name_建档载荷兜底() {
    for (idx, title) in ["", "   ", "\n\t"].iter().enumerate() {
        let env = Env::new(&format!("blank-title-{idx}"));
        let name = format!("topic-{idx}");

        env.create_with_title(&name, "回退 goal", title)
            .expect("合法输入应 Ok");

        let record = env.store.created_record();
        assert_eq!(
            record.title, name,
            "空白 title {title:?} 回退 name（title 恒非空单点）"
        );
        assert!(!record.title.is_empty(), "回退后 title 恒非空");
    }
}

/// 建档载荷含 title 字段（正向）：假 store 捕获的 ChangeStateRecord 逐字段
/// 断言（title / name / goal 载体 / 执行锚）。
#[test]
fn create建档载荷含title字段逐字段断言() {
    let env = Env::new("payload-title");

    let outcome = env
        .create_with_title("payload-change", "需求原文 goal", "载荷标题")
        .expect("合法输入应 Ok");

    let record = env.store.created_record();
    assert_eq!(record.title, "载荷标题", "载荷 title 逐字");
    assert_eq!(record.name, "payload-change", "载荷 name 逐字");
    assert_eq!(record.workflow_type, "requirement", "缺省类型 requirement");
    assert_eq!(record.status, ChangeStatus::Active, "active 起步");
    assert_eq!(record.archived_at, None);
    assert_eq!(record.active_phase, None);
    assert_eq!(
        record.worktree.as_deref(),
        Some(outcome.worktree.as_str()),
        "建档载荷携 worktree 执行锚"
    );
    assert_eq!(record.base_commit.as_deref(), Some(FAKE_HEAD));
}

/// workflow_type 透传（正向 / 新增）：传 `"test-only"` → 落库
/// `ChangeStateRecord.workflow_type == "test-only"`（硬编码 requirement 退役），
/// 建域照常（worktree 内目录 + explore.md 仍在）。
#[test]
fn create透传test_only类型的建档载荷() {
    let env = Env::new("wf-test-only");

    let outcome = env
        .create_full("topic-only", "补测试向 goal", "补测试", "test-only")
        .expect("合法 workflow_type 应 Ok");

    let record = env.store.created_record();
    assert_eq!(
        record.workflow_type, "test-only",
        "落库 workflow_type == 入参"
    );
    assert_eq!(record.id, outcome.id, "建档载荷即本次铸出 id");
    assert_eq!(
        record.title, "补测试",
        "title 与 workflow_type 两入参互不串扰"
    );
    assert!(
        env.change_dir("topic-only").join("explore.md").is_file(),
        "建域照常（workflow_type 为纯载荷，零建域分支）"
    );
}

/// workflow_type 缺省语义持衡（正向 / 改写）：传 `"requirement"` → 落库
/// `"requirement"`（既有默认行为逐字不变）。
#[test]
fn create透传requirement类型持衡() {
    let env = Env::new("wf-requirement");

    env.create_full("topic-req", "既有语义 goal", "既有语义", "requirement")
        .expect("合法 workflow_type 应 Ok");

    let record = env.store.created_record();
    assert_eq!(
        record.workflow_type, "requirement",
        "显式 requirement 逐字落库"
    );
}

/// 大小写 / 空白 canonical（边界 / 新增）：首尾空白 + 大写混排 → trim +
/// 小写后命中相位表并落 canonical 小写值。
#[test]
fn create_workflow_type大小写与空白规范化落小写() {
    let cases = [
        ("Test-Only", "test-only"),
        ("  test-only  ", "test-only"),
        ("REQUIREMENT", "requirement"),
        (" Requirement ", "requirement"),
        ("\ttest-only\n", "test-only"),
    ];
    for (idx, (input, expected)) in cases.iter().enumerate() {
        let env = Env::new(&format!("wf-canonical-{idx}"));
        let name = format!("canonical-{idx}");

        env.create_full(&name, "规范化 goal", &name, input)
            .expect("规范化后命中应 Ok");

        let record = env.store.created_record();
        assert_eq!(
            record.workflow_type, *expected,
            "入参 {input:?} 落 canonical 小写值"
        );
    }
}

/// 未知 workflow_type 拒绝（异常 / 新增）：非法值 → 显式 `Err` 且零建档 /
/// 零目录 / 零 vcs 调用（校验全 IO 前置），文案通吃 `{requirement, test-only}`。
#[test]
fn create非法workflow_type拒绝_零建档零建域() {
    for (idx, workflow_type) in ["bug-fix", "", " ", "\t\n", "refactor", "test_only"]
        .iter()
        .enumerate()
    {
        let env = Env::new(&format!("wf-invalid-{idx}"));

        let error = env
            .create_full("bad-wf", "非法类型 goal", "非法类型", workflow_type)
            .expect_err("未知 workflow_type 应 Err");

        assert!(
            error.contains("workflow_type")
                && error.contains("requirement")
                && error.contains("test-only"),
            "错误文案归因并列出合法值集，实际: {error}"
        );
        assert_eq!(env.store.create_call_count(), 0, "db 零建档");
        assert!(
            env.active_dir_names().is_empty(),
            "零目录（拒绝面零副作用），实际: {:?}",
            env.active_dir_names()
        );
        assert_eq!(
            env.worktree_root.read_dir().map(|d| d.count()).unwrap_or(0),
            0,
            "零 worktree 目录"
        );
        assert!(
            env.vcs.calls().is_empty(),
            "vcs 零调用（含 probe），实际: {:?}",
            env.vcs.calls()
        );
    }
}

/// workflow_type 边界（边界 / 新增）：超长（>1000 字符）与特殊字符（NUL / 内嵌
/// 换行 / emoji）类型值不命中合法值集 → 显式 `Err`，零建档零建域零 vcs 调用
///（校验全 IO 前置，长值不截断、不折叠）。
#[test]
fn create_workflow_type超长与特殊字符拒绝() {
    let long_unknown = "x".repeat(1200);
    let cases = [
        long_unknown.as_str(),
        "requirement\u{0}",
        "test\u{0}-only",
        "require\nment",
        "test-only🚀",
    ];
    for (idx, workflow_type) in cases.iter().enumerate() {
        let env = Env::new(&format!("wf-edge-{idx}"));

        let error = env
            .create_full("edge-wf", "边界类型 goal", "边界类型", workflow_type)
            .expect_err("超长 / 特殊字符 workflow_type 应 Err");

        assert!(
            error.contains("workflow_type"),
            "Err 记因携字段名，实际: {error}"
        );
        assert_eq!(env.store.create_call_count(), 0, "db 零建档");
        assert!(
            env.active_dir_names().is_empty(),
            "零目录（拒绝面零副作用），实际: {:?}",
            env.active_dir_names()
        );
        assert!(
            env.vcs.calls().is_empty(),
            "vcs 零调用（含 probe），实际: {:?}",
            env.vcs.calls()
        );
    }
}

/// 校验顺序边界（边界 / 新增）：workflow_type 校验先于 git 探测但后于名称 /
/// goal 校验——非法名与非法 workflow_type 同投 → 归因名称；合法名 + 非法
/// workflow_type → 归因类型。
#[test]
fn create_workflow_type校验顺序_名称先决() {
    let env = Env::new("wf-order");

    let error = env
        .create_full("Bad_Name", "ok goal", "ok", "bug-fix")
        .expect_err("非法名应 Err");
    assert!(
        error.contains("kebab-case"),
        "非法名先决于 workflow_type，实际: {error}"
    );

    let env2 = Env::new("wf-order-2");
    let error = env2
        .create_full("valid-name", "ok goal", "ok", "bug-fix")
        .expect_err("非法类型应 Err");
    assert!(
        error.contains("workflow_type"),
        "名称合法后归因 workflow_type，实际: {error}"
    );
    assert_eq!(env2.store.create_call_count(), 0, "零建档");
}

/// goal 多行 + emoji + 超长字符 + 首尾空白：explore.md 字节保真（UTF-8
/// free-form 原文直写，写面零 trim）。
#[test]
fn explore_md落goal原文零包装零trim() {
    let env = Env::new("goal-rich");
    let long_tail = "长".repeat(1000);
    let goal = format!("  第一行\n第二行\t制表符 🚀 emoji\n{long_tail}  ");
    assert!(goal.chars().count() > 1000, "前置：超 1000 字符");

    env.create("rich-goal", &goal).expect("合法输入应 Ok");

    assert_eq!(
        env.explore_bytes("rich-goal"),
        goal.as_bytes(),
        "写面零 trim 保真"
    );
}

// ---------------------------------------------------------------------------
// 异常：前置七道（D3 全 IO 前置——拒绝面零 worktree、零建档、零目录、vcs 零调用）
// ---------------------------------------------------------------------------

/// 非法名（kebab-case 违例 / 非单分量）与空白 goal → `Err` 且零目录零建档零
/// vcs 调用；校验顺序名称优先。
#[test]
fn 非法名与空白goal拒绝且零目录零建档零vcs调用() {
    let env = Env::new("bad-inputs");
    let invalid_names = [
        "Fix-Bug",  // 大写
        "fix_bug",  // 下划线
        "1fix",     // 前导数字
        "fix-",     // 尾连字符
        "fix--bug", // 连号连字符
        "",         // 空串
        "a/b",      // 穿越分量
    ];
    for name in invalid_names {
        assert!(
            env.create(name, "拒绝面 goal").is_err(),
            "非法名 {name:?} 应 Err"
        );
    }
    for goal in ["", "   ", "\n\t"] {
        assert!(
            env.create("fix-bug", goal).is_err(),
            "空白 goal {goal:?} 应 Err"
        );
    }

    assert!(
        env.active_dir_names().is_empty(),
        "零目录（校验全 IO 前置），实际: {:?}",
        env.active_dir_names()
    );
    assert!(
        env.worktree_root.read_dir().map(|d| d.count()).unwrap_or(0) == 0,
        "零 worktree 目录"
    );
    assert_eq!(env.store.create_call_count(), 0, "db 零建档");
    assert!(
        env.vcs.calls().is_empty(),
        "vcs 零调用（含 probe），实际: {:?}",
        env.vcs.calls()
    );

    // 校验顺序：非法名 + 空白 goal 同投，Err 归因名称校验
    let error = env.create("Bad_Name", "   ").expect_err("非法名应 Err");
    assert!(
        error.contains("kebab-case") && !error.contains("goal"),
        "归因名称校验（名称先决），实际: {error}"
    );
}

/// Windows 保留设备名拒绝：纯内存前置校验，零目录零建档零 vcs 调用。
#[test]
fn windows保留设备名拒绝且零目录零建档零vcs调用() {
    let env = Env::new("reserved-name");
    for name in ["nul", "con", "prn", "aux", "com1", "com9", "lpt0", "lpt9"] {
        let error = env.create(name, "拒绝面 goal").expect_err("保留名应 Err");
        assert!(
            error.contains("保留设备名"),
            "错误应点名保留设备名（{name}）: {error}"
        );
    }
    assert!(env.active_dir_names().is_empty(), "零目录");
    assert_eq!(env.store.create_call_count(), 0, "零建档");
    assert!(env.vcs.calls().is_empty(), "vcs 零调用");
}

/// 近邻名不受保留名规则误伤：COM0（git 允许）与 nul-safe 走正常创建。
#[test]
fn 保留名近邻可创建() {
    let env = Env::new("reserved-neighbors");
    env.create("com0", "合法 goal").expect("COM0 应可创建");
    env.create("nul-safe", "合法 goal")
        .expect("nul-safe 应可创建");
}

/// 超 128 字符拒绝：129 字符合法字符集名 Err 且零产生。
#[test]
fn 超128字符拒绝且零产生() {
    let env = Env::new("width-129");
    let name = format!("a{}", "b".repeat(128));
    assert_eq!(name.len(), 129, "前置：129 字节");

    let error = env.create(&name, "越界 goal").expect_err("超 128 应 Err");
    assert!(error.contains("128"), "错误归因长度限制，实际: {error}");
    assert!(env.active_dir_names().is_empty(), "零产生");
    assert_eq!(env.store.create_call_count(), 0, "db 零建档");
    assert!(env.vcs.calls().is_empty(), "vcs 零调用");
}

/// 前置③（持衡）：主仓 active 目录已存在 → `Err` 零副作用（vcs 零调用含
/// probe——冲突检查先于 git 探测的顺序锚）、既有目录零触碰、零建档。
#[test]
fn 目录冲突拒绝_零目录零建档零vcs调用() {
    let env = Env::new("dir-taken");
    let taken_dir = resolve(&env.root).changes_root.join("taken");
    fs::create_dir_all(&taken_dir).expect("预置目录失败");
    fs::write(taken_dir.join("proposal.md"), "# 既有").expect("预置文件失败");

    let error = env
        .create("taken", "新建 goal")
        .expect_err("同名目录应 Err");

    assert!(
        error.contains("已存在") && error.contains("taken"),
        "错误归因已存在并携带目录语境，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "db 零建档");
    assert!(taken_dir.join("proposal.md").is_file(), "既有目录零触碰");
    assert!(
        env.vcs.calls().is_empty(),
        "vcs 零调用（冲突检查先于 git 探测），实际: {:?}",
        env.vcs.calls()
    );
}

/// 前置④（重写）：db 已有同名 **active** 建档记录（id 各异）→ `Err` 且零目录
/// 零建档零 vcs 调用——查重改 name 扫描（同 id 防御性主键冲突面消失，「同名
/// active 冲突」归写面单点；name 无唯一约束、不作身份键）。
#[test]
fn 同名active拒绝前置_name扫描零目录零建档() {
    let env = Env::new("db-taken");
    let old_id = "0198f7a0-0000-7000-8000-0000000000b1";
    env.store.seed(old_id, "taken", ChangeStatus::Active);

    let error = env
        .create("taken", "新建 goal")
        .expect_err("同名 active 应 Err");

    assert!(
        error.contains("已存在") && error.contains("taken"),
        "错误归因同名建档冲突（携 name 语境），实际: {error}"
    );
    assert!(
        error.contains(ChangeStatus::Active.as_str()),
        "拒绝面呈现 status=active 口径（归档同名不拒——见同名单次重铸行），实际: {error}"
    );
    assert!(env.active_dir_names().is_empty(), "零目录创建");
    assert_eq!(env.store.create_call_count(), 0, "db 零建档");
    assert_eq!(env.store.record_count(), 1, "库内记录零变动（仅种子行）");
    assert_eq!(
        env.store.find_by_id(old_id).expect("种子行仍在场").name,
        "taken",
        "种子行零触碰"
    );
    assert!(
        env.vcs.calls().is_empty(),
        "vcs 零调用（含 probe），实际: {:?}",
        env.vcs.calls()
    );
}

/// 同名重铸（新增）：同名 **archived** 记录在场（id = 旧铸出值）→ 再建档
/// 合法化成功 → 新 id ≠ 旧 id、两行并存（D11：name 无唯一约束，归档同名共存
/// 合法；身份恒以 id 为锚）。
#[test]
fn 同名归档再建档_新id相异且两行并存() {
    let env = Env::new("re-cast");
    let old_id = "0198f7a0-0000-7000-8000-0000000000c1";
    env.store.seed(old_id, "same-name", ChangeStatus::Archived);

    let outcome = env
        .create("same-name", "同名重铸 goal")
        .expect("归档同名不拒（D11 合法化）");

    assert_ne!(outcome.id, old_id, "同名单次重铸 id 相异");
    assert_ne!(outcome.id.as_str(), "same-name", "id 非 name 复用");
    let records = env.store.records();
    assert_eq!(records.len(), 2, "两行并存（同名 active + archived）");
    assert!(
        records.iter().all(|record| record.name == "same-name"),
        "两行同 name（裸名属性），实际: {:?}",
        records
            .iter()
            .map(|record| &record.name)
            .collect::<Vec<_>>()
    );
    let statuses: Vec<ChangeStatus> = records.iter().map(|record| record.status).collect();
    assert!(
        statuses.contains(&ChangeStatus::Active) && statuses.contains(&ChangeStatus::Archived),
        "active 新行与 archived 旧行并存，实际: {statuses:?}"
    );
    let new_record = env.store.find_by_id(&outcome.id).expect("新 id 直查应命中");
    assert_eq!(new_record.name, "same-name");
    assert_ne!(
        new_record.created_at, 1_727_000_000_000,
        "新 id 寻址到本次建档行（非种子旧行）"
    );
    assert!(
        env.store.find_by_id(old_id).is_some(),
        "旧 id 行仍在场（id 各异两行并存）"
    );
}

/// 前置⑤：git 探测三态（git 不可发现 / 非 git 仓 / 空仓无 HEAD）→ 各自显式
/// `Err`（含引导文案）且零建档零目录零 add——MUST NOT 静默回退主 root 创建。
#[test]
fn git三态拒绝显式err不静默回退() {
    let probes = [
        "git 不可用（PATH 未发现 git）: 环境缺失",
        "主仓 X 为非 git 仓：请先 git init 并提交",
        "主仓 X 为空 git 仓（无任何提交，HEAD 不存在）：请先提交再开新 change",
    ];
    for (idx, probe_error) in probes.iter().enumerate() {
        let env = Env::new(&format!("git-probe-{idx}"));
        env.vcs.set_probe_error(probe_error);

        let error = env
            .create("fix-bug", "探测失败 goal")
            .expect_err("probe Err 应 Err");

        assert!(
            error.contains(probe_error),
            "Err 显式呈现 probe 引导文案，实际: {error}"
        );
        assert_eq!(env.store.create_call_count(), 0, "零建档");
        assert!(env.active_dir_names().is_empty(), "零目录");
        assert!(
            !env.worktree_root.join("fix-bug").exists(),
            "零 worktree（不静默回退主 root 创建）"
        );
        assert!(
            !env.vcs.calls().iter().any(|call| call.starts_with("add:")),
            "零 add_worktree"
        );
    }
}

/// 前置⑥：branch `change/<name>` 已存在 → `Err` 含 branch 名且零建档零 add。
#[test]
fn branch冲突拒绝且零建档零add() {
    let env = Env::new("branch-taken");
    env.vcs.set_branch_exists();

    let error = env
        .create("fix-bug", "branch 冲突 goal")
        .expect_err("branch 冲突应 Err");

    assert!(
        error.contains("change/fix-bug"),
        "Err 含冲突 branch 名，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "零建档");
    assert!(
        !env.vcs.calls().iter().any(|call| call.starts_with("add:")),
        "零 add_worktree"
    );
}

/// 前置⑦：worktree 目标目录已存在 → `Err` 含该路径且零建档。
#[test]
fn worktree目录冲突拒绝且零建档() {
    let env = Env::new("wt-taken");
    let taken = env.worktree_root.join("fix-bug");
    fs::create_dir_all(&taken).expect("预置 worktree 目录失败");

    let error = env
        .create("fix-bug", "目录冲突 goal")
        .expect_err("worktree 目录冲突应 Err");

    assert!(
        error.contains(taken.to_string_lossy().as_ref()),
        "Err 含冲突路径，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "零建档");
}

// ---------------------------------------------------------------------------
// 警告面：脏仓 / bootstrap（D4 / D5）
// ---------------------------------------------------------------------------

/// 脏仓：create 成功、warnings 含「先提交再开新 change」引导（D5 逐字锚）；
/// 基线段 base_commit 仍 = probe.head（基线恒 HEAD）。
#[test]
fn 脏仓警告引导先提交且基线仍head() {
    let env = Env::new("dirty-main");
    seed_main_file(&env, "pnpm-lock.yaml", "# pnpm");
    env.vcs.set_dirty();

    let outcome = env.create("fix-bug", "脏仓 goal").expect("脏仓警告不阻止");

    assert_eq!(
        outcome.warnings.len(),
        1,
        "恰脏仓一条警告，实际: {:?}",
        outcome.warnings
    );
    assert!(
        outcome.warnings[0].contains("主仓有未提交改动")
            && outcome.warnings[0].contains("建议先提交再开新 change"),
        "脏仓引导文案锚（D5），实际: {:?}",
        outcome.warnings[0]
    );
    let record = env.store.created_record();
    assert_eq!(record.name, "fix-bug", "裸名属性");
    assert_eq!(
        record.base_commit.as_deref(),
        Some(FAKE_HEAD),
        "基线仍 HEAD"
    );
}

/// bootstrap 映射表（D4 四行）：worktree 检出（假件镜像主仓树）预置各已知
/// lockfile → `run_install` 恰一次且命令串逐字命中。
#[test]
fn bootstrap映射表四行首匹配命中() {
    let table = [
        ("pnpm-lock.yaml", "pnpm install --frozen-lockfile"),
        ("package-lock.json", "npm ci"),
        ("yarn.lock", "yarn install --frozen-lockfile"),
        ("Cargo.lock", "cargo fetch --locked"),
    ];
    for (idx, (lockfile, command)) in table.iter().enumerate() {
        let env = Env::new(&format!("bootstrap-{idx}"));
        seed_main_file(&env, lockfile, "# lockfile 夹具");

        let outcome = env.create("fix-bug", "bootstrap goal").expect("建域应 Ok");

        assert_eq!(
            env.vcs.install_commands(),
            vec![(*command).to_owned()],
            "{lockfile} → 命令串逐字命中映射表"
        );
        assert!(
            outcome.warnings.is_empty(),
            "顺利安装零警告，实际: {:?}",
            outcome.warnings
        );
    }
}

/// 首匹配序：多 lockfile 并存 → 恰命中映射表首匹配项、`run_install` 恰一次。
#[test]
fn bootstrap首匹配序多lockfile并存恰一次() {
    let env = Env::new("bootstrap-first");
    seed_main_file(&env, "package-lock.json", "# npm");
    seed_main_file(&env, "pnpm-lock.yaml", "# pnpm");

    env.create("fix-bug", "首匹配 goal").expect("建域应 Ok");

    assert_eq!(
        env.vcs.install_commands(),
        vec!["pnpm install --frozen-lockfile".to_owned()],
        "首匹配 = pnpm 行"
    );
}

/// 未知管理器：无任何已知 lockfile → 零 `run_install` 调用 + 注记入 warnings
///（D5 逐字锚）。
#[test]
fn 未知管理器跳过且注记() {
    let env = Env::new("no-manager");

    let outcome = env.create("fix-bug", "未知管理器 goal").expect("建域应 Ok");

    assert!(env.vcs.install_commands().is_empty(), "零 run_install 调用");
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning == "未识别依赖管理器，跳过依赖引导"),
        "未识别注记逐字锚（D5），实际: {:?}",
        outcome.warnings
    );
}

/// Cargo 无 lock 专项注记：`Cargo.toml` 在场而 `Cargo.lock` 缺席 → 跳过 +
/// 专项注记（先于「未识别」判定）。
#[test]
fn cargo无lock专项注记先于未识别() {
    let env = Env::new("cargo-no-lock");
    seed_main_file(&env, "Cargo.toml", "[package]");

    let outcome = env.create("fix-bug", "cargo goal").expect("建域应 Ok");

    assert!(env.vcs.install_commands().is_empty(), "跳过安装");
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("检测到 Cargo.toml 但无 Cargo.lock")),
        "Cargo 无 lock 专项注记（D5），实际: {:?}",
        outcome.warnings
    );
    assert!(
        !outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("未识别依赖管理器")),
        "不落「未识别」注记（专项先于未识别判定）"
    );
}

/// 安装非零退出：create 以 Ok 收口（建档与 worktree 保留——AC-5 不回滚）且
/// warnings 含「依赖引导失败（{command}）: {summary}」。
#[test]
fn 安装非零退出不回滚且警告呈现() {
    let env = Env::new("install-fail");
    seed_main_file(&env, "pnpm-lock.yaml", "# pnpm");
    env.vcs.set_install_failure("ERR_PNPM_PEER_RESOLUTION");

    let outcome = env
        .create("fix-bug", "安装失败 goal")
        .expect("bootstrap 失败不回滚不阻断");

    assert!(
        env.store.find_by_id(&env.store.created_id()).is_some(),
        "建档保留（不回滚）"
    );
    assert!(
        env.worktree_root.join("fix-bug").is_dir(),
        "worktree 保留（不回滚）"
    );
    assert!(
        outcome.warnings.iter().any(|warning| warning
            == "依赖引导失败（pnpm install --frozen-lockfile）: ERR_PNPM_PEER_RESOLUTION"),
        "失败警告逐字锚（D5），实际: {:?}",
        outcome.warnings
    );
}

/// 安装拉起失败：create 以 Ok 收口且 warnings 含「依赖引导未执行成功
///（{command}）: {error}」（bootstrap 段失败不回收——D3）。
#[test]
fn 安装拉起失败不回收且警告呈现() {
    let env = Env::new("install-error");
    seed_main_file(&env, "pnpm-lock.yaml", "# pnpm");
    env.vcs
        .set_install_error("安装命令拉起失败（pnpm install --frozen-lockfile）: 无此程序");

    let outcome = env
        .create("fix-bug", "拉起失败 goal")
        .expect("bootstrap 失败不回滚不阻断");

    assert!(
        env.store.find_by_id(&env.store.created_id()).is_some(),
        "建档保留"
    );
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning
                .starts_with("依赖引导未执行成功（pnpm install --frozen-lockfile）")),
        "拉起失败警告锚（D5），实际: {:?}",
        outcome.warnings
    );
}

// ---------------------------------------------------------------------------
// bootstrap：projects[].root 项目模式（配置了项目根才逐项目扫描）
// ---------------------------------------------------------------------------

/// 多项目按配置序逐个扫描：各自命中 lockfile → 各自在项目目录执行安装
///（cwd = `<worktree>/<root>`），顺利时零警告。
#[test]
fn projects逐个扫描按项目根执行安装_多安装按序() {
    let env = Env::new("projects-multi");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "service/Cargo.lock", "# cargo");
    let roots = [proj("web"), proj("service")];

    let outcome = env
        .create_with_projects("fix-bug", "多项目 goal", &roots)
        .expect("建域应 Ok");

    let worktree = env.worktree_root.join("fix-bug");
    assert_eq!(
        env.vcs.installs(),
        vec![
            (
                worktree.join("web"),
                "pnpm install --frozen-lockfile".to_owned()
            ),
            (worktree.join("service"), "cargo fetch --locked".to_owned()),
        ],
        "按配置序逐项目安装，cwd = 项目根"
    );
    assert!(
        outcome.warnings.is_empty(),
        "顺利安装零警告，实际: {:?}",
        outcome.warnings
    );
}

/// 项目内首匹配即收口：同一项目多 lockfile 并存 → 该项目的映射表首匹配项、
/// 恰一次安装；后续项目继续。
#[test]
fn projects内首匹配单安装且后续项目继续() {
    let env = Env::new("projects-first");
    seed_main_path(&env, "web/package-lock.json", "# npm");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/pnpm-lock.yaml", "# pnpm");
    let roots = [proj("web"), proj("api")];

    let outcome = env
        .create_with_projects("fix-bug", "首匹配 goal", &roots)
        .expect("建域应 Ok");

    let worktree = env.worktree_root.join("fix-bug");
    assert_eq!(
        env.vcs.installs(),
        vec![
            (
                worktree.join("web"),
                "pnpm install --frozen-lockfile".to_owned()
            ),
            (
                worktree.join("api"),
                "pnpm install --frozen-lockfile".to_owned()
            ),
        ],
        "每项目恰一次安装（首匹配）"
    );
    assert!(outcome.warnings.is_empty());
}

/// 配置根在执行根内不存在：零安装 + 逐项目警告，create 照常成功。
#[test]
fn projects根不存在警告且零安装() {
    let env = Env::new("projects-missing");
    let roots = [proj("missing")];

    let outcome = env
        .create_with_projects("fix-bug", "缺失根 goal", &roots)
        .expect("建域应 Ok");

    assert!(env.vcs.install_commands().is_empty(), "零安装");
    assert_eq!(
        outcome.warnings,
        vec!["项目根 \"missing\"：在执行根内不存在，跳过依赖引导".to_owned()]
    );
}

/// 项目无 lockfile / Cargo 无 lock：逐项目专项注记，零安装。
#[test]
fn projects无lockfile与cargo无lock逐项目注记() {
    let env = Env::new("projects-notes");
    seed_main_path(&env, "plain/.keep", "");
    seed_main_path(&env, "rs/Cargo.toml", "[package]");
    let roots = [proj("plain"), proj("rs")];

    let outcome = env
        .create_with_projects("fix-bug", "注记 goal", &roots)
        .expect("建域应 Ok");

    assert!(env.vcs.install_commands().is_empty(), "零安装");
    assert_eq!(
        outcome.warnings,
        vec![
            "项目根 \"plain\"：未识别依赖管理器，跳过依赖引导".to_owned(),
            "项目根 \"rs\"：检测到 Cargo.toml 但无 Cargo.lock，跳过依赖引导（避免生成未跟踪 lockfile 混入变更上下文）".to_owned(),
        ]
    );
}

/// 非空 projects 抑制 worktree 根扫描：根上有 lockfile 也不安装。
#[test]
fn projects非空抑制worktree根扫描() {
    let env = Env::new("projects-suppress");
    seed_main_file(&env, "pnpm-lock.yaml", "# root pnpm");
    seed_main_path(&env, "web/.keep", "");
    let roots = [proj("web")];

    let outcome = env
        .create_with_projects("fix-bug", "抑制根扫描 goal", &roots)
        .expect("建域应 Ok");

    assert!(
        env.vcs.install_commands().is_empty(),
        "根 lockfile 不参与项目模式扫描"
    );
    assert_eq!(
        outcome.warnings,
        vec!["项目根 \"web\"：未识别依赖管理器，跳过依赖引导".to_owned()]
    );
}

/// 显式空 projects（命令层未配置）：回退 legacy worktree 根扫描，cwd = worktree 根。
#[test]
fn 空projects回退worktree根扫描() {
    let env = Env::new("projects-empty-legacy");
    seed_main_file(&env, "pnpm-lock.yaml", "# pnpm");

    let outcome = env
        .create_with_projects("fix-bug", "空 projects goal", &[])
        .expect("建域应 Ok");

    assert_eq!(
        env.vcs.installs(),
        vec![(
            env.worktree_root.join("fix-bug"),
            "pnpm install --frozen-lockfile".to_owned()
        )],
        "legacy 路径 cwd = worktree 根"
    );
    assert!(outcome.warnings.is_empty());
}

/// 单项目安装失败：不中断后续项目、不回滚 create，逐项目失败警告按序累积。
#[test]
fn projects单项目安装失败继续后续且不回滚() {
    let env = Env::new("projects-fail");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/package-lock.json", "# npm");
    env.vcs.set_install_failure("ERR_PNPM");
    let roots = [proj("web"), proj("api")];

    let outcome = env
        .create_with_projects("fix-bug", "失败继续 goal", &roots)
        .expect("bootstrap 失败不回滚不阻断");

    assert_eq!(env.vcs.installs().len(), 2, "失败后仍继续后续项目");
    assert_eq!(
        outcome.warnings,
        vec![
            "项目根 \"web\"：依赖引导失败（pnpm install --frozen-lockfile）: ERR_PNPM".to_owned(),
            "项目根 \"api\"：依赖引导失败（npm ci）: ERR_PNPM".to_owned(),
        ]
    );
    assert!(
        env.store.find_by_id(&env.store.created_id()).is_some(),
        "建档保留（不回滚）"
    );
    assert!(
        env.worktree_root.join("fix-bug").is_dir(),
        "worktree 保留（不回滚）"
    );
}

/// 脏仓警告先于项目警告：警告顺序 = 脏仓 → 项目配置序。
#[test]
fn 脏仓警告先于项目警告() {
    let env = Env::new("projects-dirty");
    env.vcs.set_dirty();
    let roots = [proj("missing")];

    let outcome = env
        .create_with_projects("fix-bug", "脏仓 goal", &roots)
        .expect("建域应 Ok");

    assert_eq!(outcome.warnings.len(), 2, "实际: {:?}", outcome.warnings);
    assert!(outcome.warnings[0].contains("主仓有未提交改动"));
    assert_eq!(
        outcome.warnings[1],
        "项目根 \"missing\"：在执行根内不存在，跳过依赖引导"
    );
}

/// `.` 项目根：等价扫描 worktree 根并在该处安装。
#[test]
fn projects点号根按worktree根扫描安装() {
    let env = Env::new("projects-dot");
    seed_main_file(&env, "pnpm-lock.yaml", "# pnpm");
    let roots = [proj(".")];

    let outcome = env
        .create_with_projects("fix-bug", "点号根 goal", &roots)
        .expect("建域应 Ok");

    assert_eq!(
        env.vcs.installs(),
        vec![(
            env.worktree_root.join("fix-bug"),
            "pnpm install --frozen-lockfile".to_owned()
        )],
        "`.` 解析为 worktree 根"
    );
    assert!(outcome.warnings.is_empty());
}

// ---------------------------------------------------------------------------
// 项目模式：depends_on 拓扑序安装（依赖先于依赖方，同层声明序）
// ---------------------------------------------------------------------------

/// 带依赖声明的项目锚点（`depends_on` 逐字取自入参，保序）。
fn proj_with(root: &str, depends_on: &[&str]) -> BootstrapProject {
    BootstrapProject {
        root: root.to_owned(),
        depends_on: depends_on
            .iter()
            .map(|dependency| (*dependency).to_owned())
            .collect(),
    }
}

/// 安装序观察面：各次安装 cwd 相对 worktree 根的路径（`.` 以 `"."` 表示）——
/// `install_order` 为私有纯函数（兄弟测试模块不可达），产出序列经假件 vcs 的
/// 捕获面间接断言。
fn install_roots(env: &Env, name: &str) -> Vec<String> {
    let worktree = env.worktree_root.join(name);
    env.vcs
        .installs()
        .into_iter()
        .map(|(cwd, _)| {
            let relative = cwd
                .strip_prefix(&worktree)
                .expect("安装 cwd 应在 worktree 内")
                .to_string_lossy()
                .into_owned();
            if relative.is_empty() {
                ".".to_owned()
            } else {
                relative
            }
        })
        .collect()
}

/// 拓扑序（正向）：声明序 `[src-tauri(dep→desktop), desktop]` → 安装序反转
///（依赖先于依赖方），cwd 各为 `<worktree>/<root>`，零警告。
#[test]
fn 拓扑序_依赖先于依赖方且cwd为项目根() {
    let env = Env::new("topo-basic");
    seed_main_path(&env, "packages/desktop/src-tauri/Cargo.lock", "# cargo");
    seed_main_path(&env, "packages/desktop/pnpm-lock.yaml", "# pnpm");
    let projects = [
        proj_with("packages/desktop/src-tauri", &["packages/desktop"]),
        proj("packages/desktop"),
    ];

    let outcome = env
        .create_with_projects("fix-bug", "拓扑序 goal", &projects)
        .expect("建域应 Ok");

    let worktree = env.worktree_root.join("fix-bug");
    assert_eq!(
        env.vcs.installs(),
        vec![
            (
                worktree.join("packages/desktop"),
                "pnpm install --frozen-lockfile".to_owned()
            ),
            (
                worktree.join("packages/desktop/src-tauri"),
                "cargo fetch --locked".to_owned()
            ),
        ],
        "依赖项目先安装，cwd = 各自项目根"
    );
    assert!(
        outcome.warnings.is_empty(),
        "顺利安装零警告，实际 {:?}",
        outcome.warnings
    );
}

/// 无依赖退化（正向）：全无 `depends_on` → 安装序逐字等于声明序（tie-break
/// 为声明序，非字典序）。
#[test]
fn 无依赖退化_安装序等于声明序非字典序() {
    let env = Env::new("topo-flat");
    seed_main_path(&env, "zeta/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "alpha/package-lock.json", "# npm");
    seed_main_path(&env, "mid/Cargo.lock", "# cargo");
    let projects = [proj("zeta"), proj("alpha"), proj("mid")];

    let outcome = env
        .create_with_projects("fix-bug", "退化 goal", &projects)
        .expect("建域应 Ok");

    assert_eq!(
        install_roots(&env, "fix-bug"),
        ["zeta", "alpha", "mid"],
        "无依赖时安装序 = 声明序（非字典序）"
    );
    assert!(outcome.warnings.is_empty());
}

/// 多层链（正向）：`web(dep→api)` / `api(dep→db)` / `db` → 安装序 `db, api, web`
///（后序 DFS 逐层下沉）。
#[test]
fn 多层链依赖_后序dfs逐层下沉() {
    let env = Env::new("topo-chain");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/package-lock.json", "# npm");
    seed_main_path(&env, "db/Cargo.lock", "# cargo");
    let projects = [
        proj_with("web", &["api"]),
        proj_with("api", &["db"]),
        proj("db"),
    ];

    let outcome = env
        .create_with_projects("fix-bug", "多层链 goal", &projects)
        .expect("建域应 Ok");

    assert_eq!(
        install_roots(&env, "fix-bug"),
        ["db", "api", "web"],
        "三层依赖链按后序 DFS 逐层下沉"
    );
    assert!(outcome.warnings.is_empty());
}

/// 入口再导出可见性：入参类型经 `write::` 出口取型（`super::BootstrapProject`
/// 而非 `create::` 私有路径）构造并传入 `create()` → 编译通过且拓扑序生效。
#[test]
fn 出口再导出取型_经write出口构造入参且拓扑序生效() {
    let env = Env::new("topo-reexport");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/package-lock.json", "# npm");
    let projects: Vec<super::BootstrapProject> = vec![
        super::BootstrapProject {
            root: "web".to_owned(),
            depends_on: vec!["api".to_owned()],
        },
        super::BootstrapProject {
            root: "api".to_owned(),
            depends_on: Vec::new(),
        },
    ];

    let outcome = env
        .create_with_projects("fix-bug", "再导出 goal", &projects)
        .expect("建域应 Ok");

    assert_eq!(
        install_roots(&env, "fix-bug"),
        ["api", "web"],
        "写面不区分「未设」与「空」：空 Vec 与显式空数组同处置"
    );
    assert!(outcome.warnings.is_empty());
}

/// 重复 `root`（边界）：同一根出现两次 → 以首次出现为准，第二条元素整体丢弃
///（其 `depends_on` 一并失效）→ `web` 恰安装一次，安装序 `web, api`。
#[test]
fn 重复root_首次出现为准且恰安装一次() {
    let env = Env::new("topo-dup-root");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/package-lock.json", "# npm");
    let projects = [proj("web"), proj_with("web", &["api"]), proj("api")];

    let outcome = env
        .create_with_projects("fix-bug", "重复根 goal", &projects)
        .expect("脏数据零 Err");

    assert_eq!(
        install_roots(&env, "fix-bug"),
        ["web", "api"],
        "重复 root 折叠为首次出现（第二条的 depends_on 一并丢弃）"
    );
    assert!(
        outcome.warnings.is_empty(),
        "重复 root 不产生新增警告，实际 {:?}",
        outcome.warnings
    );
}

/// 依赖项重复与空串（边界）：`depends_on: ["api", "api", ""]` → visited 幂等
/// 收口（`api` 恰一次），空串按悬空跳过，零 `Err`。
#[test]
fn 依赖项重复与空串_幂等收口且空串按悬空跳过() {
    let env = Env::new("topo-dup-dep");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/package-lock.json", "# npm");
    let projects = [proj_with("web", &["api", "api", ""]), proj("api")];

    let outcome = env
        .create_with_projects("fix-bug", "重复依赖 goal", &projects)
        .expect("脏依赖零 Err");

    assert_eq!(
        install_roots(&env, "fix-bug"),
        ["api", "web"],
        "重复依赖项幂等收口，空串按悬空跳过"
    );
    assert_eq!(env.vcs.installs().len(), 2, "每项目恰安装一次");
    assert!(outcome.warnings.is_empty());
}

/// 超大输入（边界）：200 个无依赖项目 + 20 层依赖链 → 线性完成、序确定、
/// 无栈溢出 / 死循环，安装次数 = 项目数。
#[test]
fn 超大输入_200项目加20层链线性完成且序确定() {
    let env = Env::new("topo-large");
    let chain_len = 20;
    let flat_len = 200;
    let mut projects: Vec<BootstrapProject> = (0..chain_len)
        .map(|index| BootstrapProject {
            root: format!("c{index}"),
            depends_on: if index + 1 < chain_len {
                vec![format!("c{}", index + 1)]
            } else {
                Vec::new()
            },
        })
        .collect();
    for index in 0..chain_len {
        seed_main_path(&env, &format!("c{index}/package-lock.json"), "# npm");
    }
    for index in 0..flat_len {
        let root = format!("p{index}");
        seed_main_path(&env, &format!("{root}/package-lock.json"), "# npm");
        projects.push(proj(&root));
    }

    let outcome = env
        .create_with_projects("fix-bug", "超大输入 goal", &projects)
        .expect("建域应 Ok");

    let mut expected: Vec<String> = (0..chain_len)
        .rev()
        .map(|index| format!("c{index}"))
        .collect();
    expected.extend((0..flat_len).map(|index| format!("p{index}")));
    assert_eq!(
        install_roots(&env, "fix-bug"),
        expected,
        "链倒序下沉 + 平铺项目保持声明序"
    );
    assert_eq!(
        env.vcs.installs().len(),
        chain_len + flat_len,
        "每项目恰安装一次，无重复 / 无遗漏"
    );
    assert!(outcome.warnings.is_empty());
}

/// 点号根 + 依赖（边界）：`.` 解析为 worktree 根，安装序仍按拓扑序（保持既有
/// 「点号根」语义）。
#[test]
fn 点号根携带依赖_解析worktree根且序仍为拓扑序() {
    let env = Env::new("topo-dot");
    seed_main_file(&env, "pnpm-lock.yaml", "# root pnpm");
    seed_main_path(&env, "web/package-lock.json", "# npm");
    let projects = [proj_with("web", &["."]), proj(".")];

    let outcome = env
        .create_with_projects("fix-bug", "点号根 goal", &projects)
        .expect("建域应 Ok");

    assert_eq!(
        install_roots(&env, "fix-bug"),
        [".", "web"],
        "`.` 解析为 worktree 根且依赖先于依赖方"
    );
    assert!(outcome.warnings.is_empty());
}

/// 悬空依赖（异常）：`web(dep→ghost)` + `api` → 零 `Err`、两项照常安装、警告
/// 集合与无 `depends_on` 时同形（无「未知依赖」类新增警告）。
#[test]
fn 悬空依赖_零err零新增警告且两项照常安装() {
    let env = Env::new("topo-dangling");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/package-lock.json", "# npm");
    let projects = [proj_with("web", &["ghost"]), proj("api")];

    let outcome = env
        .create_with_projects("fix-bug", "悬空依赖 goal", &projects)
        .expect("悬空依赖零 Err");

    assert_eq!(
        install_roots(&env, "fix-bug"),
        ["web", "api"],
        "悬空依赖跳过，其余按声明序安装"
    );
    assert!(
        outcome.warnings.is_empty(),
        "悬空依赖不产生新增警告，实际 {:?}",
        outcome.warnings
    );
}

/// 自引用（异常）：`web(dep→web)` → 零 `Err`、`web` 恰安装一次、无死循环。
#[test]
fn 自引用_零err且恰安装一次无死循环() {
    let env = Env::new("topo-self");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    let projects = [proj_with("web", &["web"])];

    let outcome = env
        .create_with_projects("fix-bug", "自引用 goal", &projects)
        .expect("自引用零 Err");

    assert_eq!(install_roots(&env, "fix-bug"), ["web"], "自引用恰安装一次");
    assert_eq!(env.vcs.installs().len(), 1);
    assert!(outcome.warnings.is_empty());
}

/// 成环（异常）：声明序 `[web(dep→api), api(dep→web)]` → 零 `Err`、各安装恰
/// 一次、产出确定序 `[api, web]`。
#[test]
fn 成环_零err各一次且产出确定序() {
    let env = Env::new("topo-cycle");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/package-lock.json", "# npm");
    let projects = [proj_with("web", &["api"]), proj_with("api", &["web"])];

    let outcome = env
        .create_with_projects("fix-bug", "成环 goal", &projects)
        .expect("成环零 Err");

    assert_eq!(
        install_roots(&env, "fix-bug"),
        ["api", "web"],
        "成环由 visited 收口，产出确定序（依赖侧先落地）"
    );
    assert_eq!(env.vcs.installs().len(), 2, "各项目恰安装一次");
    assert!(outcome.warnings.is_empty());
}

/// 拓扑序 + 安装失败（异常）：`web(dep→api)` 两项均失败 → 警告按**安装序**
/// 累积（`api` 的失败警告先于 `web`）、仍返回 `Ok`、建档与 worktree 均保留。
#[test]
fn 拓扑序安装失败_警告按安装序累积且不回滚() {
    let env = Env::new("topo-fail");
    seed_main_path(&env, "web/pnpm-lock.yaml", "# pnpm");
    seed_main_path(&env, "api/package-lock.json", "# npm");
    env.vcs.set_install_failure("ERR_BOOT");
    let projects = [proj_with("web", &["api"]), proj("api")];

    let outcome = env
        .create_with_projects("fix-bug", "失败 goal", &projects)
        .expect("bootstrap 失败不回滚");

    assert_eq!(
        outcome.warnings,
        vec![
            "项目根 \"api\"：依赖引导失败（npm ci）: ERR_BOOT".to_owned(),
            "项目根 \"web\"：依赖引导失败（pnpm install --frozen-lockfile）: ERR_BOOT".to_owned(),
        ],
        "警告按安装序累积（依赖方在前）"
    );
    assert!(
        env.store.find_by_id(&env.store.created_id()).is_some(),
        "建档保留（不回滚）"
    );
    assert!(
        env.worktree_root.join("fix-bug").is_dir(),
        "worktree 保留（不回滚）"
    );
}

/// 拓扑序 + 项目根不存在（异常）：`web(dep→api)` 两根本均缺失 → 警告按安装序
/// 出现两条「在执行根内不存在」，零安装。
#[test]
fn 拓扑序项目根不存在_警告按安装序排列且零安装() {
    let env = Env::new("topo-missing");
    let projects = [proj_with("web", &["api"]), proj("api")];

    let outcome = env
        .create_with_projects("fix-bug", "缺失 goal", &projects)
        .expect("建域应 Ok");

    assert!(env.vcs.install_commands().is_empty(), "两根本缺失，零安装");
    assert_eq!(
        outcome.warnings,
        vec![
            "项目根 \"api\"：在执行根内不存在，跳过依赖引导".to_owned(),
            "项目根 \"web\"：在执行根内不存在，跳过依赖引导".to_owned(),
        ],
        "缺失警告按安装序排列（依赖方在前）"
    );
}

// ---------------------------------------------------------------------------
// 异常：补偿链（D3）
// ---------------------------------------------------------------------------

/// add 失败补偿（按 id）：`delete_change_record` 删本次自插行（id = 本次铸出
/// 值，恰一次）+ 尽力 `delete_branch`（捕获调用）；Err 呈现失败与补偿事实。
#[test]
fn add失败补偿删建档且尽力删分支() {
    let env = Env::new("add-fail");
    env.vcs.set_add_fault("worktree add 模拟失败");

    let error = env
        .create("fix-bug", "add 失败 goal")
        .expect_err("add 失败应 Err");

    assert!(error.contains("补偿"), "Err 呈现补偿事实，实际: {error}");
    let id = env.store.created_id();
    assert_eq!(
        env.store.deletes.lock().expect("删除锁不可中毒").as_slice(),
        [id.as_str()],
        "补偿删除按本次铸出 id 恰调用一次"
    );
    assert!(env.store.find_by_id(&id).is_none(), "本次自插行零残留");
    assert_eq!(env.store.record_count(), 0, "list 空（db 零残留）");
    assert!(
        env.vcs
            .calls()
            .iter()
            .any(|call| call == "delete_branch:change/fix-bug"),
        "尽力删分支下发"
    );
}

/// 树写出失败补偿（补偿成功面）：worktree 内路径分量预置文件占位（真实 fs 注入）
/// → `remove_worktree` → `delete_branch` → 删建档 全链下发（调用序）；Err 呈现
/// 补偿完成 + 双源对账 hint（落位根绝对路径逐字在场，`git worktree list` ∧
/// 直接检查），不含单源祈使指引。
#[test]
fn 树写出失败补偿链全下发且残留指引() {
    let env = Env::new("fs-fail-compensate");
    // 占位打在主仓 openspec/changes 路径分量（假件 add 镜像检出携入 worktree，
    // 目录树 / explore.md 写出段真实失败；直接预置 worktree 会先撞前置⑦）
    fs::create_dir_all(env.root.join("openspec")).expect("预置主仓域根失败");
    fs::write(
        env.root.join("openspec").join("changes"),
        "changes_root 文件占位",
    )
    .expect("预置占位失败");
    let root_str = env.worktree_root.display().to_string();

    let error = env
        .create("fix-bug", "补偿路径 goal")
        .expect_err("fs 半边失败应 Err");

    assert!(
        error.contains("补偿"),
        "Err 呈现补偿回滚事实，实际: {error}"
    );
    let calls = env.vcs.calls();
    let remove_at = calls
        .iter()
        .position(|call| call == "remove_worktree")
        .expect("remove_worktree 下发");
    let branch_at = calls
        .iter()
        .position(|call| call == "delete_branch:change/fix-bug")
        .expect("delete_branch 下发");
    assert!(
        remove_at < branch_at,
        "补偿链调用序：remove → delete_branch"
    );
    let id = env.store.created_id();
    assert_eq!(
        env.store.deletes.lock().expect("删除锁不可中毒").as_slice(),
        [id.as_str()],
        "删建档按本次铸出 id 恰一次"
    );
    assert!(
        env.store.find_by_id(&id).is_none(),
        "补偿删除本次自插行（按 id 零残留）"
    );
    assert_eq!(env.store.record_count(), 0, "list 空（db 零残留）");
    assert!(
        error.contains("已补偿回收"),
        "Err 呈现补偿成功事实，实际: {error}"
    );
    assert!(
        error.contains("git worktree list") && error.contains("直接检查"),
        "Err 携双源对账 hint，实际: {error}"
    );
    assert!(
        error.contains(&root_str),
        "hint 携落位根绝对路径（display 逐字），实际: {error}"
    );
    assert!(
        !error.contains("请手动执行"),
        "MUST NOT 含单源祈使指引，实际: {error}"
    );
}

/// 补偿链再失败（双故障角落）：链中 `remove_worktree` 注入 Err → Err 呈现
/// 残留对象（worktree 路径 / branch 名）+ 双源对账 hint + branch 侧兜底指引，
/// 不静默自愈；`add_worktree` 支同理（残留 branch + 记因）。
#[test]
fn 补偿链再失败呈现残留对象与指引() {
    let env = Env::new("compensate-fault");
    env.vcs.set_remove_fault("worktree remove 模拟失败");
    fs::create_dir_all(env.root.join("openspec")).expect("预置主仓域根失败");
    fs::write(
        env.root.join("openspec").join("changes"),
        "changes_root 文件占位",
    )
    .expect("预置占位失败");
    let root_str = env.worktree_root.display().to_string();
    let target_str = env.worktree_root.join("orphan-name").display().to_string();

    let error = env
        .create("orphan-name", "双故障角落 goal")
        .expect_err("补偿再失败应 Err");

    assert!(
        error.contains(&format!("worktree {target_str}: worktree remove 模拟失败")),
        "Err 残留行按「worktree <路径>: <注入原文>」逐字呈现，实际: {error}"
    );
    assert!(
        error.contains(&root_str) && error.contains(&target_str),
        "落位根与残留 worktree 路径同现（各自正确、不串位），实际: {error}"
    );
    assert!(
        error.contains("git worktree list") && error.contains("直接检查"),
        "Err 携双源对账 hint，实际: {error}"
    );
    assert!(
        error.contains("branch 侧仍以 git branch -D change/orphan-name 清理"),
        "Err 携 branch 侧兜底指引，实际: {error}"
    );
    assert!(
        !error.contains("请手动执行"),
        "MUST NOT 含单源祈使指引，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 1, "建档先行恰好一次");
    let id = env.store.created_id();
    assert!(
        env.store.find_by_id(&id).is_none(),
        "记录半边回收（双故障角落仅 worktree 残留）"
    );

    // add 失败补偿链的双故障角落：add 注入 Err + delete_branch 再注入 Err →
    // Err 呈现残留对象（branch 名）与双源对账指引（不静默自愈）
    let env = Env::new("add-fail-branch-residual");
    env.vcs.set_add_fault("worktree add 模拟失败");
    env.vcs.set_delete_branch_fault("branch -D 模拟失败");
    let root_str = env.worktree_root.display().to_string();

    let error = env
        .create("add-orphan", "add 双故障 goal")
        .expect_err("add 失败 + 删分支再失败应 Err");

    assert!(
        error.contains("worktree add 模拟失败"),
        "Err 呈现原始失败记因，实际: {error}"
    );
    assert!(
        error.contains("branch \"change/add-orphan\"") && error.contains("branch -D 模拟失败"),
        "Err 呈现 branch 残留对象与记因，实际: {error}"
    );
    assert!(
        error.contains("git worktree list") && error.contains("直接检查"),
        "Err 携双源对账 hint（不静默自愈），实际: {error}"
    );
    assert!(
        error.contains(&root_str),
        "hint 携落位根绝对路径，实际: {error}"
    );
    assert!(
        error.contains("branch 侧仍以 git branch -D change/add-orphan 清理"),
        "Err 携 branch 侧兜底指引，实际: {error}"
    );
    assert!(
        !error.contains("请手动执行"),
        "MUST NOT 含单源祈使指引，实际: {error}"
    );
    // 建档补偿删除照常下发（记录半边回收不受 branch 残留影响）
    let id = env.store.created_id();
    assert_eq!(
        env.store.deletes.lock().expect("删除锁不可中毒").as_slice(),
        [id.as_str()],
        "建档补偿删除按 id 恰一次（记录半边回收）"
    );

    // 三故障角落：建档补偿删除再失败 → 残留记因随 Err 呈现（残留行按 id
    // 在场——随下次同 id 铸出撞号的防御拒绝显式暴露）
    let env = Env::new("add-fail-record-residual");
    env.vcs.set_add_fault("worktree add 模拟失败");
    env.store
        .set_delete_fault(StoreFault::Db("建档删除模拟失败".to_owned()));

    let error = env
        .create("record-orphan", "三故障 goal")
        .expect_err("建档补偿删除失败应 Err");

    let id = env.store.created_id();
    assert!(
        error.contains(&format!("建档记录 \"{id}\"")) && error.contains("建档删除模拟失败"),
        "Err 呈现建档残留对象（按 id 指认）与记因，实际: {error}"
    );
    assert!(
        env.store.find_by_id(&id).is_some(),
        "补偿再失败：残留行仍在场（不静默自愈）"
    );
    assert_eq!(env.store.record_count(), 1, "恰一条残留行");
    assert!(
        error.contains("git worktree list"),
        "手动清理指引在场，实际: {error}"
    );
}

/// 正向（`(None, None)` 支零改动判据）：`add_worktree` 注入 `Err` 且无其它故障
/// → Err 为「…（建档与分支已补偿回收）」；文案单点不外溢到本支（无双源 hint、
/// 无残留清单）。
#[test]
fn add失败补偿全成功_err呈已补偿回收且不含双源hint() {
    let env = Env::new("add-fail-clean");
    env.vcs.set_add_fault("worktree add 模拟失败");

    let error = env
        .create("fix-bug", "add 失败 goal")
        .expect_err("add 失败应 Err");

    assert!(
        error.contains("（建档与分支已补偿回收）"),
        "Err 呈现补偿完成事实，实际: {error}"
    );
    assert!(
        !error.contains("worktree 残留对账须双源"),
        "双源 hint 不外溢到 (None, None) 支，实际: {error}"
    );
    assert!(
        !error.contains("残留"),
        "无残留清单（两半边均已回收），实际: {error}"
    );
    assert_eq!(env.store.record_count(), 0, "db 零残留");
}

/// 正向（新增形参面）：两处补偿出口的 hint 均携**落位根父锚**的 `display()`
/// 逐字（而非 `worktree_root.join(<name>)` 的目标目录路径）；tree 支文案中
/// 「落位根」与「残留行里的 worktree 路径」两个路径同时在场且各自正确。
#[test]
fn 补偿出口形参_落位根为父锚而非目标目录() {
    // tree 支（残留面）
    let env = Env::new("param-tree");
    env.vcs.set_remove_fault("worktree remove 模拟失败");
    fs::create_dir_all(env.root.join("openspec")).expect("预置主仓域根失败");
    fs::write(
        env.root.join("openspec").join("changes"),
        "changes_root 文件占位",
    )
    .expect("预置占位失败");
    let root_str = env.worktree_root.display().to_string();
    let target_str = env.worktree_root.join("orphan-name").display().to_string();

    let error = env
        .create("orphan-name", "形参面 goal")
        .expect_err("补偿再失败应 Err");

    assert!(
        error.contains(&format!("落位根 {root_str} 下的子目录")),
        "hint 携落位根父锚 display() 逐字，实际: {error}"
    );
    assert!(
        !error.contains(&format!("落位根 {target_str} 下的子目录")),
        "hint 落位根不是目标目录（形参未串位），实际: {error}"
    );
    assert!(
        error.contains(&format!("worktree {target_str}: ")),
        "残留行 worktree 路径独立在场，实际: {error}"
    );

    // add 支（残留面）：hint 同样为父锚，目标目录路径零出现
    let env = Env::new("param-add");
    env.vcs.set_add_fault("worktree add 模拟失败");
    env.vcs.set_delete_branch_fault("branch -D 模拟失败");
    let root_str = env.worktree_root.display().to_string();
    let target_str = env.worktree_root.join("add-orphan").display().to_string();

    let error = env
        .create("add-orphan", "add 形参面 goal")
        .expect_err("add 失败 + 删分支失败应 Err");

    assert!(
        error.contains(&format!("落位根 {root_str} 下的子目录")),
        "add 支 hint 携落位根父锚 display() 逐字，实际: {error}"
    );
    assert!(
        !error.contains(&target_str),
        "add 支零残留目标目录路径（形参为父锚），实际: {error}"
    );
}

/// 边界（DD8 单行与零包转）：前置⑨ 与两处补偿三出口的 `Err` 均零换行；落位根
/// 路径按 `display()` 原样插入（Windows 反斜杠不转义、分隔符不归一）。
#[test]
fn 双源文案三出口皆单行且落位根零转义() {
    // 出口一：前置⑨（8 个直属子目录越门槛）
    let threshold_env = Env::new("oneline-threshold");
    fs::create_dir_all(&threshold_env.worktree_root).expect("建 worktree_root 失败");
    for index in 0..8 {
        fs::create_dir_all(
            threshold_env
                .worktree_root
                .join(format!("residual-{index}")),
        )
        .expect("预置残留目录失败");
    }
    let threshold_error = threshold_env
        .create("oneline-threshold", "单行 goal")
        .expect_err("越门槛应 Err");

    // 出口二：补偿成功面（树写出真实失败 + 全链补偿成功）
    let clean_env = Env::new("oneline-clean");
    fs::create_dir_all(clean_env.root.join("openspec")).expect("预置主仓域根失败");
    fs::write(
        clean_env.root.join("openspec").join("changes"),
        "changes_root 文件占位",
    )
    .expect("预置占位失败");
    let clean_error = clean_env
        .create("fix-bug", "单行 goal")
        .expect_err("fs 半边失败应 Err");

    // 出口三：补偿残留面（remove_worktree 注入 Err）
    let residual_env = Env::new("oneline-residual");
    residual_env
        .vcs
        .set_remove_fault("worktree remove 模拟失败");
    fs::create_dir_all(residual_env.root.join("openspec")).expect("预置主仓域根失败");
    fs::write(
        residual_env.root.join("openspec").join("changes"),
        "changes_root 文件占位",
    )
    .expect("预置占位失败");
    let residual_error = residual_env
        .create("orphan-name", "单行 goal")
        .expect_err("补偿再失败应 Err");

    for (label, error) in [
        ("前置⑨", &threshold_error),
        ("补偿成功面", &clean_error),
        ("补偿残留面", &residual_error),
    ] {
        assert!(
            !error.contains('\n'),
            "{label} 文案单行（零换行——UI / 日志渲染面）: {error}"
        );
    }
    assert!(
        threshold_error.contains(&threshold_env.worktree_root.display().to_string()),
        "落位根按 display() 原样插入（反斜杠不转义）: {threshold_error}"
    );
    assert!(
        clean_error.contains(&clean_env.worktree_root.display().to_string()),
        "补偿成功面落位根同样原样插入: {clean_error}"
    );
}

/// 边界（组合行：残留不改变链次序）：`remove_worktree` 注入 `Err` 时
/// `delete_branch` 与建档删除仍照常下发；`residuals` 空与非空两支互斥（同一次
/// 调用不可能既报残留又报已补偿）。
#[test]
fn 残留不改变链次序_删分支与建档删除仍照常下发() {
    let env = Env::new("residual-order");
    env.vcs.set_remove_fault("worktree remove 模拟失败");
    fs::create_dir_all(env.root.join("openspec")).expect("预置主仓域根失败");
    fs::write(
        env.root.join("openspec").join("changes"),
        "changes_root 文件占位",
    )
    .expect("预置占位失败");

    let error = env
        .create("orphan-name", "链次序 goal")
        .expect_err("补偿再失败应 Err");

    let calls = env.vcs.calls();
    assert!(
        calls.iter().any(|call| call == "remove_worktree"),
        "链首 remove_worktree 下发: {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|call| call == "delete_branch:change/orphan-name"),
        "remove 残留不阻断删分支下发: {calls:?}"
    );
    let id = env.store.created_id();
    assert_eq!(
        env.store.deletes.lock().expect("删除锁不可中毒").as_slice(),
        [id.as_str()],
        "remove 残留不阻断建档补偿删除（按 id 恰一次）"
    );
    assert!(
        error.contains("残留") && !error.contains("已补偿回收"),
        "残留支与已补偿支互斥，实际: {error}"
    );
}

// ---------------------------------------------------------------------------
// 边界：created 出线 + 清单立即可见（组合行）+ CreateOutcome 线形状
// ---------------------------------------------------------------------------

/// CreateOutcome.created 取 db created_at 日期（UTC 日界口径）；成功后立即
/// 经 list_changes 可见（db 归组：主仓目录缺席不入未知组、不丢弃）。
#[test]
fn created出线且成功立即可见() {
    let env = Env::new("visible");
    let before = utc_date_today();

    let outcome = env
        .create("combo-visible", "组合用例 goal")
        .expect("create 应 Ok");
    let after = utc_date_today();

    assert_today(&outcome.created, &before, &after);

    // 清单立即可见（db status 权威归组——主仓目录缺席仍在 active 组）
    let list = list_changes(&env.store);
    assert_eq!(list.active.len(), 1, "active 恰一条");
    assert_eq!(
        list.active[0].id, outcome.id,
        "列表条目 id = 本次铸出身份锚（db 单源直供）"
    );
    assert_eq!(list.active[0].name, "combo-visible", "列表条目裸名");
    assert_eq!(list.active[0].status, Some(ChangeStatus::Active));
    assert_eq!(
        list.active[0].created, outcome.created,
        "created 透传建档日期（列表投影 = 建档出线值）"
    );
}

/// CreateOutcome serde 线形状：序列化恰 `id` / `name` / `created` /
/// `worktree` / `execRoot` / `warnings` 六键（camelCase；`id` 为本次铸出的身份
/// 锚、声明序首字段）；`warnings` 空清单出线 `[]` 非 null、`execRoot` 为空时
/// 出线 `null`（键恒在场，不省键）。
/// 注：serde_json 对象为键自然序（BTreeMap）——声明序锚以 Debug 字段序断言
/// （DTO 声明序即 specta / TS 绑定字段序，签面逐字段断言归 bindings 守卫节）。
#[test]
fn create_outcome_serde线形状恰六键且id首字段() {
    let outcome = CreateOutcome {
        id: "0198f7a0-0000-7000-8000-000000000000".to_owned(),
        name: "fix-bug".to_owned(),
        created: "2026-10-02".to_owned(),
        worktree: "C:\\home\\.dev-team\\worktrees\\seg\\fix-bug".to_owned(),
        exec_root: None,
        warnings: Vec::new(),
    };

    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let object = value.as_object().expect("序列化为对象");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["created", "execRoot", "id", "name", "warnings", "worktree"],
        "恰六键面（id 为身份锚、worktree 与 execRoot 为刻意出线的两根）"
    );
    assert_eq!(object["id"], "0198f7a0-0000-7000-8000-000000000000");
    assert_eq!(object["name"], "fix-bug");
    assert_eq!(object["created"], "2026-10-02");
    assert_eq!(
        object["warnings"],
        serde_json::json!([]),
        "warnings 空清单出线 [] 非 null"
    );
    assert_eq!(
        object["execRoot"],
        serde_json::json!(null),
        "execRoot 键恒在场（None → null，不省键）"
    );

    // id 首字段（声明序）：字段序即 TS 绑定字段序（id / name / created /
    // worktree / execRoot / warnings 六字段面）
    let debug = format!("{outcome:?}");
    assert!(
        debug.starts_with("CreateOutcome { id: "),
        "id 为声明序首字段，实际: {debug}"
    );
}

// ---------------------------------------------------------------------------
// 前置⑧：active 建档数上限（worktree-cleanup-and-active-cap / AC-1 / AC-3）：
// 硬编码上限（源码常量单点）全 IO 前置——拒绝面零建档零目录零 worktree 零 vcs
// 调用；归档记录不计入
// ---------------------------------------------------------------------------

/// 前置⑧ 拒绝（异常）：db 预置 5 条 `status=active` → 显式 `Err`；文案含
/// 「当前 5」「上限 5」与「归档或手动清理」引导（D6 单点文案）；拒绝面零
/// 副作用（store 零建档、vcs 零调用——上限道先于 git 探测）。
#[test]
fn active上限拒绝_5条active时err文案含计数与引导且零副作用() {
    let env = Env::new("cap-reject");
    for index in 0..5 {
        env.store.seed(
            &format!("0198f7a0-0000-7000-8000-0000000000c{index}"),
            &format!("cap-filled-{index}"),
            ChangeStatus::Active,
        );
    }

    let error = env
        .create("cap-sixth", "第 6 个 change goal")
        .expect_err("5 条 active 已达上限应 Err");

    assert!(
        error.contains("active change 数已达上限"),
        "错误归因 active 上限道，实际: {error}"
    );
    assert!(
        error.contains("当前 5") && error.contains("上限 5"),
        "文案含当前计数 5 与上限 5（D6 单点文案）: {error}"
    );
    assert!(
        error.contains("归档或手动清理"),
        "文案携「归档或手动清理」引导: {error}"
    );
    // 拒绝面零副作用（AC-1 / AC-3：全 IO 前置）
    assert_eq!(env.store.create_call_count(), 0, "db 零建档调用");
    assert_eq!(env.store.record_count(), 5, "库内仅种子五行零变动");
    assert!(
        env.active_dir_names().is_empty(),
        "主仓 active 树零目录，实际: {:?}",
        env.active_dir_names()
    );
    assert!(
        dir_names(&env.worktree_root).is_empty(),
        "worktree_root 零新目录"
    );
    assert!(
        env.vcs.calls().is_empty(),
        "vcs 零调用（上限道先于 git 探测），实际: {:?}",
        env.vcs.calls()
    );
}

/// 前置⑧ 边界（正向）：db 4 条 active → 第 5 个创建成功（`CreateOutcome` 正常
/// 出线，建域四段照常；上限为「达到即拒」——4 < 5 仍可创建）。
#[test]
fn active上限下第5个创建成功_4条active时() {
    let env = Env::new("cap-under");
    for index in 0..4 {
        env.store.seed(
            &format!("0198f7a0-0000-7000-8000-0000000000d{index}"),
            &format!("cap-under-{index}"),
            ChangeStatus::Active,
        );
    }

    let outcome = env
        .create("cap-fifth", "第 5 个 change goal")
        .expect("4 条 active 时第 5 个应创建成功");

    assert!(
        !outcome.id.is_empty(),
        "CreateOutcome 正常出线（身份锚在案）"
    );
    assert_eq!(outcome.name, "cap-fifth");
    assert_eq!(env.store.create_call_count(), 1, "db 恰一次建档");
    assert_eq!(env.store.record_count(), 5, "库内 4 种子 + 1 新建档");
    assert!(
        env.change_dir("cap-fifth").join("explore.md").is_file(),
        "建域四段照常（worktree 内目录树 + explore.md）"
    );
}

/// 前置⑧ 边界：归档记录不计入——5 条 `status=archived` + 4 条 active → 创建
/// 成功（计数仅 status=active 面）。
#[test]
fn 归档记录不计入_5条archived加4条active创建成功() {
    let env = Env::new("cap-archived-free");
    for index in 0..5 {
        env.store.seed(
            &format!("0198f7a0-0000-7000-8000-0000000000e{index}"),
            &format!("cap-archived-{index}"),
            ChangeStatus::Archived,
        );
    }
    for index in 0..4 {
        env.store.seed(
            &format!("0198f7a0-0000-7000-8000-0000000000f{index}"),
            &format!("cap-live-{index}"),
            ChangeStatus::Active,
        );
    }

    let outcome = env
        .create("cap-archived-free-new", "归档不占额度 goal")
        .expect("归档记录不计入，第 5 个 active 应创建成功");

    assert!(!outcome.id.is_empty());
    let records = env.store.records();
    assert_eq!(records.len(), 10, "5 归档 + 4 active + 1 新建档");
    assert_eq!(
        records
            .iter()
            .filter(|record| record.status == ChangeStatus::Active)
            .count(),
        5,
        "active 恰 5 条（上限道只在 create 时刻生效，创建后达上限合法）"
    );
}

// ---------------------------------------------------------------------------
// 前置⑨：worktree 磁盘保底门槛（AC-2）：门槛 = 上限 + SLACK = 7，fs 扫
// `worktree_root` 直属子目录数（仅 `file_type().is_dir()` 计入）——db 与磁盘
// 漂移防线（孤儿 / 手动残留不占 active 额度但占磁盘）
// ---------------------------------------------------------------------------

/// 前置⑨ 拒绝（异常）：`worktree_root` 直属子目录 8 个 → 显式 `Err`；文案含
/// 目录数 8、门槛 7、落位根绝对路径与双源对账引导（`git worktree list` +
/// 直接检查落位根子目录）；拒绝面零副作用。
#[test]
fn worktree门槛拒绝_8个直属子目录err文案含双源对账引导() {
    let env = Env::new("threshold-reject");
    fs::create_dir_all(&env.worktree_root).expect("建 worktree_root 失败");
    for index in 0..8 {
        fs::create_dir_all(env.worktree_root.join(format!("residual-{index}")))
            .expect("预置残留目录失败");
    }
    let root_str = env.worktree_root.display().to_string();

    let error = env
        .create("threshold-next", "门槛道 goal")
        .expect_err("8 个直属子目录越门槛应 Err");

    assert!(
        error.contains("worktree 目录数已达保底门槛"),
        "错误归因磁盘保底门槛道，实际: {error}"
    );
    assert!(
        error.contains("当前 8") && error.contains("门槛 7"),
        "文案含目录数 8 与门槛 7（严格大于才拒的派生门槛）: {error}"
    );
    assert!(
        error.contains(&root_str),
        "文案携落位根绝对路径（计数源逐字给出），实际: {error}"
    );
    assert!(
        error.contains("git worktree list"),
        "文案携「git worktree list」对账引导: {error}"
    );
    assert!(
        error.contains("直接检查"),
        "文案携「直接检查落位根子目录」第二源: {error}"
    );
    assert!(
        error.contains("清理后再创建"),
        "文案携动作收尾（清理后再创建）: {error}"
    );
    assert!(
        !error.contains("请手动执行") && !error.contains("git worktree remove --force"),
        "MUST NOT 含单源手动命令祈使指引: {error}"
    );
    // 拒绝面零副作用
    assert_eq!(env.store.create_call_count(), 0, "db 零建档调用");
    assert_eq!(
        env.vcs.calls(),
        vec!["placement"],
        "仅落位派生一次（门槛道先于 probe / add）"
    );
}

/// 异常（双源形态横向判据，spec Scenario「残留指引双源」）：前置⑨ 出口的两源
/// 措辞同时在场（`git worktree list` ∧ `直接检查`），且不以 git 单源祈使句收尾
/// （无 `请以 … 对账并手动清理`）。
#[test]
fn worktree门槛拒绝文案双源形态_两源俱在且无单源祈使收尾() {
    let env = Env::new("threshold-dual-source");
    fs::create_dir_all(&env.worktree_root).expect("建 worktree_root 失败");
    for index in 0..8 {
        fs::create_dir_all(env.worktree_root.join(format!("residual-{index}")))
            .expect("预置残留目录失败");
    }

    let error = env
        .create("dual-source", "双源判据 goal")
        .expect_err("8 个直属子目录越门槛应 Err");

    assert!(
        error.contains("git worktree list") && error.contains("直接检查"),
        "两源措辞同时在场: {error}"
    );
    assert!(
        !error.contains("对账并手动清理"),
        "MUST NOT 以 git 单源的祈使句收尾: {error}"
    );
    assert!(
        error.contains("不在 git 登记面"),
        "双源的存在理由在案（残留不在 git 登记面）: {error}"
    );
}

/// 前置⑨ 正向：7 个直属子目录（恰为门槛值）→ 照常创建（严格大于才拒）。
#[test]
fn worktree门槛恰7个目录照常创建() {
    let env = Env::new("threshold-at");
    fs::create_dir_all(&env.worktree_root).expect("建 worktree_root 失败");
    for index in 0..7 {
        fs::create_dir_all(env.worktree_root.join(format!("kept-{index}")))
            .expect("预置在途目录失败");
    }

    let outcome = env
        .create("threshold-ok", "门槛值 goal")
        .expect("恰 7 个目录（== 门槛）应照常创建");

    assert!(!outcome.id.is_empty(), "CreateOutcome 正常出线");
    assert_eq!(
        dir_names(&env.worktree_root).len(),
        8,
        "创建后 worktree_root 共 8 目录（7 预置 + 1 新建域）"
    );
}

/// 前置⑨ 边界：`worktree_root` 不存在（NotFound）→ 计 0 通过（尚无 worktree
/// 面）；目录内普通文件条目不计入（仅 `file_type().is_dir()`——7 目录 + 3 文件
/// 仍照常创建）。
#[test]
fn worktree门槛_目录缺席计零通过且普通文件不计入() {
    // 形态一：worktree_root 不存在（Env::new 后即缺席）→ 计 0 → 创建成功
    let absent = Env::new("threshold-absent");
    assert!(
        !absent.worktree_root.exists(),
        "前置：worktree_root 尚不存在"
    );
    absent
        .create("absent-threshold", "尚无 worktree 面 goal")
        .expect("worktree_root 缺席应计 0 通过");

    // 形态二：7 目录 + 3 普通文件 → 仅目录计入（7 == 门槛）→ 照常创建
    let files = Env::new("threshold-files");
    fs::create_dir_all(&files.worktree_root).expect("建 worktree_root 失败");
    for index in 0..7 {
        fs::create_dir_all(files.worktree_root.join(format!("kept-{index}")))
            .expect("预置目录失败");
    }
    for index in 0..3 {
        fs::write(
            files.worktree_root.join(format!("loose-{index}.txt")),
            "散落文件\n",
        )
        .expect("预置普通文件失败");
    }

    files
        .create("files-not-counted", "文件不计入 goal")
        .expect("普通文件条目不计入，7 目录应照常创建");
}

/// 前置⑨ 异常：`fs::read_dir` 非 NotFound 读错误（worktree_root 为普通文件
/// 形态）→ 显式 `Err`「worktree 目录扫描失败」（不静默计 0、不伪装通过），
/// 仅落位派生一次 vcs 调用。
#[test]
fn worktree门槛_非notfound读错误显式err() {
    let env = Env::new("threshold-io-err");
    // worktree_root 位点为普通文件（父目录 temp_dir 已在）→ read_dir 产生
    // 非 NotFound 读错误（ENOTDIR 面——误扫成 0 会绕过门槛，故显式 Err）
    fs::write(&env.worktree_root, "not a directory\n").expect("预置普通文件失败");
    assert!(
        env.worktree_root.is_file(),
        "前置：worktree_root 为普通文件形态"
    );

    let error = env
        .create("threshold-io", "读错误 goal")
        .expect_err("非 NotFound 读错误应显式 Err");

    assert!(
        error.contains("worktree 目录扫描失败"),
        "错误归因扫描失败（不静默计 0），实际: {error}"
    );
    assert_eq!(
        env.vcs.calls(),
        vec!["placement"],
        "仅落位派生一次（读错误道先于 probe / add）"
    );
}

/// 前置⑨ 边界：db 与磁盘漂移场景——3 条 active（各携在盘 worktree）+ 5 个
/// 残留目录 = 磁盘 8 目录 > 门槛 7 → 前置⑨拦（孤儿 / 手动残留防线，AC-2）。
#[test]
fn db与磁盘漂移_3active加5残留合计8目录被门槛拦() {
    let env = Env::new("threshold-drift");
    // 3 条 active（在盘 worktree 目录在场——create 建域的正常残留形态）
    for index in 0..3 {
        env.store.seed(
            &format!("0198f7a0-0000-7000-8000-00000000010{index}"),
            &format!("drift-live-{index}"),
            ChangeStatus::Active,
        );
        fs::create_dir_all(env.worktree_root.join(format!("drift-live-{index}")))
            .expect("预置 active worktree 目录失败");
    }
    // 5 个孤儿 / 手动残留目录（不占 active 额度但占磁盘）
    for index in 0..5 {
        fs::create_dir_all(env.worktree_root.join(format!("orphan-{index}")))
            .expect("预置残留目录失败");
    }

    let error = env
        .create("drift-next", "漂移场景 goal")
        .expect_err("3 active + 5 残留 = 磁盘 8 目录应被门槛拦");

    assert!(
        error.contains("worktree 目录数已达保底门槛")
            && error.contains("当前 8")
            && error.contains("门槛 7"),
        "前置⑨拦（漂移防线——active 计数 3 < 上限 5 未拦）: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "db 零建档调用");
}

// ---------------------------------------------------------------------------
// 前置序：⑧⑨ 插在前置④（同名 active 查重）之后、前置⑤（git 探测）之前
//（AC-1 / AC-3 全 IO 前置的可辨面）
// ---------------------------------------------------------------------------

/// 前置序（边界）：同名 active 场景仍先④拒——错误文案为同名建档道而非上限道
///（即使 active 计数已达上限）。
#[test]
fn 前置序_同名active先于上限道拒绝() {
    let env = Env::new("order-same-name");
    for index in 0..5 {
        let name = if index == 0 {
            "dupe"
        } else {
            &format!("order-fill-{index}")
        };
        env.store.seed(
            &format!("0198f7a0-0000-7000-8000-00000000011{index}"),
            name,
            ChangeStatus::Active,
        );
    }

    let error = env
        .create("dupe", "同名 goal")
        .expect_err("同名 active 应 Err");

    assert!(
        error.contains("已存在同名建档记录"),
        "先④拒（同名建档道文案），实际: {error}"
    );
    assert!(
        !error.contains("active change 数已达上限"),
        "同名道先于上限道（错误可辨不串道），实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "db 零建档调用");
    assert!(env.vcs.calls().is_empty(), "vcs 零调用");
}

/// 前置序（边界）：active 计数 5 且磁盘目录超门槛 → 错误文案为 active 上限道
///（先⑧后⑨可辨——两道同越时归因不混淆）；vcs 零调用（⑧⑨ 均先于 git 探测⑤）。
#[test]
fn 前置序_active上限道先于磁盘门槛道() {
    let env = Env::new("order-cap-then-threshold");
    for index in 0..5 {
        env.store.seed(
            &format!("0198f7a0-0000-7000-8000-00000000012{index}"),
            &format!("order-cap-{index}"),
            ChangeStatus::Active,
        );
    }
    fs::create_dir_all(&env.worktree_root).expect("建 worktree_root 失败");
    for index in 0..8 {
        fs::create_dir_all(env.worktree_root.join(format!("residual-{index}")))
            .expect("预置残留目录失败");
    }

    let error = env
        .create("order-next", "双道同越 goal")
        .expect_err("两道同越应 Err");

    assert!(
        error.contains("active change 数已达上限"),
        "先⑧后⑨（active 上限道文案），实际: {error}"
    );
    assert!(
        !error.contains("保底门槛"),
        "⑨ 不可先于 ⑧（归因可辨），实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "db 零建档调用");
    assert!(
        env.vcs.calls().is_empty(),
        "⑧⑨ 均先于 git 探测⑤（vcs 零调用），实际: {:?}",
        env.vcs.calls()
    );
}

// ---------------------------------------------------------------------------
// 落位 + 执行根派生：`WorktreePort::placement` / `tracked_at_head` 的契约面与
// create 消费面（根 workspace `rel = ""` 与子目录 workspace 两形态）
// ---------------------------------------------------------------------------

/// 子目录 workspace 夹具：主仓内预置 `rel` 子树（假件 `add_worktree` 镜像检
/// 出时一并落进 worktree，执行根由此在场）。
fn seed_subdir(env: &Env, rel: &str) {
    seed_main_path(env, &format!("{rel}/keep.txt"), "子目录 workspace 夹具\n");
}

/// 超长偏移（> 1000 字符、多层）：逐段 40 字符、30 段。
fn long_rel() -> String {
    let segments: Vec<String> = (0..30)
        .map(|index| format!("layer{index:02}").repeat(5))
        .collect();
    segments.join("/")
}

/// placement（正向）：假件返 `root = <tmp>/.worktrees/repo`、`rel =
/// "packages/desktop"` → host = `root.join(name)`、exec root =
/// `host.join(rel)`，两处取值与假件值逐字一致。
#[test]
fn placement_非空rel_host与执行根两值逐字来自假件() {
    let env = Env::new("placement-subdir");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");

    let outcome = env
        .create("fix-bug", "子目录 workspace goal")
        .expect("建域应 Ok");

    let host = env.worktree_root.join("fix-bug");
    let exec_root = host.join("packages/desktop");
    assert_eq!(
        outcome.worktree,
        host.to_string_lossy(),
        "worktree 出线 = 落位父锚 join change 名（host）"
    );
    assert_eq!(
        outcome.exec_root.as_deref(),
        Some(exec_root.to_string_lossy().as_ref()),
        "执行根 = host join rel（逐字）"
    );
    assert!(exec_root.is_dir(), "执行根随检出落在盘上");
}

/// placement（边界）：`rel = ""` → 执行根派生抽化为 host——记录
/// `exec_root = None`、`CreateOutcome.execRoot = None`。
#[test]
fn placement_rel空串_执行根抽化为host且exec_root列不写() {
    let env = Env::new("placement-root");

    let outcome = env
        .create("fix-bug", "根 workspace goal")
        .expect("建域应 Ok");

    assert_eq!(outcome.exec_root, None, "根 workspace 出线 execRoot = null");
    assert_eq!(
        env.store.created_record().exec_root,
        None,
        "db 记录 exec_root 列不写（= 与 worktree 同值）"
    );
    assert_eq!(
        outcome.worktree,
        env.worktree_root.join("fix-bug").to_string_lossy(),
        "worktree 串与根 workspace 今日形态逐字同"
    );
}

/// placement（边界）：`rel` 单段（`"client"`）与多段（`"client/app/pkg"`）两
/// 形态 → 执行根派生同解（host + `/` + rel），无尾分隔符残留。
#[test]
fn placement_rel单段与多段_执行根派生同解且无尾分隔符() {
    for rel in ["client", "client/app/pkg"] {
        let env = Env::new(&format!("placement-seg-{}", rel.replace('/', "-")));
        env.vcs.set_rel(rel);
        seed_subdir(&env, rel);

        let outcome = env
            .create("fix-bug", "分段 goal")
            .unwrap_or_else(|error| panic!("rel = {rel} 建域应 Ok，实际 Err: {error}"));

        let expected = env
            .worktree_root
            .join("fix-bug")
            .join(rel)
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            outcome.exec_root.as_deref(),
            Some(expected.as_str()),
            "rel = {rel}"
        );
        assert!(
            !expected.ends_with('/') && !expected.ends_with('\\'),
            "执行根串无尾分隔符，实际: {expected}"
        );
    }
}

/// placement（边界）：`rel` 超长（> 1000 字符多层偏移）→ 原样透传零截断，
/// 执行根串 = host + `/` + rel 逐字。
#[test]
fn placement_rel超长_原样透传零截断() {
    let env = Env::new("placement-long-rel");
    let rel = long_rel();
    assert!(
        rel.len() > 1000,
        "夹具偏移应超 1000 字符，实际 {}",
        rel.len()
    );
    env.vcs.set_rel(&rel);
    seed_subdir(&env, &rel);

    let outcome = env.create("fix-bug", "超长偏移 goal").expect("建域应 Ok");

    let expected = env
        .worktree_root
        .join("fix-bug")
        .join(&rel)
        .to_string_lossy()
        .into_owned();
    let actual = outcome.exec_root.expect("超长偏移仍派生执行根");
    assert_eq!(actual, expected, "超长偏移原样透传零截断");
    assert_eq!(actual.len(), expected.len(), "长度零截断");
}

/// placement（异常）：假件返 `Err`（非 git 仓 / 裸仓 / 空输出三档引导文案）
/// → 取落位即 `Err`，零建档零 `add_worktree` 零 branch。
#[test]
fn placement_三档失败引导_取落位即err且零副作用() {
    for fault in [
        "主仓为非 git 仓：change worktree 落位需要 git 仓库，请先 git init",
        "主仓为裸仓（bare repository）：rev-parse 报 must be run in a work tree",
        "主仓的 git 工作树根解析为空（rev-parse --show-toplevel 无输出）",
    ] {
        let env = Env::new(&format!("placement-fault-{}", fault.len()));
        env.vcs.set_placement_error(fault);

        let error = env
            .create("fix-bug", "落位失败 goal")
            .expect_err("取落位应 Err");

        assert!(error.contains(fault), "Err 透传落位引导文案，实际: {error}");
        assert_eq!(env.store.create_call_count(), 0, "零建档");
        assert_eq!(
            env.vcs.calls(),
            vec!["placement"],
            "仅落位派生一次（零 add 零 branch）"
        );
        assert!(
            !env.worktree_root.join("fix-bug").exists(),
            "零 worktree 目录"
        );
    }
}

/// tracked_at_head（正向）：假件返 `Ok(true)` 且 `rel` 非空 → create 照常建
/// 档；调用恰一次且实参 `(main_root, rel)` 逐字（捕获面断言）。
#[test]
fn tracked_at_head_在案_照常建档且实参逐字恰一次() {
    let env = Env::new("tracked-in-head");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");

    env.create("fix-bug", "在案 goal").expect("在案应建域 Ok");

    assert_eq!(
        env.vcs.tracked_calls(),
        vec!["packages/desktop".to_owned()],
        "判据恰一次且 rel 实参逐字"
    );
    assert!(
        env.vcs.calls().contains(&"add:change/fix-bug".to_owned()),
        "在案后照常建域，实际: {:?}",
        env.vcs.calls()
    );
}

/// tracked_at_head（异常）：假件返 `Ok(false)` → 前置第十道 `Err`，文案含
/// `rel`、三形态成因与两条引导；零建档零目录零 `add_worktree` 零 branch。
#[test]
fn tracked_at_head_不在案_前置第十道拒绝且零副作用() {
    let env = Env::new("tracked-miss");
    env.vcs.set_rel("packages/desktop");
    env.vcs.set_untracked();

    let error = env
        .create("fix-bug", "不在案 goal")
        .expect_err("不在案应 Err");

    assert!(
        error.contains("packages/desktop"),
        "Err 文案含 rel，实际: {error}"
    );
    for cause in ["从未纳入 git 跟踪", "已在 HEAD 中删除", "gitignore"] {
        assert!(
            error.contains(cause),
            "Err 并列成因「{cause}」，实际: {error}"
        );
    }
    for guide in ["纳入 git 跟踪并提交", "改用仓库根"] {
        assert!(
            error.contains(guide),
            "Err 含引导「{guide}」，实际: {error}"
        );
    }
    assert_eq!(env.store.create_call_count(), 0, "零建档");
    assert!(
        env.vcs.adds.lock().expect("add 锁不可中毒").is_empty(),
        "零 add_worktree"
    );
    assert!(
        !env.vcs
            .calls()
            .iter()
            .any(|call| call.starts_with("delete_branch")),
        "零 branch 铸出 / 删除，实际: {:?}",
        env.vcs.calls()
    );
    assert!(
        !env.worktree_root.join("fix-bug").exists(),
        "零 worktree 目录"
    );
}

/// tracked_at_head（异常）：假件返 `Err`（fail-closed：问不出来 ≠ 不在案）→
/// `Err` 透传，零建档零 `add_worktree`。
#[test]
fn tracked_at_head_询问失败_fail_closed透传() {
    let env = Env::new("tracked-error");
    env.vcs.set_rel("packages/desktop");
    env.vcs
        .set_tracked_error("git rev-parse --verify HEAD:packages/desktop 失败（退出码 128）");

    let error = env
        .create("fix-bug", "询问失败 goal")
        .expect_err("询问失败应 Err");

    assert!(
        error.contains("退出码 128"),
        "Err 为判据原文（与「不在案」的拒绝文案可辨），实际: {error}"
    );
    assert!(
        !error.contains("从未纳入 git 跟踪"),
        "询问失败不伪装成「不在案」，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "零建档");
    assert!(
        env.vcs.adds.lock().expect("add 锁不可中毒").is_empty(),
        "零 add_worktree"
    );
}

/// tracked_at_head（边界）：`rel = ""` → 假件调用计数恒 0（create 侧整段跳
/// 过；后置 `is_dir` 断言同段跳过），vcs 调用序无新增项。
#[test]
fn tracked_at_head_rel空串_零调用且后置断言跳过() {
    let env = Env::new("tracked-root-skip");

    env.create("fix-bug", "根 workspace goal")
        .expect("建域应 Ok");

    assert!(env.vcs.tracked_calls().is_empty(), "rel 空串零判据调用");
    assert_eq!(
        env.vcs.calls(),
        vec![
            "placement".to_owned(),
            "probe".to_owned(),
            "branch_exists:change/fix-bug".to_owned(),
            "add:change/fix-bug".to_owned(),
        ],
        "根 workspace 调用序无新增项（判据与后置断言整段跳过）"
    );
}

/// `worktree_root`（异常）：同签面迁移后调用记录中零 `worktree_root` 项（旧
/// 落位方法不再被任何写路径触达）。
#[test]
fn 旧落位方法名零调用_迁移后只走placement() {
    let env = Env::new("legacy-root-name");
    env.vcs.set_rel("client");
    seed_subdir(&env, "client");

    env.create("fix-bug", "迁移 goal").expect("建域应 Ok");

    let calls = env.vcs.calls();
    assert!(
        !calls.iter().any(|call| call == "worktree_root"),
        "旧方法零触达，实际: {calls:?}"
    );
    assert!(calls.contains(&"placement".to_owned()), "落位派生走新方法");
}

/// `WorktreePlacement`（正向）：两字段全公开可构造（结构体字面量不经构造函
/// 数），`rel` 恒 `/` 分隔（`\` 不得拼入 `rel`）。
#[test]
fn worktree_placement_两字段公开可构造且rel以斜杠分隔() {
    let placement = WorktreePlacement {
        root: PathBuf::from("C:\\data\\.worktrees\\repo"),
        rel: "client/app/pkg".to_owned(),
    };

    assert_eq!(placement.root, PathBuf::from("C:\\data\\.worktrees\\repo"));
    assert_eq!(placement.rel, "client/app/pkg", "rel 字段公开可读且逐字");
    assert!(
        !placement.rel.contains('\\'),
        "rel 为 `/` 分隔（不含反斜杠）"
    );
    assert_eq!(placement.rel.split('/').count(), 3, "多段偏移逐段可切分");
    let empty = WorktreePlacement {
        root: placement.root.clone(),
        rel: String::new(),
    };
    assert!(
        empty.rel.is_empty(),
        "空串语义可辨（= workspace root 即 git 根）"
    );
}

/// `WorktreePlacement`（边界）：`rel = ""` + `root` 组合 → `root.join("")` 语
/// 义不产出尾分隔符（出线串与 host 串逐字相等）。
#[test]
fn worktree_placement_rel空串与root拼接_出线串等于host串() {
    let env = Env::new("placement-empty-rel");
    let placement = WorktreePlacement {
        root: env.worktree_root.clone(),
        rel: String::new(),
    };
    assert_eq!(
        placement.root.join(&placement.rel),
        placement.root,
        "Path 组件级比较不因空串追加分隔符"
    );

    let outcome = env.create("fix-bug", "空偏移 goal").expect("建域应 Ok");

    let host = placement
        .root
        .join("fix-bug")
        .to_string_lossy()
        .into_owned();
    assert_eq!(outcome.worktree, host, "出线串与 host 串逐字相等");
    assert!(
        !outcome.worktree.ends_with('/') && !outcome.worktree.ends_with('\\'),
        "host 串无尾分隔符，实际: {}",
        outcome.worktree
    );
}

// ---------------------------------------------------------------------------
// 门面再导出：`write::WorktreePlacement` 经出口取型构造 / 消费，零包装
// ---------------------------------------------------------------------------

/// 门面再导出（正向）：经 `workflow::write` 出口取型构造 `WorktreePlacement`
/// 并驱动 create → 编译面与行为面同时成立。
#[test]
fn write出口再导出_取型构造placement并驱动create() {
    let env = Env::new("reexport-placement");
    let placement: super::WorktreePlacement = super::WorktreePlacement {
        root: env.worktree_root.clone(),
        rel: "packages/desktop".to_owned(),
    };
    env.vcs.set_rel(&placement.rel);
    seed_subdir(&env, &placement.rel);

    let outcome = env.create("fix-bug", "再导出 goal").expect("建域应 Ok");

    let expected = placement
        .root
        .join("fix-bug")
        .join(&placement.rel)
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        outcome.exec_root.as_deref(),
        Some(expected.as_str()),
        "出口取型的 placement 值直接驱动落位与执行根派生"
    );
    assert_eq!(
        outcome.worktree,
        placement.root.join("fix-bug").to_string_lossy()
    );
}

/// 门面再导出（边界）：再导出零新增字段 / 方法——`root` 与 `rel` 两字段公开
/// 可读，`rel` 空串语义可辨。
#[test]
fn write出口再导出_零新增字段且rel空串语义可辨() {
    let placement: super::WorktreePlacement = super::WorktreePlacement {
        root: PathBuf::from("/tmp/.worktrees/repo"),
        rel: String::new(),
    };

    assert_eq!(placement.root, PathBuf::from("/tmp/.worktrees/repo"));
    assert!(placement.rel.is_empty(), "空串 = workspace root 即 git 根");
    let cloned = placement.clone();
    assert_eq!(cloned, placement, "门面再导出保留 Clone / PartialEq 面");
}

/// 门面再导出（异常）：门面层零包装——假件 `placement` 返 `Err` 时经 `write`
/// 出口取型的消费点原样透传 `Err`（带上下文但不改写文案）。
#[test]
fn write出口再导出_placement错误原样透传不改写文案() {
    let env = Env::new("reexport-placement-err");
    let original = "主仓 /tmp/x 为非 git 仓：change worktree 落位需要 git 仓库";
    env.vcs.set_placement_error(original);

    let error = env
        .create("fix-bug", "门面透传 goal")
        .expect_err("落位失败应 Err");

    assert_eq!(error, original, "门面零包装：Err 文案逐字透传");
    let _: super::WorktreePlacement = WorktreePlacement {
        root: PathBuf::from("/tmp/.worktrees/repo"),
        rel: String::new(),
    };
}

// ---------------------------------------------------------------------------
// 子目录 workspace 建域：双列建档 / 建域落位 / add 目标 / 出线两串（AC-2）
// ---------------------------------------------------------------------------

/// 双列建档（正向）：`rel = "packages/desktop"` → 记录 `worktree` =
/// `<落位根>/<name>`、`exec_root` = `worktree/packages/desktop`（两串逐字、均
/// 绝对）。
#[test]
fn create子目录workspace_双列建档两串逐字且均绝对() {
    let env = Env::new("create-dual-column");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");

    env.create("fix-bug", "双列 goal").expect("建域应 Ok");

    let record = env.store.created_record();
    let host = env.worktree_root.join("fix-bug");
    assert_eq!(
        record.worktree.as_deref(),
        Some(host.to_string_lossy().as_ref()),
        "worktree 列 = host"
    );
    assert_eq!(
        record.exec_root.as_deref(),
        Some(host.join("packages/desktop").to_string_lossy().as_ref()),
        "exec_root 列 = host/rel"
    );
    let host_str = record.worktree.expect("worktree 列在场");
    let exec_str = record.exec_root.expect("exec_root 列在场");
    assert!(
        Path::new(&host_str).is_absolute() && Path::new(&exec_str).is_absolute(),
        "两串均绝对路径"
    );
}

/// 建域落位（正向）：`rel` 非空 → `openspec/changes/<name>/explore.md` 落
/// `exec_root` 下；主仓 `<repo>/<rel>/openspec/changes/` 与
/// `worktree/openspec/` 两侧均无该目录。
#[test]
fn create子目录workspace_explore落执行根且host根零目录() {
    let env = Env::new("create-exec-root-tree");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");

    env.create("fix-bug", "执行根 goal").expect("建域应 Ok");

    let host = env.worktree_root.join("fix-bug");
    let exec_change_dir = resolve(&host.join("packages/desktop"))
        .changes_root
        .join("fix-bug");
    assert!(
        exec_change_dir.join("explore.md").is_file(),
        "explore.md 落执行根 change 目录: {}",
        exec_change_dir.display()
    );
    assert!(
        !resolve(&host).changes_root.join("fix-bug").exists(),
        "host 根下无 change 目录（执行根唯一落点）"
    );
    assert!(
        !env.root
            .join("packages/desktop/openspec/changes/fix-bug")
            .exists(),
        "主仓对应子目录下零 change 目录"
    );
    assert!(
        !env.root.join("openspec/changes/fix-bug").exists(),
        "主仓根下零 change 目录"
    );
}

/// 落位目标（正向）：`rel` 非空 → `add_worktree` 收到 `(main_root, host,
/// branch)`（目标为 host 而非执行根）。
#[test]
fn create子目录workspace_add目标恒host() {
    let env = Env::new("create-add-host");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");

    env.create("fix-bug", "add 目标 goal").expect("建域应 Ok");

    let adds = env.vcs.adds.lock().expect("add 锁不可中毒").clone();
    assert_eq!(adds.len(), 1, "add_worktree 恰一次");
    assert_eq!(adds[0].0, env.root, "add 收到主仓根");
    assert_eq!(
        adds[0].1,
        env.worktree_root.join("fix-bug"),
        "add 目标 = host（MUST NOT 为执行根）"
    );
    assert_ne!(
        adds[0].1,
        env.worktree_root.join("fix-bug").join("packages/desktop"),
        "add 目标非执行根"
    );
    assert_eq!(adds[0].2, "change/fix-bug", "add 收到铸分支名");
}

/// 出线两串（正向）：`CreateOutcome.worktree` = host 串、`execRoot` =
/// host/rel 串（两串可辨、互不替代）。
#[test]
fn create子目录workspace_出线两串可辨互不替代() {
    let env = Env::new("create-outcome-two-roots");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");

    let outcome = env.create("fix-bug", "出线两串 goal").expect("建域应 Ok");

    let exec_root = outcome.exec_root.clone().expect("execRoot 出线");
    assert_ne!(outcome.worktree, exec_root, "两串可辨");
    assert!(
        Path::new(&exec_root).starts_with(&outcome.worktree),
        "execRoot 在 worktree 之内: {exec_root} ⊄ {}",
        outcome.worktree
    );
    assert_eq!(
        env.store.created_record().exec_root.as_deref(),
        Some(exec_root.as_str()),
        "出线与记录同源（互不替代）"
    );
}

/// 执行根派生（边界）：`rel` 单段（`"client"`）与多段（`"client/app"`）→ 执行
/// 根派生同解且两列同源。
#[test]
fn create执行根派生_单段与多段两形态同解() {
    for rel in ["client", "client/app"] {
        let env = Env::new(&format!("create-derive-{}", rel.replace('/', "-")));
        env.vcs.set_rel(rel);
        seed_subdir(&env, rel);

        let outcome = env.create("fix-bug", "派生 goal").expect("建域应 Ok");

        let expected = env
            .worktree_root
            .join("fix-bug")
            .join(rel)
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            outcome.exec_root.as_deref(),
            Some(expected.as_str()),
            "rel = {rel}"
        );
        assert_eq!(
            env.store.created_record().exec_root.as_deref(),
            Some(expected.as_str()),
            "记录列与 DTO 同源（rel = {rel}）"
        );
    }
}

/// 执行根派生（边界）：`rel` 超长（> 1000 字符）→ 记录与 DTO 的执行根串原样
/// 透传零截断、不 panic。
#[test]
fn create执行根派生_超长rel记录与dto原样透传零截断() {
    let env = Env::new("create-long-rel");
    let rel = long_rel();
    env.vcs.set_rel(&rel);
    seed_subdir(&env, &rel);

    let outcome = env.create("fix-bug", "超长 goal").expect("建域应 Ok");

    let expected = env
        .worktree_root
        .join("fix-bug")
        .join(&rel)
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        outcome.exec_root.as_deref(),
        Some(expected.as_str()),
        "DTO 零截断"
    );
    assert_eq!(
        env.store.created_record().exec_root.as_deref(),
        Some(expected.as_str()),
        "记录零截断"
    );
}

/// 前置第十道形态二三（异常）：HEAD 已删除（`Ok(false)`）与 gitignore 命中
/// （`Ok(false)`）→ 同一判据同形拒绝（文案并列三形态、不逐形态分叉）。
#[test]
fn create_前置第十道三形态同形拒绝不分叉() {
    let mut errors = Vec::new();
    for tag in ["head-deleted", "gitignored"] {
        let env = Env::new(&format!("tracked-form-{tag}"));
        env.vcs.set_rel("packages/desktop");
        env.vcs.set_untracked();

        errors.push(env.create("fix-bug", "同形拒绝 goal").expect_err("应 Err"));
    }

    assert_eq!(errors[0], errors[1], "三形态同形拒绝（文案不逐形态分叉）");
    for cause in ["从未纳入 git 跟踪", "已在 HEAD 中删除", "被 gitignore 排除"] {
        assert!(
            errors[0].contains(cause),
            "成因三形态并列呈现（「{cause}」），实际: {}",
            errors[0]
        );
    }
}

/// 前置第十道形态四（正向）：子目录被跟踪但仅含被 ignore 的文件
/// （`Ok(true)`）→ 建档成功（`exec_root` 列写入）。
#[test]
fn create_前置第十道形态四_仅含ignored文件仍建档() {
    let env = Env::new("tracked-ignored-only");
    env.vcs.set_rel("packages/desktop");
    seed_main_path(&env, "packages/desktop/ignored.log", "被 ignore 的内容\n");

    // 判据只答「HEAD 有没有这个路径」，不答「目录里有什么」——假件恒 true
    // 即该语义面
    let outcome = env
        .create("fix-bug", "ignored-only goal")
        .expect("建档应 Ok");

    assert_eq!(env.vcs.tracked_calls().len(), 1, "判据仍恰一次");
    assert!(
        outcome.exec_root.is_some(),
        "形态四不拒绝（exec_root 列写入）"
    );
    assert_eq!(
        env.store.created_record().exec_root,
        outcome.exec_root,
        "记录与 DTO 同源"
    );
}

/// 前置第十道形态六 + 后置断言（异常）：前置在案但检出后执行根缺席（假件
/// 「不镜像 rel 子树」形态）→ 补偿链入参恒 host（`remove_worktree` 收到
/// host 串）、`Err` 含「已补偿回收」与双源残留指引；零 worktree 零 branch 零
/// 建档。
#[test]
fn create_后置断言_执行根缺席_补偿链入参恒host() {
    let env = Env::new("post-assert-miss");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");
    env.vcs.set_drop_exec_root();

    let error = env
        .create("fix-bug", "执行根缺席 goal")
        .expect_err("后置断言应 Err");

    let host = env.worktree_root.join("fix-bug");
    assert!(
        error.contains("不在盘"),
        "Err 记因执行根缺席，实际: {error}"
    );
    assert!(
        error.contains("已补偿回收"),
        "Err 呈补偿完成，实际: {error}"
    );
    assert!(
        error.contains("worktree 残留对账须双源"),
        "Err 携双源残留指引，实际: {error}"
    );
    assert_eq!(
        env.vcs.removes(),
        vec![host.clone()],
        "remove_worktree 收到 host（MUST NOT 为执行根）"
    );
    assert!(!host.exists(), "worktree 已回收");
    assert!(
        !host.join("packages/desktop").exists(),
        "执行根残留随整树回收"
    );
    assert_eq!(
        env.store.find_by_id(&env.store.created_id()),
        None,
        "建档已补偿删除"
    );
    assert_eq!(
        env.store.create_call_count(),
        1,
        "建档先行后补偿删除（零残留）"
    );
}

/// 后置断言时序（边界）：同一「不镜像 rel 子树」形态下，执行根不得被
/// `create_dir_all` 建出（断言早于 `write_fs_half`）——以「store 零残留 + host
/// 已回收」而非「建域成功」收口。
#[test]
fn create_后置断言早于写树_执行根不被create_dir_all建出() {
    let env = Env::new("post-assert-order");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");
    env.vcs.set_drop_exec_root();

    let error = env
        .create("fix-bug", "时序 goal")
        .expect_err("断言应先于写树 Err");

    let host = env.worktree_root.join("fix-bug");
    assert!(
        !error.contains("创建 change 目录失败"),
        "失败点为后置断言而非写树（时序可辨），实际: {error}"
    );
    assert!(!host.exists(), "host 已回收");
    assert!(
        env.store.find_by_id(&env.store.created_id()).is_none(),
        "store 零残留"
    );
    assert!(
        env.vcs.calls().contains(&"remove_worktree".to_owned()),
        "补偿链已下发，实际: {:?}",
        env.vcs.calls()
    );
}

/// D7 同仓同名拒绝（异常）：同仓第二个子目录 workspace 建同名 change → 撞前
/// 置⑥（branch 已存在）或前置⑦（worktree 目录已存在），拒绝面零建档零新目
/// 录。
#[test]
fn create_同仓同名子目录_撞前置六七拒绝且零建档() {
    // 形态一：branch 已存在（前置⑥）
    let env = Env::new("d7-branch");
    env.vcs.set_rel("packages/desktop");
    env.vcs.set_branch_exists();
    let error = env
        .create("fix-bug", "同名 goal")
        .expect_err("branch 冲突应 Err");
    assert!(
        error.contains("branch \"change/fix-bug\" 已存在"),
        "撞前置⑥，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "零建档");

    // 形态二：worktree 目录已存在（前置⑦）
    let env = Env::new("d7-dir");
    env.vcs.set_rel("packages/desktop");
    fs::create_dir_all(env.worktree_root.join("fix-bug")).expect("预置落位目录失败");
    let error = env
        .create("fix-bug", "同名 goal")
        .expect_err("目录冲突应 Err");
    assert!(
        error.contains("worktree 目录已存在"),
        "撞前置⑦，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "零建档");
}

// ---------------------------------------------------------------------------
// bootstrap 锚执行根（D5 词汇口径）
// ---------------------------------------------------------------------------

/// bootstrap 锚执行根（正向）：`projects = [{ root: "." }]` 且 `rel` 非空 →
/// 安装 cwd = 执行根。
#[test]
fn bootstrap锚执行根_点号项目按执行根安装() {
    let env = Env::new("bootstrap-exec-dot");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");
    seed_main_path(&env, "packages/desktop/pnpm-lock.yaml", "# pnpm");

    let outcome = env
        .create_with_projects("fix-bug", "点号 goal", &[proj(".")])
        .expect("建域应 Ok");

    assert_eq!(
        env.vcs.installs(),
        vec![(
            env.worktree_root.join("fix-bug").join("packages/desktop"),
            "pnpm install --frozen-lockfile".to_owned()
        )],
        "点号项目根的安装 cwd = 执行根"
    );
    assert!(
        outcome.warnings.is_empty(),
        "顺利安装零警告，实际 {:?}",
        outcome.warnings
    );
}

/// bootstrap 锚执行根（正向）：`projects = [{ root: "packages/desktop" }]` 且
/// `rel = "packages/desktop"` → 安装 cwd = `exec_root/packages/desktop`（相对
/// 执行根同解）。
#[test]
fn bootstrap锚执行根_项目根相对执行根同解() {
    let env = Env::new("bootstrap-exec-nested");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");
    seed_main_path(
        &env,
        "packages/desktop/packages/desktop/Cargo.lock",
        "# cargo",
    );

    let outcome = env
        .create_with_projects("fix-bug", "嵌套 goal", &[proj("packages/desktop")])
        .expect("建域应 Ok");

    assert_eq!(
        env.vcs.installs(),
        vec![(
            env.worktree_root
                .join("fix-bug")
                .join("packages/desktop")
                .join("packages/desktop"),
            "cargo fetch --locked".to_owned()
        )],
        "项目根相对执行根同解"
    );
    assert!(outcome.warnings.is_empty());
}

/// bootstrap 锚执行根（异常）：项目根在执行根内不存在 → 警告「项目根 x：在
/// 执行根内不存在，跳过依赖引导」（执行根口径文案）。
#[test]
fn bootstrap锚执行根_项目根缺席警告执行根口径() {
    let env = Env::new("bootstrap-exec-missing");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");

    let outcome = env
        .create_with_projects("fix-bug", "缺席 goal", &[proj("missing")])
        .expect("建域应 Ok");

    assert_eq!(
        outcome.warnings,
        vec!["项目根 \"missing\"：在执行根内不存在，跳过依赖引导".to_owned()]
    );
    assert!(env.vcs.installs().is_empty(), "零安装");
}

/// bootstrap 锚执行根（边界）：`projects` 空 + `rel` 非空 → legacy 扫描锚 =
/// 执行根（lockfile 在执行根内命中、host 根不命中）。
#[test]
fn bootstrap锚执行根_空projects按执行根扫描lockfile() {
    let env = Env::new("bootstrap-exec-legacy");
    env.vcs.set_rel("packages/desktop");
    seed_subdir(&env, "packages/desktop");
    seed_main_path(&env, "packages/desktop/pnpm-lock.yaml", "# pnpm");

    env.create("fix-bug", "legacy goal").expect("建域应 Ok");

    assert_eq!(
        env.vcs.installs(),
        vec![(
            env.worktree_root.join("fix-bug").join("packages/desktop"),
            "pnpm install --frozen-lockfile".to_owned()
        )],
        "legacy 扫描锚 = 执行根（host 根零 lockfile）"
    );
}

// ---------------------------------------------------------------------------
// CreateOutcome.exec_root 出线形态（camelCase / 键恒在场 / Debug 字段序）
// ---------------------------------------------------------------------------

/// `CreateOutcome.exec_root`（边界）：`None` → `execRoot` 键恒在场且值为
/// `null`；`Some` → 字符串出线。
#[test]
fn create_outcome_exec_root出线两态_键恒在场() {
    for exec_root in [
        None,
        Some("C:\\home\\.dev-team\\worktrees\\seg\\fix-bug\\packages\\desktop"),
    ] {
        let outcome = CreateOutcome {
            id: "0198f7a0-0000-7000-8000-000000000000".to_owned(),
            name: "fix-bug".to_owned(),
            created: "2026-10-02".to_owned(),
            worktree: "C:\\home\\.dev-team\\worktrees\\seg\\fix-bug".to_owned(),
            exec_root: exec_root.map(str::to_owned),
            warnings: Vec::new(),
        };

        let value = serde_json::to_value(&outcome).expect("序列化应成功");
        let object = value.as_object().expect("线形状为对象");
        assert!(
            object.contains_key("execRoot"),
            "execRoot 键恒在场（不省键），实际键集: {:?}",
            object.keys().collect::<Vec<_>>()
        );
        match exec_root {
            None => assert_eq!(object["execRoot"], serde_json::json!(null), "None → null"),
            Some(root) => assert_eq!(
                object["execRoot"],
                serde_json::json!(root),
                "Some → 字符串出线"
            ),
        }
    }
}

/// `CreateOutcome.exec_root`（边界）：Debug 形态字段清单含 `execRoot`（五 →
/// 六字段，`starts_with("CreateOutcome { id: ")` 断言零改）。
#[test]
fn create_outcome_debug字段清单含exec_root() {
    let outcome = CreateOutcome {
        id: "0198f7a0-0000-7000-8000-000000000001".to_owned(),
        name: "fix-bug".to_owned(),
        created: "2026-10-02".to_owned(),
        worktree: "C:\\host".to_owned(),
        exec_root: Some("C:\\host\\rel".to_owned()),
        warnings: Vec::new(),
    };

    let debug = format!("{outcome:?}");
    assert!(
        debug.starts_with("CreateOutcome { id: "),
        "id 首字段（声明序）"
    );
    assert!(
        debug.contains("exec_root: Some("),
        "Debug 含 exec_root 字段"
    );
    let worktree_at = debug.find("worktree:").expect("Debug 含 worktree");
    let exec_at = debug.find("exec_root:").expect("Debug 含 exec_root");
    let warnings_at = debug.find("warnings:").expect("Debug 含 warnings");
    assert!(
        worktree_at < exec_at && exec_at < warnings_at,
        "声明序 worktree → exec_root → warnings，实际: {debug}"
    );
}
