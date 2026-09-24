//! `envelope`（信封 API 实现体）的单元测试：注册表结构、scan 分页边界矩阵、
//! key/value 信封形态（JSON 口径、u128 键可表达性）、未知模型名异常面（AC-5）。
//!
//! 存储层不 mock：tempfile 真开库并经 `Store` 公共 API 写入构造数据（信封
//! 实现体经 `Store::list_models` / `Store::scan` 委托触达）。

use std::fs;
use std::path::{Path, PathBuf};

use agent::{AgentEvent, AgentEventKind};

use crate::{AgentRunRecord, Store, StoreError};

/// db 文件 + workspace 根目录临时环境：tempfile RAII，测试结束自动清理。
struct Env {
    db_dir: tempfile::TempDir,
    ws_root: tempfile::TempDir,
}

impl Env {
    fn new(tag: &str) -> Self {
        let db_dir = tempfile::Builder::new()
            .prefix(&format!("envelope-test-{tag}-db-"))
            .tempdir()
            .expect("创建 db 临时目录失败");
        let ws_root = tempfile::Builder::new()
            .prefix(&format!("envelope-test-{tag}-ws-"))
            .tempdir()
            .expect("创建 workspace 临时目录失败");
        Self { db_dir, ws_root }
    }

    fn db_path(&self) -> PathBuf {
        self.db_dir.path().join("test.redb")
    }

    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }
}

fn open_ok(path: &Path) -> Store {
    Store::open(path).unwrap_or_else(|e| panic!("open 应成功: {e}"))
}

fn add_ok(store: &Store, dir: &Path) -> crate::WorkspaceRecord {
    store
        .add_workspace(dir)
        .unwrap_or_else(|e| panic!("add_workspace 应成功: {e}"))
}

fn begin_run(store: &Store, prompt: &str, started_at: i64) -> crate::AgentRunRecord {
    store
        .begin_agent_run(&AgentRunRecord {
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
        })
        .unwrap_or_else(|e| panic!("begin_agent_run 应成功: {e}"))
}

