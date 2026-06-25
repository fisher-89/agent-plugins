## Module Contract

### Module: unit-test-executor.md (Agent Specification)

| Aspect | Description |
|--------|-------------|
| Model | `sonnet-4.6` |
| Input | `test-design.md`, project `CLAUDE.md` |
| Output | `openspec/changes/<change-name>/reports/unit-test-execution.json` |
| Key Step | Step 4 — Coverage parsing from raw text files |

### Coverage parsing per `coverage_format`

| Format | Source file | Extraction method |
|--------|------------|-------------------|
| `istanbul` | `coverage/coverage-summary.json` | Read JSON: `total.lines.pct` → `lines`, `total.branches.pct` → `branches`, `total.functions.pct` → `functions` |
| `llvm-cov` | JSON output file | Read JSON: `data[0].totals.lines.percent` → `lines`, `data[0].totals.branches.percent` → `branches`, `data[0].totals.functions.percent` → `functions` |
| `node-test` | `coverage/node-test-output.txt` | Regex: match `all files` row, extract `% Stmts`, `% Branch`, `% Funcs`, `% Lines` columns |
| `go-cover` | `coverage/func-summary.txt` | Regex: match `total:` line, extract percentage |
| `coverage-py` | `coverage.json` | Read JSON: `totals.percent_covered` → `lines`, `totals.percent_covered_branches` → `branches` |

---

## ADDED Requirements

### Requirement: node-test coverage parsing from raw text table

**ID**: REQ-UTE-1
**Priority**: MUST
**Description**: The executor agent SHALL parse `node-test` coverage from the raw text table output file (`coverage/node-test-output.txt`) using regex to extract the `all files` summary row, instead of reading a pre-parsed Istanbul-format JSON file. This follows the same pattern as `go-cover` which parses `func-summary.txt` with a `total:` line regex.

#### Scenario: node-test coverage parsing via "all files" regex

**WHEN** processing coverage for `node-test` format in step 4
**THEN** the agent SHALL read `coverage/node-test-output.txt`
**AND** apply a regex matching the `all files` summary row (containing `% Stmts`, `% Branch`, `% Funcs`, `% Lines` columns)
**AND** extract `lines` from the `% Lines` percentage column
**AND** extract `branches` from the `% Branch` percentage column
**AND** extract `functions` from the `% Funcs` percentage column

#### Scenario: node-test parser script no longer referenced

**WHEN** reading the executor agent's step 4 section for `node-test`
**THEN** the instructions SHALL NOT reference `parse-node-test-coverage.mjs` or any intermediate parser script
**AND** the instructions SHALL NOT reference `coverage-summary.json` for `node-test` format

### Requirement: node-test follows go-cover parsing pattern

**ID**: REQ-UTE-2
**Priority**: SHOULD
**Description**: The `node-test` coverage parsing instructions in the executor agent SHALL follow the same principle as `go-cover`: raw text file as input, regex extraction, structured data output. Both formats produce coverage percentages without requiring an intermediate JSON deserialization.

#### Scenario: parsing pattern mirrors go-cover

**WHEN** reviewing the executor agent's step 4 for both `go-cover` and `node-test`
**THEN** both entries SHALL:
- read a `.txt` coverage output file
- use regex to extract the relevant summary line
- produce `{lines, branches, functions}` as the parsed result
**AND** neither entry SHALL require calling an external script or tool to convert the raw output

### Requirement: coverage result structure unchanged

**ID**: REQ-UTE-3
**Priority**: MUST
**Description**: Despite the change in where coverage data comes from (raw text table instead of JSON), the parsed coverage result for `node-test` SHALL produce the same three-dimension structure `{lines: number, branches: number, functions: number}` as before.

#### Scenario: three dimensions produced from raw text

**WHEN** the executor parses `node-test` coverage from `coverage/node-test-output.txt`
**THEN** the result SHALL contain exactly three keys: `lines` (from `% Lines`), `branches` (from `% Branch`), `functions` (from `% Funcs`)
**AND** each value SHALL be a number (percentage 0-100)
**AND** the structure SHALL match the format expected by the report JSON schema
