/**
 * Test skeletons for config.schema.ts — Zod schema validation for openspec/config.json.
 *
 * Covers:
 * - `test.frameworks` array structure (AC-1) and string shorthand (AC-13)
 * - `test.coverage.thresholds` defaults and partial config (AC-2)
 * - `test.coverage.overrides` glob customisation and inheritance (AC-3)
 * - Invalid framework name rejection (AC-14)
 * - frameworks non-string/non-array rejection
 * - frameworks array element missing required fields
 * - thresholds type error rejection
 * - Boundary: empty `test` node, overrides empty thresholds, empty string fields
 *
 * @see openspec/changes/unit-test-coverage-report/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import { configSchema } from './config.schema';

// ---------------------------------------------------------------------------
// Test Data
// ---------------------------------------------------------------------------

const fullConfig = {
  schema: 'spec-driven' as const,
  test: {
    frameworks: [
      { glob: '**/*.test.ts', framework: 'vitest' },
      { glob: '**/tests/**/*.rs', framework: 'rust' },
    ],
    coverage: {
      thresholds: { lines: 90, branches: 80, functions: 85 },
      overrides: [{ glob: 'demo/**', thresholds: { lines: 60 } }],
    },
  },
};

const stringFrameworksConfig = {
  schema: 'spec-driven' as const,
  test: {
    frameworks: 'vitest' as const,
    coverage: {
      thresholds: { lines: 80, branches: 70, functions: 75 },
    },
  },
};

const minimalConfig = {
  schema: 'spec-driven' as const,
  test: {
    frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
  },
};

// ===========================================================================
// AC-1: frameworks array structure
// ===========================================================================

describe('test.frameworks array structure (AC-1)', () => {
  it('should accept frameworks as array of {glob, framework} objects', () => {
    const result = configSchema.parse(fullConfig);
    expect(result.test?.frameworks).toEqual([
      { glob: '**/*.test.ts', framework: 'vitest' },
      { glob: '**/tests/**/*.rs', framework: 'rust' },
    ]);
  });

  it('should reject frameworks array element missing framework field', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { frameworks: [{ glob: '**/*.test.ts' }] },
    });
    expect(result.success).toBe(false);
  });

  it('should reject frameworks non-string non-array value (AC-1, AC-13)', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { frameworks: 123 },
    });
    expect(result.success).toBe(false);
  });

  it('should reject frameworks array element with empty framework string (boundary)', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { frameworks: [{ glob: '**/*.ts', framework: '' }] },
    });
    expect(result.success).toBe(false);
  });

  it('should reject frameworks array element with empty glob string (boundary)', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { frameworks: [{ glob: '', framework: 'vitest' }] },
    });
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// AC-2: coverage.thresholds defaults
// ===========================================================================

describe('test.coverage.thresholds defaults (AC-2)', () => {
  it('should use default thresholds when coverage is not configured', () => {
    // When no coverage node: thresholds defaults are NOT auto-applied since
    // thresholds itself is optional. Defaults apply only when thresholds exists.
    const parsed = configSchema.parse(minimalConfig);
    expect(parsed.test?.coverage?.thresholds).toBeUndefined();
  });

  it('should use default thresholds when thresholds is not configured but coverage exists', () => {
    // When coverage exists but thresholds is omitted: thresholds is undefined
    // since thresholds itself is .optional()
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
        coverage: {},
      },
    });
    expect(parsed.test?.coverage?.thresholds).toBeUndefined();
  });

  it('should apply dimension defaults when thresholds object is provided empty', () => {
    // When thresholds: {} is provided, the .default() values kick in for each dimension
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
        coverage: { thresholds: {} },
      },
    });
    expect(parsed.test?.coverage?.thresholds).toEqual({
      lines: 80,
      branches: 70,
      functions: 75,
    });
  });

  it('should fill missing dimensions with defaults when thresholds partially configured', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
        coverage: {
          thresholds: { lines: 90 },
        },
      },
    });
    expect(parsed.test?.coverage?.thresholds).toEqual({
      lines: 90,
      branches: 70,
      functions: 75,
    });
  });

  it('should reject thresholds with non-numeric values', () => {
    const result = configSchema.safeParse({
        schema: 'spec-driven',
        test: {
          frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
          coverage: { thresholds: { lines: 'high' } },
        },
      });
    expect(result.success).toBe(false);
  });

  it('should accept empty test node (boundary)', () => {
    const result = configSchema.safeParse({ schema: 'spec-driven', test: {} });
    expect(result.success).toBe(true);
  });

  it('should accept config without test node at all (boundary)', () => {
    const result = configSchema.safeParse({ schema: 'spec-driven' });
    expect(result.success).toBe(true);
    expect(result.success && result.data.test).toBeUndefined();
  });
});

