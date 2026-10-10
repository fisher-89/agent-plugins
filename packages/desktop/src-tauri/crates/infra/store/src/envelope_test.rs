use std::fs;
use std::path::{Path, PathBuf};

use agent::{AgentEvent, AgentEventKind};

use crate::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord,
    SessionConfigSnapshot, SessionRecord, Store, StoreError,
};
use workflow::state::{ChangeStateRecord, ChangeStatus};

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

    /// db 文件路径（tag 区分全局库 / workspace 库文件：维度在打开点锁定，
    /// 同一文件不能混开两模型组）。
    fn db_path(&self, tag: &str) -> PathBuf {
        self.db_dir.path().join(format!("{tag}.redb"))
    }

    fn ws(&self, name: &str) -> PathBuf {
        let dir = self.ws_root.path().join(name);
        fs::create_dir_all(&dir).expect("创建 workspace 目录失败");
        dir
    }
}

fn open_ws_ok(path: &Path) -> Store {
    Store::open_workspace(path).unwrap_or_else(|e| panic!("open_workspace 应成功: {e}"))
}

fn open_global_ok(path: &Path) -> Store {
    Store::open_global(path).unwrap_or_else(|e| panic!("open_global 应成功: {e}"))
}

fn add_ok(store: &Store, dir: &Path) -> crate::WorkspaceRecord {
    store
        .add_workspace(dir)
        .unwrap_or_else(|e| panic!("add_workspace 应成功: {e}"))
}

fn seed_session(store: &Store, id: &str) -> String {
    store
        .create_session(&SessionRecord {
            id: id.to_owned(),
            engine_session_id: None,
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                model: Some("m-high".to_owned()),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
            },
            source: "debug".to_owned(),
            source_ref: None,
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .unwrap_or_else(|e| panic!("create_session 应成功: {e}"));
    id.to_owned()
}

fn append_raw(store: &Store, session_id: &str, seq: u64) {
    let event = AgentEvent::stamp(
        seq,
        AgentEventKind::Raw {
            event_type: "mystery".to_owned(),
            raw_json: format!(r#"{{"type":"mystery","seq":{seq}}}"#),
        },
    );
    store
        .append_session_events(session_id, &[event])
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
// 注册表：按维度分组列出、计数一致
// ---------------------------------------------------------------------------

#[test]
fn 注册表按维度分组列出模型且list_models计数与写入量一致() {
    let env = Env::new("registry");

    // 全局库：三模型组（workspace 注册表 + agent 管理两模型），计数与写入量一致
    let global = open_global_ok(&env.db_path("global"));
    add_ok(&global, &env.ws("one"));
    let global_models = global.list_models().unwrap();
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
        "全局库静态注册表为三模型组（agent 管理两行计数 0 也列出），计数与写入量一致"
    );
    drop(global);

    // workspace 库：八行按登记序，计数与各模型写入量一致（轮统计行 / explore
    // 与 change 流程状态四模型计数 0 也列出；agent_event 退役出注册）
    let ws = open_ws_ok(&env.db_path("ws"));
    let session_id = seed_session(&ws, "ses-registry");
    append_raw(&ws, &session_id, 0);

    let models = ws.list_models().unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "agent_run",
            "session",
            "session_event",
            "explore",
            "change",
            "phase",
            "checklist_item",
            "step"
        ],
        "workspace 库静态注册表 4→8 恰八行，顺序即登记序"
    );
    let counts: Vec<u64> = models.iter().map(|model| model.count).collect();
    assert_eq!(
        counts,
        vec![0, 1, 1, 0, 0, 0, 0, 0],
        "计数与各模型写入量一致"
    );
}

// ---------------------------------------------------------------------------
// 新模型登记行（AC-11）：agent_provider / agent_instance 两信封注册行经
// Store 公共 API 触达（登记行为断言不虚构 envelope 私有条目）
// ---------------------------------------------------------------------------

fn fixture_provider(name: &str) -> AgentProviderRecord {
    AgentProviderRecord::new(
        name.to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
        None,
    )
}

fn upsert_provider_ok(store: &Store, provider: AgentProviderRecord) -> AgentProviderRecord {
    store
        .upsert_agent_provider(provider)
        .unwrap_or_else(|e| panic!("upsert_agent_provider 应成功: {e}"))
}

