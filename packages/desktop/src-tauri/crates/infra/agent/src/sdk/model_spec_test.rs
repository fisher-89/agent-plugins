//! `[1m]` 模型名后缀解析单测：后缀剥离 / 无后缀保真 / 空白裁剪 / 拒收面 /
//! 字符串与长度边界 / 值语义（`ModelSpec` + `ModelSpecError`）。
//!
//! 零进程边界：`parse_model_spec` 为纯 std 函数，无 I/O / 网络 / store /
//! 全局态，全部真实调用（不 mock）。

use crate::sdk::model_spec::{parse_model_spec, ModelSpec, ModelSpecError, MILLION_CONTEXT_WINDOW};

/// 逐字断言「裸名 + 窗长候选」两字段（成功臂的收敛装置）。
fn assert_spec(raw: &str, name: &str, length: Option<u64>) {
    let spec = parse_model_spec(raw).unwrap_or_else(|error| {
        panic!("{raw:?} 应解析成功，实际 Err: {error}");
    });
    assert_eq!(
        spec,
        ModelSpec {
            name: name.to_owned(),
            length,
        },
        "{raw:?} 解析产物逐字段一致"
    );
}

/// 逐字断言拒收面（Err 臂的收敛装置）：`raw` 为传入原文、`reason` 非空。
fn assert_reject(raw: &str) -> ModelSpecError {
    let error = match parse_model_spec(raw) {
        Err(error) => error,
        Ok(spec) => panic!("{raw:?} 应拒收，实际 Ok: {spec:?}"),
    };
    assert_eq!(error.raw, raw, "{raw:?} 拒收时承载传入原文（未 trim 原文）");
    assert!(!error.reason.is_empty(), "{raw:?} 拒收时 reason 非空");
    error
}

// ---------------------------------------------------------------------------
// 正向：后缀剥离 / 无后缀保真 / 空白裁剪
// ---------------------------------------------------------------------------

#[test]
fn 后缀剥离出裸名且窗长候选为1000进制一百万() {
    assert_spec(
        "claude-sonnet-4-5[1m]",
        "claude-sonnet-4-5",
        Some(1_000_000),
    );
    assert_eq!(
        MILLION_CONTEXT_WINDOW, 1_000_000,
        "[1m] 对应 1000 进制一百万（非 1024 基）"
    );
    // 连字符 + 数字裸名同样剥离（后缀识别只认末尾四字节）
    assert_spec("claude-haiku-4-5[1m]", "claude-haiku-4-5", Some(1_000_000));
}

#[test]
fn 后缀大小写三形态同结果且裸名大小写保真() {
    // ASCII 大小写不敏感只作用于后缀本身，不改裸名大小写
    for raw in ["x[1m]", "x[1M]", "X[1M]"] {
        let spec = parse_model_spec(raw).expect("大小写形态均应成功");
        assert_eq!(spec.length, Some(1_000_000), "{raw:?} 窗长候选同值");
    }
    assert_spec("x[1m]", "x", Some(1_000_000));
    assert_spec("x[1M]", "x", Some(1_000_000));
    assert_spec("X[1M]", "X", Some(1_000_000));
    // 裸名内的 1m 字面量不被误剥离
    assert_spec("gpt-1m-preview", "gpt-1m-preview", None);
}

#[test]
fn 无后缀模型名逐字原文且窗长候选缺失() {
    assert_spec("claude-opus-4-1", "claude-opus-4-1", None);
    // openai 侧回归面：无方括号即零改写
    assert_spec("gpt-4o-mini", "gpt-4o-mini", None);
}

#[test]
fn 两端空白裁剪且剥离后裸名再次trim() {
    assert_spec("  x[1m]  ", "x", Some(1_000_000));
    // 后缀与裸名之间的空白随剥离被裁掉
    assert_spec("x [1m]", "x", Some(1_000_000));
    // 无后缀：trim 后原文
    assert_spec("  claude-opus-4-1  ", "claude-opus-4-1", None);
}

// ---------------------------------------------------------------------------
// 异常：拒收面 / 方括号位置 / 内空白
// ---------------------------------------------------------------------------

#[test]
fn 非1m后缀与空方括号逐个拒收且理由非空() {
    for raw in ["[2m]", "[200k]", "[8k]", "[]"] {
        assert_reject(raw);
    }
    // 不做单位换算、不降级为无后缀
    assert_reject("x[2m]");
    assert_reject("claude-sonnet-4-5[2m]");
}

#[test]
fn 方括号非末尾或残留时拒收() {
    for raw in [
        "x[1m]y",     // 后缀不在末尾
        "[1m]x",      // 后缀在首位
        "x[1m]y[1m]", // 剥离尾缀后裸名残留 `[`
        "[[1m]",      // 剥离后裸名残留 `[`
        "x[1m][1m]",  // 重复后缀：不贪婪多剥
        "x]y",        // 落单右方括号
        "x[y",        // 落单左方括号
    ] {
        assert_reject(raw);
    }
}

#[test]
fn 后缀内空白与裸名空后缀拒收() {
    for raw in ["[ 1m ]", "[1 m]", "[1m ]", "[ 1m]"] {
        assert_reject(raw);
    }
    // 后缀在场但裸名为空：方括号语义悬空
    let error = assert_reject("[1m]");
    assert!(
        error.reason.contains("[1m]"),
        "裸名空档的 reason 点名后缀形态，实际: {}",
        error.reason
    );
}

// ---------------------------------------------------------------------------
// 边界：空输入 / 字符串载荷 / 长度阈值形态
// ---------------------------------------------------------------------------

