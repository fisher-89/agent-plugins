//! `ClaudeCliRunner` 与 stdout 泵的单元/集成测试（AC-2 × AC-1 接缝）：泵核心
//! [`crate::runner::pump_lines`] 以内存行流 + 可注入退出码驱动（不 spawn 真实
//! 进程，真实 spawn 端到端见 test-design 不可测试项 4），验证「CLI stdout
//! JSONL → core::agent AgentEvent 逻辑事件流」跨 crate 契约：事件线格式可被
//! core 反序列化、seq 从 0 单调、EOF 无 result 补发合成 RunResult（D6）、
//! 有界通道背压不丢事件（D1）。

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use agent::{AgentBlock, AgentEvent, AgentEventKind, AgentRunner, RunHandle};

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
/// 停止信号缝取默认句柄（未请求）、击杀缝取空闭包（本文件不覆盖停止路径）。
async fn pump_all(fixture: String, capacity: usize, exit_code: Option<i32>) -> Vec<AgentEvent> {
    let (sender, mut receiver) = mpsc::channel(capacity);
    let exit_code = std::future::ready(exit_code);
    pump_lines(
        BufReader::new(Cursor::new(fixture.into_bytes())),
        exit_code,
        sender,
        &RunHandle::default(),
        || {},
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
    // 通道容量 256 < 输入 315 行：生产端阻塞在 send 而非丢弃（D1 通道口径）。
    // 泵以 &RunHandle 借用驱动（pump_lines 缝签名），故 spawn 的是接收端排空
    // 任务（持有 receiver 满足 'static），泵本体留在测试线程上并发推进。
    let (sender, mut receiver) = mpsc::channel(256);
    let fixture = flood_fixture(315);
    let handle = RunHandle::default();
    let drain = tokio::spawn(async move {
        let mut received = Vec::new();
        while let Some(event) = receiver.recv().await {
            received.push(event);
        }
        received
    });

    pump_lines(
        BufReader::new(Cursor::new(fixture.into_bytes())),
        std::future::ready(Some(0i32)),
        sender,
        &handle,
        || {},
    )
    .await;
    let received = drain.await.expect("排空任务正常结束");

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
    let second = ClaudeCliRunner;
    assert_runner(&first);
    assert_runner(&second);

    // 无状态：零大小单元结构，两个实例无可共享的运行态字段
    assert_eq!(std::mem::size_of_val(&first), 0);
    assert_eq!(std::mem::size_of_val(&second), 0);
}

// ---------------------------------------------------------------------------
// 泵任务：停止信号分支（select 行流 vs 停止信号——停止链「RunHandle 信号 →
// CLI 租户泵收敛」的真实泵落点；编排队列的停止全链见 exec/agent_test.rs）。
// 停止路径以 duplex 内存行流 + 捕获型击杀缝驱动（不触真实进程），EOF 时序
// 由测试持有的行流写端单边决定，无投递竞速。
// ---------------------------------------------------------------------------

/// 停止路径驱动装置：泵在独立任务上运行，测试持有行流写端与停止句柄。
struct StopRig {
    pump: tokio::task::JoinHandle<()>,
    receiver: mpsc::Receiver<AgentEvent>,
    handle: RunHandle,
    /// 击杀缝调用记录（pid 入参按调用序收集）。
    kill_calls: Arc<Mutex<Vec<Option<u32>>>>,
    /// 行流写端（持有以维持流打开；drop 即 EOF）。
    writer: tokio::io::DuplexStream,
}

fn stop_rig(pid: Option<u32>) -> StopRig {
    let (writer, reader) = tokio::io::duplex(64);
    let (sender, receiver) = mpsc::channel(16);
    let handle = RunHandle::default();
    let kill_calls = Arc::new(Mutex::new(Vec::new()));
    let spy = Arc::clone(&kill_calls);
    let pump_handle = handle.clone();
    let pump = tokio::spawn(async move {
        pump_lines(
            BufReader::new(reader),
            std::future::ready(None),
            sender,
            &pump_handle,
            move || {
                spy.lock().expect("击杀记录锁不可中毒").push(pid);
            },
        )
        .await;
    });
    StopRig {
        pump,
        receiver,
        handle,
        kill_calls,
        writer,
    }
}

/// 写入一行 JSONL 并刷新（读端存活前提下的受控写入）。
async fn write_line(writer: &mut tokio::io::DuplexStream, line: &str) {
    writer
        .write_all(format!("{line}\n").as_bytes())
        .await
        .expect("写入行流失败");
    writer.flush().await.expect("刷新行流失败");
}

async fn drain_events(receiver: &mut mpsc::Receiver<AgentEvent>) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    events
}

