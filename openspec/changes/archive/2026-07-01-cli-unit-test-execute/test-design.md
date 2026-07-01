# 测试设计: CLI 单元测试执行下沉

> **日期**: 2026-06-30

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `dev-team unit-test` CLI 命令：调用 `runTestDetectFrameworks` 获取 plan，对每个 framework 执行测试，生成子报告和汇总报告 JSON | 集成测试 | `plugins/dev-team/bin/__tests__/cli-unit-test-execute/cli-unit-test-execute.test.ts` | cli-unit-test-execute |
| AC-1 | `dev-team unit-test` CLI 命令：调用 `runTestDetectFrameworks` 获取 plan，对每个 framework 执行测试，生成子报告和汇总报告 JSON | 单元测试 | `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest |
| AC-1 | `dev-team unit-test` CLI 子命令注册 | 单元测试 | `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration |
| AC-2 | pytest test_cmd 包含链式命令模式 `; _X=$?; pytest --cov=` 和 `exit $_X` | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 |
| AC-2 | rust test_cmd 包含链式命令模式 `; _X=$?; cargo llvm-cov` 和 `exit $_X` | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 |
| AC-3 | `test_cmd` 占位符 `{files}`、`{directory}`、`{project_root}` 在执行时被替换 | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution |
| AC-3 | test_cmd 在 FrameworkConfig 和 plan 中为模板字符串格式 | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- test_cmd 模板化 |
| AC-4 | 给定 vitest/jest JSON stdout，json-parser 提取 `total`、`passed`、`failed`、`skipped` 及 `test_cases[]` | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput |
| AC-4 | go test -json line-delimited JSON 解析：逐行解析，过滤 Action=pass/fail/skip | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput |
| AC-4 | Parser dispatch 按 format 分发到 json-parser 和 go-parser | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/index.test.ts` | parseTestOutput -- dispatch |
| AC-5 | 给定 bun/cargo/node-test 文本 stdout，text-parser 提取 `total`、`passed`、`failed`、`skipped` | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput |
| AC-5 | Parser dispatch 默认分发到 text-parser | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/index.test.ts` | parseTestOutput -- default dispatch |
| AC-6 | 给定每种覆盖率格式的输出文件，coverage-parser 提取 `lines`、`branches`、`functions`（支持 null 维度） | 单元测试 | `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput |
| AC-7 | 汇总报告含 `conclusion`、`problems[]`、`coverage`（pass/measured/thresholds/by_framework）、`total`、`passed`、`failed`、`skipped`、`duration`，不含用例和文件明细 | 单元测试 | `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport |
| AC-7 | 汇总报告 Zod schema 验证 | 单元测试 | `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema |
| AC-8 | 子报告含 `test_cases[]`、`test_files[]`、`source_files[]`、`file_coverage`、`coverage`、`summary`、`exit_code` | 单元测试 | `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport |
| AC-8 | 子报告 Zod schema 验证 | 单元测试 | `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema |
| AC-9 | `coverage.pass` 为布尔值，基于 `measured >= thresholds` 判定（null 维度跳过） | 单元测试 | `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage threshold |
| AC-10 | 链式命令中 pytest 按顺序执行测试+覆盖率，退出码为测试退出码 | 集成测试 | `plugins/dev-team/bin/__tests__/chained-command/chained-command.test.ts` | chained-command |
| AC-10 | 链式命令中即使测试失败，覆盖率命令仍执行 | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- chained command |
| AC-10 | 链式命令退出码反映测试命令退出码 | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- chained command |
| AC-11 | 非零退出码时 test-runner 记录失败信息和退出码，不阻塞其他 framework | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- non-zero exit |
| AC-11 | 非零退出码处理端到端 | 集成测试 | `plugins/dev-team/bin/__tests__/non-zero-exit-code/non-zero-exit-code.test.ts` | non-zero-exit-code |
| AC-12 | FrameworkConfig 无 merge_mode 字段 | 单元测试 | `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 无 merge_mode |
| AC-12 | PlanEntry schema 无 merge_mode 字段 | 单元测试 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | PlanEntry schema -- 无 merge_mode |
| AC-12 | PlanEntry 运行时无 merge_mode 字段 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan 无 merge_mode |

---

## 单元测试

### 用例

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 正向 | pytest test_cmd 含 `; _X=$?; pytest --cov=` | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 正向 | pytest test_cmd 以 `exit $_X` 结尾 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 正向 | rust test_cmd 含 `; _X=$?; cargo llvm-cov` | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 正向 | rust test_cmd 以 `exit $_X` 结尾 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 正向 | rust test_cmd 含 `--output-path coverage/coverage-summary.json` | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 正向 | 非链式框架（vitest/jest/bun/node-test/go/vite-plus）test_cmd 不含 `; _X=$?` | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 正向 | 非链式框架 test_cmd 不含 `exit $_X` 模式 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 异常 | pytest test_cmd 链式语法格式错误（如缺 `;` 分隔符）时可通过正则检测（不期待 `; _X=$?` 模式） | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 异常 | rust test_cmd 链式命令缺少 `exit $_X` 时正则不匹配 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 边界 | pytest test_cmd 链式命令各部分顺序正确：测试命令 → `; _X=$?;` → 覆盖率命令 → `; exit $_X` | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 链式命令 | 边界 | 链式命令含有覆盖率后处理步骤（如 `rm coverage.out`）时仍保留核心链式结构 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 无 merge_mode | 正向 | FrameworkConfig 对象不含 merge_mode 字段 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 无 merge_mode | 正向 | 所有八个框架返回对象均不含 merge_mode | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 无 merge_mode | 边界 | FrameworkConfig 接口字段数量为 8（不含 merge_mode） | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- 无 merge_mode | 边界 | FrameworkConfig 接口无 merge_mode 属性时编译通过 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- test_cmd 模板化 | 正向 | vitest test_cmd 含 `--reporter=json` 和 `{files}` 占位符 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- test_cmd 模板化 | 正向 | go test_cmd 含 `-json` 和 `{directory}` | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- test_cmd 模板化 | 正向 | 所有八框架的 test_cmd 类型为 string 且非空 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- test_cmd 模板化 | 正向 | pytest test_cmd 为链式命令含 `{files}` 占位符 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- test_cmd 模板化 | 边界 | rust test_cmd "cargo test" 不含 `{files}`/`{directory}` 占位符（链式命令的前半部分） | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- test_cmd 模板化 | 边界 | test_cmd 含多个占位符同时出现时模板字符串格式正确 | 修改 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | getFrameworkConfig -- test_cmd 模板化 | 边界 | 单命令框架（jest/vitest/vite-plus/bun/node-test/go）test_cmd 同时含 `{files}` 或 `{directory}` 占位符 | 修改 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | PlanEntry schema -- 无 merge_mode | 正向 | plan 条目不含 `merge_mode` 字段时通过验证 | 修改 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | PlanEntry schema -- 无 merge_mode | 正向 | plan 条目含 `test_cmd: string` 通过验证 | 修改 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | PlanEntry schema -- 无 merge_mode | 异常 | `merge_mode` 出现时被 schema 拒绝（无 passthrough）或 strip | 修改 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | PlanEntry schema -- 无 merge_mode | 异常 | `test_cmd` 为非字符串时拒绝 | 修改 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | PlanEntry schema -- 无 merge_mode | 边界 | plan 条目仅含 `directory`/`framework`/`test_cmd`/`coverage_cmd`/`coverage_format`/`coverage_output`/`coverage_artifacts`/`coverage_cleanup`/`script` 九个字段，无多余字段 | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan 无 merge_mode | 正向 | vitest plan 条目包含 `test_cmd` 但不包含 `merge_mode` | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan 无 merge_mode | 正向 | pytest plan 条目包含 `test_cmd` 但不包含 `merge_mode` | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan 无 merge_mode | 正向 | 多框架每个 plan 条目均包含 `test_cmd` 但不含 `merge_mode` | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan 无 merge_mode | 异常 | plan 条目中 test_cmd 与 getFrameworkConfig 返回值一致 | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan 无 merge_mode | 边界 | plan 条目的 test_cmd 为模板字符串格式（含 `{files}`/`{directory}`/`{project_root}` 占位符），未被替换 | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan 无 merge_mode | 边界 | 大量框架配置（如 8 框架全配置）时 plan 每条目均正确携带 test_cmd | 修改 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks -- plan 无 merge_mode | 边界 | 每个 plan 条目的字段数量为 9（不含 merge_mode） | 修改 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 正向 | `{files}` 占位符替换为以空格分隔的文件列表 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 正向 | `{directory}` 占位符替换为 plan entry 的 directory 值 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 正向 | `{project_root}` 占位符替换为项目根目录路径 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 正向 | 多个占位符同时出现在一个命令中时全部替换 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 边界 | files 为空数组时 `{files}` 替换为空字符串 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 边界 | 命令中无占位符时原样返回 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 边界 | 占位符含有特殊字符时正确替换（如带空格的路径） | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 异常 | `{files}` 中文件路径含特殊 shell 字符时正确转义或引用 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- template substitution | 异常 | `{files}` 为 null 时替换为空字符串 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 单一命令执行 | 正向 | 执行 test_cmd 一次命令（无 merge_mode 分支，统一执行路径） | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 单一命令执行 | 正向 | 从 test_cmd 执行结果提取测试用例和覆盖率 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 单一命令执行 | 正向 | 所有框架（含链式命令和单命令）共享同一执行路径 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 单一命令执行 | 异常 | 执行命令超时时返回 error 字段，不崩溃 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 单一命令执行 | 异常 | test_cmd 为空字符串时返回 error，不执行子进程 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 单一命令执行 | 边界 | 执行命令返回空 stdout 时，parsed testCases 为空数组 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 单一命令执行 | 边界 | 执行命令不可识别（如 binary 不存在）时 exitCode 非零 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 链式命令 | 正向 | pytest 链式命令：测试命令先执行，覆盖率命令后执行 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 链式命令 | 正向 | 链式命令退出码反映测试命令的退出码（`exit $_X`） | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 链式命令 | 正向 | 链式命令中覆盖率从文件读取（`coverage.json` / `coverage/coverage-summary.json`）而非 stdout | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 链式命令 | 异常 | 链式命令中测试命令失败时覆盖率命令仍执行（`; _X=$?;` 而非 `&&`） | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 链式命令 | 异常 | 链式命令中测试命令失败，最终退出码为测试退出码 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 链式命令 | 边界 | 链式命令中覆盖率命令失败但测试通过，最终退出码为 0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 链式命令 | 边界 | 链式命令 stdout 包含测试输出和覆盖率命令输出，解析器只关心测试输出 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 非零退出码 | 正向 | 退出码非零时记录 exitCode 和错误信息到 ExecutionResult | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 非零退出码 | 正向 | 非零退出码不影响其他 framework 执行 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 非零退出码 | 边界 | 退出码为 0 时 error 字段为 undefined | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 非零退出码 | 边界 | 退出码为负数（如被信号终止）时处理 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry -- 非零退出码 | 边界 | 退出码为非数字类型时转换为数字或返回 error | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput -- vitest JSON | 正向 | 解析 vitest JSON reporter 输出，正确提取 total/passed/failed/skipped/test_cases | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput -- jest JSON | 正向 | 解析 jest JSON reporter 输出，正确提取 total/passed/failed/skipped/test_cases | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput -- vitest JSON | 正向 | 解析 test_cases 每个用例含 name/status/duration_ms/file/line | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput -- vitest JSON | 正向 | 解析 failed 用例含 error_type/error_message/stack_trace | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput | 异常 | 非 JSON 字符串输入时抛出错误或返回错误结果 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput | 异常 | JSON 结构缺少 testResults 字段时处理 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput | 异常 | testResults 数组为空时返回总数为 0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput | 边界 | 超长 JSON 输入（>10000 字符）时正确解析 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | parseJsonOutput | 边界 | 所有用例 passed 时 total === passed, failed=0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput | 正向 | 逐行解析 go test -json line-delimited JSON，过滤 Action=pass/fail/skip 的行 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput | 正向 | 正确计算 total/passed/failed/skipped | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput | 正向 | 提取 test_cases 含 name/status/duration_ms | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput | 异常 | 空输入时返回所有计数为 0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput | 异常 | Action 为 pass/fail/skip 外的行被忽略 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput | 异常 | 非 JSON 行被忽略不中断解析 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput | 边界 | 超长 line-delimited JSON 输出（1000+ 行）时正确解析 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | parseGoOutput | 边界 | 同 Test 名出现多次时只取最终 Action 状态 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput -- bun | 正向 | 解析 bun test 文本输出提取 total/passed/failed/skipped | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput -- cargo | 正向 | 解析 cargo test 文本输出提取 total/passed/failed/skipped | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput -- node-test | 正向 | 解析 node --test 文本输出提取 total/passed/failed/skipped | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput -- fallback | 正向 | 多层 fallback 策略：精确匹配 -> 正则提取 -> 按行 heuristic | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput | 异常 | 空 stdout 时返回所有计数为 0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput | 异常 | 未知格式的文本输出不抛出异常，返回最佳猜测或计数 0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput | 边界 | 输出中只有 passed 用例时 skipped=0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput | 边界 | 输出中只有 failed 用例时 passed=0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput | 边界 | 混合 pass/fail/skip 用例时三个计数均正确 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | parseTextOutput | 边界 | 超长文本输出（>10000 行）时正确解析 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput -- istanbul | 正向 | 解析 istanbul JSON 输出提取 lines/branches/functions | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput -- llvm-cov | 正向 | 解析 llvm-cov JSON 输出提取 lines/branches/functions | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput -- go-cover | 正向 | 解析 go-cover 文本输出提取 lines（branches=null, functions=null） | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput -- coverage-py | 正向 | 解析 coverage-py JSON 输出提取 lines/branches（functions=null） | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput -- node-test | 正向 | 解析 node-test 文本输出提取 lines/branches/functions | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput -- istanbul | 正向 | 文件级覆盖率和汇总覆盖率均提取 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput | 异常 | 文件不存在时抛出或返回错误 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput | 异常 | 不支持的文件格式时处理 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput | 异常 | 空文件或非法格式时处理 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput | 边界 | lines/branches/functions 为 null 时该维度标记为 null 而非 0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput | 边界 | 所有维度均为 100% 时正确解析 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput | 边界 | 所有维度均为 0% 时正确解析 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | parseCoverageOutput | 边界 | 覆盖率百分比为小数（如 85.57）时正确保留精度 | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/index.test.ts` | parseTestOutput -- dispatch | 正向 | `format="json"` 时分派到 json-parser | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/index.test.ts` | parseTestOutput -- dispatch | 正向 | `format="go-json"` 时分派到 go-parser | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/index.test.ts` | parseTestOutput -- dispatch | 正向 | 其他 format（或省略）时分派到 text-parser | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/index.test.ts` | parseTestOutput -- dispatch | 异常 | 未知 format 字符串时不抛出，fallback 到 text-parser | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/index.test.ts` | parseTestOutput -- dispatch | 边界 | format 为空字符串时 fallback 到 text-parser | 新增 |
| `plugins/dev-team/bin/src/lib/test-parser/index.test.ts` | parseTestOutput -- dispatch | 边界 | format 大小写敏感：`"JSON"` 不匹配 `"json"` | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 正向 | 子报告含 framework、timestamp、duration_ms、exit_code、summary | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 正向 | 子报告含 test_cases 数组，每个用例含 name/status/duration_ms | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 正向 | 子报告含 test_files、source_files 数组 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 正向 | 子报告含 file_coverage（支持 null） | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 正向 | 子报告含 coverage 对象（measured/thresholds/pass，支持 null） | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 异常 | execution result 中 exit_code 非零时 exit_code 正确传递 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 边界 | 0 个 test_cases 时 summary 中 total=0 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 边界 | 大量 test_cases（>1000）时报告正确生成 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 幂等 | 相同输入重复调用两次应产生相同子报告对象（字段值一致） | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSubReport | 幂等 | 子报告文件已存在时重新写入内容一致无副作用 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 正向 | 汇总报告含 phase、command、timestamp、duration_seconds | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 正向 | 汇总报告含 total/passed/failed/skipped 聚合值 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 正向 | 汇总报告含 conclusion（pass/fail/error） | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 正向 | 汇总报告含 problems 数组，每项含 framework/type/message | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage threshold | 正向 | coverage.measured >= coverage.thresholds 时 pass=true | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage threshold | 正向 | 任一维度 measured < thresholds 时 pass=false | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage threshold | 正向 | measured 某维度为 null 时跳过该维度判定 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage threshold | 正向 | 所有 measured 维度均为 null 时该维度跳过不影响其他维度 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage threshold | 正向 | coverage.by_framework 包含每个 framework 的 measured 值 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage threshold | 边界 | measured 某维度恰好等于 threshold（如 80.0 == 80.0）时 pass=true | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage threshold | 异常 | thresholds 对象缺失或 undefined 时不崩溃，有合适的 fallback 行为 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage weighted average | 正向 | 加权平均各维度按 source_files.length 加权 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage weighted average | 正向 | 某框架某维度为 null 时不参与该维度加权平均 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage weighted average | 正向 | 所有框架某维度均为 null 时总体该维度为 null | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage weighted average | 边界 | 所有框架 source_files 等长时退化为简单平均 | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport -- coverage weighted average | 异常 | 某框架 source_files 为空数组时分母不含该项（不参与加权） | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 异常 | 所有子报告测试均失败时 conclusion=fail | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 异常 | 没有子报告时 conclusion=pass（未执行不视为失败） | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 边界 | 汇总报告不包含 test_cases 和 test_files 明细字段（AC-7） | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 边界 | 子报告 coverage 全为 null 时汇总报告 coverage=null | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 幂等 | 相同子报告列表重复调用两次应产生相同汇总报告对象（字段值一致） | 新增 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | generateSummaryReport | 幂等 | 汇总报告文件已存在时重新写入内容一致无副作用 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema | 正向 | 完整子报告对象通过 schema 验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema | 正向 | file_coverage=null 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema | 正向 | coverage=null 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema | 异常 | 缺少必填字段（如 framework、summary）时拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema | 异常 | test_cases 元素缺少必填字段时拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema | 异常 | summary 中 total/passed/failed/skipped 类型错误时拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema | 边界 | test_cases 为空数组时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSubReport schema | 边界 | test_files/source_files 为空数组时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema | 正向 | 完整汇总报告对象通过 schema 验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema | 正向 | coverage=null 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema | 正向 | problems 为空数组时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema | 异常 | 缺少必填字段（如 conclusion、total）时拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema | 异常 | conclusion 非 pass/fail/error 时拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema | 异常 | coverage.measured 中 lines 超出 0-100 范围时拒绝 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema | 边界 | duration_seconds=0 时通过验证 | 新增 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | UnitTestSummaryReport schema | 边界 | total=0 时通过验证（无测试用例） | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 正向 | 调用 `runTestDetectFrameworks` 获取 plan | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 正向 | 对 plan 中每个 framework 调用 `executePlanEntry` | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 正向 | 在每个 framework 执行后调用 `generateSubReport` 写入子报告 | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 正向 | 所有 framework 执行后调用 `generateSummaryReport` 写入汇总报告 | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 正向 | 子报告写入 `reports/unit-test/<framework>.json` | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 正向 | 汇总报告写入 `reports/unit-test-execution.json` | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 异常 | plan 为空时不执行任何测试，退出码 0 | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 异常 | 某个 framework 执行失败不阻塞后续 framework | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 边界 | options.projectRoot 为 undefined 时使用默认 project dir | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 幂等 | 相同 plan 重复执行两次 generateSubReport 产生相同的子报告内容 | 新增 |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | runUnitTest | 幂等 | 相同 plan 重复执行两次 generateSummaryReport 产生相同的汇总报告内容 | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration | 正向 | CLI 注册 `unit-test` 子命令 | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration | 正向 | 命令接受 `--project-root` 选项 | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration | 正向 | 命令 action 调用 `runUnitTest` | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration | 正向 | `runUnitTest` 返回非零时进程以 exit(1) 退出 | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration | 异常 | 未注册 `unit-test` 子命令时 CLI 报错而非静默忽略 | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration | 异常 | `--project-root` 后缺省值时 CLI 报错 | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration | 边界 | `--project-root` 值为空字符串时使用默认 project dir | 新增 |
| `plugins/dev-team/bin/src/cli.test.ts` | dev-team unit-test command registration | 边界 | 同时传递多个未知选项时不影响 unit-test 命令正常注册和执行 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | exec-command / child_process | Mock `execSync` 或 `spawnSync`，返回可控的 stdout/stderr/exitCode | executePlanEntry -- template substitution, chained command, non-zero exit |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | fs.readFileSync | Mock 文件读取，返回预设的覆盖率文件内容 | parseCoverageOutput |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | test-runner, test-report | Mock `executePlanEntry` 和 `generateSubReport`/`generateSummaryReport`，验证编排逻辑而非实际执行 | runUnitTest |
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | test-detect-frameworks | Mock `runTestDetectFrameworks` 返回预设 plan | runUnitTest |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | fs.writeFileSync / fs.mkdirSync | Mock 文件系统 API，验证写入路径和内容；验证重复写入的幂等性 | generateSubReport, generateSummaryReport, 幂等性验证 |

