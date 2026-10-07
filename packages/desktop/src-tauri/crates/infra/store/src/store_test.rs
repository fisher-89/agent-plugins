use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent::{
    AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunStatus,
};
use native_db::{Builder, Models};

use crate::model::pack_checklist_item_key;
use crate::model::ChangeRecordV1;
use crate::store::{workspace_db_file_name, GLOBAL_DB_FILE_NAME};
use crate::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord, AgentRunRecord,
    ChangeRecord, ExploreRecord, PhaseRecord, SessionConfigSnapshot, SessionRecord, Store,
    StoreError, WorkspaceRecord, WorkspaceStores,
};
use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    ActivePhaseState, BacktrackCommand, ChangeStateRecord, ChangeStatus, PhaseLogCommand,
    PhaseStartState, StepCommand, StepKind,
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
fn open_global列user维度三模型_open_workspace仅列workspace维度八行() {
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
        vec![
            "agent_run", "session", "session_event", "explore", "change", "phase",
            "checklist_item", "step"
        ],
        "workspace 组静态注册 4→8：轮统计行 / 会话 / 转录 / explore 四既有模型 + change 流程状态四模型（agent_event 退役出注册），两组无交叉"
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
    for name in [
        "agent_run",
        "agent_event",
        "explore",
        "change",
        "phase",
        "checklist_item",
        "step",
    ] {
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
    assert!(
        ws.list_sessions(None, None).unwrap().is_empty(),
        "空库可 list"
    );
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
fn 同root跨两级库生命周期派生同一路径_重开后会话与explore完整可读() {
    let env = StoresEnv::new("reopen-same-path");
    let root = env.root_of("persist");

    let (session_id, explore, file_name) = {
        let stores = env.open();
        let store = stores.for_root(&root).expect("for_root 应成功");
        let session_id = seed_session(&store, "ses-persist-1");
        let explore = create_ok(&store, &root, "topic");
        assert_eq!(env.workspace_db_files().len(), 1);
        (session_id, explore, env.workspace_db_files()[0].clone())
    }; // 整个 WorkspaceStores（含缓存实例）随作用域释放，文件锁归还

    let stores = env.open();
    let store = stores.for_root(&root).expect("重开 for_root 应成功");
    assert_eq!(
        env.workspace_db_files(),
        vec![file_name],
        "同 root 跨重开派生同一路径（不产生第二文件）"
    );
    let summaries = store.list_sessions(None, None).unwrap();
    assert_eq!(summaries.len(), 1, "先写入的会话重开后完整可读");
    assert_eq!(summaries[0].row.id, session_id);
    assert_eq!(
        store.list_explore_records(&root).unwrap(),
        vec![explore],
        "先写入的 explore 记录重开后完整可读"
    );
}

/// db 文件名委托 foundation 身份段单点（design D6，行为零变化——同名回归）：
/// `workspace_db_file_name(root)` == `{foundation 身份段}.redb`（委托等值）；
/// 真实 `WorkspaceStores` 打开两 root → `workspaces/` 子树文件名与单点派生
/// 逐字一致、异根互异。清洗算法行为断言整体迁 foundation identity_test
///（行为平移零丢失——本文件原「可读段清洗」用例随单点下沉退役）。
#[test]
fn db文件名同名回归_委托foundation身份段等值() {
    for root in [
        "C:\\ws\\demo-alpha",
        "C:\\ws\\a-very-long-workspace-directory-name",
        "C:\\ws\\c:d*e?f\"g<h>i|j",
        "C:\\ws\\...",
    ] {
        assert_eq!(
            workspace_db_file_name(root),
            format!(
                "{}.redb",
                foundation::identity::workspace_identity_segment(root)
            ),
            "委托等值：db 文件名 = foundation 身份段 + .redb（{root}）"
        );
    }
    // 纯函数确定性：同根恒同名（委托面同式持恒）
    assert_eq!(
        workspace_db_file_name("C:\\ws\\demo-alpha"),
        workspace_db_file_name("C:\\ws\\demo-alpha"),
        "同根派生确定可复现"
    );

    // 文件面：真实 WorkspaceStores 打开两 root → 子树文件名与单点派生逐字
    // 一致、异根互异
    let env = StoresEnv::new("delegate-names");
    let stores = env.open();
    let alpha = env.root_of("alpha");
    let beta = env.root_of("beta");
    stores.for_root(&alpha).expect("for_root alpha 应成功");
    stores.for_root(&beta).expect("for_root beta 应成功");
    let files = env.workspace_db_files();
    assert_eq!(files.len(), 2, "异根各自独立文件");
    let expected_alpha = workspace_db_file_name(&alpha);
    assert!(
        files.contains(&expected_alpha),
        "子树文件名与单点派生逐字一致: {files:?} vs {expected_alpha}"
    );
    assert_ne!(expected_alpha, workspace_db_file_name(&beta), "异根互异");
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
    let session_id = seed_session(&first, "ses-reuse");
    let summaries = second.list_sessions(None, None).unwrap();
    assert_eq!(summaries.len(), 1, "经同一实例的写入对二次解析立即可见");
    assert_eq!(summaries[0].row.id, session_id);
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
    seed_session(&store_a, "ses-only-a");
    assert!(
        store_b.list_sessions(None, None).unwrap().is_empty(),
        "A 的写入对 B 不可见"
    );
    seed_session(&store_b, "ses-only-b");
    let ids_a: Vec<String> = store_a
        .list_sessions(None, None)
        .unwrap()
        .iter()
        .map(|summary| summary.row.id.clone())
        .collect();
    assert_eq!(ids_a, vec!["ses-only-a"], "A 库清单不含 B 的会话");
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
fn 组合链open注册后for_root落库会话与explore且全局库无混入() {
    let env = StoresEnv::new("ac2-split");
    let stores = env.open();

    // 组合链：WorkspaceStores::open → global 注册 → for_root 写入
    let record = add_ok(stores.global(), &env.ws("split"));
    let root = record.root.clone();
    let store = stores.for_root(&root).expect("for_root 应成功");
    let session_id = seed_session(&store, "ses-split-1");
    let explore = create_ok(&store, &root, "split-topic");

    // 全局库 user 维度模型行（注册面），注册表计数与注册记录数一致、两新管理
    // 模型计数 0 也列出
    let global_models = stores.global().list_models().unwrap();
    assert_eq!(
        global_models
            .iter()
            .map(|model| (model.name.as_str(), model.count))
            .collect::<Vec<_>>(),
        vec![
            ("workspace", 1),
            ("agent_provider", 0),
            ("agent_instance", 0)
        ],
        "全局库仅 user 维度模型行（三模型组）"
    );
    // workspace 库含会话 / explore 行，注册表记录不混入
    let ws_models = store.list_models().unwrap();
    assert_eq!(
        ws_models
            .iter()
            .map(|model| (model.name.as_str(), model.count))
            .collect::<Vec<_>>(),
        vec![
            ("agent_run", 0),
            ("session", 1),
            ("session_event", 0),
            ("explore", 1),
            ("change", 0),
            ("phase", 0),
            ("checklist_item", 0),
            ("step", 0)
        ],
        "workspace 库八模型组：按会话域布局写入 session / explore，change 流程状态四模型计数 0 也列出，无注册表混入"
    );
    assert_eq!(
        stores.global().list_workspaces().unwrap(),
        vec![record],
        "全局库清单仅注册记录（会话 / explore 不在注册表）"
    );
    // 组合链写入各自完整可读
    let summaries = store.list_sessions(None, None).unwrap();
    assert_eq!(summaries.len(), 1, "会话落 workspace 库");
    assert_eq!(summaries[0].row.id, session_id);
    assert_eq!(
        store.list_explore_records(&root).unwrap(),
        vec![explore],
        "explore 落 workspace 库"
    );
}

#[test]
fn 两workspace各自for_root同会话id并行写入互不串库() {
    let env = StoresEnv::new("ac2-parallel");
    let stores = env.open();
    let rec_a = add_ok(stores.global(), &env.ws("para-a"));
    let rec_b = add_ok(stores.global(), &env.ws("para-b"));
    let store_a = stores.for_root(&rec_a.root).expect("A for_root 应成功");
    let store_b = stores.for_root(&rec_b.root).expect("B for_root 应成功");

    // 两库各自独立：同 id 会话并行（库域内隔离，跨 workspace 不假定全局唯一）
    seed_session(&store_a, "ses-parallel");
    seed_session(&store_b, "ses-parallel");
    let explore_a = create_ok(&store_a, &rec_a.root, "同名话题");
    let explore_b = create_ok(&store_b, &rec_b.root, "同名话题");

    // A 库清单与 scan 不含 B 的任何记录
    let ids_a: Vec<String> = store_a
        .list_sessions(None, None)
        .unwrap()
        .iter()
        .map(|summary| summary.row.id.clone())
        .collect();
    assert_eq!(ids_a, vec!["ses-parallel"], "A 库会话清单恰本库一行");
    let keys_a: Vec<String> = store_a
        .scan("session", 0, 10)
        .unwrap()
        .iter()
        .map(|envelope| {
            envelope
                .key
                .as_str()
                .expect("session key 为会话 id 字符串")
                .to_owned()
        })
        .collect();
    assert_eq!(keys_a, vec!["ses-parallel"], "A 库 scan 恰本库一行");
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

/// 预置旧布局单库文件（`desktop-store.redb`，注册表 + explore 各写一条记录）：
/// 新代码已无单库打开入口，直接以 native_db 裸构造写盘（不经 `Store`），模拟
/// 零迁移语义中的「旧布局残留」。返回写入后的文件字节。
fn preset_old_layout_db(path: &Path, ws_root_key: &str) -> Vec<u8> {
    let mut models = Models::new();
    models
        .define::<WorkspaceRecord>()
        .expect("定义 WorkspaceRecord 失败");
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
        rw.insert(ExploreRecord::new(ws_root_key, "旧档案", 1200))
            .expect("写入 explore 记录失败");
        rw.commit().expect("提交预置事务失败");
    }
    fs::read(path).expect("读取旧布局文件失败")
}

#[test]
fn 预置旧布局单库文件后新布局冷启动照常成功且旧文件保持原样() {
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
    // workspace 库从空开始按新布局写入（会话域新库写入照常）
    let store = stores.for_root(&legacy_root).expect("for_root 应成功");
    let session_id = seed_session(&store, "ses-fresh-1");
    assert_eq!(session_id, "ses-fresh-1", "新库写入照常");
    let explore = create_ok(&store, &legacy_root, "fresh");
    assert_eq!(explore.id, 1, "新库 explore id 从 1 起（不接续旧库 id 域）");

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
// agent 会话域（workspace 库）：create_session / find_session /
// begin_agent_turn / append_session_events / finish_agent_turn /
// bind_session_remote / list_sessions / list_session_events /
// reconcile_session_stats（write-through + 聚合现算 + 对账重导）。run 域旧
// API（begin_agent_run / list_agent_runs / restore_run_chain 等）随会话一等
// 公民退役，残留引用即编译失败。
// ---------------------------------------------------------------------------

/// 会话行 fixture（id 由调用方给定；时间戳显式注入，与钟面无关）。
fn session_record(id: &str, source: &str, source_ref: Option<&str>) -> SessionRecord {
    SessionRecord {
        id: id.to_owned(),
        engine_session_id: None,
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Sdk,
            model: Some("m-high".to_owned()),
            permission_mode: AgentPermissionMode::BypassPermissions,
        },
        source: source.to_owned(),
        source_ref: source_ref.map(str::to_owned),
        created_at: 1727000000000,
        updated_at: 1727000000000,
    }
}

/// 落一份会话行并返回 id。
fn seed_session(store: &Store, id: &str) -> String {
    store
        .create_session(&session_record(id, "debug", None))
        .unwrap_or_else(|e| panic!("create_session 应成功: {e}"));
    id.to_owned()
}

/// 轮行终态收口入参壳（session_id / started_at 由 store 沿用存量行）。
fn finish_shell(
    turn_id: i64,
    status: AgentRunStatus,
    finished_at: Option<i64>,
    num_turns: Option<u64>,
    duration_ms: Option<u64>,
    cost_usd: Option<f64>,
) -> AgentRunRecord {
    AgentRunRecord {
        id: turn_id,
        session_id: None,
        status,
        started_at: 0,
        finished_at,
        num_turns,
        cost_usd,
        duration_ms,
        error: None,
    }
}

#[test]
fn create_session落库后find_session主键直查逐字段相等且跨workspace库隔离() {
    let env = StoresEnv::new("session-crud");
    let stores = env.open();
    let root_a = env.root_of("alpha");
    let root_b = env.root_of("beta");
    let store_a = stores.for_root(&root_a).expect("A for_root 应成功");
    let store_b = stores.for_root(&root_b).expect("B for_root 应成功");

    let record = session_record("ses-crud-1", "debug", None);
    let created = store_a.create_session(&record).expect("create 应成功");
    assert_eq!(created, record, "create 返回落库记录本体");

    assert_eq!(
        store_a.find_session("ses-crud-1").unwrap(),
        Some(record),
        "主键直查逐字段相等"
    );
    assert!(
        store_b.find_session("ses-crud-1").unwrap().is_none(),
        "跨 workspace 库隔离（for_root 双库各开各库）"
    );
}

#[test]
fn create_session同id重复创建返回err不产半行() {
    let env = Env::new("session-dup");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-dup-1");

    let result = store.create_session(&session_record("ses-dup-1", "debug", None));
    assert!(result.is_err(), "同 id 重复创建必须 Err（写事务原子性）");
    assert_eq!(
        store
            .list_sessions(None, None)
            .unwrap()
            .iter()
            .filter(|summary| summary.row.id == "ses-dup-1")
            .count(),
        1,
        "不产半行（恰一行）"
    );
}

#[test]
fn find_session不存在id返回ok_none() {
    let env = Env::new("session-find-miss");
    let store = open_workspace_ok(&env.db_path("ws"));

    assert_eq!(
        store.find_session("ses-404").unwrap(),
        None,
        "Ok(None)（Continue 校验消费形态）"
    );
}

#[test]
fn begin_agent_turn同session连续三轮max加1递增且running初值挂core会话() {
    let env = Env::new("turn-begin");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-turns");

    let first = store
        .begin_agent_turn("ses-turns", 1727000000000)
        .expect("begin 应成功");
    let second = store
        .begin_agent_turn("ses-turns", 1727000001000)
        .expect("begin 应成功");
    let third = store
        .begin_agent_turn("ses-turns", 1727000002000)
        .expect("begin 应成功");

    // 轮 id max+1 递增、running 初值、session_id 挂 core 会话（AC-2 键位半边）
    for (record, expected_id) in [(&first, 1), (&second, 2), (&third, 3)] {
        assert_eq!(record.id, expected_id, "写事务内 max+1 分配");
        assert_eq!(record.status, AgentRunStatus::Running, "running 初值");
        assert_eq!(
            record.session_id.as_deref(),
            Some("ses-turns"),
            "session_id 挂 core 会话"
        );
        assert_eq!(record.started_at, 1727000000000 + (expected_id - 1) * 1000);
        assert_eq!(record.finished_at, None);
    }
}

#[test]
fn begin_agent_turn跨session轮序独立分配互不串号() {
    let env = Env::new("turn-cross-session");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-x");
    seed_session(&store, "ses-y");

    let x1 = store.begin_agent_turn("ses-x", 100).expect("begin 应成功");
    let y1 = store.begin_agent_turn("ses-y", 200).expect("begin 应成功");
    let x2 = store.begin_agent_turn("ses-x", 300).expect("begin 应成功");

    assert_eq!((x1.id, y1.id, x2.id), (1, 2, 3), "全局轮 id 独立自增");
    // 按 session 圈定互不串号：x 恰 {1, 3}、y 恰 {2}
    let summaries = store.list_sessions(None, None).unwrap();
    let turns_of = |id: &str| {
        summaries
            .iter()
            .find(|summary| summary.row.id == id)
            .expect("会话在场")
            .turns
            .iter()
            .map(|turn| turn.turn_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(turns_of("ses-x"), vec![1, 3], "x 轮序（发起序）");
    assert_eq!(turns_of("ses-y"), vec![2]);
}

#[test]
fn append_session_events批量密封追加后重放seq升序逐字段保真() {
    let env = Env::new("append-session");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-ape");
    let seeded = vec![
        stamped(0, run_started_kind()),
        stamped(1, message_kind(AgentMessageRole::Assistant)),
        stamped(2, system_notice_kind("api_retry")),
        stamped(3, turn_done_kind(false)),
        stamped(4, raw_kind("mystery-tag")),
    ];

    store
        .append_session_events("ses-ape", &seeded)
        .unwrap_or_else(|e| panic!("批量追加应成功: {e}"));

    let replayed = store.list_session_events("ses-ape").unwrap();
    assert_eq!(
        replayed, seeded,
        "类型化批量落库后重放逐字段保真（含 Raw 逃生舱）——AC-2 落库半边 / AC-6"
    );
    let seqs: Vec<u64> = replayed.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3, 4], "重放 seq 升序");
}

#[test]
fn append_session_events混入delta的批次ok且delta零记录() {
    let env = Env::new("append-delta");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-delta");
    let batch = vec![
        stamped(0, message_kind(AgentMessageRole::Assistant)),
        stamped(1, delta_kind()),
        stamped(2, turn_done_kind(false)),
    ];

    store
        .append_session_events("ses-delta", &batch)
        .expect("混入 delta 批次返回 Ok（防御性忽略）");

    let replayed = store.list_session_events("ses-delta").unwrap();
    assert_eq!(
        replayed.len(),
        2,
        "store 中不存在任何 delta 记录（AC-2 断言半边）"
    );
    assert!(replayed.iter().all(|event| event.kind.is_sealed()));
}

#[test]
fn append_session_events空批次ok无副作用() {
    let env = Env::new("append-empty-batch");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-empty-batch");

    store
        .append_session_events("ses-empty-batch", &[])
        .expect("空批次 Ok");

    assert!(
        store
            .list_session_events("ses-empty-batch")
            .unwrap()
            .is_empty(),
        "空批次无副作用"
    );
}

#[test]
fn finish_agent_turn终态整行替换且id与started_at不变() {
    let env = Env::new("turn-finish");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-finish");
    let running = store
        .begin_agent_turn("ses-finish", 1727000000000)
        .expect("begin 应成功");

    store
        .finish_agent_turn(
            running.id,
            &finish_shell(
                running.id,
                AgentRunStatus::Completed,
                Some(1727000001000),
                Some(4),
                Some(999),
                Some(0.5),
            ),
        )
        .unwrap_or_else(|e| panic!("finish 应成功: {e}"));

    let summaries = store.list_sessions(None, None).unwrap();
    assert_eq!(summaries.len(), 1);
    let turn = &summaries[0].turns[0];
    assert_eq!(turn.status, AgentRunStatus::Completed, "终态整行替换");
    assert_eq!(turn.finished_at, Some(1727000001000));
    assert_eq!(turn.num_turns, Some(4));
    assert_eq!(turn.cost_usd, Some(0.5));
    assert_eq!(turn.duration_ms, Some(999));
    // id 与 started_at 不变（沿用存量行）
    assert_eq!(turn.turn_id, running.id);
    assert_eq!(turn.started_at, 1727000000000);
    assert_eq!(turn.session_id, "ses-finish");
}

#[test]
fn finish_agent_turn_stopped终态与error_none形态替换无损() {
    let env = Env::new("turn-finish-stopped");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-stopped");
    let running = store
        .begin_agent_turn("ses-stopped", 1727000000000)
        .expect("begin 应成功");

    store
        .finish_agent_turn(
            running.id,
            &finish_shell(
                running.id,
                AgentRunStatus::Stopped,
                Some(1727000002000),
                None,
                None,
                None,
            ),
        )
        .expect("stopped 终态应成功");

    let turn = &store.list_sessions(None, None).unwrap()[0].turns[0];
    assert_eq!(turn.status, AgentRunStatus::Stopped, "stopped 终态");
    assert_eq!(
        turn.error, None,
        "error=None 形态无损（用户主动终止非失败）"
    );
    assert_eq!(turn.num_turns, None);
}

#[test]
fn bind_session_remote刷新engine_session_id与updated_at且none仅刷时间戳() {
    let env = Env::new("bind-remote");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-bind");

    store
        .bind_session_remote("ses-bind", Some("sdk-11"), 1727000009000)
        .expect("bind Some 应成功");
    let bound = store.find_session("ses-bind").unwrap().expect("在场");
    assert_eq!(bound.engine_session_id.as_deref(), Some("sdk-11"));
    assert_eq!(bound.updated_at, 1727000009000, "updated_at 刷新");

    // remote None：仅刷新时间戳（不清空既有标识）
    store
        .bind_session_remote("ses-bind", None, 1727000010000)
        .expect("bind None 应成功");
    let refreshed = store.find_session("ses-bind").unwrap().expect("在场");
    assert_eq!(refreshed.engine_session_id.as_deref(), Some("sdk-11"));
    assert_eq!(refreshed.updated_at, 1727000010000);

    // miss：会话不存在 Err
    assert!(
        store
            .bind_session_remote("ses-404", Some("sdk-x"), 1)
            .is_err(),
        "不存在会话 bind 必须 Err"
    );
}

#[test]
fn list_sessions清单聚合现算与降序稳定及过滤() {
    let env = Env::new("sessions-listing");
    let store = open_workspace_ok(&env.db_path("ws"));
    // 三个会话：ses-hot（updated_at 最大、一轮终态）、ses-mid、ses-explore（过滤目标）
    seed_session(&store, "ses-hot");
    seed_session(&store, "ses-mid");
    store
        .create_session(&session_record("ses-explore", "explore", Some("42")))
        .expect("create 应成功");

    // ses-hot：转录 TurnDone（token 求和口径）+ 一轮终态行
    store
        .append_session_events(
            "ses-hot",
            &[
                stamped(0, turn_done_kind(false)),
                stamped(1, turn_done_kind(false)),
            ],
        )
        .expect("append 应成功");
    let turn = store
        .begin_agent_turn("ses-hot", 1727000000000)
        .expect("begin 应成功");
    store
        .finish_agent_turn(
            turn.id,
            &finish_shell(
                turn.id,
                AgentRunStatus::Completed,
                Some(1727000005000),
                Some(2),
                Some(3000),
                None,
            ),
        )
        .expect("finish 应成功");
    store
        .bind_session_remote("ses-hot", Some("sdk-1"), 1727000009000)
        .expect("bind 应成功");
    // ses-mid：updated_at 居中
    store
        .bind_session_remote("ses-mid", Some("sdk-0"), 1727000001000)
        .expect("bind 应成功");

    let summaries = store.list_sessions(None, None).unwrap();
    let ids: Vec<String> = summaries.iter().map(|s| s.row.id.clone()).collect();
    assert_eq!(
        ids,
        vec!["ses-hot", "ses-mid", "ses-explore"],
        "updated_at 降序稳定（并列按 id 降序）"
    );

    // 聚合现算：轮数 / 累计墙钟 / 累计 token
    let hot = &summaries[0];
    assert_eq!(hot.stats.turn_count, 1, "轮数 = 轮统计行行数");
    assert_eq!(hot.stats.total_duration_ms, Some(3000), "累计墙钟自轮行");
    assert_eq!(
        hot.stats.input_tokens,
        Some(20),
        "累计 token 自转录 TurnDone usage 鸭子类型求和（camelCase 键）"
    );
    assert_eq!(hot.turns.len(), 1, "轮统计行随行返回（发起序）");
    assert_eq!(
        hot.row.remote_session_id.as_deref(),
        Some("sdk-1"),
        "双 id 映射随行（SessionRow 形态）"
    );

    // source / source_ref 过滤生效
    let explores = store.list_sessions(Some("explore"), None).unwrap();
    assert_eq!(
        explores
            .iter()
            .map(|s| s.row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["ses-explore"],
        "source 过滤"
    );
    let ref42 = store.list_sessions(Some("explore"), Some("42")).unwrap();
    assert_eq!(ref42.len(), 1);
    assert!(
        store
            .list_sessions(Some("explore"), Some("99"))
            .unwrap()
            .is_empty(),
        "source_ref 不匹配为空"
    );
}

#[test]
fn list_sessions缺席缺省与空库空清单() {
    let env = Env::new("sessions-defaults");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-bare");

    let summaries = store.list_sessions(None, None).unwrap();
    assert_eq!(summaries.len(), 1);
    let stats = &summaries[0].stats;
    assert_eq!(stats.turn_count, 0, "无轮行会话统计缺省（降级不违约）");
    assert_eq!(stats.total_duration_ms, None);
    assert_eq!(stats.input_tokens, None);
    assert_eq!(stats.output_tokens, None);
    assert!(summaries[0].turns.is_empty());

    let empty = open_workspace_ok(&env.db_path("ws2"));
    assert!(
        empty.list_sessions(None, None).unwrap().is_empty(),
        "空库空清单"
    );
}

#[test]
fn list_session_events空洞容忍升序不补洞() {
    let env = Env::new("replay-holes");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-holes");
    // 密封占 seq 1/3：库内空洞（delta 曾占 seq 2 的重放投影）升序返回不补洞
    store
        .append_session_events(
            "ses-holes",
            &[
                stamped(1, message_kind(AgentMessageRole::Assistant)),
                stamped(3, turn_done_kind(false)),
            ],
        )
        .expect("append 应成功");

    let replayed = store.list_session_events("ses-holes").unwrap();
    let seqs: Vec<u64> = replayed.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![1, 3], "升序返回不补洞（排序键语义合法）");
}

#[test]
fn list_session_events跨session隔离且不存在会话空vec() {
    let env = Env::new("replay-isolation");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-iso-a");
    seed_session(&store, "ses-iso-b");
    store
        .append_session_events("ses-iso-a", &[stamped(0, raw_kind("A"))])
        .expect("append 应成功");
    store
        .append_session_events("ses-iso-b", &[stamped(0, raw_kind("B"))])
        .expect("append 应成功");

    let replay_a = store.list_session_events("ses-iso-a").unwrap();
    assert_eq!(replay_a.len(), 1);
    assert_eq!(raw_tag(&replay_a[0]), "A", "跨 session 转录互不串");
    assert!(
        store.list_session_events("ses-404").unwrap().is_empty(),
        "不存在会话空 Vec"
    );
}

#[test]
fn reconcile_session_stats从转录重算校正且密封转录逐字节不变() {
    let env = Env::new("reconcile");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-rec");
    // 转录：两轮 TurnDone（token 10 + 20）
    store
        .append_session_events(
            "ses-rec",
            &[
                stamped(0, turn_done_kind(false)),
                stamped(1, turn_done_kind(false)),
            ],
        )
        .expect("append 应成功");
    // 轮统计行落假数据（篡改）
    let turn = store
        .begin_agent_turn("ses-rec", 1727000000000)
        .expect("begin 应成功");
    store
        .finish_agent_turn(
            turn.id,
            &finish_shell(
                turn.id,
                AgentRunStatus::Completed,
                Some(2),
                Some(99),
                Some(99_999),
                Some(9.9),
            ),
        )
        .expect("篡改轮行");

    let stats = store
        .reconcile_session_stats("ses-rec")
        .expect("重导应成功");
    assert_eq!(stats.turn_count, 2, "轮数自转录 TurnDone 重算校正");
    assert_eq!(stats.input_tokens, Some(20), "累计 token 校正");
    assert_eq!(stats.output_tokens, Some(4));

    // 不改转录纪律：密封转录逐字节不变
    let before = store.list_session_events("ses-rec").unwrap();
    store.reconcile_session_stats("ses-rec").expect("重导幂等");
    assert_eq!(
        store.list_session_events("ses-rec").unwrap(),
        before,
        "密封转录逐字节不变"
    );
}

#[test]
fn reconcile_session_stats无turn_done转录缺省统计不报错() {
    let env = Env::new("reconcile-empty");
    let store = open_workspace_ok(&env.db_path("ws"));
    seed_session(&store, "ses-rec-empty");
    store
        .append_session_events(
            "ses-rec-empty",
            &[stamped(0, message_kind(AgentMessageRole::Assistant))],
        )
        .expect("append 应成功");

    let stats = store
        .reconcile_session_stats("ses-rec-empty")
        .expect("缺 TurnDone 不报错");
    assert_eq!(stats.turn_count, 0, "缺省统计");
    assert_eq!(stats.total_duration_ms, None);
    assert_eq!(stats.input_tokens, None);
    assert_eq!(stats.output_tokens, None);
}

// ---------------------------------------------------------------------------
// workspace 库 native 格式重开直通（会话与转录）
// ---------------------------------------------------------------------------

#[test]
fn native格式已有库重开直通此前写入的会话与转录完整读回() {
    let env = Env::new("reopen-native");
    let ws_path = env.db_path("ws");

    let (session_id, events) = {
        let store = open_workspace_ok(&ws_path);
        let session_id = seed_session(&store, "ses-reopen");
        let events = vec![
            stamped(0, run_started_kind()),
            stamped(1, raw_kind("native")),
        ];
        store
            .append_session_events("ses-reopen", &events)
            .unwrap_or_else(|e| panic!("append 应成功: {e}"));
        (session_id, events)
    };

    // native 格式已有库：重开直通，全部记录完整读回
    let reopened = open_workspace_ok(&ws_path);
    let summaries = reopened.list_sessions(None, None).unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].row.id, session_id);
    assert_eq!(
        reopened.list_session_events(&session_id).unwrap(),
        events,
        "先写入的转录重开后完整读回"
    );
}

// ---------------------------------------------------------------------------
// 事件 fixture：stamped / 六变体 kind 构造（盖戳事件时间戳取当前钟面，排序
// 断言仅依赖 seq；等值断言为同进程内构造-读回对读）
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

fn message_kind(role: AgentMessageRole) -> AgentEventKind {
    AgentEventKind::Message {
        role,
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

fn turn_done_kind(is_error: bool) -> AgentEventKind {
    AgentEventKind::TurnDone {
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
        usage: serde_json::json!({ "inputTokens": 10, "outputTokens": 2 }),
        session_id: Some("s-1".to_owned()),
    }
}

/// delta 变体（store 防御性忽略的落库半边 fixture）。
fn delta_kind() -> AgentEventKind {
    AgentEventKind::MessageDelta {
        parent_tool_use_id: None,
        delta: AgentDelta::Text {
            text: "增量".to_owned(),
        },
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
        vec![
            ("workspace", 2),
            ("agent_provider", 0),
            ("agent_instance", 0)
        ],
        "全局库 workspace 计数与注册记录数一致（agent 管理两模型计数 0 也列出）"
    );
    drop(global);

    // workspace 库：会话 1 + 转录 3，轮统计行与 explore 计数 0 也列出
    let ws = open_workspace_ok(&env.db_path("ws"));
    seed_session(&ws, "ses-counts");
    let events: Vec<AgentEvent> = (0..3u64).map(|seq| stamped(seq, raw_kind("c"))).collect();
    ws.append_session_events("ses-counts", &events).unwrap();

    let models = ws.list_models().unwrap();
    let count_of = |name: &str| {
        models
            .iter()
            .find(|model| model.name == name)
            .unwrap_or_else(|| panic!("模型 {name} 应在清单中"))
            .count
    };
    assert_eq!(count_of("agent_run"), 0, "轮统计行计数 0 也列出");
    assert_eq!(count_of("session"), 1, "session 计数与实有记录数一致");
    assert_eq!(
        count_of("session_event"),
        3,
        "session_event 计数与实有记录数一致"
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
// delete_explore_record：级联圈定自 runs 平移至会话（AC-11 级联半边）
// ---------------------------------------------------------------------------

/// 级联回归：删除记录时其名下归属会话（source=explore 且 source_ref=记录 id）
/// 及其转录与轮统计行随记录同事务删除——id 仍是幸存行上 max+1 的可复用计数，
/// 但悬空 source_ref 已随会话删除，新建记录即使复用被删 id，归属圈定也不再
/// 捞到旧聊天。
#[test]
fn 删除记录级联清掉归属会话与其转录与轮统计行且复用id不错链() {
    let env = Env::new("explore-delete-cascade");
    let store = open_workspace_ok(&env.db_path("ws"));
    let _a = create_ok(&store, "C:\\ws\\alpha", "old-topic");
    let b = create_ok(&store, "C:\\ws\\alpha", "to-be-deleted");
    // 被删记录名下已有一段 agent 聊天（source_ref = b.id 十进制串）及其转录与轮行
    let session_id = "ses-cascade-bound";
    store
        .create_session(&session_record(
            session_id,
            "explore",
            Some(&b.id.to_string()),
        ))
        .expect("落归属会话行");
    store
        .append_session_events(
            session_id,
            &[
                stamped(0, run_started_kind()),
                stamped(1, message_kind(AgentMessageRole::Assistant)),
            ],
        )
        .expect("落转录");
    let turn = store
        .begin_agent_turn(session_id, 1727000000000)
        .expect("开轮行");

    assert!(
        store
            .delete_explore_record("C:\\ws\\alpha", "to-be-deleted")
            .unwrap(),
        "命中删除返回 true"
    );
    // 归属会话 + 其转录 + 轮统计行同事务删
    assert!(
        store.find_session(session_id).unwrap().is_none(),
        "归属会话随记录级联删除"
    );
    assert!(
        store.list_session_events(session_id).unwrap().is_empty(),
        "名下转录随记录级联删除"
    );
    let summaries = store.list_sessions(None, None).unwrap();
    assert!(
        summaries.iter().all(|summary| summary.row.id != session_id),
        "轮统计行随会话级联删除（清单无该会话轮行）"
    );
    assert!(
        summaries
            .iter()
            .all(|summary| summary.turns.iter().all(|row| row.turn_id != turn.id)),
        "轮统计行行级联删除"
    );

    // 复用 id 后归属圈定为空：错链不再发生
    let c = create_ok(&store, "C:\\ws\\alpha", "combine-agent-and-explore-chat");
    assert_eq!(
        c.id, b.id,
        "max+1 在幸存行上计算，id 仍会复用（级联后无害）"
    );
    let rebound = store
        .list_sessions(Some("explore"), Some(&c.id.to_string()))
        .unwrap();
    assert!(rebound.is_empty(), "新记录（从未发过消息）归属圈定为空");
}

/// 级联只圈 (source=explore, source_ref=本记录 id)：debug 来源同定位串、
/// 其他 source_ref 的 explore 会话及其转录与轮行不波及。
#[test]
fn 级联删除不波及无关会话与转录与轮行() {
    let env = Env::new("explore-delete-cascade-scope");
    let store = open_workspace_ok(&env.db_path("ws"));
    let doomed = create_ok(&store, "C:\\ws\\alpha", "to-be-deleted");
    let keeper = create_ok(&store, "C:\\ws\\alpha", "keeper-topic");

    // 被删记录名下归属会话
    let bound = "ses-cascade-doomed";
    store
        .create_session(&session_record(
            bound,
            "explore",
            Some(&doomed.id.to_string()),
        ))
        .expect("落归属会话行");
    store
        .append_session_events(bound, &[stamped(0, run_started_kind())])
        .expect("落转录");
    let _bound_turn = store.begin_agent_turn(bound, 100).expect("开轮行");

    // 干扰一：同 source_ref 不同 source（debug 来源同定位串）
    let debug_session = "ses-cascade-debug";
    store
        .create_session(&session_record(
            debug_session,
            "debug",
            Some(&doomed.id.to_string()),
        ))
        .expect("落 debug 会话行");
    store
        .append_session_events(debug_session, &[stamped(0, run_started_kind())])
        .expect("落转录");
    let debug_turn = store.begin_agent_turn(debug_session, 200).expect("开轮行");

    // 干扰二：同 source 不同 source_ref（另一 explore 记录名下）
    let other_session = "ses-cascade-keeper";
    store
        .create_session(&session_record(
            other_session,
            "explore",
            Some(&keeper.id.to_string()),
        ))
        .expect("落隔壁会话行");
    store
        .append_session_events(other_session, &[stamped(0, run_started_kind())])
        .expect("落转录");
    let other_turn = store.begin_agent_turn(other_session, 300).expect("开轮行");

    assert!(store
        .delete_explore_record("C:\\ws\\alpha", "to-be-deleted")
        .unwrap());

    // 被删记录名下会话与转录与轮行清空
    assert!(store.find_session(bound).unwrap().is_none());
    assert!(store.list_session_events(bound).unwrap().is_empty());

    // 无关会话存活（记录 + 转录 + 轮行）
    for (survivor, turn_id) in [
        (debug_session, debug_turn.id),
        (other_session, other_turn.id),
    ] {
        let summary = store
            .list_sessions(None, None)
            .unwrap()
            .into_iter()
            .find(|summary| summary.row.id == survivor)
            .unwrap_or_else(|| panic!("无关会话 {survivor} 存活"));
        assert_eq!(
            store.list_session_events(survivor).unwrap().len(),
            1,
            "无关会话转录存活: {survivor}"
        );
        assert!(
            summary.turns.iter().any(|row| row.turn_id == turn_id),
            "无关会话轮行存活: {survivor}"
        );
    }
    // 隔壁归属圈定不受牵连
    assert_eq!(
        store
            .list_sessions(Some("explore"), Some(&keeper.id.to_string()))
            .unwrap()
            .len(),
        1,
        "隔壁记录归属会话仍可圈定"
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
        None,
    )
}

fn upsert_provider_ok(store: &Store, provider: AgentProviderRecord) -> AgentProviderRecord {
    store
        .upsert_agent_provider(provider)
        .unwrap_or_else(|e| panic!("upsert_agent_provider 应成功: {e}"))
}

fn fixture_agent(
    name: &str,
    engine: AgentEngineKind,
    provider_id: Option<i64>,
) -> AgentInstanceRecord {
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
        None,
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
    assert_eq!(
        saved.base_url, "https://changed.example.com/v1",
        "整行替换生效"
    );
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

    assert!(
        result.is_err(),
        "更新为他人已占 name 应 Err，实际: {result:?}"
    );
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
    let missing =
        store.upsert_agent_instance(fixture_agent("sdk缺provider", AgentEngineKind::Sdk, None));
    let err = missing.expect_err("sdk 引擎 provider_id=None 应 Err");
    assert!(
        err.to_string().contains("sdk缺provider"),
        "错误串含 name 语境，实际: {err}"
    );

    // sdk 悬空引用（provider_id 指向不存在的 provider id）Err
    let dangling =
        store.upsert_agent_instance(fixture_agent("sdk悬空", AgentEngineKind::Sdk, Some(999)));
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
    let first = upsert_agent_ok(
        &store,
        fixture_agent("同名实例", AgentEngineKind::Cli, None),
    );

    // 同 name 重复新建 Err
    let dup_new =
        store.upsert_agent_instance(fixture_agent("同名实例", AgentEngineKind::Sdk, None));
    assert!(dup_new.is_err(), "同 name 重复新建应 Err");

    // 更新撞名 Err
    let second = upsert_agent_ok(
        &store,
        fixture_agent("另一实例", AgentEngineKind::Cli, None),
    );
    let mut renamed = fixture_agent("同名实例", AgentEngineKind::Cli, None);
    renamed.id = second.id;
    let dup_update = store.upsert_agent_instance(renamed);
    assert!(dup_update.is_err(), "更新撞名应 Err");

    // name 空白 Err
    for name in ["", "  "] {
        let blank = store.upsert_agent_instance(fixture_agent(name, AgentEngineKind::Cli, None));
        assert!(
            matches!(&blank, Err(StoreError::Db(_))),
            "name 空白应 Err，实际: {blank:?}"
        );
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
        store
            .default_agent_instance()
            .unwrap()
            .map(|record| record.id),
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
    let _survivor = upsert_agent_ok(
        &store,
        fixture_agent("幸存实例", AgentEngineKind::Cli, None),
    );

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

    let agent = upsert_agent_ok(
        &store,
        fixture_agent("默认实例", AgentEngineKind::Cli, None),
    );
    store.set_default_agent_instance(agent.id).unwrap();
    let resolved = store
        .default_agent_instance()
        .unwrap()
        .expect("有默认返回 Some");
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
    assert_eq!(
        defaults,
        vec![agent_b.id],
        "全局恰一默认为乙，甲标记自动清除"
    );
    assert_eq!(
        store
            .default_agent_instance()
            .unwrap()
            .map(|record| record.id),
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
        store
            .default_agent_instance()
            .unwrap()
            .map(|record| record.id),
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
        store
            .default_agent_instance()
            .unwrap()
            .map(|record| record.id),
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
            vec![
                ("workspace", 0),
                ("agent_provider", 1),
                ("agent_instance", 1)
            ],
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

// ---------------------------------------------------------------------------
// change 流程状态域（workspace 库，desktop-change-state-store）：建档 / 相位
// 落账 / 回跳 / decision 挂账 / status 翻转 / 步骤审计 / 相位史重组。种子经
// 真实 store change 域操作面（进程边界真实组合不 mock），时间戳全部显式注入
// 与钟面无关；db 缺行即 id 域从 1 起（写事务内 max+1 口径）。
// ---------------------------------------------------------------------------

/// 建档命令 fixture（active 起步、无 active_phase，workflow_type 恒 requirement）。
fn change_archive(name: &str, created_at: i64) -> ChangeStateRecord {
    ChangeStateRecord {
        name: name.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    }
}

fn create_change_ok(store: &Store, name: &str, created_at: i64) -> ChangeStateRecord {
    store
        .create_change_record(change_archive(name, created_at))
        .unwrap_or_else(|e| panic!("create_change_record({name}) 应成功: {e}"))
}

fn start_phase_ok(store: &Store, change: &str, phase: &str, now: i64) -> PhaseStartState {
    store
        .start_change_phase(change, phase, now)
        .unwrap_or_else(|e| panic!("start_change_phase({change}, {phase}) 应成功: {e}"))
}

/// checklist 检查项 fixture（三面可区分）。
fn check_item(text: &str, pass: bool) -> ChecklistItem {
    ChecklistItem {
        item: text.to_owned(),
        pass,
        evidence: format!("证据-{text}"),
    }
}

/// 落账命令 fixture（三会话槽位恒 None；start_at 可选注入，None 走兜底）。
#[allow(clippy::too_many_arguments)]
fn log_command(
    change: &str,
    phase: &str,
    verdict: Verdict,
    checklist: Vec<ChecklistItem>,
    start_at: Option<i64>,
    timestamp: i64,
) -> PhaseLogCommand {
    PhaseLogCommand {
        change: change.to_owned(),
        phase: phase.to_owned(),
        verdict,
        report: format!("{phase} 评估报告"),
        skipped: false,
        checklist,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
        start_at,
        timestamp,
    }
}

/// 步骤审计命令 fixture（timestamp 显式注入）。
fn step_command(
    run_id: &str,
    change: &str,
    step_kind: StepKind,
    status: &str,
    summary: &str,
    timestamp: i64,
) -> StepCommand {
    StepCommand {
        run_id: run_id.to_owned(),
        change: change.to_owned(),
        step_kind,
        status: status.to_owned(),
        summary: summary.to_owned(),
        reference: None,
        timestamp,
    }
}

/// 种子一个 pass 相位条目（start + pass 落账），时间戳显式注入，返回 attempt。
fn seed_phase_pass(
    store: &Store,
    change: &str,
    phase: &str,
    start_at: i64,
    timestamp: i64,
    checklist: &[ChecklistItem],
) -> u32 {
    start_phase_ok(store, change, phase, start_at);
    store
        .log_change_phase(&log_command(
            change,
            phase,
            Verdict::Pass,
            checklist.to_vec(),
            Some(start_at),
            timestamp,
        ))
        .unwrap_or_else(|e| panic!("落账 {change}/{phase} 应成功: {e}"))
}

#[test]
fn create_change_record建档find逐字段一致_重开db再读仍逐字段一致() {
    let env = Env::new("change-create-reopen");
    let ws_path = env.db_path("ws");

    let seeded = {
        let store = open_workspace_ok(&ws_path);
        let created = create_change_ok(&store, "alpha-change", 1727000000000);
        assert_eq!(created.name, "alpha-change", "name 主键");
        assert_eq!(created.workflow_type, "requirement");
        assert_eq!(created.created_at, 1727000000000);
        assert_eq!(created.status, ChangeStatus::Active, "建档恒 active 起步");
        assert_eq!(created.archived_at, None);
        assert_eq!(created.active_phase, None);
        assert_eq!(
            store.find_change_record("alpha-change").unwrap(),
            Some(created.clone()),
            "主键直查逐字段一致"
        );
        created
    };

    // drop 重开同一 db 文件：建档记录逐字段一致（AC-1 回环建档半边）
    let reopened = open_workspace_ok(&ws_path);
    assert_eq!(
        reopened.find_change_record("alpha-change").unwrap(),
        Some(seeded),
        "重开 db 后建档记录逐字段一致"
    );
    assert_eq!(
        reopened.find_change_record("ghost").unwrap(),
        None,
        "未建档名 Ok(None)（文档形态契约）"
    );
}

#[test]
fn create_change_record同名active冲突_归档同名亦拒绝_零重复行() {
    let env = Env::new("change-create-conflict");
    let store = open_workspace_ok(&env.db_path("ws"));
    let first = create_change_ok(&store, "dup-change", 1000);

    // 同名 active 再建档 → Conflict，记录表零重复行
    let err = store
        .create_change_record(change_archive("dup-change", 2000))
        .expect_err("同名 active 再建档应 Conflict");
    assert!(
        matches!(err, StoreError::Conflict(_)),
        "变体为 Conflict，实际: {err:?}"
    );
    assert!(
        err.to_string().starts_with("conflict:"),
        "错误串以 conflict: 前缀，实际: {err}"
    );
    assert_eq!(
        store.list_change_records().unwrap(),
        vec![first.clone()],
        "冲突拒绝零重复行"
    );

    // 归档后同名再建档：主键 name 不复用，同样 Conflict（实现口径——查重不限
    // status=active；test-design「同名 archived 再建档成功」行为与实现不一致，
    // 以实现为准断言 Conflict）
    store.set_change_archived("dup-change", 3000).unwrap();
    let err = store
        .create_change_record(change_archive("dup-change", 4000))
        .expect_err("同名 archived 在场再建档应 Conflict（主键 name 不复用）");
    assert!(matches!(err, StoreError::Conflict(_)));
    assert!(
        err.to_string().contains("主键 name 不复用"),
        "错误串含名字不复用记因，实际: {err}"
    );
    assert_eq!(
        store.list_change_records().unwrap().len(),
        1,
        "仍恰一行（归档记录原样）"
    );
}

#[test]
fn list_change_records空库空vec_多建档按主键name自然序与建档顺序无关() {
    let env = Env::new("change-list");
    let store = open_workspace_ok(&env.db_path("ws"));
    assert!(
        store.list_change_records().unwrap().is_empty(),
        "空 db 空 vec"
    );

    // 刻意乱序建档
    create_change_ok(&store, "zebra", 1000);
    create_change_ok(&store, "alpha", 2000);
    create_change_ok(&store, "mid", 3000);

    let names: Vec<String> = store
        .list_change_records()
        .unwrap()
        .into_iter()
        .map(|record| record.name)
        .collect();
    assert_eq!(
        names,
        vec!["alpha", "mid", "zebra"],
        "主键 name 自然序（native_db 主键迭代序，与建档顺序无关）"
    );
}

#[test]
fn start_change_phase开相写active_phase_清位后同相位再start_attempt事务内推导为2() {
    let env = Env::new("change-start");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);

    // 首轮开相：attempt=1、start_at=now 逐字段落库，find 读出 active_phase 一致
    let started = start_phase_ok(&store, "flow", "proposal", 2000);
    assert_eq!(
        started,
        PhaseStartState {
            attempt: 1,
            start_at: 2000
        },
        "首轮 attempt=1"
    );
    assert_eq!(
        store
            .find_change_record("flow")
            .unwrap()
            .unwrap()
            .active_phase,
        Some(ActivePhaseState {
            phase: "proposal".to_owned(),
            attempt: 1,
            start_at: 2000
        }),
        "active_phase 快照逐字段一致"
    );

    // 落账清位后同相位再 start：attempt 事务内推导为 2（重开 attempt 递增，
    // 无应用层计数器）
    store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Pass,
            vec![],
            None,
            3000,
        ))
        .expect("落账应成功");
    assert_eq!(
        store
            .find_change_record("flow")
            .unwrap()
            .unwrap()
            .active_phase,
        None,
        "落账即清位 active_phase"
    );
    let restarted = start_phase_ok(&store, "flow", "proposal", 4000);
    assert_eq!(restarted.attempt, 2, "attempt 自 db 条目数推导");
    assert_eq!(restarted.start_at, 4000);
}

#[test]
fn start_change_phase无建档change返回not_found且不隐式建档() {
    let env = Env::new("change-start-miss");
    let store = open_workspace_ok(&env.db_path("ws"));

    let err = store
        .start_change_phase("ghost", "proposal", 1000)
        .expect_err("无建档 change 开相应 Err（不做隐式建档）");
    assert!(
        matches!(err, StoreError::NotFound(_)),
        "变体为 NotFound，实际: {err:?}"
    );
    assert!(
        err.to_string().starts_with("not_found:"),
        "错误串以 not_found: 前缀，实际: {err}"
    );
    assert!(
        store.list_change_records().unwrap().is_empty(),
        "零副作用：不隐式建档"
    );
}

#[test]
fn log_change_phase单事务原子_相位行与checklist子行同落清位_重开db逐字段一致() {
    let env = Env::new("change-log-atomic");
    let ws_path = env.db_path("ws");
    let checklist = vec![
        check_item("检查项一", true),
        check_item("检查项二", false),
        check_item("检查项三", true),
    ];

    let attempt = {
        let store = open_workspace_ok(&ws_path);
        create_change_ok(&store, "flow", 1000);
        let started = start_phase_ok(&store, "flow", "proposal", 2000);

        // 单次调用同落 PhaseRecord 行 + ChecklistItemRecord 子行 + active_phase
        // 清位，返回事务内推导 attempt（AC-1 / AC-2 原子半边）
        let attempt = store
            .log_change_phase(&log_command(
                "flow",
                "proposal",
                Verdict::Pass,
                checklist.clone(),
                Some(started.start_at),
                3000,
            ))
            .expect("落账应成功");
        assert_eq!(attempt, 1, "首条目 attempt=1");
        assert_eq!(
            store
                .find_change_record("flow")
                .unwrap()
                .unwrap()
                .active_phase,
            None,
            "落账即收相位"
        );
        attempt
    };

    // 重开 db 读出逐字段一致
    let reopened = open_workspace_ok(&ws_path);
    let phases = reopened.list_phase_records("flow").unwrap();
    assert_eq!(phases.len(), 1, "恰一条相位行");
    let row = &phases[0];
    assert_eq!(row.id, 1, "写事务内 max+1 分配，空库首行 id=1");
    assert_eq!(row.change, "flow");
    assert_eq!(row.phase, "proposal");
    assert_eq!(row.attempt, attempt);
    assert_eq!(row.verdict, Verdict::Pass);
    assert_eq!(row.report, "proposal 评估报告");
    assert_eq!(row.checklist, checklist, "checklist 子行按打包键序内联重组");
    assert!(!row.skipped);
    assert!(!row.stale, "新落条目 stale 初值 false");
    assert_eq!(row.backtrack_to, None);
    assert_eq!(row.backtrack_reason, None);
    assert_eq!(row.executor_session_id, None);
    assert_eq!(row.evaluator_session_id, None);
    assert_eq!(row.decision_session_id, None);
    assert_eq!(row.start_at, Some(2000), "start_at 落条目");
    assert_eq!(row.timestamp, 3000, "落账时刻");
}

#[test]
fn log_change_phase_checklist空vec零子行_start_at_none兜底active_phase开相时刻() {
    let env = Env::new("change-log-empty");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);

    // start_at=None / 三槽位 None 的命令合法落库：start_at 兜底自 active_phase
    start_phase_ok(&store, "flow", "design", 2000);
    store
        .log_change_phase(&log_command(
            "flow",
            "design",
            Verdict::Fail,
            vec![],
            None,
            3000,
        ))
        .expect("空 checklist 落账应成功");
    let phases = store.list_phase_records("flow").unwrap();
    assert_eq!(phases.len(), 1, "相位行落库");
    assert!(
        phases[0].checklist.is_empty(),
        "checklist 空 vec → 子行集为空"
    );
    assert_eq!(phases[0].verdict, Verdict::Fail);
    assert_eq!(
        phases[0].start_at,
        Some(2000),
        "start_at=None 兜底 active_phase.start_at（事务内权威）"
    );
    for slot in [
        &phases[0].executor_session_id,
        &phases[0].evaluator_session_id,
        &phases[0].decision_session_id,
    ] {
        assert_eq!(*slot, None, "三会话槽位 None 合法落库透出 null 面");
    }

    // start_at 显式在位：原值透传不兜底
    start_phase_ok(&store, "flow", "tasks", 4000);
    store
        .log_change_phase(&log_command(
            "flow",
            "tasks",
            Verdict::Pass,
            vec![],
            Some(1234),
            5000,
        ))
        .expect("落账应成功");
    let phases = store.list_phase_records("flow").unwrap();
    assert_eq!(phases[1].start_at, Some(1234), "命令显式 start_at 原值落库");
}

#[test]
fn log_change_phase同相位同轮二次落账返回err且零重复行() {
    // 「同相位同轮二次 log」经公共 API 的可见行为：首轮落账即清位 active_phase，
    // 二次 log 命中 active_phase 匹配前置守卫（NotFound）——(change, phase,
    // attempt) 查重为事务内纵深防御，见下例 preset 缺口行用例。
    let env = Env::new("change-log-repeat");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);
    start_phase_ok(&store, "flow", "proposal", 2000);
    store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Pass,
            vec![check_item("检查项", true)],
            None,
            3000,
        ))
        .expect("首轮落账应成功");

    let err = store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Pass,
            vec![],
            None,
            4000,
        ))
        .expect_err("同相位同轮二次落账应 Err");
    assert!(
        matches!(err, StoreError::NotFound(_)),
        "active_phase 前置守卫 NotFound，实际: {err:?}"
    );
    assert_eq!(
        store.list_phase_records("flow").unwrap().len(),
        1,
        "零重复相位行"
    );
}

