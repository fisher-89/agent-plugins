use std::path::PathBuf;
use std::sync::Arc;

use agent::{
    AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunner,
    AgentStartError, ModelLevel, SessionCtx, SessionInjections, SessionOpen, SessionRef,
};

use crate::sdk::config::EngineConfig;
use crate::sdk::runner::SdkRunner;
use crate::{EngineFacade, EngineKind, ResumeTranscript};

// ---------------------------------------------------------------------------
// 装置：配置底座、open 入参底座、转录装载缝
// ---------------------------------------------------------------------------

/// 端点指向本机 discard 端口（恒拒连，无网络触达真实 provider）。
fn complete_config() -> EngineConfig {
    EngineConfig {
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
    let runner = SdkRunner::new(complete_config(), Some(resume), None, Some(Arc::clone(&store)));

    let mut open = open_continue("ses-explore-e2e");
    open.injections = explore_injections();
    let mut session = runner
        .open_session(open)
        .expect("explore 注入 open 应成功");
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
    let err_runner =
        SdkRunner::new(complete_config(), Some(err_loader), None, Some(Arc::clone(&store)));
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
    let none_runner =
        SdkRunner::new(complete_config(), Some(none_loader), None, Some(Arc::clone(&store)));
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
        tools.contains(&"create_explore_note".to_owned())
            && !tools.contains(&"write".to_owned()),
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
