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
            agent_instance_id: None,
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
            agent_instance_id: None,
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
            agent_instance_id: None,
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
            <crate::model::RunNodeRecord as native_model::Model>::native_model_id(),
            <crate::model::RunNodeRecord as native_model::Model>::native_model_version(),
        ),
        (
            <crate::model::RunEdgeRecord as native_model::Model>::native_model_id(),
            <crate::model::RunEdgeRecord as native_model::Model>::native_model_version(),
        ),
        (
            <crate::model::StoreMetaRecord as native_model::Model>::native_model_id(),
            <crate::model::StoreMetaRecord as native_model::Model>::native_model_version(),
        ),
    ];
    assert_eq!(
        workspace_group.map(|(id, _)| id),
        [2, 7, 8, 4, 9, 10, 11, 12, 13, 16, 17, 15],
        "workspace 组注册：run 执行图节点 / 边两模型入册（id 14 RunStepRecord 退役，\n         id 16/17 增补）"
    );
    assert!(
        !workspace_group.iter().any(|(id, _)| *id == 14),
        "RunStepRecord 注册摘除（id 14 退役）"
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

use crate::model::{
    ChangeRecord, ChangeRecordV3, ExploreRecord, ExploreRecordV1, ExploreRecordV2, ExploreRecordV3,
};
use workflow::state::ChangeStatus;

// ---------------------------------------------------------------------------
// explore / change 模型 v3 / v4（desktop-test-only-flow）：explore 新增
// `workflow_type` 字段面（v2 → v3 decode-only 升级，缺列读出默认
// `"requirement"`）+ 升级链双向 `From`（v1 ⇄ v2 ⇄ v3）。
// ---------------------------------------------------------------------------

/// ExploreRecord v4 回环：title / workflow_type 两态（requirement 与
/// test-only）/ promoted_to 两态 / session_id 两态构造 → encode / decode 往返逐字段相等
///（版本头 = 4）。
#[test]
fn explore_record_v4回环_session_id两态逐字段相等() {
    for (workflow_type, promoted_to, session_id) in [
        ("requirement", None, None),
        (
            "test-only",
            Some("0198f7a0-0000-7000-8000-0000000000c9".to_owned()),
            Some("ses-3-1727000000000".to_owned()),
        ),
    ] {
        let record = ExploreRecord {
            id: 7,
            root: r"C:\ws\alpha".to_owned(),
            name: "api-retry".to_owned(),
            title: "接口重试策略".to_owned(),
            workflow_type: workflow_type.to_owned(),
            promoted_to: promoted_to.clone(),
            session_id: session_id.clone(),
            created_at: 1_727_000_000_000,
            updated_at: 1_727_000_090_000,
        };
        let bytes = native_model::encode(&record).expect("encode v4 应成功");
        let (decoded, version) =
            native_model::decode::<ExploreRecord>(bytes).expect("decode v4 应成功");
        assert_eq!(
            version, 4,
            "ExploreRecord 4:v4 版本头（session_id 字段面入册）"
        );
        assert_eq!(
            decoded, record,
            "v4 往返逐字段相等（workflow_type {workflow_type} / promoted_to {promoted_to:?} / session_id {session_id:?}）"
        );
        assert_eq!(decoded.id, 7, "id 主键往返保真（身份锚）");
        assert_eq!(decoded.name, "api-retry", "name 寻址键往返保真");
        assert_eq!(decoded.title, "接口重试策略", "title 独立字段往返保真");
        assert_eq!(
            decoded.workflow_type, workflow_type,
            "workflow_type 两态往返保真"
        );
        assert_eq!(decoded.promoted_to, promoted_to, "promoted_to 两态保真");
        assert_eq!(decoded.session_id, session_id, "session_id 两态保真");
    }
}

/// ExploreRecord V2 → V3 → V4 decode-only 升级：V2 形态记录 encode → 以 V4
/// 解码 → `workflow_type == "requirement"`、`session_id == None`（缺列读出默认），
/// 其余字段逐字直透；零 `migrate` 调用。
#[test]
fn explore_record_v2到v4_decode_only升级_workflow_type与session_id默认() {
    for promoted_to in [
        None,
        Some("0198f7a0-0000-7000-8000-0000000000d1".to_owned()),
    ] {
        let legacy = ExploreRecordV2 {
            id: 5,
            root: r"C:\ws\v2".to_owned(),
            name: "v2-topic".to_owned(),
            title: "存量标题".to_owned(),
            promoted_to: promoted_to.clone(),
            created_at: 1_700_000_000_000,
            updated_at: 1_710_000_000_000,
        };
        let legacy_bytes = native_model::encode(&legacy).expect("encode v2 应成功");
        // v2 载荷以 v2 模型解码逐字段相等（历史形态读出不变）
        let (legacy_decoded, legacy_version) =
            native_model::decode::<ExploreRecordV2>(legacy_bytes.clone()).expect("v2 解码应成功");
        assert_eq!(legacy_version, 2, "存量形态封装为 version 2");
        assert_eq!(legacy_decoded, legacy, "v2 载荷按 v2 解码逐字段相等");

        // 同一载荷经版本机制透明升级为 v4（零 migrate 调用）
        let (upgraded, version) =
            native_model::decode::<ExploreRecord>(legacy_bytes).expect("v2 载荷应升级为 v4");
        assert_eq!(version, 2, "decode 返回载荷头版本（升级链源版本）");
        assert_eq!(
            upgraded.workflow_type, "requirement",
            "缺列读兼容：存量记录读出默认 requirement（零迁移）"
        );
        assert_eq!(upgraded.session_id, None, "session_id 缺列读出默认 None");
        assert_eq!(upgraded.title, "存量标题", "title 直透（v2 已有列）");
        assert_eq!(upgraded.promoted_to, promoted_to, "promoted_to 直透");
        assert_eq!(upgraded.id, legacy.id, "id 主键逐字一致");
        assert_eq!(upgraded.root, legacy.root, "root 归属逐字一致");
        assert_eq!(upgraded.name, legacy.name, "name 寻址键逐字一致");
        assert_eq!(
            upgraded.created_at, legacy.created_at,
            "created_at 逐字一致"
        );
        assert_eq!(
            upgraded.updated_at, legacy.updated_at,
            "updated_at 逐字一致"
        );
        // 升级形态回环无损（v4 编码 → v4 解码）
        let round_trip = native_model::encode(&upgraded).expect("升级形态 encode 应成功");
        let (again, again_version) =
            native_model::decode::<ExploreRecord>(round_trip).expect("v4 回环解码应成功");
        assert_eq!(again_version, 4);
        assert_eq!(again, upgraded, "升级形态再编码回环逐字段相等");
    }
}

/// ExploreRecord native_model 元数据（边界）：`version = 4` 且升级链
/// `from = ExploreRecordV3`；物表名版本段随之升为 `4_4_id`（旧表 4_3_id 对新
/// 读面不可见）。
#[test]
fn explore_record_v4元数据_版本升四且升级链自v3() {
    assert_eq!(
        (
            <ExploreRecord as native_model::Model>::native_model_id(),
            <ExploreRecord as native_model::Model>::native_model_version()
        ),
        (4, 4),
        "ExploreRecord 4:v4（新增 session_id 列）"
    );
    assert_eq!(
        (
            <ExploreRecordV3 as native_model::Model>::native_model_id(),
            <ExploreRecordV3 as native_model::Model>::native_model_version()
        ),
        (4, 3),
        "中转形态 ExploreRecordV3 4:v3 入升级链"
    );
    assert_eq!(
        (
            <ExploreRecordV2 as native_model::Model>::native_model_id(),
            <ExploreRecordV2 as native_model::Model>::native_model_version()
        ),
        (4, 2),
        "中转形态 ExploreRecordV2 4:v2 入升级链"
    );
    assert_eq!(
        (
            <ExploreRecordV1 as native_model::Model>::native_model_id(),
            <ExploreRecordV1 as native_model::Model>::native_model_version()
        ),
        (4, 1),
        "链首 ExploreRecordV1 4:v1"
    );

    // 升级链 V1 → V2 → V3 → V4 全程可解码（逐跳 version 头可辨）
    let v1 = ExploreRecordV1 {
        id: 1,
        root: r"C:\ws\chain".to_owned(),
        name: "chain-topic".to_owned(),
        created_at: 1,
        updated_at: 2,
    };
    let (as_v2, v2_version) = native_model::decode::<ExploreRecordV2>(
        native_model::encode(&v1).expect("encode v1 应成功"),
    )
    .expect("v1 → v2 应可解码");
    assert_eq!(v2_version, 1, "解码返回源载荷版本");
    assert_eq!(as_v2.title, "chain-topic", "v1 → v2：title 兜底 name");
    assert_eq!(as_v2.promoted_to, None);

    let (as_v3, v3_version) = native_model::decode::<ExploreRecordV3>(
        native_model::encode(&as_v2).expect("encode v2 应成功"),
    )
    .expect("v2 → v3 应可解码");
    assert_eq!(v3_version, 2);
    assert_eq!(
        as_v3.workflow_type, "requirement",
        "升级链读出默认 requirement"
    );
    assert_eq!(as_v3.title, "chain-topic");

    let (as_v4, v4_version) = native_model::decode::<ExploreRecord>(
        native_model::encode(&as_v3).expect("encode v3 应成功"),
    )
    .expect("v3 → v4 应可解码");
    assert_eq!(v4_version, 3);
    assert_eq!(as_v4.workflow_type, "requirement");
    assert_eq!(
        as_v4.session_id, None,
        "升级链末端 session_id = None（缺列读兼容）"
    );
    assert_eq!(as_v4.title, "chain-topic");
    assert_eq!(as_v4.id, 1);
}

/// From<ExploreRecord> for ExploreRecordV2（边界）：降级半边形经 V3 中转丢弃
/// `session_id` 与 `workflow_type`，其余字段逐字一致（只保升级语义真实性，降级形态不作数据
/// 承诺）；降级后升级回来 workflow_type 归默认 `"requirement"`、session_id 归 None。
#[test]
fn explore_record_v4到v2降级半边经v3中转丢弃session_id与workflow_type() {
    let record = ExploreRecord {
        id: 11,
        root: r"C:\ws\down".to_owned(),
        name: "down-grade".to_owned(),
        title: "降级样本".to_owned(),
        workflow_type: "test-only".to_owned(),
        promoted_to: Some("chg-9".to_owned()),
        session_id: Some("ses-1-1700000000000".to_owned()),
        created_at: 1_727_000_000_000,
        updated_at: 1_727_000_080_000,
    };
    let downgraded = ExploreRecordV2::from(ExploreRecordV3::from(record.clone()));
    assert_eq!(downgraded.id, record.id);
    assert_eq!(downgraded.root, record.root);
    assert_eq!(downgraded.name, record.name);
    assert_eq!(downgraded.title, record.title);
    assert_eq!(downgraded.promoted_to, record.promoted_to);
    assert_eq!(downgraded.created_at, record.created_at);
    assert_eq!(downgraded.updated_at, record.updated_at);

    // 再降一跳：v2 → v1 丢弃 title / promoted_to
    let oldest = ExploreRecordV1::from(downgraded);
    assert_eq!(oldest.id, record.id);
    assert_eq!(oldest.name, record.name);
    assert_eq!(oldest.created_at, record.created_at);
    assert_eq!(oldest.updated_at, record.updated_at);

    // 升级回来：workflow_type 归默认 requirement、session_id 归 None（降级不保）
    let round_trip = ExploreRecord::from(ExploreRecordV3::from(ExploreRecordV2::from(
        ExploreRecordV1 {
            id: record.id,
            root: record.root.clone(),
            name: record.name.clone(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        },
    )));
    assert_eq!(
        round_trip.workflow_type, "requirement",
        "降级丢弃后升级归默认 requirement"
    );
    assert_eq!(
        round_trip.session_id, None,
        "降级丢弃后升级 session_id 归 None"
    );
    assert_eq!(
        round_trip.title, record.name,
        "降级丢弃后升级兜底 title = name"
    );
    assert_eq!(
        round_trip.promoted_to, None,
        "降级丢弃后升级 promoted_to = None"
    );
}

/// ExploreRecordV2 历史形态（边界 / AC-9）：v2 恰七字段（无 `workflow_type`）；
/// `From<ExploreRecordV1>` 平移 title = name、promoted_to = None，其余字段直透。
#[test]
fn explore_record_v2历史形态七字段无workflow_type() {
    let v1 = ExploreRecordV1 {
        id: 9,
        root: r"C:\ws\hist".to_owned(),
        name: "hist-topic".to_owned(),
        created_at: 1_600_000_000_000,
        updated_at: 1_600_000_500_000,
    };
    let v2 = ExploreRecordV2::from(v1.clone());

    assert_eq!(v2.id, v1.id);
    assert_eq!(v2.root, v1.root);
    assert_eq!(v2.name, v1.name);
    assert_eq!(v2.title, v1.name, "v1 → v2：title 兜底 name");
    assert_eq!(v2.promoted_to, None, "v1 → v2：promoted_to 空起步");
    assert_eq!(v2.created_at, v1.created_at);
    assert_eq!(v2.updated_at, v1.updated_at);

    // v2 有 title / promoted_to、无 workflow_type：以 v2 解码读出的字段面即七字段
    let bytes = native_model::encode(&v2).expect("encode v2 应成功");
    let value = serde_json::to_value(&v2).expect("v2 出线应成功");
    let keys: Vec<String> = value
        .as_object()
        .expect("v2 线形应为对象")
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys.len(), 7, "v2 恰七字段，实际: {keys:?}");
    assert!(
        !keys.iter().any(|key| key == "workflowType"),
        "v2 无 workflow_type 列（版本演进事实）"
    );
    let (back, version) = native_model::decode::<ExploreRecordV2>(bytes).expect("v2 回环应成功");
    assert_eq!(version, 2);
    assert_eq!(back, v2, "v2 回环逐字段相等");
}

/// ExploreRecord V1 → V4 decode-only 升级：V1 形态记录 encode → 以当前
/// 形态解码 → `title == name`、`promoted_to == None`、`workflow_type ==
/// "requirement"`、`session_id == None`、其余字段逐字一致；零 `migrate` 调用
///（升级由 native_model `from` 链 V1 → V2 → V3 → V4 在读取时透明完成）。
#[test]
fn explore_record_v1到v4_decode_only升级_title兜底name且零迁移() {
    let legacy = ExploreRecordV1 {
        id: 3,
        root: r"C:\ws\legacy".to_owned(),
        name: "legacy-topic".to_owned(),
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_500_000,
    };
    let legacy_bytes = native_model::encode(&legacy).expect("encode v1 应成功");
    // v1 载荷以 v1 模型解码逐字段相等
    let (legacy_decoded, legacy_version) =
        native_model::decode::<ExploreRecordV1>(legacy_bytes.clone()).expect("v1 解码应成功");
    assert_eq!(legacy_version, 1, "存量形态封装为 version 1");
    assert_eq!(legacy_decoded, legacy, "v1 载荷按 v1 解码逐字段相等");

    // 同一载荷经版本机制透明升级为 v4（零 migrate 调用）
    let (upgraded, version) =
        native_model::decode::<ExploreRecord>(legacy_bytes).expect("v1 载荷应升级为 v4");
    assert_eq!(version, 1, "decode 返回载荷头版本（升级链源版本）");
    assert_eq!(
        upgraded.title, upgraded.name,
        "升级半边补 title = name（旧记录无标题列，展示语义与现状一致）"
    );
    assert_eq!(upgraded.title, "legacy-topic");
    assert_eq!(
        upgraded.promoted_to, None,
        "升级半边 promoted_to = None（草稿态）"
    );
    assert_eq!(
        upgraded.workflow_type, "requirement",
        "升级末端 workflow_type = requirement（缺列读兼容）"
    );
    assert_eq!(
        upgraded.session_id, None,
        "升级末端 session_id = None（缺列读兼容）"
    );
    assert_eq!(upgraded.id, legacy.id, "id 主键逐字一致");
    assert_eq!(upgraded.root, legacy.root, "root 归属逐字一致");
    assert_eq!(upgraded.name, legacy.name, "name 寻址键逐字一致");
    assert_eq!(
        upgraded.created_at, legacy.created_at,
        "created_at 逐字一致"
    );
    assert_eq!(
        upgraded.updated_at, legacy.updated_at,
        "updated_at 逐字一致"
    );
}

/// ExploreRecord V2 → V1 降级半边（边界）：双向 `From` 的降级方向丢弃
/// `title` / `promoted_to`，剩余字段逐字一致（运行时无降级读取路径，仅
/// native_model `from` 属性的语义完备性）。
#[test]
fn explore_record_v2到v1降级半边丢弃新字段() {
    let record = ExploreRecord {
        id: 11,
        root: r"C:\ws\down".to_owned(),
        name: "down-grade".to_owned(),
        title: "降级样本".to_owned(),
        workflow_type: "requirement".to_owned(),
        promoted_to: Some("chg-9".to_owned()),
        session_id: None,
        created_at: 1_727_000_000_000,
        updated_at: 1_727_000_080_000,
    };
    let v2 = ExploreRecordV2::from(ExploreRecordV3::from(record.clone()));
    let downgraded = ExploreRecordV1::from(v2);
    assert_eq!(downgraded.id, record.id);
    assert_eq!(downgraded.root, record.root);
    assert_eq!(downgraded.name, record.name);
    assert_eq!(downgraded.created_at, record.created_at);
    assert_eq!(downgraded.updated_at, record.updated_at);
    // 再升级回来：新字段按升级语义兜底（title = name、promoted_to = None、
    // workflow_type = requirement、session_id = None）
    let round_trip = ExploreRecord::from(ExploreRecordV3::from(ExploreRecordV2::from(downgraded)));
    assert_eq!(
        round_trip.title, record.name,
        "降级丢弃后升级兜底 title = name"
    );
    assert_eq!(
        round_trip.promoted_to, None,
        "降级丢弃后升级 promoted_to = None"
    );
    assert_eq!(
        round_trip.workflow_type, "requirement",
        "降级丢弃后升级 workflow_type = requirement"
    );
    assert_eq!(
        round_trip.session_id, None,
        "降级丢弃后升级 session_id = None"
    );
    assert_eq!(round_trip.id, record.id);
}

/// ExploreRecord.new 默认 title = name（正向）：`new(root, name, now)` →
/// `title == name`、`workflow_type == "requirement"`、`promoted_to == None`、
/// `id == 0`、`created_at == updated_at == now`。
#[test]
fn explore_record_new默认title等于name且promoted_to为none() {
    let now = 1_727_000_000_000;
    let record = ExploreRecord::new(r"C:\ws\alpha", "api-retry", now);

    assert_eq!(
        record.title, record.name,
        "构造默认 title = name（恒非空不变量兜底）"
    );
    assert_eq!(record.title, "api-retry");
    assert_eq!(
        record.workflow_type, "requirement",
        "构造默认 workflow_type = requirement（缺省语义单点）"
    );
    assert_eq!(record.promoted_to, None, "构造默认草稿态");
    assert_eq!(record.id, 0, "id 置 0 由写事务 max+1 覆盖");
    assert_eq!(record.created_at, now);
    assert_eq!(record.updated_at, now, "新建语义 created_at = updated_at");
}

/// v4 回环：id 主键（身份锚）+ name 普通属性 + title 显式值 + worktree /
/// base_commit 双态构造 → encode / decode 往返逐字段相等（版本头 = 4）。
#[test]
fn change_record_v4回环含title与worktree双态逐字段相等() {
    for (worktree, base_commit) in [
        (
            Some(r"C:\app-data\worktrees\seg\fix-bug".to_owned()),
            Some("0000000000000000000000000000000000000001".to_owned()),
        ),
        (None, None),
    ] {
        let record = ChangeRecord {
            id: "chg-018f3a-0001".to_owned(),
            name: "fix-bug".to_owned(),
            title: "修复登录竞态".to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: 1_727_000_000_000,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: worktree.clone(),
            base_commit: base_commit.clone(),
        };
        let bytes = native_model::encode(&record).expect("encode v4 应成功");
        let (decoded, version) =
            native_model::decode::<ChangeRecord>(bytes).expect("decode v4 应成功");
        assert_eq!(version, 4, "ChangeRecord 9:v4 版本头（v4 新增 title 列）");
        assert_eq!(
            decoded, record,
            "v4 往返逐字段相等（worktree {worktree:?} / base_commit {base_commit:?}）"
        );
        assert_eq!(decoded.id, "chg-018f3a-0001", "id 主键往返保真（身份锚）");
        assert_eq!(
            decoded.name, "fix-bug",
            "name 普通属性往返保真（非主键，无唯一约束）"
        );
        assert_eq!(decoded.title, "修复登录竞态", "title 独立字段往返保真");
    }
}

/// ChangeRecord V3 → V4 decode-only 升级（AC-2）：V3 形态记录 encode → 以 V4
/// 解码 → `title == name`，其余字段（含 archived_at / active_phase / worktree
/// / base_commit 两态）逐字一致。
#[test]
fn change_record_v3到v4_decode_only升级_title兜底name且零迁移() {
    let legacy = ChangeRecordV3 {
        id: "chg-legacy-0001".to_owned(),
        name: "legacy-change".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1_700_000_000_000,
        status: ChangeStatus::Archived,
        archived_at: Some(1_700_000_900_000),
        active_phase: None,
        worktree: None,
        base_commit: Some("0000000000000000000000000000000000000002".to_owned()),
    };
    let legacy_bytes = native_model::encode(&legacy).expect("encode v3 应成功");
    let (legacy_decoded, legacy_version) =
        native_model::decode::<ChangeRecordV3>(legacy_bytes.clone()).expect("v3 解码应成功");
    assert_eq!(legacy_version, 3, "存量形态封装为 version 3");
    assert_eq!(legacy_decoded, legacy, "v3 载荷按 v3 解码逐字段相等");

    let (upgraded, version) =
        native_model::decode::<ChangeRecord>(legacy_bytes).expect("v3 载荷应升级为 v4");
    assert_eq!(version, 3, "decode 返回载荷头版本（升级链源版本）");
    assert_eq!(
        upgraded.title, upgraded.name,
        "升级半边补 title = name（缺列读兼容）"
    );
    assert_eq!(upgraded.title, "legacy-change");
    assert_eq!(upgraded.id, legacy.id, "id 主键逐字一致（身份锚零换锚）");
    assert_eq!(upgraded.name, legacy.name);
    assert_eq!(upgraded.workflow_type, legacy.workflow_type);
    assert_eq!(upgraded.created_at, legacy.created_at);
    assert_eq!(upgraded.status, ChangeStatus::Archived);
    assert_eq!(upgraded.archived_at, legacy.archived_at);
    assert_eq!(upgraded.base_commit, legacy.base_commit);
}

/// ChangeRecord V4 → V3 降级半边（边界）：双向 `From` 的降级方向丢弃
/// `title`，剩余字段逐字一致（运行时无降级读取路径）。
#[test]
fn change_record_v4到v3降级半边丢弃title() {
    let record = ChangeRecord {
        id: "chg-down-1".to_owned(),
        name: "down-change".to_owned(),
        title: "降级标题".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1_727_000_000_000,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: Some(r"C:\wt\down-change".to_owned()),
        base_commit: None,
    };
    let downgraded = ChangeRecordV3::from(record.clone());
    assert_eq!(downgraded.id, record.id);
    assert_eq!(downgraded.name, record.name);
    assert_eq!(downgraded.workflow_type, record.workflow_type);
    assert_eq!(downgraded.created_at, record.created_at);
    assert_eq!(downgraded.status, record.status);
    assert_eq!(downgraded.archived_at, record.archived_at);
    assert_eq!(downgraded.worktree, record.worktree);
    assert_eq!(downgraded.base_commit, record.base_commit);
    // 再升级回来：title 按升级语义兜底（= name）
    let round_trip = ChangeRecord::from(downgraded);
    assert_eq!(
        round_trip.title, record.name,
        "降级丢弃后升级兜底 title = name"
    );
    assert_eq!(round_trip.id, record.id);
}

/// title 非空不变量（边界）：构造与升级两路径下 title 恒非空（默认 = name
/// 兜底）；`promoted_to` 默认 None。
#[test]
fn title恒非空不变量_构造与升级两路径默认name兜底() {
    // 构造路径：name 非空 → title 非空同值
    let explore = ExploreRecord::new("C:\\ws\\alpha", "topic-a", 1);
    assert!(!explore.title.is_empty(), "构造路径 title 恒非空");
    assert_eq!(explore.title, explore.name);
    assert_eq!(explore.promoted_to, None);

    let change = ChangeRecord::new("chg-1", "change-a", "requirement", 1, None, None);
    assert!(!change.title.is_empty(), "构造路径 title 恒非空");
    assert_eq!(change.title, change.name);

    // 升级路径：V1 / V3 升级半边 title 亦非空（= name）
    let upgraded_explore = ExploreRecord::from(ExploreRecordV3::from(ExploreRecordV2::from(
        ExploreRecordV1 {
            id: 1,
            root: "C:\\ws\\alpha".to_owned(),
            name: "upgraded-topic".to_owned(),
            created_at: 1,
            updated_at: 2,
        },
    )));
    assert_eq!(upgraded_explore.title, "upgraded-topic");
    assert!(!upgraded_explore.title.is_empty());
    assert_eq!(upgraded_explore.promoted_to, None);
    assert_eq!(
        upgraded_explore.workflow_type, "requirement",
        "升级路径 workflow_type 默认 requirement"
    );

    let upgraded_change = ChangeRecord::from(ChangeRecordV3 {
        id: "chg-up".to_owned(),
        name: "upgraded-change".to_owned(),
        workflow_type: "requirement".to_owned(),
        created_at: 1,
        status: ChangeStatus::Active,
        archived_at: None,
        active_phase: None,
        worktree: None,
        base_commit: None,
    });
    assert_eq!(upgraded_change.title, "upgraded-change");
    assert!(!upgraded_change.title.is_empty());
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

/// new 携 id 构造（重写）：`new(id, name, workflow_type, created_at, worktree,
/// base_commit)` 逐字段对位、title 默认 = name；status 恒 active 起步 /
/// archived_at / active_phase 空起步持衡。
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
    assert_eq!(
        record.title, record.name,
        "构造默认 title = name（恒非空不变量兜底）"
    );
    assert_eq!(record.title, "显式传入");
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
        legacy_shape.title, "legacy形态",
        "legacy 形态 title 亦兜底 name"
    );
    assert_eq!(
        legacy_shape.worktree, None,
        "None 传入 = legacy 主 root 形态"
    );
    assert_eq!(legacy_shape.base_commit, None);
}

