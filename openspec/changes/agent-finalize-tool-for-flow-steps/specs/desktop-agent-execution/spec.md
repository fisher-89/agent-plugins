# desktop-agent-execution Specification (Delta)

## MODIFIED Requirements

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 自本条起以**名字清单为唯一单源**（`toolset` 位与 `SessionToolset` 枚举退役，专用工具不再有第二套注入机制）：

- **core 契约扩展**：`SessionInjections` 形态 SHALL 为 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec> }`——`tools` 为**本轮下发的工具名字清单**（`None` = infra 内建七件：read / grep / glob / ls / write / edit / bash），专用工具（`create_explore_note` / `archi_module` / `archi_search`）以**名字进清单**而不经枚举追加；`finish` 为 `#[serde(default)]` 的收口档位词汇（`FinishSpec { EvaluatorChecklist, Decision }`，`Copy + Eq` 闭合枚举，见「`finish_step` 工具协议与本步收口」）。`preamble` 位按来源装配（唯一生产者 = `source = "archi"` 的架构会话知识），并由 loop 与 `AGENT.md` 合并为单条 System 消息（见「SDK 引擎系统提示词（AGENT.md 注入）」）；前端与自由字符串 MUST NOT 直接供给该位。**词汇封闭原则改写**：core 只承载**名字**与**收口档位词汇**，工具定义（description / parameters）与准入档位（tier）唯一真源在 infra 的挂载清单（见「工具挂载单源与 tier 准入」）；非法工具名 SHALL 在装配层（compose）显式 `Err`（携非法名），MUST NOT 流进执行面。`PartialEq` / `Eq` 派生链 SHALL 完好（`FinishSpec` 为闭合枚举、清单为字符串向量——`ToolMount` 一类含 `serde_json::Value` 的结构 MUST NOT 进 core）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分三臂——① `provenance.source == "explore"`：注入 `tools = Some([read, grep, glob, ls, edit, bash, create_explore_note])`（`write` 移除）；② `provenance.source == "archi"`：注入 `tools = Some([read, grep, glob, ls, write, edit, bash, archi_module, archi_search])` + `preamble = Some(架构会话知识)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析（`source_ref` 取任意值不改变该臂）；③ `provenance.source == "change"`：SHALL 按 `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}` 的尾三段解析 `(phase, role)` 并查 `(phase, role)` → archi 工具子集矩阵（矩阵单源见 desktop-archi-agent「archi 工具面与 (phase, role) 注入矩阵」），命中行把矩阵勾选的 archi 工具名并入七工具名字清单，`finish` 位按 `role` 置位（`evaluator` → `Some(EvaluatorChecklist)`、`decision` → `Some(Decision)`、其余 `None`），未命中行与**解析失败（畸形 / 短段 / 未知 phase）**SHALL 恒零 archi 工具（fail-closed），其余来源（debug 等）SHALL 恒 `SessionInjections::default()`（`tools = None` 即内建七件、`preamble = None`、`finish = None`）。三臂的每个工具名 SHALL 在返回前对 infra 挂载清单做成员校验（未知名 `Err`）。
- **loop 组装与硬禁用**：definitions SHALL 由名字清单查 infra 挂载清单组装（`definitions(injections, prompt)`），`finish` 位置位时按档铸造 `finish_step` 一项追加；`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从清单移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root），sdk loop 经 `LoopTurn` 携带的上下文路由；per-turn 铸造判据 SHALL 为**名字清单包含**（`create_explore_note` 在场即铸笔记上下文；`archi_module` / `archi_search` 任一在场即铸 archi 上下文），MUST NOT 用枚举比较。`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **tier 档位归属**：`create_explore_note` SHALL 归写面档位（`ToolTier::Write`：Default 拒绝、AcceptEdits / BypassPermissions 放行）；两个 archi 只读工具 SHALL 归只读面（`ToolTier::Read`：三档全放行，与 `read` / `grep` / `glob` / `ls` 同款）；`finish_step` 归 `ToolTier::Protocol`（三档全放行）。档位准入由 tier 比较单点表达（见「SDK 引擎静态权限与路径沙箱」），MUST NOT 恢复按工具名的常量清单。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note（无 `write`），`tools` 名字清单逐名一致，`RunStarted.tools` 逐名一致上报该面

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

