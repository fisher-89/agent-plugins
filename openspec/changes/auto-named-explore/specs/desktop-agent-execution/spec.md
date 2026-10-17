# desktop-agent-execution Specification (Delta)

## ADDED Requirements

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 自本条起激活为引擎消费面（切片③收编），形态为「收窄 + 受控追加」双位：

- **core 契约扩展**：`SessionInjections` SHALL 新增受控枚举位 `toolset: Option<SessionToolset>`（首值 `Explore`）；既有 `tools: Option<Vec<String>>` 位激活为七工具子集收窄表达（值必须为七工具名子集，非法名在装配层 `Err`）。`preamble` 位维持不消费（系统提示词唯一来源仍为 `AGENT.md`，见「SDK 引擎系统提示词（AGENT.md 注入）」）。词汇封闭原则延续：专用工具仅经 `toolset` 枚举注入，自由字符串工具名 MUST NOT 进执行面；core 仅承载词汇，解释归 infra。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance.source == "explore"` 判别——explore 会话注入 `tools = [read, grep, glob, ls, edit, bash]`（`write` 移除）+ `toolset = Explore`；其余来源（debug / change 等）SHALL 恒 `SessionInjections::default()`（七工具面，零变化）。
- **loop 组装与硬禁用**：definitions SHALL 按注入面组装（收窄子集 + `create_explore_note` 定义）；`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从 definitions 移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行（现分发链 `policy → sandbox → execute` 无此校验的缺口收口）。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root），sdk loop 经 `LoopTurn` 携带的上下文路由；`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **policy 档位归属**：`create_explore_note` SHALL 归写面档位（Default 拒绝、AcceptEdits / BypassPermissions 放行）——决策表纯函数扩展（见「SDK 引擎静态权限与路径沙箱」表）。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note（无 `write`），`RunStarted.tools` 逐名一致上报该面

#### Scenario: 幻觉 write 调用被拒

- **WHEN** explore 会话中模型发出已从 definitions 移除的 `write` 工具调用
- **THEN** 成员校验拒绝：`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult（与 tool_use 同 id）回灌，run 不中断、不触执行体

#### Scenario: create 原子定名

- **WHEN** 占位记录（`draft-1760000000000`）会话中模型调用 `create_explore_note(name="api-retry", content=<初稿>)`
- **THEN** 磁盘出现 `openspec/explores/api-retry.md`（内容 = content），store 记录 `name="api-retry"`（`id` / `session_id` 不变），返回文本含笔记路径；占位名文件全程不存在

#### Scenario: create 拒绝面零半边残留

- **WHEN** 分别以非法 kebab 名、撞已有记录名、目标文件已存在（既有笔记）调用 `create_explore_note`
- **THEN** 均显式 `Err` 回灌（文件已存在臂附路径），磁盘与库内零变化（前置校验先于任何写动作）

#### Scenario: 已定名记录异名拦截

- **WHEN** 已定名记录（`api-retry`，笔记在盘）会话中模型调用 `create_explore_note(name="other-name", …)`
- **THEN** `Err` 附现有名 `api-retry` 与笔记路径，零改动；同名且文件缺失时补建成功（孤儿重建）

#### Scenario: 非 explore 来源零变化

- **WHEN** 以 `source="debug"`（或不传来源）发起会话
- **THEN** definitions 与 `RunStarted.tools` 仍为七工具（含 `write`），成员校验对七工具全放行，行为与注入机制上线前一致

