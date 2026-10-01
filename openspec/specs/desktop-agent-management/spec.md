# desktop-agent-management Specification

## Purpose

定义全局 agent 管理能力：provider / agent 实例的 CRUD 与默认标记命令面、运行发起的引擎与连接配置解析单点、Agent 管理页（`/agents` 两栏视图），以及 api_key 机密边界治理与本期显式边界留痕。

## Requirements

### Requirement: 全局 agent 管理命令面

命令层 SHALL 新增全局 agent 管理命令轨道（命名与文件分组由 design 定稿，落 `commands/` 全局轨，经 `WorkspaceStores::global()` 全局库实例执行，MUST NOT 误路由 workspace 库），承载两类实例的 CRUD 与默认标记：

- **provider 实例**：新增 / 更新 / 删除 / 清单。name SHALL 全局唯一（重名保存 SHALL 以 `Err` 报错）；base_url、api_key、models 三档（high / medium / low）随记录保存；api_key 编辑语义 SHALL 为留空 = 保持原值不变（前端遮蔽占位配套，见「Agent 管理页」）；
- **agent 实例**：新增 / 更新 / 删除 / 清单与默认标记。name 全局唯一；engine 二值（`cli` | `sdk`）；engine 为 `sdk` 时 SHALL 必选 provider 引用（缺失 SHALL 报错），engine 为 `cli` 时 provider 引用 SHALL 可空（CLI 引擎不消费 provider，避免"必须选 provider 但引擎不用"）；
- **删除语义**：删被 agent 引用的 provider SHALL 阻止并报错（错误信息提示引用方，MUST NOT 级联删除）；删默认 agent SHALL 清空默认标记后删除（默认不顺延到其他 agent），MUST NOT 报错。

错误约定沿用既有 `Result<T, String>` 模板（store `StoreError` MUST NOT 进入命令签名）；校验失败与 db 读写失败 MUST NOT 被静默吞掉。

#### Scenario: provider 重名报错

- **WHEN** 以已有 provider 的 name 再次保存 provider
- **THEN** 命令返回 `Err` 且指明重名，库中 provider 记录数量与内容不变

#### Scenario: sdk 必选 provider

- **WHEN** 保存 engine 为 `sdk` 且未选 provider 的 agent
- **THEN** 保存返回 `Err`；engine 为 `cli` 且 provider 为空时保存成功

#### Scenario: 删被引用 provider 阻止

- **WHEN** 删除一个仍被某 agent 引用的 provider
- **THEN** 命令返回 `Err` 且错误信息含引用方提示，provider 与引用它的 agent 记录均原样保留（无级联）

#### Scenario: 删默认 agent 清标记

- **WHEN** 删除被标记为默认的 agent
- **THEN** 删除成功且默认标记消失（无其他 agent 被顺延标记）；此后缺省运行发起显式报错（见「默认 agent 与运行发起解析」）

### Requirement: 默认 agent 与运行发起解析

agent 实例 SHALL 支持「默认」标记，全局至多一个；标记新默认 SHALL 原子清除旧默认（切换语义，无需先手动取消）。

运行发起的引擎与连接配置 SHALL 由**内核组合根**单点解析（desktop-agent-execution 会话内核落地，解析点自命令层下沉——解析语义与消费面承诺不变）：

- **缺省路径**（explore 页等一切不显式指定 agent 的发起）：解析默认 agent；无默认 agent SHALL 以 `Err` 显式失败并提示前往管理页配置，MUST NOT 静默回退硬编码引擎或空配置；
- **显式路径**（调试页 agent 选择器）：按所选 agent 解析；
- **解析产物**：`agent.engine` → `EngineKind`（`cli` | `sdk`）；engine 为 `sdk` 时由引用 provider 组装 `EngineConfig{api_key, base_url, model}`，model 固定取 provider 三档中的 **high** 档（档位可调点由 design 定稿）；engine 为 `cli` 时不消费 `EngineConfig`（门面既有约定不变）。

