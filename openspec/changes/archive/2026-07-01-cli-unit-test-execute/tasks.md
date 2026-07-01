# 任务: CLI 单元测试执行下沉

> **变更**: cli-unit-test-execute
> **版本**: v2 (backtrack — 移除 merge_mode，统一链式命令)

---

## 阶段 1: FrameworkConfig 改造（移除 merge_mode，链式命令）

- [x] **1.1 FrameworkConfig 接口移除 merge_mode** — 在 `lib/test-framework.ts` 中从 `FrameworkConfig` 接口删除 `merge_mode: boolean` 字段
- [x] **1.2 各框架 test_cmd 模板化** — 更新 `FRAMEWORK_REGISTRY` 中各框架 `test_cmd`：
  - jest: `npx jest --verbose --json --coverage --coverageReporters=json-summary {files}`
  - vitest: `npx vitest run --reporter=json --coverage --coverage.reporter=json-summary {files}`
  - vite-plus: `vp test --coverage --coverage.reporter=json-summary {files}`
  - bun: `bun test --coverage --coverageReporters=json-summary {files}`
  - node-test: `node --test --experimental-test-coverage {files}`
  - go: `go test -json -coverprofile=coverage.out -covermode=atomic {directory}`
  - pytest: `pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X`（链式命令）
  - rust: `cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X`（链式命令）
- [x] **1.3 删除各框架 merge_mode 字段** — 从所有 8 个框架条目的注册数据中移除 `merge_mode` 属性

## 阶段 2: PlanEntry 改造（移除 merge_mode）

- [x] **2.1 PlanEntry 类型移除 merge_mode** — 在 `commands/test-detect-frameworks.ts` 中从 `PlanEntry` 接口删除 `merge_mode: boolean`
- [x] **2.2 buildPlanFromMappings 不再填充 merge_mode** — 删除 `config.merge_mode` 的读取和传递
- [x] **2.3 test-detect-frameworks schema 移除字段** — 在 `schemas/test-detect-frameworks.schema.ts` 的 `plan` 条目 Zod schema 中删除 `merge_mode: z.boolean()`

## 阶段 3: 输出报告 Zod Schema

- [x] **3.1 定义 TestCaseResultSchema** — Zod v4 schema：name（string）、file（string）、duration_ms（number nullable）、status（enum passed/failed/skipped）、line（optional number）、error_type/message/stack_trace（optional string）
- [x] **3.2 定义 sub-report schema** — `unitTestSubReportSchema`：framework、timestamp、exit_code、duration_ms、summary（total/passed/failed/skipped）、test_cases（TestCaseResultSchema 数组）、test_files/source_files（string 数组）、file_coverage（Record nullable）、coverage（对象 nullable，含 measured/thresholds/pass）、findings（optional string 数组）
- [x] **3.3 定义 summary-report schema** — `unitTestSummaryReportSchema`：phase、command、timestamp、duration_seconds、total/passed/failed/skipped、conclusion（enum pass/fail/error）、problems（数组）、coverage（含 pass/measured/thresholds/by_framework，整体 nullable）；SHALL NOT 包含 test_cases 或 test_files 字段

## 阶段 4: 测试用例解析器 — Dispatch

- [x] **4.1 定义 ParsedTestResult 和 TestCase 类型** — 在 `lib/test-parser/index.ts` 中定义公共接口：ParsedTestResult（total/passed/failed/skipped/testCases/testFiles/sourceFiles/error），TestCase（name/file/durationMs/status/line/errorType/errorMessage/stackTrace）
- [x] **4.2 实现 parseTestOutput dispatch 函数** — 根据 framework 名称路由：`vitest`/`jest`/`vite-plus` → `parseJsonOutput`；`go` → `parseGoOutput`；默认（rust/bun/node-test/pytest） → `parseTextOutput`

## 阶段 5: 测试用例解析器 — JSON Parser

- [x] **5.1 实现 parseJsonOutput** — 解析 vitest/jest JSON reporter 输出的 `testResults[].assertionResults[]`：每个 assertionResult 提取 title（与 ancestorTitles 拼接为全名）、status、duration、failureMessages；聚合 total/passed/failed/skipped；提取 testFiles 和 sourceFiles
- [x] **5.2 处理边缘情况** — 无效 JSON 返回 `{ error: "..." }` 结果不抛出异常；含 null/undefined 字段时容错；testResults 为空数组时返回全零计数

