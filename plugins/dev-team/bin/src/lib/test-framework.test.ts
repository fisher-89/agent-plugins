/**
 * Tests for lib/test-framework -- internal module providing the hardcoded
 * test framework registry and query functions.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import * as testFramework from './test-framework';
import { detectFrameworkVersion, type FrameworkConfig, getFrameworkConfig } from './test-framework';

// ---------------------------------------------------------------------------
// Expected configs per spec (v3: merge_mode removed, coverage_cmd removed,
// chained commands in test_cmd, shell/cmd nested for platform split)
// ---------------------------------------------------------------------------

type TestExecutionBuilder = FrameworkConfig['shell']['test_execution'];
/** Resolve builder at a high version so gated flags are included. */
function te(builder: TestExecutionBuilder, version = '99.0.0'): string {
  return builder(version);
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
// Framework-specific coverage_cleanup
// ===========================================================================

describe('getFrameworkConfig -- coverage_cleanup', () => {
  it('vitest should return coverage_cleanup correctly', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.shell.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('rust should return coverage_cleanup containing coverage and target/llvm-cov', () => {
    const result = getFrameworkConfig('rust');
    expect(result.shell.coverage_cleanup).toContain('coverage');
    expect(result.shell.coverage_cleanup).toContain('target/llvm-cov');
    expect(result.shell.coverage_cleanup).toHaveLength(2);
  });

  it('jest should return coverage_cleanup fields', () => {
    const result = getFrameworkConfig('jest');
    expect(result.shell.coverage_cleanup).toBeDefined();
    expect(result.shell.coverage_cleanup).toEqual(['coverage', '.nyc_output']);
  });

  it('vite-plus should return coverage_cleanup fields', () => {
    const result = getFrameworkConfig('vite-plus');
    expect(result.shell.coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
  });

  it('bun should return coverage_cleanup without .nyc_output (only coverage)', () => {
    const result = getFrameworkConfig('bun');
    expect(result.shell.coverage_cleanup).toEqual(['coverage']);
  });
});

// ===========================================================================
// All frameworks coverage fields non-empty
// ===========================================================================

describe('getFrameworkConfig -- all frameworks coverage fields non-empty', () => {
  it('each framework should have non-empty coverage_output', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(typeof result.coverage_output).toBe('string');
      expect(result.coverage_output.length).toBeGreaterThan(0);
    }
  });

  it('each framework should have non-empty shell.coverage_cleanup array', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.shell.coverage_cleanup)).toBe(true);
      expect(result.shell.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });

  it('each framework should return all top-level fields (shell, cmd, no merge_mode, no coverage_cmd, no coverage_artifacts)', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      const keys = Object.keys(result);
      expect(keys).toContain('coverage_output');
      expect(keys).toContain('shell');
      expect(keys).toContain('cmd');
      expect(keys).not.toContain('merge_mode');
      expect(keys).not.toContain('coverage_cmd');
      expect(keys).not.toContain('coverage_artifacts');
      expect(keys).not.toContain('test_cmd');
      expect(keys).not.toContain('coverage_cleanup');
      expect(keys).toContain('config_flag');
      expect(keys).toContain('version_command');
      expect(keys.length).toBe(9);
    }
  });
});

// ===========================================================================
// AC-2: pytest 命令
// ===========================================================================

describe('getFrameworkConfig -- pytest 命令 (AC-2)', () => {
  it('pytest 框架 cmd.test_execution 包含测试和覆盖语句', () => {
    const result = getFrameworkConfig('pytest');
    expect(te(result.shell.test_execution)).toContain('pytest -v {files}');
    expect(te(result.cmd.test_execution)).toContain('pytest --cov=');
  });
});

// ===========================================================================
// AC-2: rust 命令
// ===========================================================================

