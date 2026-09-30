//! `EngineConfig` 的单元测试（AC-2 前置）：MVP 硬编码预留位构造的三字段
//! 保真、空缺省形状锁定（SdkRunner start 以 ConfigMissing 失败的前提）、
//! `is_complete` 三件套判定与纯函数性。纯结构体构造断言，无进程边界依赖，
//! 不需要 Mock。

use agent::AgentStartError;

use crate::sdk::config::EngineConfig;

// ---------------------------------------------------------------------------
// 硬编码预留位构造：三件套字段保真（正向）
// ---------------------------------------------------------------------------

#[test]
fn from_hardcoded_slot返回三字段结构体且手填值逐字段保真可读() {
    // 构造体三字段形状冻结（api_key / base_url / model 三 String）
    let config = EngineConfig {
        api_key: "sk-test-🎉".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model: "test-model".to_owned(),
    };
    assert_eq!(config.api_key, "sk-test-🎉");
    assert_eq!(config.base_url, "https://api.example.com/v1");
    assert_eq!(config.model, "test-model");

    // 硬编码预留位：未手填形态三字段空串（空缺省值锁定，防实现引入隐式缺省）
    let empty = EngineConfig::from_hardcoded_slot();
    assert_eq!(empty.api_key, "", "api_key 缺省空串");
    assert_eq!(empty.base_url, "", "base_url 缺省空串");
    assert_eq!(empty.model, "", "model 缺省空串");
}

// ---------------------------------------------------------------------------
// 空缺省形状 → SdkRunner start 显式失败（异常半边的前提锁定）
// ---------------------------------------------------------------------------

#[test]
fn 未手填缺省形态经is_complete判定为缺失且sdk启动以config_missing失败() {
    let empty = EngineConfig::from_hardcoded_slot();
    assert!(!empty.is_complete(), "三字段空串 → 配置缺失判定");

    // 空缺省形状是 SdkRunner start 以 ConfigMissing 显式失败的前提（首个
    // 消费点经门面分发锚定：v2 起缺省收敛 DEFAULT_ENGINE 后该失败路径覆盖
    // 全部未传 engine 调用）。门面构造的 SdkRunner 对空配置 start 即 Err。
    use crate::EngineFacade;
    use agent::{AgentPermissionMode, AgentRunParams};
    use std::path::PathBuf;

    let runner = EngineFacade::new().runner_for(crate::EngineKind::Sdk, empty);
    let error = runner
        .start(AgentRunParams {
            prompt: "配置缺失轮".to_owned(),
            cwd: PathBuf::from("C:\\ws"),
            permission_mode: AgentPermissionMode::BypassPermissions,
            resume_session_id: None,
        })
        .expect_err("空配置 start 必须显式失败");
    assert!(
        matches!(error, AgentStartError::ConfigMissing(ref msg) if msg.contains("api_key") && msg.contains("base_url") && msg.contains("model")),
        "启动校验区分缺失字段成因，实际: {error:?}"
    );

    // 手填齐备后 is_complete 判定翻正（真机验证路径的前置形状）
    let complete = EngineConfig {
        api_key: "k".to_owned(),
        base_url: "https://api.example.com/v1".to_owned(),
        model: "m".to_owned(),
    };
    assert!(complete.is_complete());
}

// ---------------------------------------------------------------------------
// 纯函数性（边界）：重复调用等值 / 特殊字符字段保真
// （EngineConfig 刻意不派生 Debug——防凭据进日志——比较一律走 assert!/字段面）
// ---------------------------------------------------------------------------

#[test]
fn 重复调用返回等值结构且特殊字符字段构造保真() {
    // 无隐藏状态、无副作用：重复调用返回等值结构
    let first = EngineConfig::from_hardcoded_slot();
    let second = EngineConfig::from_hardcoded_slot();
    assert!(first == second, "纯构造：重复调用等值");

    // 字段含特殊字符（空格 / 中文 / emoji）时构造保真（PartialEq 按载荷逐字段）
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
