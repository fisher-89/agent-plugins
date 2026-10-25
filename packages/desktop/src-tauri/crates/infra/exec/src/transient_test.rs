//! 瞬态签名与有限重试策略测试：纯 std 判定（零 IO / 零 spawn / 零时钟），
//! `RunReport` / `AttemptTrace` / `RetryPolicy` 全部以结构体字面量直构。
//!
//! 覆盖面：13 条封闭签名表的逐条命中与大小写不敏感、两路合并匹配、多行噪声、
//! 诊断源码摘录行排除、三合一判据的三条反例（超时 / 取消 / 绿 / 真红 / 裸
//! `failed to write`）、字节面边界（空 / 单字节 / 大块 / 非 UTF-8 / 特殊字符）、
//! `RetryPolicy` 两构造的三常量锚定与直构边界、`AttemptTrace` 字段面；增量推进
//! 判据三函数（[`compiling_count`] / [`progress_series`] / [`is_progress_stalled`]）
//! 的口径 / 序 / 长度 / 字节面边界。

use std::time::Duration;

use super::{
    compiling_count, is_progress_stalled, is_transient, progress_series, transient_signature,
    AttemptTrace, RetryPolicy,
};
use crate::run::{RunOutcome, RunReport};

// ---------------------------------------------------------------------------
// 装置：`RunReport` 字面量构造（不 spawn、不落盘）
// ---------------------------------------------------------------------------

/// 直构执行报告（被测 API 的显式入参；`attempts` / `retried` 与判定无关，
/// 取最小合法值）。
fn report(outcome: RunOutcome, exit_code: i32, stdout: &[u8], stderr: &[u8]) -> RunReport {
    RunReport {
        outcome,
        exit_code,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        attempts: 1,
        retried: Vec::new(),
    }
}

/// 非零退出的 `stdout` 单路命中形态（判定面最常用形态）。
fn failed_stdout(bytes: &[u8]) -> RunReport {
    report(RunOutcome::Completed, 1, bytes, b"")
}

/// 此前尝试的留痕（留痕面最常用形态：非零退出 + 命中签名）。
fn trace(stdout: &[u8], stderr: &[u8]) -> AttemptTrace {
    AttemptTrace {
        exit_code: 1,
        transient: Some("os error 32".to_owned()),
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    }
}

/// N 行 `Compiling <crate> v0.1.0`（rustc 缩进形态：三空格前缀）的原始字节。
fn compiling_lines(count: u32) -> Vec<u8> {
    (0..count)
        .map(|index| {
            format!(
                "   Compiling crate{index} v0.1.0
"
            )
        })
        .collect::<String>()
        .into_bytes()
}

// ---------------------------------------------------------------------------
// 正向：封闭签名表逐条命中
// ---------------------------------------------------------------------------

/// 正向（表驱动 13 条封闭表）：签名表逐条命中 —— `Completed` 且 `exit_code = 1`
/// 且 `stdout` 含该行签名时返回**表中原文**（`LNK1104` 保持大写、`os error 32`
/// 保持小写，不返回输出片段）。
#[test]
fn 签名表逐条命中_返回表中原文() {
    // 封闭表 13 条（Windows 锁族 7 / MSVC 1 / POSIX·Node 5）——逐条硬编码即
    // 「删条目 / 改写法即被击杀」；第 6 条为收窄后的 `failed to remove`
    // （**不含** `file`——生产文本为 `failed to remove <path>`）
    const TABLE: [&str; 13] = [
        "os error 32",
        "os error 5",
        "另一个程序正在使用此文件",
        "being used by another process",
        "failed to rename archive file",
        "failed to remove",
        "Access is denied",
        "LNK1104",
        "EBUSY",
        "ETXTBSY",
        "Text file busy",
        "EPERM: operation not permitted",
        "resource busy or locked",
    ];

    for signature in TABLE {
        let stdout = format!("error: {signature}: detail");
        let hit = is_transient(&failed_stdout(stdout.as_bytes()));
        assert_eq!(
            hit,
            Some(signature),
            "签名 {signature:?} 应命中且返回表中原文"
        );
    }
}

/// 正向（参数化 3 形态 × 抽样 4 条）：大小写不敏感——全大写 / 首字母大写 /
/// 全小写三形态注入 → 均命中且返回**表中原文**（不是输出里的变形）。
#[test]
fn 大小写不敏感_三形态均返回表中原文() {
    // (表中原文, 全大写, 首字母大写) —— 中文条目不参与大小写变换
    const CASES: [(&str, &str, &str); 4] = [
        ("os error 32", "OS ERROR 32", "Os Error 32"),
        ("lnk1104", "LNK1104", "Lnk1104"),
        ("ebusy", "EBUSY", "Ebusy"),
        (
            "resource busy or locked",
            "RESOURCE BUSY OR LOCKED",
            "Resource Busy Or Locked",
        ),
    ];
    // 表内 `LNK1104` / `EBUSY` 原文为大写：断言取实际返回，再由下方表驱动钉住原文
    const CANONICAL: [&str; 4] = ["os error 32", "LNK1104", "EBUSY", "resource busy or locked"];

    for (index, (_, upper, title)) in CASES.iter().enumerate() {
        for variant in [upper, title] {
            let stdout = format!("error: {variant}");
            let hit = is_transient(&failed_stdout(stdout.as_bytes()))
                .unwrap_or_else(|| panic!("形态 {variant:?} 应命中（大小写不敏感）"));
            assert_eq!(
                hit, CANONICAL[index],
                "返回表中原文而非输出片段（输入 {variant:?}）"
            );
        }
    }
}

