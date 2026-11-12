//! 脚本化 SSE provider 桩（本机回环 HTTP 应答面，仅测试用）。
//!
//! 为什么需要它：`AskPort` 的「句柄是否真的抵达 `Flow` 档执行体」与「执行体两拍
//! 是否真的广播」这两条判据，只有在模型**真的调用** `AskUserQuestion` 时才可观测。
//! discard 端点（恒拒连）只能证装配不 panic——`compose` / `worker` 两组用例因此
//! 曾停在「端口在场、零调用」的装配面，判据空转。本桩按请求序回放 SSE 作为
//! **网络边界**的替身：首请求回一次 `AskUserQuestion` 调用，其后恒回收口流。

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 回环端点上的脚本化 provider（`Drop` 即关停接收线程）。
pub(crate) struct AskScriptedProvider {
    /// 供 provider 记录落库的端点（`http://127.0.0.1:<port>/v1`）。
    pub(crate) base_url: String,
    shutdown: Arc<AtomicBool>,
}

impl AskScriptedProvider {
    /// 起桩：首个请求回 `AskUserQuestion`（工具名 + 完整参数单块），其后恒回
    /// 文本收口流。
    pub(crate) fn new(tag: &str, tool_call_id: &str, question: &str, options: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定回环端口失败");
        let port = listener.local_addr().expect("取本地地址失败").port();
        listener.set_nonblocking(true).expect("置非阻塞失败");
        let ask_stream = sse_ask_body(tool_call_id, question, options);
        let stop_stream = sse_stop_body("已按你的选择继续");
        let shutdown = Arc::new(AtomicBool::new(false));
        let state_shutdown = Arc::clone(&shutdown);
        std::thread::Builder::new()
            .name(format!("ask-scripted-{tag}"))
            .spawn(move || {
                let mut served = 0usize;
                while !state_shutdown.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let body = if served == 0 {
                                ask_stream.clone()
                            } else {
                                stop_stream.clone()
                            };
                            served += 1;
                            serve_one(stream, &body);
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
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

/// 单连接应答（读请求头 + body → 回一段 SSE 后关闭）。
fn serve_one(stream: std::net::TcpStream, sse_body: &str) {
    stream.set_nonblocking(false).expect("置阻塞模式失败");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("置读超时失败");
    let mut reader = BufReader::new(stream.try_clone().expect("克隆连接失败"));
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse::<usize>().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 && std::io::Read::read_exact(&mut reader, &mut body).is_err() {
        return;
    }
    let mut stream = stream;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        sse_body.len(),
        sse_body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// 一条 SSE 数据行（OpenAI chat-completions 兼容形态）。
fn sse_data(payload: &serde_json::Value) -> String {
    format!("data: {payload}\n\n")
}

/// 「调 `AskUserQuestion`」的一整段流（工具名 + 完整参数单块）。
fn sse_ask_body(tool_call_id: &str, question: &str, options: &[&str]) -> String {
    let mut body = String::new();
    body.push_str(&sse_data(&serde_json::json!({
        "id": "chatcmpl-ask",
        "object": "chat.completion.chunk",
        "choices": [{
            "index": 0,
            "delta": {
                "tool_calls": [{
                    "index": 0,
                    "id": tool_call_id,
                    "type": "function",
                    "function": {
                        "name": "AskUserQuestion",
                        "arguments": serde_json::json!({ "question": question, "options": options }).to_string(),
                    },
                }],
            },
        }],
    })));
    body.push_str(&sse_data(&serde_json::json!({
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
    body.push_str(&sse_data(&serde_json::json!({
        "id": "chatcmpl-stop",
        "object": "chat.completion.chunk",
        "choices": [{"index": 0, "delta": {"content": text}}],
    })));
    body.push_str(&sse_data(&serde_json::json!({
        "id": "chatcmpl-stop",
        "object": "chat.completion.chunk",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
    })));
    body.push_str("data: [DONE]\n\n");
    body
}