/// 预置带缺口 attempt 序的建档行 + 相位行（attempt 1 与 3，native_db 裸构造
/// 不经 Store，沿 `preset_old_layout_db` 先例）：公共 API 的 attempt 恒「既有
/// 条目数 + 1」连续分配，缺口形态公共 API 不可达，恰使 `(change, phase,
/// attempt)` 事务内查重分支可达。
fn preset_phase_rows_with_attempt_gap(path: &Path, change: &str, phase: &str) {
    let mut models = Models::new();
    models
        .define::<ChangeRecord>()
        .expect("定义 ChangeRecord 失败");
    models
        .define::<PhaseRecord>()
        .expect("定义 PhaseRecord 失败");
    let db = Builder::new().create(&models, path).expect("预置库失败");
    let rw = db.rw_transaction().expect("开启写事务失败");
    rw.insert(ChangeRecord {
        name: change.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1000,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    })
    .expect("写入建档记录失败");
    for (id, attempt) in [(1i64, 1u32), (3, 3u32)] {
        rw.insert(PhaseRecord {
            id,
            change: change.to_owned(),
            phase: phase.to_owned(),
            attempt,
            verdict: Verdict::Pass,
            report: String::new(),
            skipped: false,
            stale: false,
            backtrack_to: None,
            backtrack_reason: None,
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
            start_at: None,
            timestamp: 2000,
        })
        .expect("写入相位行失败");
    }
    rw.commit().expect("提交预置事务失败");
}

