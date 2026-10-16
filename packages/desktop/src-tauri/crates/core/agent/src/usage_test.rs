//! `usage.rs`（token 求和单源）的单元测试（test-design「usage.rs ->
//! usage_test.rs」节）：`sum_usage_tokens` 转录 `TurnDone.usage` 鸭子类型键
//! 求和——多事件遍历 / 单事件 / 空转录 / 缺键 / 混合事件（any-then-some）/
//! u64 饱和 / 超大转录 / 类型漂移 / 干扰事件 / 两键对照。
//!
//! Mock策略（test-design 本节 Mock 表）：零 mock——纯函数直测，`AgentEvent`
//! 字面量直构（`AgentEvent::stamp(seq, AgentEventKind::TurnDone { usage, .. })`，
//! 非 TurnDone 变体同式构造）。
//!
//! AC 锚注记：本函数自 infra/store 上移（desktop-step-running-snapshot D3 /
//! AC-10）——求和语义与上移前逐行一致；store 侧消费半边（SessionStats）由
//! infra/store 的 store_test.rs 节承载（依赖方向 infra → core，本 crate 不
//! 反向引用 store）。

use serde_json::{Value, json};

use crate::event::{AgentBlock, AgentDelta, AgentEvent, AgentEventKind, AgentMessageRole};
use crate::usage::sum_usage_tokens;

/// TurnDone 事件 fixture（usage 显式注入；其余统计字段中性值——时间戳由
/// `stamp` 取当前时钟，不参与等值断言）。
fn turn_done(seq: u64, usage: Value) -> AgentEvent {
    AgentEvent::stamp(
        seq,
        AgentEventKind::TurnDone {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(1),
            duration_ms: Some(1_000),
            cost_usd: Some(0.01),
            usage,
            session_id: None,
        },
    )
}

/// 多事件携指定 key → 逐值之和（`input_tokens` / `output_tokens` 两键各测一组）。
#[test]
fn 多事件携key逐值求和_两键各一组() {
    let input_events = vec![
        turn_done(0, json!({ "input_tokens": 100 })),
        turn_done(1, json!({ "input_tokens": 250 })),
        turn_done(2, json!({ "input_tokens": 47 })),
    ];
    assert_eq!(
        sum_usage_tokens(&input_events, "input_tokens"),
        Some(397),
        "input_tokens 多事件求和"
    );

    let output_events = vec![
        turn_done(0, json!({ "output_tokens": 5 })),
        turn_done(1, json!({ "output_tokens": 1_200 })),
    ];
    assert_eq!(
        sum_usage_tokens(&output_events, "output_tokens"),
        Some(1_205),
        "output_tokens 多事件求和"
    );
}

/// 单事件携 key → Some(该值)。
#[test]
fn 单事件携key返回该值() {
    let events = vec![turn_done(0, json!({ "input_tokens": 12_345 }))];
    assert_eq!(sum_usage_tokens(&events, "input_tokens"), Some(12_345));
}

/// 空转录 `&[]` → None（零事件边界）。
#[test]
fn 空转录none() {
    assert_eq!(sum_usage_tokens(&[], "input_tokens"), None);
    assert_eq!(sum_usage_tokens(&[], "output_tokens"), None);
}

/// usage 在场但 key 缺席（`{}` / `Value::Null`）→ None。
#[test]
fn usage在场但key缺席none_空对象与null() {
    let events = vec![turn_done(0, json!({})), turn_done(1, Value::Null)];
    assert_eq!(sum_usage_tokens(&events, "input_tokens"), None);
    assert_eq!(sum_usage_tokens(&events, "output_tokens"), None);
    // 未知键同名缺席
    let events = vec![turn_done(0, json!({ "input_tokens": 7 }))];
    assert_eq!(sum_usage_tokens(&events, "cache_read_tokens"), None);
}

/// 部分事件携 key、部分缺席 → Some(在场值之和)（any-then-some 语义锚）。
#[test]
fn 部分事件携keyany_then_some语义() {
    let events = vec![
        turn_done(0, json!({ "input_tokens": 100 })),
        turn_done(1, json!({})),
        turn_done(2, json!({ "input_tokens": 30 })),
        turn_done(3, Value::Null),
    ];
    assert_eq!(sum_usage_tokens(&events, "input_tokens"), Some(130));
}

/// 求和溢出 u64 → saturating_add 饱和不 panic。
#[test]
fn 求和溢出饱和不panic() {
    // u64::MAX 邻近两值相加 → 恒等饱和于 u64::MAX
    let events = vec![
        turn_done(0, json!({ "input_tokens": u64::MAX })),
        turn_done(1, json!({ "input_tokens": u64::MAX })),
    ];
    assert_eq!(sum_usage_tokens(&events, "input_tokens"), Some(u64::MAX));
    // 邻近值 + 小值 → 部分求和后饱和（不回绕）
    let events = vec![
        turn_done(0, json!({ "input_tokens": u64::MAX - 1 })),
        turn_done(1, json!({ "input_tokens": 10 })),
    ];
    assert_eq!(sum_usage_tokens(&events, "input_tokens"), Some(u64::MAX));
}

