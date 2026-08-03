# 测试设计: mutation-score-below-60

> **日期**: 2026-07-31
> **框架**: vite-plus（`vite-plus/test` / vitest API）
> **基线报告**: `openspec/changes/archive/2026-07-31-test-execution-result-file/reports/test/plugins_dev-team_vite-plus/mutation.json`

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------------|
| AC-1 | 对该文件重跑 Stryker 后 mutation score ≥ 60（`test-framework.ts`） | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.ts` |
| AC-2 | 对该文件重跑 Stryker 后 mutation score ≥ 60（`test-runner.ts`） | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.ts` |
| AC-3 | 对该文件重跑 Stryker 后 mutation score ≥ 60（`stryker-config.ts`） | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` |
| AC-4 | 对该文件重跑 Stryker 后 mutation score ≥ 60（`test-report.ts`） | 单元测试 | `plugins/dev-team/bin/src/lib/test-report.ts` |
| AC-5 | 对该文件重跑 Stryker 后 mutation score ≥ 60（`go-parser.ts`） | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts` |
| AC-6 | 对该文件重跑 Stryker 后 mutation score ≥ 60（`text-parser.ts`） | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts` |
| AC-7 | 对该文件重跑 Stryker 后 mutation score ≥ 60（`test-detect-frameworks.ts`） | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` |
| AC-8 | 目标包既有相关单测全部通过；新增用例 colocated 且不引入 test-only export | 单元测试 / 集成测试 | 七个 colocated `*.test.ts`；可选 `__tests__/` |
| AC-9 | 无未说明的生产逻辑变更；若有 bugfix，须在变更说明中单列文件与原因 | 不可自动化（过程门禁） | 生产七文件（默认不改） |

分数定义：`killed / (killed + survived + noCoverage)`。最终证据为更新后 Stryker JSON（或等价 mutation 产物）中对应 `files` 条目；本设计中的用例以消灭 Survived / 覆盖 NoCoverage 为主。

---

## 单元测试

### plugins/dev-team/bin/src/lib/test-framework.ts -> plugins/dev-team/bin/src/lib/test-framework.test.ts

#### 待测功能

