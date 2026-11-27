use std::fs;
use std::path::{Path, PathBuf};

use agent::{AgentEvent, AgentEventKind};

use crate::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, DbDimension, RecordEnvelope, RecordsPage, SessionConfigSnapshot,
    SessionRecord, Store, StoreError,
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
                agent_instance_id: None,
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
        None,
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
        Ok(page) => panic!("模型 {model:?} 应 Err，实际返回 {} 行", page.records.len()),
    }
}

// ---------------------------------------------------------------------------
// 注册表：按维度分组列出、计数一致
// ---------------------------------------------------------------------------

#[test]
fn 注册表按维度分组列出模型且list_models计数与写入量一致() {
    let env = Env::new("registry");

    // 全局库：四模型组（workspace 注册表 + agent 管理两模型 + 通知收件箱），
    // 计数与写入量一致
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
            ("agent_instance", 0),
            ("notification", 0)
        ],
        "全局库静态注册表为四模型组（agent 管理两行与通知收件箱计数 0 也列出），计数与写入量一致"
    );
    drop(global);

    // workspace 库：十二行按登记序，计数与各模型写入量一致（轮统计行 / explore
    // 与 change 流程状态四模型 / run 执行图三模型 / 会话系统提示词快照计数 0 也
    // 列出；agent_event 退役出注册）
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
            "step",
            "run",
            "run_node",
            "run_edge",
            "session_preamble"
        ],
        "workspace 库静态注册表 8→12 恰十二行，顺序即登记序"
    );
    let counts: Vec<u64> = models.iter().map(|model| model.count).collect();
    assert_eq!(
        counts,
        vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
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
        AgentProviderProtocol::Openai,
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
        None,
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
        AgentInstanceRecord::new(
            "扫描实例".to_owned(),
            AgentEngineKind::Sdk,
            Some(seeded[0].id),
        ),
    );

    // 分页扫描：offset/limit 翻页拼接不重不漏（主键自然序）
    let page_keys = |model: &str, offset: u32, limit: u32| -> Vec<serde_json::Value> {
        global
            .scan(model, offset, limit)
            .unwrap_or_else(|e| panic!("scan({model}) 应成功: {e}"))
            .records
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

    // 各页 total 恒全量计数（分页信封随动后新模型行零漂移）
    assert_eq!(
        global.scan("agent_provider", 0, 2).unwrap().total,
        3,
        "agent_provider 各页 total 恒全量计数（非本页条数）"
    );
    assert_eq!(
        global.scan("agent_provider", 2, 2).unwrap().total,
        3,
        "agent_provider 续页 total 与首页一致（同一读事务快照口径）"
    );

    // agent_instance 同口径（单行）
    let agent_page = global.scan("agent_instance", 0, 10).unwrap();
    assert_eq!(agent_page.records.len(), 1, "agent_instance 恰一行");
    assert_eq!(agent_page.total, 1, "agent_instance total 与实有记录数一致");
    assert!(
        agent_page.records[0].key.is_i64(),
        "agent_instance key 为数值 id 信封"
    );

    // value 为 camelCase JSON（db-inspector 查看器零改动触达前提）
    let provider_page = global.scan("agent_provider", 0, 10).unwrap();
    assert_eq!(provider_page.total, 3, "provider total 与实有记录数一致");
    let value = &provider_page.records[0].value;
    assert_eq!(value["name"], serde_json::json!("丙"));
    assert_eq!(
        value["baseUrl"],
        serde_json::json!("https://api.example.com/v1")
    );
    assert_eq!(value["apiKey"], serde_json::json!("sk-live-1234567890"));
    assert_eq!(value["models"]["high"], serde_json::json!("m-high"));
    let agent_value = &agent_page.records[0].value;
    assert_eq!(agent_value["name"], serde_json::json!("扫描实例"));
    assert_eq!(agent_value["engine"], serde_json::json!("sdk"));
    assert_eq!(agent_value["isDefault"], serde_json::json!(false));

    // 空页边界：offset 恰等于总数返回空页不报错，total 恒全量（快照自洽 R2）
    let beyond = global.scan("agent_provider", 3, 10).unwrap();
    assert!(
        beyond.records.is_empty(),
        "offset 恰等于记录总数返回空 records"
    );
    assert_eq!(
        beyond.total, 3,
        "offset 恰等总数：records 空而 total 恒全量"
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
        exec_root: None,
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
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.total, 1, "change total 与实有记录数一致");
    assert_eq!(
        page.records[0].key,
        serde_json::json!("chg-envelope-1"),
        "change 信封 key = 记录 id（自 name 面翻转；身份锚随主键换锚）"
    );
    assert_eq!(
        page.records[0].value["name"],
        serde_json::json!("换锚档"),
        "value 内 name 仍为普通属性在场（非主键）"
    );
    assert_eq!(
        page.records[0].value["id"],
        serde_json::json!("chg-envelope-1"),
        "value 内 id 与信封 key 同源"
    );
}

// ---------------------------------------------------------------------------
// 注册表零新增行（StoreMetaRecord 不入信封注册表——无查看面需求）
// ---------------------------------------------------------------------------

