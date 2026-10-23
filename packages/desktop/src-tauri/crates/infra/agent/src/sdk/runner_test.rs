use std::path::PathBuf;
use std::sync::Arc;

use agent::{
    AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunner,
    AgentStartError, ModelLevel, SessionCtx, SessionInjections, SessionOpen, SessionRef,
};

use crate::sdk::config::{EngineConfig, Protocol};
use crate::sdk::model_spec::{parse_model_spec, BETA_CONTEXT_1M, MILLION_CONTEXT_WINDOW};
use crate::sdk::runner::{
    anthropic_provider, build_model, openai_provider, resolve_output_tokens, SdkRunner,
    MIN_OUTPUT_TOKENS,
};
use crate::{EngineFacade, EngineKind, ResumeTranscript};
use rig::driver::DynModel;
use rig::operation::Completion;
use rig::providers::anthropic::AnthropicConfig;

// ---------------------------------------------------------------------------
// 装置：配置底座、open 入参底座、转录装载缝
// ---------------------------------------------------------------------------

/// 端点指向本机 discard 端口（恒拒连，无网络触达真实 provider）。
fn complete_config() -> EngineConfig {
    EngineConfig {
        protocol: Protocol::Openai,
        api_key: "sk-test".to_owned(),
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        model_high: "rig-test-model-high".to_owned(),
        model_low: "rig-test-model-low".to_owned(),
    }
}

fn open_new() -> SessionOpen {
    SessionOpen {
        injections: SessionInjections::default(),
        ctx: SessionCtx {
            workspace_root: PathBuf::from("D:\\工作区"),
            permission_mode: AgentPermissionMode::BypassPermissions,
            model_level: ModelLevel::High,
        },
        session: SessionRef::New,
        prior_handle: None,
    }
}

/// 指定模型档位的 open 入参（Low 档选型断言缝；其余面同 [`open_new`]）。
fn open_new_at_level(level: ModelLevel) -> SessionOpen {
    SessionOpen {
        ctx: SessionCtx {
            model_level: level,
            ..open_new().ctx
        },
        ..open_new()
    }
}

fn open_continue(session_id: &str) -> SessionOpen {
    SessionOpen {
        session: SessionRef::Continue {
            id: session_id.to_owned(),
        },
        prior_handle: Some(session_id.to_owned()),
        ..open_new()
    }
}

/// 全史转录装载缝替身（内存三形态：Ok(Some 全史) / Ok(None) / Err）。
fn loader(
    result: Result<Option<Vec<agent::AgentEvent>>, String>,
) -> (ResumeTranscript, Arc<std::sync::atomic::AtomicUsize>) {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let resume: ResumeTranscript = Arc::new(move |session_id: &str| {
        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert!(!session_id.is_empty(), "装载缝以会话 id 寻址");
        match &result {
            Ok(events) => Ok(events.clone()),
            Err(error) => Err(error.clone()),
        }
    });
    (resume, calls)
}

