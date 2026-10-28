# desktop-archi-agent Specification（delta: archi-page-agent-window）

## MODIFIED Requirements

### Requirement: SessionToolset::Archi 与 (phase, role) 注入矩阵

`core/agent` 的 `SessionToolset` SHALL 新增变体 `Archi { module: bool, search: bool }`（两工具封闭集上的子集表达；serde additive，旧线 JSON 无该字段仍读 `None`）。词汇封闭原则不变：专用工具仅经 `toolset` 枚举注入，自由字符串工具名 MUST NOT 进执行面。

会话装配（`agent-runtime` 组合根）SHALL 按 `(source, phase, role)` 决定注入面：

| phase | role | `archi_module` | `archi_search` |
|---|---|---|---|
| `proposal` | executor | ❌ | ✅ |
| `dev-design` | executor / evaluator | ✅ | ✅ |
| `test-design` | executor | ❌ | ✅ |
| `implement` | executor | ✅ | ❌ |
| `code-review` | evaluator | ✅ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ |
| 其余相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync 等） | 任意 | ❌ | ❌ |

- **archi 来源臂（新增）**：`source = "archi"` 的会话 SHALL 装配为 `tools = None`（七通用工具全量，含 `write` / `edit` / `bash`）+ `toolset = Archi { module: true, search: true }` + `preamble = Some(<架构会话知识文本>)`（知识内容见「架构会话的系统知识注入」）。该臂 SHALL 以 `source` 判定，MUST NOT 进入 `source_ref` 相位解析——`source_ref` 为 null（一 workspace 一份历史）与该定式无关，且其取任意值（含畸形四段）MUST NOT 改变该臂的下发面，MUST NOT 因解析失败退化为零 archi 工具。
- `phase` / `role` SHALL 取自既有 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`（不新增契约字段）；解析 SHALL 取尾三段的定式（`attempt` → `role` → `phase`）。
- 解析不出该定式（畸形 / 短段 / 未知 phase）SHALL **fail-closed**：零 archi 工具（MUST NOT 退化为"给全部工具"）。
- 相位矩阵 SHALL 只对 `source = "change"` 生效（archi 来源臂不查矩阵、恒两工具全开）。
- `source = "explore"` 臂 SHALL 逐字不变（六工具收窄 + `Explore`）；其余非 change 来源（debug 等）SHALL 恒 `SessionInjections::default()`（七工具面，零变化）。
- 注入 archi 工具面时 `tools` 位 SHALL 保持 `None`（七通用工具全量），`toolset` 位携 `Archi`——MUST NOT 以扩宽 `tools` 名单表达 archi 工具（否则"七工具子集"契约被污染）。
- 矩阵只决定**下发面**，不决定 prompt 文本（prompt 侧见 `desktop-phase-prompts`）；`preamble` 位只由装配层按来源决定（当前唯一生产者 = archi 臂），前端 / 自由字符串 MUST NOT 直接供给。

#### Scenario: 逐相位矩阵命中

- **WHEN** 分别以 7 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** 每行 definitions 中 archi 工具恰为矩阵勾选者（proposal 只 `archi_search`；dev-design 两者；implement / code-review / acceptance 只 `archi_module`），`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` / `(code-analyze, executor)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；该轮调用 archi 工具按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具（回落 default），MUST NOT 抛出装配错误、MUST NOT 给全量 archi 工具

#### Scenario: archi 来源全工具面加知识

- **WHEN** 以 `source = "archi"`（`source_ref = null`）发起会话并审查该轮请求组装
- **THEN** `tools` 位为 `None`（七通用工具全量含 `write`）、`toolset = Archi { module: true, search: true }`、`preamble` 为架构会话知识文本；definitions 含 `archi_module` / `archi_search` 两定义且 `RunStarted.tools` 逐名一致

#### Scenario: archi 臂不因 source_ref 变形漂移

- **WHEN** 分别以 `source_ref = null`、`source_ref = "some-id"`、`source_ref = "a/b/c/1"` 发起 `source = "archi"` 会话
- **THEN** 三者装配结果完全相同（同一 archi 臂：两工具全开 + 七工具面 + 知识 preamble）；MUST NOT 因 `source_ref` 形态触发相位解析或 fail-closed

#### Scenario: explore 与 debug 来源零变化

