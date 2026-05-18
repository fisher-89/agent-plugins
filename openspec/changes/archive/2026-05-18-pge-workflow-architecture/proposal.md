## Why

The current plugin uses a linear report-driven gate system: each SDD step generates a report with a pass/fail status bit, and hooks check that reports exist in order. There is no adversarial quality control — nothing evaluates whether the generated artifact actually meets the intent. A proposal can be incoherent, a design can miss critical constraints, and tests can have gaps, all while reports show `pass`. Introducing a Planner-Generator-Evaluator (PGE) pattern at each phase creates an adversarial quality loop: every artifact is evaluated against a static binary checklist before the workflow can proceed.

## What Changes

- **BREAKING**: Replace markdown-based design artifacts (proposal.md, design.md) with .md templates for Planner phases; Generator phases produce code directly on disk (git diff IS the artifact, no JSON report)
- **BREAKING**: Replace flat skill set with 7 phase-gated skills: `phase-requirements`, `phase-test-design`, `phase-dev-proposal`, `phase-test-gen`, `phase-implement`, `phase-code-review`, `phase-acceptance`
- Add 12 new agent definitions: 3 Planners (write .md artifacts directly, no checklist generation), 2 Generators (test-gen, implementation — write code directly to disk, git diff IS the artifact), 7 Evaluators (each with static checklist embedded in prompt — 3 document evaluators Read/Write only, 2 code evaluators with Bash, 2 with full codebase access)
- Three phase patterns: DESIGN uses P→E (Planner writes .md, Evaluator checks with static checklist), EXECUTION uses G→E (Generator writes code to disk, git diff IS the artifact, Evaluator checks code against prior DESIGN .md), EVALUATOR-ONLY uses E (Evaluator inspects code/docs directly, no Generator)
- Add static binary checklist evaluation: each Evaluator has a phase-specific checklist embedded in its agent prompt. No Planner-generated checklists — evaluation criteria are stable and per-phase
- All evaluators append to a single `eval.json` array with timestamp field — full iteration history preserved
- Process evaluation replaced by deterministic Python script (`utils/eval-check.py`) that reads eval.json and validates all phases passed
- Add archive flow (non-PGE): eval check script → openspec archive → git commit, replacing commit hook gates
- Add backtrack support: acceptance-phase Evaluator can trigger re-evaluation of prior phases when requirements gaps are found
- Preserve auto-trigger for static-check and test-execution phases (no PGE, run automatically after implementation)
- Preserve existing architecture agent and model utilities unchanged
- Remove commit gate hooks (`before-commit.py`, `quality.py`, `architecture.py`)

## Capabilities

### New Capabilities

- `pge-workflow-engine`: Core PGE loop — DESIGN phases use P→E (Planner writes .md artifact), EXECUTION phases use G→E (Generator writes code to disk, git diff IS the artifact), EVALUATOR-ONLY phases use E (Evaluator inspects codebase directly). All Evaluators append to a single eval.json array. Archive flow executes sequentially (eval check → archive → commit).
- `phase-agents`: 12 specialized agents (3 Planner, 2 Generator, 7 Evaluator). Planners write .md; Generators write code to disk (git diff IS the artifact, no JSON reports); Evaluators have static checklists. Context isolation means agents cannot read each other's reasoning chains — only artifacts and (for E4-E7) codebase via git diff or direct inspection.
- `json-design-schemas`: JSON schemas for shared evaluation formats (eval.json entries, checklist.json). .md templates for Planner outputs (proposal.md, test-design.md, design.md). Generator outputs are code on disk, not JSON — no Generator schemas needed.
- `phase-skills`: 7 user-triggered skills mapping to the pipeline (3 DESIGN with P→E, 2 EXEC with G→E, 2 EVALUATOR-ONLY with E, 2 AUTO)
- `pipeline-backtrack`: Mechanism for later-phase Evaluators (acceptance, code-review) to mark `backtrack_to` in their eval entry, triggering prior-phase re-evaluation on next skill invocation

### Modified Capabilities

- `intent-router`: Existing skill routing logic extended to route the 7 new phase skills to their Planner/Generator/Evaluator agent chains
- `hook-output-import`: Hook output formats eval status from eval.json; archive flow replaces commit gate hooks

## Impact

- All agent definitions in `plugins/dev-team/agents/` (12 new, 1 modified)
- All skill definitions in `plugins/dev-team/skills/` (7 new, 3 existing deprecated)
- New utility script: `plugins/dev-team/utils/eval-check.py` (archive eval check)
- Removed: `plugins/dev-team/hooks/commit-gates/before-commit.py`, `quality.py`, `architecture.py`
- Template files: new .md templates and JSON schemas in `plugins/dev-team/templates/artifacts/`
- User workflow: skill invocation changes from `/openspec-propose` → `/phase-requirements` + `/phase-dev-proposal`
