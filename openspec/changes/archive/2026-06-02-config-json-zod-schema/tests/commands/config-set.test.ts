/**
 * Unit and integration tests for config/set MCP tool command.
 *
 * Unit tests cover:
 * - Command layer parameter handling and response formatting
 * - project_root parameter forwarding
 * - Schema validation
 * - AC-7: String values are no longer type-inferred
 *
 * Integration tests cover:
 * - Real filesystem auto-creation of skeleton
 * - Idempotent writes (same key/value twice)
 * - Overwrite idempotency (same key, different value)
 * - JSON file content verification
 *
 * @see openspec/changes/config-json-zod-schema/test-design.md
 * @see openspec/changes/config-json-zod-schema/specs/config-set/spec.md
 */

import { describe, it, expect, beforeAll, afterAll } from 'vite-plus/test';
import { z } from 'zod/v4';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { runConfigSet } from '../../../../../plugins/dev-team/bin/src/commands/config-set';
import { configSetInputSchema, configSetOutputSchema } from '../../../../../plugins/dev-team/bin/src/schemas/config-set.schema';
import { readConfig } from '../../../../../plugins/dev-team/bin/src/lib/config';

// ---------------------------------------------------------------------------
// Unit Tests: runConfigSet
// ---------------------------------------------------------------------------