#[tokio::test]
async fn 停止信号中止泵且击杀缝以捕获pid被调恰一次且剩余行不再产出() {
    let mut rig = stop_rig(Some(4242));
    write_line(&mut rig.writer, INIT_LINE).await;
    let first = rig.receiver.recv().await.expect("行事件先于停止信号产出");
    assert!(
        matches!(first.kind, AgentEventKind::RunStarted { .. }),
        "停止前已写入的行按序产出"
    );

    // 停止信号单边决定收敛：此时无行可读，select 必命中停止分支
    rig.handle.request_stop();
    rig.pump.await.expect("泵任务正常结束（停止分支命中）");

    // 泵已退出：行流剩余行不再产出（读端已关闭，尽力写入被忽略）
    let _ = rig
        .writer
        .write_all(ASSISTANT_TOOL_USE_LINE.as_bytes())
        .await;

    assert!(
        drain_events(&mut rig.receiver).await.is_empty(),
        "停止中止后剩余行不再产出"
    );
    assert_eq!(
        *rig.kill_calls.lock().expect("击杀记录锁不可中毒"),
        vec![Some(4242)],
        "击杀缝恰被调一次且携带捕获的 pid"
    );
}

#[tokio::test]
async fn 停止路径不合成run_result已产出事件原样保留() {
    let mut rig = stop_rig(Some(7));
    write_line(&mut rig.writer, INIT_LINE).await;
    let first = rig.receiver.recv().await.expect("已产出事件先到达");

    rig.handle.request_stop();
    rig.pump.await.expect("泵任务正常结束");

    let drained = drain_events(&mut rig.receiver).await;
    // 已产出事件原样保留（不因停止清除），且不补发 error_process_exit 合成
    // 收敛——给编排侧 stopped 显式收敛让路（真实泵停止分支的唯一自动化落点）
    assert!(matches!(first.kind, AgentEventKind::RunStarted { .. }));
    assert!(
        drained.is_empty(),
        "停止路径不合成 RunResult，事件流以已产出事件收尾"
    );
}

#[tokio::test]
async fn 无pid时停止路径仍收敛且击杀缝以none被调不panic() {
    // spawn 未达（pid 为 None）的尽力语义：停止路径照常收敛、击杀缝照常被调
    let mut rig = stop_rig(None);
    write_line(&mut rig.writer, INIT_LINE).await;
    let _first = rig.receiver.recv().await.expect("已产出事件先到达");

    rig.handle.request_stop();
    rig.pump.await.expect("停止路径收敛不 panic");

    assert_eq!(
        *rig.kill_calls.lock().expect("击杀记录锁不可中毒"),
        vec![None],
        "无 pid 时击杀缝以 None 被调恰一次"
    );
}

#[tokio::test]
async fn 信号晚于eof时泵已按eof语义收敛无二次合成且击杀缝不被调() {
    let mut rig = stop_rig(Some(9));
    write_line(&mut rig.writer, INIT_LINE).await;
    let first = rig.receiver.recv().await.expect("行事件产出");
    assert!(matches!(first.kind, AgentEventKind::RunStarted { .. }));

    // 行流先 EOF（stop 未请求）：既有 EOF 合成 result 语义不回归
    drop(rig.writer);
    rig.pump.await.expect("泵按 EOF 语义正常收敛");

    let mut events = vec![first];
    while let Some(event) = rig.receiver.recv().await {
        events.push(event);
    }
    assert_eq!(events.len(), 2, "行事件 + 一条合成收敛");
    let last = events.last().expect("非空");
    assert!(matches!(
        last.kind,
        AgentEventKind::RunResult {
            ref subtype,
            is_error: true,
            ..
        } if subtype == "error_process_exit"
    ));

    // stop 请求在泵退出后才置位：无二次合成、击杀缝不被调（泵已不在）
    rig.handle.request_stop();
    assert!(
        drain_events(&mut rig.receiver).await.is_empty(),
        "泵已退出：停止信号不产生二次收敛"
    );
    assert!(
        rig.kill_calls
            .lock()
            .expect("击杀记录锁不可中毒")
            .is_empty(),
        "EOF 收敛路径不触达击杀缝"
    );
}
