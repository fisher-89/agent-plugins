# 任务: add-dev-team-cli-eval-log

> **变更**: add-dev-team-cli-eval-log
> **日期**: 2026-05-21
> **基于**: design.md

---

## 阶段一：TypeScript 项目骨架

- [x] 1.1 创建 `plugins/dev-team/bin/package.json`，声明 esbuild (^0.20) 和 cac (^6.x) 为 dependencies（cac 必须为 dependencies 而非 devDependencies，因为 esbuild 需要将其内联到 bundle 中）、build script（`esbuild src/index.ts --bundle --platform=node --outfile=dev-team-bundle.js --banner:js="#!/usr/bin/env node"`）、start script 和 type 字段 (commonjs)
- [x] 1.2 创建 `plugins/dev-team/bin/tsconfig.json`，设置 target ES2020、module commonjs、strict mode、outDir dist、rootDir src、declaration true
- [x] 1.3 创建 `plugins/dev-team/bin/src/` 目录结构：`commands/`、`lib/`

## 阶段二：底层库实现

- [x] 2.1 创建 `plugins/dev-team/bin/src/lib/change.ts`，实现 `resolveChangeDir(changeName: string): string` 函数 — 拼接 `openspec/changes/<change-name>/phases` 路径，使用 `path.resolve()` 规范化
- [x] 2.2 在 `plugins/dev-team/bin/src/lib/change.ts` 中导出 `getPhasesDir(changeName: string): string` 返回 phases 子目录路径
- [x] 2.3 创建 `plugins/dev-team/bin/src/lib/workflow.ts`，定义 `PHASES` 常量数组（7 个阶段按顺序从 `01-requirements` 到 `07-acceptance`）
- [x] 2.4 在 `plugins/dev-team/bin/src/lib/workflow.ts` 中实现 `getPhaseIndex(phase: string): number` — 返回阶段在 `PHASES` 数组中的索引
- [x] 2.5 在 `plugins/dev-team/bin/src/lib/workflow.ts` 中实现 `getPriorPhases(phase: string): string[]` — 返回给定阶段之前的所有阶段 ID 列表。若 phase 为第一阶段则返回空数组。若 phase 不在 `PHASES` 中则返回空数组（容错行为）
- [x] 2.6 创建 `plugins/dev-team/bin/src/lib/eval-json.ts`，实现 `readEvalJson(phasesDir: string): any[]` — 读取 eval.json 文件并解析为 JS 数组。文件不存在时返回空数组 `[]`。JSON 解析失败时抛出包含解析错误详细信息的异常
- [x] 2.7 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 中实现 `validateVerdict(verdict: any): void` — 校验 verdict 必须是 `"pass"` 或 `"fail"`，否则抛出异常
- [x] 2.8 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 中实现 `validateReportLength(report: string): void` — 校验 report 不超过 500 字符，否则抛出异常
- [x] 2.9 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 中实现 `buildEntry(params: BuildEntryParams): object` — 构建包含所有必要字段的条目对象。自动填充 `timestamp` (new Date().toISOString()) 和 `schema_version` ("1.0")。只有 params 中明确包含的 `backtrack_to` 字段才写入条目（允许 undefined 时为 null）
- [x] 2.10 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 中实现 `computeAttempt(entries: any[], phase: string, explicitAttempt?: number): number` — 若提供了 `explicitAttempt` 直接返回，否则从 entries 中统计 phase 匹配的条目数 +1
- [x] 2.11 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 中实现 `checkGate(entries: any[], priorPhases: string[]): { passed: boolean; missing: string[] }` — 检查每个前置阶段是否至少有一条 verdict 为 "pass" 的条目。返回检查结果和缺少 pass 记录的阶段列表
- [x] 2.12 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 中实现 `appendEntry(phasesDir: string, entry: object): void` — 确保 phasesDir 目录存在 (recursive mkdir)，读取 eval.json（不存在时为 `[]`），追加 entry 到数组末尾，写入 eval.json（2 空格缩进，`JSON.stringify(data, null, 2) + "\n"`）
- [x] 2.13 在 `plugins/dev-team/bin/src/lib/eval-json.ts` 中实现 `validateItemsJson(itemsStr: string): any[]` — 解析 items JSON 字符串，解析失败时抛出包含指导信息的异常

## 阶段三：CLI 入口和命令实现（基于 cac）

本阶段使用 `cac` 框架替代手动参数解析。与原有设计的关键区别：
- 无 `parseArgs()` 函数 — `cac` 自动解析 `--key value` 格式参数
- 无 `showHelp()` / `showHelpEvalLog()` 函数 — `cac` 根据 `.option()` 声明自动生成帮助文本
- `index.ts` 仅创建 cac 实例并注册命令，不包含手动路由逻辑
- eval-log 的帮助文本（参数描述、用法示例）与命令定义同处于 `commands/eval-log.ts`

