# 测试设计: evaluatorchecklist

> **变更**: evaluatorchecklist
> **日期**: 2026-06-03
> **基于**: proposal.md, design.md, specs/phase-agents/spec.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 集成测试 | 验证 7 个评估器 Markdown prompt 文件（proposal-evaluator, test-design-evaluator, dev-design-evaluator, code-review-evaluator, test-gen-evaluator, implementation-evaluator, acceptance-evaluator）的静态检核表内容、判定规则表述和检查项范围引用是否符合预期；验证 2 个排除文件无变更 | Bash 脚本 (`test_*.sh`), bats (`test_*.bats`) | 100% 覆盖所有验收标准（AC-1 到 AC-6），验证 Markdown 表格结构正确性和判定规则语义正确性 |

**说明**: 本变更为纯 Markdown prompt 文件编辑，不涉及代码逻辑、运行时行为或 API 接口。因此不需要单元测试（无函数/模块可测试），也不需要端到端测试（无运行时流程）。集成测试验证的是文件内容属性而非运行时行为。

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1（所有评估器无 `必须` 列） | `openspec/changes/evaluatorchecklist/tests/test_checklist_column_removed.bats` | 集成 | 内容验证 — 逐文件 grep 检查 7 个评估器文件的 `## Static Checklist` 表格中无 `必须` 列 |
| AC-2（原可选项升级为强制项后不通过则 fail） | `openspec/changes/evaluatorchecklist/tests/test_optional_items_promoted.sh` | 集成 | 语义验证 — 确认 6 个升级项（R7, T5, T8, D7, D9, A5）在判定规则范围内且无特殊豁免逻辑 |
| AC-3（code-review-evaluator 范围 C1-C8） | `openspec/changes/evaluatorchecklist/tests/test_code_review_range.sh` | 集成 | 内容验证 — 读取判定规则行确认 `(C1-C8)` |
| AC-4（acceptance-evaluator 范围 A1-A5） | `openspec/changes/evaluatorchecklist/tests/test_acceptance_range.sh` | 集成 | 内容验证 — 读取判定规则行确认 `(A1-A5)` |
| AC-5（判定规则统一为 "ALL items must pass"） | `openspec/changes/evaluatorchecklist/tests/test_verdict_rule_unified.sh` | 集成 | 内容验证 — grep 确认 7 个文件中无 "ALL required items" 模式 |
| AC-6（unit-test-evaluator 和 integration-test-evaluator 无变更） | `openspec/changes/evaluatorchecklist/tests/test_excluded_files_unchanged.bats` | 集成 | 回归验证 — `git diff` 确认两个排除文件无变更 |
| 表格格式验证（列数正确：ID、检查项、判断依据） | `openspec/changes/evaluatorchecklist/tests/test_checklist_column_removed.bats` | 集成 | 内容验证 — 确认每个静态检核表 Markdown 表格仅有 3 列且列头匹配预期 |

---

## 3. 测试策略

### 3.1 方法

本变更的测试策略为**内容验证（content verification）**，使用 Bash 脚本和 bats 测试框架对 7 个 Markdown prompt 文件执行静态度量检查和模式匹配验证。由于变更不涉及可执行代码，测试方法聚焦于：

1. **模式匹配（Pattern Matching）**：使用 `grep` 搜索检核表表格结构中是否还残留 `必须` 列
2. **文本断言（Text Assertion）**：使用 `grep` 或 `sed` 精确匹配判定规则行的预期内容
3. **文件差异比对（Diff Verification）**：使用 `git diff` 验证排除文件未受影响
4. **表格解析（Table Parsing）**：使用 `awk` 或 `sed` 解析 Markdown 表格的列数和列头内容

测试脚本统一放置在 `openspec/changes/evaluatorchecklist/tests/` 目录下，所有脚本应在变更实施完成后运行。测试不涉及外部服务、数据库或网络请求。

### 3.2 测试分类

- **集成测试（内容验证）**: 所有 6 个 AC 的测试均归为集成测试层级。测试针对文件内容执行断言，验证编辑操作的准确性。

#### 测试分类说明

- **集成测试（内容验证）**: 不涉及运行时环境。测试脚本直接读取目标 Markdown 文件，通过模式匹配验证检核表结构、列数、判定规则文本和检查项范围引用的正确性。使用 `grep`、`awk`、`sed` 等命令行工具进行文本分析。

> **注意**: 本变更不涉及单元测试（无可运行代码模块）和端到端测试（无运行时流程）。不依赖真实外部环境。

### 3.3 模拟策略