#[test]
fn 注册表新增三run模型_workspace维度恰十二行_format模型不可见且scan_err() {
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
            "step",
            "run",
            "run_node",
            "run_edge",
            "session_preamble"
        ],
        "workspace 维度 list_models 行集合与计数（12 行：登记面 additive）"
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
// scan 分页边界矩阵（以 session 载荷承载：create_session 无目录依赖）——
// RecordsPage 两字段断言（records 区间 + total 恒全量计数，同一读事务快照）
// ---------------------------------------------------------------------------

#[test]
fn scan_offset首页恰为总数与超过总数三形态_records区间与total同快照() {
    let env = Env::new("scan-offset");
    let store = open_ws_ok(&env.db_path("ws"));
    for index in 0..3 {
        seed_session(&store, &format!("ses-offset-{index}"));
    }

    let page_ids = |offset: u32, limit: u32| -> (Vec<String>, u64) {
        let page = store.scan("session", offset, limit).unwrap();
        let ids = page
            .records
            .iter()
            .map(|envelope| {
                envelope
                    .key
                    .as_str()
                    .expect("session key 为会话 id 字符串")
                    .to_owned()
            })
            .collect();
        (ids, page.total)
    };

    let (first, total) = page_ids(0, 10);
    assert_eq!(
        first,
        vec!["ses-offset-0", "ses-offset-1", "ses-offset-2"],
        "offset=0 首页全量"
    );
    assert_eq!(total, 3, "total 为全量计数（非本页条数）");

    let (at_end, total) = page_ids(3, 10);
    assert!(at_end.is_empty(), "offset 恰等于记录总数返回空页");
    assert_eq!(
        total, 3,
        "offset 恰等总数：records 空而 total 恒全量（快照级自洽 R2）"
    );

    let (beyond, total) = page_ids(u32::MAX, 10);
    assert!(
        beyond.is_empty(),
        "offset 超过记录总数（u32::MAX）返回空页不报错"
    );
    assert_eq!(total, 3, "offset 超界：total 恒全量计数");
}

#[test]
fn scan_limit为零返回空大limit返回剩余全部_total恒全量() {
    let env = Env::new("scan-limit");
    let store = open_ws_ok(&env.db_path("ws"));
    for index in 0..3 {
        seed_session(&store, &format!("ses-limit-{index}"));
    }

    let zero = store.scan("session", 0, 0).unwrap();
    assert!(zero.records.is_empty(), "limit=0 返回空页（不推进不炸）");
    assert_eq!(zero.total, 3, "limit=0：total 恒全量计数");

    let rest = store.scan("session", 1, 100).unwrap();
    assert_eq!(rest.records.len(), 2, "limit 大于剩余记录数返回剩余全部");
    assert_eq!(rest.total, 3, "records 区间截断不缩水 total");

    let exact = store.scan("session", 0, 2).unwrap();
    assert_eq!(exact.records.len(), 2, "limit 小于总数返回恰 limit 条");
    assert_eq!(exact.total, 3, "records 区间截断不缩水 total");
}

