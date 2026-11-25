use serde_json::{json, Value};

use crate::runner::AgentRunStatus;
use crate::session::{
    new_session_id, NewSessionRow, SessionPreambleInfo, SessionProvenance, SessionRow,
    SessionStats, SessionSummary, TurnSummary,
};

/// 固定 provenance 底座。
fn provenance(source: &str, source_ref: Option<&str>) -> SessionProvenance {
    SessionProvenance {
        source: source.to_owned(),
        source_ref: source_ref.map(str::to_owned),
    }
}

/// 全字段 SessionRow 底座（时间戳显式固定值）。
fn full_row() -> SessionRow {
    SessionRow {
        id: "ses-1-1727000000000".to_owned(),
        remote_session_id: Some("sdk-7-1727000000001".to_owned()),
        config_snapshot: json!({
            "engine": "sdk",
            "model": "gpt-x",
            "permissionMode": "bypassPermissions"
        }),
        provenance: provenance("explore", Some("42")),
        created_at: 1727000000000,
        updated_at: 1727000005000,
    }
}

// ---------------------------------------------------------------------------
// 会话 id 铸造（core 铸 id 单点）
// ---------------------------------------------------------------------------

#[test]
fn 连续铸造多枚id均带ses前缀且两两互异() {
    let first = new_session_id();
    let second = new_session_id();
    let third = new_session_id();

    for id in [&first, &second, &third] {
        assert!(id.starts_with("ses-"), "前缀逐字命中 ses-，实际: {id}");
    }
    assert_ne!(first, second, "连续铸造互异");
    assert_ne!(second, third, "连续铸造互异");
    assert_ne!(first, third);
}

#[test]
fn 千次铸造零碰撞且id形态稳定无空白与非法字符() {
    let mut ids = Vec::with_capacity(1000);
    for _ in 0..1000 {
        ids.push(new_session_id());
    }
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "千次铸造零碰撞");

    for id in &ids {
        assert!(
            id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "id 仅含字母数字与连字符（无空白与非法字符），实际: {id}"
        );
        let suffix = id.strip_prefix("ses-").expect("前缀已断言");
        assert!(
            suffix.contains('-'),
            "形态为 前缀+计数-时戳 两段唯一后缀，实际: {id}"
        );
    }
}

// ---------------------------------------------------------------------------
// SessionRow serde 往返（camelCase 线格式）
// ---------------------------------------------------------------------------

#[test]
fn session_row全字段驼峰线格式往返逐字段相等() {
    let row = full_row();

    let value = serde_json::to_value(&row).expect("序列化成功");
    assert_eq!(value["id"], json!("ses-1-1727000000000"));
    assert_eq!(value["remoteSessionId"], json!("sdk-7-1727000000001"));
    assert!(
        value.get("configSnapshot").is_some(),
        "configSnapshot 驼峰键"
    );
    assert_eq!(value["provenance"]["source"], json!("explore"));
    assert_eq!(value["provenance"]["sourceRef"], json!("42"));
    assert_eq!(value["createdAt"], json!(1727000000000_i64));
    assert_eq!(value["updatedAt"], json!(1727000005000_i64));

    let roundtrip: SessionRow = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, row, "全字段往返逐字段相等");
}

#[test]
fn remote_session_id_null与有值两形态可区分() {
    let with_remote = full_row();
    let mut without_remote = full_row();
    without_remote.remote_session_id = None;

    let some_value = serde_json::to_value(&with_remote).expect("序列化成功");
    let null_value = serde_json::to_value(&without_remote).expect("序列化成功");
    assert_eq!(
        some_value["remoteSessionId"],
        json!("sdk-7-1727000000001"),
        "有值形态"
    );
    assert_eq!(
        null_value["remoteSessionId"],
        Value::Null,
        "null 形态（双 id 映射未上报）"
    );
    assert_ne!(some_value["remoteSessionId"], null_value["remoteSessionId"]);

    for row in [with_remote, without_remote] {
        let value = serde_json::to_value(&row).expect("序列化成功");
        let roundtrip: SessionRow = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, row);
    }
}

