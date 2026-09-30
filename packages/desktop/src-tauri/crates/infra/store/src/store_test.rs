use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent::{AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunStatus};
use native_db::{Builder, Models};

use crate::store::{workspace_db_file_name, GLOBAL_DB_FILE_NAME};
use crate::{
    AgentEngineKind, AgentEventRecord, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord,
    AgentRunRecord, ExploreRecord, Store, StoreError, WorkspaceRecord, WorkspaceStores,
};

// ---------------------------------------------------------------------------
// 装置：单库直接构造器用 Env（db 文件 + workspace 根）与 WorkspaceStores 用
// StoresEnv（数据根 + workspace 根），tempfile RAII 测试结束自动清理
// ---------------------------------------------------------------------------

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

    /// db 文件路径（tag 区分全局库 / workspace 库文件：同一文件不能双维度混开）。
    fn db_path(&self, tag: &str) -> PathBuf {
        self.db_dir.path().join(format!("{tag}.redb"))
    }

    /// 在 workspace 根下创建一个真实目录并返回路径（add 的入参目录）。
    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }
}

/// 数据根 + workspace 根目录临时环境（`WorkspaceStores` 数据根注入口径）。
struct StoresEnv {
    data_root: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl StoresEnv {
    fn new(tag: &str) -> Self {
        let data_root = tempfile::Builder::new()
            .prefix(&format!("store-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("store-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_root, ws_root }
    }

    /// 打开两级库注册表（数据根注入，零环境解析）。
    fn open(&self) -> WorkspaceStores {
        WorkspaceStores::open(self.data_root.path())
            .unwrap_or_else(|e| panic!("WorkspaceStores::open 应成功: {e}"))
    }

    /// `workspaces/` 子树根（workspace 库文件派生基准）。
    fn workspaces_dir(&self) -> PathBuf {
        self.data_root.path().join("workspaces")
    }

    /// 在 workspace 根下创建一个真实目录并返回路径（for_root 入参目录）。
    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }

    /// 目录路径转 for_root 入参串（canonical 口径由 for_root 前置归一）。
    fn root_of(&self, name: &str) -> String {
        self.ws(name).to_string_lossy().into_owned()
    }

    /// `workspaces/` 子树内的 db 文件名清单（排序稳定）。
    fn workspace_db_files(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.workspaces_dir())
            .expect("读取 workspaces 子树失败")
            .map(|entry| {
                entry
                    .expect("遍历目录项失败")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }
}

fn open_global_ok(path: &Path) -> Store {
    Store::open_global(path).unwrap_or_else(|e| panic!("open_global 应成功: {e}"))
}

fn open_workspace_ok(path: &Path) -> Store {
    Store::open_workspace(path).unwrap_or_else(|e| panic!("open_workspace 应成功: {e}"))
}

fn add_ok(store: &Store, dir: &Path) -> WorkspaceRecord {
    store
        .add_workspace(dir)
        .unwrap_or_else(|e| panic!("add_workspace 应成功: {e}"))
}

// ---------------------------------------------------------------------------
// AC-1：双库布局——模型注册分组（两组静态注册无交叉）
// ---------------------------------------------------------------------------

#[test]
fn open_global列user维度三模型_open_workspace仅列workspace维度三行() {
    let env = Env::new("registry-split");

    let global = open_global_ok(&env.db_path("global"));
    let global_models = global.list_models().unwrap();
    assert_eq!(
        global_models
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace", "agent_provider", "agent_instance"],
        "user 组静态注册恰 workspace 注册表 + agent 管理两模型（空库计数 0 也列出）"
    );
    assert!(global_models.iter().all(|model| model.count == 0));
    drop(global);

    let ws = open_workspace_ok(&env.db_path("ws"));
    let ws_models = ws.list_models().unwrap();
    assert_eq!(
        ws_models
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        vec!["agent_run", "agent_event", "explore"],
        "workspace 组静态注册恰 run / 事件 / explore 三模型，两组无交叉"
    );
    assert!(ws_models.iter().all(|model| model.count == 0));
}

#[test]
fn 两级库open后经global操作注册表add与list落全局库() {
    let env = StoresEnv::new("global-registry");
    let stores = env.open();

    let record = add_ok(stores.global(), &env.ws("registered"));

    assert_eq!(
        stores.global().list_workspaces().unwrap(),
        vec![record],
        "add_workspace / list_workspaces 走全局库（user 维度注册表）"
    );
    assert!(
        env.data_root.path().join(GLOBAL_DB_FILE_NAME).exists(),
        "全局库文件落数据根 desktop-global.redb"
    );
    assert!(
        !env.workspaces_dir().exists(),
        "纯注册表操作不创建 workspace 库子树"
    );
}

#[test]
fn 跨维度模型名不可达_workspace库scan_workspace与全局库scan_agent_run均未知模型err() {
    let env = Env::new("cross-dimension");

    let ws = open_workspace_ok(&env.db_path("ws"));
    for name in ["workspace", "agent_provider", "agent_instance"] {
        let err = ws
            .scan(name, 0, 10)
            .expect_err("workspace 库 scan user 维度模型名应 Err（模型分组使混入在打开点不可能）");
        assert!(
            err.to_string().contains("未知模型"),
            "错误串含「未知模型」语境，实际: {err}"
        );
    }
    drop(ws);

    let global = open_global_ok(&env.db_path("global"));
    for name in ["agent_run", "agent_event", "explore"] {
        let err = global
            .scan(name, 0, 10)
            .expect_err("全局库 scan workspace 维度模型名应 Err");
        assert!(
            err.to_string().contains("未知模型"),
            "错误串含「未知模型」语境，实际: {err}"
        );
    }
}

#[test]
fn open_global与open_workspace对不存在路径创建db文件与父目录且空库可list() {
    let env = Env::new("open-create");

    let global_path = env.db_dir.path().join("nested/global/test.redb");
    assert!(!global_path.exists());
    let global = open_global_ok(&global_path);
    assert!(global_path.exists(), "open_global 创建 db 文件");
    assert!(global_path.parent().unwrap().is_dir(), "父目录被创建");
    assert!(global.list_workspaces().unwrap().is_empty(), "空库可 list");
    drop(global);

    let ws_path = env.db_dir.path().join("nested/ws/test.redb");
    let ws = open_workspace_ok(&ws_path);
    assert!(ws_path.exists(), "open_workspace 创建 db 文件");
    assert!(ws.list_agent_runs().unwrap().is_empty(), "空库可 list");
    assert!(ws.list_explore_records("").unwrap().is_empty());
}

#[test]
fn 父路径被同名普通文件占据时open_global返回db错误不panic() {
    let env = Env::new("open-blocked");
    let blocker = env.db_dir.path().join("blocker");
    fs::write(&blocker, "普通文件占位").expect("写占位文件失败");

    let result = Store::open_global(&blocker.join("test.redb"));

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
fn workspace库文件损坏时open_workspace返回err不静默降级为空库() {
    let env = Env::new("open-corrupt");
    let corrupt = env.db_path("ws");
    let garbage = "这不是一个合法的 db 数据库文件。".repeat(32);
    fs::write(&corrupt, garbage).expect("写损坏文件失败");

    let result = Store::open_workspace(&corrupt);

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("非合法 db 文件必须报错，不得静默降级为空库"),
    };
    assert!(
        err.to_string().starts_with("db:"),
        "错误串以 db: 前缀，实际: {err}"
    );
}

// ---------------------------------------------------------------------------
// AC-1/AC-3：workspace 库路径派生单点
// ---------------------------------------------------------------------------

#[test]
fn for_root派生的workspace库文件落workspaces子树且名为可读段加32位小写hex哈希() {
    let env = StoresEnv::new("derived-name");
    let stores = env.open();
    let root = env.root_of("alpha");

    stores.for_root(&root).expect("for_root 应成功");

    let files = env.workspace_db_files();
    assert_eq!(files.len(), 1, "workspaces/ 子树恰一个 db 文件");
    let name = &files[0];
    assert!(
        !name.contains('/') && !name.contains('\\') && !name.contains(':'),
        "派生文件名不含路径分隔符与盘符: {name}"
    );
    let stem = name
        .strip_suffix(".redb")
        .unwrap_or_else(|| panic!("文件名以 .redb 结尾: {name}"));
    let (readable, hash) = stem
        .rsplit_once('-')
        .unwrap_or_else(|| panic!("文件名形如 可读段-哈希: {name}"));
    assert_eq!(readable, "alpha", "可读段取 dir_name 末段");
    assert_eq!(hash.len(), 32, "哈希成分 32 位（SHA-256 前 16 字节 hex）");
    assert!(
        hash.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "哈希为小写 hex: {hash}"
    );

    // 抗碰撞：异根必不同名
    let other = env.root_of("beta");
    stores.for_root(&other).expect("for_root 应成功");
    assert_eq!(env.workspace_db_files().len(), 2, "异根各自独立文件");
    assert_ne!(files[0], env.workspace_db_files()[1]);
}

#[test]
fn 同root跨两级库生命周期派生同一路径_重开后run与explore完整可读() {
    let env = StoresEnv::new("reopen-same-path");
    let root = env.root_of("persist");

    let (run, explore, file_name) = {
        let stores = env.open();
        let store = stores.for_root(&root).expect("for_root 应成功");
        let run = begin_ok(&store, "重开前首轮", 100);
        let explore = create_ok(&store, &root, "topic");
        assert_eq!(env.workspace_db_files().len(), 1);
        (run, explore, env.workspace_db_files()[0].clone())
    }; // 整个 WorkspaceStores（含缓存实例）随作用域释放，文件锁归还

    let stores = env.open();
    let store = stores.for_root(&root).expect("重开 for_root 应成功");
    assert_eq!(
        env.workspace_db_files(),
        vec![file_name],
        "同 root 跨重开派生同一路径（不产生第二文件）"
    );
    assert_eq!(
        store.list_agent_runs().unwrap(),
        vec![run],
        "先写入的 run 重开后完整可读"
    );
    assert_eq!(
        store.list_explore_records(&root).unwrap(),
        vec![explore],
        "先写入的 explore 记录重开后完整可读"
    );
}

#[test]
fn 派生单点纯函数可读段清洗_截断_非法字符_尾点空格与空回退() {
    // 超 24 字符按 char boundary 截断（≤24 字符）
    let long = "C:\\ws\\a-very-long-workspace-directory-name";
    let name = workspace_db_file_name(long);
    let readable = readable_of(&name);
    assert_eq!(
        readable, "a-very-long-workspace-di",
        "超 24 字符截断为前 24 字符"
    );

    // 多字节字符按 char boundary 截断：恰 24 个四字节 emoji 不悬挂不 panic
    let emoji_root = "C:\\ws\\😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀";
    let name = workspace_db_file_name(emoji_root);
    let readable = readable_of(&name);
    assert_eq!(readable.chars().count(), 24, "char boundary 截断 24 字符");
    assert!(readable.chars().all(|c| c == '😀'), "截断不产生半个字符");

    // OS 非法字符（路径分隔与盘符、Windows 保留）逐字置换 _
    let illegal = "C:\\ws\\a/b\\c:d*e?f\"g<h>i|j";
    let name = workspace_db_file_name(illegal);
    let readable = readable_of(&name);
    assert_eq!(readable, "c_d_e_f_g_h_i_j", "非法字符逐字置换 _");
    assert!(
        !name.contains('/')
            && !name.contains('\\')
            && !name.contains(':')
            && !name.contains('*')
            && !name.contains('?')
            && !name.contains('"')
            && !name.contains('<')
            && !name.contains('>')
            && !name.contains('|'),
        "派生文件名不含任何 OS 非法字符: {name}"
    );

    // 尾部 `.` 与空格去除（Windows 保留语义）
    let dotted = "C:\\ws\\trailing...  ";
    assert_eq!(
        readable_of(&workspace_db_file_name(dotted)),
        "trailing",
        "尾部 `.` 与空格去除"
    );

    // 清洗后为空回退纯哈希名（32 位小写 hex，无可读段连字符）
    let empty = "C:\\ws\\...";
    let name = workspace_db_file_name(empty);
    let stem = name.strip_suffix(".redb").expect("以 .redb 结尾");
    assert!(!stem.contains('-'), "空可读段回退纯哈希名: {name}");
    assert_eq!(stem.len(), 32);
    assert!(
        stem.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "回退名为纯 32 位小写 hex: {stem}"
    );

    // 纯函数确定性：同根恒同名
    assert_eq!(
        workspace_db_file_name(long),
        workspace_db_file_name(long),
        "同根派生确定可复现"
    );
}

/// 取派生文件名的可读段（`可读段-哈希.redb` 的连字符前半）。
fn readable_of(file_name: &str) -> &str {
    let stem = file_name
        .strip_suffix(".redb")
        .unwrap_or_else(|| panic!("文件名以 .redb 结尾: {file_name}"));
    stem.rsplit_once('-')
        .unwrap_or_else(|| panic!("文件名形如 可读段-哈希: {file_name}"))
        .0
}

#[test]
fn for_root对同一目录的等价书写派生同一文件并复用同一缓存实例() {
    let env = StoresEnv::new("normalize");
    let stores = env.open();
    let root = env.root_of("NormDir");
    let first = stores.for_root(&root).expect("for_root 应成功");

    // 大小写不同书写（Windows 盘上真实大小写归一）
    let flipped = Path::new(&root)
        .with_file_name("nORMdIR")
        .to_string_lossy()
        .into_owned();
    let second = stores.for_root(&flipped).expect("等价书写 for_root 应成功");
    assert!(
        Arc::ptr_eq(&first, &second),
        "大小写不同书写经前置归一命中同一缓存实例"
    );

    // 尾分隔符书写
    let trailing = format!("{root}\\");
    let third = stores
        .for_root(&trailing)
        .expect("尾分隔符 for_root 应成功");
    assert!(Arc::ptr_eq(&first, &third), "尾分隔符归一同键");

    // 正反斜杠混写
    let forward = root.replace('\\', "/");
    let fourth = stores.for_root(&forward).expect("正斜杠 for_root 应成功");
    assert!(Arc::ptr_eq(&first, &fourth), "正反斜杠混写归一同键");

    assert_eq!(
        env.workspace_db_files().len(),
        1,
        "等价书写不产生第二个 db 文件（派生同一路径）"
    );
}

// ---------------------------------------------------------------------------
// AC-3：进程内单开与复用
// ---------------------------------------------------------------------------

#[test]
fn 同root连续两次for_root返回同一共享实例且可连续读写() {
    let env = StoresEnv::new("cache-reuse");
    let stores = env.open();
    let root = env.root_of("reuse");

    let first = stores.for_root(&root).expect("首次 for_root 应成功");
    let second = stores.for_root(&root).expect("二次 for_root 应成功");

    assert!(
        Arc::ptr_eq(&first, &second),
        "同 root 返回同一共享实例（不二次打开文件、无 redb 锁冲突）"
    );
    // 同一实例可连续读写
    let run = begin_ok(&first, "连续读写", 100);
    assert_eq!(
        second.list_agent_runs().unwrap(),
        vec![run],
        "经同一实例的写入对二次解析立即可见"
    );
}

#[test]
fn 不同root各自独立实例_写入互不可见_文件各自独立() {
    let env = StoresEnv::new("independent");
    let stores = env.open();
    let root_a = env.root_of("alpha");
    let root_b = env.root_of("beta");

    let store_a = stores.for_root(&root_a).expect("A for_root 应成功");
    let store_b = stores.for_root(&root_b).expect("B for_root 应成功");

    assert!(!Arc::ptr_eq(&store_a, &store_b), "不同 root 各自独立实例");
    let run_a = begin_ok(&store_a, "A 库首轮", 100);
    assert!(
        store_b.list_agent_runs().unwrap().is_empty(),
        "A 的写入对 B 不可见"
    );
    let run_b = begin_ok(&store_b, "B 库首轮", 200);
    assert_eq!(run_b.id, 1, "B 库 id 独立自增（不接续 A 库）");
    assert_eq!(store_a.list_agent_runs().unwrap(), vec![run_a]);
    assert_eq!(env.workspace_db_files().len(), 2, "两 root 文件各自独立");
}

#[test]
fn for_root派生路径上文件损坏时返回err不静默降级为空库() {
    let env = StoresEnv::new("for-root-corrupt");
    let root = env.root_of("victim");
    {
        let stores = env.open();
        stores.for_root(&root).expect("首次 for_root 应成功");
    } // 文件锁归还
    let corrupt = env
        .workspaces_dir()
        .join(env.workspace_db_files()[0].as_str());
    fs::write(&corrupt, "损坏的字节序列，不是合法 redb 文件。".repeat(16)).expect("写损坏文件失败");

    let stores = env.open();
    let result = stores.for_root(&root);

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("坏文件使用时必须暴露，不静默降级为空库"),
    };
    assert!(
        err.to_string().starts_with("db:"),
        "错误串以 db: 前缀，实际: {err}"
    );
}

// ---------------------------------------------------------------------------
// AC-2：注册表 → workspace 库分流写入（组合：WorkspaceStores 为两库入口）
// ---------------------------------------------------------------------------

#[test]
fn 组合链open注册后for_root落库run与explore且全局库无混入() {
    let env = StoresEnv::new("ac2-split");
    let stores = env.open();

    // 组合链：WorkspaceStores::open → global 注册 → for_root 写入
    let record = add_ok(stores.global(), &env.ws("split"));
    let root = record.root.clone();
    let store = stores.for_root(&root).expect("for_root 应成功");
    let run = begin_ok(&store, "分流首轮", 100);
    let explore = create_ok(&store, &record.root, "split-topic");

    // 全局库 user 维度模型行（注册面），注册表计数与注册记录数一致、两新管理
    // 模型计数 0 也列出
    let global_models = stores.global().list_models().unwrap();
    assert_eq!(
        global_models
            .iter()
            .map(|model| (model.name.as_str(), model.count))
            .collect::<Vec<_>>(),
        vec![("workspace", 1), ("agent_provider", 0), ("agent_instance", 0)],
        "全局库仅 user 维度模型行（三模型组）"
    );
    // workspace 库含 run / explore 行，注册表记录不混入
    let ws_models = store.list_models().unwrap();
    assert_eq!(
        ws_models
            .iter()
            .map(|model| (model.name.as_str(), model.count))
            .collect::<Vec<_>>(),
        vec![("agent_run", 1), ("agent_event", 0), ("explore", 1)],
        "workspace 库按新布局写入 run / explore，无注册表混入"
    );
    assert_eq!(
        stores.global().list_workspaces().unwrap(),
        vec![record],
        "全局库清单仅注册记录（run / explore 不在注册表）"
    );
    // 组合链写入各自完整可读
    assert_eq!(
        store.list_agent_runs().unwrap(),
        vec![run],
        "run 落 workspace 库"
    );
    assert_eq!(
        store.list_explore_records(&root).unwrap(),
        vec![explore],
        "explore 落 workspace 库"
    );
}

#[test]
fn 两workspace各自for_root同id并行写入互不串库() {
    let env = StoresEnv::new("ac2-parallel");
    let stores = env.open();
    let rec_a = add_ok(stores.global(), &env.ws("para-a"));
    let rec_b = add_ok(stores.global(), &env.ws("para-b"));
    let store_a = stores.for_root(&rec_a.root).expect("A for_root 应成功");
    let store_b = stores.for_root(&rec_b.root).expect("B for_root 应成功");

    // 两库各自 max+1 分配：同 id 并行（库域内自增，跨 workspace 不假定全局唯一）
    let run_a = begin_ok(&store_a, "A 库首轮", 100);
    let run_b = begin_ok(&store_b, "B 库首轮", 200);
    assert_eq!((run_a.id, run_b.id), (1, 1), "同 id 并行");
    let explore_a = create_ok(&store_a, &rec_a.root, "同名话题");
    let explore_b = create_ok(&store_b, &rec_b.root, "同名话题");

    // A 库清单与 scan 不含 B 的任何记录
    let runs_a = store_a.list_agent_runs().unwrap();
    let prompts_a: Vec<&str> = runs_a.iter().map(|record| record.prompt.as_str()).collect();
    assert_eq!(prompts_a, vec!["A 库首轮"], "A 库 run 清单不含 B 的记录");
    let ids_a: Vec<i64> = store_a
        .scan("agent_run", 0, 10)
        .unwrap()
        .iter()
        .map(|envelope| envelope.key.as_i64().expect("agent_run key 为数值"))
        .collect();
    assert_eq!(ids_a, vec![1], "A 库 scan 恰本库一行");
    assert_eq!(
        store_a.list_explore_records(&rec_a.root).unwrap(),
        vec![explore_a],
        "A 库 explore 清单不含 B 的记录"
    );
    assert_eq!(
        store_b.list_explore_records(&rec_b.root).unwrap(),
        vec![explore_b],
        "B 库同名记录互不冲突"
    );
    assert_eq!(env.workspace_db_files().len(), 2, "两库文件各自独立");
}

// ---------------------------------------------------------------------------
// AC-4：全新文件组冷启动（零迁移）
// ---------------------------------------------------------------------------

/// 预置旧布局单库文件（`desktop-store.redb`，四模型静态注册 + 各写一条记录）：
/// 新代码已无单库打开入口，直接以 native_db 裸构造四模型库写入（不经
/// `Store`），模拟零迁移语义中的「旧布局残留」。返回写入后的文件字节。
fn preset_old_layout_db(path: &Path, ws_root_key: &str) -> Vec<u8> {
    let mut models = Models::new();
    models
        .define::<WorkspaceRecord>()
        .expect("定义 WorkspaceRecord 失败");
    models
        .define::<AgentRunRecord>()
        .expect("定义 AgentRunRecord 失败");
    models
        .define::<AgentEventRecord>()
        .expect("定义 AgentEventRecord 失败");
    models
        .define::<ExploreRecord>()
        .expect("定义 ExploreRecord 失败");
    {
        let db = Builder::new()
            .create(&models, path)
            .expect("预置旧布局库失败");
        let rw = db.rw_transaction().expect("开启写事务失败");
        rw.insert(WorkspaceRecord::from_root(ws_root_key, 1000))
            .expect("写入 workspace 记录失败");
        rw.insert(AgentRunRecord {
            id: 7,
            prompt: "旧库首轮".to_owned(),
            cwd: ws_root_key.to_owned(),
            env: AgentEnvMode::Default,
            permission_mode: AgentPermissionMode::BypassPermissions,
            status: AgentRunStatus::Running,
            started_at: 1100,
            finished_at: None,
            num_turns: None,
            cost_usd: None,
            duration_ms: None,
            session_id: None,
            error: None,
            source: "debug".to_owned(),
            source_ref: None,
            parent_run_id: None,
        })
        .expect("写入 run 记录失败");
        rw.insert(AgentEventRecord::new(7, stamped(0, run_started_kind())))
            .expect("写入事件失败");
        rw.insert(ExploreRecord::new(ws_root_key, "旧档案", 1200))
            .expect("写入 explore 记录失败");
        rw.commit().expect("提交预置事务失败");
    }
    fs::read(path).expect("读取旧布局文件失败")
}

#[test]
fn 预置旧四模型单库文件后新布局冷启动照常成功且旧文件保持原样() {
    let env = StoresEnv::new("cold-start");
    let old_path = env.data_root.path().join("desktop-store.redb");
    // 旧库内注册的 workspace root（新布局不得读取该记录）
    let legacy_root = env.root_of("legacy");
    let old_bytes = preset_old_layout_db(&old_path, &legacy_root);

    let stores = env.open();

    // 新布局启动照常成功：全局库从空开始（旧注册记录不读入）
    assert!(
        stores.global().list_workspaces().unwrap().is_empty(),
        "旧布局注册表不读入新全局库"
    );
    // workspace 库从空开始按新布局写入：id 从 1 起不接续旧库 id 域
    let store = stores.for_root(&legacy_root).expect("for_root 应成功");
    let run = begin_ok(&store, "新库首轮", 100);
    assert_eq!(run.id, 1, "新库 id 域从 1 起（不接续旧库 id=7）");
    let explore = create_ok(&store, &legacy_root, "fresh");
    assert_eq!(explore.id, 1, "新库 explore id 从 1 起（不接续旧库 id=5）");

    // 旧文件不读、不改名、不删除：文件名与字节保持原样
    assert!(old_path.exists(), "旧布局文件不被删除");
    assert_eq!(
        old_path.file_name().unwrap().to_string_lossy(),
        "desktop-store.redb",
        "不改名"
    );
    assert_eq!(
        fs::read(&old_path).expect("读取旧布局文件失败"),
        old_bytes,
        "旧文件字节保持原样"
    );
    assert_eq!(
        env.workspace_db_files().len(),
        1,
        "workspace 库按新布局落 workspaces/ 子树"
    );
}

#[test]
fn 全局库文件损坏时两级库open返回err不静默降级() {
    let env = StoresEnv::new("global-corrupt");
    let global_path = env.data_root.path().join(GLOBAL_DB_FILE_NAME);
    let garbage = "全局库损坏字节序列。".repeat(64);
    fs::write(&global_path, garbage).expect("写损坏全局库失败");

    let result = WorkspaceStores::open(env.data_root.path());

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("全局库打不开必须 Err（setup fail fast 口径）"),
    };
    assert!(
        err.to_string().starts_with("db:"),
        "错误串以 db: 前缀，实际: {err}"
    );
    assert!(
        fs::read(&global_path).expect("读取损坏文件失败").len() > 0,
        "fail fast 不破坏现场文件"
    );
}

