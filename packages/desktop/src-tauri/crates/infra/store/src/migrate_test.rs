//! 「legacy 旧库格式 → Store::open 探测 → migrate 一次性迁移 → native_db 新库」
//! 集成关系测试（AC-1/AC-2/AC-3）：全量迁移逐行一致、损坏行 `Raw` 兜底零丢失、
//! `.bak` 留档与失败回退原子性（R1 三场景）。
//!
//! 旧 redb 手写格式库 fixture 由测试侧以裸 redb 句柄直写三表构造（镜像旧表
//! 定义常量），属测试数据构造而非依赖 mock；文件系统改名 / 占用不 mock——
//! tempfile 真实路径上断言 `.bak` 存在性与回退，失败注入用「路径被目录占据」
//! 等真实可达形态。全部触发经 `Store::open`（migrate 为 crate 私有，由 open
//! 隐式调用）。

use std::fs;
use std::path::{Path, PathBuf};

use redb::{Database, ReadOnlyDatabase, ReadableDatabase, ReadableTableMetadata, TableDefinition};

use crate::{Store, StoreError};

// --- 旧表定义常量（逐字平移自旧 store 手写表，与 migrate.rs 内私有常量镜像）---

/// legacy user 维度注册表（key = canonical root，value = `WorkspaceRecord` JSON）
const USER_WORKSPACES: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new("user_workspaces");

/// legacy agent 运行元数据（key = run id，value = `AgentRunRecord` JSON）
const USER_AGENT_RUNS: TableDefinition<'static, i64, &[u8]> =
    TableDefinition::new("user_agent_runs");

/// legacy agent 运行事件流（key = `(run_id, seq)` 复合键，value = 事件 JSON）
const USER_AGENT_RUN_EVENTS: TableDefinition<'static, (i64, u64), &[u8]> =
    TableDefinition::new("user_agent_run_events");

/// legacy META 表（`schema_version` 轮账随迁移退役，仅作格式判定）
const USER_META: TableDefinition<'static, &str, u64> = TableDefinition::new("user_meta");

// ---------------------------------------------------------------------------
// 装置：legacy fixture 构造（裸 redb 4.x 写端 = 旧手写格式同款）
// ---------------------------------------------------------------------------

struct Env {
    db_dir: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("migrate-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        Self { db_dir }
    }

    fn db_path(&self) -> PathBuf {
        self.db_dir.path().join("desktop-store.redb")
    }

    fn backup_path(&self) -> PathBuf {
        self.db_path().with_file_name("desktop-store.redb.bak")
    }

    fn tmp_path(&self) -> PathBuf {
        self.db_path()
            .with_file_name("desktop-store.redb.native-tmp")
    }
}

/// 建空 legacy 库并关闭句柄：user_meta 落 schema_version，三业务表建空表
/// （旧 store 建库即建全部表，迁移读面要求三表齐备——fixture 镜像该形态）。
fn create_legacy_db(path: &Path) -> Database {
    let db = Database::create(path).expect("创建 legacy fixture 库失败");
    let txn = db.begin_write().expect("开启写事务失败");
    txn.open_table(USER_WORKSPACES).expect("建表失败");
    txn.open_table(USER_AGENT_RUNS).expect("建表失败");
    txn.open_table(USER_AGENT_RUN_EVENTS).expect("建表失败");
    let mut meta = txn.open_table(USER_META).expect("打开 user_meta 失败");
    meta.insert("schema_version", 1u64)
        .expect("写 schema_version 失败");
    drop(meta);
    txn.commit().expect("提交 fixture 事务失败");
    db
}

/// `user_workspaces` 写一行 JSON fixture（key = root）。
fn insert_workspace(db: &Database, key: &str, value: &serde_json::Value) {
    let txn = db.begin_write().expect("开启写事务失败");
    let mut handle = txn.open_table(USER_WORKSPACES).expect("打开表失败");
    handle
        .insert(key, serde_json::to_vec(value).unwrap().as_slice())
        .expect("写 fixture 行失败");
    drop(handle);
    txn.commit().expect("提交失败");
}

