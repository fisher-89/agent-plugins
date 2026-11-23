use std::path::Path;
use std::sync::Arc;

use agent::{
    AgentRunner, RunningTurn, SessionCtx, SessionKernel, SessionProvenance, SessionQuery,
    SessionRef, SessionStreamRegistry, StopRegistry, TurnRequest,
};
use store::{AgentEngineKind, AgentProviderProtocol, Store, WorkspaceStores, CLI_TENANT_RETIRED};

use crate::sdk::config::Protocol;
use crate::sdk::mount;
use crate::store_port::{StoreQuery, StoreSink};
use crate::{EngineConfig, EngineFacade, EngineKind};

/// explore 来源受控字符串（与 store `EXPLORE_RUN_SOURCE` / 前端 `EXPLORE_SOURCE`
/// 三处同字面量，改动需同步）——`begin` 据此装配 explore 会话注入面。
const EXPLORE_SOURCE: &str = "explore";

/// change 来源受控字符串（与 orchestration `SOURCE_CHANGE` 同字面量）——archi
/// 工具面注入矩阵的适用来源。
const CHANGE_SOURCE: &str = "change";

/// archi 来源受控字符串（架构页通用会话）——与命令层 `ARCHI_CHAT_SOURCE`、前端
/// `use-archi-chat` 的 `ARCHI_CHAT_SOURCE` 三处同字面量，改动需同步。
const ARCHI_SOURCE: &str = "archi";

/// config 来源受控字符串（配置页常驻会话）——与命令层 `CONFIG_CHAT_SOURCE`、
/// 前端 `use-config-chat` 的 `CONFIG_CHAT_SOURCE` 三处同字面量，改动需同步。
const CONFIG_SOURCE: &str = "config";

