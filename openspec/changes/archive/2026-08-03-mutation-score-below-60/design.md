# 设计: mutation-score-below-60

> **变更**: mutation-score-below-60
> **日期**: 2026-07-31
> **工作流**: test-only（观察既有架构；默认不改生产逻辑）

---

## 架构组件

本变更为质量门禁补测：被测面是 `plugins/dev-team` 测试执行管线中 mutation score < 60 的七个已实现模块。管线关系（detect → execute → report）保持不变。

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Framework Registry | 硬编码八框架的 `version_command`、shell/cmd 模板、`coverage_format` / `coverage_output`、`mutation_framework`、`config_flag`；提供查找与版本探测 | `plugins/dev-team/bin/src/lib/test-framework.ts` | `../schemas`（`TestFramework`）、`./exec-command` | TypeScript |
| Test Runner | 单 plan 执行：清空 `reportDir`、`preparePlanArtifacts`、展开占位符、条件 stdout 重定向、`execSync` 跑命令、垂直解析、可选 Stryker mutation | `plugins/dev-team/bin/src/lib/test-runner.ts` | `test-framework`、`test-parser/*`、`stryker-config`、`config`、`test-exclude`、`schemas` | TypeScript / `child_process` / fs |
| Stryker Config Resolver | 始终生成临时 `stryker.config.<rand>.json`；`mutate` 路径归一为相对 `rootPath` 的 POSIX；`jsonReporter.fileName` → `{reportDir}/mutation.json` | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | `crypto` / `fs` / `path` | TypeScript |
| Test Report Generator | 写原子 `report.json` 与汇总 `summary.json`；聚合 coverage / mutation；`derivePlanId` 目录 id；suite 级 overrides | `plugins/dev-team/bin/src/lib/test-report.ts` | `ExecutionResult`、`schemas`、`config`、`glob`、`test-exclude`、`test-framework` | TypeScript / fs |
| Go Output Parser | 解析 `go test -json` NDJSON；终端 Action `pass`/`fail`/`skip`；推导 `*_test.go` / `.go` 路径 | `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts` | `zod`、`./types` | TypeScript |
| Text Output Parser | 多层 fallback：bun / cargo / node / pytest → 通用正则 → 行启发式 | `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts` | `./types` | TypeScript |
| Detect Frameworks Command | 读 `config.tests[]`，匹配文件到 suite，生成仍含占位符的 `TestPlan`（shell/cmd） | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `test-framework`、`config`、`glob`、`test-exclude`、`project-root`、`schemas` | TypeScript |
| Colocated 单测（既有） | 对上表公共 API 的 vitest/`vite-plus` 单测；本次补强对象 | 见「变更清单」测试文件 | 对应源模块；常用 mock：`fs`、`execCommand`、`execSync`、`detectFrameworkVersion` | TypeScript / vite-plus |
| Vertical Parser Barrel（边界，≥60） | `parsePlanArtifacts` 分发到 go/js/text + coverage；execute 主入口 | `plugins/dev-team/bin/src/lib/test-parser/index.ts` | go/text/js/coverage parsers | TypeScript |
| test-execution CLI（边界，≥60） | 编排 detect → `executePlanEntry` → `generateSubReport` / `generateSummaryReport` | `plugins/dev-team/bin/src/commands/test-execution.ts` | 上述组件 | TypeScript / CLI |

### 组件协作（观察）

```
config.tests[]
    → runTestDetectFrameworks
        → getFrameworkConfig + detectFrameworkVersion
        → TestPlan[]（script 含未展开占位符）
    → executePlanEntry(plan, projectRoot, { reportsDir, … })
        → preparePlanArtifacts / expandCommandTemplate / applyResultsRedirect
        → runCommand → parsePlanArtifacts
            → parseGoOutput | parseTextOutput | …
        →（可选）resolveStrykerConfig → Stryker → mutation.json
    → generateSubReport → <planId>/report.json
    → generateSummaryReport → summary.json
```

---

## 变更清单

本次默认**只改/补强 colocated 测试**；生产源码仅在单测证实真实 bug 时最小修复（AC-9）。

### 新增文件

