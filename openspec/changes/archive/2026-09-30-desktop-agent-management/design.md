# 设计: desktop-agent-management

> **变更**: desktop-agent-management
> **日期**: 2026-09-30

---

## 提案与规格同步状态

`proposal.md` 与五个能力规格（desktop-agent-management 新增；desktop-workspace-store / desktop-agent-execution / desktop-page-routing / desktop-data-dimensions 修改）已在提案阶段写入，本设计不再将其列入变更清单与任务。本设计承接提案路由至 dev-design 的 5 项定夺（model 消费档位、name 唯一性实现形态、`agent_start` 的 agent 参数形态、store CRUD API 与命令命名、R1 落点），见「关键设计定夺」。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 管理记录模型 | `AgentProviderRecord`（name / base_url / api_key / models 三档；手写遮蔽 Debug）与 `AgentInstanceRecord`（name / engine / provider_id / 默认标记）定义；native_model 新 id 5 / 6 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | native_db / native_model / serde / specta | `#[native_model]` + `#[native_db]` derive，嵌套纯类型 `AgentModelTiers` / `AgentEngineKind`（同 `AgentEvent` 嵌装先例） |
| 全局库模型组扩展 | `global_models()` 追加两模型注册（全局组三模型，workspace 组不动，无交叉注册） | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | native_db | `OnceLock<&'static Models>` 既有静态追加 |
| 管理操作面 | provider / agent 的 upsert / 删 / 清单 / 直查与默认标记（重名查重、sdk 必填 provider 且引用存在、引用删除阻止、删默认同事务清标记、标记切换原子清旧） | 同上（`Store` 方法） | native_db 事务 | 单写事务查重（`create_explore_record` 同型）+ max+1 id 分配 |
| 记录信封注册 | `MODEL_ENTRIES` 登记两行（`agent_provider` / `agent_instance`，user 维度）；机制与签名零改动 | `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | serde_json | fn-pointer 静态注册表（既有机制，「新模型 = 登记一行」数据行） |
| 公共导出面 | 新类型出 crate（记录 + 两嵌套类型） | `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | — | `pub use` |
| 管理命令轨道 | provider / agent CRUD + 默认标记七命令；全局库实例路由（`global()`）；留空 key 回填原值的参数转换 | `packages/desktop/src-tauri/src/commands/agents/mod.rs`（新） | `WorkspaceStores` | `State<'_, WorkspaceStores>` 薄包装，`Result<T, String>` 模板 |
| 命令登记 | 七命令注入 `all_commands!`（原生 handler 与 export-bindings 两侧自动同步） | `packages/desktop/src-tauri/src/commands/mod.rs` | — | 既有宏清单追加 |
| 运行发起解析单点 | `agent: Option<i64>` → 默认 / 显式 agent → (`EngineKind`, `EngineConfig`)；无默认显式 `Err` 引导管理页；model 取 provider high 档 | `packages/desktop/src-tauri/src/commands/exec/agent.rs` | `WorkspaceStores`、`EngineFacade` | 纯解析函数 `resolve_agent_engine`，产物 `ResolvedEngine` |
| 编排薄入口 | `start_agent_run` 尾参换解析产物；`start_agent_run_with` / `drive_agent_run` 零改动 | 同上 | `EngineFacade` | 门面 `runner_for(kind, engine_cfg)` 签名不动 |
| EngineConfig 构造源 | `from_hardcoded_slot()` 退役 → `empty()`（CLI 臂不消费占位）；机密面标记注释更新；字段面与 `is_complete` 零变化 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` | — | 三字段结构体形态不动 |
| Agent 管理页 | `/agents` 两栏（Providers / Agents）：CRUD 表单、默认标记切换、api_key 遮蔽展示 | `packages/desktop/src/views/agents/agents-view.tsx`（新） | 生成绑定 | react 函数组件 + 就近取数 hooks（显式动作刷新，无轮询） |
| 调试页发起面 | engine 下拉退役 → agent 选择器（默认选中默认 agent）；`AgentStartInput.engine` → `agent` | `packages/desktop/src/views/agent/components/agent-run-form.tsx`、`agent-debug-view.tsx`、`hooks/use-agent-options.ts`（新） | 生成绑定 | 下拉（`ModeSelect` 同型）+ 取数 hook |
| 前端参数面穿透 | transport 位置参数与 body 键 `engine` → `agent`；explore 链缺省路径零改动 | `packages/desktop/src/lib/agent-transport.ts`、`packages/desktop/src/hooks/use-agent-chat.ts` | 生成绑定 | `AgentStartArgs[8]` 类型派生随再生成更新 |
| 路由与侧栏 | `/agents` 路由（不携 root）；「系统工具」组 [Agent 管理] 入口 `nav-agents` | `packages/desktop/src/routes.tsx`、`packages/desktop/src/components/app-sidebar.tsx` | react-router | `NavLink` active 由 URL 派生 |

### 关键设计定夺（proposal 待决问题落点）

1. **model 消费档位取 high（定稿）**：`resolve_agent_engine` 组装 `EngineConfig.model = provider.models.high`。与 proposal D1、spec「默认 agent 与运行发起解析」及 AC-6 逐字一致；effort 进 run 参数与档位选择器为后续迭代，本期不预挂。
2. **name 唯一性 = 写事务内查重，不建唯一二级索引（定稿）**：`upsert` 在单写事务内先按 name 线性查重 → 命中即 `Err`。理由：与 `create_explore_record` 的 find-then-insert 同型先例；native_db 单写者事务内查重无 TOCTOU，唯一性口径与「唯一二级索引」等效（spec 明文允许等效约束），R4「唯一索引支持度未知」风险就此消除；name 当前无按名查询消费方（解析走 id 主键直查），管理数据量（个位数~几十）线性查重零成本，不引入未经本仓验证的 `#[secondary_key(unique)]` API 面。
3. **`agent_start` 的 agent 参数形态（定稿）**：`agent: Option<i64>`——agent 实例稳定 id（D4 主键策略，与后续 workspace→agent 关联同一引用锚点）；`None` = 缺省路径解析默认 agent（explore 链等正式场景恒不传）；`Some(id)` = 调试页显式选择。取代原尾参 `engine: Option<EngineKind>`（同位替换，其余参数序不动）。
4. **store CRUD API 与命令命名（定稿）**：store 侧 upsert 语义（`upsert_agent_provider` / `upsert_agent_instance`，id=0 新建 / id>0 整行替换）+ `remove_*` + `list_*` + 直查 `find_agent_provider` / `find_agent_instance` / `default_agent_instance` + `set_default_agent_instance`（签名见公共函数表）；命令侧七条 `list_agent_providers` / `save_agent_provider` / `delete_agent_provider` / `list_agent_instances` / `save_agent_instance` / `delete_agent_instance` / `set_default_agent_instance`（新增/更新合为 save，id Option 区分，与表单编辑语义同构）。留空 key 的回填属命令层参数转换段：id 存在且入参 api_key 为空 → 读存量记录回填原值，store 恒收全字段。
5. **默认标记唯一写口**：`is_default` 的写入只发生在 `set_default_agent_instance`（写事务内先清全部既有默认再置新，全局恒至多一）与 `remove_agent_instance`（默认 agent 删除时同事务清标记）。`upsert_agent_instance` 新建臂强制 `is_default=false`、更新臂保留存量标记——入参标记不参与写，不变式免受前端入参影响。
6. **R1 存量库 additive 验证落点**：实现阶段一完成模型注册后，以磁盘既有 `desktop-global.redb` 手工验证打开与读写（两新模型可写读、`WorkspaceRecord` 原样可读）并留痕；异常时读 native_db 文档确认 additive 打开语义再继续，不引入迁移层（零迁移纪律不变）。
7. **api_key 机密边界治理（D2 修订 / D7 / D8 落地）**：后端全链路明文（写向 IPC body、读向 IPC body、全局库文件、进程内存），遮蔽只在前端展示层；`AgentProviderRecord` 破例不 derive `Debug`、手写遮蔽 impl（api_key 位输出 `sk-***abc` 形态），测试比较走 `PartialEq`；错误串不拼接机密字段（重名 / 引用阻止等错误只含 name / 计数）。统一可 grep 注释标记（固定 token）`// 机密面有意放宽:…，边界表见 specs/desktop-agent-management` 放置四处：`EngineConfig` 注释更新处、`AgentProviderRecord` 类型与 api_key 字段（读写单 DTO 即记录本体，D2 裁决）、前端遮蔽展示组件。边界表锚点为本 change 的能力 spec（长存），不在 design。token 为相对域根形式，不含 layout 命名隔离扫描的双禁令字面量（磁盘域根目录名与配置文件名），合规。db-inspector 经信封明文展示为 D2 明文接受的自然推论（信封值 = 读向 IPC body 形态），不另设遮蔽臂。
8. **store 零新增依赖**：engine 值域以 store 本地枚举 `AgentEngineKind { Cli, Sdk }`（serde camelCase `"cli" | "sdk"`，与 `EngineKind` 同线值）承载，解析单点一次性映射到 `EngineKind`。理由：store 现仅依赖 core `agent` 纯类型，禁拖入引擎门面 crate（rig-core 等实现依赖不进 store 依赖树）；持久化的是配置值域而非引擎实现，与「core 枚举嵌装记录」先例同理。映射收在 `resolve_agent_engine` 单点（两臂 match）。
9. **「编排层零改动」的边界解释（AC-7）**：`start_agent_run_with` / `drive_agent_run`（编排核心）与 `EngineFacade::runner_for` 签名、`EngineConfig` 字段面零 diff、无引擎 / agent 分支；变化仅两处接线面——`start_agent_run` 薄入口尾参 `engine: EngineKind` → `resolved: ResolvedEngine`（参数传递面），`agent_start` 命令体解析调用一段（参数转换段，符合三件事纪律）。
10. **侧栏与路由落位**：路由 `/agents` → `AgentsView`（不携 root prop，页面不触发任何 workspace 命令）；「系统工具」组排序 [Agent 管理] [Agent 调试] [数据库]（管理页是调试的配置前置，阅读顺序自然），入口 testid `nav-agents`、active = `pathname === '/agents'`，图标取 lucide `Boxes`（同族图标可微调）。

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  proposal.md「变更范围 - 实现文件」10 条全部覆盖；级联文件以「清单外补入」标注。
  测试文件（store / commands / 前端 *_test.rs、*.test.tsx）由 test-design / test-gen 阶段承接，不在本清单。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/src/commands/agents/mod.rs` | 管理命令轨道：七命令薄包装（全局库实例路由 + 参数转换 + `Result<T, String>`），含轨道文档注释；命令注册进 `commands/mod.rs` |
| `packages/desktop/src/views/agents/agents-view.tsx` | 管理页骨架：单页两栏（Providers / Agents），不依赖 workspace root，纯内容页 |
| `packages/desktop/src/views/agents/components/provider-panel.tsx` | Providers 栏：清单 + 新建 / 编辑表单（name / base_url / api_key 遮蔽占位留空保持 / 三档 model）+ 删除（引用阻止错误呈现） |
| `packages/desktop/src/views/agents/components/agent-panel.tsx` | Agents 栏：清单 + 新建 / 编辑表单（name / engine 二值 / provider 选择，sdk 必填）+ 默认标记切换 + 删除 |
| `packages/desktop/src/views/agents/components/masked-api-key.tsx` | api_key 遮蔽展示组件：`sk-***abc` 形态（取末 3 字符，长度不足或为空恒 `sk-***`），机密标记注释放置点 |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | provider 清单取数收口：挂载 invoke 一次 + 动作轨道（save / delete，失败 toast 固定前缀，清单加载失败 inline error 态），显式动作刷新，无轮询 |
| `packages/desktop/src/views/agents/hooks/use-agent-instances.ts` | agent 清单取数收口：同上 + `setDefault` 动作（标记即切换，成功后刷新清单） |
| `packages/desktop/src/views/agent/hooks/use-agent-options.ts` | 调试页 agent 选择器取数：挂载 invoke `list_agent_instances` 一次，派生默认选中（`is_default` 记录 id，无默认回 null） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 新增 `AgentProviderRecord`（native_model id 5 version 1，无 `derive(Debug)` + 手写遮蔽 impl）与 `AgentInstanceRecord`（id 6 version 1）及嵌套类型 `AgentModelTiers` / `AgentEngineKind`；两记录构造器 `new`（id=0，写事务 max+1 覆盖）；机密标记注释 | 既有四模型（id 1–4）零触碰 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `global_models()` 追加两模型注册；`Store` 新增管理操作面十方法（upsert / remove / list / find / default / set_default，见公共函数表）：单写事务内 name 查重、sdk 必填 provider 且引用存在、引用删除阻止（`Err` 含引用方 name）、删默认同事务清标记、set_default 原子清旧置新、max+1 id 分配；`DbDimension::User` 文档注释「及未来 user 维度租户」演进为已落地代表留痕 | 唯一性为事务内查重（定夺 2），无迁移代码路径（AC-10） |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | 公共导出追加 `AgentProviderRecord` / `AgentInstanceRecord` / `AgentModelTiers` / `AgentEngineKind` | native_db 类型不越 crate 公共面纪律不变 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `pub mod agents;` + `all_commands!` 追加七命令路径 | proposal「新轨道，注册进命令 mod」条目的注册半边；24 → 31 条 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `agent_start` 尾参 `engine: Option<EngineKind>` → `agent: Option<i64>`；解析调用 `agent::resolve_agent_engine(stores.inner(), agent)?`（无默认 `Err` 抵达前端）；`EngineKind` import 随退役清除；命令文档注释同步（缺省 = 解析默认 agent） | 其余四命令零改动；`Result<T, String>` 模板不变 |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | 删除 `DEFAULT_ENGINE` 常量；新增 `ResolvedEngine` 结构体与 `resolve_agent_engine`（默认 / 显式解析 + `AgentEngineKind` → `EngineKind` 映射 + sdk 由引用 provider 组装 `EngineConfig{api_key, base_url, model: models.high}`，cli 臂 `EngineConfig::empty()`）；`start_agent_run` 尾参换 `resolved: ResolvedEngine`；`start_agent_run_with` / `drive_agent_run` 零改动 | 换源零改动承诺兑现（定夺 9） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` | `from_hardcoded_slot()` 删除，新增 `EngineConfig::empty()`（字段全空，CLI 臂不消费占位）；模块与类型文档改写：构造源为命令层管理数据解析，机密标记注释落位；字段面与 `is_complete` 零变化 | 三字段结构体形态不动（AC-7 消费面零改动） |
| `packages/desktop/src/routes.tsx` | 路由表新增 `/agents` → `<AgentsView />`（不传 root） | 兜底重定向与既有路由零变化 |
| `packages/desktop/src/components/app-sidebar.tsx` | `SystemToolsGroup` 新增 [Agent 管理] 项（`data-testid="nav-agents"`，`active={pathname === '/agents'}`，lucide `Boxes`），组内排序 [Agent 管理] [Agent 调试] [数据库]；组件文档注释同步 | 「页面」组四项与既有 testid 零变化 |
| `packages/desktop/src/views/agent/components/agent-run-form.tsx` | `AgentStartInput.engine: EngineKind` → `agent: number \| null`；`ENGINE_OPTIONS` 与 engine state 退役 → agent 选择器（选项 = agent 实例清单，`data-testid="agent-select"`，默认选中默认 agent，清单为空可发起走后端缺省解析报错）；组件文档注释同步 | permission-mode 下拉与 prompt 面不变 |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 挂 `useAgentOptions()` 取 agent 实例清单，透传 `agents` prop 给 `AgentRunForm`；文档注释更新（engine 直选 → agent 选择） | 时间线 / 原始流 / 历史区 / 停止入口零改动 |
| `packages/desktop/src/lib/agent-transport.ts` | `AgentStartChainParams.engine` → `agent`（`AgentStartArgs[8]` 派生随绑定更新）；`readEngine` → `readAgentId`（`number \| null` 运行时校验）；invoke 尾参同步 | 通道翻译与信封处理零改动 |
| `packages/desktop/src/hooks/use-agent-chat.ts` | `AgentChatSendInput.engine?: EngineKind` → `agent?: number`；body 键 `engine:` → `agent:`；`EngineKind` import 清除 | hook 对外签名不变；explore 链不传 agent（缺省语义零改动，AC-7） |

