//! `store` 的单元测试（AC-1/2/3/4/5/8/10）：三操作 + open + StoreError。
//!
//! tempdir 真开 redb 文件（存储层不 mock）；全部断言经 `Store` 公共 API
//! （D6 三名字 `Store` / `StoreError` / `WorkspaceRecord`），内部协作
//! （model 编解码、canonical 口径）由此间接覆盖。系统时钟不 mock：
//! `added_at` 仅记录入库时间，清单排序与其无关（默认序按表主键）。

use std::fs;
use std::path::{Path, PathBuf};

use redb::{Database, ReadOnlyDatabase, ReadableDatabase, TableDefinition};

use crate::{AgentRunRecord, Store, StoreError, WorkspaceRecord};

/// 测试侧直读 schema_version 用的表定义（镜像 store.rs 的 `user_meta`，
/// 仅作为检查手段，redb 自身事务/持久化语义不在断言范围）。
const TEST_USER_META: TableDefinition<'static, &str, u64> = TableDefinition::new("user_meta");

/// 测试侧直读 agent 两表的表定义（镜像 store.rs 的 `user_agent_runs` /
/// `user_agent_run_events`，仅用于表名前缀存在性检查）。
const TEST_USER_AGENT_RUNS: TableDefinition<'static, i64, &[u8]> =
    TableDefinition::new("user_agent_runs");
const TEST_USER_AGENT_RUN_EVENTS: TableDefinition<'static, (i64, u64), &[u8]> =
    TableDefinition::new("user_agent_run_events");

/// 当前库 schema 版本（镜像 store.rs 的 `SCHEMA_VERSION`）。
const SCHEMA_VERSION: u64 = 1;

/// db 文件 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    db_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("store-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("store-test-{tag}-ws-"))
            .tempdir()
            .expect("创建 workspace 临时目录失败");
        Self { db_dir, ws_root }
    }

    /// 默认 db 文件路径（固定文件名，多次调用同值，支撑重开场景）。
    fn db_path(&self) -> PathBuf {
        self.db_dir.path().join("test.redb")
    }

    /// 在 workspace 根下创建一个真实目录并返回路径（add 的入参目录）。
    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }
}

fn open_ok(path: &Path) -> Store {
    Store::open(path).unwrap_or_else(|e| panic!("open 应成功: {e}"))
}

fn add_ok(store: &Store, dir: &Path) -> WorkspaceRecord {
    store
        .add_workspace(dir)
        .unwrap_or_else(|e| panic!("add_workspace 应成功: {e}"))
}

// ---------------------------------------------------------------------------
// Store::open
// ---------------------------------------------------------------------------

#[test]
fn open对不存在的路径返回ok并创建db文件与父目录() {
    let env = Env::new("open-create");
    let db_path = env.db_dir.path().join("nested/sub/test.redb");
    assert!(!db_path.exists());

    let store = open_ok(&db_path);

    assert!(db_path.exists(), "db 文件被创建");
    assert!(db_path.parent().unwrap().is_dir(), "父目录被创建");
    // 业务表已随 init_schema 建好，空库可正常 list
    assert!(store.list_workspaces().unwrap().is_empty());
}

#[test]
fn 已有db文件再次open返回ok且此前写入的记录完整读回() {
    let env = Env::new("reopen");
    let dir = env.ws("persisted");

    let first = open_ok(&env.db_path());
    let record = add_ok(&first, &dir);
    drop(first);

    let second = open_ok(&env.db_path());
    let list = second.list_workspaces().unwrap();
    assert_eq!(list, vec![record], "重开同一 db 文件后记录仍在");
}

#[test]
fn 父路径被同名普通文件占据时open返回db错误不panic() {
    let env = Env::new("open-blocked");
    let blocker = env.db_dir.path().join("blocker");
    fs::write(&blocker, "普通文件占位").expect("写占位文件失败");

    let result = Store::open(&blocker.join("test.redb"));

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("create_dir_all 无法建目录应失败"),
    };
    assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
    assert!(
        err.to_string().starts_with("db:"),
        "错误串以 db: 前缀，实际: {err}"
    );
}

#[test]
fn 目标为损坏文件时open返回err不静默降级为空库() {
    let env = Env::new("open-corrupt");
    let corrupt = env.db_dir.path().join("corrupt.redb");
    let garbage = "这不是一个 redb 数据库文件。".repeat(32);
    fs::write(&corrupt, garbage).expect("写损坏文件失败");

    let result = Store::open(&corrupt);

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("非合法 redb 文件必须报错，不得静默降级为空库"),
    };
    assert!(
        err.to_string().starts_with("db:"),
        "错误串以 db: 前缀，实际: {err}"
    );
}