/// 两枚密封往返的最小全史转录（重建非空的前提）。
fn minimal_transcript() -> Vec<agent::AgentEvent> {
    use agent::{AgentEvent, AgentEventKind};
    vec![
        AgentEvent::stamp(
            0,
            AgentEventKind::Message {
                role: AgentMessageRole::User,
                blocks: vec![AgentBlock::Text {
                    text: "上一轮问".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![AgentBlock::Text {
                    text: "上一轮答".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
    ]
}

/// 超水位全史转录：工具轮回灌形态（assistant tool_use + 超门槛老 tool_result
/// 约 25k tokens）+ 大块 filler user（约 50k tokens，越过保护窗）——重建史
/// 触发第一调用点 L2 先行剪裁的底座。
fn oversized_transcript() -> Vec<agent::AgentEvent> {
    use agent::{AgentEvent, AgentEventKind};
    vec![
        AgentEvent::stamp(
            0,
            AgentEventKind::Message {
                role: AgentMessageRole::User,
                blocks: vec![AgentBlock::Text {
                    text: "上一轮问".to_owned(),
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            1,
            AgentEventKind::Message {
                role: AgentMessageRole::Assistant,
                blocks: vec![AgentBlock::ToolUse {
                    id: "tu_big".to_owned(),
                    name: "read".to_owned(),
                    input: serde_json::json!({ "path": "big.txt" }),
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            2,
            AgentEventKind::Message {
                role: AgentMessageRole::Tool,
                blocks: vec![AgentBlock::ToolResult {
                    id: "tu_big".to_owned(),
                    content: "x".repeat(100_000),
                    is_error: false,
                }],
                parent_tool_use_id: None,
            },
        ),
        AgentEvent::stamp(
            3,
            AgentEventKind::Message {
                role: AgentMessageRole::User,
                blocks: vec![AgentBlock::Text {
                    text: "f".repeat(200_000),
                }],
                parent_tool_use_id: None,
            },
        ),
    ]
}

// ---------------------------------------------------------------------------
// open 段：配置三件套校验（启动失败不产生记录）
// ---------------------------------------------------------------------------

#[test]
fn 空缺省配置open返回config_missing且三成因逐字可辨() {
    let runner = SdkRunner::new(EngineConfig::empty(), None, None, None);

    let error = runner.open_session(open_new()).expect_err("空配置必须 Err");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("api_key") && message.contains("base_url") && message.contains("model"),
        "三成因逐字在列，实际: {message}"
    );
}

#[test]
fn 单字段缺失open消息区分api_key_base_url双档model成因() {
    let cases = [
        (
            "api_key",
            EngineConfig {
                protocol: Protocol::Openai,
                api_key: String::new(),
                base_url: "http://127.0.0.1:9/v1".to_owned(),
                model_high: "m-h".to_owned(),
                model_low: "m-l".to_owned(),
            },
            ModelLevel::High,
        ),
        (
            "base_url",
            EngineConfig {
                protocol: Protocol::Openai,
                api_key: "k".to_owned(),
                base_url: String::new(),
                model_high: "m-h".to_owned(),
                model_low: "m-l".to_owned(),
            },
            ModelLevel::High,
        ),
        (
            "model_high",
            EngineConfig {
                protocol: Protocol::Openai,
                api_key: "k".to_owned(),
                base_url: "http://127.0.0.1:9/v1".to_owned(),
                model_high: String::new(),
                model_low: "m-l".to_owned(),
            },
            ModelLevel::High,
        ),
        (
            "model_low",
            EngineConfig {
                protocol: Protocol::Openai,
                api_key: "k".to_owned(),
                base_url: "http://127.0.0.1:9/v1".to_owned(),
                model_high: "m-h".to_owned(),
                model_low: String::new(),
            },
            ModelLevel::Low,
        ),
    ];
    for (field, config, level) in cases {
        let runner = SdkRunner::new(config, None, None, None);
        let error = runner
            .open_session(open_new_at_level(level))
            .expect_err("缺项必须 Err");
        let AgentStartError::ConfigMissing(message) = &error else {
            panic!("应为 ConfigMissing，实际: {error:?}");
        };
        assert!(message.contains(field), "{field} 成因可辨，实际: {message}");
        // 其余成因不在消息中（只列缺失项——双档互不误列）
        for other in ["api_key", "base_url", "model_high", "model_low"] {
            if other != field {
                assert!(
                    !message.contains(other),
                    "{field} 缺失时不列 {other}，实际: {message}"
                );
            }
        }
    }
}

/// 低档缺配的半配 provider：仅 Low 档轮显式失败，High 档轮 open 照常
///（逐档解耦——选型后缺档才计缺失）。
#[tokio::test]
async fn 低档缺配仅低档轮失败_高档轮open照常() {
    let half = || EngineConfig {
        protocol: Protocol::Openai,
        model_low: String::new(),
        ..complete_config()
    };
    let error = SdkRunner::new(half(), None, None, None)
        .open_session(open_new_at_level(ModelLevel::Low))
        .expect_err("低档轮缺 model_low 必须 Err");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("model_low") && !message.contains("model_high"),
        "缺档文案点名低档不误列高档，实际: {message}"
    );
    let _session = SdkRunner::new(half(), None, None, None)
        .open_session(open_new_at_level(ModelLevel::High))
        .expect("High 档轮照常 open（半配 provider 高档可用）");
}

// ---------------------------------------------------------------------------
// open 段：Continue 装载缝三形态
// ---------------------------------------------------------------------------

#[test]
fn continue装载缝none形态报会话不存在且成因可区分() {
    let (resume, calls) = loader(Ok(None));
    let runner = SdkRunner::new(complete_config(), Some(resume), None, None);

    let error = runner
        .open_session(open_continue("ses-404"))
        .expect_err("loader None 必须 Err");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("会话不存在") && message.contains("ses-404"),
        "None 成因（会话缺失）携 id 记因，实际: {message}"
    );
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "装载缝恰被调用一次"
    );
}

#[test]
fn continue装载缝err形态报转录读取失败并记因() {
    let (resume, _calls) = loader(Err("库读取失败".to_owned()));
    let runner = SdkRunner::new(complete_config(), Some(resume), None, None);

    let error = runner
        .open_session(open_continue("ses-err"))
        .expect_err("loader Err 必须 Err");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("转录读取失败") && message.contains("库读取失败"),
        "Err 成因携原错误记因，实际: {message}"
    );
}

/// 空转录（会话存在但从未开轮——explore 建档预建空会话）按空史首轮开跑，
/// 不再视同会话缺失；会话缺失 / 装载失败两成因仍显式失败且互不重合。
#[tokio::test]
async fn continue空转录按空史首轮开跑_缺失与装载失败仍显式失败() {
    let (resume, _calls) = loader(Ok(Some(Vec::new())));
    let runner = SdkRunner::new(complete_config(), Some(resume), None, None);

    let session = runner
        .open_session(open_continue("ses-empty"))
        .expect("空转录会话首轮应开跑（空史开跑，非会话缺失）");
    drop(session);

    // 两形态成因互不重合（可区分）
    let (none_loader, _) = loader(Ok(None));
    let none_message = SdkRunner::new(complete_config(), Some(none_loader), None, None)
        .open_session(open_continue("ses-a"))
        .expect_err("会话不存在必须 Err")
        .to_string();
    let (err_loader, _) = loader(Err("读取失败".to_owned()));
    let err_message = SdkRunner::new(complete_config(), Some(err_loader), None, None)
        .open_session(open_continue("ses-b"))
        .expect_err("装载失败必须 Err")
        .to_string();
    assert!(
        none_message.contains("会话不存在"),
        "会话缺失成因，实际: {none_message}"
    );
    assert!(
        err_message.contains("转录读取失败"),
        "装载失败成因，实际: {err_message}"
    );
    assert_ne!(none_message, err_message);
}

#[test]
fn continue装载器未注入时显式失败不静默空史() {
    // resume 缝未注入（直构 runner 无 loader）的 Continue：显式 ConfigMissing
    let runner = SdkRunner::new(complete_config(), None, None, None);
    let error = runner
        .open_session(open_continue("ses-noloader"))
        .expect_err("装载器未注入必须 Err");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("装载器未注入"),
        "显式失败（不静默按空史续跑），实际: {message}"
    );
}

#[tokio::test]
async fn continue全史装载重建后open成功且new会话零装载调用() {
    let (resume, calls) = loader(Ok(Some(minimal_transcript())));
    let runner = SdkRunner::new(complete_config(), Some(resume), None, None);

    // Continue：全史转录重建非空 → open 成功（全史装载半边；重建史注入首轮
    // 请求由 loop_test 捕获缝承载）
    let session = runner
        .open_session(open_continue("ses-hist"))
        .expect("Continue 全史装载应成功");
    drop(session);

    // New：全新运行空史，装载缝零调用
    let (new_resume, new_calls) = loader(Ok(Some(minimal_transcript())));
    let new_runner = SdkRunner::new(complete_config(), Some(new_resume), None, None);
    new_runner.open_session(open_new()).expect("New 会话应成功");
    assert_eq!(
        new_calls.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "New 不触装载缝"
    );
    assert!(
        calls.load(std::sync::atomic::Ordering::Relaxed) >= 1,
        "Continue 触发装载缝"
    );
}

// ---------------------------------------------------------------------------
// 上下文窗防线装配（AC-6 resume 重建后第一调用点 + AC-9 窗长承接半边）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn continue携超门槛工具结果的小窗runner_防线notice先于runstarted() {
    let (resume, _calls) = loader(Ok(Some(oversized_transcript())));
    // 显式小窗：window = 64 → L2 水位 48 tokens，重建史必然越线
    let runner = SdkRunner::new(complete_config(), Some(resume), Some(64), None);

    let mut session = runner
        .open_session(open_continue("ses-defense"))
        .expect("超水位重建史防线剪裁后 open 仍成功");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "续会话一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    // 事件序：SystemNotice{context_pruned}（重建史防线留痕，泵内先行转发）先于
    // RunStarted（同任务同通道 FIFO 保序）
    let notice = session.observations.recv().await.expect("防线 notice");
    let AgentEventKind::SystemNotice {
        subtype, payload, ..
    } = &notice
    else {
        panic!("首事件应为防线 notice，实际: {notice:?}");
    };
    assert_eq!(subtype.as_str(), "context_pruned", "重建史 L2 剪裁留痕");
    let payload = serde_json::to_value(payload).expect("payload 序列化");
    assert_eq!(payload["layer"], serde_json::json!("l2"), "layer 记剪裁层");
    assert!(
        payload["after"].as_u64() < payload["before"].as_u64(),
        "重建史剪裁有效减重: {payload}"
    );

    let started = session.observations.recv().await.expect("RunStarted");
    assert!(
        matches!(&started, AgentEventKind::RunStarted { .. }),
        "防线 notice 先于 RunStarted，实际: {started:?}"
    );
    drop(session);
}

#[tokio::test]
async fn 防线continue时全史事件仍全量进入重建_装载缝无写通道零反写() {
    let transcript = oversized_transcript();
    let (resume, calls) = loader(Ok(Some(transcript.clone())));
    let runner = SdkRunner::new(complete_config(), Some(resume), Some(64), None);

    let mut session = runner
        .open_session(open_continue("ses-full"))
        .expect("open 应成功");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "全量重建一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    // notice 载荷 before == 全史重建史的估算 tokens：防线只作用请求史，
    // 装载缝返回的全量事件完整进入重建（未因防线缩水）
    let notice = session.observations.recv().await.expect("防线 notice");
    let AgentEventKind::SystemNotice { payload, .. } = &notice else {
        panic!("应为防线 notice，实际: {notice:?}");
    };
    let payload = serde_json::to_value(payload).expect("payload 序列化");
    let rebuilt = crate::sdk::resume::rebuild(&transcript).expect("重建应成功");
    assert_eq!(
        payload["before"],
        serde_json::json!(crate::sdk::context::estimate_history(&rebuilt)),
        "before == 全史重建估算（全量事件进入防线）"
    );
    // 装载缝无写通道：调用计数恰一（读侧单点，无反写）
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "装载缝恰调用一次（无写通道零反写）"
    );
    drop(session);
}

// ---------------------------------------------------------------------------
// ask 泵：未盖戳观察词汇与流失败收敛（真实泵 + 拒连端点）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn runstarted_tools恰七工具清单() {
    use crate::sdk::tools::TOOL_NAMES;

    let runner = SdkRunner::new(complete_config(), None, None, None);
    let mut session = runner.open_session(open_new()).expect("open 应成功");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "工具清单一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    let started = session.observations.recv().await.expect("RunStarted");
    let AgentEventKind::RunStarted {
        tools, mcp_servers, ..
    } = &started
    else {
        panic!("首事件应为 RunStarted，实际: {started:?}");
    };
    assert_eq!(
        tools.as_slice(),
        TOOL_NAMES.as_slice(),
        "RunStarted.tools 与 TOOL_NAMES 同源（恰七工具，bash 在册）"
    );
    assert_eq!(tools.len(), 7, "恰七工具");
    assert!(tools.contains(&"bash".to_owned()), "bash 在工具清单内");
    assert!(mcp_servers.is_empty(), "MCP 恒空（本期无 MCP 面）");
    drop(session);
}

#[tokio::test(start_paused = true)]
async fn ask泵观察回流未盖戳词汇_变体序以收敛收尾且remote_id_sdk前缀上报() {
    let runner = SdkRunner::new(complete_config(), None, None, None);
    let mut session = runner.open_session(open_new()).expect("open 应成功");

    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "帮我跑一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    // 未盖戳观察词汇：泵产出 agent::AgentEventKind（盖戳收内核）。有界读取：
    // 泵一轮一命，拒连端点恒失败——重试护栏逐次留痕（退避经 paused 时钟快
    // 进）后耗尽失败收敛，本轮恒产 7 事件后泵任务返回（观测通道随之关闭）
    let mut events = Vec::new();
    for _ in 0..7 {
        events.push(session.observations.recv().await.expect("本轮事件"));
    }

    // 变体序：RunStarted 先导 → user 密封提示词 → api_retry × 3 逐次留痕 →
    // api_error 记因 → TurnDone 收敛（拒连端点：重试耗尽即收敛，delta 面由
    // loop_test 假流承载）
    assert!(
        matches!(&events[0], AgentEventKind::RunStarted { .. }),
        "RunStarted 先导，实际: {:?}",
        events.first()
    );
    assert!(matches!(
        &events[1],
        AgentEventKind::Message { role, .. } if role == &AgentMessageRole::User
    ));
    for (index, event) in events[2..5].iter().enumerate() {
        assert!(
            matches!(event, AgentEventKind::SystemNotice { subtype, .. } if subtype == "api_retry"),
            "第 {} 次重试留痕应为 api_retry，实际: {event:?}",
            index + 1
        );
    }
    assert!(matches!(
        &events[5],
        AgentEventKind::SystemNotice { subtype, .. } if subtype == "api_error"
    ));
    let AgentEventKind::TurnDone {
        is_error, subtype, ..
    } = events.last().expect("非空")
    else {
        panic!("事件流恒以 TurnDone 收敛，实际: {:?}", events.last());
    };
    assert!(*is_error, "拒连端点失败收敛");
    assert_eq!(subtype, "api_error");

    // remote id 铸造上报（双 id 映射的来源半边）
    for event in &events {
        match event {
            AgentEventKind::RunStarted { session_id, .. }
            | AgentEventKind::TurnDone { session_id, .. } => {
                let remote = session_id.as_deref().expect("remote id 在场");
                assert!(
                    remote.starts_with("sdk-"),
                    "sdk- 前缀 remote id 每轮铸造，实际: {remote}"
                );
            }
            _ => {}
        }
    }

    // 一轮一命收口：单问服务完毕即泵任务返回 → observations 发送端全 drop
    // → 观测通道关闭（内核 EOF 收口缝：recv 返回 None）
    assert!(
        session.observations.recv().await.is_none(),
        "单轮收敛后观测通道必须关闭（EOF 收口缝）"
    );
    // 第二问送达被拒：泵已终止，questions 接收端随之 drop
    assert!(
        session
            .questions
            .send(agent::TurnQuestion {
                prompt: "第二问".to_owned(),
            })
            .await
            .is_err(),
        "一轮一命：泵终止后第二问送达必须被拒（接收端已 drop）"
    );
}

#[tokio::test]
async fn 停止先置位时泵select停止臂命中_future_drop不合成收敛() {
    let runner = SdkRunner::new(complete_config(), None, None, None);
    let mut session = runner.open_session(open_new()).expect("open 应成功");
    session.handle.request_stop(); // 停止先置位

    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "不会执行的一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    // 停止先置位：loop 轮间快速路径在先导事件后终止（select 停止臂 / 快速路径
    // 两机制任一命中即不进入流消费）。有界读取：泵一轮一命，本轮恒产
    // RunStarted + user 密封提示词两枚先导事件后泵任务返回
    let mut events = Vec::new();
    for _ in 0..2 {
        events.push(session.observations.recv().await.expect("先导事件"));
    }
    assert!(matches!(&events[0], AgentEventKind::RunStarted { .. }));
    assert!(matches!(
        &events[1],
        AgentEventKind::Message { role, .. } if role == &AgentMessageRole::User
    ));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEventKind::TurnDone { .. })),
        "停止先置位：不合成 TurnDone，实际: {events:?}"
    );
    // 一轮一命收口：停止截停后泵任务返回 → observations 发送端全 drop →
    // 观测通道关闭（先导两枚之后无后续产出）
    assert!(
        session.observations.recv().await.is_none(),
        "停止截停后泵终止，观测通道必须关闭"
    );
}

