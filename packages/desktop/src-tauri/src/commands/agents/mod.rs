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

use tauri::State;

use store::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord, WorkspaceStores,
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

/// 保存 provider（新建 / 更新合一，id `None` 新建 / `Some` 整行更新）：参数
/// 转换段——id 存在且入参 api_key 为空 → 读存量记录回填原值（前端编辑态
/// api_key 恒空 + 遮蔽占位，「留空 = 保持原值」语义的后端承接半边，store 恒
/// 收全字段）；重名 reject（store 事务内查重）。
#[tauri::command]
#[specta::specta]
pub fn save_agent_provider(
    stores: State<'_, WorkspaceStores>,
    id: Option<i64>,
    name: String,
    base_url: String,
    api_key: String,
    models: AgentModelTiers,
) -> Result<AgentProviderRecord, String> {
    let mut record = AgentProviderRecord::new(name, base_url, api_key, models);
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

/// 删除 provider：被 agent 引用 reject（含引用方提示，不级联）；miss 幂等
/// `Ok(false)`。
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
/// store 单点；默认标记不由入参写（新建恒非默认、更新保留存量标记）。
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

/// 标记默认 agent（标记即切换，旧默认自动清除）：返回更新后记录。
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
