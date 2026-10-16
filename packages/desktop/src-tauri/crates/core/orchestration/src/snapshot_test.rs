use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use foundation::layout::resolve;
use store::Store;
use workflow::model::{ChecklistItem, Verdict};
use workflow::queries::{change_detail, ChangeSource};
use workflow::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand,
    PhaseStartState, PhaseStateRecord, RunEdgeStateRecord, RunFinishCommand, RunNodeStateRecord,
    RunStartCommand, RunStateRecord, StepCommand, StepStateRecord, StoreFault,
};

use crate::port::WorkflowSnapshotPort;
use crate::snapshot::StoreSnapshot;

// ---------------------------------------------------------------------------
// 装置：tempdir 磁盘根 + 真实 workspace 库 + store 种子
// ---------------------------------------------------------------------------

/// 种子基准时刻：2026-10-01T08:00:00Z 定值 UTC unix 毫秒（确定性断言面）。
const TS_BASE: i64 = 1_790_841_600_000;

/// 固定 change **id** 字面量（一切身份寻址入参——`detail(root, id)` 第二参）。
const CHANGE_ID: &str = "7c3f9a20-5b41-4e8d-a6f2-0d1b2c3e4f50";

/// 展示名（磁盘目录面供给值：`openspec/changes/<name>`——id ≠ name 形态下
/// id 寻址 / name 出线逐点可辨）。
const NAME: &str = "demo-change";