fn upsert_agent_ok(store: &Store, agent: AgentInstanceRecord) -> AgentInstanceRecord {
    store
        .upsert_agent_instance(agent)
        .unwrap_or_else(|e| panic!("upsert_agent_instance 应成功: {e}"))
}

#[test]
fn open_global后list_models出现两新模型行_写入后计数与实有记录数一致() {
    let env = Env::new("envelope-new-entries");

    // 空库计数 0 也列出（信封 API 签名零变化：list_models 形参面与返回形态不变，
    // 仅注册表数据行追加）
    let global = open_global_ok(&env.db_path("global"));
    let names: Vec<String> = global
        .list_models()
        .unwrap()
        .into_iter()
        .map(|model| model.name)
        .collect();
    assert!(
        names.contains(&"agent_provider".to_owned())
            && names.contains(&"agent_instance".to_owned()),
        "open_global 后 list_models 出现 agent_provider / agent_instance 两行，实际: {names:?}"
    );

    // 写入后 list_models 计数与实有记录数一致
    let provider = upsert_provider_ok(&global, fixture_provider("信封端点"));
    let _agent = upsert_agent_ok(
        &global,
        AgentInstanceRecord::new(
            "信封实例".to_owned(),
            AgentEngineKind::Sdk,
            Some(provider.id),
        ),
    );
    let models = global.list_models().unwrap();
    let count_of = |name: &str| {
        models
            .iter()
            .find(|model| model.name == name)
            .unwrap_or_else(|| panic!("模型 {name} 应在清单中"))
            .count
    };
    assert_eq!(
        count_of("agent_provider"),
        1,
        "provider 计数与实有记录数一致"
    );
    assert_eq!(count_of("agent_instance"), 1, "agent 计数与实有记录数一致");
}

#[test]
fn scan两新模型分页主键自然序翻页不重不漏_key数值id信封value为小驼峰json() {
    let env = Env::new("envelope-new-scan");
    let global = open_global_ok(&env.db_path("global"));
    // 三 provider（乱序名写入，主键自然序断言与写入序无关）
    let seeded: Vec<AgentProviderRecord> = ["丙", "甲", "乙"]
        .iter()
        .map(|name| upsert_provider_ok(&global, fixture_provider(name)))
        .collect();
    let _agent = upsert_agent_ok(
        &global,
        AgentInstanceRecord::new("扫描实例".to_owned(), AgentEngineKind::Cli, None),
    );

    // 分页扫描：offset/limit 翻页拼接不重不漏（主键自然序）
    let page_keys = |model: &str, offset: u32, limit: u32| -> Vec<serde_json::Value> {
        global
            .scan(model, offset, limit)
            .unwrap_or_else(|e| panic!("scan({model}) 应成功: {e}"))
            .into_iter()
            .map(|envelope| envelope.key)
            .collect()
    };
    let mut union = page_keys("agent_provider", 0, 2);
    union.extend(page_keys("agent_provider", 2, 2));
    let ids: Vec<i64> = union
        .iter()
        .map(|key| key.as_i64().expect("key 为数值 id 信封"))
        .collect();
    assert_eq!(
        ids,
        seeded.iter().map(|record| record.id).collect::<Vec<i64>>(),
        "agent_provider 分页拼接恰为全部记录主键自然序，不重不漏"
    );

    // agent_instance 同口径（单行）
    let agent_page = global.scan("agent_instance", 0, 10).unwrap();
    assert_eq!(agent_page.len(), 1);
    assert!(
        agent_page[0].key.is_i64(),
        "agent_instance key 为数值 id 信封"
    );

    // value 为 camelCase JSON（db-inspector 查看器零改动触达前提）
    let provider_page = global.scan("agent_provider", 0, 10).unwrap();
    let value = &provider_page[0].value;
    assert_eq!(value["name"], serde_json::json!("丙"));
    assert_eq!(
        value["baseUrl"],
        serde_json::json!("https://api.example.com/v1")
    );
    assert_eq!(value["apiKey"], serde_json::json!("sk-live-1234567890"));
    assert_eq!(value["models"]["high"], serde_json::json!("m-high"));
    let agent_value = &agent_page[0].value;
    assert_eq!(agent_value["name"], serde_json::json!("扫描实例"));
    assert_eq!(agent_value["engine"], serde_json::json!("cli"));
    assert_eq!(agent_value["isDefault"], serde_json::json!(false));

    // 空页边界：offset 恰等于总数返回空数组不报错（信封 API 签名零变化）
    assert!(
        global.scan("agent_provider", 3, 10).unwrap().is_empty(),
        "offset 恰等于记录总数返回空数组"
    );
}

