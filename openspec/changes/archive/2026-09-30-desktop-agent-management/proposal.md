# 提案: desktop-agent-management

> **变更**: desktop-agent-management
> **日期**: 2026-09-30
> **状态**: draft

---

## 问题

Desktop 的 agent 执行面当前有三处显式预留的"待决座位"（代码注释明言后续迭代），恰好构成本需求要填的坑：

1. **SDK 引擎连接配置硬编码空占位**：`EngineConfig::from_hardcoded_slot()`（`crates/infra/agent/src/sdk/config.rs`）三字段（api_key / base_url / model）全空，用户须手填源码才能跑 SDK 引擎；
2. **默认引擎硬编码**：`DEFAULT_ENGINE = EngineKind::Sdk` 常量（`commands/exec/agent.rs`），`agent_start` 缺省收敛取值，无用户可调入口；
3. **user 维度租户预留**：全局库 `desktop-global.redb` 现仅注册 `WorkspaceRecord`，`DbDimension::User` 注释预留"及未来 user 维度租户"。

需求原文：【Desktop】增加全局 agent 管理页，不关联工作区；包含两类实例 provider 与 agent；每个 agent 可自定义名称、选择 agent engine 与 provider；provider 支持配置 url、api_key、model（high / medium / low）参数；将某个 agent 标记成默认，workspace 先使用默认，后续改造 workspace 关联 agent。

---

## 提案

新增**全局 Agent 管理能力**（user 维度，落全局库，天然不关联 workspace）：

- **两类实例**：`AgentProvider`（name 唯一 / base_url / api_key / models 三档 high·medium·low）与 `AgentInstance`（name 唯一 / engine `cli|sdk` / provider 引用——sdk 必填 cli 可空 / 默认标记全局至多一）。均以稳定 id 为主键、name 走唯一二级索引，为后续 workspace→agent 关联预留引用锚点。
- **管理页**：路由 `/agents` 单页两栏（Providers + Agents），侧栏「系统工具」组新增入口；CRUD 表单、默认标记切换、api_key 前端展示遮蔽（`sk-***abc` 形态，留空 = 保持原值）。
- **命令面**：新增全局轨管理命令（CRUD + 默认标记，`Result<T, String>` 既有模板）；删除语义拍板——删被 agent 引用的 provider 阻止报错不级联，删默认 agent 清空标记后删除。
- **运行发起接线**：解析收在命令层单点——缺省（explore 页等）解析默认 agent，调试页显式选 agent；解析产物 `agent.engine → EngineKind`，sdk 时由引用 provider 组装 `EngineConfig`（model 固定取三档中的 high 档，本期只存不选）。`runner_for` 签名、`EngineConfig` 三字段结构体与编排层**零改动**（`desktop-agent-execution` 既有"换源零改动"承诺就此兑现）；调试页 engine 下拉同步换为 agent 选择器。
- **机密边界**：api_key 明文过 IPC 入全局库、后端全链路明文读、遮蔽只在前端展示层（D2 修订）；`AgentProviderRecord` 破例手写遮蔽 Debug（结构保证明文不进日志），机密边界表锚定本 change 的 spec（长存），放宽点带统一可 grep 注释标记。

无默认 agent 时运行发起显式报错引导管理页，MUST NOT 静默回退硬编码引擎；`crates/core/agent` 全程零触碰。

---

## 能力

### 新增能力

- **desktop-agent-management** — 全局 agent 管理：provider / agent 两类实例的持久化与 CRUD 命令面、默认标记与运行发起解析、`/agents` 管理页、api_key 机密边界治理。

### 修改的能力

