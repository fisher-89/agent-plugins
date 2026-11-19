//! `db_tool.rs`（两 DB 只读工具执行体）的单元测试：`db_tables` 两形态 /
//! `db_query` 全链（scope 解析 → 形状解析 → 敏感字段守卫 → store 原语 →
//! 预算闸 → 打码 → 结果成形）/ 双档截断 / 错误文案自纠。
//!
//! 装置：`tempfile` 真目录 + 真 `Store::open_global` / `Store::open_workspace`
//!（redb 单进程单句柄，进程内各开一次）+ 真写面 API 播种；`DbContext::new` 显式
//! 持两把 `Arc<Store>`（不经 `WorkspaceStores`——装配面归 `compose_test.rs`）。
//!
//! 不 mock：`store::{describe*, query}`（「不实现第二份查询语义」的机械判据：
//! 文案与词表必须来自同一来源）、`serde_json`（无半截 JSON 只能由真解析器证）、
//! `tools::cap_result`（断言「未触发」比替身更严格）。

use std::sync::Arc;

use serde_json::{json, Value};
use store::{
    AgentEffortTiers, AgentEngineKind, AgentModelTiers, AgentProviderProtocol, AgentProviderRecord,
    SessionConfigSnapshot, SessionRecord, Store,
};

use agent::{AgentEvent, AgentEventKind, AgentPermissionMode};

use crate::sdk::db_tool::{DbContext, ENVELOPE_RESERVE_BYTES, MASKED_VALUE, RESULT_BUDGET_BYTES};
use crate::sdk::tools::MAX_RESULT_BYTES;

const DB_TABLES: &str = "db_tables";
const DB_QUERY: &str = "db_query";
const PLAINTEXT_KEYS: [&str; 2] = ["sk-secret-alpha-7788", "sk-secret-beta-9900"];

// ---------------------------------------------------------------------------
// 装置：临时目录 + 真双库 + 真播种
// ---------------------------------------------------------------------------

struct Env {
    dir: tempfile::TempDir,
    global: Arc<Store>,
    workspace: Arc<Store>,
}

impl Env {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("db-tool-test-{tag}-"))
            .tempdir()
            .expect("创建临时目录失败");
        let global = Arc::new(
            Store::open_global(&dir.path().join("global.redb"))
                .unwrap_or_else(|error| panic!("打开全局库失败: {error}")),
        );
        let workspace = Arc::new(
            Store::open_workspace(&dir.path().join("ws.redb"))
                .unwrap_or_else(|error| panic!("打开 workspace 库失败: {error}")),
        );
        Self {
            dir,
            global,
            workspace,
        }
    }

    fn context(&self) -> DbContext {
        DbContext::new(Arc::clone(&self.global), Arc::clone(&self.workspace))
    }

    fn seed_providers(&self) {
        for (index, key) in PLAINTEXT_KEYS.iter().enumerate() {
            self.global
                .upsert_agent_provider(AgentProviderRecord {
                    id: 0,
                    name: format!("provider-{}", index + 1),
                    protocol: AgentProviderProtocol::Openai,
                    base_url: "http://127.0.0.1:9/v1".to_owned(),
                    api_key: (*key).to_owned(),
                    models: AgentModelTiers {
                        high: "m-high".to_owned(),
                        medium: "m-medium".to_owned(),
                        low: "m-low".to_owned(),
                    },
                    context_length: Some(128_000),
                    max_tokens: None,
                    web_search: false,
                    efforts: AgentEffortTiers::default(),
                })
                .unwrap_or_else(|error| panic!("落 provider 失败: {error}"));
        }
    }

    fn seed_session(&self, id: &str) {
        self.workspace
            .create_session(&SessionRecord {
                id: id.to_owned(),
                engine_session_id: None,
                config_snapshot: SessionConfigSnapshot {
                    engine: AgentEngineKind::Sdk,
                    agent_instance_id: None,
                    model: Some("m-high".to_owned()),
                    permission_mode: AgentPermissionMode::Default,
                },
                source: "debug".to_owned(),
                source_ref: None,
                created_at: 1727000000000,
                updated_at: 1727000000000,
            })
            .unwrap_or_else(|error| panic!("落会话失败: {error}"));
    }

    /// 转录行播种（`blob_chars` 控制单行体积——行数档与字节档两轴的语料）。
    fn seed_events(&self, session_id: &str, count: u64, blob_chars: usize) {
        let blob = "x".repeat(blob_chars);
        let events: Vec<AgentEvent> = (0..count)
            .map(|seq| AgentEvent {
                turn_id: None,
                seq,
                timestamp_ms: 1727000000000 + seq as i64,
                kind: AgentEventKind::Raw {
                    event_type: "mystery".to_owned(),
                    raw_json: format!(r#"{{"type":"mystery","blob":"{blob}"}}"#),
                },
            })
            .collect();
        self.workspace
            .append_session_events(session_id, &events)
            .unwrap_or_else(|error| panic!("落转录失败: {error}"));
    }

    /// 两条会话各按量播种（`total = 2 × per_session`）。
    fn seed_two_sessions(&self, per_session: u64, blob_chars: usize) {
        for id in ["ses-a", "ses-b"] {
            self.seed_session(id);
            self.seed_events(id, per_session, blob_chars);
        }
    }
}