/// 会话注入面装配（生产路径：explore 臂 + archi 臂 + config 臂 + change 臂），四臂全产
/// **工具名字清单**，`finish` 位独立按 `role` 置位（与矩阵解耦——决策会话恒
/// 不命中矩阵，收口档位不得随矩阵 `None` 一并丢）：
/// - `explore` 来源 = 六工具名（write 移除）+ `create_explore_note` + 两只只读
///   DB 工具（单源见 desktop-agent-db-tools）；
/// - `archi` 来源 = 七工具名 + 五 archi 名 + 架构会话知识 preamble；该臂以
///   `source` 短路命中，零 `source_ref` 读取（不碰相位解析，`source_ref` 取任意
///   值下发面不变）；
/// - `config` 来源 = 六工具名（只读探测面 + 唯一提案出口，**无** `write` / `edit`、
///   无 db 工具）+ 配置协议知识 preamble + `ask = Generic` + `finish = None`；该臂以
///   `source` 判定，不进 `source_ref` 相位解析；
/// - `change` 来源按 `source_ref` 尾三段解析 `(phase, role)` 查 archi 工具面
///   矩阵，命中即把勾选名并入七工具名清单，**矩阵之后恒 push `db_tables` /
///   `db_query` 两名**（不分 phase / 不分矩阵命中，与 `ask` 位恒 `Flow` 同构）；
/// - 其余来源（**非 change、非 config** 的 debug 等）显式构造 `tools = None`（内建
///   七件）+ 零 archi 工具 + `ask = Some(Generic)`——MUST NOT 以默认构造兜底（默认值
///   的 `ask = None` 即静默禁用，通用会话会失去 ask）。
///
/// `protect` 位由单点 [`model_write_lock`] 逐臂置位（模型写权的相位锁；判定矩阵归
/// `desktop-write-protection`）：archi 臂与两处写权相位 `None`（不挂锁），其余一切
/// `Some(一条)`。
///
/// `before_done` 位（收口前钩子档位）只由 archi 臂置位 `Some(ArchiPreclose)`——
/// 自检装配的判据是**来源**，与 `preamble`（知识，尽力增强项）及工具名清单解耦；
/// 其余臂一律 `None`。
///
/// 四臂的每个名字 SHALL 经 [`mount::lookup`] 成员校验（未知名 `Err`）。`ask` 位
/// 逐臂显式置位：explore / archi / config / 其余来源 → `Generic`，change（**不分
/// role / 不分矩阵命中**）→ `Flow`。
///
/// `preamble` 位在此按 provenance 产**知识段**；最终系统提示词由
/// [`ComposedTurn::begin`] 冻结合成后覆盖同一位（快照语义见
/// `desktop-agent-execution`「会话系统提示词快照的拍存与续轮复用」）。
fn assemble_injections(provenance: &SessionProvenance) -> Result<agent::SessionInjections, String> {
    if provenance.source == EXPLORE_SOURCE {
        const EXPLORE_TOOLS: [&str; 9] = [
            "read",
            "grep",
            "glob",
            "ls",
            "edit",
            "bash",
            mount::EXPLORE_NOTE_TOOL,
            mount::DB_TABLES_TOOL,
            mount::DB_QUERY_TOOL,
        ];
        return injected(
            &EXPLORE_TOOLS,
            None,
            None,
            Some(agent::AskMode::Generic),
            None,
            provenance,
        );
    }
    if provenance.source == ARCHI_SOURCE {
        let mut names: Vec<&'static str> = mount::builtin().to_vec();
        names.push(mount::ARCHI_MODULE_TOOL);
        names.push(mount::ARCHI_SEARCH_TOOL);
        names.push(mount::ARCHI_API_TOOL);
        names.push(mount::ARCHI_DRIFT_TOOL);
        names.push(mount::ARCHI_SCAFFOLD_TOOL);
        // 知识为尽力增强项：拼装失败（`None`）只丢 preamble 位，工具面与收口前
        // 自检装配（`before_done` 档位）不受影响
        return injected(
            &names,
            crate::archi_knowledge::knowledge(),
            None,
            Some(agent::AskMode::Generic),
            Some(agent::BeforeDoneSpec::ArchiPreclose),
            provenance,
        );
    }
    if provenance.source == CONFIG_SOURCE {
        // 只读探测面 + 唯一提案出口（无 write / edit、无 db 工具）；知识为尽力增强项，
        // 拼装失败只丢 preamble 位
        const CONFIG_TOOLS: [&str; 6] = [
            "read",
            "grep",
            "glob",
            "ls",
            "bash",
            mount::PROPOSE_CONFIG_TOOL,
        ];
        return injected(
            &CONFIG_TOOLS,
            crate::config_knowledge::knowledge(),
            None,
            Some(agent::AskMode::Generic),
            None,
            provenance,
        );
    }
    if provenance.source != CHANGE_SOURCE {
        // 显式构造（MUST NOT default()：其 ask = None 即静默禁用）——非 change、
        // 非 config 来源（debug 等）的兜底
        return Ok(agent::SessionInjections {
            preamble: None,
            tools: None,
            finish: None,
            ask: Some(agent::AskMode::Generic),
            protect: model_write_lock(provenance),
            before_done: None,
        });
    }
    let mut names: Vec<&'static str> = mount::builtin().to_vec();
    let phase_role = change_phase_role(provenance.source_ref.as_deref().unwrap_or(""));
    if let Some((phase, role)) = phase_role {
        if let Some(selected) = archi_matrix(phase, role) {
            names.extend(selected);
        }
    }
    // DB 工具两名恒在：不分 phase / 不分矩阵命中，与 ask 位恒 Flow 同构
    names.push(mount::DB_TABLES_TOOL);
    names.push(mount::DB_QUERY_TOOL);
    let finish = match phase_role.map(|(_, role)| role) {
        Some("evaluator") => Some(agent::FinishSpec::EvaluatorChecklist),
        Some("decision") => Some(agent::FinishSpec::Decision),
        _ => None,
    };
    // change 来源的 ask 档位恒 Flow（不分 role / 不分矩阵命中）
    injected(
        &names,
        None,
        finish,
        Some(agent::AskMode::Flow),
        None,
        provenance,
    )
}

