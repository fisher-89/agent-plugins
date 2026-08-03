## ADDED Requirements

### Requirement: Per-file mutation score threshold for gap-fill targets

The change SHALL raise the Stryker mutation score of each target source file under `plugins/dev-team` to at least 60. Score MUST be computed as `killed / (killed + survived + noCoverage)` using the same semantics as the archived baseline report at `openspec/changes/archive/2026-07-31-test-execution-result-file/reports/test/plugins_dev-team_vite-plus/mutation.json`.

Target files MUST be exactly:

1. `bin/src/lib/test-framework.ts`
2. `bin/src/lib/test-runner.ts`
3. `bin/src/lib/test-parser/stryker-config.ts`
4. `bin/src/lib/test-report.ts`
5. `bin/src/lib/test-parser/go-parser.ts`
6. `bin/src/lib/test-parser/text-parser.ts`
7. `bin/src/commands/test-detect-frameworks.ts`

（路径相对 `plugins/dev-team`。）

Files already at or above 60 in the baseline report MUST NOT be required to improve further as part of this change.

#### Scenario: each listed file reaches score >= 60

- **WHEN** Stryker (or equivalent `dev-team test-execution` mutation phase) is re-run for `plugins/dev-team` after the gap-fill tests land
- **THEN** each of the seven target files SHALL have mutation score ≥ 60 in the resulting JSON report `files` entry
- **AND** score SHALL use `killed / (killed + survived + noCoverage)`

#### Scenario: non-target files are out of scope for the gate

- **WHEN** evaluating acceptance for this change
- **THEN** files such as `bin/src/lib/test-parser/coverage-parser.ts`、`js-parser.ts`、`mutation-parser.ts` that were already ≥ 60 in the baseline MUST NOT be treated as failing this change solely for remaining below a higher informal bar

### Requirement: Prefer colocated unit tests over production edits

Gap-fill work SHALL primarily add or strengthen colocated unit tests next to the target modules. Production source MUST NOT be changed unless a test demonstrates a real functional bug. Test-only `export` of otherwise private symbols MUST NOT be introduced.

Colocated test files SHOULD be:

| Source | Test |
|--------|------|
| `lib/test-framework.ts` | `lib/test-framework.test.ts` |
| `lib/test-runner.ts` | `lib/test-runner.test.ts` |
| `lib/test-parser/stryker-config.ts` | `lib/test-parser/stryker-config.test.ts` |
| `lib/test-report.ts` | `lib/test-report.test.ts` |
| `lib/test-parser/go-parser.ts` | `lib/test-parser/go-parser.test.ts` |
| `lib/test-parser/text-parser.ts` | `lib/test-parser/text-parser.test.ts` |
| `commands/test-detect-frameworks.ts` | `commands/test-detect-frameworks.test.ts` |

#### Scenario: new assertions live in colocated tests

- **WHEN** a Survived or NoCoverage mutant in a target file is addressed
- **THEN** the killing assertion MUST be added in the colocated `*.test.ts` (or an existing `bin/__tests__/` fixture if unit scope is insufficient)
- **AND** the production module MUST NOT gain a new `export` solely for test access

#### Scenario: production fix only on real bug

- **WHEN** a new or strengthened test fails against current production behavior and the failure indicates incorrect product behavior (not merely hard-to-kill equivalent mutant)
- **THEN** a minimal production fix MAY be applied in that target file
- **AND** the fix MUST be accompanied by a regression test that would have failed before the fix

### Requirement: Strengthen assertions against Survived and NoCoverage mutants

For each target file, added tests MUST exercise public APIs with inputs and expectations sharp enough to kill Survived mutants and cover NoCoverage regions highlighted by the baseline report. Tests SHOULD prioritize common mutator classes observed in the baseline (StringLiteral、ConditionalExpression、LogicalOperator、Regex、BlockStatement、EqualityOperator、MethodExpression).

#### Scenario: framework registry literals are asserted

- **WHEN** tests cover `getFrameworkConfig` / `detectFrameworkVersion` for known frameworks
- **THEN** they MUST assert concrete command template fragments, `coverage_format` / `coverage_output` / `mutation_framework` / `config_flag` values such that emptying or altering StringLiteral mutants is detected
- **AND** unknown framework lookup MUST still throw with an error listing supported framework names

#### Scenario: stryker path normalization edges are covered

