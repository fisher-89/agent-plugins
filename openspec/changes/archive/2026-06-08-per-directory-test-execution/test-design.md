# 测试设计: per-directory-test-execution

> **变更**: per-directory-test-execution
> **日期**: 2026-06-08
> **基于**: proposal.md, specs/test-execution-diagnostics/spec.md

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `test_detect_frameworks` 输出包含 `plan` 数组，每个条目包含 `directory`、`framework`、`coverage_cmd`、`coverage_format`、`coverage_output` | 单元测试 | `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema — plan 字段 Zod schema 验证 |
| AC-2 | `deriveWorkingDirectory` 对 `"plugins/dev-team/bin"` 返回 `"plugins/dev-team/bin"`（无通配符） | 单元测试 | `test-detect-frameworks.test.ts` | deriveWorkingDirectory — 无通配符路径 |
| AC-3 | `deriveWorkingDirectory` 对 `"src/**/*.test.ts"` 返回 `"src"`（取第一个通配符前路径） | 单元测试 | `test-detect-frameworks.test.ts` | deriveWorkingDirectory — `**` 通配符 |
| AC-4 | `deriveWorkingDirectory` 对 `"**/*.test.ts"` 返回 `"."`（通配符在起始位置） | 单元测试 | `test-detect-frameworks.test.ts` | deriveWorkingDirectory — 通配符起始 |
| AC-5 | `deriveWorkingDirectory` 对 `"{src,lib}/*.test.ts"` 返回 `"."`（`{` 被视为通配符） | 单元测试 | `test-detect-frameworks.test.ts` | deriveWorkingDirectory — `{}` 通配符 |
| AC-6 | `test_detect_frameworks` 在 `test.frameworks` 配置为 `[{glob: "plugins/dev-team/bin", framework: "vite-plus"}]` 时，`plan` 包含 `{directory: "plugins/dev-team/bin", framework: "vite-plus", coverage_cmd: "vp test --coverage", ...}` | 集成测试 | `test-detect-frameworks.test.ts` | runTestDetectFrameworks — plan 内容正确性 |
| AC-7 | unit-test-executor 在每个框架对应的 `directory` 下执行 `coverage_cmd`（而非项目根目录） | 文档审核 | `unit-test-executor.md` | Agent 指南 — 步骤 3 包含逐目录执行 |
| AC-8 | unit-test-executor 不再单独执行 `test_cmd`，只执行 `coverage_cmd` | 文档审核 | `unit-test-executor.md` | Agent 指南 — 步骤 3+4 合并为单步 |
| AC-9 | 当 `test.frameworks` 配置为字符串简写时（如 `"vitest"`），`plan` 依然正确生成，`directory` 为 `"."` | 集成测试 | `test-detect-frameworks.test.ts` | runTestDetectFrameworks — 字符串简写 plan |
| AC-10 | 当 `test.frameworks` 未配置或为空数组时，`plan` 为空数组 | 集成测试 | `test-detect-frameworks.test.ts` | runTestDetectFrameworks — 空配置 plan |
| AC-11 | 向后兼容：旧版 `detected` 和 `frameworks` 字段保持不变 | 回归测试 | `test-detect-frameworks.test.ts` | runTestDetectFrameworks — 现有检测行为不变 |

---

## 单元测试

### 用例

#### deriveWorkingDirectory 纯函数测试

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 正向 | 无通配符路径: `"plugins/dev-team/bin"` → `"plugins/dev-team/bin"` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 正向 | `**` 通配符: `"src/**/*.test.ts"` → `"src"` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 正向 | 通配符起始: `"**/*.test.ts"` → `"."` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 正向 | `{}` 通配符: `"{src,lib}/*.test.ts"` → `"."` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 正向 | `?` 通配符: `"tests/?nit/*.test.ts"` → `"tests"` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 正向 | 多层 `*`: `"packages/*/src/__tests__/*.test.ts"` → `"packages"` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 正向 | Windows 反斜杠: `"plugins\\dev-team\\bin"` → `"plugins/dev-team/bin"` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 异常 | 空字符串 `""` → `""`（无通配符规则，返回自身） | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 异常 | `null` / `undefined` 输入（TypeScript 类型保护，运行时需防御） | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 边界 | 超长无通配符 glob: `"a".repeat(1000)` → 返回自身 | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 边界 | 超长含通配符 glob: `"a".repeat(500) + "/*/b"` → `"a...a"`（500 个 a） | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 边界 | 特殊字符路径: `"src/my test file/*.ts"` → `"src/my test file"` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 边界 | 纯通配符: `"*"` → `"."`（通配符在起始位置） | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 边界 | 以 `/` 分隔的通配符: `"src/**"` → `"src"` | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 边界 | 仅有点的路径: `"."` → `"."`（无通配符，返回自身） | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 边界 | 仅 `/` 的路径: `"/"` → `"/"`（无通配符，返回自身） | 新增 |
| `test-detect-frameworks.test.ts` | deriveWorkingDirectory | 边界 | 含连续分隔符: `"src//lib/**/*.ts"` → 连续分隔符应合并 → `"src/lib"` | 新增 |

#### testDetectFrameworksOutputSchema plan 字段 schema 验证

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 正向 | 完整 plan 数组通过 schema 验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 正向 | plan 数组包含多个条目 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 正向 | plan 为空数组 [] | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 异常 | plan 条目缺少 `directory` 字段 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 异常 | plan 条目缺少 `framework` 字段 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 异常 | plan 条目缺少 `coverage_cmd` 字段 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 异常 | plan 条目 `directory` 为 null | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 异常 | plan 条目的 `coverage_format` 为非法枚举值（非 istanbul/llvm-cov） | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 异常 | plan 条目类型为数组而非对象 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 边界 | plan 条目 `coverage_format` 字段大小写敏感测试（"istanbul" 通过，"Istanbul" 拒绝） | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 边界 | plan 条目包含多余未知字段（应通过 passthrough） | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 边界 | 完整输出对象（detected + frameworks + plan）通过验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema | 异常 | plan 非数组类型（如字符串） | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `test-detect-frameworks.schema.test.ts` | 无 | 不依赖 Mock，直接调用 Zod schema 的 `.safeParse()` | 所有 schema 验证场景 |
| `test-detect-frameworks.test.ts` — deriveWorkingDirectory | 无 | 纯函数，无 Mock 需求 | 所有 deriveWorkingDirectory 测试 |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-6 | `test-detect-frameworks.test.ts` | 单框架单目录 plan 正确性 | 配置 `[{glob: "plugins/dev-team/bin", framework: "vite-plus"}]`，验证 plan 包含 directory、coverage_cmd、coverage_format、coverage_output 映射正确 | 新增 |
| AC-6 | `test-detect-frameworks.test.ts` | 多框架多目录 plan | 配置 `[{glob: "plugins/dev-team/bin", framework: "vite-plus"}, {glob: "src/**/*.test.ts", framework: "vitest"}]`，验证 plan 有两条记录，directory 分别为 `"plugins/dev-team/bin"` 和 `"src"` | 新增 |
| AC-9 | `test-detect-frameworks.test.ts` | 字符串简写 "vitest" plan | 配置 `frameworks: "vitest"`，验证 plan 包含 `{directory: ".", framework: "vitest", coverage_cmd: "npx vitest run --coverage", coverage_format: "istanbul", coverage_output: "coverage/coverage-summary.json"}` | 新增 |
| AC-9 | `test-detect-frameworks.test.ts` | 字符串简写 "jest" plan | 配置 `frameworks: "jest"`，验证 plan directory 为 `"."`，coverage_cmd 为 `"npx jest --coverage"` | 新增 |
| AC-9 | `test-detect-frameworks.test.ts` | 字符串简写 "rust" plan | 配置 `frameworks: "rust"`，验证 plan directory 为 `"."`（默认 glob `"**/tests/**/*.rs"` 通配符起始），coverage_format 为 `"llvm-cov"` | 新增 |
| AC-10 | `test-detect-frameworks.test.ts` | test.frameworks 为空数组 | 配置 `frameworks: []`，验证 `plan` 为 `[]`，`detected` 均为 `"unknown"` | 新增 |
| AC-10 | `test-detect-frameworks.test.ts` | 无 test.frameworks 配置 | 配置中不包含 `test.frameworks` 字段，验证 `plan` 为 `[]` | 新增 |
| AC-11 | `test-detect-frameworks.test.ts` | detected 字段不变 | 与现有测试相同的输入，验证 `detected` 数组结构和内容与修改前一致 | 回归 |
| AC-11 | `test-detect-frameworks.test.ts` | frameworks 字段不变 | 与现有测试相同的输入，验证 `frameworks` 数组结构和内容与修改前一致 | 回归 |
| AC-11 | `test-detect-frameworks.test.ts` | 首匹配规则不变 | 文件匹配多个 glob 时，仍按 `test.frameworks` 数组顺序首匹配 | 回归 |
| AC-11 | `test-detect-frameworks.test.ts` | 无匹配文件返回 "unknown" | 不匹配任何 glob 的文件仍返回 `framework: "unknown"` | 回归 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `test-detect-frameworks.test.ts` — plan 内容 | `readConfig` | 使用临时目录 + openspec/config.json（已有模式 `createTempProject`）注入配置；`runTestGetFrameworkConfig` 不 Mock，使用真实注册表 | 所有 runTestDetectFrameworks 集成测试 |
| `test-detect-frameworks.test.ts` — plan 内容 | `collectFiles`（自动扫描） | 使用临时目录创建实际文件 + openspec/config.json；auto-scan 时确保目录中有匹配和不匹配的文件 | 自动扫描场景 |
| `test-detect-frameworks.test.ts` — 回归 | `readConfig` | 与现有测试相同的 `createTempProject` 模式，无需新增 Mock | 所有回归场景 |

---

## 文档审核

以下 AC 不通过自动化测试验证，而通过人工审核 `unit-test-executor.md` 文档内容确认：

| AC ID | 审核项目 | 审核方法 |
|--------|---------|----------|
| AC-7 | Executor 在每个框架对应的 `directory` 下执行 `coverage_cmd` | 确认步骤 3（新）中描述为"遍历 `plan` 条目，在 `directory` 下执行 `coverage_cmd`"，且无全局根目录执行描述 |
| AC-8 | Executor 不再单独执行 `test_cmd` | 确认原步骤 3（test_cmd 执行）和步骤 4（coverage_cmd 执行）已合并为单步；确认 `test_get_framework_config` 不再被步骤引用；确认 `test_cmd` 仅保留在注册表中供其他消费者使用 |

---

## 不可测试项

- `unit-test-executor.md` Agent 指南的运行时行为 — **原因**: Agent 指南是 Markdown 文档，定义 LLM 的行为流程而非可执行代码。其正确性需要通过人工文档审核（AC-7、AC-8）和端到端集成测试（AC-6 验证 plan 数据正确性后，Executor 的消费行为由文档约束保证）来验证。无法编写自动化测试直接验证 LLM 是否按文档执行。
- `test-get-framework-config.ts` — **原因**: 该文件保持不变（在变更范围"不要修改"中明确），不新增测试需求。现有测试全部通过即证明无回归。

---

## 参数类型系统性边界映射 — deriveWorkingDirectory

| 参数情况 | 输入示例 | 预期输出 | 边界类别 |
|---------|---------|----------|----------|
| 正常路径（无通配符） | `"plugins/dev-team/bin"` | `"plugins/dev-team/bin"` | 正常 |
| 正常路径（`*` 通配符） | `"packages/*/src/*.ts"` | `"packages"` | 正常 |
| 空字符串 | `""` | `""`（无通配符，返回自身） | 边界 |
| 超长字符串（>1000 字符，无通配符） | `"x".repeat(1000)` | `"x".repeat(1000)` | 边界 |
| 超长字符串（>1000 字符，含通配符） | `"a".repeat(500) + "/*/b"` | `"a".repeat(500)` | 边界 |
| 纯通配符 | `"*"` | `"."` | 边界 |
| 特殊字符（空格） | `"my project/src/*.ts"` | `"my project/src"` | 边界 |
| 特殊字符（Unicode/中文） | `"源代码/**/*.ts"` | `"源代码"` | 边界 |
| 特殊字符（emoji） | `"src/🚀-test/**/*.ts"` | `"src/🚀-test"` | 边界 |
| 特殊字符（换行符） | `"src\nlib/*.ts"` | `"src\nlib"` | 边界 |
| Windows 反斜杠 | `"plugins\\dev-team\\bin"` | `"plugins/dev-team/bin"` | 边界 |
| 连续分隔符 | `"src//lib/**/*.ts"` | `"src/lib"` | 边界 |
| null/undefined（运行时防御） | `null as unknown as string` | 抛出 TypeError 或返回 `"."`（取决于实现决策） | 异常 |
| 仅有点 | `"."` | `"."` | 边界 |

## 参数类型系统性边界映射 — testDetectFrameworksOutputSchema.plan

| 参数情况 | 输入示例 | 预期结果 | 边界类别 |
|---------|---------|----------|----------|
| 正常 plan 数组 | `[{directory: "src", framework: "vitest", coverage_cmd: "npx vitest run --coverage", coverage_format: "istanbul", coverage_output: "coverage/coverage-summary.json"}]` | 验证通过 | 正常 |
| plan 空数组 | `[]` | 验证通过 | 边界（空列表） |
| plan 缺失（undefined） | 输出对象中无 `plan` 字段 | 验证失败 | 边界 |
| plan 为 null | `null` | 验证失败 | 边界 |
| plan 为非数组 | `"string"` | 验证失败 | 边界 |
| plan 条目缺少 directory | `[{framework: "vitest", ...}]` | 验证失败 | 边界（缺失必填字段） |
| plan 条目 directory 为空字符串 | `[{directory: "", ...}]` | 验证通过（string） | 边界 |
| plan 条目 coverage_format 非法 | `"cobertura"` | 验证失败 | 异常 |
| plan 条目多余字段 | `[{..., extra: "value"}]` | 验证通过（passthrough） | 边界 |