/// 正向（表驱动 3 形态）：两路合并匹配——签名只在 `stderr`（`stdout` 空）→
/// 命中；只在 `stdout`（`stderr` 空）→ 命中；两路各含不同签名 → 命中且取合并
/// 文本中**靠前**的命中原文。
#[test]
fn 两路合并匹配_stdout与stderr各形态() {
    // 形态一：只在 stderr
    let only_stderr = report(RunOutcome::Completed, 1, b"", b"fatal: EBUSY");
    assert_eq!(
        is_transient(&only_stderr),
        Some("EBUSY"),
        "仅 stderr 含签名应命中"
    );

    // 形态二：只在 stdout
    let only_stdout = report(RunOutcome::Completed, 1, b"fatal: ETXTBSY", b"");
    assert_eq!(
        is_transient(&only_stdout),
        Some("ETXTBSY"),
        "仅 stdout 含签名应命中"
    );

    // 形态三：两路各含不同签名 → 合并顺序 stdout 在前，故取 stdout 侧签名
    let both = report(
        RunOutcome::Completed,
        1,
        b"stdout-side: os error 32",
        b"stderr-side: resource busy or locked",
    );
    assert_eq!(
        is_transient(&both),
        Some("os error 32"),
        "合并 = stdout ++ stderr，命中取合并文本中靠前者"
    );

    // 对照：交换两路内容即取 stderr 侧（证明合并顺序确实影响选择）
    let swapped = report(
        RunOutcome::Completed,
        1,
        b"stdout-side: resource busy or locked",
        b"stderr-side: os error 32",
    );
    assert_eq!(
        is_transient(&swapped),
        Some("os error 32"),
        "表内靠前者优先于合并顺序（表迭代序决定选中项）"
    );
}

/// 正向（参数化 2 形态）：多行与前后噪声——首行 `Compiling …`、末行锁错误 →
/// 命中；签名前后含 CRLF / 制表符 / 前后空白 → 逐行包含匹配仍命中。
#[test]
fn 多行与前后噪声_逐行包含匹配() {
    let multiline = b"Compiling core v0.1.0\n   Compiling infra v0.1.0\nerror: failed to rename archive file: /tmp/liba.rlib: os error 32\n";
    assert_eq!(
        is_transient(&failed_stdout(multiline)),
        Some("os error 32"),
        "多行输出的末行签名应命中"
    );

    let noisy = b"\tea\r\n  EBUSY\t \r\n";
    assert_eq!(
        is_transient(&failed_stdout(noisy)),
        Some("EBUSY"),
        "CRLF / 制表符 / 空白噪声不阻断命中"
    );
}

/// 回归：编译诊断的源码摘录行不是环境证据——rustc / clippy 报错会引用出错
/// 源码行（`287 | b"… os error 32"`），fixture 里的签名措辞 MUST NOT 命中；
/// 同一输出中出现真实错误行时仍应命中。
#[test]
fn 源码摘录行不命中_真实错误行仍命中() {
    let quoted = "error[E0061]: this function takes 4 arguments but 3 arguments were supplied\n   --> crates\\infra\\checks\\src\\environment_test.rs:284:16\n    |\n284 |       let text = fault_text(\n285 | |         5,\n287 |         b\"error: failed to rename archive file: /t/liba.rlib: os error 32\",\n    |\n";
    assert_eq!(
        is_transient(&failed_stdout(quoted.as_bytes())),
        None,
        "源码摘录行中的签名不构成瞬态证据"
    );

    let with_real = format!(
        "{quoted}error: failed to write C:\\w\\target\\lib.rmeta: 另一个程序正在使用此文件 (os error 32)\n"
    );
    assert_eq!(
        is_transient(&failed_stdout(with_real.as_bytes())),
        Some("os error 32"),
        "同一输出里的真实错误行仍命中"
    );
}

// ---------------------------------------------------------------------------
// 异常：三合一判据的三条反例
// ---------------------------------------------------------------------------

/// 异常（AC-2）：`TimedOut` 恒不命中——输出全文含 `os error 32` 且 `exit_code`
/// 非零仍 `None`（超时重试会把 1200 s 放大成数小时）。
#[test]
fn timedout恒不命中() {
    let timed_out = report(
        RunOutcome::TimedOut,
        -1,
        b"error: failed to rename archive file: /tmp/x.rlib: os error 32",
        b"",
    );
    assert_eq!(
        is_transient(&timed_out),
        None,
        "超时臂零重试：输出含签名也不得命中"
    );
}

