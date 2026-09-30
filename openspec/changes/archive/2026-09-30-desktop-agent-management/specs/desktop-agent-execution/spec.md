# desktop-agent-execution 变更规格

## MODIFIED Requirements

### Requirement: 引擎门面与 EngineKind 参数选择

`agent-runtime` SHALL 作为 agent 引擎门面：对外一套应用协议（invoke 面 + 事件流 + store 记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎；协议统一 SHALL 落在应用协议层而非 core trait 层（core 契约除 `AgentStartError` 加法变体外零改动）。门面 SHALL 提供 `runner_for(kind, engine_cfg)` 形状的构造入口——`EngineConfig` 结构体形态（api_key / base_url / model 三件套）由壳层注入，SDK 引擎用以组装 rig client；CLI 引擎不消费 engine_cfg。engine_cfg 构造源 SHALL 为命令层的管理数据解析（desktop-agent-management「默认 agent 与运行发起解析」：默认 agent / 显式 agent → `EngineKind` + `EngineConfig`，sdk 时由引用 provider 组装、model 取 provider 三档 high 档）；原 MVP 硬编码位（`EngineConfig::from_hardcoded_slot()` 与 `DEFAULT_ENGINE` 常量）SHALL 退役。换构造源时 `EngineConfig` 消费面 MUST NOT 变化（本承诺于 desktop-agent-management 兑现）。启动失败抹平：`AgentStartError` 的中性变体 `ConfigMissing` 承接 SDK 侧连接配置不齐（key 未配 / 模型不存在）等启动失败——core 唯一触碰点，变体集 MUST NOT 再增。

#### Scenario: 门面构造与 core 零污染

- **WHEN** 审查 `runner_for` 实现与 `crates/core/agent` 源码
- **THEN** 引擎分发唯一 match、返回 `AgentRunner` 实现；core 无 engine / `EngineConfig` / rig 概念，`AgentStartError` 变体集与换源前一致

#### Scenario: 构造源换为管理数据解析零改动

- **WHEN** 审查换源后的 engine_cfg 构造点与门面签名
- **THEN** 构造仅发生在命令层解析单点（默认 agent / 显式 agent → `EngineKind` + `EngineConfig`），硬编码位（`from_hardcoded_slot` / `DEFAULT_ENGINE`）无残留，`runner_for` 签名与 `EngineConfig` 消费面（SDK 引擎内部）零改动

### Requirement: agent 执行命令面

`commands/exec/` SHALL 承载执行与查询命令：

- `agent_start`：执行命令。body SHALL 维持三件事纪律（参数转换 → 调用 → 错误映射），编排 SHALL 收在 `run_agent()` 编排函数（组装 runner → 事件流 tee 双 sink → 状态收敛）；该函数与 `*_inner` 同列 app 层微形态（详见 desktop-app-shell）。命令 SHALL 增可选参数：`agent`（运行 agent 选择，**debug-only**——agent 选择 UI 仅调试页表单暴露，正式场景（explore 链等）无选择入口且 MUST NOT 传 agent；缺席时壳层缺省收敛为解析默认 agent（desktop-agent-management「默认 agent 与运行发起解析」：agent → `EngineKind` + `EngineConfig` 解析收在命令层单点，sdk 时由引用 provider 组装 EngineConfig、model 取 provider 三档 high 档；无默认 agent 显式 `Err` 引导管理页。原 `engine: Option<EngineKind>` 直选参数与 `DEFAULT_ENGINE` 硬编码缺省随本演进退役）。「参数选择 agent」语义落在命令面：解析单点在命令层，引擎接线（runner 构造、engine_cfg 消费）全在门面内消化，命令体 MUST NOT 膨胀）、`resume_session_id`（续会话）、`source`（来源受控字符串，缺省 `debug`）、`source_ref`（来源内定位）、`parent_run_id`（链上游 run）——编排 SHALL 将链参数写入 run 记录字段面；不传链参数时生成的记录 `source="debug"`、链字段为 `None`，字段面行为与演进前一致（返回时序为提前 resolve running 记录）。落库 SHALL 经 root 解析所属 workspace 库（`WorkspaceStores::for_root`，见 desktop-workspace-store），run 与事件写入当前 workspace 的库文件。runner 组装 SHALL 经门面 `runner_for(kind, engine_cfg)`——MUST NOT 在命令面直接构造具体引擎
- `agent_runs` / `agent_run_events` / `agent_run_chain`：查询薄包装（无状态，参数含 root → workspace 库解析 → store 查询 → DTO）；`agent_run_chain` 的 `source_ref` 为 workspace 库域内的 explore 记录 id，root 寻址与库域内 id 配套消解跨库歧义
- `agent_stop`：终止命令（寻址、`stopped` 收敛与引擎终止见「agent_stop 终止与提前 resolve」）；寻址 SHALL 携 root（run id 为 workspace 库域内自增，裸 id 跨库歧义由 root 消解），`RunStopRegistry` 寻址键随 root 演进

