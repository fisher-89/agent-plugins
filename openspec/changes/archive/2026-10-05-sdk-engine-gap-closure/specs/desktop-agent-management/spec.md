# desktop-agent-management（delta：provider context_length 列）

## MODIFIED Requirements

### Requirement: 全局 agent 管理命令面

命令层 SHALL 新增全局 agent 管理命令轨道（命名与文件分组由 design 定稿，落 `commands/` 全局轨，经 `WorkspaceStores::global()` 全局库实例执行，MUST NOT 误路由 workspace 库），承载两类实例的 CRUD 与默认标记：

- **provider 实例**：新增 / 更新 / 删除 / 清单。name SHALL 全局唯一（重名保存 SHALL 以 `Err` 报错）；base_url、api_key、models 三档（high / medium / low）、**可空 `context_length`**（provider 上下文窗长，token 数；sdk 引擎上下文防线消费，缺席走 128K 缺省启发式——desktop-agent-execution「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）随记录保存；`context_length` 编辑语义 SHALL 为留空 = 未配置（存 `None`，MUST NOT 落 0 或把缺省启发式字面写库）；api_key 编辑语义 SHALL 为留空 = 保持原值不变（前端遮蔽占位配套，见「Agent 管理页」）；
- **agent 实例**：新增 / 更新 / 删除 / 清单与默认标记。name 全局唯一；engine 二值（`cli` | `sdk`）；engine 为 `sdk` 时 SHALL 必选 provider 引用（缺失 SHALL 报错），engine 为 `cli` 时 provider 引用 SHALL 可空（CLI 引擎不消费 provider，避免"必须选 provider 但引擎不用"）；
- **删除语义**：删被 agent 引用的 provider SHALL 阻止并报错（错误信息提示引用方，MUST NOT 级联删除）；删默认 agent SHALL 清空默认标记后删除（默认不顺延到其他 agent），MUST NOT 报错。

错误约定沿用既有 `Result<T, String>` 模板（store `StoreError` MUST NOT 进入命令签名）；校验失败与 db 读写失败 MUST NOT 被静默吞掉。`context_length` 的消费接线 MUST NOT 经 `EngineConfig`（三字段结构不动承诺维持），抵达 sdk 引擎的路径由 design 定稿。

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

#### Scenario: context_length 携带与留空语义

- **WHEN** 保存 provider 携带 `context_length = 200000`，随后读取清单
- **THEN** 记录携带该值原样返回；再次保存留空提交
- **AND** 记录的 `context_length` 为未配置（`None`），而非 0 或 128000 字面值

### Requirement: Agent 管理页

前端 SHALL 提供全局 Agent 管理页（路由 `/agents`，侧栏「系统工具」组新增入口，`data-testid="nav-agents"`，命名 design 可调）：

- **单页两栏**：Providers 清单 + Agents 清单，各自承载新建 / 编辑 / 删除交互；Agents 栏另承载默认标记切换（标记即切换，至多一个默认）；Providers 栏编辑表单 SHALL 增可空 `context_length` 数字字段（留空 = 未配置，跟随缺省启发式的语义提示在场）；
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

#### Scenario: context_length 编辑与清空

- **WHEN** 在 provider 编辑表单填 `context_length = 200000` 保存后重开编辑态
- **THEN** 字段显示 200000；清空该字段再保存
- **AND** 记录回到未配置态，表单呈现「跟随缺省」语义而非 0

#### Scenario: 删除阻止错误呈现

- **WHEN** 在管理页删除仍被 agent 引用的 provider
- **THEN** 页面以 error 态呈现引用阻止信息，Providers 清单不变

#### Scenario: 取数无轮询

- **WHEN** 停留在管理页不进行任何操作
- **THEN** 无定时取数发生；新建 / 编辑 / 删除等显式动作后清单刷新

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | provider 记录加列 | `AgentProviderRecord.context_length: Option<…>`（可空；serde 缺省读兼容旧记录）；envelope 版本演进沿既有惯例（不做 legacy 迁移层） |
| `crates/infra/store` 操作面 + 信封 | 读写携带 | provider CRUD 原样携带可空列；`None` = 未配置语义贯穿存储层 |
| `src/commands/agents/` | 管理命令面 | 保存 / 清单 DTO 携带 `context_length`；留空提交 = `None`；`Result<T, String>` 模板与既有校验语义不变 |
| `packages/desktop/src/views/agents/` | 管理页 Providers 栏 | `context_length` 数字字段（留空 = 未配置 + 缺省提示）；取数 hooks 显式刷新不变 |
| `src/bindings/` | IPC 类型镜像 | 管理 DTO 演进经 export-bindings 再生成，非手改；快照测试同步再生成 |
| `EngineConfig`（core 组合根） | 本变更零 diff | 三字段结构与 `runner_for` 签名不动；`context_length` 抵达 sdk 引擎的接线归 design（不经 `EngineConfig`） |
