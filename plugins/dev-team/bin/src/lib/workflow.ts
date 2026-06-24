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

interface PhaseAgentDef {
  agent_type: string;
  prompt: string;
}

export interface PhaseDefinition {
  id: string;
  description: string;
  planner: PhaseAgentDef | null;
  evaluator: PhaseAgentDef | null;
}

// ---------------------------------------------------------------------------
// Phase Tables — defined per workflow_type
// ---------------------------------------------------------------------------

const DEFAULT_WORKFLOW: string = 'requirement';

const PHASE_REQUIREMENT: PhaseDefinition[] = [
  {
    id: '01-proposal',
    description: '需求提案与规格说明',
    planner: {
      agent_type: 'dev-team:proposal-planner',
      prompt: 'Write proposal.md and specs/ for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:proposal-evaluator',
      prompt: 'Evaluate proposal.md for change "<change>" against checklist.',
    },
  },
  {
    id: '02-dev-design',
    description: '详细设计与任务拆解',
    planner: {
      agent_type: 'dev-team:dev-design-planner',
      prompt: 'Write design.md and tasks.md for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:dev-design-evaluator',
      prompt:
        'Evaluate design.md and tasks.md for change "<change>" against proposal.md. Append result to eval.json.',
    },
  },
  {
    id: '03-test-design',
    description: '测试设计',
    planner: {
      agent_type: 'dev-team:test-design-planner',
      prompt: 'Write test design for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:test-design-evaluator',
      prompt:
        'Evaluate test design for change "<change>" against design.md. Append result to eval.json.',
    },
  },
  {
    id: '05-implement',
    description: '代码实现',
    planner: {
      agent_type: 'dev-team:implementation-generator',
      prompt: 'Implement the code for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:implementation-evaluator',
      prompt:
        'Evaluate implementation for change "<change>" against design. Append result to eval.json.',
    },
  },
  {
    id: '04-test-gen',
    description: '测试代码生成',
    planner: {
      agent_type: 'dev-team:test-gen-generator',
      prompt: 'Generate test code for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:test-gen-evaluator',
      prompt: 'Evaluate generated tests for change "<change>". Append result to eval.json.',
    },
  },
  {
    id: '06-unit-test',
    description: '单元测试执行与诊断',
    planner: {
      agent_type: 'dev-team:unit-test-executor',
      prompt: 'Run and fix unit tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:unit-test-evaluator',
      prompt: 'Evaluate unit test results for change "<change>". Append result to eval.json.',
    },
  },
  {
    id: '07-code-review',
    description: '代码审查',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:code-review-evaluator',
      prompt: 'Perform code review for change "<change>". Append result to eval.json.',
    },
  },
  {
    id: '08-integration-test',
    description: '集成测试执行与诊断',
    planner: {
      agent_type: 'dev-team:integration-test-executor',
      prompt: 'Run and fix integration tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:integration-test-evaluator',
      prompt:
        'Evaluate integration test results for change "<change>". Append result to eval.json.',
    },
  },
  {
    id: '09-acceptance',
    description: '验收评估',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:acceptance-evaluator',
      prompt: 'Perform acceptance evaluation for change "<change>". Append result to eval.json.',
    },
  },
];

