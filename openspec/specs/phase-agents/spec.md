# phase-agents Specification

## Purpose
Phase agent file content, phase IDs, and report paths. Backtrack routing lives in `pipeline-backtrack` (decision C3=A: evaluators do not set `backtrack_to`; skills call `backtrack`). 本变更要求 evaluator 经 `phase_log` 把结果写入 `workflow.json.eval`，禁止把独立 `eval.json` 当作写入目标。

## Requirements

### Requirement: Evaluator / Executor 使用 consolidated phase ID

相关 agent SHALL 使用 `phase: "test-execution"`（经 `phase_log`）。已删除：`unit-test-*`、`integration-test-*` agent 文件。

| Agent | Phase |
|-------|-------|
| test-execution-evaluator.md | test-execution |
| test-execution-executor.md | test-execution（含 no-op skip 的 phase_log） |
| 其余 evaluator（proposal…acceptance、code-analyze） | 各自现行 ID，不变 |

#### Scenario: 旧 agent 不存在 / 新 agent 用 test-execution

- **WHEN** 检查 `integration-test-evaluator.md` / `unit-test-executor.md` 等
- **THEN** SHALL NOT 存在
- **WHEN** 读 `test-execution-evaluator.md` / `test-execution-executor.md`
- **THEN** `phase_log` 使用 `phase: "test-execution"`

### Requirement: test-execution agents 报告路径

`test-execution-executor.md` 与 `test-execution-evaluator.md` SHALL 读取聚合 `reports/test/summary.json`（有 change 时：`openspec/changes/<change-name>/reports/test/summary.json`）与经 `summary.plans[].path` 定位的 `<path>/report.json`。MUST NOT 要求 `reports/test-execution.json` 或 `reports/test-execution/<framework>.json` 作为权威路径。

**ID**: REQ-PA-TEF-1

#### Scenario: 路径更新

- **WHEN** 读上述两个 agent 文件
- **THEN** 引用 `reports/test/summary.json` + `plans[]`；不引用旧权威路径

### Requirement: Evaluator 不拥有 backtrack 路由

`test-execution-evaluator`（及其它 evaluator）SHALL 只做诊断并写入 report；MUST NOT 设置 `backtrack_to`。允许的回溯目标语义由 skill 层按 `pipeline-backtrack` 决定（常见目标：`test-gen`、`implement`、`test-design`、`dev-design`；不含 `integration-test`）。

#### Scenario: evaluator 不设 backtrack_to

- **WHEN** 读 `test-execution-evaluator.md` 及其它 evaluator 指令
- **THEN** SHALL NOT 要求设置 `backtrack_to`
- **AND** 回溯由 skill 按 `pipeline-backtrack` 决定

### Requirement: generator Process 结束前引用 static-analysis-gate include

源文件 `plugins/dev-team/agents/implementation-generator.md` 与 `plugins/dev-team/agents/test-gen-generator.md` SHALL 在各自 `## Process` 结束前（全部既有步骤之后）包含字面量：

```
__INCLUDE:static-analysis-gate__
```

该引用 SHALL 作为构建期片段注入点，而非在源文件中手写平台分支正文。Claude 组装后该处展开为空；Cursor / cursorHome 组装后展开为 `run_static_analysis` 软门禁（见 `include-fragments`）。

两 generator MUST NOT 在源正文中再次嵌入与 Claude `SubagentStop` hook 重复的硬编码静态检查步骤清单（避免 Claude 双重要求）；门禁差异仅通过 include 的平台档实现。

#### Scenario: implementation-generator 源含 include

**WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md`
**THEN** `## Process` 区域内 SHALL 包含 `__INCLUDE:static-analysis-gate__`
**AND** 该标记位于既有 Process 步骤之后

#### Scenario: test-gen-generator 源含 include

**WHEN** 读取 `plugins/dev-team/agents/test-gen-generator.md`
**THEN** `## Process` 区域内 SHALL 包含 `__INCLUDE:static-analysis-gate__`
**AND** 该标记位于既有 Process 步骤之后

#### Scenario: Claude 组装后 generator 无软门禁步骤

**WHEN** assemble 完成 `claude` 产物
**AND** 读取产物中对应 implementation-generator / test-gen-generator 文件
**THEN** 文件 SHALL NOT 因该 include 含有 `run_static_analysis` 结束前门禁段落

#### Scenario: Cursor 组装后 generator 含软门禁步骤

**WHEN** assemble 完成 `cursor` 或 `cursorHome` 产物
**AND** 读取产物中对应 implementation-generator / test-gen-generator 文件（含 `namePrefix` 重命名后的文件名）
**THEN** 文件 SHALL 含结束前执行 `run_static_analysis` 并在未通过时不得结束的说明