- **desktop-agent-execution** — `agent_start` 参数面演进：`engine: Option<EngineKind>` 直选 + `DEFAULT_ENGINE` 硬编码缺省退役，换为 agent 选择（缺省 = 解析默认 agent）；engine_cfg 构造源从硬编码位换为管理数据解析；调试页 engine 下拉换为 agent 选择器；SDK 租户"engine_cfg 硬编码位"限制偿还。
- **desktop-workspace-store** — 全局库模型组追加注册 `AgentProviderRecord` / `AgentInstanceRecord`；新增管理记录模型与操作面（唯一 name、默认标记唯一、引用删除阻止、存量库 additive 兼容）。
- **desktop-page-routing** — 路由表新增 `/agents`；侧栏「系统工具」组新增 [Agent 管理] 入口（`nav-agents`）。
- **desktop-data-dimensions** — user 维度新增已落地代表（agent 管理配置），维度声明留痕。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — 新增 `AgentProviderRecord` / `AgentInstanceRecord`（native_model 新 id，既有 1–4 已占用，分配 design 定稿；provider 记录手写遮蔽 Debug）
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（及 `lib.rs` 公共导出）— `global_models()` 追加两模型注册；provider / agent CRUD 操作面与默认标记设置（唯一 name、默认唯一、引用删除阻止）
- `packages/desktop/src-tauri/src/commands/agents/`（新轨道，注册进命令 mod）— 管理命令薄包装（全局库实例路由，`Result<T, String>` 模板）
- `packages/desktop/src-tauri/src/commands/exec/mod.rs` / `agent.rs` — `agent_start` 参数面演进（engine 直选 → agent 选择，缺省解析默认 agent）；`DEFAULT_ENGINE` 常量与硬编码收敛点退役
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` — `from_hardcoded_slot()` 退役与机密面放宽注释标记（`EngineConfig` 结构体形态不动）
- `packages/desktop/src-tauri/src/bindings/` — 经 export-bindings 再生成（非手改），快照测试同步再生成
- `packages/desktop/src/routes.tsx` — `/agents` 路由
- `packages/desktop/src/components/app-sidebar.tsx` — 「系统工具」组新增 [Agent 管理] 入口
- `packages/desktop/src/views/agents/`（新）— 管理页（两栏清单、CRUD 表单、遮蔽展示组件）+ 就近取数 hooks
- `packages/desktop/src/views/agent/components/agent-run-form.tsx` — engine 下拉换为 agent 选择器（默认选中默认 agent）

### 测试文件

- `crates/infra/store/src/*_test.rs` — 模型回环读写、重名报错、默认标记唯一、引用删除阻止、存量全局库 additive 打开
- `src-tauri/src/commands/agents/mod_test.rs`（新）— CRUD 命令、校验错误、删除语义
- `src-tauri/src/commands/exec/mod_test.rs` / `agent_test.rs` — 缺省解析默认 agent、无默认显式报错、sdk 解析档位取值；`DEFAULT_ENGINE` 相关既有断言随退役更新
- `packages/desktop/src` 前端测试 — 管理页 CRUD 交互与遮蔽展示、路由 active、agent-run-form agent 选择器

### 删除文件

- 无（`DEFAULT_ENGINE` 常量与 `from_hardcoded_slot()` 构造随实现退役，属代码级删除，不涉整文件删除）

### 不要修改

- `crates/core/agent` 全 crate — 契约零触碰（`AgentRunParams` 无 agent / engine 字段；`AgentStartError` 不加变体——配置不齐走既有 `ConfigMissing`）
- `EngineFacade::runner_for` 签名与 `EngineConfig` 三字段结构体形态
- workspace 库模型注册清单与 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord` 三模型
- `tests/golden` golden wire 契约（冻结，不可重写）
- desktop-db-inspector 查看器代码（记录信封 API 自动覆盖新模型，零改动）
- store 的 legacy 迁移层（不得重新引入——"不做旧库兼容、零迁移"纪律不变）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `/agents` 路由与侧栏入口 | URL `/agents` 渲染管理页两栏（Providers / Agents）；侧栏「系统工具」组出现 [Agent 管理] 且 `/agents` 时 active（testid 可查） |
| AC-2 | provider CRUD | 管理页可新建 / 编辑 / 删除 provider；重名保存报 `Err`；api_key 编辑框留空提交后原值不变 |
| AC-3 | agent CRUD | 可新建 / 编辑 / 删除 agent；`sdk` 引擎未选 provider 时保存报错；`cli` 引擎 provider 可空 |
| AC-4 | 默认标记 | 标记默认后全局恰一个；切换默认时旧标记自动清除 |
| AC-5 | 删除语义 | 删被 agent 引用的 provider 返回 `Err` 且两类记录均不消失；删默认 agent 后记录删除且默认标记消失 |
| AC-6 | 三档模型存储与消费 | provider 记录含 `models {high, medium, low}`；sdk 运行解析的 `EngineConfig.model` 取 high 档 |
| AC-7 | 缺省运行解析 | 不带显式 agent 的发起（explore 缺省路径）解析默认 agent 组装 runner；无默认 agent 返回 `Err` 引导管理页；`crates/core/agent` 无 agent / engine 字样，`runner_for` 签名与编排层零改动 |
| AC-8 | 调试页 agent 选择器 | 调试页 run form 出现 agent 选择器且默认选中默认 agent；选择 sdk agent 发起走 SDK 引擎（时间线 / 落库 / 重放组件零改动复用） |
| AC-9 | 机密边界 | `AgentProviderRecord` 无 `derive(Debug)` 且手写遮蔽 impl；统一标记注释可 grep（固定 token）；前端列表 / 详情呈现 `sk-***abc` 形态；错误串无 api_key 明文 |
| AC-10 | 存量库 additive | 既有 `desktop-global.redb` 打开成功，`WorkspaceRecord` 原样可读，两新模型可写入读出；store 无迁移代码 |
| AC-11 | 信封零改动覆盖 | db-inspector 全局库清单出现两新模型并可分页扫描（查看器零改动） |
| AC-12 | 全管线 | `cargo test --workspace` 全绿；`pnpm -C packages/desktop run client:check` 与 `run test` 全绿 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| native_db 对存量全局库 additive 加模型未经验证 | 打不开全局库 = 管理与 workspace 注册全不可用 | 中 | 实现首步以存量 `desktop-global.redb` 真机验证（探索已留验证手段）；必要时读文档确认 additive 打开语义；不做迁移层 |
| api_key 机密面无意识扩散 | 凭据进日志 / git / 错误串 | 低 | 手写遮蔽 Debug（结构保证）+ 统一注释标记（可 grep 意识化）+ 机密边界表锚定 spec 三层防线 |
| `agent_start` 参数面演进波及既有调用方 | explore / 调试页发起回归破坏 | 低 | explore 缺省路径零改动自动获得默认解析（回归测试锚定）；无默认 agent 的显式报错口径单测 |
| native_db 唯一二级索引支持度未知 | 唯一约束实现方式返工 | 中 | design 阶段 spike 先行；不支持则退事务内查重校验（口径一致） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| model 三档消费（D1） | 本期只存不选，消费固定取 high 档 | effort 进 run 参数 + 前端档位选择器为后续迭代，不进本期 | 本期加 effort 参数与档位选择器 |
| api_key 遮蔽层（D2 修订） | 后端全链路明文，遮蔽只在前端展示层 | 单用户桌面应用风险可控；读写单 DTO，免遮蔽信封臂 | 后端遮蔽 DTO / 序列化层遮蔽（已否决——遮蔽即丢真值）/ DPAPI 加密（范围外） |
| 调试页发起面（D3） | engine 下拉换为 agent 选择器，`agent_start` 参数面同步演进 | 发起面与配置面同步收敛，避免引擎直选与 agent 配置两套语义并存 | 保留引擎下拉（割裂默认解析语义） |
| 主键策略（D4） | 稳定 id 主键 + name 唯一二级索引 | 后续 workspace→agent 关联引用 id，实体改名不破引用 | name 作主键（改名破引用） |
| 删除语义（D5） | 删被引用 provider 阻止报错；删默认 agent 清标记 | 不级联不静默；无默认时显式报错引导管理页 | 级联删除 / 默认顺延 |
| 实体命名（D6） | `AgentProvider` / `AgentInstance`（UI 文案 Provider / Agent） | 呼应需求"两类实例"；回避 `agent` 契约 crate 与 `AgentRunRecord` 撞名 | `AgentConfig` 等 |
| Debug 处置（D7） | `AgentProviderRecord` 破例手写遮蔽 Debug | 现"明文不进日志"靠日志不存在（过程保证）；遮蔽 Debug ~5 行换成结构保证 | `derive(Debug)`（明文可泄漏） |
| 机密意识化载体（D8） | 机密边界表锚定本 change 的 spec + 统一注释标记 | spec 长存（design 随归档）；标记解决误判误修，遮蔽 Debug 解决无意识扩散，互补 | 锚定 design.md（随归档失效） |

### 待决问题

- model 消费档位是否最终取 high（design 可调）
- native_db 唯一二级索引支持度（design spike；不支持退事务内查重）
- `agent_start` 的 agent 参数具体形态（agent id 直传 vs 仅缺省语义，design 定稿）
- store CRUD API 签名与管理命令命名（design 定稿）
- R1 存量全局库 additive 打开验证（实现阶段真机执行并留痕）

---
