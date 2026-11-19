//! `query.rs`（只读查询原语）的单元测试：op 词表逐格 / 点号下钻 / `LIKE` /
//! `IN` / `IS NULL` / null 比较语义 / 类型校验 / 排序 / 投影 / 分页 / 三元口径 /
//! 入参形状与错误面 / 只读零写面。
//!
//! 零 mock：被测面是进程内纯函数 + 单读事务扫描（无 IO / 配置 / 子进程 /
//! 网络 / 时间依赖）；`native_db` 用 `create_in_memory` 真内存库 + 真 `rw.insert`
//! 播种（跨类型值 / null 列 / 520 行语料都需真实编码，替身承载不了 `walk` /
//! `scan` 语义）；`crate::describe` 的字段索引真实调用（类型校验与渲染同一分类器
//! 正是被测性质，替身化会让两处口径漂移不可见）。

use native_db::Database;
use serde_json::{json, Value};

use agent::AgentPermissionMode;
use workflow::model::{ChecklistItem, Verdict};

use crate::envelope::{list_models, RecordEnvelope};
use crate::model::{
    pack_session_event_key, AgentEngineKind, SessionConfigSnapshot, SessionEventRecord,
    SessionRecord,
};
use crate::query::{
    run, QueryError, QueryPage, QuerySpec, DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT, QUERY_OPS,
};
use crate::store::{global_models, workspace_models, DbDimension};

// ---------------------------------------------------------------------------
// 装置：内存库 + 人工可核算语料（每格断言可手工对拍）
// ---------------------------------------------------------------------------

fn workspace_db() -> Database<'static> {
    native_db::Builder::new()
        .create_in_memory(workspace_models())
        .expect("建 workspace 内存库")
}

fn global_db() -> Database<'static> {
    native_db::Builder::new()
        .create_in_memory(global_models())
        .expect("建全局内存库")
}

struct PhaseRow {
    id: i64,
    change_id: &'static str,
    phase: &'static str,
    attempt: u32,
    verdict: Verdict,
    backtrack_reason: Option<&'static str>,
    timestamp: i64,
}

/// 五条相位行（changeId × phase × attempt × verdict × backtrack × timestamp 五维
/// 各取两个以上取值，使每格 op 断言的命中量可人工核算）。
const PHASES: [PhaseRow; 5] = [
    PhaseRow {
        id: 1,
        change_id: "chg-a",
        phase: "dev-design",
        attempt: 1,
        verdict: Verdict::Pass,
        backtrack_reason: None,
        timestamp: 100,
    },
    PhaseRow {
        id: 2,
        change_id: "chg-a",
        phase: "dev-design",
        attempt: 2,
        verdict: Verdict::Fail,
        backtrack_reason: Some("dev-design"),
        timestamp: 200,
    },
    PhaseRow {
        id: 3,
        change_id: "chg-a",
        phase: "test-gen",
        attempt: 1,
        verdict: Verdict::Pass,
        backtrack_reason: None,
        timestamp: 300,
    },
    PhaseRow {
        id: 4,
        change_id: "chg-b",
        phase: "dev-design",
        attempt: 1,
        verdict: Verdict::Pass,
        backtrack_reason: None,
        timestamp: 400,
    },
    PhaseRow {
        id: 5,
        change_id: "chg-b",
        phase: "archive",
        attempt: 1,
        verdict: Verdict::Fail,
        backtrack_reason: Some("x"),
        timestamp: 500,
    },
];

fn seed_phases(db: &Database<'static>) {
    let rw = db.rw_transaction().expect("写事务");
    for row in &PHASES {
        let id = row.id;
        rw.insert(crate::PhaseRecord {
            id,
            change_id: row.change_id.to_owned(),
            phase: row.phase.to_owned(),
            attempt: row.attempt,
            verdict: row.verdict,
            report: format!("报告 {id}"),
            skipped: false,
            stale: false,
            backtrack_to: row.backtrack_reason.map(str::to_owned),
            backtrack_reason: row.backtrack_reason.map(str::to_owned),
            executor_session_id: None,
            evaluator_session_id: None,
            decision_session_id: None,
            start_at: None,
            timestamp: row.timestamp,
        })
        .expect("落相位行");
    }
    rw.commit().expect("提交");
}

/// `LIKE` 语料（change 名五形态：`-` / `_` / 字面 `%` / 大写 / 空后缀）。
const CHANGE_NAMES: [&str; 5] = ["dev-design", "dev_design", "dev%x", "Dev-upper", "dev-"];

fn seed_changes(db: &Database<'static>) {
    let rw = db.rw_transaction().expect("写事务");
    for (index, name) in CHANGE_NAMES.iter().enumerate() {
        rw.insert(crate::ChangeRecord::new(
            &format!("chg-like-{index}"),
            name,
            "requirement",
            1000 + index as i64,
            None,
            None,
        ))
        .expect("落建档行");
    }
    rw.commit().expect("提交");
}

fn seed_sessions(db: &Database<'static>) {
    let rw = db.rw_transaction().expect("写事务");
    for (id, model) in [
        ("ses-1", Some("m-high".to_owned())),
        ("ses-2", None::<String>),
    ] {
        rw.insert(SessionRecord {
            id: id.to_owned(),
            engine_session_id: None,
            config_snapshot: SessionConfigSnapshot {
                engine: AgentEngineKind::Sdk,
                agent_instance_id: None,
                model,
                permission_mode: AgentPermissionMode::Default,
            },
            source: "debug".to_owned(),
            source_ref: None,
            created_at: 1727000000000,
            updated_at: 1727000000000,
        })
        .expect("落会话行");
    }
    rw.commit().expect("提交");
}

