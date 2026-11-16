//! `blocks` 能力模块单测：密封层双向分派（choice 投影 / 事件块回重建载荷）。
//!
//! 装置约定同 `web_search_test`：纯函数 + 值对象，零 I/O / 零 async / 零 Mock。
//! 装置（raw wire 构造等）自 `web_search_test` 的既有形态平移，断言口径不变。

use agent::AgentBlock;
use rig::message::{
    AdditionalParams, AssistantContent, DocumentSourceKind, Image, Issuer, Reasoning, Text,
    ToolName, UserContent,
};
use serde_json::{json, Value};

use crate::sdk::blocks::{assistant_content, project_choice, user_content};
use crate::sdk::web_search::{
    replay_raw, replay_text, ANTHROPIC_RAW_CONTENT_KEY, CITATIONS_KEY, WEB_SEARCH_TOOL_NAME,
};

// ---------------------------------------------------------------------------
// 装置：raw wire 载荷构造与 Text 块构造（rig 类型真实构造，不 mock）
// ---------------------------------------------------------------------------

/// 携原始 wire 的文本块（raw server 块在 rig 面的承载形态：**空文本 + 原始值**）。
fn raw_block(raw: Value) -> AssistantContent {
    AssistantContent::Text(Text {
        text: String::new(),
        additional_params: AdditionalParams::from_entries([(ANTHROPIC_RAW_CONTENT_KEY, raw)]),
    })
}

/// 纯文本块（无 raw / 无引用）。
fn plain_text(text: &str) -> AssistantContent {
    AssistantContent::Text(Text::new(text))
}

/// 携 `additional_params` 的文本块。
fn text_with_entries(text: &str, entries: Vec<(&str, Value)>) -> Text {
    Text {
        text: text.to_owned(),
        additional_params: AdditionalParams::from_entries(entries),
    }
}

/// `web_search_result` 单条来源 wire。
fn source(url: &str, title: Option<&str>, page_age: Option<&str>) -> Value {
    let mut item = json!({ "type": "web_search_result", "url": url });
    if let Some(title) = title {
        item["title"] = json!(title);
    }
    if let Some(page_age) = page_age {
        item["page_age"] = json!(page_age);
    }
    item
}

/// 成功 result wire（`content` 为来源数组）。
fn result_raw(use_id: &str, sources: Vec<Value>) -> Value {
    json!({
        "type": "web_search_tool_result",
        "tool_use_id": use_id,
        "content": sources,
    })
}

/// 错误 result wire（`content` 为 error 对象）。
fn error_result_raw(use_id: &str, error_code: &str) -> Value {
    json!({
        "type": "web_search_tool_result",
        "tool_use_id": use_id,
        "content": { "type": "web_search_tool_result_error", "error_code": error_code },
    })
}

/// use wire（`server_tool_use`；`query` 为 `Some` 才挂 `input.query`）。
fn use_raw(use_id: &str, name: &str, query: Option<&str>) -> Value {
    let mut raw = json!({ "type": "server_tool_use", "id": use_id, "name": name });
    if let Some(query) = query {
        raw["input"] = json!({ "query": query });
    }
    raw
}

/// 三条来源的 result wire（展示字段三种缺失组合：全有 / 缺 title / 缺 page_age）。
fn three_sources(use_id: &str) -> Value {
    result_raw(
        use_id,
        vec![
            source(
                "https://a.example.com/1",
                Some("甲标题"),
                Some("2025-10-01"),
            ),
            source("https://b.example.com/2", None, Some("2025-09-30")),
            source("https://c.example.com/3", Some("丙标题"), None),
        ],
    )
}

/// 摘 `AssistantContent::Text`（其余变体 panic）。
fn as_text(content: &AssistantContent) -> &Text {
    match content {
        AssistantContent::Text(text) => text,
        other => panic!("应为 Text 变体，实际: {other:?}"),
    }
}

