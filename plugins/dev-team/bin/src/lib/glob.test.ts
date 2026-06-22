/**
 * 单元测试: lib/glob.ts — matchGlob
 *
 * 覆盖 use-fast-glob 变更 AC-2 ~ AC-5：
 * - matchGlob 项目 glob 模式正向/反向/边界匹配
 * - Windows/POSIX 路径分隔符归一化
 *
 * @see openspec/changes/use-fast-glob/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import { matchGlob } from './glob';

// ===========================================================================
// glob.ts 模块导出（AC-2）
// ===========================================================================

describe('glob.ts 模块导出', () => {
  it('应从 lib/glob.ts 可导入 matchGlob 且为函数类型', () => {
    expect(typeof matchGlob).toBe('function');
  });

  it('matchGlob 对相同输入多次调用结果应一致（纯函数确定性）', () => {
    const filePath = 'src/utils/helper.test.ts';
    const pattern = '**/*.{test,spec}.{js,ts,jsx,tsx}';
    expect(matchGlob(filePath, pattern)).toBe(matchGlob(filePath, pattern));
  });
});

// ===========================================================================
// matchGlob — 项目 glob 模式正向匹配（AC-3）
// ===========================================================================

describe('matchGlob — 项目 glob 模式正向匹配', () => {
  it('应匹配 vitest 默认 glob: src/utils/helper.test.ts', () => {
    expect(matchGlob('src/utils/helper.test.ts', '**/*.{test,spec}.{js,ts,jsx,tsx}')).toBe(true);
  });

  it('应匹配 .spec.js 文件: src/util.spec.js', () => {
    expect(matchGlob('src/util.spec.js', '**/*.{test,spec}.{js,ts,jsx,tsx}')).toBe(true);
  });

  it('应匹配 rust 测试 glob: tests/integration/test_auth.rs', () => {
    expect(matchGlob('tests/integration/test_auth.rs', '**/tests/**/*.rs')).toBe(true);
  });

  it('应匹配无通配符目录前缀: plugins/dev-team/bin/src/foo.test.ts', () => {
    expect(matchGlob('plugins/dev-team/bin/src/foo.test.ts', 'plugins/dev-team/bin')).toBe(true);
  });

  it('应匹配 e2e 目录 glob: tests/e2e/test_app.ts', () => {
    expect(matchGlob('tests/e2e/test_app.ts', '**/e2e/**')).toBe(true);
  });
});

// ===========================================================================
// matchGlob — 路径分隔符归一化（AC-4）
// ===========================================================================

describe('matchGlob — 路径分隔符归一化', () => {
  it('Windows 反斜杠与 POSIX 正斜杠路径应对 **/*.test.ts 均返回 true', () => {
    const pattern = '**/*.test.ts';
    expect(matchGlob('src\\utils\\helper.test.ts', pattern)).toBe(true);
    expect(matchGlob('src/utils/helper.test.ts', pattern)).toBe(true);
  });

  it('Windows 反斜杠路径应对无通配符目录前缀返回 true', () => {
    expect(matchGlob('plugins\\dev-team\\bin\\src\\foo.test.ts', 'plugins/dev-team/bin')).toBe(
      true,
    );
  });

  it('pattern 含反斜杠与正斜杠时对同一路径应产生相同结果', () => {
    const filePath = 'src/utils/helper.test.ts';
    expect(matchGlob(filePath, '**\\*.test.ts')).toBe(matchGlob(filePath, '**/*.test.ts'));
  });
});

// ===========================================================================
// matchGlob — 不匹配路径（AC-3）
// ===========================================================================

describe('matchGlob — 不匹配路径', () => {
  it('非测试文件 helper.ts 不应匹配 **/*.test.ts', () => {
    expect(matchGlob('src/utils/helper.ts', '**/*.test.ts')).toBe(false);
  });

  it('readme.md 不应匹配 vitest 默认 glob', () => {
    expect(matchGlob('src/readme.md', '**/*.{test,spec}.{js,ts,jsx,tsx}')).toBe(false);
  });
});

// ===========================================================================
// matchGlob — glob 语法边界（AC-3）
// ===========================================================================

describe('matchGlob — glob 语法边界', () => {
  it('** 零段路径: foo.test.ts 应匹配 **/*.test.ts', () => {
    expect(matchGlob('foo.test.ts', '**/*.test.ts')).toBe(true);
  });

  it('? 单字符通配: tests/unit/foo.test.ts 应匹配 tests/?nit/*.test.ts', () => {
    expect(matchGlob('tests/unit/foo.test.ts', 'tests/?nit/*.test.ts')).toBe(true);
  });

  it('花括号备选: lib/utils.test.ts 应匹配 {src,lib}/*.test.ts', () => {
    expect(matchGlob('lib/utils.test.ts', '{src,lib}/*.test.ts')).toBe(true);
  });

  it('无通配符精确路径不匹配: other/foo.test.ts 不应匹配 plugins/dev-team/bin', () => {
    expect(matchGlob('other/foo.test.ts', 'plugins/dev-team/bin')).toBe(false);
  });

  it('无通配符目录自身应匹配: plugins/dev-team/bin', () => {
    expect(matchGlob('plugins/dev-team/bin', 'plugins/dev-team/bin')).toBe(true);
  });
});

// ===========================================================================
// matchGlob — 输入边界
// ===========================================================================

describe('matchGlob — 输入边界', () => {
  it('filePath 为空字符串时应返回确定性布尔值', () => {
    const result = matchGlob('', '**/*.test.ts');
    expect(typeof result).toBe('boolean');
    expect(result).toBe(false);
  });

  it('pattern 为空字符串时应返回确定性结果', () => {
    const result = matchGlob('src/foo.test.ts', '');
    expect(typeof result).toBe('boolean');
  });

  it('超长路径（>1000 字符）+ 标准 glob 模式不应抛出异常', () => {
    const longSegment = 'a'.repeat(1000);
    expect(() => matchGlob(`${longSegment}/foo.test.ts`, '**/*.test.ts')).not.toThrow();
  });

  it('路径含空格与 Unicode 时应匹配 **/*.test.ts', () => {
    expect(matchGlob('src/my test/测试.test.ts', '**/*.test.ts')).toBe(true);
  });
});