#[test]
fn scan_limit超过500截断为500上限语义_total仍全量计数() {
    let env = Env::new("scan-limit-cap");
    let store = open_ws_ok(&env.db_path("ws"));
    // 505 条 > 上限 500：截断语义与「恰好 500 条」歧义区分
    for index in 0..505 {
        seed_session(&store, &format!("ses-cap-{index}"));
    }

    let first_page = store.scan("session", 0, 5000).unwrap();
    assert_eq!(first_page.records.len(), 500, "limit 超上限截断为 500");
    assert_eq!(
        first_page.total, 505,
        "截断只作用 records 区间，total 仍全量计数（不随截断缩水）"
    );
    let rest = store.scan("session", 500, 5000).unwrap();
    assert_eq!(rest.records.len(), 5, "截断后剩余记录可经 offset 续读");
    assert_eq!(rest.total, 505, "续页 total 恒全量计数");
    let mut union: Vec<String> = first_page
        .records
        .iter()
        .chain(rest.records.iter())
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
// 库存形态边界（list 参数映射）：空库 / 单条库 / offset 超界——快照级自洽
// ---------------------------------------------------------------------------

#[test]
fn scan库存形态边界_空库单条库与offset超界records空而total恒自洽() {
    let env = Env::new("scan-stock-shape");
    let store = open_ws_ok(&env.db_path("ws"));

    // 空库：恰 { records: [], total: 0 }
    let empty = store
        .scan("session", 0, 10)
        .unwrap_or_else(|e| panic!("空库 scan 应成功: {e}"));
    assert_eq!(
        empty,
        RecordsPage {
            records: Vec::new(),
            total: 0
        },
        "空库返回空分页信封（records 空数组 + total 0）"
    );

    // 单条库：{ records: [1 条], total: 1 }
    seed_session(&store, "ses-single");
    let single = store.scan("session", 0, 10).unwrap();
    assert_eq!(single.records.len(), 1, "单条库 records 恰 1 条");
    assert_eq!(single.total, 1, "单条库 total 恒 1");

    // limit = 0 连续取页不推进不炸（records 恒空、total 恒全量）
    let zero_limit = store.scan("session", 0, 0).unwrap();
    assert!(
        zero_limit.records.is_empty(),
        "limit=0 连续取页不推进（records 恒空）"
    );
    assert_eq!(zero_limit.total, 1, "limit=0：total 恒全量计数");

    // offset 超界：records 空而 total 恒全量
    let beyond = store.scan("session", 1, 10).unwrap();
    assert!(beyond.records.is_empty(), "offset 恰等总数 records 空页");
    assert_eq!(
        beyond.total, 1,
        "offset 超界：records 空而 total 恒全量（快照级自洽 R2）"
    );
    let way_beyond = store.scan("session", u32::MAX, 10).unwrap();
    assert!(
        way_beyond.records.is_empty(),
        "offset u32::MAX 同口径空页不报错"
    );
    assert_eq!(way_beyond.total, 1, "offset u32::MAX：total 恒全量计数");
}

// ---------------------------------------------------------------------------
// 翻页拼接与幽灵空页边界（total 口径）
// ---------------------------------------------------------------------------

#[test]
fn scan_total恰为limit整数倍_末页后空页records空而total不变() {
    let env = Env::new("scan-ghost-page");
    let store = open_ws_ok(&env.db_path("ws"));
    for index in 0..100 {
        seed_session(&store, &format!("ses-mul-{index:03}"));
    }

    // total=100 恰为 limit=50 整数倍：末页 records 50 条、无剩余可翻
    let page2 = store.scan("session", 50, 50).expect("第二页应成功");
    assert_eq!(page2.records.len(), 50, "末页 records 恰 limit 条");
    assert_eq!(
        50 + page2.records.len() as u64,
        page2.total,
        "末页 offset + records.len() == total"
    );

    // 下一页：records 空而 total 不变——「幽灵空页」在数据接口侧的边界锚
    //（前端由此导出 next 禁用）
    let ghost = store
        .scan("session", 100, 50)
        .expect("下一页应成功（空页非 Err）");
    assert!(
        ghost.records.is_empty(),
        "幽灵空页 records 空（接口侧边界锚）"
    );
    assert_eq!(ghost.total, 100, "空页 total 不变（页界与行集同快照）");
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
    assert_eq!(ws_page.records.len(), 1);
    assert_eq!(
        ws_page.records[0].key,
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
    assert_eq!(session_page.records.len(), 1);
    assert_eq!(
        session_page.records[0].key,
        serde_json::json!(session_id),
        "session key 信封为 core 铸会话 id 字符串"
    );

    let event_page = ws.scan("session_event", 0, 10).unwrap();
    assert_eq!(event_page.records.len(), 1);
    assert_eq!(
        event_page.records[0].key,
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
    assert_eq!(page.records.len(), 1);

    let value = &page.records[0].value;
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
    let ws_value = &global.scan("workspace", 0, 10).unwrap().records[0].value;
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
    let session_value = &ws.scan("session", 0, 10).unwrap().records[0].value;
    assert_eq!(session_value["id"], serde_json::json!(session_id));
    assert_eq!(session_value["source"], serde_json::json!("debug"));
    assert_eq!(
        session_value["configSnapshot"]["engine"],
        serde_json::json!("sdk")
    );

    let event_value = &ws.scan("session_event", 0, 10).unwrap().records[0].value;
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

    // 异常不产生副作用：本库已注册模型仍可扫描（空库空信封口径）
    let intact = store.scan("session", 0, 10).unwrap();
    assert!(intact.records.is_empty(), "本库已注册模型仍可扫描");
    assert_eq!(intact.total, 0, "空库 total 0");
}

// ---------------------------------------------------------------------------
// RecordsPage DTO 与新模型零改动覆盖
// ---------------------------------------------------------------------------

#[test]
fn records_page_dto线面camel_case恰两字段且derive面可用() {
    let env = Env::new("records-page-dto");
    let store = open_ws_ok(&env.db_path("ws"));
    seed_session(&store, "ses-dto");

    let page = store.scan("session", 0, 10).expect("scan 应成功");

    // serde camelCase 序列化形态恰 { records, total }（与 ModelInfo /
    // RecordEnvelope 同口径）
    let wire = serde_json::to_value(&page).expect("RecordsPage 可序列化");
    let object = wire.as_object().expect("线面为 JSON 对象");
    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["records", "total"],
        "camelCase 线面恰 {{ records, total }} 两键"
    );
    assert_eq!(wire["total"], serde_json::json!(1), "total 为数值计数");

    // Debug / Clone / PartialEq derive 面可用
    let cloned = page.clone();
    assert_eq!(page, cloned, "Clone 后 PartialEq 相等");
    let debug = format!("{page:?}");
    assert!(
        debug.contains("RecordsPage") && debug.contains("total"),
        "Debug 面可读: {debug}"
    );
}

#[test]
fn records_page_total为u64大值序列化为合法json_number不溢出不乱码() {
    let page = RecordsPage {
        records: Vec::new(),
        total: u64::MAX,
    };
    let wire = serde_json::to_value(&page).expect("大值 total 可序列化");
    assert_eq!(
        wire["total"].as_u64(),
        Some(u64::MAX),
        "超 u32 范围的 u64 total 序列化为合法 JSON number 无损"
    );
    let text = serde_json::to_string(&wire).expect("产物为合法 JSON 文本");
    assert!(!text.contains('\u{FFFD}'), "无乱码标记: {text}");
    let reparsed: serde_json::Value = serde_json::from_str(&text).expect("可无损再解析");
    assert_eq!(reparsed, wire, "大值 total 文本往返无损");
}

#[test]
fn 全部注册模型空库扫描均回空分页信封_十六个per_model包装零新分支() {
    let env = Env::new("all-models-scan");

    // 全局库四模型：ScanFn 返回类型随动后空库均恰空分页信封（登记一行接入
    // 不因分页信封漂移）
    let global = open_global_ok(&env.db_path("global"));
    for model in [
        "workspace",
        "agent_provider",
        "agent_instance",
        "notification",
    ] {
        let page = global
            .scan(model, 0, 10)
            .unwrap_or_else(|e| panic!("全局库 scan({model}) 应成功: {e}"));
        assert_eq!(
            page,
            RecordsPage {
                records: Vec::new(),
                total: 0
            },
            "全局库模型 {model} 空库空分页信封（per-model 包装零新分支）"
        );
    }
    drop(global);

    // workspace 库十二模型同口径（MODEL_ENTRIES 登记面 additive）
    let ws = open_ws_ok(&env.db_path("ws"));
    for model in [
        "agent_run",
        "session",
        "session_event",
        "explore",
        "change",
        "phase",
        "checklist_item",
        "step",
        "run",
        "run_node",
        "run_edge",
        "session_preamble",
    ] {
        let page = ws
            .scan(model, 0, 10)
            .unwrap_or_else(|e| panic!("workspace 库 scan({model}) 应成功: {e}"));
        assert_eq!(
            page.records,
            Vec::<RecordEnvelope>::new(),
            "workspace 库模型 {model} 空库 records 空"
        );
        assert_eq!(page.total, 0, "workspace 库模型 {model} 空库 total 0");
    }
}

// ---------------------------------------------------------------------------
// 本变更：注册表三新表（run / run_node / run_edge）+ `entries()` / `dimension_of`
//
// 观测口径：注册表是「新模型 = 定义 struct + 登记一行」的最小机制，故断言落在
// 四处可观察面：`list_models` 行集 / `scan` 三表键形态 / `entries()` 只读出口 /
// `dimension_of` 派库判据。播种走真写面 API（`start_change_run` /
// `finish_change_run`），不直插 native_db。
// ---------------------------------------------------------------------------

use workflow::state::{
    RunEdgeEntry, RunEdgeKind, RunFinishCommand, RunNodeEntry, RunStartCommand, RunStatus,
    RunStepKind, RunStepStatus,
};

/// 播种一条完整 run（主行 + 两节点 + 一边），返回 (run_id, 节点数, 边数)。
fn seed_run(store: &Store, run_id: &str) -> (String, u64, u64) {
    // run 收口同事务清 `active_phase` → 档行须在案（写面 miss 即 NotFound）
    store
        .create_change_record(change_archive("chg-run", "chg-run"))
        .unwrap_or_else(|e| panic!("create_change_record 应成功: {e}"));
    store
        .start_change_run(&RunStartCommand {
            run_id: run_id.to_owned(),
            change_id: "chg-run".to_owned(),
            started_at: 1000,
        })
        .unwrap_or_else(|e| panic!("start_change_run 应成功: {e}"));
    let node = |step: RunStepKind, instance_no: u32| RunNodeEntry {
        phase: "dev-design".to_owned(),
        attempt: 1,
        step,
        instance_no,
        status: RunStepStatus::Passed,
        session_id: None,
        detail: Some("记因".to_owned()),
        output_ref: None,
        started_at: 1000,
        ended_at: Some(2000),
        token_input: None,
        token_output: None,
    };
    let nodes = vec![
        node(RunStepKind::Executor, 0),
        node(RunStepKind::StaticCheck, 0),
    ];
    let edges = vec![RunEdgeEntry {
        from_node_key: "dev-design:1:executor:0".to_owned(),
        to_node_key: "dev-design:1:static_check:0".to_owned(),
        edge_kind: RunEdgeKind::Forward,
    }];
    store
        .finish_change_run(&RunFinishCommand {
            run_id: run_id.to_owned(),
            change_id: "chg-run".to_owned(),
            status: RunStatus::Completed,
            reason: None,
            finished_at: 3000,
            nodes: nodes.clone(),
            edges: edges.clone(),
        })
        .unwrap_or_else(|e| panic!("finish_change_run 应成功: {e}"));
    (run_id.to_owned(), nodes.len() as u64, edges.len() as u64)
}

/// 正向（AC-2）：`entries()` 长度恰 16、名字唯一、序 = 登记序（user 四行在前 /
/// workspace 十二行在后），维度分组与名字集一致。
#[test]
fn entries长度与登记序与维度分组() {
    let entries = crate::envelope::entries();
    assert_eq!(
        entries.len(),
        16,
        "注册表恰十六行（本变更补 session_preamble 一行）"
    );
    let names: Vec<&str> = entries.iter().map(|entry| entry.name).collect();
    assert_eq!(
        names,
        vec![
            "workspace",
            "agent_provider",
            "agent_instance",
            "notification",
            "agent_run",
            "session",
            "session_event",
            "explore",
            "change",
            "phase",
            "checklist_item",
            "step",
            "run",
            "run_node",
            "run_edge",
            "session_preamble",
        ],
        "名字逐值有序（user 四行在前 / workspace 十二行在后）"
    );
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 16, "名字唯一");

    let user: Vec<&str> = entries
        .iter()
        .filter(|entry| entry.dimension == DbDimension::User)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(
        user,
        vec![
            "workspace",
            "agent_provider",
            "agent_instance",
            "notification"
        ],
        "user 维度四行（registry 登记面 additive）"
    );
    let workspace: Vec<&str> = entries
        .iter()
        .filter(|entry| entry.dimension == DbDimension::Workspace)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(workspace.len(), 12, "workspace 维度十二行");
    assert_eq!(
        workspace.first(),
        Some(&"agent_run"),
        "workspace 段首项（登记序锚）"
    );
    assert_eq!(
        workspace.last(),
        Some(&"session_preamble"),
        "workspace 段末项（登记序锚）"
    );
}

/// 边界（AC-2）：`entries()` 与 `list_models` 两个维度清单并集逐名相等（同一
/// 真源），首项 `workspace`、末项 `session_preamble`。
#[test]
fn entries与list_models同源且首尾锚定() {
    let env = Env::new("envelope-entries");
    let global = open_global_ok(&env.db_path("global"));
    let ws = open_ws_ok(&env.db_path("ws"));

    let mut union: Vec<String> = global
        .list_models()
        .expect("全局库清单")
        .into_iter()
        .map(|model| model.name)
        .collect();
    union.extend(
        ws.list_models()
            .expect("workspace 库清单")
            .into_iter()
            .map(|model| model.name),
    );
    let entries: Vec<String> = crate::envelope::entries()
        .iter()
        .map(|entry| entry.name.to_owned())
        .collect();
    assert_eq!(union, entries, "两读面并集逐名相等（同一注册表真源）");
    assert_eq!(entries.first().map(String::as_str), Some("workspace"));
    assert_eq!(entries.last().map(String::as_str), Some("session_preamble"));
}

/// 正向（AC-3）：`dimension_of` 六表逐表派库；异常面 `store_meta` / 拼写 /
/// 空串 / 纯空白 / 大小写漂移 / 特殊字符 / 超长串均 `None`（零归一、零 trim）。
#[test]
fn dimension_of六表命中与七形态miss() {
    for (table, dimension) in [
        ("run", DbDimension::Workspace),
        ("run_node", DbDimension::Workspace),
        ("run_edge", DbDimension::Workspace),
        ("phase", DbDimension::Workspace),
        ("workspace", DbDimension::User),
        ("agent_provider", DbDimension::User),
        ("agent_instance", DbDimension::User),
    ] {
        assert_eq!(
            crate::envelope::dimension_of(table),
            Some(dimension),
            "表 {table} 的维度"
        );
    }
    let long = "字".repeat(1200);
    for miss in [
        "store_meta",
        "phases",
        "",
        "   ",
        "Phase",
        "a\nb",
        "a\0b🚀",
        &long,
    ] {
        assert_eq!(
            crate::envelope::dimension_of(miss),
            None,
            "未注册名 {miss:?} → None（零归一）"
        );
    }
}

/// 正向（AC-1 / AC-10）：三新表 `scan` 主键信封形态（`run` 字符串 / `run_node`、
/// `run_edge` 十六进制串）与播种行可读；空表 `records` 空 + `total` 0。
#[test]
fn scan三run表主键形态与空表零行() {
    let env = Env::new("envelope-runs");
    let ws = open_ws_ok(&env.db_path("ws"));

    // 空表面：三表 records 空 + total 0（登记即接入，零 per-table 特殊分支）
    for model in ["run", "run_node", "run_edge"] {
        let page = ws.scan(model, 0, 10).expect("空表 scan 应成功");
        assert!(page.records.is_empty(), "{model} 空表 records 空");
        assert_eq!(page.total, 0, "{model} 空表 total 0");
    }

    let (run_id, node_count, edge_count) = seed_run(&ws, "run-envelope");

    let runs = ws.scan("run", 0, 10).expect("run scan 应成功");
    assert_eq!(runs.total, 1, "run 计数");
    assert_eq!(runs.records.len(), 1);
    assert_eq!(
        runs.records[0].key,
        serde_json::json!(run_id),
        "run 主键为 run id 字符串"
    );
    assert_eq!(
        runs.records[0].value["changeId"],
        serde_json::json!("chg-run"),
        "run 行 value 为小驼峰 JSON"
    );

    let nodes = ws.scan("run_node", 0, 10).expect("run_node scan 应成功");
    assert_eq!(nodes.total, node_count, "run_node 计数 = 播种节点数");
    for record in &nodes.records {
        // 键形态 `{runId, nodeKey}`（打包 u128 键为十六进制串的 JSON 可读投影）
        assert_eq!(
            record.key["runId"],
            serde_json::json!(run_id),
            "节点键含 run 归属"
        );
        let node_key = record.key["nodeKey"]
            .as_str()
            .unwrap_or_else(|| panic!("run_node 打包键为十六进制串: {}", record.key));
        assert!(node_key.starts_with("0x"), "十六进制前缀: {node_key}");
        assert!(
            node_key[2..].chars().all(|char| char.is_ascii_hexdigit()),
            "`0x` 后全十六进制: {node_key}"
        );
        assert_eq!(record.value["runId"], serde_json::json!(run_id));
    }

    let edges = ws.scan("run_edge", 0, 10).expect("run_edge scan 应成功");
    assert_eq!(edges.total, edge_count, "run_edge 计数 = 播种边数");
    let edge_key = edges.records[0].key["edgeKey"]
        .as_str()
        .unwrap_or_else(|| panic!("run_edge 打包键为十六进制串: {}", edges.records[0].key));
    assert!(edge_key.starts_with("0x"), "十六进制前缀: {edge_key}");
    assert_eq!(
        edges.records[0].key["runId"],
        serde_json::json!(run_id),
        "边键含 run 归属"
    );
    assert_eq!(
        edges.records[0].value["edgeKind"],
        serde_json::json!("forward"),
        "边 kind 线值"
    );
}

/// 正向（AC-2）：workspace 维度 `list_models` 三新表计数与播种量一致（`run` 1 /
/// `run_node` n / `run_edge` m）；异常面未知模型名与跨维度模型名逐形态 `Err`。
#[test]
fn list_models三新表计数与scan错误面() {
    let env = Env::new("envelope-run-counts");
    let ws = open_ws_ok(&env.db_path("ws"));
    let (_run_id, node_count, edge_count) = seed_run(&ws, "run-counts");

    let models = ws.list_models().expect("清单应成功");
    let count_of = |name: &str| {
        models
            .iter()
            .find(|model| model.name == name)
            .unwrap_or_else(|| panic!("清单应含 {name}"))
            .count
    };
    assert_eq!(count_of("run"), 1, "run 计数");
    assert_eq!(count_of("run_node"), node_count, "run_node 计数");
    assert_eq!(count_of("run_edge"), edge_count, "run_edge 计数");
    assert_eq!(count_of("phase"), 0, "零播种表计数 0");

    for model in ["store_meta", "format", "runs", ""] {
        let error = ws.scan(model, 0, 10).expect_err("未知名应 Err");
        assert!(
            error.to_string().contains("未知模型"),
            "模型 {model:?} 的文案: {error}"
        );
    }

    // 跨维度模型名：user 维度库按维度锁定 → workspace 模型名按未知模型 Err
    let global = open_global_ok(&env.db_path("global"));
    for model in ["phase", "run", "run_node", "run_edge"] {
        let error = global.scan(model, 0, 10).expect_err("跨维度应 Err");
        assert!(
            error.to_string().contains("未知模型"),
            "全局库 scan({model}) 的文案: {error}"
        );
    }
}

/// 边界（回归登记）：查看页分页上限 `MAX_SCAN_LIMIT = 500` 口径零改动——三新表
/// 亦按同一截断语义（`limit` 500 上限 / `limit = 0` 空页 / `offset` 越过全量）。
#[test]
fn scan三新表沿用查看页五百上限口径() {
    let env = Env::new("envelope-run-clamp");
    let ws = open_ws_ok(&env.db_path("ws"));
    let (run_id, _nodes, _edges) = seed_run(&ws, "run-clamp");

    let capped = ws
        .scan("run_node", 0, 1000)
        .expect("超上限 scan 应成功（钳到 500 而非 Err）");
    assert_eq!(capped.total, 2, "total 恒全量计数");
    assert_eq!(capped.records.len(), 2, "少量行时上限不生效");

    let empty = ws.scan("run_node", 0, 0).expect("limit 0 应成功");
    assert!(empty.records.is_empty(), "limit 0 → 空页");
    assert_eq!(empty.total, 2, "total 照常");

    let beyond = ws.scan("run_node", 10, 10).expect("offset 越过全量应成功");
    assert!(beyond.records.is_empty(), "offset 越过全量 → 空页");
    assert_eq!(beyond.total, 2, "total 照常");

    let runs = ws.scan("run", 0, 10).expect("run scan 应成功");
    assert_eq!(runs.records[0].value["runId"], serde_json::json!(run_id));
}

// ---------------------------------------------------------------------------
// 通知表登记面（desktop-notification-hub）：登记一行 = db 查看器零模型特定代码
// 即可见（清单 / 计数 / 扫描 / 表说明四读面同源）
// ---------------------------------------------------------------------------

/// 通知载荷 fixture（事件轨，`kind = Event`）。
fn fixture_notification(id: &str) -> ::notification::Notification {
    ::notification::Notification {
        id: id.to_owned(),
        kind: ::notification::NotificationKind::Event,
        level: ::notification::NotificationLevel::Warning,
        message: format!("变更「add-feature」停在 dev-design 前等你确认（{id}）"),
        body: Some("设计相位产出。".to_owned()),
        root: Some("/ws/alpha".to_owned()),
        source_ref: Some("add-feature".to_owned()),
        created_at: 1_727_000_000_000,
    }
}

/// 正向：`entry` 命中且维度为 user；全局库 `list_models` 含该行，计数与写入量一致
///（登记面 additive——空库计数 0 也列出）。
#[test]
fn 通知表登记行维度为user且计数与写入量一致() {
    let entry = crate::envelope::entry("notification").expect("注册表应含 notification 行");
    assert_eq!(entry.dimension, DbDimension::User, "收件箱是 user 维度");

    let env = Env::new("notification-registry");
    let global = open_global_ok(&env.db_path("global"));
    let models = global.list_models().expect("清单应成功");
    let names: Vec<&str> = models.iter().map(|model| model.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "workspace",
            "agent_provider",
            "agent_instance",
            "notification"
        ],
        "user 段序 = 登记序（notification 在 agent_instance 之后）"
    );
    let count_of = |name: &str| {
        models
            .iter()
            .find(|model| model.name == name)
            .unwrap_or_else(|| panic!("模型 {name} 应在清单中"))
            .count
    };
    assert_eq!(count_of("notification"), 0, "空库计数 0 也列出");

    for index in 0..3 {
        global
            .publish_notification(&fixture_notification(&format!("run:r{index}:finished")))
            .expect("落库应成功");
    }
    let models = global.list_models().expect("清单应成功");
    let count = models
        .iter()
        .find(|model| model.name == "notification")
        .map(|model| model.count)
        .expect("计数行应在场");
    assert_eq!(count, 3, "计数随写入量");
}

