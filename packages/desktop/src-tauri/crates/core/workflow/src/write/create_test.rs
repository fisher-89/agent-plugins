//! `write::create` 的单元测试（test-design「create.rs -> create_test.rs」节）：
//! create 建档三合一双写（D5）——目录 + explore.md（goal 原文）+ ChangeRecord
//! 建档；冲突双检查全 IO 前置（目录已存在 / db 同名 active 拒绝零副作用）；
//! kebab-case / goal 校验保留；fs 失败补偿删除本次建档（真实 fs 注入：预置
//! 目录位文件占位）+ 补偿后 db 零残留；补偿再失败呈现残留记录名；created
//! 出线取 db created_at 日期；成功后清单立即可见（组合行）；目录树零
//! workflow.json 产出（双向墙写半边）。
//!
//! Mock策略（test-design 本节 Mock 表）：db 半边以进程内假件实现
//! [`ChangeStateStore`]（捕获建档 / 补偿删除调用 + 可编程补偿故障；真实
//! tempfile Store 组合行收 tests/corpus_golden_test.rs 集成面——workflow 自环
//! dev-dep 在 lib-test 与普通 lib 双工件下类型不统一，见变更报告）；文件系统
//! 真实 tempdir + 预置目录位文件占位注入写失败（不经 mock）。「当日」断言用
//! 调用前后 UTC 日期并集界，零 wall-clock 等值比较。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use foundation::layout::{resolve, Layout};
use time::OffsetDateTime;

use super::create;
use super::CreateOutcome;
use crate::queries::list_changes;
use crate::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStateRecord, StepCommand, StepStateRecord, StoreFault,
};

// ---------------------------------------------------------------------------
// 装置：真实 tempdir workspace 根（fs 半边全真实）+ 进程内假件 store
// ---------------------------------------------------------------------------

struct Env {
    root: PathBuf,
    store: CreateStore,
    layout: Layout,
}

impl Env {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "workflow-create-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&root);
        let layout = resolve(&root);
        Self {
            root,
            store: CreateStore::new(),
            layout,
        }
    }

    fn create(&self, name: &str, goal: &str) -> Result<CreateOutcome, String> {
        create(&self.layout, &self.store, name, goal)
    }

    fn change_dir(&self, name: &str) -> PathBuf {
        self.layout.changes_root.join(name)
    }

    fn explore_bytes(&self, name: &str) -> Vec<u8> {
        fs::read(self.change_dir(name).join("explore.md")).expect("读 explore.md 失败")
    }

    /// changes_root 下现存目录名（根不存在即空集）——「零产生」观察面。
    fn active_dir_names(&self) -> Vec<String> {
        dir_names(&self.layout.changes_root)
    }

    /// changes_root 全树下文件名集合（递归；根不存在即空集）——「零
    /// workflow.json 产出」观察面。
    fn tree_file_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        collect_file_names(&self.layout.changes_root, &mut names);
        names
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
// 正向：三合一成功
// ---------------------------------------------------------------------------

/// 成功 → 目录创建 + explore.md 含 goal 原文 + ChangeRecord 建档
///（workflow_type=requirement、active、零 active_phase）；目录树内零
/// workflow.json 产出（双向墙写半边——AC-6）。
#[test]
fn 三合一成功_目录与explore与db建档且零workflow_json产出() {
    let env = Env::new("happy");
    let before = utc_date_today();

    let outcome = env
        .create("fix-bug", "修复登录重试的竞态问题")
        .expect("合法输入应 Ok");

    let after = utc_date_today();
    assert_eq!(outcome.name, "fix-bug");
    assert_today(&outcome.created, &before, &after);

    // fs 半边：目录 + explore.md 落 goal 原文
    assert!(env.change_dir("fix-bug").is_dir(), "change 目录创建");
    assert_eq!(env.explore_bytes("fix-bug"), "修复登录重试的竞态问题".as_bytes());

    // db 半边：ChangeRecord 建档（workflow_type 随表）
    let record = env.store.find("fix-bug").expect("建档记录应在场");
    assert_eq!(record.name, "fix-bug");
    assert_eq!(record.workflow_type, "requirement");
    assert_eq!(record.status, ChangeStatus::Active);
    assert_eq!(record.archived_at, None);
    assert!(record.active_phase.is_none(), "建档默认零 active_phase");

    // 双向墙：目录树内零 workflow.json 产出
    let files = env.tree_file_names();
    assert!(
        !files.iter().any(|name| name == "workflow.json"),
        "目录树零 workflow.json 产出，实际: {files:?}"
    );
    assert_eq!(files, vec!["explore.md"], "目录树仅 explore.md 一文件");
}

