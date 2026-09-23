//! `ClaudeCliRunner` 与 stdout 泵的单元/集成测试（AC-2 × AC-1 接缝）：泵核心
//! [`crate::runner::pump_lines`] 以内存行流 + 可注入退出码驱动（不 spawn 真实
//! 进程，真实 spawn 端到端见 test-design 不可测试项 4），验证「CLI stdout
//! JSONL → core::agent AgentEvent 逻辑事件流」跨 crate 契约：事件线格式可被
//! core 反序列化、seq 从 0 单调、EOF 无 result 补发合成 RunResult（D6）、
//! 有界通道背压不丢事件（D1）。

use std::io::Cursor;

use tokio::io::BufReader;
use tokio::sync::mpsc;

use agent::{AgentBlock, AgentEvent, AgentEventKind, AgentRunner};

use crate::runner::{pump_lines, ClaudeCliRunner};

/// system/init 行。
const INIT_LINE: &str = r#"{"type":"system","subtype":"init","model":"claude-opus","session_id":"s-1","tools":["Bash"],"mcp_servers":[]}"#;
/// assistant 行：tool_use 块。
const ASSISTANT_TOOL_USE_LINE: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tu_1","name":"Bash","input":{"command":"ls"}}]}}"#;
/// user 行：tool_result 块。
const USER_TOOL_RESULT_LINE: &str = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","id":"tu_1","content":"ok"}]}}"#;
/// 正常收敛 result 行。
const RESULT_LINE: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":2,"duration_ms":99,"total_cost_usd":0.1,"usage":{},"session_id":"s-1"}"#;

/// 以内存行流驱动泵核心并回收全部事件（通道容量取测试所需，洪峰用例 256）。
async fn pump_all(fixture: String, capacity: usize, exit_code: Option<i32>) -> Vec<AgentEvent> {
    let (sender, mut receiver) = mpsc::channel(capacity);
    let exit_code = std::future::ready(exit_code);
    pump_lines(
        BufReader::new(Cursor::new(fixture.into_bytes())),
        exit_code,
        sender,
    )
    .await;
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    events
}

/// 洪峰 fixture：314 条 message + 1 条 result（含 result 收尾，不触发合成）。
fn flood_fixture(lines: usize) -> String {
    let mut fixture = String::new();
    for index in 0..lines - 1 {
        fixture.push_str(&format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"行{index}"}}]}}}}"#
        ));
        fixture.push('\n');
    }
    fixture.push_str(RESULT_LINE);
    fixture.push('\n');
    fixture
}

// ---------------------------------------------------------------------------
// 泵任务：完整会话与 seq 单调（单元 + 集成关系 1 正向）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 完整会话fixture逐行入泵事件按序送达且线格式可被core表达() {
    let fixture =
        format!("{INIT_LINE}\n{ASSISTANT_TOOL_USE_LINE}\n{USER_TOOL_RESULT_LINE}\n{RESULT_LINE}\n");
    let events = pump_all(fixture, 256, Some(0i32)).await;

    assert_eq!(
        events.len(),
        4,
        "四行 → 四事件，正常 result 收尾不补发（D6）"
    );

    // 线格式断言：camelCase 顶层键 + kind 判别（两 crate 线格式一致的接缝验证）
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.seq, index as u64, "seq 从 0 单调递增");
        let value = serde_json::to_value(event).expect("序列化成功");
        assert!(value.get("seq").is_some() && value.get("timestampMs").is_some());
        // 每条事件 JSON 均可经 core 信封反序列化（infra 不私造 core 不认识的变体）
        let roundtrip: AgentEvent = serde_json::from_value(value).expect("core 信封可反序列化");
        assert_eq!(roundtrip, *event);
    }

    // 事件序：init → assistant(tool_use) → user(tool_result) → result
    assert!(matches!(
        &events[0].kind,
        AgentEventKind::RunStarted { model: Some(model), .. } if model == "claude-opus"
    ));
    assert!(matches!(
        &events[1].kind,
        AgentEventKind::Message { role, blocks, .. }
            if role == "assistant"
                && matches!(&blocks[0], AgentBlock::ToolUse { id, .. } if id == "tu_1")
    ));
    assert!(matches!(
        &events[2].kind,
        AgentEventKind::Message { role, blocks, .. }
            if role == "user"
                && matches!(&blocks[0], AgentBlock::ToolResult { id, is_error, .. } if id == "tu_1" && !is_error)
    ));
    assert!(matches!(
        &events[3].kind,
        AgentEventKind::RunResult {
            is_error: false,
            num_turns: Some(2),
            ..
        }
    ));
}

