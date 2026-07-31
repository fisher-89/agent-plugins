# 设计: test-execution-result-file

> **变更**: test-execution-result-file
> **日期**: 2026-07-31

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| test-execution CLI | 决议 `reports/test/` 根；编排 detect → execute → 写 `summary.json` / `report.json` | `plugins/dev-team/bin/src/commands/test-execution.ts` | `test-detect-frameworks`, `test-runner`, `test-report` | TypeScript / CLI |
| test-detect-frameworks | 生成仍含占位符的 `script.shell` / `script.cmd`；不再烤死 report 路径；不再注入 suite cwd `coverage_cleanup` | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `test-framework`, config | TypeScript |
| test-framework registry | 八框架模板与垂直产物约定名；bun → lcov + 显式 config；删除/废弃 `coverage_cleanup` 语义 | `plugins/dev-team/bin/src/lib/test-framework.ts` | `TestFramework` schema | TypeScript |
| test-runner | `preparePlanArtifacts`；条件段级 `>`；跑命令；调用垂直 `parsePlanArtifacts`；扩展 `ExecutionResult`；mutation 读 planDir | `plugins/dev-team/bin/src/lib/test-runner.ts` | `test-parser/*`, `stryker-config`, `test-framework` | TypeScript / `child_process` |
| test-report | `derivePlanId` 目录 id；写 `<planId>/report.json` 与 `summary.json`（含 `plans[]`） | `plugins/dev-team/bin/src/lib/test-report.ts` | `ExecutionResult`, schemas | TypeScript / fs |
| vertical parsers | 按框架从 `reportDir` 读约定产物；横向 coverage-parser 作库调用 | `plugins/dev-team/bin/src/lib/test-parser/**` | fs, zod | TypeScript |
| stryker-config | 临时配置 `jsonReporter.fileName` → `reportDir/mutation.json` | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | fs/path | TypeScript |
| output schema | summary 增加 `plans[]`；路径相关描述对齐 | `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` | zod/v4 | TypeScript |
| detect schema | `coverage_format` 增加 `lcov`；`coverage_output` 语义改为相对 reportDir | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | zod/v4 | TypeScript |
| executor / evaluator agents | 读 `reports/test/summary.json`，经 `plans[]` 定位 `report.json` | `plugins/dev-team/agents/test-execution-executor.md`, `…/test-execution-evaluator.md` | 无 | Markdown |
| phase skill | 核对并同步报告路径文案（若有） | `plugins/dev-team/skills/phase-test-execution/SKILL.md` | 无 | Markdown |

---

## 变更清单

### 新增文件

