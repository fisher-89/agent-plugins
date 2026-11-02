use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::model::{TestCaseResult, TestCaseStatus};

use super::text::split_error_head;

/// 解析 cargo-nextest `profile.<name>.junit` 产出的 JUnit XML 为用例行
pub fn parse_junit(text: &str) -> Result<Vec<TestCaseResult>, String> {
    let mut reader = Reader::from_str(text);
    let mut buf = Vec::new();
    let mut cases: Vec<TestCaseResult> = Vec::new();
    let mut saw_root = false;
    let mut declared_tests: Option<u64> = None;

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|error| format!("JUnit 结果工件解析失败: {error}"))?;
        match event {
            Event::Start(start) => {
                let name = start.name();
                match name.as_ref() {
                    "testsuites" => {
                        saw_root = true;
                        declared_tests = attr(&start, "tests").and_then(|value| value.parse().ok());
                    }
                    "testcase" => cases.push(read_test_case(&mut reader, &start, false)?),
                    _ => {}
                }
            }
            // 自闭合形态（`<testsuites …/>` 零用例、`<testcase …/>` 无子元素）走
            // `Empty` 而非 `Start`
            Event::Empty(start) => match start.name().as_ref() {
                "testsuites" => {
                    saw_root = true;
                    declared_tests = attr(&start, "tests").and_then(|value| value.parse().ok());
                }
                "testcase" => cases.push(read_test_case(&mut reader, &start, true)?),
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if !saw_root && cases.is_empty() {
        return Err("JUnit 结果工件不含任何 testsuites / testcase 元素".to_owned());
    }
    if let Some(declared) = declared_tests {
        if declared != cases.len() as u64 {
            return Err(format!(
                "JUnit 结果工件计数不符（tests=\"{declared}\"，实际用例 {} 条）——工件疑似截断",
                cases.len()
            ));
        }
    }
    Ok(cases)
}

/// 读一条 `<testcase>` 到其结束标签
fn read_test_case(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    self_closing: bool,
) -> Result<TestCaseResult, String> {
    let raw_name = attr(start, "name").unwrap_or_default();
    let classname = attr(start, "classname").unwrap_or_default();
    let name = if classname.is_empty() {
        raw_name
    } else {
        format!("{classname}${raw_name}")
    };
    let time_seconds = attr(start, "time").and_then(|value| value.parse::<f64>().ok());

    let mut skipped = false;
    let mut failure: Option<Failure> = None;

    if !self_closing {
        let mut buf = Vec::new();
        let mut discard = Vec::new();
        loop {
            let event = reader
                .read_event_into(&mut buf)
                .map_err(|error| format!("JUnit 结果工件解析失败: {error}"))?;
            match event {
                Event::Start(element) => {
                    // 元素名先取自有副本——后续 `read_to_end_into` 会复用 `buf`
                    let tag = element.name().as_ref().to_owned();
                    match tag.as_str() {
                        "skipped" => skipped = true,
                        "failure" | "error" => failure = Some(read_failure(reader, &element)?),
                        // 其余子元素（system-out / system-err / flakyFailure 等）不
                        // 参与用例结论，整体跳过其子树
                        _ => {}
                    }
                    if tag != "failure" && tag != "error" {
                        discard.clear();
                        reader
                            .read_to_end_into(element.name(), &mut discard)
                            .map_err(|error| format!("JUnit 结果工件解析失败: {error}"))?;
                    }
                }
                // 自闭合子元素：`<skipped/>` 与 `<failure type="…"/>`（退出码形态）
                Event::Empty(element) => match element.name().as_ref() {
                    "skipped" => skipped = true,
                    "failure" | "error" => {
                        failure = Some(Failure {
                            body: None,
                            message: attr(&element, "message"),
                            kind: attr(&element, "type"),
                        })
                    }
                    _ => {}
                },
                Event::End(element) if element.name().as_ref() == "testcase" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
    }

    let status = if failure.is_some() {
        TestCaseStatus::Failed
    } else if skipped {
        TestCaseStatus::Skipped
    } else {
        TestCaseStatus::Passed
    };
    let duration_ms = (status != TestCaseStatus::Skipped)
        .then_some(time_seconds.map(|seconds| seconds * 1000.0))
        .flatten();
    let (error_type, error_message, stack_trace) = match &failure {
        Some(failure) => failure.fields(),
        None => (None, None, None),
    };

    Ok(TestCaseResult {
        name,
        file: None,
        duration_ms,
        status,
        line: None,
        error_type,
        error_message,
        stack_trace,
    })
}

/// `<failure>` / `<error>` 的原始面：正文（已解实体）+ 两个属性。
struct Failure {
    body: Option<String>,
    message: Option<String>,
    kind: Option<String>,
}

impl Failure {
    /// `（错误类型, 消息, 堆栈）`——正文形如 libtest-json 的失败 `stdout` 块时走
    /// 同一拆分口径；空正文形态（进程退出码类）退化为属性面。
    fn fields(&self) -> (Option<String>, Option<String>, Option<String>) {
        match self
            .body
            .as_deref()
            .map(str::trim)
            .filter(|body| !body.is_empty())
        {
            Some(block) => {
                let first = signal_line(block);
                let (error_type, error_message) = match first {
                    Some(line) => {
                        let (error_type, error_message) = split_error_head(line);
                        (Some(error_type), Some(error_message))
                    }
                    None => (None, None),
                };
                (error_type, error_message, Some(block.to_owned()))
            }
            None => (self.kind.clone(), self.message.clone(), None),
        }
    }
}

/// 读 `<failure>` / `<error>` 的正文到结束标签（`read_text` 只拼正文、不做实体
/// 解码，故再走一次 `escape::unescape`）。
fn read_failure(reader: &mut Reader<&[u8]>, start: &BytesStart<'_>) -> Result<Failure, String> {
    let message = attr(start, "message");
    let kind = attr(start, "type");
    let raw = reader
        .read_text(start.name())
        .map_err(|error| format!("JUnit 结果工件解析失败: {error}"))?;
    let body = quick_xml::escape::unescape(&raw)
        .map_err(|error| format!("JUnit 结果工件解析失败: 实体解码失败 {error}"))?
        .into_owned();
    Ok(Failure {
        body: (!body.trim().is_empty()).then_some(body),
        message,
        kind,
    })
}

/// 元素属性取值（实体已解码）。
fn attr(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .filter_map(|attribute| attribute.ok())
        .find(|attribute| attribute.key.as_ref() == key)
        .and_then(|attribute| {
            attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.into_owned())
        })
}

/// panic 信号行：正文块里 panic 定位行（`thread '…' panicked at …:`）之后的
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
