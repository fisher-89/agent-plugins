/**
 * 单元测试: lib/test-exclude.ts — isFileExcluded / getExcludeGlobs
 *
 * 覆盖 suite-scoped excludes（相对 suite.root），不再使用旧 test.exclude。
 *
 * @see openspec/changes/tests-array-cwd-config/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import type { OpenSpecConfig, TestSuite } from '../schemas';
import { isFileExcluded, getExcludeGlobs } from './test-exclude';

function makeSuite(
  overrides: Partial<TestSuite> & Pick<TestSuite, 'root' | 'framework'>,
): TestSuite {
  return {
    cwd: '.',
    coverage: { lines: 80, branches: 70, functions: 75 },
    mutation: { score: 70 },
    ...overrides,
  };
}

function makeConfig(tests: TestSuite[]): OpenSpecConfig {
  return { schema: 'spec-driven', tests };
}

// ===========================================================================
// isFileExcluded — suite excludes (AC-4)
// ===========================================================================

describe('isFileExcluded — suite excludes (AC-4)', () => {
  it('文件在 suite.root 下且匹配 excludes 时返回 true', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['**/*.test.ts'] }),
    ]);
    expect(isFileExcluded('pkg/src/foo.test.ts', config)).toBe(true);
  });

  it('文件匹配 excludes 但不在该 suite.root 下时返回 false（范围隔离）', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg-a', framework: 'vite-plus', excludes: ['**/*.test.ts'] }),
    ]);
    expect(isFileExcluded('pkg-b/src/foo.test.ts', config)).toBe(false);
  });

  it('文件在 root 下但不匹配 excludes 时返回 false', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['**/*.test.ts'] }),
    ]);
    expect(isFileExcluded('pkg/src/foo.ts', config)).toBe(false);
  });

  it('多 suite 各自 excludes 仅作用于对应 root 树', () => {
    const config = makeConfig([
      makeSuite({ root: 'dir-a', framework: 'vite-plus', excludes: ['**/*.test.ts'] }),
      makeSuite({ root: 'dir-b', framework: 'vitest', excludes: ['**/ignored/**'] }),
    ]);
    expect(isFileExcluded('dir-a/src/foo.test.ts', config)).toBe(true);
    expect(isFileExcluded('dir-b/src/foo.test.ts', config)).toBe(false);
    expect(isFileExcluded('dir-b/ignored/x.ts', config)).toBe(true);
  });

  it('filePath 为 undefined/null 时明确抛 TypeError', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['**/*'] }),
    ]);
    expect(() => {
      // @ts-expect-error intentional invalid runtime input
      isFileExcluded(undefined, config);
    }).toThrow();
    expect(() => {
      // @ts-expect-error intentional invalid runtime input
      isFileExcluded(null, config);
    }).toThrow();
  });

  it('config.tests 缺失或非数组时安全回落（false / 不崩溃）', () => {
    expect(
      isFileExcluded('pkg/a.ts', {
        schema: 'spec-driven',
        // @ts-expect-error intentional missing tests for runtime fallback
        tests: undefined,
      }),
    ).toBe(false);
    expect(
      isFileExcluded('pkg/a.ts', {
        schema: 'spec-driven',
        // @ts-expect-error intentional invalid tests shape for runtime fallback
        tests: null,
      }),
    ).toBe(false);
  });

  it('filePath 为空字符串时返回 false', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['**/*'] }),
    ]);
    expect(isFileExcluded('', config)).toBe(false);
  });

  it('filePath 为超长路径（>1000）不抛异常且匹配结果确定', () => {
    const longSeg = 'a'.repeat(1000);
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['**/*.test.ts'] }),
    ]);
    expect(() => isFileExcluded(`pkg/${longSeg}/foo.test.ts`, config)).not.toThrow();
    expect(isFileExcluded(`pkg/${longSeg}/foo.test.ts`, config)).toBe(true);
  });

  it('filePath 含特殊字符（空格 / emoji / Unicode）时匹配正确', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['**/*.test.ts'] }),
    ]);
    expect(isFileExcluded('pkg/my test/测试🚀.test.ts', config)).toBe(true);
  });

  it('excludes: [] 或省略时返回 false', () => {
    expect(
      isFileExcluded(
        'pkg/a.ts',
        makeConfig([makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: [] })]),
      ),
    ).toBe(false);
    expect(
      isFileExcluded('pkg/a.ts', makeConfig([makeSuite({ root: 'pkg', framework: 'vite-plus' })])),
    ).toBe(false);
  });

  it('tests: [] 时返回 false', () => {
    expect(isFileExcluded('pkg/a.ts', makeConfig([]))).toBe(false);
  });

  it('Windows 反斜杠路径归一化为 POSIX 后匹配', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['src/**/*.test.ts'] }),
    ]);
    expect(isFileExcluded('pkg\\src\\utils\\helper.test.ts', config)).toBe(true);
    expect(isFileExcluded('pkg\\src\\utils\\app.ts', config)).toBe(false);
  });
});

