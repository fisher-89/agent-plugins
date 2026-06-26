import { describe, it, expect } from 'vite-plus/test';

import { getPhaseTable, getDependents } from './workflow';

function requirementPhaseIndex(phase: string): number {
  return getPhaseTable('requirement').findIndex((p) => p.id === phase);
}

describe('requirement phase index', () => {
  it('should return 0 for first phase', () => {
    expect(requirementPhaseIndex('proposal')).toBe(0);
  });

  it('should return 3 for implement（AC-1）', () => {
    expect(requirementPhaseIndex('implement')).toBe(3);
  });

  it('should return 4 for test-gen（AC-1）', () => {
    expect(requirementPhaseIndex('test-gen')).toBe(4);
  });

  it('should return 5 for unit-test（索引随 04/05 对调后仍正确）', () => {
    expect(requirementPhaseIndex('unit-test')).toBe(5);
  });

  it('should return correct index for new integration-test', () => {
    expect(requirementPhaseIndex('integration-test')).toBe(7);
  });

  it('should return -1 for unknown phase', () => {
    expect(requirementPhaseIndex('99-unknown')).toBe(-1);
  });

  it('should return -1 for old phase name', () => {
    expect(requirementPhaseIndex('05-implementation')).toBe(-1);
  });
});

// ---------------------------------------------------------------------------
// getDependents — AC-4
// ---------------------------------------------------------------------------

describe('getDependents', () => {
  it('should return [dev-design, test-design, acceptance] for proposal (AC-4)', () => {
    const deps = getDependents('proposal', 'requirement');
    expect(deps).toEqual(['dev-design', 'test-design', 'acceptance']);
  });

  it('should return [test-design, implement, acceptance] for dev-design (AC-4)', () => {
    const deps = getDependents('dev-design', 'requirement');
    expect(deps).toEqual(['test-design', 'implement', 'acceptance']);
  });

  it('should return [test-gen] for test-design — 03 不依赖 05，05 非其 downstream（AC-4）', () => {
    const deps = getDependents('test-design', 'requirement');
    expect(deps).toEqual(['test-gen']);
  });

  it('should return [unit-test, code-review, integration-test] for test-gen（不变）', () => {
    const deps = getDependents('test-gen', 'requirement');
    expect(deps).toEqual(['unit-test', 'code-review', 'integration-test']);
  });

  it('should return [test-gen, 06, 07, 08, 09] for implement — 04 为直接 downstream（AC-4）', () => {
    const deps = getDependents('implement', 'requirement');
    expect(deps).toEqual([
      'test-gen',
      'unit-test',
      'code-review',
      'integration-test',
      'acceptance',
    ]);
  });

  it('should return [] for leaf phases (06, 07, 08, 09)', () => {
    expect(getDependents('unit-test', 'requirement')).toEqual([]);
    expect(getDependents('code-review', 'requirement')).toEqual([]);
    expect(getDependents('integration-test', 'requirement')).toEqual([]);
    expect(getDependents('acceptance', 'requirement')).toEqual([]);
  });

  it('should return [] for unknown phase (fault-tolerant)', () => {
    expect(getDependents('99-unknown')).toEqual([]);
  });

  it('should default to requirement workflow_type', () => {
    const deps = getDependents('dev-design');
    expect(deps).toEqual(['test-design', 'implement', 'acceptance']);
  });

  it('should return [code-analyze, test-design] for proposal in test-only (AC-5)', () => {
    const deps = getDependents('proposal', 'test-only');
    expect(deps).toEqual(['code-analyze', 'test-design']);
  });

  it('should return [test-design] for code-analyze in test-only (AC-6)', () => {
    const deps = getDependents('code-analyze', 'test-only');
    expect(deps).toEqual(['test-design']);
  });
});

// ---------------------------------------------------------------------------
// test-only phase table
// ---------------------------------------------------------------------------

