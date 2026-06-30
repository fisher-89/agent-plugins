/**
 * Tests for lib/test-framework -- internal module providing the hardcoded
 * test framework registry and query functions.
 *
 * Covers:
 * - AC-1: All eight frameworks registered with correct fields
 * - AC-8: Each framework's coverage_cmd, coverage_format, coverage_output,
 *   coverage_artifacts, coverage_cleanup, default_glob are correct
 * - AC-5: Unknown framework returns error
 * - AC-1: vitest coverage_artifacts and coverage_cleanup fields
 * - AC-2: rust coverage_artifacts and coverage_cleanup fields
 * - AC-3: All frameworks have non-empty coverage_artifacts and coverage_cleanup
 * - Boundary: empty framework name, special characters, etc.
 *
 * @see openspec/changes/merge-framework-config-into-detect/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import { getFrameworkConfig, getDefaultGlobForFramework } from './test-framework';

// ---------------------------------------------------------------------------
// Expected configs per spec
// ---------------------------------------------------------------------------

interface FrameworkConfig {
  framework: string;
  test_cmd: string;
  coverage_cmd: string;
  coverage_format: string;
  coverage_output: string;
  coverage_artifacts: string[];
  coverage_cleanup: string[];
  default_glob: string;
}

const EXPECTED_CONFIGS: Record<string, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    test_cmd: 'npx jest --verbose',
    coverage_cmd: 'npx jest --coverage --coverageReporters=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  vitest: {
    framework: 'vitest',
    test_cmd: 'npx vitest run --reporter=verbose',
    coverage_cmd: 'npx vitest run --coverage --coverage.reporter=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  'vite-plus': {
    framework: 'vite-plus',
    test_cmd: 'vp test',
    coverage_cmd: 'vp test --coverage --coverage.reporter=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  bun: {
    framework: 'bun',
    test_cmd: 'bun test',
    coverage_cmd: 'bun test --coverage --coverageReporters=json-summary',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  rust: {
    framework: 'rust',
    test_cmd: 'cargo test',
    coverage_cmd: 'cargo llvm-cov --json',
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', 'target/llvm-cov'],
    default_glob: '**/tests/**/*.rs',
  },
  'node-test': {
    framework: 'node-test',
    test_cmd: 'node --test',
    coverage_cmd: 'node --test --experimental-test-coverage',
    coverage_format: 'node-test',
    coverage_output: 'coverage/node-test-output.txt',
    coverage_artifacts: ['coverage/node-test-output.txt'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.test.{mjs,js,cjs}',
  },
  go: {
    framework: 'go',
    test_cmd: 'go test ./...',
    coverage_cmd:
      'go test -coverprofile=coverage.out -covermode=atomic ./... && mkdir -p coverage && go tool cover -func=coverage.out > coverage/func-summary.txt',
    coverage_format: 'go-cover',
    coverage_output: 'coverage/func-summary.txt',
    coverage_artifacts: ['coverage/func-summary.txt'],
    coverage_cleanup: ['coverage', 'coverage.out'],
    default_glob: '**/*_test.go',
  },
  pytest: {
    framework: 'pytest',
    test_cmd: 'pytest -v',
    coverage_cmd: 'pytest --cov=. --cov-report=json --cov-branch -q',
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    coverage_artifacts: ['coverage.json'],
    coverage_cleanup: ['.coverage', 'htmlcov'],
    default_glob: '**/test_*.py',
  },
};

// ===========================================================================
// Known frameworks
// ===========================================================================

describe('getFrameworkConfig -- known frameworks', () => {
  for (const [fw, expected] of Object.entries(EXPECTED_CONFIGS)) {
    it(`should return correct config for "${fw}" framework`, () => {
      const result = getFrameworkConfig(fw);
      expect(result).toEqual(expected);
    });
  }

  it('should return correct coverage_format for istanbul frameworks', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun']) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_format).toBe('istanbul');
    }
  });

  it('should return correct coverage_format for llvm-cov (rust)', () => {
    const result = getFrameworkConfig('rust');
    expect(result.coverage_format).toBe('llvm-cov');
  });

  it('should return coverage_output path for all frameworks', () => {
    for (const fw of Object.keys(EXPECTED_CONFIGS)) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_output).toBeTruthy();
    }
  });
});

// ===========================================================================
// Unknown framework
// ===========================================================================

