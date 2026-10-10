use agent::{
    AgentBlock, AgentEvent, AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunStatus,
};

use workflow::model::{ChecklistItem, Verdict};

use crate::model::{
    pack_checklist_item_key, pack_session_event_key, AgentEngineKind, AgentInstanceRecord,
    AgentModelTiers, AgentProviderRecord, AgentProviderRecordV1, AgentRunRecord, AgentRunRecordV3,
    ChecklistItemRecord, PhaseRecord, SessionConfigSnapshot, SessionEventRecord, SessionRecord,
    WorkspaceRecord,
};
use crate::store::Store;

// ---------------------------------------------------------------------------
// 装置：固定时间戳事件构造（等值断言与钟面无关）
// ---------------------------------------------------------------------------

fn event(seq: u64, kind: AgentEventKind) -> AgentEvent {
    AgentEvent {
        seq,
        timestamp_ms: 1727000000000 + seq as i64,
        kind,
    }
}

fn run_started(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::RunStarted {
            model: Some("claude-opus".to_owned()),
            session_id: Some("s-1".to_owned()),
            tools: vec!["Bash".to_owned()],
            mcp_servers: Vec::new(),
        },
    )
}

fn message(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Message {
            role: AgentMessageRole::Assistant,
            blocks: vec![AgentBlock::Text {
                text: "你好，世界".to_owned(),
            }],
            parent_tool_use_id: None,
        },
    )
}

fn system_notice(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::SystemNotice {
            subtype: "api_retry".to_owned(),
            payload: serde_json::json!({ "attempt": 2, "note": "重试中" }),
        },
    )
}

fn turn_done(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::TurnDone {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(3),
            duration_ms: Some(1234),
            cost_usd: Some(0.5),
            usage: serde_json::json!({ "input_tokens": 10 }),
            session_id: Some("s-1".to_owned()),
        },
    )
}

fn raw(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Raw {
            event_type: "mystery".to_owned(),
            raw_json: r#"{"type":"mystery"}"#.to_owned(),
        },
    )
}

/// native_model 封装内存往返（serde_json codec）：不经 db 文件。
fn roundtrip(record: &SessionEventRecord) -> SessionEventRecord {
    let bytes = native_model::encode(record).expect("native_model encode 应成功");
    let (decoded, version) =
        native_model::decode::<SessionEventRecord>(bytes).expect("native_model decode 应成功");
    assert_eq!(version, 1, "native_model 版本封装为 version 1");
    decoded
}

// ---------------------------------------------------------------------------
// 键打包：同会话保序、跨会话隔离、极值不溢出（合成 u128 主键）
// ---------------------------------------------------------------------------

#[test]
fn 键打包同session内seq增大则event_key严格增大() {
    let keys: Vec<u128> = (0..5u64)
        .map(|seq| SessionEventRecord::new("ses-1", raw(seq)).event_key)
        .collect();

    for pair in keys.windows(2) {
        assert!(
            pair[0] < pair[1],
            "同 session 内 seq 增大则 event_key 严格增大（大端序字典序 = seq 序）: {keys:?}"
        );
    }
}

#[test]
fn 键打包不同session同seq经hash高64位隔离不串键() {
    let session_a = SessionEventRecord::new("ses-甲", raw(3)).event_key;
    let session_b = SessionEventRecord::new("ses-乙", raw(3)).event_key;

    assert_ne!(
        session_a, session_b,
        "不同 session_id 同 seq：键区间经 hash64 高 64 位隔离不串键"
    );
    // 高 64 位不同（隔离区间），低 64 位同 seq
    assert_eq!(
        session_a & (u64::MAX as u128),
        session_b & (u64::MAX as u128),
        "低 64 位为同源 seq"
    );
    assert_ne!(
        session_a >> 64,
        session_b >> 64,
        "高 64 位 hash64(session_id) 互异"
    );
}

#[test]
fn 键打包seq极值不溢出不回绕() {
    // 同会话内比较（高 64 位恒一致，序完全由 seq 决定）
    let min = pack_session_event_key("ses-x", 0);
    let max = pack_session_event_key("ses-x", u64::MAX);

    assert_eq!(min & (u64::MAX as u128), 0, "最小键低 64 位 seq=0");
    assert_eq!(
        max & (u64::MAX as u128),
        u64::MAX as u128,
        "seq=u64::MAX 极值不溢出（低 64 位满幅）"
    );
    assert!(min < max, "同会话 seq 极值保序（打包键低 64 位即 seq）");
    // 打包口径：高 64 位 hash、低 64 位 seq（组装点唯一）
    let packed = pack_session_event_key("ses-x", 7);
    assert_eq!(packed & (u64::MAX as u128), 7u128);
    // 同会话高 64 位恒一致
    assert_eq!(
        pack_session_event_key("ses-x", 0) >> 64,
        pack_session_event_key("ses-x", u64::MAX) >> 64,
        "同会话打包键高 64 位恒一致"
    );
}

#[test]
fn 记录构造event_key打包自session与seq且访问器与载荷同源() {
    let record = SessionEventRecord::new("ses-9", message(4));

    assert_eq!(
        record.event_key,
        pack_session_event_key("ses-9", 4),
        "event_key 打包口径"
    );
    assert_eq!(record.session_id(), "ses-9", "session_id 访问器");
    assert_eq!(record.seq(), 4, "seq 访问器与载荷 event.seq 同源");
}

// ---------------------------------------------------------------------------
// 嵌装往返：密封事件逐字段保真（native_model serde_json codec，serde flatten）
// ---------------------------------------------------------------------------

#[test]
fn 嵌装往返密封五变体各构造一条逐字段相等() {
    let session_id = "ses-nest";
    let seeded = [
        run_started(0),
        message(1),
        system_notice(2),
        turn_done(3),
        raw(4),
    ];

    for original in seeded {
        let record = SessionEventRecord::new(session_id, original.clone());
        let decoded = roundtrip(&record);

        assert_eq!(decoded, record, "变体 seq={} 往返逐字段相等", original.seq);
        assert_eq!(
            decoded.event, original,
            "嵌装载荷与原密封事件逐字段相等（serde flatten 承载）"
        );
    }
}

#[test]
fn 嵌装往返raw逃生舱中文emoji引号原文保真() {
    let original = event(
        5,
        AgentEventKind::Raw {
            event_type: "外星事件".to_owned(),
            raw_json: "{\"kind\":\"外星事件\",\"note\":\"引号\\\"与emoji🚀\"}".to_owned(),
        },
    );
    let record = SessionEventRecord::new("ses-raw", original.clone());

    let decoded = roundtrip(&record);

    assert_eq!(decoded, record);
    assert!(
        matches!(
            &decoded.event.kind,
            AgentEventKind::Raw { event_type, raw_json }
                if event_type == "外星事件"
                    && raw_json == "{\"kind\":\"外星事件\",\"note\":\"引号\\\"与emoji🚀\"}"
        ),
        "Raw 逃生舱原文（中文/emoji/引号）嵌装往返保真"
    );
}

#[test]
fn 嵌装往返大seq打包键经编解码不回绕且十六进制串serde() {
    // u128 打包键超 u64 上界：经 serde 十六进制字符串往返后仍还原一致
    let record = SessionEventRecord::new("ses-huge", raw(u64::MAX));

    let decoded = roundtrip(&record);

    assert_eq!(decoded.event_key, record.event_key, "最大打包键往返一致");
    assert_eq!(decoded.seq(), u64::MAX);

    // serde 线格式：eventKey 十六进制字符串（serde_json 无 u128 数字面）
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");
    assert_eq!(
        value["eventKey"],
        serde_json::json!(format!("{:#034x}", record.event_key)),
        "eventKey 以十六进制字符串呈现（合法 JSON、无二进制）"
    );
    assert_eq!(value["sessionId"], serde_json::json!("ses-huge"));
    assert_eq!(value["event"]["seq"], serde_json::json!(u64::MAX));
}

