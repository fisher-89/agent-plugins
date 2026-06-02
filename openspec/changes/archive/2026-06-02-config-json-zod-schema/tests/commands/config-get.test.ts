/**
 * Unit tests for config/get MCP tool command.
 *
 * Tests cover:
 * - Command layer parameter parsing and response formatting
 * - Dot-separated key path handling
 * - Non-existent key responses
 * - project_root parameter override
 * - Schema validation (input/output Zod schemas)
 * - Graceful degradation on invalid JSON
 *
 * @see openspec/changes/config-json-zod-schema/test-design.md
 * @see openspec/changes/config-json-zod-schema/specs/config-get/spec.md
 */

import { describe, it, expect, beforeAll, afterAll } from 'vite-plus/test';
import { z } from 'zod/v4';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { runConfigGet } from '../../../../../plugins/dev-team/bin/src/commands/config-get';
import { configGetInputSchema, configGetOutputSchema } from '../../../../../plugins/dev-team/bin/src/schemas/config-get.schema';
import { writeConfig } from '../../../../../plugins/dev-team/bin/src/lib/config';

// ---------------------------------------------------------------------------
// Unit Tests: runConfigGet
// ---------------------------------------------------------------------------

describe('runConfigGet — command logic', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-get-'));
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* Windows EPERM */ }
  });

  it('should return value and exists: true for an existing key', () => {
    const result = runConfigGet({ key: 'schema', projectRoot: tmpDir });
    expect(result.key).toBe('schema');
    expect(result.value).toBe('spec-driven');
    expect(result.exists).toBe(true);
  });

  it('should return exists: false for a non-existent key', () => {
    const result = runConfigGet({ key: 'nonexistent', projectRoot: tmpDir });
    expect(result.key).toBe('nonexistent');
    expect(result.exists).toBe(false);
    expect(result.value).toBeUndefined();
  });

  it('should handle dot-separated nested key paths', () => {
    writeConfig(tmpDir, {
      schema: 'spec-driven',
      test_scripts: { unit: 'npm run test:unit' },
    });
    const result = runConfigGet({ key: 'test_scripts.unit', projectRoot: tmpDir });
    expect(result.key).toBe('test_scripts.unit');
    expect(result.value).toBe('npm run test:unit');
    expect(result.exists).toBe(true);
  });

  it('should use custom project_root directory', () => {
    const customDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-get-custom-'));
    try {
      const result = runConfigGet({ key: 'schema', projectRoot: customDir });
      expect(result.exists).toBe(true);
      expect(result.value).toBe('spec-driven');
    } finally {
      try { fs.rmSync(customDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should auto-create skeleton when project_root does not yet exist', () => {
    const nonExistent = path.join(os.tmpdir(), 'config-get-new-' + Date.now());
    try {
      const result = runConfigGet({ key: 'schema', projectRoot: nonExistent });
      expect(result.exists).toBe(true);
      expect(result.value).toBe('spec-driven');
      const configPath = path.join(nonExistent, 'openspec', 'config.json');
      expect(fs.existsSync(configPath)).toBe(true);
    } finally {
      try { fs.rmSync(nonExistent, { recursive: true, force: true }); } catch {}
    }
  });

  it('should not throw when config file does not exist', () => {
    const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-get-empty-'));
    try {
      expect(() => runConfigGet({ key: 'schema', projectRoot: emptyDir })).not.toThrow();
    } finally {
      try { fs.rmSync(emptyDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should gracefully degrade when config file has invalid JSON', () => {
    const badJsonDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-get-badjson-'));
    try {
      const dir = path.join(badJsonDir, 'openspec');
      fs.mkdirSync(dir, { recursive: true });
      fs.writeFileSync(path.join(dir, 'config.json'), '{ invalid json content here\n', 'utf-8');
      const result = runConfigGet({ key: 'schema', projectRoot: badJsonDir });
      // readConfig degrades gracefully and returns default { schema: 'spec-driven' }
      expect(result.exists).toBe(true);
      expect(result.value).toBe('spec-driven');
    } finally {
      try { fs.rmSync(badJsonDir, { recursive: true, force: true }); } catch {}
    }
  });
});

// ---------------------------------------------------------------------------
// Schema Validation: configGetInputSchema
// ---------------------------------------------------------------------------

describe('configGetInputSchema — Zod schema validation', () => {
  const schema = z.object(configGetInputSchema);

  it('should accept valid input with key only', () => {
    const result = schema.safeParse({ key: 'schema' });
    expect(result.success).toBe(true);
  });

  it('should accept valid input with key and project_root', () => {
    const result = schema.safeParse({ key: 'schema', project_root: '/tmp/project' });
    expect(result.success).toBe(true);
  });

  it('should reject input without key', () => {
    const result = schema.safeParse({});
    expect(result.success).toBe(false);
  });

  it('should reject input with empty string key', () => {
    const result = schema.safeParse({ key: '' });
    expect(result.success).toBe(false);
  });

  it('should reject input with non-string key', () => {
    const result = schema.safeParse({ key: 123 });
    expect(result.success).toBe(false);
  });

  it('should accept input with null project_root', () => {
    const result = schema.safeParse({ key: 'schema', project_root: null });
    expect(result.success).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Schema Validation: configGetOutputSchema
// ---------------------------------------------------------------------------

describe('configGetOutputSchema — output schema validation', () => {
  it('should accept output with exists: true and value present', () => {
    const result = configGetOutputSchema.safeParse({ key: 'schema', value: 'spec-driven', exists: true });
    expect(result.success).toBe(true);
  });

  it('should accept output with exists: false and no value', () => {
    const result = configGetOutputSchema.safeParse({ key: 'schema', exists: false });
    expect(result.success).toBe(true);
  });

  it('should accept output with exists: true and no value (value is optional)', () => {
    const result = configGetOutputSchema.safeParse({ key: 'schema', exists: true });
    expect(result.success).toBe(true);
  });

  it('should reject output missing key field', () => {
    const result = configGetOutputSchema.safeParse({ value: 'test', exists: true });
    expect(result.success).toBe(false);
  });

  it('should reject output with wrong key type', () => {
    const result = configGetOutputSchema.safeParse({ key: 123, value: 'test', exists: true });
    expect(result.success).toBe(false);
  });
});