const PHASE_BUG_FIX: PhaseDefinition[] = [
  {
    id: '01-proposal',
    description: '需求提案与规格说明',
    planner: {
      agent_type: 'dev-team:proposal-planner',
      prompt: 'Write proposal.md and specs/ for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:proposal-evaluator',
      prompt: 'Evaluate proposal.md for change "<change>" against checklist.',
    },
  },
  {
    id: '02-dev-design',
    description: '详细设计与任务拆解',
    planner: {
      agent_type: 'dev-team:dev-design-planner',
      prompt: 'Write design.md and tasks.md for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:dev-design-evaluator',
      prompt:
        'Evaluate design.md and tasks.md for change "<change>" against proposal.md. Append result to eval.json.',
    },
  },
  {
    id: '05-implement',
    description: '代码实现',
    planner: {
      agent_type: 'dev-team:implementation-generator',
      prompt: 'Implement the code for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:implementation-evaluator',
      prompt:
        'Evaluate implementation for change "<change>" against design. Append result to eval.json.',
    },
  },
  {
    id: '06-unit-test',
    description: '单元测试执行与诊断',
    planner: {
      agent_type: 'dev-team:unit-test-executor',
      prompt: 'Run and fix unit tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:unit-test-evaluator',
      prompt: 'Evaluate unit test results for change "<change>". Append result to eval.json.',
    },
  },
  {
    id: '07-code-review',
    description: '代码审查',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:code-review-evaluator',
      prompt: 'Perform code review for change "<change>". Append result to eval.json.',
    },
  },
  {
    id: '09-acceptance',
    description: '验收评估',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:acceptance-evaluator',
      prompt: 'Perform acceptance evaluation for change "<change>". Append result to eval.json.',
    },
  },
];

const PHASE_REFACTOR: PhaseDefinition[] = PHASE_REQUIREMENT;

const WORKFLOW_CONTEXT_TEST_ONLY =
  'WORKFLOW_CONTEXT: test-only — 无 implement/dev-design 阶段。若 phase_log 因 backtrack 目标不存在而拒绝调用，以 backtrack_to:null 重新记录，report 中包含发现的代码 bug 详情，然后返回主 agent 附带 bug 信息摘要。';

const PHASE_TEST_ONLY: PhaseDefinition[] = [
  {
    id: '01-proposal',
    description: '测试需求提案与规格说明',
    planner: {
      agent_type: 'dev-team:proposal-planner',
      prompt:
        'Write test-focused proposal.md and specs/ for change "<change>": coverage gaps, testing strategy, and acceptance criteria for existing code.',
    },
    evaluator: {
      agent_type: 'dev-team:proposal-evaluator',
      prompt: 'Evaluate proposal.md for change "<change>" against checklist.',
    },
  },
  {
    id: '02-code-analyze',
    description: '逆向分析现有代码架构',
    planner: {
      agent_type: 'dev-team:code-analyze-planner',
      prompt:
        'Reverse-engineer existing code architecture and write design.md for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:code-analyze-evaluator',
      prompt:
        'Evaluate design.md for change "<change>" against proposal.md. Append result to eval.json.',
    },
  },
  {
    id: '03-test-design',
    description: '测试设计',
    planner: {
      agent_type: 'dev-team:test-design-planner',
      prompt: 'Write test design for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:test-design-evaluator',
      prompt:
        'Evaluate test design for change "<change>" against design.md. Append result to eval.json.',
    },
  },
  {
    id: '04-test-gen',
    description: '测试代码生成',
    planner: {
      agent_type: 'dev-team:test-gen-generator',
      prompt: 'Generate test code for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:test-gen-evaluator',
      prompt: 'Evaluate generated tests for change "<change>". Append result to eval.json.',
    },
  },
  {
    id: '06-unit-test',
    description: '单元测试执行与诊断',
    planner: {
      agent_type: 'dev-team:unit-test-executor',
      prompt: 'Run and fix unit tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:unit-test-evaluator',
      prompt: `Evaluate unit test results for change "<change>". Append result to eval.json. ${WORKFLOW_CONTEXT_TEST_ONLY}`,
    },
  },
  {
    id: '08-integration-test',
    description: '集成测试执行与诊断',
    planner: {
      agent_type: 'dev-team:integration-test-executor',
      prompt: 'Run and fix integration tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:integration-test-evaluator',
      prompt: `Evaluate integration test results for change "<change>". Append result to eval.json. ${WORKFLOW_CONTEXT_TEST_ONLY}`,
    },
  },
];

const PHASE_TABLES: Record<string, PhaseDefinition[]> = {
  requirement: PHASE_REQUIREMENT,
  'bug-fix': PHASE_BUG_FIX,
  refactor: PHASE_REFACTOR,
  'test-only': PHASE_TEST_ONLY,
};

// ---------------------------------------------------------------------------
// Prerequisite tables — defined per workflow_type
// ---------------------------------------------------------------------------