#### 清单外补入（级联文件）

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | `MODEL_ENTRIES` 登记 `agent_provider` / `agent_instance` 两行（user 维度，count / scan / key_of fn-pointer），新增对应 `scan_*` / `*_key` 私有数据行函数 | 级联：AC-11 依赖新模型出现在全局库清单；「新模型 = 定义 struct + 登记一行」既有扩展机制的数据行追加，信封 API 签名与查看器代码零改动（同 db-split 先例口径） |
| `packages/desktop/src/types/generated/bindings.ts` | 随 `bindings:export` 再生成：七新命令 + `agent_start` 参数面 + `AgentProviderRecord` / `AgentInstanceRecord` / `AgentModelTiers` / `AgentEngineKind` 类型出线 | 级联：生成物，零手写 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 绑定快照随再生成命令同步更新（机械再生成，非手改用例） | 级联：proposal「快照测试同步再生成」条目 |

### 删除文件

<!-- 无删除文件：DEFAULT_ENGINE 常量与 from_hardcoded_slot() 构造随实现退役，属代码级删除，不涉整文件删除 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `AgentProviderRecord::new` | `.../infra/store/src/model.rs` | 新增 | `pub fn new(name: String, base_url: String, api_key: String, models: AgentModelTiers) -> Self` | 新建语义构造：id 置 0（写事务 max+1 覆盖）；api_key 传空即空（无原值可保） |
| `AgentInstanceRecord::new` | `.../infra/store/src/model.rs` | 新增 | `pub fn new(name: String, engine: AgentEngineKind, provider_id: Option<i64>) -> Self` | 新建语义构造：id 置 0、`is_default = false`（默认标记唯一写口为 set_default） |
| `Store::upsert_agent_provider` | `.../infra/store/src/store.rs` | 新增 | `pub fn upsert_agent_provider(&self, provider: AgentProviderRecord) -> Result<AgentProviderRecord, StoreError>` | id=0 新建（事务内 name 查重 + max+1 分配）/ id>0 整行替换（查重排除自身）；name 空白 `Err`；返回落库记录 |
| `Store::remove_agent_provider` | `.../infra/store/src/store.rs` | 新增 | `pub fn remove_agent_provider(&self, id: i64) -> Result<bool, StoreError>` | 被 agent 引用 → `Err`（含引用方 name 提示，不级联不删除）；miss 幂等 `Ok(false)` |
| `Store::list_agent_providers` | `.../infra/store/src/store.rs` | 新增 | `pub fn list_agent_providers(&self) -> Result<Vec<AgentProviderRecord>, StoreError>` | 主键 id 升序自然序（稳定可复现） |
| `Store::find_agent_provider` | `.../infra/store/src/store.rs` | 新增 | `pub fn find_agent_provider(&self, id: i64) -> Result<Option<AgentProviderRecord>, StoreError>` | 主键直查（解析单点消费 + save 回填原值） |
| `Store::upsert_agent_instance` | `.../infra/store/src/store.rs` | 新增 | `pub fn upsert_agent_instance(&self, agent: AgentInstanceRecord) -> Result<AgentInstanceRecord, StoreError>` | 新建 / 整行替换 + name 查重；`engine == Sdk` 时 `provider_id` 必填且引用的 provider 必须存在（缺失 / 悬空均 `Err`）；新建臂强制 `is_default=false`、更新臂保留存量标记 |
| `Store::remove_agent_instance` | `.../infra/store/src/store.rs` | 新增 | `pub fn remove_agent_instance(&self, id: i64) -> Result<bool, StoreError>` | 默认 agent 同事务清标记后删（无顺延）；miss 幂等 `Ok(false)` |
| `Store::list_agent_instances` | `.../infra/store/src/store.rs` | 新增 | `pub fn list_agent_instances(&self) -> Result<Vec<AgentInstanceRecord>, StoreError>` | 主键 id 升序自然序 |
| `Store::find_agent_instance` | `.../infra/store/src/store.rs` | 新增 | `pub fn find_agent_instance(&self, id: i64) -> Result<Option<AgentInstanceRecord>, StoreError>` | 主键直查（显式路径解析） |
| `Store::default_agent_instance` | `.../infra/store/src/store.rs` | 新增 | `pub fn default_agent_instance(&self) -> Result<Option<AgentInstanceRecord>, StoreError>` | 缺省路径解析入口（清单扫 `is_default`，恒零或一） |
| `Store::set_default_agent_instance` | `.../infra/store/src/store.rs` | 新增 | `pub fn set_default_agent_instance(&self, id: i64) -> Result<AgentInstanceRecord, StoreError>` | 标记即切换：单写事务内清全部既有默认 → 置目标；miss `Err`；返回更新后记录 |
| `EngineConfig::empty` | `.../infra/agent/src/sdk/config.rs` | 新增 | `pub fn empty() -> Self` | 三字段全空占位（CLI 臂不消费）；替代退役的 `from_hardcoded_slot()` |
| `EngineConfig::from_hardcoded_slot` | `.../infra/agent/src/sdk/config.rs` | 修改 | `pub fn from_hardcoded_slot() -> Self` | 退役（删除）：硬编码预留位由管理数据解析取代 |
| `resolve_agent_engine` | `.../src/commands/exec/agent.rs` | 新增 | `pub(crate) fn resolve_agent_engine(stores: &WorkspaceStores, agent: Option<i64>) -> Result<ResolvedEngine, String>` | 解析单点：`None` → `default_agent_instance`（无默认 `Err` 引导管理页）/ `Some(id)` → `find_agent_instance`；sdk 由引用 provider 组装 config（model 取 high 档），cli 臂 `empty()` |
| `start_agent_run` | `.../src/commands/exec/agent.rs` | 修改 | `pub(crate) fn start_agent_run<T: Runtime>(app: AppHandle<T>, stores: &WorkspaceStores, on_event: Channel<AgentRunMessage>, params: AgentRunParams, provenance: RunProvenance, resolved: ResolvedEngine) -> Result<AgentRunRecord, String>` | 尾参 `engine: EngineKind` → `resolved: ResolvedEngine`；`runner_for(resolved.kind, resolved.config)`；形参数仍 6 个 |
| `DEFAULT_ENGINE` | `.../src/commands/exec/agent.rs` | 修改 | `pub(crate) const DEFAULT_ENGINE: EngineKind` | 退役（删除）：缺省收敛改由解析单点承载 |
| `agent_start` | `.../src/commands/exec/mod.rs` | 修改 | `pub async fn agent_start(app: AppHandle, on_event: Channel<AgentRunMessage>, root: String, prompt: String, permission_mode: AgentPermissionMode, resume_session_id: Option<String>, source: Option<String>, source_ref: Option<String>, parent_run_id: Option<i64>, agent: Option<i64>) -> Result<AgentRunRecord, String>` | 尾参 `engine: Option<EngineKind>` → `agent: Option<i64>`；解析 `Err`（无默认 / agent 不存在）reject 前端 |
| `list_agent_providers` | `.../src/commands/agents/mod.rs` | 新增 | `pub fn list_agent_providers(stores: State<'_, WorkspaceStores>) -> Result<Vec<AgentProviderRecord>, String>` | 全局库实例（`global()`）薄包装 |
| `save_agent_provider` | `.../src/commands/agents/mod.rs` | 新增 | `pub fn save_agent_provider(stores: State<'_, WorkspaceStores>, id: Option<i64>, name: String, base_url: String, api_key: String, models: AgentModelTiers) -> Result<AgentProviderRecord, String>` | id `None` 新建 / `Some` 更新；参数转换段：id 存在且 api_key 为空 → 读存量记录回填原值（留空 = 保持原值） |
| `delete_agent_provider` | `.../src/commands/agents/mod.rs` | 新增 | `pub fn delete_agent_provider(stores: State<'_, WorkspaceStores>, id: i64) -> Result<bool, String>` | 引用阻止 `Err` / miss 幂等 `Ok(false)` |
| `list_agent_instances` | `.../src/commands/agents/mod.rs` | 新增 | `pub fn list_agent_instances(stores: State<'_, WorkspaceStores>) -> Result<Vec<AgentInstanceRecord>, String>` | 同 list 模板 |
| `save_agent_instance` | `.../src/commands/agents/mod.rs` | 新增 | `pub fn save_agent_instance(stores: State<'_, WorkspaceStores>, id: Option<i64>, name: String, engine: AgentEngineKind, provider_id: Option<i64>) -> Result<AgentInstanceRecord, String>` | sdk 缺 provider 校验在 store 单点 |
| `delete_agent_instance` | `.../src/commands/agents/mod.rs` | 新增 | `pub fn delete_agent_instance(stores: State<'_, WorkspaceStores>, id: i64) -> Result<bool, String>` | 默认 agent 删后清标记（store 同事务） |
| `set_default_agent_instance` | `.../src/commands/agents/mod.rs` | 新增 | `pub fn set_default_agent_instance(stores: State<'_, WorkspaceStores>, id: i64) -> Result<AgentInstanceRecord, String>` | 标记即切换，返回更新后记录 |
| `AgentsView` | `packages/desktop/src/views/agents/agents-view.tsx` | 新增 | `function AgentsView(): React.JSX.Element` | 无 props（全局语义，不依赖 root）；组合根：挂两取数 hook 后分别下发两 panel |
| `ProviderPanel` | `packages/desktop/src/views/agents/components/provider-panel.tsx` | 新增 | `function ProviderPanel(props: { state: AgentProvidersState }): React.JSX.Element` | Providers 栏：清单（name / base_url / 三档 model / 遮蔽展示）+ 新建 / 编辑表单（api_key 编辑态恒空 + 遮蔽占位留空保持原值）+ 删除（引用阻止 `Err` 呈现，清单不变） |
| `AgentPanel` | `packages/desktop/src/views/agents/components/agent-panel.tsx` | 新增 | `function AgentPanel(props: { state: AgentInstancesState; providers: AgentProviderRecord[] }): React.JSX.Element` | Agents 栏：清单（name / engine / 引用 provider 名 / 默认标记）+ 新建 / 编辑表单（engine 二值 + provider 选择，sdk 必填）+ 默认标记切换 + 删除 |
| `MaskedApiKey` | `packages/desktop/src/views/agents/components/masked-api-key.tsx` | 新增 | `function MaskedApiKey(props: { apiKey: string }): React.JSX.Element` | 遮蔽展示：末 3 字符 + `sk-***` 前缀；空 / 过短恒 `sk-***`；机密标记注释 |
| `useAgentProviders` | `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | 新增 | `function useAgentProviders(): AgentProvidersState` | 挂载取数一次 + save / remove 动作轨道（失败 toast，不置 error 态） |
| `useAgentInstances` | `packages/desktop/src/views/agents/hooks/use-agent-instances.ts` | 新增 | `function useAgentInstances(): AgentInstancesState` | 同上 + `setDefault` 动作 |
| `useAgentOptions` | `packages/desktop/src/views/agent/hooks/use-agent-options.ts` | 新增 | `function useAgentOptions(): AgentOptionsState` | 调试页选择器取数（挂载一次，无轮询） |
| `AgentRunForm` | `packages/desktop/src/views/agent/components/agent-run-form.tsx` | 修改 | `function AgentRunForm(props: { disabled: boolean; agents: AgentInstanceRecord[]; onStart: (input: AgentStartInput) => void }): React.JSX.Element` | engine 下拉 → agent 选择器（`data-testid="agent-select"`，默认选中默认 agent） |

<!-- 私有函数不列：envelope 的 scan_* / *_key 数据行、store 查重与 id 分配内部实现（pub(crate) / 私有）、agent.rs 内部映射臂均为实现体；useAgentChat / TauriAgentTransport 对外签名零变化（仅 body 键与入参校验内部演进），见修改文件行 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `AgentProviderRecord` | `.../infra/store/src/model.rs` | 新增 | native_model id 5 / version 1；字段 `id: i64`（PK）/ `name: String` / `base_url: String` / `api_key: String` / `models: AgentModelTiers`；derive `Clone, PartialEq, Eq, Serialize, Deserialize, Type` + serde camelCase（IPC 读写单 DTO）；**无 `derive(Debug)`**，手写遮蔽 impl（api_key 位 `sk-***abc` 形态） |
| `AgentInstanceRecord` | `.../infra/store/src/model.rs` | 新增 | native_model id 6 / version 1；字段 `id: i64`（PK）/ `name: String` / `engine: AgentEngineKind` / `provider_id: Option<i64>` / `is_default: bool`；derive 含 `Debug`（无机密字段） |
| `AgentModelTiers` | `.../infra/store/src/model.rs` | 新增 | 纯嵌套 struct：`high / medium / low` 三 `String`；derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type` + serde camelCase（嵌装先例同 `AgentEvent`，无 native_db derive） |
| `AgentEngineKind` | `.../infra/store/src/model.rs` | 新增 | 枚举 `Cli / Sdk`；derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type` + serde camelCase（线值 `"cli" | "sdk"`，与 `EngineKind` 同值域；store 本地枚举，映射收解析单点，定夺 8） |
| `ResolvedEngine` | `.../src/commands/exec/agent.rs` | 新增 | `pub(crate) struct { kind: EngineKind, config: EngineConfig }`：解析产物（kind + 组装好的连接配置），编排薄入口入参 |
| `AgentStartInput` | `packages/desktop/src/views/agent/components/agent-run-form.tsx` | 修改 | `{ prompt: string; permissionMode: AgentPermissionMode; agent: number \| null }`（原 `engine: EngineKind` 字段退役） |
| `AgentProvidersState` | `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | 新增 | `{ providers, loading, error, save, remove }`：清单态 + 动作轨道（沿 use-workspaces 双轨错误呈现） |
| `AgentInstancesState` | `packages/desktop/src/views/agents/hooks/use-agent-instances.ts` | 新增 | `{ instances, loading, error, save, remove, setDefault }` |
| `AgentOptionsState` | `packages/desktop/src/views/agent/hooks/use-agent-options.ts` | 新增 | `{ instances, loading, defaultId }`：选择器数据面（`defaultId` = 默认 agent id 或 null） |

