## Purpose

本能力提供共享工具 `lib/test-exclude.ts`，依据 `openspec/config.json` 中 **`tests[].excludes`**（相对各 suite 的 `root`）判断源文件是否应排除出测试管线。

消费者：
- `commands/test-detect-frameworks.ts`
- `commands/test-resolve-paths.ts`
- `lib/test-runner.ts`（变异阶段）

---

## Requirements

### Requirement: isFileExcluded checks file against all exclude globs

**ID**: REQ-EXC-1
**Priority**: MUST
**Description**: A function `isFileExcluded(filePath: string, config: OpenSpecConfig): boolean` SHALL be exported from `lib/test-exclude.ts`. It SHALL collect applicable exclude globs via `getExcludeGlobs(config)`（或按 suite 现场匹配）and return `true` if the normalized file path matches any suite-scoped exclude using `matchGlob` from `lib/glob.ts`. The file path SHALL be normalized to POSIX forward-slash format before matching.

Suite-scoped 语义：对每个 `tests[]` 条目，将 `excludes` 中的 glob 解释为相对该 suite `root` 的模式，并匹配 `projectRoot` 相对路径（实现可将模式拼为 `root/excludeGlob`，对 `"."` / `..` 段做规范化）。仅当文件落在该 suite 的 `root` 树下时，该 suite 的 `excludes` 才对其生效。

SHALL NOT 再读取 `config.test.exclude` 或 `config.test.overrides[].exclude`。

#### Scenario: file matches suite excludes under root

**WHEN** `config.tests` contains `{ root: "src", framework: "vitest", excludes: ["generated/**"] }`
**AND** `isFileExcluded("src/generated/api.ts", config)` is called
**THEN** it SHALL return `true`

#### Scenario: file under different root is not excluded by another suite excludes

**WHEN** `config.tests` contains `{ root: "plugins/dev-team/bin", framework: "vite-plus", excludes: ["vendor/**"] }`
**AND** `isFileExcluded("other/vendor/lib.ts", config)` is called
**THEN** it SHALL return `false`

#### Scenario: file matches neither suite exclude

**WHEN** `config.tests` contains `{ root: "src", framework: "vitest", excludes: ["generated/**"] }`
**AND** `isFileExcluded("src/app.ts", config)` is called
**THEN** it SHALL return `false`

#### Scenario: no excludes configured returns false

**WHEN** `config.tests` is `[{ root: "src", framework: "vitest" }]` with no `excludes`
**AND** `isFileExcluded("src/app.ts", config)` is called
**THEN** it SHALL return `false`

#### Scenario: file path is normalized to POSIX before matching

**WHEN** `isFileExcluded` is called with a Windows backslash path `"src\\generated\\api.ts"`
**AND** the suite is `{ root: "src", excludes: ["generated/**"] }`
**THEN** it SHALL normalize to `"src/generated/api.ts"` and return `true`

### Requirement: getExcludeGlobs merges global and override excludes

**ID**: REQ-EXC-2
**Priority**: MUST
**Description**: A function `getExcludeGlobs(config: OpenSpecConfig): string[]` SHALL be exported from `lib/test-exclude.ts`. It SHALL return the union of all suite-scoped exclude globs derived from `tests[].excludes`（已拼成相对 projectRoot 的匹配模式）。The array SHALL be deduplicated. When no excludes are configured, it SHALL return an empty array.

函数名可保留；语义从「全局 + overrides」改为「所有 suite 的 root-scoped excludes」。

#### Scenario: returns merged suite excludes

**WHEN** `config.tests` has two suites with `excludes: ["generated/**"]` under `root: "a"` and `excludes: ["vendor/**"]` under `root: "b"`
**THEN** `getExcludeGlobs(config)` SHALL return patterns that match both `a/generated/**` and `b/vendor/**` scopes（具体字符串形式以实现拼接为准，但匹配效果等价）

#### Scenario: deduplicates identical scoped globs

**WHEN** two suites produce the same project-relative exclude pattern
**THEN** `getExcludeGlobs(config)` SHALL return that pattern only once

#### Scenario: no exclude configured returns empty array

**WHEN** every suite omits `excludes` or `tests` is empty
**THEN** `getExcludeGlobs(config)` SHALL return `[]`

### Requirement: test-runner excludes filtered files from mutation phase

**ID**: REQ-EXC-3
**Priority**: MUST
**Description**: The `executePlanEntry` function in `lib/test-runner.ts` SHALL use `isFileExcluded` to filter excluded source files out of the mutation scope. Before passing source files to `runMutationPhase` / `resolveStrykerConfig`, the module SHALL read the project config via `readConfig`, and filter the `sourceFiles` array to exclude any file for which `isFileExcluded` returns `true`.

#### Scenario: excluded source files filtered from StrykerJS mutation

**WHEN** a project has `tests: [{ root: "src", framework: "vitest", excludes: ["generated/**"] }]`
**AND** the test execution produces `sourceFiles: ["src/app.ts", "src/generated/api.ts"]`
**AND** `executePlanEntry` is called for this project
**THEN** the `sourceFiles` passed to `resolveStrykerConfig` SHALL be `["src/app.ts"]`
**AND** `"src/generated/api.ts"` SHALL NOT appear in the StrykerJS configuration

#### Scenario: no exclude configured runs mutation on all source files

**WHEN** a project has a suite with no `excludes`
**AND** the test execution produces `sourceFiles: ["src/app.ts", "src/utils.ts"]`
**AND** `executePlanEntry` is called for this project
**THEN** both `"src/app.ts"` and `"src/utils.ts"` SHALL appear in the StrykerJS configuration

---

## Module Contract

### Function: isFileExcluded

| Property | Description |
|----------|-------------|
| **Module** | `lib/test-exclude.ts` |
| **Signature** | `isFileExcluded(filePath: string, config: OpenSpecConfig): boolean` |
| **Input** | `filePath` — source file path；`config` — parsed `OpenSpecConfig` |
| **Output** | `boolean` — `true` when the file matches any suite-scoped exclude |
| **Side Effects** | None |
| **Dependencies** | `matchGlob` / path normalization from `lib/glob.ts` |

### Function: getExcludeGlobs

| Property | Description |
|----------|-------------|
| **Module** | `lib/test-exclude.ts` |
| **Signature** | `getExcludeGlobs(config: OpenSpecConfig): string[]` |
| **Input** | `config` — parsed `OpenSpecConfig` |
| **Output** | `string[]` — deduplicated project-relative exclude patterns |
| **Behavior** | 遍历 `config.tests`，将各 suite `excludes` 相对 `root` 规范化后去重 |

### Consumer: lib/test-runner.ts mutation phase

| Property | Description |
|----------|-------------|
| **Change point** | After `deriveSourceFiles` / before `resolveStrykerConfig` |
| **Logic** | Filter via `isFileExcluded(file, config)` |
| **Backward note** | 无 `excludes` 时为 no-op |
