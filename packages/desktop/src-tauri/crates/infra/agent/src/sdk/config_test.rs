use agent::AgentStartError;

use crate::sdk::config::{EngineConfig, Protocol};

// ---------------------------------------------------------------------------
// empty() 构造：三字段全空占位（正向，空缺省形状锁定）
// ---------------------------------------------------------------------------

#[test]
fn empty返回api_key_base_url_model三字段全空串() {
    // 空缺省形状锁定（防实现引入隐式缺省）：CLI 臂不消费的占位构造
    let empty = EngineConfig::empty();
    assert_eq!(empty.api_key, "", "api_key 缺省空串");
    assert_eq!(empty.base_url, "", "base_url 缺省空串");
    assert_eq!(empty.model_high, "", "model 缺省空串");
    assert_eq!(empty.model_low, "", "model 缺省空串");

    // 构造体三字段形状冻结（api_key / base_url / model 三 String）：手填构造
    // 逐字段保真可读（解析单点产物的字段面）
    let config = EngineConfig {
        protocol: Protocol::Openai,
        api_key: "sk-test-🎉".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model_high: "test-model-high".to_owned(),
        model_low: "test-model-low".to_owned(),
    };
    assert_eq!(config.api_key, "sk-test-🎉");
    assert_eq!(config.base_url, "https://api.example.com/v1");
    assert_eq!(config.model_high, "test-model-high");
    assert_eq!(config.model_low, "test-model-low");
}

// ---------------------------------------------------------------------------
// 纯函数性（边界）：重复调用等值 / 特殊字符字段保真
// （EngineConfig 刻意不派生 Debug——防凭据进日志——比较一律走 assert!/字段面）
// ---------------------------------------------------------------------------

#[test]
fn 重复调用empty返回等值结构且特殊字符字段构造保真() {
    // 无隐藏状态、无副作用：重复调用返回等值结构（纯构造）
    let first = EngineConfig::empty();
    let second = EngineConfig::empty();
    assert!(first == second, "纯构造：重复调用等值");

    // 字段含特殊字符（空格 / 中文 / emoji / 换行）时构造保真（PartialEq 按载荷
    // 逐字段；既有断言换构造源后保留）
    let special = EngineConfig {
        protocol: Protocol::Openai,
        api_key: "带 空格 的 key 中文 🎉\n换行".to_owned(),
        base_url: "https://例子.测试/v1 🚀".to_owned(),
        model_high: "模型/名: v1".to_owned(),
        model_low: "模型/名: v2".to_owned(),
    };
    assert_eq!(special.api_key, "带 空格 的 key 中文 🎉\n换行");
    assert_eq!(special.base_url, "https://例子.测试/v1 🚀");
    assert_eq!(special.model_high, "模型/名: v1");
    assert_eq!(special.model_low, "模型/名: v2");
    let cloned = special.clone();
    assert!(cloned == special, "Clone 后逐字段保真");
}

// ---------------------------------------------------------------------------
// 空配置 → SdkRunner start 显式失败（异常半边的前提锁定）
// ---------------------------------------------------------------------------

