## Purpose

This capability defines the MCP tool `test_detect_frameworks` which detects test frameworks for given source files and returns a structured plan with per-directory framework configuration. Framework config data (coverage commands, output paths, globs) is sourced from the internal `lib/test-framework.ts` module.

---

## Module Contract

### Module: commands/test-detect-frameworks.ts (Modified — Import Path)

The `test-detect-frameworks.ts` command module SHALL have its internal import updated to reference the extracted `lib/test-framework.ts` instead of `commands/test-get-framework-config.ts`. Its external API (input/output schema, MCP registration, function signature) is unchanged.

| Aspect | Before | After |
|--------|--------|-------|
| Import source | `'./test-get-framework-config'` | `'../lib/test-framework'` |
| Exported symbols consumed | `runTestGetFrameworkConfig`, `getDefaultGlobForFramework` | Same symbols, same names |
| Function signature | `runTestDetectFrameworks(options)` | Unchanged |
| Module location | `commands/test-detect-frameworks.ts` | Unchanged |
| MCP registration | `registerTestDetectFrameworksTool` in `mcp.ts` | Unchanged |

### Module: lib/test-framework.ts (New Internal Module)

A new internal module SHALL contain the `FRAMEWORK_REGISTRY` constant and its associated query functions, extracted from `commands/test-get-framework-config.ts`. This module is NOT exposed as an MCP tool — it is a shared library consumed by `test-detect-frameworks.ts` and potentially other internal consumers.

| Function | Input | Output | Side Effects |
|----------|-------|--------|-------------|
| `runTestGetFrameworkConfig({framework})` | `{framework: TestFrameworks}` | `FrameworkConfig` | None — pure lookup |
| `getDefaultGlobForFramework(framework)` | `framework: string` | `string` (glob pattern) | None — pure lookup |

### MCP Tool: test_detect_frameworks (Unchanged)

The MCP tool `test_detect_frameworks` remains registered, its input/output schema is unchanged, and all existing consumers continue to work identically. The change is purely internal: the framework config lookup now comes from `lib/test-framework.ts` instead of `commands/test-get-framework-config.ts`.

| Aspect | Detail |
|--------|--------|
| Tool name | `test_detect_frameworks` |
| Input | `{files?: string[], projectRoot?: string}` |
| Output | `TestDetectFrameworksResult` (`{ detected: DetectedFile[], frameworks: string[], plan: PlanEntry[] }`) |
| Plan entry fields | `directory`, `framework`, `test_cmd`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`, `mutation_framework`, `mutation_config`, `mutation_score`, `script` |
| Registration | `registerTestDetectFrameworksTool` in `mcp.ts` — unchanged |
| Consumers | `test-gen-generator.md`, `unit-test-executor.md` — unchanged |

---

## Requirements

### Requirement: Internal import path updated to lib/test-framework

**ID**: REQ-TDF-1
**Priority**: MUST
**Description**: `commands/test-detect-frameworks.ts` SHALL import `runTestGetFrameworkConfig` and `getDefaultGlobForFramework` from `../lib/test-framework` instead of `./test-get-framework-config`. This is the only behavioral change to this module.

#### Scenario: import statement references new path

**WHEN** `commands/test-detect-frameworks.ts` is inspected
**THEN** the import line for `runTestGetFrameworkConfig` and `getDefaultGlobForFramework` SHALL read `from '../lib/test-framework'`
**AND** SHALL NOT read `from './test-get-framework-config'`

#### Scenario: unit tests pass with updated import

**WHEN** unit tests for `test-detect-frameworks.ts` are executed
**THEN** all existing test cases SHALL pass
**AND** no import-related errors SHALL be reported

### Requirement: External MCP API unchanged

**ID**: REQ-TDF-2
**Priority**: MUST
**Description**: The MCP tool `test_detect_frameworks` SHALL remain registered with the same tool name, input schema, and output schema. The MCP registration code in `mcp.ts` for this tool SHALL NOT be modified.

#### Scenario: tool still registered in mcp.ts

**WHEN** `mcp.ts` is inspected
**THEN** `registerTestDetectFrameworksTool` SHALL be called in the server initialization sequence
**AND** the tool SHALL be registered under the name `'test_detect_frameworks'`

#### Scenario: tool input/output schema unchanged

**WHEN** `test_detect_frameworks` is invoked with `{files: ["src/example.test.ts"], projectRoot: "/tmp/test-project"}`
**THEN** the result SHALL contain the `detected`, `frameworks`, and `plan` arrays
**AND** each entry in `plan` SHALL contain the fields `directory`, `framework`, `test_cmd`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`, and `script`

