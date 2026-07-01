# 提案: CLI 单元测试执行下沉

> **变更**: cli-unit-test-execute
> **日期**: 2026-06-30
> **状态**: 修订

---

## 问题

当前 dev-team 插件的单元测试执行完全由 **unit-test-executor agent**（sonnet）驱动：Agent 通过 LLM 调用 MCP 工具获取框架配置，用 Bash 执行测试命令，再用 LLM 解析测试输出生成 JSON 报告。这种架构存在三个核心问题：

1. **解析不可靠** — LLM 可能误解测试输出格式（如 vitest JSON 中的字段名、go 的 PASS/FAIL 标识），导致报告中的用例数、通过/失败计数不准确
2. **速度慢** — 每次执行需要多次 LLM 往返：获取框架配置、执行测试、解析输出、生成报告。框架配置已经由 CLI 的 `test_detect_frameworks` 工具提供，但仍需要 Agent 调用 MCP 获取
3. **成本高** — sonnet 模型调用产生显著 token 消耗，而测试执行和输出解析本质上是确定性任务，无需 LLM 参与

此外，当前 `FrameworkConfig` 中的 `test_cmd` 和 `coverage_cmd` 是静态字符串，不支持按文件列表、工作目录等上下文动态替换，限制了 CLI 直接复用的能力。

## 提案

将单元测试的**执行、输出解析、报告生成**下沉到确定性 TypeScript CLI 中，由 `dev-team unit-test` 单一命令完成。Agent 保留高层编排和诊断决策，数据源从直接执行改为读取 CLI 产出的结构化汇总报告。

### 核心设计

1. **CLI 完全替代 executor agent 的执行能力** — `dev-team unit-test` 命令执行测试、解析输出、生成子报告和汇总报告。unit-test-executor agent 的执行角色废弃，仅保留诊断决策树
2. **混合解析器策略** — 优先 JSON 解析（vitest、jest），fallback 文本解析（cargo test、bun、node-test）。Go 使用 line-delimited JSON 解析
3. **一个命令完成** — `dev-team unit-test [options]` 执行+汇总一气呵成，无需 Agent 多次 MCP 往返
4. **子报告 + 汇总报告** — 每个 framework 生成独立子报告（含用例清单、文件清单、文件覆盖率），汇总报告只包含结论/问题/覆盖度指标和实际值，不含明细
5. **命令模板 + 通配符** — FrameworkConfig 的 `test_cmd` 支持 `{files}`、`{directory}`、`{project_root}` 占位符，执行时替换
6. **统一命令模板** — 移除 `merge_mode` 字段。所有框架使用单一 `test_cmd`，利用 shell 链式执行机制（`; _X=$?; ...; exit $_X`）将覆盖率和测试命令合并为一条命令。覆盖率命令写入文件而非 stdout，解析器从文件读取

### 链式命令模式

对于需要两步执行的框架（pytest、rust），`test_cmd` 使用 shell 链式命令：

- **pytest**: `pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X`
- **rust**: `cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X`

关键细节：
- `_X=$?` 保存测试命令的退出码（使用 `;` 而非 `&&`，确保即使测试失败覆盖率命令仍执行）
- `exit $_X` 恢复测试命令的退出码作为最终 shell 退出码
- 覆盖率解析从文件读取（`parseCoverageFromFile`），不受 stdout 影响
- pytest 使用 `-q` 标志减少 stdout 输出量
- rust 使用 `--output-path` 将 JSON 写入文件而非 stdout

---

## 能力

### 新增能力

- **`cli-unit-test-execute`** — `dev-team unit-test` CLI 命令，包含测试执行编排（test-runner）、输出解析（json-parser、go-parser、text-parser）、覆盖率解析（coverage-parser）、报告生成（test-report），以及对应的 Zod schema。完全替代 unit-test-executor agent 的执行能力

### 修改的能力

- **`test-get-framework-config`** — `lib/test-framework.ts` 的 `FrameworkConfig` 接口移除 `merge_mode: boolean` 字段。pytest 和 rust 的 `test_cmd` 更新为链式命令。`test_cmd` 改为模板字符串支持 `{files}`、`{directory}`、`{project_root}` 占位符。各框架条目的 `test_cmd` 相应更新为模板格式
- **`unit-test-executor`** — 执行角色移除，Agent 不再直接执行测试命令和解析输出。Agent 改为读取 CLI 产出的汇总报告 `reports/unit-test-execution.json`，保留诊断决策树和覆盖率阈值判断逻辑

---

## 变更范围