/// 身份锚不变（边界）：ExploreRecord.id 主键（i64 max+1 分配）与 ChangeRecord.id
/// 主键（UUID 形态）零换锚；name 均降为可变属性（无唯一约束）——同 name 不同
/// id 两条记录合法且不相等。
#[test]
fn 身份锚不变_explore与change主键零换锚且name为可变属性() {
    let explore_a = ExploreRecord::new("C:\\ws\\alpha", "同名-topic", 1);
    let mut explore_b = ExploreRecord::new("C:\\ws\\alpha", "同名-topic", 1);
    explore_b.id = 2;
    assert_eq!(explore_a.name, explore_b.name, "同 name 合法（无唯一约束）");
    assert_ne!(explore_a.id, explore_b.id, "id 互异（主键身份锚）");
    assert_eq!(
        explore_a.title, explore_b.title,
        "title 与 name 同源默认（同名同题）"
    );

    let change_a = ChangeRecord::new("chg-甲", "同名档", "requirement", 1000, None, None);
    let change_b = ChangeRecord::new("chg-乙", "同名档", "requirement", 1000, None, None);
    assert_eq!(
        change_a.name, change_b.name,
        "同 name 合法（change name 无唯一约束）"
    );
    assert_ne!(change_a.id, change_b.id, "id 互异（身份锚独立可辨）");
    assert_ne!(
        change_a, change_b,
        "同 name 不同 id 两条记录不相等（name 非身份键）"
    );

    // name 为可变属性：改 name 不换主键（in-place 语义的类型面）
    let mutated = ExploreRecord {
        name: "renamed-topic".to_owned(),
        ..explore_a.clone()
    };
    assert_eq!(mutated.id, explore_a.id, "改名不换主键（保会话链）");
    assert_eq!(
        mutated.title, explore_a.title,
        "title 随记录保留（改名不动 title）"
    );
}

