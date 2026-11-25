# desktop-phase-prompts Specification（delta: archi-agent-context-review）

## Purpose

本 delta 只改三份相位 prompt 的内容面（写权相位与实现相位），其余 11 份零改动。

- **写权相位**（`proposal-planner` / `dev-design-planner`）：工具清单补 `archi_api`（`bootstrap` 双草稿 / `link` 算 `$ref`）；新增**设计顺序**（先拓扑、后接触面、最后理由）与**留白写法**（空 schema `$defs {}` + `properties {}`，MUST NOT 编造导出）。
- **实现相位**（`implementation-generator`）：一句「改了导出面就同批同步接口定义 + `archi_api` 的 `check` 自查」。
- **纪律不变**：矩阵未命中者零 archi 工具字样；零路径模板、零字段形态长表、零占位符；装配仍全静态 `include_str!`。

## MODIFIED Requirements

### Requirement: 桌面端特性适配（迁移是改写非搬运）

每个 md SHALL 适配桌面执行环境：只引用桌面七工具（read / grep / glob / ls / write / edit / bash）、**注入矩阵命中的 archi 工具**（五只读：`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`；两写：`archi_element` / `archi_relation`——矩阵单源见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」）与 cwd=workspace root；MUST NOT 携带插件 Agent / MCP 工具假设；SHALL 明确**静态脚本由桌面端执行、agent 内不重复执行**。本条自此为**防回退护栏**：新增或改写 md MUST NOT 反向携带插件时代的工具 / 令牌假设。

**archi 工具引用纪律**：仅矩阵命中该工具的角色 md SHALL 提及它——

| 角色 md | 可提及的 archi 工具 |
|---|---|
| `proposal-planner` | `archi_module` + `archi_search` + **`archi_api`** + `archi_scaffold` + `archi_element` + `archi_relation` |
| `dev-design-planner` | `archi_module` + `archi_search` + **`archi_api`** + `archi_drift` + `archi_scaffold` + `archi_element` + `archi_relation` |
| `dev-design-evaluator` | `archi_module` + `archi_search` |
| `test-design-planner` | 仅 `archi_search` |
| `implementation-generator` | `archi_module` + `archi_api` + `archi_drift` |
| `code-review-evaluator` / `acceptance-evaluator` | 仅 `archi_module` |
| 未命中角色（initialize 相关 / `test-gen` / `test-execution` / `code-analyze` / `archive` SpecSync / `proposal-evaluator` / `test-design-evaluator` / `implementation-evaluator` / `test-gen-evaluator`） | 零 archi 字样 |

菜单外提及等于在 prompt 里许诺一个未下发的工具面（模型会尝试调用并被成员校验拒绝）。archi 工具的提及 SHALL 只给「何时用（定位 / 发现 / 读漂移 / 取模板 / 查接口面 / 写元素与边）」与「返回什么（邻接与三份资产、`modelFile` + `modelLine` 可直接 `edit`、漂移计数与明细、字段模板、写回的三通道响应与 `hash`）」两类信息：MUST NOT 插值具体路径模板、MUST NOT 教模型拼架构域目录字面路径（路径知识归工具返回值，接口文件路径经 `archi_module` 的 `assets.interface.path` 取）、MUST NOT 复述字段形态长表。

**模型写面按相位分流**：

- **写权相位（`proposal-planner` / `dev-design-planner`）** SHALL 指示三件事：
  1. **写面与自查**——架构模型经 `archi_element` / `archi_relation` 维护（参数面即字段清单、坏文本零落盘；工具表达不了的——重排文件 / 批量整理——才用 `write` / `edit` 手改）；只为**已确定**的元素与关系落笔，设计未定的留到 `dev-design`；写完用 `archi_drift`（`proposal` 无该工具时省）与 `archi_module`（结构诊断）自查。
  2. **设计顺序**——先画拓扑（边界与依赖），再定接触面（接口 `properties` / `$defs` 键），最后写理由（文档散文）；三者的落笔顺序 MUST NOT 倒置（先写理由再改接口会让文档反复重写）。
  3. **接口面纪律**——接口写在接口定义文件里（`archi_api` 的 `bootstrap` 取双草稿、`link` 算跨元素 `$ref`）；**接口面留白（无对外约定）就写一个空 schema（`$defs {}` + `properties {}`），MUST NOT 编造导出**；改完导出面 MUST 同批改接口定义（实现相位同一条）。
  该段 MUST NOT 复述字段形态长表，MUST NOT 把「读工具」「写工具」以外的 archi 工具名写进本相位。