### 实现文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `plugins/dev-team/bin/src/commands/unit-test.ts` | 新增 | CLI 命令 action handler，编排 test-runner + parser + report |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 新增 | 执行编排：替换占位符，执行单一 test_cmd（无 merge_mode 分支） |
| `plugins/dev-team/bin/src/lib/test-parser/index.ts` | 新增 | Parser dispatch：根据输出类型分发到子 parser |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.ts` | 新增 | vitest/jest JSON output 解析 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts` | 新增 | go test -json line-delimited JSON 解析 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts` | 新增 | cargo/bun/node-test 文本输出解析 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.ts` | 新增 | 所有覆盖率格式解析（istanbul/llvm-cov/go-cover/coverage-py） |
| `plugins/dev-team/bin/src/lib/test-report.ts` | 新增 | 子报告 + 汇总报告生成，阈值计算 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` | 新增 | 子报告和汇总报告的 Zod schema |
| `plugins/dev-team/bin/src/lib/test-framework.ts` | 修改 | **移除** `merge_mode` 字段；`test_cmd` 模板化；pytest/rust 更新为链式命令 |
| `plugins/dev-team/bin/src/cli.ts` | 修改 | 注册 `dev-team unit-test` 子命令 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 修改 | `PlanEntry` 接口移除 `merge_mode` 字段 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | 修改 | plan 条目 Zod schema 移除 `merge_mode` |

### 测试文件

| 文件 | 说明 |
|------|------|
| `plugins/dev-team/bin/src/commands/unit-test.test.ts` | CLI 命令集成测试 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | 执行编排单元测试（移除 merge_mode 相关 describe 块，新增链式命令测试） |
| `plugins/dev-team/bin/src/lib/test-parser.test.ts` | Parser dispatch 单元测试 |
| `plugins/dev-team/bin/src/lib/test-parser/json-parser.test.ts` | JSON 解析单元测试 |
| `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts` | Go 解析单元测试 |
| `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts` | 文本解析单元测试 |
| `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.test.ts` | 覆盖率解析单元测试 |
| `plugins/dev-team/bin/src/lib/test-report.test.ts` | 报告生成单元测试 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.test.ts` | Schema 验证测试 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | 更新测试：移除 merge_mode 断言，添加链式命令 test_cmd 断言 |

### 不要修改