// ---------------------------------------------------------------------------
// Store::add_workspace（全局库注册表存量回归）
// ---------------------------------------------------------------------------

#[test]
fn add新目录返回canonical完整路径name为目录名末段() {
    let env = Env::new("add-basic");
    let store = open_global_ok(&env.db_path("global"));
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
    let store = open_global_ok(&env.db_path("global"));
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
    let store = open_global_ok(&env.db_path("global"));
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
    let store = open_global_ok(&env.db_path("global"));
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
    // 记录经模型编解码落库：读回与返回记录逐字段相等（往返无损）
    assert_eq!(store.list_workspaces().unwrap(), vec![record]);
}

#[test]
fn add不存在的目录返回canonicalize错误() {
    let env = Env::new("add-missing");
    let store = open_global_ok(&env.db_path("global"));
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
    let store = open_global_ok(&env.db_path("global"));

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
    let store = open_global_ok(&env.db_path("global"));
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
    let store = open_global_ok(&env.db_path("global"));
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
    let store = open_global_ok(&env.db_path("global"));

    assert!(store.list_workspaces().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Store::remove_workspace
// ---------------------------------------------------------------------------

#[test]
fn remove已存在key返回true且list不再含该项() {
    let env = Env::new("remove-hit");
    let store = open_global_ok(&env.db_path("global"));
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
    let store = open_global_ok(&env.db_path("global"));
    let record = add_ok(&store, &env.ws("registered"));
    let ghost = env.ws("ghost");

    let hit = store.remove_workspace(&ghost).unwrap();

    assert!(!hit, "未注册路径 remove 幂等 miss");
    assert_eq!(store.list_workspaces().unwrap(), vec![record], "库内容不变");
}

#[test]
fn remove大小写不同等价路径命中删除同一条无孤儿条目() {
    let env = Env::new("remove-case");
    let store = open_global_ok(&env.db_path("global"));
    let record = add_ok(&store, &env.ws("CaseKey"));

    let flipped = Path::new(&record.root).with_file_name("cASEkEY");
    let hit = store.remove_workspace(&flipped).unwrap();

    assert!(hit, "等价路径命中同一条");
    assert!(store.list_workspaces().unwrap().is_empty(), "无孤儿条目");
}

#[test]
fn remove目录消失后回退匹配命中删除() {
    let env = Env::new("remove-vanish");
    let store = open_global_ok(&env.db_path("global"));
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
    let global_path = env.db_path("global");
    let (rec_a_root, snapshot) = {
        let store = open_global_ok(&global_path);
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
    let reopened = open_global_ok(&global_path);
    let after_reopen = reopened.list_workspaces().unwrap();
    assert_eq!(after_reopen, snapshot);

    // 再经一次 remove + 重开：删除同样持久化，仅剩 beta
    assert!(reopened.remove_workspace(Path::new(&rec_a_root)).unwrap());
    drop(reopened);
    let again = open_global_ok(&global_path);
    let remaining = again.list_workspaces().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].root, snapshot[1].root);
}

// ---------------------------------------------------------------------------
// agent 域（workspace 库）：begin / finish / list_runs 存量回归
// ---------------------------------------------------------------------------

/// 构造一份 running 形态的 run 记录（id 由 begin 分配，入参不参与匹配）。
/// 三字段直写契约枚举（v3 起落库载体即枚举）。
fn running_run(prompt: &str, started_at: i64) -> AgentRunRecord {
    AgentRunRecord {
        id: 0,
        prompt: prompt.to_owned(),
        cwd: "C:\\ws\\demo".to_owned(),
        env: AgentEnvMode::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
        status: AgentRunStatus::Running,
        started_at,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
        source: "debug".to_owned(),
        source_ref: None,
        parent_run_id: None,
    }
}

fn begin_ok(store: &Store, prompt: &str, started_at: i64) -> AgentRunRecord {
    store
        .begin_agent_run(&running_run(prompt, started_at))
        .unwrap_or_else(|e| panic!("begin_agent_run 应成功: {e}"))
}

#[test]
fn begin_agent_run空库首跑返回id为1且status为running且started_at落值() {
    let env = Env::new("agent-begin-first");
    let store = open_workspace_ok(&env.db_path("ws"));

    let record = begin_ok(&store, "首轮", 1727000000000);

    assert_eq!(record.id, 1, "空库首跑 max+1 分配 id=1");
    assert_eq!(record.status, AgentRunStatus::Running, "落 running 行");
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
    let store = open_workspace_ok(&env.db_path("ws"));

    let first = begin_ok(&store, "第一跑", 100);
    let second = begin_ok(&store, "第二跑", 200);
    let third = begin_ok(&store, "第三跑", 300);

    assert_eq!((first.id, second.id, third.id), (1, 2, 3), "max+1 严格递增");
}

#[test]
fn begin_agent_run传入记录的id字段不参与匹配以分配id落行为准() {
    let env = Env::new("agent-begin-id");
    let store = open_workspace_ok(&env.db_path("ws"));

    let mut requested = running_run("调用方自填 id", 1727000000000);
    requested.id = 999;
    let record = store.begin_agent_run(&requested).unwrap();

    assert_eq!(record.id, 1, "id 由写事务内 max+1 分配，入参 id 被覆盖");
    assert_eq!(store.list_agent_runs().unwrap()[0].id, 1, "以分配 id 落行");
}

#[test]
fn append空切片返回ok且不产生行() {
    let env = Env::new("agent-append-empty");
    let store = open_workspace_ok(&env.db_path("ws"));
    let run = begin_ok(&store, "空事件流", 1727000000000);

    store.append_agent_run_events(run.id, &[]).unwrap();

    assert!(store.list_agent_run_events(run.id).unwrap().is_empty());
}

#[test]
fn finish后整行替换为终态且list反映() {
    let env = Env::new("agent-finish");
    let store = open_workspace_ok(&env.db_path("ws"));
    let run = begin_ok(&store, "待收敛", 1727000000000);

    let mut finished = run.clone();
    finished.status = AgentRunStatus::Completed;
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
    assert_eq!(listed[0].status, AgentRunStatus::Completed, "终态整行替换");
    assert_eq!(listed[0].num_turns, Some(4));
    assert_eq!(listed[0].cost_usd, Some(0.5));
    assert_eq!(listed[0].duration_ms, Some(999));
    assert_eq!(listed[0].session_id.as_deref(), Some("s-1"));
    assert_eq!(listed[0].finished_at, Some(1727000001000));
}

#[test]
fn list_agent_runs按started_at降序并列时按id降序() {
    let env = Env::new("agent-list-order");
    let store = open_workspace_ok(&env.db_path("ws"));
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
    let store = open_workspace_ok(&env.db_path("ws"));

    assert!(store.list_agent_runs().unwrap().is_empty());
}

#[test]
fn 不存在run_id的list_agent_run_events返回空向量不报错() {
    let env = Env::new("agent-events-miss");
    let store = open_workspace_ok(&env.db_path("ws"));

    assert!(store.list_agent_run_events(42).unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// workspace 库 native 格式重开直通（run 与事件）
// ---------------------------------------------------------------------------

#[test]
fn native格式已有库重开直通此前写入的run与事件完整读回() {
    let env = Env::new("reopen-native");
    let ws_path = env.db_path("ws");

    let (run, events) = {
        let store = open_workspace_ok(&ws_path);
        let run = begin_ok(&store, "native 重开", 1727000000000);
        let events = vec![
            stamped(0, run_started_kind()),
            stamped(1, raw_kind("native")),
        ];
        store
            .append_agent_run_events(run.id, &events)
            .unwrap_or_else(|e| panic!("append 应成功: {e}"));
        (run, events)
    };

    // native 格式已有库：重开直通，全部记录完整读回
    let reopened = open_workspace_ok(&ws_path);
    assert_eq!(reopened.list_agent_runs().unwrap(), vec![run.clone()]);
    assert_eq!(reopened.list_agent_run_events(run.id).unwrap(), events);
}

// ---------------------------------------------------------------------------
// 事件类型化：append / list_agent_run_events（五变体、重复合成主键、千级 seq、
// 两 run 隔离）
// ---------------------------------------------------------------------------

/// 以指定 kind 构造盖戳事件（seq 由调用方给定，时间戳取当前钟面）。
fn stamped(seq: u64, kind: AgentEventKind) -> AgentEvent {
    AgentEvent::stamp(seq, kind)
}

fn run_started_kind() -> AgentEventKind {
    AgentEventKind::RunStarted {
        model: Some("claude-opus".to_owned()),
        session_id: Some("s-1".to_owned()),
        tools: vec!["Bash".to_owned()],
        mcp_servers: Vec::new(),
    }
}

fn message_kind(role: &str) -> AgentEventKind {
    AgentEventKind::Message {
        role: role.to_owned(),
        blocks: Vec::new(),
        parent_tool_use_id: None,
    }
}

fn system_notice_kind(subtype: &str) -> AgentEventKind {
    AgentEventKind::SystemNotice {
        subtype: subtype.to_owned(),
        payload: serde_json::json!({ "attempt": 2 }),
    }
}

fn run_result_kind(is_error: bool) -> AgentEventKind {
    AgentEventKind::RunResult {
        subtype: if is_error {
            "error_max_turns"
        } else {
            "success"
        }
        .to_owned(),
        is_error,
        num_turns: Some(1),
        duration_ms: Some(1234),
        cost_usd: Some(0.5),
        usage: serde_json::Value::Null,
        session_id: Some("s-1".to_owned()),
    }
}

/// Raw 变体：`tag` 进入原文载荷，供归属/保真断言提取。
fn raw_kind(tag: &str) -> AgentEventKind {
    AgentEventKind::Raw {
        event_type: "mystery".to_owned(),
        raw_json: format!(r#"{{"type":"mystery","tag":"{tag}"}}"#),
    }
}

/// 从 Raw 变体提取 tag（fixture 口径漂移即 panic）。
fn raw_tag(event: &AgentEvent) -> &str {
    match &event.kind {
        AgentEventKind::Raw { raw_json, .. } => raw_json
            .split(r#""tag":""#)
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_else(|| panic!("raw fixture 无 tag: {event:?}")),
        other => panic!("期望 Raw 变体，实际: {other:?}"),
    }
}

#[test]
fn append五变体类型化批量追加后重放seq升序逐字段保真() {
    let env = Env::new("append-five");
    let store = open_workspace_ok(&env.db_path("ws"));
    let run = begin_ok(&store, "五变体", 1727000000000);
    let seeded = vec![
        stamped(0, run_started_kind()),
        stamped(1, message_kind("assistant")),
        stamped(2, system_notice_kind("api_retry")),
        stamped(3, run_result_kind(false)),
        stamped(4, raw_kind("mystery-tag")),
    ];

    store
        .append_agent_run_events(run.id, &seeded)
        .unwrap_or_else(|e| panic!("批量追加应成功: {e}"));

    let replayed = store.list_agent_run_events(run.id).unwrap();
    assert_eq!(
        replayed, seeded,
        "类型化批量落库后重放逐字段保真（含 Raw 逃生舱）"
    );
    let seqs: Vec<u64> = replayed.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3, 4], "重放 seq 升序");
}

#[test]
fn 同run重复seq二次追加由合成主键冲突拒绝且重放恰一条() {
    let env = Env::new("append-dup-seq");
    let store = open_workspace_ok(&env.db_path("ws"));
    let run = begin_ok(&store, "重复 seq", 1727000000000);
    let first = stamped(0, raw_kind("first"));

    store.append_agent_run_events(run.id, &[first]).unwrap();

    // 同 (run_id, seq) 再追加：合成主键冲突（native_db insert 语义），二次追加报错
    let second = stamped(0, raw_kind("second"));
    let result = store.append_agent_run_events(run.id, &[second]);
    assert!(
        result.is_err(),
        "同合成主键二次追加被拒绝，实际: {result:?}"
    );

    // 重放面恰一条：不产生重复行（与旧载体「重放不重」口径一致）
    let replayed = store.list_agent_run_events(run.id).unwrap();
    assert_eq!(replayed.len(), 1, "重放恰一条不重复");
    assert_eq!(raw_tag(&replayed[0]), "first", "保留先写入的一条");
}

#[test]
fn 单run千级seq批量追加后重放序完整不回绕() {
    let env = Env::new("append-thousand");
    let store = open_workspace_ok(&env.db_path("ws"));
    let run = begin_ok(&store, "千级 seq", 1727000000000);
    let seeded: Vec<AgentEvent> = (0..1000u64)
        .map(|seq| stamped(seq, raw_kind(&seq.to_string())))
        .collect();

    store
        .append_agent_run_events(run.id, &seeded)
        .unwrap_or_else(|e| panic!("批量追加应成功: {e}"));

    let replayed = store.list_agent_run_events(run.id).unwrap();
    assert_eq!(replayed.len(), 1000, "千级事件一条不丢");
    // u128 打包键在大 seq 下保序：重放序完整、严格单调不回绕
    for (index, event) in replayed.iter().enumerate() {
        assert_eq!(
            event.seq, index as u64,
            "重放第 {index} 条 seq 恰为 {index}"
        );
    }
}

#[test]
fn 两run同seq区间互不串扰经run_id二级索引隔离() {
    let env = Env::new("append-isolation");
    let store = open_workspace_ok(&env.db_path("ws"));
    let run_a = begin_ok(&store, "run A", 100);
    let run_b = begin_ok(&store, "run B", 200);
    assert_ne!(run_a.id, run_b.id);
    let events_a: Vec<AgentEvent> = (0..4u64).map(|seq| stamped(seq, raw_kind("A"))).collect();
    let events_b: Vec<AgentEvent> = (0..4u64).map(|seq| stamped(seq, raw_kind("B"))).collect();
    // 两 run 落完全重叠的 seq 区间：run_id 高位隔离缺失即串扰
    store.append_agent_run_events(run_a.id, &events_a).unwrap();
    store.append_agent_run_events(run_b.id, &events_b).unwrap();

    let replay_a = store.list_agent_run_events(run_a.id).unwrap();
    assert_eq!(replay_a, events_a, "run A 重放恰为自己 seq 区间的四条");
    assert!(replay_a.iter().all(|event| raw_tag(event) == "A"));
    let replay_b = store.list_agent_run_events(run_b.id).unwrap();
    assert_eq!(replay_b, events_b, "run B 重放恰为自己 seq 区间的四条");
    assert!(replay_b.iter().all(|event| raw_tag(event) == "B"));
}

// ---------------------------------------------------------------------------
// 信封 API（分维度）：list_models 计数一致性 + scan 分页正向往返（分页边界
// 与信封形态矩阵见 envelope_test.rs）
// ---------------------------------------------------------------------------

#[test]
fn list_models分维度计数与各库实有记录数一致() {
    let env = Env::new("models-counts");
    // 全局库：注册表两笔
    let global = open_global_ok(&env.db_path("global"));
    add_ok(&global, &env.ws("alpha"));
    add_ok(&global, &env.ws("beta"));
    let global_models = global.list_models().unwrap();
    assert_eq!(
        global_models
            .iter()
            .map(|model| (model.name.as_str(), model.count))
            .collect::<Vec<_>>(),
        vec![("workspace", 2), ("agent_provider", 0), ("agent_instance", 0)],
        "全局库 workspace 计数与注册记录数一致（agent 管理两模型计数 0 也列出）"
    );
    drop(global);

    // workspace 库：run 1 + 事件 3，explore 计数 0 也列出
    let ws = open_workspace_ok(&env.db_path("ws"));
    let run = begin_ok(&ws, "计数复核", 1727000000000);
    let events: Vec<AgentEvent> = (0..3u64).map(|seq| stamped(seq, raw_kind("c"))).collect();
    ws.append_agent_run_events(run.id, &events).unwrap();

    let models = ws.list_models().unwrap();
    let count_of = |name: &str| {
        models
            .iter()
            .find(|model| model.name == name)
            .unwrap_or_else(|| panic!("模型 {name} 应在清单中"))
            .count
    };
    assert_eq!(count_of("agent_run"), 1, "agent_run 计数与实有记录数一致");
    assert_eq!(
        count_of("agent_event"),
        3,
        "agent_event 计数与实有记录数一致"
    );
    assert_eq!(count_of("explore"), 0, "计数 0 也列出");
}

#[test]
fn scan分页按offset_limit返回主键自然序翻页拼接不重不漏() {
    let env = Env::new("scan-paging");
    let store = open_global_ok(&env.db_path("global"));
    let seeded: Vec<WorkspaceRecord> = ["alpha", "beta", "gamma", "delta", "epsilon"]
        .iter()
        .map(|name| add_ok(&store, &env.ws(name)))
        .collect();

    let all_keys = |offset: u32, limit: u32| -> Vec<String> {
        store
            .scan("workspace", offset, limit)
            .unwrap_or_else(|e| panic!("scan 应成功: {e}"))
            .into_iter()
            .map(|envelope| {
                envelope
                    .key
                    .as_str()
                    .expect("workspace key 为字符串")
                    .to_owned()
            })
            .collect()
    };

    let page1 = all_keys(0, 2);
    let page2 = all_keys(2, 2);
    let page3 = all_keys(4, 2);
    let mut union = page1;
    union.extend(page2);
    union.extend(page3);
    let mut sorted = union.clone();
    sorted.sort();
    assert_eq!(sorted, union, "翻页拼接不重不漏且全局唯一");
    let mut expected: Vec<String> = seeded.iter().map(|record| record.root.clone()).collect();
    expected.sort();
    assert_eq!(union, expected, "拼接结果恰为全部记录的主键自然序");
}

#[test]
fn scan未知模型名与空串返回err不panic且错误串可读() {
    let env = Env::new("scan-unknown");
    let store = open_global_ok(&env.db_path("global"));

    for name in ["nope", ""] {
        let result = store.scan(name, 0, 10);
        let err = result.expect_err(&format!("未知模型 {name:?} 应 Err"));
        assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
        assert!(
            err.to_string().contains("未知模型"),
            "错误串含「未知模型」语境便于排查，实际: {err}"
        );
    }
    // 失败不产生任何副作用：库仍可正常读写
    assert!(store.list_workspaces().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// explore 记录 CRUD（workspace 库）：建档 / 清单 / 寻址 / 改名 / 删除
// ---------------------------------------------------------------------------

fn create_ok(store: &Store, root: &str, name: &str) -> ExploreRecord {
    store
        .create_explore_record(root, name)
        .unwrap_or_else(|e| panic!("create_explore_record({name}) 应成功: {e}"))
}

#[test]
fn create_explore_record两次建档id递增且created_at等于updated_at() {
    let env = Env::new("explore-create-incr");
    let store = open_workspace_ok(&env.db_path("ws"));

    let first = create_ok(&store, "C:\\ws\\alpha", "api-retry");
    let second = create_ok(&store, "C:\\ws\\alpha", "layout-design");

    assert_eq!(
        (first.id, second.id),
        (1, 2),
        "写事务内 max+1 分配，空库首行 id=1"
    );
    for record in [&first, &second] {
        assert_eq!(
            record.created_at, record.updated_at,
            "新建语义 created_at = updated_at（入库时刻毫秒值）"
        );
        assert!(record.created_at > 0, "入库时刻为正毫秒值");
    }
}

#[test]
fn 同root同name重复建档返回err() {
    let env = Env::new("explore-create-dup");
    let store = open_workspace_ok(&env.db_path("ws"));
    let first = create_ok(&store, "C:\\ws\\alpha", "api-retry");

    let result = store.create_explore_record("C:\\ws\\alpha", "api-retry");

    let err = result.expect_err("同 (root, name) 重复建档应 Err");
    assert!(
        matches!(err, StoreError::Db(_)),
        "变体为 Db（命令层转 Err(String)），实际: {err:?}"
    );
    assert_eq!(
        store.list_explore_records("C:\\ws\\alpha").unwrap(),
        vec![first],
        "失败不产生第二条记录"
    );
}

#[test]
fn 非法记录名建档返回err() {
    let env = Env::new("explore-create-invalid");
    let store = open_workspace_ok(&env.db_path("ws"));

    for name in ["", ".", "..", "a/b", "a\\b", "a:b", "../x"] {
        let result = store.create_explore_record("C:\\ws\\alpha", name);
        assert!(
            matches!(&result, Err(StoreError::Db(_))),
            "非法名 {name:?} 应 Err（单分量校验），实际: {result:?}"
        );
    }
    assert!(
        store
            .list_explore_records("C:\\ws\\alpha")
            .unwrap()
            .is_empty(),
        "全部拒绝：不产生任何记录"
    );
}

#[test]
fn 同名不同root各建一档互不影响() {
    let env = Env::new("explore-create-roots");
    let store = open_workspace_ok(&env.db_path("ws"));

    let alpha = create_ok(&store, "C:\\ws\\alpha", "api-retry");
    let beta = create_ok(&store, "C:\\ws\\beta", "api-retry");

    assert_ne!(
        alpha.id, beta.id,
        "主键独立分配（root 是归属键，非主键分量）"
    );
    assert_eq!(
        store
            .find_explore_record("C:\\ws\\alpha", "api-retry")
            .unwrap(),
        Some(alpha),
        "各 root 自行寻址互不影响"
    );
    assert_eq!(
        store
            .find_explore_record("C:\\ws\\beta", "api-retry")
            .unwrap(),
        Some(beta)
    );
}

#[test]
fn list_explore_records仅返回入参root的记录且id升序() {
    let env = Env::new("explore-list");
    let store = open_workspace_ok(&env.db_path("ws"));
    let a1 = create_ok(&store, "C:\\ws\\alpha", "a-first");
    let _b1 = create_ok(&store, "C:\\ws\\beta", "b-only");
    let a2 = create_ok(&store, "C:\\ws\\alpha", "a-second");
    let a3 = create_ok(&store, "C:\\ws\\alpha", "a-third");

    let list = store.list_explore_records("C:\\ws\\alpha").unwrap();

    let ids: Vec<i64> = list.iter().map(|record| record.id).collect();
    assert_eq!(
        ids,
        vec![a1.id, a2.id, a3.id],
        "仅入参 root 的记录、主键 id 升序稳定序"
    );
    assert!(
        list.iter().all(|record| record.root == "C:\\ws\\alpha"),
        "无他 root 记录混入"
    );
}

#[test]
fn 空root串清单返回空vec() {
    let env = Env::new("explore-list-blank");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_ok(&store, "C:\\ws\\alpha", "api-retry");

    assert!(
        store.list_explore_records("").unwrap().is_empty(),
        "blank root 不匹配任何归属键，返回空 Vec（blank root 纪律的 store 半）"
    );
}

#[test]
fn find_explore_record命中root与name返回some() {
    let env = Env::new("explore-find-hit");
    let store = open_workspace_ok(&env.db_path("ws"));
    let record = create_ok(&store, "C:\\ws\\alpha", "api-retry");

    assert_eq!(
        store
            .find_explore_record("C:\\ws\\alpha", "api-retry")
            .unwrap(),
        Some(record),
        "命中按归属与名称寻址"
    );
}

#[test]
fn find_explore_record未命中name或root返回none() {
    let env = Env::new("explore-find-miss");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_ok(&store, "C:\\ws\\alpha", "api-retry");

    assert_eq!(
        store
            .find_explore_record("C:\\ws\\alpha", "no-such")
            .unwrap(),
        None,
        "name 未命中"
    );
    assert_eq!(
        store
            .find_explore_record("C:\\ws\\beta", "api-retry")
            .unwrap(),
        None,
        "root 未命中"
    );
}

#[test]
fn rename原地改名主键不变旧名不再命中新名命中() {
    let env = Env::new("explore-rename");
    let store = open_workspace_ok(&env.db_path("ws"));
    let original = create_ok(&store, "C:\\ws\\alpha", "old-name");

    let renamed = store
        .rename_explore_record("C:\\ws\\alpha", "old-name", "new-name")
        .expect("rename 应成功");

    assert_eq!(
        renamed.id, original.id,
        "in-place 改名保主键（保 source_ref 链绑定）"
    );
    assert_eq!(renamed.name, "new-name");
    assert!(
        renamed.updated_at >= original.updated_at,
        "updated_at 刷新（不早于原值）"
    );
    assert_eq!(
        store
            .find_explore_record("C:\\ws\\alpha", "old-name")
            .unwrap(),
        None,
        "原 (root, old_name) 不再命中"
    );
    assert_eq!(
        store
            .find_explore_record("C:\\ws\\alpha", "new-name")
            .unwrap(),
        Some(renamed),
        "(root, new_name) 命中"
    );
}

#[test]
fn rename目标名已存在返回err且原记录不被破坏() {
    let env = Env::new("explore-rename-conflict");
    let store = open_workspace_ok(&env.db_path("ws"));
    let keeper = create_ok(&store, "C:\\ws\\alpha", "keeper");
    create_ok(&store, "C:\\ws\\alpha", "mover");

    let result = store.rename_explore_record("C:\\ws\\alpha", "mover", "keeper");

    assert!(
        matches!(&result, Err(StoreError::Db(_))),
        "目标名已存在应 Err，实际: {result:?}"
    );
    assert_eq!(
        store
            .find_explore_record("C:\\ws\\alpha", "mover")
            .unwrap()
            .map(|r| r.name),
        Some("mover".to_owned()),
        "原记录不被破坏"
    );
    assert_eq!(
        store
            .find_explore_record("C:\\ws\\alpha", "keeper")
            .unwrap(),
        Some(keeper),
        "同名既有记录不受影响"
    );
}

#[test]
fn rename被改记录miss返回err() {
    let env = Env::new("explore-rename-miss");
    let store = open_workspace_ok(&env.db_path("ws"));

    let result = store.rename_explore_record("C:\\ws\\alpha", "ghost", "any");
    assert!(
        matches!(&result, Err(StoreError::Db(_))),
        "被改记录不存在应 Err，实际: {result:?}"
    );
}

#[test]
fn delete后find为none返回true且磁盘同名文件保留() {
    let env = Env::new("explore-delete");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_ok(&store, "C:\\ws\\alpha", "api-retry");
    // 记录对应的磁盘笔记文件（store 不触磁盘：文件由 agent 会话流程创建）
    let note = env.ws("alpha").join("api-retry.md");
    fs::write(&note, "# 探索笔记").expect("写磁盘笔记失败");

    let hit = store
        .delete_explore_record("C:\\ws\\alpha", "api-retry")
        .unwrap();

    assert!(hit, "命中删除返回 true");
    assert_eq!(
        store
            .find_explore_record("C:\\ws\\alpha", "api-retry")
            .unwrap(),
        None,
        "删除后不再命中"
    );
    assert!(
        note.exists(),
        "孤儿保留：删除记录不动磁盘文件（文件是可丢弃投影）"
    );
    assert_eq!(
        fs::read_to_string(&note).unwrap(),
        "# 探索笔记",
        "文件内容原样"
    );
}

#[test]
fn delete_miss幂等返回false() {
    let env = Env::new("explore-delete-miss");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_ok(&store, "C:\\ws\\alpha", "api-retry");

    assert!(
        !store
            .delete_explore_record("C:\\ws\\alpha", "ghost")
            .unwrap(),
        "miss 返回 false"
    );
    assert!(store
        .delete_explore_record("C:\\ws\\alpha", "api-retry")
        .unwrap());
    assert!(
        !store
            .delete_explore_record("C:\\ws\\alpha", "api-retry")
            .unwrap(),
        "重复删除幂等（二次 miss）"
    );
}

// ---------------------------------------------------------------------------
// restore_run_chain：单链还原收口单点
// ---------------------------------------------------------------------------

/// 级联回归（原错链 BUG 的修复语义）：删除记录时其名下 runs 与事件随记录
/// 同事务删除——id 仍是幸存行上 max+1 的可复用计数,但悬空 `source_ref` 已清,
/// 新建记录即使复用被删 id,链还原也不再捞到旧聊天。
#[test]
fn 删除记录级联清掉名下runs与事件且复用id不错链() {
    let env = Env::new("explore-delete-cascade");
    let store = open_workspace_ok(&env.db_path("ws"));
    let _a = create_ok(&store, "C:\\ws\\alpha", "old-topic");
    let b = create_ok(&store, "C:\\ws\\alpha", "to-be-deleted");
    // 被删记录名下已有一段 agent 聊天（source_ref = b.id 十进制串）及其事件
    let run = begin_provenance_run(
        &store,
        "旧聊天",
        100,
        "explore",
        Some(&b.id.to_string()),
        None,
    );
    store
        .append_agent_run_events(
            run.id,
            &[
                stamped(0, run_started_kind()),
                stamped(1, message_kind("assistant")),
            ],
        )
        .unwrap();

    assert!(
        store
            .delete_explore_record("C:\\ws\\alpha", "to-be-deleted")
            .unwrap(),
        "命中删除返回 true"
    );
    assert!(
        store.list_agent_run_events(run.id).unwrap().is_empty(),
        "名下事件随记录级联删除"
    );
    assert!(
        !store
            .list_agent_runs()
            .unwrap()
            .iter()
            .any(|left| left.id == run.id),
        "名下 run 随记录级联删除"
    );

    // 复用 id 后链还原为空：错链不再发生
    let c = create_ok(&store, "C:\\ws\\alpha", "combine-agent-and-explore-chat");
    assert_eq!(c.id, b.id, "max+1 在幸存行上计算,id 仍会复用（级联后无害）");
    assert!(
        store
            .restore_run_chain("explore", &c.id.to_string())
            .unwrap()
            .is_empty(),
        "新记录（从未发过消息）链还原为空"
    );
}

/// 级联只圈 (source=explore, source_ref=本记录 id)：debug 来源同定位串、
/// 其他 source_ref 的 explore run 及其事件不波及。
#[test]
fn 级联删除不波及无关runs与事件() {
    let env = Env::new("explore-delete-cascade-scope");
    let store = open_workspace_ok(&env.db_path("ws"));
    let doomed = create_ok(&store, "C:\\ws\\alpha", "to-be-deleted");
    let keeper = create_ok(&store, "C:\\ws\\alpha", "keeper-topic");
    let bound = begin_provenance_run(
        &store,
        "被删链",
        100,
        "explore",
        Some(&doomed.id.to_string()),
        None,
    );
    // 干扰一：同 source_ref 不同 source（debug 来源同定位串）
    let debug_run = begin_provenance_run(
        &store,
        "调试 run",
        200,
        "debug",
        Some(&doomed.id.to_string()),
        None,
    );
    // 干扰二：同 source 不同 source_ref（另一 explore 记录名下）
    let other_explore = begin_provenance_run(
        &store,
        "隔壁链",
        300,
        "explore",
        Some(&keeper.id.to_string()),
        Some(bound.id),
    );
    for run in [&bound, &debug_run, &other_explore] {
        store
            .append_agent_run_events(run.id, &[stamped(0, run_started_kind())])
            .unwrap();
    }

    assert!(store
        .delete_explore_record("C:\\ws\\alpha", "to-be-deleted")
        .unwrap());

    assert!(
        store.list_agent_run_events(bound.id).unwrap().is_empty(),
        "被删记录名下事件清空"
    );
    for survivor in [debug_run.clone(), other_explore.clone()] {
        assert!(
            store
                .list_agent_runs()
                .unwrap()
                .iter()
                .any(|left| left.id == survivor.id),
            "无关 run 存活: {:?}",
            survivor.prompt
        );
        assert_eq!(
            store.list_agent_run_events(survivor.id).unwrap().len(),
            1,
            "无关 run 事件存活: {:?}",
            survivor.prompt
        );
    }
    // 隔壁链还原不受牵连（parent 指向已删 run 的尾段仍由本链自身锚定）
    let chain = store
        .restore_run_chain("explore", &keeper.id.to_string())
        .unwrap();
    assert_eq!(
        chain.len(),
        1,
        "隔壁链仍可还原（其 parent_run_id 指向已删 run 时截断回溯）"
    );
}

/// 构造带来源三元组的 running 形态 run 记录（id 由 begin 分配）。
fn provenance_run(
    prompt: &str,
    started_at: i64,
    source: &str,
    source_ref: Option<&str>,
    parent_run_id: Option<i64>,
) -> AgentRunRecord {
    AgentRunRecord {
        source: source.to_owned(),
        source_ref: source_ref.map(str::to_owned),
        parent_run_id,
        ..running_run(prompt, started_at)
    }
}

fn begin_provenance_run(
    store: &Store,
    prompt: &str,
    started_at: i64,
    source: &str,
    source_ref: Option<&str>,
    parent_run_id: Option<i64>,
) -> AgentRunRecord {
    store
        .begin_agent_run(&provenance_run(
            prompt,
            started_at,
            source,
            source_ref,
            parent_run_id,
        ))
        .expect("begin_agent_run 应成功")
}

#[test]
fn 同source_source_ref两条链式run按发起序还原且链头在末位() {
    let env = Env::new("chain-two");
    let store = open_workspace_ok(&env.db_path("ws"));
    let first = begin_provenance_run(&store, "首轮", 100, "explore", Some("7"), None);
    let second = begin_provenance_run(&store, "续轮", 200, "explore", Some("7"), Some(first.id));

    let chain = store.restore_run_chain("explore", "7").unwrap();

    let ids: Vec<i64> = chain.iter().map(|record| record.id).collect();
    assert_eq!(
        ids,
        vec![first.id, second.id],
        "按发起顺序还原，链头（最新）在末位"
    );
    assert_eq!(chain[0].parent_run_id, None, "链首无上游指针");
    assert_eq!(chain[1].parent_run_id, Some(first.id), "链尾指向前一轮");
}

#[test]
fn 未命中source_source_ref返回空vec() {
    let env = Env::new("chain-miss");
    let store = open_workspace_ok(&env.db_path("ws"));
    begin_provenance_run(&store, "首轮", 100, "explore", Some("7"), None);

    assert!(
        store
            .restore_run_chain("explore", "404")
            .unwrap()
            .is_empty(),
        "无链返回空 Vec（起链语义）"
    );
}

#[test]
fn 混入干扰记录均不入链() {
    let env = Env::new("chain-decoy");
    let store = open_workspace_ok(&env.db_path("ws"));
    let first = begin_provenance_run(&store, "首轮", 100, "explore", Some("7"), None);
    let second = begin_provenance_run(&store, "续轮", 200, "explore", Some("7"), Some(first.id));
    // 干扰一：同 source_ref 不同 source（调试来源同定位串）
    let _decoy_source = begin_provenance_run(&store, "调试 run", 300, "debug", Some("7"), None);
    // 干扰二：同 source 不同 source_ref（另一 explore 记录），且 parent 指入本链
    let _decoy_ref =
        begin_provenance_run(&store, "隔壁链", 400, "explore", Some("8"), Some(second.id));

    let chain = store.restore_run_chain("explore", "7").unwrap();

    let ids: Vec<i64> = chain.iter().map(|record| record.id).collect();
    assert_eq!(
        ids,
        vec![first.id, second.id],
        "(source, source_ref) 双键过滤，干扰记录不入链"
    );
}

#[test]
fn parent_run_id成环时防环截断不悬挂且成员完整() {
    let env = Env::new("chain-cycle");
    let store = open_workspace_ok(&env.db_path("ws"));
    let a = begin_provenance_run(&store, "A", 100, "explore", Some("7"), None);
    let b = begin_provenance_run(&store, "B", 200, "explore", Some("7"), Some(a.id));
    // 构造指针环 A→B→A：终态替换把 A 的上游改指 B
    let mut cyclic = a.clone();
    cyclic.parent_run_id = Some(b.id);
    store.finish_agent_run(a.id, &cyclic).expect("构造环应成功");

    let chain = store.restore_run_chain("explore", "7").expect("环不得悬挂");

    let mut ids: Vec<i64> = chain.iter().map(|record| record.id).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![a.id, b.id].into_iter().collect::<Vec<i64>>(),
        "visited 集截断：成员完整不重复"
    );
}

#[test]
fn 分叉再汇聚还原为单链链头唯一取最新不重复不遗漏() {
    let env = Env::new("chain-fork");
    let store = open_workspace_ok(&env.db_path("ws"));
    let root = begin_provenance_run(&store, "链首", 100, "explore", Some("9"), None);
    let late = begin_provenance_run(&store, "分叉晚", 300, "explore", Some("9"), Some(root.id));
    let _early = begin_provenance_run(&store, "分叉早", 200, "explore", Some("9"), Some(root.id));

    let chain = store.restore_run_chain("explore", "9").unwrap();

    let ids: Vec<i64> = chain.iter().map(|record| record.id).collect();
    assert_eq!(
        ids,
        vec![root.id, late.id],
        "链头唯一取 (started_at, id) 最新：还原为单链（root → late），early 分叉不入列"
    );
}

// ---------------------------------------------------------------------------
// 来源三元组演进：三字段往返与两代写入语义并存
// ---------------------------------------------------------------------------

#[test]
fn v2写入三字段非缺省重开db读回往返保真() {
    let env = Env::new("explore-v2-roundtrip");
    let ws_path = env.db_path("ws");

    let seeded = {
        let store = open_workspace_ok(&ws_path);
        let parent = begin_provenance_run(&store, "上一轮", 100, "explore", Some("7"), None);
        let mut requested = provenance_run(
            "explore 续轮",
            1727000000000,
            "explore",
            Some("7"),
            Some(parent.id),
        );
        requested.session_id = Some("s-tail".to_owned());
        let mut record = store.begin_agent_run(&requested).expect("begin 应成功");
        record.status = AgentRunStatus::Completed;
        record.finished_at = Some(1727000001000);
        store
            .finish_agent_run(record.id, &record)
            .expect("finish 应成功");
        record
    };
    assert_eq!(seeded.source, "explore", "来源非缺省");
    assert_eq!(seeded.source_ref.as_deref(), Some("7"), "定位非缺省");
    assert!(seeded.parent_run_id.is_some(), "链指针非缺省");

    // 重开同一 db 文件：v2 三字段往返保真
    let reopened = open_workspace_ok(&ws_path);
    let listed = reopened.list_agent_runs().unwrap();
    let tail = listed
        .iter()
        .find(|record| record.id == seeded.id)
        .expect("链尾应可读");
    assert_eq!(tail.source, "explore");
    assert_eq!(tail.source_ref.as_deref(), Some("7"));
    assert_eq!(tail.parent_run_id, seeded.parent_run_id, "链指针往返保真");
    assert_eq!(
        tail.session_id.as_deref(),
        Some("s-tail"),
        "其余字段一并保真"
    );
}

#[test]
fn 缺省来源与显式来源记录并存全量可读且按started_at统一排序() {
    let env = Env::new("explore-mixed");
    let store = open_workspace_ok(&env.db_path("ws"));
    // v1 时代写入语义（source 缺省 debug、两字段 None）与 v2 显式三元组并存
    let legacy = begin_provenance_run(&store, "调试旧轮", 100, "debug", None, None);
    let explore_run = begin_provenance_run(
        &store,
        "explore 轮",
        300,
        "explore",
        Some("5"),
        Some(legacy.id),
    );
    let middle = begin_provenance_run(&store, "调试新轮", 200, "debug", None, None);

    let listed = store.list_agent_runs().unwrap();

    let ids: Vec<i64> = listed.iter().map(|record| record.id).collect();
    assert_eq!(ids.len(), 3, "两代写入语义的记录全量可读");
    assert_eq!(
        ids,
        vec![explore_run.id, middle.id, legacy.id],
        "按 (started_at, id) 统一排序"
    );
    assert_eq!(legacy.source, "debug");
    assert_eq!(explore_run.source, "explore");
    assert_eq!(explore_run.parent_run_id, Some(legacy.id));
}

#[test]
fn v1与v2语义run的事件记录照常经append与list重放() {
    // 事件表全局 run_id 锚定，不受 run 模型演进影响（重放前提）
    let env = Env::new("explore-events");
    let ws_path = env.db_path("ws");
    let events: Vec<AgentEvent> = (0..3u64)
        .map(|seq| stamped(seq, raw_kind(&seq.to_string())))
        .collect();

    let run_id = {
        let store = open_workspace_ok(&ws_path);
        let run = begin_provenance_run(&store, "explore 带事件", 100, "explore", Some("5"), None);
        store
            .append_agent_run_events(run.id, &events)
            .expect("append 应成功");
        run.id
    };

    let reopened = open_workspace_ok(&ws_path);
    assert_eq!(
        reopened.list_agent_run_events(run_id).unwrap(),
        events,
        "显式来源 run 的事件重放逐字段保真"
    );
}

// ---------------------------------------------------------------------------
// agent 管理操作面（user 维度，全局库）：provider upsert / remove / list /
// find。tempdir 真开全局库（存储层不 mock 惯例），全部断言经公共 API。
// ---------------------------------------------------------------------------

/// 三档模型 fixture（三档可区分值，供逐字段与消费半边断言）。
fn fixture_tiers() -> AgentModelTiers {
    AgentModelTiers {
        high: "m-high".to_owned(),
        medium: "m-medium".to_owned(),
        low: "m-low".to_owned(),
    }
}

fn fixture_provider(name: &str) -> AgentProviderRecord {
    AgentProviderRecord::new(
        name.to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        fixture_tiers(),
    )
}

fn upsert_provider_ok(store: &Store, provider: AgentProviderRecord) -> AgentProviderRecord {
    store
        .upsert_agent_provider(provider)
        .unwrap_or_else(|e| panic!("upsert_agent_provider 应成功: {e}"))
}

fn fixture_agent(name: &str, engine: AgentEngineKind, provider_id: Option<i64>) -> AgentInstanceRecord {
    AgentInstanceRecord::new(name.to_owned(), engine, provider_id)
}

fn upsert_agent_ok(store: &Store, agent: AgentInstanceRecord) -> AgentInstanceRecord {
    store
        .upsert_agent_instance(agent)
        .unwrap_or_else(|e| panic!("upsert_agent_instance 应成功: {e}"))
}

#[test]
fn provider_upsert新建_max加1分配id从1起_返回落库记录且清单可读() {
    let env = Env::new("mgmt-provider-new");
    let store = open_global_ok(&env.db_path("global"));

    let saved = upsert_provider_ok(&store, fixture_provider("端点甲"));

    assert_eq!(saved.id, 1, "空库新建 max+1 分配 id=1");
    assert_eq!(saved.name, "端点甲");
    assert_eq!(saved.api_key, "sk-live-1234567890", "返回落库记录含全字段");
    assert_eq!(
        store.list_agent_providers().unwrap(),
        vec![saved.clone()],
        "list_agent_providers 可读同记录"
    );
    // 直查命中同记录（save 回填原值的消费半边）
    assert_eq!(store.find_agent_provider(saved.id).unwrap(), Some(saved));
}

#[test]
fn provider_upsert同名重复新建err_清单不产生第二条() {
    let env = Env::new("mgmt-provider-dup");
    let store = open_global_ok(&env.db_path("global"));
    let first = upsert_provider_ok(&store, fixture_provider("端点甲"));

    let result = store.upsert_agent_provider(fixture_provider("端点甲"));

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("同 name 重复新建应 Err（单写事务内查重）"),
    };
    assert!(
        err.to_string().contains("端点甲"),
        "错误串含 name 语境，实际: {err}"
    );
    assert_eq!(
        store.list_agent_providers().unwrap(),
        vec![first],
        "失败无副作用：清单不产生第二条"
    );
}

#[test]
fn provider_upsert_name空白err_api_key空串允许落库() {
    let env = Env::new("mgmt-provider-blank");
    let store = open_global_ok(&env.db_path("global"));

    for name in ["", "   ", "\n\t "] {
        let result = store.upsert_agent_provider(fixture_provider(name));
        assert!(
            matches!(&result, Err(StoreError::Db(_))),
            "name 空白 {name:?} 应 Err（空白校验拒绝），实际: {result:?}"
        );
    }
    assert!(
        store.list_agent_providers().unwrap().is_empty(),
        "全部拒绝：不产生任何记录"
    );

    // api_key 空串允许落库（新建语义无原值可保）
    let blank_key = AgentProviderRecord::new(
        "空key端点".to_owned(),
        "https://api.example.com/v1".to_owned(),
        String::new(),
        fixture_tiers(),
    );
    let saved = upsert_provider_ok(&store, blank_key);
    assert_eq!(saved.api_key, "", "api_key 空串原样落库");
}

#[test]
fn provider_upsert更新整行替换_id不变且查重排除自身() {
    let env = Env::new("mgmt-provider-update");
    let store = open_global_ok(&env.db_path("global"));
    let first = upsert_provider_ok(&store, fixture_provider("端点甲"));

    // 保留原名更新其他字段：查重排除自身，不误报重名
    let mut updated = fixture_provider("端点甲");
    updated.id = first.id;
    updated.base_url = "https://changed.example.com/v1".to_owned();
    updated.api_key = "sk-new-key-999".to_owned();
    updated.models = AgentModelTiers {
        high: "new-high".to_owned(),
        medium: "new-medium".to_owned(),
        low: "new-low".to_owned(),
    };
    let saved = upsert_provider_ok(&store, updated);

    assert_eq!(saved.id, first.id, "更新臂 id 不变");
    assert_eq!(saved.base_url, "https://changed.example.com/v1", "整行替换生效");
    assert_eq!(saved.api_key, "sk-new-key-999");
    assert_eq!(saved.models.high, "new-high");
    assert_eq!(store.list_agent_providers().unwrap(), vec![saved]);
}

#[test]
fn provider_upsert更新为他人已占name_err_两条记录均原样() {
    let env = Env::new("mgmt-provider-conflict");
    let store = open_global_ok(&env.db_path("global"));
    let alpha = upsert_provider_ok(&store, fixture_provider("甲"));
    let beta = upsert_provider_ok(&store, fixture_provider("乙"));

    // 甲更新为乙的 name：Err 且两条记录均原样（不产生半更新）
    let mut renamed = fixture_provider("乙");
    renamed.id = alpha.id;
    let result = store.upsert_agent_provider(renamed);

    assert!(result.is_err(), "更新为他人已占 name 应 Err，实际: {result:?}");
    let listed = store.list_agent_providers().unwrap();
    assert_eq!(listed, vec![alpha, beta], "两条记录均原样");
}

#[test]
fn provider_id分配_连续新建严格递增_删除最大id后新建复用槽位() {
    let env = Env::new("mgmt-provider-ids");
    let store = open_global_ok(&env.db_path("global"));
    let first = upsert_provider_ok(&store, fixture_provider("甲"));
    let second = upsert_provider_ok(&store, fixture_provider("乙"));
    let third = upsert_provider_ok(&store, fixture_provider("丙"));
    assert_eq!(
        (first.id, second.id, third.id),
        (1, 2, 3),
        "连续新建 max+1 严格递增"
    );

    // 删除最大 id 记录后新建复用该槽位（max+1 口径、无永久计数器）
    assert!(store.remove_agent_provider(third.id).unwrap());
    let reused = upsert_provider_ok(&store, fixture_provider("丁"));
    assert_eq!(reused.id, third.id, "删除表尾后新建复用 max+1 槽位");
}

#[test]
fn remove_agent_provider被agent引用时err含引用方name_两类记录均原样保留() {
    let env = Env::new("mgmt-provider-refused");
    let store = open_global_ok(&env.db_path("global"));
    let provider = upsert_provider_ok(&store, fixture_provider("被引用端点"));
    let agent = upsert_agent_ok(
        &store,
        fixture_agent("引用方agent", AgentEngineKind::Sdk, Some(provider.id)),
    );

    let result = store.remove_agent_provider(provider.id);

    let err = result.expect_err("被 agent 引用时应 Err（引用完整性）");
    assert!(
        err.to_string().contains("引用方agent"),
        "错误串含引用方 name 提示，实际: {err}"
    );
    // 不级联不静默删除：两类记录均原样保留
    assert_eq!(
        store.list_agent_providers().unwrap(),
        vec![provider],
        "provider 原样保留"
    );
    assert_eq!(
        store.list_agent_instances().unwrap(),
        vec![agent],
        "agent 原样保留"
    );
}

#[test]
fn remove_agent_provider未被引用命中删除ok_true且清单不再含该项() {
    let env = Env::new("mgmt-provider-remove");
    let store = open_global_ok(&env.db_path("global"));
    let provider = upsert_provider_ok(&store, fixture_provider("可删端点"));

    let hit = store.remove_agent_provider(provider.id).unwrap();

    assert!(hit, "命中删除返回 true");
    assert!(
        store.list_agent_providers().unwrap().is_empty(),
        "清单不再含该项"
    );
}

#[test]
fn remove_agent_provider_miss幂等ok_false且库内容不变() {
    let env = Env::new("mgmt-provider-remove-miss");
    let store = open_global_ok(&env.db_path("global"));
    let provider = upsert_provider_ok(&store, fixture_provider("在库端点"));

    let hit = store.remove_agent_provider(999).unwrap();

    assert!(!hit, "miss 幂等返回 false");
    assert_eq!(
        store.list_agent_providers().unwrap(),
        vec![provider],
        "库内容不变"
    );
}

#[test]
fn provider清单主键id升序与添加顺序无关_两次调用序稳定确定() {
    let env = Env::new("mgmt-provider-order");
    let store = open_global_ok(&env.db_path("global"));
    // 刻意乱序添加
    let gamma = upsert_provider_ok(&store, fixture_provider("丙"));
    let alpha = upsert_provider_ok(&store, fixture_provider("甲"));
    let beta = upsert_provider_ok(&store, fixture_provider("乙"));

    let first = store.list_agent_providers().unwrap();
    let second = store.list_agent_providers().unwrap();

    let ids: Vec<i64> = first.iter().map(|record| record.id).collect();
    // 丙先插入（id=1）、甲次之（id=2）、乙最后（id=3）：清单按主键 id 升序，
    // 与添加顺序无关
    assert_eq!(
        ids,
        vec![gamma.id, alpha.id, beta.id],
        "主键 id 升序自然序，与添加顺序无关"
    );
    assert_eq!(first, second, "两次调用序稳定确定");
}

#[test]
fn provider空库list空向量_find对不存在与0与负数与i64max均none() {
    let env = Env::new("mgmt-provider-find-edge");
    let store = open_global_ok(&env.db_path("global"));

    assert!(
        store.list_agent_providers().unwrap().is_empty(),
        "空库 list 返回空向量"
    );
    for id in [404, 0, -1, i64::MAX] {
        assert_eq!(
            store.find_agent_provider(id).unwrap(),
            None,
            "find_agent_provider({id}) 不存在返回 None 不报错"
        );
    }
}

// ---------------------------------------------------------------------------
// agent 实例操作面：engine 约束、name 查重、默认标记唯一写口、删除清标记、
// set_default 标记即切换
// ---------------------------------------------------------------------------

#[test]
fn agent_upsert_cli引擎provider可空_sdk选存量provider保存成功() {
    let env = Env::new("mgmt-agent-engine");
    let store = open_global_ok(&env.db_path("global"));

    // cli 引擎 provider 可空（provider_id=None 保存成功）
    let cli = upsert_agent_ok(&store, fixture_agent("cli实例", AgentEngineKind::Cli, None));
    assert_eq!(cli.provider_id, None);
    assert_eq!(cli.id, 1);

    // sdk 引擎选存量 provider 保存成功
    let provider = upsert_provider_ok(&store, fixture_provider("端点"));
    let sdk = upsert_agent_ok(
        &store,
        fixture_agent("sdk实例", AgentEngineKind::Sdk, Some(provider.id)),
    );
    assert_eq!(sdk.provider_id, Some(provider.id));
    assert_eq!(
        store.list_agent_instances().unwrap(),
        vec![cli, sdk],
        "两引擎实例并存"
    );
}

#[test]
fn agent_upsert_sdk缺provider_err_悬空引用err() {
    let env = Env::new("mgmt-agent-sdk-require");
    let store = open_global_ok(&env.db_path("global"));
    let provider = upsert_provider_ok(&store, fixture_provider("端点"));

    // sdk 引擎 provider_id=None Err
    let missing = store.upsert_agent_instance(fixture_agent("sdk缺provider", AgentEngineKind::Sdk, None));
    let err = missing.expect_err("sdk 引擎 provider_id=None 应 Err");
    assert!(
        err.to_string().contains("sdk缺provider"),
        "错误串含 name 语境，实际: {err}"
    );

    // sdk 悬空引用（provider_id 指向不存在的 provider id）Err
    let dangling = store.upsert_agent_instance(fixture_agent("sdk悬空", AgentEngineKind::Sdk, Some(999)));
    let err = dangling.expect_err("sdk 悬空引用应 Err");
    assert!(
        err.to_string().contains("999"),
        "错误串含 provider id 语境，实际: {err}"
    );

    assert!(
        store.list_agent_instances().unwrap().is_empty(),
        "失败无副作用：不产生任何 agent 记录"
    );
    assert_eq!(
        store.list_agent_providers().unwrap(),
        vec![provider],
        "悬空引用拒绝不影响 provider 存量"
    );
}

#[test]
fn agent_upsert_name查重_同name重复新建与更新撞名err_name空白err() {
    let env = Env::new("mgmt-agent-dup");
    let store = open_global_ok(&env.db_path("global"));
    let first = upsert_agent_ok(&store, fixture_agent("同名实例", AgentEngineKind::Cli, None));

    // 同 name 重复新建 Err
    let dup_new = store.upsert_agent_instance(fixture_agent("同名实例", AgentEngineKind::Sdk, None));
    assert!(dup_new.is_err(), "同 name 重复新建应 Err");

    // 更新撞名 Err
    let second = upsert_agent_ok(&store, fixture_agent("另一实例", AgentEngineKind::Cli, None));
    let mut renamed = fixture_agent("同名实例", AgentEngineKind::Cli, None);
    renamed.id = second.id;
    let dup_update = store.upsert_agent_instance(renamed);
    assert!(dup_update.is_err(), "更新撞名应 Err");

    // name 空白 Err
    for name in ["", "  "] {
        let blank = store.upsert_agent_instance(fixture_agent(name, AgentEngineKind::Cli, None));
        assert!(matches!(&blank, Err(StoreError::Db(_))), "name 空白应 Err，实际: {blank:?}");
    }

    assert_eq!(
        store.list_agent_instances().unwrap(),
        vec![first, second],
        "失败无副作用：两条记录原样"
    );
}

#[test]
fn agent_upsert默认标记写口_新建臂入参true被强制false_更新臂保留存量标记() {
    let env = Env::new("mgmt-agent-default-write");
    let store = open_global_ok(&env.db_path("global"));

    // 新建臂：入参 is_default=true 被强制落 false（入参标记不参与写）
    let mut requested = fixture_agent("新建臂", AgentEngineKind::Cli, None);
    requested.is_default = true;
    let created = upsert_agent_ok(&store, requested);
    assert!(
        !created.is_default,
        "新建臂入参 is_default=true 强制落 false（默认标记唯一写口为 set_default）"
    );
    assert!(
        store.default_agent_instance().unwrap().is_none(),
        "新建后全局无默认"
    );

    // 更新臂：保留存量标记（默认 agent 改名 / 换 provider 后仍为默认）
    assert!(store.set_default_agent_instance(created.id).is_ok());
    let mut renamed = fixture_agent("改名后", AgentEngineKind::Cli, None);
    renamed.id = created.id;
    renamed.is_default = false; // 入参标记 false 同样不参与写
    let updated = upsert_agent_ok(&store, renamed);
    assert!(
        updated.is_default,
        "更新臂保留存量默认标记（入参标记不参与写）"
    );
    assert_eq!(
        store.default_agent_instance().unwrap().map(|record| record.id),
        Some(created.id),
        "默认 agent 改名后仍为默认"
    );
}

#[test]
fn remove_agent_instance删默认agent_ok后default解析none且清单少一行() {
    let env = Env::new("mgmt-agent-remove-default");
    let store = open_global_ok(&env.db_path("global"));
    let provider = upsert_provider_ok(&store, fixture_provider("端点"));
    let agent = upsert_agent_ok(
        &store,
        fixture_agent("默认实例", AgentEngineKind::Sdk, Some(provider.id)),
    );
    store.set_default_agent_instance(agent.id).unwrap();
    let _survivor = upsert_agent_ok(&store, fixture_agent("幸存实例", AgentEngineKind::Cli, None));

    let hit = store.remove_agent_instance(agent.id).unwrap();

    assert!(hit, "命中删除返回 true");
    assert_eq!(
        store.default_agent_instance().unwrap(),
        None,
        "删默认 agent 后 default_agent_instance 返回 None（同事务清标记，无顺延）"
    );
    assert_eq!(store.list_agent_instances().unwrap().len(), 1, "清单少一行");
}

#[test]
fn remove_agent_instance_miss幂等okfalse_删非默认不影响既有默认() {
    let env = Env::new("mgmt-agent-remove-miss");
    let store = open_global_ok(&env.db_path("global"));
    let default_agent = upsert_agent_ok(&store, fixture_agent("默认", AgentEngineKind::Cli, None));
    store.set_default_agent_instance(default_agent.id).unwrap();
    let other = upsert_agent_ok(&store, fixture_agent("非默认", AgentEngineKind::Cli, None));

    assert!(
        !store.remove_agent_instance(999).unwrap(),
        "miss 幂等返回 false"
    );

    // 删非默认 agent：既有默认不受影响
    assert!(store.remove_agent_instance(other.id).unwrap());
    let remaining_default = store
        .default_agent_instance()
        .unwrap()
        .expect("默认 agent 不受删非默认影响");
    assert_eq!(remaining_default.id, default_agent.id);
}

#[test]
fn default_agent_instance有默认some无默认none() {
    let env = Env::new("mgmt-agent-default-resolve");
    let store = open_global_ok(&env.db_path("global"));

    assert_eq!(
        store.default_agent_instance().unwrap(),
        None,
        "无默认返回 None（清单扫 is_default，恒零或一）"
    );

    let agent = upsert_agent_ok(&store, fixture_agent("默认实例", AgentEngineKind::Cli, None));
    store.set_default_agent_instance(agent.id).unwrap();
    let resolved = store.default_agent_instance().unwrap().expect("有默认返回 Some");
    assert_eq!(resolved.id, agent.id);
    assert!(resolved.is_default);
}

#[test]
fn set_default标记即切换_甲到乙全局恰一默认为乙_返回更新后记录() {
    let env = Env::new("mgmt-agent-default-switch");
    let store = open_global_ok(&env.db_path("global"));
    let agent_a = upsert_agent_ok(&store, fixture_agent("甲", AgentEngineKind::Cli, None));
    let agent_b = upsert_agent_ok(&store, fixture_agent("乙", AgentEngineKind::Cli, None));
    store.set_default_agent_instance(agent_a.id).unwrap();

    let switched = store.set_default_agent_instance(agent_b.id).unwrap();

    // 返回更新后记录（is_default=true）
    assert_eq!(switched.id, agent_b.id);
    assert!(switched.is_default, "返回更新后记录");
    // 甲→乙切换后全局恰一默认为乙、甲标记自动清除
    let listed = store.list_agent_instances().unwrap();
    let defaults: Vec<i64> = listed
        .iter()
        .filter(|record| record.is_default)
        .map(|record| record.id)
        .collect();
    assert_eq!(defaults, vec![agent_b.id], "全局恰一默认为乙，甲标记自动清除");
    assert_eq!(
        store.default_agent_instance().unwrap().map(|record| record.id),
        Some(agent_b.id)
    );
}

#[test]
fn set_default重复标记同一agent幂等_切换后全清单is_default恰一true() {
    let env = Env::new("mgmt-agent-default-idempotent");
    let store = open_global_ok(&env.db_path("global"));
    let agent = upsert_agent_ok(&store, fixture_agent("甲", AgentEngineKind::Cli, None));
    let other = upsert_agent_ok(&store, fixture_agent("乙", AgentEngineKind::Cli, None));
    store.set_default_agent_instance(agent.id).unwrap();

    let again = store.set_default_agent_instance(agent.id).unwrap();

    assert!(again.is_default, "重复标记同一 agent 幂等（仍恰一默认）");
    let default_count = store
        .list_agent_instances()
        .unwrap()
        .iter()
        .filter(|record| record.is_default)
        .count();
    assert_eq!(default_count, 1, "切换后全清单 is_default 恰一为 true");
    assert_eq!(
        store.default_agent_instance().unwrap().map(|record| record.id),
        Some(agent.id),
        "默认未漂移到 {other:?}"
    );
}

#[test]
fn set_default_miss_id_err且既有默认标记不变() {
    let env = Env::new("mgmt-agent-default-miss");
    let store = open_global_ok(&env.db_path("global"));
    let agent = upsert_agent_ok(&store, fixture_agent("甲", AgentEngineKind::Cli, None));
    store.set_default_agent_instance(agent.id).unwrap();

    let result = store.set_default_agent_instance(999);

    let err = result.expect_err("miss id 应 Err");
    assert!(
        err.to_string().contains("999"),
        "错误串含 id 语境，实际: {err}"
    );
    assert_eq!(
        store.default_agent_instance().unwrap().map(|record| record.id),
        Some(agent.id),
        "既有默认标记不变"
    );
}

// ---------------------------------------------------------------------------
// 存量库 additive 打开（AC-10）：native_db 裸构造仅 WorkspaceRecord 的存量
// 全局库写盘（fixture 构造，非 mock）→ 新代码 open_global 成功读写。
// ---------------------------------------------------------------------------

/// 预置仅注册 WorkspaceRecord 并写入一条记录的存量全局库（native_db 裸
/// 构造，不经 Store）：模拟 agent 管理两模型登记前的存量 desktop-global.redb。
fn preset_global_with_workspace_only(path: &Path, root_key: &str) {
    let mut models = Models::new();
    models
        .define::<WorkspaceRecord>()
        .expect("定义 WorkspaceRecord 失败");
    let db = Builder::new()
        .create(&models, path)
        .expect("预置存量全局库失败");
    let rw = db.rw_transaction().expect("开启写事务失败");
    rw.insert(WorkspaceRecord::from_root(root_key, 1000))
        .expect("写入 workspace 记录失败");
    rw.commit().expect("提交预置事务失败");
}

#[test]
fn 存量库仅workspace注册时新代码additive打开成功_原记录可读_两新模型可写读() {
    let env = Env::new("mgmt-additive-open");
    let global_path = env.db_path("global");
    let legacy_root = "C:\\ws\\legacy-demo";
    preset_global_with_workspace_only(&global_path, legacy_root);

    // 新代码 open_global 成功（additive 追加模型，无任何迁移代码路径）
    let store = open_global_ok(&global_path);

    // 原记录原样可读
    let workspaces = store.list_workspaces().unwrap();
    assert_eq!(workspaces.len(), 1, "存量 workspace 记录原样可读");
    assert_eq!(workspaces[0].root, legacy_root);

    // 两新模型可写入读出
    let provider = upsert_provider_ok(&store, fixture_provider("additive 端点"));
    let agent = upsert_agent_ok(
        &store,
        fixture_agent("additive 实例", AgentEngineKind::Sdk, Some(provider.id)),
    );
    assert_eq!(
        store.list_agent_providers().unwrap(),
        vec![provider.clone()],
        "provider 写入读出"
    );
    assert_eq!(
        store.list_agent_instances().unwrap(),
        vec![agent],
        "agent 写入读出"
    );
    drop(store);

    // 重开同文件：三模型状态一致（additive 打开可复现）
    let reopened = open_global_ok(&global_path);
    assert_eq!(reopened.list_workspaces().unwrap().len(), 1);
    assert_eq!(reopened.list_agent_providers().unwrap().len(), 1);
    assert_eq!(reopened.list_agent_instances().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// 全局库单库贯通（组合）：三模型同库共存互不干扰，drop 重开状态一致
// ---------------------------------------------------------------------------

#[test]
fn 组合链provider新建_sdkagent新建引用_set_default_解析命中_重开同一库文件状态一致() {
    let env = Env::new("mgmt-combined-chain");
    let global_path = env.db_path("global");

    let snapshot = {
        let store = open_global_ok(&global_path);
        // 组合链：provider 新建 → sdk agent 新建（引用）→ set_default → 解析命中
        let provider = upsert_provider_ok(&store, fixture_provider("贯通端点"));
        let agent = upsert_agent_ok(
            &store,
            fixture_agent("贯通实例", AgentEngineKind::Sdk, Some(provider.id)),
        );
        store.set_default_agent_instance(agent.id).unwrap();
        let resolved = store.default_agent_instance().unwrap().expect("解析命中");
        assert_eq!(resolved.id, agent.id);
        assert!(resolved.is_default);

        // 三模型同库共存：注册表与管理数据互不干扰（workspace 维度模型不在本库）
        let models = store.list_models().unwrap();
        assert_eq!(
            models
                .iter()
                .map(|model| (model.name.as_str(), model.count))
                .collect::<Vec<_>>(),
            vec![("workspace", 0), ("agent_provider", 1), ("agent_instance", 1)],
            "全局库单库贯通：三模型行共存，计数与实有记录一致"
        );
        (
            provider,
            store.list_agent_instances().unwrap(),
            store.default_agent_instance().unwrap(),
        )
    };

    // drop 重开同一库文件后全部状态一致
    let reopened = open_global_ok(&global_path);
    assert_eq!(
        reopened.list_agent_providers().unwrap(),
        vec![snapshot.0],
        "provider 状态跨重开一致"
    );
    assert_eq!(
        reopened.list_agent_instances().unwrap(),
        snapshot.1,
        "agent 清单跨重开一致"
    );
    assert_eq!(
        reopened.default_agent_instance().unwrap(),
        snapshot.2,
        "默认标记跨重开一致"
    );
}
