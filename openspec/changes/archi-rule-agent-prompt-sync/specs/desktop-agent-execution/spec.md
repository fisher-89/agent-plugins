# desktop-agent-execution Specification (Delta)

## ADDED Requirements

### Requirement: 自然收敛前的架构判据自检与收敛事件序

SDK 引擎 loop SHALL 在**自然收敛**收口前执行一次会话级的收口前自检，并把结果作为一条 `SystemNotice` 落在该轮 `TurnDone` **之前**；该机制 SHALL 对模型不可见（不回灌、不催轮、不发新请求），且 MUST NOT 改变任何收敛语义：

- **事件序**：`Message`（终答密封）→ `SystemNotice{subtype:"archi_preclose"}`（或 `archi_preclose_skipped`）→ `TurnDone`。notice MUST 先于 `finish_success` 发出；`TurnDone` 的 `subtype` / `is_error` / `num_turns` / `usage` 与本变更前逐字一致（自检结论 MUST NOT 进入 `TurnDone`）。
- **挂点唯一**：只在「无 tool_calls ∧ 非 `pause_turn`」自然收敛支执行一次；空 choice / 输出截断 / 异常终局（拒答 / 未知终局）/ `stop_requested`（轮间 / 轮头 / 续跑间隙）/ `pause_turn` 续跑到顶五个出口 MUST NOT 执行。触发布尔与判据由会话侧决定（见 `desktop-archi-agent`「收口前引擎自检（架构判据读数）」），本能力只承载「何时跑」与「事件序」。
- **零请求增量**：自检 MUST NOT 触发模型请求、MUST NOT 追加对话史消息（`history` 逐条不变）、MUST NOT 增加轮次（`turn_index` 不因它推进）。
- **阻塞面**：判据为 CPU / IO 型同步工作，SHALL 经 `tokio::task::spawn_blocking` 执行；loop 的 async 泵 MUST NOT 被其阻塞（自检期间不得阻断事件通道的其它在途发送）。
- **fail-open**：自检任何失败（判据返回 `Err` / 任务 panic 兜底）SHALL 以 `SystemNotice{subtype:"archi_preclose_skipped", payload:{reason}}` 留痕后照常收敛；MUST NOT 阻断、MUST NOT 把失败升级为 `api_error`、MUST NOT 新增收敛退出码或事件词汇。
- **subtype 开放词典**：`archi_preclose` 与 `archi_preclose_skipped` 为 `SystemNotice` 开放词典的两个新词（与 `api_retry` / `context_pruned` / `output_truncated` 同列），MUST NOT 引入新的信封类型或事件变体。
- **非 archi 会话零射程**：判据为假（无会话 preamble）时会话 MUST NOT 产生任何 notice、MUST NOT 增加可观测事件（最多一行引擎日志）。

#### Scenario: notice 先于 TurnDone

- **WHEN** 触发判据命中的 archi 会话自然收敛
- **THEN** 该轮事件序为「密封 `Message` → `SystemNotice{archi_preclose}` → `TurnDone`」；`TurnDone` 字段与本变更前逐字一致；`history` 无新增消息

#### Scenario: 五出口只跑一支

- **WHEN** 分别构造自然收敛 / 空 choice / 输出截断 / 用户停止 / `pause_turn` 续跑到顶五个出口（触发判据置真）
- **THEN** 只有自然收敛支出 notice；其余四出口零 `archi_preclose` 事件，收敛语义与既有断言逐条一致

#### Scenario: 失败面照常收口

- **WHEN** 自检执行体返回 `Err`（如架构域配置非法）
- **THEN** 流出 `SystemNotice{archi_preclose_skipped}`（载荷含成因）后 `TurnDone{subtype:"success", is_error:false}` 照常；run 不中断、无 `api_error`、无额外请求

#### Scenario: 不阻塞事件通道

- **WHEN** 自检执行期间检查事件通道
- **THEN** 该轮其它在途事件照常送达（自检在 blocking 线程池执行，泵未被同步阻塞）；判据耗时经 notice 载荷的 `durationMs` 可见

