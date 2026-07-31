# 任务: test-execution-result-file

> **变更**: test-execution-result-file
> **日期**: 2026-07-31

---

## 阶段 1: 报告布局与 planId（路径层）

- [x] **1.1** 导出并修正 `derivePlanId` — 在 `lib/test-report.ts` 中导出；返回目录 id（去掉 `.json`）；`directory === '.'` → `"<framework>"`；嵌套目录 → `sanitize(directory)_<framework>`；修正过时 JSDoc（`_vitest`）
- [x] **1.2** 原子报告落盘 — `generateSubReport` 写入 `path.join(reportsDir, planId, 'report.json')`（先 mkdir）
- [x] **1.3** summary 落盘 — `generateSummaryReport` 写入 `path.join(reportsDir, 'summary.json')`，删除写兄弟文件 `../test-execution.json` 的逻辑
- [x] **1.4** `plans[]` schema — 在 `schemas/test-execution-output.schema.ts` 为 summary 增加 `plans` 数组（`id`/`framework`/`directory`/`path`）；导出 `PlanIndexEntry` 类型
- [x] **1.5** 填充 `plans[]` — 汇总时为每个尝试执行的 plan 投影索引；`path` 相对 project root（POSIX）；半失败 plan 仍包含；不写 status
- [x] **1.6** CLI 报告根 — `commands/test-execution.ts` 的 `resolveReportsDir` 改为 `…/reports/test`（有/无 `--change`）

## 阶段 2: Registry 与 detect 占位符

- [x] **2.1** `FrameworkConfig` / `coverage_format` — 联合类型与 registry 增加 `'lcov'`；同步 `test-detect-frameworks.schema.ts` enum
- [x] **2.2** 垂直 `coverage_output` — 八框架改为相对 reportDir 文件名（如 `coverage-summary.json` / `lcov.info` / `func-summary.txt` / `coverage.json` 等，对齐 design 终表）
- [x] **2.3** 移除 suite cwd cleanup — 删除或停用 `shell`/`cmd.coverage_cleanup`；`test-detect-frameworks` 不再注入 `rm -rf` / `rmdir` 清理行
- [x] **2.4** 模板占位符 — 各框架 `test_execution` 嵌入 `{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}`（及 go 的 `{coverprofile_file}` 如需）
- [x] **2.5** jest 原生文件通道模板 — 加入 `--outputFile={results_file}`、`--coverageDirectory={report_dir}`、`--coverageReporters=json-summary`；**不**依赖壳层 `>`
- [x] **2.6** vitest / vite-plus 模板 — CLI 指定 json `outputFile` 与 coverage `reportsDirectory`（按待决项钉死后写入）；默认不写临时 config
- [x] **2.7** bun registry 纠正 — 去掉伪 `--coverageReporters=json-summary`；`coverage_format: 'lcov'`；`coverage_output: 'lcov.info'`；`config_flag` 设为可显式注入（如 `'--config'`）
- [x] **2.8** C 类模板 — go / rust / pytest / node-test：原生 path 旗标指向 planDir 占位符；测试段留给 execute 条件 `>`
- [x] **2.9** detect 推迟展开 — `runTestDetectFrameworks` 生成的 script **保留** `{config_args}` 与 report 相关占位符，不在 detect 阶段展开 report 路径

## 阶段 3: preparePlanArtifacts 与 execute 管线

- [x] **3.1** 定义 `PreparePlanArtifactsInput` / `PreparePlanArtifactsResult` / `PlanPlaceholders` — 落在 `lib/test-runner.ts`
- [x] **3.2** 实现 `preparePlanArtifacts` — 按框架填充 placeholders（相对 absCwd）；设置 `redirectStdoutToResults`；A 类默认无 temp；返回 `configArgs`（用户 config 或 bun 临时 config）
- [x] **3.3** bun 临时 bunfig — 在 absCwd 写 `bunfig.dev-team-<rand>.toml`（overlay `coverageDir`/`coverageReporter=["lcov"]`，只读合并用户 bunfig）；加入 `tempPaths`；失败路径也要能 cleanup
- [x] **3.4** `executePlanEntry` 接入 reportsDir — options 增加必填 `reportsDir`；算 `planId`/`reportDir`；mkdir；**仅清空当前** plan 目录
- [x] **3.5** 占位符展开与条件 `>` — prepare → expand `{config_args}` → substitute（含 `{files}` 等）→ 若 `redirectStdoutToResults` 则对测试结果段追加重定向（shell / cmd 分别处理）；原生 outputFile 族不追加
- [x] **3.6** 扩展 `ExecutionResult` — 增加 `planId`、`reportDir`、`resultsFile?`；prepare/解析失败写 `error` 并仍可生成空/半失败结果
- [x] **3.7** CLI 传参 — `runTestExecution` 将 `reportsDir` 传入每次 `executePlanEntry`；用返回的 result 生成 sub/summary（含 `plans[]`）
- [x] **3.8** temp cleanup — execute 结束（成功或失败）best-effort 删除 `tempPaths`；不改用户长期 config

