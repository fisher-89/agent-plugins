# 测试设计: retire-cli-engine

> **日期**: 2026-10-13
> **依据**: proposal.md（AC-1–AC-11）+ design.md（架构组件、设计定值 DD1–DD12、变更清单、公共函数 / API、决策留痕 #6）

---

## 验收范围

<!-- 逐条映射 proposal.md 验收标准；`被测文件或模块` = design「变更清单」修改文件表的源文件
    （无源文件承载的机械步骤记 `—`）。跨模块组合用例挂靠链路入口模块的 per-file 章节
    （无独立集成测试层）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | cli/ 引擎删除面：`crates/infra/agent/src/cli/` 零残留；`lib.rs` 无 `cli::` / `ClaudeCliRunner` 引用；`cargo test --workspace` 全绿；`regex` / `glob` 按实际引用面保留 | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`（门面删除面回归）+ `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs`（discover 预检删除回归）；grep 面与依赖核查见不可测试项 3 / 9 |
| AC-2 | 多租户机制不塌：`AgentRunner` trait / `EngineKind` 双值 / `runner_for` 唯一 match 点零塌；`crates/core/agent` 零 diff（`AgentStartError` 变体集不变）；`AgentEngineKind` 变体集不动 | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` + `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`（DD9 双臂 match）+ `packages/desktop/src-tauri/crates/infra/store/src/model.rs`（变体集）；core 零 diff 见不可测试项 6 |
| AC-3 | 变体序号锚定：`AgentEngineKind` bincode 序号断言在位（`Cli`=0 / `Sdk`=1 字节级）；含存量两变体行的库 roundtrip 照读零错位 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` -> `model_test.rs` |
| AC-4 | 解析层显式拒绝：`resolve_agent_engine` 对 `engine: Cli` 实例（缺省与显式路径同）返回 `Err`（文案含「CLI 租户已下线」与管理页引导）；存量 cli 会话续接显式失败（不落库不推流），会话列表 / 转录照常可读 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `compose_test.rs` |
| AC-5 | 写面拒 Cli：`save_agent` 新建/更新携 `engine: Cli` 均拒（存量行可读可删不可改）；`set_default_agent_instance` 对 Cli 实例拒；存量默认 cli 实例的缺省发起显式 `Err` 且引导改默认（不清 `is_default`，零数据写入） | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` + `packages/desktop/src-tauri/src/commands/agents/mod.rs` + `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`（缺省发起半边）+ `packages/desktop/src/views/agents/components/agent-panel.tsx`（入口禁用 DD4） |
| AC-6 | 引擎句柄硬门槛退役：`begin()` 不再因 `engine_session_id` 缺失拒续会话（`prior_handle` Option 透传）；`SessionRecord.engine_session_id` / `SessionRow.remoteSessionId` / `bind_session_remote` 留痕保留 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` + `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（bind_session_remote 留痕半边） |
| AC-7 | 快照实例外键：`SessionConfigSnapshot.agent_instance_id` 新会话恒 `Some(实例 id)`（缺省与显式路径同）；存量 `SessionRecord` 经 v1→v2 `from` 链读出 `None`；前端 configSnapshot mock 补 `agentInstanceId` 键 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` + `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`（快照组装半边）+ 前端 9 处 configSnapshot fixture（逐文件节：use-agent-chat / use-agent-run-history / agent-debug-view / use-session-transcript / session-transcript-panel / change-detail-view / detail-drawer / use-explore-session / agent-run-history） |
| AC-8 | 产品面收窄：agent-panel engine 选项仅 `sdk`（无 'cli' 档）；`canSave` 无 cli 分支；provider 选择器恒启用必选；存量 cli 行只读可删 | `packages/desktop/src/views/agents/components/agent-panel.tsx` -> `agent-panel.test.tsx` |
| AC-9 | 契约面一致：五份 spec delta 落盘（REMOVED 带 Reason/Migration、MODIFIED header 逐字一致、scenario 标记恰 4 个 #）；「引擎恒 CLI」边界与 SpecSync「CLI 引擎」措辞同步翻案；「Claude CLI 租户（MVP）」整段退役 | —（机械步骤，openspec 静态校验轨道；见不可测试项 7） |
| AC-10 | 测试与管线：测试文件清单全数更新；`client:check` 与 `run test` / `cargo test --workspace` 全绿；覆盖率达阈值（lines 90 / branches 85 / functions 90）、mutation score 达 80；`openspec/config.json` 无新增 excludes | 测试文件清单各 per-file 章节（fixture 面 + cli mock 处置 + bindings 守卫同步 + mutation 管线修复三节——AC-10 mutation 门在位的前置修复）；管线半边 —（机械步骤，见不可测试项 8） |
| AC-11 | 版本交付（归档时执行）：`packages/desktop/package.json` version 0.5.9 → 0.5.10；`tauri.conf.json` 维持 `../package.json` 引用、`src-tauri/Cargo.toml` 不随动、`plugins/dev-team` 保持 2.10.44 零改动 | `packages/desktop/package.json`（见不可测试项 5） |

> 组合链承载说明：AC-4 / AC-5 的「解析拒收 → 写面拒收 → 命令面透传 → 管理页入口禁用」为同一拒收事实的多层落点，各半边断言分别挂 compose / store / commands / agent-panel 章节；跨模块组合用例（`compose_turn → resolve_agent_engine → runner_for`、命令面 → store 双拒透传、`AgentPanel → use-agent-instances → invoke` 双层兜底）挂靠各链路发起方章节的 `#### 用例` 表，无独立集成测试区。

---

## 单元测试

<!-- 测试框架与布局（框架识别结论）：Rust 侧 cargo test（workspace 级注册，测试文件与源文件
     同目录同名 `*_test.rs` 模块、`#[cfg(test)] #[path]` 挂载，`assert_eq!` / `assert!` +
     `Result` 断言，中文测试名）；TS 侧 vitest（vite-plus，`vp test`），共置 `*.test.ts(x)`，
     `describe` / `it` + `expect`，`@testing-library/react` 渲染，`data-testid` 查询挂钩
    （禁样式类名查询），`vi.mock('@tauri-apps/api/core')` / `vi.fn()` 替身进程边界。
     Mock 纪律（沿仓库既有）：Rust 侧零框架级 mock——db 边界一律 tempfile 真件库真实组合，
     进程内假件仅限「作为被测 API 显式入参 / 注入依赖」的 trait 假件（入参例外）与子进程
     边界假执行体（checks crate 既有 SpawnWindow 装置）；前端仅 mock 进程边界（invoke /
     计时器）与显式入参注入的 state 动作，内部模块与 `ui/**` 真实组合。
     跨模块组合用例挂靠链路发起方 / 最上层调用方的 per-file 章节（`#### 用例` 表内，describe
     标题写链路方向），不设独立集成测试区。
     映射说明：proposal「测试文件」清单中 9 处 configSnapshot mock 补键与
     `use-agent-instances.test.ts` 的 cli mock 数据处置，其被测源文件零改动（fixture 随动级联
     触点），按既有共置布局建节承接（先例：`app.tsx -> app.test.tsx` 节承接 fixture 形状随动）；
     `agent-run-history.test.tsx` 为 grep 盘点补齐的清单外补入（同载 configSnapshot fixture）；
     mutation 管线三节（cargo_mutants / segments / mutation mod，清单外补入·测试管线修复）为
     AC-10 mutation 门在位前置修复的 per-file 承接；exec / change_flow `mod_test.rs` 两条保绿
     随动归实现相位（决策留痕 #6），见不可测试项 14。
     参数边界按系统化映射推导：int（0 / -1 / MAX_INT / nullish）、str（空串 / 超长 >1000 字符 /
     特殊字符 `\n` `\0` emoji / nullish）、bool（True / False / nullish）、list（空 / 单元素 /
     超大 / nullish）、object（空 / 缺必填键 / 多余键 / nullish）、Optional[T]（None 并入其他
     边界）、Enum（各值 + 非法枚举值）、float（0.0 / -0.0 / NaN / Inf / nullish）；不适用形态
     在行内显式标注。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

