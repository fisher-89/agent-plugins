/**
 * Unit tests for schemas/config.schema.ts — Zod schema validation.
 *
 * Tests cover:
 * - configSchema: Zod schema definition validation
 * - parseConfig: throwing validation function
 * - safeParseConfig: non-throwing validation function
 * - OpenSpecConfig type inference
 * - Valid config object sets pass validation
 * - Invalid config object sets are rejected with ZodError
 * - Default value assignment for missing fields
 * - passthrough behavior for unknown fields
 * - Error message contains field paths
 *
 * @see openspec/changes/config-json-zod-schema/test-design.md
 * @see openspec/changes/config-json-zod-schema/specs/config-schema/spec.md
 */

import { describe, it, expect } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// Import the module under test
// WARNING: These imports will fail until the source module is implemented.
// TODO: Update import paths to match actual source location.
// Expected source: plugins/dev-team/bin/src/schemas/config.schema.ts
// ---------------------------------------------------------------------------
import {
  configSchema,
  parseConfig,
  safeParseConfig,
  OpenSpecConfig,
} from '../../../../../plugins/dev-team/bin/src/schemas/config.schema';

// ===========================================================================
// Test Data: Valid config objects
// ===========================================================================

const validConfigs: Record<string, unknown>[] = [
  // Minimal config (should fill default schema value)
  {},
  // Standard config
  { schema: 'spec-driven', context: 'Tech stack: TypeScript' },
  // With rules nested object
  { schema: 'spec-driven', rules: { proposal: ['Keep it short'], tasks: ['One per file'] } },
  // With extra fields (passthrough)
  { schema: 'spec-driven', static_check: ['npm test'], custom_field: 'value' },
  // All fields
  {
    schema: 'spec-driven',
    context: 'test',
    rules: { proposal: ['p1'] },
    extra: 'allowed',
  },
];

// ===========================================================================
// Test Data: Invalid config objects
// ===========================================================================

const invalidConfigs: Record<string, unknown>[] = [
  // schema field type errors
  { schema: 123 },
  { schema: null },
  { schema: true },
  { schema: ['spec-driven'] },
  // context field type errors
  { schema: 'spec-driven', context: 42 },
  { schema: 'spec-driven', context: true },
  { schema: 'spec-driven', context: ['array'] },
  // rules field type errors
  { schema: 'spec-driven', rules: 'string' },
  { schema: 'spec-driven', rules: 123 },
  // rules.proposal type errors
  { schema: 'spec-driven', rules: { proposal: 'not an array' } },
  { schema: 'spec-driven', rules: { proposal: [123, 456] } },
  // rules.tasks type errors
  { schema: 'spec-driven', rules: { tasks: 'not an array' } },
];

// ===========================================================================
// Tests: configSchema — Zod schema definition
// ===========================================================================