/// `user_agent_runs` 写一行 JSON fixture（key = run id）。
fn insert_run(db: &Database, key: i64, value: &serde_json::Value) {
    let txn = db.begin_write().expect("开启写事务失败");
    let mut handle = txn.open_table(USER_AGENT_RUNS).expect("打开表失败");
    handle
        .insert(key, serde_json::to_vec(value).unwrap().as_slice())
        .expect("写 fixture 行失败");
    drop(handle);
    txn.commit().expect("提交失败");
}

/// `user_agent_run_events` 写一行 JSON fixture（key = (run_id, seq) 复合键）。
fn insert_event(db: &Database, key: (i64, u64), value: &serde_json::Value) {
    let txn = db.begin_write().expect("开启写事务失败");
    let mut handle = txn.open_table(USER_AGENT_RUN_EVENTS).expect("打开表失败");
    handle
        .insert(key, serde_json::to_vec(value).unwrap().as_slice())
        .expect("写 fixture 行失败");
    drop(handle);
    txn.commit().expect("提交失败");
}

/// `user_agent_run_events` 写一行原始字节（损坏行 fixture 用）。
fn insert_event_bytes(db: &Database, key: (i64, u64), value: &[u8]) {
    let txn = db.begin_write().expect("开启写事务失败");
    let mut handle = txn.open_table(USER_AGENT_RUN_EVENTS).expect("打开表失败");
    handle.insert(key, value).expect("写 fixture 行失败");
    drop(handle);
    txn.commit().expect("提交失败");
}

/// 表行数（只读复核：.bak 留档完整性 / 失败后旧库原样）。
fn readonly_count(
    path: &Path,
    table: TableDefinition<'static, impl redb::Key, impl redb::Value>,
) -> usize {
    let db = ReadOnlyDatabase::open(path)
        .unwrap_or_else(|e| panic!("只读打开 {} 失败: {e}", path.display()));
    let txn = db.begin_read().expect("开启只读事务失败");
    let handle = txn.open_table(table).expect("打开表失败");
    handle.len().expect("统计行数失败") as usize
}

/// open 成功帮手（迁移触发即在此发生）。
fn open_ok(path: &Path) -> Store {
    Store::open(path).unwrap_or_else(|e| panic!("open 应成功: {e}"))
}

// --- fixture JSON（镜像旧手写 encode 的 camelCase 线格式）---

fn workspace_json(root: &str, name: &str, added_at: i64) -> serde_json::Value {
    serde_json::json!({ "root": root, "name": name, "addedAt": added_at })
}

fn run_json(
    id: i64,
    prompt: &str,
    status: &str,
    started_at: i64,
    finished_at: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "prompt": prompt,
        "cwd": "C:\\ws\\demo",
        "env": "default",
        "permissionMode": "bypassPermissions",
        "status": status,
        "startedAt": started_at,
        "finishedAt": finished_at,
        "numTurns": if status == "completed" { Some(2) } else { None },
        "costUsd": if status == "completed" { Some(0.5) } else { None },
        "durationMs": if status == "completed" { Some(800) } else { None },
        "sessionId": if status == "completed" { Some("s-1") } else { None },
        "error": Option::<String>::None,
    })
}

fn event_started_json(seq: u64) -> serde_json::Value {
    serde_json::json!({
        "seq": seq,
        "timestampMs": 1727000000000i64,
        "kind": "runStarted",
        "model": "claude-opus",
        "sessionId": "s-1",
        "tools": ["Bash"],
        "mcpServers": [],
    })
}

/// 含中文 / emoji / 深嵌套块的合法 message 事件。
fn event_message_json(seq: u64) -> serde_json::Value {
    serde_json::json!({
        "seq": seq,
        "timestampMs": 1727000000001i64,
        "kind": "message",
        "role": "助手 🚀",
        "blocks": [
            { "kind": "text", "text": "你好，世界" },
            { "kind": "toolUse", "id": "tu_1", "name": "Bash",
              "input": { "cmd": "ls", "nested": { "deep": [1, 2, { "leaf": "深嵌套" }] } } },
        ],
        "parentToolUseId": null,
    })
}