- `getFrameworkConfig(framework: string): FrameworkConfig` — 从 `FRAMEWORK_REGISTRY` 浅拷贝返回配置；未知框架抛错并列出八框架名
- `detectFrameworkVersion(framework: string, cwd: string): string` — 执行 `version_command`（timeout 30_000），合并 stdout/stderr 抽取首个 `major.minor.patch`；失败或无 semver 返回 `''`
- （间接）`isVersionAtLeast` / `extractSemver` — 仅 jest 模板在版本 ≥ `29.5.0` 时注入 `--randomize`；经 `getFrameworkConfig('jest').shell|cmd.test_execution(version)` 观测

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| getFrameworkConfig -- 字面量杀伤 | 正向 | 八框架 `version_command` 精确等于约定命令（含 `npx jest --version` / `vp --version` / `go version` 等） | 新增 |
| getFrameworkConfig -- 字面量杀伤 | 正向 | 八框架 `coverage_format` / `coverage_output` / `default_glob` / `mutation_framework` / `config_flag` 精确断言（含 bun=`lcov`+`lcov.info`+`--config`；go=`go-cover`+`func-summary.txt`+`null`；rust/node-test/pytest 的 `config_flag===null`） | 新增 |
| getFrameworkConfig -- 字面量杀伤 | 正向 | jest/vitest/vite-plus 的 shell 与 cmd `mutation_execution` 精确为 `npx stryker run "{config}"`；bun/rust/go/pytest/node-test 无 `mutation_execution` | 新增 |
| getFrameworkConfig -- 模板字面量 | 正向 | jest shell/cmd 在 version=`29.5.0` 时含完整片段：`--randomize`、`--no-verbose`、`--json`、`--outputFile={results_file}`、`--silent`、`--coverage`、`--coverageDirectory={report_dir}`、`--coverageReporters=json-summary`、`{config_args}`、`{files}` | 新增 |
| getFrameworkConfig -- 模板字面量 | 正向 | vitest / vite-plus shell 与 cmd 精确含 `--sequence.shuffle`、`--reporter=json`、`--outputFile={results_file}`、`--coverage.reportsDirectory={report_dir}`、`--coverage.reporter=json-summary`（vite-plus 前缀为 `vp test`，vitest 为 `npx vitest run`） | 新增 |
| getFrameworkConfig -- 模板字面量 | 正向 | bun shell/cmd 精确为 `bun {config_args} test --coverage {files}`（无 bunfig 硬编码路径） | 新增 |
| getFrameworkConfig -- 模板字面量 | 正向 | go shell 含 `go test -json -coverprofile={coverprofile_file} -covermode=atomic {directory}` 与 `go tool cover -func={coverprofile_file} > {coverage_file}`；cmd 含对应 `&` / `exit /b` 链 | 新增 |
| getFrameworkConfig -- 模板字面量 | 正向 | rust shell 含 `cargo test;` 与 `cargo llvm-cov --json --output-path {coverage_file}`；pytest shell 用 `;`、cmd 用 `&&` 连接两段 pytest | 新增 |
| getFrameworkConfig -- 模板字面量 | 正向 | node-test shell/cmd 精确含 `node --test --experimental-test-coverage {files}` | 新增 |
| getFrameworkConfig -- jest 版本门控 | 边界 | version=`29.5.0` → 含 `--randomize`；`29.4.9` / `29.4.0` / `''` / `'not-a-version'` / `'29'` → 不含 | 新增 |
| getFrameworkConfig -- jest 版本门控 | 边界 | version=`30.0.0` / `29.5.1` → 含 `--randomize`；major 更大时仍注入 | 新增 |
| getFrameworkConfig -- 未知框架 | 异常 | `getFrameworkConfig('unknown')` / `''` / `'jest '` 抛错，message 含 `Unknown framework` 且完整列出八键名 | 新增 |
| getFrameworkConfig -- 浅拷贝 | 边界 | 修改返回对象的 `coverage_output` 不影响再次 `getFrameworkConfig` 的值 | 新增 |
| detectFrameworkVersion | 正向 | mock `execCommand` status=0、stdout=`1.2.3` → 返回 `1.2.3`；调用参数含 `version_command`、`cwd`、`timeout: 30000` | 新增 |
| detectFrameworkVersion | 正向 | semver 在 stderr（stdout 空）→ 仍抽取；stdout/stderr 拼接后取首个 `x.y.z` | 新增 |
| detectFrameworkVersion | 异常 | status≠0 → `''`；status=0 但无 semver → `''` | 新增 |
| detectFrameworkVersion | 边界 | stdout=`v29.5.0-beta` 类文本含 `29.5.0` → 返回 `29.5.0`；空 stdout+stderr → `''`；超长噪声字符串中夹带 `0.0.1` → 返回该 semver | 新增 |
| detectFrameworkVersion | 异常 | 未知 framework → 抛错（与 getFrameworkConfig 一致），不调用 exec | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `./exec-command` 的 `execCommand` | `vi.mock` 返回可控 `{ status, stdout, stderr }`；断言 timeout=30000 | detectFrameworkVersion 全部分支 |
| 无文件系统 / 无子进程 | getFrameworkConfig 纯内存断言 | 字面量与版本门控 |

---

### plugins/dev-team/bin/src/lib/test-runner.ts -> plugins/dev-team/bin/src/lib/test-runner.test.ts

#### 待测功能

