use std::sync::Arc;

use agent::{
    AgentRunner, RunningTurn, SessionCtx, SessionKernel, SessionProvenance, SessionQuery,
    SessionRef, StopRegistry, TurnRequest,
};
use store::{AgentEngineKind, Store, WorkspaceStores};

use crate::store_port::{StoreQuery, StoreSink};
use crate::{EngineConfig, EngineFacade, EngineKind};

/// 解析产物（组合根内聚）：引擎 kind + 组装好的连接配置 + 快照定型双面。
/// 引擎具体类型不出本结构（门面 `runner_for` 签名与编排层零改动承诺的参数
/// 传递面）。
struct ResolvedEngine {
    /// 引擎二值（门面 `EngineKind`，自 store 本地 `AgentEngineKind` 映射）
    kind: EngineKind,
    /// 连接配置（sdk 臂由引用 provider 组装；cli 臂 `EngineConfig::empty()`
    /// 占位，CLI 引擎不消费）
    config: EngineConfig,
    /// sdk 引擎上下文窗长（provider `context_length` 列旁路，不经
    /// `EngineConfig`；`None` = 未配置走 128K 缺省；cli 臂恒 None）
    context_window: Option<u64>,
}

/// 组合根装配产物：内核 + 解析好的 runner + store 查询面（Continue 校验）+
/// 快照定型双面。命令层经 [`ComposedTurn::begin`] 发起一轮。
pub struct ComposedTurn {
    /// 会话内核（治理面 + 运行面驱动）
    kernel: SessionKernel,
    /// 解析好的引擎（门面构造唯一 match 点的产物）
    runner: Arc<dyn AgentRunner>,
    /// store 查询面（Continue 校验 + resume 装载缝底座）
    query: Arc<StoreQuery>,
    /// store 本地引擎二值（快照定型与归属比对面）
    store_kind: AgentEngineKind,
    /// 快照模型双档（provider high / low 档）
    models: Option<(String, String)>,
}

/// 运行发起解析 + 装配（解析单点）：`registry` 为壳层托管的停止注册表（内核
/// 治理面的挂载注入）；`store` 为命令层预解析注入的 workspace root 库实例
///（design D8 拆参——store 身份锚恒 workspace root，worktree 路径在类型上
/// 不可能进 `for_root`；cwd 半边经既有 `SessionCtx` 通道不经 compose）。
pub fn compose_turn(
    stores: &WorkspaceStores,
    registry: Arc<StopRegistry>,
    store: Arc<Store>,
    agent: Option<i64>,
) -> Result<ComposedTurn, String> {
    // 解析语义自命令层原样平移（缺省 / 显式 / sdk provider 组装 / high 档 /
    // 无默认 Err 引导管理页）
    let resolved = resolve_agent_engine(stores, agent)?;
    let query = Arc::new(StoreQuery::new(Arc::clone(&store)));
    let kernel = SessionKernel::new(Arc::new(StoreSink::new(store)), registry);
    // resume 装载缝以会话全史转录闭合（引擎中立数据：session_id → 密封事件
    // 全史；CLI 引擎持有不消费，CLI 续会话仍走引擎侧先行句柄）
    let resume: crate::ResumeTranscript = {
        let query = Arc::clone(&query);
        Arc::new(move |session_id: &str| query.transcript(session_id).map(Some))
    };
    // 快照定型双面：引擎二值映射 + 模型双档（sdk 取 provider 双档、cli 无模型
    // 装配概念；档位定型归 begin 按轮级 model_level 取档）
    let (store_kind, models) = match resolved.kind {
        EngineKind::Cli => (AgentEngineKind::Cli, None),
        EngineKind::Sdk => (
            AgentEngineKind::Sdk,
            Some((
                resolved.config.model_high.clone(),
                resolved.config.model_low.clone(),
            )),
        ),
    };
    let runner: Arc<dyn AgentRunner> = Arc::from(
        EngineFacade::with_resume_transcript(resume)
            .with_context_window(resolved.context_window)
            .runner_for(resolved.kind, resolved.config),
    );
    Ok(ComposedTurn {
        kernel,
        runner,
        query,
        store_kind,
        models,
    })
}