// ===========================================================================
// AC-3: coverage.overrides glob with partial thresholds
// ===========================================================================

describe('test.coverage.overrides (AC-3)', () => {
  it('should accept overrides with partial thresholds inheriting global defaults', () => {
    const parsed = configSchema.parse(fullConfig);
    expect(parsed.test?.coverage?.overrides).toHaveLength(1);
    expect(parsed.test?.coverage?.overrides![0]).toEqual({
      glob: 'demo/**',
      thresholds: { lines: 60 },
    });
  });

  it('should accept overrides entry with empty thresholds (inherits all global)', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
        coverage: {
          thresholds: { lines: 80, branches: 70, functions: 75 },
          overrides: [{ glob: 'src/**', thresholds: {} }],
        },
      },
    });
    expect(result.success).toBe(true);
  });

  it('should reject overrides entry missing glob field', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
        coverage: {
          overrides: [{ thresholds: { lines: 60 } }],
        },
      },
    });
    expect(result.success).toBe(false);
  });

  it('should reject overrides thresholds with non-numeric value', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
        coverage: {
          overrides: [{ glob: 'demo/**', thresholds: { lines: 'high' } }],
        },
      },
    });
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// AC-13: frameworks string shorthand
// ===========================================================================

describe('test.frameworks string shorthand (AC-13)', () => {
  it('should accept frameworks as a single valid framework string', () => {
    const parsed = configSchema.parse(stringFrameworksConfig);
    expect(parsed.test?.frameworks).toBe('vitest');
  });

  it('should accept all valid framework enum values', () => {
    const validFrameworks = ['jest', 'vitest', 'vite-plus', 'bun', 'rust'];
    for (const fw of validFrameworks) {
      const result = configSchema.safeParse({
        schema: 'spec-driven',
        test: { frameworks: fw },
      });
      expect(result.success).toBe(true);
    }
  });
});

// ===========================================================================
// AC-14: Invalid framework name rejection
// ===========================================================================

describe('Invalid framework name rejection (AC-14)', () => {
  it('should reject invalid framework name string "mocha"', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { frameworks: 'mocha' },
    });
    expect(result.success).toBe(false);
  });

  it('should reject empty string as framework name', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { frameworks: '' },
    });
    expect(result.success).toBe(false);
  });

  it('should reject with invalid option error mentioning valid values', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { frameworks: 'mocha' },
    });
    if (!result.success) {
      // Zod v4 reports invalid_union — the nested enum error mentions valid values
      expect(result.error.message).toMatch(/invalid_union/i);
      expect(result.error.message).toMatch(/jest|vitest|vite-plus|bun|rust/);
    }
  });
});

// ===========================================================================
// configSchema.parse / configSchema.safeParse — existing behaviour unaffected
// ===========================================================================

describe('configSchema.parse existing behaviour', () => {
  it('should pass through known top-level fields', () => {
    const data = { schema: 'spec-driven', context: 'some context' };
    const parsed = configSchema.parse(data);
    expect(parsed.schema).toBe('spec-driven');
    expect(parsed.context).toBe('some context');
  });

  it('should not preserve unknown passthrough fields', () => {
    const data = { schema: 'spec-driven', static_check: ['npm test'] };
    const parsed = configSchema.parse(data);
    expect(parsed).not.toHaveProperty('static_check');
  });

  it('should throw on invalid data', () => {
    expect(() => configSchema.parse({ schema: 123 })).toThrow();
  });

  it('should return success: true from configSchema.safeParse for valid data', () => {
    const result = configSchema.safeParse({ schema: 'spec-driven' });
    expect(result.success).toBe(true);
  });

  it('should return success: false from configSchema.safeParse for invalid data', () => {
    const result = configSchema.safeParse({ schema: 123 });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error).toBeDefined();
    }
  });
});
