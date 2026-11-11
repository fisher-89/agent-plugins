# desktop-archi-agent Specification (Delta)

## RENAMED Requirements

### RENAMED: SessionToolset::Archi 与 (phase, role) 注入矩阵 → archi 工具面与 (phase, role) 注入矩阵

- **FROM**: `### Requirement: SessionToolset::Archi 与 (phase, role) 注入矩阵` —— `core/agent` 的 `SessionToolset` 新增 `Archi { module: bool, search: bool }` 变体，专用工具经 `toolset` 枚举注入，`(source, phase, role)` 决定注入面
- **TO**: `### Requirement: archi 工具面与 (phase, role) 注入矩阵` —— 内容如下：

两个 archi 只读工具 SHALL 与其余专用工具**同列**（无第二套注入机制）：`archi_module` / `archi_search` 以**名字**进 `SessionInjections.tools` 名字清单，定义由 infra 挂载清单提供（`ToolTier::Read` / `ToolExec::Archi`，见 desktop-agent-execution「工具挂载单源与 tier 准入」）。`SessionToolset` 枚举与 `toolset` 位已退役，MUST NOT 为本能力恢复枚举承载——词汇封闭原则的现形态是「core 只承载名字，定义与准入档唯一真源在 infra 挂载清单，非法名在装配层 `Err`」。

会话装配（`agent-runtime` 组合根）SHALL 按 `(source, phase, role)` 决定 archi 工具名是否入清单：

| phase | role | `archi_module` | `archi_search` |
|---|---|---|---|
| `proposal` | executor | ❌ | ✅ |
| `dev-design` | executor / evaluator | ✅ | ✅ |
| `test-design` | executor | ❌ | ✅ |
| `implement` | executor | ✅ | ❌ |
| `code-review` | evaluator | ✅ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ |
| 其余相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync 等） | 任意 | ❌ | ❌ |

- **矩阵返回形态**：矩阵行 SHALL 返回**工具名字清单**（`Vec<&'static str>`），与七工具名合并为单一名清单下发；MUST NOT 返回枚举变体、MUST NOT 以扩宽「七工具子集」表达 archi 工具——名字清单即唯一表达。
- **archi 来源臂**：`source = "archi"` 的会话 SHALL 装配为七工具名 + `archi_module` + `archi_search` 共九名 + `preamble = Some(<架构会话知识文本>)`（知识内容见「架构会话的系统知识注入」）。该臂 SHALL 以 `source` 判定，MUST NOT 进入 `source_ref` 相位解析——`source_ref` 为 null（一 workspace 一份历史）与该定式无关，且其取任意值（含畸形四段）MUST NOT 改变该臂的下发面，MUST NOT 因解析失败退化为零 archi 工具。
- `phase` / `role` SHALL 取自既有 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`（不新增契约字段）；解析 SHALL 取尾三段的定式（`attempt` → `role` → `phase`）。
- 解析不出该定式（畸形 / 短段 / 未知 phase）SHALL **fail-closed**：零 archi 工具名（MUST NOT 退化为「给全部工具」）。
- 相位矩阵 SHALL 只对 `source = "change"` 生效（archi 来源臂不查矩阵、恒两工具全开）。
- `source = "explore"` 臂 SHALL 语义不变（六工具 + `create_explore_note`）；其余非 change 来源（debug 等）SHALL 恒 `SessionInjections::default()`（`tools = None` 即内建七件、零 archi 工具，语义零变化）。
- 矩阵只决定**下发面**，不决定 prompt 文本（prompt 侧见 `desktop-phase-prompts`）；`preamble` 位只由装配层按来源决定（当前唯一生产者 = archi 臂），前端 / 自由字符串 MUST NOT 直接供给。

#### Scenario: 逐相位矩阵命中

- **WHEN** 分别以 7 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** 每行 definitions 中 archi 工具恰为矩阵勾选者（proposal 只 `archi_search`；dev-design 两者；implement / code-review / acceptance 只 `archi_module`），`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` / `(code-analyze, executor)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；该轮调用 archi 工具按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具名（回落内建七件），MUST NOT 抛出装配错误、MUST NOT 给全量 archi 工具

#### Scenario: archi 来源全工具面加知识