// ---------------------------------------------------------------------------
// SessionRecord / SessionConfigSnapshot：serde 线格式往返
// ---------------------------------------------------------------------------

#[test]
fn session_record全字段serde往返_camel_case线格式逐字段() {
    let record = SessionRecord {
        id: "ses-1-1727000000000".to_owned(),
        engine_session_id: Some("sdk-7-1727000000001".to_owned()),
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Sdk,
            model: Some("m-high".to_owned()),
            permission_mode: AgentPermissionMode::BypassPermissions,
        },
        source: "explore".to_owned(),
        source_ref: Some("42".to_owned()),
        created_at: 1727000000000,
        updated_at: 1727000005000,
    };

    let value = serde_json::to_value(&record).expect("serde 序列化应成功");
    assert_eq!(value["id"], serde_json::json!("ses-1-1727000000000"));
    assert_eq!(
        value["engineSessionId"],
        serde_json::json!("sdk-7-1727000000001")
    );
    assert_eq!(
        value["configSnapshot"]["engine"],
        serde_json::json!("sdk"),
        "快照嵌套（engine/model/permission）"
    );
    assert_eq!(
        value["configSnapshot"]["model"],
        serde_json::json!("m-high")
    );
    assert_eq!(
        value["configSnapshot"]["permissionMode"],
        serde_json::json!("bypassPermissions")
    );
    assert_eq!(value["source"], serde_json::json!("explore"));
    assert_eq!(value["sourceRef"], serde_json::json!("42"));
    assert_eq!(value["createdAt"], serde_json::json!(1727000000000_i64));
    assert_eq!(value["updatedAt"], serde_json::json!(1727000005000_i64));

    let back: SessionRecord = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(back, record, "全字段往返逐字段相等");
}

#[test]
fn session_config_snapshot三字段往返且model_none与engine_session_id_none形态() {
    for (engine, model) in [
        (AgentEngineKind::Cli, None),
        (AgentEngineKind::Sdk, Some("m-high".to_owned())),
    ] {
        let snapshot = SessionConfigSnapshot {
            engine,
            model: model.clone(),
            permission_mode: AgentPermissionMode::AcceptEdits,
        };
        let value = serde_json::to_value(&snapshot).expect("序列化应成功");
        assert_eq!(
            value["engine"],
            serde_json::json!(if matches!(engine, AgentEngineKind::Cli) {
                "cli"
            } else {
                "sdk"
            })
        );
        let back: SessionConfigSnapshot = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(back, snapshot);
    }

    // session 行的 remote None 形态可区分
    let mut bare = SessionRecord {
        id: "ses-bare".to_owned(),
        engine_session_id: None,
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Cli,
            model: None,
            permission_mode: AgentPermissionMode::Default,
        },
        source: "debug".to_owned(),
        source_ref: None,
        created_at: 1,
        updated_at: 1,
    };
    let none_value = serde_json::to_value(&bare).expect("序列化应成功");
    assert_eq!(none_value["engineSessionId"], serde_json::Value::Null);
    let none_back: SessionRecord = serde_json::from_value(none_value).expect("反序列化成功");
    assert_eq!(none_back, bare);
    bare.engine_session_id = Some("sdk-1".to_owned());
    assert_ne!(
        serde_json::to_value(&bare).expect("序列化应成功")["engineSessionId"],
        serde_json::Value::Null
    );
}

// ---------------------------------------------------------------------------
// AgentRunRecord：native_model 版本 3→4 原地演进（v4 轮统计行化）
// ---------------------------------------------------------------------------

/// v4 轮统计行底座。
fn run_record_v4() -> AgentRunRecord {
    AgentRunRecord {
        id: 7,
        session_id: Some("ses-1-1727000000000".to_owned()),
        status: AgentRunStatus::Completed,
        started_at: 1727000000000,
        finished_at: Some(1727000004000),
        num_turns: Some(2),
        cost_usd: None,
        duration_ms: Some(4000),
        error: None,
    }
}

#[test]
fn agent_run_record_v4编解码版本断言4且往返逐字段相等() {
    let record = run_record_v4();

    let bytes = native_model::encode(&record).expect("native_model encode 应成功");
    let (decoded, version) =
        native_model::decode::<AgentRunRecord>(bytes).expect("native_model decode 应成功");
    assert_eq!(
        version, 4,
        "native_model 版本封装为 version 4（轮统计行化）"
    );
    assert_eq!(decoded, record, "v4 往返逐字段相等");
}

#[test]
fn v4线格式含session_id_option与统计字段且退役字段不在线格式() {
    let record = run_record_v4();
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");

    // 在场：sessionId Option / status / 起止时间戳 / 统计三字段 / error
    assert_eq!(
        value["sessionId"],
        serde_json::json!("ses-1-1727000000000"),
        "sessionId Option 承接"
    );
    for key in [
        "status",
        "startedAt",
        "finishedAt",
        "numTurns",
        "costUsd",
        "durationMs",
        "error",
    ] {
        assert!(value.get(key).is_some(), "v4 线格式含 {key}");
    }

    // 退役字段不在线格式（v3→v4 平移：来源归属主平移至 SessionRecord、链指针
    // 语义由会话归属取代）
    for retired in [
        "prompt",
        "cwd",
        "env",
        "permissionMode",
        "source",
        "sourceRef",
        "parentRunId",
    ] {
        assert!(
            value.get(retired).is_none(),
            "退役字段 {retired} 不在 v4 线格式"
        );
    }

    // session_id None 形态（孤儿轮行）往返
    let mut orphan = run_record_v4();
    orphan.session_id = None;
    let value = serde_json::to_value(&orphan).expect("序列化应成功");
    assert_eq!(value["sessionId"], serde_json::Value::Null);
    let back: AgentRunRecord = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(back, orphan);
}

#[test]
fn 存量v3行经版本机制自动升级为孤儿轮行_session_id置none统计保留() {
    // v3 历史形态（全平文字段，仅升级链解码目标）
    let legacy = AgentRunRecordV3 {
        id: 42,
        prompt: "存量首轮".to_owned(),
        cwd: "C:\\legacy".to_owned(),
        env: crate::model::AgentEnvModeLegacy::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
        status: AgentRunStatus::Completed,
        started_at: 1727000000000,
        finished_at: Some(1727000004000),
        num_turns: Some(2),
        cost_usd: Some(0.1),
        duration_ms: Some(4000),
        session_id: Some("s-legacy-cli".to_owned()),
        error: None,
        source: "debug".to_owned(),
        source_ref: None,
        parent_run_id: Some(41),
    };
    let legacy_bytes = native_model::encode(&legacy).expect("encode v3 应成功");
    // v3 载荷以 v3 模型 id 解出（版本机制识别 id=2 + version=3）
    let (legacy_decoded, legacy_version) =
        native_model::decode::<AgentRunRecordV3>(legacy_bytes.clone())
            .expect("v3 载荷可按 v3 模型解码");
    assert_eq!(legacy_version, 3, "存量形态封装为 version 3");
    assert_eq!(legacy_decoded, legacy, "v3 载荷按 v3 解码逐字段相等");

    // 同一载荷经版本机制自动升级为 v4：session_id 置 None（孤儿轮行）、统计与
    // 时间戳保留、退役字段不入（零迁移代码路径）
    let (upgraded, version) =
        native_model::decode::<AgentRunRecord>(legacy_bytes).expect("v3 载荷应经版本机制升级为 v4");
    assert_eq!(
        version, 3,
        "decode 返回载荷头版本（升级链源版本）；升级由值面承载（下方逐字段）"
    );
    assert_eq!(upgraded.id, 42);
    assert_eq!(
        upgraded.session_id, None,
        "存量行升级为无会话归属孤儿轮行（旧引擎侧会话 id 不平移）"
    );
    assert_eq!(upgraded.status, AgentRunStatus::Completed, "统计保留");
    assert_eq!(upgraded.started_at, 1727000000000, "时间戳保留");
    assert_eq!(upgraded.finished_at, Some(1727000004000));
    assert_eq!(upgraded.num_turns, Some(2));
    assert_eq!(upgraded.cost_usd, Some(0.1));
    assert_eq!(upgraded.duration_ms, Some(4000));
    assert_eq!(upgraded.error, None);
}

