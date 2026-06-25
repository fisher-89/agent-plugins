# 任务: remove-parse-node-test-coverage-script

> **变更**: remove-parse-node-test-coverage-script
> **日期**: 2026-06-24

---

## 阶段 1: 简化 node-test 框架注册表配置

- [x] 在 `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` 中将 `node-test` 条目的 `coverage_cmd` 改为 `'node --test --experimental-test-coverage'`，去除 `tee` 管道和 `parse-node-test-coverage.mjs` 调用
- [x] 将 `coverage_output` 改为 `'coverage/node-test-output.txt'`
- [x] 将 `coverage_artifacts` 改为 `['coverage/node-test-output.txt']`
- [x] 确认 `coverage_format`、`coverage_cleanup`、`default_glob` 保持不变
- [x] 确认其他七个框架条目未被修改

## 阶段 2: 更新 unit-test-executor Agent 步骤 4

- [x] 在 `plugins/dev-team/agents/unit-test-executor.md` 步骤 4 中，将 `node-test` 覆盖率解析描述从读取 `coverage-summary.json` 改为读取 `coverage/node-test-output.txt` 原始文本表格
- [x] 提供 regex 示例：`/all\s+files\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)\s*\|\s*([\d.]+)/i`，提取 `% Lines` 列 → `lines`，`% Branch` 列 → `branches`，`% Funcs` 列 → `functions`
- [x] 确认描述与 `go-cover` 模式一致：原始文本文件 + regex 提取，无需外部脚本
- [x] 确认不再引用 `parse-node-test-coverage.mjs` 或 `coverage-summary.json`

## 阶段 3: 删除解析脚本及其测试

- [x] 删除 `plugins/dev-team/scripts/parse-node-test-coverage.mjs`
- [x] 删除 `plugins/dev-team/scripts/parse-node-test-coverage.test.mjs`

## 阶段 4: 更新测试断言

- [x] 在 `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` 中：
  - 更新 `NEW_FRAMEWORK_EXPECTED['node-test']` 的 `coverage_cmd`、`coverage_output`、`coverage_artifacts`
  - 更新 `node-test` 测试用例：`coverage_cmd` 断言改为不包含 `tee`、`parse-node-test-coverage.mjs`、`|`
- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` 中：
  - 更新 `node-test plan (AC-5)` 测试用例：`coverage_cmd` 断言不再包含 `parse-node-test-coverage.mjs`

## 阶段 5: 确认完整性

- [x] 运行 `pnpm run -C ./plugins/dev-team/bin check`，确认 TypeScript 与 schema 无类型错误
- [x] 全局搜索 `parse-node-test-coverage` 确认无残留引用（dev-team-mcp.cjs 中若有引用需评估是否需更新——注意 `parse-node-test-coverage` 在 mcp.cjs 中仅出现在 `node --test` 命令字符串中，随注册表更新自动清理；`openspec/` 下 spec 和 archive 文件的引用是历史记录，无需修改）
- [x] 确认 `coverage_summary: istanbul` 路径不变——变更前后 node-test 维度均为三个 `number`（非 null），门控行为不受影响