/**
 * Prerequisite dependency table for the `requirement` workflow_type.
 * Each phase lists its direct dependencies (phases that must have a non-stale
 * pass entry before this phase can execute).
 *
 * This is the single source of truth for the dependency graph.
 * `getDependents()` derives the reverse mapping from this table.
 */
const PHASE_PREREQUISITES: Record<string, string[]> = {
  '01-proposal': [],
  '02-dev-design': ['01-proposal'],
  '03-test-design': ['01-proposal', '02-dev-design'],
  '04-test-gen': ['03-test-design', '05-implement'],
  '05-implement': ['02-dev-design'],
  '06-unit-test': ['04-test-gen', '05-implement'],
  '07-code-review': ['04-test-gen', '05-implement'],
  '08-integration-test': ['04-test-gen', '05-implement'],
  '09-acceptance': ['01-proposal', '02-dev-design', '05-implement'],
};

/**
 * Prerequisite dependency table for the `bug-fix` workflow_type.
 * Simplified pipeline — only core development phases.
 */
const PHASE_BUG_FIX_PREREQUISITES: Record<string, string[]> = {
  '01-proposal': [],
  '02-dev-design': ['01-proposal'],
  '05-implement': ['02-dev-design'],
  '06-unit-test': ['05-implement'],
  '07-code-review': ['05-implement'],
  '09-acceptance': ['07-code-review'],
};

/**
 * Prerequisite dependency table for the `refactor` workflow_type.
 * Matches the `requirement` table.
 */
const PHASE_REFACTOR_PREREQUISITES: Record<string, string[]> = PHASE_PREREQUISITES;

const PHASE_TEST_ONLY_PREREQUISITES: Record<string, string[]> = {
  '01-proposal': [],
  '02-code-analyze': ['01-proposal'],
  '03-test-design': ['01-proposal', '02-code-analyze'],
  '04-test-gen': ['03-test-design'],
  '06-unit-test': ['04-test-gen'],
  '08-integration-test': ['04-test-gen'],
};

const PHASE_PREREQUISITES_TABLES: Record<string, Record<string, string[]>> = {
  requirement: PHASE_PREREQUISITES,
  'bug-fix': PHASE_BUG_FIX_PREREQUISITES,
  refactor: PHASE_REFACTOR_PREREQUISITES,
  'test-only': PHASE_TEST_ONLY_PREREQUISITES,
};

/**
 * Return the prerequisite table for the given workflow_type.
 * Defaults to "requirement" if unknown.
 */
function getPrerequisiteTable(workflowType?: string): Record<string, string[]> {
  const key = (workflowType || DEFAULT_WORKFLOW).toLowerCase();
  return PHASE_PREREQUISITES_TABLES[key] || PHASE_PREREQUISITES_TABLES[DEFAULT_WORKFLOW];
}

/**
 * Return the list of prerequisite phase IDs for a given phase in the specified workflow.
 *
 * - Returns an empty array for phases with no prerequisites.
 * - Returns an empty array for unknown phase IDs (fault-tolerant).
 * - Defaults to "requirement" workflow_type.
 */
function getPrerequisites(phaseId: string, workflowType?: string): string[] {
  const table = getPrerequisiteTable(workflowType);
  return table[phaseId] || [];
}

/**
 * Return the list of phases that depend on the given phase in the specified workflow.
 *
 * Derived from `getPrerequisites()` (single source of truth): a phase B depends on phase A
 * iff `getPrerequisites(B)` includes A. This function iterates all phases in the workflow
 * table and returns those whose prerequisites include `phaseId`.
 *
 * - Returns an empty array for phases with no dependents.
 * - Returns an empty array for unknown phase IDs (fault-tolerant).
 * - Defaults to "requirement" workflow_type.
 */
export function getDependents(phaseId: string, workflowType?: string): string[] {
  const table = getPhaseTable(workflowType);
  return table
    .filter((p) => {
      const prereqs = getPrerequisites(p.id, workflowType);
      return prereqs.includes(phaseId);
    })
    .map((p) => p.id);
}

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
