/**
 * Unit tests for config/context MCP tool command.
 *
 * Tests cover:
 * - Reading context (no argument) returns current context value
 * - Writing context (with argument) updates context and returns written: true
 * - project_root parameter forwarding
 * - Context not set scenario
 * - Write-then-read round-trip
 * - Schema validation (input/output Zod schemas)
 * - Auto-create config file when writing to empty project
 *
 * @see openspec/changes/config-json-zod-schema/test-design.md
 * @see openspec/changes/config-json-zod-schema/specs/config-context/spec.md
 */

import { describe, it, expect, beforeAll, afterAll } from 'vite-plus/test';
import { z } from 'zod/v4';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { runConfigContext } from '../../../../../plugins/dev-team/bin/src/commands/config-context';
import { configContextInputSchema, configContextOutputSchema } from '../../../../../plugins/dev-team/bin/src/schemas/config-context.schema';
import { writeConfig } from '../../../../../plugins/dev-team/bin/src/lib/config';

// ---------------------------------------------------------------------------
// Unit Tests: runConfigContext (read mode — no context arg)
// ---------------------------------------------------------------------------

describe('runConfigContext — read mode (no context argument)', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-'));
    // Write a config with context
    writeConfig(tmpDir, {
      schema: 'spec-driven',
      context: 'Tech stack: TypeScript, Node.js',
    });
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* Windows EPERM */ }
  });

  it('should return current context when context is set', () => {
    const result = runConfigContext({ projectRoot: tmpDir });
    expect(result.context).toBe('Tech stack: TypeScript, Node.js');
    expect(result.written).toBeUndefined();
  });

  it('should return empty string when context key is missing in config', () => {
    const noCtxDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-noctx-'));
    try {
      writeConfig(noCtxDir, { schema: 'spec-driven' });
      const result = runConfigContext({ projectRoot: noCtxDir });
      expect(result.context).toBe('');
      expect(result.written).toBeUndefined();
    } finally {
      try { fs.rmSync(noCtxDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should return empty string when config file does not exist', () => {
    const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-empty-'));
    try {
      const result = runConfigContext({ projectRoot: emptyDir });
      expect(result.context).toBe('');
      expect(result.written).toBeUndefined();
    } finally {
      try { fs.rmSync(emptyDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should forward project_root in read mode', () => {
    const customDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-custom-'));
    try {
      writeConfig(customDir, { context: 'custom project' });
      const result = runConfigContext({ projectRoot: customDir });
      expect(result.context).toBe('custom project');
    } finally {
      try { fs.rmSync(customDir, { recursive: true, force: true }); } catch {}
    }
  });
});

// ---------------------------------------------------------------------------
// Unit Tests: runConfigContext (write mode — with context arg)
// ---------------------------------------------------------------------------

describe('runConfigContext — write mode (with context argument)', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-write-'));
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch { /* Windows EPERM */ }
  });

  it('should write context and return { context, written: true }', () => {
    const result = runConfigContext({ context: 'new context', projectRoot: tmpDir });
    expect(result.context).toBe('new context');
    expect(result.written).toBe(true);
  });

  it('should forward project_root in write mode', () => {
    const customDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-write2-'));
    try {
      const result = runConfigContext({ context: 'ctx', projectRoot: customDir });
      expect(result.context).toBe('ctx');
      expect(result.written).toBe(true);
    } finally {
      try { fs.rmSync(customDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should overwrite existing context value', () => {
    runConfigContext({ context: 'v1', projectRoot: tmpDir });
    const second = runConfigContext({ context: 'v2', projectRoot: tmpDir });
    expect(second.context).toBe('v2');
    expect(second.written).toBe(true);
  });

  it('should handle empty string context value', () => {
    const result = runConfigContext({ context: '', projectRoot: tmpDir });
    expect(result.context).toBe('');
    expect(result.written).toBe(true);
  });

  it('should auto-create config file when writing context to empty project', () => {
    const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-autocreate-'));
    try {
      const result = runConfigContext({ context: 'new project', projectRoot: emptyDir });
      expect(result.written).toBe(true);
      const configPath = path.join(emptyDir, 'openspec', 'config.json');
      expect(fs.existsSync(configPath)).toBe(true);
      const raw = fs.readFileSync(configPath, 'utf-8');
      const parsed = JSON.parse(raw);
      expect(parsed.context).toBe('new project');
      expect(parsed.schema).toBe('spec-driven');
    } finally {
      try { fs.rmSync(emptyDir, { recursive: true, force: true }); } catch {}
    }
  });

  it('should reject write when config.json contains invalid content (Zod validation)', () => {
    const invalidDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-invalid-'));
    try {
      // Write a config with invalid content (schema field type error)
      const cfgDir = path.join(invalidDir, 'openspec');
      fs.mkdirSync(cfgDir, { recursive: true });
      fs.writeFileSync(
        path.join(cfgDir, 'config.json'),
        JSON.stringify({ schema: null }, null, 2),
        'utf-8',
      );
      // Attempting to write context should fail parseConfig validation
      expect(() => runConfigContext({ context: 'new context', projectRoot: invalidDir })).toThrow();
      // The config.json file should remain unmodified (original invalid content)
      const raw = fs.readFileSync(path.join(cfgDir, 'config.json'), 'utf-8');
      const parsed = JSON.parse(raw);
      expect(parsed.schema).toBeNull();
    } finally {
      try { fs.rmSync(invalidDir, { recursive: true, force: true }); } catch {}
    }
  });
});

// ---------------------------------------------------------------------------
// Schema Validation: configContextInputSchema
// ---------------------------------------------------------------------------

describe('configContextInputSchema — Zod input validation', () => {
  const schema = z.object(configContextInputSchema);

  it('should accept empty input (read mode)', () => {
    const result = schema.safeParse({});
    expect(result.success).toBe(true);
  });

  it('should accept input with context (write mode)', () => {
    const result = schema.safeParse({ context: 'new context' });
    expect(result.success).toBe(true);
  });

  it('should accept input with project_root only', () => {
    const result = schema.safeParse({ project_root: '/tmp' });
    expect(result.success).toBe(true);
  });

  it('should accept input with context and project_root', () => {
    const result = schema.safeParse({ context: 'ctx', project_root: '/tmp' });
    expect(result.success).toBe(true);
  });

  it('should accept empty string context', () => {
    const result = schema.safeParse({ context: '' });
    expect(result.success).toBe(true);
  });

  it('should reject non-string context', () => {
    const result = schema.safeParse({ context: 123 });
    expect(result.success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Schema Validation: configContextOutputSchema
// ---------------------------------------------------------------------------

describe('configContextOutputSchema — Zod output validation', () => {
  it('should accept read-mode output (context only)', () => {
    const result = configContextOutputSchema.safeParse({ context: 'test context' });
    expect(result.success).toBe(true);
  });

  it('should accept write-mode output (context + written)', () => {
    const result = configContextOutputSchema.safeParse({ context: 'test', written: true });
    expect(result.success).toBe(true);
  });

  it('should reject output missing context', () => {
    const result = configContextOutputSchema.safeParse({});
    expect(result.success).toBe(false);
  });

  it('should reject output with non-string context', () => {
    const result = configContextOutputSchema.safeParse({ context: 42 });
    expect(result.success).toBe(false);
  });

  it('should reject output with non-boolean written', () => {
    const result = configContextOutputSchema.safeParse({ context: 'test', written: 'yes' });
    expect(result.success).toBe(false);
  });
});
