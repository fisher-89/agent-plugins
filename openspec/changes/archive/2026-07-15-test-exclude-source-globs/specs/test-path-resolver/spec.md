## ADDED Requirements

### Requirement: Excluded files filtered from non-empty modules resolution

**ID**: REQ-TPR-EXC-1
**Priority**: MUST
**Description**: The `processNonEmptyModules` function in `commands/test-resolve-paths.ts` SHALL apply exclude filtering after the "in test config scope" check and before adding a source file to the `unitTestMap`. The function SHALL import `isFileExcluded` from `../lib/test-exclude`, obtain the project config via `readConfig`, and for each valid source file check `isFileExcluded(posix, config)`. If the function returns `true`, the file SHALL be skipped — no `unit_tests` entry SHALL be added and the file SHALL NOT appear in `collectedSources`.

Files excluded at this stage SHALL NOT produce an error entry in the `errors` array (the exclusion is intentional, not an error).

#### Scenario: excluded file not in unit_tests for non-empty modules

**WHEN** the project config has `test.exclude: ["**/generated/**"]`
**AND** `resolveTestPaths` is called with `modules: ["src/app.ts", "src/generated/api.ts"]`
**AND** test config covers both files
**THEN** `unit_tests` SHALL contain `{source: "src/app.ts", test_file: "src/app.test.ts"}`
**AND** `unit_tests` SHALL NOT contain any entry with source `"src/generated/api.ts"`
**AND** `errors` SHALL NOT contain an entry for `"src/generated/api.ts"` (exclusion is silent)

#### Scenario: override-level exclude also filtered in non-empty mode

**WHEN** the project config has `test.overrides: [{ file: "plugins/dev-team/bin", framework: "vite-plus", exclude: ["**/vendor/**"] }]`
**AND** `resolveTestPaths` is called with `modules: ["plugins/dev-team/bin/src/app.ts", "plugins/dev-team/bin/vendor/lib.ts"]`
**THEN** `unit_tests` SHALL contain `{source: "plugins/dev-team/bin/src/app.ts", ...}`
**AND** `unit_tests` SHALL NOT contain an entry for `"plugins/dev-team/bin/vendor/lib.ts"`

### Requirement: Excluded files filtered from empty-modules auto-scan

**ID**: REQ-TPR-EXC-2
**Priority**: MUST
**Description**: The `processEmptyModules` function SHALL apply exclude filtering after the `isSourceFile` check and before adding a file to the `sourceFiles` set. For each discovered source file, SHALL call `isFileExcluded(posix, config)`; if `true`, the file SHALL be skipped.

#### Scenario: excluded file not in unit_tests for empty-modules mode

**WHEN** the project config has `test.exclude: ["**/generated/**"]`
**AND** `resolveTestPaths` is called with `modules: []`
**AND** the auto-scan discovers `src/app.ts`, `src/utils.ts`, and `src/generated/api.ts`
**AND** test config covers the scanning directory
**THEN** `unit_tests` SHALL contain entries for `"src/app.ts"` and `"src/utils.ts"`
**AND** `unit_tests` SHALL NOT contain an entry for `"src/generated/api.ts"`

### Requirement: Excluded files filtered from git-change mode

**ID**: REQ-TPR-EXC-3
**Priority**: MUST
**Description**: When `modules` is `"git-change"`, the exclude filtering SHALL be applied through the same path as non-empty modules — since `resolveEffectiveModules` converts git diff output into a module list and delegates to `processNonEmptyModules`, the exclude filtering defined in REQ-TPR-EXC-1 SHALL apply automatically.

#### Scenario: git-change mode respects test.exclude

**WHEN** the project config has `test.exclude: ["**/generated/**"]`
**AND** `resolveTestPaths` is called with `modules: "git-change"`
**AND** `git diff HEAD --name-only` returns `["src/app.ts", "src/generated/api.ts"]`
**AND** test config covers both files
**THEN** `unit_tests` SHALL contain only `{source: "src/app.ts", test_file: "src/app.test.ts"}`
**AND** `unit_tests` SHALL NOT contain an entry for `"src/generated/api.ts"`

### Requirement: No exclude configured — backward compatible

**ID**: REQ-TPR-EXC-4
**Priority**: MUST
**Description**: When no `test.exclude` or `test.overrides[].exclude` is configured, the behavior of all three resolution modes (non-empty modules, empty modules, git-change) SHALL be identical to the pre-change behavior.

#### Scenario: non-empty modules unchanged when no exclude

**WHEN** the project config has no `test.exclude` and no `test.overrides[].exclude`
**AND** `resolveTestPaths` is called with `modules: ["src/app.ts", "src/utils.ts"]`
**AND** test config covers both files
**THEN** `unit_tests` SHALL contain both entries (same as before the change)

#### Scenario: empty modules unchanged when no exclude

**WHEN** the project config has no `test.exclude` and no `test.overrides[].exclude`
**AND** `resolveTestPaths` is called with `modules: []`
**THEN** the auto-scan behavior SHALL be identical to the pre-change behavior

#### Scenario: git-change mode unchanged when no exclude

**WHEN** the project config has no `test.exclude` and no `test.overrides[].exclude`
**AND** `resolveTestPaths` is called with `modules: "git-change"`
**THEN** the git diff behavior SHALL be identical to the pre-change behavior

---

## Module Contract (exclude additions)

### Module: commands/test-resolve-paths.ts (Added behavior)

| Aspect | Detail |
|--------|--------|
| **New import** | `import { readConfig } from '../lib/config'` and `import { isFileExcluded } from '../lib/test-exclude'` |
| **Import (if not already present)** | `readConfig` is already available in the module scope; verify or add |
| **Change point in processNonEmptyModules** | After the `isSourceFile(posix)` check passes, before `collectedSources.push(posix)` + `addUnitTest(...)` |
| **Added logic in processNonEmptyModules** | `if (isFileExcluded(posix, config)) continue;` |
| **Change point in processEmptyModules** | After `if (isSourceFile(posix))`, before `sourceFiles.add(posix)` |
| **Added logic in processEmptyModules** | `if (isFileExcluded(posix, config)) continue;` |
| **Config access** | In `resolveTestPaths`, call `readConfig(projectRoot)` once and pass it to both `processNonEmptyModules` and `processEmptyModules` |
| **Error behavior** | Excluded files produce NO error entries — the skip is silent, distinguishing exclusion from "not in test config scope" |

### Function signature change: processNonEmptyModules

| Property | Before | After |
|----------|--------|-------|
| **Parameters** | `(effectiveModules, projectRoot, unitTestMap, errors, collectedSources)` | `(effectiveModules, projectRoot, unitTestMap, errors, collectedSources, config)` |
| **New parameter** | — | `config: OpenSpecConfig` — parsed config used for exclude check |

### Function signature change: processEmptyModules

| Property | Before | After |
|----------|--------|-------|
| **Parameters** | `(projectRoot, unitTestMap, errors, collectedSources)` | `(projectRoot, unitTestMap, errors, collectedSources, config)` |
| **New parameter** | — | `config: OpenSpecConfig` — parsed config used for exclude check |
