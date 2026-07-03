/**
 * PGE workflow phase configuration — single source of truth for all phase definitions.
 *
 * This is the only file that defines phase ordering, phase patterns
 * (DESIGN/EXEC/EVAL-ONLY), agent assignments, and prompt templates.
 * All other modules (eval-next, eval-check, eval-log) import from here.
 */

import { type z } from 'zod/v4';

import { type phaseIdSchema } from '../schemas';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type PhaseId = z.infer<typeof phaseIdSchema>;

interface PhaseAgentDef {
  agent_type: string;
  prompt: string;
}

export interface PhaseDefinition {
  id: PhaseId;
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
    id: 'proposal',
    description: '需求提案与规格说明',
    planner: {
      agent_type: 'dev-team:proposal-planner',
      prompt: 'Write proposal.md and specs/ for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:proposal-evaluator',
      prompt: 'Evaluate <phase> phase for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'dev-design',
    description: '详细设计与任务拆解',
    planner: {
      agent_type: 'dev-team:dev-design-planner',
      prompt: 'Write design.md and tasks.md for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:dev-design-evaluator',
      prompt:
        'Evaluate <phase> phase: design.md and tasks.md for change "<change>" against proposal.md. Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'test-design',
    description: '测试设计',
    planner: {
      agent_type: 'dev-team:test-design-planner',
      prompt: 'Write test design for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:test-design-evaluator',
      prompt:
        'Evaluate <phase> phase: test design for change "<change>" against design.md. Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'implement',
    description: '代码实现',
    planner: {
      agent_type: 'dev-team:implementation-generator',
      prompt: 'Implement the code for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:implementation-evaluator',
      prompt:
        'Evaluate <phase> phase: implementation for change "<change>" against design. Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'test-gen',
    description: '测试代码生成',
    planner: {
      agent_type: 'dev-team:test-gen-generator',
      prompt: 'Generate test code for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:test-gen-evaluator',
      prompt:
        'Evaluate <phase> phase: generated tests for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'unit-test',
    description: '单元测试执行与诊断',
    planner: {
      agent_type: 'dev-team:unit-test-executor',
      prompt: 'Run and fix unit tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:unit-test-evaluator',
      prompt:
        'Evaluate <phase> phase: unit test results for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'code-review',
    description: '代码审查',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:code-review-evaluator',
      prompt:
        'Evaluate <phase> phase: code review for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'integration-test',
    description: '集成测试执行与诊断',
    planner: {
      agent_type: 'dev-team:integration-test-executor',
      prompt: 'Run and fix integration tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:integration-test-evaluator',
      prompt:
        'Evaluate <phase> phase: integration test results for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'acceptance',
    description: '验收评估',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:acceptance-evaluator',
      prompt:
        'Evaluate <phase> phase: acceptance for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
];

const PHASE_BUG_FIX: PhaseDefinition[] = [
  {
    id: 'proposal',
    description: '需求提案与规格说明',
    planner: {
      agent_type: 'dev-team:proposal-planner',
      prompt: 'Write proposal.md and specs/ for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:proposal-evaluator',
      prompt: 'Evaluate <phase> phase for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'dev-design',
    description: '详细设计与任务拆解',
    planner: {
      agent_type: 'dev-team:dev-design-planner',
      prompt: 'Write design.md and tasks.md for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:dev-design-evaluator',
      prompt:
        'Evaluate <phase> phase: design.md and tasks.md for change "<change>" against proposal.md. Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'implement',
    description: '代码实现',
    planner: {
      agent_type: 'dev-team:implementation-generator',
      prompt: 'Implement the code for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:implementation-evaluator',
      prompt:
        'Evaluate <phase> phase: implementation for change "<change>" against design. Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'unit-test',
    description: '单元测试执行与诊断',
    planner: {
      agent_type: 'dev-team:unit-test-executor',
      prompt: 'Run and fix unit tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:unit-test-evaluator',
      prompt:
        'Evaluate <phase> phase: unit test results for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'code-review',
    description: '代码审查',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:code-review-evaluator',
      prompt:
        'Evaluate <phase> phase: code review for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'acceptance',
    description: '验收评估',
    planner: null,
    evaluator: {
      agent_type: 'dev-team:acceptance-evaluator',
      prompt:
        'Evaluate <phase> phase: acceptance for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
];

const PHASE_REFACTOR: PhaseDefinition[] = PHASE_REQUIREMENT;

const WORKFLOW_CONTEXT_TEST_ONLY =
  'WORKFLOW_CONTEXT: test-only — 无 implement/dev-design 阶段。若 phase_log 因 backtrack 目标不存在而拒绝调用，以 backtrack_to:null 重新记录，report 中包含发现的代码 bug 详情，然后返回主 agent 附带 bug 信息摘要。';

const PHASE_TEST_ONLY: PhaseDefinition[] = [
  {
    id: 'proposal',
    description: '测试需求提案与规格说明',
    planner: {
      agent_type: 'dev-team:proposal-planner',
      prompt:
        'Write test-focused proposal.md and specs/ for change "<change>": coverage gaps, testing strategy, and acceptance criteria for existing code.',
    },
    evaluator: {
      agent_type: 'dev-team:proposal-evaluator',
      prompt: 'Evaluate <phase> phase for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'code-analyze',
    description: '逆向分析现有代码架构',
    planner: {
      agent_type: 'dev-team:code-analyze-planner',
      prompt:
        'Reverse-engineer existing code architecture and write design.md for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:code-analyze-evaluator',
      prompt:
        'Evaluate <phase> phase: design.md for change "<change>" against proposal.md. Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'test-design',
    description: '测试设计',
    planner: {
      agent_type: 'dev-team:test-design-planner',
      prompt: 'Write test design for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:test-design-evaluator',
      prompt:
        'Evaluate <phase> phase: test design for change "<change>" against design.md. Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'test-gen',
    description: '测试代码生成',
    planner: {
      agent_type: 'dev-team:test-gen-generator',
      prompt: 'Generate test code for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:test-gen-evaluator',
      prompt:
        'Evaluate <phase> phase: generated tests for change "<change>". Call phase_log with phase="<phase>".',
    },
  },
  {
    id: 'unit-test',
    description: '单元测试执行与诊断',
    planner: {
      agent_type: 'dev-team:unit-test-executor',
      prompt: 'Run and fix unit tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:unit-test-evaluator',
      prompt: `Evaluate <phase> phase: unit test results for change "<change>". Call phase_log with phase="<phase>". ${WORKFLOW_CONTEXT_TEST_ONLY}`,
    },
  },
  {
    id: 'integration-test',
    description: '集成测试执行与诊断',
    planner: {
      agent_type: 'dev-team:integration-test-executor',
      prompt: 'Run and fix integration tests for change "<change>".',
    },
    evaluator: {
      agent_type: 'dev-team:integration-test-evaluator',
      prompt: `Evaluate <phase> phase: integration test results for change "<change>". Call phase_log with phase="<phase>". ${WORKFLOW_CONTEXT_TEST_ONLY}`,
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
  proposal: [],
  'dev-design': ['proposal'],
  'test-design': ['proposal', 'dev-design'],
  'test-gen': ['test-design', 'implement'],
  implement: ['dev-design'],
  'unit-test': ['test-gen', 'implement'],
  'code-review': ['test-gen', 'implement'],
  'integration-test': ['test-gen', 'implement'],
  acceptance: ['proposal', 'dev-design', 'implement'],
};

/**
 * Prerequisite dependency table for the `bug-fix` workflow_type.
 * Simplified pipeline — only core development phases.
 */
const PHASE_BUG_FIX_PREREQUISITES: Record<string, string[]> = {
  proposal: [],
  'dev-design': ['proposal'],
  implement: ['dev-design'],
  'unit-test': ['implement'],
  'code-review': ['implement'],
  acceptance: ['code-review'],
};

/**
 * Prerequisite dependency table for the `refactor` workflow_type.
 * Matches the `requirement` table.
 */
const PHASE_REFACTOR_PREREQUISITES: Record<string, string[]> = PHASE_PREREQUISITES;

const PHASE_TEST_ONLY_PREREQUISITES: Record<string, string[]> = {
  proposal: [],
  'code-analyze': ['proposal'],
  'test-design': ['proposal', 'code-analyze'],
  'test-gen': ['test-design'],
  'unit-test': ['test-gen'],
  'integration-test': ['test-gen'],
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
export function getDependents(phaseId: string, workflowType: string): string[] {
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
 */
export function getPhaseTable(workflowType: string): PhaseDefinition[] {
  if (Reflect.has(PHASE_TABLES, workflowType.toLowerCase())) {
    return PHASE_TABLES[workflowType.toLowerCase()];
  } else {
    throw new Error('workflowType 不存在');
  }
}