<!-- 挂 AC-1（门面删除面回归）/ AC-2（唯一 match 点）。`mod cli` / `pub use cli::…` /
     `ClaudeCliRunner` 删除为编译期事实（无运行时行为可断言）；`lib.rs` 无 `cli::` 引用的
     grep 面归守线静态审查（见不可测试项 9）。 -->

#### 待测功能

- EngineFacade::runner_for(kind: EngineKind, engine_cfg: EngineConfig) -> Result<Box<dyn AgentRunner>, String>: DD1 `Result` 包装——`EngineKind::Cli` 臂 `Err(CLI_TENANT_RETIRED.to_owned())`（退役态·协议预留、防御兜底，弃 `unreachable!`，R8）；`EngineKind::Sdk` 臂 `Ok(Box::new(SdkRunner::new(engine_cfg, self.resume.clone(), self.context_window)))`；唯一 match 点结构不塌（AC-2）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `runner_for` Sdk 臂 Ok 返回面（构造接线） | 正向 | `EngineKind::Sdk` + 正常 `EngineConfig`（api_key / base_url / model_high / model_low 均在场）→ `Ok(Box<dyn AgentRunner>)`，runner 经 `AgentRunner` trait 面可驱动；`engine_cfg` / `resume`（`with_resume_transcript` 注入）/ `context_window`（`new` 注入）三面构造携带（既有 SdkRunner 接线断言保绿） | 适配 |
| `runner_for` Sdk 臂 Ok 返回面（构造接线） | 异常 | `EngineConfig` object 边界（dict 映射）：`EngineConfig::empty()` 空字段 → Ok 不 panic（参数校验归 runner 侧，门面不抢做）；字段 str 边界（空串 / 超长 >1000 / 特殊字符 `\n` `\0` emoji）原样携带不加工；多余键不适用（Rust struct 类型面排除） | 适配 |
| `runner_for` Sdk 臂 Ok 返回面（构造接线） | 边界 | `context_window: Option<u64>` 边界（Optional + int 映射）：None / Some(0) / Some(u64::MAX) 均 Ok 透传；`resume` 缺省与显式注入两形态（装载缝闭包替身调用计数）；`EngineKind` 枚举穷举（Sdk 归本对象、Cli 归下对象；非法值不适用——Rust 枚举不可旁路构造） | 适配 |
| `runner_for` Cli 臂显式 Err 防御兜底（DD1） | 正向 | `EngineKind::Cli` 直入 `runner_for`（旁路构造防御形态）→ `Err`，错误串逐字 `CLI_TENANT_RETIRED`（四拒点同文案，DD3）；不 panic（弃 `unreachable!` 的 R8 锚） | 新增 |
| `runner_for` Cli 臂显式 Err 防御兜底（DD1） | 异常 | Err 面无 `ClaudeCliRunner` 产物（类型已删的编译锚）；返回 `Result` 后零 Ok 变体混入 Cli 臂（不可达死臂不产 runner）；Cli 臂拒收面不触装载缝（调用计数恒零） | 新增 |
| `runner_for` Cli 臂显式 Err 防御兜底（DD1） | 边界 | 既有 `lib_test.rs` Cli 臂用例群（`ClaudeCliRunner` import / `runner_for` 返回 `Box` 形态断言 / PATH_LOCK CLI 真实驱动）翻转为 Err 断言或随删除面废弃；`EngineKind` 双值 match 穷举编译期保证唯一分发点不塌（AC-2）；crate 根导出面锚定（re-export 收窄后组合根 / 查询面 / 门面可达性回归） | 重写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ResumeTranscript` 装载缝（注入依赖） | 闭包替身 + 调用计数（入参例外，沿既有 `counting_loader` 装置） | resume 透传与调用面用例 |
| `SdkRunner` / `AgentRunner` | 不 mock——`runner_for` 返回真实 `SdkRunner`（内部模块真实组合），经 trait 面观察构造 | 本节全部用例 |
| PATH / 子进程（原 CLI shim 边界） | 不 mock 且不再触达（Cli 臂退役后无进程边界消费面；既有 PATH_LOCK 装置随 cli 用例废弃） | — |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

<!-- 挂 AC-4 / AC-5（缺省发起半边）/ AC-6 / AC-7（快照组装半边）。`resolve_agent_engine` 为
     模块私有解析单点（design「公共函数 / API」表显式不列）、`is_cli_engine()` 删除为 crate
     内方法退役——均不虚构 API 条目，拒收与解析行为经 `compose_turn` 公共面真实组合驱动。
     跨模块组合用例挂靠本章节（链路发起方 = `compose_turn` / `ComposedTurn::begin`）：
     `compose_turn → resolve_agent_engine → runner_for` 与 `begin → kernel.begin_turn`
     全链真实组合，describe 标题写链路方向。 -->

#### 待测功能

- compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, store: Arc<Store>, agent: Option<i64>) -> Result<ComposedTurn, String>: 签名不变；`resolve_agent_engine` 的 `AgentEngineKind::Cli` 臂 → `Err(CLI_TENANT_RETIRED.to_owned())`（缺省与显式路径同臂）经 `?` 传导；`ResolvedEngine` / `ComposedTurn` 携 `agent_instance_id: i64` 接线（DD8）；`store_kind / models` 双臂 match 保留（DD9）
- ComposedTurn::begin(&self, session: SessionRef, question: String, ctx: SessionCtx, provenance: SessionProvenance) -> Result<RunningTurn, String>: 签名不变；`prior_handle` Option 透传（硬门槛退役，DD2）；快照组装填 `agent_instance_id: Some(self.agent_instance_id)`（DD8）；跨引擎续会话校验零改动（DD10）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `compose_turn` 解析拒收与实例 id 接线（链路：`compose_turn` → `resolve_agent_engine` → `runner_for`） | 正向 | 显式 `agent: Some(sdk_id)`（sdk 实例 + provider 齐）→ `Ok(ComposedTurn)`：携 `agent_instance_id = sdk_id`（非 Option）、`store_kind` = `AgentEngineKind::Sdk`、`models` 双臂 match 保留（DD9 死臂零成本不 panic） | 适配 |
| `compose_turn` 解析拒收与实例 id 接线（链路：`compose_turn` → `resolve_agent_engine` → `runner_for`） | 异常 | 显式 `agent: Some(cli_id)`（存量 cli 实例）→ `Err` 逐字 `CLI_TENANT_RETIRED`（AC-4 显式路径）；缺省 `agent: None` 且默认实例为存量 cli → 同臂同文案 `Err`（AC-5 缺省发起半边：不清 `is_default`、零数据写入）；`runner_for` 的 `Err` 经 `?` 传导不吞 | 新增 |
| `compose_turn` 解析拒收与实例 id 接线（链路：`compose_turn` → `resolve_agent_engine` → `runner_for`） | 边界 | `agent: Option<i64>` 边界（int + Optional 映射）：None（缺省解析）/ Some(0) / Some(-1) / Some(i64::MAX) → miss 走既有「agent 不存在」Err 零改动；未设默认 → 既有「未设置默认 agent」Err 零改动；sdk 缺 provider / 悬空 provider 既有 Err 零改动 | 适配 |
| `ComposedTurn::begin` 引擎句柄硬门槛退役（DD2） | 正向 | `SessionRef::Continue` 续会话缺 `engine_session_id`（列值 `None`）→ 不再 `Err("会话缺少引擎句柄，无法续接")`，`prior_handle` 透传 `None`（AC-6 主锚） | 新增 |
| `ComposedTurn::begin` 引擎句柄硬门槛退役（DD2） | 异常 | 会话 id 不存在 → 既有 `Err("会话不存在: …")` 零改动；`question` str 边界（空串 / 超长 / 特殊字符 `\n` `\0` emoji）透传不加工（nullish 不适用——Rust `String` 类型面排除） | 适配 |
| `ComposedTurn::begin` 引擎句柄硬门槛退役（DD2） | 边界 | `engine_session_id` 为 `Some("")` 空串 / 超长 >1000 / 特殊字符 → `Some(值)` 原样透传（Option 透传不校验）；`SessionRef::New` → `prior_handle` 恒 `None`；既有「缺句柄拒续」断言删除（硬门槛退役） | 重写 |
| `ComposedTurn::begin` 快照实例外键组装（DD8 / AC-7） | 正向 | 新会话（`SessionRef::New`）快照 `config_snapshot` JSON 含 `agentInstanceId` = 实例 id（`Some` 恒在）+ `engine` / `model` / `permissionMode` 零变化；缺省（`agent: None`）与显式（`Some(id)`）路径同经 resolve 取实例、同填 `Some` | 新增 |
| `ComposedTurn::begin` 快照实例外键组装（DD8 / AC-7） | 异常 | `models: None`（无模型装配）→ 快照 `model: null` 照常、`agentInstanceId` 恒 `Some`；`SessionRef::Continue` 续会话组装快照同口径（不回填、按当次装配填 `Some`） | 适配 |
| `ComposedTurn::begin` 快照实例外键组装（DD8 / AC-7） | 边界 | 实例 id int 边界（0 / -1 / i64::MAX 构造）经 serde 数值不失真；`ctx.model_level` 枚举（High / Low）双值取模型臂零改动；快照 serde 组装失败路径标注不适用（`Value` 序列化不可失败） | 适配 |
| `ComposedTurn::begin` 跨引擎续会话校验零改动（DD10） | 正向 | 存量 cli 会话（快照 `engine: Cli`）续接（`store_kind` 恒 Sdk）→ 既有 `Err("会话非当前引擎产出（跨引擎续会话不支持）: …")` 文案逐字零改动——AC-4「存量 cli 会话续接显式失败」落点（`begin()` 前置 Err 不落库不推流，kernel 假件零调用计数锚） | 适配 |
| `ComposedTurn::begin` 跨引擎续会话校验零改动（DD10） | 异常 | 同引擎续会话（快照 `engine: Sdk`）不经该门 Err（校验不误伤）；门序保持：跨引擎校验先于 `prior_handle` 透传（硬门槛退役不挪既有拒因次序） | 适配 |
| `ComposedTurn::begin` 跨引擎续会话校验零改动（DD10） | 边界 | 续会话 id str 边界（空串 / 超长 / 特殊字符）→ `find_session` miss 走「会话不存在」分支不 panic；会话列表 / 转录读面（`session_query` 侧）零改动照常可读（R5 呈现兜底） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 / redb 库文件（进程边界） | 不 mock：tempfile 双环境真开全局库 + workspace 库（沿既有 `DualEnv` 装置），fixture 经 store 公共写面落库 | 本节全部用例 |
| kernel `begin_turn` 端口（注入依赖） | trait 假件 + 调用计数（入参例外，沿既有装置）：断言跨引擎 Err 时 kernel 零调用、Ok 时 `TurnRequest.prior_handle` / `config_snapshot` 载荷透传 | begin() 载荷断言用例 |
| `StopRegistry` | 不 mock——进程内注册表真实组合（非进程边界） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/worker.rs -> packages/desktop/src-tauri/crates/infra/agent/src/worker_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`execute()` 为 `WorkerAgentPort` 实现面、
     `is_cli_engine()` 为 pub(crate) 私有方法删除——「公共函数 / API」表显式不列）；
     不虚构条目。被测面 = 修改文件表「`is_cli_engine()` 的 discover 轮前预检分支删除、
     `ConfigMissing` 归 `begin()` open 段」的行为半边，按 proposal 测试文件清单落本测试文件
     （既有「discover 预检 / CliMissing 收敛」断言面即布局于此）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `execute()` discover 轮前预检分支删除回归（AC-1） | 正向 | sdk 轮执行全程不再触 `crate::discover::discover()`（无 PATH 轮前预检）；既有 worker 全链用例（假 sink + `DualEnv` 真库 + `KernelWorkerPort`）保绿（回归锚） | 适配 |
| `execute()` discover 轮前预检分支删除回归（AC-1） | 异常 | provider 配置缺失形态归 `begin()` open 段显式 `Err`（ConfigMissing 归位口径：预检删除后错误面不移位、不静默、零半成品记录纪律保持）；既有 `CliMissing` 收敛用例废弃（`AgentStartError::CliMissing` 变体保留为协议预留、退役后无生产方，core 零 diff） | 重写 |
| `execute()` discover 轮前预检分支删除回归（AC-1） | 边界 | 既有 cli 档用例群（PATH 环境变量隔离 + 假 CLI shim + `TEST_PATH_LOCK` 串行装置）随 CLI 租户退役废弃（含装置静态清理——无消费者即删）；轮执行一轮一命语义零改动 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| orchestration sink / `WorkerAgentPort` 端口（注入依赖） | 假件注入 + 调用观察（入参例外，沿既有假 sink 装置） | 本节全部用例 |
| 文件系统 / redb 库（进程边界） | 不 mock：tempfile 双环境真开库 | 本节全部用例 |
| 子进程 / PATH（原 CLI shim 边界） | 随预检分支删除不再触达，无 mock 需求（既有假 CLI shim 装置随用例废弃） | — |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- 挂 AC-3 / AC-5（文案面）/ AC-7（版本链半边）。`AgentEngineKind` 变体序号解码锚与
     `SessionRecord` v1→v2 `from` 链 / `SessionConfigSnapshot.agent_instance_id` 为类型级
     变更（design「类型定义」表承载、「公共函数 / API」表不列），按修改文件表行为半边落
     `#### 用例` 测试对象，不虚构 API 条目。 -->