// ---------------------------------------------------------------------------
// agent 管理记录回归：构造语义、遮蔽 Debug、嵌装往返、serde 线格式（内存构造
// + native_model 封装内存往返，无 mock）
// ---------------------------------------------------------------------------

/// 三档模型 fixture（high / medium / low 三档可区分，消费半边断言取 high 档）。
fn tiers(high: &str, medium: &str, low: &str) -> AgentModelTiers {
    AgentModelTiers {
        high: high.to_owned(),
        medium: medium.to_owned(),
        low: low.to_owned(),
    }
}

#[test]
fn provider构造new五参载荷与三档models与context_length逐字段保真且id置0() {
    // 显式窗长：五参构造逐字段保真
    let windowed = AgentProviderRecord::new(
        "自建端点".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        Some(200_000),
    );
    assert_eq!(windowed.id, 0, "new 构造 id 恒置 0");
    assert_eq!(windowed.name, "自建端点");
    assert_eq!(windowed.base_url, "https://api.example.com/v1");
    assert_eq!(windowed.api_key, "sk-live-1234567890");
    assert_eq!(
        windowed.models,
        tiers("m-high", "m-medium", "m-low"),
        "三档 models 逐字段保真（PartialEq）"
    );
    assert_eq!(
        windowed.context_length,
        Some(200_000),
        "context_length 逐字段保真（None = 未配置语义入存储层）"
    );

    // 未配置：None 语义入列（不落 0 / 128000 字面）
    let record = AgentProviderRecord::new(
        "自建端点".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        None,
    );
    assert_eq!(record.id, 0, "new 构造 id 恒置 0");
    assert_eq!(record.context_length, None, "未配置 None 原样承载");

    // Clone / PartialEq 保真
    let cloned = record.clone();
    assert_eq!(cloned, record, "Clone 后逐字段相等");
    assert_ne!(cloned, windowed, "context_length 双态记录可区分");
}

#[test]
fn provider手写遮蔽debug输出api_key位为末三字符遮蔽形态_全文不含明文key() {
    let record = AgentProviderRecord::new(
        "遮蔽回归".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        None,
    );

    let debug_text = format!("{record:?}");

    // 手写遮蔽 Debug：api_key 位呈现 sk-***abc 形态（前缀掩码 + 末 3 字符）
    assert!(
        debug_text.contains("sk-***890"),
        "Debug 输出 api_key 位为「sk-*** + 末 3 字符」遮蔽形态，实际: {debug_text}"
    );
    // 结构保证明文不进日志：完整 key 全文不出现
    assert!(
        !debug_text.contains("sk-live-1234567890"),
        "Debug 输出不得含 api_key 明文，实际: {debug_text}"
    );
    // 结构体名与字段名在位（字段面与 derive 形态对齐，仅 api_key 位遮蔽）
    assert!(debug_text.contains("AgentProviderRecord"));
    assert!(debug_text.contains("api_key"));
    assert!(debug_text.contains("base_url"));
}

#[test]
fn provider遮蔽debug在api_key空串或长度不足3字符时恒sk三星号全遮蔽兜底() {
    for api_key in ["", "a", "ab", "abc"] {
        let record = AgentProviderRecord::new(
            "过短兜底".to_owned(),
            "https://api.example.com/v1".to_owned(),
            api_key.to_owned(),
            tiers("h", "m", "l"),
            None,
        );

        let debug_text = format!("{record:?}");

        assert!(
            debug_text.contains("sk-***"),
            "api_key={api_key:?} 过短时 Debug 恒含 sk-*** 掩码，实际: {debug_text}"
        );
        // 全遮蔽兜底：api_key 位恒为纯掩码（无末 3 字符回显，防 sk-***abc 恰为
        // 原文泄露——恰 3 字符时回显后缀即泄露原文）
        assert!(
            debug_text.contains("api_key: \"sk-***\""),
            "api_key={api_key:?} 过短时 api_key 位恒为 sk-*** 全遮蔽，实际: {debug_text}"
        );
    }
}

#[test]
fn provider构造特殊字符字段clone与partial_eq保真且models三档全空串构造合法() {
    let record = AgentProviderRecord::new(
        "名 字 中文 🎉 \"引号\"\n换行".to_owned(),
        "https://例子.测试/v1 🚀".to_owned(),
        "带 空格 的 \"key\" 🎉\n".to_owned(),
        tiers("", "", ""),
        None,
    );

    // 模型层不做字段校验：三档全空串构造合法（校验单点在 store）
    assert_eq!(record.models.high, "");
    assert_eq!(record.models.medium, "");
    assert_eq!(record.models.low, "");

    let cloned = record.clone();
    assert_eq!(cloned, record, "特殊字符字段 Clone / PartialEq 保真");
}

/// provider 记录的 native_model 内存往返（默认 bincode codec，不经 db 文件）。
fn provider_roundtrip(record: &AgentProviderRecord) -> AgentProviderRecord {
    let bytes = native_model::encode(record).expect("native_model encode 应成功");
    let (decoded, version) =
        native_model::decode::<AgentProviderRecord>(bytes).expect("native_model decode 应成功");
    assert_eq!(
        version, 2,
        "native_model 版本封装为 version 2（context_length 演进落位）"
    );
    decoded
}

#[test]
fn provider嵌装往返v2含context_length双态逐字段相等() {
    for context_length in [Some(200_000u64), None] {
        let record = AgentProviderRecord::new(
            "往返回归".to_owned(),
            "https://api.example.com/v1".to_owned(),
            "sk-live-1234567890".to_owned(),
            tiers("m-high", "m-medium", "m-low"),
            context_length,
        );

        let decoded = provider_roundtrip(&record);

        assert_eq!(
            decoded, record,
            "provider 记录（含三档 models + context_length {context_length:?}）往返逐字段相等"
        );
        assert_eq!(
            decoded.context_length, context_length,
            "context_length 双态经编解码不漂移"
        );
    }
}

#[test]
fn provider记录编解码版本断言2() {
    // envelope 版本演进落位：v2 编码载荷按 v2 解出（版本头 = 2）
    let record = AgentProviderRecord::new(
        "版本头".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        Some(200_000),
    );
    let bytes = native_model::encode(&record).expect("native_model encode 应成功");
    let (_decoded, version) =
        native_model::decode::<AgentProviderRecord>(bytes).expect("native_model decode 应成功");
    assert_eq!(version, 2, "native_model 版本封装为 version 2");
}

