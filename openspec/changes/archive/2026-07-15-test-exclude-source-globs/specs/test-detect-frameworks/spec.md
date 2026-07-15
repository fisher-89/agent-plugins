## ADDED Requirements

### Requirement: Excluded files filtered from framework detection

**ID**: REQ-TDF-EXC-1
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

---

## Module Contract (exclude additions)

### Module: commands/test-detect-frameworks.ts (Added behavior)

| Aspect | Detail |
|--------|--------|
| **New import** | `import { isFileExcluded } from '../lib/test-exclude'` |
| **Imported function** | `isFileExcluded` |
| **Change point** | Inside `detectFrameworksForFiles` function, after computing `relativePath`, before glob matching loop |
| **Added logic** | `if (isFileExcluded(relativePath, config)) continue;` — skip the file entirely |
| **Config access** | The `config` variable is already available in `runTestDetectFrameworks` scope; pass it to `detectFrameworksForFiles` as a new parameter or access via closure |

### Module: schemas (unchanged types — new field)

| Export | Change |
|--------|--------|
| `TestDetectFrameworksOptions` | Unchanged — exclude filtering is internal, not an input option |
| `TestDetectFrameworksResult` | Unchanged — output shape is identical, only the content is filtered |