#[test]
fn schema_version等于当前版本时open幂等成功() {
    let env = Env::new("schema-equal");

    drop(open_ok(&env.db_path())); // 首次 open 写入当前版本
    let second = open_ok(&env.db_path()); // 等版本重开：幂等成功
    let dir = env.ws("any");
    assert!(add_ok(&second, &dir).root.ends_with("any"));
    drop(second);

    let third = open_ok(&env.db_path());
    assert_eq!(
        third.list_workspaces().unwrap().len(),
        1,
        "再次 open 仍幂等"
    );
}

#[test]
fn schema_version高于支持版本时open返回err() {
    let env = Env::new("schema-newer");
    drop(open_ok(&env.db_path()));

    // 以裸 redb 句柄伪造「由更新版本应用创建」的库：schema_version = 999
    let raw = Database::create(env.db_path()).expect("打开裸句柄失败");
    let txn = raw.begin_write().expect("开启写事务失败");
    {
        let mut meta = txn.open_table(TEST_USER_META).expect("打开 user_meta 失败");
        meta.insert("schema_version", 999_u64)
            .expect("写入高版本号失败");
    }
    txn.commit().expect("提交失败");
    drop(raw);

    let result = Store::open(&env.db_path());
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("高于支持版本的库必须拒绝打开"),
    };
    assert!(
        err.to_string().starts_with("db:"),
        "错误串以 db: 前缀，实际: {err}"
    );
    assert!(
        err.to_string().contains("999"),
        "错误串携带实际版本号，实际: {err}"
    );
}

#[test]
fn 全新库open后经只读句柄直读user_meta表schema_version恰为当前版本() {
    let env = Env::new("schema-fresh");
    drop(open_ok(&env.db_path()));

    // Store 已 drop（写句柄已释放），只读句柄可独占打开做检查
    let ro = ReadOnlyDatabase::open(env.db_path()).expect("只读打开失败");
    let txn = ro.begin_read().expect("开启读事务失败");
    let meta = txn.open_table(TEST_USER_META).expect("打开 user_meta 失败");
    let stored = meta
        .get("schema_version")
        .expect("读取 schema_version 失败")
        .expect("schema_version 必须已写入");
    assert_eq!(stored.value(), SCHEMA_VERSION, "新开 db 即写入当前版本");
}

// ---------------------------------------------------------------------------
// Store::add_workspace
// ---------------------------------------------------------------------------

#[test]
fn add新目录返回canonical完整路径name为目录名末段() {
    let env = Env::new("add-basic");
    let store = open_ok(&env.db_path());
    let dir = env.ws("alpha");

    let record = add_ok(&store, &dir);

    assert_eq!(
        record.root,
        dunce::canonicalize(&dir)
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        "root 为 canonical 完整路径"
    );
    assert_eq!(record.name, "alpha", "name 为目录名最后一段");
    assert_eq!(store.list_workspaces().unwrap(), vec![record]);
}

#[test]
fn add大小写不同的等价路径仅一条且原记录原样返回保留首添added_at() {
    let env = Env::new("case-dedup");
    let store = open_ok(&env.db_path());
    let dir = env.ws("DedupMe");
    let first = add_ok(&store, &dir);

    // 同一目录、大小写不同的书写形态（Windows 盘上真实大小写归一）
    let flipped = Path::new(&first.root).with_file_name("dEDUPmE");
    let second = add_ok(&store, &flipped);

    assert_eq!(
        second, first,
        "等价路径去重为同一 canonical key，原记录原样返回"
    );
    let list = store.list_workspaces().unwrap();
    assert_eq!(list.len(), 1, "清单仅一条记录");
    assert_eq!(list[0].added_at, first.added_at, "added_at 保留首添值");
}

#[test]
fn add尾分隔符与正反斜杠混写路径与已存key去重为一条() {
    let env = Env::new("slash-dedup");
    let store = open_ok(&env.db_path());
    let dir = env.ws("SlashDir");
    let first = add_ok(&store, &dir);

    let trailing = format!("{}\\", first.root);
    let second = add_ok(&store, Path::new(&trailing));
    assert_eq!(second.root, first.root, "尾分隔符书写差异去重");

    let forward = first.root.replace('\\', "/");
    let third = add_ok(&store, Path::new(&forward));
    assert_eq!(third.root, first.root, "正反斜杠混写去重");

    assert_eq!(store.list_workspaces().unwrap().len(), 1, "清单仅一条记录");
}