#[test]
fn 存量v1行经版本机制升级读入且context_length置none() {
    // v1 历史形态（无 context_length 列，仅升级链解码目标）
    let legacy = AgentProviderRecordV1 {
        id: 9,
        name: "存量供应".to_owned(),
        base_url: "https://legacy.example.com/v1".to_owned(),
        api_key: "sk-legacy-1234567890".to_owned(),
        models: tiers("v1-high", "v1-medium", "v1-low"),
    };
    let legacy_bytes = native_model::encode(&legacy).expect("encode v1 应成功");
    // v1 载荷以 v1 模型解出（版本机制识别 id=5 + version=1）
    let (legacy_decoded, legacy_version) =
        native_model::decode::<AgentProviderRecordV1>(legacy_bytes.clone())
            .expect("v1 载荷可按 v1 模型解码");
    assert_eq!(legacy_version, 1, "存量形态封装为 version 1");
    assert_eq!(legacy_decoded, legacy, "v1 载荷按 v1 解码逐字段相等");

    // 同一载荷经版本机制自动升级为 v2：context_length = None（旧记录缺列读
    // 兼容，v3→v4 先例同型），五字段原值保留
    let (upgraded, version) =
        native_model::decode::<AgentProviderRecord>(legacy_bytes).expect("v1 载荷应升级为 v2");
    assert_eq!(
        version, 1,
        "decode 返回载荷头版本（升级链源版本）；升级由值面承载（下方逐字段）"
    );
    assert_eq!(upgraded.id, 9, "id 保留");
    assert_eq!(upgraded.name, "存量供应");
    assert_eq!(upgraded.base_url, "https://legacy.example.com/v1");
    assert_eq!(upgraded.api_key, "sk-legacy-1234567890");
    assert_eq!(upgraded.models, tiers("v1-high", "v1-medium", "v1-low"));
    assert_eq!(
        upgraded.context_length, None,
        "升级读入 context_length = None（未配置语义）"
    );
}

#[test]
fn v1_from双向upgrade补none与downgrade丢新字段无损() {
    // upgrade：V1 → 记录，补 context_length = None
    let legacy = AgentProviderRecordV1 {
        id: 3,
        name: "双向供应".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        api_key: "sk-live-1234567890".to_owned(),
        models: tiers("h", "m", "l"),
    };
    let upgraded = AgentProviderRecord::from(legacy.clone());
    assert_eq!(
        upgraded.context_length, None,
        "升级补 context_length = None（缺列读兼容）"
    );
    assert_eq!(upgraded.id, legacy.id);
    assert_eq!(upgraded.name, legacy.name);
    assert_eq!(upgraded.base_url, legacy.base_url);
    assert_eq!(upgraded.api_key, legacy.api_key);
    assert_eq!(upgraded.models, legacy.models);

    // downgrade：记录 → V1，丢新字段无损（五字段原形还原；id / name 对齐后比较）
    let mut record = AgentProviderRecord::new(
        "双向供应".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("h", "m", "l"),
        Some(200_000),
    );
    record.id = legacy.id;
    let downgraded = AgentProviderRecordV1::from(record);
    assert_eq!(downgraded, legacy, "降级还原五字段原形（新字段丢弃无损）");
}

#[test]
fn provider遮蔽debug补context_length位且api_key位仍遮蔽() {
    let record = AgentProviderRecord::new(
        "窗长遮蔽".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        Some(200_000),
    );

    let debug_text = format!("{record:?}");

    // 字段清单对齐：context_length 字段位在 Debug 输出（含值）
    assert!(
        debug_text.contains("context_length: Some(200000)"),
        "Debug 输出含 context_length 字段位，实际: {debug_text}"
    );
    // api_key 位仍为遮蔽形态（明文不进 Debug，新字段不破坏既有遮蔽）
    assert!(debug_text.contains("sk-***890"));
    assert!(!debug_text.contains("sk-live-1234567890"));

    // None 形态同在位
    let none_record = AgentProviderRecord::new(
        "缺列遮蔽".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        None,
    );
    assert!(
        format!("{none_record:?}").contains("context_length: None"),
        "None 形态 context_length 位在 Debug 输出"
    );
}

#[test]
fn provider_serde线格式含context_length键且null与缺席均解为none() {
    let record = AgentProviderRecord::new(
        "线窗长".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        Some(200_000),
    );
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");
    assert_eq!(
        value["contextLength"],
        serde_json::json!(200_000),
        "serde camelCase 线格式含 contextLength 键"
    );

    // null 形态：解为 None
    let none_record = AgentProviderRecord::new(
        "线缺列".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        None,
    );
    let none_value = serde_json::to_value(&none_record).expect("序列化应成功");
    assert_eq!(none_value["contextLength"], serde_json::Value::Null);
    let none_back: AgentProviderRecord =
        serde_json::from_value(none_value.clone()).expect("反序列化成功");
    assert_eq!(none_back.context_length, None, "null 解为 None");

    // 缺席形态：serde default 承接（缺列读兼容的 serde 半边）
    let mut absent = none_value.clone();
    absent
        .as_object_mut()
        .expect("object 形态")
        .remove("contextLength");
    let absent_back: AgentProviderRecord =
        serde_json::from_value(absent).expect("缺席键反序列化成功");
    assert_eq!(
        absent_back.context_length, None,
        "缺席键经 serde default 解为 None（旧线格式读兼容）"
    );
    assert_eq!(absent_back, none_record, "缺席与 null 两形态等价 None");
}

#[test]
fn instance记录往返engine两变体与provider_id两态option语义经编解码不漂移() {
    for engine in [AgentEngineKind::Cli, AgentEngineKind::Sdk] {
        for provider_id in [None, Some(42)] {
            let record = AgentInstanceRecord::new(format!("实例-{engine:?}"), engine, provider_id);

            let bytes = native_model::encode(&record).expect("native_model encode 应成功");
            let (decoded, version) = native_model::decode::<AgentInstanceRecord>(bytes)
                .expect("native_model decode 应成功");
            assert_eq!(version, 1, "native_model 版本封装为 version 1");
            assert_eq!(decoded, record, "instance 记录往返保真");
            assert_eq!(
                decoded.provider_id, provider_id,
                "Option 语义经编解码不漂移（None / Some 两态）"
            );
        }
    }
}

#[test]
fn instance构造new恒id0与is_defaultfalse且engine两变体与provider_id两态逐字段保真() {
    // 构造器不产默认标记：is_default 恒 false（默认标记唯一写口为 set_default）
    let cli_agent = AgentInstanceRecord::new("cli-甲".to_owned(), AgentEngineKind::Cli, None);
    assert_eq!(cli_agent.id, 0, "new 构造 id 恒置 0");
    assert!(!cli_agent.is_default, "new 构造恒非默认");
    assert_eq!(cli_agent.name, "cli-甲");
    assert_eq!(cli_agent.engine, AgentEngineKind::Cli);
    assert_eq!(cli_agent.provider_id, None, "cli 臂 provider 可空透传");

    let sdk_agent = AgentInstanceRecord::new("sdk-乙".to_owned(), AgentEngineKind::Sdk, Some(7));
    assert_eq!(sdk_agent.id, 0);
    assert!(!sdk_agent.is_default);
    assert_eq!(sdk_agent.engine, AgentEngineKind::Sdk);
    assert_eq!(sdk_agent.provider_id, Some(7), "provider_id Some 两态透传");

    // Clone / PartialEq 保真
    assert_eq!(sdk_agent.clone(), sdk_agent);
    assert_ne!(cli_agent, sdk_agent, "两形态记录不等");
}

