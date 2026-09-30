# desktop-agent-execution Specification

## Purpose

定义 agent 执行能力（第五类边界）的完整契约：`core/agent` 中立契约（AgentEvent 信封、AgentRunner trait、run 状态机）、`infra/agent`（裸名 `agent-runtime`）的引擎门面与双引擎租户（`cli/` 本机 Claude CLI、`sdk/` rig 进程内 SDK）、workspace 维度运行记录落库与重放、exec 轨道首批命令与前端 Agent 调试页，以及两租户 MVP 边界留痕。

## Requirements

### Requirement: agent 边界与双 crate 分层

Desktop 后端 SHALL 将「agent 执行」确立为第五类边界（agent 边界），并按 core/infra 分离落地为两个 crate：

- `crates/core/agent`（裸名 `agent`）：agent 域中立契约——`AgentEvent` 信封、`AgentRunner` trait、run 状态机（running → completed/failed/stopped）、逻辑事件流抽象。MUST NOT 依赖 Tauri，MUST NOT 执行进程 spawn，MUST NOT 认识 claude（不出现 claude 特有 flag 名与 JSONL 行解析）。本变更后 MUST NOT 认识引擎（不出现 engine / rig / openai 概念）——引擎选择不进 core 契约。
- `crates/infra/agent`（裸名 `agent-runtime`，原 `agent-cli`——升格引擎门面后旧名名不副实，趁多引擎落地前改名；目录名不变，受 workspace 内 crate 名唯一性约束）：agent 边界的引擎门面与实现——对外以 `EngineKind`（`cli` | `sdk`）参数选择引擎，引擎分发 SHALL 为门面内唯一 match 点；内部承载 `cli/` 引擎（CLI 发现（Windows `.cmd`）、进程 spawn、stdout 逐行泵、JSONL 解析归一化）与 `sdk/` 引擎（rig 进程内租户：agent loop、自建工具面、静态权限 policy、路径沙箱、事件归一化、store 转录 resume）。SHALL 经门面对外提供 `core/agent` 的 `AgentRunner` 实现；MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，非 shell 边界职责）；`rig-core` SHALL 恒编译、MUST NOT 设 feature gate（SDK 引擎定位为不依赖本地环境的 agent，开箱即用，不提供 CLI-only 裁剪档）。

依赖方向 SHALL 为：壳 crate → `agent` + `agent-runtime`，`agent-runtime → agent`；`agent` MUST NOT 依赖 workspace 内任何 crate。两 crate SHALL 注册进 `src-tauri/Cargo.toml` workspace members（rust 套件注册于 src-tauri 根，测试自动覆盖），且 MUST NOT 据此预建其他 infra 边界目录。单 crate 起步（`cli/`、`sdk/` 子模块收拢）；拆引擎子 crate + 门面 re-export 仅作膨胀后的逃生门，MUST NOT 预建。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查 `src-tauri/Cargo.toml` workspace members 与各 crate 依赖声明
- **THEN** members 含 `crates/core/agent` 与 `crates/infra/agent`，`agent-runtime` 依赖 `agent` 与 `rig-core`，壳依赖两者
- **AND** `agent` 与 `agent-runtime` 的依赖中无 tauri 系条目，`agent` 无 workspace 内依赖

#### Scenario: core 不认识 claude 与引擎

- **WHEN** 审查 `crates/core/agent` 源码
- **THEN** 无 claude 特有概念（flag 名、JSONL 行解析、claude 事件 type 名），亦无引擎概念（engine / rig / openai 字样）；claude 细节全部关在 `agent-runtime` 的 `cli/` 引擎内，rig / openai 细节全部关在 `sdk/` 引擎内

#### Scenario: spawn 住在 infra

- **WHEN** 检索进程 spawn 调用（`std::process` / `tokio::process`）
- **THEN** 仅出现在 `crates/infra/agent` 的 `cli/` 引擎子模块；`core/agent` 与壳 command 层无 spawn

#### Scenario: 引擎唯一分发点

- **WHEN** 审查 `agent-runtime` 的引擎构造入口（`runner_for` 形状）
- **THEN** `EngineKind` → 引擎构造的 match 唯一，壳层仅做参数映射不经手引擎细节；新增引擎只动门面一处

