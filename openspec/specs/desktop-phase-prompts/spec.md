# desktop-phase-prompts Specification

## Purpose

桌面相位角色知识集中地：`core/workflow/src/prompts/` 下 14 个 md（6 executor + 8 evaluator，含 code-analyze 两份）作为 executor / evaluator 会话 prompt 主体的单一真源。每文件 = 角色定位（`## Role`）+ 阶段要求（输入 / 过程 / 输出 / 约束）+ evaluator 的 `## Check List`；文本静态，唯一运行时槽位为 `{{change}}`（由写面 `phase_next` 以记录 name 渲染）；经 `phase_table.rs` 编译期 `include_str!` 显式字面路径装配（无 `<role>` 间接读取）；文本为桌面端自持——按桌面端特性（七工具 / cwd=workspace root / change 名经模板槽位 / 静态脚本外置 / acceptance 可执行真实验收但不得影响生产数据）撰写与维护。

## Requirements

### Requirement: 14 个静态角色知识文件清单

`core/workflow/src/prompts/` SHALL 含 14 个 md 文件，文件名与角色一一对应：

| 类别 | 角色 | 文件 |
|------|------|------|
| executor | proposal-planner | `prompts/proposal-planner.md` |
| executor | dev-design-planner | `prompts/dev-design-planner.md` |
| executor | code-analyze-planner | `prompts/code-analyze-planner.md` |
| executor | test-design-planner | `prompts/test-design-planner.md` |
| executor | implementation-generator | `prompts/implementation-generator.md` |
| executor | test-gen-generator | `prompts/test-gen-generator.md` |
| evaluator | proposal-evaluator | `prompts/proposal-evaluator.md` |
| evaluator | dev-design-evaluator | `prompts/dev-design-evaluator.md` |
| evaluator | code-analyze-evaluator | `prompts/code-analyze-evaluator.md` |
| evaluator | test-design-evaluator | `prompts/test-design-evaluator.md` |
| evaluator | implementation-evaluator | `prompts/implementation-evaluator.md` |
| evaluator | test-gen-evaluator | `prompts/test-gen-evaluator.md` |
| evaluator | code-review-evaluator | `prompts/code-review-evaluator.md` |
| evaluator | acceptance-evaluator | `prompts/acceptance-evaluator.md` |

即 6 executor + 8 evaluator。`code-analyze` 为 test-only 工作流相位，executor / evaluator 双档俱在（`code-analyze-planner` / `code-analyze-evaluator`，装配见 `CODE_ANALYZE_STEPS` 相位表的 `include_str!` 字面路径）。code-review / acceptance 为 evaluator-only 相位，SHALL NOT 存在对应 executor md（`executor: None` 不变）。test-execution 为确定性相位，executor / evaluator 双档俱缺（`executor: None` / `evaluator: None`，与 archive 同形）——其「首败即交决策」语义由门禁步声明 `OnFail::Escalate` 承载（见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」与 desktop-change-orchestration「verdict fail 直接决策分叉」），MUST NOT 以角色 spec 存在性表达。每文件 SHALL 含角色定位（`## Role` 段：身份 / 职责自述）与阶段要求（输入 / 过程 / 输出 / 约束的桌面化表述；章节标题可按文件采用 Role / Task / Input / Output / Rules 等英文短标题，不要求逐文件同构）；8 个 evaluator 文件另 SHALL 含 `## Check List` 静态清单表（检查项 ID + 检查项 + 判断依据）。

**acceptance 与 code-review 的判据分工**：acceptance-evaluator SHALL 只按 `proposal.md` 的目标（验收标准）判断目标是否达成，并 SHALL 可自行构建 / 运行 / 执行测试做真实验收；其验收动作 MUST NOT 影响生产数据（数据库 / 应用启动类验证在临时副本或独立数据根进行）。代码级验收（范围蔓延、`不要修改` 清单、风险缓解措施可见性）SHALL 归 code-review-evaluator 的静态清单（C8 / C9 / C10），MUST NOT 在 acceptance 重复。

#### Scenario: 清单齐全与确定性相位无角色文件

- **WHEN** 列出 `core/workflow/src/prompts/` 下全部 md
- **THEN** 恰好 14 个文件，角色清单与上表一一对应（6 executor + 8 evaluator）
- **AND** 无 `code-review-*.md` / `acceptance-*.md` 的 executor 文件（只有 evaluator 文件）
- **AND** 无 `test-execution-*.md`（确定性相位零角色知识文件）
- **AND** 每个 evaluator 文件含 `## Check List` 表，每个 executor 文件含阶段要求

#### Scenario: acceptance 只判目标达成且代码级验收归 code-review