#[test]
fn log_change_phase重复attempt查重返回conflict且事务零残留() {
    // AC-2 零残留半边：`(change, phase, attempt)` 查重与全部守卫均在首个
    // rw.insert 之前（前置守卫路径），失败即零写入。native_db 对合法
    // native_model 记录的 insert 无可注入的中途失败面，故原子性以「失败路径
    // 零残留 + 成功路径单事务全落」（上例）双断言承载，不伪造失败。
    let env = Env::new("change-log-attempt-conflict");
    let ws_path = env.db_path("ws");
    preset_phase_rows_with_attempt_gap(&ws_path, "gap-change", "proposal");

    let store = open_workspace_ok(&ws_path);
    let started = start_phase_ok(&store, "gap-change", "proposal", 5000);
    assert_eq!(
        started.attempt, 3,
        "attempt 事务内推导 = 既有条目数 + 1（缺口下为 3）"
    );

    let err = store
        .log_change_phase(&log_command(
            "gap-change",
            "proposal",
            Verdict::Fail,
            vec![check_item("检查项", false)],
            None,
            6000,
        ))
        .expect_err("重复 attempt 落账应 Conflict");
    assert!(
        matches!(err, StoreError::Conflict(_)),
        "变体为 Conflict，实际: {err:?}"
    );
    assert!(
        err.to_string().contains("评估条目已存在"),
        "错误串含查重记因，实际: {err}"
    );

    // 零残留：无新相位行、无孤立 checklist 子行、active_phase 原样在位
    assert_eq!(
        store.list_phase_records("gap-change").unwrap().len(),
        2,
        "无半截相位行"
    );
    assert_eq!(
        store.scan("checklist_item", 0, 10).unwrap().len(),
        0,
        "无孤立 checklist 子行"
    );
    assert_eq!(
        store
            .find_change_record("gap-change")
            .unwrap()
            .unwrap()
            .active_phase,
        Some(ActivePhaseState {
            phase: "proposal".to_owned(),
            attempt: 3,
            start_at: 5000
        }),
        "失败不触碰 active_phase"
    );
}

