use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent::{AgentEvent, AgentEventKind};
use serde_json::json;
use tauri::{App, Manager};
use tempfile::TempDir;

use super::{add_workspace, list_workspaces, remove_workspace};
use store::{AgentEngineKind, SessionConfigSnapshot, SessionRecord, WorkspaceStores};

/// 数据根 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    data_dir: TempDir,
    ws_root: TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let data_dir = tempfile::Builder::new()
            .prefix(&format!("ws-cmd-test-{tag}-data-"))
            .tempdir()
            .expect("创建数据根临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("ws-cmd-test-{tag}-root-"))
            .tempdir()
            .expect("创建 workspace 根临时目录失败");
        Self { data_dir, ws_root }
    }

    /// `workspaces/` 子树根（workspace 库文件派生落位）。
    fn workspaces_dir(&self) -> PathBuf {
        self.data_dir.path().join("workspaces")
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

/// 以 MockRuntime 建测用 app，并在其中 manage 真实 WorkspaceStores（打开 env
/// 数据根的全局库；workspace 库经 add 预开 / for_root 惰性开）。
fn app_with_stores(env: &Env) -> App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let stores = WorkspaceStores::open(env.data_dir.path()).expect("打开测试全局库失败");
    app.manage(stores);
    app
}

/// 落一份带来源三元组的会话行（会话域级联/历史断言的底座）。
fn seed_chat_session(
    store: &store::Store,
    id: &str,
    source: &str,
    source_ref: Option<&str>,
) -> String {
    store
        .create_session(&SessionRecord {
            id: id.to_owned(),
            engine_session_id: None,
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                model: Some("m-high".to_owned()),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
            },
            source: source.to_owned(),
            source_ref: source_ref.map(str::to_owned),
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("create_session 应成功");
    id.to_owned()
}

/// 一条盖戳 Raw 事件（级联与重放断言的最小载荷）。
fn raw_event(seq: u64) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::Raw {
            event_type: "mystery".to_owned(),
            raw_json: format!(r#"{{"type":"mystery","seq":{seq}}}"#),
        },
    )
}

// ---------------------------------------------------------------------------
// State 切换回归：薄包装不加工（命令面与直连 WorkspaceStores 公共 API 一致）
// ---------------------------------------------------------------------------

#[test]
fn 三命令结果与直连两级库注册表公共api结果serde一致_薄包装不加工() {
    let env = Env::new("passthrough");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let rec_b = add_workspace(state.clone(), env.root_of("cmd-b")).expect("add b 应成功");
    let rec_a = add_workspace(state.clone(), env.root_of("cmd-a")).expect("add a 应成功");

    // list：命令面与直连 WorkspaceStores 公共 API 的 serde 值一致（同库同状态）
    let via_command = list_workspaces(state.clone()).expect("list 应成功");
    let via_stores = state.global().list_workspaces().expect("直连 list 应成功");
    assert_eq!(
        serde_json::to_value(&via_command).unwrap(),
        serde_json::to_value(&via_stores).unwrap()
    );
    // add：命令返回值即落库记录（root / name 字段直比；时间戳由 list 一致性间接锁定）
    let roots: Vec<String> = via_command.iter().map(|r| r.root.clone()).collect();
    assert_eq!(
        roots,
        vec![rec_a.root.clone(), rec_b.root],
        "命令面与直连的默认序（主键自然序）一致"
    );
    let stored_a = via_command.iter().find(|r| r.root == rec_a.root).unwrap();
    assert_eq!(stored_a.name, rec_a.name);
}

#[test]
fn add_workspace返回值序列化顶层键恰为三字段驼峰命名无redb概念泄漏() {
    let env = Env::new("serde-shape");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let record = add_workspace(state.clone(), env.root_of("shape")).expect("add 应成功");

    let value = serde_json::to_value(&record).expect("序列化失败");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("顶层为对象")
        .keys()
        .map(|k| k.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["addedAt", "name", "root"]);
    assert_eq!(value["name"], json!("shape"));
}

