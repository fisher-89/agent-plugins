# desktop-agent-execution Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 把注入面的三处计数与一处档位对齐新工具：archi 来源臂七名 → **八名**（+ `archi_generate`）、change 臂矩阵随之扩、`tier` 归属多一个写面工具、相位锁由「一条」改「**两条**」（模型目录 + 契约资产）。其余语义（名字清单单源、成员校验、fail-closed、`ask` / `finish` 两位独立、explore 与 config 臂）逐字不变。

## MODIFIED Requirements

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 以**名字清单为唯一单源**（`toolset` 位与 `SessionToolset` 枚举已退役，专用工具不再有第二套注入机制），并自本变更起以 **`protect` 位**承载会话级写入禁区（第三规则来源；`ask` 位承载 ask 装配档位）：

- **core 契约扩展**：`SessionInjections` 形态 SHALL 为 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec>, ask: Option<AskMode>, protect: Option<Vec<SessionProtectRule>>, before_done: Option<BeforeDoneSpec> }`——`tools` 为**本轮下发的工具名字清单**（`None` = infra 内建七件：read / grep / glob / ls / write / edit / bash），专用工具（`create_explore_note` / `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation` / `archi_generate` / `db_tables` / `db_query` / `propose_config`）以**名字进清单**而不经枚举追加；`finish`（`FinishSpec { EvaluatorChecklist, Decision }`，`Copy + Eq` 闭合枚举）为 `#[serde(default)]` 的收口档位词汇，`ask`（`AskMode { Generic, Flow }`，`Copy + Eq` 闭合枚举，见「ask 工具协议与阻塞应答通道」）为 `#[serde(default)]` 的 ask 装配档位——**`None` 是显式禁用**（该会话拿不到对应工具），非「未实现」，装配层 MUST 逐臂显式置位。**`preamble` 位按来源装配，生产者三处**——`source = "archi"` 的架构会话知识、`source = "change"` 在**写权放开**时的同一份架构会话知识（判据 = `model_write_lock(provenance) == None`，与两**模型**写工具下发同集，见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」）、`source = "config"` 的配置协议知识；位的内容由 loop 与 `AGENT.md` 合并为单条 System 消息（见「SDK 引擎系统提示词（AGENT.md 注入）」）；前端与自由字符串 MUST NOT 直接供给该位。**`protect` 位**为 `#[serde(default)]` 的会话级写入禁区清单（`SessionProtectRule { glob: String, reason: String }`，`reason` 携 `%s` / `%t` 占位符），**`None` = 不追加**（语义与装配单点见 `desktop-write-protection`「会话级写入禁区（相位锁）」）；该位 SHALL NOT 派生 `specta::Type`（引擎侧会话词汇不出线 IPC，与 `FinishSpec` / `AskMode` 同款），为此**不进 TS bindings**。`before_done` 位见「自然收敛前的收口前钩子与收敛事件序」。**词汇封闭原则**：core 只承载**名字**、**档位词汇**与**纯数据规则串**，工具定义（description / parameters）与准入档位（tier）唯一真源在 infra 的挂载清单（定义与档位均由挂载清单给定，见本条「tier 档位归属」）；非法工具名 SHALL 在装配层（compose）显式 `Err`（携非法名），MUST NOT 流进执行面。`PartialEq` / `Eq` 派生链 SHALL 完好（`FinishSpec` / `AskMode` 均为闭合枚举、清单为字符串向量、`SessionProtectRule` 为两字符串纯数据——`ToolMount` 一类含 `serde_json::Value` 的结构 MUST NOT 进 core）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分四臂——① `source == "explore"`：`tools = Some([read, grep, glob, ls, edit, bash, create_explore_note, db_tables, db_query])`（`write` 移除）+ `ask = Some(Generic)`（两名只读 DB 工具见 desktop-agent-db-tools）；② `source == "archi"`：`tools = Some([read, grep, glob, ls, write, edit, bash, archi_module, archi_search, archi_api, archi_drift, archi_scaffold, archi_element, archi_relation, archi_generate])`（七内建全量 + **八个** archi 工具名，与 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」的 archi 来源臂单源一致）+ `preamble = Some(架构会话知识)` + `ask = Some(Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析（`source_ref` 取任意值不改变该臂）；③ `source == "change"`：按 `source_ref` 尾三段解析 `(phase, role)` 并查 `(phase, role)` → archi 工具子集矩阵（矩阵单源见 desktop-archi-agent「archi 工具面注入与 (phase, role) 矩阵」），命中行把矩阵勾选的 archi 工具名并入七工具名字清单，**`db_tables` / `db_query` 两名字在矩阵之后恒 push（不分 `phase` / 不分矩阵命中，与 `ask` 位恒 `Flow` 同构）**，`finish` 位按 `role` 置位（`evaluator` → `Some(EvaluatorChecklist)`、`decision` → `Some(Decision)`、其余 `None`），**`ask` 位恒 `Some(Flow)`（不分 role / 不分矩阵命中）**，未命中行与**解析失败（畸形 / 短段 / 未知 phase）**SHALL 恒零 archi 工具（fail-closed），**`preamble` 位 SHALL 为「`model_write_lock(provenance) == None` ⇒ `Some(架构会话知识)`，否则 `None`」**（单一判据、与写权同源，MUST NOT 另立一份矩阵命中判定；知识拼装失败降级 `None`、不影响工具面）；④ `source == "config"`：`tools = Some([read, grep, glob, ls, bash, propose_config])`（**无 `write` / `edit`、无 db 工具**）+ `preamble = Some(配置协议知识)`（生产者与内容见 `desktop-config-agent`「配置协议知识的静态 preamble」）+ `finish = None` + `ask = Some(Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析。其余来源（debug 等，即上述四臂之外者）SHALL 注入 `tools = None`（内建七件）+ `preamble = None` + `finish = None` + **`ask = Some(Generic)`**——MUST NOT 以 `SessionInjections::default()`（其 `ask` 为 `None`）为兜底装配，否则通用会话静默失去 ask。**四臂的 `protect` 位 SHALL 由装配层单点 `model_write_lock(provenance)` 逐臂置位**（archi 臂与 change 臂的 `(proposal|dev-design, executor)` → `None`；其余一律 `Some(两条)`——模型声明目录 + 契约资产目录，两串模板各自单源；规则 glob 由装配层经 `foundation::layout` 派生，`core` 不解释路径语义——判定矩阵归 `desktop-write-protection`）。四臂的每个工具名 SHALL 在返回前对 infra 挂载清单做成员校验（未知名 `Err`）。
- **loop 组装与硬禁用**：definitions SHALL 由名字清单查 infra 挂载清单组装（`definitions(injections, prompt)`），并按两个**互不依赖**的位追加动态挂载项——`ask` 位为 `Some(mode)` 时追加 `ask_user_question`（`ToolTier::Protocol`，执行体 `ToolExec::Ask(mode)`），`finish` 位为 `Some(spec)` 时追加 `finish_step`（`ToolTier::Protocol`，执行体 `ToolExec::FinishStep(spec)`）；两条追加分支 MUST NOT 互相引用（无合取判据——`finish = None` 的会话仍有 ask，`ask = None` 的会话仍有 `finish_step`）。`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从清单移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行。
- **三段逐字沿用（档位段本变更扩一格）**：`create_explore_note(name, content)` 契约、执行体装配缝逐字沿用；档位归属段除新增 `archi_generate` 一格（落写面）外逐字沿用——五 archi 只读工具仍 `ToolTier::Read`、两模型写工具仍 `ToolTier::Write`。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具（含 ask）的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root + ask 注册表句柄 + core session id），sdk loop 经 `LoopTurn` 携带的上下文路由；per-turn 铸造判据 SHALL 为**名字清单包含**（`create_explore_note` 在场即铸笔记上下文；`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation` / `archi_generate` **任一**在场即铸 archi 上下文——上下文另携带生成物写盘端口，见 `desktop-crate-layout`「core/archi crate 落地登记」；`db_tables` / `db_query` 任一在场即铸 `DbContext`——全局库句柄经 `WorkspaceStores::global_shared()`、workspace 库句柄取会话 workspace root 的库实例，change 会话 cwd 是 worktree 但库身份锚恒主仓 root；`propose_config` 在场即铸 `ConfigContext`——其 `root` 取 **`PumpPayload.workspace_root`（主仓 root）**，MUST NOT 取 exec root / worktree，见 `desktop-config-agent`「`ConfigContext` 铸造与 root 语义」）与**位存在**（`ask` 位为 `Some` 即铸 ask 上下文），MUST NOT 用枚举比较或自由字符串嗅探。`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **tier 档位归属**：`create_explore_note` SHALL 归写面档位（`ToolTier::Write`：Default 拒绝、AcceptEdits / BypassPermissions 放行）；`archi_element` / `archi_relation` SHALL 同归写面档位（`ToolTier::Write`，与 `write` / `edit` 同款——两工具的写盘与保护射程见 `desktop-archi-agent`「archi 写工具面」与 `desktop-write-protection`「会话级写入禁区（相位锁）」）；**`archi_generate`（本变更新增）SHALL 同归写面档位**（`ToolTier::Write`：Default 拒绝、AcceptEdits / BypassPermissions 放行——它写盘且受写入保护逐份判定，见 `desktop-archi-agent`「archi_generate 生成物工具」）；五个 archi 只读工具（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`）SHALL 归只读面（`ToolTier::Read`：三档全放行）；`db_tables` / `db_query` SHALL 同归只读面（`ToolTier::Read`：三档全放行；无路径参数，沙箱改写面对其恒原样返回，写入保护零介入——契约细则见 desktop-agent-db-tools）；`propose_config` SHALL 同归只读面（`ToolTier::Read`：三档全放行；无路径参数，`tools::input_path` 对其恒 `None`——候选里的任意键名不进沙箱改写面，见 `desktop-config-agent`「`propose_config` 工具契约」）；`finish_step` 与 `ask_user_question` SHALL 归 `ToolTier::Protocol`（三档全放行——询问用户与投递决策不写盘、不执行命令，与权限档正交）。档位准入由 tier 比较单点表达（见「SDK 引擎静态权限与路径沙箱」），MUST NOT 恢复按工具名的常量清单。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note / db_tables / db_query / ask_user_question（无 `write`），`tools` 名字清单逐名一致（不含 ask 名——ask 按位追加，不占名字清单），`ask = Some(Generic)`，`RunStarted.tools` 逐名一致上报该面

#### Scenario: 幻觉 write 调用被拒

- **WHEN** explore 会话中模型发出已从清单移除的 `write` 工具调用
- **THEN** 成员校验拒绝：`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult（与 tool_use 同 id）回灌，run 不中断、不触执行体

