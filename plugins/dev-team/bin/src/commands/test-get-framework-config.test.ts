/**
 * Tests for test-get-framework-config -- MCP tool returning hardcoded
 * test and coverage command configuration per framework.
 *
 * Covers:
 * - AC-5: Known frameworks return correct config (jest, vitest, vite-plus, bun, rust)
 * - Reverse AC-5: Unknown framework returns error
 * - AC-11: Framework with no coverage tool (never applicable here since all 5 have coverage)
 * - AC-1: vitest coverage_artifacts and coverage_cleanup fields
 * - AC-2: rust coverage_artifacts and coverage_cleanup fields
 * - AC-3: All five frameworks have non-empty coverage_artifacts and coverage_cleanup
 * - Boundary: empty framework name, special characters, etc.
 *
 * @see openspec/changes/unified-coverage-artifacts/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import {
  runTestGetFrameworkConfig,
  getSupportedFrameworks,
  getDefaultGlobForFramework,
} from './test-get-framework-config';

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
    coverage_cmd: 'npx jest --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**'],
    coverage_cleanup: ['coverage', '.nyc_output'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  vitest: {
    framework: 'vitest',
    test_cmd: 'npx vitest run --reporter=verbose',
    coverage_cmd: 'npx vitest run --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  'vite-plus': {
    framework: 'vite-plus',
    test_cmd: 'vp test',
    coverage_cmd: 'vp test --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  bun: {
    framework: 'bun',
    test_cmd: 'bun test',
    coverage_cmd: 'bun test --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
  },
  rust: {
    framework: 'rust',
    test_cmd: 'cargo test',
    coverage_cmd: 'cargo llvm-cov --all --coverage',
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/**', 'target/llvm-cov/**'],
    coverage_cleanup: ['coverage', 'target/llvm-cov'],
    default_glob: '**/tests/**/*.rs',
  },
};

// ===========================================================================
// AC-5: Known frameworks
// ===========================================================================

describe('getFrameworkConfig -- known frameworks (AC-5)', () => {
  for (const [fw, expected] of Object.entries(EXPECTED_CONFIGS)) {
    it(`should return correct config for "${fw}" framework`, () => {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result).toEqual(expected);
    });
  }

  it('should return correct coverage_format for istanbul frameworks', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun']) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.coverage_format).toBe('istanbul');
    }
  });

  it('should return correct coverage_format for llvm-cov (rust)', () => {
    const result = runTestGetFrameworkConfig({ framework: 'rust' });
    expect(result.coverage_format).toBe('llvm-cov');
  });

  it('should return coverage_output path for all frameworks', () => {
    for (const fw of Object.keys(EXPECTED_CONFIGS)) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.coverage_output).toBeTruthy();
    }
  });
});

// ===========================================================================
// Reverse AC-5: Unknown framework
// ===========================================================================

describe('getFrameworkConfig -- unknown framework (reverse AC-5)', () => {
  it('should throw for unknown framework name "unknown-framework"', () => {
    expect(() => runTestGetFrameworkConfig({ framework: 'unknown-framework' })).toThrow();
  });

  it('should throw for empty framework name', () => {
    expect(() => runTestGetFrameworkConfig({ framework: '' })).toThrow();
  });

  it('should throw for nullish framework name', () => {
    expect(() => runTestGetFrameworkConfig({ framework: null! })).toThrow();
    expect(() => runTestGetFrameworkConfig({ framework: undefined! })).toThrow();
  });
});

// ===========================================================================
// AC-11: Framework with no coverage tool
// ===========================================================================

describe('getFrameworkConfig -- coverage tool availability (AC-11)', () => {
  it('should return coverage_cmd for all known frameworks', () => {
    for (const fw of Object.keys(EXPECTED_CONFIGS)) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.coverage_cmd).toBeTruthy();
    }
  });
});

// ===========================================================================
// Edge cases -- parameter type systematic mapping
// ===========================================================================

describe('getFrameworkConfig -- edge cases', () => {
  it('should reject framework name with leading/trailing whitespace', () => {
    expect(() => runTestGetFrameworkConfig({ framework: '  vitest  ' })).toThrow();
  });

  it('should be case-sensitive and reject uppercase "Vitest"', () => {
    expect(() => runTestGetFrameworkConfig({ framework: 'Vitest' })).toThrow();
  });

  it('should handle framework names with special regex characters (e.g. "jest?")', () => {
    expect(() => runTestGetFrameworkConfig({ framework: 'jest?' })).toThrow();
  });

  it('should handle a very long framework name string (>1000 chars)', () => {
    const longName = 'x'.repeat(1001);
    expect(() => runTestGetFrameworkConfig({ framework: longName })).toThrow();
  });
});