/// 场景一 fixture：2 workspace、2 run（completed 终态 + running）、两 run 各多条乱序 seq。
fn seed_full_legacy(path: &Path) {
    let db = create_legacy_db(path);
    insert_workspace(
        &db,
        "C:\\ws\\beta",
        &workspace_json("C:\\ws\\beta", "beta", 2),
    );
    insert_workspace(
        &db,
        "C:\\ws\\alpha",
        &workspace_json("C:\\ws\\alpha", "alpha", 1),
    );
    insert_run(&db, 1, &run_json(1, "首轮", "completed", 100, Some(900)));
    insert_run(&db, 2, &run_json(2, "进行中", "running", 300, None));
    // 乱序 seq 写入：迁移后重放必须按键升序还原
    insert_event(&db, (1, 2), &event_started_json(2));
    insert_event(&db, (1, 0), &event_message_json(0));
    insert_event(&db, (1, 1), &event_started_json(1));
    insert_event(&db, (2, 1), &event_message_json(1));
    insert_event(&db, (2, 0), &event_started_json(0));
    drop(db);
}

// ---------------------------------------------------------------------------
// 场景一：旧格式库首启自动迁移且各表记录一致
// ---------------------------------------------------------------------------

#[test]
fn 三表fixture全量迁移后经store公共api读回逐行一致() {
    let env = Env::new("full-migrate");
    seed_full_legacy(&env.db_path());

    let store = open_ok(&env.db_path());

    // workspace 读面：root / name / addedAt 逐行一致
    let workspaces = store.list_workspaces().unwrap();
    assert_eq!(workspaces.len(), 2, "workspace 行数一致");
    assert_eq!(
        workspaces
            .iter()
            .map(|record| record.root.as_str())
            .collect::<Vec<_>>(),
        vec!["C:\\ws\\alpha", "C:\\ws\\beta"],
        "主键（canonical root）自然序"
    );
    assert_eq!(workspaces[0].name, "alpha");
    assert_eq!(workspaces[0].added_at, 1);

    // run 读面：startedAt 降序不变，字段面逐行一致
    let runs = store.list_agent_runs().unwrap();
    assert_eq!(runs.len(), 2, "run 行数一致");
    assert_eq!(
        runs.iter().map(|record| record.id).collect::<Vec<_>>(),
        vec![2, 1],
        "startedAt 降序（run 2 晚启动在前）"
    );
    let completed = runs.iter().find(|record| record.id == 1).unwrap();
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.num_turns, Some(2));
    assert_eq!(completed.cost_usd, Some(0.5));
    assert_eq!(completed.finished_at, Some(900));

    // 事件读面：seq 升序、嵌装载荷逐字段还原（乱序写入被键序纠正）
    let run1 = store.list_agent_run_events(1).unwrap();
    let seqs: Vec<u64> = run1.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2], "run 1 事件按 seq 升序重放");
    assert_eq!(
        serde_json::to_value(&run1[0]).unwrap(),
        event_message_json(0),
        "message 事件嵌装载荷与旧库 JSON 逐字段一致"
    );
    let run2 = store.list_agent_run_events(2).unwrap();
    let seqs: Vec<u64> = run2.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1], "run 2 事件按 seq 升序重放");
}

#[test]
fn 迁移后重开同一路径直通重读结果与首次一致且bak不再变动() {
    let env = Env::new("reopen-passthrough");
    seed_full_legacy(&env.db_path());

    let first = open_ok(&env.db_path());
    let workspaces = first.list_workspaces().unwrap();
    let runs = first.list_agent_runs().unwrap();
    let run1 = first.list_agent_run_events(1).unwrap();
    drop(first);
    let bak_after_migrate = fs::read(env.backup_path()).expect("读 .bak 失败");

    // 重开：native 格式即迁移完成标记，不再触发探测迁移
    let second = open_ok(&env.db_path());
    assert_eq!(second.list_workspaces().unwrap(), workspaces);
    assert_eq!(second.list_agent_runs().unwrap(), runs);
    assert_eq!(second.list_agent_run_events(1).unwrap(), run1);
    let bak_after_reopen = fs::read(env.backup_path()).expect("读 .bak 失败");
    assert_eq!(
        bak_after_reopen, bak_after_migrate,
        "重开直通不重复迁移：.bak 字节未变动"
    );
}