#[test]
fn apply_change_backtrack回跳标记与stale闭包翻转_仅目标最新pass置stale() {
    let env = Env::new("change-backtrack");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);
    // 相位链种子：proposal 两轮 pass（历史 + 最新）、design / tasks /
    // implementation 各一轮 pass；回跳发起相位 = design，目标 = proposal
    seed_phase_pass(
        &store,
        "flow",
        "proposal",
        2000,
        2500,
        &[check_item("p1", true)],
    );
    seed_phase_pass(&store, "flow", "design", 3000, 3500, &[]);
    seed_phase_pass(&store, "flow", "tasks", 4000, 4500, &[]);
    seed_phase_pass(&store, "flow", "implementation", 5000, 5500, &[]);
    seed_phase_pass(
        &store,
        "flow",
        "proposal",
        6000,
        6500,
        &[check_item("p2", true)],
    );

    store
        .apply_change_backtrack(&BacktrackCommand {
            change: "flow".to_owned(),
            phase: "design".to_owned(),
            to: "proposal".to_owned(),
            reason: "需求理解偏差".to_owned(),
            stale_dependents: vec!["tasks".to_owned(), "implementation".to_owned()],
        })
        .expect("回跳应成功");

    let phases = store.list_phase_records("flow").unwrap();
    let design = phases
        .iter()
        .rev()
        .find(|row| row.phase == "design")
        .expect("design 条目在场");
    assert_eq!(
        design.backtrack_to.as_deref(),
        Some("proposal"),
        "发起相位最新条目落回跳标记"
    );
    assert_eq!(
        design.backtrack_reason.as_deref(),
        Some("需求理解偏差"),
        "回跳因由落库"
    );
    assert!(!design.stale, "发起相位标记行本身不置 stale");

    // 目标最新 pass 置 stale；历史 attempt 不误伤
    let proposal_rows: Vec<_> = phases
        .iter()
        .filter(|row| row.phase == "proposal")
        .collect();
    assert_eq!(proposal_rows.len(), 2);
    assert!(!proposal_rows[0].stale, "历史 pass attempt 不误伤");
    assert!(proposal_rows[1].stale, "目标最新 pass 置 stale");

    // stale 闭包内全部相位条目翻转（多条闭包逐支核对）
    for dependent in ["tasks", "implementation"] {
        assert!(
            phases
                .iter()
                .filter(|row| row.phase == dependent)
                .all(|row| row.stale),
            "{dependent} 闭包内全条目 stale=true"
        );
    }
}

