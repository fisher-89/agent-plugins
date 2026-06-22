import { describe, it, expect } from 'vite-plus/test';

import { getPhaseIndex, getDependents } from './workflow';

describe('getPhaseIndex', () => {
  it('should return 0 for first phase', () => {
    expect(getPhaseIndex('01-proposal')).toBe(0);
  });

  it('should return 3 for 05-implement（AC-1）', () => {
    expect(getPhaseIndex('05-implement')).toBe(3);
  });

  it('should return 4 for 04-test-gen（AC-1）', () => {
    expect(getPhaseIndex('04-test-gen')).toBe(4);
  });

  it('should return 5 for 06-unit-test（索引随 04/05 对调后仍正确）', () => {
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
});
