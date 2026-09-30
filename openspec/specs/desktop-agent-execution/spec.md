# desktop-agent-execution Specification

## Purpose

定义 agent 执行能力（第五类边界）的完整契约：`core/agent` 中立契约（AgentEvent 信封、AgentRunner trait、run 状态机）、`infra/agent` 的 Claude CLI 租户 MVP、workspace 维度运行记录落库与重放、exec 轨道首批命令与前端 Agent 调试页，以及 MVP 边界留痕。

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

`core/agent` SHALL 定义 `AgentRunner` trait：`start(params) → (逻辑事件流, 运行句柄)`。trait 面 SHALL 仅暴露逻辑事件与运行参数（prompt / permission-mode / cwd 等），进程模型（spawn、stdout/stdin、退出码）MUST NOT 出现在 trait 面上。事件流 SHALL 以 tokio mpsc channel 承载逻辑事件；抽象 SHALL 支持以假 runner（预录事件序列）替换真实 runner 供上层测试。三租户（本机 CLI / 进程内 SDK / 远程 API）SHALL 都能落在该 trait 预留内——CLI 泵 stdout 生产流，SDK 进程内调用，API 以轮询 / watch 模拟流；本变更仅实现 CLI 租户，SDK / API 租户 MUST NOT 预建实现。

run 状态机 SHALL 位于 `core/agent`：running → completed | failed | stopped，由 `RunResult`（含 is_error）驱动收敛为 completed / failed，显式终止请求（`agent_stop`）SHALL 经独立收敛分支收敛为 `stopped`（见「agent_stop 终止与提前 resolve」），收敛后 MUST NOT 再接受状态变更。

#### Scenario: trait 面无进程概念

- **WHEN** 审查 `AgentRunner` trait 与 params / 事件流类型签名
- **THEN** 无进程句柄、stdout/stdin、子进程退出码等进程模型类型，仅逻辑事件与运行参数

#### Scenario: 假 runner 可替换

- **WHEN** 上层（编排函数）测试以预录事件序列的假 runner 注入
- **THEN** tee / 状态机收敛的编排路径全程可测，无需真实 CLI 进程

#### Scenario: 状态机收敛

- **WHEN** 事件流出现 `is_error=true` 的 RunResult
- **THEN** run 状态收敛为 failed；success 时收敛为 completed；显式终止请求时收敛为 stopped；收敛后不再接受状态变更

### Requirement: agent_stop 终止与提前 resolve

命令面 SHALL 新增 `agent_stop(run_id)`，并 SHALL 演进 `agent_start` 的返回契约：

- `agent_start` SHALL 在 run 记录落库并进入 running 后**提前 resolve**，返回 running 态记录（id 立即可知）；run 执行 SHALL 转入后台任务继续（后台任务持有 Channel 与 store 收尾），终态记录经 Channel 以与实时事件同构的状态部件（`data-run-record`）流出，MUST NOT 再依赖 invoke 返回携带终态。
- `agent_stop` SHALL 按运行中 run 的 id 寻址: 将 run 状态收敛为新增受控终态 `stopped`、尽力击杀 CLI 进程树（Windows `cmd /C` 包装下 `taskkill /T` 或等价，机制由 design 定稿核实）、经 Channel 流出终止终态部件（与实时事件同构）。`stopped` SHALL 与 `completed` / `failed` 同列受控终态（用户主动终止与 CLI 失败语义区分，收敛后 MUST NOT 再接受状态变更）。
- 对非 running run（已终态或不存在）的 `agent_stop` SHALL 幂等忽略，MUST NOT 报错崩溃或误改既有终态。
- 停止后的重放 SHALL 与实时一致: 重放路径从链记录（含 `stopped` 终态）还原与实时查看相同的呈现（终态部件同构）。

#### Scenario: 提前 resolve

- **WHEN** 前端发起 `agent_start`
- **THEN** invoke 在 run 记录落库进入 running 后即 resolve 返回 running 态记录（id 可用），CLI 事件随后经 Channel 持续流入，终态以 `data-run-record` 部件经 Channel 流出

#### Scenario: stop 收敛与击杀

