# desktop-phase-prompts Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 把 prompt 面的 archi 工具引用纪律与「契约作者 vs 契约实现者」的分界对齐：

- **两 planner 加 `archi_generate`**（生成物过期的可执行出路）+ `proposal-planner` 补 md 责任句（`contractPath` 取路径）。
- **`test-design-planner` 加 `archi_module`**（契约定死后要看得见接口键名）并保留 4b 的 grep 禁令。
- **`implementation-generator` 的接口同步句反转**：不再「改了导出面就同批同步 schema」，而是「不改契约、受相位锁、两条出路」。
- 写权段的「（实现相位同一条纪律）」删除——它是错纪律最直接的来源。

## MODIFIED Requirements

### Requirement: 桌面端特性适配（迁移是改写非搬运）

每个 md SHALL 适配桌面执行环境：只引用桌面七工具（read / grep / glob / ls / write / edit / bash）、**注入矩阵命中的 archi 工具**（五只读：`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`；两模型写：`archi_element` / `archi_relation`；一生成物写：`archi_generate`——矩阵单源见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」）与 cwd=workspace root；MUST NOT 携带插件 Agent / MCP 工具假设（`__MCP:spec_list__` / `__MCP:phase_log__` / `__MCP:workflow_files__` / `__MCP:test_detect_frameworks__` 等）；SHALL 明确**静态脚本由桌面端执行、agent 内不重复执行**（static-check / test-execution 等机械步骤已外置，agent 只做知识性工作，省 token 为显式约束）。原文的「迁移」已完成且源侧文件已随插件退役删除——本条自此为**防回退护栏**：新增或改写 md MUST NOT 反向携带插件时代的工具 / 令牌假设。

**archi 工具引用纪律**：仅矩阵命中该工具的角色 md SHALL 提及它——

| 角色 md | 可提及的 archi 工具 |
|---|---|
| `proposal-planner` | `archi_module` + `archi_search` + `archi_api` + `archi_scaffold` + `archi_element` + `archi_relation` + **`archi_generate`** |
| `dev-design-planner` | `archi_module` + `archi_search` + `archi_api` + `archi_drift` + `archi_scaffold` + `archi_element` + `archi_relation` + **`archi_generate`** |
| `dev-design-evaluator` | `archi_module` + `archi_search` |
| `test-design-planner` | **`archi_search` + `archi_module`** |
| `implementation-generator` | `archi_module` + `archi_api` + `archi_drift`（**零 `archi_generate`**：本相位不改契约，生成物恒新鲜） |
| `code-review-evaluator` / `acceptance-evaluator` | 仅 `archi_module`（`acceptance-evaluator` 见下） |

（`acceptance-evaluator` 的特殊口径：不点名任何 archi 工具——`archi_module` 仍由注入矩阵下发，prompt 侧不承诺）；未命中角色（`initialize` 相关、`test-gen`、`test-execution`、`code-analyze`、`archive` SpecSync、`acceptance-evaluator`，以及 `proposal-evaluator` / `test-design-evaluator` / `implementation-evaluator` / `test-gen-evaluator`）MUST NOT 提及任何 archi 工具——否则等于在 prompt 里许诺一个未下发的工具面（模型会尝试调用并被成员校验拒绝）。archi 工具的提及 SHALL 只给「何时用（定位 / 发现 / 读漂移 / 取模板 / 查接口面 / 写元素与边 / 生成生成物）」与「返回什么（邻接与三份资产、`modelFile` + `modelLine` 可直接 `edit`、漂移计数与明细、字段模板、写回的三通道响应与 `hash`、生成的三字段响应）」两类信息：MUST NOT 插值具体路径模板、MUST NOT 教模型拼架构域目录字面路径（路径知识归工具返回值，接口文件路径经 `archi_module` 的 `assets.interface.path` 取）、MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 的返回值与写工具的参数面）。

**模型写面按相位分流**：

- **写权相位（`proposal-planner` / `dev-design-planner`）** SHALL 指示四件事：
  1. **写面与自查**——架构模型经 `archi_element` / `archi_relation` 维护（参数面即字段清单、坏文本零落盘；工具表达不了的才用 `write` / `edit` 手改——**口径**：「表达不了」= 批量整理与同层手排序；子树连续与关系组连续由写面自动维持，故 MUST NOT 把整份文件排序列为手改理由，md 侧保留简写即可）；只为**已确定**的元素与关系落笔，设计未定的留到 `dev-design`；写完用 `archi_drift`（`proposal` 无该工具时省）与 `archi_module`（结构诊断）自查。
  2. **设计顺序**——先画拓扑（边界与依赖），再定接触面（接口 `properties` / `$defs` 键），最后写理由（文档散文）；三者的落笔顺序 MUST NOT 倒置（先写理由再改接口会让文档反复重写）。
  3. **接口面纪律**——接口写在接口定义文件里（`archi_api` 的 `bootstrap` 取双草稿、`link` 算跨元素 `$ref`）；**接口面留白（无对外约定）就写一个空 schema（`$defs {}` + `properties {}`），MUST NOT 编造导出**；改完导出面 MUST 同批改接口定义（**本相位是契约作者**；`implement` 相位不可改契约，见下条其余 md 的纪律）。
  4. **生成物与理由的落点（本变更新增）**——生成物过期时调 `archi_generate`（幂等、只增只改；**入口不在盘 / 接口面留白的新模块零产出是正常的**），MUST NOT 手改生成物；本 change 涉及元素的 `<叶段>.md`（理由）**由本相位写**，路径经 `archi_module` 的 `contractPath` 取（勿手拼目录），且 MUST NOT 在文档里列接口（导出名归 `.schema.json`）。

  该段 MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 的返回值与写工具参数面），MUST NOT 把「读工具」「写工具」「生成工具」以外的 archi 工具名写进本相位；MUST NOT 要求 agent 记得或执行任何文件排序动作（写入序归工具）。