### Requirement: plan[] output carries all framework config fields

**ID**: REQ-TDF-3
**Priority**: MUST
**Description**: The `plan[]` array in `test_detect_frameworks` output SHALL continue to include all framework config fields (`coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`) sourced from the internal `lib/test-framework.ts` FRAMEWORK_REGISTRY. The `coverage_cmd` field has been removed — all coverage commands are now embedded within `test_cmd`.

#### Scenario: plan entry coverage fields match framework registry

**WHEN** `test_detect_frameworks` is called on a project configured with framework `"vitest"`
**THEN** `plan[0].coverage_format` SHALL be `"istanbul"`
**AND** `plan[0].coverage_output` SHALL be `"coverage/coverage-summary.json"`
**AND** `plan[0].coverage_artifacts` SHALL contain `"coverage/coverage-summary.json"`
**AND** `plan[0].coverage_cleanup` SHALL be a non-empty array

#### Scenario: node-test framework plan entry carries simplified fields

**WHEN** `test_detect_frameworks` is called on a project configured with framework `"node-test"`
**THEN** `plan[0].coverage_output` SHALL be `"coverage/node-test-output.txt"`
**AND** `plan[0].coverage_artifacts` SHALL contain `"coverage/node-test-output.txt"`

### Requirement: FRAMEWORK_REGISTRY extracted to shared lib module

**ID**: REQ-TDF-4
**Priority**: MUST
**Description**: The `FRAMEWORK_REGISTRY` constant, `runTestGetFrameworkConfig()` function, and `getDefaultGlobForFramework()` function SHALL be located in `lib/test-framework.ts` as a shared internal module. The `test-detect-frameworks.ts` SHALL be the sole consumer via internal import (not MCP). The `test-get-framework-config.ts` source file SHALL be deleted.

#### Scenario: lib/test-framework.ts exists and exports functions

**WHEN** `lib/test-framework.ts` is inspected
**THEN** it SHALL export `runTestGetFrameworkConfig`
**AND** it SHALL export `getDefaultGlobForFramework`
**AND** it SHALL contain the `FRAMEWORK_REGISTRY` constant with all 8 framework entries

#### Scenario: commands/test-get-framework-config.ts is deleted

**WHEN** the filesystem is inspected
**THEN** `commands/test-get-framework-config.ts` SHALL NOT exist

### Requirement: Excluded files filtered from framework detection

**ID**: REQ-TDF-5
**Priority**: MUST
**Description**: The `detectFrameworksForFiles` function in `commands/test-detect-frameworks.ts` SHALL filter out excluded source files before building the `detected` array. After determining the relative file path, the function SHALL call `isFileExcluded(relativePath, config)` from `lib/test-exclude.ts`. If the function returns `true`, the file SHALL be skipped entirely — it SHALL NOT appear in `detected` and SHALL NOT be treated as "unknown".

The module SHALL import `isFileExcluded` from `../lib/test-exclude` at the top of the file.

#### Scenario: excluded file is not in detected array

**WHEN** the project config has `test.exclude: ["**/generated/**"]`
**AND** `runTestDetectFrameworks` is called with `files: ["src/app.ts", "src/generated/api.ts"]`
**AND** the test framework is configured as `"vitest"`
**THEN** `result.detected` SHALL contain an entry for `"src/app.ts"`
**AND** `result.detected` SHALL NOT contain any entry where `file` includes `"generated/api.ts"`
**AND** `result.detected` SHALL have length 1

#### Scenario: excluded file is not treated as "unknown"

**WHEN** the project config has `test.exclude: ["**/generated/**"]`
**AND** `runTestDetectFrameworks` is called with `files: ["src/generated/api.ts", "src/unknown.py"]`
**AND** the test framework is configured as `"vitest"`
**THEN** `result.detected` SHALL contain only `"src/unknown.py"` with framework `"unknown"`
**AND** `result.detected` SHALL NOT contain any entry for `"src/generated/api.ts"`

#### Scenario: override-level exclude also filtered