#[test]
fn 管理记录serde线格式键名为小驼峰且engine出线cli与sdk串值() {
    let provider = AgentProviderRecord::new(
        "线格式".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
        None,
    );
    let provider_value = serde_json::to_value(&provider).expect("serde 序列化应成功");
    assert_eq!(provider_value["name"], serde_json::json!("线格式"));
    assert_eq!(
        provider_value["baseUrl"],
        serde_json::json!("https://api.example.com/v1")
    );
    assert_eq!(
        provider_value["apiKey"],
        serde_json::json!("sk-live-1234567890")
    );
    assert_eq!(
        provider_value["models"]["high"],
        serde_json::json!("m-high")
    );
    assert_eq!(
        provider_value["models"]["medium"],
        serde_json::json!("m-medium")
    );
    assert_eq!(provider_value["models"]["low"], serde_json::json!("m-low"));

    let instance = AgentInstanceRecord::new("线格式实例".to_owned(), AgentEngineKind::Sdk, Some(7));
    let instance_value = serde_json::to_value(&instance).expect("serde 序列化应成功");
    assert_eq!(instance_value["engine"], serde_json::json!("sdk"));
    assert_eq!(instance_value["providerId"], serde_json::json!(7));
    assert_eq!(instance_value["isDefault"], serde_json::json!(false));

    let cli = AgentInstanceRecord::new("线格式cli".to_owned(), AgentEngineKind::Cli, None);
    let cli_value = serde_json::to_value(&cli).expect("serde 序列化应成功");
    assert_eq!(
        cli_value["engine"],
        serde_json::json!("cli"),
        "engine 出线 \"cli\""
    );
    assert_eq!(cli_value["providerId"], serde_json::json!(null));

    // 反序列化 roundtrip 一致（线格式双向受控）
    let provider_back: AgentProviderRecord =
        serde_json::from_value(provider_value).expect("反序列化成功");
    assert_eq!(provider_back, provider);
    let instance_back: AgentInstanceRecord =
        serde_json::from_value(instance_value).expect("反序列化成功");
    assert_eq!(instance_back, instance);
}

#[test]
fn serde线格式非法engine串反序列化err() {
    // 受控值域拒绝：非 "cli"/"sdk" 串反序列化 Err（与既有三枚举线格式口径同型）
    let result = serde_json::from_value::<AgentEngineKind>(serde_json::json!("yolo"));
    assert!(result.is_err(), "非法 engine 串应 Err，实际: {result:?}");
}

// ---------------------------------------------------------------------------
// change 流程状态四模型（desktop-change-state-store）：打包键自然序 / 十六
// 进制 serde 出线 / 注册面零改动覆盖 / 缺省构造与 native_model 往返。纯内存
// 构造 + tempfile 真实 workspace db 注册（Env 装置先例，进程边界用真实临时
// 实例不 mock）；native_model 编解码自身语义不逐项验证（库语义），只测自研
// 打包键与注册面。
// ---------------------------------------------------------------------------

/// checklist 检查项 fixture（item / pass / evidence 三面可区分）。
fn checklist_item(item: &str, pass: bool) -> ChecklistItem {
    ChecklistItem {
        item: item.to_owned(),
        pass,
        evidence: format!("证据-{item}"),
    }
}

#[test]
fn checklist打包键同phase_id下item_index升序则item_key严格递增() {
    // 主键自然序 = item_index 升序 = evaluator 输出序（AC-1 打包键序半边）：
    // 高 64 位 phase_id 恒一致，序完全由低 64 位 item_index 决定
    let keys: Vec<u128> = (0..6u32)
        .map(|item_index| {
            ChecklistItemRecord::new(
                7,
                item_index,
                checklist_item(&format!("项-{item_index}"), true),
            )
            .item_key
        })
        .collect();

    for pair in keys.windows(2) {
        assert!(
            pair[0] < pair[1],
            "同 phase_id 下 item_index 升序则 item_key 严格递增（大端序字典序 = 数值序）: {keys:?}"
        );
    }
}

#[test]
fn checklist打包键组合不串位_还原往返与跨phase_id隔离() {
    // 打包口径：高 64 位 phase_id、低 64 位 item_index（组装点唯一）
    let packed = pack_checklist_item_key(7, 5);
    assert_eq!(packed & (u64::MAX as u128), 5u128, "低 64 位为 item_index");
    assert_eq!(packed >> 64, 7u128, "高 64 位为 phase_id（组合不串位）");

    // 打包 / 还原往返：(phase_id, item_index) 逐字段一致
    let record = ChecklistItemRecord::new(7, 5, checklist_item("项-5", false));
    assert_eq!(record.item_key, packed, "构造器与打包单点同源");
    assert_eq!(
        record.item_key & (u64::MAX as u128),
        5u128,
        "还原 item_index"
    );
    assert_eq!(record.item_key >> 64, 7u128, "还原 phase_id");

    // 跨 phase_id 隔离：同 item_index 高 64 位互异、低 64 位相等（互不串键）
    let phase_a = pack_checklist_item_key(7, 3);
    let phase_b = pack_checklist_item_key(8, 3);
    assert_ne!(phase_a >> 64, phase_b >> 64, "高 64 位 phase_id 隔离区间");
    assert_eq!(
        phase_a & (u64::MAX as u128),
        phase_b & (u64::MAX as u128),
        "低 64 位同源 item_index"
    );

    // 极值不溢出不回绕：phase_id 高位满幅 + item_index 低 64 位内最大仍保序
    let min = pack_checklist_item_key(0, 0);
    let max = pack_checklist_item_key(i64::MAX, u32::MAX);
    assert!(min < max, "极值组合保序");
    assert_eq!(max >> 64, i64::MAX as u128, "极值高 64 位不回绕");
    assert_eq!(
        max & (u64::MAX as u128),
        u32::MAX as u128,
        "极值低 64 位不回绕"
    );
}

#[test]
fn checklist_item_record_item_key十六进制串serde出线与反向解码往返无损() {
    let record = ChecklistItemRecord::new(7, 5, checklist_item("检查项", true));

    // serde 定制出线：itemKey 为 `{:#034x}` 十六进制字符串（serde_json 无
    // u128 数字面，信封 API 要把记录转 JSON——自研绕法与 event_key 同型）
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");
    assert_eq!(
        value["itemKey"],
        serde_json::json!(format!("{:#034x}", record.item_key)),
        "itemKey 以 34 位（含 0x 前缀）十六进制字符串呈现（合法 JSON、无二进制）"
    );
    assert_eq!(value["phaseId"], serde_json::json!(7), "phaseId 常规数字面");
    assert_eq!(value["item"], serde_json::json!("检查项"));

    // 反向解码往返无损
    let back: ChecklistItemRecord = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(back, record, "十六进制串出线往返逐字段一致");
}