async fn tables_of(context: &DbContext, input: Value) -> Value {
    let text = context
        .execute(DB_TABLES, &input)
        .await
        .unwrap_or_else(|error| panic!("db_tables 应成功: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("db_tables 结果为 JSON: {error}"))
}

async fn query_ok(context: &DbContext, input: Value) -> Value {
    let text = context
        .execute(DB_QUERY, &input)
        .await
        .unwrap_or_else(|error| panic!("db_query 应成功: {error}"));
    assert!(
        !text.contains("已截断：单结果超"),
        "结果体恒低于 L1 硬墙（`cap_result` 不实际触发）: {} 字节",
        text.len()
    );
    assert!(
        text.len() < MAX_RESULT_BYTES,
        "结果串长 {} < 50000",
        text.len()
    );
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("db_query 结果为合法完整 JSON: {error}"))
}

async fn query_err(context: &DbContext, input: Value) -> String {
    match context.execute(DB_QUERY, &input).await {
        Err(error) => error,
        Ok(text) => panic!("应 Err，实际返回: {text}"),
    }
}

fn rows(body: &Value) -> &Vec<Value> {
    body["records"]
        .as_array()
        .unwrap_or_else(|| panic!("records 为数组: {body}"))
}

// ---------------------------------------------------------------------------
// db_tables：无参清单 / 下钻 / scope 三态 / 错误面
// ---------------------------------------------------------------------------

#[tokio::test]
async fn db_tables无参返回十四行清单且不展开字段() {
    let env = Env::new("tables-list");
    env.seed_providers();
    env.seed_session("ses-db");
    env.seed_events("ses-db", 3, 10);
    let context = env.context();

    let body = tables_of(&context, json!({})).await;
    let list = body
        .as_array()
        .unwrap_or_else(|| panic!("无参形态为 JSON 数组: {body}"));
    assert_eq!(list.len(), 14, "两库全列恰 14 行");
    for row in list {
        assert!(row.get("dimension").is_some(), "每行含 dimension: {row}");
        assert!(row.get("table").is_some(), "每行含 table: {row}");
        assert!(row.get("count").is_some(), "每行含 count: {row}");
        assert!(row.get("summary").is_some(), "每行含 summary: {row}");
        assert!(row.get("fields").is_none(), "清单形态不展开字段: {row}");
        assert!(
            row["summary"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()),
            "summary 非空: {row}"
        );
    }
    let row_of = |table: &str| {
        list.iter()
            .find(|row| row["table"] == json!(table))
            .unwrap_or_else(|| panic!("清单应含 {table}"))
            .clone()
    };
    assert_eq!(row_of("agent_provider")["count"], json!(2), "user 计数");
    assert_eq!(row_of("agent_provider")["dimension"], json!("user"));
    assert_eq!(row_of("session")["count"], json!(1), "workspace 计数");
    assert_eq!(row_of("session")["dimension"], json!("workspace"));
    assert_eq!(row_of("session_event")["count"], json!(3));
    assert_eq!(row_of("change")["count"], json!(0), "零播种表计数 0");
    assert!(
        !list.iter().any(|row| row["table"] == json!("store_meta")),
        "store_meta 不出现在任一维度清单"
    );
}

#[tokio::test]
async fn db_tables带table下钻返回字段对象且线名为驼峰() {
    let env = Env::new("tables-drill");
    let context = env.context();
    let body = tables_of(&context, json!({"table": "phase"})).await;
    assert!(body.is_object(), "下钻形态为 JSON 对象: {body}");
    assert_eq!(body["table"], json!("phase"));
    assert_eq!(body["dimension"], json!("workspace"));
    assert!(
        body.get("count").is_none(),
        "下钻形态不含 count 键（两形态字段集互不重叠）"
    );
    assert!(
        body["summary"]
            .as_str()
            .is_some_and(|text| !text.is_empty()),
        "下钻含表介绍"
    );
    let fields = body["fields"].as_array().expect("fields 为数组");
    assert_eq!(fields.len(), 15, "phase 全字段");
    for field in fields {
        assert!(field.get("name").is_some(), "字段含 name: {field}");
        assert!(field.get("type").is_some(), "字段含 type: {field}");
        assert!(field.get("doc").is_some(), "字段含 doc: {field}");
    }
    let names: Vec<&str> = fields
        .iter()
        .map(|field| field["name"].as_str().expect("name 字符串"))
        .collect();
    assert!(names.contains(&"changeId"), "camelCase 线名在场: {names:?}");
    assert!(!names.contains(&"change_id"), "蛇形名缺席: {names:?}");
}

#[tokio::test]
async fn db_tables_scope三态收窄且下钻按维度派库() {
    let env = Env::new("tables-scope");
    env.seed_providers();
    let context = env.context();

    let user = tables_of(&context, json!({"scope": "user"})).await;
    let user_rows = user.as_array().expect("数组");
    assert_eq!(user_rows.len(), 3, "scope user → 3 行");
    assert!(
        user_rows
            .iter()
            .all(|row| row["dimension"] == json!("user")),
        "user 侧清单互不混列"
    );

    let workspace = tables_of(&context, json!({"scope": "workspace"})).await;
    let workspace_rows = workspace.as_array().expect("数组");
    assert_eq!(workspace_rows.len(), 11, "scope workspace → 11 行");
    assert!(
        workspace_rows
            .iter()
            .all(|row| row["dimension"] == json!("workspace")),
        "workspace 侧清单互不混列"
    );

    let both = tables_of(&context, json!({"scope": Value::Null})).await;
    assert_eq!(
        both.as_array().expect("数组").len(),
        14,
        "无 scope → 两库全列"
    );

    // 下钻：按表所属维度解析（scope 与该维度一致时通过）
    let drilled = tables_of(
        &context,
        json!({"table": "agent_provider", "scope": "user"}),
    )
    .await;
    assert_eq!(drilled["dimension"], json!("user"));
    let mismatch = context
        .execute(
            DB_TABLES,
            &json!({"table": "agent_provider", "scope": "workspace"}),
        )
        .await
        .expect_err("维度与 scope 不符应 Err");
    assert!(
        mismatch.contains("user") && mismatch.contains("scope"),
        "维度自纠文案: {mismatch}"
    );
}

#[tokio::test]
async fn db_tables错误面与入参边界() {
    let env = Env::new("tables-errors");
    let context = env.context();

    let unknown = context
        .execute(DB_TABLES, &json!({"table": "phases"}))
        .await
        .expect_err("未知表应 Err");
    assert!(unknown.contains("phases"), "文案携入参名: {unknown}");
    assert!(unknown.contains("phase"), "文案携候选表名: {unknown}");

    for scope in [json!("global"), json!(""), json!(1), json!({})] {
        let error = context
            .execute(DB_TABLES, &json!({"scope": scope}))
            .await
            .expect_err("非法 scope 应 Err");
        assert!(
            error.contains("user") && error.contains("workspace"),
            "scope {scope} 的文案列两合法值: {error}"
        );
    }

    // 入参形状错误一律显式回灌：清单是「省略参数」的专属形态，MUST NOT 由写错
    // 的入参静默退化出来（模型会把「传错了」读成「查询成功」）
    for (input, keyword) in [
        (json!("str"), "入参形状不符"),
        (json!([]), "入参形状不符"),
        (json!(null), "入参形状不符"),
        (json!({"table": 1}), "table 须为字符串"),
        (json!({"table": "   "}), "table 不可为空白"),
    ] {
        let error = context
            .execute(DB_TABLES, &input)
            .await
            .expect_err(&format!("入参 {input} 形状错误应 Err（不得退化为表清单）"));
        assert!(
            error.contains(keyword),
            "入参 {input} 的文案应含 `{keyword}`: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// db_query：全链形状 / scope / 入参透传
// ---------------------------------------------------------------------------

#[tokio::test]
async fn db_query全链返回信封形状且三元自洽() {
    let env = Env::new("query-chain");
    env.seed_two_sessions(15, 10);
    let context = env.context();

    let body = query_ok(
        &context,
        json!({
            "table": "session_event",
            "where": [{"field": "sessionId", "op": "=", "value": "ses-a"}],
            "order": {"field": "eventKey", "desc": true},
            "select": ["eventKey", "sessionId"],
            "offset": 0,
            "limit": 5
        }),
    )
    .await;
    assert_eq!(body["table"], json!("session_event"));
    for key in [
        "total",
        "matched",
        "returned",
        "truncated",
        "truncatedBy",
        "records",
    ] {
        assert!(body.get(key).is_some(), "结果体含 {key}: {body}");
    }
    assert_eq!(body["total"], json!(30), "total = 表全量");
    assert_eq!(
        body["matched"],
        json!(15),
        "matched = 过滤后行数（ses-a 15 条）"
    );
    assert_eq!(body["returned"], json!(5), "returned = 本页行数");
    assert_eq!(
        body["truncated"],
        json!(true),
        "returned == limit 且 matched 更大"
    );
    assert_eq!(body["truncatedBy"], json!("limit"), "行数档");
    let returned = body["returned"].as_u64().expect("returned 数字");
    let matched = body["matched"].as_u64().expect("matched 数字");
    assert!(
        returned <= matched && matched <= body["total"].as_u64().expect("total 数字"),
        "returned ≤ matched ≤ total"
    );
    for record in rows(&body) {
        assert!(record.get("key").is_some(), "records 逐条含 key: {record}");
        assert!(
            record.get("value").is_some(),
            "records 逐条含 value: {record}"
        );
        assert_eq!(
            record["value"]["sessionId"],
            json!("ses-a"),
            "过滤后逐条命中"
        );
        assert!(
            record["value"].get("event").is_none(),
            "select 未选字段不出现: {record}"
        );
    }

    let untruncated = query_ok(&context, json!({"table": "session_event", "limit": 50})).await;
    assert_eq!(untruncated["truncated"], json!(false));
    assert_eq!(untruncated["truncatedBy"], Value::Null, "未触发为 null");
}

#[tokio::test]
async fn db_query_scope派库与维度自纠() {
    let env = Env::new("query-scope");
    env.seed_providers();
    env.seed_session("ses-db");
    let context = env.context();

    let user = query_ok(
        &context,
        json!({"table": "agent_provider", "scope": "user", "select": ["id", "name"]}),
    )
    .await;
    assert_eq!(user["matched"], json!(2), "scope user 走全局库");

    let defaulted = query_ok(&context, json!({"table": "session"})).await;
    assert_eq!(defaulted["matched"], json!(1), "缺省 scope 走 workspace 库");

    for (table, scope) in [("agent_provider", "workspace"), ("session", "user")] {
        let error = query_err(&context, json!({"table": table, "scope": scope})).await;
        assert!(error.contains(table), "维度不符文案携表名 {table}: {error}");
        assert!(
            error.contains("scope"),
            "维度不符文案给改正指引（加 scope）: {error}"
        );
    }
    for scope in [json!("global"), json!(""), json!(1)] {
        let error = query_err(&context, json!({"table": "session", "scope": scope})).await;
        assert!(
            error.contains("user") && error.contains("workspace"),
            "非法 scope {scope} 的文案列两合法值: {error}"
        );
    }
}

#[tokio::test]
async fn db_query入参透传_scope被先摘且三类显式null等价缺省() {
    let env = Env::new("query-passthrough");
    env.seed_session("ses-db");
    let context = env.context();

    // `scope` 不进 `QuerySpec`（`deny_unknown_fields`）→ 执行体先摘，不得 Err
    let body = query_ok(&context, json!({"table": "session", "scope": "workspace"})).await;
    assert_eq!(body["matched"], json!(1));

    // `where` / `order` / `select` 三类显式 null 与服务端缺省等价（同一「没给」的
    // 写法 MUST NOT 分叉成「全量」与「形状错误」两种结果）
    let baseline = query_ok(&context, json!({"table": "session"})).await;
    let nulls = query_ok(
        &context,
        json!({
            "table": "session",
            "where": Value::Null,
            "order": Value::Null,
            "select": Value::Null
        }),
    )
    .await;
    assert_eq!(nulls, baseline, "三类显式 null ≡ 服务端缺省（逐字段等价）");
    assert_eq!(nulls["matched"], json!(1));
    let value = &rows(&nulls)[0]["value"];
    assert!(
        value.get("configSnapshot").is_some(),
        "where null = 不过滤、select null = 全字段: {value}"
    );
}

// ---------------------------------------------------------------------------
// 预算闸与两档截断
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 预算闸按字节先到者止且无半截json() {
    let env = Env::new("budget-bytes");
    // 40 条约 1.6 KB 的转录行：缺省 20 行落预算内，limit 50 被预算截断
    env.seed_events("ses-big", 40, 1500);
    let context = env.context();

    let defaulted = query_ok(&context, json!({"table": "session_event"})).await;
    assert_eq!(defaulted["matched"], json!(40));
    assert_eq!(
        defaulted["returned"],
        json!(20),
        "缺省 20 行落在 40 KB 预算内"
    );

    let big = query_ok(&context, json!({"table": "session_event", "limit": 50})).await;
    assert_eq!(big["matched"], json!(40), "matched 恒精确");
    assert_eq!(big["truncated"], json!(true), "预算先到");
    assert_eq!(big["truncatedBy"], json!("bytes"), "截断成因 = 字节预算");
    let returned = big["returned"].as_u64().expect("returned");
    assert!(
        returned < big["matched"].as_u64().expect("matched"),
        "如实回报 returned < matched"
    );
    let hint = big["hint"].as_str().expect("截断带 hint");
    for guidance in ["select", "where", "offset"] {
        assert!(
            hint.contains(guidance),
            "hint 含收缩指引 `{guidance}`: {hint}"
        );
    }
    // 结果串可完整解析（上面 `query_ok` 已反序列化）→ records 条条完整
    assert_eq!(
        rows(&big).len() as u64,
        returned,
        "records 条数 == returned"
    );
}

#[tokio::test]
async fn 行数档与预算档两因可区分() {
    let env = Env::new("budget-limit");
    env.seed_two_sessions(15, 10);
    let context = env.context();

    let limit_stopped = query_ok(&context, json!({"table": "session_event", "limit": 20})).await;
    assert_eq!(limit_stopped["matched"], json!(30));
    assert_eq!(limit_stopped["returned"], json!(20));
    assert_eq!(limit_stopped["truncatedBy"], json!("limit"));
    let hint = limit_stopped["hint"].as_str().expect("行数档亦给 hint");
    assert!(
        hint.contains("行数上限") && hint.contains("本页返回 20 行"),
        "行数档文案如实回报三元: {hint}"
    );

    let fits = query_ok(&context, json!({"table": "session_event", "limit": 50})).await;
    assert_eq!(fits["returned"], json!(30), "行数与预算两档皆未触发");
    assert_eq!(fits["truncated"], json!(false));
    assert_eq!(fits["truncatedBy"], Value::Null);
    assert!(fits.get("hint").is_none(), "零截断不给 hint");
}

#[tokio::test]
async fn 预算边界_单条即超预算与恰到界先到者止() {
    let env = Env::new("budget-boundary");
    env.seed_events("ses-normal", 1, 10);
    env.seed_events("ses-huge", 1, 60_000);
    let context = env.context();

    // 单条记录序列化即 > 40 KB → 整条原子排除（returned == 0 + bytes 档）
    let huge = query_ok(
        &context,
        json!({"table": "session_event", "where": [{"field": "sessionId", "op": "=", "value": "ses-huge"}]}),
    )
    .await;
    assert_eq!(huge["matched"], json!(1));
    assert_eq!(huge["returned"], json!(0), "单条即超预算 → 零行");
    assert_eq!(huge["truncatedBy"], json!("bytes"));
    assert!(rows(&huge).is_empty(), "无半截记录");

    // 恰到界：以实测单条体积算出可容纳条数，断言「再加一条即越预算」即停
    let boundary = Env::new("budget-exact");
    boundary.seed_events("ses-exact", 6, 7000);
    let context = boundary.context();
    let probe = query_ok(&context, json!({"table": "session_event", "limit": 1})).await;
    let entry = rows(&probe)[0].clone();
    let unit = serde_json::to_string(&entry)
        .expect("单条记录可序列化")
        .len()
        + 1;
    let budget = RESULT_BUDGET_BYTES - ENVELOPE_RESERVE_BYTES;
    assert!(unit < budget, "单条 {unit} 字节应小于预算 {budget}");

    let page = query_ok(&context, json!({"table": "session_event", "limit": 6})).await;
    let returned = page["returned"].as_u64().expect("returned") as usize;
    assert_eq!(returned, rows(&page).len(), "returned == records 条数");
    assert!(
        returned * unit <= budget,
        "已收条数 {returned} × {unit} 未越预算 {budget}"
    );
    assert!(
        (returned + 1) * unit > budget,
        "先到者止：再加一条即越预算（{returned} + 1）× {unit} > {budget}"
    );
    assert_eq!(page["truncatedBy"], json!("bytes"));
}

// ---------------------------------------------------------------------------
// 敏感面：打码 + 比较面禁用
// ---------------------------------------------------------------------------

#[tokio::test]
async fn api_key输出恒打码且全文不含明文() {
    let env = Env::new("mask");
    env.seed_providers();
    let context = env.context();

    for input in [
        json!({"table": "agent_provider", "scope": "user"}),
        json!({"table": "agent_provider", "scope": "user", "select": ["id", "apiKey"]}),
    ] {
        let text = context
            .execute(DB_QUERY, &input)
            .await
            .unwrap_or_else(|error| panic!("查询应成功: {error}"));
        let body: Value = serde_json::from_str(&text).expect("结果 JSON");
        for record in rows(&body) {
            assert_eq!(
                record["value"]["apiKey"],
                json!(MASKED_VALUE),
                "apiKey 恒打码（{input}）"
            );
        }
        for plaintext in PLAINTEXT_KEYS {
            assert!(!text.contains(plaintext), "结果全文不含明文 key（{input}）");
            assert!(
                !text.contains(&plaintext[..4]),
                "结果全文不含明文前缀段 `{}`（{input}）",
                &plaintext[..4]
            );
        }
        assert_eq!(rows(&body).len(), 2, "两条 provider 皆在结果内");
    }
}

#[tokio::test]
async fn api_key不可用于过滤与排序三形态均err() {
    let env = Env::new("mask-guard");
    env.seed_providers();
    let context = env.context();

    for input in [
        json!({"table": "agent_provider", "scope": "user",
               "where": [{"field": "apiKey", "op": "LIKE", "value": "sk-s%"}]}),
        json!({"table": "agent_provider", "scope": "user",
               "where": [{"field": "apiKey", "op": "=", "value": PLAINTEXT_KEYS[0]}]}),
        json!({"table": "agent_provider", "scope": "user", "order": {"field": "apiKey"}}),
    ] {
        let error = query_err(&context, input.clone()).await;
        assert!(
            error.contains("apiKey") && error.contains("不可用于过滤 / 排序"),
            "形态 {input} 的守卫文案: {error}"
        );
        assert!(
            !error.contains(PLAINTEXT_KEYS[0]),
            "错误串不回显明文 key: {error}"
        );
    }
    // 嵌套段命中亦拦（`x.apiKey` 同址守卫）
    let nested = query_err(
        &context,
        json!({"table": "agent_provider", "scope": "user",
               "where": [{"field": "models.apiKey", "op": "=", "value": "x"}]}),
    )
    .await;
    assert!(
        nested.contains("不可用于过滤 / 排序"),
        "嵌套段同守卫: {nested}"
    );
}

// ---------------------------------------------------------------------------
// 错误文案即自纠通道
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 错误文案自纠六形态() {
    let env = Env::new("errors");
    env.seed_session("ses-db");
    env.seed_events("ses-db", 2, 10);
    let context = env.context();

    let unknown_field = query_err(
        &context,
        json!({"table": "session_event", "where": [{"field": "session_id", "op": "=", "value": "ses-db"}]}),
    )
    .await;
    assert!(
        unknown_field.contains("session_id") && unknown_field.contains("sessionId"),
        "未知字段携候选线名: {unknown_field}"
    );

    let unknown_op = query_err(
        &context,
        json!({"table": "session_event", "where": [{"field": "sessionId", "op": "contains", "value": "ses-db"}]}),
    )
    .await;
    for word in store::QUERY_OPS {
        assert!(
            unknown_op.contains(word),
            "未知 op 文案含词表逐词 {word}: {unknown_op}"
        );
    }

    let type_mismatch = query_err(
        &context,
        json!({"table": "session_event", "where": [{"field": "eventKey", "op": "=", "value": 1}]}),
    )
    .await;
    assert!(
        type_mismatch.contains("string"),
        "类型不符携字段渲染类型: {type_mismatch}"
    );

    let index_path = query_err(
        &context,
        json!({"table": "session_event",
               "where": [{"field": "event.blocks.0.name", "op": "=", "value": "text"}]}),
    )
    .await;
    assert!(
        index_path.contains("下标"),
        "数组下标携改正指引: {index_path}"
    );

    let in_shape = query_err(
        &context,
        json!({"table": "session_event", "where": [{"field": "sessionId", "op": "IN", "value": "ses-db"}]}),
    )
    .await;
    assert!(in_shape.contains("数组"), "IN 非数组携期望形状: {in_shape}");

    let isnull_shape = query_err(
        &context,
        json!({"table": "session_event", "where": [{"field": "sessionId", "op": "IS NULL", "value": 1}]}),
    )
    .await;
    assert!(
        isnull_shape.contains("不接 value"),
        "IS NULL 带值携形状提示: {isnull_shape}"
    );
}

#[tokio::test]
async fn 嵌套路径零命中才给点号hint() {
    let env = Env::new("hint");
    env.seed_session("ses-db");
    env.seed_events("ses-db", 2, 10);
    let context = env.context();

    let nested = query_ok(
        &context,
        json!({"table": "session_event", "where": [{"field": "event.nope", "op": "=", "value": "x"}]}),
    )
    .await;
    assert_eq!(nested["matched"], json!(0));
    let hint = nested["hint"].as_str().expect("零命中带点号 hint");
    assert!(
        hint.contains("嵌套路径") && hint.contains("db_tables"),
        "hint 提示先确认字段名: {hint}"
    );

    let flat = query_ok(
        &context,
        json!({"table": "session_event", "where": [{"field": "sessionId", "op": "=", "value": "不存在"}]}),
    )
    .await;
    assert_eq!(flat["matched"], json!(0));
    assert!(
        flat.get("hint").is_none(),
        "无点号路径的零命中不给误导性 hint"
    );
}

#[tokio::test]
async fn 未知名逐形态err且出口过cap_result() {
    let env = Env::new("unknown-name");
    let context = env.context();
    for name in ["db_tables2", "", "DB_QUERY", "db_query "] {
        let error = context
            .execute(name, &json!({}))
            .await
            .expect_err("未知名应 Err");
        assert!(
            error.contains(name) || error.contains("未知 DB 工具"),
            "未知名 {name:?} 的文案: {error}"
        );
    }
    // 成功路径出口经 `cap_result`：结果串不含截断留痕、长度低于硬墙
    let text = context
        .execute(DB_TABLES, &json!({}))
        .await
        .expect("表清单应成功");
    assert!(!text.contains("已截断：单结果超"));
    assert!(text.len() < MAX_RESULT_BYTES);
}

// ---------------------------------------------------------------------------
// 组合行：同一上下文连续调用互不污染
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 同一上下文两工具连续调用互不污染() {
    let env = Env::new("combo");
    env.seed_providers();
    env.seed_session("ses-db");
    let context = env.context();

    let first_tables = tables_of(&context, json!({})).await;
    let first_query = query_ok(&context, json!({"table": "session"})).await;
    let second_tables = tables_of(&context, json!({})).await;
    let second_query = query_ok(&context, json!({"table": "session"})).await;
    assert_eq!(first_tables, second_tables, "表清单无跨调用状态");
    assert_eq!(first_query, second_query, "查询无跨调用状态");
    assert_eq!(
        first_tables.as_array().expect("数组").len(),
        14,
        "两库句柄皆在场（14 行）"
    );
    assert_eq!(first_query["matched"], json!(1), "workspace 句柄可读");
    assert!(
        env.dir.path().exists(),
        "库文件仍在临时目录内（无二次 open 的旁路文件）"
    );
}