- **WHEN** 运行中 invoke `agent_stop(run_id)`
- **THEN** run 状态收敛为 `stopped`，Channel 流出终止终态部件，CLI 进程树被击杀（claude 子进程不再存活）

#### Scenario: 幂等忽略

- **WHEN** 对一条已 `completed` 的 run invoke `agent_stop`
- **THEN** 幂等忽略: 状态不变、不报错、不产生新事件

#### Scenario: 停止后重放一致

- **WHEN** 停止一轮运行后重新打开该链查看
- **THEN** 重放呈现与停止时实时呈现一致（含 `stopped` 终态部件），续话以链尾记录为准

### Requirement: Claude CLI 租户（MVP）

`agent-cli` SHALL 以本机 Claude Code CLI（`-p` 无头 + `--output-format stream-json --verbose`）实现 `AgentRunner`：

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

- `agent_start`：执行命令。body SHALL 维持三件事纪律（参数转换 → 调用 → 错误映射），编排 SHALL 收在 `run_agent()` 编排函数（组装 runner → 事件流 tee 双 sink → 状态收敛）；该函数与 `*_inner` 同列 app 层微形态（详见 desktop-app-shell）。命令 SHALL 增可选参数：`resume_session_id`（续会话）、`source`（来源受控字符串，缺省 `debug`）、`source_ref`（来源内定位）、`parent_run_id`（链上游 run）——编排 SHALL 将其写入 run 记录字段面；不传链参数时生成的记录 `source="debug"`、链字段为 `None`，字段面行为与演进前一致（返回时序演进为提前 resolve running 记录，见「agent_stop 终止与提前 resolve」）。落库 SHALL 经 root 解析所属 workspace 库（`WorkspaceStores::for_root`，见 desktop-workspace-store），run 与事件写入当前 workspace 的库文件
- `agent_runs` / `agent_run_events` / `agent_run_chain`：查询薄包装（无状态，参数含 root → workspace 库解析 → store 查询 → DTO）；`agent_run_chain` 的 `source_ref` 为 workspace 库域内的 explore 记录 id，root 寻址与库域内 id 配套消解跨库歧义
- `agent_stop`：终止命令（寻址、`stopped` 收敛与进程树击杀见「agent_stop 终止与提前 resolve」）；寻址 SHALL 携 root（run id 为 workspace 库域内自增，裸 id 跨库歧义由 root 消解），`RunStopRegistry` 寻址键随 root 演进

错误约定沿用既有模板 `Result<T, String>`：CLI 不可发现、spawn 失败、workspace 库打开失败等 SHALL 以 `Err` 抵达前端，MUST NOT 静默吞掉。

#### Scenario: agent_start 编排收口

- **WHEN** 审查 `agent_start` 实现
- **THEN** 命令体为参数转换 + 调用 `run_agent()` + 错误映射三段，runner 组装与 tee 在编排函数内，不膨胀命令体

#### Scenario: 来源与 resume 参数透传

- **WHEN** explore 页以 `source="explore"`、`source_ref=<记录键>`、`resume_session_id=<sid>` invoke `agent_start`
- **THEN** 生成 run 记录携带对应字段且进程以 `--resume` 续话；调试页 invoke（不传这些参数）生成的记录 `source="debug"`、链字段为 `None`

#### Scenario: 查询与停止 root 寻址

- **WHEN** 前端 invoke `agent_runs` / `agent_run_events` / `agent_run_chain` / `agent_stop`
- **THEN** 命令无状态、按 root 解析所属 workspace 库直查/寻址返回 DTO，无领域解释；workspace A 的调用看不到 B 的 runs，同 id 并行时停止命中发起方所在库的 run

#### Scenario: 错误 reject 传达

- **WHEN** CLI 不可发现或所属 workspace 库打开失败时发起 `agent_start`
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

### Requirement: Agent 调试页

前端 SHALL 新增 Agent 调试页（`AgentDebugView`），经侧栏页面导航组（[变更] [Agent 调试]）进入；视图切换 SHALL 沿用本地 state，MUST NOT 引入路由。页面 SHALL 包含：