## 阶段 6: 测试用例解析器 — Go Parser

- [x] **6.1 实现 parseGoOutput** — 逐行解析 `go test -json` 输出：每行 JSON 解析后检查 `Action` 字段；仅统计 `"pass"`/`"fail"`/`"skip"` 的行作为终态用例；忽略 `"output"` 和 `"run"` 行；提取 `Test` 字段为用例名；从关联的 `"output"` 行中提取失败详情
- [x] **6.2 处理边缘情况** — 非 JSON 行跳过不崩溃；package 级输出（无 Test 字段）过滤；Buffer 行（`=== RUN` 等）忽略

## 阶段 7: 测试用例解析器 — Text Parser

- [x] **7.1 实现 parseTextOutput 第一层：框架特定正则** — bun 的 `[PASS]`/`[FAIL]`/`[SKIP]` 标记匹配；cargo test 的 `test result: ok / FAILED` 汇总行解析（提取 passed/failed/ignored 计数）；node-test 的 `# pass`/`# fail`/`# skip` 行匹配
- [x] **7.2 实现 parseTextOutput 第二层：通用正则 fallback** — 匹配 `Tests: N passed, M failed, S total` 等常见格式；匹配 `PASSED`/`FAILED` 按行统计
- [x] **7.3 实现 parseTextOutput 第三层：逐行 heuristic** — 按行匹配已知 PASS/FAIL 关键词；统计比例；提取未被框架特定正则捕获的结果
- [x] **7.4 处理完全无法解析的情况** — 无任何可识别模式时返回 `{ total: 0, passed: 0, failed: 0, skipped: 0, testCases: [], error: "Unable to parse test output" }` 不抛出异常

## 阶段 8: 覆盖率解析器

- [x] **8.1 定义 ParsedCoverage 类型** — `{ lines: number; branches: number | null; functions: number | null; fileCoverage: FileCoverageEntry[] | null }`；定义 FileCoverageEntry `{ file: string; lines: number; branches: number | null; functions: number | null }`
- [x] **8.2 实现 istanbul 格式解析** — 读取 `coverage-summary.json`，提取 `total.lines.pct` / `total.branches.pct` / `total.functions.pct`；提取文件级覆盖率各条目
- [x] **8.3 实现 llvm-cov 格式解析** — 读取 JSON，提取 `data[0].totals.lines.percent` / `data[0].totals.branches.percent` / `data[0].totals.functions.percent`
- [x] **8.4 实现 node-test 格式解析** — 读取 `node-test-output.txt` 文本表格，regex 匹配 `All files` 行提取各列百分比，branches/functions 可为 null
- [x] **8.5 实现 go-cover 格式解析** — 读取 `func-summary.txt`，regex 匹配 `total:` 行提取 statements 百分比；branches 和 functions 设为 null
- [x] **8.6 实现 coverage-py 格式解析** — 读取 `coverage.json`，提取 `totals.percent_covered` → lines；`totals.percent_covered_branches` → branches（存在时）；functions → null
- [x] **8.7 实现 parseCoverageFromFile 主函数** — 根据 format 参数路由到子解析方法；文件不存在或格式错误时返回 null（由调用方处理）

## 阶段 9: 执行编排 (Test Runner) — 简化（无 merge_mode 分支）