- **WHEN** 读取 acceptance-evaluator md 与 code-review-evaluator md 的静态清单
- **THEN** acceptance 清单只含「proposal 目标是否达成」与「验收动作未影响生产数据」两类判据，且正文允许自行构建 / 运行 / 测试做真实验收
- **AND** 范围蔓延 / `不要修改` 清单 / 风险缓解措施三类判据只出现在 code-review 清单（C8 / C9 / C10），acceptance 零复述

### Requirement: 模板槽位受控（仅 `{{change}}`）

每个 md SHALL 为静态文本：唯一允许的运行时模板槽位是 Tera 变量 `{{change}}`（由写面 `build_phase_response` 以记录 `name` 渲染，见 desktop-workflow-write-face「PhaseAgentSpec 静态化与通配符退役」）；MUST NOT 含 `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:*` / `__INCLUDE:*` / `__MODEL_*__` / `__DEV_TEAM_ROOT__` 或任何其它 `{{…}}` / `{%…%}` / `{#…#}` 模板标记。change 名只在 `{{change}}` 槽位出现（渲染后即产物路径与命令中的裸名）；phase / attempt 由 walker 按 provenance 盖戳，agent MUST NOT 回声。MUST NOT 出现「change 名由上下文行给出」类已退役措辞。

#### Scenario: 模板槽位扫描

- **WHEN** 全文扫描 14 个 md
- **THEN** 除 `{{change}}` 外无任何 `{{…}}` / `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:` / `__INCLUDE:` 等占位符或令牌字样
- **AND** 每份 md 至少一处 `{{change}}`，且以样例 change 名经 Tera 渲染后零占位残留

### Requirement: include_str! 显式字面路径装配

`core/workflow/src/write/phase_table.rs` 表条目 SHALL 以 `include_str!("prompts/<role>.md")` 显式字面路径装配 `PhaseAgentSpec.prompt`；MUST NOT 按 `<role>` 变量间接读取（无运行时文件 IO、无目录遍历、无动态字符串路径）。任何未被 `include_str!` 引用的 `prompts/*.md` SHALL 为可识别的废弃文件（无引用即死代码）。

#### Scenario: 显式路径无间接读取

- **WHEN** 审查 `phase_table.rs` 对 14 个 md 的装配方式
- **THEN** 每个 prompt 经 `include_str!("prompts/<role>.md")` 字面路径取得
- **AND** 无任何按 `<role>` / 变量拼接路径的动态读取调用

#### Scenario: 无引用即废弃

- **WHEN** 存在未被任何 `include_str!` 引用的 `prompts/*.md`
- **THEN** 该文件可被静态识别为废弃文件（不被装配进任何相位 prompt）

### Requirement: 桌面端特性适配（迁移是改写非搬运）

每个 md SHALL 适配桌面执行环境：只引用桌面七工具（read / grep / glob / ls / write / edit / bash）、**注入矩阵命中的 archi 工具**（五只读：`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`；两写：`archi_element` / `archi_relation`——矩阵单源见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」）与 cwd=workspace root；MUST NOT 携带插件 Agent / MCP 工具假设（`__MCP:spec_list__` / `__MCP:phase_log__` / `__MCP:workflow_files__` / `__MCP:test_detect_frameworks__` 等）；SHALL 明确**静态脚本由桌面端执行、agent 内不重复执行**（static-check / test-execution 等机械步骤已外置，agent 只做知识性工作，省 token 为显式约束）。原文的「迁移」已完成且源侧文件已随插件退役删除——本条自此为**防回退护栏**：新增或改写 md MUST NOT 反向携带插件时代的工具 / 令牌假设。

**archi 工具引用纪律**：仅矩阵命中该工具的角色 md SHALL 提及它（`proposal-planner` → `archi_module` + `archi_search` + `archi_scaffold` + `archi_element` + `archi_relation`；`dev-design-planner` → `archi_module` + `archi_search` + `archi_drift` + `archi_scaffold` + `archi_element` + `archi_relation`；`dev-design-evaluator` → `archi_module` + `archi_search`；`test-design-planner` → 仅 `archi_search`；`implementation-generator` → `archi_module` + `archi_api` + `archi_drift`；`code-review-evaluator` → 仅 `archi_module`；`acceptance-evaluator` 不点名任何 archi 工具——`archi_module` 仍由注入矩阵下发，prompt 侧不承诺）；未命中角色（`initialize` 相关、`test-gen`、`test-execution`、`code-analyze`、`archive` SpecSync、`acceptance-evaluator`，以及 `proposal-evaluator` / `test-design-evaluator` / `implementation-evaluator` / `test-gen-evaluator`）MUST NOT 提及任何 archi 工具——否则等于在 prompt 里许诺一个未下发的工具面（模型会尝试调用并被成员校验拒绝）。archi 工具的提及 SHALL 只给「何时用（定位 / 发现 / 读漂移 / 取模板 / 查契约面 / 写元素与边）」与「返回什么（邻接与文档引用、`modelFile` + `modelLine` 可直接 `edit`、漂移计数与明细、字段模板、写回的三通道响应与 `hash`）」两类信息：MUST NOT 插值具体路径模板、MUST NOT 教模型拼架构域目录字面路径（路径知识归工具返回值），MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 的返回值与写工具的参数面）。