#[test]
fn 空legacy库迁移后为空新库list_models计数全0不报错() {
    let env = Env::new("empty-legacy");
    // 三业务表存在但零行（create_legacy_db 即此形态）
    create_legacy_db(&env.db_path());

    let store = open_ok(&env.db_path());

    let models = store.list_models().unwrap();
    assert!(
        models.iter().all(|model| model.count == 0),
        "空 legacy 迁移后为空新库，三模型计数全 0: {models:?}"
    );
    assert_eq!(models.len(), 3, "三模型照列");
}

#[test]
fn legacy库含user_meta表时迁移成功且user_meta不出现在新库任何可读面() {
    let env = Env::new("user-meta-retired");
    let db = create_legacy_db(&env.db_path());
    insert_workspace(
        &db,
        "C:\\ws\\alpha",
        &workspace_json("C:\\ws\\alpha", "alpha", 1),
    );
    drop(db);

    let store = open_ok(&env.db_path());

    // 数据面正常：三模型注册表即全部可读面，user_meta 无对应模型
    let models = store.list_models().unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace", "agent_run", "agent_event"],
        "注册表即全部可读面（schema_version 轮账概念无落点）"
    );
    assert_eq!(store.list_workspaces().unwrap().len(), 1);
    // 结构面复核：只读句柄前先 drop Store（redb 文件锁不跨句柄共享）
    drop(store);
    // 原路径已是 native（redb 2.x）新库：不再可被 legacy 口径（redb 4.x 只读
    // 端）打开——user_meta 随旧文件格式整体退役，仅存于 .bak 留档（AC-3）
    let probe = ReadOnlyDatabase::open(&env.db_path());
    let err = match probe {
        Err(err) => err,
        Ok(_) => panic!("native 新库不应再可被 legacy 只读口径打开"),
    };
    assert!(
        format!("{err:?}").contains("UpgradeRequired"),
        "原路径为 native 新格式（升级才可经 4.x 读端打开）: {err:?}"
    );
    let bak = ReadOnlyDatabase::open(env.backup_path()).expect(".bak 留档仍为 legacy 格式");
    let txn = bak.begin_read().expect("开启只读事务失败");
    assert!(
        txn.open_table(USER_META).is_ok(),
        "user_meta 仅存在于 .bak 留档，未进入新库"
    );
}

// ---------------------------------------------------------------------------
// 场景二：损坏事件行 Raw 兜底零丢失
// ---------------------------------------------------------------------------

/// 场景二 fixture：合法行 + 三类损坏行（非法 JSON / 缺 kind / 未知 kind）。
fn seed_corrupt_legacy(path: &Path) {
    let db = create_legacy_db(path);
    // 合法行：run 1 seq 0（runStarted）、run 2 seq 0（中文 emoji 深嵌套 message）
    insert_event(&db, (1, 0), &event_started_json(0));
    insert_event(&db, (2, 0), &event_message_json(0));
    // 损坏一：非法 JSON 字节串（run 1 seq 10）
    insert_event_bytes(&db, (1, 10), b"not-json {{{");
    // 损坏二：合法 JSON 但缺 kind；value 内 seq=99 与键 seq=42 相异——以键为准
    insert_event(
        &db,
        (1, 42),
        &serde_json::json!({ "seq": 99, "timestampMs": 9000 }),
    );
    // 损坏三：kind 为未知判别值（run 2 seq 6）
    insert_event(
        &db,
        (2, 6),
        &serde_json::json!({ "seq": 6, "timestampMs": 8000, "kind": "mysteryKind", "extra": 1 }),
    );
    drop(db);
}

