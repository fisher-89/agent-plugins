# 任务: unified-coverage-artifacts

> **变更**: unified-coverage-artifacts
> **日期**: 2026-06-08

---

## 阶段 1: FrameworkConfig 接口扩展与注册表填充

- [x] 在 `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` 的 `FrameworkConfig` 接口中新增 `coverage_artifacts: string[]` 和 `coverage_cleanup: string[]` 字段
- [x] 为 `FRAMEWORK_REGISTRY` 中的 jest 条目填充 `coverage_artifacts: ["coverage/**"]` 和 `coverage_cleanup: ["coverage", ".nyc_output"]`
- [x] 为 `FRAMEWORK_REGISTRY` 中的 vitest 条目填充 `coverage_artifacts: ["coverage/**"]` 和 `coverage_cleanup: ["coverage", ".nyc_output"]`
- [x] 为 `FRAMEWORK_REGISTRY` 中的 vite-plus 条目填充 `coverage_artifacts: ["coverage/**"]` 和 `coverage_cleanup: ["coverage", ".nyc_output"]`
- [x] 为 `FRAMEWORK_REGISTRY` 中的 bun 条目填充 `coverage_artifacts: ["coverage/**"]` 和 `coverage_cleanup: ["coverage"]`
- [x] 为 `FRAMEWORK_REGISTRY` 中的 rust 条目填充 `coverage_artifacts: ["coverage/**", "target/llvm-cov/**"]` 和 `coverage_cleanup: ["coverage", "target/llvm-cov"]`

## 阶段 2: Zod 输出 Schema 扩展

- [x] 在 `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.ts` 的 `testGetFrameworkConfigOutputSchema` 中新增 `coverage_artifacts: z.array(z.string()).optional()` 和 `coverage_cleanup: z.array(z.string()).optional()` 字段
- [x] 在 `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` 的 plan 条目 Zod schema 中新增 `coverage_artifacts: z.array(z.string()).optional()` 和 `coverage_cleanup: z.array(z.string()).optional()` 字段

## 阶段 3: PlanEntry 接口扩展与 plan 生成

- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` 的 `PlanEntry` 接口中新增 `coverage_artifacts: string[]` 和 `coverage_cleanup: string[]` 字段
- [x] 在 `runTestDetectFrameworks` 的 plan 条目构造处（push 到 `plan` 数组时）从 `runTestGetFrameworkConfig` 结果中解构 `coverage_artifacts` 和 `coverage_cleanup` 并注入 plan 条目

## 阶段 4: Executor 指南更新

- [x] 在 `plugins/dev-team/agents/unit-test-executor.md` 的步骤 2（逐目录执行覆盖率命令）和步骤 3（覆盖率解析）之间插入新的"移动覆盖率产物"步骤
- [x] 在新增步骤中描述：遍历 plan 条目，对每个 `coverage_artifacts` 非空的条目，创建统一目标目录 `reports/coverage/<framework>/` 并按 glob 匹配移动产物
- [x] 在新增步骤中描述：移动成功后更新 `coverage_output` 路径为统一位置下的路径（如 `reports/coverage/vitest/coverage-summary.json`）
- [x] 在新增步骤中描述：移动成功后删除 `coverage_cleanup` 中的原始目录
- [x] 在新增步骤中描述：移动失败时的容错处理（记录 findings，保留原始状态，不执行清理，不阻断流程）
- [x] 在新增步骤中描述：`coverage_artifacts` 为空数组时跳过移动步骤
- [x] 在新增步骤中描述：更新 `html_report` 路径为统一位置下的路径（如 `reports/coverage/vitest/index.html`）
- [x] 更新步骤 3（覆盖率解析）的说明，使其从更新后的 `coverage_output` 路径读取（路径已指向统一位置）
- [x] 更新步骤 4（报告写入）的 `coverage_by_framework` 输出格式，确保 `html_report` 路径指向统一位置

## 阶段 5: 单元测试

- [x] 更新 `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts`：为每个框架的 EXPECTED_CONFIGS 添加 `coverage_artifacts` 和 `coverage_cleanup` 字段，更新全部 `toEqual` 断言（AC-1, AC-2, AC-3）
- [x] 新增测试：验证 `test_get_framework_config("vitest")` 返回的配置包含 `coverage_artifacts: ["coverage/**"]` 和 `coverage_cleanup: ["coverage", ".nyc_output"]`（AC-1）
- [x] 新增测试：验证 `test_get_framework_config("rust")` 返回的配置中 `coverage_artifacts` 包含 `coverage/**` 和 `target/llvm-cov/**`，`coverage_cleanup` 包含 `coverage` 和 `target/llvm-cov`（AC-2）
- [x] 新增测试：遍历全部五个框架（jest/vitest/vite-plus/bun/rust），验证每个框架的 `coverage_artifacts` 和 `coverage_cleanup` 均为非空数组（AC-3）
- [x] 更新 `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts`：在 plan 条目测试中添加包含 `coverage_artifacts` 和 `coverage_cleanup` 的正向验证（AC-4）
- [x] 在 schema 测试中新增：验证不包含 `coverage_artifacts` 和 `coverage_cleanup` 的旧格式 plan 条目仍然通过解析（向后兼容，AC-5）
- [x] 新增 schema 测试：验证 `testGetFrameworkConfigOutputSchema` 允许 `coverage_artifacts` 和 `coverage_cleanup` 为可选字段（AC-5）

## 阶段 6: 测试执行与验证

- [x] 运行全部现有测试，验证扩展后的 schema 和接口不破坏向后兼容
- [x] 运行全部新增测试，验证 AC-1 至 AC-5 全部通过
- [x] 运行 `pnpm run -C ./plugins/dev-team/bin check`（静态分析），验证无类型错误
- [x] 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（从 `2.6.0` 升级）
