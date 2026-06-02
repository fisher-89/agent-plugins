/**
 * Unit and integration tests for config/unset MCP tool command.
 *
 * Unit tests cover:
 * - Command layer parameter handling and response formatting
 * - Non-existent key returns { key, removed: false }
 * - project_root parameter forwarding
 * - Schema validation
 *
 * Integration tests cover:
 * - Idempotent unset operations
 * - First unset returns removed: true, second returns removed: false
 *
 * @see openspec/changes/add-config-mcp-tools/test-design.md
 * @see openspec/changes/add-config-mcp-tools/specs/config-unset/spec.md
 */

import { describe, it, expect, beforeAll, afterAll } from 'vite-plus/test';
import { z } from 'zod/v4';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { runConfigUnset } from '../../../../../plugins/dev-team/bin/src/commands/config-unset';
import { configUnsetInputSchema, configUnsetOutputSchema } from '../../../../../plugins/dev-team/bin/src/schemas/config-unset.schema';
import { runConfigSet } from '../../../../../plugins/dev-team/bin/src/commands/config-set';
import { readConfig } from '../../../../../plugins/dev-team/bin/src/lib/config';

// ---------------------------------------------------------------------------
// Unit Tests: runConfigUnset
// ---------------------------------------------------------------------------

describe('runConfigUnset — command logic (unit)', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-unset-'));
    // Seed the config with known keys
    runConfigSet({ key: 'schema', value: 'spec-driven', projectRoot: tmpDir });
    runConfigSet({ key: 'context', value: 'test', projectRoot: tmpDir });
    runConfigSet({ key: 'test_scripts.unit', value: 'npm test', projectRoot: tmpDir });
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* Windows EPERM */ }
  });

  it('should return { key, removed: true } when key exists', () => {
    const result = runConfigUnset({ key: 'context', projectRoot: tmpDir });
    expect(result.key).toBe('context');
    expect(result.removed).toBe(true);
  });

  it('should return { key, removed: false } when key does not exist', () => {
    const result = runConfigUnset({ key: 'nonexistent', projectRoot: tmpDir });
    expect(result.key).toBe('nonexistent');
    expect(result.removed).toBe(false);
  });

  it('should forward project_root parameter', () => {
    const customDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-unset-custom-'));
    try {
      // First write a key
      runConfigSet({ key: 'schema', value: 'spec-driven', projectRoot: customDir });
      const result = runConfigUnset({ key: 'schema', projectRoot: customDir });
      expect(result.removed).toBe(true);
      const config = readConfig(customDir);
      expect(config.schema).toBeUndefined();
    } finally {
      try { fs.rmSync(customDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should handle nested dot-path keys', () => {
    const result = runConfigUnset({ key: 'test_scripts.unit', projectRoot: tmpDir });
    expect(result.key).toBe('test_scripts.unit');
    expect(result.removed).toBe(true);
    const config = readConfig(tmpDir);
    expect(config.test_scripts).toBeDefined();
    expect((config.test_scripts as Record<string, unknown>).unit).toBeUndefined();
  });

  it('should not throw when config file does not exist', () => {
    const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-unset-empty-'));
    try {
      expect(() => runConfigUnset({ key: 'schema', projectRoot: emptyDir })).not.toThrow();
    } finally {
      try { fs.rmSync(emptyDir, { recursive: true, force: true }); } catch {}
    }
  });
});

// ---------------------------------------------------------------------------
// Schema Validation: configUnsetInputSchema
// ---------------------------------------------------------------------------

describe('configUnsetInputSchema — Zod input validation', () => {
  const schema = z.object(configUnsetInputSchema);

  it('should accept valid input with key only', () => {
    const result = schema.safeParse({ key: 'schema' });
    expect(result.success).toBe(true);
  });

  it('should accept input with project_root', () => {
    const result = schema.safeParse({ key: 'schema', project_root: '/tmp' });
    expect(result.success).toBe(true);
  });

  it('should reject input without key', () => {
    const result = schema.safeParse({});
    expect(result.success).toBe(false);
  });

  it('should reject input with empty key', () => {
    const result = schema.safeParse({ key: '' });
    expect(result.success).toBe(false);
  });

  it('should reject input with non-string key', () => {
    const result = schema.safeParse({ key: 123 });
    expect(result.success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Schema Validation: configUnsetOutputSchema
// ---------------------------------------------------------------------------

describe('configUnsetOutputSchema — Zod output validation', () => {
  it('should accept output with removed: true', () => {
    const result = configUnsetOutputSchema.safeParse({ key: 'schema', removed: true });
    expect(result.success).toBe(true);
  });

  it('should accept output with removed: false', () => {
    const result = configUnsetOutputSchema.safeParse({ key: 'schema', removed: false });
    expect(result.success).toBe(true);
  });

  it('should reject output missing key', () => {
    const result = configUnsetOutputSchema.safeParse({ removed: true });
    expect(result.success).toBe(false);
  });

  it('should reject output with non-boolean removed', () => {
    const result = configUnsetOutputSchema.safeParse({ key: 'schema', removed: 'yes' });
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// Integration Tests: Idempotency (幂等性)
// ===========================================================================

describe('config/unset idempotency — filesystem', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-unset-idemp-'));
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* Windows EPERM */ }
  });

  it('should return removed: false for already-removed key (幂等性-3)', () => {
    runConfigSet({ key: 'test_scripts.unit', value: 'npm test', projectRoot: tmpDir });
    const first = runConfigUnset({ key: 'test_scripts.unit', projectRoot: tmpDir });
    expect(first.removed).toBe(true);
    const second = runConfigUnset({ key: 'test_scripts.unit', projectRoot: tmpDir });
    expect(second.removed).toBe(false);
  });

  it('should return removed: true then removed: false for same key (幂等性-4)', () => {
    runConfigSet({ key: 'static_check', value: ['npm test'], projectRoot: tmpDir });
    const first = runConfigUnset({ key: 'static_check', projectRoot: tmpDir });
    expect(first.removed).toBe(true);
    const second = runConfigUnset({ key: 'static_check', projectRoot: tmpDir });
    expect(second.removed).toBe(false);
  });

  it('should handle nested key unset idempotently', () => {
    runConfigSet({ key: 'a.b.c', value: 'deep', projectRoot: tmpDir });
    const first = runConfigUnset({ key: 'a.b.c', projectRoot: tmpDir });
    expect(first.removed).toBe(true);
    const second = runConfigUnset({ key: 'a.b.c', projectRoot: tmpDir });
    expect(second.removed).toBe(false);
  });
});