/// 表名版本段公式锚（`{native_model_id}_{version}_{主键字段名小写}`——与
/// store_test 物表名常量同源）：ExploreRecord 4:v3 / ChangeRecord 9:v4 /
/// PhaseRecord 10:v2 / StepRecord 12:v2 / RunRecord 13:v2 / StoreMetaRecord 15:v1。
#[test]
fn 表名版本段_native_model_id与version断言_公式锚() {
    let table = |id: u32, version: u32, key_field: &str| format!("{id}_{version}_{key_field}");

    assert_eq!(
        (
            <ExploreRecord as native_model::Model>::native_model_id(),
            <ExploreRecord as native_model::Model>::native_model_version()
        ),
        (4, 4),
        "ExploreRecord 4:v4（新增 session_id 列——物表 4_3_id → 4_4_id）"
    );
    assert_eq!(
        table(4, 4, "id"),
        "4_4_id",
        "explore 物表名公式锚（旧表 4_3_id 对新读面不可见）"
    );

    assert_eq!(
        (
            <ChangeRecord as native_model::Model>::native_model_id(),
            <ChangeRecord as native_model::Model>::native_model_version()
        ),
        (9, 4),
        "ChangeRecord 9:v4（新增 title 列——物表 9_3_id → 9_4_id）"
    );
    assert_eq!(
        table(9, 4, "id"),
        "9_4_id",
        "change 建档物表名公式锚（旧表 9_3_id 对新读面不可见）"
    );

    // 旧形态类型（仅升级链解码目标，不注册）：版本段恒为历史值
    assert_eq!(
        (
            <ExploreRecordV1 as native_model::Model>::native_model_id(),
            <ExploreRecordV1 as native_model::Model>::native_model_version()
        ),
        (4, 1),
        "ExploreRecordV1 4:v1（pub(crate) 不注册进 workspace_models）"
    );
    assert_eq!(
        (
            <ChangeRecordV3 as native_model::Model>::native_model_id(),
            <ChangeRecordV3 as native_model::Model>::native_model_version()
        ),
        (9, 3),
        "ChangeRecordV3 9:v3（pub(crate) 不注册进 workspace_models）"
    );

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
        WORKSPACE_STORE_FORMAT_VERSION, 3,
        "当前格式版本 = 3（run 执行图节点 / 边两模型入册；2 = change 身份锚 id 形态）"
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
    assert_eq!(decoded.format_version, 3);

    // serde camelCase 线格式（内部治理记录——不入信封注册表，无查看面）
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");
    assert_eq!(value["key"], serde_json::json!("format"));
    assert_eq!(value["formatVersion"], serde_json::json!(3));
}

