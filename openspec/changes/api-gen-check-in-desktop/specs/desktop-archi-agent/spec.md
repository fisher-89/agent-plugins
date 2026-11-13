# desktop-archi-agent Specification（delta: api-gen-check-in-desktop）

## Purpose

本 delta 给架构 agent 工具面加**第三个只读工具** `archi_api`（自检两轴 + 契约草稿），并把注入矩阵、只读档位归属、知识文本与既有计数表述随动对齐；同时把「`SessionToolset::Archi` 变体」这一**已退役机制的残留表述**校正为实存形态（工具名字清单注入）——代码侧 `SessionToolset` / `toolset` 零命中，spec 不得继续描述已删除的代码。`archi_module` / `archi_search` 两工具的语义、写纪律（模型 = 源码、无专用写工具、两条写通路）与 fail-closed 纪律逐字不变。

## ADDED Requirements

### Requirement: archi_api 只读工具的自检与草稿契约

SDK 引擎 SHALL 提供专用**只读**工具 `archi_api`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**契约面自检**：

```json
{ "name": "archi_api",
  "arguments": { "action": "check" | "bootstrap", "element": "<FQN>" } }
```

- **两动作封闭**：`action` SHALL 只接受 `check` / `bootstrap` 两值（封闭值集；词汇外值以 `is_error` 回灌并点名合法值，MUST NOT 静默退化）。
- **`check` = 两轴结构化结果**：返回「生成物新鲜度」与「契约 vs 代码」两轴的并列内容——① 三类差异（`mismatched` / `orphaned` / `missing`，repo 相对串，取自与 IPC / CLI 同一份比对实现）；② 契约判据摘要（按 `category` 计数的映射 + 逐条 `{ kind, category, element, target, message }`）。响应 SHALL 复用既有出线形状（`ArchiApiCheck` 的字段与 `ArchiApiFinding` 的条目），MUST NOT 在工具执行体内另拼一份投影；MUST NOT 各自实现在第二处（判定单源）。
- **`bootstrap` = 草稿文本**：`element` 必填，返回与 CLI `bootstrap` 同一份草稿文本（契约 md 骨架，只出 stdout 语义）；`element` 缺失 / 元素不存在 / 无实现入口 ⇒ `is_error` 并透传 `ApiError` 文案。
- **零写盘**：两动作 MUST NOT 创建 / 覆盖 / 删除任何文件——生成仍由会话经既有 `bash` 命令（`pnpm api:export` / `cargo run -p archi-api-gen -- generate`）执行。MUST NOT 存在「写生成物」的专用工具（`archi-runtime` 对 agent 面不可达，见 `desktop-crate-layout`）。
- **结构错以 `is_error` 回灌**：模型含任一结构诊断时两动作 SHALL 返回 `is_error` ToolResult（诊断清单带 `file:line`），MUST NOT 返回部分结果、MUST NOT 抛 panic。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）；超限时摘要（按 `category` 计数）优先保留。
- **上下文路由沿用**：执行体 SHALL 经 `LoopTurn` 的 per-turn `ArchiContext` 路由（root + 模型读面），MUST NOT 新增 `ToolExec` 变体、MUST NOT 把 store 句柄塞进七通用工具的 `execute` 签名。
- **无路径参数**：本工具不带 `path` 参数 ⇒ 路径沙箱对它恒原样返回（MUST NOT 因缺字段报错），写入保护对它零射程（读面全开放）。
- **提示面**：被下发本工具的相位 prompt SHALL 有一行说明（沿既有「额外可用只读工具」写法，当前为 `implementation-generator.md`），内容限于「改完代码可自查生成物新鲜度与契约判据；生成物过期用既有命令重生成，MUST NOT 手改生成物」。

#### Scenario: check 返回两轴且零写盘

- **WHEN** 在 implement/executor 会话（或架构页 `source = "archi"` 会话）中调用 `archi_api(action = "check")`
- **THEN** 响应含三类差异与按 `category` 计数的判据摘要（逐条含 `kind` / `element` / `message`）；调用前后磁盘字节完全相同；`archi_api` 与 IPC 的 `archi_api_check` 报的三类差异集合逐条相等

