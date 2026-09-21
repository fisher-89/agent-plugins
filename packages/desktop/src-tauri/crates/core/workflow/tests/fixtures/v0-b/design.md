## Context

The current plugin (`dev-team` v2.1.5) implements a linear SDD workflow: explore → propose → apply-change → archive. Quality gates are report-driven — hooks check that JSON reports exist in `reports/` with `status: pass`, but no one evaluates whether the artifact content meets requirements. The agent architecture is flat (2 agents: code-review, architecture), and skills are simple routing wrappers around the OpenSpec CLI.

The user wants a pipeline where each phase has adversarial quality control. DESIGN phases use P→E (Planner writes .md artifact directly), EXECUTION phases use G→E (Generator writes code to disk, git diff IS the artifact), and EVALUATOR-ONLY phases have just an Evaluator (no Planner, no Generator). AUTO phases (static-check, test-execution) have no PGE overhead. Archive is a non-PGE sequential flow (eval check → archive → commit).

## Goals / Non-Goals

**Goals:**

- Implement PGE adversarial loops: 3 DESIGN (P→E), 2 EXECUTION (G→E), 2 EVALUATOR-ONLY (E)
- Planner outputs are .md with suggested templates (human-authored design docs)
- Generator outputs are code changes on disk — git diff IS the artifact (no JSON report overhead)
- Evaluators use static checklists embedded in agent prompts (no generated checklists)
- All evaluators append to a single eval.json array with timestamps for full audit trail
- Provide 7 user-visible skills to drive the pipeline manually
- Support backtracking from later phases (acceptance, code-review) to prior phases
- Evaluator-only phases (code-review, acceptance) have access to code/docs, not just isolated artifact pairs
- Archive flow executes sequentially: eval check script → openspec archive → git commit
- Replace commit hook gates with Evaluator agents + eval check script

**Non-Goals:**

- Full automation of the pipeline (user manually triggers each phase)
- Real-time collaborative evaluation
- Phase-specific adversarial intensity tuning (start with standard, tune later)
- Changing the architecture model DSL or validation engine
- Replacing the OpenSpec CLI — skills remain thin routing wrappers

## Decisions

### D1: Hybrid artifact formats — .md for design, .json for reports

Planner agents produce markdown artifacts (proposal.md, test-design.md, design.md) following suggested templates in `templates/artifacts/`. Markdown fits design documents — LLMs write natural prose better than structured JSON, and humans read design docs. Generator agents (test-gen, implementation) produce code changes directly on disk — the git diff of uncommitted changes IS the artifact. No intermediate JSON report: the code is the ground truth, and describing it in JSON adds no value.

**Alternatives considered**: All-JSON (constrains Planner creativity), Generator JSON reports (redundant — the code already describes itself). Hybrid markdown + git-diff chosen because DESIGN phases benefit from freeform expression while EXECUTION output is self-documenting code.

### D2: 12-agent PGE architecture

Three phase types, three patterns:

- **DESIGN phases** (requirements, test-design, dev-proposal): Planner → Evaluator. Planner writes .md artifact directly (no separate Generator needed — the design work IS the artifact).
- **EXECUTION phases** (test-gen, implementation): Generator → Evaluator. Generator writes code directly to disk (test files, implementation changes). The git diff of uncommitted changes IS the artifact. Evaluator inspects git diff + prior DESIGN .md artifact.
- **EVALUATOR-ONLY phases** (code-review, acceptance): Evaluator only. No Generator — the evaluation IS the work. Evaluator inspects codebase/diffs/reports directly and appends result to eval.json.

Phase-specific behavior lives in agent prompts and static checklists, not in code. The skill SKILL.md orchestrates the loop.

