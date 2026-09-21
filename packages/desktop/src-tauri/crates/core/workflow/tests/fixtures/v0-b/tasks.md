## 1. Artifact Templates & Schemas

- [x] 1.1 Create `templates/artifacts/proposal.md.template` — Problem, Stakeholders, Scope (in_scope/out_of_scope), Risks (with mitigations), Acceptance Criteria (with validation_method)
- [x] 1.2 Create `templates/artifacts/test-design.md.template` — Test Levels, Coverage Map, Test Strategy, Boundary Cases
- [x] 1.3 Create `templates/artifacts/design.md.template` — Architecture Components, Data Flow, Route Design, Decisions (with rationale and alternatives)
- [x] 1.4 Create `templates/artifacts/eval.schema.json` — verdict, items[{item_id, pass, evidence, notes}], backtrack_to
- [x] 1.5 Create `templates/artifacts/checklist.schema.json` — reference format: id, criterion, required, evidence_hint

## 2. Planner Agents (.md output, opus)

- [x] 2.1 Create `agents/requirements-planner.md` — writes proposal.md following template
- [x] 2.2 Create `agents/test-design-planner.md` — reads proposal.md, writes test-design.md following template
- [x] 2.3 Create `agents/dev-proposal-planner.md` — reads proposal.md + test-design.md, writes design.md + tasks.md

## 3. Generator Agents (code output, sonnet)

- [x] 3.1 Create `agents/test-gen-generator.md` — reads test-design.md, writes test files to disk (git diff IS the artifact, no JSON)
- [x] 3.2 Create `agents/implementation-generator.md` — reads design.md + tasks.md, writes implementation code to disk (git diff IS the artifact, no JSON)

## 4. Evaluator Agents (static checklist, append to eval.json, opus)

- [x] 4.1 Create `agents/requirements-evaluator.md` — static checklist for proposal.md completeness/clarity/coverage; Read/Write only
- [x] 4.2 Create `agents/test-design-evaluator.md` — static checklist for test-design.md vs proposal.md; Read/Write only
- [x] 4.3 Create `agents/dev-proposal-evaluator.md` — static checklist for design.md completeness/decisions; Read/Write only
- [x] 4.4 Create `agents/test-gen-evaluator.md` — static checklist for generated test code (via git diff) vs test-design.md; Read/Write/Bash
- [x] 4.5 Create `agents/implementation-evaluator.md` — static checklist for implementation code (via git diff) vs design.md; Read/Write/Bash
- [x] 4.6 Create `agents/code-review-evaluator.md` — static checklist for code diff vs design.md (security, tests, error handling); Read/Write/Grep/Glob/Bash
- [x] 4.7 Create `agents/acceptance-evaluator.md` — static checklist for codebase vs proposal.md (req traceability, no scope creep); Read/Write/Grep/Glob/Bash

## 5. Phase Skills (thin orchestrators)

- [x] 5.1 Create `skills/phase-requirements/SKILL.md` — P→E loop (Planner writes proposal.md, Evaluator checks, loop on fail)
- [x] 5.2 Create `skills/phase-test-design/SKILL.md` — P→E loop (Planner writes test-design.md, Evaluator checks, loop on fail)
- [x] 5.3 Create `skills/phase-dev-proposal/SKILL.md` — P→E loop (Planner writes design.md, Evaluator checks, loop on fail)
- [x] 5.4 Create `skills/phase-test-gen/SKILL.md` — G→E loop (Generator writes test files to disk, Evaluator inspects git diff, loop on fail)
- [x] 5.5 Create `skills/phase-implement/SKILL.md` — G→E loop with AUTO trigger (Generator writes implementation code, static-check + test-execution auto-run, Evaluator inspects git diff)
- [x] 5.6 Create `skills/phase-code-review/SKILL.md` — E only (Evaluator inspects code diff directly, no Generator)
- [x] 5.7 Create `skills/phase-acceptance/SKILL.md` — E only (Evaluator traces requirements in codebase, backtrack support)

## 6. Archive Script & Flow

- [x] 6.1 Create `utils/eval-check.py` — read eval.json, extract latest per phase by timestamp, verify all phases pass (P1-P5, P7, P9), verify no gaps, verify all tasks.md items marked [x]; exit 0 on pass, non-zero on fail
- [x] 6.2 Create archive flow orchestration — sequential: eval-check.py → openspec archive → git commit; abort on first failure
- [x] 6.3 Update `hooks/pre-tool-skill.py` — handle 7 new phase skills + archive flow, EVALUATOR-ONLY routing (no Generator)
- [x] 6.4 Remove `hooks/commit-gates/before-commit.py` — replaced by eval check script
- [x] 6.5 Remove `hooks/commit-gates/quality.py` — replaced by Evaluator agents + eval check script
- [x] 6.6 Remove `hooks/commit-gates/architecture.py` — replaced by architecture Evaluator checks
- [x] 6.7 Update `hooks/pre-tool-openspec-test.py` — handle .md design artifacts for TDD mapping

## 7. Migration & Deprecation

- [x] 7.1 Update `skills/openspec-propose/SKILL.md` — migration message pointing to phase-requirements + phase-dev-proposal
- [x] 7.2 Update `skills/openspec-apply-change/SKILL.md` — migration message pointing to phase-implement
- [x] 7.3 Update `skills/code-review/SKILL.md` — migration message pointing to phase-code-review

## 8. Validation & Testing

- [x] 8.1 Test DESIGN P→E: Planner writes .md, Evaluator checks with static checklist, appends to eval.json
- [x] 8.2 Test DESIGN P→E fail → loop back: Planner re-invoked with failed items, eval.json preserves both attempts
- [x] 8.3 Test EXECUTION G→E: Generator writes code to disk, Evaluator inspects git diff + prior .md artifact, appends to eval.json
- [x] 8.4 Test EXECUTION G→E fail → loop back: Generator re-invoked with failed items
- [x] 8.5 Test EVALUATOR-ONLY code-review: Evaluator inspects code diff against design.md, appends findings to eval.json
- [x] 8.6 Test EVALUATOR-ONLY acceptance: Evaluator traces requirements from proposal.md → code, detects gaps
- [x] 8.7 Test backtrack: acceptance detects gap → backtrack_to requirements → requirements re-eval clears marker
- [x] 8.8 Test AUTO phases trigger after implementation Generator completes
- [x] 8.9 Test eval check script: all phases pass → exit 0; missing phase → exit 1; failed phase → exit 1
- [x] 8.10 Test eval check script: incomplete tasks.md → exit 1; all tasks [x] → passes
- [x] 8.11 Test archive flow: eval check fail → archive aborted; eval check pass → archive + commit proceed
- [x] 8.12 Test deprecated skill migration messages render correctly