### Requirement: Evaluator persists via phase_log into workflow.json

所有 evaluator / 会调用 `phase_log` 的 executor（含 `test-execution-evaluator`、`test-execution-executor` 的 skip 路径，以及 proposal…acceptance、code-analyze evaluator）SHALL：

- 使用 MCP `phase_log` 追加评估结果
- 将持久化目标描述为 change 的 `workflow.json`（字段 `eval`）
- MUST NOT 指示使用 Write/Edit/Bash 创建或修改 `eval.json` 或 `workflow.json`

标题或步骤名若仍写「Append to eval.json」，SHALL 改为指向 `phase_log` / `workflow.json`。

#### Scenario: test-execution-evaluator 写入说明

- **WHEN** 读取 `plugins/dev-team/agents/test-execution-evaluator.md` 的 phase_log 步骤
- **THEN** 该步骤要求调用 `phase_log` 且提及 `workflow.json`（或等价「工作流元数据中的评估记录」）
- **AND** SHALL NOT 要求直接写文件 `eval.json`

#### Scenario: evaluator 仍不设 backtrack_to

- **WHEN** evaluator 判定 fail 并调用 `phase_log`
- **THEN** 条目无 `backtrack_to`（不变，见 `pipeline-backtrack`）

### Requirement: implementation-evaluator 范围核对改为三态对账

`plugins/dev-team/agents/implementation-evaluator.md` 的 I1 / I2 / I6 条目 SHALL 重写：范围核对的权威由 `git diff` 改为 design 变更清单 × `workflow.json` 的 `files` 清单 × 文件系统三态对账（判定表见 `workflow-file-inventory` 规格「消费方对账三态」）：

- 计划有、actual 无、文件不存在 → 硬 fail（未实现）
- 计划有、actual 无、文件存在 → 良性漏记，内容照常核对（I1 / I2 的内容核对独立于 actual）
- actual 有、计划无 → agent 判断；额外删除从严
- design 声明删、文件系统仍存在 → 硬 fail（未删）

提示词 MUST NOT 把 `git diff` 当作范围判定依据；MUST NOT 要求读取 `files` 字段缺失的旧 change（按硬报错指引重建）。

「actual 有、计划无」存在回退遗留时（backtrack 后 redo：`files` 含新 design 未声明的路径且磁盘内容偏离 HEAD），evaluator SHALL 判 fail 并在 report 的 failed items 中逐文件列出最短清理指令：HEAD 已有文件 → `git restore <path>`；旧方案新建文件 → 删除该文件。清理指令由 generator 在 redo 轮执行（PostToolUse 记录器随后按折叠语义对冲消除，见 `workflow-file-inventory`「backtrack 保持清单，遗留由 redo 轮对冲」）；evaluator 保持只读，MUST NOT 执行清理动作。

#### Scenario: I6 范围核对重写

- **WHEN** 读取 `implementation-evaluator.md` 的 I6 条目
- **THEN** 范围核对按三态对账表述，权威来源为 design 清单 + `files` + 文件系统
- **AND** SHALL NOT 要求以 `git diff` 判定变更范围

#### Scenario: 声明删除未删判 fail

- **WHEN** design 声明删除 `src/old.ts` 而文件系统仍存在该文件
- **THEN** evaluator 判定 fail（未删）
- **AND** 该判定不依赖 `files.deleted` 是否含该路径

#### Scenario: 回退遗留新建文件打回删除

- **WHEN** 回退后 `files.written` 含新 design 未声明的旧方案新建文件 `src/legacy.ts`（磁盘存在）
- **THEN** evaluator 判 fail，report 含「删除 `src/legacy.ts`」类清理指令
- **AND** evaluator 自身 MUST NOT 执行删除，由 generator 在 redo 轮执行

#### Scenario: 回退遗留修改打回还原

- **WHEN** 回退后 `files.written` 含新 design 未声明的 HEAD 已有文件 `src/a.ts`（磁盘内容偏离 HEAD）
- **THEN** evaluator 判 fail，report 含「`git restore src/a.ts`」类清理指令
- **AND** evaluator 自身 MUST NOT 执行还原

### Requirement: test-gen agents 的 artifact 语义改清单

`plugins/dev-team/agents/test-gen-generator.md` SHALL 移除 "git diff IS the artifact" 语义：测试生成的产出范围 SHALL 以 design / test-design 声明与 `files.written` 清单为准；`plugins/dev-team/agents/test-gen-evaluator.md` 的范围核对 SHALL 同样改为三态对账，MUST NOT 要求 evaluator 跑 `git diff` 圈定被检文件。

#### Scenario: generator 不再把 git diff 当产出契约