#[test]
fn config_snapshot不透明value任意json保真() {
    // 快照不透明：core 不解释，任意 JSON 形态（数组/标量/深层嵌套）原样往返
    for snapshot in [
        json!({ "engine": "cli", "model": null, "permissionMode": "default" }),
        json!([1, "二", { "深": [true, null] }]),
        json!("标量形态"),
        json!(null),
    ] {
        let row = SessionRow {
            config_snapshot: snapshot.clone(),
            ..full_row()
        };
        let value = serde_json::to_value(&row).expect("序列化成功");
        assert_eq!(value["configSnapshot"], snapshot, "快照原样保真");
        let roundtrip: SessionRow = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip.config_snapshot, snapshot);
    }
}

#[test]
fn 超长source_ref原样往返不截断() {
    let long = "长".repeat(1001);
    let row = SessionRow {
        provenance: provenance("explore", Some(&long)),
        ..full_row()
    };

    let value = serde_json::to_value(&row).expect("序列化成功");
    assert_eq!(
        value["provenance"]["sourceRef"],
        json!(long),
        "超长 sourceRef 原样"
    );
    let roundtrip: SessionRow = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(
        roundtrip.provenance.source_ref.as_deref(),
        Some(long.as_str()),
        "大于 1000 字符原样往返"
    );
}

#[test]
fn 缺必填字段id或provenance反序列化err不产半成品() {
    let full = serde_json::to_value(full_row()).expect("序列化成功");

    // 缺 id
    let mut missing_id = full.clone();
    missing_id
        .as_object_mut()
        .expect("对象形态")
        .remove("id")
        .expect("移除 id");
    assert!(
        serde_json::from_value::<SessionRow>(missing_id).is_err(),
        "缺 id 必须 Err"
    );

    // 缺 provenance
    let mut missing_provenance = full.clone();
    missing_provenance
        .as_object_mut()
        .expect("对象形态")
        .remove("provenance")
        .expect("移除 provenance");
    assert!(
        serde_json::from_value::<SessionRow>(missing_provenance).is_err(),
        "缺 provenance 必须 Err"
    );

    // 对照组：全字段齐备即可反序列化
    assert!(serde_json::from_value::<SessionRow>(full).is_ok());
}

// ---------------------------------------------------------------------------
// NewSessionRow：建行入参形态（id 内核铸造后传入，无时间戳字段）
// ---------------------------------------------------------------------------

#[test]
fn new_session_row字段面仅id快照与provenance且无时间戳语义() {
    let new_row = NewSessionRow {
        id: "ses-2-1727000000002".to_owned(),
        config_snapshot: json!({ "engine": "cli" }),
        provenance: provenance("debug", None),
    };

    // 建行入参为进程内形态（非线格式类型）：字段面以构造与克隆保真承载
    assert_eq!(new_row.id, "ses-2-1727000000002");
    assert_eq!(new_row.config_snapshot, json!({ "engine": "cli" }));
    assert_eq!(new_row.provenance.source, "debug");
    assert_eq!(new_row.provenance.source_ref, None);
    let cloned = new_row.clone();
    assert_eq!(cloned, new_row, "Clone 保真（内核铸造后传入 sink）");
}

// ---------------------------------------------------------------------------
// SessionStats：缺席合法缺省（降级不违约）
// ---------------------------------------------------------------------------

#[test]
fn session_stats全none形态与全有值形态均往返无损() {
    let bare = SessionStats {
        turn_count: 0,
        total_duration_ms: None,
        input_tokens: None,
        output_tokens: None,
        cached_input_tokens: None,
        cache_creation_input_tokens: None,
    };
    let full = SessionStats {
        turn_count: 7,
        total_duration_ms: Some(9000),
        input_tokens: Some(120),
        output_tokens: Some(340),
        cached_input_tokens: Some(90),
        cache_creation_input_tokens: Some(20),
    };

    for stats in [bare, full] {
        let value = serde_json::to_value(&stats).expect("序列化成功");
        assert_eq!(value["turnCount"], json!(stats.turn_count));
        let roundtrip: SessionStats = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, stats);
    }

    // 全 None 形态出线为 null（统计字段缺席合法缺省）
    let value = serde_json::to_value(&SessionStats {
        turn_count: 0,
        total_duration_ms: None,
        input_tokens: None,
        output_tokens: None,
        cached_input_tokens: None,
        cache_creation_input_tokens: None,
    })
    .expect("序列化成功");
    assert_eq!(value["totalDurationMs"], Value::Null);
    assert_eq!(value["inputTokens"], Value::Null);
    assert_eq!(value["outputTokens"], Value::Null);
    assert_eq!(value["cachedInputTokens"], Value::Null);
    assert_eq!(value["cacheCreationInputTokens"], Value::Null);
}