<!-- EngineConfig / EngineKind / 既有四模型 / AgentRunMessage 零变化，不列；EngineConfig 仅构造方法退役更替（见公共函数表），字段面不动 -->

### 配置

| 配置键 | 所在文件 | 类型 | 值 | 说明 |
|--------|----------|------|-----|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.3.13"`（自 `"0.3.12"`） | 用户可见行为变更（新增 Agent 管理页 + 调试页发起面演进；沿用 desktop 版本口径：仅用户可见变更 bump） |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `AgentProviderRecord` | `id`（PK，写事务内 max+1）/ `name`（唯一 = 事务内查重）/ `base_url` / `api_key`（明文，遮蔽 Debug）/ `models: AgentModelTiers{high, medium, low}` | 被 `AgentInstanceRecord.provider_id` N:1 引用（删除阻止）；后续 workspace→agent 关联链的引用锚点之一（稳定 id） | **全局库**（`home_dir()/.dev-team/desktop-global.redb`，user 维度，native_model id 5 v1） |
| `AgentInstanceRecord` | `id`（PK，写事务内 max+1）/ `name`（唯一 = 事务内查重）/ `engine: AgentEngineKind` / `provider_id: Option<i64>`（sdk 必填且须存在、cli 可空）/ `is_default: bool`（全局恒至多一） | `provider_id` → 同库 `AgentProviderRecord`；`is_default` 为缺省运行解析入口；后续 workspace 关联引用本记录稳定 id | 全局库（native_model id 6 v1） |
| `WorkspaceRecord`（既有） | 零变化 | 同库共存，互不影响 | 全局库（id 1 v1，存量原样可读，AC-10） |
| `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`（既有） | 零变化 | — | workspace 库（id 2 / 3 / 4，`不要修改` 清单项） |