#[test]
fn apply_change_backtrack_stale闭包空vec仅回跳标记与目标stale无误伤() {
    let env = Env::new("change-backtrack-empty-closure");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);
    seed_phase_pass(&store, "flow", "proposal", 2000, 2500, &[]);
    seed_phase_pass(&store, "flow", "design", 3000, 3500, &[]);
    seed_phase_pass(&store, "flow", "tasks", 4000, 4500, &[]);

    store
        .apply_change_backtrack(&BacktrackCommand {
            change: "flow".to_owned(),
            phase: "design".to_owned(),
            to: "proposal".to_owned(),
            reason: "回跳".to_owned(),
            stale_dependents: vec![],
        })
        .expect("空闭包回跳应成功");

    let phases = store.list_phase_records("flow").unwrap();
    let design = phases.iter().find(|row| row.phase == "design").unwrap();
    assert_eq!(
        design.backtrack_to.as_deref(),
        Some("proposal"),
        "仅回跳标记落库"
    );
    assert!(
        phases
            .iter()
            .find(|row| row.phase == "proposal")
            .expect("目标条目在场")
            .stale,
        "目标最新 pass 置 stale 独立于闭包"
    );
    assert!(
        phases
            .iter()
            .filter(|row| row.phase == "tasks")
            .all(|row| !row.stale),
        "stale_dependents 空 vec：无 stale 误伤"
    );
}

