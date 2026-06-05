/**
 * Tests for test-get-framework-config -- MCP tool returning hardcoded
 * test and coverage command configuration per framework.
 *
 * Covers:
 * - AC-5: Known frameworks return correct config (jest, vitest, vite-plus, bun, rust)
 * - Reverse AC-5: Unknown framework returns error
 * - AC-11: Framework with no coverage tool (never applicable here since all 5 have coverage)
 * - Boundary: empty framework name, special characters, etc.
 *
 * @see openspec/changes/unit-test-coverage-report/test-design.md
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
}

const EXPECTED_CONFIGS: Record<string, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    test_cmd: 'npx jest --verbose',
    coverage_cmd: 'npx jest --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
  },
  vitest: {
    framework: 'vitest',
    test_cmd: 'npx vitest run --reporter=verbose',
    coverage_cmd: 'npx vitest run --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
  },
  'vite-plus': {
    framework: 'vite-plus',
    test_cmd: 'vp test',
    coverage_cmd: 'vp test --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
  },
  bun: {
    framework: 'bun',
    test_cmd: 'bun test',
    coverage_cmd: 'bun test --coverage',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
  },
  rust: {
    framework: 'rust',
    test_cmd: 'cargo test',
    coverage_cmd: 'cargo llvm-cov --all --coverage',
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
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
    expect(() => runTestGetFrameworkConfig({ framework: null as unknown as string })).toThrow();
    expect(() => runTestGetFrameworkConfig({ framework: undefined as unknown as string })).toThrow();
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
  it('should handle framework name with leading/trailing whitespace', () => {
    const result = runTestGetFrameworkConfig({ framework: '  vitest  ' });
    expect(result.framework).toBe('vitest');
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