#### Scenario: 非 explore 来源零变化

- **WHEN** 以 `source="debug"`（或不传来源）发起会话
- **THEN** definitions 与 `RunStarted.tools` 仍为内建七工具（含 `write`），`finish` 位为 `None`（无 `finish_step`），成员校验对七工具全放行，行为与注入机制上线前一致

#### Scenario: change 来源按矩阵注archi工具面

- **WHEN** 以 `source="change"` 且 `source_ref` 为 `<change_id>/dev-design/executor/1` 发起会话
- **THEN** 名字清单为七工具 + `archi_module` + `archi_search`（矩阵命中行勾选者）；definitions 含两名定义，`RunStarted.tools` 逐名一致上报；`finish` 位为 `None`

#### Scenario: 矩阵未命中行与畸形 source_ref fail-closed

- **WHEN** 分别以 `<change_id>/test-gen/executor/1`、`<change_id>/implement/executor`（段数不足）与空 `source_ref` 发起 change 会话
- **THEN** 三者均落零 archi 工具（七工具名字清单或 `tools = None`），不抛装配错误、不回退为全量 archi 工具

#### Scenario: archi 来源装配面

- **WHEN** 以 `source="archi"`、`source_ref` 为 null 发起会话并审查该轮装配产物
- **THEN** `tools` 名字清单为七工具（含 `write`）+ `archi_module` / `archi_search`、`preamble` 为架构会话知识；definitions 为该九项，`RunStarted.tools` 逐名一致

#### Scenario: Explore 臂零回归

- **WHEN** 以 `source="explore"` 发起会话并审查装配产物
- **THEN** 六工具 + `create_explore_note` 的名字清单与本次变更前 definitions 的可观察产出逐字相同（成员校验 / 硬禁用 / `RunStarted.tools` 上报三条语义零改写）

#### Scenario: 非法工具名装配层显式拒绝

- **WHEN** 以任何来源构造出含挂载清单外名字的注入面（如 `tools = Some(["read", "fly"])`）
- **THEN** 装配层显式 `Err`（文案携非法名与外来源），会话不建立、不进执行面

### Requirement: SDK 引擎（rig 进程内租户）

`agent-runtime` 的 `sdk/` 引擎 SHALL 以 `rig` facade（rig-core 的 re-export 门面）实现进程内编码 agent 租户：无需本地安装 claude CLI（或任何 CLI）即可用，应用开箱即跑；`rig` facade SHALL 恒编译、MUST NOT 设 feature gate。约定：