/// 摘 `additional_params` 的键值（缺席 panic）。
fn param_of<'a>(text: &'a Text, key: &str) -> &'a Value {
    text.additional_params
        .as_ref()
        .unwrap_or_else(|| panic!("additional_params 应在场"))
        .get(key)
        .unwrap_or_else(|| panic!("{key} 键应在场"))
}

// ---------------------------------------------------------------------------
// project_choice：use + result 收敛 / 展示字段 / 错误 / 孤儿
// ---------------------------------------------------------------------------

#[test]
fn project_choice_use与配对result收敛为恰一块且不进客户端工具面() {
    let raw_use = use_raw("srvtoolu_1", WEB_SEARCH_TOOL_NAME, Some("今日汇率"));
    let raw_result = three_sources("srvtoolu_1");
    let choice = vec![raw_block(raw_use.clone()), raw_block(raw_result.clone())];

    let (blocks, tool_calls) = project_choice(&choice);

    assert_eq!(blocks.len(), 1, "use + 配对 result 收敛为恰一块");
    assert_eq!(tool_calls.len(), 0, "server 块不进客户端工具面");
    let AgentBlock::WebSearch {
        id,
        query,
        results,
        error,
        raw_use: block_raw_use,
        raw_result: block_raw_result,
    } = &blocks[0]
    else {
        panic!("应为 WebSearch 块，实际: {:?}", blocks[0]);
    };
    assert_eq!(id, "srvtoolu_1", "id 取 server_tool_use.id");
    assert_eq!(query, "今日汇率");
    assert_eq!(results.len(), 3, "result 的三条来源并入该块");
    assert_eq!(error, &None, "成功 result 无错误码");
    assert_eq!(block_raw_use.as_ref(), Some(&raw_use), "use 原文原样承载");
    assert_eq!(
        block_raw_result.as_ref(),
        Some(&raw_result),
        "result 原文原样承载（result 侧不重复产块）"
    );
}

#[test]
fn project_choice展示字段逐条映射url_title_page_age且query缺席为空串() {
    let choice = vec![
        raw_block(use_raw(
            "srvtoolu_2",
            WEB_SEARCH_TOOL_NAME,
            Some("rust 生命周期"),
        )),
        raw_block(result_raw(
            "srvtoolu_2",
            vec![
                source("https://a.example.com/1", Some("甲"), Some("2025-10-01")),
                source("https://b.example.com/2", None, None),
            ],
        )),
    ];

    let (blocks, _) = project_choice(&choice);
    let AgentBlock::WebSearch { query, results, .. } = &blocks[0] else {
        panic!("应为 WebSearch 块");
    };
    assert_eq!(
        query, "rust 生命周期",
        "query 取自 server_tool_use.input.query"
    );
    assert_eq!(results[0].url, "https://a.example.com/1", "url 逐字");
    assert_eq!(results[0].title.as_deref(), Some("甲"));
    assert_eq!(results[0].page_age.as_deref(), Some("2025-10-01"));
    assert_eq!(results[1].title, None, "title 缺席 → None");
    assert_eq!(results[1].page_age, None, "page_age 缺席 → None");

    // input.query 缺席 → query 空串（不 panic）
    let no_query = vec![raw_block(use_raw("srvtoolu_3", WEB_SEARCH_TOOL_NAME, None))];
    let (blocks, _) = project_choice(&no_query);
    let AgentBlock::WebSearch { query, results, .. } = &blocks[0] else {
        panic!("应为 WebSearch 块");
    };
    assert_eq!(query, "", "input.query 缺席 → 空串");
    assert!(results.is_empty(), "无 result 时来源为空");
}