- **WHEN** tests call `resolveStrykerConfig` with absolute paths, Windows-style drive paths, rootPath-equal paths, and relative paths under absCwd
- **THEN** the generated temp config `mutate` entries MUST be forward-slash paths relative to `rootPath`
- **AND** `jsonReporter.fileName` MUST resolve under the provided `reportDir` as `mutation.json`

#### Scenario: parser layers reject near-miss text

- **WHEN** tests feed `parseTextOutput` / `parseGoOutput` with near-miss markers, empty content, skip/fail/pass mixes, and malformed NDJSON lines
- **THEN** the parsed totals and case statuses MUST match the documented layer semantics
- **AND** Regex / LogicalOperator mutants that loosen matching MUST cause at least one assertion to fail

#### Scenario: runner and report plan-directory branches are exercised

- **WHEN** tests drive `executePlanEntry` and `generateSubReport` / `generateSummaryReport` across mutation on/off, missing artifacts, suite cwd variants, and threshold boundary scores
- **THEN** Survived ConditionalExpression / BlockStatement / LogicalOperator mutants on those branches MUST be killed by observable result fields (`mutation` block, pass flags, problems, durations)

#### Scenario: detect-frameworks plan building is tightly asserted

- **WHEN** tests call `runTestDetectFrameworks` with empty suites, explicit `files`, auto-scan, and multi-suite configs
- **THEN** returned `detected` / `plan` fields (including `mutation_framework` and `mutation_score` where applicable) MUST match expectations such that ArrayDeclaration and StringLiteral mutants in builders are killed

### Requirement: Baseline report remains the gap inventory

The archived baseline `mutation.json` SHALL remain the authoritative inventory of which files were below 60 at change start. Implementation and test-design MAY consult per-file Survived / NoCoverage samples from that report. Re-running mutation for verification MUST NOT silently drop a baseline target file from the acceptance set.

#### Scenario: acceptance set matches explore inventory

- **WHEN** listing files that must reach ≥ 60 for this change
- **THEN** the set MUST equal the seven files enumerated in Requirement "Per-file mutation score threshold for gap-fill targets"
- **AND** MUST match the explore.md inventory derived from the archived report

## Module Contract

### Module: lib/test-framework.ts

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-framework.ts` |
| **Exports** | `FrameworkConfig`, `getFrameworkConfig(framework)`, `detectFrameworkVersion(framework, cwd)` |
| **Test focus** | Registry command templates、version gating、`mutation_framework`、coverage 字段、未知框架错误文案、`detectFrameworkVersion` 失败/无 semver |
| **Baseline score** | 11.49（Survived 为主，大量 StringLiteral） |

### Module: lib/test-runner.ts

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-runner.ts` |
| **Exports** | `ExecutionResult`, `executePlanEntry(entry, projectRoot, options)` |
| **Test focus** | 测试/覆盖率/mutation 阶段分支、`noMutation`、cwd/absCwd、产物缺失、超时与清理 |
| **Baseline score** | 31.19 |

### Module: lib/test-parser/stryker-config.ts

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` |
| **Exports** | `resolveStrykerConfig(rootPath, sourceFiles, framework, reportDir)` |
| **Test focus** | 临时 config 生成、`mutate` 相对化、Windows/POSIX 路径、`reportDir`→`mutation.json`、不支持框架抛错 |
| **Baseline score** | 32.81 |

### Module: lib/test-report.ts

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-report.ts` |
| **Exports** | `generateSubReport(...)`, `generateSummaryReport(...)` |
| **Test focus** | suite cwd 解析、framework 过滤、mutation/coverage pass 边界、problems 聚合 |
| **Baseline score** | 45.06 |

### Module: lib/test-parser/go-parser.ts

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts` |
| **Exports** | `parseGoOutput(content)` |
| **Test focus** | 空内容、pass/fail/skip Action、忽略 run/output、非法 JSON 行、源文件推导 |
| **Baseline score** | 50.00 |

### Module: lib/test-parser/text-parser.ts

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts` |
| **Exports** | `parseTextOutput(output)` |
| **Test focus** | bun/cargo/node/pytest 分层、generic regex、heuristic fallback、近失配负向样例 |
| **Baseline score** | 53.19 |

### Module: commands/test-detect-frameworks.ts

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` |
| **Exports** | `TestDetectFrameworksOptions`, `runTestDetectFrameworks(options)` |
| **Test focus** | 无 suites、files 显式/自动扫描、plan 字段填充、`mutation_score` / `mutation_framework` |
| **Baseline score** | 58.85 |
