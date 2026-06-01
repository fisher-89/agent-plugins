/**
 * PGE workflow phase configuration — single source of truth for all phase definitions.
 *
 * This is the only file that defines phase ordering, phase patterns
 * (DESIGN/EXEC/EVAL-ONLY), agent assignments, and prompt templates.
 * All other modules (eval-next, eval-check, eval-log) import from here.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PhaseAgentDef {
  agent_type: string;
  prompt: string;
}

export type PhasePattern = 'DESIGN' | 'EXEC' | 'EVAL-ONLY';

export interface PhaseDefinition {
  id: string;
  pattern: PhasePattern;
  planner: PhaseAgentDef | null;
  evaluator: PhaseAgentDef | null;
  auto_steps: string[];
}

// ---------------------------------------------------------------------------
// Phase Tables — defined per workflow_type
// ---------------------------------------------------------------------------

const DEFAULT_WORKFLOW: string = 'requirement';

const PHASE_REQUIREMENT: PhaseDefinition[] = [
  {
    id: '01-proposal',
    pattern: 'DESIGN',
    planner: {
      agent_type: 'dev-team:proposal-planner',
      prompt: "Write proposal.md and specs/ for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:proposal-evaluator',
      prompt: "Evaluate proposal.md for change '<change>' against checklist.",
    },
    auto_steps: [],
  },
  {
    id: '02-dev-design',
    pattern: 'DESIGN',
    planner: {
      agent_type: 'dev-team:dev-design-planner',
      prompt: "Write design.md and tasks.md for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:dev-design-evaluator',
      prompt:
        "Evaluate design.md and tasks.md for change '<change>' against proposal.md. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '03-test-design',
    pattern: 'DESIGN',
    planner: {
      agent_type: 'dev-team:test-design-planner',
      prompt: "Write test design for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:test-design-evaluator',
      prompt:
        "Evaluate test design for change '<change>' against design.md. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '04-test-gen',
    pattern: 'EXEC',
    planner: {
      agent_type: 'dev-team:test-gen-generator',
      prompt: "Generate test code for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:test-gen-evaluator',
      prompt: "Evaluate generated tests for change '<change>'. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '05-implement',
    pattern: 'EXEC',
    planner: {
      agent_type: 'dev-team:implementation-generator',
      prompt: "Implement the code for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:implementation-evaluator',
      prompt:
        "Evaluate implementation for change '<change>' against design. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '06-unit-test',
    pattern: 'EXEC',
    planner: {
      agent_type: 'dev-team:unit-test-executor',
      prompt: "Run and fix unit tests for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:unit-test-evaluator',
      prompt:
        "Evaluate unit test results for change '<change>'. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '07-code-review',
    pattern: 'EVAL-ONLY',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:code-review-evaluator',
      prompt:
        "Perform code review for change '<change>'. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '08-integration-test',
    pattern: 'EXEC',
    planner: {
      agent_type: 'dev-team:integration-test-executor',
      prompt: "Run and fix integration tests for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:integration-test-evaluator',
      prompt:
        "Evaluate integration test results for change '<change>'. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '09-acceptance',
    pattern: 'EVAL-ONLY',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:acceptance-evaluator',
      prompt: "Perform acceptance evaluation for change '<change>'. Append result to eval.json.",
    },
    auto_steps: [],
  },
];

const PHASE_BUG_FIX: PhaseDefinition[] = [
  {
    id: '01-proposal',
    pattern: 'DESIGN',
    planner: {
      agent_type: 'dev-team:proposal-planner',
      prompt: "Write proposal.md and specs/ for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:proposal-evaluator',
      prompt: "Evaluate proposal.md for change '<change>' against checklist.",
    },
    auto_steps: [],
  },
  {
    id: '02-dev-design',
    pattern: 'DESIGN',
    planner: {
      agent_type: 'dev-team:dev-design-planner',
      prompt: "Write design.md and tasks.md for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:dev-design-evaluator',
      prompt:
        "Evaluate design.md and tasks.md for change '<change>' against proposal.md. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '05-implement',
    pattern: 'EXEC',
    planner: {
      agent_type: 'dev-team:implementation-generator',
      prompt: "Implement the code for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:implementation-evaluator',
      prompt:
        "Evaluate implementation for change '<change>' against design. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '06-unit-test',
    pattern: 'EXEC',
    planner: {
      agent_type: 'dev-team:unit-test-executor',
      prompt: "Run and fix unit tests for change '<change>'.",
    },
    evaluator: {
      agent_type: 'dev-team:unit-test-evaluator',
      prompt:
        "Evaluate unit test results for change '<change>'. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '07-code-review',
    pattern: 'EVAL-ONLY',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:code-review-evaluator',
      prompt:
        "Perform code review for change '<change>'. Append result to eval.json.",
    },
    auto_steps: [],
  },
  {
    id: '09-acceptance',
    pattern: 'EVAL-ONLY',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:acceptance-evaluator',
      prompt: "Perform acceptance evaluation for change '<change>'. Append result to eval.json.",
    },
    auto_steps: [],
  },
];

const PHASE_REFACTOR: PhaseDefinition[] = PHASE_REQUIREMENT;

export const PHASE_TABLES: Record<string, PhaseDefinition[]> = {
  requirement: PHASE_REQUIREMENT,
  'bug-fix': PHASE_BUG_FIX,
  refactor: PHASE_REFACTOR,
};

// ---------------------------------------------------------------------------
// Derived: ordered phase ID list (requirement workflow is canonical)
// ---------------------------------------------------------------------------

/**
 * Ordered list of PGE workflow phases.
 * Derived from PHASE_REQUIREMENT — the single source of truth.
 */
export const PHASES: readonly string[] = PHASE_REQUIREMENT.map((p) => p.id) as const;

// ---------------------------------------------------------------------------
// Accessors
// ---------------------------------------------------------------------------

/**
 * Return the phase table for the given workflow_type.
 * Defaults to "requirement" if unknown.
 */
export function getPhaseTable(workflowType?: string): PhaseDefinition[] {
  const key = (workflowType || DEFAULT_WORKFLOW).toLowerCase();
  return PHASE_TABLES[key] || PHASE_TABLES[DEFAULT_WORKFLOW];
}

/**
 * Return the phase pattern for a given phase ID.
 * Returns null if the phase is not in the requirement table.
 */
export function getPhasePattern(phaseId: string): PhasePattern | null {
  const table = getPhaseTable('requirement');
  const phase = table.find((p) => p.id === phaseId);
  return phase ? phase.pattern : null;
}

/**
 * Return the index of a phase in the PHASES array.
 * Returns -1 if the phase is not found.
 */
export function getPhaseIndex(phase: string): number {
  return PHASES.indexOf(phase);
}

/**
 * Return the list of phases that come before the given phase.
 * - If phase is the first phase, returns an empty array.
 * - If phase is not in PHASES, returns an empty array (fault-tolerant).
 */
export function getPriorPhases(phase: string): string[] {
  const idx = getPhaseIndex(phase);
  if (idx <= 0) return [];
  return PHASES.slice(0, idx);
}
