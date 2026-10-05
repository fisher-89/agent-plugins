use std::path::PathBuf;
use std::sync::Arc;

use agent::{
    AgentBlock, AgentEventKind, AgentMessageRole, AgentPermissionMode, AgentRunner,
    AgentStartError, SessionCtx, SessionInjections, SessionOpen, SessionRef,
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
        model: "rig-test-model".to_owned(),
    }
}

fn open_new() -> SessionOpen {
    SessionOpen {
        injections: SessionInjections::default(),
        ctx: SessionCtx {
            workspace_root: PathBuf::from("D:\\工作区"),
            permission_mode: AgentPermissionMode::BypassPermissions,
        },
        session: SessionRef::New,
        prior_handle: None,
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
    let runner = SdkRunner::new(EngineConfig::empty(), None, None);

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
fn 单字段缺失open消息区分api_key_base_url_model成因() {
    let cases = [
        (
            "api_key",
            EngineConfig {
                api_key: String::new(),
                base_url: "http://127.0.0.1:9/v1".to_owned(),
                model: "m".to_owned(),
            },
        ),
        (
            "base_url",
            EngineConfig {
                api_key: "k".to_owned(),
                base_url: String::new(),
                model: "m".to_owned(),
            },
        ),
        (
            "model",
            EngineConfig {
                api_key: "k".to_owned(),
                base_url: "http://127.0.0.1:9/v1".to_owned(),
                model: String::new(),
            },
        ),
    ];
    for (field, config) in cases {
        let runner = SdkRunner::new(config, None, None);
        let error = runner.open_session(open_new()).expect_err("缺项必须 Err");
        let AgentStartError::ConfigMissing(message) = &error else {
            panic!("应为 ConfigMissing，实际: {error:?}");
        };
        assert!(message.contains(field), "{field} 成因可辨，实际: {message}");
        // 其余两项成因不在消息中（只列缺失项）
        for other in ["api_key", "base_url", "model"] {
            if other != field {
                assert!(
                    !message.contains(other),
                    "{field} 缺失时不列 {other}，实际: {message}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// open 段：Continue 装载缝三形态
// ---------------------------------------------------------------------------

#[test]
fn continue装载缝none形态报会话不存在且成因可区分() {
    let (resume, calls) = loader(Ok(None));
    let runner = SdkRunner::new(complete_config(), Some(resume), None);

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
    let runner = SdkRunner::new(complete_config(), Some(resume), None);

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

#[test]
fn continue重建空史报会话缺失语义且三成因互不重合() {
    let (resume, _calls) = loader(Ok(Some(Vec::new())));
    let runner = SdkRunner::new(complete_config(), Some(resume), None);

    let error = runner
        .open_session(open_continue("ses-empty"))
        .expect_err("重建空史必须 Err");
    let AgentStartError::ConfigMissing(message) = &error else {
        panic!("应为 ConfigMissing，实际: {error:?}");
    };
    assert!(
        message.contains("重建历史为空"),
        "空史成因（视同会话缺失），实际: {message}"
    );

    // 三形态成因互不重合（可区分）
    let (none_loader, _) = loader(Ok(None));
    let none_message = SdkRunner::new(complete_config(), Some(none_loader), None)
        .open_session(open_continue("ses-a"))
        .expect_err("")
        .to_string();
    let (err_loader, _) = loader(Err("读取失败".to_owned()));
    let err_message = SdkRunner::new(complete_config(), Some(err_loader), None)
        .open_session(open_continue("ses-b"))
        .expect_err("")
        .to_string();
    let empty_message = error.to_string();
    assert_ne!(none_message, err_message);
    assert_ne!(err_message, empty_message);
    assert_ne!(none_message, empty_message);
}

#[test]
fn continue装载器未注入时显式失败不静默空史() {
    // resume 缝未注入（直构 runner 无 loader）的 Continue：显式 ConfigMissing
    let runner = SdkRunner::new(complete_config(), None, None);
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
    let runner = SdkRunner::new(complete_config(), Some(resume), None);

    // Continue：全史转录重建非空 → open 成功（全史装载半边；重建史注入首轮
    // 请求由 loop_test 捕获缝承载）
    let session = runner
        .open_session(open_continue("ses-hist"))
        .expect("Continue 全史装载应成功");
    drop(session);

    // New：全新运行空史，装载缝零调用
    let (new_resume, new_calls) = loader(Ok(Some(minimal_transcript())));
    let new_runner = SdkRunner::new(complete_config(), Some(new_resume), None);
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
    let runner = SdkRunner::new(complete_config(), Some(resume), Some(64));

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
    let runner = SdkRunner::new(complete_config(), Some(resume), Some(64));

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

    let runner = SdkRunner::new(complete_config(), None, None);
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

#[tokio::test]
async fn ask泵观察回流未盖戳词汇_变体序以收敛收尾且remote_id_sdk前缀上报() {
    let runner = SdkRunner::new(complete_config(), None, None);
    let mut session = runner.open_session(open_new()).expect("open 应成功");

    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "帮我跑一轮".to_owned(),
        })
        .await
        .expect("问题送达");

    // 未盖戳观察词汇：泵产出 agent::AgentEventKind（盖戳收内核）。有界读取：
    // 泵一轮一命，本轮恒产 4 事件后泵任务返回（观测通道随之关闭）
    let mut events = Vec::new();
    for _ in 0..4 {
        events.push(session.observations.recv().await.expect("本轮事件"));
    }

    // 变体序：RunStarted 先导 → user 密封提示词 → SystemNotice 记因 →
    // TurnDone 收敛（拒连端点：流失败即收敛，delta 面由 loop_test 假流承载）
    assert!(
        matches!(&events[0], AgentEventKind::RunStarted { .. }),
        "RunStarted 先导，实际: {:?}",
        events.first()
    );
    assert!(matches!(
        &events[1],
        AgentEventKind::Message { role, .. } if role == &AgentMessageRole::User
    ));
    assert!(matches!(
        &events[2],
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
    let runner = SdkRunner::new(complete_config(), None, None);
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

#[tokio::test]
async fn 停止晚于eof时泵已收敛无二次收敛事件() {
    let runner = SdkRunner::new(complete_config(), None, None);
    let mut session = runner.open_session(open_new()).expect("open 应成功");

    session
        .questions
        .send(agent::TurnQuestion {
            prompt: "跑一轮".to_owned(),
        })
        .await
        .expect("问题送达");
    // 本轮事件有界读取至收敛事件（泵一轮一命，收敛即泵返回）
    let mut events = Vec::new();
    for _ in 0..4 {
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
    let via_facade = EngineFacade::new().runner_for(EngineKind::Sdk, EngineConfig::empty());
    let via_direct = SdkRunner::new(EngineConfig::empty(), None, None);

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
    let with_resume =
        EngineFacade::with_resume_transcript(resume).runner_for(EngineKind::Sdk, complete_config());
    let error = with_resume
        .open_session(open_continue("ses-facade"))
        .expect_err("门面 Continue 校验必须 Err");
    assert!(matches!(error, AgentStartError::ConfigMissing(_)));
}