## MODIFIED Requirements

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 首公民 SHALL 为 openai-compatible 端点（自定义 base_url——GLM / DeepSeek / Kimi / 自部署第一公民）；anthropic provider 降为后续可选
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层
- **流面双层交付**：流内 `Text` / `Reasoning` 片段增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面；rig 0.43 流事件模型为部分生命周期——Start / Arguments / End 为簿记事件不出增量）；轮末 `finish()` 折叠的 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：默认（无会话注入）工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；会话级注入 SHALL 可收窄该面并追加受控专用工具（见「会话级工具面注入与 explore 笔记工具」）；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求系统提示词 SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections.preamble` 位维持不消费，`tools` / `toolset` 位按来源装配（见「会话级工具面注入与 explore 笔记工具」）
- **上下文管理**：喂 provider 的请求史 SHALL 经 L1-L3 上下文防线治理（见「SDK 引擎工具质量与单结果上限（L1）」「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）；store 转录全量不变
- 事件归一化：openai chat completions 形状的 `tool_calls` 数组 SHALL 映射为 ToolUse 块并与 ToolResult 成对（同 id）；`RunStarted` SHALL 各报各的（model 取 `EngineConfig.model`、tools 报本轮实际下发的工具面——默认七工具、注入面见「会话级工具面注入与 explore 笔记工具」）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
- 依赖足迹：workspace 依赖 SHALL 为 `rig` facade 单条目（`=0.43.0` 锁版——升版经显式修订而非静默漂移；`default-features = false` + 显式 features：`reqwest` / `native-tls` / memory feature，reqwest / native-tls 经 facade 转发到 rig-core）；代码 `rig_core::` 路径零残留。0.42 时代的 `rig-core` 双条目解析钉子退役（2026-10-06 修订）：0.43 家族拆出 rig-reqwest / rig-http / rig-tungstenite 等同伴 crate 后双条目只钉得住 rig-core 一个，全家族精确版本由 Cargo.lock 承担

#### Scenario: 事件归一化 fixture

- **WHEN** 以 openai 形状流 fixture（text delta × N、reasoning delta、tool_calls、未知字段、轮末 choice）驱动 normalize 纯函数
- **THEN** 产出 MessageDelta(text/thinking) 序列、Message(ToolUse) 与成对 Message(ToolResult)（同 id）、轮末恰一条密封 Message（块齐）、未知内容 Raw 透传，seq 单调递增；全程不打网络

#### Scenario: 无 CLI 环境可用

- **WHEN** 无本地 claude CLI 的机器上以 sdk 引擎发起运行（手填 EngineConfig）
- **THEN** 运行正常发起并完成完整 loop（CLI 发现路径不被触及）

#### Scenario: facade 依赖足迹

- **WHEN** 审查 workspace 依赖与 `crates/infra/agent` 源码
- **THEN** 依赖为 `rig` facade 单条目（锁版 `=0.43.0`、`default-features = false` + 显式 features），`rig_core::` 路径零残留；恒编译、无 feature gate

### Requirement: SDK 引擎 bash 工具

SDK 引擎工具面 SHALL 增第七工具 `bash`，`RunStarted.tools` SHALL 报**本轮实际下发的工具面**（无注入会话即七工具；注入面见「会话级工具面注入与 explore 笔记工具」）：

- **档位归属**：Default 档拒绝、AcceptEdits / BypassPermissions 档放行（见「SDK 引擎静态权限与路径沙箱」的三档分化）；拒绝形态与 write/edit 同构（`SystemNotice{subtype:"permission_denied"}` + is_error=true ToolResult，run 不中断）
- **进程执行**：`tokio::process`，cwd = workspace root，环境继承当前进程；shell 底座 Windows SHALL 优先探测 git-bash（unix 语法对模型成功率最高），探测失败退 `cmd /C` 兜底（不损开箱即跑）；unix 侧 `sh -c`
- **停止语义**：`kill_on_drop(true)` + Windows 进程树清理沿 `taskkill /T /F` 形状（原 `cli/runner.rs` `kill_process_tree` 同形，形状随 retire-cli-engine 迁驻 `sdk/bash.rs` 唯一持有；future drop 不自动杀子进程树）
- **超时**：缺省 120s、钳位 [1s, 600s]——活性护栏，非安全护栏
- **知情边界**：授权/沙箱/安全机制不进本期——BypassPermissions 默认档下 bash 即模型可执行任意命令、护栏为零；命令级白/黑名单、真沙箱、交互审批留后续 change（边界条款见「SDK 租户 MVP 边界与已知限制」）

#### Scenario: 放行档执行回灌

- **WHEN** AcceptEdits（或 BypassPermissions）档下模型调用 bash 执行命令
- **THEN** 命令执行、ToolResult 含 stdout/stderr 合并输出，bash 事件与其他工具同构进出转录

#### Scenario: Default 档拒绝

- **WHEN** Default 档下模型请求 bash 工具
- **THEN** 拒绝流出（`permission_denied` SystemNotice + is_error=true ToolResult），run 不中断

#### Scenario: 超时与进程树清理

- **WHEN** bash 命令超时，或命令运行中会话被终止
- **THEN** 进程树被清理（`taskkill /T /F` 形状），无孤儿进程残留；超时以活性护栏错误收口，不挂死 run

#### Scenario: Windows 兜底探测

- **WHEN** Windows 环境无 git-bash
- **THEN** 经 `cmd /C` 兜底执行，开箱即跑不破坏；探测与兜底选择可测（探测函数纯化）

### Requirement: SDK 引擎系统提示词（AGENT.md 注入）

SDK 引擎 SHALL 在起播前读取 workspace root 的 `AGENT.md` 单份文件作为系统提示词来源：文件存在 → 内容**逐字**注入请求系统提示词位（rig 0.43 起 `preamble` 字段退役，以 chat_history 首条 `Message::System` 承载；不包装、不改写）；文件缺席 → 不注入 System 消息（现状语义不变）。读取范围 SHALL 严格限于 `AGENT.md`，MUST NOT 做 `CLAUDE.md` 等兜底回退。注入 SHALL **每轮重读**：会话中途修改 `AGENT.md`，改动自下一轮请求生效（不做起播快照——bash 引入后 agent 可改自己的系统提示词，该语义行为最简且可预期）。

该机制 SHALL 为引擎内部特性（P2 原则：跨会话持久上下文是 agent 产品特性非协议义务），本机制自身 `crates/core/agent` 零改动；`SessionInjections.preamble` 位维持不消费（系统提示词唯一来源仍为 `AGENT.md`，会话级系统前导不进 MVP），工具面注入位自「会话级工具面注入与 explore 笔记工具」起按 provenance 来源装配（compose 不再恒传 default）。注入体量上限 32KB（char 边界截断）、嵌套 `AGENT.md` 不级联。

#### Scenario: AGENT.md 在场逐字注入

- **WHEN** workspace root 存在 `AGENT.md` 时以 sdk 引擎发起运行
- **THEN** 请求系统提示词（chat_history 首条 System 消息）为该文件内容逐字（不包装、不改写）；AGENT.md 注入路径零 diff（compose 的注入装配仅工具面，见「会话级工具面注入与 explore 笔记工具」）

#### Scenario: 缺席维持现状

- **WHEN** workspace root 无 `AGENT.md` 时以 sdk 引擎发起运行
- **THEN** 请求不注入 System 消息，请求形状与注入机制上线前一致

#### Scenario: 会话中修改下一轮生效

- **WHEN** 会话运行中途修改 `AGENT.md` 后续发下一轮
- **THEN** 已发轮次不受影响，下一轮请求注入修改后的内容（每轮重读，无起播快照）

#### Scenario: 无兜底回退

- **WHEN** workspace root 只有 `CLAUDE.md` 而无 `AGENT.md`
- **THEN** 不发生兜底读取，系统提示词位仍空（严格单文件语义）

## Module Contract

| 模块 | 类别 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/runner.rs` | API | `SessionInjections` 增 `toolset: Option<SessionToolset>`（首值 `Explore`）；`preamble` 位维持不消费；serde additive（缺省读兼容旧线格式） |
| `crates/infra/agent/src/compose.rs` | API / Function | `begin` 按 `provenance.source == "explore"` 装配注入（六工具收窄 + `Explore` 枚举）；per-turn 专用工具执行上下文铸造（store 句柄 + root）；其余来源恒 default |
| `crates/infra/agent/src/sdk/loop.rs` | Function | definitions 按注入面组装；`RunStarted.tools` 报实际下发面；执行前成员校验（`tool_unavailable` 拒绝，run 不中断）；专用工具经 `LoopTurn` 上下文路由 |
| `crates/infra/agent/src/sdk/tools.rs` | Function | 定义面组装参数化 + `create_explore_note` 定义（name / content 入参）；`execute` 对七通用工具保持 `(root, name, input)` 纯函数 |
| `crates/infra/agent/src/sdk/policy.rs` | Function | `create_explore_note` 归写面档位（Default 拒绝、AcceptEdits / BypassPermissions 放行） |
| `crates/infra/agent/src/sdk/`（create 执行体，新） | Function | 执行时按会话指针圈定当前 explore 记录；占位 → 前置校验 + 落盘 + in-place 改名；已定名 → 同名补建 / 异名 `Err` 附路径；失败面显式 `Err` 附恢复指引 |