#[test]
fn add传入书写不等价string返回root与库内canonical_key同源() {
    let env = Env::new("canonical-same");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

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
fn 未注册root的remove经命令面返回false幂等() {
    let env = Env::new("cmd-miss");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let registered = add_workspace(state.clone(), env.root_of("registered")).expect("add 应成功");
    let ghost = env.root_of("ghost"); // 存在但未注册

    assert!(!remove_workspace(state.clone(), ghost).unwrap());
    assert_eq!(
        list_workspaces(state.clone()).unwrap(),
        vec![registered],
        "miss 后库内容不变"
    );
}

#[test]
fn add传入不存在的目录返回err且含canonicalize前缀() {
    let env = Env::new("cmd-missing");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
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
fn 空字符串root时add为err而remove为ok_false两命令语义一致() {
    let env = Env::new("cmd-empty");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let registered = add_workspace(state.clone(), env.root_of("registered")).expect("add 应成功");

    let add_err = add_workspace(state.clone(), String::new()).expect_err("空 root add 应 Err");
    assert!(add_err.starts_with("canonicalize:"), "实际: {add_err}");
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
// 注册表操作 → 全局库 + 预开校验
// ---------------------------------------------------------------------------

#[test]
fn add_workspace后注册记录落全局库且对应workspace库文件已创建() {
    let env = Env::new("preopen-hit");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let record = add_workspace(state.clone(), env.root_of("preopen")).expect("add 应成功");

    assert_eq!(
        list_workspaces(state.clone()).unwrap(),
        vec![record],
        "注册记录落全局库"
    );
    assert_eq!(
        env.workspace_db_files().len(),
        1,
        "注册成功后预开对应 workspace 库（文件落 workspaces/ 子树）"
    );
}

#[test]
fn workspace库文件损坏时add_workspace返回err且注册记录保留() {
    let env = Env::new("preopen-corrupt");
    let root;
    {
        let app = app_with_stores(&env);
        let state = app.state::<WorkspaceStores>();
        root = env.root_of("victim");
        add_workspace(state.clone(), root.clone()).expect("首次 add 应成功");
        // tauri 托管值生命周期长于 App（redb 文件锁不随 App drop 释放）：
        // 显式取回注册表销毁，损坏文件才能覆写
        #[allow(deprecated)]
        let _stores = app.unmanage::<WorkspaceStores>().expect("应处于托管中");
    }
    let file_path = env
        .workspaces_dir()
        .join(env.workspace_db_files()[0].as_str());
    fs::write(
        &file_path,
        "损坏的非法字节序列，不是合法 db 文件。".repeat(32),
    )
    .expect("写损坏文件失败");

    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let err = add_workspace(state.clone(), root.clone())
        .expect_err("坏文件注册时以 Err 暴露（预开校验 fail fast）");
    assert!(!err.is_empty(), "错误串可直抵前端，实际: {err}");

    // 先注册后预开：注册记录保留（重加同 root upsert 幂等并再次校验）
    let list = list_workspaces(state.clone()).unwrap();
    assert_eq!(list.len(), 1, "注册记录保留");
    assert_eq!(list[0].root, root, "root 为 canonical key");
}

// ---------------------------------------------------------------------------
// 集成：tempdir 全链路经命令面（add → list → remove → 重开）与等价路径命中
// ---------------------------------------------------------------------------

#[test]
fn 空库经命令面list返回空数组() {
    let env = Env::new("int-empty");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();

    let list = list_workspaces(state.clone()).expect("list 应成功");

    assert_eq!(serde_json::to_value(&list).unwrap(), json!([]));
}

#[test]
fn 经命令面add_list顺序与直连一致_删除与重开后状态经命令面可复现() {
    let env = Env::new("int-full");
    let rec_a;
    let rec_c;
    {
        let app = app_with_stores(&env);
        let state = app.state::<WorkspaceStores>();

        rec_c = add_workspace(state.clone(), env.root_of("cmd-c")).expect("add c 应成功");
        let rec_b = add_workspace(state.clone(), env.root_of("cmd-b")).expect("add b 应成功");
        rec_a = add_workspace(state.clone(), env.root_of("cmd-a")).expect("add a 应成功");

        let roots: Vec<String> = list_workspaces(state.clone())
            .unwrap()
            .iter()
            .map(|r| r.root.clone())
            .collect();
        assert_eq!(
            roots,
            vec![rec_a.root.clone(), rec_b.root.clone(), rec_c.root.clone()],
            "默认序（主键自然序）与添加顺序无关"
        );

        // remove 命中：true 且清单不含该项
        assert!(remove_workspace(state.clone(), rec_b.root.clone()).unwrap());
        let after_remove: Vec<String> = list_workspaces(state.clone())
            .unwrap()
            .iter()
            .map(|r| r.root.clone())
            .collect();
        assert_eq!(after_remove, vec![rec_a.root.clone(), rec_c.root.clone()]);
        // 显式取回注册表所有权并释放：App 内部 Arc 环不保证同步 drop，全局库文件
        // 锁须显式还（沿既有 unmanage 惯例）。
        #[allow(deprecated)]
        let _stores = app.unmanage::<WorkspaceStores>().expect("应已 manage");
    }

    // WorkspaceStores 已释放文件锁，重开同一数据根再经命令面 list：结果与删除
    // 后状态一致
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
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
fn 等价路径经命令面remove命中同一条无孤儿条目() {
    let env = Env::new("int-equivalent");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let raw = env.root_of("CmdDedup");
    let record = add_workspace(state.clone(), raw).expect("add 应成功");

    // 正斜杠书写形态 remove：命中
    let forward = record.root.replace('\\', "/");
    assert!(
        remove_workspace(state.clone(), forward).unwrap(),
        "等价路径 remove 命中"
    );

    let remaining = list_workspaces(state.clone()).unwrap();
    assert!(remaining.is_empty(), "无孤儿条目");
}

#[test]
fn 大小写不同string二次add经命令面仅一条且原记录原样返回() {
    let env = Env::new("int-case");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let raw = env.root_of("CaseAdd");
    let first = add_workspace(state.clone(), raw).expect("首次 add 应成功");

    let flipped = Path::new(&first.root)
        .with_file_name("cASEaDD")
        .to_string_lossy()
        .into_owned();
    let second = add_workspace(state.clone(), flipped).expect("二次 add 应成功");

    assert_eq!(second, first, "原记录原样返回");
    let list = list_workspaces(state.clone()).unwrap();
    assert_eq!(list.len(), 1, "仅一条记录");
    assert_eq!(list[0].added_at, first.added_at, "added_at 保留首添值");
}

// ---------------------------------------------------------------------------
// AC-7 组合：workspace命令面 → WorkspaceStores 缓存复用 → workspace 库历史保留
// ---------------------------------------------------------------------------

#[test]
fn remove仅删注册记录_重加同root后explore与会话链历史完整可读() {
    let env = Env::new("ac7-history");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("history");

    let record = add_workspace(state.clone(), root.clone()).expect("add 应成功");
    // 预开的 workspace 库种数据：explore 记录 + 会话链 runs（含事件）
    {
        let ws = state.for_root(&record.root).expect("预开实例应可解析");
        let explore = ws
            .create_explore_record(&record.root, "history-topic")
            .expect("建档应成功");
        let session_id = seed_chat_session(
            &ws,
            "ses-history-bound",
            "explore",
            Some(&explore.id.to_string()),
        );
        ws.append_session_events(&session_id, &[raw_event(0), raw_event(1)])
            .expect("append 应成功");
    }

    // remove：仅删注册记录，db 文件与缓存实例保留
    assert!(remove_workspace(state.clone(), record.root.clone()).unwrap());
    assert!(
        list_workspaces(state.clone()).unwrap().is_empty(),
        "注册记录消失"
    );
    assert_eq!(
        env.workspace_db_files().len(),
        1,
        "workspace db 文件仍在磁盘"
    );

    // 重新 add 同 root：explore 清单与会话链历史完整可读
    let readded = add_workspace(state.clone(), root).expect("重加应成功");
    assert_eq!(readded.root, record.root, "重加为同一 canonical root");
    let ws = state.for_root(&readded.root).expect("for_root 应成功");
    let explores = ws.list_explore_records(&readded.root).unwrap();
    assert_eq!(explores.len(), 1, "explore 清单历史完整");
    assert_eq!(explores[0].name, "history-topic");
    let summaries = ws
        .list_sessions(Some("explore"), Some(&explores[0].id.to_string()))
        .unwrap();
    assert_eq!(summaries.len(), 1, "归属会话历史完整");
    assert_eq!(
        summaries[0].row.provenance.source_ref.as_deref(),
        Some(explores[0].id.to_string().as_str()),
        "链绑定（source_ref）跨 remove-readd 保持"
    );
    assert_eq!(
        ws.list_session_events("ses-history-bound").unwrap().len(),
        2,
        "转录历史完整可读"
    );
}

#[test]
fn remove后db文件名与字节保持原样不自动清理() {
    let env = Env::new("ac7-bytes");
    // 阶段一：注册 + 种数据（文件持锁期间不做外部字节读取——redb 独占字节锁）
    let root;
    let file_name;
    {
        let app = app_with_stores(&env);
        let state = app.state::<WorkspaceStores>();
        root = env.root_of("keep-bytes");
        let record = add_workspace(state.clone(), root.clone()).expect("add 应成功");
        let ws = state.for_root(&record.root).expect("for_root 应成功");
        seed_chat_session(&ws, "ses-keep-bytes", "debug", None);
        file_name = env.workspace_db_files()[0].clone();
        // tauri 托管值生命周期长于 App（redb 文件锁不随 App drop 释放）：
        // 显式取回注册表销毁，db 文件字节才能被外部读取
        #[allow(deprecated)]
        let _stores = app.unmanage::<WorkspaceStores>().expect("应处于托管中");
    }
    let file_path = env.workspaces_dir().join(&file_name);
    let bytes_before = fs::read(&file_path).expect("读取 db 文件失败");

    // 阶段二：remove 仅删注册记录（本 app 未开 workspace 库，文件不持锁）
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    assert!(remove_workspace(state.clone(), root).unwrap());

    assert_eq!(
        env.workspace_db_files(),
        vec![file_name],
        "派生路径上 db 文件名保持原样（不自动清理）"
    );
    assert_eq!(
        fs::read(&file_path).expect("读取 db 文件失败"),
        bytes_before,
        "db 文件字节保持原样"
    );
}

#[test]
fn remove不驱逐缓存_重加同root经缓存实例即读即得() {
    let env = Env::new("ac7-cache");
    let app = app_with_stores(&env);
    let state = app.state::<WorkspaceStores>();
    let root = env.root_of("keep-cache");

    let record = add_workspace(state.clone(), root).expect("add 应成功");
    let arc_before = state.for_root(&record.root).expect("for_root 应成功");
    // 缓存实例内写入一笔数据
    seed_chat_session(&arc_before, "ses-keep-cache", "debug", None);

    // remove：注册记录消失，缓存实例保留
    assert!(remove_workspace(state.clone(), record.root.clone()).unwrap());
    assert!(list_workspaces(state.clone()).unwrap().is_empty());

    // 重加同 root：命中同一缓存实例（无二次文件打开），实例内数据即读即得
    add_workspace(state.clone(), record.root.clone()).expect("重加应成功");
    let arc_after = state.for_root(&record.root).expect("for_root 应成功");
    assert!(
        Arc::ptr_eq(&arc_before, &arc_after),
        "重加后命中同一缓存实例（remove 不驱逐缓存）"
    );
    assert_eq!(
        arc_after.list_sessions(None, None).unwrap().len(),
        1,
        "缓存实例数据即读即得（remove-readd 不清空 workspace 库）"
    );
}
