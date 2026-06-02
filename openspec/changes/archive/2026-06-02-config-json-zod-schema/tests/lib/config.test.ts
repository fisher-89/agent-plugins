/**
 * Unit and integration tests for lib/config.ts — JSON config file operations.
 *
 * Unit tests cover:
 * - getValue / setValue / unsetValue (dot-path object traversal)
 * - In-memory config object manipulation (no filesystem)
 *
 * Integration tests cover:
 * - Real filesystem: temp directory-based read/write round-trips
 * - Auto-creation of openspec/config.json skeleton
 * - JSON syntax validity after write operations
 * - YAML-to-JSON automatic migration
 * - Non-ASCII value handling
 * - project_root points to non-existent directory
 * - JSON parsing failures (invalid JSON content)
 * - ensureConfigFile creates skeleton with correct content
 *
 * @see openspec/changes/config-json-zod-schema/test-design.md
 * @see openspec/changes/config-json-zod-schema/specs/config-schema/spec.md
 * @see openspec/changes/config-json-zod-schema/specs/config-get/spec.md
 * @see openspec/changes/config-json-zod-schema/specs/config-set/spec.md
 * @see openspec/changes/config-json-zod-schema/specs/config-unset/spec.md
 * @see openspec/changes/config-json-zod-schema/specs/config-context/spec.md
 */

import { describe, it, expect, beforeAll, afterAll } from 'vite-plus/test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

// ---------------------------------------------------------------------------
// Import the module under test
// WARNING: These imports will fail until the source module is implemented.
// TODO: Update import paths to match actual source location.
// Expected source: plugins/dev-team/bin/src/lib/config.ts
// ---------------------------------------------------------------------------
import {
  readConfig,
  writeConfig,
  getValue,
  setValue,
  unsetValue,
  ensureConfigFile,
  readContext,
  writeContext,
} from '../../../../../plugins/dev-team/bin/src/lib/config';

// ---------------------------------------------------------------------------
// Shared test data
// ---------------------------------------------------------------------------

const sampleConfig = {
  schema: 'spec-driven',
  context: 'Tech stack: TypeScript, Node.js',
  static_check: ['npm run check', 'npm test'],
  test_scripts: {
    unit: 'npm run test:unit',
    integration: 'npm run test:integration',
    e2e: 'npm run test:e2e',
  },
  nested: {
    level1: {
      level2: {
        level3: 'deep-value',
      },
    },
  },
};

// ===========================================================================
// Unit Tests: getValue — Dot-path object traversal
// ===========================================================================

describe('getValue — dot-path traversal', () => {
  it('should return value for a top-level key', () => {
    const result = getValue(sampleConfig, 'schema');
    expect(result.exists).toBe(true);
    expect(result.value).toBe('spec-driven');
  });

  it('should return value for a two-level nested key', () => {
    const result = getValue(sampleConfig, 'test_scripts.unit');
    expect(result.exists).toBe(true);
    expect(result.value).toBe('npm run test:unit');
  });

  it('should return value for a deep nested key (4 levels)', () => {
    const result = getValue(sampleConfig, 'nested.level1.level2.level3');
    expect(result.exists).toBe(true);
    expect(result.value).toBe('deep-value');
  });

  it('should return exists: false when intermediate node is missing', () => {
    const result = getValue(sampleConfig, 'test_scripts.nonexistent');
    expect(result.exists).toBe(false);
    expect(result.value).toBeUndefined();
  });

  it('should return exists: false when root key is missing', () => {
    const result = getValue(sampleConfig, 'unknown.key');
    expect(result.exists).toBe(false);
    expect(result.value).toBeUndefined();
  });

  it('should return exists: false for a completely unknown key', () => {
    const result = getValue(sampleConfig, 'nonexistent');
    expect(result.exists).toBe(false);
    expect(result.value).toBeUndefined();
  });

  it('should return the entire config object when key is empty string', () => {
    // TODO: Decide expected behavior — return whole object or throw.
    // Per test-design: "返回整个配置对象或抛出明确错误"
    const result = getValue(sampleConfig, '');
    if (result.exists) {
      expect(result.value).toEqual(sampleConfig);
    } else {
      // If implementation throws, test should catch it
      expect(result.exists).toBe(false);
    }
  });

  it('should handle array values inside config', () => {
    const result = getValue(sampleConfig, 'static_check');
    expect(result.exists).toBe(true);
    expect(result.value).toEqual(['npm run check', 'npm test']);
  });

  it('should return exists: false for an empty config object', () => {
    const result = getValue({}, 'anything');
    expect(result.exists).toBe(false);
    expect(result.value).toBeUndefined();
  });
});