#[test]
fn project_choice错误result取error_code且仍收敛为一块() {
    let choice = vec![
        raw_block(use_raw(
            "srvtoolu_err",
            WEB_SEARCH_TOOL_NAME,
            Some("超限查询"),
        )),
        raw_block(error_result_raw("srvtoolu_err", "max_uses_exceeded")),
    ];

    let (blocks, tool_calls) = project_choice(&choice);

    assert_eq!(blocks.len(), 1, "配对 use 在场时仍恰一块（不额外产错误块）");
    assert!(tool_calls.is_empty());
    let AgentBlock::WebSearch { error, results, .. } = &blocks[0] else {
        panic!("应为 WebSearch 块");
    };
    assert_eq!(
        error.as_deref(),
        Some("max_uses_exceeded"),
        "错误码取自 error_code"
    );
    assert!(results.is_empty(), "错误 result 的来源为空");

    // 线格式落键 "results": []（D1 第四稿：键恒在，不再断言「不落键」）
    let value = serde_json::to_value(&blocks[0]).expect("序列化成功");
    assert_eq!(
        value.get("results"),
        Some(&json!([])),
        "空 results 落键且为空数组，实际: {value}"
    );
    assert_eq!(value["error"], json!("max_uses_exceeded"));
}

#[test]
fn project_choice孤儿result单独成块且缺tool_use_id的wild块跳过() {
    // 前序无配对 use 的 result
    let raw_result = result_raw(
        "srvtoolu_orphan",
        vec![source(
            "https://orphan.example.com/1",
            Some("孤儿来源"),
            None,
        )],
    );
    let choice = vec![raw_block(raw_result.clone())];

    let (blocks, _) = project_choice(&choice);
    assert_eq!(blocks.len(), 1, "孤儿 result 单独成块");
    let AgentBlock::WebSearch {
        id,
        query,
        results,
        raw_use,
        raw_result: block_raw_result,
        ..
    } = &blocks[0]
    else {
        panic!("应为 WebSearch 块");
    };
    assert_eq!(id, "srvtoolu_orphan", "id 取 tool_use_id");
    assert_eq!(query, "", "孤儿块 query 为空串");
    assert!(
        raw_use.is_none(),
        "raw_use 为 None（前端按 rawUse 判别孤儿）"
    );
    assert_eq!(block_raw_result.as_ref(), Some(&raw_result));
    assert_eq!(results.len(), 1, "来源照常映射");

    // 缺 tool_use_id 的 wild result → 无 id 可承载，跳过（不产块、不 panic）
    let wild = json!({ "type": "web_search_tool_result", "content": [] });
    let (blocks, tool_calls) = project_choice(&[raw_block(wild)]);
    assert!(blocks.is_empty(), "wild result 跳过");
    assert!(tool_calls.is_empty());

    // 配对块与孤儿块并存：同 id 的第二条 result 不入配对索引（`result_by_use`
    // 同 id 取首个）→ 无人消费，以孤儿形态（`raw_use: None`）成块；两块同 id
    // 语义靠 `rawUse` 判别（前端判据）
    let mixed = vec![
        raw_block(use_raw("srvtoolu_x", WEB_SEARCH_TOOL_NAME, Some("查询"))),
        raw_block(result_raw(
            "srvtoolu_x",
            vec![source("https://a/1", None, None)],
        )),
        raw_block(result_raw(
            "srvtoolu_x",
            vec![source("https://a/2", None, None)],
        )),
    ];
    let (blocks, _) = project_choice(&mixed);
    assert_eq!(
        blocks.len(),
        2,
        "首个 result 并入 use 块；同 id 的第二条 result 无配对 → 孤儿块"
    );
    let AgentBlock::WebSearch {
        raw_use,
        results,
        raw_result: paired_raw,
        ..
    } = &blocks[0]
    else {
        panic!("首块应为配对 WebSearch 块，实际: {:?}", blocks[0]);
    };
    assert!(raw_use.is_some(), "配对块 raw_use 有值");
    assert_eq!(
        paired_raw.as_ref(),
        Some(&result_raw(
            "srvtoolu_x",
            vec![source("https://a/1", None, None)],
        ))
    );
    assert_eq!(results.len(), 1, "配对侧来源取首个 result");
    assert_eq!(results[0].url, "https://a/1");
    let AgentBlock::WebSearch {
        id,
        raw_use,
        results,
        ..
    } = &blocks[1]
    else {
        panic!("次块应为孤儿 WebSearch 块，实际: {:?}", blocks[1]);
    };
    assert_eq!(
        id, "srvtoolu_x",
        "孤儿块 id 取 tool_use_id（与配对块同 id）"
    );
    assert!(raw_use.is_none(), "孤儿块 raw_use 为 None");
    assert_eq!(results[0].url, "https://a/2", "孤儿块来源照常映射");
}