#[test]
fn 十一模型注册workspace组全覆盖_新id与既有id无冲突_list_models覆盖持衡() {
    let db_dir = tempfile::Builder::new()
        .prefix("store-test-model-registry-")
        .tempdir()
        .expect("创建临时目录失败");

    // 真实 workspace db：id 9 / 10 / 11 / 12 / 13 / 14 / 15 与既有 id
    // （1/2/4/5/6/7/8）无一冲突即打开点注册成功（Models::define 对同
    // id+版本 panic，打开成功即注册面无撞号）；id=3 历史退役空缺不复用（D3）
    let store = Store::open_workspace(&db_dir.path().join("ws.redb"))
        .unwrap_or_else(|e| panic!("open_workspace 应成功（十一模型注册无 id 冲突）: {e}"));

    // 注册面 = 十一模型（workspace 组）：逐类型 id / 版本与本组既有模型、全局组
    // 模型（1 / 5 / 6）全表无重复
    let workspace_group = [
        (
            <crate::model::AgentRunRecord as native_model::Model>::native_model_id(),
            <crate::model::AgentRunRecord as native_model::Model>::native_model_version(),
        ),
        (
            <SessionRecord as native_model::Model>::native_model_id(),
            <SessionRecord as native_model::Model>::native_model_version(),
        ),
        (
            <SessionEventRecord as native_model::Model>::native_model_id(),
            <SessionEventRecord as native_model::Model>::native_model_version(),
        ),
        (
            <crate::model::ExploreRecord as native_model::Model>::native_model_id(),
            <crate::model::ExploreRecord as native_model::Model>::native_model_version(),
        ),
        (
            <ChangeRecord as native_model::Model>::native_model_id(),
            <ChangeRecord as native_model::Model>::native_model_version(),
        ),
        (
            <PhaseRecord as native_model::Model>::native_model_id(),
            <PhaseRecord as native_model::Model>::native_model_version(),
        ),
        (
            <ChecklistItemRecord as native_model::Model>::native_model_id(),
            <ChecklistItemRecord as native_model::Model>::native_model_version(),
        ),
        (
            <crate::model::StepRecord as native_model::Model>::native_model_id(),
            <crate::model::StepRecord as native_model::Model>::native_model_version(),
        ),
        (
            <crate::model::RunRecord as native_model::Model>::native_model_id(),
            <crate::model::RunRecord as native_model::Model>::native_model_version(),
        ),
        (
            <crate::model::RunStepRecord as native_model::Model>::native_model_id(),
            <crate::model::RunStepRecord as native_model::Model>::native_model_version(),
        ),
        (
            <crate::model::StoreMetaRecord as native_model::Model>::native_model_id(),
            <crate::model::StoreMetaRecord as native_model::Model>::native_model_version(),
        ),
    ];
    assert_eq!(
        workspace_group.map(|(id, _)| id),
        [2, 7, 8, 4, 9, 10, 11, 12, 13, 14, 15],
        "workspace 组注册 10 → 11：run 史两模型 + 库级格式版本标记一模型入册"
    );
    let mut ids: Vec<u32> = workspace_group
        .iter()
        .map(|(id, _)| *id)
        .chain([
            <WorkspaceRecord as native_model::Model>::native_model_id(),
            <AgentProviderRecord as native_model::Model>::native_model_id(),
            <AgentInstanceRecord as native_model::Model>::native_model_id(),
        ])
        .collect();
    ids.sort_unstable();
    let mut deduped = ids.clone();
    deduped.dedup();
    assert_eq!(
        ids, deduped,
        "全册 id 无冲突（既有 1/2/4/5/6/7/8 与新 9..15）"
    );

    // list_models 零改动覆盖持衡：信封注册面不写一行即可浏览既有八模型
    // （空库计数 0 也列出；run 史 / 标记模型无查看面需求，不入注册表）
    let models = store.list_models().unwrap();
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
        "workspace 组信封注册面恰八行（零改动覆盖）"
    );
    assert!(
        models.iter().all(|model| model.count == 0),
        "空库全部计数 0（注册面覆盖先于任何写入）"
    );
}

#[test]
fn phase_record_v2缺省构造三槽位与start_at与backtrack全none可落且往返保真() {
    // 缺省构造合法：三会话槽位 / start_at / backtrack 字段全 None（native_model
    // 平直字段无 flatten、默认 bincode——D3 附加约束的编译锚定）；归属列
    // change_id（10:v2 换锚）往返保真
    let record = PhaseRecord {
        id: 11,
        change_id: "chg-demo".to_owned(),
        phase: "proposal".to_owned(),
        attempt: 1,
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
    };

    let bytes = native_model::encode(&record).expect("native_model encode 应成功");
    let (decoded, version) =
        native_model::decode::<PhaseRecord>(bytes).expect("native_model decode 应成功");

    assert_eq!(
        version, 2,
        "native_model 10:v2 版本封装（归属列换锚 change_id）"
    );
    assert_eq!(
        decoded, record,
        "缺省构造记录往返逐字段相等（None 槽位不漂移）"
    );
    assert_eq!(decoded.change_id, "chg-demo", "归属列 change_id 往返保真");
}

/// StepRecord v2 往返（12:v2 归属列换锚）：id 主键 + change_id / run_id 归属列
/// 与七值步种逐字段保真。
#[test]
fn step_record_v2_change_id归属列往返逐字段保真() {
    let command = workflow::state::StepCommand {
        run_id: "run-1727000000000".to_owned(),
        change_id: "chg-step".to_owned(),
        step_kind: workflow::state::StepKind::PhaseNext,
        status: "ok".to_owned(),
        summary: "轮次推进".to_owned(),
        reference: Some("checks/reports".to_owned()),
        timestamp: 1_727_000_000_000,
    };
    let record = crate::model::StepRecord::new(&command);

    let bytes = native_model::encode(&record).expect("native_model encode 应成功");
    let (decoded, version) = native_model::decode::<crate::model::StepRecord>(bytes)
        .expect("native_model decode 应成功");

    assert_eq!(
        version, 2,
        "native_model 12:v2 版本封装（归属列 change_id）"
    );
    assert_eq!(decoded, record, "StepRecord 往返逐字段相等");
    assert_eq!(decoded.change_id, "chg-step", "归属列 change_id 往返保真");
    assert_eq!(decoded.run_id, "run-1727000000000");
    assert_eq!(decoded.reference.as_deref(), Some("checks/reports"));
}

#[test]
fn checklist_item_record_native_model往返版本1逐字段保真() {
    // 独立版本链：checklist 子行与 PhaseRecord 解耦（evidence 长文本演进面）；
    // 本变更零触点——11:v1 持衡
    let record = ChecklistItemRecord::new(7, 2, checklist_item("往返检查项", false));

    let bytes = native_model::encode(&record).expect("native_model encode 应成功");
    let (decoded, version) =
        native_model::decode::<ChecklistItemRecord>(bytes).expect("native_model decode 应成功");

    assert_eq!(version, 1, "native_model 11:v1 版本封装（零改动）");
    assert_eq!(decoded, record, "打包键与三面载荷往返逐字段相等");
    assert_eq!(decoded.phase_id, 7, "二级索引列往返保真");
    assert_eq!(decoded.pass, false);
}

// ---------------------------------------------------------------------------
// ChangeRecord v3（身份锚换 id / test-design AC-1 · AC-8 模型面；9:v2 → 9:v3）：
// id 主键 + name 普通属性回环 / 身份面（同 name 异 id 合法不相等）/ new 携 id
// 构造 / 表名版本段公式锚。v1 解码链（`ChangeRecordV1` 与双向 `From`）随身份
// 换锚整体退役——旧形态数据零 decode 路径（D5：不可达或整体丢弃）。
// ---------------------------------------------------------------------------

use crate::model::ChangeRecord;
use workflow::state::ChangeStatus;

/// v3 回环：id 主键（身份锚）+ name 普通属性 + worktree / base_commit 双态
/// 构造 → encode / decode 往返逐字段相等（版本头 = 3）。
#[test]
fn change_record_v3回环含worktree双态逐字段相等() {
    for (worktree, base_commit) in [
        (
            Some(r"C:\app-data\worktrees\seg\fix-bug".to_owned()),
            Some("0000000000000000000000000000000000000001".to_owned()),
        ),
        (None, None),
    ] {
        let record = ChangeRecord::new(
            "chg-018f3a-0001",
            "回回归",
            "requirement",
            1_727_000_000_000,
            worktree.clone(),
            base_commit.clone(),
        );
        let bytes = native_model::encode(&record).expect("encode v3 应成功");
        let (decoded, version) =
            native_model::decode::<ChangeRecord>(bytes).expect("decode v3 应成功");
        assert_eq!(
            version, 3,
            "v3 编码载荷按 v3 解出（版本头 = 3；换锚 9:v2 → 9:v3 主键 name → id）"
        );
        assert_eq!(
            decoded, record,
            "v3 往返逐字段相等（worktree {worktree:?} / base_commit {base_commit:?}）"
        );
        assert_eq!(decoded.id, "chg-018f3a-0001", "id 主键往返保真（身份锚）");
        assert_eq!(
            decoded.name, "回回归",
            "name 普通属性往返保真（非主键，无唯一约束）"
        );
    }
}

