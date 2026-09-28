## MODIFIED Requirements

### Requirement: Markdown templates for Planner artifacts

系统 SHALL 在 `templates/artifacts/` 提供建议模板（非强制结构）：

- `proposal.md.template` — Problem, Scope, Risks, Acceptance Criteria；「变更范围」SHALL 支持删除声明（实现文件 / 测试文件 / 删除文件 / 不要修改）
- `test-design.md.template` — 单层 sociable 单元测试设计：`验收范围` 表 3 列（`AC ID | 验收条件 | 被测文件或模块`；被测列单值填承载用例的测试文件）；`## 单元测试` per-file 章节（`#### 待测功能` / `#### 用例` / `#### Mock策略`）。`#### Mock策略` 注释 SHALL 写入最小 mock 原则：允许 mock 进程边界依赖（数据文件 / 配置 / DB / 接口 / 网络 / 子进程 / 全局变量 / 运行环境）与作为被测 API 显式入参 / 注入依赖传入的内部模块（入参例外），其余内部模块间调用必须真实组合；章节注释 SHALL 写入组合用例挂靠规则（跨模块组合用例 colocate 到链路入口模块的 `#### 用例` 表，describe 标题可写链路）。模板 MUST NOT 含「测试类型」列与 `## 集成测试` 整章
- `design.md.template` — Architecture Components, Change Inventory, Data Model, Route/API, Dependencies, Open Questions；变更清单 SHALL 含「删除文件」子节（与「新增文件」「修改文件」并列，可省略注记保留），允许声明目录级删除

Evaluators 检查内容质量，不强制章节顺序。

计划侧删除声明与 actual 侧 `files.deleted` 同构：均以路径（含目录前缀）表达，消费方按前缀匹配对账（见 `workflow-file-inventory` 规格的对账三态）。

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

#### Scenario: test-design 模板单层化

- **WHEN** 读取 `plugins/dev-team/templates/artifacts/test-design.md.template`
- **THEN** 验收范围表头为 `AC ID | 验收条件 | 被测文件或模块`，无「测试类型」列
- **AND** 不存在 `## 集成测试` 整章及其关系章节结构
- **AND** `#### Mock策略` 注释含进程边界白名单、入参例外与内部模块真实组合要求

#### Scenario: 模板写入挂靠规则

- **WHEN** planner 按模板为跨模块链路设计用例
- **THEN** 模板注释指引把组合用例写入链路入口模块的 per-file 章节 `#### 用例` 表
- **AND** 模板不含独立集成测试章节或 `__tests__/` 组合测试区的任何占位

## Module Contract

### `plugins/dev-team/templates/artifacts/test-design.md.template`

| 章节 | 变更 |
|------|------|
| `## 验收范围` | MODIFIED — 4 列改 3 列：`AC ID \| 验收条件 \| 被测文件或模块`（被测列单值） |
| `## 单元测试` | MODIFIED — 章节注释写入最小 mock 原则（含入参例外）与组合用例挂靠规则；per-file 章节结构（待测功能 / 用例 / Mock策略）不变 |
| `## 集成测试` | REMOVED — 整章及 `### 关系 → 测试文件` / 涉及模块 / 关联AC / 场景 / 用例 / Mock策略 子结构 |
| `## 不可测试项` | UNCHANGED |