**模型写面按相位分流**：

- **写权相位（`proposal-planner` / `dev-design-planner`）** SHALL 指示写面与自查：架构模型 SHALL 经 `archi_element` / `archi_relation` 维护（参数面即字段清单、坏文本零落盘；工具表达不了的——重排文件 / 批量整理——才用既有 `write` / `edit` 手改）；只为**已确定**的元素与关系落笔，设计未定的留到 `dev-design`；写完用 `archi_drift`（读漂移）与 `archi_module`（结构诊断）自查。该指示 MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 的返回值），MUST NOT 把「读工具」「写工具」以外的 archi 工具名写进本相位。
- **其余 12 份 md**（`test-design-planner` / `implementation-generator` / `test-gen-generator` / `code-analyze-planner` / 8 份 evaluator）SHALL 各含模型锁定表述：或为一句「**架构模型已锁**」并携落地指路（MUST NOT 尝试写模型；需要改模型时把需求写进产物（`design.md` / `tasks.md` / 评审结论）由 `dev-design` 相位落地，或提示用户改用架构页会话），或为只读规则行「**MUST NOT** 修改源码、变更文档、配置、架构模型」（`code-review-evaluator` / `acceptance-evaluator` 重写后采用后者，不再单列已锁句）。该表述 MUST 为**中文措辞**且 MUST NOT 含 ASCII 子串 `archi`（未命中档案的「零 archi 字样」断言在案），MUST NOT 指示任何路由 / 落账工具调用（evaluator md 仍 MUST 保持零 `backtrack` 字样，见「evaluator 知识不含落账 / 路由指令」）。
- **两半必须对称**：写权指示与「已锁」指路 MUST 按同一张 `(phase, role)` 白名单落地（`proposal` / `dev-design` 的 executor 为写权侧，其余全部为锁定侧）——只写一半（例如 `implement` 既不说明已锁、也不指示写面）会让模型在拒绝串之外没有第二次机会知道边界。写权侧提及**写工具**，锁定侧 MUST NOT 提及任何 archi 工具名（含两写工具）。

#### Scenario: 只引桌面七工具与矩阵命中的 archi 工具

