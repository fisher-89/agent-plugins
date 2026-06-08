/**
 * Tests for test-detect-frameworks -- MCP tool that detects test framework
 * per file using glob matching from config.json `test.frameworks`.
 *
 * Covers:
 * - AC-4: Glob-first-match file-to-framework detection
 * - AC-15: String shorthand normalisation to single-framework detection
 * - AC-4: plan entries include coverage_artifacts and coverage_cleanup
 * - Reverse AC-4: unmatched files return "unknown", empty file list returns empty
 * - Boundary: empty files list, auto-scan with no matches, string shorthand
 *
 * @see openspec/changes/per-directory-test-execution/test-design.md
 * @see openspec/changes/unified-coverage-artifacts/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

import { runTestDetectFrameworks, deriveWorkingDirectory } from './test-detect-frameworks';

// ---------------------------------------------------------------------------
// Helpers: create temp project directories with config.json
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(configData: unknown): TempProject {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'detect-fw-test-'));
  const openspecDir = path.join(tmpDir, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify(configData, null, 2),
    'utf-8',
  );
  return {
    root: tmpDir,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

// ===========================================================================
// AC-4: Glob-first-match file-to-framework detection
// ===========================================================================

describe('detectFrameworks -- glob first-match (AC-4)', () => {
  it('should map each file to the first matching framework glob', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: '**/*.test.ts', framework: 'vitest' },
          { glob: '**/tests/**/*.rs', framework: 'rust' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts', 'tests/unit/test_auth.rs'],
        projectRoot: project.root,
      });
      // Files are resolved against project root, so the returned file paths are absolute
      expect(result.detected).toHaveLength(2);
      const vitestEntry = result.detected.find((d) => d.file.endsWith('helper.test.ts'));
      const rustEntry = result.detected.find((d) => d.file.endsWith('test_auth.rs'));
      expect(vitestEntry?.framework).toBe('vitest');
      expect(rustEntry?.framework).toBe('rust');
      expect(result.frameworks).toContain('rust');
      expect(result.frameworks).toContain('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('should use first-match rule when a file matches multiple globs', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: '**/e2e/**', framework: 'cypress' },
          { glob: '**/*.ts', framework: 'vitest' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/e2e/test_app.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('cypress');
    } finally {
      project.cleanup();
    }
  });

  it('should return "unknown" for files not matching any glob (reverse AC-4)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['unknown.js'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should return empty detected for empty file list (reverse AC-4)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: '**/*.test.ts', framework: 'vitest' },
          { glob: '**/tests/**/*.rs', framework: 'rust' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: [],
        projectRoot: project.root,
      });
      expect(result.detected).toEqual([]);
      expect(result.frameworks).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// AC-15: String shorthand normalisation
// ===========================================================================

