# 实施任务: evaluatorchecklist

---

## 阶段 1: 准备工作

- [x] 确认当前工作目录为 `plugins/dev-team/agents/`，列出所有 evaluator 文件确认清单
- [x] 记录各文件的原始 git hash/sha，以便验证 unit-test-evaluator.md 和 integration-test-evaluator.md 无变更

---

## 阶段 2: 修改 proposal-evaluator（R7 升强制，删除 `必须` 列，判定规则统一）

- [x] `plugins/dev-team/agents/proposal-evaluator.md` — 删除 `## Static Checklist` 表格中的 `必须` 列（表头列和所有行数据）
- [x] `plugins/dev-team/agents/proposal-evaluator.md` — 将 `## Static Checklist` 和 `## Process` 中的判定规则从 "ALL required items" 改为 "ALL items"

---

## 阶段 3: 修改 test-design-evaluator（T5、T8 升强制，删除 `必须` 列，判定规则统一）

- [x] `plugins/dev-team/agents/test-design-evaluator.md` — 删除 `## Static Checklist` 表格中的 `必须` 列
- [x] `plugins/dev-team/agents/test-design-evaluator.md` — 将判定规则从 "ALL required items" 改为 "ALL items"

---

## 阶段 4: 修改 dev-design-evaluator（D7、D9 升强制，删除 `必须` 列，判定规则统一）

- [x] `plugins/dev-team/agents/dev-design-evaluator.md` — 删除 `## Static Checklist` 表格中的 `必须` 列
- [x] `plugins/dev-team/agents/dev-design-evaluator.md` — 将判定规则从 "ALL required items" 改为 "ALL items"

---

## 阶段 5: 修改 code-review-evaluator（删除 `必须` 列，范围修正 C1-C5 -> C1-C8，判定规则统一）

- [x] `plugins/dev-team/agents/code-review-evaluator.md` — 删除 `## Static Checklist` 表格中的 `必须` 列
- [x] `plugins/dev-team/agents/code-review-evaluator.md` — 将 `## Process` 中的判定规则从 "(C1-C5)" 修正为 "(C1-C8)"
- [x] `plugins/dev-team/agents/code-review-evaluator.md` — 将判定规则中的 "ALL required items" 改为 "ALL items"

---

## 阶段 6: 修改 test-gen-evaluator（删除 `必须` 列，判定规则统一）

- [x] `plugins/dev-team/agents/test-gen-evaluator.md` — 删除 `## Static Checklist` 表格中的 `必须` 列
- [x] `plugins/dev-team/agents/test-gen-evaluator.md` — 将判定规则从 "ALL required items" 改为 "ALL items"

---

## 阶段 7: 修改 implementation-evaluator（删除 `必须` 列，判定规则统一）

- [x] `plugins/dev-team/agents/implementation-evaluator.md` — 删除 `## Static Checklist` 表格中的 `必须` 列
- [x] `plugins/dev-team/agents/implementation-evaluator.md` — 将判定规则从 "ALL required items" 改为 "ALL items"

---

## 阶段 8: 修改 acceptance-evaluator（A5 升强制，范围修正 A1-A4,A7 -> A1-A5，判定规则统一）

- [x] `plugins/dev-team/agents/acceptance-evaluator.md` — 删除 `## Static Checklist` 表格中的 `必须` 列
- [x] `plugins/dev-team/agents/acceptance-evaluator.md` — 将 `## Process` 中的判定规则从 "(A1-A4, A7)" 修正为 "(A1-A5)"
- [x] `plugins/dev-team/agents/acceptance-evaluator.md` — 将判定规则中的 "ALL required items" 改为 "ALL items"

---

## 阶段 9: 验证

- [x] **AC-1 验证**：对 7 个被修改文件逐文件 grep，确认 `## Static Checklist` 表格中不存在 `必须` 列
- [x] **AC-2 验证**：确认原本为可选项的检查项（R7、T5、T8、D7、D9、A5）在评估器中未标记为可选项（无 `false` 值或特殊逻辑）
- [x] **AC-3 验证**：读取 `plugins/dev-team/agents/code-review-evaluator.md`，确认判定规则行引用 `(C1-C8)`
- [x] **AC-4 验证**：读取 `plugins/dev-team/agents/acceptance-evaluator.md`，确认判定规则行引用 `(A1-A5)`
- [x] **AC-5 验证**：grep 确认 "ALL required items" 模式不在上述 7 个文件中出现
- [x] **AC-6 验证**：运行 `git diff` 确认 `plugins/dev-team/agents/unit-test-evaluator.md` 和 `plugins/dev-team/agents/integration-test-evaluator.md` 无变更
- [x] **格式验证**：对每个被修改文件检查 Markdown 表格渲染正确（列数：ID、检查项、判断依据，共 3 列）