- provider 协议 SHALL **双值可选且双首公民**：`openai`（openai-compatible 端点——GLM / DeepSeek / Kimi / 自部署）与 `anthropic`（Anthropic Messages 端点及兼容网关）；协议由 provider 记录 `protocol` 声明、经 `EngineConfig.protocol` 抵达引擎（见「SDK 引擎模型协议选臂（openai / anthropic）」）——原「provider 首公民 SHALL 为 openai-compatible 端点；anthropic provider 降为后续可选」的限制在本变更偿还
- 自建 agent loop（多轮：流式响应 → 映射事件 → 执行工具 → 回灌 → 续轮；收敛与统计填充全自控）——rig 仅用作 provider client 与消息/工具类型层
- **流面双层交付**：流内 `Text` / `Reasoning` 片段增量 SHALL 映射为 `MessageDelta`（text/thinking 可辨，只上传输面；rig 0.43 流事件模型为部分生命周期——Start / Arguments / End 为簿记事件不出增量）；轮末 `finish()` 折叠的 `choice` SHALL 收口为**恰一条密封 `Message`**（全部块收进；MUST NOT 把每个 text delta 当完整块出密封 Message——碎事件根因修复）
- 自建工具面：默认（无会话注入）工具集 = read / grep / glob / ls / write / edit / **bash** 七工具（bash 执行体与档位见「SDK 引擎 bash 工具」）；会话级注入 SHALL 以**名字清单**收窄该面、以专用工具**名字**追加受控工具面（工具定义与准入档唯一真源在 infra 挂载清单，见「工具挂载单源与 tier 准入」与「会话级工具面注入与 explore 笔记工具」）；MCP 与子代理 MUST NOT 实现（`parent_tool_use_id` 恒空）
- **系统提示词**：请求系统提示词 SHALL 由引擎自读 workspace root `AGENT.md` 注入（见「SDK 引擎系统提示词（AGENT.md 注入）」）；`SessionInjections.preamble` 位按来源装配（唯一生产者 = archi 臂），`tools` / `finish` 位按来源装配（见「会话级工具面注入与 explore 笔记工具」；`toolset` 位已随枚举退役）
- **上下文管理**：喂 provider 的请求史 SHALL 经 L1-L3 上下文防线治理（见「SDK 引擎工具质量与单结果上限（L1）」「SDK 引擎上下文窗防线（L2 剪裁与 L3 compaction）」）；store 转录全量不变；窗长来源含模型名 `[1m]` 后缀（见「SDK 引擎模型名 `[1m]` 后缀与输出上限定档」）
- 事件归一化：归一化 SHALL 只消费 rig 抽象事件词汇（`StreamEvent` / `Completion` / `Message`），工具调用映射为 ToolUse 块并与 ToolResult 成对（同 id）；协议差异 MUST NOT 出现在归一化层（openai chat completions 形状 fixture 仅为测试形状，不是运行时协议判定；anthropic 经 Messages wire 产出同一抽象）；`RunStarted` SHALL 各报各的（model 报**裸名**——`[1m]` 后缀已剥离，tools 报本轮实际下发的工具面——默认七工具、注入面含 `finish_step` 见「会话级工具面注入与 explore 笔记工具」）；统计口径依 TurnDone 字段面（`cost_usd` 恒缺省 `None`——无价格表，降级不违约）；API 重试 / 错误 SHALL 经 `SystemNotice{subtype}`（开放枚举）流出
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

#### Scenario: 注入位词面无 toolset 残留

- **WHEN** 审查 `SessionInjections` 字段面与本条工具面约定
- **THEN** 只出现 `preamble` / `tools` / `finish` 三位；产品源码（`crates/**` / `src/**`）中 `toolset` 与 `SessionToolset` 零命中

### Requirement: SDK 引擎静态权限与路径沙箱

SDK 引擎 SHALL 以静态权限三档起步（无审批反向通道——事件流外的应答协议不进 MVP）：Default 档工具面 = 只读（read / grep / glob / ls），write / edit / bash 拒绝；AcceptEdits 档 = + write / edit / bash；BypassPermissions 档 = 全放行。**准入判据 SHALL 由「档位 × 工具名」改为「档位 × 挂载 tier」**（单处四行）：`ToolTier::Protocol` 与 `ToolTier::Read` 三档恒放行；`ToolTier::Write` 与 `ToolTier::Execute` 在 Default 档拒绝、AcceptEdits / BypassPermissions 放行。该映射 SHALL 与改造前的 `READONLY_TOOLS` / `WRITE_TOOLS` / `EXECUTE_TOOLS` 三张常量表**逐格等价**（`create_explore_note` 归 Write、两 archi 工具归 Read、`bash` 归 Execute），MUST NOT 出现第二份按名准入清单。bash 档位语义（Default 拒绝、AcceptEdits / BypassPermissions 放行）为第七工具的自然延伸——只读档不给执行权；**AcceptEdits 与 BypassPermissions 自此首次真实分化**（此前两臂行为完全一致）。默认档 SHALL 为 BypassPermissions（与调试页既有默认一致——本机自有 repo 的调试场景以完整循环为默认）。拒绝 SHALL 经 `SystemNotice{subtype:"permission_denied"}` + is_error=true 的 ToolResult 流出（与 CLI `-p` 无头 Default 档行为同构）。tier 准入 SHALL 为纯函数并密集测试；交互审批协议（`permission_request` 事件 + `agent_respond` 命令 + 应答句柄）MUST NOT 实现（留独立 change）。

