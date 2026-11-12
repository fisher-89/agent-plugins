// 命令测试共享装置：脚本化 SSE provider 桩（本机回环 HTTP 应答面）。
//
// 为什么需要它：`ask` 的两条端到端链（通用侧 `agent_answer`、流程侧
// `change_flow_answer`）最深的一环是「模型真的调用 `AskUserQuestion` → 执行体
// 阻塞 → 命令投递 → 同 turn 回灌」。discard 端点（恒拒连）只能证装配不 panic，
// 证不了这一环；故以本机回环 HTTP 桩按请求序回放 SSE，作为**网络边界**的替身。

pub(crate) use std::sync::Arc;

pub(crate) struct AskScriptedProvider {
    pub(crate) base_url: String,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}

impl AskScriptedProvider {
    /// 起桩：首个请求回 `AskUserQuestion` 调用，其后恒回收口流。
    pub(crate) fn new(tag: &str, question: &str, options: &[&str]) -> Self {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定回环端口失败");
        let port = listener.local_addr().expect("取本地地址失败").port();
        listener.set_nonblocking(true).expect("置非阻塞失败");
        let ask_stream = sse_ask_body(question, options);
        let stop_stream = sse_stop_body("已按你的选择继续");
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let state_shutdown = Arc::clone(&shutdown);
        std::thread::Builder::new()
            .name(format!("exec-cmd-scripted-{tag}"))
            .spawn(move || {
                let mut served = 0usize;
                while !state_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).expect("置阻塞失败");
                            stream
                                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                                .expect("置读超时失败");
                            let mut reader =
                                BufReader::new(stream.try_clone().expect("克隆连接失败"));
                            let mut content_length = 0usize;
                            let mut aborted = false;
                            loop {
                                let mut line = String::new();
                                if !matches!(reader.read_line(&mut line), Ok(n) if n > 0) {
                                    aborted = true;
                                    break;
                                }
                                if line == "\r\n" || line == "\n" {
                                    break;
                                }
                                let lower = line.to_ascii_lowercase();
                                if let Some(rest) = lower.strip_prefix("content-length:") {
                                    content_length = rest.trim().parse::<usize>().unwrap_or(0);
                                }
                            }
                            if aborted {
                                continue;
                            }
                            let mut body = vec![0u8; content_length];
                            if content_length > 0
                                && std::io::Read::read_exact(&mut reader, &mut body).is_err()
                            {
                                continue;
                            }
                            let sse = if served == 0 {
                                ask_stream.clone()
                            } else {
                                stop_stream.clone()
                            };
                            served += 1;
                            let mut stream = stream;
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                sse.len(),
                                sse
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                || error.kind() == std::io::ErrorKind::Interrupted =>
                        {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            })
            .expect("起桩线程失败");
        Self {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            shutdown,
        }
    }
}

impl Drop for AskScriptedProvider {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 一条 SSE 数据行。
fn sse_line(payload: serde_json::Value) -> String {
    format!("data: {payload}\n\n")
}

/// 「调 `AskUserQuestion`」的一整段流（工具名 + 完整参数单块）。
fn sse_ask_body(question: &str, options: &[&str]) -> String {
    let mut body = String::new();
    body.push_str(&sse_line(serde_json::json!({
        "id": "chatcmpl-ask",
        "object": "chat.completion.chunk",
        "choices": [{
            "index": 0,
            "delta": {
                "tool_calls": [{
                    "index": 0,
                    "id": "tu_cmd_ask",
                    "type": "function",
                    "function": {
                        "name": "AskUserQuestion",
                        "arguments": serde_json::json!({ "question": question, "options": options }).to_string(),
                    },
                }],
            },
        }],
    })));
    body.push_str(&sse_line(serde_json::json!({
        "id": "chatcmpl-ask",
        "object": "chat.completion.chunk",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}],
    })));
    body.push_str("data: [DONE]\n\n");
    body
}

/// 「直接停」的一整段流（模型不再调工具）。
fn sse_stop_body(text: &str) -> String {
    let mut body = String::new();
    body.push_str(&sse_line(serde_json::json!({
        "id": "chatcmpl-stop",
        "object": "chat.completion.chunk",
        "choices": [{"index": 0, "delta": {"content": text}}],
    })));
    body.push_str(&sse_line(serde_json::json!({
        "id": "chatcmpl-stop",
        "object": "chat.completion.chunk",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
    })));
    body.push_str("data: [DONE]\n\n");
    body
}

/// 轮询 `agent_answer` 至受理（登记完成即 `Ok`；未登记时稳定同文案）。
///
/// 异步等待：命令面测试跑在 current-thread 运行时上，同步阻塞睡会卡住后台泵
/// 任务，登记永远不会出现。
pub(crate) async fn answer_until_registered(
    app: &tauri::App<tauri::test::MockRuntime>,
    root: &str,
    session_id: &str,
    text: &str,
) {
    use tauri::Manager;

    for _ in 0..3000 {
        match crate::commands::exec::agent_answer(
            app.state::<std::sync::Arc<agent::AskRegistry>>(),
            root.to_owned(),
            session_id.to_owned(),
            text.to_owned(),
        ) {
            Ok(()) => return,
            Err(error) => {
                assert_eq!(error, "当前无等待中的 ask", "仅登记未完成时重试: {error}");
                tokio::task::yield_now().await;
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        }
    }
    panic!("等待 ask 登记超时（session={session_id}）");
}

/// 转录内某 `tool_use` 的配对 `tool_result`（应答回灌判据）。
pub(crate) fn tool_result_in(
    transcript: &[agent::AgentEvent],
    tool_call_id: &str,
) -> Option<(String, bool)> {
    transcript.iter().find_map(|event| match &event.kind {
        agent::AgentEventKind::Message { blocks, .. } => {
            blocks.iter().find_map(|block| match block {
                agent::AgentBlock::ToolResult {
                    id,
                    content,
                    is_error,
                } if id == tool_call_id => Some((content.clone(), *is_error)),
                _ => None,
            })
        }
        _ => None,
    })
}