- [x] 3.1 创建 `plugins/dev-team/bin/src/index.ts`，实现：
  - 导入 `cac` 和 `registerEvalLogCommand`
  - 导出一个 `main(argv: string[] = process.argv.slice(2)): void` 函数：
    - 创建 cac 实例：`const cli = cac('dev-team')`
    - 设置顶级用法描述：`cli.usage('[command] [options]')`
    - 调用 `registerEvalLogCommand(cli)` 注册 eval-log 子命令
    - 调用 `cli.parse(argv)` 开始解析
  - 在模块顶级使用 `require.main === module` 判断来决定是否自动执行 `main()`
  - 不包含任何手动帮助输出、参数解析或命令路由逻辑 — 这些全部由 cac 和命令模块处理

- [x] 3.2 创建 `plugins/dev-team/bin/src/commands/eval-log.ts`，实现：
  - 导出 `registerEvalLogCommand(cli: CAC): void` 函数
  - 使用 `cac` 声明式 API 定义 eval-log 命令：
    ```typescript
    cli
      .command('eval-log', 'Append evaluation result to eval.json for a given phase')
      .option('--change <name>', 'Change name (corresponds to openspec/changes/<name>)', { required: true })
      .option('--phase <phase>', 'Phase identifier (e.g. 01-requirements)', { required: true })
      .option('--verdict <verdict>', 'Evaluation verdict: pass or fail', { required: true })
      .option('--report <text>', 'Evaluation report text (max 500 chars)', { required: true })
      .option('--items <json>', 'Checklist items as JSON array string', { required: true })
      .option('--attempt <n>', 'Attempt number (auto-calculated from existing entries if omitted)')
      .option('--backtrack-to <phase>', 'Backtrack target phase identifier')
      .action((options) => { /* 见下方 action 回调逻辑 */ });
    ```
  - 注意：`cac` 将 `--backtrack-to` 自动转换为 camelCase，action 回调中通过 `options.backtrackTo` 访问
  - 注意：`--attempt` 传入时为字符串，action 回调中需 `parseInt(options.attempt, 10)` 转换为数字
  - 在 `.action()` 回调中实现完整的 eval-log 流程：
    - 1) 调用 `validateVerdict(options.verdict)` 校验 verdict，失败时 `console.error` + `process.exit(1)`
    - 2) 调用 `validateReportLength(options.report)` 校验 report 长度，失败时 `process.exit(1)`
    - 3) 调用 `validateItemsJson(options.items)` 解析 items JSON，失败时 `process.exit(1)`
    - 4) 调用 `getPhasesDir(options.change)` 获取 phases 目录路径
    - 5) 调用 `readEvalJson(phasesDir)` 读取已有评估记录
    - 6) 调用 `getPriorPhases(options.phase)` 获取前置阶段列表
    - 7) 若前置阶段非空，调用 `checkGate(entries, priorPhases)` 执行门控检查；不通过则输出缺失阶段列表并退出 code 1
    - 8) 调用 `computeAttempt(entries, options.phase, options.attempt ? parseInt(options.attempt, 10) : undefined)` 计算 attempt
    - 9) 调用 `buildEntry({ phase, verdict, report, items, attempt, backtrack_to: options.backtrackTo || null })` 构建条目
    - 10) 调用 `appendEntry(phasesDir, entry)` 写入 eval.json
    - 11) 输出成功 JSON：`console.log(JSON.stringify({ written: true, phase: options.phase, attempt }))`

## 阶段四：包装脚本和构建

- [x] 4.1 创建 `plugins/dev-team/bin/dev-team` bash wrapper，参照现有 `openspec` wrapper 模式：检测 `dev-team-bundle.js` 是否存在，不存在时输出"错误: dev-team-bundle.js 未找到，请先执行 npm run build"并退出 code 1；存在时执行 `exec node "$SCRIPT_DIR/dev-team-bundle.js" "$@"`
- [x] 4.2 创建 `plugins/dev-team/bin/dev-team.cmd` Windows wrapper，参照现有 `openspec.cmd` 模式：检测 `dev-team-bundle.js` 是否存在，不存在时提示构建并退出 code 1；存在时执行 `node "%SCRIPT_DIR%dev-team-bundle.js" %*`
- [x] 4.3 确认 esbuild 构建命令中的 `--banner:js="#!/usr/bin/env node"` 已添加 shebang（构建后 dev-team-bundle.js 首行为 `#!/usr/bin/env node`）
- [x] 4.4 运行 `npm install`（在 `plugins/dev-team/bin/` 下），安装 esbuild 和 cac
- [x] 4.5 在 `plugins/dev-team/bin/` 下运行 `npm run build`，验证 `dev-team-bundle.js` 成功生成，文件首行为 `#!/usr/bin/env node`，包含 cac 内联代码

## 阶段五：CLI 基本验证（手动冒烟测试）