/// 超大转录（数千事件）→ 求和结果与逐值 fold 一致（遍历不截断）。
#[test]
fn 超大转录求和与逐值fold一致() {
    let events: Vec<AgentEvent> = (0..5_000u64)
        .map(|seq| turn_done(seq, json!({ "input_tokens": 3u64, "output_tokens": 2u64 })))
        .collect();
    let manual = |key: &str| -> u64 {
        events
            .iter()
            .filter_map(|event| match &event.kind {
                AgentEventKind::TurnDone { usage, .. } => {
                    usage.get(key).and_then(Value::as_u64)
                }
                _ => None,
            })
            .fold(0u64, u64::saturating_add)
    };
    assert_eq!(
        sum_usage_tokens(&events, "input_tokens"),
        Some(manual("input_tokens")),
        "input_tokens 超大转录遍历不截断"
    );
    assert_eq!(sum_usage_tokens(&events, "input_tokens"), Some(15_000));
    assert_eq!(sum_usage_tokens(&events, "output_tokens"), Some(10_000));
}

/// key 对应值非 u64（字符串 / 浮点 / 负数 / null）→ 该事件不计入、不 panic
/// （`as_u64` 判定锚）；全漂移 → None。
#[test]
fn 值类型漂移该事件不计入不panic() {
    let events = vec![
        turn_done(0, json!({ "input_tokens": "1200" })),
        turn_done(1, json!({ "input_tokens": 12.5 })),
        turn_done(2, json!({ "input_tokens": -7 })),
        turn_done(3, json!({ "input_tokens": null })),
        turn_done(4, json!({ "input_tokens": 42 })),
    ];
    assert_eq!(sum_usage_tokens(&events, "input_tokens"), Some(42));

    let all_drift = vec![
        turn_done(0, json!({ "input_tokens": "1200" })),
        turn_done(1, json!({ "input_tokens": 12.5 })),
    ];
    assert_eq!(sum_usage_tokens(&all_drift, "input_tokens"), None);
}

/// 非 TurnDone 事件（RunStarted / Message / MessageDelta / SystemNotice / Raw）
/// 混入 → 全部跳过，仅 TurnDone 参与求和。
#[test]
fn 非turndone事件全部跳过仅turndone参与求和() {
    let noise = vec![
        AgentEvent::stamp(
            0,
            AgentEventKind::RunStarted {
                model: Some("claude-opus".to_owned()),
                session_id: None,
                tools: Vec::new(),
                mcp_servers: Vec::new(),
            },
        ),
        AgentEvent::stamp(
            1,
            AgentEventKind::MessageDelta {
                parent_tool_use_id: None,
                delta: AgentDelta::Text {
                    text: "增量".to_owned(),
                },
            },
        ),
        AgentEvent::stamp(
            2,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![AgentBlock::Text {
                    text: "正文".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            3,
            AgentEventKind::SystemNotice {
                subtype: "api_retry".to_owned(),
                payload: json!({ "attempt": 2 }),
            },
        ),
        AgentEvent::stamp(
            4,
            AgentEventKind::Raw {
                event_type: "mystery".to_owned(),
                raw_json: "{}".to_owned(),
            },
        ),
    ];
    assert_eq!(sum_usage_tokens(&noise, "input_tokens"), None);

    let mut mixed = noise.clone();
    mixed.push(turn_done(5, json!({ "input_tokens": 33 })));
    mixed.push(turn_done(6, json!({ "input_tokens": 9 })));
    assert_eq!(sum_usage_tokens(&mixed, "input_tokens"), Some(42));
}

/// 组合（AC-10 两键对照）：同一转录分别求 in / out 两键，与手工折叠同口径
/// ——两消费方同源锚（store 会话统计 `SessionStats.input/output_tokens` 同
/// 实现消费；store 侧半边见 infra/store store_test.rs 节）。
#[test]
fn 上移同源锚_同一转录两键分别求和与手工折叠一致() {
    let events = vec![
        turn_done(0, json!({ "input_tokens": 1_000, "output_tokens": 120 })),
        turn_done(1, json!({ "input_tokens": 2_300 })),
        turn_done(2, json!({ "output_tokens": 340 })),
        turn_done(3, json!({})),
    ];
    let manual = |key: &str| -> Option<u64> {
        events.iter().fold(None, |acc, event| match &event.kind {
            AgentEventKind::TurnDone { usage, .. } => {
                match usage.get(key).and_then(Value::as_u64) {
                    Some(value) => Some(acc.unwrap_or(0).saturating_add(value)),
                    None => acc,
                }
            }
            _ => acc,
        })
    };
    assert_eq!(
        sum_usage_tokens(&events, "input_tokens"),
        manual("input_tokens"),
        "in 键与手工折叠同口径"
    );
    assert_eq!(sum_usage_tokens(&events, "input_tokens"), Some(3_300));
    assert_eq!(
        sum_usage_tokens(&events, "output_tokens"),
        manual("output_tokens"),
        "out 键与手工折叠同口径"
    );
    assert_eq!(sum_usage_tokens(&events, "output_tokens"), Some(460));
}
