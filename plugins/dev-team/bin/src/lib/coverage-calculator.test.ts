/**
 * Tests for coverage-calculator -- weighted average computation,
 * ALL-pass coverage gating, and overrides group validation.
 *
 * Covers:
 * - AC-7: Weighted average across multiple frameworks (lines/branches/functions)
 * - AC-8: ALL logic -- all three dimensions pass (fwd) / one fails (rev)
 * - AC-9: Overrides group validation -- pass and fail scenarios
 * - AC-11: All coverage commands fail -> coverage=null
 * - Boundary: partial framework failure, no matching override glob
 *
 * @see openspec/changes/unit-test-coverage-report/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import {
  computeWeightedAverage,
  checkCoveragePass,
  buildCoverageOverrides,
} from './coverage-calculator';
import type { FrameworkCoverage, Thresholds, CoverageOverrideEntry } from './coverage-calculator';

// ---------------------------------------------------------------------------
// Shared test data
// ---------------------------------------------------------------------------

const thresholds: Thresholds = { lines: 80, branches: 70, functions: 75 };

const multiFramework: FrameworkCoverage[] = [
  { framework: 'vitest', coverage: { lines: 90, branches: 80, functions: 85 }, sourceFileCount: 3 },
  { framework: 'rust', coverage: { lines: 70, branches: 65, functions: 75 }, sourceFileCount: 2 },
];

const coreOverrideFail: CoverageOverrideEntry[] = [
  { glob: 'core/**', thresholds: { lines: 90 }, coverage: { lines: 85, branches: 75, functions: 80 } },
];

// ===========================================================================
// AC-7: Weighted average
// ===========================================================================

describe('computeWeightedAverage -- weighted average (AC-7)', () => {
  it('should compute weighted average across multiple frameworks', () => {
    // vitest: lines=90, branches=80, functions=85 (3 files)
    // rust:   lines=70, branches=65, functions=75 (2 files)
    // expected: lines=(90*3 + 70*2)/5 = 82, branches=(80*3 + 65*2)/5 = 74,
    //           functions=(85*3 + 75*2)/5 = 81
    const result = computeWeightedAverage(multiFramework);
    expect(result).toEqual({ lines: 82, branches: 74, functions: 81 });
  });

  it('should handle single framework (weighted average = its coverage)', () => {
    const single: FrameworkCoverage[] = [
      { framework: 'vitest', coverage: { lines: 90, branches: 80, functions: 85 }, sourceFileCount: 5 },
    ];
    const result = computeWeightedAverage(single);
    expect(result).toEqual({ lines: 90, branches: 80, functions: 85 });
  });

  it('should handle frameworks with equal source file counts', () => {
    const equal: FrameworkCoverage[] = [
      { framework: 'fw1', coverage: { lines: 100, branches: 90, functions: 95 }, sourceFileCount: 3 },
      { framework: 'fw2', coverage: { lines: 80, branches: 70, functions: 75 }, sourceFileCount: 3 },
    ];
    // lines=(100*3 + 80*3)/6 = 90
    const result = computeWeightedAverage(equal);
    expect(result!.lines).toBe(90);
  });

  it('should ignore frameworks with null coverage in weighted average (boundary)', () => {
    const partial: FrameworkCoverage[] = [
      { framework: 'vitest', coverage: { lines: 90, branches: 80, functions: 85 }, sourceFileCount: 3 },
      { framework: 'rust', coverage: null, sourceFileCount: 2 },
    ];
    const result = computeWeightedAverage(partial);
    // Only vitest contributes
    expect(result).toEqual({ lines: 90, branches: 80, functions: 85 });
  });
});

// ===========================================================================
// AC-8: ALL logic -- pass
// ===========================================================================

describe('checkCoveragePass -- ALL pass (AC-8)', () => {
  it('should return true when all three dimensions meet thresholds', () => {
    const result = checkCoveragePass(
      { lines: 82, branches: 74, functions: 81 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(true);
  });

  it('should return true when coverage exactly equals thresholds', () => {
    const result = checkCoveragePass(
      { lines: 80, branches: 70, functions: 75 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(true);
  });

  it('should return true when coverage significantly exceeds thresholds', () => {
    const result = checkCoveragePass(
      { lines: 100, branches: 100, functions: 100 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(true);
  });
});

// ===========================================================================
// Reverse AC-8: ALL logic -- fail
// ===========================================================================

describe('checkCoveragePass -- ALL fail (reverse AC-8)', () => {
  it('should return false when branches below threshold', () => {
    const result = checkCoveragePass(
      { lines: 85, branches: 60, functions: 80 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(false);
    expect(result.failingDimensions).toContain('branches');
  });

  it('should return false when lines below threshold', () => {
    const result = checkCoveragePass(
      { lines: 70, branches: 75, functions: 80 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(false);
  });

  it('should return false when functions below threshold', () => {
    const result = checkCoveragePass(
      { lines: 85, branches: 75, functions: 60 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(false);
  });

  it('should report all failing dimensions in the result', () => {
    const result = checkCoveragePass(
      { lines: 50, branches: 40, functions: 30 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(false);
    expect(result.failingDimensions).toEqual(['lines', 'branches', 'functions']);
  });
});

// ===========================================================================
// AC-9: Overrides group validation
// ===========================================================================

describe('checkCoveragePass -- overrides (AC-9)', () => {
  it('should return true when overrides groups all pass', () => {
    const result = checkCoveragePass(
      { lines: 82, branches: 74, functions: 81 },
      thresholds,
      [
        {
          glob: 'demo/**',
          thresholds: { lines: 60, branches: 70, functions: 75 },
          coverage: { lines: 65, branches: 75, functions: 80 },
          pass: true,
        },
      ],
    );
    expect(result.pass).toBe(true);
    expect(result.overrideResults![0].pass).toBe(true);
  });

  it('should return false when an overrides group fails (reverse AC-9)', () => {
    const result = checkCoveragePass(
      { lines: 82, branches: 74, functions: 81 },
      thresholds,
      [
        {
          glob: 'core/**',
          thresholds: { lines: 90, branches: 70, functions: 75 },
          coverage: { lines: 85, branches: 75, functions: 80 },
          pass: false,
        },
      ],
    );
    expect(result.pass).toBe(false);
    expect(result.overrideResults).toHaveLength(1);
    expect(result.overrideResults![0].glob).toBe('core/**');
    expect(result.overrideResults![0].pass).toBe(false);
  });

  it('should inherit global defaults for missing override threshold dimensions', () => {
    // Override only specifies lines=60, should inherit branches and functions from global
    const overrides = buildCoverageOverrides(
      [],
      [
        {
          glob: 'demo/**',
          thresholds: { lines: 60 },
          coverage: { lines: 65, branches: 75, functions: 80 },
        },
      ],
      thresholds,
    );
    expect(overrides[0].thresholds).toEqual({ lines: 60, branches: 70, functions: 75 });
    expect(overrides[0].pass).toBe(true);
  });

  it('should pass overrides group when glob matches no files (reverse AC-9)', () => {
    const result = checkCoveragePass(
      { lines: 82, branches: 74, functions: 81 },
      thresholds,
      [
        {
          glob: 'nonexistent/**',
          thresholds: { lines: 90, branches: 70, functions: 75 },
          coverage: null,
          pass: true,
        },
      ],
    );
    expect(result.pass).toBe(true);
    expect(result.overrideResults![0].pass).toBe(true);
  });
});

// ===========================================================================
// Reverse AC-7/AC-11: All coverage commands fail
// ===========================================================================

describe('checkCoveragePass -- all coverage null (reverse AC-7, AC-11)', () => {
  it('should return coverage=null and coverage_pass=false when all frameworks return null', () => {
    const allNull: FrameworkCoverage[] = [
      { framework: 'vitest', coverage: null, sourceFileCount: 3 },
      { framework: 'rust', coverage: null, sourceFileCount: 2 },
    ];
    const coverage = computeWeightedAverage(allNull);
    expect(coverage).toBeNull();
    const result = checkCoveragePass(null, thresholds, []);
    expect(result.pass).toBe(false);
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('coverage-calculator -- edge cases', () => {
  it('should handle empty coverage_by_framework array', () => {
    const result = computeWeightedAverage([]);
    expect(result).toBeNull();
  });

  it('should handle sourceFileCount of 0 gracefully', () => {
    const result = computeWeightedAverage([
      { framework: 'vitest', coverage: { lines: 90, branches: 80, functions: 85 }, sourceFileCount: 0 },
    ]);
    expect(result).toBeNull();
  });

  it('should handle negative coverage values (if input is corrupt)', () => {
    const result = checkCoveragePass(
      { lines: -1, branches: 80, functions: 85 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(false);
  });

  it('should handle coverage values above 100 (passes threshold)', () => {
    const result = checkCoveragePass(
      { lines: 150, branches: 120, functions: 200 },
      thresholds,
      [],
    );
    expect(result.pass).toBe(true);
  });

  it('should handle nullish thresholds gracefully', () => {
    const result = checkCoveragePass(
      { lines: 80, branches: 70, functions: 75 },
      null,
      [],
    );
    expect(result.pass).toBe(true);
  });

  it('should handle nullish overrides array', () => {
    const result = checkCoveragePass(
      { lines: 82, branches: 74, functions: 81 },
      thresholds,
      null,
    );
    expect(result.pass).toBe(true);
  });

  it('should handle floating point coverage values in weighted average', () => {
    const result = computeWeightedAverage([
      { framework: 'vitest', coverage: { lines: 85.3, branches: 74.7, functions: 80.1 }, sourceFileCount: 2 },
      { framework: 'rust', coverage: { lines: 70.5, branches: 65.2, functions: 75.8 }, sourceFileCount: 3 },
    ]);
    // Math.round is used: lines = (85.3*2 + 70.5*3)/5 = (170.6+211.5)/5 = 382.1/5 = 76.42 -> 76
    expect(result!.lines).toBe(76);
    expect(result!.branches).toBe(69); // (74.7*2 + 65.2*3)/5 = (149.4+195.6)/5 = 345/5 = 69 -> 69
    expect(result!.functions).toBe(78); // (80.1*2 + 75.8*3)/5 = (160.2+227.4)/5 = 387.6/5 = 77.52 -> 78
  });

  it('should handle very large sourceFileCount values', () => {
    const large: FrameworkCoverage[] = [
      { framework: 'vitest', coverage: { lines: 90, branches: 80, functions: 85 }, sourceFileCount: Number.MAX_SAFE_INTEGER },
    ];
    const result = computeWeightedAverage(large);
    expect(result).toEqual({ lines: 90, branches: 80, functions: 85 });
  });

  it('should handle overrides with extremely large number of entries (>100)', () => {
    const manyOverrides = Array.from({ length: 200 }, (_, i) => ({
      glob: `dir${i}/**`,
      thresholds: { lines: 80, branches: 70, functions: 75 } as Thresholds,
      coverage: { lines: 85, branches: 75, functions: 80 },
      pass: true,
    }));
    const result = checkCoveragePass(
      { lines: 80, branches: 70, functions: 75 },
      thresholds,
      manyOverrides,
    );
    expect(result.pass).toBe(true);
  });
});

// ===========================================================================
// buildCoverageOverrides
// ===========================================================================

describe('buildCoverageOverrides', () => {
  it('should build override results from config entries', () => {
    const result = buildCoverageOverrides(
      [],
      [
        { glob: 'demo/**', thresholds: { lines: 60 }, coverage: { lines: 65, branches: 75, functions: 80 } },
      ],
      thresholds,
    );
    expect(result).toHaveLength(1);
    expect(result[0].glob).toBe('demo/**');
    expect(result[0].pass).toBe(true);
  });

  it('should return empty array when no overrides', () => {
    const result = buildCoverageOverrides([], [], thresholds);
    expect(result).toEqual([]);
  });

  it('should handle undefined overrides gracefully', () => {
    const result = buildCoverageOverrides([], undefined as unknown as CoverageOverrideEntry[], thresholds);
    expect(result).toEqual([]);
  });

  it('should mark pass=true when override has no coverage data (no matching files)', () => {
    const result = buildCoverageOverrides(
      [],
      [{ glob: 'nonexistent/**', thresholds: { lines: 90 } }],
      thresholds,
    );
    expect(result[0].pass).toBe(true);
    expect(result[0].coverage).toBeNull();
  });
});
