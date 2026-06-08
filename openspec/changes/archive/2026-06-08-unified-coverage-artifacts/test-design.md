# 测试设计: unified-coverage-artifacts

> **变更**: unified-coverage-artifacts
> **日期**: 2026-06-08
> **基于**: proposal.md, design.md

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `test_get_framework_config("vitest")` 返回的配置包含 `coverage_artifacts: ["coverage/**"]` 和 `coverage_cleanup: ["coverage", ".nyc_output"]` | 单元测试 | `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — vitest 新增字段 |
| AC-2 | `test_get_framework_config("rust")` 返回的配置中 `coverage_artifacts` 包含 `coverage/**` 和 `target/llvm-cov/**`，`coverage_cleanup` 包含 `coverage` 和 `target/llvm-cov` | 单元测试 | `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — rust 新增字段 |
| AC-3 | 全部五个框架（jest/vitest/vite-plus/bun/rust）的 FrameworkConfig 都包含 `coverage_artifacts` 和 `coverage_cleanup` 非空数组 | 单元测试 | `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 遍历全部框架 |
| AC-4 | `test_detect_frameworks` 输出的 `plan` 条目包含 `coverage_artifacts` 和 `coverage_cleanup` 字段 | 单元测试 | `test-detect-frameworks.test.ts` + `test-detect-frameworks.schema.test.ts` | runTestDetectFrameworks — plan 新增字段；schema — plan 条目 schema |
| AC-5 | `test_get_framework_config` 的 Zod output schema 允许 `coverage_artifacts` 和 `coverage_cleanup` 为可选字段（向后兼容） | 单元测试 | `test-get-framework-config.schema.test.ts` (新建) + `test-detect-frameworks.schema.test.ts` | output schema 向后兼容 — 无新字段时通过解析 |
| AC-6 | unit-test-executor 在覆盖率命令执行后、解析前，将产物从框架工作目录移动到 `reports/coverage/<framework>/` | 集成测试 | Agent 模拟场景 | Executor 行为 — 移动产物步骤 |
| AC-7 | 移动后 `coverage_by_framework` 的 `html_report` 路径更新为统一位置下的路径（如 `reports/coverage/vitest/index.html`） | 集成测试 | Agent 模拟场景 | Executor 行为 — 路径更新 |
| AC-8 | 移动成功后，原始 `coverage/`、`.nyc_output/` 等目录被删除 | 集成测试 | Agent 模拟场景 | Executor 行为 — 清理原始目录 |
| AC-9 | 移动失败时（源目录不存在），保留原始状态，不执行清理 | 集成测试 | Agent 模拟场景 | Executor 行为 — 容错处理 |
| AC-10 | 当 `coverage_artifacts` 为空数组时，跳过移动步骤 | 集成测试 | Agent 模拟场景 | Executor 行为 — 空配置跳过 |
| AC-11 | `coverage_output` 在移动后指向统一位置（如 `reports/coverage/vitest/coverage-summary.json`），用于后续覆盖率解析 | 集成测试 | Agent 模拟场景 | Executor 行为 — 解析路径更新 |

---

## 单元测试

### 用例

#### AC-1, AC-2, AC-3: test-get-framework-config 新增字段

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — vitest 新增字段 | 正向 | vitest 框架返回 `coverage_artifacts` 包含 `["coverage/**"]`，`coverage_cleanup` 包含 `["coverage", ".nyc_output"]` | 新增 |
| `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — rust 新增字段 | 正向 | rust 框架返回 `coverage_artifacts` 包含 `["coverage/**", "target/llvm-cov/**"]`，`coverage_cleanup` 包含 `["coverage", "target/llvm-cov"]` | 新增 |
| `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — jest 新增字段 | 正向 | jest 框架返回 `coverage_artifacts` 为 `["coverage/**"]`，`coverage_cleanup` 为 `["coverage", ".nyc_output"]` | 新增 |
| `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — vite-plus 新增字段 | 正向 | vite-plus 框架返回 `coverage_artifacts` 为 `["coverage/**"]`，`coverage_cleanup` 为 `["coverage", ".nyc_output"]` | 新增 |
| `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — bun 新增字段 | 正向 | bun 框架返回 `coverage_artifacts` 为 `["coverage/**"]`，`coverage_cleanup` 为 `["coverage"]`（无 `.nyc_output`） | 新增 |
| `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 全部框架非空 | 正向 | 遍历所有五个框架，验证每个框架的 `coverage_artifacts` 和 `coverage_cleanup` 均为非空数组 | 新增 |
| `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 全部框架 `coverage_artifacts` 数组字段类型 | 边界 | 遍历全部框架，验证 `coverage_artifacts` 和 `coverage_cleanup` 均为 `string[]` 类型，元素为非空字符串 | 新增 |
| `test-get-framework-config.test.ts` | runTestGetFrameworkConfig — 返回对象结构完整性 | 正向 | 全部框架返回的配置包含全部字段（原有 5 字段 + 新增 2 字段 = 7 字段） | 新增 |

#### AC-4: PlanEntry 新增字段

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `test-detect-frameworks.test.ts` | runTestDetectFrameworks — plan 条目包含新增字段 | 正向 | 配置 vitest 框架时，plan 条目包含 `coverage_artifacts` 和 `coverage_cleanup` 字段且值等于框架注册表中的值 | 新增 |
| `test-detect-frameworks.test.ts` | runTestDetectFrameworks — 多框架 plan 条目新增字段 | 正向 | 配置 [vitest, rust] 多框架时，每个 plan 条目都包含对应的 `coverage_artifacts` 和 `coverage_cleanup` | 新增 |
| `test-detect-frameworks.test.ts` | runTestDetectFrameworks — 字符串简写 plan 新增字段 | 正向 | 字符串简写 `"vitest"` 时，plan 条目也包含 `coverage_artifacts` 和 `coverage_cleanup` | 新增 |
| `test-detect-frameworks.test.ts` | runTestDetectFrameworks — rust plan 字段验证 | 正向 | rust 框架的 plan 条目中 `coverage_artifacts` 为 `["coverage/**", "target/llvm-cov/**"]`，`coverage_cleanup` 为 `["coverage", "target/llvm-cov"]` | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan 条目含新字段通过验证 | 正向 | 包含 `coverage_artifacts` 和 `coverage_cleanup` 两个新字段的 plan 条目通过 schema 验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan 条目新字段为字符串数组 | 正向 | 新字段值为非空字符串数组时通过 schema 验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan 条目新字段为空数组 | 边界 | `coverage_artifacts: []` 和 `coverage_cleanup: []` 通过 schema 验证 | 新增 |

#### AC-5: Schema 向后兼容

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `test-get-framework-config.schema.test.ts` (新建) | testGetFrameworkConfigOutputSchema — 无新字段向后兼容 | 正向 | 输出对象无 `coverage_artifacts` 和 `coverage_cleanup` 字段时 schema 验证通过 | 新增 |
| `test-get-framework-config.schema.test.ts` (新建) | testGetFrameworkConfigOutputSchema — 新字段存在时通过 | 正向 | 输出对象包含 `coverage_artifacts: ["coverage/**"]` 和 `coverage_cleanup: ["coverage"]` 时 schema 验证通过 | 新增 |
| `test-get-framework-config.schema.test.ts` (新建) | testGetFrameworkConfigOutputSchema — 新字段为 undefined | 边界 | 新字段显式设置为 `undefined` 时 schema 验证通过 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan 无新字段向后兼容 | 正向 | plan 条目中无 `coverage_artifacts` 和 `coverage_cleanup` 时 schema 验证通过 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan 新字段与旧字段共存 | 正向 | plan 条目包含新旧全部字段通过验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan 条目多余字段 passthrough | 边界 | plan 条目含除新字段外的未知多余字段仍通过验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — 完整输出含新字段 | 正向 | detected + frameworks + plan（含新字段）的完整输出通过验证 | 新增 |

#### 参数边界测试

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — coverage_artifacts 为 null | 异常 | plan 条目 `coverage_artifacts` 为 `null` 时 schema 拒绝 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — coverage_cleanup 为非数组类型 | 异常 | plan 条目 `coverage_cleanup` 为字符串时 schema 拒绝 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan 条目新字段含非字符串元素 | 异常 | `coverage_artifacts` 包含数字元素时 schema 拒绝 | 新增 |
| `test-get-framework-config.schema.test.ts` (新建) | testGetFrameworkConfigOutputSchema — coverage_artifacts 非法类型 | 异常 | `coverage_artifacts` 为字符串而非数组时 schema 拒绝 | 新增 |
| `test-get-framework-config.schema.test.ts` (新建) | testGetFrameworkConfigOutputSchema — coverage_cleanup 含非法元素 | 异常 | `coverage_cleanup` 包含 `null` 元素时 schema 拒绝 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `test-get-framework-config.test.ts` | 无 | Framework 注册表为硬编码常量，不 Mock。直接调用 `runTestGetFrameworkConfig` 验证返回值 | 所有 AC-1/2/3 测试 |
| `test-detect-frameworks.test.ts` — plan 新增字段 | `readConfig` | 使用 `createTempProject` 模式（已有 fixture），在临时目录创建 `openspec/config.json` 注入配置；`runTestGetFrameworkConfig` 使用真实注册表 | 所有 plan 内容验证 |
| `test-get-framework-config.schema.test.ts` (新建) | 无 | 直接调用 `testGetFrameworkConfigOutputSchema.safeParse()`，不依赖外部依赖 | 所有 schema 验证 |
| `test-detect-frameworks.schema.test.ts` | 无 | 直接调用 `testDetectFrameworksOutputSchema.safeParse()`，不依赖外部依赖 | 所有 schema 验证 |

---

## 集成测试

### 用例

| AC ID | 测试文件/场景 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-6 | Agent 模拟场景 | 覆盖率产物移动到统一目录 | 在临时项目目录中模拟覆盖率执行产物生成，验证 Executor 将产物从框架工作目录移动到 `reports/coverage/<framework>/` 下 | 新增 |
| AC-7 | Agent 模拟场景 | html_report 路径更新 | 产物移动后，验证 `coverage_by_framework` 的 `html_report` 路径值为 `reports/coverage/vitest/index.html`（原为 `coverage/index.html`） | 新增 |
| AC-8 | Agent 模拟场景 | 清理原始临时目录 | 产物移动成功后，验证原始目录（如 `coverage/`、`.nyc_output/`、`target/llvm-cov/`）已被删除 | 新增 |
| AC-9 | Agent 模拟场景 | 移动失败保留原始状态 | 覆盖率命令未生成产物（源路径不存在），验证不清除、不抛出异常、findings 字段记录错误信息 | 新增 |
| AC-10 | Agent 模拟场景 | 空 coverage_artifacts 跳过移动 | 构造 `coverage_artifacts: []` 的场景，验证无移动操作执行，原始产物留在原位置 | 新增 |
| AC-11 | Agent 模拟场景 | coverage_output 路径更新 | 移动后验证解析器从统一位置 `reports/coverage/vitest/coverage-summary.json` 而非原路径读取 | 新增 |
| AC-6 | Agent 模拟场景 | 多框架产物隔离 | vitest 和 rust 两个框架同时执行，验证产物分别进入 `reports/coverage/vitest/` 和 `reports/coverage/rust/`，互不覆盖 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| Agent 模拟场景 | `node:fs` 文件系统操作 | 在临时项目目录中创建/移动/删除文件来模拟 Executor 行为。不需要 Mock，直接操作临时目录的物理文件 | 移动、删除、清理验证 |
| Agent 模拟场景 | 覆盖率命令执行 | 不执行真实覆盖率命令，在框架工作目录下手动创建 `coverage/coverage-summary.json` 和 `coverage/index.html` 等产物文件来模拟命令执行结果 | 模拟产物生成 |
| Agent 模拟场景 | `os.tmpdir` 临时目录 | 使用 `fs.mkdtempSync` 在每个测试开始时创建临时项目目录，测试结束时通过 `fs.rmSync` 清理 | 所有集成测试 |

### 集成测试场景详细描述

**场景 1: 单框架产物移动 (AC-6, AC-7, AC-8, AC-11)**
- 准备工作: 在临时目录中创建 `openspec/changes/<change>/` 结构。在框架工作目录下创建 `coverage/coverage-summary.json`、`coverage/index.html`、`.nyc_output/some.tmp`。
- 执行: 模拟 Executor 的步骤 3 操作——创建 `reports/coverage/vitest/` 目录，从 `coverage/**` 匹配并移动文件到目标目录。
- 验证: `reports/coverage/vitest/coverage-summary.json` 存在，`reports/coverage/vitest/index.html` 存在；原始 `coverage/` 目录和 `.nyc_output/` 目录不存在；`coverage_output` 和 `html_report` 路径指向统一位置。

**场景 2: 移动失败容错 (AC-9)**
- 准备工作: 框架工作目录下无 `coverage/` 目录（模拟覆盖率命令未生成产物）。
- 执行: 模拟 Executor 尝试移动 `coverage/**`。
- 验证: 无异常抛出（非阻断）；不创建 `reports/coverage/vitest/` 目录；不清除任何文件；错误信息记录在 findings 字段中。

**场景 3: 空配置跳过 (AC-10)**
- 准备工作: `coverage_artifacts` 为空数组 `[]`，框架工作目录下有 `coverage/` 产物。
- 执行: 模拟 Executor 检查 `coverage_artifacts` 为空后跳过移动步骤。
- 验证: `coverage/` 产物保留在原位置；`reports/coverage/` 目录不存在或为空；`coverage_output` 不更新。

**场景 4: 多框架隔离 (AC-6)**
- 准备工作: 项目使用 vitest 和 rust 两个框架。vitest 工作目录下有 `coverage/` 产物；rust 工作目录下有 `coverage/` 和 `target/llvm-cov/` 产物。
- 执行: 模拟 Executor 遍历 plan 数组，分别处理 vitest 和 rust 的产物移动。
- 验证: vitest 产物在 `reports/coverage/vitest/` 下；rust 产物在 `reports/coverage/rust/` 下；rust 的 `target/llvm-cov/` 被清理而 vitest 的 `.nyc_output` 被清理。

---

## 文档审核

以下 AC 不通过自动化测试验证，而通过人工审核 `unit-test-executor.md` 文档内容确认：

| AC ID | 审核项目 | 审核方法 |
|--------|---------|----------|
| AC-6 | Executor 指南在步骤 2 和步骤 3 之间插入了"移动覆盖率产物"步骤 | 确认文档中新步骤 3 包含移动逻辑描述，且位置在覆盖率执行步骤之后、覆盖率解析步骤之前 |
| AC-7 | 移动后 `html_report` 写入路径更新为统一位置 | 确认文档中步骤 3 的子步骤描述包含路径更新说明，步骤 5 中 `html_report` 路径指向统一位置 |
| AC-8 | 移动成功后执行 `coverage_cleanup` 清理 | 确认文档中包含移动成功后删除 `coverage_cleanup` 中声明的原始目录的描述 |
| AC-9 | 移动失败时保留原始状态、不执行清理、记录 findings | 确认文档中包含 `coverage_artifacts` 源路径检查、失败时 findings 记录、不清除的约束描述 |
| AC-10 | `coverage_artifacts` 为空数组时跳过移动步骤 | 确认文档中包含空数组检查的决策逻辑描述 |
| AC-11 | `coverage_output` 路径在移动后更新为指向统一位置 | 确认文档中步骤 3 之后覆盖率解析步骤使用更新后的路径 |

---

## 不可测试项

- `unit-test-executor.md` Agent 指南的运行时行为 — **原因**: Agent 指南是 Markdown 文档，定义 LLM 的行为流程而非可执行代码。其正确性需要通过人工文档审核和端到端集成测试（场景 1-4）来验证。无法编写自动化测试直接验证 LLM 是否按文档执行。
- `coverage-parser.ts` 和 `coverage-calculator.ts` 的解析和计算逻辑 — **原因**: 这两个模块在变更范围中明确标记为"不要修改"（proposal.md 第 73 行），不新增测试需求。现有测试全部通过即证明无回归。覆盖率数据源路径的变化由 Executor 在解析前更新 `coverage_output` 路径，解析器逻辑本身保持不变。
- `integration-test-executor.md` — **原因**: 该文件在变更范围中明确标记为"暂不引入覆盖率产物能力"（proposal.md 第 74 行）。
- `openspec/config.json` 配置层 — **原因**: 在变更范围中明确标记为"框架命令注册表保持硬编码"（proposal.md 第 75 行）。

---

## 参数类型系统性边界映射

### Schema 字段边界

| 参数情况 | 输入示例 | 预期结果 | 边界类别 |
|---------|---------|----------|----------|
| 正常: coverage_artifacts 为字符串数组 | `["coverage/**"]` | schema 通过 | 正常 |
| 正常: coverage_cleanup 为字符串数组 | `["coverage", ".nyc_output"]` | schema 通过 | 正常 |
| 边界: coverage_artifacts 为空数组 | `[]` | schema 通过 | 边界（空列表） |
| 边界: coverage_cleanup 为单元素数组 | `["coverage"]` | schema 通过 | 边界（单元素） |
| 边界: coverage_artifacts 超大字符串元素 | `["x".repeat(2000)]` | schema 通过（string 类型） | 边界（超长字符串） |
| 边界: coverage_artifacts 可选字段缺失 | 输出对象中无此字段 | schema 通过（optional） | 边界（向后兼容） |
| 边界: coverage_cleanup 可选字段为 undefined | `coverage_cleanup: undefined` | schema 通过（optional） | 边界 |
| 异常: coverage_artifacts 为 null | `null` | schema 拒绝 | 异常 |
| 异常: coverage_artifacts 为字符串（非数组） | `"coverage/**"` | schema 拒绝 | 异常 |
| 异常: coverage_artifacts 含数字元素 | `["coverage/**", 123]` | schema 拒绝 | 异常 |
| 异常: coverage_cleanup 含 null 元素 | `["coverage", null]` | schema 拒绝 | 异常 |
| 异常: coverage_cleanup 为对象 | `{}` | schema 拒绝 | 异常 |

### FrameworkConfig 的 coverage_artifacts/coverage_cleanup 字段边界

| 参数情况 | 框架 | 预期 `coverage_artifacts` | 预期 `coverage_cleanup` | 边界类别 |
|---------|------|--------------------------|------------------------|----------|
| 正常: istanbul 框架（jest） | jest | `["coverage/**"]` | `["coverage", ".nyc_output"]` | 正常 |
| 正常: istanbul 框架（vitest） | vitest | `["coverage/**"]` | `["coverage", ".nyc_output"]` | 正常 |
| 正常: istanbul 框架（vite-plus） | vite-plus | `["coverage/**"]` | `["coverage", ".nyc_output"]` | 正常 |
| 正常: istanbul 框架（bun） | bun | `["coverage/**"]` | `["coverage"]` | 正常（无 `.nyc_output`） |
| 正常: llvm-cov 框架（rust） | rust | `["coverage/**", "target/llvm-cov/**"]` | `["coverage", "target/llvm-cov"]` | 正常（双产物） |
| 边界: 无 `.nyc_output` 的框架（bun） | bun | `["coverage/**"]` | `["coverage"]` | 边界（cleanup 只有 1 个） |
| 边界: 双制品框架（rust） | rust | 2 个 glob 模式 | 2 个清理目标 | 边界（多产物） |