/// 空白树深层建树：调用前 changes_root 全链不存在，建树后目录链完整
///（AC-1「空白树」字面）。
#[test]
fn 空白树深层建树_全链不存在时建全树() {
    let env = Env::new("blank-tree");
    assert!(
        !env.layout.changes_root.exists(),
        "前置：changes_root 全链不存在"
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

    assert_eq!(env.explore_bytes("rich-goal"), goal.as_bytes(), "写面零 trim 保真");
}

// ---------------------------------------------------------------------------
// 异常：冲突双检查前置（D5 全 IO 前置）
// ---------------------------------------------------------------------------

/// 目录已存在 → 显式 `Err` 且 db 零建档（零副作用）。
#[test]
fn 目录已存在拒绝且db零建档() {
    let env = Env::new("dir-taken");
    fs::create_dir_all(env.change_dir("taken")).expect("预置目录失败");
    fs::write(env.change_dir("taken").join("proposal.md"), "# 既有").expect("预置文件失败");

    let error = env.create("taken", "新建 goal").expect_err("同名目录应 Err");

    assert!(
        error.contains("已存在") && error.contains("taken"),
        "错误归因已存在并携带目录语境，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 0, "db 零建档");
    assert!(
        env.change_dir("taken").join("proposal.md").is_file(),
        "既有目录零触碰"
    );
}

/// db 已有同名 active → 显式 `Err` 且零目录创建（检查全 IO 前置——D5）。
#[test]
fn db同名active拒绝且零目录创建() {
    let env = Env::new("db-taken");
    env.store.seed_active("taken");

    let error = env.create("taken", "新建 goal").expect_err("同名 active 应 Err");

    assert!(
        error.contains("已存在") && error.contains("taken"),
        "错误归因同名建档冲突，实际: {error}"
    );
    assert!(
        env.active_dir_names().is_empty(),
        "零目录创建（检查全 IO 前置），实际: {:?}",
        env.active_dir_names()
    );
    assert_eq!(env.store.create_call_count(), 0, "建档调用零下发");
}

// ---------------------------------------------------------------------------
// 异常：校验保留（kebab-case / 长度 / goal 空白，全 IO 前置）
// ---------------------------------------------------------------------------

/// 非法名（kebab-case 违例 / 非单分量）与空白 goal → `Err` 且零目录零建档；
/// 校验顺序名称优先（D4 顺序锚）。
#[test]
fn 非法名与空白goal拒绝且零目录零建档() {
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
        assert!(env.create(name, "拒绝面 goal").is_err(), "非法名 {name:?} 应 Err");
    }
    for goal in ["", "   ", "\n\t"] {
        assert!(env.create("fix-bug", goal).is_err(), "空白 goal {goal:?} 应 Err");
    }

    assert!(
        env.active_dir_names().is_empty(),
        "零目录（校验全 IO 前置），实际: {:?}",
        env.active_dir_names()
    );
    assert_eq!(env.store.create_call_count(), 0, "db 零建档");

    // 校验顺序：非法名 + 空白 goal 同投，Err 归因名称校验
    let error = env.create("Bad_Name", "   ").expect_err("非法名应 Err");
    assert!(
        error.contains("kebab-case") && !error.contains("goal"),
        "归因名称校验（名称先决），实际: {error}"
    );
}

/// 超 128 字符拒绝：129 字符合法字符集名 Err 且零产生（AC-3）。
#[test]
fn 超128字符拒绝且零产生() {
    let env = Env::new("width-129");
    let name = format!("a{}", "b".repeat(128));
    assert_eq!(name.len(), 129, "前置：129 字节");

    let error = env.create(&name, "越界 goal").expect_err("超 128 应 Err");
    assert!(error.contains("128"), "错误归因长度限制，实际: {error}");
    assert!(env.active_dir_names().is_empty(), "零产生");
    assert_eq!(env.store.create_call_count(), 0, "db 零建档");
}

// ---------------------------------------------------------------------------
// 异常：fs 失败补偿（D5）——真实 fs 注入写失败
// ---------------------------------------------------------------------------

/// 预置 changes_root 为文件占位（真实 fs 注入写失败——目录位占位会先被
/// 「目录已存在」前置检查拦截，故占位打在 create_dir_all 的路径分量上）→
/// 补偿删除本次新插建档记录（独立写面调用只删自插行）；补偿后 db 零残留。
#[test]
fn fs失败补偿删除本次建档且db零残留() {
    let env = Env::new("fs-fail-compensate");
    fs::create_dir_all(env.layout.changes_root.parent().expect("域根应存在"))
        .expect("预置域根失败");
    fs::write(&env.layout.changes_root, "changes_root 文件占位").expect("预置占位失败");

    let error = env.create("fix-bug", "补偿路径 goal").expect_err("fs 半边失败应 Err");

    assert!(
        error.contains("补偿"),
        "Err 呈现补偿回滚事实，实际: {error}"
    );
    assert!(
        env.store.find("fix-bug").is_none(),
        "补偿删除本次自插行（db 零残留）"
    );
    assert_eq!(
        env.store.deletes.lock().expect("删除锁不可中毒").as_slice(),
        ["fix-bug"],
        "补偿删除恰针对本次自插行调用一次"
    );
}

/// 补偿再失败（双故障角落）：假件 store 补偿删除返回 `Err` → `Err` 呈现
/// 残留记录名（不静默自愈——D5）。
#[test]
fn 补偿再失败呈现残留记录名() {
    let env = Env::new("compensate-fault");
    env.store
        .set_delete_fault(StoreFault::Db("注入的补偿删除故障".to_owned()));
    fs::create_dir_all(env.layout.changes_root.parent().expect("域根应存在"))
        .expect("预置域根失败");
    fs::write(&env.layout.changes_root, "changes_root 文件占位").expect("预置占位失败");

    let error = env
        .create("orphan-name", "双故障角落 goal")
        .expect_err("补偿再失败应 Err");

    assert!(
        error.contains("残留") && error.contains("orphan-name"),
        "Err 呈现残留记录名（不静默自愈），实际: {error}"
    );
    assert!(
        error.contains("注入的补偿删除故障"),
        "Err 同时呈现补偿失败记因，实际: {error}"
    );
    assert_eq!(env.store.create_call_count(), 1, "建档先行恰好一次");
    // 双故障角落：记录滞留假件（「目录在而记录缺」被禁破口的反面——显式报错）
    assert!(env.store.find("orphan-name").is_some(), "残留记录在场（随下次同名建档显式暴露）");
}

// ---------------------------------------------------------------------------
// 边界：created 出线 + 清单立即可见（组合行）+ CreateOutcome 线形状
// ---------------------------------------------------------------------------

/// CreateOutcome.created 取 db created_at 日期（UTC 日界口径）；成功后立即
/// 经 list_changes 可见（db 形态状态面在场——AC-6「成功即可见」半边）。
#[test]
fn created出线且成功立即可见() {
    let env = Env::new("visible");
    let before = utc_date_today();

    let outcome = env.create("combo-visible", "组合用例 goal").expect("create 应 Ok");
    let after = utc_date_today();

    assert_today(&outcome.created, &before, &after);

    // 清单立即可见（db 形态：status / created 状态面在场）
    let list = list_changes(&env.layout, &env.store);
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

/// CreateOutcome serde 线形状：序列化恰 `{"name":…,"created":…}` 两键
///（camelCase、零磁盘路径字段——AC-4 DTO 纪律的类型面）。
#[test]
fn create_outcome_serde线形状恰两键零磁盘路径字段() {
    let outcome = CreateOutcome {
        name: "fix-bug".to_owned(),
        created: "2026-10-02".to_owned(),
    };

    let value = serde_json::to_value(&outcome).expect("序列化失败");
    let object = value.as_object().expect("序列化为对象");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["created", "name"], "恰两键且零磁盘路径字段");
    assert_eq!(object["name"], "fix-bug");
    assert_eq!(object["created"], "2026-10-02");
}