// ---------------------------------------------------------------------------
// run 运行史两模型（unify-run-state-persistence）：native_model 回环 + 打包键
// ---------------------------------------------------------------------------

use crate::model::{pack_run_seq_key, RunEdgeRecord, RunNodeRecord, RunRecord};
use workflow::state::{
    RunEdgeEntry, RunEdgeKind, RunNodeEntry, RunStartCommand, RunStatus, RunStepKind, RunStepStatus,
};

/// run 发起命令 fixture（RunRecord::new 组装面；change_id = 身份锚 id）。
fn run_start_command() -> RunStartCommand {
    RunStartCommand {
        run_id: "run-1727000000000".to_owned(),
        change_id: "chg-demo".to_owned(),
        started_at: 1_727_000_000_000,
    }
}

/// run 收口节点整包条目 fixture（稳定身份键：实例号显式注入）。
fn run_node_entry(step: RunStepKind, status: RunStepStatus, instance_no: u32) -> RunNodeEntry {
    RunNodeEntry {
        phase: "implement".to_owned(),
        attempt: 1,
        step,
        instance_no,
        status,
        session_id: None,
        detail: None,
        output_ref: None,
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_060_000),
        token_input: None,
        token_output: None,
    }
}

/// run 收口边整包条目 fixture（端点 = 节点身份串）。
fn run_edge_entry(kind: RunEdgeKind) -> RunEdgeEntry {
    RunEdgeEntry {
        from_node_key: "implement:1:executor:0".to_owned(),
        to_node_key: "implement:1:static_check:0".to_owned(),
        edge_kind: kind,
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

/// RunNodeRecord 写读回环逐字段一致（**十值全词汇零过滤**）：step 十词汇、
/// status 四值、session_id / detail / output_ref 三态；`node_seq()` 读面 = 打包
/// 键低 64 位；`node_id()` 身份串 = 边表端点口径（AC-3）。
#[test]
fn run_node_record写读回环_十词汇与三态字段() {
    let all_kinds = [
        (RunStepKind::Executor, RunStepStatus::Passed),
        (RunStepKind::Evaluator, RunStepStatus::Failed),
        (RunStepKind::Decision, RunStepStatus::Passed),
        (RunStepKind::StaticCheck, RunStepStatus::Running),
        (RunStepKind::TestExecution, RunStepStatus::Stopped),
        (RunStepKind::PhaseStart, RunStepStatus::Passed),
        (RunStepKind::PhaseLog, RunStepStatus::Passed),
        (RunStepKind::VerdictGate, RunStepStatus::Failed),
        (RunStepKind::RetryGate, RunStepStatus::Passed),
        (RunStepKind::WhitelistGate, RunStepStatus::Passed),
    ];
    for (sequence, (kind, status)) in all_kinds.iter().enumerate() {
        let record = RunNodeRecord::new(
            "run-1727000000000",
            "chg-demo",
            sequence as u64,
            &run_node_entry(*kind, *status, 0),
            1_727_000_060_000,
        );
        assert_eq!(record.timestamp, 1_727_000_060_000, "整包同刻随构造注入");
        assert_eq!(
            record.node_seq(),
            sequence as u64,
            "node_seq() = 打包键低 64 位"
        );
        assert_eq!(record.change_id, "chg-demo", "归属列 change_id 随命令");

        let bytes = native_model::encode(&record).expect("encode 应成功");
        let (decoded, version) =
            native_model::decode::<RunNodeRecord>(bytes).expect("decode 应成功");
        assert_eq!(version, 1, "native_model id=16 版本封装 version 1");
        assert_eq!(decoded, record, "{kind:?}/{status:?} 回环逐字段相等");
        assert_eq!(
            decoded.step, *kind,
            "十值全词汇零过滤（流程面 / 三门亦落行）"
        );
        assert_eq!(decoded.status, *status);
    }

    // session_id / detail / output_ref 三态（None = 工具步 / Transient 形态）
    let bare = RunNodeRecord::new(
        "run-1",
        "chg-demo",
        0,
        &run_node_entry(RunStepKind::StaticCheck, RunStepStatus::Passed, 0),
        1_727_000_060_000,
    );
    assert_eq!(bare.session_id, None);
    assert_eq!(bare.detail, None);
    assert_eq!(bare.output_ref, None, "Transient 恒 None");
    let bytes = native_model::encode(&bare).expect("encode 应成功");
    let (decoded, _) = native_model::decode::<RunNodeRecord>(bytes).expect("decode 应成功");
    assert_eq!(decoded, bare, "None 三态回环不漂移");

    // Some 三态 + 稳定实例键（feedback 重跑递增实例）
    let mut entry = run_node_entry(RunStepKind::Executor, RunStepStatus::Passed, 1);
    entry.session_id = Some("ses-1".to_owned());
    entry.detail = Some("修复重跑".to_owned());
    entry.output_ref = Some("exec-1727000060000".to_owned());
    let record = RunNodeRecord::new("run-1", "chg-demo", 7, &entry, 1_727_000_060_000);
    assert_eq!(record.instance_no, 1, "实例号随整包落行");
    assert_eq!(
        record.node_id(),
        "implement:1:executor:1",
        "节点身份串 = phase:attempt:step:instance_no"
    );
    let bytes = native_model::encode(&record).expect("encode 应成功");
    let (decoded, _) = native_model::decode::<RunNodeRecord>(bytes).expect("decode 应成功");
    assert_eq!(decoded, record, "Some 三态 + 实例号回环不漂移");
}

/// RunEdgeRecord 写读回环逐字段一致（**四 kind 零过滤**）：`edge_kind` 四值、
/// 端点身份串（`node_id()` 口径）；`edge_seq()` 读面 = 打包键低 64 位；非法
/// kind 词反序列化拒绝（封闭集防线，AC-3）。
#[test]
fn run_edge_record写读回环_四kind与端点串() {
    for (sequence, kind) in [
        RunEdgeKind::Forward,
        RunEdgeKind::Feedback,
        RunEdgeKind::Retry,
        RunEdgeKind::Backtrack,
    ]
    .iter()
    .enumerate()
    {
        let record = RunEdgeRecord::new(
            "run-1727000000000",
            "chg-demo",
            sequence as u64,
            &run_edge_entry(*kind),
            1_727_000_060_000,
        );
        assert_eq!(record.edge_seq(), sequence as u64);
        assert_eq!(record.from_node_key, "implement:1:executor:0");
        assert_eq!(record.to_node_key, "implement:1:static_check:0");

        let bytes = native_model::encode(&record).expect("encode 应成功");
        let (decoded, version) =
            native_model::decode::<RunEdgeRecord>(bytes).expect("decode 应成功");
        assert_eq!(version, 1, "native_model id=17 版本封装 version 1");
        assert_eq!(decoded, record, "{kind:?} 回环逐字段相等");
        assert_eq!(decoded.edge_kind, *kind, "四 kind 零过滤回环保真");
    }

    // 非法 edge_kind 词反序列化拒绝（封闭集防线）
    let mut value = serde_json::to_value(RunEdgeRecord::new(
        "run-1",
        "chg-demo",
        0,
        &run_edge_entry(RunEdgeKind::Feedback),
        1_727_000_060_000,
    ))
    .expect("出线应成功");
    value["edgeKind"] = serde_json::json!("feed_back");
    let back: Result<RunEdgeRecord, _> = serde_json::from_value(value);
    assert!(back.is_err(), "边行非法 kind 词应拒绝");
}

/// 打包键数值序即分层序（AC-3 / AC-9 全史叠加库面前置）：同 run_id 下 seq 递增
/// 键严格递增；异 run_id 键域不相交（哈希高 64 位分域）；节点行 / 边行同族打包
/// （运行期零碰撞）。
#[test]
fn pack_run_seq_key数值序即分层序_异run键域不相交() {
    // 同 run_id：seq 递增键严格递增
    let mut keys: Vec<u128> = [0u64, 1, 2]
        .iter()
        .map(|seq| pack_run_seq_key("run-a", *seq))
        .collect();
    keys.sort();
    assert!(
        keys.windows(2).all(|w| w[0] < w[1]),
        "同 run 内 seq 递增键严格递增"
    );

    // 异 run_id：全键域不相交（高 64 位 = hash64(run_id) 分域）
    let run_a: Vec<u128> = (0..8u64)
        .map(|seq| pack_run_seq_key("run-a", seq))
        .collect();
    let run_b: Vec<u128> = (0..8u64)
        .map(|seq| pack_run_seq_key("run-b", seq))
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
    // 节点行 / 边行同族打包：同 run 同 seq 键相同（两表分治，零碰撞面）
    assert_eq!(
        pack_run_seq_key("run-a", 3),
        pack_run_seq_key("run-a", 3),
        "打包函数确定性（运行期零随机成分）"
    );
}

/// `node_key` / `edge_key` u128 经 event_key_serde 十六进制线面编码回环无损
///（serde_json 无 u128 数字面——hex 串线面，SessionEventRecord 先例同式，AC-3）。
#[test]
fn run_node_edge_key_hex线面编码回环无损() {
    let node = RunNodeRecord::new(
        "run-1727000000000",
        "chg-demo",
        u64::MAX,
        &run_node_entry(RunStepKind::Executor, RunStepStatus::Passed, u32::MAX),
        1_727_000_060_000,
    );
    let value = serde_json::to_value(&node).expect("serde_json 出线应成功");
    let hex = value["nodeKey"].as_str().expect("nodeKey 出 hex 串");
    assert!(hex.starts_with("0x"), "十六进制串线面（0x 前缀）: {hex}");
    assert_eq!(hex.len(), 34, "u128 满 32 位 hex + 0x 前缀");
    let back: RunNodeRecord = serde_json::from_value(value).expect("反序列化应成功");
    assert_eq!(back, node, "hex 线面回环无损（node_seq 低 64 位全 1 边界）");
    assert_eq!(
        back.node_seq(),
        u64::MAX,
        "node_seq() 读面 = 低 64 位不截断"
    );
    assert_eq!(back.instance_no, u32::MAX, "实例号边界不截断");

    let edge = RunEdgeRecord::new(
        "run-1727000000000",
        "chg-demo",
        u64::MAX,
        &run_edge_entry(RunEdgeKind::Backtrack),
        1_727_000_060_000,
    );
    let value = serde_json::to_value(&edge).expect("serde_json 出线应成功");
    let hex = value["edgeKey"].as_str().expect("edgeKey 出 hex 串");
    assert!(hex.starts_with("0x"));
    assert_eq!(hex.len(), 34);
    let back: RunEdgeRecord = serde_json::from_value(value).expect("反序列化应成功");
    assert_eq!(back, edge, "hex 线面回环无损（edge_seq 边界）");
    assert_eq!(back.edge_seq(), u64::MAX);
}

// ---------------------------------------------------------------------------
// CLI_TENANT_RETIRED 共享拒收文案（DD3）：四拒点逐字复用的常量内容面
// ---------------------------------------------------------------------------

use crate::model::{SessionConfigSnapshotV1, SessionRecordV1};

#[test]
fn cli_tenant_retired文案常量逐字锚_退役事实与引导齐备() {
    // 逐字锚：DD3 定稿文案（多处复用不拼接不加前后缀——防措辞漂移）
    assert_eq!(
        crate::model::CLI_TENANT_RETIRED,
        "CLI 租户已下线：请前往 Agent 管理页（/agents）新建或改用 sdk agent，并将其设为默认（存量 cli 实例可读可删不可改）"
    );
    let text = crate::model::CLI_TENANT_RETIRED;
    assert!(!text.is_empty(), "非空 &str");
    assert!(text.contains("CLI 租户已下线"), "退役事实在场");
    assert!(text.contains("/agents"), "管理页引导在场");
    assert!(text.contains("设为默认"), "改默认引导在场");
    assert!(
        text.contains("存量 cli 实例可读可删不可改"),
        "存量行口径在场"
    );
}

#[test]
fn cli_tenant_retired无格式化占位符残留() {
    let text = crate::model::CLI_TENANT_RETIRED;
    assert!(
        !text.contains("{}") && !text.contains("%s") && !text.contains("{0}"),
        "无格式化占位符残留（逐字复用，防多处措辞漂移），实际: {text}"
    );
    // 文案内中文 / /agents 路径 / 全角括号为固定字面（呈现面不转义不截断的
    // 前端半边断言挂 agent-panel 章节）
    assert!(text.contains('（') && text.contains('）'), "全角括号字面");
}

// ---------------------------------------------------------------------------
// AgentEngineKind 变体序号解码锚（AC-3）：bincode 字节级 0 / 1 钉死
// ---------------------------------------------------------------------------

/// bincode 序号锚定装置：以「engine 为首字段」的最小 native_model 记录直读
/// 变体序号（native_model header 8 字节 = id u32 小端 + version u32 小端，
/// bincode 1.3 体首 4 字节 = 枚举变体序号 u32 小端）。仅测试侧锚定，不注册
/// 进库模型组。
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[native_model::native_model(id = 700, version = 1)]
struct EngineOrdinalAnchor {
    engine: AgentEngineKind,
}

#[test]
fn agent_engine_kind变体序号锚_cli恰0_sdk恰1字节级() {
    let cli = native_model::encode(&EngineOrdinalAnchor {
        engine: AgentEngineKind::Cli,
    })
    .expect("encode Cli 锚");
    let sdk = native_model::encode(&EngineOrdinalAnchor {
        engine: AgentEngineKind::Sdk,
    })
    .expect("encode Sdk 锚");
    assert_eq!(
        &cli[8..12],
        &[0, 0, 0, 0],
        "Cli 编码恰序号 0（u32 小端，变体序号钉死）"
    );
    assert_eq!(
        &sdk[8..12],
        &[1, 0, 0, 0],
        "Sdk 编码恰序号 1（u32 小端，变体序号钉死）"
    );
}

#[test]
fn agent_engine_kind反向解码锚字节0与1命中_越界与空体拒解不panic() {
    let sdk = native_model::encode(&EngineOrdinalAnchor {
        engine: AgentEngineKind::Sdk,
    })
    .expect("encode Sdk 锚");

    // 字节 [1] → Sdk（正向回读）
    let (decoded, version) =
        native_model::decode::<EngineOrdinalAnchor>(sdk.clone()).expect("序号 1 解码应成功");
    assert_eq!(decoded.engine, AgentEngineKind::Sdk);
    assert_eq!(version, 1);

    // 字节 [0] → Cli（改写序号位反向锚）
    let mut cli_bytes = sdk.clone();
    cli_bytes[8..12].copy_from_slice(&[0, 0, 0, 0]);
    let (decoded, _) =
        native_model::decode::<EngineOrdinalAnchor>(cli_bytes).expect("序号 0 解码应成功");
    assert_eq!(decoded.engine, AgentEngineKind::Cli);

    // [2] 越界：两变体枚举无序号 2 → 拒解 Err 不 panic（防静默错位——两表报废级红线）
    let mut rogue = sdk.clone();
    rogue[8] = 2;
    assert!(
        native_model::decode::<EngineOrdinalAnchor>(rogue).is_err(),
        "序号 2 越界必须拒解"
    );

    // 空字节串 / 截断体：空体在 native_model header 解析段即 panic（第三方
    // 契约面 `Wrapper::deserialize` unwrap——非我方解码面可控）；截断体 header
    // 可读、体残缺走我方 `Err` 拒解。两者均不静默产出错位行。
    assert!(
        std::panic::catch_unwind(|| native_model::decode::<EngineOrdinalAnchor>(Vec::new()))
            .is_err(),
        "空字节串不静默解码（native_model header 解析 panic，非 Err 契约）"
    );
    assert!(
        native_model::decode::<EngineOrdinalAnchor>(sdk[..10].to_vec()).is_err(),
        "截断体拒解 Err"
    );
}

#[test]
fn agent_engine_kind两表载荷roundtrip逐字段一致() {
    // 含存量两变体行的库 roundtrip 照读零错位：AgentInstanceRecord.engine 与
    // SessionConfigSnapshot.engine（嵌于 SessionRecord）两表载荷 bincode
    // roundtrip 逐字段一致
    for engine in [AgentEngineKind::Cli, AgentEngineKind::Sdk] {
        let instance = AgentInstanceRecord {
            id: 3,
            name: "存量行".to_owned(),
            engine,
            provider_id: None,
            is_default: true,
        };
        let bytes = native_model::encode(&instance).expect("encode instance");
        let (decoded, version) =
            native_model::decode::<AgentInstanceRecord>(bytes).expect("decode instance");
        assert_eq!(version, 1, "实例行 v1 封装零改动");
        assert_eq!(decoded, instance, "实例行 engine roundtrip 逐字段一致");

        let record = SessionRecord {
            id: "ses-engine-rt".to_owned(),
            engine_session_id: Some("h-1".to_owned()),
            config_snapshot: SessionConfigSnapshot {
                engine,
                agent_instance_id: Some(42),
                model: Some("m".to_owned()),
                permission_mode: AgentPermissionMode::Default,
            },
            source: "debug".to_owned(),
            source_ref: None,
            created_at: 1,
            updated_at: 2,
        };
        let bytes = native_model::encode(&record).expect("encode session");
        let (decoded, version) =
            native_model::decode::<SessionRecord>(bytes).expect("decode session");
        assert_eq!(version, 2, "会话行 v2 封装");
        assert_eq!(decoded, record, "会话行快照 engine roundtrip 逐字段一致");
    }
}

// ---------------------------------------------------------------------------
// SessionConfigSnapshot 实例外键与 SessionRecord v1→v2 演进链（DD6 / DD7 / AC-7）
// ---------------------------------------------------------------------------

#[test]
fn 存量v1会话行经版本机制升级_实例外键读none其余原样() {
    let v1 = SessionRecordV1 {
        id: "ses-v1-1727000000000".to_owned(),
        engine_session_id: Some("cli-3-1727000000001".to_owned()),
        config_snapshot: SessionConfigSnapshotV1 {
            engine: AgentEngineKind::Cli,
            model: None,
            permission_mode: AgentPermissionMode::BypassPermissions,
        },
        source: "debug".to_owned(),
        source_ref: None,
        created_at: 1727000000000,
        updated_at: 1727000005000,
    };
    let legacy_bytes = native_model::encode(&v1).expect("encode v1");
    let (legacy_decoded, legacy_version) =
        native_model::decode::<SessionRecordV1>(legacy_bytes.clone()).expect("v1 载荷直解");
    assert_eq!(legacy_version, 1, "v1 封装");
    assert_eq!(legacy_decoded, v1, "v1 形态自身往返逐字段一致");

    let (upgraded, version) =
        native_model::decode::<SessionRecord>(legacy_bytes).expect("v1 载荷应经版本机制升级");
    assert_eq!(
        version, 1,
        "decode 返回载荷头版本（升级链源版本）；升级由值面承载（下方逐字段）"
    );
    assert_eq!(
        upgraded.config_snapshot.agent_instance_id, None,
        "旧快照实例外键读出 None（升级半边真实，零迁移代码路径）"
    );
    assert_eq!(upgraded.id, v1.id);
    assert_eq!(upgraded.engine_session_id, v1.engine_session_id);
    assert_eq!(upgraded.config_snapshot.engine, AgentEngineKind::Cli);
    assert_eq!(upgraded.config_snapshot.model, None);
    assert_eq!(
        upgraded.config_snapshot.permission_mode,
        AgentPermissionMode::BypassPermissions
    );
    assert_eq!(upgraded.source, v1.source);
    assert_eq!(upgraded.source_ref, v1.source_ref);
    assert_eq!(upgraded.created_at, v1.created_at);
    assert_eq!(upgraded.updated_at, v1.updated_at);
}

#[test]
fn v2新会话行快照实例外键some_roundtrip原样() {
    let v2 = SessionRecord {
        id: "ses-v2-1727000000000".to_owned(),
        engine_session_id: Some("sdk-9-1727000000001".to_owned()),
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Sdk,
            agent_instance_id: Some(1727),
            model: Some("m-high".to_owned()),
            permission_mode: AgentPermissionMode::AcceptEdits,
        },
        source: "explore".to_owned(),
        source_ref: Some("42".to_owned()),
        created_at: 1727000000000,
        updated_at: 1727000005000,
    };
    let bytes = native_model::encode(&v2).expect("encode v2");
    let (decoded, version) = native_model::decode::<SessionRecord>(bytes).expect("decode v2");
    assert_eq!(version, 2);
    assert_eq!(
        decoded.config_snapshot.agent_instance_id,
        Some(1727),
        "v2 新快照 agent_instance_id roundtrip 原样"
    );
    assert_eq!(decoded, v2, "全字段 roundtrip 一致");
}

#[test]
fn 会话记录降级半边from丢弃实例外键不panic() {
    let v2 = SessionRecord {
        id: "ses-downgrade".to_owned(),
        engine_session_id: None,
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Sdk,
            agent_instance_id: Some(9),
            model: Some("m".to_owned()),
            permission_mode: AgentPermissionMode::Default,
        },
        source: "debug".to_owned(),
        source_ref: None,
        created_at: 1,
        updated_at: 2,
    };
    // 降级半边（native_model `from` 双向 From 要求）：新增字段丢弃、缺省占位
    // ——只保升级语义真实性，降级形态不作数据承诺
    let v1: SessionRecordV1 = v2.clone().into();
    assert_eq!(v1.id, v2.id);
    assert_eq!(v1.engine_session_id, v2.engine_session_id);
    assert_eq!(v1.config_snapshot.engine, AgentEngineKind::Sdk);
    assert_eq!(v1.config_snapshot.model, Some("m".to_owned()));
    // 再升级：agent_instance_id 缺省 None（丢弃不回填）
    let back: SessionRecord = v1.into();
    assert_eq!(back.config_snapshot.agent_instance_id, None);
    assert_eq!(back.id, v2.id);
    assert_eq!(back.config_snapshot.model, v2.config_snapshot.model);
}

