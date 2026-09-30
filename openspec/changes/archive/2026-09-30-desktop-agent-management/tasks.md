# 任务: desktop-agent-management

> **变更**: desktop-agent-management
> **日期**: 2026-09-30

任务按依赖排序：阶段一（store 管理模型与操作面）是阶段二（命令轨道）的前置，阶段二先于阶段三（运行发起接线，消费 store 解析面），阶段四（bindings 再生成）依赖后端三阶段全部落定，阶段五（前端接线）消费再生成绑定，阶段六收尾依赖前五阶段。测试编写与执行归 test-design / test-gen / test-execution 阶段，不在本列表。

## 阶段一：store crate 管理模型与操作面

- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：新增 `AgentModelTiers`（`high` / `medium` / `low` 三 `String` 纯嵌套 struct，derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type` + serde camelCase）与 `AgentEngineKind`（`Cli` / `Sdk` 枚举，derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type` + serde camelCase 线值 `"cli" | "sdk"`；store 本地枚举，不引入 agent-runtime 依赖）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：新增 `AgentProviderRecord`——`#[native_model(id = 5, version = 1)]` + `#[native_db]`，字段 `id`（主键）/ `name` / `base_url` / `api_key` / `models: AgentModelTiers`，derive `Clone, PartialEq, Eq, Serialize, Deserialize, Type` + serde camelCase，**不 derive `Debug`**，手写遮蔽 Debug impl（api_key 位输出 `sk-***abc` 形态）；`new(name, base_url, api_key, models) -> Self` 构造器（id 置 0）；类型与 api_key 字段落机密标记注释 `// 机密面有意放宽:…，边界表见 specs/desktop-agent-management`
- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：新增 `AgentInstanceRecord`——`#[native_model(id = 6, version = 1)]` + `#[native_db]`，字段 `id`（主键）/ `name` / `engine: AgentEngineKind` / `provider_id: Option<i64>` / `is_default: bool`，derive 含 `Debug`；`new(name, engine, provider_id) -> Self` 构造器（id 置 0、`is_default = false`）；既有四模型（id 1–4）零触碰
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：`global_models()` 追加 `define::<AgentProviderRecord>()` 与 `define::<AgentInstanceRecord>()`（全局组三模型，`workspace_models()` 与交叉注册禁令不动）；`DbDimension::User` 文档注释由「及未来 user 维度租户」演进为 agent 管理已落地代表留痕
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：provider 操作面——`upsert_agent_provider`（id=0 新建：单写事务内 name 查重（空白名 `Err`）+ max+1 分配；id>0 整行替换：查重排除自身）、`remove_agent_provider`（任何 agent `provider_id` 命中即 `Err` 含引用方 name 提示、不级联不删除；miss 幂等 `Ok(false)`）、`list_agent_providers`（主键 id 升序）、`find_agent_provider`（主键直查）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：agent 操作面——`upsert_agent_instance`（新建 / 整行替换 + name 查重；`engine == Sdk` 时 `provider_id` 必填且引用 provider 须存在（缺失 / 悬空均 `Err`）；新建臂强制 `is_default = false`、更新臂保留存量标记）、`remove_agent_instance`（默认 agent 同事务清标记后删；miss 幂等）、`list_agent_instances`（id 升序）、`find_agent_instance`、`default_agent_instance`（清单扫 `is_default`，恒零或一）、`set_default_agent_instance`（单写事务内清全部既有默认 → 置目标；miss `Err`；返回更新后记录）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs`：`MODEL_ENTRIES` 登记 `agent_provider` / `agent_instance` 两行（`DbDimension::User`，count / scan / key_of fn-pointer）及对应 `scan_*` / `*_key` 私有数据行函数；`list_models` / `scan` 签名与信封类型零变化
- [x] `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`：公共导出追加 `AgentProviderRecord` / `AgentInstanceRecord` / `AgentModelTiers` / `AgentEngineKind`
- [x] R1 留痕：以磁盘既有存量 `desktop-global.redb` 手工验证 additive 打开与读写（`WorkspaceRecord` 原样可读、两新模型可写读、`list_models` 出现四行）；异常时读 native_db 文档确认 additive 打开语义后回修，不引入迁移层（手工验证，非自动化测试）

## 阶段二：管理命令轨道

- [x] `packages/desktop/src-tauri/src/commands/agents/mod.rs`（新）：轨道文档注释（全局轨经 `global()`、MUST NOT 误路由 workspace 库、`Result<T, String>` 模板）+ 七命令薄包装——`list_agent_providers(stores)`、`save_agent_provider(stores, id, name, base_url, api_key, models)`（参数转换段：id 存在且 api_key 为空 → `find_agent_provider` 回填原值后整行 upsert）、`delete_agent_provider(stores, id)`、`list_agent_instances(stores)`、`save_agent_instance(stores, id, name, engine, provider_id)`、`delete_agent_instance(stores, id)`、`set_default_agent_instance(stores, id)`；store `StoreError` 以 `.to_string()` 映射，不进命令签名
- [x] `packages/desktop/src-tauri/src/commands/mod.rs`：`pub mod agents;` + `all_commands!` 追加七命令路径（原生 handler 与 export-bindings 两侧自动同步）；轨道清点注释 24 → 31 条

## 阶段三：运行发起接线与硬编码退役

- [x] `packages/desktop/src-tauri/src/commands/exec/agent.rs`：删除 `DEFAULT_ENGINE` 常量；新增 `pub(crate) struct ResolvedEngine { kind: EngineKind, config: EngineConfig }` 与 `pub(crate) fn resolve_agent_engine(stores: &WorkspaceStores, agent: Option<i64>) -> Result<ResolvedEngine, String>`——`None` 走 `default_agent_instance`（无默认 `Err`：「未设置默认 agent：请前往 Agent 管理页（/agents）配置后再发起」）、`Some(id)` 走 `find_agent_instance`（miss `Err`）；sdk 臂经 `find_agent_provider` 组装 `EngineConfig { api_key, base_url, model: models.high }`（provider 缺失兜底 `Err`），cli 臂 `EngineConfig::empty()`；`AgentEngineKind` → `EngineKind` 两臂映射收此单点
- [x] `packages/desktop/src-tauri/src/commands/exec/agent.rs`：`start_agent_run` 尾参 `engine: EngineKind` → `resolved: ResolvedEngine`，runner 构造改 `runner_for(resolved.kind, resolved.config)`；`start_agent_run_with` / `drive_agent_run` / `RunStopRegistry` 零改动；文档注释同步（换源承诺兑现与「编排层零改动」边界）
- [x] `packages/desktop/src-tauri/src/commands/exec/mod.rs`：`agent_start` 尾参 `engine: Option<EngineKind>` → `agent: Option<i64>`；`unwrap_or(DEFAULT_ENGINE)` 消费点换为 `agent::resolve_agent_engine(stores.inner(), agent)?`（解析 `Err` reject 前端）；`EngineKind` import 清除；命令文档注释同步（缺省 = 解析默认 agent、显式 = 调试页选择）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs`：`from_hardcoded_slot()` 删除，新增 `EngineConfig::empty()`（三字段全空，CLI 臂不消费占位）；模块与类型文档改写为「构造源 = 命令层管理数据解析（默认 agent / 显式 agent）」并落机密标记注释；字段面、`is_complete` 与 `PartialEq` 比较语义零变化

