# 提案: code-driven-test-phases

> **变更**: code-driven-test-phases
> **日期**: 2026-06-03
> **状态**: 提案中

---

## 问题

现有测试设计（test-design）和测试生成（test-gen）工作流阶段存在以下问题：

1. **test-design 和 test-gen 职责边界模糊**：test-design 在生成测试设计时容易过早引入具体参数类型信息，但此时代码可能尚未实现或更新，类型假设与最终实现存在矛盾风险。test-gen 阶段被禁止读取源码，无法从实际代码中获取参数签名，导致边界场景推导缺乏类型依据。两阶段需要明确的职责分工：test-design 聚焦业务场景，test-gen 处理技术类型细节。

2. **test-gen-generator 被禁止读取源码**：代理定义中包含文件类型黑名单（`.ts`, `.tsx`, `.js`, `.jsx`, `.py`, `.rs`, `.go`, `.java` 等），阻止 generator 读取源码文件来理解方法语义。生成的测试骨架只能基于设计文档而非实际代码。

3. **测试文件隔离存放**：测试骨架被写入 `openspec/changes/<name>/tests/` 目录，与实际源码分离。这不符合多数项目的测试文件管理习惯，增加了开发者理解测试与源码对应关系的心智负担。

4. **边界场景缺乏系统推导**：边界场景完全依赖人工编写，没有从参数类型系统地进行推导。常见边界值（int → 0/-1/MAX_INT、str → 空/超长/特殊字符 等）需要重复编写，容易遗漏。

---

## 提案

将 test-design 和 test-gen 阶段改造为**源码驱动**的工作流，明确两个阶段的职责边界。

### test-design（P→E）— 处理 WHAT to test

- **主要输入不变**：proposal.md + design.md（需求与架构设计 + 变动 API 签名）
- **新增补充输入**：通过 Grep 源码提取变更涉及的真实 API 签名（函数名、参数类型、返回类型）
- **输出新增内容**：
  - **正向 AC（Forward ACs）**：快乐路径业务场景
  - **反向 AC（Reverse ACs）**：异常路径业务场景（错误处理、无效输入、边界条件）
- **定位**：回答 WHAT to test — 定义业务场景层面的测试覆盖

### test-gen（G→E）— 处理 HOW to test

- **输入变更**：test-design.md + 直接读取源码提取参数签名并理解方法语义
- **移除约束**：删除文件类型黑名单，test-gen-generator SHALL 能够读取所有源码文件
- **输出变更**：测试文件与源码文件**共存**（colocated），如 `src/auth.py` → `src/test_auth.py`
- **内容新增**：
  - 快乐路径测试（从 Forward ACs 推导）
  - 异常路径测试（从 Reverse ACs 推导）
  - 边界场景测试（从变动 API 签名的参数类型和源码系统推导）
- **语言适配**：.py → pytest, .ts → jest/vitest, .rs → cargo test, .go → go test
- **无类型推断**：根据参数名推断类型（username → str, count → int, flags → boolean）
- **定位**：回答 HOW to test — 技术实现细节和边界场景补全

### 关键原则

> **test-design 处理 WHAT（业务场景），test-gen 处理 HOW（技术实现和边界补全）。**

### 参数类型 → 边界场景映射规范

test-gen-generator 在生成边界测试时使用以下系统映射：

| 类型 | 边界值 |
|------|--------|
| int/number | 0, -1, MAX_INT, None/undefined |
| str/string | "", 超长字符串, 特殊字符(\n \0 emoji), None |
| bool | True, False, None |
| list/array | [], [单元素], 超大列表, None |
| dict/object | {}, 缺失必填字段, 多余字段, None |
| Optional[T] | None |
| Enum | 每个枚举值, 非法枚举值 |
| float | 0.0, -0.0, NaN, Inf, None |

---

## 能力

### 修改的能力

- `phase-agents` — 更新 test-design-planner、test-gen-generator 代理定义及对应评估器检查清单。添加正向/反向 ACs 分类（test-design）、参数类型提取和源码读取（test-gen）、共存测试文件生成（test-gen）、系统化边界场景推导（test-gen）。test-design-planner 在 test-design 阶段不输出参数类型和风险标记。
- `phase-skills` — 确认 phase-test-design 和 phase-test-gen 技能编排无需修改，代理定义自身承载所有新增行为。