解析 SHALL 收在内核组合根单点：`EngineFacade::runner_for` 签名、`EngineConfig` 三字段结构体 MUST NOT 变化（既有换源零改动承诺平移为「换解析落点零改动」）；命令层 SHALL 无引擎/agent 解析残留。`crates/core/agent` MUST NOT 出现 agent / engine 概念：协议参数面无 agent 字段，`AgentStartError` 不加变体（连接配置不齐走既有 `ConfigMissing` 显式启动失败）。

#### Scenario: 缺省解析默认 agent

- **WHEN** explore 页等不显式指定 agent 发起运行，且存在已标记默认的 sdk agent
- **THEN** 内核组合根解析该 agent 并经门面构造 SDK runner（`EngineConfig` 来自其引用 provider，model 取 high 档），发起路径与显式选择该 agent 完全一致

#### Scenario: 无默认显式报错

- **WHEN** 无默认 agent 时以缺省路径发起运行
- **THEN** 命令返回 `Err` 且提示前往管理页配置，不产生会话/轮记录、不推流、不静默回退

#### Scenario: cli 默认 agent 解析

- **WHEN** 默认 agent 为 `cli` 引擎时以缺省路径发起运行
- **THEN** 走 CLI 引擎且 provider 引用不被消费（可为空）；CLI 不可发现时按既有 `CliMissing` 口径显式失败

#### Scenario: 解析点下沉零改动

- **WHEN** 审查下沉后的组合根解析实现、命令层与 `crates/core/agent` 源码
- **THEN** 解析语义与下沉前一致（默认/显式/sdk 组装/high 档/无默认 `Err`），`runner_for` 签名与 `EngineConfig` 结构体零 diff、命令层无解析残留；core 无 agent / engine 字样且 `AgentStartError` 变体集不变

### Requirement: Agent 管理页

前端 SHALL 提供全局 Agent 管理页（路由 `/agents`，侧栏「系统工具」组新增入口，`data-testid="nav-agents"`，命名 design 可调）：

- **单页两栏**：Providers 清单 + Agents 清单，各自承载新建 / 编辑 / 删除交互；Agents 栏另承载默认标记切换（标记即切换，至多一个默认）；
- **api_key 遮蔽展示**：列表与详情 SHALL 呈现遮蔽形态（`sk-***abc`）；编辑态输入框 SHALL 展示遮蔽占位，留空提交 = 保持原值不变；
- **错误呈现**：删除被引用 provider、保存重名、无默认发起等错误 SHALL 以既有 error 态呈现，MUST NOT 静默；
- **取数模型**：清单经取数 hooks 收口、显式动作触发，MUST NOT 轮询（管理数据无文件 watch 例外）。

页面 SHALL 不依赖当前 workspace root（全局语义，壳态下任意时刻可用）；页面为纯内容页，MUST NOT 触发任何 workspace 命令。

#### Scenario: 路由与入口 active

- **WHEN** URL 为 `/agents`
- **THEN** 渲染管理页两栏视图，侧栏 `nav-agents` 呈现 active 态，`nav-agent` / `nav-db` 非 active

#### Scenario: 遮蔽与留空保持

- **WHEN** 编辑既有 provider 且 api_key 输入框留空提交
- **THEN** 保存成功且记录的 api_key 保持原值；列表中该 provider 的 key 呈现 `sk-***abc` 遮蔽形态而非明文

#### Scenario: 删除阻止错误呈现

- **WHEN** 在管理页删除仍被 agent 引用的 provider
- **THEN** 页面以 error 态呈现引用阻止信息，Providers 清单不变

#### Scenario: 取数无轮询

- **WHEN** 停留在管理页不进行任何操作
- **THEN** 无定时取数发生；新建 / 编辑 / 删除等显式动作后清单刷新

### Requirement: api_key 机密边界

api_key SHALL 按「后端全链路明文 + 前端展示遮蔽」的机密边界表治理（单用户桌面应用裁决，2026-09-30）：

- **明文允许出现**：写向 IPC body、读向 IPC body、全局库文件、进程内存；
- **明文禁止出现**：`Debug` / 日志输出、git 追踪文件、错误串（含 `Err(String)` 返回面）。

