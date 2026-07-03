## Module Contract

### Module: lib/test-runner.ts (Execution Orchestration — Mutation Phase)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-runner.ts` |
| **Exports** | `executePlanEntry(entry: PlanEntry, projectRoot: string, options): ExecutionResult` |
| **Mutation Phase** | 在步骤 3（覆盖率解析）之后、步骤 4（返回结果）之前插入 mutation 阶段 |
| **Mutation Trigger** | 当 `entry.mutation_framework` 非空时执行；为 null 时跳过 |
| **Mutation Steps** | a) 检测/生成 StrykerJS 配置 → b) 执行 `npx stryker run` → c) 解析 JSON 报告 → d) 清理临时配置 |
| **Mutation Timeout** | 使用 StrykerJS 的 `--timeoutMS` 和 `--maxTestRunnerReuse` 限制；总执行时长计入 `durationMs` |

### Module: lib/test-parser/stryker-config.ts (NEW — StrykerJS Configuration Generator)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` |
| **Exports** | `resolveStrykerConfig(projectRoot: string, options: StrykerConfigOptions): StrykerConfigResult` |
| **Input** | `projectRoot` — 项目根目录；`StrykerConfigOptions` — `{ framework: string, sourceFiles: string[], testFiles: string[], mutationScore?: number }` |
| **Output** | `StrykerConfigResult`: `{ configPath: string, cleanup: boolean }` — `configPath` 是最终使用的 StrykerJS 配置文件路径；`cleanup` 标记是否需要执行后清理 |
| **Strategy** | 1) 检查 `stryker.config.{json,mjs,cjs}` 是否存在；2) 存在则直接使用（`cleanup = false`）；3) 不存在则从内置模板生成临时文件（`cleanup = true`） |
| **Side Effects** | 可能创建临时文件到项目根目录 |

### Module: lib/test-parser/mutation-parser.ts (NEW — StrykerJS JSON Report Parser)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.ts` |
| **Exports** | `parseMutationReport(reportPath: string): MutationReportResult` |
| **Input** | StrykerJS JSON 报告文件路径（默认 `reports/mutation/mutation.json`） |
| **Output** | `MutationReportResult`: `{ score: number, measured: MutationMeasured, sourceFiles: string[] }` |
| **Parsing** | 读取 StrykerJS JSON 报告，提取 `mutationScore`、`killed`、`survived`、`timeout`、`noCoverage`、`compileError`、`runtimeError`、`ignored`、`totalDetected`、`totalUndetected`、`totalMutants` |
| **Side Effects** | 读取文件 |

### Module: lib/test-framework.ts (Framework Registry — Mutation Fields)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-framework.ts` |
| **NEW Fields** | `mutation_framework: string | null` — StrykerJS 支持的框架标记；`mutation_config: string | null` — 默认 StrykerJS 配置预设 |
| **Registry Values** | jest → `"stryker-js"`, vitest → `"stryker-js"`, vite-plus → `"stryker-js"`, 其余 → `null` |

### Module: commands/test-detect-frameworks.ts (PlanEntry — Mutation Fields)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` |
| **NEW Fields in PlanEntry** | `mutation_framework: string | null` — 从 registry 继承；`mutation_score: number | null` — 从 config 读取的阈值 |
| **Filling** | 在 `buildPlanFromMappings` 中从 `getFrameworkConfig(framework).mutation_framework` 和 `config.test.mutation.score` 填充 |

### Schema: schemas/config/config.schema.ts (Mutation Threshold)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` |
| **NEW Constants** | `TEST_MUTATION_SCORE_DEFAULT = 80` |
| **NEW Schemas** | `testMutationSchema`: `z.object({ score: z.number().optional().prefault(80).describe("变异得分阈值（百分比）") })` |
| **Modified** | `test` object 添加 `mutation: testMutationSchema.optional()`；`overrides` item 添加 `mutation: testMutationSchema.optional()` |

