//! `write::promote_explore` 的单元测试（test-design「write/promote.rs ->
//! promote_test.rs」节，新增文件）：promote 的 move 半边——复用 `create` 建
//! change（`name = explore.name`、`goal = 笔记全文`、`title = explore.title`
//! 显式继承）→ worktree 内 `changes/<name>/explore.md` 即笔记全文 → 成功后删
//! 主仓 `explores/<name>.md`（移走半边，MUST NOT `fs::rename`）。失败面：
//! create 失败（前置拒绝 / vcs 注入 Err）→ `Err` 且主仓笔记零删除；删笔记失败
//! → `Err` 携 change id 与「笔记全文已在 change explore.md 留底」回写指引；
//! `PromoteOutcome` 两字段 serde camelCase 出线；sync 零 tokio 上下文依赖。
//!
//! Mock策略（test-design 本节 Mock 表）：db 半边以进程内假件实现
//! [`ChangeStateStore`]（捕获 create 建档载荷 + 可编程失败）；vcs 半边以进程内
//! 脚本化假件实现 [`WorktreePort`]（probe / branch_exists / add_worktree 可编程
//! 注入 Err + 调用捕获；add 成功即镜像主仓树——HEAD 检出的最小模拟）；文件系统
//! 真实 tempdir 双根；删笔记失败经「预置同名目录占位」注入（`remove_file` 对
//! 目录必失败），不经 mock。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use foundation::layout::resolve;

use super::promote::PromoteOutcome;
use super::promote_explore;
use super::{InstallRun, RepoProbe, WorktreePort};
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, PhaseStateRecord, RunFinishCommand, RunStartCommand, RunStateRecord,
    RunStepStateRecord, StepCommand, StepStateRecord, StoreFault,
};

// ---------------------------------------------------------------------------
// 装置：真实 tempdir 双根（主仓根 + worktree 落位父锚）+ 进程内假件 store / vcs
// ---------------------------------------------------------------------------

struct Env {
    root: PathBuf,
    worktree_root: PathBuf,
    store: PromoteStore,
    vcs: FakeVcs,
}

impl Env {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "workflow-promote-test-{}-{}",
            std::process::id(),
            tag
        ));
        let worktree_root = std::env::temp_dir().join(format!(
            "workflow-promote-worktrees-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&worktree_root);
        Self {
            root,
            worktree_root,
            store: PromoteStore::new(),
            vcs: FakeVcs::new(),
        }
    }

    /// 主仓 explore 笔记路径（`openspec/explores/<name>.md`）。
    fn note_path(&self, name: &str) -> PathBuf {
        resolve(&self.root).explores_root.join(format!("{name}.md"))
    }

    /// 预置主仓笔记（`explores/<name>.md` 全文写入）。
    fn seed_note(&self, name: &str, content: &str) -> PathBuf {
        let path = self.note_path(name);
        fs::create_dir_all(path.parent().expect("笔记路径应有父目录")).expect("建笔记目录失败");
        fs::write(&path, content).expect("写主仓笔记失败");
        path
    }

    /// worktree 内 change 目录（explore.md 落点）。
    fn change_dir(&self, name: &str) -> PathBuf {
        resolve(&self.worktree_root.join(name))
            .changes_root
            .join(name)
    }

    fn promote(&self, name: &str, note: &str, title: &str) -> Result<PromoteOutcome, String> {
        promote_explore(
            &self.root,
            &self.worktree_root,
            &self.store,
            &self.vcs,
            name,
            note,
            title,
        )
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir_all(&self.worktree_root);
    }
}

/// 递归复制目录内容（假件 add_worktree 的「HEAD 检出」最小模拟）。
fn mirror_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("假件建 worktree 失败: {error}"))?;
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(());
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
// 假件 vcs：probe / branch_exists / add_worktree 可编程注入 Err + 调用捕获
// ---------------------------------------------------------------------------

const FAKE_HEAD: &str = "0000000000000000000000000000000000000001";

#[derive(Default)]
struct VcsProgram {
    probe: Option<Result<RepoProbe, String>>,
    add_fault: Option<String>,
    remove_fault: Option<String>,
    delete_branch_fault: Option<String>,
}

struct FakeVcs {
    program: Mutex<VcsProgram>,
    calls: Mutex<Vec<String>>,
}