## 阶段四：bindings 再生成

- [x] `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`，核对七新命令、`agent_start` 尾参 `agent?: number | null` 与 `AgentProviderRecord` / `AgentInstanceRecord` / `AgentModelTiers` / `AgentEngineKind` 类型出线；`src-tauri/src/bindings/mod_test.rs` 快照随再生成同步（机械再生成，非手改用例）

## 阶段五：前端接线

- [x] `packages/desktop/src/views/agents/hooks/use-agent-providers.ts`（新）：`useAgentProviders(): AgentProvidersState`——挂载 invoke `listAgentProviders` 一次（无轮询），`save` / `remove` 动作轨道失败 toast 固定前缀、清单加载失败 inline error 态，动作成功刷新清单（沿 use-workspaces 双轨错误呈现）
- [x] `packages/desktop/src/views/agents/hooks/use-agent-instances.ts`（新）：`useAgentInstances(): AgentInstancesState`——同上 + `setDefault` 动作（标记即切换，成功刷新清单）
- [x] `packages/desktop/src/views/agents/components/masked-api-key.tsx`（新）：`MaskedApiKey({ apiKey })`——`sk-***` + 末 3 字符，空 / 过短恒 `sk-***`；机密标记注释落位
- [x] `packages/desktop/src/views/agents/components/provider-panel.tsx`（新）：Providers 栏——清单（name / base_url / 三档 model / `MaskedApiKey`）、新建 / 编辑表单（api_key 编辑态恒空 + 遮蔽占位「留空保持原值」）、删除（引用阻止 `Err` 经 error 呈现、清单不变）
- [x] `packages/desktop/src/views/agents/components/agent-panel.tsx`（新）：Agents 栏——清单（name / engine / 引用 provider 名 / 默认标记）、新建 / 编辑表单（engine 二值下拉 + provider 选择，sdk 未选 provider 前端禁提交）、默认标记切换（`setDefault`）、删除
- [x] `packages/desktop/src/views/agents/agents-view.tsx`（新）：`AgentsView()` 两栏骨架挂两 panel（`data-testid="agents-view"`）；不依赖 workspace root、不触发任何 workspace 命令
- [x] `packages/desktop/src/views/agent/hooks/use-agent-options.ts`（新）：`useAgentOptions(): AgentOptionsState`——挂载 invoke `listAgentInstances` 一次，派生 `defaultId`（`is_default` 记录 id 或 null）
- [x] `packages/desktop/src/views/agent/components/agent-run-form.tsx`：`AgentStartInput` 字段 `engine: EngineKind` → `agent: number | null`；`ENGINE_OPTIONS` 与 engine state 退役 → agent 选择器（`ModeSelect` 同型，`data-testid="agent-select"`，选项 = 实例清单，默认选中 `defaultId`，空清单可发起走后端缺省解析报错横幅）
- [x] `packages/desktop/src/views/agent/agent-debug-view.tsx`：挂 `useAgentOptions()`，透传 `agents` 给 `AgentRunForm`；文档注释同步
- [x] `packages/desktop/src/lib/agent-transport.ts`：`AgentStartChainParams.engine` → `agent`（`AgentStartArgs[8]` 派生随再生成更新）；`readEngine` → `readAgentId`（`number | null` 校验）；invoke 尾参同步
- [x] `packages/desktop/src/hooks/use-agent-chat.ts`：`AgentChatSendInput` 字段 `engine?` → `agent?`；body 键 `engine:` → `agent:`；`EngineKind` import 清除（explore 链不传 agent，缺省语义零改动）
- [x] `packages/desktop/src/routes.tsx`：`/agents` 路由 → `<AgentsView />`（不传 root）
- [x] `packages/desktop/src/components/app-sidebar.tsx`：`SystemToolsGroup` 新增 [Agent 管理]（`data-testid="nav-agents"`，`active={pathname === '/agents'}`，lucide `Boxes`），组内排序 [Agent 管理] [Agent 调试] [数据库]

## 阶段六：版本与守线（静态，不含测试执行）

- [x] `packages/desktop/package.json`：`version` 0.3.12 → 0.3.13（用户可见行为变更：新增 Agent 管理页 + 调试页发起面演进）
- [x] 守线（静态）：`pnpm -C packages/desktop run server:check`（cargo fmt + cargo clippy 零告警）
- [x] 守线（静态）：`pnpm -C packages/desktop run client:check`（vp check --fix + knip 零未用导出）
- [x] 守线（静态）：`pnpm -C packages/desktop run bindings:check`（生成物零 diff 边界检查）

以上守线均为静态检查；测试执行（全管线）由 test-execution 阶段承接。
