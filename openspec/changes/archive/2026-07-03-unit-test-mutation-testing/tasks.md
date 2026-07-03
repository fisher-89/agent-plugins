# 任务: unit-test-mutation-testing

> **变更**: unit-test-mutation-testing
> **日期**: 2026-07-01

---

## 第一阶段：Schema 定义

- [x] 在 `config.schema.ts` 中新增 `testMutationScoreSchema`（可选数字，默认 80）和 `mutationOverrideSchema`（`{ score: number }`）
- [x] 在 `config.schema.ts` 的 `test` 对象中添加 `mutation` 字段（`testMutationScoreSchema`），在 `overrides` 数组元素中添加 `mutation` 字段（`mutationOverrideSchema`）
- [x] 在 `unit-test-output.schema.ts` 中新增 `MutationMeasured` schema（killed、survived、timeout、noCoverage、compileError、runtimeError、ignored、total、detected、undetected 均为 `z.number().int().min(0)`）
- [x] 在 `unit-test-output.schema.ts` 中新增 `MutationOverride` schema（glob、score、threshold、pass、file_count、passed_count）
- [x] 在 `unit-test-output.schema.ts` 中新增 `MutationBlock` schema（pass、score、threshold、measured、by_framework、overrides）
- [x] 在 `unit-test-output.schema.ts` 的 `UnitTestSubReport` 和 `UnitTestSummaryReport` 中添加 `mutation: MutationBlock | null` 字段

## 第二阶段：框架注册表扩展

- [x] 在 `test-framework.ts` 的 `FrameworkConfig` 接口中添加 `mutation_framework: string | null` 字段
- [x] 在 `FRAMEWORK_REGISTRY` 中为 jest、vitest、vite-plus 设置 `mutation_framework: "stryker-js"`，其余框架设置为 `null`
- [x] 在 `test-detect-frameworks.ts` 的 `PlanEntry` 接口中添加 `mutation_framework: string | null`、`mutation_config: { score: number } | null`、`mutation_score: number | null` 字段
- [x] 在 `test-detect-frameworks.ts` 的 `buildPlanFromMappings()` 中从 `getFrameworkConfig()` 和 `readConfig()` 填充 `PlanEntry` 的 mutation 字段
- [x] 在 `test-detect-frameworks.schema.ts` 的 `PlanEntry` output schema 中添加 `mutation_framework`、`mutation_config`、`mutation_score` 字段

## 第三阶段：StrykerJS 配置生成器

- [x] 新建 `stryker-config.ts`，实现 `resolveStrykerConfig()` 函数：检测项目根目录是否存在 `stryker.config.{json,mjs,cjs}`，存在则直接返回路径
- [x] 在 `resolveStrykerConfig()` 中实现临时配置生成逻辑：写入 `stryker.config.<random>.json` 到项目根目录，配置包含 mutate、testRunner、plugins、reporters（["json"]）、thresholds、timeoutMS、maxTestRunnerReuse
- [x] 临时配置的 `mutate` 限定为 sourceFiles 路径列表，`testRunner` 根据 framework 选择 "jest-runner" 或 "vitest-runner"
- [x] 返回 `{ configPath: string, cleanup: boolean }`，cleanup 为 true 时调用方需清理临时文件

## 第四阶段：Mutation 报告解析器

- [x] 新建 `mutation-parser.ts`，定义 `MutationReport` 接口（score、killed、survived、timeout、noCoverage、compileError、runtimeError、ignored、total、detected、undetected）
- [x] 实现 `parseMutationReport(reportPath: string): MutationReport | null` 函数：读取 `reports/mutation/mutation.json`，提取 StrykerJS 输出中的 metrics 字段
- [x] 处理 StrykerJS 报告文件不存在或格式异常的情况（返回 null，调用方静默跳过）

## 第五阶段：测试运行器扩展

- [x] 在 `test-runner.ts` 的 `ExecutionResult` 接口中添加 `mutation: MutationBlock | null` 字段（使用已定义的 MutationBlock 类型）
- [x] 在 `executePlanEntry()` 中添加 `noMutation?: boolean` 选项参数
- [x] 在 `executePlanEntry()` 中，覆盖率解析之后，添加 mutation 执行阶段：检查 `entry.mutation_framework` 非空且 `!noMutation`
- [x] mutation 执行阶段：调用 `resolveStrykerConfig()` 获取配置路径，执行 `npx stryker run --config <configPath> --reporters json`
- [x] mutation 执行后：调用 `parseMutationReport()` 解析结果，清理临时文件和 `reports/mutation/` 目录
- [x] mutation 执行时长计入 `durationMs`（在 `startTime` 之后统一计算）
- [x] mutation 执行失败（命令退出码非 0 或报告解析失败）时静默跳过，mutation 字段为 null，不阻断测试流程

## 第六阶段：报告生成器扩展

- [x] 在 `test-report.ts` 中添加 `computeMutationResult()` 函数：从 subReports 聚合 mutation 数据，计算加权平均 score
- [x] 在 `test-report.ts` 中添加 `computeMutationOverrides()` 函数：读取 `config.test.overrides[].mutation`，使用 `matchGlob()` 匹配文件，计算 pass/fail
- [x] 在 `generateSubReport()` 中添加 mutation 块构建逻辑：从 `result.mutation` 读取数据，构建 `MutationBlock`
- [x] 在 `generateSummaryReport()` 中添加 mutation 聚合：调用 `computeMutationResult()` 和 `computeMutationOverrides()`，构建汇总 `MutationBlock`
- [x] 在汇总报告的 conclusion 判定中纳入 mutation pass/fail 状态

## 第七阶段：CLI 和命令入口扩展

- [x] 在 `cli.ts` 的 `unit-test` 命令中添加 `.option('--no-mutation', 'Skip mutation testing phase')`
- [x] 在 `unit-test.ts` 的 `UnitTestOptions` 接口中添加 `noMutation?: boolean`
- [x] 在 `unit-test.ts` 的 `runUnitTest()` 中将 `noMutation` 标志传递给 `executePlanEntry()`

## 第八阶段：集成验证

- [ ] 确认 config schema 变更后 `openspec/config.json` 的 `test.mutation.score` 和 `test.overrides[].mutation` 能被正确读取
- [ ] 确认 `FrameworkConfig` 中 jest/vitest/vite-plus 的 `mutation_framework` 为 `"stryker-js"`，其余框架为 `null`
- [ ] 确认 `PlanEntry` 的 mutation 字段从 registry 和 config 正确填充
- [ ] 确认 `--no-mutation` 标志能跳过 mutation 执行阶段
- [ ] 确认不支持框架（bun/node-test/go/rust/pytest）静默跳过 mutation 阶段，报告 mutation 为 null
- [ ] 确认子报告和汇总报告正确包含 mutation 块
- [ ] 确认变异得分低于阈值时报告判定为 fail
- [ ] 确认 overrides 中 mutation 阈值能正确覆盖全局阈值