/// 异常（AC-2）：`Cancelled` 恒不命中——输出含 `failed to rename archive file`
/// 仍 `None`（取消必须立即生效）。
#[test]
fn cancelled恒不命中() {
    let cancelled = report(
        RunOutcome::Cancelled,
        -1,
        b"error: failed to rename archive file: /tmp/x.rlib",
        b"",
    );
    assert_eq!(
        is_transient(&cancelled),
        None,
        "取消臂零重试：输出含签名也不得命中"
    );
}

/// 异常：绿输出不命中——`exit_code == 0` 时即便输出含签名（测试用例自打印该
/// 串）也 `None`（绿不因文本重试）。
#[test]
fn 零退出不命中() {
    let green = report(
        RunOutcome::Completed,
        0,
        "test 打印了 os error 32 这句话\nok".as_bytes(),
        b"",
    );
    assert_eq!(is_transient(&green), None, "绿不因文本重试");
}

/// 异常（AC-2 / R-1）：真红不命中——普通编译错误 `error[E0308]: mismatched types`
/// 不属瞬态签名表 → `None`。
#[test]
fn 真红不命中() {
    let real_red = report(
        RunOutcome::Completed,
        1,
        b"",
        b"error[E0308]: mismatched types\n  --> src/main.rs:3:9",
    );
    assert_eq!(
        is_transient(&real_red),
        None,
        "普通编译错误不重试（不吞成环境错误）"
    );
}

// ---------------------------------------------------------------------------
// 边界：裸措辞、字节面、特殊字符、哨兵
// ---------------------------------------------------------------------------

/// 边界（表驱动 2 形态，AC-1 / D5）：裸 `failed to write <path>`（无 `os error`
/// 码、无锁措辞）→ `None`；同串外加 `os error 32` → 命中（同 line 覆盖口径）。
#[test]
fn 裸failed_to_write_n不命中而同串带错误码命中() {
    let bare = b"error: failed to write /tmp/x.rmeta\n";
    assert_eq!(
        is_transient(&failed_stdout(bare)),
        None,
        "裸措辞过泛，刻意不收（真实写盘缺陷不得被吞成环境错误）"
    );

    let with_code = b"error: failed to write /tmp/x.rmeta: os error 32\n";
    assert_eq!(
        is_transient(&failed_stdout(with_code)),
        Some("os error 32"),
        "同行内的 os error 32 覆盖写盘失败面"
    );
}

/// 边界（表驱动 4 形态）：输出字节边界——双空 → `None`；单字节 → `None`；
/// >64 KiB 且签名在末尾 → 命中（不截断判定）；非 UTF-8 原始字节（`0xFF 0xFE`
/// + 签名）→ lossy 后仍命中、不 panic。
#[test]
fn 输出字节边界_四形态() {
    // 形态一：双空
    let empty = report(RunOutcome::Completed, 1, b"", b"");
    assert_eq!(is_transient(&empty), None, "双空输出无签名可命中");

    // 形态二：单字节
    let single = report(RunOutcome::Completed, 1, b"x", b"");
    assert_eq!(is_transient(&single), None, "单字节无签名");

    // 形态三：>64 KiB 且签名在末尾（判定不截断）
    let mut big = vec![b'x'; 64 * 1024 + 1024];
    big.extend_from_slice(b"\nerror: EBUSY");
    assert_eq!(
        is_transient(&report(RunOutcome::Completed, 1, &big, b"")),
        Some("EBUSY"),
        "大块输出末尾的签名不被截断丢弃"
    );

    // 形态四：非 UTF-8 原始字节（lossy 后仍可判）
    let mut raw = vec![0xFF_u8, 0xFE];
    raw.extend_from_slice(b" error: os error 32");
    assert_eq!(
        is_transient(&report(RunOutcome::Completed, 1, &raw, b"")),
        Some("os error 32"),
        "非 UTF-8 字节 lossy 后仍命中且不 panic"
    );
}

/// 边界（参数化 2 形态）：特殊字符诊断——签名混在含引号 / 换行 / `\0` / emoji
/// 的输出中 → 命中且不 panic；中文签名单独一臂验证 UTF-8 比对。
#[test]
fn 特殊字符诊断_两形态() {
    let quoted = "say \"os error 32\" then\n\u{0} emoji 🎉 完成".as_bytes();
    assert_eq!(
        is_transient(&failed_stdout(quoted)),
        Some("os error 32"),
        "引号 / 换行 / NUL / emoji 混排不阻断命中"
    );

    let chinese = "错误：另一个程序正在使用此文件，进程无法访问。".as_bytes();
    assert_eq!(
        is_transient(&failed_stdout(chinese)),
        Some("另一个程序正在使用此文件"),
        "中文签名经 UTF-8 比对命中"
    );
}