/// 临时 workspace 根 RAII（沿 core/workflow detail_test 装置先例）：测试结束
/// 自动清理。
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "orchestration-snapshot-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// 根路径字符串（`detail` 的 root 寻址入参形态）。
    fn root_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    /// 预置一个 change 目录（磁盘面按 **name** 目录名——产物发现面；状态面单源
    /// workspace 库，workflow.json 零产出）。
    fn change(&self, name: &str) {
        let dir = self.0.join("openspec/changes").join(name);
        fs::create_dir_all(dir.join("specs/demo-capability")).expect("创建 change 骨架目录失败");
        fs::write(dir.join("proposal.md"), "# 提案\n").expect("写 proposal.md 失败");
        fs::write(dir.join("tasks.md"), "- [ ] 任务\n").expect("写 tasks.md 失败");
        fs::write(
            dir.join("specs/demo-capability/spec.md"),
            "# 能力\n## 需求\n",
        )
        .expect("写 spec.md 失败");
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 真实 workspace 库装置：tempfile db 文件（store crate dev-dep 真件组合）。
struct TestDb {
    /// store 句柄（字段声明先于 db 目录：drop 序先关库再删目录，Windows 句柄
    /// 纪律）
    store: Arc<Store>,
    _db_dir: tempfile::TempDir,
}

impl TestDb {
    fn open(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("orchestration-snapshot-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let store =
            Store::open_workspace(&db_dir.path().join("ws.redb")).expect("打开 workspace 库应成功");
        Self {
            store: Arc::new(store),
            _db_dir: db_dir,
        }
    }

    /// store 缝注入面（`Arc<dyn ChangeStateStore>` 类型擦除——组合根同式装配）。
    fn store_arc(&self) -> Arc<dyn ChangeStateStore> {
        Arc::clone(&self.store) as Arc<dyn ChangeStateStore>
    }
}

/// 建档种子：workflow_type requirement、active 起步——id 归键、name 独立
/// （展示属性；id / name 均不随寻址漂移）。
fn seed_change(store: &Store, id: &str, name: &str) {
    store
        .create_change_record(ChangeStateRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            title: name.to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: TS_BASE,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("建档种子应成功");
}

/// 评估条目种子：开相 + 落账一条（store change 域操作面种子路径，按 id 归键）。
fn seed_entry(
    store: &Store,
    change_id: &str,
    phase: &str,
    verdict: Verdict,
    report: &str,
    checklist: Vec<ChecklistItem>,
    ts: i64,
) {
    let started = store
        .start_change_phase(change_id, phase, ts)
        .expect("开相种子应成功");
    store
        .log_change_phase(&PhaseLogCommand {
            change_id: change_id.to_owned(),
            phase: phase.to_owned(),
            verdict,
            report: report.to_owned(),
            skipped: false,
            checklist,
            executor_session_id: Some(format!("sess-exec-{phase}")),
            evaluator_session_id: Some(format!("sess-eval-{phase}")),
            decision_session_id: None,
            start_at: Some(started.start_at),
            timestamp: ts + 30_000,
        })
        .expect("落账种子应成功");
}

/// 建档 + 两条相位条目种子（proposal pass 带 checklist 行 / implement fail）
/// + dev-design 开相在途（active_phase 状态面在场）。
fn seed_documented_change(db: &TestDb) {
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "proposal",
        Verdict::Pass,
        "提案通过",
        vec![ChecklistItem {
            item: "验收标准在场".to_owned(),
            pass: true,
            evidence: "proposal.md 含验收节".to_owned(),
        }],
        TS_BASE,
    );
    seed_entry(
        db.store.as_ref(),
        CHANGE_ID,
        "implement",
        Verdict::Fail,
        "首轮未过",
        Vec::new(),
        TS_BASE + 120_000,
    );
    db.store
        .start_change_phase(CHANGE_ID, "dev-design", TS_BASE + 180_000)
        .expect("开相种子应成功");
}

// ---------------------------------------------------------------------------
// db 读源 detail：按 id 寻址 + 与 queries 直调 serde 等值（换血不加工）
// ---------------------------------------------------------------------------

/// db 读源 detail：store 种子（建档 + 相位行 + active_phase）→ detail(root,
/// id) 与 queries::change_detail 直调结果 serde 等值（port 实现零加工换血）；
/// 字段面抽查身份锚（id 寻址出线 / name 自记录直读）/ 状态面 / 流水线重组
///（AC-5 快照读源半边）。
#[test]
fn db读源detail按id寻址_与queries直调serde等值() {
    let root = TempRoot::new("db-source");
    root.change(NAME);
    let db = TestDb::open("db-source");
    seed_documented_change(&db);
    let root_str = root.root_str();

    let snapshot = StoreSnapshot::new(root_str.clone(), db.store_arc());
    assert_eq!(snapshot.root, root_str, "构造绑定根原样承接");

    let detail = snapshot
        .detail(&root_str, CHANGE_ID)
        .expect("建档 id 应装配成功");

    // 身份锚：id 寻址命中 / name 自记录直读（id ≠ name 形态下零混同）
    assert_eq!(detail.id, CHANGE_ID, "详情身份锚 = 寻址 id");
    assert_eq!(detail.name, NAME, "name 自记录直读（展示属性）");

    // 与 queries::change_detail 直调逐字节 serde 等值（换血不加工）
    let direct = change_detail(&resolve(&root.0), db.store.as_ref(), CHANGE_ID)
        .expect("直调应 Ok")
        .expect("直调应命中同记录");
    assert_eq!(
        serde_json::to_value(&detail).expect("线面序列化应成功"),
        serde_json::to_value(&direct).expect("直调序列化应成功"),
        "detail 与 queries 直调 serde 等值"
    );

    // 字段面抽查：建档判别 / 状态面 / 9 站流水线重组（requirement 表相位序）
    assert_eq!(detail.source, ChangeSource::Active);
    assert_eq!(detail.status, Some(ChangeStatus::Active));
    assert_eq!(
        detail.created.as_deref(),
        Some("2026-10-01"),
        "created 取 db created_at 日期（UTC 日界口径）"
    );
    assert_eq!(
        detail.pipeline.len(),
        9,
        "requirement 9 站全量流水线（末列 archive；列序自 phase_table 出线）"
    );
    let proposal = &detail.pipeline[0];
    assert_eq!(proposal.attempts.len(), 1);
    assert_eq!(proposal.attempts[0].verdict, Verdict::Pass);
    assert_eq!(proposal.attempts[0].checklist.len(), 1);
    assert_eq!(proposal.attempts[0].checklist[0].item, "验收标准在场");
    assert_eq!(
        proposal.attempts[0].executor_session_id.as_deref(),
        Some("sess-exec-proposal"),
        "会话槽位三列自 PhaseRecord 直读透出"
    );
    let implement = &detail.pipeline[3];
    assert_eq!(implement.attempts[0].verdict, Verdict::Fail);
    let active = detail
        .active_phase
        .as_ref()
        .expect("开相在途应有 active_phase");
    assert_eq!(active.phase, "dev-design");
    assert_eq!(active.attempt, 1);
    assert!(
        active.start_at.is_some(),
        "start_at ISO 串出线（millis 转换收 queries 单点）"
    );

    // 产物清单：骨架文档树进入（磁盘扫描保留为产物发现——name 目录名）
    assert!(
        detail.artifacts.iter().any(|a| a.kind == "markdown-doc"),
        "openspec 骨架文档应进入产物清单"
    );
}

// ---------------------------------------------------------------------------
// 未知 id：显式 Err（原「文档形态 db 缺记录磁盘在场走通不 err」反转——未找到
// 降级；MUST NOT 回退磁盘目录解析）
// ---------------------------------------------------------------------------

/// 未建档 id → 显式 `Err` 记因（原文档形态「磁盘在场走通不 Err」行反转）：
/// 磁盘 change 目录在场而 db 无该 id 记录 → `Err` 携 id（零磁盘目录解析回退、
/// 零空流水线文档形态、零 workflow.json 读取）；两树皆无 id → 同式 `Err`
/// （未知 id 未找到降级单点）。
#[test]
fn 未知id显式err_未建档不回退磁盘目录解析() {
    let root = TempRoot::new("unknown-id");
    // 磁盘在场（存量 CLI 建目录形态）但 db 无该 id 记录
    root.change("legacy-cli-change");
    let db = TestDb::open("unknown-id");
    seed_documented_change(&db);
    let root_str = root.root_str();

    let snapshot = StoreSnapshot::new(root_str.clone(), db.store_arc());

    // 磁盘目录名 == 寻址串：db 缺记录 → Err（不回退磁盘目录解析成文档形态）
    let err = snapshot
        .detail(&root_str, "legacy-cli-change")
        .expect_err("未建档 id 应显式 Err");
    assert!(
        err.contains("legacy-cli-change") && err.contains("change 不存在"),
        "miss 记因显式携带 id: {err}"
    );

    // 磁盘目录名 ≠ 寻址 id：登记 id 之外的未建档串同式 Err
    let err = snapshot
        .detail(&root_str, "unknown-id-2")
        .expect_err("未建档 id 应显式 Err");
    assert!(err.contains("unknown-id-2"), "miss 记因显式携带 id: {err}");

    // 建档 id 不受误伤（对照面）
    assert!(
        snapshot.detail(&root_str, CHANGE_ID).is_ok(),
        "在场建档 id 可达详情"
    );
}

// ---------------------------------------------------------------------------
// port 契约持衡：trait object 装配 + 未知 id / root 失配显式 Err
// ---------------------------------------------------------------------------

#[test]
fn port契约_traitobject装配且未知change与root失配显式err() {
    let root = TempRoot::new("port-contract");
    root.change(NAME);
    let db = TestDb::open("port-contract");
    seed_change(db.store.as_ref(), CHANGE_ID, NAME);
    let root_str = root.root_str();

    // walker 消费面同式：trait object 装配后经 `dyn` 调用可达
    let port: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root_str.clone(), db.store_arc()));
    assert!(
        port.detail(&root_str, CHANGE_ID).is_ok(),
        "在场 change 可达"
    );

    // 未知 change（两树均无目录且 db 无记录）→ change_detail None → Err
    let err = port
        .detail(&root_str, "不存在的-change")
        .expect_err("未知 change 应显式 Err");
    assert!(
        err.contains("不存在的-change") && err.contains("change 不存在"),
        "miss 记因显式携带 change id: {err}"
    );

    // root 失配：layout 无此 change 目录，但 db 建档记录在场 → D12 建档记录
    // 恒可达详情（定位 miss → dir 缺席、产物清单空、状态面在）→ Ok 而非 Err
    let mismatched = port
        .detail("/tmp/不存在的根", CHANGE_ID)
        .expect("建档记录恒可达（db 状态面权威，定位 miss 不虚构 None）");
    assert_eq!(mismatched.status, Some(ChangeStatus::Active), "状态面在");
    assert!(mismatched.artifacts.is_empty(), "定位 miss 产物清单空");

    // 在场 change 不受误伤（对照面）
    assert!(port.detail(&root_str, CHANGE_ID).is_ok());
}