/// 装配产物单点（成员校验 + 名字清单落地）：未知名显式 `Err` 携名与来源，
/// 不静默滤过（滤过会让模型「什么都不能用」，装配面报错才可归因）。
fn injected(
    names: &[&'static str],
    preamble: Option<String>,
    finish: Option<agent::FinishSpec>,
    ask: Option<agent::AskMode>,
    before_done: Option<agent::BeforeDoneSpec>,
    provenance: &SessionProvenance,
) -> Result<agent::SessionInjections, String> {
    let protect = model_write_lock(provenance);
    for name in names {
        if mount::lookup(name).is_none() {
            return Err(format!(
                "注入面非法工具名: {name}（挂载清单外，来源 {}）",
                provenance.source
            ));
        }
    }
    Ok(agent::SessionInjections {
        preamble,
        tools: Some(names.iter().map(|name| (*name).to_owned()).collect()),
        finish,
        ask,
        protect,
        before_done,
    })
}

/// `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}` → `(phase, role)`：按
/// `/` 切分、段数 ≥ 4 且末段可解析为 `u32`（attempt）才认，取尾三段自尾向前。
/// 畸形 / 短段 / 末段非数字一律 `None`（fail-closed 判据单点）。
fn change_phase_role(source_ref: &str) -> Option<(&str, &str)> {
    let segments: Vec<&str> = source_ref.split('/').collect();
    if segments.len() < 4 {
        return None;
    }
    segments.last()?.parse::<u32>().ok()?;
    let role = segments.get(segments.len() - 2)?;
    let phase = segments.get(segments.len() - 3)?;
    if phase.is_empty() || role.is_empty() {
        return None;
    }
    Some((phase, role))
}

/// archi 工具面注入矩阵单源（8 行含兜底行，`(phase, role)` → 下发工具名清单；
/// 未命中即零 archi 工具）。矩阵只决定**下发面**，不决定 prompt 文本。
///
/// `drift` 只给两处**直接产出漂移**的相位（写完模型 / 改完代码）；`scaffold` 只给
/// 两处写权相位与 archi 臂。`code-review` / `acceptance` 仍只读模型面（保守案：时机
/// 差是理由，不是给 evaluator 发读数工具的充分理由）。
fn archi_matrix(phase: &str, role: &str) -> Option<Vec<&'static str>> {
    match (phase, role) {
        ("proposal", "executor") => Some(vec![
            mount::ARCHI_SEARCH_TOOL,
            mount::ARCHI_MODULE_TOOL,
            mount::ARCHI_SCAFFOLD_TOOL,
        ]),
        ("dev-design", "executor") => Some(vec![
            mount::ARCHI_MODULE_TOOL,
            mount::ARCHI_SEARCH_TOOL,
            mount::ARCHI_DRIFT_TOOL,
            mount::ARCHI_SCAFFOLD_TOOL,
        ]),
        ("dev-design", "evaluator") => {
            Some(vec![mount::ARCHI_MODULE_TOOL, mount::ARCHI_SEARCH_TOOL])
        }
        ("test-design", "executor") => Some(vec![mount::ARCHI_SEARCH_TOOL]),
        ("implement", "executor") => Some(vec![
            mount::ARCHI_MODULE_TOOL,
            mount::ARCHI_API_TOOL,
            mount::ARCHI_DRIFT_TOOL,
        ]),
        ("code-review", "evaluator") => Some(vec![mount::ARCHI_MODULE_TOOL]),
        ("acceptance", "evaluator") => Some(vec![mount::ARCHI_MODULE_TOOL]),
        _ => None,
    }
}