/// 身份面（D11 模型面锚）：id 与 name 双字段独立可辨——同 name 不同 id 两条
/// 记录构造合法且不相等（name 非身份键）。
#[test]
fn change_record身份面同name不同id两记录合法且不相等() {
    let first = ChangeRecord::new("chg-甲", "同名档", "requirement", 1000, None, None);
    let second = ChangeRecord::new("chg-乙", "同名档", "requirement", 1000, None, None);

    assert_eq!(first.name, second.name, "同 name 合法（name 无唯一约束）");
    assert_ne!(first.id, second.id, "id 互异（身份锚独立可辨）");
    assert_ne!(
        first, second,
        "同 name 不同 id 两条记录不相等（name 非身份键）"
    );
    assert_eq!(first.clone(), first, "PartialEq / Clone 自反");
}

/// new 携 id 构造：`new(id, name, workflow_type, created_at, worktree,
/// base_commit)` 逐字段对位；status 恒 active 起步 / archived_at / active_phase
/// 空起步持衡。
#[test]
fn change_record_new携id构造逐字段对位_active与空态起步() {
    let worktree = r"C:\app-data\worktrees\seg\fix-bug".to_owned();
    let base = "0000000000000000000000000000000000000001".to_owned();
    let record = ChangeRecord::new(
        "chg-0001",
        "显式传入",
        "requirement",
        1_727_000_000_000,
        Some(worktree.clone()),
        Some(base.clone()),
    );
    assert_eq!(
        record.id, "chg-0001",
        "id 入参逐字段对位（身份锚随构造入列）"
    );
    assert_eq!(record.name, "显式传入", "name 入参逐字段对位（普通属性）");
    assert_eq!(record.workflow_type, "requirement");
    assert_eq!(record.created_at, 1_727_000_000_000);
    assert_eq!(record.status, ChangeStatus::Active, "status 恒 active 起步");
    assert_eq!(record.archived_at, None, "archived_at 空起步持衡");
    assert_eq!(record.active_phase, None, "active_phase 空起步持衡");
    assert_eq!(record.worktree.as_deref(), Some(worktree.as_str()));
    assert_eq!(record.base_commit.as_deref(), Some(base.as_str()));

    let legacy_shape = ChangeRecord::new(
        "chg-legacy",
        "legacy形态",
        "requirement",
        1_727_000_000_000,
        None,
        None,
    );
    assert_eq!(legacy_shape.id, "chg-legacy");
    assert_eq!(legacy_shape.name, "legacy形态");
    assert_eq!(
        legacy_shape.worktree, None,
        "None 传入 = legacy 主 root 形态"
    );
    assert_eq!(legacy_shape.base_commit, None);
}

/// 表名版本段公式锚（`{native_model_id}_{version}_{主键字段名小写}`——与
/// store_test 物表名常量同源）：ChangeRecord 9:v3 / PhaseRecord 10:v2 /
/// StepRecord 12:v2 / RunRecord 13:v2 / StoreMetaRecord 15:v1。
#[test]
fn 表名版本段_native_model_id与version断言_公式锚() {
    let table = |id: u32, version: u32, key_field: &str| format!("{id}_{version}_{key_field}");

    assert_eq!(
        (
            <ChangeRecord as native_model::Model>::native_model_id(),
            <ChangeRecord as native_model::Model>::native_model_version()
        ),
        (9, 3),
        "ChangeRecord 9:v3（身份锚换 id —— 物表 9_2_name → 9_3_id）"
    );
    assert_eq!(table(9, 3, "id"), "9_3_id", "change 建档物表名公式锚");

    assert_eq!(
        (
            <crate::model::PhaseRecord as native_model::Model>::native_model_id(),
            <crate::model::PhaseRecord as native_model::Model>::native_model_version()
        ),
        (10, 2),
        "PhaseRecord 10:v2（归属列换锚 change → change_id）"
    );
    assert_eq!(table(10, 2, "id"), "10_2_id");
    assert_eq!(
        table(10, 2, "change_id"),
        "10_2_change_id",
        "归属列二级索引表名"
    );

    assert_eq!(
        (
            <crate::model::StepRecord as native_model::Model>::native_model_id(),
            <crate::model::StepRecord as native_model::Model>::native_model_version()
        ),
        (12, 2),
        "StepRecord 12:v2"
    );
    assert_eq!(table(12, 2, "id"), "12_2_id");
    assert_eq!(table(12, 2, "change_id"), "12_2_change_id");

    assert_eq!(
        (
            <crate::model::RunRecord as native_model::Model>::native_model_id(),
            <crate::model::RunRecord as native_model::Model>::native_model_version()
        ),
        (13, 2),
        "RunRecord 13:v2（主键 run_id）"
    );
    assert_eq!(table(13, 2, "run_id"), "13_2_run_id");
    assert_eq!(table(13, 2, "change_id"), "13_2_change_id");

    assert_eq!(
        (
            <crate::model::StoreMetaRecord as native_model::Model>::native_model_id(),
            <crate::model::StoreMetaRecord as native_model::Model>::native_model_version()
        ),
        (15, 1),
        "StoreMetaRecord 15:v1（库级格式版本标记，主键字段 key）"
    );
    assert_eq!(
        table(15, 1, "key"),
        "15_1_key",
        "标记物表名公式锚（表内固定单键行 `format`——单键值非表名成分）"
    );
}

/// StoreMetaRecord 单键固定形态往返：key="format" +
/// format_version=WORKSPACE_STORE_FORMAT_VERSION 编解码往返相等；常量口径
/// （探测判定源——D3）断言。
#[test]
fn store_meta_record往返_format单键与格式版本常量() {
    use crate::model::{
        StoreMetaRecord, WORKSPACE_STORE_FORMAT_KEY, WORKSPACE_STORE_FORMAT_VERSION,
    };

    assert_eq!(WORKSPACE_STORE_FORMAT_KEY, "format", "标记固定单键");
    assert_eq!(
        WORKSPACE_STORE_FORMAT_VERSION, 2,
        "当前格式版本 = 2（change 身份锚 id 形态；1 = name 主键形态时代）"
    );

    let record = StoreMetaRecord {
        key: WORKSPACE_STORE_FORMAT_KEY.to_owned(),
        format_version: WORKSPACE_STORE_FORMAT_VERSION,
    };
    let bytes = native_model::encode(&record).expect("encode 标记应成功");
    let (decoded, version) =
        native_model::decode::<StoreMetaRecord>(bytes).expect("decode 标记应成功");
    assert_eq!(version, 1, "StoreMetaRecord 15:v1 版本封装");
    assert_eq!(decoded, record, "单键固定形态往返逐字段相等");
    assert_eq!(decoded.key, "format");
    assert_eq!(decoded.format_version, 2);

    // serde camelCase 线格式（内部治理记录——不入信封注册表，无查看面）
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");
    assert_eq!(value["key"], serde_json::json!("format"));
    assert_eq!(value["formatVersion"], serde_json::json!(2));
}

// ---------------------------------------------------------------------------
// run 运行史两模型（unify-run-state-persistence）：native_model 回环 + 打包键
// ---------------------------------------------------------------------------