#### Scenario: create 原子定名

- **WHEN** 占位记录（`draft-1760000000000`）会话中模型调用 `create_explore_note(name="api-retry", content=<初稿>)`
- **THEN** 磁盘出现 `.dev-team/explores/api-retry.md`（内容 = content），store 记录 `name="api-retry"`（`id` / `session_id` 不变），返回文本含笔记路径；占位名文件全程不存在

#### Scenario: create 拒绝面零半边残留

- **WHEN** 分别以非法 kebab 名、撞已有记录名、目标文件已存在（既有笔记）调用 `create_explore_note`
- **THEN** 均显式 `Err` 回灌（文件已存在臂附路径），磁盘与库内零变化（前置校验先于任何写动作）

#### Scenario: 已定名记录异名拦截

- **WHEN** 已定名记录（`api-retry`，笔记在盘）会话中模型调用 `create_explore_note(name="other-name", …)`
- **THEN** `Err` 附现有名 `api-retry` 与笔记路径，零改动；同名且文件缺失时补建成功（孤儿重建）

#### Scenario: 非 explore 来源仍获 ask

- **WHEN** 以 `source="debug"`（或不传来源）发起会话
- **THEN** definitions 与 `RunStarted.tools` 为内建七工具（含 `write`）+ `ask_user_question`，`finish` 位为 `None`（无 `finish_step`），`ask = Some(Generic)`，`protect = Some(两条：模型声明目录 + 契约资产目录)`（非 white-listed 来源一律挂锁）；成员校验对七工具与 ask 放行，七工具行为与注入机制上线前一致

