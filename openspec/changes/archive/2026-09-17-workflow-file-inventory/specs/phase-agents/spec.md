# phase-agents Specification（workflow-file-inventory 增量）

## ADDED Requirements

### Requirement: implementation-evaluator 范围核对改为三态对账

`plugins/dev-team/agents/implementation-evaluator.md` 的 I1 / I2 / I6 条目 SHALL 重写：范围核对的权威由 `git diff` 改为 design 变更清单 × `workflow.json` 的 `files` 清单 × 文件系统三态对账（判定表见 `workflow-file-inventory` 变更「消费方对账三态」）：

- 计划有、actual 无、文件不存在 → 硬 fail（未实现）
- 计划有、actual 无、文件存在 → 良性漏记，内容照常核对（I1 / I2 的内容核对独立于 actual）
- actual 有、计划无 → agent 判断；额外删除从严
- design 声明删、文件系统仍存在 → 硬 fail（未删）

提示词 MUST NOT 把 `git diff` 当作范围判定依据；MUST NOT 要求读取 `files` 字段缺失的旧 change（按硬报错指引重建）。

#### Scenario: I6 范围核对重写

- **WHEN** 读取 `implementation-evaluator.md` 的 I6 条目
- **THEN** 范围核对按三态对账表述，权威来源为 design 清单 + `files` + 文件系统
- **AND** SHALL NOT 要求以 `git diff` 判定变更范围

#### Scenario: 声明删除未删判 fail

- **WHEN** design 声明删除 `src/old.ts` 而文件系统仍存在该文件
- **THEN** evaluator 判定 fail（未删）
- **AND** 该判定不依赖 `files.deleted` 是否含该路径

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

## Module Contract

### Agent 源（增量）

| 文件 | 变更 |
|------|------|
| `plugins/dev-team/agents/implementation-evaluator.md` | I1 / I2 / I6 重写为三态对账（清单权威） |
| `plugins/dev-team/agents/test-gen-generator.md` | artifact 语义：git diff → 清单 |
| `plugins/dev-team/agents/test-gen-evaluator.md` | 范围核对改三态对账 |
| `plugins/dev-team/agents/acceptance-evaluator.md` | `git diff` 降级为观察辅助 |
| `plugins/dev-team/agents/code-review-evaluator.md` | 同上；backtrack 约束不变 |
| `plugins/dev-team/agents/implementation-generator.md` | **ADDED** 规范还原动作（`git restore <path>`） |