#[tokio::test(start_paused = true)]
async fn 停止晚于eof时泵已收敛无二次收敛事件() {
    let runner = SdkRunner::new(complete_config(), None, None, None);
    let mut session = runner.open_session(open_new()).expect("open 应成功");

    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "跑一轮".to_owned(),
        })
        .await
        .expect("问题送达");
    // 本轮事件有界读取至收敛事件（泵一轮一命，收敛即泵返回；拒连端点恒失
    // 败：重试护栏 3 次留痕 + 记因 + 收敛共 7 事件，退避经 paused 时钟快进）
    let mut events = Vec::new();
    for _ in 0..7 {
        events.push(session.observations.recv().await.expect("本轮事件"));
    }
    assert!(matches!(
        events.last(),
        Some(AgentEventKind::TurnDone { .. })
    ));
    // 一轮一命收口：单轮收敛后泵任务已返回，观测通道关闭
    assert!(
        session.observations.recv().await.is_none(),
        "单轮收敛后观测通道必须关闭"
    );

    // EOF（本轮收敛）后停止：无二次收敛事件（下一轮不再被驱动——泵已随单轮
    // 服务终止，问题通道随接收端 drop 已关闭）
    session.handle.request_stop();
    drop(session);
}

// ---------------------------------------------------------------------------
// 门面分发一致性
// ---------------------------------------------------------------------------