/// 边界（表驱动 2 形态）：`Completed` + `-1` 哨兵——「无可读退出码」也属
/// `exit_code != 0`，故有签名即命中；无签名 → `None`（哨兵不被误读为绿）。
#[test]
fn completed负一哨兵_两形态() {
    let with_signature = report(RunOutcome::Completed, -1, b"LNK1104: cannot open file", b"");
    assert_eq!(
        is_transient(&with_signature),
        Some("LNK1104"),
        "哨兵 -1 属非零，签名命中"
    );

    let no_signature = report(RunOutcome::Completed, -1, b"", b"");
    assert_eq!(
        is_transient(&no_signature),
        None,
        "哨兵不被误读为绿，但无签名仍不命中"
    );
}

/// 边界（表驱动 4 组输入）：私有判定核心直驱——`transient_signature(outcome,
/// exit_code, stdout, stderr)` 与 `is_transient(&report)` 对同一组输入逐值
/// 一致（公开面只做委托，判定逻辑单点）。
#[test]
fn 私有判定核心与公开面逐值一致() {
    let cases: [(RunOutcome, i32, &[u8], &[u8]); 4] = [
        (RunOutcome::Completed, 1, b"os error 32", b""),
        (RunOutcome::Completed, 0, b"os error 32", b""),
        (RunOutcome::TimedOut, -1, b"EBUSY", b""),
        (RunOutcome::Completed, 1, b"", b"Access is denied"),
    ];

    for (outcome, exit_code, stdout, stderr) in cases {
        let core = transient_signature(outcome, exit_code, stdout, stderr);
        let public = is_transient(&report(outcome, exit_code, stdout, stderr));
        assert_eq!(
            core, public,
            "私有核心与公开面对 ({outcome:?}, {exit_code}, {stdout:?}, {stderr:?}) 逐值一致"
        );
    }
}

// ---------------------------------------------------------------------------
// 正向 / 边界：RetryPolicy 两构造与字段面
// ---------------------------------------------------------------------------

/// 正向（AC-3）：`RetryPolicy::transient()` 三常量锚定——`max_attempts == 5`
/// （首跑 + 4 次重试）、`backoff == [5, 10, 20, 40] s`（慢档）、
/// `total_budget == 1500 s`（改值即被击杀）。
#[test]
fn transient策略三常量锚定() {
    let policy = RetryPolicy::transient();
    assert_eq!(policy.max_attempts, 5, "首跑 + 4 次重试");
    assert_eq!(
        policy.backoff,
        [
            Duration::from_secs(5),
            Duration::from_secs(10),
            Duration::from_secs(20),
            Duration::from_secs(40),
        ],
        "退避四段递增（慢档）"
    );
    assert_eq!(
        policy.total_budget,
        Duration::from_secs(1500),
        "总预算 1500 s（自首跑起算）"
    );
}

/// 正向（AC-2）：`RetryPolicy::none()` 单次语义——`max_attempts == 1`；
/// `backoff` / `total_budget` 取同类常量值（不被消费）；连续两次调用相等。
#[test]
fn none策略单次语义且两次调用相等() {
    let policy = RetryPolicy::none();
    assert_eq!(policy.max_attempts, 1, "单次执行，退避与预算不消费");
    assert_eq!(
        policy.backoff,
        [
            Duration::from_secs(5),
            Duration::from_secs(10),
            Duration::from_secs(20),
            Duration::from_secs(40),
        ],
        "退避取同类常量值（不消费）"
    );
    assert_eq!(
        policy.total_budget,
        Duration::from_secs(1500),
        "预算取同类常量值（不消费）"
    );
    assert_eq!(RetryPolicy::none(), policy, "两次调用相等（纯构造）");
}

/// 边界（表驱动 2 形态）：`RetryPolicy` 相等 / clone / Debug 派生面——
/// `none() != transient()`；`transient().clone() == transient()`；`Debug` 渲染
/// 含 `max_attempts` / `total_budget`。
#[test]
fn 策略相等clone与debug面() {
    assert_ne!(
        RetryPolicy::none(),
        RetryPolicy::transient(),
        "单次与瞬态重试互不相等"
    );
    assert_eq!(
        RetryPolicy::transient().clone(),
        RetryPolicy::transient(),
        "clone 后逐字段相等"
    );

    let debug = format!("{:?}", RetryPolicy::transient());
    for field in ["max_attempts", "backoff", "total_budget"] {
        assert!(debug.contains(field), "Debug 含字段 {field}: {debug}");
    }
}

