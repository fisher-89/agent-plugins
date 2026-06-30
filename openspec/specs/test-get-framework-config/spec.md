## Purpose

This capability defined the `test_get_framework_config` MCP tool and its underlying `FRAMEWORK_REGISTRY` in `commands/test-get-framework-config.ts`. As of the `merge-framework-config-into-detect` change, the MCP tool is **removed** and the registry has been extracted to an internal lib module at `lib/test-framework.ts`, consumed by `test-detect-frameworks.ts` rather than exposed as a standalone MCP tool.

For framework configuration queries, consumers SHALL use `test_detect_frameworks` which returns all fields (`coverage_cmd`, `coverage_format`, `coverage_output`, `coverage_artifacts`, `coverage_cleanup`) in its `plan[]` entries.

---

## Module Contract

### Module: lib/test-framework.ts (Internal — Formerly commands/test-get-framework-config.ts)

The `FRAMEWORK_REGISTRY` constant, `runTestGetFrameworkConfig()`, and `getDefaultGlobForFramework()` are located in `lib/test-framework.ts` as an internal (non-MCP) module.

| Function | Input | Output | Side Effects |
|----------|-------|--------|-------------|
| `runTestGetFrameworkConfig({framework})` | `{framework: TestFrameworks}` | `FrameworkConfig` | None — pure lookup |
| `getDefaultGlobForFramework(framework)` | `framework: string` | `string` (glob pattern) | None — pure lookup |

All eight framework entries are preserved: jest, vitest, vite-plus, bun, rust, node-test, go, pytest.

### MCP Tool: test_get_framework_config (Removed)

| Aspect | Detail |
|--------|--------|
| Tool name | `test_get_framework_config` (removed) |
| Input | `{framework: TestFrameworks, project_root?: string}` |
| Reason | All output fields are already available in `test_detect_frameworks.plan[]`. No consumer actually needs `test_cmd`. |
| Migration | Call `test_detect_frameworks({files: [...]})` and use `plan[].framework` for framework name, `plan[].coverage_cmd`/`plan[].coverage_format`/`plan[].coverage_output`/`plan[].coverage_artifacts`/`plan[].coverage_cleanup` for config. |

---

## Requirements

### Requirement: lib/test-framework.ts internal module exists

**ID**: REQ-TF-1
**Priority**: MUST
**Description**: A new internal module `lib/test-framework.ts` SHALL contain the `FRAMEWORK_REGISTRY` constant and the `runTestGetFrameworkConfig()` and `getDefaultGlobForFramework()` functions, preserving all eight framework entries (jest, vitest, vite-plus, bun, rust, node-test, go, pytest) with their complete configuration.

#### Scenario: lib/test-framework.ts exports the registry functions

**WHEN** `import { runTestGetFrameworkConfig, getDefaultGlobForFramework } from '../lib/test-framework'`
**THEN** both functions SHALL be exported and callable
**AND** `runTestGetFrameworkConfig({framework: "jest"})` SHALL return a `FrameworkConfig` object with `framework: "jest"` and all 8 fields

#### Scenario: all eight frameworks are registered

**WHEN** `runTestGetFrameworkConfig()` is called for each of `["jest", "vitest", "vite-plus", "bun", "rust", "node-test", "go", "pytest"]`
**THEN** each call SHALL return a valid `FrameworkConfig` object with non-empty `test_cmd`, `coverage_cmd`, `coverage_artifacts`, `coverage_cleanup`, and `default_glob`

### Requirement: test-detect-frameworks imports from lib/test-framework

**ID**: REQ-TF-2
**Priority**: MUST
**Description**: `commands/test-detect-frameworks.ts` SHALL import `runTestGetFrameworkConfig` and `getDefaultGlobForFramework` from `../lib/test-framework` instead of `./test-get-framework-config`.

#### Scenario: import path updated

**WHEN** the source of `commands/test-detect-frameworks.ts` is inspected
**THEN** the import line SHALL read `from '../lib/test-framework'`
**AND** SHALL NOT read `from './test-get-framework-config'`
**AND** all existing `test-detect-frameworks` unit tests SHALL pass

### Requirement: MCP tool test_get_framework_config removed

**ID**: REQ-TF-3
**Priority**: MUST
**Description**: The MCP tool `test_get_framework_config` SHALL be removed from `mcp.ts`, including its registration function, schema import, and the associated schema file.

#### Scenario: MCP registration removed

**WHEN** `mcp.ts` is inspected
**THEN** there SHALL be no call to `registerTestGetFrameworkConfigTool`
**AND** there SHALL be no import from `'./commands/test-get-framework-config'`
**AND** `schemas/index.ts` SHALL NOT export `testGetFrameworkConfigInputSchema` or `testGetFrameworkConfigOutputSchema`

#### Scenario: schema file deleted

**WHEN** the filesystem is inspected
**THEN** `schemas/test-get-framework-config.schema.ts` SHALL NOT exist
**AND** `commands/test-get-framework-config.ts` SHALL NOT exist

### Requirement: test-gen-generator stops calling test_get_framework_config

**ID**: REQ-TF-4
**Priority**: MUST
**Description**: The `test-gen-generator.md` agent SHALL be updated to remove all references to `test_get_framework_config`. The agent SHALL use the `frameworks[]` or `plan[].framework` values from `test_detect_frameworks` to determine framework name for syntax selection.

#### Scenario: agent references removed

**WHEN** `plugins/dev-team/agents/test-gen-generator.md` is inspected
**THEN** it SHALL NOT contain the string `test_get_framework_config`
**AND** the framework resolution step SHALL reference framework names from `test_detect_frameworks` result only

#### Scenario: framework name still available for syntax selection

**WHEN** `test_detect_frameworks({})` is called
**AND** the result contains `frameworks: ["vitest"]`
**THEN** the agent SHALL use `"vitest"` from the result to select the vitest test syntax