- `executePlanEntry(entry: TestPlan, projectRoot: string, options: { files?; timeout?; noMutation?; mutationDiffFiles?; reportsDir }): ExecutionResult` — 清空 planDir、prepare 占位符/临时 bunfig、展开模板、条件 stdout 重定向、`execSync` 执行、垂直解析、可选 Stryker mutation；prepare 失败 → `emptyResult`（exitCode=-1）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| executePlanEntry -- 占位符展开 | 正向 | 捕获 cmd：`{results_file}`/`{report_dir}`/`{coverage_file}`/`{coverprofile_file}` 均已替换为相对 absCwd 的 POSIX 路径且指向 `reportsDir/<planId>/…` | 新增 |
| executePlanEntry -- 占位符展开 | 正向 | `options.files=['a.test.ts','b.test.ts']` → cmd 中 `{files}` 为 `a.test.ts b.test.ts`；`files` 省略且 scope=`src` → `{files}` 回落为 `src`；scope=`'.'` → `{files}` 为空串 | 新增 |
| executePlanEntry -- 占位符展开 | 正向 | go plan：`{directory}` 在 scope=`'.'` 时为 `./...`，scope=`pkg` 时为 `./pkg/...` | 新增 |
| executePlanEntry -- 占位符展开 | 正向 | suite 声明 config 且 framework 有 `config_flag` → cmd 含 `--config <rel>`；无 config → `{config_args}` 被剥离且不残留多余空格占位 | 新增 |
| executePlanEntry -- 重定向 | 正向 | jest/vitest/vite-plus：**不含** `> "`；bun/node-test：整段末尾 `> "…results.txt"` | 新增 |
| executePlanEntry -- 重定向 | 正向 | go：重定向插在 `go test …` 与链分隔符之间（shell 为 `;`，win cmd 为 `&`），coverage 段不被重定向吞掉 | 新增 |
| executePlanEntry -- 重定向 | 正向 | rust：`cargo test > "…"` 后仍保留 llvm-cov 链；pytest：仅第一段 pytest 后插入 `>`，第二段 cov 保留 | 新增 |
| executePlanEntry -- 空命令 | 异常 | `script.shell`/`cmd` 为空或仅空白 → `exitCode=-1`、`error` 含 `Empty test command`、不调用 `execSync` | 新增 |
| executePlanEntry -- prepare 失败 | 异常 | 未知 framework → `error` 匹配 `/Unknown framework/`、`mutation===null`、`testCases=[]` | 新增 |
| executePlanEntry -- 解析失败 | 异常 | exitCode≠0 且 planDir 无结果文件 → `error` 含 `Missing or unparseable results file`；exitCode=0 但空结果按 parser 错误透传 | 新增 |
| executePlanEntry -- mutation 开关 | 正向 | `mutation_framework` 有值、测试全通过、mock stryker 写 `reportDir/mutation.json` → `mutation.score/threshold/pass` 正确；临时 `stryker.config.*.json` 执行后删除 | 新增 |
| executePlanEntry -- mutation 开关 | 边界 | `noMutation: true` → 不跑 stryker、`mutation===null`；`mutation_framework` 为空/null → 跳过；有 failed 用例 → 跳过 mutation | 新增 |
| executePlanEntry -- mutation 开关 | 边界 | `mutationDiffFiles` 限定后无交集 → 跳过 stryker；exclude 配置过滤全部 sourceFiles → 跳过 | 新增 |
| executePlanEntry -- mutation 失败 | 异常 | stryker exit≠0 或缺少 mutation.json → `mutation===null` 且不抛未捕获异常；不读 `reports/mutation/` 旧路径 | 新增 |
| executePlanEntry -- planId | 边界 | `directory='.'` → planId=`{framework}`；`directory='plugins/dev-team/bin'` → `plugins_dev-team_bin_{framework}`；反斜杠目录同样归一 | 新增 |
| executePlanEntry -- timeout | 正向 | 传入 `timeout: 1234` 时 `execSync` options.timeout 为 1234 | 新增 |
| executePlanEntry -- timeout | 边界 | 省略 `timeout`（Optional None）→ `execSync` options.timeout 为默认 60000 | 新增 |
| executePlanEntry -- timeout | 边界 | 传入 `timeout: 0` → `execSync` options.timeout 为 0（不回落 60000） | 新增 |
| executePlanEntry -- timeout | 边界 | 传入 `timeout: -1` → `execSync` options.timeout 为 -1（不回落 60000） | 新增 |
| executePlanEntry -- timeout | 边界 | 传入 `timeout: Number.MAX_SAFE_INTEGER`（MAX_INT）→ `execSync` options.timeout 为该值 | 新增 |
| executePlanEntry -- bunfig | 正向 | 存在用户 `bunfig.toml` 时临时 overlay 前缀包含原内容且含 `coverageDir`/`coverageReporter = ["lcov"]`；用户文件字节不变；结束后 temp 删除 | 新增 |
| executePlanEntry -- 执行错误缓冲 | 边界 | exec 抛错且 stdout/stderr 为 Buffer → 仍能解析 planDir 文件并设置 exitCode/status | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `child_process.execSync` | `vi.mock` 捕获 cmd/cwd/timeout；按框架写入 planDir 结果/coverage/mutation 文件后返回或抛带 `status` 的 Error | executePlanEntry 全部分支 |
| `fs` 临时目录 | `os.tmpdir` 下构造 `openspec/config.json` + suite 源文件树 | config / exclude / mutationDiff |
| `console.log` | spy 吞日志 | 全部分支 |
| `process.env.SHELL` / platform | 需要时固定 SHELL 以稳定 shell vs cmd 选择（仅断言可观测 cmd 形态） | 重定向分隔符场景 |

---

### plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts -> plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts

#### 待测功能

