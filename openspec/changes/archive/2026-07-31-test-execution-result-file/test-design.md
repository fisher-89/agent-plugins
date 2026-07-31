# 测试设计: test-execution-result-file

> **日期**: 2026-07-31

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 有/无 `--change` 时分别写入 `…/reports/test/summary.json` 与 `…/reports/test/<planId>/report.json`；不再写 `reports/test-execution.json` 或扁平 `<planId>.json` | 单元测试 + 集成测试 | `plugins/dev-team/bin/src/lib/test-report.test.ts`；`plugins/dev-team/bin/src/commands/test-execution.test.ts`；`plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` |
| AC-2 | `directory === '.'` → 目录名为 `<framework>`；非根 suite → `sanitize(directory)_<framework>`；均无 `.json` 后缀 | 单元测试 | `plugins/dev-team/bin/src/lib/test-report.test.ts` |
| AC-3 | summary 含 `plans[]`，字段 `id/framework/directory/path`；`path` 相对 project root；半失败 plan 仍出现且带 path | 单元测试 + 集成测试 | `plugins/dev-team/bin/src/lib/test-report.test.ts`；`plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts` |
| AC-4 | jest 使用 `--outputFile` 写入 planDir；parser 直接读文件成功；**不**对整段 stdout 做 `JSON.parse`；**不**追加壳层 `>` | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts`；`plugins/dev-team/bin/src/lib/test-runner.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/js-parser.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/index.test.ts` |
| AC-5 | 有原生文件输出的框架不追加 `>`；无原生能力的测试段才段级重定向；不做 tee | 单元测试 + 集成测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts`；`plugins/dev-team/bin/src/lib/test-framework.test.ts`；`plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-redirect.test.ts` |
| AC-6 | 只从 plan 目录读 coverage；无侧车则为 null；不读 suite cwd 旧路径；无 coverageMap fallback | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/index.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` |
| AC-7 | execute 前决议路径/占位符；detect script 仍含未展开的 report 占位符；临时文件 cwd + 用后删 | 单元测试 + 集成测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts`；`plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`；`plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-placeholders.test.ts` |
| AC-8 | 临时 bunfig 直写 `lcov.info` 到 planDir；`coverage_format` 为 lcov；不改用户 bunfig；不做 post-copy 默认策略 | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts`；`plugins/dev-team/bin/src/lib/test-runner.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` |
| AC-9 | Stryker 报告落在 `reportDir/mutation.json`；解析不读旧 `reports/mutation/` | 单元测试 + 集成测试 | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts`；`plugins/dev-team/bin/src/lib/test-runner.test.ts`；`plugins/dev-team/bin/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` |
| AC-10 | executor/evaluator 文案使用 `reports/test/summary.json` 与 `plans[]`→`report.json`；禁止 `<framework>.json` | 不可测试项 | agent / skill Markdown（见「不可测试项」） |
| AC-11 | 每个框架至少 1 条「约定产物在 reportDir」单测（可 mock fs） | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/index.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/js-parser.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` |
| AC-12 | 对段级重定向框架保留噪声/前缀场景验收；原生 outputFile 族以读 plan 文件成功为主 | 单元测试 + 集成测试 | `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts`；`plugins/dev-team/bin/src/lib/test-parser/js-parser.test.ts`；`plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-redirect.test.ts` |

---

## 单元测试

### plugins/dev-team/bin/src/lib/test-report.ts -> plugins/dev-team/bin/src/lib/test-report.test.ts

#### 待测功能

- `derivePlanId(directory, framework): string`: 由 directory + framework 生成无 `.json` 后缀的目录 id（`.` → `<framework>`）
- `generateSubReport(framework, result, projectRoot, reportsDir, planDirectory): TestExecutionSubReport`: 写入 `reportsDir/<planId>/report.json`
- `generateSummaryReport(subReports, projectRoot, reportsDir, planResults?): TestExecutionSummaryReport`: 写入 `reportsDir/summary.json` 并填充 `plans[]`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| derivePlanId | 正向 | `directory='.'` + `framework='vitest'` → `'vitest'`（无前导 `_`、无 `.json`） | 新增 |
| derivePlanId | 正向 | `directory='plugins/dev-team/bin'` + `framework='vite-plus'` → `'plugins_dev-team_bin_vite-plus'` | 新增 |
| derivePlanId | 异常 | `framework=''` 时空字符串仍参与拼接，结果可预测且不抛异常 | 新增 |
| derivePlanId | 边界 | `directory=''` 与 `directory='.'` 行为一致或文档化差异 | 新增 |
| derivePlanId | 边界 | Windows 反斜杠路径 `plugins\\dev-team\\bin` 消毒为下划线 | 新增 |
| derivePlanId | 边界 | 尾部斜杠 `plugins/dev-team/` 去尾后消毒 | 新增 |
| derivePlanId | 边界 | 超长 directory（>1000 chars）仍返回无 `.json` 的字符串 | 新增 |
| generateSubReport | 正向 | 写入 `reports/test/<planId>/report.json` 而非扁平 `<planId>.json` | 新增 |
| generateSubReport | 正向 | 根 suite（`.`）时 plan 目录名为框架名（如 `vitest/report.json`） | 新增 |
| generateSubReport | 异常 | `reportsDir` 指向只读或不可创建路径时抛出或透传 fs 错误 | 新增 |
| generateSubReport | 异常 | `result` 缺必填字段（如 `testCases` 为 undefined）时不写出非法 JSON，抛错或返回可诊断失败 | 新增 |
| generateSubReport | 边界 | 重复调用同一 planId 覆盖 `report.json`（幂等） | 新增 |
| generateSubReport | 边界 | `result.error` 半失败仍写出完整 `report.json` | 新增 |
| generateSummaryReport | 正向 | 落盘 `summary.json`（非 `test-execution.json`），含 `plans[]` | 新增 |
| generateSummaryReport | 正向 | `plans[]` 每项含 `id/framework/directory/path`，无 status 字段 | 新增 |
| generateSummaryReport | 正向 | `path` 为相对 project root 的 POSIX 路径（含 change 前缀场景） | 新增 |
| generateSummaryReport | 异常 | 空 `subReports` 仍写 summary，`plans=[]` | 新增 |
| generateSummaryReport | 边界 | 半失败 plan（exitCode≠0 / error 有值）仍进入 `plans[]` 且带 path | 新增 |
| generateSummaryReport | 边界 | 多 plan 时每个 id 唯一且 path 指向对应目录 | 新增 |
| generateSummaryReport | 边界 | 旧路径断言 `reports/test-execution.json` / 扁平 `<planId>.json` 用例标记废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件系统 | `fs.mkdtempSync` 创建 temp projectRoot / reportsDir，测后 `fs.rmSync` | 全部 generateSubReport / generateSummaryReport |
| openspec/config.json | 写入最小 config（可选 coverage thresholds） | coverage / conclusion 相关既有用例 |

---

### plugins/dev-team/bin/src/lib/test-runner.ts -> plugins/dev-team/bin/src/lib/test-runner.test.ts

#### 待测功能

- `preparePlanArtifacts(input: PreparePlanArtifactsInput): PreparePlanArtifactsResult`: 决议占位符、`configArgs`、`redirectStdoutToResults`、临时文件路径
- `executePlanEntry(entry, projectRoot, options: { reportsDir: string; ... }): ExecutionResult`: mkdir/清空 planDir → prepare → 展开 → 条件 `>` → 跑命令 → `parsePlanArtifacts` → 扩展字段

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| preparePlanArtifacts | 正向 | jest：`redirectStdoutToResults=false`；placeholders 含相对 absCwd 的 `results_file`/`coverage_file`/`report_dir` | 新增 |
| preparePlanArtifacts | 正向 | vitest/vite-plus：`redirectStdoutToResults=false`；含 outputFile / reportsDirectory 占位符 | 新增 |
| preparePlanArtifacts | 正向 | bun：`redirectStdoutToResults=true`；生成临时 bunfig；`tempPaths` 非空；`configArgs` 含 `--config` | 新增 |
| preparePlanArtifacts | 正向 | go/rust/pytest/node-test：`redirectStdoutToResults=true`（测试段） | 新增 |
| preparePlanArtifacts | 异常 | 未知 framework 抛错或返回可诊断错误 | 新增 |
| preparePlanArtifacts | 边界 | `userConfigPath=null` 时 bun 仍写临时 bunfig；不触碰用户路径 | 新增 |
| preparePlanArtifacts | 边界 | `reportDir` 为空字符串时路径决议失败或规范化 | 新增 |
| executePlanEntry | 正向 | 必填 `reportsDir`：先 mkdir 并清空该 plan 目录，不影响兄弟 plan 目录 | 新增 |
| executePlanEntry | 正向 | jest 命令含 `--outputFile=` 指向 planDir/`results.json`，命令字符串不含段级 `>` | 新增 |
| executePlanEntry | 正向 | 返回 `ExecutionResult` 含 `planId`/`reportDir`/`resultsFile` | 新增 |
| executePlanEntry | 正向 | 有侧车 coverage 文件时从 planDir 解析；无则 `coverage=null` | 新增 |
| executePlanEntry | 异常 | 命令失败（非 0）仍调用垂直 parse；半失败结果含 error 与 planId | 新增 |
| executePlanEntry | 异常 | 缺 `reportsDir` 时 TypeScript/运行期失败（测试侧显式传入） | 新增 |
| executePlanEntry | 边界 | planDir 已有旧文件时执行前清空该目录 | 新增 |
| executePlanEntry | 边界 | bun 执行后 best-effort 删除 `tempPaths`；用户 bunfig 未被修改 | 新增 |
| executePlanEntry | 边界 | mutation 启用时读取 `reportDir/mutation.json`，不读 `reports/mutation/` | 新增 |
| executePlanEntry | 边界 | 不做 tee：命令中不出现同时写 stdout 与文件的 tee 形态 | 新增 |
| executePlanEntry | 废弃 | 依赖整段 stdout `JSON.parse` / suite cwd `coverage/coverage-summary.json` 的旧断言 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `child_process.execSync` | `vi.mock` 返回固定 stdout/stderr/status；捕获实际命令字符串 | executePlanEntry 全部 |
| 文件系统 | temp dir 预置 `results.json` / `coverage-summary.json` / `lcov.info` / `results.txt` 等 | 文件通道 parse 与 coverage |
| `parsePlanArtifacts` / 垂直 parser | 可通过真实实现读 temp 文件；必要时 spy | 解析路径断言 |
| `resolveStrykerConfig` / `parseMutationReport` | mock 返回 temp mutation.json 路径与报告对象 | mutation 路径用例 |

---

### plugins/dev-team/bin/src/lib/test-framework.ts -> plugins/dev-team/bin/src/lib/test-framework.test.ts

#### 待测功能

- `getFrameworkConfig(framework): FrameworkConfig`: 八框架模板占位符、`coverage_output` 相对 reportDir、`coverage_format` 含 `lcov`、bun `config_flag`
- `detectFrameworkVersion(framework, cwd): string`: 版本探测（本变更非主路径，保留既有覆盖）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| getFrameworkConfig | 正向 | jest 模板含 `--json --outputFile={results_file}` 与 `--coverageDirectory={report_dir}`；`coverage_output='coverage-summary.json'` | 新增 |
| getFrameworkConfig | 正向 | vitest/vite-plus 模板含 `{results_file}` / `{report_dir}`（或等价 coverage 占位符） | 新增 |
| getFrameworkConfig | 正向 | bun：`coverage_format='lcov'`；`coverage_output='lcov.info'`；`config_flag='--config'`（非 null） | 新增 |
| getFrameworkConfig | 正向 | go：`coverage_output` 为相对 reportDir 的 `func-summary.txt`；模板含 `{coverprofile_file}` 或 `{coverage_file}` | 新增 |
| getFrameworkConfig | 正向 | rust：llvm-cov `--output-path={coverage_file}`；测试段可段级重定向 | 新增 |
| getFrameworkConfig | 正向 | pytest：`--cov-report=json:{coverage_file}`；`coverage_output='coverage.json'` | 新增 |
| getFrameworkConfig | 正向 | node-test：模板支持 `{results_file}`；`redirect` 语义由 runner 决策 | 新增 |
| getFrameworkConfig | 异常 | 未知框架名抛错且错误信息列出八框架 | 新增 |
| getFrameworkConfig | 边界 | `coverage_cleanup` 字段删除或为空且不再驱动 suite cwd 清理（旧「非空 cleanup」断言废弃） | 新增 |
| getFrameworkConfig | 边界 | 八框架 `shell.test_execution` / `cmd.test_execution` 均含报告相关占位符或由 runner 追加 `>` | 新增 |
| getFrameworkConfig | 边界 | bun 模板不含 post-copy / 不写死用户 bunfig 路径 | 新增 |
| getFrameworkConfig | 废弃 | bun `coverage_format='istanbul'` / `coverage/coverage-summary.json` / `config_flag=null` | 废弃 |
| getFrameworkConfig | 废弃 | 各框架 `coverage_output` 指向 suite cwd 旧路径（如 `coverage/coverage-summary.json`）的断言 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无外部 IO | 纯 registry 读取，无需 mock | getFrameworkConfig |
| `execCommand` | mock 版本命令输出（既有） | detectFrameworkVersion |

---

### plugins/dev-team/bin/src/lib/test-parser/index.ts -> plugins/dev-team/bin/src/lib/test-parser/index.test.ts

#### 待测功能

- `parsePlanArtifacts(framework, reportDir): ParsedPlanArtifacts`: 垂直分发，从 planDir 读约定产物并返回 testCases/coverage 等
- `parseTestOutput(stdout, stderr, framework): ParsedTestResult`: 文本段内部复用（execute 主路径不再依赖 stdout JSON）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parsePlanArtifacts | 正向 | jest：reportDir 含 `results.json` + `coverage-summary.json` → 解析成功且 coverage 非 null | 新增 |
| parsePlanArtifacts | 正向 | vitest/vite-plus：同 jest 产物约定 | 新增 |
| parsePlanArtifacts | 正向 | bun：读 `results.txt` + `lcov.info` | 新增 |
| parsePlanArtifacts | 正向 | go：读 `results.ndjson` + `func-summary.txt` | 新增 |
| parsePlanArtifacts | 正向 | rust：读 `results.txt` + `coverage-summary.json` | 新增 |
| parsePlanArtifacts | 正向 | pytest：读 `results.txt` + `coverage.json` | 新增 |
| parsePlanArtifacts | 正向 | node-test：读 `results.txt`（tests + coverage 同源） | 新增 |
| parsePlanArtifacts | 异常 | reportDir 不存在 → 返回 error，coverage=null | 新增 |
| parsePlanArtifacts | 异常 | 未知 framework → 可诊断错误或 text 回退（与实现一致） | 新增 |
| parsePlanArtifacts | 边界 | 仅有 results 无 coverage 侧车 → coverage=null | 新增 |
| parsePlanArtifacts | 边界 | 不读取 suite cwd 下旧 `coverage/` 路径 | 新增 |
| parseTestOutput | 正向 | 既有 dispatch（vitest/jest/go/text）保持可用供内部复用 | 新增 |
| parseTestOutput | 异常 | 未知 framework → 可诊断 error 或 text 回退，不抛未捕获异常 | 新增 |
| parseTestOutput | 异常 | jest/vitest 的 stdout 为非法 JSON → 返回带 error 的 ParsedTestResult，不抛异常 | 新增 |
| parseTestOutput | 边界 | 脏 stdout JSON 对 execute 主路径不再是唯一入口（主验收在 parsePlanArtifacts） | 新增 |
| parseTestOutput | 边界 | stdout/stderr 为空字符串时返回空结果或 error（与既有一致） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时 reportDir | `fs.mkdtempSync` 写入各框架约定文件名与最小合法内容 | parsePlanArtifacts 八框架 |
| 子 parser | 真实调用垂直实现（不 mock），保证文件通道端到端 | AC-11 |

---

### plugins/dev-team/bin/src/lib/test-parser/js-parser.ts -> plugins/dev-team/bin/src/lib/test-parser/js-parser.test.ts

#### 待测功能

- `parseJsOutput(stdout): ParsedTestResult`: 既有字符串解析（内部复用）
- （新增或经 index 调度）从 `results.json` 文件路径读取再解析：非整段 stdout `JSON.parse` 作为 execute 主路径

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parseJsOutput / 文件读 | 正向 | 合法 vitest/jest `results.json` 文件内容解析出 passed/failed | 新增 |
| parseJsOutput / 文件读 | 正向 | 有 coverage-summary 时由横向库解析（本文件专注 results） | 新增 |
| parseJsOutput | 异常 | 非法 JSON 文件返回 error，不抛未捕获异常 | 新增 |
| parseJsOutput | 异常 | 空文件 / 缺失文件 | 新增 |
| parseJsOutput | 边界 | 文件内容合法而「若被拼进脏 stdout」会失败的场景：证明文件通道不受前缀噪声影响 | 新增 |
| parseJsOutput | 边界 | 超大 JSON（>1000 cases）可解析 | 新增 |
| parseJsOutput | 废弃 | 「必须对整段 stdout 做 JSON.parse」作为唯一成功路径的假设 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件 | 写入 fixture JSON（含/不含噪声仅用于对比说明） | 文件读路径 |
| fs | 可选 spy `readFileSync` 断言读取的是 reportDir 路径 | 路径权威源 |

---

### plugins/dev-team/bin/src/lib/test-parser/go-parser.ts -> plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts

#### 待测功能

- `parseGoOutput(stdout): ParsedTestResult`: NDJSON 解析（可先读 `results.ndjson` 再调用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parseGoOutput | 正向 | 从 `results.ndjson` 文件读入后解析 pass/fail/skip | 新增 |
| parseGoOutput | 异常 | 空文件 → 空结果或 error（与既有一致） | 新增 |
| parseGoOutput | 边界 | 夹杂非 JSON 行不崩溃 | 新增 |
| parseGoOutput | 边界 | 超大 ndjson 列表 | 新增 |
| parseGoOutput | 正向 | 配合 coverage-parser 读 reportDir/`func-summary.txt`（可在 index 层断言） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件 | 写入 go test -json 样例行 | 文件通道场景 |

---

### plugins/dev-team/bin/src/lib/test-parser/text-parser.ts -> plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts

#### 待测功能

- `parseTextOutput(output): ParsedTestResult`: bun/rust/pytest/node-test 文本解析；文件通道下先读 `results.txt` 再调用

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parseTextOutput | 正向 | 从 `results.txt` 解析 bun `[PASS]/[FAIL]` | 新增 |
| parseTextOutput | 正向 | cargo / pytest / node-test 既有格式仍可用 | 新增 |
| parseTextOutput | 正向 | 前缀噪声 + 合法结果段：文件通道读纯净文件仍成功（AC-12） | 新增 |
| parseTextOutput | 异常 | 空字符串 / 仅空白 → error | 新增 |
| parseTextOutput | 边界 | 超长输出（>1000 行） | 新增 |
| parseTextOutput | 边界 | 特殊字符（emoji、`\n`）夹在用例名中不抛异常 | 新增 |
| parseTextOutput | 边界 | 未知格式不抛异常 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时 `results.txt` | 写入带/不带前缀噪声的 fixture | AC-12 噪声场景 |

---

### plugins/dev-team/bin/src/lib/test-parser/coverage-parser.ts -> plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts

#### 待测功能

- `parseCoverageFromFile(filePath, format): ParsedCoverage | null`: 支持 `'lcov'`；既有 istanbul/llvm-cov/go-cover/coverage-py/node-test

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parseCoverageFromFile | 正向 | `format='lcov'` 读取合法 `lcov.info` → 归一为 lines 等字段 | 新增 |
| parseCoverageFromFile | 正向 | istanbul `coverage-summary.json` 从 planDir 路径读取 | 新增 |
| parseCoverageFromFile | 异常 | 文件不存在 → null | 新增 |
| parseCoverageFromFile | 异常 | `format='lcov'` 但内容损坏 → null | 新增 |
| parseCoverageFromFile | 异常 | 未知 format → null | 新增 |
| parseCoverageFromFile | 边界 | 空 lcov 文件 → null | 新增 |
| parseCoverageFromFile | 边界 | lcov 全 0% / 全 100% | 新增 |
| parseCoverageFromFile | 边界 | 超大 lcov（多 SF 段） | 新增 |
| parseCoverageFromFile | 边界 | format 为 `undefined`/空字符串 → null | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件 | 写入最小 lcov / istanbul fixture | lcov 与既有格式 |

---

### plugins/dev-team/bin/src/lib/test-parser/mutation-parser.ts -> plugins/dev-team/bin/src/lib/test-parser/mutation-parser.test.ts

#### 待测功能

- `parseMutationReport(reportPath): MutationReport | null`: API 签名不变；调用方传入 planDir 下绝对路径

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parseMutationReport | 正向 | 读取 `reportDir/mutation.json` 合法 Stryker 报告成功 | 新增 |
| parseMutationReport | 异常 | 路径指向旧 `reports/mutation/` 且文件不存在 → null（runner 不应再传旧路径） | 新增 |
| parseMutationReport | 边界 | 空文件 / 非法 JSON → null | 新增 |
| parseMutationReport | 正向 | 既有分数/killed 字段解析保持 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时 mutation.json | 写入最小 Stryker JSON | 正向/边界 |

---

### plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts -> plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts

#### 待测功能

- `resolveStrykerConfig(rootPath, sourceFiles, framework, reportDir): { configPath; tempDirPath }`: `jsonReporter.fileName` 指向 planDir/`mutation.json`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| resolveStrykerConfig | 正向 | 传入 reportDir 后临时配置中 `jsonReporter.fileName` 指向该目录下 `mutation.json` | 新增 |
| resolveStrykerConfig | 正向 | 临时配置落在 absCwd，不改用户长期 config | 新增 |
| resolveStrykerConfig | 异常 | 不支持的 framework 抛错 | 新增 |
| resolveStrykerConfig | 边界 | `sourceFiles=[]` 仍生成配置 | 新增 |
| resolveStrykerConfig | 边界 | `reportDir` 为相对/绝对路径均可落盘到目标 planDir | 新增 |
| resolveStrykerConfig | 废弃 | 默认写入旧 `reports/mutation/` 的断言 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时 cwd + reportDir | `mkdtemp` 双目录，读回生成的 stryker JSON | 全部 |

---

### plugins/dev-team/bin/src/commands/test-detect-frameworks.ts -> plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts

#### 待测功能

- `runTestDetectFrameworks(options): TestDetectFrameworksResult`: script 保留 report 占位符；不注入 suite cwd `coverage_cleanup`；不 mkdir reportDir

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestDetectFrameworks | 正向 | 产出 script 含 `{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` 等未展开占位符 | 新增 |
| runTestDetectFrameworks | 正向 | `coverage_output` 为相对 reportDir 的垂直约定名 | 新增 |
| runTestDetectFrameworks | 正向 | bun plan 的 `coverage_format` 为 `lcov` | 新增 |
| runTestDetectFrameworks | 异常 | 无 tests 配置 → 空 plan | 新增 |
| runTestDetectFrameworks | 边界 | script 中不出现已展开的绝对 `reports/test/...` 路径 | 新增 |
| runTestDetectFrameworks | 边界 | 不注入 `rm -rf coverage` 等 suite cwd cleanup | 新增 |
| runTestDetectFrameworks | 废弃 | detect 烤死 reportDir / 注入 coverage_cleanup 的旧断言 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时项目 + config.json | 写入八框架或单框架 suite | 正向 |
| 版本探测命令 | mock `execCommand` / version | 稳定模板选择 |

---

### plugins/dev-team/bin/src/commands/test-execution.ts -> plugins/dev-team/bin/src/commands/test-execution.test.ts

#### 待测功能

- `runTestExecution(options): Promise<number>`: `resolveReportsDir` → `…/reports/test`；向 `executePlanEntry` 传 `reportsDir`；summary/原子报告新路径

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestExecution | 正向 | 无 `--change`：`reportsDir` 为 `{projectRoot}/reports/test` | 新增 |
| runTestExecution | 正向 | 有 `--change`：`openspec/changes/<change>/reports/test` | 新增 |
| runTestExecution | 正向 | 调用 `executePlanEntry` 时 options 含 `reportsDir` | 新增 |
| runTestExecution | 正向 | `generateSummaryReport` / `generateSubReport` 使用新 reportsDir | 新增 |
| runTestExecution | 异常 | detect 为空退出 0 并提示 | 新增 |
| runTestExecution | 异常 | 单 plan 失败不阻塞后续 | 新增 |
| runTestExecution | 边界 | 旧路径 `reports/test-execution` 断言废弃 | 废弃 |
| runTestExecution | 边界 | 扁平 `<framework>.json` 路径断言废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `runTestDetectFrameworks` | `vi.mock` 返回固定 plan | 全部 |
| `executePlanEntry` | mock 返回含 planId/reportDir 的 ExecutionResult | 全部 |
| `generateSubReport` / `generateSummaryReport` | mock 并断言传入的 reportsDir | 路径用例 |
| 临时 projectRoot | mkdtemp + config.json | 全部 |

---

## 集成测试

### CLI reportsDir → summary.json / plan report.json → `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-execute.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/test-execution.ts` | 触发方：决议 `reports/test` 并编排 |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 执行方：产出带 planId/reportDir 的 ExecutionResult |
| `plugins/dev-team/bin/src/lib/test-report.ts` | 写入方：落盘 `summary.json` 与 `<planId>/report.json` |

**关联AC**: AC-1, AC-2, AC-3

**关系描述**:

CLI 将报告根从 `reports/test-execution` 迁到 `reports/test`，runner 与 report 模块必须共用同一 `reportsDir` 与 `derivePlanId` 规则，才能让 summary 的 `plans[].path` 与真实目录一致。集成失败模式包括：CLI 仍传旧根、原子报告写成扁平 json、或半失败 plan 未进入 `plans[]`。

#### 场景: 无 change 时新布局落盘

验证无 `--change` 时聚合与原子报告路径。前置：临时项目含 vitest plan；mock execute 返回成功结果。输入：`runTestExecution({ projectRoot })`。预期：存在 `reports/test/summary.json` 与 `reports/test/<planId>/report.json`；不存在 `reports/test-execution.json` 与扁平 `<planId>.json`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 无 change：写入 `reports/test/summary.json` 与 `reports/test/vitest/report.json`（根 suite） | 新增 |
| 异常 | execute 半失败仍写 report.json，summary.plans 含该 plan 的 path | 新增 |
| 边界 | 多 directory plan：`plugins_dev-team_bin_vite-plus/report.json` 目录 id 无 `.json` | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `runTestDetectFrameworks` | 返回 1～2 个 plan | 全部 |
| `executePlanEntry` | 返回可控 ExecutionResult（含 planId/reportDir） | 全部 |
| 真实 fs + generate* | 使用真实 test-report 写盘，或 mock 后断言路径参数 | 正向/边界 |

#### 场景: 有 change 时 change 专属目录

验证 `--change` 前缀。前置：临时 openspec change 目录。输入：`runTestExecution({ change: 'foo' })`。预期：文件落在 `openspec/changes/foo/reports/test/...`，且 `plans[].path` 相对 project root 含该前缀。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | change 专属 `…/reports/test/summary.json` | 新增 |
| 边界 | `plans[].path` 以 `openspec/changes/<change>/reports/test/` 开头 | 新增 |
| 异常 | change 名含特殊字符时路径仍可创建或被拒绝（与现实现一致） | 新增 |

---

### preparePlanArtifacts → 条件重定向 → parsePlanArtifacts → `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-redirect.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/test-framework.ts` | 提供模板与垂直产物约定 |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 中间件：prepare、条件 `>`、调度 parse |
| `plugins/dev-team/bin/src/lib/test-parser/index.ts` | 读取方：从 reportDir 解析产物 |

**关联AC**: AC-4, AC-5, AC-6, AC-12

**关系描述**:

原生 outputFile 族（jest/vitest/vite-plus）必须在命令中带文件输出旗标且不追加壳层 `>`；段级重定向族则由 runner 追加 `>`，parser 只信 planDir 文件。集成价值在于同时断言「命令形态」与「解析权威源」。出错模式：仍 `JSON.parse(stdout)`、tee、或 coverage 回读 suite cwd。

#### 场景: jest 原生文件通道

验证 jest 不追加 `>` 且从 planDir 读 `results.json`。前置：temp reportDir 预置产物；mock execSync。输入：executePlanEntry(jest plan, { reportsDir })。预期：命令含 `--outputFile=`；不含 `> "`；解析成功；脏 stdout 不影响结果。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | jest 命令含 `--outputFile=` 指向 planDir/`results.json`，无壳层 `>` | 新增 |
| 正向 | exec 返回脏 stdout 时仍从文件解析成功 | 新增 |
| 异常 | results.json 缺失 → error，不回退 stdout JSON | 新增 |
| 边界 | coverage 仅从 planDir/`coverage-summary.json` 读取；cwd 旧 coverage 被忽略 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `child_process.execSync` | 返回带前缀噪声的 stdout；exit 0 | 正向/边界 |
| 文件系统 | 预写 planDir 约定产物；可选在 suite cwd 放诱饵 coverage | coverage 权威源 |

#### 场景: 段级重定向框架（bun/go/pytest）与噪声

验证无原生 outputFile 时追加 `>`，且噪声场景读文件成功。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | bun/go/pytest 测试段命令含 `> "{results_file}"`（或 cmd 等价） | 新增 |
| 正向 | 文件内为干净文本/ndjson 时解析成功（AC-12） | 新增 |
| 异常 | 结果文件空 → 可诊断失败 | 新增 |
| 边界 | 命令字符串不出现 tee | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `execSync` | 捕获命令；不真正跑框架 | 重定向形态 |
| results 文件 | 写入带/不带噪声的干净内容 | AC-12 |

---

### detect 占位符 → execute 展开 → reportDir 产物 → `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-placeholders.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 写入方：生成含占位符的 script |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 展开方：preparePlanArtifacts + 占位符替换 |
| `plugins/dev-team/bin/src/lib/test-framework.ts` | 模板源 |

**关联AC**: AC-7, AC-8

**关系描述**:

detect 不得烤死 report 路径；execute 才决议 reportDir 并展开。集成验证「detect 输出可被 execute 消费」的契约，避免占位符名不一致或 detect 侧误 mkdir/写临时 bunfig。bun 临时 bunfig 仅在 execute 出现且用后删除。

#### 场景: 占位符延迟展开

前置：真实或近真实 detect 产出 jest/bun plan。输入：将 detect script 交给 executePlanEntry。预期：detect 阶段 script 仍含 `{results_file}` 等；execute 后实际命令为具体相对路径；bun 临时文件被清理。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | detect 后 script 含未展开 `{results_file}`/`{report_dir}`/`{config_args}` | 新增 |
| 正向 | execute 后命令中占位符已被相对 absCwd 路径替换 | 新增 |
| 异常 | bun `--config` 加载失败降级：结果 `>` + coverage=null（若实现） | 新增 |
| 边界 | detect 不创建 reportDir、不写临时 bunfig | 新增 |
| 边界 | execute 后 bunfig tempPaths 被删除；用户目录无残留 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 版本探测 / exec | mock 版本与测试命令 | 全部 |
| 临时项目 | 真实 fs 观察 temp 文件生命周期 | bun 边界 |

---

### Stryker reportDir → mutation.json → 解析 → `plugins/dev-team/bin/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | 配置写入方：`jsonReporter.fileName` → planDir |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 触发方：跑 mutation 并传入 report 路径 |
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.ts` | 读取方：解析传入的绝对路径 |

**关联AC**: AC-9

**关系描述**:

mutation 报告权威源迁到 plan 目录后，配置生成、执行与解析必须指向同一 `mutation.json`。若 runner 仍读旧 `reports/mutation/` 或配置未改 fileName，集成会失败。

#### 场景: mutation 报告落在 planDir

前置：jest/vitest plan，mutation 启用；mock stryker 命令并预置 `reportDir/mutation.json`。预期：配置 fileName 与 parse 路径均为 planDir；不访问旧 mutation 目录。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 临时 stryker 配置 `jsonReporter.fileName` 指向 `reportDir/mutation.json` | 新增 |
| 正向 | runner 解析该文件得到 mutation 块 | 新增 |
| 异常 | mutation.json 缺失 → mutation=null，不读旧路径 | 新增 |
| 边界 | 临时配置与 `.stryker-tmp` 用后清理 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| stryker 子进程 | mock exec，不真正跑 Stryker | 全部 |
| mutation.json | 预写合法报告到 planDir | 正向 |

---

## 不可测试项

- AC-10（executor / evaluator / phase skill 文案路径迁移）— **原因**: 被测物为 Markdown agent/skill 文档，不在 vite-plus 测试配置的可执行模块范围内；需人工审阅或静态字符串检查，不纳入自动化单测/集成测。
- `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts`（`plans[]` schema）— **原因**: `test_resolve_paths` 返回 `Not in test config scope`；通过 `test-report` / CLI 集成对 summary 形状做间接验收。
- `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts`（`coverage_format` 含 `lcov`）— **原因**: 同上，不在测试配置 scope；由 `test-detect-frameworks` / `test-framework` 行为测试间接覆盖。
- jest/vitest CLI 是否稳定覆盖用户 config 内同名输出键 — **原因**: 依赖本机已安装的具体框架版本与用户 config，属 design 待决项；自动化以「命令含 CLI 旗标 + 文件通道 parse」为准，不绑定真实 jest 二进制。
- Windows cmd 与 Git Bash 在真实 shell 下的 errorlevel 行为 — **原因**: CI/开发机 shell 差异大；集成测以捕获生成的 `cmd`/`shell` 字符串形态为主，完整真实 shell 矩阵列为手动/抽样验证。