impl FakeVcs {
    fn new() -> Self {
        Self {
            program: Mutex::new(VcsProgram::default()),
            calls: Mutex::new(Vec::new()),
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

    fn set_add_fault(&self, error: &str) {
        self.program.lock().expect("程序锁不可中毒").add_fault = Some(error.to_owned());
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("vcs 调用锁不可中毒").clone()
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
        Ok(false)
    }

    fn add_worktree(&self, main_root: &Path, worktree: &Path, branch: &str) -> Result<(), String> {
        self.record(&format!("add:{branch}"));
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

    fn run_install(&self, _worktree: &Path, command: &str) -> Result<InstallRun, String> {
        self.record(&format!("install:{command}"));
        Ok(InstallRun {
            success: true,
            summary: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// 假件 store：list_change_records / create_change_record / delete_change_record
// 三个 min 操作镜像真件语义（create 前置扫描 + 补链），其余 unimplemented。
// ---------------------------------------------------------------------------

struct PromoteStore {
    records: Mutex<Vec<ChangeStateRecord>>,
    creates: Mutex<Vec<ChangeStateRecord>>,
}

impl PromoteStore {
    fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            creates: Mutex::new(Vec::new()),
        }
    }

    /// 最近一次建档捕获（title / name / goal 载荷断言面）。
    fn created_record(&self) -> ChangeStateRecord {
        self.creates
            .lock()
            .expect("建档锁不可中毒")
            .last()
            .expect("建档记录应已捕获")
            .clone()
    }
}

impl ChangeStateStore for PromoteStore {
    fn get_change(&self, _id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        Ok(None)
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

// ---------------------------------------------------------------------------
// 正向：move 全链（AC-6 写面半边）
// ---------------------------------------------------------------------------

/// promote 成功 move 全链（正向）：预置主仓 `explores/<name>.md` → promote
/// 成功：假 store 捕获 create 建档载荷（name / goal / title）；worktree 内
/// `changes/<name>/explore.md` 内容 == note 全文；主仓原笔记已删；
/// `PromoteOutcome` 返回 change_id / change_name。
#[test]
fn promote成功move全链_搬全文删原笔记且载荷title继承() {
    let env = Env::new("move-happy");
    let note = "# 我的标题\n\n首段正文\n\n- 线索一\n- 线索二";
    let note_path = env.seed_note("api-retry", note);

    let outcome = env
        .promote("api-retry", note, "我的标题")
        .expect("合法 kebab explore promote 应成功");

    // db 半边：建档载荷 name = explore.name、goal = note 全文、title 继承
    let record = env.store.created_record();
    assert_eq!(
        record.name, "api-retry",
        "change 名 = explore.name（免转换）"
    );
    assert_eq!(record.title, "我的标题", "title 显式继承 explore.title");
    assert_eq!(record.status, ChangeStatus::Active);
    assert_eq!(
        record.worktree.as_deref(),
        Some(
            env.worktree_root
                .join("api-retry")
                .to_string_lossy()
                .as_ref()
        ),
        "建档载荷携 worktree 执行锚"
    );

    // fs 半边：worktree 内 explore.md = note 全文（move 半边，MUST NOT rename）
    let landed = env.change_dir("api-retry").join("explore.md");
    assert_eq!(
        fs::read_to_string(&landed).expect("读 worktree 内 explore.md 失败"),
        note,
        "worktree 内 explore.md 为笔记全文（含首行标题）"
    );
    // 主仓原笔记已删
    assert!(
        !note_path.exists(),
        "主仓 explores/api-retry.md 已删（move 语义）"
    );

    // 出线
    assert_eq!(outcome.change_id, record.id, "change_id = 本次铸出 id");
    assert!(!outcome.change_id.is_empty(), "change_id 非空（身份锚）");
    assert_eq!(
        outcome.change_name, "api-retry",
        "change_name = explore.name"
    );
}

/// note 全文含首行标题（边界）：note 含首行 `# 标题` → 原样写入 worktree
/// explore.md（零内容转换，标题行保留；title 由调用方显式传入，写面不解析）。
#[test]
fn promote笔记全文原样落盘_首行标题零转换() {
    let env = Env::new("note-verbatim");
    let note = "# 探索标题 emoji 🚀\n\n一段正文\t制表符\n";
    env.seed_note("rich-note", note);

    env.promote("rich-note", note, "探索标题 emoji 🚀")
        .expect("promote 应成功");

    let landed =
        fs::read(env.change_dir("rich-note").join("explore.md")).expect("读 explore.md 失败");
    assert_eq!(landed, note.as_bytes(), "note 全文零转换（含首行标题）");
}

/// title 显式继承（正向）：promote 传入 title == explore.title → create 建档
/// 载荷 title 逐字一致（不做回退）。
#[test]
fn promote_title显式继承不回退() {
    let env = Env::new("title-inherit");
    let note = "# 继承来的标题\n正文";
    env.seed_note("inherit-topic", note);

    env.promote("inherit-topic", note, "继承来的标题")
        .expect("promote 应成功");

    let record = env.store.created_record();
    assert_eq!(
        record.title, "继承来的标题",
        "title 显式继承（非 name 回退）"
    );
    assert_ne!(
        record.title, record.name,
        "title ≠ name 证明未走回退单点（promote 路径显式传 explore.title）"
    );
}

// ---------------------------------------------------------------------------
// 异常：create 失败零副作用 / 删笔记失败显式 Err
// ---------------------------------------------------------------------------

/// create 失败零副作用（异常）：create 前置拒绝（probe 注入 Err）→ promote
/// 返回 `Err`，主仓笔记零删除、零变化。
#[test]
fn promote_create失败零副作用_主仓笔记零删除() {
    let env = Env::new("create-fail");
    let note = "# 标题\n正文";
    let note_path = env.seed_note("fail-topic", note);
    env.vcs
        .set_probe_error("git 不可用（PATH 未发现 git）: 环境缺失");

    let error = env
        .promote("fail-topic", note, "标题")
        .expect_err("create 前置失败应 Err");

    assert!(
        error.contains("git 不可用"),
        "create Err 原文透传，实际: {error}"
    );
    assert!(note_path.is_file(), "主仓笔记零删除");
    assert_eq!(
        fs::read_to_string(&note_path).expect("读笔记失败"),
        note,
        "主仓笔记内容零变化"
    );
    assert_eq!(
        env.store.records.lock().expect("记录锁不可中毒").len(),
        0,
        "零建档（前置拒绝在任何 IO 之前）"
    );
}

/// create 失败零副作用（异常，add 半途失败）：add_worktree 注入 Err → create
/// 补偿链删本次建档 → promote 返回 `Err` 且主仓笔记仍在。
#[test]
fn promote_add失败补偿后主仓笔记零删除() {
    let env = Env::new("add-fail");
    let note = "# 标题\n正文";
    let note_path = env.seed_note("add-fail-topic", note);
    env.vcs.set_add_fault("worktree 建域失败: 磁盘只读");

    let error = env
        .promote("add-fail-topic", note, "标题")
        .expect_err("add 失败应 Err");

    assert!(error.contains("建域失败"), "Err 携 create 记因: {error}");
    assert!(
        note_path.is_file(),
        "主仓笔记零删除（create 失败不触删笔记）"
    );
    assert!(
        env.vcs.calls().iter().any(|call| call.starts_with("add:")),
        "add_worktree 已尝试（失败点落在 add 半途），实际: {:?}",
        env.vcs.calls()
    );
    assert_eq!(
        env.store.records.lock().expect("记录锁不可中毒").len(),
        0,
        "create 补偿链按 id 删本次建档（零残留）"
    );
}

/// 删笔记失败显式 Err（异常）：create 成功后预置同名目录占位使
/// `fs::remove_file` 失败 → `Err` 携 change id + 「笔记全文已在 change
/// explore.md 留底」回写指引（R1 残留显式呈现）。
#[test]
fn promote_删笔记失败显式err携change_id与回写指引() {
    let env = Env::new("remove-fail");
    let note = "# 标题\n正文";
    // 同名目录占位：`remove_file` 对目录必失败（不经 mock）
    let blocked = env.note_path("blocked-topic");
    fs::create_dir_all(&blocked).expect("预置同名目录占位失败");

    let error = env
        .promote("blocked-topic", note, "标题")
        .expect_err("删笔记失败应显式 Err");

    let change_id = env.store.created_record().id;
    assert!(
        error.contains(&change_id),
        "Err 携 change id（残留对象显式呈现），实际: {error}"
    );
    assert!(
        error.contains("笔记全文已在 change explore.md 留底"),
        "Err 携回写指引，实际: {error}"
    );
    // change 已建成（create 成功半边成立）
    assert!(
        env.change_dir("blocked-topic").join("explore.md").is_file(),
        "worktree 内 explore.md 已留底"
    );
    assert!(blocked.is_dir(), "主仓占位目录未被删除（残留显式）");
    assert_eq!(
        env.store.records.lock().expect("记录锁不可中毒").len(),
        1,
        "建档保留（create 成功，删笔记失败不回滚 change）"
    );
}

// ---------------------------------------------------------------------------
// 出线契约与 sync 纪律
// ---------------------------------------------------------------------------

/// PromoteOutcome 线形（边界）：返回 DTO 两字段 serde camelCase 出线
/// （`changeId` / `changeName`），键集恰两键。
#[test]
fn promote_outcome线形camel_case两键() {
    let outcome = PromoteOutcome {
        change_id: "0198f7a0-0000-7000-8000-0000000000a1".to_owned(),
        change_name: "api-retry".to_owned(),
    };
    let value = serde_json::to_value(&outcome).expect("serde 出线应成功");
    assert_eq!(
        value["changeId"],
        serde_json::json!("0198f7a0-0000-7000-8000-0000000000a1")
    );
    assert_eq!(value["changeName"], serde_json::json!("api-retry"));
    assert_eq!(
        value.as_object().map(|map| map.len()),
        Some(2),
        "线形恰两键（changeId / changeName）"
    );
    // 往返无损
    let back: PromoteOutcome = serde_json::from_value(value).expect("反序列化应成功");
    assert_eq!(back, outcome, "两字段往返逐字段相等");
}

/// sync 零 Tauri / 零 tokio 纪律（边界）：`promote_explore` 为 sync 函数——在
/// 无 tokio 运行时的裸 std 线程内直调可用（命令层经 `spawn_blocking` 驱动）。
#[test]
fn promote_explore_sync零tokio上下文依赖() {
    let handle = std::thread::spawn(|| {
        let env = Env::new("sync-thread");
        env.seed_note("sync-topic", "# 标题\n正文");
        env.promote("sync-topic", "# 标题\n正文", "标题")
            .map(|outcome| outcome.change_name)
    });

    let change_name = handle
        .join()
        .expect("裸线程应正常结束")
        .expect("promote 应成功");
    assert_eq!(change_name, "sync-topic", "无 tokio 上下文亦可直调");
}
