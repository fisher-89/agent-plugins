## Why

Architecture model operations (writing DSL, suggesting updates, creating ADRs, running validation) are currently spread across Python utils with no intelligent orchestrator. Users need an agent that can understand the codebase and propose architecture changes conversationally, and a skill to invoke it.

## What Changes

- **NEW**: `agents/architecture.md` — multi-function subagent for propose, validate, decide, review modes
- **NEW**: `skills/update-architecture/SKILL.md` — thin routing skill with `disable-model-invocation: true`
- **BREAKING**: `architecture-model` — single `model.c4` replaced by directory tree `models/*.c4`; `archi-model.py` aggregates across all files
- **MODIFIED**: `architecture-validation` — `load_model()` reads all `models/*.c4` instead of single file
- **MODIFIED**: `architecture-commit-gate` — checks `models/` directory existence instead of `model.c4`

## Capabilities

### New Capabilities

- `architecture-agent`: LLM subagent that orchestrates architecture operations (propose model changes, validate code against model, create ADRs, review model quality) via Python utils. Always proposes before writing — no autonomous model modification.
- `update-architecture-skill`: User-invocable skill (`/dev-team:update-architecture`) that routes to the architecture subagent. `disable-model-invocation: true` prevents the model from suggesting it proactively.

### Modified Capabilities

- `architecture-model`: Model storage changes from single `openspec/architecture/model.c4` file to `openspec/architecture/models/*.c4` directory tree. `archi-model.py` aggregates across all files. `write` command requires `--path` to target a specific file.
- `architecture-validation`: `load_model()` reads and merges all `models/*.c4` files. All other validation logic unchanged.
- `architecture-commit-gate`: `model_exists()` checks for `models/` directory instead of `model.c4` file.

## Impact

- `plugins/dev-team/agents/architecture.md` (new)
- `plugins/dev-team/skills/update-architecture/SKILL.md` (new)
- `plugins/dev-team/.claude-plugin/plugin.json` (add subagent + skill declarations)
- `plugins/dev-team/utils/archi-model.py` (single file → directory aggregation, write requires `--path`)
- `plugins/dev-team/utils/archi-validate.py` (`load_model()` reads directory)
- `plugins/dev-team/hooks/commit-gates/architecture.py` (`model_exists()` checks `models/`)
- `openspec/architecture/model.c4` → `openspec/architecture/models/` (migration)
