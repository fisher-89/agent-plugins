//! `effort` shaping 单点单测：三常量键面 + 两协议 wire 形状 + 留空零痕迹 +
//! 零值域校验（AC-4 / AC-5 引擎侧半边）。
//!
//! 装置约定（见 test-design `Mock策略`）：**无 Mock**——纯函数零 IO、零环境依
//! 赖，直接以 `Protocol` 变体与字符串入参驱动；`serde_json::Value` 仅作断言对
//! 象。`Protocol` 是封闭两变体枚举（无法构造非法变体），故以「两臂穷举 + 每
//! 臂正 / 反 / 边界三类」承担 N+1 的等价覆盖。

use serde_json::json;

use crate::sdk::config::Protocol;
use crate::sdk::effort::{
    params, ANTHROPIC_EFFORT_KEY, ANTHROPIC_OUTPUT_CONFIG_KEY, OPENAI_EFFORT_KEY,
};

/// 两协议穷举（封闭枚举的值域全覆盖）。
const PROTOCOLS: [Protocol; 2] = [Protocol::Openai, Protocol::Anthropic];

// ---------------------------------------------------------------------------
// 正向：两臂 wire 形状
// ---------------------------------------------------------------------------

#[test]
fn openai臂载荷恰为reasoning_effort单键() {
    let payload = params(Protocol::Openai, "high").expect("非空 effort 应成形");

    assert_eq!(
        payload,
        json!({ "reasoning_effort": "high" }),
        "openai 臂载荷为 {{reasoning_effort: v}}"
    );
    let object = payload.as_object().expect("顶层为对象");
    assert_eq!(object.len(), 1, "顶层键数恰 1（无多余键）");
    assert!(
        !object.contains_key(ANTHROPIC_OUTPUT_CONFIG_KEY),
        "openai 臂不带 anthropic 外层键，实际: {payload}"
    );
}

#[test]
fn anthropic臂载荷恰为output_config套effort且内外各一键() {
    let payload = params(Protocol::Anthropic, "high").expect("非空 effort 应成形");

    assert_eq!(
        payload,
        json!({ "output_config": { "effort": "high" } }),
        "anthropic 臂载荷为 {{output_config: {{effort: v}}}}"
    );
    let outer = payload.as_object().expect("外层为对象");
    assert_eq!(outer.len(), 1, "外层键数恰 1");
    let inner = outer[ANTHROPIC_OUTPUT_CONFIG_KEY]
        .as_object()
        .expect("内层为对象");
    assert_eq!(inner.len(), 1, "内层键数恰 1");
    assert!(
        !outer.contains_key(OPENAI_EFFORT_KEY),
        "anthropic 臂不带 openai 平铺键，实际: {payload}"
    );
}

// ---------------------------------------------------------------------------
// 边界：留空零痕迹（空串 / 纯空白 ⇒ None）
// ---------------------------------------------------------------------------

#[test]
fn 空串effort两协议均返回none() {
    for protocol in PROTOCOLS {
        assert_eq!(
            params(protocol, ""),
            None,
            "{protocol:?} 臂空串 ⇒ None（留空 = 不发，不落空对象）"
        );
    }
}

#[test]
fn 纯空白effort两协议均返回none() {
    // 半角空格 / 制表换行 / 全角空格三类空白形态
    for effort in ["   ", "\t\n", "\u{3000}"] {
        for protocol in PROTOCOLS {
            assert_eq!(
                params(protocol, effort),
                None,
                "{protocol:?} 臂 effort={effort:?} 裁剪后为空 ⇒ None"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 边界：裁剪与透传（调用方零预处理）
// ---------------------------------------------------------------------------

#[test]
fn 两侧空白被裁剪后载荷值取裁剪结果() {
    assert_eq!(
        params(Protocol::Openai, "  low  "),
        Some(json!({ "reasoning_effort": "low" })),
        "openai 臂载荷值 = 裁剪后原值（调用方零预处理）"
    );
    assert_eq!(
        params(Protocol::Anthropic, "\tmax\n"),
        Some(json!({ "output_config": { "effort": "max" } })),
        "anthropic 臂同口径裁剪"
    );
}

#[test]
fn 超长值原样透传不截断() {
    let long = "e".repeat(1_500);
    for protocol in PROTOCOLS {
        let payload = params(protocol, &long).expect("超长非空值应成形");
        let carried = match protocol {
            Protocol::Openai => payload[OPENAI_EFFORT_KEY].clone(),
            Protocol::Anthropic => {
                payload[ANTHROPIC_OUTPUT_CONFIG_KEY][ANTHROPIC_EFFORT_KEY].clone()
            }
        };
        assert_eq!(
            carried,
            json!(long),
            "{protocol:?} 臂超长值原样透传（不截断）"
        );
    }
}

#[test]
fn 特殊字符只裁两侧内部形态原样透传() {
    for effort in ["a\nb", "thinking 🎉", "a b"] {
        assert_eq!(
            params(Protocol::Openai, effort),
            Some(json!({ "reasoning_effort": effort })),
            "内部换行 / emoji / 中间空格原样承载（只裁两侧）：{effort:?}"
        );
        assert_eq!(
            params(Protocol::Anthropic, effort),
            Some(json!({ "output_config": { "effort": effort } })),
            "anthropic 臂同口径：{effort:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 异常：目录外取值（零值域校验、零大小写归一）
// ---------------------------------------------------------------------------

#[test]
fn 目录外取值原样透传零值域校验() {
    for effort in ["ultra-thinking", "none", "MAX"] {
        assert_eq!(
            params(Protocol::Openai, effort),
            Some(json!({ "reasoning_effort": effort })),
            "目录外取值 {effort:?} 原样透传（保存期只 trim、shaping 零值域校验）"
        );
        assert_eq!(
            params(Protocol::Anthropic, effort),
            Some(json!({ "output_config": { "effort": effort } })),
            "「MAX」不被归一为「max」（零大小写归一）"
        );
    }
}

// ---------------------------------------------------------------------------
// 常量面与纯函数性
// ---------------------------------------------------------------------------

#[test]
fn 键名常量逐字锚定且为载荷唯一键源() {
    assert_eq!(OPENAI_EFFORT_KEY, "reasoning_effort");
    assert_eq!(ANTHROPIC_OUTPUT_CONFIG_KEY, "output_config");
    assert_eq!(ANTHROPIC_EFFORT_KEY, "effort");
    assert_ne!(
        OPENAI_EFFORT_KEY, ANTHROPIC_OUTPUT_CONFIG_KEY,
        "两协议外层键不同名（否则门控无法按键区分）"
    );

    // 键名单点：载荷键名与常量逐字对齐（`Map` 构造使常量成为唯一键源）
    let openai = params(Protocol::Openai, "v").expect("成形");
    assert!(openai.get(OPENAI_EFFORT_KEY).is_some());
    let anthropic = params(Protocol::Anthropic, "v").expect("成形");
    let inner = anthropic
        .get(ANTHROPIC_OUTPUT_CONFIG_KEY)
        .and_then(|value| value.get(ANTHROPIC_EFFORT_KEY));
    assert!(inner.is_some(), "anthropic 内层键名与常量对齐");
}

#[test]
fn 纯函数性_同入参连续两次调用结果相等() {
    for protocol in PROTOCOLS {
        let first = params(protocol, "  high  ");
        let second = params(protocol, "  high  ");
        assert_eq!(
            first, second,
            "{protocol:?} 臂无状态、零 IO（无全局态漂移）"
        );
    }
}
