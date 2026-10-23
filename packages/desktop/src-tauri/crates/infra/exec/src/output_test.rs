//! `ProcessOutput` 单元测试：错误行组提取（前 10 组 / 续行 / 遇空止）、
//! tail 行口径三态、`failure_message` 拼装与空面省略。

use super::ProcessOutput;

#[test]
fn 错误信息_多行错误续行携带与遇空行止() {
    let output = ProcessOutput::new(
        1,
        "running...\nError: Unable to parse foo.json. No parser registered for .json!\n    at parse (x.js:1:1)\n    at async run (y.js:2:2)\n    extra frame line\n\nlog line after blank\n"
            .into(),
    );
    // 组 = 错误行 + 3 续行（空行不入组、截到续行上限）；后续无错误行
    assert_eq!(
        output.error_excerpt(),
        "Error: Unable to parse foo.json. No parser registered for .json!\n    at parse (x.js:1:1)\n    at async run (y.js:2:2)\n    extra frame line"
    );
}

#[test]
fn 错误信息_多组以省略号分隔且超十组截断() {
    let mut text = String::new();
    for index in 0..12 {
        text.push_str(&format!(
            "noise {index}\nerror: boom {index}\ndetail a\ndetail b\ndetail c\ndetail d\n"
        ));
    }
    let output = ProcessOutput::new(1, text.into_bytes());
    let excerpt = output.error_excerpt();
    // 前 10 组、每组 1+3 行（第 4 续行 detail d 不入组）；组间 `…` 行；尾截断 `…`
    assert!(
        excerpt.starts_with("error: boom 0\ndetail a\ndetail b\ndetail c"),
        "首组形态: {excerpt}"
    );
    assert!(
        excerpt.contains("\n…\nerror: boom 1\n"),
        "组间省略号分隔: {excerpt}"
    );
    assert!(!excerpt.contains("boom 10"), "第 11 组起截断: {excerpt}");
    assert!(excerpt.ends_with("\n…"), "截断尾标记: {excerpt}");
    // 已被前组消费的 detail 行不再开新组（非重叠）
    assert_eq!(
        excerpt.matches("detail a").count(),
        10,
        "恰 10 组各携 detail a"
    );
}

#[test]
fn 错误信息_大小写不敏感与无命中空串() {
    let hit = ProcessOutput::new(1, b"PANIC at thread\nnpm ERR! code ELIFECYCLE\nException in thread\nTraceback (most recent call last)\nthrow new Err();\nFAILED src/x.test.ts\nwarning: just a log".to_vec());
    let excerpt = hit.error_excerpt();
    assert!(excerpt.contains("PANIC"), "大写命中: {excerpt}");
    assert!(excerpt.contains("npm ERR!"), "npm 命中: {excerpt}");
    assert!(excerpt.contains("Exception"), "Exception 命中: {excerpt}");
    assert!(excerpt.contains("Traceback"), "Traceback 命中: {excerpt}");
    assert!(excerpt.contains("throw"), "throw 命中: {excerpt}");
    assert!(excerpt.contains("FAILED"), "FAILED 命中: {excerpt}");
    let miss = ProcessOutput::new(1, b"info: started\nok 1 test\nall green\n".to_vec());
    assert_eq!(miss.error_excerpt(), "", "无命中空串");
    let empty = ProcessOutput::new(0, Vec::new());
    assert_eq!(empty.error_excerpt(), "", "空输出空串");
}

#[test]
fn 末尾行_行口径三态与crlf正文化() {
    // 不足全给（CRLF 剥 \r）
    let short = ProcessOutput::new(1, b"a\r\nb\r\n".to_vec());
    assert_eq!(short.tail(), "a\nb");
    // 恰 50 行：无省略号
    let exact: String = (0..50).map(|index| format!("l{index}\n")).collect();
    let exact = ProcessOutput::new(1, exact.into_bytes());
    let tail = exact.tail();
    assert_eq!(tail.lines().count(), 50, "恰 50 行原样: {tail}");
    assert!(!tail.starts_with('…'), "无截断前缀");
    assert!(tail.starts_with("l0\n"), "首行在案");
    assert!(tail.ends_with("l49"), "末行在案");
    // 51 行：`…` 前缀行 + 末 50 行
    let over: String = (0..51).map(|index| format!("l{index}\n")).collect();
    let over = ProcessOutput::new(1, over.into_bytes());
    let tail = over.tail();
    assert!(
        tail.starts_with("…\nl1\n"),
        "省略号前缀 + l1 起（l0 截掉）: {tail}"
    );
    assert_eq!(tail.lines().count(), 51, "省略号行 + 50 行");
    // 空输出
    assert_eq!(ProcessOutput::new(1, Vec::new()).tail(), "");
    // 纯空白
    assert_eq!(ProcessOutput::new(1, b"  \n \n".to_vec()).tail(), "");
}

#[test]
fn 失败消息_空面省略与双段拼装() {
    // 空输出 → 逐字等于 head（空面无冒号缀语义）
    let empty = ProcessOutput::new(3, Vec::new());
    assert_eq!(
        empty.failure_message("工具失败（退出码 3）"),
        "工具失败（退出码 3）"
    );
    // 有错误行 + 尾部：双段（head 前置、错误信息在前）
    let output = ProcessOutput::new(7, b"before\nerror: could not compile\n".to_vec());
    assert_eq!(
        output.failure_message("测试命令非零退出（7）"),
        "测试命令非零退出（7）\n错误信息：\nerror: could not compile\n输出末尾（50 行）：\nbefore\nerror: could not compile"
    );
    // 无错误模式命中：仅尾段
    let no_error = ProcessOutput::new(7, b"plain output line\n".to_vec());
    assert_eq!(
        no_error.failure_message("测试命令非零退出（7）"),
        "测试命令非零退出（7）\n输出末尾（50 行）：\nplain output line"
    );
}