本变更不涉及模拟（Mock）。所有测试直接读取文件系统中的 Markdown prompt 文件，不调用 API、不启动服务、不依赖外部资源。测试的"通过/失败"完全由文件内容的文本属性决定。

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 升级项 R7 在判定规则中被遗漏 | `proposal-evaluator.md` 判定规则行仍写为 `"pass" only if ALL required items pass` 且未包含 R7 | 测试检测到残留的 "required" 表述或 R7 不在判定范围内，报告 fail | `test_optional_items_promoted.sh` |
| 升级项 T5/T8 在判定规则中被遗漏 | `test-design-evaluator.md` 判定规则范围只覆盖 T1-T4/T6-T7 而遗漏 T5/T8 | 测试检测到 T5/T8 未出现在判定规则引用中，报告 fail | `test_optional_items_promoted.sh` |
| 升级项 D7/D9 在判定规则中被遗漏 | `dev-design-evaluator.md` 判定规则范围未涵盖 D7/D9 | 测试检测到 D7/D9 判定规则中无引用，报告 fail | `test_optional_items_promoted.sh` |
| 升级项 A5 在判定规则中被遗漏或仍被排除 | `acceptance-evaluator.md` 判定规则仍写为 `(A1-A4, A7)` 或范围不包含 A5 | 测试检测到旧的范围引用或 A5 不在范围内，报告 fail | `test_acceptance_range.sh` |
| C6-C8 在判定规则中仍被忽略 | `code-review-evaluator.md` 判定规则仍写为 `(C1-C5)` | 测试检测到旧的范围引用 `(C1-C5)`，报告 fail | `test_code_review_range.sh` |
| 某文件中 `必须` 列已从表格中删除但判定规则行仍残留 "required" | `test-gen-evaluator.md` 表格已修改但 `## Process` 段仍写有 `ALL required items` | 测试 grep 检查到 "ALL required items" 模式，报告 fail | `test_verdict_rule_unified.sh` |
| 某文件中 `必须` 列被部分删除（表头已删但某行数据列残留分隔符） | `implementation-evaluator.md` 表格表头仅 3 列但某行仍有 4 个单元格 | 测试解析 Markdown 表格列数时发现某行列数不一致，报告 fail | `test_checklist_column_removed.bats` |
| 3 列表格渲染后列头文本错误（如「判断依据」写为「证据」） | 某评估器表格第三列列头不是「判断依据」 | 测试检测到列头文本与预期不符，报告 fail | `test_checklist_column_removed.bats` |
| units-test-evaluator 或 integration-test-evaluator 被意外修改 | `git diff` 显示这两个文件有变更 | 测试检测到这两个文件有 diff，报告 fail | `test_excluded_files_unchanged.bats` |
| 多个文件同时存在残留 `必须` 列 | 7 个文件中有 1 个或多个仍包含 `必须` 列 | 测试逐文件检查后汇总所有失败文件，输出详细报告 | `test_checklist_column_removed.bats` |

---

## 5. 测试数据

本变更的测试数据即为被修改的 7 个评估器 Markdown prompt 文件本身：

| 文件 | 路径 | 备注 |
|------|------|------|
| proposal-evaluator | `plugins/dev-team/agents/proposal-evaluator.md` | R7 升级，判定规则去除 "required" |
| test-design-evaluator | `plugins/dev-team/agents/test-design-evaluator.md` | T5/T8 升级，判定规则去除 "required" |
| dev-design-evaluator | `plugins/dev-team/agents/dev-design-evaluator.md` | D7/D9 升级，判定规则去除 "required" |
| code-review-evaluator | `plugins/dev-team/agents/code-review-evaluator.md` | 范围修正 C1-C8，判定规则去除 "required" |
| test-gen-evaluator | `plugins/dev-team/agents/test-gen-evaluator.md` | 判定规则去除 "required" |
| implementation-evaluator | `plugins/dev-team/agents/implementation-evaluator.md` | 判定规则去除 "required" |
| acceptance-evaluator | `plugins/dev-team/agents/acceptance-evaluator.md` | A5 升级，范围修正 A1-A5，判定规则去除 "required" |
| unit-test-evaluator | `plugins/dev-team/agents/unit-test-evaluator.md` | 验证无变更 |
| integration-test-evaluator | `plugins/dev-team/agents/integration-test-evaluator.md` | 验证无变更 |

测试脚本无需构造额外测试数据。所有断言直接基于上述文件的文本内容。

---

## 6. 不可测试项

- **评估器在 AI 运行时中的实际判定行为** — 原因：评估器的判定由 Claude AI 代理在运行时执行，其行为受到 prompt 内容、模型版本、上下文窗口等多种因素影响。本次测试仅限于验证 prompt 文件内容本身是否正确，无法在自动化测试中验证 AI 的推理输出。AI 运行时行为应在人工审查和使用中验证。
- **评估日志 JSON 输出格式** — 原因：`eval.schema.json` 在本次变更中不受影响（已在 proposal.md out_of_scope 中明确），items 数组不区分 required/optional，无需测试。
- **OpenSpec CLI 行为** — 原因：本变更不涉及 CLI 代码修改，CLI 的行为在 out_of_scope 中明确排除。
- **MCP 工具行为** — 原因：本变更不涉及 MCP 工具代码修改，MCP 工具的 schema 和行为在 out_of_scope 中明确排除。
