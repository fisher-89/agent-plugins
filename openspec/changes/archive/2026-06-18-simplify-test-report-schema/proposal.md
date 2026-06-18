# 提案: simplify-test-report-schema

> **变更**: simplify-test-report-schema
> **日期**: 2026-06-18
> **状态**: draft

---

## 问题

`plugins/dev-team/agents/unit-test-executor.md` 定义的 `unit-test-execution.json` 报告格式（约第 168 行）存在多处冗余字段与命名歧义，增加 Executor 写入与 Evaluator 读取的复杂度，且同一语义在不同层级重复使用相同字段名。

具体问题包括：

1. **`html_reports` 及 `html_report` 冗余** — HTML 覆盖率报告仅供人工浏览，对自动化链路无用；覆盖率输出应仅保留 JSON 格式以供机器解析
2. **`failures[]` 与 `test_cases[]` 重叠** — 失败用例在两条数组中重复出现（`name`/`file` 重复），`failures` 仅额外携带错误详情字段
3. **`integration_test.failures[]` / `integration_test.test_cases[]`** — 嵌套对象内存在相同的重叠模式
4. **6 个扁平 `coverage_*` 字段命名歧义**：
   - 顶层 `coverage` = 加权平均测量值，但名称过于泛化
   - `coverage_overrides[].coverage` = 按目录测量值，与顶层 `coverage` 同名但语义不同
   - `coverage_thresholds` 与 `coverage_overrides[].thresholds` 将配置与测量混在同一层级
   - `coverage_pass` 为总判定，但未体现其与 overrides 的从属关系
5. **`command` 字段** — 单字符串无法表达多框架场景下的多条命令（本次不改动，留待后续）

Evaluator 的决策树逻辑、覆盖率门控语义（`coverage: null` 自动通过）必须保持不变；报告仍须为单一自包含 JSON 文件。

---

## 提案

对 `unit-test-execution.json` 做三项结构性简化，同步更新 Executor 写入与 Evaluator 消费逻辑：

### 1. 覆盖率字段合并：6 个扁平字段 → 1 个嵌套 `coverage` 对象

将 `coverage`（测量值）、`coverage_thresholds`、`coverage_pass`、`coverage_by_framework`、`coverage_overrides` 合并为单一嵌套对象；未生成覆盖率时顶层 `"coverage": null`：

```json
{
  "coverage": {
    "pass": true,
    "measured": { "lines": 82, "branches": 74, "functions": 81 },
    "thresholds": { "lines": 80, "branches": 70, "functions": 75 },
    "by_framework": [
      {
        "framework": "vitest",
        "measured": { "lines": 90, "branches": 80, "functions": 85 }
      }
    ],
    "overrides": [
      {
        "glob": "demo/**",
        "thresholds": { "lines": 60, "branches": 70, "functions": 75 },
        "measured": { "lines": 65, "branches": 75, "functions": 80 },
        "pass": true
      }
    ]
  }
}
```

字段语义映射：

| 旧字段 | 新路径 |
|--------|--------|
| `coverage: {lines, branches, functions}` | `coverage.measured` |
| `coverage_thresholds` | `coverage.thresholds` |
| `coverage_pass` | `coverage.pass` |
| `coverage_by_framework` | `coverage.by_framework`（`coverage` → `measured`） |
| `coverage_overrides` | `coverage.overrides`（`coverage` → `measured`） |
| `html_reports` | 删除 |
| `coverage_by_framework[].html_report` | 删除（覆盖率改为仅 JSON 输出） |

### 2. 失败详情并入 `test_cases`

在 `status: "failed"` 的 `test_cases[]` 条目上直接附加 `line`、`error_type`、`error_message`、`stack_trace`（及可选 `design_ref`），删除顶层 `failures[]` 及 `integration_test.failures[]`。

### 3. 覆盖率仅输出 JSON，删除 HTML 报告相关字段

Executor 执行覆盖率脚本时使用 JSON reporter（如 vitest 的 `json-summary`、jest 的 `json-summary`、pytest-cov 的 `json`）。覆盖率产物仅保留 JSON 摘要文件（`coverage-summary.json`），不再生成或移动 HTML 覆盖率报告。删除 `html_reports` 顶层字段和 `coverage.by_framework[].html_report`。

---

## 能力

### 新增能力

（无 — 本次仅修改现有能力。）

### 修改的能力

- **test-execution-diagnostics** — 更新 `reports/unit-test-execution.json` 的 schema 定义：嵌套 `coverage` 对象、合并失败详情到 `test_cases`、移除 `html_reports` 与 `failures[]`；更新 Module Contract 中的 Coverage Report Schema 表
- **phase-agents** — 更新 `unit-test-executor.md` 报告写入模板与字段说明；更新 `unit-test-evaluator.md` 的 U1/U3 检核项、Step 1 完整性校验、覆盖率子检查、决策树输入（从 `test_cases` 中 `status: "failed"` 条目读取错误字段）及 findings 构建逻辑；更新 `FRAMEWORK_REGISTRY` 使覆盖率脚本输出 JSON-only

