## 1. Update archi-model.py for directory tree

- [x] 1.1 Update `read_model()` to aggregate all `models/*.c4` files in alphabetical order
- [x] 1.2 Add deprecation warning when reading from legacy `model.c4`
- [x] 1.3 Update `write_dsl()` to require `--path` and reject paths outside `models/`
- [x] 1.4 Add duplicate `specification {}` block detection in `_validate_structure()`
- [x] 1.5 Update `query_model()` to read from aggregated files
- [x] 1.6 Ensure backward compat: if `models/` absent, fall back to `model.c4` with deprecation warning

## 2. Update archi-validate.py for directory tree

- [x] 2.1 Update `load_model()` to read all `models/*.c4` in alphabetical order
- [x] 2.2 Add backward compat: fall back to `model.c4` if `models/` absent
- [x] 2.3 Verify `model_changes` detection scans `models/` directory

## 3. Update architecture commit gate

- [x] 3.1 Update `model_exists()` to check `models/` directory for `*.c4` files
- [x] 3.2 Add backward compat: also check legacy `model.c4`
- [x] 3.3 Update deny message to reference `models/` paths

## 4. Create architecture subagent definition

- [x] 4.1 Create `plugins/dev-team/agents/architecture.md` with frontmatter (name, description, model: opus, tools)
- [x] 4.2 Write agent instructions for PROPOSE mode: read models, grep code, draft DSL, validate, present diff, wait for confirmation
- [x] 4.3 Write agent instructions for VALIDATE mode: run archi-validate.py, explain violations
- [x] 4.4 Write agent instructions for DECIDE mode: help draft ADR via archi-decide.py
- [x] 4.5 Write agent instructions for REVIEW mode: read all models, critique completeness/consistency/coupling
- [x] 4.6 Add explicit guard: agent MUST NOT write model files without user confirmation

## 5. Create update-architecture skill

- [x] 5.1 Create `plugins/dev-team/skills/update-architecture/SKILL.md` with `disable-model-invocation: true`
- [x] 5.2 Write thin routing instructions: parse user intent, invoke `Agent({subagent_type: "architecture", ...})`

## 6. Update plugin manifest and templates

- [x] 6.1 Register `update-architecture` skill and `architecture` subagent in plugin.json (if needed)
- [x] 6.2 Migrate `templates/model.c4` to `openspec/architecture/models/01-core.c4` in demo-project or docs
