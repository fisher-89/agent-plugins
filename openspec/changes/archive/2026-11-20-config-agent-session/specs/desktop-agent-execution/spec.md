# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 协议 SHALL **双值可选且双首公民**：`openai`（openai-compatible 端点——GLM / DeepSeek / Kimi / 自部署）与 `anthropic`（Anthropic Messages 端点及兼容网关）；协议由 provider 记录 `protocol` 声明、经 `EngineConfig.protocol` 抵达引擎（见「SDK 引擎模型协议选臂（openai / anthropic）」）——原「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」的限制在本变更偿还
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层；**工具执行体 SHALL 允许阻塞等待外部输入**（ask 工具的 turn 内应答等待，见「ask 工具协议与阻塞应答通道」），阻塞期间该轮不收敛、不产生额外模型请求
- **流面双层交付**：流内 `Text` / `Reasoning` 片段增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面；rig 0.43 流事件模型为部分生命周期——Start / Arguments / End 为簿记事件不出增量）；轮末 `finish()` 折叠的 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：默认（无会话注入）工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；会话级注入 SHALL 以**名字清单**收窄该面、以专用工具**名字**追加受控工具面（工具定义与准入档唯一真源在 infra 挂载清单，见「会话级工具面注入与 explore 笔记工具」）；**`AskUserQuestion` 为基线交互工具**——与七工具并列的能力，MUST NOT 进 `TOOL_NAMES`（它不是「名字清单可收窄的七件之一」），其在场与否由 `SessionInjections.ask` 位决定；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求系统提示词 SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections.preamble` 位按来源装配（生产者 = archi 臂与 config 臂），`tools` / `finish` / `ask` 位按来源装配（见「会话级工具面注入与 explore 笔记工具」；`toolset` 位已随枚举退役）
- **上下文管理**：喂 provider 的请求史 SHALL 经 L1-L3 上下文防线治理（见「SDK 引擎工具质量与单结果上限（L1）」「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）；store 转录全量不变；窗长来源含模型名 `[1m]` 后缀（见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）
- 事件归一化：归一化 SHALL 只消费 rig 抽象事件词汇（`StreamEvent` / `Completion` / `Message`），工具调用映射为 ToolUse 块并与 ToolResult 成对（同 id）；协议差异 MUST NOT 出现在归一化层（openai chat completions 形状 fixture 仅为测试形状，不是运行时协议判定；anthropic 经 Messages wire 产出同一抽象）；`RunStarted` SHALL 各报各的（model 报**裸名**——`[1m]` 后缀已剥离，tools 报本轮实际下发的工具面——默认七工具、注入面含 `finish_step` / `AskUserQuestion` 见「会话级工具面注入与 explore 笔记工具」）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
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

#### Scenario: 双协议均可发起

- **WHEN** 分别以 `protocol = openai` 与 `protocol = anthropic` 的 provider 发起运行
- **THEN** 两者都走同一 `sdk/loop.rs` 全链（工具面 / 防线 / 事件词汇一致），仅 wire 与请求参数不同；归一化层与 core 侧无协议分支

#### Scenario: 注入位词面为四位

- **WHEN** 审查 `SessionInjections` 字段面与本条工具面约定
- **THEN** 只出现 `preamble` / `tools` / `finish` / `ask` 四位；产品源码（`crates/**` / `src/**`）中 `toolset` 与 `SessionToolset` 零命中

#### Scenario: 阻塞执行体不额外消耗请求

- **WHEN** 模型在一轮内调用 `AskUserQuestion` 并等待用户应答
- **THEN** 阻塞期间该轮不收敛、不发起新的模型请求；应答回灌后同一轮的下一次模型请求照常发出（`turn_index` 递增一次，`session_id` 不变）

### Requirement: SDK 引擎系统提示词（AGENT.md 注入）

SDK 引擎 SHALL 在起播前读取 workspace root 的 `AGENT.md` 单份文件作为系统提示词来源：文件存在 → 内容**逐字**注入请求系统提示词位（rig 0.43 起 `preamble` 字段退役，以 chat_history 首条 `Message::System` 承载；不包装、不改写）；文件缺席 → 不注入 System 消息（现状语义不变）。读取范围 SHALL 严格限于 `AGENT.md`，MUST NOT 做 `CLAUDE.md` 等兜底回退。注入 SHALL **每轮重读**：会话中途修改 `AGENT.md`，改动自下一轮请求生效（不做起播快照——bash 引入后 agent 可改自己的系统提示词，该语义行为最简且可预期）。

该机制 SHALL 为引擎内部特性（P2 原则：跨会话持久上下文是 agent 产品特性非协议义务），`crates/core/agent` 零改动；工具面注入位自「会话级工具面注入与 explore 笔记工具」起按 provenance 来源装配（compose 不再恒传 default）。注入体量上限 32KB（char 边界截断）、嵌套 `AGENT.md` 不级联。

`SessionInjections.preamble` 位自本变更起**按来源装配消费**（生产者两类：`source = "archi"` 的架构会话知识，见 `desktop-archi-agent`「架构会话的系统知识注入」；`source = "config"` 的配置协议知识，见 `desktop-config-agent`「配置协议知识的静态 preamble」。其余来源恒 `None`，会话级系统前导不进 MVP 的裁定对它们继续生效）：装配产物与本轮 `AGENT.md` SHALL 合并为**恰一条** System 消息，MUST NOT 发两条 System 消息：

- **顺序**：会话 preamble（产品知识）在前，`AGENT.md` 内容逐字在后，两段之间以空行分隔。
- **单侧在场**：仅 `AGENT.md` 在场 → System 消息内容为该文件内容逐字（与本变更前逐字一致，不包装不改写）；仅 preamble 在场 → 单条为 preamble 内容；两者皆无 → 不注入 System 消息。
- **截断口径分立**：`AGENT.md` 的 32KB / char 边界截断口径不变（只作用于该文件自身）；会话 preamble 不在该截断逻辑内，其体量由生产者侧测试锚定上限。
- **来源约束**：preamble 只由装配层按 provenance 决定，前端 / 自由字符串 MUST NOT 直接供给（`core/agent` 词汇零改动，`SessionInjections.preamble` 字段语义不变）。

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

#### Scenario: config 臂同样合为单条 System

- **WHEN** 以 `source = "config"` 发起会话且 workspace root 存在 `AGENT.md`
- **THEN** chat_history 恰有一条 System 消息，内容为「配置协议知识 + 空行 + `AGENT.md` 逐字」；MUST NOT 出现第二条 System 消息

#### Scenario: 会话 preamble 与 AGENT.md 合为单条

- **WHEN** 以 `source = "archi"` 发起会话且 workspace root 存在 `AGENT.md`
- **THEN** chat_history 恰有一条 System 消息，内容为「架构会话知识 + 空行 + `AGENT.md` 逐字」；MUST NOT 出现第二条 System 消息

#### Scenario: 仅单侧在场

- **WHEN** 分别以 `source = "archi"`（无 `AGENT.md`）与 `source = "debug"`（有 `AGENT.md`）发起会话
- **THEN** 前者 System 消息为知识 preamble 单条；后者为该文件内容逐字（与本变更前一致）；两者皆无时零 System 消息

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 以**名字清单为唯一单源**（`toolset` 位与 `SessionToolset` 枚举已退役，专用工具不再有第二套注入机制），并自本变更起以 **`ask` 位**承载 ask 工具的装配档位：

- **core 契约扩展**：`SessionInjections` 形态 SHALL 为 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec>, ask: Option<AskMode> }`——`tools` 为**本轮下发的工具名字清单**（`None` = infra 内建七件：read / grep / glob / ls / write / edit / bash），专用工具（`create_explore_note` / `archi_module` / `archi_search` / `archi_api` / `db_tables` / `db_query` / `propose_config`）以**名字进清单**而不经枚举追加；`finish` 为 `#[serde(default)]` 的收口档位词汇（`FinishSpec { EvaluatorChecklist, Decision }`，`Copy + Eq` 闭合枚举）；`ask` 为 `#[serde(default)]` 的 ask 装配档位（`AskMode { Generic, Flow }`，`Copy + Eq` 闭合枚举，见「ask 工具协议与阻塞应答通道」），**`None` 是显式禁用**（该会话拿不到 ask 工具），非「未实现」——装配层 MUST 逐臂显式置位。`preamble` 位按来源装配（生产者 = `source = "archi"` 的架构会话知识与 `source = "config"` 的配置协议知识），并由 loop 与 `AGENT.md` 合并为单条 System 消息（见「SDK 引擎系统提示词（AGENT.md 注入）」）；前端与自由字符串 MUST NOT 直接供给该位。**词汇封闭原则改写**：core 只承载**名字**、**收口档位词汇**与**ask 档位词汇**，工具定义（description / parameters）与准入档位（tier）唯一真源在 infra 的挂载清单（定义与档位均由挂载清单给定，见本条「tier 档位归属」）；非法工具名 SHALL 在装配层（compose）显式 `Err`（携非法名），MUST NOT 流进执行面。`PartialEq` / `Eq` 派生链 SHALL 完好（`FinishSpec` / `AskMode` 均为闭合枚举、清单为字符串向量——`ToolMount` 一类含 `serde_json::Value` 的结构 MUST NOT 进 core）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分四臂——① `provenance.source == "explore"`：注入 `tools = Some([read, grep, glob, ls, edit, bash, create_explore_note, db_tables, db_query])`（`write` 移除）+ `ask = Some(AskMode::Generic)`（两名只读 DB 工具见 desktop-agent-db-tools）；② `provenance.source == "archi"`：注入 `tools = Some([read, grep, glob, ls, write, edit, bash, archi_module, archi_search, archi_api])`（七内建全量 + 三个 archi 工具名，与 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」的 archi 来源臂单源一致）+ `preamble = Some(架构会话知识)` + `ask = Some(AskMode::Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析（`source_ref` 取任意值不改变该臂）；③ `provenance.source == "change"`：SHALL 按 `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}` 的尾三段解析 `(phase, role)` 并查 `(phase, role)` → archi 工具子集矩阵（矩阵单源见 desktop-archi-agent「archi 工具面与 (phase, role) 注入矩阵」），命中行把矩阵勾选的 archi 工具名并入七工具名字清单，**`db_tables` / `db_query` 两名字在矩阵之后恒 push（不分 `phase` / 不分矩阵命中，与 `ask` 位恒 `Flow` 同构）**，`finish` 位按 `role` 置位（`evaluator` → `Some(EvaluatorChecklist)`、`decision` → `Some(Decision)`、其余 `None`），**`ask` 位恒 `Some(AskMode::Flow)`（不分 role / 不分矩阵命中）**，未命中行与**解析失败（畸形 / 短段 / 未知 phase）**SHALL 恒零 archi 工具（fail-closed）；④ `provenance.source == "config"`：注入 `tools = Some([read, grep, glob, ls, bash, propose_config])`（**无 `write` / `edit`、无 db 工具**）+ `preamble = Some(配置协议知识)`（生产者与内容见 `desktop-config-agent`「配置协议知识的静态 preamble」）+ `finish = None` + `ask = Some(AskMode::Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析。其余来源（debug 等，即上述四臂之外者）SHALL 注入 `tools = None`（内建七件）+ `preamble = None` + `finish = None` + **`ask = Some(AskMode::Generic)`**——MUST NOT 以 `SessionInjections::default()`（其 `ask` 为 `None`）作为兜底装配，否则通用会话静默失去 ask。四臂的每个工具名 SHALL 在返回前对 infra 挂载清单做成员校验（未知名 `Err`）。
- **loop 组装与硬禁用**：definitions SHALL 由名字清单查 infra 挂载清单组装（`definitions(injections, prompt)`），并按两个**互不依赖**的位追加动态挂载项——`ask` 位为 `Some(mode)` 时追加 `AskUserQuestion`（`ToolTier::Protocol`，执行体 `ToolExec::Ask(mode)`），`finish` 位为 `Some(spec)` 时追加 `finish_step`（`ToolTier::Protocol`，执行体 `ToolExec::FinishStep(spec)`）；两条追加分支 MUST NOT 互相引用（无合取判据——`finish = None` 的会话仍有 ask，`ask = None` 的会话仍有 `finish_step`）。`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从清单移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具（含 ask）的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root + ask 注册表句柄 + core session id），sdk loop 经 `LoopTurn` 携带的上下文路由；per-turn 铸造判据 SHALL 为**名字清单包含**（`create_explore_note` 在场即铸笔记上下文；`archi_module` / `archi_search` / `archi_api` 任一在场即铸 archi 上下文；`db_tables` / `db_query` 任一在场即铸 `DbContext`——全局库句柄经 `WorkspaceStores::global_shared()`、workspace 库句柄取会话 workspace root 的库实例，change 会话 cwd 是 worktree 但库身份锚恒主仓 root；`propose_config` 在场即铸 `ConfigContext`——其 `root` 取 **`PumpPayload.workspace_root`（主仓 root）**，MUST NOT 取 exec root / worktree，见 `desktop-config-agent`「`ConfigContext` 铸造与 root 语义」）与**位存在**（`ask` 位为 `Some` 即铸 ask 上下文），MUST NOT 用枚举比较或自由字符串嗅探。`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **tier 档位归属**：`create_explore_note` SHALL 归写面档位（`ToolTier::Write`：Default 拒绝、AcceptEdits / BypassPermissions 放行）；三个 archi 只读工具（`archi_module` / `archi_search` / `archi_api`）SHALL 归只读面（`ToolTier::Read`：三档全放行）；`db_tables` / `db_query` SHALL 同归只读面（`ToolTier::Read`：三档全放行；无路径参数，沙箱改写面对其恒原样返回，写入保护零介入——契约细则见 desktop-agent-db-tools）；`propose_config` SHALL 同归只读面（`ToolTier::Read`：三档全放行；无路径参数，`tools::input_path` 对其恒 `None`——候选里的任意键名不进沙箱改写面，见 `desktop-config-agent`「`propose_config` 工具契约」）；`finish_step` 与 `AskUserQuestion` SHALL 归 `ToolTier::Protocol`（三档全放行——询问用户与投递决策不写盘、不执行命令，与权限档正交）。档位准入由 tier 比较单点表达（见「SDK 引擎静态权限与路径沙箱」），MUST NOT 恢复按工具名的常量清单。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note / db_tables / db_query / AskUserQuestion（无 `write`），`tools` 名字清单逐名一致（不含 ask 名——ask 按位追加，不占名字清单），`ask = Some(Generic)`，`RunStarted.tools` 逐名一致上报该面

#### Scenario: 幻觉 write 调用被拒

- **WHEN** explore 会话中模型发出已从清单移除的 `write` 工具调用
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

#### Scenario: 非 explore 来源仍获 ask

- **WHEN** 以 `source="debug"`（或不传来源）发起会话
- **THEN** definitions 与 `RunStarted.tools` 为内建七工具（含 `write`）+ `AskUserQuestion`，`finish` 位为 `None`（无 `finish_step`），`ask = Some(Generic)`；成员校验对七工具与 ask 放行，七工具行为与注入机制上线前一致

#### Scenario: change 来源按矩阵注入 archi 工具面

- **WHEN** 以 `source="change"` 且 `source_ref` 为 `<change_id>/dev-design/executor/1` 发起会话
- **THEN** 名字清单为七工具 + `archi_module` + `archi_search`（矩阵命中行勾选者）+ `db_tables` + `db_query`；definitions 含四名定义 + `AskUserQuestion`，`RunStarted.tools` 逐名一致上报；`finish` 位为 `None`、`ask = Some(Flow)`

#### Scenario: 矩阵未命中行与畸形 source_ref fail-closed

- **WHEN** 分别以 `<change_id>/test-gen/executor/1`、`<change_id>/implement/executor`（段数不足）与空 `source_ref` 发起 change 会话
- **THEN** 三者均落零 archi 工具（七工具 + `db_tables` / `db_query` 名字清单），不抛装配错误、不回退为全量 archi 工具；`ask` 仍为 `Some(Flow)`（ask 位不随矩阵）

#### Scenario: archi 来源装配面

- **WHEN** 以 `source="archi"`、`source_ref` 为 null 发起会话并审查该轮装配产物
- **THEN** `tools` 名字清单为七工具（含 `write`）+ `archi_module` / `archi_search` / `archi_api`（三 archi 工具，与 `desktop-archi-agent` 的 archi 来源臂单源一致）、`preamble` 为架构会话知识、`ask = Some(Generic)`；definitions 为该十项 + `AskUserQuestion`，`RunStarted.tools` 逐名一致

#### Scenario: config 来源装配面

- **WHEN** 以 `source="config"`、`source_ref` 为 null 发起会话并审查该轮装配产物
- **THEN** `tools` 名字清单为 `read` / `grep` / `glob` / `ls` / `bash` / `propose_config` 六名（**无** `write` / `edit`、无 `db_tables` / `db_query`、无 archi 工具）、`preamble` 为配置协议知识、`finish` 位为 `None`、`ask = Some(Generic)`；definitions 为该六项 + `AskUserQuestion`，`RunStarted.tools` 逐名一致；该臂不进入 `source_ref` 相位解析（`source_ref` 取任意值不改变下发面）

#### Scenario: Explore 臂零回归

- **WHEN** 以 `source="explore"` 发起会话并审查装配产物
- **THEN** 六工具 + `create_explore_note` + `db_tables` / `db_query` 的名字清单与本次变更前 definitions 的可观察产出逐条一致（成员校验 / 硬禁用 / `RunStarted.tools` 上报三条语义零改写），差异恰为多下发的 `AskUserQuestion` 与只读 DB 工具两件

#### Scenario: 非法工具名装配层显式拒绝

- **WHEN** 以任何来源构造出含挂载清单外名字的注入面（如 `tools = Some(["read", "fly"])`）
- **THEN** 装配层显式 `Err`（文案携非法名与外来源），会话不建立、不进执行面

#### Scenario: ask 位与 finish 位四组合独立

- **WHEN** 分别以 `(ask, finish)` 的四种组合构造注入面
- **THEN** `(Some(_), Some(_))` 下发两项；`(Some(_), None)` 只下发 `AskUserQuestion`；`(None, Some(_))` 只下发 `finish_step`；`(None, None)` 两项都不下发——两分支无合取判据，任一缺席不牵连另一项

#### Scenario: DB 工具两臂与另两臂的分野

- **WHEN** 分别以 `source="explore"`、`source="change"`（含 `test-gen/executor` 一类矩阵未命中行与畸形 `source_ref`）、`source="archi"`、`source="config"`、`source="debug"` 发起会话并审查该轮名字清单
- **THEN** 前两臂含 `db_tables` / `db_query`（change 臂不分 `phase` / 不分矩阵命中，与 `ask` 位恒在同理），后三臂不含；未下发两工具的会话调用它们一律走成员校验拒绝（`tool_unavailable` + `is_error`，run 不中断、不触执行体）


## Module Contract

### 模块增量：config 会话臂与 `propose_config` 工具面（config-agent-session）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|----------|
| `crates/infra/agent/src/compose.rs`（扩） | 注入装配第四臂 | `assemble_injections` 增 `source == "config"` 臂（`CONFIG_SOURCE`）：`tools = Some([read, grep, glob, ls, bash, propose_config])` + `preamble = config_knowledge::knowledge()` + `finish = None` + `ask = Some(Generic)`；签名不变、成员校验语义不变；doc 三臂转四臂 |
| `crates/infra/agent/src/config_knowledge.rs` + `config-knowledge.md`（新） | 知识 preamble 生产者 | `knowledge() -> Option<String>`：静态骨架（`include_str!`）+ 派生词表（`config::known_keys` / `TestFramework::ALL`）+ 使用指引；零入参、零 IO、零占位符；失败只丢 `preamble` 位 |
| `crates/infra/agent/src/sdk/config_propose.rs`（新） | 提案工具执行体 | 两模式（只读盘面 / 候选校验回环）；输出单条 file-face JSON 字符串；`changes` / `summary` 后端算；零副作用 |
| `crates/infra/agent/src/sdk/mount.rs`（扩） | 挂载清单 | `PROPOSE_CONFIG_TOOL` 常量 + 第 14 项挂载（`ToolTier::Read`、`ToolExec::Config`）；`with_capacity(13)` → 14 与两段 doc 随动；`TOOL_NAMES` 七件不动 |
| `crates/infra/agent/src/sdk/tools.rs`（扩） | 沙箱豁免 | `input_path` 无路径白名单补 `propose_config`（候选中任意键名不被当路径改写） |
| `crates/infra/agent/src/sdk/runner.rs` + `sdk/loop.rs`（扩） | per-turn 铸造与分派 | `ConfigContext { root }` + `mint_config_context(names, payload.workspace_root)`（名字判据）；`LoopTurn` 增字段 + `ToolExec::Config` 分派臂；上下文缺席以 `is_error` 回灌 |
| `crates/core/agent/**` | 零改动面 | `SessionInjections` 四字段形态零扩展（`propose_config` 走名字清单通道）；事件词汇零扩展 |
| `crates/infra/agent/src/sdk/{policy,sandbox,protect}` | 零改动面 | 三档决策表判据零改（期望随挂载名数 13 → 14 随动）；`Read` 档天然覆盖 `propose_config`；写保护零改动 |