- `resolveStrykerConfig(rootPath, sourceFiles, framework, reportDir): { configPath; tempDirPath }` — 始终写临时 `stryker.config.<rand>.json`；`mutate` 路径 POSIX 相对化；`testRunner`/`plugins`/`jsonReporter.fileName`/`ignoreStatic`/`timeoutMS` 按约定填充；不支持的 framework 抛错

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| resolveStrykerConfig -- runner/plugins | 正向 | jest → testRunner=`jest`、plugins=`['@stryker-mutator/jest-runner']`；vitest/vite-plus → testRunner=`vitest`、plugins=`['@stryker-mutator/vitest-runner']` | 新增 |
| resolveStrykerConfig -- 配置字段 | 正向 | 写入 JSON 含 `$schema` 后缀 `stryker-schema.json`、`ignoreStatic===true`、`reporters` 含 `json` 与 `html`、`timeoutMS===10000`、`tempDirPath` 为 `root/.stryker-tmp` | 新增 |
| resolveStrykerConfig -- 路径归一 | 正向 | 相对路径 `src/foo.ts` → mutate 为 `src/foo.ts`（正斜杠） | 新增 |
| resolveStrykerConfig -- 路径归一 | 正向 | 绝对路径位于 root 下 → 相对 POSIX；Windows 风格反斜杠输入 → 输出仅 `/` | 新增 |
| resolveStrykerConfig -- 路径归一 | 边界 | 绝对路径等于 root → mutate 条目为 `'.'` | 新增 |
| resolveStrykerConfig -- 路径归一 | 边界 | 盘符绝对路径（`C:\…` 形态）落在 root 下 → 相对化；落在 root 外 → `path.relative` 结果且为 POSIX | 新增 |
| resolveStrykerConfig -- 路径归一 | 边界 | 相对路径误带 root 绝对前缀字符串 → 剥离前缀；`sourceFiles=[]` → `mutate=[]` | 新增 |
| resolveStrykerConfig -- reportDir | 正向 | `jsonReporter.fileName` 解析后绝对路径等于 `path.join(reportDir,'mutation.json')`；不含 `reports/mutation/` | 新增 |
| resolveStrykerConfig -- 不改用户配置 | 正向 | 预置用户 `stryker.config.json` 内容在调用前后一致；返回 `configPath` ≠ 用户文件 | 新增 |
| resolveStrykerConfig -- 不支持框架 | 异常 | framework=`bun`/`pytest`/`go`/`''` 抛错，message 匹配 `/Unsupported/` 且列出 `jest, vitest, vite-plus` | 新增 |
| resolveStrykerConfig -- 多文件 | 边界 | 多 sourceFiles（含单元素、超大列表>100）顺序保留且全部归一；非法空字符串路径仍写出配置不抛 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 真实临时目录 `fs` | `mkdtempSync` + `afterEach` 清理；读回 JSON 断言 | 全部路径与字段 |
| 无 mock crypto | 依赖真实 `randomBytes`；仅断言文件名前缀 `stryker.config.` 与存在性 | 临时文件生成 |

---

### plugins/dev-team/bin/src/lib/test-report.ts -> plugins/dev-team/bin/src/lib/test-report.test.ts

#### 待测功能

