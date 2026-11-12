# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 以**名字清单为唯一单源**（`toolset` 位与 `SessionToolset` 枚举已退役，专用工具不再有第二套注入机制），并自本变更起以 **`ask` 位**承载 ask 工具的装配档位：

- **core 契约扩展**：`SessionInjections` 形态 SHALL 为 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec>, ask: Option<AskMode> }`——`tools` 为**本轮下发的工具名字清单**（`None` = infra 内建七件：read / grep / glob / ls / write / edit / bash），专用工具（`create_explore_note` / `archi_module` / `archi_search`）以**名字进清单**而不经枚举追加；`finish` 为 `#[serde(default)]` 的收口档位词汇（`FinishSpec { EvaluatorChecklist, Decision }`，`Copy + Eq` 闭合枚举）；`ask` 为 `#[serde(default)]` 的 ask 装配档位（`AskMode { Generic, Flow }`，`Copy + Eq` 闭合枚举，见「ask 工具协议与阻塞应答通道」），**`None` 是显式禁用**（该会话拿不到 ask 工具），非「未实现」——装配层 MUST 逐臂显式置位。`preamble` 位按来源装配（唯一生产者 = `source = "archi"` 的架构会话知识），并由 loop 与 `AGENT.md` 合并为单条 System 消息（见「SDK 引擎系统提示词（AGENT.md 注入）」）；前端与自由字符串 MUST NOT 直接供给该位。**词汇封闭原则改写**：core 只承载**名字**、**收口档位词汇**与**ask 档位词汇**，工具定义（description / parameters）与准入档位（tier）唯一真源在 infra 的挂载清单（见「工具挂载单源与 tier 准入」）；非法工具名 SHALL 在装配层（compose）显式 `Err`（携非法名），MUST NOT 流进执行面。`PartialEq` / `Eq` 派生链 SHALL 完好（`FinishSpec` / `AskMode` 均为闭合枚举、清单为字符串向量——`ToolMount` 一类含 `serde_json::Value` 的结构 MUST NOT 进 core）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分三臂——① `provenance.source == "explore"`：注入 `tools = Some([read, grep, glob, ls, edit, bash, create_explore_note])`（`write` 移除）+ `ask = Some(AskMode::Generic)`；② `provenance.source == "archi"`：注入 `tools = Some([read, grep, glob, ls, write, edit, bash, archi_module, archi_search])` + `preamble = Some(架构会话知识)` + `ask = Some(AskMode::Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析（`source_ref` 取任意值不改变该臂）；③ `provenance.source == "change"`：SHALL 按 `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}` 的尾三段解析 `(phase, role)` 并查 `(phase, role)` → archi 工具子集矩阵（矩阵单源见 desktop-archi-agent「archi 工具面与 (phase, role) 注入矩阵」），命中行把矩阵勾选的 archi 工具名并入七工具名字清单，`finish` 位按 `role` 置位（`evaluator` → `Some(EvaluatorChecklist)`、`decision` → `Some(Decision)`、其余 `None`），**`ask` 位恒 `Some(AskMode::Flow)`（不分 role / 不分矩阵命中）**，未命中行与**解析失败（畸形 / 短段 / 未知 phase）**SHALL 恒零 archi 工具（fail-closed）。其余来源（debug 等）SHALL 注入 `tools = None`（内建七件）+ `preamble = None` + `finish = None` + **`ask = Some(AskMode::Generic)`**——MUST NOT 以 `SessionInjections::default()`（其 `ask` 为 `None`）作为兜底装配，否则通用会话静默失去 ask。三臂的每个工具名 SHALL 在返回前对 infra 挂载清单做成员校验（未知名 `Err`）。
- **loop 组装与硬禁用**：definitions SHALL 由名字清单查 infra 挂载清单组装（`definitions(injections, prompt)`），并按两个**互不依赖**的位追加动态挂载项——`ask` 位为 `Some(mode)` 时追加 `AskUserQuestion`（`ToolTier::Protocol`，执行体 `ToolExec::Ask(mode)`），`finish` 位为 `Some(spec)` 时追加 `finish_step`（`ToolTier::Protocol`，执行体 `ToolExec::FinishStep(spec)`）；两条追加分支 MUST NOT 互相引用（无合取判据——`finish = None` 的会话仍有 ask，`ask = None` 的会话仍有 `finish_step`）。`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从清单移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具（含 ask）的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root + ask 注册表句柄 + core session id），sdk loop 经 `LoopTurn` 携带的上下文路由；per-turn 铸造判据 SHALL 为**名字清单包含**（`create_explore_note` 在场即铸笔记上下文；`archi_module` / `archi_search` 任一在场即铸 archi 上下文）与**位存在**（`ask` 位为 `Some` 即铸 ask 上下文），MUST NOT 用枚举比较或自由字符串嗅探。`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **tier 档位归属**：`create_explore_note` SHALL 归写面档位（`ToolTier::Write`：Default 拒绝、AcceptEdits / BypassPermissions 放行）；两个 archi 只读工具 SHALL 归只读面（`ToolTier::Read`：三档全放行）；`finish_step` 与 `AskUserQuestion` SHALL 归 `ToolTier::Protocol`（三档全放行——询问用户与投递决策不写盘、不执行命令，与权限档正交）。档位准入由 tier 比较单点表达（见「SDK 引擎静态权限与路径沙箱」），MUST NOT 恢复按工具名的常量清单。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note / AskUserQuestion（无 `write`），`tools` 名字清单逐名一致（不含 ask 名——ask 按位追加，不占名字清单），`ask = Some(Generic)`，`RunStarted.tools` 逐名一致上报该面

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
- **THEN** 名字清单为七工具 + `archi_module` + `archi_search`（矩阵命中行勾选者）；definitions 含两名定义 + `AskUserQuestion`，`RunStarted.tools` 逐名一致上报；`finish` 位为 `None`、`ask = Some(Flow)`