/// 模型写权的装配单点（相位锁）：`None` = 不挂锁（允许写模型），`Some(一条)` = 挂
/// 会话级禁区规则。
///
/// 政策：可写 ⟺ 架构页会话（`source = "archi"`） ∨ (`source = "change"` ∧
/// `(phase, role) = (proposal|dev-design, executor)`)。其余一切锁定——含
/// `dev-design/evaluator` 与 `explore` / `debug`。fail-closed 方向：畸形
/// `source_ref` / 未知 phase / 非 change 来源一律挂锁。
///
/// 锁面只 `graph/model/**`：契约资产 md 不锁（`implement/executor` 持有
/// `archi_api`，改代码相位必须能同步契约）、索引生成物（「MUST NOT 手改」已足够）、
/// 架构域看护配置（不是模型）不锁。
fn model_write_lock(provenance: &SessionProvenance) -> Option<Vec<agent::SessionProtectRule>> {
    if provenance.source == ARCHI_SOURCE {
        return None;
    }
    if provenance.source == CHANGE_SOURCE {
        let phase_role = change_phase_role(provenance.source_ref.as_deref().unwrap_or(""));
        if let Some((phase, "executor")) = phase_role {
            if phase == "proposal" || phase == "dev-design" {
                return None;
            }
        }
    }
    Some(vec![agent::SessionProtectRule {
        glob: format!("**/{}/**", foundation::layout::archi_graph_model_rel_dir()),
        reason: crate::sdk::protect::session_lock_template().to_owned(),
    }])
}

