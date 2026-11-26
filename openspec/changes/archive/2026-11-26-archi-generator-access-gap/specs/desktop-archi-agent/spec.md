# desktop-archi-agent Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 给 agent 面加**一个写生成物的工具**，并把「生成物过期怎么办」的三处悬空指代换成可执行引用：

- **新工具 `archi_generate`**：`workspace_render`（纯读）→ 逐份写入保护判定 → 端口 `ArtifactWriter` 委托写盘单点；响应 `{ written, unchanged, conflicts }`；不写模型、不写契约。
- **下发面**：两处起草相位（`proposal` / `dev-design` 的执行侧）与 archi 来源臂各加一格；`implement` 明确不给（契约冻结 ⇒ 生成物恒新鲜）；`test-design` 另补 `archi_module`（契约定死之后要看得见接口键名）。
- **不变量收窄**：矩阵与 `model_write_lock` 的「同集」纪律从「写工具」收窄为「**模型**写工具」——`archi_generate` 是写工具但不写模型，不收窄则对拍必红。
- **旧句改写**：`archi_api` 的「MUST NOT 存在专用写生成物工具」废除、`模型 = 源码` 的「三类生成物没有专用写工具」改写、知识骨架的「用既有命令重生成」换指，并给 `archi_api(check)` 补机器可读处置结论 `remedy`。
- 零新依赖边：端口是 `core/archi` 的纯类型，实现由壳层注入并委托 `write_artifacts` 单点；`agent-runtime` 仍不依赖 `archi-runtime`。

## RENAMED Requirements

- FROM: `### Requirement: archi 工具档位归属与成员校验（五只读 + 两写）`
- TO: `### Requirement: archi 工具档位归属与成员校验（五只读 + 三写）`

## ADDED Requirements

### Requirement: archi_generate 生成物工具（渲染 → 保护 → 写盘端口）

SDK 引擎 SHALL 提供专用工具 `archi_generate`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**按当前模型的契约渲染并写入三类生成物**：

```jsonc
{ "name": "archi_generate", "parameters": {} }   // 无参数：root 取 per-turn 上下文的会话 cwd
```

- **执行链三步，任一步失败即零写盘**：① `archi::workspace_render(root)`（既有单点，纯读、零写盘、零 spawn）；② 对**每一份**目标文件过**同一** `ProtectRules`（规则序与首命中即拒归 `desktop-write-protection`）；③ 全部通过后调端口 `ArchiContext.writer` 的 `write(root, files)`，实现委托 `archi_runtime::api_artifacts::write_artifacts`（唯一写盘单点）。MUST NOT 在执行体内重实现渲染、比对或写盘，MUST NOT 出现第二份 `fs::write` 生成物。
- **响应三字段**：`{ written: string[], unchanged: string[], conflicts: [{ path, reason }] }`（repo 相对 POSIX 串）。`conflicts` 非空 ⇒ `written` 与 `unchanged` 皆空（整批拒绝、零写盘），工具仍返回 `Ok`（冲突要 agent 转告用户处置，不是 run 的错误）。该形状 SHALL 在**工具响应层**拼装：`ArchiApiWriteOutcome` 与 IPC DTO 逐字不变，MUST NOT 为它新增 DTO 字段或枚举。
- **幂等**：字节相同的目标计入 `unchanged` 且不重写（`mtime` 不动）；生成器**从不删除**文件（孤儿生成物只由检查命令报告）。
- **零产出是正常**：`render_all` 对「`path` 为空 / 无 `<叶段>.schema.json` / 接口面留白（无 op 无类型无透传）/ 入口未解析」的元素一律跳过 ⇒ 新模块在起草期零产出常见，MUST NOT 以 `Err` 表达。
- **档位与归属**：`ToolTier::Write`（`default` 档拒绝，`acceptEdits` / `bypassPermissions` 放行，与 `write` / `edit` 同档）、`ToolExec::Archi`、无路径参数（`tools::input_path` 对其恒 `None`，沙箱改写面原样返回）。写入保护对它**有射程**（执行体内逐份判定），MUST NOT 因「无 `path` 入参」判在射程之外。
- **写作范围封闭**：本工具只写三类生成物（架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`），MUST NOT 写模型 TOML、MUST NOT 写接口定义或契约文档（后者归 `write` / `edit` 与写闸）。
- **上下文缺席**：端口未装配（生产不可达）或 per-turn 上下文缺席 SHALL 以 `is_error` 回灌，MUST NOT panic、MUST NOT 静默成功。
- **描述面（`mount.rs`）SHALL 含五句，且 MUST NOT 省略任意一句**：

  ① **用法与响应**：按当前模型的契约渲染并写入三类生成物，幂等（字节相同即不重写、不动 `mtime`），返回 `written` / `unchanged` / `conflicts`；
  ② **零产出**：入口未解析 / 接口面留白 / 未写接口定义的元素不产生生成物——零产出是正常，不是失败；
  ③ **孤儿**：生成器从不删除孤儿生成物，`orphaned` 非空属常态，MUST NOT 为了清空它去手改生成物；
  ④ **覆盖风险**：生成会覆盖同名 `<入口主干>.i.*`——调用前先 `git status` 确认工作树干净，以便意外可 `git restore` 回滚；
  ⑤ **冲突处置**：`conflicts` 非空即被写保护拒绝、整批零写盘；把冲突报告给用户，MUST NOT 换工具重试。

  描述 MUST NOT 内建本仓专用命令名（生成命令归属各仓文档面）、MUST NOT 把 `.schema.json` 说成生成物。

#### Scenario: 生成成功且第二次幂等

- **WHEN** 在 `(dev-design, executor)` 会话中连续两次调用 `archi_generate`
- **THEN** 第一次的 `written` 与盘上实际变化一一对应、第二次 `written` 为空且全部目标进 `unchanged`；两次调用期间未变化的生成物 `mtime` 不变；响应的三个键恒在场

#### Scenario: 零产出三成因不被当作失败

- **WHEN** 对「模型有元素但没写 `.schema.json`」「schema 在盘但 `properties` 与 `$defs` 皆空」「schema 有 op 但入口路径不在盘」三类元素调用 `archi_generate`
- **THEN** 三次均返回 `written = []` 与 `unchanged = []`、`is_error` 为假；磁盘零变化；坏模型（结构错）才走 `is_error`

#### Scenario: 冲突整批零写盘

- **WHEN** 应生成集里任一份目标命中用户 `write_protection.files` 规则（其余不命中）时调用
- **THEN** `conflicts` 恰含该份及其 `reason`，`written` 与 `unchanged` 皆为空数组，**命中之前的其它目标也不写**（磁盘字节零变化）；工具返回 `Ok` 而非 `Err`

#### Scenario: 端口缺席与坏模型各自可辨

- **WHEN** 分别在端口未装配的上下文调用 `archi_generate`、在模型被写坏后调用
- **THEN** 前者 `is_error`（文案点名装配缺失）、后者 `is_error` 且诊断带 `file:line`；两例均零写盘、进程不 panic、run 不中断

#### Scenario: 描述面五句与写作范围

- **WHEN** 审查 `mount.rs` 中 `archi_generate` 的定义与描述
- **THEN** `parameters` 无属性（空对象）；描述含用法与三字段、零产出正常、孤儿常态、覆盖风险与 `git status`、冲突处置共五条语义；描述内零本仓命令名（`api:export` 一类零命中）、零「`.schema.json` 是生成物」表述

## MODIFIED Requirements

### Requirement: archi_api 只读工具的自检与草稿契约

SDK 引擎 SHALL 提供专用**只读**工具 `archi_api`（不在七通用工具 `TOOL_NAMES` 封闭集内，经注入面的工具名字清单受控下发，且经 `mount::lookup` 成员校验），语义为**接口面自检、起草与引用计算**（`check` 自检、`bootstrap` 双草稿、`link` 算 `$ref`）：

```json
{ "name": "archi_api",
  "arguments": { "action": "check" | "bootstrap" | "link", "element": "<FQN>",
                 "from": "<FQN>", "to": "<FQN>", "name": "<导出名>" } }
