/**
 * Tests for lib/test-framework -- internal module providing the hardcoded
 * test framework registry and query functions.
 */

import { describe, it, expect } from 'vite-plus/test';

import { getFrameworkConfig } from './test-framework';

// ---------------------------------------------------------------------------
// Expected configs per spec (v3: merge_mode removed, coverage_cmd removed,
// chained commands in test_cmd, shell/cmd nested for platform split)
// ---------------------------------------------------------------------------

interface FrameworkConfig {
  framework: string;
  shell: {
    test_execution: string;
    coverage_cleanup: string[];
  };
  cmd: {
    test_execution: string;
    coverage_cleanup: string[];
  };
  coverage_format: string;
  coverage_output: string;
  coverage_artifacts: string[];
  default_glob: string;
  mutation_framework: string | null;
}

const ALL_EIGHT = ['jest', 'vitest', 'vite-plus', 'bun', 'rust', 'node-test', 'go', 'pytest'];

// ===========================================================================
// Known frameworks
// ===========================================================================

describe('getFrameworkConfig -- known frameworks', () => {
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
    for (const fw of ALL_EIGHT) {
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
    expect(() => getFrameworkConfig('mocha')).toThrow(new RegExp(ALL_EIGHT.join('|')));
  });
});

// ===========================================================================
// Framework-specific coverage_artifacts and coverage_cleanup
// ===========================================================================

describe('getFrameworkConfig -- coverage_artifacts and coverage_cleanup', () => {
  it('vitest should return coverage_artifacts and coverage_cleanup correctly', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.shell.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('rust should return single-element JSON summary path for coverage_artifacts', () => {
    const result = getFrameworkConfig('rust');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_artifacts).not.toContain('target/llvm-cov/**');
    expect(result.coverage_artifacts).toHaveLength(1);
  });

  it('rust should return coverage_cleanup containing coverage and target/llvm-cov', () => {
    const result = getFrameworkConfig('rust');
    expect(result.shell.coverage_cleanup).toContain('coverage');
    expect(result.shell.coverage_cleanup).toContain('target/llvm-cov');
    expect(result.shell.coverage_cleanup).toHaveLength(2);
  });

  it('jest should return coverage_artifacts and coverage_cleanup fields', () => {
    const result = getFrameworkConfig('jest');
    expect(result.coverage_artifacts).toBeDefined();
    expect(result.shell.coverage_cleanup).toBeDefined();
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.shell.coverage_cleanup).toEqual(['coverage', '.nyc_output']);
  });

  it('vite-plus should return coverage_artifacts and coverage_cleanup fields', () => {
    const result = getFrameworkConfig('vite-plus');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.shell.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('bun should return coverage_cleanup without .nyc_output (only coverage)', () => {
    const result = getFrameworkConfig('bun');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.shell.coverage_cleanup).toEqual(['coverage']);
  });
});

// ===========================================================================
// All frameworks coverage fields non-empty
// ===========================================================================