describe('getFrameworkConfig -- rust 链式命令 (AC-2)', () => {
  it('rust shell.test_execution 应包含 `; _X=$?;` 链式分隔符', () => {
    const result = getFrameworkConfig('rust');
    expect(te(result.shell.test_execution)).toContain('; _X=$?;');
  });

  it('rust shell.test_execution 应以 `exit $_X` 结尾', () => {
    const result = getFrameworkConfig('rust');
    expect(te(result.shell.test_execution).endsWith('exit $_X')).toBe(true);
  });

  it('rust shell.test_execution 应包含 `cargo llvm-cov` 覆盖率命令', () => {
    const result = getFrameworkConfig('rust');
    expect(te(result.shell.test_execution)).toContain('cargo llvm-cov');
  });

  it('rust shell.test_execution 应包含 `--output-path coverage/coverage-summary.json`', () => {
    const result = getFrameworkConfig('rust');
    expect(te(result.shell.test_execution)).toContain(
      '--output-path coverage/coverage-summary.json',
    );
  });

  it('rust shell.test_execution 链式命令顺序：测试命令 -> `; _X=$?;` -> 覆盖率命令 -> `; exit $_X`', () => {
    const result = getFrameworkConfig('rust');
    const parts = te(result.shell.test_execution).split(';');
    expect(parts[0].trim()).toMatch(/^cargo test/);
    expect(parts[1].trim()).toMatch(/^_X=\$[?]/);
    expect(parts[2].trim()).toMatch(/^cargo llvm-cov/);
    const lastPart = parts[parts.length - 1].trim();
    expect(lastPart).toBe('exit $_X');
  });

  it('rust shell.test_execution "cargo test" 不含 `{files}`/`{directory}` 占位符（链式命令的前半部分）', () => {
    const result = getFrameworkConfig('rust');
    expect(te(result.shell.test_execution)).not.toContain('{files}');
    expect(te(result.shell.test_execution)).not.toContain('{directory}');
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
      expect(te(result.shell.test_execution)).not.toContain('; _X=$?');
    }
  });

  it('非链式框架 test_execution 不含 `exit $_X` 模式', () => {
    for (const fw of SINGLE_CMD_FRAMEWORKS) {
      const result = getFrameworkConfig(fw);
      expect(te(result.shell.test_execution)).not.toContain('exit $_X');
    }
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

  it('FrameworkConfig 接口字段数量为 9（含 shell/cmd/config_flag/version_command，不含 merge_mode/coverage_cmd/coverage_artifacts/test_cmd/coverage_cleanup）', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      const keys = Object.keys(result);
      expect(keys.length).toBe(9);
      expect(keys).not.toContain('merge_mode');
      expect(keys).not.toContain('coverage_cmd');
      expect(keys).not.toContain('coverage_artifacts');
      expect(keys).not.toContain('test_cmd');
      expect(keys).not.toContain('coverage_cleanup');
      expect(keys).toContain('config_flag');
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
          'default_glob',
          'mutation_framework',
          'config_flag',
          'version_command',
        ]),
      );
      expect(keys).not.toContain('coverage_artifacts');
      expect(keys).toHaveLength(9);
    }
  });
});

// ===========================================================================
// AC-3: test_execution 模板化
// ===========================================================================

describe('getFrameworkConfig -- test_execution 模板化 (AC-3)', () => {
  it('vitest shell.test_execution 含 `--reporter=json` 和 `{files}` 占位符', () => {
    const result = getFrameworkConfig('vitest');
    expect(te(result.shell.test_execution)).toContain('--reporter=json');
    expect(te(result.shell.test_execution)).toContain('{files}');
  });

  it('go shell.test_execution 含 `-json` 和 `{directory}` 占位符', () => {
    const result = getFrameworkConfig('go');
    expect(te(result.shell.test_execution)).toContain('-json');
    expect(te(result.shell.test_execution)).toContain('{directory}');
  });

  it('所有八框架的 shell.test_execution 类型为 function 且返回非空字符串', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(typeof result.shell.test_execution).toBe('function');
      expect(te(result.shell.test_execution).length).toBeGreaterThan(0);
    }
  });

  it('jest shell.test_execution 含 `--json` 和 `{files}` 占位符', () => {
    const result = getFrameworkConfig('jest');
    expect(te(result.shell.test_execution)).toContain('--json');
    expect(te(result.shell.test_execution)).toContain('{files}');
  });

  it('bun shell.test_execution 含 `--coverage` 和 `{files}` 占位符', () => {
    const result = getFrameworkConfig('bun');
    expect(te(result.shell.test_execution)).toContain('--coverage');
    expect(te(result.shell.test_execution)).toContain('{files}');
  });

  it('node-test shell.test_execution 含 `--experimental-test-coverage` 和 `{files}` 占位符', () => {
    const result = getFrameworkConfig('node-test');
    expect(te(result.shell.test_execution)).toContain('--experimental-test-coverage');
    expect(te(result.shell.test_execution)).toContain('{files}');
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
      expect(te(result.shell.test_execution)).toContain(placeholder);
    }
  });

  it('shell.test_execution 含多个占位符时模板字符串格式正确', () => {
    const result = getFrameworkConfig('vitest');
    expect(te(result.shell.test_execution)).toContain('{files}');
  });
});