/// 边界：`dimension_of` 精确名判据（近似名 / 空串 / 空白一律 None）。
#[test]
fn 通知表名精确命中且近似形态全miss() {
    assert_eq!(
        crate::envelope::dimension_of("notification"),
        Some(DbDimension::User)
    );
    for name in ["notifications", "Notification", "", "   "] {
        assert_eq!(
            crate::envelope::dimension_of(name),
            None,
            "精确名判据（零归一）：{name:?}"
        );
    }
}

/// 异常：workspace 库 `list_models` 零 notification 行（收件箱 MUST NOT 落
/// workspace 库的注册面判据）。
#[test]
fn workspace库清单零通知行() {
    let env = Env::new("notification-workspace-registry");
    let ws = open_ws_ok(&env.db_path("ws"));

    let names: Vec<String> = ws
        .list_models()
        .expect("清单应成功")
        .into_iter()
        .map(|model| model.name)
        .collect();
    assert!(
        !names.iter().any(|name| name == "notification"),
        "workspace 清单不含通知表，实际: {names:?}"
    );
}

/// 正向：`scan` 的每条信封 key == 记录 id；`total` == 全量行数。
#[test]
fn 通知表scan的信封键为记录id且total为全量() {
    let env = Env::new("notification-scan");
    let global = open_global_ok(&env.db_path("global"));
    for index in 0..3 {
        global
            .publish_notification(&fixture_notification(&format!("run:r{index}:finished")))
            .expect("落库应成功");
    }

    let page = global.scan("notification", 0, 10).expect("扫描应成功");
    assert_eq!(page.total, 3, "total = 全量行数");
    assert_eq!(page.records.len(), 3);
    for record in &page.records {
        assert_eq!(
            record.key, record.value["id"],
            "notification_key 取 json[\"id\"]"
        );
    }
}