// ---------------------------------------------------------------------------
// TurnSummary：running / 终态两形态与 status 五值线格式
// ---------------------------------------------------------------------------

fn running_summary() -> TurnSummary {
    TurnSummary {
        turn_id: 3,
        session_id: "ses-1-1727000000000".to_owned(),
        status: AgentRunStatus::Running,
        started_at: 1727000000000,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        error: None,
    }
}

fn finished_summary(status: AgentRunStatus, error: Option<&str>) -> TurnSummary {
    TurnSummary {
        status,
        finished_at: Some(1727000005000),
        num_turns: Some(4),
        cost_usd: Some(0.12),
        duration_ms: Some(5000),
        error: error.map(str::to_owned),
        ..running_summary()
    }
}

#[test]
fn turn_summary_running形态统计全null且终态形态统计齐备() {
    let running = running_summary();
    let value = serde_json::to_value(&running).expect("序列化成功");
    assert_eq!(value["status"], json!("running"));
    assert_eq!(value["finishedAt"], Value::Null);
    assert_eq!(value["numTurns"], Value::Null);
    assert_eq!(value["costUsd"], Value::Null);
    assert_eq!(value["durationMs"], Value::Null);
    assert_eq!(value["error"], Value::Null);
    assert_eq!(value["turnId"], json!(3), "驼峰键 turnId/sessionId");
    let roundtrip: TurnSummary = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, running);

    // 终态形态：统计齐备 + error 记因
    let failed = finished_summary(AgentRunStatus::Failed, Some("事件落库失败: db: x"));
    let value = serde_json::to_value(&failed).expect("序列化成功");
    assert_eq!(value["status"], json!("failed"));
    assert_eq!(value["finishedAt"], json!(1727000005000_i64));
    assert_eq!(value["numTurns"], json!(4));
    assert_eq!(value["costUsd"], json!(0.12));
    assert_eq!(value["durationMs"], json!(5000));
    assert_eq!(value["error"], json!("事件落库失败: db: x"));
    let roundtrip: TurnSummary = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, failed);
}

#[test]
fn turn_summary_status五值线格式running_completed_failed_stopped_interrupted() {
    for (status, expected) in [
        (AgentRunStatus::Running, "running"),
        (AgentRunStatus::Completed, "completed"),
        (AgentRunStatus::Failed, "failed"),
        (AgentRunStatus::Stopped, "stopped"),
        (AgentRunStatus::Interrupted, "interrupted"),
    ] {
        let summary = finished_summary(status, None);
        let value = serde_json::to_value(&summary).expect("序列化成功");
        assert_eq!(value["status"], json!(expected), "线值逐字 {expected}");
        let roundtrip: TurnSummary = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, summary);
    }
}

/// 越过线值域的状态串显式 `Err`（异常）：轮行 DTO 不接受词表外状态，
/// 无兜底折进既有变体。
#[test]
fn turn_summary_status越过线值域三形态均err() {
    let mut value = serde_json::to_value(running_summary()).expect("序列化成功");
    for (label, invalid) in [
        ("词表外", json!("paused")),
        ("近似形态", json!("interrupted2")),
        ("空串", json!("")),
    ] {
        value["status"] = invalid.clone();
        let result: Result<TurnSummary, _> = serde_json::from_value(value.clone());
        assert!(
            result.is_err(),
            "越过线值域形态 {label}（{invalid}）应 Err，实际: {result:?}"
        );
    }
}

/// `interrupted` 形态的完整字段面（边界）：标定记因进 `error`、`finishedAt` 为
/// 数值、统计三字段仍 `None`（不虚构统计）。
#[test]
fn turn_summary_interrupted形态记因在位且统计三字段不虚构() {
    let calibrated = TurnSummary {
        status: AgentRunStatus::Interrupted,
        finished_at: Some(1727000009000),
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        error: Some("重启标定：桌面进程中断，会话轮客观已终止".to_owned()),
        ..running_summary()
    };
    let value = serde_json::to_value(&calibrated).expect("序列化成功");
    assert_eq!(value["status"], json!("interrupted"));
    assert_eq!(value["finishedAt"], json!(1727000009000_i64));
    assert_eq!(
        value["error"],
        json!("重启标定：桌面进程中断，会话轮客观已终止")
    );
    assert_eq!(value["numTurns"], Value::Null, "统计三字段不虚构");
    assert_eq!(value["costUsd"], Value::Null);
    assert_eq!(value["durationMs"], Value::Null);
    let roundtrip: TurnSummary = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, calibrated);
}