- **其余 11 份 md** SHALL 各含一句「**架构模型已锁**」指路：MUST NOT 尝试写模型；需要改模型时把需求写进产物（`design.md` / `tasks.md` / 评审结论）由 `dev-design` 相位落地，或提示用户改用架构页会话。该句 MUST 为**中文措辞**且 MUST NOT 含 ASCII 子串 `archi`，MUST NOT 指示任何路由 / 落账工具调用。
- **`implementation-generator` 的接口同步句**：该 md SHALL 另含一句「改了导出面（新增 / 删除 / 改签名）就同批同步该元素的接口定义文件，改完用 `archi_api` 的 `check` 自查生成物新鲜度与判据」——接口漂移正是在实现相位产生。
- **两半必须对称**：写权指示与「已锁」指路 MUST 按同一张 `(phase, role)` 白名单落地（`proposal` / `dev-design` 的 executor 为写权侧，其余全部为锁定侧）。

#### Scenario: 只引桌面七工具与矩阵命中的 archi 工具

- **WHEN** 审查 14 个 md 的工具引用
- **THEN** 通用工具仅出现 read / grep / glob / ls / write / edit / bash 七项，无任何 `__MCP:*` 调用；archi 工具只出现在上表命中行内（`proposal-planner` 含 `archi_api`、`dev-design-planner` 含 `archi_api`，其余按表），其余 md 零 archi 工具字样（含两写工具与 `archi_api` 零命中）

#### Scenario: archi 工具只给用法不给路径

- **WHEN** 读取命中角色的 md 内 archi 工具表述
- **THEN** 只含「何时用」与「返回什么」两类；不含具体路径模板（`archi_module` 的 `assets.interface.path` 只作为取路径的指路出现）、不含教模型拼架构域目录字面路径的措辞、不含字段形态长表

#### Scenario: 写权相位含设计顺序与留白写法

- **WHEN** 读取 `proposal-planner.md` 与 `dev-design-planner.md`
- **THEN** 两者均含写工具句（点名 `archi_element` / `archi_relation` 为写面、「工具表达不了的才手改」）、设计顺序句（「先拓扑、后接触面、最后理由」同义表述）与接口面纪律句（`archi_api` 的 `bootstrap` / `link`；空 schema 形态 `$defs {}` + `properties {}`；MUST NOT 编造导出）；两者均零字段形态长表、零架构域路径模板

#### Scenario: implementation-generator 接口同步句在场

- **WHEN** 读取 `implementation-generator.md`
- **THEN** 含「改了导出面就同步接口定义 + 用 `archi_api` 的 `check` 自查」一句；其 archi 工具集合仍为 `archi_module` / `archi_api` / `archi_drift` 三名（不新增）

#### Scenario: 其余十一份写「已锁」且不越权

- **WHEN** 逐份读取其余 11 个 md
- **THEN** 各含一句「架构模型已锁」指路（中文措辞、零 ASCII `archi`）；8 份 evaluator md 仍零 `backtrack` / `phase_log` / `phase_next` 字样；未命中工具面的 md 零 archi 工具名（`archi_api` 同样零命中）

#### Scenario: 静态脚本外置声明

- **WHEN** 读取 executor / evaluator md 中对 static-check / test-execution 等机械步骤的表述
- **THEN** 含「由桌面端执行、agent 内不重复执行」类说明

#### Scenario: 零占位符与全静态

- **WHEN** 全文扫描 14 个 md
- **THEN** 无 `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:` / `__INCLUDE:` / `{{ }}` 等占位符；无「change 名已注入本 prompt」类插值假设

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/prompts/proposal-planner.md` | 写权相位角色知识 | 工具行补 `archi_api`（`bootstrap` 取双草稿 / `link` 算 `$ref`）；写面句追加「设计顺序」（先拓扑、后接触面、最后理由）与「接口面留白写空 schema、不编造导出」；零路径模板、零字段长表 |
| `crates/core/workflow/src/prompts/dev-design-planner.md` | 写权相位角色知识 | 同上；工具行补 `archi_api`（矩阵已补该格）；既有 `archi_drift` / `archi_scaffold` 行逐字保留 |
| `crates/core/workflow/src/prompts/implementation-generator.md` | 锁定相位角色知识 | 单加一句「改了导出面就同批同步接口定义 + `archi_api` 的 `check` 自查」；工具行三名逐字不变 |
| `crates/core/workflow/src/prompts/*.md`（其余 11 份） | 锁定相位角色知识 | **零改动**（工具行与「已锁」句逐字不变；`archi_api` 零新增提及） |
| `crates/core/workflow/src/write/phase_table.rs` | 静态装配单源 | 14 条 `include_str!` 字面路径与角色表逐字不变 |

### CLI: 相位 prompt 断言面

| 项 | 变化 | 描述 |
|---|---|---|
| 三份 md 的断言 | 新增 / 改写 | 工具清单按新表断言；设计顺序三句 + 空 schema 句 + 接口同步句在场；`proposal-planner` / `dev-design-planner` 零字段长表与零路径模板仍绿 |
| 其余 11 份 | 零改动 | 零 archi 工具字样 / 零 `backtrack` / 零占位符三条断言逐字不变 |