- [x] 5.1 运行 `bash plugins/dev-team/bin/dev-team --help`，验证输出包含 cac 自动生成的帮助信息（包含 "eval-log" 子命令和 "Usage:"）
- [x] 5.2 运行 `bash plugins/dev-team/bin/dev-team eval-log --help`，验证输出包含 cac 根据 .option() 声明自动生成的参数说明（包含 --change、--phase、--verdict、--report、--items、--attempt、--backtrack-to）
- [x] 5.3 在临时目录中运行 eval-log 完整命令，验证 eval.json 被正确创建和追加
- [x] 5.4 运行 eval-log 缺少必填参数（如 `dev-team eval-log --change test`），验证 cac 退出码为 1 且有对应的必填参数缺失错误信息
- [x] 5.5 删除 `dev-team-bundle.js` 后运行 `dev-team --help`，验证输出 bundle 缺失错误

## 阶段六：Evaluator Agent 更新

- [x] 6.1 更新 `plugins/dev-team/agents/requirements-evaluator.md`：在 Process 步骤中将"计算 attempt 并追加写入 eval.json"替换为"调用 `dev-team eval-log --change <name> --phase 01-requirements --verdict <verdict> --report "<report>" --items '<items>'`"；在 Output 部分改为"输出格式参考 eval.schema.json，但不再直接写入文件，改为准备数据并通过 CLI 写入"
- [x] 6.2 更新 `plugins/dev-team/agents/test-design-evaluator.md`：同 6.1 模式，phase 改为 02-test-design
- [x] 6.3 更新 `plugins/dev-team/agents/dev-proposal-evaluator.md`：同 6.1 模式，phase 改为 03-dev-proposal
- [x] 6.4 更新 `plugins/dev-team/agents/test-gen-evaluator.md`：同 6.1 模式，phase 改为 04-test-gen
- [x] 6.5 更新 `plugins/dev-team/agents/implementation-evaluator.md`：同 6.1 模式，phase 改为 05-implementation
- [x] 6.6 更新 `plugins/dev-team/agents/code-review-evaluator.md`：同 6.1 模式，phase 改为 06-code-review，保留 backtrack_to 设置逻辑 (03-dev-proposal)
- [x] 6.7 更新 `plugins/dev-team/agents/acceptance-evaluator.md`：同 6.1 模式，phase 改为 07-acceptance，保留 backtrack_to 设置逻辑 (01-requirements)
- [x] 6.8 运行 `check_agent_updates.sh` 扫描 7 个 Agent 文件，确认不再包含 "Compute attempt" 和 "Append to eval.json" 等旧模式关键词

## 阶段七：单元测试

- [x] 7.1 创建 `openspec/changes/add-dev-team-cli-eval-log/tests/test_workflow_unit.test.mjs` — 测试 `getPriorPhases()` 对全部 7 个阶段的返回结果：第一阶段返回空数组，第二阶段返回 [01-requirements]，第三阶段返回 [01-requirements, 02-test-design]，依此类推。测试 `PHASES` 常量顺序正确性和内容完整性
- [x] 7.2 创建 `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_json_unit.test.mjs` — 测试 `readEvalJson()` (文件不存在返回空数组、JSON 格式错误抛出异常)、`appendEntry()` (目录自动创建、追加写入、2 空格缩进)、`buildEntry()` (自动填充 timestamp 和 schema_version、backtrack_to 为 null 处理)、`computeAttempt()` (空数组返回 1、有记录返回记录数+1、跨阶段不干扰、显式 attempt 覆盖自动计算)、`checkGate()` (全部 pass 通过、部分 fail 拒绝、空列表通过)、`validateVerdict()`、`validateReportLength()`、`validateItemsJson()`
- [x] 7.3 创建 `openspec/changes/add-dev-team-cli-eval-log/tests/test_change_unit.test.mjs` — 测试 `resolveChangeDir()` 路径拼接正确性、空格和特殊字符处理
- [x] 7.4 创建 `openspec/changes/add-dev-team-cli-eval-log/tests/test_index_unit.test.mjs` — 测试基于 cac 的 CLI 路由：
  - `main(['eval-log', '--change', 'test', '--phase', '01-requirements', '--verdict', 'pass', '--report', 'ok', '--items', '[]'])` 路由到 eval-log 并成功执行
  - `main(['--help'])` 输出 cac 自动生成的帮助信息
  - `main(['eval-log', '--help'])` 输出 eval-log 的 cac 自动生成帮助信息
  - `main(['unknown-command'])` 输出未知命令错误并退出 code 1
  - 使用 `mock.fn()` 拦截 `process.exit`、`console.log`、`console.error` 进行验证
  - 注意：不再测试 `parseArgs()`、`showHelp()`、`showHelpEvalLog()` — 这些函数已被 cac 替代

## 阶段八：最终验证与归档

- [x] 8.1 更新 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（当前版本基础上适当递增）
- [x] 8.2 运行 `bash openspec/changes/add-dev-team-cli-eval-log/tests/check_agent_updates.sh`，确认所有 Agent 文件已迁移
- [x] 8.3 人工审查所有 7 个 Evaluator Agent 提示词，确保 CLI 调用示例中的参数格式正确、phase 值匹配各自阶段
- [x] 8.4 在临时变更目录上运行完整的 eval-log 调用，验证 AC-3 至 AC-13 全部满足