### Schema: schemas/unit-test-output.schema.ts (Mutation Report)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` |
| **NEW Schemas** | `mutationMeasuredSchema`, `mutationOverrideSchema`, `mutationBlockSchema` |
| **Modified** | `UnitTestSubReport` 添加 `mutation: MutationBlock | null`；`UnitTestSummaryReport` 添加 `mutation: MutationBlock | null` |

### Schema: schemas/test-detect-frameworks.schema.ts (PlanEntry Mutation Fields)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` |
| **NEW Fields** | `plan[].mutation_framework: z.string().nullable()`；`plan[].mutation_score: z.number().nullable()` |

### Module: lib/test-report.ts (Report Generation — Mutation Block)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-report.ts` |
| **Sub-report** | `generateSubReport` 读取 `result.mutation`，生成 `mutation` 块（含 pass/score/threshold/measured/by_framework/overrides） |
| **Summary** | `generateSummaryReport` 聚合各子报告的 mutation 数据，计算全局 `mutation.pass` |
| **Pass Logic** | `measured.score >= thresholds.score` → pass；overrides 中所有分组的 score >= 其阈值 → 额外 pass 条件 |
| **Conclusion** | mutation 不达标（`mutation.pass === false`）时，添加 `mutation_failure` 类型 problem |

### Module: cli.ts (CLI Entry — --no-mutation Flag)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/cli.ts` |
| **NEW Option** | `--no-mutation` — 跳过 mutation 执行阶段 |
| **Pass-through** | 选项值传递到 `runUnitTest({ ...noMutation })`，再传递到 `executePlanEntry({ ...skipMutation })` |

---

## ADDED Requirements

### Requirement: StrykerJS 配置生成策略（模板 + 覆盖）

**ID**: REQ-MT-1
**Priority**: MUST
**Description**: `lib/test-parser/stryker-config.ts` 的 `resolveStrykerConfig` SHALL 实现"模板 + 覆盖"配置策略：
1. 检测项目根目录是否存在 `stryker.config.json`、`stryker.config.mjs` 或 `stryker.config.cjs`
2. 存在时 SHALL 直接使用用户自定义配置，返回 `{ configPath: "<检测到的文件路径>", cleanup: false }`
3. 不存在时 SHALL 从内置模板生成临时 `stryker.config.json` 文件到项目根目录，返回 `{ configPath: "<临时文件路径>", cleanup: true }`
4. 内置模板 SHALL 包含以下配置项：`mutate`（源文件 glob）、`testRunner`（映射到 jest/vitest）、`reporters`（`["json"]`）、`jsonFilename`（`reports/mutation/mutation.json`）、`thresholds`（`{ high: score, low: score - 10, break: score }`）、`timeoutMS`、`maxTestRunnerReuse`

#### Scenario: 用户自定义配置优先

**WHEN** 项目根目录存在 `stryker.config.json`
**THEN** `resolveStrykerConfig` SHALL 返回该文件路径
**AND** `cleanup` SHALL 为 `false`
**AND** SHALL NOT 生成临时配置文件

#### Scenario: 无自定义配置时生成临时配置

**WHEN** 项目根目录不存在任何 `stryker.config.*` 文件
**AND** `options.framework` 为 `"vitest"`
**AND** `options.sourceFiles` 为 `["src/index.ts", "src/utils.ts"]`
**THEN** `resolveStrykerConfig` SHALL 在项目根目录创建临时 `stryker.config.json`
**AND** 临时配置的 `mutate` SHALL 包含 `["src/index.ts", "src/utils.ts"]`
**AND** `testRunner` SHALL 为 `"vitest"`
**AND** `reporters` SHALL 包含 `"json"`
**AND** `jsonFilename` SHALL 为 `"reports/mutation/mutation.json"`
**AND** `thresholds.high` SHALL 等于 `options.mutationScore`
**AND** `cleanup` SHALL 为 `true`

#### Scenario: 临时配置执行后清理

**WHEN** StrykerJS 执行完成
**AND** `cleanup === true`
**THEN** `executePlanEntry` SHALL 删除临时生成的 `stryker.config.json`
**AND** SHALL 删除 `reports/mutation/` 目录
**AND** SHALL NOT 删除 `reports/mutation/mutation.json`（已读取到内存）

