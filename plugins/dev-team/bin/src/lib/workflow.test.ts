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
// requirement phase table
// ---------------------------------------------------------------------------

describe('PHASE_TABLES', () => {
  it('should have exactly 8 phases for requirement workflow_type', () => {
    expect(getPhaseTable('requirement').length).toBe(8);
  });

  it('should start with proposal (not requirements)', () => {
    expect(getPhaseTable('requirement')[0].id).toBe('proposal');
  });

  it('phase 表 id 顺序与 AC-1 一致 — implement 在 test-gen 之前（AC-1）', () => {
    const phases = getPhaseTable('requirement').map((p) => p.id);
    expect(phases).toEqual([
      'proposal',
      'dev-design',
      'test-design',
      'implement',
      'test-gen',
      'test-execution',
      'code-review',
      'acceptance',
    ]);
  });

  it('should have 6 phases for bug-fix workflow_type', () => {
    const phases = getPhaseTable('bug-fix').map((p) => p.id);
    expect(phases.length).toBe(6);
    // bug-fix skips test-design, test-gen
    expect(phases).not.toContain('test-design');
    expect(phases).not.toContain('test-gen');
  });

  it('should have same 8 phases for refactor workflow_type', () => {
    const req = getPhaseTable('requirement').map((p) => p.id);
    const ref = getPhaseTable('refactor').map((p) => p.id);
    expect(ref).toEqual(req);
  });

  it('should have 5 phases for test-only workflow_type', () => {
    expect(getPhaseTable('test-only')).toHaveLength(5);
  });

  it('requirement 表应不包含 unit-test 和 integration-test', () => {
    const phases = getPhaseTable('requirement').map((p) => p.id);
    expect(phases).not.toContain('unit-test');
    expect(phases).not.toContain('integration-test');
  });

  it('bug-fix 表应不包含 integration-test', () => {
    const phases = getPhaseTable('bug-fix').map((p) => p.id);
    expect(phases).not.toContain('integration-test');
  });

  it('getPhaseTable("UNKNOWN") 应降级到 requirement 表（容错）', () => {
    const phases = getPhaseTable('UNKNOWN');
    expect(phases.map((p) => p.id)).toEqual(getPhaseTable('requirement').map((p) => p.id));
  });

  it('getPhaseTable("") 应降级到 requirement 表（容错）', () => {
    const phases = getPhaseTable('');
    expect(phases.map((p) => p.id)).toEqual(getPhaseTable('requirement').map((p) => p.id));
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
    expect(phase?.executor?.agent_type).toBe('__CALL_AGENT:code-analyze-planner__');
    expect(phase?.evaluator?.agent_type).toBe('__CALL_AGENT:code-analyze-evaluator__');
  });
});