/// 六条转录行：`event` 载荷手写（`kind` / `blocks` / `mixed` 供下钻与跨类型定序
/// 用），其中 `mixed` 逐行取 number / string / bool / object / array / 缺席。
fn seed_events(db: &Database<'static>) {
    let payloads = [
        json!({"seq": 0, "timestampMs": 10, "kind": "message", "role": "assistant", "blocks": [{"kind": "text", "text": "你好"}], "mixed": 1}),
        json!({"seq": 1, "timestampMs": 20, "kind": "tool", "mixed": "s"}),
        json!({"seq": 2, "timestampMs": 30, "kind": "system", "mixed": true}),
        json!({"seq": 3, "timestampMs": 40, "kind": "message", "mixed": {"a": 1}}),
        json!({"seq": 4, "timestampMs": 50, "kind": "raw", "mixed": []}),
        json!({"seq": 5, "timestampMs": 60, "kind": "message"}),
    ];
    let rw = db.rw_transaction().expect("写事务");
    for (seq, payload) in payloads.into_iter().enumerate() {
        let text = payload.to_string();
        rw.insert(SessionEventRecord {
            event_key: pack_session_event_key("ses-1", seq as u64),
            session_id: "ses-1".to_owned(),
            event: serde_json::value::RawValue::from_string(text).expect("载荷为合法 JSON"),
        })
        .expect("落转录行");
    }
    rw.commit().expect("提交");
}

/// 520 条 checklist 子行（`limit` 上限不钳到查看页 500 的语料）。
fn seed_checklist(db: &Database<'static>, count: u32) {
    let rw = db.rw_transaction().expect("写事务");
    for index in 0..count {
        rw.insert(crate::ChecklistItemRecord::new(
            1,
            index,
            ChecklistItem {
                item: format!("项 {index}"),
                pass: index % 2 == 0,
                evidence: "证据".to_owned(),
            },
        ))
        .expect("落子行");
    }
    rw.commit().expect("提交");
}

fn seed_providers(db: &Database<'static>, count: i64) {
    let rw = db.rw_transaction().expect("写事务");
    for id in 1..=count {
        rw.insert(crate::AgentProviderRecord {
            id,
            name: format!("provider-{id}"),
            protocol: crate::AgentProviderProtocol::Openai,
            base_url: "http://127.0.0.1:9/v1".to_owned(),
            api_key: format!("sk-secret-{id}"),
            models: crate::AgentModelTiers {
                high: "m-high".to_owned(),
                medium: "m-medium".to_owned(),
                low: "m-low".to_owned(),
            },
            context_length: None,
            max_tokens: None,
            web_search: false,
            efforts: crate::AgentEffortTiers::default(),
        })
        .expect("落 provider 行");
    }
    rw.commit().expect("提交");
}

fn spec(value: Value) -> QuerySpec {
    serde_json::from_value(value).unwrap_or_else(|error| panic!("spec 应可解析: {error}"))
}

fn run_ws(db: &Database<'static>, value: Value) -> Result<QueryPage, QueryError> {
    run(db, DbDimension::Workspace, &spec(value))
}

fn matched(db: &Database<'static>, value: Value) -> u64 {
    run_ws(db, value)
        .unwrap_or_else(|error| panic!("查询应成功: {error}"))
        .matched
}

fn err_of(db: &Database<'static>, value: Value) -> String {
    match run_ws(db, value) {
        Err(error) => error.to_string(),
        Ok(page) => panic!("应 Err，实际返回 {} 行", page.records.len()),
    }
}

fn keys_of(page: &QueryPage) -> Vec<Value> {
    page.records
        .iter()
        .map(|record| record.key.clone())
        .collect()
}

fn mixed_of(page: &QueryPage) -> Vec<Value> {
    page.records
        .iter()
        .map(|record| record.value["event"]["mixed"].clone())
        .collect()
}

/// 相位语料库（五条行）。
fn phase_corpus() -> Database<'static> {
    let db = workspace_db();
    seed_phases(&db);
    db
}

// ---------------------------------------------------------------------------
// op 词表：逐格断言 + 词表单点
// ---------------------------------------------------------------------------

#[test]
fn op词表逐格命中量与人工核算相等() {
    let db = phase_corpus();
    struct Case {
        cond: Value,
        expected: u64,
    }
    let cases = vec![
        // `=` / `!=`
        Case {
            cond: json!({"field": "changeId", "op": "=", "value": "chg-a"}),
            expected: 3,
        },
        Case {
            cond: json!({"field": "changeId", "op": "!=", "value": "chg-a"}),
            expected: 2,
        },
        // 大小比较：`>` / `<` 不含边界，`>=` / `<=` 含边界
        Case {
            cond: json!({"field": "timestamp", "op": ">", "value": 300}),
            expected: 2,
        },
        Case {
            cond: json!({"field": "timestamp", "op": ">=", "value": 300}),
            expected: 3,
        },
        Case {
            cond: json!({"field": "timestamp", "op": "<", "value": 300}),
            expected: 2,
        },
        Case {
            cond: json!({"field": "timestamp", "op": "<=", "value": 300}),
            expected: 3,
        },
        Case {
            cond: json!({"field": "phase", "op": "LIKE", "value": "dev-%"}),
            expected: 3,
        },
        Case {
            cond: json!({"field": "changeId", "op": "IN", "value": ["chg-b"]}),
            expected: 2,
        },
        Case {
            cond: json!({"field": "backtrackReason", "op": "IS NULL"}),
            expected: 3,
        },
        Case {
            cond: json!({"field": "backtrackReason", "op": "IS NOT NULL"}),
            expected: 2,
        },
    ];
    assert_eq!(cases.len(), QUERY_OPS.len(), "词表逐格（10 词各一）");
    for Case { cond, expected } in cases {
        let tag = cond["op"].as_str().expect("op").to_owned();
        assert_eq!(
            matched(&db, json!({"table": "phase", "where": [cond.clone()]})),
            expected,
            "op {tag}：命中量与人工核算一致（条件 {cond}）"
        );
    }
}

