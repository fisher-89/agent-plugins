## ADDED Requirements

### Requirement: Executor (sonnet) 执行测试并产出结构化报告
unit-test-executor 和 integration-test-executor SHALL 在其所属的独立阶段（06-unit-test 和 08-integration-test）中被调用，负责执行对应类型的测试并产出结构化报告文件。
Executor SHALL 使用 sonnet 模型。
Executor SHALL 执行测试命令（如 `npm test`、`pytest`、`go test` 等），捕获测试输出（stdout/stderr、退出码）。
Executor SHALL 从测试输出中提取：总用例数、通过数、失败数、跳过数、测试覆盖率百分比。
Executor SHALL 将提取的数据写入结构化报告文件（`reports/unit-test-execution.json` 或 `reports/integration-test-execution.json`）。
报告文件 SHALL 包含以下字段：
- `phase`: 阶段标识（06-unit-test 或 08-integration-test）
- `timestamp`: ISO 8601 时间戳
- `total`: 总用例数
- `passed`: 通过数
- `failed`: 失败数
- `skipped`: 跳过数
- `coverage`: 覆盖率百分比（字符串或 null）
- `coverage_threshold`: 最低覆盖率阈值（从 CLAUDE.md 读取）
- `failures`: 失败用例详情数组（测试名、错误信息、堆栈第一行、涉及文件路径）
- `duration_seconds`: 执行时长（秒）
- `test_command`: 实际执行的测试命令

#### Scenario: 单元测试 Executor 运行并产出报告
- **WHEN** unit-test-executor（sonnet）被 06-unit-test 阶段调用
- **THEN** 它执行项目相应的单元测试命令（由 CLAUDE.md 中的 `testCommand` 配置指定）
- **AND** 它从测试输出中提取用例统计和覆盖率指标
- **AND** 它将这些数据写入 `reports/unit-test-execution.json`
- **AND** 报告包含所有必需字段（phase、timestamp、total、passed、failed、skipped、coverage、failures、duration_seconds）

#### Scenario: 集成测试 Executor 运行并产出报告
- **WHEN** integration-test-executor（sonnet）被 08-integration-test 阶段调用
- **THEN** 它检查 `tests/` 目录下是否有标记为 integration 的测试文件
- **AND** 如果有，则执行对应的集成测试命令
- **AND** 它将结果写入 `reports/integration-test-execution.json`
- **AND** 报告格式与 unit-test 报告完全一致（复用同一 JSON schema）

#### Scenario: 测试命令执行失败（非零退出码）
- **WHEN** 测试命令以非零退出码退出且无测试输出
- **THEN** executor 将 `failed` 设为总用例数，`failures` 包含退出码和 stderr 内容
- **AND** 写入报告后退出（不重试）

### Requirement: Evaluator 读取报告并应用诊断决策树
unit-test-evaluator 和 integration-test-evaluator SHALL 读取对应 executor 产出的报告文件，验证报告完整性，并在存在失败用例时应用诊断决策树。
Evaluator SHALL 使用 opus 模型。
Evaluator SHALL 检查报告文件是否存在及是否包含所有必需字段。如果报告缺失关键字段（如 `total`、`failed`、`failures`），Evaluator SHALL 设置 verdict 为 fail，findings 包含 "report_incomplete"。
当报告显示存在失败用例时，Evaluator SHALL 应用诊断决策树按以下优先级依次判断：

1. 语法/import/类型错误：检查 `failures` 中每个失败的错误信息是否包含语法错误、模块导入失败、类型不匹配。如果错误行指向测试文件 -> 回溯 test-gen 阶段
2. 逻辑错误/返回值不符：检查失败信息中是否有 assertion error、expected vs actual 不匹配。如果错误行指向实现文件 -> 回溯 implement 阶段
3. 测试期望与 test-design.md 冲突：检查失败中的期望值是否与 test-design.md 中描述的预期行为冲突 -> 回溯 test-design 阶段
4. 接口签名不匹配：检查错误是否涉及函数调用参数不匹配，且测试和实现各自引用的模块边界契约不一致 -> 回溯 dev-design 阶段
5. 无法判断：如果上述规则均无法匹配 -> 调用 AskUserQuestion 工具向用户询问回溯目标。提问内容包含诊断摘要、已排除项列表和可选回溯目标（test-design/dev-design/test-gen/implement/unit-test/code-review/其他）。
Evaluator SHALL 在 eval.json 条目的 `findings` 字段中包含诊断推理过程。

#### Scenario: 语法错误判定为 test-gen 问题
- **WHEN** 报告中的 `failures` 包含 `SyntaxError` 或 `Module not found` 且错误堆栈第一行指向 `__tests__/` 或 `tests/` 目录下的文件
- **THEN** Evaluator 设置 `backtrack_to` 为 "04-test-gen"
- **AND** 在 `findings` 中记录 "测试文件存在语法/import 错误，回溯到 test-gen 阶段"