- **WHEN** 分别以 `source="explore"` 与 `source="debug"` 发起会话
- **THEN** explore 为 read / grep / glob / ls / edit / bash + `create_explore_note`（逐字不变），debug 为七工具（含 `write`）；两者均无 archi 工具、`preamble` 均为 `None`

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
- **THEN** 无「写模型」专用工具（`toolset` 词表内只有两个只读工具）；写模型恰有两条通路（change worktree / 架构页通用会话），MUST NOT 存在第三条（如独立写命令或画布写入口）

## ADDED Requirements

### Requirement: 架构会话的系统知识注入

`source = "archi"` 的会话 SHALL 注入一段架构会话知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`），内容 SHALL 覆盖以下关注点：

- **模型位置与聚合**：模型文件位于架构域 `models/` 下、按文件名字典序聚合；目录路径 SHALL 为**派生所得**的 repo 相对路径（见 `workspace-layout-resolution`），MUST NOT 在产品源码或知识文本中手写目录字面量。
- **格式约定**：一元素一表头、键为带引号的完整 FQN 单键（禁点分嵌套写法）；元素字段面为 `kind`（必填）/ `gate` / `path` / `docs` / `summary` / `metadata`；关系为 `[[relation]]` 条目（`from` / `to` / `desc`）；`metadata` MUST 用内联表，MUST NOT 用子表（子表会被解析为另一个元素）。
- **层级与缺省**：层级由 FQN 最长前缀推导（子元素须先声明祖先）；新元素 `gate` 缺省 `off`（ratchet 逐块收紧）。
- **边界**：模型里 MUST NOT 写接口 / 导出符号（契约权威在 `openspec/specs`，只经 `docs` 引用）。
- **自检纪律**：改完 SHALL 立即复查——调用 `archi_module`（不带 `element`）取得薄概览或结构诊断清单（诊断带 `file:line`），按行修复；该纪律 SHALL 为纯提示词约束，MUST NOT 依赖写盘校验或引擎自动校验回灌。
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
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 两 archi 工具不变），MUST NOT 抛装配错误

#### Scenario: 自检纪律在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完调用 `archi_module` 复查并按其 `file:line` 修复」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」三条；且知识文本内零 `<>` 占位符

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/compose.rs` | 注入装配（新增第三臂） | `assemble_injections`：explore 臂逐字不变 → **archi 臂**（`source == "archi"` 短路命中：`preamble = Some(knowledge())`、`tools = None`、`toolset = Archi { module: true, search: true }`，不碰相位解析）→ change 臂（7 行矩阵 + fail-closed，零改动）→ default；`ARCHI_SOURCE` 常量为来源词单点 |
| `crates/infra/agent/src/archi_knowledge.rs`（新） | 知识拼装 | `knowledge() -> Option<String>`：`include_str!("archi-knowledge.md")` + `ElementKind::ALL` / `GateTier::ALL` / `GateTier::default()` / `DiagnosticKind::ALL` / `foundation::layout` 相对路径派生族；纯函数零 IO；失败 → `None` |
| `crates/infra/agent/src/archi-knowledge.md`（新） | 知识静态骨架 | 格式与键形约定 / 层级推导 / `gate` 缺省 / 边界 / 自检 / 写面与可见性 / 历史会话续接 + 一段可解析示例 TOML；零 `<>` 占位符、零 `include_str!` 之外的动态指令 |
| `crates/core/archi/src/dto.rs` | 词汇单点（补 `ALL`） | `ArchiDiagnosticKind::ALL: [Self; 9]`，与 `as_str` 同源；既有 `ElementKind::ALL` / `GateTier::ALL` 复用 |
| `crates/core/foundation/src/layout/mod.rs` | 相对路径派生 | `archi_rel_dir()` / `archi_models_rel_dir()` / `archi_decisions_rel_dir()`（`/` 分隔、零 IO、零行内字面量） |
| `crates/infra/agent/src/sdk/loop.rs` | 系统消息消费 | 由 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」承载：知识 preamble 与 `AGENT.md` 合并为单条 System |
| `crates/infra/agent/src/sdk/tools.rs` / `sdk/policy.rs` / `sdk/archi_tool.rs` | 工具面与执行体（零改动） | archi 臂复用既有两工具定义、只读面三档放行与执行体（root = 会话 cwd = 主仓根） |
