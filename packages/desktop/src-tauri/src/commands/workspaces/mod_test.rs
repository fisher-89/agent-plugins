//! `commands::workspaces` 的单元测试 + 「workspace命令面 → Store持久化」集成关系
//! （dev-team 为纯 binary crate，无库目标，集成用例按仓库既有模式与本文件共置）。
//!
//! 四命令为薄包装（State 取 store + String→Path 参数转换 + StoreError→Err(String)
//! 映射）：`#[tauri::command]` 保留原函数可直调，测试不启动真实 Tauri runtime——
//! 以 `tauri::test::mock_app()`（MockRuntime，无窗口无事件循环）manage 真实
//! Store 后经 `app.state::<Store>()` 取 State。tempdir 真开 redb 文件。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use tauri::{App, Manager};
use tempfile::TempDir;

use super::{add_workspace, list_workspaces, remove_workspace, touch_workspace};
use store::Store;

/// db 文件 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    db_dir: TempDir,
    ws_root: TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("ws-cmd-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("ws-cmd-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { db_dir, ws_root }
    }

    /// 默认 db 文件路径（固定文件名，多次调用同值，支撑重开场景）。
    fn db_path(&self) -> PathBuf {
        self.db_dir.path().join("test.redb")
    }

    /// 在 workspace 根下创建一个真实目录并返回路径。
    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }

    /// 目录路径转命令面 String root。
    fn root_of(&self, name: &str) -> String {
        self.ws(name).to_string_lossy().into_owned()
    }
}

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 Store（打开 env 的 db 文件）。
fn app_with_store(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let store = Store::open(&env.db_path()).expect("打开测试 db 失败");
    app.manage(store);
    app
}

// ---------------------------------------------------------------------------
// 单元：薄包装不加工 DTO（AC-7）与错误约定（AC-3/D1/D8）
// ---------------------------------------------------------------------------

#[test]
fn 四命令结果与直连store公共api结果serde一致_薄包装不加工() {
    let env = Env::new("passthrough");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let rec_a = add_workspace(state.clone(), env.root_of("cmd-a")).expect("add a 应成功");
    sleep_millis(2);
    let rec_b = add_workspace(state.clone(), env.root_of("cmd-b")).expect("add b 应成功");
    sleep_millis(2);
    assert!(
        touch_workspace(state.clone(), rec_a.root.clone()).unwrap(),
        "touch a 命中"
    );

    // list：命令面与直连 store 公共 API 的 serde 值一致（同库同状态）
    let via_command = list_workspaces(state.clone()).expect("list 应成功");
    let via_store = state.list_workspaces().expect("直连 list 应成功");
    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_store).unwrap()
    );
    // add：命令返回值即落库记录（root / name 字段直比；时间戳由 list 一致性间接锁定）
    let roots: Vec<String> = via_command.iter().map(|r| r.root.clone()).collect();
    assert_eq!(
        roots,
        vec![rec_a.root.clone(), rec_b.root],
        "命令面 touch 后排序一致"
    );
    let stored_a = via_command.iter().find(|r| r.root == rec_a.root).unwrap();
    assert_eq!(stored_a.name, rec_a.name);
}

#[test]
fn add_workspace返回值序列化顶层键恰为四字段驼峰命名无redb概念泄漏() {
    let env = Env::new("serde-shape");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let record = add_workspace(state.clone(), env.root_of("shape")).expect("add 应成功");

    let value = serde_json::to_value(&record).expect("序列化失败");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("顶层为对象")
        .keys()
        .map(|k| k.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["addedAt", "lastOpenedAt", "name", "root"]);
    assert_eq!(value["name"], json!("shape"));
}

#[test]
fn add传入书写不等价string返回root与库内canonical_key同源() {
    let env = Env::new("canonical-same");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let raw = env.root_of("DedupMe");
    let first = add_workspace(state.clone(), raw).expect("首次 add 应成功");

    // 大小写不同、指向同一目录的书写形态
    let flipped = Path::new(&first.root)
        .with_file_name("dEDUPmE")
        .to_string_lossy()
        .into_owned();
    let second = add_workspace(state.clone(), flipped).expect("二次 add 应成功");

    assert_eq!(
        second.root, first.root,
        "返回 root 与库内 canonical key 同源"
    );
    assert_eq!(
        list_workspaces(state.clone()).unwrap().len(),
        1,
        "清单仅一条"
    );
}

#[test]
fn 未注册root的remove_touch经命令面返回false幂等() {
    let env = Env::new("cmd-miss");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let registered = add_workspace(state.clone(), env.root_of("registered")).expect("add 应成功");
    let ghost = env.root_of("ghost"); // 存在但未注册

    assert!(!remove_workspace(state.clone(), ghost.clone()).unwrap());
    assert!(!touch_workspace(state.clone(), ghost).unwrap());
    assert_eq!(
        list_workspaces(state.clone()).unwrap(),
        vec![registered],
        "miss 后库内容不变"
    );
}

#[test]
fn add传入不存在的目录返回err且含canonicalize前缀() {
    let env = Env::new("cmd-missing");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let missing = env
        .ws_root
        .path()
        .join("no-such-dir")
        .to_string_lossy()
        .into_owned();

    let err = add_workspace(state.clone(), missing).expect_err("不存在的目录应 Err");

    assert!(
        err.starts_with("canonicalize:"),
        "错误约定前缀保真，实际: {err}"
    );
}