### Requirement: AgentEvent 信封与 loop 可见性

`core/agent` SHALL 定义统一事件信封 `AgentEvent`，作为三租户共享的逻辑事件模型（非 stdout 直译）：

- `RunStarted` { model, session_id, tools, mcp_servers, … }（源自 system/init）
- `Message` { role, blocks, parent_tool_use_id }（源自 assistant / user 事件），其中 Block = Text | Thinking | ToolUse{id, name, input} | ToolResult{id, content, is_error}
- `SystemNotice` { subtype, payload }（permission_denied / api_retry 等 system 事件，subtype 不做封闭枚举）
- `RunResult` { subtype, is_error, num_turns, duration, cost, usage, session_id }
- `Raw` { type, 原文 JSON }（未知 / 未来事件类型透传）

loop 调试关键信息 SHALL 在一等公民位：tool_use 与 tool_result 成对可见、子代理经 `parent_tool_use_id` 归因、`RunResult` 携带 num_turns / cost / duration / session_id。每事件 SHALL 携带 `seq`（单调序号，入库排序键）与时间戳。`Raw` 变体 SHALL 承接所有未识别事件：MUST NOT 因未知 type 丢弃事件或解析失败（永不丢事件、永不炸解析），与 ArtifactEnvelope 的 Fallback 同哲学。

#### Scenario: 五变体归一化

- **WHEN** 以真实 stream-json 事件序列（init / assistant 含 tool_use / user 含 tool_result / system / result / 未知 type）驱动归一化
- **THEN** 分别产出 RunStarted / Message(ToolUse) / Message(ToolResult，与 tool_use 同 id) / SystemNotice / RunResult / Raw，seq 单调递增

#### Scenario: 子代理归因保留

- **WHEN** 事件携带非空 `parent_tool_use_id`（子代理消息）
- **THEN** 归一化后的 Message 保留该字段，消费方可按父工具调用分组

#### Scenario: 未知事件透传

- **WHEN** 输入包含官方新增的未知事件 type 或未知 system subtype
- **THEN** 产出 Raw 变体且原文 JSON 完整保留，解析不报错、不丢行

### Requirement: AgentRunner trait 与逻辑事件流抽象

`core/agent` SHALL 定义 `AgentRunner` trait：`start(params) → (逻辑事件流, 运行句柄)`。trait 面 SHALL 仅暴露逻辑事件与运行参数（prompt / permission-mode / cwd 等），进程模型（spawn、stdout/stdin、退出码）MUST NOT 出现在 trait 面上。事件流 SHALL 以 tokio mpsc channel 承载逻辑事件；抽象 SHALL 支持以假 runner（预录事件序列）替换真实 runner 供上层测试。三租户（本机 CLI / 进程内 SDK / 远程 API）SHALL 都能落在该 trait 预留内——CLI 泵 stdout 生产流，SDK 进程内 spawn tokio 泵任务灌流（与 CLI 泵同构），API 以轮询 / watch 模拟流；本变更实现 CLI 与 SDK（rig 进程内）两租户，远程 API 租户 MUST NOT 预建实现。`AgentRunParams` MUST NOT 增设 engine 字段——引擎选择住 infra 门面（invoke body 显式参数），core 契约零污染。

run 状态机 SHALL 位于 `core/agent`：running → completed | failed | stopped，由 `RunResult`（含 is_error）驱动收敛为 completed / failed，显式终止请求（`agent_stop`）SHALL 经独立收敛分支收敛为 `stopped`（见「agent_stop 终止与提前 resolve」），收敛后 MUST NOT 再接受状态变更。

#### Scenario: trait 面无进程概念

- **WHEN** 审查 `AgentRunner` trait 与 params / 事件流类型签名
- **THEN** 无进程句柄、stdout/stdin、子进程退出码等进程模型类型，仅逻辑事件与运行参数；且无引擎选择参数（`AgentRunParams` 无 engine 字段）

#### Scenario: 假 runner 可替换

- **WHEN** 上层（编排函数）测试以预录事件序列的假 runner 注入
- **THEN** tee / 状态机收敛的编排路径全程可测，无需真实 CLI 进程或真实 SDK 调用

#### Scenario: 状态机收敛

