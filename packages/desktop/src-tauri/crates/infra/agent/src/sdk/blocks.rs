//! 密封块的**双向分派单点**：rig 内容 ↔ `AgentBlock` 序列。
//!
//! 与 `normalize`（增量层：rig 流项 → `AgentDelta`）对称——本模块是密封层的
//! 对应位置。域模块只持有各自的原语（`web_search`：wire 形状与回放构造；
//! `reasoning`：思考凭证的投影与回放），**遍历与变体分派住这里**，故新增块
//! 变体只有一处需要改。
//!
//! 两个方向：
//! - [`project_choice`]：一次 assistant 回应 → 事件块 + 客户端工具调用；
//! - [`assistant_content`] / [`user_content`]：事件块 → rig 重建载荷。
//!
//! 投影侧不变量（与 `web_search` 模块 doc 同源）：raw server 块按
//! `tool_use_id` 预索引配对、顺序无关；展示字段不参与回放。

use std::collections::{HashMap, HashSet};

use agent::AgentBlock;
use rig::message::{AssistantContent, ToolCall, ToolName, UserContent};
use serde_json::Value;

use crate::sdk::{reasoning, web_search};

/// 块投影唯一实现（`loop` 只调用，不再各自解 raw）：先**完整预索引**（收集
/// 全部 `web_search` 的 `server_tool_use` id + 按 `tool_use_id` 索引全部
/// `web_search_tool_result`，并在预索引期即预置「已消费」下标），再顺序遍历
/// choice——故 result 与 use 的**顺序无关**（`pause_turn` 拆分后 result 可能
/// 先到）：
///
/// - `server_tool_use`（`name == "web_search"`）→ 一块
///   [`AgentBlock::WebSearch`]，其配对 result 的载荷并入该块（result 侧不重复
///   产出）；
/// - 孤儿 `web_search_tool_result`（无配对 use）→ 单独成块
///   （`raw_use: None`、`id` 取 `tool_use_id`、`query` 空串）；
/// - 其他 raw server 块（非 `web_search` 的 `server_tool_use`、
///   `code_execution_tool_result` 等）→ 跳过（不产块、不报错）；
/// - `Text` 无 raw → [`AgentBlock::Text`]（citations 自
///   `additional_params["citations"]` 提取）；
/// - `Reasoning` → **逐载荷**产 `Thinking`（不拼接；见 `reasoning::project`）；
/// - `ToolCall` / `Image` → 现状不变。
pub(crate) fn project_choice(choice: &[AssistantContent]) -> (Vec<AgentBlock>, Vec<ToolCall>) {
    // 预索引源 1：`web_search` 的 `server_tool_use` id 集合（配对判别值）
    let mut use_ids: HashSet<&str> = HashSet::new();
    // 预索引源 2：tool_use_id → （choice 下标, result 原文）；同 id 取首个
    let mut result_by_use: HashMap<&str, (usize, &Value)> = HashMap::new();
    for (index, content) in choice.iter().enumerate() {
        let AssistantContent::Text(text) = content else {
            continue;
        };
        let Some(raw) = web_search::raw_content(text) else {
            continue;
        };
        if let Some(use_id) = web_search::use_id(raw) {
            use_ids.insert(use_id);
            continue;
        }
        let Some(use_id) = web_search::result_use_id(raw) else {
            continue;
        };
        result_by_use.entry(use_id).or_insert((index, raw));
    }

    let mut consumed: Vec<bool> = vec![false; choice.len()];
    // 「已消费」在预索引期落定（而非遍历到 use 时才标）——否则 choice 倒序
    // （result 先于 use）时 result 会先以孤儿成块、随后 use 再配出第二块。
    // 只预置**有配对 use** 的首个 result 下标：无 use 的 result 不预置，保留
    // 孤儿规则；同 id 的第二条 result 亦不预置，照常成块。
    for (use_id, (index, _)) in &result_by_use {
        if use_ids.contains(use_id) {
            consumed[*index] = true;
        }
    }

    let mut blocks: Vec<AgentBlock> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    for (index, content) in choice.iter().enumerate() {
        match content {
            AssistantContent::Text(text) => {
                let Some(raw) = web_search::raw_content(text) else {
                    // 普通文本：正文 + 引用原值
                    blocks.push(AgentBlock::Text {
                        text: text.text.clone(),
                        citations: web_search::citations_of(text),
                    });
                    continue;
                };
                if let Some(use_id) = web_search::use_id(raw) {
                    // 配对已由预索引落定（含 result 先于 use 的倒序形态），
                    // 此处只取值、不再改 `consumed`
                    let raw_result = result_by_use.get(use_id).map(|(_, result)| *result);
                    blocks.push(web_search::server_block(raw, raw_result));
                } else if web_search::result_use_id(raw).is_some() {
                    if consumed[index] {
                        continue; // 已被 use 侧消费：载荷并入、不重复产块
                    }
                    blocks.extend(web_search::orphan_block(raw));
                }
                // 其他 raw server 块（code_execution_tool_result 等）与无
                // `tool_use_id` 的 wild result → 跳过（不产块、不报错）
            }
            AssistantContent::Reasoning(reasoning) => blocks.extend(reasoning::project(reasoning)),
            AssistantContent::ToolCall(tool_call) => {
                blocks.push(AgentBlock::ToolUse {
                    id: tool_call.id.to_string(),
                    name: tool_call.function.name.as_str().to_owned(),
                    input: tool_call.function.arguments.clone(),
                });
                tool_calls.push(tool_call.clone());
            }
            AssistantContent::Image(_) => {} // sdk 引擎不产图像内容（工具面无边）
        }
    }
    (blocks, tool_calls)
}

/// user 位文本块映射（非文本块跳过；user 侧 citations 不参与重建）。
pub(crate) fn user_content(blocks: &[AgentBlock]) -> Vec<UserContent> {
    blocks
        .iter()
        .filter_map(|block| match block {
            AgentBlock::Text { text, .. } => Some(UserContent::text(text.clone())),
            _ => None, // user 消息内的 Thinking / ToolUse 非法形态跳过
        })
        .collect()
}

/// assistant 消息块映射
pub(crate) fn assistant_content(blocks: &[AgentBlock]) -> Vec<AssistantContent> {
    let mut content: Vec<AssistantContent> = Vec::with_capacity(blocks.len());
    for block in blocks {
        match block {
            // 正文 + 引用原值回填（非空 citations 才挂）
            AgentBlock::Text { text, citations } => {
                content.push(web_search::replay_text(text, citations.as_deref()));
            }
            AgentBlock::Thinking {
                thinking,
                signature,
                redacted,
            } => {
                if let Some(item) =
                    reasoning::replay(thinking, signature.as_deref(), redacted.as_deref())
                {
                    content.push(item);
                }
            }
            AgentBlock::ToolUse { id, name, input } => {
                let Some(name) = ToolName::new(name.clone()).ok() else {
                    continue;
                };
                content.push(AssistantContent::tool_call(id.clone(), name, input.clone()));
            }
            // 一次搜索 = 一条 use → 一条 result（保原始 wire 顺序）
            AgentBlock::WebSearch {
                raw_use,
                raw_result,
                ..
            } => {
                if let Some(raw) = raw_use {
                    content.push(web_search::replay_raw(raw));
                }
                if let Some(raw) = raw_result {
                    content.push(web_search::replay_raw(raw));
                }
            }
            // assistant 消息内的 ToolResult 非法形态跳过
            AgentBlock::ToolResult { .. } => {}
        }
    }
    content
}