describe('getFrameworkConfig -- unknown framework', () => {
  it('should throw for unknown framework name "unknown-framework"', () => {
    expect(() => getFrameworkConfig('unknown-framework')).toThrow();
  });

  it('should throw for empty framework name', () => {
    expect(() => getFrameworkConfig('')).toThrow();
  });

  it('should throw for nullish framework name', () => {
    expect(() => getFrameworkConfig(null!)).toThrow();
    expect(() => getFrameworkConfig(undefined!)).toThrow();
  });
});

// ===========================================================================
// Coverage tool availability
// ===========================================================================

describe('getFrameworkConfig -- coverage tool availability', () => {
  it('should return coverage_cmd for all known frameworks', () => {
    for (const fw of Object.keys(EXPECTED_CONFIGS)) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_cmd).toBeTruthy();
    }
  });
});

// ===========================================================================
// Edge cases -- parameter type systematic mapping
// ===========================================================================

describe('getFrameworkConfig -- edge cases', () => {
  it('should reject framework name with leading/trailing whitespace', () => {
    expect(() => getFrameworkConfig('  vitest  ')).toThrow();
  });

  it('should be case-sensitive and reject uppercase "Vitest"', () => {
    expect(() => getFrameworkConfig('Vitest')).toThrow();
  });

  it('should handle framework names with special regex characters (e.g. "jest?")', () => {
    expect(() => getFrameworkConfig('jest?')).toThrow();
  });

  it('should handle a very long framework name string (>1000 chars)', () => {
    const longName = 'x'.repeat(1001);
    expect(() => getFrameworkConfig(longName)).toThrow();
  });

  it('未知框架名抛出错误，错误信息列出全部八个框架名', () => {
    const allFrameworks = [
      'jest',
      'vitest',
      'vite-plus',
      'bun',
      'rust',
      'node-test',
      'go',
      'pytest',
    ];
    expect(() => getFrameworkConfig('mocha')).toThrow(new RegExp(allFrameworks.join('|')));
  });
});

// ===========================================================================
// Framework-specific coverage_artifacts and coverage_cleanup
// ===========================================================================

describe('getFrameworkConfig -- coverage_artifacts and coverage_cleanup', () => {
  it('vitest should return coverage_artifacts and coverage_cleanup correctly', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('rust should return single-element JSON summary path for coverage_artifacts', () => {
    const result = getFrameworkConfig('rust');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_artifacts).not.toContain('target/llvm-cov/**');
    expect(result.coverage_artifacts).toHaveLength(1);
  });

  it('rust should return coverage_cleanup containing coverage and target/llvm-cov', () => {
    const result = getFrameworkConfig('rust');
    expect(result.coverage_cleanup).toContain('coverage');
    expect(result.coverage_cleanup).toContain('target/llvm-cov');
    expect(result.coverage_cleanup).toHaveLength(2);
  });

  it('jest should return coverage_artifacts and coverage_cleanup fields', () => {
    const result = getFrameworkConfig('jest');
    expect(result.coverage_artifacts).toBeDefined();
    expect(result.coverage_cleanup).toBeDefined();
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output']);
  });

  it('vite-plus should return coverage_artifacts and coverage_cleanup fields', () => {
    const result = getFrameworkConfig('vite-plus');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('bun should return coverage_cleanup without .nyc_output (only coverage)', () => {
    const result = getFrameworkConfig('bun');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_cleanup).toEqual(['coverage']);
  });
});

// ===========================================================================
// All frameworks coverage fields non-empty
// ===========================================================================

describe('getFrameworkConfig -- all frameworks coverage fields non-empty', () => {
  const ALL_EIGHT = ['jest', 'vitest', 'vite-plus', 'bun', 'rust', 'node-test', 'go', 'pytest'];

  it('each framework should have non-empty coverage_artifacts array', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.coverage_artifacts)).toBe(true);
      expect(result.coverage_artifacts.length).toBeGreaterThan(0);
    }
  });

  it('each framework should have non-empty coverage_cleanup array', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.coverage_cleanup)).toBe(true);
      expect(result.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });

  it('each framework should return all 8 fields', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      const keys = Object.keys(result);
      expect(keys).toContain('coverage_artifacts');
      expect(keys).toContain('coverage_cleanup');
      expect(keys).toContain('default_glob');
      expect(keys.length).toBe(8);
    }
  });

  it('coverage_artifacts should contain only non-empty strings', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      for (const artifact of result.coverage_artifacts) {
        expect(typeof artifact).toBe('string');
        expect(artifact.length).toBeGreaterThan(0);
      }
    }
  });
});