路径沙箱：write / edit / read 工具全链 SHALL 经 canonicalize 后做 workspace root 前缀校验（拦 `..\` 相对逃逸与符号链接绕过 root 外目标），校验 SHALL 为纯函数并可密集测试。BypassPermissions 默认档下路径沙箱 SHALL 是模型与 workspace root **之外**任意文件写之间的唯一护栏，其实现质量为必过关卡；workspace root **之内**的文件写另有第二道独立闸——写入保护闸（受保护文件面，见「loop 守卫链写入保护闸与工作区根注入缝」与 desktop-write-protection），二者射程不重叠：沙箱管「能否越出 root」，保护闸管「是否是受保护文件」。bash 进工具面后，**命令执行本身不在路径沙箱射程内**——路径沙箱不过问命令内容，作用于受保护文件的写入由保护闸经命令文本提取覆盖（含重定向 / 删除 / 移动 / 内联脚本）；未受保护文件的改动仍无护栏，真沙箱（job object / 容器）与交互审批留后续 change；命令级白/黑名单 MUST NOT 作为安全手段（假安全）。本轮 **`tools::input_path` 与 `protect::evaluate` 两处按工具名的 match 保留**（`glob` 取 `pattern`、两 archi 工具显式 `None`；`"write" / "edit"` 按路径、`"bash"` 按命令文本）——这是知情边界：新增「带路径参数或带写入面」的工具仍需改这两处，代价已登记。

#### Scenario: 三档决策表

- **WHEN** 以三档 × 四 tier 矩阵驱动准入纯函数
- **THEN** Default 档只放 `Read` / `Protocol`、拒绝 `Write` / `Execute`；AcceptEdits 与 BypassPermissions 四 tier 全放行，逐格断言通过（12 格）

#### Scenario: 与原常量表逐格等价

- **WHEN** 对七内建工具 + `create_explore_note` + 两 archi 工具逐一求值「改造前三表判定」与「新 tier 判定」
- **THEN** 三档下两套判定结果逐工具相同（等价性由对拍断言钉住，不留人工核对）

#### Scenario: AcceptEdits 与 Bypass 分化

- **WHEN** 以 AcceptEdits 档与 Default 档分别驱动 `bash`（`ToolTier::Execute`）
- **THEN** AcceptEdits 放行、Default 拒绝——两档对同一工具首次给出不同决策，既有「两臂一致」行为退役（分化由锁定测试钉住）

#### Scenario: 拒绝流出形态

- **WHEN** Default 档下模型请求 `write` 工具
- **THEN** 事件流含 `SystemNotice{subtype:"permission_denied"}` 与 is_error=true 的 ToolResult（与 tool_use 同 id），run 不中断

#### Scenario: 逃逸拦截

- **WHEN** 工具入参路径含 `..\` 相对逃逸，或为符号链接指向 workspace root 外目标
- **THEN** canonicalize + 前缀校验拒绝执行（tempdir 驱动密集用例全拦）；root 内合法路径不受影响

#### Scenario: 两道闸并存不互相替代

- **WHEN** AcceptEdits 档下 root 内普通文件（未受保护）被写，以及 root 内受保护文件被写
- **THEN** 前者经沙箱放行后正常执行；后者沙箱放行、保护闸拒绝——沙箱不承担保护职责，保护闸不承担逃逸职责

#### Scenario: 两处按名 match 保留为知情边界

- **WHEN** 审查 `tools::input_path` 与 `protect::evaluate`
- **THEN** 两处仍为按工具名的 match（`glob` 的 `pattern` 特判、两 archi 工具显式 `None`；`"write" / "edit"` 与 `"bash"` 两臂），`finish_step` 与两 archi 工具在 `protect` 分派表走默认放行臂；新增挂载项 MUST NOT 被要求改这两处之外的地方（代价登记于 spec，非缺陷）

## ADDED Requirements

### Requirement: 工具挂载单源与 tier 准入

infra 引擎 SHALL 以**一张挂载清单**作为工具面的唯一事实源（`sdk/mount.rs`），取消「收窄清单 + 枚举追加 + 按名常量表 + 按名分派」的四源并行：

- **类型面**：`ToolMount { definition: ToolDefinition, tier: ToolTier, exec: ToolExec }`；`ToolTier { Read, Write, Execute, Protocol }`（`Protocol` = 任何档位放行）；`ToolExec { Pure, ExploreNote, Archi, FinishStep(FinishSpec) }`。`ToolMount` / `ToolTier` / `ToolExec` SHALL 全部驻 infra（core MUST NOT 认识其中任何一个——`serde_json::Value` 无 `Eq`，落 core 即塌 `SessionInjections` 的 `Eq` 链）。
- **清单面**：`registry() -> &'static [ToolMount]` SHALL 静态持有十项（七内建 + `create_explore_note` + `archi_module` + `archi_search`），名字两两不同；七内建与两个 archi / explore 工具的定义文本 SHALL 自既有定义函数原样搬入（字面量零改）。`finish_step` MUST NOT 进静态清单（其 description / parameters 按 step 动态，见「`finish_step` 工具协议与本步收口」）。
- **组装面**：`definitions(injections, prompt) -> Vec<ToolMount>` SHALL 按名字查表（`tools = None` → 内建七件；`Some(names)` → 按清单序过滤），`finish` 位置位时铸造并追加 `finish_step` 一行；名字不在清单即 `Err`。
- **消费面单源**：成员校验、tier 准入、`RunStarted.tools`、loop 执行分派四处 SHALL 全查同一 `definitions` 产出（MUST NOT 各自维护第二份名单）；`TOOL_NAMES` SHALL 降级为「七件内建的定义作者处」，MUST NOT 参与任何判定。
- **执行分派**：loop SHALL 以 `match mount.exec` 分派（`Pure` → `tools::execute`、`ExploreNote` → `LoopTurn.explore_note`、`Archi` → `LoopTurn.archi`、`FinishStep` → 收口路径），MUST NOT 保留按工具名的 `if` 链。