/// 边界：空库返回空分页信封；`limit > 500` 钳位（`MAX_SCAN_LIMIT` 口径不变）。
#[test]
fn 通知表scan空库与limit钳位() {
    let env = Env::new("notification-scan-bounds");
    let global = open_global_ok(&env.db_path("global"));

    let empty = global.scan("notification", 0, 10).expect("扫描应成功");
    assert!(empty.records.is_empty(), "空库空分页");
    assert_eq!(empty.total, 0);

    global
        .publish_notification(&fixture_notification("run:r1:finished"))
        .expect("落库应成功");
    let clamped = global
        .scan("notification", 0, 10_000)
        .expect("大 limit 应成功");
    assert_eq!(clamped.total, 1);
    assert_eq!(clamped.records.len(), 1, "钳位不改变行集");
}

/// 异常：以 workspace 维度查通知表 → Err（跨维度不可达）。
#[test]
fn workspace库查通知表err() {
    let env = Env::new("notification-cross-dimension");
    let ws = open_ws_ok(&env.db_path("ws"));

    assert!(
        ws.scan("notification", 0, 10).is_err(),
        "跨维度模型名即未知模型"
    );
    assert!(
        crate::describe::table(DbDimension::Workspace, "notification").is_err(),
        "表说明面同样按维度锁定"
    );
}

