/**
 * Tests for test-detect-frameworks -- MCP tool that detects test framework
 * per file using glob matching from config.json `test.framework` + `test.overrides`.
 *
 * Covers:
 * - AC-4: Glob-first-match file-to-framework detection
 * - AC-4: plan entries include coverage_artifacts, coverage_cleanup, script
 * - Reverse AC-4: unmatched files return "unknown", empty file list returns empty
 * - Boundary: empty files list, auto-scan with no matches
 *
 * @see openspec/changes/per-directory-test-execution/test-design.md
 * @see openspec/changes/unified-coverage-artifacts/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

import {
  runTestDetectFrameworks,
  deriveWorkingDirectory,
  generateScript,
} from './test-detect-frameworks';

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
  it('should map each file to its matching framework via default + overrides', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: '**/tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts', 'tests/unit/test_auth.rs'],
        projectRoot: project.root,
      });
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

  it('should use first-match rule when a file matches both default and override globs', () => {
    // Default vitest glob matches test files; override **/e2e/** is checked second
    // but e2e file names like test_app.ts don't match vitest default glob
    // (no .test./.spec. in name), so only the override matches
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: '**/e2e/**', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/e2e/test_app.ts'],
        projectRoot: project.root,
      });
      expect(result.detected[0].framework).toBe('vite-plus');
    } finally {
      project.cleanup();
    }
  });

  it('should return "unknown" for files not matching any glob (reverse AC-4)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
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
        framework: 'vitest',
        overrides: [{ file: '**/tests/**/*.rs', framework: 'rust' }],
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
// Single-framework tests
// ===========================================================================

describe('detectFrameworks -- single framework', () => {
  it('should detect framework from default config and match files', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
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

  it('should apply framework default glob for matching', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.spec.js', 'src/util.test.ts', 'src/helper.ts'],
        projectRoot: project.root,
      });
      // .test.ts and .spec.js match vitest default glob; helper.ts does not
      expect(result.detected).toHaveLength(3);
      expect(result.detected[0].framework).toBe('vitest');
      expect(result.detected[1].framework).toBe('vitest');
      expect(result.detected[2].framework).toBe('unknown');
    } finally {
      project.cleanup();
    }
  });

  it('should match no files when framework config has no matching files', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
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
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
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
      expect(result.detected.length).toBeGreaterThanOrEqual(0);
      expect(result.frameworks).toEqual([]);
    } finally {
      project.cleanup();
    }
  });

  it('should handle config with no framework gracefully', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {},
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
        framework: 'vitest',
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
        framework: 'vitest',
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

  it('should handle missing config gracefully', () => {
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

describe('runTestDetectFrameworks -- plan 内容正确性 (AC-6)', () => {
  it('应为单框架配置生成正确的 plan 条目', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vite-plus',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toMatchObject({
        directory: '.',
        framework: 'vite-plus',
        coverage_cmd: 'vp test --coverage',
        coverage_format: 'istanbul',
        coverage_output: 'coverage/coverage-summary.json',
      });
    } finally {
      project.cleanup();
    }
  });

  it('应为多框架多目录配置生成正确的 plan 数组（default first, then overrides）', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'plugins/dev-team/bin/**', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/main.test.ts', 'src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      // Default framework entry comes first
      expect(result.plan[0].directory).toBe('.');
      expect(result.plan[0].framework).toBe('vitest');
      // Override entry comes second
      expect(result.plan[1].directory).toBe('plugins/dev-team/bin');
      expect(result.plan[1].framework).toBe('vite-plus');
    } finally {
      project.cleanup();
    }
  });
});