#### Scenario: 矩阵未命中行与畸形 source_ref fail-closed

- **WHEN** 分别以 `<change_id>/test-gen/executor/1`、`<change_id>/implement/executor`（段数不足）与空 `source_ref` 发起 change 会话
- **THEN** 三者均落零 archi 工具（七工具名字清单），不抛装配错误、不回退为全量 archi 工具；`ask` 仍为 `Some(Flow)`（ask 位不随矩阵）

#### Scenario: archi 来源装配面

- **WHEN** 以 `source="archi"`、`source_ref` 为 null 发起会话并审查该轮装配产物
- **THEN** `tools` 名字清单为七工具（含 `write`）+ `archi_module` / `archi_search`、`preamble` 为架构会话知识、`ask = Some(Generic)`；definitions 为该九项 + `AskUserQuestion`，`RunStarted.tools` 逐名一致

#### Scenario: Explore 臂零回归

- **WHEN** 以 `source="explore"` 发起会话并审查装配产物
- **THEN** 六工具 + `create_explore_note` 的名字清单与本次变更前 definitions 的可观察产出逐字相同（成员校验 / 硬禁用 / `RunStarted.tools` 上报三条语义零改写），唯一差异是多下发了 `AskUserQuestion`

#### Scenario: 非法工具名装配层显式拒绝

- **WHEN** 以任何来源构造出含挂载清单外名字的注入面（如 `tools = Some(["read", "fly"])`）
- **THEN** 装配层显式 `Err`（文案携非法名与外来源），会话不建立、不进执行面

#### Scenario: ask 位与 finish 位四组合独立

