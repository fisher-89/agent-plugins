# desktop-phase-prompts Specification (Delta)

## MODIFIED Requirements

### Requirement: 桌面端特性适配（迁移是改写非搬运）

每个 md SHALL 适配桌面执行环境：只引用桌面七工具（read / grep / glob / ls / write / edit / bash）、**注入矩阵命中的 archi 只读工具**（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`，矩阵单源见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」）与 cwd=workspace root；MUST NOT 携带插件 Agent / MCP 工具假设（`__MCP:spec_list__` / `__MCP:phase_log__` / `__MCP:workflow_files__` / `__MCP:test_detect_frameworks__` 等）；SHALL 明确**静态脚本由桌面端执行、agent 内不重复执行**（static-check / test-execution 等机械步骤已外置，agent 只做知识性工作，省 token 为显式约束）。原文的「迁移」已完成且源侧文件已随插件退役删除——本条自此为**防回退护栏**：新增或改写 md MUST NOT 反向携带插件时代的工具 / 令牌假设。

**archi 工具引用纪律**：仅矩阵命中该工具的角色 md SHALL 提及它（`proposal-planner` → `archi_module` + `archi_search` + `archi_scaffold`；`dev-design-planner` → `archi_module` + `archi_search` + `archi_drift` + `archi_scaffold`；`dev-design-evaluator` → `archi_module` + `archi_search`；`test-design-planner` → 仅 `archi_search`；`implementation-generator` → `archi_module` + `archi_api` + `archi_drift`；`code-review-evaluator` / `acceptance-evaluator` → 仅 `archi_module`）；未命中角色（`initialize` 相关、`test-gen`、`test-execution`、`code-analyze`、`archive` SpecSync，以及 `proposal-evaluator` / `test-design-evaluator` / `implementation-evaluator` / `test-gen-evaluator`）MUST NOT 提及任何 archi 工具——否则等于在 prompt 里许诺一个未下发的工具面（模型会尝试调用并被成员校验拒绝）。archi 工具的提及 SHALL 只给「何时用（定位 / 发现 / 读漂移 / 取模板 / 查契约面）」与「返回什么（邻接与文档引用、`modelFile` + `modelLine` 可直接 `edit`、漂移计数与明细、字段模板）」两类信息：MUST NOT 插值具体路径模板、MUST NOT 教模型拼架构域目录字面路径（路径知识归工具返回值）。

**模型写面按相位分流（本变更新增）**：

- **写权相位（`proposal-planner` / `dev-design-planner`）** SHALL 指示写面与自查：可经既有 `write` / `edit` 维护架构模型（无专用写工具）；只为**已确定**的元素与关系落笔，设计未定的留到 `dev-design`；写完用 `archi_drift`（读漂移）与 `archi_module`（结构诊断）自查。该指示 MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 的返回值）。
- **其余 12 份 md**（`test-design-planner` / `implementation-generator` / `test-gen-generator` / `code-analyze-planner` / 8 份 evaluator）SHALL 各含一句「**架构模型已锁**」指路：MUST NOT 尝试写模型；需要改模型时把需求写进产物（`design.md` / `tasks.md` / 评审结论）由 `dev-design` 相位落地，或提示用户改用架构页会话。该句 MUST 为**中文措辞**且 MUST NOT 含 ASCII 子串 `archi`（未命中档案的「零 archi 字样」断言在案），MUST NOT 指示任何路由 / 落账工具调用（evaluator md 仍 MUST 保持零 `backtrack` 字样，见「evaluator 知识不含落账 / 路由指令」）。
- **两半必须对称**：写权指示与「已锁」指路 MUST 按同一张 `(phase, role)` 白名单落地（`proposal` / `dev-design` 的 executor 为写权侧，其余全部为锁定侧）——只写一半（例如 `implement` 既不说明已锁、也不指示写面）会让模型在拒绝串之外没有第二次机会知道边界。

#### Scenario: 只引桌面七工具与矩阵命中的 archi 工具

- **WHEN** 审查 14 个 md 的工具引用
- **THEN** 通用工具仅出现 read / grep / glob / ls / write / edit / bash 七项，无任何 `__MCP:*` 调用
- **AND** archi 工具只出现在矩阵命中该工具的角色 md 内（`proposal-planner` → `module` / `search` / `scaffold`；`dev-design-planner` → `module` / `search` / `drift` / `scaffold`；`dev-design-evaluator` → `module` / `search`；`test-design-planner` → 仅 `search`；`implementation-generator` → `module` / `api` / `drift`；`code-review-evaluator` / `acceptance-evaluator` → 仅 `module`），其余 md 零 archi 工具字样
- **AND** 无插件侧 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` / `__MODEL_*__` 片段注入标记