### Requirement: 变异测试执行阶段

**ID**: REQ-MT-2
**Priority**: MUST
**Description**: `lib/test-runner.ts` 的 `executePlanEntry` SHALL 在覆盖率解析完成后、返回 `ExecutionResult` 之前，插入 mutation 执行阶段。变异测试 SHALL 在以下条件全部满足时执行：
1. `entry.mutation_framework` 非 null
2. `options.skipMutation` 不为 `true`
3. `testFiles` 数组非空

执行流程：
1. 调用 `resolveStrykerConfig(projectRoot, { framework, sourceFiles, testFiles, mutationScore })`
2. 执行 `execSync("npx stryker run --config <configPath>", { cwd: projectRoot })`
3. 调用 `parseMutationReport("reports/mutation/mutation.json")`
4. 如果 `cleanup === true`，清理临时配置文件和 `reports/mutation/` 目录
5. 将解析结果存入 `ExecutionResult.mutation`

#### Scenario: 支持的框架执行 StrykerJS

**WHEN** `entry.framework` 为 `"vitest"`
**AND** `entry.mutation_framework` 为 `"stryker-js"`
**AND** `testFiles` 包含至少一个文件
**AND** `options.skipMutation` 未设置
**THEN** `executePlanEntry` SHALL 调用 `resolveStrykerConfig`
**AND** SHALL 执行 `npx stryker run`
**AND** SHALL 调用 `parseMutationReport`
**AND** 返回的 `ExecutionResult.mutation` SHALL NOT 为 null

#### Scenario: 不支持的框架跳过 mutation

**WHEN** `entry.framework` 为 `"go"`
**AND** `entry.mutation_framework` 为 `null`
**THEN** `executePlanEntry` SHALL NOT 执行 StrykerJS
**AND** 返回的 `ExecutionResult.mutation` SHALL 为 `null`

#### Scenario: --no-mutation 跳过执行

**WHEN** `options.skipMutation` 为 `true`
**AND** `entry.mutation_framework` 为 `"stryker-js"`
**THEN** `executePlanEntry` SHALL NOT 执行 StrykerJS
**AND** SHALL NOT 调用 `resolveStrykerConfig`
**AND** 返回的 `ExecutionResult.mutation` SHALL 为 `null`

#### Scenario: 测试文件为空时跳过 mutation

**WHEN** `testFiles` 数组为空或 `testFiles.length === 0`
**AND** `entry.mutation_framework` 为 `"stryker-js"`
**THEN** `executePlanEntry` SHALL NOT 执行 StrykerJS
**AND** 返回的 `ExecutionResult.mutation` SHALL 为 `null`

#### Scenario: StrykerJS 执行失败不阻塞整体流程

**WHEN** `npx stryker run` 执行失败（退出码非零）
**THEN** `executePlanEntry` SHALL 捕获异常
**AND** `ExecutionResult.mutation` SHALL 包含 `error` 字段描述失败原因
**AND** `ExecutionResult.exitCode` SHALL NOT 受此影响（退出码仍由测试命令决定）

### Requirement: StrykerJS JSON 报告解析

**ID**: REQ-MT-3
**Priority**: MUST
**Description**: `lib/test-parser/mutation-parser.ts` 的 `parseMutationReport` SHALL 解析 StrykerJS JSON 报告文件，提取变异得分和各类变异体计数。

提取字段映射：

| StrykerJS 字段 | 输出字段 | 说明 |
|----------------|---------|------|
| `mutationScore` | `score` | 变异得分百分比 |
| `killed` | `measured.killed` | 被测试杀死的变异体数 |
| `survived` | `measured.survived` | 未被测试杀死的变异体数 |
| `timeout` | `measured.timeout` | 超时的变异体数 |
| `noCoverage` | `measured.noCoverage` | 未被覆盖的变异体数 |
| `compileError` | `measured.compileError` | 编译错误的变异体数 |
| `runtimeError` | `measured.runtimeError` | 运行时错误的变异体数 |
| `ignored` | `measured.ignored` | 被忽略的变异体数 |
| `totalMutants` | `measured.total` | 变异体总数 |
| `totalDetected` | `measured.detected` | 被检测到的变异体数 |
| `totalUndetected` | `measured.undetected` | 未被检测到的变异体数 |