// ===========================================================================
// Helper: getDefaultGlobForFramework
// ===========================================================================

describe('getDefaultGlobForFramework', () => {
  it('should return the correct default glob for JS/TS frameworks', () => {
    expect(getDefaultGlobForFramework('jest')).toBe('**/*.{test,spec}.{js,ts,jsx,tsx}');
    expect(getDefaultGlobForFramework('vitest')).toBe('**/*.{test,spec}.{js,ts,jsx,tsx}');
    expect(getDefaultGlobForFramework('vite-plus')).toBe('**/*.{test,spec}.{js,ts,jsx,tsx}');
    expect(getDefaultGlobForFramework('bun')).toBe('**/*.{test,spec}.{js,ts,jsx,tsx}');
  });

  it('should return the correct default glob for rust', () => {
    expect(getDefaultGlobForFramework('rust')).toBe('**/tests/**/*.rs');
  });

  it('should return the correct default glob for node-test', () => {
    expect(getDefaultGlobForFramework('node-test')).toBe('**/*.test.{mjs,js,cjs}');
  });

  it('should return the correct default glob for go', () => {
    expect(getDefaultGlobForFramework('go')).toBe('**/*_test.go');
  });

  it('should return the correct default glob for pytest', () => {
    expect(getDefaultGlobForFramework('pytest')).toBe('**/test_*.py');
  });

  it('should throw for unknown framework', () => {
    expect(() => getDefaultGlobForFramework('unknown')).toThrow();
  });
});

// ===========================================================================
// JSON-only coverage configuration
// @see openspec/changes/simplify-test-report-schema/test-design.md
// ===========================================================================

const JSON_ONLY_EXPECTED_CONFIGS: Record<
  string,
  Pick<FrameworkConfig, 'coverage_cmd' | 'coverage_artifacts' | 'coverage_cleanup'>
> = {
  jest: {
    coverage_cmd: 'npx jest --coverage --coverageReporters=json-summary',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output'],
  },
  vitest: {
    coverage_cmd: 'npx vitest run --coverage --coverage.reporter=json-summary',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
  },
  'vite-plus': {
    coverage_cmd: 'vp test --coverage --coverage.reporter=json-summary',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
  },
  bun: {
    coverage_cmd: 'bun test --coverage --coverageReporters=json-summary',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage'],
  },
  rust: {
    coverage_cmd: 'cargo llvm-cov --json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', 'target/llvm-cov'],
  },
};

describe('getFrameworkConfig -- JSON-only coverage config', () => {
  it('jest should return JSON-only coverage_cmd with single-element coverage_artifacts', () => {
    const result = getFrameworkConfig('jest');
    expect(result.coverage_cmd).toBe(JSON_ONLY_EXPECTED_CONFIGS.jest.coverage_cmd);
    expect(result.coverage_artifacts).toEqual(JSON_ONLY_EXPECTED_CONFIGS.jest.coverage_artifacts);
  });

  it('vitest should return coverage_cmd with --coverage.reporter=json-summary', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.coverage_cmd).toBe(JSON_ONLY_EXPECTED_CONFIGS.vitest.coverage_cmd);
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('vite-plus should return coverage_cmd with --coverage.reporter=json-summary', () => {
    const result = getFrameworkConfig('vite-plus');
    expect(result.coverage_cmd).toBe(JSON_ONLY_EXPECTED_CONFIGS['vite-plus'].coverage_cmd);
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('bun should return coverage_cmd with --coverageReporters=json-summary', () => {
    const result = getFrameworkConfig('bun');
    expect(result.coverage_cmd).toBe(JSON_ONLY_EXPECTED_CONFIGS.bun.coverage_cmd);
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('rust should return cargo llvm-cov --json with coverage_artifacts not containing target/llvm-cov/**', () => {
    const result = getFrameworkConfig('rust');
    expect(result.coverage_cmd).toBe('cargo llvm-cov --json');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_artifacts).not.toContain('target/llvm-cov/**');
  });

  it('five JS/TS frameworks should have JSON reporter in coverage_cmd and single-element JSON artifacts', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust'] as const) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_cmd).toMatch(/json/i);
      expect(result.coverage_artifacts).toHaveLength(1);
      expect(result.coverage_artifacts[0]).toBe('coverage/coverage-summary.json');
    }
  });

  it('each framework coverage_cleanup should match expected', () => {
    for (const [fw, expected] of Object.entries(JSON_ONLY_EXPECTED_CONFIGS)) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_cleanup).toEqual(expected.coverage_cleanup);
    }
  });

  it('updated EXPECTED_CONFIGS should match getFrameworkConfig output for all five frameworks', () => {
    for (const [fw, jsonOnly] of Object.entries(JSON_ONLY_EXPECTED_CONFIGS)) {
      const base = EXPECTED_CONFIGS[fw];
      const merged = { ...base, ...jsonOnly };
      const result = getFrameworkConfig(fw);
      expect(result).toEqual(merged);
    }
  });
});