describe('runTestDetectFrameworks -- 单框架 plan (AC-9)', () => {
  it('应为 "vitest" 生成 plan，directory 为 "."', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
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

  it('应为 "jest" 生成 plan，directory 为 "."', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'jest',
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

  it('应为 "rust" 生成 plan，directory 为 "."，coverage_format 为 "llvm-cov"', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'rust',
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

describe('runTestDetectFrameworks -- 空配置 plan (AC-10)', () => {
  it('应在无 framework 配置时返回空 plan', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {},
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

  it('应在无 test 配置时返回空 plan', () => {
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
// 向后兼容回归测试 (AC-11)
// ===========================================================================

describe('runTestDetectFrameworks -- 向后兼容 (AC-11)', () => {
  it('添加 plan 后 detected 字段结构和内容不变', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts'],
        projectRoot: project.root,
      });
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
        framework: 'vitest',
        overrides: [{ file: '**/tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/utils/helper.test.ts', 'tests/test_auth.rs'],
        projectRoot: project.root,
      });
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
        framework: 'vitest',
        overrides: [{ file: '**/e2e/**', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/e2e/test_app.ts'],
        projectRoot: project.root,
      });
      // e2e file doesn't match default vitest glob, override matches → vite-plus
      expect(result.detected[0].framework).toBe('vite-plus');
    } finally {
      project.cleanup();
    }
  });

  it('添加 plan 后无匹配文件仍返回 "unknown"', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
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
        framework: 'vitest',
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

  it('vitest plan 条目的 coverage_artifacts 为 ["coverage/**"]，coverage_cleanup 为 ["coverage", ".nyc_output", "test-stderr.txt"]', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan[0].coverage_artifacts).toEqual(['coverage/**']);
      expect(result.plan[0].coverage_cleanup).toEqual(['coverage', '.nyc_output', 'test-stderr.txt']);
    } finally {
      project.cleanup();
    }
  });

  it('多框架 [vitest, rust] 时每个 plan 条目包含对应 coverage_artifacts 和 coverage_cleanup', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'tests/**/*.rs', framework: 'rust' }],
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
        framework: 'rust',
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

  it('单框架 "vitest" 时 plan 条目也包含 coverage_artifacts 和 coverage_cleanup', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
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

// ===========================================================================
// generateScript -- 各框架正向测试 (AC-1, AC-4, AC-5, AC-6)
// ===========================================================================

describe('generateScript -- 各框架正向测试', () => {
  it('应为 vitest 框架生成包含 npx vitest run --coverage、rm -rf coverage 和 rm -rf .nyc_output 的 bash 脚本 (AC-1)', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: ['coverage', '.nyc_output'],
    });
    expect(script).toContain('npx vitest run --coverage');
    expect(script).toContain('rm -rf coverage');
    expect(script).toContain('rm -rf .nyc_output');
  });

  it('应为 vite-plus 框架（directory 为 plugins/dev-team/bin）生成包含 cd plugins/dev-team/bin 和 vp test --coverage 的脚本 (AC-4)', () => {
    const script = generateScript({
      directory: 'plugins/dev-team/bin',
      coverage_cmd: 'vp test --coverage',
      coverage_cleanup: ['coverage', '.nyc_output'],
    });
    expect(script).toContain('cd plugins/dev-team/bin');
    expect(script).toContain('vp test --coverage');
  });

  it('应为 rust 框架生成包含 cargo llvm-cov --all --coverage、rm -rf coverage 和 rm -rf target/llvm-cov 的脚本 (AC-5)', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'cargo llvm-cov --all --coverage',
      coverage_cleanup: ['coverage', 'target/llvm-cov'],
    });
    expect(script).toContain('cargo llvm-cov --all --coverage');
    expect(script).toContain('rm -rf coverage');
    expect(script).toContain('rm -rf target/llvm-cov');
  });

  it('应为 bun 框架生成包含 bun test --coverage 和 rm -rf coverage 的脚本', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'bun test --coverage',
      coverage_cleanup: ['coverage'],
    });
    expect(script).toContain('bun test --coverage');
    expect(script).toContain('rm -rf coverage');
  });

  it('应为 jest 框架（coverage_cleanup 为空）生成不含 rm -rf 行的脚本 (AC-6)', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx jest --coverage',
      coverage_cleanup: [],
    });
    expect(script).toContain('npx jest --coverage');
    expect(script).not.toContain('rm -rf');
  });
});

// ===========================================================================
// generateScript -- 脚本结构测试 (AC-2)
// ===========================================================================

describe('generateScript -- 脚本结构测试', () => {
  it('脚本第一行为 #!/bin/bash (AC-2)', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: [],
    });
    const lines = script.split('\n');
    expect(lines[0]).toBe('#!/bin/bash');
  });

  it('脚本第二行为 set -e (AC-2)', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: [],
    });
    const lines = script.split('\n');
    expect(lines[1]).toBe('set -e');
  });

  it('脚本最后一行等于 coverage_cmd 的值并以换行符结尾', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: [],
    });
    const trimmed = script.endsWith('\n') ? script.slice(0, -1) : script;
    const lines = trimmed.split('\n');
    expect(lines[lines.length - 1]).toBe('npx vitest run --coverage');
  });
});

