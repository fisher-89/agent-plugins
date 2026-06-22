/**
 * 单元测试: lib/glob.ts — matchGlob 与 scanProjectFiles
 *
 * 覆盖 use-fast-glob 变更 AC-2 ~ AC-5：
 * - matchGlob 项目 glob 模式正向/反向/边界匹配
 * - Windows/POSIX 路径分隔符归一化
 * - scanProjectFiles 多模式扫描、默认排除目录、排序去重
 *
 * @see openspec/changes/use-fast-glob/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { describe, it, expect } from 'vite-plus/test';

import { matchGlob, scanProjectFiles } from './glob';

// ---------------------------------------------------------------------------
// Helpers: 临时项目目录
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'glob-test-'));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function writeFile(projectRoot: string, relativePath: string, content = ''): string {
  const fullPath = path.join(projectRoot, relativePath);
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });
  fs.writeFileSync(fullPath, content, 'utf-8');
  return fullPath;
}

// ===========================================================================
// glob.ts 模块导出（AC-2）
// ===========================================================================

describe('glob.ts 模块导出', () => {
  it('应从 lib/glob.ts 可导入 matchGlob 与 scanProjectFiles 且为函数类型', () => {
    expect(typeof matchGlob).toBe('function');
    expect(typeof scanProjectFiles).toBe('function');
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

// ===========================================================================
// scanProjectFiles — 多模式扫描（AC-3）
// ===========================================================================

describe('scanProjectFiles — 多模式扫描', () => {
  it('多 pattern 应返回 src/a.test.ts 与 tests/foo.rs 的绝对路径', () => {
    const project = createTempProject();
    try {
      const testTs = writeFile(project.root, 'src/a.test.ts');
      const testRs = writeFile(project.root, 'tests/foo.rs');
      const results = scanProjectFiles(project.root, ['**/*.test.ts', '**/tests/**/*.rs']);
      expect(results).toContain(testTs);
      expect(results).toContain(testRs);
    } finally {
      project.cleanup();
    }
  });

  it('无通配符目录 pattern 应扫描子目录文件', () => {
    const project = createTempProject();
    try {
      const testFile = writeFile(project.root, 'plugins/dev-team/bin/src/foo.test.ts');
      const results = scanProjectFiles(project.root, ['plugins/dev-team/bin']);
      expect(results).toContain(testFile);
    } finally {
      project.cleanup();
    }
  });

  it('多 pattern 重叠匹配同一文件时结果应去重且按字典序排序', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/a.test.ts');
      const results = scanProjectFiles(project.root, ['**/*.test.ts', 'src/**/*.ts']);
      const unique = new Set(results);
      expect(unique.size).toBe(results.length);
      expect([...results]).toEqual([...results].sort());
    } finally {
      project.cleanup();
    }
  });

  it('扫描结果应仅含文件不含目录节点', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/a.test.ts');
      const results = scanProjectFiles(project.root, ['**/*.test.ts']);
      for (const p of results) {
        expect(fs.statSync(p).isFile()).toBe(true);
      }
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// scanProjectFiles — 默认排除目录（AC-5）
// ===========================================================================

describe('scanProjectFiles — 默认排除目录', () => {
  const excludedPaths = [
    'node_modules/pkg/index.test.ts',
    '.git/hooks/pre-commit.test.ts',
    'dist/bundle.test.js',
    'build/output.test.js',
    'target/debug/deps/test.rs',
    '.vp/cache.test.ts',
    'coverage/lcov.test.ts',
    '.nyc_output/temp.test.ts',
    '.claude/cache.test.ts',
  ] as const;

  for (const relativePath of excludedPaths) {
    it(`排除目录内文件不应出现在结果中: ${relativePath}`, () => {
      const project = createTempProject();
      try {
        writeFile(project.root, relativePath);
        const results = scanProjectFiles(project.root, ['**/*']);
        const matched = results.some((p) => p.replace(/\\/g, '/').endsWith(relativePath));
        expect(matched).toBe(false);
      } finally {
        project.cleanup();
      }
    });
  }

  it('排除目录之外的 src/valid.test.ts 应正常返回', () => {
    const project = createTempProject();
    try {
      const validFile = writeFile(project.root, 'src/valid.test.ts');
      const results = scanProjectFiles(project.root, ['**/*.test.ts']);
      expect(results).toContain(validFile);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// scanProjectFiles — 输入边界
// ===========================================================================

describe('scanProjectFiles — 输入边界', () => {
  it('patterns 为空数组时应返回 []', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'src/a.test.ts');
      expect(scanProjectFiles(project.root, [])).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('项目根目录无匹配文件时应返回 []', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'readme.md');
      expect(scanProjectFiles(project.root, ['**/*.test.ts'])).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('通过 options.ignore 追加排除模式后对应路径应被过滤', () => {
    const project = createTempProject();
    try {
      writeFile(project.root, 'vendor/pkg/index.test.ts');
      const withoutIgnore = scanProjectFiles(project.root, ['**/*.test.ts']);
      const withIgnore = scanProjectFiles(project.root, ['**/*.test.ts'], {
        ignore: ['**/vendor/**'],
      });
      expect(withoutIgnore.length).toBeGreaterThan(0);
      expect(withIgnore).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});
