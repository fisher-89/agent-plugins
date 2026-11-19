//! 模型目录单测：两级结构 + 五条不变量（AC-7 / AC-8 目录侧半边）。
//!
//! 装置约定（见 test-design `Mock策略`）：**无 Mock**——目录是编译期字面量，
//! 零 IO / 零 DB / 零网络，断言只读 `catalog()` 的返回值。
//!
//! 五条不变量：①（唯一 + 非空）②（模型级 effort 覆盖语义）③（零凭据 / 零端
//! 点）④（`[1m]` 条目窗长 = 1M）⑤（`supports_thinking == false` ⇒ 显式空
//! `efforts`）。名单本身会过时是知情代价，故断言只锚约束、不锚具体条目名。

use serde_json::Value;

use store::AgentProviderProtocol;

use crate::model_catalog::{catalog, CatalogModel, CatalogProtocol};

// ---------------------------------------------------------------------------
// 结构面：两级目录非空 + 两协议在场
// ---------------------------------------------------------------------------

#[test]
fn 目录两级结构非空且两协议皆在场() {
    let root = catalog();
    assert!(!root.protocols.is_empty(), "目录根 protocols 非空");

    let protocols: Vec<AgentProviderProtocol> =
        root.protocols.iter().map(|entry| entry.protocol).collect();
    for expected in [
        AgentProviderProtocol::Openai,
        AgentProviderProtocol::Anthropic,
    ] {
        assert!(
            protocols.contains(&expected),
            "协议段 {expected:?} 应在场，实际: {protocols:?}"
        );
    }
    // 同一协议不重复成段（否则前端 find 命中的段取决于顺序）
    let mut deduped = protocols.clone();
    deduped.dedup();
    assert_eq!(protocols, deduped, "协议段不重复");
}

#[test]
fn 每条协议模型清单与协议级effort建议均非空() {
    for entry in catalog().protocols {
        let label = entry.protocol;
        assert!(!entry.models.is_empty(), "{label:?} 段 models 非空");
        assert!(
            !entry.efforts.is_empty(),
            "{label:?} 段协议级 efforts 建议非空（不变量①后段）"
        );
        for effort in &entry.efforts {
            assert!(
                !effort.trim().is_empty(),
                "{label:?} 段协议级建议不含空白项，实际: {effort:?}"
            );
        }
    }
}

#[test]
fn 同协议内模型原文无重复() {
    for entry in catalog().protocols {
        let values: Vec<&str> = entry
            .models
            .iter()
            .map(|model| model.value.as_str())
            .collect();
        let mut deduped = values.clone();
        deduped.dedup();
        assert_eq!(
            values, deduped,
            "{:?} 段模型原文无重复（不变量①）",
            entry.protocol
        );
        assert!(
            values.iter().all(|value| !value.trim().is_empty()),
            "{:?} 段不含空模型原文",
            entry.protocol
        );
    }
}

// ---------------------------------------------------------------------------
// 不变量④：`[1m]` 条目窗长恒 1M
// ---------------------------------------------------------------------------

#[test]
fn 后缀1m条目窗长恒为一百万且命中集非空() {
    let hits: Vec<CatalogModel> = catalog()
        .protocols
        .into_iter()
        .flat_map(|entry| entry.models)
        .filter(|model| model.value.ends_with("[1m]"))
        .collect();

    assert!(
        !hits.is_empty(),
        "首版目录含 [1m] 条目（筛选集为空即用例前置失败）"
    );
    for model in hits {
        assert_eq!(
            model.context_length,
            Some(1_000_000),
            "{} 的窗长恒 1M（目录内不自相矛盾，不变量④）",
            model.value
        );
    }
}

// ---------------------------------------------------------------------------
// 不变量⑤ / ②：`supports_thinking` 与模型级 effort 覆盖语义
// ---------------------------------------------------------------------------

#[test]
fn 不支持思考的条目effort为显式空清单() {
    let mut hits = 0;
    for entry in catalog().protocols {
        for model in &entry.models {
            if model.supports_thinking {
                continue;
            }
            hits += 1;
            assert_eq!(
                model.efforts,
                Some(Vec::new()),
                "{} supports_thinking=false ⇒ 显式空 efforts（不靠继承协议级，不变量⑤）",
                model.value
            );
        }
    }
    assert!(
        hits > 0,
        "首版目录含不支持思考的条目（筛选集为空即前置失败）"
    );
}

#[test]
fn 支持思考的条目efforts为none或显式非空清单() {
    for entry in catalog().protocols {
        for model in &entry.models {
            if !model.supports_thinking {
                continue;
            }
            match &model.efforts {
                None => {
                    // None = 继承协议级（语义面：前端回落协议级建议）
                }
                Some(list) => {
                    assert!(
                        !list.is_empty(),
                        "{} 支持思考时模型级覆盖不得为空清单（空清单只表「无建议」，不变量②）",
                        model.value
                    );
                }
            }
        }
    }
}

#[test]
fn 模型级覆盖清单是协议级建议的真子集语义面() {
    for entry in catalog().protocols {
        for model in &entry.models {
            let Some(list) = &model.efforts else {
                continue;
            };
            for effort in list {
                assert!(
                    !effort.trim().is_empty(),
                    "{} 的模型级建议不含空白项",
                    model.value
                );
            }
            assert_eq!(
                entry
                    .models
                    .iter()
                    .filter(|row| row.value == model.value)
                    .count(),
                1,
                "覆盖语义按 (协议, 模型原文) 单点寻址：模型原文在本段内唯一"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 不变量③：零凭据 / 零端点
// ---------------------------------------------------------------------------

#[test]
fn 目录零凭据零端点() {
    let text = serde_json::to_string(&catalog()).expect("目录可序列化");
    for forbidden in [
        "api_key", "apiKey", "base_url", "baseUrl", "sk-", "http://", "https://",
    ] {
        assert!(
            !text.contains(forbidden),
            "目录全文不含 {forbidden}（不变量③：纯静态参考数据）"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界面：元数据取值域 / 新构造 / 线值枚举复用
// ---------------------------------------------------------------------------

#[test]
fn 条目窗长与输出上限为none或正数() {
    for entry in catalog().protocols {
        for model in &entry.models {
            for (label, value) in [
                ("context_length", model.context_length),
                ("max_tokens", model.max_tokens),
            ] {
                if let Some(tokens) = value {
                    assert!(tokens > 0, "{} 的 {label} 不落 0 字面值", model.value);
                }
            }
        }
    }
}

#[test]
fn 每次调用返回新构造对象且两次结果相等() {
    let first = catalog();
    let second = catalog();
    assert_eq!(first, second, "两次调用值相等（内容同源）");
    // 纯函数无共享态：改动第一份副本不影响第二份（非 OnceLock 共享可变边界）
    let mut mutated: CatalogProtocol = first.protocols[0].clone();
    mutated.efforts.clear();
    assert!(
        !second.protocols[0].efforts.is_empty(),
        "两份产物互不共享（每次调用新构造）"
    );
}

#[test]
fn 协议字段复用store线值枚举序列化逐字为openai与anthropic() {
    for entry in catalog().protocols {
        let value: Value = serde_json::to_value(entry.protocol).expect("协议可序列化");
        let expected = match entry.protocol {
            AgentProviderProtocol::Openai => "openai",
            AgentProviderProtocol::Anthropic => "anthropic",
        };
        assert_eq!(
            value,
            Value::String(expected.to_owned()),
            "协议线值与 store 枚举逐字一致（零平行枚举）"
        );
    }
}
