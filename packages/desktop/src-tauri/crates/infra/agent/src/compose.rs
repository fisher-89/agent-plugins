use std::sync::Arc;

use agent::{
    AgentRunner, RunningTurn, SessionCtx, SessionKernel, SessionProvenance, SessionQuery,
    SessionRef, SessionStreamRegistry, StopRegistry, TurnRequest,
};
use store::{AgentEngineKind, Store, WorkspaceStores, CLI_TENANT_RETIRED};

use crate::store_port::{StoreQuery, StoreSink};
use crate::{EngineConfig, EngineFacade, EngineKind};

/// explore 来源受控字符串（与 store `EXPLORE_RUN_SOURCE` / 前端 `EXPLORE_SOURCE`
/// 三处同字面量，改动需同步）——`begin` 据此装配 explore 会话注入面。
const EXPLORE_SOURCE: &str = "explore";

/// explore 会话注入面装配（生产路径首次非 default）：六工具收窄子集（write
/// 移除，序同 `TOOL_NAMES`）+ `Explore` 枚举；六名逐个校验 ∈ 七通用工具封闭集
/// （非法名显式 `Err`——装配层校验）。其余来源（debug / change 等）恒
/// `SessionInjections::default()`（七工具面，零变化）。
fn assemble_injections(source: &str) -> Result<agent::SessionInjections, String> {
    if source != EXPLORE_SOURCE {
        return Ok(agent::SessionInjections::default());
    }
    const EXPLORE_TOOLS: [&str; 6] = ["read", "grep", "glob", "ls", "edit", "bash"];
    for name in EXPLORE_TOOLS {
        if !crate::sdk::tools::TOOL_NAMES.contains(&name) {
            return Err(format!("注入面非法工具名: {name}（七通用工具封闭集外）"));
        }
    }
    Ok(agent::SessionInjections {
        preamble: None,
        tools: Some(
            EXPLORE_TOOLS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        ),
        toolset: Some(agent::SessionToolset::Explore),
    })
}

/// 解析产物（组合根内聚）：引擎 kind + 组装好的连接配置 + 快照定型双面 +
/// 实例外键。引擎具体类型不出本结构（门面 `runner_for` 签名与编排层零改动
/// 承诺的参数传递面）。
struct ResolvedEngine {
    /// 引擎二值（门面 `EngineKind`，自 store 本地 `AgentEngineKind` 映射；
    /// `Cli` 实例在解析单点即拒，恒 `Sdk`）
    kind: EngineKind,
    /// 连接配置（sdk 臂由引用 provider 组装）
    config: EngineConfig,
    /// sdk 引擎上下文窗长（provider `context_length` 列旁路，不经
    /// `EngineConfig`；`None` = 未配置走 128K 缺省）
    context_window: Option<u64>,
    /// 解析目标实例 id（快照 `agent_instance_id` 载荷；缺省与显式路径同经
    /// 解析单点，恒在）
    agent_instance_id: i64,
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
    /// 解析目标实例 id（快照 `agent_instance_id` 装配载荷）
    agent_instance_id: i64,
}