#[test]
fn workspace库实例scan新模型名err维度过滤_两新模型仅注册全局组() {
    let env = Env::new("envelope-dimension-filter");
    let ws = open_ws_ok(&env.db_path("ws"));

    for name in ["agent_provider", "agent_instance"] {
        let result = ws.scan(name, 0, 10);
        let err = result.expect_err("workspace 库 scan 新模型名应 Err（维度过滤）");
        assert!(matches!(err, StoreError::Db(_)), "变体为 Db，实际: {err:?}");
        assert!(
            err.to_string().contains("未知模型"),
            "错误串含「未知模型」语境，实际: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// change 信封 key 投影（AC-1 信封半边）：主键 name → id 换锚
// ---------------------------------------------------------------------------

/// 建档命令 fixture（id 主键与 name 属性双值可辨）。
fn change_archive(id: &str, name: &str) -> ChangeStateRecord {
    ChangeStateRecord {
        id: id.to_owned(),
        name: name.to_owned(),
        title: name.to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1727000000000,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    }
}

#[test]
fn scan_change信封key投影为记录id_value内name仍为属性在场() {
    let env = Env::new("envelope-change-key");
    let ws = open_ws_ok(&env.db_path("ws"));
    ws.create_change_record(change_archive("chg-envelope-1", "换锚档"))
        .unwrap_or_else(|e| panic!("create_change_record 应成功: {e}"));

    let page = ws.scan("change", 0, 10).unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(
        page[0].key,
        serde_json::json!("chg-envelope-1"),
        "change 信封 key = 记录 id（自 name 面翻转；身份锚随主键换锚）"
    );
    assert_eq!(
        page[0].value["name"],
        serde_json::json!("换锚档"),
        "value 内 name 仍为普通属性在场（非主键）"
    );
    assert_eq!(
        page[0].value["id"],
        serde_json::json!("chg-envelope-1"),
        "value 内 id 与信封 key 同源"
    );
}

// ---------------------------------------------------------------------------
// 注册表零新增行（StoreMetaRecord 不入信封注册表——无查看面需求）
// ---------------------------------------------------------------------------

#[test]
fn 注册表零新增行_workspace维度恰八行_format模型不可见且scan_err() {
    let env = Env::new("envelope-registry-zero-new");
    let ws = open_ws_ok(&env.db_path("ws"));
    // 库级格式版本标记已随打开就位（15_1_key 单键 format 行），但零呈现
    let models = ws.list_models().unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "agent_run",
            "session",
            "session_event",
            "explore",
            "change",
            "phase",
            "checklist_item",
            "step"
        ],
        "workspace 维度 list_models 行集合与计数（8 行）零变化"
    );
    assert!(
        models.iter().all(|model| model.count == 0),
        "空库计数全 0（标记行不计数——不入注册表）"
    );
    assert!(
        !models.iter().any(|model| model.name == "format"),
        "`format` 模型名不可见（内部治理记录负断言）"
    );

    // 标记模型名经信封 scan 走未知模型错误面（不入注册表即无查看面）
    for model in ["format", "store_meta", "meta"] {
        let err = scan_err(&ws, model);
        assert!(
            err.contains("未知模型"),
            "模型 {model:?} 应未知模型 Err，实际: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// scan 分页边界矩阵（以 session 载荷承载：create_session 无目录依赖）
// ---------------------------------------------------------------------------

#[test]
fn scan_offset首页恰为总数与超过总数三种形态返回正确() {
    let env = Env::new("scan-offset");
    let store = open_ws_ok(&env.db_path("ws"));
    for index in 0..3 {
        seed_session(&store, &format!("ses-offset-{index}"));
    }

    let ids = |offset: u32, limit: u32| -> Vec<String> {
        store
            .scan("session", offset, limit)
            .unwrap()
            .iter()
            .map(|envelope| {
                envelope
                    .key
                    .as_str()
                    .expect("session key 为会话 id 字符串")
                    .to_owned()
            })
            .collect()
    };

    assert_eq!(
        ids(0, 10),
        vec!["ses-offset-0", "ses-offset-1", "ses-offset-2"],
        "offset=0 首页全量"
    );
    assert_eq!(
        ids(3, 10),
        Vec::<String>::new(),
        "offset 恰等于记录总数返回空页"
    );
    assert_eq!(
        ids(u32::MAX, 10),
        Vec::<String>::new(),
        "offset 超过记录总数（u32::MAX）返回空数组不报错"
    );
}

#[test]
fn scan_limit为零返回空大limit返回剩余全部() {
    let env = Env::new("scan-limit");
    let store = open_ws_ok(&env.db_path("ws"));
    for index in 0..3 {
        seed_session(&store, &format!("ses-limit-{index}"));
    }

    let len = |offset: u32, limit: u32| store.scan("session", offset, limit).unwrap().len();

    assert_eq!(len(0, 0), 0, "limit=0 返回空数组");
    assert_eq!(len(1, 100), 2, "limit 大于剩余记录数返回剩余全部");
    assert_eq!(len(0, 2), 2, "limit 小于总数返回恰 limit 条");
}

#[test]
fn scan_limit超过500截断为500上限语义() {
    let env = Env::new("scan-limit-cap");
    let store = open_ws_ok(&env.db_path("ws"));
    // 505 条 > 上限 500：截断语义与「恰好 500 条」歧义区分
    for index in 0..505 {
        seed_session(&store, &format!("ses-cap-{index}"));
    }

    let first_page = store.scan("session", 0, 5000).unwrap();
    assert_eq!(first_page.len(), 500, "limit 超上限截断为 500");
    let rest = store.scan("session", 500, 5000).unwrap();
    assert_eq!(rest.len(), 5, "截断后剩余记录可经 offset 续读");
    let mut union: Vec<String> = first_page
        .iter()
        .chain(rest.iter())
        .map(|envelope| envelope.key.as_str().expect("session key 串").to_owned())
        .collect();
    union.sort_unstable();
    // 主键自然序为字符串字典序（session 主键即 core 铸会话 id 字符串）
    let mut expected: Vec<String> = (0..505).map(|index| format!("ses-cap-{index}")).collect();
    expected.sort_unstable();
    assert_eq!(
        union, expected,
        "两页拼接覆盖全部 505 条不重不漏（同序逐字一致）"
    );
}

// ---------------------------------------------------------------------------
// 信封形态：key 口径与 value JSON 可读性（native_db 类型不越信封）
// ---------------------------------------------------------------------------

#[test]
fn scan_key信封workspace根串run数值event对象三口径各自成立() {
    let env = Env::new("envelope-keys");

    // workspace → root 字符串（全局库）
    let global = open_global_ok(&env.db_path("global"));
    let workspace = add_ok(&global, &env.ws("keyed"));
    let ws_page = global.scan("workspace", 0, 10).unwrap();
    assert_eq!(ws_page.len(), 1);
    assert_eq!(
        ws_page[0].key,
        serde_json::json!(workspace.root),
        "workspace key 信封为 canonical root 字符串"
    );
    drop(global);

    // session → 会话 id 字符串；session_event → {sessionId, seq} JSON 形态
    //（u128 打包键的可读投影）——均在 workspace 库
    let ws = open_ws_ok(&env.db_path("ws"));
    let session_id = seed_session(&ws, "ses-keyed");
    append_raw(&ws, &session_id, 7);

    let session_page = ws.scan("session", 0, 10).unwrap();
    assert_eq!(session_page.len(), 1);
    assert_eq!(
        session_page[0].key,
        serde_json::json!(session_id),
        "session key 信封为 core 铸会话 id 字符串"
    );

    let event_page = ws.scan("session_event", 0, 10).unwrap();
    assert_eq!(event_page.len(), 1);
    assert_eq!(
        event_page[0].key,
        serde_json::json!({ "sessionId": session_id, "seq": 7 }),
        "session_event key 信封为 sessionId/seq 对象形态"
    );
}

#[test]
fn scan_event_value信封u128打包键为十六进制字符串合法json可还原() {
    let env = Env::new("envelope-u128");
    let store = open_ws_ok(&env.db_path("ws"));
    let session_id = seed_session(&store, "ses-packed");
    append_raw(&store, &session_id, 3);

    let page = store.scan("session_event", 0, 10).unwrap();
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
        packed & (u64::MAX as u128),
        3u128,
        "打包键低 64 位还原恰为 seq"
    );
    assert!(
        (packed >> 64) > 0,
        "高 64 位为 hash64(session_id)（超 u64 上界的十六进制串形态）"
    );
    assert_eq!(value["sessionId"], serde_json::json!(session_id));
    assert_eq!(value["event"]["seq"], serde_json::json!(3));
}

#[test]
fn scan_value信封为小驼峰结构化json人可读无二进制泄漏() {
    let env = Env::new("envelope-readable");

    // workspace value：三字段 camelCase 且文本可读（无 bincode 字节串）——全局库
    let global = open_global_ok(&env.db_path("global"));
    let workspace = add_ok(&global, &env.ws("readable"));
    let ws_value = &global.scan("workspace", 0, 10).unwrap()[0].value;
    let mut ws_keys: Vec<&str> = ws_value
        .as_object()
        .expect("value 为 JSON 对象")
        .keys()
        .map(String::as_str)
        .collect();
    ws_keys.sort_unstable();
    assert_eq!(ws_keys, vec!["addedAt", "name", "root"], "camelCase 字段面");
    assert_eq!(ws_value["root"], serde_json::json!(workspace.root));
    // 信封值为人可读文本 JSON：序列化产物无 lossy 乱码标记、可无损再解析
    // （bincode 二进制若泄漏必然以不可解析字节串或替换符形态显现）
    assert_readable_json(ws_value);
    drop(global);

    // session value：camelCase 快照字段面；session_event value：嵌装载荷
    // 结构化呈现——workspace 库
    let ws = open_ws_ok(&env.db_path("ws"));
    let session_id = seed_session(&ws, "ses-readable");
    append_raw(&ws, &session_id, 0);
    let session_value = &ws.scan("session", 0, 10).unwrap()[0].value;
    assert_eq!(session_value["id"], serde_json::json!(session_id));
    assert_eq!(session_value["source"], serde_json::json!("debug"));
    assert_eq!(
        session_value["configSnapshot"]["engine"],
        serde_json::json!("sdk")
    );

    let event_value = &ws.scan("session_event", 0, 10).unwrap()[0].value;
    assert_eq!(
        event_value["event"]["kind"],
        serde_json::json!("raw"),
        "嵌装载荷以结构化 JSON 呈现（tag 在位）"
    );
    assert_eq!(
        event_value["event"]["rawJson"],
        serde_json::json!(r#"{"type":"mystery","seq":0}"#)
    );
    for value in [session_value, event_value] {
        assert_readable_json(value);
    }
}

/// 信封值文本往返无损断言（无二进制泄漏的代理判据）。
fn assert_readable_json(value: &serde_json::Value) {
    let text = serde_json::to_string(value).expect("信封值可序列化");
    assert!(
        !text.contains('\u{FFFD}'),
        "无 UTF-8 lossy 乱码标记（无二进制泄漏）: {text}"
    );
    let reparsed: serde_json::Value = serde_json::from_str(&text).expect("产物为合法 JSON 文本");
    assert_eq!(&reparsed, value, "信封值文本往返无损");
}

// ---------------------------------------------------------------------------
// scan 异常面：未知模型名 / 空串，错误串含模型名
// ---------------------------------------------------------------------------

#[test]
fn scan未知模型名与空串返回err且错误串含模型名() {
    let env = Env::new("envelope-unknown");
    let store = open_ws_ok(&env.db_path("ws"));

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

    // 异常不产生副作用：本库已注册模型仍可扫描
    assert!(store.scan("session", 0, 10).unwrap().is_empty());
}
