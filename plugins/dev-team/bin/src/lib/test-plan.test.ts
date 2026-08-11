/**
 * Unit tests for lib/test-plan — planId, pathFilter, file scoping, suite scope.
 */

import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { configSchema, type OpenSpecConfig, type TestSuite } from '../schemas';
import {
  derivePlanId,
  findSuite,
  isInSuiteScope,
  pathFilterFromPlan,
  resolveAllSuites,
  resolvePlanFiles,
} from './test-plan';

function suite(overrides: Partial<TestSuite> & Pick<TestSuite, 'root' | 'framework'>): TestSuite {
  return {
    cwd: '.',
    coverage: { lines: 80, branches: 70, functions: 75 },
    mutation: { score: 70 },
    ...overrides,
  };
}

function config(tests: TestSuite[]): OpenSpecConfig {
  return configSchema.parse({ schema: 'spec-driven', tests });
}

describe('derivePlanId', () => {
  it("root='.' → framework only", () => {
    expect(derivePlanId('.', 'vitest')).toBe('vitest');
  });

  it('nested root sanitizes separators', () => {
    expect(derivePlanId('plugins/dev-team/bin/src', 'vite-plus')).toBe(
      'plugins_dev-team_bin_src_vite-plus',
    );
  });

  it('backslash root normalizes like forward slash', () => {
    expect(derivePlanId('plugins\\dev-team\\bin', 'vitest')).toBe('plugins_dev-team_bin_vitest');
  });
});

describe('pathFilterFromPlan', () => {
  it('cwd === root → "."', () => {
    expect(pathFilterFromPlan({ cwd: 'pkg', root: 'pkg' })).toBe('.');
  });

  it('cwd parent of root → relative child path', () => {
    expect(
      pathFilterFromPlan({ cwd: 'plugins/dev-team/bin', root: 'plugins/dev-team/bin/src' }),
    ).toBe('src');
  });
});

describe('resolvePlanFiles', () => {
  it('keeps files under root and rewrites relative to cwd', () => {
    expect(resolvePlanFiles(['a/b/x.test.ts', 'a/c/y.test.ts'], { cwd: 'a', root: 'a/b' })).toEqual(
      ['b/x.test.ts'],
    );
  });

  it('undefined files stays undefined', () => {
    expect(resolvePlanFiles(undefined, { cwd: '.', root: '.' })).toBeUndefined();
  });
});

describe('resolveAllSuites / findSuite', () => {
  const projectRoot = path.resolve('/virtual-project-root');

  it('resolveAllSuites maps cwd parent of root', () => {
    const [resolved] = resolveAllSuites(
      [suite({ root: 'plugins/dev-team/bin/src', cwd: '..', framework: 'vite-plus' })],
      projectRoot,
    );
    expect(resolved.root).toBe('plugins/dev-team/bin/src');
    expect(resolved.cwd).toBe('plugins/dev-team/bin');
    expect(resolved.frameworkConfig.framework).toBe('vite-plus');
  });

  it('preserves order; findSuite matches root then framework', () => {
    const a = suite({ root: 'pkg-a', framework: 'vitest' });
    const b = suite({ root: 'pkg-b', framework: 'jest' });
    const cfg = config([a, b]);
    const resolved = resolveAllSuites(cfg.tests ?? [], projectRoot);
    expect(resolved).toHaveLength(2);
    expect(findSuite(cfg, 'vitest', 'pkg-a')?.root).toBe('pkg-a');
    expect(findSuite(cfg, 'jest')?.root).toBe('pkg-b');
    expect(findSuite(cfg)?.root).toBe('pkg-a');
  });
});

describe('isInSuiteScope', () => {
  it('matches under root + default_glob; suite-local exclude without config', () => {
    const s = suite({
      root: 'pkg',
      framework: 'vitest',
      excludes: ['**/skip/**'],
    });
    expect(isInSuiteScope('pkg/foo.test.ts', s)).toBe(true);
    expect(isInSuiteScope('pkg/skip/x.test.ts', s)).toBe(false);
    expect(isInSuiteScope('other/foo.test.ts', s)).toBe(false);
  });

  it('with config uses any-suite excludes (isFileExcluded)', () => {
    const a = suite({ root: 'a', framework: 'vitest', excludes: ['**/legacy/**'] });
    const b = suite({ root: 'b', framework: 'vitest' });
    const cfg = config([a, b]);
    // File under b, but excluded by suite a's pattern only applies under a —
    // global isFileExcluded still keys off each suite's root.
    expect(isInSuiteScope('b/foo.test.ts', b, cfg)).toBe(true);
    expect(isInSuiteScope('a/legacy/old.test.ts', a, cfg)).toBe(false);
  });
});
