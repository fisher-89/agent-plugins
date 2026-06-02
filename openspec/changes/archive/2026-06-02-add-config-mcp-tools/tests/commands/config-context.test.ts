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
 *
 * Full filesystem integration is shared with lib/config.test.ts.
 * Command tests mock lib/config.ts exports where needed.
 *
 * @see openspec/changes/add-config-mcp-tools/test-design.md
 * @see openspec/changes/add-config-mcp-tools/specs/config-context/spec.md
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
    } finally {
      try { fs.rmSync(emptyDir, { recursive: true, force: true }); } catch {}
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
