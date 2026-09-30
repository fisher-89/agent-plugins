//! `jsonl` 的单元测试（AC-2 / D8）：单行归一化口径——system init → RunStarted、
//! 其余 system → SystemNotice、assistant/user → Message、result → RunResult、
//! 未知 type / 非 JSON → Raw 透传（永不丢事件、永不炸解析）、空白行跳过不占
//! seq。行 fixture 以内嵌常量构造，无进程/文件边界，不需要 Mock。

use agent::AgentEventKind;

use crate::jsonl::normalize_line;

/// system/init 行：model / session_id / tools / mcp_servers 齐备。
const INIT_LINE: &str = r#"{"type":"system","subtype":"init","model":"claude-opus","session_id":"s-1","tools":["Bash","Read"],"mcp_servers":["mcp-a"]}"#;
/// assistant 行：thinking + tool_use 两块。
const ASSISTANT_TOOL_USE_LINE: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"想一下"},{"type":"tool_use","id":"tu_1","name":"Bash","input":{"command":"ls"}}]},"parent_tool_use_id":null}"#;
/// user 行：tool_result 块（content 为数组形态）。
const USER_TOOL_RESULT_LINE: &str = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","id":"tu_1","content":[{"type":"text","text":"行一"},{"type":"text","text":"行二"}]}]}}"#;
/// result 行：全字段齐备。
const RESULT_LINE: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":3,"duration_ms":1234,"total_cost_usd":0.42,"usage":{"input_tokens":10},"session_id":"s-1"}"#;

// ---------------------------------------------------------------------------
// 正向：五类行的归一化
// ---------------------------------------------------------------------------

#[test]
fn system_init行归一化为run_started并提取四字段() {
    let Some(AgentEventKind::RunStarted {
        model,
        session_id,
        tools,
        mcp_servers,
    }) = normalize_line(INIT_LINE)
    else {
        panic!("init 行应归一化为 RunStarted");
    };

    assert_eq!(model.as_deref(), Some("claude-opus"));
    assert_eq!(session_id.as_deref(), Some("s-1"));
    assert_eq!(tools, vec!["Bash".to_owned(), "Read".to_owned()]);
    assert_eq!(mcp_servers, vec!["mcp-a".to_owned()]);
}

#[test]
fn system其余subtype归一化为system_notice且payload存原json() {
    let line = r#"{"type":"system","subtype":"permission_denial","tool":"Bash"}"#;
    let Some(AgentEventKind::SystemNotice { subtype, payload }) = normalize_line(line) else {
        panic!("其余 system 行应归一化为 SystemNotice（不封闭枚举）");
    };

    assert_eq!(subtype, "permission_denial");
    assert_eq!(
        payload,
        serde_json::json!({ "type": "system", "subtype": "permission_denial", "tool": "Bash" }),
        "payload 为原 JSON 全量"
    );
}

#[test]
fn assistant与user行归一化为message且blocks按类型映射() {
    let Some(AgentEventKind::Message {
        role,
        blocks,
        parent_tool_use_id,
    }) = normalize_line(ASSISTANT_TOOL_USE_LINE)
    else {
        panic!("assistant 行应归一化为 Message");
    };
    assert_eq!(role, "assistant", "role 取自 type");
    assert_eq!(parent_tool_use_id, None);
    assert_eq!(blocks.len(), 2, "thinking + tool_use 两块");
    assert!(matches!(&blocks[0], agent::AgentBlock::Thinking { thinking } if thinking == "想一下"));
}

#[test]
fn tool_use与tool_result块的id_name_input与扁平content映射正确() {
    let Some(AgentEventKind::Message { blocks, .. }) = normalize_line(ASSISTANT_TOOL_USE_LINE)
    else {
        panic!("assistant 行应归一化为 Message");
    };
    // 第二块为 tool_use：id/name/input 直取
    assert!(
        matches!(
            &blocks[1],
            agent::AgentBlock::ToolUse { id, name, input }
                if id == "tu_1" && name == "Bash" && input == &serde_json::json!({ "command": "ls" })
        ),
        "tool_use 块字段映射，实际: {:?}",
        blocks[1]
    );

    // user 行：tool_result content 数组逐元素取 text 按行拼接
    let Some(AgentEventKind::Message { blocks, .. }) = normalize_line(USER_TOOL_RESULT_LINE) else {
        panic!("user 行应归一化为 Message");
    };
    assert!(
        matches!(
            &blocks[0],
            agent::AgentBlock::ToolResult { id, content, is_error }
                if id == "tu_1" && content == "行一\n行二" && !*is_error
        ),
        "tool_result 数组 content 扁平化拼接，实际: {:?}",
        blocks[0]
    );
}