#### Scenario: change 来源按矩阵注入 archi 工具面

- **WHEN** 以 `source="change"` 且 `source_ref` 为 `<change_id>/dev-design/executor/1` 发起会话
- **THEN** 名字清单为七工具 + `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation` / `archi_generate`（**八名** archi 工具，矩阵命中行勾选者）+ `db_tables` / `db_query`（名字清单共 17 名）；definitions 为该 17 项 + `ask_user_question`，`RunStarted.tools` 逐名一致上报；`finish` 位为 `None`、`ask = Some(Flow)`、`protect = None`（写权相位不挂锁）、`preamble = Some(架构会话知识)`

#### Scenario: change 臂知识段与写权同源

- **WHEN** 分别以 `<id>/proposal/executor/1`、`<id>/dev-design/executor/1`、`<id>/implement/executor/1`、`<id>/dev-design/evaluator/1`、`<id>/code-review/evaluator/1` 发起会话
- **THEN** 前两者 `preamble` 为架构会话知识（与 archi 臂逐字相同）且 `protect = None`；后三者 `preamble = None`（`implement/executor` 虽有 `archi_api` / `archi_drift`，但写权锁在位 ⇒ 不注入写模型纪律，且它零 `archi_generate`）；五者的 `protect` 与 `preamble` 两列逐格互不矛盾

#### Scenario: 模型写权位逐臂置位

- **WHEN** 分别以 `source="archi"`、`<id>/proposal/executor/1`、`<id>/dev-design/executor/1`、`<id>/dev-design/evaluator/1`、`<id>/implement/executor/1`、`<id>/test-gen/executor/1`、畸形 `source_ref`、`source="explore"`、`source="debug"` 发起会话
- **THEN** 前三者 `protect = None`（写权通路）；后六者 `protect = Some([两条：模型声明目录锁 + 契约资产目录锁])`（fail-closed 方向）；两锁 glob 与两份拒绝文案归 `desktop-write-protection`