/// 正向 / 边界：表说明面——表名 / 九字段 / 每字段 doc 非空 / 四 Option 字段渲染为
/// 可空 / `id` 为主键说明（`///` doc 派生，零模型特定代码）。
#[test]
fn 通知表说明面九字段且doc与非空渲染齐备() {
    let doc = crate::describe::table(DbDimension::User, "notification").expect("表说明应成功");

    assert_eq!(doc.table, "notification");
    assert_eq!(doc.dimension, DbDimension::User);
    assert!(!doc.summary.trim().is_empty(), "表介绍非空");
    let names: Vec<&str> = doc.fields.iter().map(|field| field.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "id",
            "kind",
            "level",
            "message",
            "body",
            "root",
            "sourceRef",
            "createdAt",
            "readAt",
        ],
        "九字段按源码序且线名为 camelCase"
    );
    for field in &doc.fields {
        assert!(!field.doc.trim().is_empty(), "字段 {} 说明非空", field.name);
    }
    let ty_of = |name: &str| {
        doc.fields
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("字段 {name} 应在场"))
            .ty
            .clone()
    };
    for optional in ["body", "root", "sourceRef"] {
        assert_eq!(ty_of(optional), "string | null", "{optional} 渲染为可空");
    }
    assert_eq!(
        ty_of("readAt"),
        "number | null",
        "readAt 是毫秒时间戳（i64），渲染为可空数"
    );
    assert!(
        ty_of("kind").contains("NotificationKind"),
        "kind 渲染为枚举引用，实际: {}",
        ty_of("kind")
    );
    assert!(
        ty_of("level").contains("NotificationLevel"),
        "level 渲染为枚举引用，实际: {}",
        ty_of("level")
    );
    assert_eq!(ty_of("id"), "string", "id 为文本主键");
    assert!(
        doc.fields
            .iter()
            .find(|field| field.name == "id")
            .map(|field| field.doc.contains("主键"))
            .unwrap_or(false),
        "id 的 doc 标注主键"
    );
}