- [x] **9.1 定义 ExecutionResult 类型** — `{ framework: string; exitCode: number; stdout: string; stderr: string; testCases: TestCase[]; coverage: ParsedCoverage | null; durationMs: number; testFiles: string[]; sourceFiles: string[]; error?: string }`
- [x] **9.2 实现模板占位符替换函数** — 将 test_cmd 中的 `{files}`（空格分隔的文件路径）、`{directory}`（entry.directory）、`{project_root}`（projectRoot）替换为实际值；不存在的占位符保持原样
- [x] **9.3 实现 executePlanEntry（单一命令模式，无 merge_mode 分支）** — cd 到 entry.directory → 替换模板占位符 → 执行 test_cmd（不再检查 merge_mode，统一执行单一 shell 命令）→ 捕获 stdout/stderr/exitCode/duration → 调用 `parseTestOutput` 解析测试用例 → 查找覆盖率文件 → 调用 `parseCoverageFromFile`
- [x] **9.4 删除 executeMerged 和 executeTwoPhase 分支函数** — 移除 `executeMerged()` 和 `executeTwoPhase()` 函数；`executePlanEntry` 不再包含 `if (entry.merge_mode)` 分支逻辑；`coverage_cmd` 字段不再被 runner 使用（链式命令已内嵌覆盖率）
- [x] **9.5 非零退出码处理** — 测试命令以非零退出码退出时：记录 exitCode 和 stderr 到结果；仍尝试解析 stdout 中的部分用例结果；不阻塞其他 framework 执行；失败的 framework 在子报告中标记 exit_code 和 findings
- [x] **9.6 实现 source_files 推导** — 从 test_files 中去掉 `.test.`/`_test.`/`test_` 等测试文件名标记，匹配项目中对应的源文件路径

## 阶段 10: 报告生成

- [x] **10.1 实现 generateSubReport** — 接收 framework 名和 ExecutionResult：组装 UnitTestSubReport 对象（含 summary、test_cases、test_files、source_files、file_coverage、coverage 结论）；写入 `reports/unit-test/<framework>.json`；创建目录（如果不存在）
- [x] **10.2 实现 generateSummaryReport** — 聚合所有子报告：求和 total/passed/failed/skipped/duration_ms；计算加权平均覆盖率（按 source_files.length 加权，null 维度跳过）；执行全局阈值判定得到 coverage.pass；收集失败用例转为 problems[]；写入 `reports/unit-test-execution.json`
- [x] **10.3 实现覆盖率加权平均** — 按 source_files.length 为权重，每个维度独立计算：`weighted_dim = Σ(fw.measured.dim * fw.source_files.length) / Σ(fw.source_files.length)`（仅非 null 维度参与）；全 null 时该维度为 null
- [x] **10.4 实现 overrides 阈值判定** — 从 config.json 读取 `test.coverage.overrides`；按 file_coverage 匹配 override 的 glob 分组；各分组独立计算 measured、与 thresholds 比对得到 pass；加入汇总报告 overrides 数组
- [x] **10.5 处理所有框架覆盖率失败的情况** — 当全部框架 `parseCoverageFromFile` 返回 null 时：汇总报告 `coverage` 字段设为 null（非 `{ pass: false, ... }`）

## 阶段 11: CLI 命令注册

- [x] **11.1 定义 UnitTestOptions 类型** — `{ change?: string; projectRoot?: string; files?: string[]; framework?: string }`
- [x] **11.2 实现 runUnitTest 主函数** — 解析选项 → 调用 `runTestDetectFrameworks({})` 获取 plan → 读 config 获取阈值 → 遍历 plan 逐 framework 调用 `executePlanEntry` → 逐结果调用 `generateSubReport` → 聚合调用 `generateSummaryReport`
- [x] **11.3 处理 plan 为空** — 当 `runTestDetectFrameworks` 返回空 plan 时（未配置 test.framework），打印提示信息并以退出码 0 退出
- [x] **11.4 处理 --framework 过滤** — 当指定 `--framework` 选项时，只执行匹配该 framework 的 plan entry
- [x] **11.5 注册 unit-test 子命令** — 在 `cli.ts` 中增加 `.command('unit-test', 'Run unit tests with coverage and generate execution report')`，支持 `--change`、`--project-root`、`--files`、`--framework` 选项
- [x] **11.6 退出码映射** — 实现退出码规则：全部通过且覆盖率达标（或未配置）→ 0；存在测试失败或覆盖率不达标 → 1；plan 为空 → 0

## 阶段 12: Agent 定义文件更新

- [x] **12.1 更新 unit-test-executor.md** — 将 agent 的 Process 章节从「执行测试命令和解析输出」改为「读取 CLI 产出的汇总报告」：删除步骤 1-2 的 test_detect_frameworks 调用和 shell 执行；删除步骤 3 的覆盖率产物移动；删除步骤 4 的覆盖率解析；删除步骤 6 的集成测试执行；保留步骤 5 的阈值判定（改为直接从报告读取）；添加「Step 0: 验证 reports/unit-test-execution.json 存在」