#[test]
fn 门面sdk分发的启动校验行为与直构runner一致() {
    // 空配置：门面产物与直构 runner 同样以 ConfigMissing 拒绝且消息一致
    let via_facade = EngineFacade::new()
        .runner_for(EngineKind::Sdk, EngineConfig::empty())
        .expect("Sdk 臂构造成功");
    let via_direct = SdkRunner::new(EngineConfig::empty(), None, None, None);

    let facade_error = via_facade
        .open_session(open_new())
        .expect_err("门面产物空配置必须 Err");
    let direct_error = via_direct
        .open_session(open_new())
        .expect_err("直构 runner 空配置必须 Err");
    assert_eq!(
        facade_error, direct_error,
        "门面分发一致性：校验行为与直构一致"
    );

    // 携装载缝的门面：Continue 校验同样一致
    let (resume, _) = loader(Ok(None));
    let with_resume = EngineFacade::with_resume_transcript(resume)
        .runner_for(EngineKind::Sdk, complete_config())
        .expect("Sdk 臂构造成功");
    let error = with_resume
        .open_session(open_continue("ses-facade"))
        .expect_err("门面 Continue 校验必须 Err");
    assert!(matches!(error, AgentStartError::ConfigMissing(_)));
}

// ---------------------------------------------------------------------------
// auto-named-explore：第四参 store 句柄构造 + explore 注入经 open_session
// 端到端（泵铸造 → loop 下发面）
// ---------------------------------------------------------------------------

use agent::SessionToolset;
use store::Store;

/// 真实临时 workspace 库句柄装置（tempfile + open_workspace，不 mock）。
fn temp_store(tag: &str) -> (tempfile::TempDir, Arc<Store>) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("sdk-runner-explore-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败");
    let db_path = dir.path().join("ws.redb");
    let store = Arc::new(
        Store::open_workspace(&db_path)
            .unwrap_or_else(|e| panic!("打开临时 workspace 库失败: {e}")),
    );
    (dir, store)
}

/// explore 来源注入底座（六名收窄 + Explore toolset）。
fn explore_injections() -> SessionInjections {
    SessionInjections {
        preamble: None,
        tools: Some(vec![
            "read".to_owned(),
            "grep".to_owned(),
            "glob".to_owned(),
            "ls".to_owned(),
            "edit".to_owned(),
            "bash".to_owned(),
        ]),
        toolset: Some(SessionToolset::Explore),
    }
}

/// 第四参构造（`Option<Arc<Store>>`）：`Some(句柄)` 构造成功且既有三参调用面
/// 以 `None` 兼容（构造 + 引擎选择面编译期锚定——本文件既有用例即三参 None
/// 形态的编译锚）。
#[test]
fn sdk_runner第四参构造_some句柄与none缺省均编译可达() {
    let (_dir, store) = temp_store("ctor");
    // Some(句柄)：构造成功
    let with_store = SdkRunner::new(complete_config(), None, None, Some(Arc::clone(&store)));
    let _ = with_store;
    // None 缺省：既有三参语义面（构造 + open 失败面同旧口径）
    let without_store = SdkRunner::new(complete_config(), None, None, None);
    let error = without_store
        .open_session(open_continue("ses-ctor-none"))
        .expect_err("无装载缝 Continue 必须显式失败");
    assert!(
        matches!(error, AgentStartError::ConfigMissing(_)),
        "None 句柄不改变既有失败面，实际: {error:?}"
    );
}

/// explore 注入经 open_session 端到端（链路：open_session → 泵铸造 → loop 下
/// 发面）：Continue 预建空会话 + `injections.toolset = Explore` + 句柄在场 →
/// `RunStarted.tools` 报六工具 + `create_explore_note`（泵内 LoopTurn 铸造贯
/// 通的行为面证明；模型 IO 走 discard 端点，RunStarted 先于 IO 可观察）。
#[tokio::test]
async fn explore注入open_session端到端_runstarted报六工具加专用工具() {
    use crate::sdk::tools::EXPLORE_NOTE_TOOL;

    let (dir, store) = temp_store("e2e");
    let (resume, _calls) = loader(Ok(Some(Vec::new()))); // 预建空会话：空史开跑
    let runner = SdkRunner::new(
        complete_config(),
        Some(resume),
        None,
        Some(Arc::clone(&store)),
    );

    let mut open = open_continue("ses-explore-e2e");
    open.injections = explore_injections();
    let mut session = runner.open_session(open).expect("explore 注入 open 应成功");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "探索首轮".to_owned(),
        })
        .await
        .expect("问题送达");

    let started = session.observations.recv().await.expect("RunStarted");
    let AgentEventKind::RunStarted {
        tools, mcp_servers, ..
    } = &started
    else {
        panic!("首事件应为 RunStarted，实际: {started:?}");
    };
    assert_eq!(
        tools,
        &[
            "read".to_owned(),
            "grep".to_owned(),
            "glob".to_owned(),
            "ls".to_owned(),
            "edit".to_owned(),
            "bash".to_owned(),
            EXPLORE_NOTE_TOOL.to_owned(),
        ],
        "explore 轮实际下发面：六工具保序 + 末位 create_explore_note"
    );
    assert!(
        !tools.contains(&"write".to_owned()),
        "explore 工具面不含 write（AC-5 硬约束的接线半边）"
    );
    assert!(mcp_servers.is_empty(), "MCP 恒空");
    drop(session);
    drop(dir);
    drop(store);
}