- **WHEN** 以 `source = "archi"`（`source_ref = null`）发起会话并审查该轮请求组装
- **THEN** 名字清单为七工具 + `archi_module` + `archi_search`（含 `write` / `edit` / `bash`）、`preamble` 为架构会话知识文本；definitions 含两名定义且 `RunStarted.tools` 逐名一致

#### Scenario: archi 臂不因 source_ref 变形漂移

- **WHEN** 分别以 `source_ref = null`、`source_ref = "some-id"`、`source_ref = "a/b/c/1"` 发起 `source = "archi"` 会话
- **THEN** 三者装配结果完全相同（同一 archi 臂：两工具全开 + 七工具名 + 知识 preamble）；MUST NOT 因 `source_ref` 形态触发相位解析或 fail-closed

#### Scenario: explore 与 debug 来源零变化

- **WHEN** 分别以 `source="explore"` 与 `source="debug"` 发起会话
- **THEN** explore 为 read / grep / glob / ls / edit / bash + `create_explore_note`（语义逐字不变，形态为名字清单），debug 为内建七工具（含 `write`）；两者均无 archi 工具、`preamble` 均为 `None`

#### Scenario: 矩阵返回名字清单

- **WHEN** 审查矩阵行的返回形态与下发组装
- **THEN** 矩阵返回 `Vec<&'static str>` 与七工具名合并为单一名清单；下发面全链无 `SessionToolset` 字样（`grep` 零命中），MUST NOT 出现枚举变体或第二个工具清单

## MODIFIED Requirements

### Requirement: archi_module 只读工具的定位契约

SDK 引擎 SHALL 提供专用只读工具 `archi_module`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面名字清单下发——core 承载名字，定义与准入档唯一真源在 infra 挂载清单，见 desktop-agent-execution「工具挂载单源与 tier 准入」），语义为**定位**（已知在看哪个模块）：

```jsonc
{ "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" } }
```

- 指定 `element` 时响应 SHALL 含：`element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine`）、`dependsOn`（`target` + `description`）、`dependedBy`（`source` + `description`）、`children`（直接子元素 FQN）、`docs`（四字段关联条目）、**`api`（并列契约状态字段）**。
- 省略 `element` 时 SHALL 返回全部元素的**薄概览**（`fqn` / `kind` / `gate` / 一行 `summary` / **`api.status`**，无邻接、无文档、无契约路径），并受既有单结果字节上限约束（超限截断留痕）。
- `modelFile` + `modelLine` SHALL 来自解析器已产出的声明位置（零额外实现），使 agent 能直接 `edit` 到该行而无需 grep 猜测。
- 元素不存在 SHALL 返回可诊断 `Err`（附近似 FQN 或总量提示），MUST NOT 返回空对象。
- 模型结构错时 SHALL 返回结构化诊断清单作为 `Err`（绝不 panic），使 agent 能看出"模型本身坏了"而不是"没有这个模块"。
- 响应 SHALL 经 `archi::dto` 单点投影产出（MUST NOT 在工具执行体内另拼一份 JSON）。
- **`api` 并列字段（本变更新增）**：单元素响应 SHALL 并出 `api` 对象（`status`（`declared` / `none`）/ `contractPath` / `schemaPath` / `generatedPath`），定位 SHALL 按 `<archi_api_dir>/<Domain>/<FQN>.md` 与相应的 `.schema.json` / 生成物路径**派生**（全经 `foundation::layout` 的派生族，MUST NOT 行内拼路径）；文件不存在即 `status = none` 且三个路径字段为 `null`（零配置，「有契约 / 无契约」成为可枚举事实）。该字段 MUST NOT 承载协议正文（正文由 agent 用 `read` 按路径自取），MUST NOT 成为独立工具、MUST NOT 改变本工具的只读属性与注入矩阵。

#### Scenario: 定位单元素

- **WHEN** 模型会话中调用 `archi_module(element="App.Auth")`
- **THEN** 返回该元素的 `kind` / `gate` / `path` / `metadata` / `modelFile` / `modelLine`，其出边与入边（各含对端 FQN 与 `description`）、直接子元素清单、`docs` 条目（按强度序）

#### Scenario: 省略元素给薄概览并截断