// ===========================================================================
// AC-1, AC-2: Framework-specific coverage_artifacts and coverage_cleanup
// ===========================================================================

describe('getFrameworkConfig -- coverage_artifacts 和 coverage_cleanup 字段 (AC-1, AC-2)', () => {
  it('vitest 框架应返回 coverage_artifacts: ["coverage/**"] 和 coverage_cleanup: ["coverage", ".nyc_output", "test-stderr.txt"]', () => {
    const result = runTestGetFrameworkConfig({ framework: 'vitest' });
    expect(result.coverage_artifacts).toEqual(['coverage/**']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('rust 框架应返回 coverage_artifacts 包含 coverage/** 和 target/llvm-cov/**', () => {
    const result = runTestGetFrameworkConfig({ framework: 'rust' });
    expect(result.coverage_artifacts).toContain('coverage/**');
    expect(result.coverage_artifacts).toContain('target/llvm-cov/**');
    expect(result.coverage_artifacts).toHaveLength(2);
  });

  it('rust 框架应返回 coverage_cleanup 包含 coverage 和 target/llvm-cov', () => {
    const result = runTestGetFrameworkConfig({ framework: 'rust' });
    expect(result.coverage_cleanup).toContain('coverage');
    expect(result.coverage_cleanup).toContain('target/llvm-cov');
    expect(result.coverage_cleanup).toHaveLength(2);
  });

  it('jest 框架应返回 coverage_artifacts 和 coverage_cleanup 字段', () => {
    const result = runTestGetFrameworkConfig({ framework: 'jest' });
    expect(result.coverage_artifacts).toBeDefined();
    expect(result.coverage_cleanup).toBeDefined();
    expect(result.coverage_artifacts).toEqual(['coverage/**']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output']);
  });

  it('vite-plus 框架应返回 coverage_artifacts 和 coverage_cleanup 字段', () => {
    const result = runTestGetFrameworkConfig({ framework: 'vite-plus' });
    expect(result.coverage_artifacts).toEqual(['coverage/**']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('bun 框架应返回 coverage_cleanup 不含 .nyc_output（仅 coverage）', () => {
    const result = runTestGetFrameworkConfig({ framework: 'bun' });
    expect(result.coverage_artifacts).toEqual(['coverage/**']);
    expect(result.coverage_cleanup).toEqual(['coverage']);
  });
});

// ===========================================================================
// AC-3: 全部框架 coverage_artifacts 和 coverage_cleanup 非空
// ===========================================================================

describe('getFrameworkConfig -- 全部框架 coverage 字段非空 (AC-3)', () => {
  const ALL_FRAMEWORKS = ['jest', 'vitest', 'vite-plus', 'bun', 'rust'];

  it('每个框架的 coverage_artifacts 应为非空字符串数组', () => {
    for (const fw of ALL_FRAMEWORKS) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(Array.isArray(result.coverage_artifacts)).toBe(true);
      expect(result.coverage_artifacts.length).toBeGreaterThan(0);
    }
  });

  it('每个框架的 coverage_cleanup 应为非空字符串数组', () => {
    for (const fw of ALL_FRAMEWORKS) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(Array.isArray(result.coverage_cleanup)).toBe(true);
      expect(result.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });

  it('返回对象应包含全部 8 个字段（原有 5 字段 + coverage_artifacts + coverage_cleanup + default_glob）', () => {
    for (const fw of ALL_FRAMEWORKS) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      const keys = Object.keys(result);
      expect(keys).toContain('coverage_artifacts');
      expect(keys).toContain('coverage_cleanup');
      expect(keys).toContain('default_glob');
      expect(keys.length).toBe(8);
    }
  });

  it('coverage_artifacts 中每个元素均为非空字符串', () => {
    for (const fw of ALL_FRAMEWORKS) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      for (const artifact of result.coverage_artifacts) {
        expect(typeof artifact).toBe('string');
        expect(artifact.length).toBeGreaterThan(0);
      }
    }
  });
});

// ===========================================================================
// Helper: getSupportedFrameworks
// ===========================================================================

describe('getSupportedFrameworks', () => {
  it('should return all five known frameworks', () => {
    const frameworks = getSupportedFrameworks();
    expect(frameworks).toEqual(['jest', 'vitest', 'vite-plus', 'bun', 'rust']);
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

  it('should throw for unknown framework', () => {
    expect(() => getDefaultGlobForFramework('unknown')).toThrow();
  });
});
