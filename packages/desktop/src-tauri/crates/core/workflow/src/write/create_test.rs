//! `write::create` 的单元测试（test-design「create.rs -> create_test.rs」节）：
//! create 建域四段组合（D2/D3/D4/D5）——建档（携 worktree / base_commit）+
//! worktree add（HEAD 基线铸 change/<name> 分支）+ worktree 内目录树与
//! explore.md（goal 原文）+ bootstrap / 警告收集；前置七道全 IO 前置（拒绝面
//! 零 worktree、零建档、零目录、vcs 零调用）；补偿链（add 失败：删建档 + 尽力
//! 删分支；树写出失败：remove → 删分支 → 删建档；bootstrap 段失败不回收）；
//! created 出线取 db created_at 日期；成功后清单立即可见；目录树零
//! workflow.json 产出（双向墙写半边）。
//!
//! Mock策略（test-design 本节 Mock 表）：db 半边以进程内假件实现
//! [`ChangeStateStore`]（捕获建档 / 补偿删除调用 + 可编程补偿故障；真实
//! tempfile Store 组合行收 tests/corpus_golden_test.rs 集成面）；vcs 半边以
//! 进程内脚本化假件实现 [`WorktreePort`]（probe / branch_exists / 各方法
//! Err 可编程注入 + 调用与参数捕获；add_worktree 成功即真实建目录并镜像主仓
//! 树——HEAD 检出的最小模拟；真实 git 夹具行收 vcs-runtime crate 测试面）；
//! 文件系统真实 tempdir × 2（主仓根 + worktree 落位父锚）+ 预置路径分量文件
//! 占位注入写失败（不经 mock）。「当日」断言用调用前后 UTC 日期并集界，零
//! wall-clock 等值比较。

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
    PhaseStateRecord, RunFinishCommand, RunStartCommand, RunStateRecord, RunStepStateRecord,
    StepCommand, StepStateRecord, StoreFault,
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
            worktree_root,
            store: CreateStore::new(),
            vcs: FakeVcs::new(),
        }
    }

    fn create(&self, name: &str, goal: &str) -> Result<CreateOutcome, String> {
        create(
            &self.root,
            &self.worktree_root,
            &self.store,
            &self.vcs,
            name,
            goal,
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
    program: Mutex<VcsProgram>,
    calls: Mutex<Vec<String>>,
    adds: Mutex<Vec<(PathBuf, PathBuf, String)>>,
    installs: Mutex<Vec<(PathBuf, String)>>,
}

impl FakeVcs {
    fn new() -> Self {
        Self {
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
}

impl WorktreePort for FakeVcs {
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
// delete_change_record 四个 min 操作镜像真件语义（主键 name 存取、补偿删除
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

    fn seed_active(&self, name: &str) {
        self.records
            .lock()
            .expect("记录锁不可中毒")
            .push(ChangeStateRecord {
                name: name.to_owned(),
                workflow_type: "requirement".to_owned(),
                created_at: 1_727_000_000_000,
                status: ChangeStatus::Active,
                archived_at: None,
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

    fn find(&self, name: &str) -> Option<ChangeStateRecord> {
        self.records
            .lock()
            .expect("记录锁不可中毒")
            .iter()
            .find(|record| record.name == name)
            .cloned()
    }
}

impl ChangeStateStore for CreateStore {
    fn get_change(&self, name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(self.find(name))
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
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

    fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault> {
        self.creates
            .lock()
            .expect("建档锁不可中毒")
            .push(record.clone());
        self.records.lock().expect("记录锁不可中毒").push(record);
        Ok(())
    }

    fn delete_change_record(&self, name: &str) -> Result<bool, StoreFault> {
        self.deletes
            .lock()
            .expect("删除锁不可中毒")
            .push(name.to_owned());
        if let Some(fault) = self.delete_fault.lock().expect("故障锁不可中毒").clone() {
            return Err(fault);
        }
        let mut records = self.records.lock().expect("记录锁不可中毒");
        let before = records.len();
        records.retain(|record| record.name != name);
        Ok(records.len() < before)
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

    fn list_runs(&self, _change: &str) -> Result<Vec<RunStateRecord>, StoreFault> {
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
/// 原文 + ChangeRecord 建档（worktree / base_commit 随行）；add_worktree 收
/// 到 (main_root, worktree, change/<name>)；主仓 active 树不含该目录；目录
/// 树内零 workflow.json 产出（双向墙写半边）。
#[test]
fn 建域四件套成功_worktree内目录与explore与db建档且主仓零目录() {
    let env = Env::new("happy");
    let before = utc_date_today();

    let outcome = env
        .create("fix-bug", "修复登录重试的竞态问题")
        .expect("合法输入应 Ok");

    let after = utc_date_today();
    assert_eq!(outcome.name, "fix-bug");
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

    // db 半边：ChangeRecord 建档（worktree / base_commit 随行）
    let record = env.store.find("fix-bug").expect("建档记录应在场");
    assert_eq!(record.name, "fix-bug");
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

/// 前置③④：主仓 active 目录已存在 / db 同名 active → `Err` 且 vcs 零调用
///（含 probe——冲突检查先于 git 探测的顺序锚）、既有目录零触碰。
#[test]
fn 目录与建档冲突拒绝且vcs零调用() {
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

    // db 同名 active：同序锚（vcs 零调用）
    let env = Env::new("db-taken");
    env.store.seed_active("taken");
    let error = env
        .create("taken", "新建 goal")
        .expect_err("同名 active 应 Err");
    assert!(
        error.contains("已存在") && error.contains("taken"),
        "错误归因同名建档冲突，实际: {error}"
    );
    assert!(env.active_dir_names().is_empty(), "零目录创建");
    assert!(
        env.vcs.calls().is_empty(),
        "vcs 零调用，实际: {:?}",
        env.vcs.calls()
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
    let record = env.store.find("fix-bug").expect("建档记录应在场");
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

    assert!(env.store.find("fix-bug").is_some(), "建档保留（不回滚）");
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

    assert!(env.store.find("fix-bug").is_some(), "建档保留");
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
// 异常：补偿链（D3）
// ---------------------------------------------------------------------------

/// add 失败补偿：`delete_change_record` 删本次建档（恰一次）+ 尽力
/// `delete_branch`（捕获调用）；Err 呈现失败与补偿事实。
#[test]
fn add失败补偿删建档且尽力删分支() {
    let env = Env::new("add-fail");
    env.vcs.set_add_fault("worktree add 模拟失败");

    let error = env
        .create("fix-bug", "add 失败 goal")
        .expect_err("add 失败应 Err");

    assert!(error.contains("补偿"), "Err 呈现补偿事实，实际: {error}");
    assert_eq!(
        env.store.deletes.lock().expect("删除锁不可中毒").as_slice(),
        ["fix-bug"],
        "补偿删除恰针对本次自插行调用一次"
    );
    assert!(env.store.find("fix-bug").is_none(), "db 零残留");
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
    assert_eq!(
        env.store.deletes.lock().expect("删除锁不可中毒").as_slice(),
        ["fix-bug"],
        "删建档恰一次"
    );
    assert!(
        env.store.find("fix-bug").is_none(),
        "补偿删除本次自插行（db 零残留）"
    );
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
    assert_eq!(
        env.store.deletes.lock().expect("删除锁不可中毒").as_slice(),
        ["add-orphan"],
        "建档补偿删除恰一次（记录半边回收）"
    );

    // 三故障角落：建档补偿删除再失败 → 残留记因随 Err 呈现（随下次同名建档
    // 的冲突检查显式暴露）
    let env = Env::new("add-fail-record-residual");
    env.vcs.set_add_fault("worktree add 模拟失败");
    env.store
        .set_delete_fault(StoreFault::Db("建档删除模拟失败".to_owned()));

    let error = env
        .create("record-orphan", "三故障 goal")
        .expect_err("建档补偿删除失败应 Err");

    assert!(
        error.contains("建档记录 \"record-orphan\"") && error.contains("建档删除模拟失败"),
        "Err 呈现建档残留记因，实际: {error}"
    );
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
    let list = list_changes(&resolve(&env.root), &env.store);
    assert_eq!(list.active.len(), 1, "active 恰一条");
    assert_eq!(list.active[0].name, "combo-visible");
    assert_eq!(list.active[0].status, Some(ChangeStatus::Active));
    assert!(
        list.active[0].created.as_deref() == Some(before.as_str())
            || list.active[0].created.as_deref() == Some(after.as_str()),
        "created 透传建档日期，实际: {:?}",
        list.active[0].created
    );
}

/// CreateOutcome serde 线形状：序列化恰 `name` / `created` / `worktree` /
/// `warnings` 四键（camelCase）；`warnings` 空清单出线 `[]` 非 null。
#[test]
fn create_outcome_serde线形状恰四键() {
    let outcome = CreateOutcome {
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
        vec!["created", "name", "warnings", "worktree"],
        "恰四键面（worktree 为刻意出线的执行锚）"
    );
    assert_eq!(object["name"], "fix-bug");
    assert_eq!(object["created"], "2026-10-02");
    assert_eq!(
        object["warnings"],
        serde_json::json!([]),
        "warnings 空清单出线 [] 非 null"
    );
}