错误约定沿用既有模板 `Result<T, String>`：CLI 不可发现、spawn 失败、SDK 引擎配置缺失（经 `AgentStartError` 中性变体）、无默认 agent 可解析、workspace 库打开失败等 SHALL 以 `Err` 抵达前端，MUST NOT 静默吞掉。

#### Scenario: agent_start 编排收口

- **WHEN** 审查 `agent_start` 实现
- **THEN** 命令体为参数转换 + 调用 `run_agent()` + 错误映射三段，runner 组装经门面分发且 tee 在编排函数内，不膨胀命令体

#### Scenario: 来源与 resume 参数透传

- **WHEN** explore 页以 `source="explore"`、`source_ref=<记录键>`、`resume_session_id=<sid>` invoke `agent_start`
- **THEN** 生成 run 记录携带对应字段且以该会话续话（CLI 引擎组装 `--resume`、SDK 引擎走 store 转录重建）；调试页 invoke（不传这些参数）生成的记录 `source="debug"`、链字段为 `None`

#### Scenario: agent 缺省与解析

- **WHEN** invoke 不带 `agent` 参数（正式场景如 explore 链恒不传）
- **THEN** 壳层缺省解析默认 agent 并经门面构造对应引擎 runner（无默认 agent 时显式 `Err`，不落库不推流、不静默回退）；调试页显式传 agent 时按所选解析；`AgentRunParams` 全程无 agent / engine 字段（core 零污染）

#### Scenario: 查询与停止 root 寻址

- **WHEN** 前端 invoke `agent_runs` / `agent_run_events` / `agent_run_chain` / `agent_stop`
- **THEN** 命令无状态、按 root 解析所属 workspace 库直查/寻址返回 DTO，无领域解释；workspace A 的调用看不到 B 的 runs，同 id 并行时停止命中发起方所在库的 run

#### Scenario: 错误 reject 传达

- **WHEN** CLI 不可发现、SDK 引擎配置缺失、无默认 agent 可解析或所属 workspace 库打开失败时发起 `agent_start`
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

### Requirement: Agent 调试页

前端 SHALL 提供 Agent 调试页（`AgentDebugView`），经侧栏「系统工具」组进入；视图切换 SHALL 沿用本地 state，MUST NOT 引入路由。页面 SHALL 包含：

- **参数面（最小集）**：prompt（必填）、permission-mode 三档下拉（默认 bypassPermissions）、agent 选择器（管理页 agent 实例清单，默认选中默认 agent——与后端缺省解析一致；**仅调试页暴露**，正式场景无 agent 选择入口）；cwd 不设参数（隐含当前 workspace root）、model 不设参数（SDK 引擎取解析所得 engine_cfg，见「引擎门面与 EngineKind 参数选择」）、env 不设参数（运行恒为完整环境，见 CLI 租户约定）
- **事件时间线**：SHALL 经共享组件族 `AgentTimeline`（desktop-agent-chat-infra）以保真透镜呈现——seq 序、tool_use / tool_result 成对、子代理按 `parent_tool_use_id` 分组归因、result 汇总卡（num_turns / cost / duration / session_id 可复制）；两引擎共用同一时间线组件（SDK 引擎 `parent_tool_use_id` 恒空、`cost_usd` 恒 None，如实呈现）
- **运行中停止**：运行中 SHALL 呈现停止入口，触发 `agent_stop`（见「agent_stop 终止与提前 resolve」）
- **原始 JSONL 切换**：全部事件（含 Raw）的原文可见
- **历史运行**：run 列表 → 点开自 store 重放（invoke 查询）

页面状态 SHALL 经统一会话基建（`use-agent-chat`）承载；run 表单 / 历史列表 / JSONL 开关为页面级 chrome，包在共享核心外圈。实时流经 transport 走 Tauri Channel 订阅；该订阅属执行流通道（`agent_start` 命令作用域），不属于「刷新取数模型」所禁止的轮询取数；查询类取数（历史 run 列表 / 事件重放）仍由用户显式动作触发。