describe('getFrameworkConfig -- JSON-only boundary and exception', () => {
  it('five frameworks should have coverage_artifacts.length === 1 with value coverage/coverage-summary.json', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust']) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_artifacts).toHaveLength(1);
      expect(result.coverage_artifacts[0]).toBe('coverage/coverage-summary.json');
    }
  });

  it('five frameworks should have non-empty coverage_cmd containing coverage or JSON keyword', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust']) {
      const result = getFrameworkConfig(fw);
      expect(typeof result.coverage_cmd).toBe('string');
      expect(result.coverage_cmd.length).toBeGreaterThan(0);
      if (fw === 'rust') {
        expect(result.coverage_cmd.toLowerCase()).toMatch(/json|llvm-cov/);
      } else {
        expect(result.coverage_cmd.toLowerCase()).toContain('coverage');
      }
    }
  });
});

// ===========================================================================
// go / node-test / pytest framework entries
// @see openspec/changes/add-node-go-pytest-frameworks
// ===========================================================================

const FRAMEWORK_SPEC_EXPECTED: Record<string, FrameworkConfig> = {
  go: {
    framework: 'go',
    test_cmd: 'go test ./...',
    coverage_cmd:
      'go test -coverprofile=coverage.out -covermode=atomic ./... && mkdir -p coverage && go tool cover -func=coverage.out > coverage/func-summary.txt',
    coverage_format: 'go-cover',
    coverage_output: 'coverage/func-summary.txt',
    coverage_artifacts: ['coverage/func-summary.txt'],
    coverage_cleanup: ['coverage', 'coverage.out'],
    default_glob: '**/*_test.go',
  },
  'node-test': {
    framework: 'node-test',
    test_cmd: 'node --test',
    coverage_cmd: 'node --test --experimental-test-coverage',
    coverage_format: 'node-test',
    coverage_output: 'coverage/node-test-output.txt',
    coverage_artifacts: ['coverage/node-test-output.txt'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.test.{mjs,js,cjs}',
  },
  pytest: {
    framework: 'pytest',
    test_cmd: 'pytest -v',
    coverage_cmd: 'pytest --cov=. --cov-report=json --cov-branch -q',
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    coverage_artifacts: ['coverage.json'],
    coverage_cleanup: ['.coverage', 'htmlcov'],
    default_glob: '**/test_*.py',
  },
};

describe('getFrameworkConfig -- go', () => {
  it('should return coverage_format: "go-cover" with complete artifacts/cleanup/default_glob', () => {
    const result = getFrameworkConfig('go');
    expect(result).toEqual(FRAMEWORK_SPEC_EXPECTED.go);
  });
});

// ===========================================================================
// node-test coverage_cmd simplification
// ===========================================================================

describe('getFrameworkConfig -- node-test (coverage_cmd)', () => {
  it('coverage_cmd should be "node --test --experimental-test-coverage" (no tee/pipe)', () => {
    const result = getFrameworkConfig('node-test');
    expect(result.coverage_cmd).toBe('node --test --experimental-test-coverage');
    expect(result.coverage_cmd).not.toContain('|');
    expect(result.coverage_cmd).not.toContain('parse-node-test-coverage.mjs');
  });

  it('coverage_cmd should only contain --test and --experimental-test-coverage flags, no pipe/chain/tee/mjs', () => {
    const result = getFrameworkConfig('node-test');
    const flags = result.coverage_cmd.split(/\s+/).filter((s) => s.startsWith('-'));
    expect(flags).toEqual(['--test', '--experimental-test-coverage']);
    expect(result.coverage_cmd).not.toContain('|');
    expect(result.coverage_cmd).not.toContain('&&');
    expect(result.coverage_cmd).not.toContain('tee');
    expect(result.coverage_cmd).not.toContain('.mjs');
  });

  it('other frameworks (jest/vitest/vite-plus/bun) coverage_cmd should still contain their reporter params', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun'] as const) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_cmd).toMatch(/json|coverage/i);
    }
  });
});