- **其余 12 份 md**（`test-design-planner` / `implementation-generator` / `test-gen-generator` / `code-analyze-planner` / 8 份 evaluator）SHALL 各含模型锁定表述：或为一句「**架构模型已锁**」并携落地指路（MUST NOT 尝试写模型；需要改模型时把需求写进产物（`design.md` / `tasks.md` / 评审结论）由 `dev-design` 相位落地，或提示用户改用架构页会话），或为只读规则行「**MUST NOT** 修改源码、变更文档、配置、架构模型」（`code-review-evaluator` / `acceptance-evaluator` 重写后采用后者，不再单列已锁句）。该表述 MUST 为**中文措辞**且 MUST NOT 含 ASCII 子串 `archi`（未命中档案的「零 archi 字样」断言在案），MUST NOT 指示任何路由 / 落账工具调用（evaluator md 仍 MUST 保持零 `backtrack` 字样，见「evaluator 知识不含落账 / 路由指令」）。
- **`implementation-generator` 的契约纪律句（本变更反转）**：该 md SHALL 含三条——① **不改契约**：接口定义（`.schema.json`）与模块文档（`.md`）在本相位受相位锁保护，改了会被拒，且校验失败时 MUST 守住「坏文本零落盘」；② **生成物新鲜度自查**：改完代码用 `archi_api` 的 `check` 看两轴（本相位无 `archi_generate`，据 `remedy` 的指路转告用户）；③ **两条出路**：代码与契约不符时先确认哪一方对——契约对就**改代码追上它**；契约确实不可实现或写错，就停下来说明冲突，由用户决定走 `backtrack` 回 `dev-design` 修订（或改用架构页会话）。MUST NOT 再写「改了导出面就同批同步接口定义」一类正句（它把「改契约迎合代码」写成纪律）。
- **`test-design-planner` 的契约阅读纪律（本变更新增）**：该 md SHALL 补一句 `archi_module` 用法——取 `assets.interface` 的 `ops` / `types` 键名摘要，用于核对 `design.md` 的「公共函数 / API」是否与接口定义一致；并 SHALL 明写**接口定义是手写真相，MUST NOT 用 grep 源码导出的方式替代它**（既有「不得 grep 源文件导出声明或虚构条目」的禁令逐字保留，两者不冲突：前者读 schema、后者扫源码）。
- **两半必须对称**：写权指示与「已锁」指路 MUST 按同一张 `(phase, role)` 白名单落地（`proposal` / `dev-design` 的 executor 为写权侧，其余全部为锁定侧）——只写一半（例如 `implement` 既不说明已锁、也不指示写面）会让模型在拒绝串之外没有第二次机会知道边界。写权侧提及**写工具**，锁定侧 MUST NOT 提及任何 archi 工具名（含两模型写工具与 `archi_generate`；`implementation-generator` 是例外——它命中三只读工具，但 MUST NOT 提及 `archi_generate`）。

#### Scenario: 只引桌面七工具与矩阵命中的 archi 工具

