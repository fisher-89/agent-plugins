# 测试设计: restructure-test-design-by-file

> **日期**: 2026-07-21

---

## 验收范围

<!-- 逐条映射 proposal.md 的验收标准 -->

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | 模板：单元测试章节按 per-file 组织（`### {{源文件}} -> {{测试文件}}`），每文件章节包含 `#### 待测功能`（列表）、`#### 用例`（表格）、`#### Mock策略`（表格） | 不可测试 | — | — |
| AC-2 | 模板：集成测试章节按自由命名关系标题组织（`### {{关系标题}} → {{测试文件}}`），每关系章节包含 `**涉及模块**` 表格、`**关联AC**`、`**关系描述**` 段落、至少一个 `#### 场景:` | 不可测试 | — | — |
| AC-3 | 模板：待测功能用列表、涉及模块用表格、关系描述用段落 | 不可测试 | — | — |
| AC-4 | 模板：表格列定义符合新结构（单元测试 `#### 用例` 列名 `测试对象\|路径类型\|测试条件\|迭代类型`；集成测试场景 `##### 用例` 列名 `路径类型\|测试条件\|迭代类型`；Mock策略表列名 `Mock主体\|Mock方案\|应用场景`） | 不可测试 | — | — |
| AC-5 | 模板：集成测试每项关系至少一个场景，场景含用例表 | 不可测试 | — | — |
| AC-6 | planner：输出指令匹配新模板，Process 步骤按 per-file 描述单元测试生成逻辑、按自由命名关系标题描述集成测试生成逻辑 | 不可测试 | — | — |
| AC-7 | evaluator：T8 格式检查适配新结构，检查点涵盖 per-file 分组标题格式、关系标题 `→` 模式、涉及模块 ≥2 行、关联AC 非空、关系描述非空、场景标题格式、用例表列名 | 不可测试 | — | — |

---

## 单元测试

本变更为纯模板和 agent prompt 调整，涉及的三个文件均为 Markdown 文档，无可执行运行时源码：

- `plugins/dev-team/templates/artifacts/test-design.md.template` — 模板文件，定义 test-design.md 的结构格式
- `plugins/dev-team/agents/test-design-planner.md` — 生成器 agent 指令文档
- `plugins/dev-team/agents/test-design-evaluator.md` — 检查器 agent 指令文档

`test_resolve_paths` 确认上述文件均无关联的单元测试路径（返回 `Not a testable source file`）。不新增单元测试文件。

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| — | — | — | 无单元测试用例（无运行时源码变更） | — |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| — | — | — | — |

---

## 集成测试

本变更不涉及跨进程/跨模块边界的运行时集成场景。三个修改文件均为 Markdown 文档类型，其正确性通过以下方式验证：

1. **AC-1~AC-5（模板结构）**：通过 code review 逐项比对 design.md 中模板修改明细与 spec 定义，确认模板 headings、table columns、section ordering 与设计一致
2. **AC-6（planner Process 指令）**：通过 code review 确认 planner 的 Process 步骤描述与模板结构对齐
3. **AC-7（evaluator T8 格式检查）**：通过 code review 确认 T8 检查项覆盖新模板的所有结构维度

不新增 `__tests__/` 集成测试文件。

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| — | — | — | 无集成测试场景（无运行时模块交互） | — |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| — | — | — | — |

---

## 不可测试项

| 条目 | 原因 |
|------|------|
| AC-1 ~ AC-5：模板结构重组 — `plugins/dev-team/templates/artifacts/test-design.md.template` | 该文件为 Markdown 模板文档，非可执行代码。`test_resolve_paths` 返回 `Not a testable source file`。模板结构正确性通过 code review（逐项对照 design.md 与 spec 确认 headings、table columns、section ordering 匹配设计）和 evaluator T8 运行时检查（在后续 test-design 阶段由 test-design-evaluator 自动验证合格产物的模板一致性）来验证 |
| AC-6：Planner 输出指令更新 — `plugins/dev-team/agents/test-design-planner.md` | 该文件为 Markdown agent 指令文档，非可执行代码。`test_resolve_paths` 返回 `Not a testable source file`。Planner 的 Process 步骤与模板结构的对齐需通过 code review 逐项确认：Step 8（集成测试场景生成逻辑）、Step 10~11（单元测试 per-file 映射逻辑）、Step 13（模板确认方式）与 design.md 中「Planner 修改明细」章节的一致性 |
| AC-7：Evaluator T8 格式检查适配 — `plugins/dev-team/agents/test-design-evaluator.md` | 该文件为 Markdown agent 指令文档，非可执行代码。`test_resolve_paths` 返回 `Not a testable source file`。T8 检查新增的 12 个结构维度（per-file 分组标题模式、关系标题 `→` 模式、涉及模块 ≥2 行、关联AC 非空、关系描述非空、场景标题格式、用例表列名等）需通过 code review 逐项对照 design.md 中「Evaluator 修改明细」章节与 spec 定义，确认每个检查维度在 evaluator 的 checklist 中有对应项且判定逻辑正确 |
| 下游文件不改动验证 — `plugins/dev-team/agents/test-gen-generator.md`、`plugins/dev-team/agents/test-gen-evaluator.md` | 设计.md 明确标注这些文件为「不要修改」。回归验证通过 code review 确认 diff 中不包含对这些文件的改动；无需自动化测试 |