- **WHEN** 审查 14 个 md 的工具引用
- **THEN** 通用工具仅出现 read / grep / glob / ls / write / edit / bash 七项，无任何 `__MCP:*` 调用
- **AND** archi 工具只出现在矩阵命中该工具的角色 md 内（`proposal-planner` → `module` / `search` / `scaffold` / `element` / `relation`；`dev-design-planner` → `module` / `search` / `drift` / `scaffold` / `element` / `relation`；`dev-design-evaluator` → `module` / `search`；`test-design-planner` → 仅 `search`；`implementation-generator` → `module` / `api` / `drift`；`code-review-evaluator` → 仅 `module`；`acceptance-evaluator` 零 archi 工具字样——工具仍下发但 prompt 侧不承诺），其余 md 零 archi 工具字样（含两写工具零命中）
- **AND** 无插件侧 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` / `__MODEL_*__` 片段注入标记

#### Scenario: archi 工具只给用法不给路径

- **WHEN** 读取命中角色的 md 内 archi 工具表述
- **THEN** 只含「何时用（定位 / 发现 / 读漂移 / 取模板 / 查契约面 / 写元素与边）」与「返回什么（邻接、文档引用、`modelFile` + `modelLine` 可直接 `edit`、漂移计数与明细、字段模板、写回三通道与 `hash`）」；不含具体路径模板、不含教模型拼架构域目录字面路径的措辞、不含字段形态长表（形态归 `archi_scaffold` 与写工具参数面）

#### Scenario: 写权相位指示写面与自查

- **WHEN** 读取 `proposal-planner.md` 与 `dev-design-planner.md`
- **THEN** 两者均含写工具句（点名 `archi_element` / `archi_relation` 为写面、「工具表达不了的才手改」的边界）与自查句（`archi_drift` / `archi_module`）；`proposal-planner` 另含「只为已确定元素落笔」一句；两者均零字段形态长表、零架构域路径模板；两者 MUST NOT 再出现「无专用写工具」一类已失效的绝对表述

#### Scenario: 静态脚本外置声明

- **WHEN** 读取 executor / evaluator md 中对 static-check / test-execution 等机械步骤的表述
- **THEN** 含「由桌面端执行、agent 内不重复执行」类说明，agent 不承担重复运行测试执行门禁或静态检查脚本的职责
- **AND** acceptance 的构建 / 运行 / 测试动作以「验收手段」定位与上述门禁区分，不构成重复执行门禁

#### Scenario: 其余十二份含模型锁定表述且不越权

- **WHEN** 逐份读取其余 12 个 md
- **THEN** 各含模型锁定表述（「架构模型已锁」+ 需求写进产物交由 `dev-design` 落地的指路，或只读规则行「MUST NOT 修改源码、变更文档、配置、架构模型」）；该表述为中文措辞、零 ASCII `archi` 字样；8 份 evaluator md 仍零 `backtrack` / `phase_log` / `phase_next` 字样；未命中工具面的 md 零 archi 工具名（`archi_element` / `archi_relation` 同样零命中）

### Requirement: evaluator 知识不含落账 / 路由指令

8 个 evaluator md MUST NOT 含 "Call phase_log" / `phase_next` / `phase_start` / `backtrack` 等落账或路由指令（落账由 walker 代写；这些名字属插件时代的 MCP 工具面，桌面端不存在）；evaluator SHALL 只产出评估结论（经 `finish_step` 工具收口，形状见 desktop-change-orchestration「evaluator 输出协议瘦身」），MUST NOT 指示用 Write/Edit/Bash 写 `eval.json` / `workflow.json`。

#### Scenario: evaluator md 零落账指令

- **WHEN** 读取 8 个 evaluator md
- **THEN** 无 phase_log / phase_next / phase_start / backtrack 调用指令
- **AND** 无「写入 eval.json / workflow.json」指令；输出面仅为经 `finish_step` 提交的评估结论

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/prompts/`（14 份 md） | 相位角色知识单一真源 | 6 executor + 8 evaluator；静态文本 + 唯一模板槽位 `{{change}}`；`code-analyze-planner` / `code-analyze-evaluator` 两份**登记入表**（此前漏登导致 spec 计数漂移）；test-execution 确定性相位零角色文件 |
| `crates/core/workflow/src/write/phase_table.rs` | 静态装配单源 | 14 条 `include_str!("../prompts/<role>.md")` 字面路径；`CODE_ANALYZE_EXECUTOR_PROMPT` / `CODE_ANALYZE_EVALUATOR_PROMPT` 与其余 12 条同权 |
| `crates/core/workflow/src/prompts/*.md`（矩阵命中 6 份：proposal-planner / dev-design-planner / dev-design-evaluator / test-design-planner / implementation-generator / code-review-evaluator） | 角色知识补充 | 各补 archi 工具「何时用 / 返回什么」一句（按矩阵勾选，不越权提及未下发工具）；仅 `{{change}}` 槽位、零 MCP、零路径模板、零写模型指令 |
| `crates/core/workflow/src/prompts/*.md`（其余 8 份） | 零 archi 字样 | 矩阵未命中者（含 `acceptance-evaluator`）MUST NOT 出现任何 archi 工具字样（防"许诺未下发工具面"） |
| `crates/core/workflow/src/write/phase_table.rs`（装配零改动） | 装配零改动 | 14 条 `include_str!` 字面路径与角色表逐字不变；工具面真源仍在 infra 挂载清单，prompt 不含工具清单真源 |

### 模块增量：架构规则与 agent prompt 同步（archi-rule-agent-prompt-sync）

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/prompts/proposal-planner.md` | 写权相位 | 工具行补 `archi_module` / `archi_scaffold`；写面句（`write` / `edit` 维护模型、「只为已确定元素落笔」、写完自查） |
| `crates/core/workflow/src/prompts/dev-design-planner.md` | 写权相位 | 工具行补 `archi_drift` / `archi_scaffold`；写面自查句 |
| `crates/core/workflow/src/prompts/implementation-generator.md` | 锁定相位 + 已有 `archi_api` | 工具行补 `archi_drift`（改完代码自查未声明依赖）；补「架构模型已锁」一句 |
| `crates/core/workflow/src/prompts/*.md`（其余 11 份） | 锁定相位 | 各补「架构模型已锁」一句（中文、零 ASCII `archi`、零路由工具指示） |
| `crates/core/workflow/src/prompts/dev-design-evaluator.md` / `test-design-planner.md` / `code-review-evaluator.md` / `acceptance-evaluator.md` | 矩阵不变 | 工具行逐字不变；仅补「已锁」一句 |
| `crates/core/workflow/src/write/phase_table.rs` | 装配零改动 | 14 条 `include_str!` 字面路径与角色表逐字不变；工具面真源仍在 infra 挂载清单 |
