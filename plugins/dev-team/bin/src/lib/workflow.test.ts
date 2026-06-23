import { describe, it, expect } from 'vite-plus/test';

import { getPhaseTable, getDependents } from './workflow';

function requirementPhaseIndex(phase: string): number {
  return getPhaseTable('requirement').findIndex((p) => p.id === phase);
}

describe('requirement phase index', () => {
  it('should return 0 for first phase', () => {
    expect(requirementPhaseIndex('01-proposal')).toBe(0);
  });

  it('should return 3 for 05-implement（AC-1）', () => {
    expect(requirementPhaseIndex('05-implement')).toBe(3);
  });

  it('should return 4 for 04-test-gen（AC-1）', () => {
    expect(requirementPhaseIndex('04-test-gen')).toBe(4);
  });

  it('should return 5 for 06-unit-test（索引随 04/05 对调后仍正确）', () => {
    expect(requirementPhaseIndex('06-unit-test')).toBe(5);
  });

  it('should return correct index for new 08-integration-test', () => {
    expect(requirementPhaseIndex('08-integration-test')).toBe(7);
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
  it('should return [02-dev-design, 03-test-design, 09-acceptance] for 01-proposal (AC-4)', () => {
    const deps = getDependents('01-proposal', 'requirement');
    expect(deps).toEqual(['02-dev-design', '03-test-design', '09-acceptance']);
  });

  it('should return [03-test-design, 05-implement, 09-acceptance] for 02-dev-design (AC-4)', () => {
    const deps = getDependents('02-dev-design', 'requirement');
    expect(deps).toEqual(['03-test-design', '05-implement', '09-acceptance']);
  });

  it('should return [04-test-gen] for 03-test-design — 03 不依赖 05，05 非其 downstream（AC-4）', () => {
    const deps = getDependents('03-test-design', 'requirement');
    expect(deps).toEqual(['04-test-gen']);
  });

  it('should return [06, 07, 08] for 04-test-gen（不变）', () => {
    const deps = getDependents('04-test-gen', 'requirement');
    expect(deps).toEqual(['06-unit-test', '07-code-review', '08-integration-test']);
  });

  it('should return [04-test-gen, 06, 07, 08, 09] for 05-implement — 04 为直接 downstream（AC-4）', () => {
    const deps = getDependents('05-implement', 'requirement');
    expect(deps).toEqual([
      '04-test-gen',
      '06-unit-test',
      '07-code-review',
      '08-integration-test',
      '09-acceptance',
    ]);
  });

  it('should return [] for leaf phases (06, 07, 08, 09)', () => {
    expect(getDependents('06-unit-test', 'requirement')).toEqual([]);
    expect(getDependents('07-code-review', 'requirement')).toEqual([]);
    expect(getDependents('08-integration-test', 'requirement')).toEqual([]);
    expect(getDependents('09-acceptance', 'requirement')).toEqual([]);
  });

  it('should return [] for unknown phase (fault-tolerant)', () => {
    expect(getDependents('99-unknown')).toEqual([]);
  });

  it('should default to requirement workflow_type', () => {
    const deps = getDependents('02-dev-design');
    expect(deps).toEqual(['03-test-design', '05-implement', '09-acceptance']);
  });

  it('should return [02-code-analyze, 03-test-design] for 01-proposal in test-only (AC-5)', () => {
    const deps = getDependents('01-proposal', 'test-only');
    expect(deps).toEqual(['02-code-analyze', '03-test-design']);
  });

  it('should return [03-test-design] for 02-code-analyze in test-only (AC-6)', () => {
    const deps = getDependents('02-code-analyze', 'test-only');
    expect(deps).toEqual(['03-test-design']);
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
      '01-proposal',
      '02-code-analyze',
      '03-test-design',
      '04-test-gen',
      '06-unit-test',
      '08-integration-test',
    ]);
  });

  it('should not include dev-design, implement, review, or acceptance phases', () => {
    const ids = getPhaseTable('test-only').map((p) => p.id);
    expect(ids).not.toContain('02-dev-design');
    expect(ids).not.toContain('05-implement');
    expect(ids).not.toContain('07-code-review');
    expect(ids).not.toContain('09-acceptance');
  });

  it('should use code-analyze agents for 02-code-analyze (AC-3)', () => {
    const phase = getPhaseTable('test-only').find((p) => p.id === '02-code-analyze');
    expect(phase?.planner?.agent_type).toBe('dev-team:code-analyze-planner');
    expect(phase?.evaluator?.agent_type).toBe('dev-team:code-analyze-evaluator');
  });
});