#### Scenario: 正常解析完整报告

**WHEN** StrykerJS JSON 报告文件包含所有标准字段
**AND** 内容为 `{"mutationScore": 72.5, "killed": 116, "survived": 18, "timeout": 3, "noCoverage": 7, "compileError": 0, "runtimeError": 2, "ignored": 5, "totalMutants": 151, "totalDetected": 119, "totalUndetected": 25}`
**THEN** 返回 `{ score: 72.5, measured: { killed: 116, survived: 18, timeout: 3, noCoverage: 7, compileError: 0, runtimeError: 2, ignored: 5, total: 151, detected: 119, undetected: 25 } }`
**AND** `sourceFiles` 从报告中的 `files` 字段提取

#### Scenario: 文件不存在时返回错误

**WHEN** 指定的报告文件路径不存在
**THEN** `parseMutationReport` SHALL 抛出错误或返回含 `error` 的结果
**AND** 调用方 SHALL 在 `ExecutionResult.mutation.error` 中记录该错误

#### Scenario: 无效 JSON 内容

**WHEN** 报告文件存在但内容不是合法 JSON
**THEN** `parseMutationReport` SHALL 抛出解析错误
**AND** 不崩溃

### Requirement: MutationBlock 和 MutationMeasured Schema 定义

**ID**: REQ-MT-4
**Priority**: MUST
**Description**: `schemas/unit-test-output.schema.ts` SHALL 使用 Zod v4 定义以下新增 schema 类型：

**MutationMeasured**:
- `killed` — `z.number().int().min(0)` — 被杀死的变异体数
- `survived` — `z.number().int().min(0)` — 存活的变异体数
- `timeout` — `z.number().int().min(0)` — 超时的变异体数
- `noCoverage` — `z.number().int().min(0)` — 未覆盖的变异体数
- `compileError` — `z.number().int().min(0)` — 编译错误的变异体数
- `runtimeError` — `z.number().int().min(0)` — 运行时错误的变异体数
- `ignored` — `z.number().int().min(0)` — 被忽略的变异体数
- `total` — `z.number().int().min(0)` — 变异体总数
- `detected` — `z.number().int().min(0)` — 被检测到的变异体数
- `undetected` — `z.number().int().min(0)` — 未被检测到的变异体数

**MutationBlock**:
- `pass` — `z.boolean()` — 是否达到阈值
- `score` — `z.number().min(0).max(100)` — 变异得分百分比
- `threshold` — `z.number().min(0).max(100)` — 得分阈值
- `measured` — `MutationMeasured` — 各类变异体计数
- `by_framework` — `z.record(z.string(), z.object({ measured: MutationMeasured, source_files: z.array(z.string()) }))` — 按框架的详细结果
- `overrides` — `z.array(MutationOverride).optional()` — 按 overrides 配置的逐个分组结果
- `error` — `z.string().optional()` — 执行失败时的错误信息

**MutationOverride**:
- `glob` — `z.string()` — 文件 glob 模式
- `threshold` — `z.number().min(0).max(100)` — 该分组的得分阈值
- `score` — `z.number().min(0).max(100)` — 该分组的实际得分
- `pass` — `z.boolean()` — 是否达标
- `file_count` — `z.number().int().min(0)` — 匹配的文件数

#### Scenario: 有效 MutationBlock 通过 schema 验证

**WHEN** 一个包含所有必需字段的有效 MutationBlock 对象通过 schema
**THEN** 验证 SHALL 成功
**AND** 返回解码后的类型安全对象

#### Scenario: MutationBlock 的 score 超范围被拒绝

