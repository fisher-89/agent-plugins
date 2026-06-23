/**
 * Test skeletons for config.schema.ts — Zod schema validation for openspec/config.json.
 *
 * Covers:
 * - `test.framework` enum validation (AC-1) and string rejection
 * - `test.coverage` flat defaults and partial config (AC-2)
 * - `test.overrides` file-level framework/coverage overrides (AC-3)
 * - Invalid framework name rejection
 * - Boundary: empty `test` node, missing coverage, empty string fields
 */

import { describe, it, expect } from 'vite-plus/test';

import { configSchema } from './config.schema';

// ---------------------------------------------------------------------------
// Test Data
// ---------------------------------------------------------------------------

const fullConfig = {
  schema: 'spec-driven' as const,
  test: {
    framework: 'vitest' as const,
    coverage: { lines: 90, branches: 80, functions: 85 },
    overrides: [{ file: 'demo/**', framework: 'vite-plus' as const, coverage: { lines: 60 } }],
  },
};

const minimalConfig = {
  schema: 'spec-driven' as const,
  test: {
    framework: 'vitest' as const,
  },
};

// ===========================================================================
// test.framework — enum validation
// ===========================================================================

describe('test.framework enum validation', () => {
  it('should accept all valid framework enum values', () => {
    const validFrameworks = ['jest', 'vitest', 'vite-plus', 'bun', 'rust'];
    for (const fw of validFrameworks) {
      const result = configSchema.safeParse({
        schema: 'spec-driven',
        test: { framework: fw },
      });
      expect(result.success).toBe(true);
    }
  });

  it('should reject invalid framework name "mocha"', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { framework: 'mocha' },
    });
    expect(result.success).toBe(false);
  });

  it('should reject empty string as framework name', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { framework: '' },
    });
    expect(result.success).toBe(false);
  });

  it('should reject non-string framework value', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: { framework: 123 },
    });
    expect(result.success).toBe(false);
  });

  it('should pass when framework is not set', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {},
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.test?.framework).toBeUndefined();
    }
  });

  it('应接受 node-test、go、pytest 三个新枚举值 (AC-1)', () => {
    for (const fw of ['node-test', 'go', 'pytest']) {
      const result = configSchema.safeParse({
        schema: 'spec-driven',
        test: { framework: fw },
      });
      expect(result.success).toBe(true);
    }
  });
});

// ===========================================================================
// test.coverage — flat defaults and partial config
// ===========================================================================

describe('test.coverage defaults', () => {
  it('should fill all coverage dimensions with defaults when coverage not configured', () => {
    const parsed = configSchema.parse(minimalConfig);
    expect(parsed.test?.coverage).toEqual({
      lines: 80,
      branches: 70,
      functions: 75,
    });
  });

  it('should fill all dimensions with defaults when coverage is empty object', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        coverage: {},
      },
    });
    expect(parsed.test?.coverage).toEqual({
      lines: 80,
      branches: 70,
      functions: 75,
    });
  });

  it('should fill missing dimensions with defaults when coverage is partial', () => {
    const parsed = configSchema.parse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        coverage: { lines: 90 },
      },
    });
    expect(parsed.test?.coverage).toEqual({
      lines: 90,
      branches: 70,
      functions: 75,
    });
  });

  it('should reject coverage dimensions with non-numeric values', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        coverage: { lines: 'high' },
      },
    });
    expect(result.success).toBe(false);
  });

  it('should accept empty test node and fill coverage defaults', () => {
    const result = configSchema.safeParse({ schema: 'spec-driven', test: {} });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.test?.coverage).toEqual({
        lines: 80,
        branches: 70,
        functions: 75,
      });
    }
  });

  it('should fill defaults even when test node is missing entirely', () => {
    const result = configSchema.safeParse({ schema: 'spec-driven' });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.test).toBeDefined();
      expect(result.data.test?.coverage).toEqual({
        lines: 80,
        branches: 70,
        functions: 75,
      });
    }
  });
});

// ===========================================================================
// test.overrides — file-level framework and coverage overrides
// ===========================================================================

describe('test.overrides', () => {
  it('should accept overrides with framework and coverage', () => {
    const parsed = configSchema.parse(fullConfig);
    expect(parsed.test?.overrides).toHaveLength(1);
    expect(parsed.test?.overrides![0]).toEqual({
      file: 'demo/**',
      framework: 'vite-plus',
      coverage: { lines: 60, branches: 70, functions: 75 },
    });
  });

  it('should accept override with only file (no framework or coverage)', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'src/**' }],
      },
    });
    expect(result.success).toBe(true);
  });

  it('should accept override with file and partial coverage', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'demo/**', coverage: { lines: 60 } }],
      },
    });
    expect(result.success).toBe(true);
  });

  it('should reject override missing file field', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ framework: 'jest' }],
      },
    });
    expect(result.success).toBe(false);
  });

  it('should reject override with empty file string', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: '' }],
      },
    });
    expect(result.success).toBe(false);
  });

  it('should reject override framework with invalid enum value', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'src/**', framework: 'mocha' }],
      },
    });
    expect(result.success).toBe(false);
  });

  it('should reject override coverage with non-numeric value', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'demo/**', coverage: { lines: 'high' } }],
      },
    });
    expect(result.success).toBe(false);
  });

  it('should accept multiple overrides', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [
          { file: 'src/**', framework: 'jest' },
          { file: 'tests/**/*.rs', framework: 'rust' },
        ],
      },
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.test?.overrides).toHaveLength(2);
    }
  });

  it('overrides 条目中 framework 为 go / node-test / pytest 时通过验证 (AC-1)', () => {
    for (const fw of ['go', 'node-test', 'pytest']) {
      const result = configSchema.safeParse({
        schema: 'spec-driven',
        test: {
          framework: 'vitest',
          overrides: [{ file: 'src/**', framework: fw }],
        },
      });
      expect(result.success).toBe(true);
    }
  });
});

// ===========================================================================
// configSchema.parse / configSchema.safeParse — general behaviour
// ===========================================================================

describe('configSchema.parse general behaviour', () => {
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

  it('should return success: true from safeParse for valid data', () => {
    const result = configSchema.safeParse({ schema: 'spec-driven' });
    expect(result.success).toBe(true);
  });

  it('should return success: false from safeParse for invalid data', () => {
    const result = configSchema.safeParse({ schema: 123 });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error).toBeDefined();
    }
  });
});

describe('default value cascade', () => {
  it('should fill all nested defaults from empty object', () => {
    const parsed = configSchema.parse({});
    expect(parsed.test?.coverage?.branches).toBe(70);
    expect(parsed.schema).toBe('spec-driven');
  });
});