- **WHEN** 事件流出现 `is_error=true` 的 RunResult
- **THEN** run 状态收敛为 failed；success 时收敛为 completed；显式终止请求时收敛为 stopped；收敛后不再接受状态变更

#### Scenario: SDK 泵同构零编排改动

- **WHEN** SDK 引擎实现 `AgentRunner::start` 并被同一编排函数（tee 双 sink + 状态机收敛）消费
- **THEN** 编排层代码无引擎分支（零改动复用）；停止信号经 `RunHandle` 对两引擎同样生效

### Requirement: agent_stop 终止与提前 resolve

命令面 SHALL 提供 `agent_stop(run_id)`，`agent_start` 维持提前 resolve 契约：

- `agent_start` SHALL 在 run 记录落库并进入 running 后**提前 resolve**，返回 running 态记录（id 立即可知）；run 执行 SHALL 转入后台任务继续（后台任务持有 Channel 与 store 收尾），终态记录经 Channel 以与实时事件同构的状态部件（`data-run-record`）流出，MUST NOT 再依赖 invoke 返回携带终态。
- `agent_stop` SHALL 按运行中 run 的 id 寻址: 将 run 状态收敛为受控终态 `stopped`、尽力终止引擎执行体（CLI 引擎为 Windows 进程树击杀（`cmd /C` 包装下 `taskkill /T` 或等价）；SDK 引擎为经 `RunHandle` 信号取消泵任务（drop future）——机制由 design 定稿核实）、经 Channel 流出终止终态部件（与实时事件同构）。`stopped` SHALL 与 `completed` / `failed` 同列受控终态（用户主动终止与引擎失败语义区分，收敛后 MUST NOT 再接受状态变更）。
- 对非 running run（已终态或不存在）的 `agent_stop` SHALL 幂等忽略，MUST NOT 报错崩溃或误改既有终态。
- 停止后的重放 SHALL 与实时一致: 重放路径从链记录（含 `stopped` 终态）还原与实时查看相同的呈现（终态部件同构）。

#### Scenario: 提前 resolve

- **WHEN** 前端发起 `agent_start`
- **THEN** invoke 在 run 记录落库进入 running 后即 resolve 返回 running 态记录（id 可用），引擎事件随后经 Channel 持续流入，终态以 `data-run-record` 部件经 Channel 流出

#### Scenario: stop 收敛与引擎终止

- **WHEN** 运行中 invoke `agent_stop(run_id)`
- **THEN** run 状态收敛为 `stopped`，Channel 流出终止终态部件；CLI 运行的进程树被击杀（claude 子进程不再存活），SDK 运行的泵任务被取消（事件流终止、future drop）

#### Scenario: 幂等忽略

- **WHEN** 对一条已 `completed` 的 run invoke `agent_stop`
- **THEN** 幂等忽略: 状态不变、不报错、不产生新事件

#### Scenario: 停止后重放一致

- **WHEN** 停止一轮运行后重新打开该链查看
- **THEN** 重放呈现与停止时实时呈现一致（含 `stopped` 终态部件），续话以链尾记录为准

### Requirement: Claude CLI 租户（MVP）

`agent-runtime` 的 `cli/` 引擎 SHALL 以本机 Claude Code CLI（`-p` 无头 + `--output-format stream-json --verbose`）实现 `AgentRunner`：

- stream-json SHALL 是唯一线上格式（解析路径唯一）；text / json 格式 MUST NOT 进入实现
- 环境档位 MUST NOT 进运行参数面：`AgentRunParams` 无 env 字段，`agent_start` 无 env 参数，运行恒走完整环境（default）；`--bare` flag MUST NOT 组装（bare 档与其认证前提提示已从调试页 / explore 页参数面移除）。run 记录 `env` 列保留（store 既有 schema），落库恒为 `"default"`
- permission-mode SHALL 参数面三档（`default` / `acceptEdits` / `bypassPermissions`），默认 `bypassPermissions`（`--dangerously-skip-permissions`）——`-p` 默认 `default` 档下需审批工具直接被拒、看不到真实 loop，本机自有 repo 的调试页场景裁决以完整循环为默认
- cwd SHALL 为当前 workspace root（隐含，不设参数）；model MUST NOT 进 MVP 参数面（继承用户 CLI 默认）
- CLI 发现 SHALL 处理 Windows `.cmd` shim（`cmd /C` 包装或解析真实入口）；CLI 不可发现时 SHALL 显式报错（错误事件 / Err），MUST NOT 静默空转
- **resume 续会话 SHALL 实现**：`AgentRunParams` 增可选 `resume_session_id: Option<String>`，非空时 flag 组装 `--resume <id>`（与 `-p` 组合即「以该 session 续发新一轮」）；`None` 时行为与既有 one-shot 完全一致。`--continue`（隐式续最近会话）MUST NOT 实现——续话一律显式 `session_id`，语义明确可追溯
- 用户终止运行 SHALL 经 `agent_stop` 命令实现（进程树击杀与状态收敛见「agent_stop 终止与提前 resolve」，非 flag 面职责）；MVP MUST NOT 实现：`--continue` 隐式续会话、`--include-partial-messages` 增量流