#### Scenario: 十项静态挂载

- **WHEN** 审查 `registry()` 返回值
- **THEN** 恰十项且名字为 read / grep / glob / ls / write / edit / bash / create_explore_note / archi_module / archi_search，两两不同；tier 归属为 Read 六项（read / grep / glob / ls / archi_module / archi_search）、Write 三项（write / edit / create_explore_note）、Execute 一项（bash）

#### Scenario: 名字清单查表组装

- **WHEN** 分别以 `tools = None`、`tools = Some(["read","grep"])`、`tools = Some(["read","create_explore_note"])` 求 `definitions`
- **THEN** 依次得内建七件、两项子集、两项子集（后者含 `create_explore_note` 定义）；名称与顺序均由清单决定，MUST NOT 出现第二份过滤名单

#### Scenario: 名称非法即装配错误

- **WHEN** 以 `tools = Some(["read","not_a_tool"])` 求 `definitions`
- **THEN** 返回 `Err`（文案携非法名），零工具下发

#### Scenario: 执行分派按 exec 而非名字

- **WHEN** 审查 loop 的工具执行分派代码
- **THEN** 为 `match mount.exec` 四臂（`Pure` / `ExploreNote` / `Archi` / `FinishStep`），无按工具名的条件链；`protect::evaluate` 与 `tools::input_path` 两处按名 match 为显式保留

#### Scenario: 分发清单三处一致

- **WHEN** 同一次请求内比对 `RunStarted.tools`、definitions 名单与成员校验命中集
- **THEN** 三者逐名相同（成员校验按 definitions 的 mount 清单判定，`RunStarted.tools` 自同一名单枚举）

### Requirement: finish_step 工具协议与本步收口

evaluator 与 decision 会话 SHALL 以**工具调用**而非文本 JSON 收口：`finish_step` 的语义为**本 step 结束**（一轮一命拓扑下等价于本 turn 收口），挂载面为「挂载清单里多一项」，MUST NOT 引入第二套收口机制：

