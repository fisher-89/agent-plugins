//! 检查域环境归因装配测试：`attempts_log` / `fault_text` 均为纯函数（零 io /
//! 零 async），`RunReport` / `AttemptTrace` 以结构体字面量直构；归因原料经真实
//! `exec::lockprobe::attribution_text` 与 `exec::ProcessOutput` 组合（不 mock
//! ——摘要口径与归因原料正是被测语义），诊断路径取真实 `tempfile` 路径使探针零
//! 阻塞返回。

use exec::{AttemptTrace, RunOutcome, RunReport};

use super::{attempts_log, fault_text, DIAGNOSIS_EXCERPT_LIMIT, ENV_GUIDANCE};

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
    assert!(attempts_log(&single(b"", b"", 0)).is_empty(), "双空输出零字节");
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
            log.windows(body.len()).any(|window| window == body.as_slice()),
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
        String::from_utf8_lossy(&log).matches("===== attempt ").count(),
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
    );

    assert!(
        text.contains("已自动重试 4 次（每次重试会跳过已完成部分）"),
        "重试次数 = attempts - 1 且带说明: {text}"
    );
    assert!(text.contains("排除"), "含处置指引: {text}");
    assert!(text.contains("workspace 父目录"), "指引点名父目录: {text}");
    assert!(text.contains("逐 worktree"), "指引说明逐 worktree 无效: {text}");
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

    let text = fault_text(3, 1, diagnostics.as_bytes());

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
    let text = fault_text(2, 1, diagnostics);

    assert!(
        text.contains("最后一次诊断："),
        "非空诊断产摘要段: {text}"
    );
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
    let text = fault_text(1, 1, b"error: os error 32");

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
    let text = fault_text(4, 1, &[]);

    assert!(
        !text.contains("最后一次诊断："),
        "空诊断不产摘要段头: {text}"
    );
    assert!(text.contains("已自动重试 3 次"), "次数照常: {text}");
    assert!(text.ends_with(ENV_GUIDANCE), "指引照常: {text}");
    assert!(text.contains("被锁文件："), "归因段降级为说明而非空段: {text}");
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
    let text = fault_text(5, 1, long_body.as_bytes());
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
    let text = fault_text(2, 1, special.as_bytes());
    assert!(text.contains("os error 32"), "特殊字符诊断的签名行在案: {text}");
    assert!(text.ends_with(ENV_GUIDANCE), "指引逐字: {text}");

    // 形态三：非 UTF-8 原始字节（lossy 不 panic；替换符随摘要行承载）
    let mut raw = b"error: os error 32 ".to_vec();
    raw.extend_from_slice(&[0xFF, 0xFE]);
    raw.push(b'\n');
    let text = fault_text(2, 1, &raw);
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
    assert!(ENV_GUIDANCE.contains("逐 worktree"), "指引说明逐 worktree 无效");
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