**WHEN** `score` 为 `105`（超过 100）
**THEN** 验证 SHALL 失败
**AND** 错误消息 SHALL 指明范围限制

### Requirement: 子报告生成中包含 mutation 块

**ID**: REQ-MT-5
**Priority**: MUST
**Description**: `lib/test-report.ts` 的 `generateSubReport` SHALL 在 `ExecutionResult` 包含 `mutation` 字段时，生成 mutation 块写入子报告。mutation 块 SHALL 包含：
- `pass` — `score >= threshold` 的计算结果
- `score` — 从 parser 解析的变异得分
- `threshold` — 从 config 读取的变异得分阈值
- `measured` — 各类变异体计数
- `by_framework` — 当前框架的 mutation 结果
- `overrides` — 按 overrides 配置的逐个分组结果（仅在 mutation overrides 存在时）
- `error` — 仅当 mutation 执行失败时存在

#### Scenario: vitest 子报告包含 mutation 块

**WHEN** `ExecutionResult.mutation` 包含 `{ score: 85, measured: { killed: 20, survived: 3, total: 25, ... } }`
**AND** config 中 `test.mutation.score` 为 `80`
**THEN** 子报告 `mutation` 块的 `pass` SHALL 为 `true`（85 >= 80）
**AND** `score` SHALL 为 `85`
**AND** `threshold` SHALL 为 `80`
**AND** `by_framework` SHALL 包含 `{ vitest: { measured: { killed: 20, survived: 3, total: 25, ... }, source_files: [...] } }`

#### Scenario: mutation 不达标时 pass 为 false

**WHEN** `ExecutionResult.mutation.score` 为 `65`
**AND** config 中 `test.mutation.score` 为 `80`
**THEN** 子报告 `mutation.pass` SHALL 为 `false`

#### Scenario: mutation 执行失败时 mutation 块含 error

**WHEN** `ExecutionResult.mutation` 包含 `{ error: "StrykerJS not installed" }`
**AND** `score` 为 `null`
**THEN** 子报告 `mutation` 块的 `pass` SHALL 为 `false`
**AND** `error` SHALL 为 `"StrykerJS not installed"`
**AND** `measured` SHALL 为所有计数为 0 的默认值

#### Scenario: mutation 框架跳过时 mutation 为 null

**WHEN** `entry.mutation_framework` 为 `null`（不支持的框架）
**THEN** `ExecutionResult.mutation` SHALL 为 `null`
**AND** 子报告 `mutation` 字段 SHALL 为 `null`

### Requirement: 汇总报告生成中包含 mutation 块

**ID**: REQ-MT-6
**Priority**: MUST
**Description**: `lib/test-report.ts` 的 `generateSummaryReport` SHALL 聚合所有子报告的 mutation 数据，生成汇总报告中的 mutation 块。聚合规则：
1. 如果所有子报告的 mutation 均为 null，汇总报告 mutation 为 null
2. 如果至少一个子报告包含 mutation，聚合 score 按 source_files 加权平均
3. 汇总 `measured` 按字段求和（各框架独立运行，变异体计数不重叠）
4. `by_framework` 包含所有执行了 mutation 的框架数据
5. `overrides` 继承自子报告，按 glob 去重合并
6. 如果 mutation 不达标（`mutation.pass === false`），添加 `mutation_failure` 类型 problem

#### Scenario: 单框架 mutation 汇总

**WHEN** 只有一个框架（vitest）执行了 mutation，score 85，阈值 80
**THEN** 汇总报告 `mutation.pass` SHALL 为 `true`
**AND** `mutation.score` SHALL 为 `85`
**AND** `mutation.threshold` SHALL 为 `80`
**AND** `mutation.by_framework` SHALL 包含 vitest 的详细数据

#### Scenario: 多框架 mutation 汇总

**WHEN** vitest score=85（5 个源文件）、jest score=72（3 个源文件）
**THEN** 汇总 score SHALL 为 `(85*5 + 72*3) / (5+3)` = `80.125`
**AND** 汇总 measured 的 killed SHALL 为 vitest.killed + jest.killed
**AND** `by_framework` SHALL 包含两个框架