<!-- 本变更以扩展既有模块为主，不强制新增独立源文件。若实现时将 bunfig overlay / lcov 解析抽离，可落在 `test-parser/` 下，但非验收前置。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 新增 `preparePlanArtifacts`；`executePlanEntry` 增加 `reportsDir`；跑前 mkdir/清空 planDir；展开占位符；条件 `>`；改调 `parsePlanArtifacts`；扩展 `ExecutionResult`；mutation 报告路径改 planDir；best-effort cleanup `tempPaths` | AC-4/5/6/7/9；REQ-TEF-PREP/PARSE/ER |
| `plugins/dev-team/bin/src/lib/test-report.ts` | `derivePlanId` 返回无 `.json` 目录 id 并导出；`generateSubReport` 写 `<planId>/report.json`；`generateSummaryReport` 写 `summary.json` 并填充 `plans[]` | AC-1/2/3 |
| `plugins/dev-team/bin/src/lib/test-framework.ts` | 八框架模板嵌入 `{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` 等；`coverage_output` 改为相对 reportDir 文件名；`coverage_format` 增加 `lcov`；bun 纠正为 lcov + `config_flag`；移除或清空 `coverage_cleanup`（不再驱动 suite cwd 清理） | AC-4/5/8；REQ-TF-TEF |
| `plugins/dev-team/bin/src/lib/test-parser/index.ts` | 新增 `parsePlanArtifacts(framework, reportDir)` 垂直分发；保留 `parseTestOutput` 仅供文本段内部复用（不再作为 execute 主入口） | AC-4/6/11 |
| `plugins/dev-team/bin/src/lib/test-parser/js-parser.ts` | 支持从 `results.json` 读文件解析（非整段 stdout `JSON.parse`）；覆盖率经横向库读 `coverage-summary.json` | AC-4/6；jest/vitest/vite-plus |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts` | 从 `results.ndjson` + `func-summary.txt`（及 `coverage.out` 如需）解析 | AC-5/6 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts` | bun/rust/pytest/node-test：从 planDir 内 `results.txt`（等）读文件再解析 | AC-5/12 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.ts` | 新增 `lcov` 分支（读 `lcov.info` 归一为 `ParsedCoverage`）；仍作横向库 | AC-8 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.ts` | **仅文档/JSDoc**：默认报告路径说明改为 planDir 下 `mutation.json`；解析 API 签名与行为不变（仍接受调用方传入的绝对路径）；实际读路径改动在 `test-runner.ts` | AC-9 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | `jsonReporter.fileName` → 指向 `reportDir/mutation.json`（相对 absCwd 或绝对均可，以能落盘到 planDir 为准）；签名增加 `reportDir` | AC-9 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | script 保留 `{config_args}` / report 占位符不展开；删除 suite cwd `coverage_cleanup` 注入（`rm -rf` 等） | AC-7；C1-b |
| `plugins/dev-team/bin/src/commands/test-execution.ts` | `resolveReportsDir` → `…/reports/test`；向 `executePlanEntry` 传入 `reportsDir`；summary/原子报告新路径 | AC-1 |
| `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` | summary 增加 `plans[]`（`id`/`framework`/`directory`/`path`）；禁止 status 类字段 | AC-3 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | `coverage_format` enum 增加 `'lcov'`；`coverage_output` 描述改为相对 reportDir | 支撑 AC-8 与 plan 校验 |
| `plugins/dev-team/agents/test-execution-executor.md` | 读 `reports/test/summary.json`；经 `plans[]` → `<path>/report.json`；禁止 `<framework>.json` / 旧 `test-execution.json` | AC-10 |
| `plugins/dev-team/agents/test-execution-evaluator.md` | 聚合报告路径改为 `reports/test/summary.json` | AC-10 |
| `plugins/dev-team/skills/phase-test-execution/SKILL.md` | 核对路径文案；若无旧报告路径字面量则可仅确认无需改 | AC-10 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `preparePlanArtifacts` | `plugins/dev-team/bin/src/lib/test-runner.ts` | 新增 | `function preparePlanArtifacts(input: PreparePlanArtifactsInput): PreparePlanArtifactsResult` | 决议占位符 / configArgs / 是否 `>` / 临时文件；默认仅 bun 写临时 bunfig |
| `executePlanEntry` | `plugins/dev-team/bin/src/lib/test-runner.ts` | 修改 | `function executePlanEntry(entry: TestPlan, projectRoot: string, options: { files?: string[]; timeout?: number; noMutation?: boolean; mutationDiffFiles?: string[]; reportsDir: string }): ExecutionResult` | 增加必填 `reportsDir`；内部走 prepare → run → vertical parse |
| `parsePlanArtifacts` | `plugins/dev-team/bin/src/lib/test-parser/index.ts` | 新增 | `function parsePlanArtifacts(framework: string, reportDir: string): ParsedPlanArtifacts` | 垂直读 planDir；返回 testCases/coverage/sourceFiles/testFiles/error 等 |
| `parseTestOutput` | `plugins/dev-team/bin/src/lib/test-parser/index.ts` | 修改（降级为内部复用） | `function parseTestOutput(stdout: string, stderr: string, framework: string): ParsedTestResult` | execute 主路径不再依赖 stdout JSON；文本族可先读文件再调用 |
| `parseCoverageFromFile` | `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.ts` | 修改 | `function parseCoverageFromFile(filePath: string, format: string): ParsedCoverage \| null` | `format` 支持 `'lcov'` |
| `derivePlanId` | `plugins/dev-team/bin/src/lib/test-report.ts` | 新增导出（原私有） | `function derivePlanId(directory: string, framework: string): string` | 目录 id，无 `.json`；`.` → `<framework>` |
| `generateSubReport` | `plugins/dev-team/bin/src/lib/test-report.ts` | 修改 | `function generateSubReport(framework: string, result: ExecutionResult, projectRoot: string, reportsDir: string, planDirectory: string): TestExecutionSubReport` | 写 `path.join(reportsDir, planId, 'report.json')` |
| `generateSummaryReport` | `plugins/dev-team/bin/src/lib/test-report.ts` | 修改 | `function generateSummaryReport(subReports: TestExecutionSubReport[], projectRoot: string, reportsDir: string, planResults?: ExecutionResult[]): TestExecutionSummaryReport` | 写 `path.join(reportsDir, 'summary.json')`；填充 `plans[]`（可由 `planResults` 或 subReports+derivePlanId 投影） |
| `getFrameworkConfig` | `plugins/dev-team/bin/src/lib/test-framework.ts` | 修改 | `function getFrameworkConfig(framework: string): FrameworkConfig` | 返回值字段语义变更（见类型）；签名不变 |
| `resolveStrykerConfig` | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | 修改 | `function resolveStrykerConfig(rootPath: string, sourceFiles: string[], framework: string, reportDir: string): { configPath: string; tempDirPath: string }` | `jsonReporter.fileName` 指向 planDir 下 `mutation.json` |
| `runTestDetectFrameworks` | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 修改 | `function runTestDetectFrameworks(options: TestDetectFrameworksOptions): TestDetectFrameworksResult` | script 保留占位符；移除 coverage_cleanup 注入；签名可保持 |
| `runTestExecution` | `plugins/dev-team/bin/src/commands/test-execution.ts` | 修改 | `async function runTestExecution(options: TestExecutionOptions): Promise<number>` | reports 根改为 `reports/test`；传入 `reportsDir` |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ExecutionResult` | `test-runner.ts` | 修改 | 增量：`planId: string`；`reportDir: string`；`resultsFile?: string`；保留 `error?` |
| `PreparePlanArtifactsInput` | `test-runner.ts` | 新增 | `{ framework: string; absCwd: string; reportDir: string; userConfigPath: string \| null; projectRoot: string }` |
| `PreparePlanArtifactsResult` | `test-runner.ts` | 新增 | `{ configArgs: string; redirectStdoutToResults: boolean; placeholders: PlanPlaceholders; tempPaths: string[]; env?: Record<string, string> }` |
| `PlanPlaceholders` | `test-runner.ts` | 新增 | `{ report_dir: string; results_file: string; coverage_file: string; coverprofile_file?: string; mutation_file?: string }` — 路径建议相对 absCwd，便于命令拼接 |
| `ParsedPlanArtifacts` | `test-parser/index.ts` | 新增 | 在 `ParsedTestResult` 上增加 `coverage: ParsedCoverage \| null`（或并列返回） |
| `FrameworkConfig` | `test-framework.ts` | 修改 | `coverage_format` 联合类型增加 `'lcov'`；`coverage_output` 相对 reportDir；`coverage_cleanup` 删除或忽略；bun `config_flag` 非 null（建议 `'--config'`） |
| `TestExecutionSummaryReport` | `test-execution-output.schema.ts` | 修改 | 增加 `plans: PlanIndexEntry[]` |
| `PlanIndexEntry` | `test-execution-output.schema.ts` | 新增 | `{ id: string; framework: string; directory: string; path: string }` — 无 status |
| `TestPlan` / detect schema | `test-detect-frameworks.schema.ts` | 修改 | `coverage_format` 含 `lcov`；`coverage_output` 描述更新 |

### 配置

<!-- 本变更不修改 openspec/config.json / plugin.json 配置键；用户长期 bunfig / jest config 禁止就地改写。临时 bunfig / stryker 配置为运行时产物，非持久配置键。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| 报告根目录 | 无 `--change`：`{projectRoot}/reports/test/`；有 `--change`：`{projectRoot}/openspec/changes/<change>/reports/test/` | CLI `resolveReportsDir` 决议 | 文件系统目录 |
| `planId` | `directory === '.'` → `"<framework>"`；否则 `sanitize(directory) + "_" + framework`（`\`/`/` → `_`，去尾部 `/`）；**无** `.json` | 目录名 = `id` = `ExecutionResult.planId` | 目录名 |
| plan 目录产物 | 跨框架强制：`report.json`；垂直：见下表；Stryker：`mutation.json` | 隶属于 `reports/test/<planId>/` | 文件系统 |
| `summary.json` | 既有聚合字段 + `plans[]` | 一对多 plan 目录 | `reports/test/summary.json` |
| `plans[]` 元素 | `id`, `framework`, `directory`, `path`（相对 **project root**，POSIX） | 仅索引；成败在 `report.json` / `problems` | 嵌在 summary |
| `ExecutionResult` | 既有字段 + `planId` / `reportDir` / `resultsFile?` | 投影为 `plans[]` 与原子报告 | 内存 → JSON |
| 临时文件 | `bunfig.dev-team-<rand>.toml`；`stryker.config.<rand>.json`；`.stryker-tmp` | 落在 absCwd；用后删 | 短暂文件，不进用户长期 config |

### 垂直产物文件名（终表）

| 框架 | 测试结果 | 覆盖率 | `redirectStdoutToResults` | 命令要点 |
|------|----------|--------|---------------------------|----------|
| jest | `results.json` | `coverage-summary.json` | `false` | `--json --outputFile={results_file}` + `--coverageDirectory={report_dir}` + `--coverageReporters=json-summary` |
| vitest | `results.json` | `coverage-summary.json` | `false` | `--reporter=json --outputFile={results_file}` + `--coverage.reportsDirectory={report_dir}` + `--coverage.reporter=json-summary`（CLI 以实现对齐，见待决） |
| vite-plus | 同 vitest | 同 vitest | `false` | `vp test` 等价旗标 |
| bun | `results.txt` | `lcov.info` | `true` | 临时 bunfig：`coverageDir`/`coverageReporter=["lcov"]`；`bun --config=<temp> test --coverage {files}` + `>` |
| go | `results.ndjson` | `func-summary.txt`（+ `coverage.out`） | `true`（测试段） | `-coverprofile={coverprofile_file}`；段级 `>` 到 results；再 `go tool cover -func=… > {coverage_file}` |
| rust | `results.txt` | `coverage-summary.json` | `true`（cargo test 段） | llvm-cov `--output-path={coverage_file}`；与现有 errorlevel 链式对齐 |
| pytest | `results.txt` | `coverage.json` | `true`（测试段） | `--cov-report=json:{coverage_file}`；测试段 `>` |
| node-test | `results.txt` | （可同文件文本表；`coverage_output` 可仍指向 `results.txt` 或空） | `true` | 仅 `>`；parser 从同一文件抽 tests + coverage |

### execute 管线（所有框架）

```
1. planId = derivePlanId(directory, framework)
2. reportDir = join(reportsDir, planId)；mkdir；清空该目录
3. prepared = preparePlanArtifacts(...)
4. expand {config_args} + substitute placeholders（含 {files}/{directory}/…）
5. 若 redirectStdoutToResults：对测试结果段追加 > "{results_file}"（cmd 用等价重定向）
6. runCommand(cwd=absCwd)
7. parsePlanArtifacts(framework, reportDir)
8. （可选）mutation：临时 stryker → reportDir/mutation.json → 解析 → cleanup
9. 返回 ExecutionResult（含 planId/reportDir/resultsFile）
10. generateSubReport → report.json；汇总时写 summary.json + plans[]
11. cleanup prepared.tempPaths（best-effort）
```

**detect**：模板含未展开占位符；**不** mkdir reportDir；**不**写临时 bunfig。

### 占位符路径约定

- `report_dir` / `results_file` / `coverage_file`：相对 **absCwd** 的路径字符串（由 `path.relative(absCwd, absPath)` 生成，POSIX `/`），以便在 suite cwd 下执行时正确落盘到 planDir。
- `plans[].path`：相对 **projectRoot**（有/无 change 前缀不同）。

---

## 依赖

### 运行时依赖

- 既有 Node 内置 `fs` / `path` / `child_process` — 目录清空、临时文件、命令执行
- 既有 `zod` / `zod/v4` — schema 与解析校验
- 各框架 CLI（jest / vitest / bun / cargo llvm-cov / go / pytest / node）— 原生文件输出或段级重定向（不新增 npm 依赖）

### 构建/测试依赖

- 无新增；沿用仓库现有 Vitest / TypeScript 工具链（本设计不写测试任务）

---

## 待决问题

- jest：CLI `--coverageDirectory` / `--outputFile` 是否稳定覆盖用户 config 内同名键；若否 → 该框架降级临时 config overlay（C3 例外，非默认）
- vitest / vite-plus：json reporter `outputFile` 与 coverage `reportsDirectory` 的确切 CLI 形态（版本差异）；若某版本无 CLI → 降级 `>` 或临时 config
- bun：`bun --config=<file> test …` 参数位置；加载失败时降级为测试结果 `>` + `coverage=null`，不阻断其它框架
- go：`func-summary.txt` → 现有 `ParsedCoverage` 字段映射（沿用 `go-cover` 解析；确认 statements% → lines 即可）
- Windows cmd：链式框架段级 `>` 与 `errorlevel` 是否与现有 rust 模式完全一致（实现时对齐）
- `coverage_cleanup` 字段：彻底从 `FrameworkConfig` 删除 vs 保留空数组兼容旧测试夹具 — 倾向删除并同步 detect/schema 消费方
- `generateSummaryReport` 第四参 `planResults`：若仅靠 `subReports` + `derivePlanId` 即可投影 `plans[]`，可省略该参（实现时二选一）
