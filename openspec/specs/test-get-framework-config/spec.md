## Module Contract

### Module: test-get-framework-config.ts (Framework Command Registry)

| Function | Input | Output | Side Effects |
|----------|-------|--------|-------------|
| `runTestGetFrameworkConfig({framework: "node-test"})` | `{framework: "node-test"}` | `FrameworkConfig` | None — pure lookup |

### FrameworkConfig for `node-test`

| Field | Value after change | Previously |
|-------|-------------------|------------|
| `framework` | `"node-test"` | `"node-test"` (unchanged) |
| `test_cmd` | `"node --test"` | `"node --test"` (unchanged) |
| `coverage_cmd` | `"node --test --experimental-test-coverage"` | `"node --test --experimental-test-coverage 2>&1 \| tee coverage/node-test-output.txt && node plugins/dev-team/scripts/parse-node-test-coverage.mjs coverage/node-test-output.txt coverage/coverage-summary.json"` |
| `coverage_format` | `"node-test"` | `"node-test"` (unchanged) |
| `coverage_output` | `"coverage/node-test-output.txt"` | `"coverage/coverage-summary.json"` |
| `coverage_artifacts` | `["coverage/node-test-output.txt"]` | `["coverage/coverage-summary.json"]` |
| `coverage_cleanup` | `["coverage"]` | `["coverage"]` (unchanged) |
| `default_glob` | `"**/*.test.{mjs,js,cjs}"` | `"**/*.test.{mjs,js,cjs}"` (unchanged) |

---

## ADDED Requirements

All requirements below are new for this capability, documenting the simplified `node-test` framework configuration.

### Requirement: node-test framework coverage command simplification

**ID**: REQ-TGFC-1
**Priority**: MUST
**Description**: The `node-test` framework's `coverage_cmd` in `FRAMEWORK_REGISTRY` SHALL be simplified to run `node --test --experimental-test-coverage` without piping through `tee` or calling an external parser script.

#### Scenario: coverage_cmd returns simplified command

**WHEN** `runTestGetFrameworkConfig({framework: "node-test"})` is called
**THEN** the returned `coverage_cmd` SHALL be exactly `"node --test --experimental-test-coverage"`
**AND** `coverage_cmd` SHALL NOT contain the string `"tee"`
**AND** `coverage_cmd` SHALL NOT contain the string `"parse-node-test-coverage.mjs"`
**AND** `coverage_cmd` SHALL NOT contain the pipe character `"|"`

### Requirement: node-test framework coverage_output points to raw text file

**ID**: REQ-TGFC-2
**Priority**: MUST
**Description**: The `node-test` framework's `coverage_output` SHALL point to the raw text table output file from `node:test` rather than a JSON file. This follows the same pattern as `go-cover` where `coverage_output` is `coverage/func-summary.txt`.

#### Scenario: coverage_output returns raw text path

**WHEN** `runTestGetFrameworkConfig({framework: "node-test"})` is called
**THEN** the returned `coverage_output` SHALL be `"coverage/node-test-output.txt"`

### Requirement: node-test framework coverage_artifacts updated

**ID**: REQ-TGFC-3
**Priority**: SHOULD
**Description**: The `node-test` framework's `coverage_artifacts` SHALL reference the raw text output file for artifact management (moving to unified reports directory).

#### Scenario: coverage_artifacts returns raw text path

**WHEN** `runTestGetFrameworkConfig({framework: "node-test"})` is called
**THEN** the returned `coverage_artifacts` SHALL contain exactly one element
**AND** that element SHALL be `"coverage/node-test-output.txt"`

### Requirement: other framework entries remain unchanged

**ID**: REQ-TGFC-4
**Priority**: MUST
**Description**: Only the `node-test` framework entry SHALL be modified. All other framework entries in `FRAMEWORK_REGISTRY` (jest, vitest, vite-plus, bun, rust, go, pytest) SHALL remain exactly as they were before this change.

#### Scenario: jest framework unchanged

**WHEN** `runTestGetFrameworkConfig({framework: "jest"})` is called
**THEN** the returned config SHALL match the existing expected jest config with `coverage_cmd` containing `"npx jest --coverage"`, `coverage_output` being `"coverage/coverage-summary.json"`, and `coverage_artifacts` being `["coverage/coverage-summary.json"]`

#### Scenario: go framework unchanged

**WHEN** `runTestGetFrameworkConfig({framework: "go"})` is called
**THEN** the returned config SHALL match the existing expected go config with `coverage_cmd` containing `"go tool cover"`, `coverage_output` being `"coverage/func-summary.txt"`, and `coverage_artifacts` being `["coverage/func-summary.txt"]`

#### Scenario: all eight frameworks are still registered

**WHEN** `runTestGetFrameworkConfig()` is called for each of `["jest", "vitest", "vite-plus", "bun", "rust", "node-test", "go", "pytest"]`
**THEN** each call SHALL return a valid `FrameworkConfig` object with non-empty `test_cmd`, `coverage_cmd`, `coverage_artifacts`, and `coverage_cleanup`