- **参数面（最小集）**：prompt（必填）、permission-mode 三档下拉（默认 bypassPermissions）；cwd 不设参数（隐含当前 workspace root）、model 不设参数、env 不设参数（运行恒为完整环境，见 CLI 租户约定）
- **事件时间线**：SHALL 经共享组件族 `AgentTimeline`（desktop-agent-chat-infra）以保真透镜呈现——seq 序、tool_use / tool_result 成对、子代理按 `parent_tool_use_id` 分组归因、result 汇总卡（num_turns / cost / duration / session_id 可复制）
- **运行中停止**：运行中 SHALL 呈现停止入口，触发 `agent_stop`（见「agent_stop 终止与提前 resolve」）
- **原始 JSONL 切换**：全部事件（含 Raw）的原文可见
- **历史运行**：run 列表 → 点开自 store 重放（invoke 查询）

页面状态 SHALL 经统一会话基建（`use-agent-chat`）承载；run 表单 / 历史列表 / JSONL 开关为页面级 chrome，包在共享核心外圈。实时流经 transport 走 Tauri Channel 订阅；该订阅属执行流通道（`agent_start` 命令作用域），不属于「刷新取数模型」所禁止的轮询取数；查询类取数（历史 run 列表 / 事件重放）仍由用户显式动作触发。

#### Scenario: 参数面默认值

- **WHEN** 打开调试页
- **THEN** permission-mode 默认 bypassPermissions、prompt 为空且必填；无 model 输入、无 cwd 输入、无 env 输入

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/agent`（新，裸名 `agent`） | agent 域中立契约 | `AgentEvent` 五变体信封（Block / `parent_tool_use_id` / seq+时间戳 / Raw 透传）；`AgentRunner::start(params) → (事件流, 句柄)`；run 状态机 running → completed/failed/stopped（`agent_stop` 独立收敛分支，收敛后不接受状态变更）；零 Tauri、零 spawn、不认识 claude |
| `packages/desktop/src-tauri/crates/infra/agent`（新，裸名 `agent-cli`） | CLI 租户实现 | `ClaudeCliRunner`：CLI 发现（Windows `.cmd`）+ flag 组装 + spawn + JSONL 逐行泵 + 归一化；stream-json + `--verbose` 唯一格式；零 Tauri，实现 `AgentRunner` |
| `packages/desktop/src-tauri/crates/infra/store`（run 持久化） | run / 事件持久化（workspace 维度） | `AgentRunRecord` / `AgentEventRecord` 落所属 workspace 库；run id 库域内自增；模型 shape 与 id / version 零变化；重放查询入口签名不变（实例即上下文） |
| `dev-team::commands::exec` | 执行 + 查询命令 | `agent_start`（三件事，编排收 `run_agent()`，落库即提前 resolve，经 root 解析 workspace 库）；`agent_stop(root, run_id)`（root 消歧，幂等，进程树击杀，`stopped` 收敛）；`agent_runs` / `agent_run_events` / `agent_run_chain` 携 root 薄包装；`Result<T, String>` 错误模板 |
| `run_agent()` 编排函数 | app 层微形态（后台任务） | 组装 runner → 事件流 tee（Tauri Channel + store sink）→ 状态收敛；后台任务持有 Channel 与 workspace 库句柄收尾，提前 resolve 后 tee 双 sink 不变；将来抽 crate 平移复用不重写 |
| `RunStopRegistry` | 运行中停止句柄注册表 | 寻址键演进为含 root（run id 域内化配套）；其余语义不变 |
| `packages/desktop/src/views/agent/agent-debug-view.tsx`（新） | Agent 调试页 | 参数面（prompt 必填 / permission-mode 默认 bypassPermissions / env 不设参数）；时间线经 `AgentTimeline` 保真透镜；运行中停止入口（`agent_stop`）；原始 JSONL 切换；历史运行重放；run 表单 / 历史 / JSONL 开关留外圈 chrome |
| `packages/desktop/src/components/app-sidebar.tsx` | 页面导航组 | [变更] [Agent 调试]；本地 state 切视图，无路由 |
| agent 域前端 hooks（新） | 流订阅 + 查询 | Tauri Channel 实时订阅（执行流通道例外）+ invoke 重放查询；查询仍显式触发 |