#[test]
fn 词表十词有序且每词经run均不err() {
    assert_eq!(
        QUERY_OPS,
        [
            "=",
            "!=",
            ">",
            ">=",
            "<",
            "<=",
            "LIKE",
            "IN",
            "IS NULL",
            "IS NOT NULL"
        ],
        "词表有序且逐值固定"
    );
    assert_eq!(DEFAULT_QUERY_LIMIT, 20, "limit 缺省 20");
    assert_eq!(MAX_QUERY_LIMIT, 1000, "limit 上限 1000");
    let db = phase_corpus();
    for op in QUERY_OPS {
        // 值须与字段类型相符（`changeId` 是 string，故大小比较亦取字符串——
        // 本行只断言「词表每词均可解析」，命中量归上一格逐格用例）
        let (field, value) = match op {
            "IN" => ("changeId", json!(["chg-a"])),
            "IS NULL" | "IS NOT NULL" => ("changeId", Value::Null),
            "LIKE" => ("phase", json!("dev-%")),
            ">" | ">=" | "<" | "<=" => ("attempt", json!(1)),
            _ => ("changeId", json!("chg-a")),
        };
        run_ws(
            &db,
            json!({"table": "phase", "where": [{"field": field, "op": op, "value": value}]}),
        )
        .unwrap_or_else(|error| panic!("词表内 op {op} 不应 Err: {error}"));
    }
}

