use serde_json::Value;

use crate::model::{TestCaseResult, TestCaseStatus};

use super::text::split_error_head;

/// 解析 cargo-nextest `--message-format libtest-json` 事件流为用例行
pub fn parse_libtest_json(text: &str) -> Result<Vec<TestCaseResult>, String> {
    let mut cases: Vec<TestCaseResult> = Vec::new();
    let mut event_lines = 0usize;
    let mut broken_lines = 0usize;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            broken_lines += 1;
            continue;
        };
        event_lines += 1;
        if event.get("type").and_then(Value::as_str) != Some("test") {
            continue;
        }
        let Some(name) = event.get("name").and_then(Value::as_str) else {
            continue;
        };
        let status = match event.get("event").and_then(Value::as_str) {
            Some("ok") => TestCaseStatus::Passed,
            Some("failed") => TestCaseStatus::Failed,
            Some("ignored") => TestCaseStatus::Skipped,
            // `started` 与未知 event 不产用例
            _ => continue,
        };
        let duration_ms = event
            .get("exec_time")
            .and_then(Value::as_f64)
            .map(|seconds| seconds * 1000.0);
        let (error_type, error_message, stack_trace) =
            match event.get("stdout").and_then(Value::as_str) {
                Some(stdout) if status == TestCaseStatus::Failed => failure_fields(stdout),
                _ => (None, None, None),
            };
        let case = TestCaseResult {
            name: name.to_owned(),
            file: None,
            duration_ms,
            status,
            line: None,
            error_type,
            error_message,
            stack_trace,
        };
        match cases.iter_mut().find(|existing| existing.name == name) {
            Some(slot) => *slot = case,
            None => cases.push(case),
        }
    }

    if event_lines == 0 {
        return Err(format!(
            "libtest-json 结果工件不含任何事件行（{broken_lines} 行不可解析）"
        ));
    }
    Ok(cases)
}

/// 失败用例错误面：`（错误类型, 消息, 堆栈）`。
///
/// libtest-json 的 `stdout` 形如
/// `\nthread 'it_fails' (123) panicked at tests/it.rs:11:16:\nassertion …failed: boom\n …`
/// ——首行非空行为 panic 定位行，跳过它再按 `错误类型: 消息` 拆分；无 panic 头
/// （超时 / `ExecFail` 等无输出形态）时退化为「首行即信号」，全空则仅留堆栈空位。
fn failure_fields(stdout: &str) -> (Option<String>, Option<String>, Option<String>) {
    let trimmed = stdout.trim();
    let first = signal_line(trimmed);
    let (error_type, error_message) = match first {
        Some(line) => {
            let (error_type, error_message) = split_error_head(line);
            (Some(error_type), Some(error_message))
        }
        None => (None, None),
    };
    let stack_trace = (!trimmed.is_empty()).then(|| trimmed.to_owned());
    (error_type, error_message, stack_trace)
}

/// panic 信号行：`stdout` 块里 panic 定位行（`thread '…' panicked at …:`）之后的
/// 首个非空行才是错误信号——用例自身的 `println!` 输出排在它前面，不能当信号。
/// 无 panic 头（`ExecFail` / 超时等无输出形态）时退化为首个非空行。
fn signal_line(block: &str) -> Option<&str> {
    let lines: Vec<&str> = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let first = *lines.first()?;
    match lines.iter().position(|line| is_panic_header(line)) {
        Some(index) => lines.get(index + 1).copied().or(Some(first)),
        None => Some(first),
    }
}

/// panic 定位行判定（libtest 与 nextest 同形：`thread '<name>' (<tid>) panicked at
/// <loc>:`；老版本无 tid 段）。
fn is_panic_header(line: &str) -> bool {
    line.starts_with("thread '") && line.contains("panicked at")
}
