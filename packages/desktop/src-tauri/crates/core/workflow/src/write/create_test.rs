use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use foundation::layout::resolve;
use time::OffsetDateTime;

use super::create;
use super::CreateOutcome;
use super::{InstallRun, RepoProbe, WorktreePort};
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
        project_roots: &[String],
    ) -> Result<CreateOutcome, String> {
        create(
            &self.root,
            &self.store,
            &self.vcs,
            project_roots,
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
}

struct FakeVcs {
    /// worktree 落位父锚（create 经 port 派生；夹具根由 Env 注入）。
    worktree_root: PathBuf,
    program: Mutex<VcsProgram>,
    calls: Mutex<Vec<String>>,
    adds: Mutex<Vec<(PathBuf, PathBuf, String)>>,
    installs: Mutex<Vec<(PathBuf, String)>>,
}

impl FakeVcs {
    fn new(worktree_root: PathBuf) -> Self {
        Self {
            worktree_root,
            program: Mutex::new(VcsProgram::default()),
            calls: Mutex::new(Vec::new()),
            adds: Mutex::new(Vec::new()),
            installs: Mutex::new(Vec::new()),
        }
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
    fn worktree_root(&self, _main_root: &Path) -> Result<PathBuf, String> {
        self.record("worktree_root");
        Ok(self.worktree_root.clone())
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
        mirror_tree(main_root, worktree)
    }

    fn remove_worktree(&self, _main_root: &Path, worktree: &Path) -> Result<(), String> {
        self.record("remove_worktree");
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
    let roots = ["web".to_owned(), "service".to_owned()];

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
    let roots = ["web".to_owned(), "api".to_owned()];

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

/// 配置根在 worktree 内不存在：零安装 + 逐项目警告，create 照常成功。
#[test]
fn projects根不存在警告且零安装() {
    let env = Env::new("projects-missing");
    let roots = ["missing".to_owned()];

    let outcome = env
        .create_with_projects("fix-bug", "缺失根 goal", &roots)
        .expect("建域应 Ok");

    assert!(env.vcs.install_commands().is_empty(), "零安装");
    assert_eq!(
        outcome.warnings,
        vec!["项目根 \"missing\"：在 worktree 内不存在，跳过依赖引导".to_owned()]
    );
}

/// 项目无 lockfile / Cargo 无 lock：逐项目专项注记，零安装。
#[test]
fn projects无lockfile与cargo无lock逐项目注记() {
    let env = Env::new("projects-notes");
    seed_main_path(&env, "plain/.keep", "");
    seed_main_path(&env, "rs/Cargo.toml", "[package]");
    let roots = ["plain".to_owned(), "rs".to_owned()];

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
    let roots = ["web".to_owned()];

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
    let roots = ["web".to_owned(), "api".to_owned()];

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
    let roots = ["missing".to_owned()];

    let outcome = env
        .create_with_projects("fix-bug", "脏仓 goal", &roots)
        .expect("建域应 Ok");

    assert_eq!(outcome.warnings.len(), 2, "实际: {:?}", outcome.warnings);
    assert!(outcome.warnings[0].contains("主仓有未提交改动"));
    assert_eq!(
        outcome.warnings[1],
        "项目根 \"missing\"：在 worktree 内不存在，跳过依赖引导"
    );
}

/// `.` 项目根：等价扫描 worktree 根并在该处安装。
#[test]
fn projects点号根按worktree根扫描安装() {
    let env = Env::new("projects-dot");
    seed_main_file(&env, "pnpm-lock.yaml", "# pnpm");
    let roots = [".".to_owned()];

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

/// 树写出失败补偿：worktree 内路径分量预置文件占位（真实 fs 注入）→
/// `remove_worktree` → `delete_branch` → 删建档 全链下发（调用序）；Err 呈现
/// 补偿完成与 `git worktree list` 残留清理指引。
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
        error.contains("git worktree list"),
        "Err 呈现手动清理指引（git worktree list 文案锚），实际: {error}"
    );
}

/// 补偿链再失败（双故障角落）：链中 `remove_worktree` 注入 Err → Err 呈现
/// 残留对象（worktree / branch 名）与手动清理指引，不静默自愈。
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

    let error = env
        .create("orphan-name", "双故障角落 goal")
        .expect_err("补偿再失败应 Err");

    assert!(
        error.contains("残留") && error.contains("change/orphan-name"),
        "Err 呈现残留对象（worktree / branch 名），实际: {error}"
    );
    assert!(
        error.contains("git worktree list"),
        "Err 呈现手动清理指引，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 1, "建档先行恰好一次");
    let id = env.store.created_id();
    assert!(
        env.store.find_by_id(&id).is_none(),
        "记录半边回收（双故障角落仅 worktree 残留）"
    );

    // add 失败补偿链的双故障角落：add 注入 Err + delete_branch 再注入 Err →
    // Err 呈现残留对象（branch 名）与手动清理指引（不静默自愈）
    let env = Env::new("add-fail-branch-residual");
    env.vcs.set_add_fault("worktree add 模拟失败");
    env.vcs.set_delete_branch_fault("branch -D 模拟失败");

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
        error.contains("git worktree list"),
        "Err 呈现手动清理指引（不静默自愈），实际: {error}"
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
    assert!(
        list.active[0].created.as_deref() == Some(before.as_str())
            || list.active[0].created.as_deref() == Some(after.as_str()),
        "created 透传建档日期，实际: {:?}",
        list.active[0].created
    );
}

/// CreateOutcome serde 线形状：序列化恰 `id` / `name` / `created` /
/// `worktree` / `warnings` 五键（camelCase；`id` 为本次铸出的身份锚、声明序
/// 首字段）；`warnings` 空清单出线 `[]` 非 null。
/// 注：serde_json 对象为键自然序（BTreeMap）——声明序锚以 Debug 字段序断言
/// （DTO 声明序即 specta / TS 绑定字段序，签面逐字段断言归 bindings 守卫节）。
#[test]
fn create_outcome_serde线形状恰五键且id首字段() {
    let outcome = CreateOutcome {
        id: "0198f7a0-0000-7000-8000-000000000000".to_owned(),
        name: "fix-bug".to_owned(),
        created: "2026-10-02".to_owned(),
        worktree: "C:\\home\\.dev-team\\worktrees\\seg\\fix-bug".to_owned(),
        warnings: Vec::new(),
    };

    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let object = value.as_object().expect("序列化为对象");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["created", "id", "name", "warnings", "worktree"],
        "恰五键面（id 为身份锚、worktree 为刻意出线的执行锚）"
    );
    assert_eq!(object["id"], "0198f7a0-0000-7000-8000-000000000000");
    assert_eq!(object["name"], "fix-bug");
    assert_eq!(object["created"], "2026-10-02");
    assert_eq!(
        object["warnings"],
        serde_json::json!([]),
        "warnings 空清单出线 [] 非 null"
    );

    // id 首字段（声明序）：字段序即 TS 绑定字段序（id / name / created /
    // worktree / warnings 五字段面）
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
/// 目录数 8、门槛 7 与「git worktree list」对账引导；拒绝面零副作用。
#[test]
fn worktree门槛拒绝_8个直属子目录err文案含对账引导() {
    let env = Env::new("threshold-reject");
    fs::create_dir_all(&env.worktree_root).expect("建 worktree_root 失败");
    for index in 0..8 {
        fs::create_dir_all(env.worktree_root.join(format!("residual-{index}")))
            .expect("预置残留目录失败");
    }

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
        error.contains("git worktree list"),
        "文案携「git worktree list」对账引导: {error}"
    );
    // 拒绝面零副作用
    assert_eq!(env.store.create_call_count(), 0, "db 零建档调用");
    assert_eq!(
        env.vcs.calls(),
        vec!["worktree_root"],
        "仅落位派生一次（门槛道先于 probe / add）"
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
        vec!["worktree_root"],
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