#### Scenario: bootstrap 只出草稿

- **WHEN** 调用 `archi_api(action = "bootstrap", element = "App.Auth")`
- **THEN** 返回契约 md 草稿文本（H1 + 职能 / 不变量 / 操作 / 类型段），磁盘零变化；缺 `element` 或元素不存在 ⇒ `is_error` 且文案点名成因

#### Scenario: 词汇外动作被拒

- **WHEN** 调用 `archi_api(action = "generate")` 或 `action = "write"`（词汇外值）
- **THEN** 返回 `is_error` 并点名合法值 `check` / `bootstrap`；零写盘、零副作用

#### Scenario: 结构错回灌而不崩

- **WHEN** 模型被写成语法错后调用 `archi_api(action = "check")`
- **THEN** 返回 `is_error` ToolResult（诊断带 `file:line`），run 不中断、进程不 panic

#### Scenario: 体量经单点上限

- **WHEN** 契约面有数十条判据时调用 `check`
- **THEN** 响应被 `tools::cap_result` 截断（沿既有 L1 上限），摘要保留；工具执行体内零第二份截断实现

## RENAMED Requirements

### RENAMED: SessionToolset::Archi 与 (phase, role) 注入矩阵 → archi 工具面注入与 (phase, role) 矩阵

- **FROM**: `### Requirement: SessionToolset::Archi 与 (phase, role) 注入矩阵` —— 正文以 `core/agent` 的 `SessionToolset` 枚举变体 `Archi { module: bool, search: bool }` 表达下发面（「专用工具仅经 `toolset` 枚举注入」）。该机制在代码侧已退役（`crates/core/agent` 与 `crates/infra/agent` 的产品源码内 `SessionToolset` / `toolset` 零命中，`sdk/runner_test.rs` 有显式「零命中」断言），实存注入面是 **工具名字清单**（`SessionInjections.tools`），本变更加入第三个 archi 工具时必须一并校正，否则 spec 继续描述已删除的代码。
- **TO**: `### Requirement: archi 工具面注入与 (phase, role) 矩阵` —— 内容如下：

archi 专用工具的**下发面** SHALL 由 `agent-runtime` 组合根以**工具名字清单**表达（`SessionInjections.tools: Option<Vec<String>>`；`None` = 七内建全量），MUST NOT 依赖已退役的枚举式 toolset 概念。清单内每个名字 SHALL 经 `mount::lookup` 成员校验（未知名显式 `Err` 携名与来源，不静默滤过）；自由字符串工具名 MUST NOT 进执行面。

会话装配（`crates/infra/agent/src/compose.rs`）SHALL 按 `(source, phase, role)` 决定下发面：

| phase | role | `archi_module` | `archi_search` | `archi_api` |
|---|---|---|---|---|
| `proposal` | executor | ❌ | ✅ | ❌ |
| `dev-design` | executor / evaluator | ✅ | ✅ | ❌ |
| `test-design` | executor | ❌ | ✅ | ❌ |
| `implement` | executor | ✅ | ❌ | ✅ |
| `code-review` | evaluator | ✅ | ❌ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ | ❌ |
| 其余相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync 等） | 任意 | ❌ | ❌ | ❌ |

