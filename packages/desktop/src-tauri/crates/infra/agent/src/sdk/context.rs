//! 上下文窗防线 L2（确定性剪裁，sdk 引擎内部特性）：请求前对**喂 provider 的
//! 请求史**（非 store 转录）实施确定性剪裁——先占位符替换保护窗外单体超门槛
//! 的老 tool_result（call / name 保留，tool_use / tool_result 配对结构完整），
//! 不够再丢最老完整轮对，恒保首条 user（phase agent 首条 user 即任务书）；
//! 孤儿配对清扫兜底。剪裁产出 `context_pruned` notice（开放词典 subtype +
//! before / after / layer 载荷），store 转录全量不变。L3 compaction 见
//! [`crate::sdk::compact`]，调用点编排见 [`crate::sdk::r#loop`] 与
//! [`crate::sdk::runner`]。

use std::collections::HashSet;

use rig::message::{AssistantContent, Message, ToolResult, ToolResultContent, UserContent};
use serde_json::Value;

/// 缺省窗长（token 数）：provider 记录 `context_length` 缺席时的 128K 启发式。
const DEFAULT_CONTEXT_WINDOW: u64 = 128 * 1024;

/// 防线配置（引擎内部，不入 core 契约）：窗长 + 触发比（L2 75% / L3 90%，
/// 由窗长导出）+ 保护窗 + 单结果 prune 门槛。
pub(crate) struct ContextDefense {
    /// 上下文窗长（token 数；provider `context_length` 或缺省启发式）
    context_window: u64,
    /// 保护窗（40k tokens）：最近历史不参与剪裁（余量覆盖响应 token 与
    /// 估算误差）
    protected_tokens: u64,
    /// 单结果 prune 门槛（20k tokens）：单体超门槛的老 tool_result 才裁
    prune_threshold: u64,
}

impl ContextDefense {
    /// 窗长解析：provider `context_length` 缺席（或非法 0）走 128K 缺省；
    /// 触发比 75% / 90% 与保护窗 40k、prune 门槛 20k 为 design 拍板定值
    /// （阈值常数留实现期微调窗，结构不动）。
    pub(crate) fn resolve(context_window: Option<u64>) -> Self {
        let context_window = match context_window {
            Some(window) if window > 0 => window,
            _ => DEFAULT_CONTEXT_WINDOW,
        };
        Self {
            context_window,
            protected_tokens: 40 * 1024,
            prune_threshold: 20 * 1024,
        }
    }

    /// L2 触发水位：窗长 75%——水位过线即请求前剪裁。
    fn l2_threshold(&self) -> u64 {
        self.context_window * 3 / 4
    }

    /// L3 触发水位：窗长 90%——prune 后仍过线即 LLM compaction。
    fn l3_threshold(&self) -> u64 {
        self.context_window * 9 / 10
    }

    /// L2 触发判定：估算水位过 75% 窗长。
    pub(crate) fn over_l2(&self, history: &[Message]) -> bool {
        estimate_history(history) > self.l2_threshold()
    }

    /// L3 触发判定：估算水位过 90% 窗长。
    pub(crate) fn over_l3(&self, history: &[Message]) -> bool {
        estimate_history(history) > self.l3_threshold()
    }
}

/// 防线产出的 `SystemNotice` 前体（subtype 开放词典 + 载荷；经调用点转发为
/// 密封事件流出落库，非 delta 事件零落库纪律不破）。
pub(crate) struct DefenseNotice {
    pub(crate) subtype: String,
    pub(crate) payload: Value,
}

/// 字节启发式 token 估算：各 Message 的 serde_json 序列化 UTF-8 字节 ÷ 4 向上
/// 取整求和（常数口径对齐 rig-memory `HeuristicTokenCounter`；序列化字节含
/// 结构开销 = 保守高估，宁早裁不触 L4）。
pub(crate) fn estimate_history(history: &[Message]) -> u64 {
    history
        .iter()
        .map(|message| {
            let bytes = serde_json::to_vec(message)
                .map(|bytes| bytes.len())
                .unwrap_or(0);
            (bytes as u64).div_ceil(4)
        })
        .sum()
}

/// L2 确定性剪裁：未过 L2 水位零动作；过线先占位符替换保护窗外单体超门槛的
/// 老 tool_result（配对完整），不够再丢最老完整轮对（assistant 起至下一
/// assistant 前的完整交换段，恒保首条 user），孤儿配对清扫兜底。有裁才产
/// `context_pruned` notice（before / after 为估算 tokens，layer = "l2"）。
pub(crate) fn prune(
    history: Vec<Message>,
    defense: &ContextDefense,
) -> (Vec<Message>, Vec<DefenseNotice>) {
    let before = estimate_history(&history);
    if !defense.over_l2(&history) {
        return (history, Vec::new());
    }
    let protected = protected_start(&history, defense);
    // 第一层：超大老工具结果占位符替换（密度不可恢复前的确定性卸载）
    let mut current = replace_oversized_results(history, defense, protected);
    // 第二层：仍过水位则丢最老完整轮对（配对结构完整优先于历史完整）
    while defense.over_l2(&current) {
        match oldest_unit_range(&current, protected_start(&current, defense)) {
            Some((start, end)) => {
                current.drain(start..end);
            }
            None => break,
        }
    }
    let current = sweep_orphans(current);
    let after = estimate_history(&current);
    let notices = if after < before {
        vec![DefenseNotice {
            subtype: "context_pruned".to_owned(),
            payload: serde_json::json!({ "before": before, "after": after, "layer": "l2" }),
        }]
    } else {
        Vec::new()
    };
    (current, notices)
}