impl ComposedTurn {
    /// 发起一轮（Continue 校验 + prior_handle 提取 + 快照组装 + 内核 begin）：
    /// 会话不存在 / 非当前引擎产出（快照 engine ≠ 解析 kind）/ 引擎句柄缺失
    /// → `Err` 显式失败（不落库不推流）。
    pub fn begin(
        &self,
        session: SessionRef,
        question: String,
        ctx: SessionCtx,
        provenance: SessionProvenance,
    ) -> Result<RunningTurn, String> {
        let prior_handle = match &session {
            SessionRef::New => None,
            SessionRef::Continue { id } => {
                let record = self
                    .query
                    .store()
                    .find_session(id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("会话不存在: {id}"))?;
                if record.config_snapshot.engine != self.store_kind {
                    return Err(format!("会话非当前引擎产出（跨引擎续会话不支持）: {id}"));
                }
                Some(
                    record
                        .engine_session_id
                        .clone()
                        .ok_or_else(|| format!("会话缺少引擎句柄，无法续接: {id}"))?,
                )
            }
        };
        // 快照由组合根组装（core 不解释）
        let model = self
            .models
            .as_ref()
            .map(|(high, low)| match ctx.model_level {
                agent::ModelLevel::High => high.clone(),
                agent::ModelLevel::Low => low.clone(),
            });
        let config_snapshot = serde_json::to_value(store::SessionConfigSnapshot {
            engine: self.store_kind,
            model,
            permission_mode: ctx.permission_mode,
        })
        .map_err(|e| format!("配置快照组装失败: {e}"))?;
        self.kernel
            .begin_turn(
                Arc::clone(&self.runner),
                TurnRequest {
                    session,
                    question,
                    injections: agent::SessionInjections::default(),
                    ctx,
                    provenance,
                    config_snapshot,
                    prior_handle,
                },
            )
            .map_err(|e| e.to_string())
    }

    /// 引擎二值观测（crate 内）：CLI 臂由 worker 轮前预检 CLI 可发现性
    ///（CliMissing 收敛 `Err` + 零半成品记录）；SDK 臂不预检（ConfigMissing
    /// 归 [`Self::begin`] 的 open 段显式 `Err`）。
    pub(crate) fn is_cli_engine(&self) -> bool {
        self.store_kind == AgentEngineKind::Cli
    }
}

/// 运行发起解析单点（语义自命令层原样平移；解析产物含快照定型双面）。
fn resolve_agent_engine(
    stores: &WorkspaceStores,
    agent: Option<i64>,
) -> Result<ResolvedEngine, String> {
    let global = stores.global();
    let instance = match agent {
        None => global
            .default_agent_instance()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                "未设置默认 agent：请前往 Agent 管理页（/agents）配置后再发起".to_owned()
            })?,
        Some(id) => global
            .find_agent_instance(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("agent 不存在: id={id}"))?,
    };
    match instance.engine {
        AgentEngineKind::Sdk => {
            let provider_id = instance.provider_id.ok_or_else(|| {
                format!(
                    "agent {:?} 未配置 provider：请前往 Agent 管理页（/agents）修正",
                    instance.name
                )
            })?;
            let provider = global
                .find_agent_provider(provider_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    format!(
                        "agent {:?} 引用的 provider 不存在: id={provider_id}",
                        instance.name
                    )
                })?;
            Ok(ResolvedEngine {
                kind: EngineKind::Sdk,
                config: EngineConfig {
                    api_key: provider.api_key,
                    base_url: provider.base_url,
                    model_high: provider.models.high,
                    model_low: provider.models.low,
                },
                // 窗长旁路：provider 可空列原样携带（None = 缺省启发式在
                // runner 侧解析，不落缺省字面）
                context_window: provider.context_length,
            })
        }
        AgentEngineKind::Cli => Ok(ResolvedEngine {
            kind: EngineKind::Cli,
            config: EngineConfig::empty(),
            context_window: None,
        }),
    }
}

#[cfg(test)]
#[path = "compose_test.rs"]
mod compose_test;