- **unit-test-executor agent 定义文件** (`plugins/dev-team/agents/unit-test-executor.md`) — 保留，仅更新其执行角色为读取 CLI 报告。不删除 agent，以保留诊断决策能力
- **unit-test-evaluator agent** — 不变。仍然读取汇总报告做决策树诊断
- **test-detect-frameworks 工具** — 不变。plan[] 输出将不再包含 merge_mode，但检测逻辑不变
- **test-path-resolver** — 不变。仍然由其他流程调用
- **测试代码生成** — test-gen-generator、test-design 等不受影响
- **OpenSpec workflow 流程** — phase 定义、workflow engine 不变

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `dev-team unit-test` CLI 命令 | 运行 `dev-team unit-test` 时，SHALL 调用 `runTestDetectFrameworks` 获取 plan，对每个 framework 执行测试，在 `reports/unit-test/<framework>/` 下生成子报告 JSON，在 `reports/unit-test-execution.json` 下生成汇总报告 JSON |
| AC-2 | pytest 链式命令 | `getFrameworkConfig("pytest").test_cmd` SHALL 包含 `; _X=$?; pytest --cov=` 模式和 `exit $_X` 结尾 |
| AC-3 | 测试命令模板化 | `test_cmd` 占位符 `{files}`、`{directory}`、`{project_root}` 在执行时被替换为实际值 |
| AC-4 | JSON 输出解析 | 给定 vitest JSON stdout，json-parser SHALL 正确提取 `total`、`passed`、`failed`、`skipped` 及 `test_cases[]` |
| AC-5 | 文本输出解析 | 给定 bun/cargo/node-test 的文本 stdout，text-parser SHALL 提取 `total`、`passed`、`failed`、`skipped` |
| AC-6 | 覆盖率解析 | 给定每种覆盖率格式的输出文件，coverage-parser SHALL 提取 `lines`、`branches`、`functions`（支持 null 维度） |
| AC-7 | 汇总报告格式 | 汇总报告 `reports/unit-test-execution.json` SHALL 包含 `conclusion`、`problems[]`、`coverage`（含 pass/measured/thresholds/by_framework）、`total`、`passed`、`failed`、`skipped`、`duration`，SHALL NOT 包含用例和文件明细 |
| AC-8 | 子报告格式 | 子报告 `reports/unit-test/<framework>.json` SHALL 包含 `test_cases[]`、`test_files[]`、`source_files[]`、`file_coverage`、`coverage`、`summary`、`exit_code` |
| AC-9 | 阈值判定 | 汇总报告中 `coverage.pass` SHALL 为布尔值，基于 `coverage.measured >= coverage.thresholds` 判定（null 维度跳过） |
| AC-10 | 链式命令执行顺序和退出码 | pytest 的链式命令 SHALL 按顺序执行：先 `pytest -v`，再覆盖率命令。即使测试失败覆盖率命令仍执行。最终退出码 SHALL 反映测试命令的退出码（通过 `exit $_X`） |
| AC-11 | 非零退出码处理 | 测试命令以非零退出码退出时，test-runner SHALL 记录失败信息和退出码到报告，不阻塞其他 framework 执行 |
| AC-12 | FrameworkConfig 无 merge_mode | `FrameworkConfig` 接口 SHALL NOT 包含 `merge_mode` 字段；`PlanEntry` 接口 SHALL NOT 包含 `merge_mode` 字段；`test-detect-frameworks.schema.ts` 的 plan 条目 SHALL NOT 包含 `merge_mode` 字段 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Agent 到 CLI 的执行数据格式不一致 | 汇总报告字段与 unit-test-evaluator 期望的不匹配，导致诊断决策错误 | 中 | 子报告和汇总报告的 JSON schema 与原有 report schema（test-execution-diagnostics）对齐；evaluator 适配阶段验证所有字段 |
| 文本解析器（text-parser）无法覆盖所有输出格式变体 | 部分框架（如 cargo test 的自定义输出）解析失败或遗漏用例 | 中 | 文本解析器采用多层 fallback 策略：精确匹配 → 正则提取 → 按行 heuristic；解析失败时将原始 stdout 原样保存到报告 `findings` 字段 |
| 链式命令 `_X=$?` 跨平台兼容性 | Windows Git Bash、WSL 或原生 shell 行为不一致 | 低 | `test_cmd` 设计为在 bash shell 中执行（shell: true）；链式命令语法是 POSIX sh 兼容的，在所有 Unix-like shell 中行为一致 |
| 链式命令 stdout 混合 | pytest 覆盖率命令使用 `-q` 减少输出，但 stdout 中仍可能混合测试输出和覆盖率输出 | 低 | 覆盖率解析从文件读取（`coverage.json`），不依赖 stdout；文本解析器只关心测试输出 |
| 子报告文件写入性能 | 大量测试用例（>10000）时 JSON 序列化和写入耗时 | 低 | 子报告写入为流式 JSON 文件写入；汇总报告仅含聚合数据，体量恒定 |
| rust `--output-path` 路径不存在 | llvm-cov 输出到 `coverage/` 目录不存在时可能失败 | 低 | test-runner 在执行前确保 coverage/ 目录存在；解析失败时返回 null 不阻塞 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 是否保留 unit-test-executor agent？ | 保留，仅调整数据源 | Agent 的高层编排和诊断决策仍有价值；确定性执行下沉后 Agent 更专注 | 完全删除 agent，全部交给 CLI。否决理由：诊断决策需上下文感知，仍有 LLM 的价值 |
| ~~merge_mode 设计~~ | **移除 merge_mode**，统一使用链式命令 | 移除 `merge_mode` 减少了执行分支和测试矩阵复杂度。pytest 和 rust 的 `test_cmd` 使用 `; _X=$?; ...; exit $_X` 模式统一为单条命令。所有框架的 `executePlanEntry` 执行相同逻辑 | 保留 merge_mode 布尔字段。否决理由：增加了 test-runner.ts 中 `executeMerged()` 和 `executeTwoPhase()` 两个分支，增加了测试矩阵 |
| 子报告是否含覆盖率 | 是，每框架独立 | evaluator 可能按框架维度读取覆盖率做诊断 | 只在汇总报告存放覆盖率。否决理由：逐框架排错时需要 |
| 汇总报告是否含明细 | 否 | 汇总报告职责是聚合和结论；明细在子报告 | 汇总报告含全部内容。否决理由：汇总报告体积会随用例数增长，evaluator 读取耗时 |
| test_cmd 模板占位符格式 | `{files}`、`{directory}`、`{project_root}` | 与常见模板语法一致，可读性好 | `${files}` 或 `%files%`。否决理由：前者可能与 shell 变量冲突，后者可读性差 |
| 链式命令中覆盖率是否写文件 | 是，写入文件后解析 | 避免 stdout 混合，解析更可靠。pytest 使用 `--cov-report=json`，rust 使用 `--output-path` | 从 stdout 解析。否决理由：链式命令 stdout 中混合测试输出和覆盖率输出，解析复杂度高 |

### 待决问题

- 无
