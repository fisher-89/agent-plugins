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
| Plan entry fields | `directory`, `framework`, `coverage_cmd`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`, `script` |
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
**AND** each entry in `plan` SHALL contain the fields `directory`, `framework`, `coverage_cmd`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`, and `script`

### Requirement: plan[] output carries all framework config fields

**ID**: REQ-TDF-3
**Priority**: MUST
**Description**: The `plan[]` array in `test_detect_frameworks` output SHALL continue to include all framework config fields (`coverage_cmd`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`) sourced from the internal `lib/test-framework.ts` FRAMEWORK_REGISTRY. Every field that was previously populated via `runTestGetFrameworkConfig()` SHALL be present after the import path change.

#### Scenario: plan entry coverage fields match framework registry

**WHEN** `test_detect_frameworks` is called on a project configured with framework `"vitest"`
**THEN** the returned `plan[0].coverage_cmd` SHALL match the `coverage_cmd` value in the FRAMEWORK_REGISTRY for `vitest`
**AND** `plan[0].coverage_format` SHALL be `"istanbul"`
**AND** `plan[0].coverage_output` SHALL be `"coverage/coverage-summary.json"`
**AND** `plan[0].coverage_artifacts` SHALL contain `"coverage/coverage-summary.json"`
**AND** `plan[0].coverage_cleanup` SHALL be a non-empty array

#### Scenario: node-test framework plan entry carries simplified fields

**WHEN** `test_detect_frameworks` is called on a project configured with framework `"node-test"`
**THEN** the returned `plan[0].coverage_cmd` SHALL be exactly `"node --test --experimental-test-coverage"`
**AND** `plan[0].coverage_output` SHALL be `"coverage/node-test-output.txt"`
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