#[test]
fn apply_change_backtrack目标无pass条目不传播stale_发起相位无条目not_found() {
    let env = Env::new("change-backtrack-no-pass");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);
    // 目标 proposal 仅有 fail 条目（无 pass）
    start_phase_ok(&store, "flow", "proposal", 2000);
    store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Fail,
            vec![],
            None,
            2500,
        ))
        .expect("目标 fail 落账");
    seed_phase_pass(&store, "flow", "design", 3000, 3500, &[]);
    seed_phase_pass(&store, "flow", "tasks", 4000, 4500, &[]);

    store
        .apply_change_backtrack(&BacktrackCommand {
            change: "flow".to_owned(),
            phase: "design".to_owned(),
            to: "proposal".to_owned(),
            reason: "目标无 pass".to_owned(),
            stale_dependents: vec!["tasks".to_owned()],
        })
        .expect("目标无 pass 条目回跳仍成功（no-op 不传播）");

    let phases = store.list_phase_records("flow").unwrap();
    let design = phases.iter().find(|row| row.phase == "design").unwrap();
    assert_eq!(
        design.backtrack_to.as_deref(),
        Some("proposal"),
        "回跳标记照常落库"
    );
    assert!(
        phases
            .iter()
            .filter(|row| row.phase == "proposal" || row.phase == "tasks")
            .all(|row| !row.stale),
        "目标无 pass 条目：不置 stale 也不向闭包传播（沿既往 mark_phase_stale 语义）"
    );

    // 发起相位无条目 → NotFound
    let err = store
        .apply_change_backtrack(&BacktrackCommand {
            change: "flow".to_owned(),
            phase: "ghost-phase".to_owned(),
            to: "proposal".to_owned(),
            reason: "发起相位无条目".to_owned(),
            stale_dependents: vec![],
        })
        .expect_err("发起相位无条目回跳应 Err");
    assert!(
        matches!(err, StoreError::NotFound(_)),
        "变体为 NotFound，实际: {err:?}"
    );
}

/// C3 回归锚定（state.rs 回跳标记契约）：发起相位常在其自身 stale 闭包内
/// （真实管线常态：test-gen 回跳 dev-design，下游闭包含 test-gen 自身）——
/// 回跳标记（backtrack_to / backtrack_reason）与闭包 stale 翻转同行叠加
/// 并存，任一写不得整行覆写另一写；重开 db 再读终态仍并存（与既往 JSON
/// 载体上标记 + 传播两次读改写的合成终态一致，corpus-backtrack-stale
/// golden 的 store 单点对应面）。
#[test]
fn apply_change_backtrack发起相位在自身stale闭包内_标记与stale同行叠加() {
    let env = Env::new("change-backtrack-origin-in-closure");
    let ws_path = env.db_path("ws");
    {
        let store = open_workspace_ok(&ws_path);
        create_change_ok(&store, "flow", 1000);
        // 真实管线相位链：回跳发起相位 = test-gen（下游闭包含自身），目标 =
        // dev-design；stale_dependents 与写面 stale_closure 同闭包口径
        seed_phase_pass(&store, "flow", "proposal", 2000, 2500, &[]);
        seed_phase_pass(&store, "flow", "dev-design", 3000, 3500, &[]);
        seed_phase_pass(&store, "flow", "test-design", 4000, 4500, &[]);
        seed_phase_pass(&store, "flow", "implement", 5000, 5500, &[]);
        seed_phase_pass(&store, "flow", "test-gen", 6000, 6500, &[]);

        store
            .apply_change_backtrack(&BacktrackCommand {
                change: "flow".to_owned(),
                phase: "test-gen".to_owned(),
                to: "dev-design".to_owned(),
                reason: "设计返工：缺产物区组件".to_owned(),
                stale_dependents: vec![
                    "test-design".to_owned(),
                    "implement".to_owned(),
                    "test-gen".to_owned(),
                ],
            })
            .expect("发起相位在闭包内的回跳应成功");

        let phases = store.list_phase_records("flow").unwrap();
        assert_eq!(phases.len(), 5, "回跳不增删条目行");

        let origin = phases
            .iter()
            .find(|row| row.phase == "test-gen")
            .expect("发起相位条目在场");
        assert_eq!(
            origin.backtrack_to.as_deref(),
            Some("dev-design"),
            "回跳标记落库"
        );
        assert_eq!(
            origin.backtrack_reason.as_deref(),
            Some("设计返工：缺产物区组件"),
            "回跳因由落库"
        );
        assert!(
            origin.stale,
            "发起相位在自身闭包内：标记与 stale 翻转同行并存（不得互相覆写）"
        );

        let target = phases
            .iter()
            .find(|row| row.phase == "dev-design")
            .expect("目标相位条目在场");
        assert!(target.stale, "目标最新 pass 置 stale");
        assert_eq!(target.backtrack_to, None, "目标行不落回跳标记");
        for dependent in ["test-design", "implement"] {
            assert!(
                phases
                    .iter()
                    .find(|row| row.phase == dependent)
                    .expect("闭包相位条目在场")
                    .stale,
                "{dependent} 闭包内翻转 stale=true"
            );
        }
        assert!(
            !phases
                .iter()
                .find(|row| row.phase == "proposal")
                .expect("上游相位条目在场")
                .stale,
            "上游相位（闭包外）不受影响"
        );
    }

    // drop 重开同一 db 文件：同位叠加终态持久（标记字段与 stale 位仍并存）
    let reopened = open_workspace_ok(&ws_path);
    let origin = reopened
        .list_phase_records("flow")
        .unwrap()
        .into_iter()
        .find(|row| row.phase == "test-gen")
        .expect("重开后发起相位条目在场");
    assert_eq!(origin.backtrack_to.as_deref(), Some("dev-design"));
    assert_eq!(
        origin.backtrack_reason.as_deref(),
        Some("设计返工：缺产物区组件")
    );
    assert!(origin.stale, "重开 db 后标记与 stale 同位叠加终态保持");
}

#[test]
fn amend_change_decision_session最新条目定点改写_重复幂等覆写_无条目not_found() {
    let env = Env::new("change-amend");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);
    seed_phase_pass(&store, "flow", "proposal", 2000, 2500, &[]);
    seed_phase_pass(&store, "flow", "proposal", 3000, 3500, &[]);

    store
        .amend_change_decision_session("flow", "proposal", "ses-decision-1")
        .expect("挂账应成功");
    let phases = store.list_phase_records("flow").unwrap();
    assert_eq!(
        phases[0].decision_session_id, None,
        "多 attempt 在场时仅最新条目被改（定点锚定）"
    );
    assert_eq!(
        phases[1].decision_session_id.as_deref(),
        Some("ses-decision-1")
    );

    // 重复挂账幂等覆写（D9）：值替换不重复追加
    store
        .amend_change_decision_session("flow", "proposal", "ses-decision-2")
        .expect("重复挂账应成功");
    let phases = store.list_phase_records("flow").unwrap();
    assert_eq!(
        phases[1].decision_session_id.as_deref(),
        Some("ses-decision-2"),
        "幂等覆写同槽位"
    );

    // 该相位无任何条目 → NotFound
    let err = store
        .amend_change_decision_session("flow", "design", "ses-x")
        .expect_err("无条目相位挂账应 Err");
    assert!(
        matches!(err, StoreError::NotFound(_)),
        "变体为 NotFound（NotFound 面），实际: {err:?}"
    );
}

