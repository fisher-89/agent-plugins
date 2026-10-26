//! 检查域环境归因装配测试：`attempts_log` / `fault_text` 均为纯函数（零 io /
//! 零 async），`RunReport` / `AttemptTrace` 以结构体字面量直构；归因原料经真实
//! `exec::lockprobe::attribution_text` 与 `exec::ProcessOutput` 组合（不 mock
//! ——摘要口径与归因原料正是被测语义），诊断路径取真实 `tempfile` 路径使探针零
//! 阻塞返回。

use exec::{AttemptTrace, RunOutcome, RunReport};

use super::{
    argv_budget_text, argv_degrade_note, attempts_log, fault_text, DIAGNOSIS_EXCERPT_LIMIT,
    ENV_GUIDANCE,
};

// ---------------------------------------------------------------------------
// 装置：`RunReport` / `AttemptTrace` 字面量构造
// ---------------------------------------------------------------------------

/// 单次执行报告（`attempts == 1` 且 `retried` 空 = 未重试形态）。
fn single(stdout: &[u8], stderr: &[u8], exit_code: i32) -> RunReport {
    RunReport {
        outcome: RunOutcome::Completed,
        exit_code,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        attempts: 1,
        retried: Vec::new(),
    }
}

/// 此前尝试的留痕（`transient = None` 表示该次未命中签名）。
fn trace(exit_code: i32, transient: Option<&str>, stdout: &[u8], stderr: &[u8]) -> AttemptTrace {
    AttemptTrace {
        exit_code,
        transient: transient.map(str::to_owned),
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    }
}

/// 分隔头（与实现同式的期望值构造：段头逐字锚定，含 `·` 分隔符与末尾 `=====`）。
fn header(attempt: u32, max: u32, exit_code: i32, signature: Option<&str>) -> String {
    format!(
        "===== attempt {attempt}/{max} · exit={exit_code} · transient={} =====\n",
        signature.unwrap_or("-")
    )
}

// ---------------------------------------------------------------------------
// 正向 / 边界：attempts_log 单次语义（零分隔头）
// ---------------------------------------------------------------------------

/// 正向（AC-8 / D11）：单次零分隔头——`attempts = 1`、`retried = []` → 返回恰为
/// `stdout ++ stderr`（逐字节），不含 `===== attempt` 前缀（既有 record 断言不被
/// 破）。
#[test]
fn 单次尝试零分隔头逐字节等同现状() {
    let log = attempts_log(&single(b"out\n", b"err\n", 0));
    assert_eq!(log, b"out\nerr\n", "单次尝试逐字节等同 `stdout ++ stderr`");
    assert!(
        !String::from_utf8_lossy(&log).contains("===== attempt"),
        "单次尝试不产生分隔头: {:?}",
        String::from_utf8_lossy(&log)
    );
}

/// 边界：单次双空——`attempts = 1` + 双空输出 → 返回空 `Vec`（零字节，不产生
/// 空段头）。
#[test]
fn 单次双空输出返回零字节() {
    assert!(
        attempts_log(&single(b"", b"", 0)).is_empty(),
        "双空输出零字节"
    );
}

// ---------------------------------------------------------------------------
// 正向：attempts_log 重试面
// ---------------------------------------------------------------------------

/// 正向（AC-3）：重试两段——`retried` 一条 + 末次成功 → 两段按序，`max` 恒 5，
/// 末次段头 `transient=-`。
#[test]
fn 重试两段按序且段头逐字() {
    let report = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 0,
        stdout: b"a2".to_vec(),
        stderr: b"e2".to_vec(),
        attempts: 2,
        retried: vec![trace(1, Some("os error 32"), b"a1", b"e1")],
    };

    let expected = format!(
        "{}{}{}{}",
        header(1, 5, 1, Some("os error 32")),
        "a1e1",
        header(2, 5, 0, None),
        "a2e2",
    );
    assert_eq!(
        String::from_utf8_lossy(&attempts_log(&report)),
        expected,
        "两段按尝试序，末次段头 transient=-"
    );
}

/// 正向（表驱动 5 段）：耗尽面——`retried` 4 条 + 末次同（`exit_code = 1`）→ 恰
/// 5 段，序号 1..5 递增，每段前恰一个换行；总长度 == Σ(段头 + 该次双路字节)。
#[test]
fn 耗尽面五段序号递增且总长度可核() {
    let mut retried = Vec::new();
    for _ in 0..4 {
        retried.push(trace(1, Some("os error 32"), b"xx", b"yy"));
    }
    let report = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: b"os error 32".to_vec(),
        stderr: b"".to_vec(),
        attempts: 5,
        retried,
    };

    let log = attempts_log(&report);
    let text = String::from_utf8_lossy(&log).into_owned();

    assert_eq!(
        text.matches("===== attempt ").count(),
        5,
        "瞬态耗尽恰 5 段: {text}"
    );
    let mut expected_len = 0usize;
    for attempt in 1..=5u32 {
        let expected_header = header(attempt, 5, 1, Some("os error 32"));
        assert!(
            text.contains(&expected_header),
            "第 {attempt} 段段头逐字在案: {expected_header:?} | {text}"
        );
        expected_len += expected_header.len();
    }
    // 前 4 段正文各 4 字节（"xx" + "yy"），末次正文 12 字节（"os error 32"）
    expected_len += 4 * 4 + "os error 32".len();
    assert_eq!(log.len(), expected_len, "总长度 == Σ(段头 + 该次双路字节)");

    // 序号递增（1/5 … 5/5 逐段在案）
    let mut cursor = 0usize;
    for attempt in 1..=5u32 {
        let marker = format!("===== attempt {attempt}/5");
        let found = text[cursor..]
            .find(&marker)
            .unwrap_or_else(|| panic!("第 {attempt} 段序号缺失: {text}"));
        cursor += found + marker.len();
    }
}

