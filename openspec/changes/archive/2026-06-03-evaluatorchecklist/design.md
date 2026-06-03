# 设计: evaluatorchecklist

> **变更**: evaluatorchecklist
> **日期**: 2026-06-03
> **基于**: proposal.md, specs/phase-agents/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| proposal-evaluator | 评估 proposal.md 的完整性和质量；判定规则从 "ALL required items pass" 改为 "ALL items pass"；R7 从可选项升级为强制项 | `plugins/dev-team/agents/proposal-evaluator.md` | 无（独立 Markdown prompt 文件） | Markdown prompt |
| test-design-evaluator | 评估 test-design.md 的完整性和覆盖度；判定规则从 "ALL required items pass" 改为 "ALL items pass"；T5、T8 从可选项升级为强制项 | `plugins/dev-team/agents/test-design-evaluator.md` | 无（独立 Markdown prompt 文件） | Markdown prompt |
| dev-design-evaluator | 评估 design.md 的完整性和决策质量；判定规则从 "ALL required items pass" 改为 "ALL items pass"；D7、D9 从可选项升级为强制项 | `plugins/dev-team/agents/dev-design-evaluator.md` | 无（独立 Markdown prompt 文件） | Markdown prompt |
| code-review-evaluator | 评估代码 diff 的安全性、测试覆盖和错误处理；判定规则从 "(C1-C5)" 修正为 "(C1-C8)"，删除 `必须` 列 | `plugins/dev-team/agents/code-review-evaluator.md` | 无（独立 Markdown prompt 文件） | Markdown prompt |
| test-gen-evaluator | 评估生成的测试代码与 test-design.md 的一致性；判定规则从 "ALL required items pass" 改为 "ALL items pass" | `plugins/dev-team/agents/test-gen-evaluator.md` | 无（独立 Markdown prompt 文件） | Markdown prompt |
| implementation-evaluator | 评估实现代码与 design.md 的一致性；判定规则从 "ALL required items pass" 改为 "ALL items pass" | `plugins/dev-team/agents/implementation-evaluator.md` | 无（独立 Markdown prompt 文件） | Markdown prompt |
| acceptance-evaluator | 追踪需求从 proposal.md 到代码的完整实现链路；判定规则从 "(A1-A4, A7)" 修正为 "(A1-A5)"；A5 从可选项升级为强制项 | `plugins/dev-team/agents/acceptance-evaluator.md` | 无（独立 Markdown prompt 文件） | Markdown prompt |

### 不受影响的组件

| 组件 | 原因 | 文件位置 |
|------|------|----------|
| unit-test-evaluator | 使用诊断决策树格式，无静态检核表和 `必须` 列 | `plugins/dev-team/agents/unit-test-evaluator.md` |
| integration-test-evaluator | 使用诊断决策树格式，无静态检核表和 `必须` 列 | `plugins/dev-team/agents/integration-test-evaluator.md` |
| eval.schema.json | 输出模式不变，items 数组不区分 required/optional | `plugins/dev-team/templates/artifacts/eval.schema.json` |

### 组件图

本变更仅修改 7 个独立的 Markdown prompt 文件（评估器代理定义）。每个文件是 Claude Code 在对应工作流阶段加载的 prompt，它们之间无运行时依赖关系，修改是独立的文本编辑操作。变更不涉及代码逻辑、运行时架构或 API 接口。

```
                    +-- proposal-evaluator.md (R7 升强制)
                    |-- test-design-evaluator.md (T5, T8 升强制)
                    |-- dev-design-evaluator.md (D7, D9 升强制)
所有评估器 prompt --|-- code-review-evaluator.md (范围修正: C1-C5 -> C1-C8)
                    |-- test-gen-evaluator.md (表述统一)
                    |-- implementation-evaluator.md (表述统一)
                    |-- acceptance-evaluator.md (范围修正: A1-A4,A7 -> A1-A5; A5 升强制)
                    
不受影响:
                    |-- unit-test-evaluator.md (无静态检核表)
                    |-- integration-test-evaluator.md (无静态检核表)
```

---

## 数据流

### 流程描述

本变更是纯文本编辑操作，不涉及运行时数据流动。每个变更步骤的流程如下：