#### Scenario: 矩阵未命中行与畸形 source_ref fail-closed

- **WHEN** 分别以 `<change_id>/test-gen/executor/1`、`<change_id>/implement/executor`（段数不足）与空 `source_ref` 发起 change 会话
- **THEN** 三者均落零 archi 工具（七工具 + `db_tables` / `db_query` 名字清单）、`preamble = None`，不抛装配错误、不回退为全量 archi 工具；`ask` 仍为 `Some(Flow)`（ask 位不随矩阵）

#### Scenario: archi 来源装配面

- **WHEN** 以 `source="archi"`、`source_ref` 为 null 发起会话并审查该轮装配产物
- **THEN** `tools` 名字清单为七工具（含 `write`）+ `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation` / `archi_generate`（**八个** archi 工具，与 `desktop-archi-agent` 的 archi 来源臂单源一致）、`preamble` 为架构会话知识、`ask = Some(Generic)`；definitions 为该十五项 + `ask_user_question`，`RunStarted.tools` 逐名一致

#### Scenario: config 来源装配面

- **WHEN** 以 `source="config"`、`source_ref` 为 null 发起会话并审查该轮装配产物
- **THEN** `tools` 名字清单为 `read` / `grep` / `glob` / `ls` / `bash` / `propose_config` 六名（**无** `write` / `edit`、无 `db_tables` / `db_query`、无 archi 工具）、`preamble` 为配置协议知识、`finish` 位为 `None`、`ask = Some(Generic)`；definitions 为该六项 + `ask_user_question`，`RunStarted.tools` 逐名一致；该臂不进入 `source_ref` 相位解析（`source_ref` 取任意值不改变下发面）

#### Scenario: Explore 臂零回归

- **WHEN** 以 `source="explore"` 发起会话并审查装配产物
- **THEN** 六工具 + `create_explore_note` + `db_tables` / `db_query` 的名字清单与本次变更前 definitions 的可观察产出逐条一致（成员校验 / 硬禁用 / `RunStarted.tools` 上报三条语义零改写），差异恰为多下发的 `ask_user_question` 与只读 DB 工具两件

#### Scenario: 非法工具名装配层显式拒绝

- **WHEN** 以任何来源构造出含挂载清单外名字的注入面（如 `tools = Some(["read", "fly"])`）
- **THEN** 装配层显式 `Err`（文案携非法名与外来源），会话不建立、不进执行面

#### Scenario: ask 位与 finish 位四组合独立

- **WHEN** 分别以 `(ask, finish)` 的四种组合构造注入面
- **THEN** `(Some(_), Some(_))` 下发两项；`(Some(_), None)` 只下发 `ask_user_question`；`(None, Some(_))` 只下发 `finish_step`；`(None, None)` 两项都不下发——两分支无合取判据，任一缺席不牵连另一项

#### Scenario: DB 工具两臂与另两臂的分野

- **WHEN** 分别以 `source="explore"`、`source="change"`（含 `test-gen/executor` 一类矩阵未命中行与畸形 `source_ref`）、`source="archi"`、`source="config"`、`source="debug"` 发起会话并审查该轮名字清单
- **THEN** 前两臂含 `db_tables` / `db_query`（change 臂不分 `phase` / 不分矩阵命中，与 `ask` 位恒在同理），后三臂不含；未下发两工具的会话调用它们一律走成员校验拒绝（`tool_unavailable` + `is_error`，run 不中断、不触执行体）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### Function: `crates/infra/agent/src/compose.rs` / `sdk/{runner,mount}.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| compose archi 臂 | 改 | 名字清单七 archi 名 → **八**名（+ `archi_generate`） |
| compose change 臂 | 改 | 矩阵命中行随之扩（两起草相位各 + `archi_generate`，`test-design` + `archi_module`）；`db_tables` / `db_query` 的恒 push 与 `ask` / `finish` 位逐字不变 |
| `model_write_lock` 的 `protect` 位 | 改 | 挂锁格由「一条」改「两条」（模型声明目录 + 契约资产目录，两串模板各自单源）；不挂锁格仍 `None` |
| `mint_archi_context` | 改 | 铸造判据七名 → **八名**（含 `archi_generate`） |
| `mount.rs` | 改 | 静态清单 18 → **19** 项；`archi_generate` 落 `ToolTier::Write` / `ToolExec::Archi`（细则归 `desktop-archi-agent`） |

### API: 注入契约（零改动）

| 项 | 描述 |
|---|---|
| `SessionInjections` 六位形态 | 逐字不变（`tools` 仍是名字清单；本变更只换清单内容与 `protect` 的规则条数） |
| `SessionProtectRule` / `FinishSpec` / `AskMode` | 逐字不变（零新字段、零新枚举变体） |
