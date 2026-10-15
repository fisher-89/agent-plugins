# desktop-agent-management（delta：CLI 租户退役的产品面收窄）

## MODIFIED Requirements

### Requirement: 全局 agent 管理命令面

命令层 SHALL 新增全局 agent 管理命令轨道（命名与文件分组由 design 定稿，落 `commands/` 全局轨，经 `WorkspaceStores::global()` 全局库实例执行，MUST NOT 误路由 workspace 库），承载两类实例的 CRUD 与默认标记：

- **provider 实例**：新增 / 更新 / 删除 / 清单。name SHALL 全局唯一（重名保存 SHALL 以 `Err` 报错）；base_url、api_key、models 三档（high / medium / low）、**可空 `context_length`**（provider 上下文窗长，token 数；sdk 引擎上下文防线消费，缺席走 128K 缺省启发式——desktop-agent-execution「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）随记录保存；`context_length` 编辑语义 SHALL 为留空 = 未配置（存 `None`，MUST NOT 落 0 或把缺省启发式字面写库）；api_key 编辑语义 SHALL 为留空 = 保持原值不变（前端遮蔽占位配套，见「Agent 管理页」）；
- **agent 实例**：新增 / 更新 / 删除 / 清单与默认标记。name 全局唯一；engine 线值域 `cli` | `sdk` 保留（`cli` 为存量只读解码值——`AgentEngineKind::Cli` 变体是 bincode 解码锚，线值不演进），写入面 SHALL 仅接受 `engine: sdk`：携 `engine: Cli` 的保存（新建与更新整行同拒）SHALL 以 `Err("CLI 租户已下线…")` 拒绝，存量 cli 实例可读、可删、不可改；provider 引用 SHALL 必选（`sdk` 唯一活档，CLI 引擎不消费 provider 的「cli 可空」语义随 CLI 租户退役）；
- **删除语义**：删被 agent 引用的 provider SHALL 阻止并报错（错误信息提示引用方，MUST NOT 级联删除）；删默认 agent SHALL 清空默认标记后删除（默认不顺延到其他 agent），MUST NOT 报错。

错误约定沿用既有 `Result<T, String>` 模板（store `StoreError` MUST NOT 进入命令签名）；校验失败与 db 读写失败 MUST NOT 被静默吞掉。`context_length` 的消费接线 MUST NOT 经 `EngineConfig`（三字段结构不动承诺维持），抵达 sdk 引擎经 compose `ResolvedEngine` → `EngineFacade::with_context_window`。

#### Scenario: provider 重名报错

- **WHEN** 以已有 provider 的 name 再次保存 provider
- **THEN** 命令返回 `Err` 且指明重名，库中 provider 记录数量与内容不变

#### Scenario: sdk 必选 provider

- **WHEN** 保存 engine 为 `sdk` 且未选 provider 的 agent
- **THEN** 保存返回 `Err`；engine 为 `sdk` 且 provider 在选时保存成功

#### Scenario: cli 写入拒收

- **WHEN** 以 `engine: Cli` 新建 agent，或对存量 cli 实例发起更新（含改名、含改 engine 为 sdk 的整行更新）
- **THEN** 保存返回 `Err("CLI 租户已下线…")`，库中记录不变；存量 cli 实例仍可读取、可删除（「可读可删不可改」）

#### Scenario: 删被引用 provider 阻止

- **WHEN** 删除一个仍被某 agent 引用的 provider
- **THEN** 命令返回 `Err` 且错误信息含引用方提示，provider 与引用它的 agent 记录均原样保留（无级联）

#### Scenario: 删默认 agent 清标记

- **WHEN** 删除被标记为默认的 agent
- **THEN** 删除成功且默认标记消失（无其他 agent 被顺延标记）；此后缺省运行发起显式报错（见「默认 agent 与运行发起解析」）

#### Scenario: context_length 携带与留空语义

- **WHEN** 保存 provider 携带 `context_length = 200000`，随后读取清单
- **THEN** 记录携带该值原样返回；再次保存留空提交
- **AND** 记录的 `context_length` 为未配置（`None`），而非 0 或 128000 字面值

### Requirement: 默认 agent 与运行发起解析

agent 实例 SHALL 支持「默认」标记，全局至多一个；标记新默认 SHALL 原子清除旧默认（切换语义，无需先手动取消）。默认标记 SHALL 仅可落在 `sdk` 实例上：对存量 `cli` 实例设默认 SHALL 以 `Err("CLI 租户已下线…")` 拒绝（防造出必失败的缺省发起路径）。

运行发起的引擎与连接配置 SHALL 由**内核组合根**单点解析（desktop-agent-execution 会话内核落地，解析点自命令层下沉——解析语义与消费面承诺不变）：

