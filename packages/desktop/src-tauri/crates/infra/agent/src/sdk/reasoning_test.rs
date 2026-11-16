//! `reasoning` 能力模块单测：回放凭证 → rig reasoning、跨轮剔除与退化保护。
//!
//! 装置约定同 `web_search_test`：纯函数 + 值对象，零 I/O / 零 async / 零 Mock。

use rig::message::{AssistantContent, Issuer, Message, Reasoning, ReasoningContent, Text};

use crate::sdk::reasoning::{replay, strip_all, without_reasoning, ISSUER};

/// 展开回放载荷的可读内容（断言用）。
fn opened(content: &AssistantContent) -> Vec<ReasoningContent> {
    let AssistantContent::Reasoning(sealed) = content else {
        panic!("应为 reasoning，实际: {content:?}");
    };
    sealed
        .open(sealed.issuer())
        .expect("自身签发方可读")
        .content
        .clone()
}

fn text_with_signature(text: &str, signature: &str) -> AssistantContent {
    AssistantContent::Reasoning(
        Reasoning::new_with_signature(text, Some(signature.to_owned()))
            .sealed(Issuer::from_static(ISSUER)),
    )
}

#[test]
fn 签名块回放为文本与签名配对的reasoning() {
    let item = replay("先想一下", Some("sig-1"), None).expect("有签名即可回放");
    assert_eq!(
        opened(&item),
        vec![ReasoningContent::Text {
            text: "先想一下".to_owned(),
            signature: Some("sig-1".to_owned()),
        }]
    );
}

#[test]
fn 空文本带签名仍回放_显示省略不等于无思考() {
    let item = replay("", Some("sig-only"), None).expect("凭证在场即回放");
    assert_eq!(
        opened(&item),
        vec![ReasoningContent::Text {
            text: String::new(),
            signature: Some("sig-only".to_owned()),
        }],
        "空文本是正常形态（display omitted），凭证携带全量推理"
    );
}

#[test]
fn redacted块回放为密文载荷且优先于签名() {
    let item = replay("", None, Some("密文")).expect("redacted 即凭证");
    assert_eq!(
        opened(&item),
        vec![ReasoningContent::Redacted {
            data: "密文".to_owned(),
        }]
    );
    // 两凭证同时在场（Provider 不会这样发）：redacted 优先，不产双载荷
    let both = replay("文本", Some("sig-1"), Some("密文")).expect("有凭证");
    assert_eq!(
        opened(&both),
        vec![ReasoningContent::Redacted {
            data: "密文".to_owned(),
        }]
    );
}

#[test]
fn 无凭证的思考块不回放() {
    assert!(
        replay("只有摘要没有凭证", None, None).is_none(),
        "无凭证发裸文本只会被拒"
    );
}

#[test]
fn 剔除保留实体内容_滤空时返回_none() {
    let mixed = Message::Assistant {
        id: None,
        content: vec![
            text_with_signature("思考", "sig-1"),
            AssistantContent::Text(Text::new("正文")),
        ],
    };
    let kept = without_reasoning(mixed.clone()).expect("有实体内容");
    let Message::Assistant { content, .. } = kept else {
        panic!("应为 assistant 消息");
    };
    assert_eq!(content.len(), 1, "思考项被剔除: {content:?}");
    assert!(matches!(&content[0], AssistantContent::Text(text) if text.text == "正文"));

    // 仅思考的 assistant 消息：滤空 → None（调用方据此外退原样保留）
    let only = Message::Assistant {
        id: None,
        content: vec![text_with_signature("只有思考", "sig-1")],
    };
    assert!(without_reasoning(only).is_none(), "滤空即无实体内容");
}

#[test]
fn 非assistant消息原样返回() {
    let user = Message::user("提问");
    assert_eq!(without_reasoning(user.clone()), Some(user));
}

#[test]
fn 跨轮剔除后仅思考消息原样保留_防空消息() {
    let history = vec![
        Message::user("第一问"),
        Message::Assistant {
            id: None,
            content: vec![
                text_with_signature("思考一", "sig-1"),
                AssistantContent::Text(Text::new("答一")),
            ],
        },
        Message::user("第二问"),
        Message::Assistant {
            id: None,
            content: vec![text_with_signature("只有思考", "sig-2")],
        },
    ];
    let stripped = strip_all(history.clone());
    assert_eq!(stripped.len(), 4, "消息条数不变（无整条丢弃）");
    let Message::Assistant { content, .. } = &stripped[1] else {
        panic!("应为 assistant 消息");
    };
    assert_eq!(content.len(), 1, "有实体内容时思考被剔除");
    assert_eq!(
        &stripped[3], &history[3],
        "仅思考的 assistant 消息原样保留（否则成空消息被拒）"
    );
}