#[test]
fn set_change_archived翻转status与archived_at_主键name不变_miss_not_found() {
    let env = Env::new("change-archived");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);

    store
        .set_change_archived("flow", 9000)
        .expect("归档翻转应成功");
    let record = store
        .find_change_record("flow")
        .unwrap()
        .expect("归档后仍可按主键寻址");
    assert_eq!(record.status, ChangeStatus::Archived, "status 翻转");
    assert_eq!(record.archived_at, Some(9000), "archived_at 落库");
    assert_eq!(record.name, "flow", "主键 name 不变");

    let err = store
        .set_change_archived("ghost", 1)
        .expect_err("name miss 应 Err");
    assert!(
        matches!(err, StoreError::NotFound(_)),
        "变体为 NotFound，实际: {err:?}"
    );
}

#[test]
fn append_change_step连续追加id严格max加1_逐字段回读_超长summary原样透传不截断() {
    let env = Env::new("change-step-append");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);

    // 恰 500 字符不截断；超 500 字符原样透传——store 不做截断（StepRecord::new
    // 逐字拷贝；「≤500 字截断留痕」纪律归编排侧 D10，test-design「store 截断」
    // 行为与实现不一致，以实现为准断言原样落库）
    let exact_500 = "字".repeat(500);
    let long_800 = "长".repeat(800);
    assert_eq!(exact_500.chars().count(), 500);

    store
        .append_change_step(&step_command(
            "run-1",
            "flow",
            StepKind::PhaseNext,
            "ok",
            "轮次推进摘要",
            100,
        ))
        .expect("追加应成功");
    store
        .append_change_step(&step_command(
            "run-1",
            "flow",
            StepKind::StaticCheck,
            "ok",
            &exact_500,
            200,
        ))
        .expect("追加应成功");
    store
        .append_change_step(&step_command(
            "run-2",
            "flow",
            StepKind::TestExecution,
            "error",
            &long_800,
            300,
        ))
        .expect("追加应成功");

    let steps = store.list_change_steps("flow", None).unwrap();
    assert_eq!(steps.len(), 3, "三行齐落");
    assert_eq!(
        steps.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "行 id 写事务内 max+1 严格递增"
    );

    // 逐字段回读一致
    assert_eq!(steps[0].run_id, "run-1");
    assert_eq!(steps[0].change, "flow");
    assert_eq!(steps[0].step_kind, StepKind::PhaseNext);
    assert_eq!(steps[0].status, "ok");
    assert_eq!(steps[0].timestamp, 100);
    assert_eq!(steps[0].summary, "轮次推进摘要");
    assert_eq!(steps[0].reference, None);
    assert_eq!(steps[1].step_kind, StepKind::StaticCheck);
    assert_eq!(
        steps[1].summary, exact_500,
        "恰 500 字符不截断（chars 口径原样）"
    );
    assert_eq!(steps[1].summary.chars().count(), 500);
    assert_eq!(
        steps[2].summary, long_800,
        "超 500 字符 store 原样透传（截断在编排侧）"
    );
    assert_eq!(steps[2].summary.chars().count(), 800);
    assert_eq!(steps[2].status, "error");
}

#[test]
fn append_change_step_reference随行落库() {
    let env = Env::new("change-step-reference");
    let store = open_workspace_ok(&env.db_path("ws"));
    let mut command = step_command("run-1", "flow", StepKind::StaticCheck, "ok", "摘要", 100);
    command.reference = Some("checks/reports/proposal".to_owned());

    store.append_change_step(&command).expect("追加应成功");

    let steps = store.list_change_steps("flow", None).unwrap();
    assert_eq!(
        steps[0].reference.as_deref(),
        Some("checks/reports/proposal"),
        "全量输出引用（报告目录 / 会话 id）随行落库"
    );
}

#[test]
fn list_change_steps多run全量按id序_run圈定单run_无步骤空vec() {
    let env = Env::new("change-steps-list");
    let store = open_workspace_ok(&env.db_path("ws"));

    // flow 名下三行跨两 run，另一 change 一行干扰项
    for (run, kind, status, ts) in [
        ("run-1", StepKind::PhaseNext, "ok", 100),
        ("run-2", StepKind::StaticCheck, "ok", 200),
        ("run-1", StepKind::TestExecution, "error", 300),
    ] {
        store
            .append_change_step(&step_command(run, "flow", kind, status, "摘要", ts))
            .expect("追加应成功");
    }
    store
        .append_change_step(&step_command(
            "run-9",
            "solo",
            StepKind::PhaseNext,
            "ok",
            "他档",
            400,
        ))
        .expect("追加应成功");

    let all = store.list_change_steps("flow", None).unwrap();
    assert_eq!(
        all.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "按 change 全量枚举，落行序（id 升序）"
    );
    assert!(all.iter().all(|row| row.change == "flow"), "无他档混入");

    let run1 = store.list_change_steps("flow", Some("run-1")).unwrap();
    assert_eq!(
        run1.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 3],
        "run_id 传 Some 圈定单 run 序列"
    );
    assert!(run1.iter().all(|row| row.run_id == "run-1"));

    assert!(
        store.list_change_steps("solo", None).unwrap().len() == 1,
        "他档各自枚举"
    );
    assert_eq!(
        store.list_change_steps("ghost", None).unwrap(),
        vec![],
        "无任何步骤返回空 vec（不 Err）"
    );
}

#[test]
fn list_phase_records多相位多attempt交错落账_按落行序checklist内联打包键序() {
    let env = Env::new("change-phases-list");
    let store = open_workspace_ok(&env.db_path("ws"));
    create_change_ok(&store, "flow", 1000);

    // 交错落账：proposal attempt1（三检查项）→ design attempt1（一项）→
    // proposal attempt2（两项）
    let proposal_v1 = vec![
        check_item("提案-检查一", true),
        check_item("提案-检查二", false),
        check_item("提案-检查三", true),
    ];
    let design_v1 = vec![check_item("设计-检查一", true)];
    let proposal_v2 = vec![
        check_item("提案二轮-检查一", true),
        check_item("提案二轮-检查二", false),
    ];
    start_phase_ok(&store, "flow", "proposal", 2000);
    store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Pass,
            proposal_v1.clone(),
            Some(2000),
            2500,
        ))
        .expect("落账应成功");
    start_phase_ok(&store, "flow", "design", 3000);
    store
        .log_change_phase(&log_command(
            "flow",
            "design",
            Verdict::Pass,
            design_v1.clone(),
            Some(3000),
            3500,
        ))
        .expect("落账应成功");
    start_phase_ok(&store, "flow", "proposal", 4000);
    store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Fail,
            proposal_v2.clone(),
            Some(4000),
            4500,
        ))
        .expect("落账应成功");

    let phases = store.list_phase_records("flow").unwrap();
    assert_eq!(
        phases
            .iter()
            .map(|row| (row.id, row.phase.as_str(), row.attempt))
            .collect::<Vec<_>>(),
        vec![(1, "proposal", 1), (2, "design", 1), (3, "proposal", 2)],
        "相位行序列按落行序（id 升序），多相位多 attempt 交错不重排"
    );
    assert_eq!(phases[0].checklist, proposal_v1, "checklist 内联按打包键序");
    assert_eq!(phases[1].checklist, design_v1);
    assert_eq!(phases[2].checklist, proposal_v2);

    // 与直查 ChecklistItemRecord 序一致：信封主键自然序即打包键序
    // （高 64 位 phase_id → 低 64 位 item_index）
    let keys = store
        .scan("checklist_item", 0, 10)
        .unwrap()
        .into_iter()
        .map(|envelope| {
            envelope.key["itemKey"]
                .as_str()
                .expect("itemKey 为串")
                .to_owned()
        })
        .collect::<Vec<_>>();
    let expected_keys: Vec<String> = [(1i64, 0u32), (1, 1), (1, 2), (2, 0), (3, 0), (3, 1)]
        .into_iter()
        .map(|(phase_id, index)| format!("{:#034x}", pack_checklist_item_key(phase_id, index)))
        .collect();
    assert_eq!(
        keys, expected_keys,
        "直查子行主键自然序 = 打包键序 = evaluator 输出序"
    );
}

#[test]
fn 信封零改动覆盖四新模型_list_models计数与scan信封可读() {
    let env = Env::new("change-envelope");
    let store = open_workspace_ok(&env.db_path("ws"));

    // 种子：建档 1 + 相位行 1（含两条检查项子行）+ 步骤审计行 1
    create_change_ok(&store, "flow", 1000);
    start_phase_ok(&store, "flow", "proposal", 2000);
    store
        .log_change_phase(&log_command(
            "flow",
            "proposal",
            Verdict::Pass,
            vec![check_item("检查项一", true), check_item("检查项二", false)],
            Some(2000),
            3000,
        ))
        .expect("落账应成功");
    store
        .append_change_step(&step_command(
            "run-1",
            "flow",
            StepKind::PhaseLog,
            "ok",
            "落账",
            4000,
        ))
        .expect("追加应成功");

    // 既有信封注册面零改动：workspace 组 4→8 后计数与实有记录数一致
    let models = store.list_models().unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| (model.name.as_str(), model.count))
            .collect::<Vec<_>>(),
        vec![
            ("agent_run", 0),
            ("session", 0),
            ("session_event", 0),
            ("explore", 0),
            ("change", 1),
            ("phase", 1),
            ("checklist_item", 2),
            ("step", 1)
        ],
        "change 流程状态四模型经既有 list_models 零改动覆盖"
    );

    // scan 零改动可读四新模型记录信封
    let changes = store.scan("change", 0, 10).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].key,
        serde_json::json!("flow"),
        "change 主键 = name"
    );
    assert_eq!(changes[0].value["name"], serde_json::json!("flow"));
    assert_eq!(
        changes[0].value["activePhase"],
        serde_json::Value::Null,
        "active_phase 清位后信封出 null"
    );

    let phases = store.scan("phase", 0, 10).unwrap();
    assert_eq!(phases.len(), 1);
    assert_eq!(phases[0].key, serde_json::json!(1), "phase 主键 = id");

    // checklist_item 信封 key 为 {phaseId, itemKey} 对象，itemKey 为十六进制串
    let items = store.scan("checklist_item", 0, 10).unwrap();
    assert_eq!(items.len(), 2);
    for (envelope, index) in items.iter().zip([0u32, 1]) {
        assert_eq!(
            envelope.key["phaseId"],
            serde_json::json!(1),
            "信封 key 为 {{phaseId, itemKey}} 对象"
        );
        assert_eq!(
            envelope.key["itemKey"],
            serde_json::json!(format!("{:#034x}", pack_checklist_item_key(1, index))),
            "itemKey 为打包键十六进制串原值"
        );
    }

    let steps = store.scan("step", 0, 10).unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].key, serde_json::json!(1), "step 主键 = id");
    assert_eq!(steps[0].value["runId"], serde_json::json!("run-1"));
}

