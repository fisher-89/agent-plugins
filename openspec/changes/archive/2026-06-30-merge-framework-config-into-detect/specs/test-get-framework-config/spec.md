## Module Contract

### Module: test-get-framework-config.ts → lib/test-framework.ts (Relocated)

The `test-get-framework-config.ts` command module SHALL be removed. Its `FRAMEWORK_REGISTRY`, `runTestGetFrameworkConfig()`, and `getDefaultGlobForFramework()` are relocated to `lib/test-framework.ts` as an internal module.

| Change | Detail |
|--------|--------|
| Old location | `commands/test-get-framework-config.ts` (MCP-tool-facing) |
| New location | `lib/test-framework.ts` (internal lib — no MCP exposure) |
| Exported API | Unchanged: `runTestGetFrameworkConfig()`, `getDefaultGlobForFramework()` |
| Consumers | `test-detect-frameworks.ts` imports from `'../lib/test-framework'` (previously `'./test-get-framework-config'`) |

### MCP Tool Removed: `test_get_framework_config`

| Aspect | Detail |
|--------|--------|
| Tool name | `test_get_framework_config` |
| Input | `{framework: TestFrameworks, project_root?: string}` |
| Output | `FrameworkConfig` (test_cmd, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup) |
| Reason | All output fields except `test_cmd` are already available in `test_detect_frameworks.plan[]`. No consumer actually needs `test_cmd`. |
| Migration | Call `test_detect_frameworks({files: [...]})` and use `plan[].framework` for framework name, `plan[].coverage_cmd`/`plan[].coverage_format`/`plan[].coverage_output`/`plan[].coverage_artifacts`/`plan[].coverage_cleanup` for config. If `test_cmd` is needed, construct from framework conventions. |

---

## REMOVED Requirements

All requirements previously defined under this capability are removed because the MCP tool `test_get_framework_config` is being removed. The underlying framework data (FRAMEWORK_REGISTRY) persists as an internal lib module.

### Requirement: REQ-TGFC-1 — node-test framework coverage command simplification

**Reason**: This requirement documents the `node-test` coverage command format in the now-removed MCP tool. The coverage command data persists in `lib/test-framework.ts` and is accessible via `test_detect_frameworks.plan[].coverage_cmd`.

**Migration**: To verify the `node-test` coverage command, call `test_detect_frameworks({files: [...]})` and assert `plan[].coverage_cmd === "node --test --experimental-test-coverage"`.

### Requirement: REQ-TGFC-2 — node-test framework coverage_output points to raw text file

**Reason**: This requirement validates the `node-test` coverage output path in the now-removed MCP tool. The output path data persists in `lib/test-framework.ts` and is accessible via `test_detect_frameworks.plan[].coverage_output`.

**Migration**: To verify the `node-test` coverage output path, call `test_detect_frameworks({files: [...]})` and assert `plan[].coverage_output === "coverage/node-test-output.txt"`.

### Requirement: REQ-TGFC-3 — node-test framework coverage_artifacts updated

**Reason**: This requirement validates the `node-test` coverage artifacts in the now-removed MCP tool. The artifacts data persists in `lib/test-framework.ts` and is accessible via `test_detect_frameworks.plan[].coverage_artifacts`.

**Migration**: To verify the `node-test` coverage artifacts, call `test_detect_frameworks({files: [...]})` and assert `plan[].coverage_artifacts === ["coverage/node-test-output.txt"]`.

### Requirement: REQ-TGFC-4 — other framework entries remain unchanged

**Reason**: This requirement validates all eight framework entries remain registered. The registry persists in `lib/test-framework.ts` and the same validation can be performed on internal exports.

**Migration**: To verify all eight frameworks, import `runTestGetFrameworkConfig` from `lib/test-framework.ts` directly and assert each framework returns valid config.

---

## ADDED Requirements

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