**WHEN** the project config has `test.overrides: [{ file: "plugins/dev-team/bin", framework: "vite-plus", exclude: ["**/vendor/**"] }]`
**AND** `runTestDetectFrameworks` is called with `files: ["plugins/dev-team/bin/src/app.ts", "plugins/dev-team/bin/vendor/lib.ts"]`
**THEN** `result.detected` SHALL contain an entry for `"plugins/dev-team/bin/src/app.ts"`
**AND** `result.detected` SHALL NOT contain an entry for `"plugins/dev-team/bin/vendor/lib.ts"`

#### Scenario: no exclude configured — backward compatible

**WHEN** the project config has no `test.exclude` and no `test.overrides[].exclude`
**AND** `runTestDetectFrameworks` is called with `["src/app.test.ts", "src/utils.ts"]`
**AND** the test framework is configured as `"vitest"`
**THEN** `result.detected` SHALL contain both `"src/app.test.ts"` (framework: `"vitest"`) and `"src/utils.ts"` (framework: `"unknown"`)
**AND** the results SHALL match the pre-change behavior exactly

#### Scenario: auto-scan mode respects exclude globs

**WHEN** the project config has `test.exclude: ["**/generated/**"]`
**AND** `runTestDetectFrameworks` is called without a `files` argument (auto-scan mode)
**AND** the project filesystem contains `src/app.ts`, `src/generated/api.ts`, and `tools/helper.ts`
**AND** the test framework is configured as `"vitest"`
**THEN** `result.detected` SHALL include `"src/app.ts"` (if it matches vitest default glob)
**AND** `result.detected` SHALL NOT include `"src/generated/api.ts"` regardless of glob matching

### Module Contract (exclude additions)

#### Module: commands/test-detect-frameworks.ts (Added behavior)

| Aspect | Detail |
|--------|--------|
| **New import** | `import { isFileExcluded } from '../lib/test-exclude'` |
| **Imported function** | `isFileExcluded` |
| **Change point** | Inside `detectFrameworksForFiles` function, after computing `relativePath`, before glob matching loop |
| **Added logic** | `if (isFileExcluded(relativePath, config)) continue;` — skip the file entirely |
| **Config access** | The `config` variable is already available in `runTestDetectFrameworks` scope; pass it to `detectFrameworksForFiles` as a new parameter or access via closure |

#### Module: schemas (unchanged types — new field)

| Export | Change |
|--------|--------|
| `TestDetectFrameworksOptions` | Unchanged — exclude filtering is internal, not an input option |
| `TestDetectFrameworksResult` | Unchanged — output shape is identical, only the content is filtered |

---

## ADDED Requirements (Platform-Aware Framework Registry)

### Requirement: FRAMEWORK_REGISTRY entries use shell/cmd nested structure

**ID**: REQ-TDF-PLAT-1
**Priority**: MUST
**Description**: `FrameworkConfig` interface in `lib/test-framework.ts` SHALL be restructured into a two-level nested form. Platform-specific fields (`test_execution`, `coverage_cleanup`) SHALL be grouped under `shell` and `cmd` sub-objects. Platform-independent fields (`framework`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `default_glob`, `mutation_framework`) SHALL remain at the top level.

```typescript
interface FrameworkConfig {
  framework: TestFrameworks;
  shell: {
    test_execution: string;      // Shell (bash/POSIX) test command template
    coverage_cleanup: string[];  // Shell coverage cleanup paths (rm -rf)
  };
  cmd: {
    test_execution: string;      // Windows cmd.exe test command template
    coverage_cleanup: string[];  // Windows cmd.exe coverage cleanup paths (rmdir /s/q, del /q/f)
  };
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  coverage_artifacts: string[];
  default_glob: string;
  mutation_framework: string | null;
}
```

**Field mapping (old → new)**:
- `test_cmd` → `shell.test_execution`
- `coverage_cleanup` → `shell.coverage_cleanup`
- (new) → `cmd.test_execution`
- (new) → `cmd.coverage_cleanup`

#### Scenario: each framework entry has shell and cmd sub-objects

**WHEN** `FRAMEWORK_REGISTRY` is inspected
**THEN** each framework entry SHALL have `shell` and `cmd` sub-objects
**AND** `shell` SHALL contain `test_execution: string` and `coverage_cleanup: string[]`
**AND** `cmd` SHALL contain `test_execution: string` and `coverage_cleanup: string[]`
**AND** the number of framework entries SHALL remain 8

