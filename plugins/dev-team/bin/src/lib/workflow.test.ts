import { describe, it, expect } from 'vite-plus/test';

import { PHASES, getPhaseIndex, getPriorPhases } from './workflow';

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
