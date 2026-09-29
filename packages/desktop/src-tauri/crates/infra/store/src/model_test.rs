//! `model` 的单元测试：`AgentEventRecord` 键打包 + 嵌装往返（AC-4）；
//! `AgentRunRecord` v2 存量升级与三枚举线格式（AC-2）。
//!
//! 键打包与嵌装往返用例为内存构造，无 IO 无进程边界，无 mock。编解码经
//! native_model 封装（serde_json codec）内存往返，只验证 store 包装层与 core
//! flatten 类型的组装兼容，不重复验证 serde 自身语义。v2 存量升级用例驱动
//! native_model 真实读路径（native_db 读记录即 `native_model::decode`，见
//! native_db serialization.rs）；「存量 fixture 库打开与重放」组合用例经
//! tempdir 真库 `Store` 公共 API 真实组合（不 mock store）。既有手写
//! `encode` / `decode` 用例随模型层平移 `#[native_model]` + `#[native_db]`
//! 已废弃。

use agent::{
    AgentBlock, AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunStatus,
};

use crate::model::{
    pack_event_key, AgentEventRecord, AgentRunRecord, AgentRunRecordV1, AgentRunRecordV2,
};
use crate::store::Store;

// ---------------------------------------------------------------------------
// 装置：固定时间戳事件构造（等值断言与钟面无关）
// ---------------------------------------------------------------------------

fn event(seq: u64, kind: AgentEventKind) -> AgentEvent {
    AgentEvent {
        seq,
        timestamp_ms: 1727000000000 + seq as i64,
        kind,
    }
}

fn run_started(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::RunStarted {
            model: Some("claude-opus".to_owned()),
            session_id: Some("s-1".to_owned()),
            tools: vec!["Bash".to_owned()],
            mcp_servers: Vec::new(),
        },
    )
}

fn message(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: vec![AgentBlock::Text {
                text: "你好，世界".to_owned(),
            }],
            parent_tool_use_id: None,
        },
    )
}

fn system_notice(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::SystemNotice {
            subtype: "api_retry".to_owned(),
            payload: serde_json::json!({ "attempt": 2, "note": "重试中" }),
        },
    )
}

fn run_result(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::RunResult {
            subtype: "success".to_owned(),
            is_error: false,
            num_turns: Some(3),
            duration_ms: Some(1234),
            cost_usd: Some(0.5),
            usage: serde_json::json!({ "input_tokens": 10 }),
            session_id: Some("s-1".to_owned()),
        },
    )
}

fn raw(seq: u64) -> AgentEvent {
    event(
        seq,
        AgentEventKind::Raw {
            event_type: "mystery".to_owned(),
            raw_json: r#"{"type":"mystery"}"#.to_owned(),
        },
    )
}

/// native_model 封装内存往返（serde_json codec）：不经 db 文件。
fn roundtrip(record: &AgentEventRecord) -> AgentEventRecord {
    let bytes = native_model::encode(record).expect("native_model encode 应成功");
    let (decoded, version) =
        native_model::decode::<AgentEventRecord>(bytes).expect("native_model decode 应成功");
    assert_eq!(version, 1, "native_model 版本封装为 version 1");
    decoded
}

// ---------------------------------------------------------------------------
// 键打包：同 run 保序、run 区间上移、极值不溢出、相邻 run 不串键
// ---------------------------------------------------------------------------

#[test]
fn 键打包同run内seq增大则event_key严格增大() {
    let keys: Vec<u128> = (0..5u64)
        .map(|seq| AgentEventRecord::new(7, raw(seq)).event_key)
        .collect();

    for pair in keys.windows(2) {
        assert!(
            pair[0] < pair[1],
            "同 run 内 seq 增大则 event_key 严格增大: {keys:?}"
        );
    }
}

#[test]
fn 键打包run_id增大则整段键区间上移() {
    // run 1 的全部键（seq 遍历全域小样本）必须整体小于 run 2 的最小键：
    // 扫描自然序即「先 run 1 全部、后 run 2」的重放序前提
    let run1_max = AgentEventRecord::new(1, raw(u64::MAX)).event_key;
    let run2_min = AgentEventRecord::new(2, raw(0)).event_key;

    assert!(
        run1_max < run2_min,
        "run_id 高 64 位隔离：run 1 最大键 < run 2 最小键"
    );
}

