# 提案: unified-coverage-artifacts

> **变更**: unified-coverage-artifacts
> **日期**: 2026-06-08
> **状态**: 草稿

---

## 问题

per-directory-test-execution 实现后，覆盖率命令在每个框架对应的工作目录下执行（如 `plugins/dev-team/bin/`、`src/` 等），但覆盖率产物（coverage-summary.json、index.html、.lcov 文件等）被散落在各个框架的执行目录中。

当前存在以下四个问题：

1. **产物分散**：coverage/ 目录、.nyc_output/ 等文件留在各个框架工作目录下，没有统一收集到变更目录中。
2. **报告路径不统一**：report JSON 中的 `coverage_by_framework[*].html_report` 和 `html_reports` 路径指向各框架工作目录下的原始位置，这些路径在报告归档后将失效。
3. **临时文件残留**：生成覆盖率过程中产生的中间文件（.nyc_output/、target/llvm-cov/ 等）在执行后未被清理，污染项目目录。
4. **框架感知不足**：FrameworkConfig 目前只声明了 `coverage_output`（要解析的文件），缺少哪些额外产物需要移动、哪些目录需要清理的声明式配置。

---

## 提案

对框架注册表（FrameworkConfig）、PlanEntry、schema 和执行器流程进行以下改造：

1. **FrameworkConfig 新增 artifact 字段**：在框架注册表中新增 `coverage_artifacts: string[]`（声明要移动的产物 glob 列表）和 `coverage_cleanup: string[]`（声明要删除的目录/文件列表）。
2. **PlanEntry 扩展**：PlanEntry 类型新增 `coverage_artifacts` 和 `coverage_cleanup` 字段，`test_detect_frameworks` 生成 plan 时一并携带这些信息。
3. **Schema 同步扩展**：`test-detect-frameworks.schema.ts` 和 `test-get-framework-config.schema.ts` 的输出 schema 新增相应字段，通过 Zod 校验。
4. **Executor 新增"移动覆盖率产物"步骤**：在覆盖率命令执行后、覆盖率解析前，将产物从各框架工作目录移动到统一位置 `openspec/changes/<change>/reports/coverage/<framework>/`。
5. **路径更新**：移动后，`coverage_output` 解析路径和 `html_report` 写入路径更新为指向统一位置。

统一目录结构约定：

```
openspec/changes/<change>/
  reports/
    coverage/
      <framework-1>/
        coverage-summary.json
        index.html
        ...
      <framework-2>/
        coverage-summary.json
        index.html
        ...
```

---

## 能力

### 修改的能力

- **test-execution-diagnostics** — FrameworkConfig 接口新增 `coverage_artifacts` 和 `coverage_cleanup` 字段；PlanEntry 同步新增；test-detect-frameworks.schema.ts 和 test-get-framework-config.schema.ts 输出 schema 同步扩展；unit-test-executor 的 Executor agent 指南在覆盖率执行和解析之间插入"移动覆盖率产物"步骤，并更新路径为统一位置

---

## 变更范围

### 实现以下特性

- `test-get-framework-config.ts`：FrameworkConfig 接口新增 `coverage_artifacts: string[]` 和 `coverage_cleanup: string[]`；五个框架注册表条目填充对应的产物和清理配置
- `test-get-framework-config.schema.ts`：输出 schema 新增 `coverage_artifacts` 和 `coverage_cleanup` 字段
- `test-detect-frameworks.ts`：PlanEntry 接口新增 `coverage_artifacts` 和 `coverage_cleanup`；生成 plan 时从框架注册表携带这些字段
- `test-detect-frameworks.schema.ts`：plan 条目 schema 新增 `coverage_artifacts` 和 `coverage_cleanup` 字段
- `unit-test-executor.md`：在步骤 2（逐目录执行覆盖率命令）和步骤 3（覆盖率解析）之间插入新的"移动覆盖率产物"步骤，包含移动、路径更新、清理三个子步骤
- 统一路径约定：`reports/coverage/<framework>/<filename>`，相对于变更目录
- 清理逻辑：移动成功后删除原始临时目录（如 `coverage/`、`.nyc_output/`、`target/llvm-cov/`）