1. **分析**：读取目标评估器文件，确认静态检核表（`## Static Checklist`）的表格结构和判定规则行
2. **修改检核表**：将 Markdown 表格从 4 列（ID、检查项、必须、判断依据）缩减为 3 列（ID、检查项、判断依据），删除整列数据和表头
3. **修改判定规则**：将 `## Process` 和 `## Static Checklist` 中的 "ALL required items" 替换为 "ALL items"
4. **修正范围引用**：对 code-review-evaluator 和 acceptance-evaluator，将判定范围从错误的引用修正为与检核表条目一致
5. **验证**：逐文件 grep 确认 `必须` 列已移除；确认 unit-test-evaluator 和 integration-test-evaluator 无变更

### 数据模型

本变更不涉及新的数据模型。评估器的输出格式（`eval.schema.json`）不受影响，items 数组结构不变。

---

## 路由/API 设计

不适用。本变更不涉及 API 或路由修改。

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **所有检查项均为强制项** | 检核表中的每个检查项之所以存在，是因为它对质量有影响。将部分项标记为"可选项"（`必须: false`）削弱了评估环节的严谨性，且造成判定规则与检核表不匹配 | **备选：保留可选项但添加注释说明"可选项失败仅记录不阻断"**。被拒理由：仍然保留区分逻辑但隐藏它，复杂度未降；审计日志中 `pass: false` 的可选项仍然带来解读歧义；且新增检查项时仍需判断是否"必须"，维护负担没有消除 |
| D2 | **统一判定规则表述为 "ALL items must pass"** | 消除 "ALL required items" 与 "ALL items" 两种表述的不一致，使规则语义清晰：全部项都必须通过 | **备选：保留各自表述，仅在检核表中删除 `必须` 列**。被拒理由：检核表已无"required"概念，判定规则中保留 "required" 会造成语义矛盾；统一表述使所有评估器行为一致，降低 AI 代理理解偏差 |
| D3 | **code-review-evaluator 范围从 (C1-C5) 修正为 (C1-C8)** | 当前判定规则仅引用 C1-C5，但检核表已有 C1-C8 共 8 项且全部为 `必须: true`。引用不完整导致评估器可能忽略 C6-C8 的结果 | **备选：将 C6-C8 降级为可选项，保持判定规则为 (C1-C5)**。被拒理由：C6（死代码）、C7（可读性）、C8（类型复用）均为代码质量的关键方面，降级不可接受；与本次变更"全部强制"的目标相悖 |
| D4 | **acceptance-evaluator 范围从 (A1-A4, A7) 修正为 (A1-A5)** | 当前判定规则引用 A7，但检核表中只有 A1-A5（不存在 A7）。新规则直接引用 A1-A5，与检核表一一对应 | **备选：补上 A7 检查项使范围合法**。被拒理由：A7 原本指向"tasks.md 未完成项"的检查（记录在 Constraints 中但不在检核表中），作为检核表项不如作为前提条件；直接使用 A1-A5 更清晰且无歧义 |
| D5 | **unit-test-evaluator 和 integration-test-evaluator 不在变更范围内** | 这两个评估器使用诊断决策树格式，无静态检核表和 `必须` 列，不需要修改 | **备选：统一也修改这两个文件**。被拒理由：修改无静态检核表的文件会增加回归风险，且无实际收益；AC-6 要求通过 git diff 确认无变更 |

---

## 依赖

### 运行时依赖

- 无。本变更仅修改 7 个 Markdown prompt 文件，不引入运行时依赖。

### 构建/测试依赖

- 无。本变更不涉及代码编译、测试或构建。

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 原本可选项升级后导致现有变更评估失败率短期上升 | 开发流程中增加回溯迭代次数 | 中 | 在提案中明确列出哪些项从可选项升级（R7、T5、T8、D7、D9、A5），使团队有预期；这些项本身就有价值，升级对质量有利 |
| 部分评估器判定规则的范围存在不一致（如 C1-C5 vs C1-C8）可能导致人工判断错误 | 误判 verdict | 低 | 本变更统一修正所有不一致的范围引用 |
| 误改 unit-test-evaluator / integration-test-evaluator | 对诊断决策树造成破坏 | 低 | 已在 out_of_scope 中明确排除；验收标准 AC-6 通过 git diff 验证这两个文件无变更 |
| 文件修改过程中表格格式错乱（列数不对齐） | 评估器 prompt 渲染异常，AI 代理无法正确解析 | 低 | 每修改一个文件后手动验证 Markdown 表格列数一致；通过渲染预览确认表格可读 |

---

## 迁移步骤

本变更不涉及迁移。变更完成后，所有新的评估流程将自动使用更新后的检核表格式和判定规则。现有已通过的评估日志不受影响。

---

## 待决问题

- 无。