管理两记录与 `WorkspaceRecord` 同住全局库（user 维度），不与 workspace 域三模型交叉注册；时间戳字段不设（管理页无展示与排序诉求，simplicity first）。后续 workspace 关联 agent 时解析顺序演变为 workspace 关联 → 全局默认，本期不预建关联字段或解析分支（spec 边界 2）。

---

## 路由/API 设计

本变更不涉及 HTTP API。前端路由 `/agents` 新增（HashRouter，壳态顶层路由，不携 root 段）；以下为 Tauri IPC 命令面（前端经生成绑定 invoke，错误经 `Result<T, String>` reject）。

| 命令 | 类型 | 入参 | 返回 | 说明 |
|------|------|------|------|------|
| `list_agent_providers` | 新增 | — | `Promise<AgentProviderRecord[]>` | 全局库清单（id 升序） |
| `save_agent_provider` | 新增 | `id?` `name` `baseUrl` `apiKey` `models{high, medium, low}` | `Promise<AgentProviderRecord>` | 新建（id null）/ 更新；重名 reject；留空 key = 保持原值 |
| `delete_agent_provider` | 新增 | `id` | `Promise<boolean>` | 被引用 reject（含引用方提示）；miss 幂等 false |
| `list_agent_instances` | 新增 | — | `Promise<AgentInstanceRecord[]>` | 全局库清单（id 升序） |
| `save_agent_instance` | 新增 | `id?` `name` `engine`（`"cli" \| "sdk"`） `providerId?` | `Promise<AgentInstanceRecord>` | sdk 缺 provider / 悬空引用 reject |
| `delete_agent_instance` | 新增 | `id` | `Promise<boolean>` | 默认 agent 删除同事务清标记 |
| `set_default_agent_instance` | 新增 | `id` | `Promise<AgentInstanceRecord>` | 标记即切换（旧默认自动清除） |
| `agent_start` | 修改 | `root` `prompt` `permissionMode` `resumeSessionId?` `source?` `sourceRef?` `parentRunId?` + `agent?: number`（原 `engine?` 位替换）+ `Channel<AgentRunMessage>` | `Promise<AgentRunRecord>`（提前 resolve running 记录） | 尾参语义演进；无默认 agent / agent 不存在 reject；提前 resolve 契约与 Channel 流零改动 |

