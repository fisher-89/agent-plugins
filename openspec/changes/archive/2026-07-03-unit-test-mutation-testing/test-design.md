# 测试设计: unit-test-mutation-testing

> **日期**: 2026-07-01

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|-------|---------|---------|----------|-----------------|
| AC-1 | `config.test.mutation.score` 为可选数字，默认值 80；`config.test.overrides[].mutation` 为可选对象，含 `score` 数字字段 | 单元测试 | `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 |
| AC-2 | `FrameworkConfig` 包含 `mutation_framework: string \| null` 字段；jest/vitest/vite-plus 注册为 `"stryker-js"`，其余框架为 `null` | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 |
| AC-3 | `PlanEntry` 包含 `mutation_framework`、`mutation_config`、`mutation_score` 字段，从 registry 和 config 正确填充 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan mutation 字段 |
| AC-4 | 当 `mutation_framework` 非空时，`executePlanEntry` 在覆盖率解析后执行 StrykerJS；输出存入 `ExecutionResult.mutation`；执行时长计入 `durationMs` | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 |
| AC-5 | 项目根目录存在 `stryker.config.*` 时直接使用；否则生成临时配置，执行后清理 | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 |
| AC-6 | 解析 `reports/mutation/mutation.json` 的 StrykerJS JSON 输出，正确提取 score 和各类变异体计数 | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | parseMutationReport -- StrykerJS 报告解析 |
| AC-7 | `MutationMeasured` 包含 killed/survived/timeout/noCoverage/compileError/runtimeError/ignored/total/detected/undetected 字段；`MutationBlock` 包含 pass/score/threshold/measured/by_framework 字段 | 单元测试 | `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock |
| AC-8 | 子报告和汇总报告包含 `mutation` 字段（mutation 框架时）；阈值判定基于 `measured.score >= thresholds.score` | 单元测试 | `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 |
| AC-9 | `--no-mutation` 选项跳过所有 mutation 执行；mutation 框架检测仍正常进行 | 单元测试 | `plugins/dev-team/bin/src/cli.test.ts` | cli -- --no-mutation 选项 |
| AC-9 | `--no-mutation` 透传到 executePlanEntry 的 noMutation 选项 | 单元测试 | `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest -- noMutation 透传 |
| AC-10 | bun/node-test/go/rust/pytest 的 `mutation_framework` 为 null，执行时跳过 mutation 阶段，report 中 mutation 为 null | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 不支持框架 mutation_framework=null |
| AC-10 | mutation_framework=null 时 executePlanEntry 跳过 StrykerJS 执行 | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation_framework=null 跳过 |
| AC-1 至 AC-10 | 端到端：mutation 配置 → 框架检测 → plan 生成 → StrykerJS 执行 → 报告输出 | 集成测试 | `plugins/dev-team/bin/src/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` | mutation 执行全流程 |
| AC-7, AC-8 | 子报告和汇总报告 mutation 块结构正确性 | 集成测试 | `plugins/dev-team/bin/src/__tests__/mutation-report-format/mutation-report-format.test.ts` | mutation 报告格式 |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 正向 | `test.mutation.score` 为可选数字，默认值为 80 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 正向 | `test.mutation.score` 显式设置为 90 时取值为 90 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 正向 | `test.overrides[].mutation` 为 `{ score: 85 }` 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 异常 | `test.mutation.score` 为负数时被 schema 拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 异常 | `test.mutation.score` 为字符串（如 `"80"`）时被拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 异常 | `test.mutation.score` 超过 100 时被拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 边界 | `test.mutation.score` 为 0 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 边界 | `test.mutation` 未设置时，`mutation` 在解析结果中为 undefined | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | config.schema -- mutation 字段 | 边界 | `test.overrides[].mutation.score` 为 100（最大值）时通过验证 | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 正向 | jest 的 `mutation_framework` 为 `"stryker-js"` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 正向 | vitest 的 `mutation_framework` 为 `"stryker-js"` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 正向 | vite-plus 的 `mutation_framework` 为 `"stryker-js"` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 正向 | bun 的 `mutation_framework` 为 `null` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 正向 | node-test 的 `mutation_framework` 为 `null` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 正向 | go 的 `mutation_framework` 为 `null` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 正向 | rust 的 `mutation_framework` 为 `null` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 正向 | pytest 的 `mutation_framework` 为 `null` | 新增 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- mutation_framework 字段 | 边界 | FrameworkConfig 字段数量为 9（新增 mutation_framework） | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan mutation 字段 | 正向 | vitest 框架时 `PlanEntry.mutation_framework` 为 `"stryker-js"` | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan mutation 字段 | 正向 | vitest 框架时 `PlanEntry.mutation_config` 从 config 正确填充 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan mutation 字段 | 正向 | vitest 框架时 `PlanEntry.mutation_score` 为 config 中的 score 值 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan mutation 字段 | 正向 | bun 框架时 `PlanEntry.mutation_framework` 为 null | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan mutation 字段 | 边界 | config 未设置 `test.mutation` 时 `mutation_config` 为 null，`mutation_score` 为 null | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan mutation 字段 | 边界 | 多框架（vitest + go）时每个 plan 条目正确携带各自的 mutation 字段 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema -- PlanEntry mutation 字段 | 正向 | plan 条目包含 `mutation_framework`、`mutation_config`、`mutation_score` 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema -- PlanEntry mutation 字段 | 正向 | `mutation_framework` 为 null 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema -- PlanEntry mutation 字段 | 正向 | `mutation_config` 为 null 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema -- PlanEntry mutation 字段 | 异常 | `mutation_config` 类型错误（如字符串）时拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | testDetectFrameworksOutputSchema -- PlanEntry mutation 字段 | 边界 | plan 条目缺少 mutation 字段时仍通过验证（向后兼容） | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 正向 | `MutationMeasured` 包含全部 10 个数字字段（killed/survived/timeout/noCoverage/compileError/runtimeError/ignored/total/detected/undetected） | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 正向 | `MutationBlock` 包含 pass/score/threshold/measured/by_framework 字段 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 正向 | `MutationBlock.overrides` 为可选数组，包含 `MutationOverride` 条目 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 正向 | `UnitTestSubReport.mutation` 为 `MutationBlock \| null` 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 正向 | `UnitTestSummaryReport.mutation` 为 `MutationBlock \| null` 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 异常 | `MutationMeasured.killed` 为负数时被拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 异常 | `MutationBlock.score` 超过 100 时被拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 边界 | `MutationMeasured` 所有字段为 0 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | unit-test-output.schema -- MutationMeasured/MutationBlock | 边界 | `UnitTestSubReport.mutation` 为 null 时通过验证（不支持框架场景） | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | parseMutationReport -- StrykerJS 报告解析 | 正向 | 解析完整 StrykerJS JSON 报告，正确提取 score 和 killed/survived/timeout 等计数 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | parseMutationReport -- StrykerJS 报告解析 | 正向 | 解析的 score 为浮点数时保留精度 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | parseMutationReport -- StrykerJS 报告解析 | 异常 | 报告文件不存在时返回 null | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | parseMutationReport -- StrykerJS 报告解析 | 异常 | 报告文件为空时返回 null | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | parseMutationReport -- StrykerJS 报告解析 | 异常 | 报告 JSON 格式异常时返回 null | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | parseMutationReport -- StrykerJS 报告解析 | 边界 | 所有变异体计数均为 0（无变异体）时返回 score=100 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | parseMutationReport -- StrykerJS 报告解析 | 边界 | score 为 0（所有变异体存活）时正确返回 0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 正向 | 项目根目录存在 `stryker.config.json` 时直接返回该路径，cleanup=false | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 正向 | 项目根目录存在 `stryker.config.mjs` 时直接返回该路径，cleanup=false | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 正向 | 项目根目录存在 `stryker.config.cjs` 时直接返回该路径，cleanup=false | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 正向 | 无自定义配置时生成临时配置文件，返回路径和 cleanup=true | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 正向 | 生成临时配置时 mutate 限定为 sourceFiles 路径列表 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 正向 | 生成临时配置时 testRunner 根据 framework 选择 "jest-runner" 或 "vitest-runner" | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 正向 | 生成临时配置时 reporters 设置为 `["json"]` | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 正向 | 生成临时配置时 thresholds 从传入参数读取 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 异常 | sourceFiles 为空数组时 mutate 为空列表 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | resolveStrykerConfig -- 配置检测与生成 | 边界 | 临时配置文件名使用随机后缀避免冲突 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 正向 | `mutation_framework` 非空且 `noMutation=false` 时在覆盖率解析后执行 StrykerJS（通过 execSync 调用 npx stryker run） | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 正向 | mutation 执行结果存入 `ExecutionResult.mutation`，包含正确 score 和 measured | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 正向 | mutation 执行时长计入 `durationMs` | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 正向 | mutation 执行后清理临时配置文件和 `reports/mutation/` 目录 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 异常 | `mutation_framework` 为 null 时跳过 StrykerJS 执行，mutation=null | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 异常 | `noMutation=true` 时跳过 StrykerJS 执行，mutation=null | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 异常 | StrykerJS 命令失败（退出码非 0）时静默跳过，mutation=null，不阻断测试流程 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 异常 | StrykerJS 报告文件不存在时 mutation=null，不崩溃 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- mutation 执行阶段 | 边界 | 执行顺序：测试命令 → 覆盖率解析 → StrykerJS | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 正向 | `result.mutation` 非 null 时子报告 `mutation` 块包含正确的 score/threshold/pass/measured | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 正向 | `measured.score >= thresholds.score` 时 `pass=true` | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 正向 | `measured.score < thresholds.score` 时 `pass=false` | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 正向 | `computeMutationOverrides()` 正确匹配 glob 并计算 pass/fail | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 正向 | 汇总报告 `computeMutationResult()` 按 source_files 加权平均计算 score | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 异常 | `result.mutation` 为 null 时子报告 `mutation` 为 null | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 异常 | 所有子报告 mutation 均为 null 时汇总报告 mutation 为 null | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 边界 | mutation pass=false 时汇总报告 conclusion 为 fail | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 边界 | mutation overrides 中某个 override 失败时总体 pass=false | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 边界 | mutation score 恰好等于 threshold 时 pass=true | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport / generateSummaryReport -- mutation 块 | 边界 | 多框架时加权平均正确计算，某框架 source_files 为空时不参与加权 | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | cli -- --no-mutation 选项 | 正向 | CLI 注册 `--no-mutation` 选项 | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | cli -- --no-mutation 选项 | 正向 | `--no-mutation` 选项传递到 `UnitTestOptions.noMutation` | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest -- noMutation 透传 | 正向 | `UnitTestOptions.noMutation` 为 true 时透传到 `executePlanEntry` 的 options 中 | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest -- noMutation 透传 | 正向 | `UnitTestOptions.noMutation` 为 false 时透传到 `executePlanEntry` 的 options 中 | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest -- noMutation 透传 | 边界 | `UnitTestOptions.noMutation` 为 undefined 时等价于 false（默认执行 mutation） | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `child_process.execSync` | `vi.mock('child_process', () => ({ execSync: vi.fn() }))`，模拟返回 StrykerJS 命令的 stdout | executePlanEntry -- mutation 执行阶段 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `fs.existsSync` / `fs.readFileSync` | 模拟 StrykerJS JSON 报告文件的存在和内容 | executePlanEntry -- mutation 报告解析 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | `fs.existsSync` | 模拟 `stryker.config.*` 文件的存在/不存在 | resolveStrykerConfig -- 配置检测 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | `fs.writeFileSync` / `fs.rmSync` | 使用 temp 目录验证临时配置写入和清理 | resolveStrykerConfig -- 临时配置生成与清理 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts` | `fs.readFileSync` | 使用临时文件写入 StrykerJS JSON 内容，测试解析逻辑 | parseMutationReport -- 报告解析 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | `fs` 文件系统 | 使用临时目录创建/读取报告文件 | generateSubReport / generateSummaryReport -- mutation 块 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `config.json` 文件系统 | 创建临时项目目录并写入 `openspec/config.json`，通过 `readConfig` 读取 | runTestDetectFrameworks -- plan mutation 字段 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | `test-detect-frameworks`、`test-runner`、`test-report` | 使用 `vi.mock` 模拟 `runTestDetectFrameworks`、`executePlanEntry`、`generateSubReport`、`generateSummaryReport` | runUnitTest -- noMutation 透传 |
| `plugins/dev-team/bin/src/cli.test.ts` | `runUnitTest` | 使用 `vi.mock` 模拟 `./commands/unit-test` 模块 | cli -- --no-mutation 选项 |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|-------|---------|---------|----------|----------|
| AC-1 至 AC-10 | `plugins/dev-team/bin/src/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` | mutation 执行全流程 | 完整配置（vitest + mutation.score=80）→ 框架检测 → plan 生成（含 mutation 字段）→ executePlanEntry 执行 → 报告生成（含 mutation 块） | 新增 |
| AC-1 至 AC-10 | `plugins/dev-team/bin/src/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` | mutation 执行全流程 | `--no-mutation` 标志跳过 mutation 阶段，其余流程正常 | 新增 |
| AC-1 至 AC-10 | `plugins/dev-team/bin/src/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` | mutation 执行全流程 | 不支持框架（如 bun）配置时 mutation 全流程跳过 | 新增 |
| AC-1 至 AC-10 | `plugins/dev-team/bin/src/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` | mutation 执行全流程 | 配置 override 中 mutation 阈值覆盖全局阈值 | 新增 |
| AC-7, AC-8 | `plugins/dev-team/bin/src/__tests__/mutation-report-format/mutation-report-format.test.ts` | mutation 报告格式 | 子报告 mutation 块结构：pass/score/threshold/measured/by_framework | 新增 |
| AC-7, AC-8 | `plugins/dev-team/bin/src/__tests__/mutation-report-format/mutation-report-format.test.ts` | mutation 报告格式 | 汇总报告 mutation 块结构：pass/score/threshold/measured/by_framework/overrides | 新增 |
| AC-7, AC-8 | `plugins/dev-team/bin/src/__tests__/mutation-report-format/mutation-report-format.test.ts` | mutation 报告格式 | 变异得分低于阈值时报告 pass=false，conclusion=fail | 新增 |
| AC-7, AC-8 | `plugins/dev-team/bin/src/__tests__/mutation-report-format/mutation-report-format.test.ts` | mutation 报告格式 | 不支持框架时子报告和汇总报告 mutation 均为 null | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` | `child_process.execSync` | 模拟 StrykerJS 命令执行，返回模拟的 mutation.json 输出 | mutation 执行全流程 |
| `plugins/dev-team/bin/src/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` | 文件系统 | 创建临时项目目录，写入 config.json 和模拟的 StrykerJS 报告文件 | mutation 执行全流程 |
| `plugins/dev-team/bin/src/__tests__/mutation-report-format/mutation-report-format.test.ts` | 文件系统 | 创建临时目录，手动构造包含 mutation 数据的子报告和汇总报告 | mutation 报告格式 |

---

## 不可测试项

- 无