- **WHEN** 读取 `test-gen-generator.md`
- **THEN** 产出范围以清单与设计文档表述
- **AND** SHALL NOT 含 "git diff IS the artifact" 类文案

### Requirement: 观察型 evaluator 的 git diff 降级为纯观察辅助

`plugins/dev-team/agents/acceptance-evaluator.md` 与 `plugins/dev-team/agents/code-review-evaluator.md` SHALL 把 `git diff` 重新定位为**纯观察辅助**（查看修改内容与被删内容以评估质量），范围判定 SHALL 读 `files` 清单 + design 声明 + 文件系统。两个 evaluator SHALL 保留"禁止设置 backtrack_to"约束不变。

#### Scenario: acceptance 范围判定不依赖 git

- **WHEN** 读取 `acceptance-evaluator.md` 的范围核对步骤
- **THEN** AC 核对范围来自 proposal/design 声明与文件清单
- **AND** `git diff` 仅以观察辅助角色出现（可用、非权威）

### Requirement: implementation-generator 规范还原动作

`plugins/dev-team/agents/implementation-generator.md` SHALL 注明规范还原动作：撤销某文件的修改 SHALL 使用 `git restore <path>`（可被 PostToolUse 记录器识别为 revert 并折叠为净 untouched），MUST NOT 指示通过重写文件内容为原样来"还原"（该形态折叠为 written，需靠内容核对去噪）。

#### Scenario: generator 注明规范还原

- **WHEN** 读取 `implementation-generator.md`
- **THEN** 含"撤销文件修改用 `git restore <path>`"类指引
- **AND** 无"重写内容还原"的替代指引

### Requirement: evaluator 经查询 API 获取文件清单

`plugins/dev-team/agents/implementation-evaluator.md`、`plugins/dev-team/agents/code-review-evaluator.md` 与 `plugins/dev-team/agents/acceptance-evaluator.md` 的范围圈定步骤 SHALL 改为调用 `__MCP:workflow_files__`（输入目标 change 名）获取文件清单净状态，MUST NOT 指示直接 Read `workflow.json` 读取 `files` 清单。范围核对的三态对账判定语义不变——权威仍为 design 变更清单 × `files` 清单 × 文件系统（判定表见 `workflow-file-inventory` 规格「消费方对账三态」），本要求仅改变清单的获取通道。提示词 MUST NOT 依赖查询输出中不存在的字段（如 `source` 来源审计映射）。

#### Scenario: 提示词不再指示直接读

- **WHEN** 检查三个 evaluator md 的范围圈定步骤
- **THEN** 清单获取方式为调用 `__MCP:workflow_files__`
- **AND** 不存在「Read `workflow.json`」的 `files` 清单读取指令

#### Scenario: 对账语义不变

- **WHEN** evaluator 按新通道取得清单后执行范围核对
- **THEN** 判定仍按 design 清单 × `files` × 文件系统三态对账执行
- **AND** 判定结果与清单获取通道无关

## Module Contract

| Agent | Model | Phase | Input | Backtrack |
|-------|-------|-------|-------|-----------|
| test-execution-executor | sonnet-4.6 | test-execution | `reports/test/summary.json` + `plans[]` | n/a（执行器） |
| test-execution-evaluator | sonnet（系统定义） | test-execution | `reports/test/summary.json` | skill 决策；evaluator 不设 `backtrack_to` |

### Agent 源：`implementation-generator` / `test-gen-generator`

| 方面 | 描述 |
|------|------|
| **注入点** | `## Process` 末尾 `__INCLUDE:static-analysis-gate__` |
| **Claude 行为** | include → 空；硬门禁仍由 `SubagentStop` + `static-check` |
| **Cursor 行为** | include → 软门禁文案；无 `subagentStop` hook |
| **禁止** | 源文件手写双端重复静态检查长文；多 fragments 根 |

### Agent 持久化文案（evaluator / executor）

| 文件 | 变更 |
|------|------|
| `plugins/dev-team/agents/*-evaluator.md` | 持久化文案：`phase_log` → `workflow.json` |
| `plugins/dev-team/agents/test-execution-executor.md` | skip 时 `phase_log` 说明同步 |

### 位置: `plugins/dev-team/agents/*.md`

| 位置 | Contract |
|------|----------|
| `plugins/dev-team/agents/implementation-evaluator.md` | 范围圈定经 `__MCP:workflow_files__` 获取清单，不直接 Read `workflow.json` |
| `plugins/dev-team/agents/code-review-evaluator.md` | 同上 |
| `plugins/dev-team/agents/acceptance-evaluator.md` | 范围权威表述引用查询 API，不直接 Read `workflow.json` |