#[test]
fn project_choice结果先于use时预索引仍配对且不另产块() {
    // 顺序为 result → use（同一 tool_use_id）：wire 正常序恒为 use → result，
    // 但 `pause_turn` 拆分后 result 可能先到，故投影的配对与「已消费」标记都在
    // **预索引期**落定（设计 D2），与遍历顺序无关。
    let result_wire = result_raw(
        "srvtoolu_rev",
        vec![source("https://rev.example.com/1", Some("倒序来源"), None)],
    );
    let choice = vec![
        raw_block(result_wire.clone()),
        raw_block(use_raw(
            "srvtoolu_rev",
            WEB_SEARCH_TOOL_NAME,
            Some("倒序查询"),
        )),
    ];

    let (blocks, tool_calls) = project_choice(&choice);

    // 恰一块：result 在预索引期即被标「已消费」→ 遍历到它时直接跳过，不另产块
    assert_eq!(
        blocks.len(),
        1,
        "倒序（result → use）仍恰一块 WebSearch，实际: {blocks:?}"
    );
    assert!(tool_calls.is_empty(), "server 块不进客户端工具面");
    let AgentBlock::WebSearch {
        id,
        query,
        results,
        error,
        raw_use,
        raw_result,
    } = &blocks[0]
    else {
        panic!("应为配对 WebSearch 块，实际: {:?}", blocks[0]);
    };
    assert_eq!(id, "srvtoolu_rev");
    assert_eq!(query, "倒序查询", "query 取自 use 侧 input");
    assert!(error.is_none(), "成功 result 不产错误码");
    assert_eq!(results.len(), 1, "results 取自预索引到的 result 原文");
    assert_eq!(results[0].url, "https://rev.example.com/1");
    assert_eq!(results[0].title.as_deref(), Some("倒序来源"));
    assert!(raw_use.is_some(), "use 原文在场（配对块形态）");
    assert_eq!(
        raw_result.as_ref(),
        Some(&result_wire),
        "result 原文并入 use 侧块（逐字节等价）"
    );

    // 位置取 use 的遍历位：result(0) 被消费跳过 → use(1) 成块，两侧正文保序
    let bordered = vec![
        plain_text("前置正文"),
        raw_block(result_wire.clone()),
        raw_block(use_raw(
            "srvtoolu_rev",
            WEB_SEARCH_TOOL_NAME,
            Some("倒序查询"),
        )),
        plain_text("后置正文"),
    ];
    let (blocks, _) = project_choice(&bordered);
    assert_eq!(
        blocks.len(),
        3,
        "两条正文 + 一块配对 WebSearch（result 位不产块），实际: {blocks:?}"
    );
    assert!(
        matches!(&blocks[0], AgentBlock::Text { text, .. } if text == "前置正文"),
        "前置正文保持首位"
    );
    let AgentBlock::WebSearch { id, raw_use, .. } = &blocks[1] else {
        panic!("配对块应落在 use 的遍历位，实际: {:?}", blocks[1]);
    };
    assert_eq!(id, "srvtoolu_rev", "配对块取 use 侧 id");
    assert!(
        raw_use.is_some(),
        "配对块落在 use 的遍历位（result 位不产块）"
    );
    assert!(
        matches!(&blocks[2], AgentBlock::Text { text, .. } if text == "后置正文"),
        "后置正文保持末位"
    );

    // `consumed` 按 result 下标生效：同 id 只索引首个 result（`or_insert`），
    // 故第二个同 id result 不在预置范围内、照孤儿规则成块；被消费的首个 result
    // 无论其遍历位在 use 之前或之后都不产块
    let duplicated = vec![
        raw_block(result_raw(
            "srvtoolu_dup",
            vec![source("https://dup.example.com/1", None, None)],
        )),
        raw_block(result_raw(
            "srvtoolu_dup",
            vec![source("https://dup.example.com/2", None, None)],
        )),
        raw_block(use_raw(
            "srvtoolu_dup",
            WEB_SEARCH_TOOL_NAME,
            Some("重复来源查询"),
        )),
    ];
    let (blocks, _) = project_choice(&duplicated);
    assert_eq!(
        blocks.len(),
        2,
        "被消费的首个 result 不成块 + 第二个同 id result 成孤儿块，实际: {blocks:?}"
    );
    let AgentBlock::WebSearch {
        raw_use, results, ..
    } = &blocks[0]
    else {
        panic!("首块应为孤儿 WebSearch 块，实际: {:?}", blocks[0]);
    };
    assert!(
        raw_use.is_none(),
        "第二个同 id result 无配对 → 孤儿形态（consumed 只标首个下标）"
    );
    assert_eq!(
        results[0].url, "https://dup.example.com/2",
        "孤儿块来源取未被消费的那条 result"
    );
    let AgentBlock::WebSearch {
        raw_use,
        raw_result,
        ..
    } = &blocks[1]
    else {
        panic!("次块应为配对 WebSearch 块，实际: {:?}", blocks[1]);
    };
    assert!(raw_use.is_some(), "use 遍历位产配对块");
    assert_eq!(
        raw_result.as_ref(),
        Some(&result_raw(
            "srvtoolu_dup",
            vec![source("https://dup.example.com/1", None, None)],
        )),
        "配对原文取预索引命中的首个 result"
    );
}

