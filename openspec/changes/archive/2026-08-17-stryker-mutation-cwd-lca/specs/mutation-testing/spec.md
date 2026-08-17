## Module Contract

### Module: lib/test-framework.ts (MUTATION_EXECUTION)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-framework.ts` |
| **Constant** | `MUTATION_EXECUTION = 'npx --prefix "{prefix}" stryker run "{config}"'` |
| **Registry** | jest / vitest / vite-plus 的 `shell.mutation_execution` 与 `cmd.mutation_execution` 均返回该模板 |
| **禁止** | 模板 MUST NOT 包含 `npx -p`（`--prefix` ≠ `-p`） |

### Module: lib/test-runner.ts (`genStrykerCommand` / exec cwd)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-runner.ts` |
| **Command** | `genStrykerCommand` 将 `{config}` 换为 Stryker 配置路径，将 `{prefix}` 换为 `path.resolve(projectRoot, entry.cwd)` 的绝对路径 |
| **exec cwd** | `runCommand` / `execSync` 的 `cwd` 仍为 `entry.mutation_cwd`（相对路径如 `'.'` 仍原样传递） |
| **分离** | `{prefix}` 可 ≠ `entry.mutation_cwd`（抬根时 prefix 指向子目录 suite cwd，sandbox 在父目录） |

### Module: lib/test-runner.ts / stryker-config.ts (mutation_cwd)

| Aspect | Detail |
|--------|--------|
| rootPath / cwd | `entry.mutation_cwd`（detect 产出为相对 projectRoot 的 POSIX 路径；缺省为 LCA，见 test-detect-frameworks REQ-TDF-MUT-CWD-1） |
| Temp artifacts | under mutation_cwd（临时 config / `.stryker-tmp/`，用后删） |
| Mutation JSON | `<reportDir>/mutation.json`（权威产物；非 mutation_cwd `reports/mutation/`） |
| mutate paths | relative to mutation_cwd |
| npx prefix | 绝对 `path.resolve(projectRoot, entry.cwd)`；不是相对 mutation_cwd |

---

## MODIFIED Requirements

### Requirement: 变异测试执行阶段

**ID**: REQ-MT-2
**Priority**: MUST
**Description**: `lib/test-runner.ts` 的 `executePlanEntry` SHALL 在覆盖率解析完成后、返回 `ExecutionResult` 之前，插入 mutation 执行阶段。变异测试 SHALL 在以下条件全部满足时执行：
1. `entry.mutation_framework` 非 null
2. `options.skipMutation` 不为 `true`
3. `testFiles` 数组非空

执行流程：
1. 调用 `resolveStrykerConfig`（工作目录为 `entry.mutation_cwd`；可将 `jsonReporter.fileName` overlay 到当前 plan 的 `reportDir/mutation.json`）
2. 执行 `npx --prefix "{prefix}" stryker run "{config}"`（`{prefix}` 见 REQ-MT-PREFIX-1；cwd = `entry.mutation_cwd`）
3. 调用 `parseMutationReport("<reportDir>/mutation.json")`（路径为当前 plan 产物目录）
4. 如果需要清理，best-effort 删除临时配置文件；MUST NOT 依赖或清理 suite cwd 旧 `reports/mutation/` 作为权威产物
5. 将解析结果存入 `ExecutionResult.mutation`

**Changes from previous version**:
- 权威 mutation 报告路径：`reports/mutation/mutation.json` → `<reportDir>/mutation.json`
- Stryker cwd：`projectRoot` / suite `absCwd` → `entry.mutation_cwd`
- 解析 MUST NOT 回退读取旧路径
- npx 命令增加绝对 `--prefix`（suite cwd），以便 mutation_cwd 抬到父目录后仍能解析 `@stryker-mutator/*`

#### Scenario: 支持的框架执行 StrykerJS

**WHEN** `entry.framework` 为 `"vitest"`
**AND** `entry.mutation_framework` 为 `"stryker-js"`
**AND** `testFiles` 包含至少一个文件
**AND** `options.skipMutation` 未设置
**THEN** `executePlanEntry` SHALL 调用 `resolveStrykerConfig`
**AND** SHALL 执行含 `npx --prefix` 与 `stryker run` 的命令
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

**WHEN** mutation 命令（`npx --prefix … stryker run …`）执行失败（退出码非零）
**THEN** `executePlanEntry` SHALL 捕获异常
**AND** `ExecutionResult.mutation` SHALL 包含 `error` 字段描述失败原因
**AND** `ExecutionResult.exitCode` SHALL NOT 受此影响（退出码仍由测试命令决定）

#### Scenario: mutation report written under plan reportDir

**WHEN** StrykerJS 成功完成
**THEN** mutation JSON SHALL 存在于当前 plan 的 `reportDir/mutation.json`
**AND** `parseMutationReport` SHALL 读取该路径
**AND** SHALL NOT 以 mutation_cwd 下 `reports/mutation/mutation.json` 作为权威来源

### Requirement: Stryker 工作目录与产物落在 mutation_cwd

**ID**: REQ-MT-CWD-1
**Priority**: MUST
**Description**: 变异阶段执行时，Stryker 的工作根（`rootPath` / `cwd`）SHALL 为当前 plan entry 的 `mutation_cwd`（detect：`toPosixRelative(projectRoot, absMutationCwd)`，其中缺省 `absMutationCwd` 为 `LCA(absRoot, absCwd, dirname(absConfig)?)`，显式 `suite.mutation.cwd` 则 `resolve(absRoot, suite.mutation.cwd)`，见 test-detect-frameworks REQ-TDF-MUT-CWD-1）。临时配置（`stryker.config.*`）、`.stryker-tmp/` SHALL 默认落在该 mutation_cwd 下并在用后删除。