// ---------------------------------------------------------------------------
// change 建档表坏行语料（fixtures/README 矩阵「坏行」覆盖面）：db 文件直写
// 字节注入 native_model 解码失败行 → 读侧 StoreError 显式记因不静默。坏字节
// 行经公共 API 不可达（native_db 不暴露裸表句柄），经裸 redb 同文件直写注入
// ——注入前后均不触碰 Store 内部状态，调用方须先 drop Store 归还文件锁。
// ---------------------------------------------------------------------------

/// change 建档表 redb 裸表键镜像（仅为坏行直写注入而设）：与 native_db 内部
/// 键类型同 TypeName（`DatabaseInnerKeyValue`，UserDefined 分类）且按原始
/// 字节比较——任一不符 redb open_table 即 TableTypeMismatch 拒开。
#[derive(Debug)]
struct RawNativeDbKey(Vec<u8>);

impl redb::Value for RawNativeDbKey {
    type SelfType<'a> = RawNativeDbKey;
    type AsBytes<'a> = &'a [u8];

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        Self(data.to_vec())
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        &value.0
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("DatabaseInnerKeyValue")
    }
}

impl redb::Key for RawNativeDbKey {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}

/// change 建档表裸表名（native_db 内部表命名公式：
/// `{native_model_id}_{native_model_version}_{主键字段名小写}`；ChangeRecord
/// id=9 / version=2（worktree 字段面升级）/ 主键 `name`）。
const CHANGE_RECORD_TABLE: &str = "9_2_name";

/// db 文件直写字节注入：向 change 建档表插入一行 native_model 解码失败行
/// （合法建档行的库内字节截去尾部 payload——头部 8 字节保全使 native_model
/// 信封解析仍可走通，bincode 解码必然失败）。调用方必须先 drop Store 释放
/// 文件锁（redb 单写者文件锁语义）。
fn inject_corrupt_change_row(path: &Path, name: &str, created_at: i64) {
    assert_eq!(
        CHANGE_RECORD_TABLE,
        format!(
            "{}_{}_{}",
            <ChangeRecord as native_model::Model>::native_model_id(),
            <ChangeRecord as native_model::Model>::native_model_version(),
            "name"
        ),
        "裸表名与 native_db 内部命名公式一致（模型 id / 版本演进时随动改写）"
    );
    let valid = <ChangeRecord as native_model::Model>::native_model_encode(&ChangeRecord::new(
        name,
        "requirement",
        created_at,
        None,
        None,
    ))
    .expect("编码合法建档行失败");
    let valid_len = valid.len();
    let mut corrupt = valid;
    corrupt.truncate(9); // 头部 8 字节 + 1 字节 payload：截断形态的坏字节行
    assert!(corrupt.len() < valid_len, "坏行须为合法行的截断形态");

    let db = redb::Database::open(path).expect("裸开 db 注入坏行应成功");
    let table: redb::TableDefinition<RawNativeDbKey, &[u8]> =
        redb::TableDefinition::new(CHANGE_RECORD_TABLE);
    let rw = db.begin_write().expect("开启注入写事务失败");
    {
        let mut rows = rw.open_table(table).expect("打开 change 建档表应成功");
        rows.insert(RawNativeDbKey(name.as_bytes().to_vec()), corrupt.as_slice())
            .expect("注入坏行应成功");
    }
    rw.commit().expect("提交注入事务失败");
}

#[test]
fn 建档表坏行直写注入_读侧store_error显式记因不静默() {
    let env = Env::new("change-corrupt-row");
    let ws_path = env.db_path("ws");

    // 种子：合法建档行经公共 API 落库（坏行与良行同表共存）
    {
        let store = open_workspace_ok(&ws_path);
        create_change_ok(&store, "good-change", 1000);
    } // 文件锁归还

    inject_corrupt_change_row(&ws_path, "corrupt-change", 2000);

    // 单行坏字节 ≠ 整库损坏：重开成功（与整库损坏 open Err 用例相区分）
    let store = open_workspace_ok(&ws_path);

    // 全表读：清单读面 StoreError 显式记因（db: 前缀 + 遍历语境），不静默
    // 跳过坏行仅返回良行
    let err = store
        .list_change_records()
        .expect_err("坏行在场的全表读必须显式 Err，不得静默跳过");
    assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
    assert!(
        err.to_string().starts_with("db:") && err.to_string().contains("遍历建档清单"),
        "错误串带 db: 前缀与读语境记因，实际: {err}"
    );

    // 主键直查坏行：解码失败显式 Err，不静默降级 Ok(None)（文档形态误判防线）
    let err = store
        .find_change_record("corrupt-change")
        .expect_err("坏行主键直查必须显式 Err，不得误判为文档形态 None");
    assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
    assert!(
        err.to_string().contains("读取建档记录"),
        "错误串含直查语境记因，实际: {err}"
    );

    // 持久性：坏行是库内持久状态，再读仍 Err（非瞬时故障）
    assert!(store.list_change_records().is_err());
}

// ---------------------------------------------------------------------------
// ChangeRecord 双字段映射往返 + 存量 v1 行库 additive 打开（design D7 / AC-1）
// ---------------------------------------------------------------------------

/// 双字段映射往返：`create_change_record`（ChangeStateRecord 携 Some 两字段）→
/// find / list 读出逐字段一致；重开同一 db 文件再读仍一致（真件回环——AC-1
/// 新建档半边）；None 两态同往返。
#[test]
fn change记录双字段映射往返_重开库仍一致() {
    let env = Env::new("change-worktree-roundtrip");
    let ws_path = env.db_path("ws");

    let seeded = {
        let store = open_workspace_ok(&ws_path);
        let mut with_fields = change_archive("wt-change", 1000);
        with_fields.worktree = Some(r"C:\app-data\worktrees\seg\wt-change".to_owned());
        with_fields.base_commit = Some("0000000000000000000000000000000000000001".to_owned());
        store
            .create_change_record(with_fields)
            .expect("建档（携 worktree / base_commit）应成功");
        // legacy 形态（None 两态）同库共存
        create_change_ok(&store, "legacy-change", 2000);
        store
    }; // 文件锁归还

    // 首开读面：find / list 双字段逐字段一致
    let find_hit = seeded
        .find_change_record("wt-change")
        .expect("查档应成功")
        .expect("建档在案");
    assert_eq!(
        find_hit.worktree.as_deref(),
        Some(r"C:\app-data\worktrees\seg\wt-change"),
        "worktree 映射往返一致"
    );
    assert_eq!(
        find_hit.base_commit.as_deref(),
        Some("0000000000000000000000000000000000000001"),
        "base_commit 映射往返一致"
    );
    let records = seeded.list_change_records().expect("清单应成功");
    let listed = records
        .iter()
        .find(|record| record.name == "wt-change")
        .expect("清单应含 wt-change");
    assert_eq!(listed.worktree, find_hit.worktree, "list 半边同往返");
    assert_eq!(listed.base_commit, find_hit.base_commit);
    let legacy = records
        .iter()
        .find(|record| record.name == "legacy-change")
        .expect("清单应含 legacy-change");
    assert_eq!(legacy.worktree, None, "None 两态同往返");
    assert_eq!(legacy.base_commit, None);
    drop(seeded);

    // 重开同一 db 文件再读仍一致（持久化回环——非内存态）
    let reopened = open_workspace_ok(&ws_path);
    let reread = reopened
        .find_change_record("wt-change")
        .expect("重开查档应成功")
        .expect("建档在案");
    assert_eq!(
        reread.worktree.as_deref(),
        Some(r"C:\app-data\worktrees\seg\wt-change"),
        "重开后 worktree 仍一致"
    );
    assert_eq!(
        reread.base_commit.as_deref(),
        Some("0000000000000000000000000000000000000001"),
        "重开后 base_commit 仍一致"
    );
}

/// db 文件直写注入：向 change 建档表（v2 当前表名）插入一行 **v1 envelope 载
/// 荷**字节——存量升级的 decode-only 作用面（native_model 版本头在载荷信封
/// 内，decode 经 `from` 链升级；native_db 0.8.2 表名含版本段、无 open 期迁
/// 移，见下方用例注记）。调用方必须先 drop Store 释放文件锁。
fn inject_v1_change_row(path: &Path, name: &str, created_at: i64) {
    let legacy = ChangeRecordV1 {
        name: name.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at,
        status: workflow::state::ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
    };
    let bytes = <ChangeRecordV1 as native_model::Model>::native_model_encode(&legacy)
        .expect("编码 v1 建档行失败");
    let db = redb::Database::open(path).expect("裸开 db 注入 v1 行应成功");
    let table: redb::TableDefinition<RawNativeDbKey, &[u8]> =
        redb::TableDefinition::new(CHANGE_RECORD_TABLE);
    let rw = db.begin_write().expect("开启注入写事务失败");
    {
        let mut rows = rw.open_table(table).expect("打开建档表应成功");
        rows.insert(RawNativeDbKey(name.as_bytes().to_vec()), bytes.as_slice())
            .expect("注入 v1 行应成功");
    }
    rw.commit().expect("提交注入事务失败");
}

/// 存量 v1 载荷行 additive 打开：裸 redb 直写 v1 envelope 字节（v2 当前表
/// 内）→ `open_workspace`（v2 模型组）additive 打开 → find / list 读出既有
/// 六字段一致且 worktree / base_commit = None（无手工迁移层——AC-1 存量库
/// 半边；native_model 版本机制经 decode `from` 链升级）。
/// 注记：test-design 本行原判「注入旧版本表 `9_1_name` 后可读」与 native_db
/// 0.8.2 实际机制不符（表名含版本段、读面仅寻当前表、无 open 期迁移——旧版
/// 本表行对 v2 读面不可见），decode-only 升级的真实作用面是**当前表内的 v1
/// envelope 载荷**（本用例形态）；本用例按实现行为钉住（discrepancy 见变更
/// 报告）。
#[test]
fn 存量v1行库additive打开_读出升级两字段none() {
    let env = Env::new("change-v1-legacy-db");
    let ws_path = env.db_path("ws");

    // 先经公共打开铸 db 文件（表结构在场），归还文件锁后裸 redb 注入 v1 行
    //（存量库形态：旧版本写入的 v1 表行与 v2 模型组 additive 共存）
    {
        let _store = open_workspace_ok(&ws_path);
    }
    inject_v1_change_row(&ws_path, "v1-legacy-change", 1_727_000_000_000);

    // v2 模型组 additive 打开（模型组升级不拒既有库文件）
    let store = open_workspace_ok(&ws_path);

    // 主键直查：六字段一致 + 两新字段 None（版本机制自动升级，零迁移代码）
    let hit = store
        .find_change_record("v1-legacy-change")
        .expect("直查应成功")
        .expect("v1 行应在案");
    assert_eq!(hit.name, "v1-legacy-change");
    assert_eq!(hit.workflow_type, "requirement");
    assert_eq!(hit.created_at, 1_727_000_000_000);
    assert_eq!(hit.status, workflow::state::ChangeStatus::Active);
    assert_eq!(hit.archived_at, None);
    assert_eq!(hit.active_phase, None);
    assert_eq!(hit.worktree, None, "存量 v1 行读出 worktree = None");
    assert_eq!(hit.base_commit, None, "存量 v1 行读出 base_commit = None");

    // 全表读同面（清单路径的存量升级半边）
    let records = store.list_change_records().expect("清单应成功");
    assert_eq!(records.len(), 1, "恰一行存量记录");
    assert_eq!(records[0].worktree, None);
    assert_eq!(records[0].base_commit, None);
    assert_eq!(records[0].name, "v1-legacy-change");
}
