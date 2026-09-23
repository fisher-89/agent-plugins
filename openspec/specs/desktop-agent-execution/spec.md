# desktop-agent-execution Specification

## Purpose

定义 agent 执行能力（第五类边界）的完整契约：`core/agent` 中立契约（AgentEvent 信封、AgentRunner trait、run 状态机）、`infra/agent` 的 Claude CLI 租户 MVP、user 维度运行记录落库与重放、exec 轨道首批命令与前端 Agent 调试页，以及 MVP 边界留痕。

## Requirements

### Requirement: agent 边界与双 crate 分层

Desktop 后端 SHALL 将「agent 执行」确立为第五类边界（agent 边界），并按 core/infra 分离落地为两个新 crate：

- `crates/core/agent`（裸名 `agent`）：agent 域中立契约——`AgentEvent` 信封、`AgentRunner` trait、run 状态机（running → completed/failed）、逻辑事件流抽象。MUST NOT 依赖 Tauri，MUST NOT 执行进程 spawn，MUST NOT 认识 claude（不出现 claude 特有 flag 名与 JSONL 行解析）。
- `crates/infra/agent`（裸名 `agent-cli`，受 workspace 内 crate 名唯一性约束）：agent 边界自身的实现——CLI 发现（Windows `.cmd`）、进程 spawn、stdout 逐行泵、JSONL 解析归一化。SHALL 实现 `core/agent` 的 `AgentRunner` trait；MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，非 shell 边界职责）。

依赖方向 SHALL 为：壳 crate → `agent` + `agent-cli`，`agent-cli → agent`；`agent` MUST NOT 依赖 workspace 内任何 crate。两 crate SHALL 注册进 `src-tauri/Cargo.toml` workspace members（rust 套件注册于 src-tauri 根，测试自动覆盖），且 MUST NOT 据此预建其他 infra 边界目录。

#### Scenario: 依赖方向机械可验

- **WHEN** 检查 `src-tauri/Cargo.toml` workspace members 与各 crate 依赖声明
- **THEN** members 含 `crates/core/agent` 与 `crates/infra/agent`，`agent-cli` 依赖 `agent`，壳依赖两者
- **AND** `agent` 与 `agent-cli` 的依赖中无 tauri 系条目，`agent` 无 workspace 内依赖

#### Scenario: core 不认识 claude

- **WHEN** 审查 `crates/core/agent` 源码
- **THEN** 无 claude 特有概念（flag 名、JSONL 行解析、claude 事件 type 名）；claude 细节全部关在 `agent-cli` 内

#### Scenario: spawn 住在 infra

- **WHEN** 检索进程 spawn 调用（`std::process` / `tokio::process`）
- **THEN** 仅出现在 `crates/infra/agent`；`core/agent` 与壳 command 层无 spawn

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

`core/agent` SHALL 定义 `AgentRunner` trait：`start(params) → (逻辑事件流, 运行句柄)`。trait 面 SHALL 仅暴露逻辑事件与运行参数（prompt / env 档位 / permission-mode / cwd 等），进程模型（spawn、stdout/stdin、退出码）MUST NOT 出现在 trait 面上。事件流 SHALL 以 tokio mpsc channel 承载逻辑事件；抽象 SHALL 支持以假 runner（预录事件序列）替换真实 runner 供上层测试。三租户（本机 CLI / 进程内 SDK / 远程 API）SHALL 都能落在该 trait 预留内——CLI 泵 stdout 生产流，SDK 进程内调用，API 以轮询 / watch 模拟流；本变更仅实现 CLI 租户，SDK / API 租户 MUST NOT 预建实现。

run 状态机 SHALL 位于 `core/agent`：running → completed | failed，由 `RunResult`（含 is_error）驱动收敛，收敛后 MUST NOT 再接受状态变更。

#### Scenario: trait 面无进程概念

- **WHEN** 审查 `AgentRunner` trait 与 params / 事件流类型签名
- **THEN** 无进程句柄、stdout/stdin、子进程退出码等进程模型类型，仅逻辑事件与运行参数

#### Scenario: 假 runner 可替换

- **WHEN** 上层（编排函数）测试以预录事件序列的假 runner 注入
- **THEN** tee / 状态机收敛的编排路径全程可测，无需真实 CLI 进程

#### Scenario: 状态机收敛

- **WHEN** 事件流出现 `is_error=true` 的 RunResult
- **THEN** run 状态收敛为 failed；success 时收敛为 completed；收敛后不再接受状态变更

### Requirement: Claude CLI 租户（MVP）

`agent-cli` SHALL 以本机 Claude Code CLI（`-p` 无头 + `--output-format stream-json --verbose`）实现 `AgentRunner`：

