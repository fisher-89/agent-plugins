use agent::AgentStartError;

use crate::sdk::config::EngineConfig;

// ---------------------------------------------------------------------------
// empty() 构造：三字段全空占位（正向，空缺省形状锁定）
// ---------------------------------------------------------------------------

#[test]
fn empty返回api_key_base_url_model三字段全空串() {
    // 空缺省形状锁定（防实现引入隐式缺省）：CLI 臂不消费的占位构造
    let empty = EngineConfig::empty();
    assert_eq!(empty.api_key, "", "api_key 缺省空串");
    assert_eq!(empty.base_url, "", "base_url 缺省空串");
    assert_eq!(empty.model, "", "model 缺省空串");

    // 构造体三字段形状冻结（api_key / base_url / model 三 String）：手填构造
    // 逐字段保真可读（解析单点产物的字段面）
    let config = EngineConfig {
        api_key: "sk-test-🎉".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model: "test-model".to_owned(),
    };
    assert_eq!(config.api_key, "sk-test-🎉");
    assert_eq!(config.base_url, "https://api.example.com/v1");
    assert_eq!(config.model, "test-model");
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
        api_key: "带 空格 的 key 中文 🎉\n换行".to_owned(),
        base_url: "https://例子.测试/v1 🚀".to_owned(),
        model: "模型/名: v1".to_owned(),
    };
    assert_eq!(special.api_key, "带 空格 的 key 中文 🎉\n换行");
    assert_eq!(special.base_url, "https://例子.测试/v1 🚀");
    assert_eq!(special.model, "模型/名: v1");
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

    let runner = EngineFacade::new().runner_for(crate::EngineKind::Sdk, empty);
    let error = runner
        .open_session(agent::SessionOpen {
            injections: agent::SessionInjections::default(),
            ctx: agent::SessionCtx {
                workspace_root: PathBuf::from("C:\\ws"),
                permission_mode: agent::AgentPermissionMode::BypassPermissions,
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
        api_key: "sk-live-1234567890".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model: "m-high".to_owned(),
    };
    assert!(complete.is_complete(), "三字段齐备 is_complete 判定翻正");

    // 任一字段空即缺失（三件套逐字段判定）
    let no_key = EngineConfig {
        api_key: String::new(),
        base_url: "https://api.example.com/v1".to_owned(),
        model: "m-high".to_owned(),
    };
    let no_url = EngineConfig {
        api_key: "k".to_owned(),
        base_url: String::new(),
        model: "m-high".to_owned(),
    };
    let no_model = EngineConfig {
        api_key: "k".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model: String::new(),
    };
    assert!(!no_key.is_complete(), "缺 api_key 判定缺失");
    assert!(!no_url.is_complete(), "缺 base_url 判定缺失");
    assert!(!no_model.is_complete(), "缺 model 判定缺失");
}