/// L3 失败降级的硬裁：保首条 user + 保护窗，中间整段丢弃（不摘要不保密度）。
/// notice 恒 `context_compacted` + `layer:"l3"` + `fallback:true`（降级留痕，
/// run 不失败收敛）。
pub(crate) fn hard_prune(
    history: Vec<Message>,
    defense: &ContextDefense,
) -> (Vec<Message>, DefenseNotice) {
    let before = estimate_history(&history);
    let protected = protected_start(&history, defense);
    let mut pruned: Vec<Message> = Vec::new();
    if let Some(first) = history.first() {
        pruned.push(first.clone());
    }
    pruned.extend(history.into_iter().skip(protected.max(1)));
    let pruned = sweep_orphans(pruned);
    let after = estimate_history(&pruned);
    (
        pruned,
        DefenseNotice {
            subtype: "context_compacted".to_owned(),
            payload: serde_json::json!({
                "before": before,
                "after": after,
                "layer": "l3",
                "fallback": true
            }),
        },
    )
}

/// 保护窗起点：自尾部反向累计估算 token（末条恒保——单体超窗的巨块不再
/// 外溢裁剪），越过保护窗的最早消息下标（该下标起至尾为保护窗）。
pub(crate) fn protected_start(history: &[Message], defense: &ContextDefense) -> usize {
    let len = history.len();
    if len == 0 {
        return 0;
    }
    let mut total = estimate_message(&history[len - 1]);
    let mut index = len - 1;
    while index > 0 {
        let message_tokens = estimate_message(&history[index - 1]);
        if total + message_tokens > defense.protected_tokens {
            break;
        }
        total += message_tokens;
        index -= 1;
    }
    index
}

/// 第一层剪裁：保护窗外的老 tool_result 正文超 prune 门槛时替换为占位符
/// （call / provider / name 原样保留——tool_use / tool_result 配对结构完整，
/// provider 侧重放不受损；仅正文不可恢复地让位）。
fn replace_oversized_results(
    history: Vec<Message>,
    defense: &ContextDefense,
    protected: usize,
) -> Vec<Message> {
    history
        .into_iter()
        .enumerate()
        .map(|(index, message)| match (index < protected, message) {
            // 剪裁对象 = 保护窗外的老消息（下标 < protected；[protected..] 为
            // 最近窗口不裁——「最近窗口保护」的确定性半边）
            (true, Message::User { content }) => Message::User {
                content: content
                    .into_iter()
                    .map(|item| match item {
                        UserContent::ToolResult(result)
                            if estimate_result_content(&result) > defense.prune_threshold =>
                        {
                            let tokens = estimate_result_content(&result);
                            UserContent::ToolResult(ToolResult {
                                call: result.call,
                                provider: result.provider,
                                name: result.name,
                                content: vec![ToolResultContent::text(format!(
                                    "[已剪裁] 本工具结果约 {tokens} tokens（超单结果保留门槛 \
                                     {}），原内容已从请求史移除；store 转录全量保留",
                                    defense.prune_threshold
                                ))],
                            })
                        }
                        item => item,
                    })
                    .collect(),
            },
            (_, message) => message,
        })
        .collect()
}

/// 最老可丢完整轮对的区间（assistant 起至下一 assistant 前的完整交换段）：
/// 首条消息恒不在区间内（保首条 user），区间尾不越保护窗。无可丢轮对 None。
fn oldest_unit_range(history: &[Message], protected: usize) -> Option<(usize, usize)> {
    let start = history
        .iter()
        .position(|message| matches!(message, Message::Assistant { .. }))
        .filter(|&index| index >= 1)?;
    let end = history[start + 1..]
        .iter()
        .position(|message| matches!(message, Message::Assistant { .. }))
        .map_or(history.len(), |offset| start + 1 + offset);
    (end <= protected.max(start + 1)).then_some((start, end))
}

/// 孤儿配对清扫：tool_result 的 tool_use 伙伴不在其前（assistant 已被裁）
/// 时剔除该结果条目（provider 端 tool 配对协议要求成对出现），纯文本位保留；
/// 清空后无内容的 user 位消息整条移除。
fn sweep_orphans(history: Vec<Message>) -> Vec<Message> {
    let mut answered: HashSet<String> = HashSet::new();
    history
        .into_iter()
        .filter_map(|message| match message {
            Message::Assistant { id, content } => {
                for item in &content {
                    if let AssistantContent::ToolCall(call) = item {
                        answered.insert(call.id.as_str().to_owned());
                    }
                }
                Some(Message::Assistant { id, content })
            }
            Message::User { content } => {
                let kept: Vec<UserContent> = content
                    .into_iter()
                    .filter(|item| match item {
                        UserContent::ToolResult(result) => answered.contains(result.call.as_str()),
                        _ => true,
                    })
                    .collect();
                (!kept.is_empty()).then_some(Message::User { content: kept })
            }
            message => Some(message),
        })
        .collect()
}

/// 单条消息估算（历史求和的复用面）。
fn estimate_message(message: &Message) -> u64 {
    let bytes = serde_json::to_vec(message)
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    (bytes as u64).div_ceil(4)
}

/// 工具结果正文口径估算（与历史估算同启发式，仅载荷不含信封）。
fn estimate_result_content(result: &ToolResult) -> u64 {
    let bytes = serde_json::to_vec(&result.content)
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    (bytes as u64).div_ceil(4)
}