describe('getFrameworkConfig -- all frameworks coverage fields non-empty', () => {
  it('each framework should have non-empty coverage_artifacts array', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.coverage_artifacts)).toBe(true);
      expect(result.coverage_artifacts.length).toBeGreaterThan(0);
    }
  });

  it('each framework should have non-empty shell.coverage_cleanup array', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.shell.coverage_cleanup)).toBe(true);
      expect(result.shell.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });

  it('each framework should return all 8 fields (shell, cmd, no merge_mode, no coverage_cmd)', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      const keys = Object.keys(result);
      expect(keys).toContain('coverage_artifacts');
      expect(keys).toContain('shell');
      expect(keys).toContain('cmd');
      expect(keys).not.toContain('merge_mode');
      expect(keys).not.toContain('coverage_cmd');
      expect(keys).not.toContain('test_cmd');
      expect(keys).not.toContain('coverage_cleanup');
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
// AC-2: pytest 链式命令
// ===========================================================================

describe('getFrameworkConfig -- pytest 链式命令 (AC-2)', () => {
  it('pytest shell.test_execution 应包含 `; _X=$?;` 链式分隔符', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.shell.test_execution).toContain('; _X=$?;');
  });

  it('pytest shell.test_execution 应以 `exit $_X` 结尾', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.shell.test_execution.endsWith('exit $_X')).toBe(true);
  });

  it('pytest shell.test_execution 应包含覆盖率命令 pytest --cov=', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.shell.test_execution).toContain('pytest --cov=.');
  });

  it('pytest shell.test_execution 链式命令顺序：测试命令 -> `; _X=$?;` -> 覆盖率命令 -> `; exit $_X`', () => {
    const result = getFrameworkConfig('pytest');
    const parts = result.shell.test_execution.split(';');
    expect(parts[0].trim()).toMatch(/^pytest -v/);
    expect(parts[1].trim()).toMatch(/^_X=\$[?]/);
    expect(parts[2].trim()).toMatch(/^pytest --cov=/);
    const lastPart = parts[parts.length - 1].trim();
    expect(lastPart).toBe('exit $_X');
  });

  it('pytest shell.test_execution 应包含 `{files}` 占位符', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.shell.test_execution).toContain('{files}');
  });
});

// ===========================================================================
// AC-2: rust 链式命令
// ===========================================================================

describe('getFrameworkConfig -- rust 链式命令 (AC-2)', () => {
  it('rust shell.test_execution 应包含 `; _X=$?;` 链式分隔符', () => {
    const result = getFrameworkConfig('rust');
    expect(result.shell.test_execution).toContain('; _X=$?;');
  });

  it('rust shell.test_execution 应以 `exit $_X` 结尾', () => {
    const result = getFrameworkConfig('rust');
    expect(result.shell.test_execution.endsWith('exit $_X')).toBe(true);
  });

  it('rust shell.test_execution 应包含 `cargo llvm-cov` 覆盖率命令', () => {
    const result = getFrameworkConfig('rust');
    expect(result.shell.test_execution).toContain('cargo llvm-cov');
  });

  it('rust shell.test_execution 应包含 `--output-path coverage/coverage-summary.json`', () => {
    const result = getFrameworkConfig('rust');
    expect(result.shell.test_execution).toContain('--output-path coverage/coverage-summary.json');
  });

  it('rust shell.test_execution 链式命令顺序：测试命令 -> `; _X=$?;` -> 覆盖率命令 -> `; exit $_X`', () => {
    const result = getFrameworkConfig('rust');
    const parts = result.shell.test_execution.split(';');
    expect(parts[0].trim()).toMatch(/^cargo test/);
    expect(parts[1].trim()).toMatch(/^_X=\$[?]/);
    expect(parts[2].trim()).toMatch(/^cargo llvm-cov/);
    const lastPart = parts[parts.length - 1].trim();
    expect(lastPart).toBe('exit $_X');
  });

  it('rust shell.test_execution "cargo test" 不含 `{files}`/`{directory}` 占位符（链式命令的前半部分）', () => {
    const result = getFrameworkConfig('rust');
    expect(result.shell.test_execution).not.toContain('{files}');
    expect(result.shell.test_execution).not.toContain('{directory}');
  });
});

// ===========================================================================
// AC-2: 非链式框架不应包含链式命令模式
// ===========================================================================

describe('getFrameworkConfig -- 非链式框架 test_execution 不含链式模式', () => {
  const SINGLE_CMD_FRAMEWORKS = ['jest', 'vitest', 'vite-plus', 'bun', 'node-test', 'go'];

  it('非链式框架（jest/vitest/vite-plus/bun/node-test/go）test_execution 不含 `; _X=$?`', () => {
    for (const fw of SINGLE_CMD_FRAMEWORKS) {
      const result = getFrameworkConfig(fw);
      expect(result.shell.test_execution).not.toContain('; _X=$?');
    }
  });

  it('非链式框架 test_execution 不含 `exit $_X` 模式', () => {
    for (const fw of SINGLE_CMD_FRAMEWORKS) {
      const result = getFrameworkConfig(fw);
      expect(result.shell.test_execution).not.toContain('exit $_X');
    }
  });
});