#### 待测功能

- CLI_TENANT_RETIRED: `pub const CLI_TENANT_RETIRED: &str`——共享拒收文案（DD3：「CLI 租户已下线：请前往 Agent 管理页（/agents）新建或改用 sdk agent，并将其设为默认（存量 cli 实例可读可删不可改）」）；四拒点（`runner_for` / `resolve_agent_engine` / `upsert_agent_instance` / `set_default_agent_instance`）逐字复用、不加前后缀

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `CLI_TENANT_RETIRED` 共享拒收文案（DD3） | 正向 | 常量内容逐字锚：含「CLI 租户已下线」+ 管理页引导（`/agents`）+ 改默认引导 + 「存量 cli 实例可读可删不可改」口径；四拒点 Err 文案断言逐字复用同一常量（跨模块断言挂各拒点章节，本节锚常量内容面） | 新增 |
| `CLI_TENANT_RETIRED` 共享拒收文案（DD3） | 异常 | 非空 `&str`、无格式化占位符（`{}` / `%s`）残留——逐字复用不拼接不加前后缀（防多处措辞漂移） | 新增 |
| `CLI_TENANT_RETIRED` 共享拒收文案（DD3） | 边界 | 文案 str 边界不适用（常量字面量固定、非入参）；文案内中文 / `/agents` 路径 / 全角括号在前端 title 呈现不转义不截断（呈现面断言挂 agent-panel 章节） | 新增 |
| `AgentEngineKind` 变体序号解码锚（AC-3） | 正向 | bincode 字节级断言：`Cli` 编码恰 `0`、`Sdk` 编码恰 `1`（变体序号钉死，解码锚回归钉） | 新增 |
| `AgentEngineKind` 变体序号解码锚（AC-3） | 异常 | 含存量两变体行的库 roundtrip 照读零错位（`AgentInstanceRecord.engine` 与 `SessionConfigSnapshot.engine` 两表载荷 bincode roundtrip 逐字段一致） | 新增 |
| `AgentEngineKind` 变体序号解码锚（AC-3） | 边界 | 反向解码锚：字节 `[0]` → `Cli` / `[1]` → `Sdk`；`[2]` 及空字节串非法输入 → 反序列化 `Err` 不 panic（越界拒解，防静默错位——两表报废级红线） | 新增 |
| `SessionConfigSnapshot` 实例外键与 `SessionRecord` v1→v2 演进链（DD6 / DD7 / AC-7） | 正向 | v1 `SessionRecord` bincode 载荷（旧三字段快照）经 `native_model` 版本链读出 v2：`agent_instance_id = None`、其余字段原样（升级半边真实，零迁移代码路径）；v2 新快照 `agent_instance_id = Some(id)` roundtrip 原样 | 新增 |
| `SessionConfigSnapshot` 实例外键与 `SessionRecord` v1→v2 演进链（DD6 / DD7 / AC-7） | 异常 | 旧 JSON 形态缺 `agentInstanceId` 键（`serde_json::from_value` 旧 Value / store_port 旧载荷）→ `#[serde(default)]` 读出 `None` 不炸；降级半边 `From`（v2 → V1）丢弃新增字段、缺省占位不 panic（只保升级语义真实性） | 新增 |
| `SessionConfigSnapshot` 实例外键与 `SessionRecord` v1→v2 演进链（DD6 / DD7 / AC-7） | 边界 | `agent_instance_id: Option<i64>` 边界（None / Some(0) / Some(-1) / Some(i64::MAX)）serde 线键 `agentInstanceId`（camelCase）数值不失真；字段定序「`engine` 之后」（DD7）的 v2 bincode 布局锚——布局重排即错位的回归钉（与序号锚同为解码兼容红线） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| （无进程边界） | 纯类型面真实组合：bincode 1.3 编解码 + `native_model` 版本链 / `From` 映射 + `serde_json` 全真（不 mock） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- 挂 AC-5（写面双拒单点）/ AC-6（留痕保留）。 -->