<!-- 无强制新增源文件。夹具优先落在既有 colocated `*.test.ts`；若单元范围不足可增量扩展 `plugins/dev-team/bin/__tests__/`。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | 补强 `getFrameworkConfig` / `detectFrameworkVersion` 断言（字面量、semver 门控、未知框架错误文案） | AC-1；杀 StringLiteral / ConditionalExpression |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | 补强 `executePlanEntry`：prepare、占位符展开、重定向、空命令、mutation 开关、失败路径 | AC-2；覆盖 NoCoverage 与分支 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts` | 补强路径归一（绝对/盘符/相对/root 相等）、`plugins` / `testRunner` 映射、unsupported framework | AC-3 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | 补强 sub/summary：计数、conclusion、coverage/mutation 聚合与 overrides、`derivePlanId` 边界 | AC-4 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | 补强 NDJSON 终端事件、fail 优先于后续 pass、output 拼接、空/畸形行、源文件推导 | AC-5 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | 补强四层框架标记、近失配负向、generic/heuristic 边界 | AC-6 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | 补强 scope/exclude、auto-scan、无 suites、config_flag 校验、脚本占位符未展开 | AC-7 |
| 上表七个生产 `.ts`（可选） | 仅真实 bug 的最小修复 | AC-9；须在变更说明单列 |

### 公共函数 / API

生产公共 API **不变**（无新增 test-only `export`）。下列为补测入口面：

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `getFrameworkConfig` | `test-framework.ts` | 既有 | `function getFrameworkConfig(framework: string): FrameworkConfig` | 浅拷贝 registry 条目；未知名抛错 |
| `detectFrameworkVersion` | `test-framework.ts` | 既有 | `function detectFrameworkVersion(framework: string, cwd: string): string` | 跑 `version_command`；失败或无 semver → `''` |
| `executePlanEntry` | `test-runner.ts` | 既有 | `function executePlanEntry(entry: TestPlan, projectRoot: string, options: { files?: string[]; timeout?: number; noMutation?: boolean; mutationDiffFiles?: string[]; reportsDir: string }): ExecutionResult` | 单 plan 全流程；prepare 失败 → `emptyResult` |
| `resolveStrykerConfig` | `stryker-config.ts` | 既有 | `function resolveStrykerConfig(rootPath: string, sourceFiles: string[], framework: string, reportDir: string): { configPath: string; tempDirPath: string }` | 支持 `jest` / `vitest` / `vite-plus` |
| `generateSubReport` | `test-report.ts` | 既有 | `function generateSubReport(framework: string, result: ExecutionResult, projectRoot: string, reportsDir: string, planDirectory: string): TestExecutionSubReport` | 写 `<planId>/report.json` |
| `generateSummaryReport` | `test-report.ts` | 既有 | `function generateSummaryReport(subReports: TestExecutionSubReport[], projectRoot: string, reportsDir: string): TestExecutionSummaryReport` | 写 `summary.json` |
| `parseGoOutput` | `go-parser.ts` | 既有 | `function parseGoOutput(content: string): ParsedTestResult` | NDJSON 文本 → 结构化结果 |
| `parseTextOutput` | `text-parser.ts` | 既有 | `function parseTextOutput(output: string): ParsedTestResult` | 多层 fallback |
| `runTestDetectFrameworks` | `test-detect-frameworks.ts` | 既有 | `function runTestDetectFrameworks(options: TestDetectFrameworksOptions): TestDetectFrameworksResult` | `{ detected, plan }` |

私有辅助（`preparePlanArtifacts`、`normalizeSourceFilesForStryker`、`mapGoStatus`、`parseBunOutput`、`derivePlanId` 等）**不导出**；通过公共 API 的可观测行为间接杀变异体（AC-8）。

### 类型定义

<!-- 无新增/修改类型定义；下列为既有被测类型，供 test-design 对照。 -->

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `FrameworkConfig` | `test-framework.ts` | 既有 | `framework`、`version_command`、`shell`/`cmd` builders、`coverage_format`、`coverage_output`、`default_glob`、`mutation_framework`、`config_flag` |
| `ExecutionResult` | `test-runner.ts` | 既有 | `framework`、`exitCode`、`testCases`、`coverage`、`mutation?`、`durationMs`、`testFiles`、`sourceFiles`、`error?`、`planId`、`reportDir`、`resultsFile?` |
| `TestDetectFrameworksOptions` | `test-detect-frameworks.ts` | 既有 | `{ files?: string[]; projectRoot?: string }` |
| `ParsedTestResult` / `TestCase` | `test-parser/types.ts` | 既有 | 解析器统一输出；含可选 `error` |
| `TestPlan` / `TestDetectFrameworksResult` | `schemas/test-detect-frameworks.schema.ts` | 既有 | plan 与 detect 输出契约 |
| `TestExecutionSubReport` / `TestExecutionSummaryReport` | `schemas`（test-execution-output） | 既有 | 报告 JSON 形状 |

### 配置

<!-- 本变更不修改 plugin.json / openspec config / Stryker 阈值键；临时 stryker/bunfig 为运行时产物。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `FRAMEWORK_REGISTRY` | 八键：`jest`/`vitest`/`vite-plus`/`bun`/`rust`/`node-test`/`go`/`pytest` | 被 detect / runner / report 读取 | 源码常量 |
| `TestPlan` | `directory`、`scope`、`framework`、`coverage_*`、`mutation_*`、`script.{shell,cmd}` | detect 产出 → runner 消费 | 内存 / MCP 输出 |
| plan 目录产物 | `results.json`/`results.ndjson`/`results.txt`、coverage 文件、`mutation.json`、`report.json` | `reports/test/<planId>/` | 文件系统 |
| `ExecutionResult` | 见类型表 | 投影为 sub-report | 内存 |
| `ParsedTestResult` | `total/passed/failed/skipped`、`testCases`、`testFiles`、`sourceFiles`、`error?` | go/text 解析器返回；barrel 再挂 `coverage` | 内存 |
| Stryker 临时配置 | `mutate`、`testRunner`、`plugins`、`jsonReporter.fileName`、`ignoreStatic`、`timeoutMS` | 落在 absCwd；用后删除 | 短暂文件 |
| 基线 mutation 缺口 | 每文件 `killed`/`survived`/`noCoverage` → score | 验收对照 | 归档 `mutation.json` |

### 框架垂直产物（与 runner 常量一致）

| 框架 | 结果文件 | coverage_output | native outputFile | mutation_framework |
|------|----------|-----------------|-------------------|--------------------|
| jest / vitest / vite-plus | `results.json` | `coverage-summary.json` | 是 | `stryker-js` |
| bun | `results.txt` | `lcov.info` | 否（`>`） | `null` |
| go | `results.ndjson` | `func-summary.txt` | 否（段级 `>`） | `null` |
| rust | `results.txt` | `coverage-summary.json` | 否 | `null` |
| pytest | `results.txt` | `coverage.json` | 否 | `null` |
| node-test | `results.txt` | `results.txt` | 否 | `null` |

### 各模块内部结构（观察）

#### `test-framework.ts`（AC-1）

- `FRAMEWORK_REGISTRY`：模板含 `{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` / `{files}` / `{directory}` / `{coverprofile_file}` 等占位符。
- `isVersionAtLeast` + `extractSemver`：仅 jest 在 `≥ 29.5.0` 时注入 `--randomize`。
- `getFrameworkConfig`：校验后 `{ ...entry }` 浅拷贝。
- `detectFrameworkVersion`：`execCommand(version_command, { cwd, timeout: 30_000 })`，合并 stdout/stderr 抽 semver。

#### `test-runner.ts`（AC-2）

- `NATIVE_OUTPUT_FILE_FRAMEWORKS` = jest/vitest/vite-plus → 不追加 `>`。
- `preparePlanArtifacts`：相对 absCwd 的占位符；bun 写临时 bunfig overlay。
- `expandCommandTemplate` / `applyResultsRedirect`：按框架段级重定向（go/rust/pytest/default）。
- `runMutationPhase`：需 `mutation_framework` 且非 `noMutation`、有未 exclude 的 sourceFiles；经 `resolveStrykerConfig` + `runCommand`。
- `derivePlanId`：与 `test-report` 同算法，内联避免循环依赖。

#### `stryker-config.ts`（AC-3）

- `normalizeSourceFilesForStryker`：`\` → `/`；绝对/盘符相对化；root 相等 → `'.'`。
- `resolveTestRunner`：`jest`→`jest`；`vitest`/`vite-plus`→`vitest`；否则抛错。
- `resolvePluginPackage`：`jest` / `vitest` → `@stryker-mutator/*-runner`；未知 runner 回退 `` `@stryker-mutator/${testRunner}` ``。

#### `test-report.ts`（AC-4）

- `generateSubReport`：汇总用例计数、`error_cases`、coverage block、mutation透传、可选 `findings`。
- `generateSummaryReport`：`aggregateTotals` → `collectProblems` → coverage/mutation 聚合 → `determineConclusion`（`error`/`fail`/`pass`）→ `plans[]`。
- Mutation 分数聚合公式（报告内）：`(killed + timeout) / (total - ignored - compileError - runtimeError) * 100`（与本变更验收用的 per-file `killed/(killed+survived+noCoverage)` 不同语境）。

#### `go-parser.ts`（AC-5）

- 仅处理带 `Test` 字段且 Action 为 `pass`/`fail`/`skip` 的事件；已失败用例忽略后续 `pass`。
- `output` Action：仅对已见且 failed 的用例追加 `errorMessage`。
- `deriveGoTestFiles`：`^Test([A-Z].*)` → snake_case + `_test.go`；`deriveGoSourceFiles`：`_test.go` → `.go`。

#### `text-parser.ts`（AC-6）

- Layer1 顺序：bun → cargo → node → pytest；任一命中即返回。
- Layer2：`Tests: N passed…` / `N passed, M failed` / `N tests passed`。
- Layer3：`PASSED`/`FAILED`/`SKIP`/`ok`/`not ok` 启发式；`total===0` → `Unable to parse test output`。

#### `test-detect-frameworks.ts`（AC-7）

- `files` 省略 → `collectFiles` 自动扫描（排除 `node_modules`/`.git`/`dist` 等）；`files: []` → 空结果。
- suite 优先级：数组顺序，首个 `isInSuiteScope` 命中。
- `validateSuiteConfig`：声明 `config` 但 `config_flag==null` 或无法解析 absConfig → 抛错。
- 脚本生成：占位符**不**展开；无 `cd` 前缀（execute 以 absCwd 运行）。

---

## 路由/API 设计

<!-- 本变更不新增 HTTP 路由。既有 MCP/CLI 入口仅作被测面边界说明。 -->

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP tool | `test_detect_frameworks` | 调用 `runTestDetectFrameworks` | `TestDetectFrameworksOptions`（files/projectRoot） | `TestDetectFrameworksResult` | 无（本地 MCP） |
| CLI（编排） | `dev-team test-execution` | 间接调用 runner + report（本变更不改命令行为） | CLI flags / change 上下文 | `reports/test/summary.json` 等 | 无 |

---

## 依赖

### 运行时依赖

- Node 内置 `fs` / `path` / `child_process` / `crypto` — 文件、进程、临时配置
- `zod` — Go NDJSON 事件校验
- `./exec-command` — 框架版本探测（非裸 `execSync`）
- 各测试框架 CLI / StrykerJS — 仅 execute/mutation 路径运行时需要；单测侧以 mock 替代

### 构建/测试依赖

- `vite-plus/test`（vitest API）— colocated 单测
- 既有 `plugins/dev-team/bin/__tests__/test-setup.ts` — 全局 spy（如 `detectFrameworkVersion` → `'99.0.0'`）
- Stryker（或 `dev-team test-execution` mutation 阶段）— 验收时重算七文件 score（AC-1…AC-7）

---

## Decisions（代码中已体现的架构模式）

| ID | 观察 | 含义 |
|----|------|------|
| D1 | 框架命令在 `FRAMEWORK_REGISTRY` 硬编码，不在 `config.json` | 补测应断言模板字面量与字段，而非改产品配置面 |
| D2 | detect 保留占位符，execute 展开并落盘 planDir | 测 detect 时断言占位符仍在；测 runner 时断言展开与重定向 |
| D3 | 垂直产物按框架约定文件名；解析读文件而非脏 stdout | go/text 单测直接喂文件内容字符串即可 |
| D4 | 无私有符号的 test-only `export`；knip 将「仅测试引用的 export」视为死代码 | 补测必须经公共 API / 可观测 IO（AC-8） |
| D5 | `test-runner` 内联 `derivePlanId`，避免与 `test-report` 循环依赖 | 两处算法须保持一致；可通过各自公共出口间接覆盖 |
| D6 | Stryker 始终写临时配置，从不改用户长期 config | 测后清理 `configPath`；断言用户文件未变 |
| D7 | text-parser 多层短路；go-parser 忽略非终端 Action | 负向近失配输入用于杀 Regex/LogicalOperator 存活体 |

---

## 验收映射

| AC | 设计落点 |
|----|----------|
| AC-1 | 补强 `test-framework.test.ts` 针对 registry / version API |
| AC-2 | 补强 `test-runner.test.ts` 针对 `executePlanEntry` 全分支 |
| AC-3 | 补强 `stryker-config.test.ts` 路径归一与 runner/plugin 映射 |
| AC-4 | 补强 `test-report.test.ts` 针对 sub/summary 聚合与边界 |
| AC-5 | 补强 `go-parser.test.ts` 针对 NDJSON 语义 |
| AC-6 | 补强 `text-parser.test.ts` 针对分层 fallback 与近失配 |
| AC-7 | 补强 `test-detect-frameworks.test.ts` 针对 detect/plan 行为 |
| AC-8 | 仅 colocated（或既有 `__tests__`）用例；无新 production export；既有相关单测保持通过 |
| AC-9 | 生产改动默认无；若有则最小修复并单列说明 |

分数定义与归档报告一致：`killed / (killed + survived + noCoverage)`；证据为更新后 Stryker JSON（或等价 mutation 产物）中对应 `files` 条目。

---

## 待决问题

- 最终验证是「仅 mutate 这 7 个文件」还是「整包 vite-plus suite 全量 mutation」——验收以各文件 score ≥ 60 为准，执行策略可在 test-execution 阶段按耗时选择（与 proposal 一致）。