- **挂载条件**：`SessionInjections.finish = Some(EvaluatorChecklist)` 时挂 evaluator 形态、`Some(Decision)` 时挂 decision 形态、`None` 时不挂（其余步骤调用 `finish_step` 被成员校验拒为 `tool_unavailable`）。`tier` 恒 `ToolTier::Protocol`（三档全放行）。
- **形态来源（动态 schema）**：description 与 parameters 的唯一真源 SHALL 为 core/orchestration（`finish::description(spec)` / `finish::parameters(spec, prompt)`）——infra MUST NOT 认识 verdict / 动作语义。evaluator 形态的 `parameters.properties.items.items.properties.id.enum` SHALL 由**本轮 prompt 文本**（`## 静态清单` 段落）解析出的 ID 集合派生，使「校验的 ID 集 == 模型眼前那份 prompt 的 ID 集」天然自一致（换 prompt 变体、加 ID 自动跟随，零 md 读取依赖）。
- **校验边界**：工具执行体的校验 SHALL 只做**自包含**事项——JSON 形状、action tag 封闭集、`report` 长度上限、checklist 非空、`pass` 布尔、ID 集合等于 prompt 解析集合；`backtrack` 目标白名单与自环判定（需运行时 `allowed`）、ask 停等（需 guard）SHALL 留在编排层。校验器签名 SHALL 为 `(FinishSpec, &Value, prompt: &str) -> Result<FinishPayload, String>` 的纯函数。**ID 集合相等这一条 MUST NOT 外溢到机械 checklist**：walker 的 7 个机械代写条目（静态检查 / 架构检查 / 测试执行 / 突变测试 / suite 结论一致 / 聚合计数一致 / mutation null 自动通过）由桌面代写到 `phase_log`，不经 `finish_step`、零 ID 要求——校验器的唯一调用点是 `ToolExec::FinishStep` 分派臂。
- **Err 反馈面唯一**：校验 `Err` SHALL 以 is_error=true 的 ToolResult **当场回灌**（同一 turn 内模型可立即重试），MUST NOT 续一轮请求、MUST NOT 落 notice。文案 SHALL 自足：含**哪个字段**、**期望什么**、**当前值 / 实际集合**、**超出上限的具体数量**（`verdict_fix_prompt` / `decision_fix_prompt` 两个跨轮修复面已退役，工具 Err 文案是唯一反馈面）。
- **loop 内收口三纪律**：① 同批 tool_calls 中 `finish_step` 之后的调用 SHALL 产出**假结果**且 `is_error: false`（文案「本步已 finish_step 收口，该调用未执行」），并 SHALL **真的经事件通道发出**（进转录，MUST NOT 依赖事后补 `INTERRUPTED_RESULT`）；② 同批全部 `tool_use` 的结果 SHALL 收进**恰一条** user 消息，且该收口 SHALL 先于 `return`（provider 配对硬要求）；③ 校验 `Ok` 后 SHALL 发 `SystemNotice{subtype:"finish_step", payload: FinishPayload(JSON)}`、以既有成功链收口（`TurnDone{is_error:false}`）并**零额外请求**返回。
- **终止在 loop 内闭合**：MUST NOT 复用 `RunHandle::request_stop()`（其语义链为 `Stopped → Terminal::stopped → UI「已停止」`，会把正常评估收尾显示成用户终止）；四层（loop / pump / kernel / `run_worker`）MUST 全走既有成功链，零改动。
- **载荷出口**：`FinishPayload` SHALL 经 `SystemNotice` 载荷出 loop（密封事件进 transcript 与时间线），由 `orchestration::transcript::finish_payload(&transcript)` 提取后填入 `WorkerTurnOutcome.finalized`；MUST NOT 扩 `TurnOutcome`（store 轮行落库与 IPC 终态装配共用的唯一收口形状）、MUST NOT 扩 `TurnDone`（infra 认识协议语义会撞分层三角）。
- **截断面（显式接受的回退）**：输出截断（`max_tokens` / `context_window_exceeded`）仍走「正常收敛 + `output_truncated` 留痕」，此时 `finalized` 恒缺席、相位判失败；MUST NOT 加「截断时回退解析文本 JSON」的降级分支（两条收口路径即两套判据两套测试）。

#### Scenario: evaluator 步挂载 evaluator 形态