// ---------------------------------------------------------------------------
// store 读 Err → 错误串透传（取代误报「不存在」）与 None 分支回归（假件注入缝
// ——test-design Mock 表双轨之②，沿 workflow detail_test DetailStore 先例）
// ---------------------------------------------------------------------------

/// 进程内假件 store（`get_change` 可编程应答——Err / None 面唯一可达注入缝）；
/// 其余 unimplemented（越权触写即 panic）。
struct FaultStore {
    get_change_result: Mutex<Result<Option<ChangeStateRecord>, StoreFault>>,
}

impl FaultStore {
    /// `Ok(None)` 形态（未知 id 与真实库未建档同语义的两形态之一）。
    fn none() -> Self {
        Self { get_change_result: Mutex::new(Ok(None)) }
    }

    /// `Err` 形态（三 kind 任意变体——错误串透传断言面）。
    fn fault(fault: StoreFault) -> Self {
        Self { get_change_result: Mutex::new(Err(fault)) }
    }
}

impl ChangeStateStore for FaultStore {
    fn get_change(&self, _id: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        self.get_change_result
            .lock()
            .expect("fault 锁不可中毒")
            .clone()
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

/// 假件 `get_change` 返回 `Err(StoreFault::Db)` → `Err("db: …注入串")` 错误串
/// 逐字透传（`fault.to_string()` 单点映射）；MUST NOT 含「change 不存在」误报
/// 文案（Err 与 None 分流出线不混同——错误保真度提升回归）。
#[test]
fn store读err_错误串逐字透传不再误报不存在() {
    let root = TempRoot::new("fault-err");
    root.change(NAME);
    let root_str = root.root_str();
    let injected = StoreFault::Db("注入失败：读事务不可用".to_owned());
    let snapshot = StoreSnapshot::new(root_str.clone(), Arc::new(FaultStore::fault(injected)));

    let err = snapshot
        .detail(&root_str, CHANGE_ID)
        .expect_err("store 读 Err 应透传为 Err");

    assert_eq!(
        err, "db: 注入失败：读事务不可用",
        "错误串逐字（Display 前缀 `db: ` 单点映射，零改写零包装）"
    );
    assert!(
        !err.contains("change 不存在"),
        "MUST NOT 误报「change 不存在」（Err 与 None 分流出线不混同）: {err}"
    );
}

/// `StoreFault` 三 kind（`Db` / `Conflict` / `NotFound`）逐变体注入 → 各自
/// Display 前缀（`db: ` / `conflict: ` / `not_found: `）逐字透传（D4 Display
/// 单点回归——port 面零第二处前缀组装）。
#[test]
fn store_fault三kind_display前缀逐字透传() {
    for (tag, fault) in [
        ("db", StoreFault::Db("db 面注入".to_owned())),
        ("conflict", StoreFault::Conflict("conflict 面注入".to_owned())),
        ("not_found", StoreFault::NotFound("not_found 面注入".to_owned())),
    ] {
        let root = TempRoot::new(&format!("fault-kind-{tag}"));
        root.change(NAME);
        let root_str = root.root_str();
        let snapshot = StoreSnapshot::new(root_str.clone(), Arc::new(FaultStore::fault(fault)));

        let err = match snapshot.detail(&root_str, CHANGE_ID) {
            Err(message) => message,
            Ok(_) => panic!("{tag} 变体注入应 Err 透传"),
        };
        assert_eq!(
            err,
            format!("{tag}: {tag} 面注入"),
            "Display 前缀 `{tag}: ` 逐字透传（错误串 = fault.to_string()）"
        );
    }
}

/// 未知 id（假件 `get_change` → `Ok(None)` 形态）→ 既有
/// `Err("change 不存在: {id}")` 文案逐字不变（None 分支零改动回归——与真实库
/// 未建档同语义的两形态对拍）。
#[test]
fn 假件ok_none未知id_不存在文案逐字不变() {
    let root = TempRoot::new("fake-none");
    root.change(NAME);
    let root_str = root.root_str();
    let snapshot = StoreSnapshot::new(root_str.clone(), Arc::new(FaultStore::none()));

    let err = snapshot
        .detail(&root_str, "fake-ghost-id")
        .expect_err("未知 id 应显式 Err（真未找到语义）");

    assert_eq!(
        err, "change 不存在: fake-ghost-id",
        "None 分支文案逐字不变（零 Err 混同、零改写）"
    );
}

/// trait object 装配（`Arc<dyn WorkflowSnapshotPort>`）下 Err 分支同样成立
///（port 契约持衡——walker 消费面同式调用）；Ok / None 两分支既有用例已锚，
/// 此处补 Err 面的 `dyn` 调用等价性。
#[test]
fn port契约_traitobject装配下err分支同式成立() {
    let root = TempRoot::new("port-fault");
    root.change(NAME);
    let root_str = root.root_str();
    let injected = StoreFault::Conflict("注入失败：trait object 面".to_owned());
    let port: Arc<dyn WorkflowSnapshotPort> = Arc::new(StoreSnapshot::new(
        root_str.clone(),
        Arc::new(FaultStore::fault(injected)),
    ));

    let err = port
        .detail(&root_str, CHANGE_ID)
        .expect_err("trait object 调用下 Err 透传同样可达");

    assert_eq!(
        err, "conflict: 注入失败：trait object 面",
        "trait object 装配下错误串逐字透传（port 契约零特判）"
    );
}