#### 待测功能

- Store::upsert_agent_instance(agent: AgentInstanceRecord) -> Result<AgentInstanceRecord, StoreError>: 签名不变；拒 Cli 双口（DD5）——①入参 `engine == Cli`（新建与更新均拒）②`id > 0` 且存量行 `engine == Cli`（整行不可改，含「改 engine 为 sdk 的整行更新」）；Err 文案逐字 `CLI_TENANT_RETIRED`
- Store::set_default_agent_instance(id: i64) -> Result<AgentInstanceRecord, StoreError>: 签名不变；目标实例 `engine == Cli` → `Err(CLI_TENANT_RETIRED)`（默认标记仅可落 sdk 实例），拒时零数据写入（不清任何 `is_default`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `Store::upsert_agent_instance` 拒 Cli 双口（DD5 / AC-5） | 正向 | `engine: Sdk` + provider 新建（id=0）/ 更新（id>0）照常 Ok（表尾 max+1 分配、恒非默认、更新臂保留存量默认标记——既有语义零改动回归锚） | 适配 |
| `Store::upsert_agent_instance` 拒 Cli 双口（DD5 / AC-5） | 异常 | 入参 `engine: Cli` 新建 / 更新均 `Err(StoreError)`，错误串逐字 `CLI_TENANT_RETIRED`（入参口）；更新臂存量行 `engine: Cli` 时入参改 `sdk` 亦拒（存量行口：「含改 engine 为 sdk 的整行更新」）；拒后零写入（存量行照旧可读可删：find / remove 语义零改动） | 新增 |
| `Store::upsert_agent_instance` 拒 Cli 双口（DD5 / AC-5） | 边界 | `AgentInstanceRecord` object 边界（dict 映射）：name 空串 / 纯空白 / 超长 / 特殊字符（既有「agent 名不得为空白」与查重 Err 零改动）；`provider_id` None（sdk 既有必选拒）/ Some(悬空) 既有 Err；id 边界 0 / -1 / i64::MAX（miss → 既有「agent 不存在」） | 适配 |
| `Store::set_default_agent_instance` 拒 Cli 目标（DD5 / AC-5） | 正向 | sdk 实例设默认 Ok、原子清旧默认（全局恒至多一，`set_default` 切换一次操作修复 R4——既有切换语义零改动） | 适配 |
| `Store::set_default_agent_instance` 拒 Cli 目标（DD5 / AC-5） | 异常 | 目标实例 `engine: Cli` → `Err` 逐字 `CLI_TENANT_RETIRED`，且零数据写入（不清任何 `is_default`——存量默认 cli 不被顺手清，AC-5「不清 is_default」主锚） | 新增 |
| `Store::set_default_agent_instance` 拒 Cli 目标（DD5 / AC-5） | 边界 | miss id（0 / -1 / i64::MAX）既有「agent 不存在」Err；目标已默认幂等 Ok；默认标记唯一写口纪律（upsert 臂恒非默认）保持 | 适配 |
| `Store::bind_session_remote` 双 id 留痕写面保留（AC-6） | 正向 | 调用后 `SessionRecord.engine_session_id` 照常置 `Some(remote)`（`SessionRow.remoteSessionId` 线面同源）——留痕写面零改动回归锚 | 适配 |
| `Store::bind_session_remote` 双 id 留痕写面保留（AC-6） | 异常 | 会话不存在 → 既有 Err 形态零改动；`remote` 拒收形态不适用（留痕写面不校验语义保持） | 适配 |
| `Store::bind_session_remote` 双 id 留痕写面保留（AC-6） | 边界 | `remote` 参数 str 边界（空串 / 超长 >1000 / 特殊字符 `\n` `\0` emoji）原样留痕不校验；重复绑定覆盖写（last-write）语义零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 / redb 库（进程边界） | 不 mock：tempfile 真开库（存储层不 mock 既有口径），fixture 经公共写面落库 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/agents/mod.rs -> packages/desktop/src-tauri/src/commands/agents/mod_test.rs

<!-- 挂 AC-5（命令面透传）。跨模块组合用例挂靠本章节（链路发起方 = 管理命令面，最上层调用方）：
     `save_agent_instance → Store::upsert_agent_instance`、
     `set_default_agent_instance → Store::set_default_agent_instance` 全链真实组合 store crate，
     describe 标题写链路方向。命令体三件事纪律不变、签名与 `AgentEngineKind` 参数类型不收窄
    （校验层拒绝、非类型层收窄——bindings 零演进）。 -->

#### 待测功能