#[test]
fn empty经is_complete判定为缺失且sdk启动以config_missing失败且文案区分三字段名() {
    let empty = EngineConfig::empty();
    assert!(!empty.is_complete(), "三字段空串 → 配置缺失判定");

    // 空缺省形状是 SdkRunner open_session 以 ConfigMissing 显式失败的前提（首个
    // 消费点经门面分发锚定）。门面构造的 SdkRunner 对空配置 open_session 即 Err。
    use crate::EngineFacade;
    use std::path::PathBuf;

    let runner = EngineFacade::new()
        .runner_for(crate::EngineKind::Sdk, empty)
        .expect("Sdk 臂构造成功");
    let error = runner
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: PathBuf::from("C:\\ws"),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
                model_level: agent::ModelLevel::High,
            },
            session: agent::SessionRef::New,
            prior_handle: None,
        })
        .expect_err("空配置 open_session 必须显式失败");
    assert!(
        matches!(error, AgentStartError::ConfigMissing(ref msg) if msg.contains("api_key") && msg.contains("base_url") && msg.contains("model")),
        "启动校验区分缺失字段成因，实际: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// 手填齐备形态（正向）：解析单点产物形态（三字段齐备）is_complete 判定翻正
// ---------------------------------------------------------------------------

#[test]
fn 手填齐备形态三字段齐备is_complete判定翻正() {
    // 解析单点产物形态：api_key / base_url 自引用 provider、model 取三档 high 档
    let complete = EngineConfig {
        protocol: Protocol::Openai,
        api_key: "sk-live-1234567890".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model_high: "m-high".to_owned(),
        model_low: "m-low".to_owned(),
    };
    assert!(complete.is_complete(), "三字段齐备 is_complete 判定翻正");

    // 任一字段空即缺失（三件套逐字段判定）
    let no_key = EngineConfig {
        protocol: Protocol::Openai,
        api_key: String::new(),
        base_url: "https://api.example.com/v1".to_owned(),
        model_high: "m-high".to_owned(),
        model_low: "m-low".to_owned(),
    };
    let no_url = EngineConfig {
        protocol: Protocol::Openai,
        api_key: "k".to_owned(),
        base_url: String::new(),
        model_high: "m-high".to_owned(),
        model_low: "m-low".to_owned(),
    };
    let no_model = EngineConfig {
        protocol: Protocol::Openai,
        api_key: "k".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model_high: String::new(),
        model_low: String::new(),
    };
    assert!(!no_key.is_complete(), "缺 api_key 判定缺失");
    assert!(!no_url.is_complete(), "缺 base_url 判定缺失");
    assert!(!no_model.is_complete(), "缺 model 判定缺失");
}

// ---------------------------------------------------------------------------
// Protocol：日志线值 / 缺省 / 值语义（DD1 / DD12）
// ---------------------------------------------------------------------------

#[test]
fn protocol_as_str线值逐字为openai与anthropic() {
    // 日志 / 诊断面 `protocol=…` 的取值锚：不得为 debug 名或大小写变体
    assert_eq!(Protocol::Openai.as_str(), "openai", "openai 线值逐字");
    assert_eq!(
        Protocol::Anthropic.as_str(),
        "anthropic",
        "anthropic 线值逐字"
    );
    // 两变体线值互异（日志可区分）
    assert_ne!(Protocol::Openai.as_str(), Protocol::Anthropic.as_str());
}

#[test]
fn protocol缺省为openai且copy与eq可用() {
    // EngineConfig::empty() 的协议缺省来源
    assert!(
        Protocol::default() == Protocol::Openai,
        "缺省变体为 openai（存量行行为零变化）"
    );
    let copied = Protocol::Anthropic;
    assert!(copied == Protocol::Anthropic, "Copy 后等值");
    // 两变体互不相等
    assert!(Protocol::Openai != Protocol::Anthropic, "两变体互不相等");
    // 引擎内部词汇：不出 crate、不进 IPC bindings（无 serde / specta 派生——
    // 编译期锚：若派生则下方「无 serde 派生」的注释失效，此处以 Debug 名锚定）
    assert_eq!(
        format!("{:?}", Protocol::Anthropic),
        "Anthropic",
        "变体名与门面词汇同源（无 serde 线值形态）"
    );
}

// ---------------------------------------------------------------------------
// EngineConfig 协议字段：empty 缺省 / 比较语义 / 字段载荷边界
// ---------------------------------------------------------------------------

#[test]
fn empty协议缺省openai且手填anthropic逐字段保真() {
    assert!(
        EngineConfig::empty().protocol == Protocol::Openai,
        "empty() 协议缺省 openai（无隐式缺省）"
    );

    // 手填构造（含 anthropic）逐字段保真可读
    let anthropic = EngineConfig {
        protocol: Protocol::Anthropic,
        api_key: "sk-ant-1".to_owned(),
        base_url: "https://api.anthropic.com".to_owned(),
        model_high: "claude-sonnet-4-5[1m]".to_owned(),
        model_low: "claude-haiku-4-5".to_owned(),
    };
    assert!(anthropic.protocol == Protocol::Anthropic);
    assert_eq!(anthropic.base_url, "https://api.anthropic.com");
    assert_eq!(anthropic.model_high, "claude-sonnet-4-5[1m]");
    assert_eq!(anthropic.model_low, "claude-haiku-4-5");
}

#[test]
fn is_complete语义不变_协议不参与校验() {
    let base = EngineConfig {
        protocol: Protocol::Openai,
        api_key: "k".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model_high: "m-high".to_owned(),
        model_low: "m-low".to_owned(),
    };
    assert!(base.is_complete(), "四字段齐备即 true");

    // 协议为任一值均不影响判定（协议恒有值，不参与校验）
    let anthropic = EngineConfig {
        protocol: Protocol::Anthropic,
        ..base.clone()
    };
    assert!(anthropic.is_complete(), "协议 anthropic 时仍 true");

    // 四字段各自单独为空 → false（表驱动锁逐字段判定）
    let cases: [(&str, EngineConfig); 4] = [
        (
            "api_key",
            EngineConfig {
                api_key: String::new(),
                ..base.clone()
            },
        ),
        (
            "base_url",
            EngineConfig {
                base_url: String::new(),
                ..base.clone()
            },
        ),
        (
            "model_high",
            EngineConfig {
                model_high: String::new(),
                ..base.clone()
            },
        ),
        (
            "model_low",
            EngineConfig {
                model_low: String::new(),
                ..base.clone()
            },
        ),
    ];
    for (field, config) in cases {
        assert!(!config.is_complete(), "{field} 单独缺失即 false");
        // 协议无关：同缺失形态换成 anthropic 判定一致
        let flipped = EngineConfig {
            protocol: Protocol::Anthropic,
            ..config
        };
        assert!(!flipped.is_complete(), "{field} 缺失判定不随协议漂移");
    }

    // 多字段同时缺失仍 false
    let multi = EngineConfig {
        protocol: Protocol::Anthropic,
        api_key: String::new(),
        base_url: String::new(),
        model_high: String::new(),
        model_low: "m-low".to_owned(),
    };
    assert!(!multi.is_complete(), "多字段同时缺失仍 false");
}

#[test]
fn is_complete只判is_empty_空白串非空即true且partial_eq按协议与字段面() {
    // 空白串 / \0 / emoji / 超长字段：is_complete 只判 is_empty()，不新增 trim
    let blank = EngineConfig {
        protocol: Protocol::Openai,
        api_key: "   ".to_owned(),
        base_url: "\t".to_owned(),
        model_high: "\u{0}".to_owned(),
        model_low: "😀".to_owned(),
    };
    assert!(
        blank.is_complete(),
        "全空白 / 特殊字符串非空即 true（语义与现状一致）"
    );

    let long = EngineConfig {
        protocol: Protocol::Anthropic,
        api_key: "k".repeat(1_500),
        base_url: format!("https://例子.测试/{}/v1", "x".repeat(1_500)),
        model_high: format!("{}-high", "m".repeat(1_500)),
        model_low: format!("{}-low", "m".repeat(1_500)),
    };
    assert!(long.is_complete(), "超长字段非空即 true");

    // PartialEq：仅协议不同则不等；其余字段相同且同协议则相等
    let openai = EngineConfig {
        protocol: Protocol::Openai,
        ..long.clone()
    };
    let anthropic = EngineConfig {
        protocol: Protocol::Anthropic,
        ..long.clone()
    };
    assert!(openai != anthropic, "仅协议不同两配置不相等");
    assert!(anthropic == long.clone(), "同协议同字段相等");
    assert!(
        anthropic.clone() == long,
        "Clone 后相等（PartialEq 不受协议位影响）"
    );
}