/// 边界（表驱动 3 形态）：字段直构边界策略——`max_attempts = 1` / `2`、
/// `total_budget = ZERO` 均可读且构造无 `panic`（为执行体重试用例提供装置；
/// 生产构造面仍收口于两 fn）。
#[test]
fn 直构边界策略三形态() {
    let single = RetryPolicy {
        max_attempts: 1,
        ..RetryPolicy::transient()
    };
    assert_eq!(
        single.max_attempts, 1,
        "上限 1 可直构（等价 none 的行为面）"
    );
    assert_eq!(
        single.total_budget,
        Duration::from_secs(1500),
        "其余字段随动"
    );

    let twice = RetryPolicy {
        max_attempts: 2,
        ..RetryPolicy::transient()
    };
    assert_eq!(twice.max_attempts, 2, "上限 2 可直构（一次重试）");

    let no_budget = RetryPolicy {
        total_budget: Duration::ZERO,
        ..RetryPolicy::transient()
    };
    assert_eq!(no_budget.total_budget, Duration::ZERO, "零预算可直构");
    assert!(
        no_budget.total_budget < Duration::from_secs(1),
        "零预算极小"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 边界：AttemptTrace 字段面
// ---------------------------------------------------------------------------

/// 正向（AC-1）：`AttemptTrace` 四字段面——构造后可读、`clone` 载荷逐字节
/// 相等、`PartialEq` 自反；`transient: None`（未命中）同样合法。
#[test]
fn attempt_trace四字段面() {
    let trace = AttemptTrace {
        exit_code: 1,
        transient: Some("os error 32".to_owned()),
        stdout: b"out".to_vec(),
        stderr: b"err".to_vec(),
    };

    assert_eq!(trace.exit_code, 1, "退出码可读");
    assert_eq!(
        trace.transient.as_deref(),
        Some("os error 32"),
        "命中签名原文可读"
    );
    assert_eq!(trace.stdout, b"out", "stdout 字节可读");
    assert_eq!(trace.stderr, b"err", "stderr 字节可读");

    let cloned = trace.clone();
    assert_eq!(cloned, trace, "clone 载荷逐字节相等");
    assert_eq!(cloned.stdout, trace.stdout, "stdout 逐字节相等");
    assert_eq!(cloned.stderr, trace.stderr, "stderr 逐字节相等");
    assert_eq!(cloned, cloned, "PartialEq 自反");

    let unmatched = AttemptTrace {
        exit_code: 1,
        transient: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    assert!(unmatched.transient.is_none(), "未命中签名的留痕合法");
    assert_ne!(unmatched, trace, "命中断言参与相等判定");
}

/// 边界（表驱动 4 形态）：`AttemptTrace` 输出边界——双空 / 单字节 / 大块 /
/// 非 UTF-8 字段原样保留（零改写零截断）；`transient` 取空串（防御性直构，
/// 生产不产生）不 panic。
#[test]
fn attempt_trace输出边界四形态() {
    let big = vec![b'q'; 64 * 1024 + 1];
    let raw = vec![0xFF_u8, 0xFE, 0x00];

    let cases: [(&str, Vec<u8>, Vec<u8>); 4] = [
        ("双空", Vec::new(), Vec::new()),
        ("单字节", b"S".to_vec(), b"E".to_vec()),
        ("大块", big.clone(), big.clone()),
        ("非 UTF-8", raw.clone(), raw.clone()),
    ];

    for (label, stdout, stderr) in cases {
        let trace = AttemptTrace {
            exit_code: 255,
            transient: None,
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        };
        assert_eq!(trace.stdout, stdout, "{label}: stdout 原样保留");
        assert_eq!(trace.stderr, stderr, "{label}: stderr 原样保留");
        assert_eq!(trace.stdout.len(), stdout.len(), "{label}: 零截断");
    }

    // 防御性直构：transient 为空串不 panic（生产不产生该形态）
    let empty_signature = AttemptTrace {
        exit_code: 1,
        transient: Some(String::new()),
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    assert_eq!(
        empty_signature.transient.as_deref(),
        Some(""),
        "空串签名可读且不 panic"
    );
    assert!(
        format!("{empty_signature:?}").contains("AttemptTrace"),
        "Debug 派生可读"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 边界（P4）：remove 形态措辞收窄（经 `is_transient` 公共面）
// ---------------------------------------------------------------------------

/// 正向（表驱动 2 形态，AC-4）：`failed to remove` 真实命中——生产文本
/// `failed to remove D:\…\x.rcgu.o`（同一行无 `os error` 码）→ 返回**表内原文**；
/// 带旧措辞 `failed to remove file` 的输入同样返回 `failed to remove`（表中已无
/// `file` 分支——返回的是表内原文而非输出片段）。
#[test]
fn remove形态真实命中_返回表内原文() {
    let production = b"error: failed to remove D:\\w\\target\\debug\\deps\\x.rcgu.o";
    assert_eq!(
        is_transient(&failed_stdout(production)),
        Some("failed to remove"),
        "生产 remove 文本命中且返回表内原文"
    );
    assert!(
        !String::from_utf8_lossy(production).contains("os error"),
        "该形态可单独命中（不靠同行错误码兜住）"
    );

    let legacy = b"error: failed to remove file: /t/x.rcgu.o";
    let hit = is_transient(&failed_stdout(legacy));
    assert_eq!(hit, Some("failed to remove"), "旧措辞输入仍命中");
    assert!(
        !hit.unwrap_or_default().contains("file"),
        "返回原文不含 `file`（表内已无该分支）"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 异常 / 边界：`compiling_count`（待编译计数口径单点）
// ---------------------------------------------------------------------------

/// 正向（表驱动 3 形态）：多行计数——rustc 缩进形态两路合并口径（`stdout ++
/// stderr`）、非 `Compiling` 行（`Finished …`）不计数。
#[test]
fn 编译计数_多行与两路合并() {
    // 形态一：stdout 两行
    assert_eq!(
        compiling_count(b"   Compiling a v0.1.0\n   Compiling b v0.2.0\n", b""),
        2,
        "stdout 侧两行计数"
    );

    // 形态二：stderr 侧另含 1 行（两路合并）
    assert_eq!(
        compiling_count(
            b"   Compiling a v0.1.0\n   Compiling b v0.2.0\n",
            b"   Compiling c v0.3.0\n",
        ),
        3,
        "合并 = stdout ++ stderr，两路都计"
    );

    // 形态三：末行 `Finished dev …` 不计数
    assert_eq!(
        compiling_count(
            b"   Compiling a v0.1.0\n    Finished dev [unoptimized] target(s)\n",
            b"",
        ),
        1,
        "Finished 行不计数"
    );
}

/// 异常（表驱动 6 形态）：非 cargo 输出恒 `0`——锁错误行 / 普通编译错误 /
/// 全小写 `compiling` / 无尾随空格的 `Compiling` / 前缀不成立的 `Compilingx` /
/// 行中出现的 `Compiling`（非行首）均不计数。
#[test]
fn 编译计数_非cargo输出恒零() {
    const NON_CARGO: [&[u8]; 6] = [
        b"error: failed to rename archive file: /t/liba.rlib: os error 32",
        b"error[E0308]: mismatched types",
        b"compiling a v0.1.0",
        b"Compiling",
        b"Compilingx",
        b"error: Compiling failed",
    ];

    for body in NON_CARGO {
        assert_eq!(
            compiling_count(body, b""),
            0,
            "非 cargo 形态 {:?} 计数为 0",
            String::from_utf8_lossy(body)
        );
    }
}

/// 边界（表驱动 7 形态）：前导空白与行内形态——零缩进 / 空格 / 制表符 / 混合
/// 空白均计数；`Compiling ` 后直接换行（无 crate 名）仍计数；行首 `\0` 或 emoji
/// 前置不计数（`trim_start` 只去空白）。
#[test]
fn 编译计数_前导空白与行内形态() {
    assert_eq!(
        compiling_count(b"Compiling a v0.1.0\n", b""),
        1,
        "零缩进形态计数"
    );
    assert_eq!(
        compiling_count(b"   Compiling a v0.1.0\n", b""),
        1,
        "空格缩进计数"
    );
    assert_eq!(
        compiling_count(b"\tCompiling a v0.1.0\n", b""),
        1,
        "制表符缩进计数"
    );
    assert_eq!(
        compiling_count(b" \t \t Compiling a v0.1.0\n", b""),
        1,
        "混合空白缩进计数"
    );
    assert_eq!(
        compiling_count(b"Compiling \n", b""),
        1,
        "无 crate 名的 `Compiling ` 仍计数（含尾随空格）"
    );
    assert_eq!(
        compiling_count(b"\0Compiling a v0.1.0\n", b""),
        0,
        "行首 NUL 不属空白 → 不计数"
    );
    assert_eq!(
        compiling_count("🎉Compiling a v0.1.0\n".as_bytes(), b""),
        0,
        "行首 emoji 不属空白 → 不计数"
    );
}

/// 边界（表驱动 5 形态）：字节面边界——双空 / 单字节 / >64 KiB 且 5000 行
/// `Compiling`（不截断）/ 非 UTF-8 前缀字节（lossy 后仍计数、不 panic）/
/// 纯 CRLF 行尾（`lines()` 吃 `\r`）。
#[test]
fn 编译计数_字节面边界() {
    // 形态一：双空
    assert_eq!(compiling_count(b"", b""), 0, "双空输出计数 0");

    // 形态二：单字节
    assert_eq!(compiling_count(b"x", b""), 0, "单字节计数 0");

    // 形态三：大块（>64 KiB）且 5000 行 Compiling —— 不截断
    let big = compiling_lines(5000);
    assert!(big.len() > 64 * 1024, "样本确实超过 64 KiB");
    assert_eq!(compiling_count(&big, b""), 5000, "大块逐行全量计数");

    // 形态四：非 UTF-8 前缀字节（lossy 不 panic）
    let mut raw = vec![0xFF_u8, 0xFE];
    raw.extend_from_slice(b"\nCompiling x v0.1.0\n");
    assert_eq!(
        compiling_count(&raw, b""),
        1,
        "非 UTF-8 前缀 lossy 后仍计数（该行不计数、下一行计数）"
    );

    // 形态五：纯 CRLF 行尾（`lines()` 天然吃掉 `\r`）
    assert_eq!(
        compiling_count(b"   Compiling a v0.1.0\r\n   Compiling b v0.1.0\r\n", b""),
        2,
        "CRLF 行尾逐行计数"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 边界：`progress_series`（按尝试序的计数序列）
// ---------------------------------------------------------------------------

/// 正向：未重试长度 1——`attempts == 1`、`retried` 空、`stdout` 5 行
/// `Compiling` → `vec![5]`（长度 1 == `attempts`）。
#[test]
fn 推进序列_未重试长度一() {
    let report = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: compiling_lines(5),
        stderr: Vec::new(),
        attempts: 1,
        retried: Vec::new(),
    };

    let series = progress_series(&report);
    assert_eq!(series, vec![5], "未重试即长度 1 且取末次计数");
    assert_eq!(series.len() as u32, report.attempts, "长度 == attempts");
}

/// 正向：序与长度（生产形态）——`retried` 三条（264 / 5 / 0 行）+ 末次 0 行、
/// `attempts == 4` → `[264, 5, 0, 0]`（按尝试序，末次在后，长度 == `attempts`）。
#[test]
fn 推进序列_按尝试序与长度() {
    let report = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: b"error: os error 32\n".to_vec(),
        stderr: Vec::new(),
        attempts: 4,
        retried: vec![
            trace(&compiling_lines(264), b""),
            trace(&compiling_lines(5), b""),
            trace(&compiling_lines(0), b""),
        ],
    };

    let series = progress_series(&report);
    assert_eq!(series, vec![264, 5, 0, 0], "按尝试序排列，末次在后");
    assert_eq!(series.len() as u32, report.attempts, "长度 == attempts");
}

/// 边界（表驱动 2 形态）：耗尽与越界长度——`retried` 4 条 + 末次 → 长度 5
/// （`transient()` 上限形态）；`retried` 5 条直构（越出生产不变量）→ 长度 6、
/// 零 panic。
#[test]
fn 推进序列_耗尽与越界长度() {
    let exhausted = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: Vec::new(),
        stderr: Vec::new(),
        attempts: 5,
        retried: vec![
            trace(&compiling_lines(10), b""),
            trace(&compiling_lines(8), b""),
            trace(&compiling_lines(3), b""),
            trace(&compiling_lines(1), b""),
        ],
    };
    assert_eq!(
        progress_series(&exhausted),
        vec![10, 8, 3, 1, 0],
        "上限形态长度 5（== attempts）"
    );

    let overflow = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: Vec::new(),
        stderr: Vec::new(),
        attempts: 6,
        retried: vec![
            trace(&compiling_lines(1), b""),
            trace(&compiling_lines(1), b""),
            trace(&compiling_lines(1), b""),
            trace(&compiling_lines(1), b""),
            trace(&compiling_lines(1), b""),
        ],
    };
    assert_eq!(
        progress_series(&overflow).len(),
        6,
        "越界直构长度 = retried.len() + 1，不 panic"
    );
}

/// 边界（表驱动 2 形态）：直构违反不变量——`attempts = 3` 但 `retried = []`
/// → 长度 1（doc 口径「长度 = `retried.len() + 1`」，不按 `attempts` 补齐）；
/// `attempts = 0` 直构同样不 panic。
#[test]
fn 推进序列_直构违反不变量() {
    let mismatched = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: compiling_lines(3),
        stderr: Vec::new(),
        attempts: 3,
        retried: Vec::new(),
    };
    assert_eq!(
        progress_series(&mismatched),
        vec![3],
        "序列按 retried 反推，不按 attempts 补齐"
    );

    let zero_attempts = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 0,
        stdout: Vec::new(),
        stderr: Vec::new(),
        attempts: 0,
        retried: Vec::new(),
    };
    assert_eq!(
        progress_series(&zero_attempts),
        vec![0],
        "attempts == 0 直构不 panic"
    );
}

/// 边界（表驱动 3 形态）：两路合并与字节形态——某 `retried` 条目的
/// `Compiling` 行仅在 `stderr` → 计入该位；末次 `stderr` 侧含 `Compiling` 行 →
/// 计入末位；非 UTF-8 / emoji / `\0` 混排 → lossy 计数不 panic。
#[test]
fn 推进序列_两路合并与字节形态() {
    // 形态一：retried 条目的 Compiling 行只在 stderr
    let stderr_side = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: b"error: os error 32\n".to_vec(),
        stderr: Vec::new(),
        attempts: 2,
        retried: vec![trace(b"", &compiling_lines(2))],
    };
    assert_eq!(
        progress_series(&stderr_side),
        vec![2, 0],
        "此前尝试的 stderr 侧 Compiling 行计入该位"
    );

    // 形态二：末次 stderr 侧含 Compiling 行
    let last_stderr = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: b"error: os error 32\n".to_vec(),
        stderr: compiling_lines(4),
        attempts: 2,
        retried: vec![trace(&compiling_lines(7), b"")],
    };
    assert_eq!(
        progress_series(&last_stderr),
        vec![7, 4],
        "末次两路合并计数"
    );

    // 形态三：非 UTF-8 / emoji / NUL 混排 → lossy 计数不 panic
    let mut raw = vec![0xFF_u8, 0xFE, b'\n'];
    raw.extend_from_slice("🎉\u{0} noise\n   Compiling a v0.1.0\n".as_bytes());
    let messy = RunReport {
        outcome: RunOutcome::Completed,
        exit_code: 1,
        stdout: raw,
        stderr: Vec::new(),
        attempts: 1,
        retried: Vec::new(),
    };
    assert_eq!(
        progress_series(&messy),
        vec![1],
        "特殊字符混排下计数不 panic"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 异常 / 边界：`is_progress_stalled`（卡死判据）
// ---------------------------------------------------------------------------

/// 正向（表驱动 4 形态）：判卡死——末两次相等 `[50, 50]` / 末尾相等
/// `[264, 5, 5]` / **上升** `[5, 9]`（非降而非相等）/ `[u32::MAX, u32::MAX]`
/// → `true`。
#[test]
fn 判据_末两次非降判卡死() {
    const STALLED: [&[u32]; 4] = [&[50, 50], &[264, 5, 5], &[5, 9], &[u32::MAX, u32::MAX]];

    for series in STALLED {
        assert!(
            is_progress_stalled(series),
            "序列 {series:?} 应判卡死（末值 > 0 且非降）"
        );
    }
}

/// 异常（表驱动 5 形态）：推进或收尾不卡死——递减 `[264, 5]` /
/// `[264, 5, 0]` / `[264, 5, 0, 0]` / `[9, 5]` / 末值归零 `[10, 0]` → `false`。
#[test]
fn 判据_递减与末值归零不卡死() {
    const PROGRESSING: [&[u32]; 5] = [&[264, 5], &[264, 5, 0], &[264, 5, 0, 0], &[9, 5], &[10, 0]];

    for series in PROGRESSING {
        assert!(
            !is_progress_stalled(series),
            "序列 {series:?} 不判卡死（递减或末值 0）"
        );
    }
}

/// 边界（表驱动 3 形态）：空与单值——`[]` / `[50]`（`attempts == 1` 无前值）/
/// `[0]` → 均 `false`。
#[test]
fn 判据_空与单值不卡死() {
    assert!(!is_progress_stalled(&[]), "空序列无前值");
    assert!(!is_progress_stalled(&[50]), "单值无前值（首跑不触发）");
    assert!(!is_progress_stalled(&[0]), "单值 0 判否");
}

/// 边界（表驱动 4 形态）：全 0 与大列表——`[0, 0]` / `[0, 0, 0, 0]` 末值 0 恒
/// 判否；`[0, 3]` 前值 0、末值 > 0 仍判卡死；1000 项全等判卡死（超大列表不
/// 退化）。
#[test]
fn 判据_全零与大列表() {
    assert!(!is_progress_stalled(&[0, 0]), "全 0 判否");
    assert!(!is_progress_stalled(&[0, 0, 0, 0]), "全 0 长序列判否");
    assert!(
        is_progress_stalled(&[0, 3]),
        "前值 0、末值 > 0 仍属「写不进去」"
    );

    let huge = vec![7u32; 1000];
    assert!(is_progress_stalled(&huge), "1000 项全等（超大列表）判卡死");
}

// ---------------------------------------------------------------------------
// 边界：`RetryPolicy` 常量关系与两构造
// ---------------------------------------------------------------------------

/// 边界（表驱动 3 形态）：常量关系与两构造——`backoff` 四段严格递增且总和
/// `75 s < total_budget`（最坏重试总等待仍远低于硬顶）；`none()` 的
/// `backoff` / `total_budget` 取同类常量（不被消费）；`none() != transient()`。
#[test]
fn 策略常量关系与两构造边界() {
    let transient = RetryPolicy::transient();
    for pair in transient.backoff.windows(2) {
        assert!(
            pair[0] < pair[1],
            "退避四段严格递增：{:?} 不小于 {:?}",
            pair[0],
            pair[1]
        );
    }
    let total_wait: Duration = transient.backoff.iter().sum();
    assert_eq!(total_wait, Duration::from_secs(75), "四段总和 75 s");
    assert!(
        total_wait < transient.total_budget,
        "最坏重试总等待 75 s 远低于总预算 1500 s"
    );

    let none = RetryPolicy::none();
    assert_eq!(none.backoff, transient.backoff, "none 取同类退避常量");
    assert_eq!(
        none.total_budget, transient.total_budget,
        "none 取同类预算常量"
    );
    assert_eq!(none.max_attempts, 1, "none 单次执行");
    assert_ne!(none, transient, "两构造互不相等");
}