- `generateSubReport(framework, result, projectRoot, reportsDir, planDirectory): TestExecutionSubReport` — 汇总计数、`error_cases`、coverage/mutation、findings；写入 `<planId>/report.json`
- `generateSummaryReport(subReports, projectRoot, reportsDir): TestExecutionSummaryReport` — 聚合 totals/problems/coverage/mutation/conclusion/`plans[]`；写入 `summary.json`
- （间接）`derivePlanId`、`determineConclusion`、coverage/mutation overrides 与加权聚合 — 仅通过上述公共 API 的可观测 JSON 断言

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| generateSubReport -- 计数 | 正向 | 混合 passed/failed/skipped → summary 四计数字段精确；`error_cases` 仅含失败/错误用例 | 新增 |
| generateSubReport -- findings | 正向 | `result.error` 有值 → `findings===[error]`；无 error → findings 缺省/undefined | 新增 |
| generateSubReport -- mutation 透传 | 正向 | `result.mutation` 对象原样写入 sub-report；`null`/缺省 → `mutation===null` | 新增 |
| generateSubReport -- coverage | 正向 | 有 coverage → block 含 measured/thresholds/pass；无 coverage → null | 新增 |
| generateSubReport -- 执行失败透传 | 异常 | `exitCode≠0` 且 `result.error` 非空 → `exit_code`/findings 反映失败，仍写出 `report.json` 且不抛未捕获异常 | 新增 |
| generateSubReport -- error_cases | 异常 | 含 `failed`/`error` status 的用例 → `error_cases` 保留 name/errorMessage/errorType/stackTrace；`passed` 不入列 | 新增 |
| generateSubReport -- coverage 未达标 | 异常 | coverage measured 低于 suite thresholds → `coverage.pass===false`，sub-report 仍落盘 | 新增 |
| generateSubReport -- 写盘失败 | 异常 | `reportsDir` 不可写（父路径为已存在文件等）→ `writeJsonFile` 抛出文件系统错误并向上传播 | 新增 |
| generateSubReport -- derivePlanId | 边界 | planDirectory=`'.'` → 写入 `reportsDir/{framework}/report.json`；含 `/` 与 `\` 的目录 → 分隔符变 `_` 且无尾缀多余 `_` | 新增 |
| generateSubReport -- source_files | 边界 | sourceFiles 有路径但无 fileCoverage → 条目仅 `file`；有匹配 fileCoverage → 挂上 coverage 计数；无关 coverage 文件被忽略 | 新增 |
| generateSubReport -- 空用例 | 边界 | `testCases=[]`、`exitCode=-1` → summary 全 0 且 findings 可含 error | 新增 |
| generateSummaryReport -- conclusion | 正向 | 全通过、coverage/mutation 均 pass → `conclusion==='pass'` | 新增 |
| generateSummaryReport -- conclusion | 异常 | 存在 `execution_error` problem → `conclusion==='error'`（优先于 fail） | 新增 |
| generateSummaryReport -- conclusion | 异常 | failed>0 或 coverage.pass=false 或 mutation.pass=false → `conclusion==='fail'` | 新增 |
| generateSummaryReport -- 聚合 | 正向 | 多 subReports 的 total/passed/failed/skipped/duration 求和；`duration_seconds` 为毫秒和四舍五入到秒 | 新增 |
| generateSummaryReport -- 空输入 | 边界 | `subReports=[]` → totals 全 0、plans=[]、conclusion 为 pass 或按实现约定且不抛 | 新增 |
| generateSummaryReport -- suite 匹配 | 正向 | config 中多 suite 时按 framework+directory 命中阈值；仅 framework 命中次之；否则 suites[0] | 新增 |
| generateSummaryReport -- coverage overrides | 正向 | suite.coverage.overrides 命中源文件后 overrides 数组非空且影响 pass 计数 | 新增 |
| generateSummaryReport -- mutation 聚合 | 正向 | 多 plan mutation measured 累加后 score 公式为 `(killed+timeout)/(total-ignored-compileError-runtimeError)*100`；threshold 取 suite/默认 | 新增 |
| generateSummaryReport -- mutation 聚合 | 边界 | 分母为 0 → score 安全回落（0 或 null 按实现）；ignored/compileError 从分母剔除 | 新增 |
| generateSummaryReport -- mutation overrides | 正向 | suite.mutation.overrides 命中文件后覆盖块出现在 summary.mutation.overrides | 新增 |
| generateSummaryReport -- plans 索引 | 正向 | `plans[].id/framework/path` 指向各 planDir 相对 projectRoot 的 POSIX 路径 | 新增 |
| generateSummaryReport -- problems | 边界 | failed cases 超过 10 时 problems 截断为前 10 条 test_failure；coverage/mutation 失败各推一条 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时 projectRoot + `openspec/config.json` | 写入 tests[] 阈值/overrides | suite 匹配与 overrides |
| 内存构造 `ExecutionResult` / `TestExecutionSubReport` | 不跑真实测试命令 | sub/summary 全部分支 |
| 真实 `fs` 读写 report 路径 | 断言落盘 JSON 与返回值一致（幂等二次写入） | 文件通道 |

---

### plugins/dev-team/bin/src/lib/test-parser/go-parser.ts -> plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts

#### 待测功能

- `parseGoOutput(content: string): ParsedTestResult` — 解析 `go test -json` NDJSON；仅终端 Action `pass`/`fail`/`skip` 计用例；fail 后忽略后续 pass；`output` 仅追加到已失败用例；推导 `*_test.go` / `.go`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parseGoOutput -- 终端事件 | 正向 | pass/fail/skip 计数与 status 映射为 passed/failed/skipped；Elapsed=0.05 → durationMs=50 | 新增 |
| parseGoOutput -- fail 优先 | 正向 | 先 fail 后 pass 同名 → 保持 failed；先 pass 后 fail → 最终 failed | 新增 |
| parseGoOutput -- output 拼接 | 正向 | fail 后多条 Action=output 且带 Output → errorMessage 按序拼接；pass 用例的 output 不写入 errorMessage | 新增 |
| parseGoOutput -- 忽略非终端 | 正向 | Action=run/output（未见终端前）/未知 Action 不计 total；无 Test 字段的 package 事件忽略 | 新增 |
| parseGoOutput -- 空输入 | 边界 | `''` / 仅空白 / `'\n\n'` → total=0 且 `error==='Empty results content'` | 新增 |
| parseGoOutput -- 畸形行 | 异常 | 非 JSON 行跳过不抛；JSON 缺必填形状（schema 失败）跳过；夹杂合法事件仍计数 | 新增 |
| parseGoOutput -- Elapsed | 边界 | Elapsed 缺省 → durationMs undefined；Elapsed=0 → 0；Elapsed=1.234 → 1234 | 新增 |
| parseGoOutput -- 源文件推导 | 正向 | `TestFooBar` → testFiles 含 `foo_bar_test.go`，sourceFiles 含 `foo_bar.go`（排序） | 新增 |
| parseGoOutput -- 源文件推导 | 边界 | 名不匹配 `^Test([A-Z].*)`（如 `Test`/`testFoo`/`Test_foo`）→ 不推文件；重复用例名去重后仍单条 | 新增 |
| parseGoOutput -- 源文件推导 | 边界 | 人为构造含 `__tests__/` 的 testFile 路径时 sourceFiles 跳过（经可观测推导链覆盖） | 新增 |
| parseGoOutput -- 大规模 | 边界 | 500+ 行 NDJSON 仍正确计数 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无外部依赖 | 直接传入 NDJSON 字符串 | 全部用例 |
| 可选临时 results.ndjson | 仅在既有「文件通道」用例保留；新补强优先纯函数输入 | 回归兼容 |

---

### plugins/dev-team/bin/src/lib/test-parser/text-parser.ts -> plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts

#### 待测功能

- `parseTextOutput(output: string): ParsedTestResult` — Layer1 bun→cargo→node→pytest 短路；Layer2 通用正则；Layer3 行启发式；全失败 → `Unable to parse test output`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| parseTextOutput -- bun | 正向 | 行首 `N [PASS|FAIL|SKIP] name`（多空格）→ 正确 status；混合三类计数 | 新增 |
| parseTextOutput -- bun 近失配 | 边界 | 缺行首锚定数字、`[PASS]` 两侧单空格不足、`PASS` 无括号 → **不**走 bun，落入后续层或 error | 新增 |
| parseTextOutput -- cargo | 正向 | `test result: ok. N passed; M failed; S ignored…` 与 `FAILED.` 变体；前置 `test name ... ok/FAILED` 进入 testCases | 新增 |
| parseTextOutput -- cargo 近失配 | 边界 | 缺 `ignored` 段、分隔符为逗号、无 `test result:` 前缀 → 不命中 cargo | 新增 |
| parseTextOutput -- node | 正向 | `# pass N` / `# fail N` / `# skip N` 与 `ℹ pass`/`passed`/`failed`/`skipped` 变体累加；`ok 1 name` / `not ok 2 name` 进 testCases | 新增 |
| parseTextOutput -- node 近失配 | 边界 | 无 `#`/`ℹ` 摘要行仅有 ok 行 → 不命中 node（hasNodeMarker=false） | 新增 |
| parseTextOutput -- pytest | 正向 | `path::test PASSED|FAILED|SKIPPED` → status 映射；无 `::` 行不计入 pytest | 新增 |
| parseTextOutput -- pytest 近失配 | 边界 | `PASSED` 但无 `::`、状态小写 `passed` → 不命中 pytest layer1 | 新增 |
| parseTextOutput -- layer 短路 | 正向 | 同时含 bun 标记与 cargo 摘要 → 仅按 bun 结果（layer1 顺序） | 新增 |
| parseTextOutput -- generic | 正向 | `Tests: N passed, M failed, S total` / `Tests: N passed, M failed` / `N passed, M failed` / `N tests passed` 分别命中且 skipped 默认 0 | 新增 |
| parseTextOutput -- generic 近失配 | 边界 | 缺逗号/空格、`pass` 非 `passed`、总数位缺失导致误解析时用精确期望锁正则 | 新增 |
| parseTextOutput -- heuristic | 正向 | 行含 `PASSED`/`FAILED`/`SKIP`/`ok`/`not ok` 计数；含 `PASS` 且同时含 `FAIL` 的噪声行按实现跳过 | 新增 |
| parseTextOutput -- 空与不可解析 | 异常 | `''`/空白 → `error==='Empty output'`；无任何标记文本 → `error==='Unable to parse test output'` | 新增 |
| parseTextOutput -- 边界字符串 | 边界 | 超长单行（>1000）含合法 bun 标记仍解析；含 emoji/特殊字符的用例名 trim 后保留；`null` 不在类型内但空串覆盖 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无外部依赖 | 构造多行文本夹具（正向 + 逐字符近失配） | Regex 杀伤核心 |
| 不 mock 文件系统 | 纯 `parseTextOutput` 调用 | 全部新增用例 |