**Alternatives considered**: Generator for all phases (unnecessary indirection — code review doesn't need a "review generator" when the evaluator can review directly). Python PGE engine (would require more maintenance, harder to iterate on prompt engineering).

### D3: Static binary checklists per Evaluator

Every Evaluator has a static binary checklist embedded in its agent prompt — no Planner-generated checklist JSON. Each checklist is designed for its phase:

- DESIGN evaluators (E1-E3): check .md artifact completeness, clarity, coverage
- EXECUTION evaluators (E4-E5): check generated code (via git diff) against prior .md artifact for consistency and completeness
- EVALUATOR-ONLY evaluators (E6-E7): check code/diffs/reports directly against design/proposal artifacts

All evaluators use the same output format: `{verdict: "pass"|"fail", items: [{item_id, pass, evidence, notes}], backtrack_to: string|null}`. If any required item fails → `verdict: fail`, loop back to the producing agent (Planner for DESIGN, Generator for EXECUTION). For EVALUATOR-ONLY phases, fail triggers backtrack.

**Alternatives considered**: Planner-generated checklists (adds complexity — Planner must predict evaluation criteria; static checklists are simpler and ensure consistent evaluation across runs). Rubric scoring (too subjective). Binary checklist chosen because it's deterministic, debuggable, and gives clear go/no-go.

### D4: Context isolation — no reasoning leakage

Evaluator context isolation means: no Evaluator can read another agent's reasoning chain, conversation history, or tool-call details. Evaluators only consume:

- The artifact being evaluated (.md or .json file)
- For EXECUTION phases: the prior DESIGN .md artifact + git diff output (uncommitted code changes)
- For EVALUATOR-ONLY phases: codebase access (code diffs, project files, reports)

This prevents groupthink while allowing evaluators to verify claims against the actual code and design documents. DESIGN evaluators (E1-E3) only need Read/Write — they evaluate documents, not code. EXECUTION evaluators (E4-E5) need Read/Write/Bash — they run git diff to inspect the Generator's code output. EVALUATOR-ONLY evaluators (E6-E7) need Read/Write/Grep/Glob/Bash — they inspect the entire codebase.

### D5: Linear pipeline with explicit backtrack points

The PGE phases execute linearly (7 phases: P1-P5, P7, P9). Backtrack is allowed at two specific points:

- P9 (acceptance) → can trigger re-evaluation of P1 (requirements)
- P7 (code-review) → can trigger re-evaluation of P3 (dev-proposal)

Backtrack is implemented via a `backtrack_to` field in the eval JSON. The skill for the target phase checks for backtrack markers and re-runs its Evaluator before allowing progression.

After all PGE phases complete, the archive flow runs sequentially (non-PGE): eval check script → openspec archive → git commit.

### D6: Agent model assignment

Planner agents use `opus` (complex reasoning about design completeness). Generator agents use `sonnet` (good at producing structured output quickly). Evaluator agents use `opus` (careful evaluation against checklists requires strong reasoning).

### D7: eval.json as append-only array

All 7 Evaluators append results to a single `openspec/changes/<name>/phases/eval.json` file as an array. Each entry includes `phase`, `timestamp` (ISO 8601), `attempt` (1-based, incrementing on re-evaluation), `verdict`, `report` (human-readable summary ≤500 chars), `items`, and `backtrack_to`. Previous entries are never overwritten — the full iteration history is preserved. The "latest" result per phase is determined by maximum `timestamp`.

**Rationale**: A single file is easier to parse than 7 separate files. Append-only preserves the full audit trail (how many attempts each phase took, what failed). The timestamp field provides unambiguous ordering.

### D8: Process evaluation replaced by eval check script

The process-evaluator agent (E8) is replaced by a deterministic Python script (`utils/eval-check.py`). The script reads eval.json, extracts the latest entry per phase by timestamp, validates all required phases have verdict "pass" with no gaps, and checks that all tasks in tasks.md are marked complete (`[x]`).

**Rationale**: Process evaluation is mechanical — "did all phases pass?" — not qualitative. A script is faster (milliseconds vs seconds), cheaper (no LLM cost), and deterministic. The qualitative evaluation already happened in each phase's Evaluator.

### D9: Archive flow replaces commit hooks

The archive flow executes sequentially: (1) eval check script, (2) openspec archive, (3) git commit. If step 1 fails, steps 2-3 do not execute. All commit-time hook gates (`before-commit.py`, `quality.py`, `architecture.py`) are removed. Code review and compliance checks are now performed by Evaluator agents (E6, E7) during PGE phases, not by commit hooks.

**Rationale**: Commit gates duplicate evaluation work already done by Evaluator agents. Moving validation into PGE phases provides richer feedback (evidence, backtrack_to) than binary pass/fail hooks. The archive flow bundles commit as the final step, making C8 (no uncommitted) intentionally removed — we expect uncommitted changes at archive time.

## Risks / Trade-offs

- [Complexity] 12 new agents + 7 new skills → Mitigation: All agents follow the same patterns (markdown template → eval JSON for DESIGN, code diff → eval JSON for EXECUTION). Skill files are thin orchestrators.
- [LLM variance] Same Evaluator may produce different results on re-run → Mitigation: Static binary checklist format constrains output to pass/fail. JSON schema validation catches malformed eval outputs.
- [User friction] 7 manual skill invocations per change → Mitigation: Each skill invocation is a meaningful phase gate. Users can skip phases for trivial changes by invoking skills out of order.
- [Prompt engineering burden] Each agent needs carefully tuned prompts, and each Evaluator needs a phase-specific static checklist → Mitigation: Checklist items are independent and testable. Shared eval output format across all evaluators.
- [Checklist staleness] Static checklists may drift from project needs → Mitigation: Checklists live in agent .md files in `agents/`, editable alongside code. Future iteration can tune checklist items without changing pipeline logic.

## Migration Plan

1. Create new agent definitions alongside existing ones (no deletion)
2. Create new skill SKILL.md files
3. Create .md templates in `templates/artifacts/` for Planner phases
4. Create eval check script at `utils/eval-check.py`
5. Deprecate old skills (`openspec-propose`, `openspec-apply-change`) with migration messages pointing to new phase skills
6. Remove commit gate hooks (`before-commit.py`, `quality.py`, `architecture.py`) — replaced by Evaluator agents and eval check script
7. Existing changes using old workflow complete normally — no schema migration
8. Old skills removed in a follow-up cleanup change
