//! `model::AgentRunRecord` 的单元测试（AC-3）：JSON 编解码（沿
//! `WorkspaceRecord` 模式）——camelCase 顶层键、全字段往返、None 形态、
//! 特殊字符保真、status 受控串直存直取、非法输入 Err 不 panic。纯编解码，
//! 无外部依赖，不需要 Mock。

use crate::model::AgentRunRecord;

/// 全字段填充的记录（None 全为 Some，含汇总与 error）。
fn full_record() -> AgentRunRecord {
    AgentRunRecord {
        id: 7,
        prompt: "帮我跑一轮 loop".to_owned(),
        cwd: "D:\\项目 目录\\demo".to_owned(),
        env: "default".to_owned(),
        permission_mode: "bypassPermissions".to_owned(),
        status: "completed".to_owned(),
        started_at: 1727000000000,
        finished_at: Some(1727000001000),
        num_turns: Some(5),
        cost_usd: Some(0.42),
        duration_ms: Some(1234),
        session_id: Some("s-abc".to_owned()),
        error: None,
    }
}

// ---------------------------------------------------------------------------
// encode：camelCase 顶层键
// ---------------------------------------------------------------------------

#[test]
fn encode产物为驼峰字节串且顶层键恰为十三字段() {
    let bytes = full_record().encode().expect("编码成功");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("产物为合法 JSON");

    let mut keys: Vec<&str> = value
        .as_object()
        .expect("顶层为对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "costUsd",
            "cwd",
            "durationMs",
            "env",
            "error",
            "finishedAt",
            "id",
            "numTurns",
            "permissionMode",
            "prompt",
            "sessionId",
            "startedAt",
            "status",
        ],
        "顶层键恰为 13 个 camelCase 字段，无 redb 概念泄漏"
    );
}

// ---------------------------------------------------------------------------
// encode/decode 往返
// ---------------------------------------------------------------------------

#[test]
fn 全字段填充记录encode_decode往返逐字段相等() {
    let record = full_record();
    let bytes = record.encode().expect("编码成功");
    let decoded = AgentRunRecord::decode(&bytes).expect("解码成功");
    assert_eq!(decoded, record);
}

#[test]
fn running行全option为none时序列化含null键且decode回none() {
    let record = AgentRunRecord {
        id: 1,
        prompt: "进行中".to_owned(),
        cwd: "C:\\ws".to_owned(),
        env: "bare".to_owned(),
        permission_mode: "acceptEdits".to_owned(),
        status: "running".to_owned(),
        started_at: 1727000000000,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
    };

    let bytes = record.encode().expect("编码成功");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("合法 JSON");
    for key in [
        "finishedAt",
        "numTurns",
        "costUsd",
        "durationMs",
        "sessionId",
        "error",
    ] {
        assert_eq!(
            value[key],
            serde_json::Value::Null,
            "汇总字段留空形态为 null 键"
        );
    }

    let decoded = AgentRunRecord::decode(&bytes).expect("解码成功");
    assert_eq!(decoded, record, "None 形态逐字段还原");
}

// ---------------------------------------------------------------------------
// 字符串保真与受控串
// ---------------------------------------------------------------------------

#[test]
fn prompt与error含中文emoji换行与超长串时往返无损() {
    let long = "长".repeat(1200);
    let record = AgentRunRecord {
        prompt: format!("第一行 🎉\n\"quoted\" {long}"),
        error: Some("失败原因\n第二行 ⚠️".to_owned()),
        ..full_record()
    };

    let bytes = record.encode().expect("编码成功");
    let decoded = AgentRunRecord::decode(&bytes).expect("解码成功");
    assert_eq!(decoded, record);
}

#[test]
fn status三受控值直存直取store不引本地枚举() {
    for status in ["running", "completed", "failed"] {
        let record = AgentRunRecord {
            status: status.to_owned(),
            ..full_record()
        };
        let bytes = record.encode().expect("编码成功");
        let decoded = AgentRunRecord::decode(&bytes).expect("解码成功");
        assert_eq!(decoded.status, status, "受控串按原样落库读出");
    }
}

// ---------------------------------------------------------------------------
// decode：非法输入
// ---------------------------------------------------------------------------

#[test]
fn 非法json字节decode返回err不panic() {
    let garbage = "这不是 JSON 字节串".as_bytes().to_vec();
    let result = AgentRunRecord::decode(&garbage);
    assert!(result.is_err(), "非法 JSON 必须 Err");

    // 合法 JSON 但缺必填字段同样 Err（不产半成品记录）
    let missing = br#"{"id":1}"#.to_vec();
    assert!(AgentRunRecord::decode(&missing).is_err());
}