/// 解析产物（组合根内聚）：引擎 kind + 组装好的连接配置 + 快照定型双面 +
/// 实例外键。引擎具体类型不出本结构（门面 `runner_for` 签名与编排层零改动
/// 承诺的参数传递面）。
struct ResolvedEngine {
    /// 引擎二值（门面 `EngineKind`，自 store 本地 `AgentEngineKind` 映射；
    /// `Cli` 实例在解析单点即拒，恒 `Sdk`）
    kind: EngineKind,
    /// 连接配置（sdk 臂由引用 provider 组装；含 `protocol`——协议决定 wire
    /// 语义，属连接必需项）
    config: EngineConfig,
    /// sdk 引擎上下文窗长（可空调参旁路，不经 `EngineConfig`；`None` = 未配置
    /// 走 128K 缺省，该档模型名带 `[1m]` 后缀时被覆盖）
    context_window: Option<u64>,
    /// sdk 引擎输出上限（可空调参旁路，与 `context_window` 并列；`None` = 按
    /// 协议缺省，仅 anthropic 协议消费）
    max_tokens: Option<u64>,
    /// Anthropic 云端 `web_search` 服务端工具开关（**已按协议门控**的最终值：
    /// `provider.web_search && protocol == Anthropic`——门控判定单点在本组合
    /// 根，门面与 runner 零二次判定）
    web_search: bool,
    /// 两档思考强度（provider 原值对**原样**带出，**零门控**——两协议都有
    /// effort 语义；wire 形状差异归引擎侧 shaping 单点，MUST NOT 并入上面的
    /// `web_search` 门控判定）
    efforts: store::AgentEffortTiers,
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
/// cwd 半边经既有 `SessionCtx` 通道不经 compose）；`workspace_root` 为命令层
/// 在手的主仓 root（写入保护规则的装配根，与 `store` 半边同为「root 身份锚」），
/// 会话执行根 `exec_root` / worktree **不经此通道**。
///
/// `ask_registry` 为壳层托管的 ask 应答注册表（透传内核 + 门面，两侧同一张表）；
/// `steer_registry` 为壳层托管的 steer 注入注册表（两张会话级注册表成组注入
/// 内核；引擎门面不参与——登记发生在内核）；`ask_port` 仅门面（编排侧状态面的
/// 实现方是壳层 sink，内核侧不需要它）。
#[allow(clippy::too_many_arguments)]
pub fn compose_turn(
    stores: &WorkspaceStores,
    registry: Arc<StopRegistry>,
    streams: Arc<SessionStreamRegistry>,
    ask_registry: Arc<agent::AskRegistry>,
    steer_registry: Arc<agent::SteerRegistry>,
    store: Arc<Store>,
    agent: Option<i64>,
    workspace_root: &Path,
    ask_port: Option<Arc<dyn orchestration::port::AskPort>>,
) -> Result<ComposedTurn, String> {
    // 解析语义自命令层原样平移（缺省 / 显式 / sdk provider 组装 / high 档 /
    // 无默认 Err 引导管理页）
    let resolved = resolve_agent_engine(stores, agent)?;
    let query = Arc::new(StoreQuery::new(Arc::clone(&store)));
    // explore 专用工具执行上下文的句柄半边（组合根铸造、`tools::execute` 纯
    // 函数不动——句柄经门面 builder 下行至 pump per-turn 铸造）
    let explore_note_store = Arc::clone(&store);
    // DB 两工具的句柄半边：workspace 库 = 会话 workspace root 的既有实例（沿用
    // 上面 `store` 的 Arc），全局库经共享出口取得（MUST NOT 二次 open——redb
    // 单进程单句柄）
    let db_workspace = Arc::clone(&store);
    let db_global = stores.global_shared();
    let kernel = SessionKernel::new(
        Arc::new(StoreSink::new(store)),
        registry,
        streams,
        Arc::clone(&ask_registry),
        steer_registry,
    );
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
            .with_max_tokens(resolved.max_tokens)
            .with_web_search(resolved.web_search)
            .with_efforts(resolved.efforts)
            .with_ask_registry(ask_registry)
            .with_ask_port(ask_port)
            .with_explore_note_store(explore_note_store)
            .with_db_stores(db_global, db_workspace)
            .with_workspace_root(workspace_root.to_path_buf())
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
    /// 发起一轮（Continue 校验 + prior_handle 提取 + 会话系统提示词快照的拍存 /
    /// 读回 / 补拍 + 内核 begin）
    ///
    /// 快照三分支：①**首轮**（`New`，或 `Continue` 且未开轮）现算正文、本轮直接
    /// 用内存体、`begin_turn` 返回后落库；②**续轮已开轮**读库正文覆盖注入位（不
    /// 读文件、不算知识）；③**存量补拍**（已开轮但快照行缺席）按首轮口径现算一
    /// 次并落库——补拍取**补拍当轮**的知识（无历史见证，唯一可实现的口径），此
    /// 后冻结。落库 MUST 在 `begin_turn` 之后（`New` 路径只有此刻才知会话 id），
    /// 本轮 MUST NOT 读回自己刚写的快照；落库失败仅日志留痕、MUST NOT 中断本轮。
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
        // 知识段先由装配面产出；最终系统提示词由下方三分支冻结合成后覆盖同一位
        let mut injections = assemble_injections(&provenance)?;
        let (prior_handle, stamp) = match &session {
            SessionRef::New => {
                let body = freeze_preamble(&injections, &ctx);
                injections.preamble = injectable(body.clone());
                (None, Some(body))
            }
            SessionRef::Continue { id } => {
                let record = self
                    .query
                    .store()
                    .find_session(id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("会话不存在: {id}"))?;
                // 未开轮（无句柄 + 无轮行 + 无转录）：预建空会话首轮，按全新
                // 会话开跑 + 快照定型；其余续会话治理三连。首轮判据只取一次
                // ——`begin_turn` 落轮行后同判据必然转假
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
                    let body = freeze_preamble(&injections, &ctx);
                    injections.preamble = injectable(body.clone());
                    (None, Some(body))
                } else {
                    if record.config_snapshot.engine != self.store_kind {
                        return Err(format!("会话非当前引擎产出（跨引擎续会话不支持）: {id}"));
                    }
                    let prior_handle = record.engine_session_id.clone();
                    // 读点：快照正文覆盖注入位（空正文 → None）；不读文件、不算知识
                    match self
                        .query
                        .store()
                        .find_session_preamble(id)
                        .map_err(|e| e.to_string())?
                    {
                        Some(preamble) => {
                            injections.preamble = injectable(preamble.content);
                            (prior_handle, None)
                        }
                        None => {
                            // 存量补拍：本变更前创建的会话首次续轮现算一次并冻结
                            let body = freeze_preamble(&injections, &ctx);
                            injections.preamble = injectable(body.clone());
                            (prior_handle, Some(body))
                        }
                    }
                }
            }
        };
        let config_snapshot =
            serde_json::to_value(snapshot).map_err(|e| format!("配置快照组装失败: {e}"))?;
        let running = self
            .kernel
            .begin_turn(
                Arc::clone(&self.runner),
                TurnRequest {
                    session,
                    question,
                    injections,
                    ctx,
                    provenance,
                    config_snapshot,
                    prior_handle,
                },
            )
            .map_err(|e| e.to_string())?;
        if let Some(body) = stamp {
            if let Err(error) = self
                .query
                .store()
                .stamp_session_preamble(&running.session_id, &body)
            {
                crate::sdk::log::append_engine_log(&format!(
                    "会话系统提示词快照落库失败 session={} cause={error}",
                    running.session_id
                ));
            }
        }
        Ok(running)
    }
}

