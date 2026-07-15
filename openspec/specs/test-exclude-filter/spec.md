## Purpose

This capability defines a shared utility module `lib/test-exclude.ts` that provides functions to determine whether a source file should be excluded from the testing pipeline based on `test.exclude` and `test.overrides[].exclude` globs in `openspec/config.json`.

The module is consumed by:
- `commands/test-detect-frameworks.ts` — filters out excluded files from framework detection results
- `commands/test-resolve-paths.ts` — filters out excluded files from unit test path derivation
- `lib/test-runner.ts` — filters out excluded source files before running StrykerJS mutation testing

---

## Requirements

### Requirement: isFileExcluded checks file against all exclude globs

**ID**: REQ-EXC-1
**Priority**: MUST
**Description**: A function `isFileExcluded(filePath: string, config: OpenSpecConfig): boolean` SHALL be exported from `lib/test-exclude.ts`. It SHALL collect all exclude globs via `getExcludeGlobs(config)` and return `true` if the normalized file path matches any of them using the existing `matchGlob` function from `lib/glob.ts`. The file path SHALL be normalized to POSIX forward-slash format before matching.

#### Scenario: file matches global test.exclude glob

**WHEN** `config.test.exclude` is set to `["**/generated/**"]`
**AND** `isFileExcluded("src/generated/api.ts", config)` is called
**THEN** it SHALL return `true`

#### Scenario: file matches override-level exclude glob

**WHEN** `config.test.overrides` contains `{ file: "plugins/dev-team/bin", exclude: ["**/vendor/**"] }`
**AND** `isFileExcluded("plugins/dev-team/bin/vendor/lib.ts", config)` is called
**THEN** it SHALL return `true`

#### Scenario: file matches neither global nor override exclude

**WHEN** `config.test.exclude` is set to `["**/generated/**"]`
**AND** `config.test.overrides[0].exclude` is set to `["**/vendor/**"]`
**AND** `isFileExcluded("src/app.ts", config)` is called
**THEN** it SHALL return `false`

#### Scenario: no exclude configured returns false

**WHEN** `config.test` has no `exclude` field and no `overrides`
**AND** `isFileExcluded("src/app.ts", config)` is called
**THEN** it SHALL return `false`

#### Scenario: file path is normalized to POSIX before matching

**WHEN** `isFileExcluded` is called with a Windows backslash path `"src\\generated\\api.ts"`
**AND** `config.test.exclude` is `["**/generated/**"]`
**THEN** it SHALL normalize to `"src/generated/api.ts"` and return `true`

### Requirement: getExcludeGlobs merges global and override excludes

**ID**: REQ-EXC-2
**Priority**: MUST
**Description**: A function `getExcludeGlobs(config: OpenSpecConfig): string[]` SHALL be exported from `lib/test-exclude.ts`. It SHALL return the union of all exclude globs from `test.exclude` and `test.overrides[].exclude`. The array SHALL be deduplicated. When no excludes are configured, it SHALL return an empty array.

#### Scenario: returns merged global + override excludes

**WHEN** `config` has `test.exclude: ["**/generated/**"]` and `test.overrides[0].exclude: ["**/vendor/**"]`
**THEN** `getExcludeGlobs(config)` SHALL return an array containing both `"**/generated/**"` and `"**/vendor/**"`

#### Scenario: deduplicates identical globs

**WHEN** `config` has `test.exclude: ["**/generated/**"]` and `test.overrides[0].exclude: ["**/generated/**"]`
**THEN** `getExcludeGlobs(config)` SHALL return `["**/generated/**"]` (one entry only)

#### Scenario: no exclude configured returns empty array

**WHEN** `config.test` has no `exclude` field and no `overrides`
**THEN** `getExcludeGlobs(config)` SHALL return `[]`

### Requirement: test-runner excludes filtered files from mutation phase

**ID**: REQ-EXC-3
**Priority**: MUST
**Description**: The `executePlanEntry` function in `lib/test-runner.ts` SHALL use `isFileExcluded` to filter excluded source files out of the mutation scope. Before passing source files to `runMutationPhase`, the module SHALL read the project config via `readConfig`, and filter the `sourceFiles` array to exclude any file for which `isFileExcluded` returns `true`.

#### Scenario: excluded source files filtered from StrykerJS mutation

**WHEN** a project has `test.exclude: ["**/generated/**"]`
**AND** the test execution produces `sourceFiles: ["src/app.ts", "src/generated/api.ts"]`
**AND** `executePlanEntry` is called for this project
**THEN** the `sourceFiles` passed to `resolveStrykerConfig` SHALL be `["src/app.ts"]`
**AND** `"src/generated/api.ts"` SHALL NOT appear in the StrykerJS configuration

#### Scenario: no exclude configured runs mutation on all source files

**WHEN** a project has no `test.exclude` configuration
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
| **Input** | `filePath` — source file path, may be POSIX or Windows format; `config` — parsed `OpenSpecConfig` from `readConfig()` |
| **Output** | `boolean` — `true` when the file matches any exclude glob |
| **Side Effects** | None — pure function, deterministic |
| **Dependencies** | `matchGlob` from `lib/glob.ts`; `toForwardSlash` from `lib/glob.ts` |

### Function: getExcludeGlobs

| Property | Description |
|----------|-------------|
| **Module** | `lib/test-exclude.ts` |
| **Signature** | `getExcludeGlobs(config: OpenSpecConfig): string[]` |
| **Input** | `config` — parsed `OpenSpecConfig` |
| **Output** | `string[]` — deduplicated array of glob patterns |
| **Side Effects** | None — pure function, deterministic |
| **Behavior** | Collects `test.exclude` globs, then iterates `test.overrides` and collects each entry's `exclude` globs; deduplicates via `Set` |

### Consumer: lib/test-runner.ts mutation phase

| Property | Description |
|----------|-------------|
| **Module** | `lib/test-runner.ts` |
| **Change point** | After `deriveSourceFiles` produces the `sourceFiles` array, before `restrictMutationScope` |
| **New logic** | Filter `sourceFiles` through `isFileExcluded(file, config)`; files returning `true` are excluded from mutation |
| **Config read** | At the top of `executePlanEntry` or `runMutationPhase`, the module SHALL call `readConfig(projectRoot)` once |
| **Backward compat** | When config has no `test.exclude`, filtering is a no-op — all source files pass through |
