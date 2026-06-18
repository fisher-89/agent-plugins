# 任务: simplify-test-report-schema

> **变更**: simplify-test-report-schema
> **日期**: 2026-06-18

---

## 阶段 1: FRAMEWORK_REGISTRY JSON-only 覆盖率配置

- [x] 在 `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` 中将 jest 的 `coverage_cmd` 改为 `npx jest --coverage --coverageReporters=json-summary`，`coverage_artifacts` 改为 `['coverage/coverage-summary.json']`
- [x] 将 vitest 的 `coverage_cmd` 改为 `npx vitest run --coverage --coverage.reporter=json-summary`，`coverage_artifacts` 改为 `['coverage/coverage-summary.json']`
- [x] 将 vite-plus 的 `coverage_cmd` 改为 `vp test --coverage --coverage.reporter=json-summary`，`coverage_artifacts` 改为 `['coverage/coverage-summary.json']`
- [x] 将 bun 的 `coverage_cmd` 改为 `bun test --coverage --coverageReporters=json-summary`，`coverage_artifacts` 改为 `['coverage/coverage-summary.json']`
- [x] 将 rust 的 `coverage_cmd` 改为 `cargo llvm-cov --json`，`coverage_artifacts` 改为 `['coverage/coverage-summary.json']`（`coverage_cleanup` 保持不变）
- [x] 更新 `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` 中 `EXPECTED_CONFIGS` 的 `coverage_cmd` 与 `coverage_artifacts` 期望值（AC-9）
- [x] 运行 `pnpm run -C ./plugins/dev-team/bin test -- test-get-framework-config` 确认通过

## 阶段 2: unit-test-executor 报告 schema 更新

- [x] 更新步骤 2「Failure handling」：删除「Exclude the framework from `html_reports`」及相关 HTML 引用
- [x] 更新步骤 3「Move coverage artifacts」：产物移动改为仅处理 `coverage_artifacts` 中的 JSON 摘要文件；删除 `html_report` 路径设置（第 5 步子项）
- [x] 更新步骤 5「Thresholds & overrides check」：将 `coverage_pass` 写入说明改为写入嵌套 `coverage.pass`；`coverage.measured`、`coverage.thresholds`、`coverage.overrides`、`coverage.by_framework` 字段路径
- [x] 更新步骤 6「Execute integration tests」：删除 `failures[]` 提取说明；失败详情直接写入 `integration_test.test_cases[]` 失败条目
- [x] 更新步骤 7 主报告 JSON 示例：嵌套 `coverage` 对象；`test_cases[]` 失败条目含 `line`/`error_type`/`error_message`/`stack_trace`；删除 `failures[]`、`html_reports`、`coverage_thresholds`、`coverage_pass`、`coverage_by_framework`、`coverage_overrides` 及 `html_report`（AC-1、AC-3）
- [x] 更新 no-coverage 示例：仅 `"coverage": null`，不含其他 coverage 相关顶层字段（AC-2）
- [x] 更新 `integration_test` 示例：失败详情在 `test_cases[]` 中，无 `failures[]`（AC-4）

## 阶段 3: unit-test-evaluator 消费逻辑更新

- [x] 更新 Static Checklist U1：必需字段改为 `phase, command, timestamp, total, passed, failed, skipped, coverage, duration_seconds, test_cases`；删除 `coverage_thresholds, coverage_pass, coverage_by_framework, html_reports, failures`（AC-5）
- [x] 更新 Static Checklist U3：判断依据改为 `coverage.pass === true` 或 `coverage === null`（AC-5、AC-7）
- [x] 更新 Step 1：校验 `coverage` 为嵌套对象（含 `pass, measured, thresholds, by_framework, overrides`）或 `null`；删除对 `failures`、`html_reports`、扁平 coverage 字段的要求
- [x] 更新 Step 3 覆盖率子检查：引用 `coverage.pass`、`coverage.measured`、`coverage.thresholds`、`coverage.overrides`；`coverage === null` 时 evidence 为「覆盖率检查未配置或生成失败，跳过」（AC-7）
- [x] 更新 Step 4：决策树输入改为 `test_cases.filter(c => c.status === "failed")`，从条目读取 `error_type`、`file`、`line`；判定规则与优先级保持不变（AC-6）
- [x] 更新 Step 5 findings 模板：使用 `coverage.measured`、`coverage.pass`、`coverage.by_framework[].measured`；删除所有 HTML 报告路径引用（AC-8）
- [x] 更新 Constraints：将 `coverage_pass` 引用改为 `coverage.pass`

## 阶段 4: 验证与收尾

- [x] 在 `plugins/dev-team/agents/` 下 grep 确认无残留旧字段引用：`coverage_pass`、`coverage_thresholds`、`coverage_by_framework`、`coverage_overrides`、`html_reports`、`html_report`、顶层 `failures`（AC-5、AC-8）
- [x] 运行 `pnpm run -C ./plugins/dev-team/bin check`（静态分析）
- [x] 运行 `pnpm run -C ./plugins/dev-team/bin test`（全量单元测试）
- [x] 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（当前 `2.6.19` → 下一 patch 版本）
