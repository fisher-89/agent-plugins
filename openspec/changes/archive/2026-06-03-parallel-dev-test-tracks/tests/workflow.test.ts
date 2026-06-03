/**
 * Unit tests for workflow.ts — prerequisite dependency table and dependents graph.
 *
 * Covers:
 * - AC-1: getPrerequisites("02-dev-design") returns ["01-proposal"]
 * - AC-2: getPrerequisites("03-test-design") returns ["01-proposal", "02-dev-design"]
 * - AC-3: getPrerequisites("07-code-review") returns ["04-test-gen", "05-implement"]
 * - AC-4: getDependents("02-dev-design") returns ["03-test-design", "05-implement", "09-acceptance"]
 * - Boundary: root phase, unknown phase, bug-fix workflow, default workflow_type, consistency
 *
 * @see openspec/changes/parallel-dev-test-tracks/specs/workflow-orchestration/spec.md
 * @see openspec/changes/parallel-dev-test-tracks/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import {
  getPrerequisites,
  getDependents,
  PHASE_PREREQUISITES,
} from '../../../../plugins/dev-team/bin/src/lib/workflow';

// ---------------------------------------------------------------------------
// PHASE_PREREQUISITES constant
// ---------------------------------------------------------------------------

describe('PHASE_PREREQUISITES', () => {
  it('should define prerequisites for all 9 phases in requirement workflow', () => {
    // TODO: verify PHASE_PREREQUISITES has entries for all 9 phase IDs
    expect(Object.keys(PHASE_PREREQUISITES).length).toBeGreaterThanOrEqual(9);
  });

  it('should have 01-proposal with empty prerequisites', () => {
    // TODO: verify PHASE_PREREQUISITES['01-proposal'] === []
    expect(PHASE_PREREQUISITES['01-proposal']).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// getPrerequisites — requirement workflow
// ---------------------------------------------------------------------------

describe('getPrerequisites (requirement workflow)', () => {
  it('should return empty array for root phase 01-proposal (AC-1)', () => {
    expect(getPrerequisites('01-proposal', 'requirement')).toEqual([]);
  });

  it('should return ["01-proposal"] for 02-dev-design (AC-1)', () => {
    expect(getPrerequisites('02-dev-design', 'requirement')).toEqual(['01-proposal']);
  });

  it('should return ["01-proposal", "02-dev-design"] for 03-test-design (AC-2)', () => {
    expect(getPrerequisites('03-test-design', 'requirement')).toEqual(['01-proposal', '02-dev-design']);
  });

  it('should return ["03-test-design"] for 04-test-gen', () => {
    expect(getPrerequisites('04-test-gen', 'requirement')).toEqual(['03-test-design']);
  });

  it('should return ["02-dev-design"] for 05-implement', () => {
    expect(getPrerequisites('05-implement', 'requirement')).toEqual(['02-dev-design']);
  });

  it('should return ["04-test-gen", "05-implement"] for 06-unit-test', () => {
    expect(getPrerequisites('06-unit-test', 'requirement')).toEqual(['04-test-gen', '05-implement']);
  });

  it('should return ["04-test-gen", "05-implement"] for 07-code-review (AC-3)', () => {
    expect(getPrerequisites('07-code-review', 'requirement')).toEqual(['04-test-gen', '05-implement']);
  });

  it('should return ["04-test-gen", "05-implement"] for 08-integration-test', () => {
    expect(getPrerequisites('08-integration-test', 'requirement')).toEqual(['04-test-gen', '05-implement']);
  });

  it('should return ["01-proposal", "02-dev-design", "05-implement"] for 09-acceptance', () => {
    expect(getPrerequisites('09-acceptance', 'requirement')).toEqual(['01-proposal', '02-dev-design', '05-implement']);
  });
});

// ---------------------------------------------------------------------------
// getPrerequisites — bug-fix workflow
// ---------------------------------------------------------------------------

describe('getPrerequisites (bug-fix workflow)', () => {
  it('should return ["02-dev-design"] for 05-implement in bug-fix', () => {
    expect(getPrerequisites('05-implement', 'bug-fix')).toEqual(['02-dev-design']);
  });

  it('should return ["01-proposal"] for 02-dev-design in bug-fix', () => {
    expect(getPrerequisites('02-dev-design', 'bug-fix')).toEqual(['01-proposal']);
  });

  it('should return ["07-code-review"] for 09-acceptance in bug-fix', () => {
    // TODO: verify exact value; spec says bug-fix 09-acceptance depends on 07-code-review
    const prereqs = getPrerequisites('09-acceptance', 'bug-fix');
    expect(prereqs).toContain('07-code-review');
  });
});

// ---------------------------------------------------------------------------
// getPrerequisites — refactor workflow (matches requirement)
// ---------------------------------------------------------------------------

describe('getPrerequisites (refactor workflow)', () => {
  it('should match requirement workflow for refactor', () => {
    const reqPrereqs = getPrerequisites('06-unit-test', 'requirement');
    const refPrereqs = getPrerequisites('06-unit-test', 'refactor');
    expect(refPrereqs).toEqual(reqPrereqs);
  });
});

// ---------------------------------------------------------------------------
// getPrerequisites — boundary and fault tolerance
// ---------------------------------------------------------------------------

describe('getPrerequisites — boundary cases', () => {
  it('should return empty array for unknown phase ID', () => {
    expect(getPrerequisites('99-invalid')).toEqual([]);
  });

  it('should default to requirement workflow when workflow_type is omitted', () => {
    const withDefault = getPrerequisites('06-unit-test');
    const explicit = getPrerequisites('06-unit-test', 'requirement');
    expect(withDefault).toEqual(explicit);
  });

  it('should default to requirement workflow for unknown workflow_type', () => {
    const withDefault = getPrerequisites('06-unit-test', 'unknown-workflow');
    const explicit = getPrerequisites('06-unit-test', 'requirement');
    expect(withDefault).toEqual(explicit);
  });
});

// ---------------------------------------------------------------------------
// getDependents — requirement workflow
// ---------------------------------------------------------------------------

describe('getDependents (requirement workflow)', () => {
  it('should return [02-dev-design, 03-test-design, 09-acceptance] for 01-proposal', () => {
    const deps = getDependents('01-proposal', 'requirement');
    expect(deps).toContain('02-dev-design');
    expect(deps).toContain('03-test-design');
    expect(deps).toContain('09-acceptance');
    expect(deps.length).toBe(3);
  });

  it('should return [03-test-design, 05-implement, 09-acceptance] for 02-dev-design (AC-4)', () => {
    const deps = getDependents('02-dev-design', 'requirement');
    expect(deps).toContain('03-test-design');
    expect(deps).toContain('05-implement');
    expect(deps).toContain('09-acceptance');
    expect(deps.length).toBe(3);
  });

  it('should return [04-test-gen] for 03-test-design', () => {
    expect(getDependents('03-test-design', 'requirement')).toEqual(['04-test-gen']);
  });

  it('should return [06-unit-test, 07-code-review, 08-integration-test] for 04-test-gen', () => {
    const deps = getDependents('04-test-gen', 'requirement');
    expect(deps).toContain('06-unit-test');
    expect(deps).toContain('07-code-review');
    expect(deps).toContain('08-integration-test');
    expect(deps.length).toBe(3);
  });

  it('should return [06-unit-test, 07-code-review, 08-integration-test, 09-acceptance] for 05-implement', () => {
    const deps = getDependents('05-implement', 'requirement');
    expect(deps).toContain('06-unit-test');
    expect(deps).toContain('07-code-review');
    expect(deps).toContain('08-integration-test');
    expect(deps).toContain('09-acceptance');
    expect(deps.length).toBe(4);
  });

  it('should return empty array for 06-unit-test (leaf node)', () => {
    expect(getDependents('06-unit-test', 'requirement')).toEqual([]);
  });

  it('should return empty array for 07-code-review (leaf node)', () => {
    expect(getDependents('07-code-review', 'requirement')).toEqual([]);
  });

  it('should return empty array for 08-integration-test (leaf node)', () => {
    expect(getDependents('08-integration-test', 'requirement')).toEqual([]);
  });

  it('should return empty array for 09-acceptance (leaf node)', () => {
    expect(getDependents('09-acceptance', 'requirement')).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// getDependents — bug-fix workflow
// ---------------------------------------------------------------------------

describe('getDependents (bug-fix workflow)', () => {
  it('should return [05-implement] for 02-dev-design in bug-fix (no test track phases)', () => {
    const deps = getDependents('02-dev-design', 'bug-fix');
    // bug-fix: test track phases (03, 04, 08) are absent
    expect(deps).toContain('05-implement');
    expect(deps).not.toContain('03-test-design');
  });
});

// ---------------------------------------------------------------------------
// getDependents — boundary cases
// ---------------------------------------------------------------------------

describe('getDependents — boundary cases', () => {
  it('should return empty array for unknown phase ID', () => {
    expect(getDependents('99-invalid')).toEqual([]);
  });

  it('should default to requirement workflow when workflow_type is omitted', () => {
    const withDefault = getDependents('02-dev-design');
    const explicit = getDependents('02-dev-design', 'requirement');
    expect(withDefault).toEqual(explicit);
  });
});

// ---------------------------------------------------------------------------
// Consistency: getPrerequisites <-> getDependents inverse relationship
// ---------------------------------------------------------------------------

describe('getPrerequisites <-> getDependents consistency', () => {
  const allPhases = [
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

  it('should satisfy inverse relationship for all phases in requirement workflow', () => {
    // For every phase P, getDependents(P) should list exactly those phases Q
    // where P appears in getPrerequisites(Q).
    for (const phase of allPhases) {
      const dependents = getDependents(phase, 'requirement');
      for (const dependent of dependents) {
        const prereqs = getPrerequisites(dependent, 'requirement');
        expect(prereqs).toContain(phase);
      }
      // Also verify the reverse: every phase Q that lists P as prerequisite
      // should appear in getDependents(P).
      for (const candidate of allPhases) {
        const prereqs = getPrerequisites(candidate, 'requirement');
        if (prereqs.includes(phase)) {
          expect(dependents).toContain(candidate);
        }
      }
    }
  });

  it('should satisfy inverse relationship for bug-fix workflow', () => {
    const bugFixPhases = [
      '01-proposal',
      '02-dev-design',
      '05-implement',
      '06-unit-test',
      '07-code-review',
      '09-acceptance',
    ];
    for (const phase of bugFixPhases) {
      const dependents = getDependents(phase, 'bug-fix');
      for (const dependent of dependents) {
        const prereqs = getPrerequisites(dependent, 'bug-fix');
        expect(prereqs).toContain(phase);
      }
      for (const candidate of bugFixPhases) {
        const prereqs = getPrerequisites(candidate, 'bug-fix');
        if (prereqs.includes(phase)) {
          expect(dependents).toContain(candidate);
        }
      }
    }
  });
});