- **WHEN** 分别以 `(ask, finish)` 的四种组合构造注入面
- **THEN** `(Some(_), Some(_))` 下发两项；`(Some(_), None)` 只下发 `AskUserQuestion`；`(None, Some(_))` 只下发 `finish_step`；`(None, None)` 两项都不下发——两分支无合取判据，任一缺席不牵连另一项

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 协议 SHALL **双值可选且双首公民**：`openai`（openai-compatible 端点——GLM / DeepSeek / Kimi / 自部署）与 `anthropic`（Anthropic Messages 端点及兼容网关）；协议由 provider 记录 `protocol` 声明、经 `EngineConfig.protocol` 抵达引擎（见「SDK 引擎模型协议选臂（openai / anthropic）」）
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层；**工具执行体 SHALL 允许阻塞等待外部输入**（ask 工具的 turn 内应答等待，见「ask 工具协议与阻塞应答通道」），阻塞期间该轮不收敛、不产生额外模型请求
- **流面双层交付**：流内 `Text` / `Reasoning` 片段增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面；rig 0.43 流事件模型为部分生命周期——Start / Arguments / End 为簿记事件不出增量）；轮末 `finish()` 折叠的 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：默认（无会话注入）工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；会话级注入 SHALL 以**名字清单**收窄该面、以专用工具**名字**追加受控工具面（工具定义与准入档唯一真源在 infra 挂载清单，见「工具挂载单源与 tier 准入」与「会话级工具面注入与 explore 笔记工具」）；**`AskUserQuestion` 为基线交互工具**——与七工具并列的能力，MUST NOT 进 `TOOL_NAMES`（它不是「名字清单可收窄的七件之一」），其在场与否由 `SessionInjections.ask` 位决定；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求系统提示词 SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections.preamble` 位按来源装配（唯一生产者 = archi 臂），`tools` / `finish` / `ask` 位按来源装配（见「会话级工具面注入与 explore 笔记工具」；`toolset` 位已随枚举退役）
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

### Requirement: agent 执行命令面

`commands/exec/` SHALL 承载执行与查询命令，命令体 SHALL 收敛为内核公共 API 的薄包装（三件事纪律：参数转换 → 调用 → 错误映射）：

- 发起命令（现 `agent_start`）：参数转换后调用内核 `run_session` 形状 API——`New` 建立会话、`Continue{id}` 续轮（现 `resume_session_id` 语义平移）；编排（runner 组装、tee、落库、盖戳、收敛）SHALL 全部下沉内核，命令体 MUST NOT 编排。来源参数（`source` / `source_ref`）SHALL 写入会话归属（provenance 记录用）；`source` 不传时缺省 `debug`；`source_ref` SHALL 可携来源侧引用（如 change 详情通用 agent 面板传 change id——两段形态与 change 流程会话的四段 `{changeId}/{phase}/{role}/{attempt}` 精确匹配下不撞），仅用于归属记录与清单筛选，MUST NOT 用于寻址 / 重放（寻址仍走 `sessionId` 直查）；未传时为 null。cwd SHALL 由尾部可选参数 `cwd: Option<String>` 承载（位置索引 8，`None` → `root`）：`cwd` 只赋会话执行根（`SessionCtx.workspace_root`——工具路径沙箱根 / bash `current_dir` / 每轮 `AGENT.md` 读取根），`root` 的 store 身份半边 SHALL 恒为 workspace root（双 root 不变量，与 change 流程 `exec_root` / `store_root` 拆分同构——`cwd` MUST NOT 参与 `for_root`，否则会静默铸出第二份 workspace 库）；`cwd` 非空 SHALL 先做 `is_dir` 探测并以显式 `Err`（携路径）拒绝缺失目录，MUST NOT 让失败深埋进引擎工具调用；cwd MUST NOT 落会话配置快照（续话由来源侧恒显式携带；非来源侧续话不携 cwd 为已知缺口，见 desktop-change-chat）；引擎组装 SHALL 经内核组合根（见「引擎门面与 EngineKind 参数选择」），命令体 MUST NOT 直接构造具体引擎；装配 SHALL 把壳层托管的 ask 注册表句柄注入组合根（见「ask 工具协议与阻塞应答通道」）。
- 查询命令（现 `agent_runs` / `agent_run_events` / `agent_run_chain`）：演进为会话域查询薄包装（分页会话清单含聚合统计与 total 计数 / 会话转录重放 / 源过滤清单），参数含 root → workspace 库解析 → store/内核查询 → DTO；前端寻址与重放 SHALL 走 `session_detail` 单查（见「session_detail 单查面」）而非按来源反查取最近一条；另 SHALL 提供 `list_change_chat_sessions(root, id, offset, limit) -> SessionPage` 薄包装（等价 `session_query(source="change", source_ref=<id>)` 的分页清单——`source` 词常量收在命令层，前端零来源词汇；blank root → 空页；见 desktop-change-chat「change 会话归属与新会话 / 历史会话」）
- 终止命令（现 `agent_stop`）：见「agent_stop 终止与提前 resolve」；root 寻址保留
- **应答命令（新 `agent_answer`）**：签名 SHALL 为 `agent_answer(root: String, session_id: String, text: String) -> Result<(), String>`，命令体为**薄包装**——blank `root` / blank `session_id` SHALL 显式 `Err`（文案携字段名），否则转投壳层托管的 ask 注册表（见「ask 工具协议与阻塞应答通道」），命令体 MUST NOT 触碰会话状态机、MUST NOT 广播通知、MUST NOT 清理流程停等态。该命令的 miss 语义 SHALL 为**显式 `Err`**（「当前无等待中的 ask」）而非幂等 `Ok`——与 `agent_stop` 的 miss 幂等是不同语义：重复应答 / 迟到应答都是用户可感知的「卡已不在」事实，静默成功会掩盖双端不同步。命令 SHALL 落在 `commands/exec/` 轨道并登记于单一命令清单。
- 订阅命令（新 `agent_session_watch`）：见「会话级实时流订阅面」；参数 `(root, session_id, since_seq)` + `Channel<AgentRunMessage>`，命令体为薄包装（登记订阅 → Channel 前向 → 结束释放），MUST NOT 编排会话生命周期；blank root / 无运行轮幂等结束（与 `agent_stop` miss 幂等同口径）
- 发起命令 SHALL 保持 `Channel<AgentRunMessage>` 实时出口与提前 resolve 契约（发送方流零空洞：Channel 先于 invoke 建立）；MUST NOT 因新增订阅命令而收窄或移除该出口（两出口同源，非二选一）

错误约定沿用既有模板 `Result<T, String>`：spawn 失败、SDK 引擎配置缺失（经 `AgentStartError` 中性变体）、无默认 agent 可解析、存量 cli 实例解析（CLI 租户已下线）、workspace 库打开失败等 SHALL 以 `Err` 抵达前端，MUST NOT 静默吞掉。

#### Scenario: 命令体薄包装

- **WHEN** 审查发起命令实现
- **THEN** 命令体为参数转换 + 调用内核 API + 错误映射三段，编排（tee/落库/收敛/盖戳）在内核内，不膨胀命令体

#### Scenario: 来源与会话续接参数透传

- **WHEN** explore 页以 `source="explore"`、`source_ref=null` 且携带记录回写的 `sessionId` 发起续轮
- **THEN** 生成会话归属 `source="explore"` 且以该会话全史续接（store 全史转录重建）；调试页 invoke（不传来源参数）生成的会话 `source="debug"`

#### Scenario: agent 缺省与解析

- **WHEN** invoke 不带 `agent` 参数（正式场景如 explore 链恒不传）
- **THEN** 内核组合根缺省解析默认 agent 并经门面构造对应引擎（无默认 agent 时显式 `Err`，不落库不推流、不静默回退）；调试页显式传 agent 时按所选解析；协议参数面全程无 agent / engine 概念（core 零污染）

#### Scenario: 查询与停止 root 寻址

- **WHEN** 前端 invoke 会话查询与终止命令
- **THEN** 命令无状态、按 root 解析所属 workspace 库直查/寻址返回 DTO，无领域解释；workspace A 的调用看不到 B 的会话

#### Scenario: 订阅命令薄包装与幂等

- **WHEN** 审查 `agent_session_watch` 实现并对一条已收口会话 invoke
- **THEN** 命令体为参数转换 + 注册表订阅 + Channel 前向三段（无编排、无落库）；已收口会话调用立即成功结束（零事件、无副作用）

#### Scenario: 应答命令三段纪律

- **WHEN** 审查 `agent_answer` 实现并以登记在案的 session id 调用
- **THEN** 命令体为参数校验 + 注册表转投 + 错误映射三段（零状态机操作、零通知广播、零停等态清理）；等待方收到原文、命令返回 `Ok`

#### Scenario: 应答命令 miss 显式失败

- **WHEN** 分别以 blank root、blank session_id、未登记（或已应答过）的 session id 调用 `agent_answer`
- **THEN** 三者均返回显式 `Err`（blank 臂携字段名、miss 臂文案含「无等待中的 ask」），MUST NOT 幂等 `Ok`

#### Scenario: 错误 reject 传达

- **WHEN** SDK 引擎配置缺失、解析目标为存量 cli 实例（CLI 租户已下线）、无默认 agent 可解析或所属 workspace 库打开失败时发起运行
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

#### Scenario: cwd 与 store 身份双 root 不变量

- **WHEN** 以 `cwd = <change worktree>`（存在且为目录）与 `root = <workspace root>` invoke 发起命令
- **THEN** 该轮工具路径沙箱根 / bash `current_dir` / `AGENT.md` 读取根取 worktree，而会话行落于 workspace root 所属库（`for_root` 只受 `root` 影响，cwd 路径不派生第二份 workspace 库）；同会话可经 workspace 侧清单查到

#### Scenario: cwd 缺失显式失败

- **WHEN** 以不存在的目录作 `cwd` invoke 发起命令
- **THEN** 发起点返回显式 `Err`（错误串含该路径），不进入引擎、不产生会话行与转录、不留半截状态

#### Scenario: cwd 缺省零语义漂移

- **WHEN** 既有消费面（调试页 / explore 链）不传 `cwd` 发起运行
- **THEN** 执行根回落 `root`，行为与参数面增位前逐字一致（既有位置参数 1–7 语义不变）

#### Scenario: 来源引用归属与清单不撞

- **WHEN** 某 change 既跑过流程（会话 `source_ref` 为四段）又有面板 chat 会话（`source_ref` 为 change id 两段）
- **THEN** 以 `source = "change"` + `source_ref = <change id>` 的清单筛选只命中 chat 会话，四段来源的流程会话一条都不命中；`source_ref` 只用于归属与清单，装载仍以 `sessionId` 直查

### Requirement: 引擎泵一轮一命与 headless 无问收口

`agent-runtime` 的 `sdk/` 引擎会话泵任务（`session_pump`）SHALL 服务**恰一个提问**：轮终态产出后泵任务 SHALL 返回（观察通道发送端随之释放 → 内核观测通道关闭）。泵 MUST NOT 回到提问接收等待下一轮（多问循环 MUST NOT 残留）；spawn 失败与停止路径的既有单轮终止语义 SHALL 保持不变。会话史 SHALL 不依赖泵内跨轮回灌（SDK 泵的跨轮 `history_slot` 随一轮一命移除；会话史唯一来源仍是 store 全史转录重建）。

`core/agent` 的 `drive` SHALL 保持「观测通道关闭（引擎侧单边 EOF）→ converge」为**唯一收口缝**，MUST NOT 引入状态机 break、stop select 或第二收口路径；内核测试假 runner 的引擎侧单边 EOF（`close_stream`）即该契约的测试面，真实泵 SHALL 履行同一契约。

消费纪律：任何消费方对同一逻辑会话的每一轮 SHALL 经 `compose_turn → begin → open_session` 新开引擎会话（既有消费现实：chat Continue 与 walker 每步均如此）；泵返回后同一引擎会话通道上的提问送达 SHALL 走内核 ask 失败臂（failed 记因「轮提问送达失败（引擎侧已终止）」），MUST NOT 复活常驻泵。

headless 交互面（本变更修订）：**ask 为阻塞工具，不消耗整轮**——模型需用户决策时 SHALL 调用 `AskUserQuestion`（工具在 turn 内阻塞、应答经会话级 ask 注册表回流、同一 turn 内继续），MUST NOT 以「问题文本作为本轮最终消息收口 + 应答后 Continue 重开一整轮」作为交互实现；`ask` 位为 `None` 的会话不挂该工具（显式禁用），此时原「问题文本收口 + Continue 续轮」路径仍是该会话的可用退路，但 MUST NOT 被当作 ask 的实现。一轮一命语义不受影响：ask 阻塞发生在轮内工具执行段，不新开轮、不复活泵。

注释口径：kernel / runner 的「逐轮等待提问 / 回到等待下一轮 / 逐轮驱动（ask 语义）」注释 SHALL 更新为一轮一命口径。

#### Scenario: 泵单轮收口驱动内核 EOF converge

- **WHEN** 引擎泵产出轮终态事件（SDK loop 收敛）
- **THEN** 泵任务返回且观察通道发送端释放（观测通道 `recv` 返回 `None`），内核 `drive` 的观测循环以通道关闭退出并 converge（finish_turn + `TurnFinished` 流出、轮行落库终态、注册表除名）
- **AND** 泵任务不再回到提问接收等待下一轮

#### Scenario: 泵终止后提问按失败收敛

- **WHEN** 同一引擎会话通道在泵返回后再次收到提问送达
- **THEN** 内核 ask 失败臂收敛 failed（记因「轮提问送达失败（引擎侧已终止）」），系统中不存在挂起的常驻泵
- **AND** 消费方每轮经 `compose_turn → begin → open_session` 新开引擎会话（chat Continue 与 walker 每步既有路径零改动）

#### Scenario: 无 ask 禁令残留的组装面

- **WHEN** 审查 SDK 引擎的工具面与命令组装面
- **THEN** 组装面无 disallowedTools 概念（`--disallowedTools` 末位组装规则随 CLI 租户退役，历史 cli 会话转录不受影响）；工具面不再是「封闭清单」——`AskUserQuestion` 按 `ask` 位下发（见「ask 工具协议与阻塞应答通道」），`RunStarted.tools` 如实上报

#### Scenario: ask 阻塞不改一轮一命

- **WHEN** 一轮内模型调用 `AskUserQuestion`，用户应答后模型继续并在收口条件满足时结束
- **THEN** 全程恰一个泵任务、恰一个提问（`questions.recv` 一次）、恰一个轮终态；阻塞期间无第二条提问送达、无新会话建立

#### Scenario: 无问文本收口与同会话续轮

- **WHEN** 会话以 `ask = None` 装配（该工具不下发）且模型需用户决策
- **THEN** 该轮不发生 ask 类工具调用，模型只能以问题文本作为本轮最终消息收口（`turnDone` 正常 converge）；用户应答后经 Continue（store 全史转录重建）同会话续轮，前轮全史在场——该路径为**显式禁用 ask 的退路**，MUST NOT 被当作 ask 的实现

#### Scenario: 停止与 spawn 失败语义保持

- **WHEN** 泵轮中途收到停止信号，或 spawn 失败（SDK 启动失败）
- **THEN** 既有语义不变：停止路径不合成收敛、终止缝被调、轮收敛 `stopped`；spawn 失败以合成收敛事件 failed 记因且泵终止
- **AND** 两路径均为单轮终止，与一轮一命语义同形，无回归

### Requirement: MVP 边界与已知限制

以下边界 SHALL 作为 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **kill 已实现（偿还「无 kill」限制）**：`agent_stop` 提供终止入口（见「agent_stop 终止与提前 resolve」）；残余限制——bash 工具进程树清理失败时子进程可能残留，应用中途关闭时的孤儿进程仍为已知限制
2. **续会话经转录重建落地**：续会话以 store 全史转录重建为唯一重建源（见「SDK 引擎 resume（store 即会话）」）；`--continue` 隐式续会话语义（CLI 时代）随 CLI 租户退役；调试页自身仍不提供 resume 入口（续话仅 explore 会话链消费）
3. **留存无清理**：事件转录无上限增长
4. **交互工具边界（ask-tool-in-flow 修订，原「无 ask 类工具」限制已偿还）**：`AskUserQuestion` SHALL 为基线阻塞工具（`ToolTier::Protocol`，`Generic` / `Flow` 两档执行体，见「ask 工具协议与阻塞应答通道」），初始 `Generic` / `Flow` 之外的禁用场景仅以 `SessionInjections.ask = None` 表达且**首期不接线**（无禁用场景）；**已知限制两条**：① 一次 ask 单问题（`question` + 一组 `options`），多问需多次调用，`options` 必填非空（用户可用自由文本作答，但模型必须给选项）；② ask 阻塞期间无超时自动应答、无「稍后作答」的持久化（回答通道活在进程内，应用重启即失，`AskRegistry` 槽由轮收敛除名）——停等态因此仍不落库；`questionnaire` 组件接线为二期。
5. **bypassPermissions 默认**：deny 规则仍生效；agent 运行可在 workspace 内无审批改动文件——本裁决限于本机自有 repo 的调试场景

#### Scenario: 终止入口可用

- **WHEN** 审查调试页与 explore 页
- **THEN** 运行中提供停止入口且触发 `agent_stop`；进程树清理失败时的孤儿进程残余留痕于本 spec

#### Scenario: 限制留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含 ask 的两条已知限制），后续变更无需重新论证是否知情

#### Scenario: 原「无 ask 类工具」表述零残留

- **WHEN** 审查本 spec 与产品源码
- **THEN** 「工具面为封闭清单，无 ask 类工具」一类的禁令表述不再出现；`AskUserQuestion` 在挂载清单与 `RunStarted.tools` 中可被观察到

## ADDED Requirements

### Requirement: ask 工具协议与阻塞应答通道

SDK 引擎 SHALL 提供基线阻塞工具 `AskUserQuestion`，其应答通道 SHALL 为**会话级注册表**（`AskRegistry`，与 `StopRegistry` 同址托管）：

- **工具契约**（定义单源在 `core/orchestration`，见 desktop-change-orchestration「流程内 ask 停等与双入口应答」）：工具名 `AskUserQuestion`，`parameters` 为 `{ question: string（必填）, options: string[]（必填，`minItems: 1`）}`——**一次问一个问题**，多问经多次调用表达；`options` 为空数组 SHALL 被工具校验 `Err` 拒绝（文案自足：字段名 + 期望「至少一个选项」+ 当前值）；用户应答 MAY 为任一选项文本或自由文本，工具 MUST NOT 限制应答取值域。
- **`AskRegistry`（`core/agent`）**：形态 SHALL 为 `{ slots: Mutex<HashMap<String, oneshot::Sender<String>>> }`，键 = **core 铸的会话 id**（`core/agent` 不认识 workspace root / change——两段寻址由壳层翻译）；方法三件：`register(session_id, tx) -> bool`（登记等待方，同键覆盖 = 上一问已被放弃，返回是否登记成功）、`answer(session_id, text) -> Result<(), String>`（**take 语义**：单次投递，重复 / miss → `Err`，不产生第二次回灌）、`remove(session_id)`（轮收敛除名，miss 幂等）。注册表 MUST NOT 携带任何编排域类型（无 change id、无 `RunEventSink`）。
- **会话 id 下传**：`open_session` 的入参 SHALL 携带 core 铸的会话 id（`SessionOpen.session_id`），使引擎侧能以自己的执行体上下文登记到正确键；逐轮的 `TurnQuestion` MUST NOT 重复携带该 id（会话级标识在会话建立时一次可知）。
- **两档执行体（同名覆盖）**：`ToolExec::Ask(AskMode)` 为单一挂载变体带档位，`TOOL_NAME` 与 `parameters` 两档共用（schema 单源），差别只在执行体：
  - `Generic`：`register(session_id, tx)` → 阻塞等待应答（无编排广播）→ 返回应答文本作为 tool result；
  - `Flow`：经 `AskPort` 先广播「ask 已开始」（`AskPayload` 含 `tool_call_id` / `session_id`）→ `register` → 阻塞等待 → **两臂（应答 / 停止）返回后都广播「ask 已解除」**（流程停等态清理单点在此，见 desktop-change-orchestration）。
  两档 SHALL 与 `finish_step` **互不依赖**：`ask` 位与 `finish` 位独立置位，两模块代码互不引用（`grep` 可验的零互引，见 desktop-change-orchestration 同条）。
- **`ask` 位与挂载**：`SessionInjections.ask: Option<AskMode>` 决定是否挂载（`None` = 不挂 = 硬禁用：`RunStarted.tools` 无该名，模型调用即 `tool_unavailable`；`Some(_)` = 挂载，tier 恒 `Protocol`，三档权限全放行）。执行上下文 SHALL 经组合根以 per-turn 形态铸造（注册表句柄 + core session id + 可选 `AskPort`）；上下文缺席 / 登记失败 SHALL 以 is_error ToolResult 回灌（MUST NOT panic、MUST NOT 静默成功）。
- **停止语义**：ask 阻塞期间的用户停止 SHALL 由外层泵的 `select!` **轮中 drop 整个 loop future** 实现（既有机制，零新增）；因此 ask 执行体 MUST NOT 依赖「停止时回灌 is_error 让模型看见」——做不到，且转录会留下未配对的 `tool_use`，该缺口 SHALL 由 `resume::rebuild` 的 `INTERRUPTED_RESULT` 补齐（既有机制）。`Flow` 档的 `select!` SHALL 显式带停止臂（`RunHandle::wait_requested()`）——外层 drop 不执行清理代码，故停等态清理必须落在执行体内的这个臂上。
- **幂等与可归因**：同一 ask 二次应答 SHALL 返回 `Err`（文案「当前无等待中的 ask」）；会话已收口后的迟到应答 SHALL 同样 `Err`（注册表槽已由轮收敛除名）；两处 `Err` 都 MUST NOT 改写任何会话状态。

#### Scenario: 通用会话 turn 内阻塞问答

- **WHEN** `ask = Some(Generic)` 的会话里模型调用 `AskUserQuestion { question, options }`，用户经 `agent_answer` 投递应答
- **THEN** 该轮不收敛、不发额外模型请求；应答文本作为该 `tool_use` 的 tool result 回灌，模型在同一 turn 内继续（同一 `session_id`、`turn_index` 递增一次）

#### Scenario: 同名覆盖只多广播

- **WHEN** `ask = Some(Flow)` 的 change 会话里模型调用同一工具
- **THEN** 下发的工具名与 `parameters` 与 `Generic` 档逐字相同（模型视角零差异、历史转录无需分辨来源），额外经 `AskPort` 先后广播「已开始（携 `tool_call_id` 与 `session_id`）」与「已解除」；应答文本照常回灌

#### Scenario: 显式禁用即硬禁用

- **WHEN** 会话以 `ask = None` 装配，模型仍发出 `AskUserQuestion` 调用
- **THEN** `RunStarted.tools` 不含该名；调用被成员校验拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌，run 不中断，注册表无登记

#### Scenario: 停止不改写协议配对

- **WHEN** ask 阻塞期间用户停止该会话
- **THEN** 轮收敛 `stopped`；转录保留该 `tool_use` 的未配对记录（无 is_error 回灌——future 已 drop）；以该会话续轮时 `resume::rebuild` 补 `INTERRUPTED_RESULT` 使 provider 配对协议不破；注册表该键已除名（其后 `answer` → `Err`）

#### Scenario: 重复应答只回灌一次

- **WHEN** 同一 ask 被连续应答两次（两入口或同入口重放）
- **THEN** 第一次成功且等待方收到原文，第二次返回 `Err`（「当前无等待中的 ask」），等待方 MUST NOT 收到第二条、转录 MUST NOT 出现第二条 tool result

#### Scenario: 上下文缺席回灌而非崩溃

- **WHEN** `ask` 位为 `Some` 但执行上下文未装配（测试构造 / 装配漂移）
- **THEN** 该工具调用以 is_error ToolResult 回灌（「ask 执行上下文未装配」类文案），run 不中断、不 panic

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/agent/src/ask.rs`（新） | 会话级应答注册表 | `AskRegistry::new() -> Self`；`register(&self, session_id: &str, tx: tokio::sync::oneshot::Sender<String>) -> bool`；`answer(&self, session_id: &str, text: String) -> Result<(), String>`（take 语义，重复 / miss → `Err`）；`remove(&self, session_id: &str)`（miss 幂等）。`Default` 派生；键 = core session id；零编排域类型 |
| `crates/core/agent/src/runner.rs`（扩） | 注入位与 ask 档位词汇 | `pub enum AskMode { Generic, Flow }`（`Copy + Eq + Serialize + Deserialize`，serde camelCase，**不派生 `specta::Type`**）；`SessionInjections` 增 `#[serde(default)] pub ask: Option<AskMode>`（键恒在，四字段 `PartialEq + Eq` 链完好）；`SessionOpen` 增 `pub session_id: String`（core 铸 id 下传，用途 = ask 登记键与引擎侧归属） |
| `crates/core/agent/src/kernel.rs`（扩） | 内核持注册表 + 轮收敛除名 | `SessionKernel::new(sink, registry, streams, ask_registry: Arc<AskRegistry>)`（第 4 参）；`RunningTurn` 携该 `Arc`，`converge` 内 `ask_registry.remove(session_id)`（与 `registry.remove` 同址兜底）；`begin_turn` 把 session id 填进 `SessionOpen` |
| `crates/core/agent/src/lib.rs`（扩） | 出口 | 导出 `AskRegistry`（`AskMode` 随 `runner` 出口） |
| `crates/infra/agent/src/sdk/mount.rs`（扩） | 挂载单源追加 ask 项 | `ToolExec` 增 `Ask(agent::AskMode)`；`definitions(injections, prompt)` 在 `injections.ask` 为 `Some(mode)` 时追加 `AskUserQuestion`（`description` / `parameters` 取自 `orchestration::ask`，`tier = ToolTier::Protocol`，`exec = Ask(mode)`）；与 `finish` 位两段**独立** `if`（MUST NOT 合取）；doc 增 provider 侧不变量（原生 ask 工具一律不挂 = 覆盖） |
| `crates/infra/agent/src/sdk/loop.rs`（扩） | ask 执行体两臂与阻塞 | `LoopTurn` 增 `ask: Option<AskContext>`（`AskContext { registry: Arc<AskRegistry>, core_session_id: String, port: Option<Arc<dyn orchestration::port::AskPort>> }`——`core_session_id` 是**注册表键**（core 铸 id，与 `LoopTurn.session_id` 的引擎侧 `sdk-…` id 同名不同义，MUST NOT 复用后者作键））；`ToolExec::Ask(Generic)` = `register` + `select!(oneshot, handle.wait_requested())`；`ToolExec::Ask(Flow)` = `port.ask_started(AskPayload{..})` + `register` + `select!` + `port.ask_resolved()`（两臂返回后都执行）；`register` 失败或上下文缺席 → is_error 回灌 |
| `crates/infra/agent/src/sdk/runner.rs`（扩） | per-turn 铸造与透传 | `PumpPayload` 增 `ask_registry: Arc<AskRegistry>` 与 `ask_port: Option<Arc<dyn AskPort>>`；`session_pump` 按 `injections.ask` 位铸 `AskContext`（`Flow` 档把 `ask_port` 一并放入；`Generic` 档为 `None`；`ask_port` 缺席而档位为 `Flow` 时铸造出的上下文为「无 port」形态 → 执行体 is_error 回灌） |
| `crates/infra/agent/src/lib.rs` / `compose.rs`（扩） | 注入缝与装配 | `EngineFacade::with_ask_registry(Arc<AskRegistry>)` + `with_ask_port(Option<Arc<dyn orchestration::port::AskPort>>)`（与 `with_explore_note_store` 同型的两条 builder 缝）；`compose_turn(..., ask_registry: Arc<AskRegistry>, ask_port: Option<Arc<dyn AskPort>>, ...)`——前者透传内核与门面，后者仅门面（编排侧状态面的实现方是壳层 sink，内核侧不需要它）；`assemble_injections` 逐臂置 `ask` 位（explore / archi → `Generic`，change → `Flow`，其余来源**显式** `Generic`，MUST NOT 走 `SessionInjections::default()`） |
| `src-tauri/src/main.rs`（扩） | 壳层托管 | `app.manage(Arc::new(AskRegistry::default()))`（与 `StopRegistry` / `SessionStreamRegistry` 三层同址）；`ChangeFlowControl` 构造传入该 `Arc` |
| `src-tauri/src/commands/exec/mod.rs`（扩） | 应答命令（CLI 面） | `agent_answer(root: String, session_id: String, text: String) -> Result<(), String>`（tauri command；blank → `Err`、miss → `Err`、命中 → 转投注册表）；`agent_start` 把注册表句柄传入 `compose_turn` |
| `packages/desktop/src/types/generated/bindings.ts`（生成物） | 前端调用面 | `agentAnswer: (root: string, sessionId: string, text: string) => Promise<null>`；`AskPayload` 增 `toolCallId: string \| null` / `sessionId: string \| null` |
| `packages/desktop/src/hooks/use-agent-session.ts`（扩） | 会话动作面 | `UseAgentSessionState` 增 `answer: (text: string) => void`（`root` / `sessionId` 为 null 时不 invoke；`Err` 经既有错误面呈现） |