#[test]
fn add目录名含空格中文emoji的路径name提取正确且记录往返无损() {
    let env = Env::new("unicode");
    let store = open_ok(&env.db_path());
    let dir = env.ws("my 项目 📁");

    let record = add_ok(&store, &dir);

    assert_eq!(record.name, "my 项目 📁", "name 提取正确");
    assert_eq!(
        record.root,
        dunce::canonicalize(&dir)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    );
    // 记录以 JSON 落库：读回与返回记录逐字段相等（编解码往返无损）
    assert_eq!(store.list_workspaces().unwrap(), vec![record]);
}

#[test]
fn add不存在的目录返回canonicalize错误() {
    let env = Env::new("add-missing");
    let store = open_ok(&env.db_path());
    let missing = env.ws_root.path().join("no-such-dir");

    let err = store
        .add_workspace(&missing)
        .expect_err("不存在的目录应失败");

    assert!(
        matches!(err, StoreError::Canonicalize(_)),
        "变体为 Canonicalize，实际: {err:?}"
    );
    assert!(
        err.to_string().starts_with("canonicalize:"),
        "错误串以 canonicalize: 前缀，实际: {err}"
    );
}

#[test]
fn add空路径返回canonicalize错误不panic() {
    let env = Env::new("add-empty");
    let store = open_ok(&env.db_path());

    let err = store
        .add_workspace(Path::new(""))
        .expect_err("空路径应失败");

    assert!(
        err.to_string().starts_with("canonicalize:"),
        "错误串以 canonicalize: 前缀，实际: {err}"
    );
    assert!(
        store.list_workspaces().unwrap().is_empty(),
        "失败不产生记录"
    );
}

// ---------------------------------------------------------------------------
// Store::list_workspaces
// ---------------------------------------------------------------------------

#[test]
fn list按canonical_root字典序升序与添加顺序无关() {
    let env = Env::new("ordering");
    let store = open_ok(&env.db_path());
    // 刻意乱序 add：默认序不随添加（或打开）时间变化
    let rec_gamma = add_ok(&store, &env.ws("gamma"));
    let rec_alpha = add_ok(&store, &env.ws("alpha"));
    let rec_beta = add_ok(&store, &env.ws("beta"));

    let roots: Vec<String> = store
        .list_workspaces()
        .unwrap()
        .into_iter()
        .map(|r| r.root)
        .collect();

    assert_eq!(
        roots,
        vec![rec_alpha.root, rec_beta.root, rec_gamma.root],
        "表主键（canonical root）自然序升序，与添加顺序无关"
    );
}

#[test]
fn list顺序确定可复现不因读写抖动() {
    let env = Env::new("stable-order");
    let store = open_ok(&env.db_path());
    let rec_a = add_ok(&store, &env.ws("zzz-last"));
    let rec_b = add_ok(&store, &env.ws("aaa-first"));

    let first = store.list_workspaces().unwrap();
    let second = store.list_workspaces().unwrap();
    assert_eq!(first, second, "两次 list 顺序确定可复现");
    assert_eq!(
        first.iter().map(|r| r.root.clone()).collect::<Vec<_>>(),
        vec![rec_b.root, rec_a.root],
        "主键自然序稳定，先添的 zzz 不因晚读而置顶"
    );
}

#[test]
fn 空库list返回空向量不报错() {
    let env = Env::new("list-empty");
    let store = open_ok(&env.db_path());

    assert!(store.list_workspaces().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Store::remove_workspace
// ---------------------------------------------------------------------------

#[test]
fn remove已存在key返回true且list不再含该项() {
    let env = Env::new("remove-hit");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("alpha"));

    let hit = store.remove_workspace(Path::new(&record.root)).unwrap();

    assert!(hit);
    assert!(
        store.list_workspaces().unwrap().is_empty(),
        "list 不再含该项"
    );
}

#[test]
fn remove未注册路径返回false且库内容不变() {
    let env = Env::new("remove-miss");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("registered"));
    let ghost = env.ws("ghost");

    let hit = store.remove_workspace(&ghost).unwrap();

    assert!(!hit, "未注册路径 remove 幂等 miss");
    assert_eq!(store.list_workspaces().unwrap(), vec![record], "库内容不变");
}