#### Scenario: 参数面默认值

- **WHEN** 打开调试页
- **THEN** permission-mode 默认 bypassPermissions、agent 选择器默认选中默认 agent（与后端缺省解析一致）、prompt 为空且必填；无 model 输入、无 cwd 输入、无 env 输入、无引擎直选项

#### Scenario: agent 切换

- **WHEN** agent 选择器选 sdk agent 发起一次含工具调用的运行
- **THEN** 运行走 SDK 引擎（连接配置来自所选 agent 的引用 provider），时间线 / 落库 / 重放组件零改动复用（切换其他 agent 不改任何组件代码）

#### Scenario: loop 可见性

- **WHEN** 一次含工具调用的运行完成
- **THEN** 时间线可按序看到 assistant(tool_use) → tool_result → … → result 汇总卡，num_turns / cost / duration / session_id 可读可复制，子代理消息归因到父工具调用

#### Scenario: 运行中停止

- **WHEN** 调试页运行中点击停止
- **THEN** 触发 `agent_stop`，事件流以终止终态收尾，run 状态收敛为 `stopped`

#### Scenario: 原始流切换

- **WHEN** 切换原始 JSONL 视图
- **THEN** 全部事件（含 Raw 透传件）原文可见，与落库事件一致

#### Scenario: 历史重放

- **WHEN** 从历史运行列表点开一条已完成 run
- **THEN** 经 invoke 查询以落库事件渲染完整时间线，不要求原运行进程存活

### Requirement: SDK 租户 MVP 边界与已知限制

以下边界 SHALL 作为 SDK 租户 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 MCP**：SDK 引擎不接 MCP 服务器（`RunStarted.mcp_servers` 恒空）
2. **无子代理**：不派生子代理，`parent_tool_use_id` 恒空
3. **无交互审批**：静态三档起步，无审批反向通道（协议留独立 change）
4. **bash 缺席**：工具面不含 bash（最大刀刃移除；三档表中 bash 拒绝语义为将来预留）
5. **cost 恒缺**：`RunResult.cost_usd` 恒 `None`（无价格表），汇总卡如实呈现
6. **engine_cfg 硬编码位（已偿还，desktop-agent-management）**：api_key / base_url / model 手填真机验证的限制已由全局 agent 管理偿还——engine_cfg 构造源换为管理数据解析（默认 agent / 显式 agent），连接配置经管理页维护，明文边界与遮蔽治理见 desktop-agent-management「api_key 机密边界」
7. **resume 保真度缺口**：Raw 丢弃 / 子代理压平 / 仅顶层重建
8. **引擎归属留痕未裁**：run 记录暂不区分引擎（座位 design 裁定后偿还）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 八条边界均可考，后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `dev-team::commands::exec` | 执行 + 查询命令 | `agent_start` 参数 `engine: Option<EngineKind>` 退役 → `agent` 选择参数（debug-only；缺席解析默认 agent，desktop-agent-management 单点解析）；`DEFAULT_ENGINE` 退役；三件事纪律不变；`Result<T, String>` 错误模板不变（无默认 agent / `ConfigMissing` 均 `Err` 抵达前端） |
| `crates/infra/agent`（裸名 `agent-runtime`） | 引擎门面 | `runner_for(kind, engine_cfg)` 签名与 `EngineConfig` 三字段结构体不变；构造源换为管理数据解析（换点唯一），`from_hardcoded_slot()` 退役；机密面放宽注释标记随 `EngineConfig` 注释更新（desktop-agent-management 边界表） |
| `crates/core/agent` | 中立契约 | 零触碰：`AgentRunParams` 无 agent / engine 字段，`AgentStartError` 变体集不变 |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 调试页参数面 | engine 下拉 → agent 选择器（默认选中默认 agent，仅调试页暴露）；时间线 / 落库 / 重放组件零改动复用 |
| `packages/desktop/src/views/agent/components/agent-run-form.tsx` | run 表单 | `engine` state 与下拉退役 → agent 选择 state（选中 id 随 `agent_start` 传参形态 design 定稿） |
| `packages/desktop/src-tauri/src/bindings/` | IPC 类型镜像 | 经 export-bindings 再生成（`agent_start` 参数面变化），非手改；快照测试同步再生成 |