#[test]
fn project_choice未知server块与非websearch工具一律跳过且其余块相对顺序保持() {
    let choice = vec![
        plain_text("前置正文"),
        // 非 web_search 的 server_tool_use
        raw_block(use_raw("srvtoolu_code", "code_execution", Some("x"))),
        // server_tool_use 缺 name
        raw_block(json!({ "type": "server_tool_use", "id": "srvtoolu_noname" })),
        // server_tool_use 缺 id
        raw_block(json!({ "type": "server_tool_use", "name": "web_search" })),
        // 未知 server result 类型
        raw_block(json!({ "type": "code_execution_tool_result", "tool_use_id": "srvtoolu_code" })),
        // additional_params 携未知 raw type
        raw_block(json!({ "type": "mystery_server_block" })),
        plain_text("后置正文"),
    ];

    let (blocks, tool_calls) = project_choice(&choice);

    assert_eq!(blocks.len(), 2, "未知 server 块全部跳过，仅两条正文块");
    assert!(tool_calls.is_empty());
    assert!(
        matches!(&blocks[0], AgentBlock::Text { text, .. } if text == "前置正文"),
        "相对顺序保持（前）"
    );
    assert!(
        matches!(&blocks[1], AgentBlock::Text { text, .. } if text == "后置正文"),
        "相对顺序保持（后）"
    );
}