/// default 注入零专用上下文：`toolset = None`（debug 来源装配缺省）→ 轮内不
/// 铸专用上下文、`RunStarted.tools` 报七名（非 explore 会话零变化，AC-7 接线
/// 半边）。
#[tokio::test]
async fn default注入零专用上下文_runstarted报七名() {
    let (_dir, store) = temp_store("default");
    let runner = SdkRunner::new(complete_config(), None, None, Some(Arc::clone(&store)));

    let mut session = runner.open_session(open_new()).expect("open 应成功");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "debug 一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    let started = session.observations.recv().await.expect("RunStarted");
    let AgentEventKind::RunStarted { tools, .. } = &started else {
        panic!("首事件应为 RunStarted，实际: {started:?}");
    };
    assert_eq!(
        tools.as_slice(),
        crate::sdk::tools::TOOL_NAMES.as_slice(),
        "default 轮工具面与七通用清单同源同序（含 write，AC-7）"
    );
    assert!(
        !tools.contains(&"create_explore_note".to_owned()),
        "default 轮不追加专用工具"
    );
    drop(session);
}

/// 空配置失败面回归：`EngineConfig::empty()` + 句柄在场 → `open_session` 仍
/// `Err(ConfigMissing)`（配置三件套校验先于泵铸造，新字段不破坏既有失败面）。
#[test]
fn 空配置加句柄open仍config_missing_校验先于泵铸造() {
    let (_dir, store) = temp_store("empty-cfg");
    let runner = SdkRunner::new(EngineConfig::empty(), None, None, Some(Arc::clone(&store)));
    let error = runner
        .open_session(explore_continue_open("ses-empty-cfg"))
        .expect_err("空配置必须 Err");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("api_key") && message.contains("base_url") && message.contains("model"),
        "配置三件套校验照旧先行，实际: {message}"
    );
}

/// explore 注入形态的 Continue open 底座（空会话 id 装载缝 + explore 注入）。
fn explore_continue_open(session_id: &str) -> SessionOpen {
    SessionOpen {
        injections: explore_injections(),
        ..open_continue(session_id)
    }
}

/// 转录装载失败回归（参数化）：句柄在场 + resume loader `Err`（库读取失败）/
/// `None`（会话不存在）→ `ConfigMissing`，失败语义不随新字段漂移
///（counting_loader 替身驱动）。
#[test]
fn 转录装载失败回归_句柄在场loader两形态仍config_missing() {
    let (_dir, store) = temp_store("loader-fail");

    // Err 形态：库读取失败
    let (err_loader, err_calls) = loader(Err("库读取失败".to_owned()));
    let err_runner = SdkRunner::new(
        complete_config(),
        Some(err_loader),
        None,
        Some(Arc::clone(&store)),
    );
    let err_error = err_runner
        .open_session(explore_continue_open("ses-loader-err"))
        .expect_err("loader Err 必须 Err");
    let AgentStartError::ConfigMissing(err_message) = &err_error else {
        panic!("应为 ConfigMissing，实际: {err_error:?}");
    };
    assert!(
        err_message.contains("转录读取失败") && err_message.contains("库读取失败"),
        "Err 成因携原错误记因，实际: {err_message}"
    );

    // None 形态：会话不存在
    let (none_loader, none_calls) = loader(Ok(None));
    let none_runner = SdkRunner::new(
        complete_config(),
        Some(none_loader),
        None,
        Some(Arc::clone(&store)),
    );
    let none_error = none_runner
        .open_session(explore_continue_open("ses-loader-none"))
        .expect_err("loader None 必须 Err");
    let AgentStartError::ConfigMissing(none_message) = &none_error else {
        panic!("应为 ConfigMissing，实际: {none_error:?}");
    };
    assert!(
        none_message.contains("会话不存在"),
        "None 成因（会话缺失）记因，实际: {none_message}"
    );

    assert_eq!(
        err_calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "Err 形态装载缝恰调用一次"
    );
    assert_eq!(
        none_calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "None 形态装载缝恰调用一次"
    );
}

/// Explore 会话 Continue id 缺席（New 会话形态，生产不可达）：上下文铸
/// `None`、run 照常开跑不 panic（loop 侧「执行上下文未装配」臂的前置半边
/// ——下发面不受影响：definitions 仍按 injections 组装六工具 + 专用工具）。
#[tokio::test]
async fn explore注入new会话continue缺席_上下文none_run照常开跑() {
    let (_dir, store) = temp_store("new-explore");
    let runner = SdkRunner::new(complete_config(), None, None, Some(Arc::clone(&store)));

    // New 会话 + explore 注入 + 句柄在场：生产不可达形态（explore 恒 Continue），
    // 泵 zip(Continue id 缺席) → explore_note = None，run 不 panic
    let mut open = open_new();
    open.injections = explore_injections();
    let mut session = runner
        .open_session(open)
        .expect("New 会话 + explore 注入 open 应成功（不 panic）");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "不可达形态的一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    // 下发面照常：RunStarted.tools 仍为六工具 + create_explore_note（definitions
    // 自 injections 组装，与 explore_note 上下文有无无关）
    let started = session.observations.recv().await.expect("RunStarted");
    let AgentEventKind::RunStarted { tools, .. } = &started else {
        panic!("首事件应为 RunStarted，实际: {started:?}");
    };
    assert!(
        tools.contains(&"create_explore_note".to_owned()) && !tools.contains(&"write".to_owned()),
        "下发面按注入组装（与上下文铸造解耦），实际: {tools:?}"
    );
    // user 密封提示词照常产出（泵链路未因上下文 None 中断）
    let user = session.observations.recv().await.expect("user 密封");
    assert!(
        matches!(&user, AgentEventKind::Message { role, .. } if role == &AgentMessageRole::User),
        "run 照常开跑，实际: {user:?}"
    );
    drop(session);
}

// ---------------------------------------------------------------------------
// 协议 / 上限载荷接线（AC-5 / AC-8 / AC-9 / AC-7）：组装面直断（build_model +
// 两臂 provider 接缝），端点指向本机 discard 端口（恒拒连、不打网络）
// ---------------------------------------------------------------------------

/// anthropic 协议配置底座（端点 discard，模型名按用例覆写）。
fn anthropic_config(high: &str, low: &str) -> EngineConfig {
    EngineConfig {
        protocol: Protocol::Anthropic,
        api_key: "sk-ant-test".to_owned(),
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        model_high: high.to_owned(),
        model_low: low.to_owned(),
    }
}

/// 指定档位的 Continue open 底座（按档窗长 / 按档解析用例的轮级入口）。
fn open_continue_at_level(session_id: &str, level: ModelLevel) -> SessionOpen {
    SessionOpen {
        ctx: SessionCtx {
            model_level: level,
            ..open_continue(session_id).ctx
        },
        ..open_continue(session_id)
    }
}

