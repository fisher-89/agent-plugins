# desktop-phase-prompts Specification（delta: archi-graph-doc-agent）

## MODIFIED Requirements

### Requirement: 桌面端特性适配（迁移是改写非搬运）

每个 md SHALL 适配桌面执行环境：只引用桌面七工具（read / grep / glob / ls / write / edit / bash）、**注入矩阵命中的 archi 只读工具**（`archi_module` / `archi_search`，矩阵单源见 `desktop-archi-agent`「SessionToolset::Archi 与 (phase, role) 注入矩阵」）与 cwd=workspace root；MUST NOT 携带插件 Agent / MCP 工具假设（`__MCP:spec_list__` / `__MCP:phase_log__` / `__MCP:workflow_files__` / `__MCP:test_detect_frameworks__` 等）；SHALL 明确**静态脚本由桌面端执行、agent 内不重复执行**（static-check / test-execution 等机械步骤已外置，agent 只做知识性工作，省 token 为显式约束）。原文的「迁移」已完成且源侧文件已随插件退役删除——本条自此为**防回退护栏**：新增或改写 md MUST NOT 反向携带插件时代的工具 / 令牌假设。

**archi 工具引用纪律**：仅矩阵命中该工具的角色 md SHALL 提及它（`proposal` 仅提及 `archi_search`；`dev-design` 两者；`test-design` 仅 `archi_search`；`implement` / `code-review` / `acceptance` 仅 `archi_module`）；未命中角色（`initialize` 相关、`test-gen`、`test-execution`、`code-analyze`、`archive` SpecSync）MUST NOT 提及任何 archi 工具——否则等于在 prompt 里许诺一个未下发的工具面（模型会尝试调用并被成员校验拒绝）。archi 工具的提及 SHALL 只给「何时用（定位 / 发现）」与「返回什么（邻接与文档引用、`modelFile` + `modelLine` 可直接 `edit`）」两类信息：MUST NOT 插值具体路径模板、MUST NOT 教模型拼 `openspec/architecture/...` 字面路径（路径知识归工具返回值）、MUST NOT 指示模型写模型文件（写面走既有 `write` / `edit` 与 `dev-design` 的常规纪律，模型 = 源码见 `desktop-archi-agent`）。

#### Scenario: 只引桌面七工具与矩阵命中的 archi 工具

- **WHEN** 审查 16 个 md 的工具引用
- **THEN** 通用工具仅出现 read / grep / glob / ls / write / edit / bash 七项，无任何 `__MCP:*` 调用
- **AND** archi 工具只出现在矩阵命中该工具的角色 md 内（`proposal` → 仅 `archi_search`；`dev-design` → 两者；`test-design` → 仅 `archi_search`；`implement` / `code-review` / `acceptance` → 仅 `archi_module`），其余 md 零 archi 工具字样
- **AND** 无插件侧 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` / `__MODEL_*__` 片段注入标记

#### Scenario: archi 工具只给用法不给路径与写指令

- **WHEN** 读取命中角色的 md 内 archi 工具表述
- **THEN** 只含「何时用（定位 / 发现）」与「返回什么（邻接、文档引用、`modelFile` + `modelLine` 可直接 `edit`）」；不含具体路径模板、不含教模型拼架构域目录字面路径的措辞、不含「用某专用工具写模型」的指令

#### Scenario: 静态脚本外置声明

- **WHEN** 读取 executor / evaluator md 中对 static-check / test-execution 等机械步骤的表述
- **THEN** 含「由桌面端执行、agent 内不重复执行」类说明，agent 不承担重复运行测试执行门禁或静态检查脚本的职责

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/prompts/*.md`（矩阵命中 7 份：proposal-planner / dev-design-planner / dev-design-evaluator / test-design-planner / implementation-generator / code-review-evaluator / acceptance-evaluator） | 角色知识补充 | 各补 archi 工具「何时用 / 返回什么」一句（按矩阵勾选，不越权提及未下发工具）；零占位符、零 MCP、零路径模板、零写模型指令 |
| `crates/core/workflow/src/prompts/*.md`（其余 9 份） | 零改动 | 矩阵未命中者 MUST NOT 出现任何 archi 工具字样（防"许诺未下发工具面"） |
| `crates/core/workflow/src/write/phase_table.rs` | 装配零改动 | 16 条 `include_str!` 字面路径与角色表逐字不变；工具面由 `SessionToolset` 注入承载，prompt 不含工具清单真源 |