// ===========================================================================
// AC-2: 链式命令边界和异常
// ===========================================================================

describe('getFrameworkConfig -- 链式命令边界和异常', () => {
  it('pytest shell.test_execution 链式命令各部分顺序正确：测试命令 -> `; _X=$?;` -> 覆盖率命令 -> `; exit $_X`', () => {
    const result = getFrameworkConfig('pytest');
    const match = result.shell.test_execution.match(
      /^pytest -v .+?; _X=\$.; pytest --cov=..+?; exit \$_X$/,
    );
    expect(match).toBeTruthy();
  });

  it('pytest shell.test_execution 链式语法格式正确（包含 `;` 分隔符模式 `; _X=$?;`）', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.shell.test_execution).toMatch(/; _X=\$.;/);
    expect(result.shell.test_execution).toMatch(/; exit \$_X$/);
  });

  it('rust shell.test_execution 链式命令包含 `exit $_X`', () => {
    const result = getFrameworkConfig('rust');
    expect(result.shell.test_execution).toMatch(/; exit \$_X$/);
  });

  it('pytest shell.test_execution 链式命令含有覆盖率后处理步骤时仍保留核心链式结构', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.shell.test_execution).toContain('; _X=$?;');
    expect(result.shell.test_execution).toContain('exit $_X');
    expect(result.shell.test_execution).toContain('pytest --cov=');
    expect(result.shell.test_execution).toContain('pytest -v');
  });
});

// ===========================================================================
// AC-12: FrameworkConfig 无 merge_mode 字段
// ===========================================================================

describe('getFrameworkConfig -- 无 merge_mode (AC-12)', () => {
  it('FrameworkConfig 对象不含 merge_mode 字段', () => {
    const result = getFrameworkConfig('vitest');
    expect(result).not.toHaveProperty('merge_mode');
  });

  it('所有八个框架返回对象均不含 merge_mode', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(result).not.toHaveProperty('merge_mode');
    }
  });

  it('FrameworkConfig 接口字段数量为 8（含 shell/cmd，不含 merge_mode/coverage_cmd/test_cmd/coverage_cleanup）', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      const keys = Object.keys(result);
      expect(keys.length).toBe(8);
      expect(keys).not.toContain('merge_mode');
      expect(keys).not.toContain('coverage_cmd');
      expect(keys).not.toContain('test_cmd');
      expect(keys).not.toContain('coverage_cleanup');
    }
  });

  it('FrameworkConfig 无 merge_mode/coverage_cmd 属性时编译通过（纯类型检查，运行时验证字段数量）', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      const keys = Object.keys(result);
      expect(keys).toEqual(
        expect.arrayContaining([
          'framework',
          'shell',
          'cmd',
          'coverage_format',
          'coverage_output',
          'coverage_artifacts',
          'default_glob',
          'mutation_framework',
        ]),
      );
      expect(keys).toHaveLength(8);
    }
  });
});

// ===========================================================================
// AC-3: test_execution 模板化
// ===========================================================================