// ---------------------------------------------------------------------------
// session-preamble-snapshot：`session_preamble` 信封登记行（scan / walk / key）
// ---------------------------------------------------------------------------

/// 播种 n 份快照行（走真写面 `stamp_session_preamble`，不直插 native_db）。
fn seed_preambles(store: &Store, n: usize) {
    for index in 0..n {
        assert!(
            store
                .stamp_session_preamble(
                    &format!("ses-preamble-{index}"),
                    &format!("快照正文 {index}"),
                )
                .expect("拍存应成功"),
            "首次拍板必写入"
        );
    }
}

#[test]
fn session_preamble登记行scan分页总数与键值形态() {
    let env = Env::new("envelope-preamble");
    let store = open_ws_ok(&env.db_path("ws"));
    seed_preambles(&store, 2);

    let page = store.scan("session_preamble", 0, 10).expect("scan 应成功");
    assert_eq!(page.total, 2, "计数与播种量一致");
    assert_eq!(page.records.len(), 2);
    for record in &page.records {
        let session_id = record
            .key
            .as_str()
            .unwrap_or_else(|| panic!("key 信封为 sessionId 字符串: {}", record.key));
        assert!(
            session_id.starts_with("ses-preamble-"),
            "键面取自 value[\"sessionId\"]: {session_id}"
        );
        let mut keys: Vec<&String> = record
            .value
            .as_object()
            .expect("value 为对象")
            .keys()
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec!["content", "savedAt", "sessionId"],
            "value 三键 camelCase"
        );
        assert_eq!(record.value["sessionId"], serde_json::json!(session_id));
    }
}