- **缺省路径**（explore 页等一切不显式指定 agent 的发起）：解析默认 agent；无默认 agent SHALL 以 `Err` 显式失败并提示前往管理页配置，MUST NOT 静默回退硬编码引擎或空配置；默认实例 `engine` 为 `cli`（存量态）时同 SHALL 显式 `Err("CLI 租户已下线…")` 并提示管理页改默认（缺省发起不迁数据、不清 `is_default`——改标 sdk 默认经切换语义原子清旧）；
- **显式路径**（调试页 agent 选择器）：按所选 agent 解析；所选实例为存量 `cli` 实例时 SHALL 显式 `Err("CLI 租户已下线…")`，MUST NOT 静默降级；
- **解析产物**：`agent.engine` → `EngineKind`（值域 `cli` | `sdk` 保留为多租户协议预留，`Cli` 为退役态）；engine 为 `sdk` 时由引用 provider 组装 `EngineConfig{api_key, base_url, model}`，model 固定取 provider 三档中的 **high** 档（档位可调点由 design 定稿）。

解析 SHALL 收在内核组合根单点：`EngineConfig` 三字段结构体 MUST NOT 变化（既有换源零改动承诺平移为「换解析落点零改动」）；`EngineFacade::runner_for` 唯一 match 点结构 MUST NOT 塌（`EngineKind::Cli` 臂显式 `Err("CLI 租户已下线")`，签名形态由 design 定稿）；命令层 SHALL 无引擎/agent 解析残留。`crates/core/agent` MUST NOT 出现 agent / engine 概念：协议参数面无 agent 字段，`AgentStartError` 不加变体（连接配置不齐走既有 `ConfigMissing` 显式启动失败）。

#### Scenario: 缺省解析默认 agent

- **WHEN** explore 页等不显式指定 agent 发起运行，且存在已标记默认的 sdk agent
- **THEN** 内核组合根解析该 agent 并经门面构造 SDK runner（`EngineConfig` 来自其引用 provider，model 取 high 档），发起路径与显式选择该 agent 完全一致

#### Scenario: 无默认显式报错

- **WHEN** 无默认 agent 时以缺省路径发起运行
- **THEN** 命令返回 `Err` 且提示前往管理页配置，不产生会话/轮记录、不推流、不静默回退

#### Scenario: cli 默认 agent 解析

- **WHEN** 默认 agent 为存量 `cli` 实例时以缺省路径发起运行
- **THEN** 显式 `Err("CLI 租户已下线…")`（文案提示管理页改默认或新建 sdk 实例），不产生会话/轮记录、不推流；用户改标 sdk 默认后缺省发起恢复（切换语义原子清旧默认），全程零数据迁移脚本

#### Scenario: cli 实例设默认拒收

- **WHEN** 对存量 cli 实例 invoke 设默认命令
- **THEN** 命令返回 `Err("CLI 租户已下线…")`，默认标记分布不变；对 sdk 实例设默认照常（切换语义原子清旧默认）

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现、命令层与 `crates/core/agent` 源码
- **THEN** 解析语义与设计一致（默认/显式/sdk 组装/high 档/无默认 `Err`/cli 显式 `Err`），`runner_for` 唯一 match 点结构与 `EngineConfig` 结构体零塌、命令层无解析残留；core 无 agent / engine 字样且 `AgentStartError` 变体集不变

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `commands/agents/`（既有轨道，收窄） | 管理命令面 | save / set_default 对 `engine: Cli` 显式 `Err`（存量行可读可删不可改）；provider 引用恒必选（`sdk` 唯一活档）；`Result<T, String>` 模板不变 |
| `crates/infra/store`（写面校验单点） | 拒 Cli 校验落点 | `upsert_agent_instance` / `set_default_agent_instance` 拒 `engine: Cli`（校验在 store 单点，命令面透传）；`AgentEngineKind` 线值 `("cli" \| "sdk")` 不演进、参数类型不收窄（校验层拒绝） |
| 内核组合根（落点见 desktop-agent-execution） | 运行发起解析单点 | 缺省/显式路径遇 `cli` 实例显式 `Err("CLI 租户已下线…")`；无默认 `Err` 引导管理页；`EngineConfig` 零 diff |
| `packages/desktop/src/views/agents/`（产品面收窄） | 管理页 | engine 选项仅 `sdk`（下拉去 'cli'）；`canSave` 的 cli 分支退役；provider 选择器恒启用必选；存量 cli 行只读可删（保存/设默认被拒的错误沿既有 error 态呈现） |
| `packages/desktop/src-tauri/src/bindings/` | IPC 类型镜像 | 经 export-bindings 再生成（注释级 diff 预期），非手改 |
