## MODIFIED Requirements

### Requirement: 变异测试执行阶段

**ID**: REQ-MT-2
**Priority**: MUST
**Description**: `lib/test-runner.ts` 的 `executePlanEntry` SHALL 在覆盖率解析完成后、返回 `ExecutionResult` 之前，插入 mutation 执行阶段。变异测试 SHALL 在以下条件全部满足时执行：
1. `entry.mutation_framework` 非 null
2. `options.skipMutation` 不为 `true`
3. `testFiles` 数组非空

执行流程：
1. 调用 `resolveStrykerConfig`（工作目录仍为 absCwd；可将 `jsonReporter.fileName` overlay 到当前 plan 的 `reportDir/mutation.json`）
2. 执行 `npx stryker run --config <configPath>`（cwd = absCwd）
3. 调用 `parseMutationReport("<reportDir>/mutation.json")`（路径为当前 plan 产物目录）
4. 如果需要清理，best-effort 删除临时配置文件；MUST NOT 依赖或清理 suite cwd 旧 `reports/mutation/` 作为权威产物
5. 将解析结果存入 `ExecutionResult.mutation`

**Changes from previous version**:
- 权威 mutation 报告路径：`reports/mutation/mutation.json`（相对 absCwd）→ `<reportDir>/mutation.json`
- 解析 MUST NOT 回退读取旧路径

#### Scenario: 支持的框架执行 StrykerJS

**WHEN** `entry.framework` 为 `"vitest"`
**AND** `entry.mutation_framework` 为 `"stryker-js"`
**AND** `testFiles` 包含至少一个文件
**AND** `options.skipMutation` 未设置
**THEN** `executePlanEntry` SHALL 调用 `resolveStrykerConfig`
**AND** SHALL 执行 `npx stryker run`
**AND** SHALL 调用 `parseMutationReport` 并传入 plan 目录下的 `mutation.json`
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

#### Scenario: mutation report written under plan reportDir

**WHEN** StrykerJS 成功完成
**THEN** mutation JSON SHALL 存在于当前 plan 的 `reportDir/mutation.json`
**AND** `parseMutationReport` SHALL 读取该路径
**AND** SHALL NOT 以 absCwd 下 `reports/mutation/mutation.json` 作为权威来源

### Requirement: StrykerJS JSON 报告解析

**ID**: REQ-MT-3
**Priority**: MUST
**Description**: `lib/test-parser/mutation-parser.ts` 的 `parseMutationReport` SHALL 解析 StrykerJS JSON 报告文件，提取变异得分和各类变异体计数。调用方 SHALL 传入当前 plan 的 `reportDir/mutation.json`（或等价绝对路径）。

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
**AND** 调用方 SHALL NOT 自动回退到旧 `reports/mutation/mutation.json`

#### Scenario: 无效 JSON 内容

**WHEN** 报告文件存在但内容不是合法 JSON
**THEN** `parseMutationReport` SHALL 抛出解析错误
**AND** 不崩溃

### Requirement: Stryker 工作目录与产物落在 absCwd

**ID**: REQ-MT-CWD-1
**Priority**: MUST
**Description**: 变异阶段执行时，Stryker 的工作根（`rootPath` / `cwd`）SHALL 为当前 plan entry 的 absCwd（即 `projectRoot / plan.directory`）。临时配置（`stryker.config.*`）、`.stryker-tmp/` SHALL 默认落在该 absCwd 下并在用后删除（与 C4 一致）。

权威 mutation JSON 报告 SHALL 写入当前 plan 的 `reportDir/mutation.json`（通过配置 overlay `jsonReporter.fileName` 或等价），MUST NOT 以 absCwd 下 `reports/mutation/` 作为解析权威源。

传入 `resolveStrykerConfig` 的 `mutate` / sourceFiles 路径 SHALL 重写为相对 absCwd 的 POSIX 路径。

#### Scenario: mutation 在 suite absCwd 下执行

**WHEN** plan entry `directory` 为 `"plugins/dev-team/bin"`
**AND** mutation 阶段启动
**THEN** Stryker 命令的 cwd / rootPath SHALL 解析为 `projectRoot/plugins/dev-team/bin`
**AND** 临时 `stryker.config.*` SHALL 创建于该目录下（当需要生成临时配置时）

#### Scenario: mutate 路径相对 absCwd

**WHEN** sourceFiles 含项目相对路径 `"plugins/dev-team/bin/src/foo.ts"`
**AND** absCwd 为 `plugins/dev-team/bin`
**THEN** 写入 Stryker 配置的 mutate 条目 SHALL 为 `"src/foo.ts"`（相对 absCwd）

#### Scenario: jsonReporter targets plan reportDir

**WHEN** 生成或 overlay Stryker 配置
**AND** 当前 plan 的 `reportDir` 已决议
**THEN** 配置中的 JSON reporter 输出路径 SHALL 指向 `reportDir/mutation.json`
**AND** 解析阶段 SHALL 只读取该路径

## Module Contract

### Module: lib/test-parser/stryker-config.ts

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` |
| Change | overlay `jsonReporter.fileName` → `<reportDir>/mutation.json`；临时 config 仍在 absCwd，用后删 |

### Module: lib/test-parser/mutation-parser.ts

| Property | Description |
|----------|-------------|
| File | `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.ts` |
| Input | plan 目录下 `mutation.json` 路径（非旧 `reports/mutation/mutation.json`） |
