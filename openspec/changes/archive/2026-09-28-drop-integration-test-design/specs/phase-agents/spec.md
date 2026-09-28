## ADDED Requirements

### Requirement: test-design 单层 sociable 单元测试模型

`plugins/dev-team/agents/test-design-planner.md` SHALL 生成单层 sociable 单元测试设计，MUST NOT 再产出集成测试层：

- Step 8 SHALL 改名「测试框架识别」；`__MCP:test_detect_frameworks__` 调用本身保留，仅去掉「集成测试框架 / 集成测试文件」表述
- SHALL NOT 含「识别跨模块交互并生成集成测试章节」步骤（原 Step 9）与「集成测试关系标题命名指南」整节
- per-file 单元测试章节生成步骤（原 Step 11）SHALL 含组合用例挂靠指引：跨模块组合用例 colocate 到链路入口模块（链路的发起方 / 最上层调用方）的 `### <源文件> -> <测试文件>` 章节 `#### 用例` 表，describe 标题可写链路（如 `CLI参数 → workflow.json持久化`）；MUST NOT 设独立集成测试章节，MUST NOT 指引放置到 `__tests__/` 组合测试区
- 写入顺序（原 Step 13）SHALL NOT 含 `## 集成测试` 分段
- 验收范围映射规则 SHALL 与模板 3 列一致：`被测文件或模块` 列只填承载用例的测试文件（单值）

#### Scenario: planner 无集成层步骤

- **WHEN** 读取 `plugins/dev-team/agents/test-design-planner.md`
- **THEN** 不存在跨模块交互 → 集成测试章节的生成步骤，不存在「集成测试关系标题命名指南」节
- **AND** 框架识别步骤标题为「测试框架识别」
- **AND** 写入顺序的分段清单不含 `## 集成测试`

#### Scenario: 组合用例挂靠指引存在

- **WHEN** 读取 test-design-planner.md 的 per-file 单元测试章节生成步骤
- **THEN** 含「组合用例挂靠链路入口模块」与「describe 标题可写链路」的指引
- **AND** 无任何「集成测试文件放置在 `__tests__/`」类指引

#### Scenario: 被测入口模块约定

- **WHEN** planner 需要为一条跨模块链路选择承载用例的测试文件
- **THEN** 按链路发起方 / 最上层调用方确定入口模块，用例写入该模块的 per-file 章节

### Requirement: test-design evaluator checklist 去集成化并新增最小 mock 硬检查

`plugins/dev-team/agents/test-design-evaluator.md` SHALL：

- T2 仅保留单元测试方向：验收范围条目与 `## 单元测试` 的 `### <source> -> <test_file>` heading 双向无缺失；MUST NOT 再检查集成关系的 `**关联AC**` 双向映射
- T5 章节清单 SHALL 为：验收范围、单元测试、不可测试项（可选）；MUST NOT 列「集成测试（可选）」
- 删除 T8-I1..I8 整块（集成测试部分检查）；T8-G1 SHALL 改为与模板一致的 `AC ID | 验收条件 | 被测文件或模块` 3 列判据（消除「保留 5 列却只列 4 个列名」笔误）
- 新增最小 mock 硬检查项：`Mock策略` 表的 `Mock主体` 必须为进程边界依赖（数据文件 / 配置 / DB / 接口 / 网络 / 子进程 / 全局变量 / 运行环境）或「被测 API 显式入参 / 注入依赖传入的内部模块」；出现被测模块内部 wiring 的 mock（如 `vi.mock(仓库内模块路径)`）→ fail，判断依据指引改为组合真实模块

#### Scenario: checklist 无集成残留

- **WHEN** 读取 `plugins/dev-team/agents/test-design-evaluator.md`
- **THEN** 静态清单与 T8 详细项均不含集成测试条目（无 T8-I*）
- **AND** T2 / T5 的判断依据不含「集成测试方向」表述
- **AND** T8-G1 判据为 3 列且列名与模板验收范围表头一致

#### Scenario: 内部模块 mock 判 fail

- **WHEN** test-design.md 的 `Mock策略` 表声明 mock 被测模块内部 import 的协作模块（非显式入参传入）
- **THEN** 最小 mock 检查项判 fail
- **AND** evidence 指向该行并说明应组合真实模块

#### Scenario: 入参例外与进程边界 mock 判 pass

- **WHEN** `Mock主体` 为进程边界依赖，或为作为被测 API 显式入参 / 注入依赖传入的内部模块
- **THEN** 最小 mock 检查项 pass

### Requirement: test-gen generator/evaluator 去集成表格并收紧 mock 合法性对照

`plugins/dev-team/agents/test-gen-generator.md` 的 Step 2 SHALL 只解析 test-design.md 的 `单元测试 > 用例` 与 `单元测试 > Mock策略` 表格，MUST NOT 引用集成测试表格。

`plugins/dev-team/agents/test-gen-evaluator.md` SHALL：

- 删除 G4（`集成测试 > 用例` 骨架检查），其后条目重编号（原 G5..G9 → G4..G8）；组合用例由 G3 的 `单元测试 > 用例` 单表核对覆盖
- mock 检查项（原 G5）SHALL 收紧为：逐行对照 `单元测试 > Mock策略` 表，验证 `Mock主体` 有对应 mock 声明、`Mock方案` 与实现一致、`应用场景` 的 describe 中正确应用；另验证生成代码中的模块 mock 声明必须能对应到 Mock策略 表中的合法条目，无对应条目的内部模块 mock → fail
- Process 的逐表对照步骤 SHALL 只对照 `单元测试 > 用例` 与 `Mock策略` 表格，MUST NOT 再对集成测试表格做检查

#### Scenario: generator 只解析单表

- **WHEN** 读取 `plugins/dev-team/agents/test-gen-generator.md` 的 Step 2
- **THEN** 仅引用 `单元测试` 的 `用例` / `Mock策略` 表格
- **AND** 全步骤无 `集成测试` 字样

#### Scenario: evaluator 无集成骨架检查且 mock 对照收紧

- **WHEN** 读取 `plugins/dev-team/agents/test-gen-evaluator.md`
- **THEN** 静态清单不含集成测试用例骨架检查行
- **AND** mock 检查项含「模块 mock 声明必须对应 Mock策略 表合法条目，否则 fail」
- **AND** Process step 4 的逐表对照仅剩单元测试表格

## Module Contract

### Agent 源：test-design / test-gen 四文件（本变更后）

| 文件 | 变更 |
|------|------|
| `plugins/dev-team/agents/test-design-planner.md` | 删 Step 9 与命名指南；Step 8 改名「测试框架识别」；Step 11 增组合用例挂靠；Step 13 去集成段 |
| `plugins/dev-team/agents/test-design-evaluator.md` | T2/T5 去集成；删 T8-I1..I8；T8-G1 改 3 列；新增最小 mock 硬检查 |
| `plugins/dev-team/agents/test-gen-generator.md` | Step 2 只解析单元测试表格 |
| `plugins/dev-team/agents/test-gen-evaluator.md` | 删 G4 并重编号；mock 检查收紧为合法性对照；Process step 4 去集成对照 |

### 不变项

| 项 | 约定 |
|----|------|
| `__MCP:test_detect_frameworks__` | planner / generator 照常调用 |
| `__MCP:test_resolve_paths__` | colocated 推导零改动，天然覆盖组合用例 |
| evaluator 持久化 | 仍经 `phase_log` 写 `workflow.json`，不设 `backtrack_to`（见既有要求） |