/// `TurnSummary.status` doc 同步（边界，源码自证）：该 doc 是 `bindings.ts` 注释源，
/// 取值清单须含 `interrupted`，防「union 五值 / 注释四值」自相矛盾。
#[test]
fn turn_summary_status_doc取值清单含interrupted() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/session.rs"),
    )
    .expect("读取 session.rs");
    let field_start = own
        .find("pub struct TurnSummary")
        .expect("TurnSummary 定义在场");
    let block = &own[field_start..];
    let status_doc = block[..block.find("pub status:").expect("status 字段在场")]
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with("///"))
        .expect("status 字段 doc 在场");
    assert!(
        status_doc.contains("running")
            && status_doc.contains("completed")
            && status_doc.contains("failed")
            && status_doc.contains("stopped")
            && status_doc.contains("interrupted"),
        "doc 取值清单为五值（含 interrupted），实际: {status_doc}"
    );
}

// ---------------------------------------------------------------------------
// SessionSummary 组装（row + stats + turns）
// ---------------------------------------------------------------------------

#[test]
fn session_summary组装往返_turns空数组与多轮两形态() {
    // 空轮形态
    let empty = SessionSummary {
        row: full_row(),
        stats: SessionStats {
            turn_count: 0,
            total_duration_ms: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            cache_creation_input_tokens: None,
        },
        turns: Vec::new(),
        preamble: None,
    };
    let value = serde_json::to_value(&empty).expect("序列化成功");
    assert_eq!(value["turns"], json!([]), "turns 空数组形态");
    let roundtrip: SessionSummary = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, empty);

    // 多轮形态
    let multi = SessionSummary {
        row: full_row(),
        stats: SessionStats {
            turn_count: 2,
            total_duration_ms: Some(8000),
            input_tokens: Some(60),
            output_tokens: Some(140),
            cached_input_tokens: Some(40),
            cache_creation_input_tokens: Some(12),
        },
        turns: vec![
            running_summary(),
            finished_summary(AgentRunStatus::Completed, None),
        ],
        preamble: Some(preamble_info()),
    };
    let value = serde_json::to_value(&multi).expect("序列化成功");
    assert_eq!(value["turns"].as_array().expect("数组").len(), 2);
    assert_eq!(value["stats"]["turnCount"], json!(2));
    assert_eq!(
        value["preamble"]["content"],
        json!("快照正文\n段二"),
        "多轮形态携快照面"
    );
    let roundtrip: SessionSummary = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, multi, "row+stats+turns 组装往返");
}

// ---------------------------------------------------------------------------
// SessionPreambleInfo：会话系统提示词快照的只读出线面
// ---------------------------------------------------------------------------

/// 快照底座（拍存时刻与正文显式固定值）。
fn preamble_info() -> SessionPreambleInfo {
    SessionPreambleInfo {
        saved_at: 1727000000000,
        content: "快照正文\n段二".to_owned(),
    }
}

/// 无轮空库形态的会话清单项底座（快照面由调用方决定）。
fn summary_with(preamble: Option<SessionPreambleInfo>) -> SessionSummary {
    SessionSummary {
        row: full_row(),
        stats: SessionStats {
            turn_count: 0,
            total_duration_ms: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            cache_creation_input_tokens: None,
        },
        turns: Vec::new(),
        preamble,
    }
}

#[test]
fn session_summary_preamble有值形态驼峰线名逐字段且往返相等() {
    let summary = summary_with(Some(SessionPreambleInfo {
        saved_at: 1727000000000,
        content: "快照正文\n段二".to_owned(),
    }));

    let value = serde_json::to_value(&summary).expect("序列化成功");
    assert_eq!(
        value["preamble"]["savedAt"],
        json!(1727000000000_i64),
        "拍存时刻驼峰线名"
    );
    assert_eq!(
        value["preamble"]["content"],
        json!("快照正文\n段二"),
        "正文逐字（含换行）"
    );
    assert_eq!(
        value["preamble"].as_object().expect("对象形态").len(),
        2,
        "快照只出面两键（无多余包装层）"
    );

    let roundtrip: SessionSummary = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, summary, "含快照面往返逐字段相等");
}