#[test]
fn 键打包最小键与最大键不溢出不回绕() {
    let min = pack_event_key(0, 0);
    let max = pack_event_key(i64::MAX, u64::MAX);

    assert_eq!(min, 0, "最小键 (run_id=0, seq=0) 打包为 0");
    assert_eq!(
        max,
        ((i64::MAX as u128) << 64) | (u64::MAX as u128),
        "最大键 (run_id=i64::MAX, seq=u64::MAX) 打包不溢出"
    );
    assert!(min < max);
    // 打包往返一致（组装点唯一口径）
    assert_eq!(pack_event_key(42, 7), ((42i64 as u128) << 64) | 7u128);
}

#[test]
fn 键打包相邻run边界不串键高64位隔离() {
    // 同一 seq 区间内，相邻 run 的键区间必须无缝且不重叠：
    // run 1 尾键（seq=u64::MAX）紧邻 run 2 首键（seq=0）
    let run1_tail = AgentEventRecord::new(1, raw(u64::MAX)).event_key;
    let run2_head = AgentEventRecord::new(2, raw(0)).event_key;

    assert_eq!(
        run2_head - run1_tail,
        1,
        "相邻 run 边界键恰好相邻不重叠（高 64 位隔离）"
    );
}

#[test]
fn 记录构造event_key打包自run_id与seq且访问器与载荷同源() {
    let record = AgentEventRecord::new(9, message(4));

    assert_eq!(record.event_key, pack_event_key(9, 4), "event_key 打包口径");
    assert_eq!(record.run_id(), 9, "run_id 访问器");
    assert_eq!(record.seq(), 4, "seq 访问器与载荷 event.seq 同源");
}

// ---------------------------------------------------------------------------
// 嵌装往返：五变体逐字段保真（native_model serde_json codec 封装）
// ---------------------------------------------------------------------------

#[test]
fn 嵌装往返五变体各构造一条逐字段相等() {
    let run_id = 3;
    let seeded = [
        run_started(0),
        message(1),
        system_notice(2),
        run_result(3),
        raw(4),
    ];

    for original in seeded {
        let record = AgentEventRecord::new(run_id, original.clone());
        let decoded = roundtrip(&record);

        assert_eq!(decoded, record, "变体 {:?} 往返逐字段相等", original.seq);
        assert_eq!(
            decoded.event, original,
            "嵌装载荷与原事件逐字段相等（store 包装层与 core flatten 组装兼容）"
        );
    }
}

#[test]
fn 嵌装往返raw变体中文emoji引号原文保真() {
    let original = event(
        5,
        AgentEventKind::Raw {
            event_type: "外星事件".to_owned(),
            raw_json: "{\"kind\":\"外星事件\",\"note\":\"引号\\\"与emoji🚀\"}".to_owned(),
        },
    );
    let record = AgentEventRecord::new(11, original.clone());

    let decoded = roundtrip(&record);

    assert_eq!(decoded, record);
    assert!(
        matches!(
            &decoded.event.kind,
            AgentEventKind::Raw { event_type, raw_json }
                if event_type == "外星事件"
                    && raw_json == "{\"kind\":\"外星事件\",\"note\":\"引号\\\"与emoji🚀\"}"
        ),
        "Raw 变体原文（中文/emoji/引号）嵌装往返保真"
    );
}

#[test]
fn 嵌装往返大seq与大run_id打包键经编解码不回绕() {
    // u128 打包键超 u64 上界：经 serde 十六进制字符串 serde 化往返后仍还原一致
    let record = AgentEventRecord::new(i64::MAX, raw(u64::MAX));

    let decoded = roundtrip(&record);

    assert_eq!(decoded.event_key, record.event_key, "最大打包键往返一致");
    assert_eq!(decoded.run_id(), i64::MAX);
    assert_eq!(decoded.seq(), u64::MAX);
}

#[test]
fn 嵌装往返serde线格式event_key为十六进制字符串() {
    // serde_json 无 u128 数字面：event_key 定制为十六进制字符串（信封 API 的
    // JSON 可表达性前提）；此处断言 serde 层序列化形态（native_model 封装头
    // 之外的载荷编码即此形态）
    let record = AgentEventRecord::new(1, raw(2));
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");

    assert_eq!(
        value["eventKey"],
        serde_json::json!(format!("{:#034x}", record.event_key)),
        "eventKey 以十六进制字符串呈现（合法 JSON、无二进制）"
    );
    assert_eq!(value["runId"], serde_json::json!(1));
    assert_eq!(value["event"]["seq"], serde_json::json!(2));
}