#### Scenario: mutation 不达标时添加 problem

**WHEN** 汇总 `mutation.pass` 为 `false`
**AND** `mutation.score` 为 `72.5`
**AND** `mutation.threshold` 为 `80`
**THEN** `problems` 数组 SHALL 包含类型为 `mutation_failure` 的条目
**AND** `message` SHALL 包含 `"Mutation score 72.5% < 80%"`

#### Scenario: 所有框架均跳过 mutation 时 mutation 为 null

**WHEN** 所有子报告的 `mutation` 均为 `null`
**THEN** 汇总报告 `mutation` SHALL 为 `null`
**AND** SHALL NOT 添加 `mutation_failure` 类型 problem

### Requirement: --no-mutation CLI 选项

**ID**: REQ-MT-7
**Priority**: MUST
**Description**: `cli.ts` 的 `unit-test` 子命令 SHALL 支持 `--no-mutation` 选项。当此选项被设置时，`runUnitTest` SHALL 传递 `skipMutation: true` 到 `executePlanEntry`，跳过所有 mutation 执行。

#### Scenario: --no-mutation 选项存在

**WHEN** `dev-team unit-test --no-mutation` 被调用
**THEN** CLI SHALL 解析该选项
**AND** `runUnitTest` 的 `options.noMutation` SHALL 为 `true`
**AND** 所有框架的 mutation 阶段 SHALL 被跳过

#### Scenario: --no-mutation 不影响其他功能

**WHEN** `dev-team unit-test --no-mutation --change my-change` 被调用
**THEN** 测试执行 SHALL 正常进行
**AND** 覆盖率收集 SHALL 正常进行
**AND** 报告生成 SHALL 正常进行
**AND** 仅 mutation 阶段被跳过
**AND** 报告中 `mutation` 字段 SHALL 为 `null`

### Requirement: config.schema.ts 的 mutation 阈值配置

**ID**: REQ-MT-8
**Priority**: MUST
**Description**: `schemas/config/config.schema.ts` SHALL 定义 `testMutationSchema`，包含可选的 `score` 数字字段，默认值为 `80`。SHALL 在 `test` 对象中添加 `mutation` 字段（可选）。SHALL 在 `overrides` 数组的 item 中添加 `mutation` 字段（可选）。

#### Scenario: 全局 mutation 阈值默认值

**WHEN** `config.test.mutation` 未在 config.json 中设置
**THEN** `config.test.mutation.score` 的默认值 SHALL 为 `80`

#### Scenario: override 中的 mutation 阈值

**WHEN** `config.test.overrides` 包含 `{ "file": "src/core/**", "mutation": { "score": 90 } }`
**THEN** override 的 mutation score 阈值 SHALL 为 `90`
**AND** 该 override 的 `coverage` 阈值与 `mutation` 阈值独立配置

#### Scenario: override 未设置 mutation 时继承全局

**WHEN** `config.test.overrides` 包含 `{ "file": "src/core/**", "coverage": { "lines": 85 } }`
**AND** 该 override 未包含 `mutation` 字段
**THEN** 该 override 的 mutation 阈值 SHALL 继承全局 `test.mutation.score`
**AND** 仍按 overrides 的 glob 分组计算该分组的实际 mutation score

### Requirement: PlanEntry 传递 mutation 字段

**ID**: REQ-MT-9
**Priority**: MUST
**Description**: `PlanEntry` 接口和 schema SHALL 新增 `mutation_framework: string | null` 和 `mutation_score: number | null` 字段。`buildPlanFromMappings` 在构建 PlanEntry 时 SHALL 从 `getFrameworkConfig(framework).mutation_framework` 获取 mutation_framework，从 `config.test.mutation.score` 获取 mutation_score。

#### Scenario: 支持框架的 PlanEntry 含 mutation_framework

**WHEN** `buildPlanFromMappings` 处理 vitest 框架映射
**THEN** 生成的 `PlanEntry` 的 `mutation_framework` SHALL 为 `"stryker-js"`
**AND** `mutation_score` SHALL 为从 config 读取的数值

