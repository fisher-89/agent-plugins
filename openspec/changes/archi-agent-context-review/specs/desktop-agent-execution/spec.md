# desktop-agent-execution Specification（delta: archi-agent-context-review）

## Purpose

本 delta 给 `write` / `edit` 补一层**接口定义写闸**，并把架构知识段的第三个生产者（change 臂写权相位）落进注入面。

- **写闸**：目标路径落在 `<archi_models_dir>/**/<FQN>.schema.json` 时，落盘前调 `archi::api::schema::validate_schema`，`Err` 零落盘、成功追加「接口已校验」；`edit` 校验替换后正文；射程只有接口文件，其余路径写前零校验不变。
- **不属保护面**：写闸判「文件级自洽」，不触碰 `ProtectRules` / 三档 / 沙箱；写权仍只由相位锁表达。
- **注入面**：`preamble` 生产者三处——archi 臂、change 臂两写权相位（判据 `model_write_lock(provenance) == None`）、config 臂；`implement/executor` 有 archi 只读工具但不注入（知识含写模型纪律，与相位锁矛盾）。
- 系统提示词合成次序、单条 System、快照冻结与截断口径逐字不变。

## ADDED Requirements

### Requirement: `write` / `edit` 的接口定义写闸（坏文本不可产出）

`write` 与 `edit` 两个内建工具 SHALL 在落盘前对**接口定义文件**做一次内存校验，失败即 `Err` 且零落盘：

```text
落盘前：目标路径（sandbox 之后的绝对路径）落在 <root>/<archi_models_rel_dir>/ 之下
        ∧ 文件名主干是该目录内某元素的 FQN（形如 <FQN>.schema.json）？
  ├─ 否 → 原样落盘（零改动、零额外 IO）
  └─ 是 → 调 archi::api::schema::validate_schema(<FQN>, <候选文本>)
           ├─ Err → 工具 Err（第一行自足：文件 + 期望值 + 一条出路），零落盘
           └─ Ok  → 落盘，响应追加「接口已校验」一行
```

- **同一实现**：写闸 SHALL 调 `desktop-archi-api`「接口定义文件的装载校验与写闸（零第二份校验）」定义的**同一个** `validate_schema`；装载期（`load_contracts`）也调它。MUST NOT 在本层另写一份 JSON / JSON Schema 校验，MUST NOT 只校验部分面。
- **射程封闭**：只有 `<archi_models_dir>/**/<FQN>.schema.json` 进校验面；其余一切路径（普通源码 / 文档 / 模型 TOML / `INDEX.toml` / `.i.rs` / `.i.ts`）**写前零校验**的既有承诺逐字不变。目录前缀 SHALL 经 `foundation::layout` 派生（MUST NOT 行内 `models` / `archi` 字面量）。
- **`root` 穿参**：`write` / `edit` 的执行体 SHALL 接收会话执行根 `root`（`tools::execute(root, name, input)` 已持有），用于派生接口目录前缀；`tools::execute` 的对外签名 MUST NOT 改动。
- **`edit` 校验替换后的正文**：校验对象 SHALL 是 `old_string` → `new_string` 替换后的完整候选文本，MUST NOT 是替换前的旧文。
- **失败语义**：`Err` 文案第一行自足（含文件、期望值、出路之一），MUST NOT 只写「非法」；`Err` 由模型在同一 turn 内自纠重试，**run 不中断**（与既有工具 `Err` 同构）。
- **不属保护面**：本闸判「文件级自洽」，MUST NOT 触碰 `ProtectRules`、`policy` 三档、路径沙箱或写入保护的任何判定；写权仍只由相位锁表达（见 `desktop-write-protection`）。工具档位不变（`write` / `edit` 的既有档位语义逐字不动）。

#### Scenario: 三类坏输入各一条 Err 且零落盘

- **WHEN** 分别以「截断的 JSON」「`$id` 与文件名不符」「`T` 同时作 op 键与 `$defs` 键」经 `write` 落 `<models>/…/<FQN>.schema.json`
- **THEN** 三次均 `Err`（文案分别点名解析位置、期望 `$id`、冲突键与两个键空间），盘上字节零变化（新建文件不产生、既有文件不被截断或半写）

#### Scenario: 合法落盘并回执

- **WHEN** 以 `write` 落一份合法 schema
- **THEN** 落盘成功，响应含「接口已校验」；随后 `load_contracts` 装载该文件零错误