describe('getFrameworkConfig -- JSON-only coverage_output', () => {
  it('five frameworks should use coverage/coverage-summary.json as coverage_output', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus', 'bun', 'rust'] as const) {
      const result = getFrameworkConfig(fw);
      expect(result.coverage_output).toBe('coverage/coverage-summary.json');
    }
  });
});

// ===========================================================================
// go / node-test / pytest framework entries
// ===========================================================================

const FRAMEWORK_SPEC_EXPECTED: Record<string, FrameworkConfig> = {
  go: {
    framework: 'go',
    version_command: 'go version',
    shell: {
      test_execution: (_version: string) =>
        'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
      coverage_cleanup: ['coverage', 'coverage.out'],
    },
    cmd: {
      test_execution: (_version: string) =>
        'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
      coverage_cleanup: ['coverage', 'coverage.out'],
    },
    coverage_format: 'go-cover',
    coverage_output: 'coverage/func-summary.txt',
    default_glob: '**/*_test.go',
    mutation_framework: null,
    config_flag: null,
  },
  'node-test': {
    framework: 'node-test',
    version_command: 'node --version',
    shell: {
      test_execution: (_version: string) => 'node --test --experimental-test-coverage {files}',
      coverage_cleanup: ['coverage'],
    },
    cmd: {
      test_execution: (_version: string) => 'node --test --experimental-test-coverage {files}',
      coverage_cleanup: ['coverage'],
    },
    coverage_format: 'node-test',
    coverage_output: 'coverage/node-test-output.txt',
    default_glob: '**/*.test.{mjs,js,cjs}',
    mutation_framework: null,
    config_flag: null,
  },
  pytest: {
    framework: 'pytest',
    version_command: 'pytest --version',
    shell: {
      test_execution: (_version: string) =>
        'pytest -v {files}; pytest --cov=. --cov-report=json --cov-branch -q',
      coverage_cleanup: ['.coverage', 'htmlcov'],
    },
    cmd: {
      test_execution: (_version: string) =>
        'pytest -v {files} && pytest --cov=. --cov-report=json --cov-branch -q',
      coverage_cleanup: ['.coverage', 'htmlcov'],
    },
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    default_glob: '**/test_*.py',
    mutation_framework: null,
    config_flag: null,
  },
};

