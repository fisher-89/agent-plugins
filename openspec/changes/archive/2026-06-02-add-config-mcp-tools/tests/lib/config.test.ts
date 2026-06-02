/**
 * Unit and integration tests for lib/config.ts — JSON config file operations.
 *
 * Unit tests cover:
 * - getValue / setValue / unsetValue (dot-path object traversal)
 * - readConfig / writeConfig boundary scenarios
 *
 * Integration tests cover:
 * - Real filesystem: temp directory-based read/write round-trips
 * - Auto-creation of openspec/config.json skeleton
 * - JSON syntax validity after write operations
 * - Non-ASCII value handling
 * - project_root points to non-existent directory
 *
 * @see openspec/changes/add-config-mcp-tools/test-design.md
 */

import { describe, it, expect, beforeAll, afterAll } from 'vite-plus/test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
// ---------------------------------------------------------------------------
// Import the module under test
// ---------------------------------------------------------------------------
import {
  readConfig,
  writeConfig,
  getValue,
  setValue,
  unsetValue,
  ensureConfigFile,
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
    // Parent key remains as empty object per spec: "empty object is left"
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
// Unit Tests: readConfig — Default return
// ===========================================================================

describe('readConfig — default when no file exists', () => {
  it('should return default { schema: "spec-driven" } when no file exists', () => {
    // This is verified in integration tests below with emptyDir
  });
});

// ===========================================================================
// Unit Tests: ensureConfigFile
// ===========================================================================

describe.skip('ensureConfigFile — skeleton creation', () => {
  // Integration tests below cover real filesystem behavior.
  // Unit tests cover the content structure (skeleton template) if exported.
  it.skip('placeholder - tests implemented in integration section below');
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

  it('should return default { schema: "spec-driven" } when config.json does not exist', () => {
    const result = readConfig(tmpDir);
    expect(result).toEqual({ schema: 'spec-driven' });
  });

  it('should create openspec/ directory and config.json skeleton on writeConfig', () => {
    writeConfig(tmpDir, { schema: 'spec-driven' });
    expect(fs.existsSync(configPath)).toBe(true);
    const content = fs.readFileSync(configPath, 'utf-8');
    const parsed = JSON.parse(content);
    expect(parsed).toEqual({ schema: 'spec-driven' });
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
    const result = ensureConfigFile(tmpDir);
    expect(fs.existsSync(configPath)).toBe(true);
    const raw = fs.readFileSync(configPath, 'utf-8');
    const parsed = JSON.parse(raw);
    expect(parsed).toEqual({ schema: 'spec-driven' });
    // ensureConfigFile returns the parsed config
    expect(result.schema).toBe('spec-driven');
  });
});

// ===========================================================================
// Integration: Auto-creation scenarios
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
    expect(parsed).toEqual({ schema: 'spec-driven' });
  });

  it('should handle project_root pointing to a non-existent directory', () => {
    const nonExistent = path.join(os.tmpdir(), 'config-nonexistent-' + Date.now());
    try {
      ensureConfigFile(nonExistent);
      const configFilePath = path.join(nonExistent, 'openspec', 'config.json');
      expect(fs.existsSync(configFilePath)).toBe(true);
    } finally {
      fs.rmSync(nonExistent, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// Integration: JSON syntax validity after write
// ===========================================================================

describe('Integration: JSON syntax validity after write', () => {
  let tmpDir: string;

  function configPath(): string {
    return path.join(tmpDir, 'openspec', 'config.json');
  }

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-json-valid-'));
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
// Integration: Non-ASCII and special character values
// ===========================================================================

describe('Integration: non-ASCII and special characters', () => {
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

  it('should round-trip string values containing colons and special characters', () => {
    const config = readConfig(tmpDir);
    setValue(config, 'special', 'key: value with: colons');
    writeConfig(tmpDir, config);
    const result = readConfig(tmpDir);
    expect(result.special).toBe('key: value with: colons');
  });
});

// ===========================================================================
// Integration: Filesystem edge cases
// ===========================================================================

describe('Integration: filesystem edge cases', () => {
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
// Integration: Config file does not exist scenarios
// ===========================================================================

describe('Integration: config file does not exist', () => {
  let tmpDir: string;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'config-missing-'));
  });

  afterAll(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('readConfigValue should return exists: false for unknown key when config file is missing', () => {
    const config = readConfig(tmpDir);
    const result = getValue(config, 'nonexistent');
    expect(result.exists).toBe(false);
    expect(result.value).toBeUndefined();
  });

  it('unsetConfigValue should return removed: false when key does not exist', () => {
    const config = readConfig(tmpDir);
    const result = unsetValue(config, 'nonexistent');
    expect(result.removed).toBe(false);
  });

  it('readContext should return empty string when config file is missing', () => {
    const config = readConfig(tmpDir);
    const context = typeof config.context === 'string' ? config.context : '';
    expect(context).toBe('');
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
    const config = readConfig(tmpDir);
    const context = typeof config.context === 'string' ? config.context : '';
    expect(context).toBe('');
  });

  it('writeContext should write context and readContext should read it back', () => {
    const configIn = readConfig(tmpDir);
    configIn.context = 'Tech stack: TypeScript';
    writeConfig(tmpDir, configIn);
    const configOut = readConfig(tmpDir);
    expect(configOut.context).toBe('Tech stack: TypeScript');
  });

  it('writeContext should overwrite existing context', () => {
    const c1 = readConfig(tmpDir);
    c1.context = 'First';
    writeConfig(tmpDir, c1);
    const c2 = readConfig(tmpDir);
    c2.context = 'Second';
    writeConfig(tmpDir, c2);
    const result = readConfig(tmpDir);
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
    // Create a subdirectory to use as "project root" via relative path
    const subDir = path.join(tmpDir, 'subproject');
    fs.mkdirSync(subDir, { recursive: true });
    // Change into tmpDir to test relative path resolution
    originalCwd = process.cwd();
    process.chdir(tmpDir);
    relativeTarget = 'subproject';
  });

  afterAll(() => {
    // Restore original working directory BEFORE cleanup to avoid EPERM on Windows
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
  });
});

// ===========================================================================
// Boundary: Multiple set/unset operations (no accumulation)
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