---

## 变更范围

### 实现以下特性

- test-design-planner 拆分单元测试和集成测试 ACs
- test-design.md.template：拆分单元测试和集成测试 ACs 章节
- test-design-evaluator 检查清单：更新 T2 从「tests/ 目录路径检查」为「coverage map 覆盖所有正向 AC 和反向 AC」
- test-gen-generator 代理定义：移除文件类型黑名单，添加源码读取、共存文件输出、参数类型 → 边界场景映射
- test-gen-evaluator 检查清单：更新 G1 从「tests/ 目录下文件」为「源码目录中对应测试文件」，更新 G2 从「tests/ 目录位置」为「与源码共存且命名规范」

### 不要修改

- phase-proposal、phase-dev-design、phase-implement 工作流阶段
- workflow-requirement 编排逻辑
- 集成测试阶段（phase-integration-test）以及集成测试执行器和评估器
- 单元测试阶段（phase-unit-test）以及单元测试执行器和评估器
- 代码审查阶段（phase-code-review）以及代码审查评估器
- 架构代理（architecture agent）及 C4 架构模型
- 其他非测试相关的代理定义和技能定义

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | test-design-planner 区分正向 AC 和反向 AC | 审查 test-design.md，确认快乐路径和异常路径分别列出并标识 | P0 |
| AC-2 | test-gen-generator 可以读取并理解所有源码文件类型 | 验证 generator 成功读取 .ts/.rs 文件的方法签名和逻辑，无黑名单限制 | P0 |
| AC-3 | 测试文件与源码文件共存于同一目录 | 验证生成的测试文件（如 src/test_auth.py）紧邻被测试源文件（src/auth.py） | P0 |
| AC-4 | 测试文件命名遵循语言约定 | 验证 .py → test_*.py, .ts → *.test.ts, .rs → *_test.rs, .go → *_test.go | P0 |
| AC-5 | test-gen-generator 从参数类型系统推导边界场景 | 审查生成的测试文件，确认包含 int→0/-1/MAX、str→空/超长/特殊字符、list→[]/超大、bool→True/False/None 等系统化边界测试 | P0 |
| AC-6 | test-gen-generator 为反向 AC 生成异常路径测试 | 审查生成的测试文件，确认包含错误处理、无效输入、None/undefined 等异常路径测试用例 | P0 |
| AC-7 | test-design-evaluator T2 检查 coverage map 覆盖所有 AC | 执行 test-design 阶段评估，确认 T2 检查项验证每个正向 AC 和反向 AC 在 coverage map 中有对应条目 | P1 |
| AC-8 | test-gen-evaluator G1/G2 检查测试文件共存位置 | 执行 test-gen 阶段评估，确认 G1 检查文件是否存在、G2 检查命名和位置是否符合规范 | P1 |
| AC-9 | test-gen-generator 对无类型文件根据参数名推断类型 | 审查 generator 对无类型文件的行为，确认参数名 → 类型推断（username→str, count→int, flags→boolean 等） | P2 |
| AC-10 | 现有 phase-test-design 和 phase-test-gen 技能无需修改即可支持新行为 | 验证技能编排无需改动，代理定义承载所有新增行为 | P1 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 测试文件写入源码目录可能被误提交到生产环境 | 源码目录混入测试文件 | 低 | 测试文件遵循明确命名规范（test_* / *.test.ts），便于 gitignore 过滤 |
| 移除黑名单后 test-gen 读取过多文件超出上下文窗口 | generator 上下文溢出或输出质量下降 | 中 | 限制读取范围为变更涉及的文件（参考 proposal in_scope 和 spec files）；通过代理定义中的约束控制范围 |
| 无类型语言参数推断不准确导致无效测试 | 生成的边界测试无法实际运行 | 中 | test-gen 对无类型参数生成的测试标记为 P2 优先级并添加 TODO 标记，提示开发者审阅后启用 |
| 共存测试文件与现有 CI/CD 配置冲突 | CI 流水线可能尝试执行新生成的骨架测试 | 低 | 生成的测试骨架包含 TODO 或 skip 标记，不会影响现有测试套件；明确告知用户需审阅后再启用 |
