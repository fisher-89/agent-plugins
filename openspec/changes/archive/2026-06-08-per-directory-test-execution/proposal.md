# 提案: per-directory-test-execution

> **变更**: per-directory-test-execution
> **日期**: 2026-06-08
> **状态**: 草稿

---

## 问题

当前的 unit-test-executor 存在两个效率问题：

1. **两阶段执行冗余**：Executor 先执行 `test_cmd`（步骤 3），再单独执行 `coverage_cmd`（步骤 4）。由于 `coverage_cmd` 本身就包含测试运行（如 `vp test --coverage`），这导致测试被重复运行，执行时间翻倍。

2. **全局根目录执行**：所有测试和覆盖率命令都从项目根目录运行，而非从框架对应的具体目录运行。但在实际项目中，不同目录可能使用不同的测试框架或需要不同的工作目录上下文。例如 `test.frameworks` 配置 `{glob: "plugins/dev-team/bin", framework: "vite-plus"}` 时，命令应当在其代码所在的 `plugins/dev-team/bin` 目录下执行，而非项目根目录。

此外，`test_detect_frameworks` 工具的输出只返回文件级别的框架归属信息，Executor 需要额外调用 `test_get_framework_config` 来获取命令配置，并自行推断应该在哪个目录执行。这使得 Executor 的流程冗余且容易出错。

---

## 提案

对 `test_detect_frameworks` 和 `unit-test-executor` 进行以下改造：

1. **test_detect_frameworks 输出扩展 `plan` 字段**：在原有 `detected` 和 `frameworks` 字段基础上，新增 `plan` 数组。`plan` 中的每个条目包含执行计划信息：工作目录（`directory`）、框架名（`framework`）、覆盖率命令（`coverage_cmd`）、覆盖率格式（`coverage_format`）、覆盖率输出路径（`coverage_output`）。Executor 直接使用 `plan` 即可执行，无需再额外调用 `test_get_framework_config`。

2. **引入 `deriveWorkingDirectory()` 辅助函数**：从 glob 模式中推导出命令执行的工作目录。推导规则：取 glob 模式中第一个通配符（`*`、`**`、`?`、`{`）之前的前缀路径。如果无通配符，整个 glob 就是工作目录。例如 `"plugins/dev-team/bin"` -> `"plugins/dev-team/bin"`，`"src/**/*.test.ts"` -> `"src"`，`"**/*.test.ts"` -> `"."`。

3. **合并 test + coverage 为单步执行**：Executor 始终运行 `coverage_cmd`（它包含测试运行），不再单独运行 `test_cmd`。单次执行，更快完成。

4. **Executor 逐目录执行**：Executor 使用 `plan` 中的 `directory` 字段，在每个框架对应的目录下执行覆盖率命令。命令在对应工作目录中运行，保证上下文正确。

---

## 能力

### 修改的能力

- **test-execution-diagnostics** — test_detect_frameworks 输出新增 `plan` 字段；unit-test-executor 合并步骤 3+4，使用 `plan` 逐目录执行覆盖率命令；新增 `deriveWorkingDirectory()` 辅助函数用于从 glob 推导工作目录

---

## 变更范围

### 实现以下特性

- `test-detect-frameworks.ts`：新增 `deriveWorkingDirectory()` 辅助函数，根据 glob 模式推导工作目录；扩展 `TestDetectFrameworksResult` 类型，新增 `plan` 字段；在 `runTestDetectFrameworks()` 中为每个匹配的框架生成执行计划条目
- `test-detect-frameworks.schema.ts`：扩展输出 schema，新增 `plan` 数组字段，每项包含 `directory`、`framework`、`coverage_cmd`、`coverage_format`、`coverage_output`
- `unit-test-executor.md`：删除步骤 3（单独运行 test_cmd）和步骤 4（单独运行 coverage_cmd）；新增合并步骤：使用 `plan` 逐目录执行 `coverage_cmd`；更新约束和场景描述以反映新流程
- 现有测试文件需要更新以覆盖 `plan` 输出字段

