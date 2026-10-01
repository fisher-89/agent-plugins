use serde_json::{json, Value};

use crate::runner::AgentRunStatus;
use crate::session::{
    new_session_id, NewSessionRow, SessionProvenance, SessionRow, SessionStats, SessionSummary,
    TurnSummary,
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
        assert!(
            id.starts_with("ses-"),
            "前缀逐字命中 ses-，实际: {id}"
        );
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
    assert!(value.get("configSnapshot").is_some(), "configSnapshot 驼峰键");
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
    assert_eq!(value["provenance"]["sourceRef"], json!(long), "超长 sourceRef 原样");
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
    };
    let full = SessionStats {
        turn_count: 7,
        total_duration_ms: Some(9000),
        input_tokens: Some(120),
        output_tokens: Some(340),
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
    })
    .expect("序列化成功");
    assert_eq!(value["totalDurationMs"], Value::Null);
    assert_eq!(value["inputTokens"], Value::Null);
    assert_eq!(value["outputTokens"], Value::Null);
}

// ---------------------------------------------------------------------------
// TurnSummary：running / 终态两形态与 status 四值线格式
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
fn turn_summary_status四值线格式running_completed_failed_stopped() {
    for (status, expected) in [
        (AgentRunStatus::Running, "running"),
        (AgentRunStatus::Completed, "completed"),
        (AgentRunStatus::Failed, "failed"),
        (AgentRunStatus::Stopped, "stopped"),
    ] {
        let summary = finished_summary(status, None);
        let value = serde_json::to_value(&summary).expect("序列化成功");
        assert_eq!(value["status"], json!(expected), "线值逐字 {expected}");
        let roundtrip: TurnSummary = serde_json::from_value(value).expect("反序列化成功");
        assert_eq!(roundtrip, summary);
    }
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
        },
        turns: Vec::new(),
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
        },
        turns: vec![running_summary(), finished_summary(AgentRunStatus::Completed, None)],
    };
    let value = serde_json::to_value(&multi).expect("序列化成功");
    assert_eq!(value["turns"].as_array().expect("数组").len(), 2);
    assert_eq!(value["stats"]["turnCount"], json!(2));
    let roundtrip: SessionSummary = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, multi, "row+stats+turns 组装往返");
}