- **矩阵单源**：`(phase, role)` → archi 工具名清单的映射 SHALL 只在 `archi_matrix` 一处（未命中即零 archi 工具），MUST NOT 在别处再写一份同名映射。
- **archi 来源臂**：`source = "archi"` 的会话 SHALL 装配为七内建全量（含 `write` / `edit` / `bash`）+ 三个 archi 工具名（`archi_module` / `archi_search` / `archi_api`）+ `preamble = Some(<架构会话知识文本>)`（知识内容见「架构会话的系统知识注入」）。该臂 SHALL 以 `source` 短路命中，MUST NOT 进入 `source_ref` 相位解析——`source_ref` 为 null（一 workspace 一份历史）与该定式无关，且其取任意值（含畸形四段）MUST NOT 改变该臂的下发面，MUST NOT 因解析失败退化为零 archi 工具。
- `phase` / `role` SHALL 取自既有 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`（不新增契约字段）；解析 SHALL 取尾三段的定式（`attempt` → `role` → `phase`），段数 < 4 或末段非数字即 `None`。
- 解析不出该定式（畸形 / 短段 / 未知 phase）SHALL **fail-closed**：零 archi 工具（MUST NOT 退化为"给全部工具"）。
- 相位矩阵 SHALL 只对 `source = "change"` 生效（archi 来源臂不查矩阵、恒三工具全开）。
- `source = "explore"` 臂 SHALL 逐字不变（六工具收窄 + `create_explore_note`）；其余非 change 来源（debug 等）SHALL 恒「七内建全量 + 零 archi 工具 + `ask = Generic`」的显式构造（MUST NOT 以默认构造兜底——默认值的 `ask = None` 即静默禁用）。
- 矩阵只决定**下发面**，不决定 prompt 文本（prompt 侧见 `desktop-phase-prompts`）；`preamble` 位只由装配层按来源决定（当前唯一生产者 = archi 臂），前端 / 自由字符串 MUST NOT 直接供给。
- **`archi_api` 只给 implement/executor 与 archi 臂**：其余相位 MUST NOT 下发（`code-review` / `acceptance` 判「改动是否越界」只读模型面，不自检生成物新鲜度——新鲜度由门禁与契约面板承担）。

#### Scenario: 逐相位矩阵命中

- **WHEN** 分别以 7 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** 每行 definitions 中 archi 工具恰为矩阵勾选者（proposal 只 `archi_search`；dev-design 两工具；implement 三工具；code-review / acceptance 只 `archi_module`），`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` / `(code-analyze, executor)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；该轮调用 archi 工具按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具（回落七内建全量），MUST NOT 抛出装配错误、MUST NOT 给全量 archi 工具

#### Scenario: archi 来源全工具面加知识

- **WHEN** 以 `source = "archi"`（`source_ref = null`）发起会话并审查该轮请求组装
- **THEN** `tools` 位为七内建全量（含 `write`）另加三个 archi 工具名、`preamble` 为架构会话知识文本；definitions 含 `archi_module` / `archi_search` / `archi_api` 三定义且 `RunStarted.tools` 逐名一致

#### Scenario: archi 臂不因 source_ref 变形漂移

- **WHEN** 分别以 `source_ref = null`、`source_ref = "some-id"`、`source_ref = "a/b/c/1"` 发起 `source = "archi"` 会话
- **THEN** 三者装配结果完全相同（同一 archi 臂：三工具全开 + 七内建全量 + 知识 preamble）；MUST NOT 因 `source_ref` 形态触发相位解析或 fail-closed

#### Scenario: explore 与 debug 来源零变化

- **WHEN** 分别以 `source="explore"` 与 `source="debug"` 发起会话
- **THEN** explore 为 read / grep / glob / ls / edit / bash + `create_explore_note`（逐字不变），debug 为七内建（含 `write`）；两者均无 archi 工具、`preamble` 均为 `None`

## MODIFIED Requirements

### Requirement: 只读档位归属与成员校验不变

三个 archi 工具 SHALL 归 **只读面**：挂载清单（`sdk/mount.rs`）中三者的准入档 SHALL 为 `ToolTier::Read`，由此 `policy::allows` 对三档（`default` / `acceptEdits` / `bypassPermissions`）**全部放行**——只读工具在 `default` 档亦可用，与 `read` / `grep` / `glob` / `ls` 同款；MUST NOT 为任一 archi 工具引入单独的放行名单（准入判据单点在挂载清单的准入档）。

以下既有语义 MUST NOT 变化：