// ===========================================================================
// node-test coverage_output
// ===========================================================================

describe('getFrameworkConfig -- node-test (coverage_output)', () => {
  it('node-test coverage_output should be "coverage/node-test-output.txt"', () => {
    const result = getFrameworkConfig('node-test');
    expect(result.coverage_output).toBe('coverage/node-test-output.txt');
    expect(result.coverage_output).not.toBe('coverage/coverage-summary.json');
  });

  it('node-test coverage_output should end with .txt, unlike other frameworks .json output', () => {
    const allEight = ['jest', 'vitest', 'vite-plus', 'bun', 'rust', 'node-test', 'go', 'pytest'];
    for (const fw of allEight) {
      const result = getFrameworkConfig(fw);
      if (fw === 'node-test' || fw === 'go') {
        expect(result.coverage_output).toMatch(/\.txt$/);
      } else {
        expect(result.coverage_output).toMatch(/\.json$/);
      }
    }
  });

  it('other frameworks coverage_output should remain unchanged (jest/vitest/vite-plus/bun still coverage/coverage-summary.json)', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun'] as const) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_output).toBe('coverage/coverage-summary.json');
    }
  });
});

// ===========================================================================
// node-test coverage_artifacts
// ===========================================================================

describe('getFrameworkConfig -- node-test (coverage_artifacts)', () => {
  it('coverage_artifacts should be ["coverage/node-test-output.txt"]', () => {
    const result = getFrameworkConfig('node-test');
    expect(result.coverage_artifacts).toEqual(['coverage/node-test-output.txt']);
    expect(result.coverage_artifacts).not.toEqual(['coverage/coverage-summary.json']);
  });

  it('coverage_cleanup should still be ["coverage"]', () => {
    const result = getFrameworkConfig('node-test');
    expect(result.coverage_cleanup).toEqual(['coverage']);
  });
});

// ===========================================================================
// node-test coverage_format
// ===========================================================================

describe('getFrameworkConfig -- node-test (coverage_format)', () => {
  it('coverage_format should still be "node-test"', () => {
    const result = getFrameworkConfig('node-test');
    expect(result.coverage_format).toBe('node-test');
  });
});

// ===========================================================================
// node-test coverage_output points to raw text file
// ===========================================================================

describe('getFrameworkConfig -- node-test (coverage_output text file)', () => {
  it('coverage_output should point to a .txt file, not .json', () => {
    const result = getFrameworkConfig('node-test');
    expect(result.coverage_output).toMatch(/\.txt$/);
    expect(result.coverage_output).not.toMatch(/\.json$/);
  });
});

// ===========================================================================
// EXPECTED_CONFIGS -- node-test entry
// ===========================================================================

describe('EXPECTED_CONFIGS -- node-test entry', () => {
  it('EXPECTED_CONFIGS should contain node-test entry with all 8 fields matching registry', () => {
    expect(EXPECTED_CONFIGS['node-test']).toBeDefined();
    const result = getFrameworkConfig('node-test');
    expect(result).toEqual(EXPECTED_CONFIGS['node-test']);
    expect(Object.keys(result)).toEqual(
      expect.arrayContaining([
        'framework',
        'test_cmd',
        'coverage_cmd',
        'coverage_format',
        'coverage_output',
        'coverage_artifacts',
        'coverage_cleanup',
        'default_glob',
      ]),
    );
  });
});

// ===========================================================================
// pytest
// ===========================================================================

describe('getFrameworkConfig -- pytest', () => {
  it('should return coverage_format: "coverage-py" and coverage_output: "coverage.json"', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.coverage_format).toBe('coverage-py');
    expect(result.coverage_output).toBe('coverage.json');
  });
});

// ===========================================================================
// Full eight-framework completeness
// ===========================================================================

describe('getFrameworkConfig -- eight-framework completeness', () => {
  const ALL_EIGHT = ['jest', 'vitest', 'vite-plus', 'bun', 'rust', 'node-test', 'go', 'pytest'];

  it('each of the eight frameworks should return non-empty test_cmd, coverage_cmd, coverage_artifacts, coverage_cleanup', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(result.test_cmd.length).toBeGreaterThan(0);
      expect(result.coverage_cmd.length).toBeGreaterThan(0);
      expect(result.coverage_artifacts.length).toBeGreaterThan(0);
      expect(result.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });
});
