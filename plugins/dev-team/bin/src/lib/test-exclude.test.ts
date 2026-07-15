/**
 * 单元测试: lib/test-exclude.ts — isFileExcluded / getExcludeGlobs
 *
 * 覆盖:
 * - AC-2: isFileExcluded 正向（全局/override/混合/不匹配/范围隔离）
 * - AC-2: getExcludeGlobs 正向（全局/合并）
 * - AC-2: isFileExcluded 边界（空数组/undefined/特殊字符/Windows路径/超长路径）
 * - AC-2: getExcludeGlobs 边界（去重/空配置）
 * - AC-6: 向后兼容（不配置 exclude 时返回 false）
 *
 * @see openspec/changes/test-exclude-source-globs/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import type { OpenSpecConfig } from '../schemas';
import { isFileExcluded, getExcludeGlobs } from './test-exclude';

// ===========================================================================
// isFileExcluded — 正向场景
// ===========================================================================

describe('isFileExcluded — 正向', () => {
  it('文件匹配全局 exclude glob 时返回 true', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: { exclude: ['**/node_modules/**'] },
    } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/node_modules/pkg/index.js', config)).toBe(true);
  });

  it('文件匹配 override-level exclude glob 时返回 true', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: {
        overrides: [{ file: 'plugins/dev-team/bin', exclude: ['**/*.test.ts'] }],
      },
    } satisfies OpenSpecConfig;
    expect(isFileExcluded('plugins/dev-team/bin/src/foo.test.ts', config)).toBe(true);
  });

  it('文件同时匹配全局和 override exclude 时返回 true', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: {
        exclude: ['**/*.snap'],
        overrides: [{ file: 'plugins/dev-team/bin', exclude: ['**/*.snap'] }],
      },
    } satisfies OpenSpecConfig;
    expect(isFileExcluded('plugins/dev-team/bin/src/foo.snap', config)).toBe(true);
  });

  it('文件不匹配任何 exclude glob 时返回 false', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: { exclude: ['**/node_modules/**'] },
    } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/app.ts', config)).toBe(false);
  });

  it('override-level exclude 仅在该 override file 范围内生效', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: {
        overrides: [
          { file: 'dir-a', exclude: ['**/*.test.ts'] },
          { file: 'dir-b', exclude: ['**/ignored/**'] },
        ],
      },
    } satisfies OpenSpecConfig;
    // dir-b 下的 .test.ts 文件不应被 dir-a 的 exclude 排除
    expect(isFileExcluded('dir-b/src/foo.test.ts', config)).toBe(false);
  });
});

// ===========================================================================
// isFileExcluded — 边界场景
// ===========================================================================

describe('isFileExcluded — 边界', () => {
  it('exclude 数组为空时返回 false', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: { exclude: [] },
    } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/app.ts', config)).toBe(false);
  });

  it('exclude 未配置（undefined）时返回 false', () => {
    const config = { schema: 'spec-driven' as const, test: {} } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/app.ts', config)).toBe(false);

    const config2 = { schema: 'spec-driven' as const, test: {} } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/app.ts', config2)).toBe(false);
  });

  it('路径含特殊字符（空格、Unicode）时 glob 匹配正确', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: { exclude: ['**/*.test.ts'] },
    } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/my test/测试.test.ts', config)).toBe(true);
    expect(isFileExcluded('src/normal.test.ts', config)).toBe(true);
  });

  it('Windows 反斜杠路径在 glob 匹配前被转换为 POSIX 格式', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: { exclude: ['src/**/*.test.ts'] },
    } satisfies OpenSpecConfig;
    expect(isFileExcluded('src\\utils\\helper.test.ts', config)).toBe(true);
    expect(isFileExcluded('src\\utils\\app.ts', config)).toBe(false);
  });

  it('超长路径字符串不应导致异常', () => {
    const longSegment = 'a'.repeat(1000);
    const config = {
      schema: 'spec-driven' as const,
      test: { exclude: ['**/*.test.ts'] },
    } satisfies OpenSpecConfig;
    expect(() => isFileExcluded(`${longSegment}/foo.test.ts`, config)).not.toThrow();
  });
});

// ===========================================================================
// isFileExcluded — 向后兼容 (AC-6)
// ===========================================================================

describe('isFileExcluded — 向后兼容 (AC-6)', () => {
  it('不配置 exclude 时返回 false（不影响既有行为）', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: { framework: 'vitest' as const },
    } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/app.ts', config)).toBe(false);
  });

  it('test 节只包含默认值时返回 false（无 exclude）', () => {
    const config = { schema: 'spec-driven' as const, test: {} } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/app.ts', config)).toBe(false);
  });

  it('仅含 schema 的默认 config 返回 false', () => {
    const config = { schema: 'spec-driven' as const, test: {} } satisfies OpenSpecConfig;
    expect(isFileExcluded('src/app.ts', config)).toBe(false);
  });
});

// ===========================================================================
// getExcludeGlobs — 正向场景
// ===========================================================================

describe('getExcludeGlobs — 正向', () => {
  it('仅全局 exclude 时返回全局列表', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: { exclude: ['**/node_modules/**', '**/dist/**'] },
    } satisfies OpenSpecConfig;
    expect(getExcludeGlobs(config)).toEqual(['**/node_modules/**', '**/dist/**']);
  });

  it('合并全局和 override-level 的 exclude globs', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: {
        exclude: ['**/node_modules/**'],
        overrides: [
          { file: 'dir-a', exclude: ['**/*.test.ts'] },
          { file: 'dir-b', exclude: ['**/generated/**'] },
        ],
      },
    } satisfies OpenSpecConfig;
    const result = getExcludeGlobs(config);
    expect(result).toContain('**/node_modules/**');
    expect(result).toContain('**/*.test.ts');
    expect(result).toContain('**/generated/**');
    expect(result).toHaveLength(3);
  });
});

// ===========================================================================
// getExcludeGlobs — 边界场景
// ===========================================================================

describe('getExcludeGlobs — 边界', () => {
  it('全局和 override 存在重复 glob 时去重', () => {
    const config = {
      schema: 'spec-driven' as const,
      test: {
        exclude: ['**/node_modules/**'],
        overrides: [{ file: 'dir-a', exclude: ['**/node_modules/**'] }],
      },
    } satisfies OpenSpecConfig;
    expect(getExcludeGlobs(config)).toEqual(['**/node_modules/**']);
  });

  it('无任何 exclude 时返回空数组', () => {
    const config = { schema: 'spec-driven' as const, test: {} } satisfies OpenSpecConfig;
    expect(getExcludeGlobs(config)).toEqual([]);

    const config2 = { schema: 'spec-driven' as const, test: {} } satisfies OpenSpecConfig;
    expect(getExcludeGlobs(config2)).toEqual([]);
  });
});