---

## 集成测试

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-1 | `plugins/dev-team/bin/__tests__/cli-unit-test-execute/cli-unit-test-execute.test.ts` | cli-unit-test-execute | 运行 `dev-team unit-test` 时调用 `runTestDetectFrameworks` 获取 plan | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/cli-unit-test-execute/cli-unit-test-execute.test.ts` | cli-unit-test-execute | 对每个 framework plan entry 执行测试命令 | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/cli-unit-test-execute/cli-unit-test-execute.test.ts` | cli-unit-test-execute | 在 `reports/unit-test/<framework>.json` 生成子报告 | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/cli-unit-test-execute/cli-unit-test-execute.test.ts` | cli-unit-test-execute | 在 `reports/unit-test-execution.json` 生成汇总报告 | 新增 |
| AC-1 | `plugins/dev-team/bin/__tests__/cli-unit-test-execute/cli-unit-test-execute.test.ts` | cli-unit-test-execute | 重复执行 CLI 命令时报告文件内容一致（幂等性） | 新增 |
| AC-10 | `plugins/dev-team/bin/__tests__/chained-command/chained-command.test.ts` | chained-command | 链式命令中测试+覆盖率按序执行，退出码为测试退出码 | 新增 |
| AC-10 | `plugins/dev-team/bin/__tests__/chained-command/chained-command.test.ts` | chained-command | 链式命令中测试失败时覆盖率仍执行 | 新增 |
| AC-10 | `plugins/dev-team/bin/__tests__/chained-command/chained-command.test.ts` | chained-command | 链式命令覆盖率从文件读取而非 stdout | 新增 |
| AC-10 | `plugins/dev-team/bin/__tests__/chained-command/chained-command.test.ts` | chained-command | 链式命令 stdout 包含测试输出和覆盖率命令输出 | 新增 |
| AC-11 | `plugins/dev-team/bin/__tests__/non-zero-exit-code/non-zero-exit-code.test.ts` | non-zero-exit-code | 测试命令非零退出时记录失败和退出码 | 新增 |
| AC-11 | `plugins/dev-team/bin/__tests__/non-zero-exit-code/non-zero-exit-code.test.ts` | non-zero-exit-code | 非零退出不阻塞同项目其他 framework 执行 | 新增 |
| AC-11 | `plugins/dev-team/bin/__tests__/non-zero-exit-code/non-zero-exit-code.test.ts` | non-zero-exit-code | 所有 framework 执行完成后退出码反映汇总结果 | 新增 |
| AC-12 | `plugins/dev-team/bin/__tests__/cli-unit-test-execute/cli-unit-test-execute.test.ts` | cli-unit-test-execute | FrameworkConfig 和 PlanEntry 不含 merge_mode 字段 | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/cli-unit-test-execute/cli-unit-test-execute.test.ts` | child_process / exec | 使用实际子进程执行 CLI，但通过临时目录和 mock 配置文件控制行为；通过重复执行验证幂等性 | cli-unit-test-execute |
| `plugins/dev-team/bin/__tests__/chained-command/chained-command.test.ts` | child_process / exec | 使用实际子进程执行 CLI，通过 fixture 项目验证链式命令行为 | chained-command |
| `plugins/dev-team/bin/__tests__/non-zero-exit-code/non-zero-exit-code.test.ts` | child_process / exec | Mock 测试命令返回非零退出码或用 fixture 项目验证 | non-zero-exit-code |

---

## 不可测试项


- `parseCoverageOutput` 实际读取文件系统的行为：单元测试中通过 mock fs 模块解决，集成测试中通过 fixture 文件验证。

- CLI 命令的进程退出行为（`process.exit`）：单元测试中验证 `cli.ts` action handler 调用 `runUnitTest` 并传递退出码；`process.exit` 本身不在进程内测试（会终止测试运行器），通过集成测试验证实际 CLI 命令的行为。

- 链式命令中覆盖率命令 `coverage_cmd` 字段保留在 `FrameworkConfig` 中但不再被 runner 使用：通过单元测试验证 test-runner 不调用 `coverage_cmd`，仅执行 `test_cmd`。
