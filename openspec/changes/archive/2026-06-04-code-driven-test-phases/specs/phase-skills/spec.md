# phase-skills Specification

## ADDED Requirements

### Requirement: phase-test-design skill delegates scenario-oriented behavior to agent
The phase-test-design skill SHALL remain a thin P→E orchestrator. All Forward/Reverse AC categorization logic SHALL reside in the test-design-planner agent definition, not in the skill itself. The planner SHALL grep source code to extract real API signatures (function names, parameter types, return types) as supplementary input alongside proposal.md and design.md, but SHALL NOT output parameter types or risk markers in test-design.md.

#### Scenario: phase-test-design prompt unchanged, agent respects no-type constraint
- **WHEN** phase-test-design invokes test-design-planner
- **THEN** the skill uses the existing one-line prompt: `"Write test-design.md for change '<name>'."`
- **AND** the agent greps source code to extract real API signatures as supplementary input
- **AND** the agent does NOT output parameter type tables or risk markers in test-design.md
- **AND** the skill does NOT pass any additional source-file-related context in the prompt

#### Scenario: phase-test-design gate check remains unchanged
- **WHEN** phase-test-design executes
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__phase_check` with `change` and `phase="03-test-design"`
- **AND** the gate check logic is unaffected by the agent-level changes

### Requirement: phase-test-gen skill delegates colocated output and edge case generation to agent
The phase-test-gen skill SHALL remain a thin G→E orchestrator. All source code reading, test file colocation, and edge case generation logic SHALL reside in the test-gen-generator agent definition, not in the skill itself.

#### Scenario: phase-test-gen prompt unchanged with code-aware generator
- **WHEN** phase-test-gen invokes test-gen-generator
- **THEN** the skill uses the existing one-line prompt: `"Generate test files for change '<name>'."`
- **AND** the agent handles all source code reading, colocated file placement, and edge case generation internally
- **AND** the skill does NOT specify an output directory in the prompt

#### Scenario: phase-test-gen evaluator checklist driven by agent definition
- **WHEN** phase-test-gen invokes test-gen-evaluator
- **THEN** the skill uses the existing one-line prompt: `"Evaluate generated test code for change '<name>' against test-design.md. Append result to eval.json."`
- **AND** the evaluator reads its updated checklist (G1, G2) from the agent definition
- **AND** the skill does NOT hardcode checklist items

#### Scenario: phase-test-gen gate check remains unchanged
- **WHEN** phase-test-gen executes
- **THEN** the skill first calls `mcp__plugin_dev-team_dev-team__phase_check` with `change` and `phase="04-test-gen"`
- **AND** the gate check logic is unaffected by the agent-level changes

## Module Contract

### Skill Files (`plugins/dev-team/skills/`)

| Skill | Status | Contract |
|-------|--------|----------|
| phase-test-design | UNCHANGED (compatible) | P→E loop, phase `03-test-design`. Agent definition handles Forward/Reverse AC categorization from proposal.md + design.md, supplemented by Grep source code for API signatures. Prompt, gate check, evaluator invocation, and verdict loop are unchanged. |
| phase-test-gen | UNCHANGED (compatible) | G→E loop, phase `04-test-gen`. Agent definition now handles colocated output and edge case generation. Prompt, gate check, evaluator invocation, and verdict loop are unchanged. |

### Design Rationale

The thin orchestrator pattern ensures that behavioral changes to test phases are contained within agent definitions rather than propagated to skill files. This keeps the skill layer stable and allows agent-level iteration without modifying the orchestration. Both skills pass generic one-line prompts; all domain logic about Forward/Reverse AC categorization (test-design), source code reading and parameter type extraction (test-gen), test file placement (test-gen), and edge case generation (test-gen) lives in the respective agent definitions.