describe('getFrameworkConfig -- go', () => {
  it('should return coverage_format: "go-cover" with complete artifacts/cleanup/default_glob', () => {
    const result = getFrameworkConfig('go');
    const expected = FRAMEWORK_SPEC_EXPECTED.go;
    expect(result.framework).toBe(expected.framework);
    expect(result.version_command).toBe(expected.version_command);
    expect(result.coverage_format).toBe(expected.coverage_format);
    expect(result.coverage_output).toBe(expected.coverage_output);
    expect(result.default_glob).toBe(expected.default_glob);
    expect(result.mutation_framework).toBe(expected.mutation_framework);
    expect(result.config_flag).toBe(expected.config_flag);
    expect(result.shell.coverage_cleanup).toEqual(expected.shell.coverage_cleanup);
    expect(result.cmd.coverage_cleanup).toEqual(expected.cmd.coverage_cleanup);
    expect(te(result.shell.test_execution)).toBe(te(expected.shell.test_execution));
    expect(te(result.cmd.test_execution)).toBe(te(expected.cmd.test_execution));
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
// node-test coverage_cleanup
// ===========================================================================

describe('getFrameworkConfig -- node-test (coverage_cleanup)', () => {
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
  it('each of the eight frameworks should return non-empty shell.test_execution, coverage_output, shell.coverage_cleanup', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(te(result.shell.test_execution).length).toBeGreaterThan(0);
      expect(result.coverage_output.length).toBeGreaterThan(0);
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

  it('FrameworkConfig 字段数量为 9（含 shell/cmd/mutation_framework/config_flag/version_command，不含 coverage_cmd/coverage_artifacts/merge_mode）', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      const keys = Object.keys(result);
      expect(keys).toContain('mutation_framework');
      expect(keys).toContain('config_flag');
      expect(keys).toContain('shell');
      expect(keys).toContain('cmd');
      expect(keys).not.toContain('coverage_cmd');
      expect(keys).not.toContain('coverage_artifacts');
      expect(keys).not.toContain('merge_mode');
      expect(keys).not.toContain('test_cmd');
      expect(keys).not.toContain('coverage_cleanup');
      expect(keys.length).toBe(9);
    }
  });
});

// ===========================================================================
// AC-1: shell/cmd 二级嵌套结构
// ===========================================================================

describe('getFrameworkConfig -- shell/cmd 二级嵌套结构 (AC-1)', () => {
  it('所有 8 框架的 shell.test_execution 为返回非空字符串的 function', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(typeof result.shell.test_execution).toBe('function');
      expect(te(result.shell.test_execution).length).toBeGreaterThan(0);
    }
  });

  it('所有 8 框架的 shell.coverage_cleanup 为非空数组', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(Array.isArray(result.shell.coverage_cleanup)).toBe(true);
      expect(result.shell.coverage_cleanup.length).toBeGreaterThan(0);
    }
  });

  it('所有 8 框架的 cmd.test_execution 为返回非空字符串的 function', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(typeof result.cmd.test_execution).toBe('function');
      expect(te(result.cmd.test_execution).length).toBeGreaterThan(0);
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
      expect(te(result.cmd.test_execution)).toBe(te(result.shell.test_execution));
    }
  });

  it('rust 框架 cmd.test_execution 使用 `if errorlevel` 模式代替 `; _X=$?;`', () => {
    const result = getFrameworkConfig('rust');
    expect(te(result.cmd.test_execution)).not.toContain('; _X=$?;');
    expect(te(result.cmd.test_execution)).toContain('if errorlevel');
    expect(te(result.cmd.test_execution)).toContain('%errorlevel%');
    expect(te(result.cmd.test_execution)).toContain('exit /b');
  });

  it('共享字段保持在 FrameworkConfig 顶层', () => {
    for (const fw of ALL_EIGHT) {
      const result = getFrameworkConfig(fw);
      expect(result).toHaveProperty('coverage_format');
      expect(result).toHaveProperty('coverage_output');
      expect(result).not.toHaveProperty('coverage_artifacts');
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

// ===========================================================================
// AC-3: config_flag 与 {config_args}
// ===========================================================================

describe('getFrameworkConfig — config_flag 与 {config_args} (AC-3)', () => {
  it('jest 的 config_flag 为 "--config" 且 shell/cmd test_execution 均含 {config_args}', () => {
    const result = getFrameworkConfig('jest');
    expect(result.config_flag).toBe('--config');
    expect(te(result.shell.test_execution)).toContain('{config_args}');
    expect(te(result.cmd.test_execution)).toContain('{config_args}');
  });

  it('vitest 的 config_flag 为 "--config" 且 shell/cmd test_execution 均含 {config_args}', () => {
    const result = getFrameworkConfig('vitest');
    expect(result.config_flag).toBe('--config');
    expect(te(result.shell.test_execution)).toContain('{config_args}');
    expect(te(result.cmd.test_execution)).toContain('{config_args}');
  });

  it('vite-plus 的 config_flag 为 "--config" 且 shell/cmd test_execution 均含 {config_args}', () => {
    const result = getFrameworkConfig('vite-plus');
    expect(result.config_flag).toBe('--config');
    expect(te(result.shell.test_execution)).toContain('{config_args}');
    expect(te(result.cmd.test_execution)).toContain('{config_args}');
  });

  it('pytest / rust / go / bun / node-test 的 config_flag 为 null', () => {
    for (const fw of ['pytest', 'rust', 'go', 'bun', 'node-test']) {
      expect(getFrameworkConfig(fw).config_flag).toBeNull();
    }
  });

  it('未知框架名（非法枚举）抛出 Error', () => {
    expect(() => getFrameworkConfig('mocha')).toThrow(/Unknown framework/);
  });

  it('framework 为 undefined/null 强转调用时抛错', () => {
    expect(() => getFrameworkConfig(null!)).toThrow();
    expect(() => getFrameworkConfig(undefined!)).toThrow();
  });

  it('framework 为空字符串时抛错', () => {
    expect(() => getFrameworkConfig('')).toThrow();
  });

  it('framework 为超长字符串（>1000 chars）时抛错', () => {
    expect(() => getFrameworkConfig('x'.repeat(1001))).toThrow();
  });

  it('framework 含特殊字符（\\n / emoji）时抛错', () => {
    expect(() => getFrameworkConfig('vit\nest')).toThrow();
    expect(() => getFrameworkConfig('vitest🚀')).toThrow();
  });

  it('八框架均具备 config_flag 字段（string | null），且返回对象为浅拷贝', () => {
    for (const fw of ALL_EIGHT) {
      const a = getFrameworkConfig(fw);
      const b = getFrameworkConfig(fw);
      expect('config_flag' in a).toBe(true);
      expect(a).not.toBe(b);
      expect(a).toEqual(b);
      a.config_flag = 'mutated';
      expect(getFrameworkConfig(fw).config_flag).not.toBe('mutated');
    }
  });
});

describe('getFrameworkConfig — test_execution 含 config_args 占位 (AC-3)', () => {
  it('既有 {files}/{directory} 占位断言保留；支持 config 的框架额外含 {config_args}', () => {
    for (const fw of ['jest', 'vitest', 'vite-plus']) {
      const result = getFrameworkConfig(fw);
      expect(te(result.shell.test_execution)).toContain('{files}');
      expect(te(result.shell.test_execution)).toContain('{config_args}');
    }
    expect(te(getFrameworkConfig('go').shell.test_execution)).toContain('{directory}');
    expect(te(getFrameworkConfig('go').shell.test_execution)).not.toContain('{config_args}');
  });

  it('未知框架名查询时抛 Error，不返回残缺 test_execution 模板', () => {
    expect(() => getFrameworkConfig('unknown-fw')).toThrow(/Unknown framework/);
  });
});

// ===========================================================================
// jest --randomize version gating
// ===========================================================================

describe('jest --randomize version gating', () => {
  it('jest >= 29.5.0 时 shell/cmd test_execution 含 --randomize', () => {
    const result = getFrameworkConfig('jest');
    expect(te(result.shell.test_execution, '29.5.0')).toContain('--randomize');
    expect(te(result.cmd.test_execution, '29.5.0')).toContain('--randomize');
    expect(te(result.shell.test_execution, '30.0.0')).toContain('--randomize');
  });

  it('jest < 29.5.0 或版本未知时不含 --randomize', () => {
    const result = getFrameworkConfig('jest');
    expect(te(result.shell.test_execution, '29.4.0')).not.toContain('--randomize');
    expect(te(result.shell.test_execution, '')).not.toContain('--randomize');
    expect(te(result.cmd.test_execution, '28.0.0')).not.toContain('--randomize');
  });
});

describe('detectFrameworkVersion', () => {
  // Global setup mocks this; restore real impl for probing tests.
  beforeEach(() => {
    vi.mocked(testFramework.detectFrameworkVersion).mockRestore();
  });
  afterEach(() => {
    vi.spyOn(testFramework, 'detectFrameworkVersion').mockReturnValue('99.0.0');
  });

  it('version_command 失败时返回空字符串', () => {
    const version = detectFrameworkVersion('bun', 'D:\\nonexistent-cwd-for-version-detect');
    expect(version).toBe('');
  });
});

describe('getFrameworkConfig -- version_command', () => {
  it('各框架提供约定的 version_command', () => {
    expect(getFrameworkConfig('jest').version_command).toBe('npx jest --version');
    expect(getFrameworkConfig('vitest').version_command).toBe('npx vitest --version');
    expect(getFrameworkConfig('vite-plus').version_command).toBe('vp --version');
    expect(getFrameworkConfig('bun').version_command).toBe('bun --version');
    expect(getFrameworkConfig('rust').version_command).toBe('cargo --version');
    expect(getFrameworkConfig('node-test').version_command).toBe('node --version');
    expect(getFrameworkConfig('go').version_command).toBe('go version');
    expect(getFrameworkConfig('pytest').version_command).toBe('pytest --version');
  });
});