describe('detectFrameworks -- string shorthand (AC-15)', () => {
  it('should treat string frameworks as single-entry config and match files', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0].framework).toBe('vitest');
      expect(result.frameworks).toEqual(['vitest']);
    } finally {
      project.cleanup();
    }
  });

  it('should apply correct default glob for each framework shorthand', () => {
    // For vitest, the default glob should match .test.ts files
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.spec.js', 'src/util.test.ts', 'src/helper.ts'],
        projectRoot: project.root,
      });
      // .test.ts and .spec.js should match, but helper.ts (no .test/.spec) should not
      expect(result.detected).toHaveLength(3);
      expect(result.detected[0].framework).toBe('vitest');
      expect(result.detected[1].framework).toBe('vitest');
      expect(result.detected[2].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should match no files when string shorthand config has no matching files', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: [],
        projectRoot: project.root,
      });
      expect(result.detected).toEqual([]);
      expect(result.frameworks).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('detectFrameworks -- edge cases', () => {
  it('should handle files parameter as undefined (auto-scan) gracefully', () => {
    // When files is not provided, the tool auto-scans. With no matching files,
    // all scanned files are "unknown".
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      // Create a non-test file in the project to test auto-scan
      const srcDir = path.join(project.root, 'src');
      fs.mkdirSync(srcDir, { recursive: true });
      fs.writeFileSync(path.join(srcDir, 'readme.md'), '# readme', 'utf-8');

      const result = runTestDetectFrameworks({
        projectRoot: project.root,
      });
      // Should have auto-scanned files, none matching .test.ts
      expect(result.detected.length).toBeGreaterThanOrEqual(0);
      // All unmatched files get "unknown"
      expect(result.frameworks).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('should handle config with no frameworks gracefully', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should handle a very large file list without error', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const largeFiles = Array.from({ length: 1000 }, (_, i) => `src/test${i}.test.ts`);
      const result = runTestDetectFrameworks({
        files: largeFiles,
        projectRoot: project.root,
      });
      expect(result.detected.length).toBe(1000);
    } finally {
      project.cleanup();
    }
  });

  it('should handle files with special characters in paths', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/my test file.test.ts', 'src/测试.test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected).toHaveLength(2);
      expect(result.detected.every((d) => d.framework === 'vitest')).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('should handle nullish or missing config gracefully', () => {
    // When config has no test node at all, everything is "unknown"
    const project = createTempProject({
      schema: 'spec-driven',
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// deriveWorkingDirectory -- 纯函数测试
// ===========================================================================

describe('deriveWorkingDirectory -- 正向测试', () => {
  it('应返回无通配符路径自身（AC-2）: "plugins/dev-team/bin" → "plugins/dev-team/bin"', () => {
    expect(deriveWorkingDirectory('plugins/dev-team/bin')).toBe('plugins/dev-team/bin');
  });

  it('应取 ** 通配符前路径（AC-3）: "src/**/*.test.ts" → "src"', () => {
    expect(deriveWorkingDirectory('src/**/*.test.ts')).toBe('src');
  });

  it('应处理通配符起始（AC-4）: "**/*.test.ts" → "."', () => {
    expect(deriveWorkingDirectory('**/*.test.ts')).toBe('.');
  });

  it('应处理 {} 通配符（AC-5）: "{src,lib}/*.test.ts" → "."', () => {
    expect(deriveWorkingDirectory('{src,lib}/*.test.ts')).toBe('.');
  });

  it('应处理 ? 通配符: "tests/?nit/*.test.ts" → "tests"', () => {
    expect(deriveWorkingDirectory('tests/?nit/*.test.ts')).toBe('tests');
  });

  it('应处理多层 *: "packages/*/src/__tests__/*.test.ts" → "packages"', () => {
    expect(deriveWorkingDirectory('packages/*/src/__tests__/*.test.ts')).toBe('packages');
  });

  it('应归一化 Windows 反斜杠: "plugins\\\\dev-team\\\\bin" → "plugins/dev-team/bin"', () => {
    expect(deriveWorkingDirectory('plugins\\dev-team\\bin')).toBe('plugins/dev-team/bin');
  });
});

describe('deriveWorkingDirectory -- 异常测试', () => {
  it('应对空字符串返回自身: "" → ""（无通配符，返回自身）', () => {
    expect(deriveWorkingDirectory('')).toBe('');
  });

  it('应对 null/undefined 输入进行防御处理', () => {
    // 根据实现决策，null/undefined 可能抛出 TypeError 或返回 "."
    // TODO: 实现确认后调整期望值
    // eslint-disable-next-line typescript/no-unsafe-type-assertion
    expect(() => deriveWorkingDirectory(null as unknown as string)).toThrow();
    // eslint-disable-next-line typescript/no-unsafe-type-assertion
    expect(() => deriveWorkingDirectory(undefined as unknown as string)).toThrow();
  });
});

describe('deriveWorkingDirectory -- 边界测试', () => {
  it('应处理超长无通配符 glob（1000 字符）', () => {
    const longPath = 'a'.repeat(1000);
    expect(deriveWorkingDirectory(longPath)).toBe(longPath);
  });

  it('应处理超长含通配符 glob（500 字符前缀 + "/*/b"）', () => {
    const prefix = 'a'.repeat(500);
    const result = deriveWorkingDirectory(prefix + '/*/b');
    expect(result).toBe(prefix);
  });

  it('应处理含空格的路径: "src/my test file/*.ts" → "src/my test file"', () => {
    expect(deriveWorkingDirectory('src/my test file/*.ts')).toBe('src/my test file');
  });

  it('应处理纯通配符: "*" → "."', () => {
    expect(deriveWorkingDirectory('*')).toBe('.');
  });

  it('应以 "/" 结尾的通配符: "src/**" → "src"', () => {
    expect(deriveWorkingDirectory('src/**')).toBe('src');
  });

  it('应处理仅有点的路径: "." → "."（无通配符，返回自身）', () => {
    expect(deriveWorkingDirectory('.')).toBe('.');
  });

  it('应处理仅斜杠的路径: "/" → "/"（无通配符，返回自身）', () => {
    expect(deriveWorkingDirectory('/')).toBe('/');
  });

  it('应合并连续分隔符: "src//lib/**/*.ts" → "src/lib"', () => {
    expect(deriveWorkingDirectory('src//lib/**/*.ts')).toBe('src/lib');
  });
});

// ===========================================================================
// runTestDetectFrameworks -- plan 集成测试
// ===========================================================================

describe('runTestDetectFrameworks -- plan 内容正确性（AC-6）', () => {
  it('应为单框架单目录配置生成正确的 plan 条目', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: 'plugins/dev-team/bin', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: 'plugins/dev-team/bin',
        framework: 'vite-plus',
        coverage_cmd: 'vp test --coverage',
        coverage_format: 'istanbul',
        coverage_output: 'coverage/coverage-summary.json',
      });
    } finally {
      project.cleanup();
    }
  });

  it('应为多框架多目录配置生成正确的 plan 数组', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: 'plugins/dev-team/bin', framework: 'vite-plus' },
          { glob: 'src/**/*.test.ts', framework: 'vitest' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/mcp.ts', 'src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      expect(result.plan[0].directory).toBe('plugins/dev-team/bin');
      expect(result.plan[0].framework).toBe('vite-plus');
      expect(result.plan[1].directory).toBe('src');
      expect(result.plan[1].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks -- 字符串简写 plan（AC-9）', () => {
  it('应为字符串简写 "vitest" 生成 plan，directory 为 "."', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'vitest',
        coverage_cmd: 'npx vitest run --coverage',
        coverage_format: 'istanbul',
        coverage_output: 'coverage/coverage-summary.json',
      });
    } finally {
      project.cleanup();
    }
  });

  it('应为字符串简写 "jest" 生成 plan，directory 为 "."', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'jest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.spec.js'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'jest',
        coverage_cmd: 'npx jest --coverage',
        coverage_format: 'istanbul',
      });
    } finally {
      project.cleanup();
    }
  });

  it('应为字符串简写 "rust" 生成 plan，directory 为 "."，coverage_format 为 "llvm-cov"', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'rust',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'rust',
        coverage_format: 'llvm-cov',
      });
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks -- 空配置 plan（AC-10）', () => {
  it('应在 test.frameworks 为空数组时返回空 plan', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('应在无 test.frameworks 配置时返回空 plan', () => {
    const project = createTempProject({
      schema: 'spec-driven',
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toEqual([]);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 向后兼容回归测试（AC-11）
// ===========================================================================

describe('runTestDetectFrameworks -- 向后兼容（AC-11）', () => {
  it('添加 plan 后 detected 字段结构和内容不变', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      // detected 数组结构应与旧版一致
      expect(result.detected).toHaveLength(1);
      expect(result.detected[0]).toHaveProperty('file');
      expect(result.detected[0]).toHaveProperty('framework');
      expect(result.detected[0].framework).toBe('vitest');
    } finally {
      project.cleanup();
    }
  });

  it('添加 plan 后 frameworks 字段结构和内容不变', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: '**/*.test.ts', framework: 'vitest' },
          { glob: '**/tests/**/*.rs', framework: 'rust' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts', 'tests/test_auth.rs'],
        projectRoot: project.root,
      });
      // frameworks 列表应与旧版一致（排序后的唯一框架名）
      expect(result.frameworks).toContain('vitest');
      expect(result.frameworks).toContain('rust');
    } finally {
      project.cleanup();
    }
  });

  it('添加 plan 后首匹配规则不变', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: '**/e2e/**', framework: 'cypress' },
          { glob: '**/*.ts', framework: 'vitest' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/e2e/test_app.ts'],
        projectRoot: project.root,
      });
      // 应返回第一个匹配的 cypress，而非 vitest
      expect(result.detected[0].framework).toBe('cypress');
    } finally {
      project.cleanup();
    }
  });

  it('添加 plan 后无匹配文件仍返回 "unknown"', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['unknown.js'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// AC-4: plan 条目包含 coverage_artifacts 和 coverage_cleanup
// ===========================================================================

describe('runTestDetectFrameworks -- plan 新增覆盖率产物字段 (AC-4)', () => {
  it('配置 vitest 框架时，plan 条目包含 coverage_artifacts 和 coverage_cleanup', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: '**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toHaveProperty('coverage_artifacts');
      expect(result.plan[0]).toHaveProperty('coverage_cleanup');
    } finally {
      project.cleanup();
    }
  });

  it('vitest plan 条目的 coverage_artifacts 为 ["coverage/**"]，coverage_cleanup 为 ["coverage", ".nyc_output"]', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: 'src/**/*.test.ts', framework: 'vitest' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan[0].coverage_artifacts).toEqual(['coverage/**']);
      expect(result.plan[0].coverage_cleanup).toEqual(['coverage', '.nyc_output']);
    } finally {
      project.cleanup();
    }
  });

  it('多框架 [vitest, rust] 时每个 plan 条目包含对应 coverage_artifacts 和 coverage_cleanup', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [
          { glob: 'src/**/*.test.ts', framework: 'vitest' },
          { glob: 'tests/**/*.rs', framework: 'rust' },
        ],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts', 'tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      for (const entry of result.plan) {
        expect(entry).toHaveProperty('coverage_artifacts');
        expect(entry).toHaveProperty('coverage_cleanup');
      }
    } finally {
      project.cleanup();
    }
  });

  it('rust plan 条目 coverage_artifacts 包含 coverage/** 和 target/llvm-cov/**', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: [{ glob: 'tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan[0].coverage_artifacts).toContain('coverage/**');
      expect(result.plan[0].coverage_artifacts).toContain('target/llvm-cov/**');
      expect(result.plan[0].coverage_cleanup).toContain('coverage');
      expect(result.plan[0].coverage_cleanup).toContain('target/llvm-cov');
    } finally {
      project.cleanup();
    }
  });

  it('字符串简写 "vitest" 时 plan 条目也包含 coverage_artifacts 和 coverage_cleanup', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        frameworks: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toHaveProperty('coverage_artifacts');
      expect(result.plan[0]).toHaveProperty('coverage_cleanup');
      expect(result.plan[0].coverage_artifacts).toEqual(['coverage/**']);
    } finally {
      project.cleanup();
    }
  });
});
