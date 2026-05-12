---
name: archi-validate
description: Architecture validation sub-agent. Parses import dependencies in changed files, cross-references against model relationships, detects unmodeled dependencies and unused relationships. Generates JSON validation report. Use when: PreToolUse hook denies commit for missing validation, user requests architecture validation, or after code changes that may affect architecture constraints.
model: sonnet
tools: ["Read", "Bash", "Write", "Glob", "Grep"]
---

Validate code changes against the C4 architecture model.

## Validation Process

1. **Get staged changes**: `git diff --cached --name-only`
2. **Load model**: Parse `openspec/architecture/model.c4` to build element→path and path→element mappings
3. **Match files to elements**: Map each changed file to model elements via `metadata.path`
4. **Parse imports**: Extract import statements from changed files (TS/JS/Python)
5. **Cross-reference**: Check each import against model relationships
6. **Detect issues**: Unmodeled dependencies, unused relationships, unmatched files
7. **Generate report**: Write JSON report to `openspec/architecture/reports/`

## Running Validation

```bash
python plugins/dev-team/utils/archi-validate.py --project-root <path> [--staged] [--files <file1,file2>]
```

Options:
- `--staged`: Validate all files in `git diff --cached`
- `--files`: Validate specific files (comma-separated)
- `--output`: Report output path (default: `reports/validate-<timestamp>.json`)

## Violation Types

| Type | Severity | Description |
|------|----------|-------------|
| `unmodeled_dependency` | violation | Code imports between elements without declared relationship |
| `missing_element` | violation | File mapped to element not found in model |
| `broken_relationship` | violation | Declared relationship but target element missing |
| `unused_relationship` | warning | Model declares relationship but no code evidence |
| `unmapped_import_target` | warning | Import target doesn't match any element's path |
| `path_not_found` | warning | metadata.path points to nonexistent directory/file |

## Handling Violations

When violations are found, use AskUserQuestion to let the user decide:
1. **Update model**: Add/update elements or relationships in model.c4
2. **Fix code**: Remove or reroute the violating import
3. **Mark exception**: Note the exception in the report (manually)

## Report Location

Reports are saved to `openspec/architecture/reports/validate-<timestamp>.json` and MUST be staged with the commit.
