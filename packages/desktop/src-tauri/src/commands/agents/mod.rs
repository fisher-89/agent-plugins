//! agents 管理命令轨道：agent 管理两类实例（provider / agent）的 CRUD 与
//! 默认标记七命令薄包装（desktop-agent-management）。
//!
//! 数据面落**全局库**（user 维度，desktop-data-dimensions）——命令体恒经
//! `WorkspaceStores::global()` 路由，MUST NOT 误路由 workspace 库（`for_root`
//! 是 per-root 归属语义，管理数据全局共享、无归属键）。命令 body 三件事：
//! 参数转换（IPC 入参 → 记录 / store 入参，含 save_provider 的留空 key 回填
//! 原值一段）、调用（store 管理操作面）、错误映射（`StoreError` 以
//! `.to_string()` 转 `Err(String)`，本类型不进命令签名）。错误约定沿全轨道
//! 模板：`Err` 由 Tauri 转为前端 reject，MUST NOT 静默吞掉失败。

use agent_runtime::{parse_model_spec, MIN_OUTPUT_TOKENS};
use tauri::State;

use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, WorkspaceStores,
};

#[cfg(test)]
mod mod_test;

/// provider 清单（主键 id 升序）。
#[tauri::command]
#[specta::specta]
pub fn list_agent_providers(
    stores: State<'_, WorkspaceStores>,
) -> Result<Vec<AgentProviderRecord>, String> {
    stores
        .global()
        .list_agent_providers()
        .map_err(|e| e.to_string())
}

/// 保存 provider（新建 / 更新合一，id `None` 新建 / `Some` 整行更新）。
///
/// 参数序：`protocol` 紧跟 `name`——协议决定 `base_url` 语义与 wire，属连接
/// 配置核心项，签名上显式；`web_search` 居末位（provider 属性，缺省关）。
///
/// 保存期校验单点（避免「保存成功但跑不起来」）：三档模型名经引擎侧**同一**
/// `parse_model_spec`（`high → medium → low`，首个非法即 `Err`，不造第二套
/// 规则）；`max_tokens` 非空时 SHALL `>= MIN_OUTPUT_TOKENS`（`< 4096` 拒收，
/// 不静默截断或夹到边界）。错误串只含档位名 / 模型名原文 / 下限——结构上
/// 不含 api_key。
///
/// `web_search` **不因协议拒收或清零**（openai 下同收原值）——生效门在运行
/// 发起解析期（`provider.web_search && protocol == Anthropic`），保存期纠偏
/// 会让「先开开关后改协议」与「先改协议后开开关」收敛到不同记录形态。
// 九参为 IPC 命令面定稿签名（design DD16 / DD17：协议紧跟 name，可空调参相邻），
// 拆结构体只增 IPC 载荷间接且与前端实参序脱耦——沿 `agent_start` 命令面先例。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub fn save_agent_provider(
    stores: State<'_, WorkspaceStores>,
    id: Option<i64>,
    name: String,
    protocol: AgentProviderProtocol,
    base_url: String,
    api_key: String,
    models: AgentModelTiers,
    context_length: Option<u64>,
    max_tokens: Option<u64>,
    web_search: bool,
) -> Result<AgentProviderRecord, String> {
    for (tier, raw) in [
        ("high", &models.high),
        ("medium", &models.medium),
        ("low", &models.low),
    ] {
        parse_model_spec(raw).map_err(|error| format!("models.{tier} 档{error}"))?;
    }
    if let Some(tokens) = max_tokens.filter(|tokens| *tokens < MIN_OUTPUT_TOKENS) {
        return Err(format!(
            "max_tokens 过小: {tokens}（输出上限下限 {MIN_OUTPUT_TOKENS}）"
        ));
    }
    let mut record = AgentProviderRecord::new(
        name,
        protocol,
        base_url,
        api_key,
        models,
        context_length,
        max_tokens,
    )
    .with_web_search(web_search);
    record.id = id.unwrap_or(0);
    if record.id != 0 && record.api_key.is_empty() {
        // 留空 key 回填原值：读存量记录（miss 由 upsert 的行存在校验统一报错）
        record.api_key = stores
            .global()
            .find_agent_provider(record.id)
            .map_err(|e| e.to_string())?
            .map(|stored| stored.api_key)
            .unwrap_or_default();
    }
    stores
        .global()
        .upsert_agent_provider(record)
        .map_err(|e| e.to_string())
}

/// 删除 provider：被 agent 引用 reject（含引用方提示，不级联）
#[tauri::command]
#[specta::specta]
pub fn delete_agent_provider(stores: State<'_, WorkspaceStores>, id: i64) -> Result<bool, String> {
    stores
        .global()
        .remove_agent_provider(id)
        .map_err(|e| e.to_string())
}

/// agent 实例清单（主键 id 升序）。
#[tauri::command]
#[specta::specta]
pub fn list_agent_instances(
    stores: State<'_, WorkspaceStores>,
) -> Result<Vec<AgentInstanceRecord>, String> {
    stores
        .global()
        .list_agent_instances()
        .map_err(|e| e.to_string())
}

/// 保存 agent 实例（新建 / 更新合一）：sdk 缺 provider / 悬空引用校验在
/// store 单点；默认标记不由入参写（新建恒非默认、更新保留存量标记）。携
/// `engine: Cli` 拒绝（存量行可读可删不可改）——拒面经 store 校验
/// `Err(String)` 透传，命令体零校验逻辑、签名与参数类型不收窄。
#[tauri::command]
#[specta::specta]
pub fn save_agent_instance(
    stores: State<'_, WorkspaceStores>,
    id: Option<i64>,
    name: String,
    engine: AgentEngineKind,
    provider_id: Option<i64>,
) -> Result<AgentInstanceRecord, String> {
    let mut record = AgentInstanceRecord::new(name, engine, provider_id);
    record.id = id.unwrap_or(0);
    stores
        .global()
        .upsert_agent_instance(record)
        .map_err(|e| e.to_string())
}

/// 删除 agent 实例：默认 agent 删除时同事务清标记（删后全局无默认，无顺延）；
/// miss 幂等 `Ok(false)`。
#[tauri::command]
#[specta::specta]
pub fn delete_agent_instance(stores: State<'_, WorkspaceStores>, id: i64) -> Result<bool, String> {
    stores
        .global()
        .remove_agent_instance(id)
        .map_err(|e| e.to_string())
}

/// 标记默认 agent（标记即切换，旧默认自动清除）：返回更新后记录。Cli 实例
/// 拒绝（默认标记仅可落 sdk 实例）——拒面经 store 校验 `Err(String)` 透传，
/// 命令体零校验逻辑。
#[tauri::command]
#[specta::specta]
pub fn set_default_agent_instance(
    stores: State<'_, WorkspaceStores>,
    id: i64,
) -> Result<AgentInstanceRecord, String> {
    stores
        .global()
        .set_default_agent_instance(id)
        .map_err(|e| e.to_string())
}