---

### plugins/dev-team/bin/src/commands/test-detect-frameworks.ts -> plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts

#### 待测功能

- `runTestDetectFrameworks(options: { files?; projectRoot? }): TestDetectFrameworksResult` — 读 `config.tests[]`，匹配文件到 suite，生成仍含占位符的 `TestPlan`（shell/cmd）；`files` 省略自动扫描；`files: []` 空结果

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestDetectFrameworks -- files | 正向 | 显式 files 命中 suite → detected.framework 正确且 plan 含对应 framework | 新增 |
| runTestDetectFrameworks -- files | 边界 | `files: []` → `{ detected: [], plan: [] }`（即使有 suites） | 新增 |
| runTestDetectFrameworks -- files | 边界 | 省略 files → auto-scan 收集项目文件；命中 includes 的文件出现在 detected | 新增 |
| runTestDetectFrameworks -- 排除目录 | 边界 | auto-scan 不进入 `node_modules`/`.git`/`dist`/`build`/`target`/`.vp`/`coverage`/`.nyc_output`/`.claude`；其下文件不出现在 detected | 新增 |
| runTestDetectFrameworks -- suite 优先级 | 正向 | 两 suite 均可匹配时数组顺序前者胜出 | 新增 |
| runTestDetectFrameworks -- exclude | 正向 | 全局 exclude 与 suite.excludes 命中的文件不出现在 detected | 新增 |
| runTestDetectFrameworks -- unknown | 正向 | 显式 files 未匹配任一 suite → framework=`unknown`；auto-scan 未匹配则省略该文件 | 新增 |
| runTestDetectFrameworks -- 无 suites | 边界 | 无 tests / tests=[]：显式 files → 全 unknown 且 plan=[]；auto-scan → detected=[] 且 plan=[] | 新增 |
| runTestDetectFrameworks -- config_flag | 异常 | suite 声明 config 但 framework 的 `config_flag===null`（如 go）→ 抛错，message 含 `does not support config injection` | 新增 |
| runTestDetectFrameworks -- config 路径 | 异常 | 声明 config 但无法解析 absConfig 的场景（构造非法 root/config）→ 抛错含 `absConfig could not be resolved`（若可稳定构造） | 新增 |
| runTestDetectFrameworks -- 占位符 | 正向 | script.shell/cmd 仍含 `{results_file}`/`{report_dir}`/`{config_args}` 等，且无 `cd` 前缀、无已展开 reports 绝对路径 | 新增 |
| runTestDetectFrameworks -- plan 字段 | 正向 | coverage_format/output、mutation_framework 来自 registry；`mutation_score` 来自 suite.mutation.score，缺省为 null | 新增 |
| runTestDetectFrameworks -- 去重 | 边界 | 两 suite 解析到相同 directory+framework → plan 仅一条 | 新增 |
| runTestDetectFrameworks -- includes 默认 | 边界 | suite 无 includes → 使用 framework `default_glob` 匹配；自定义 includes 覆盖默认 | 新增 |
| runTestDetectFrameworks -- projectRoot | 边界 | 省略 projectRoot 时走 `getProjectDir`（mock 返回临时根）；绝对/相对 files 均可 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `detectFrameworkVersion` | 沿用 `__tests__/test-setup.ts` 全局 spy 默认 `'99.0.0'`；个别用例改返回值 | plan 脚本生成 |
| 临时目录 + `openspec/config.json` | 写入 tests/excludes/includes/mutation | 范围与校验 |
| `getProjectDir`（如需） | `vi.spyOn` 返回临时 root | 省略 projectRoot |
| 真实 fs 树 | 放置命中/未命中文件与排除目录 | auto-scan / exclude |