#[test]
fn 旧json形态缺agentInstanceId键经serde_default读none不炸() {
    // store_port 旧载荷 / 前端旧形态 JSON：缺新键经 `#[serde(default)]` 读 None
    let legacy = serde_json::json!({
        "engine": "sdk",
        "model": "m-high",
        "permissionMode": "acceptEdits"
    });
    let snapshot: SessionConfigSnapshot =
        serde_json::from_value(legacy).expect("旧三键 JSON 应可读");
    assert_eq!(snapshot.agent_instance_id, None, "缺键读 None 不炸");
    assert_eq!(snapshot.engine, AgentEngineKind::Sdk);
    assert_eq!(snapshot.model, Some("m-high".to_owned()));
    assert_eq!(snapshot.permission_mode, AgentPermissionMode::AcceptEdits);
}

#[test]
fn 实例外键serde边界_none与零与负与i64max线键camel_case不失真() {
    for id in [None, Some(0), Some(-1), Some(i64::MAX)] {
        let snapshot = SessionConfigSnapshot {
            engine: AgentEngineKind::Sdk,
            agent_instance_id: id,
            model: Some("m".to_owned()),
            permission_mode: AgentPermissionMode::Default,
        };
        let value = serde_json::to_value(&snapshot).expect("序列化应成功");
        assert!(
            value.get("agentInstanceId").is_some(),
            "线键恒在场（None 出线 null 不省略），实际: {value}"
        );
        assert_eq!(
            value["agentInstanceId"],
            serde_json::json!(id),
            "数值不失真（0 / -1 / i64::MAX）"
        );
        let back: SessionConfigSnapshot = serde_json::from_value(value).expect("反序列化应成功");
        assert_eq!(back.agent_instance_id, id, "回读原样");
    }
}