#[test]
fn 空输入与纯空白保现状为合法空裸名() {
    // 保现状：provider 三档模型名可留空保存，保存期不得新增拒收面
    for raw in ["", "   ", "\t\n", " \u{3000} "] {
        assert_spec(raw, "", None);
        let spec = parse_model_spec(raw).expect("空输入合法");
        assert_eq!(spec.length, None, "空输入窗长候选恒 None");
    }
}

#[test]
fn 超长模型名带与不带后缀均正常剥离且不panic() {
    let long = "m".repeat(1_500);
    let with_suffix = format!("{long}[1m]");
    let spec = parse_model_spec(&with_suffix).expect("超长裸名 + 后缀应成功");
    assert_eq!(spec.length, Some(1_000_000));
    assert_eq!(spec.name.len(), 1_500, "超长裸名不截断");
    assert!(
        spec.name.starts_with('m') && spec.name.ends_with('m'),
        "首尾无截断"
    );

    let plain = format!("{long}-plain");
    let spec = parse_model_spec(&plain).expect("超长无后缀应成功");
    assert_eq!(spec.length, None);
    assert_eq!(spec.name, plain, "超长无后缀逐字保真");
}

#[test]
fn 多字节与特殊字符模型名按字节边界剥离不切碎utf8() {
    // `]` 为 ASCII 边界：切片不落在多字节序列中间
    assert_spec("模型-🎉[1m]", "模型-🎉", Some(1_000_000));
    assert_spec("claude-🎉-模型", "claude-🎉-模型", None);
    // `\n` / `\0` / emoji 裸名原样保真（不做字符清洗）
    assert_spec("x\ny", "x\ny", None);
    assert_spec("x\u{0}y", "x\u{0}y", None);
    assert_spec("😀😀", "😀😀", None);
    assert_spec("😀[1m]", "😀", Some(1_000_000));
    // 非法形态含 `\0`：拒收且 Display 拼装不 panic
    let error = assert_reject("\u{0}[2m]");
    let text = error.to_string();
    assert!(text.contains("[2m]"), "Display 含原文，实际: {text}");
}

#[test]
fn 长度阈值形态恰四字节与三字节分支() {
    // 恰 4 字节 `[1m]`：后缀在场但裸名空 → 拒收
    assert_reject("[1m]");
    // 3 字节含 `[` 但非完整后缀 → 拒收
    assert_reject("[1");
    assert_reject("1m]");
    // 裸名恰 1 字符 → 成功
    assert_spec("a[1m]", "a", Some(1_000_000));
    // 无方括号且短于 4 字节 → 成功（不做长度下限校验）
    assert_spec("ab", "ab", None);
    assert_spec("a", "a", None);
}

// ---------------------------------------------------------------------------
// 边界：值语义（PartialEq / Clone / Debug / Display / 千位口径）
// ---------------------------------------------------------------------------

#[test]
fn 值类型派生可用且错误相等按原文加原因精确匹配() {
    let spec = ModelSpec {
        name: "claude-x".to_owned(),
        length: Some(MILLION_CONTEXT_WINDOW),
    };
    let cloned = spec.clone();
    assert_eq!(cloned, spec, "Clone 后 PartialEq 相等");
    assert_ne!(
        ModelSpec {
            name: "claude-x".to_owned(),
            length: None,
        },
        spec,
        "length 双态可区分（后缀在场与否的等价判据）"
    );
    assert!(!format!("{spec:?}").is_empty(), "ModelSpec Debug 派生可用");

    let error = ModelSpecError {
        raw: "x[2m]".to_owned(),
        reason: "仅支持末尾 [1m] 后缀".to_owned(),
    };
    assert_eq!(
        error.clone(),
        error,
        "ModelSpecError Clone / PartialEq 可用"
    );
    assert_ne!(
        ModelSpecError {
            raw: "x[2m]".to_owned(),
            reason: "其它原因".to_owned(),
        },
        error,
        "错误相等按 raw + reason 精确匹配（非按消息字符串）"
    );
    assert!(
        !format!("{error:?}").is_empty(),
        "ModelSpecError Debug 派生可用"
    );
}

#[test]
fn display携原文与原因两段且结构上不含凭据类字段() {
    let raw = "  x[8k]  ";
    let error = assert_reject(raw);
    let text = error.to_string();
    assert!(text.contains(raw), "Display 含模型名原文，实际: {text}");
    assert!(
        text.contains(&error.reason),
        "Display 含 reason，实际: {text}"
    );
    assert!(
        text.starts_with("模型名后缀非法: "),
        "固定前缀，实际: {text}"
    );
    // 结构保证（非约定）：类型只有 raw / reason 两字段，凭据类字段不可能入串
    match parse_model_spec(raw) {
        Err(ModelSpecError {
            raw: got_raw,
            reason,
        }) => {
            assert_eq!(got_raw, raw, "错误载荷承载模型名原文");
            assert!(!reason.is_empty(), "错误载荷承载原因");
        }
        Ok(spec) => panic!("应拒收，实际 Ok: {spec:?}"),
    }
}

#[test]
fn 窗长候选千位口径_一百万一零四八五七六两值不等() {
    let spec = parse_model_spec("x[1m]").expect("后缀解析成功");
    assert_eq!(spec.length, Some(1_000_000), "1000 进制标称值");
    assert_ne!(
        spec.length,
        Some(1_048_576),
        "防实现误用 1024 基（与 128K 缺省启发式是两处独立口径）"
    );
    assert_eq!(
        MILLION_CONTEXT_WINDOW, 1_000_000,
        "常量为唯一取值来源（不落 1_048_576 字面）"
    );
}