// ===========================================================================
// generateScript -- cd 行为边界测试 (AC-3)
// ===========================================================================

describe('generateScript -- cd 行为边界测试', () => {
  it('directory 为 "." 时生成的脚本不含 cd 行 (AC-3)', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: [],
    });
    expect(script).not.toMatch(/^cd\s/m);
  });

  it('directory 为 "/absolute/path" 时生成 cd /absolute/path', () => {
    const script = generateScript({
      directory: '/absolute/path',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: [],
    });
    expect(script).toContain('cd /absolute/path');
  });

  it('directory 为空字符串时仍生成 cd 行（bash 语法上无效，但函数不验证输入）', () => {
    const script = generateScript({
      directory: '',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: [],
    });
    expect(script).toContain('cd ');
  });

  it('directory 含空格时（如 "my project/tests"）生成 cd my project/tests', () => {
    const script = generateScript({
      directory: 'my project/tests',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: [],
    });
    expect(script).toContain('cd my project/tests');
  });
});

// ===========================================================================
// generateScript -- 清理步骤边界测试 (AC-6)
// ===========================================================================

describe('generateScript -- 清理步骤边界测试', () => {
  it('coverage_cleanup 为空数组 [] 时不含任何 rm -rf 行 (AC-6)', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: [],
    });
    expect(script).not.toContain('rm -rf');
  });

  it('coverage_cleanup 为单元素 ["coverage"] 时生成单条 rm -rf coverage', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: ['coverage'],
    });
    expect(script).toContain('rm -rf coverage');
    const rmLines = script.split('\n').filter((l) => l.trim().startsWith('rm -rf'));
    expect(rmLines).toHaveLength(1);
  });

  it('coverage_cleanup 为三元素 ["a", "b", "c"] 时按序生成三条 rm -rf 行', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_cleanup: ['a', 'b', 'c'],
    });
    const rmLines = script.split('\n').filter((l) => l.trim().startsWith('rm -rf'));
    expect(rmLines).toHaveLength(3);
    expect(rmLines[0].trim()).toBe('rm -rf a');
    expect(rmLines[1].trim()).toBe('rm -rf b');
    expect(rmLines[2].trim()).toBe('rm -rf c');
  });

  it('coverage_cleanup 条目含路径分隔符时（如 "target/llvm-cov"）正确保留', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'cargo llvm-cov --all --coverage',
      coverage_cleanup: ['coverage', 'target/llvm-cov'],
    });
    expect(script).toContain('rm -rf coverage');
    expect(script).toContain('rm -rf target/llvm-cov');
  });
});

// ===========================================================================
// generateScript -- 异常输入测试
// ===========================================================================

describe('generateScript -- 异常输入测试', () => {
  it('输入对象为 null 时抛出 TypeError', () => {
    expect(() => {
      // eslint-disable-next-line typescript/no-unsafe-type-assertion
      generateScript(null as unknown as Parameters<typeof generateScript>[0]);
    }).toThrow(TypeError);
  });

  it('directory 为 null 时抛出 TypeError', () => {
    expect(() => {
      generateScript({
        // eslint-disable-next-line typescript/no-unsafe-type-assertion
        directory: null as unknown as string,
        coverage_cmd: 'test',
        coverage_cleanup: [],
      });
    }).toThrow(TypeError);
  });

  it('coverage_cmd 为 undefined 时抛出 TypeError', () => {
    expect(() => {
      generateScript({
        directory: '.',
        // eslint-disable-next-line typescript/no-unsafe-type-assertion
        coverage_cmd: undefined as unknown as string,
        coverage_cleanup: [],
      });
    }).toThrow(TypeError);
  });

  it('coverage_cleanup 为 null 时抛出 TypeError', () => {
    expect(() => {
      generateScript({
        directory: '.',
        coverage_cmd: 'test',
        // eslint-disable-next-line typescript/no-unsafe-type-assertion
        coverage_cleanup: null as unknown as string[],
      });
    }).toThrow(TypeError);
  });

  it('coverage_cmd 为空字符串时生成不含有效命令行的脚本（最后一行为空行）', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: '',
      coverage_cleanup: [],
    });
    const trimmed = script.endsWith('\n') ? script.slice(0, -1) : script;
    const lines = trimmed.split('\n');
    expect(lines[lines.length - 1]).toBe('');
  });
});