#[test]
fn result行归一化为run_result且字段名对齐() {
    let Some(AgentEventKind::RunResult {
        subtype,
        is_error,
        num_turns,
        duration_ms,
        cost_usd,
        usage,
        session_id,
    }) = normalize_line(RESULT_LINE)
    else {
        panic!("result 行应归一化为 RunResult");
    };

    assert_eq!(subtype, "success");
    assert!(!is_error);
    assert_eq!(num_turns, Some(3));
    assert_eq!(duration_ms, Some(1234));
    assert_eq!(cost_usd, Some(0.42), "total_cost_usd → cost_usd 对齐");
    assert_eq!(usage, serde_json::json!({ "input_tokens": 10 }));
    assert_eq!(session_id.as_deref(), Some("s-1"));
}

// ---------------------------------------------------------------------------
// 正向：未知 / 非 JSON 的 Raw 兜底
// ---------------------------------------------------------------------------

#[test]
fn 未知type合法json归一化为raw且原文透传() {
    let line = r#"{"type":"stream_event","x":1}"#;
    let Some(AgentEventKind::Raw {
        event_type,
        raw_json,
    }) = normalize_line(line)
    else {
        panic!("未知 type 应归一化为 Raw（AC-2）");
    };
    assert_eq!(event_type, "stream_event", "eventType 取自 type 值");
    assert_eq!(raw_json, line, "rawJson 为原文");
}

#[test]
fn 非json行归一化为unparsable且永不炸解析() {
    let line = "这不是 JSON {";
    let Some(AgentEventKind::Raw {
        event_type,
        raw_json,
    }) = normalize_line(line)
    else {
        panic!("非 JSON 行应归一化为 Raw");
    };
    assert_eq!(event_type, "unparsable");
    assert_eq!(raw_json, line);
}

#[test]
fn 缺type键的合法json归一化为unknown形态raw() {
    let line = r#"{"foo":1}"#;
    let Some(AgentEventKind::Raw {
        event_type,
        raw_json,
    }) = normalize_line(line)
    else {
        panic!("缺 type 应有 Raw 兜底，不丢事件");
    };
    assert_eq!(event_type, "unknown");
    assert_eq!(raw_json, line);
}

// ---------------------------------------------------------------------------
// 边界：空白行 / 字段缺失 / content 形态
// ---------------------------------------------------------------------------

#[test]
fn 空白行跳过返回none不占seq() {
    assert_eq!(normalize_line(""), None);
    assert_eq!(normalize_line("   "), None);
    assert_eq!(normalize_line("\t \r"), None);
}

#[test]
fn message的content为字符串时降级为单个text块() {
    let line = r#"{"type":"assistant","message":{"content":"纯文本正文"}}"#;
    let Some(AgentEventKind::Message { blocks, .. }) = normalize_line(line) else {
        panic!("应归一化为 Message");
    };
    assert_eq!(blocks.len(), 1, "字符串 content 降级为单个块，不丢正文");
    assert!(
        matches!(&blocks[0], agent::AgentBlock::Text { text } if text == "纯文本正文"),
        "实际: {:?}",
        blocks[0]
    );
}

#[test]
fn message含未知块类型时跳过该块不堵信封() {
    let line = r#"{"type":"assistant","message":{"content":[{"type":"mystery_block","x":1},{"type":"text","text":"正文"}]}}"#;
    let Some(AgentEventKind::Message { blocks, .. }) = normalize_line(line) else {
        panic!("块级未知不堵事件信封");
    };
    assert_eq!(blocks.len(), 1, "未知块跳过，已知块保留");
    assert!(matches!(&blocks[0], agent::AgentBlock::Text { text } if text == "正文"));
}

