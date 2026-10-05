use std::collections::VecDeque;
use std::sync::Mutex;

use rig::completion::{
    CompletionError, CompletionModel, CompletionRequest, CompletionResponse, Usage,
};
use rig::message::{AssistantContent, Message};
use rig::streaming::StreamingCompletionResponse;

use crate::sdk::compact::summarize;
use crate::sdk::context::ContextDefense;

// ---------------------------------------------------------------------------
// 装置：假摘要模型、防线底座、史 fixture
// ---------------------------------------------------------------------------

/// 假摘要模型：`completion` 按预编程序列返回摘要文本（Ok）或错误串（Err）；
/// 逐次捕获 CompletionRequest。`stream` 路径不被 summarize 消费（恒 Err）。
struct SummaryModel {
    outcomes: Mutex<VecDeque<Result<String, String>>>,
    requests: Mutex<Vec<CompletionRequest>>,
}

impl SummaryModel {
    fn with_outcomes(outcomes: Vec<Result<String, String>>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn captured_requests(&self) -> Vec<CompletionRequest> {
        self.requests.lock().expect("请求捕获锁不可中毒").clone()
    }
}

impl CompletionModel for SummaryModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, CompletionError> {
        self.requests
            .lock()
            .expect("请求捕获锁不可中毒")
            .push(request);
        match self.outcomes.lock().expect("序列锁不可中毒").pop_front() {
            Some(Ok(text)) => Ok(CompletionResponse::new(
                vec![AssistantContent::text(text)],
                Usage::new(),
                "fake-summary-provider",
            )),
            Some(Err(message)) => Err(CompletionError::ProviderError(message)),
            None => panic!("摘要请求次数超出预编程序列"),
        }
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse, CompletionError> {
        Err(CompletionError::ProviderError(
            "stream 路径未被 summarize 消费".to_owned(),
        ))
    }
}

/// 小窗防线（与 context_test 同底座）：保护窗止于大块 filler 之前，中段非空。
fn defense() -> ContextDefense {
    ContextDefense::resolve(Some(64))
}

/// 摘要史 fixture：[首条 user 任务书, assistant 上一轮答, 大块 filler]。
/// filler 使保护窗起点止于 index 2 → 待摘要段 = [1..2] 非空。
fn summary_history() -> Vec<Message> {
    vec![
        Message::user("任务书"),
        Message::assistant("上一轮答（含决策与产物）"),
        Message::user("a".repeat(200_000)),
    ]
}

/// 摘史内 user 消息的纯文本拼接（chat_history 断言缝的文本抽取）。
fn user_text(message: &Message) -> String {
    match message {
        Message::User { content } => content
            .iter()
            .filter_map(|item| match item {
                rig::message::UserContent::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// 正向：成功新史形态
// ---------------------------------------------------------------------------

#[tokio::test]
async fn summarize成功新史形态为摘要置顶加首条user加最近轮对() {
    let model = SummaryModel::with_outcomes(vec![Ok("要点摘要正文".to_owned())]);
    let history = summary_history();

    let compacted = summarize(&model, &history, &defense())
        .await
        .expect("摘要成功应返回新史");

    // 三段形态逐位断言：[摘要（[历史摘要] 头 user 置顶）, 原首条 user, 最近轮对]
    assert_eq!(compacted.len(), 3, "新史三段：摘要 + 首条 + 保护窗");
    assert!(
        user_text(&compacted[0]).starts_with("[历史摘要]\n"),
        "摘要消息带 [历史摘要] 头置顶: {:?}",
        user_text(&compacted[0])
    );
    assert!(
        user_text(&compacted[0]).contains("要点摘要正文"),
        "摘要正文逐字收进置顶消息"
    );
    assert!(
        matches!(compacted[0], Message::User { .. }),
        "摘要以 user 消息承载"
    );
    assert_eq!(compacted[1], history[0], "原首条 user 原位保留");
    assert_eq!(compacted[2], history[2], "保护窗内容原样殿后");
}

#[tokio::test]
async fn summarize摘要请求禁工具且携摘要器preamble() {
    let model = SummaryModel::with_outcomes(vec![Ok("摘要".to_owned())]);

    summarize(&model, &summary_history(), &defense())
        .await
        .expect("摘要成功");

    let requests = model.captured_requests();
    assert_eq!(requests.len(), 1, "恰一次摘要请求（首次成功）");
    let request = &requests[0];
    assert!(
        request.tools.is_empty(),
        "摘要请求不携带工具定义面（禁工具口径）"
    );
    assert!(request.documents.is_empty(), "摘要请求无文档面");
    assert_eq!(
        request.preamble.as_deref(),
        Some("你是对话历史摘要器：只输出摘要正文，不执行任何工具。"),
        "摘要器 preamble（只输出摘要正文语义）"
    );
}

#[tokio::test]
async fn summarize摘要prompt组装含前缀形态与要点集关键词() {
    let model = SummaryModel::with_outcomes(vec![Ok("摘要".to_owned())]);

    summarize(&model, &summary_history(), &defense())
        .await
        .expect("摘要成功");

    let request = &model.captured_requests()[0];
    assert_eq!(request.chat_history.len(), 1, "摘要请求单条 user 提示");
    let prompt = user_text(&request.chat_history.first().expect("提示词在场"));
    // 前缀形态（中文要点指令，措辞自研组装层——不逐字全量比对）
    assert!(
        prompt.starts_with("这是历史摘要，请勿重复已完成的工作"),
        "前缀形态: {prompt}"
    );
    // 要点集关键词（已完成工作 / 决策及其原因 / 下一步）
    for keyword in ["已完成的工作", "决策及其原因", "下一步"] {
        assert!(prompt.contains(keyword), "要点集含「{keyword}」: {prompt}");
    }
    // 待摘要历史段：中段转录以 JSON 序列化收进提示
    assert!(prompt.contains("<待摘要历史>"), "待摘要段定界: {prompt}");
    assert!(prompt.contains("上一轮答"), "中段转录内容在场: {prompt}");
}

// ---------------------------------------------------------------------------
// 异常：失败一次重试 / 两次失败交降级 / 空史 / 空摘要
// ---------------------------------------------------------------------------

#[tokio::test]
async fn summarize失败一次重试后成功_恰发出两次摘要请求() {
    let model = SummaryModel::with_outcomes(vec![
        Err("端点过载".to_owned()),
        Ok("重试后的摘要".to_owned()),
    ]);

    let compacted = summarize(&model, &summary_history(), &defense())
        .await
        .expect("一次重试后应成功");

    assert_eq!(
        model.captured_requests().len(),
        2,
        "失败一次重试：恰 2 次请求"
    );
    assert!(
        user_text(&compacted[0]).contains("重试后的摘要"),
        "返回成功新史（重试产物置顶）"
    );
}

#[tokio::test]
async fn summarize两次失败交降级err_run收敛归loop编排() {
    let model = SummaryModel::with_outcomes(vec![
        Err("端点过载".to_owned()),
        Err("端点仍然过载".to_owned()),
    ]);

    let error = summarize(&model, &summary_history(), &defense())
        .await
        .expect_err("两次失败必须 Err（降级编排便归 loop 承载）");

    assert!(error.contains("摘要失败"), "Err 记因指向摘要失败: {error}");
    assert_eq!(model.captured_requests().len(), 2, "恰 2 次（含一次重试）");
}

#[tokio::test]
async fn summarize空史与无中段史显式err零请求不panic() {
    // 空史：无首条 user 可保
    let model = SummaryModel::with_outcomes(vec![]);
    let error = summarize(&model, &[], &defense())
        .await
        .expect_err("空史必须显式 Err");
    assert!(error.contains("空历史"), "记因: {error}");

    // 仅首条 user（无中段可摘要）：同口径显式 Err
    let single = vec![Message::user("任务书")];
    let error = summarize(&model, &single, &defense())
        .await
        .expect_err("无中段史必须显式 Err");
    assert!(error.contains("无可摘要段"), "记因: {error}");
    assert_eq!(model.captured_requests().len(), 0, "两种形态均零摘要请求");
}

#[tokio::test]
async fn summarize空摘要文本按实现定稿重试一次后err() {
    // 实现定稿锁定：空文本无置换资格 → 同空响应/请求失败一列，重试一次后 Err
    let model = SummaryModel::with_outcomes(vec![Ok("   ".to_owned()), Ok("  ".to_owned())]);

    let error = summarize(&model, &summary_history(), &defense())
        .await
        .expect_err("空摘要文本必须 Err（不产空新史）");

    assert!(error.contains("摘要失败"), "记因: {error}");
    assert_eq!(model.captured_requests().len(), 2, "空文本触发一次重试");
}
