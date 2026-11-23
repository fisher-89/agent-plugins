//! `describe.rs`（表 / 字段说明的机械派生）的单元测试：表清单两维度行集 /
//! 单表全字段 / 字段索引三元组 / 说明截断 / 类型渲染分类器 / 线名 ratchet。
//!
//! 零 mock：被测面是进程内纯派生链（注册表 + `specta` 类型图 +
//! `specta_serde::Format` + 纯 formatter），无 IO / 配置 / 子进程 / 网络；
//! `native_db` 用 `create_in_memory` 真内存库 + 真 `rw.insert` 播种（计数面需
//! 真实模型登记，替身会让 `count` 断言空转）。

use native_db::Database;
use specta::datatype::{DataType, Enum, List, Map, NamedDataType, Primitive, Reference, Struct};
use specta::Types;

use agent::{AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunStatus};

use workflow::model::{ChecklistItem, Verdict};
use workflow::state::{
    RunEdgeEntry, RunEdgeKind, RunNodeEntry, RunStartCommand, RunStepKind, RunStepStatus,
    StepCommand, StepKind,
};

use crate::describe::{
    all_candidates, candidates, dimension_name, field_index, field_kind, render_type, schema_of,
    table, tables, truncate_doc, FieldKind,
};
use crate::envelope::entries;
use ::notification::{NotificationKind, NotificationLevel};

use crate::model::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, AgentRunRecord, ChangeRecord, ChecklistItemRecord, ExploreRecord,
    NotificationRecord, PhaseRecord, RunEdgeRecord, RunNodeRecord, RunRecord,
    SessionConfigSnapshot, SessionEventRecord, SessionRecord, StepRecord, WorkspaceRecord,
};
use crate::store::{global_models, workspace_models, DbDimension};
use crate::QueryError;

// ---------------------------------------------------------------------------
// 装置：内存库（真模型组 / 真写入）+ 记录字面构造
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

fn phase_record(id: i64, change_id: &str, phase: &str, attempt: u32) -> PhaseRecord {
    PhaseRecord {
        id,
        change_id: change_id.to_owned(),
        phase: phase.to_owned(),
        attempt,
        verdict: Verdict::Pass,
        report: "评估报告".to_owned(),
        skipped: false,
        stale: false,
        backtrack_to: None,
        backtrack_reason: None,
        executor_session_id: None,
        evaluator_session_id: None,
        decision_session_id: None,
        start_at: None,
        timestamp: 1727000000000,
    }
}

fn session_record(id: &str) -> SessionRecord {
    SessionRecord {
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
    }
}

fn provider_record(id: i64, name: &str) -> AgentProviderRecord {
    AgentProviderRecord {
        id,
        name: name.to_owned(),
        protocol: AgentProviderProtocol::Openai,
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        api_key: format!("sk-{name}"),
        models: AgentModelTiers {
            high: "m-high".to_owned(),
            medium: "m-medium".to_owned(),
            low: "m-low".to_owned(),
        },
        context_length: Some(128_000),
        max_tokens: None,
        web_search: false,
        efforts: crate::AgentEffortTiers::default(),
    }
}