`agent_start` 生成绑定参数序不变（`agent` 仍为第 8 个 IPC 参数位），`agent-transport.ts` 的 `AgentStartArgs` 索引随再生成同步；`/agents` 路由不触发任何 workspace 命令（spec「路由表与 Router 选型」场景）。

---

## 依赖

### 运行时依赖

- 无新增依赖 — store / agent-runtime / app 依赖树零变化（native_db / native_model / serde / specta / dunce / sha2 / tauri 系均既有；定夺 8 保证 store 不引入 agent-runtime 依赖）

### 构建/测试依赖

- `tauri-specta` / `specta-typescript`（既有）— 命令面与类型变更后 TS bindings 再生成（`bindings:export`）
- `cargo fmt` / `cargo clippy` / `vp check` / `knip`（既有）— 守线静态检查
- `tempfile`（既有）— store / commands 测试临时目录（测试编写与执行归 test-design / test-gen / test-execution 阶段，不在本变更任务）

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `routes.tsx` `/agents` 路由 + `agents-view.tsx` 两栏骨架；`app-sidebar.tsx` 系统工具组 [Agent 管理]（`nav-agents`，active 由 `pathname === '/agents'` 派生） |
| AC-2 | `provider-panel.tsx` CRUD 表单；重名保存经 store 事务内查重 `Err` reject（toast 呈现）；编辑态 api_key 输入框恒空 + 遮蔽占位，`save_agent_provider` 参数转换段回填原值（留空 = 原值不变） |
| AC-3 | `agent-panel.tsx` CRUD；store `upsert_agent_instance` 对 `engine == Sdk` 且 `provider_id` 缺失 / 悬空 `Err`，cli 可空保存成功 |
| AC-4 | `set_default_agent_instance` 单写事务内清旧置新（全局恒至多一）；前端 `setDefault` 动作后刷新清单呈现唯一默认 |
| AC-5 | `remove_agent_provider` 引用检查 `Err`（不级联，两类记录均原样保留，页面 error 呈现清单不变）；`remove_agent_instance` 对默认 agent 同事务清标记后删 |
| AC-6 | `AgentModelTiers{high, medium, low}` 随 provider 记录存储；`resolve_agent_engine` sdk 臂 `model: provider.models.high`（定夺 1） |
| AC-7 | `agent: Option<i64>` 缺省 → `default_agent_instance`，无默认 `Err` 引导管理页（不落库不推流不回退）；explore 链不传 agent 零改动自动获得解析；`crates/core/agent` 零触碰（`AgentRunParams` 无 agent 字段、`AgentStartError` 变体集不变）；`runner_for` 签名 / `EngineConfig` 字段面 / `start_agent_run_with` / `drive_agent_run` 零 diff（定夺 9） |
| AC-8 | `use-agent-options.ts` 取清单 + `agent-run-form.tsx` agent 选择器默认选中默认 agent；选 sdk agent 发起经解析单点走 SDK 引擎，时间线 / 落库 / 重放组件零改动复用 |
| AC-9 | `AgentProviderRecord` 无 `derive(Debug)` + 手写遮蔽 impl；统一标记注释四处落位（定夺 7），token 可 grep 且无 layout 双禁令字面量；`MaskedApiKey` 组件 `sk-***abc` 形态；错误串只含 name / 计数，零机密字段拼接 |
| AC-10 | `global_models()` additive 追加两模型 + R1 存量库真机验证（定夺 6，阶段一首任务留痕）；store 无迁移代码路径（零迁移纪律不变） |
| AC-11 | `envelope.rs` 注册表登记两行（既有扩展机制数据行），`list_models` / `scan` 签名与 db-inspector 前端、`commands/db` 零改动 |
| AC-12 | 全部实现任务完成后由 test-execution 阶段执行全管线验证；实现守线为静态检查（server:check / client:check / bindings:check），本设计任务列表不含测试执行 |

---

## 待决问题

- R1 存量全局库 additive 打开：定夺 6 已定验证落点（阶段一手工真机验证 + 留痕），结果待实现阶段回填；若 native_db 对存量文件追加模型的打开路径异常，回溯本设计再议（不引入迁移层为硬约束）
- 侧栏图标 `Boxes` 与遮蔽形态细节（末 3 字符截取）为实现可微调项，语义约束（testid、`sk-***abc` 形态、全遮蔽兜底）已钉死
- 无阻塞性待决：proposal 路由至 dev-design 的 5 项（model 消费档位、name 唯一性实现形态、agent 参数形态、store CRUD API 与命令命名、R1 落点）已全部在「关键设计定夺」落定