#[test]
fn tool_result的content为数组时拼接为非数组时json序列化兜底() {
    // 非数组形态：对象 → JSON 序列化
    let object_line = r#"{"type":"user","message":{"content":[{"type":"tool_result","id":"tu_2","content":{"weird":true}}]}}"#;
    let Some(AgentEventKind::Message { blocks, .. }) = normalize_line(object_line) else {
        panic!("应归一化为 Message");
    };
    assert!(
        matches!(&blocks[0], agent::AgentBlock::ToolResult { content, .. }
            if content == r#"{"weird":true}"#),
        "非数组 content JSON 序列化兜底，实际: {:?}",
        blocks[0]
    );

    // 字符串形态：原样
    let string_line = r#"{"type":"user","message":{"content":[{"type":"tool_result","id":"tu_3","content":"直接文本"}]}}"#;
    let Some(AgentEventKind::Message { blocks, .. }) = normalize_line(string_line) else {
        panic!("应归一化为 Message");
    };
    assert!(
        matches!(&blocks[0], agent::AgentBlock::ToolResult { content, .. } if content == "直接文本"),
    );
}

#[test]
fn tool_result的is_error缺失视作false有true时保真() {
    let error_line = r#"{"type":"user","message":{"content":[{"type":"tool_result","id":"tu_4","content":"失败","is_error":true}]}}"#;
    let Some(AgentEventKind::Message { blocks, .. }) = normalize_line(error_line) else {
        panic!("应归一化为 Message");
    };
    assert!(matches!(
        &blocks[0],
        agent::AgentBlock::ToolResult { is_error: true, .. }
    ));
    // 缺失 → false（与 USER_TOOL_RESULT_LINE 同口径）
    let Some(AgentEventKind::Message { blocks, .. }) = normalize_line(USER_TOOL_RESULT_LINE) else {
        panic!("应归一化为 Message");
    };
    assert!(matches!(
        &blocks[0],
        agent::AgentBlock::ToolResult {
            is_error: false,
            ..
        }
    ));
}

#[test]
fn init缺model与session置null缺tools与mcp置空数组() {
    let line = r#"{"type":"system","subtype":"init"}"#;
    let Some(AgentEventKind::RunStarted {
        model,
        session_id,
        tools,
        mcp_servers,
    }) = normalize_line(line)
    else {
        panic!("缺字段 init 行仍归一化为 RunStarted");
    };
    assert_eq!(model, None);
    assert_eq!(session_id, None);
    assert!(tools.is_empty());
    assert!(mcp_servers.is_empty());
}

#[test]
fn result缺汇总字段时置null与false() {
    let line = r#"{"type":"result"}"#;
    let Some(AgentEventKind::RunResult {
        subtype,
        is_error,
        num_turns,
        duration_ms,
        cost_usd,
        usage,
        session_id,
    }) = normalize_line(line)
    else {
        panic!("缺字段 result 行仍归一化为 RunResult");
    };
    assert_eq!(subtype, "");
    assert!(!is_error);
    assert_eq!(num_turns, None);
    assert_eq!(duration_ms, None);
    assert_eq!(cost_usd, None);
    assert_eq!(usage, serde_json::Value::Null);
    assert_eq!(session_id, None);
}

#[test]
fn message行携parent_tool_use_id时提取为有值形态() {
    let line = r#"{"type":"user","message":{"content":[]},"parent_tool_use_id":"tu_9"}"#;
    let Some(AgentEventKind::Message {
        parent_tool_use_id, ..
    }) = normalize_line(line)
    else {
        panic!("应归一化为 Message");
    };
    assert_eq!(
        parent_tool_use_id.as_deref(),
        Some("tu_9"),
        "子代理归因字段提取"
    );
}

// ---------------------------------------------------------------------------
// 边界：多行序列、洪峰与深嵌套（seq 单调口径在 jsonl 层即「空白行不占号」）
// ---------------------------------------------------------------------------

#[test]
fn 多行混合fixture归一化序列仅含非空白行且空白行不占位() {
    let lines = [
        INIT_LINE,
        "", // 空白行：None，不占归一化位（泵侧 seq 不递增）
        ASSISTANT_TOOL_USE_LINE,
        "   ",
        USER_TOOL_RESULT_LINE,
        RESULT_LINE,
    ];
    let kinds: Vec<AgentEventKind> = lines
        .iter()
        .filter_map(|line| normalize_line(line))
        .collect();

    // 4 条非空白行 → 恰 4 个归一化产物（泵按序盖 seq 0..4，AC-2 seq 单调）
    assert_eq!(kinds.len(), 4, "空白行不占归一化位");
    assert!(matches!(kinds[0], AgentEventKind::RunStarted { .. }));
    assert!(matches!(kinds[1], AgentEventKind::Message { .. }));
    assert!(matches!(kinds[2], AgentEventKind::Message { .. }));
    assert!(matches!(kinds[3], AgentEventKind::RunResult { .. }));
}

#[test]
fn 零行输入产出零事件不报错() {
    assert_eq!(normalize_line(""), None, "空输入归一化为 None");
}

#[test]
fn 三百行混合fixture全部产出事件不丢不炸() {
    let mut count = 0usize;
    for index in 0..300 {
        let line = match index % 5 {
            0 => INIT_LINE.to_owned(),
            1 => ASSISTANT_TOOL_USE_LINE.to_owned(),
            2 => USER_TOOL_RESULT_LINE.to_owned(),
            3 => RESULT_LINE.to_owned(),
            _ => format!(r#"{{"type":"noise-{index}","n":{index}}}"#),
        };
        if normalize_line(&line).is_some() {
            count += 1;
        }
    }
    assert_eq!(count, 300, "洪峰全量产出（含未知 type 的 Raw），零丢失");
}

#[test]
fn 深嵌套payload与超长字符串不炸且保真() {
    // SystemNotice payload 深嵌套
    let deep_line = r#"{"type":"system","subtype":"dump","a":{"b":{"c":{"d":[1,2,{"e":"深"}]}}}}"#;
    let Some(AgentEventKind::SystemNotice { payload, .. }) = normalize_line(deep_line) else {
        panic!("深嵌套 payload 不炸解析");
    };
    assert_eq!(payload["a"]["b"]["c"]["d"][2]["e"], serde_json::json!("深"));

    // Raw 行含超长字符串（>1000 字符）
    let long_value = "长".repeat(1500);
    let long_line = format!(r#"{{"type":"huge","blob":"{long_value}"}}"#);
    let Some(AgentEventKind::Raw { raw_json, .. }) = normalize_line(&long_line) else {
        panic!("超长行不炸解析");
    };
    assert!(raw_json.contains(&long_value), "rawJson 超长原文保真");
}
