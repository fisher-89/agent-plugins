# 提案: 评估器检核表全部项均为强制项

> **变更**: evaluatorchecklist
> **日期**: 2026-06-03
> **状态**: 提案

---

## 问题

当前所有评估器（Evaluator）代理的静态检核表（Static Checklist）中存在 `必须` 列，用于区分"强制项"（`true`）和"可选项"（`false`）。这一设计引入以下问题：

1. **语义矛盾**：检核表中的每个检查项之所以存在，是因为它对质量有影响。将部分项标记为"可选项"意味着这些项可以不通过就判定为"pass"，削弱了评估环节的严谨性。

2. **判定逻辑不一致**：部分评估器的判定规则中引用的检查项范围与检核表实际条目不匹配：
   - `code-review-evaluator` 判定规则写 `(C1-C5)`，但 C6、C7、C8 在表中也是 `必须: true`，却未被纳入判定条件
   - `acceptance-evaluator` 判定规则写 `(A1-A4, A7)`，但 A7 在检核表中不存在，A5 被标记为可选项

3. **审计歧义**：可选项在评估日志中若标记为 `pass: false`，读日志的人无法判断这是否是必须解决的问题，还是仅仅是"参考信息"。

4. **维护负担**：需要维护"必须"列的语义 —— 每次新增检查项时都需要判断它是否是"必须"的，增加了决策开销。

目前含有可选项（`必须: false`）的评估器及条目：

| 评估器 | 可选项 ID | 检查项描述 |
|--------|-----------|-----------|
| proposal-evaluator | R7 | 所有非能力的模板章节已填写实质性内容 |
| test-design-evaluator | T5 | 存在外部依赖时描述了 Mock 策略 |
| test-design-evaluator | T8 | 写操作测试包含幂等性验证 |
| dev-design-evaluator | D7 | 依赖项（运行时和构建/测试）已列出 |
| dev-design-evaluator | D9 | 所有模板章节已填写实质性内容 |
| acceptance-evaluator | A5 | proposal.md 中所有风险都有对应的代码缓解措施 |

---

## 提案

**移除所有评估器静态检核表中的 `必须` 列，使每个检查项都是强制项。** 判定规则改为"ALL items must pass"（所有项必须通过），而非"ALL required items must pass"。

具体变更：
1. 删除静态检核表 Markdown 表格中的 `必须` 列及对应的 `true`/`false` 值
2. 将原本标记为 `false` 的检查项提升为强制项
3. 所有评估器的判定规则统一表述为 "ALL items must pass"（所有项必须通过）
4. 修正判定规则中引用的检查项范围，使其与检核表实际条目一一对应
5. `unit-test-evaluator` 和 `integration-test-evaluator` 不受影响（使用诊断决策树格式，无静态检核表）

---

## 能力

### 修改的能力

- `phase-agents` — 更新评估器代理定义：移除静态检核表中的 `必须` 列，所有检查项升为强制项；修正判定规则表述和引用的检查项范围

---

## 变更范围

### 实现以下特性

- `agents/proposal-evaluator.md`：删除 `必须` 列，R7 从可选项升级为强制项，判定规则改为 "ALL items must pass"
- `agents/test-design-evaluator.md`：删除 `必须` 列，T5、T8 从可选项升级为强制项，判定规则改为 "ALL items must pass"
- `agents/dev-design-evaluator.md`：删除 `必须` 列，D7、D9 从可选项升级为强制项，判定规则改为 "ALL items must pass"
- `agents/code-review-evaluator.md`：删除 `必须` 列（所有项已为 `true`），判定规则从 `(C1-C5)` 修正为 `(C1-C8)`
- `agents/implementation-evaluator.md`：删除 `必须` 列（所有项已为 `true`），判定规则改为 "ALL items must pass"
- `agents/test-gen-evaluator.md`：删除 `必须` 列（所有项已为 `true`），判定规则改为 "ALL items must pass"
- `agents/acceptance-evaluator.md`：删除 `必须` 列，A5 从可选项升级为强制项，判定规则从 `(A1-A4, A7)` 修正为 `(A1-A5)`
- 所有 `"Each required item must pass"` 或 `"ALL required items"` 改为统一表述
- 静态检核表 Markdown 表格删除第三列（`必须`）

### 不要修改

- `agents/unit-test-evaluator.md` 和 `agents/integration-test-evaluator.md`：使用诊断决策树格式，无静态检核表和 `必须` 列，不在变更范围内
- `plugins/dev-team/templates/artifacts/eval.schema.json`：输出格式不变，items 数组不区分 required/optional
- 评估器之外的代理文件（如 planner、generator、executor）
- OpenSpec CLI 或 MCP 工具的行为

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | 所有评估器（proposal、test-design、dev-design、code-review、test-gen、implementation、acceptance）的静态检核表不再包含 `必须` 列 | 逐文件 grep 确认 `必须` 列不存在于上述 7 个文件的静态检核表中 | P0 |
| AC-2 | 原本标记为 `false` 的检查项（R7、T5、T8、D7、D9、A5）在评估时若未满足，会导致 verdict 为 "fail" | 构造对应的不满足条件输入，运行评估器确认 verdict 为 "fail" | P0 |
| AC-3 | `code-review-evaluator` 的判定规则引用 `(C1-C8)` 而非 `(C1-C5)` | 读取 `agents/code-review-evaluator.md`，确认判定规则行中的范围 | P0 |
| AC-4 | `acceptance-evaluator` 的判定规则引用 `(A1-A5)` 而非 `(A1-A4, A7)` | 读取 `agents/acceptance-evaluator.md`，确认判定规则行中的范围 | P0 |
| AC-5 | 所有评估器的判定规则统一表述为 "ALL items must pass" | grep 确认 "ALL required items" 模式不存在于上述 7 个文件中 | P1 |
| AC-6 | `unit-test-evaluator` 和 `integration-test-evaluator` 文件未发生任何变更 | `git diff` 确认这两个文件无变更 | P0 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 原本可选项升级后导致现有变更评估失败率短期上升 | 开发流程中增加回溯迭代次数 | 中 | 在提案中明确列出哪些项从可选项升级，使团队有预期；这些项本身就有价值，升级对质量有利 |
| 部分评估器判定规则的范围存在不一致（如 C1-C5 vs C1-C8）可能导致人工判断错误 | 误判 verdict | 低 | 本变更统一修正所有不一致的范围引用 |
| 误改 unit-test-evaluator / integration-test-evaluator（它们没有 `必须` 列但表格结构类似） | 对诊断决策树造成破坏 | 低 | 已在 out_of_scope 中明确排除；验收标准 AC-6 通过 git diff 验证这两个文件无变更 |