## MODIFIED Requirements

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 协议 SHALL **双值可选且双首公民**：`openai`（openai-compatible 端点——GLM / DeepSeek / Kimi / 自部署）与 `anthropic`（Anthropic Messages 端点及兼容网关）；协议由 provider 记录 `protocol` 声明、经 `EngineConfig.protocol` 抵达引擎（见「SDK 引擎模型协议选臂（openai / anthropic）」）——原「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」的限制在本变更偿还
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层；**工具执行体 SHALL 允许阻塞等待外部输入**（ask 工具的 turn 内应答等待，见「ask 工具协议与阻塞应答通道」），阻塞期间该轮不收敛、不产生额外模型请求
- **流面双层交付**：流内 `Text` / `Reasoning` 片段增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面；rig 0.43 流事件模型为部分生命周期——Start / Arguments / End 为簿记事件不出增量）；轮末 `finish()` 折叠的 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：默认（无会话注入）工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；会话级注入 SHALL 以**名字清单**收窄该面、以专用工具**名字**追加受控工具面（工具定义与准入档唯一真源在 infra 挂载清单，见「会话级工具面注入与 explore 笔记工具」）；**`AskUserQuestion` 为基线交互工具**——与七工具并列的能力，MUST NOT 进 `TOOL_NAMES`（它不是「名字清单可收窄的七件之一」），其在场与否由 `SessionInjections.ask` 位决定；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求系统提示词 SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections.preamble` 位按来源装配（唯一生产者 = archi 臂），其余四位（`tools` / `finish` / `ask` / `protect`）同样按来源装配（见「会话级工具面注入与 explore 笔记工具」；`toolset` 位已随枚举退役）
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

#### Scenario: 注入位词面为五位

- **WHEN** 审查 `SessionInjections` 字段面与本条工具面约定
- **THEN** 只出现 `preamble` / `tools` / `finish` / `ask` / `protect` 五位；产品源码（`crates/**` / `src/**`）中 `toolset` 与 `SessionToolset` 零命中

#### Scenario: 阻塞执行体不额外消耗请求

- **WHEN** 模型在一轮内调用 `AskUserQuestion` 并等待用户应答
- **THEN** 阻塞期间该轮不收敛、不发起新的模型请求；应答回灌后同一轮的下一次模型请求照常发出（`turn_index` 递增一次，`session_id` 不变）

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 以**名字清单为唯一单源**（`toolset` 位与 `SessionToolset` 枚举已退役，专用工具不再有第二套注入机制），并自本变更起以 **`protect` 位**承载会话级写入禁区（第三规则来源；`ask` 位承载 ask 装配档位）：

- **core 契约扩展**：`SessionInjections` 形态 SHALL 为 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec>, ask: Option<AskMode>, protect: Option<Vec<SessionProtectRule>> }`——`tools` 为**本轮下发的工具名字清单**（`None` = infra 内建七件：read / grep / glob / ls / write / edit / bash），专用工具（`create_explore_note` / `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `db_tables` / `db_query`）以**名字进清单**而不经枚举追加；`finish` 为 `#[serde(default)]` 的收口档位词汇（`FinishSpec { EvaluatorChecklist, Decision }`，`Copy + Eq` 闭合枚举）；`ask` 为 `#[serde(default)]` 的 ask 装配档位（`AskMode { Generic, Flow }`，`Copy + Eq` 闭合枚举，见「ask 工具协议与阻塞应答通道」），**`None` 是显式禁用**（该会话拿不到 ask 工具），非「未实现」——装配层 MUST 逐臂显式置位。`preamble` 位按来源装配（唯一生产者 = `source = "archi"` 的架构会话知识），并由 loop 与 `AGENT.md` 合并为单条 System 消息（见「SDK 引擎系统提示词（AGENT.md 注入）」）；前端与自由字符串 MUST NOT 直接供给该位。**`protect` 位**为 `#[serde(default)]` 的会话级写入禁区清单（`SessionProtectRule { glob: String, reason: String }`，`reason` 携 `%s` / `%t` 占位符），**`None` = 不追加**，语义与装配单点见 `desktop-write-protection`「会话级写入禁区（相位锁）」；该位 SHALL NOT 派生 `specta::Type`（引擎侧会话词汇不出线 IPC，与 `FinishSpec` / `AskMode` 同款），为此**不进 TS bindings**。**词汇封闭原则改写**：core 只承载**名字**、**收口档位词汇**、**ask 档位词汇**与**纯数据规则串**，工具定义（description / parameters）与准入档位（tier）唯一真源在 infra 的挂载清单（定义与档位均由挂载清单给定，见本条「tier 档位归属」）；非法工具名 SHALL 在装配层（compose）显式 `Err`（携非法名），MUST NOT 流进执行面。`PartialEq` / `Eq` 派生链 SHALL 完好（`FinishSpec` / `AskMode` 均为闭合枚举、清单为字符串向量、`SessionProtectRule` 为两字符串纯数据——`ToolMount` 一类含 `serde_json::Value` 的结构 MUST NOT 进 core）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分三臂——① `provenance.source == "explore"`：注入 `tools = Some([read, grep, glob, ls, edit, bash, create_explore_note, db_tables, db_query])`（`write` 移除）+ `ask = Some(AskMode::Generic)`（两名只读 DB 工具见 desktop-agent-db-tools）；② `provenance.source == "archi"`：注入 `tools = Some([read, grep, glob, ls, write, edit, bash, archi_module, archi_search, archi_api, archi_drift, archi_scaffold])`（七内建全量 + 五个 archi 工具名，与 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」的 archi 来源臂单源一致）+ `preamble = Some(架构会话知识)` + `ask = Some(AskMode::Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析（`source_ref` 取任意值不改变该臂）；③ `provenance.source == "change"`：SHALL 按 `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}` 的尾三段解析 `(phase, role)` 并查 `(phase, role)` → archi 工具子集矩阵（矩阵单源见 desktop-archi-agent「archi 工具面注入与 (phase, role) 矩阵」），命中行把矩阵勾选的 archi 工具名并入七工具名字清单，**`db_tables` / `db_query` 两名字在矩阵之后恒 push（不分 `phase` / 不分矩阵命中，与 `ask` 位恒 `Flow` 同构）**，`finish` 位按 `role` 置位（`evaluator` → `Some(EvaluatorChecklist)`、`decision` → `Some(Decision)`、其余 `None`），**`ask` 位恒 `Some(AskMode::Flow)`（不分 role / 不分矩阵命中）**，未命中行与**解析失败（畸形 / 短段 / 未知 phase）**SHALL 恒零 archi 工具（fail-closed）。其余来源（debug 等）SHALL 注入 `tools = None`（内建七件）+ `preamble = None` + `finish = None` + **`ask = Some(AskMode::Generic)`**——MUST NOT 以 `SessionInjections::default()`（其 `ask` 为 `None`）作为兜底装配，否则通用会话静默失去 ask。**三臂的 `protect` 位 SHALL 由装配层单点 `model_write_lock(provenance)` 逐臂置位**（archi 臂与 change 臂的 `(proposal|dev-design, executor)` → `None`；其余一律 `Some(一条)`；规则 glob 由装配层经 `foundation::layout` 派生，`core` 不解释路径语义——判定矩阵归 `desktop-write-protection`）。三臂的每个工具名 SHALL 在返回前对 infra 挂载清单做成员校验（未知名 `Err`）。
- **loop 组装与硬禁用**：definitions SHALL 由名字清单查 infra 挂载清单组装（`definitions(injections, prompt)`），并按两个**互不依赖**的位追加动态挂载项——`ask` 位为 `Some(mode)` 时追加 `AskUserQuestion`（`ToolTier::Protocol`，执行体 `ToolExec::Ask(mode)`），`finish` 位为 `Some(spec)` 时追加 `finish_step`（`ToolTier::Protocol`，执行体 `ToolExec::FinishStep(spec)`）；两条追加分支 MUST NOT 互相引用（无合取判据——`finish = None` 的会话仍有 ask，`ask = None` 的会话仍有 `finish_step`）。`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从清单移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具（含 ask）的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root + ask 注册表句柄 + core session id），sdk loop 经 `LoopTurn` 携带的上下文路由；per-turn 铸造判据 SHALL 为**名字清单包含**（`create_explore_note` 在场即铸笔记上下文；`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` **任一**在场即铸 archi 上下文；`db_tables` / `db_query` 任一在场即铸 `DbContext`——全局库句柄经 `WorkspaceStores::global_shared()`、workspace 库句柄取会话 workspace root 的库实例，change 会话 cwd 是 worktree 但库身份锚恒主仓 root）与**位存在**（`ask` 位为 `Some` 即铸 ask 上下文），MUST NOT 用枚举比较或自由字符串嗅探。`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **tier 档位归属**：`create_explore_note` SHALL 归写面档位（`ToolTier::Write`：Default 拒绝、AcceptEdits / BypassPermissions 放行）；五个 archi 只读工具（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`）SHALL 归只读面（`ToolTier::Read`：三档全放行）；`db_tables` / `db_query` SHALL 同归只读面（`ToolTier::Read`：三档全放行；无路径参数，沙箱改写面对其恒原样返回，写入保护零介入——契约细则见 desktop-agent-db-tools）；`finish_step` 与 `AskUserQuestion` SHALL 归 `ToolTier::Protocol`（三档全放行——询问用户与投递决策不写盘、不执行命令，与权限档正交）。档位准入由 tier 比较单点表达（见「SDK 引擎静态权限与路径沙箱」），MUST NOT 恢复按工具名的常量清单。

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
- **THEN** definitions 与 `RunStarted.tools` 为内建七工具（含 `write`）+ `AskUserQuestion`，`finish` 位为 `None`（无 `finish_step`），`ask = Some(Generic)`，`protect = Some(相位锁一条)`（非 white-listed 来源一律挂锁）；成员校验对七工具与 ask 放行，七工具行为与注入机制上线前一致

#### Scenario: change 来源按矩阵注入 archi 工具面

- **WHEN** 以 `source="change"` 且 `source_ref` 为 `<change_id>/dev-design/executor/1` 发起会话
- **THEN** 名字清单为七工具 + `archi_module` + `archi_search` + `archi_drift` + `archi_scaffold`（矩阵命中行勾选者）+ `db_tables` + `db_query`；definitions 含六名定义 + `AskUserQuestion`，`RunStarted.tools` 逐名一致上报；`finish` 位为 `None`、`ask = Some(Flow)`、`protect = None`（写权相位不挂锁）

#### Scenario: 模型写权位逐臂置位

- **WHEN** 分别以 `source="archi"`、`<id>/proposal/executor/1`、`<id>/dev-design/executor/1`、`<id>/dev-design/evaluator/1`、`<id>/implement/executor/1`、`<id>/test-gen/executor/1`、畸形 `source_ref`、`source="explore"`、`source="debug"` 发起会话
- **THEN** 前三者 `protect = None`（写权通路）；后六者 `protect = Some([一条模型目录锁])`（fail-closed 方向）；锁 glob 与拒绝文案归 `desktop-write-protection`

### Requirement: loop 守卫链写入保护闸与工作区根注入缝

SDK 引擎的 loop 分发链 SHALL 在既有三段守卫之后、执行之前增设第四段「写入保护闸」，并由一条与 `with_explore_note_store` 同构的 builder 缝注入主仓 root：

- **四段顺序不变量**：`① 成员校验 → ② 档位门（policy）→ ③ 路径沙箱（sandbox_rewrite）→ ④ 写入保护闸 → 执行`。保护闸 MUST 位于沙箱改写**之后**（此时 `path` 已是 canonical 绝对路径，判定无需再做 IO 级规范化），MUST 位于 `tools::execute` / `explore_note` 执行**之前**；MUST NOT 提前到沙箱之前，MUST NOT 下沉进 `tools::execute`（那是纯函数，塞入读配置会让签名长出 IO 依赖，且拒绝形态只能由 loop 合成）。
- **判定输入**：`write` / `edit` 取沙箱改写后的 `path` 字段；`bash` 取 `command` 原文（命令内容不可 canonicalize）与会话执行根 `LoopTurn.cwd`（相对路径解析基座）。
- **拒绝形态复用既有 `deny()`**：`SystemNotice{subtype:"permission_denied", payload:{tool, reason}}` + 与 tool_use 同 id 的 is_error `ToolResult`，**run 不中断**（模型换路自愈）。MUST NOT 新增 subtype、MUST NOT 新增事件词汇、MUST NOT 新增收敛退出码；保护闸 MUST NOT 自行发事件（拒绝串内容归 desktop-write-protection）。
- **档位面**：保护闸在任何 `permission_mode` 下均生效（Default 档本就拒绝 write / edit / bash；AcceptEdits 与 BypassPermissions 档下受保护文件仍被拒）。MUST NOT 修改 policy 三档决策表、MUST NOT 引入审批通道。
- **装配时机**：保护规则 SHALL 每 run 装配一次（会话泵起播时 `protect::load`）**并就地合并会话级规则**（`with_session(injections.protect)`，见下条），随 `LoopTurn.protect` 下发（与 `defense` / `retry` / `liveness` 的「装配点算好、loop 纯消费」形态同构）；loop 内 MUST NOT 读盘、MUST NOT 每 tool_call 重读配置。
- **规则来源三处与位次**：`ProtectRules` SHALL 由三个来源按次装配——**内置（工作区 `config.json`）→ 会话（相位锁）→ 用户（自选）**，首个命中即定（位次即优先级，MUST NOT 拼接多条 reason）。会话级规则的语义与范围归 `desktop-write-protection`「会话级写入禁区（相位锁）」；本能力只保证它经 `SessionInjections.protect` → `with_session` → `LoopTurn.protect` 同路抵达判定面（`load` 的签名与「唯一 IO 点」语义逐字不变）。
- **工作区根通道（新 builder 缝）**：root 只在命令层显式在手（store 句柄无 root 字段），故 SHALL 沿既有先例逐层下行：

```
compose_turn(stores, registry, streams, store, agent, workspace_root: &Path)
  → EngineFacade::with_workspace_root(root)
  → SdkRunner.workspace_root: Option<PathBuf>
  → PumpPayload.workspace_root → 泵起播时 protect::load(…).with_session(injections.protect)
  → LoopTurn.protect: ProtectRules
```

  `workspace_root` MUST 为**主仓** root（命令层 `root` 参数），MUST NOT 取会话执行根 `SessionCtx.workspace_root`（change 流程下为 worktree）——工作区配置的权威在主仓；会话规则的 glob 由装配层派生（与 root 无关，worktree 与主仓同样命中）。
- **缺席语义**：`workspace_root` 为 `None`（未注入）时 SHALL 只装载内置规则（用户规则缺席），MUST NOT 关闭保护闸、MUST NOT 退化为空规则集；`protect` 位为 `None` 时会话规则缺席（不追加一条空 glob）。
- **射程边界**：专用工具 `create_explore_note` MUST NOT 经保护闸（专用工具不在保护射程内为既有裁定，一致性缺口留痕于 desktop-write-protection）；五个 archi 只读工具同样不在射程（`_ => None` 分派臂）。
- **错误归属次序**：沙箱拒绝（`sandbox_denied`）SHALL 优先于保护拒绝（`permission_denied`）——逃逸路径先按沙箱报错，不被保护闸抢先。

#### Scenario: 四段顺序与拒绝形态

- **WHEN** BypassPermissions 档下模型调用 `write` 指向 `<root>/.dev-team/config.json`（路径在 root 内）
- **THEN** 事件流含 `SystemNotice{subtype:"permission_denied", payload:{tool:"write", reason:…}}` 与同 id 的 is_error ToolResult（内容为同一拒绝串）；`tools::execute` 未被调用（文件未被写）；run 不中断，同轮后续工具调用照常执行

#### Scenario: 沙箱先于保护闸

- **WHEN** 模型调用 `write` 指向 `<root>/../outside/.dev-team/config.json`（越出 workspace root）
- **THEN** 拒绝理由为沙箱成因（`subtype:"sandbox_denied"`），而非 `permission_denied`——两段闸的错误归属不重叠、不抢报

#### Scenario: 每 run 一次装配与 loop 零 IO

- **WHEN** 在一次 run 进行中修改 `.dev-team/config.json` 的 `write_protection` 与会话注入面，随后同一 run 内继续调用工具
- **THEN** 本轮行为不随修改改变（规则在泵起播时已装配）；`sdk/loop.rs` 内无 `config::load` / `fs` / `canonicalize` 调用点

#### Scenario: 主仓口径与 worktree 执行根

- **WHEN** change 流程会话的执行根为 worktree（`SessionCtx.workspace_root` = worktree 路径），主仓 root 另经 builder 缝注入
- **THEN** 用户规则取自主仓 `config.json`；写 worktree 内 `.dev-team/config.json` 仍被内置规则拒（内置 glob 不区分主仓 / worktree）；相位锁对 worktree 内的模型目录同样命中（会话规则 glob 与执行根无关）

#### Scenario: 未注入只内置

- **WHEN** 门面未注入 `workspace_root`（`None`）
- **THEN** 保护闸仍生效且只含内置规则（写工作区配置被拒），用户配置的 glob 不生效；会话规则若在位仍照常生效（其来源不是配置读取）

#### Scenario: 专用工具直通

- **WHEN** explore 会话调用 `create_explore_note`，且用户配置保护了该工具的目标目录
- **THEN** 调用不被保护闸拦截（专用工具直通为知情边界），`write` / `edit` 对同目录的写入仍被拒

#### Scenario: 会话规则位次可辨

- **WHEN** 用户配置显式保护了模型目录（`write_protection.files` 命中），且当前会话不在写权白名单内
- **THEN** 拒绝串取**会话规则**（相位锁文案携指路）而非用户规则文案——会话规则位次在内置之后、用户之前；内置 `config.json` 规则仍恒最先

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/runner.rs` | 会话契约（第五位） | `SessionInjections { preamble, tools, finish, ask, protect }`；`protect: Option<Vec<SessionProtectRule>>` 为 `#[serde(default)]` 的纯数据（`SessionProtectRule { glob, reason }`），`Clone + Eq`、**无 `specta::Type`**（不进 TS bindings）；`load` / round-trip / 缺字段读 `None` 语义 additive |
| `crates/infra/agent/src/compose.rs` | 装配单点 | `assemble_injections` 三臂 + `injected(...)` 签名携带 `protect`；`model_write_lock(provenance)` 为唯一置位点（archi 臂与 `(proposal｜dev-design, executor)` → `None`，其余 → `Some([一条])`）；`archi_matrix` 8 行含两新工具列；archi 臂名字清单五 archi 名 |
| `crates/infra/agent/src/sdk/runner.rs` | 装配挂载 | `ProtectRules::load(workspace_root).with_session(injections.protect)`；`mint_archi_context` 判据含五名；`with_workspace_root` 通道逐字不变 |
| `crates/infra/agent/src/sdk/loop.rs` | 守卫链 + 收敛挂点 | 四段顺序（成员校验 → 档位 → 沙箱 → 保护闸）逐字不变；自然收敛支 `preclose::run` 先于 `finish_success`；其余四出口零调用 |
| `crates/infra/agent/src/sdk/preclose.rs`（新） | 收口自检执行体 | `spawn_blocking`；`CancelFlag::never()`；fail-open → `archi_preclose_skipped`；零额外请求、零历史追加 |
| `crates/infra/agent/src/sdk/protect/{mod,rules,reason}.rs` | 第三规则来源（详见 `desktop-write-protection`） | `with_session` 合并 + `ReasonKind::Session` + 模板出口；`load` 签名不变 |

### API: `SessionInjections`（`crates/core/agent/src/runner.rs`）

| 项 | 变化 | 描述 |
|---|---|---|
| `SessionInjections.protect` | 新增（第五位） | `Option<Vec<SessionProtectRule>>`，serde additive；`None` = 不追加会话规则 |
| `SessionProtectRule` | 新增类型 | `{ glob: String, reason: String }`；`reason` 携 `%s` / `%t` 占位符；core 不解释路径语义 |
| `FinishSpec` / `AskMode` / `tools` | 零改动 | 词汇与语义逐字不变 |

### Function: `crates/infra/agent/src/sdk/loop.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `run`（自然收敛支） | 改 | 先 `preclose::run(&sender, turn, archi_touched)` 再 `finish_success`；`archi_touched` 在工具执行前由写入面调用置位 |
| 其余四出口 | 零改动 | 空 choice / 截断 / 异常终局 / 停止 / `pause_turn` 到顶的收敛路径与文案逐字不变 |
| 守卫链 | 零改动 | 四段顺序、拒绝形态、`deny()` 双面传递逐字不变（会话规则只是 `ProtectRules` 的一条来源） |