/// 运行发起解析 + 装配（解析单点）：`registry` 为壳层托管的停止注册表（内核
/// 治理面的挂载注入）；`streams` 为壳层托管的会话实时流注册表（沿 `registry`
/// 注入先例：壳层 `Arc` 单例经组合根下行内核，exec / change 双轨同源进入同一
/// 注册表）；`store` 为命令层预解析注入的 workspace root 库实例（design D8 拆参
/// ——store 身份锚恒 workspace root，worktree 路径在类型上不可能进 `for_root`；
/// cwd 半边经既有 `SessionCtx` 通道不经 compose）。
pub fn compose_turn(
    stores: &WorkspaceStores,
    registry: Arc<StopRegistry>,
    streams: Arc<SessionStreamRegistry>,
    store: Arc<Store>,
    agent: Option<i64>,
) -> Result<ComposedTurn, String> {
    // 解析语义自命令层原样平移（缺省 / 显式 / sdk provider 组装 / high 档 /
    // 无默认 Err 引导管理页）
    let resolved = resolve_agent_engine(stores, agent)?;
    let query = Arc::new(StoreQuery::new(Arc::clone(&store)));
    // explore 专用工具执行上下文的句柄半边（组合根铸造、`tools::execute` 纯
    // 函数不动——句柄经门面 builder 下行至 pump per-turn 铸造）
    let explore_note_store = Arc::clone(&store);
    let kernel = SessionKernel::new(Arc::new(StoreSink::new(store)), registry, streams);
    // resume 装载缝以会话全史转录闭合（引擎中立数据：session_id → 密封事件
    // 全史）
    let resume: crate::ResumeTranscript = {
        let query = Arc::clone(&query);
        Arc::new(move |session_id: &str| query.transcript(session_id).map(Some))
    };
    // 快照定型双面：引擎二值映射 + 模型双档（sdk 取 provider 双档；档位定型
    // 归 begin 按轮级 model_level 取档）。`Cli` 臂为解析拒收后的不可达死臂
    // ——映射原样保留（多租户机制不塌），不引入 panic 形态（design DD9）
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
    // runner_for 的 Err 经 `?` 传导（`Cli` 臂生产不可达，防御兜底）
    let runner: Arc<dyn AgentRunner> = Arc::from(
        EngineFacade::with_resume_transcript(resume)
            .with_context_window(resolved.context_window)
            .with_explore_note_store(explore_note_store)
            .runner_for(resolved.kind, resolved.config)?,
    );
    Ok(ComposedTurn {
        kernel,
        runner,
        query,
        store_kind,
        models,
        agent_instance_id: resolved.agent_instance_id,
    })
}

impl ComposedTurn {
    /// 发起一轮（Continue 校验 + prior_handle 提取 + 快照组装 + 内核 begin）
    pub fn begin(
        &self,
        session: SessionRef,
        question: String,
        ctx: SessionCtx,
        provenance: SessionProvenance,
    ) -> Result<RunningTurn, String> {
        // 快照由组合根组装（core 不解释）
        let model = self
            .models
            .as_ref()
            .map(|(high, low)| match ctx.model_level {
                agent::ModelLevel::High => high.clone(),
                agent::ModelLevel::Low => low.clone(),
            });
        let snapshot = store::SessionConfigSnapshot {
            engine: self.store_kind,
            agent_instance_id: Some(self.agent_instance_id),
            model,
            permission_mode: ctx.permission_mode,
        };
        let prior_handle = match &session {
            SessionRef::New => None,
            SessionRef::Continue { id } => {
                let record = self
                    .query
                    .store()
                    .find_session(id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("会话不存在: {id}"))?;
                // 未开轮（无句柄 + 无轮行 + 无转录）：预建空会话首轮，按全新
                // 会话开跑 + 快照定型；其余续会话治理三连
                if self
                    .query
                    .store()
                    .is_unstarted_session(id)
                    .map_err(|e| e.to_string())?
                {
                    self.query
                        .store()
                        .restamp_session_config(id, &snapshot)
                        .map_err(|e| e.to_string())?;
                    None
                } else {
                    if record.config_snapshot.engine != self.store_kind {
                        return Err(format!("会话非当前引擎产出（跨引擎续会话不支持）: {id}"));
                    }
                    record.engine_session_id.clone()
                }
            }
        };
        let config_snapshot =
            serde_json::to_value(snapshot).map_err(|e| format!("配置快照组装失败: {e}"))?;
        self.kernel
            .begin_turn(
                Arc::clone(&self.runner),
                TurnRequest {
                    session,
                    question,
                    injections: assemble_injections(&provenance.source)?,
                    ctx,
                    provenance,
                    config_snapshot,
                    prior_handle,
                },
            )
            .map_err(|e| e.to_string())
    }
}

/// 运行发起解析单点（语义自命令层原样平移；解析产物含快照定型双面 + 实例
/// 外键）。缺省与显式路径同臂：`engine: Cli` 实例一律显式 `Err`
/// （[`CLI_TENANT_RETIRED`] 逐字文案，不构造引擎、不静默回退）。
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
                agent_instance_id: instance.id,
            })
        }
        AgentEngineKind::Cli => Err(CLI_TENANT_RETIRED.to_owned()),
    }
}

#[cfg(test)]
#[path = "compose_test.rs"]
mod compose_test;