#[test]
fn remove大小写不同等价路径命中删除同一条无孤儿条目() {
    let env = Env::new("remove-case");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("CaseKey"));

    let flipped = Path::new(&record.root).with_file_name("cASEkEY");
    let hit = store.remove_workspace(&flipped).unwrap();

    assert!(hit, "等价路径命中同一条");
    assert!(store.list_workspaces().unwrap().is_empty(), "无孤儿条目");
}

#[test]
fn remove目录消失后回退匹配命中删除() {
    let env = Env::new("remove-vanish");
    let store = open_ok(&env.db_path());
    let dir = env.ws("gone");
    let record = add_ok(&store, &dir);
    fs::remove_dir_all(&dir).expect("删除目录失败");

    let hit = store.remove_workspace(Path::new(&record.root)).unwrap();

    assert!(hit, "残留清单项可被清理（D1 用户救济路径）");
    assert!(store.list_workspaces().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// StoreError 与全链路
// ---------------------------------------------------------------------------

#[test]
fn store_error两变体display携带db与canonicalize前缀() {
    let db = StoreError::Db("打开失败".to_string());
    let canonicalize = StoreError::Canonicalize("无效路径".to_string());

    assert_eq!(db.to_string(), "db: 打开失败");
    assert_eq!(canonicalize.to_string(), "canonicalize: 无效路径");
}

#[test]
fn 全链路add_list_remove后重开同一db文件清单状态与各操作返回一致() {
    let env = Env::new("full-cycle");
    let (rec_a_root, snapshot) = {
        let store = open_ok(&env.db_path());
        let rec_a = add_ok(&store, &env.ws("alpha"));
        let rec_b = add_ok(&store, &env.ws("beta"));
        let list = store.list_workspaces().unwrap();
        let roots: Vec<String> = list.iter().map(|r| r.root.clone()).collect();
        assert_eq!(
            roots,
            vec![rec_a.root.clone(), rec_b.root],
            "默认序：alpha 字典序在前，与添加顺序无关（此处恰同序）"
        );
        (rec_a.root, list)
    };

    // drop 并重开同一 db 文件：清单状态与操作序列的最终状态一致
    let reopened = open_ok(&env.db_path());
    let after_reopen = reopened.list_workspaces().unwrap();
    assert_eq!(after_reopen, snapshot);

    // 再经一次 remove + 重开：删除同样持久化，仅剩 beta
    assert!(reopened.remove_workspace(Path::new(&rec_a_root)).unwrap());
    drop(reopened);
    let again = open_ok(&env.db_path());
    let remaining = again.list_workspaces().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].root, snapshot[1].root);
}

// ---------------------------------------------------------------------------
// agent 域两表：begin / append / finish / list_runs / list_events（AC-3）
// ---------------------------------------------------------------------------

/// 构造一份 running 形态的 run 记录（id 由 begin 分配，入参不参与匹配）。
fn running_run(prompt: &str, started_at: i64) -> AgentRunRecord {
    AgentRunRecord {
        id: 0,
        prompt: prompt.to_owned(),
        cwd: "C:\\ws\\demo".to_owned(),
        env: "default".to_owned(),
        permission_mode: "bypassPermissions".to_owned(),
        status: "running".to_owned(),
        started_at,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
    }
}

/// 构造一条事件 JSON（seq 必带；payload 可携带任意结构）。
fn event_value(seq: u64, note: &str) -> serde_json::Value {
    serde_json::json!({
        "seq": seq,
        "timestampMs": 1727000000000i64 + seq as i64,
        "kind": "message",
        "role": "assistant",
        "blocks": [],
        "parentToolUseId": null,
        "note": note,
    })
}

fn begin_ok(store: &Store, prompt: &str, started_at: i64) -> AgentRunRecord {
    store
        .begin_agent_run(&running_run(prompt, started_at))
        .unwrap_or_else(|e| panic!("begin_agent_run 应成功: {e}"))
}

#[test]
fn begin_agent_run空库首跑返回id为1且status为running且started_at落值() {
    let env = Env::new("agent-begin-first");
    let store = open_ok(&env.db_path());

    let record = begin_ok(&store, "首轮", 1727000000000);

    assert_eq!(record.id, 1, "空库首跑 max+1 分配 id=1");
    assert_eq!(record.status, "running", "落 running 行");
    assert_eq!(
        record.started_at, 1727000000000,
        "started_at 按调用方值落库"
    );
    assert_eq!(record.finished_at, None, "running 行无结束时间");
    // 清单可见 running 行
    assert_eq!(store.list_agent_runs().unwrap(), vec![record.clone()]);
}

