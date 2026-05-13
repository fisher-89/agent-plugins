## Context

Currently, `archi-model.py`, `archi-validate.py`, and `archi-decide.py` provide architecture primitives but require the calling agent to know when and how to invoke them. The code-review subagent pattern (`agents/code-review.md` + `skills/code-review/SKILL.md`) proves that routing skills to specialized subagents works well. We extend this pattern to architecture.

The architecture model currently lives in a single `model.c4` file. As systems grow, splitting into a directory tree (`models/*.c4`) allows independent management of subdomains.

## Goals / Non-Goals

**Goals:**
- Provide an LLM agent that orchestrates `archi-model.py`, `archi-validate.py`, `archi-decide.py` intelligently
- Agent always proposes before writing — never modifies model autonomously
- Move model storage from single `model.c4` to `models/*.c4` directory tree
- Provide a user-invocable-only skill (`disable-model-invocation: true`)
- Keep existing Python utils as the authoritative implementation layer

**Non-Goals:**
- Agent does NOT auto-write model files (always propose→confirm)
- Agent does NOT replace the commit gate's automated validation
- No changes to ADR format or `archi-decide.py` logic
- No changes to C4 DSL syntax

## Decisions

### 1. Agent orchestration: LLM reads code + Python validates/writes

The agent uses `Read`/`Grep`/`Glob` to understand the codebase, then calls `archi-model.py` for DSL validation/writing and `archi-validate.py` for cross-referencing. Python utils remain the single source of truth for all model operations.

**Alternatives considered:**
- Let the agent write DSL directly with `Write` tool → rejected: no structural validation before write
- Embed DSL knowledge entirely in the agent prompt → rejected: Python utils already exist and are tested

### 2. Directory tree structure: numbered prefix for deterministic ordering

Files in `models/` use optional numeric prefixes (`01-core.c4`, `02-api-layer.c4`) so that `archi-model.py` can aggregate in a stable order. The prefix is a convention, not enforced — files are sorted alphabetically.

```
openspec/architecture/
├── models/
│   ├── 01-core.c4            # specification + top-level model skeleton
│   │                         # always loaded first, defines element kinds
│   └── 02-services.c4        # additional elements (optional)
├── decisions/                # unchanged
└── reports/                  # unchanged
```

**Why 01-core.c4 holds `specification {}`:** The `specification` block defines element kinds and must appear exactly once. By convention it lives in the first-loaded file. `archi-model.py` parses `specification` only from the first file that contains it; duplicates are an error.

### 3. Two-phase interaction model

```
PROPOSE phase (always):
  Agent reads code → drafts DSL → validates via archi-model.py validate --source
  → presents diff to user with explanation → WAITS

WRITE phase (on confirmation):
  User says "yes/write/do it" → Agent calls archi-model.py write --path ... --source
```

The agent prompt explicitly forbids writing without user confirmation. The `Write` tool is in the agent's tool list but the prompt instructs it to only use `archi-model.py write`.

### 4. Skill routing: same pattern as code-review

```markdown
---
name: update-architecture
description: ...
disable-model-invocation: true
---

Use the Agent tool to spawn the architecture subagent:
Agent({
  description: "...",
  subagent_type: "architecture",
  prompt: "<tailored prompt>"
})
```

**Why `disable-model-invocation: true`:** Architecture changes are sensitive. Only user-initiated invocations. The model will not suggest this skill proactively.

### 5. archi-model.py API changes

| Command | Before | After |
|---------|--------|-------|
| `query` | reads `model.c4` | aggregates `models/*.c4` |
| `validate` | reads `model.c4` or `--source` | aggregates `models/*.c4` or `--source` |
| `write` | writes `model.c4` | requires `--path models/xx.c4`, writes single file |

New `--path` flag for write is a **BREAKING** change. The only consumer is the architecture agent, which doesn't exist yet, so no backward-compat needed.

### 6. archi-validate.py load_model() changes

`load_model()` currently reads `openspec/architecture/model.c4`. After:
1. Check `openspec/architecture/models/` directory
2. Read all `*.c4` files in alphabetical order
3. Concatenate, then parse with existing `_parse_model_dsl()`
4. If `specification` appears in multiple files, warn and use the first

## Risks / Trade-offs

- [Risk] `archi-model.py write --path` could overwrite user changes → Agent proposes diff first, user confirms
- [Risk] Splitting model into multiple files could cause cross-file reference errors → `validate` command checks the aggregated DSL
- [Risk] Existing `model.c4` files in user projects break → commit gate `model_exists()` checks both `model.c4` and `models/` during transition; `archi-model.py` prefers `models/` but falls back to `model.c4` with a deprecation warning
- [Risk] Model becomes too "smart" and over-eager → `disable-model-invocation: true` on the skill prevents the model from suggesting it

## Open Questions

None — all decisions resolved during exploration.