#[test]
fn v2快照bincode布局锚_实例外键紧随engine之后() {
    // DD7 定序回归钉：`agent_instance_id` 置 `engine` 之后（bincode 位置编码
    // ——布局重排即存量行解码错位）。以「engine 序号（Sdk=1）+ Some 标记 +
    // i64::MAX 小端」唯一组合窗口定位：engine 序号恰紧邻实例外键字节。
    let record = SessionRecord {
        id: "layout-anchor".to_owned(),
        engine_session_id: None,
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Sdk,
            agent_instance_id: Some(i64::MAX),
            model: None,
            permission_mode: AgentPermissionMode::Default,
        },
        source: "debug".to_owned(),
        source_ref: None,
        created_at: 0,
        updated_at: 0,
    };
    let bytes = native_model::encode(&record).expect("encode v2");
    // i64::MAX 小端 = 7×0xFF + 0x7F；窗口 = engine 序号（Sdk=1，u32 小端）
    // + Some 标记 + i64::MAX 值
    let pattern: Vec<u8> = [1_u8, 0, 0, 0, 1]
        .into_iter()
        .chain(std::iter::repeat(0xFF_u8).take(7))
        .chain([0x7F_u8])
        .collect();
    let hits: Vec<usize> = bytes
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(index, window)| (window == pattern.as_slice()).then_some(index))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "「engine 序号 1 + Some 标记 + i64::MAX」唯一落位（engine 之后紧邻居位），实际命中: {hits:?}"
    );
    // 同位反证：engine 置 Cli（序号 0）时组合窗口不再命中（engine 字节确属窗口）
    let cli_record = SessionRecord {
        config_snapshot: SessionConfigSnapshot {
            engine: AgentEngineKind::Cli,
            ..record.config_snapshot.clone()
        },
        ..record.clone()
    };
    let cli_bytes = native_model::encode(&cli_record).expect("encode Cli 布局对照");
    assert!(
        !cli_bytes
            .windows(pattern.len())
            .any(|window| window == pattern.as_slice()),
        "engine 换 Cli 后窗口不再命中（engine 字节确属紧邻组合）"
    );
}

// ---------------------------------------------------------------------------
// RunNodeRecord.step 词汇随动（worktree-cleanup-and-active-cap / AC-9 +
// check-before-flow-start）：RunStepKind 二十一值封闭集白名单式扩展 + `cleanup`
// 与 `clean`（零迁移——native_model serde 编解码随枚举词面自然可用）
// ---------------------------------------------------------------------------