```

- **三动作封闭**：`action` SHALL 只接受 `check` / `bootstrap` / `link` 三值（封闭值集；词汇外值以 `is_error` 回灌并点名三个合法值，MUST NOT 静默退化）。
- **`check` = 两轴结构化结果**：返回「生成物新鲜度」与「契约 vs 代码」两轴的并列内容——① 三类差异（`mismatched` / `orphaned` / `missing`，repo 相对串，取自与 IPC / CLI 同一份比对实现）；② 契约判据摘要（按 `category` 计数的映射 + 逐条 `{ kind, category, element, target, message }`）。响应 SHALL 复用既有出线形状（`ArchiApiCheck` 的字段与 `ArchiApiFinding` 的条目），MUST NOT 在工具执行体内另拼一份投影；MUST NOT 各自实现在第二处（判定单源）。**历史 `api-*` fence MUST NOT 产生任何计数或提示**（它已退化为普通文档内容）。
- **`check` 的处置结论 `remedy`（本变更新增，工具响应层）**：`check` 响应 SHALL 另含一个顶层 `remedy` 键，按三态取值——① 三类差异全空 ⇒ `null`（无处置可给）；② 有差异且**本轮下发面含** `archi_generate` ⇒ 指向该工具本身（调用即生成，MUST NOT 手改生成物）；③ 有差异且本轮**无**该工具（如 `implement/executor`）⇒ 指向「请用户在架构页会话生成」并明写「MUST NOT 手改生成物」。该键 SHALL 只存在于工具响应层：MUST NOT 进 `ArchiApiCheck` DTO、MUST NOT 进 bindings、MUST NOT 成为枚举或新词汇（与既有「agent 面是文本、人类面是结构化 DTO」的差异同源）。
- **`bootstrap` = 双草稿（起草第一步）**：`element` 必填，返回 `{ schema, doc }` 两段草稿文本——与 CLI `bootstrap` 同一份内容（形态归 `desktop-archi-api`「bootstrap 双草稿（schema 骨架 + doc 骨架）」），本能力 MUST NOT 另立第二份形态判据。该动作是新建 / 补接口的**第一步**，描述面 SHALL 说出这句指路（否则 agent 只知它能出草稿、不知该先用它）。`element` 缺失 / 元素不存在 / 无实现入口 ⇒ `is_error` 并透传 `ApiError` 文案。
- **`link` = `$ref` 串计算（纯计算，零读盘写盘）**：
  - 跨元素形态：`{ from, to, name }` → `"<ref_relative(from, to)>#/$defs/<name>"`（例：`"../../Desktop/Components/Agent/AgentInput.schema.json#/$defs/AgentInputProps"`）；
  - 本模块形态：`{ element, name }` → `"#/$defs/<name>"`；
  - `from` / `to` 缺失或非 FQN 形态、`name` 非 ASCII 标识符 ⇒ `is_error` 点名成因；两形态互斥（同给 `to` 与省略 `to` 以外的一切组合都拒）。
  - 描述面 SHALL 明写「跨元素 `$ref` 用 `link` 算，勿手拼」——手算 `../` 层数少一层会绕过 `apiRefMissing`，到编译期才炸。
- **零写盘（本变更改写）**：三动作 MUST NOT 创建 / 覆盖 / 删除任何文件——落盘仍由会话经 `write` / `edit`（写闸校验）执行。**生成物过期不再由本工具或「本仓命令」承担**：渲染与写盘由专用工具 `archi_generate` 经端口委托写盘单点完成（见「archi_generate 生成物工具（渲染 → 保护 → 写盘端口）」）；`archi_api` 仍**只读**，且 MUST NOT 因生成工具存在而扩动作或升档。旧句「MUST NOT 存在『写生成物 / 写契约』的专用工具」**作废**：写生成物的专用工具 SHALL 存在（`archi_generate`），但 `archi-runtime` 对 agent 面仍不可达（写盘经 `core/archi` 的端口由壳层注入，见 `desktop-crate-layout`）；写契约仍无专用工具（走 `write` / `edit` + 写闸）。
- **结构错以 `is_error` 回灌**：模型含任一结构诊断时三动作 SHALL 返回 `is_error` ToolResult（诊断清单带 `file:line`），MUST NOT 返回部分结果、MUST NOT 抛 panic。
- **体量受上限约束**：响应 SHALL 经 `tools::cap_result` 的既有 L1 上限单点（MUST NOT 为本工具新增截断逻辑）；超限时摘要（按 `category` 计数）优先保留；`remedy` 是短串，MUST NOT 因它改上限。
- **上下文路由沿用**：执行体 SHALL 经 `LoopTurn` 的 per-turn `ArchiContext` 路由（root + 模型读面），MUST NOT 新增 `ToolExec` 变体、MUST NOT 把 store 句柄塞进七通用工具的 `execute` 签名。
- **无路径参数**：本工具不带 `path` 参数 ⇒ 路径沙箱对它恒原样返回（MUST NOT 因缺字段报错），写入保护对它零射程（读面全开放）。
- **提示面（本变更改写）**：被下发本工具的相位 prompt SHALL 有一行说明（沿既有「额外可用只读工具」写法，当前为 `implementation-generator.md`），内容限于「改完代码可自查生成物新鲜度与契约判据（`check`）；**本相位不改契约**——契约不符时改代码追上契约，或停下说明冲突由用户决定 backtrack」；MUST NOT 再写「生成物过期用既有命令重生成」（该指代在通用工作区不成立）。
- **描述面（`mount.rs`）**：`archi_api` 的工具描述 SHALL 含四句，且 MUST NOT 复制契约 md 的段形态长表（形态归 `desktop-archi-api`）、MUST NOT 内建本仓专用命令名（生成命令归属各仓文档面）——① `bootstrap` 是新建 / 补接口的第一步（给两份草稿）；② `link` 算 `$ref` 串（勿手拼）；③ **三类**生成物的点名（架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）连同「MUST NOT 手改生成物」，且过期时改由 `archi_generate` 重生成；④ 冷启动口径「`artifacts.enabled = false` 即接口面未启用，此时 `findings` 恒空且**不等于**零问题」。MUST NOT 把 `.schema.json` 说成生成物。

#### Scenario: check 返回两轴且历史 fence 零计数

- **WHEN** 在 implement/executor 会话（或架构页 `source = "archi"` 会话）中调用 `archi_api(action = "check")`，某元素的文档仍含两个历史 `api-op` fence
- **THEN** 响应含三类差异与按 `category` 计数的判据摘要（逐条含 `kind` / `element` / `message`）；响应内零「fence 残留」类计数键；调用前后磁盘字节完全相同；`archi_api` 与 IPC 的 `archi_api_check` 报的三类差异集合逐条相等

#### Scenario: remedy 三态

- **WHEN** 分别在「三类差异全空」「有 `mismatched` 且本轮下发 `archi_generate`（`dev-design/executor`）」「有 `mismatched` 且本轮无该工具（`implement/executor`）」三种会话调用 `check`
- **THEN** `remedy` 依次为 `null` / 指向 `archi_generate` 的短串 / 指向「架构页会话生成」且含「MUST NOT 手改生成物」的短串；三例的 `artifacts` 与 `findings` 两轴内容不受 `remedy` 影响

#### Scenario: remedy 不进 DTO 面

- **WHEN** 审查 `check` 响应键集与 `bindings.ts` / `bindings.zod.ts`
- **THEN** 工具响应含 `remedy`，而 `ArchiApiCheck` 的 DTO 键集（`enabled` / `meta` / `mismatched` / `orphaned` / `missing`）与两份 bindings 文件逐字不变（零 `remedy` 字样）

#### Scenario: bootstrap 双草稿只出文本

- **WHEN** 调用 `archi_api(action = "bootstrap", element = "App.Auth")`
- **THEN** 返回 `{ schema, doc }`：`schema` 含 `$id` / `title` / `properties` 占位与 `$defs`，`doc` 只有 H1 + 一句指引（形态归 `desktop-archi-api`）；磁盘零变化；缺 `element` 或元素不存在 ⇒ `is_error` 且文案点名成因

#### Scenario: link 两形态各出正确串

- **WHEN** 分别以 `{ from: "Backend.Infra.ArchiRuntime", to: "Backend.Core.Foundation", name: "Summary" }` 与 `{ element: "Backend.Core.Foundation", name: "Summary" }` 调用
- **THEN** 前者出 `"../../Core/Foundation/Backend.Core.Foundation.schema.json#/$defs/Summary"`（前缀与 `paths::ref_relative` 逐字一致）、后者出 `"#/$defs/Summary"`；两次都零写盘

#### Scenario: link 参数面互斥与校验

- **WHEN** 以 `{ from, name }`（缺 `to` 且缺 `element`）、`{ from, to, name: "a.b" }`、`action = "generate"` 分别调用
- **THEN** 三者均 `is_error`：前者点名缺 `to` / `element`，中者点名 `name` 须为标识符，后者点名合法三值（`check` / `bootstrap` / `link`，`generate` 不是本工具的动作）；零写盘

#### Scenario: 词汇外动作被拒

- **WHEN** 调用 `archi_api(action = "generate")` 或 `action = "write"`（词汇外值）
- **THEN** 返回 `is_error` 并点名合法值 `check` / `bootstrap` / `link`；零写盘、零副作用

#### Scenario: 结构错回灌而不崩

- **WHEN** 模型被写成语法错后调用 `archi_api(action = "check")`
- **THEN** 返回 `is_error` ToolResult（诊断带 `file:line`），run 不中断、进程不 panic

#### Scenario: 体量经单点上限

- **WHEN** 契约面有数十条判据时调用 `check`
- **THEN** 响应被 `tools::cap_result` 截断（沿既有 L1 上限），摘要保留；工具执行体内零第二份截断实现

### Requirement: archi 工具面注入与 (phase, role) 矩阵

archi 专用工具的**下发面** SHALL 由 `agent-runtime` 组合根以**工具名字清单**表达（`SessionInjections.tools: Option<Vec<String>>`；`None` = 七内建全量），MUST NOT 依赖已退役的枚举式 toolset 概念。清单内每个名字 SHALL 经 `mount::lookup` 成员校验（未知名显式 `Err` 携名与来源，不静默滤过）；自由字符串工具名 MUST NOT 进执行面。

会话装配（`crates/infra/agent/src/compose.rs`）SHALL 按 `(source, phase, role)` 决定下发面：

| phase | role | `archi_module` | `archi_search` | `archi_api` | `archi_drift` | `archi_scaffold` | `archi_element` | `archi_relation` | `archi_generate` |
|---|---|---|---|---|---|---|---|---|---|
| `proposal` | executor | ✅ | ✅ | **✅** | ❌ | ✅ | ✅ | ✅ | **✅** |
| `dev-design` | executor | ✅ | ✅ | **✅** | ✅ | ✅ | ✅ | ✅ | **✅** |
| `dev-design` | evaluator | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `test-design` | executor | **✅** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `implement` | executor | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `code-review` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `acceptance` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 其余相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync 等） | 任意 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

- **`archi_api` 的两格扩面（本变更）**：`(proposal, executor)` / `(dev-design, executor)` 由 ❌ 改 ✅。理由：接口定义成了手写真相，`bootstrap` 双草稿与 `link` 的 `$ref` 计算价值恰在起草相位；不补则 `archi_scaffold` 描述里「接口草稿用 `archi_api` 的 `bootstrap`」指向一个未下发的工具。`archi_api` 是**只读**档，扩面不扩写权（两**模型**写工具的矩阵集与 `model_write_lock` 的同集纪律逐字不变）。
- **`archi_generate` 的两格 + 臂（本变更新增）**：只给两处**契约起草相位**（`proposal/executor`、`dev-design/executor`）与 archi 来源臂。理由：生成物的渲染输入只有 schema，代码变化不影响生成物 ⇒ 契约冻结的 `implement` 期生成物恒新鲜，不给即正确；而通用工作区没有「本仓生成命令」，起草相位与架构页会话是唯一可行落点。`implement/executor` MUST NOT 下发（它仍持 `archi_api`，据 `remedy` 的第三态指向架构页会话）。
- **`test-design/executor` 加 `archi_module`（本变更新增）**：契约在 `dev-design` 定死之后，「对着接口协议设计测试」需要 `assets.interface` 的 `ops` / `types` 键名，而 `archi_search` 只给发现面。该格 SHALL 恰为 `[archi_search, archi_module]`，MUST NOT 追加其它 archi 工具。
- **矩阵单源**：`(phase, role)` → archi 工具名清单的映射 SHALL 只在 `archi_matrix` 一处（未命中即零 archi 工具），MUST NOT 在别处再写一份同名映射。`dev-design` 的 executor / evaluator **分两行**（`drift` 与两写工具只给写模型的执行侧），MUST NOT 合并成一行后按 role 分支。
- **写权不变量收窄（本变更改写）**：矩阵下发**模型写工具**（`archi_element` / `archi_relation`）的 `(phase, role)` 集合 SHALL 与 `compose::model_write_lock` 判定为 `None`（不挂会话锁）的集合**逐格相等**——MUST NOT 出现「矩阵给了工具但写权锁着」或「锁开了却没有写工具」的错配（两条方向都设断言）。**`archi_generate` 不在该对拍集**：它是写工具但不写模型（只写三类生成物），相位锁对它零射程，MUST NOT 为凑对拍而把它下发给 `implement` 或从起草相位撤掉。
- **`archi_drift` 只给两处直接产出漂移的相位**：`dev-design/executor`（写元素与边）与 `implement/executor`（改代码 ⇒ 未声明依赖）。`code-review` / `acceptance` MUST NOT 下发（越界判断仍只读模型面；「时机差」作为将来放宽的理由需独立裁决，MUST NOT 由本期实现顺手放开）。
- **`archi_scaffold` 只给两处写权相位**（`proposal/executor`、`dev-design/executor`）与 archi 来源臂——模型写权之外的下发等于许诺一个不会被允许落地的模板。
- **`archi_api` 只给三处并集**（`proposal/executor`、`dev-design/executor`、`implement/executor`）与 archi 来源臂：其余相位 MUST NOT 下发（生成物新鲜度由门禁与契约面板承担）。
- **archi 来源臂**：`source = "archi"` 的会话 SHALL 装配为七内建全量（含 `write` / `edit` / `bash`）+ **八个** archi 工具名（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation` / `archi_generate`）+ `preamble = Some(<架构会话知识文本>)`（知识内容见「架构会话的系统知识注入」）+ `before_done = Some(ArchiPreclose)`（收口前自检装配档位，见「收口前引擎自检（架构判据读数）」）。该臂 SHALL 以 `source` 短路命中，MUST NOT 进入 `source_ref` 相位解析——`source_ref` 为 null（一 workspace 一份历史）与该定式无关，且其取任意值（含畸形四段）MUST NOT 改变该臂的下发面，MUST NOT 因解析失败退化为零 archi 工具。该臂是「full 模式页内会话清理过期生成物」的**唯一通路**（画布仍零生成入口，见 `desktop-archi-canvas`）。
- `phase` / `role` SHALL 取自既有 provenance `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}`（不新增契约字段）；解析 SHALL 取尾三段的定式（`attempt` → `role` → `phase`），段数 < 4 或末段非数字即 `None`。
- 解析不出该定式（畸形 / 短段 / 未知 phase）SHALL **fail-closed**：零 archi 工具（MUST NOT 退化为"给全部工具"）。
- 相位矩阵 SHALL 只对 `source = "change"` 生效（archi 来源臂不查矩阵、恒八工具全开）。
- `source = "explore"` 臂的 **archi 面 SHALL 恒为空**（零 archi 工具，逐字沿用）；该臂的**完整**名字清单（六工具收窄 + `create_explore_note` + 只读 DB 工具 `db_tables` / `db_query` 两件）单源见 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」与 `desktop-agent-db-tools`，本能力 MUST NOT 另写一份；其余**非 change、非 config** 来源（debug 等）SHALL 恒「七内建全量 + 零 archi 工具 + `ask = Generic`」的显式构造（`source = "config"` 臂的名字清单与位取值单源见 `desktop-config-agent` 与 `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」；该臂零 archi 工具、`preamble` 为配置协议知识而非架构会话知识，本能力 MUST NOT 另写一份；MUST NOT 以默认构造兜底——默认值的 `ask = None` 即静默禁用）。
- **change 臂的知识段（本变更）**：`source = "change"` 会话在 `model_write_lock(provenance) == None`（即两个写权相位）时 SHALL 注入**同一份**架构会话知识（`archi_knowledge::knowledge()`，单来源）；其余相位（含 `implement/executor` —— 它有 archi 只读工具但不能写模型，也**没有** `archi_generate`）SHALL 保持 `preamble = None`。判据 MUST NOT 另立一份矩阵命中判定（写权与知识同源）。知识拼装失败 SHALL 降级为 `None`，MUST NOT 影响工具面与 run。
- 矩阵只决定**下发面**，不决定 prompt 文本（prompt 侧见 `desktop-phase-prompts`）；`preamble` 位只由装配层按来源决定（生产者 = archi 臂 / change 写权相位 / config 臂），前端 / 自由字符串 MUST NOT 直接供给。

#### Scenario: 逐相位矩阵命中（含 generate 与 test-design 两格）

- **WHEN** 分别以 8 行矩阵的 `(phase, role)` 发起会话并审查该轮 definitions
- **THEN** 每行 definitions 中 archi 工具恰为矩阵勾选者（proposal 为 `module` / `search` / `api` / `scaffold` / `element` / `relation` / `generate`；dev-design 执行为 `module` / `search` / `api` / `drift` / `scaffold` / `element` / `relation` / `generate`、评估为 `module` / `search`；test-design 为 `search` / `module`；implement 为 `module` / `api` / `drift`（**零 `generate`**）；code-review / acceptance 只 `module`），`RunStarted.tools` 与实际下发面逐名一致

#### Scenario: 写工具与写权锁同集（收窄到模型写工具）

- **WHEN** 遍历全部 `(phase, role)` 组合，对每个组合分别取矩阵结果与 `model_write_lock` 的返回值
- **THEN** 「矩阵含 `archi_element`」⟺「`model_write_lock` 返回 `None`」，两个方向逐格相等（archi 来源臂同判）；对拍集合恰为 `{archi_element, archi_relation}`——`archi_generate` 在「矩阵含它」而「挂锁」的格（`implement/executor`）为假、在「不挂锁」的格（两起草相位、archi 臂）为真，且它 MUST NOT 参与该对拍；任一格不成立即测试红

#### Scenario: 知识段与写权同源

- **WHEN** 分别以 `(proposal, executor)` / `(dev-design, executor)` / `(implement, executor)` / `(code-review, evaluator)` 发起 change 会话并审查 `preamble` 位
- **THEN** 前两者 `preamble` 为架构会话知识（与 archi 臂逐字相同）；后两者 `preamble` 为 `None`；四者的工具面仍按矩阵下发（`implement` 有 `api` / `drift` 但零 `generate`）

#### Scenario: 知识降级不影响工具面

- **WHEN** 知识拼装失败（派生异常）后以 `(dev-design, executor)` 发起会话
- **THEN** `preamble = None`，会话照常发起、矩阵命中的工具面与 `before_done` 位不变

#### Scenario: 矩阵外零 archi 工具

- **WHEN** 以 `(test-gen, executor)` / `(test-execution, executor)` / `(archive, spec_sync)` / `(code-analyze, executor)` 发起会话
- **THEN** definitions 与 `RunStarted.tools` 均不含任何 archi 工具；该轮调用 archi 工具（含 `archi_generate`）按成员校验拒绝

#### Scenario: 畸形 source_ref fail-closed

- **WHEN** `source = "change"` 而 `source_ref` 缺失 / 段数不足 / phase 段不可识别
- **THEN** 装配结果为零 archi 工具（回落七内建全量）、`preamble = None`（写权判据亦不成立），MUST NOT 抛出装配错误、MUST NOT 给全量 archi 工具

#### Scenario: archi 来源全工具面加知识

- **WHEN** 以 `source = "archi"`（`source_ref = null`）发起会话并审查该轮请求组装
- **THEN** `tools` 位为七内建全量（含 `write`）另加**八个** archi 工具名、`preamble` 为架构会话知识文本、`before_done` 为 `Some(ArchiPreclose)`；definitions 含八个 archi 定义且 `RunStarted.tools` 逐名一致

#### Scenario: archi 臂不因 source_ref 变形漂移

- **WHEN** 分别以 `source_ref = null`、`source_ref = "some-id"`、`source_ref = "a/b/c/1"` 发起 `source = "archi"` 会话
- **THEN** 三者装配结果完全相同（同一 archi 臂：八工具全开 + 七内建全量 + 知识 preamble）；MUST NOT 因 `source_ref` 形态触发相位解析或 fail-closed

#### Scenario: explore 与 debug 来源零 archi 工具

- **WHEN** 分别以 `source="explore"` 与 `source="debug"` 发起会话
- **THEN** explore 为 read / grep / glob / ls / edit / bash + `create_explore_note` + `db_tables` / `db_query`（只读 DB 工具两件，清单单源见 desktop-agent-db-tools），debug 为七内建（含 `write`）；两者均无 archi 工具、`preamble` 均为 `None`

### Requirement: archi 工具档位归属与成员校验（五只读 + 三写）

八个 archi 工具 SHALL 按读写面分两档：`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` 归 **只读面**（挂载清单 `sdk/mount.rs` 中准入档 SHALL 为 `ToolTier::Read`，`policy::allows` 对三档——`default` / `acceptEdits` / `bypassPermissions`——**全部放行**）；`archi_element` / `archi_relation` / `archi_generate` 归 **写面**（准入档 SHALL 为 `ToolTier::Write`：`default` 档拒绝，`acceptEdits` / `bypassPermissions` 放行，与 `write` / `edit` 同档）。MUST NOT 为任一 archi 工具引入单独的放行名单（准入判据单点在挂载清单的准入档）。

以下既有语义 MUST NOT 变化：

- **成员校验先行**：调用名 ∉ 本轮实际下发 definitions 即 `SystemNotice{subtype:"tool_unavailable"}` + `is_error` ToolResult 回灌，run 不中断、不触执行体；
- **路径沙箱不介入**：八个工具无路径参数，`sandbox` 改写面对其恒原样返回（MUST NOT 因缺 `path` 字段报错；两写工具的落点由 `file` / 父元素推得，`archi_generate` 的 root 取 per-turn 会话 cwd，`tools::input_path` 对三者恒 `None`）；
- **写入保护按面分**：五只读工具为读面，`write_protection` 的守卫 MUST NOT 拦截；三个**写**工具写盘前 SHALL 过同一 `ProtectRules`——两模型写工具在落点解析之后按目标文件判定，`archi_generate` 在写盘前**逐份**判定应生成物（任一份命中即整批拒绝）（细则归 `desktop-write-protection`「会话级写入禁区（相位锁）」）；
- **未知工具仍拒**：词汇外的工具名在任何档位一律拒绝（封闭清单语义不变）。
- 工具执行上下文 SHALL 经 `LoopTurn` 的 per-turn 上下文路由（root + 模型读面 + 生成物写盘端口），MUST NOT 把 store 句柄塞进七通用工具的 `execute` 纯函数签名；上下文缺席 SHALL 以 `is_error` 回灌（生产不可达）。铸造判据 SHALL 为「本轮名字清单含任一 archi 工具名」（**八名**逐一在场判断，MUST NOT 漏掉新增名）。

#### Scenario: 默认档放行五个 archi 工具

- **WHEN** 以 `permissionMode = default` 在矩阵命中相位调用 `archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`
- **THEN** 五者均放行并执行（无 `permission_denied`）；同档调用 `write` 与 `archi_element` / `archi_relation` / `archi_generate` 仍被拒（只读面未扩宽）

#### Scenario: 写工具在非默认档放行且先过成员校验

- **WHEN** 以 `permissionMode = acceptEdits` 在矩阵命中相位调用 `archi_element` / `archi_relation` / `archi_generate`；另在矩阵**未命中**相位以同一档位调用三者
- **THEN** 前者放行并执行；后者被成员校验拒绝（`tool_unavailable` + `is_error`，run 不中断、执行体零调用）——档位与成员校验是两道独立闸，任一不通过都不落到写盘

#### Scenario: 矩阵外调用被成员校验拒绝

- **WHEN** 在 `(test-gen, executor)` 会话中模型幻觉调用 `archi_search` 或 `archi_drift` 或 `archi_generate`
- **THEN** 收到 `tool_unavailable` + `is_error` ToolResult 回灌，run 不中断、执行体零调用

#### Scenario: 沙箱与写入保护按面分

- **WHEN** 在八个 archi 工具被下发的会话中审查 `sandbox` 改写与写入保护判定
- **THEN** 八个均原样通过沙箱（无 `path` 入参）；五只读工具零保护射程，三个写工具受保护判定约束（`archi_generate` 为逐份）；受保护文件的 `write` / `edit` 行为逐字不变

#### Scenario: 挂载清单项数

- **WHEN** 审查 `mount::registry()` 的项数与归组
- **THEN** 恰 **19** 项：七内建 + `create_explore_note` + 五 archi 只读 + 三 archi 写 + 两 DB 只读 + `propose_config`；五只读项 `ToolTier::Read`、三写项 `ToolTier::Write`，八个 archi 项均 `ToolExec::Archi`

### Requirement: 模型 = 源码（两条写入口与写纪律）

架构模型文件 SHALL 与源码同权：agent SHALL 直改 `<archi_dir>/graph/model/*.toml`。**写模型有两个入口，MUST NOT 存在第三个**：

| 入口 | 形态 | 失败语义 | 定位 |
|---|---|---|---|
| 专用写工具 `archi_element` / `archi_relation` | 结构化参数面（`desktop-archi-agent`「archi 写工具面」） | **坏文本不可产出**：渲染 → 重解析自证 → 回读一致，任一失败即 `Err` 且零落盘 | **canonical**：键形 / 值域 / 祖先 / 计数 / 契约告警全在工具里 |
| `write` / `edit` 手改 | 自由文本 | **写坏模型仍是合法状态**：由读取面报错（结构诊断 `file:line`、画布停摆、`archi_preclose` 兜底） | **second path**：工具表达不了的（批量整理 / 同层手排序）才用，改完必须自查 |

两条与文件序相关的边界 SHALL 与本表同批成立（本变更新增）：

- **子树连续与关系组连续由写面维持**：专用写工具的产出恒满足「父先于子 / 子树连续 / 同一 `from` 组连续」（见 `desktop-archi-model`「写入序不变量」）；手改拆开的子树 MUST NOT 被承诺长期保留——下一次工具写会把它合回原位（同层兄弟的手排相对序则被保留）。
- **手改的正当余量 = 批量整理与同层手排序**：MUST NOT 把手改的适用边界再表述为「重排文件」——整份文件的规范化已由每次工具写承担。

写权（谁能写）与写入口（怎么写）是两件事，MUST NOT 混说：

- **写权 SHALL 有且只有两条通路**，且**通路 A 按相位收窄**：

| 通路 | 触发面 | 执行根 | 确认时机 | 归属能力 |
|---|---|---|---|---|
| A（既有，按相位收窄） | change 流程相位会话——**仅 `phase ∈ {proposal, dev-design}` ∧ `role = executor`** | `detail.worktree` | 回合末归档确认面（模型区块 + 冲突分流） | `desktop-change-archive` |
| B（既有） | 架构页通用会话（`source = "archi"`），不限相位 | workspace root（主仓，`cwd = null` 回落） | 事后 `git diff` 评审 + 用户手动刷新校验 | `desktop-archi-chat` |

- **写权是强制面问题，不是工具面问题**：`archi` crate 全程只读盘（模型唯一落盘点是 `write_atomic` 单点），故「写入口」才分工具 / 手改两种；写权政策实做是**掐掉其余相位对模型目录与契约资产目录的射程**——由会话级写入禁区（第三规则来源）承载，见 `desktop-write-protection`「会话级写入禁区（相位锁）」。两写工具只在写权放开的相位下发（矩阵与写权锁同集，见「archi 工具面注入与 (phase, role) 矩阵」）；MUST NOT 出现「有工具但无写权」或「有写权但无工具」的错配。
- **契约资产（`models/**`）同属写权面（本变更新增）**：接口定义（`.schema.json`）与模块文档（`.md`）SHALL 与模型同相位开放——可写 ⟺ 架构页会话 ∨（`proposal` / `dev-design` 的 `executor`）；实现相位对该目录的手改由**会话级相位锁**拒绝（锁面表与拒串归 `desktop-write-protection`）。理由：`apiMissing` / `apiSignatureMismatch` 是唯二 `error` 档判据，「改契约迎合代码」必须从结构上不可达，否则 `error` 档名不副实。**相位锁不是写前校验**：它按相位掐通路、不判文本自洽——写权相位内用 `write` 写坏一份 `.schema.json` 仍由写闸拒绝（坏文本零落盘），写坏模型仍由读取面报错。
- **两条失败语义 MUST NOT 混为一句**：写工具面 MUST NOT 产出坏文本（自证链使它结构上不可达）；手改面 MUST NOT 因加工具而收紧——**写坏模型仍由读取面报错**（`archi_module` 的结构诊断带 `file:line`、画布停摆态、`archi_preclose` 读数）。MUST NOT 把「写盘前校验」当写权护栏（`write` / `edit` 对**接口定义文件之外**的一切路径零预校验是既有承诺）。
- **两条推论（政策是绝对命题，不是步骤枚举）**：
  - **`dev-design/evaluator` 被锁**：白名单按 `(phase, role)` 而非 phase——评审批次不写模型、不写契约。
  - **`explore` / `debug` 也被锁**：两者不是「步骤」，但政策覆盖一切来源；explore 的 `edit` 只服务笔记，锁模型零损失。
- **不依赖运行时审批**：写盘那一刻 MUST NOT 要求用户确认（工具分派链只有 `deny`，无挂起等确认能力）。通路 A 的设计权由「回合必须人工确认」承载；通路 B 的设计权由事后 diff 评审 + 手动刷新可见性承载（MUST NOT 承诺写盘前拦截）。
- **内置锁面仍不纳入**：`<archi_dir>/graph/model/`、契约资产 `models/**` 与架构域配置 MUST NOT 进入 `write_protection` 的**内置**锁面（内置规则是相位无关的绝对路径 glob，纳入即等于两条通路都改不了）；相位条件的锁 SHALL 走**会话级**规则面（见 `desktop-write-protection`「会话级写入禁区（相位锁）」）。
- **两通路并存，互不替代**：MUST NOT 因通路 B 存在而移除通路 A（change 侧 worktree 隔离 + 归档确认仍是「改动随 change 一起评审」的唯一形态）；亦 MUST NOT 因通路 A 收窄而收紧通路 B（架构页会话的用途恰是「不进 change 也能维护模型与契约」）。
- **场景归属**：冷启动生成骨架 / 增量维护（新增或重构模块）/ 按现状补声明 / 直接修复图或文档错误，四类场景 SHALL 可走任一条通路（走 change 时落 `dev-design` 阶段，走架构页时落通用会话），MUST NOT 强制单一入口。非白名单相位遇到「模型或契约该改」的需求 SHALL 把需求写进产物（design / tasks）交 `dev-design` 落地，MUST NOT 尝试绕过。
- **新元素缺省档**：两入口生成的新元素均缺省 `gate = "off"`（ratchet 逐块收紧）。
- **通路 B 的边界（软约束，非写入面强制）**：会话知识 SHALL 要求「禁止修改架构目录之外的文件（源码 / 配置 / 脚本 / 域外文档一律不改，用户要求亦不越界；生成物重生成除外）」，但写入面 MUST NOT 为该约束设硬隔离——该缺口 SHALL 在能力 spec 中如实留痕，MUST NOT 被表述为「会话只能改模型」。
- **生成物同权但有专用写工具（本变更改写）**：`.schema.json` **不是生成物**（它是手写真相，走 `write` / `edit` + 写闸），生成物只剩 `INDEX.toml` / `.i.rs` / `.i.ts` 三类。三类生成物 SHALL 有专用写工具 `archi_generate`（渲染 → 逐份保护 → 端口委托写盘单点，见「archi_generate 生成物工具」）——它是生成物的 **canonical** 入口；`write` / `edit` 手改生成物仍**零写前校验**、仍只有事后兜底（零 diff 门 + 契约面板），会话知识仍要求「MUST NOT 手改生成物」。两模型写工具的 `file` 只接受模型声明目录一级 `*.toml`，射程 MUST NOT 扩到生成物；`archi_generate` 的射程 MUST NOT 扩到模型或契约。
- **手写接口面有一处显式写前校验（本变更新增的唯一例外）**：`write` / `edit` 落 `<archi_models_dir>/**/<叶段>.schema.json` 时 SHALL 先过接口校验（坏文本 `Err` 零落盘，见 `desktop-agent-execution`「`write` / `edit` 的接口定义写闸」）。该例外 MUST NOT 被读成写权护栏或保护面：它只判「文件级自洽」，与谁能写无关；手改通路对**其余一切文件**（含 `INDEX.toml` / `.i.*` / 模型 TOML / 普通源码）仍零预校验。

#### Scenario: worktree 内直改模型成功（白名单相位）

- **WHEN** agent 在 `dev-design/executor` 会话中用 `write` 新建 `<archi_dir>/graph/model/03-billing.toml`，或用 `edit` 修改既有模型的 `gate` 行
- **THEN** 写入成功（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后读取面解析出该改动，主仓模型零变化

#### Scenario: 白名单外相位写模型被拒

- **WHEN** `implement/executor` / `code-review/evaluator` / `dev-design/evaluator` 会话对 `<archi_dir>/graph/model/*.toml` 调用 `write` 或 `edit`
- **THEN** 拒绝串含路径、工具名、指路（写进 design / tasks 交 `dev-design` 落地，或改用架构页会话）与「不要改用其他命令或工具尝试绕过」；run 不中断、文件零变化

#### Scenario: 白名单外相位写契约资产被拒（本变更新增）

- **WHEN** `implement/executor` 会话对 `<archi_dir>/models/<分层…>/<叶段>.md` 或同目录 `.schema.json` 调用 `write` / `edit`
- **THEN** 一律拒绝，拒串为**契约专用**那份（携路径与 `%t`、「只在 `proposal` / `dev-design` 执行侧与架构页会话写入」、两条出路：契约对就改代码追上契约 / 契约不可实现就停下述冲突由用户 backtrack、或改用架构页会话、「不要改用其他命令或工具尝试绕过」）；run 不中断、文件零变化；同一会话写 `graph/INDEX.toml` 与 `.i.*` 仍放行（锁面只到 `models/**`）

#### Scenario: 主仓直改模型成功（架构页）

- **WHEN** agent 在架构页通用会话（`cwd = null` → 主仓根）内用 `write` / `edit` 改 `<archi_dir>/graph/model/*.toml`（或同域的 `.schema.json` / `.md`）
- **THEN** 写入落在主仓该文件（会话级锁未在位、内置锁面零命中、无额外校验插口）；随后同 workspace 的 `archi_module` 与画布取数读到该改动，改动保持未提交由用户 `git diff` 评审

#### Scenario: 手写接口面是唯一预校验面

- **WHEN** 分别以 `write` 落一份坏 `.schema.json`、一份坏 `INDEX.toml`、一份坏模型 `*.toml`
- **THEN** 第一者 `Err` 零落盘（写闸）；后两者原样落盘、由读取面 / 零 diff 门报错（写前零校验不变）

#### Scenario: 生成物只三类且有专用写工具

- **WHEN** 审查写纪律文本与生成物清单
- **THEN** 只点名 `INDEX.toml` / `.i.rs` / `.i.ts` 三类；`.schema.json` 被表述为**手写真相**（走 `write` / `edit` + 写闸），MUST NOT 与三类并列；三类生成物的 canonical 入口为 `archi_generate`，且文中零「MUST NOT 存在专用写生成物工具」一类绝对禁令

#### Scenario: 写坏模型不阻断写盘（手改面）

- **WHEN** agent 用 `write` / `edit`（不是写工具）写出语法错的模型文件（白名单相位或通路 B）
- **THEN** 写盘成功；随后 `archi_module` / 画布数据返回结构诊断（`is_error` / 停摆态）且进程不 panic；收口时 `archi_preclose` 读数含该结构错

#### Scenario: 写工具面不可产出坏文本

- **WHEN** 以任何合法入参调用 `archi_element` / `archi_relation`，并构造一个会渲染出诊断的边界（如 `apis = [""]` / 非法 `path` 形态）
- **THEN** 一律 `Err` 且**零落盘**（自证链在交文本之前拦下），盘上字节零变化；该失败语义不得被表述为「与手改面相同」

#### Scenario: 两入口并存且无第三条通路

- **WHEN** 审查执行面工具清单与写模型入口
- **THEN** 写模型恰有两个入口（专用写工具 canonical / `write`+`edit` second path），工具面在写权相位下发两个模型写工具且在非写权相位零下发；MUST NOT 存在第三条通路（如独立 IPC 写命令或画布写入口，含任何 `archi_sort` 一类手动排序入口）；`archi_set_gate` 与写工具共用同一 `plan_element_upsert`（同实现 ⇒ 同文本）；`archi_generate` MUST NOT 被计为写模型入口（它只写三类生成物）

#### Scenario: 手改与写面对文件序的分工可辨

- **WHEN** 审查本 requirement 的手改出口表格行与两条「文件序边界」（骨架 md 侧只保留简写「工具表达不了的才手改」）；随后用 `edit` 把某元素的子表搬到其父之前，再调一次 `archi_element` 改该元素的 `summary`
- **THEN** 本条把边界表述为「批量整理 / 同层手排序」并写明子树连续与关系组连续由写面维持（「重排文件」不在其中）；子先于父不产生结构诊断（`orphanHierarchy` 判「父是否声明」，不判顺序）；`archi_element` 的响应 `reordered.moved ≥ 1`，块回到父的子树区间内

### Requirement: 架构会话的系统知识注入

`source = "archi"` 会话与 **change 臂的两写权相位** SHALL 注入同一段架构会话知识到 `SessionInjections.preamble`，内容 SHALL 覆盖以下**五段**（本变更是骨架的第三次改写：三资产分工与设计顺序取代语法类知识，散文只留不可脚本化的判断面；本轮只换「生成物重生成」一句的指代）：

- **三资产分工（C）**：模型 TOML 答「谁在谁的里面、谁依赖谁」（拓扑）；`<叶段>.schema.json`（与文档同目录）答「跨过这条边时接触面长什么样」（接口，**唯一凭据**）；`<叶段>.md`（同目录）答「为什么这样切、有什么坑、什么不归它管」（理由）。**过度设计 = 回答自己不该回答的那个问题**：文档不列接口、schema 不写理由、TOML 不造空元素。**文档是产品面**——架构页「文档」段直接渲染给用户读，人 + AI 共读；接口由写闸校验、人经 UI 阅读。
- **设计顺序（B/C）**：① 定位现状（`archi_search` 发现 → `archi_module` 一次取全邻居 / 三份资产 / 声明位置）；② 画拓扑（`archi_element` / `archi_relation`）——**先边界、后接触面、最后写理由**；③ 补两份资产（`archi_api` 的 `bootstrap` 取双草稿 → 接口用 `write` / `edit` 落 `.schema.json`（坏文本零落盘）→ 理由落 `.md`，段落按内容自定）。
- **三条不可越界（C）**：① **文档不列接口**——导出名 / 签名 / 类型的唯一持有者是 schema；历史 `api-*` fence 是遗留内容（读面不读、清理中），MUST NOT 新增；② **schema 不写理由**——`x-function.description` 只写调用方要知道的语义（做什么 / 边界 / 为什么），MUST NOT 写实现细节、MUST NOT 复述签名；`x-` 面封闭为 `x-function` / `x-class` / `x-common` / `x-constructor` / `x-static` **五项**（逐名列出，MUST NOT 写成「见 spec」），MUST NOT 自造新前缀键；③ **TOML 不造空元素**——每个元素至少一条接触面（`path` / 边 / 导出）。
- **工具面（A/B，指路）**：字段面、键形、缺省、值域、祖先与计数全部由工具承担——MUST NOT 凭记忆手拼 TOML。读五件各一句「何时用」（`archi_search` 发现 / `archi_module` 定位与三资产 / `archi_drift` 漂移读数 / `archi_api` 接口三动作 / `archi_scaffold` 字段词典）；写两件（`archi_element` / `archi_relation`，参数面即字段清单、坏文本零落盘）；**手改出口（永久内容）**：工具表达不了的才手改 TOML（骨架保持这一简写；**适用边界由本能力裁定** = 批量整理 / 同层手排序——子树连续与关系组连续由写面自动维持，手改拆开的子树会在下一次工具写时被合回，故「重排文件」不在边界内），改完必须 `archi_module` 自查——坏模型由读取面报错（结构诊断带 `file:line`）；**生成物过期调 `archi_generate` 重生成、MUST NOT 手改**（该句由「用既有命令重生成」换指：命令名归属各仓文档面，通用工作区里没有那条命令）。自检纪律：改完 `archi_drift` 非空即逐条修、`archi_api`（`check`）查两轴。契约资产 `config.json` 管 `ignore` / `exclude`，非法由 `archi_drift` 点名。
- **留给判断 + 兼容（C）**：`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——没有机械答案；**纯分组容器用 `composite`**；**接口面留白（无对外约定）就写一个空 schema（`$defs {}` + `properties {}`），不要编造导出**；旧元素可能仍把接口写在 `.md` 的 `api-*` fence 里——读面不读，接口一律写 `.schema.json`，MUST NOT 往 `.md` 新增 fence。

两条工程约束与下列「值域不进知识文本」纪律逐字沿用：

- `gate` / `api` / `kind` 的值域与缺省档 SHALL 由工具面承载（参数 `description` 与 `archi_scaffold` 词典），骨架 MUST NOT 再写值域行；键形与字段形态 MUST NOT 进骨架；结构诊断全量词表 MUST NOT 进知识文本（`orphanHierarchy` / `dependsOnAncestor` 亦不在骨架，祖先缺失由写工具 `Err`、越界依赖由 `archi_drift` 承载）。**文件序不进骨架**：子树连续与关系组连续是写面默认行为，骨架既不解释也不要求 agent 记得——骨架的手改出口句保持简写（「工具表达不了的才手改」），适用边界的裁定（批量整理 / 同层手排序）与「拆开会在下次写时合回」留在本节与工具描述，**不进骨架**。
- `entry` / `doc` / `apis` MUST NOT 进任何值域词表（开放集）；判据长表 MUST NOT 复制进知识文本。
- **协议词汇的枚举 MUST 逐名进骨架，判据面的事实 MUST NOT 进骨架**：`x-` 面五项是**协议词汇**（写 `.schema.json` 时要用到的名字），枚举本身 SHALL 在骨架内逐名列出；而「每个 `x-class` 至少一条构造通路」是跨元素判据面的事实（判定面在后续 change），MUST NOT 进骨架——两者的区别是「写的时候要用的词」vs「写完才判得出的事」。
- **体量锚（本次换句，MUST NOT 抬锚）**：骨架 SHALL ≤ **1831 B**、全文 SHALL ≤ **2031 B**，实测值 SHALL 被冻结断言（测量口径沿用 `lf_bytes(trim_end)`——LF 归一、去尾随换行）。本变更把「用既有命令重生成」换成指向 `archi_generate` 的一句，字节差 SHALL 由两处精确 `assert_eq!` 按**重测值**同批更新（门限 1831 / 2031 逐字不变，余量不得为负）；超限 SHALL 压缩同段冗余表述，MUST NOT 删除三资产分工 / 设计顺序 / 手改出口 / 留给判断四段任一，MUST NOT 借抬锚恢复语法教学（值域行 / 字段形态 / 诊断词表三类仍在禁令内）。
- **尽力增强、不炸 run**：知识拼装失败 SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 五段结构在骨架内可指认

- **WHEN** 审查骨架 md 的分段
- **THEN** 恰含五段语义：三资产分工（含「文档是产品面」）、设计顺序（三步 + 「先边界、后接触面、最后理由」）、三条不可越界（文档不列接口 / schema 不写理由 / TOML 不造空元素）、工具面（读五件的「何时用」+ 写两件的「参数面即字段清单」+ 手改出口一句 + 自检纪律）、留给判断 + 兼容；MUST NOT 出现语法教学段

#### Scenario: 语法知识零残留

- **WHEN** 对骨架做机械检查
- **THEN** 零示例 TOML 围栏（` ``` ` 计数为 0）、零 `[element.` 前缀、零字段值域行（`off` / `warn` / `strict` / `declared` / `none` 不同时作为值域枚举出现）、零结构诊断词（`orphanHierarchy` / `dependsOnAncestor` / `invalidValue` / `unknownField` / `missingField` / `syntaxError` 全零命中）、零 `entry` 三档长表

#### Scenario: 三条不可越界逐条在场

- **WHEN** 对骨架做关键词断言
- **THEN** 「文档不列接口」/「MUST NOT 复述签名」/「TOML 不造空元素」三条各至少一处；`x-` 面封闭**五项逐名**在场（`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`），且该行内 MUST NOT 出现「见 spec」一类指路；「空 schema」与「不要编造导出」在场；「历史 fence 读面不读、MUST NOT 新增」在场

#### Scenario: 五项枚举有机械守卫

- **WHEN** 从骨架的 `x-` 面枚举里删掉一项（如删 `x-constructor`）后跑知识语料测试
- **THEN** 测试红（`指令表收窄且新行集可判` 的 needle 集逐名核对两新键的带反引号原样文本）；恢复该项后绿——枚举本身不得只靠人眼守

#### Scenario: 手改出口是永久内容

- **WHEN** 审查骨架「工具面」段的最后一句
- **THEN** 该句含三要素：①「工具表达不了的才手改」的适用边界；②「改完必须自查」（点名 `archi_module`）；③ 坏模型由读取面报错（带 `file:line`）；MUST NOT 被表述为过渡措施或待删项；且句内不含「重排」一类已被写面接管的动作

#### Scenario: 值域迁到工具面而非丢弃

- **WHEN** 分别审查骨架与两工具的 `parameters` / `archi_scaffold` 草稿
- **THEN** 骨架零值域行；`gate` / `api` 的合法值与缺省档在 `archi_element` 的对应 `description` 内逐值在场（由 `ALL` 派生）且在草稿词典各有一行；`kind` 四值语义在枚举描述与草稿词典内逐值在场——「散文删掉的」都能在工具面查到

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 八 archi 工具不变），MUST NOT 抛装配错误；收口前自检照常装配（`before_done` 档位与知识解耦；T1 / T2 皆假时仍零 notice）

#### Scenario: 自检纪律与可见性在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完可调 `archi_drift` 取读数并按条修」「写工具坏了/手改写坏 → 结构诊断经 `archi_module` 带 `file:line`」「契约面可调 `archi_api` 查两轴」「生成物过期调 `archi_generate` 重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」六条；且知识文本内零「既有命令」死指代、零 `<>` 占位符

#### Scenario: 体量与目录方位实测

- **WHEN** 取 `knowledge()` 输出与骨架字节
- **THEN** 骨架 ≤ 1831 B、全文 ≤ 2031 B 且重测值被两处精确断言冻结（相对基线 1808 B / 1997 B 的差值恰为换句的字节差）；四数与三处文案（模块头门限表述 / 测试 doc 的抬锚口径 / 两条失败文案）同批更新；程序追加段含模型声明目录与契约资产目录（文档 + 接口两条同目录资产）两处方位，零契约索引串，骨架零目录字面量

#### Scenario: 判据面事实不进骨架

- **WHEN** 审查骨架全文
- **THEN** 零「构造通路」/「至少一条」一类跨元素判据表述（它属 `desktop-archi-api` 与后续 change 的判据面）；零「object 等同 JSON 对象」的复述（写闸已强制，骨架再加只是复述）；不得新增除换句外的第二条字节增量（门限未抬，增量仅来自该句替换）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### Function: `crates/core/archi/src/api/mod.rs` / `lib.rs`（端口）

| 项 | 变化 | 描述 |
|---|---|---|
| `ArtifactWriter` | 新增 | `pub trait ArtifactWriter: Send + Sync { fn write(&self, root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>; }`；纯类型，零 IO / 零写盘 / 零 spawn；复用既有 `GeneratedFile` 与 `ArchiApiWriteOutcome`（零新 DTO、零 specta 出线） |
| `lib.rs` | 改 | 出口随动（`pub use api::ArtifactWriter;`） |

### Function: `crates/infra/agent/src/sdk/**`

| 项 | 变化 | 描述 |
|---|---|---|
| `mount.rs` | 改 | 常量 `ARCHI_GENERATE_TOOL = "archi_generate"`；无参定义 + 五句描述（用法与三字段 / 零产出 / 孤儿 / 覆盖风险与 `git status` / 冲突处置）；静态清单 18 → **19** 项（`ToolTier::Write` / `ToolExec::Archi`，插在两 archi 写之后、两 DB 之前）；`Vec::with_capacity(18)` → `(19)` 与 `registry` / `build_registry` 两段 doc 的「十八项」措辞同批随动；`ToolTier` / `ToolExec` 的 doc 名单同步 |
| `archi_tool.rs` | 改 | `ArchiContext` 增 `writer: Option<Arc<dyn archi::ArtifactWriter>>` + `with_writer`（与 `with_protect` 同型、装配侧单点）；`execute` 第八臂（`workspace_render` → 逐份 `ProtectRules` 判定 → `writer.write` → `{ written, unchanged, conflicts }`）；端口缺席 ⇒ `is_error`；`archi_api` 的 `check` 分支加 `remedy` 三态（按本轮下发面判定）；输出过 `tools::cap_result`；零自拼渲染 / 零第二份写盘 |
| `runner.rs` | 改 | `SdkRunner` 字段 + builder 缝 `with_artifact_writer`；`PumpPayload` 字段；`mint_archi_context` 判据七名 → **八名**（含 `archi_generate`，任一在场即铸） |
| `lib.rs` | 改 | `EngineFacade` 字段 + builder + `runner_for` 透传（与 `with_ask_port` 同型） |
| `sdk/mod.rs` | 改 | 执行体模块头注释「五只读 + 两写」→「五只读 + 三写（两模型写 + 一生成物写）」 |
| `protect/reason.rs` | 改 | 新增 `CONTRACT_LOCK_TEMPLATE` + `contract_lock_template()` 出口（MUST NOT 复用 `SESSION_LOCK_TEMPLATE`）；四要素：受相位锁保护并携 `%s` / 契约只在起草相位执行侧与架构页会话写入 / 三条出路（改代码追上契约 → 停下述冲突由用户 backtrack → 改用架构页会话）/ 不要换工具绕过；既有三类串逐字不变 |
| `archi-knowledge.md` | 改 | 「生成物过期用既有命令重生成」→ 指向 `archi_generate`（单句换指） |
| `archi_knowledge.rs` | 改 | 模块头注释的门限表述随重测值更新（拼装面与降级语义不变） |

### Function: `crates/infra/agent/src/compose.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `compose_turn` | 改 | 增参数（`Arc<dyn archi::ArtifactWriter>`），透传到门面 builder；其余位装配语义不变 |
| `archi_matrix` | 改 | 两格加 `ARCHI_GENERATE_TOOL`（`proposal/executor`、`dev-design/executor`）；`("test-design", "executor")` 由 `[ARCHI_SEARCH_TOOL]` 改 `[ARCHI_SEARCH_TOOL, ARCHI_MODULE_TOOL]`；其余行逐字不变 |
| archi 臂名字清单 | 改 | 七 archi 名 → **八** archi 名（读五 + 模型写两 + 生成物写一）；`preamble` / `before_done` / `ask` 三位逐字不变 |
| `model_write_lock` | 改 | 政策不变（不挂锁格不变）；`Some` 分支由「恰一条」改「恰两条」——第二条 `glob = "**/{archi_models_rel_dir}/**"`、`reason = protect::contract_lock_template()`；成因注释整段改写（去掉「改代码相位必须能同步契约」的旧理由，如实写代价：实现相位发现契约错的唯一出路是 backtrack） |

### API: 工具响应（不进 DTO）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_generate` 响应 | 新增 | `{ written, unchanged, conflicts }` 在工具执行体内拼装；`ArchiApiWriteOutcome` DTO / IPC / bindings 零改动 |
| `archi_api(check)` 响应 | 改 | 顶层增 `remedy`（三态字符串或 `null`）；`ArchiApiCheck` DTO 键集与 bindings 零改动 |

### CLI / Component: 范围面

| 项 | 变化 | 描述 |
|---|---|---|
| `crates/core/workflow/src/prompts/implementation-generator.md` | 改 | 接口同步句反转：不改 schema / md、受相位锁、两条出路；零「同批同步接口定义」正句 |
| `.../prompts/{proposal-planner,dev-design-planner}.md` | 改 | 补 `archi_generate` 一行 + 零产出正常一句；`proposal-planner` 另补 md 责任句与 `contractPath` 取路径句；`dev-design-planner` 删「（实现相位同一条纪律）」 |
| `.../prompts/test-design-planner.md` | 改 | 补 `archi_module` 用法一句 + 「MUST NOT 用 grep 源码导出替代接口定义」边界句（4b 禁令逐字保留） |
| 九个 IPC 命令 | 零改动 | 清单与签名零变化：`archi_generate` 是 **agent 工具**，不是命令；`archi_api_generate` 仍走同一 `write_artifacts` 单点 |
| 画布 / 契约面板 | 面板文案一行 | `full` 模式提示文案改「生成需开 change，或让左侧会话生成」；生成按钮与模式权限断言逐字不变 |
| `.dev-team/architecture/**` | 零字节变化 | 模型拓扑与表头计数逐字不变；`api:export` / `api:check` 零 diff |