#[test]
fn begin_agent_run连续begin时id严格递增() {
    let env = Env::new("agent-begin-incr");
    let store = open_ok(&env.db_path());

    let first = begin_ok(&store, "第一跑", 100);
    let second = begin_ok(&store, "第二跑", 200);
    let third = begin_ok(&store, "第三跑", 300);

    assert_eq!((first.id, second.id, third.id), (1, 2, 3), "max+1 严格递增");
}

#[test]
fn begin_agent_run传入记录的id字段不参与匹配以分配id落行为准() {
    let env = Env::new("agent-begin-id");
    let store = open_ok(&env.db_path());

    let mut requested = running_run("调用方自填 id", 1727000000000);
    requested.id = 999;
    let record = store.begin_agent_run(&requested).unwrap();

    assert_eq!(record.id, 1, "id 由写事务内 max+1 分配，入参 id 被覆盖");
    assert_eq!(store.list_agent_runs().unwrap()[0].id, 1, "以分配 id 落行");
}

#[test]
fn append后按run_id重放事件seq升序读回() {
    let env = Env::new("agent-append");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "事件流", 1727000000000);

    // 乱序写入：seq 2 / 0 / 1
    let events = vec![
        event_value(2, "乱序乙"),
        event_value(0, "首条"),
        event_value(1, "次条"),
    ];
    store
        .append_agent_run_events(run.id, &events)
        .unwrap_or_else(|e| panic!("append 应成功: {e}"));

    let replay = store.list_agent_run_events(run.id).unwrap();
    assert_eq!(replay.len(), 3, "三事件全部读回");
    let seqs: Vec<u64> = replay
        .iter()
        .map(|value| value["seq"].as_u64().expect("seq 为数值"))
        .collect();
    assert_eq!(
        seqs,
        vec![0, 1, 2],
        "AC-3 重放：按 (run_id, seq) 复合键升序"
    );
}

#[test]
fn append空切片返回ok且不产生行() {
    let env = Env::new("agent-append-empty");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "空事件流", 1727000000000);

    store.append_agent_run_events(run.id, &[]).unwrap();

    assert!(store.list_agent_run_events(run.id).unwrap().is_empty());
}

#[test]
fn append事件含中文emoji与深嵌套时进出无损() {
    let env = Env::new("agent-append-unicode");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "保真事件流", 1727000000000);

    let event = serde_json::json!({
        "seq": 0,
        "timestampMs": 1,
        "kind": "systemNotice",
        "subtype": "dump",
        "payload": { "嵌套": { "深层": ["🎉", {"再深": "换行\n中文"}] } },
    });
    store
        .append_agent_run_events(run.id, &[event.clone()])
        .unwrap();

    let replay = store.list_agent_run_events(run.id).unwrap();
    assert_eq!(replay, vec![event], "事件行以 Value 进出 store，无损（D3）");
}

#[test]
fn finish后整行替换为终态且list反映() {
    let env = Env::new("agent-finish");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "待收敛", 1727000000000);

    let mut finished = run.clone();
    finished.status = "completed".to_owned();
    finished.finished_at = Some(1727000001000);
    finished.num_turns = Some(4);
    finished.cost_usd = Some(0.5);
    finished.duration_ms = Some(999);
    finished.session_id = Some("s-1".to_owned());
    store
        .finish_agent_run(run.id, &finished)
        .unwrap_or_else(|e| panic!("finish 应成功: {e}"));

    let listed = store.list_agent_runs().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, "completed", "终态整行替换");
    assert_eq!(listed[0].num_turns, Some(4));
    assert_eq!(listed[0].cost_usd, Some(0.5));
    assert_eq!(listed[0].duration_ms, Some(999));
    assert_eq!(listed[0].session_id.as_deref(), Some("s-1"));
    assert_eq!(listed[0].finished_at, Some(1727000001000));
}