`AgentProviderRecord` SHALL 破例不 derive `Debug`，手写遮蔽 Debug impl（api_key 位输出 `sk-***abc` 形态）——以结构保证明文不进日志，不依赖未来日志配置；key 的测试比较 SHALL 走 `PartialEq`（沿 `EngineConfig` 既有惯例）。机密边界 SHALL 以本 spec 为锚点（design.md 随 change 归档不承锚）；放宽点 SHALL 带统一可 grep 注释标记（固定 token）：`// 机密面有意放宽:…，边界表见 specs/desktop-agent-management`，放置于 `EngineConfig` 注释更新处、`AgentProviderRecord` 类型与 api_key 字段、读写 DTO 的 api_key 字段、前端遮蔽展示组件。

#### Scenario: 遮蔽 Debug

- **WHEN** 审查 `AgentProviderRecord` 类型定义并以含 api_key 的记录触发 `Debug` 格式化
- **THEN** 类型无 `derive(Debug)`、有手写 impl，输出中 api_key 位为 `sk-***abc` 形态且无明文；`assert_eq!` 失败信息与 `PartialEq` 比较行为不受影响

#### Scenario: 标记可 grep

- **WHEN** 以固定 token 检索放宽点注释
- **THEN** `EngineConfig` 注释、`AgentProviderRecord` 类型与 api_key 字段、读写 DTO 的 api_key 字段、前端遮蔽展示组件处标记在场，且各标记指向本 spec 边界表

#### Scenario: 错误串无明文

- **WHEN** 审查管理命令与运行发起的错误返回路径
- **THEN** `Err(String)` 内容不含 api_key 明文（校验与 db 错误信息不拼接机密字段）

### Requirement: 本期边界与后续路线

以下边界 SHALL 作为本期显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **model 三档只存不选**：provider 的 `models {high, medium, low}` 存储完整，消费固定取 high 档；`AgentRunParams` 增 effort 参数 + 前端档位选择器为后续迭代；
2. **workspace 关联 agent 后续改造**：届时运行解析顺序演变为 workspace 关联 → 全局默认；本期数据模型以稳定 id 主键预留支撑（关联引用 id，实体改名不破引用），MUST NOT 预建关联字段或解析分支；
3. **api_key 静态加密落盘（Windows DPAPI）不做**：单用户同机威胁模型下收益边际，未来有合规要求再启；
4. **provider 连通性测试按钮不做**（未要求）。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 四条边界均可考，后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store`（模型 + 操作面） | 管理记录持久化 | `AgentProviderRecord` / `AgentInstanceRecord` 落全局库；稳定 id 主键 + name 唯一二级索引；默认标记至多一（切换原子清旧）；引用删除阻止、删默认清标记；手写遮蔽 Debug（provider）；信封 API 零改动覆盖 |
| `commands/agents/`（新轨道） | 管理命令面 | provider / agent CRUD + 默认标记薄包装；全局库实例路由（`global()`）；`Result<T, String>` 模板；重名 / sdk 缺 provider / 引用删除均 `Err` |
| `commands/exec`（消费接线） | 运行发起命令面 | 引擎与连接配置解析已下沉内核组合根（desktop-agent-execution），命令层无引擎/agent 解析残留；`DEFAULT_ENGINE` 与 `from_hardcoded_slot()` 退役 |
| 内核组合根（落点见 desktop-agent-execution） | 运行发起解析单点（自 `commands/exec` 下沉） | 缺省解析默认 agent / 显式 agent → (`EngineKind`, `EngineConfig`)；model 取 high 档；无默认 `Err` 引导管理页；`runner_for` / `EngineConfig` 零 diff |
| `packages/desktop/src/views/agents/`（新） | 管理页 | `/agents` 两栏 CRUD + 默认标记 + api_key 遮蔽展示（留空保持原值）；hooks 取数收口、无轮询 |
| `packages/desktop/src/views/agent/components/agent-run-form.tsx` | 调试页发起面 | engine 下拉 → agent 选择器（默认选中默认 agent） |
| `packages/desktop/src-tauri/src/bindings/` | IPC 类型镜像 | 经 export-bindings 再生成（管理 DTO 与 agent 参数变化），非手改 |
