use agent::{
    AgentBlock, AgentEnvMode, AgentEvent, AgentEventKind, AgentPermissionMode, AgentRunStatus,
};

use crate::model::{
    pack_event_key, AgentEngineKind, AgentEventRecord, AgentInstanceRecord, AgentModelTiers,
    AgentProviderRecord, AgentRunRecord,
};

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
// AgentRunRecord 三枚举线格式（AC-1）：serde 值域全组合
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// agent 管理记录（AC-9 / AC-6 存储半边）：构造语义、遮蔽 Debug、嵌装往返、
// serde 线格式（内存构造 + native_model 封装内存往返，无 mock）
// ---------------------------------------------------------------------------

/// 三档模型 fixture（high / medium / low 三档可区分，消费半边断言取 high 档）。
fn tiers(high: &str, medium: &str, low: &str) -> AgentModelTiers {
    AgentModelTiers {
        high: high.to_owned(),
        medium: medium.to_owned(),
        low: low.to_owned(),
    }
}

#[test]
fn provider构造new三字段载荷与三档models逐字段保真且id置0比较走partial_eq与字段面() {
    let record = AgentProviderRecord::new(
        "自建端点".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
    );

    // 新建语义构造：id 置 0（写事务 max+1 分配覆盖）
    assert_eq!(record.id, 0, "new 构造 id 恒置 0");
    // 逐字段保真（字段面比较，EngineConfig 同口径不走 Debug）
    assert_eq!(record.name, "自建端点");
    assert_eq!(record.base_url, "https://api.example.com/v1");
    assert_eq!(record.api_key, "sk-live-1234567890");
    assert_eq!(
        record.models,
        tiers("m-high", "m-medium", "m-low"),
        "三档 models 逐字段保真（PartialEq）"
    );

    // Clone / PartialEq 保真
    let cloned = record.clone();
    assert_eq!(cloned, record, "Clone 后逐字段相等");
}

#[test]
fn provider手写遮蔽debug输出api_key位为末三字符遮蔽形态_全文不含明文key() {
    let record = AgentProviderRecord::new(
        "遮蔽回归".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
    );

    let debug_text = format!("{record:?}");

    // 手写遮蔽 Debug：api_key 位呈现 sk-***abc 形态（前缀掩码 + 末 3 字符）
    assert!(
        debug_text.contains("sk-***890"),
        "Debug 输出 api_key 位为「sk-*** + 末 3 字符」遮蔽形态，实际: {debug_text}"
    );
    // 结构保证明文不进日志：完整 key 全文不出现
    assert!(
        !debug_text.contains("sk-live-1234567890"),
        "Debug 输出不得含 api_key 明文，实际: {debug_text}"
    );
    // 结构体名与字段名在位（字段面与 derive 形态对齐，仅 api_key 位遮蔽）
    assert!(debug_text.contains("AgentProviderRecord"));
    assert!(debug_text.contains("api_key"));
    assert!(debug_text.contains("base_url"));
}

#[test]
fn provider遮蔽debug在api_key空串或长度不足3字符时恒sk三星号全遮蔽兜底() {
    for api_key in ["", "a", "ab", "abc"] {
        let record = AgentProviderRecord::new(
            "过短兜底".to_owned(),
            "https://api.example.com/v1".to_owned(),
            api_key.to_owned(),
            tiers("h", "m", "l"),
        );

        let debug_text = format!("{record:?}");

        assert!(
            debug_text.contains("sk-***"),
            "api_key={api_key:?} 过短时 Debug 恒含 sk-*** 掩码，实际: {debug_text}"
        );
        // 全遮蔽兜底：api_key 位恒为纯掩码（无末 3 字符回显，防 sk-***abc 恰为
        // 原文泄露——恰 3 字符时回显后缀即泄露原文）
        assert!(
            debug_text.contains("api_key: \"sk-***\""),
            "api_key={api_key:?} 过短时 api_key 位恒为 sk-*** 全遮蔽，实际: {debug_text}"
        );
    }
}

#[test]
fn provider构造特殊字符字段clone与partial_eq保真且models三档全空串构造合法() {
    let record = AgentProviderRecord::new(
        "名 字 中文 🎉 \"引号\"\n换行".to_owned(),
        "https://例子.测试/v1 🚀".to_owned(),
        "带 空格 的 \"key\" 🎉\n".to_owned(),
        tiers("", "", ""),
    );

    // 模型层不做字段校验：三档全空串构造合法（校验单点在 store）
    assert_eq!(record.models.high, "");
    assert_eq!(record.models.medium, "");
    assert_eq!(record.models.low, "");

    let cloned = record.clone();
    assert_eq!(cloned, record, "特殊字符字段 Clone / PartialEq 保真");
}

