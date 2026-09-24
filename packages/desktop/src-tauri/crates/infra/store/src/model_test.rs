//! `model` 的单元测试：`AgentEventRecord` 键打包 + 嵌装往返（AC-4）。
//!
//! 纯类型层：全部用例为内存构造，无 IO 无进程边界，无 mock。编解码经
//! native_model 封装（serde_json codec）内存往返，只验证 store 包装层与 core
//! flatten 类型的组装兼容，不重复验证 serde 自身语义；落库往返由
//! `store_test.rs` 经 `Store` 公共 API 承载。既有手写 `encode` / `decode`
//! 用例随模型层平移 `#[native_model]` + `#[native_db]` 已废弃。

use agent::{AgentBlock, AgentEvent, AgentEventKind};

use crate::model::{pack_event_key, AgentEventRecord};

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
            role: "assistant".to_owned(),
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

fn run_result(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::RunResult {
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
fn roundtrip(record: &AgentEventRecord) -> AgentEventRecord {
    let bytes = native_model::encode(record).expect("native_model encode 应成功");
    let (decoded, version) =
        native_model::decode::<AgentEventRecord>(bytes).expect("native_model decode 应成功");
    assert_eq!(version, 1, "native_model 版本封装为 version 1");
    decoded
}

// ---------------------------------------------------------------------------
// 键打包：同 run 保序、run 区间上移、极值不溢出、相邻 run 不串键
// ---------------------------------------------------------------------------

#[test]
fn 键打包同run内seq增大则event_key严格增大() {
    let keys: Vec<u128> = (0..5u64)
        .map(|seq| AgentEventRecord::new(7, raw(seq)).event_key)
        .collect();

    for pair in keys.windows(2) {
        assert!(
            pair[0] < pair[1],
            "同 run 内 seq 增大则 event_key 严格增大: {keys:?}"
        );
    }
}

#[test]
fn 键打包run_id增大则整段键区间上移() {
    // run 1 的全部键（seq 遍历全域小样本）必须整体小于 run 2 的最小键：
    // 扫描自然序即「先 run 1 全部、后 run 2」的重放序前提
    let run1_max = AgentEventRecord::new(1, raw(u64::MAX)).event_key;
    let run2_min = AgentEventRecord::new(2, raw(0)).event_key;

    assert!(
        run1_max < run2_min,
        "run_id 高 64 位隔离：run 1 最大键 < run 2 最小键"
    );
}

#[test]
fn 键打包最小键与最大键不溢出不回绕() {
    let min = pack_event_key(0, 0);
    let max = pack_event_key(i64::MAX, u64::MAX);

    assert_eq!(min, 0, "最小键 (run_id=0, seq=0) 打包为 0");
    assert_eq!(
        max,
        ((i64::MAX as u128) << 64) | (u64::MAX as u128),
        "最大键 (run_id=i64::MAX, seq=u64::MAX) 打包不溢出"
    );
    assert!(min < max);
    // 打包往返一致（组装点唯一口径）
    assert_eq!(pack_event_key(42, 7), ((42i64 as u128) << 64) | 7u128);
}

#[test]
fn 键打包相邻run边界不串键高64位隔离() {
    // 同一 seq 区间内，相邻 run 的键区间必须无缝且不重叠：
    // run 1 尾键（seq=u64::MAX）紧邻 run 2 首键（seq=0）
    let run1_tail = AgentEventRecord::new(1, raw(u64::MAX)).event_key;
    let run2_head = AgentEventRecord::new(2, raw(0)).event_key;

    assert_eq!(
        run2_head - run1_tail,
        1,
        "相邻 run 边界键恰好相邻不重叠（高 64 位隔离）"
    );
}

#[test]
fn 记录构造event_key打包自run_id与seq且访问器与载荷同源() {
    let record = AgentEventRecord::new(9, message(4));

    assert_eq!(record.event_key, pack_event_key(9, 4), "event_key 打包口径");
    assert_eq!(record.run_id(), 9, "run_id 访问器");
    assert_eq!(record.seq(), 4, "seq 访问器与载荷 event.seq 同源");
}

// ---------------------------------------------------------------------------
// 嵌装往返：五变体逐字段保真（native_model serde_json codec 封装）
// ---------------------------------------------------------------------------

#[test]
fn 嵌装往返五变体各构造一条逐字段相等() {
    let run_id = 3;
    let seeded = [
        run_started(0),
        message(1),
        system_notice(2),
        run_result(3),
        raw(4),
    ];

    for original in seeded {
        let record = AgentEventRecord::new(run_id, original.clone());
        let decoded = roundtrip(&record);

        assert_eq!(decoded, record, "变体 {:?} 往返逐字段相等", original.seq);
        assert_eq!(
            decoded.event, original,
            "嵌装载荷与原事件逐字段相等（store 包装层与 core flatten 组装兼容）"
        );
    }
}

#[test]
fn 嵌装往返raw变体中文emoji引号原文保真() {
    let original = event(
        5,
        AgentEventKind::Raw {
            event_type: "外星事件".to_owned(),
            raw_json: "{\"kind\":\"外星事件\",\"note\":\"引号\\\"与emoji🚀\"}".to_owned(),
        },
    );
    let record = AgentEventRecord::new(11, original.clone());

    let decoded = roundtrip(&record);

    assert_eq!(decoded, record);
    assert!(
        matches!(
            &decoded.event.kind,
            AgentEventKind::Raw { event_type, raw_json }
                if event_type == "外星事件"
                    && raw_json == "{\"kind\":\"外星事件\",\"note\":\"引号\\\"与emoji🚀\"}"
        ),
        "Raw 变体原文（中文/emoji/引号）嵌装往返保真"
    );
}

#[test]
fn 嵌装往返大seq与大run_id打包键经编解码不回绕() {
    // u128 打包键超 u64 上界：经 serde 十六进制字符串 serde 化往返后仍还原一致
    let record = AgentEventRecord::new(i64::MAX, raw(u64::MAX));

    let decoded = roundtrip(&record);

    assert_eq!(decoded.event_key, record.event_key, "最大打包键往返一致");
    assert_eq!(decoded.run_id(), i64::MAX);
    assert_eq!(decoded.seq(), u64::MAX);
}

#[test]
fn 嵌装往返serde线格式event_key为十六进制字符串() {
    // serde_json 无 u128 数字面：event_key 定制为十六进制字符串（信封 API 的
    // JSON 可表达性前提）；此处断言 serde 层序列化形态（native_model 封装头
    // 之外的载荷编码即此形态）
    let record = AgentEventRecord::new(1, raw(2));
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");

    assert_eq!(
        value["eventKey"],
        serde_json::json!(format!("{:#034x}", record.event_key)),
        "eventKey 以十六进制字符串呈现（合法 JSON、无二进制）"
    );
    assert_eq!(value["runId"], serde_json::json!(1));
    assert_eq!(value["event"]["seq"], serde_json::json!(2));
}
