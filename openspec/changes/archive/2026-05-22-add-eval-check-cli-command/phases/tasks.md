# 任务: add-eval-check-cli-command

> **变更**: add-eval-check-cli-command
> **日期**: 2026-05-22
> **基于**: proposal.md, test-design.md, design.md

---

## 阶段 1：基础设施

- [x] 在 `plugins/dev-team/bin/package.json` 的 devDependencies 中添加 `vitest@^3.0.0`，运行 `npm install` 安装
- [x] 在 `plugins/dev-team/bin/package.json` 的 scripts 中添加 `"test": "vitest run"` 和 `"test:watch": "vitest"` 脚本
- [x] 创建 vitest 配置文件 `plugins/dev-team/bin/vitest.config.ts`，设置 `test.globals: true`、`test.include: ["src/**/*.test.ts", "../../../openspec/changes/**/tests/*.test.ts"]`

## 阶段 2：核心逻辑实现

- [x] 在 `plugins/dev-team/bin/src/commands/eval-check.ts` 中实现 `checkPriorPhases(entries, priorPhases)` 纯函数，复用 `eval-json.ts` 的 `checkGate()`，返回 `{ passed, missing }`
- [x] 在 `eval-check.ts` 中实现 `checkTimestampOrder(entries, priorPhases)` 纯函数：收集每个前置阶段的最新 pass 记录 timestamp，验证 timestamp 按阶段顺序单调递增（使用 `>=` 比较），返回 `{ passed, order_valid, issues }`
- [x] 在 `eval-check.ts` 中实现 `checkBacktrack(entries, priorPhases)` 纯函数：对每个前置阶段按 timestamp 降序取最新条目，检查 `backtrack_to` 字段是否为非 null，返回 `{ passed, active_backtrack_phases }`
- [x] 在 `eval-check.ts` 中实现 `determinePhaseState(entries, currentPhase)` 纯函数：取当前阶段最新条目判定状态 — 无条目返回 `"first_run"`，最新 verdict 为 `"fail"` 返回 `"retry"`，最新 verdict 为 `"pass"` 返回 `"passed"`
- [x] 在 `eval-check.ts` 中实现 `buildEvalCheckResult(options)` 函数：聚合四项检查结果和阶段状态，返回 `EvalCheckResult` 对象（包含 `passed`、`phase`、`prior_phases`、`block_reasons`、`phase_state`、`details`）
- [x] 在 `eval-check.ts` 中实现 `registerEvalCheckCommand(cli)`：使用 CAC 注册 `eval-check` 子命令，定义 `--change`、`--phase`、`--json` 选项；action handler 中实现参数验证（--change 和 --phase 必填、phase 是否合法、变更目录存在性验证）、依次调用核心函数、格式化输出、设置退出码
- [x] 验证 `schema_version` 不匹配时输出警告但不阻断：在注册命令的 action handler 中，读取 entries 后检查是否有条目的 `schema_version` 与 `SCHEMA_VERSION` 不一致，不一致时输出警告消息到 stderr

## 阶段 3：CLI 入口注册

- [x] 在 `plugins/dev-team/bin/src/index.ts` 中导入 `registerEvalCheckCommand` 并在 `main()` 函数中调用（与 `registerEvalLogCommand` 并列）
- [x] 构建项目：运行 `npm run build` 验证编译通过，生成更新后的 `dev-team-bundle.js`

## 阶段 4：单元测试

- [x] 创建 `openspec/changes/add-eval-check-cli-command/tests/eval-check.unit.test.ts`，对 `checkPriorPhases` 进行测试（覆盖：所有前置阶段有 pass 记录返回 passed=true、缺少前置阶段返回 passed=false 并在 missing 中列出、所有前置阶段缺失、空 entries、首个阶段无前置阶段时 passed=true）
- [x] 在单元测试文件中添加 `checkTimestampOrder` 的测试（覆盖：timestamp 单调递增通过、timestamp 逆序阻断、timestamp 完全相同视为通过、仅有单个前置阶段时无需顺序检查）
- [x] 在单元测试文件中添加 `checkBacktrack` 的测试（覆盖：无 backtrack 通过、前置阶段最新条目有活跃 backtrack 阻断、前置阶段非最新条目有 backtrack 但最新条目已清除视为通过）
- [x] 在单元测试文件中添加 `determinePhaseState` 的测试（覆盖：无条目返回 `"first_run"`、最新条目 verdict fail 返回 `"retry"`、最新条目 verdict pass 返回 `"passed"`、多条条目时取最新判定）
- [x] 在单元测试文件中添加 `buildEvalCheckResult` 的测试（覆盖：所有检查通过时输出结构正确包含所有必填字段、阻断时 block_reasons 聚合所有问题、phase_state 正确传递）、以及 schema_version 不匹配时警告输出的验证
- [x] 运行 `npm test` 验证单元测试全部通过

## 阶段 5：集成测试

- [x] 创建 `openspec/changes/add-eval-check-cli-command/tests/eval-check.integration.test.ts`，编写辅助函数：创建临时变更目录结构和 eval.json、清理
- [x] 在集成测试文件中添加 AC-1 的测试：创建包含 01-requirements 和 02-test-design pass 记录的 eval.json，运行 `eval-check --change <tmp> --phase 03-dev-proposal`，验证退出码 0
- [x] 在集成测试文件中添加 AC-4/AC-5 的测试：同 AC-1 场景并添加 `--json` 标志，验证 stdout 可被 `JSON.parse()` 解析，验证输出包含 `passed`、`phase`、`prior_phases`、`block_reasons`、`phase_state` 字段且类型正确
- [x] 在集成测试文件中添加 AC-11 的测试：运行 `eval-check --change non-existent --phase 01-requirements`，验证输出包含有意义的错误信息且退出码为 1
- [x] 在集成测试文件中添加非法 phase 名称的测试：运行 `eval-check --change test --phase invalid-phase`，验证退出码为 1 并输出有意义的错误信息
- [x] 运行 `npm test` 验证集成测试全部通过

## 阶段 6：手动验证脚本

- [x] 创建 `openspec/changes/add-eval-check-cli-command/tests/manual-eval-check.js`：提供预设场景（全部通过、缺失阶段、时间戳逆序、回溯标记、首次运行、重试、已通过），调用 CLI 命令并输出结果
- [x] 在脚本注释中记录手动验证的使用方法

## 阶段 7：验证与文档

- [x] 执行完整测试套件：`npm test`，确认所有单元测试和集成测试通过
- [x] 手动验证：运行 `node dev-team-bundle.js eval-check --help` 确认帮助信息正确显示 `--change`、`--phase`、`--json` 参数
- [x] 手动验证：对一个真实的、已完成 01-requirements 和 02-test-design 阶段的变更运行 `eval-check --change <real-change> --phase 03-dev-proposal --json`，确认输出合理
- [x] 确认 `eval-check.py`（归档验证脚本）未被修改，确认 `eval-json.ts`、`workflow.ts`、`change.ts` 未被修改