- stream-json SHALL 是唯一线上格式（解析路径唯一）；text / json 格式 MUST NOT 进入实现
- 环境档位 SHALL 双档：`default`（完整环境，页面默认）/ `bare`（`--bare` 显式开关，跳过 hooks / skills / custom commands / subagents / plugins / MCP / 自动记忆发现）。bare 档 SHALL 提示认证前提（bare 不读 OAuth 凭据与系统 keychain，须 `ANTHROPIC_API_KEY` 或 `--settings` 配 `apiKeyHelper`）
- permission-mode SHALL 参数面三档（`default` / `acceptEdits` / `bypassPermissions`），默认 `bypassPermissions`（`--dangerously-skip-permissions`）——`-p` 默认 `default` 档下需审批工具直接被拒、看不到真实 loop，本机自有 repo 的调试页场景裁决以完整循环为默认
- cwd SHALL 为当前 workspace root（隐含，不设参数）；model MUST NOT 进 MVP 参数面（继承用户 CLI 默认）
- CLI 发现 SHALL 处理 Windows `.cmd` shim（`cmd /C` 包装或解析真实入口）；CLI 不可发现时 SHALL 显式报错（错误事件 / Err），MUST NOT 静默空转
- MVP MUST NOT 实现：用户 kill 运行、`--continue` / `--resume` 续会话（信封保留 session_id 不堵路）、`--include-partial-messages` 增量流

#### Scenario: flag 组装

- **WHEN** 以 default + bypassPermissions 参数组装命令行
- **THEN** 含 `-p --output-format stream-json --verbose --dangerously-skip-permissions`，不含 `--bare`
- **AND** bare 档时命令行含 `--bare`，stream-json 与 `--verbose` 不变

#### Scenario: JSONL 逐行泵

- **WHEN** 以多行 stream-json 输出 fixture 驱动 runner 解析（不经真实进程）
- **THEN** 逐行归一化为 AgentEvent 序列且行间无串扰，进程退出后 run 收敛

#### Scenario: Windows CLI 发现

- **WHEN** PATH 上的 `claude` 为 `.cmd` shim
- **THEN** spawn 成功（经 `cmd /C` 包装或解析后的真实入口）；CLI 不存在时得到显式错误而非静默空转

#### Scenario: bare 认证失败如实呈现

- **WHEN** 无 `ANTHROPIC_API_KEY` 时以 bare 档发起运行
- **THEN** 失败以错误事件流入时间线呈现，不静默、不崩 UI

### Requirement: run 事件落库与重放（user 维度）

agent 运行记录 SHALL 以 **user 维度**持久化（个人活动历史：不可重建、非 workspace 域派生数据，MUST NOT 落 workspace repo），落盘于既有 app data dir db 文件，新增两表：

- `user_agent_runs`：run 元数据（id、params 摘要、状态、起止时间、num_turns / cost 等汇总）
- `user_agent_run_events`：事件流，key 为 `(run_id, seq)`，value 为事件编码

事件投递 SHALL 双路 tee：Tauri Channel（命令作用域实时流，推前端时间线）+ store sink（逐事件落库），两路 seq 一致。历史查看 SHALL 走 store 查询重放（`agent_runs` / `agent_run_events` invoke），MUST NOT 依赖全局事件广播或要求运行进程存活。MVP MUST NOT 实现留存清理（转录无上限增长为已知限制，留痕未来账）。

#### Scenario: 双路 tee

- **WHEN** 运行产生事件
- **THEN** 每事件同时到达 Channel 与 store sink 且 seq 一致；run 结束后 `user_agent_runs` 元数据状态收敛

#### Scenario: 重放不依赖进程

- **WHEN** 运行结束后（或应用重启后）点开历史运行
- **THEN** 经 `agent_runs` / `agent_run_events` 查询还原完整事件时间线，与实时流内容一致

#### Scenario: user 维度落位

- **WHEN** 审查两表落盘位置与命名
- **THEN** 位于既有 app data dir db、表名携 user 维度前缀；workspace 目录与 repo 内无 agent 运行数据

### Requirement: agent 执行命令面

`commands/exec/` SHALL 开通轨道首批命令（结束空轨道状态）：

- `agent_start`：执行命令。body SHALL 维持三件事纪律（参数转换 → 调用 → 错误映射），编排 SHALL 收在 `run_agent()` 编排函数（组装 runner → 事件流 tee 双 sink → 状态收敛）；该函数与 `*_inner` 同列 app 层微形态（详见 desktop-app-shell）
- `agent_runs` / `agent_run_events`：查询薄包装（无状态，参数 → store 查询 → DTO）

错误约定沿用既有模板 `Result<T, String>`：CLI 不可发现、spawn 失败等 SHALL 以 `Err` 抵达前端，MUST NOT 静默吞掉。

#### Scenario: agent_start 编排收口

- **WHEN** 审查 `agent_start` 实现
- **THEN** 命令体为参数转换 + 调用 `run_agent()` + 错误映射三段，runner 组装与 tee 在编排函数内，不膨胀命令体

#### Scenario: 查询薄包装

- **WHEN** 前端 invoke `agent_runs` / `agent_run_events`
- **THEN** 命令无状态、直查 store 返回 DTO，无领域解释

#### Scenario: 错误 reject 传达