## 阶段 4: 垂直 parser（文件通道）

- [x] **4.1** 新增 `parsePlanArtifacts(framework, reportDir)` — 在 `test-parser/index.ts` 分发；execute 主路径改调此 API，**不再**对整段 stdout 做 jest/vitest `JSON.parse`
- [x] **4.2** JS 族文件解析 — jest/vitest/vite-plus：读 `results.json` + 经 `parseCoverageFromFile(..., 'istanbul')` 读 `coverage-summary.json`；无侧车则 coverage=null；**无** coverageMap fallback；**不**读 suite cwd 旧路径
- [x] **4.3** go 文件解析 — 读 `results.ndjson` + `func-summary.txt`（必要时 `coverage.out`）
- [x] **4.4** 文本族文件解析 — bun/rust/pytest/node-test：从 planDir 约定 `results.*` 读入再复用/适配现有 text parser
- [x] **4.5** lcov 支持 — `coverage-parser.ts` 增加 `'lcov'` 分支，归一为 `ParsedCoverage`；bun 使用之
- [x] **4.6** 缺失/空结果文件 — 命令非 0 且约定结果文件缺失/空/无法解析时，记 execution_error（或等价），仍写 `report.json` 并进入 `plans[]`

## 阶段 5: Mutation（Stryker → planDir）

- [x] **5.1** `resolveStrykerConfig` 增加 `reportDir` — `jsonReporter.fileName` 指向 plan 目录下 `mutation.json`（保证相对 absCwd 解析后落在 reportDir）
- [x] **5.2** runner mutation 读取路径 — `buildMutationBlockFromReport` 改为读 `join(reportDir, 'mutation.json')`，**不**读 `reports/mutation/`
- [x] **5.3** `mutation-parser.ts` 文档对齐 — 更新 JSDoc/注释中的默认报告路径为 planDir 下 `mutation.json`；`parseMutationReport` 签名与行为不变（仍接受绝对路径，由调用方传入）
- [x] **5.4** 临时文件原则 — 临时 stryker config 仍在 absCwd；用后删；不改用户长期 Stryker 配置

## 阶段 6: Agent / Skill 文案

- [x] **6.1** 更新 `agents/test-execution-executor.md` — 读 `reports/test/summary.json`；经 `plans[]` 打开 `{path}/report.json`；删除旧 `test-execution.json` / `<framework>.json` 指引
- [x] **6.2** 更新 `agents/test-execution-evaluator.md` — 聚合报告路径改为 `reports/test/summary.json`
- [x] **6.3** 核对 `skills/phase-test-execution/SKILL.md` — 若存在旧报告路径字面量则改为新路径；否则确认无需改动

## 阶段 7: 收尾与实现期核对

- [x] **7.1** 关闭 C3 待决 — 实证 jest CLI 覆盖用户 config 输出键；不足则仅该框架加临时 config overlay
- [x] **7.2** 钉死 vitest/vite-plus CLI — 确认 `outputFile` / `reportsDirectory` 旗标并回写 registry 模板
- [x] **7.3** 钉死 bun `--config=` — 确认参数位置；失败降级：`>` + `coverage=null`
- [x] **7.4** Windows cmd 链式重定向 — 与现有 rust `errorlevel` 模式对齐 go/pytest/rust 段级 `>`
- [x] **7.5** 清理旧路径引用 — 源码/注释/agent 中移除 `reports/test-execution`、suite cwd `coverage/` 作为权威源的描述；**不做**双读或迁移