#[test]
fn project_choice既有分支不变_文本引用思考工具调用与图像() {
    let citation = json!({ "url": "https://cite.example.com/1", "title": "引用标题" });
    let choice = vec![
        plain_text("普通正文"),
        raw_block(use_raw("srvtoolu_keep", WEB_SEARCH_TOOL_NAME, Some("查询"))),
        AssistantContent::Text(text_with_entries(
            "携引用正文",
            vec![(CITATIONS_KEY, json!([citation]))],
        )),
        AssistantContent::Reasoning(Reasoning::new("先想一下").sealed(Issuer::from_static("test"))),
        AssistantContent::tool_call(
            "tu_1",
            ToolName::new("Bash").expect("工具名非空"),
            json!({ "command": "ls" }),
        ),
    ];

    let (blocks, tool_calls) = project_choice(&choice);

    // 无 raw 的 Text → Text{citations: None}
    assert!(
        matches!(&blocks[0], AgentBlock::Text { text, citations } if text == "普通正文" && citations.is_none()),
        "无引用文本块 citations 为 None，实际: {:?}",
        blocks[0]
    );
    // use 侧块（无配对 result → 来源空）
    assert!(matches!(&blocks[1], AgentBlock::WebSearch { .. }));
    // 携 citations 的 Text → Some(...)
    match &blocks[2] {
        AgentBlock::Text { text, citations } => {
            assert_eq!(text, "携引用正文");
            let citations = citations.as_ref().expect("citations 应在场");
            assert_eq!(citations.len(), 1);
            assert_eq!(citations[0].raw, citation);
        }
        other => panic!("应为 Text 块，实际: {other:?}"),
    }
    // Reasoning → Thinking（展示文本原样；无签名块凭证为空）
    assert!(
        matches!(&blocks[3], AgentBlock::Thinking { thinking, signature, redacted }
            if thinking == "先想一下" && signature.is_none() && redacted.is_none()),
        "Reasoning 投影为 Thinking，实际: {:?}",
        blocks[3]
    );
    // ToolCall → ToolUse 且同条进第二返回值
    assert!(
        matches!(&blocks[4], AgentBlock::ToolUse { id, name, .. } if id == "tu_1" && name == "Bash"),
        "ToolCall 投影为 ToolUse，实际: {:?}",
        blocks[4]
    );
    assert_eq!(tool_calls.len(), 1, "客户端工具调用进第二返回值");
    assert_eq!(tool_calls[0].id.to_string(), "tu_1");

    // Image → 不产块（现状）：真实构造该变体，与同 choice 的正文块共存验证
    // 「本变体零产出」而非「整体为空」
    let with_image = vec![
        plain_text("图像前后"),
        AssistantContent::Image(Image {
            data: DocumentSourceKind::url("https://img.example.com/chart.png"),
            media_type: None,
            detail: None,
            additional_params: None,
        }),
    ];
    let (image_blocks, image_calls) = project_choice(&with_image);
    assert_eq!(
        image_blocks.len(),
        1,
        "Image 变体零产出（仅正文块留下），实际: {image_blocks:?}"
    );
    assert!(
        matches!(&image_blocks[0], AgentBlock::Text { text, .. } if text == "图像前后"),
        "其余分支相对位置不变，实际: {image_blocks:?}"
    );
    assert!(image_calls.is_empty(), "Image 不进客户端工具面");
}

