use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use foundation::layout::resolve;
use store::Store;
use workflow::model::{ChecklistItem, Verdict};
use workflow::queries::{change_detail, ChangeSource};
use workflow::state::{ChangeStateRecord, ChangeStateStore, ChangeStatus, PhaseLogCommand};

use crate::port::WorkflowSnapshotPort;
use crate::snapshot::StoreSnapshot;

// ---------------------------------------------------------------------------
// 装置：tempdir 磁盘根 + 真实 workspace 库 + store 种子
// ---------------------------------------------------------------------------

/// 种子基准时刻：2026-10-01T08:00:00Z 定值 UTC unix 毫秒（确定性断言面）。
const TS_BASE: i64 = 1_790_841_600_000;

const CHANGE: &str = "demo-change";

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

    /// 预置一个 change 目录：openspec 骨架文档（产物发现面）。磁盘仅产物树
    /// ——状态面单源 workspace 库，workflow.json 零产出（存量 CLI 惰性字节
    /// 样本另有退役回归行显式写入）。
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

    /// 读取 change 目录内任意文件字节（只读性 / 惰性样本比对面）。
    fn file_bytes(&self, name: &str, file: &str) -> Vec<u8> {
        fs::read(self.0.join("openspec/changes").join(name).join(file)).expect("读文件失败")
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

/// 建档种子：workflow_type requirement、active 起步。
fn seed_change(store: &Store, name: &str) {
    store
        .create_change_record(ChangeStateRecord {
            name: name.to_owned(),
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

/// 评估条目种子：开相 + 落账一条（store change 域操作面种子路径）。
fn seed_entry(
    store: &Store,
    change: &str,
    phase: &str,
    verdict: Verdict,
    report: &str,
    checklist: Vec<ChecklistItem>,
    ts: i64,
) {
    let started = store
        .start_change_phase(change, phase, ts)
        .expect("开相种子应成功");
    store
        .log_change_phase(&PhaseLogCommand {
            change: change.to_owned(),
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
    seed_change(db.store.as_ref(), CHANGE);
    seed_entry(
        db.store.as_ref(),
        CHANGE,
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
        CHANGE,
        "implement",
        Verdict::Fail,
        "首轮未过",
        Vec::new(),
        TS_BASE + 120_000,
    );
    db.store
        .start_change_phase(CHANGE, "dev-design", TS_BASE + 180_000)
        .expect("开相种子应成功");
}

// ---------------------------------------------------------------------------
// db 读源 detail：与 queries 直调 serde 等值（换血不加工）
// ---------------------------------------------------------------------------

/// db 读源 detail：store 种子（建档 + 相位行 + active_phase）→ detail(root,
/// change) 与 queries::change_detail 直调结果 serde 等值（port 实现零加工换血
/// ——AC-5 快照读源半边）；字段面抽查建档判别 / 状态面 / 流水线重组。
#[test]
fn db读源detail与queries直调serde等值() {
    let root = TempRoot::new("db-source");
    root.change(CHANGE);
    let db = TestDb::open("db-source");
    seed_documented_change(&db);
    let root_str = root.root_str();

    let snapshot = StoreSnapshot::new(root_str.clone(), db.store_arc());
    assert_eq!(snapshot.root, root_str, "构造绑定根原样承接");

    let detail = snapshot
        .detail(&root_str, CHANGE)
        .expect("建档 change 应装配成功");

    // 与 queries::change_detail 直调逐字节 serde 等值（换血不加工）
    let direct =
        change_detail(&resolve(&root.0), db.store.as_ref(), CHANGE).expect("直调应命中同记录");
    assert_eq!(
        serde_json::to_value(&detail).expect("线面序列化应成功"),
        serde_json::to_value(&direct).expect("直调序列化应成功"),
        "detail 与 queries 直调 serde 等值"
    );

    // 字段面抽查：身份 / 建档判别 / 状态面 / 9 站流水线重组
    assert_eq!(detail.name, CHANGE);
    assert_eq!(detail.source, ChangeSource::Active);
    assert_eq!(detail.status, Some(ChangeStatus::Active));
    assert_eq!(
        detail.created.as_deref(),
        Some("2026-10-01"),
        "created 取 db created_at 日期（UTC 日界口径）"
    );
    assert_eq!(detail.pipeline.len(), 9, "固定 9 站全量流水线");
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

    // 产物清单：骨架文档树进入（磁盘扫描保留为产物发现）
    assert!(
        detail.artifacts.iter().any(|a| a.kind == "markdown-doc"),
        "openspec 骨架文档应进入产物清单"
    );
}

/// 文档形态（db 缺记录磁盘在场）：detail 走通不 Err——空流水线 + 产物清单，
/// status / created / active_phase 状态面缺位（None = 文档形态契约）。
#[test]
fn 文档形态db缺记录磁盘在场走通不err() {
    let root = TempRoot::new("doc-form");
    root.change("legacy-cli-change");
    let db = TestDb::open("doc-form");
    let root_str = root.root_str();

    let snapshot = StoreSnapshot::new(root_str.clone(), db.store_arc());
    let detail = snapshot
        .detail(&root_str, "legacy-cli-change")
        .expect("文档形态走通不 Err");

    assert_eq!(detail.name, "legacy-cli-change");
    assert_eq!(detail.source, ChangeSource::Active);
    assert_eq!(detail.status, None, "db 缺记录 → status None（建档判别面）");
    assert!(detail.pipeline.is_empty(), "文档形态零状态面（空流水线）");
    assert!(detail.active_phase.is_none());
    assert_eq!(detail.created, None, "active 树文档形态 created 无回退源");
    assert!(
        detail.artifacts.iter().any(|a| a.kind == "markdown-doc"),
        "产物清单照常装配（磁盘仅产物发现）"
    );
}

// ---------------------------------------------------------------------------
// port 契约持衡：trait object 装配 + 未知 change / root 失配显式 Err
// ---------------------------------------------------------------------------

/// WorkflowSnapshotPort trait object 装配（walker 消费面）可达；未知 change
/// 与 root 失配（layout 无此 change 目录）→ `Err` 显式携 change 名——port
/// 契约不随读源换血漂移。
#[test]
fn port契约_traitobject装配且未知change与root失配显式err() {
    let root = TempRoot::new("port-contract");
    root.change(CHANGE);
    let db = TestDb::open("port-contract");
    seed_change(db.store.as_ref(), CHANGE);
    let root_str = root.root_str();

    // walker 消费面同式：trait object 装配后经 `dyn` 调用可达
    let port: Arc<dyn WorkflowSnapshotPort> =
        Arc::new(StoreSnapshot::new(root_str.clone(), db.store_arc()));
    assert!(port.detail(&root_str, CHANGE).is_ok(), "在场 change 可达");

    // 未知 change（两树均无目录且 db 无记录）→ change_detail None → Err
    let err = port
        .detail(&root_str, "不存在的-change")
        .expect_err("未知 change 应显式 Err");
    assert!(
        err.contains("不存在的-change") && err.contains("change 不存在"),
        "miss 记因显式携带 change 名: {err}"
    );

    // root 失配：layout 无此 change 目录，但 db 建档记录在场 → D12 建档记录
    // 恒可达详情（定位 miss → dir 缺席、产物清单空、状态面在）→ Ok 而非 Err
    let mismatched = port
        .detail("/tmp/不存在的根", CHANGE)
        .expect("建档记录恒可达（db 状态面权威，定位 miss 不虚构 None）");
    assert_eq!(mismatched.status, Some(ChangeStatus::Active), "状态面在");
    assert!(mismatched.artifacts.is_empty(), "定位 miss 产物清单空");

    // 在场 change 不受误伤（对照面）
    assert!(port.detail(&root_str, CHANGE).is_ok());
}

// ---------------------------------------------------------------------------
// unparsable 分支退役：损坏 workflow.json 字节零读取
// ---------------------------------------------------------------------------

/// 磁盘 workflow.json 损坏字节样本在场（db 已建档）→ 不再显式 `Err`、零读取
/// 照常出建档 detail（退役回归行——AC-3 双向墙读半边随动）；调用前后磁盘字节
/// 原样（读触点退役的进程内证据）。
#[test]
fn 损坏workflowjson字节样本零读取照常出detail() {
    let root = TempRoot::new("corrupt-retired");
    root.change(CHANGE);
    // 惰性损坏字节样本（存量 CLI 残照——desktop 不解析）
    let corrupt_path = root
        .0
        .join("openspec/changes")
        .join(CHANGE)
        .join("workflow.json");
    fs::write(&corrupt_path, "{ not valid json !!!").expect("写损坏样本失败");
    let db = TestDb::open("corrupt-retired");
    seed_documented_change(&db);
    let root_str = root.root_str();

    let bytes_before = root.file_bytes(CHANGE, "workflow.json");

    let snapshot = StoreSnapshot::new(root_str.clone(), db.store_arc());
    let detail = snapshot
        .detail(&root_str, CHANGE)
        .expect("损坏字节样本在场应照常装配（unparsable 分支退役）");

    // 建档 detail 照常（db 读源单源，磁盘字节零进投影）
    assert_eq!(detail.status, Some(ChangeStatus::Active));
    assert_eq!(detail.pipeline.len(), 9);
    assert_eq!(detail.pipeline[3].attempts[0].verdict, Verdict::Fail);

    // 与 queries 直调 serde 等值（读源换血不加工的退役形态复核）
    let direct =
        change_detail(&resolve(&root.0), db.store.as_ref(), CHANGE).expect("直调应命中同记录");
    assert_eq!(
        serde_json::to_value(&detail).expect("线面序列化应成功"),
        serde_json::to_value(&direct).expect("直调序列化应成功")
    );

    // 字节原样：调用前后 workflow.json 不变（零读取）
    assert_eq!(
        root.file_bytes(CHANGE, "workflow.json"),
        bytes_before,
        "只读：调用前后 workflow.json 字节不变"
    );
}