// ---------------------------------------------------------------------------
// AgentRunRecord v2 存量升级与三枚举线格式（AC-2 / AC-1）：
// v1→v2→v3 链式升级、野值 fail-fast、serde 值域全组合、存量 fixture 库
// 打开与重放。fixture 升级走 native_model 真实读路径（native_db 读记录即
// `native_model::decode`），库打开 / 读取 / 重放经 Store 公共 API 真实组合。
// ---------------------------------------------------------------------------

/// v2 形态 16 字段记录（枚举化前写入形态，三受控字符串 + 全字段非缺省值）。
fn v2_record(id: i64) -> AgentRunRecordV2 {
    AgentRunRecordV2 {
        id,
        prompt: "枚举化前的一轮".to_owned(),
        cwd: "C:\\ws\\legacy".to_owned(),
        env: "bare".to_owned(),
        permission_mode: "acceptEdits".to_owned(),
        status: "completed".to_owned(),
        started_at: 1726000000000,
        finished_at: Some(1726000001000),
        num_turns: Some(5),
        cost_usd: Some(0.25),
        duration_ms: Some(4321),
        session_id: Some("s-legacy".to_owned()),
        error: Some("历史失败记因".to_owned()),
        source: "explore".to_owned(),
        source_ref: Some("7".to_owned()),
        parent_run_id: Some(41),
    }
}

/// v1 形态 13 字段记录（演进前写入形态，三受控字符串 + 全字段非缺省值）。
fn v1_record(id: i64) -> AgentRunRecordV1 {
    AgentRunRecordV1 {
        id,
        prompt: "演进前的一轮".to_owned(),
        cwd: "C:\\ws\\legacy".to_owned(),
        env: "bare".to_owned(),
        permission_mode: "acceptEdits".to_owned(),
        status: "completed".to_owned(),
        started_at: 1726000000000,
        finished_at: Some(1726000001000),
        num_turns: Some(5),
        cost_usd: Some(0.25),
        duration_ms: Some(4321),
        session_id: Some("s-legacy".to_owned()),
        error: None,
    }
}

#[test]
fn v2载荷经读路径升级v3三字段转枚举且其余十三字段保真() {
    // native_db 读记录 = `native_model::decode`（serialization.rs 同一调用）：
    // v2 版本头字节经 From<AgentRunRecordV2> 自动升级 v3（AC-2，无手工迁移）
    let v2 = v2_record(42);
    let bytes = native_model::encode(&v2).expect("v2 编码应成功");

    let (upgraded, source_version) =
        native_model::decode::<AgentRunRecord>(bytes).expect("v2 字节应被 v3 模型读路径消费");

    assert_eq!(source_version, 2, "存量字节确为 v2 版本头（升级输入前提）");
    assert_eq!(
        <AgentRunRecord as native_model::Model>::native_model_version(),
        3,
        "升级落点为 v3（模型注册版本）"
    );
    // 三受控字符串转对应枚举变体
    assert_eq!(upgraded.env, AgentEnvMode::Bare);
    assert_eq!(upgraded.permission_mode, AgentPermissionMode::AcceptEdits);
    assert_eq!(upgraded.status, AgentRunStatus::Completed);
    // 其余 13 字段逐字段保真（不经被测的 From 构造期望值）
    assert_eq!(upgraded.id, v2.id);
    assert_eq!(upgraded.prompt, v2.prompt);
    assert_eq!(upgraded.cwd, v2.cwd);
    assert_eq!(upgraded.started_at, v2.started_at);
    assert_eq!(upgraded.finished_at, v2.finished_at);
    assert_eq!(upgraded.num_turns, v2.num_turns);
    assert_eq!(upgraded.cost_usd, v2.cost_usd);
    assert_eq!(upgraded.duration_ms, v2.duration_ms);
    assert_eq!(upgraded.session_id, v2.session_id);
    assert_eq!(upgraded.error, v2.error);
    assert_eq!(upgraded.source, v2.source);
    assert_eq!(upgraded.source_ref, v2.source_ref);
    assert_eq!(upgraded.parent_run_id, v2.parent_run_id);
}