#### Scenario: shell sub-object preserves existing semantics

**WHEN** inspecting `FRAMEWORK_REGISTRY['jest'].shell`
**THEN** `shell.test_execution` SHALL equal the previous `test_cmd` value
**AND** `shell.coverage_cleanup` SHALL equal the previous `coverage_cleanup` value
**AND** the same SHALL be true for all 8 framework entries

#### Scenario: simple framework commands have identical test_execution across platforms

**WHEN** inspecting `FRAMEWORK_REGISTRY['jest']`
**THEN** `shell.test_execution` SHALL equal `cmd.test_execution`
**AND** the same SHALL be true for `vitest`, `vite-plus`, `bun`, `node-test`, and `go`

#### Scenario: chained framework commands have cmd.exe syntax in cmd.test_execution

**WHEN** inspecting `FRAMEWORK_REGISTRY['rust'].cmd.test_execution`
**THEN** it SHALL NOT contain `; _X=$?;`
**AND** it SHALL contain `%errorlevel%` or `errorlevel` for exit code handling
**AND** the same SHALL be true for `pytest`

#### Scenario: cmd.coverage_cleanup mirrors shell.coverage_cleanup paths

**WHEN** inspecting `FRAMEWORK_REGISTRY['jest'].cmd.coverage_cleanup`
**THEN** it SHALL have the same number of entries as `shell.coverage_cleanup`
**AND** each entry SHALL be a valid path or glob
**AND** `getFrameworkConfig('jest').cmd.coverage_cleanup` SHALL return a non-empty array

#### Scenario: getFrameworkConfig returns new nested structure

**WHEN** `getFrameworkConfig('vitest')` is called
**THEN** the returned object SHALL have `shell` and `cmd` sub-objects
**AND** `shell.test_execution` and `cmd.test_execution` SHALL be populated
**AND** `shell.coverage_cleanup` and `cmd.coverage_cleanup` SHALL be populated
**AND** platform-independent fields SHALL be at the top level

### Requirement: TestPlan.script restructured as { shell, cmd } object

**ID**: REQ-TDF-PLAT-2
**Priority**: MUST
**Description**: The `testPlanSchema` in `schemas/test-detect-frameworks.schema.ts` SHALL change the `script` field from `z.string()` to `z.object({ shell: z.string(), cmd: z.string() })`.

- `script.shell` — execution script for bash/POSIX shell (generated by `generateShellScript()` from `frameworkConfig.shell`)
- `script.cmd` — execution script for Windows cmd.exe (generated by `generateCmdScript()` from `frameworkConfig.cmd`)

#### Scenario: script field is an object with shell and cmd

**WHEN** `testPlanSchema` is inspected
**THEN** `script` SHALL be of type `z.ZodObject`
**AND** `script` SHALL contain `shell: z.ZodString` and `cmd: z.ZodString`

#### Scenario: TestPlan type includes script.shell and script.cmd

**WHEN** a `TestPlan` object is created with TypeScript
**THEN** `script.shell: string` SHALL be a required property
**AND** `script.cmd: string` SHALL be a required property
**AND** `script` SHALL be a plain object (not a string)

### Requirement: generateCmdScript produces Windows-compatible cmd.exe script

**ID**: REQ-TDF-PLAT-3
**Priority**: MUST
**Description**: A new function `generateCmdScript(directory: string, frameworkConfig: FrameworkConfig): string` SHALL be added to `commands/test-detect-frameworks.ts`. It SHALL read from `frameworkConfig.cmd` and produce a Windows cmd.exe script:

1. If `directory` is not `'.'`, prepend `cd /d <directory>`
2. For each item in `frameworkConfig.cmd.coverage_cleanup`, append `if exist <item> rmdir /s/q <item>` (directories) or `if exist <item> del /q /f <item>` (files)
3. Append `frameworkConfig.cmd.test_execution`
4. Join with `\r\n` + trailing `\r\n`

Input validation:
- `frameworkConfig` MUST NOT be null/undefined
- `directory` MUST be a string
- `frameworkConfig.cmd.test_execution` MUST be a string
- `frameworkConfig.cmd.coverage_cleanup` MUST be an array

The existing `generateScript()` SHALL be renamed to `generateShellScript()`.

#### Scenario: generateCmdScript validates inputs

**WHEN** `generateCmdScript('.', null)` is called
**THEN** it SHALL throw `TypeError`

