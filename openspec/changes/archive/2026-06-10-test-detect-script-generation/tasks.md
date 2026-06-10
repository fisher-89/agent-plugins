# 实施任务: test-detect-script-generation

---

## 阶段 1: Schema 扩展

- [x] 修改 `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts`，在 `testDetectFrameworksOutputSchema` 的 plan 条目 Zod 对象中新增 `script: z.string().describe('Bash execution script with shebang, set -e, cd, rm -rf, and coverage command')` 必填字段

## 阶段 2: 核心实现

- [x] 修改 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts`，在 `PlanEntry` 接口中新增 `script: string` 必填字段（非可选）
- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` 中实现并导出 `generateScript()` 纯函数，接收 `{ directory: string; coverage_cleanup: string[]; coverage_cmd: string }` 参数，返回按以下规则生成的 bash 脚本字符串：
  - 第一行: `#!/bin/bash`
  - 第二行: `set -e`
  - 当 `directory !== "."` 时插入 `cd <directory>` 行
  - 对 `coverage_cleanup` 每个条目生成一行 `rm -rf <item>`（保持数组顺序）
  - 当 `coverage_cleanup` 为空数组时不生成任何 `rm -rf` 行
  - 最后一行: `<coverage_cmd>`
  - 所有行以 `\n` 分隔，末尾包含换行符
- [x] 修改 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` 中 `runTestDetectFrameworks()` 函数，在生成 plan 条目时调用 `generateScript({ directory, coverage_cleanup, coverage_cmd })`，将返回值填充到 `script` 字段

## 阶段 3: 单元测试

### `generateScript` 纯函数测试

- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` 中新增 `generateScript` describe 块，编写 AC-1 测试: vitest 框架生成包含 `npx vitest run --coverage` 和 `rm -rf coverage`、`rm -rf .nyc_output` 的脚本
- [x] 编写测试验证脚本以 `#!/bin/bash` 和 `set -e` 开头（AC-2）
- [x] 编写测试验证 `directory` 为 `"."` 时脚本不含 `cd` 行（AC-3）
- [x] 编写测试验证 `directory` 为 `"plugins/dev-team/bin"` 时脚本包含 `cd plugins/dev-team/bin`（AC-4）
- [x] 编写测试验证 rust 框架生成包含 `cargo llvm-cov --all --coverage`、`rm -rf coverage` 和 `rm -rf target/llvm-cov` 的脚本（AC-5）
- [x] 编写测试验证 `coverage_cleanup` 为空数组时不含任何 `rm -rf` 行（AC-6）
- [x] 编写测试验证 `coverage_cleanup` 数组中多个条目生成对应多行 `rm -rf`（顺序一致）
- [x] 编写测试验证相同输入产生相同输出（确定性/纯函数）

### Schema 测试补充

- [x] 在 `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` 中新增测试，验证包含 `script` 字段的 plan 条目通过 schema 验证（AC-7）
- [x] 编写测试验证 plan 条目缺失 `script` 字段时 schema 拒绝（AC-8）
- [x] 编写测试验证 `script` 为 `null` 时 schema 拒绝
- [x] 编写测试验证 `script` 为空字符串时 schema 通过（string 类型，空字符串是合法的 string 值）

### Plan 集成测试补充

- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` 的 plan 集成测试中，补充验证每个 plan 条目包含 `script` 非空字符串字段
- [x] 编写测试验证字符串简写配置（如 `"vitest"`）时 plan 条目的 `script` 字段为非空（AC-10）

## 阶段 4: 验证

- [x] 运行 `npx vite-plus test`（或项目对应的测试命令）确保所有测试通过，无回归
- [x] 运行 TypeScript 编译检查（`npx tsc --noEmit`）确认 `PlanEntry` 接口的 `script` 必填字段在所有实现处正确填充（AC-9）