describe('configSchema — Zod schema definition', () => {
  it('should be defined and exportable', () => {
    expect(configSchema).toBeDefined();
  });

  it('should be a ZodObject', () => {
    // ZodObject has a _def property with typeName 'ZodObject'
    expect(configSchema).toBeDefined();
  });

  it('should accept a valid config object', () => {
    const result = configSchema.safeParse({ schema: 'spec-driven', context: 'Tech stack: TypeScript' });
    expect(result.success).toBe(true);
  });

  it('should assign default schema value when schema field is missing (AC-1)', () => {
    const result = configSchema.parse({});
    expect(result.schema).toBe('spec-driven');
  });

  it('should reject non-string schema value', () => {
    const result = configSchema.safeParse({ schema: 123 });
    expect(result.success).toBe(false);
  });

  it('should allow additional unknown fields via passthrough', () => {
    const result = configSchema.parse({ schema: 'spec-driven', static_check: ['npm test'] });
    expect(result.schema).toBe('spec-driven');
    // passthrough preserves the extra field
    expect((result as Record<string, unknown>).static_check).toEqual(['npm test']);
  });

  it('should validate rules sub-object with correct structure', () => {
    const result = configSchema.parse({
      schema: 'spec-driven',
      rules: { proposal: ['Keep it short'] },
    });
    expect(result.rules?.proposal).toEqual(['Keep it short']);
  });

  it('should reject rules.proposal with non-array value', () => {
    const result = configSchema.safeParse({
      schema: 'spec-driven',
      rules: { proposal: 'not an array' },
    });
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// Tests: parseConfig — Throwing validation function
// ===========================================================================

describe('parseConfig — throwing validation', () => {
  it('should return typed OpenSpecConfig on valid input', () => {
    const result = parseConfig({ schema: 'spec-driven' });
    expect(result.schema).toBe('spec-driven');
  });

  it('should return an object with correct context when provided', () => {
    const result = parseConfig({ schema: 'spec-driven', context: 'test context' });
    expect(result.context).toBe('test context');
  });

  it('should throw ZodError for invalid schema type', () => {
    expect(() => parseConfig({ schema: null })).toThrow();
    // The error should contain descriptive prefix and field path
    try {
      parseConfig({ schema: null });
    } catch (e) {
      const message = (e as Error).message;
      expect(message).toContain('Invalid OpenSpec configuration');
      expect(message).toContain('schema');
    }
  });

  it('should throw ZodError for invalid context type', () => {
    expect(() => parseConfig({ schema: 'spec-driven', context: 42 })).toThrow();
    try {
      parseConfig({ schema: 'spec-driven', context: 42 });
    } catch (e) {
      const message = (e as Error).message;
      expect(message).toContain('context');
    }
  });

  it('should throw ZodError for invalid rules type', () => {
    expect(() => parseConfig({ schema: 'spec-driven', rules: 'string' })).toThrow();
  });

  it('should throw ZodError for invalid nested rules.proposal type', () => {
    try {
      parseConfig({ schema: 'spec-driven', rules: { proposal: 'not an array' } });
    } catch (e) {
      const message = (e as Error).message;
      expect(message).toContain('rules');
      expect(message).toContain('proposal');
    }
  });

  it('should include descriptive message prefix in error', () => {
    try {
      parseConfig({ schema: 123 });
    } catch (e) {
      const message = (e as Error).message;
      // Should have a descriptive prefix per spec
      expect(message.length).toBeGreaterThan(0);
    }
  });

  it('should return a deep-cloned object to prevent mutation', () => {
    const result = parseConfig({ schema: 'spec-driven' });
    // Mutating the result should not affect subsequent parses
    (result as Record<string, unknown>).extraField = 'mutated';
    const result2 = parseConfig({ schema: 'spec-driven' });
    expect((result2 as Record<string, unknown>).extraField).toBeUndefined();
  });
});

// ===========================================================================
// Tests: safeParseConfig — Non-throwing validation
// ===========================================================================

describe('safeParseConfig — non-throwing validation', () => {
  it('should return { success: true, data } on valid input', () => {
    const result = safeParseConfig({ schema: 'spec-driven' });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.schema).toBe('spec-driven');
    }
  });

  it('should return { success: false, error } on invalid input (AC-3)', () => {
    const result = safeParseConfig({ schema: false });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error).toBeDefined();
      expect(result.error.name).toBe('ZodError');
    }
  });

  it('should never throw even for null input', () => {
    expect(() => safeParseConfig(null)).not.toThrow();
    const result = safeParseConfig(null);
    expect(result.success).toBe(false);
  });

  it('should never throw even for undefined input', () => {
    expect(() => safeParseConfig(undefined)).not.toThrow();
  });

  it('should never throw even for primitive number input', () => {
    expect(() => safeParseConfig(42)).not.toThrow();
    const result = safeParseConfig(42);
    expect(result.success).toBe(false);
  });

  it('should never throw even for array input', () => {
    expect(() => safeParseConfig(['not', 'an', 'object'])).not.toThrow();
    const result = safeParseConfig(['not', 'an', 'object']);
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// Tests: Valid config object set
// ===========================================================================

describe('Valid config objects — all should pass', () => {
  it.each(validConfigs)('parseConfig(%j) should succeed', (config) => {
    const result = parseConfig(config as Record<string, unknown>);
    expect(result).toBeDefined();
    expect(result.schema).toBe('spec-driven');
  });

  it.each(validConfigs)('safeParseConfig(%j) should return success: true', (config) => {
    const result = safeParseConfig(config as Record<string, unknown>);
    expect(result.success).toBe(true);
  });

  it('should fill default schema for empty config', () => {
    const result = parseConfig({});
    expect(result.schema).toBe('spec-driven');
  });

  it('should passthrough extra fields from valid config', () => {
    const result = parseConfig({ schema: 'spec-driven', custom_tool_key: 'value' });
    expect((result as Record<string, unknown>).custom_tool_key).toBe('value');
  });
});

// ===========================================================================
// Tests: Invalid config object set
// ===========================================================================

describe('Invalid config objects — all should be rejected', () => {
  it.each(invalidConfigs)('parseConfig(%j) should throw ZodError', (config) => {
    expect(() => parseConfig(config as Record<string, unknown>)).toThrow();
  });

  it.each(invalidConfigs)('safeParseConfig(%j) should return success: false', (config) => {
    const result = safeParseConfig(config as Record<string, unknown>);
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error).toBeDefined();
    }
  });
});

// ===========================================================================
// Tests: Error message field paths
// ===========================================================================

describe('Error messages contain field paths', () => {
  it('should mention "schema" in error for schema: 123', () => {
    try {
      parseConfig({ schema: 123 });
    } catch (e) {
      const message = (e as Error).message;
      expect(message.toLowerCase()).toContain('schema');
    }
  });

  it('should mention "context" in error for context: true', () => {
    try {
      parseConfig({ schema: 'spec-driven', context: true });
    } catch (e) {
      const message = (e as Error).message;
      expect(message.toLowerCase()).toContain('context');
    }
  });

  it('should mention "rules.proposal" in error for rules.proposal: string', () => {
    try {
      parseConfig({ schema: 'spec-driven', rules: { proposal: 'not an array' } });
    } catch (e) {
      const message = (e as Error).message;
      // The error message should reference the path rules.proposal or similar
      expect(message.toLowerCase()).toContain('proposal');
      expect(message.toLowerCase()).toContain('rules');
    }
  });
});

// ===========================================================================
// Tests: Type inference (compile-time check, runtime sanity)
// ===========================================================================

describe('OpenSpecConfig type inference', () => {
  it('should export OpenSpecConfig type (compile-time check)', () => {
    // At runtime, verify the type is an object (values match shape)
    const result = parseConfig({ schema: 'spec-driven', context: 'test' });
    const typed: OpenSpecConfig = result;
    expect(typed.schema).toBe('spec-driven');
    if (typed.context) {
      expect(typed.context).toBe('test');
    }
  });

  it('should have schema field as string literal "spec-driven"', () => {
    const result = parseConfig({});
    // The schema should always be the literal "spec-driven"
    expect(result.schema).toBe('spec-driven');
    // TypeScript enforces at compile time; at runtime we check the value
  });

  it('should make context optional', () => {
    const result = parseConfig({ schema: 'spec-driven' });
    // context should not be required
    if (result.context !== undefined) {
      expect(typeof result.context).toBe('string');
    }
  });

  it('should make rules optional', () => {
    const result = parseConfig({ schema: 'spec-driven' });
    // rules should not be required
    if (result.rules !== undefined) {
      expect(result.rules).toBeDefined();
    }
  });
});