#[test]
fn v1载荷经链式升级v3枚举断言source缺省debug且十三字段保真() {
    // v1 → v2 → v3 链式自动升级（AC-2 无手工迁移）：source 缺省 debug、
    // source_ref / parent_run_id 为 None，既有 13 字段经枚举化逐值保真
    let v1 = v1_record(7);
    let bytes = native_model::encode(&v1).expect("v1 编码应成功");

    let (upgraded, source_version) =
        native_model::decode::<AgentRunRecord>(bytes).expect("v1 字节应被 v3 模型读路径消费");

    assert_eq!(
        source_version, 1,
        "存量字节确为 v1 版本头（链式升级输入前提）"
    );
    assert_eq!(upgraded.source, "debug", "v1 记录 source 缺省 debug");
    assert_eq!(upgraded.source_ref, None, "v1 记录无来源定位");
    assert_eq!(upgraded.parent_run_id, None, "v1 记录无链指针");
    assert_eq!(upgraded.env, AgentEnvMode::Bare);
    assert_eq!(upgraded.permission_mode, AgentPermissionMode::AcceptEdits);
    assert_eq!(upgraded.status, AgentRunStatus::Completed);
    // 既有 13 字段保真
    assert_eq!(upgraded.id, v1.id);
    assert_eq!(upgraded.prompt, v1.prompt);
    assert_eq!(upgraded.cwd, v1.cwd);
    assert_eq!(upgraded.started_at, v1.started_at);
    assert_eq!(upgraded.finished_at, v1.finished_at);
    assert_eq!(upgraded.num_turns, v1.num_turns);
    assert_eq!(upgraded.cost_usd, v1.cost_usd);
    assert_eq!(upgraded.duration_ms, v1.duration_ms);
    assert_eq!(upgraded.session_id, v1.session_id);
    assert_eq!(upgraded.error, v1.error);
}

#[test]
fn v3记录serde线格式三枚举字段为受控驼峰串与枚举化前逐字一致() {
    // serde camelCase JSON 线格式零变化（AC-1）：env / permissionMode / status
    // 出线为受控字符串，键名 camelCase
    let record = AgentRunRecord {
        id: 1,
        prompt: "线格式回归".to_owned(),
        cwd: "C:\\ws\\demo".to_owned(),
        env: AgentEnvMode::Default,
        permission_mode: AgentPermissionMode::BypassPermissions,
        status: AgentRunStatus::Running,
        started_at: 1727000000000,
        finished_at: None,
        num_turns: None,
        cost_usd: None,
        duration_ms: None,
        session_id: None,
        error: None,
        source: "debug".to_owned(),
        source_ref: None,
        parent_run_id: None,
    };
    let value = serde_json::to_value(&record).expect("serde 序列化应成功");

    assert_eq!(value["env"], "default");
    assert_eq!(value["permissionMode"], "bypassPermissions");
    assert_eq!(value["status"], "running");
    // 反序列化 roundtrip 一致（线格式双向受控）
    let roundtrip: AgentRunRecord = serde_json::from_value(value).expect("反序列化成功");
    assert_eq!(roundtrip, record);
}

#[test]
fn 三枚举全组合serde串值域逐字断言() {
    // env 2 × permission-mode 3 × status 4 = 24 组合穷尽：serde 串值域的
    // 机械化回归面（值域扫描结论的锁定形态，AC-1）
    let env_wire = |mode: AgentEnvMode| match mode {
        AgentEnvMode::Default => "default",
        AgentEnvMode::Bare => "bare",
    };
    let mode_wire = |mode: AgentPermissionMode| match mode {
        AgentPermissionMode::Default => "default",
        AgentPermissionMode::AcceptEdits => "acceptEdits",
        AgentPermissionMode::BypassPermissions => "bypassPermissions",
    };
    let status_wire = |status: AgentRunStatus| match status {
        AgentRunStatus::Running => "running",
        AgentRunStatus::Completed => "completed",
        AgentRunStatus::Failed => "failed",
        AgentRunStatus::Stopped => "stopped",
    };

    let mut combos = 0;
    for env in [AgentEnvMode::Default, AgentEnvMode::Bare] {
        for permission_mode in [
            AgentPermissionMode::Default,
            AgentPermissionMode::AcceptEdits,
            AgentPermissionMode::BypassPermissions,
        ] {
            for status in [
                AgentRunStatus::Running,
                AgentRunStatus::Completed,
                AgentRunStatus::Failed,
                AgentRunStatus::Stopped,
            ] {
                let serialized = serde_json::to_string(&(env, permission_mode, status))
                    .expect("组合序列化应成功");
                assert_eq!(
                    serialized,
                    format!(
                        "[\"{}\",\"{}\",\"{}\"]",
                        env_wire(env),
                        mode_wire(permission_mode),
                        status_wire(status)
                    ),
                    "组合 ({env:?}, {permission_mode:?}, {status:?}) 串值域逐字一致"
                );
                combos += 1;
            }
        }
    }
    assert_eq!(combos, 24, "全组合恰 2×3×4=24 项穷尽");
}

