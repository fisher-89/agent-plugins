# 提案: test-detect-script-generation

> **变更**: test-detect-script-generation
> **日期**: 2026-06-09
> **状态**: 草稿

---

## 问题

per-directory-test-execution 和 unified-coverage-artifacts 实现后，`test_detect_frameworks` 的 `plan` 数组已包含每个框架的完整执行信息：工作目录（`directory`）、覆盖率命令（`coverage_cmd`）、产物清理列表（`coverage_cleanup`）。然而，Executor（AI agent）目前需要自行根据这些分散的字段手动拼装 bash 执行脚本。

当前存在以下两个问题：

1. **重复劳动**：Executor 每次都需要将 `directory`、`coverage_cleanup`、`coverage_cmd` 三个字段组合成类似 `cd <dir> && rm -rf <cleanup> && <coverage_cmd>` 的脚本模板。这一过程在每个 PlanEntry 上重复，消耗 AI 上下文资源且容易出错。
2. **不一致性风险**：不同 Executor 会话可能生成不同风格的执行脚本（如是否包含 `set -e`、`cd` 的处理方式、`rm -rf` 的顺序等），导致测试执行行为不可预测。

PlanEntry 已包含所有必要数据，自动生成脚本的时机成熟。

---

## 提案

在 PlanEntry 中新增 `script` 字段，由 `runTestDetectFrameworks` 在生成 plan 时同步生成。同时新增 `generateScript()` 纯函数，将脚本生成逻辑集中管理。

具体改造方案：

1. **PlanEntry 接口新增 `script` 字段**：类型为 `string`，内容为完整的 bash shell 脚本。
2. **实现 `generateScript()` 函数**：根据 PlanEntry 的 `directory`、`coverage_cleanup`、`coverage_cmd` 生成 bash 脚本。脚本模板如下：

```bash
#!/bin/bash
set -e
cd <directory>            # 仅当 directory != "." 时
rm -rf <cleanup[0]>
rm -rf <cleanup[1]>
<coverage_cmd>
```

3. **Zod 输出 schema 同步扩展**：`testDetectFrameworksOutputSchema` 的 plan 条目新增必填 `script` 字段。
4. **单元测试覆盖**：为 `generateScript()` 编写纯函数测试，验证各框架的脚本内容正确性。

所有数据来源已存在于 FRAMEWORK_REGISTRY 和 PlanEntry 中，无新增依赖。

---

## 能力

### 修改的能力

- **test-execution-diagnostics** — PlanEntry 接口新增 `script` 字段；`test_detect_frameworks` MCP 工具的输出 schema 同步扩展；`runTestDetectFrameworks` 生成 plan 时自动调用 `generateScript()` 填充 script 字段

### 新增能力

- **无** — `generateScript()` 作为 `test-execution-diagnostics` 能力下的新增需求，不独立为单独的能力

---

## 变更范围

### 实现以下特性

- `test-detect-frameworks.ts`：PlanEntry 接口新增 `script: string`；实现并导出 `generateScript(entry: { directory, coverage_cleanup, coverage_cmd }): string` 函数；`runTestDetectFrameworks` 在生成 plan 条目时调用 `generateScript()` 填充 `script`
- `test-detect-frameworks.schema.ts`：plan 条目 Zod 对象新增 `script: z.string()` 必填字段
- `test-detect-frameworks.test.ts`：为 `generateScript()` 编写单元测试，覆盖：
  - 各框架（jest/vitest/vite-plus/bun/rust）生成正确的脚本内容
  - `cd <directory>` 仅在 `directory != "."` 时生成
  - 清理步骤包含全部 `coverage_cleanup` 条目
  - `coverage_cleanup` 为空数组时无 `rm -rf` 行
  - plan 条目的 `script` 字段为必填非空字符串

### 不要修改

- `test-get-framework-config.ts` 和 FRAMEWORK_REGISTRY — 无需修改
- `test-get-framework-config.schema.ts` — `test_get_framework_config` 工具不涉及 `script` 字段
- Executor Agent 行为 — 本变更仅在 plan 中生成脚本，不改变 Executor 如何使用脚本
- PowerShell 脚本生成 — 仅支持 bash
- `deriveWorkingDirectory` 函数逻辑和测试
- `detected` 和 `frameworks` 字段的结构和内容
- MCP 工具注册代码（`mcp.ts`）

### Working Rules

- **test-design 阶段文档不写入测试命令**: test-design.md 中不应包含具体测试命令（如 `npx jest --coverage`、`npx vitest run` 等）。设计文档应专注于测试策略、覆盖范围、测试场景，测试命令由 MCP 工具 `test_detect_frameworks` 和 `test_get_framework_config` 在运行时生成

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `generateScript()` 为 vitest 框架生成包含 `npx vitest run --coverage` 和 `rm -rf coverage .nyc_output` 的 bash 脚本 | 单元测试，验证脚本字符串内容 |
| AC-2 | `generateScript()` 生成的脚本以 `#!/bin/bash` 和 `set -e` 开头 | 单元测试，正则匹配脚本前两行 |
| AC-3 | 当 `directory` 为 `"."` 时，生成的脚本不含 `cd` 行 | 单元测试 |
| AC-4 | 当 `directory` 为 `"plugins/dev-team/bin"` 时，生成的脚本包含 `cd plugins/dev-team/bin` | 单元测试 |
| AC-5 | `generateScript()` 为 rust 框架生成包含 `cargo llvm-cov --all --coverage`、`rm -rf coverage` 和 `rm -rf target/llvm-cov` 的脚本 | 单元测试 |
| AC-6 | `generateScript()` 在 `coverage_cleanup` 为空数组时不含任何 `rm -rf` 行 | 单元测试 |
| AC-7 | `test_detect_frameworks` 输出的 plan 条目 `script` 字段为非空字符串 | Zod schema 校验 |
| AC-8 | `test_detect_frameworks` 输出的 plan 条目 `script` 字段为必填（不可缺失不可为 null） | Zod schema strict 校验 |
| AC-9 | PlanEntry 接口 `script` 字段类型为 `string`（非可选） | TypeScript 编译检查 |
| AC-10 | 使用字符串简写配置（如 `"vitest"`）时，plan 条目同样包含 `script` 非空字段 | 单元测试验证 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 脚本模板与 Executor 期望的执行方式不一致 | Executor 使用生成的脚本但执行结果不符合预期 | 低 | `generateScript()` 使用的 `coverage_cmd` 和 `coverage_cleanup` 与 Executor 原本手动拼装的数据完全一致，无行为差异 |
| `cd <directory>` 在目录不存在时导致脚本失败 | 脚本以非零退出码终止 | 低 | 脚本使用 `set -e` 确保失败显式暴露，与预期行为一致；目录不存在属于配置错误，应尽早暴露 |
| `rm -rf` 删除超出预期的文件 | 意外删除用户文件 | 低 | 清理路径来自 FRAMEWORK_REGISTRY 的硬编码配置（`coverage`、`.nyc_output`、`target/llvm-cov`），经过审核的已知临时目录名 |
| 仅支持 bash 限制 Windows 用户体验 | Windows 用户无法直接执行生成的脚本 | 中 | 明确设计决策仅支持 bash；Windows 用户可使用 WSL 或 Git Bash 执行。PowerShell 支持可延迟到后续变更 |