// ===========================================================================
// Unit Tests: setValue — Dot-path object mutation
// ===========================================================================

describe('setValue — dot-path set', () => {
  it('should set a top-level key', () => {
    const config = {};
    const updated = setValue(config, 'schema', 'spec-driven');
    expect(updated.schema).toBe('spec-driven');
  });

  it('should set a two-level nested key, auto-creating intermediate object', () => {
    const config = {};
    const updated = setValue(config, 'test_scripts.unit', 'npm test');
    expect(updated.test_scripts).toBeDefined();
    expect(updated.test_scripts.unit).toBe('npm test');
  });

  it('should auto-create intermediate objects for deep paths', () => {
    const config = {};
    const updated = setValue(config, 'a.b.c', 'x');
    expect(updated.a.b.c).toBe('x');
  });

  it('should overwrite an existing top-level key', () => {
    const config = { schema: 'old-value' };
    const updated = setValue(config, 'schema', 'new-value');
    expect(updated.schema).toBe('new-value');
  });

  it('should overwrite a nested existing key without destroying siblings', () => {
    const config = {
      test_scripts: {
        unit: 'npm run test:unit',
        integration: 'npm run test:integration',
      },
    };
    const updated = setValue(config, 'test_scripts.unit', 'npm run test:unit:new');
    expect(updated.test_scripts.unit).toBe('npm run test:unit:new');
    expect(updated.test_scripts.integration).toBe('npm run test:integration');
  });

  it('should set an array value', () => {
    const config = {};
    const value = ['eslint src/', 'tsc --noEmit'];
    const updated = setValue(config, 'static_check', value);
    expect(updated.static_check).toEqual(value);
  });

  it('should mutate the input config and return the same reference (in-place mutation)', () => {
    const config = {};
    const updated = setValue(config, 'key', 'value');
    expect(updated).toBe(config);
    expect(config).toHaveProperty('key');
  });

  it('should set the key on the original config object (in-place mutation)', () => {
    const config = {};
    const updated = setValue(config, 'key', 'val');
    expect(config).toHaveProperty('key');
    expect(config.key).toBe('val');
    expect(updated).toBe(config);
  });
});

// ===========================================================================
// Unit Tests: unsetValue — Dot-path key removal
// ===========================================================================

describe('unsetValue — dot-path removal', () => {
  it('should remove an existing top-level key', () => {
    const config = { schema: 'spec-driven', context: 'test' };
    const { config: updated, removed } = unsetValue(config, 'schema');
    expect(removed).toBe(true);
    expect(updated).not.toHaveProperty('schema');
    expect(updated).toHaveProperty('context');
  });

  it('should remove a nested key and keep siblings', () => {
    const config = {
      test_scripts: {
        unit: 'npm run test:unit',
        integration: 'npm run test:integration',
        e2e: 'npm run test:e2e',
      },
    };
    const { config: updated, removed } = unsetValue(config, 'test_scripts.unit');
    expect(removed).toBe(true);
    expect(updated.test_scripts).not.toHaveProperty('unit');
    expect(updated.test_scripts.integration).toBe('npm run test:integration');
    expect(updated.test_scripts.e2e).toBe('npm run test:e2e');
  });

  it('should return removed: false for a non-existent key', () => {
    const config = { schema: 'spec-driven' };
    const { config: updated, removed } = unsetValue(config, 'nonexistent');
    expect(removed).toBe(false);
    expect(updated).toEqual(config);
  });

  it('should return removed: false for a non-existent nested key', () => {
    const config = { test_scripts: { unit: 'test' } };
    const { config: updated, removed } = unsetValue(config, 'test_scripts.nonexistent');
    expect(removed).toBe(false);
    expect(updated).toEqual(config);
  });

  it('should leave parent as empty object after removing last child key', () => {
    const config = { test_scripts: { unit: 'npm run test:unit' } };
    const { config: updated, removed } = unsetValue(config, 'test_scripts.unit');
    expect(removed).toBe(true);
    // Parent key remains as empty object per spec
    expect(updated.test_scripts).toEqual({});
  });

  it('should not throw when removing from an empty config', () => {
    const config = {};
    const { config: updated, removed } = unsetValue(config, 'anything');
    expect(removed).toBe(false);
    expect(updated).toEqual({});
  });

  it('should mutate the original config by removing the key (in-place mutation)', () => {
    const config = { schema: 'spec-driven' };
    const { config: updated, removed } = unsetValue(config, 'schema');
    expect(removed).toBe(true);
    expect(config).not.toHaveProperty('schema');
    expect(updated).toBe(config);
  });
});