describe('getFrameworkConfig -- test_execution 模板化 (AC-3)', () => {
  it('vitest shell.test_execution 含 `--reporter=json` 和 `{files}` 占位符', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.shell.test_execution).toContain('--reporter=json');
    expect(result.shell.test_execution).toContain('{files}');
  });

  it('go shell.test_execution 含 `-json` 和 `{directory}` 占位符', () => {
    const result = getFrameworkConfig('go');
    expect(result.shell.test_execution).toContain('-json');
    expect(result.shell.test_execution).toContain('{directory}');
  });

  it('所有八框架的 shell.test_execution 类型为 string 且非空', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(typeof result.shell.test_execution).toBe('string');
      expect(result.shell.test_execution.length).toBeGreaterThan(0);
    }
  });

  it('pytest shell.test_execution 为链式命令含 `{files}` 占位符', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.shell.test_execution).toContain('{files}');
    expect(result.shell.test_execution).toContain('; _X=$?;');
    expect(result.shell.test_execution).toContain('exit $_X');
  });

  it('jest shell.test_execution 含 `--json` 和 `{files}` 占位符', () => {
    const result = getFrameworkConfig('jest');
    expect(result.shell.test_execution).toContain('--json');
    expect(result.shell.test_execution).toContain('{files}');
  });

  it('bun shell.test_execution 含 `--coverage` 和 `{files}` 占位符', () => {
    const result = getFrameworkConfig('bun');
    expect(result.shell.test_execution).toContain('--coverage');
    expect(result.shell.test_execution).toContain('{files}');
  });

  it('node-test shell.test_execution 含 `--experimental-test-coverage` 和 `{files}` 占位符', () => {
    const result = getFrameworkConfig('node-test');
    expect(result.shell.test_execution).toContain('--experimental-test-coverage');
    expect(result.shell.test_execution).toContain('{files}');
  });

  it('单命令框架（jest/vitest/vite-plus/bun/node-test/go）shell.test_execution 同时含 `{files}` 或 `{directory}` 占位符', () => {
    const SINGLE_CMD_FRAMEWORKS: Record<string, string> = {
      jest: '{files}',
      vitest: '{files}',
      'vite-plus': '{files}',
      bun: '{files}',
      'node-test': '{files}',
      go: '{directory}',
    };
    for (const [fw, placeholder] of Object.entries(SINGLE_CMD_FRAMEWORKS)) {
      const result = getFrameworkConfig(fw);
      expect(result.shell.test_execution).toContain(placeholder);
    }
  });

  it('shell.test_execution 含多个占位符时模板字符串格式正确', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.shell.test_execution).toContain('{files}');
  });
});

describe('getFrameworkConfig -- JSON-only coverage artifact config', () => {
  it('vitest should return coverage_artifacts with single JSON summary', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('vite-plus should return coverage_artifacts with single JSON summary', () => {
    const result = getFrameworkConfig('vite-plus');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('bun should return coverage_artifacts with single JSON summary', () => {
    const result = getFrameworkConfig('bun');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
  });

  it('rust should return coverage_artifacts not containing target/llvm-cov/**', () => {
    const result = getFrameworkConfig('rust');
    expect(result.coverage_artifacts).toEqual(['coverage/coverage-summary.json']);
    expect(result.coverage_artifacts).not.toContain('target/llvm-cov/**');
  });

  it('five JS/TS frameworks should have single-element JSON artifacts', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust'] as const) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_artifacts).toHaveLength(1);
      expect(result.coverage_artifacts[0]).toBe('coverage/coverage-summary.json');
    }
  });
});

describe('getFrameworkConfig -- coverage artifact boundary and exception', () => {
  it('five frameworks should have coverage_artifacts.length === 1 with value coverage/coverage-summary.json', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust']) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_artifacts).toHaveLength(1);
      expect(result.coverage_artifacts[0]).toBe('coverage/coverage-summary.json');
    }
  });

  it('five frameworks should have non-empty coverage_artifacts containing coverage keyword', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust']) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.coverage_artifacts)).toBe(true);
      expect(result.coverage_artifacts.length).toBeGreaterThan(0);
      for (const artifact of result.coverage_artifacts) {
        expect(artifact.toLowerCase()).toMatch(/coverage/);
      }
    }
  });
});

// ===========================================================================
// go / node-test / pytest framework entries
// ===========================================================================