- **WHEN** 在不传 `element` 的调用下模型含数百元素
- **THEN** 返回全元素薄概览（无邻接无文档），超出单结果字节上限时截断并留痕（提示可指定元素收窄）

#### Scenario: 坏模型可辨

- **WHEN** 模型文件被写坏（语法错 / 环）后调用 `archi_module`
- **THEN** 返回结构化诊断（含 `file:line`）且 `is_error` 为真；执行体不 panic，run 不中断（agent 可转去修模型）

#### Scenario: api 并列字段两态可辨

- **WHEN** 分别对契约在案的元素（`App.Backend.Infra.ArchiRuntime`）与无契约元素调用 `archi_module(element = …)`
- **THEN** 前者 `api.status = "declared"` 且三个路径字段指向实际文件；后者 `api.status = "none"` 且三者 `null`；其余字段（元素 / 邻接 / 子元素 / `docs`）与既有契约逐字一致

#### Scenario: 薄概览带 api.status 但不带路径

- **WHEN** 省略 `element` 调用 `archi_module`（模型含数百元素）
- **THEN** 每条薄概览含 `api.status`（`declared` / `none`），不含 `contractPath` / `schemaPath` / `generatedPath`；超单结果字节上限时仍按既有纪律截断留痕

#### Scenario: 契约正文不经工具响应

- **WHEN** 审查 `archi_module` 响应体与体积
- **THEN** 只含 `api` 四字段（状态 + 三路径），零签名、零 `$defs`、零散文；正文由 agent 用 `read` 按 `contractPath` 自取

### Requirement: 模型 = 源码（写纪律与不设专用写工具）

架构模型文件 SHALL 与源码同权：agent SHALL 经既有 `write` / `edit` 直接新建与修改 `<archi_dir>/models/*.toml`，MUST NOT 存在专用「写模型」工具（因而也不存在"该不该给某相位发写工具"的问题）。写权 SHALL 有且只有**两条通路**：

| 通路 | 触发面 | 执行根 | 确认时机 | 归属能力 |
|---|---|---|---|---|
| A（既有） | change 流程相位会话 | `detail.worktree` | 回合末归档确认面（模型区块 + 冲突分流） | `desktop-change-archive` |
| B（新增） | 架构页通用会话（`source = "archi"`） | workspace root（主仓，`cwd = null` 回落） | 事后 `git diff` 评审 + 用户手动刷新校验 | `desktop-archi-chat` |

- **写入不校验，读取报错**：写盘路径 MUST NOT 增加模型校验插口；坏模型的表现是读取面返回结构诊断（见「archi_module 只读工具的定位契约」）与画布停摆，与"写坏 Rust 代码编译不过"同构。
- **不依赖运行时审批**：写盘那一刻 MUST NOT 要求用户确认（工具分派链只有 `deny`，无挂起等确认能力）。通路 A 的设计权由「回合必须人工确认」承载；通路 B 的设计权由事后 diff 评审 + 手动刷新可见性承载（MUST NOT 承诺写盘前拦截）。
- **写入保护不纳入**：`<archi_dir>/models/` 与架构域配置 MUST NOT 进入 `write_protection` 的内置锁面——保护规则是绝对路径 glob，在 worktree 与主仓两条通路下同样命中，纳入即等于 agent 改不了模型（详见 `desktop-write-protection` 的对应 requirement）。
- **两通路并存，互不替代**：MUST NOT 因通路 B 存在而移除通路 A（change 侧 worktree 隔离 + 归档确认仍是「改动随 change 一起评审」的唯一形态）；亦 MUST NOT 因通路 A 存在而禁通路 B（架构页会话的用途恰是「不进 change 也能维护模型」）。
- **场景归属**：冷启动生成骨架 / 增量维护（新增或重构模块）/ 按现状补声明 / 直接修复图或文档错误，四类场景 SHALL 可走任一条通路（走 change 时落 dev-design 阶段，走架构页时落通用会话），MUST NOT 强制单一入口。
- **新元素缺省档**：两通路生成的新元素均缺省 `gate = "off"`（ratchet 逐块收紧）。
- **通路 B 的边界（软约束，非写入面强制）**：会话知识 SHALL 要求「只改架构域模型与 `docs` 关联文档；改源码须用户明确要求」，但写入面 MUST NOT 为该约束设硬隔离——该缺口 SHALL 在能力 spec 中如实留痕，MUST NOT 被表述为「会话只能改模型」。