- **成员校验先行**：调用名 ∉ 本轮实际下发 definitions 即 `SystemNotice{subtype:"tool_unavailable"}` + `is_error` ToolResult 回灌，run 不中断、不触执行体；
- **路径沙箱不介入**：三工具无路径参数，`sandbox` 改写面对其恒原样返回（MUST NOT 因缺 `path` 字段报错）；
- **写入保护不介入**：三工具为读面，`write_protection` 的守卫 MUST NOT 拦截（读面全开放是既有裁定）；
- **未知工具仍拒**：词汇外的工具名在任何档位一律拒绝（封闭清单语义不变）。
- 工具执行上下文 SHALL 经 `LoopTurn` 的 per-turn 上下文路由（root + 模型读面），MUST NOT 把 store 句柄塞进七通用工具的 `execute` 纯函数签名；上下文缺席 SHALL 以 `is_error` 回灌（生产不可达）。

#### Scenario: 默认档放行三个 archi 工具

- **WHEN** 以 `permissionMode = default` 在矩阵命中相位调用 `archi_module` / `archi_search` / `archi_api`
- **THEN** 三者均放行并执行（无 `permission_denied`）；同档调用 `write` 仍被拒（只读面未扩宽）

#### Scenario: 矩阵外调用被成员校验拒绝

- **WHEN** 在 `(test-gen, executor)` 会话中模型幻觉调用 `archi_search` 或 `archi_api`
- **THEN** 收到 `tool_unavailable` + `is_error` ToolResult 回灌，run 不中断、执行体零调用

#### Scenario: 沙箱与写入保护零介入

- **WHEN** 在三个 archi 工具被下发的会话中审查 `sandbox` 改写与写入保护判定
- **THEN** 三工具均原样通过（无路径参数、读面无保护射程）；受保护文件的 `write` / `edit` 行为逐字不变

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
- **生成物同权但不可手改**：四类契约生成物（`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts`）与模型文件同属磁盘可写面（零专用工具、零写前校验），但会话知识 SHALL 要求「MUST NOT 手改生成物，用既有命令重生成」——该约束同样只有事后兜底（零 diff 门 + 契约面板），MUST NOT 表述为写入面拦截。

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
- **THEN** 无「写模型」专用写工具（注入面可及的 archi 工具只有三个只读工具：`archi_module` / `archi_search` / `archi_api`，且三者 `ToolExec::Archi` 零写盘）；写模型恰有两条通路（change worktree / 架构页通用会话），MUST NOT 存在第三条（如独立写命令或画布写入口）

### Requirement: 架构会话的系统知识注入