const FRAMEWORK_SPEC_EXPECTED: Record<string, FrameworkConfig> = {
  go: {
    framework: 'go',
    shell: {
      test_execution: 'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
      coverage_cleanup: ['coverage', 'coverage.out'],
    },
    cmd: {
      test_execution: 'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
      coverage_cleanup: ['coverage', 'coverage.out'],
    },
    coverage_format: 'go-cover',
    coverage_output: 'coverage/func-summary.txt',
    coverage_artifacts: ['coverage/func-summary.txt'],
    default_glob: '**/*_test.go',
    mutation_framework: null,
  },
  'node-test': {
    framework: 'node-test',
    shell: {
      test_execution: 'node --test --experimental-test-coverage {files}',
      coverage_cleanup: ['coverage'],
    },
    cmd: {
      test_execution: 'node --test --experimental-test-coverage {files}',
      coverage_cleanup: ['coverage'],
    },
    coverage_format: 'node-test',
    coverage_output: 'coverage/node-test-output.txt',
    coverage_artifacts: ['coverage/node-test-output.txt'],
    default_glob: '**/*.test.{mjs,js,cjs}',
    mutation_framework: null,
  },
  pytest: {
    framework: 'pytest',
    shell: {
      test_execution:
        'pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X',
      coverage_cleanup: ['.coverage', 'htmlcov'],
    },
    cmd: {
      test_execution:
        'pytest -v {files}\nif errorlevel 1 set _X=%errorlevel%\npytest --cov=. --cov-report=json --cov-branch -q\nexit /b %_X%',
      coverage_cleanup: ['.coverage', 'htmlcov'],
    },
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    coverage_artifacts: ['coverage.json'],
    default_glob: '**/test_*.py',
    mutation_framework: null,
  },
};

describe('getFrameworkConfig -- go', () => {
  it('should return coverage_format: "go-cover" with complete artifacts/cleanup/default_glob', () => {
    const result = getFrameworkConfig('go');
    expect(result).toEqual(FRAMEWORK_SPEC_EXPECTED.go);
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
    for (const fw of ALL_EIGHT) {
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
    expect(result.shell.coverage_cleanup).toEqual(['coverage']);
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
  it('each of the eight frameworks should return non-empty shell.test_execution, coverage_artifacts, shell.coverage_cleanup', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(result.shell.test_execution.length).toBeGreaterThan(0);
      expect(result.coverage_artifacts.length).toBeGreaterThan(0);
      expect(result.shell.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });
});

// ===========================================================================
// mutation_framework 字段 (AC-2)
// ===========================================================================

describe('getFrameworkConfig -- mutation_framework 字段', () => {
  it('jest 的 mutation_framework 为 "stryker-js"', () => {
    const result = getFrameworkConfig('jest');
    expect(result.mutation_framework).toBe('stryker-js');
  });

  it('vitest 的 mutation_framework 为 "stryker-js"', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.mutation_framework).toBe('stryker-js');
  });

  it('vite-plus 的 mutation_framework 为 "stryker-js"', () => {
    const result = getFrameworkConfig('vite-plus');
    expect(result.mutation_framework).toBe('stryker-js');
  });

  it('bun 的 mutation_framework 为 null', () => {
    const result = getFrameworkConfig('bun');
    expect(result.mutation_framework).toBeNull();
  });

  it('node-test 的 mutation_framework 为 null', () => {
    const result = getFrameworkConfig('node-test');
    expect(result.mutation_framework).toBeNull();
  });

  it('go 的 mutation_framework 为 null', () => {
    const result = getFrameworkConfig('go');
    expect(result.mutation_framework).toBeNull();
  });

  it('rust 的 mutation_framework 为 null', () => {
    const result = getFrameworkConfig('rust');
    expect(result.mutation_framework).toBeNull();
  });

  it('pytest 的 mutation_framework 为 null', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.mutation_framework).toBeNull();
  });

  it('FrameworkConfig 字段数量为 8（含 shell/cmd/mutation_framework，不含 coverage_cmd/merge_mode）', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      const keys = Object.keys(result);
      expect(keys).toContain('mutation_framework');
      expect(keys).toContain('shell');
      expect(keys).toContain('cmd');
      expect(keys).not.toContain('coverage_cmd');
      expect(keys).not.toContain('merge_mode');
      expect(keys).not.toContain('test_cmd');
      expect(keys).not.toContain('coverage_cleanup');
      expect(keys.length).toBe(8);
    }
  });
});

// ===========================================================================
// AC-1: shell/cmd 二级嵌套结构
// ===========================================================================

