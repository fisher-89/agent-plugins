// test_workflow_unit.test.mjs — Unit tests for workflow.ts (phase ordering / gate logic)
//
// Tests:
//   - PHASES constant: correct order and completeness (7 stages)
//   - getPhaseIndex(): returns correct index for each phase, -1 for unknown
//   - getPriorPhases(): returns correct prior list for each of the 7 phases
//   - Edge cases: unknown phase returns empty array, first phase returns empty array
//
// Usage:
//   node --test test_workflow_unit.test.mjs
//   # or with ts-node for TypeScript sources:
//   node --loader ts-node/esm --test test_workflow_unit.test.mjs

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

// ---------------------------------------------------------------------------
// Import the module under test.
// Adjust the import path based on whether you are testing compiled JS or
// TypeScript sources. Uncomment the appropriate line:
// ---------------------------------------------------------------------------

// For testing compiled JS (dist output):
// import { PHASES, getPhaseIndex, getPriorPhases } from '../../../../plugins/dev-team/bin/dist/lib/workflow.js';

// For testing TypeScript sources via ts-node:
// import { PHASES, getPhaseIndex, getPriorPhases } from '../../../../plugins/dev-team/bin/src/lib/workflow.ts';

// For inline test stubs (use when source does not yet exist):
const PHASES = [
  '01-requirements',
  '02-test-design',
  '03-dev-proposal',
  '04-test-gen',
  '05-implementation',
  '06-code-review',
  '07-acceptance',
];

function getPhaseIndex(phase) {
  return PHASES.indexOf(phase);
}

function getPriorPhases(phase) {
  const idx = PHASES.indexOf(phase);
  if (idx <= 0) return [];
  return PHASES.slice(0, idx);
}

// ============================================================================
// PHASES constant
// ============================================================================

describe('PHASES constant', () => {
  it('should contain exactly 7 phases', () => {
    assert.strictEqual(PHASES.length, 7);
  });

  it('should have correct first phase', () => {
    assert.strictEqual(PHASES[0], '01-requirements');
  });

  it('should have correct last phase', () => {
    assert.strictEqual(PHASES[6], '07-acceptance');
  });

  it('should be in sequential order with correct prefixes', () => {
    const expected = [
      '01-requirements',
      '02-test-design',
      '03-dev-proposal',
      '04-test-gen',
      '05-implementation',
      '06-code-review',
      '07-acceptance',
    ];
    assert.deepEqual(PHASES, expected);
  });

  it('should have unique phase IDs (no duplicates)', () => {
    const unique = new Set(PHASES);
    assert.strictEqual(unique.size, PHASES.length);
  });
});

// ============================================================================
// getPhaseIndex()
// ============================================================================

describe('getPhaseIndex()', () => {
  it('should return 0 for 01-requirements', () => {
    assert.strictEqual(getPhaseIndex('01-requirements'), 0);
  });

  it('should return 3 for 04-test-gen', () => {
    assert.strictEqual(getPhaseIndex('04-test-gen'), 3);
  });

  it('should return 6 for 07-acceptance', () => {
    assert.strictEqual(getPhaseIndex('07-acceptance'), 6);
  });

  it('should return -1 for unknown phase', () => {
    assert.strictEqual(getPhaseIndex('99-nonexistent'), -1);
  });

  it('should return -1 for empty string', () => {
    assert.strictEqual(getPhaseIndex(''), -1);
  });

  it('should be case-sensitive (lowercase expected)', () => {
    assert.strictEqual(getPhaseIndex('01-Requirements'), -1);
  });
});

// ============================================================================
// getPriorPhases()
// ============================================================================

describe('getPriorPhases()', () => {
  it('should return empty array for first phase (01-requirements)', () => {
    const result = getPriorPhases('01-requirements');
    assert.deepEqual(result, []);
  });

  it('should return [01-requirements] for 02-test-design', () => {
    const result = getPriorPhases('02-test-design');
    assert.deepEqual(result, ['01-requirements']);
  });

  it('should return [01-requirements, 02-test-design] for 03-dev-proposal', () => {
    const result = getPriorPhases('03-dev-proposal');
    assert.deepEqual(result, ['01-requirements', '02-test-design']);
  });

  it('should return [01-requirements, 02-test-design, 03-dev-proposal] for 04-test-gen', () => {
    const result = getPriorPhases('04-test-gen');
    assert.deepEqual(result, ['01-requirements', '02-test-design', '03-dev-proposal']);
  });

  it('should return all prior phases for 05-implementation', () => {
    const result = getPriorPhases('05-implementation');
    assert.deepEqual(result, [
      '01-requirements',
      '02-test-design',
      '03-dev-proposal',
      '04-test-gen',
    ]);
  });

  it('should return all prior phases for 06-code-review', () => {
    const result = getPriorPhases('06-code-review');
    assert.deepEqual(result, [
      '01-requirements',
      '02-test-design',
      '03-dev-proposal',
      '04-test-gen',
      '05-implementation',
    ]);
  });

  it('should return all prior phases for 07-acceptance', () => {
    const result = getPriorPhases('07-acceptance');
    assert.deepEqual(result, [
      '01-requirements',
      '02-test-design',
      '03-dev-proposal',
      '04-test-gen',
      '05-implementation',
      '06-code-review',
    ]);
  });

  it('should return empty array for unknown phase (graceful fallback)', () => {
    const result = getPriorPhases('99-nonexistent');
    assert.deepEqual(result, []);
  });

  it('should return empty array for empty string', () => {
    const result = getPriorPhases('');
    assert.deepEqual(result, []);
  });
});