#[tokio::test]
async fn seq单调性跨归一化与通道全程成立且空白行不占号() {
    // 混入空白行：归一化跳过，seq 不得出现空洞
    let fixture = format!(
        "{INIT_LINE}\n\n   \n{ASSISTANT_TOOL_USE_LINE}\n{USER_TOOL_RESULT_LINE}\n{RESULT_LINE}\n"
    );
    let events = pump_all(fixture, 256, None).await;

    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3], "空白行不占 seq，全程 0..n 单调");
}

// ---------------------------------------------------------------------------
// 泵任务：EOF 合成收敛（D6）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eof无result且携带退出码时补发合成run_result() {
    // 模拟进程异常退出（退出码 7）：EOF 无 result
    let fixture = format!("{INIT_LINE}\n{ASSISTANT_TOOL_USE_LINE}\n");
    let events = pump_all(fixture, 256, Some(7i32)).await;

    assert_eq!(events.len(), 3, "两行事件 + 一条合成收敛");
    let Some(AgentEventKind::RunResult {
        subtype,
        is_error,
        usage,
        ..
    }) = events.last().map(|event| event.kind.clone())
    else {
        panic!("事件流恒以 RunResult 终止（单一收敛机制）");
    };
    assert_eq!(subtype, "error_process_exit");
    assert!(is_error, "合成收敛为失败形态");
    assert_eq!(
        usage,
        serde_json::json!({ "exitCode": 7 }),
        "退出码记入 usage"
    );
    assert_eq!(
        events.last().expect("有事件").seq,
        2,
        "合成事件占下一个 seq"
    );
}

#[tokio::test]
async fn 空输出流即退出时仅一条合成run_result且seq为0() {
    let events = pump_all(String::new(), 256, Some(1i32)).await;

    assert_eq!(events.len(), 1, "零输入 → 单条合成收敛");
    assert_eq!(events[0].seq, 0);
    assert!(matches!(
        &events[0].kind,
        AgentEventKind::RunResult { subtype, is_error: true, .. } if subtype == "error_process_exit"
    ));
}

// ---------------------------------------------------------------------------
// 泵任务：有界通道背压与 Raw 透传
// ---------------------------------------------------------------------------

#[tokio::test]
async fn 超容量洪峰315行经背压全量送达零丢失() {
    // 通道容量 256 < 输入 315 行：生产端阻塞在 send 而非丢弃（D1 通道口径）
    let (sender, mut receiver) = mpsc::channel(256);
    let fixture = flood_fixture(315);
    let pump = tokio::spawn(pump_lines(
        BufReader::new(Cursor::new(fixture.into_bytes())),
        std::future::ready(Some(0i32)),
        sender,
    ));

    let mut received = Vec::new();
    while let Some(event) = receiver.recv().await {
        received.push(event);
    }
    pump.await.expect("泵任务正常结束");

    assert_eq!(
        received.len(),
        315,
        "315 行全量送达（含 result 收尾），零丢失"
    );
    let seqs: Vec<u64> = received.iter().map(|event| event.seq).collect();
    let expected: Vec<u64> = (0..315).collect();
    assert_eq!(seqs, expected, "seq 全程单调无跳号");
    assert!(matches!(
        received.last().expect("非空").kind,
        AgentEventKind::RunResult {
            is_error: false,
            ..
        }
    ));
}

#[tokio::test]
async fn 混入未知type与非json行时raw占seq透传与jsonl口径一致() {
    let unknown = r#"{"type":"mystery","x":1}"#;
    let not_json = "?? 不是 JSON ??";
    let fixture = format!("{INIT_LINE}\n{unknown}\n{not_json}\n\n{RESULT_LINE}\n");
    let events = pump_all(fixture, 256, None).await;

    assert_eq!(events.len(), 4, "空白行跳过，未知/非 JSON 行以 Raw 占 seq");
    assert!(matches!(
        &events[1].kind,
        AgentEventKind::Raw { event_type, raw_json }
            if event_type == "mystery" && raw_json == unknown
    ));
    assert!(matches!(
        &events[2].kind,
        AgentEventKind::Raw { event_type, raw_json }
            if event_type == "unparsable" && raw_json == not_json
    ));
    // seq 仍然连续：Raw 透传不跳号（D8）
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3]);
}

// ---------------------------------------------------------------------------
// ClaudeCliRunner：无状态构造
// ---------------------------------------------------------------------------

#[test]
fn 两次new实例互不共享状态() {
    fn assert_runner<R: AgentRunner>(_: &R) {}

    let first = ClaudeCliRunner::new();
    let second = ClaudeCliRunner::default();
    assert_runner(&first);
    assert_runner(&second);

    // 无状态：零大小单元结构，两个实例无可共享的运行态字段
    assert_eq!(std::mem::size_of_val(&first), 0);
    assert_eq!(std::mem::size_of_val(&second), 0);
}