`source = "archi"` 的会话 SHALL 注入一段架构会话知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`），内容 SHALL 覆盖以下关注点：

- **模型位置与聚合**：模型文件位于架构域 `models/` 下、按文件名字典序聚合；目录路径 SHALL 为**派生所得**的 repo 相对路径（见 `workspace-layout-resolution`），MUST NOT 在产品源码或知识文本中手写目录字面量。
- **格式约定**：一元素一表头、键为带引号的完整 FQN 单键（禁点分嵌套写法）；元素字段面为 `kind`（必填）/ `gate` / `path` / `docs` / `summary` / `metadata`；关系为 `[[relation]]` 条目（`from` / `to` / `desc`）；`metadata` MUST 用内联表，MUST NOT 用子表（子表会被解析为另一个元素）。
- **层级与缺省**：层级由 FQN 最长前缀推导（子元素须先声明祖先）；新元素 `gate` 缺省 `off`（ratchet 逐块收紧）。
- **边界**：模型里 MUST NOT 写接口 / 导出符号（契约权威在契约资产，只经 `docs` 引用）。
- **自检纪律**：改完 SHALL 立即复查——调用 `archi_module`（不带 `element`）取得薄概览或结构诊断清单（诊断带 `file:line`），按行修复；契约面另可调 `archi_api`（`action = "check"`）查两轴（生成物三类差异 + 契约判据）；生成物过期 SHALL 用既有命令（`pnpm api:export` / `cargo run -p archi-api-gen -- generate`）重生成，MUST NOT 手改生成物。该纪律 SHALL 为纯提示词约束，MUST NOT 依赖写盘校验或引擎自动校验回灌。
- **写面与可见性**：直改主仓（无 worktree、无归档确认），改动随 `git diff` 评审；只改架构域模型与 `docs` 关联文档，改源码须用户明确要求；写盘后画布不自动更新，须提示用户点「刷新」。
- **历史会话续接**：续接历史会话时 SHALL 先取当下模型现状（`archi_module` 无参概览或指定元素）再回答「现在是什么样」一类问题，MUST NOT 依赖会话早前的描述。

词表类内容 SHALL 从代码单点派生，MUST NOT 手抄：

- `kind` 四值 ← `ElementKind::ALL`；`gate` 三值 + 缺省档 ← `GateTier::ALL` / `GateTier::default()`；结构诊断九类词 ← `DiagnosticKind::ALL`（本变更补齐该常量）；目录相对路径 ← `foundation::layout` 的架构域相对路径派生族。
- 知识文本 SHALL 为**静态骨架 md（`include_str!`）+ 上述派生片段**在装配侧拼成的单条文本；新增 / 删除任一封闭词汇而知识未同步时，派生消费面测试 MUST 变红（词汇四面一致性见 `desktop-archi-model`）。

两条工程约束：

- **自证可解析**：知识文本 SHALL 含一段示例 TOML，测试 SHALL 以 `archi` 的解析面解析该片段并要求零结构诊断（字段名 / 内联 `metadata` / 键形任一漂移即红）。
- **尽力增强、不炸 run**：知识拼装失败（派生异常 / 读取失败）SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符（路径由派生注入），体量 SHALL 受上限约束（测试锚定，防与 `AGENT.md` 合体后挤压上下文）。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 词表由代码派生

- **WHEN** 检查知识文本的 `kind` / `gate` / 诊断词清单，并新增一个结构诊断变体（同时补 `DiagnosticKind::ALL`）
- **THEN** 知识文本随即含该新词（无需改静态骨架）；若只加变体而未补 `ALL`，`ALL` 与 `as_str` 对拍测试红

#### Scenario: 示例片段可解析

- **WHEN** 取出知识文本中的示例 TOML 片段并用 `archi` 解析
- **THEN** 零结构诊断；把 `metadata` 改成子表或改掉任一字段名后该测试红

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 三 archi 工具不变），MUST NOT 抛装配错误

#### Scenario: 自检纪律在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完调用 `archi_module` 复查并按其 `file:line` 修复」「契约面可调 `archi_api` 查两轴」「生成物过期用既有命令重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」五条；且知识文本内零 `<>` 占位符

## Module Contract

### Function: `crates/infra/agent/src/sdk/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `mount.rs` | 改 | 静态清单 10 → 11 项：`ARCHI_API_TOOL = "archi_api"`（`ToolTier::Read`、`ToolExec::Archi`）；`ToolTier::Read` 注释与 `ToolExec::Archi` 臂名清单同步 |
| `archi_tool.rs` | 改 | `ArchiContext::execute` 加 `archi_api` 一臂：`check`（`graph_view` → `load_contracts` → `api::check` + `api::compare`，按 `category` 计数摘要）/ `bootstrap`（`api::bootstrap`）；输出过 `tools::cap_result`；零写盘 |
| `runner.rs` | 改 | `mint_archi_context` 的 present 判据加 `archi_api` |
| `archi-knowledge.md` | 改 | 自检纪律 bullet 补 `archi_api` 两轴查询与「生成物用既有命令重生成、MUST NOT 手改」 |

### Function: `crates/infra/agent/src/compose.rs`（矩阵与来源臂）

| 项 | 变化 | 描述 |
|---|---|---|
| 来源臂与矩阵 | 改 | `archi_matrix` 的 `(implement, executor)` 臂追加 `mount::ARCHI_API_TOOL`；archi 来源臂名字清单追加同名；其余相位与臂零变化；`core/agent` 零改动（注入面本就是工具名字清单，无枚举增字段） |

### CLI: `crates/core/workflow/src/prompts/implementation-generator.md`

| 项 | 变化 | 描述 |
|---|---|---|
| 只读工具行 | 改 | 补 `archi_api`（改完代码自查生成物新鲜度与契约判据）；其余相位 prompt 零改动（见 `desktop-phase-prompts`） |