/// 正向（AC-8）：段内合流顺序——该段 `stdout = A`、`stderr = B` → 段内正文为
/// `AB`（stdout 前缀 + stderr 后缀，不 `2>&1`、不交错重排）。
#[test]
fn 段内正文为stdout前缀加stderr后缀() {
    let report = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 0,
        stdout: b"C".to_vec(),
        stderr: b"D".to_vec(),
        attempts: 2,
        retried: vec![trace(1, Some("os error 32"), b"A", b"B")],
    };

    let text = String::from_utf8_lossy(&attempts_log(&report)).into_owned();
    assert!(text.contains("\nAB"), "第 1 段正文为 AB: {text}");
    assert!(
        text.contains(&format!("AB{}", header(2, 5, 0, None))),
        "段间紧邻（正文后直接下一段段头）: {text}"
    );
    assert!(text.ends_with("CD"), "末次正文为 stdout + stderr: {text}");
}

/// 边界：未命中此前尝试——`retried` 中一条 `transient = None` → 该段头渲染
/// `transient=-`（不写 `None`、不省略字段）。
#[test]
fn 未命中此前尝试的段头渲染短横() {
    let report = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 0,
        stdout: b"ok".to_vec(),
        stderr: Vec::new(),
        attempts: 2,
        retried: vec![trace(1, None, b"red", b"")],
    };

    let text = String::from_utf8_lossy(&attempts_log(&report)).into_owned();
    assert!(
        text.contains(&header(1, 5, 1, None)),
        "未命中留痕渲染 transient=-: {text}"
    );
    assert!(!text.contains("None"), "不写 Rust 的 None 字样: {text}");
}

// ---------------------------------------------------------------------------
// 边界：attempts_log 字节面
// ---------------------------------------------------------------------------

/// 边界（表驱动 3 形态）：特殊字节与非 UTF-8——段头 `exit_code` 取 `-1` 哨兵 /
/// `255`；正文含非 UTF-8（`0xFF 0xFE`）、`\0`、emoji → 正文逐字节保留（本函数
/// **不做 lossy**），段头为 ASCII。
#[test]
fn 特殊字节与非utf8正文逐字节保留() {
    let raw = vec![0xFF_u8, 0xFE, 0x00, b'a'];
    let emoji = "🎉 完成".as_bytes().to_vec();
    let nul = vec![b'a', 0u8, b'b'];

    let cases: [(&str, i32, Vec<u8>); 3] = [
        ("非 UTF-8", -1, raw),
        ("emoji", 255, emoji),
        ("NUL", 1, nul),
    ];

    for (label, exit_code, body) in cases {
        let mut retried = vec![trace(1, Some("os error 32"), b"", b"")];
        // 让末次尝试携带该形态正文（`transient` 由 `is_transient` 复算：exit_code
        // 非零且无签名 → None）
        retried[0].stdout = body.clone();
        retried[0].exit_code = exit_code;
        let report = RunReport {
            outcome: RunOutcome::Completed,
            exit_code,
            stdout: body.clone(),
            stderr: Vec::new(),
            attempts: 2,
            retried,
        };

        let log = attempts_log(&report);
        assert!(
            log.windows(body.len())
                .any(|window| window == body.as_slice()),
            "{label}: 正文逐字节保留在案"
        );
        let head = header(1, 5, exit_code, Some("os error 32"));
        assert!(
            log.starts_with(head.as_bytes()),
            "{label}: 段头为 ASCII 且 exit_code 逐值渲染（{exit_code}）"
        );
    }
}