#### Scenario: 逻辑错误判定为 implement 问题
- **WHEN** 报告中的 `failures` 包含 `AssertionError` 或 `expected * received *` 且错误堆栈第一行指向 `src/` 目录下的实现文件
- **THEN** Evaluator 设置 `backtrack_to` 为 "05-implementation"
- **AND** 在 `findings` 中记录 "实现代码逻辑错误，回溯到 implement 阶段"

#### Scenario: 测试期望与 test-design 冲突
- **WHEN** 报告中的预期值与 test-design.md 中明确描述的预期行为矛盾（Evaluator 通过读取 test-design.md 确认）
- **THEN** Evaluator 设置 `backtrack_to` 为 "03-test-design"
- **AND** 输出 "[test-design.md] 描述: X, 测试期望: Y, 冲突"

#### Scenario: 接口签名不匹配
- **WHEN** 报告中的失败涉及函数调用参数数量或类型不匹配，且测试代码引用的模块边界契约版本与实现代码引用的版本不同
- **THEN** Evaluator 设置 `backtrack_to` 为 "02-dev-design"
- **AND** 在 `findings` 中记录接口名称和两个版本的不一致详情

#### Scenario: 报告不完整导致 Evaluator 拒绝
- **WHEN** 报告文件缺失 `total` 字段或 `failures` 字段
- **THEN** Evaluator 设置 verdict 为 "fail"，findings 包含 "report_incomplete: 缺少必填字段"
- **AND** backtrack_to 保持 null（不回溯，要求重新执行 executor）

#### Scenario: 无法判断失败根因调用 AskUserQuestion
- **WHEN** Evaluator 无法将失败归类到上述任一场景
- **THEN** Evaluator 调用 AskUserQuestion 工具
- **AND** 提问内容包含：失败的测试阶段（unit-test / integration-test）和测试名称、诊断摘要（已排除原因及排除依据）、可选回溯目标列表（test-design / dev-design / test-gen / implement / unit-test / code-review / 其他）
- **AND** Evaluator 等待用户响应

### Requirement: 测试执行覆盖率检查
unit-test-evaluator 和 integration-test-evaluator SHALL 从 executor 产出的报告中读取覆盖率数据。
如果覆盖率低于项目配置的最低阈值（由 CLAUDE.md 中的 `coverageThreshold` 定义，默认 80%），Evaluator SHALL 将该 checklist 项标记为 fail。
如果报告中的 `coverage` 为 null（项目没有配置覆盖率工具），Evaluator SHALL 将该 checklist 项标记为 pass 并在 evidence 中注明 "覆盖率检查未配置"。

#### Scenario: 覆盖率满足阈值
- **WHEN** 报告显示 line coverage 达到 85%，高于阈值 80%
- **THEN** checklist 中覆盖率条目标记为 pass，evidence 包含覆盖率百分比

#### Scenario: 覆盖率低于阈值
- **WHEN** 报告显示 line coverage 为 65%，低于阈值 80%
- **THEN** checklist 中覆盖率条目标记为 fail，evidence 包含覆盖率百分比和未覆盖的文件列表

#### Scenario: 覆盖率工具未配置
- **WHEN** 报告中的 `coverage` 为 null（项目未配置覆盖率工具）
- **THEN** checklist 中覆盖率条目标记为 pass，evidence 注明 "覆盖率检查未配置，跳过"

### Requirement: Executor 和 Evaluator 写入 eval.json
Executor 执行完毕后 SHALL 将执行概要追加到 eval.json 中，条目 SHALL 包含：phase、phase_suffix（"executor"）、timestamp、attempt、report（不超过 500 字符的执行摘要）、test_command、exit_code。
Evaluator 执行完毕后 SHALL 将评估结果追加到 eval.json 中，条目 SHALL 包含：phase、phase_suffix（"evaluator"）、timestamp、attempt、verdict、report（不超过 500 字符）、items（含 checklist 条目）、backtrack_to、findings（诊断推理文本）。
当测试全部通过时，Evaluator 的 verdict SHALL 为 "pass"，backtrack_to SHALL 为 null。
当存在失败测试时，Evaluator 的 verdict SHALL 为 "fail"，backtrack_to SHALL 根据诊断决策树设置。

#### Scenario: 全部通过写入 eval.json
- **WHEN** 所有测试用例通过且覆盖率满足阈值
- **THEN** Executor 写入执行概要条目到 eval.json，verdict 为 "pass"
- **AND** Evaluator 写入评估条目到 eval.json，verdict 为 "pass"，backtrack_to 为 null

#### Scenario: 存在失败用例写入 eval.json
- **WHEN** 存在失败的测试用例
- **THEN** Executor 写入执行概要条目到 eval.json（含失败计数）
- **AND** Evaluator 写入评估条目到 eval.json，verdict 为 "fail"，backtrack_to 根据诊断决策树设置
- **AND** items 数组中包含每个失败测试用例的具体信息和证据