#[test]
fn 混入三类损坏行迁移成功重放行数与旧行数相等零丢失() {
    let env = Env::new("corrupt-zero-loss");
    seed_corrupt_legacy(&env.db_path());

    // open 不因损坏行失败
    let store = open_ok(&env.db_path());

    // 零丢失：重放行数与旧行数相等（5 = 2 合法 + 3 损坏）
    let run1 = store.list_agent_run_events(1).unwrap();
    let run2 = store.list_agent_run_events(2).unwrap();
    assert_eq!(
        run1.len() + run2.len(),
        5,
        "任何一行不丢失（构造性保证成立）"
    );
    assert_eq!(run1.len(), 3, "run 1 恰 3 行（1 合法 + 2 损坏）");
    assert_eq!(run2.len(), 2, "run 2 恰 2 行（1 合法 + 1 损坏）");
}

#[test]
fn 三类损坏行各自落raw且seq恒取旧表键兜底口径取值() {
    let env = Env::new("corrupt-raw-fallback");
    seed_corrupt_legacy(&env.db_path());
    let store = open_ok(&env.db_path());

    // 损坏一：非法 JSON → event_type "unknown"、timestampMs 0、原文完整保留、seq 取键
    let run1 = store.list_agent_run_events(1).unwrap();
    let by_seq = |seq: u64| {
        run1.iter()
            .find(|event| event.seq == seq)
            .unwrap_or_else(|| panic!("缺 seq {seq} 行"))
    };
    let raw_invalid = by_seq(10);
    match &raw_invalid.kind {
        agent::AgentEventKind::Raw {
            event_type,
            raw_json,
        } => {
            assert_eq!(event_type, "unknown", "无 kind 字段可取 → unknown");
            assert_eq!(raw_json, "not-json {{{", "原文经 lossy 完整保留");
        }
        other => panic!("损坏行应落 Raw，实际: {other:?}"),
    }
    assert_eq!(raw_invalid.timestamp_ms, 0, "非法 JSON 无 timestampMs → 0");

    // 损坏二：缺 kind → unknown；seq 取旧表键 42（不取 value 内的 99）
    let raw_missing_kind = by_seq(42);
    match &raw_missing_kind.kind {
        agent::AgentEventKind::Raw {
            event_type,
            raw_json,
        } => {
            assert_eq!(event_type, "unknown");
            assert_eq!(
                raw_json, r#"{"seq":99,"timestampMs":9000}"#,
                "原文逐字保留（含与键相异的 value seq）"
            );
        }
        other => panic!("缺 kind 行应落 Raw，实际: {other:?}"),
    }
    assert_eq!(
        raw_missing_kind.timestamp_ms, 9000,
        "timestampMs 取 JSON 值"
    );
    assert_eq!(
        raw_missing_kind.seq, 42,
        "seq 恒取旧表复合键（新库主键空间与原键一一对应）"
    );

    // 损坏三：未知 kind → event_type 取 JSON kind 字段
    let run2 = store.list_agent_run_events(2).unwrap();
    let raw_unknown_kind = run2.iter().find(|event| event.seq == 6).unwrap();
    match &raw_unknown_kind.kind {
        agent::AgentEventKind::Raw { event_type, .. } => {
            assert_eq!(event_type, "mysteryKind", "kind 为未知判别值时原样透传");
        }
        other => panic!("未知 kind 行应落 Raw，实际: {other:?}"),
    }
    assert_eq!(raw_unknown_kind.timestamp_ms, 8000);

    // 合法行不受损坏行影响：逐字段保真
    let legal = run1.iter().find(|event| event.seq == 0).unwrap();
    assert_eq!(
        serde_json::to_value(legal).unwrap(),
        event_started_json(0),
        "合法行不受同表损坏行影响"
    );
}

#[test]
fn 事件value含中文emoji深嵌套的合法行迁移后嵌装载荷端到端无损() {
    let env = Env::new("corrupt-legal-fidelity");
    seed_corrupt_legacy(&env.db_path());
    let store = open_ok(&env.db_path());

    let run2 = store.list_agent_run_events(2).unwrap();
    let message = run2.iter().find(|event| event.seq == 0).unwrap();

    assert_eq!(
        serde_json::to_value(message).unwrap(),
        event_message_json(0),
        "中文 / emoji / 深嵌套载荷嵌装往返端到端无损"
    );
    assert!(
        matches!(message.kind, agent::AgentEventKind::Message { .. }),
        "合法行不落 Raw"
    );
}