### 不要修改

- `test-detect-frameworks.ts` 的检测逻辑：`detected`、`frameworks` 字段结构、首匹配规则、`deriveWorkingDirectory` 保持不变
- `coverage-parser.ts` 和 `coverage-calculator.ts`：覆盖率解析和计算逻辑保持不变
- 报告 JSON schema 顶层字段结构不变（`reports/unit-test-execution.json` 的字段不增不减，仅路径值变更）
- `integration-test-executor.md`：集成测试执行器暂不引入覆盖率产物能力
- `openspec/config.json`：配置层不变，框架命令注册表保持硬编码

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `test_get_framework_config("vitest")` 返回的配置包含 `coverage_artifacts: ["coverage/**"]` 和 `coverage_cleanup: ["coverage", ".nyc_output"]` | 单元测试，对比返回对象的字段值 |
| AC-2 | `test_get_framework_config("rust")` 返回的配置中 `coverage_artifacts` 包含 `coverage/**` 和 `target/llvm-cov/**`，`coverage_cleanup` 包含 `coverage` 和 `target/llvm-cov` | 单元测试验证 |
| AC-3 | 全部五个框架（jest/vitest/vite-plus/bun/rust）的 FrameworkConfig 都包含 `coverage_artifacts` 和 `coverage_cleanup` 非空数组 | 单元测试遍历所有框架验证 |
| AC-4 | `test_detect_frameworks` 输出的 `plan` 条目包含 `coverage_artifacts` 和 `coverage_cleanup` 字段 | Zod schema 验证 plan 数组条目 |
| AC-5 | `test_get_framework_config` 的 Zod output schema 允许 `coverage_artifacts` 和 `coverage_cleanup` 为可选字段（向后兼容） | Schema parse 验证：无这两个字段时不会抛错 |
| AC-6 | unit-test-executor 在覆盖率命令执行后、解析前，将产物从框架工作目录移动到 `reports/coverage/<framework>/` | Agent 模拟场景验证：执行后目标路径存在产物文件 |
| AC-7 | 移动后 `coverage_by_framework` 的 `html_report` 路径更新为统一位置下的路径（如 `reports/coverage/vitest/index.html`） | Agent 模拟场景验证 |
| AC-8 | 移动成功后，原始 `coverage/`、`.nyc_output/` 等目录被删除 | Agent 模拟场景验证：源路径不存在 |
| AC-9 | 移动失败时（源目录不存在），保留原始状态，不执行清理 | Agent 模拟场景安全验证：不抛出异常，findings 记录错误 |
| AC-10 | 当 `coverage_artifacts` 为空数组时，跳过移动步骤 | Agent 模拟场景验证：无移动操作执行 |
| AC-11 | `coverage_output` 在移动后指向统一位置（如 `reports/coverage/vitest/coverage-summary.json`），用于后续覆盖率解析 | Agent 模拟场景验证：解析器从统一位置读取 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 移动操作在覆盖率命令执行但未生成产物时失败 | Executor 流程中断，覆盖率数据丢失 | 低 | 移动前检查源路径是否存在；移动失败时记录错误到 findings 但不阻断流程，`coverage` 设为 null |
| 清理操作误删用户文件 | 项目目录中用户文件被意外删除 | 极低 | 仅清理在 `coverage_cleanup` 中声明的目录名，且仅在移动成功后执行；清理范围限制在各框架工作目录下 |
| 并发执行时不同框架的产物互相覆盖 | 覆盖率报告被错误数据污染 | 低 | 统一位置按框架名隔离（`reports/coverage/<framework>/`），天然互斥 |
| Windows 路径兼容性问题（反斜杠 vs 正斜杠） | 路径匹配失败或产物未正确移动 | 低 | 代码中使用 `path.posix` 风格或显式归一化处理；executor 指令强调使用正斜杠规则 |