/// `RunStepKind::Clean` 节点构造 / 序列化 / 反序列化往返等值（正向，
/// check-before-flow-start）：serde 线词 `"clean"` 白名单式扩展可用——native_model
/// encode/decode 回环逐字段相等、serde_json 线面 step 词逐字、`node_id()` 身份串
/// 含 clean 词面（initialize 基线恢复步落库面）。
#[test]
fn run_node_record_clean词节点往返等值() {
    let record = RunNodeRecord::new(
        "run-1727000000000",
        "chg-demo",
        7,
        &run_node_entry(RunStepKind::Clean, RunStepStatus::Passed, 0),
        1_727_000_060_000,
    );
    assert_eq!(record.step, RunStepKind::Clean, "构造直入 clean 词");

    // native_model encode/decode 回环（serde 词面承载——零迁移）
    let bytes = native_model::encode(&record).expect("encode 应成功");
    let (decoded, version) =
        native_model::decode::<RunNodeRecord>(bytes).expect("decode 应成功");
    assert_eq!(version, 1, "native_model id=16 版本封装 version 1 持衡");
    assert_eq!(decoded, record, "clean 词节点回环逐字段相等");
    assert_eq!(decoded.step, RunStepKind::Clean);

    // serde_json 线面：step 词 `clean` 逐字（与 cleanup 词面可辨）+ node_id()
    let value = serde_json::to_value(&record).expect("serde_json 出线应成功");
    assert_eq!(value["step"], serde_json::json!("clean"), "线词白名单扩展在案");
    assert_eq!(
        record.node_id(),
        "implement:1:clean:0",
        "身份串 = 边表端点口径（clean 词面在案——fixture 相位随行）"
    );

    // serde_json 反序列化回环（同词面）
    let back: RunNodeRecord = serde_json::from_value(value).expect("反序列化应成功");
    assert_eq!(back, record, "serde_json 回环逐字段相等");
}

/// `RunStepKind::Cleanup` 节点构造 / 序列化 / 反序列化往返等值（正向）：serde
/// 线词 `"cleanup"` 白名单式扩展可用——native_model encode/decode 回环逐字段
/// 相等、serde_json 线面 step 词逐字（`node_id()` 身份串含 cleanup 词面）。
#[test]
fn run_node_record_cleanup词节点往返等值() {
    let record = RunNodeRecord::new(
        "run-1727000000000",
        "chg-demo",
        7,
        &run_node_entry(RunStepKind::Cleanup, RunStepStatus::Passed, 0),
        1_727_000_060_000,
    );
    assert_eq!(record.step, RunStepKind::Cleanup, "构造直入 cleanup 词");

    // native_model encode/decode 回环（serde 词面承载——零迁移）
    let bytes = native_model::encode(&record).expect("encode 应成功");
    let (decoded, version) =
        native_model::decode::<RunNodeRecord>(bytes).expect("decode 应成功");
    assert_eq!(version, 1, "native_model id=16 版本封装 version 1 持衡");
    assert_eq!(decoded, record, "cleanup 词节点回环逐字段相等");
    assert_eq!(decoded.step, RunStepKind::Cleanup);

    // serde_json 线面：step 词 `cleanup` 逐字 + node_id() 身份串含词面
    let value = serde_json::to_value(&record).expect("serde_json 出线应成功");
    assert_eq!(value["step"], serde_json::json!("cleanup"), "线词白名单扩展在案");
    assert_eq!(
        record.node_id(),
        "implement:1:cleanup:0",
        "身份串 = 边表端点口径（cleanup 词面在案）"
    );

    // serde_json 反序列化回环（同词面）
    let back: RunNodeRecord = serde_json::from_value(value).expect("反序列化应成功");
    assert_eq!(back, record, "serde_json 回环逐字段相等");
}

/// 二十一值全词汇封闭集（边界）：既有二十值词汇断言扩为二十一值——全枚举经
/// native_model 回环逐值相等（len 与词面互异持衡；旧值 round-trip 兼容读不破
/// ——零迁移面，cleanup / clean 仅白名单式追加，行内词面扩展旧库可读）。
#[test]
fn run_node_record_step二十一值全词汇回环与词面互异() {
    let all = [
        RunStepKind::Executor,
        RunStepKind::Evaluator,
        RunStepKind::Decision,
        RunStepKind::StaticCheck,
        RunStepKind::TestExecution,
        RunStepKind::MutationTest,
        RunStepKind::Clean,
        RunStepKind::PhaseStart,
        RunStepKind::PhaseLog,
        RunStepKind::VerdictGate,
        RunStepKind::RetryGate,
        RunStepKind::WhitelistGate,
        RunStepKind::Preflight,
        RunStepKind::Commit,
        RunStepKind::Rebase,
        RunStepKind::ResolveConflicts,
        RunStepKind::Merge,
        RunStepKind::SpecSync,
        RunStepKind::Cleanup,
        RunStepKind::Seal,
        RunStepKind::Finalize,
    ];
    assert_eq!(all.len(), 21, "二十一值封闭集全枚举（20 → 21 扩位）");

    // 词面互异（封闭集防线：无两词同形）
    let mut words: Vec<&str> = all.iter().map(|kind| kind.as_str()).collect();
    let total = words.len();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), total, "二十一值词面互异: {words:?}");
    assert!(
        words.contains(&"clean") && words.contains(&"cleanup"),
        "clean 与 cleanup 两词面同集可辨"
    );

    // 全枚举 native_model 回环逐值相等（旧二十值兼容读 + clean 新值在案）
    for (sequence, kind) in all.iter().enumerate() {
        let record = RunNodeRecord::new(
            "run-1727000000000",
            "chg-demo",
            sequence as u64,
            &run_node_entry(*kind, RunStepStatus::Passed, 0),
            1_727_000_060_000,
        );
        let bytes = native_model::encode(&record).expect("encode 应成功");
        let (decoded, _) = native_model::decode::<RunNodeRecord>(bytes).expect("decode 应成功");
        assert_eq!(decoded, record, "{kind:?} 回环逐字段相等（零迁移）");
        assert_eq!(decoded.step, *kind);
    }
}

/// 越集词反序列化拒绝持衡（异常）：封闭集防线不松动——`RunStepKind` serde
/// Deserialize 面对越集词形（camel 近似 / kebab / 空串 / cleanup 变体词 / clean
/// 近似词与大写词形）一律 Err（扩词不改变拒绝面——"clean" 本词已合法入集）。
#[test]
fn run_node_record_step越集词反序列化拒绝持衡() {
    for illegal in [
        "cleanUp",
        "clean-up",
        "cleanups",
        "Cleanup",
        "clear",
        "clean step",
        "Clean",
        "mutationTest",
        "resolveConflicts",
        "archive_stage",
        "",
    ] {
        let back: Result<RunStepKind, _> =
            serde_json::from_value(serde_json::json!(illegal));
        assert!(back.is_err(), "RunStepKind 越集词 {illegal:?} 应拒绝");
    }
}

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（AC-1 / AC-5）：RunNodeRecord::new 四字段直复制
// ——旧行 serde(default) 兼容 / wire 面 / 防御值 / 写幂等 / 真件回环
// ---------------------------------------------------------------------------

/// 四字段全值节点条目 fixture（D10 词面同源定值：started 1_727_000_000_000 /
/// ended +200_000 / token 12_000 与 3_400）。
fn full_metrics_entry() -> RunNodeEntry {
    RunNodeEntry {
        started_at: 1_727_000_000_000,
        ended_at: Some(1_727_000_200_000),
        token_input: Some(12_000),
        token_output: Some(3_400),
        ..run_node_entry(RunStepKind::Executor, RunStepStatus::Passed, 0)
    }
}

/// 正向：entry 四字段全值 → record 四列逐字段等值；timestamp 参数独立赋值
///（与 started_at 不混淆——timestamp 语义保留锚）。
#[test]
fn run_node_record四字段全值直复制与timestamp独立() {
    let record = RunNodeRecord::new(
        "run-1727000000000",
        "chg-demo",
        3,
        &full_metrics_entry(),
        1_727_000_300_000,
    );
    assert_eq!(record.started_at, 1_727_000_000_000, "started_at 直复制");
    assert_eq!(record.ended_at, Some(1_727_000_200_000), "ended_at 直复制");
    assert_eq!(record.token_input, Some(12_000), "token_input 直复制");
    assert_eq!(record.token_output, Some(3_400), "token_output 直复制");
    // timestamp 独立赋值：与 started_at 语义分立（整包同刻 = finished_at）
    assert_eq!(record.timestamp, 1_727_000_300_000);
    assert_ne!(record.timestamp, record.started_at, "timestamp 与 started_at 不混淆");
}

/// 正向：native_model 编码 → decode 回环四列逐字段一致（id=16 version=1 编解码
/// 回归——加列后 codec 面）。
#[test]
fn run_node_record四列编解码回环逐字段一致() {
    let record = RunNodeRecord::new(
        "run-1727000000000",
        "chg-demo",
        0,
        &full_metrics_entry(),
        1_727_000_300_000,
    );
    let bytes = native_model::encode(&record).expect("encode 应成功");
    let (decoded, version) =
        native_model::decode::<RunNodeRecord>(bytes).expect("decode 应成功");
    assert_eq!(version, 1, "id=16 version=1 持衡");
    assert_eq!(decoded.started_at, record.started_at);
    assert_eq!(decoded.ended_at, record.ended_at);
    assert_eq!(decoded.token_input, record.token_input);
    assert_eq!(decoded.token_output, record.token_output);
    assert_eq!(decoded, record, "回环全字段一致");
}