---

## 集成测试

### detect 占位符 → executePlanEntry 展开 → `plugins/dev-team/bin/__tests__/cli-test-execution-execute/cli-test-execution-placeholders.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 产出仍含占位符的 `TestPlan.script` |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 展开占位符并在 absCwd 执行，落盘 planDir |

**关联AC**: AC-2, AC-7, AC-8

**关系描述**:

detect 与 execute 通过 `TestPlan` 契约衔接：detect 故意不展开 `{results_file}` 等占位符，execute 在清空 planDir 后替换为相对 suite cwd 的路径。该契约若被破坏，单测各自绿仍可能在真实编排下写错目录。补强集成夹具可锁定「detect 输出可被 execute 消费且产物落在 `reports/test/<planId>/`」，并防止回归引入 `cd` 前缀或双重展开。

#### 场景: 占位符延迟展开端到端

验证从 `runTestDetectFrameworks` 得到的 plan 交给 `executePlanEntry`（mock `execSync`）后，实际命令不再含未替换占位符，且结果文件落在 `options.reportsDir/<planId>/`。前置：临时项目含 vite-plus/jest suite。输入：detect 无 files 或显式测试文件。预期：cmd 含具体 results 路径；planDir 出现垂直结果文件。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | detect plan → execute 后 cmd 不含 `{results_file}`/`{report_dir}`，planDir 含 results 文件 | 新增 |
| 异常 | detect 因 config_flag 校验失败抛错时不进入 execute | 新增 |
| 边界 | bun plan：execute 生成临时 bunfig 且 detect 阶段项目根无 bunfig.dev-team-* | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `execSync` | mock 写入垂直结果后返回 | 正向/边界 |
| `detectFrameworkVersion` | 固定版本字符串 | 全部 |