#[test]
fn 未知op逐形态err且文案含全词表() {
    let db = phase_corpus();
    for op in ["contains", "like", "", "   ", "=="] {
        let message = err_of(
            &db,
            json!({"table": "phase", "where": [{"field": "phase", "op": op, "value": "dev-design"}]}),
        );
        assert!(message.contains("未知 op"), "op {op:?} 文案: {message}");
        for word in QUERY_OPS {
            assert!(
                message.contains(word),
                "op {op:?} 的错误文案含词表逐词 {word}: {message}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// where 组合 / 点号下钻
// ---------------------------------------------------------------------------

#[test]
fn where按and组合且缺省或空数组为全量() {
    let db = phase_corpus();
    assert_eq!(
        matched(
            &db,
            json!({"table": "phase", "where": [
                {"field": "changeId", "op": "=", "value": "chg-a"},
                {"field": "backtrackReason", "op": "IS NOT NULL"}
            ]})
        ),
        1,
        "两条件取交集（chg-a 且回跳记因在场）"
    );
    assert_eq!(
        matched(
            &db,
            json!({"table": "phase", "where": [
                {"field": "changeId", "op": "=", "value": "chg-a"},
                {"field": "verdict", "op": "=", "value": "pass"},
                {"field": "attempt", "op": "=", "value": 1}
            ]})
        ),
        2,
        "三条件三重收窄（chg-a / pass / attempt 1 → id 1 与 3）"
    );
    assert_eq!(
        matched(
            &db,
            json!({"table": "phase", "where": [
                {"field": "changeId", "op": "=", "value": "chg-a"},
                {"field": "phase", "op": "LIKE", "value": "dev-%"},
                {"field": "attempt", "op": "=", "value": 1},
                {"field": "verdict", "op": "=", "value": "pass"},
                {"field": "executorSessionId", "op": "IS NULL"}
            ]})
        ),
        1,
        "五条件（超大列表）逐条 AND 收窄到 id 1"
    );
    for value in [
        json!({"table": "phase"}),
        json!({"table": "phase", "where": []}),
    ] {
        let page = run_ws(&db, value.clone()).expect("全量查询应成功");
        assert_eq!(page.matched, 5, "where 缺失 / 空数组 = 全量（{value}）");
        assert_eq!(page.total, 5, "total 恒表全量");
    }
}

#[test]
fn 点号下钻命中嵌套字段且数组下标err() {
    let db = workspace_db();
    seed_events(&db);
    seed_sessions(&db);

    assert_eq!(
        matched(
            &db,
            json!({"table": "session_event", "where": [{"field": "event.kind", "op": "=", "value": "message"}]})
        ),
        3,
        "event.kind 下钻命中三条 message 行"
    );
    assert_eq!(
        matched(
            &db,
            json!({"table": "session", "where": [{"field": "configSnapshot.model", "op": "=", "value": "m-high"}]})
        ),
        1,
        "configSnapshot.model 嵌套命中"
    );

    let message = err_of(
        &db,
        json!({"table": "session_event", "where": [{"field": "event.blocks.0.name", "op": "=", "value": "text"}]}),
    );
    assert!(
        message.contains("下标") && message.contains("不支持下标"),
        "数组下标显式拒绝并给改正指引: {message}"
    );
}

#[test]
fn 点号下钻不存在中段视作null且不err() {
    let db = workspace_db();
    seed_events(&db);
    assert_eq!(
        matched(
            &db,
            json!({"table": "session_event", "where": [{"field": "event.nope", "op": "IS NULL"}]})
        ),
        6,
        "缺路径等价 null → IS NULL 全命中"
    );
    assert_eq!(
        matched(
            &db,
            json!({"table": "session_event", "where": [{"field": "event.nope", "op": "=", "value": "x"}]})
        ),
        0,
        "缺路径 = 恒不命中"
    );
    assert_eq!(
        matched(
            &db,
            json!({"table": "session_event", "where": [{"field": "event.nope", "op": "IS NOT NULL"}]})
        ),
        0,
        "缺路径 IS NOT NULL 不命中"
    );
}

// ---------------------------------------------------------------------------
// LIKE 四形 + 转义 + 大小写
// ---------------------------------------------------------------------------

#[test]
fn like前缀后缀包含与无通配退化为等值() {
    let db = workspace_db();
    seed_changes(&db);
    // 语料五名：dev-design / dev_design / dev%x / Dev-upper / dev-
    for (pattern, expected, tag) in [
        (
            "dev-%",
            2u64,
            "前缀命中 dev-design 与 dev-（`%` 亦匹配空串）",
        ),
        ("%-design", 1, "后缀"),
        ("%sign%", 2, "包含"),
        ("dev-design", 1, "无通配 → 等值"),
        ("%", 5, "单 `%` 匹配全部"),
        ("%%", 5, "`%%` 等价 `%`"),
    ] {
        assert_eq!(
            matched(
                &db,
                json!({"table": "change", "where": [{"field": "name", "op": "LIKE", "value": pattern}]})
            ),
            expected,
            "LIKE {pattern}（{tag}）"
        );
    }
    assert_eq!(
        matched(
            &db,
            json!({"table": "change", "where": [{"field": "name", "op": "LIKE", "value": "Dev-%"}]})
        ),
        1,
        "区分大小写：`Dev-%` 只命中 Dev-upper"
    );
}

#[test]
fn like下划线不作通配且反斜杠转义为字面() {
    let db = workspace_db();
    seed_changes(&db);
    assert_eq!(
        matched(
            &db,
            json!({"table": "change", "where": [{"field": "name", "op": "LIKE", "value": "dev_design"}]})
        ),
        1,
        "`_` 不作单字符通配：只命中字面 dev_design，不命中 dev-design"
    );
    assert_eq!(
        matched(
            &db,
            json!({"table": "change", "where": [{"field": "name", "op": "LIKE", "value": "dev\\%x"}]})
        ),
        1,
        "`\\%` 为字面 `%`"
    );
    assert_eq!(
        matched(
            &db,
            json!({"table": "change", "where": [{"field": "name", "op": "LIKE", "value": "dev\\_design"}]})
        ),
        1,
        "`\\_` 为字面 `_`"
    );
    // null 不匹配任何 LIKE（含 `%` / `%%`）：语料用全 null 列
    // `executorSessionId`（`backtrackReason` 有两行非空，作 null 列会误判）
    let phases = phase_corpus();
    assert_eq!(
        matched(
            &phases,
            json!({"table": "phase", "where": [{"field": "executorSessionId", "op": "IS NULL"}]})
        ),
        5,
        "该列语料全 null（LIKE 断言的 null 侧前提）"
    );
    for pattern in ["%", "%%", "dev-%"] {
        assert_eq!(
            matched(
                &phases,
                json!({"table": "phase", "where": [{"field": "executorSessionId", "op": "LIKE", "value": pattern}]})
            ),
            0,
            "null 列不匹配 LIKE {pattern}"
        );
    }
}

// ---------------------------------------------------------------------------
// IN / IS NULL
// ---------------------------------------------------------------------------

#[test]
fn in逐元素按等值语义且空数组零命中() {
    let db = phase_corpus();
    assert_eq!(
        matched(
            &db,
            json!({"table": "phase", "where": [{"field": "attempt", "op": "IN", "value": [1, 2]}]})
        ),
        5,
        "IN [1,2] 含全部 attempt 1 / 2 行"
    );
    assert_eq!(
        matched(
            &db,
            json!({"table": "phase", "where": [{"field": "attempt", "op": "IN", "value": [2]}]})
        ),
        1,
        "单元素数组"
    );
    let many: Vec<u32> = (0..20).collect();
    assert_eq!(
        matched(
            &db,
            json!({"table": "phase", "where": [{"field": "attempt", "op": "IN", "value": many}]})
        ),
        5,
        "20 元素列表逐元素等值"
    );

    let page = run_ws(
        &db,
        json!({"table": "phase", "where": [{"field": "attempt", "op": "IN", "value": []}]}),
    )
    .expect("空数组不 Err");
    assert_eq!(page.matched, 0, "IN [] 零命中");
    assert!(page.records.is_empty(), "records 空");
    assert_eq!(page.total, 5, "total 照常");

    assert_eq!(
        matched(
            &db,
            json!({"table": "phase", "where": [{"field": "attempt", "op": "IN", "value": [1, null]}]})
        ),
        4,
        "数组含 null 元素按等值语义恒不命中（null 不参与等值）"
    );
}

#[test]
fn in非数组与isnull带值均err() {
    let db = phase_corpus();
    for value in [json!("chg-a"), json!(1), json!({"a": 1}), Value::Null] {
        let message = err_of(
            &db,
            json!({"table": "phase", "where": [{"field": "changeId", "op": "IN", "value": value}]}),
        );
        assert!(
            message.contains("IN") && message.contains("数组"),
            "IN 非数组携期望形状: {message}"
        );
    }
    for (op, value) in [("IS NULL", json!(1)), ("IS NOT NULL", json!("x"))] {
        let message = err_of(
            &db,
            json!({"table": "phase", "where": [{"field": "backtrackReason", "op": op, "value": value}]}),
        );
        assert!(
            message.contains("不接 value"),
            "op {op} 带非 null value 携形状提示: {message}"
        );
    }
    // 显式 null 等价于省略：正向通过
    for op in ["IS NULL", "IS NOT NULL"] {
        run_ws(
            &db,
            json!({"table": "phase", "where": [{"field": "backtrackReason", "op": op, "value": Value::Null}]}),
        )
        .unwrap_or_else(|error| panic!("op {op} 带显式 null 应通过: {error}"));
    }
}

#[test]
fn isnull与isnotnull两态互补且并集为全量() {
    let db = phase_corpus();
    let null_page = run_ws(
        &db,
        json!({"table": "phase", "where": [{"field": "backtrackReason", "op": "IS NULL"}],
               "select": ["id"]}),
    )
    .expect("IS NULL 应成功");
    let not_null_page = run_ws(
        &db,
        json!({"table": "phase", "where": [{"field": "backtrackReason", "op": "IS NOT NULL"}],
               "select": ["id"]}),
    )
    .expect("IS NOT NULL 应成功");
    let null_ids = keys_of(&null_page);
    let not_null_ids = keys_of(&not_null_page);
    for key in &null_ids {
        assert!(!not_null_ids.contains(key), "两态命中集不交: {key}");
    }
    assert_eq!(
        null_ids.len() as u64 + not_null_ids.len() as u64,
        null_page.total,
        "两态并集 == 表全量"
    );
    assert_eq!(null_page.matched, 3, "三条空值行落 IS NULL 侧");
}

#[test]
fn null比较语义六格() {
    let db = phase_corpus();
    for (op, expected) in [
        ("=", 0u64),
        ("!=", 5),
        (">", 0),
        (">=", 0),
        ("<", 0),
        ("<=", 0),
    ] {
        assert_eq!(
            matched(
                &db,
                json!({"table": "phase", "where": [{"field": "backtrackReason", "op": op, "value": Value::Null}]})
            ),
            expected,
            "字段 null 与值 null 的比较（op {op}）"
        );
    }
}

// ---------------------------------------------------------------------------
// 类型校验（错误文案携字段渲染类型）
// ---------------------------------------------------------------------------

#[test]
fn 值类型与字段类型不符逐组合err携渲染类型() {
    let db = phase_corpus();
    for (path, value, render) in [
        ("attempt", json!("1"), "number"),
        ("skipped", json!("true"), "boolean"),
        ("phase", json!(1), "string"),
    ] {
        let message = err_of(
            &db,
            json!({"table": "phase", "where": [{"field": path, "op": "=", "value": value}]}),
        );
        assert!(
            message.contains(path) && message.contains(render),
            "字段 {path} 的错误文案携渲染类型 {render}: {message}"
        );
    }
    // 命名枚举（kind = Text）：接受字符串、拒绝数字
    run_ws(
        &db,
        json!({"table": "phase", "where": [{"field": "verdict", "op": "=", "value": "pass"}]}),
    )
    .expect("verdict 接受字符串");
    let message = err_of(
        &db,
        json!({"table": "phase", "where": [{"field": "verdict", "op": "=", "value": 1}]}),
    );
    assert!(message.contains("Verdict"), "verdict 拒绝数字: {message}");
}

#[test]
fn 嵌套路径不做值类型校验() {
    let db = workspace_db();
    seed_events(&db);
    for value in [json!(1), json!("message"), json!({"a": 1})] {
        run_ws(
            &db,
            json!({"table": "session_event", "where": [{"field": "event.kind", "op": "=", "value": value}]}),
        )
        .unwrap_or_else(|error| panic!("嵌套路径 {value} 不 static 校验: {error}"));
    }
}

// ---------------------------------------------------------------------------
// 排序
// ---------------------------------------------------------------------------

#[test]
fn 排序升降与缺省升序与无order自然序() {
    let db = phase_corpus();
    let ascending = run_ws(
        &db,
        json!({"table": "phase", "order": {"field": "timestamp"}, "select": ["timestamp"]}),
    )
    .expect("升序应成功");
    let asc_values: Vec<i64> = ascending
        .records
        .iter()
        .map(|record| {
            record.value["timestamp"]
                .as_i64()
                .expect("timestamp number")
        })
        .collect();
    assert_eq!(asc_values, vec![100, 200, 300, 400, 500], "升序严格单调");

    let descending = run_ws(
        &db,
        json!({"table": "phase", "order": {"field": "timestamp", "desc": true}, "select": ["timestamp"]}),
    )
    .expect("降序应成功");
    let desc_values: Vec<i64> = descending
        .records
        .iter()
        .map(|record| {
            record.value["timestamp"]
                .as_i64()
                .expect("timestamp number")
        })
        .collect();
    assert_eq!(desc_values, vec![500, 400, 300, 200, 100], "降序严格单调");

    let explicit_false = run_ws(
        &db,
        json!({"table": "phase", "order": {"field": "timestamp", "desc": false}, "select": ["timestamp"]}),
    )
    .expect("显式 desc:false 应成功");
    assert_eq!(
        explicit_false
            .records
            .iter()
            .map(|record| record.value["timestamp"].clone())
            .collect::<Vec<_>>(),
        ascending
            .records
            .iter()
            .map(|record| record.value["timestamp"].clone())
            .collect::<Vec<_>>(),
        "缺 desc ≡ desc:false（升序）"
    );

    // 无 order：主键自然序（与 scan 一致）
    let natural = run_ws(&db, json!({"table": "phase"})).expect("自然序应成功");
    assert_eq!(
        keys_of(&natural),
        vec![json!(1), json!(2), json!(3), json!(4), json!(5)],
        "无 order 为主键自然序"
    );
}

#[test]
fn 排序null恒末位且跨类型定序稳定() {
    let db = workspace_db();
    seed_events(&db);
    let ascending = run_ws(
        &db,
        json!({"table": "session_event", "order": {"field": "event.mixed"}}),
    )
    .expect("按 mixed 升序应成功");
    assert_eq!(
        mixed_of(&ascending),
        vec![
            json!(1),
            json!("s"),
            json!(true),
            json!({"a": 1}),
            json!([]),
            Value::Null
        ],
        "跨类型定序 number < string < bool < object < array < null"
    );

    let descending = run_ws(
        &db,
        json!({"table": "session_event", "order": {"field": "event.mixed", "desc": true}}),
    )
    .expect("按 mixed 降序应成功");
    let desc = mixed_of(&descending);
    assert_eq!(
        desc,
        vec![
            json!([]),
            json!({"a": 1}),
            json!(true),
            json!("s"),
            json!(1),
            Value::Null
        ],
        "降序反转非 null 段"
    );
    assert_eq!(desc.last(), Some(&Value::Null), "null 恒末位且与方向无关");

    // null 列排序：升 / 降序皆末位
    let phases = phase_corpus();
    for desc in [false, true] {
        let page = run_ws(
            &phases,
            json!({"table": "phase", "order": {"field": "backtrackReason", "desc": desc}}),
        )
        .expect("按可空列排序应成功");
        let null_positions: Vec<usize> = page
            .records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.value["backtrackReason"].is_null())
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            null_positions,
            vec![2, 3, 4],
            "desc={desc}: 三条 null 行恒占末三位（与方向无关）"
        );
    }

    // 相等值不锁相对序，只断言集合与页切无重无漏
    let first_page = run_ws(
        &phases,
        json!({"table": "phase", "order": {"field": "verdict"}, "limit": 2, "offset": 0}),
    )
    .expect("首页应成功");
    let second_page = run_ws(
        &phases,
        json!({"table": "phase", "order": {"field": "verdict"}, "limit": 3, "offset": 2}),
    )
    .expect("次页应成功");
    let mut union: Vec<Value> = keys_of(&first_page);
    union.extend(keys_of(&second_page));
    union.sort_by_key(|key| key.as_i64().unwrap_or_default());
    union.dedup();
    assert_eq!(
        union,
        vec![json!(1), json!(2), json!(3), json!(4), json!(5)],
        "两页无重无漏"
    );
}

// ---------------------------------------------------------------------------
// 投影
// ---------------------------------------------------------------------------

#[test]
fn 投影后key恒在且未选字段不出现且缺省为全字段() {
    let full = crate::describe::table(DbDimension::Workspace, "phase")
        .expect("phase 下钻")
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    let db = phase_corpus();

    let projected = run_ws(
        &db,
        json!({"table": "phase", "select": ["id", "phase", "attempt"], "limit": 1}),
    )
    .expect("投影应成功");
    let value = &projected.records[0].value;
    let mut projected_keys: Vec<String> = value
        .as_object()
        .expect("投影为对象")
        .keys()
        .cloned()
        .collect();
    projected_keys.sort();
    assert_eq!(
        projected_keys,
        vec!["attempt".to_owned(), "id".to_owned(), "phase".to_owned()],
        "投影后恰三键（未选字段不出现）"
    );
    assert_eq!(projected.records[0].key, json!(1), "key 恒在");

    let defaulted = run_ws(&db, json!({"table": "phase", "limit": 1})).expect("缺省投影应成功");
    let mut default_keys: Vec<String> = defaulted.records[0]
        .value
        .as_object()
        .expect("缺省为全字段对象")
        .keys()
        .cloned()
        .collect();
    default_keys.sort();
    let mut expected = full.clone();
    expected.sort();
    assert_eq!(
        default_keys, expected,
        "select 缺省 = 全字段（键集与表线名集一致）"
    );
}

#[test]
fn 投影点号路径重建嵌套对象() {
    let db = workspace_db();
    seed_events(&db);
    let page = run_ws(
        &db,
        json!({"table": "session_event",
               "select": ["sessionId", "event.kind", "event.role"],
               "where": [{"field": "event.kind", "op": "=", "value": "message"}],
               "limit": 1}),
    )
    .expect("点号投影应成功");
    let value = &page.records[0].value;
    assert_eq!(value["sessionId"], json!("ses-1"));
    let event = value["event"].as_object().expect("event 重建为对象");
    let mut keys: Vec<&str> = event.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["kind", "role"], "event 只含被选两段");
}

#[test]
fn 投影边界形态() {
    let db = phase_corpus();
    let empty = run_ws(&db, json!({"table": "phase", "select": []})).expect("空投影应成功");
    assert_eq!(
        empty.records[0].value,
        json!({}),
        "select: [] → value 为空对象"
    );
    assert_eq!(empty.records[0].key, json!(1), "key 仍在");

    let duplicated = run_ws(
        &db,
        json!({"table": "phase", "select": ["id", "id"], "limit": 1}),
    )
    .expect("重复投影项应成功");
    assert_eq!(
        duplicated.records[0].value,
        json!({"id": 1}),
        "重复项不重复输出"
    );

    let message = err_of(&db, json!({"table": "phase", "select": ["change_id"]}));
    assert!(
        message.contains("未知字段") && message.contains("changeId"),
        "未知投影字段携候选线名: {message}"
    );

    assert!(
        serde_json::from_value::<QuerySpec>(json!({"table": "phase", "select": "id"})).is_err(),
        "select 非数组 → serde Err（形状在反序列化面拦下）"
    );
}

// ---------------------------------------------------------------------------
// 分页
// ---------------------------------------------------------------------------

#[test]
fn 分页两页并集为全量且缺省limit为20() {
    let db = workspace_db();
    seed_checklist(&db, 45);
    let total = 45u64;

    let first = run_ws(
        &db,
        json!({"table": "checklist_item", "limit": 20, "offset": 0}),
    )
    .expect("首页应成功");
    let second = run_ws(
        &db,
        json!({"table": "checklist_item", "limit": 25, "offset": 20}),
    )
    .expect("次页应成功");
    assert_eq!(first.records.len(), 20, "首页 20 行");
    assert_eq!(second.records.len(), 25, "次页 25 行");
    let mut union: Vec<Value> = keys_of(&first);
    union.extend(keys_of(&second));
    union.sort_by_key(|value| value.to_string());
    union.dedup();
    assert_eq!(
        union.len() as u64,
        total,
        "两页并集 == 全量命中集（无重无漏）"
    );

    let zero_offset = run_ws(
        &db,
        json!({"table": "checklist_item", "offset": 0, "limit": 1}),
    )
    .expect("offset 0 首页应成功");
    assert_eq!(zero_offset.records.len(), 1, "offset = 0 首页");

    let defaulted = run_ws(&db, json!({"table": "checklist_item"})).expect("缺省 limit 应成功");
    assert_eq!(defaulted.records.len(), 20, "limit 缺失 → 20 条");
    assert_eq!(defaulted.matched, total, "matched 仍精确");
}

#[test]
fn 分页越界逐形态err不钳位() {
    let db = workspace_db();
    seed_checklist(&db, 3);
    for value in [json!(MAX_QUERY_LIMIT + 1), json!(u32::MAX)] {
        let mut payload = json!({"table": "checklist_item"});
        payload["limit"] = value.clone();
        let message = err_of(&db, payload);
        assert!(
            message.contains("limit") && message.contains("1000"),
            "limit = {value} 的错误文案含上限（不钳位、不返回装成功的页）: {message}"
        );
    }
}

#[test]
fn 分页恰上限一千不复用查看页五百且offset超界空页() {
    let db = workspace_db();
    seed_checklist(&db, 520);
    let page =
        run_ws(&db, json!({"table": "checklist_item", "limit": 1000})).expect("恰上限应成功");
    assert_eq!(page.matched, 520, "matched 精确");
    assert_eq!(page.total, 520, "total 精确");
    assert_eq!(page.records.len(), 520, "returned == min(matched, 1000)");
    assert!(
        page.records.len() > 500,
        "未复用查看页 500 上限（钳到 500 即本行红）"
    );

    // `limit = 0` 是合法页长（只钉上限，无下界）：空页但三元仍精确
    let zero = run_ws(&db, json!({"table": "checklist_item", "limit": 0}))
        .expect("limit = 0 不 Err（页长零合法）");
    assert_eq!(zero.records.len(), 0, "limit = 0 → returned == 0");
    assert_eq!(zero.matched, 520, "matched 仍精确");
    assert_eq!(zero.total, 520, "total 仍精确");

    let beyond = run_ws(
        &db,
        json!({"table": "checklist_item", "offset": 600, "limit": 10}),
    )
    .expect("offset 超界不 Err");
    assert!(beyond.records.is_empty(), "offset > matched → 空页");
    assert_eq!(beyond.total, 520, "total 照常");
    assert_eq!(beyond.matched, 520, "matched 照常");
}

// ---------------------------------------------------------------------------
// 三元口径
// ---------------------------------------------------------------------------

#[test]
fn 三元口径四场景可区分且恒有returned不超matched不超total() {
    let db = workspace_db();
    seed_phases(&db);

    let unfiltered_small = run_ws(&db, json!({"table": "phase", "limit": 2})).expect("场景一");
    assert_eq!(
        (
            unfiltered_small.total,
            unfiltered_small.matched,
            unfiltered_small.records.len()
        ),
        (5, 5, 2),
        "无过滤 + 小 limit：total == matched > returned"
    );

    let filtered_big = run_ws(
        &db,
        json!({"table": "phase", "where": [{"field": "changeId", "op": "=", "value": "chg-a"}], "limit": 100}),
    )
    .expect("场景二");
    assert_eq!(
        (
            filtered_big.total,
            filtered_big.matched,
            filtered_big.records.len()
        ),
        (5, 3, 3),
        "有过滤 + 大 limit：total > matched == returned"
    );

    let filtered_offset = run_ws(
        &db,
        json!({"table": "phase", "where": [{"field": "changeId", "op": "=", "value": "chg-a"}], "offset": 2, "limit": 100}),
    )
    .expect("场景三");
    assert_eq!(
        (
            filtered_offset.total,
            filtered_offset.matched,
            filtered_offset.records.len()
        ),
        (5, 3, 1),
        "有过滤 + offset：returned < matched"
    );

    let empty = workspace_db();
    let empty_page = run_ws(&empty, json!({"table": "phase"})).expect("场景四");
    assert_eq!(
        (
            empty_page.total,
            empty_page.matched,
            empty_page.records.len()
        ),
        (0, 0, 0),
        "空表三元全 0"
    );

    for page in [unfiltered_small, filtered_big, filtered_offset] {
        assert!(
            page.records.len() as u64 <= page.matched && page.matched <= page.total,
            "恒有 returned ≤ matched ≤ total"
        );
    }
}

// ---------------------------------------------------------------------------
// spec 形态与错误面
// ---------------------------------------------------------------------------

#[test]
fn spec未知表与跨维度表名逐形态err() {
    let workspace = workspace_db();
    for name in ["phases", "", "   ", "Phase"] {
        let message = err_of(&workspace, json!({"table": name}));
        assert!(message.contains("未知表"), "表名 {name:?} 文案: {message}");
        assert!(message.contains("phase"), "候选含真实表名: {message}");
    }
    let message = err_of(&workspace, json!({"table": "store_meta"}));
    assert!(
        message.contains("未知表"),
        "内部治理记录未注册（不入查询面）: {message}"
    );

    let global = global_db();
    seed_providers(&global, 2);
    let error = run(&global, DbDimension::User, &spec(json!({"table": "phase"})))
        .expect_err("跨维度表名应 Err");
    assert!(
        error.to_string().contains("workspace"),
        "文案注明维度归属: {error}"
    );
    // 本库候选 = user 维度三表
    assert!(
        error.to_string().contains("agent_provider"),
        "候选列本库表名: {error}"
    );
}

#[test]
fn spec未知字段逐形态err携候选线名() {
    let db = phase_corpus();
    for path in ["change_id", "ChangeId", "changeIdd"] {
        let message = err_of(
            &db,
            json!({"table": "phase", "where": [{"field": path, "op": "=", "value": "x"}]}),
        );
        assert!(
            message.contains(path) && message.contains("changeId"),
            "未知字段 {path} 携候选线名: {message}"
        );
    }
    let message = err_of(
        &db,
        json!({"table": "phase", "order": {"field": "change_id"}}),
    );
    assert!(
        message.contains("未知字段") && message.contains("changeId"),
        "order.field 同形校验: {message}"
    );
}

#[test]
fn spec缺省语义与未知键与越界字面逐形态() {
    let parsed = spec(json!({"table": "phase"}));
    assert!(parsed.conds.is_empty(), "缺 where → []");
    assert!(parsed.order.is_none(), "缺 order → None");
    assert!(parsed.select.is_none(), "缺 select → None");
    assert_eq!(parsed.offset, 0, "缺 offset → 0");
    assert_eq!(parsed.limit, 20, "缺 limit → 20");

    for payload in [
        json!({"table": "phase", "orderBy": "id"}),
        json!({"table": "phase", "scope": "workspace"}),
    ] {
        assert!(
            serde_json::from_value::<QuerySpec>(payload.clone()).is_err(),
            "未知键 {payload} → serde Err（deny_unknown_fields）"
        );
    }
    for payload in [
        json!({"table": "phase", "limit": -1}),
        json!({"table": "phase", "limit": 4_294_967_296u64}),
        json!({"table": "phase", "offset": -1}),
        json!({"where": []}),
    ] {
        assert!(
            serde_json::from_value::<QuerySpec>(payload.clone()).is_err(),
            "非法字面 {payload} → serde Err"
        );
    }
    // 形状不符（`where` 须数组 / `order` 须对象且字段必填）：静默按缺省放行会让
    // 「写错形状」表现为零命中
    for payload in [
        json!({"table": "phase", "where": "changeId"}),
        json!({"table": "phase", "where": [{"op": "="}]}),
        json!({"table": "phase", "order": "timestamp"}),
        json!({"table": "phase", "order": {"desc": true}}),
    ] {
        assert!(
            serde_json::from_value::<QuerySpec>(payload.clone()).is_err(),
            "形状不符 {payload} → serde Err"
        );
    }
}

#[test]
fn queryerror_display两臂前缀() {
    assert_eq!(
        QueryError::Invalid("未知表".to_owned()).to_string(),
        "invalid: 未知表"
    );
    assert_eq!(
        QueryError::Db("redb 失败".to_owned()).to_string(),
        "db: redb 失败"
    );
    assert_ne!(
        QueryError::Invalid("x".to_owned()),
        QueryError::Db("x".to_owned()),
        "两变体分立（同载荷亦可区分）"
    );
    let error: &dyn std::error::Error = &QueryError::Invalid("x".to_owned());
    assert_eq!(error.to_string(), "invalid: x", "std::error::Error 面可达");
}

// ---------------------------------------------------------------------------
// 只读：查询前后计数不变
// ---------------------------------------------------------------------------

#[test]
fn 查询全程零写面_三查询前后表计数不变() {
    let db = workspace_db();
    seed_phases(&db);
    seed_changes(&db);
    seed_events(&db);

    let before = list_models(&db, DbDimension::Workspace).expect("查询前清单");
    for value in [
        json!({"table": "phase"}),
        json!({"table": "change", "where": [{"field": "name", "op": "LIKE", "value": "dev-%"}]}),
        json!({"table": "session_event", "order": {"field": "event.mixed"}, "limit": 3}),
    ] {
        run_ws(&db, value).expect("查询应成功");
    }
    let after = list_models(&db, DbDimension::Workspace).expect("查询后清单");
    assert_eq!(before, after, "三查询前后逐表计数不变（只读零写面）");
    let phase_count = after
        .iter()
        .find(|model| model.name == "phase")
        .expect("phase 行在清单")
        .count;
    assert_eq!(phase_count, 5, "相位行数不变");
    assert!(
        !after.iter().any(|model| model.name == "store_meta"),
        "store_meta 未被写入 / 未变可见"
    );
}

#[test]
fn 结果记录信封两字段形态() {
    let db = phase_corpus();
    let page = run_ws(&db, json!({"table": "phase", "limit": 1})).expect("查询应成功");
    let record: &RecordEnvelope = &page.records[0];
    assert_eq!(record.key, json!(1), "key 为记录主键 JSON 值");
    assert!(
        record.value.is_object(),
        "value 为记录本体 JSON 值（非二进制）"
    );
    assert_eq!(
        record.value["id"],
        json!(1),
        "value 与 key 同源（同一记录）"
    );
}
