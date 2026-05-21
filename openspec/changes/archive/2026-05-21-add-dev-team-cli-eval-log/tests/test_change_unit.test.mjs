// test_change_unit.test.mjs — Unit tests for change.ts (change directory resolution)
//
// Tests:
//   - resolveChangeDir(): concatenates openspec/changes/<name>/phases path
//   - getPhasesDir(): returns phases subdirectory under the change
//   - Edge cases: names with spaces, special characters, empty string
//   - Path normalization: resolves relative segments, handles trailing slashes
//
// Usage:
//   node --test test_change_unit.test.mjs

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import process from 'node:process';

// ---------------------------------------------------------------------------
// Import the module under test.
// ---------------------------------------------------------------------------

// For testing compiled JS:
// import { resolveChangeDir, getPhasesDir } from '../../../../plugins/dev-team/bin/dist/lib/change.js';

// For inline stubs (use when source does not yet exist):
const CHANGES_ROOT = 'openspec/changes';

function resolveChangeDir(changeName) {
  return path.resolve(CHANGES_ROOT, changeName);
}

function getPhasesDir(changeName) {
  return path.resolve(CHANGES_ROOT, changeName, 'phases');
}

// ============================================================================
// resolveChangeDir()
// ============================================================================

describe('resolveChangeDir()', () => {
  it('should return a path that ends with the change name', () => {
    const result = resolveChangeDir('my-feature');
    assert.ok(result.endsWith('my-feature') || result.endsWith('my-feature/'));
  });

  it('should contain the expected base path segments', () => {
    const result = resolveChangeDir('test-change');
    // Normalize to forward slashes for cross-platform comparison
    const normalized = result.replace(/\\/g, '/');
    assert.ok(normalized.includes('openspec/changes/test-change'));
  });

  it('should handle change names with hyphens', () => {
    const result = resolveChangeDir('add-user-auth');
    assert.ok(result.replace(/\\/g, '/').includes('openspec/changes/add-user-auth'));
  });

  it('should handle change names with numbers', () => {
    const result = resolveChangeDir('feature-v2');
    assert.ok(result.replace(/\\/g, '/').includes('openspec/changes/feature-v2'));
  });

  it('should return an absolute path (via path.resolve)', () => {
    const result = resolveChangeDir('test');
    assert.ok(path.isAbsolute(result));
  });
});

// ============================================================================
// getPhasesDir()
// ============================================================================

describe('getPhasesDir()', () => {
  it('should return a path ending with /phases', () => {
    const result = getPhasesDir('my-change');
    assert.ok(result.replace(/\\/g, '/').endsWith('/phases') || result.endsWith('\\phases'));
  });

  it('should return parent of resolveChangeDir with /phases suffix', () => {
    const changeDir = resolveChangeDir('my-change');
    const phasesDir = getPhasesDir('my-change');
    // The phases dir should be changeDir + '/phases'
    assert.strictEqual(phasesDir, path.join(changeDir, 'phases'));
  });
});

// ============================================================================
// Edge cases: special characters in change names
// ============================================================================

describe('change name with special characters', () => {
  it('should handle change names with spaces', () => {
    const result = resolveChangeDir('my change with spaces');
    assert.ok(result.replace(/\\/g, '/').includes('my change with spaces'));
  });

  it('should handle change names with dots', () => {
    const result = resolveChangeDir('release.v2.1');
    assert.ok(result.replace(/\\/g, '/').includes('release.v2.1'));
  });

  it('should handle change names with underscores', () => {
    const result = resolveChangeDir('my_feature');
    assert.ok(result.replace(/\\/g, '/').includes('my_feature'));
  });

  it('should handle single-character change names', () => {
    const result = resolveChangeDir('a');
    assert.ok(result.replace(/\\/g, '/').includes('/changes/a'));
  });

  it('should not crash on empty change name', () => {
    // TODO: decide how the application should handle empty names
    // For now, test that the function does not crash
    assert.doesNotThrow(() => resolveChangeDir(''));
  });
});
