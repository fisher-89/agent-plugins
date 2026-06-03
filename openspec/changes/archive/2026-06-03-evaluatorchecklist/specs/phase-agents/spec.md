## ADDED Requirements

### Requirement: 评估器静态检核表无可选检查项
所有评估器（Evaluator）代理的静态检核表（Static Checklist）SHALL 不在检查项表格中包含 `必须` 列（或任何等效的 required/optional 标识列）。所有检查项均为强制项。

受影响的评估器代理文件列表：
- `agents/proposal-evaluator.md` — 原 R7 为可选项，已升级为强制项
- `agents/test-design-evaluator.md` — 原 T5、T8 为可选项，已升级为强制项
- `agents/dev-design-evaluator.md` — 原 D7、D9 为可选项，已升级为强制项
- `agents/code-review-evaluator.md` — 所有项原已为强制项，删除 `必须` 列
- `agents/test-gen-evaluator.md` — 所有项原已为强制项，删除 `必须` 列
- `agents/implementation-evaluator.md` — 所有项原已为强制项，删除 `必须` 列
- `agents/acceptance-evaluator.md` — 原 A5 为可选项，已升级为强制项

以下评估器 SHALL NOT 包含在本次变更范围内：
- `agents/unit-test-evaluator.md` — 无静态检核表（使用诊断决策树格式），不继承本规则
- `agents/integration-test-evaluator.md` — 无静态检核表（使用诊断决策树格式），不继承本规则

#### Scenario: 静态检核表不含 required/optional 标识列
- **WHEN** 读取 `agents/proposal-evaluator.md` 的 `## Static Checklist` 表格
- **THEN** 表格列头不包含 `必须` 或任何 required/optional 标识列
- **AND** 表格行包含 `ID`、`检查项`、`判断依据` 三列

#### Scenario: 原本可选项未满足时导致 fail
- **WHEN** 评估器执行时，原本标记为 `false` 的检查项（如 proposal-evaluator 的 R7）被判定为不通过
- **THEN** verdict SHALL 为 `"fail"`
- **AND** 评估日志中该检查项的 `pass` 字段为 `false`

### Requirement: 判定规则统一为 "ALL items must pass"
所有评估器的 Process 步骤中的判定规则 SHALL 使用 "ALL items must pass" 表述，而非 "ALL required items must pass"。判定规则中的检查项范围引用 SHALL 与实际检核表条目一一对应。

修正的判定规则：
- `agents/proposal-evaluator.md`：`"pass" only if ALL items pass`
- `agents/test-design-evaluator.md`：`"pass" only if ALL items pass`
- `agents/dev-design-evaluator.md`：`"pass" only if ALL items pass`
- `agents/code-review-evaluator.md`：`"pass" only if ALL items pass (C1-C8)` — 原值 `(C1-C5)` 不完整
- `agents/test-gen-evaluator.md`：`"pass" only if ALL items pass`
- `agents/implementation-evaluator.md`：`"pass" only if ALL items pass`
- `agents/acceptance-evaluator.md`：`"pass" only if ALL items pass (A1-A5)` — 原值 `(A1-A4, A7)` 中 A7 不存在

#### Scenario: code-review-evaluator 判定规则引用全部 C1-C8
- **WHEN** 读取 `agents/code-review-evaluator.md` 的判定规则行
- **THEN** 行内容包含 `(C1-C8)` 而非 `(C1-C5)`
- **AND** 无 `"required"` 限定词修饰 ALL items

#### Scenario: acceptance-evaluator 判定规则引用 A1-A5
- **WHEN** 读取 `agents/acceptance-evaluator.md` 的判定规则行
- **THEN** 行内容包含 `(A1-A5)` 而非 `(A1-A4, A7)`
- **AND** 无 `"required"` 限定词修饰 ALL items

## MODIFIED Requirements

### Requirement: Phase identifier updates in evaluator agents (判定规则和检核表格式更新)
**原 Requirement（来自 openspec/specs/phase-agents/spec.md）：**

All evaluator agents that reference phase identifiers in eval-log commands SHALL use the updated identifiers...

**变更：**

除了已有的阶段标识符更新外，本变更更新以下内容：
- 评估器的静态检核表格式从四列（ID、检查项、必须、证据提示）变为三列（ID、检查项、证据提示）
- 判定规则从 "ALL required items must pass" 变为 "ALL items must pass"
- code-review-evaluator 的判定范围从 (C1-C5) 修正为 (C1-C8)
- acceptance-evaluator 的判定范围从 (A1-A4, A7) 修正为 (A1-A5)

#### Scenario: proposal-evaluator 检核表格式更新
- **WHEN** 读取 `agents/proposal-evaluator.md`
- **THEN** `## Static Checklist` 表格列头为 `ID`、`检查项`、`判断依据`
- **AND** 无 `必须` 列
- **AND** 判定规则表述为 `"pass" only if ALL items pass`

#### Scenario: test-design-evaluator 检核表格式更新
- **WHEN** 读取 `agents/test-design-evaluator.md`
- **THEN** `## Static Checklist` 表格无 `必须` 列
- **AND** T5、T8 如不满足则导致 verdict 为 `"fail"`

#### Scenario: dev-design-evaluator 检核表格式更新
- **WHEN** 读取 `agents/dev-design-evaluator.md`
- **THEN** `## Static Checklist` 表格无 `必须` 列
- **AND** D7、D9 如不满足则导致 verdict 为 `"fail"`

#### Scenario: acceptance-evaluator 检核表格式更新
- **WHEN** 读取 `agents/acceptance-evaluator.md`
- **THEN** `## Static Checklist` 表格无 `必须` 列
- **AND** A5 如不满足则导致 verdict 为 `"fail"`
- **AND** 判定规则引用 `(A1-A5)` 而非 `(A1-A4, A7)`

## Module Contract

### Agent Files (`plugins/dev-team/agents/`)

| Agent | Checklist Format | Verdict Rule | 变更说明 |
|-------|-----------------|--------------|---------|
| proposal-evaluator | 3 列（ID、检查项、证据提示） | ALL items pass | 删除 `必须` 列；R7 升强制；判定规则去除 "required" |
| test-design-evaluator | 3 列（ID、检查项、证据提示） | ALL items pass | 删除 `必须` 列；T5、T8 升强制 |
| dev-design-evaluator | 3 列（ID、检查项、证据提示） | ALL items pass | 删除 `必须` 列；D7、D9 升强制 |
| code-review-evaluator | 3 列（ID、检查项、证据提示） | ALL items pass (C1-C8) | 删除 `必须` 列；范围从 C1-C5 修正为 C1-C8 |
| test-gen-evaluator | 3 列（ID、检查项、证据提示） | ALL items pass | 删除 `必须` 列；判定规则去除 "required" |
| implementation-evaluator | 3 列（ID、检查项、证据提示） | ALL items pass | 删除 `必须` 列；判定规则去除 "required" |
| acceptance-evaluator | 3 列（ID、检查项、证据提示） | ALL items pass (A1-A5) | 删除 `必须` 列；A5 升强制；范围从 A1-A4,A7 修正为 A1-A5 |
| unit-test-evaluator | 无静态检核表（决策树格式） | 诊断决策树 | 不涉及变更 |
| integration-test-evaluator | 无静态检核表（决策树格式） | 诊断决策树 | 不涉及变更 |