- **WHEN** 以 `source="change"`、`source_ref` 尾段为 `<phase>/evaluator/1` 发起会话，审查该轮 definitions 与请求
- **THEN** definitions 含 `finish_step` 一项，其 parameters 为 `{ report, items: [{ id, pass, evidence }] }` 形态且 `id` 的 enum 逐值等于该轮 prompt 的静态清单 ID 集；tier 为 `Protocol`（Default 档亦放行）

#### Scenario: decision 步挂载 decision 形态

- **WHEN** 以 `source_ref` 尾段为 `<phase>/decision/1` 发起会话
- **THEN** definitions 含 `finish_step`，parameters 为三动作形状（`action` tag + backtrack 的 `to` / `reason`、stop 的 `reason`、ask 的 `question` / `options`）

#### Scenario: 其余步骤无 finish_step

- **WHEN** 以 `<phase>/executor/1` 或 debug 会话发起
- **THEN** definitions 与 `RunStarted.tools` 均不含 `finish_step`；模型若调用之，成员校验以 `tool_unavailable` + is_error ToolResult 拒绝，run 不中断

#### Scenario: 校验 Err 当场回灌

- **WHEN** evaluator 步调用 `finish_step` 且 `report` 为 2500 字符（上限 2000）
- **THEN** 该 tool_use 得 is_error=true 的 ToolResult（文案含字段名、上限 2000、当前 2500、超出 500），无新请求、无 notice；模型可在同一 turn 内以合规参数重试并成功收口

#### Scenario: 同批假结果与收口

- **WHEN** 同一 assistant 消息内先发合规 `finish_step` 再发一次 `read`
- **THEN** `read` 得 `is_error: false` 的假结果（文案「本步已 finish_step 收口，该调用未执行」）且该结果经事件通道进转录；同批两条结果进**恰一条** user 消息；随后发 `SystemNotice{subtype:"finish_step"}` 与 `TurnDone{is_error:false}`，零额外请求

#### Scenario: 载荷进 WorkerTurnOutcome

- **WHEN** 一轮以 `finish_step` 收口（`status = Completed`）
- **THEN** 转录末条含 `SystemNotice{subtype:"finish_step"}`，`WorkerTurnOutcome.finalized` 为该载荷（`Evaluator` / `Decision` 一臂）；缺 notice 或载荷损坏时 `finalized` 为 `None`

#### Scenario: 截断不发 finalized

- **WHEN** evaluator 输出因 `max_tokens` 截断收场
- **THEN** 事件流含 `output_truncated` notice、`TurnDone{is_error:false}`，`finalized` 为 `None`；编排侧据此判失败（成因文案与「未调 `finish_step`」可区分）

## Module Contract

### Function: `crates/infra/agent/src/sdk/mount.rs`（新）

| 项 | 签名 | 描述 |
|------|------|------|
| `ToolMount` | `{ definition: ToolDefinition, tier: ToolTier, exec: ToolExec }` | 挂载项三元组（定义 / 准入档 / 执行体） |
| `ToolTier` | `enum { Read, Write, Execute, Protocol }` | 准入档（`Protocol` = 任何档位放行） |
| `ToolExec` | `enum { Pure, ExploreNote, Archi, FinishStep(FinishSpec) }` | 执行体分类（loop 分派单点） |
| `registry` | `fn() -> &'static [ToolMount]` | 十项静态挂载清单（唯一工具事实源） |
| `definitions` | `fn(&SessionInjections, prompt: &str) -> Result<Vec<ToolMount>, String>` | 名字清单查表 + `finish` 位铸造 + 非法名 `Err` |
| `injected` | `pub(crate) fn(names: &[&str]) -> SessionInjections` | 测试夹具（压平 `SessionInjections` 构造面） |

### Function: `crates/infra/agent/src/sdk/policy.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|------|
| `allows` | `fn(AgentPermissionMode, ToolTier) -> bool` | 三张按名常量表删除；`Protocol \| Read` 恒真、`Write \| Execute` 需 `mode != Default` |

### Function: `crates/infra/agent/src/sdk/loop.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `run` | 组装 / 校验 / 分派 / 收口四段 | `definitions(&turn.injections, &turn.question)`；成员校验按 mount 清单；`policy::allows(mode, mount.tier)`；`match mount.exec`；`finalized` 收口（假结果 `is_error:false` + `results` 先收齐 + `SystemNotice` + `finish_success` + `return`） |