use crate::model::{pack_run_step_key, RunRecord, RunStepRecord};
use workflow::state::{RunStartCommand, RunStatus, RunStepEntry, RunStepKind, RunStepStatus};

/// run 发起命令 fixture（RunRecord::new 组装面；change_id = 身份锚 id）。
fn run_start_command() -> RunStartCommand {
    RunStartCommand {
        run_id: "run-1727000000000".to_owned(),
        change_id: "chg-demo".to_owned(),
        started_at: 1_727_000_000_000,
    }
}

/// run 收口整包条目 fixture。
fn run_step_entry(seq: u64, step: RunStepKind, status: RunStepStatus) -> RunStepEntry {
    RunStepEntry {
        seq,
        phase: "implement".to_owned(),
        attempt: 1,
        step,
        status,
        session_id: None,
        detail: None,
    }
}

/// RunRecord 写读回环逐字段一致：status 五值各一轮（native_model encode/decode
/// 往返）、reason / finished_at None 与 Some 两态（AC-1）；归属列 change_id
/// （13:v2 换锚）往返保真。
#[test]
fn run_record写读回环_status五值与两态字段() {
    // running 起步形态（RunRecord::new 命令组装：无 reason、无收口时刻）
    let running = RunRecord::new(&run_start_command());
    assert_eq!(running.status, RunStatus::Running);
    assert_eq!(running.reason, None, "running 恒无记因");
    assert_eq!(running.finished_at, None, "running 恒无收口时刻");
    assert_eq!(
        running.started_at, 1_727_000_000_000,
        "started_at 命令携带原值"
    );
    assert_eq!(running.change_id, "chg-demo", "归属列 = 命令载荷 change_id");

    for status in [
        RunStatus::Running,
        RunStatus::Completed,
        RunStatus::Stopped,
        RunStatus::Failed,
        RunStatus::Interrupted,
    ] {
        let record = RunRecord {
            run_id: format!("run-{status:?}"),
            change_id: "chg-demo".to_owned(),
            status,
            reason: Some("终态记因".to_owned()),
            started_at: 1_727_000_000_000,
            finished_at: Some(1_727_000_060_000),
        };
        let bytes = native_model::encode(&record).expect("native_model encode 应成功");
        let (decoded, version) =
            native_model::decode::<RunRecord>(bytes).expect("native_model decode 应成功");
        assert_eq!(
            version, 2,
            "native_model id=13 版本封装 version 2（归属列换锚）"
        );
        assert_eq!(decoded, record, "{status:?} 回环逐字段相等（AC-1）");
        assert_eq!(decoded.change_id, "chg-demo", "归属列 change_id 往返保真");
    }

    // reason / finished_at None 形态（running 行读回）
    let bare = RunRecord::new(&run_start_command());
    let bytes = native_model::encode(&bare).expect("encode 应成功");
    let (decoded, _) = native_model::decode::<RunRecord>(bytes).expect("decode 应成功");
    assert_eq!(decoded, bare, "None 两态回环不漂移");
}

/// RunStepRecord 写读回环逐字段一致：step 五词汇、status 四值、session_id /
/// detail 两态；seq() 读面 = 打包键低 64 位（AC-1）。
#[test]
fn run_step_record写读回环_词汇与状态与两态() {
    for (kind, status) in [
        (RunStepKind::Executor, RunStepStatus::Passed),
        (RunStepKind::Evaluator, RunStepStatus::Failed),
        (RunStepKind::Decision, RunStepStatus::Passed),
        (RunStepKind::StaticCheck, RunStepStatus::Running),
        (RunStepKind::TestExecution, RunStepStatus::Stopped),
    ] {
        let record = RunStepRecord::new(
            "run-1727000000000",
            &run_step_entry(3, kind, status),
            1_727_000_060_000,
        );
        assert_eq!(record.timestamp, 1_727_000_060_000, "整包同刻随构造注入");
        assert_eq!(record.seq(), 3, "seq() = 打包键低 64 位");

        let bytes = native_model::encode(&record).expect("encode 应成功");
        let (decoded, version) =
            native_model::decode::<RunStepRecord>(bytes).expect("decode 应成功");
        assert_eq!(
            version, 1,
            "native_model id=14 版本封装 version 1（本变更零触点）"
        );
        assert_eq!(decoded, record, "{kind:?}/{status:?} 回环逐字段相等");
        assert_eq!(decoded.step, kind);
        assert_eq!(decoded.status, status);
    }

    // session_id / detail 两态（None = 工具步形态）
    let bare = RunStepRecord::new(
        "run-1",
        &run_step_entry(0, RunStepKind::StaticCheck, RunStepStatus::Passed),
        1_727_000_060_000,
    );
    assert_eq!(bare.session_id, None);
    assert_eq!(bare.detail, None);
    let bytes = native_model::encode(&bare).expect("encode 应成功");
    let (decoded, _) = native_model::decode::<RunStepRecord>(bytes).expect("decode 应成功");
    assert_eq!(decoded, bare, "None 两态回环不漂移");
}

/// 打包键数值序即分层序（AC-9 全史叠加库面前置）：同 run_id 下 seq 递增键严
/// 格递增；异 run_id 键域不相交（哈希高 64 位分域）。
#[test]
fn pack_run_step_key数值序即分层序_异run键域不相交() {
    // 同 run_id：seq 递增键严格递增（含空洞 seq 0,2,5）
    let mut keys: Vec<u128> = [0u64, 2, 5]
        .iter()
        .map(|seq| pack_run_step_key("run-a", *seq))
        .collect();
    keys.sort();
    assert!(
        keys.windows(2).all(|w| w[0] < w[1]),
        "seq 空洞序合法且严格递增"
    );

    // 异 run_id：全键域不相交（高 64 位 = hash64(run_id) 分域）
    let run_a: Vec<u128> = (0..8u64)
        .map(|seq| pack_run_step_key("run-a", seq))
        .collect();
    let run_b: Vec<u128> = (0..8u64)
        .map(|seq| pack_run_step_key("run-b", seq))
        .collect();
    for key_a in &run_a {
        assert!(
            !run_b.contains(key_a),
            "异 run 键域不相交（{key_a} 不应撞 b 域）"
        );
    }
    // 同 run 内高 64 位一致（run 二级索引语义的键面）
    let high_a = run_a[0] >> 64;
    assert!(
        run_a.iter().all(|key| key >> 64 == high_a),
        "同 run 键高 64 位恒一致"
    );
}

/// step_key u128 经 event_key_serde 十六进制线面编码回环无损（serde_json 无
/// u128 数字面——hex 串线面，SessionEventRecord 先例同式，AC-1）。
#[test]
fn run_step_record_step_key_hex线面编码回环无损() {
    let record = RunStepRecord::new(
        "run-1727000000000",
        &run_step_entry(u64::MAX, RunStepKind::Executor, RunStepStatus::Passed),
        1_727_000_060_000,
    );

    let value = serde_json::to_value(&record).expect("serde_json 出线应成功");
    let hex = value["stepKey"].as_str().expect("stepKey 出 hex 串");
    assert!(hex.starts_with("0x"), "十六进制串线面（0x 前缀）: {hex}");
    assert_eq!(hex.len(), 34, "u128 满 32 位 hex + 0x 前缀");

    let back: RunStepRecord = serde_json::from_value(value).expect("反序列化应成功");
    assert_eq!(back, record, "hex 线面回环无损（seq 低 64 位全 1 边界）");
    assert_eq!(back.seq(), u64::MAX, "seq() 读面 = 低 64 位（边界不截断）");
}
