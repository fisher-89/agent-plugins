use rig::completion::{CompletionModel, CompletionRequest};
use rig::message::{AssistantContent, Message};

use crate::sdk::context::{protected_start, ContextDefense};

/// 摘要指令要点集（措辞留实现期打磨窗，要点集不减）。
const SUMMARY_INSTRUCTION: &str = "这是历史摘要，请勿重复已完成的工作。请将以下对话历史压缩为\
    要点集，保留继续工作所需的全部关键信息：\n\
    1. 已完成的工作与产物（文件路径、改动位置）\n\
    2. 关键技术决策及其原因\n\
    3. 进行中的工作与当前状态\n\
    4. 下一步计划\n\
    5. 重要约束（用户要求、验收标准、边界条件）\n\
    直接输出摘要正文，不要输出开场白或解释。";

/// L3 摘要：成功返回 `[摘要（[历史摘要] 头 user 置顶）, 首条 user, 保护窗]`
/// 新史；失败 `Err`（调用方降级硬裁）。
pub(crate) async fn summarize<M: CompletionModel>(
    model: &M,
    history: &[Message],
    defense: &ContextDefense,
) -> Result<Vec<Message>, String> {
    let Some(first) = history.first() else {
        return Err("空历史无从摘要".to_owned());
    };
    let protected = protected_start(history, defense);
    let middle = &history[1..protected.max(1)];
    if middle.is_empty() {
        return Err("保护窗前无可摘要段".to_owned());
    }
    let transcript =
        serde_json::to_string(middle).map_err(|e| format!("待摘要历史序列化失败: {e}"))?;
    let request = CompletionRequest {
        model: None,
        preamble: Some("你是对话历史摘要器：只输出摘要正文，不执行任何工具。".to_owned()),
        chat_history: vec![Message::user(format!(
            "{SUMMARY_INSTRUCTION}\n\n<待摘要历史>\n{transcript}\n</待摘要历史>"
        ))],
        documents: Vec::new(),
        // 禁工具：摘要请求不携带工具面（工具选择不在摘要语义内）
        tools: Vec::new(),
        temperature: None,
        max_tokens: None,
        tool_choice: None,
        additional_params: None,
        output_schema: None,
        record_telemetry_content: false,
    };
    // 一次重试：空响应与请求失败同列（摘要必须非空才有置换资格）
    let mut last_error = String::new();
    for _ in 0..2 {
        match model.completion(request.clone()).await {
            Ok(response) => {
                let summary = response_text(&response.choice);
                if !summary.trim().is_empty() {
                    let mut compacted = Vec::with_capacity(2 + history.len() - protected);
                    compacted.push(Message::user(format!("[历史摘要]\n{summary}")));
                    compacted.push(first.clone());
                    compacted.extend(history.iter().skip(protected.max(1)).cloned());
                    return Ok(compacted);
                }
                last_error = "摘要响应为空".to_owned();
            }
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(format!("摘要失败（含一次重试）: {last_error}"))
}

/// 摘要响应文本提取（choice 的 Text 块拼接；非文本块忽略——摘要请求禁工具，
/// 模型越权工具调用不进摘要面）。
fn response_text(choice: &[AssistantContent]) -> String {
    choice
        .iter()
        .filter_map(|content| match content {
            AssistantContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