/// 会话系统提示词快照现算（首轮与存量补拍共用）：读取根 = 会话执行根
/// `SessionCtx.workspace_root`（worktree 即分支版），每次会话读取一次；知识段
/// 取自注入面已产出的 `preamble` 位（provenance 判定单源在
/// [`assemble_injections`]）。
fn freeze_preamble(injections: &agent::SessionInjections, ctx: &agent::SessionCtx) -> String {
    let agent_md = crate::sdk::preamble::load(&ctx.workspace_root);
    crate::sdk::preamble::freeze(injections.preamble.as_deref(), agent_md.as_deref())
}

/// 空快照正文 → `None`：无正文可注入即零 System 消息（指引常量不单独成条）。
fn injectable(body: String) -> Option<String> {
    (!body.is_empty()).then_some(body)
}

/// store 协议枚举 → 门面协议枚举（两臂 match；沿 `AgentEngineKind` →
/// `EngineKind` 双枚举映射先例——store 禁拖引擎门面 crate，故两侧各一枚、
/// 映射收组合根单点）。
fn map_protocol(protocol: AgentProviderProtocol) -> Protocol {
    match protocol {
        AgentProviderProtocol::Openai => Protocol::Openai,
        AgentProviderProtocol::Anthropic => Protocol::Anthropic,
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
            let protocol = map_protocol(provider.protocol);
            Ok(ResolvedEngine {
                kind: EngineKind::Sdk,
                config: EngineConfig {
                    // 协议经双枚举映射进连接配置（选臂只在引擎组装点，解析层
                    // 不对协议做二次判断）
                    protocol,
                    api_key: provider.api_key,
                    base_url: provider.base_url,
                    model_high: provider.models.high,
                    model_low: provider.models.low,
                },
                // 可空调参旁路：provider 可空列原样携带（None = 缺省在 runner
                // 侧解析，不落缺省字面）
                context_window: provider.context_length,
                max_tokens: provider.max_tokens,
                // 原样带出、零门控：两协议都有 effort 语义（wire 形状差异由
                // 引擎侧 shaping 吸收），MUST NOT 与下面的协议判定合并
                efforts: provider.efforts,
                // 门控单点：web_search 是 anthropic 专有 server tool，openai
                // 协议恒不生效（不静默透传、不报错）；按值移动前先算
                web_search: provider.web_search && matches!(protocol, Protocol::Anthropic),
                agent_instance_id: instance.id,
            })
        }
        AgentEngineKind::Cli => Err(CLI_TENANT_RETIRED.to_owned()),
    }
}

/// 测试夹具缝：以注入 runner 装配组合轮（`worker_test` 的收口载荷用例以脚本化
/// runner 驱动 `KernelWorkerPort`——真实 [`compose_turn`] 恒装 `SdkRunner`，
/// 无法预录 `SystemNotice{finish_step}` 进转录）。
#[cfg(test)]
pub(crate) fn composed_with_runner(
    store: Arc<Store>,
    runner: Arc<dyn AgentRunner>,
) -> ComposedTurn {
    ComposedTurn {
        kernel: SessionKernel::new(
            Arc::new(StoreSink::new(Arc::clone(&store))),
            Arc::new(StopRegistry::new()),
            Arc::new(SessionStreamRegistry::new()),
            Arc::new(agent::AskRegistry::default()),
            Arc::new(agent::SteerRegistry::default()),
        ),
        runner,
        query: Arc::new(StoreQuery::new(store)),
        store_kind: AgentEngineKind::Sdk,
        models: Some(("m-high".to_owned(), "m-low".to_owned())),
        agent_instance_id: 0,
    }
}

#[cfg(test)]
#[path = "compose_test.rs"]
mod compose_test;