#### Scenario: worktree 内直改模型成功

- **WHEN** agent 在 change 的 worktree 内用 `write` 新建 `<archi_dir>/models/03-billing.toml`，或用 `edit` 修改既有模型的 `gate` 行
- **THEN** 写入成功（内置锁面零命中、无额外校验插口）；随后读取面解析出该改动，主仓模型零变化

#### Scenario: 主仓直改模型成功

- **WHEN** agent 在架构页通用会话（`cwd = null` → 主仓根）内用 `write` / `edit` 改 `<archi_dir>/models/*.toml`
- **THEN** 写入落在主仓该文件（内置锁面零命中、无额外校验插口）；随后同 workspace 的 `archi_module` 与画布取数读到该改动，改动保持未提交由用户 `git diff` 评审

#### Scenario: 写坏模型不阻断写盘

- **WHEN** agent 写出语法错的模型文件（任一条通路）
- **THEN** 写盘成功；随后 `archi_module` / 画布数据返回结构诊断（`is_error` / 停摆态）且进程不 panic

#### Scenario: 无专用写工具且两通路并存

- **WHEN** 审查执行面工具清单与写模型入口
- **THEN** 无「写模型」专用工具（工具挂载清单内只有 `archi_module` / `archi_search` 两个 archi 只读工具，二者 tier 均为 Read）；写模型恰有两条通路（change worktree / 架构页通用会话），MUST NOT 存在第三条（如独立写命令或画布写入口）

### Requirement: 只读档位归属与成员校验不变

两个 archi 工具 SHALL 归 **只读面**：挂载清单中两者 `tier = ToolTier::Read`，使三档（`default` / `acceptEdits` / `bypassPermissions`）**全部放行**——只读工具在 `default` 档亦可用，与 `read` / `grep` / `glob` / `ls` 同款。准入判定 SHALL 经 tier 比较单点表达（见 desktop-agent-execution「SDK 引擎静态权限与路径沙箱」），MUST NOT 恢复按工具名的只读常量清单。

以下既有语义 MUST NOT 变化：

- **成员校验先行**：调用名 ∉ 本轮实际下发 definitions 即 `SystemNotice{subtype:"tool_unavailable"}` + `is_error` ToolResult 回灌，run 不中断、不触执行体；
- **路径沙箱不介入**：两工具无路径参数，`tools::input_path` 对其显式返回 `None`、`sandbox` 改写面对其恒原样返回（MUST NOT 因缺 `path` 字段报错）；
- **写入保护不介入**：两工具为读面，`protect::evaluate` 的按名 match 对其走默认放行臂（读面全开放是既有裁定）；
- **未知工具仍拒**：清单外的工具名在任何档位一律拒绝（封闭清单语义不变）。
- 工具执行上下文 SHALL 经 `LoopTurn` 的 per-turn 上下文路由（root + 模型读面），per-turn 铸造判据 SHALL 为**名字清单包含**（`archi_module` 或 `archi_search` 任一在场即铸），MUST NOT 用枚举比较；MUST NOT 把 store 句柄塞进七通用工具的 `execute` 纯函数签名；上下文缺席 SHALL 以 `is_error` 回灌（生产不可达）。

#### Scenario: 默认档放行两个 archi 工具

- **WHEN** 以 `permissionMode = default` 在矩阵命中相位调用 `archi_module`
- **THEN** 调用放行并执行（无 `permission_denied`）；同档调用 `write` 仍被拒（只读档未扩宽）

#### Scenario: 矩阵外调用被成员校验拒绝

- **WHEN** 在 `(test-gen, executor)` 会话中模型幻觉调用 `archi_search`
- **THEN** 收到 `tool_unavailable` + `is_error` ToolResult 回灌，run 不中断、执行体零调用

#### Scenario: 沙箱与写入保护零介入

- **WHEN** 在两个 archi 工具被下发的会话中审查 `sandbox` 改写与写入保护判定
- **THEN** 两工具均原样通过（无路径参数、读面无保护射程）；受保护文件的 `write` / `edit` 行为逐字不变