describe('getFrameworkConfig -- shell/cmd 二级嵌套结构 (AC-1)', () => {
  it('所有 8 框架的 shell.test_execution 为非空字符串', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(typeof result.shell.test_execution).toBe('string');
      expect(result.shell.test_execution.length).toBeGreaterThan(0);
    }
  });

  it('所有 8 框架的 shell.coverage_cleanup 为非空数组', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.shell.coverage_cleanup)).toBe(true);
      expect(result.shell.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });

  it('所有 8 框架的 cmd.test_execution 为非空字符串', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(typeof result.cmd.test_execution).toBe('string');
      expect(result.cmd.test_execution.length).toBeGreaterThan(0);
    }
  });

  it('所有 8 框架的 cmd.coverage_cleanup 为非空数组', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.cmd.coverage_cleanup)).toBe(true);
      expect(result.cmd.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });

  it('简单框架（jest/vitest/vite-plus/bun/node-test/go）的 cmd.test_execution 与 shell.test_execution 相同', () => {
    const SIMPLE_FRAMEWORKS = ['jest', 'vitest', 'vite-plus', 'bun', 'node-test', 'go'];
    for (const fw of SIMPLE_FRAMEWORKS) {
      const result = getFrameworkConfig(fw);
      expect(result.cmd.test_execution).toBe(result.shell.test_execution);
    }
  });

  it('rust 框架 cmd.test_execution 使用 `if errorlevel` 模式代替 `; _X=$?;`', () => {
    const result = getFrameworkConfig('rust');
    expect(result.cmd.test_execution).not.toContain('; _X=$?;');
    expect(result.cmd.test_execution).toContain('if errorlevel');
    expect(result.cmd.test_execution).toContain('%errorlevel%');
    expect(result.cmd.test_execution).toContain('exit /b');
  });

  it('pytest 框架 cmd.test_execution 使用 `if errorlevel` 模式代替 `; _X=$?;`', () => {
    const result = getFrameworkConfig('pytest');
    expect(result.cmd.test_execution).not.toContain('; _X=$?;');
    expect(result.cmd.test_execution).toContain('if errorlevel');
    expect(result.cmd.test_execution).toContain('%errorlevel%');
    expect(result.cmd.test_execution).toContain('exit /b');
  });

  it('共享字段保持在 FrameworkConfig 顶层', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(result).toHaveProperty('coverage_format');
      expect(result).toHaveProperty('coverage_output');
      expect(result).toHaveProperty('coverage_artifacts');
      expect(result).toHaveProperty('default_glob');
      expect(result).toHaveProperty('mutation_framework');
      expect(result.shell).not.toHaveProperty('coverage_format');
      expect(result.cmd).not.toHaveProperty('coverage_format');
    }
  });

  it('shell.coverage_cleanup 与 cmd.coverage_cleanup 路径名相同', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(result.shell.coverage_cleanup).toEqual(result.cmd.coverage_cleanup);
    }
  });

  it('有 mutation 框架（jest/vitest/vite-plus）的 shell.mutation_execution 为 `npx stryker run "{config}"`', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus']) {
      const result = getFrameworkConfig(fw);
      expect(result.shell.mutation_execution).toBe('npx stryker run "{config}"');
    }
  });

  it('有 mutation 框架的 cmd.mutation_execution 与 shell 相同', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus']) {
      const result = getFrameworkConfig(fw);
      expect(result.cmd.mutation_execution).toBe('npx stryker run "{config}"');
      expect(result.cmd.mutation_execution).toBe(result.shell.mutation_execution);
    }
  });

  it('无 mutation 框架的 shell.mutation_execution 和 cmd.mutation_execution 均为 undefined', () => {
    const NO_MUTATION = ['bun', 'node-test', 'go', 'rust', 'pytest'];
    for (const fw of NO_MUTATION) {
      const result = getFrameworkConfig(fw);
      expect(result.shell.mutation_execution).toBeUndefined();
      expect(result.cmd.mutation_execution).toBeUndefined();
    }
  });

  it('所有 8 框架的顶层不包含旧字段 test_cmd', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(result).not.toHaveProperty('test_cmd');
    }
  });

  it('所有 8 框架的顶层不包含旧字段 coverage_cleanup', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(result).not.toHaveProperty('coverage_cleanup');
    }
  });
});