#[tokio::test]
#[allow(clippy::type_complexity)]
async fn with_max_tokens构造可达且不改变非相关行为() {
    // builder 缝：四参 new（既有调用点零改）+ 旁路上限载荷
    let with_payload =
        SdkRunner::new(complete_config(), None, None, None).with_max_tokens(Some(8192));
    let without_payload = SdkRunner::new(complete_config(), None, None, None);

    // 载荷不改变非相关行为：两条 runner 的启动校验面逐字一致
    let with_error = with_payload
        .open_session(open_continue("ses-payload"))
        .expect_err("无装载缝续会话必须显式失败");
    let without_error = without_payload
        .open_session(open_continue("ses-payload"))
        .expect_err("同口径失败");
    assert_eq!(with_error, without_error, "上限载荷不改变启动校验面");

    // 完整配置 New 会话 open 照常成功（载荷不改变开面）
    drop(
        with_payload
            .open_session(open_new())
            .expect("完整配置 New 会话 open 应成功"),
    );
    // SdkRunner::new 四参调用形态编译可达（36 处零改的编译面锚）
    let _four_args: fn(
        EngineConfig,
        Option<ResumeTranscript>,
        Option<u64>,
        Option<Arc<Store>>,
    ) -> SdkRunner = SdkRunner::new;
}

#[test]
fn min_output_tokens常量逐字为4096且经crate根可达() {
    assert_eq!(MIN_OUTPUT_TOKENS, 4_096, "保存期下限单点逐字 4_096");
    assert_eq!(
        crate::MIN_OUTPUT_TOKENS,
        MIN_OUTPUT_TOKENS,
        "经 crate 根再导出同一常量（保存期与引擎同一门槛，无第二套字面量）"
    );
}

#[test]
fn 双臂组装_openai臂描述面与现状逐字等价() {
    let cfg = complete_config();
    let spec = parse_model_spec(&cfg.model_high).expect("无后缀解析成功");
    let model = build_model(&cfg, &spec, None);

    assert_eq!(model.name(), "openai", "openai 臂 provider 描述名逐字");
    assert_eq!(
        model.id(),
        Some("rig-test-model-high"),
        "openai 臂 id = 模型名原文（无后缀零改写）"
    );

    // 带 `[1m]` 后缀的 openai 档：裸名下发（本地窗长仍取 1M，但不发 beta）
    let suffixed = parse_model_spec("gpt-4o[1m]").expect("后缀解析成功");
    let model = build_model(&cfg, &suffixed, Some(8192));
    assert_eq!(model.name(), "openai");
    assert_eq!(model.id(), Some("gpt-4o"), "openai 臂下发裸名");
}

#[test]
fn 双臂组装_anthropic臂描述名为anthropic且id为裸名() {
    let cfg = anthropic_config("claude-sonnet-4-5[1m]", "claude-haiku-4-5");
    let spec = parse_model_spec(&cfg.model_high).expect("后缀解析成功");
    let model = build_model(&cfg, &spec, None);

    assert_eq!(
        model.name(),
        "anthropic",
        "anthropic 臂 provider 描述名逐字"
    );
    assert_eq!(
        model.id(),
        Some("claude-sonnet-4-5"),
        "anthropic 臂 id = 裸名（后缀已剥离，反映实际发送）"
    );

    // 两臂同型：同一变量位接受两臂产物（编译期锚，loop 零改动可消费）
    let openai_spec = parse_model_spec("gpt-4o").expect("无后缀解析成功");
    let arms: [DynModel<Completion>; 2] = [
        build_model(&complete_config(), &openai_spec, None),
        build_model(&cfg, &spec, None),
    ];
    assert_eq!(arms[0].name(), "openai");
    assert_eq!(arms[1].name(), "anthropic");
}

#[test]
fn base_url归一化_anthropic三形态不产生重复版本段() {
    // 三形态（裸域 / 带 /v1 / 带 /v1/messages）归一化后均无版本段后缀
    for raw in [
        "https://api.anthropic.com",
        "https://api.anthropic.com/v1",
        "https://api.anthropic.com/v1/messages",
    ] {
        let cfg = EngineConfig {
            protocol: Protocol::Anthropic,
            base_url: raw.to_owned(),
            ..anthropic_config("claude-x", "claude-y")
        };
        let provider = anthropic_provider(&cfg, &parse_model_spec("claude-x").expect("解析成功"));
        assert_eq!(
            provider.base_url, "https://api.anthropic.com",
            "{raw} 归一化后不产生重复版本段（rig 拼 /v1/messages）"
        );
        assert!(
            !provider.base_url.ends_with("/v1"),
            "{raw} 归一化后不带版本段，实际: {}",
            provider.base_url
        );
    }
}

#[test]
fn base_url归一化_openai臂保持原样拼接() {
    // openai 契约要求含版本段：引擎不改写（逐字等价现状）
    for raw in [
        "http://127.0.0.1:9/v1",
        "https://api.example.com/v1",
        "https://gateway.example.com/openai/v1",
    ] {
        let cfg = EngineConfig {
            base_url: raw.to_owned(),
            ..complete_config()
        };
        assert_eq!(
            openai_provider(&cfg).base_url,
            raw,
            "{raw} openai 臂 base_url 原样承接"
        );
    }
    // 尾部斜杠同样原样（引擎不加不减）
    let cfg = EngineConfig {
        base_url: "https://api.example.com/v1/".to_owned(),
        ..complete_config()
    };
    assert_eq!(
        openai_provider(&cfg).base_url,
        "https://api.example.com/v1/"
    );
}

#[tokio::test]
async fn 裸名上报_runstarted报裸名而非配置原文() {
    let cfg = anthropic_config("claude-x[1m]", "claude-y");
    let runner = SdkRunner::new(cfg.clone(), None, None, None);
    let mut session = runner.open_session(open_new()).expect("open 应成功");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "裸名上报一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    let started = session.observations.recv().await.expect("RunStarted");
    let AgentEventKind::RunStarted { model, .. } = &started else {
        panic!("首事件应为 RunStarted，实际: {started:?}");
    };
    assert_eq!(
        model.as_deref(),
        Some("claude-x"),
        "RunStarted.model 报裸名（反映实际发送，后缀不随请求下发）"
    );
    assert_ne!(
        model.as_deref(),
        Some(cfg.model_high.as_str()),
        "上报值不是配置原文"
    );
    drop(session);
}

