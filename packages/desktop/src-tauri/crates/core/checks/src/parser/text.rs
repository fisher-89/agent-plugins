use crate::model::{TestCaseResult, TestCaseStatus};

/// node-test spec 汇总行关键词（`ℹ <关键词> <数>` 形态的统计行，非用例行）。
const SPEC_SUMMARY_KEYWORDS: [&str; 8] = [
    "tests",
    "suites",
    "pass",
    "failed",
    "fail",
    "cancelled",
    "skipped",
    "todo",
];

/// 解析 node-test `--test-reporter=spec` 文本结果为用例行
pub fn parse_spec_report(text: &str) -> Result<Vec<TestCaseResult>, String> {
    let mut cases: Vec<TestCaseResult> = Vec::new();
    // 待配对的套件起始名（`▶ name` 入栈、同名 pass 标记行判为套件收口）
    let mut open_suites: Vec<String> = Vec::new();
    // 当前失败用例下标（错误块附着面）
    let mut failing: Option<usize> = None;
    let mut error_block: Vec<String> = Vec::new();

    for raw in strip_ansi(text).lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        match classify_spec_line(trimmed) {
            SpecLine::SuiteStart(name) => {
                open_suites.push(name);
                failing = None;
            }
            SpecLine::Case {
                status,
                name,
                duration_ms,
            } => {
                if status == TestCaseStatus::Passed && open_suites.iter().rev().any(|s| s == &name)
                {
                    // 同名套件收口行（spec 报告器复用 pass 标记），不产用例
                    if let Some(pos) = open_suites.iter().rposition(|s| s == &name) {
                        open_suites.remove(pos);
                    }
                    failing = None;
                    continue;
                }
                let case = TestCaseResult {
                    name,
                    file: None,
                    duration_ms,
                    status,
                    line: None,
                    error_type: None,
                    error_message: None,
                    stack_trace: None,
                };
                failing = match status {
                    TestCaseStatus::Failed => {
                        error_block.clear();
                        Some(cases.len())
                    }
                    _ => None,
                };
                cases.push(case);
            }
            SpecLine::Noise => {
                if let (Some(index), true) = (&failing, indent > 0) {
                    error_block.push(trimmed.to_owned());
                    let first = error_block.first().cloned().unwrap_or_default();
                    let (error_type, error_message) = split_error_head(&first);
                    let case = &mut cases[*index];
                    case.error_type = Some(error_type);
                    case.error_message = Some(error_message);
                    case.stack_trace = Some(error_block.join("\n"));
                }
            }
        }
    }
    Ok(cases)
}

/// spec 文本行分类（锚模式判定）。
enum SpecLine {
    /// `▶ name` 套件起始行
    SuiteStart(String),
    /// 结果行（状态 + 用例名 + 可选耗时）
    Case {
        status: TestCaseStatus,
        name: String,
        duration_ms: Option<f64>,
    },
    /// 汇总 / 诊断 / 空白等噪声行
    Noise,
}

/// 行锚分类：首个字符判标记族，`ℹ` 需排除汇总行形态。
fn classify_spec_line(line: &str) -> SpecLine {
    let mut chars = line.chars();
    let (status, rest) = match chars.next() {
        Some('✔') | Some('√') => (Some(TestCaseStatus::Passed), &line['✔'.len_utf8()..]),
        Some('✖') | Some('✗') | Some('×') => {
            (Some(TestCaseStatus::Failed), &line['✖'.len_utf8()..])
        }
        Some('﹣') => (Some(TestCaseStatus::Skipped), &line['﹣'.len_utf8()..]),
        Some('ℹ') => {
            let rest = line['ℹ'.len_utf8()..].trim();
            if is_summary_line(rest) {
                return SpecLine::Noise;
            }
            (Some(TestCaseStatus::Skipped), &line['ℹ'.len_utf8()..])
        }
        Some('-') => (Some(TestCaseStatus::Skipped), &line[1..]),
        Some('▶') => {
            let name = line['▶'.len_utf8()..].trim();
            let name = strip_duration(name).0;
            return SpecLine::SuiteStart(name.to_owned());
        }
        _ => return SpecLine::Noise,
    };
    let status = status.unwrap_or(TestCaseStatus::Passed);
    let (name, duration_ms) = strip_duration(rest.trim());
    if name.is_empty() {
        return SpecLine::Noise;
    }
    SpecLine::Case {
        status,
        name: name.to_owned(),
        duration_ms,
    }
}

/// 汇总行形态判定：`<关键词> <纯数字>`（如 "pass 3"、"duration_ms 42"）。
fn is_summary_line(rest: &str) -> bool {
    let mut parts = rest.split_whitespace();
    let Some(keyword) = parts.next() else {
        return true;
    };
    match (parts.next(), parts.next()) {
        (Some(number), None) => {
            SPEC_SUMMARY_KEYWORDS.contains(&keyword) && number.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

/// 剥离行尾耗时注记 ` (12 ms)` / `(1ms)` →（净名，耗时毫秒）。
fn strip_duration(name: &str) -> (&str, Option<f64>) {
    let trimmed = name.trim_end();
    if let Some(open) = trimmed.rfind('(') {
        let inner = trimmed[open + 1..].trim_end_matches(')').trim();
        if let Some(number) = inner.strip_suffix("ms") {
            let number = number.trim();
            if let Ok(millis) = number.parse::<f64>() {
                return (trimmed[..open].trim(), Some(millis));
            }
        }
    }
    (trimmed, None)
}

/// 错误块首行拆 `错误类型: 消息`（无冒号形态类型取 "Error"）。node-test spec 错误
/// 块与 libtest-json 失败事件共用同一拆分口径（两个报告器的 panic 面同形）。
pub(crate) fn split_error_head(first_line: &str) -> (String, String) {
    match first_line.split_once(':') {
        Some((error_type, message)) => (error_type.trim().to_owned(), message.trim().to_owned()),
        None => ("Error".to_owned(), first_line.to_owned()),
    }
}

/// 剥离 ANSI 颜色转义序列（`ESC [ ... m`；spec 报告器着色输出归一）。
fn strip_ansi(text: &str) -> String {
    if !text.contains('\x1b') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for escape in chars.by_ref() {
                if escape.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