describe('runConfigSet — command logic (unit)', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-set-'));
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* Windows EPERM */ }
  });

  it('should return { key, written: true } on successful write', () => {
    const result = runConfigSet({ key: 'schema', value: 'spec-driven', projectRoot: tmpDir });
    expect(result.key).toBe('schema');
    expect(result.written).toBe(true);
  });

  it('should forward project_root parameter', () => {
    const customDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-set-custom-'));
    try {
      const result = runConfigSet({ key: 'schema', value: 'spec-driven', projectRoot: customDir });
      expect(result.written).toBe(true);
      const configPath = path.join(customDir, 'openspec', 'config.json');
      expect(fs.existsSync(configPath)).toBe(true);
      const raw = fs.readFileSync(configPath, 'utf-8');
      expect(JSON.parse(raw).schema).toBe('spec-driven');
    } finally {
      try { fs.rmSync(customDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should store string "true" as string, not boolean (AC-7)', () => {
    // AC-7: No type inference — "true" must remain a string
    runConfigSet({ key: 'flag', value: 'true', projectRoot: tmpDir });
    const config = readConfig(tmpDir);
    expect(config.flag).toBe('true');
    expect(typeof config.flag).toBe('string');
  });

  it('should store string "false" as string, not boolean (AC-7)', () => {
    runConfigSet({ key: 'flag', value: 'false', projectRoot: tmpDir });
    const config = readConfig(tmpDir);
    expect(config.flag).toBe('false');
    expect(typeof config.flag).toBe('string');
  });

  it('should store string "42" as string, not number (AC-7)', () => {
    runConfigSet({ key: 'count', value: '42', projectRoot: tmpDir });
    const config = readConfig(tmpDir);
    expect(config.count).toBe('42');
    expect(typeof config.count).toBe('string');
  });

  it('should preserve number type for numeric values', () => {
    runConfigSet({ key: 'count', value: 42, projectRoot: tmpDir });
    const config = readConfig(tmpDir);
    expect(config.count).toBe(42);
    expect(typeof config.count).toBe('number');
  });

  it('should preserve boolean type for boolean values', () => {
    runConfigSet({ key: 'flag', value: true, projectRoot: tmpDir });
    const config = readConfig(tmpDir);
    expect(config.flag).toBe(true);
    expect(typeof config.flag).toBe('boolean');
  });

  it('should handle nested dot-path keys', () => {
    const result = runConfigSet({ key: 'test_scripts.unit', value: 'npm test', projectRoot: tmpDir });
    expect(result.key).toBe('test_scripts.unit');
    expect(result.written).toBe(true);
    const config = readConfig(tmpDir);
    expect((config.test_scripts as Record<string, unknown>).unit).toBe('npm test');
  });

  it('should not throw on missing config file (auto-creates)', () => {
    const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-set-empty-'));
    try {
      expect(() => runConfigSet({ key: 'schema', value: 'spec-driven', projectRoot: emptyDir })).not.toThrow();
    } finally {
      try { fs.rmSync(emptyDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should validate against schema before write and reject invalid values', () => {
    const testDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-set-validate-'));
    try {
      // Setting schema to null should be rejected by schema validation
      expect(() => runConfigSet({ key: 'schema', value: null, projectRoot: testDir })).toThrow();
      // File should NOT have been created
      const configPath = path.join(testDir, 'openspec', 'config.json');
      expect(fs.existsSync(configPath)).toBe(false);
    } finally {
      try { fs.rmSync(testDir, { recursive: true, force: true }); } catch {}
    }
  });
});

// ---------------------------------------------------------------------------
// Schema Validation: configSetInputSchema
// ---------------------------------------------------------------------------

describe('configSetInputSchema — Zod input validation', () => {
  const schema = z.object(configSetInputSchema);

  it('should accept valid input with key and value', () => {
    const result = schema.safeParse({ key: 'schema', value: 'spec-driven' });
    expect(result.success).toBe(true);
  });

  it('should accept input with project_root', () => {
    const result = schema.safeParse({ key: 'x', value: 'y', project_root: '/tmp' });
    expect(result.success).toBe(true);
  });

  it('should accept nested key paths', () => {
    const result = schema.safeParse({ key: 'a.b', value: 42 });
    expect(result.success).toBe(true);
  });

  it('should reject input without key', () => {
    const result = schema.safeParse({ value: 'test' });
    expect(result.success).toBe(false);
  });

  it('should reject input without value', () => {
    const result = schema.safeParse({ key: 'x' });
    expect(result.success).toBe(false);
  });

  it('should reject input with empty key', () => {
    const result = schema.safeParse({ key: '', value: 'test' });
    expect(result.success).toBe(false);
  });

  it('should accept input with complex value types (array, object)', () => {
    const result1 = schema.safeParse({ key: 'arr', value: [1, 2, 3] });
    expect(result1.success).toBe(true);
    const result2 = schema.safeParse({ key: 'obj', value: { nested: true } });
    expect(result2.success).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Schema Validation: configSetOutputSchema
// ---------------------------------------------------------------------------

describe('configSetOutputSchema — Zod output validation', () => {
  it('should accept output with key and written: true', () => {
    const result = configSetOutputSchema.safeParse({ key: 'schema', written: true });
    expect(result.success).toBe(true);
  });

  it('should reject output missing key', () => {
    const result = configSetOutputSchema.safeParse({ written: true });
    expect(result.success).toBe(false);
  });

  it('should reject output with non-boolean written', () => {
    const result = configSetOutputSchema.safeParse({ key: 'schema', written: 'yes' });
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// Integration Tests: Real filesystem
// ===========================================================================

describe('Integration: config/set filesystem behavior', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-set-integration-'));
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* Windows EPERM */ }
  });

  it('should auto-create openspec/config.json when writing to empty directory', () => {
    const result = runConfigSet({ key: 'context', value: 'test', projectRoot: tmpDir });
    expect(result.written).toBe(true);
    const configPath = path.join(tmpDir, 'openspec', 'config.json');
    expect(fs.existsSync(configPath)).toBe(true);
    const content = fs.readFileSync(configPath, 'utf-8');
    const parsed = JSON.parse(content);
    expect(parsed.schema).toBe('spec-driven');
  });

  it('should auto-create skeleton when project_root does not exist', () => {
    const nonExistent = path.join(os.tmpdir(), 'config-set-nonexistent-' + Date.now());
    try {
      const result = runConfigSet({ key: 'schema', value: 'spec-driven', projectRoot: nonExistent });
      expect(result.written).toBe(true);
      const configPath = path.join(nonExistent, 'openspec', 'config.json');
      expect(fs.existsSync(configPath)).toBe(true);
      const raw = fs.readFileSync(configPath, 'utf-8');
      expect(JSON.parse(raw).schema).toBe('spec-driven');
    } finally {
      try { fs.rmSync(nonExistent, { recursive: true, force: true }); } catch {}
    }
  });

  it('should write a key and verify JSON file content with JSON.parse (AC-2)', () => {
    runConfigSet({ key: 'static_check', value: ['npm test'], projectRoot: tmpDir });
    const configPath = path.join(tmpDir, 'openspec', 'config.json');
    const raw = fs.readFileSync(configPath, 'utf-8');
    const parsed = JSON.parse(raw);
    expect(parsed.static_check).toEqual(['npm test']);
  });
});

// ===========================================================================
// Idempotency (幂等性): config/set
// ===========================================================================

describe('config/set idempotency', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-set-idempotent-'));
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* Windows EPERM */ }
  });

  it('should return identical results when setting same key/value twice (幂等性-1)', () => {
    const first = runConfigSet({ key: 'test_scripts.unit', value: 'npm test', projectRoot: tmpDir });
    const second = runConfigSet({ key: 'test_scripts.unit', value: 'npm test', projectRoot: tmpDir });
    expect(first).toEqual(second);
    const readResult = readConfig(tmpDir);
    expect((readResult.test_scripts as Record<string, unknown>).unit).toBe('npm test');
  });

  it('should correctly overwrite when setting same key with different value (幂等性-2)', () => {
    runConfigSet({ key: 'context', value: 'v1', projectRoot: tmpDir });
    runConfigSet({ key: 'context', value: 'v2', projectRoot: tmpDir });
    const result = readConfig(tmpDir);
    expect(result.context).toBe('v2');
    expect(Array.isArray(result.context)).toBe(false);
  });
});