### Function: `crates/core/agent/src/runner.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|------|
| `SessionToolset` | **删除** | 枚举整体退役（工具面单源化后无第三源） |
| `FinishSpec` | `enum { EvaluatorChecklist, Decision }`（`Copy + Eq`，camelCase 线值） | 收口档位词汇（core 只承载词汇，不认识 verdict / 动作语义） |
| `SessionInjections` | `{ preamble, tools: Option<Vec<String>>, finish: Option<FinishSpec> }` | `toolset` 位删除；`finish` 带 `#[serde(default)]`（旧线缺字段读 `None`）；`PartialEq + Eq` 完好 |

### API: `crates/infra/agent/src/compose.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `assemble_injections` | 三臂改产名字清单 + `finish` 位 | explore 七名（含 `create_explore_note`）/ archi 九名 / change 七名 + 矩阵勾选；每个名字对 `registry()` 做成员校验（未知名 `Err`）；`finish` 按 role 置位 |
| `archi_matrix` | 返回 `Vec<&'static str>` | 矩阵值由枚举改为名字清单（矩阵行语义不变，见 desktop-archi-agent） |

### Function: `crates/infra/agent/src/sdk/runner.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `session_pump` 的 per-turn 铸造 | 判据改名字包含 | `names.contains(EXPLORE_NOTE_TOOL)` → 铸笔记上下文；`names.contains(ARCHI_MODULE_TOOL) \|\| names.contains(ARCHI_SEARCH_TOOL)` → 铸 archi 上下文 |
| `mint_archi_context` | `fn(&[String], &Path) -> Option<ArchiContext>` | 入参由 `Option<&SessionToolset>` 改名字切片 |

### Function: `crates/infra/agent/src/worker.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `execute` | `WorkerTurnOutcome.finalized` 填充 | `finalized = orchestration::transcript::finish_payload(&transcript)`（末条 `finish_step` notice 提取） |

### 被本条取代的主基线 Module Contract 行

以下主基线行描述的是已退役的 `toolset` 承载形态；归档合并后 SHALL 以本 delta 的对应行替代（`grep -F` 旧行片段仍在即说明合并漏项）：

| 主基线旧行（片段） | 取代为 |
|---|---|
| `crates/core/agent/src/runner.rs` →「`SessionInjections` 增 `toolset: Option<SessionToolset>`（首值 `Explore`）…」 | 本文件「Function: `crates/core/agent/src/runner.rs`」表（三位形态 + `FinishSpec`） |
| `crates/core/agent/**`（注入面除外）、`cli/**` →「core 仅注入面词汇增位（`toolset`）…」 | 同上（注入面词汇增位＝名字清单 + `finish` 位） |
| `crates/infra/agent/src/compose.rs`（注入装配双臂）→「命中时 `tools = None` + `toolset = Archi{..}`」 | 本文件「API: `crates/infra/agent/src/compose.rs`」表（三臂产名字清单） |
| `crates/infra/agent/src/sdk/tools.rs`（definitions 组装）→「按 `toolset` 位追加 archi 工具定义…」 | 本文件「Function: `crates/infra/agent/src/sdk/mount.rs`」表（名字查表 + `finish` 位铸造） |
| `crates/core/agent/src/runner.rs`（`SessionToolset` 词表）→「值集 = `Explore` \| `Archi { module, search }`…」 | 同上（枚举删除，名字清单表达） |

### 零改动面

| 模块 | 说明 |
|------|------|
| `crates/infra/agent/src/sdk/{protect,sandbox,context,compact,resume,normalize,web_search,bash,model_spec}/**` | 零改动（守卫链、防线、回放、工具执行体皆不涉协议变更）；`tools::input_path` 与 `protect::evaluate` 两处按名 match 显式保留 |
| `crates/infra/agent/src/sdk/tools.rs` | 七工具执行体 / `execute` / `input_path` / `cap_result` 逐字不动（定义函数迁出） |
| `crates/infra/store/**` | 零改动：`ChecklistItemRecord` 形状与 `PhaseRecord` 落账不变 |