#[test]
fn list_agent_runs按started_at降序并列时按id降序() {
    let env = Env::new("agent-list-order");
    let store = open_ok(&env.db_path());
    // started_at 显式注入（排序依据由调用方落库值决定），无需 sleep
    let early = begin_ok(&store, "早", 100);
    let late = begin_ok(&store, "晚", 300);
    let middle = begin_ok(&store, "中", 200);

    let ids: Vec<i64> = store
        .list_agent_runs()
        .unwrap()
        .into_iter()
        .map(|record| record.id)
        .collect();
    assert_eq!(ids, vec![late.id, middle.id, early.id], "started_at 降序");

    // 并列时 id 降序（顺序确定）
    let tie_a = begin_ok(&store, "并列甲", 300);
    let tie_b = begin_ok(&store, "并列乙", 300);
    let top_two: Vec<i64> = store
        .list_agent_runs()
        .unwrap()
        .into_iter()
        .take(2)
        .map(|record| record.id)
        .collect();
    assert_eq!(top_two, vec![tie_b.id, tie_a.id], "并列按 id 降序");
}

#[test]
fn 空库list_agent_runs返回空向量不报错() {
    let env = Env::new("agent-list-empty");
    let store = open_ok(&env.db_path());

    assert!(store.list_agent_runs().unwrap().is_empty());
}

#[test]
fn 不存在run_id的list_agent_run_events返回空向量不报错() {
    let env = Env::new("agent-events-miss");
    let store = open_ok(&env.db_path());

    assert!(store.list_agent_run_events(42).unwrap().is_empty());
}

#[test]
fn 两run同seq互不串扰且复合键半开区间按run_id隔离() {
    let env = Env::new("agent-isolation");
    let store = open_ok(&env.db_path());
    let run_a = begin_ok(&store, "run A", 100);
    let run_b = begin_ok(&store, "run B", 200);

    store
        .append_agent_run_events(run_a.id, &[event_value(0, "A0"), event_value(1, "A1")])
        .unwrap();
    store
        .append_agent_run_events(run_b.id, &[event_value(0, "B0"), event_value(1, "B1")])
        .unwrap();

    let replay_a = store.list_agent_run_events(run_a.id).unwrap();
    let notes: Vec<&str> = replay_a
        .iter()
        .map(|value| value["note"].as_str().expect("note 为字符串"))
        .collect();
    assert_eq!(notes, vec!["A0", "A1"], "run A 不串入 run B 的任何事件");
    assert_eq!(store.list_agent_run_events(run_b.id).unwrap().len(), 2);
}

#[test]
fn run与events写入后drop重开同一db文件记录与事件完整() {
    let env = Env::new("agent-reopen");
    let run;
    let snapshot_events;
    {
        let store = open_ok(&env.db_path());
        run = begin_ok(&store, "持久化验证", 1727000000000);
        let events = vec![event_value(0, "第一条"), event_value(1, "第二条")];
        store.append_agent_run_events(run.id, &events).unwrap();
        let mut finished = run.clone();
        finished.status = "failed".to_owned();
        finished.finished_at = Some(1727000002000);
        finished.error = Some("进程结束但未产出 result 事件".to_owned());
        store.finish_agent_run(run.id, &finished).unwrap();
        snapshot_events = store.list_agent_run_events(run.id).unwrap();
        // drop 前显式释放文件锁（与 workspace 重开场景同口径）
    }

    let reopened = open_ok(&env.db_path());
    let listed = reopened.list_agent_runs().unwrap();
    assert_eq!(listed.len(), 1, "重开后 run 记录完整");
    assert_eq!(listed[0].status, "failed");
    assert_eq!(
        listed[0].error.as_deref(),
        Some("进程结束但未产出 result 事件")
    );
    assert_eq!(
        reopened.list_agent_run_events(run.id).unwrap(),
        snapshot_events,
        "AC-3 重开持久性：事件流完整"
    );
}

#[test]
fn 裸redb只读句柄可见user前缀两agent表() {
    let env = Env::new("agent-table-prefix");
    let store = open_ok(&env.db_path());
    let run = begin_ok(&store, "建表核验", 1727000000000);
    store
        .append_agent_run_events(run.id, &[event_value(0, "n")])
        .unwrap();
    drop(store); // 释放写句柄文件锁

    // Store 已 drop：只读句柄可独占打开，open_table 成功即表存在
    let ro = ReadOnlyDatabase::open(env.db_path()).expect("只读打开失败");
    let txn = ro.begin_read().expect("开启读事务失败");
    txn.open_table(TEST_USER_AGENT_RUNS)
        .expect("user_agent_runs 表存在（user 维度前缀）");
    txn.open_table(TEST_USER_AGENT_RUN_EVENTS)
        .expect("user_agent_run_events 表存在（user 维度前缀）");
}