- **WHEN** CLI 不可发现时发起 `agent_start`
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

### Requirement: Agent 调试页

前端 SHALL 新增 Agent 调试页（`AgentDebugView`），经侧栏页面导航组（[变更] [Agent 调试]）进入；视图切换 SHALL 沿用本地 state，MUST NOT 引入路由。页面 SHALL 包含：

- **参数面（最小集）**：prompt（必填）、env 档位（default / bare，默认 default，bare 旁认证前提提示）、permission-mode 三档下拉（默认 bypassPermissions）；cwd 不设参数（隐含当前 workspace root）、model 不设参数
- **事件时间线**：assistant / user 消息渲染为对话流；tool_use / tool_result 折叠块成对呈现；子代理按 `parent_tool_use_id` 分组归因；result 汇总卡（num_turns / cost / duration / session_id 可复制）
- **原始 JSONL 切换**：全部事件（含 Raw）的原文可见
- **历史运行**：run 列表 → 点开自 store 重放（invoke 查询）

实时流经 Tauri Channel 订阅；该订阅属执行流通道（`agent_start` 命令作用域），不属于「刷新取数模型」所禁止的轮询取数；查询类取数（历史 run 列表 / 事件重放）仍由用户显式动作触发。

#### Scenario: 参数面默认值

- **WHEN** 打开调试页
- **THEN** env 默认 default、permission-mode 默认 bypassPermissions、prompt 为空且必填；无 model 输入、无 cwd 输入

#### Scenario: loop 可见性

- **WHEN** 一次含工具调用的运行完成
- **THEN** 时间线可按序看到 assistant(tool_use) → tool_result → … → result 汇总卡，num_turns / cost / duration / session_id 可读可复制，子代理消息归因到父工具调用

#### Scenario: 原始流切换

- **WHEN** 切换原始 JSONL 视图
- **THEN** 全部事件（含 Raw 透传件）原文可见，与落库事件一致

#### Scenario: 历史重放

- **WHEN** 从历史运行列表点开一条已完成 run
- **THEN** 经 invoke 查询以落库事件渲染完整时间线，不要求原运行进程存活

### Requirement: MVP 边界与已知限制

以下边界 SHALL 作为 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 kill**：MUST NOT 提供终止运行入口；应用中途关闭时 claude 子进程可能残留（Windows 尤甚）——孤儿进程为已知限制
2. **无续会话**：`--continue` / `--resume` 不实现，信封保留 session_id
3. **留存无清理**：事件转录无上限增长
4. **交互工具限制**：`-p` 下 AskUserQuestion 类交互工具表现为直接拒绝（permission_denial），调试页如实呈现
5. **bypassPermissions 默认**：deny 规则仍生效；agent 运行可在 workspace 内无审批改动文件——本裁决限于本机自有 repo 的调试场景

#### Scenario: 无终止入口

- **WHEN** 审查调试页与命令面
- **THEN** 无 kill / cancel 命令或按钮，运行中页面仅呈现流式状态

#### Scenario: 限制留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/agent`（新，裸名 `agent`） | agent 域中立契约 | `AgentEvent` 五变体信封（Block / `parent_tool_use_id` / seq+时间戳 / Raw 透传）；`AgentRunner::start(params) → (事件流, 句柄)`；run 状态机 running → completed/failed；零 Tauri、零 spawn、不认识 claude |
| `packages/desktop/src-tauri/crates/infra/agent`（新，裸名 `agent-cli`） | CLI 租户实现 | `ClaudeCliRunner`：CLI 发现（Windows `.cmd`）+ flag 组装 + spawn + JSONL 逐行泵 + 归一化；stream-json + `--verbose` 唯一格式；零 Tauri，实现 `AgentRunner` |
| `packages/desktop/src-tauri/crates/infra/store`（增两表） | run 持久化 | `user_agent_runs`（元数据）/ `user_agent_run_events`（(run_id, seq) 事件流）；user 维度 app data dir；重放查询入口 |
| `dev-team::commands::exec`（开通） | 执行 + 查询命令 | `agent_start`（三件事，编排收 `run_agent()`）；`agent_runs` / `agent_run_events` 无状态薄包装；`Result<T, String>` 错误模板 |
| `run_agent()` 编排函数 | app 层微形态 | 组装 runner → 事件流 tee（Tauri Channel + store sink）→ 状态收敛；将来抽 crate 平移复用不重写 |
| `packages/desktop/src/views/agent/AgentDebugView.tsx`（新） | Agent 调试页 | 参数面（prompt 必填 / env 双档默认 default / permission-mode 默认 bypassPermissions）；事件时间线；原始 JSONL 切换；历史运行重放 |
| `packages/desktop/src/components/AppSidebar.tsx` | 页面导航组 | [变更] [Agent 调试]；本地 state 切视图，无路由 |
| agent 域前端 hooks（新） | 流订阅 + 查询 | Tauri Channel 实时订阅（执行流通道例外）+ invoke 重放查询；查询仍显式触发 |