#[test]
fn instance构造new恒id0与is_defaultfalse且engine两变体与provider_id两态逐字段保真() {
    // 构造器不产默认标记：is_default 恒 false（默认标记唯一写口为 set_default）
    let cli_agent =
        AgentInstanceRecord::new("cli-甲".to_owned(), AgentEngineKind::Cli, None);
    assert_eq!(cli_agent.id, 0, "new 构造 id 恒置 0");
    assert!(!cli_agent.is_default, "new 构造恒非默认");
    assert_eq!(cli_agent.name, "cli-甲");
    assert_eq!(cli_agent.engine, AgentEngineKind::Cli);
    assert_eq!(cli_agent.provider_id, None, "cli 臂 provider 可空透传");

    let sdk_agent =
        AgentInstanceRecord::new("sdk-乙".to_owned(), AgentEngineKind::Sdk, Some(7));
    assert_eq!(sdk_agent.id, 0);
    assert!(!sdk_agent.is_default);
    assert_eq!(sdk_agent.engine, AgentEngineKind::Sdk);
    assert_eq!(sdk_agent.provider_id, Some(7), "provider_id Some 两态透传");

    // Clone / PartialEq 保真
    assert_eq!(sdk_agent.clone(), sdk_agent);
    assert_ne!(cli_agent, sdk_agent, "两形态记录不等");
}

/// provider 记录的 native_model 内存往返（默认 bincode codec，不经 db 文件）。
fn provider_roundtrip(record: &AgentProviderRecord) -> AgentProviderRecord {
    let bytes = native_model::encode(record).expect("native_model encode 应成功");
    let (decoded, version) =
        native_model::decode::<AgentProviderRecord>(bytes).expect("native_model decode 应成功");
    assert_eq!(version, 1, "native_model 版本封装为 version 1（id 5 新登记不与既有 1–4 冲突由打开成功锚定）");
    decoded
}

#[test]
fn provider记录嵌装往返含三档models逐字段相等() {
    let record = AgentProviderRecord::new(
        "往返回归".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
    );

    let decoded = provider_roundtrip(&record);

    assert_eq!(decoded, record, "provider 记录（含三档 models）往返逐字段相等");
}

#[test]
fn instance记录往返engine两变体与provider_id两态option语义经编解码不漂移() {
    for engine in [AgentEngineKind::Cli, AgentEngineKind::Sdk] {
        for provider_id in [None, Some(42)] {
            let record =
                AgentInstanceRecord::new(format!("实例-{engine:?}"), engine, provider_id);

            let bytes = native_model::encode(&record).expect("native_model encode 应成功");
            let (decoded, version) = native_model::decode::<AgentInstanceRecord>(bytes)
                .expect("native_model decode 应成功");
            assert_eq!(version, 1, "native_model 版本封装为 version 1");
            assert_eq!(decoded, record, "instance 记录往返保真");
            assert_eq!(
                decoded.provider_id, provider_id,
                "Option 语义经编解码不漂移（None / Some 两态）"
            );
        }
    }
}

#[test]
fn 管理记录serde线格式键名为小驼峰且engine出线cli与sdk串值() {
    let provider = AgentProviderRecord::new(
        "线格式".to_owned(),
        "https://api.example.com/v1".to_owned(),
        "sk-live-1234567890".to_owned(),
        tiers("m-high", "m-medium", "m-low"),
    );
    let provider_value = serde_json::to_value(&provider).expect("serde 序列化应成功");
    assert_eq!(provider_value["name"], serde_json::json!("线格式"));
    assert_eq!(provider_value["baseUrl"], serde_json::json!("https://api.example.com/v1"));
    assert_eq!(provider_value["apiKey"], serde_json::json!("sk-live-1234567890"));
    assert_eq!(provider_value["models"]["high"], serde_json::json!("m-high"));
    assert_eq!(provider_value["models"]["medium"], serde_json::json!("m-medium"));
    assert_eq!(provider_value["models"]["low"], serde_json::json!("m-low"));

    let instance = AgentInstanceRecord::new(
        "线格式实例".to_owned(),
        AgentEngineKind::Sdk,
        Some(7),
    );
    let instance_value = serde_json::to_value(&instance).expect("serde 序列化应成功");
    assert_eq!(instance_value["engine"], serde_json::json!("sdk"));
    assert_eq!(instance_value["providerId"], serde_json::json!(7));
    assert_eq!(instance_value["isDefault"], serde_json::json!(false));

    let cli = AgentInstanceRecord::new(
        "线格式cli".to_owned(),
        AgentEngineKind::Cli,
        None,
    );
    let cli_value = serde_json::to_value(&cli).expect("serde 序列化应成功");
    assert_eq!(cli_value["engine"], serde_json::json!("cli"), "engine 出线 \"cli\"");
    assert_eq!(cli_value["providerId"], serde_json::json!(null));

    // 反序列化 roundtrip 一致（线格式双向受控）
    let provider_back: AgentProviderRecord =
        serde_json::from_value(provider_value).expect("反序列化成功");
    assert_eq!(provider_back, provider);
    let instance_back: AgentInstanceRecord =
        serde_json::from_value(instance_value).expect("反序列化成功");
    assert_eq!(instance_back, instance);
}

#[test]
fn serde线格式非法engine串反序列化err() {
    // 受控值域拒绝：非 "cli"/"sdk" 串反序列化 Err（与既有三枚举线格式口径同型）
    let result = serde_json::from_value::<AgentEngineKind>(serde_json::json!("yolo"));
    assert!(
        result.is_err(),
        "非法 engine 串应 Err，实际: {result:?}"
    );
}