#[test]
fn project_choice容器边界_空切片单元素与百条混合块条数守恒() {
    // 空切片
    let (blocks, tool_calls) = project_choice(&[]);
    assert!(
        blocks.is_empty() && tool_calls.is_empty(),
        "空切片 → (vec![], vec![])"
    );

    // 单元素
    let (blocks, _) = project_choice(&[plain_text("唯一")]);
    assert_eq!(blocks.len(), 1);

    // 100 条混合块（Text / use / result / 未知 raw 交错）：条数守恒、无丢块、无重排
    let mut choice: Vec<AssistantContent> = Vec::new();
    let mut expected_texts: Vec<String> = Vec::new();
    for index in 0..25 {
        let text = format!("正文-{index}");
        choice.push(plain_text(&text));
        expected_texts.push(text);
        choice.push(raw_block(use_raw(
            &format!("srvtoolu_{index}"),
            WEB_SEARCH_TOOL_NAME,
            Some(&format!("查询-{index}")),
        )));
        choice.push(raw_block(result_raw(
            &format!("srvtoolu_{index}"),
            vec![source(&format!("https://s/{index}"), None, None)],
        )));
        choice.push(raw_block(json!({ "type": "unknown_raw_block" })));
    }
    assert_eq!(choice.len(), 100, "fixture 恰 100 条");

    let (blocks, tool_calls) = project_choice(&choice);
    assert_eq!(
        blocks.len(),
        50,
        "25 条正文 + 25 个收敛块（result / 未知块零产出）"
    );
    assert!(tool_calls.is_empty());
    for (index, expected) in expected_texts.iter().enumerate() {
        match &blocks[index * 2] {
            AgentBlock::Text { text, .. } => assert_eq!(text, expected, "正文块保序"),
            other => panic!("第 {} 块应为 Text，实际: {other:?}", index * 2),
        }
        match &blocks[index * 2 + 1] {
            AgentBlock::WebSearch { id, .. } => assert_eq!(id, &format!("srvtoolu_{index}")),
            other => panic!("收敛块保序，实际: {other:?}"),
        }
    }

    // use 与 result 之间夹 Text：收敛块位置取 use 的顺序位
    let interleaved = vec![
        raw_block(use_raw("srvtoolu_mid", WEB_SEARCH_TOOL_NAME, Some("夹心"))),
        plain_text("中间正文"),
        raw_block(result_raw("srvtoolu_mid", vec![])),
    ];
    let (blocks, _) = project_choice(&interleaved);
    assert_eq!(blocks.len(), 2, "夹心形态不额外产块");
    assert!(matches!(&blocks[0], AgentBlock::WebSearch { id, .. } if id == "srvtoolu_mid"));
    assert!(matches!(&blocks[1], AgentBlock::Text { text, .. } if text == "中间正文"));
}

// ---------------------------------------------------------------------------
// 组合：project → replay 往返语义不变
// ---------------------------------------------------------------------------

#[test]
fn project与replay往返后additional_params与原始wire逐字节等价且保原序() {
    let raw_use = json!({
        "type": "server_tool_use",
        "id": "srvtoolu_rt",
        "name": "web_search",
        "input": { "query": "往返查询" },
        "encrypted_content": "加密 🎉",
    });
    let raw_result = json!({
        "type": "web_search_tool_result",
        "tool_use_id": "srvtoolu_rt",
        "content": [{ "type": "web_search_result", "url": "https://rt/1", "title": "甲" }],
        "encrypted_content": "结果加密",
    });
    let citation = json!({ "url": "https://cite/1", "title": "引用", "cited_text": "片段" });

    // 原始 wire：use raw + result raw + 携 citations 的 text
    let choice = vec![
        raw_block(raw_use.clone()),
        raw_block(raw_result.clone()),
        AssistantContent::Text(text_with_entries(
            "带引用的正文",
            vec![(CITATIONS_KEY, json!([citation.clone()]))],
        )),
    ];

    let (blocks, _) = project_choice(&choice);
    assert_eq!(blocks.len(), 2, "use + result 收敛一块，正文一块");

    // 块 → 回放（raw_use / raw_result 交 replay_raw，citations 交 replay_text）
    let mut rebuilt: Vec<AssistantContent> = Vec::new();
    for block in &blocks {
        match block {
            AgentBlock::WebSearch {
                raw_use,
                raw_result,
                ..
            } => {
                if let Some(raw) = raw_use {
                    rebuilt.push(replay_raw(raw));
                }
                if let Some(raw) = raw_result {
                    rebuilt.push(replay_raw(raw));
                }
            }
            AgentBlock::Text { text, citations } => {
                rebuilt.push(replay_text(text, citations.as_deref()));
            }
            other => panic!("往返面不应出现其他块，实际: {other:?}"),
        }
    }

    assert_eq!(rebuilt.len(), 3, "use → result → text 三条（保原序）");
    assert_eq!(
        param_of(as_text(&rebuilt[0]), ANTHROPIC_RAW_CONTENT_KEY),
        &raw_use,
        "use 侧 raw 逐字节等价"
    );
    assert_eq!(
        param_of(as_text(&rebuilt[1]), ANTHROPIC_RAW_CONTENT_KEY),
        &raw_result,
        "result 侧 raw 逐字节等价"
    );
    assert_eq!(as_text(&rebuilt[2]).text, "带引用的正文");
    assert_eq!(
        param_of(as_text(&rebuilt[2]), CITATIONS_KEY),
        &json!([citation]),
        "citations 原值回填"
    );
}