#### Scenario: flag 组装

- **WHEN** 以 bypassPermissions 参数组装命令行
- **THEN** 含 `-p --output-format stream-json --verbose --dangerously-skip-permissions`，不含 `--bare` 与 `--resume`

#### Scenario: resume flag 组装

- **WHEN** 以 `resume_session_id = Some("sess-1")` 组装命令行
- **THEN** 参数含 `--resume sess-1`，其余 flag（`-p` / stream-json / `--verbose` / permission 档位）不变；`None` 时不出现 `--resume`

#### Scenario: JSONL 逐行泵

- **WHEN** 以多行 stream-json 输出 fixture 驱动 runner 解析（不经真实进程）
- **THEN** 逐行归一化为 AgentEvent 序列且行间无串扰，进程退出后 run 收敛

#### Scenario: Windows CLI 发现

- **WHEN** PATH 上的 `claude` 为 `.cmd` shim
- **THEN** spawn 成功（经 `cmd /C` 包装或解析后的真实入口）；CLI 不存在时得到显式错误而非静默空转

### Requirement: 引擎门面与 EngineKind 参数选择

`agent-runtime` SHALL 作为 agent 引擎门面：对外一套应用协议（invoke 面 + 事件流 + store 记录 + Timeline 渲染），经 `EngineKind`（`cli` | `sdk`）参数选择引擎；协议统一 SHALL 落在应用协议层而非 core trait 层（core 契约除 `AgentStartError` 加法变体外零改动）。门面 SHALL 提供 `runner_for(kind, engine_cfg)` 形状的构造入口——`EngineConfig` 结构体形态（api_key / base_url / model 三件套）由壳层注入，SDK 引擎用以组装 rig client；CLI 引擎不消费 engine_cfg。engine_cfg 构造源 SHALL 为命令层的管理数据解析（desktop-agent-management「默认 agent 与运行发起解析」：默认 agent / 显式 agent → `EngineKind` + `EngineConfig`，sdk 时由引用 provider 组装、model 取 provider 三档 high 档）；原 MVP 硬编码位（`EngineConfig::from_hardcoded_slot()` 与 `DEFAULT_ENGINE` 常量）SHALL 退役。换构造源时 `EngineConfig` 消费面 MUST NOT 变化（本承诺于 desktop-agent-management 兑现）。启动失败抹平：`AgentStartError` 的中性变体 `ConfigMissing` 承接 SDK 侧连接配置不齐（key 未配 / 模型不存在）等启动失败——core 唯一触碰点，变体集 MUST NOT 再增。

#### Scenario: 门面构造与 core 零污染

- **WHEN** 审查 `runner_for` 实现与 `crates/core/agent` 源码
- **THEN** 引擎分发唯一 match、返回 `AgentRunner` 实现；core 无 engine / `EngineConfig` / rig 概念，`AgentStartError` 变体集与换源前一致

#### Scenario: 构造源换为管理数据解析零改动