// ---------------------------------------------------------------------------
// 场景三：.bak 留档与失败回退原子性
// ---------------------------------------------------------------------------

#[test]
fn 迁移成功后原路径为native新库且bak为完整旧库可只读复核() {
    let env = Env::new("bak-archive");
    seed_full_legacy(&env.db_path());

    // 原路径为 native 新库：可经 Store 正常打开使用
    let store = open_ok(&env.db_path());
    assert_eq!(store.list_workspaces().unwrap().len(), 2);
    drop(store);

    // .bak 存在且内容为完整旧库（只读 redb 句柄复核三表行数）
    let backup = env.backup_path();
    assert!(backup.exists(), ".bak 留档存在");
    assert_eq!(
        readonly_count(&backup, USER_WORKSPACES),
        2,
        "留档 workspace 表完整"
    );
    assert_eq!(
        readonly_count(&backup, USER_AGENT_RUNS),
        2,
        "留档 run 表完整"
    );
    assert_eq!(
        readonly_count(&backup, USER_AGENT_RUN_EVENTS),
        5,
        "留档事件表完整"
    );
    assert_eq!(readonly_count(&backup, USER_META), 1, "留档含原 META 表");
}

#[test]
fn 写新库失败注入时open返回db错误带迁移前缀且旧库保持原样() {
    let env = Env::new("fail-write-new");
    seed_full_legacy(&env.db_path());
    // 注入：临时文件路径被目录占据 → 建新库失败（真实可达形态，非 mock）
    fs::create_dir_all(env.tmp_path()).expect("建占据目录失败");

    let result = Store::open(&env.db_path());

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("写新失败必须 Err"),
    };
    assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
    assert!(
        err.to_string().contains("迁移:"),
        "错误串携带「迁移:」语境前缀，实际: {err}"
    );
    // 旧文件保持原样：原路径仍可按 legacy 口径读出全部记录
    assert_eq!(
        readonly_count(&env.db_path(), USER_WORKSPACES),
        2,
        "旧库未被截断或移走"
    );
    assert!(!env.backup_path().exists(), "失败路径不产生 .bak");
}

#[test]
fn 留档改名受阻注入时open报错且旧库保持原路径原样() {
    let env = Env::new("fail-rename");
    seed_full_legacy(&env.db_path());
    // 注入：.bak 路径被目录占据 → 移除既有留档失败（真实可达形态）
    fs::create_dir_all(env.backup_path()).expect("建占据目录失败");

    let result = Store::open(&env.db_path());

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("留档受阻必须 Err"),
    };
    assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
    assert!(
        err.to_string().contains("迁移:"),
        "错误串携带「迁移:」语境前缀，实际: {err}"
    );
    // 旧文件保持原样：仍在原路径、可按 legacy 口径完整读出
    assert_eq!(
        readonly_count(&env.db_path(), USER_WORKSPACES),
        2,
        "旧库未动（任何时点不丢）"
    );
}

#[test]
fn 同路径bak已存在残留时成功迁移先移除再改名bak为新留档() {
    let env = Env::new("bak-leftover");
    seed_full_legacy(&env.db_path());
    // 上次迁移残留：.bak 为旧内容（非本次旧库）
    fs::write(env.backup_path(), "上次迁移残留").expect("写残留文件失败");

    let store = open_ok(&env.db_path());
    assert_eq!(store.list_workspaces().unwrap().len(), 2, "迁移成功");
    drop(store);

    // 残留被先移除再改名：.bak 现为本次旧库的完整留档（残留内容消失）
    let backup = env.backup_path();
    assert_eq!(
        readonly_count(&backup, USER_WORKSPACES),
        2,
        "成功迁移后 .bak 为本次旧库留档（先移除再改名）"
    );
    assert_eq!(readonly_count(&backup, USER_META), 1, "留档 META 表在位");
}
