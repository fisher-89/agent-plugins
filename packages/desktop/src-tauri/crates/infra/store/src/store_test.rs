//! `store` 的单元测试（AC-1/2/3/4/5/8/10）：四操作 + open + StoreError。
//!
//! tempdir 真开 redb 文件（存储层不 mock）；全部断言经 `Store` 公共 API
//! （D6 三名字 `Store` / `StoreError` / `WorkspaceRecord`），内部协作
//! （model 编解码、canonical 口径）由此间接覆盖。系统时钟不 mock：
//! 并列场景以 tie-break 与「不减」断言表述，需要严格时间差的场景以
//! 毫秒级 sleep 拉开。

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use redb::{Database, ReadOnlyDatabase, ReadableDatabase, TableDefinition};

use crate::{Store, StoreError, WorkspaceRecord};

/// 测试侧直读 schema_version 用的表定义（镜像 store.rs 的 `user_meta`，
/// 仅作为检查手段，redb 自身事务/持久化语义不在断言范围）。
const TEST_USER_META: TableDefinition<'static, &str, u64> = TableDefinition::new("user_meta");

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
fn add新目录返回canonical完整路径name为目录名末段且两时间戳同值() {
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
    assert_eq!(
        record.added_at, record.last_opened_at,
        "新建语义两时间戳同值"
    );
    assert_eq!(store.list_workspaces().unwrap(), vec![record]);
}

#[test]
fn add大小写不同的等价路径仅一条且刷新时间保留首添added_at() {
    let env = Env::new("case-dedup");
    let store = open_ok(&env.db_path());
    let dir = env.ws("DedupMe");
    let first = add_ok(&store, &dir);

    // 同一目录、大小写不同的书写形态（Windows 盘上真实大小写归一）
    let flipped = Path::new(&first.root).with_file_name("dEDUPmE");
    sleep(Duration::from_millis(5));
    let second = add_ok(&store, &flipped);

    assert_eq!(second.root, first.root, "等价路径去重为同一 canonical key");
    let list = store.list_workspaces().unwrap();
    assert_eq!(list.len(), 1, "清单仅一条记录");
    assert_eq!(list[0].added_at, first.added_at, "added_at 保留首添值");
    assert!(
        list[0].last_opened_at > first.last_opened_at,
        "last_opened_at 已刷新"
    );
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
fn list按last_opened_at降序且第一名即最近touch() {
    let env = Env::new("ordering");
    let store = open_ok(&env.db_path());
    let rec_gamma = add_ok(&store, &env.ws("gamma"));
    sleep(Duration::from_millis(3));
    let rec_alpha = add_ok(&store, &env.ws("alpha"));
    sleep(Duration::from_millis(3));
    let rec_beta = add_ok(&store, &env.ws("beta"));
    sleep(Duration::from_millis(3));
    assert!(store.touch_workspace(Path::new(&rec_alpha.root)).unwrap());

    let roots: Vec<String> = store
        .list_workspaces()
        .unwrap()
        .into_iter()
        .map(|r| r.root)
        .collect();

    // alpha 最近 touch 置顶；beta 晚于 gamma add，降序在前（并列时 root 升序亦同序）
    assert_eq!(roots, vec![rec_alpha.root, rec_beta.root, rec_gamma.root]);
}

#[test]
fn list并列时按root字典序升序且顺序确定可复现() {
    let env = Env::new("tie-break");
    let store = open_ok(&env.db_path());
    // 同毫秒连续 add（刻意不 sleep）：时间戳很可能并列
    let rec_a = add_ok(&store, &env.ws("zzz-last"));
    let rec_b = add_ok(&store, &env.ws("aaa-first"));
    assert_eq!(rec_a.added_at, rec_a.last_opened_at, "新建语义两时间戳同值");
    assert_eq!(rec_b.added_at, rec_b.last_opened_at, "新建语义两时间戳同值");

    let first = store.list_workspaces().unwrap();
    let second = store.list_workspaces().unwrap();
    assert_eq!(first, second, "两次 list 顺序确定可复现");

    for pair in first.windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        let ordered = prev.last_opened_at > next.last_opened_at
            || (prev.last_opened_at == next.last_opened_at && prev.root <= next.root);
        assert!(ordered, "须按 last_opened_at 降序、并列按 root 字典序升序");
    }
}

#[test]
fn 空库list返回空向量不报错() {
    let env = Env::new("list-empty");
    let store = open_ok(&env.db_path());

    assert!(store.list_workspaces().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Store::touch_workspace
// ---------------------------------------------------------------------------

#[test]
fn touch已存在key返回true且last_opened_at不减() {
    let env = Env::new("touch-hit");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("alpha"));

    sleep(Duration::from_millis(2));
    let hit = store.touch_workspace(Path::new(&record.root)).unwrap();

    assert!(hit, "已存在 key touch 命中");
    let stored = &store.list_workspaces().unwrap()[0];
    assert!(
        stored.last_opened_at >= record.last_opened_at,
        "last_opened_at 不减"
    );
}

#[test]
fn touch未注册路径返回false幂等不产生新记录() {
    let env = Env::new("touch-miss");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("registered"));
    let ghost = env.ws("ghost"); // 存在但未注册

    let hit = store.touch_workspace(&ghost).unwrap();

    assert!(!hit, "未注册路径 touch 幂等 miss");
    assert_eq!(
        store.list_workspaces().unwrap(),
        vec![record],
        "不产生新记录"
    );
}

#[test]
fn touch目录消失后以原路径走词法回退命中并刷新() {
    let env = Env::new("touch-vanish");
    let store = open_ok(&env.db_path());
    let dir = env.ws("vanish");
    let record = add_ok(&store, &dir);
    fs::remove_dir_all(&dir).expect("删除目录失败");

    // 目录已消失：canonicalize 失败 → 词法归一化回退匹配存量 key
    let hit = store.touch_workspace(Path::new(&record.root)).unwrap();

    assert!(hit, "回退匹配命中存量 key");
    let stored = &store.list_workspaces().unwrap()[0];
    assert!(
        stored.last_opened_at >= record.last_opened_at,
        "命中记录的 last_opened_at 已刷新"
    );
}

#[test]
fn touch空路径回退不命中返回false幂等() {
    let env = Env::new("touch-empty");
    let store = open_ok(&env.db_path());
    let record = add_ok(&store, &env.ws("registered"));

    let hit = store.touch_workspace(Path::new("")).unwrap();

    assert!(!hit, "空串边界穿透回退路径不命中");
    assert_eq!(store.list_workspaces().unwrap(), vec![record], "库内容不变");
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
fn 全链路add_list_touch_remove后重开同一db文件清单状态与各操作返回一致() {
    let env = Env::new("full-cycle");
    let (rec_a_root, snapshot) = {
        let store = open_ok(&env.db_path());
        let rec_a = add_ok(&store, &env.ws("alpha"));
        sleep(Duration::from_millis(2));
        let rec_b = add_ok(&store, &env.ws("beta"));
        sleep(Duration::from_millis(2));
        assert!(
            store.touch_workspace(Path::new(&rec_a.root)).unwrap(),
            "touch alpha 命中"
        );
        let list = store.list_workspaces().unwrap();
        let roots: Vec<String> = list.iter().map(|r| r.root.clone()).collect();
        assert_eq!(
            roots,
            vec![rec_a.root.clone(), rec_b.root],
            "操作后清单顺序符合预期"
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
