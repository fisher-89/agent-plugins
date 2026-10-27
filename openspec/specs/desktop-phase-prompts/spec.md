# desktop-phase-prompts Specification

## Purpose

桌面相位角色知识集中地：`core/workflow/src/prompts/` 下 16 个全静态 md（7 executor + 9 evaluator，含 code-analyze 两份）作为 executor / evaluator 会话 prompt 主体的单一真源。每文件 = 角色自述 + 阶段要求 + checklist，全静态零占位；经 `phase_table.rs` 编译期 `include_str!` 显式字面路径装配（无 `<role>` 间接读取）；文本为桌面端自持——按桌面端特性（七工具 / cwd=workspace root / change 名走上下文头 / 静态脚本外置）撰写与维护。

## Requirements

### Requirement: 16 个静态角色知识文件清单

`core/workflow/src/prompts/` SHALL 含 16 个 md 文件，文件名与角色一一对应：

| 类别 | 角色 | 文件 |
|------|------|------|
| executor | proposal-planner | `prompts/proposal-planner.md` |
| executor | dev-design-planner | `prompts/dev-design-planner.md` |
| executor | code-analyze-planner | `prompts/code-analyze-planner.md` |
| executor | test-design-planner | `prompts/test-design-planner.md` |
| executor | implementation-generator | `prompts/implementation-generator.md` |
| executor | test-gen-generator | `prompts/test-gen-generator.md` |
| executor | test-execution-executor | `prompts/test-execution-executor.md` |
| evaluator | proposal-evaluator | `prompts/proposal-evaluator.md` |
| evaluator | dev-design-evaluator | `prompts/dev-design-evaluator.md` |
| evaluator | code-analyze-evaluator | `prompts/code-analyze-evaluator.md` |
| evaluator | test-design-evaluator | `prompts/test-design-evaluator.md` |
| evaluator | implementation-evaluator | `prompts/implementation-evaluator.md` |
| evaluator | test-gen-evaluator | `prompts/test-gen-evaluator.md` |
| evaluator | test-execution-evaluator | `prompts/test-execution-evaluator.md` |
| evaluator | code-review-evaluator | `prompts/code-review-evaluator.md` |
| evaluator | acceptance-evaluator | `prompts/acceptance-evaluator.md` |

即 7 executor + 9 evaluator。`code-analyze` 为 test-only 工作流相位，executor / evaluator 双档俱在（`code-analyze-planner` / `code-analyze-evaluator`，装配见 `CODE_ANALYZE_STEPS` 相位表的 `include_str!` 字面路径）。code-review / acceptance 为 evaluator-only 相位，SHALL NOT 存在对应 executor md（`executor: None` 不变）。每文件 SHALL 含角色自述、阶段要求（输入 / 过程 / 输出 / 约束的桌面化表述）；9 个 evaluator 文件另 SHALL 含静态 checklist 表（检查项 ID + 检查项 + 判断依据）。

#### Scenario: 清单齐全与 evaluator-only 无 executor

- **WHEN** 列出 `core/workflow/src/prompts/` 下全部 md
- **THEN** 恰好 16 个文件，角色清单与上表一一对应（7 executor + 9 evaluator）
- **AND** 无 `code-review-*.md` / `acceptance-*.md` 的 executor 文件（只有 evaluator 文件）
- **AND** 每个 evaluator 文件含静态 checklist 表，每个 executor 文件含阶段要求

### Requirement: 全静态零占位

每个 md SHALL 为全静态文本：MUST NOT 含 `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:*` / `__INCLUDE:*` / `__MODEL_*__` / `__DEV_TEAM_ROOT__` / `{{...}}` 等任何运行时占位符、令牌或片段注入标记。change 名经上下文头（见 desktop-change-orchestration「prompt 组装面静态化」）供给；phase / attempt 由 walker 按 provenance 盖戳，agent MUST NOT 回声。MUST NOT 出现「change 名在 prompt 里」类措辞（md 静态块引用 `openspec/changes/<name>/...` 产物路径时，`<name>` 以「目标 change 名（由上下文头给出）」语义表述，而非依赖运行时插值）。

#### Scenario: 零占位符扫描

- **WHEN** 全文扫描 16 个 md
- **THEN** 无 `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:` / `__INCLUDE:` / `{{ }}` 等占位符或令牌字样
- **AND** 无「change 名已注入本 prompt」类插值假设表述

### Requirement: include_str! 显式字面路径装配

`core/workflow/src/write/phase_table.rs` 表条目 SHALL 以 `include_str!("prompts/<role>.md")` 显式字面路径装配 `PhaseAgentSpec.prompt`；MUST NOT 按 `<role>` 变量间接读取（无运行时文件 IO、无目录遍历、无动态字符串路径）。任何未被 `include_str!` 引用的 `prompts/*.md` SHALL 为可识别的废弃文件（无引用即死代码）。

#### Scenario: 显式路径无间接读取

- **WHEN** 审查 `phase_table.rs` 对 16 个 md 的装配方式
- **THEN** 每个 prompt 经 `include_str!("prompts/<role>.md")` 字面路径取得
- **AND** 无任何按 `<role>` / 变量拼接路径的动态读取调用

#### Scenario: 无引用即废弃

- **WHEN** 存在未被任何 `include_str!` 引用的 `prompts/*.md`
- **THEN** 该文件可被静态识别为废弃文件（不被装配进任何相位 prompt）

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

### Requirement: evaluator 知识不含落账 / 路由指令

9 个 evaluator md MUST NOT 含 "Call phase_log" / `phase_next` / `phase_start` / `backtrack` 等 MCP 落账或路由指令（桌面 evaluator 禁 MCP，落账由 walker 代写）；evaluator SHALL 只产出评估结论（最终消息 checklist JSON，形状见 desktop-change-orchestration「evaluator 输出协议瘦身」），MUST NOT 指示用 Write/Edit/Bash 写 `eval.json` / `workflow.json`。

#### Scenario: evaluator md 零落账指令

- **WHEN** 读取 9 个 evaluator md
- **THEN** 无 phase_log / phase_next / phase_start / backtrack 调用指令
- **AND** 无「写入 eval.json / workflow.json」指令；输出面仅为评估结论（checklist JSON）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/prompts/`（16 份 md） | 相位角色知识单一真源 | 7 executor + 9 evaluator；全静态零占位；`code-analyze-planner` / `code-analyze-evaluator` 两份**登记入表**（此前漏登导致 spec 计数漂移） |
| `crates/core/workflow/src/write/phase_table.rs` | 静态装配单源 | 16 条 `include_str!("../prompts/<role>.md")` 字面路径；`CODE_ANALYZE_EXECUTOR_PROMPT` / `CODE_ANALYZE_EVALUATOR_PROMPT` 与其余 14 条同权 |
| `crates/core/workflow/src/prompts/*.md`（矩阵命中 7 份：proposal-planner / dev-design-planner / dev-design-evaluator / test-design-planner / implementation-generator / code-review-evaluator / acceptance-evaluator） | 角色知识补充 | 各补 archi 工具「何时用 / 返回什么」一句（按矩阵勾选，不越权提及未下发工具）；零占位符、零 MCP、零路径模板、零写模型指令 |
| `crates/core/workflow/src/prompts/*.md`（其余 9 份） | 零改动 | 矩阵未命中者 MUST NOT 出现任何 archi 工具字样（防"许诺未下发工具面"） |
| `crates/core/workflow/src/write/phase_table.rs`（装配零改动） | 装配零改动 | 16 条 `include_str!` 字面路径与角色表逐字不变；工具面由 `SessionToolset` 注入承载，prompt 不含工具清单真源 |