#[test]
fn session_summary_preamble为none时线面恒出null键而不省略() {
    let summary = summary_with(None);

    let value = serde_json::to_value(&summary).expect("序列化成功");
    assert!(
        value.get("preamble").is_some(),
        "键恒在场（不 skip 序列化），实际: {value}"
    );
    assert_eq!(value["preamble"], Value::Null, "清单路径/无快照恒 null 值");

    // 有值 ↔ 无值两形态线面可区分
    let with_preamble =
        serde_json::to_value(summary_with(Some(preamble_info()))).expect("序列化成功");
    assert_ne!(
        with_preamble["preamble"], value["preamble"],
        "Some 与 None 线面可区分"
    );
}

#[test]
fn session_summary缺preamble键反序列化为none而其余三字段缺失均err() {
    let full = serde_json::to_value(summary_with(Some(preamble_info()))).expect("序列化成功");

    // 缺 preamble：Option 隐式缺省（存量出线路由不产半成品）
    let mut missing_preamble = full.clone();
    missing_preamble
        .as_object_mut()
        .expect("对象形态")
        .remove("preamble")
        .expect("移除 preamble");
    let decoded: SessionSummary =
        serde_json::from_value(missing_preamble).expect("缺 preamble 应可反序列化");
    assert_eq!(decoded.preamble, None, "缺键即 None（不产半成品）");

    // 对照：三字段缺失逐个 Err
    for missing in ["row", "stats", "turns"] {
        let mut value = full.clone();
        value
            .as_object_mut()
            .expect("对象形态")
            .remove(missing)
            .unwrap_or_else(|| panic!("移除 {missing}"));
        let result: Result<SessionSummary, _> = serde_json::from_value(value);
        assert!(result.is_err(), "缺 {missing} 必须 Err，实际: {result:?}");
    }

    // 对照组：四字段齐备即可反序列化
    assert!(serde_json::from_value::<SessionSummary>(full).is_ok());
}

#[test]
fn session_preamble_info越界形态三形态均err不静默折成none() {
    let ok_value = json!({ "savedAt": 1727000000000_i64, "content": "正文" });
    assert!(
        serde_json::from_value::<SessionPreambleInfo>(ok_value.clone()).is_ok(),
        "对照组：两键齐备可解码"
    );

    for (label, invalid) in [
        (
            "preamble 为字符串",
            json!({ "row": full_row_value(), "stats": stats_value(), "turns": [], "preamble": "字符串形态" }),
        ),
        (
            "对象缺 content",
            json!({ "row": full_row_value(), "stats": stats_value(), "turns": [], "preamble": { "savedAt": 1 } }),
        ),
        (
            "savedAt 为字符串",
            json!({ "row": full_row_value(), "stats": stats_value(), "turns": [], "preamble": { "savedAt": "1", "content": "正文" } }),
        ),
    ] {
        let result: Result<SessionSummary, _> = serde_json::from_value(invalid);
        assert!(
            result.is_err(),
            "越界形态「{label}」必须 Err（不静默折成 None），实际: {result:?}"
        );
    }
}

/// 线面底座：全字段 `SessionRow` 的 JSON 形态。
fn full_row_value() -> Value {
    serde_json::to_value(full_row()).expect("序列化成功")
}

/// 线面底座：全 `None` 统计的 JSON 形态。
fn stats_value() -> Value {
    serde_json::to_value(SessionStats {
        turn_count: 0,
        total_duration_ms: None,
        input_tokens: None,
        output_tokens: None,
        cached_input_tokens: None,
        cache_creation_input_tokens: None,
    })
    .expect("序列化成功")
}

#[test]
fn session_preamble_info独立构造序列化恰两键且往返等值() {
    let info = preamble_info();

    let value = serde_json::to_value(&info).expect("序列化成功");
    assert_eq!(
        value,
        json!({ "savedAt": 1727000000000_i64, "content": "快照正文\n段二" }),
        "键集恰 savedAt / content，无多余键"
    );
    let roundtrip: SessionPreambleInfo = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, info, "往返等值");
}