#### Scenario: archi 工具只给用法不给路径

- **WHEN** 读取命中角色的 md 内 archi 工具表述
- **THEN** 只含「何时用（定位 / 发现 / 读漂移 / 取模板 / 查契约面）」与「返回什么（邻接、文档引用、`modelFile` + `modelLine` 可直接 `edit`、漂移计数与明细、字段模板）」；不含具体路径模板、不含教模型拼架构域目录字面路径的措辞、不含「用某专用工具写模型」的指令

#### Scenario: 写权相位指示写面与自查

- **WHEN** 读取 `proposal-planner.md` 与 `dev-design-planner.md`
- **THEN** 两者均含「可经 `write` / `edit` 维护架构模型（无专用写工具）」与自查句（`archi_drift` / `archi_module`）；`proposal-planner` 另含「只为已确定元素落笔」一句；两者均零字段形态长表、零架构域路径模板

#### Scenario: 其余十二份写「已锁」且不越权

- **WHEN** 逐份读取其余 12 个 md
- **THEN** 各含一句「架构模型已锁」指路（不允许写模型 + 需求写进产物交由 `dev-design` 落地或改用架构页会话）；该句为中文措辞、零 ASCII `archi` 字样；8 份 evaluator md 仍零 `backtrack` / `phase_log` / `phase_next` 字样；未命中工具面的 md 零 archi 工具名

#### Scenario: 静态脚本外置声明

- **WHEN** 读取 executor / evaluator md 中对 static-check / test-execution 等机械步骤的表述
- **THEN** 含「由桌面端执行、agent 内不重复执行」类说明，agent 不承担重复运行测试执行门禁或静态检查脚本的职责
- **AND** acceptance 的构建 / 运行 / 测试动作以「验收手段」定位与上述门禁区分，不构成重复执行门禁

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/prompts/proposal-planner.md` | 写权相位 | 工具行补 `archi_module` / `archi_scaffold`；写面句（`write` / `edit` 维护模型、「只为已确定元素落笔」、写完自查） |
| `crates/core/workflow/src/prompts/dev-design-planner.md` | 写权相位 | 工具行补 `archi_drift` / `archi_scaffold`；写面自查句 |
| `crates/core/workflow/src/prompts/implementation-generator.md` | 锁定相位 + 已有 `archi_api` | 工具行补 `archi_drift`（改完代码自查未声明依赖）；补「架构模型已锁」一句 |
| `crates/core/workflow/src/prompts/*.md`（其余 11 份） | 锁定相位 | 各补「架构模型已锁」一句（中文、零 ASCII `archi`、零路由工具指示） |
| `crates/core/workflow/src/prompts/dev-design-evaluator.md` / `test-design-planner.md` / `code-review-evaluator.md` / `acceptance-evaluator.md` | 矩阵不变 | 工具行逐字不变；仅补「已锁」一句 |
| `crates/core/workflow/src/write/phase_table.rs` | 装配零改动 | 14 条 `include_str!` 字面路径与角色表逐字不变；工具面真源仍在 infra 挂载清单 |