// ---------------------------------------------------------------------------
// 反方向：事件块 → 重建载荷（`resume::rebuild` 的块映射面）
// ---------------------------------------------------------------------------

#[test]
fn assistant_content逐块展开且web_search产use到result两条() {
    let blocks = vec![
        AgentBlock::Text {
            text: "结论".to_owned(),
            citations: None,
        },
        AgentBlock::Thinking {
            thinking: "有凭证".to_owned(),
            signature: Some("sig-1".to_owned()),
            redacted: None,
        },
        AgentBlock::ToolUse {
            id: "tu_1".to_owned(),
            name: "read".to_owned(),
            input: json!({ "path": "a.txt" }),
        },
        AgentBlock::WebSearch {
            id: "srvtoolu_1".to_owned(),
            query: "查询".to_owned(),
            results: Vec::new(),
            error: None,
            raw_use: Some(use_raw("srvtoolu_1", WEB_SEARCH_TOOL_NAME, Some("查询"))),
            raw_result: Some(result_raw("srvtoolu_1", Vec::new())),
        },
    ];

    let content = assistant_content(&blocks);
    assert_eq!(
        content.len(),
        5,
        "Text + Thinking + ToolUse + use/result 两条"
    );
    assert!(matches!(&content[0], AssistantContent::Text(text) if text.text == "结论"));
    assert!(
        matches!(&content[1], AssistantContent::Reasoning(_)),
        "带凭证的思考块回放"
    );
    assert!(matches!(&content[2], AssistantContent::ToolCall(_)));
    // WebSearch 保原始 use → result 顺序，且两块的承载形态均为空文本 + raw
    assert_eq!(as_text(&content[3]).text, "", "use 侧空文本");
    assert_eq!(as_text(&content[4]).text, "", "result 侧空文本");
    assert_eq!(
        param_of(as_text(&content[3]), ANTHROPIC_RAW_CONTENT_KEY)["type"],
        json!("server_tool_use")
    );
}

#[test]
fn assistant_content跳过无凭证思考与非法工具名() {
    let blocks = vec![
        AgentBlock::Thinking {
            thinking: "只有摘要".to_owned(),
            signature: None,
            redacted: None,
        },
        AgentBlock::ToolUse {
            id: "tu_1".to_owned(),
            name: String::new(),
            input: json!({}),
        },
        AgentBlock::ToolResult {
            id: "tu_1".to_owned(),
            content: "结果".to_owned(),
            is_error: false,
        },
    ];

    assert!(
        assistant_content(&blocks).is_empty(),
        "无凭证思考 / 空工具名 / assistant 位结果一律不产载荷"
    );
}

#[test]
fn user_content只收文本块且忽略其余变体() {
    let blocks = vec![
        AgentBlock::Text {
            text: "提问".to_owned(),
            citations: None,
        },
        AgentBlock::Thinking {
            thinking: "越界形态".to_owned(),
            signature: Some("sig".to_owned()),
            redacted: None,
        },
        AgentBlock::ToolUse {
            id: "tu_1".to_owned(),
            name: "read".to_owned(),
            input: json!({}),
        },
    ];

    let content = user_content(&blocks);
    assert_eq!(content.len(), 1, "user 位只收文本块");
    assert!(matches!(&content[0], UserContent::Text(text) if text.text == "提问"));
}