权威 mutation JSON 报告 SHALL 写入当前 plan 的 `reportDir/mutation.json`（通过配置 overlay `jsonReporter.fileName` 或等价），MUST NOT 以 mutation_cwd 下 `reports/mutation/` 作为解析权威源。

传入 `resolveStrykerConfig` 的 `mutate` / sourceFiles 路径 SHALL 重写为相对 mutation_cwd 的 POSIX 路径。

npx 解析 CLI / `@stryker-mutator/*` 的 `--prefix` SHALL 指向 suite cwd 的绝对路径（REQ-MT-PREFIX-1），MUST NOT 因此把 `execSync` cwd 改回 suite cwd。

#### Scenario: mutation 在 mutation_cwd 下执行

**WHEN** plan entry `mutation_cwd` 为 `"plugins/dev-team/bin"`
**AND** mutation 阶段启动
**THEN** Stryker 命令的 cwd / rootPath SHALL 为 `plugins/dev-team/bin`（即 `entry.mutation_cwd` 原样）
**AND** 临时 `stryker.config.*` SHALL 创建于该目录下（当需要生成临时配置时）
**AND** MUST NOT 强制改用 `projectRoot` 或 suite `cwd`（二者可与 `mutation_cwd` 不同）

#### Scenario: mutate 路径相对 mutation_cwd

**WHEN** sourceFiles 含绝对路径指向 `plugins/dev-team/bin/src/foo.ts`
**AND** mutation_cwd 为 `plugins/dev-team/bin`
**THEN** 写入 Stryker 配置的 mutate 条目 SHALL 为 `"src/foo.ts"`（相对 mutation_cwd）

#### Scenario: jsonReporter targets plan reportDir

**WHEN** 生成或 overlay Stryker 配置
**AND** 当前 plan 的 `reportDir` 已决议
**THEN** 配置中的 JSON reporter 输出路径 SHALL 指向 `reportDir/mutation.json`
**AND** 解析阶段 SHALL 只读取该路径

---

## ADDED Requirements

### Requirement: npx --prefix 使用 suite cwd 绝对路径

**ID**: REQ-MT-PREFIX-1
**Priority**: MUST
**Description**: jest / vitest / vite-plus 的 `mutation_execution` 模板 SHALL 为 `npx --prefix "{prefix}" stryker run "{config}"`（`shell` 与 `cmd` 相同）。`genStrykerCommand` SHALL 将 `{prefix}` 替换为 `path.resolve(projectRoot, entry.cwd)` 的绝对路径（Windows 下可为带盘符的绝对路径），MUST NOT 使用相对 `entry.mutation_cwd` 的相对前缀。

展开后的命令 MUST NOT 匹配 `npx` 的 `-p` / `--package` 自动安装形态（`--prefix` 不是 `-p`）。

`runCommand` / `execSync` 的 `cwd` option SHALL 仍为 `entry.mutation_cwd`。当 `mutation_cwd` 为相对路径（如 `'.'`）时，该相对值 SHALL 原样传给 stryker cwd，但 `{prefix}` 仍 MUST 为绝对路径。

抬根场景（`mutation_cwd` 为父目录、`entry.cwd` 为子目录）下，prefix SHALL 指向子目录的绝对 cwd，以便 npx 从该树解析 `@stryker-mutator/*`；本要求不覆盖 sandbox 内被测代码 `import` 的 `node_modules`（层 2，本变更不做）。

#### Scenario: 模板含 --prefix 占位且无 -p

**WHEN** `getFrameworkConfig("jest" | "vitest" | "vite-plus")` 被调用
**THEN** `shell.mutation_execution` 与 `cmd.mutation_execution` SHALL 精确等于 `npx --prefix "{prefix}" stryker run "{config}"`
**AND** 该字符串 SHALL NOT 匹配 `/(^|\s)-p(\s|$)/`

#### Scenario: prefix 为 suite cwd 绝对路径且 cwd 为 mutation_cwd

**WHEN** mutation 阶段对某 plan entry 调用 `execSync`
**AND** `entry.cwd` 为 `"pkg/jest"`、`entry.mutation_cwd` 为 `"pkg"`、`projectRoot` 为某绝对目录
**THEN** 命令 SHALL 包含 `npx --prefix` 后接 `path.resolve(projectRoot, "pkg/jest")` 的绝对路径
**AND** `execSync` 的 `cwd` option SHALL 为 `"pkg"`（即 `entry.mutation_cwd`）
**AND** 命令 SHALL NOT 包含 `npx -p`

#### Scenario: 相对 mutation_cwd 时 prefix 仍绝对

**WHEN** plan `mutation_cwd` 为 `"."`
**AND** mutation 阶段调用 `execSync`
**THEN** `execSync` 的 `cwd` option SHALL 为 `"."`
**AND** 命令中的 `--prefix` 参数 SHALL 为 `path.resolve(projectRoot, entry.cwd)` 的绝对路径
**AND** SHALL NOT 使用相对路径 `"."` 作为 prefix

#### Scenario: 抬根时 prefix 指向子目录不必真装 Stryker

**WHEN** 临时目录中存在 `pkg/` 与 `pkg/jest/`，plan `mutation_cwd` 指向 `pkg`、`entry.cwd` 指向 `pkg/jest`
**AND** `execSync` 被 mock（不启动真实 Stryker）
**THEN** 捕获到的命令 SHALL 含指向 `pkg/jest` 绝对路径的 `--prefix`
**AND** 捕获到的 `cwd` SHALL 为 `pkg`（或其相对/绝对等价形态，与 plan 写入值一致）