- **WHEN** 审查 14 个 md 的工具引用
- **THEN** 通用工具仅出现 read / grep / glob / ls / write / edit / bash 七项，无任何 `__MCP:*` 调用
- **AND** archi 工具只出现在矩阵命中该工具的角色 md 内（`proposal-planner` → `module` / `search` / `api` / `scaffold` / `element` / `relation` / `generate`；`dev-design-planner` → `module` / `search` / `api` / `drift` / `scaffold` / `element` / `relation` / `generate`；`dev-design-evaluator` → `module` / `search`；`test-design-planner` → `search` / `module`；`implementation-generator` → `module` / `api` / `drift`；`code-review-evaluator` → 仅 `module`；`acceptance-evaluator` 零 archi 工具字样——工具仍下发但 prompt 侧不承诺），其余 md 零 archi 工具字样（含 `archi_generate` 与两写工具零命中）
- **AND** 无插件侧 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` / `__MODEL_*__` 片段注入标记

#### Scenario: archi 工具只给用法不给路径

- **WHEN** 读取命中角色的 md 内 archi 工具表述
- **THEN** 只含「何时用（定位 / 发现 / 读漂移 / 取模板 / 查契约面 / 写元素与边 / 生成生成物）」与「返回什么（邻接、文档引用、`modelFile` + `modelLine` 可直接 `edit`、漂移计数与明细、字段模板、写回三通道与 `hash`、生成三字段与零产出）」；不含具体路径模板、不含教模型拼架构域目录字面路径的措辞、不含字段形态长表（形态归 `archi_scaffold` 与写工具参数面）

#### Scenario: 写权相位含设计顺序与留白写法

- **WHEN** 读取 `proposal-planner.md` 与 `dev-design-planner.md`
- **THEN** 两者均含写工具句（点名 `archi_element` / `archi_relation` 为写面、「工具表达不了的才手改」）、设计顺序句（「先拓扑、后接触面、最后理由」同义表述）与接口面纪律句（`archi_api` 的 `bootstrap` / `link`；空 schema 形态 `$defs {}` + `properties {}`；MUST NOT 编造导出）；两者均含 `archi_generate` 句（含「零产出正常」语义）与「MUST NOT 手改生成物」；`proposal-planner` 另含 md 责任句（本 change 元素的 `.md` 由本相位写、路径经 `contractPath` 取）；两者均零字段形态长表、零架构域路径模板

#### Scenario: implementation-generator 契约纪律句反转

- **WHEN** 读取 `implementation-generator.md`
- **THEN** 含「不改契约（受相位锁）」「用 `archi_api` 的 `check` 自查生成物新鲜度」「两条出路（改代码追上契约 / 停下述冲突由用户 backtrack）」三要素；对「同批同步接口定义」「生成物过期用既有命令重生成」两类正句零命中；其 archi 工具集合仍为 `archi_module` / `archi_api` / `archi_drift` 三名（不新增 `archi_generate`）

#### Scenario: 写权相位指示写面与自查

- **WHEN** 读取 `proposal-planner.md` 与 `dev-design-planner.md`
- **THEN** 两者均含写工具句（点名 `archi_element` / `archi_relation` 为写面、「工具表达不了的才手改」的边界）与自查句（`archi_drift` / `archi_module`）；`proposal-planner` 另含「只为已确定元素落笔」一句；两者均零字段形态长表、零架构域路径模板；两者 MUST NOT 再出现「无专用写工具」一类已失效的绝对表述，亦 MUST NOT 出现「重排文件」一类把整份文件排序推给 agent 的表述；两者 MUST NOT 出现「（实现相位同一条）」一类把契约作者纪律复制到实现相位的括注

#### Scenario: test-design-planner 读接口定义而非 grep 源码

- **WHEN** 读取 `test-design-planner.md`
- **THEN** 含 `archi_module` 的用法一句（取 `assets.interface` 的 `ops` / `types` 键名核对 `design.md` 的公共面）与「接口定义是手写真相、MUST NOT 用 grep 源码导出替代它」的边界句；「不得 grep 源文件导出声明或虚构条目」的既有禁令逐字仍在场

#### Scenario: 静态脚本外置声明

- **WHEN** 读取 executor / evaluator md 中对 static-check / test-execution 等机械步骤的表述
- **THEN** 含「由桌面端执行、agent 内不重复执行」类说明，agent 不承担重复运行测试执行门禁或静态检查脚本的职责
- **AND** acceptance 的构建 / 运行 / 测试动作以「验收手段」定位与上述门禁区分，不构成重复执行门禁

#### Scenario: 其余十二份含模型锁定表述且不越权

- **WHEN** 逐份读取其余 12 个 md
- **THEN** 各含模型锁定表述（「架构模型已锁」+ 需求写进产物交由 `dev-design` 落地的指路，或只读规则行「MUST NOT 修改源码、变更文档、配置、架构模型」）；该表述为中文措辞、零 ASCII `archi` 字样；8 份 evaluator md 仍零 `backtrack` / `phase_log` / `phase_next` 字样；未命中工具面的 md 零 archi 工具名（`archi_element` / `archi_relation` / `archi_generate` 同样零命中）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### CLI: `crates/core/workflow/src/prompts/*.md`

| 文件 | 变化 | 描述 |
|---|---|---|
| `proposal-planner.md` | 改 | 补 `archi_generate` 句（含零产出正常）+ md 责任句（`contractPath` 取路径）；接口面纪律去掉「实现相位同一条」 |
| `dev-design-planner.md` | 改 | 补 `archi_generate` 句；删「（实现相位同一条纪律）」 |
| `test-design-planner.md` | 改 | 补 `archi_module` 用法 + 接口定义优先于 grep 的边界句（4b 禁令逐字保留） |
| `implementation-generator.md` | 改 | 契约纪律三要素（不改契约 / `check` 自查 / 两条出路）；零「同批同步接口定义」、零「既有命令」 |
| 其余 10 份 md | 零改动 | 矩阵未命中者 MUST NOT 出现 archi 工具字样（防许诺未下发工具面） |