#### Scenario: per-turn 铸造按名字包含

- **WHEN** 分别以含 `archi_module`、含 `archi_search`、两者皆无的三份名字清单起播
- **THEN** 前两者各铸 archi 执行上下文、第三者不铸；判据为名字包含，源码零 `SessionToolset` 引用

### 被本条取代的主基线 Module Contract 行

| 主基线旧行（片段） | 取代为 |
|---|---|
| `crates/core/agent/src/runner.rs` →「`SessionToolset` 增 `Archi { module: bool, search: bool }`…」 | 本文件「`crates/core/agent/src/runner.rs`」行（枚举删除） |
| `crates/infra/agent/src/compose.rs` →「…`tools = None`、`toolset = Archi { module: true, search: true }`…」 | 本文件「`crates/infra/agent/src/compose.rs`」行（七工具名 + 两 archi 名） |
| `crates/infra/agent/src/sdk/tools.rs` →「`definitions` 在 `toolset` 命中时按位追加…」 | 本文件「`crates/infra/agent/src/sdk/mount.rs`」行（挂载清单 + 名字查表） |
| `crates/infra/agent/src/sdk/policy.rs` →「`READONLY_TOOLS` += `archi_module` / `archi_search`…」 | 本文件「`crates/infra/agent/src/sdk/policy.rs`」行（`tier = Read`） |

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/runner.rs` | 注入词表 | `SessionToolset` 与 `toolset` 位**删除**；archi 工具面以名字表达（定义与准入档归 infra） |
| `crates/infra/agent/src/compose.rs` | 注入装配（三臂） | `assemble_injections`：explore 臂（七名）→ **archi 臂**（`source == "archi"` 短路命中：七工具名 + 两 archi 名 + `preamble = Some(knowledge())`，不碰相位解析）→ change 臂（7 行矩阵产名字清单 + fail-closed）→ default；`ARCHI_SOURCE` 常量为来源词单点；每个名字对挂载清单做成员校验 |
| `crates/infra/agent/src/compose.rs`（`archi_matrix`） | 矩阵返回形态 | `fn(&str, &str) -> Option<Vec<&'static str>>`（7 行不变，值为工具名；MUST NOT 返回枚举） |
| `crates/infra/agent/src/sdk/mount.rs`（新） | 工具定义面 | 两 archi 名常量 + 定义（schema：`element`；`query` + `scope`）原样搬入 `registry()`（`tier = Read`、`exec = Archi`）；`input_path` 对两名返回 `None` 保留在 `tools.rs` |
| `crates/infra/agent/src/sdk/archi_tool.rs` | 工具执行体 | 经 `LoopTurn` per-turn 上下文取 root → `archi::dto` 投影（`archi_module` 两形态、`archi_search` 三 scope）；命中既有 L1 单结果上限收口；坏模型 → `is_error` 不 panic（**零改动**） |
| `crates/infra/agent/src/sdk/policy.rs` | 静态权限表 | 只读归属由「`READONLY_TOOLS += archi_module / archi_search`」改为「两挂载项 `tier = Read`」；三档全放行语义不变 |
| `crates/infra/agent/src/sdk/loop.rs` | 执行臂路由 | 两 archi 名经 `match mount.exec` 的 `Archi` 臂路由至执行体；上下文缺席以 `is_error` 回灌；知识 preamble 与 `AGENT.md` 合并为单条 System（由 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」承载） |
| `crates/infra/agent/src/sdk/runner.rs` | per-turn 铸造 | `mint_archi_context` 入参改名字切片；判据为两名任一在场 |
| `crates/infra/agent/Cargo.toml` | 依赖边 | `archi = { workspace = true }`（零改动，用途注在案） |
| `crates/core/workflow/src/prompts/*.md`（矩阵命中 7 份） | 角色知识 | 只补工具名与用法一句（何时用定位 / 何时用发现）；零占位符、零 MCP、其余 7 份零改动（见 `desktop-phase-prompts`） |
| `crates/core/archi/**`、`crates/infra/agent/src/archi_knowledge.rs` / `archi-knowledge.md` | 零改动 | 投影面与知识文本不涉工具面形态变更 |