#[test]
#[should_panic(expected = "status 含野值: \"succeeded\"")]
fn v2载荷status含清单外字符串时升级转换panic记因含原值() {
    // 野值 fail-fast（design 定夺：不兜底变体；数据损坏不伪装成合法状态）：
    // 升级转换 panic 且 panic 信息含原字符串值
    let mut wild = v2_record(99);
    wild.status = "succeeded".to_owned();
    let bytes = native_model::encode(&wild).expect("v2 编码应成功");

    let _ = native_model::decode::<AgentRunRecord>(bytes).expect("野值载荷必须 panic");
}

#[test]
fn v2存量fixture库打开后读取与事件重放自动升级无手工迁移() {
    // 跨模块组合用例（AC-2）：枚举化前形态（v2 载荷）→ store 读路径（真实
    // db 文件 + Store 公共 API）——fixture 库打开、记录读取与事件重放全链
    // 成功，无手工迁移步骤。
    let dir = tempfile::Builder::new()
        .prefix("model-test-v2-fixture-")
        .tempdir()
        .expect("创建临时目录失败");
    let db_path = dir.path().join("test.redb");

    // 存量 v2 载荷经 native_model 真实读路径升级（native_db 读记录同款调用）
    let legacy = v2_record(1);
    let v2_bytes = native_model::encode(&legacy).expect("v2 编码应成功");
    let (mut upgraded, source_version) =
        native_model::decode::<AgentRunRecord>(v2_bytes).expect("存量 v2 字节应自动升级");
    assert_eq!(source_version, 2, "fixture 载荷确为枚举化前 v2 版本头");
    assert_eq!(upgraded.status, AgentRunStatus::Completed);
    assert_eq!(upgraded.env, AgentEnvMode::Bare);
    assert_eq!(upgraded.permission_mode, AgentPermissionMode::AcceptEdits);

    // fixture 库：真开 db → 落 running 行 + 事件 → 以升级后的记录收敛终态
    // （块结束即 drop 写句柄，重开走真实打开流程）
    let event = AgentEvent::stamp(
        0,
        AgentEventKind::Message {
            role: "assistant".to_owned(),
            blocks: Vec::new(),
            parent_tool_use_id: None,
        },
    );
    {
        let store = Store::open(&db_path).expect("fixture 库应可打开");
        let running = AgentRunRecord {
            status: AgentRunStatus::Running,
            finished_at: None,
            ..upgraded.clone()
        };
        let begun = store.begin_agent_run(&running).expect("begin 应成功");
        assert_eq!(begun.id, 1, "空库首跑 max+1 分配 id=1");
        store
            .append_agent_run_events(begun.id, std::slice::from_ref(&event))
            .expect("事件追加应成功");
        upgraded.id = begun.id;
        store
            .finish_agent_run(begun.id, &upgraded)
            .expect("升级后的终态记录应可落库");
    }

    // 重开 fixture 库：读取与事件重放成功（三字段以枚举形态读回，无手工迁移）
    let reopened = Store::open(&db_path).expect("存量 fixture 库重开应直通");
    let listed = reopened.list_agent_runs().expect("读取应成功");
    assert_eq!(listed, vec![upgraded.clone()], "升级记录逐字段读回");
    assert_eq!(listed[0].status, AgentRunStatus::Completed);
    assert_eq!(
        reopened
            .list_agent_run_events(upgraded.id)
            .expect("重放应成功"),
        vec![event],
        "事件重放成功"
    );
}