#[test]
fn 输出上限取值链_显式优先_名单内保留官方值_名单外兜底32000() {
    // rig 名单内模型的官方默认值（claude-opus-4* / claude-sonnet-4* /
    // claude-haiku-4-5 走 64_000）
    let rig_default_of = |model: &str| {
        let provider = AnthropicConfig::new("k").with_base_url("http://127.0.0.1:9/v1");
        provider.client().completion(model).wire.default_max_tokens
    };

    // ① provider 显式值优先（不被 rig 官方值覆盖）
    let in_list = rig_default_of("claude-opus-4-1");
    assert_eq!(in_list, Some(64_000), "名单内模型有 rig 官方默认值");
    assert_eq!(
        resolve_output_tokens(Some(8192), in_list),
        8192,
        "显式值优先（MUST NOT 被官方默认值覆盖）"
    );

    // ② None + 名单内：保留 rig 官方值（不降级）
    assert_eq!(
        resolve_output_tokens(None, in_list),
        64_000,
        "名单内模型保留官方默认值"
    );
    assert_eq!(
        resolve_output_tokens(None, rig_default_of("claude-sonnet-4-5")),
        64_000,
        "claude-sonnet-4* 名单内"
    );
    assert_eq!(
        resolve_output_tokens(None, rig_default_of("claude-haiku-4-5")),
        64_000,
        "claude-haiku-4-5 名单内"
    );

    // ③ None + 名单外（老代 / 第三方网关名）：兜底 32_000
    for out_of_list in ["claude-3-5-sonnet-20241022", "my-gateway-model-v2"] {
        let rig_default = rig_default_of(out_of_list);
        assert_eq!(rig_default, None, "{out_of_list} 不在 rig 名单内");
        assert_eq!(
            resolve_output_tokens(None, rig_default),
            32_000,
            "{out_of_list} 兜底 32_000（不再触发编码期 max_tokens must be set）"
        );
    }

    // 门面零下限校验：极小值原样作为 wire 默认值（拒面在命令层）
    assert_eq!(resolve_output_tokens(Some(0), None), 0, "Some(0) 原样透传");
    assert_eq!(
        resolve_output_tokens(Some(u64::MAX), None),
        u64::MAX,
        "u64::MAX 原样透传（引擎不做上限校验）"
    );

    // anthropic 臂构造不 panic（上限写入 wire 的接线面）
    let cfg = anthropic_config("claude-x", "claude-y");
    let spec = parse_model_spec("claude-x").expect("解析成功");
    for max_tokens in [None, Some(0), Some(4096), Some(u64::MAX)] {
        assert_eq!(
            build_model(&cfg, &spec, max_tokens).name(),
            "anthropic",
            "上限载荷 {max_tokens:?} 下臂构造可达"
        );
    }
}

#[test]
fn beta条件发_仅anthropic协议且后缀在场时携1m头() {
    let betas_of = |protocol: Protocol, raw: &str| {
        let cfg = EngineConfig {
            protocol,
            ..anthropic_config(raw, raw)
        };
        let spec = parse_model_spec(raw).expect("解析成功");
        anthropic_provider(&cfg, &spec).betas
    };

    // anthropic + `[1m]` → 发 1M beta
    assert_eq!(
        betas_of(Protocol::Anthropic, "claude-x[1m]"),
        vec![BETA_CONTEXT_1M.to_owned()],
        "anthropic + 后缀 → anthropic-beta 携 1M 标记"
    );
    assert_eq!(BETA_CONTEXT_1M, "context-1m-2025-08-07", "beta 头值逐字");

    // anthropic + 无后缀 → 不发
    assert!(
        betas_of(Protocol::Anthropic, "claude-x").is_empty(),
        "anthropic 无后缀不发 beta"
    );

    // openai + `[1m]` → 不发（本地窗长仍 1M，但不发 beta，R4）
    let openai_suffixed = parse_model_spec("claude-x[1m]").expect("解析成功");
    assert_eq!(
        openai_suffixed.length,
        Some(MILLION_CONTEXT_WINDOW),
        "窗长候选仍在（openai 侧本地防线仍按 1M）"
    );
    assert_eq!(
        build_model(&complete_config(), &openai_suffixed, None).name(),
        "openai",
        "openai 臂选臂不产生 anthropic provider（结构上无 beta 写入点，故不发 beta）"
    );
    // openai + 无后缀 → 不发（同一结构面）
    let openai_plain = parse_model_spec("claude-x").expect("解析成功");
    assert_eq!(
        build_model(&complete_config(), &openai_plain, None).name(),
        "openai"
    );

    // 后缀大小写形态同样命中（判定只认可考表示 spec.length）
    assert_eq!(
        betas_of(Protocol::Anthropic, "claude-x[1M]"),
        vec![BETA_CONTEXT_1M.to_owned()],
        "后缀大小写形态同判"
    );
    // 非 1M 后缀在解析面即拒收，不进 beta 判定
    assert!(
        parse_model_spec("claude-x[2m]").is_err(),
        "非 1M 后缀不产生窗长候选，结构上无法触达 beta 分支"
    );
}

#[test]
fn 模型名边界形态组装不panic且裸名保真() {
    let cfg = anthropic_config("claude-x", "claude-y");
    // 超长（>1000 字符，带 / 不带后缀）/ emoji / \0 / 换行模型名
    let long = format!("{}-模型", "m".repeat(1_500));
    for raw in [
        long.clone(),
        format!("{long}[1m]"),
        "claude-🎉[1m]".to_owned(),
        "claude-🎉".to_owned(),
        "claude-\u{0}-x".to_owned(),
        "claude\nx".to_owned(),
    ] {
        let spec = parse_model_spec(&raw).expect("形态应解析成功");
        let model = build_model(&cfg, &spec, None);
        assert_eq!(model.name(), "anthropic", "模型名 {raw:?} 下臂构造可达");
        assert_eq!(model.id(), Some(spec.name.as_str()), "下发裸名保真");
    }
}

#[tokio::test]
async fn 按档窗长_后缀胜出且两档独立解析() {
    // 同一 runner：high 档带 `[1m]`（1M 窗压过 context_length = 64）→ 无 L2
    // notice；low 档无后缀（回落 64 窗）→ 有 L2 notice
    let (resume, _calls) = loader(Ok(Some(oversized_transcript())));
    let cfg = anthropic_config("claude-x[1m]", "claude-y");
    let runner = SdkRunner::new(cfg, Some(Arc::clone(&resume)), Some(64), None);

    let mut high = runner
        .open_session(open_continue_at_level("ses-tier-high", ModelLevel::High))
        .expect("high 档 open 应成功");
    high.questions
        .send(agent::TurnQuestion {
            prompt: "高档轮".to_owned(),
        })
        .await
        .expect("问题送达");
    let first_high = high.observations.recv().await.expect("首事件");
    assert!(
        matches!(&first_high, AgentEventKind::RunStarted { .. }),
        "1M 窗压过 context_length → 重建史未越 L2，首事件即 RunStarted，实际: {first_high:?}"
    );
    drop(high);

    let mut low = runner
        .open_session(open_continue_at_level("ses-tier-low", ModelLevel::Low))
        .expect("low 档 open 应成功");
    low.questions
        .send(agent::TurnQuestion {
            prompt: "低档轮".to_owned(),
        })
        .await
        .expect("问题送达");
    let first_low = low.observations.recv().await.expect("首事件");
    let AgentEventKind::SystemNotice { subtype, .. } = &first_low else {
        panic!("无后缀档回落 64 窗应触 L2 notice，实际: {first_low:?}");
    };
    assert_eq!(subtype.as_str(), "context_pruned", "低档 L2 剪裁留痕");
    let started = low.observations.recv().await.expect("RunStarted");
    assert!(
        matches!(&started, AgentEventKind::RunStarted { model, .. } if model.as_deref() == Some("claude-y")),
        "低档裸名上报，实际: {started:?}"
    );
    drop(low);
}