// ===========================================================================
// getExcludeGlobs — suite excludes (AC-4)
// ===========================================================================

describe('getExcludeGlobs — suite excludes (AC-4)', () => {
  it('单 suite 返回拼成 project-relative 的 exclude 模式', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['**/*.snap', 'dist/**'] }),
    ]);
    const result = getExcludeGlobs(config);
    expect(result).toContain('pkg/**/*.snap');
    expect(result).toContain('pkg/dist/**');
    expect(result).toHaveLength(2);
  });

  it('多 suite 返回并集', () => {
    const config = makeConfig([
      makeSuite({ root: 'a', framework: 'vite-plus', excludes: ['**/*.test.ts'] }),
      makeSuite({ root: 'b', framework: 'vitest', excludes: ['**/gen/**'] }),
    ]);
    const result = getExcludeGlobs(config);
    expect(result).toContain('a/**/*.test.ts');
    expect(result).toContain('b/**/gen/**');
    expect(result).toHaveLength(2);
  });

  it('config 为 null/undefined 时明确抛错', () => {
    expect(() => {
      // @ts-expect-error intentional invalid runtime input
      getExcludeGlobs(null);
    }).toThrow();
    expect(() => {
      // @ts-expect-error intentional invalid runtime input
      getExcludeGlobs(undefined);
    }).toThrow();
  });

  it('excludes 为单元素数组时返回对应一条 project-relative 模式', () => {
    const config = makeConfig([
      makeSuite({ root: 'src', framework: 'vite-plus', excludes: ['**/node_modules/**'] }),
    ]);
    expect(getExcludeGlobs(config)).toEqual(['src/**/node_modules/**']);
  });

  it('excludes 超大列表（大量模式）时返回完整并集', () => {
    const patterns = Array.from({ length: 200 }, (_, i) => `p${i}/**`);
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: patterns }),
    ]);
    const result = getExcludeGlobs(config);
    expect(result).toHaveLength(200);
    expect(result[0]).toBe('pkg/p0/**');
    expect(result[199]).toBe('pkg/p199/**');
  });

  it('重复模式去重', () => {
    const config = makeConfig([
      makeSuite({ root: 'pkg', framework: 'vite-plus', excludes: ['**/*.ts', '**/*.ts'] }),
      makeSuite({ root: 'pkg', framework: 'vitest', excludes: ['**/*.ts'] }),
    ]);
    expect(getExcludeGlobs(config)).toEqual(['pkg/**/*.ts']);
  });

  it('无 excludes / 空 tests 时返回 []', () => {
    expect(getExcludeGlobs(makeConfig([]))).toEqual([]);
    expect(
      getExcludeGlobs(makeConfig([makeSuite({ root: 'pkg', framework: 'vite-plus' })])),
    ).toEqual([]);
  });
});