/// 边界：entry token 两值 / ended_at None → 列 None 直透（缺席合法缺省）。
#[test]
fn run_node_record_none列直透() {
    let entry = run_node_entry(RunStepKind::StaticCheck, RunStepStatus::Passed, 0);
    assert_eq!(entry.ended_at, Some(1_727_000_060_000));
    let mut bare = entry.clone();
    bare.ended_at = None;
    bare.token_input = None;
    bare.token_output = None;
    let record = RunNodeRecord::new("run-1", "chg-demo", 0, &bare, 1_727_000_300_000);
    assert_eq!(record.ended_at, None, "ended None 直透");
    assert_eq!(record.token_input, None);
    assert_eq!(record.token_output, None);
}

/// 边界：旧 JSON 形态（无四新键、仅既有键）反序列化 → started_at=0 /
/// ended_at=None / token None，零报错零迁移（`#[serde(default)]` 锚，AC-5）。
#[test]
fn run_node_record旧json形态缺四新键解码零报错零迁移() {
    let record = RunNodeRecord::new(
        "run-legacy",
        "chg-demo",
        0,
        &run_node_entry(RunStepKind::Executor, RunStepStatus::Passed, 0),
        1_727_000_300_000,
    );
    let mut value = serde_json::to_value(&record).expect("出线应成功");
    // 剥离四新键 → 旧形态 wire 面（既有键原样保留）
    let object = value.as_object_mut().expect("对象线面");
    for key in ["startedAt", "endedAt", "tokenInput", "tokenOutput"] {
        assert!(object.remove(key).is_some(), "新键 {key} 在案可剥离");
    }
    let legacy: RunNodeRecord = serde_json::from_value(value).expect("旧形态解码应成功");
    assert_eq!(legacy.started_at, 0, "旧行 started 缺省 0 = 步级时间缺席");
    assert_eq!(legacy.ended_at, None);
    assert_eq!(legacy.token_input, None);
    assert_eq!(legacy.token_output, None);
    // 既有列零迁移原样（身份面 / 载荷面持衡）
    assert_eq!(legacy.run_id, record.run_id);
    assert_eq!(legacy.node_key, record.node_key);
    assert_eq!(legacy.step, record.step);
    assert_eq!(legacy.status, record.status);
    assert_eq!(legacy.timestamp, record.timestamp);
}

/// 边界：同 run 多行 node_seq 递增 → node_key 打包不撞键；node_id() /
/// node_seq() 身份面不因新列漂移。
#[test]
fn run_node_record同run多行node_seq递增不撞键身份面持衡() {
    let entry = full_metrics_entry();
    let keys: Vec<u128> = (0..5u64)
        .map(|seq| RunNodeRecord::new("run-same", "chg-demo", seq, &entry, 1).node_key)
        .collect();
    for pair in keys.windows(2) {
        assert!(pair[0] < pair[1], "同 run node_seq 递增 → node_key 严格增大");
    }
    let record = RunNodeRecord::new("run-same", "chg-demo", 7, &entry, 1);
    assert_eq!(record.node_seq(), 7, "node_seq() = 打包键低 64 位");
    assert_eq!(
        record.node_id(),
        "implement:1:executor:0",
        "身份串 = 边表端点口径（不因新列漂移）"
    );
}

/// 边界：序列化 wire 面：camelCase 四新键 `startedAt` / `endedAt` /
/// `tokenInput` / `tokenOutput` 在案。
#[test]
fn run_node_record_wire面camelcase四新键在案() {
    let value = serde_json::to_value(RunNodeRecord::new(
        "run-wire",
        "chg-demo",
        0,
        &full_metrics_entry(),
        1,
    ))
    .expect("出线应成功");
    assert_eq!(value["startedAt"], serde_json::json!(1_727_000_000_000_i64));
    assert_eq!(value["endedAt"], serde_json::json!(1_727_000_200_000_i64));
    assert_eq!(value["tokenInput"], serde_json::json!(12_000));
    assert_eq!(value["tokenOutput"], serde_json::json!(3_400));
    // timestamp 既有键语义保留
    assert_eq!(value["timestamp"], serde_json::json!(1));
}

/// 异常：entry 载荷防御形态值（started_at 负值 / token 接近 u64::MAX）→ 直复
/// 制不 panic 不钳制。
#[test]
fn run_node_record防御形态值直复制不钳制() {
    let mut defensive = full_metrics_entry();
    defensive.started_at = -1;
    defensive.token_input = Some(u64::MAX);
    defensive.token_output = Some(u64::MAX - 1);
    let record = RunNodeRecord::new("run-def", "chg-demo", 0, &defensive, 1);
    assert_eq!(record.started_at, -1, "负值原样（载荷面零校验零钳制）");
    assert_eq!(record.token_input, Some(u64::MAX));
    assert_eq!(record.token_output, Some(u64::MAX - 1));
}

/// 边界（写幂等锚一）：同一 entry 连续两次 `new` → 两产物 `PartialEq` 全等
///（纯构造可重复调用、零副作用累积——AC-5 加列后构造面稳定性）。
#[test]
fn run_node_record重复构造幂等两产物全等() {
    let entry = full_metrics_entry();
    let first = RunNodeRecord::new("run-idem", "chg-demo", 0, &entry, 1_727_000_300_000);
    let second = RunNodeRecord::new("run-idem", "chg-demo", 0, &entry, 1_727_000_300_000);
    assert_eq!(first, second, "连续两次 new 逐字段全等（零副作用累积）");
}

/// 边界（写幂等锚二）：native_model 编码 → 解码 → 再编码，两次编码字节形态
/// 一致（version=1 additive 加列不引入重编码漂移，重复落读不累积改写）。
#[test]
fn run_node_record编解码再编码字节形态一致() {
    let record = RunNodeRecord::new(
        "run-encode-idem",
        "chg-demo",
        2,
        &full_metrics_entry(),
        1_727_000_300_000,
    );
    let first = native_model::encode(&record).expect("首次 encode 应成功");
    let (decoded, version) =
        native_model::decode::<RunNodeRecord>(first.clone()).expect("decode 应成功");
    assert_eq!(version, 1);
    let second = native_model::encode(&decoded).expect("再 encode 应成功");
    assert_eq!(first, second, "两次编码字节形态一致（无重编码漂移）");
}

/// 组合（AC-5 写读同字段集）：真件 Store 经 finish_change_run 单事务落库后
/// `list_run_nodes` 读回四列逐字段一致（写读同字段集锚；读面半边见
/// store_test.rs 节）。
#[test]
fn run_node_record真件落库读回四列逐字段一致() {
    use workflow::state::{
        ChangeStateRecord, ChangeStatus, RunFinishCommand, RunNodeEntry as Entry,
        RunStartCommand, RunStatus,
    };

    let db_dir = tempfile::Builder::new()
        .prefix("store-test-run-node-record-roundtrip-")
        .tempdir()
        .expect("创建临时目录失败");
    let store = Store::open_workspace(&db_dir.path().join("ws.redb"))
        .unwrap_or_else(|e| panic!("open_workspace 应成功: {e}"));
    store
        .create_change_record(ChangeStateRecord {
            id: "chg-roundtrip".to_owned(),
            name: "roundtrip".to_owned(),
            title: "roundtrip".to_owned(),
            workflow_type: "requirement".to_owned(),
            created_at: 1_000,
            status: ChangeStatus::Active,
            archived_at: None,
            active_phase: None,
            worktree: None,
            base_commit: None,
        })
        .expect("建档种子应成功");
    store
        .start_change_run(&RunStartCommand {
            run_id: "run-roundtrip".to_owned(),
            change_id: "chg-roundtrip".to_owned(),
            started_at: 1_727_000_000_000,
        })
        .expect("发起应成功");

    let command = RunFinishCommand {
        run_id: "run-roundtrip".to_owned(),
        change_id: "chg-roundtrip".to_owned(),
        status: RunStatus::Completed,
        reason: None,
        finished_at: 1_727_000_300_000,
        nodes: vec![
            Entry {
                phase: "implement".to_owned(),
                attempt: 1,
                step: RunStepKind::Executor,
                instance_no: 0,
                status: RunStepStatus::Passed,
                session_id: Some("ses-1".to_owned()),
                detail: Some("实现完成".to_owned()),
                output_ref: None,
                started_at: 1_727_000_000_000,
                ended_at: Some(1_727_000_200_000),
                token_input: Some(12_000),
                token_output: Some(3_400),
            },
            Entry {
                phase: "implement".to_owned(),
                attempt: 1,
                step: RunStepKind::StaticCheck,
                instance_no: 0,
                status: RunStepStatus::Passed,
                session_id: None,
                detail: None,
                output_ref: None,
                started_at: 1_727_000_200_000,
                ended_at: None,
                token_input: None,
                token_output: None,
            },
        ],
        edges: Vec::new(),
    };
    store
        .finish_change_run(&command)
        .expect("finish_change_run 应成功");

    let rows = store
        .list_run_nodes("run-roundtrip")
        .expect("list_run_nodes 应成功");
    assert_eq!(rows.len(), 2);
    // 四列逐字段与命令载荷一致（写读同字段集——node_key 主键防重不窜行）
    assert_eq!(rows[0].started_at, 1_727_000_000_000);
    assert_eq!(rows[0].ended_at, Some(1_727_000_200_000));
    assert_eq!(rows[0].token_input, Some(12_000));
    assert_eq!(rows[0].token_output, Some(3_400));
    assert_eq!(rows[1].started_at, 1_727_000_200_000);
    assert_eq!(rows[1].ended_at, None, "None 列读回原样");
    assert_eq!(rows[1].token_input, None);
    assert_eq!(rows[1].token_output, None);
    assert_eq!(rows[0].timestamp, 1_727_000_300_000, "整包同刻读回");
}
