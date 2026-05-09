## Why

The 6 hook files in `plugin/hooks/` each contain ~25 lines of duplicate code for importing `hook-output.py` with try/except fallback handling. Since `hook-output.py` is shipped with the plugin, the fallback is never needed. This complexity adds maintenance burden and makes the code harder to read.

## What Changes

- Remove the `importlib.util` dynamic import pattern for `hook-output.py` from all 6 hook files
- Replace with direct `from hook_output import ...` statements
- Delete the fallback implementations (2 identical copies per file)
- Keep the `sys.path.insert(0, UTILS_DIR)` pattern that enables the imports

## Capabilities

### New Capabilities

(none - this is a refactoring)

### Modified Capabilities

(none - this is a refactoring, no spec-level behavior changes)

## Impact

Affected files:
- `plugin/hooks/on-user-prompt.py`
- `plugin/hooks/pre-tool-openspec-test.py`
- `plugin/hooks/pre-tool-commit-review.py`
- `plugin/hooks/pre-tool-skill.py`
- `plugin/hooks/session-start-ensure-openspec.py`
- `plugin/hooks/session-start-worktree.py`

No API or dependency changes. Each file becomes ~20 lines shorter.
