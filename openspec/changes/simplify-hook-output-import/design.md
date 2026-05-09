## Context

All 6 hook files in `plugin/hooks/` import `hook-output.py` using a defensive pattern:

```python
try:
    import importlib.util
    _ho_path = os.path.join(UTILS_DIR, "hook-output.py")
    if os.path.isfile(_ho_path):
        _ho_spec = importlib.util.spec_from_file_location("hook_output", _ho_path)
        _ho_module = importlib.util.module_from_spec(_ho_spec)
        _ho_spec.loader.exec_module(_ho_module)
        output_xxx = _ho_module.output_xxx
    else:
        def output_xxx(...):  # fallback 1
            ...
except Exception:
    def output_xxx(...):  # fallback 2 (identical to fallback 1)
        ...
```

This pattern is copied into every hook file. The `sys.path.insert(0, UTILS_DIR)` call already adds the utils directory to the import path, making direct imports possible.

## Goals / Non-Goals

**Goals:**
- Replace `importlib.util` dynamic import of `hook-output.py` with direct `from hook_output import ...`
- Remove all fallback implementations (they are unreachable since `hook-output.py` ships with the plugin)
- Reduce code duplication across hook files

**Non-Goals:**
- Refactoring imports of other utils (`active-change.py`, `report-chain.py`, etc.) — out of scope for this change
- Adding `__init__.py` to `plugin/utils/` — not needed for the `sys.path` import approach
- Changing the hook output API or behavior

## Decisions

**Decision 1: Use `from hook_output import <function>` directly**

Since `sys.path.insert(0, UTILS_DIR)` already runs before the import, `hook_output` is importable as a standard module.

Alternative considered: Keep `importlib.util` but remove fallbacks — rejected because it adds no value over a direct import.

**Decision 2: Keep `sys.path.insert` pattern**

The `SCRIPT_DIR → PLUGIN_ROOT → UTILS_DIR → sys.path.insert` pattern is the established way to locate utils. No reason to change it.

**Decision 3: Import only the needed function per file**

Each hook imports only the output function it needs:
- PreToolUse hooks: `from hook_output import output_pre_tool_use`
- UserPromptSubmit hooks: `from hook_output import output_user_prompt_submit`
- SessionStart hooks: `from hook_output import output_session_start`

## Risks / Trade-offs

- [Risk: ImportError if utils dir not on path] → Mitigation: `sys.path.insert` runs before the import, and this is the same mechanism already in use. No real risk.
- [Risk: Breaking change if hook-output.py is removed] → Mitigation: `hook-output.py` is part of the plugin and always deployed together. The old fallback was also unreachable in practice.