fn append_raw(store: &Store, run_id: i64, seq: u64) {
    let event = AgentEvent::stamp(
        seq,
        AgentEventKind::Raw {
            event_type: "mystery".to_owned(),
            raw_json: format!(r#"{{"type":"mystery","seq":{seq}}}"#),
        },
    );
    store
        .append_agent_run_events(run_id, &[event])
        .unwrap_or_else(|e| panic!("append 应成功: {e}"));
}

/// 断言 scan 的 Err 面并返回错误串（未知模型名语境）。
fn scan_err(store: &Store, model: &str) -> String {
    match store.scan(model, 0, 10) {
        Err(err @ StoreError::Db(_)) => err.to_string(),
        Err(other) => panic!("模型 {model:?} 应返回 Db 变体，实际: {other:?}"),
        Ok(page) => panic!("模型 {model:?} 应 Err，实际返回 {} 行", page.len()),
    }
}

// ---------------------------------------------------------------------------
// 注册表：三模型全列、计数一致
// ---------------------------------------------------------------------------

#[test]
fn 注册表覆盖三个已注册模型且list_models计数与写入量一致() {
    let env = Env::new("registry");
    let store = open_ok(&env.db_path());
    add_ok(&store, &env.ws("one"));
    let run = begin_run(&store, "注册表复核", 100);
    append_raw(&store, run.id, 0);

    let models = store.list_models().unwrap();

    assert_eq!(
        models
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace", "agent_run", "agent_event"],
        "静态注册表恰三行，顺序即登记序"
    );
    let counts: Vec<u64> = models.iter().map(|model| model.count).collect();
    assert_eq!(counts, vec![1, 1, 1], "计数与各模型写入量一致");
}

// ---------------------------------------------------------------------------
// scan 分页边界矩阵（以 agent_run 载荷承载：begin 分配 id 无目录依赖）
// ---------------------------------------------------------------------------

#[test]
fn scan_offset首页恰为总数与超过总数三种形态返回正确() {
    let env = Env::new("scan-offset");
    let store = open_ok(&env.db_path());
    for index in 0..3 {
        begin_run(&store, &format!("run-{index}"), 100 + index);
    }

    let ids = |offset: u32, limit: u32| -> Vec<i64> {
        store
            .scan("agent_run", offset, limit)
            .unwrap()
            .iter()
            .map(|envelope| envelope.key.as_i64().expect("agent_run key 为数值"))
            .collect()
    };

    assert_eq!(ids(0, 10), vec![1, 2, 3], "offset=0 首页全量");
    assert_eq!(
        ids(3, 10),
        Vec::<i64>::new(),
        "offset 恰等于记录总数返回空页"
    );
    assert_eq!(
        ids(u32::MAX, 10),
        Vec::<i64>::new(),
        "offset 超过记录总数（u32::MAX）返回空数组不报错"
    );
}

#[test]
fn scan_limit为零返回空大limit返回剩余全部() {
    let env = Env::new("scan-limit");
    let store = open_ok(&env.db_path());
    for index in 0..3 {
        begin_run(&store, &format!("run-{index}"), 100 + index);
    }

    let len = |offset: u32, limit: u32| store.scan("agent_run", offset, limit).unwrap().len();

    assert_eq!(len(0, 0), 0, "limit=0 返回空数组");
    assert_eq!(len(1, 100), 2, "limit 大于剩余记录数返回剩余全部");
    assert_eq!(len(0, 2), 2, "limit 小于总数返回恰 limit 条");
}

#[test]
fn scan_limit超过500截断为500上限语义() {
    let env = Env::new("scan-limit-cap");
    let store = open_ok(&env.db_path());
    // 505 条 > 上限 500：截断语义与「恰好 500 条」歧义区分
    for index in 0..505 {
        begin_run(&store, &format!("run-{index}"), index);
    }

    let first_page = store.scan("agent_run", 0, 5000).unwrap();
    assert_eq!(first_page.len(), 500, "limit 超上限截断为 500");
    let rest = store.scan("agent_run", 500, 5000).unwrap();
    assert_eq!(rest.len(), 5, "截断后剩余记录可经 offset 续读");
    let mut union: Vec<i64> = first_page
        .iter()
        .chain(rest.iter())
        .map(|envelope| envelope.key.as_i64().unwrap())
        .collect();
    union.sort_unstable();
    assert_eq!(
        union,
        (1..=505).collect::<Vec<i64>>(),
        "两页拼接覆盖全部 505 条不重不漏"
    );
}

// ---------------------------------------------------------------------------
// 信封形态：key 口径与 value JSON 可读性（native_db 类型不越信封）
// ---------------------------------------------------------------------------

#[test]
fn scan_key信封三模型各自口径workspace根串run数值event为run与seq对象() {
    let env = Env::new("envelope-keys");
    let store = open_ok(&env.db_path());
    let workspace = add_ok(&store, &env.ws("keyed"));
    let run = begin_run(&store, "键口径", 100);
    append_raw(&store, run.id, 7);

    // workspace → root 字符串
    let ws_page = store.scan("workspace", 0, 10).unwrap();
    assert_eq!(ws_page.len(), 1);
    assert_eq!(
        ws_page[0].key,
        serde_json::json!(workspace.root),
        "workspace key 信封为 canonical root 字符串"
    );

    // agent_run → id 数值
    let run_page = store.scan("agent_run", 0, 10).unwrap();
    assert_eq!(run_page.len(), 1);
    assert_eq!(
        run_page[0].key,
        serde_json::json!(run.id),
        "agent_run key 信封为 id 数值"
    );

    // agent_event → {runId, seq} JSON 形态（u128 打包键的可读投影）
    let event_page = store.scan("agent_event", 0, 10).unwrap();
    assert_eq!(event_page.len(), 1);
    assert_eq!(
        event_page[0].key,
        serde_json::json!({ "runId": run.id, "seq": 7 }),
        "agent_event key 信封为 runId/seq 对象形态"
    );
}

#[test]
fn scan_event_value信封u128打包键为十六进制字符串合法json可还原() {
    let env = Env::new("envelope-u128");
    let store = open_ok(&env.db_path());
    let run = begin_run(&store, "u128 键", 100);
    append_raw(&store, run.id, 3);

    let page = store.scan("agent_event", 0, 10).unwrap();
    assert_eq!(page.len(), 1);

    let value = &page[0].value;
    let event_key_text = value["eventKey"].as_str().expect("eventKey 为字符串");
    assert!(
        event_key_text.starts_with("0x"),
        "u128 打包键序列化为十六进制字符串（合法 JSON 形态）: {event_key_text}"
    );
    let digits = event_key_text.trim_start_matches("0x");
    let packed = u128::from_str_radix(digits, 16).expect("十六进制可解析");
    assert_eq!(
        packed,
        ((run.id as u128) << 64) | 3u128,
        "信封键还原恰为 (run_id << 64) | seq 打包值"
    );
    assert_eq!(value["runId"], serde_json::json!(run.id));
    assert_eq!(value["event"]["seq"], serde_json::json!(3));
}

#[test]
fn scan_value信封为小驼峰结构化json人可读无二进制泄漏() {
    let env = Env::new("envelope-readable");
    let store = open_ok(&env.db_path());
    let workspace = add_ok(&store, &env.ws("readable"));
    let run = begin_run(&store, "可读性", 1727000000000);
    append_raw(&store, run.id, 0);

    // workspace value：三字段 camelCase 且文本可读（无 bincode 字节串）
    let ws_value = &store.scan("workspace", 0, 10).unwrap()[0].value;
    let mut ws_keys: Vec<&str> = ws_value
        .as_object()
        .expect("value 为 JSON 对象")
        .keys()
        .map(String::as_str)
        .collect();
    ws_keys.sort_unstable();
    assert_eq!(ws_keys, vec!["addedAt", "name", "root"], "camelCase 字段面");
    assert_eq!(ws_value["root"], serde_json::json!(workspace.root));

    // agent_run value：camelCase 汇总字段面
    let run_value = &store.scan("agent_run", 0, 10).unwrap()[0].value;
    assert_eq!(run_value["prompt"], serde_json::json!("可读性"));
    assert_eq!(run_value["startedAt"], serde_json::json!(1727000000000i64));
    assert_eq!(
        run_value["permissionMode"],
        serde_json::json!("bypassPermissions")
    );

    // agent_event value：嵌装载荷结构化呈现
    let event_value = &store.scan("agent_event", 0, 10).unwrap()[0].value;
    assert_eq!(
        event_value["event"]["kind"],
        serde_json::json!("raw"),
        "嵌装载荷以结构化 JSON 呈现（tag 在位）"
    );
    assert_eq!(
        event_value["event"]["rawJson"],
        serde_json::json!(r#"{"type":"mystery","seq":0}"#)
    );
    // 信封值为人可读文本 JSON：序列化产物无 lossy 乱码标记、可无损再解析
    // （bincode 二进制若泄漏必然以不可解析字节串或替换符形态显现）
    for value in [ws_value, run_value, event_value] {
        let text = serde_json::to_string(value).expect("信封值可序列化");
        assert!(
            !text.contains('\u{FFFD}'),
            "无 UTF-8 lossy 乱码标记（无二进制泄漏）: {text}"
        );
        let reparsed: serde_json::Value =
            serde_json::from_str(&text).expect("产物为合法 JSON 文本");
        assert_eq!(&reparsed, value, "信封值文本往返无损");
    }
}

// ---------------------------------------------------------------------------
// scan 异常面：未知模型名 / 空串，错误串含模型名
// ---------------------------------------------------------------------------

#[test]
fn scan未知模型名与空串返回err且错误串含模型名() {
    let env = Env::new("envelope-unknown");
    let store = open_ok(&env.db_path());

    let err_named = scan_err(&store, "nope");
    assert!(err_named.contains("未知模型"), "语境前缀在位: {err_named}");
    assert!(
        err_named.contains("nope"),
        "错误串含模型名便于排查: {err_named}"
    );

    let err_empty = scan_err(&store, "");
    assert!(
        err_empty.contains("未知模型"),
        "空串同样走未知模型错误面: {err_empty}"
    );

    // 异常不产生副作用：正常模型仍可扫描
    assert!(store.scan("workspace", 0, 10).unwrap().is_empty());
}