---

### executePlanEntry → resolveStrykerConfig → mutation.json → `plugins/dev-team/bin/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 触发 mutation 阶段并解析 `reportDir/mutation.json` |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | 生成临时 Stryker 配置（mutate / jsonReporter） |

**关联AC**: AC-2, AC-3, AC-8

**关系描述**:

mutation 路径上 runner 调用 `resolveStrykerConfig` 后执行 stryker，再从 **planDir** 读 `mutation.json`。路径归一或 `jsonReporter.fileName` 偏差会导致「命令成功但 mutation 块为 null」。在既有 mutation-execution-flow 夹具上补强，可覆盖单测分测时不易串联的临时配置清理与报告落点。

#### 场景: 临时配置与 planDir mutation 落点

验证启用 `mutation_framework` 且测试通过时，stryker 命令引用的临时配置中 `mutate` 为相对路径、`fileName` 指向 planDir；执行后临时配置删除；`ExecutionResult.mutation` 非 null。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | mock stryker 读到临时 config 并写 planDir/mutation.json → result.mutation.score 可读 | 新增 |
| 异常 | stryker 非 0 退出 → mutation null 且临时 config 仍被清理 | 新增 |
| 边界 | `noMutation: true` 或 sourceFiles 被 exclude 清空 → 不生成临时 config | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `execSync` | 识别 stryker cmd，解析 configPath，写 mutation.json | 正向/异常 |
| 项目 config.json | 提供 tests + excludes | 边界 |

---

## 不可测试项

- **AC-1…AC-7 的最终 mutation score 数值** — **原因**: score 只能由 Stryker（或 `dev-team test-execution` mutation 阶段）在验证阶段重算；本设计产出的是杀伤用例清单，不能在单元测试内直接断言整文件 mutationScore≥60。
- **AC-9 生产改动边界** — **原因**: 属流程/评审门禁（diff 审查与变更说明），非自动化功能断言；若补测发现真实 bug，最小修复须人工单列。
- **明显等价变异体（Equivalent mutants）** — **原因**: 如仅改日志文案顺序、best-effort catch 空块、对可观测结果无影响的中间变量替换等，无法用合理断言杀死；依赖其它分支覆盖拉高整体分数，并在实现时优先杀非等价 Survived。
- **私有辅助函数的直接单测** — **原因**: `preparePlanArtifacts`、`normalizeSourceFilesForStryker`、`mapGoStatus`、`parseBunOutput`、`derivePlanId`、`collectFiles` 等未导出；项目禁止 test-only `export`。一律经公共 API / IO 间接覆盖。
- **`resolvePluginPackage` 对未知 testRunner 的回退分支** — **原因**: 当前 `resolveTestRunner` 仅放行 jest/vitest，未知 runner 在更早处抛错，回退模板 `` `@stryker-mutator/${testRunner}` `` 在生产路径上不可达（潜在死代码/等价不可测）；不为此新增 production export。
