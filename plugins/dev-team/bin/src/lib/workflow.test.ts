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

  it('should return 5 for test-execution（索引随 04/05 对调后仍正确）', () => {
    expect(requirementPhaseIndex('test-execution')).toBe(5);
  });

  it('should return -1 for unknown phase', () => {
    expect(requirementPhaseIndex('99-unknown')).toBe(-1);
  });

  it('should return -1 for old phase name', () => {
    expect(requirementPhaseIndex('05-implementation')).toBe(-1);
  });

  it('should return -1 for empty string phase', () => {
    expect(requirementPhaseIndex('')).toBe(-1);
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

  it('should return [test-execution, code-review] for test-gen', () => {
    const deps = getDependents('test-gen', 'requirement');
    expect(deps).toEqual(['test-execution', 'code-review']);
  });

  it('should return [test-gen, test-execution, code-review, acceptance] for implement', () => {
    const deps = getDependents('implement', 'requirement');
    expect(deps).toEqual(['test-gen', 'test-execution', 'code-review', 'acceptance']);
  });

  it('should return [] for leaf phases (06, 07, 08)', () => {
    expect(getDependents('test-execution', 'requirement')).toEqual([]);
    expect(getDependents('code-review', 'requirement')).toEqual([]);
    expect(getDependents('acceptance', 'requirement')).toEqual([]);
  });

  it('should return [] for unknown phase (fault-tolerant)', () => {
    expect(getDependents('99-unknown', 'requirement')).toEqual([]);
  });

  it('should return [] for empty string phase', () => {
    expect(getDependents('', 'requirement')).toEqual([]);
  });

  it('should fallback to requirement table for empty string workflowType', () => {
    expect(getDependents('test-gen', '')).toEqual(['test-execution', 'code-review']);
  });

  it('should fallback to requirement table for unknown workflowType (case-insensitive tolerant)', () => {
    expect(getDependents('test-gen', 'UNKNOWN_WORKFLOW')).toEqual([
      'test-execution',
      'code-review',
    ]);
  });

  it('should default to requirement workflow_type', () => {
    const deps = getDependents('dev-design', 'requirement');
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
  it('should have exactly 5 phases for test-only workflow_type', () => {
    expect(getPhaseTable('test-only')).toHaveLength(5);
  });

  it('should list test-only phases in expected order', () => {
    const ids = getPhaseTable('test-only').map((p) => p.id);
    expect(ids).toEqual(['proposal', 'code-analyze', 'test-design', 'test-gen', 'test-execution']);
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
  it('test-execution evaluator prompt includes WORKFLOW_CONTEXT', () => {
    const prompt = getPhaseTable('test-only').find((p) => p.id === 'test-execution')!.evaluator!
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

  it('should return [test-execution] for test-gen test-only', () => {
    expect(getDependents('test-gen', 'test-only')).toEqual(['test-execution']);
  });

  it('should return [] for leaf phase test-execution test-only', () => {
    expect(getDependents('test-execution', 'test-only')).toEqual([]);
  });

  it('should return [] for unknown phase 99-unknown test-only', () => {
    expect(getDependents('99-unknown', 'test-only')).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// getPrerequisites — test-only (derived from getDependents inverse)
// ---------------------------------------------------------------------------

function inferPrerequisites(phaseId: string, workflowType: string): string[] {
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

  it('should return [test-gen] for test-execution test-only', () => {
    expect(inferPrerequisites('test-execution', 'test-only')).toEqual(['test-gen']);
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
      'test-execution',
      'code-review',
      'acceptance',
    ]);
  });

  it('bug-fix phase table 应不包含 integration-test', () => {
    const phases = getPhaseTable('bug-fix').map((p) => p.id);
    expect(phases).not.toContain('integration-test');
    expect(phases.length).toBe(6);
  });

  it('refactor phase table matches requirement', () => {
    expect(getPhaseTable('refactor').map((p) => p.id)).toEqual(
      getPhaseTable('requirement').map((p) => p.id),
    );
  });

  it('getPhaseTable("INVALID") 应降级到 requirement 表（容错）', () => {
    const phases = getPhaseTable('INVALID');
    expect(phases.map((p) => p.id)).toEqual(getPhaseTable('requirement').map((p) => p.id));
  });

  it('getPhaseTable("") 应降级到 requirement 表（容错）', () => {
    const phases = getPhaseTable('');
    expect(phases.map((p) => p.id)).toEqual(getPhaseTable('requirement').map((p) => p.id));
  });
});

// ---------------------------------------------------------------------------
// getPhaseTable — error handling
// ---------------------------------------------------------------------------

describe('getPhaseTable — 降级容错', () => {
  it('未知 workflowType 应降级到 requirement 表', () => {
    const phases = getPhaseTable('NONEXISTENT');
    expect(phases.map((p) => p.id)).toEqual(getPhaseTable('requirement').map((p) => p.id));
  });

  it('小写未知 workflowType 也应降级到 requirement 表', () => {
    const phases = getPhaseTable('nonexistent');
    expect(phases.map((p) => p.id)).toEqual(getPhaseTable('requirement').map((p) => p.id));
  });
});

// ---------------------------------------------------------------------------
// getPrerequisites — test-only extended (boundary)
// ---------------------------------------------------------------------------

describe('getPrerequisites — test-only 边界', () => {
  it('空 phase 应返回 []', () => {
    expect(inferPrerequisites('', 'test-only')).toEqual([]);
  });

  it('空 phase 在 requirement 表中也应返回 []', () => {
    expect(inferPrerequisites('', 'requirement')).toEqual([]);
  });

  it('inferPrerequisites("test-execution", "") 使用 requirement 默认表返回 [implement, test-gen]', () => {
    const result = inferPrerequisites('test-execution', '');
    expect(result).toEqual(expect.arrayContaining(['test-gen', 'implement']));
    expect(result).toHaveLength(2);
  });

  it('未知 phase 99-unknown 应返回 []（容错）', () => {
    expect(inferPrerequisites('99-unknown', 'test-only')).toEqual([]);
  });

  it('未知 phase 在 test-only 表中应返回 []', () => {
    expect(inferPrerequisites('99-unknown', 'test-only')).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// PHASE_TEST_ONLY — code-analyze 不含 WORKFLOW_CONTEXT
// ---------------------------------------------------------------------------

describe('PHASE_TEST_ONLY — code-analyze prompt 不含 WORKFLOW_CONTEXT', () => {
  it('code-analyze evaluator prompt 不应包含 WORKFLOW_CONTEXT', () => {
    const prompt = getPhaseTable('test-only').find((p) => p.id === 'code-analyze')!.evaluator!
      .prompt;
    expect(prompt).not.toMatch(/WORKFLOW_CONTEXT/);
  });
});