#[test]
fn session_preamble登记行scan越界offset与超上限limit截断() {
    let env = Env::new("envelope-preamble-page");
    let store = open_ws_ok(&env.db_path("ws"));
    seed_preambles(&store, 3);

    let beyond = store
        .scan("session_preamble", 9, 10)
        .expect("越界 scan 应成功");
    assert!(beyond.records.is_empty(), "越界 offset → 空 records");
    assert_eq!(beyond.total, 3, "total 恒全量（分页信封语义）");

    let clamped = store
        .scan("session_preamble", 0, 501)
        .expect("超上限 limit 应成功");
    assert_eq!(clamped.total, 3, "limit 超上限不改 total");
    assert!(
        clamped.records.len() <= 500,
        "limit 超上限截断到 500 以内，实际 {}",
        clamped.records.len()
    );
    assert_eq!(clamped.records.len(), 3, "三行全在本页");
}

#[test]
fn session_preamble登记行scan零limit与整页与walk通路() {
    let env = Env::new("envelope-preamble-walk");
    let store = open_ws_ok(&env.db_path("ws"));
    seed_preambles(&store, 3);

    let zero = store
        .scan("session_preamble", 0, 0)
        .expect("零 limit 应成功");
    assert!(zero.records.is_empty(), "limit = 0 → 空 records");
    assert_eq!(zero.total, 3, "total 恒全量");

    let exact = store
        .scan("session_preamble", 0, 3)
        .expect("整页 scan 应成功");
    assert_eq!(exact.records.len(), 3, "limit 恰等于总行数 → 全量一页");
    assert_eq!(exact.total, 3);

    // 分页跨越 walk 通路：`where` 条件迫使走注册表 `walk`（非 scan 快路径），
    // 一次读事务内逐条解码且计数一致
    let spec: crate::QuerySpec = serde_json::from_value(serde_json::json!({
        "table": "session_preamble",
        "where": [{ "field": "content", "op": "LIKE", "value": "快照正文%" }],
        "limit": 40
    }))
    .expect("spec 可解析");
    let page = store.query(&spec).expect("walk 通路查询应成功");
    assert_eq!(page.total, 3, "walk 通路的 total 精确计数");
    assert_eq!(page.matched, 3, "条件全命中");
    assert_eq!(page.records.len(), 3, "保留窗与计数一致");
}

#[test]
fn session_preamble登记行拼写错误与跨维度均err() {
    let env = Env::new("envelope-preamble-err");
    let ws = open_ws_ok(&env.db_path("ws"));
    let misspelled = scan_err(&ws, "session_preambles");
    assert!(
        misspelled.contains("未知模型"),
        "拼写错误名 → 未知模型 Err，实际: {misspelled}"
    );

    let global = open_global_ok(&env.db_path("global"));
    let wrong_dimension = scan_err(&global, "session_preamble");
    assert!(
        wrong_dimension.contains("未知模型"),
        "user 维度库上 session_preamble 跨维度不可达，实际: {wrong_dimension}"
    );
    assert!(
        ws.scan("session_preamble", 0, 10).is_ok(),
        "对照：workspace 维度库可达"
    );
}