#### Scenario: 不支持框架的 PlanEntry 的 mutation_framework 为 null

**WHEN** `buildPlanFromMappings` 处理 go 框架映射
**THEN** 生成的 `PlanEntry` 的 `mutation_framework` SHALL 为 `null`

### Requirement: FrameworkConfig 添加 mutation_framework 字段

**ID**: REQ-MT-10
**Priority**: MUST
**Description**: `FrameworkConfig` 接口 SHALL 新增 `mutation_framework: string | null` 字段。FRAMEWORK_REGISTRY 中：
- jest、vitest、vite-plus 的 `mutation_framework` 设为 `"stryker-js"`
- bun、rust、node-test、go、pytest 的 `mutation_framework` 设为 `null`

#### Scenario: jest/vitest/vite-plus 支持 mutation

**WHEN** `getFrameworkConfig("jest")` 被调用
**THEN** 返回的 `FrameworkConfig.mutation_framework` SHALL 为 `"stryker-js"`

**WHEN** `getFrameworkConfig("vitest")` 被调用
**THEN** 返回的 `FrameworkConfig.mutation_framework` SHALL 为 `"stryker-js"`

**WHEN** `getFrameworkConfig("vite-plus")` 被调用
**THEN** 返回的 `FrameworkConfig.mutation_framework` SHALL 为 `"stryker-js"`

#### Scenario: 不支持框架的 mutation_framework 为 null

**WHEN** `getFrameworkConfig("go")` 被调用
**THEN** 返回的 `FrameworkConfig.mutation_framework` SHALL 为 `null`

**WHEN** `getFrameworkConfig("pytest")` 被调用
**THEN** 返回的 `FrameworkConfig.mutation_framework` SHALL 为 `null`

**WHEN** `getFrameworkConfig("rust")` 被调用
**THEN** 返回的 `FrameworkConfig.mutation_framework` SHALL 为 `null`

**WHEN** `getFrameworkConfig("node-test")` 被调用
**THEN** 返回的 `FrameworkConfig.mutation_framework` SHALL 为 `null`

**WHEN** `getFrameworkConfig("bun")` 被调用
**THEN** 返回的 `FrameworkConfig.mutation_framework` SHALL 为 `null`

### Requirement: mutation overrides 计算

**ID**: REQ-MT-11
**Priority**: MUST
**Description**: `lib/test-report.ts` 的 `computeOverrides` 函数 SHALL 扩展支持 `mutation` 字段。对于每个 override 配置，如果包含 `mutation.score`，SHALL 计算该 glob 分组的实际 mutation score 并生成 `MutationOverride` 条目。mutation score 的计算基于该 glob 匹配的源文件在 StrykerJS 报告中的 `files` 字段。

由于 StrykerJS 报告按文件提供变异体数据，计算方式为：
1. 根据 override 的 `file` glob 匹配源文件
2. 从 StrykerJS 报告的 `files` 字段中提取匹配文件的变异体数据
3. 计算匹配文件的加权平均 mutation score
4. 对比阈值生成 `pass` 结果

#### Scenario: override 匹配部分源文件

**WHEN** override 配置 `{ "file": "src/core/**", "mutation": { "score": 90 } }`
**AND** StrykerJS 报告中 `src/core/index.ts` 的变异体统计为 `{ killed: 10, survived: 1, total: 12 }`（score=83.3）
**AND** 该 glob 匹配了 2 个源文件
**THEN** 该 override 的 `score` SHALL 为匹配文件的 weighted average
**AND** `pass` SHALL 根据实际 score 是否 >= 90 判断
**AND** `file_count` SHALL 为匹配的源文件数

#### Scenario: override 未设置 mutation 时不生成 mutation override

**WHEN** override 配置仅包含 `coverage` 字段，无 `mutation` 字段
**THEN** `computeOverrides` SHALL NOT 为该 override 生成 mutation override 条目
**AND** coverage override 计算不受影响