#[tokio::test]
async fn 按档窗长_context_length与128k缺省各按档生效() {
    // 无后缀 + context_window = 64：重建史越 L2 → 有 notice
    let (resume, _calls) = loader(Ok(Some(oversized_transcript())));
    let small = SdkRunner::new(complete_config(), Some(resume), Some(64), None);
    let mut session = small
        .open_session(open_continue("ses-window-64"))
        .expect("open 应成功");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "小窗轮".to_owned(),
        })
        .await
        .expect("问题送达");
    assert!(
        matches!(
            session.observations.recv().await.expect("首事件"),
            AgentEventKind::SystemNotice { .. }
        ),
        "context_length = 64 档防线生效（L2 先行剪裁）"
    );
    drop(session);

    // 无后缀 + context_window 缺席 / Some(0)：128K 缺省 → 重建史未越线
    for window in [None, Some(0)] {
        let (resume, _calls) = loader(Ok(Some(oversized_transcript())));
        let default_window = SdkRunner::new(complete_config(), Some(resume), window, None);
        let mut session = default_window
            .open_session(open_continue("ses-window-default"))
            .expect("open 应成功");
        session
            .questions
            .send(agent::TurnQuestion {
                prompt: "缺省窗轮".to_owned(),
            })
            .await
            .expect("问题送达");
        let first = session.observations.recv().await.expect("首事件");
        assert!(
            matches!(&first, AgentEventKind::RunStarted { .. }),
            "窗长 {window:?} 走 128K 缺省（重建史未越线），实际: {first:?}"
        );
        drop(session);
    }

    // high / low 各带后缀形态：两档窗长各自独立（两档 1M 值互不串档）
    let (resume, _calls) = loader(Ok(Some(oversized_transcript())));
    let both_suffixed = EngineConfig {
        model_high: "claude-x[1m]".to_owned(),
        model_low: "claude-y[1M]".to_owned(),
        ..complete_config()
    };
    let runner = SdkRunner::new(both_suffixed, Some(resume), Some(64), None);
    for level in [ModelLevel::High, ModelLevel::Low] {
        let mut session = runner
            .open_session(open_continue_at_level("ses-both-suffixed", level))
            .expect("open 应成功");
        session
            .questions
            .send(agent::TurnQuestion {
                prompt: "双后缀轮".to_owned(),
            })
            .await
            .expect("问题送达");
        let first = session.observations.recv().await.expect("首事件");
        assert!(
            matches!(&first, AgentEventKind::RunStarted { .. }),
            "{level:?} 档 1M 后缀覆盖 context_length（无 L2 notice），实际: {first:?}"
        );
        drop(session);
    }
}

#[tokio::test]
async fn 非法后缀启动失败_逐档解析且不panic() {
    // high 档非法：Err 携档位名与 ModelSpecError 原文，不产事件
    let cfg = EngineConfig {
        model_high: "x[2m]".to_owned(),
        ..complete_config()
    };
    let error = SdkRunner::new(cfg, None, None, None)
        .open_session(open_new())
        .expect_err("非法后缀必须显式失败");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("model_high") && message.contains("x[2m]"),
        "文案含档位名与模型名原文，实际: {message}"
    );
    assert!(
        message.contains("模型名后缀非法"),
        "文案含 ModelSpecError 原文，实际: {message}"
    );

    // low 档非法：低档轮显式失败，高档轮照常
    let cfg = EngineConfig {
        model_low: "x[1m]y".to_owned(),
        ..complete_config()
    };
    let runner = SdkRunner::new(cfg, None, None, None);
    let error = runner
        .open_session(open_new_at_level(ModelLevel::Low))
        .expect_err("低档非法后缀必须显式失败");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("model_low") && message.contains("x[1m]y"),
        "低档成因逐字，实际: {message}"
    );
    drop(
        runner
            .open_session(open_new_at_level(ModelLevel::High))
            .expect("高档无非法后缀，照常 open"),
    );
}

#[test]
fn 空模型名既有路径_配置校验先于后缀解析() {
    // 空名不进入 parse 分支：仍是既有三件套缺失文案（不含后缀错误）
    let cfg = EngineConfig {
        model_high: String::new(),
        ..complete_config()
    };
    for window in [None, Some(200_000)] {
        let error = SdkRunner::new(cfg.clone(), None, window, None)
            .open_session(open_new_at_level(ModelLevel::High))
            .expect_err("空模型名必须 Err");
        let AgentStartError::ConfigMissing(message) = &error else {
            panic!("应为 ConfigMissing，实际: {error:?}");
        };
        assert!(
            message.contains("model_high"),
            "空名走既有配置缺失成因，实际: {message}"
        );
        assert!(
            !message.contains("后缀"),
            "空名不产生后缀错误（校验先于 parse），实际: {message}"
        );
    }
}

#[tokio::test]
async fn 引擎三模块链路_裸名与窗长与上限一次装配不打架() {
    // 入口 = open_session：model_spec（解析）+ context（按档防线）+ 双臂
    // build_model（上限载荷）三面同源、单次装配
    let (resume, _calls) = loader(Ok(Some(oversized_transcript())));
    let cfg = anthropic_config("claude-x[1m]", "claude-y");
    let runner =
        SdkRunner::new(cfg.clone(), Some(resume), Some(64), None).with_max_tokens(Some(8192));
    let mut session = runner
        .open_session(open_continue("ses-combined"))
        .expect("三模块链路 open 应成功");
    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "三模块链路一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    // 裸名上报 + 1M 窗压过 context_length（无 L2 notice）同一装配产物
    let first = session.observations.recv().await.expect("首事件");
    let AgentEventKind::RunStarted { model, .. } = &first else {
        panic!("三面同源装配下首事件应为 RunStarted，实际: {first:?}");
    };
    assert_eq!(model.as_deref(), Some("claude-x"), "裸名上报（后缀剥离）");
    // 上限载荷经同一 runner 携带（组装面直断的旁路半边）
    let spec = parse_model_spec(&cfg.model_high).expect("解析成功");
    assert_eq!(
        build_model(&cfg, &spec, Some(8192)).name(),
        "anthropic",
        "同一配置上限载荷下臂构造可达"
    );
    drop(session);
}