**WHEN** `generateCmdScript(null as unknown as string, validConfig)` is called
**THEN** it SHALL throw `TypeError`

**WHEN** `generateCmdScript('.', configWithMissingCmdTestExecution)` is called
**THEN** it SHALL throw `TypeError`

#### Scenario: generateCmdScript produces cmd.exe script for non-root directory

**WHEN** `generateCmdScript('src/lib', frameworkConfig)` is called
**AND** `frameworkConfig.cmd.test_execution` is `'npx jest {files}'`
**AND** `frameworkConfig.cmd.coverage_cleanup` is `['coverage', '.nyc_output']`
**THEN** the result SHALL start with `cd /d src/lib\r\n`
**AND** the result SHALL contain `if exist coverage rmdir /s/q coverage\r\n`
**AND** the result SHALL contain `if exist .nyc_output rmdir /s/q .nyc_output\r\n`
**AND** the result SHALL end with `npx jest {files}\r\n`

#### Scenario: generateCmdScript for root directory skips cd

**WHEN** `generateCmdScript('.', frameworkConfig)` is called
**THEN** the result SHALL NOT contain `cd`
**AND** the result SHALL directly start with cleanup commands and cmd.test_execution

### Requirement: generateShellScript reads from frameworkConfig.shell

**ID**: REQ-TDF-PLAT-4
**Priority**: MUST
**Description**: The renamed `generateShellScript()` function SHALL read from `frameworkConfig.shell` (instead of old flat `frameworkConfig.test_cmd` / `frameworkConfig.coverage_cleanup`). Output SHALL be identical to the previous `generateScript()`.

#### Scenario: generateShellScript output matches old generateScript

**WHEN** `generateShellScript('.', jestConfig)` is called
**THEN** the output SHALL equal the previous `generateScript('.', jestConfig)` for the same inputs
**AND** the same SHALL be true for all 8 frameworks and both `directory === '.'` / `directory !== '.'` cases

### Requirement: buildPlanFromMappings produces script object

**ID**: REQ-TDF-PLAT-5
**Priority**: MUST
**Description**: `buildPlanFromMappings()` SHALL set `script` to `{ shell: generateShellScript(...), cmd: generateCmdScript(...) }` on each plan entry.

#### Scenario: plan entry script is an object

**WHEN** `runTestDetectFrameworks({})` is called
**THEN** each entry in `plan` SHALL have `script` of type object
**AND** `script.shell` SHALL be a non-empty string
**AND** `script.cmd` SHALL be a non-empty string
**AND** `script.cmd` SHALL NOT equal `script.shell` for frameworks with different command syntax (rust, pytest)

#### Scenario: script.cmd uses cmd.exe syntax

**WHEN** inspecting `plan[0].script.cmd` for framework `"jest"` with `directory === "."`
**THEN** `script.cmd` SHALL contain `rmdir /s/q` (not `rm -rf`)

## Module Contract (Platform-Aware Framework Config)

### Interface: FrameworkConfig (MODIFIED)

```typescript
export interface FrameworkConfig {
  framework: TestFrameworks;
  shell: {
    test_execution: string;
    coverage_cleanup: string[];
  };
  cmd: {
    test_execution: string;
    coverage_cleanup: string[];
  };
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  coverage_artifacts: string[];
  default_glob: string;
  mutation_framework: string | null;
}
```

### Schema: testPlanSchema (MODIFIED)

```typescript
const testPlanSchema = z.object({
  // ... all existing fields unchanged ...
  script: z.object({
    shell: z.string().describe('Shell (bash/POSIX) execution script'),
    cmd: z.string().describe('Windows cmd.exe execution script'),
  }),
});
```

### Module: commands/test-detect-frameworks.ts (MODIFIED)

| Function | Before | After |
|----------|--------|-------|
| `generateScript(d, cfg)` | Reads `cfg.test_cmd`, `cfg.coverage_cleanup` | Renamed to `generateShellScript()` — reads `cfg.shell` |
| `generateShellScript(d, cfg)` | (did not exist) | Former `generateScript()` — reads `cfg.shell` |
| `generateCmdScript(d, cfg)` | (did not exist) | New — reads `cfg.cmd`, produces cmd.exe script |
| `buildPlanFromMappings(maps, root)` | Sets `script: string` | Sets `script: { shell, cmd }` via both generators |
