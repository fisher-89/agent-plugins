# 任务: add-node-go-pytest-frameworks

> **变更**: add-node-go-pytest-frameworks
> **日期**: 2026-06-23

---

## 阶段 1: Config Schema 扩展

- [x] 在 `plugins/dev-team/bin/src/schemas/config/config.schema.ts` 的 `testFrameworkSchema` 枚举中新增 `'node-test'`、`'go'`、`'pytest'`
- [x] 确认 `TestFrameworks` 类型随 schema 推断自动包含三个新字面量

## 阶段 2: 框架注册表与 TypeScript 类型

- [x] 在 `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` 将 `FrameworkConfig.coverage_format` 联合类型扩展为 `'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py'`
- [x] 在 `FRAMEWORK_REGISTRY` 新增 `node-test` 条目：`test_cmd`、`coverage_cmd`（两步脚本含 `--experimental-test-coverage` 与 `parse-node-test-coverage.mjs`）、`coverage_format: 'node-test'`、`coverage_output`、`coverage_artifacts`、`coverage_cleanup`、`default_glob: '**/*.test.{mjs,js,cjs}'`
- [x] 在 `FRAMEWORK_REGISTRY` 新增 `go` 条目：`go test -coverprofile` + `go tool cover -func` 链式命令、`coverage_format: 'go-cover'`、`coverage_output: 'coverage/func-summary.txt'` 等
- [x] 在 `FRAMEWORK_REGISTRY` 新增 `pytest` 条目：`pytest --cov=. --cov-report=json --cov-branch`、`coverage_format: 'coverage-py'`、`coverage_output: 'coverage.json'` 等
- [x] 确认既有五框架（jest/vitest/vite-plus/bun/rust）条目未被修改

## 阶段 3: node:test 覆盖率解析脚本

- [x] 新建 `plugins/dev-team/scripts/parse-node-test-coverage.mjs`，实现 CLI：`node parse-node-test-coverage.mjs <stdout-file> <output-json-path>`
- [x] 实现从 node:test stdout 文本表格提取 `lines`、`branches`、`functions` 总覆盖率百分比
- [x] 写入 istanbul 兼容 JSON（`total.lines.pct`、`total.branches.pct`、`total.functions.pct`）
- [x] 解析失败时非零退出码 + stderr 错误信息

## 阶段 4: MCP 输出 Schema 同步

- [x] 在 `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.ts` 将 `coverage_format` 枚举扩展为五值
- [x] 在 `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` 将 plan 条目 `coverage_format` 枚举扩展为五值
- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` 同步 `PlanEntry` 接口的 `coverage_format` 类型（若与 schema 分离定义）

## 阶段 5: unit-test-executor Agent 更新

- [x] 更新 Step 1 文档：`coverage_format` 枚举说明扩展为五值
- [x] 更新 Step 2 测试输出解析：新增 **node-test**（`✔`/`✖`、`# Subtest:`）、**go**（`--- PASS:`/`--- FAIL:`、`ok`/`FAIL` 包摘要）、**pytest**（现有规则显式列入）
- [x] 更新 Step 4 覆盖率解析：新增 `node-test`（读 parser 产出的 `coverage-summary.json`）、`go-cover`（`total:` 行 → lines；branches/functions = null）、`coverage-py`（`totals.percent_covered` / `percent_covered_branches`；functions = null）
- [x] 更新 Step 5：`coverage.measured` 与 `by_framework[].measured` 类型为 `{lines: number, branches: number | null, functions: number | null}`
- [x] 更新 Step 5 加权平均公式：null 维度不参与分子分母；全 null 时该维度 overall 为 null
- [x] 更新 Step 5 `coverage.pass` 规则：null 维度跳过阈值比较（视为通过）；overrides 分组同样跳过 null
- [x] 更新报告 JSON 示例，展示 Go 框架 null 维度与混合框架加权平均场景

## 阶段 6: unit-test-evaluator Agent 更新

- [x] U1 校验：`coverage.measured.branches` / `functions` 接受 `null` 为合法值
- [x] U3 规则：null 维度存在但 `coverage.pass === true` 时不失败
- [x] Step 5 findings 模板：null 维度标注 `"N/A (框架不支持)"`（global measured 与 `by_framework` 均适用）
- [x] 确认 Evaluator 不重新计算覆盖率、不引用 HTML 报告路径

## 阶段 7: Python 工具链扩展

- [x] 在 `plugins/dev-team/utils/test-framework.py` 的 `detect_test_framework` 中，于 Node 检测之前或适当位置增加 `go.mod` 检测，返回 `("go", "go test ./...")`
- [x] 在 `get_test_file_extension` / `get_test_dir` 映射中补充 `go`、`node-test` 条目（若调用方需要）
- [x] 在 `plugins/dev-team/utils/test-runner.py` 的 `_execute_test_command` 中新增 `go` 分支：执行 `go test ./...`
- [x] 在 `test-runner.py` 中新增 `node-test` 分支：执行 `node --test`
- [x] 在 `_parse_test_result` 中新增 go（`--- PASS:`/`--- FAIL:`）与 node-test（`✔`/`✖`）输出计数解析

## 阶段 8: 构建验证与版本

- [x] 运行 `pnpm run -C ./plugins/dev-team/bin check`，确认 TypeScript 与 schema 无类型错误
- [x] 按 CLAUDE.md 规则升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号
