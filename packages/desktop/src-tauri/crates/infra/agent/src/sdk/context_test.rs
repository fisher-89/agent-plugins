use rig::message::{AssistantContent, Message, ToolResultContent, UserContent};
use serde_json::json;

use crate::sdk::context::{estimate_history, hard_prune, protected_start, prune, ContextDefense};

// ---------------------------------------------------------------------------
// 装置：防线底座、消息构造、精确 tokens 构造（自校准）、史抽取
// ---------------------------------------------------------------------------

/// L2 触发水位 tokens（窗长 × 3/4，与实现推导式同源）。
fn l2_threshold_of(window: u64) -> u64 {
    window * 3 / 4
}

/// 小窗防线（window = 64 → L2 水位 48 tokens）：少 tokens 即触线的测试底座。
fn defense_small() -> ContextDefense {
    ContextDefense::resolve(Some(64))
}

/// user 消息的序列化常数开销（信封底数），供精确 tokens 构造。
fn user_overhead() -> usize {
    let probe = Message::user("a".repeat(1000));
    serde_json::to_vec(&probe).expect("probe 序列化成功").len() - 1000
}

/// 构造估算恰为 `tokens` 的 user 消息（序列化字节 = 4 × tokens，÷4 无余数，
/// 自校准不依赖 wire 格式常数）。
fn user_at_tokens(tokens: u64) -> Message {
    let body = "a".repeat(tokens as usize * 4 - user_overhead());
    Message::user(body)
}

/// tool_result 正文的序列化常数开销（仅载荷不含信封，与估算口径同面）。
fn tool_result_content_overhead() -> usize {
    let probe = vec![ToolResultContent::text("a".repeat(1000))];
    serde_json::to_vec(&probe).expect("probe 序列化成功").len() - 1000
}

/// 构造正文估算恰为 `tokens` 的 tool_result 消息（单体门槛的精确边界基座）。
fn tool_result_at_tokens(call: &str, tokens: u64) -> Message {
    let body = "a".repeat(tokens as usize * 4 - tool_result_content_overhead());
    Message::tool_result(call, "read", body)
}

/// 携单枚工具调用的 assistant 消息（tool_use 半边）。
fn assistant_with_call(id: &str) -> Message {
    Message::Assistant {
        id: None,
        content: vec![AssistantContent::tool_call(
            id,
            "read",
            json!({ "path": "a.txt" }),
        )],
    }
}

/// 摘史内指定 call id 的 tool_result 正文（首个文本块）。
fn tool_result_text(history: &[Message], call: &str) -> Option<String> {
    history.iter().find_map(|message| match message {
        Message::User { content } => content.iter().find_map(|item| match item {
            UserContent::ToolResult(result) if result.call.as_str() == call => {
                match &result.content[0] {
                    ToolResultContent::Text(text) => Some(text.text.clone()),
                    _ => None,
                }
            }
            _ => None,
        }),
        _ => None,
    })
}