describe('PHASE_TEST_ONLY — prompt customization', () => {
  it('01-proposal planner prompt differs from requirement (AC-9)', () => {
    const reqPrompt = getPhaseTable('requirement').find((p) => p.id === '01-proposal')!.planner!
      .prompt;
    const testPrompt = getPhaseTable('test-only').find((p) => p.id === '01-proposal')!.planner!
      .prompt;
    expect(testPrompt).not.toBe(reqPrompt);
    expect(testPrompt).toMatch(/coverage gaps|testing strategy/i);
  });
});

describe('PHASE_TEST_ONLY — WORKFLOW_CONTEXT prompts', () => {
  it('06-unit-test evaluator prompt includes WORKFLOW_CONTEXT (AC-14)', () => {
    const prompt = getPhaseTable('test-only').find((p) => p.id === '06-unit-test')!.evaluator!
      .prompt;
    expect(prompt).toMatch(/WORKFLOW_CONTEXT/);
  });

  it('08-integration-test evaluator prompt includes WORKFLOW_CONTEXT (AC-14)', () => {
    const prompt = getPhaseTable('test-only').find((p) => p.id === '08-integration-test')!
      .evaluator!.prompt;
    expect(prompt).toMatch(/WORKFLOW_CONTEXT/);
  });
});

// ---------------------------------------------------------------------------
// getDependents — test-only extended (AC-5/6)
// ---------------------------------------------------------------------------

describe('getDependents — test-only extended', () => {
  it('should return [04-test-gen] for 03-test-design test-only', () => {
    expect(getDependents('03-test-design', 'test-only')).toEqual(['04-test-gen']);
  });

  it('should return [06-unit-test, 08-integration-test] for 04-test-gen test-only', () => {
    expect(getDependents('04-test-gen', 'test-only')).toEqual([
      '06-unit-test',
      '08-integration-test',
    ]);
  });

  it('should return [] for leaf phases 06-unit-test and 08-integration-test test-only', () => {
    expect(getDependents('06-unit-test', 'test-only')).toEqual([]);
    expect(getDependents('08-integration-test', 'test-only')).toEqual([]);
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
  it('should return [01-proposal, 02-code-analyze] for 03-test-design test-only', () => {
    expect(inferPrerequisites('03-test-design', 'test-only')).toEqual([
      '01-proposal',
      '02-code-analyze',
    ]);
  });

  it('should return [03-test-design] for 04-test-gen test-only', () => {
    expect(inferPrerequisites('04-test-gen', 'test-only')).toEqual(['03-test-design']);
  });

  it('should return [04-test-gen] for 06-unit-test and 08-integration-test test-only', () => {
    expect(inferPrerequisites('06-unit-test', 'test-only')).toEqual(['04-test-gen']);
    expect(inferPrerequisites('08-integration-test', 'test-only')).toEqual(['04-test-gen']);
  });
});

// ---------------------------------------------------------------------------
// Regression — AC-10
// ---------------------------------------------------------------------------

describe('getPhaseTable — regression (AC-10)', () => {
  it('bug-fix phase table length and ids unchanged', () => {
    const phases = getPhaseTable('bug-fix').map((p) => p.id);
    expect(phases).toEqual([
      '01-proposal',
      '02-dev-design',
      '05-implement',
      '06-unit-test',
      '07-code-review',
      '09-acceptance',
    ]);
  });

  it('refactor phase table matches requirement', () => {
    expect(getPhaseTable('refactor').map((p) => p.id)).toEqual(
      getPhaseTable('requirement').map((p) => p.id),
    );
  });
});