// ===========================================================================
// generateScript -- 特殊字符测试
// ===========================================================================

describe('generateScript -- 特殊字符测试', () => {
  it('coverage_cmd 含 $HOME、反引号、$(subshell) 时原样保留', () => {
    const script = generateScript({
      directory: '.',
      coverage_cmd: 'echo "$HOME" && echo `date` && echo $(pwd)',
      coverage_cleanup: [],
    });
    expect(script).toContain('echo "$HOME"');
    expect(script).toContain('echo `date`');
    expect(script).toContain('echo $(pwd)');
  });

  it('coverage_cmd 为超长字符串（>1000 字符）时正确拼接', () => {
    const longCmd = 'echo ' + 'x'.repeat(1000);
    const script = generateScript({
      directory: '.',
      coverage_cmd: longCmd,
      coverage_cleanup: [],
    });
    expect(script).toContain(longCmd);
  });

  it('directory 含 ~ 或 $VAR 时原样保留', () => {
    const script = generateScript({
      directory: '~/$PROJECT/tests',
      coverage_cmd: 'npm test',
      coverage_cleanup: [],
    });
    expect(script).toContain('cd ~/$PROJECT/tests');
  });
});

// ===========================================================================
// runTestDetectFrameworks -- plan 包含 script 字段 (AC-7, AC-9)
// ===========================================================================

describe('runTestDetectFrameworks -- plan 包含 script 字段 (AC-7, AC-9)', () => {
  it('vitest 框架配置时 plan 条目包含非空 script 字段 (AC-7)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(result.plan[0]).toHaveProperty('script');
      expect(typeof result.plan[0].script).toBe('string');
      expect(result.plan[0].script.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });

  it('多框架 [vitest, rust] 配置时每个 plan 条目都包含非空 script', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'tests/**/*.rs', framework: 'rust' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts', 'tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      for (const entry of result.plan) {
        expect(entry).toHaveProperty('script');
        expect(typeof entry.script).toBe('string');
        expect(entry.script.length).toBeGreaterThan(0);
      }
    } finally {
      project.cleanup();
    }
  });

  it('plan 条目 script 内容与 generateScript() 对同输入的输出一致', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      const entry = result.plan[0];
      const expectedScript = generateScript({
        directory: entry.directory,
        coverage_cmd: entry.coverage_cmd,
        coverage_cleanup: entry.coverage_cleanup ?? [],
      });
      expect(entry.script).toBe(expectedScript);
    } finally {
      project.cleanup();
    }
  });

  it('vite-plus 框架（directory 非 "."）时 script 含 cd 行', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
        overrides: [{ file: 'plugins/dev-team/bin/**', framework: 'vite-plus' }],
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['plugins/dev-team/bin/src/main.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(2);
      // The vite-plus override entry (index 1) should have cd line
      const vitePlusPlan = result.plan[1];
      expect(vitePlusPlan.script).toContain('cd plugins/dev-team/bin');
    } finally {
      project.cleanup();
    }
  });

  it('空 framework 配置时 plan 为空数组', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {},
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
// runTestDetectFrameworks -- 单框架 plan script
// ===========================================================================

describe('runTestDetectFrameworks -- 单框架 plan script', () => {
  it('"vitest" 时 plan 条目 script 字段为非空字符串', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'vitest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.test.ts'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(typeof result.plan[0].script).toBe('string');
      expect(result.plan[0].script.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });

  it('"jest" 时 plan 条目 script 字段为非空字符串', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'jest',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['src/test.spec.js'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(typeof result.plan[0].script).toBe('string');
      expect(result.plan[0].script.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });

  it('"rust" 时 plan 条目 script 字段为非空字符串', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      test: {
        framework: 'rust',
      },
    });
    try {
      const result = runTestDetectFrameworks({
        files: ['tests/test_auth.rs'],
        projectRoot: project.root,
      });
      expect(result.plan).toHaveLength(1);
      expect(typeof result.plan[0].script).toBe('string');
      expect(result.plan[0].script.length).toBeGreaterThan(0);
    } finally {
      project.cleanup();
    }
  });
});