#[test]
fn session_preamble_info_content三形态逐字往返零截断() {
    let cases = [
        ("空串", String::new()),
        (
            "特殊字符",
            "首行\n\n空行后\t制表\0空字节 🎉🚀 尾行\n".to_owned(),
        ),
        ("超长中文", "中".repeat(20000)),
    ];
    for (label, content) in cases {
        let info = SessionPreambleInfo {
            saved_at: 1,
            content: content.clone(),
        };
        let value = serde_json::to_value(&info).expect("序列化成功");
        assert_eq!(value["content"], json!(content), "形态「{label}」线面逐字");
        let roundtrip: SessionPreambleInfo = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip.content, content, "形态「{label}」往返逐字");
        assert_eq!(
            roundtrip.content.chars().count(),
            content.chars().count(),
            "形态「{label}」零截断"
        );
    }
    assert_eq!(
        "中".repeat(20000).len(),
        60000,
        "超长形态越过 32KB 线（截断只在读取侧）"
    );
}

#[test]
fn session_preamble_info_saved_at四值线面为数字且往返保真() {
    for saved_at in [0, -1, i64::MAX, 1_727_000_000_000] {
        let info = SessionPreambleInfo {
            saved_at,
            content: String::new(),
        };
        let value = serde_json::to_value(&info).expect("序列化成功");
        assert!(
            value["savedAt"].is_number(),
            "线面为 JSON 数字（无范围校验），实际: {}",
            value["savedAt"]
        );
        assert_eq!(value["savedAt"], json!(saved_at));
        let roundtrip: SessionPreambleInfo = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip.saved_at, saved_at, "{saved_at} 往返保真");
    }
}

#[test]
fn session_preamble_info派生链clone保真且空正文区别于无快照() {
    let info = preamble_info();
    let cloned = info.clone();
    assert_eq!(cloned, info, "Clone 保真");

    // 空正文 ≠ 无快照：Some(空串) 与 None 可区分（PartialEq 判据）
    let empty_body = SessionPreambleInfo {
        content: String::new(),
        ..preamble_info()
    };
    assert_ne!(
        Some(empty_body.clone()),
        None,
        "空正文不是无快照（两状态 MUST NOT 混用）"
    );
    assert_eq!(
        Some(empty_body.clone()),
        Some(SessionPreambleInfo {
            content: String::new(),
            ..preamble_info()
        }),
        "同字段值相等"
    );
    assert_ne!(empty_body, info, "空正文与有正文不等");
}

#[test]
fn session_summary三字段键集零改动_快照字段不渗入() {
    let value = serde_json::to_value(summary_with(Some(preamble_info()))).expect("序列化成功");

    assert_eq!(
        value
            .as_object()
            .expect("对象形态")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["row", "stats", "turns", "preamble"],
        "顶层键集 = 既有三字段 + 新增快照位"
    );
    let mut row_keys: Vec<&String> = value["row"].as_object().expect("对象").keys().collect();
    row_keys.sort();
    assert_eq!(
        row_keys,
        vec![
            "configSnapshot",
            "createdAt",
            "id",
            "provenance",
            "remoteSessionId",
            "updatedAt"
        ],
        "SessionRow 键集零改动"
    );
    let mut stats_keys: Vec<&String> = value["stats"].as_object().expect("对象").keys().collect();
    stats_keys.sort();
    assert_eq!(
        stats_keys,
        vec![
            "cacheCreationInputTokens",
            "cachedInputTokens",
            "inputTokens",
            "outputTokens",
            "totalDurationMs",
            "turnCount"
        ],
        "SessionStats 键集 = 既有四字段 + 缓存读写计数（快照体量不落库）"
    );
}

/// `SessionSummary.preamble` 字段块零 `skip_serializing_if`（源码自证，边界）：
/// 线面恒出键的机械判据——加了 skip 即 AB 面裂出「键有时在有时不在」。
#[test]
fn session_summary_preamble字段块零skip_serializing_if() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/session.rs"),
    )
    .expect("读取 session.rs");
    let lines: Vec<&str> = own.lines().collect();
    let field_index = lines
        .iter()
        .position(|line| line.trim_start().starts_with("pub preamble:"))
        .expect("pub preamble 字段在场");

    // 字段块 = 字段行 + 其上紧邻的 doc / 属性行
    let mut block = vec![lines[field_index]];
    for line in lines[..field_index].iter().rev() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("///") || trimmed.starts_with("#[") {
            block.push(line);
        } else {
            break;
        }
    }
    assert!(
        block
            .iter()
            .all(|line| !line.contains("skip_serializing_if")),
        "快照字段块 MUST NOT 带 skip_serializing_if，实际块: {block:?}"
    );
    assert!(
        lines[field_index].contains("Option<SessionPreambleInfo>"),
        "字段类型为可空快照 DTO，实际: {}",
        lines[field_index]
    );
}
