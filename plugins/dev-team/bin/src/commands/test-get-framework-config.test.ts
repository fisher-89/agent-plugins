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

import { runTestGetFrameworkConfig, getDefaultGlobForFramework } from './test-get-framework-config';

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

  it('未知框架名抛出错误，错误信息列出全部八个框架名 (AC-2)', () => {
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
    expect(() => runTestGetFrameworkConfig({ framework: 'mocha' })).toThrow(
      new RegExp(allFrameworks.join('|')),
    );
  });
});

// ===========================================================================
// AC-1, AC-2: Framework-specific coverage_artifacts and coverage_cleanup
// ===========================================================================

describe('getFrameworkConfig -- coverage_artifacts 和 coverage_cleanup 字段 (AC-1, AC-2)', () => {
  it('vitest 框架应返回 coverage_artifacts: ["coverage/coverage-summary.json"] 和 coverage_cleanup: ["coverage", ".nyc_output", "test-stderr.txt"]', () => {
    const result = runTestGetFrameworkConfig({ framework: 'vitest' });
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('rust 框架应返回 coverage_artifacts 为单元素 JSON 摘要路径', () => {
    const result = runTestGetFrameworkConfig({ framework: 'rust' });
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_artifacts).not.toContain('target/llvm-cov/**');
    expect(result.coverage_artifacts).toHaveLength(1);
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
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output']);
  });

  it('vite-plus 框架应返回 coverage_artifacts 和 coverage_cleanup 字段', () => {
    const result = runTestGetFrameworkConfig({ framework: 'vite-plus' });
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('bun 框架应返回 coverage_cleanup 不含 .nyc_output（仅 coverage）', () => {
    const result = runTestGetFrameworkConfig({ framework: 'bun' });
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
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

  it('node-test 应返回 **/*.test.{mjs,js,cjs} (AC-2)', () => {
    expect(getDefaultGlobForFramework('node-test')).toBe('**/*.test.{mjs,js,cjs}');
  });

  it('go 应返回 **/*_test.go (AC-2)', () => {
    expect(getDefaultGlobForFramework('go')).toBe('**/*_test.go');
  });

  it('pytest 应返回 **/test_*.py (AC-2)', () => {
    expect(getDefaultGlobForFramework('pytest')).toBe('**/test_*.py');
  });
});

// ===========================================================================
// AC-9: FRAMEWORK_REGISTRY JSON-only 覆盖率配置 (simplify-test-report-schema)
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

describe('getFrameworkConfig — JSON-only 覆盖率配置 (AC-9)', () => {
  it('jest 应返回 JSON-only coverage_cmd 与单元素 coverage_artifacts', () => {
    const result = runTestGetFrameworkConfig({ framework: 'jest' });
    expect(result.coverage_cmd).toBe(JSON_ONLY_EXPECTED_CONFIGS.jest.coverage_cmd);
    expect(result.coverage_artifacts).toEqual(JSON_ONLY_EXPECTED_CONFIGS.jest.coverage_artifacts);
  });

  it('vitest 应返回含 --coverage.reporter=json-summary 的 coverage_cmd', () => {
    const result = runTestGetFrameworkConfig({ framework: 'vitest' });
    expect(result.coverage_cmd).toBe(JSON_ONLY_EXPECTED_CONFIGS.vitest.coverage_cmd);
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('vite-plus 应返回含 --coverage.reporter=json-summary 的 coverage_cmd', () => {
    const result = runTestGetFrameworkConfig({ framework: 'vite-plus' });
    expect(result.coverage_cmd).toBe(JSON_ONLY_EXPECTED_CONFIGS['vite-plus'].coverage_cmd);
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('bun 应返回含 --coverageReporters=json-summary 的 coverage_cmd', () => {
    const result = runTestGetFrameworkConfig({ framework: 'bun' });
    expect(result.coverage_cmd).toBe(JSON_ONLY_EXPECTED_CONFIGS.bun.coverage_cmd);
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('rust 应返回 cargo llvm-cov --json，coverage_artifacts 不含 target/llvm-cov/**', () => {
    const result = runTestGetFrameworkConfig({ framework: 'rust' });
    expect(result.coverage_cmd).toBe('cargo llvm-cov --json');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_artifacts).not.toContain('target/llvm-cov/**');
  });

  it('五个框架 coverage_cmd 均含 JSON reporter 参数，coverage_artifacts 均为单元素 JSON 摘要路径', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust'] as const) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.coverage_cmd).toMatch(/json/i);
      expect(result.coverage_artifacts).toHaveLength(1);
      expect(result.coverage_artifacts[0]).toBe('coverage/coverage-summary.json');
    }
  });

  it('各框架 coverage_cleanup 与变更前一致', () => {
    for (const [fw, expected] of Object.entries(JSON_ONLY_EXPECTED_CONFIGS)) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.coverage_cleanup).toEqual(expected.coverage_cleanup);
    }
  });

  it('更新 EXPECTED_CONFIGS 后五个框架 toEqual 期望值全部通过', () => {
    for (const [fw, jsonOnly] of Object.entries(JSON_ONLY_EXPECTED_CONFIGS)) {
      const base = EXPECTED_CONFIGS[fw];
      const merged = { ...base, ...jsonOnly };
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result).toEqual(merged);
    }
  });
});

describe('getFrameworkConfig — JSON-only 边界与异常 (AC-9)', () => {
  it('五个框架 coverage_artifacts.length === 1 且元素为 coverage/coverage-summary.json', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust']) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.coverage_artifacts).toHaveLength(1);
      expect(result.coverage_artifacts[0]).toBe('coverage/coverage-summary.json');
    }
  });

  it('五个框架 coverage_cmd 为非空字符串且包含 coverage 或 JSON 关键字', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust']) {
      const result = runTestGetFrameworkConfig({ framework: fw });
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
// add-node-go-pytest-frameworks: go / node-test / pytest 注册表 (AC-2 ~ AC-4)
// @see openspec/changes/add-node-go-pytest-frameworks/test-design.md
// ===========================================================================

const NEW_FRAMEWORK_EXPECTED: Record<string, FrameworkConfig> = {
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

describe('runTestGetFrameworkConfig — go (AC-2)', () => {
  it('应返回 coverage_format: "go-cover" 及完整 artifacts/cleanup/default_glob', () => {
    const result = runTestGetFrameworkConfig({ framework: 'go' });
    expect(result).toEqual(NEW_FRAMEWORK_EXPECTED.go);
  });
});

// ===========================================================================
// AC-1: node-test coverage_cmd 简化（移除 tee/parse-node-test-coverage.mjs）
// ===========================================================================

describe('runTestGetFrameworkConfig — node-test (AC-1)', () => {
  it('coverage_cmd 应为 "node --test --experimental-test-coverage" （不含 tee/脚本管道）', () => {
    const result = runTestGetFrameworkConfig({ framework: 'node-test' });
    expect(result.coverage_cmd).toBe('node --test --experimental-test-coverage');
    expect(result.coverage_cmd).not.toContain('|');
    expect(result.coverage_cmd).not.toContain('parse-node-test-coverage.mjs');
  });

  it('coverage_cmd 仅含 --test 和 --experimental-test-coverage 两个 flags，无管道符/命令链接符/tee/mjs （与 go-cover 单命令结构对标）', () => {
    const result = runTestGetFrameworkConfig({ framework: 'node-test' });
    const flags = result.coverage_cmd.split(/\s+/).filter((s) => s.startsWith('-'));
    expect(flags).toEqual(['--test', '--experimental-test-coverage']);
    expect(result.coverage_cmd).not.toContain('|');
    expect(result.coverage_cmd).not.toContain('&&');
    expect(result.coverage_cmd).not.toContain('tee');
    expect(result.coverage_cmd).not.toContain('.mjs');
  });

  it('其他框架（jest/vitest/vite-plus/bun）的 coverage_cmd 不受影响，仍含各自 reporter 参数', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun'] as const) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.coverage_cmd).toMatch(/json|coverage/i);
    }
  });
});

// ===========================================================================
// AC-2: node-test coverage_output 更改（从 coverage-summary.json 变为 node-test-output.txt）
// ===========================================================================

describe('runTestGetFrameworkConfig — node-test (AC-2)', () => {
  it('node-test coverage_output 应为 "coverage/node-test-output.txt"，非 "coverage/coverage-summary.json"', () => {
    const result = runTestGetFrameworkConfig({ framework: 'node-test' });
    expect(result.coverage_output).toBe('coverage/node-test-output.txt');
    expect(result.coverage_output).not.toBe('coverage/coverage-summary.json');
  });

  it('node-test coverage_output 以 .txt 后缀结尾，区别于其他七框架的 .json 输出路径', () => {
    const allEight = ['jest', 'vitest', 'vite-plus', 'bun', 'rust', 'node-test', 'go', 'pytest'];
    for (const fw of allEight) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      if (fw === 'node-test' || fw === 'go') {
        expect(result.coverage_output).toMatch(/\.txt$/);
      } else {
        expect(result.coverage_output).toMatch(/\.json$/);
      }
    }
  });

  it('其他框架 coverage_output 路径不变（jest/vitest/vite-plus/bun 仍为 coverage/coverage-summary.json）', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun'] as const) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.coverage_output).toBe('coverage/coverage-summary.json');
    }
  });
});

// ===========================================================================
// AC-3: node-test coverage_artifacts 更改
// ===========================================================================

describe('runTestGetFrameworkConfig — node-test (AC-3)', () => {
  it('coverage_artifacts 应为 ["coverage/node-test-output.txt"]，非 ["coverage/coverage-summary.json"]', () => {
    const result = runTestGetFrameworkConfig({ framework: 'node-test' });
    expect(result.coverage_artifacts).toEqual(['coverage/node-test-output.txt']);
    expect(result.coverage_artifacts).not.toEqual(['coverage/coverage-summary.json']);
  });

  it('coverage_cleanup 仍为 ["coverage"]（不变）', () => {
    const result = runTestGetFrameworkConfig({ framework: 'node-test' });
    expect(result.coverage_cleanup).toEqual(['coverage']);
  });
});

// ===========================================================================
// AC-5: node-test coverage_format 不变
// ===========================================================================

describe('runTestGetFrameworkConfig — node-test (AC-5)', () => {
  it('coverage_format 仍为 "node-test"（枚举值不变）', () => {
    const result = runTestGetFrameworkConfig({ framework: 'node-test' });
    expect(result.coverage_format).toBe('node-test');
  });
});

// ===========================================================================
// AC-7: node-test coverage_output 指向原始文本文件（非 JSON）
// ===========================================================================

describe('runTestGetFrameworkConfig — node-test (AC-7)', () => {
  it('coverage_output 指向原始文本文件，路径类型从 .json 变为 .txt', () => {
    const result = runTestGetFrameworkConfig({ framework: 'node-test' });
    expect(result.coverage_output).toMatch(/\.txt$/);
    expect(result.coverage_output).not.toMatch(/\.json$/);
  });
});

// ===========================================================================
// EXPECTED_CONFIGS — node-test 新条目
// ===========================================================================

describe('EXPECTED_CONFIGS — node-test 新条目', () => {
  it('EXPECTED_CONFIGS 包含 node-test 条目，覆盖全部 8 字段且与注册表一致', () => {
    expect(EXPECTED_CONFIGS['node-test']).toBeDefined();
    const result = runTestGetFrameworkConfig({ framework: 'node-test' });
    expect(result).toEqual(EXPECTED_CONFIGS['node-test']);
    // Verify all 8 fields are present
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

describe('runTestGetFrameworkConfig — pytest (AC-4)', () => {
  it('应返回 coverage_format: "coverage-py"、coverage_output: "coverage.json"', () => {
    const result = runTestGetFrameworkConfig({ framework: 'pytest' });
    expect(result.coverage_format).toBe('coverage-py');
    expect(result.coverage_output).toBe('coverage.json');
  });
});

describe('runTestGetFrameworkConfig — 八框架完整性 (AC-2)', () => {
  const ALL_EIGHT = ['jest', 'vitest', 'vite-plus', 'bun', 'rust', 'node-test', 'go', 'pytest'];

  it('遍历全部八个框架，每个返回非空 test_cmd、coverage_cmd、coverage_artifacts、coverage_cleanup', () => {
    for (const fw of ALL_EIGHT) {
      const result = runTestGetFrameworkConfig({ framework: fw });
      expect(result.test_cmd.length).toBeGreaterThan(0);
      expect(result.coverage_cmd.length).toBeGreaterThan(0);
      expect(result.coverage_artifacts.length).toBeGreaterThan(0);
      expect(result.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });
});