describe('getPhaseTable — test-only', () => {
  it('should have exactly 6 phases for test-only workflow_type', () => {
    expect(getPhaseTable('test-only')).toHaveLength(6);
  });

  it('should list test-only phases in expected order', () => {
    const ids = getPhaseTable('test-only').map((p) => p.id);
    expect(ids).toEqual([
      'proposal',
      'code-analyze',
      'test-design',
      'test-gen',
      'unit-test',
      'integration-test',
    ]);
  });

  it('should not include dev-design, implement, review, or acceptance phases', () => {
    const ids = getPhaseTable('test-only').map((p) => p.id);
    expect(ids).not.toContain('dev-design');
    expect(ids).not.toContain('implement');
    expect(ids).not.toContain('code-review');
    expect(ids).not.toContain('acceptance');
  });

  it('should use code-analyze agents for code-analyze (AC-3)', () => {
    const phase = getPhaseTable('test-only').find((p) => p.id === 'code-analyze');
    expect(phase?.planner?.agent_type).toBe('dev-team:code-analyze-planner');
    expect(phase?.evaluator?.agent_type).toBe('dev-team:code-analyze-evaluator');
  });
});

describe('PHASE_TEST_ONLY — prompt customization', () => {
  it('proposal planner prompt differs from requirement (AC-9)', () => {
    const reqPrompt = getPhaseTable('requirement').find((p) => p.id === 'proposal')!.planner!
      .prompt;
    const testPrompt = getPhaseTable('test-only').find((p) => p.id === 'proposal')!.planner!.prompt;
    expect(testPrompt).not.toBe(reqPrompt);
    expect(testPrompt).toMatch(/coverage gaps|testing strategy/i);
  });
});

describe('PHASE_TEST_ONLY — WORKFLOW_CONTEXT prompts', () => {
  it('unit-test evaluator prompt includes WORKFLOW_CONTEXT (AC-14)', () => {
    const prompt = getPhaseTable('test-only').find((p) => p.id === 'unit-test')!.evaluator!.prompt;
    expect(prompt).toMatch(/WORKFLOW_CONTEXT/);
  });

  it('integration-test evaluator prompt includes WORKFLOW_CONTEXT (AC-14)', () => {
    const prompt = getPhaseTable('test-only').find((p) => p.id === 'integration-test')!.evaluator!
      .prompt;
    expect(prompt).toMatch(/WORKFLOW_CONTEXT/);
  });
});

// ---------------------------------------------------------------------------
// getDependents — test-only extended (AC-5/6)
// ---------------------------------------------------------------------------

describe('getDependents — test-only extended', () => {
  it('should return [test-gen] for test-design test-only', () => {
    expect(getDependents('test-design', 'test-only')).toEqual(['test-gen']);
  });

  it('should return [unit-test, integration-test] for test-gen test-only', () => {
    expect(getDependents('test-gen', 'test-only')).toEqual(['unit-test', 'integration-test']);
  });

  it('should return [] for leaf phases unit-test and integration-test test-only', () => {
    expect(getDependents('unit-test', 'test-only')).toEqual([]);
    expect(getDependents('integration-test', 'test-only')).toEqual([]);
  });

  it('should return [] for unknown phase 99-unknown test-only', () => {
    expect(getDependents('99-unknown', 'test-only')).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// getPrerequisites — test-only (derived from getDependents inverse)
// ---------------------------------------------------------------------------

function inferPrerequisites(phaseId: string, workflowType?: string): string[] {
  const table = getPhaseTable(workflowType);
  return table.map((p) => p.id).filter((id) => getDependents(id, workflowType).includes(phaseId));
}

describe('getPrerequisites — test-only', () => {
  it('should return [proposal, code-analyze] for test-design test-only', () => {
    expect(inferPrerequisites('test-design', 'test-only')).toEqual(['proposal', 'code-analyze']);
  });

  it('should return [test-design] for test-gen test-only', () => {
    expect(inferPrerequisites('test-gen', 'test-only')).toEqual(['test-design']);
  });

  it('should return [test-gen] for unit-test and integration-test test-only', () => {
    expect(inferPrerequisites('unit-test', 'test-only')).toEqual(['test-gen']);
    expect(inferPrerequisites('integration-test', 'test-only')).toEqual(['test-gen']);
  });
});

// ---------------------------------------------------------------------------
// Regression — AC-10
// ---------------------------------------------------------------------------

describe('getPhaseTable — regression (AC-10)', () => {
  it('bug-fix phase table length and ids unchanged', () => {
    const phases = getPhaseTable('bug-fix').map((p) => p.id);
    expect(phases).toEqual([
      'proposal',
      'dev-design',
      'implement',
      'unit-test',
      'code-review',
      'acceptance',
    ]);
  });

  it('refactor phase table matches requirement', () => {
    expect(getPhaseTable('refactor').map((p) => p.id)).toEqual(
      getPhaseTable('requirement').map((p) => p.id),
    );
  });
});