fn raw_event(seq: u64) -> AgentEvent {
    AgentEvent {
        turn_id: None,
        seq,
        timestamp_ms: 1727000000000 + seq as i64,
        kind: AgentEventKind::Raw {
            event_type: "mystery".to_owned(),
            raw_json: format!(r#"{{"type":"mystery","seq":{seq}}}"#),
        },
    }
}

/// 15 张表各一条真实记录（ratchet 的真 serde 键集来源）。表名与记录一一对应，
/// 顺序即注册表登记序。
fn sample_records() -> Vec<(&'static str, serde_json::Value)> {
    let node_entry = RunNodeEntry {
        phase: "dev-design".to_owned(),
        attempt: 1,
        step: RunStepKind::Executor,
        instance_no: 0,
        status: RunStepStatus::Passed,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at: 1,
        ended_at: Some(2),
        token_input: None,
        token_output: None,
    };
    let edge_entry = RunEdgeEntry {
        from_node_key: "dev-design:1:executor:0".to_owned(),
        to_node_key: "test-gen:1:executor:0".to_owned(),
        edge_kind: RunEdgeKind::Forward,
    };
    let records: Vec<(&'static str, serde_json::Value)> = vec![
        (
            "workspace",
            value_of(WorkspaceRecord::from_root("C:/ws-ratchet", 1000)),
        ),
        ("agent_provider", value_of(provider_record(1, "ratchet"))),
        (
            "agent_instance",
            value_of(AgentInstanceRecord::new(
                "实例".to_owned(),
                AgentEngineKind::Sdk,
                Some(1),
            )),
        ),
        (
            "notification",
            value_of(NotificationRecord {
                id: "run:run-ratchet:finished".to_owned(),
                kind: NotificationKind::Event,
                level: NotificationLevel::Success,
                message: "变更「ratchet」已完成全部相位".to_owned(),
                body: None,
                root: Some("C:/ws-ratchet".to_owned()),
                source_ref: Some("chg-ratchet".to_owned()),
                created_at: 1000,
                read_at: None,
            }),
        ),
        (
            "agent_run",
            value_of(AgentRunRecord {
                id: 1,
                session_id: Some("ses-ratchet".to_owned()),
                status: AgentRunStatus::Completed,
                started_at: 1,
                finished_at: Some(2),
                num_turns: Some(1),
                cost_usd: None,
                duration_ms: Some(3),
                error: None,
            }),
        ),
        ("session", value_of(session_record("ses-ratchet"))),
        (
            "session_event",
            value_of(SessionEventRecord::new("ses-ratchet", raw_event(0))),
        ),
        (
            "explore",
            value_of(ExploreRecord::new("C:/ws-ratchet", "笔记", 1000)),
        ),
        (
            "change",
            value_of(ChangeRecord::new(
                "chg-ratchet",
                "ratchet",
                "requirement",
                1000,
                None,
                None,
            )),
        ),
        (
            "phase",
            value_of(phase_record(1, "chg-ratchet", "dev-design", 1)),
        ),
        (
            "checklist_item",
            value_of(ChecklistItemRecord::new(
                1,
                0,
                ChecklistItem {
                    item: "检查项".to_owned(),
                    pass: true,
                    evidence: "证据".to_owned(),
                },
            )),
        ),
        (
            "step",
            value_of(StepRecord::new(&StepCommand {
                run_id: "run-ratchet".to_owned(),
                change_id: "chg-ratchet".to_owned(),
                step_kind: StepKind::PhaseNext,
                status: "ok".to_owned(),
                summary: "摘要".to_owned(),
                reference: None,
                timestamp: 1,
            })),
        ),
        (
            "run",
            value_of(RunRecord::new(&RunStartCommand {
                run_id: "run-ratchet".to_owned(),
                change_id: "chg-ratchet".to_owned(),
                started_at: 1,
            })),
        ),
        (
            "run_node",
            value_of(RunNodeRecord::new(
                "run-ratchet",
                "chg-ratchet",
                0,
                &node_entry,
                3,
            )),
        ),
        (
            "run_edge",
            value_of(RunEdgeRecord::new(
                "run-ratchet",
                "chg-ratchet",
                0,
                &edge_entry,
                3,
            )),
        ),
    ];
    assert_eq!(records.len(), entries().len(), "15 张表逐表一条样本");
    records
}

fn value_of<T: serde::Serialize>(record: T) -> serde_json::Value {
    serde_json::to_value(record).expect("记录可序列化为 JSON 值")
}

fn json_keys(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("记录 JSON 为对象")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn field_names(table_doc: &crate::TableDoc) -> Vec<&str> {
    table_doc
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect()
}

const WORKSPACE_TABLES: [&str; 11] = [
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
];

const USER_TABLES: [&str; 4] = [
    "workspace",
    "agent_provider",
    "agent_instance",
    "notification",
];

// ---------------------------------------------------------------------------
// 表清单：两维度的行集 / 序 / 摘要 / 计数
// ---------------------------------------------------------------------------

#[test]
fn 表清单两维度行集与登记序且摘要非空计数与播种量一致() {
    let workspace = workspace_db();
    {
        let rw = workspace.rw_transaction().expect("写事务");
        rw.insert(phase_record(1, "chg-a", "dev-design", 1))
            .expect("落相位行 1");
        rw.insert(phase_record(2, "chg-a", "dev-design", 2))
            .expect("落相位行 2");
        rw.insert(ChangeRecord::new(
            "chg-a",
            "a",
            "requirement",
            1000,
            None,
            None,
        ))
        .expect("落建档行");
        rw.insert(session_record("ses-a")).expect("落会话行");
        rw.commit().expect("提交");
    }
    let global = global_db();
    {
        let rw = global.rw_transaction().expect("写事务");
        rw.insert(provider_record(1, "one"))
            .expect("落 provider 行");
        rw.commit().expect("提交");
    }

    for (db, dimension, expected) in [
        (
            &workspace,
            DbDimension::Workspace,
            WORKSPACE_TABLES.to_vec(),
        ),
        (&global, DbDimension::User, USER_TABLES.to_vec()),
    ] {
        let docs = tables(db, dimension).expect("表清单应成功");
        assert_eq!(
            docs.iter()
                .map(|doc| doc.table.as_str())
                .collect::<Vec<_>>(),
            expected,
            "表名逐值有序且序 = 注册表登记序"
        );
        for doc in &docs {
            assert_eq!(doc.dimension, dimension, "每行维度即入参维度");
            assert!(
                !doc.summary.trim().is_empty(),
                "表 {} 的 summary 非空（struct doc 派生，零手写）",
                doc.table
            );
        }
        let count_of = |table: &str| {
            docs.iter()
                .find(|doc| doc.table == table)
                .unwrap_or_else(|| panic!("表清单应含 {table}"))
                .count
        };
        match dimension {
            DbDimension::Workspace => {
                assert_eq!(count_of("phase"), 2, "phase 计数与播种量一致");
                assert_eq!(count_of("change"), 1, "change 计数与播种量一致");
                assert_eq!(count_of("session"), 1, "session 计数与播种量一致");
                assert_eq!(count_of("run"), 0, "零播种表计数 0");
                assert_eq!(count_of("run_edge"), 0, "零播种表计数 0");
            }
            DbDimension::User => {
                assert_eq!(count_of("agent_provider"), 1, "provider 计数与播种量一致");
                assert_eq!(count_of("workspace"), 0, "零播种表计数 0");
                assert_eq!(count_of("agent_instance"), 0, "零播种表计数 0");
            }
        }
    }
}

#[test]
fn 清单形态只含四字段且字段说明只经下钻供给() {
    let db = workspace_db();
    let mut docs = tables(&db, DbDimension::Workspace).expect("表清单应成功");
    assert!(!docs.is_empty(), "非空清单");
    let first = docs.remove(0);
    // 形态侧证：解构即全部字段（`table` / `dimension` / `count` / `summary`），
    // 清单行无字段清单载体——字段说明只经 `describe::table` 供给
    let crate::ModelDoc {
        table,
        dimension,
        count,
        summary,
    } = first;
    assert_eq!(table, "agent_run");
    assert_eq!(dimension, DbDimension::Workspace);
    assert_eq!(count, 0);
    assert!(!summary.is_empty());
}

#[test]
fn store_meta与内嵌命名类型不出现在任一维度表名集() {
    let workspace = workspace_db();
    let global = global_db();
    for (db, dimension) in [
        (&workspace, DbDimension::Workspace),
        (&global, DbDimension::User),
    ] {
        let names: Vec<String> = tables(db, dimension)
            .expect("表清单应成功")
            .into_iter()
            .map(|doc| doc.table)
            .collect();
        assert!(
            !names.iter().any(|name| name == "store_meta"),
            "{dimension:?}: 库级格式标记不入表清单: {names:?}"
        );
        for embedded in [
            "AgentModelTiers",
            "SessionConfigSnapshot",
            "ChangeActivePhase",
        ] {
            assert!(
                !names.iter().any(|name| name == embedded),
                "{dimension:?}: 内嵌命名类型 {embedded} 不入表清单（注册外过滤）"
            );
        }
    }
}

#[test]
fn 空库两维度仍全列且计数全零() {
    assert_eq!(
        tables(&workspace_db(), DbDimension::Workspace)
            .expect("空库表清单应成功")
            .iter()
            .map(|doc| (doc.table.as_str(), doc.count))
            .collect::<Vec<_>>(),
        WORKSPACE_TABLES
            .iter()
            .map(|name| (*name, 0u64))
            .collect::<Vec<_>>(),
        "空库仍列 11 行且计数全 0（与 list_models 的「计数 0 也列出」同口径）"
    );
    assert_eq!(
        tables(&global_db(), DbDimension::User)
            .expect("空库表清单应成功")
            .iter()
            .map(|doc| (doc.table.as_str(), doc.count))
            .collect::<Vec<_>>(),
        USER_TABLES
            .iter()
            .map(|name| (*name, 0u64))
            .collect::<Vec<_>>(),
        "user 维度空库仍列 4 行且计数全 0"
    );
}

#[test]
fn 维度入参决定输出_模型组不匹配即db错误不panic() {
    // 内存库按 workspace 模型组打开，但以 User 维度查询：维度是注册表过滤入参
    // （不是从库里探测出来的），故输出仍是 user 四行；而 user 四张表不在该库
    // 模型组里 → 计数面显式 `QueryError::Db`（零 panic、零静默 0）。生产不可达：
    // `Store` 打开点即锁定维度与模型组，两面对齐。
    let workspace_group = workspace_db();
    match tables(&workspace_group, DbDimension::User) {
        Err(QueryError::Db(message)) => {
            assert!(
                message.contains("table") || message.contains("Table"),
                "缺失模型表的错误串携库层语境: {message}"
            );
        }
        Err(other) => panic!("应为 Db 变体（库层失败），实际: {other:?}"),
        Ok(docs) => assert_eq!(
            docs.iter()
                .map(|doc| doc.table.as_str())
                .collect::<Vec<_>>(),
            USER_TABLES.to_vec(),
            "若库层容忍缺表，则行集仍为 user 四行（维度过滤不探测库）"
        ),
    }
}

// ---------------------------------------------------------------------------
// 单表下钻：字段线名 / 渲染类型 / 说明
// ---------------------------------------------------------------------------

#[test]
fn phase表十五字段线名序与渲染类型与说明() {
    let doc = table(DbDimension::Workspace, "phase").expect("phase 在册");
    assert_eq!(doc.table, "phase");
    assert_eq!(doc.dimension, DbDimension::Workspace);
    assert!(!doc.summary.is_empty(), "表介绍非空");
    assert_eq!(
        field_names(&doc),
        vec![
            "id",
            "changeId",
            "phase",
            "attempt",
            "verdict",
            "report",
            "skipped",
            "stale",
            "backtrackTo",
            "backtrackReason",
            "executorSessionId",
            "evaluatorSessionId",
            "decisionSessionId",
            "startAt",
            "timestamp",
        ],
        "15 字段逐线名（camelCase，序 = 源码序）"
    );
    let ty_of = |name: &str| {
        doc.fields
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("phase 应有字段 {name}"))
            .ty
            .as_str()
    };
    assert_eq!(ty_of("id"), "number");
    assert_eq!(ty_of("changeId"), "string");
    assert_eq!(ty_of("skipped"), "boolean");
    assert_eq!(ty_of("verdict"), "Verdict", "命名枚举取类型名");
    assert_eq!(ty_of("backtrackTo"), "string | null", "可空渲染");
    assert_eq!(ty_of("startAt"), "number | null", "可空数值渲染");
    for field in &doc.fields {
        assert!(
            !field.doc.trim().is_empty(),
            "字段 {} 的 doc 非空（`///` 派生）",
            field.name
        );
    }
}

#[test]
fn session_event三字段手写schema线名与类型与说明() {
    let doc = table(DbDimension::Workspace, "session_event").expect("session_event 在册");
    assert_eq!(field_names(&doc), vec!["eventKey", "sessionId", "event"]);
    let ty_of = |name: &str| {
        doc.fields
            .iter()
            .find(|field| field.name == name)
            .expect("字段在场")
            .ty
            .as_str()
    };
    assert_eq!(ty_of("eventKey"), "string", "打包键线格式为十六进制串");
    assert_eq!(ty_of("sessionId"), "string");
    assert_eq!(
        ty_of("event"),
        "object",
        "opaque 载荷渲染为 object（可点号下钻）"
    );
    assert!(!doc.summary.is_empty(), "手写 schema 亦供表介绍");
    for field in &doc.fields {
        assert!(!field.doc.trim().is_empty(), "字段 {} 说明非空", field.name);
    }
    assert_eq!(doc.fields.len(), 3, "恰三字段");
}

#[test]
fn 未知表与跨维度表名均err携候选() {
    let error = table(DbDimension::Workspace, "phases").expect_err("拼写错误应 Err");
    let QueryError::Invalid(message) = error else {
        panic!("未知表应为 Invalid 变体");
    };
    assert!(
        message.contains("未知表: phases"),
        "文案携入参名: {message}"
    );
    assert!(
        message.contains("phase") && message.contains("run_edge"),
        "候选列本维度表名: {message}"
    );

    // 跨维度表名：user 维度查 workspace 表 → Err + 本维度候选（不静默成功）
    let error = table(DbDimension::User, "phase").expect_err("跨维度表名应 Err");
    let message = error.to_string();
    assert!(message.contains("phase"), "文案携表名: {message}");
    for candidate in USER_TABLES {
        assert!(
            message.contains(candidate),
            "候选含 user 维度表 {candidate}: {message}"
        );
    }
    assert!(
        !message.contains("run_node"),
        "跨维度候选不列 workspace 表名: {message}"
    );
}

#[test]
fn 表名边界形态逐形态err零归一() {
    let long = "字".repeat(1200);
    for name in ["", "   ", "Phase", "phase ", &long, "a\nb", "a\0b🚀"] {
        let error =
            table(DbDimension::Workspace, name).expect_err("零大小写归一、零 trim → 逐形态 Err");
        let message = error.to_string();
        assert!(
            matches!(error, QueryError::Invalid(_)),
            "表名 {name:?} 走 Invalid 变体"
        );
        assert!(
            message.contains("候选") || message.contains("未知表"),
            "表名 {name:?} 的 Err 携候选: {message}"
        );
    }
}

#[test]
fn 说明截断五形态() {
    // ① 句读截断（含该字符）
    assert_eq!(
        truncate_doc("第一句介绍。第二句被丢掉；第三段"),
        "第一句介绍。",
        "取首个句读并含该字符"
    );
    assert_eq!(
        truncate_doc("一段\n下一行"),
        "一段",
        "换行亦是句读（尾部空白剥离）"
    );
    // ② 无句读且超 120 字 → 截至 120 字 + 省略号
    let long = "文".repeat(200);
    let truncated = truncate_doc(&long);
    assert_eq!(truncated.chars().count(), 121, "120 字 + 省略号");
    assert!(truncated.ends_with('…'), "补省略号");
    assert_eq!(
        &truncated[..truncated.len() - '…'.len_utf8()],
        &long[..120 * 3]
    );
    // ③ 恰 120 字不补省略号
    let exact = "字".repeat(120);
    assert_eq!(truncate_doc(&exact), exact, "恰上限原样返回（零省略号）");
    // ④ 尾部空白剥离
    assert_eq!(truncate_doc("  说明文本。  "), "说明文本。", "首尾空白剥离");
    // ⑤ 句读位 > 120 字 → 走字数截断（句读不被采信）
    let late = format!("{}{}", "铺".repeat(150), "。");
    let truncated = truncate_doc(&late);
    assert!(truncated.ends_with('…'), "句读过远不采信: {truncated}");
    assert_eq!(truncated.chars().count(), 121);
    // 括号平衡收口：句读截断落在未闭合括号内 → 回退到开括号之前
    assert_eq!(
        truncate_doc("介绍（半句。后文"),
        "介绍",
        "未闭合括号不留残句"
    );
}

// ---------------------------------------------------------------------------
// 类型渲染 / 比较类分类器（同一棵 match 的两半）
// ---------------------------------------------------------------------------

#[test]
fn 类型渲染与比较类分类器九形态() {
    let mut types = Types::default();
    let struct_named = NamedDataType::new("内嵌结构", &mut types, |_, named| {
        named.ty = Some(DataType::Struct(Struct::unit()));
    });
    let enum_named = NamedDataType::new("Verdict", &mut types, |_, named| {
        named.ty = Some(DataType::Enum(Enum::default()));
    });

    struct Case {
        tag: &'static str,
        ty: DataType,
        render: &'static str,
        kind: FieldKind,
    }
    let cases = vec![
        Case {
            tag: "bool",
            ty: DataType::Primitive(Primitive::bool),
            render: "boolean",
            kind: FieldKind::Bool,
        },
        Case {
            tag: "str",
            ty: DataType::Primitive(Primitive::str),
            render: "string",
            kind: FieldKind::Text,
        },
        Case {
            tag: "u32",
            ty: DataType::Primitive(Primitive::u32),
            render: "number",
            kind: FieldKind::Number,
        },
        Case {
            tag: "f64",
            ty: DataType::Primitive(Primitive::f64),
            render: "number",
            kind: FieldKind::Number,
        },
        Case {
            tag: "nullable",
            ty: DataType::Nullable(Box::new(DataType::Primitive(Primitive::str))),
            render: "string | null",
            kind: FieldKind::Text,
        },
        Case {
            tag: "list",
            ty: DataType::List(List::new(DataType::Primitive(Primitive::str))),
            render: "string[]",
            kind: FieldKind::Object,
        },
        Case {
            tag: "map",
            ty: DataType::Map(Map::new(
                DataType::Primitive(Primitive::str),
                DataType::Primitive(Primitive::u32),
            )),
            render: "{ [key: string]: number }",
            kind: FieldKind::Object,
        },
        Case {
            tag: "enum",
            ty: DataType::Enum(Enum::default()),
            render: "string",
            kind: FieldKind::Text,
        },
        Case {
            tag: "struct",
            ty: DataType::Struct(Struct::unit()),
            render: "object",
            kind: FieldKind::Object,
        },
        Case {
            tag: "named-enum",
            ty: DataType::Reference(enum_named.reference(Vec::new())),
            render: "Verdict",
            kind: FieldKind::Text,
        },
        Case {
            tag: "named-struct",
            ty: DataType::Reference(struct_named.reference(Vec::new())),
            render: "内嵌结构",
            kind: FieldKind::Object,
        },
        Case {
            tag: "opaque",
            ty: DataType::Reference(Reference::opaque("payload")),
            render: "object",
            kind: FieldKind::Unknown,
        },
    ];
    for Case {
        tag,
        ty,
        render,
        kind,
    } in cases
    {
        assert_eq!(render_type(&ty, &types), render, "{tag}: 渲染文本");
        assert_eq!(field_kind(&ty, &types), kind, "{tag}: 比较类");
    }

    // 命名类型缺失（引用未注册）→ 渲染 unknown（不 panic）
    let mut holder = Types::default();
    let orphan = NamedDataType::new("孤儿", &mut holder, |_, named| {
        named.ty = Some(DataType::Struct(Struct::unit()));
    });
    let unrelated = Types::default();
    assert_eq!(
        render_type(
            &DataType::Reference(orphan.reference(Vec::new())),
            &unrelated
        ),
        "unknown",
        "命名类型缺失时渲染 unknown"
    );
    assert_eq!(
        field_kind(
            &DataType::Reference(orphan.reference(Vec::new())),
            &unrelated
        ),
        FieldKind::Unknown,
        "命名类型缺失时比较类 Unknown"
    );
}

#[test]
fn 字段索引与渲染类型同源且嵌套路径归unknown() {
    let index = field_index("phase").expect("phase 字段索引");
    assert!(index.names().contains(&"changeId"), "含 camelCase 线名");
    assert!(
        !index.names().contains(&"change_id"),
        "线名集不含蛇形名（`Format` 已应用）"
    );
    for (path, kind, render) in [
        ("attempt", FieldKind::Number, "number"),
        ("phase", FieldKind::Text, "string"),
        ("skipped", FieldKind::Bool, "boolean"),
        ("verdict", FieldKind::Text, "Verdict"),
    ] {
        assert_eq!(index.kind_of(path), kind, "{path} 的比较类");
        assert_eq!(index.render_of(path), Some(render), "{path} 的渲染文本");
    }
    assert_eq!(
        index.kind_of("event.blocks"),
        FieldKind::Unknown,
        "嵌套路径归 Unknown（不做静态深校验）"
    );
    assert_eq!(index.render_of("event.blocks"), None, "未知路径无渲染文本");
    assert_eq!(index.kind_of("不存在的字段"), FieldKind::Unknown);

    match field_index("phases") {
        Err(error) => assert!(error.to_string().contains("未知表"), "文案: {error}"),
        Ok(_) => panic!("未知表字段索引应 Err"),
    }
}

#[test]
fn session_event字段索引三字段与手写schema同源() {
    let index = field_index("session_event").expect("session_event 字段索引");
    assert_eq!(index.names(), vec!["eventKey", "sessionId", "event"]);
    assert_eq!(index.kind_of("eventKey"), FieldKind::Text);
    assert_eq!(
        index.kind_of("event"),
        FieldKind::Unknown,
        "opaque 载荷不校验值形状"
    );
    assert_eq!(index.render_of("event"), Some("object"));
}

// ---------------------------------------------------------------------------
// ratchet：说明非空 + 线名集 ↔ 真实 serde 键集双向一致
// ---------------------------------------------------------------------------

#[test]
fn ratchet十五表每字段说明非空且线名集与serde键集双向一致() {
    for (table_name, value) in sample_records() {
        let dimension = crate::dimension_of(table_name).expect("样本表名在注册表内");
        let doc = table(dimension, table_name).expect("下钻应成功");
        for field in &doc.fields {
            assert!(
                !field.doc.trim().is_empty(),
                "表 {table_name} 字段 {} 的 doc 非空（空 doc 即指名失败）",
                field.name
            );
            assert!(
                !field.ty.is_empty(),
                "表 {table_name} 字段 {} 的渲染类型非空",
                field.name
            );
        }
        let wire: Vec<String> = doc.fields.iter().map(|field| field.name.clone()).collect();
        let mut wire_sorted = wire.clone();
        wire_sorted.sort();
        let keys = json_keys(&value);
        // 方向一：线名 ⊆ 真实键集
        let missing_in_json: Vec<&String> = wire_sorted
            .iter()
            .filter(|name| !keys.contains(name))
            .collect();
        assert!(
            missing_in_json.is_empty(),
            "表 {table_name}：线名 {missing_in_json:?} 不在 serde 键集 {keys:?}"
        );
        // 方向二：真实键集 ⊆ 线名
        let missing_in_wire: Vec<&String> = keys
            .iter()
            .filter(|key| !wire_sorted.contains(key))
            .collect();
        assert!(
            missing_in_wire.is_empty(),
            "表 {table_name}：serde 键 {missing_in_wire:?} 不在线名集 {wire_sorted:?}"
        );
        assert_eq!(wire.len(), keys.len(), "表 {table_name}: 字段数 == 键数");
    }
}

#[test]
fn ratchet三specta字符串字段线格式为十六进制且蛇形名全线缺席() {
    for (table_name, field, prefix) in [
        ("checklist_item", "itemKey", "0x"),
        ("run_node", "nodeKey", "0x"),
        ("run_edge", "edgeKey", "0x"),
    ] {
        let dimension = crate::dimension_of(table_name).expect("表在册");
        let doc = table(dimension, table_name).expect("下钻应成功");
        let field_doc = doc
            .fields
            .iter()
            .find(|candidate| candidate.name == field)
            .unwrap_or_else(|| panic!("{table_name} 应有字段 {field}"));
        assert_eq!(field_doc.ty, "string", "{table_name}.{field} 渲染为 string");

        let value = sample_records()
            .into_iter()
            .find(|(name, _)| *name == table_name)
            .expect("样本在场")
            .1;
        let wire = value[field]
            .as_str()
            .unwrap_or_else(|| panic!("{table_name}.{field} 线格式为字符串: {value}"));
        assert!(wire.starts_with(prefix), "十六进制串形态: {wire}");
        assert!(
            wire[2..].chars().all(|char| char.is_ascii_hexdigit()),
            "`0x` 后全十六进制: {wire}"
        );
    }

    // 反向判据：任一表线名集中蛇形名缺席（防「未应用 `Format`」回归）
    for (table_name, value) in sample_records() {
        let dimension = crate::dimension_of(table_name).expect("表在册");
        let doc = table(dimension, table_name).expect("下钻应成功");
        let names = field_names(&doc);
        for key in json_keys(&value) {
            if key.contains('_') {
                panic!("表 {table_name} 的 serde 键 {key} 为蛇形（键集本身漂移）");
            }
            assert!(
                names.contains(&key.as_str()),
                "表 {table_name}: 键 {key} 应在 camelCase 线名集 {names:?}"
            );
        }
        assert!(
            !names.contains(&"change_id") && !names.contains(&"run_id"),
            "表 {table_name}: 蛇形线名缺席: {names:?}"
        );
    }
}

#[test]
fn 候选与维度名为自纠文案单点() {
    assert_eq!(dimension_name(DbDimension::User), "user");
    assert_eq!(dimension_name(DbDimension::Workspace), "workspace");
    assert_eq!(
        candidates(DbDimension::User),
        "workspace / agent_provider / agent_instance / notification",
        "本维度候选按登记序 ` / ` 分隔"
    );
    assert_eq!(
        all_candidates().matches(" / ").count(),
        entries().len() - 1,
        "全量候选 15 名（14 个分隔符）"
    );
    assert!(all_candidates().starts_with("workspace"));
    assert!(all_candidates().ends_with("run_edge"));
}

#[test]
fn 每次调用重建类型图且连调两次结果逐字相等() {
    let db = workspace_db();
    let first = tables(&db, DbDimension::Workspace).expect("第一次表清单");
    let second = tables(&db, DbDimension::Workspace).expect("第二次表清单");
    assert_eq!(first, second, "连调两次逐字相等（无跨调用污染）");
    assert_eq!(
        table(DbDimension::Workspace, "phase").expect("第一次下钻"),
        table(DbDimension::Workspace, "phase").expect("第二次下钻"),
        "同一表两次下钻输出相等"
    );
}

#[test]
fn schema_of与schema_session_event均给可解析根引用() {
    let mut types = Types::default();
    let root = schema_of::<PhaseRecord>(&mut types);
    assert!(
        matches!(root, DataType::Reference(_)),
        "derive 型 schema 根为命名引用（tables/table 据此取名与字段）"
    );
    let mut types = Types::default();
    let root = crate::describe::schema_session_event(&mut types);
    assert!(
        matches!(root, DataType::Reference(Reference::Named(_))),
        "手写 schema 根同为命名引用"
    );
}
