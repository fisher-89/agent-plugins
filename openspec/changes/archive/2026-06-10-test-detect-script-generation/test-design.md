# 测试设计: test-detect-script-generation

> **日期**: 2026-06-09

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `generateScript()` 为 vitest 框架生成包含 `npx vitest run --coverage` 和 `rm -rf coverage .nyc_output` 的 bash 脚本 | 单元测试 | `test-detect-frameworks.test.ts` | `generateScript()` -- 各框架正向测试 |
| AC-2 | `generateScript()` 生成的脚本以 `#!/bin/bash` 和 `set -e` 开头 | 单元测试 | `test-detect-frameworks.test.ts` | `generateScript()` -- 脚本结构测试 |
| AC-3 | 当 `directory` 为 `"."` 时，生成的脚本不含 `cd` 行 | 单元测试 | `test-detect-frameworks.test.ts` | `generateScript()` -- cd 行为边界测试 |
| AC-4 | 当 `directory` 为 `"plugins/dev-team/bin"` 时，生成的脚本包含 `cd plugins/dev-team/bin` | 单元测试 | `test-detect-frameworks.test.ts` | `generateScript()` -- 各框架正向测试 |
| AC-5 | `generateScript()` 为 rust 框架生成包含 `cargo llvm-cov --all --coverage`、`rm -rf coverage` 和 `rm -rf target/llvm-cov` 的脚本 | 单元测试 | `test-detect-frameworks.test.ts` | `generateScript()` -- 各框架正向测试 |
| AC-6 | `generateScript()` 在 `coverage_cleanup` 为空数组时不含任何 `rm -rf` 行 | 单元测试 | `test-detect-frameworks.test.ts` | `generateScript()` -- 清理步骤边界测试 |
| AC-7 | `test_detect_frameworks` 输出的 plan 条目 `script` 字段为非空字符串 | 单元测试 | `test-detect-frameworks.schema.test.ts` | plan script -- 正向测试 |
| AC-8 | `test_detect_frameworks` 输出的 plan 条目 `script` 字段为必填（不可缺失不可为 null） | 单元测试 | `test-detect-frameworks.schema.test.ts` | plan script -- 异常测试 |
| AC-9 | PlanEntry 接口 `script` 字段类型为 `string`（非可选） | 集成测试 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- plan 包含 script 字段 |
| AC-10 | 使用字符串简写配置（如 `"vitest"`）时，plan 条目同样包含 `script` 非空字段 | 集成测试 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- 字符串简写 plan script |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `test-detect-frameworks.test.ts` | `generateScript()` -- 各框架正向测试 | 正向 | 为 vitest 框架生成包含 `npx vitest run --coverage`、`rm -rf coverage` 和 `rm -rf .nyc_output` 的 bash 脚本 (AC-1) | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 各框架正向测试 | 正向 | 为 vite-plus 框架（directory 为 `plugins/dev-team/bin`）生成包含 `cd plugins/dev-team/bin`、`vp test --coverage` 的脚本 (AC-4) | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 各框架正向测试 | 正向 | 为 rust 框架生成包含 `cargo llvm-cov --all --coverage`、`rm -rf coverage` 和 `rm -rf target/llvm-cov` 的脚本 (AC-5) | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 各框架正向测试 | 正向 | 为 bun 框架生成包含 `bun test --coverage` 和 `rm -rf coverage` 的脚本 | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 各框架正向测试 | 正向 | 为 jest 框架（coverage_cleanup 为空）生成不含 `rm -rf` 行的脚本 (AC-6) | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 脚本结构测试 | 正向 | 脚本第一行为 `#!/bin/bash` (AC-2) | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 脚本结构测试 | 正向 | 脚本第二行为 `set -e` (AC-2) | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 脚本结构测试 | 正向 | 脚本最后一行等于 `coverage_cmd` 的值并以换行符结尾 | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- cd 行为边界测试 | 边界 | directory 为 `"."` 时生成的脚本不含 `cd` 行 (AC-3) | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- cd 行为边界测试 | 边界 | directory 为 `"/absolute/path"` 时生成 `cd /absolute/path` | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- cd 行为边界测试 | 边界 | directory 为空字符串时仍生成 `cd ` 行（bash 语法上无效，但函数不验证输入） | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- cd 行为边界测试 | 边界 | directory 含空格时（如 `"my project/tests"`）生成 `cd my project/tests` | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 清理步骤边界测试 | 边界 | coverage_cleanup 为空数组 `[]` 时不含任何 `rm -rf` 行 (AC-6) | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 清理步骤边界测试 | 边界 | coverage_cleanup 为单元素 `["coverage"]` 时生成单条 `rm -rf coverage` | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 清理步骤边界测试 | 边界 | coverage_cleanup 为三元素 `["a", "b", "c"]` 时按序生成三条 `rm -rf` 行 | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 清理步骤边界测试 | 边界 | coverage_cleanup 条目含路径分隔符时（如 `"target/llvm-cov"`）正确保留 | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 异常输入测试 | 异常 | 输入对象为 `null` 时抛出 TypeError | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 异常输入测试 | 异常 | directory 为 `null` 时抛出 TypeError | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 异常输入测试 | 异常 | coverage_cmd 为 `undefined` 时抛出 TypeError | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 异常输入测试 | 异常 | coverage_cleanup 为 `null` 时抛出 TypeError | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 异常输入测试 | 异常 | coverage_cmd 为空字符串时生成不含有效命令行的脚本（最后一行为空行） | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 特殊字符测试 | 边界 | coverage_cmd 含 `$HOME`、反引号、`$(subshell)` 时原样保留 | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 特殊字符测试 | 边界 | coverage_cmd 为超长字符串（>1000 字符）时正确拼接 | 新增 |
| `test-detect-frameworks.test.ts` | `generateScript()` -- 特殊字符测试 | 边界 | directory 含 `~` 或 `$VAR` 时原样保留 | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 正向测试 | 正向 | plan 条目包含 `script` 字符串字段时通过 schema 验证 (AC-7) | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 正向测试 | 正向 | script 为多行 bash 脚本（含 `\n`）时通过验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 正向测试 | 正向 | 多个 plan 条目各自包含 script 字段时通过验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 异常测试 | 异常 | plan 条目缺少 `script` 字段时 schema 拒绝 (AC-8) | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 异常测试 | 异常 | plan 条目 `script` 为 `null` 时 schema 拒绝 (AC-8) | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 异常测试 | 异常 | plan 条目 `script` 为数字时 schema 拒绝 | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 异常测试 | 异常 | plan 条目 `script` 为数组时 schema 拒绝 | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 边界测试 | 边界 | script 为空字符串 `""` 时通过 schema 验证（string 类型允许空值） | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 边界测试 | 边界 | script 为超长字符串（10000 字符）时通过验证 | 新增 |
| `test-detect-frameworks.schema.test.ts` | plan script -- 边界测试 | 边界 | plan 条目含多余未知字段 + script 时通过 passthrough 验证 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `test-detect-frameworks.test.ts` | 无 | `generateScript()` 为纯函数，无外部依赖，无需 mock | 所有 `generateScript()` 测试用例 |
| `test-detect-frameworks.schema.test.ts` | 无 | Zod schema 为纯数据验证逻辑，无需 mock | 所有 schema 测试用例 |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-7, AC-9 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- plan 包含 script 字段 | vitest 框架配置时 plan 条目包含非空 `script` 字段 | 新增 |
| AC-7, AC-9 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- plan 包含 script 字段 | 多框架 [vitest, rust] 配置时每个 plan 条目都包含非空 script | 新增 |
| AC-7, AC-9 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- plan 包含 script 字段 | plan 条目 script 内容与 `generateScript()` 对同输入的输出一致 | 新增 |
| AC-9 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- plan 包含 script 字段 | vite-plus 框架（directory 非 `.`）时 script 含 `cd` 行 | 新增 |
| AC-9 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- plan 包含 script 字段 | 空 frameworks 配置时 plan 为空数组 | 新增 |
| AC-10 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- 字符串简写 plan script | 字符串简写 `"vitest"` 时 plan 条目 script 字段为非空字符串 | 新增 |
| AC-10 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- 字符串简写 plan script | 字符串简写 `"jest"` 时 plan 条目 script 字段为非空字符串 | 新增 |
| AC-10 | `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` -- 字符串简写 plan script | 字符串简写 `"rust"` 时 plan 条目 script 字段为非空字符串 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `test-detect-frameworks.test.ts` | 文件系统 | 通过 `fs.mkdtempSync` + `fs.writeFileSync` 创建临时项目目录，测试结束后通过 `cleanup()` 清理。不 mock 文件系统，使用真实临时目录（沿用现有测试模式） | 所有 `runTestDetectFrameworks` 测试用例 |
| `test-detect-frameworks.test.ts` | `readConfig` | 不 mock，使用真实 `readConfig` 从临时项目的 `openspec/config.json` 读取配置 | 所有 `runTestDetectFrameworks` 测试用例 |
| `test-detect-frameworks.test.ts` | `runTestGetFrameworkConfig` | 不 mock，使用真实 `runTestGetFrameworkConfig` 从 FRAMEWORK_REGISTRY 获取框架配置 | 所有 `runTestDetectFrameworks` 测试用例 |

---

## 不可测试项

- **PlanEntry 接口 `script: string` 的 TypeScript 类型约束（AC-9 的编译检查）** -- 编译时类型检查由 TypeScript 编译器保证，无法通过运行时测试直接验证。通过集成测试（验证 `runTestDetectFrameworks` 输出始终包含非空 `script` 字段）间接确保接口行为正确。
- **生成的脚本在真实 bash 环境中执行并产生正确的测试结果** -- 测试仅在 Node.js 进程内验证脚本字符串的内容正确性，不实际执行 bash 脚本。bash 执行属于 Executor Agent 的职责范围，不在本变更的测试范围内。
- **PowerShell 脚本生成** -- 设计决策明确仅支持 bash，PowerShell 支持可延迟到后续变更，不在本测试覆盖范围。