- **WHEN** 审查换源后的 engine_cfg 构造点与门面签名
- **THEN** 构造仅发生在命令层解析单点（默认 agent / 显式 agent → `EngineKind` + `EngineConfig`），硬编码位（`from_hardcoded_slot` / `DEFAULT_ENGINE`）无残留，`runner_for` 签名与 `EngineConfig` 消费面（SDK 引擎内部）零改动

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 rig-core 实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig-core` SHALL 恒编译、MUST NOT 设 feature gate。MVP 约定：

- provider 首公民 SHALL 为 openai-compatible 端点（自定义 base_url——GLM / DeepSeek / Kimi / 自部署第一公民）；anthropic provider 降为后续可选
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与 `RunResult` 填充全自控）——rig 仅用作 provider client 与消息/工具类型层（loop 实现归属 rig multi_turn vs 手搓由 design 依 spike 实测定稿）
- 自建工具面：MVP 工具集 = read / grep / glob / ls / write / edit；bash 工具 MUST NOT 进 MVP 工具面（权限三档表中 bash 拒绝语义为将来引入时的预留档位）；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- 事件归一化：openai chat completions 形状的 `tool_calls` 数组 SHALL 映射为 ToolUse 块并与 ToolResult 成对（同 id）；`RunStarted` SHALL 各报各的（model 取 `EngineConfig.model`、tools 报自建工具集）；`RunResult.cost_usd` SHALL 恒为 `None`（无价格表）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
- 依赖足迹：SHALL 以 `default-features = false` + openai feature 最小化入树；spike 实测为必过关卡（无 gate 回退逃生门）

#### Scenario: 事件归一化 fixture

- **WHEN** 以 openai 形状响应 fixture（含 tool_calls、纯文本、未知字段）驱动 normalize 纯函数
- **THEN** 产出 Message(ToolUse) 与成对 Message(ToolResult)（同 id）、RunStarted（model/tools 如实）、未知内容 Raw 透传，seq 单调递增；全程不打网络

#### Scenario: 无 CLI 环境可用

- **WHEN** 无本地 claude CLI 的机器上以 sdk 引擎发起运行（手填 EngineConfig）
- **THEN** 运行正常发起并完成完整 loop（CLI 发现路径不被触及）

#### Scenario: 依赖足迹实测留痕

- **WHEN** spike 以 `default-features = false` + openai feature 实测 rig-core 入树
- **THEN** 体积与新增传递依赖（reqwest / schemars / tracing 系）有数并留痕 design 文档

### Requirement: SDK 引擎静态权限与路径沙箱

SDK 引擎 SHALL 以静态权限三档起步（无审批反向通道——事件流外的应答协议不进 MVP）：Default 档工具面 = 只读（read / grep / glob / ls），write / edit 拒绝；AcceptEdits 档 = + write / edit；BypassPermissions 档 = 全放行。默认档 SHALL 为 BypassPermissions（与调试页既有默认一致——本机自有 repo 的调试场景以完整循环为默认）。拒绝 SHALL 经 `SystemNotice{subtype:"permission_denied"}` + is_error=true 的 ToolResult 流出（与 CLI `-p` 无头 Default 档行为同构）。policy SHALL 为纯决策表（档位 × 工具名 → 允许 / 拒绝）密集测试；交互审批协议（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）MUST NOT 实现（留独立 change）。

路径沙箱：write / edit / read 工具全链 SHALL 经 canonicalize 后做 workspace root 前缀校验（拦 `..\` 相对逃逸与符号链接绕过 root 外目标），校验 SHALL 为纯函数并可密集测试。BypassPermissions 默认档下路径沙箱 SHALL 是模型与任意文件写之间的唯一护栏，其实现质量为必过关卡；命令级白/黑名单 MUST NOT 作为安全手段（假安全），真沙箱（job object / 容器）非本期体量。

#### Scenario: 三档决策表

- **WHEN** 以三档 × 工具名矩阵驱动 policy 纯函数
- **THEN** Default 档对 write/edit 拒绝、AcceptEdits 档放行 write/edit、BypassPermissions 档全放行，逐格断言通过

#### Scenario: 拒绝流出形态

- **WHEN** Default 档下模型请求 write 工具
- **THEN** 事件流含 `SystemNotice{subtype:"permission_denied"}` 与 is_error=true 的 ToolResult（与 tool_use 同 id），run 不中断

#### Scenario: 逃逸拦截

- **WHEN** 工具入参路径含 `..\` 相对逃逸，或为符号链接指向 workspace root 外目标
- **THEN** canonicalize + 前缀校验拒绝执行（tempdir 驱动密集用例全拦）；root 内合法路径不受影响

### Requirement: SDK 引擎 resume（store 即会话）

SDK 引擎 MUST NOT 自建会话存储：resume 时 SHALL 从所属 workspace 库事件转录重建对话历史喂 rig（同一 `resume_session_id` 参数，引擎各自解释——CLI 组装 `--resume` flag，SDK 走转录重建）。重建 SHALL 仅覆盖顶层对话（保真度缺口留痕：Raw 事件丢弃、子代理消息压平）；引用不存在或非 SDK 产出的会话 SHALL 以 `AgentStartError` 中性变体显式启动失败，MUST NOT 静默当新会话。

#### Scenario: 转录重建

- **WHEN** 以一段含顶层 user / assistant(tool_use)/tool_result 往返的 store 转录 fixture 驱动重建
- **THEN** 重建出等价的对话历史序列喂 rig（tool 往返保持成对），Raw 件与子代理消息不进入重建序列

#### Scenario: 会话缺失显式失败

- **WHEN** 以不存在的 session id 调 sdk 引擎 resume
- **THEN** 启动阶段返回 `AgentStartError` 中性变体（`Err` 抵达前端），不产生 run 记录、不静默降级为新会话

### Requirement: run 事件落库与重放（workspace 维度）

agent 运行记录 SHALL 以 **workspace 维度**持久化（维度重裁定，2026-09-29：每条 run 的 cwd 恒为当前 workspace root，运行历史属 workspace 数据），落盘于所属 workspace 的独立 db 文件（全局数据目录 `workspaces/` 子树，归属与寻址见 desktop-workspace-store「全局库与 workspace 库双库布局」），MUST NOT 落全局库。两记录模型不变：`AgentRunRecord`（run 元数据）与 `AgentEventRecord`（事件流，key 为 `(run_id, seq)` 打包键）；模型 shape 与 native_model id / version 零变化，本裁定仅平移落盘归属。run id 语义演进：id 为所属 workspace 库域内自增（写事务内 max+1），MUST NOT 假定跨 workspace 全局唯一；跨库定位 SHALL 携 root（见「agent 执行命令面」root 寻址）。

事件投递 SHALL 双路 tee：Tauri Channel（命令作用域实时流，推前端时间线）+ store sink（逐事件落库），两路 seq 一致。历史查看 SHALL 走所属 workspace 库的 store 查询重放（`agent_runs` / `agent_run_events` invoke，携 root），MUST NOT 依赖全局事件广播或要求运行进程存活。MVP MUST NOT 实现留存清理（转录无上限增长为已知限制，留痕未来账）；workspace 库文件级治理（备份 / 清理）随 remove 保留语义留待二期。

#### Scenario: 双路 tee

- **WHEN** 运行产生事件
- **THEN** 每事件同时到达 Channel 与 store sink 且 seq 一致；run 结束后所属 workspace 库中的 run 元数据状态收敛

#### Scenario: 重放不依赖进程

- **WHEN** 运行结束后（或应用重启后）在原 workspace 下点开历史运行
- **THEN** 经 `agent_runs` / `agent_run_events`（携 root）查询还原完整事件时间线，与实时流内容一致

#### Scenario: workspace 库落位与隔离

- **WHEN** workspace A 与 B 各有运行历史后审查两库内容与查询结果
- **THEN** A 的 runs / 事件仅存在于 A 的 workspace 库文件，B 的任何查询不可见（反之亦然）；全局库无 run / 事件数据；两库允许出现相同 run id（域内自增）

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

### Requirement: MVP 边界与已知限制

以下边界 SHALL 作为 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **kill 已实现（偿还「无 kill」限制）**：`agent_stop` 提供终止入口（见「agent_stop 终止与提前 resolve」）；残余限制——进程树击杀失败时 claude 子进程可能残留（Windows 尤甚），应用中途关闭时的孤儿进程仍为已知限制
2. **续会话经 resume 落地**：`--resume <session_id>` 随 explore 会话链实现（见「Claude CLI 租户（MVP）」）；`--continue` 隐式续会话 MUST NOT 实现；调试页自身仍不提供 resume 入口（续话仅 explore 会话链消费）
3. **留存无清理**：事件转录无上限增长
4. **交互工具限制**：`-p` 下 AskUserQuestion 类交互工具的行为面不变（拒绝或受限如实呈现）；explore 页将其 ToolUse 渲染为静态可读卡片（问题/选项原样呈现），答案经 composer 文本输入并 resume 续话；questionnaire 组件接线为二期
5. **bypassPermissions 默认**：deny 规则仍生效；agent 运行可在 workspace 内无审批改动文件——本裁决限于本机自有 repo 的调试场景

#### Scenario: 终止入口可用

- **WHEN** 审查调试页与 explore 页
- **THEN** 运行中提供停止入口且触发 `agent_stop`；进程树击杀失败时的孤儿进程残余留痕于本 spec

#### Scenario: 限制留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

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
| `packages/desktop/src-tauri/crates/infra/agent`（裸名 `agent-runtime`，原 `agent-cli`） | 引擎门面 + 双引擎实现 | `EngineKind`（`cli` \| `sdk`，specta camelCase）；`runner_for(kind, engine_cfg) → AgentRunner 实现`（唯一 match 分发，返回形状 design 定稿）；`EngineConfig{api_key, base_url, model}`（构造源为命令层管理数据解析——换点唯一，`from_hardcoded_slot()` 退役；机密面放宽注释标记随 `EngineConfig` 注释更新，见 desktop-agent-management 边界表）；`cli/` 引擎（既有 discover / flags / jsonl / runner 平移，唯一 spawn 触点）；`sdk/` 引擎（loop / tools / policy / sandbox / normalize / resume）；rig-core 恒编译（`default-features = false` + openai feature） |
| `packages/desktop/src-tauri/crates/core/agent`（裸名 `agent`） | 中立契约 | `AgentStartError` 含中性变体（`ConfigMissing`）+ Display，变体集不再增；trait / `AgentEvent` / `AgentRunParams` / `RunHandle` / 状态机零变化；无 agent / engine / rig / openai 字样 |
| `packages/desktop/src-tauri/crates/infra/store`（run 持久化） | run / 事件持久化（workspace 维度） | `AgentRunRecord` / `AgentEventRecord` 落所属 workspace 库；run id 库域内自增；模型 shape 与 id / version 零变化；重放查询入口签名不变（实例即上下文） |
| `dev-team::commands::exec` | 执行 + 查询命令 | `agent_start` 参数 `engine: Option<EngineKind>` 退役 → `agent` 选择参数（debug-only；缺席解析默认 agent，desktop-agent-management 单点解析）；`DEFAULT_ENGINE` 退役；三件事纪律不变；`Result<T, String>` 错误模板不变（无默认 agent / `ConfigMissing` 均 `Err` 抵达前端） |
| `run_agent()` 编排函数 | app 层微形态（后台任务） | 经门面 `runner_for` 组装 runner → 事件流 tee（Tauri Channel + store sink）→ 状态收敛；后台任务持有 Channel 与 workspace 库句柄收尾，提前 resolve 后 tee 双 sink 不变；将来抽 crate 平移复用不重写 |
| `RunStopRegistry` | 运行中停止句柄注册表 | 寻址键演进为含 root（run id 域内化配套）；其余语义不变 |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | 调试页参数面 | agent 选择器（默认选中默认 agent，仅调试页暴露）；时间线 / 落库 / 重放组件零改动复用 |
| `packages/desktop/src/views/agent/components/agent-run-form.tsx` | run 表单 | agent 选择 state（选中 id 随 `agent_start` 传参；`engine` state 与下拉退役） |
| `packages/desktop/src/components/app-sidebar.tsx` | 页面导航组 | [变更] [Agent 调试]；本地 state 切视图，无路由 |
| agent 域前端 hooks（新） | 流订阅 + 查询 | Tauri Channel 实时订阅（执行流通道例外）+ invoke 重放查询；查询仍显式触发 |
| `packages/desktop/src-tauri/Cargo.toml` | workspace 依赖收敛 | workspace.dependencies 键 `agent-cli → agent-runtime`；新增 `rig-core` pin 条目；members 行与目录名不变 |
| `packages/desktop/src-tauri/src/bindings/` | IPC 类型镜像 | 经 export-bindings 再生成（`agent_start` 参数面与管理 DTO 变化），非手改；快照测试同步再生成 |
