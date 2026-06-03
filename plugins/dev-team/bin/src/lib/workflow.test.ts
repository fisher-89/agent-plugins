import { describe, it, expect } from 'vite-plus/test';

import {
  PHASES,
  PHASE_PREREQUISITES,
  getPhaseIndex,
  getPriorPhases,
  getPrerequisites,
  getDependents,
} from './workflow';

describe('PHASES', () => {
  it('should have exactly 9 phases', () => {
    expect(PHASES.length).toBe(9);
  });

  it('should have correct order', () => {
    const expected = [
      '01-proposal',
      '02-dev-design',
      '03-test-design',
      '04-test-gen',
      '05-implement',
      '06-unit-test',
      '07-code-review',
      '08-integration-test',
      '09-acceptance',
    ];
    expect(PHASES).toEqual(expected);
  });

  it('should include new phases 06-unit-test and 08-integration-test', () => {
    expect(PHASES).toContain('06-unit-test');
    expect(PHASES).toContain('08-integration-test');
  });

  it('should have renamed 05-implementation to 05-implement', () => {
    expect(PHASES).toContain('05-implement');
    expect(PHASES).not.toContain('05-implementation');
  });
});

describe('getPhaseIndex', () => {
  it('should return 0 for first phase', () => {
    expect(getPhaseIndex('01-proposal')).toBe(0);
  });

  it('should return correct index for 05-implement', () => {
    expect(getPhaseIndex('05-implement')).toBe(4);
  });

  it('should return correct index for new 06-unit-test', () => {
    expect(getPhaseIndex('06-unit-test')).toBe(5);
  });

  it('should return correct index for new 08-integration-test', () => {
    expect(getPhaseIndex('08-integration-test')).toBe(7);
  });

  it('should return -1 for unknown phase', () => {
    expect(getPhaseIndex('99-unknown')).toBe(-1);
  });

  it('should return -1 for old phase name', () => {
    expect(getPhaseIndex('05-implementation')).toBe(-1);
  });
});

describe('getPriorPhases', () => {
  it('should return empty array for first phase', () => {
    expect(getPriorPhases('01-proposal')).toEqual([]);
  });

  it('should return 5 prior phases for 06-unit-test', () => {
    const prior = getPriorPhases('06-unit-test');
    expect(prior).toEqual([
      '01-proposal',
      '02-dev-design',
      '03-test-design',
      '04-test-gen',
      '05-implement',
    ]);
  });

  it('should return all phases except the last for 09-acceptance', () => {
    const prior = getPriorPhases('09-acceptance');
    expect(prior.length).toBe(8);
    expect(prior[prior.length - 1]).toBe('08-integration-test');
  });

  it('should return empty array for unknown phase', () => {
    expect(getPriorPhases('99-unknown')).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// PHASE_PREREQUISITES
// ---------------------------------------------------------------------------

describe('PHASE_PREREQUISITES', () => {
  it('should have empty prerequisites for 01-proposal (AC-1)', () => {
    expect(PHASE_PREREQUISITES['01-proposal']).toEqual([]);
  });

  it('should have 01-proposal as prerequisite for 02-dev-design (AC-1)', () => {
    expect(PHASE_PREREQUISITES['02-dev-design']).toEqual(['01-proposal']);
  });

  it('should have 01-proposal and 02-dev-design as prerequisites for 03-test-design (AC-2)', () => {
    expect(PHASE_PREREQUISITES['03-test-design']).toEqual(['01-proposal', '02-dev-design']);
  });

  it('should have 04-test-gen and 05-implement as prerequisites for 07-code-review (AC-3)', () => {
    expect(PHASE_PREREQUISITES['07-code-review']).toEqual(['04-test-gen', '05-implement']);
  });
});

// ---------------------------------------------------------------------------
// getPrerequisites — AC-1, AC-2, AC-3
// ---------------------------------------------------------------------------

describe('getPrerequisites', () => {
  it('should return [] for 01-proposal in requirement workflow', () => {
    expect(getPrerequisites('01-proposal', 'requirement')).toEqual([]);
  });

  it('should return ["01-proposal"] for 02-dev-design requirement (AC-1)', () => {
    expect(getPrerequisites('02-dev-design', 'requirement')).toEqual(['01-proposal']);
  });

  it('should return ["01-proposal", "02-dev-design"] for 03-test-design requirement (AC-2)', () => {
    expect(getPrerequisites('03-test-design', 'requirement')).toEqual([
      '01-proposal',
      '02-dev-design',
    ]);
  });

  it('should return ["04-test-gen", "05-implement"] for 07-code-review requirement (AC-3)', () => {
    expect(getPrerequisites('07-code-review', 'requirement')).toEqual([
      '04-test-gen',
      '05-implement',
    ]);
  });

  it('should return ["02-dev-design"] for 05-implement bug-fix', () => {
    expect(getPrerequisites('05-implement', 'bug-fix')).toEqual(['02-dev-design']);
  });

  it('should return ["07-code-review"] for 09-acceptance bug-fix', () => {
    expect(getPrerequisites('09-acceptance', 'bug-fix')).toEqual(['07-code-review']);
  });

  it('should return [] for unknown phase (fault-tolerant)', () => {
    expect(getPrerequisites('99-unknown')).toEqual([]);
  });

  it('should default to requirement workflow_type', () => {
    expect(getPrerequisites('02-dev-design')).toEqual(['01-proposal']);
  });

  it('should match requirement for refactor workflow_type', () => {
    const req = getPrerequisites('06-unit-test', 'requirement');
    const ref = getPrerequisites('06-unit-test', 'refactor');
    expect(ref).toEqual(req);
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

  it('should return [04-test-gen] for 03-test-design', () => {
    const deps = getDependents('03-test-design', 'requirement');
    expect(deps).toEqual(['04-test-gen']);
  });

  it('should return [06, 07, 08] for 04-test-gen', () => {
    const deps = getDependents('04-test-gen', 'requirement');
    expect(deps).toEqual(['06-unit-test', '07-code-review', '08-integration-test']);
  });

  it('should return [06, 07, 08, 09] for 05-implement', () => {
    const deps = getDependents('05-implement', 'requirement');
    expect(deps).toEqual([
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
});