#### Scenario: edit 校验候选正文

- **WHEN** 以 `edit` 把一份合法 schema 的 `title` 改成另一名字
- **THEN** `Err`（identity 闸），盘上字节零变化；改成正确 `title` 后落盘成功并回执

#### Scenario: 非接口路径零校验

- **WHEN** 以 `write` 落一份内容含坏 JSON 的 `*.md` / `*.ts` / `INDEX.toml` / 模型 `*.toml`
- **THEN** 四者均原样落盘、零校验、零「接口已校验」行（射程封闭）；坏模型仍由读取面报结构诊断（既有语义逐字不变）

#### Scenario: 写闸不越权判跨元素

- **WHEN** 以 `write` 落一份 `$ref` 指向「不在模型内的 FQN」的 schema（前缀合法）
- **THEN** 写闸放行（跨元素事实不属文件级自洽）；该问题由后续检查以 `apiRefMissing` 报出——MUST NOT 在写闸里产出第二份跨元素判据

## MODIFIED Requirements

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 以**名字清单为唯一单源**，并以 **`protect` 位**承载会话级写入禁区（第三规则来源；`ask` 位承载 ask 装配档位）：

- **core 契约扩展**：`SessionInjections` 形态 SHALL 为 `{ preamble: Option<String>, tools: Option<Vec<String>>, finish: Option<FinishSpec>, ask: Option<AskMode>, protect: Option<Vec<SessionProtectRule>>, before_done: Option<BeforeDoneSpec> }`——`tools` 为**本轮下发的工具名字清单**（`None` = infra 内建七件：read / grep / glob / ls / write / edit / bash），专用工具（`create_explore_note` / `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation` / `db_tables` / `db_query` / `propose_config`）以**名字进清单**；`finish` / `ask` 为 `#[serde(default)]` 的闭合档位词汇，**`None` 是显式禁用**（装配层 MUST 逐臂显式置位）。**`preamble` 位按来源装配，生产者三处**——`source = "archi"` 的架构会话知识、`source = "change"` 在**写权放开**时的同一份架构会话知识（判据 = `model_write_lock(provenance) == None`，与两写工具下发同集，见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」）、`source = "config"` 的配置协议知识；位的内容由 loop 与 `AGENT.md` 合并为单条 System 消息（见「SDK 引擎系统提示词（AGENT.md 注入）」）。前端与自由字符串 MUST NOT 直接供给该位。**`protect` 位**为 `#[serde(default)]` 的会话级写入禁区清单，**`None` = 不追加**（语义与装配单点见 `desktop-write-protection`「会话级写入禁区（相位锁）」）；该位 SHALL NOT 派生 `specta::Type`（不进 TS bindings）。`before_done` 位见「自然收敛前的收口前钩子与收敛事件序」。**词汇封闭原则**：core 只承载名字、档位词汇与纯数据规则串；工具定义与准入档位唯一真源在 infra 挂载清单；非法工具名 SHALL 在装配层显式 `Err`，MUST NOT 流进执行面。`PartialEq` / `Eq` 派生链 SHALL 完好（含 `serde_json::Value` 的结构 MUST NOT 进 core）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分四臂——① `source == "explore"`：`tools = Some([read, grep, glob, ls, edit, bash, create_explore_note, db_tables, db_query])`（`write` 移除）+ `ask = Some(Generic)`；② `source == "archi"`：`tools = Some([read, grep, glob, ls, write, edit, bash, archi_module, archi_search, archi_api, archi_drift, archi_scaffold, archi_element, archi_relation])`（七内建全量 + 七个 archi 工具名）+ `preamble = Some(架构会话知识)` + `ask = Some(Generic)`，该臂以 `source` 判定、MUST NOT 进入 `source_ref` 相位解析；③ `source == "change"`：按 `source_ref` 尾三段解析 `(phase, role)` 并查矩阵，命中行把勾选的 archi 工具名并入七工具名字清单，**`db_tables` / `db_query` 两名字在矩阵之后恒 push**，`finish` 位按 `role` 置位，**`ask` 位恒 `Some(Flow)`**，未命中行与解析失败 SHALL 恒零 archi 工具（fail-closed）；**`preamble` 位 SHALL 为「`model_write_lock(provenance) == None` ⇒ `Some(架构会话知识)`，否则 `None`」**（单一判据、与写权同源，MUST NOT 另立一份矩阵命中判定；知识拼装失败降级 `None`、不影响工具面）；④ `source == "config"`：`tools = Some([read, grep, glob, ls, bash, propose_config])` + `preamble = Some(配置协议知识)` + `finish = None` + `ask = Some(Generic)`。其余来源（debug 等）SHALL 注入 `tools = None` + `preamble = None` + `finish = None` + `ask = Some(Generic)`——MUST NOT 以 `SessionInjections::default()` 兜底。**四臂的 `protect` 位 SHALL 由装配层单点 `model_write_lock(provenance)` 逐臂置位**。四臂的每个工具名 SHALL 在返回前对挂载清单做成员校验。
- **loop 组装与硬禁用**：definitions SHALL 由名字清单查挂载清单组装，并按两个**互不依赖**的位追加动态挂载项（`ask` → `ask_user_question`；`finish` → `finish_step`）；两条追加分支 MUST NOT 互相引用。`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮 definitions 即 `SystemNotice{subtype:"tool_unavailable"}` + is_error ToolResult 回灌（run 不中断）。「从清单移除」等于硬禁用。
- **`create_explore_note(name, content)` 契约**、**执行体装配缝**、**tier 档位归属** 三段逐字沿用（`archi_api` 的矩阵扩面只改下发表，不改档位：五 archi 只读工具仍 `ToolTier::Read`、三档全放行）。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note / db_tables / db_query / ask_user_question（无 `write`），`tools` 名字清单逐名一致，`ask = Some(Generic)`，`RunStarted.tools` 逐名一致

#### Scenario: 幻觉 write 调用被拒

- **WHEN** explore 会话中模型发出已从清单移除的 `write` 工具调用
- **THEN** 成员校验拒绝：`SystemNotice{subtype:"tool_unavailable"}` + is_error ToolResult 回灌，run 不中断、不触执行体

#### Scenario: change 来源按矩阵注入 archi 工具面

- **WHEN** 以 `source="change"` 且 `source_ref` 为 `<change_id>/dev-design/executor/1` 发起会话
- **THEN** 名字清单为七工具 + `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation`（**七名** archi 工具，本变更给两写权相位补 `archi_api`）+ `db_tables` / `db_query`（名字清单共 16 名）；definitions 为该 16 项 + `ask_user_question`，`RunStarted.tools` 逐名一致；`finish = None`、`ask = Some(Flow)`、`protect = None`、`preamble = Some(架构会话知识)`

#### Scenario: change 臂知识段与写权同源

- **WHEN** 分别以 `<id>/proposal/executor/1`、`<id>/dev-design/executor/1`、`<id>/implement/executor/1`、`<id>/dev-design/evaluator/1`、`<id>/code-review/evaluator/1` 发起会话
- **THEN** 前两者 `preamble` 为架构会话知识（与 archi 臂逐字相同）且 `protect = None`；后三者 `preamble = None`（`implement/executor` 虽有 `archi_api` / `archi_drift`，但写权锁在位 ⇒ 不注入写模型纪律）；五者的 `protect` 与 `preamble` 两列逐格互不矛盾

#### Scenario: 模型写权位逐臂置位

- **WHEN** 分别以 `source="archi"`、`<id>/proposal/executor/1`、`<id>/dev-design/executor/1`、`<id>/dev-design/evaluator/1`、`<id>/implement/executor/1`、`<id>/test-gen/executor/1`、畸形 `source_ref`、`source="explore"`、`source="debug"` 发起会话
- **THEN** 前三者 `protect = None`（写权通路）；后六者 `protect = Some([一条模型目录锁])`；锁 glob 与拒绝文案归 `desktop-write-protection`

#### Scenario: 矩阵未命中行与畸形 source_ref fail-closed

- **WHEN** 分别以 `<change_id>/test-gen/executor/1`、`<change_id>/implement/executor`（段数不足）与空 `source_ref` 发起 change 会话
- **THEN** 三者均落零 archi 工具、`preamble = None`，不抛装配错误、不回退为全量 archi 工具；`ask` 仍为 `Some(Flow)`

#### Scenario: archi 来源装配面

- **WHEN** 以 `source="archi"`、`source_ref` 为 null 发起会话
- **THEN** `tools` 名字清单为七工具（含 `write`）+ 七个 archi 工具（与 `desktop-archi-agent` 的 archi 来源臂单源一致）、`preamble` 为架构会话知识、`ask = Some(Generic)`；definitions 为该十四项 + `ask_user_question`

#### Scenario: config 来源装配面

- **WHEN** 以 `source="config"` 发起会话
- **THEN** `tools` 为 `read` / `grep` / `glob` / `ls` / `bash` / `propose_config` 六名（无 `write` / `edit`、无 db 工具、无 archi 工具）、`preamble` 为配置协议知识、`finish = None`、`ask = Some(Generic)`；该臂不进入 `source_ref` 相位解析

#### Scenario: 非法工具名装配层显式拒绝

- **WHEN** 以任何来源构造出含挂载清单外名字的注入面
- **THEN** 装配层显式 `Err`（文案携非法名与外来源），会话不建立、不进执行面

#### Scenario: ask 位与 finish 位四组合独立

- **WHEN** 分别以 `(ask, finish)` 的四种组合构造注入面
- **THEN** `(Some, Some)` 下发两项；`(Some, None)` 只下发 `ask_user_question`；`(None, Some)` 只下发 `finish_step`；`(None, None)` 都不下发

#### Scenario: DB 工具两臂与另两臂的分野

- **WHEN** 分别以 `source="explore"` / `"change"` / `"archi"` / `"config"` / `"debug"` 发起会话并审查名字清单
- **THEN** 前两臂含 `db_tables` / `db_query`，后三臂不含；未下发的会话调用它们一律走成员校验拒绝

### Requirement: SDK 引擎系统提示词（AGENT.md 注入）

SDK 引擎的系统提示词 SHALL 来自会话级注入位 `SessionInjections.preamble` 的**逐字**值：值在场 → chat_history 首条为**恰一条** `Message::System`（不包装、不改写、不追加引擎侧自读内容）；值缺席 → 不注入 System 消息。引擎执行体（`sdk/loop.rs`）MUST NOT 为组装系统提示词读取任何文件——`AGENT.md` 的读取与合并 SHALL 发生在装配层，产物为会话系统提示词快照并逐字复用于本会话后续每一轮。

`AGENT.md` SHALL 是快照的**成分之一**，读取口径逐条不变：读取根为会话执行根 `SessionCtx.workspace_root`；严格限于 `AGENT.md` 单份文件（MUST NOT `CLAUDE.md` / `AGENTS.md` 兜底）；内容逐字并以 `<agent_md path="…">…</agent_md>` 包裹；体量上限 32KB（char 边界截断）；文件缺席 → 快照不含该段。

快照正文的合成次序 SHALL 为：会话 preamble 知识**在前**，`AGENT.md` 段在后，两段之间空行分隔；**冻结指引常量**作为末段追加。**知识段的生产者三处（本变更新增第三处）**：`source = "archi"` 的架构会话知识、**`source = "change"` 在写权放开时的同一份架构会话知识**（判据 `model_write_lock(provenance) == None`，与 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」同源）、`source = "config"` 的配置协议知识；其余来源恒无此段。

- **单侧在场**：仅知识在场 → 「知识 + 空行 + 指引常量」；仅 `AGENT.md` 在场 → 「`<agent_md …>` 段 + 空行 + 指引常量」；两者皆无 → 正文空串、不注入 System 消息（指引常量 MUST NOT 单独成条）。
- **指引常量**：SHALL 为装配层静态常量（≤ 512 B、无路径字面量、无 `<>` 占位符），语义三点——系统提示词冻结于会话开始、项目规则真身为 `AGENT.md`、用户提示规则已更新时以 `read` 重读 `AGENT.md`。
- **截断口径分立**：`AGENT.md` 的 32KB 截断不变；知识段与指引常量不在该逻辑内（体量由生产者侧测试锚定）。
- **来源约束**：注入位只由装配层按 provenance 决定并经快照供给；前端 / 自由字符串 MUST NOT 直接供给。
- **缓存纪律**：会话内 System 消息 SHALL 逐字恒定（除首轮拍存与存量首补拍）；L2 / L3 防线改写的不是 System 前缀。

#### Scenario: 首轮逐字注入与常量末段

- **WHEN** workspace 执行根存在 `AGENT.md`（`source = "debug"`，无知识段）时以 sdk 引擎发起首轮
- **THEN** 请求系统提示词恰一条 System 消息，内容为「`<agent_md …>` 段（逐字）+ 空行 + 指引常量」；MUST NOT 出现第二条 System 消息

#### Scenario: 缺席维持现状

- **WHEN** 执行根无 `AGENT.md` 且来源无知识段时发起会话
- **THEN** 请求不注入 System 消息（指引常量 MUST NOT 单独成条）

#### Scenario: 会话中修改不改前缀

- **WHEN** 会话运行中途修改 `AGENT.md` 后续发下一轮
- **THEN** 下一轮 MUST NOT 改变 System 消息（首轮拍存后冻结）；规则更新走「用户提示 + `read` 重读」通道

#### Scenario: 无兜底回退

- **WHEN** 执行根只有 `CLAUDE.md` 或 `AGENTS.md`
- **THEN** 不发生兜底读取，快照不含该段

#### Scenario: 三处生产者的单条 System

- **WHEN** 分别以 `source = "archi"`、`(dev-design, executor)` 的 change 会话、`source = "config"` 发起会话，且执行根存在 `AGENT.md`
- **THEN** 三者 chat_history 都恰有一条 System 消息，内容依次为「架构会话知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」/「架构会话知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」（与 archi 臂同一份知识、逐字相同）/「配置协议知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量」；MUST NOT 出现第二条

#### Scenario: implement 臂零知识段

- **WHEN** 以 `(implement, executor)` 的 change 会话发起，执行根存在 `AGENT.md`
- **THEN** System 消息为「`<agent_md …>` 段 + 空行 + 指引常量」（无架构知识段）；工具面仍含 `archi_api` / `archi_drift`

#### Scenario: 引擎零文件读取

- **WHEN** 审查 `crates/infra/agent/src/sdk/loop.rs` 并跑一轮续会话（注入位给定非空正文）
- **THEN** 源码中 `preamble::load` 与 `AGENT.md` 字样零命中；该轮 System 消息逐字等于注入位给定值

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/tools.rs` | 七通用工具执行体 | `write(input)` / `edit(input)` 增 `root: &Path` 入参（两个调用点机械改动）；落盘前判「是否 `<archi_models_rel_dir>/**/<FQN>.schema.json`」→ 是则调 `archi::api::schema::validate_schema`，`Err` 零落盘、成功追加「接口已校验」；`edit` 校验替换后正文；其余工具与 `tools::execute(root, name, input)` 对外签名零改动 |
| `crates/infra/agent/src/compose.rs` | 装配单点 | `assemble_injections` 的 change 臂：`preamble = model_write_lock(provenance).is_none().then(knowledge).flatten()`（单一判据、失败降级 `None`）；`archi_matrix` 两格追加 `ARCHI_API_TOOL`；archi 臂与 config 臂逐字不变 |
| `crates/infra/agent/src/archi_knowledge.rs` | 知识生产 | `knowledge()` 由 archi 臂与 change 臂共用（同一函数、同一文本）；降级 `None` 语义不变 |
| `crates/core/agent/src/runner.rs` | 注入词表 | `SessionInjections` 六位逐字不变（`preamble: Option<String>` 的语义由「archi / config 两处」扩为「archi / change 写权相位 / config 三处」——位形态零改动） |
| `crates/core/archi/src/api/schema.rs::validate_schema` | 校验单点（跨 crate 依赖） | `infra/agent` 经 `archi` crate 的公开面引用（`archi` 已在其依赖内）；MUST NOT 在 `infra/agent` 复写校验 |
| `crates/infra/agent/src/sdk/tools_test.rs`（或既有工具语料位） | 写闸语料 | 三类坏输入各一条 `Err` + 盘面零变化；合法落盘回执；非接口路径零校验；跨元素不越权 |

### API: 工具面（`write` / `edit`）

| 项 | 变化 | 描述 |
|---|---|---|
| `write` / `edit` 的 `parameters` | 零变化 | 仍为 `path` / `content` 与 `path` / `old_string` / `new_string` / `replace_all` |
| 成功响应 | 加一行（条件） | 仅当目标是接口定义文件时追加「接口已校验」；其他路径响应逐字不变 |
| `Err` 面 | 新增一类 | 接口校验失败（第一行自足：文件 + 期望值 + 出路）；既有 `Err`（缺字段 / `old_string` 未命中 / 写失败）逐字不变 |