describe('PHASE_TEST_ONLY — prompt customization', () => {
  it('proposal executor prompt differs from requirement (AC-9)', () => {
    const reqPrompt = getPhaseTable('requirement').find((p) => p.id === 'proposal')!.executor!
      .prompt;
    const testPrompt = getPhaseTable('test-only').find((p) => p.id === 'proposal')!.executor!
      .prompt;
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

// ---------------------------------------------------------------------------
// agent tokens / DEFAULT_WORKFLOW / 依赖表精确断言（mutation 补强）
// ---------------------------------------------------------------------------

const AGENT_TOKEN = /^__CALL_AGENT:[a-z0-9-]+__$/;

function collectAgentTypes(workflowType: string): string[] {
  const types: string[] = [];
  for (const phase of getPhaseTable(workflowType)) {
    if (phase.executor) types.push(phase.executor.agent_type);
    if (phase.evaluator) types.push(phase.evaluator.agent_type);
  }
  return types;
}

describe('getPhaseTable / agent tokens（突变补强）', () => {
  it('requirement 全阶段非 null 的 agent_type 精确等于 __CALL_AGENT:<logical-id>__ 清单', () => {
    const expected: Record<string, { executor?: string; evaluator?: string }> = {
      proposal: {
        executor: '__CALL_AGENT:proposal-planner__',
        evaluator: '__CALL_AGENT:proposal-evaluator__',
      },
      'dev-design': {
        executor: '__CALL_AGENT:dev-design-planner__',
        evaluator: '__CALL_AGENT:dev-design-evaluator__',
      },
      'test-design': {
        executor: '__CALL_AGENT:test-design-planner__',
        evaluator: '__CALL_AGENT:test-design-evaluator__',
      },
      implement: {
        executor: '__CALL_AGENT:implementation-generator__',
        evaluator: '__CALL_AGENT:implementation-evaluator__',
      },
      'test-gen': {
        executor: '__CALL_AGENT:test-gen-generator__',
        evaluator: '__CALL_AGENT:test-gen-evaluator__',
      },
      'test-execution': {
        executor: '__CALL_AGENT:test-execution-executor__',
        evaluator: '__CALL_AGENT:test-execution-evaluator__',
      },
      'code-review': { evaluator: '__CALL_AGENT:code-review-evaluator__' },
      acceptance: { evaluator: '__CALL_AGENT:acceptance-evaluator__' },
    };
    for (const phase of getPhaseTable('requirement')) {
      const exp = expected[phase.id];
      expect(exp).toBeDefined();
      if (phase.executor) {
        expect(phase.executor.agent_type).toBe(exp.executor);
      } else {
        expect(exp.executor).toBeUndefined();
      }
      if (phase.evaluator) {
        expect(phase.evaluator.agent_type).toBe(exp.evaluator);
      }
    }
  });

  it('test-only 的 code-analyze / test-design / test-gen / test-execution agent_type 精确匹配', () => {
    const table = getPhaseTable('test-only');
    const byId = Object.fromEntries(table.map((p) => [p.id, p]));
    expect(byId['code-analyze'].executor!.agent_type).toBe('__CALL_AGENT:code-analyze-planner__');
    expect(byId['code-analyze'].evaluator!.agent_type).toBe(
      '__CALL_AGENT:code-analyze-evaluator__',
    );
    expect(byId['test-design'].executor!.agent_type).toBe('__CALL_AGENT:test-design-planner__');
    expect(byId['test-design'].evaluator!.agent_type).toBe('__CALL_AGENT:test-design-evaluator__');
    expect(byId['test-gen'].executor!.agent_type).toBe('__CALL_AGENT:test-gen-generator__');
    expect(byId['test-gen'].evaluator!.agent_type).toBe('__CALL_AGENT:test-gen-evaluator__');
    expect(byId['test-execution'].executor!.agent_type).toBe(
      '__CALL_AGENT:test-execution-executor__',
    );
    expect(byId['test-execution'].evaluator!.agent_type).toBe(
      '__CALL_AGENT:test-execution-evaluator__',
    );
  });

  it('bug-fix / refactor 各阶段 agent_type 全部匹配 /^__CALL_AGENT:[a-z0-9-]+__$/', () => {
    for (const wf of ['bug-fix', 'refactor'] as const) {
      for (const agentType of collectAgentTypes(wf)) {
        expect(agentType).toMatch(AGENT_TOKEN);
        expect(agentType.length).toBeGreaterThan(0);
      }
    }
  });

  it('requirement 与 test-only 的 proposal executor prompt 同时包含 explore.md、merge、Do not expect inline EXPLORE_CONTEXT_SUMMARY', () => {
    for (const wf of ['requirement', 'test-only'] as const) {
      const prompt = getPhaseTable(wf).find((p) => p.id === 'proposal')!.executor!.prompt;
      expect(prompt).toContain('explore.md');
      expect(prompt).toMatch(/merge/i);
      expect(prompt).toContain('Do not expect inline EXPLORE_CONTEXT_SUMMARY');
    }
  });

  it("getPhaseTable('UNKNOWN') / getPhaseTable('') 的 phase id 与 agent_type 深度等于 requirement", () => {
    const req = getPhaseTable('requirement');
    for (const key of ['UNKNOWN', ''] as const) {
      const table = getPhaseTable(key);
      expect(table.map((p) => p.id)).toEqual(req.map((p) => p.id));
      expect(collectAgentTypes(key)).toEqual(collectAgentTypes('requirement'));
    }
  });

  it('四张表 key 均非空；refactor id 序列等于 requirement', () => {
    for (const key of ['requirement', 'bug-fix', 'refactor', 'test-only'] as const) {
      expect(getPhaseTable(key).length).toBeGreaterThan(0);
    }
    expect(getPhaseTable('refactor').map((p) => p.id)).toEqual(
      getPhaseTable('requirement').map((p) => p.id),
    );
  });
});

describe('getDependents / 依赖表精确断言（突变补强）', () => {
  it('requirement：proposal/dev-design/test-design/implement/叶子依赖精确', () => {
    expect(getDependents('proposal', 'requirement')).toEqual([
      'dev-design',
      'test-design',
      'acceptance',
    ]);
    expect(getDependents('dev-design', 'requirement')).toEqual([
      'test-design',
      'implement',
      'acceptance',
    ]);
    expect(getDependents('test-design', 'requirement')).toEqual(['test-gen']);
    expect(getDependents('implement', 'requirement')).toEqual(
      expect.arrayContaining(['test-gen', 'test-execution', 'code-review', 'acceptance']),
    );
    expect(getDependents('implement', 'requirement')).toHaveLength(4);
    for (const leaf of ['test-execution', 'code-review', 'acceptance'] as const) {
      expect(getDependents(leaf, 'requirement')).toEqual([]);
    }
  });

  it('bug-fix：proposal→含 dev-design；implement→含 test-execution/code-review；acceptance 依赖链含 code-review', () => {
    expect(getDependents('proposal', 'bug-fix')).toContain('dev-design');
    expect(getDependents('implement', 'bug-fix')).toEqual(
      expect.arrayContaining(['test-execution', 'code-review']),
    );
    expect(getDependents('code-review', 'bug-fix')).toEqual(['acceptance']);
  });

  it('test-only：proposal/code-analyze/test-design/test-gen 下游精确', () => {
    expect(getDependents('proposal', 'test-only')).toEqual(['code-analyze', 'test-design']);
    expect(getDependents('code-analyze', 'test-only')).toEqual(['test-design']);
    expect(getDependents('test-design', 'test-only')).toEqual(['test-gen']);
    expect(getDependents('test-gen', 'test-only')).toEqual(['test-execution']);
  });

  it('未知 / 空 phase → []；空/UNKNOWN workflowType 回落 requirement', () => {
    expect(getDependents('nope', 'requirement')).toEqual([]);
    expect(getDependents('', 'requirement')).toEqual([]);
    expect(getDependents('proposal', '')).toEqual(getDependents('proposal', 'requirement'));
    expect(getDependents('proposal', 'UNKNOWN')).toEqual(getDependents('proposal', 'requirement'));
  });

  it('对 requirement 每个 phase id，用逆映射重算 prerequisites 与表一致', () => {
    const expectedPrereqs: Record<string, string[]> = {
      proposal: [],
      'dev-design': ['proposal'],
      'test-design': ['proposal', 'dev-design'],
      'test-gen': ['test-design', 'implement'],
      implement: ['dev-design'],
      'test-execution': ['test-gen', 'implement'],
      'code-review': ['test-gen', 'implement'],
      acceptance: ['proposal', 'dev-design', 'implement'],
    };
    for (const phaseId of Object.keys(expectedPrereqs)) {
      const inferred = inferPrerequisites(phaseId, 'requirement');
      expect(inferred.sort()).toEqual([...expectedPrereqs[phaseId]].sort());
    }
  });
});