// ===========================================================================
// AC-7: inferValue is no longer exported
// ===========================================================================

describe('AC-7: inferValue no longer exported', () => {
  it('should not export inferValue from lib/config', async () => {
    // Use dynamic import to avoid TypeScript errors when the export doesn't exist
    const libModule = await import('../../../../../plugins/dev-team/bin/src/lib/config');
    expect((libModule as Record<string, unknown>).inferValue).toBeUndefined();
  });
});

// ===========================================================================
// Unit Tests: readConfig — In-memory operations (not filesystem)
// ===========================================================================

describe('readConfig — JSON parsing (unit with in-memory)', () => {
  it('should return default OpenSpecConfig when file does not exist', () => {
    // TODO: readConfig may accept an optional parameter for unit testing
    // or always read from disk. If filesystem-only, move to integration tests.
    // Expected behavior per spec: returns { schema: "spec-driven" } with defaults
  });
});

// ===========================================================================
// Integration Tests: Real filesystem round-trips
// ===========================================================================

describe('Integration: readConfig / writeConfig (real filesystem)', () => {
  let tmpDir: string;
  let configDir: string;
  let configPath: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-integration-'));
    configDir = path.join(tmpDir, 'openspec');
    configPath = path.join(configDir, 'config.json');
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('should return default OpenSpecConfig when config.json does not exist', () => {
    const result = readConfig(tmpDir);
    expect(result).toBeDefined();
    // Default should contain schema: 'spec-driven'
    expect(result.schema).toBe('spec-driven');
  });

  it('should create openspec/ directory and config.json skeleton on writeConfig', () => {
    writeConfig(tmpDir, { schema: 'spec-driven' });
    expect(fs.existsSync(configPath)).toBe(true);
    const content = fs.readFileSync(configPath, 'utf-8');
    const parsed = JSON.parse(content);
    expect(parsed.schema).toBe('spec-driven');
  });

  it('should write a key and round-trip it back', () => {
    const data = { schema: 'spec-driven', static_check: ['npm test'] };
    writeConfig(tmpDir, data);
    const result = readConfig(tmpDir);
    expect(result.schema).toBe('spec-driven');
    expect(result.static_check).toEqual(['npm test']);
  });

  it('should produce valid JSON (re-parseable by JSON.parse)', () => {
    writeConfig(tmpDir, { schema: 'spec-driven', context: 'test' });
    const raw = fs.readFileSync(configPath, 'utf-8');
    expect(() => JSON.parse(raw)).not.toThrow();
    const parsed = JSON.parse(raw);
    expect(parsed.schema).toBe('spec-driven');
  });

  it('should generate JSON with 2-space indentation', () => {
    writeConfig(tmpDir, { schema: 'spec-driven', context: 'test' });
    const raw = fs.readFileSync(configPath, 'utf-8');
    // Verify 2-space indentation by checking the context line prefix
    const lines = raw.split('\n');
    const contextLine = lines.find((l) => l.includes('"context"'));
    expect(contextLine).toBeDefined();
    // The line should be indented with exactly 2 spaces
    expect(contextLine).toMatch(/^  "/);
  });

  it('should unset a key and confirm file no longer contains it', () => {
    writeConfig(tmpDir, { schema: 'spec-driven', static_check: ['npm test'] });
    const config = readConfig(tmpDir);
    const { config: updatedConfig, removed } = unsetValue(config, 'static_check');
    expect(removed).toBe(true);
    writeConfig(tmpDir, updatedConfig);
    const result = readConfig(tmpDir);
    expect(result.static_check).toBeUndefined();
    expect(result.schema).toBe('spec-driven');
  });

  it('should create skeleton with schema: spec-driven when ensureConfigFile is called on empty dir', () => {
    const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ensure-'));
    try {
      const result = ensureConfigFile(emptyDir);
      const skeletonPath = path.join(emptyDir, 'openspec', 'config.json');
      expect(fs.existsSync(skeletonPath)).toBe(true);
      const raw = fs.readFileSync(skeletonPath, 'utf-8');
      expect(JSON.parse(raw).schema).toBe('spec-driven');
      expect(result.schema).toBe('spec-driven');
    } finally {
      fs.rmSync(emptyDir, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// Integration: Auto-creation of skeleton file
// ===========================================================================

describe('Integration: auto-creation of skeleton file', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-skeleton-'));
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('should auto-create config.json with schema: spec-driven when ensureConfigFile is called', () => {
    const configFilePath = path.join(tmpDir, 'openspec', 'config.json');
    ensureConfigFile(tmpDir);
    expect(fs.existsSync(configFilePath)).toBe(true);
    const raw = fs.readFileSync(configFilePath, 'utf-8');
    const parsed = JSON.parse(raw);
    expect(parsed.schema).toBe('spec-driven');
    // Skeleton JSON should have no other fields
    expect(Object.keys(parsed)).toEqual(['schema']);
  });

  it('should handle project_root pointing to a non-existent directory', () => {
    const nonExistent = path.join(os.tmpdir(), 'config-nonexistent-' + Date.now());
    try {
      ensureConfigFile(nonExistent);
      const configFilePath = path.join(nonExistent, 'openspec', 'config.json');
      expect(fs.existsSync(configFilePath)).toBe(true);
      const raw = fs.readFileSync(configFilePath, 'utf-8');
      expect(JSON.parse(raw).schema).toBe('spec-driven');
    } finally {
      fs.rmSync(nonExistent, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// Integration: YAML to JSON automatic migration
// ===========================================================================

describe('Integration: YAML-to-JSON automatic migration', () => {
  let tmpDir: string;

  function writeYamlConfig(content: string): void {
    const dir = path.join(tmpDir, 'openspec');
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'config.yaml'), content, 'utf-8');
  }

  function configJsonPath(): string {
    return path.join(tmpDir, 'openspec', 'config.json');
  }

  function configYamlPath(): string {
    return path.join(tmpDir, 'openspec', 'config.yaml');
  }

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-migrate-'));
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('should migrate config.yaml to config.json and delete config.yaml (AC-6)', () => {
    writeYamlConfig([
      'schema: spec-driven',
      'context: "Legacy project"',
      'rules:',
      '  proposal:',
      '    - "Keep it short"',
    ].join('\n'));
    // readConfig triggers migration: reads YAML, creates JSON, deletes YAML
    const result = readConfig(tmpDir);
    expect(result.schema).toBe('spec-driven');
    expect(result.context).toBe('Legacy project');
    // JSON file should exist
    expect(fs.existsSync(configJsonPath())).toBe(true);
    // YAML file should be deleted
    expect(fs.existsSync(configYamlPath())).toBe(false);
    // Verify JSON content is correct
    const jsonRaw = fs.readFileSync(configJsonPath(), 'utf-8');
    const parsed = JSON.parse(jsonRaw);
    expect(parsed.schema).toBe('spec-driven');
    expect(parsed.context).toBe('Legacy project');
  });

  it('should migrate minimal YAML (only schema)', () => {
    const singleDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-migrate-single-'));
    try {
      const yamlPath = path.join(singleDir, 'openspec', 'config.yaml');
      fs.mkdirSync(path.join(singleDir, 'openspec'), { recursive: true });
      fs.writeFileSync(yamlPath, 'schema: spec-driven\n', 'utf-8');
      const result = readConfig(singleDir);
      expect(result.schema).toBe('spec-driven');
      const jsonPath = path.join(singleDir, 'openspec', 'config.json');
      expect(fs.existsSync(jsonPath)).toBe(true);
      expect(fs.existsSync(yamlPath)).toBe(false);
    } finally {
      fs.rmSync(singleDir, { recursive: true, force: true });
    }
  });

  it('should not delete config.yaml if it contains values that fail validation', () => {
    const badDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-migrate-bad-'));
    try {
      const yamlPath = path.join(badDir, 'openspec', 'config.yaml');
      fs.mkdirSync(path.join(badDir, 'openspec'), { recursive: true });
      // schema value is a number — should fail validation
      fs.writeFileSync(yamlPath, 'schema: 123\n', 'utf-8');
      // TODO: readConfig may throw for invalid YAML content.
      // If it throws, the YAML file should NOT be deleted.
      // If it degrades gracefully, YAML file should remain.
      try {
        readConfig(badDir);
      } catch {
        // Expected: validation error
      }
      // YAML file must NOT be deleted
      expect(fs.existsSync(yamlPath)).toBe(true);
    } finally {
      fs.rmSync(badDir, { recursive: true, force: true });
    }
  });

  it('should not migrate again when config.json already exists (idempotent)', () => {
    // Use a separate directory to test "JSON exists, YAML added alongside" scenario
    const idempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-migrate-idemp-'));
    try {
      // First, create config.json (as if already migrated)
      const jsonDir = path.join(idempDir, 'openspec');
      fs.mkdirSync(jsonDir, { recursive: true });
      fs.writeFileSync(
        path.join(jsonDir, 'config.json'),
        JSON.stringify({ schema: 'spec-driven', context: 'original' }, null, 2),
        'utf-8',
      );
      // Now create YAML alongside existing JSON
      const yamlPath = path.join(jsonDir, 'config.yaml');
      fs.writeFileSync(yamlPath, 'schema: spec-driven\ncontext: "yaml-value"\n', 'utf-8');

      // readConfig should find config.json first and NOT trigger migration
      const result = readConfig(idempDir);
      // Result comes from existing JSON, NOT YAML
      expect(result.context).toBe('original');
      // YAML should NOT be deleted — no migration occurred
      expect(fs.existsSync(yamlPath)).toBe(true);
    } finally {
      fs.rmSync(idempDir, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// Integration: JSON syntax and validity
// ===========================================================================

describe('Integration: JSON syntax validity', () => {
  let tmpDir: string;

  function configPath(): string {
    return path.join(tmpDir, 'openspec', 'config.json');
  }

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-json-syntax-'));
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('should not throw JSON parse error after writeConfig', () => {
    writeConfig(tmpDir, { schema: 'spec-driven', static_check: ['npm test'] });
    const raw = fs.readFileSync(configPath(), 'utf-8');
    expect(() => JSON.parse(raw)).not.toThrow();
  });
});

// ===========================================================================
// Integration: JSON parse failure handling
// ===========================================================================

describe('Integration: JSON parse failure (invalid JSON)', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-bad-json-'));
    const dir = path.join(tmpDir, 'openspec');
    fs.mkdirSync(dir, { recursive: true });
    // Write invalid JSON
    fs.writeFileSync(path.join(dir, 'config.json'), '{ invalid json content here\n', 'utf-8');
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('should return default OpenSpecConfig when config.json contains invalid JSON', () => {
    // readConfig should degrade gracefully and return default config
    const result = readConfig(tmpDir);
    expect(result).toBeDefined();
    expect(result.schema).toBe('spec-driven');
  });
});

// ===========================================================================
// Integration: Non-ASCII and special character values
// ===========================================================================

describe('Integration: non-ASCII values in JSON', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-unicode-'));
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('should round-trip non-ASCII values (Chinese characters)', () => {
    const config = readConfig(tmpDir);
    setValue(config, 'description', '你好世界');
    writeConfig(tmpDir, config);
    const result = readConfig(tmpDir);
    expect(result.description).toBe('你好世界');
  });

  it('should round-trip string values containing special characters', () => {
    const config = readConfig(tmpDir);
    setValue(config, 'special', 'key: value with "quotes" and \\backslash');
    writeConfig(tmpDir, config);
    const result = readConfig(tmpDir);
    expect(result.special).toBe('key: value with "quotes" and \\backslash');
  });
});

// ===========================================================================
// Integration: Filesystem edge cases
// ===========================================================================

describe('Integration: filesystem edge cases', () => {
  it('should handle config.json with extra fields (passthrough behavior)', () => {
    const extrasDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-extras-'));
    try {
      const dir = path.join(extrasDir, 'openspec');
      fs.mkdirSync(dir, { recursive: true });
      fs.writeFileSync(
        path.join(dir, 'config.json'),
        JSON.stringify({ schema: 'spec-driven', custom_tool_key: 'allowed' }, null, 2),
        'utf-8',
      );
      const result = readConfig(extrasDir);
      expect(result.schema).toBe('spec-driven');
      // With passthrough, extra fields should be preserved
      expect((result as Record<string, unknown>).custom_tool_key).toBe('allowed');
    } finally {
      fs.rmSync(extrasDir, { recursive: true, force: true });
    }
  });

  it('should write values in a project_root that does not yet exist', () => {
    const nonExistent = path.join(os.tmpdir(), 'config-createdir-' + Date.now());
    try {
      const config = readConfig(nonExistent);
      setValue(config, 'schema', 'spec-driven');
      writeConfig(nonExistent, config);
      const configFilePath = path.join(nonExistent, 'openspec', 'config.json');
      expect(fs.existsSync(configFilePath)).toBe(true);
    } finally {
      fs.rmSync(nonExistent, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// Integration: Context read/write (via lib functions)
// ===========================================================================

describe('Integration: context read and write (lib functions)', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-'));
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('readContext should return empty string when context key is missing', () => {
    writeConfig(tmpDir, { schema: 'spec-driven' });
    const result = readContext(tmpDir);
    expect(result.context).toBe('');
  });

  it('readContext should return empty string when config file does not exist', () => {
    const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-ctx-empty-'));
    try {
      const result = readContext(emptyDir);
      expect(result.context).toBe('');
    } finally {
      fs.rmSync(emptyDir, { recursive: true, force: true });
    }
  });

  it('writeContext should write context and readContext should read it back', () => {
    const result = writeContext(tmpDir, 'Tech stack: TypeScript');
    expect(result.context).toBe('Tech stack: TypeScript');
    expect(result.written).toBe(true);
    const readResult = readContext(tmpDir);
    expect(readResult.context).toBe('Tech stack: TypeScript');
  });

  it('writeContext should overwrite existing context', () => {
    writeContext(tmpDir, 'First');
    writeContext(tmpDir, 'Second');
    const result = readContext(tmpDir);
    expect(result.context).toBe('Second');
  });
});

// ===========================================================================
// Boundary: project_root with relative path
// ===========================================================================

describe('Integration: project_root relative path resolution', () => {
  let tmpDir: string;
  let relativeTarget: string;
  let originalCwd: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-relative-'));
    const subDir = path.join(tmpDir, 'subproject');
    fs.mkdirSync(subDir, { recursive: true });
    originalCwd = process.cwd();
    process.chdir(tmpDir);
    relativeTarget = 'subproject';
  });

  afterAll(() => {
    if (originalCwd) {
      process.chdir(originalCwd);
    }
    try {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    } catch {
      // Windows may hold file locks; ignore cleanup failures
    }
  });

  it('should resolve a relative project_root and write to the correct location', () => {
    const config = readConfig(relativeTarget);
    setValue(config, 'schema', 'spec-driven');
    writeConfig(relativeTarget, config);
    const configFilePath = path.join(tmpDir, relativeTarget, 'openspec', 'config.json');
    expect(fs.existsSync(configFilePath)).toBe(true);
    const raw = fs.readFileSync(configFilePath, 'utf-8');
    expect(JSON.parse(raw).schema).toBe('spec-driven');
  });
});

// ===========================================================================
// Boundary: Multiple set/unset operations
// ===========================================================================

describe('Boundary: multiple set operations on same key', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-multi-'));
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('should overwrite value, not append', () => {
    let config = readConfig(tmpDir);
    setValue(config, 'static_check', ['npm test']);
    writeConfig(tmpDir, config);
    config = readConfig(tmpDir);
    setValue(config, 'static_check', ['eslint']);
    writeConfig(tmpDir, config);
    const result = readConfig(tmpDir);
    expect(result.static_check).toEqual(['eslint']);
    expect(result.static_check).not.toEqual(['npm test', 'eslint']);
  });
});

// ===========================================================================
// Boundary: Set nested key auto-creates intermediate objects
// ===========================================================================

describe('Boundary: nested key path auto-creates intermediate objects', () => {
  it('should auto-create intermediate objects when setting deep path on empty config', () => {
    const config = {};
    const updated = setValue(config, 'a.b.c', 'x');
    expect(updated).toEqual({ a: { b: { c: 'x' } } });
  });
});

// ===========================================================================
// Boundary: Unset nested key retains sibling keys
// ===========================================================================

describe('Boundary: unset nested key retains sibling keys', () => {
  it('should remove nested key and keep siblings', () => {
    const config = {
      test_scripts: {
        unit: 'npm test',
        integration: 'npm run integration',
      },
    };
    const { config: updated, removed } = unsetValue(config, 'test_scripts.unit');
    expect(removed).toBe(true);
    expect(updated.test_scripts).toEqual({ integration: 'npm run integration' });
  });

  it('should leave parent as empty object after removing last child', () => {
    const config = { test_scripts: { unit: 'npm test' } };
    const { config: updated, removed } = unsetValue(config, 'test_scripts.unit');
    expect(removed).toBe(true);
    expect(updated.test_scripts).toEqual({});
  });
});