/// 摘史内全部 tool_use call id（assistant 半边）。
fn tool_call_ids(history: &[Message]) -> Vec<String> {
    history
        .iter()
        .flat_map(|message| match message {
            Message::Assistant { content, .. } => content
                .iter()
                .filter_map(|item| match item {
                    AssistantContent::ToolCall(call) => Some(call.id.as_str().to_owned()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

/// 史内是否残留任何 tool_result 条目（孤儿判定）。
fn has_any_tool_result(history: &[Message]) -> bool {
    history.iter().any(|message| {
        matches!(message, Message::User { content }
        if content.iter().any(|item| matches!(item, UserContent::ToolResult(_))))
    })
}

// ---------------------------------------------------------------------------
// resolve：窗长解析与水位推导（AC-9 消费半边）
// ---------------------------------------------------------------------------

#[test]
fn resolve显式窗长承接且水位按比例推导() {
    let defense = ContextDefense::resolve(Some(200_000));

    // L2 水位 = 75%（150_000）：恰水位不过线（严格大于），超 1 过线
    assert_eq!(
        estimate_history(&[user_at_tokens(150_000)]),
        150_000,
        "精确 tokens 构造自洽（自校准基座）"
    );
    assert!(
        !defense.over_l2(&[user_at_tokens(150_000)]),
        "恰 75% 水位不过线"
    );
    assert!(
        defense.over_l2(&[user_at_tokens(150_001)]),
        "超 1 过线（阈值推导 0.75 承接）"
    );

    // L3 水位 = 90%（180_000）：恰水位不过线，超 1 过线
    assert!(
        !defense.over_l3(&[user_at_tokens(180_000)]),
        "恰 90% 水位不过线"
    );
    assert!(
        defense.over_l3(&[user_at_tokens(180_001)]),
        "超 1 过线（阈值推导 0.90 承接）"
    );
}

#[test]
fn resolve缺席走128k缺省启发式() {
    let defense = ContextDefense::resolve(None);

    // 缺省窗 128 * 1024 → L2 水位 98_304：边界两侧精准判别
    let boundary = l2_threshold_of(128 * 1024);
    assert_eq!(boundary, 98_304, "128K 缺省窗的 L2 水位推导");
    assert!(
        !defense.over_l2(&[user_at_tokens(boundary)]),
        "恰水位不过线"
    );
    assert!(
        defense.over_l2(&[user_at_tokens(boundary + 1)]),
        "超 1 过线（缺席走 128K 启发式，非 0 非其它字面）"
    );
}

#[test]
fn resolve退化窗长落入缺省且防线不塌缩为空史() {
    // Some(0) 非法退化：与 None 同走 128K 缺省（水位判别一致）
    let degenerate = ContextDefense::resolve(Some(0));
    let default = ContextDefense::resolve(None);
    let boundary = l2_threshold_of(128 * 1024);
    assert_eq!(
        degenerate.over_l2(&[user_at_tokens(boundary + 1)]),
        default.over_l2(&[user_at_tokens(boundary + 1)]),
        "退化窗长与缺席同口径（缺省启发式承接）"
    );

    // 防线下界不塌缩：退化窗长下 prune / hard_prune 仍保首条 user + 保护窗
    // （filler 抬至 150k tokens，越过缺省 128K 的 L2 水位 98_304）
    let history = vec![
        Message::user("任务书"),
        assistant_with_call("tu_big"),
        tool_result_at_tokens("tu_big", 25_000),
        user_at_tokens(150_000),
    ];
    let (pruned, notices) = prune(history.clone(), &degenerate);
    assert!(
        matches!(pruned.first(), Some(message) if *message == Message::user("任务书")),
        "prune 恒保首条 user"
    );
    assert!(!pruned.is_empty(), "prune 产物非空史");
    assert!(!notices.is_empty(), "超水位史在退化窗长下仍触线剪裁");

    let (hard, notice) = hard_prune(history, &degenerate);
    assert!(
        matches!(hard.first(), Some(message) if *message == Message::user("任务书")),
        "hard_prune 恒保首条 user"
    );
    assert_eq!(notice.subtype, "context_compacted", "硬裁 notice 词汇");
}

// ---------------------------------------------------------------------------
// estimate_history：字节启发式
// ---------------------------------------------------------------------------

#[test]
fn estimate_history空史为零() {
    assert_eq!(estimate_history(&[]), 0, "空史估算 0");
}

#[test]
fn estimate_history字节启发式与手工口径一致且中文emoji多字节计入() {
    // ASCII 单条：serde_json 字节数 ÷ 4 向上取整（测试内手工同口径复算）
    let ascii = Message::user("plain ascii body");
    let ascii_bytes = serde_json::to_vec(&ascii).expect("序列化成功").len() as u64;
    assert_eq!(
        estimate_history(&[ascii]),
        ascii_bytes.div_ceil(4),
        "手工口径一致（bytes/4 向上取整）"
    );

    // 中文 / emoji 多字节：UTF-8 字节计入（高估方向，宁早裁不触 L4）
    let cjk = Message::user("中文正文 🎉🚀");
    let cjk_bytes = serde_json::to_vec(&cjk).expect("序列化成功").len() as u64;
    assert!(
        cjk_bytes > "中文正文 🎉🚀".chars().count() as u64,
        "多字节内容按 UTF-8 字节计（非字符数）"
    );
    assert_eq!(
        estimate_history(&[cjk]),
        cjk_bytes.div_ceil(4),
        "中文 / emoji 行内容保真计入"
    );

    // 高估方向：序列化字节（含结构开销）≥ 纯正文字节
    let body = "a".repeat(100);
    let message = Message::user(&body);
    assert!(
        estimate_history(&[message]) * 4 >= 100,
        "序列化含结构开销 = 保守高估"
    );
}

#[test]
fn estimate_history多条线性求和无跨条压缩或常数底() {
    let first = Message::user("第一条");
    let second = assistant_with_call("tu_1");
    let third = Message::tool_result("tu_1", "read", "工具产物");

    let sum = estimate_history(&[first.clone(), second.clone(), third.clone()]);
    assert_eq!(
        sum,
        estimate_history(&[first]) + estimate_history(&[second]) + estimate_history(&[third]),
        "多条估算 = 各条之和（线性求和）"
    );
}

// ---------------------------------------------------------------------------
// prune：占位符替换（配对完整）
// ---------------------------------------------------------------------------

/// 超水位史基座：[首条 user, assistant(tool_use), 老超大 tool_result, 尾部
/// 大块 filler]。filler 使尾部越过 40k 保护窗 → 老工具结果落入可裁窗外。
fn oversized_history(call: &str, tool_tokens: u64) -> Vec<Message> {
    vec![
        Message::user("任务书"),
        assistant_with_call(call),
        tool_result_at_tokens(call, tool_tokens),
        user_at_tokens(50_000),
    ]
}

#[test]
fn prune老工具结果占位符替换且tool_use配对完整() {
    let history = oversized_history("tu_big", 25_000);
    let (pruned, notices) = prune(history, &defense_small());

    // 占位符替换：正文让位、配对结构完整（call / name 保留）
    let placeholder = tool_result_text(&pruned, "tu_big").expect("tool_result 仍在史内");
    assert!(
        placeholder.contains("[已剪裁]"),
        "老 tool_result 替换为占位符: {placeholder}"
    );
    assert!(
        placeholder.contains("store 转录全量保留"),
        "占位符留痕指向 store 全量面: {placeholder}"
    );
    assert!(!placeholder.contains('a'), "原正文已从请求史移除");
    assert_eq!(
        tool_call_ids(&pruned),
        vec!["tu_big".to_owned()],
        "tool_use 半边保留（配对结构完整，无孤儿 tool_use）"
    );

    // 首条 user 与保护窗 filler 原样
    assert_eq!(pruned[0], Message::user("任务书"), "首条 user 恒在场");
    assert_eq!(pruned.len(), 4, "占位符替换不改消息数（原位替换）");

    // 触发剪裁即产出恰一条 notice
    assert_eq!(notices.len(), 1, "有裁才有 notice");
}

#[test]
fn prune单体门槛恰边界_恰20k不裁超1即裁() {
    // 恰 20k tokens（20480）：不越门槛 → 不裁（历史原样、零 notice）
    let at_threshold = oversized_history("tu_edge", 20_480);
    let at_threshold_estimate = estimate_history(&at_threshold);
    let (unchanged, notices) = prune(at_threshold.clone(), &defense_small());
    assert!(
        tool_result_text(&unchanged, "tu_edge").is_some_and(|text| text.contains('a')),
        "恰门槛 tool_result 正文原样保留（不裁）"
    );
    assert_eq!(
        estimate_history(&unchanged),
        at_threshold_estimate,
        "恰门槛零改动"
    );
    assert!(
        notices.is_empty(),
        "无裁零 notice（after == before 不产痕）"
    );

    // 超 1：越门槛 → 占位符替换（恰阈值过 / 超 1 截断）
    let over = oversized_history("tu_edge", 20_481);
    let (pruned, notices) = prune(over, &defense_small());
    let placeholder = tool_result_text(&pruned, "tu_edge").expect("tool_result 仍在");
    assert!(
        placeholder.contains("[已剪裁]"),
        "超门槛 1 token 即触发占位符替换: {placeholder}"
    );
    assert_eq!(notices.len(), 1, "替换产出 notice");
}

#[test]
fn prune保护窗内超门槛工具结果不裁() {
    // 超门槛 tool_result 位于史尾（最近 40k 保护窗内）→ 只裁窗外，窗内不裁
    let history = vec![
        Message::user("任务书"),
        assistant_with_call("tu_recent"),
        tool_result_at_tokens("tu_recent", 25_000),
    ];
    assert_eq!(
        protected_start(&history, &defense_small()),
        0,
        "基座前置：全史落在保护窗内（尾部反推未越 40k）"
    );

    let (pruned, notices) = prune(history, &defense_small());

    let kept = tool_result_text(&pruned, "tu_recent").expect("tool_result 保留");
    assert!(
        kept.contains('a') && !kept.contains("[已剪裁]"),
        "保护窗内超门槛 tool_result 不裁（只裁窗外）"
    );
    assert!(notices.is_empty(), "零裁剪零 notice");
}

#[test]
fn prune占位不够时丢最老完整轮对且次老保留() {
    // [首条 user, 老轮对(assistant+超大 tool_result), 大块 filler, 次老 assistant]
    // → 占位替换后估算仍超水位 → 最老完整轮对整段移除，次老轮对保留
    let history = vec![
        Message::user("任务书"),
        assistant_with_call("tu_old"),
        tool_result_at_tokens("tu_old", 25_000),
        user_at_tokens(50_000),
        Message::assistant("次老轮对的回应"),
    ];
    assert_eq!(
        protected_start(&history, &defense_small()),
        4,
        "保护窗圈定史尾"
    );

    let (pruned, notices) = prune(history, &defense_small());

    // 最老完整轮对（assistant 起至下一 assistant 前的完整交换段）整段移除
    assert_eq!(
        tool_result_text(&pruned, "tu_old"),
        None,
        "老轮对 tool_result 随段移除"
    );
    assert!(
        !tool_call_ids(&pruned).contains(&"tu_old".to_owned()),
        "老轮对 tool_use 随段移除"
    );
    // 次老轮对保留 + 首条 user 保全
    assert_eq!(pruned[0], Message::user("任务书"), "首条 user 恒在场");
    assert_eq!(
        pruned.last(),
        Some(&Message::assistant("次老轮对的回应")),
        "次老轮对保留"
    );
    assert_eq!(notices.len(), 1, "轮对移除产出 notice");
}

#[test]
fn prune任意剪裁强度下首条user恒在场() {
    // 强度一：占位符替换
    let light = oversized_history("tu_a", 25_000);
    let (pruned_light, _) = prune(light.clone(), &defense_small());
    assert_eq!(pruned_light[0], light[0], "占位符强度下首条 user 原样");

    // 强度二：轮对整段移除
    let heavy = vec![
        Message::user("任务书 🎉"),
        assistant_with_call("tu_b"),
        tool_result_at_tokens("tu_b", 25_000),
        user_at_tokens(50_000),
        Message::assistant("尾巴"),
    ];
    let (pruned_heavy, _) = prune(heavy.clone(), &defense_small());
    assert_eq!(pruned_heavy[0], heavy[0], "轮对移除强度下首条 user 原样");

    // 强度三：硬裁
    let (hard, _) = hard_prune(heavy, &defense_small());
    assert_eq!(
        hard.first(),
        Some(&Message::user("任务书 🎉")),
        "硬裁强度下首条 user 原样（phase agent 任务书保全）"
    );
}

#[test]
fn prune轮对裁剪产生的孤儿tool_result被配对清扫() {
    // 轮对移除后遗留无伙伴的 tool_result（其 tool_use 所在 assistant 已被裁）
    // → 配对清扫，无悬空 id
    let history = vec![
        Message::user("任务书"),
        tool_result_at_tokens("tu_orphan", 500),
        Message::assistant("收尾"),
    ];

    let (pruned, notices) = prune(history, &defense_small());

    assert!(
        !has_any_tool_result(&pruned),
        "孤儿 tool_result 被清扫（provider 端 tool 配对协议要求成对）"
    );
    assert!(tool_call_ids(&pruned).is_empty(), "无悬空 tool_use id");
    assert_eq!(pruned, vec![Message::user("任务书")], "仅首条 user 保全");
    assert_eq!(notices.len(), 1, "清扫减重产出 notice");
}

#[test]
fn prune未越水位时原史原样返回且零notice() {
    let history = vec![
        Message::user("任务"),
        Message::assistant("回应"),
        Message::tool_result("tu_1", "read", "小产物"),
    ];
    assert!(
        defense_small().over_l2(&history),
        "小窗基座活性（史超水位才走剪裁臂——本用例改走宽窗零扰动半边）"
    );

    // 宽窗（缺省 128K）：低水位史零扰动
    let history_estimate = estimate_history(&history);
    let (pruned, notices) = prune(history.clone(), &ContextDefense::resolve(None));

    assert_eq!(
        pruned, history,
        "未越水位原史原样返回（每请求前调用点不扰动）"
    );
    assert_eq!(estimate_history(&pruned), history_estimate, "估算零变化");
    assert!(notices.is_empty(), "零防线 notice");
}

#[test]
fn prune空史与仅首条user均零裁剪零notice不panic() {
    // 空史
    let (empty, notices) = prune(Vec::new(), &defense_small());
    assert!(
        empty.is_empty() && notices.is_empty(),
        "空史零裁剪零 notice"
    );

    // 仅一条 user
    let single = vec![Message::user("任务书")];
    let (pruned, notices) = prune(single.clone(), &defense_small());
    assert_eq!(pruned, single, "仅首条 user 零裁剪");
    assert!(notices.is_empty(), "零 notice");
    // 硬裁同口径
    let (hard, _) = hard_prune(single, &defense_small());
    assert_eq!(hard, vec![Message::user("任务书")], "硬裁保首条 user 不炸");
}

#[test]
fn prune产出context_pruned载荷形状before_after_layer() {
    let history = oversized_history("tu_big", 25_000);
    let before = estimate_history(&history);

    let (pruned, notices) = prune(history, &defense_small());
    let after = estimate_history(&pruned);

    let notice = notices.first().expect("触发剪裁必有 notice");
    assert_eq!(notice.subtype, "context_pruned", "开放词典 subtype 逐字");
    assert_eq!(notice.payload["layer"], json!("l2"), "layer 记剪裁层");
    assert_eq!(
        notice.payload["before"],
        json!(before),
        "before 为剪裁前估算 tokens"
    );
    assert_eq!(
        notice.payload["after"],
        json!(after),
        "after 为剪裁后估算 tokens"
    );
    let (before_value, after_value) = (
        notice.payload["before"].as_u64().expect("before 为数值"),
        notice.payload["after"].as_u64().expect("after 为数值"),
    );
    assert!(after_value < before_value, "剪裁有效（after < before）");
}

// ---------------------------------------------------------------------------
// hard_prune：L3 失败降级硬裁
// ---------------------------------------------------------------------------

#[test]
fn hard_prune中间整段丢弃保首条user与保护窗() {
    // [首条 user, 老轮对, 超大 filler, 中段轮, 尾问] → 中间整段移除
    let history = vec![
        Message::user("任务书"),
        assistant_with_call("tu_mid"),
        tool_result_at_tokens("tu_mid", 25_000),
        user_at_tokens(50_000),
        Message::assistant("保护窗内回应"),
        Message::user("保护窗内尾问"),
    ];
    let protected = protected_start(&history, &defense_small());
    assert_eq!(protected, 4, "保护窗圈定末两条（前置自检）");

    let (pruned, _notice) = hard_prune(history, &defense_small());

    // 产物 = [首条 user, 保护窗内容]：中间整段移除、配对完整（无半边残留）
    assert_eq!(pruned.len(), 3, "首条 user + 保护窗两条");
    assert_eq!(pruned[0], Message::user("任务书"), "首条 user 保全");
    assert_eq!(pruned[1], Message::assistant("保护窗内回应"));
    assert_eq!(pruned[2], Message::user("保护窗内尾问"));
    assert!(
        !has_any_tool_result(&pruned) && tool_call_ids(&pruned).is_empty(),
        "被丢弃段的 tool 对整体离场，保留段配对完整"
    );
    let _ = protected;
}

#[test]
fn hard_prune产出恰一条context_compacted降级notice() {
    let history = vec![
        Message::user("任务书"),
        assistant_with_call("tu_x"),
        tool_result_at_tokens("tu_x", 25_000),
        user_at_tokens(50_000),
    ];
    let before = estimate_history(&history);

    let (pruned, notice) = hard_prune(history, &defense_small());
    let after = estimate_history(&pruned);

    // 恰一条 DefenseNotice，payload 含 before / after / layer / fallback
    assert_eq!(notice.subtype, "context_compacted", "降级路径 notice 词汇");
    assert_eq!(notice.payload["layer"], json!("l3"), "layer 记 L3 硬裁层");
    assert_eq!(
        notice.payload["fallback"],
        json!(true),
        "降级留痕（loop 组合口径）"
    );
    assert_eq!(notice.payload["before"], json!(before), "before 估算");
    assert_eq!(notice.payload["after"], json!(after), "after 估算");
    assert!(
        notice.payload["after"].as_u64() < notice.payload["before"].as_u64(),
        "硬裁有效减重"
    );
}