### 不要修改

- `test-get-framework-config.ts`：保持不变。`test_detect_frameworks` 内部调用 `runTestGetFrameworkConfig` 来获取命令配置，该工具仍然保留用于其他用途
- `config.json` 的 `test.frameworks` schema：保持不变，仍然使用 `{glob, framework}` 格式，无需新增字段
- `coverage-parser.ts` 和 `coverage-calculator.ts`：覆盖率解析和计算逻辑保持不变
- 报告 JSON schema 不变：`reports/unit-test-execution.json` 的字段结构不修改

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `test_detect_frameworks` 输出包含 `plan` 数组，每个条目包含 `directory`、`framework`、`coverage_cmd`、`coverage_format`、`coverage_output` | 对 `test_detect_frameworks({})` 返回的 JSON 进行 schema 校验，`plan` 字段须通过 Zod 验证 |
| AC-2 | `deriveWorkingDirectory` 对 `"plugins/dev-team/bin"` 返回 `"plugins/dev-team/bin"`（无通配符） | 单元测试验证 |
| AC-3 | `deriveWorkingDirectory` 对 `"src/**/*.test.ts"` 返回 `"src"`（取第一个通配符前路径） | 单元测试验证 |
| AC-4 | `deriveWorkingDirectory` 对 `"**/*.test.ts"` 返回 `"."`（通配符在起始位置） | 单元测试验证 |
| AC-5 | `deriveWorkingDirectory` 对 `"{src,lib}/*.test.ts"` 返回 `"."`（`{` 被视为通配符） | 单元测试验证 |
| AC-6 | `test_detect_frameworks` 在 `test.frameworks` 配置为 `[{glob: "plugins/dev-team/bin", framework: "vite-plus"}]` 时，`plan` 包含 `{directory: "plugins/dev-team/bin", framework: "vite-plus", coverage_cmd: "vp test --coverage", ...}` | 集成测试，验证 plan 内容与 test-get-framework-config 的注册表一致 |
| AC-7 | unit-test-executor 在检测到框架后，在每个框架对应的 `directory` 下执行 `coverage_cmd`（而非项目根目录） | 模拟执行场景，验证命令在正确的 `cwd` 下发起 |
| AC-8 | unit-test-executor 不再单独执行 `test_cmd`，只执行 `coverage_cmd`（单步执行即包含测试运行） | 审核 Executor agent 指南，确认步骤 3 和步骤 4 已合并 |
| AC-9 | 当 `test.frameworks` 配置为字符串简写时（如 `"vitest"`），`plan` 依然正确生成，`directory` 为 `"."`（因为默认 glob 为 `"**/*.{test,spec}.{js,ts,jsx,tsx}"`，第一个通配符前无路径前缀） | 集成测试验证 |
| AC-10 | 当 `test.frameworks` 未配置或为空数组时，`plan` 为空数组 | 集成测试验证 |
| AC-11 | 向后兼容：旧版 `detected` 和 `frameworks` 字段保持不变 | 现有单元测试全部通过 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `deriveWorkingDirectory` 对含嵌套通配符的复杂 glob 模式推导错误 | Executor 在错误目录下运行命令，导致测试失败或覆盖率数据丢失 | 低 | 实现时覆盖所有常见 glob 模式（含 `{}`、`**`、`*`、`?`）的单元测试；首次通配符前的纯路径前缀规则简单可预测 |
| Executor 直接从 `plan` 获取 `coverage_cmd`，可能错过框架注册表的更新 | Executor 使用过时的命令配置 | 低 | `plan` 在每次 `test_detect_frameworks` 调用时实时生成，总是从最新注册表获取命令；`test_get_framework_config` 保持为共享的单数据源 |
| 现有用户或脚本依赖 `test_detect_frameworks` 仅返回 `detected` 和 `frameworks` | 新增字段不影响现有消费方 | 极低 | `plan` 是新增的只读字段，不修改现有字段的含义或类型；所有现有测试继续通过 |