---

## 变更范围

### 实现以下特性

- `plugins/dev-team/agents/unit-test-executor.md`：更新 JSON 报告模板（嵌套 `coverage`、失败详情在 `test_cases`、删除 `html_reports`/`failures[]`/`html_report`）；覆盖率脚本使用 JSON reporter 输出；更新 `integration_test` 嵌套对象同样合并 `failures`；更新无覆盖率时的示例为 `"coverage": null`
- `plugins/dev-team/agents/unit-test-evaluator.md`：U1 必需字段列表改为新 schema；U3 改为 `coverage.pass === true` 或 `coverage === null`；Step 1 校验 `coverage` 为 object 或 null；决策树从 `test_cases.filter(c => c.status === "failed")` 读取错误字段；findings 模板使用 `coverage.measured`、`coverage.pass`、`coverage.by_framework`；移除所有 HTML 报告路径引用
- 保持 Evaluator 决策树优先级与回溯目标逻辑等价
- 保持 `coverage === null` 时覆盖率检查自动通过

- `plugins/dev-team/bin/src/commands/test-get-framework-config.ts`：`FRAMEWORK_REGISTRY` 中各框架的 `coverage_cmd` 改为仅输出 JSON reporter（如 vitest 加 `--coverage.reporter=json-summary`、jest 加 `--coverageReporters=json-summary`）；`coverage_artifacts` 收窄为仅 JSON 摘要文件路径（如 `['coverage/coverage-summary.json']`）；`coverage_cleanup` 保持不变（仍清理整个 `coverage/` 目录）

### 不要修改

- `command` 字段结构（单字符串 → 多命令数组留待后续变更）
- `test_detect_frameworks` 的检测逻辑（glob 匹配、文件扫描、plan 生成流程）
- 覆盖率计算与加权平均算法
- `integration-test-executor.md` / `integration-test-evaluator.md`（独立报告文件，本次仅改 unit-test 链路）
- 已归档变更目录中的历史 `unit-test-execution.json` 报告
- `openspec/config.json` 配置结构
- Evaluator 诊断决策树的分类规则与优先级

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | Executor 报告模板中覆盖率使用嵌套 `coverage` 对象（含 `pass`、`measured`、`thresholds`、`by_framework`、`overrides`），`by_framework` 条目不含 `html_report`；无顶层 `coverage_thresholds`/`coverage_pass`/`coverage_by_framework`/`coverage_overrides`/`html_reports` | 读取 `unit-test-executor.md` 报告 JSON 示例，确认字段结构 |
| AC-2 | 无覆盖率时报告仅含 `"coverage": null`，不含其他 coverage 相关顶层字段 | 检查 Executor 文档中 no-coverage 示例 |
| AC-3 | 失败用例仅在 `test_cases[]` 中，`status: "failed"` 条目含 `line`、`error_type`、`error_message`、`stack_trace`；无顶层 `failures[]` | 检查 Executor 报告模板与写入说明 |
| AC-4 | `integration_test` 嵌套对象同样无 `failures[]`，失败详情在 `test_cases` 中 | 检查 Executor 文档中 integration_test 示例 |
| AC-5 | Evaluator U1/U3 与 Step 1 引用新字段路径（`coverage.pass`、`coverage.measured` 等） | 读取 `unit-test-evaluator.md` 检核表与 Process |
| AC-6 | Evaluator 决策树从 `test_cases` 失败条目读取 `error_type`、`file`、`line`，逻辑与改前等价 | 对照决策树步骤，确认输入源变更但判定规则不变 |
| AC-7 | `coverage === null` 时 Evaluator 覆盖率项自动 pass，evidence 为「覆盖率检查未配置或生成失败，跳过」 | 检查 Evaluator Step 3 覆盖率子检查 |
| AC-8 | Evaluator findings 不引用任何 HTML 报告路径（`html_reports` 及 `html_report` 均已移除） | 检查 Step 5 findings 模板 |
| AC-9 | `FRAMEWORK_REGISTRY` 中各框架 `coverage_cmd` 使用 JSON-only reporter 参数，`coverage_artifacts` 仅含 JSON 摘要文件路径 | 读取 `test-get-framework-config.ts`，确认命令参数与 artifacts |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 进行中的变更仍使用旧 schema 写入报告 | Evaluator U1 完整性校验失败 | 中 | 变更范围仅限 agent 文档；实施时一次性切换，不保留双读兼容 |
| Evaluator 遗漏某处旧字段引用 | 运行时误判或 findings 缺失 | 低 | AC-5/AC-8 逐项 grep 旧字段名；实施阶段全文搜索 `coverage_pass`、`html_reports`、`failures` |
| `integration_test` 与顶层 `test_cases` 失败字段不一致 | 集成测试失败诊断遗漏 | 低 | AC-4 明确要求嵌套对象采用相同 test_cases 结构 |
| 嵌套 `coverage` 与旧顶层 `coverage` 同名导致混淆 | 实施者误用旧语义 | 中 | spec 中提供完整字段映射表；Module Contract 更新类型定义 |