#[test]
fn 空字符串root时add为err而touch_remove为ok_false三命令语义一致() {
    let env = Env::new("cmd-empty");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let registered = add_workspace(state.clone(), env.root_of("registered")).expect("add 应成功");

    let add_err = add_workspace(state.clone(), String::new()).expect_err("空 root add 应 Err");
    assert!(add_err.starts_with("canonicalize:"), "实际: {add_err}");
    assert!(
        !touch_workspace(state.clone(), String::new()).unwrap(),
        "空 root touch 幂等 miss"
    );
    assert!(
        !remove_workspace(state.clone(), String::new()).unwrap(),
        "空 root remove 幂等 miss"
    );
    assert_eq!(
        list_workspaces(state.clone()).unwrap(),
        vec![registered],
        "库内容不变"
    );
}

// ---------------------------------------------------------------------------
// 集成：tempdir 全链路经命令面（add → touch → list → remove → 重开）与等价路径命中
// ---------------------------------------------------------------------------

#[test]
fn 空库经命令面list返回空数组() {
    let env = Env::new("int-empty");
    let app = app_with_store(&env);
    let state = app.state::<Store>();

    let list = list_workspaces(state.clone()).expect("list 应成功");

    assert_eq!(serde_json::to_value(&list).unwrap(), json!([]));
}

#[test]
fn 经命令面add_touch_list顺序与直连store一致_删除与重开后状态经命令面可复现() {
    let env = Env::new("int-full");
    let rec_a;
    let rec_c;
    {
        let app = app_with_store(&env);
        let state = app.state::<Store>();

        rec_a = add_workspace(state.clone(), env.root_of("cmd-a")).expect("add a 应成功");
        sleep_millis(2);
        let rec_b = add_workspace(state.clone(), env.root_of("cmd-b")).expect("add b 应成功");
        sleep_millis(2);
        rec_c = add_workspace(state.clone(), env.root_of("cmd-c")).expect("add c 应成功");
        sleep_millis(2);
        assert!(touch_workspace(state.clone(), rec_a.root.clone()).unwrap());

        let roots: Vec<String> = list_workspaces(state.clone())
            .unwrap()
            .iter()
            .map(|r| r.root.clone())
            .collect();
        assert_eq!(
            roots,
            vec![rec_a.root.clone(), rec_c.root.clone(), rec_b.root.clone()],
            "降序与并列规则一致"
        );

        // remove 命中：true 且清单不含该项
        assert!(remove_workspace(state.clone(), rec_b.root.clone()).unwrap());
        let after_remove: Vec<String> = list_workspaces(state.clone())
            .unwrap()
            .iter()
            .map(|r| r.root.clone())
            .collect();
        assert_eq!(after_remove, vec![rec_a.root.clone(), rec_c.root.clone()]);
        // 显式取回 Store 所有权并释放：App 内部 Arc 环不保证同步 drop，redb 文件锁须显式还。
        // 安全性：唯一引用 state 已先行 drop，且测试单线程，unmanage 后无悬垂引用。
        drop(state);
        #[allow(deprecated)]
        let _store = app.unmanage::<Store>().expect("store 应已 manage");
    }

    // Store 已释放文件锁，重开同一 db 文件再经命令面 list：结果与删除后状态一致
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let after_reopen: Vec<String> = list_workspaces(state.clone())
        .unwrap()
        .iter()
        .map(|r| r.root.clone())
        .collect();
    assert_eq!(after_reopen, vec![rec_a.root, rec_c.root]);
    // 记录字段持久化无损
    assert_eq!(list_workspaces(state.clone()).unwrap()[0].name, rec_a.name);
}

#[test]
fn 等价路径经命令面touch_remove均命中同一条无孤儿条目() {
    let env = Env::new("int-equivalent");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let raw = env.root_of("CmdDedup");
    let record = add_workspace(state.clone(), raw).expect("add 应成功");

    // 尾分隔符书写形态 touch：命中
    let with_trailing = format!("{}\\", record.root);
    assert!(
        touch_workspace(state.clone(), with_trailing).unwrap(),
        "等价路径 touch 命中"
    );

    // 正斜杠书写形态 remove：命中同一条
    let forward = record.root.replace('\\', "/");
    assert!(
        remove_workspace(state.clone(), forward).unwrap(),
        "等价路径 remove 命中"
    );

    let remaining = list_workspaces(state.clone()).unwrap();
    assert!(remaining.is_empty(), "无孤儿条目");
}

#[test]
fn 大小写不同string二次add经命令面仅一条且时间戳刷新() {
    let env = Env::new("int-case");
    let app = app_with_store(&env);
    let state = app.state::<Store>();
    let raw = env.root_of("CaseAdd");
    let first = add_workspace(state.clone(), raw).expect("首次 add 应成功");

    sleep_millis(5);
    let flipped = Path::new(&first.root)
        .with_file_name("cASEaDD")
        .to_string_lossy()
        .into_owned();
    let second = add_workspace(state.clone(), flipped).expect("二次 add 应成功");

    assert_eq!(second.root, first.root);
    let list = list_workspaces(state.clone()).unwrap();
    assert_eq!(list.len(), 1, "仅一条记录");
    assert_eq!(list[0].added_at, first.added_at, "added_at 保留首添值");
    assert!(
        list[0].last_opened_at > first.last_opened_at,
        "last_opened_at 已刷新"
    );
}

/// 毫秒级 sleep：为需要严格时间差的断言拉开 wall clock（时钟不 mock）。
fn sleep_millis(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