- save_agent_instance(stores: State<'_, WorkspaceStores>, id: Option<i64>, name: String, engine: AgentEngineKind, provider_id: Option<i64>) -> Result<AgentInstanceRecord, String>: 签名与参数类型不变；拒 Cli 错误面随 store 单点校验 `Err(String)` 透传（DD3 文案逐字）
- set_default_agent_instance(stores: State<'_, WorkspaceStores>, id: i64) -> Result<AgentInstanceRecord, String>: 签名不变；拒 Cli 错误面透传

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `save_agent_instance` 拒 Cli 透传（链路：命令面 → `Store::upsert_agent_instance`） | 正向 | `engine: sdk` + provider 新建 / 更新 → `Ok(AgentInstanceRecord)` 透传 store 落库记录（薄包装不加工） | 适配 |
| `save_agent_instance` 拒 Cli 透传（链路：命令面 → `Store::upsert_agent_instance`） | 异常 | `engine: Cli`（新建与更新同）→ `Err(String)` 逐字 `CLI_TENANT_RETIRED`（store 单点校验透传不吞不改写）；存量 cli 行更新（含改 engine 整行）拒——组合链贯通（AC-5 全链锚） | 新增 |
| `save_agent_instance` 拒 Cli 透传（链路：命令面 → `Store::upsert_agent_instance`） | 边界 | `id: Option<i64>` 边界（None 新建 / Some(0) / Some(存量) / Some(-1) / Some(i64::MAX) miss）；`name` str 边界（空串 / 超长 / 特殊字符）透传；`provider_id` None / Some(悬空) 既有 Err 面透传 | 适配 |
| `set_default_agent_instance` 拒 Cli 透传（链路：命令面 → `Store::set_default_agent_instance`） | 正向 | sdk 实例 id → `Ok` 更新后记录透传（原子清旧默认全链） | 适配 |
| `set_default_agent_instance` 拒 Cli 透传（链路：命令面 → `Store::set_default_agent_instance`） | 异常 | Cli 目标 id → `Err(String)` 逐字 `CLI_TENANT_RETIRED` | 新增 |
| `set_default_agent_instance` 拒 Cli 透传（链路：命令面 → `Store::set_default_agent_instance`） | 边界 | id 边界 0 / -1 / i64::MAX miss → 既有「agent 不存在」Err 透传；目标已默认幂等透传 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | `tauri::test::mock_app()`（MockRuntime，无窗口无事件循环）manage 真实 `WorkspaceStores` 后经 `app.state` 取 `State`（沿既有 `mod_test.rs` 装置模式） | 本节全部用例 |
| 文件系统 / redb 库（进程边界） | 不 mock：tempfile 真实全局库 / workspace 库读写 | 本节全部用例 |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（bindings.ts 为 specta 再生成物、非手改，禁手写
     旁路）；不虚构条目。被测面 = 修改文件表「预期仅注释级 diff，`AgentEngineKind` 线值
     `("cli" | "sdk")` 不演进、参数类型不收窄」与 `SessionConfigSnapshot` 新键出线
    （AC-7 线面），经 cfg(test) 产物文本断言承载。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bindings 产物线面不演进与新键出线（AC-2 线值锚 / AC-7） | 正向 | 再生后 `AgentEngineKind` 线值恒 `("cli" \| "sdk")`（不收窄不演进）、`saveAgentInstance` 参数类型不收窄（`id: number \| null` / `name` / `engine` / `providerId: number \| null`）；`SessionConfigSnapshot` 线键增 `agentInstanceId: number \| null`（camelCase 与 serde 同形） | 适配 |
| bindings 产物线面不演进与新键出线（AC-2 线值锚 / AC-7） | 异常 | `DTO_TYPES` / `COMMAND_NAMES` 清单与产物强耦合门禁回归：无幽灵 / 遗漏条目、退役类型不残留（产物缺出线类型必挂的反向证明保持）；`bindings/mod_test.rs` Rust 侧快照镜像随再生成同步（非手改） | 适配 |
| bindings 产物线面不演进与新键出线（AC-2 线值锚 / AC-7） | 边界 | 重导幂等既有守线用例族全绿（连续两次导出逐字节一致、篡改产物重导恢复、产物无 `Result` 包装）；预期仅注释级 diff 不触线值 / 参数类型（`engine` 线值收窄即红的回归钉） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| bindings.ts 产物文件 | 进程边界（文件系统，真实组合不 mock）：`export_bindings()` 真实重导 + 生成物文本真实读取（既有装置沿用） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs

<!-- 挂 AC-10（mutation 门在位的前置修复——清单外补入·测试管线修复：cargo-mutants 段执行面
     对齐真实工具 27.1.0 实测行为，原「stdout JSON 载荷」口径对真实工具恒不可满足）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（段执行器为 checks crate 内部装置面，非公共函数
     演进）；不虚构条目。被测面 = 修改文件表四点行为半边：①argv 模板增 `--workspace`
    （混合 workspace 缺省只扫根包，`crates/**` 业务主场恒不在突变面）；②段报告权威落位改
     `mutants.out/outcomes.json`（运行面 stdout 仅人类可读进度行，`--json` 仅作用于 `--list`）；
     ③spawn 前清场 `mutants.out`（陈旧报告不误读）；④退出 0 且无报告 = diff 零可突变交叠
     （EmptyDiff / NoSourceFiles / NoMutants 恒退出 0 不产报告）→ 零突变体段合成空计数面，
     不再误判 `Interrupted`。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| argv 模板含 `--workspace`（段执行面对齐真实工具） | 正向 | spawn argv 逐参等值断言（十参全序列）：`mutants` / `--workspace` / `--in-diff` / 段补丁路径 / `--baseline skip` / `--jobs <半 CPU>` / `--timeout …`——缺 `--workspace` 则 `crates/**` 恒不在突变面（清单外补入·测试管线修复主锚） | 适配 |
| argv 模板含 `--workspace`（段执行面对齐真实工具） | 异常 | 非零退出中断 → 段 Err 携退出码与有界输出尾部（错误面不静默）；spawn 失败（路径不可达）→ 显式 fatal 不挂死 | 适配 |
| argv 模板含 `--workspace`（段执行面对齐真实工具） | 边界 | `jobs` 半 CPU 关系式抽样口径锚定；`cargo_target_dir` 多段共享 env 断言同一不重置；段补丁入参 str 边界（空串 / 超大 / 特殊字符 `\n` `\0` emoji）不破坏 argv 逐参传递（nullish 不适用——Rust 类型面排除） | 适配 |
| 段报告权威落位 `mutants.out/outcomes.json`（报告文件面，非 stdout） | 正向 | canned 真实 `outcomes.json` 全形态（outcomes 逐条 + 计数面 1 caught + 1 missed / total 2）→ 段报告 = 文件原样、解析对拍逐字段等值；报告在位且可解析 → 段完成（退出码非权威判定面） | 适配 |
| 段报告权威落位 `mutants.out/outcomes.json`（报告文件面，非 stdout） | 异常 | 运行面 stdout 仅人类可读进度行（`--json` 仅作用于 `--list`）——原「stdout JSON 载荷」提取口径不可满足：`json_payload`（stdout 提取）用例随实现删除，成功运行不再被误判中断 | 废弃 |
| 段报告权威落位 `mutants.out/outcomes.json`（报告文件面，非 stdout） | 边界 | 计数面合成口径：outcomes 逐条归并（caught / missed / timeout）与段计数对拍，容忍真实工具全形态字段（多余字段不炸、缺省字段不误读） | 适配 |
| spawn 前清场与零突变体段合成空计数面 | 正向 | 预置陈旧 `mutants.out/outcomes.json`（caught 5 计数）+ 假执行体不写新报告 → 本段不误读陈旧计数（清场先于运行，上一段 / 上次运行残留隔离） | 新增 |
| spawn 前清场与零突变体段合成空计数面 | 异常 | 退出 0 且无报告（diff 零可突变交叠：cargo-mutants 对 EmptyDiff / NoSourceFiles / NoMutants 恒退出 0 不产报告）→ 零突变体段合成空计数面走 Ok 臂，不再误判 `Interrupted`（成功运行被误判中断的反向锚） | 新增 |
| spawn 前清场与零突变体段合成空计数面 | 边界 | 超时中断 → 强制终止进程树不挂死（段 Err 携超时口径）；段中断错误面双段装配接线（错误面组装单元半边） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| cargo-mutants 子进程（进程边界） | 假执行体替换 PATH（既有 `SpawnWindow` + `install_fake_cargo` 装置）：argv 逐参记录 + 可编程退出码 / 挂起；报告文件落位经 `FAKE_SPAWN_CARGO_OUTCOMES`（+ALT argv 匹配变体）环境变量编程（既有 Mock 装置零扩展口径） | 本节全部用例 |
| 文件系统（`mutants.out` 目录 / 段补丁） | 不 mock：tempfile 真实落盘（陈旧报告预置 / 清场 / 权威读取全真） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/segments.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/segments_test.rs

<!-- 挂 AC-10（mutation 管线修复：段补丁路径重写基准——清单匹配恒项目根相对、落盘段补丁剥
     suite 根前缀，与 Stryker `mutate` 清单 `relative_to_cwd` 同约定；不剥则 rust 突变面恒空
    「No mutants to filter」空转）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`slice_patch` 为 crate 内 `pub(crate)` 装置面）；
     不虚构条目。被测面 = `slice_patch` 增 suite 根前缀剥离参数与三类路径行重写（修改文件表
     行为半边）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `slice_patch` suite 根前缀剥离（三类路径行重写） | 正向 | `diff --git a/… b/…` / `--- a/…` / `+++ b/…` 三类路径行剥 `<strip_dir>/` 前缀、hunk 体逐字保留——cargo-mutants `--in-diff` 以 spawn cwd 相对路径对源树匹配（路径原样传入恒零命中的修复主锚） | 新增 |
| `slice_patch` suite 根前缀剥离（三类路径行重写） | 异常 | 前缀不匹配（路径无 `<strip_dir>/` 前导）→ 原样返回不误剥；畸形与空入参（空 patch / 空清单）不 panic | 新增 |
| `slice_patch` suite 根前缀剥离（三类路径行重写） | 边界 | `strip_dir` 空串（suite 锚项目根）→ 逐字透传（既有 11 处调用点补空串实参、透传形态逐字等值回归锚）；`+++ /dev/null` 侧逐字保留（删除面段头 a/ 侧可辨照常入段）；特殊文件名（空格 / 引号 / 八进制转义 `\303\251` 形态）不破坏解析 | 适配 |
| 既有段切分 / 估算面零改动回归 | 正向 | 静态估算耗时公式、段数触发口径（恰等不切超一即切）、首段静态初值均分、自适应块大小修正方向与上下限、三段切分并集覆盖零交叠顺序稳定——既有断言保绿（回归锚） | 适配 |
| 既有段切分 / 估算面零改动回归 | 异常 | 补丁切片畸形形态（无头 hunk / 截断行）既有不 panic 断言保绿 | 适配 |
| 既有段切分 / 估算面零改动回归 | 边界 | 段内文件 hunks 与前导文本丢弃既有口径保绿（清单匹配半边零改动） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| （无进程边界） | 纯函数真实组合：patch 文本字面量直入直出（不 mock） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs

<!-- 挂 AC-10（mutation 管线修复：段补丁路径重写基准接线——CargoMutants 臂计算 suite 执行
     目录相对项目根的 POSIX 路径传 `slice_patch`，suite 锚项目根时空串透传）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`execute_segment` 段执行接线为 crate 内部面）；
     不虚构条目。被测面 = CargoMutants 臂的「suite 执行目录 → POSIX 相对路径 → `slice_patch`
     实参」接线与假执行体报告落位编程装置扩展（修改文件表行为半边）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `execute_segment` CargoMutants 臂路径基准接线 | 正向 | 组合根链路段补丁断言改「切片 + suite 根前缀剥离」逐字等值——`slice_patch` 实参 `strip_dir` 为 suite 执行目录相对项目根的 POSIX 路径（Windows 分隔符归一）；suite 锚项目根时空串透传（既有 11 处调用点补空串实参形态） | 适配 |
| `execute_segment` CargoMutants 臂路径基准接线 | 异常 | 假 `mutants` 执行体报告缺位且退出非零 → 段中断错误面（既有中断口径零改动回归）；`to_posix` 分隔符归一半边既有断言保绿 | 适配 |
| `execute_segment` CargoMutants 臂路径基准接线 | 边界 | cargo canned fixture 换真实 `outcomes.json` 计数面形态（装置 `set_cargo_outcomes` / `set_cargo_outcomes_alt` argv 匹配双形态编程）；`SpawnWindow` PATH 前置 msys 工具目录（`Git\usr\bin`——`cp` / `mkdir` 所在，探测失败跳过）：桌面应用门禁进程 PATH 无 usr/bin 时脚本装置不静默缺位（防空零计数假绿与 `TEST_ENV_LOCK` 中毒级联） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `mutants` / `npx` 子进程（进程边界） | 假执行体替换 PATH（既有 `SpawnWindow` 装置）：`install_fake_cargo` / `install_npx` + `FAKE_SPAWN_CARGO_OUTCOMES`（+ALT）报告文件落位编程；`TEST_ENV_LOCK` 串行窗口（环境变量全局态互斥） | 本节全部用例 |
| 文件系统（exec_dir / 报告 / 段补丁落盘） | 不 mock：tempfile 真实读写；msys 工具目录探测真实（探测失败跳过口径） | 本节全部用例 |

### packages/desktop/src/views/agents/components/agent-panel.tsx -> packages/desktop/src/views/agents/components/agent-panel.test.tsx

<!-- 挂 AC-5（入口禁用 DD4 前端半边）/ AC-8。`agent-panel.tsx` 无 design「公共函数 / API」表行
    （React 受控组件、props 面零声明）；不虚构条目。被测面 = 修改文件表（engine 下拉收单值 /
     `ProviderSelect` 删 `engine` prop 恒启用 / `canSave` 改写 / `AgentListItem` 存量 cli 行
     入口禁用）与 DD4 呈现形态。跨模块组合用例挂靠本章节（链路发起方 = `AgentPanel`，
     最上层调用方）：`AgentPanel → use-agent-instances → invoke(save_agent_instance) →
     Store 拒 Cli` 双层兜底链（入口禁用 + 写面拒互为兜底）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`AgentPanel` / `AgentForm` / `AgentListItem` /
     `ProviderSelect` 为组件面，「公共函数 / API」表显式不列）；不虚构条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| engine 下拉收单值与 `canSave` 无 cli 分支（AC-8） | 正向 | `agent-engine-kind` select options 恰 `['sdk']`（无 'cli' 档）；表单恒 sdk；`canSave` = `name 非空 && providerId 非空`（`form.engine === 'cli'` 分支退役）——sdk + name + provider 齐 → 保存启用且 `state.save` 恰调一次 | 重写 |
| engine 下拉收单值与 `canSave` 无 cli 分支（AC-8） | 异常 | 缺 provider / 空名 → 保存禁用（canSave false）；`state.save` reject（后端拒 Cli 文案）→ 错误呈现不吞（组合链：面板 → hook → invoke → store 拒收，双层兜底的后端层） | 适配 |
| engine 下拉收单值与 `canSave` 无 cli 分支（AC-8） | 边界 | `name` str 边界（空串 / 纯空白 / 超长 >1000 / 特殊字符 `\n` `\0` emoji）；`providerId` null 边界恒禁用；既有「engine 下拉仅 cli / sdk 二值」断言翻转为单值断言 | 重写 |
| provider 选择器恒启用必选（AC-8） | 正向 | `ProviderSelect` 无 `engine` prop、无 `disabled={engine === 'cli'}`——恒可交互；providers 清单选中回写 `form.providerId`（「sdk 必选」口径） | 重写 |
| provider 选择器恒启用必选（AC-8） | 异常 | providers 空清单 → 选择器空态呈现、保存不可达（canSave false）不白屏；provider 引用缺失（悬空 providerId）呈现占位既有口径 | 适配 |
| provider 选择器恒启用必选（AC-8） | 边界 | providers 单元素 / 多元素清单（list 映射）选择器稳定；既有「cli 档 provider 下拉禁用（可空语义）」用例废弃（`cli 可空` 语义退役） | 废弃 |
| 存量 cli 行只读可删（DD4 / AC-5 前端半边） | 正向 | 清单含 `engine: 'cli'` 行照常呈现（name / engine 文本 / 「—」占位 / 默认徽标）——历史可读（R5 呈现兜底） | 适配 |
| 存量 cli 行只读可删（DD4 / AC-5 前端半边） | 异常 | cli 行「编辑」「设为默认」按钮禁用 + title 提示「CLI 租户已下线，实例只读」；表单不进 cli 编辑态（编辑入口禁用为主形态，DD4） | 新增 |
| 存量 cli 行只读可删（DD4 / AC-5 前端半边） | 边界 | cli 行「删除」保留可用（`state.remove` 恰调一次）；sdk 行编辑 / 设默认不受影响；既有「cli agent provider 可空提交」「存量 cli 编辑保存」「cli 行设默认」用例群翻转为禁用断言或废弃 | 重写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `AgentInstancesState` 与 providers（显式入参注入） | 不 mock 组件内部——state 动作 `save` / `remove` / `setDefault` 以 `vi.fn()` 可编程 resolve / reject（入参例外，沿既有 `fakeState` 装置）；组件不触 invoke（无 IPC） | 本节全部用例 |
| `ui/**` / `standard/select` | 不 mock——shadcn vendored 底座与标准组件真实渲染（desktop-app-shell 纪律）；禁用态经 `disabled` 属性断言 | 本节全部用例 |

### packages/desktop/src/views/agents/hooks/use-agent-instances.ts -> packages/desktop/src/views/agents/hooks/use-agent-instances.test.ts

<!-- 挂 AC-10（测试文件清单「cli mock 数据处置」）。hook 源文件零改动（fixture 随动级联触点，
     proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（hook 签名与行为零改动）；不虚构条目。被测面 =
     proposal 测试文件清单「cli mock 数据处置」的 fixture 面（拒 Cli 校验单点在 store，
     hook 透传不抢做校验——既有口径保持）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| cli mock 数据处置（fixture 面） | 正向 | mock 清单含 cli 行（`engine: 'cli'`）照常加载呈现——hook 透传不校验（校验单点在 store）；加载 / 刷新既有断言保绿 | 适配 |
| cli mock 数据处置（fixture 面） | 异常 | save 携 cli 被后端 mock `reject`（`CLI_TENANT_RETIRED` 文案）→ 错误态透传不吞（后端拒收呈现链） | 适配 |
| cli mock 数据处置（fixture 面） | 边界 | 清单形态边界（list 映射）：纯 cli / sdk+cli 混合 / 空清单三形态既有断言保绿；fixture 构造器 `engine` 参数枚举两值（sdk / cli）均可达 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（IPC 进程边界） | `vi.mock` invoke 替身分发（含 cli 行 fixture），沿既有装置 | 本节全部用例 |

### packages/desktop/src/hooks/use-agent-chat.ts -> packages/desktop/src/hooks/use-agent-chat.test.ts

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）。源文件零改动（fixture 随动级联
     触点，proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = `SessionConfigSnapshot`
     增列后 configSnapshot mock 形状随动（补 `agentInstanceId` 键，与 bindings 新类型同形）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | 既有 configSnapshot fixture（含 `engine: 'sdk'` 与 `engine: 'cli'` 历史快照两形态）补 `agentInstanceId` 键后全部既有断言保绿（fixture 与 bindings `SessionConfigSnapshot` 新类型同形、无手写类型旁路） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键、`client:check` 承载缺键即红，防 fixture 漂移）；`engine: 'cli'` 历史快照 fixture 照常保留（`engine` 为历史事实不删） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null`（存量会话缺列语义）与具体实例 id（新会话语义；int 边界 0 / -1 / MAX 并入）两形态均被呈现断言容忍；嵌套于消息 / 转录载荷的深浅两层 fixture 同口径 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键（mock 方案零改动） | 本节全部用例 |

### packages/desktop/src/views/agent/hooks/use-agent-run-history.ts -> packages/desktop/src/views/agent/hooks/use-agent-run-history.test.ts

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）。源文件零改动（fixture 随动级联
     触点，proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = run history 载荷内
     configSnapshot mock 形状随动（补 `agentInstanceId` 键）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | run history fixture 内 configSnapshot 补 `agentInstanceId` 键后全部既有断言保绿（与 bindings 新类型同形） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键防漂移）；`engine: 'cli'` 历史快照形态照常保留 | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null` 与具体实例 id（0 / -1 / MAX 并入）两形态均被呈现断言容忍；runs 列表空 / 单元素 / 多元素（list 映射）载荷同口径 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键 | 本节全部用例 |

### packages/desktop/src/views/agent/agent-debug-view.tsx -> packages/desktop/src/views/agent/agent-debug-view.test.tsx

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）。源文件零改动（fixture 随动级联
     触点，proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = debug 视图 fixture 内
     configSnapshot 形状随动（补 `agentInstanceId` 键）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | debug 视图 fixture 补 `agentInstanceId` 键后全部既有断言保绿（与 bindings 新类型同形） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键防漂移）；`engine: 'cli'` 历史快照形态照常保留 | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null` 与具体实例 id（0 / -1 / MAX 并入）两形态均被呈现断言容忍 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键 | 本节全部用例 |

### packages/desktop/src/views/changes/hooks/use-session-transcript.ts -> packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）。源文件零改动（fixture 随动级联
     触点，proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = 转录 fixture 内
     configSnapshot 形状随动（补 `agentInstanceId` 键）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | 转录 fixture 补 `agentInstanceId` 键后全部既有断言保绿（与 bindings 新类型同形） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键防漂移）；`engine: 'cli'` 历史快照形态照常保留 | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null` 与具体实例 id（0 / -1 / MAX 并入）两形态均被呈现断言容忍；转录空 / 单事件 / 多事件（list 映射）载荷同口径 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键 | 本节全部用例 |

### packages/desktop/src/views/changes/flow/session-transcript-panel.tsx -> packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）。源文件零改动（fixture 随动级联
     触点，proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = 转录面板 fixture 内
     configSnapshot 形状随动（补 `agentInstanceId` 键）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | 转录面板 fixture 补 `agentInstanceId` 键后全部既有断言保绿（与 bindings 新类型同形） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键防漂移）；`engine: 'cli'` 历史快照形态照常保留 | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null` 与具体实例 id（0 / -1 / MAX 并入）两形态均被呈现断言容忍 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键 | 本节全部用例 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）。源文件零改动（fixture 随动级联
     触点，proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = change 详情 fixture 内
     configSnapshot 形状随动（补 `agentInstanceId` 键）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | change 详情 fixture 补 `agentInstanceId` 键后全部既有断言保绿（与 bindings 新类型同形） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键防漂移）；`engine: 'cli'` 历史快照形态照常保留 | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null` 与具体实例 id（0 / -1 / MAX 并入）两形态均被呈现断言容忍 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键 | 本节全部用例 |

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）。源文件零改动（fixture 随动级联
     触点，proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = 抽屉 fixture 内
     configSnapshot 形状随动（补 `agentInstanceId` 键）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | 抽屉 fixture 补 `agentInstanceId` 键后全部既有断言保绿（与 bindings 新类型同形） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键防漂移）；`engine: 'cli'` 历史快照形态照常保留 | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null` 与具体实例 id（0 / -1 / MAX 并入）两形态均被呈现断言容忍 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键 | 本节全部用例 |

### packages/desktop/src/views/explores/hooks/use-explore-session.ts -> packages/desktop/src/views/explores/hooks/use-explore-session.test.ts

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）。源文件零改动（fixture 随动级联
     触点，proposal「测试文件」清单承接）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = explore 会话 fixture 内
     configSnapshot 形状随动（补 `agentInstanceId` 键）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | explore 会话 fixture 补 `agentInstanceId` 键后全部既有断言保绿（与 bindings 新类型同形） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键防漂移）；`engine: 'cli'` 历史快照形态照常保留 | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null` 与具体实例 id（0 / -1 / MAX 并入）两形态均被呈现断言容忍 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键 | 本节全部用例 |

### packages/desktop/src/views/agent/components/agent-run-history.tsx -> packages/desktop/src/views/agent/components/agent-run-history.test.tsx

<!-- 挂 AC-7（configSnapshot mock 补 `agentInstanceId` 键）·清单外补入（grep 盘点补齐：
     同载 configSnapshot fixture，proposal 测试文件清单漏列）；源文件零改动（fixture 随动
     级联触点）；design「公共函数 / API」表零声明；不虚构条目。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；不虚构条目。被测面 = run history 组件 fixture 内
     configSnapshot 形状随动（补 `agentInstanceId` 键）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 正向 | run history 组件 fixture 补 `agentInstanceId` 键后全部既有断言保绿（与 bindings 新类型同形） | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 异常 | 缺键旧形态不放行（TS 类型面强制补键防漂移）；`engine: 'cli'` 历史快照形态照常保留 | 适配 |
| configSnapshot mock 补 `agentInstanceId` 键（fixture 随动） | 边界 | `agentInstanceId: null` 与具体实例 id（0 / -1 / MAX 并入）两形态均被呈现断言容忍 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke / fixture 载荷（IPC 进程边界） | 沿既有 `vi.mock` 装置，仅 fixture 形状补键 | 本节全部用例 |

---

## 不可测试项

1. `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（`CLI_TENANT_RETIRED` re-export） — **原因**: 纯 `pub use` 公共导出面（级联触点），零运行时行为、无独立测试路径可解析（不建 `lib_test.rs` 防空套件）；导出正确性由消费方编译期承载（compose / 门面消费 `store::CLI_TENANT_RETIRED` 即编译锚）。
2. `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` / `sdk/runner.rs` / `store_port.rs`（注释级口径随行：CLI 对齐措辞改中立、doc「三面」→「四面」） — **原因**: 注释级改动无逻辑面、无可断言行为；既有 `policy_test.rs` / `runner_test.rs` / `store_port_test.rs` 属零触碰面，保绿即回归锚（先例：`config/mod.rs` 模块 doc re-point 同式）。
3. `packages/desktop/src-tauri/Cargo.toml` / `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`（依赖引用面核查，预期零 diff） — **原因**: 清单静态核对项（`regex` / `glob` 现仅 `sdk/tools.rs` 消费、无 cli 独占条目，R9 勿盲删），非行为断言；由编译面与实现阶段自查承载。
4. `packages/desktop/src-tauri/src/bindings/mod_test.rs`（测试文件自身随再生成同步） — **原因**: 本身即 cfg(test) 产物断言清单（测试文件），无独立被测映射；其线面 / 快照同步断言面挂「bindings.ts -> bindings/mod_test.rs」章节承载。
5. `packages/desktop/package.json`（version 0.5.9 → 0.5.10）+ `tauri.conf.json` 引用维持 + `src-tauri/Cargo.toml` 不随动 + `plugins/dev-team` 保持 2.10.44 零改动（AC-11） — **原因**: 归档轨道版本号与产物零 diff 静态核对（DD12），无运行时行为可断言。
6. `packages/desktop/src-tauri/crates/core/agent/**` 零 diff 硬边界（`AgentRunner` / `AgentEvent` 信封 / run 状态机 / `AgentStartError` 变体集 / `TurnRequest.prior_handle` 参数位，AC-2）与 golden 线面（`core/workflow/tests/golden/` 8 份快照零 `engine` 引用） — **原因**: 「零改动」是 diff 面断言而非行为断言，归守线静态审查（git diff / grep）；行为面由既有 core 套件保绿承载。
7. AC-9 五份 spec delta 落盘与措辞翻案（REMOVED 带 Reason/Migration、MODIFIED header 逐字一致、scenario 标记恰 4 个 #、「引擎恒 CLI」与 SpecSync「CLI 引擎」措辞同步翻案） — **原因**: openspec 静态文本面机械校验，归 openspec 校验轨道（提案层产物已落盘，实现阶段不新增/改写 delta）。
8. AC-10 管线半边（`pnpm -C packages/desktop run client:check` 与 `run test` 全绿、`cargo test --workspace` 全绿、覆盖率达阈值 lines 90 / branches 85 / functions 90、mutation score 达 80、`openspec/config.json` 无新增 excludes） — **原因**: 静态脚本与测试执行类机械步骤由桌面端执行，agent 内不重复执行（归守线 / test-execution 轨道）。
9. AC-1 删除面 grep 锚（`cli/` 整目录 9 文件零残留、`lib.rs` 无 `cli::` / `ClaudeCliRunner` 引用）与随目录删除的 `discover_test.rs` / `flags_test.rs` / `jsonl_test.rs` / `runner_test.rs` — **原因**: 删除物无被测映射（删除文件不进被测范围），零残留为静态 grep 面；cli 实现的行为断言随文件退役，不留空测试。
10. `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs` / `segments_test.rs` / `mod_test.rs`（测试随行文件自身） — **原因**: 修改文件表自带的测试更新说明（断言随被测面契约演进：argv 十参 / canned 换真实 outcomes.json / 前缀剥离用例 / 装置扩展），无独立被测映射；断言面分别挂「cargo_mutants.rs -> cargo_mutants_test.rs」「segments.rs -> segments_test.rs」「mutation/mod.rs -> mod_test.rs」三章节承载（与 bindings/mod_test.rs 同式，见条目 4）。
11. `packages/desktop/src-tauri/crates/infra/checks/src/mutation/report_test.rs` / `stryker.rs` / `stryker_test.rs` / `static_check_test.rs` / `testexec/mod_test.rs` / `testexec/runner_test.rs`（rustfmt 机械随动，六文件） — **原因**: 零 token / 语义变化的格式重排（行折叠 / 展开 / import 排序），无可断言行为差异；`cargo fmt --check` 幂等 + 既有断言保绿即回归锚（server:check 的 fix 模式 `cargo fmt` 随触碰全量机械重排，非行为演进）。
12. `packages/desktop/src/views/db/db-inspector-view.test.tsx`（清单外补入·门禁稳定） — **原因**: 测试文件自身的断言机制加固（弃「点前捕获节点引用」、改 `waitFor` 内按现行首行 `bodyRows()[0]` 重查），断言语义（点行 → 选中 + 抽屉）与被测源文件（`db-inspector-view.tsx` 零改动）均不变，无新增被测面可设计。
13. `packages/desktop/pnpm-workspace.yaml`（清单外补入·环境级级联：`nodeLinker: hoisted`） — **原因**: 包管理器布局声明为开发环境门禁解锁面（isolated 布局长路径下 `tsgolint.cmd` 经 `cmd.exe` 中转超路径阈值失败），非应用运行时行为；全链 exit 0 由 `server:check` / `bindings:export` / `client:check` 管线承载（AC-10 前置）。
14. `packages/desktop/src-tauri/src/commands/exec/mod_test.rs` / `change_flow/mod_test.rs`（清单外补入·保绿随动） — **原因**: 既有用例随被测面退役的机械随动（决策留痕 #6：`seed_cli_default*` fixture 迁 `seed_provider` + `seed_sdk_default*`、PATH 隔离窗口拆除、终态断言 `"failed"` → `"stopped"` 保绿翻转、跨引擎续接用例两边翻转、快照构造补 `agent_instance_id: None` 键），归实现相位承接而非 test-design 新增覆盖；其锚定的行为半边（拒 Cli / 跨引擎续接显式失败 / ConfigMissing 归位 open 段）由 compose / store / agents 章节 + 既有套件保绿承载。



