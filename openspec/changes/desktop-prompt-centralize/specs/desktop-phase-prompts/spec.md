# desktop-phase-prompts Specification

## Purpose

桌面相位角色知识集中地：`core/workflow/src/prompts/` 下 14 个全静态 md（6 executor + 8 evaluator）作为 executor / evaluator 会话 prompt 主体的单一真源。每文件 = 角色自述 + 阶段要求 + checklist，全静态零占位；经 `phase_table.rs` 编译期 `include_str!` 显式字面路径装配（无 `<role>` 间接读取）；迁移是对插件 `plugins/dev-team/agents/*.md` 的改写而非搬运——按桌面端特性（七工具 / cwd=workspace root / change 名走上下文头 / 静态脚本外置）适配。

## ADDED Requirements

### Requirement: 14 个静态角色知识文件清单

`core/workflow/src/prompts/` SHALL 含 14 个 md 文件，文件名与角色一一对应：

| 类别 | 角色 | 文件 |
|------|------|------|
| executor | proposal-planner | `prompts/proposal-planner.md` |
| executor | dev-design-planner | `prompts/dev-design-planner.md` |
| executor | test-design-planner | `prompts/test-design-planner.md` |
| executor | implementation-generator | `prompts/implementation-generator.md` |
| executor | test-gen-generator | `prompts/test-gen-generator.md` |
| executor | test-execution-executor | `prompts/test-execution-executor.md` |
| evaluator | proposal-evaluator | `prompts/proposal-evaluator.md` |
| evaluator | dev-design-evaluator | `prompts/dev-design-evaluator.md` |
| evaluator | test-design-evaluator | `prompts/test-design-evaluator.md` |
| evaluator | implementation-evaluator | `prompts/implementation-evaluator.md` |
| evaluator | test-gen-evaluator | `prompts/test-gen-evaluator.md` |
| evaluator | test-execution-evaluator | `prompts/test-execution-evaluator.md` |
| evaluator | code-review-evaluator | `prompts/code-review-evaluator.md` |
| evaluator | acceptance-evaluator | `prompts/acceptance-evaluator.md` |

code-review / acceptance 为 evaluator-only 相位，SHALL NOT 存在对应 executor md（`executor: None` 不变）。每文件 SHALL 含角色自述、阶段要求（输入 / 过程 / 输出 / 约束的桌面化表述）；8 个 evaluator 文件另 SHALL 含静态 checklist 表（检查项 ID + 检查项 + 判断依据）。

#### Scenario: 清单齐全与 evaluator-only 无 executor

- **WHEN** 列出 `core/workflow/src/prompts/` 下全部 md
- **THEN** 恰好 14 个文件，角色清单与上表一一对应
- **AND** 无 `code-review-*.md` / `acceptance-*.md` 的 executor 文件（只有 evaluator 文件）
- **AND** 每个 evaluator 文件含静态 checklist 表，每个 executor 文件含阶段要求

### Requirement: 全静态零占位

每个 md SHALL 为全静态文本：MUST NOT 含 `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:*` / `__INCLUDE:*` / `__MODEL_*__` / `__DEV_TEAM_ROOT__` / `{{...}}` 等任何运行时占位符、令牌或片段注入标记。change 名经上下文头（见 desktop-change-orchestration「prompt 组装面静态化」）供给；phase / attempt 由 walker 按 provenance 盖戳，agent MUST NOT 回声。MUST NOT 出现「change 名在 prompt 里」类措辞（md 静态块引用 `openspec/changes/<name>/...` 产物路径时，`<name>` 以「目标 change 名（由上下文头给出）」语义表述，而非依赖运行时插值）。

#### Scenario: 零占位符扫描

- **WHEN** 全文扫描 14 个 md
- **THEN** 无 `<change>` / `<phase>` / `__CALL_AGENT` / `__MCP:` / `__INCLUDE:` / `{{ }}` 等占位符或令牌字样
- **AND** 无「change 名已注入本 prompt」类插值假设表述

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

每个 md SHALL 适配桌面执行环境：只引用桌面七工具（read / grep / glob / ls / write / edit / bash）与 cwd=workspace root；MUST NOT 携带插件 Agent / MCP 工具假设（`__MCP:spec_list__` / `__MCP:phase_log__` / `__MCP:workflow_files__` / `__MCP:test_detect_frameworks__` 等）；SHALL 明确**静态脚本由桌面端执行、agent 内不重复执行**（static-check / test-execution 等机械步骤已外置，agent 只做知识性工作，省 token 为显式约束）。

#### Scenario: 只引桌面七工具

- **WHEN** 审查 14 个 md 的工具引用
- **THEN** 仅出现 read / grep / glob / ls / write / edit / bash 七工具，无任何 `__MCP:*` 调用
- **AND** 无插件侧 `__DEV_TEAM_ROOT__` / `__INCLUDE:*` / `__MODEL_*__` 片段注入标记

#### Scenario: 静态脚本外置声明

- **WHEN** 读取 executor / evaluator md 中对 static-check / test-execution 等机械步骤的表述
- **THEN** 含「由桌面端执行、agent 内不重复执行」类说明，agent 不承担重复运行测试执行门禁或静态检查脚本的职责

### Requirement: evaluator 知识不含落账 / 路由指令

8 个 evaluator md MUST NOT 含 "Call phase_log" / `phase_next` / `phase_start` / `backtrack` 等 MCP 落账或路由指令（桌面 evaluator 禁 MCP，落账由 walker 代写）；evaluator SHALL 只产出评估结论（最终消息 checklist JSON，形状见 desktop-change-orchestration「evaluator 输出协议瘦身」），MUST NOT 指示用 Write/Edit/Bash 写 `eval.json` / `workflow.json`。

#### Scenario: evaluator md 零落账指令

- **WHEN** 读取 8 个 evaluator md
- **THEN** 无 phase_log / phase_next / phase_start / backtrack 调用指令
- **AND** 无「写入 eval.json / workflow.json」指令；输出面仅为评估结论（checklist JSON）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/prompts/*.md`（14 文件） | 相位角色知识单一真源（角色自述 + 阶段要求 + checklist） | 全静态零占位；桌面七工具 / cwd=workspace root / change 名走上下文头；静态脚本外置声明；evaluator 零落账指令 |
| `crates/core/workflow/src/write/phase_table.rs` | 相位表装配 | 每条目 `include_str!("prompts/<role>.md")` 显式字面路径；`PhaseAgentSpec { prompt: &'static str, model_level }`；无 `<role>` 动态读取 |
| 消费者：`crates/core/orchestration` | 组装动态面并起会话 | 只 append 动态面（上下文头 / 回溯原因 / 反馈注入 / 输出协议 / decision），不复制角色知识正文（见 desktop-change-orchestration） |