/// 边界：大输出不截断——每次尝试正文 >64 KiB → 全量拼接，段头计数与段数不变
///（审计不丢上下文）。
#[test]
fn 大输出全量拼接不截断() {
    let big = vec![b'q'; 64 * 1024 + 1];
    let report = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: big.clone(),
        stderr: big.clone(),
        attempts: 2,
        retried: vec![trace(1, Some("os error 32"), &big, &big)],
    };

    let log = attempts_log(&report);
    // 此前尝试携命中签名（`retried` 留痕），末次由 `is_transient(&report)` 复算——
    // 正文为纯 `q` 字节、无签名 → 段头 `transient=-`
    let expected_len = header(1, 5, 1, Some("os error 32")).len()
        + 2 * big.len()
        + header(2, 5, 1, None).len()
        + 2 * big.len();
    assert_eq!(log.len(), expected_len, "两段全量（各 2 × >64 KiB）不截断");
    assert_eq!(
        String::from_utf8_lossy(&log)
            .matches("===== attempt ")
            .count(),
        2,
        "段数不受输出规模影响"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 边界：fault_text 装配
// ---------------------------------------------------------------------------

/// 正向（AC-3）：重试次数与指引——`attempts = 5`、非零退出、诊断含锁文本 →
/// 含「已自动重试 4 次」+「每次重试会跳过已完成部分」+ `ENV_GUIDANCE`（含
/// 「排除 workspace 父目录」与「逐 worktree」无效说明）。
#[test]
fn 环境文案含重试次数与处置指引() {
    let text = fault_text(
        5,
        1,
        b"error: failed to rename archive file: /t/liba.rlib: os error 32",
        &[],
    );

    assert!(
        text.contains("已自动重试 4 次（每次重试会跳过已完成部分）"),
        "重试次数 = attempts - 1 且带说明: {text}"
    );
    assert!(text.contains("排除"), "含处置指引: {text}");
    assert!(text.contains("workspace 父目录"), "指引点名父目录: {text}");
    assert!(
        text.contains("逐 worktree"),
        "指引说明逐 worktree 无效: {text}"
    );
    assert!(
        text.ends_with(ENV_GUIDANCE),
        "指引单点来自 ENV_GUIDANCE 常量: {text}"
    );
}

/// 正向（AC-3）：占用者原料嵌入——诊断含真实临时路径（存在且无持有者）→ 文本
/// 含该路径（被锁文件）与「未定位占用者」降级说明（探针真实调用，空清单不阻断）。
#[test]
fn 环境文案嵌入被锁文件与未定位占用者() {
    let dir = tempfile::Builder::new()
        .prefix("checks-env-attribution-")
        .tempdir()
        .expect("创建临时目录失败");
    let path = dir.path().join("liba.rlib");
    std::fs::write(&path, b"body").expect("写临时文件失败");
    let diagnostics = format!(
        "error: failed to rename archive file: {}\nCaused by: os error 32",
        path.display()
    );

    let text = fault_text(3, 1, diagnostics.as_bytes(), &[]);

    assert!(
        text.contains(&path.to_string_lossy().into_owned()),
        "被锁文件路径逐字在案: {text}"
    );
    assert!(
        text.contains("未定位占用者"),
        "无持有者时给「未定位占用者」降级说明: {text}"
    );
    assert!(
        text.contains("已自动重试 2 次"),
        "次数按 attempts - 1 计: {text}"
    );
}

/// 正向：末次诊断摘要——多行输出（前置 `Compiling …` + 末行锁错误）→ 摘要取末次
/// 错误面（含签名行），沿用 `ProcessOutput` 口径（不自造截断）。
#[test]
fn 环境文案摘要取末次错误面() {
    let diagnostics = b"Compiling core v0.1.0\nCompiling infra v0.1.0\nerror: failed to rename archive file: /t/liba.rlib: os error 32\n";
    let text = fault_text(2, 1, diagnostics, &[]);

    assert!(text.contains("最后一次诊断："), "非空诊断产摘要段: {text}");
    assert!(
        text.contains("error: failed to rename archive file"),
        "摘要含锁错误行: {text}"
    );
    assert!(text.contains("os error 32"), "摘要含错误码行: {text}");
}

/// 边界：次数边界——`attempts = 1`（生产仅在早停面出现）→ 「已自动重试 0 次」，
/// 不 panic、无空括号 / 负值。
#[test]
fn 次数边界零重试不产负值() {
    let text = fault_text(1, 1, b"error: os error 32", &[]);

    assert!(
        text.contains("已自动重试 0 次"),
        "最小合法 attempts 渲染 0 次: {text}"
    );
    assert!(!text.contains("已自动重试 -1 次"), "不得出现负值: {text}");
    assert!(!text.contains("已自动重试 次"), "不得出现空次数: {text}");
}

/// 边界：空输出——`diagnostics` 为空 `&[]` → 摘要段降级为空（不出现空段头、不
/// panic），次数与指引照常在案。
#[test]
fn 空诊断不产摘要段但次数指引照常() {
    let text = fault_text(4, 1, &[], &[]);

    assert!(
        !text.contains("最后一次诊断："),
        "空诊断不产摘要段头: {text}"
    );
    assert!(text.contains("已自动重试 3 次"), "次数照常: {text}");
    assert!(text.ends_with(ENV_GUIDANCE), "指引照常: {text}");
    assert!(
        text.contains("被锁文件："),
        "归因段降级为说明而非空段: {text}"
    );
}

/// 边界（表驱动 3 形态）：超长与特殊字符——诊断含 emoji / 引号 / 换行 / 非 UTF-8
/// 字节 → lossy 不 panic，摘要按上限截断、指引逐字。
#[test]
fn 超长与特殊字符诊断按上限截断() {
    // 形态一：>2000 字符正文（无错误关键词 → 摘要走 `tail()`，进而被 chars 上限
    // 截断）
    let long_body = (0..60)
        .map(|_| "z".repeat(100))
        .collect::<Vec<_>>()
        .join("\n");
    let text = fault_text(5, 1, long_body.as_bytes(), &[]);
    let after = text
        .split("最后一次诊断：\n")
        .nth(1)
        .expect("超长诊断应产摘要段");
    let excerpt = after
        .split("\n环境侧处理")
        .next()
        .expect("摘要段以指引前的换行收尾");
    assert_eq!(
        excerpt.chars().count(),
        DIAGNOSIS_EXCERPT_LIMIT + 1,
        "摘要按 DIAGNOSIS_EXCERPT_LIMIT 截断并加省略号"
    );
    assert!(text.ends_with(ENV_GUIDANCE), "指引逐字不受截断影响");

    // 形态二：引号 / 换行 / emoji 混排
    let special = "say \"os error 32\"\n\u{0} emoji 🎉 完成\n".to_owned();
    let text = fault_text(2, 1, special.as_bytes(), &[]);
    assert!(
        text.contains("os error 32"),
        "特殊字符诊断的签名行在案: {text}"
    );
    assert!(text.ends_with(ENV_GUIDANCE), "指引逐字: {text}");

    // 形态三：非 UTF-8 原始字节（lossy 不 panic；替换符随摘要行承载）
    let mut raw = b"error: os error 32 ".to_vec();
    raw.extend_from_slice(&[0xFF, 0xFE]);
    raw.push(b'\n');
    let text = fault_text(2, 1, &raw, &[]);
    assert!(
        text.contains("os error 32"),
        "非 UTF-8 诊断 lossy 后可读: {text}"
    );
    assert!(
        text.contains('\u{FFFD}'),
        "非 UTF-8 字节转替换符后不 panic: {text}"
    );
}

// ---------------------------------------------------------------------------
// 边界：常量锚定
// ---------------------------------------------------------------------------

/// 边界（表驱动 2 常量）：`ENV_GUIDANCE` 含「父目录」与「逐 worktree」两个关键
/// 短语且与基线红文案零共用句（AC-6 的原料面）；诊断摘录上限常量 >0 且为可读
/// 规模。
#[test]
fn 指引与摘录上限常量锚定() {
    assert!(ENV_GUIDANCE.contains("父目录"), "指引点名父目录");
    assert!(
        ENV_GUIDANCE.contains("逐 worktree"),
        "指引说明逐 worktree 无效"
    );
    for baseline_phrase in ["请在基线上修复", "基线检查未通过", "基线"] {
        assert!(
            !ENV_GUIDANCE.contains(baseline_phrase),
            "与基线红文案零共用句（不得含 {baseline_phrase}）"
        );
    }

    // 取值经局部绑定断言（避免 `assertions_on_constants` 噪声）
    let limit = DIAGNOSIS_EXCERPT_LIMIT;
    assert!(limit > 0, "摘录上限为正（实际 {limit}）");
    assert!(limit >= 500, "摘录上限足够承载主因（实际 {limit}）");
}

// ---------------------------------------------------------------------------
// 正向 / 异常 / 边界：fault_text 的「重试进展」行（AC-5 / D6–D8）
// ---------------------------------------------------------------------------

/// N 行 `   Compiling <crate> v0.1.0`（rustc 缩进形态）的原始字节。
fn compiling_lines(count: u32) -> Vec<u8> {
    (0..count)
        .map(|index| format!("   Compiling crate{index} v0.1.0\n"))
        .collect::<String>()
        .into_bytes()
}

/// 锁错误行（含可解析路径，归因段非降级形态）。
const LOCK_LINE: &[u8] = b"error: failed to rename archive file: /t/liba.rlib: os error 32";

/// 正向（AC-5）：推进序列行——`attempts = 4`、`retried` 三条（264 / 5 / 0 行 +
/// 锁文本）、末次 `output` 仅锁文本 → 文本含
/// `重试进展：待编译 264 → 5 → 0 → 0`；行位在「已自动重试 3 次…」之后、
/// 「最后一次诊断：」之前；不含卡死判定语。
#[test]
fn 环境文案含推进序列行且行位在案() {
    let retried = vec![
        trace(1, Some("os error 32"), &compiling_lines(264), LOCK_LINE),
        trace(1, Some("os error 32"), &compiling_lines(5), LOCK_LINE),
        trace(1, Some("os error 32"), &compiling_lines(0), LOCK_LINE),
    ];

    let text = fault_text(4, 1, LOCK_LINE, &retried);

    assert!(
        text.contains("重试进展：待编译 264 → 5 → 0 → 0"),
        "四个计数按尝试序成行: {text}"
    );
    assert!(
        !text.contains("判定为卡死"),
        "递减形态不产卡死判定语: {text}"
    );

    let attempts_at = text.find("已自动重试 3 次").expect("次数行在案");
    let progress_at = text.find("重试进展：").expect("推进序列行在案");
    let diagnosis_at = text.find("最后一次诊断：").expect("诊断段在案");
    assert!(
        attempts_at < progress_at,
        "序列行在次数行之后（{attempts_at} < {progress_at}）"
    );
    assert!(
        progress_at < diagnosis_at,
        "序列行在诊断段之前（{progress_at} < {diagnosis_at}）"
    );
}

/// 正向（AC-5）：卡死判定附语——`attempts = 2`、两次输出各 50 行 `Compiling` +
/// 锁文本 → 含 `重试进展：待编译 50 → 50（末两次无进展，判定为卡死）`。
#[test]
fn 环境文案卡死判定附语() {
    let mut first = compiling_lines(50);
    first.extend_from_slice(LOCK_LINE);
    let mut last = compiling_lines(50);
    last.extend_from_slice(LOCK_LINE);
    let retried = vec![trace(1, Some("os error 32"), &first, b"")];

    let text = fault_text(2, 1, &last, &retried);

    assert!(
        text.contains("重试进展：待编译 50 → 50（末两次无进展，判定为卡死）"),
        "卡死时同一行追加判定附语: {text}"
    );
}

/// 异常（AC-5 抑制臂）：`attempts == 1` 无该行——`retried` 空、`output` 含 264 行
/// `Compiling` + 锁文本 → 无「重试进展」字样；归因段 / 次数行（「已自动重试 0
/// 次」）/ 摘要 / 指引照旧。
#[test]
fn 单次尝试不产推进序列行() {
    let mut output = compiling_lines(264);
    output.extend_from_slice(LOCK_LINE);

    let text = fault_text(1, 1, &output, &[]);

    assert!(!text.contains("重试进展"), "未重试不产序列行: {text}");
    assert!(text.contains("已自动重试 0 次"), "次数行照旧: {text}");
    assert!(text.contains("最后一次诊断："), "摘要照旧: {text}");
    assert!(text.ends_with(ENV_GUIDANCE), "指引照旧: {text}");
}

/// 异常（AC-5 抑制臂）：序列全 `0` 无该行——`attempts = 3`、`retried` 两条仅锁
/// 文本、末次仅锁文本（非 cargo 形态）→ 无「重试进展」；其余段零变。
#[test]
fn 序列全零不产推进序列行() {
    let retried = vec![
        trace(1, Some("os error 32"), b"", LOCK_LINE),
        trace(1, Some("os error 32"), b"", LOCK_LINE),
    ];

    let text = fault_text(3, 1, LOCK_LINE, &retried);

    assert!(!text.contains("重试进展"), "全 0 序列无可读进展: {text}");
    assert!(text.contains("已自动重试 2 次"), "次数行照旧: {text}");
    assert!(text.ends_with(ENV_GUIDANCE), "指引照旧: {text}");
}

/// 边界（D8）：部分归零仍输出——序列 `[264, 0]`（`attempts = 2`）→ 输出
/// `重试进展：待编译 264 → 0`（确有推进可读），且不含卡死判定语。
#[test]
fn 部分归零仍输推进序列行() {
    let retried = vec![trace(
        1,
        Some("os error 32"),
        &compiling_lines(264),
        LOCK_LINE,
    )];

    let text = fault_text(2, 1, LOCK_LINE, &retried);

    assert!(
        text.contains("重试进展：待编译 264 → 0"),
        "部分归零照常输出: {text}"
    );
    assert!(!text.contains("判定为卡死"), "末值 0 不判卡死: {text}");
}

/// 边界（AC-5 上限）：耗尽面五数字——`attempts = 5`、序列
/// `[10, 8, 3, 1, 0]` → 行含 5 个数字，不含判定语（末值 `0`）。
#[test]
fn 耗尽面推进序列五数字() {
    let retried = vec![
        trace(1, Some("os error 32"), &compiling_lines(10), LOCK_LINE),
        trace(1, Some("os error 32"), &compiling_lines(8), LOCK_LINE),
        trace(1, Some("os error 32"), &compiling_lines(3), LOCK_LINE),
        trace(1, Some("os error 32"), &compiling_lines(1), LOCK_LINE),
    ];

    let text = fault_text(5, 1, LOCK_LINE, &retried);

    assert!(
        text.contains("重试进展：待编译 10 → 8 → 3 → 1 → 0"),
        "上限面五个数字按尝试序: {text}"
    );
    assert!(!text.contains("判定为卡死"), "末值 0 不判卡死: {text}");
    let numbers = text
        .split("重试进展：待编译 ")
        .nth(1)
        .and_then(|rest| rest.split('\n').next())
        .unwrap_or_default()
        .matches(" → ")
        .count();
    assert_eq!(numbers, 4, "五个数字之间恰四个箭头");
}

/// 边界：直构违反不变量——`attempts = 3` 但 `retried = []`（直构语料）→ 视图
/// 序列长度 1 → 无「重试进展」、不 panic（抑制条件按序列而非 `attempts` 判）。
#[test]
fn 直构违反不变量不产推进序列行() {
    let mut output = compiling_lines(264);
    output.extend_from_slice(LOCK_LINE);

    let text = fault_text(3, 1, &output, &[]);

    assert!(
        !text.contains("重试进展"),
        "序列长度 1 即不输出（不按 attempts 判）: {text}"
    );
    assert!(
        text.contains("已自动重试 2 次"),
        "次数仍按 attempts: {text}"
    );
}

/// 边界（表驱动 3 形态）：诊断与序列共存字节面——`output` 含 emoji / 引号 /
/// 换行 / `\0` / 非 UTF-8（`0xFF 0xFE`）+ `Compiling` 行 → lossy 不 panic、序列
/// 行逐字在案、摘要按 `DIAGNOSIS_EXCERPT_LIMIT` 截断、`ENV_GUIDANCE` 结尾逐字。
#[test]
fn 诊断与序列共存字节面() {
    // 形态一：emoji / 引号 / NUL 混排
    let mut special = compiling_lines(2);
    special.extend_from_slice("say \"os error 32\"\n\u{0} emoji 🎉 完成\n".as_bytes());
    let text = fault_text(
        2,
        1,
        &special,
        &[trace(1, Some("os error 32"), &compiling_lines(7), b"")],
    );
    assert!(
        text.contains("重试进展：待编译 7 → 2"),
        "特殊字符不阻断序列行: {text}"
    );
    assert!(text.ends_with(ENV_GUIDANCE), "指引逐字结尾: {text}");

    // 形态二：非 UTF-8 原始字节
    let mut raw = compiling_lines(1);
    raw.extend_from_slice(b"error: os error 32 ");
    raw.extend_from_slice(&[0xFF, 0xFE]);
    raw.push(b'\n');
    let text = fault_text(
        2,
        1,
        &raw,
        &[trace(1, Some("os error 32"), &compiling_lines(3), b"")],
    );
    assert!(
        text.contains("重试进展：待编译 3 → 1"),
        "非 UTF-8 lossy 后序列行照旧: {text}"
    );
    assert!(
        text.contains('\u{FFFD}'),
        "非 UTF-8 字节转替换符后不 panic: {text}"
    );

    // 形态三：attempts = 5 + >2000 字符诊断（摘要按上限截断）
    let long_body = (0..30)
        .map(|_| "z".repeat(100))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(long_body.chars().count() > DIAGNOSIS_EXCERPT_LIMIT);
    let long = long_body.clone().into_bytes();
    let text = fault_text(
        5,
        1,
        &long,
        &[
            trace(1, Some("os error 32"), &compiling_lines(4), b""),
            trace(1, Some("os error 32"), &compiling_lines(3), b""),
            trace(1, Some("os error 32"), &compiling_lines(2), b""),
            trace(1, Some("os error 32"), &compiling_lines(1), b""),
        ],
    );
    assert!(
        text.contains("重试进展：待编译 4 → 3 → 2 → 1 → 0"),
        "五段序列在案（末值 0 不带判定语）: {text}"
    );
    let after = text
        .split("最后一次诊断：\n")
        .nth(1)
        .expect("超长诊断应产摘要段");
    let excerpt = after
        .split("\n环境侧处理")
        .next()
        .expect("摘要段以指引前的换行收尾");
    assert_eq!(
        excerpt.chars().count(),
        DIAGNOSIS_EXCERPT_LIMIT + 1,
        "摘要按上限截断并加省略号"
    );
    assert!(text.ends_with(ENV_GUIDANCE), "指引逐字不受截断影响");
}

/// 边界：`attempts` 大值防御——`attempts = u32::MAX`（直构）→
/// 「已自动重试 4294967294 次」不溢出、不 panic、无空括号。
#[test]
fn attempts大值防御不溢出() {
    let text = fault_text(u32::MAX, 1, LOCK_LINE, &[]);

    assert!(
        text.contains("已自动重试 4294967294 次"),
        "saturating 计数逐值可读: {text}"
    );
    assert!(!text.contains("已自动重试 次"), "无空次数: {text}");
    assert!(!text.contains("已自动重试 -"), "无负值: {text}");
    assert!(text.ends_with(ENV_GUIDANCE), "指引照旧: {text}");
}

// ---------------------------------------------------------------------------
// 正向 / 异常 / 边界：命令行超平台上限两臂文案（argv_budget_text /
// argv_degrade_note——纯函数零 io 零 async，四类参数字面量直传，零 mock）
// ---------------------------------------------------------------------------

/// 正向：拒绝面摘要三项依序——定性（harness 限制 / 非被测代码问题）+ 三数字
/// （`8230` / `7000` / `180`）+ 动作（收窄 `includes` / 拆分 suite），
/// `omitted_len = None` 时不出现「降级后仍超」句。
#[test]
fn argv拒绝文案_定性三数字与动作() {
    let text = argv_budget_text(7000, 8230, None, 180);

    // ① 定性：成因归 harness 而非被测仓库
    assert!(
        text.contains("命令行长度超平台上限（7000 字符）"),
        "定性段点名平台上限与预算值: {text}"
    );
    assert!(text.contains("harness"), "定性归 harness 限制: {text}");
    assert!(
        text.contains("非被测代码问题"),
        "定性声明非被测代码问题（修复不指向被测仓库）: {text}"
    );
    // ② 三数字（展开长度 / 平台预算 / 测试文件数）
    assert!(
        text.contains("展开 8230 字符 / 平台预算 7000 字符 / 180 个测试文件"),
        "三数字逐字一行: {text}"
    );
    // ③ 动作
    assert!(text.contains("收窄"), "动作含收窄: {text}");
    assert!(text.contains("includes"), "动作点名 includes: {text}");
    assert!(text.contains("拆分 suite"), "动作含拆分 suite: {text}");
    // 无降级无效句（`omitted_len = None` 臂）
    assert!(
        !text.contains("降级为框架自发现后仍为"),
        "首次裁决不带降级后长度: {text}"
    );
    // 三项依序：定性 → 数字 → 动作
    let (定性, 数字, 动作) = (
        text.find("命令行长度超平台上限").expect("定性段在案"),
        text.find("展开 8230 字符").expect("数字段在案"),
        text.find("请收窄").expect("动作段在案"),
    );
    assert!(
        定性 < 数字 && 数字 < 动作,
        "三项依序（{定性} < {数字} < {动作}）"
    );
}

/// 正向 / 边界（表驱动两态）：`omitted_len` 只在 `Some` 时追加「降级为框架
/// 自发现后仍为 N 字符」，使「降级无效」可回查两次长度；`None` 臂该句零出现。
#[test]
fn argv拒绝文案_降级后长度句两态() {
    let with_omitted = argv_budget_text(7000, 8230, Some(9000), 180);
    assert!(
        with_omitted.contains("降级为框架自发现后仍为 9000 字符"),
        "Some 臂追加降级后长度（AC-5 两数字可回查）: {with_omitted}"
    );
    assert!(
        with_omitted.contains("展开 8230 字符"),
        "Some 臂同时保留原始长度（两次长度并存）: {with_omitted}"
    );
    assert!(
        with_omitted.contains("收窄") && with_omitted.contains("拆分 suite"),
        "Some 臂动作建议照常: {with_omitted}"
    );

    let without_omitted = argv_budget_text(7000, 8230, None, 180);
    assert!(
        !without_omitted.contains("降级为框架自发现"),
        "None 臂零降级措辞: {without_omitted}"
    );
    assert_ne!(
        with_omitted, without_omitted,
        "两态文案不同串（`omitted_len` 真实参与装配）"
    );
}

/// 异常（负向断言逐词）：拒绝面文案 MUST NOT 误引安全软件排除指引 / 占用者 /
/// 被锁文件 / 重试次数措辞（成因是命令长度，误引会让用户去改安全软件配置）。
#[test]
fn argv拒绝文案_零误引安全软件与重试措辞() {
    for (label, text) in [
        ("None 臂", argv_budget_text(7000, 8230, None, 180)),
        ("Some 臂", argv_budget_text(7000, 8230, Some(9000), 180)),
    ] {
        for forbidden in [
            "火绒",                  // 安全软件具名（ENV_GUIDANCE 专属）
            "云枢",                  // 安全软件具名
            "排除 workspace 父目录", // ENV_GUIDANCE 全句零误引
            "已自动重试",            // 瞬态重试措辞
            "占用",                  // 占用者归因
            "被锁文件",              // 文件锁归因
        ] {
            assert!(
                !text.contains(forbidden),
                "{label} 不得含 {forbidden}（不得套用其他成因文案）: {text}"
            );
        }
        assert!(
            !text.contains(ENV_GUIDANCE),
            "{label} 不得整句引用 ENV_GUIDANCE: {text}"
        );
    }
}

/// 边界（表驱动）：三数字边界组合——全零、`omitted_len == Some(0)`、三参同时
/// `usize::MAX` → 数值逐字呈现（与 `format!` 同式逐字相等）、不 panic、不回落
/// 省略（无「收敛」「min」类回落措辞）。
#[test]
fn argv拒绝文案_数字边界组合() {
    // 形态一：全零（预算 0 / 展开 0 / 文件 0）
    let zero = argv_budget_text(0, 0, Some(0), 0);
    assert_eq!(
        zero,
        "命令行长度超平台上限（0 字符）：属 harness 限制，非被测代码问题\n\
         展开 0 字符 / 平台预算 0 字符 / 0 个测试文件\n\
         降级为框架自发现后仍为 0 字符\n\
         请收窄该 suite 的 `includes` 或拆分 suite",
        "全零逐字呈现（零值不省略、不回落）"
    );

    // 形态二：三参同时 usize::MAX（溢出面防御：拼接不 panic）
    let max = argv_budget_text(usize::MAX, usize::MAX, Some(usize::MAX), usize::MAX);
    assert!(
        max.contains(&format!("（{} 字符）", usize::MAX)),
        "预算取 MAX 逐字: {max}"
    );
    assert!(
        max.contains(&format!("展开 {} 字符", usize::MAX)),
        "展开长度取 MAX 逐字: {max}"
    );
    assert!(
        max.contains(&format!("{} 个测试文件", usize::MAX)),
        "文件数取 MAX 逐字: {max}"
    );
    assert!(
        max.contains(&format!("仍为 {} 字符", usize::MAX)),
        "降级后长度取 MAX 逐字: {max}"
    );

    // 形态三：单侧边界（预算 0 / 展开非零）
    let skewed = argv_budget_text(0, 8230, None, 180);
    assert!(
        skewed.contains("平台上限（0 字符）")
            && skewed.contains("展开 8230 字符 / 平台预算 0 字符"),
        "单侧零值不影响另一侧逐字呈现: {skewed}"
    );

    // 不回落省略：数值全部逐字（无 `…` / 无「约」）
    for text in [&zero, &max, &skewed] {
        assert!(
            !text.contains('…') && !text.contains("约"),
            "数值面零省略措辞: {text}"
        );
        assert!(
            text.contains("请收窄"),
            "各形态动作段恒在（不因边界值消失）: {text}"
        );
    }
}

/// 正向：降级面注——三数字 + 「已降级为框架自发现（K 个测试文件）」逐字，
/// 与 `PlanExecution::argv_note` 的消费者（记录头 `[argv]` 行）同源。
#[test]
fn argv降级注_三数字与自发现陈述() {
    let note = argv_degrade_note(7000, 8230, 180);

    assert_eq!(
        note, "展开 8230 字符超平台预算 7000 字符，已降级为框架自发现（180 个测试文件）",
        "降级注逐字（三数字 + 自发现陈述）"
    );
    assert!(note.contains("展开 8230 字符"), "原始长度在案: {note}");
    assert!(note.contains("平台预算 7000"), "平台预算在案: {note}");
    assert!(
        note.contains("已降级为框架自发现"),
        "陈述降级已发生: {note}"
    );
    assert!(note.contains("180 个测试文件"), "文件数在案: {note}");
}

/// 异常（负向断言）：降级面注**零动作建议**（降级已发生，只需可查）——不含
/// 「收窄」「拆分 suite」，且不误引安全软件 / 占用者 / 重试措辞。
#[test]
fn argv降级注_零动作建议() {
    let note = argv_degrade_note(7000, 8230, 180);

    for forbidden in [
        "收窄",
        "拆分 suite",
        "请",
        "火绒",
        "云枢",
        "排除 workspace 父目录",
        "占用",
        "被锁文件",
        "已自动重试",
    ] {
        assert!(
            !note.contains(forbidden),
            "降级注不得含 {forbidden}（零动作建议 / 零误引）: {note}"
        );
    }
    assert!(
        !note.contains(ENV_GUIDANCE),
        "降级注不得整句引用 ENV_GUIDANCE: {note}"
    );
}

/// 边界（表驱动）：降级注数字边界——`files = 0` / `budget = 0` /
/// `expanded_len = 0` / `usize::MAX` 组合 → 逐字呈现、零 panic（与 `format!`
/// 同式逐字相等，防截断 / 防回落）。
#[test]
fn argv降级注_数字边界组合() {
    let cases: [(usize, usize, usize); 4] = [
        (7000, 8230, 0),
        (0, 8230, 180),
        (7000, 0, 180),
        (usize::MAX, usize::MAX, usize::MAX),
    ];

    for (budget, expanded_len, files) in cases {
        let note = argv_degrade_note(budget, expanded_len, files);
        assert_eq!(
            note,
            format!(
                "展开 {expanded_len} 字符超平台预算 {budget} 字符，已降级为框架自发现（{files} 个测试文件）"
            ),
            "({budget}, {expanded_len}, {files}) 逐字呈现（零截断 / 零回落）"
        );
        assert!(!note.contains('…'), "零省略号截断: {note}");
    }
}

/// 边界（措辞分工）：两臂互不含对方专属措辞——拒绝面不含「已降级为框架
/// 自发现」，降级面不含「请收窄」（两成因面文案不得互相套用）。
#[test]
fn argv两臂措辞分工_互不含对方专属() {
    let reject = argv_budget_text(7000, 8230, Some(9000), 180);
    let degrade = argv_degrade_note(7000, 8230, 180);

    assert!(
        !reject.contains("已降级为框架自发现（"),
        "拒绝面不含降级面专属陈述（只含「降级后仍为」诊断句）: {reject}"
    );
    assert!(
        !degrade.contains("请收窄"),
        "降级面不含拒绝面专属动作建议: {degrade}"
    );
    assert!(
        reject.contains("请收窄") && degrade.contains("已降级为框架自发现"),
        "各自专属措辞只在各自臂（分工清楚）"
    );
}
