# json-design-schemas Specification（workflow-file-inventory 增量）

## MODIFIED Requirements

### Requirement: Markdown templates for Planner artifacts

系统 SHALL 在 `templates/artifacts/` 提供建议模板（非强制结构）：

- `proposal.md.template` — Problem, Scope, Risks, Acceptance Criteria；「变更范围」SHALL 支持删除声明（实现文件 / 测试文件 / 删除文件 / 不要修改）
- `test-design.md.template` — Test Levels, Coverage Map, Test Strategy, Boundary Cases
- `design.md.template` — Architecture Components, Change Inventory, Data Model, Route/API, Dependencies, Open Questions；变更清单 SHALL 含「删除文件」子节（与「新增文件」「修改文件」并列，可省略注记保留），允许声明目录级删除

Evaluators 检查内容质量，不强制章节顺序。

计划侧删除声明与 actual 侧 `files.deleted` 同构：均以路径（含目录前缀）表达，消费方按前缀匹配对账（见 `workflow-file-inventory` 变更的对账三态）。

#### Scenario: proposal-planner 覆盖建议章节

- **WHEN** `proposal-planner` 写 proposal.md
- **THEN** 输出覆盖 Problem、Scope、Risks、Acceptance Criteria（可增删章节）

#### Scenario: design 变更清单可声明删除

- **WHEN** `dev-design-planner` 按 `design.md.template` 写变更清单且本 change 需删除 `src/old.ts`
- **THEN** 「删除文件」子节含 `src/old.ts`
- **AND** 无删除时该子节以省略注记表达，MUST NOT 强制为空表

#### Scenario: 目录级删除声明

- **WHEN** design 声明删除目录 `src/old-module/`
- **THEN** 声明以目录路径表达
- **AND** 与 actual 侧 `rm -rf` 记录的目录前缀同构可对账

## Module Contract

### Template: `plugins/dev-team/templates/artifacts/design.md.template`（增量）

| 章节 | 变更 |
|------|------|
| 变更清单 | **ADDED** 「删除文件」子节（目录路径合法，注明无删除时省略） |

### Template: `plugins/dev-team/templates/artifacts/proposal.md.template`（增量）

| 章节 | 变更 |
|------|------|
| 变更范围 | **ADDED** 「删除文件」分组（与实现文件 / 测试文件 / 不要修改并列） |
